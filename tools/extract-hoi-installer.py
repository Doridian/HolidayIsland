#!/usr/bin/env python3
"""Unpack the English Holiday Island from The Collection Chamber's installer.

The download (`Install-HOI.exe` plus `Install-HOI.D01`) is a Clickteam
Install Creator archive holding a DOSBox setup: a Windows 3.1 hard disk
with the game installed (`HDD\\HOLIDAY\\`) and the CD as `CD\\track01.iso`.
This writes a game folder the port can run from: the CD's files with the
installed ones on top (the installed `HOLIDAY.EXE` is the upgraded build,
the "Erweiterte Version", which also reads `UPGRADE\\`).

    tools/extract-hoi-installer.py DOWNLOAD_DIR OUT_DIR

Needs `bsdtar` to unpack the ISO. Nothing in the archive is run.

The archive's layout, as far as it's needed here:
- blocks: u16 id, u16 flags, u32 length, then the content; the file list
  is block 0x143a (u32 raw size, u8 method, compressed data) and every
  file's data is in block 0x7f7f, which runs on into the `.D01` (whose
  first 4 bytes are a header, not data).
- a file-list entry: u16 size, u16 type, u16, u32 offset in the data
  block, u32 compressed size, u32, u32 raw size, ..., name at +68
  (NUL-terminated, cp1252).
- a file's data: u32 (a checksum), u8 method (0 stored, 1 zlib,
  2 bzip2), then `compressed size - 5` bytes.
"""

import bz2
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

LIST_BLOCK = 0x143A
DATA_BLOCK = 0x7F7F
SPLIT_HEADER = 4


def blocks(data, start):
    """Yield (id, content offset, length) from `start`."""
    p = start
    while p + 8 <= len(data):
        bid, _flags, ln = struct.unpack_from("<HHI", data, p)
        yield bid, p + 8, ln
        if bid == DATA_BLOCK:
            return
        p += 8 + ln


def first_block(data):
    """The first block comes after the installer's own code: find a list block."""
    for m in re.finditer(rb"\x3a\x14\x01\x00", data):
        p = m.start()
        ln = struct.unpack_from("<I", data, p + 4)[0]
        q = p + 8 + ln
        if q + 8 <= len(data) and struct.unpack_from("<H", data, q)[0] in (DATA_BLOCK, 0x143A, 0x143E, 0x143F, 0x1445):
            # Walk back is unnecessary: blocks() only needs the list and data blocks.
            return p
    sys.exit("no file list found")


def unpack(method, chunk):
    if method == 0:
        return chunk
    if method == 1:
        return zlib.decompress(chunk)
    if method == 2:
        dec = bz2.BZ2Decompressor()
        out = bytearray()
        step = 1 << 22
        for i in range(0, len(chunk), step):
            out += dec.decompress(chunk[i : i + step])
            if dec.eof:
                break
        return bytes(out)
    raise ValueError(f"method {method}")


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    src, out = sys.argv[1], sys.argv[2]
    with open(os.path.join(src, "Install-HOI.exe"), "rb") as f:
        data = f.read()
    d01 = os.path.join(src, "Install-HOI.D01")
    if os.path.exists(d01):
        with open(d01, "rb") as f:
            data += f.read()[SPLIT_HEADER:]

    listing = base = None
    for bid, at, ln in blocks(data, first_block(data)):
        if bid == LIST_BLOCK:
            raw, method = struct.unpack_from("<IB", data, at)
            listing = unpack(method, data[at + 5 : at + ln])
        elif bid == DATA_BLOCK:
            base = at
    if listing is None or base is None:
        sys.exit("file list or data block missing")

    os.makedirs(out, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        written = 0
        for m in re.finditer(rb"(HDD\\HOLIDAY\\|CD\\)[^\x00]+\x00", listing):
            e = m.start() - 68
            off, comp, _, raw = struct.unpack_from("<IIII", listing, e + 6)
            name = m.group()[:-1].decode("cp1252")
            p = base + off + 4
            body = unpack(data[p], data[p + 1 : p + comp])
            if len(body) != raw:
                print(f"skipped {name}: {len(body)} of {raw} bytes", file=sys.stderr)
                continue
            if name.startswith("CD\\"):
                path = os.path.join(tmp, name[3:])
            else:
                path = os.path.join(out, *name.split("\\")[2:])
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "wb") as f:
                f.write(body)
            written += 1

        iso = os.path.join(tmp, "track01.iso")
        cd = os.path.join(tmp, "cd")
        os.makedirs(cd)
        subprocess.run(["bsdtar", "-xf", iso, "-C", cd], check=True)
        # The CD first, then the installed files over it.
        for root, _dirs, files in os.walk(cd):
            for name in files:
                rel = os.path.relpath(os.path.join(root, name), cd)
                dest = os.path.join(out, rel)
                if os.path.exists(dest):
                    continue
                os.makedirs(os.path.dirname(dest), exist_ok=True)
                shutil.copyfile(os.path.join(root, name), dest)
    print(f"{written} files from the archive, CD unpacked into {out}")


if __name__ == "__main__":
    main()
