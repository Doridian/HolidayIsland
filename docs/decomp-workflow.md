# Decompilation workflow

## Approach: non-matching, port-as-you-go

The goal is a native port to Rust (Linux, macOS, WebAssembly), not a
byte-identical rebuild. The stack and structure are in
[port-architecture.md](port-architecture.md).

- `HOLIDAY.EXE` is a 16-bit Windows 3.1 program written in Delphi 1 (Object
  Pascal), see [binaries.md](binaries.md). A *matching* decomp would mean
  rewriting it in Delphi 1 Pascal for a 16-bit compiler, and the result
  wouldn't be portable.
- So each routine is decompiled in Ghidra, rewritten as readable Rust, and
  checked by behaviour (same output and state as the original) rather than by
  bytes.
- Game code never touches the platform directly: it talks to the `vcl` crate
  and to platform traits, so it also runs headless in tests.

### The VCL and RTL are replaced, not decompiled

About a third of the code (selectors `10c8`–`1108`, plus the end of `1050`) is Borland's VCL
and RTL. The port doesn't decompile it. Instead the `vcl` crate reimplements the
parts the game uses: forms, `TImage`, `TPanel`, `TLabel`, `TBitBtn`, `TTimer`,
`TMediaPlayer`, the `TCanvas` drawing calls, the `Application` loop, and Pascal
strings. The **DFM form resources** (`RCDATA`, `TPF0` binary format) describe the
forms' components, properties and event bindings. The port reads them from the
user's `HOLIDAY.EXE` at runtime, so the forms work the way the designers laid
them out. Identifying VCL routines in Ghidra is still useful: name them so the
game code calling them reads well. Borland sold the Delphi 1 VCL source (with the
Client/Server edition), and the class layouts (`TControl`, `TForm`, ...) are
well documented, so these routines are easy to recognize.

### Delphi 1 types

Things to keep in mind when turning decompiled Pascal into Rust:

- `Integer`/`Cardinal` are **16-bit**, `Longint` is 32-bit, `Word` 16, `Byte` 8.
  Use `i16`/`u16`/`i32`/`u8` to match, and `wrapping_*` where the original
  relies on wraparound: overflow behaviour matters for the simulation, and
  debug builds panic on overflow, which flags those places.
- `string` is a Pascal `ShortString`: a length byte and up to 255 chars,
  `string[N]` caps it at N. Literals are stored that way, so `tools/gh pstrings`
  finds them, not `strings`.
- `Real` is Borland's 6-byte software float, separate from `Single`/`Double`/`Extended`
  (x87 or the `WIN87EM` emulator). Find out which ones the game uses before
  porting its arithmetic: matching results means matching rounding.
- Sets are bitmaps of up to 32 bytes. Check each record's field offsets in the
  code rather than assuming an alignment.
- Far pointers are `segment:offset`: 4 bytes in memory and in any saved
  struct. Saved games (`*.map`, `*.HIS`) are parsed field by field, never by
  transmuting bytes into a struct.
- Arrays can have any lower bound; Ghidra shows the offset arithmetic.

## Ghidra setup

Needs Ghidra 12 at `/opt/ghidra` (or set `GHIDRA_INSTALL_DIR`) and Java 21+.

1. `tools/fetch_archive_org.py orig/` (or `tools/extract_cd.py` +
   `tools/extract_install.py orig/_FILES.1 orig/` from your own image)
2. `tools/ghidra/setup.sh` imports `orig/HOLIDAY.EXE` into `ghidra/HolidayIsland.gpr`
   (gitignored) with Ghidra's NE loader, auto-analyzes it (~15 s), creates
   functions for orphan code (`HolidayPost.java`) and applies
   `symbols/HOLIDAY.EXE.txt`. `--force` rebuilds it from scratch.
   `--en` adds the later build (`orig-en/HOLIDAY.EXE`) as a second program,
   `HOLIDAY_EN.EXE`, with `symbols/HOLIDAY_EN.EXE.txt`; query it with
   `GH_PROGRAM=HOLIDAY_EN.EXE tools/gh ...` ([versions.md](versions.md)).
3. Query it with `tools/gh` (`tools/gh --help`):

   ```
   tools/gh funcs [-u] [regex]          # addr, size, name (-u: user-named only)
   tools/gh decompile <addr|name>       # C for the containing function
   tools/gh disasm <addr|name>          # disassembly of the function
   tools/gh listing <addr|name> [n]     # n code units (data or code) from addr
   tools/gh xrefs <addr|name>           # references to addr
   tools/gh strings [-x] [regex]        # strings Ghidra defined (-x: referencing functions)
   tools/gh pstrings [-x] [regex]       # Pascal strings anywhere in memory
   tools/gh rename <addr|name> <new> [func|data]   # DB and symbol file
   tools/gh apply                       # symbols/HOLIDAY.EXE.txt -> DB
   tools/gh export                      # DB user names -> symbols/HOLIDAY.EXE.txt
   printf 'decompile 1000:0011\nxrefs 10f0:6e7b\n' | tools/gh batch
   ```

   Addresses are Ghidra's `selector:offset`. Segment *n* of the NE file is
   selector `0x1000 + 8·(n−1)` (so `1000` is segment 1, DGROUP is `1110`).
   Each call starts a JVM (~3 s). `batch` runs many commands in one JVM.
   Calls are serialized with `flock` on `ghidra/.gh.lock` (Ghidra can't open a
   project twice), so parallel callers are safe but queue up. Query commands
   open the program read-only; only `apply` and `rename` save to the DB.

What to expect from Ghidra on this binary:

- Analysis finds about 3,300 functions. Imports are named (`BITBLT`,
  `WAVEMIXPLAY`, ...); everything else is `FUN_ssss_oooo`.
- Most functions get `__stdcall16far`/`__stdcall16near`, which is Pascal with the
  parameters listed in reverse. Signatures in the symbol file are written in
  Pascal order, and `tools/gh` reverses them (see
  [symbols/README.md](../symbols/README.md)).
- Code-segment literals are loaded as `CS:offset` immediates, which Ghidra doesn't
  turn into references, so `pstrings -x` often shows `(none)`. Search the
  disassembly for the offset instead. `pstrings` also finds some false positives
  in code bytes.
- Far pointers show up as two 16-bit halves (`(int)x, (int)((ulong)x >> 0x10)`)
  until the parameters get 4-byte types. Ghidra's built-in `pointer32` works in
  signatures.
- `DGROUP` (`1110`) is stored as iterated data in the NE file, and the loader
  doesn't expand it, so Ghidra's initialised data in `1110` is wrong (strings,
  tables, VMTs). Addresses are still right. See [widgets.md](widgets.md#finding-the-vmts).

The Ghidra project is binary and contains copyrighted code, so it stays out of
git. Knowledge belongs in the repo as text: names, signatures and notes go in
`symbols/` (format in `symbols/README.md`). Don't commit disassembly or
decompiler dumps.

## Running the original

To check behaviour against the original, run it under Windows 3.1x or 95. The
game needs its CD and the `SETUP.DAT` the installer writes, so install it from
the CD image. Options:

- An emulated PC (86Box, PCem, DOSBox-X with Windows 3.11 or 95). This is the
  most faithful. On Windows 3.1 install `VFW1.1E\` from the CD for the AVIs.
- Wine with Win16 support (a 32-bit/old-WoW64 build; the new WoW64 mode can't
  run 16-bit code).
- [otvdm/winevdm](https://github.com/otya128/winevdm) on 64-bit Windows.

The Afterlife project's differential tests run original functions in a Unicorn
emulator. That's harder here: 16-bit protected mode needs a descriptor table
and NE relocations set up first. This tooling doesn't exist yet.

## Layout

```
crates/         the Rust port (see port-architecture.md)
tools/          Python helpers: CD and installer extraction, Ghidra CLI
docs/           format, binary and architecture notes
symbols/        what we know about HOLIDAY.EXE, as text
```

## Rules

- **Never commit original game data or binaries**: no game files, disassembly
  or decompiler dumps, no machine code, and no image, video or sound data
  (bitmaps, icons, cursors, palettes, DFM picture data, films, WAVs, MIDI).
  Users supply their own CD; the port loads the data files and
  `HOLIDAY.EXE`'s resources (DFMs, cursors, icon) from it at runtime.
- **The exception is text**: the original's texts, strings, numbers and
  formats, in German and in the official English translation, are in the
  repository as language data, one folder per language
  ([lang/README.md](../lang/README.md), [language.md](language.md)): every
  `TEXTE\` and `UPGRADE\` text file as UTF-8, the label layouts that differ
  by translation, and `strings.toml` with the EXE's strings and the
  formats. That's where text goes, not into the code. The data tables of
  `DATA\` (numbers that play) still come from the game folder.
- **EXE number tables live as named data in their modules**, the way the
  original authors had them: `const`/`static` arrays and struct tables in
  the modules that use them, with meaningful names and types and the
  original address in a comment on each item (`// 1110:15e0`), like the
  functions. EXE strings that are text go into `lang/<code>/strings.toml`,
  with the address in a comment there, and the code names them by key
  (`game::lang::Text`). Take them from the EXE with `tools/gh`; most of
  the game's text is in `TEXTE\*.TXT` anyway.
- Name functions after what they do. Note the original address in a comment
  (`// 10f0:6e7b`) so it can be traced back. Original units become modules, and
  Delphi methods keep their form's name (`hauptschirm::timer1_timer`).
- Keep the simulation (`World`) and the player's UI (`View`) apart, change the
  world only through commands, and keep world code deterministic: no wall
  clock, no stray randomness, no `HashMap` iteration, no platform math functions.
  Mark places where the original assumes one human (player 1), and anywhere
  the simulation waits on a dialog. These rules keep the
  [multiplayer stretch goal](port-architecture.md#stretch-goal-network-multiplayer)
  possible.
- Keep the original's bugs and quirks, so the port behaves the same, but
  record each one. Mark it in the code with `// BUG(original): ...` and add
  it to [original-bugs.md](original-bugs.md), so it can be made optionally
  fixable later. Where the original crashes or hangs, pick a safe fallback
  and record that as well.
- Parse all data files field by field, little-endian, with bounds checks:
  no `unsafe` in the parsers or the game crate.
- `cargo fmt` and `cargo clippy` clean, natively
  (`cargo clippy --workspace --all-targets -- -D warnings`) and for the
  browser build ([web.md](web.md)): `cargo clippy -p formats -p vcl -p game
  -p platform -p net -p holiday --target wasm32-unknown-unknown -- -D warnings`
  (the libraries and the binary; the tests and `server` are native only).

## Suggested order

1. **Data files**: the `DATA\*.DAT` tables and `TEXTE\*.TXT`, the map and scenario
   format (`*.MAP`, `*.HIS`), the high-score file. Most of this can be done from
   the files with the EXE as a reference.
2. **Startup**: CD and `SETUP.DAT` checks, unit initialization, the
   `Application.CreateForm` sequence (`entry`), `TStartform`.
3. **VCL subset and DFM reader**: enough to put the forms on screen with their
   bitmaps.
4. **Map rendering** and scrolling (`THintergrund`, `THauptschirm`,
   `TUebersichtsmap2`).
5. **Simulation**: the procedural units in `1088`–`10b0`, the economy, events
   (`EREIGN*.DAT`), auctions, sabotage.
6. **Media**: WaveMix-style mixing of the WAVs, MIDI music (`rustysynth`),
   Cinepak + MS ADPCM AVI playback for the cutscenes.
7. **Arbitrary window sizes**: once the layout code is understood, let the
   screen size follow the window (see port-architecture.md).
