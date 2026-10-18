# The original binaries

All from the German retail CD (volume `HOLIDAY`, single MODE1 data track, 210 MB),
developed by Sunflowers and distributed by Bomico, 1996. The archive.org dump
is a plain ISO (`Holiday Island (1996)(Bomico)(DE).iso`, SHA-1
`96d3704c177f97ad1b0d57619210d057ae8a6f8e`).

The game executable is **not** loose on the CD. `SETUP.EXE` unpacks the
installer from `_SETUP.1`, and the installer copies `_FILES.1` (LZHUF-compressed,
see [formats/install-archive.md](formats/install-archive.md)) to `C:\HOLIDAY`.
`tools/fetch_archive_org.py` and `tools/extract_install.py` do that unpacking.

| File | SHA-256 | What |
|---|---|---|
| `HOLIDAY.EXE` | `dfdfe07f7cc34cc1717c1a1419f2ab773a13d074e8bbcb0d33659c3e6575dd07` | The game, in `_FILES.1`. **Primary decomp target** |
| `_FILES.1` | `5860350600e53854b267b14db28262aab95ddd04d26ab4c58c808c9dc15a49b4` | Installer payload (109 files) |
| `_SETUP.1` | `131a95d88ac73e54870f6fe7334afd66872dc03092b90bfd42f46ab2e95d73b7` | The installer itself (4 files) |
| `SETUP.EXE` | `5c93bca9039dcbedaac53f239ad130fbcdfa568fbb4327924259f15c90df310f` | Delphi 1 bootstrap that unpacks `_SETUP.1` |
| `HINSTALL.EXE` | `739df43402a2b3977826d191356611e4306d2308747f57cc601e22bab8590166` | The installer (Delphi 1), in `_SETUP.1` |
| `UNINST.EXE` | `b77aed7c41f8a97dd90dbfc1279dcab32e7080deb6ce75d47b6c1e0612e9d2dd` | Uninstaller (Delphi 1) |
| `HOLIUVBE.EXE` | `0c193447355141d7f0cb5c5c4ca60acb5eac3c4403109b4539246b62a75847b7` | SciTech UniVBE loader (Watcom C/C++16), see [below](#the-vesa-variant) |
| `WAVEMIX.DLL` | `bccd6ea5572138fc267899449f9dd6373a4d7768fe5f45ba075092ec24cc46c6` | Microsoft WaveMix 1.50 (1993) |
| `WAVMIX16.DLL` | `1e40eee4846f65343bcfd18ba1b0644b7828666829fbcbeecbc8db8622b63985` | Microsoft WaveMix 1.72 (1994) |
| `WDIR61.DLL`, `PMPRO61.DLL`, `VBE\WCONFIG.EXE` | | SciTech WinDirect 6.1 and its config tool (Borland C++) |

Build date: the `_FILES.1` records date `HOLIDAY.EXE` **1996-11-11 00:59**, like
the rest of that build's files. The NE format has no link timestamp.

Everything else in `_FILES.1` (MIDI, WAVs, the tutorial map) is byte-identical
to the copies loose on the CD, except `MUSIK\FLOATIN.MID` and
`SOUND[LO]\HH_HIGH.WAV`, which exist only in the archive.

The CD also carries `VFW1.1E\` (Video for Windows 1.1e runtime, for the Cinepak
AVIs on Windows 3.1) and, by accident, a `RECYCLED\` folder: the mastering
PC's Norton Protect recycle bin with a deleted copy of ACDSee.

## HOLIDAY.EXE

- **NE (16-bit Windows 3.1)**, not PE: linker 6.1, expected Windows version 3.10,
  672,992 bytes. It runs on Windows 3.1 and Windows 95 alike; there is no 32-bit build.
- **Compiled with Borland Delphi 1.0x** (Object Pascal, 16-bit). Evidence: the
  `Borland Delphi` and `Portions Copyright (c) 1983,95 Borland` strings, VCL class
  RTTI (`TApplication`, `TForm`, `TMediaPlayer`, `TBitBtn`, ...), 23 `TPF0` DFM
  form resources (`RCDATA`), and the VCL's `TOOLHELP`/`WIN87EM` imports. The VCL resource
  strings are German (`Ressource %s nicht gefunden`), so it's the German Delphi
  edition. The installer and uninstaller are Delphi 1 too.
- No debug information (no Borland `FB0x` / CodeView signatures) and no map file.
  Names come from RTTI only: classes, published fields and event handlers
  (`tvimageMouseDown`, `Timer1Timer`), and the DFM component names.
- Entry `1000:0011` (segment 1, offset 0x11). DGROUP is segment 35: 0x2d79 bytes initialized,
  0x7786 in memory, plus a 0x2000-byte local heap and a 0x4000-byte stack.
- 34 code segments (601,310 bytes) and one data segment.

### Calling convention

Delphi 1 uses the **Pascal** convention: arguments pushed left to right,
callee pops (`RET n` / `RETF n`), results in `AX` or `DX:AX`. Procedures declared in a
unit's interface, and methods, are `far`. Implementation-only procedures are
`near` unless compiled with `{$F+}`. For methods
`Self` is pushed **last**. `TApplication.CreateForm` at `10f0:6e7b` reads `Self` at
`[bp+6]`, `Reference` at `[bp+10]` and `InstanceClass` at `[bp+14]`.

Ghidra has no Pascal prototype. Its `__stdcall16far`/`__stdcall16near` are the
same except for argument order, so Ghidra shows Pascal functions with their
parameters reversed. `tools/gh` handles that: see
[symbols/README.md](../symbols/README.md).

### Segment map

Ghidra's NE loader maps segment *n* to selector `0x1000 + 8·(n−1)`. Which unit owns
a segment, going by the class RTTI it contains:

| Selector | NE seg | Contents |
|---|---|---|
| `1000`–`10b8` | 1–24 | Game units, ~395 KB. Forms: `THintergrund` (1000), `TBaumenue` (1018), `THauptschirm`/`TEndeform` (1020), `TAuktionsform`/`TSabowin` (1028), `TCreditform` (1030), `TInfobox2`/`TKompassform`/`TLadenForm` (1038), `TDateimenue`/`THelpform`/`THighscoreform`/`TPauseform`/`TWasNunForm` (1040), `TFernseher` (1048), `TStartform` (1058), `TOptionenform` (1060), `TSchlussform` (1068), `TGeldmenuform` (1070), `TUmenuform` (1080), `TUebersichtsmap2` (10b8). `1088`–`10b0` have no classes: plain procedural units, `1088` is the largest segment (35 KB) |
| `1050` | 11 | Starts with a game unit up to about `34d6`: the island drawing and `DARSTELL.DAT` ([map.md](map.md)). The rest looks like VCL `Printers` |
| `10c0` | 25 | Not identified yet |
| `10c8` | 26 | VCL `MPlayer` (`TMediaPlayer`) |
| `10d0`–`10f8` | 27–32 | VCL: `StdCtrls`/`Buttons` (10d0), `ExtCtrls`/`Menus` (10d8), `Graphics` (10e0), `Controls` (10e8), `Forms` (10f0), `Classes` (10f8) |
| `1100` | 33 | RTL `SysUtils` (probably) |
| `1108` | 34 | RTL `System` |

So about two thirds of the code is the game and one third is VCL/RTL. The VCL/RTL
part doesn't need decompiling: the port replaces it. `entry` runs the unit
initializations, `Application.CreateForm` (`10f0:6e7b`) 18 times, then
what is presumably `Application.Run` (`10f0:6f16`).

### OS surface

Imports: `KERNEL`, `USER`, `GDI`, `KEYBOARD`, `MMSYSTEM`, `WAVEMIX`, plus the
VCL's `TOOLHELP` and `WIN87EM`. What the game uses beyond the VCL plumbing:

- **GDI**: device-dependent and DIB bitmaps (`CreateDIBitmap`, `GetDIBits`),
  `BitBlt`/`StretchBlt`/`PatBlt`, palettes (`CreatePalette`, `RealizePalette`),
  even `SetPixel`/`GetPixel`. No WinG and no DirectDraw. 8-bit 256-colour BMPs throughout.
- **MMSYSTEM**: `mciSendCommand` (AVI playback through `TMediaPlayer`/MCIAVI, and
  probably the MIDI music through the MCI sequencer), `midiOutSetVolume`,
  `waveOutGetVolume`/`waveOutSetVolume`.
- **WAVEMIX**: sound effects and speech are played through Microsoft WaveMix
  (`WaveMixConfigureInit`, `WaveMixOpenWave`, `WaveMixOpenChannel`, `WaveMixPlay`, ...),
  which mixes several WAVs into one `waveOut` stream (`WAVEMIX.INI` tunes it).
- **USER**: forms, `SetTimer` and `GetTickCount` for timing, `WinHelp`.
- **KERNEL**: file I/O through the RTL (`_hread`, `_hwrite`, `_llseek`, `_lclose`),
  `LoadLibrary`/`GetProcAddress`, `GetProfileString`/`GetProfileInt`.
  The game looks for the CD and refuses to start without it (`Bitte legen Sie die
  HOLIDAY ISLAND CD ...`).
- **Floating point** goes through `WIN87EM`, and the RTL may use Delphi's 6-byte
  software `Real`. Check which one the simulation uses before porting it: the port
  has to reproduce its rounding.

## The VESA variant

There is no separate VESA build. The installer (`HOLIDAY.CFG`) creates two
Start-menu entries for the same `HOLIDAY.EXE`:

```
FILE1=1,Holiday Island,$DPHOLIDAY.EXE,...
FILE2=0,Holiday Island mit Vesa Bios Erweiterung,$DPHOLIUVBE.EXE,...
```

`HOLIUVBE.EXE` is SciTech's UniVBE loader (Watcom C/C++16 runtime, 1993-96). It
checks for or loads the UniVBE TSR, generates `UNIVBE.DRV` through `WCONFIG.EXE`
(the README's "Autodetect / Manual override" dialog), and then starts
`holiday.exe`. `WDIR61.DLL`/`PMPRO61.DLL` (WinDirect) belong to that toolchain:
`HOLIDAY.EXE` neither imports them nor names them. The README says UniVBE speeds up
graphics output and scrolling on 486s. That's a driver-level speed-up only, so
the port can ignore it.

`HOLI95.CFG` is the same install script plus `WAVEMIX.DLL`; by its name it's
the Windows 95 one. How the installer picks between `WAVEMIX.DLL` (1.50) and
`WAVMIX16.DLL` (1.72) isn't known yet (`HINSTALL.EXE` would tell).

## Other releases

The English UK release (`orig-en/`, unpacked from a DOSBox install by
`tools/extract-hoi-installer.py`) has a later build of `HOLIDAY.EXE`,
the "Erweiterte Version", with fixes and six more scenarios in
`UPGRADE\`; the port follows it. [versions.md](versions.md) has its
hashes, how it differs from this one and how to import it into Ghidra
next to this one. The other archive.org item called "HolidayIsland"
holds a gameplay video and an unidentified `Holiday_Island.exe`, not a
CD image.
