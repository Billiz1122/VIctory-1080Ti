//! Host work that a host library records into a title's command list in
//! place of a console one (the host's FidelityFX upscaler, say): the host
//! device, a title texture's host copy in the state the library is told,
//! and the host list itself.

use windows::Win32::Graphics::Direct3D12::{
    ID3D12Device, ID3D12GraphicsCommandList, ID3D12Resource,
};

use crate::object::Object;

/// The host D3D12 device, created on first use; `None` without a host GPU.
pub fn host_device() -> Option<ID3D12Device> {
    crate::host::host().map(|h| h.device.clone())
}

/// A title texture's host copy, moved to the host resource `state` in the
/// title list `list`, and the state it is in: `state` without the bits the
/// host copy cannot take (see `host::legal_state`). What the list records
/// next finds it there and must leave it there. The list holds the texture
/// until it is reset. `None` for a buffer, or without a host list.
pub fn host_texture(
    list: *const Object,
    resource: *const Object,
    state: u32,
) -> Option<(ID3D12Resource, u32)> {
    let host = crate::list::host_list(list)?;
    let texture = crate::resource::host_resource(resource)?.clone();
    crate::list::ensure_state(host, resource, state);
    let state = crate::host::legal_state(&texture, state);
    Some((texture, state))
}

pub use crate::host::framegen::Values as FrameValues;

/// The title made its upscaler, with these flags and this largest render
/// size: what frames generated between the title's go by
/// (`crate::host::framegen`).
pub fn upscaler(flags: u32, max_render: [u32; 2]) {
    crate::host::framegen::upscaler(flags, max_render);
}

/// A frame's depth and motion vectors at the upscaling's place in the title
/// list `list`: host textures ([`host_texture`]) in the host states given,
/// with the upscaler's descriptions of the two and the values they are read
/// with. Copied there for the frames generated between the title's; nothing
/// without them.
pub fn frame_inputs(
    list: *const Object,
    depth: (&ID3D12Resource, u32),
    motion: (&ID3D12Resource, u32),
    descriptions: [[u32; 8]; 2],
    values: FrameValues,
) {
    let Some(host) = crate::host::host() else {
        return;
    };
    crate::list::with_state(list, |host_list, _| {
        crate::host::framegen::inputs(&host.device, host_list, depth, motion, descriptions, values);
    });
}

/// Runs `f` with the list that records the commands of the title list `list`
/// (open for recording), in order with them: `list` itself, now; or, while
/// list calls are written down and translated by threads of ours
/// (`VICTORY_THIN`), the list of ours they go to, when the translation gets
/// to this place. So `f` takes with it what it reads of the title's
/// arguments, and finds the host list with [`host_texture`] and [`record`]
/// on the list it is given.
pub fn in_order(list: *const Object, f: impl FnOnce(*const Object) + Send + 'static) {
    if crate::list::thin::on() {
        // (A list not being written down, closed or not begun, records nothing.)
        let _ = crate::list::thin::external(list, Box::new(f));
        return;
    }
    f(list)
}

/// Runs `f` with the host list behind the title list `list` (open for
/// recording); `None` without one. What `f` binds (descriptor heaps, root
/// signatures, pipeline) is replaced before the list's next draw or
/// dispatch.
pub fn record<R>(
    list: *const Object,
    f: impl FnOnce(&ID3D12GraphicsCommandList) -> R,
) -> Option<R> {
    let mut out = None;
    crate::list::with_state(list, |host, state| {
        out = Some(f(&host.list));
        state.host_state_lost();
    });
    out
}
