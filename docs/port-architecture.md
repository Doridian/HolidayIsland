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
crates/game/       the decompiled game, one module per original unit; no platform deps
crates/platform/   winit/wgpu/cpal glue, the data source and storage backends
crates/holiday/    the binary (native) and the wasm entry point
```

`game` and `vcl` see the platform only through traits (display, audio, data
files, storage, clock), so they run headless in tests.

## Design points

- **Game state lives in one `Game` struct.** The original's units share global
  variables and form objects that call into each other. In the port those are
  fields of `Game`, and each original routine is a function or method taking
  `&mut Game`. That keeps a 1:1 mapping to the original routines (with an
  address comment on each), which is what makes checking against the original
  practical.
- **Modal flows are `async`.** Win16 code blocks freely: `ShowModal`,
  `MessageBox`, `Application.ProcessMessages` loops, waiting for a video to end.
  The browser can't block. Instead of rewriting those routines as state
  machines, the port writes them as `async fn`, with `.await` where the
  original blocked. A tiny single-threaded executor is polled once per
  frame by the platform loop. The code keeps the original's shape and runs the
  same way everywhere.
- **Delphi integer semantics are explicit.** Delphi 1 `Integer` is 16-bit, so the
  port uses `i16`/`u16`/`i32`/`u8` exactly where the original did, and
  `wrapping_*` operations where the original relies on wraparound. Debug builds
  panic on overflow, which flags those places. If the simulation uses Borland's
  6-byte `Real`, it gets a `Real48` type with the original's rounding.
- **Deterministic core.** Time comes from a virtual clock that the platform
  advances, and randomness from a port of Delphi's `Random`: `RandSeed`
  (`1110:4a06`) steps as `RandSeed * 0x08088405 + 1` (`1108:1de9`, the multiplier
  at `CS:1e1f`). `Randomize` (`1108:1e21`) seeds it from the DOS time. Given the
  same inputs, a session replays exactly, which is what differential tests and
  bug reports need.
- **Screen size follows the window.** The original doesn't have a fixed
  resolution. It maximizes its background form and lays out for whatever the
  Windows desktop is: at least 640×480, with 800×600 recommended. The port treats
  the window's client area, divided by the UI scale, as that desktop. Resizing
  the window resizes the indexed framebuffer and re-runs the game's layout,
  so a bigger window shows more of the island rather than bigger pixels. The UI
  scale is an integer (1–4, automatic from the display's DPI by default, as in
  Afterlife), and the GPU scales the framebuffer up by it with nearest-neighbour
  filtering. The first milestone may pin the size to 640×480 or 800×600 until the
  layout code is ported and understood. Arbitrary sizes are the goal once the
  initial port is done. Sizes below 640×480 get letterboxed.
- **Window handling like Afterlife.** A resizable window that remembers its size
  and position. **Alt+Enter** or **F11** toggles fullscreen (borderless at the
  desktop resolution; Ctrl+Cmd+F on macOS; the Fullscreen API on the web, with
  the canvas following the page otherwise). Command-line options:
  `--window-size=WxH`, `--fullscreen` / `--windowed`, `--ui-scale=N`. The port
  keeps its own settings file (in the platform's config directory, browser
  storage on the web), separate from anything the original reads.
- **Game data.** Players point the port at the CD: an ISO image, or a folder
  with the CD's files. It reads the ISO and the installer's `_FILES.1` itself, so
  players don't need the Python tools; those stay for research. File lookup is
  case-insensitive. In the browser the user picks the ISO once and it's kept in
  the Origin Private File System. Saves, high scores and settings go to the
  platform's data directory natively, and to browser storage on the web.
- **Assets from the EXE** (DFMs, cursors, icon) are read from the user's
  `HOLIDAY.EXE` at runtime and never committed.

## Open questions

- Whether `SETUP.DAT` matters beyond the startup check, and what the port
  writes or emulates in its place.
- Which General MIDI SoundFont to use, and how it reaches the player.
- How much the game relies on Windows' palette manager (palette animation,
  `RealizePalette` timing), which decides how faithful the palette path must be.
- Differential testing against the original: the `unicorn-engine` crate can run
  16-bit protected-mode code once a descriptor table and the NE relocations are
  set up. Whether that's worth building depends on how tricky the simulation turns out to be.
