# VIctory on a GTX 1080 Ti

A fork of [avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX)
(itself a fork of [VIctory](https://github.com/Marvisak/VIctory), a compatibility
layer that runs an Xbox Series title natively on Windows from files the user
supplies), ported to a card that has **no mesh shaders, no barycentrics and
32-lane waves**: an NVIDIA GTX 1080 Ti.

**This repository contains no game files, code or assets.** You need your own
copy of the game. It is not affiliated with or endorsed by the game's developer
or publisher.

## Where it stands (2026-11-27)

Tested on one PC only: Windows 10, GTX 1080 Ti (11 GB), Ryzen 5 2600X, 32 GB of
RAM, the Series S profile, ray tracing off.

| | |
|---|---|
| Stability | sessions of 20 minutes from the title screen into the open world without a failure |
| The game's own frame rate | 11-14 fps in the prologue, 8-11 in the open world: the processor is the limit |
| Frames shown | 35-44 a second with four frames for one, 18-28 with two (AMD FidelityFX frame generation) |
| Memory | 20-26 GB of the PC's, about 9.5 GB of the card's 10 GB budget |

Frame generation makes the picture move more smoothly; what the game answers to
(your keys and mouse) is not any sooner.

## What this fork adds

- **Mesh shaders run as compute shaders** where the card has none, their draws
  grouped and drawn after their mesh programs.
- **Barycentrics emulated**: in those mesh pipelines, and through a geometry
  stage behind the title's own vertex shaders (hair, particles, bloom).
- **Frame generation** with AMD FidelityFX: `VICTORY_FRAME_GENERATION=1` shows
  two frames for each of the game's, `=4` four (the library used twice over).
- **Textures**: idle ones leave the card after a while
  (`VICTORY_TEXTURE_KEPT_MS`), and the room kept for render targets is sized for
  720p (`VICTORY_CARD_ROOM_FOR_TARGETS_MB`), so more of the scene's textures fit.
- **Light** in rooms the game lights with rays: the stand-in has a least level
  (`VICTORY_AMBIENT_LEAST`), so dark rooms are not black.
- **Keyboard and mouse** as in GTA V on PC (`VICTORY_KBM_GTA=1`).
- Fewer host command lists, copy buffers used again, helper threads above
  normal priority, and a command list that fails to close runs empty instead of
  ending the game.

## How to run it

You need Windows 10/11 x64, a CPU with AVX2, a Direct3D 12 GPU with shader
model 6.6 (mesh shaders are not needed), 32 GB of RAM, Rust (the toolchain is
pinned), the MSVC build tools, your own dump of the title, and two DLLs from
[AMD's FidelityFX SDK 2.4.0](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/tree/v2.4.0/Kits/FidelityFX/signedbin):
`amd_fidelityfx_upscaler_dx12.dll` and `amd_fidelityfx_framegeneration_dx12.dll`
(not included; without the upscaler the Series S world is black here).

Build it, and translate the shaders once ahead of time:

```powershell
git clone https://github.com/Billiz1122/VIctory-1080Ti
cd VIctory-1080Ti
copy victory.example.toml victory.toml      # then set your paths, outside this folder
cargo build --locked --release -p victory
target\release\victory.exe shaders <the shader archives>   # minutes; can be stopped and go on later
```

Then, from that folder, set the settings and start the game:

```powershell
$env:VICTORY_FIDELITYFX_DIR = '<the folder with the two DLLs>'
$env:VICTORY_SKIP_PIPELINES = ((Get-Content scripts\gtx1080-skip.txt) | ForEach-Object { ($_ -split ' ')[0] }) -join ','
$env:VICTORY_FRAME_GENERATION = '4'; $env:VICTORY_KBM_GTA = '1'
$env:VICTORY_OUTPUT = '1280x720'; $env:VICTORY_WINDOW_MAX = '640x360'
$env:VICTORY_THIN = '1'; $env:VICTORY_JOIN_PARTS = '8'; $env:VICTORY_UPLOAD_AHEAD = '1'
$env:VICTORY_RELAY = '2'; $env:VICTORY_WIDE_BACK = '1'; $env:VICTORY_RELAY_COPY = '1'
$env:VICTORY_JOIN_COPIES = '1'; $env:VICTORY_CARD_REHASH_RUNS = '1'; $env:VICTORY_CARD_STEPS = '2'
$env:VICTORY_INDIRECT_WINDOW = '1'; $env:VICTORY_INDIRECT_BATCH = '2'; $env:VICTORY_TEXTURE_NO_WAIT = '1'
$env:VICTORY_MAP_THREADS = '1'; $env:VICTORY_TITLE_PRIORITIES = '0'; $env:VICTORY_SHOW_AFTER = '1'
$env:VICTORY_CHAIN_ABOVE = '1'; $env:VICTORY_SNAPSHOT_POOL = '64'; $env:VICTORY_MEMORY_MAP = '0'
$env:VICTORY_LEVELS_ON_DEMAND = '1'; $env:VICTORY_CARD_BUFFERS_MIN_KB = '64'; $env:VICTORY_CARD_BUFFERS_MB = '1536'
$env:VICTORY_TEXTURE_ROOM_PERCENT = '82'; $env:VICTORY_CARD_ROOM_FOR_TARGETS_MB = '2300'; $env:VICTORY_TEXTURE_KEPT_MS = '60000'
$env:VICTORY_AMBIENT = '3'; $env:VICTORY_AMBIENT_LEAST = '0.6'; $env:VICTORY_AMBIENT_AIR = '1.5'
target\release\victory.exe run '<path of your copy of the game executable>' --execute --console series-s
```

`VICTORY_FRAME_GENERATION='1'` gives two frames for one instead of four: fewer
frames shown, a steadier picture, and the game's own frames a tenth to a fifth
faster. The first run after a new build reaches the world a few minutes later
while the mesh programs are translated again.

`scripts\play.ps1` is the RX 7900 XTX tree's launcher and does not set the
settings above.

### Keys (`VICTORY_KBM_GTA=1`)

| | |
|---|---|
| W A S D | move |
| F2 or `` ` `` | hold the mouse to turn the camera (Esc lets it go) |
| left / right mouse button | shoot / aim |
| Shift, Space, Ctrl | sprint, jump, crouch |
| F, R, E | vehicle, reload, talk |
| Tab, Q, V | weapons, cover, camera |
| Enter, Backspace | yes, back |
| Esc or P | pause |

## What is still wrong

- The game's own frame rate is far from 30. The processor is the limit: the
  thread that hands the game's work to the card, not the card.
- Frame generation smears what moves fast, and subtitles where their text
  changes.
- Not all of the open world's textures fit the card's 11 GB.
- Night scenes are dark.
- One "host command list did not close" was seen once and not explained; the
  game goes on without that list's part of the frame.
- The renderer patches of the RX 7900 XTX tree (`VICTORY_PORT_*`) made it slower
  on this card: leave them off.

Commands, debugging switches and upstream's own description:
[docs/README-fxceless.md](docs/README-fxceless.md); the reasons and the
measurements behind the layer's design: [docs/findings.md](docs/findings.md).
Every `VICTORY_...` switch is described in a comment where it is read.

## Credits

This is a tree of other people's work with more on top. The history is kept, and
commits taken from elsewhere name the commit and its author.

- **[avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX)**:
  the tree this fork is built on (written-down list calls, submissions prepared
  beside the game, the relay, card steps, frame record and replay).
- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its
  contributors: the loader, the runtime services, the Direct3D 12 back end and
  the shader recompiler.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)**: the fork
  the 7900 XTX tree started from, and the people whose pull requests it carries.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)**:
  the ambient light idea, pipeline workers, work by romainhedouin, and the GTA V
  keyboard and mouse mapping by zhu1025960209-web (its pull request 295).
- **[DexNom/VIctory](https://github.com/DexNom/VIctory)**: ray tracing fixes,
  cube gradients, sparse textures.
- **[FQY55kai/ZJOU-VICTORY](https://github.com/FQY55kai/ZJOU-VICTORY)**.
- **[myokex/victory-amd](https://github.com/myokex/victory-amd)**: the guard over
  every view of a page (with Spexivus, joroyt and tightfall), and Joshua Odom's
  reads into title memory.
- **Geckomacabre** and **cxy0072019-sketch**: the texture read-back.
- AMD's [FidelityFX SDK](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK)
  (MIT): the descriptions the frame generation calls are made with.
- **[XCL](https://github.com/ivall/xcl)** (GPL-2.0-only) was read for how the same
  title behaves under another layer; no code of it is here.
- This fork's changes were made with [Claude Code](https://claude.com/claude-code).

## License

GPL-3.0-or-later, as VIctory. See [LICENSE](LICENSE); the one file that is not
covered is named in [docs/asset-policy.md](docs/asset-policy.md).
