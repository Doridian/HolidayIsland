# Holiday Island

A decompilation and native port of **Holiday Island** (Sunflowers, 1996) to
Rust, for Linux, Windows, macOS and WebAssembly.

**Play it in your browser:** <https://doridian.github.io/HolidayIsland/> (the page
asks for the CD image, [docs/web.md](docs/web.md)), or download it for Linux,
Windows or macOS from the [releases](https://github.com/Doridian/HolidayIsland/releases)
([Packages](#packages)).

Status: **playable.** The whole game is ported: the start menu, a
random island or any of the 13 scenarios (the six of the expanded
edition's `UPGRADE\` too), building, roads, terraforming, the guests,
the settlements and the people on the island, the money menu and the
yearly accounts, the computer opponents, the auctions, sabotage, the
events and disasters, the TV news and films, the overview map, the
info box, the options, the high scores and the end of the game. Games
are saved and loaded in the original's format
([docs/savegame.md](docs/savegame.md)), and the sound, the music and
the films (Cinepak and MS ADPCM, decoded in pure Rust,
[docs/films.md](docs/films.md)) play. On top of the original:
- fixes of the original's bugs and some extensions, all of which can
  be switched off ([below](#enhancements))
- multiplayer: humans in the computers' seats, alone or several in one
  seat, over the network natively and in the browser
  ([docs/networking.md](docs/networking.md))
- the browser build ([docs/web.md](docs/web.md))
- tools that get the game from the CD image, including the installer's
  compressed archive that holds `HOLIDAY.EXE`
- a headless Ghidra setup for the 16-bit executable, and notes on the
  binaries, the data files and every part of the game ([Docs](#docs))

The port is a non-matching decompilation like the
[Afterlife](https://github.com/Doridian/Afterlife) port: the code
behaves like the original, but doesn't compile to the same bytes. The
stack is winit, wgpu and cpal, with a resizable window and a fullscreen
toggle; see [docs/port-architecture.md](docs/port-architecture.md).
`HOLIDAY.EXE` is a 16-bit Windows 3.1 program written in **Borland
Delphi 1**; [docs/binaries.md](docs/binaries.md) has the details.

This repository contains **no game data files or original code**, with one
exception: the game's texts. Its German and English texts, strings and
formats are in [lang/](lang/README.md), as UTF-8 text, one folder per
language, so the port shows either language whatever the game folder's.
The rest of the game data (pictures, films, sounds, the data tables) comes
from the original CD, which a script can download for you from the
Internet Archive.

## Getting the game data

```sh
tools/fetch_archive_org.py orig/
```

This downloads the German CD (an ISO, 210 MB) from
<https://archive.org/details/Holiday_Island_1996_Bomico_DE> and checks its
SHA-1. It extracts the CD's files and then the installer archive `_FILES.1`
(`HOLIDAY.EXE`, the DLLs and the rest of what the installer would copy) into
`orig/` (gitignored), and checks the key files against
[docs/binaries.md](docs/binaries.md). `--keep-iso FILE` keeps the ISO, and `--iso FILE`
uses one you already have.

If you have your own image of the CD (ISO or BIN/CUE):

```sh
tools/extract_cd.py /path/to/HolidayIsland.iso orig/
tools/extract_install.py orig/_FILES.1 orig/
```

Both print the SHA-256 of every file they write.

The English UK release comes from The Collection Chamber's DOSBox
installer (`Install-HOI.exe` and `Install-HOI.D01`):

```sh
tools/extract-hoi-installer.py DOWNLOAD_DIR orig-en/
```

Its `HOLIDAY.EXE` is a later build than the German CD's, with fixes and
six more scenarios in `UPGRADE\`. The port follows that build: it
takes its fixes, and plays `UPGRADE\`'s scenarios when the game's folder
has it, so the German folder plays as before and the English one with
the new scenarios. Every text comes from the port's own language data
([lang/](lang/README.md)), German or English: the folder's language by
default, or the one `--lang=de|en` or `language=` in `settings.ini`
picks, so either folder plays fully in either language (the expanded
edition's texts in German are the port's translation). The speech is the
folder's ([docs/language.md](docs/language.md)).
[docs/versions.md](docs/versions.md) has the differences.

## Packages

Every push builds the port as one file per platform
(`.github/workflows/build.yml`); a `v*` tag publishes them on its release:

- `holiday-island-linux-x86_64.AppImage`, `holiday-island-linux-arm64.AppImage`
- `holiday-island-windows-x86_64.exe` (the C runtime linked in)
- `holiday-island-macos.dmg`: `Holiday Island.app`, for Apple silicon and
  Intel (signed ad hoc only: the first start needs a right click, Open)
- `holiday-island-web.zip`: the browser build, as on the Pages site

None of them carries game data. Started without the game's folder (from
its icon) and with no `orig` in the working directory, the port asks for
the game with the system's file dialog: the CD image (`.iso`, `.bin`) or
`HOLIDAY.EXE` in the game's folder (the German CD's files, the English
release's). If the music has no SoundFont yet ([Music](#music)), it then
asks for one too, which it copies to its data directory; cancelled, the
music is silent. It keeps the game's path in its data directory
(`game-path.txt` in `~/.local/share/holiday-island`,
`%APPDATA%\Holiday Island`, `~/Library/Application Support/Holiday Island`)
and doesn't ask again while it's there; deleting the file asks again. On
Linux the dialog is the desktop's (xdg-desktop-portal, else zenity).
The icon is the port's own (`tools/make-icon.py`).

## Running

```sh
cargo run --release -p holiday -- orig
```

The argument is the directory with the game's files (`HOLIDAY.EXE`,
`GRAFIK\`, `DATA\`, the films and sounds); its texts are the port's own
([lang/](lang/README.md)), in German or English (`--lang=de|en`). It plays the intro, then shows the
background with the start menu on it. Start shows the main screen with
the scenario's island, or a new island made from the menu's settings
for a normal game; "Altes Spiel fortsetzen" loads a saved one. The arrow keys and the
number pad scroll it (Ctrl for bigger steps), so do its scroll bars and
the mouse wheel (Shift for across), Insert and Delete turn it, a right
click moves it. The button bar's tool buttons (or F1 to F12)
pick the tools; a click on the island uses them. The button bar's
first button opens the file menu (saving, loading, a new game);
Escape asks whether to quit.

### Enhancements

Fixes of the original's bugs and the port's extensions (a bigger map in a
bigger window without a second title bar, windows that stack like a
desktop's, scroll arrows that repeat, the mouse wheel) are on by default.
In a game, the options' "INFO" button lists them to switch one by one,
or all off for the original; the list saves them in the port's
`settings.ini`. `--original` switches everything off for a run (and
"INFO" shows the credits again), `--enhance=KEY=on|off` one of them, and
`--enhancements` lists the keys (`--help` the other options). Details in
[docs/original-bugs.md](docs/original-bugs.md#options).

### Multiplayer

Humans take the seats the original gives to computer opponents; two or
more can play the same seat. A signaling server (`holiday-server`,
`cargo run --release -p server`) introduces the players, who then talk
directly over WebRTC:

```sh
holiday --host=island --signal=ws://server:3536 orig  # the host picks the game on the start menu
holiday --join=island --signal=ws://server:3536 orig  # each other player
```

The browser build joins the same games. German and English players can
share a game. Details in [docs/networking.md](docs/networking.md).

### Music

The game's music is MIDI, and the port renders it with a SoundFont, which
the game doesn't ship. `tools/get-soundfont.sh` downloads
[GeneralUser GS](https://github.com/mrbumpy409/GeneralUser-GS) by
S. Christian Collins (GS-compatible, like the Roland Sound Canvas the music
was written on) into the port's data directory, where the game finds it.
`--soundfont=FILE.sf2` or `HOLIDAY_SOUNDFONT` pick another; without one the
music is silent. `--sound=hi|lo|off` picks the effects' quality and
`--no-audio` turns all sound off. Details in [docs/sound.md](docs/sound.md).

## Decompiling

Needs Ghidra 12 at `/opt/ghidra` (or `GHIDRA_INSTALL_DIR`) and Java 21+.

```sh
tools/ghidra/setup.sh            # import and analyze orig/HOLIDAY.EXE (~15 s)
tools/gh decompile 1000:0011     # the entry point
tools/gh pstrings -x CD          # Pascal strings, with referencing functions
```

[docs/decomp-workflow.md](docs/decomp-workflow.md) covers the approach, the
Ghidra tooling, Delphi 1 specifics and the rules. Names and signatures are
shared through [symbols/](symbols/README.md).

## Tools

- `tools/fetch_archive_org.py`: downloads the CD from archive.org and extracts it.
- `tools/extract_cd.py`: pure-Python ISO9660 extractor for ISO and BIN/CUE images.
- `tools/extract_install.py`: lists or extracts the installer archives (`_FILES.1`, `_SETUP.1`).
- `tools/lzhuf.py`: the LZHUF decoder those archives use.
- `tools/extract-hoi-installer.py`: unpacks the English release from its DOSBox installer.
- `tools/gh`, `tools/ghidra/`: headless Ghidra setup and CLI for the decompilation.
- `tools/get-soundfont.sh`: downloads the SoundFont the music plays with.
- `tools/build-web.sh`: builds the browser version into `web/`.

## Docs

The port and the project:
- [docs/port-architecture.md](docs/port-architecture.md): the Rust port: stack, crates, design, the seats and clients
- [docs/decomp-workflow.md](docs/decomp-workflow.md): approach, Ghidra setup, layout, conventions
- [docs/original-bugs.md](docs/original-bugs.md): every bug of the original found so far, and the enhancements that fix them
- [docs/networking.md](docs/networking.md): network games, the signaling server, the lockstep session
- [docs/web.md](docs/web.md): the browser build
- [docs/binaries.md](docs/binaries.md): what's on the CD, what the executables are
  built with (Delphi 1, 16-bit NE), segment map, OS surface, the VESA variant
- [docs/versions.md](docs/versions.md): the German v1.0 and the later (English) build, how they differ, `UPGRADE\`
- [docs/language.md](docs/language.md): how the original picks its language, and the port's German and English
- [lang/README.md](lang/README.md): the language data: the texts in German and English, and adding a language
- [docs/data-files.md](docs/data-files.md): survey of the game's data files
- [docs/formats/install-archive.md](docs/formats/install-archive.md): the installer archive format

The game:
- [docs/startup.md](docs/startup.md): what the game does from launch to the start menu
- [docs/layout.md](docs/layout.md): the windows, where they go, how they stack
- [docs/widgets.md](docs/widgets.md): the game's own widget toolkit (buttons, scroll bars, ...)
- [docs/map.md](docs/map.md): the island's map layers, how the main screen draws them, and its layout
- [docs/island-generator.md](docs/island-generator.md): how a normal game's island is made, and the rest of a new game
- [docs/player-tools.md](docs/player-tools.md): building, roads, raising and lowering the land, demolishing
- [docs/simulation.md](docs/simulation.md): the game's tick: guests, buildings, money, events, the end of a year
- [docs/settlement.md](docs/settlement.md): the plots and how the guests settle
- [docs/people.md](docs/people.md): the people, ships, boats and planes on the island
- [docs/computer.md](docs/computer.md): the computer opponents
- [docs/auction.md](docs/auction.md): the auctions
- [docs/sabotage.md](docs/sabotage.md): sabotage
- [docs/geldmenu.md](docs/geldmenu.md): the money menu
- [docs/infobox.md](docs/infobox.md): the info box
- [docs/overview-map.md](docs/overview-map.md): the overview map
- [docs/fernseher.md](docs/fernseher.md): the TV and the news
- [docs/options.md](docs/options.md): the options, and the port's list of enhancements
- [docs/endforms.md](docs/endforms.md): the end of a game, the high scores, quitting
- [docs/savegame.md](docs/savegame.md): the saved games' format, saving and loading, where saves go
- [docs/sound.md](docs/sound.md): the sound effects, the speech and the music
- [docs/films.md](docs/films.md): the films and their codecs

## Legal

Holiday Island is © Sunflowers Interactive Entertainment Software. This
project is not affiliated with or endorsed by Sunflowers, Bomico or Ubisoft.
It's intended for owners of the original game, for preservation and
interoperability.

The code in this repository is licensed under the
[GNU Affero General Public License v3.0 or later](LICENSE). The game's texts
in [lang/](lang/README.md) are Sunflowers' (but the port's own strings and
translations) and aren't covered by this license; the rest of the game data
isn't included.
