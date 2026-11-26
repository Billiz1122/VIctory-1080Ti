//! The vertex shader that draws what a mesh program run as compute wrote
//! ([`super::MeshLayout`]), reading `u0`-`u2` as `t0`-`t2` in
//! [`MESH_OUT_SPACE`]. Its outputs are the mesh program's output
//! signature, so the title's pixel shader (which links by semantic) stays
//! as it is.
//!
//! * Without per-primitive outputs it is drawn indexed (`u1` the index
//!   buffer): vertex `SV_VertexID` is vertex record `SV_VertexID`.
//! * With them, it is drawn unindexed, three vertices per primitive:
//!   vertex `SV_VertexID` is corner `SV_VertexID % 3` of primitive
//!   `SV_VertexID / 3`. It reads the corner's vertex index from `t1` and
//!   writes the primitive's record from `t2` as constant-interpolated
//!   outputs after the vertex outputs: the same value at every corner.
//! * Unindexed too for a pixel shader translated for a host without
//!   barycentrics (`lower::with_emulated_barycentrics`), whose inputs of
//!   its own it writes: the corner's weights, and the elements the pixel
//!   shader reads at the primitive's second and third vertex.

use super::bitcode::{Attr, Bin, Cast, Term, V, pred};
use super::lower::EMULATED;
use super::container::SigElement;
use super::shader::{MESH_OUT_SPACE, Shader};

/// Writes the four-dword record elements `elements` of the raw buffer
/// `t<register>` at byte `base` as outputs `first..`.
fn copy_record(s: &mut Shader, register: u8, base: V, elements: &[SigElement], first: usize) {
    let e = s.entry;
    let i32t = s.t.i32;
    let h = s.mesh_in_handle(register);
    for (k, el) in elements.iter().enumerate() {
        for col in 0..el.cols {
            let at = s.m.i32((k as u32 * 4 + u32::from(col)) * 4);
            let off = s.m.bin(e, Bin::Add, base, at);
            let x = load(s, h, off);
            let (suffix, x) = if el.comp == 9 {
                (".f32", x)
            } else {
                (".i32", s.m.cast(e, Cast::Bitcast, x, i32t))
            };
            let args = [s.m.i32((first + k) as u32), s.m.i32(0), s.m.i8(col), x];
            let void = s.t.void;
            s.op(
                e,
                &format!("storeOutput{suffix}"),
                Attr::NoUnwind,
                void,
                5,
                &args,
            );
        }
    }
}

/// The dword at byte `off` of raw buffer `h`, as a float.
fn load(s: &mut Shader, h: V, off: V) -> V {
    let e = s.entry;
    let (u, mask, al) = (s.m.undef(s.t.i32), s.m.i8(1), s.m.i32(4));
    let r = s.op(
        e,
        "rawBufferLoad.f32",
        Attr::ReadOnly,
        s.t.res_ret,
        139,
        &[h, off, u, mask, al],
    );
    s.m.extract(e, r, 0)
}

/// The vertex shader for a mesh program with these output and
/// per-primitive elements (in signature order, as the program was lowered
/// with) and record sizes in dwords; unindexed when `prims` is not empty.
pub(crate) fn vertex_shader(
    outputs: &[SigElement],
    prims: &[SigElement],
    vertex_dwords: u32,
    primitive_dwords: u32,
) -> Vec<u8> {
    vertex_shader_with(outputs, prims, vertex_dwords, primitive_dwords, &[])
        .expect("without inputs of the pixel shader's own nothing is refused")
}

/// [`vertex_shader`] that also writes `emulated`, the inputs a pixel shader
/// reads in place of barycentrics ([`EMULATED`], at the registers it has
/// them); unindexed then too. Refused if one of them is of an element the
/// mesh program does not write per vertex, or lies on a register taken.
pub(crate) fn vertex_shader_with(
    outputs: &[SigElement],
    prims: &[SigElement],
    vertex_dwords: u32,
    primitive_dwords: u32,
    emulated: &[SigElement],
) -> Result<Vec<u8>, &'static str> {
    let mut s = Shader::new(false);
    s.inputs = vec![SigElement {
        name: "SV_VertexID".into(),
        index: 0,
        kind: 1,
        comp: 5,
        interp: 0,
        row: Some(0),
        col: 0,
        cols: 1,
        usage: 1,
        half: false,
    }];
    let e = s.entry;
    let i32t = s.t.i32;
    let args = [s.m.i32(0), s.m.i32(0), s.m.i8(0), s.m.undef(i32t)];
    let id = s.op(e, "loadInput.i32", Attr::ReadNone, i32t, 4, &args);
    let unindexed = !prims.is_empty() || !emulated.is_empty();
    let vertex = if !unindexed {
        id
    } else {
        // The corner's index: triangles are three consecutive dwords.
        let four = s.m.i32(4);
        let at = s.m.bin(e, Bin::Mul, id, four);
        let h = s.mesh_in_handle(1);
        let x = load(&mut s, h, at);
        s.m.cast(e, Cast::Bitcast, x, i32t)
    };
    let record = s.m.i32(vertex_dwords * 4);
    let base = s.m.bin(e, Bin::Mul, vertex, record);
    copy_record(&mut s, 0, base, outputs, 0);
    // Per-primitive outputs go in rows after the vertex outputs, constant.
    let first_row = outputs
        .iter()
        .filter_map(|o| o.row.map(|r| r + 1))
        .max()
        .unwrap_or(0);
    let low = prims.iter().filter_map(|p| p.row).min().unwrap_or(0);
    let mut placed: Vec<SigElement> = prims
        .iter()
        .cloned()
        .map(|mut p| {
            p.row = p.row.map(|r| first_row + r - low);
            p.interp = 1;
            p
        })
        .collect();
    if !prims.is_empty() {
        let three = s.m.i32(3);
        let primitive = s.m.bin(e, Bin::UDiv, id, three);
        let record = s.m.i32(primitive_dwords * 4);
        let base = s.m.bin(e, Bin::Mul, primitive, record);
        copy_record(&mut s, 2, base, prims, outputs.len());
    }
    let mut all: Vec<SigElement> = outputs.to_vec();
    all.append(&mut placed);
    for x in emulated {
        // (Several of them share a register, column by column.)
        let own = |o: &SigElement| o.kind == 0 && EMULATED.contains(&o.name.as_str());
        let taken = |o: &SigElement| o.row == x.row && (!own(o) || (o.col < x.col + x.cols && x.col < o.col + o.cols));
        if all.iter().any(taken) {
            return Err("a register of the pixel shader's emulated inputs is taken");
        }
        let three = s.m.i32(3);
        let void = s.t.void;
        let at = all.len() as u32;
        if x.name == EMULATED[0] {
            // The corner's weight is one, the other two none.
            let corner = s.m.bin(e, Bin::URem, id, three);
            let (one, none) = (s.m.f32(1f32.to_bits()), s.m.f32(0));
            for col in 0..x.cols.min(3) {
                let this = s.m.i32(u32::from(col));
                let is = s.m.cmp(e, pred::EQ, corner, this);
                let w = s.m.select(e, is, one, none);
                let args = [s.m.i32(at), s.m.i32(0), s.m.i8(col), w];
                s.op(e, "storeOutput.f32", Attr::NoUnwind, void, 5, &args);
            }
        } else {
            // The element at the primitive's vertex 1 or 2: its register row
            // and first column are in the semantic index.
            let k = if x.name == EMULATED[1] { 1 } else { 2 };
            let (row, first) = (x.index / 4, (x.index % 4) as u8);
            let Some((n, of)) = outputs
                .iter()
                .enumerate()
                .find(|(_, o)| o.row == Some(row) && o.col <= first && first + x.cols <= o.col + o.cols)
            else {
                return Err("an input read at a vertex that the mesh program does not write per vertex");
            };
            // That vertex's index: `t1` holds three a primitive.
            let primitive = s.m.bin(e, Bin::UDiv, id, three);
            let first_index = s.m.bin(e, Bin::Mul, primitive, three);
            let kth = s.m.i32(k);
            let index_at = s.m.bin(e, Bin::Add, first_index, kth);
            let four = s.m.i32(4);
            let index_byte = s.m.bin(e, Bin::Mul, index_at, four);
            let h1 = s.mesh_in_handle(1);
            let other = load(&mut s, h1, index_byte);
            let other = s.m.cast(e, Cast::Bitcast, other, i32t);
            let other_base = s.m.bin(e, Bin::Mul, other, record);
            let h0 = s.mesh_in_handle(0);
            for col in 0..x.cols {
                let dword = n as u32 * 4 + u32::from(first - of.col + col);
                let byte = s.m.i32(dword * 4);
                let off = s.m.bin(e, Bin::Add, other_base, byte);
                let v = load(&mut s, h0, off);
                let (suffix, v) = if x.comp == 9 {
                    (".f32", v)
                } else {
                    (".i32", s.m.cast(e, Cast::Bitcast, v, i32t))
                };
                let args = [s.m.i32(at), s.m.i32(0), s.m.i8(col), v];
                s.op(e, &format!("storeOutput{suffix}"), Attr::NoUnwind, void, 5, &args);
            }
        }
        all.push(x.clone());
    }
    s.outputs = all
        .into_iter()
        .map(|mut el| {
            el.usage = (1 << el.cols) - 1;
            el
        })
        .collect();
    s.m.terminate(e, Term::Ret);
    debug_assert_eq!(MESH_OUT_SPACE, super::MESH_OUT_SPACE);
    Ok(s.finish())
}
