# Port architecture

The port is written in **Rust** (stable, edition 2024), as a Cargo workspace.
Targets are Linux (Wayland and X11), macOS, and the browser (WebAssembly).
This started as the plan the first code followed; the port is built along
it, and what isn't built yet is marked: there's no web build, no `net`
crate, and the port reads a folder with the game's files, not the CD
image itself.

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

## Workspace

```
crates/formats/    pure parsers, no I/O: NE resources, DFM (TPF0), BMP, the DATA/TEXTE
                   tables, maps/scenarios, high scores, SETUP.DAT, AVI/Cinepak/MS ADPCM
                   (planned: ISO9660 and the installer archive, which tools/ reads now)
crates/vcl/        the subset of Delphi's VCL/RTL the game uses: components built from the
                   DFMs, TCanvas over the indexed framebuffer, timers, modal flows,
                   ShortString, Delphi's Random
crates/game/       the decompiled game, one module per original unit; no platform deps.
                   World (simulation, commands) and View (one player's UI) kept apart
crates/platform/   winit/wgpu/cpal glue, the data source and storage backends
crates/holiday/    the binary (native; the wasm entry point is planned)
crates/net/        later: lockstep session, transport trait, WebSocket relay client
lang/<code>/       the texts of each language (TEXTE/, UPGRADE/, strings.toml), embedded
                   into crates/game (lang/README.md)
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
- **Modal flows are view state, in the UI only.** Win16 code blocks freely:
  `ShowModal`, `MessageDlg`, `Application.ProcessMessages` loops, waiting for a
  video to end. The browser can't block. (The plan was to write these as
  `async fn` polled once a frame; the port didn't need it.) A modal form is
  the view's state that takes the input until it answers, and what the
  original did after the blocking call runs on the answer: the message
  boxes (`game::msgdlg`), the file menu, the quit question. Waiting for a
  film is a state the frame looks at, and the work of states 1200–1500 is
  done a step a frame under the progress window. The simulation never waits on a
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
- **The people a screen needs are the view's.** The original spawns some
  of its people only where the human looks, places all of them for the
  human's rotation and draws random numbers only for what the human
  sees. With the world fix `view_people` (on by default) the world keeps
  only the people that are game state or that the world reads (ships,
  boats, planes, the rentals' and the buildings' people, the oil, the
  events' and the sabotage's) and runs them by no view; the spectators,
  builders, taxiing planes and jetty boats are a simulation in each
  player's view (`game::people::view::ViewPeople`), with the view's own
  random numbers, driven by its ticks and its viewpoint, and the
  people's sounds go to every view with their square. The view enhancement
  `people_everywhere` runs that simulation for the whole island. Off is
  the original, for one human only ([people.md](people.md#who-is-whose-fixview_people)).
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
- **Game data.** Players point the port at a folder with the game's files
  (now), or the CD's ISO image, whose installer's `_FILES.1` it would read
  itself, so players don't need the Python tools (planned; those tools do
  it now). File lookup is case-insensitive. In the browser the user picks the ISO once and it's kept in
  the Origin Private File System. Saves, high scores and settings go to the
  platform's data directory natively, and to browser storage on the web.
- **Assets from the EXE** (the DFMs, the VCL's button glyphs) are read from
  the user's `HOLIDAY.EXE` at runtime and never committed. The mouse
  pointers are the system's, in the shapes the game sets (`platform::Cursor`).
- **Text is the port's own.** The original's texts, strings and formats,
  German and English, are in the repository's `lang/` folder, one folder
  per language ([lang/README.md](../lang/README.md)), compiled into the
  `game` crate (`game::lang`, `include_str!`). The language a player picks
  decides every text of their view, whichever release the game folder is;
  only the speech, the films and the pictures come from the folder
  ([language.md](language.md)).

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
- No world state that follows one player's view: the people's part is
  `fix.view_people`, the news' and the events' `fix.view_news`
  ([below](#decided)).
- No world-side blocking on one player's UI. Interactions that are modal for
  the one human today need a multiplayer behaviour, decided per case when
  they're decompiled: auctions (`TAuktionsform`, which already runs on
  timers; ported with its timers as world ticks and the bids as
  commands, the open question in [auction.md](auction.md#open-questions)),
  the "what now?" dialog (`TWasNunForm`), events, the end-of-game forms.
  Typical answers: the dialog becomes per-player and non-blocking, a decision
  gets a timeout with a default, or the game pauses for everyone. The
  answers are [decided](#decided).
- Save games store seats and keep the original format readable. Single-player
  saves of the original load as a game with one human seat.
- Language is each player's view setting. The world's own text (the
  computers' names, the news archive's dates as the map file keeps them)
  is in the world's language,
  `World::language`: the one the game was created in, in the world's hash
  and saved with it (the port's chunk; a file without it takes the
  folder's `land`), so every peer and every reload makes the same text
  ([language.md](language.md#the-worlds-text)). The view formats the
  rest (dates, money, forms; the archive's dates read back from the
  world's text) in each player's own language.

Not in scope: matchmaking, accounts, or play with the original Windows version.

### Seats and clients

Built in [`game::seats`](../crates/game/src/seats.rs), without any
networking yet. A human is a **client**, with an id and a `View` of their
own; a **seat** is a player number (a colour), 1 to 8, and is either a
computer's or one or more clients' (**co-op**: several people play the
same colour together). The world keeps the clients, their seats and
whether each is connected (`World::seats`, in its hash).

- What a client sends acts for their seat: `World::apply_client` takes
  `(client, action)` (`ClientCommand`, lockstep's message without the
  tick), finds the client's seat and applies the `Command` with that
  seat as its `player`, so a command's `player` stays the seat.
  `World::apply` of a `Command` is the trusted path for a seat's command,
  which the one human of a single-player game uses.
- The **host** is a client, in seat 1. Host-only commands are checked by
  client, not by seat: a co-op partner in seat 1 isn't the host.
- `Action::Seat` changes the seats: a client joins a seat (a computer's,
  or one with clients, for co-op), leaves, disconnects or rejoins. Only
  the host sends them, but a client may send their own leaving; the
  host never leaves or disconnects.
- A seat goes to a computer only when **all** its clients have left or
  disconnected, and back as soon as one of them rejoins
  (`World::is_computer`: 2 to the player count, as the original's
  computers, with no client there). The computer goes on from the state
  it had; while a human plays the seat its turns are skipped and it
  doesn't bid for it.
- The forced fixes count human **clients**, connected or not: two people
  sharing seat 1 still share no view (`Fixes::for_humans`,
  `World::follow_seats`). As the second client joins, the fixes of
  `enhance::SINGLE_PLAYER_ONLY` that are off go on, and the seats
  remember which; when one client is left they go off again. The host
  can't switch them off meanwhile.
- Saved in the port's chunk (`seats.` lines,
  [savegame.md](savegame.md#the-ports-chunk)), with the clients and their
  seats. An original file loads as the host alone in seat 1; loading
  keeps the running game's clients and brings the file's others back
  disconnected.
- A human plays as the human in any seat (`World::is_human`): where the
  original means "the human" by player 1 (the money checks, the books,
  the costs, the computers' handicaps, the build menu, the insurance, the
  info box's and the overview's buttons, a disaster's damage, the road
  drag), the port asks whether a client is at the seat. With one human
  client that's `p == 1`. What the original keeps once, for player 1,
  each of seats 2 to 8 has again (`seats::SeatSlots`,
  `World::seat_slots`: the books' sums, the insurance, the money menu
  shown, the road drag, the damage and a build menu, a copy of seat 1's
  when a client first takes the seat); seat 1 keeps the original's
  fields. Saved in the port's chunk (`seat.<p>.` lines) when they aren't
  a new game's. The views take a `seat` (the tools, the info box, the
  buildings' overview, the money menu, the auction form), 1 for the one
  human.
- With several humans a seat that goes out (bankrupt) is a spectator's
  (`World::spectates`): its clients keep their views and send no
  commands but their own leaving, and the host's own commands.
- With several humans no client's dialog stops the world: the host's
  pause is a command (`Action::Pause`, `Clock::stopped`), which stops
  every peer's world on the same tick; the other clients' pause forms,
  every options form, the file menu, the quit question, the end form and
  the high scores stop nothing. With one human the view stops its own
  clock while they're up, as the original, and `Action::Pause` does
  nothing.
- The tests run several clients in one process: one world per peer and
  a view per client, fed the same commands
  (`crates/game/tests/multiplayer.rs`).

### Decided

The answers to the design issues ([original-bugs.md](original-bugs.md#design-issues-for-multiplayer)).
"Forced on" means the world fix is in `enhance::SINGLE_PLAYER_ONLY`: on
by default, on in every game with more than one human client, and off
only for one human, as the original.

| # | Decision | State |
|---|---|---|
| 1 | The news without a square, the TV archive's view origin and the bankruptcy and net worth events (settlement step 0x64) point at no square of the world; each viewer's TV uses that viewer's own view. A save writes the saving player's origin, so the original can load it. Forced on | Built: `fix.view_news` ([fernseher.md](fernseher.md#multiplayer)) |
| 2 | The computers ignore every human's road drag: they don't wait on `1110:634c` and don't use `704e`'s ground. Forced on | Built: `fix.computer_road_drag`, `fix.computer_road_ground` ([computer.md](computer.md#open-questions)) |
| 3 | The overview map's scroll arrows don't touch the world's new people flag `1110:0056` | Built: `fix.overview_arrows`, forced on ([overview-map.md](overview-map.md)) |
| 4 | The year's total is worked out as if the money menu were shown (`World::money_menu_shown`). Forced on | Built: `fix.year_total` ([geldmenu.md](geldmenu.md#world-and-view)) |
| 5 | Neither the news nor the TV's button pause the game | Built: with more than one human client `World::tv_shown` and `tv_pause` leave `1110:0067` alone, and the seats clear it as the second client joins ([fernseher.md](fernseher.md#multiplayer)) |
| 6 | The TV speaker's lights draw from the view's random numbers, not the world's `RandSeed`, and the auction doesn't wait for the speaker (`Action::Speaker`). Forced on | Built: `fix.tv_lights`, `fix.auction_speaker` |
| 7 | The auction gets a multiplayer form: every seat bids, the world waits for them, with a timeout. Within a co-op seat any client's bid or pass counts for the seat | Built: every human seat has the form; with "Auktion wartet" the world waits until each has started or passed, at most the host's `Options::auction_timeout` ticks (default 300, 30 s; `Action::AuctionTimeout`, saved as `options.auction_timeout`), then the undecided count as passed; without it the auction starts at once. Any seat holding a button stops the computers' bids, and the item goes to the computers only when every human seat has passed (`Auction::started`, `passed`, `held_by`, `waiting`; [auction.md](auction.md#multiplayer)) |
| 8 | The quit question, the end form and the high scores don't stop the clock | Built: with more than one human client they stop nothing, and the end of a game doesn't stop the world; quitting is the client's `SeatChange::Leave`, so a computer plays the seat once its last client has gone ([endforms.md](endforms.md#the-port)) |
| 9 | Winning, losing and bankruptcy per seat (all of a co-op seat's clients win or lose together) | Built for the normal game (scenario 0): each human seat is checked as the original checks player 1 (`World::january_second_seats`); a bankrupt one is out and its clients spectate; the first winner ends the game for all (`Tick::game_over`, each seat's results in `Tick::seat_results`). Scenarios 1–13 still check player 1: phase 3 ([simulation.md](simulation.md#the-scenarios-end-10080512)) |
| 10 | If any human cheats, the whole game counts as cheated | Built: the main screen's cheat is a command (`Action::Cheat`, 1020:3195, en 1020:3194) of any seat, and `1110:006f` is one flag for the game, which takes every seat's score |
| 11 | The host (a client, in seat 1) owns the world's options: speed, pause, animations, "Auktion wartet", disasters and the fixes; the world ignores commands that change them from any other client, deterministically. Other humans take computer seats (2..=8), or join a seat for co-op. A computer plays a seat while all its clients are away, until one rejoins; quitting hands a seat over only when its last client leaves | Built (`World::apply_client`: the speed, the pause, `SetOption`, the auction timeout, `SetFix`, `Load`, `Seat`). The host's pause form stops the world for everyone (`Action::Pause`, `Clock::stopped`) |
| 12 | Saves get a per-seat section (each seat's, and each client's, view) | Built: any client saves their own copy with their view in the original's fields; the seats' slots (`seat.<p>.`) are in the port's chunk, and with `World::to_map_with_views` every client's view (`view.<client>.`, `savegame::client_view`). Only the host loads in the middle of a game ([savegame.md](savegame.md#the-ports-chunk)) |
| 13 | All of this is built before any networking; tests run several clients in one process | Built for 1–12 (`crates/game/tests/multiplayer.rs`) |
| Co-op | Two or more clients may play one seat: commands come from a client and act for its seat, each client has its own view, host-only is by client, and the forced fixes count clients | Built ([above](#seats-and-clients)) |

Built in phase 2 too: the `p == 1` places now ask `World::is_human`
(the money checks, the books, sabotage, the year end's and the month's
costs, the computers' handicaps, the build menu, the info box's and the
overview's buttons, `Shown`, the insurance), and each human seat has its
own road drag (`World::road_drag`). None of this needed a new fix: each
is gated by more than one human client, or is the same as the original's
with one.

Still open, for phase 3: the scenarios 1–13's goals and losses
(`World::scenario_won`, `scenario_lost`) read player 1; Monopolus (3)
needs players 2–8 out, which a human seat only is when it goes out on
the 2nd of January, and the tutorial's, 10's and 12's bankruptcy check
player 1 only. The net worth events of settlement step 0x64 (12, 16, 18)
are player 1's, and a bankrupt human seat's property isn't auctioned as
a computer's is (event 10). A human seat that joins mid-game keeps the
money its computer had (the original's human starts with half the
`Startgeld` as a loan, 1078:3271). The high scores are each view's to
enter, from its seat's results. The host's own quitting and the
networking itself are phase 3.

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
