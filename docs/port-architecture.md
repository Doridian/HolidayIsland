# Port architecture (planned)

The port is written in **Rust** (stable, edition 2024), as a Cargo workspace.
Targets are Linux (Wayland and X11), macOS, and the browser (WebAssembly). Nothing
here is built yet; this is the plan the first code should follow.

## What the original needs from the platform

From [binaries.md](binaries.md):

- **One full-screen, 256-colour, palettized display.** Every form is borderless,
  and the background form (`THintergrund`) is maximized. The game adapts to the
  desktop size: the README asks for at least 640×480 and recommends 800×600.
  It draws 8-bit BMPs with GDI blits and palettes.
- **Mixed sound effects and speech** (WaveMix: several 8-bit PCM WAVs at once),
  **MIDI music** (MCI sequencer), and **cutscenes** (AVI: Cinepak video, MS
  ADPCM audio).
- **Timers** (`TTimer`/`SetTimer`), mouse and keyboard, the VCL's form and event model.
- **Files**: the CD's data, plus saves, high scores and settings.

## Stack

| Concern | Choice | Why |
|---|---|---|
| Window, input, event loop | `winit` | The standard Rust windowing layer: native on all three desktop targets and a `<canvas>` on the web, with HiDPI, fullscreen and IME handling. |
| Rendering | `wgpu`, used directly | The game renders into an 8-bit indexed framebuffer in software, like the original's DIBs. Each frame uploads the framebuffer and the 256-entry palette as two textures. One fragment shader does the palette lookup and the integer UI-scale upscaling, so palette effects cost nothing. Runs on Vulkan/Metal/GL natively and WebGPU or WebGL2 in browsers. |
| Audio output | `cpal` | Native audio on all targets, WebAudio in the browser. |
| Mixing | own mixer | Reproduces WaveMix's behaviour (channels, looping, 8-bit PCM resampled to the output rate). It's small, and an exact reimplementation beats adapting a general-purpose audio engine. |
| MIDI | `rustysynth` | Pure-Rust SoundFont synthesizer, renders into the same mixer. Which General MIDI SoundFont to use (user-supplied, or a freely licensed one we can distribute) is still open. |
| Video | own AVI + Cinepak + MS ADPCM decoders | All three formats are small and well documented. Pure Rust keeps the web build simple. |
| Web build | `wasm32-unknown-unknown`, `wasm-bindgen`, `trunk` | No Emscripten; `trunk serve` for development. |

Rejected:
- SDL3: C dependency, heavier web build.
- `pixels`: it tends to lag behind `wgpu` releases, and the renderer is about a hundred lines anyway.
- `softbuffer`: no GPU scaling.
- Bevy/macroquad: they want to own the game loop and the asset model.

## Workspace (planned)

```
crates/formats/    pure parsers, no I/O: ISO9660, the installer archive (LZHUF), NE resources,
                   DFM (TPF0), BMP, the DATA/TEXTE tables, maps/scenarios, high scores,
                   AVI/Cinepak/MS ADPCM, WAV
crates/vcl/        the subset of Delphi's VCL/RTL the game uses: components built from the
                   DFMs, TCanvas over the indexed framebuffer, timers, modal flows,
                   ShortString, Delphi's Random
crates/game/       the decompiled game, one module per original unit; no platform deps.
                   World (simulation, commands) and View (one player's UI) kept apart
crates/platform/   winit/wgpu/cpal glue, the data source and storage backends
crates/holiday/    the binary (native) and the wasm entry point
crates/net/        later: lockstep session, transport trait, WebSocket relay client
```

`game` and `vcl` see the platform only through traits (display, audio, data
files, storage, clock), so they run headless in tests.

## Design points

- **State is split into the world and the player's view.** The original's
  units share global variables and form objects that call into each other.
  In the port those become fields of two structs:
  - `World`: the simulation, including the island, buildings, guests, every
    player's money, the date and the RNG.
  - `View`: one human player's UI, including open forms, scroll position, the
    selected tool and hover state.
  
  Each original routine is a function taking `&mut World` or `&mut View` (plus
  `&World` to read), with the original address in a comment. That keeps a 1:1
  mapping to the original routines, which makes checking against the original
  practical. Where an original routine mixes the two, it gets split at that
  seam. That split is what [network multiplayer](#stretch-goal-network-multiplayer)
  needs, and it costs little to do from the start.
- **`World` and `View` shouldn't stay two god objects.** Putting every global
  into two structs is the simplest 1:1 mapping, but it turns into two giant
  structs that every function takes whole. When porting starts, the first step
  is to check whether the original's state splits cleanly into smaller parts
  (island map, buildings, guests, finances per player, auctions, the RNG and
  clock; forms and tool state on the view side), with routines taking only the
  parts they touch. That's only worth it if the units' globals already group
  that way without fighting the 1:1 mapping. If they don't, the port starts with
  the two structs, and once the initial port is done and checked against the
  original, a cleanup pass makes it more idiomatic Rust: split the structs,
  replace raw indices and flag integers with types and enums, and turn the
  remaining Delphi-isms into ordinary Rust. That pass keeps the original's
  integer widths and wraparound and the determinism rules below: `World` must
  still replay bit for bit, or saves, replays and lockstep multiplayer break.
- **The UI changes the world only through commands.** Building, demolishing,
  setting prices, bidding, sabotage, the game speed: anything a player does to
  the world is a `Command { player, action }`. Commands are applied at a tick
  boundary by `World::apply`. Computer players produce the same commands from
  their AI. The UI never writes to `World` directly. In single player the
  command queue is local and the extra step costs nothing.
- **Modal flows are `async`, in the UI only.** Win16 code blocks freely:
  `ShowModal`, `MessageBox`, `Application.ProcessMessages` loops, waiting for a
  video to end. The browser can't block. Instead of rewriting those routines as
  state machines, the port writes them as `async fn`, with `.await` where the
  original blocked. A tiny single-threaded executor is polled once per frame by
  the platform loop. Only `View` code awaits. The simulation never waits on a
  dialog: where the original stops the simulation for one player's decision,
  that becomes a world state (paused, or waiting for that player's command), not
  a blocked call stack.
- **Delphi integer semantics are explicit.** Delphi 1 `Integer` is 16-bit, so the
  port uses `i16`/`u16`/`i32`/`u8` exactly where the original did, and
  `wrapping_*` operations where the original relies on wraparound. Debug builds
  panic on overflow, which flags those places. If the simulation uses Borland's
  6-byte `Real`, it gets a `Real48` type with the original's rounding.
- **Deterministic core.** Time comes from a virtual clock that the platform
  advances, and randomness from a port of Delphi's `Random`: `RandSeed`
  (`1110:4a06`) steps as `RandSeed * 0x08088405 + 1` (`1108:1de9`, the multiplier
  at `CS:1e1f`). `Randomize` (`1108:1e21`) seeds it from the DOS time. Given the
  same seed and command stream, `World` replays exactly, bit for bit, on every
  target. Differential tests, bug reports, replays and lockstep multiplayer all
  rely on that. Rules for code that touches `World`:
  - It never reads the wall clock and never draws randomness from anywhere but
    the world's RNG. If the original also calls `Random` for purely cosmetic
    things (animations, sound variation), those calls get a separate view-side
    RNG. That's a deviation, noted at each call site, made because a player's
    local UI must not change the world.
  - Iteration order is defined: `Vec`, or `BTreeMap` rather than `HashMap`.
  - Floating point only where the original used it. Only IEEE basic
    operations are allowed. Transcendental functions go through one
    pure-Rust implementation (the `libm` crate, or Borland's own algorithms),
    because platform math libraries differ in the last bit. Borland's 6-byte
    `Real` is integer arithmetic anyway.
  - `World` is serializable (save games, joining a game in progress) and
    hashable (desync detection).
- **Screen size follows the window.** The original doesn't have a fixed
  resolution. It maximizes its background form and lays out for whatever the
  Windows desktop is: at least 640×480, with 800×600 recommended. The port treats
  the window's client area, divided by the UI scale, as that desktop. Resizing
  the window resizes the indexed framebuffer and re-runs the game's layout
  ([layout.md](layout.md)), so a bigger window shows more of the desktop
  rather than bigger pixels. In the original that means more room around
  the main screen, not more island: the main screen is 640×479 at every
  size, centred, and the floating forms are placed against the screen's
  edges and centre; only the background picture changes (and there's
  none for sizes other than 640×480, 800×600, 1024×768, 1152×870 and
  1280×1024). The port's `big_map` enhancement (on by default) shows more
  island instead: the main screen fills a screen bigger than 640×480 and
  its map view grows, while the world still sees the original's view in
  its middle ([layout.md](layout.md#big_map-enhancement)). The UI scale
  is an integer (1–4, automatic from the display's DPI by default, as in
  Afterlife), and the GPU scales the framebuffer up
  by it with nearest-neighbour filtering. Sizes below 640×480 get
  letterboxed. The original lays out once at startup; the port lays out
  again on every resize, a port extension
  ([layout.md](layout.md#live-resizing-port-extension)). The `World`
  never sees the window's size.
- **Enhancements, on by default.** Fixes of the original's bugs and port
  extensions are switches with stable keys (`game::enhance`): the
  world's fixes are part of the `World` (hashed, saved in a chunk the
  original ignores, changed by command), the view's are the player's
  settings. `--original` or `original=true` in `settings.ini` switches
  everything off, which is the original exactly
  ([original-bugs.md](original-bugs.md#options)).
- **Window handling like Afterlife.** A resizable window that remembers its size,
  position and fullscreen state. **Alt+Enter** or **F11** toggles fullscreen
  (borderless at the desktop resolution; Ctrl+Cmd+F on macOS; the Fullscreen
  API on the web, with the canvas following the page otherwise, once there's a
  web build). F11 is the original's shortcut for the button help, which the
  port gives up for this. Command-line options:
  `--window-size=WxH`, `--fullscreen` / `--windowed`, `--ui-scale=N`. The port
  keeps its own settings file (`settings.ini` in the platform's config
  directory, `platform::settings`; browser storage on the web), separate from
  anything the original reads.
- **Game data.** Players point the port at the CD: an ISO image, or a folder
  with the CD's files. It reads the ISO and the installer's `_FILES.1` itself, so
  players don't need the Python tools; those stay for research. File lookup is
  case-insensitive. In the browser the user picks the ISO once and it's kept in
  the Origin Private File System. Saves, high scores and settings go to the
  platform's data directory natively, and to browser storage on the web.
- **Assets from the EXE** (DFMs, cursors, icon) are read from the user's
  `HOLIDAY.EXE` at runtime and never committed.

## Stretch goal: network multiplayer

The original is already a multiplayer game with one human in it. The start
screen asks for a player colour and a number of computer opponents
(`Computergegner:`, `TEXTE\START.TXT`). Everyone builds on the same island,
competes for guests, bids in auctions, and sabotages the others (piranhas in a
competitor's pool, stink bombs, `SABOTAGE.TXT`). One scenario, "Monopolus", is
won by putting every other player out of business. The goal, after the initial
port: let humans take some or all of the computer opponents' seats over the
network.

The intended model is **deterministic lockstep**, the usual approach for
simulation games (Age of Empires, OpenTTD):

- Every peer runs the same `World`. Only commands travel over the network:
  `(tick, player, action)`. The simulation advances to tick *n* once it has every
  seat's commands for *n* (an empty set is a valid "no command" message).
  Computer seats are simulated by everyone, from the same `World`, so they cost
  no bandwidth.
- A host (one of the players) assigns seats, picks the scenario and
  settings, and decides the game speed and pause. Speed and pause are commands
  too. Joining or resyncing a game in progress sends a serialized `World` plus the
  command log from that point.
- Each peer sends a hash of `World` every few ticks. A mismatch is a desync bug,
  and the command log reproduces it.
- Transport: browsers can only use WebSocket and WebRTC, so the baseline is a
  small relay server, WebSocket for both native and web clients. Peer-to-peer
  WebRTC data channels are a later option. The game talks to a transport trait,
  not to sockets.
- All world state is visible to every peer. That's fine for playing with
  friends; it isn't cheat-proof, and doesn't try to be.

What this asks of the port long before any networking exists, already reflected
in the design points above:

- The `World`/`View` split and command-only world changes. Several players each
  have their own `View`; there is no global "current player" inside `World`.
  Places where the original assumes the human is player 1 get marked
  when they're ported.
- Bit-exact determinism across native and wasm.
- No world-side blocking on one player's UI. Interactions that are modal for
  the one human today need a multiplayer behaviour, decided per case when
  they're decompiled: auctions (`TAuktionsform`, which already runs on
  timers; ported with its timers as world ticks and the bids as
  commands, the open question in [auction.md](auction.md#open-questions)),
  the "what now?" dialog (`TWasNunForm`), events, the end-of-game forms.
  Typical answers: the dialog becomes per-player and non-blocking, a decision
  gets a timeout with a default, or the game pauses for everyone.
- Save games store seats and keep the original format readable. Single-player
  saves of the original load as a game with one human seat.
- Language is each player's view setting; the world's own text (the
  players' names, the news archive's dates) comes from the game folder's
  data and must come with the world from the host
  ([language.md](language.md#the-worlds-text)).

Not in scope: matchmaking, accounts, or play with the original Windows version.

## Open questions

- What the port uses in place of `SETUP.DAT`. It only picks directories
  and the sound/video quality ([startup.md](startup.md)), so a default
  `Setup` value should do.
- Which General MIDI SoundFont to ship, if any. For now the player
  supplies one (`--soundfont`, `HOLIDAY_SOUNDFONT` or a system one;
  [sound.md](sound.md#the-music)), and without it the music is silent.
- How much the game relies on Windows' palette manager (palette animation,
  `RealizePalette` timing), which decides how faithful the palette path must be.
- Differential testing against the original: the `unicorn-engine` crate can run
  16-bit protected-mode code once a descriptor table and the NE relocations are
  set up. Whether that's worth building depends on how tricky the simulation turns out to be.
