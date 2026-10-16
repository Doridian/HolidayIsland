# Holiday Island

A decompilation and native port of **Holiday Island** (Sunflowers, 1996) to
Rust, for Linux, macOS and WebAssembly.

Status: **early.** The Cargo workspace exists with a `formats` crate that
parses all the text tables in `DATA\` and `TEXTE\` the way the original's
loaders read them, and splits maps, scenarios and saved games into their
fields, along with the high-score table and `SETUP.DAT`. The startup
sequence is mapped ([docs/startup.md](docs/startup.md)). The `vcl` crate
builds the forms from the layouts in `HOLIDAY.EXE` and draws them with their
bitmaps into an 8-bit framebuffer, and the `holiday` binary opens a window
(`winit` and `wgpu`) that shows the end of the intro. The `game` crate has
the first ported game code: text in the game's own bitmap font. Also here:
- tools that get the game from the CD image, including the installer's
  compressed archive that holds `HOLIDAY.EXE`
- a headless Ghidra setup for the 16-bit executable
- notes on the binaries, the compiler, the data files and startup

The plan is a non-matching decompilation like the
[Afterlife](https://github.com/Doridian/Afterlife) port: the code should behave
like the original, not compile to the same bytes. The planned stack is winit,
wgpu and cpal, with a resizable window and a fullscreen toggle; see
[docs/port-architecture.md](docs/port-architecture.md). A stretch goal for
after the initial port is network multiplayer: humans taking the seats the
original gives to computer opponents. `HOLIDAY.EXE` is a 16-bit
Windows 3.1 program written in **Borland Delphi 1**; [docs/binaries.md](docs/binaries.md)
has the details.

This repository contains **no game data files or original code**. The game data
comes from the original CD, which a script can download for you from the
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

Both print the SHA-256 of every file they write. Only the German release has
been checked so far.

## Running

```sh
cargo run --release -p holiday -- orig
```

The argument is the directory with `HOLIDAY.EXE` and `GRAFIK\`. For now it
shows the title picture and then the background; Escape quits.

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
- `tools/gh`, `tools/ghidra/`: headless Ghidra setup and CLI for the decompilation.

## Docs

- [docs/binaries.md](docs/binaries.md): what's on the CD, what the executables are
  built with (Delphi 1, 16-bit NE), segment map, OS surface, the VESA variant
- [docs/decomp-workflow.md](docs/decomp-workflow.md): approach, Ghidra setup, layout, conventions
- [docs/port-architecture.md](docs/port-architecture.md): the planned Rust port: stack, crates, design
- [docs/widgets.md](docs/widgets.md): the game's own widget toolkit (buttons, scroll bars, ...)
- [docs/data-files.md](docs/data-files.md): survey of the game's data files
- [docs/formats/install-archive.md](docs/formats/install-archive.md): the installer archive format

## Legal

Holiday Island is © Sunflowers Interactive Entertainment Software. This
project is not affiliated with or endorsed by Sunflowers, Bomico or Ubisoft.
It's intended for owners of the original game, for preservation and
interoperability.

The code in this repository is licensed under the
[GNU Affero General Public License v3.0 or later](LICENSE). Game data is not
included and is not covered by this license.
