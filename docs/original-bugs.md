# Bugs in the original game

The port reproduces the original's behaviour, bugs included, so that it
plays and computes the same. The plan is to make each of these fixable later
behind an option. This file lists every bug and quirk found so far, so that
none gets lost.

Each place in the code that keeps one is marked with a `// BUG(original):`
comment, so `grep -rn "BUG(original)" crates` finds them all. Add a row
here whenever you keep a bug or work around one. Where another doc
explains a bug in detail, the row links there instead of repeating it.

## Planned options

- Fixes that change the simulation become flags in the `World`, e.g. a
  `fixes: Fixes` set. They are saved with the game and agreed by every
  player, because they change the world's hash.
- Fixes that only change the UI become view settings.
- With every flag off, the port behaves exactly as the original does.
- Crashes and hangs have no "original" mode, because the port can't
  crash. It does the fallback listed below. The option there would be
  a better fallback, where one makes sense.

## Crashes and hangs

| Where | Original | Port today |
|---|---|---|
| 1010:1129 | Planting a palm on a square with no plot counts it through a nil pointer ([simulation.md](simulation.md#open-questions)) | Skips the count |
| 1010:129e | Without `HAUSNR.DAT` (no palm stages) the palms' tick divides by zero | Treats every palm as old |
| 1090:1a25 area (1078:0002), 10a8:2ade, settlement step 0x1e | A house type with no capacity divides by zero | Skips the house |
| 1078:280b, 1030:154c | Demolishing a site no area holds reads a nil record; a type with no building time, squares or width divides by zero | Skips that step |
| 1078:0eda, 1078:124e, 1078:1b5e, 1068:2d49, 10a0:0026, 10a0:117e | A building or palm on no plot, a plot number without a record, a building without a record or a square no site holds is used through a nil pointer | Skips that step |
| 1078:203d, 1078:15d9 | A computer's budget number isn't checked, so a bad one reaches outside the budgets | Leaves the budgets |
| Settlement (1008:1bf7) | Division by zero for a type with no beds or places, or a factor whose divisor is 0 ([settlement.md](settlement.md#open-questions)) | The division gives 0 |
| 1008:01d2 | The plots' average divides by zero without plots | Gives 0 |
| 1030:1ae9, 1030:185d | Taking an effect off the location map divides by zero for a factor of 0 | Skips that cell |
| 1030:11a0 | Formatting money divides by zero for a currency factor of 0 | Works the product out in halves |
| Computer loans (10a8:2a60) | A `Kreditgroesse` of 0 or less loops forever | Skips the loans |
| 1030:23d3 `draw_text` | A word longer than a line, after the first space, writes past the 10 entries for line ends | Stops at nine lines |
| 10c0:13b0 | Dragging a scroll bar's thumb divides by its travel, which can be 0 | Leaves the position |
| 1048:0a54 | A message's speech file that doesn't open (`TMediaPlayer.Open`) raises an exception in the middle of `post_message` | The TV has no speaker for it (`Fernseher::speaker_missing`) |
| 10a0:30b6 | `play_sound` doesn't check its number: one outside 1..42 reads past its tables | Ignores it |
| 10b8:1e96, 10b8:25cb | The overview map's scroll bars divide by their range, 0 for a map window as big as the map (no layout of `UEBERMAP.DAT` has one) | Puts the thumb at the start |
| People (1088:0324, 1088:2df2, 1088/1090/1098) | Some tables are read past their end, and markers are written outside their layers for a home of -1 or a square off the map ([people.md](people.md#open-questions)) | Reads 0 (or "free"), or skips the write |
| 1070:04d3, 1070:270f | A renovation interval past 5 (from a map) reads the money menu's arrows' tables and `RenovierungToText` past their end | Leaves the interval; shows no text |
| Tables read for an index out of range: the month's length, a player's reputation (`*1110:6f24+0x2f`), player 0's advertising level and loan (1078:057a), kind 3's decay (step 0x52), the books (1070:1419) | Reads or writes the bytes next to the table | Reads 0 (31 for a month), writes nothing |
| 1080:3216 `plan_road` | A bridge dragged from water along a row of water to the map's edge wraps its byte coordinate and loops for ever (the edge and off the map count as flat water), writing owners off the map | Stops after one round of the byte; writes nothing off the map |
| 10b0:069f, 10b0:1907 | Raising or lowering a corner reads and writes past the 41 × 41 working copy where the land would spread further | The spreading stops at the copy's edge |
| 10b0:33bc | Working out the forms reads heights before row 0 or column 0 and writes forms off the map | Reads the layer's neighbouring bytes, writes nothing off the map |
| 1068:27a5, 1078:09f6, 1078:0d00, 1078:2178, 10a8:0987, 10a8:21de | A plot number without a record is used through a nil pointer | Skips that plot's update, or reads 0 |
| 10b0:2b9b | Returns an unset error for a corner not within 1..249 | Returns 0 (callers ignore it) |
| 10b0:53fe | Leaves the reference height and corners unset for a direction past 3 (nothing passes one) | Returns false |
| 1050:245b | Leaves its locals unset for a view rotation past 3 | Returns false |
| 1008:0297, 10a8:173b | Return an unset byte for a scenario past 7, or a levelling state past the known ones | Return false |
| 10a0:03ca | Handing over a sold building reads outside the layer for a square off the map, reads a nil record for a site no record holds, and writes owners outside the layer for a building past the map's edge ([auction.md](auction.md)) | Does nothing, or skips the squares off the map |
| 10b0:4d19 | A river that wanders in circles over land never reaches the sea or fails, and the generator never ends ([island-generator.md](island-generator.md#rivers)) | After 251 × 251 steps the river fails and is taken out |
| 10b0:48d7 | A river's ways on the flat that tie all the way round their row and column (its distance wraps) are looked along for ever | Stops after one round of the distance, the open ways' last winning |
| 10b0:4cb1 | Taking a failed river out finds a river square `reset_ground` leaves alone (one with a building) again and again | Stops there; river squares have no buildings |
| 1010:0414 | With no names in `NAMEN.TXT` the computers' names divide by zero | Their names stay empty |
| 1048:2a19 `load_map` | I/O checks are off: a short or missing file stops the reads at the error, keeps the rest of the old game's globals (a half-loaded game) and leaves the error pending, so later file operations fail silently ([savegame.md](savegame.md#the-port)) | Parses the whole file first and loads nothing from one that doesn't parse |
| 1038:3a13, 1040:1175 | More than 100 saved games are written past the list box's 100 entries | Lists 100 |
| 1018:16c8, 1028:3cb1 | An empty submenu of the build menu selects type -1: its name and figures are read from before the tables, and the build tool builds type 255, past them | Shows none; the tool builds nothing |
| 1018:23f8, 1018:26b9 | A type of kind 7, which has no submenu (`1110:00ea` holds -1), or of a kind past 11 is written into or looked for before or after the build menu's lists | Leaves the lists |
| 1018:1a43 | A row of -1 in a build menu list (a map that saved one) shows the name before the first | Shows none |
| 1048:087f | A message whose picture file is missing (`FILENAME.DAT`'s `Xpiranh`, `Xstrkapu`: messages 3 and 26) raises an exception when the TV shows it | Leaves the picture out |
| 1048:03c5 | A news text without its `*` (or with one right after `&1`, `}1`, `$`) is read on past its end | Stops at the end of the line |
| 1048:0a54, 1048:03c5 | A message outside 0..89 reads its kind and texts past their tables, a player outside 0..8 their name | No TV, no text, no name |
| 1018:2a17 | The start of every game draws the build menu for the pointer tool, whose heading is read from past the 12 | Draws nothing |
| 1038:1899 | The info box's island information reads a plot number without a record through a nil pointer; a building of a kind past 11 reads its kind of square after the table | Shows zeros; the ground |
| 1038:014b, 1038:0c00, 1080:02d9 | The info box's and the overview's price per visit and the overview's occupancy divide by the capacity, which can be 0 | Show 0 |
| 1038:014b | A house, hotel or restaurant without a record reads its stars through a nil pointer | Shows 0 |
| 1038:24d6 | Selling a building without a record schedules the sale at a nil pointer's square | Sells nothing |
| 1080:202b | The overview's key Up with fewer than 12 buildings moves the list's top before its start, and its rows read from before the list | Leaves those rows empty |
| Data files: `HAUSNAME.TXT` (1048:358f), `ANIMDAT2.DAT`, `ANIMDAT3.DAT` (1088:80a7), `NAMEN.TXT` (1010:0414), the high scores | A missing `***` end marker loops for ever; a group or list longer than its room overruns the next memory; a high score file of another size isn't checked | The file fails to load |
| 1040:1bdf `add_highscore` | A won game of scenario 7 (past the six tables) writes its entry 67 bytes past the end of the high score table ([endforms.md](endforms.md)) | Writes nothing |
| 1040:1d49 | Showing the high scores of a game past 6 reads the next ranks' tables and, for the last rank, past the end of the table | Shows the ones in the table, nothing past it |
| 1068:1016 | A fairly won scenario past 6 sets no film, so the film player opens whatever file name it had | Shows the results at once |
| 1068:0288 | A scenario past 7 leaves the end form's reason text as it was | Leaves it empty |

## Simulation logic

| Where | Bug | Kept |
|---|---|---|
| 1078:280b | Removing a building site leaves its location effect on the map ([simulation.md](simulation.md#open-questions)) | yes |
| 1078:280b | Removing a sea bridge adds up a cost that is never charged | yes |
| 1008:0f37 | Every price class's shop price level goes into the same byte (`+0xf`), so the last class wins; it also computes copies of the shops' places by class (1078:0780) and then drops them ([settlement.md](settlement.md#open-questions)) | yes |
| 1060:0637 | Switching disasters off on the options form sets `1110:133c` for good, so the score stays lowered after they're switched on again ([options.md](options.md)) | yes |
| 1008:0907 | Divides by `MaxPreis` where the range seems meant | yes |
| Settlement | Buildings with owner 0 add into the slots next to theirs ([settlement.md](settlement.md#the-months-sums-11104aea)) | yes |
| Settlement step 0x28 (1078:014c, 1078:0172) | The guests of houses with owner 0 are counted in player 8's interest, the `Longint` before the guests | yes |
| Settlement, year end | Only the human pays advertising, insurance, cost of living and renovation; computers pay a sixth of their interest. This may be meant as a handicap, not a bug | yes |
| 1010:129e | A palm that dies stays counted on its plot (`+0x354`) | yes |
| 10a8:21de | Checks the area of the plot the computer is leaving, not the one it moves to | yes |
| 10a8:388b | A category with more than 40 types writes them on into the next row of the computers' lists ([computer.md](computer.md#state)) | yes |
| 1098:0002 | Returns an undefined value; the port keeps the ship's heading | n/a |
| Harbour ships (1098:04f9, from 1098:109f) | The second detour checks the ship's own square instead of the squares ahead ([people.md](people.md#open-questions)) | yes |
| Harbour ships (1098:109f) | A ship with `+0x26` left marks the square ahead without going there | yes |
| 1090:2373 | Works out a stage's offset for its spectators, then looks for them on the stage's own square | yes |
| 1090:2c1f | A plane's second approach is accepted when either coordinate matches, not both | yes |
| 1078:09f6 | A corner rising from the sea between plots merges the first pair by their numbers cut to a byte | yes |
| 1080:3216 | Building any road (the computers' too) turns the "new people" animation option (`1110:0056`) on, whatever the player chose | yes |
| 1078:2396 (computers) | The computers' roads use the ground the human's last road drag started on (`1110:704e`): after a drag from water they look for bridges ([computer.md](computer.md#levelling-and-roads)) | yes |
| 1090:0014, 1090:124f | The tools' pictures go into the people's records from 1 on: for a road drag or a building of more than 35 squares they overwrite real people (square, offset, picture, kind 6) ([player-tools.md](player-tools.md#the-pictures-over-the-island-10900014-1090124f-1090155b)) | no: the view keeps the pictures |
| 10b0:03fa, 10b0:12c7 | The land only follows a moved corner where a neighbour is exactly two steps off, so existing cliffs get steeper | yes |
| 10b0:12c7 | Lowering checks the buildings (and clears palms, adding their cost) of squares it reaches after an error already stopped it | yes |
| 10b0:069f, 10b0:1907 | A palm cleared by moving the land only loses its building: its part, owner, plot count and location effect stay | yes |
| 10b0:069f, 10b0:1907 | For player 1 the money check replaces any earlier error with 0xe | yes |
| 1078:2178 | After a road is built only buildings whose square shows part 0 get their connection again, so houses with another look never do | yes |
| 1038:1a75 | The info box offers its buttons on squares of nobody (owner 0) too: the human renovates, re-prices or sells nobody's buildings and bribes on nobody's sites | yes |
| 1028:1973 | A computer bids when its limit is above the price, though its bid (5 % more) may be above its limit ([auction.md](auction.md#a-step-10281973)) | yes |
| 1028:0c82 | Passing (or the close button) hands the item to a computer at a random raise, or to nobody, even when someone (the player too) leads; after the third call the close button gives the item away again before `Timer1` does | yes |
| 1028:26e4 | Enter bids without checking the player's money, and even after the third call, when the mouse can't | yes |
| 1028:08e7 | Player 8 gets nothing for an item (`make_available` has no case for 8); item 6 gives nothing but its message, and an unknown item nothing at all, for the price | yes |
| 10a8:0167 | A computer's bid overflows when its range (what it can pay or three times the price, less the price) passes about 2 150 000, and wraps | yes |
| 10b0:3c5c | The atoll's small hills stray from the ring by `ATVariation2` but are centred by half of `ATVariation1` | yes |
| 10b0:4506 | A river's 1500th try at a start is taken without looking at its corners, so a river always starts, maybe on the flat or in the sea | yes |
| 10b0:0e0c, 10b0:2053 | The generator's raising and lowering write the working copy back after an error, with the corners moved before it: rivers leave cliffs | yes |
| 10b0:48d7 | A river's distance on the flat is an `Integer` that wraps: ways that tie to the map's edge look on round the other side | yes |
| 10b0:424f | A river square of another ground (sand to grass on the flat, water at a bend) or on a slope that falls another way gets no piece: a gap in the river | yes |
| 1088:868c, 1010:3ab4 | A new game leaves the people records' last bytes, record 0, the planes' unused track rows and the events' 13th value as the heap had them | no: 0 |
| 1048:2a19 (`World::keep_unsaved`) | Not saved, so a loaded game takes them from the game in memory (from the file menu, the game that was running): `RandSeed`, the oil slick (`1110:0088`), the people's `6fcc`, `6fce`, `6ff2`, `7040` and `3c42`, the auction's limits, joined flag and timers, and the computers' road candidates past the file's player count ([savegame.md](savegame.md#what-a-saved-game-loses)) | yes |
| 1068:30d0, 1068:304f | `save_map` writes the building sites from the list's head and `load_map` puts each in front: a reload turns them round | yes |
| 1068:2d49 | Loading builds each plot's lists of houses and facilities backwards from the houses' list, not in the order the game had them | yes |
| 1048:358f | A line shorter than three characters keeps the previous line's bytes, so one after a `***` line also ends a list of `HAUSNAME.TXT` | yes |
| 10a0:1f65 | Piranhas ordered for a building that isn't a pool do nothing, without the error 0x1a the other kinds give: the loop over the pools always ends at 6, and the check is for 5 ([sabotage.md](sabotage.md)) | yes |
| 10a0:1f65, 1028:3079 | A sabotage kind past the nine (only a command can ask for one) reads past `SABOTAGE.TXT`'s table | no: a free kind with no targets; nothing happens |
| 1048:0a54 | When the news archive first wraps, the empty 26th entry moves into the 25th, so the 27th message goes there and the 26th and 27th stay the wrong way round ([fernseher.md](fernseher.md#posting-a-message-10480a54)) | yes |
| 1048:1b3e | While the TV's speaker speaks, its lights draw `Random(5)`, `Random(20)` and `Random(3)` from the world's `RandSeed` every tick, so the game's numbers depend on the speech option and on how long the speech plays | yes: the view's TV tells the world (`Action::Speaker`), which draws them in its tick (`World::tv_lights`) ([sound.md](sound.md#the-tvs-speaker)) |

## UI quirks

The widgets' are described in
[widgets.md](widgets.md#quirks-kept-in-the-port), the start menu's in
[startup.md](startup.md#the-start-menu-tstartform) ("Quirks").

| Where | Quirk | Kept |
|---|---|---|
| 10c0:0f10 | The scroll bars' arrows step only when the mouse is released; nothing auto-repeats | yes |
| 10c0:15f7 | A scroll arrow released over another part of the bar stays drawn pressed | yes |
| 10c0:0fe3 | A scroll bar press calls `Click` whenever it repaints, even when the position didn't change | yes |
| 10c0:0998 | A button's mouse up reports "handled" whenever it was pressed, clicked or not | yes |
| 10c0:1c1b | A click on an edit field while it has focus takes the focus away | yes |
| 10c0:0431 | A list box ignores clicks with Shift held | yes |
| 1030:23d3 | `draw_text` keeps the breaking space on the line and measures it again on the next, doesn't measure a line's last character for alignment, and before the first space puts the whole rest of the string on an overlong line | yes |
| 1058:02cf | Switching pages on the start menu can leave a different scenario ticked than the one that starts | yes |
| 1058:41fb | Letting go of the right mouse button also clicks a button held down with the left | yes |
| 1028:0dde | The auction form is centred by its picture's height, not its own, so it sits 40 pixels low | yes |
| 1028:15ef | A paddle's animation puts the bare background back under it, wiping the bottom rows of the item's picture | yes |
| 1038:3983, 1040:10e5 | The lists of saved games come out in the reverse of the directory's order | yes (the port's stand-in for the order is the names sorted) |
| 1040:073c | Any press while the file menu's name field is focused unfocuses it, so clicking "Speichern" focuses it again instead of saving; only Enter saves a typed name | yes |
| 1040:0192 | A name picked from the file menu's list goes into the field without moving its cursor | yes |
| 1028:1ce0 | After a click on the title bar the title is drawn one row higher than the form opened with | yes |
| 1090:124f, 1080:31ed | The road drag's end only follows the mouse in the clock's tick with the animations and new people on: otherwise the road is one square, and it always ends where the mouse was at the last tick ([player-tools.md](player-tools.md)) | yes |
| 1090:0014 | For a square without a building the demolish tool's picture shows the area of the building it or the people code (`1110:6ff2`) last found | partly: the port's picture remembers its own |
| 1020:1984 | The tools only act on exactly `[ssLeft]`: with Shift or Ctrl held, or on the second press of a double click, a click does nothing | yes |
| 1010:2be1 | Picking any tool closes the build menu, with its sound, even when it's closed (its `Visible` is always set) | yes |
| 1010:2c8b | The road's button selects building type 1 for the build tools, until a submenu selects another | yes |
| 1018:3290 | With the build menu focused only F1–F12, 1–9 and Pause reach the main screen: the arrow keys and 0 do nothing | yes |
| 1060:2404 | The options form takes a press of any mouse button, so the right button ticks its boxes too | yes |
| 1070:2b3a | The money menu opens two pixels short of its menu bar's bottom (the first rectangle's bottom − 1); after a drag it's the bottom + 1 ([geldmenu.md](geldmenu.md#opening-and-closing)) | yes |
| 1070:2e6c | The money menu is placed one column further right than its width allows, so its last column is off the screen | yes |
| 1070:0820 | Any mouse button drags the money menu by its title bar, and rolls it up meanwhile | yes |
| 1070:39bf | With the money menu focused, `1`–`6` pick its pages, so `5` (its button's key) and `1`–`4`, `6` don't reach the button bar | yes |
| 1008:4f64 | The year's end shows the money menu without pressing the button bar's button 4 | yes |
| 1070:151b | A chart line drawn from right to left along its longer axis gets only its first dot (the charts never draw one) | yes |
| 10b8:37f5, 10b8:39b1 | A click near the overview map's edges puts the main screen's scroll position past its scroll bars' ends (`wo` from -10 to 490, `ns` from -13 to 489), with the thumbs drawn past the arrows ([overview-map.md](overview-map.md)) | yes |
| 10b8:2785 | The overview map's horizontal thumb travels the trough's full width, so near the right end it covers 4 pixels of the right arrow, which taking it off leaves behind | yes |
| 10b8:24a1, 10b8:2e99 | Letting go of the overview map's scroll bar or menu turns the "new people" option on whatever the player chose (its arrows turn it off while held) | yes |
| 10b8:04b1, 10b8:07df, 10b8:0a0a | On the overview map, grass below height 3, sand from height 6 and buildings of owner 0 (whose colour comes from the never-written `1110:0073`) get a colour from outside `Ueberma2.BMP` | the port draws nothing there, leaving the sea's tile or the square drawn before |
| 10b8:3d2b | The overview map's close button is drawn back from the small title bar's bitmap in either size (the same pixels) | yes |
| 10b8:1684, 1030:185d | The overview map's redraw after a location change covers its radius in squares, though the radius is in cells of two squares | no: the port redraws every changed cell ([overview-map.md](overview-map.md)) |
| 1038:34fc | The compass's drag isn't let go on the mouse up, so a left drag that starts elsewhere and comes over it moves it | yes |
| 1028:2f4e, 1028:394c | The sabotage window's Sabotieren only hides it: the tool was already the sabotage tool, and clicks on the island sabotage with the picked kind while the window is open too. It sets `1110:54fa` (saved), which nothing reads | yes |
| 1028:2ecc, 1028:2f4e | The sabotage window's drag state stays after the mouse comes up, until the next press in it | yes |
| 1030:108b | The prices with decimals (the info box's and the overview's price per visit) put the thousands marks one place off: "1234,50", "12.34567,89" | yes |
| 1038:014b | Off the map the info box doesn't look the building up, so after a building site it names the site's building instead of the ocean | yes |
| 1080:02d9 | The overview clears the place of the selected building's price with the background from row 0x43, not its own | yes |
| 1080:1c3b | The overview's columns 299 and 300 belong to the list and the scroll bar | yes |
| 1080:202b | The overview's keys Up and Down draw the kind from the list's top first, then Up puts the selection in the top row (when it isn't the first) and Down in the bottom row past row 10 | yes |
| 1038:24d6 | After a button's command the info box is drawn again but keeps its size | yes |
| 1048:1794 | The TV's button that went down is never cleared: a press elsewhere and a release on that button still clicks it | yes |
| 1048:1794, 1048:26f6 | The TV's first button scrolls back to the view origin saved with the news, which 1020:0787 moves 10 rows on, so the view doesn't come back where it was | yes |
| 1048:1b3e | A TV the news opened closes 6 s after it opened if its page has no film, even while the player browses | yes |
| 1048:12e9 | The TV's title is centred over 336 pixels from the bar's left edge, not over the bar after the close button | yes |
| 1020:38d4 | The quit question is kept on the screen by its 16-pixel title bar while dragged, so it can be dropped with its bottom off the screen | yes |
| 1068:01df | "Beenden" on the end form quits without saving `Last.map`, unlike the quit question's "OK" | yes |
| 1068:0288, 1068:1016 | A cheater who wins sees "not reached", the cheating text, the losing film and music, and no "Weiterspielen" | yes |
| 1040:1d49 | The tenth rank of the high scores shows "..." instead of 10 | yes |
| 1040:1ca7 | "Löschen" clears the shown high score table without asking | yes |
| 1040:31bc | The high scores' check boxes are pressed as radio buttons: a ticked one ignores a click | yes |
| 1040:3dc7 | The tooltip's timeout (`1110:009c`) is set to 10 and never read | yes |

## Sound

[sound.md](sound.md) has the details.

| Where | Quirk | Kept |
|---|---|---|
| 10a0:2824 | The auctioneer's three calls (`zum1.wav`..`zum3.wav`, sounds 0x1a–0x1c) are looked for in `SOUND\`, but they're in `SPRADEUn\`: WaveMix gets null waves and they're never heard | yes |
| 1090:5293, 10a0:30b6 | A sound's time (`1110:723c`) only counts down in `people_tick`, which runs with the "animations" option on: with it off, each sound with a time plays once and then never again (until "Neues Spiel") | yes |
| 1018:37de, 1018:367e | The playlist starts at track 0 while the intro's `holiday.mid` plays, so the first track after it is the second (`dub.mid`), and `anotrian.mid` only comes after the whole list | yes |
| 1018:367e | A track's end is only noticed when the next day begins and the position hasn't moved for a whole day: up to a day of silence between tracks, and none changes while the game is paused | yes |
| 1010:26fa | Loading a game from the file menu with the music on opens its file again without playing it: silence until a day or two later the next track starts | yes |
| 1048:0a54 | A message without speech only forgets a speech still open from the last one: it plays on, and the effects stay off, until the TV closes | yes |
| 1048:1b3e | The speech starts by the page on the TV, not the message it belongs to: browsing with the keys to a page without speech in the first 3 s keeps it from starting, and the TV then never closes by itself | yes |
| 1048:01bd, 1048:077a | A news film gives up WaveMix's wave device (10a0:2518) and only closing the TV takes it back, and only with "Geräusche" on: switching the sounds on later leaves the effects silent until something else takes it back | yes |

## Design issues for multiplayer

These aren't bugs for a single player, but the original mixes the human's UI
state into the simulation. See the open questions in
[simulation.md](simulation.md#open-questions) and [people.md](people.md):
- the TV's speaker: one player's machine and options decide how long it
  reads (the WAV's length, "Sprachausgabe"), and meanwhile the auction
  doesn't open and the TV's lights take numbers from the world's
  `RandSeed` every tick (`Action::Speaker`, [sound.md](sound.md#the-tvs-speaker));
  for one player the port does as the original. For multiplayer the lights
  would draw from a view's numbers and the auction wouldn't wait;
- the TV pause: a message the human's options pick, or the TV's button,
  stops the game for everyone until the TV closes, and the archive keeps
  the human's view origin. The news options are each player's view
  setting (`optionen::Settings::news`, saved with the view); for one
  player the port pauses as the original does. Intended for multiplayer,
  not built yet: news never pause the game, since each player can dismiss
  their own TV ([fernseher.md](fernseher.md#multiplayer));
- the animation options;
- the human's view, used when people are placed;
- the road drag state (`1110:634c`, `704e`), which the computers read
  ([player-tools.md](player-tools.md#world-and-view));
- the build menu's lists, player 1's;
- the money menu: the books, its balance, the insurance (`1110:0072`) and
  whether it's shown (which decides when the year's total is worked
  out, `World::money_menu_shown`) are player 1's
  ([geldmenu.md](geldmenu.md#world-and-view));
- the overview map's scroll arrows, which switch the "new people" option
  (`1110:0056`, the world's) off while held and on when let go: in the
  port `Action::SetOption(WorldOption::PeopleSpawn, ..)`
  ([overview-map.md](overview-map.md));
- sabotage: only player 1's money is checked, and only player 1 gets the
  books and the rising money (10a0:1f65, 10a0:1d5e); the computers never
  sabotage;
- the info box with buttons, which disables the other forms (not the
  clock) while it's open, and its and the overview's buttons, which are
  for player 1's squares ([infobox.md](infobox.md));
- the auction form, which stops the simulation while one human bids and
  waits for them to start it ([auction.md](auction.md#open-questions)), and
  whose held buttons stop the computers' bids;
- the pause (`TPauseform`) and the options (`TOptionenform`), which stop
  the clock (1020:24a0) for the one player ([options.md](options.md));
- the options' speed, animations, "Auktion wartet" and disasters, one
  player's settings that change everyone's world;
- the quit question (`TEndeform`), which stops the clock while one human
  is asked ([endforms.md](endforms.md));
- the end of a game (`TSchlussform`), which stops the clock for everyone
  until the one human picks a button, shows player 1's results, and enters
  only player 1's score in the high scores.
