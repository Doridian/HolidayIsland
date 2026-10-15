# Installer archives (`_SETUP.1`, `_FILES.1`)

The setup programs are Delphi 1 apps. Their compression component calls itself
`TYsflCompress` (with `TlzhFileStats`, `TlzhNewDiskEvent` events) and stores
files in this archive format. Reversed from the CD's two archives; no
documentation or other users of the format are known. `tools/extract_install.py`
reads it.

All integers are little-endian.

## Layout

```
header   7 bytes
record   135 bytes × count
data     the compressed streams, back to back, in record order
```

### Header

| Offset | Size | |
|---|---|---|
| 0 | u16 | record count (`_SETUP.1`: 4, `_FILES.1`: 109) |
| 2 | 5 | `00 08 00 00 00` in both archives, meaning unknown (it may be multi-disk info) |

### Record

| Offset | Size | |
|---|---|---|
| 0 | u16 | index, 1-based |
| 2 | f64 | file time as a Delphi 1 `TDateTime`: days since 0001-01-01, plus one (later Delphi versions count from 1899-12-30) |
| 10 | u8 | 0 in all records |
| 11 | u32 | offset of the last byte of the compressed data (`offset + packed - 1`) |
| 15 | `string[79]` | source path on the build machine (length byte + 79 chars), e.g. `y:\global\holiday\sound\auf8b.wav` |
| 95 | u32 | `offset + packed + 1`, meaning unknown |
| 99 | u32 | original size |
| 103 | u32 | offset of the compressed data from the start of the archive file |
| 107 | u32 | compressed size |
| 111 | u8 | 0 in all records |
| 112 | `string[22]` | file version (`1.50` for WAVEMIX.DLL, `09/09/96 @` for HINSTALL.EXE, else `0.0`) |

The source paths give the developers' layout: the game was built in
`Y:\GLOBAL\HOLIDAY\` (a network drive), the installer in `D:\HOLIDAY\INSTALL\`.
The game directory structure below `holiday\` is kept on install.

### Compressed data

Each file is one **LZHUF** stream (Okumura/Yoshizaki 1988, `LZHUF.C`): LZSS with
a 4 KB ring buffer pre-filled with spaces, matches of 3–60 bytes, an adaptive
Huffman tree for literals and lengths, a fixed code for the upper 6 position
bits, MSB-first bits. Unlike LZHUF.C's file format there is no 4-byte size
prefix: the size comes from the record. The encoder's final flush sometimes
leaves one byte the decoder never reads. `tools/lzhuf.py` is the decoder.

## Contents

`_SETUP.1`: `HINSTALL.EXE` (the installer proper), `SETUP.LST` (which CD
directories to copy, and the VfW 1.1e setup), `MTEST.MID` and `WTEST.WAV` (the
installer's sound test).

`_FILES.1`: `HOLIDAY.EXE`, `HOLIUVBE.EXE`, the WaveMix and WinDirect DLLs,
`UNINST.EXE`, the install scripts `HOLIDAY.CFG`/`HOLI95.CFG`, `README.TXT`,
`WAVEMIX.INI`, the `VBE\` tools, and copies of `MAPS\TUTORIAL.MAP`, `MUSIK\*.MID`,
`SOUND\*.WAV` and `SOUNDLO\*.WAV`. All of those copies are identical to the CD's
loose files except the ones that exist only here (`MUSIK\FLOATIN.MID`,
`SOUND[LO]\HH_HIGH.WAV`).

`SETUP.DAT`, which the install scripts list, isn't in either archive: the
installer writes it, probably with the chosen options (install path, CD drive,
which directories were copied). `HOLIDAY.EXE` reads `setup.dat` and has the
message `Bitte starten Sie zunächst SETUP.EXE von der HOLIDAY ISLAND CD.`
("please run SETUP.EXE first"). Since the tools only unpack the files, the port
will need to create it or do without it. Its format comes from `HINSTALL.EXE`
(writer) and `HOLIDAY.EXE` (reader).
