# VIctory on a GTX 1080 Ti

This branch ports [avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX)
(at `0dec5dc`) to a card without mesh shaders. The rest of this README, from
"VIctory on an RX 7900 XTX" down, is that tree's own.

## What runs here

Tested on one PC only: Windows 10, NVIDIA GTX 1080 Ti (11 GB, no mesh shaders, no
barycentrics, 32-lane waves), Ryzen 5 2600X, 32 GB of RAM, the Series S profile.

| | |
|---|---|
| Stability | sessions of 20 minutes from the title screen into the open world, no failure |
| The title's own frame rate | 11-14 fps in the prologue's cell, 8-11 in the open world (the processor is the limit) |
| Frames shown | 35-44 a second with four frames for one (`VICTORY_FRAME_GENERATION=4`), 18-28 with two |
| Memory | 20-26 GB of the PC's, 9.5 of the card's 10 GB budget |

What this branch adds:

- **Mesh shaders run as compute shaders** where the card has none, with their
  draws grouped; barycentrics emulated, in mesh pipelines and, through a
  geometry stage, behind the title's own vertex shaders (hair, particles).
- **Frame generation** with AMD FidelityFX (`VICTORY_FRAME_GENERATION=1`: two
  frames for each of the title's; `=4`: four, the library used twice over).
- **Textures:** how long idle ones stay on the card (`VICTORY_TEXTURE_KEPT_MS`),
  and the room kept for render targets sized for 720p
  (`VICTORY_CARD_ROOM_FOR_TARGETS_MB`).
- **Light** in rooms the title lights with rays: a least level for the stand-in
  (`VICTORY_AMBIENT_LEAST`).
- **Keyboard and mouse** as in GTA V on PC (`VICTORY_KBM_GTA=1`, from
  [Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)
  pull request 295: F2 holds the mouse for the camera).
- Fewer host command lists, reused copy buffers, helper threads above normal
  priority, and a list that fails to close runs empty instead of ending the run.

Settings used (environment variables), with `victory run <exe> --execute --console series-s`:

```
VICTORY_THIN=1 VICTORY_JOIN_PARTS=8 VICTORY_UPLOAD_AHEAD=1 VICTORY_RELAY=2
VICTORY_WIDE_BACK=1 VICTORY_RELAY_COPY=1 VICTORY_JOIN_COPIES=1
VICTORY_CARD_REHASH_RUNS=1 VICTORY_INDIRECT_WINDOW=1 VICTORY_INDIRECT_BATCH=2
VICTORY_CARD_STEPS=2 VICTORY_TEXTURE_NO_WAIT=1 VICTORY_MAP_THREADS=1
VICTORY_TITLE_PRIORITIES=0 VICTORY_SHOW_AFTER=1 VICTORY_CHAIN_ABOVE=1
VICTORY_SNAPSHOT_POOL=64 VICTORY_MEMORY_MAP=0 VICTORY_LEVELS_ON_DEMAND=1
VICTORY_CARD_BUFFERS_MIN_KB=64 VICTORY_CARD_BUFFERS_MB=1536
VICTORY_OUTPUT=1280x720 VICTORY_WINDOW_MAX=640x360
VICTORY_TEXTURE_ROOM_PERCENT=82 VICTORY_CARD_ROOM_FOR_TARGETS_MB=2300
VICTORY_TEXTURE_KEPT_MS=60000 VICTORY_AMBIENT=3 VICTORY_AMBIENT_LEAST=0.6
VICTORY_AMBIENT_AIR=1.5 VICTORY_FRAME_GENERATION=4 VICTORY_KBM_GTA=1
VICTORY_FIDELITYFX_DIR=<folder with AMD's DLLs>
VICTORY_SKIP_PIPELINES=<the first column of scripts/gtx1080-skip.txt, comma-separated>
```

`VICTORY_FIDELITYFX_DIR` holds `amd_fidelityfx_upscaler_dx12.dll` and
`amd_fidelityfx_framegeneration_dx12.dll` from
[AMD's FidelityFX SDK 2.4.0](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/tree/v2.4.0/Kits/FidelityFX/signedbin)
(not included). Without the upscaler the Series S world is black here.

Still wrong: the title's own frame rate is far from 30; frame generation smears
what moves fast and subtitles when their text changes, and what the game
answers to is no sooner; not all of the open world's textures fit the card;
night scenes are dark; one "host command list did not close" was seen once and
not explained (the run goes on without that list's part of the frame).

# VIctory on an RX 7900 XTX

A fork of [VIctory](https://github.com/Marvisak/VIctory), a compatibility layer
that runs an Xbox Series title natively on Windows from files the user supplies.

## The goal

**A stable 30 fps or more without ray tracing, then as high a frame rate and as
low frame times as the PC allows**, on an AMD RX 7900 XTX first and on PCs with
32 GB (later 16 GB) of RAM.

The way there is **porting, not emulating**: patching the title's renderer so it
behaves the way a PC renderer does, rather than having the layer watch and copy
the console's shared memory every frame. Shaders are translated ahead of time
into a versioned store that is kept between builds. Ray tracing stays optional
and off by default. The picture comes after the frame rate: texture pop-in and
lighting that is too bright or too dark when walking indoors are next in line.

**This repository contains no game files, code or assets.** You need your own
copy of the game. It is not affiliated with or endorsed by the game's developer
or publisher.

## Where it stands (2026-11-26)

Tested on one machine only: Windows 11, Intel i9-12900K, 64 GB RAM, Radeon
RX 7900 XTX, the Series X profile, ray tracing off.

| | |
|---|---|
| City street, renderer patches 1+2 on | about 53 ms a frame (about 19 fps) |
| Same scene before the patches | 57-75 ms (13-17 fps) |
| Shaders the title uses that translate | all of them (25,790 of 25,985; the rest are unused geometry shaders) |
| Memory | about 35 GB of private memory: **64 GB of RAM recommended** for now |

It started at 280 ms a frame in the same scenes three days earlier.

### Renderer patches (off by default while they are tested)

| Switch | What it does |
|---|---|
| `VICTORY_PORT_CONSTANTS=1` | The title's 174 MB constants heap is read by the card where the title writes it: no uploads, no fills |
| `VICTORY_PORT_DESCRIPTORS=1` | The title's descriptor-table stores are patched (byte-checked) to report the slots they write, instead of a scan of ~600,000 slots a frame |

What is known to be wrong:

- Trees, signs and far objects flicker in the distance in some places.
- Lighting is too bright or too dark after walking indoors, until the title has
  run for a while; rooms with little direct light are too dark without `-RayTracing`.
- Textures of what just came into view appear late while the title streams.
- The first run of a new build stutters while the driver makes pipelines
  (`victory warm` makes them ahead).
- Recording a frame in the world makes the title's own watchdog end it.

## How to run it

You need Windows 10/11 x64, a CPU with AVX2, a Direct3D 12 GPU with shader
model 6.6 and mesh shaders, Rust (the toolchain is pinned), the MSVC build
tools, and your own dump of the title.

```powershell
git clone https://github.com/avatarmanz/VIctory-7900XTX
cd VIctory-7900XTX
copy victory.example.toml victory.toml      # then set your paths, outside this folder
cargo build --locked --profile diag -p victory --features diagnostics
scripts\play.ps1 -Exe <path of your copy's executable>
```

`scripts\play.ps1 -Safe` leaves out the newest, faster ways; try it first when
something is wrong. `-RayTracing` turns the title's ray-traced indirect light on.

On a hybrid-graphics PC, the runtime and offline pipeline checks prefer a
high-performance adapter with Shader Model 6.6. The runtime logs its adapter
name, PCI vendor and device IDs, and dedicated VRAM. Set
`VICTORY_D3D12_ADAPTER` to a zero-based DXGI index or part of the adapter's
name to select it explicitly. With the launcher, pass
`-More @{VICTORY_D3D12_ADAPTER='NVIDIA'}`.

Before the first run, ahead of time (both can be stopped and go on later):

```powershell
target\diag\victory.exe shaders <the shader archives>   # every shader translated, minutes
target\diag\victory.exe warm --used                     # pipelines earlier runs drew with, for a new driver or build
```

Commands, debugging switches and upstream's own description:
[docs/README-fxceless.md](docs/README-fxceless.md). Every `VICTORY_...` switch is
described in a comment where it is read.

## What is different here

In short; the reasons and the measurements are in [docs/findings.md](docs/findings.md).

- **List calls are written down, not carried out, on the title's threads**
  (`VICTORY_THIN`): eight threads of ours translate them.
- **Submissions are prepared by a thread of ours and joined**
  (`VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`), with what can be spread over
  threads spread: refreshes, compares (AVX2), descriptor records, landings.
- **What shaders wrote comes back beside the write guards**, by the dwords that
  changed, on a thread of its own (`VICTORY_RELAY`, `VICTORY_LAND`).
- **Buffer fills that need bytes still on the card are done on the card**
  (`VICTORY_CARD_STEPS`) instead of waiting for it.
- **The title's writes are caught more cheaply**: one view opened at a fault,
  blocks that rest, places in the title's code that look before they write.
- **Textures go up on a copy queue of their own** and a slot shows one only after
  its upload ran, which also ended a device loss that came every 15 minutes or so.
- **Boats float**: textures that shaders write and the title's CPU reads go back
  to title memory (from fxceless and Geckomacabre, on by default here).
- **Frame record and replay** with per-pass times and shader variants compared
  turn by turn ([docs/replay.md](docs/replay.md)).

## Credits

This is a tree of other people's work with more on top. The history is kept, and
commits taken from elsewhere name the commit and its author.

- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its
  contributors: the loader, the runtime services, the Direct3D 12 back end and
  the shader recompiler.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)**: the fork
  this one started from, and the people whose pull requests it carries.
- **[DexNom/VIctory](https://github.com/DexNom/VIctory)**: ray tracing fixes,
  cube gradients, sparse textures.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)**:
  the ambient light idea, pipeline workers, and work by romainhedouin.
- **[FQY55kai/ZJOU-VICTORY](https://github.com/FQY55kai/ZJOU-VICTORY)**.
- **[myokex/victory-amd](https://github.com/myokex/victory-amd)**: the guard over
  every view of a page (with Spexivus, joroyt and tightfall), and Joshua Odom's
  reads into title memory.
- **Geckomacabre** and **cxy0072019-sketch**: the texture read-back.
- **[XCL](https://github.com/ivall/xcl)** (GPL-2.0-only) was read for how the same
  title behaves under another layer; no code of it is here.
- This fork's changes were made with [Claude Code](https://claude.com/claude-code).

## License

GPL-3.0-or-later, as VIctory. See [LICENSE](LICENSE); the one file that is not
covered is named in [docs/asset-policy.md](docs/asset-policy.md).
