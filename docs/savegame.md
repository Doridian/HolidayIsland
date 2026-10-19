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
`1110:004e`; the later build's `1.100`, which the port writes too,
[versions.md](versions.md); `load_map` reads six bytes and never looks
at them, so either build loads the other's files), then calls
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
| 1010:0d4c, 1010:0b02 | `MODATYP.DAT`'s values (but `Strassenbaukosten`), the players and opponents, the renovations, `Klassenmischung`, the computers' budgets, the forms' `Enabled` (`645a`), then each form's window but form 2's (1010:08d7, 1010:0799): `Top`, `Left`, `Width`, `Height`, the bytes at `+0x29` (`Visible`) and `+0x2a` (`Enabled`); forms 10 and 11 are saved as zeros and skipped on loading. Loading puts the forms there and opens or closes them ([layout.md](layout.md#saved-windows)) |
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
fixes ([original-bugs.md](original-bugs.md#options)) and the world's
language ([language.md](language.md#the-worlds-text)). The original still
loads such a file: `load_map` (1048:2a19) opens it (`Reset` with a record
size of 1), `BlockRead`s the version and each unit's fixed sizes, and
closes it, without looking at the file's length or at what follows.
`save_map`'s `Rewrite`
truncates, so the original saving over the file drops the chunk, and the
game it saved loads in the port with the player's defaults.

The chunk is `HIPORT`, a 0 byte and the chunk's version (1), a `Longint`
length, and that many bytes of text: one `key=value` per line. The world's
fixes are `fix.<key>=1` or `=0`, every fix the port knows, and the
world's language is `language=de` or `=en` (`game::enhance::world_chunk`).
A file without a language (`game::lang::language_in`) takes the game
folder's (`World::folder_language`). Reading the fixes
(`game::enhance::fixes_in`): a
file without the chunk (every original file, `formats::savegame::port_chunk`
gives none) or without a fix's line gives that fix its default: the
player's for a game loaded from the start menu, the running game's for one
loaded from the file menu (`World::load_map`); lines the port doesn't know
are skipped. `SaveGame::trailing` holds the chunk as read, and
`World::to_map` writes it. A sidecar file was the other choice; the chunk
keeps a game in one file and needs nothing from the original.

After the language come the seats (`game::seats`,
[port-architecture.md](port-architecture.md#stretch-goal-network-multiplayer)),
every time: `seats.host=<client>`, `seats.clients=` each human client as
`<client>:<seat>`, with `:away` after one that's disconnected (`0:1 5:1
9:2:away`: the host and a partner in seat 1, a client of seat 2 away),
and `seats.forced=` the keys of the fixes several clients switched on,
which go off again when one is left (`Seats::chunk_lines`). A file
without them (every original file) is the host alone in seat 1. Loading
(`World::load_map`, `Seats::loaded`) keeps the running game's clients as
they are; the file's others come back away, so a computer plays their
seats until they rejoin, and they still count for the forced fixes.

Then each of seats 2 to 8 whose slots (`seats::SeatSlots`, what the
original keeps once for player 1) aren't a new game's:
`seat.<p>.books=` the 29 sums, `.insured=1`, `.money_menu=1`, `.drag=`
on, ground, form, `.damage=` amount, insured, `.worth_news=` the net
worth news it has had (a bit each for events 12, 16 and 18), and
`.build_menu=` its three fields' bytes in hex (`World::seat_lines`; `SeatSlots::from_chunk`
reads them, and a file without them has none). Then
`options.auction_timeout=` the host's auction timeout in ticks
(`savegame::auction_timeout_in`; a file without it gets 300, or from the
file menu the running game's). Then, if any auctions wait their turn
(`fix.auction_queue`, [auction.md](auction.md)), `auction.queue=` their
`item,price` pairs, the first next (`auction::queue_lines`,
`auction::queue_in`; a file without it has none), and if any bankrupt
human seats' property waits for event 10 (`fix.sale_event_busy`),
`auction.property_waiting=` those seats (`auction::property_waiting_in`).

A save made by whoever knows every client's view
(`World::to_map_with_views`) has each one's as `view.<client>.` lines:
`.scroll=`, `.pointers=`, `.news=`, `.origin=` (if it has one),
`.viewpoint=` rotation, i, j, parity (with `fix.view_people`), and
`.fields=` the view's other fields, `dest#n#hex` apart by spaces.
`savegame::client_view` gives a client theirs back; the view's own
people aren't kept, each view makes its own. The original's fields have
the saving client's view, as for one player.

With `fix.view_news`, `news.nowhere=` lists the archive's entries whose
square (`p`) or way back (`o`) is nowhere in the world (`3po 7o`), and
`events.nowhere=` the events whose square is; the original's fields have
the saving player's view origin there (`MapView::origin`), as the
original would have written it, and loading puts nowhere back
(`World::take_nowhere`).

With `fix.saved_state` the chunk also keeps the world's state the
original's fields lose ([below](#what-a-saved-game-loses)), as `state.`
lines after the language (`World::unsaved_state`). Numbers are decimal,
a flag `0` or `1`, a square `i,j`, a timer that isn't running `-`, a
byte list hex without its trailing zeros:

| Key | What |
|---|---|
| `state.rand_seed` | `RandSeed` (`1110:4a06`) |
| `state.oil_slick` | An oil slick going on (`1110:0088`) |
| `state.people.lane`, `.6fce`, `.last_building`, `.beach`, `.3c42` | The people's `1110:6fcc`, `6fce`, `6ff2`, `7040`/`7042` and `3c42` |
| `state.auction.limits` | The eight players' limits (`1110:654e`), comma-separated |
| `state.auction.joined`, `.open`, `.opening`, `.sold`, `.bid_pause`, `.since_call` | Whether the player joined (`1110:65ae`), the form's open state, its timers (`Timer2`, `Timer1`), the paddle's pause and the ticks since the last call (`1110:65aa`) |
| `state.auction.frames` | The auctioneer's nine frame values (`auction::Frames::values`) |
| `state.auction.seats` | With several human seats: the seats that started, passed and hold a button (bits), and the wait's ticks left (`-` none) |
| `state.computers.<p>.road_ends`, `.roads` | The road candidates (`*[1110:70e2[i]]`, `*[1110:70e6[i]]`) of each computer `p` past the file's player count: three hex lists |
| `state.people.view_rotation` | With `fix.view_people`: the file's people are a view's, placed for this rotation ([below](#the-people-with-fixview_people)) |
| `state.people.homes` | Then: the world's people whose `+0x22` the file's lack (the door people's buildings), `n:i,j` apart by spaces |
| `state.people.last` | Then: the world's last record (`1110:6ff4`), if the file's is another |
| `state.people.free` | Then: the world's free records that the view's own took in the file, `n:hex`, as they were |

Loading (`World::load_map`, so both the file menu's command and the
start menu's `hintergrund::load_saved_game`) runs `World::keep_unsaved`
as before, then, with `fix.saved_state` on in the loaded world (the
file's chunk's, or for a file without one the game's it's loaded into),
puts each value the chunk has in place of the one kept
(`World::take_saved_state`). A line that's missing or doesn't read
leaves the kept value, and lines the port doesn't know are skipped, so
an original file, a German v1.0 one and an older port's load as before.
The free records go back after the views have taken their people from
them (`World::take_saved_free_people`: `Action::Load` after
`hand_over_view_people`, the start menu after `split_off_view_people`).
With the fix off the port writes no `state.` lines and reads none.

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

The forms' windows ([layout.md](layout.md#saved-windows)) are the
view's: loading puts the player's forms where the file has them, and
saving writes where they are (`savegame::Windows`, `holiday::windows`).
The world takes one thing from them: whether the money menu is open (its
saved height), which decides when the year's total is worked out
(`World::money_menu_shown`, 1070:2617; a file without windows leaves it
as it was).

Saving only reads the world, so any player can save their copy; it isn't
a command. Loading in the middle of a game is the command
`command::Action::Load`, the host's (`game::seats`; another client's
does nothing). The original saves the one player's screen with the
world, and the port does the same: the saving client's, its view origin
for the news that point nowhere and, with `fix.view_people`, its
viewpoint and people. The seats, the seats' slots and, from
`World::to_map_with_views`, every client's view are in the port's chunk
([above](#the-ports-chunk); decision 12 in
[port-architecture.md](port-architecture.md#decided)). The world's
seat-1 fields stay the host's seat's whoever saves: the file is the
world's, and a client of seat 2 finds their seat's books in its slot.

Each of the CD's seven files, loaded and saved again, gives the same
fields but the bytes behind the players' names and the computers' status
text and the 0x15e reserved bytes (`orig_savegame.rs`).

### The people with `fix.view_people`

With the fix the world's people go by no view, and the spectators,
builders, taxiing planes and jetty boats are the view's own
([people.md](people.md#who-is-whose-fixview_people)). A save still
writes what the original writes, so the original loads it as its own:
`MapView::of_world` puts the view's viewpoint into `1110:007c`, `5530`
and `5534` (not the world's) and the people as that view sees them into
`*1110:6fd6` and `1110:6ff4` (`MapView::people`): the world's placed
again for the view's rotation, as 1088:7a14 would have them, without the
buildings' squares only the fix keeps in `+0x22`, and the view's own in
the general range's free records, as 1088:0324 would have put them
(`ViewPeople::saved_with`). The rest of the table, the marker layers and
the counts are the world's. Without the fix (`MapView::people` none) the
file is the world's, as before.

Loading a file with the fix (any file: an original one, a German v1.0
one, the port's) hands the views' kinds over to the view
(`World::hand_over_view_people`, `Tick::view_people`; at the start of a
game `World::split_off_view_people`), and places the rest again for the
world's rotation, so the view's people start as the file had them. A
file saved with the fix loads in the original, and in the port without
the fix, with everyone in the world.

What the view's people lose of the world's this way (the door people's
buildings in `+0x22`, so that after a load they are placed by where they
stand for the view and have no building until they leave; the last
record; the free records the view's own took), `fix.saved_state`
keeps in the chunk. Loading then gives the door people their buildings
back and places everyone again for the world's rotation from them
(`place_again_from_home`, as `World::people_for_view` placed them for
the view), so the world is the one that was saved whichever way the view
was turned.

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
  `Tables::road_cost`, and the tables from `DATA\`, which come from the CD
  (and stay so with `fix.saved_state`, which keeps the rest).

And the order of two lists changes: `save_map` writes the building sites
from the list's head and `load_map` puts each in front of the ones before
it, so they come back the other way round; and loading builds each plot's
lists of houses and facilities from the houses' list (1068:2d49), the
reverse of that list, whatever order the game had. A reloaded game can
therefore play differently from the one in memory. With those turned
back, a played game, saved and loaded, plays on exactly as the game it
came from (`orig_savegame.rs`). Two fixes do it, each on its own:
`fix.saved_state` keeps the state above in the port's chunk
([above](#the-ports-chunk)), and `fix.load_order` loads the lists in
their order. Neither touches the other's part: with only
`saved_state` the lists still come back turned round, with only
`load_order` the state is still the game in memory's. For both, the
file's chunk says whether they're on, or for a file without one the
game it's loaded into (`World::set_loaded_fixes`). With both (and every
other fix) on, a game saved in the middle of an oil slick and an
auction and loaded into another running game is the game that was
saved, field for field, and plays on bit for bit
(`orig_savegame.rs`, `saved_state_plays_on_as_the_game_saved`).
`Tables` and `Strassenbaukosten` aren't saved either way: they come
from the game folder.

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

The title bar drags the form: a left press on it shrinks the form to
its 16-pixel title bar (`ClientHeight := 16`, 1040:073c, en 1040:077e),
the move keeps it on the screen by its width and that height
(`screen_height - Height`, 1040:09ca, en 1040:0a45), and the left
button's release gives it its height back, `rect0.bottom + 1` = 210
(1040:0a4c, en 1040:0ac7); unlike the main screen's there's no double
click's roll-up. So it can be dropped with all but its title bar off
the bottom of the screen ([original-bugs.md](original-bugs.md#ui-quirks));
with `enhance.form_drags` the move keeps it whole on the screen
(`Dateimenue::height`, `bounds`). Any button
offers the press to the list (which also puts the selected name into the
name field, 1040:0192), the name field and the arrows. A double click on
the selected game loads it (1040:0180). Enter in the name field is its
`Click`, the save, run while it's still focused (1040:02c8). The keys
(1040:1572), without Ctrl or Alt, after the name field had them: Escape
and Space close the menu, Enter focuses the name, `L` loads, `S` saves,
`Q` quits, `N` starts a new game, Up and Down move the selection
(without the name). Alt+F4 closes it.

The later build ([versions.md](versions.md), en 1040:077e–en 1040:110c)
draws `UPGRADE\datwin1d.BMP` with a fifth button on the left,
"Löschen" (`UPGRADE\SONSTIGE.TXT`'s 25th text) at 8,114; "Neues Spiel"
moves to 8,136, "Beenden" to 8,158 (two more entries in the table, en
`1118:1812`). "Löschen" and the key `D` delete the selected game: a
question (`Wirklich loeschen?`, `Vous êtes sûr?` or `Are you sure?` by
`land`), then `MAPS\name.map` is erased, the list filled again, and the
name field emptied and erased from the background (its length byte
`+0x191` := 0, en 1040:110c; `Dateimenue::clear_name`; its cursor stays
where it was). The
port has that menu with `UPGRADE\` (`Dateimenue::with_upgrade`) and asks
the question (`MessageDlg(…, mtConfirmation, [mbOK, mbCancel], 0)`,
en 1040:118b): only OK erases, and only the player's own copy in the
port's save directory, never the game's `MAPS\`. The invalid name's
message is `MessageDlg(…, mtError, [mbOK], 0)` (en 1040:0fb9). Both are
the port's message boxes (`game::msgdlg`), drawn like the game's quit
question rather than as Windows dialogs. Saving, loading and erasing set
`Screen.Cursor := crHourGlass` around the file's work (en 1040:0f11,
en 1040:1069, en 1040:1198), which the port shows for a frame.

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
- "Neues Spiel" (`back_to_start_menu`, en 1010:3582) frees the game
  (the later build's wait for the CD isn't ported) and puts back what `init_game_defaults` and the data reload set (the start
  menu's settings but the opponents and the name, the options but the
  speech, the sounds), so the next game starts as the first did
  ([startup.md](startup.md#back-to-the-start-menu-101025dc-en-10103582)).
- The load form doesn't cut a name to `load_map_name`'s 12 characters:
  the original's 8.3 names always fit, and a longer host file name
  couldn't be found again.

## Open questions

- What `1110:0102` (8 strings), `0x15e` bytes of the CD's files in unit
  10a0:23c9, and the other unported fields are.
