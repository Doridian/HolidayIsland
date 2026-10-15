#!/usr/bin/env python3
"""List or extract the Holiday Island installer archives (_FILES.1, _SETUP.1).

The CD's SETUP.EXE (Delphi 1) unpacks _SETUP.1 (the actual installer,
HINSTALL.EXE) and that copies _FILES.1 to the hard disk. _FILES.1 holds
HOLIDAY.EXE and everything else that isn't loose on the CD. Format: see
docs/formats/install-archive.md. Pure Python, no external tools needed.

    tools/extract_install.py -l orig/_FILES.1
    tools/extract_install.py orig/_FILES.1 orig/

Files are written to their install path below the game directory (the part
after "holiday\\"), upper-cased like the CD's ISO9660 names.
"""
import argparse
import datetime
import hashlib
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lzhuf import decode  # noqa: E402

HEADER = 7
RECORD = 135


def entries(data):
    """Yield one dict per archive record."""
    count = struct.unpack_from("<H", data, 0)[0]
    for k in range(count):
        p = HEADER + k * RECORD
        index, days = struct.unpack_from("<Hd", data, p)
        name = data[p + 16 : p + 16 + data[p + 15]].decode("latin-1")
        size, offset, packed = struct.unpack_from("<III", data, p + 99)
        version = data[p + 113 : p + 113 + data[p + 112]].decode("latin-1")
        if index != k + 1 or offset + packed > len(data):
            sys.exit(f"record {k + 1}: unexpected layout (index {index}, offset {offset:#x})")
        yield {
            "index": index,
            # Delphi 1 TDateTime: days since 0001-01-01, plus one.
            "time": datetime.datetime(1, 1, 1) + datetime.timedelta(days=days - 1),
            "path": name,
            "size": size,
            "offset": offset,
            "packed": packed,
            "version": version,
        }


def install_path(path):
    """y:\\global\\holiday\\sound\\x.wav -> SOUND/X.WAV; d:\\holiday\\install\\x -> X."""
    parts = path.split("\\")
    low = [p.lower() for p in parts]
    if "install" in low:
        rel = parts[-1:]
    elif "holiday" in low:
        rel = parts[low.index("holiday") + 1 :]
    else:
        rel = parts[-1:]
    return os.path.join(*(p.upper() for p in rel))


def extract(archive, outdir):
    """Extract every file into outdir; return {relative path: sha256}."""
    data = open(archive, "rb").read()
    hashes = {}
    for e in entries(data):
        out, used = decode(data[e["offset"] : e["offset"] + e["packed"]], e["size"])
        # The encoder's final flush can leave one byte the decoder never reads.
        if not e["packed"] - 1 <= used <= e["packed"]:
            sys.exit(f"{e['path']}: decoded {used} of {e['packed']} bytes, archive is damaged")
        rel = install_path(e["path"])
        dst = os.path.join(outdir, rel)
        os.makedirs(os.path.dirname(dst) or ".", exist_ok=True)
        with open(dst, "wb") as f:
            f.write(out)
        h = hashlib.sha256(out).hexdigest()
        print(f"{h}  {rel}")
        hashes[rel] = h
    return hashes


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-l", "--list", action="store_true", help="list the archive instead of extracting it")
    ap.add_argument("archive", help="_FILES.1 or _SETUP.1")
    ap.add_argument("outdir", nargs="?")
    args = ap.parse_args()
    if args.list:
        for e in entries(open(args.archive, "rb").read()):
            print(f"{e['index']:3}  {e['time']:%Y-%m-%d %H:%M}  {e['size']:8}  {e['packed']:8}  "
                  f"{e['version']:10}  {e['path']}")
        return
    if not args.outdir:
        ap.error("outdir is required unless --list is given")
    extract(args.archive, args.outdir)


if __name__ == "__main__":
    main()
