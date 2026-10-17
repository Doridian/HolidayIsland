#!/bin/sh
# Downloads GeneralUser GS (S. Christian Collins), a GS-compatible General
# MIDI SoundFont, and its licence into the port's data directory, where the
# game finds it (docs/sound.md#the-music). The SoundFont isn't in the repo.
#
# Usage: tools/get-soundfont.sh [DIR]   (default: the data directory below)
set -eu

COMMIT=684543d5e5efaef08d02be50dcda8d552478fa60
BASE="https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/$COMMIT"
SF2=GeneralUser-GS.sf2
SIZE=32319396
SHA256=9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe

# platform::storage::data_dir() + /soundfont
if [ $# -ge 1 ]; then
	DIR=$1
elif [ "$(uname -s)" = Darwin ]; then
	DIR="$HOME/Library/Application Support/Holiday Island/soundfont"
else
	DIR="${XDG_DATA_HOME:-$HOME/.local/share}/holiday-island/soundfont"
fi

fetch() { # url file
	if command -v curl >/dev/null 2>&1; then
		curl -fL --retry 3 -o "$2" "$1"
	elif command -v wget >/dev/null 2>&1; then
		wget -O "$2" "$1"
	else
		echo "get-soundfont: needs curl or wget" >&2
		exit 1
	fi
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d' ' -f1
	else
		shasum -a 256 "$1" | cut -d' ' -f1
	fi
}

mkdir -p "$DIR"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM

fetch "$BASE/$SF2" "$TMP/$SF2"
fetch "$BASE/documentation/LICENSE.txt" "$TMP/LICENSE.txt"

size=$(wc -c <"$TMP/$SF2" | tr -d ' ')
if [ "$size" != "$SIZE" ]; then
	echo "get-soundfont: $SF2 is $size bytes, expected $SIZE" >&2
	exit 1
fi
sum=$(sha256 "$TMP/$SF2")
if [ "$sum" != "$SHA256" ]; then
	echo "get-soundfont: $SF2 has SHA-256 $sum, expected $SHA256" >&2
	exit 1
fi

mv "$TMP/$SF2" "$DIR/$SF2"
mv "$TMP/LICENSE.txt" "$DIR/LICENSE.txt"
echo "installed $DIR/$SF2"
