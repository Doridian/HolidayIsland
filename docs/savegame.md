# Saved games

Saving and loading a game, and the file both use. The format is shared by
the CD's maps and scenarios ([data-files.md](data-files.md#maps-scenarios-and-saved-games)).
Ported in [`formats::savegame`](../crates/formats/src/savegame.rs) (the
file, field by field), [`game::savegame`](../crates/game/src/savegame.rs)
(the world and the view to and from it),
[`game::ladenform`](../crates/game/src/ladenform.rs) (the load form) and
[`game::dateimenue`](../crates/game/src/dateimenue.rs) (the file menu).

## The file

`save_map` (1048:2bae) writes the string `1.000` (`string[5]`, from
`1110:004e`; `load_map` reads six bytes and never looks at them), then calls
one routine per unit, each of which `BlockWrite`s its globals one after the
other. `load_map` (1048:2a19) calls the matching readers in the same order.
There are no tags, lengths or checksums. Each save routine writes exactly
what its load routine reads (checked routine by routine against the
decompiled pairs); the order of the reads is
[`formats::savegame::layout`](../crates/formats/src/savegame/layout.rs).
Everything is little-endian, `Integer`/`Word` 2 bytes, `Longint` 4,
`string` 256 bytes (the length, then the characters; whatever follows the
characters is whatever memory held). Heap blocks are written whole,
behind the far pointer the global holds. A file is 765–805 KB.

| Unit (load, save) | What |
|---|---|
| 1000:0c6a, 1000:032e | The options (`0054`..`0066`), the clock's slice and speed, the colours, the view's rotation and origin (twice) and parity, flood and earthquake, the start menu's island settings (`008a`..`0092`), `0102` (8 strings, unknown) and the players' names (`0902`, 9 strings), the nine map layers, the location and plot maps, the plot count, the squares `spawn_people` looks at, the main screen's tool and mouse state (`5fb2`..`6090`), the view's size, the difficulty, the road drag, the scenario's flags |
| 1068:32b5, 1068:32df | Four lists, each a `Word` count and the records: the plots (0x364), the houses (0x2c), the other buildings (0x1b), the building sites (0xd). The records hold their own far pointers to the next one, which mean nothing in the file |
| 1010:0d4c, 1010:0b02 | `MODATYP.DAT`'s values (but `Strassenbaukosten`), the players and opponents, the renovations, `Klassenmischung`, the computers' budgets, the forms' `Enabled` (`645a`), then each form's window but form 2's (1010:08d7, 1010:0799): `Top`, `Left`, `Width`, `Height`, the bytes at `+0x29` (`Visible`) and `+0x2a` (`Enabled`); forms 10 and 11 are saved as zeros and skipped on loading |
| 1008:539f, 1008:54c0 | `ABRECHNE.DAT`, the accounting schedule, the month's sums |
| 1088:8a07, 1088:8950 | The people, their paths, entries and tracks |
| 1028:2506, 1028:25f4 | The auction ([auction.md](auction.md#state-unit-10282506)) |
| 1018:34a0, 1018:3350 | The build menu's (`TBaumenue`) fields: its lists (`+0x180`, `+0x184`, `+0x1c6`, the world's) and the form's state, the selections and scrolling (the view's) ([player-tools.md](player-tools.md#the-build-menu-tbaumenue-segment-1018)) |
| 1010:2fea, 1010:2fce | The button bar's buttons held down (`651a`) |
| 10a8:32ec, 10a8:3080 | The computer players ([computer.md](computer.md#state)); their road candidates for players 2 to the player count only |
| 1050:313a, 1050:2eaa | The drawing's tables (`DARSTELL.DAT`, ...) |
| 1010:3e8a, 1010:3ee7 | The events, the days since the start |
| 1070:3bf8, 1070:3b27 | The money menu's fields and books |
| 1030:2bc5, 1030:2bae | The date |
| 1018:3e46, 1018:3dd6 | `7044`..`704c`: the mouse on the main screen, the one before, its buttons |
| 1048:25ea, 1048:2524 | The news archive and the TV ([fernseher.md](fernseher.md#saved)) |
| 1020:2b1d, 1020:2909 | The main screen's fields: the scroll position (`+0x408`), the scroll bars, the clock's counters (`+0x1d6`, `+0x1d8`) |
| 1078:34ad, 1078:33e5 | The sums over the island, the players' block, the money, loans, interest and guests, the economy's periods |
| 1080:3c6d, 1080:3c3c | The ground and form where the last road drag started |
| 10b8:50de, 10b8:4e34 | The overview map's (`TUebersichtsmap2`) fields |
| 10a0:23c9, 10a0:232f | The sabotage, 0x15e bytes `save_map` writes as zeros (`FillChar`) and `load_map` reads into a local and drops (the CD's files have other bytes there, maybe an older version's), the disaster's damage, the championships |

`1028:3e41`, `1040:1564`, `1018:38fd`, `1000:3b06`, `10a0:314c` and
`1020:3f4e` are called too but read nothing. `save_map` closes the file
after the first 13 units and opens it again at its end for the rest,
which gives the same file.

### The port's chunk

The port writes its own data after the original's last field: the world's
fixes ([original-bugs.md](original-bugs.md#options)). The original still
loads such a file: `load_map` (1048:2a19) opens it (`Reset` with a record
size of 1), `BlockRead`s the version and each unit's fixed sizes, and
closes it, without looking at the file's length or at what follows.
`save_map`'s `Rewrite`
truncates, so the original saving over the file drops the chunk, and the
game it saved loads in the port with the player's defaults.

The chunk is `HIPORT`, a 0 byte and the chunk's version (1), a `Longint`
length, and that many bytes of text: one `key=value` per line. The world's
fixes are `fix.<key>=1` or `=0`, every fix the port knows
(`game::enhance::fixes_chunk`). Reading (`game::enhance::fixes_in`): a
file without the chunk (every original file, `formats::savegame::port_chunk`
gives none) or without a fix's line gives that fix its default: the
player's for a game loaded from the start menu, the running game's for one
loaded from the file menu (`World::load_map`); lines the port doesn't know
are skipped. `SaveGame::trailing` holds the chunk as read, and
`World::to_map` writes it. A sidecar file was the other choice; the chunk
keeps a game in one file and needs nothing from the original.

## World and view

A map file holds the world and one player's screen. `game::savegame`
splits it: `World::from_map` and `World::to_map` for the world's fields,
`MapView` for the view's (the scroll position, the owner and occupancy
pointers, the news options). The view's fields are the forms' (but the
clock's counters, the date and the books), the windows, the main screen's
tool and mouse, the TV (but the news archive, which is the world's), the
player's options (`optionen::Settings`: `005b`..`0061`, `0064`,
`0066`), the drawing's tables, the view's size, the road drag's ends, the button bar and the auction form's state. Fields neither
side uses yet are kept as they were read (`Fields`, about 20 of the
world's and 150 of the view's in the CD's files) and written back, so a
loaded game saves as it came. A new game has none of them, and writes
zeros there.

Saving only reads the world, so any player can save their copy; it isn't
a command. Loading in the middle of a game is the command
`command::Action::Load`. Single human: the original saves the one
player's screen with the world, and the port does the same; a
multiplayer save would keep a `MapView` per seat, or none.

Each of the CD's seven files, loaded and saved again, gives the same
fields but the bytes behind the players' names and the computers' status
text and the 0x15e reserved bytes (`orig_savegame.rs`).

## What a saved game loses

`load_map` only overwrites what the file holds; everything else stays as
it was in memory (`World::keep_unsaved`). From the start menu that's what
the start of a game set up; from the file menu it's the game that was
running, so these carry over from one game into another:

- `RandSeed` (`1110:4a06`): a loaded game goes on with the numbers in
  memory, after the set-up's draws from the start menu.
- An oil slick going on (`1110:0088`).
- The people's `1110:6fcc`, `6fce`, `6ff2`, `7040`/`7042` and `3c42`.
- The auction's limits (`1110:654e`), whether the player joined
  (`1110:65ae`) and the form's timers.
- The computers' road candidates of the players past the file's count.
- `Strassenbaukosten` (`1110:64ee`), which the port keeps in
  `Tables::road_cost`, and the tables from `DATA\`, which come from the CD.

And the order of two lists changes: `save_map` writes the building sites
from the list's head and `load_map` puts each in front of the ones before
it, so they come back the other way round; and loading builds each plot's
lists of houses and facilities from the houses' list (1068:2d49), the
reverse of that list, whatever order the game had. A reloaded game can
therefore play differently from the one in memory. With those turned
back, a played game, saved and loaded, plays on exactly as the game it
came from (`orig_savegame.rs`). `fix.load_order` loads them that way:
the file's chunk says whether it's on, or for a file without one the
game it's loaded into (`World::set_loaded_fixes`).

## Loading from the start menu: `TLadenForm`

"Altes Spiel fortsetzen" (1058:0c7d) disables the start menu and opens
`TLadenForm` (1038:3b46): 146×210 at the bottom of the screen, centred
across, its one image `Bild` with `datwin2a.bmp` and widgets: the list of
saved games (24,46, 96×108, 15-pixel rows, `datListe.bmp`), its arrows
(57,25 and 57,159, `datUp`/`datDow`), "Laden" (34,186, `datB1.bmp`, the
text `SONSTIGE.TXT`'s 7th) and the close button.

- The list is `MAPS\*.map` (`FindFirst` with any attribute), each name
  without its last four characters put in front of the ones before
  (1038:3983), so it comes out in the reverse of the directory's order.
  The list box holds 100; the original writes more past its buffer.
- An arrow moves the list a row; it hides when the list can't go further
  that way, and the other shows when it can (1038:36d5, 1038:3776). They
  start shown only where the list can scroll.
- `BildMouseDown` (1038:3e24) offers a press to the list, the close
  button, "Laden" and the arrows; `BildMouseUp` (1038:3ef5) to "Laden",
  the close button and the arrows. Any mouse button presses the buttons.
- "Laden", or a double click on the selected game (1038:3682), with a
  game selected (1038:3a95): `load_map_name` (`1110:1322`, a `string[12]`)
  becomes the name and `.map`, the start menu hides, `hintergrund`'s
  `Timer1` goes to state 1000 with `+0x190` set and `+0x191` cleared, and
  the form closes (`caFree`). States 1000–1500 then set up a new game
  without an island and `load_map` the file over it
  ([startup.md](startup.md#starting-a-game-states-10001500),
  `hintergrund::load_saved_game`). The file's name, colours and
  difficulty stay, unlike a scenario's.
- The close button enables the start menu and closes the form
  (1038:3817). The form has no key handler.

## The file menu: `TDateimenue`

The button bar's first button (1010:2c8b), or `1` or `D` on the main
screen (1020:2e09, 1020:2daa), opens it. `FormShow` (1040:12be) stops the
clock, hides the help, disables the other forms, sizes the form to
202×210 (it stays where the DFM or the last drag put it; the map file's
windows skip it), draws `datwin1c.BMP` with the title (`SONSTIGE.TXT`'s
6th text) and the six buttons' texts, and sets up the widgets: the list
of saved games (97,46, as the load form's, listed by 1040:10e5 the same
way), its arrows (130,25 and 130,159, always shown), and the name field
(8,90, 79×19, at most 8 characters). It lets the button bar's button up
again.

The buttons are drawn on the background and found by the table at
`1110:1812` (left, top, right, bottom, inclusive; each button has one
entry to draw and the next to hit): "Laden" 8,49, "Speichern" 8,71,
"Neues Spiel" 8,114, "Beenden" 8,136, "Abbruch" 8,186, "OK" 106,186, the
close button 0,0 and the title bar 16,0. A left press draws the button
down (`datB1pr.bmp`, `closepre.bmp`, sound 8); the release draws it up
from the background (sound 10) and, still over it:

- "OK", "Abbruch", the close button: the menu hides, the clock runs.
- "Laden" (1040:0f27): with a game selected, `load_map` of
  `MAPS\name.map` over the running game, then `game_show_state`
  (1010:26fa: the view from the file's origin, the money and date, the
  forms). The menu stays open.
- "Speichern" (1040:0e03): if the name field isn't focused, it focuses
  it. Otherwise the name must be 1–8 characters, a letter, then letters,
  digits or `_` (1040:0d0f), or a message box says "Dateiname ungültig";
  with a good one, `save_map` to `MAPS\name.map` and the list again.
- "Neues Spiel": `back_to_start_menu` (1010:25dc) saves `MAPS\Last.map`,
  frees the game, hides the forms, reloads the data and shows a new start
  menu from the globals.
- "Beenden": the menu hides and `TEndeform` asks.

The title bar drags the form, kept on the screen (1040:09ca). Any button
offers the press to the list (which also puts the selected name into the
name field, 1040:0192), the name field and the arrows. A double click on
the selected game loads it (1040:0180). Enter in the name field is its
`Click`, the save, run while it's still focused (1040:02c8). The keys
(1040:1572), without Ctrl or Alt, after the name field had them: Escape
and Space close the menu, Enter focuses the name, `L` loads, `S` saves,
`Q` quits, `N` starts a new game, Up and Down move the selection
(without the name). Alt+F4 closes it.

Quirks, kept: any press while the name is focused unfocuses it, so
clicking "Speichern" while typing focuses the field again instead of
saving, and only Enter saves a typed name (`S` types an `s` while the
field has the focus); a name from the list goes into the field without moving its cursor;
`1110:6608` remembers whether "Laden" or "Speichern" was clicked last for
an Enter handler (`EditierfensterKeyDown`, 1040:0cd3) nothing calls.
With `enhance.edit_field` a press on a button leaves the name focused, so
"Speichern" saves a typed name, and a name from the list puts the cursor
after it.

## The port

- Saves go to `MAPS` in the platform's data directory
  (`platform::storage::save_dir`: `$XDG_DATA_HOME/holiday-island/MAPS` or
  `~/.local/share/holiday-island/MAPS` on Linux,
  `~/Library/Application Support/Holiday Island/MAPS` on macOS,
  `%APPDATA%\Holiday Island\MAPS` on Windows), or `--save-dir=DIR`. The
  game's own `MAPS` is only read: the lists show both (the player's file
  wins a name both have), sorted by name as a stand-in for the
  directory's order, then backwards as the original lists them (with
  `enhance.saved_games_order` in that order). `Last.map`
  goes to the player's directory too. A save is written to a temporary
  file and renamed.
- A file the parser can't read loads nothing (the original, with I/O
  checks off, stops reading at the error and keeps the rest of the old
  game, [original-bugs.md](original-bugs.md)).
- Not ported: the message box, the button
  bar's mouse handling (a click on its first button opens the menu), the
  hourglass cursor, the music and data reload of `back_to_start_menu`, and
  the progress window of states 1000–1500.
- The load form doesn't cut a name to `load_map_name`'s 12 characters:
  the original's 8.3 names always fit, and a longer host file name
  couldn't be found again.

## Open questions

- What `1110:0102` (8 strings), `0x15e` bytes of the CD's files in unit
  10a0:23c9, and the other unported fields are.
- Whether anything of the forms' windows matters to the port, which lays
  out its own.
