# The info box and the buildings' overview

The pointer tool's info box (`TInfobox2`, segment `1038`, `infobox2` at
`1110:65fc`) and the overview of the human's buildings it opens
(`TUmenuform`, segment `1080`, `umenuform` at `1110:6fb4`). Ported in
[`game::infobox`](../crates/game/src/infobox.rs) and
[`game::umenuform`](../crates/game/src/umenuform.rs); the pointer's
presses in [`game::tools`](../crates/game/src/tools.rs).

## Opening and closing (1020:1984, 1020:1b31)

With the pointer (tool 22), `HauptbildMouseDown` with exactly `[ssLeft]`
clears `1110:64b8` and shows the box (`show_info_box`, 1020:04bc), and
with `[ssLeft, ssDouble]` sets it and shows the box. `HauptbildMouseUp`
(any button) hides it (1020:050b) unless `1110:64b8` is set. So a press
shows the box until the button comes up, and a double click shows it
with buttons, open until one of them, Enter or Escape closes it: the
first press of a double click shows and hides it, the second shows it
for good.

`show_info_box` takes the square under the mouse (`cursor_square`) and
the mouse in the main screen's form, and calls 1038:1eac: the box shows
the square itself (`+0x180` cleared) and, with `1110:64b8` set,
`disable_forms(7)` disables every other form (the main screen too) until
it closes. 1038:1ccf sets `+0x1e6` (shown), the square (`+0x1e2`,
`+0x1e4`), draws the box (1038:1b92) and sizes it to what it drew; it
goes 10 pixels right of and below the mouse, or 10 left of or above it
when `x + 20 + width` (`y + 20 + height`) doesn't fit the main screen's
client area. Then it takes the focus.

Hiding (1038:1ee0) sets the height to 0 and clears `+0x1e6`; with
`1110:64b8` set it enables the forms again (1010:29cc), and the main
screen takes the focus. `1110:64b8` stays set after the box closes, until
the next press. The clock's step 1 (1010:2a0c) doesn't enable the forms
while `+0x1e6` is set. Others close the box too: a building site under it
that finishes (1008:525b), the auction form opening (1028:13e2), the TV
(1048:1556), the money menu (1070:2b3a); the end of a game frees it
(1030:3974, 1038:2302) and the start of one sets it up again
(1030:37e1, 1038:1f69).

## What it shows (1038:1899, 1038:1a75, 1038:014b)

`INFOBOX.DAT` (read by 1038:0e13, `formats::data::infobox_dat`) has, for
each kind of square (`TIBArt`: hotel, house, restaurant, shop, sport,
attraction, nature, rental, airport, harbour, energy, road, ground,
building site, island), which of the 17 lines it shows, each line's
height, its three texts' columns and alignments, the box's largest size,
the eight rows of buttons and the nine buttons. `INFOBOX.TXT` has the
lines' texts before and after the value, ten descriptions of the
ground, the button rows' texts, the buttons' captions and ten short
texts for the facilities' prices by their class.

The kind (1038:1899):

- Off the map: the ground (12), on water at height 0.
- With "Insel-Info" pressed and a plot on the square: the island (14),
  the plot's record.
- A palm, beach life, a bridge or nothing (`1110:1378`): a road (11) for
  the grounds 10–0x3a and 0x5f–100 (roads, bridges, rivers), else the
  ground (12).
- Any other building: by its `HAUSDATA.DAT` kind from the table at
  `1110:177c` (`array[-1..11]`: the building site is kind -1). A
  building site's record (1078:1995), or the building's figures
  (1078:17a0: condition, takings, rent or price, guests or how full it
  is) and its record (1078:169a).

The lines (`TIBZeilen`):

| Line | What |
|---|---|
| 0 | The long name (`HAUSNAME.TXT`'s second list); for kinds 0–2 (houses, hotels, restaurants) with `"   "`, the record's stars and `"_*"` |
| 1 | A building site's type's long name; else the ground: ocean (water at height 0), shore (water), grass, sand, grass and sand, a road on sand, on grass, on both, a bridge, a river; nothing for other grounds |
| 2 | "Baustelle" |
| 3 | The owner's name (`1110:0902`) |
| 4 | The takings a month: guests × rent, or how full × price / 100 |
| 5 | The running costs, `HAUSPREI.DAT` column 1 |
| 6 | Guests / capacity (`HAUSDATA.DAT` column 3) |
| 7 | How full × capacity / 100 / capacity |
| 8 | The rent a week: the rent / 4 |
| 9 | The price per visit, `price × 100 / 30 / capacity`, with two decimals (1030:13da); its text is the short text of the facility's class |
| 10 | The condition in percent: × 100 / 127 |
| 11 | "in *days* Tagen" (`SONSTIGE.TXT` 0x13, 0x14) |
| 12 | The height: the highest corner × 3 |
| 13 | "INSEL-INFORMATION" |
| 14–16 | The plot's quality (`+4`), holidaymakers (`+0xee`) and size (`+0x352`) |

Money goes through `format_money` (1030:11a0). The values that aren't
looked up for the square keep the last square's: off the map the
building isn't, so a building site shown before names the ocean's line.

1038:1b92 draws `Infobox.bmp` over the whole largest box, the lines from
row 3, then 2 rows more, and the frame's right edge (4 columns from
x 0x11b) and bottom edge (4 rows from y 0xcb) where the box ends; the
box is as wide as the widest line or button row (column 8 of
`ZeilenLage`).

## The buttons (1038:0d3a, 1038:2742, 1038:27b4, 1038:24d6)

Only with `1110:64b8` set: on the map, on a square of the human or
nobody (owner below 2; only the human's with `fix.nobodys_buildings`)
that isn't water, the button rows of the kind
(`WelcheButtonzeile`); else row 7, just OK. A row draws its buttons
(`WelcheButton`) at its height, its text and its value: row 1 the rent a
week, row 2 the price per visit (with the class's short text for a
text), each with the currency.

| Button | Does |
|---|---|
| 0 Renovieren | 1010:1931: the renovation price (1008:0660) if the player has it (else error 0xe for player 1): paid, booked as "other" and rising for player 1; the condition is 127 again |
| 1, 2 (minus, plus) | The house's rent a step down or up (1010:1e05, 1010:1d62), or the facility's price (1010:1c86, 1010:1ae2) |
| 3 Bestechen | 1010:19f1: for 5000 (else error 0xe) the site's days left become `min(days, max(max(0, (days - t div 4) div 2) + t div 4, days - 10))` for the building time `t` (`HAUSDATA.DAT` column 0) |
| 4 Verkaufen | While event 9 isn't running: event 9 (the sale) certain, with the price (twice `HAUSPREI.DAT` column 2 for a price of 10 or less, else a quarter of the price), the human as the seller and the building's first square |
| 5 Abbruch, 6 OK | Close |
| 7 Insel-Info | `+0x180` set and the box laid out again where it is: the plot's information (1038:1e83) |
| 8 Übersicht | Close, and the overview opens (1080:0ea8) |

A press (any mouse button) holds down the shown buttons under the mouse
with sound 8; letting go lets them up with sound 10, and the one still
under the mouse does its work. Each command draws the box again
(1038:1b92) at the same size. `FormKeyDown` (1038:284b) passes the keys
to the TV while it's shown; else Enter or Escape (alone or with Shift)
and Alt+F4 close the box.

## The overview (`TUmenuform`, 1080:0ea8)

A 320 × 320 form in the middle of the screen, set up afresh each time it
opens (1080:145f: kind 0, the first row, the top of the list): its close
button frees it (1080:02bc, 1080:1b24), Escape only hides it (1080:0e92).
`UMENU.TXT` has the title, the headings, the rent's and the buttons'
texts. Widgets: a scroll bar (299, 0x31, 16 × 0xc5), "Renovieren" and
"Springe hin" (80 × 20 at 8 and 0x60, 0x105), minus and plus (16 × 16 at
0xb6 and 0x10b, 0x109) and the close button.

The kinds (`1110:6fb8`) are the `HAUSDATA.DAT` kinds 0 to 11 but 7,
drawn as the build menu's buttons (from the button bar's pictures) along
the bottom; the selected one down (1080:0bfb, which also draws the
price's heading and starts the list at its top). The list (1080:02d9):
the human's (owner 1) houses (kinds 0, 1) or facilities of the kind, in
the lists' order, up to 0x1f5; twelve rows of 16 pixels, each with the
short name, how full it is (guests × 100 / capacity, or the facility's
percentage) and its condition in percent with a bar from `menuaus.bmp`,
and the profit a month (the takings less the running costs); the
selected row (`1110:6fbe`) in colour 7. Under it the selected one's rent
a week or price per visit.

Mouse (1080:1c3b, only with the left button): a kind's picture selects
it; a row of the list selects it; then the widgets; else the title bar
drags the form (shrunk to it, and kept so after a double click there).
"Renovieren" and the minus and plus buttons act on the selected building
like the info box's, then draw the list; "Springe hin" moves the main
screen's view to ten squares before it (1020:0787). Keys (1080:202b,
alone or with Shift): Escape and Space close, R renovates, Left and
Right go through the kinds (past 7), Up and Down move the selection
(drawing the kind from the top first, then moving the top so the
selection shows), `+` and `-` (also on the number pad) change the
price and draw the kind again, Enter jumps; Alt+F4 closes.
`umenu_refresh` (1080:0123) draws the list again if the form is shown,
after the settlement's steps 0x1d and 0x47 and a finished building
([`Tick::umenu`](../crates/game/src/simulation.rs)).

## World and view

The boxes and the overview are the view's, one human's. Their buttons
are commands for player 1 (`Action::Renovate`, `Price`, `Bribe`,
`Sell`), carried out by `World::renovate_building`, `step_price`,
`bribe_builders` and `sell_building` in `game::infobox`, which check
that the square is the player's or nobody's (the original only offers
the buttons there; with `fix.nobodys_buildings` the player's only). The world's ticks tell the view what it needs:
`Tick::finished_sites` (the box closes over one) and `Tick::umenu`.

Single human: the buttons are for squares of player 1 (or nobody), the
overview lists player 1's buildings, the errors, books and money rising
are player 1's. The box with buttons disables the other forms but
doesn't stop the clock: the simulation never waits for it.
