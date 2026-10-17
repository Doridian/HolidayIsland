# The player's tools

How the human acts on the island: the main screen's button bar, the build
menu (`TBaumenue`), and what a click on the island does with the tool
picked. Ported in [`game::tools`](../crates/game/src/tools.rs) (the
button bar, the clicks, the pictures over the island),
[`game::baumenue`](../crates/game/src/baumenue.rs) (the build menu's
form), [`game::build_menu`](../crates/game/src/build_menu.rs) (the build
menu's lists, which are the world's) and
[`game::command`](../crates/game/src/command.rs) (what the tools do to the
world). The world's routines they call are the ones the computer players
use ([computer.md](computer.md)): `can_build` and `start_building`
([`game::construction`](../crates/game/src/construction.rs)), raising and
lowering the land ([`game::terrain`](../crates/game/src/terrain.rs)),
`build_road` ([`game::roads`](../crates/game/src/roads.rs)) and
`demolish`.

## The tools

The tool (`1110:6056`, saved) is the number of the button that picked it.
The tables at `1110:14e0` and `1110:14f2` that give a tool's button hold
the same numbers.

| Button | Tool | A left click on the island | Port |
|---|---|---|---|
| 0 | | The file menu (`TDateimenue`) | [savegame.md](savegame.md) |
| 1–5 | | The options, ?, the TV, the money menu, the overview map | not ported |
| 6, 7 | Raise, lower | One step of the corner the mouse points at (1028:3b7c, 1028:3a68) | `Action::Level` |
| 8 | Demolish | `demolish` at the square (1028:3c90) | `Action::Demolish` |
| 9–15, 17–20 | Build | The build menu's submenu opens; a click builds its selected type there (1028:3cb1) | `Action::Build` |
| 16 | Road | Press and drag: the road from where the drag started (1080:3195, 1080:31ed) | `Action::RoadDragStart`, `RoadDragEnd` |
| 21 | Sabotage | `TSabowin` and 10a0:1f65 (1028:3d93) | not ported |
| 22 | Pointer | The info box (`TInfobox2`, 1020:04bc) | not ported |

Palms are a building type (12) in the parks' submenu (15):
`start_building` plants them (`plant_palm`, 1010:1129).

### The button bar (1010:2c8b)

`ButtonImage`'s `OnClick` (1020:1946) finds the button under the mouse by
the table at `1110:12c0` (left and right edge of each of the 23 buttons)
and calls `button_bar_click`; so do the keys of `FormKeyDown` (1020:2daa).
The buttons held down are `1110:651a` (saved); a new game has the
pointer's down (1010:2fa9). Each click plays sound 9.

- A button that's up goes down. For buttons 6 to 22 the old tool's button
  comes up and the build menu closes (1010:2be1: it checks the form's
  `Visible`, which is always set, so it always plays the closing sound).
  Then the tool is the button's; a submenu's opens the build menu
  (1018:2b01); the road's keeps the mouse square as the drag's start and
  end and, oddly, selects building type 1.
- A button that's down comes up. For the tools 6 to 20 (1010:2b60): the
  pointer takes over, except for a submenu whose form is closed, which
  opens again; and the road drag ends. Button 22 stays down.

### Clicks on the island (1020:1984, 1020:1b31)

`HauptbildMouseDown` acts only on exactly `[ssLeft]` (with Shift or Ctrl
held, or the second press of a double click, the tools do nothing),
`[ssLeft, ssDouble]` (the info box stays open) and `[ssRight]`, which
moves the view towards the point (by its distance from the middle in
half rows of 18 and columns of 32 pixels, `hauptschirm_scroll`). The
tool's work (1028:3dd0) goes by the square under the cursor
(1018:3d8d), which `square_at` (1018:39c2) works out: it walks the
pixel's column of the view down from the top, a half row at a time,
until the square's tile (its corner's height and form) reaches below the
point, steps back one, and takes the square or the one beside it by
whether the point is on black in the form's pointer picture
(`gfxgroun.BMP` at `GFXPOS.DAT`'s row for the form). The raise and lower
tools take the square 16 pixels higher (1018:3dad) and its corner by the
view: `(i, j)`, `(i - 1, j)`, `(i - 1, j - 1)`, `(i, j - 1)` for views 0
to 3.

`HauptbildMouseUp` with the left button ends the road drag; with the
pointer it closes the info box.

### What the tools do (1028:3a68 to 1028:3cb1)

- Build: `can_build` for player 1; if it can, sound 0x1f and
  `start_building` with the cost and budget 11. For player 1 that books
  the price (0x67) and the cost rises from the square; a type there's
  only one of comes off the build menu (1018:26b9) and the pointer's
  button is pressed. Errors: 1 shows the building in the way (1030:235a,
  the square `can_build` kept at `1110:64ba`), the rest `FEHLER.TXT`'s
  text (1030:232d).
- Raise, lower: one step of the corner (`raise_corner`, `lower_corner`).
  Done, the player pays the cost, the books' "other" (0x6a) get it, it
  rises from the square and sound 0x1f plays; else the error as above.
- Demolish: `demolish` for player 1, which shows its own errors.
- Road: the press keeps the square (`1110:6344`), its ground and form
  (`1110:704e`, `704f`) and sets `1110:634c`; letting go with it set
  builds the road to `1110:6348` (`build_road` for player 1, which shows
  the errors 0xe and 0x1f and the building in the way).

The status bar's messages stay 20 ticks (`1110:009a`, counted down by
the clock's step 2) and play sound 0xc.

### The pictures over the island (1090:0014, 1090:124f, 1090:155b)

With a tool from 6 to 21, the clock's tick (only with the animations and
new people on) works out the square under the mouse (`1110:605a`) and
puts the tool's pictures into the people's records: record 0 is the
pointer (the raise and lower tools' arrows, `ANIMDAT1.DAT` rows 0x10 and
8, the demolish tool's 0x3c, eight frames each by `1110:6fd4`), records
1 on the squares the tool would take (`ANIMDAT1.DAT` row 76 plus the
square's form where it can, 0x17c plus it where it can't):

- the road being dragged: from the drag's start along the longer axis to
  the mouse square (which becomes `1110:6348`), as far as a road can go
  (a bridge from water), the squares without a building but palms;
- the road tool before the drag: the mouse square, red without the money
  for one square or where no road or bridge goes;
- a build tool: the type's area at the mouse square, red where
  `can_build` says no;
- the demolish tool: the building under the mouse (from its first
  square), none on palms and beach life.

They're drawn after the people (1088:3a19 with source 4), without what's
in front of them. As only this tick moves the drag's end, with the
animations or new people off the road is built from where the drag
started to there: one square.

## The build menu (`TBaumenue`, segment `1018`)

`BAUMENU.DAT` has 12 submenus (tools 9 to 20; the road's 16 is never
opened) of 20 rows, each a building type and its other looks (four types
the rotate button goes through), and two layouts of 32 rectangles; the
second, bigger one is for hotels (10), leisure (14) and airports (18).
`BAUMENU.TXT` has the headings and the labels. The lists change during a
game, so they're saved with it (unit 1018:34a0) and are the world's in the
port: `make_available` (10a0:0691, kinds 0 and 1) puts a type on the end of
its kind's submenu (1018:23f8, by `HAUSDATA.DAT` column 5 and the table at
`1110:00ea`), the hotels 0x58 to 0x68 and 0x71, 0x75 with the three types
after them as looks, a type that turns into another with the view
(`HAUSNR.DAT` column 1) with that one; building a type of which there's
one (`HAUSDATA.DAT` column 6) takes it off (1018:26b9).

The form (1018:2fa2) starts hidden (no height) at the right of the screen,
59 pixels below the main screen's top. Opening a submenu (1018:2b01) picks
the layout, draws the background (`bauwin1.BMP`, `bauwin2.BMP`), the
heading, labels, buttons and list (1018:1b5e), and the selected row's type
(1018:16c8): its long name (`HAUSNAME.TXT`'s second list), price, building
time and its unit (`AUSWAHL.TXT`), running costs, capacity, rent or
takings, and its picture on sand or grass (`backsand.BMP`,
`backgras.BMP`), turned with the view (1018:0e4d); then sound 0xd and the
focus. The selected row's type is the build tools' building (`1110:6058`).

| Rectangle | What |
|---|---|
| 1 | Close: the pointer takes over, the form closes |
| 2 | The title bar: any button drags the form, which shrinks to it meanwhile; after a double click it stays shrunk until the next single click there |
| 5, 7 | Scroll the list up, down |
| 8 | The list (7 or 11 rows of 15 pixels, `1110:14ca`): a click selects, a double click also closes the form |
| 10 | Rotate, when the row has other looks |
| 0x1d | "Schließen": as close |
| 0x1f | "Bauen": closes the form, the tool stays |

Keys (1018:3290): Left and Right open the submenu before and after
(`1110:14c5`, `1110:14d1`, through `button_bar_click`), Escape is close,
Enter "Bauen", Space gives the main screen the focus; F1 to F12, the
digits 1 to 9 and Pause go to the main screen's `FormKeyDown`, the other
keys nowhere (the island doesn't scroll while the build menu has the
focus). Turning the view redraws the open form's picture (1018:29f7).

## World and view

- The world changes only through commands (`World::apply`), as the
  human's player 1: `Build`, `Level`, `Demolish`, `RoadDragStart`,
  `RoadDragEnd` and `RoadDragCancel`. Their messages come back in
  `Tick::status` with the player; the build menu's changes in
  `Tick::build_menu_removed` and `Tick::pointer_tool`.
- The road drag's flag `1110:634c` and its start's ground `1110:704e`
  stay in the world: the computers don't build roads while it's set, and
  their roads go by the ground (a drag from water makes them look for
  bridges, as in the original). The drag's squares (`1110:6344`,
  `6348`) are the view's.
- The tool, the buttons, the mouse square, the build menu's form and
  selections are the view's, saved with the map's view fields.
- The tools' pictures are the view's. The original keeps them in the
  people's records 0 on, which are the world's and saved; it writes the
  squares past record 0x23 into real people for a road or building of more
  than 35 squares (kept out of the port, see
  [original-bugs.md](original-bugs.md)).

Single human: the money checks (`can_build`, raising, lowering, the road),
the messages, the books and the build menu are player 1's in the original;
another human seat would need its own. Nothing here stops the simulation:
the build menu is no modal dialog.

## Open questions

- The info box (`TInfobox2`) and the sabotage tool and form
  (`TSabowin`, 1028:2f4e, 10a0:1f65) aren't ported, nor the forms of
  buttons 1 to 5.
- The status bar's sounds and the tools' sounds aren't played yet.
- `1110:7044`..`704c` (saved) are the mouse on `Hauptbild` and the button
  bar and the one before, which the tooltips (1018:394e) use.
- The pointer's frame `1110:6fd4` and the preview's globals
  (`1110:6ff6`, `6ff8`, `6fce`) are shared with the people code in the
  original; the port's preview keeps its own.
