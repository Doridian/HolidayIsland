# Sabotage

The button bar's button 21 picks the sabotage tool and shows the sabotage
window, `TSabowin` (segment `1028`). The window lists nine kinds of
sabotage; a click on the island with the tool orders sabotage of the kind
picked there (10a0:1f65). What sabotage does once it's under way, day by
day, is `effects_tick` (10a0:117e). The port: `game::sabowin` (the
window), `game::tools` (the tool), `game::sabotage` (the world's part),
the command `Action::Sabotage { kind, at }`.

## The data

`TEXTE\SABOTAGE.TXT` (`load_sabotage`, 1028:361f, at the program's start,
1030:3757): two labels ("Anwendbar auf:", "Kosten:", `string[20]`,
`*1110:65d2`), then nine kinds of `0x232` bytes (`*1110:65bc`): the name
(`string[20]`), the picture in `GRAFIK` (`string[12]`, `+0x15`), the
description (`+0x22`), what it can be used on as text (`+0x122`), the
price (`Longint`, `+0x222`) and six building types (`+0x226`, five
characters each, 255 for none; only the piranhas' are read; the German
file gives the shark diver 254). The port reads it from its language
data (`lang/<code>/TEXTE/`): the world's prices and pools from the
world's language's, the window's texts from the player's. It then
reads `SABOTEXT.TXT` (1048:375e): the window's title, "Schliessen" and
"Sabotieren". It also sets the picked kind (`1110:65ba`) and `1110:54fa`
to 0.

| Kind | Name | Price (DM) | Used on | Days | Where the slot is kept |
|---|---|---|---|---|---|
| 0 | Piranhas | 50 000 | a pool (types 9, 64, 70) | 120 | the building's first square |
| 1 | Sprengladung (bomb) | 200 000 | any building | 10 | the first square |
| 2 | Stinkbombe | 10 000 | any building | 90 | the first square |
| 3 | Haitaucher (shark diver) | 30 000 | flat sea (form 16, ground 2) | 120 | the square |
| 4 | Ratten | 20 000 | any building | 90 | the first square |
| 5 | Schmierseife (soft soap) | 100 000 | an airport (`HAUSDATA.DAT` column 5 = 9) | 90 | the first square |
| 6 | Bauzeit (go-slow) | 10 000 | a building site (31) | 10 | the square |
| 7 | Rufmord (character assassination) | 50 000 | any building | 10 | the square |
| 8 | Strassen | 100 000 | a road piece (ground 10–58) | 10 | the square |

"Any building" is any type but palms, beach life and none (`1110:1358`).
The prices are shown times the currency factor (`SONSTIGE.TXT`).

## The window (`TSabowin`)

One `TImage`, `Saboimage`. `FormShow` (1028:34a2) sizes the form and the
image to 411 × 217 (the DFM's 392 × 250 is never seen), loads
`samenu.bmp` (the background) and `sabupr.BMP` (a button held down),
draws everything (1028:3079) and plays sound 0xd; `FormHide` (1028:35ee)
plays 0xe and frees the two. The parts are a table of rectangles
(`1110:16b8`, left, top, right, bottom): 1 the title bar, 2 the close
button, 3 the picture, 4 "Schliessen", 6 "Sabotieren", 12 the list, 13
the kind's name, 14 its description.

1028:3079 draws the buttons' texts, the title (colour 1), the nine names
on the list (14 pixels a row, the picked one in colour 11, the others 9,
over what's there), and for the picked kind its name, its description,
"Anwendbar auf:", what it's used on, "Kosten:" and its price (each part
cleared from the background first), and its picture, loaded from
`GRAFIK` each time.

The mouse (`SaboimageMouseDown`/`Move`/`Up`, 1028:2d67, 1028:2ecc,
1028:2f4e), left button only:

- the close button, "Schliessen" and "Sabotieren" go down (sound 8) and
  come up on release (sound 10). On the button: the close button and
  "Schliessen" hide the window and give the pointer (button 21 up, 22
  down, tool 22); "Sabotieren" hides it, keeping the tool (it also sets
  the tool to 21, released anywhere, and `1110:54fa := 1`, which nothing
  reads).
- the title bar drags the form, which shrinks to the bar meanwhile
  (`ClientHeight` 16), kept on the screen; the release gives the height
  back.
- the list picks the kind on the row (rows go through `1110:173a`, the
  identity), sound 9.

`FormKeyDown` (1028:394c), without Ctrl and Alt: Escape and Space as
"Schliessen", Up and Down pick the kind above or below (through
`1110:1730`, the identity), Enter as "Sabotieren". Alt+F4 as "Schliessen"
(the key is set to 0xff).

The window isn't modal and doesn't stop the clock. The tool is 21 from
the button's click on, so the island takes sabotage clicks while the
window is open, with whatever kind is picked; "Sabotieren" only closes it.
Picking another tool (1010:2be1) or the button again (1010:2c8b) hides
it. The kind stays picked for the rest of the session (it's not saved).

## Ordering sabotage (10a0:1f65)

The tool's click (`sabotage_tool`, 1028:3d93) calls 10a0:1f65 with the
square under the mouse, the kind and player 1. In order:

1. Off the map: error 0x16. The square's owner is the player: 0x19.
   Player 1 without the price: 0xe (the computers aren't checked, but
   never sabotage).
2. By kind, the square must fit (the table above); else error 0x1a. The
   piranhas' check never gives the error: its loop always ends at 6 and
   tests for 5, so piranhas for a building that isn't a pool do nothing
   (`fix.sabotage_pool_error` gives 0x1a).
3. 10a0:1d5e: the player pays the price (player 1: the books' "other",
   0x6a, and the money rising from the square). `Random(100) < 30`: it
   fails. Then `Random(100) < 50`: quietly (message 0x1c, "a saboteur
   left the island"); else it's found out (10a0:1b31): the player's
   reputation goes down 50 (to 0 at the least) and a message by kind:
   0x1b, 0x1d, 0x3e, 0x20, 0x3f, 0x40, 0x1e, 0x1f, 0x41.
4. Otherwise the first free one of the five slots (`*1110:705a`) gets the
   attacker, the victim (the owner of the slot's square), the kind, the
   square and the days; with none free it fails (0x1c). Then its first
   effect (10a0:19d2): messages 0x21 (piranhas), 0x2f (stink bomb), 0x30
   (rats), 0x18 (go-slow), 0x19 (character assassination); the bomb and
   the road blow up what's there (10a0:1000: the people leave, it's
   demolished for no one, an explosion, people kind 0xdb, on each square
   of the building); the shark diver goes in the water (people kind 3);
   soft soap waits.

The messages have the attacker and the victim (`a`, `b`) and the square.
`effects_tick` then counts each slot down a day at a time, on the day's
second tick (1020:1e9b), see `game::sabotage`. The people code asks
whether a square has sabotage of a kind (10a0:0ec8: the piranhas in the
pool, the soap on the runway) and ends the soap (10a0:0e17).

## The tool's picture (1090:0014)

Like the demolish tool's: the building under the mouse from its first
square (none on palms, beach life and nothing, where the area of the
building last found is shown), with the pointer `ANIMDAT1.DAT` 0x44 to
0x4b at offset (0x16, 0). The squares are red unless the picked kind
fits there for player 1: on the map, not player 1's, with the money,
and the kind's check of the table (here the piranhas' works).

## The port

- `Action::Sabotage { kind, at }` → `World::sabotage_command`
  (10a0:1f65, 10a0:1d5e, 10a0:1b31, 10a0:19d2, 10a0:1000). The world
  keeps the kinds' prices and targets in `Tables::sabotage`
  (`GameData::load` reads `TEXTE/SABOTAGE.TXT`).
- `World::sabotage_fits` is the picture's check.
- `Tools::sabowin` holds the window (`Sabowin`, given by the caller);
  `Tools::click` shows and hides it, `Tools::press` turns the click into
  the command with `Tools::sabotage_kind`, and `Tools::sabowin_mouse_*` /
  `sabowin_key_down` pass the window the mouse and keys and do what its
  buttons say to the tool.
- `1110:54fa` isn't kept: nothing reads it, and a map's value goes
  through the map's unported fields unchanged.

Single human: only player 1's money is checked, and only player 1 gets
the books and the rising money.
