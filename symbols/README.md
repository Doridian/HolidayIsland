# Symbol files

`<PROGRAM>.txt` (e.g. `HOLIDAY.EXE.txt`) is the shared, text form of what we know
about a binary. The Ghidra project in `ghidra/` is local and disposable; this
file is the source of truth. `tools/gh apply` loads it into Ghidra,
`tools/gh export` writes Ghidra's user-defined names back into it.

## Format

One symbol per line:

```
ADDR KIND NAME [| SIGNATURE] [# COMMENT]

10f0:6e7b func tapplication_create_form | void tapplication_create_form(pointer32 instance_class, pointer32 reference, pointer32 self)
1110:74ca data application | pointer32  # the global Application
```

- `ADDR`: Ghidra's `selector:offset` in hex. NE segment *n* is selector
  `0x1000 + 8·(n−1)`: `1000` is segment 1, `1110` is DGROUP (segment 35).
- `KIND`: `func` (creates the function if Ghidra has none there) or `data` (a label).
- `NAME`: lowercase_snake. Delphi methods are `class_method` without the `T`
  (`hauptschirm_timer1_timer`). Plain procedures get a prefix for their unit
  where it's known. Win16 imports keep Ghidra's names.
- `SIGNATURE` (optional): for `func`, a C prototype; for `data`, a data type
  (`int`, `char[16]`, `pointer32`). Types must be built-in or already exist in
  the Ghidra DB. Far pointers are `pointer32` (4 bytes). A plain `*` is a 2-byte
  near pointer.
- `COMMENT` (optional): free text, kept in the file only.
- Lines starting with `#` are comments. A comment block is attached to the
  entry below it.

### Calling conventions

Delphi 1 compiles everything with the **Pascal** convention (arguments pushed
left to right, callee pops), and methods take `Self` as the **last** argument.
Ghidra has no Pascal prototype, so `tools/gh` maps it onto `__stdcall16far`/`near`
(same except for the argument order):

- A signature **without** a convention, or with `__pascal16far` / `__pascal16near`,
  is written in **Pascal (source) order**. `apply` reverses the parameters,
  applies the `__stdcall16` prototype and tags the function `PASCAL`. Without an
  explicit `__pascal16*`, near or far comes from Ghidra's analysis (RET vs RETF).
  Ghidra then shows the parameters reversed; that's expected.
- `export` reverses the parameters of `PASCAL`-tagged functions back, so the
  file stays in source order.
- An explicit `__cdecl16far`/`__cdecl16near`/`__stdcall16far`/`__stdcall16near`
  is applied as written, in Ghidra's order. Use it for the rare non-Pascal routines
  (RTL helpers that take arguments in registers are better left without a
  signature).

`tools/gh export` and `tools/gh rename` rewrite the file sorted by address, so
keep section comments in address order. Export only adds or updates entries.
It never deletes one, so remove stale lines by hand.

Name only what you're confident about. Use the comment for evidence (strings,
RTTI, DFM event names, call sites) and for guesses.
