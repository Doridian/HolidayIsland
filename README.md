# Holiday Island

A decompilation and native port of **Holiday Island** (Sunflowers, 1996) to
Rust, for Linux, macOS and WebAssembly.

**Play it in your browser:** <https://doridian.github.io/HolidayIsland/> (the page
asks for the CD image, [docs/web.md](docs/web.md)).

Status: **early.** The Cargo workspace exists with a `formats` crate that
parses all the text tables in `DATA\` and `TEXTE\` the way the original's
loaders read them, and splits maps, scenarios and saved games into their
fields, along with the high-score table and `SETUP.DAT`. The startup
sequence is mapped ([docs/startup.md](docs/startup.md)). The `vcl` crate
builds the forms from the layouts in `HOLIDAY.EXE` and draws them with their
bitmaps into an 8-bit framebuffer, and the `holiday` binary opens a window
(`winit` and `wgpu`) that plays the intro and shows the start menu. The
`game` crate has the first ported game code: text in the game's own bitmap
font, its widget toolkit, the start menu, and starting a game: the island
from a scenario, or one generated from the start menu's settings, is drawn
on the main screen, which scrolls and turns, and the player builds on it
from the build menu, lays roads, raises and lowers the land and
demolishes ([docs/player-tools.md](docs/player-tools.md)), while the
computer players do the same. Games are saved and loaded in
the original's format, from the start menu's load form and the in-game
file menu ([docs/savegame.md](docs/savegame.md)), and the game's
message boxes and progress window are there too. The sound, the music
and the films (Cinepak and MS ADPCM, decoded in pure Rust,
[docs/films.md](docs/films.md)) play. Also here:
- tools that get the game from the CD image, including the installer's
  compressed archive that holds `HOLIDAY.EXE`
- a headless Ghidra setup for the 16-bit executable
- notes on the binaries, the compiler, the data files and startup

The plan is a non-matching decompilation like the
[Afterlife](https://github.com/Doridian/Afterlife) port: the code should behave
like the original, not compile to the same bytes. The stack is winit,
wgpu and cpal, with a resizable window and a fullscreen toggle; see
[docs/port-architecture.md](docs/port-architecture.md). A stretch goal for
after the initial port is network multiplayer: humans taking the seats the
original gives to computer opponents. `HOLIDAY.EXE` is a 16-bit
Windows 3.1 program written in **Borland Delphi 1**; [docs/binaries.md](docs/binaries.md)
has the details.

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

## Docs

- [docs/binaries.md](docs/binaries.md): what's on the CD, what the executables are
  built with (Delphi 1, 16-bit NE), segment map, OS surface, the VESA variant
- [docs/versions.md](docs/versions.md): the German v1.0 and the later (English) build, how they differ, `UPGRADE\`
- [docs/language.md](docs/language.md): how the original picks its language, and the port's German and English
- [lang/README.md](lang/README.md): the language data: the texts in German and English, and adding a language
- [docs/decomp-workflow.md](docs/decomp-workflow.md): approach, Ghidra setup, layout, conventions
- [docs/port-architecture.md](docs/port-architecture.md): the Rust port: stack, crates, design
- [docs/widgets.md](docs/widgets.md): the game's own widget toolkit (buttons, scroll bars, ...)
- [docs/map.md](docs/map.md): the island's map layers, how the main screen draws them, and its layout
- [docs/island-generator.md](docs/island-generator.md): how a normal game's island is made, and the rest of a new game
- [docs/data-files.md](docs/data-files.md): survey of the game's data files
- [docs/savegame.md](docs/savegame.md): the saved games' format, saving and loading, where saves go
- [docs/formats/install-archive.md](docs/formats/install-archive.md): the installer archive format

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
