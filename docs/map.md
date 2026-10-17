# The island and how it's drawn

The map model is ported in [`game::world`](../crates/game/src/world.rs),
the drawing (segment `1050`) in
[`game::darstellung`](../crates/game/src/darstellung.rs) and the main
screen around it (`THauptschirm`, segment `1020`) in
[`game::hauptschirm`](../crates/game/src/hauptschirm.rs).
A normal game's island is generated ([island-generator.md](island-generator.md)).

## The map

The island is nine layers of 251 × 251 bytes, each a heap block behind a
far pointer at `1110:5500` to `1110:5520`, indexed `[i * 251 + j]`. Map
files store them in that order (`load_map`'s first unit routine, 1000:0c6a;
[savegame.md](savegame.md)).

| Layer | What |
|---|---|
| `5500` | Height of each square's corner, in steps of 9 pixels. 0 is the sea. The square at `(i, j)` has the corners `(i, j)`, `(i-1, j)`, `(i-1, j-1)`, `(i, j-1)` |
| `5504` | The ground. Below 10 a kind of ground: 0 sand, 1 grass, 2 and 3 water, 4 and 5 sand to grass, 9 a white wire grid (`GROUNDNR.DAT`'s groups, 19 rows each). 10–54 a road piece (`STRASPOS.DAT`), 55–94 a river piece (`FLUSSPOS.DAT`) |
| `5508` | The building type (`HAUSNR.DAT` row), 255 for none. 12 is a palm |
| `550c` | The form (slope), 0–18. 16 is flat |
| `5510` | Which part of a bigger building the square is, or the ground's variant |
| `5514` | The building's owner, 1–8 |
| `5518` | How full the building is, in percent |
| `551c`, `5520` | Not known yet |

`1030:1fdb` and `1030:208d` give the highest and lowest of a square's four
corners (0 within one square of the edge); buildings stand on the highest.

## The view

The view is isometric. A square is a diamond 62 pixels wide and 32 high;
`Hauptbild` shows 20 columns of 32 pixels and 27 half rows of 16
(`1110:12bc`/`12be`: 624×405, one pixel more than the image). The view's
state is all in globals, and a map file saves it:

- `1110:007c`: the rotation, 0–3. Insert and Delete turn it
  (1010:254d), keeping the origin.
- `1110:5530`, `5532`: the map position the corrections below start from.
  `1110:5534`: whether the columns start with the other half.
- `THauptschirm+0x408`, `+0x40a`: the scroll position `wo` and `ns`, which
  the two scroll bars show. `wo = i + 250 - j`, `ns = i + j` (1020:074d), and
  back (1020:0697): `i = (wo + ns + 1001) div 2 - 626`,
  `j = (ns + 1000 - wo) div 2 - 375`, parity `(wo + ns + 250) mod 2`. So a
  round trip moves `i` back by one when `wo + ns` is even; after turning the
  view, 1020:06f1 is used once instead, which doesn't. `ns` runs from 0 to
  476, `wo` from 0 to 481; for the other rotations the bars swap and run
  backwards (1020:05a7).
- Scrolling (1020:07dc) adds to `wo` and `ns` (turned by the rotation),
  clamps them to the bars' ranges and recomputes the origin. One `ns` step is
  16 pixels, one `wo` step a column (32). The arrow keys and the number pad
  move 2 steps, 6 with Ctrl (1020:2e09). The original then scrolls the
  picture with `BitBlt` and draws only the strip that came into view
  (1050:0144); diagonal or long moves redraw everything.
- `1110:0058`/`0059`: buildings show a pointer with their owner's colour, or
  else (with `0059`) one for how full they are.
- `1110:0086`: a flood is going on (an event, [simulation.md](simulation.md#events-10103084)).
  The drawing then puts everything a step lower and turns squares at sea
  level into water without buildings (palms stay). It's world state
  (`World::flood`), not the view's.

## Drawing (1050:0e68)

`draw_columns(first, last, rows)` draws columns `first..last`, all half
rows when `rows` is 0, the bottom `rows` or the top `-rows` otherwise
(for scrolling). 1050:2700 draws everything.

1. The band is filled with black (`BLACKNESS`).
2. For each column, the start on the map comes from `DARSTELL.DAT`:
   `i = spalten_x[view][parity][column] + origin.i + zeilen_x[view][parity][row]`,
   likewise `j`, where `parity` alternates per column starting from
   `(first mod 2 + 1110:5534) mod 2`. The column starts one half row above
   the top. Each step down adds 16 pixels and `extratab_x/y[view][half]` to
   the position, and switches `half`, the half of the diamonds it draws
   (left 32 pixels wide, right 30; `1110:1a8a`).
3. Off the map it draws the sea (`GFXPOS.DAT` row 54). On the map:
   - The ground tile, unless a building covers it (`HAUSPOS.DAT` column 10
     is 0). Ground: `GROUNDNR.DAT` row `1110:1202[ground] + form`, whose
     second column plus the variant is the `GROUNPOS.DAT` row; roads and
     rivers go through `DARSTELL.DAT`'s `Strassenkorrektur` and
     `Flusskorrektur` for the rotation. It's lifted by the height of the
     corner the rotation measures from (1050:075e) times 9 plus
     `1110:00ac[form]`.
   - Under a building on a slope (form not 16, `HAUSPOS` column 9 = 1), the
     foundation from `gfxhaus1.BMP`; the next ground tile then draws its mask
     first so the foundation doesn't show through.
   - The building's part: the type's first `HAUSPOS` row (`HAUSNR` column 2)
     plus the part, turned for the rotation (1050:0997, 1050:0811; some
     types have turned twins, `HAUSNR` column 1, or swap within groups of
     four), from the bitmap `HAUSNR` column 7 picks.
   - The pointer over it, from `DARSTELL.DAT`'s pointer positions.
4. It stops when a step draws nothing (the ground is below the bottom), and
   then looks at 15 more squares for buildings tall enough to reach up into
   the view.

Ground, roads and rivers are `SRCPAINT`ed onto the black, mostly without
their masks; buildings and the foundation use mask (`SRCAND`) and picture
(`SRCINVERT` or `SRCPAINT`). All of it is on palette indices; the bitmaps
share one palette whose index 0 is black and 255 white.

Rows of `HAUSPOS.DAT` (and the pointer positions): height, width, picture
x, y, mask x, y, rise above the corner, left offset in the left half's
column, pointer kind, foundation, ground shows, unused. Rows of
`GROUNPOS.DAT`, `STRASPOS.DAT`, `FLUSSPOS.DAT`, `GFXPOS.DAT`: height,
picture x, y, unused, mask x, y.

The bitmaps (`1110:5fc0`, word array of device contexts, set by 1030:1418):
0 `gfxPalm2`, 1 `gfxhaus1`, 2 `gfxhaus6`, 3 `granim2`, 4 `gfxhaus5`, 5
`auto`, 7 `gfxgroun`, 9 `gfxhaus9`, 11 `groundan`, 15 `gfxflug`.

## The main screen

`THauptschirm` is 640×479, centred on the screen (1020:1ce9), with its images
laid out from the table at `1110:1572`:

| Image | Bounds | Shows |
|---|---|---|
| `Ueberschriftbild` | 16,0 608×16 | `menulei3.bmp` with `SONSTIGE.TXT`'s first text centred |
| `CloseBild`, `Kleinbild` | 0,0 and 624,0, 16×16 | The close and minimize buttons |
| `InfoImage` | 0,16 640×18 | `statusle.bmp` |
| `ButtonImage` | 0,34 640×25 | 23 buttons, 25×25, from `buttoff.bmp` (up) or `button.bmp` (down) |
| `Hauptbild` | 0,59 624×404 | The island |
| `NSScrollbild` | 624,59 16×404 | `scronosu.BMP` with the thumb from `scroll1.BMP` |
| `WOScrollbild` | 219,463 405×16 | `scroweos.BMP` with the thumb |
| `Statusimage` | 0,463 219×16 | `infolei.bmp` |
| `eckimage` | 624,463 16×16 | The compass knob, `kompkn.bmp` |

The thumbs sit at `16 + (pos - min) * (length - 51) div (max - min)`.

The `AnimationsTimer` drives the simulation: see
[simulation.md](simulation.md). The info bar shows the money (left) and
the date (right). Not ported yet: the timer's view parts (scrolling while
the mouse is held at the scroll bars, the people's sprites), the buttons,
the status bar's messages, mouse input, the overview map.
