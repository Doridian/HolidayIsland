#!/usr/bin/env python3
"""Download the Holiday Island CD from archive.org and extract it into a directory.

Fetches the ISO of the German release (Sunflowers, distributed by Bomico, 1996)
from https://archive.org/details/Holiday_Island_1996_Bomico_DE, checks it,
extracts the CD's files with extract_cd.py and then the installer's _FILES.1
(HOLIDAY.EXE and the rest of what SETUP.EXE would copy) with
extract_install.py. Pure Python, no external tools needed.

    tools/fetch_archive_org.py orig/
    tools/fetch_archive_org.py --iso "Holiday Island (1996)(Bomico)(DE).iso" orig/
"""
import argparse
import hashlib
import os
import shutil
import sys
import tempfile
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract_cd import extract as extract_cd  # noqa: E402
from extract_install import extract as extract_install  # noqa: E402

URL = "https://archive.org/download/Holiday_Island_1996_Bomico_DE/Holiday%20Island%20(1996)(Bomico)(DE).iso"
ISO_SHA1 = "96d3704c177f97ad1b0d57619210d057ae8a6f8e"

# docs/binaries.md
EXPECTED = {
    "_FILES.1": "5860350600e53854b267b14db28262aab95ddd04d26ab4c58c808c9dc15a49b4",
    "_SETUP.1": "131a95d88ac73e54870f6fe7334afd66872dc03092b90bfd42f46ab2e95d73b7",
    "HOLIDAY.EXE": "dfdfe07f7cc34cc1717c1a1419f2ab773a13d074e8bbcb0d33659c3e6575dd07",
}


def sha1(path):
    h = hashlib.sha1()
    with open(path, "rb") as f:
        while chunk := f.read(1 << 20):
            h.update(chunk)
    return h.hexdigest()


def download(dst):
    print(f"downloading {URL}", file=sys.stderr)
    with urllib.request.urlopen(URL) as r, open(dst, "wb") as f:
        total = int(r.headers.get("Content-Length") or 0)
        done = 0
        while chunk := r.read(1 << 20):
            f.write(chunk)
            done += len(chunk)
            if total and sys.stderr.isatty():
                print(f"\r{done >> 20}/{total >> 20} MB", end="", file=sys.stderr)
    if sys.stderr.isatty():
        print(file=sys.stderr)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("outdir")
    ap.add_argument("--iso", help="use this already downloaded ISO instead of downloading it")
    ap.add_argument("--keep-iso", metavar="PATH", help="save the downloaded ISO here")
    args = ap.parse_args()

    with tempfile.TemporaryDirectory() as tmp:
        iso = args.iso
        if not iso:
            iso = os.path.join(tmp, "holiday.iso")
            download(iso)
        if sha1(iso) != ISO_SHA1:
            sys.exit(f"{iso}: SHA-1 mismatch, expected {ISO_SHA1}")
        if args.keep_iso and not args.iso:
            shutil.copyfile(iso, args.keep_iso)
        hashes = extract_cd(iso, args.outdir)

    hashes.update(extract_install(os.path.join(args.outdir, "_FILES.1"), args.outdir))
    bad = [name for name, want in EXPECTED.items() if hashes.get(name) != want]
    if bad:
        sys.exit(f"unexpected contents: {', '.join(bad)} don't match docs/binaries.md")


if __name__ == "__main__":
    main()
