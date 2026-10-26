# The browser build

The port runs in a browser as WebAssembly (`wasm32-unknown-unknown`),
without Emscripten: winit draws into a `<canvas>`, wgpu renders with
WebGPU or WebGL2, cpal plays through WebAudio, and wasm-bindgen glues it
to the page. The game is the same code as natively; only the start, the
files and the clock differ ([port-architecture.md](port-architecture.md#stack)).

## Building

Needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` CLI in
the version the crates pin (`wasm-bindgen = "=0.2.129"` in
`crates/holiday/Cargo.toml` and `crates/platform/Cargo.toml`; the CLI
refuses a module from another version):

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
tools/build-web.sh            # release build into web/
tools/build-web.sh --debug    # quicker to build, slow to play
```

The script runs `cargo build -p holiday --target wasm32-unknown-unknown`
and `wasm-bindgen --target web`, and copies the page
(`crates/holiday/web/index.html`) next to the output: `web/` holds
`index.html`, `holiday.js`, `holiday_bg.wasm` and `snippets/` (the
storage code, `crates/holiday/src/web.js`). `web/` is ignored by git.
There's no Trunk setup; the script is all it takes.

Upgrading wasm-bindgen means changing the pin in both crates and
installing the matching CLI.

## GitHub Pages

`.github/workflows/build.yml` publishes the build on every push to
`main` at <https://doridian.github.io/HolidayIsland/>, without game data
(the workflow fails if anything but the page, the glue and the module is
in the build). `crates/holiday/web/front.html` is the site's front page:
where to get the CD image and a SoundFont, and links to the game,
`holiday.html` (the build's `index.html`), with `?lang=en` or `?lang=de`.
The workflow installs the wasm-bindgen CLI in the version the crates
pin.

## Serving and playing

Any static HTTP server does; the module can't load from `file://`:

```sh
python3 -m http.server -d web 8000    # then open http://localhost:8000/
```

On the first start the page asks for the game's CD image (the German
CD's `.iso`, or a raw `.bin`) and, optionally, a SoundFont (`.sf2`) for
the music; without one the music is silent, as natively without one.
**Start** reads the image into memory, unpacks the installer's
`_FILES.1` (which holds `HOLIDAY.EXE`) and starts the game. On later
starts the page shows what it kept and **Start** goes straight on;
picking another file replaces the kept one. The page never fetches or
sends game data: the files come from the player.

The Start button is also the click the browser wants before a page may
play sound (its autoplay rules): the `AudioContext` is resumed on it and
on every later click or key press (`platform::audio::resume`), so a
browser that still held it back starts the sound with the next one.

The English release isn't a CD image (it comes as a DOSBox installer),
so the browser build plays the German CD's files; the texts are the
port's own in both languages, so `?lang=en` plays it in English (with
the German speech).

## Options

The page's query string takes the command line's options without their
`--` (`docs/networking.md`, `holiday --help`): `?lang=en`,
`?original`, `?sound=lo`, `?films=lo`, `?ui-scale=2`,
`?enhance=big_map=off`, and the network game's below. A key without a
value is a switch (`?original`). Options that name files or the window's
size (`--save-dir=`, `--soundfont=`, `--window-size=`) mean nothing
there. A bad option stops the page with the reason shown at the bottom.
What the game would print on the terminal goes to the browser's console
(F12), with `network:` in front for the network game.

## Network games

As natively ([networking.md](networking.md#the-browser-build)):

```
http://localhost:8000/?host=island&signal=wss://example.org/holiday
http://localhost:8000/?join=island&signal=wss://example.org/holiday&stun=stun:example.org:3478
```

`stun=` and `turn=` may repeat; `turn-user=` and `turn-password=` go
with `turn=`. A page served over HTTPS can only use a `wss://`
signaling server. A browser and a native game play together, either
hosting, with the German CD's files or the English release's on the
native side, and in either language (`?lang=en`).

A joining page says in the game that it waits for the host's game (a
message box where the start menu would be; Cancel gives up). When the
host quits, or its connection is lost for good (10 s after it drops,
the status bar saying so meanwhile; closing the host's tab is that),
the game stops, says why in a message box and offers to save it: Yes
opens the file menu, whose Save keeps the game in the browser's
storage, and then the start menu comes. Hosting that save again works
as natively ([networking.md](networking.md#the-protocol)).

## Storage

Everything is kept in the browser's **IndexedDB**, database
`holiday-island`, for the page's origin:

| Store | Key | What |
|---|---|---|
| `cd` | `image` | the CD image as the `File` the player picked (`{blob, name, size}`) |
| `files` | a path | the port's own files: `MAPS/*.map` (the saves), `HIGHSCOR.WAV`, `HIGHSCO2.WAV`, `settings.ini`, and the SoundFont as `soundfont/NAME.sf2` |

The game sees `files` as a directory, `/storage`
(`platform::storage::WEB_STORAGE`): both the data and the config
directory. It's read into memory at the start (`platform::data::MemoryFiles`),
and every write, rename and removal goes to IndexedDB at once, in order.
The CD image is mounted at `/cd`, the game's folder
(`platform::data::CdImage`, through `platform::fs::mount`).

Why IndexedDB and not the Origin Private File System: the game reads
its files synchronously, so the whole image is read into memory once
(about 210 MB) either way, and the advantage of OPFS, synchronous
reads, exists only in a worker (`createSyncAccessHandle`). IndexedDB
keeps the picked `File` as a Blob without the page reading it first,
works on the page's own thread in every current browser (OPFS's
`createWritable` came to Safari late), and one database holds both the
image and the small files. The page asks for persistent storage
(`navigator.storage.persist()`) so the browser doesn't clear it when
space runs low; clearing the site's data forgets the CD and the saves.

## Saved games as files

The saves in the browser's storage stay in that browser; these take them
in and out as files:

- **Drag and drop**: a saved game (`.map`, from the browser build's
  Download, a native game's save folder or the game's `MAPS\`) dropped
  onto the page loads. On the start menu (or its load form) the game
  starts with it as with "Altes Spiel fortsetzen"; in a game a question
  comes first (the game in progress is lost unless it was saved), then
  it loads as the file menu's "Laden" does. A drop during the intro, a
  film or another message box waits until they're over; a file that
  isn't a saved game gets an error. The page reads the file
  (`web.js`'s `watchDrops`) and the game takes it the next frame
  (`Holiday::take_dropped`). The native build does the same with files
  dropped onto its window (winit's `DroppedFile`).
- **Download** in the file menu, a button of the port's own in the gap
  below "Beenden" (`Dateimenue::with_download`), saves the game under the
  name in the name field (`HOLIDAY` if it's empty; the same rule as
  "Speichern": a letter, then up to seven letters, digits or `_`) as
  `NAME.map`, which the browser downloads. Nothing is written to the
  browser's storage. The later build's form (`UPGRADE\`) has no room for
  it, and the German CD the browser build plays has none.

## Browser support

The renderer asks for WebGPU first and falls back to WebGL2 where the
browser has no WebGPU adapter (`wgpu::util::new_instance_with_webgpu_detection`);
the console says which (`GPU: BrowserWebGpu` or `GPU: Gl`). The shader
and the limits are WebGL2's (`wgpu::Limits::downlevel_webgl2_defaults`),
so both draw the same.

- Chrome and Edge: WebGPU (where the GPU is supported), else WebGL2.
- Firefox: WebGPU on Windows since 141, WebGL2 elsewhere.
- Safari: WebGPU since 26, WebGL2 before.

The window is the page: the canvas fills it and the desktop the game
lays out for follows its size, as a resized window natively. F11 is the
browser's own fullscreen; Alt+Enter asks for the Fullscreen API's.
Right clicks go to the game, not to a context menu.

## What differs from the native build

- No threads: the game, the audio's callbacks and the network's message
  loop (`wasm_bindgen_futures::spawn_local`) run on the page's thread.
- The GPU is set up asynchronously; the game starts updating once it's
  there.
- The clock is `web-time`'s (`performance.now()`, `Date.now()`), which
  is `std::time` natively.
- Nothing is remembered about the window (the page has none to place).
- The page has nothing to quit to: Escape on the start menu does
  nothing, and quitting a game (its end film included) goes back to
  the start menu. Anything else that ends the game says so on the
  status line, and a reload starts again.
- The world doesn't notice any of this: it never reads the clock or the
  files, and its hash and the wire format are the same on 32-bit wasm
  as natively ([networking.md](networking.md#the-protocol)).
