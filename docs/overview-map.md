# The overview map and the compass

Two forms the main screen opens: the overview map (`TUebersichtsmap2`,
segment `10b8`), ported in
[`game::uebersichtsmap`](../crates/game/src/uebersichtsmap.rs), and the
compass (`TKompassform`, segment `1038`), ported in
[`game::kompassform`](../crates/game/src/kompassform.rs). Both are the
view's; neither changes the world, except that the overview map's scroll
arrows switch the "new people" option.

## The overview map (`TUebersichtsmap2`)

The button bar's button 5 (or `6`, `U`, `Ü`) opens it when it goes down
(10b8:4079) and closes it when it comes up (10b8:3f7f), through
`button_bar_click` (1010:2c8b). `load_game_rest` creates it for each game
(10b8:4538), hidden at the screen's top left; `game_start_setup` and
`game_show_state` lay it out big and draw it (10b8:3fab).

### Layout

`UEBERMAP.DAT` has the six pressed buttons' places in `uemenpre.BMP`
(`PressedLage`) and two layouts (big and small, `+0x213`) of 22
rectangles (`Lage`), each left, top, right, bottom inclusive. 10b8:3553
puts the five images where the layout says:

| Part | Big | Small | What |
|---|---|---|---|
| 0 | 421 × 348 | 290 × 262 | The form |
| 1 | 0,0 405 × 16 | 0,0 274 × 16 | `Ueberschriftbild`, the title bar: `uebigueb.BMP`/`uesmaueb.BMP` with `UEBERMAP.TXT`'s title centred in colour 9 |
| 2 | 0,0 16 × 16 | 0,0 | In the title bar: close (`closepre.bmp` held down) |
| 3 | 389,0 | 258,0 | In the title bar: the size button (`uesmapre.BMP` or `uebigpre.BMP` held down) |
| 4 | 405,0 16 × 16 | 274,0 | `Umschaltbild`: heights or slopes (`ueschbut.BMP`, `uehoebut.BMP`, held `ueschpre`, `uehoepre`) |
| 5 | 405,16 16 × 275 | 274,16 16 × 189 | `NSScrollbild`, the vertical scroll bar (`uebigns.BMP`, `uesmans.BMP`); 6–8 its up arrow, trough, down arrow |
| 9 | 0,16 405 × 275 | 0,16 274 × 189 | `MapBild` |
| 10 | 0,291 421 × 57 | 0,205 290 × 57 | `Menubild` (`uebigmen.BMP`, `uesmamen.BMP`); 11–14 the horizontal scroll bar, its left arrow, trough and right arrow |
| 15–20 | 74,20 … | 8,20 … | In the menu: the five colourings' buttons, 32 × 32 |
| 21 | 254,20 95 × 32 | 188,20 | "Schließen" (`SONSTIGE.TXT`'s 14th text, centred 8 below the top) |

The form is as wide as part 0, as high while shown, 0 high while hidden
and 16 while its title bar is dragged. The scroll arrows held down are the
game's `scrbutno`, `scrbutsu`, `scrbutwe`, `scrbutos.BMP`
(`1110:6022`..`602e`, 1030:307a), the thumbs `scroll1.BMP`'s.

### The map

The whole map is a picture of 1002 × 502 (`0x3ea`, `0x1f6`) that's never
drawn whole: `MapBild` is a window onto it from `+0x1b0`, `+0x1b2`, the
scroll bars' positions. A square `(i, j)` is at `x = 2 (i - j + 250)`,
`y = i + j`, so the main screen's scroll position `wo`, `ns` is at
`(2 wo, ns)`. Drawing an area (10b8:0eeb) first tiles it with
`Ueberma2.BMP`'s 48 × 38 sea, then walks it two pixels at a time, from
two above to two below and one left to one right, works out the square
at each point (10b8:038d: `half = px div 2`,
`i = (py + half + 750) div 2 - 500`, `j = (py - half + 1250) div 2 - 500`)
and, unless it's off the map or water (ground 2), copies a two by two
block of its colour from `Ueberma2.BMP` at
`(x - px mod 2, y - (py - half + 250) mod 2 - 2)`. The colour's column
and row come from two functions per colouring and shading (10b8:17cf):

| Colouring (`+0x199`) | Button | Column | Row |
|---|---|---|---|
| 0 owners | 16 (`1`, F1) | 0x30, 0x32 for free grass (heights) / 0x36 grass, 0x34 sand (slopes) | roads 0x18, rivers 0x1a, a building `(colour[owner] - 1) * 2`, else the land |
| 1 buildings | 17 | 0x38 for a building, else as above | a building `HAUSDATA` column 5 × 2 + 2, else as above |
| 2 location | 18 | 0x3a | `36 - location * 36 / 255` |
| 3 green | 19 | 0x38 | palms 0x1e, parks (kind 6) 0x20, other buildings 0x22, roads 0x22, rivers 0x20, else 0x24 |
| 4 occupancy | 20 | 0x3c | a building `32 - occupancy * 32 / 100`, else 0x24 |
| 5 the land | none down | grass 0x32 / 0x36, sand 0x30 / 0x34, rivers 0x30 | rivers 0x1a, else the land |

"The land" is, shaded by heights (`+0x19f` = 1, the toggle's default),
`min(36, (h - 3) * 2)` for grass and `h * 2 + 26` for sand, `h` the
square's corner height; shaded by slopes its form × 2. Grass below
height 3, sand from height 6 and the buildings of nobody (owner 0, whose
colour `1110:0073` is never written) get rows outside the bitmap, which
the port draws nothing for; with `enhance.overview_colours` they get the
lowest grass's row, the highest sand's (0x24) and the land's. "Free" squares
have no building, a palm or beach life (`1110:1358`); "buildings" are all
but those and the bridges' 11 and 52 (`1110:1378`); roads are grounds
10–58 and 95–100, rivers 59–94 (55–94 for the land and green). Pressing
the button that's down shows the land again (10b8:303e); each change of
colouring or shading draws the whole window.

### The frame

The main screen's view is a frame from `UeRecht.BMP`, its mask `SRCAND`
and picture `SRCINVERT`, at `(2 wo - 1, ns)` less the window's origin and
an offset for the rotation (`1110:3d80`: 40 × 40 for rotations 0 and 2,
55 × 25 for 1 and 3). Every scroll and turn of the main screen takes it off
(10b8:1485, drawing the area under it again) and puts it back (10b8:1349).
The port keeps `MapBild` without the frame and adds it on top.

The frame's box is 40 × 25 in every rotation, which is what the main
screen's 20 columns and 27 half rows cover of the map turned 0 or 2;
turned a quarter (1 or 3) they cover about 50 × 20 of it, so there the
box is too narrow and too high ([original-bugs.md](original-bugs.md#ui-quirks)).
With the port's `big_map` and a bigger main view the frame is drawn
instead as a box around what that view covers, with `UeRecht.BMP`'s
mark for the screen's bottom edge beside it
([layout.md](layout.md#big_map-enhancement)). With `enhance.overview_frame`
the original's view turned a quarter gets such a box too, around the
squares its corners show (about 50 × 20; `Hauptschirm::overview_frame`);
turned 0 or 2 it keeps the original's picture, which fits.

### The mouse and keys

- `MapBild`: exactly `[ssLeft]` puts the main screen's scroll position at
  the point less the frame's click offset (`1110:3d90`: 20, 12 for
  rotation 0): `wo = (x - 20 + origin.x) div 2`, `ns = y - 12 + origin.y`,
  unclamped (with `enhance.overview_scrolling` kept within the main
  screen's scroll bars' ends), then `hauptschirm_update_scroll` and the
  island drawn (10b8:37f5); dragging with it does the same inside the image (10b8:39b1),
  and letting go of the left button draws the island again (10b8:3aab).
  Exactly `[ssRight]` centres the window on the point (10b8:1a6e: what
  stays in view is copied, the strips that come in drawn).
- The scroll bars: the arrows step 5 and the troughs page 30 at once and
  every 25 ms while held (`NSScrolltimer`, `WOScrolltimer`, 10b8:216e,
  10b8:28e5, by their `Tag`s 1–4), clearing the "new people" option
  (`1110:0056`) while the timer runs and setting it when the mouse is let
  go (10b8:21c7, 10b8:21fa). The thumbs are dragged. The thumb is
  `16 + (pos - min) (travel - 16) / range`, `travel` the vertical bar's
  height less 35 and the horizontal trough's width (which lets it onto the
  right arrow; with `enhance.overview_scrolling` 4 less). The horizontal
  bar (in `Menubild`, 10b8:29a2) takes only the left button, but the
  vertical one (`NSScrollbild`, 10b8:222b, en 10c0:222b) doesn't look at
  the button: the right or middle one steps, pages and grabs it too, with
  sound 9 and the timer that clears `1110:0056` (unless the world has
  `fix.overview_arrows`), though only the left drags the thumb
  ([original-bugs.md](original-bugs.md#ui-quirks)); with
  `enhance.mouse_buttons` only the left button
  (`Uebersichtsmap::scroll_mouse_down`).
- `Menubild`'s colourings' buttons pick on the mouse up over the button
  (sound 9 on the press); "Schließen" goes down (sound 8) and closes
  (sound 10, then 0xe).
- The title bar: any button grabs it (the form shrinks to 16 rows while
  dragged, and stays so after a double click until the next press:
  `FUN_10f0_2e58(form, 0x10)` is the form's `ClientHeight` setter, which
  stores the value at +0x120 while the form is loading (`csLoading`) and
  otherwise calls 10e8:1919; the forms have no border, so their
  `ClientHeight` is their `Height`); the
  left button holds close or the size button, which switches big and small
  (10b8:3e73: hidden, laid out, drawn, shown).
- `Umschaltbild`: the left button switches heights and slopes.
- Keys (10b8:4c95), without Ctrl or Alt: `1`–`5` and F1–F5 the
  colourings, the arrows and the number pad scroll the window by 20, Enter
  and Escape close, Space gives the main screen the focus, anything else
  goes to the main screen's `FormKeyDown`; Alt+F4 closes.
- Tooltips: the main screen's clock (1020:1fe2) shows the help texts
  `1110:1636` (0x18–0x1c) for the colourings' buttons under the mouse,
  while the form is active and its title bar wasn't double clicked
  (`Uebersichtsmap::help_text_at`, [endforms.md](endforms.md#thelpform-the-tooltips-10403d0210403ead)).

### Redraws the world asks for

The world's routines redraw the squares they change, for the colourings
that show them: 10b8:1504 (raising and lowering, all six), 10b8:1534
(occupancy, its colouring), 10b8:1564 and 10b8:1594 (clearing and
demolishing), 10b8:15f4 (roads), 10b8:1624 (palms, beach life, transfers,
sites, replacements), 10b8:1654 (buildings; colourings 1–4) and 10b8:15c4
(location changes, its colouring). The areas grow by `MaxLageeinfluss`
for the location colouring and `MaxGrueneinfluss` (its low byte) for the
green one. The port's `show_tick` redraws the area around all of
`Tick::redraw`'s squares once, the occupancy changes of `Tick::occupancy`,
and, in the location colouring, the location map's cells that differ from
what it last drew (the world's `change_location` gets no tick). A
loaded game (`Tick::loaded`) is laid out and drawn again.

### Saved

`save_map` writes the form's fields (10b8:4e34, `[1110:723a]+0x198`..):
the button down, the colouring, the part pressed, the window's size, the
shading, the size, the flags, the window's origin, the grabs, the bars, the
last right click's centre. The port's `save` and `load` keep them all.
`load_map` (1048:2b8f) is the only caller of the loader (10b8:50de), so
a generated island's form stays as its constructor left it: no button
down, the land alone, by heights.

## The compass (`TKompassform`)

A 60 × 76 form (1038:2c04) at the screen's top right, `fsStayOnTop`, one
`TImage` with `komp.bmp` and four radio buttons (20 × 20, `kompsu`,
`kompos`, `kompno`, `kompwe.bmp` and their `pr` twins) for the views 0
(bottom), 3 (right), 2 (top) and 1 (left), and a close button at its top
left. The main screen's corner knob (`eckimage`, a radio button with
`kompkn.bmp`/`kompknpr.bmp` that's toggled like a check box, 1020:3214)
shows it (1038:314f: the button of the current view on) and hides it
(1038:3243). A button turns the view (`set_view`, 1010:254d, then the
others off: 1038:29e3, 1038:2a41, 1038:2a9f, 1038:2afd); close hides it
and lets the knob up (1038:2b5b). Turning with Insert and Delete updates
its buttons (1038:3256, from `hauptschirm_key_turn`), as does showing a
game while it's open. The title row right of the close button drags it
like the overview map's; its keys go to the main screen (1038:3557).
