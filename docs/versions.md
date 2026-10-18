# The two builds of HOLIDAY.EXE

Two builds of the game are known:

- **German v1.0** (`orig/`, the German retail CD, [binaries.md](binaries.md)):
  the build the port was first written against. Addresses without a
  build in the docs and the code are this one's.
- **The later build**, the "Erweiterte Version" (`orig-en/`, the English
  UK release, unpacked by `tools/extract-hoi-installer.py` from a DOSBox
  install: the installed `HOLIDAY.EXE` is the upgraded build). It reads a
  folder `UPGRADE\` with six more scenarios and their files. It's a later
  compile of the same source with fixes, the new scenarios and the
  English texts. **The port follows this build**; its addresses are
  written `en 1028:3620`.

German v1.0 saves load in either, and a German folder without `UPGRADE\`
plays as v1.0 content (the port's choice: the later build itself halts
without the folder's files).

## Telling them apart

| | German v1.0 | Later build |
|---|---|---|
| `HOLIDAY.EXE` size | 672,992 bytes | 688,624 bytes |
| SHA-256 | `dfdfe07f…dd07` | `c9b24b2c5a25f2909d249ccfb726d3876827614261e8a67184869ed62713ba5d` |
| NE segments | 35 (DGROUP is 35, `1110`) | 36 (DGROUP is 36, `1118`) |
| DGROUP | 0x2d79 initialized (expanded 0x4a82) | expanded 0x4c12 |
| Map version written (`1110:004e`) | `1.000` | `1.100` (the format is unchanged; neither checks it) |
| Speech folders | `SPRADEU1\`–`SPRADEU3\` | `Sprache1\`–`Sprache3\` (only the names in `init_paths`) |
| Extra folder | none | `UPGRADE\`, on the hard disk next to `MAPS\` |
| Films the CD check wants | 21 | 27: also `ABFries`, `ABCamper`, `ABBBonly`, `ABPRWund`, `ABHurry`, `ABChalle.avi` |
| `MODATYP.DAT` line 2 (`land`) | `D` | `E` (the English data; not a build difference) |

The later build also calls `SetErrorMode(0x8001)` around
`Application.Run` (en 1000:0011), so a missing CD gives the game's own
message instead of Windows' "drive not ready" box.

`LAST.MAP` from the later build parses with `formats::savegame` like any
v1.0 file; its version string is the only difference in the header. The
six `UPGRADE\*.HIS` files are ordinary maps with `scenario` (`1110:006e`)
8 to 13.

## Ghidra

The later build is a second program in the same project, so the German
one stays as it is:

```sh
tools/ghidra/setup.sh --en        # imports orig-en/HOLIDAY.EXE as HOLIDAY_EN.EXE
GH_PROGRAM=HOLIDAY_EN.EXE tools/gh decompile 1028:3620
```

`symbols/HOLIDAY_EN.EXE.txt` names its functions and data, taken over
from `symbols/HOLIDAY.EXE.txt` through the function match (below); each
line says the German address (`# de 1028:26e4`), and `changed` marks a
function whose code differs. DGROUP is selector `1118`.

### Segments

The Delphi linker fills code segments unit by unit, so a bigger unit
moves the units after it. A new segment (en 14, `1068`) holds the high
score form, and the units of de 1–9 are spread differently:

| de | en | Contents |
|---|---|---|
| 1 (`1000`) | 1, 3 | entry, paths; the background form (`THintergrund`) moved to en 3 (`1010`) |
| 2 (`1008`) | 2 | settlement, scenario conditions |
| 3 (`1010`) | 1, 3, 6 | widgets, button bar (en 1), events (en 6, `1028`) |
| 4–8 | 4–9 | build menu, main screen, auction, sabotage window (en 7), money, text, compass, load form (en 9) |
| 9 (`1040`) | 9, 14, 6, 1 | file menu (en 9), high scores (en 14, `1068`), WasNun/pause (en 6), help (en 1) |
| 10–13 | 10–13 | TV, island drawing, start menu, options |
| 14–34 | 15–35 | one up: end form `1068`→`1070`, money menu `1070`→`1078`, ..., computers `10a8`→`10b0`, generator `10b0`→`10b8`, overview map `10b8`→`10c0`, VCL and RTL `10c0`–`1108`→`10c8`–`1110` |
| 21 (`10a0`) | 22, 9 | events and sabotage (en 22, `10a8`); the sound unit moved to en 9 (`1040:3128`–`1040:3d8b`) |

DGROUP grew: the initialized data is the same up to 0x189c, gains 16
bytes there (two more file menu rects), and has RTTI for the new forms'
handlers before 0x2522; from there to the end of the typed constants
everything is 0x190 further on (so the game's tables at
`1110:34b0`–`1110:3d60` are byte for byte the same). The uninitialized
variables move by 0x190 to 0x82c (`1110:6f2c` money is en `1118:7758`).

## How the diff was made

Function by function: both programs' functions (Ghidra's, plus the
starts the symbol file creates) were disassembled with relocations
resolved and compared by the shape of their instructions; equal shapes
match first, then neighbours in order, then the most similar. Then each
matched pair was compared operand by operand with the DGROUP offsets,
far calls and code-segment strings mapped across, and the decompiled C
of the ones left was read. 2,636 of the 2,648 German functions have a
partner (the 12 without are RTL helpers, the empty `pauseform_form_close`
and the old `load_highscores`/`save_highscores`, whose work the later
build's en 1068:1adf and en 1068:1e08 took over); 160 of the pairs
differ in more than layout, and in 97 of those the decompiled C
differs, many only by the strings or the form layouts they use. The
tables below are what's left: every change in the game's code.

The VCL and RTL segments (en 27–35) are the same Delphi 1 libraries;
a handful of routines in `SysUtils` (en 34) differ, which the port
doesn't use.

## Where the later build reads its files

`init_paths` (en 1000:196d) sets `upgrade_dir` (en `1118:5404`) to
`Upgrade\`, relative like `Maps\`, `Sound\` and `Musik\` (on the hard
disk), not on the CD like `Data\` and `Texte\`. From there it reads:

| File | Read by | Instead of |
|---|---|---|
| `WELTWUND`, `CAMPER`, `BBONLY`, `PRWUNDER`, `HURRYUP`, `CHALLENG.HIS` | state 1500 (en 1010:0591) | (new: scenarios 8–13) |
| `EREIGN7.DAT`–`EREIGN12.DAT` | state 1500 | (new) |
| `SZENARIO.TXT` | `startform_load_texts` (en 1058:2c38) | `TEXTE\SZENARIO.TXT` |
| `SONSTIGE.TXT` (25 groups, `Löschen` added) | `load_sonstige` (en 1010:3e42) | `TEXTE\SONSTIGE.TXT` (24) |
| `SCHLUSS.TXT` | `schluss_load_texts` (en 1070:0294) | `TEXTE\SCHLUSS.TXT` |
| `HAUSPREI.DAT` | `load_hauspreis` (en 1048:341c) | `DATA\HAUSPREI.DAT` |
| `CREDITS.BMP` | `creditform_show` (en 1038:3925) | `GRAFIK\CREDITS.BMP` |
| `DATWIN1D.BMP` | `dateimenue_formshow` (en 1040:159e) | `GRAFIK\datwin1c.BMP` |
| `STARTSZ1.BMP` | `startform_mode_click` (en 1058:02cf) | `GRAFIK\startsze.bmp` |
| `STRAKN2.BMP`, `STRAKN2P.BMP` | `startform_init` (en 1058:378f), `highscoreform_init` (en 1068:29ac) | (new check boxes) |
| `HIGHSCO2.BMP` | `highscoreform_init` | `GRAFIK\highscor.bmp` |
| `HIGHSCO2.TXT` (the six new names) | `load_highscor_txt` (en 1068:2493) | (new; `TEXTE\HIGHSCOR.TXT` is still read) |
| `HIGHSCO2.WAV` (high scores of 8–13) | en 1068:1adf, en 1068:1e08 | (new; `SOUND\HIGHSCOR.WAV` keeps 0–6) |

The speech folders are `Sprache1\`–`Sprache3\`, otherwise used the same.

## Code changes

Kinds: **fix** (the later build fixes a bug of v1.0), **feature** (the
new scenarios and what goes with them), **balance**, **other**.

| Kind | Where (de / en) | Change |
|---|---|---|
| feature | 1000:17bc / en 1000:196d `init_paths` | `Upgrade\`; `sprache1\`–`3\`; six more films in the CD check |
| feature | 1000:2466 / en 1010:0591 `hintergrund_timer`, state 1500 | Scenarios 8–13: map from `Upgrade\`, events `Upgrade\ereign7..12.dat`, computer players 4, 3, 7, 4, 0, 5; scenarios 8, 12, 13 take `SCENARIO_BANNED` off the build menu, 11 only 0x6f, 0x71, 0x79 (en 1010:01d5), 12 also sets the build menu's scroll row of submenu 5 to 0 (en 1018:015f); 10 sets `computer_types[0x19a]` to 7 (en 10b0:0002). The computers find themselves (`computers_find_themselves`) only in scenarios 1–8, 11 and 13 (v1.0: always, which in a normal game happens elsewhere anyway) |
| other | 1000:2466 / en 1010:0591, en 1010:02b5 | The intro's states in another order: 0x6e (layout, `sound_deactivate`) → 300 → 400 (`sunflow.avi`) → 0x1a4 → 100 (the forms' bitmaps) → 0x1a6 (`holiday.mid`); `text_done`/`text_init` gone from 0x1a4. A new `hintergrund_restart` (en 1010:02b5) starts state 0x6e again when `1118:1320` is set. Same behaviour on the screen |
| feature | 1008:0297 / en 1008:0555 `scenario_won` | 8: exactly one plot of more than 41 squares (en 1008:0317: the islands joined); 9: player 1's small camp sites (type 0) have 5000 guests or more (en 1008:0134); 10: year 2021; 11: more than 8 of player 1's facilities of the reputation types (`1110:1458`: 29, 30, 35, 37, 60, 108, 110, 125, 126) are 80 % full or more (en 1008:036e); 12: year 2001; 13: on every plot player 1's houses have more than half the plot's guests (`+0xee` div 2; en 1008:03fc) |
| feature | 1008:042b / en 1008:075f `scenario_lost` | 8: 2016, 9: 2011, 10: bankrupt, 11: 2026, 12: bankrupt, 13: 2026 |
| feature | 1018:394e / en 1018:363c `button_at_mouse`; 1018:3904 / en 1018:362e; 1018:3290 / en 1018:32a5 `baumenue_formkeydown`; 1020:2e09 / en 1020:2e09 | Scenario 10 (B&B only): the button bar's button 10 (F2, the hotels) can't be pressed, and the build menu's Left/Right skip submenu 10 |
| feature | 1040:1853–1040:3347 / en 1068:0109–en 1068:34f4 | High scores for 14 games (`0x3aa` a rank, v1.0 `0x1d5`): 0–6 in `SOUND\HIGHSCOR.WAV` as before, 8–13 in `Upgrade\HIGHSCO2.WAV` slots 1–6 (slot 0 a copy of game 0's); game 7 (the tutorial) has a table in memory that's never saved. Six more check boxes (en 1068:1213–en 1068:1952) |
| feature | 1058:02cf…1058:41fb / en 1058:02cf…en 1058:547f | The start menu's scenario page has twelve scenarios (six new boxes, en 1058:1ead–en 1058:25d8 set `scenario` 8–13), `startsz1.bmp`, `strakn2.bmp` |
| feature | 1068:0288 / en 1070:0294 `schluss_load_texts`; 1068:1016 / en 1070:128d `schlussform_show_end` | Texts for 14 games; the films `ABFries`, `ABCamper`, `ABBBonly`, `ABPRWund`, `ABHurry`, `ABChalle.avi` for won scenarios 8–13 |
| feature | 1040:073c, 1040:0a4c … / en 1040:077e, en 1040:0ac7, en 1040:110c | The file menu has a `Löschen` button (part 0x10, rect en `1118:18ac`): after a question (`Wirklich loeschen?` / `Vous êtes sûr?` / `Are you sure?` by `land`) it erases the selected `MAPS\*.map` and fills the list again. The parts after it move up by two |
| feature | 1020:3dc4 / en 1010:3e42 | `SONSTIGE.TXT` 25 groups (buffer 0x1900), the 25th the file menu's `Löschen` |
| fix | 1028:26e4 / en 1028:3620 `auktionsform_formkeydown` | Enter no longer bids (nor starts a waiting auction): only Escape/Space and Alt+F4 (pass) are left. Fixes 1028:26e4 (Enter bids without checking the money, after the third call too) |
| fix | 1080:202b / en 1088:2040 `umenu_formkeydown` | Up and Down keep the list's top at 0 or more (fixes the read before the list, 1080:202b); Left and Right put the list's top and the selection back to 0 for the new kind (v1.0 keeps both, so the selection can be past the new list's end) |
| fix | 1080:1c3b / en 1088:1c3b `umenu_bild_mousedown` | A click on another kind's picture puts the selection back to 0; the pictures end at x 282 (v1.0: 283, whose kind is 12, past the last) |
| other | 1020:2e09 / en 1020:2e09 `hauptschirm_formkeydown` | The debug keys Ctrl+Alt+Shift+N and +M (the clock's interval down or up by 10 ms) are gone; Ctrl+Alt+Shift+G (money) is still there |
| feature | en 1010:2511 (new) | The start menu's twelve scenario boxes (VMTs en `1118:1da2`..) draw their text 5 pixels right of the box (the other check boxes and radio buttons: 16), in the checked colour when ticked. v1.0's `ui_radio_draw2` (1010:16b5) is still there as en 1010:25dc |
| other | 1010:25dc / en 1010:3582 `back_to_start_menu` | Waits for the CD (`surfwm.avi`) with a retry question before the start menu; leaves the background form and its picture disabled |
| other | 1040:0e03 / en 1040:0eef `dateimenue_save` | The invalid-name message in three languages by `land` |
| balance | `HAUSPREI.DAT` (data) | `Upgrade\HAUSPREI.DAT`: the pyramid's (row 126) price from 40000 to 30000. Nothing else in the tables changed |

Everything else, the whole simulation included (settlement, people,
computers, events, sabotage, the generator, the TV, the auction's
logic), is the same code.

## Bugs of v1.0 the later build fixes

Each row of [original-bugs.md](original-bugs.md) was checked against the
later build: a row whose function is unchanged keeps its bug. Fixed:

| Row | en | How |
|---|---|---|
| 1028:26e4: Enter bids without checking the money | en 1028:3620 | Enter does nothing |
| 1080:202b: the overview's key Up with fewer than 12 buildings moves the top before the list | en 1088:2040 | The top is at least 0 |
| 1040:1bdf: a won game of scenario 7 writes past the high score table | en 1068:0495 | The table has 14 games; game 7's stays in memory |
| 1040:1d49: showing the high scores of a game past 6 reads past the table | en 1068:05ff | Same: 14 games (past 13 still reads past) |

Not in the list before, fixed by the later build: the overview's Left
and Right (and its kind buttons) keep the list's top and the selection
of the kind before (en 1088:2040, en 1088:1c3b).

Still there in the later build: every other row, among them the
scenario page's tick (1058:02cf), the end form's missing film for a won
scenario 7 (now 7 and past 13, en 1070:128d), the unset results of
`scenario_won` past 13, the auctioneer's calls (en 1040:34f9), the
files menu's focus (en 1040:077e) and all the simulation's.

## Data

Apart from the translated texts (`TEXTE\`, speech, the films' sound
tracks), `orig-en/` and `orig/` differ in:

- `DATA\BAUMENU.DAT`, `GELDMENU.DAT`, `INFOBOX.DAT`: positions and
  alignments of labels and values only (columns swapped and moved for
  the English words, `left`/`right`). Layout for the translation, no
  numbers that play.
- `DATA\MODATYP.DAT`: `land` `D` → `E`; every number the same.
- `SZENARIO\URLAUBER.HIS`: one byte (0xa1448), a beach life square of
  one house (`*1110:6f10` record `+0x27`), 0xaa → 0xea.
- `UPGRADE\HAUSPREI.DAT`: the pyramid's price (above). `DATA\HAUSPREI.DAT`
  is the same as v1.0's.
- `GRAFIK\CREDITS.BMP` (English credits) and `UPGRADE\CREDITS.BMP`;
  `SOUND\HUPE3.WAV`; `SOUNDLO\HIGHSCOR.WAV` (a high score table);
  `MAPS\LAST.MAP` (a saved game); `MUSIK\FLOATIN.MID` isn't in the
  English folder (it's in the German installer's archive only).

The English CD's own installer archive (`_files.1`, `SUNFLOWERS
INSTALLATION FILE`) is another format than the German `_FILES.1` and
hasn't been looked at.

## The port

The port takes the later build's fixes unconditionally, reads
`UPGRADE\` when it's there and plays its scenarios, and without it
behaves as v1.0 with v1.0's files. Language (English texts and speech,
£) is a separate step. What's ported and what isn't yet is in the
modules' docs; [original-bugs.md](original-bugs.md) marks the rows the
later build fixed.
