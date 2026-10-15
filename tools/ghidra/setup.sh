#!/usr/bin/env bash
# (Re)create the headless Ghidra project in ghidra/ and analyze orig/HOLIDAY.EXE.
# Usage: tools/ghidra/setup.sh [--force]    (~half a minute)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GHIDRA="${GHIDRA_INSTALL_DIR:-/opt/ghidra}"
PROJ="$ROOT/ghidra"
SCRIPTS="$ROOT/tools/ghidra/scripts"

if [[ ! -f "$ROOT/orig/HOLIDAY.EXE" ]]; then
	echo "orig/HOLIDAY.EXE missing; run tools/fetch_archive_org.py orig/ first" >&2
	exit 1
fi
if [[ -e "$PROJ/HolidayIsland.gpr" ]]; then
	if [[ "${1:-}" != "--force" ]]; then
		echo "ghidra/HolidayIsland.gpr exists; pass --force to delete and re-create it" >&2
		exit 1
	fi
	rm -rf "$PROJ/HolidayIsland.gpr" "$PROJ/HolidayIsland.rep"
fi
mkdir -p "$PROJ/logs"
rm -f "$PROJ"/logs/setup*

# Import with Ghidra's NE loader (x86:LE:16:Protected Mode), auto-analyze,
# create functions for orphan code, then apply the repo's symbol file.
# HOLIDAY.EXE is Delphi 1: Ghidra's __stdcall16far/near are the right
# conventions except that Pascal pushes arguments left to right. GhCli
# reverses parameter lists for that; see symbols/README.md.
exec 9>"$PROJ/.gh.lock"
flock 9
"$GHIDRA/support/analyzeHeadless" "$PROJ" HolidayIsland \
	-import "$ROOT/orig/HOLIDAY.EXE" \
	-scriptPath "$SCRIPTS" \
	-postScript HolidayPost.java \
	-postScript GhCli.java "$PROJ/logs/setup-apply.out" apply \
	-log "$PROJ/logs/setup.log" -scriptlog "$PROJ/logs/setup-script.log" \
	>"$PROJ/logs/setup.stdout" 2>&1 || { tail -50 "$PROJ/logs/setup.stdout"; exit 1; }
grep -E "HolidayPost|ERROR" "$PROJ/logs/setup-script.log" || true
cat "$PROJ/logs/setup-apply.out" 2>/dev/null || true
echo "done: $PROJ/HolidayIsland.gpr"
