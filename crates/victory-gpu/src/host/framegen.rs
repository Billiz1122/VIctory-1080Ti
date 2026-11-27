//! Frames generated between the title's (`VICTORY_FRAME_GENERATION=1`): AMD
//! FidelityFX frame generation, the PC build's
//! `amd_fidelityfx_framegeneration_dx12.dll` in `VICTORY_FIDELITYFX_DIR`,
//! beside the upscaler that serves the title's (`victory-hle`'s
//! `fidelityfx`). A title that makes twelve frames a second on this host
//! shows twenty-four: every second one is put together from its two
//! neighbours, their depth and their motion vectors. What the title answers
//! to is no sooner for it, and a frame is shown one later.
//!
//! How it is wired:
//! * The window's swap chain is made by the library ([`swap_chain`]): a
//!   proxy that holds the real one. We fill its back buffer and present as
//!   before; a thread of its own shows the generated frame, then ours.
//! * It needs each frame's depth and motion vectors. The title hands both
//!   to its upscaler: at that place in the title's list they are copied into
//!   textures of ours ([`inputs`]), and the values that go with them wait
//!   for the frame's present.
//! * At the present ([`frame`], on the thread that presents) the library is
//!   told about the frame and prepares from our copies, in the list that
//!   fills the back buffer. So every call into it comes from one thread,
//!   frame after frame: lists are translated ahead of their submission
//!   here, and the library goes by the order it is called in.
//!
//! The descriptions are the FidelityFX SDK's (2.4.0, MIT): `ffx_api.h`,
//! `ffx_framegeneration.h`, `dx12/ffx_api_framegeneration_dx12.h`.
//!
//! shortcut: no picture without the title's interface is given, so the
//! interface is generated with the rest (it smears where it lies over a
//! moving scene), and the camera's place and axes are not known (the origin
//! and unit axes are given); pass both when a title's upscaling has them.

use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::Interface;

use super::{HostList, transition};

const DLL: &str = "amd_fidelityfx_framegeneration_dx12.dll";

// Description types.
const BACKEND: u64 = 2;
const GET_VERSIONS: u64 = 4;
const CREATE: u64 = 0x2_0001;
const CONFIGURE: u64 = 0x2_0002;
const PREPARE: u64 = 0x2_000c;
const VERSION: u64 = 0x2_000e;
const SWAP_FOR_HWND: u64 = 0x3_0006;
const SWAP_KEY_VALUE: u64 = 0x3_0008;
const SWAP_VERSION: u64 = 0x3_000b;
/// The versions of the two interfaces these descriptions are of.
const API: [u32; 2] = [4 << 22 | 2, 4 << 22];

#[repr(C)]
#[derive(Clone, Copy)]
struct Header {
    kind: u64,
    next: *mut Header,
}

#[repr(C)]
struct Backend {
    header: Header,
    device: *mut c_void,
}

#[repr(C)]
struct Version {
    header: Header,
    version: u32,
}

#[repr(C)]
struct GetVersions {
    header: Header,
    create_kind: u64,
    device: *mut c_void,
    count: *mut u64,
    ids: *mut u64,
    names: *mut *const std::ffi::c_char,
}

/// A frame generation context: flags, the sizes shown and rendered, the back
/// buffer's format.
#[repr(C)]
struct Create {
    header: Header,
    flags: u32,
    display: [u32; 2],
    max_render: [u32; 2],
    format: u32,
}

/// A swap chain for a window, made by the library.
#[repr(C)]
struct SwapForHwnd {
    header: Header,
    swap_chain: *mut *mut c_void,
    hwnd: HWND,
    desc: *const DXGI_SWAP_CHAIN_DESC1,
    fullscreen: *const c_void,
    factory: *mut c_void,
    queue: *mut c_void,
}

/// A setting of the swap chain's by its key.
#[repr(C)]
struct KeyValue {
    header: Header,
    key: u64,
    value: u64,
    pointer: *mut c_void,
}

/// How the swap chain's thread waits between the two frames it shows
/// (key 2): a margin in milliseconds, a share of the frame times' variance,
/// whether it may sleep for most of the wait and how many timer ticks it
/// stays awake before the time, whether it may wait for the card asleep.
#[repr(C)]
struct Pacing {
    margin: f32,
    variance: f32,
    sleeps: bool,
    awake: u32,
    waits_asleep: bool,
}

/// A resource, its description (type, format, sizes, mips, flags, usage) and
/// the state it is in.
#[repr(C)]
#[derive(Clone, Copy)]
struct Resource {
    resource: *mut c_void,
    description: [u32; 8],
    state: u32,
}

/// A frame's settings: the swap chain, who generates, whether to.
#[repr(C)]
struct Configure {
    header: Header,
    swap_chain: *mut c_void,
    present: usize,
    present_context: *mut c_void,
    generate: unsafe extern "C" fn(*mut Header, *mut c_void) -> u32,
    generate_context: *mut c_void,
    enabled: bool,
    async_work: bool,
    hudless: Resource,
    flags: u32,
    only_generated: bool,
    rect: [i32; 4],
    frame: u64,
}

/// A frame's depth and motion vectors, and the values they are read with.
#[repr(C)]
struct Prepare {
    header: Header,
    frame: u64,
    flags: u32,
    list: *mut c_void,
    render: [u32; 2],
    jitter: [f32; 2],
    motion_scale: [f32; 2],
    frame_time: f32,
    reset: bool,
    near: f32,
    far: f32,
    fov: f32,
    to_meters: f32,
    depth: Resource,
    motion: Resource,
    /// Position, up, right, forward.
    camera: [[f32; 3]; 4],
}

const _: () = assert!(size_of::<Create>() == 40);
const _: () = assert!(size_of::<SwapForHwnd>() == 64);
const _: () = assert!(size_of::<KeyValue>() == 40 && size_of::<Pacing>() == 20);
const _: () = assert!(size_of::<Resource>() == 48);
const _: () = assert!(size_of::<Configure>() == 144);
const _: () = assert!(size_of::<Prepare>() == 232);
const _: () = assert!(std::mem::offset_of!(Configure, hudless) == 64 && std::mem::offset_of!(Configure, frame) == 136);
const _: () = assert!(std::mem::offset_of!(Prepare, depth) == 88 && std::mem::offset_of!(Prepare, camera) == 184);

type CreateFn = unsafe extern "C" fn(*mut *mut c_void, *mut Header, *const c_void) -> u32;
type DestroyFn = unsafe extern "C" fn(*mut *mut c_void, *const c_void) -> u32;
/// Configure, query and dispatch.
type Call = unsafe extern "C" fn(*mut *mut c_void, *mut Header) -> u32;

struct Api {
    create: CreateFn,
    destroy: DestroyFn,
    configure: Call,
    query: Call,
    dispatch: Call,
}

/// The library, if frames are to be generated and it is there.
fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| {
        if std::env::var_os("VICTORY_FRAME_GENERATION").is_none_or(|v| v != "1") {
            return None;
        }
        let Some(dir) = std::env::var_os("VICTORY_FIDELITYFX_DIR") else {
            tracing::warn!("VICTORY_FRAME_GENERATION=1 without VICTORY_FIDELITYFX_DIR: no frames are generated");
            return None;
        };
        match load(std::path::Path::new(&dir)) {
            Ok(api) => Some(api),
            Err(why) => {
                tracing::warn!("frame generation: {why}; no frames are generated");
                None
            }
        }
    })
    .as_ref()
}

fn load(dir: &std::path::Path) -> Result<Api, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
    };
    // The search flags below want a full path.
    let path = std::path::absolute(dir.join(DLL)).map_err(|e| e.to_string())?;
    if !path.is_file() {
        return Err(format!("no {DLL} in {}", dir.display()));
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // Its own folder and System32 only, never the current folder.
    let search = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
    // SAFETY: a NUL-terminated path to the user's FidelityFX build.
    let module = unsafe { LoadLibraryExW(wide.as_ptr(), null_mut(), search) };
    if module.is_null() {
        return Err(format!("{DLL} did not load ({})", std::io::Error::last_os_error()));
    }
    let find = |name: &std::ffi::CStr| {
        // SAFETY: a loaded module and a NUL-terminated name.
        unsafe { GetProcAddress(module, name.as_ptr().cast()) }
            .map(|f| f as *const c_void)
            .ok_or_else(|| format!("{DLL} has no {}", name.to_string_lossy()))
    };
    // SAFETY: the API's exports, with its signatures.
    let api = unsafe {
        Api {
            create: std::mem::transmute::<*const c_void, CreateFn>(find(c"ffxCreateContext")?),
            destroy: std::mem::transmute::<*const c_void, DestroyFn>(find(c"ffxDestroyContext")?),
            configure: std::mem::transmute::<*const c_void, Call>(find(c"ffxConfigure")?),
            query: std::mem::transmute::<*const c_void, Call>(find(c"ffxQuery")?),
            dispatch: std::mem::transmute::<*const c_void, Call>(find(c"ffxDispatch")?),
        }
    };
    let device = super::host().ok_or("there is no host GPU")?.device.as_raw();
    let mut count = 0u64;
    let mut query = GetVersions {
        header: Header { kind: GET_VERSIONS, next: null_mut() },
        create_kind: CREATE,
        device,
        count: &mut count,
        ids: null_mut(),
        names: null_mut(),
    };
    // SAFETY: a version query naming the host device.
    let code = unsafe { (api.query)(null_mut(), &mut query.header) };
    if code != 0 || count == 0 {
        return Err(format!("{DLL} offers no frame generation for the host GPU (code {code})"));
    }
    let mut names = vec![null::<std::ffi::c_char>(); count as usize];
    query.names = names.as_mut_ptr();
    // SAFETY: as above, with room for `count` names.
    unsafe { (api.query)(null_mut(), &mut query.header) };
    let versions: Vec<String> = names
        .iter()
        .filter(|n| !n.is_null())
        // SAFETY: NUL-terminated names the library keeps.
        .map(|&n| unsafe { std::ffi::CStr::from_ptr(n) }.to_string_lossy().into_owned())
        .collect();
    tracing::info!(versions = versions.join(", "), "frame generation: the host's FidelityFX generates frames between the title's (VICTORY_FRAME_GENERATION=1)");
    Ok(api)
}

/// The values a frame's depth and motion vectors are read with, as the
/// title gives them to its upscaler.
#[derive(Clone, Copy, Debug)]
pub struct Values {
    pub render: [u32; 2],
    pub jitter: [f32; 2],
    pub motion_scale: [f32; 2],
    /// Milliseconds since the frame before.
    pub frame_time: f32,
    pub reset: bool,
    pub near: f32,
    pub far: f32,
    pub fov: f32,
    pub to_meters: f32,
}

/// What a frame's present prepares from.
struct Inputs {
    depth: ID3D12Resource,
    motion: ID3D12Resource,
    descriptions: [[u32; 8]; 2],
    values: Values,
}

struct State {
    /// The library's context of the swap chain (none: the swap chain is
    /// not its).
    swap: *mut c_void,
    /// What the frame generation context ([`CONTEXT`]) was made for: the
    /// size shown, its format, the largest size rendered, the flags.
    made_for: Option<([u32; 2], u32, [u32; 2], u32)>,
    /// The title's upscaler: its flags and the largest size it renders.
    upscaler: Option<(u32, [u32; 2])>,
    /// Our copies of a frame's depth and motion vectors: copy destinations
    /// but in the list that prepares from them.
    copies: Option<(ID3D12Resource, ID3D12Resource)>,
    /// Frames upscaled and not presented yet, oldest first.
    waiting: VecDeque<Inputs>,
    frame: u64,
    /// For the line of what is shown: when it was last written, and the
    /// swap chain's count of presents and ours then.
    shown: Option<(std::time::Instant, u32, u64)>,
}

// SAFETY: D3D12 objects are free-threaded; the context is the library's,
// called under the lock.
unsafe impl Send for State {}

static STATE: Mutex<State> =
    Mutex::new(State { swap: null_mut(), made_for: None, upscaler: None, copies: None, waiting: VecDeque::new(), frame: 0, shown: None });

/// Whether a frame was generated before the title's last one: twice its
/// frames are shown (the window's caption says so).
static GENERATING: AtomicBool = AtomicBool::new(false);

pub(super) fn generating() -> bool {
    GENERATING.load(Ordering::Relaxed)
}

/// The frame generation context. The library generates through a pointer to
/// it ([`generate`]), from inside its swap chain's present.
static CONTEXT: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

/// The library's debug drawing (`VICTORY_FRAME_GENERATION_DEBUG=<bits>`,
/// diagnostics builds: 1 bars at the picture's sides that change with every
/// frame shown, 4 a view of what it generates from; 256, ours: only the
/// generated frames are shown, to look at them).
fn debug_flags() -> u32 {
    static FLAGS: OnceLock<u32> = OnceLock::new();
    *FLAGS.get_or_init(|| {
        let asked = std::env::var("VICTORY_FRAME_GENERATION_DEBUG").ok().and_then(|v| v.trim().parse().ok());
        asked.filter(|_| cfg!(feature = "diagnostics")).unwrap_or(0)
    })
}

/// Generates a frame: the library's swap chain asks for it at a present.
unsafe extern "C" fn generate(what: *mut Header, context: *mut c_void) -> u32 {
    match api() {
        // SAFETY: the library's own description and the context it was
        // configured with.
        Some(api) => unsafe { (api.dispatch)(context.cast(), what) },
        None => 1,
    }
}

/// The swap chain for `hwnd`, made by the library around one of `desc` on
/// `queue`; `None` if no frames are generated (then the caller makes one).
pub(super) fn swap_chain(
    factory: &IDXGIFactory2,
    queue: &ID3D12CommandQueue,
    hwnd: HWND,
    desc: &DXGI_SWAP_CHAIN_DESC1,
) -> Option<IDXGISwapChain3> {
    let api = api()?;
    let mut made: *mut c_void = null_mut();
    let mut context: *mut c_void = null_mut();
    let mut version = Version { header: Header { kind: SWAP_VERSION, next: null_mut() }, version: API[1] };
    let mut create = SwapForHwnd {
        header: Header { kind: SWAP_FOR_HWND, next: &raw mut version.header },
        swap_chain: &raw mut made,
        hwnd,
        desc,
        fullscreen: null(),
        factory: factory.as_raw(),
        queue: queue.as_raw(),
    };
    // SAFETY: a chain of descriptions that live for the call; the factory,
    // queue and window are ours.
    let code = unsafe { (api.create)(&mut context, &raw mut create.header, null()) };
    if code != 0 || made.is_null() {
        tracing::warn!(code, "frame generation: the library made no swap chain; no frames are generated");
        return None;
    }
    // SAFETY: the swap chain the library made, a reference of ours.
    let chain = unsafe { IDXGISwapChain4::from_raw(made) }.cast::<IDXGISwapChain3>().ok()?;
    // Its thread waits asleep between the frames it shows. As it comes it
    // waits awake, on a processor of its own: with it the thread that
    // submits did the same work in 100 ms a frame where it took 78, and the
    // title's frames were 97 ms where they were 87 (Ryzen 5 2600X, six
    // cores, the prologue's cell, 2026-11-27).
    let mut pacing = Pacing { margin: 0.1, variance: 0.1, sleeps: true, awake: 2, waits_asleep: true };
    let mut set = KeyValue {
        header: Header { kind: SWAP_KEY_VALUE, next: null_mut() },
        key: 2,
        value: 0,
        pointer: (&raw mut pacing).cast(),
    };
    // SAFETY: the swap chain's context and a description that lives for the call.
    let code = unsafe { (api.configure)(&mut context, &raw mut set.header) };
    if code != 0 {
        tracing::warn!(code, "frame generation: the swap chain's thread waits awake (the library refused the setting)");
    }
    STATE.lock().unwrap_or_else(|e| e.into_inner()).swap = context;
    tracing::info!(width = desc.Width, height = desc.Height, "frame generation: the window's swap chain is the library's");
    Some(chain)
}

/// The title made its upscaler: its flags (depth inverted or infinite,
/// motion vectors jittered or of the size shown are the same bits here) and
/// the largest size it renders.
pub(crate) fn upscaler(flags: u32, max_render: [u32; 2]) {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).upscaler = Some((flags, max_render));
}

/// A texture like `source`, a copy destination.
fn copy_of(device: &ID3D12Device, source: &ID3D12Resource) -> Option<ID3D12Resource> {
    let mut made: Option<ID3D12Resource> = None;
    // SAFETY: plain creation of a texture with a live one's description.
    let result = unsafe {
        device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() },
            D3D12_HEAP_FLAG_NONE,
            &source.GetDesc(),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
            &mut made,
        )
    };
    if let Err(e) = result {
        tracing::warn!(error = %e, "frame generation: no copy of a frame's depth or motion vectors");
    }
    made
}

/// Whether `a` can take a copy of all of `b`.
fn like(a: &ID3D12Resource, b: &ID3D12Resource) -> bool {
    // SAFETY: live resources.
    let (a, b) = unsafe { (a.GetDesc(), b.GetDesc()) };
    (a.Width, a.Height, a.Format, a.DepthOrArraySize, a.MipLevels) == (b.Width, b.Height, b.Format, b.DepthOrArraySize, b.MipLevels)
}

/// At the upscaling's place in a title list (`list` records it): the frame's
/// `depth` and `motion` vectors, host textures in the host states given
/// (they stay in them), are copied for the frame's present, and
/// `descriptions` (the title's, of the two) and `values` wait with them.
pub(crate) fn inputs(
    device: &ID3D12Device,
    list: &HostList,
    depth: (&ID3D12Resource, u32),
    motion: (&ID3D12Resource, u32),
    descriptions: [[u32; 8]; 2],
    values: Values,
) {
    if api().is_none() {
        return;
    }
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.swap.is_null() {
        return;
    }
    if !state.copies.as_ref().is_some_and(|c| like(&c.0, depth.0) && like(&c.1, motion.0)) {
        state.copies = copy_of(device, depth.0).zip(copy_of(device, motion.0));
    }
    let Some((depth_copy, motion_copy)) = state.copies.clone() else {
        return;
    };
    let l: &ID3D12GraphicsCommandList = &list.list;
    for ((source, bits), copy) in [(depth, &depth_copy), (motion, &motion_copy)] {
        let from = D3D12_RESOURCE_STATES(bits as i32);
        let moved = from != D3D12_RESOURCE_STATE_COPY_SOURCE;
        // SAFETY: an open list, a live texture in `from`, and ours, a copy
        // destination until its present's list.
        unsafe {
            if moved {
                l.ResourceBarrier(&[transition(source, from, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
            }
            l.CopyResource(copy, source);
            if moved {
                l.ResourceBarrier(&[transition(source, D3D12_RESOURCE_STATE_COPY_SOURCE, from)]);
            }
        }
        list.keep(copy);
    }
    state.waiting.push_back(Inputs { depth: depth_copy, motion: motion_copy, descriptions, values });
}

/// At a present, first thing in `list` (which fills the back buffer of
/// `chain` and runs before the present): tells the library about the frame
/// and, if the title upscaled one, has it prepare from that frame's depth
/// and motion vectors. `size` and `format` are the back buffer's.
pub(super) fn frame(device: &ID3D12Device, chain: &IDXGISwapChain3, list: &HostList, size: [u32; 2], format: DXGI_FORMAT) {
    let Some(api) = api() else {
        return;
    };
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.swap.is_null() {
        return;
    }
    state.frame += 1;
    // What reaches the screen: the swap chain counts the generated frames'
    // presents with ours.
    let now = std::time::Instant::now();
    // SAFETY: a live swap chain.
    let count = unsafe { chain.GetLastPresentCount() }.unwrap_or(0);
    match state.shown {
        Some((then, counted, frame)) if now.duration_since(then).as_secs() >= 5 => {
            let seconds = now.duration_since(then).as_secs_f64();
            tracing::info!(
                "frame generation: {:.1} frames shown a second, {:.1} of them the title's",
                f64::from(count.wrapping_sub(counted)) / seconds,
                (state.frame - frame) as f64 / seconds
            );
            state.shown = Some((now, count, state.frame));
        }
        None => state.shown = Some((now, count, state.frame)),
        _ => {}
    }
    // (The title is a frame ahead of its presents at most: what is older was
    // never shown.)
    while state.waiting.len() > 2 {
        state.waiting.pop_front();
    }
    let inputs = state.waiting.pop_front();
    let surface = match format {
        DXGI_FORMAT_R16G16B16A16_FLOAT => 4,
        DXGI_FORMAT_R8G8B8A8_UNORM => 10,
        DXGI_FORMAT_B8G8R8A8_UNORM => 14,
        DXGI_FORMAT_R10G10B10A2_UNORM => 17,
        _ => 0,
    };
    let context = CONTEXT.as_ptr();
    if let (Some(i), Some((flags, max_render))) = (&inputs, state.upscaler) {
        let render = [max_render[0].max(i.values.render[0]), max_render[1].max(i.values.render[1])];
        // Depth inverted or infinite, motion vectors jittered or of the size
        // shown: the upscaler's bits. (Not its high dynamic range: that is
        // of what it reads; the back buffer is what is shown.)
        let wanted = (size, surface, render, flags & 0x1e);
        if surface != 0 && state.made_for != Some(wanted) {
            // SAFETY: the context made below, or none; its swap chain's
            // presents are not waited for: a context is made anew when the
            // back buffer changes, after `settle_now`.
            unsafe {
                if !(*context).is_null() {
                    (api.destroy)(context, null());
                    *context = null_mut();
                }
            }
            let mut backend = Backend { header: Header { kind: BACKEND, next: null_mut() }, device: device.as_raw() };
            let mut version = Version { header: Header { kind: VERSION, next: &raw mut backend.header }, version: API[0] };
            let mut create = Create {
                header: Header { kind: CREATE, next: &raw mut version.header },
                flags: wanted.3,
                display: size,
                max_render: render,
                format: surface,
            };
            // SAFETY: a chain of descriptions that live for the call.
            let code = unsafe { (api.create)(context, &raw mut create.header, null()) };
            state.made_for = (code == 0).then_some(wanted);
            if code == 0 {
                tracing::info!(shown = ?size, rendered = ?render, flags = format_args!("{:#x}", wanted.3), "frame generation: a context for the frames shown");
            } else {
                tracing::warn!(code, shown = ?size, rendered = ?render, "frame generation: the library made no context; no frames are generated");
                // SAFETY: ours alone, under the lock.
                unsafe { *context = null_mut() };
            }
        }
    }
    // SAFETY: read under the lock that every write of it is under.
    if unsafe { *context }.is_null() {
        return;
    }
    let on = inputs.is_some() && state.made_for.is_some_and(|m| m.0 == size && m.1 == surface);
    // (By turns in one run, to measure what generating costs the title's frames.)
    #[cfg(feature = "diagnostics")]
    let on = on && crate::ab::pick("VICTORY_FRAME_GENERATION").unwrap_or(true);
    let mut configure = Configure {
        header: Header { kind: CONFIGURE, next: null_mut() },
        swap_chain: chain.as_raw(),
        present: 0,
        present_context: null_mut(),
        generate,
        generate_context: context.cast(),
        enabled: on,
        async_work: false,
        hudless: Resource { resource: null_mut(), description: [0; 8], state: 0 },
        flags: debug_flags() & 0xff,
        only_generated: debug_flags() & 0x100 != 0,
        rect: [0, 0, size[0] as i32, size[1] as i32],
        frame: state.frame,
    };
    // SAFETY: the context and a description that lives for the call.
    let code = unsafe { (api.configure)(context, &raw mut configure.header) };
    if code != 0 {
        static SAID: std::sync::Once = std::sync::Once::new();
        SAID.call_once(|| tracing::warn!(code, "frame generation: the library refused a frame's settings (logged once)"));
        return;
    }
    let Some(i) = inputs.filter(|_| on) else {
        GENERATING.store(false, Ordering::Relaxed);
        return;
    };
    let l: &ID3D12GraphicsCommandList = &list.list;
    let read = D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
    let resource = |r: &ID3D12Resource, description: [u32; 8]| Resource {
        resource: r.as_raw(),
        description,
        // (Read by compute shaders.)
        state: 0x4,
    };
    let v = i.values;
    let mut prepare = Prepare {
        header: Header { kind: PREPARE, next: null_mut() },
        frame: state.frame,
        flags: 0,
        list: l.as_raw(),
        render: v.render,
        jitter: v.jitter,
        motion_scale: v.motion_scale,
        frame_time: v.frame_time,
        reset: v.reset,
        near: v.near,
        far: v.far,
        fov: v.fov,
        to_meters: v.to_meters,
        depth: resource(&i.depth, i.descriptions[0]),
        motion: resource(&i.motion, i.descriptions[1]),
        camera: [[0.0; 3], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
    };
    // SAFETY: an open list of ours; our copies, copy destinations outside
    // this list; the context and a description that lives for the call.
    let code = unsafe {
        l.ResourceBarrier(&[
            transition(&i.depth, D3D12_RESOURCE_STATE_COPY_DEST, read),
            transition(&i.motion, D3D12_RESOURCE_STATE_COPY_DEST, read),
        ]);
        let code = (api.dispatch)(context, &raw mut prepare.header);
        l.ResourceBarrier(&[
            transition(&i.depth, read, D3D12_RESOURCE_STATE_COPY_DEST),
            transition(&i.motion, read, D3D12_RESOURCE_STATE_COPY_DEST),
        ]);
        code
    };
    list.keep(&i.depth);
    list.keep(&i.motion);
    GENERATING.store(code == 0, Ordering::Relaxed);
    static SAID: [std::sync::Once; 2] = [const { std::sync::Once::new() }; 2];
    if code == 0 {
        SAID[0].call_once(|| tracing::info!(frame = state.frame, values = ?v, "frame generation: the first frame prepared; a frame is generated before each of the title's from here on"));
    } else {
        SAID[1].call_once(|| tracing::warn!(code, "frame generation: the library did not prepare a frame (logged once)"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without the switch nothing is loaded, and the caller's own swap chain
    /// and presents stay as they are.
    #[test]
    fn off_unless_asked_for() {
        if std::env::var_os("VICTORY_FRAME_GENERATION").is_none() {
            assert!(api().is_none());
        }
        // The values that go into the library's descriptions by their place.
        assert_eq!(API, [0x100_0002, 0x100_0000]);
        assert_eq!(std::mem::offset_of!(Prepare, frame_time), 64);
        assert_eq!(std::mem::offset_of!(Configure, rect), 120);
    }
}
