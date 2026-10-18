# Bugs in the original game

The port reproduces the original's behaviour, bugs included, so that it
plays and computes the same. Each bug can be fixed behind an
enhancement, a switch that is on by default; with every one off the port
is the original. This file lists every bug and quirk found so far, so
that none gets lost.

Each place in the code that keeps one is marked with a `// BUG(original):`
comment, so `grep -rn "BUG(original)" crates` finds them all. Where an
enhancement fixes it, the code branches on the enhancement and the
original's path stays there, still marked. Add a row here whenever you
keep a bug or work around one. Where another doc explains a bug in
detail, the row links there instead of repeating it.

The tables' last column ("Kept") says what the port does about it: `yes`
for kept with no fix yet, or the key of the enhancement that fixes it
(kept with that enhancement off). The crashes' table says what the port
does instead ("Port today").

## Options

Built in [`game::enhance`](../crates/game/src/enhance.rs):

- **World fixes** (`enhance::Fix`, a set is `enhance::Fixes`) change the
  simulation. They are the world's (`World::fixes`): every player has the
  same, they are in the world's hash (`World` derives `Hash`), and
  `Action::SetFix` changes one in the middle of a game, through
  `World::apply` like any command. A saved game keeps them in the port's
  chunk after the original's fields, which the original doesn't read
  ([savegame.md](savegame.md#the-ports-chunk)). A new game, a scenario
  and a game from a file without the chunk (any original file) get the
  player's defaults; loading a file without the chunk from the file
  menu keeps the running game's. `World::default()` and `World::from_map`
  of an original file have none, so the tests are in original mode unless
  they ask for a fix. A new game's world is set up with them in place
  (`hintergrund::new_world` takes them), since some change the island
  generator; a scenario's and a loaded game's get them as they're loaded
  (`World::set_loaded_fixes`, as `fix.load_order` changes what loading
  does). The fixes are keyed in "Simulation logic" below.
- **View enhancements** (`enhance::Enhancement`, a set is
  `enhance::Enhancements`) change one player's screen, sound or input.
  They are that player's settings: `window_stacking`
  ([layout.md](layout.md#stacking)), `big_map`
  ([layout.md](layout.md#big_map-enhancement)), `no_main_title`
  ([layout.md](layout.md#no_main_title-enhancement)), `mouse_wheel` (the
  wheel scrolls the island, Shift across, the overview map and the
  lists; [map.md](map.md#the-scroll-bars)) and the fixes keyed in
  "UI quirks" and "Sound" below. Each form keeps the player's set in its
  `enhance` field (`Game::set_view_enhancements` hands it out; the start
  menu, the load form and the sound get it from `Holiday`), and the
  code branches on it where the quirk is. Some of them change what the
  player's view tells the world, as any input does, never how the world
  reacts: `road_drag` (where `Action::RoadDragEnd` ends), `tv_form` and
  `tv_sound` (when the TV sends `Action::TvPause` and `Action::Speaker`),
  `overview_scrolling` (the viewpoint the people code reads).
- Each has a stable key, a short German label (for the in-game list),
  a one-line description and is on by default.
  `enhance::Config::resolve` works them out: everything on, or off with
  the master switch, then each per-key override in order. The player
  sets them in the game's list of them (below), in the port's
  `settings.ini` (`platform::settings::EnhanceSettings`, by hand or
  written by that list; the port keeps the other lines when it writes
  the window's state or the switches) and on the command line:

  ```text
  original=true                 # settings.ini: everything off
  enhance.window_stacking=off   # settings.ini: one off (on/off, 1/0, true/false, yes/no)
  --original                    # this run: everything off, settings.ini's switches ignored
  --enhance=window_stacking=on  # this run: one on or off, over both
  --enhancements                # lists them with their state and exits
  ```

  An unknown key in `settings.ini` is reported and skipped; on the
  command line it stops the program with the list.
- Adding one: a line in `Fix`'s or `Enhancement`'s list in
  `game::enhance`, a branch on it where the bug is kept (`world.fixes.has(..)`
  or the view's set), with the original's path marked `// BUG(original):`,
  and its key in the bug's row below. Tests construct either mode
  (`Config::ORIGINAL`, `Config::default()`); the existing tests use
  `Config::ORIGINAL`.
- Crashes and hangs have no "original" mode, because the port can't
  crash. It does the fallback listed below. The option there would be
  a better fallback, where one makes sense.
- **In the game**: the options' "INFO" (or Space) opens the list of
  them (`game::enhanceform`, [options.md](options.md#the-list-of-enhancements-port-extension)),
  itself a view enhancement, `enhancements_screen`, so with everything
  off (`--original`) "INFO" opens the credits (`TCreditform`,
  `game::creditform`) as in the original; the list has a button for
  them. Two lists of check boxes, the world's fixes and the view's, each
  key's German label, scrolled by their bars; "Alle an", "Alle aus
  (Original)". A fix goes to the running game as `Action::SetFix`
  through `World::apply` and becomes the player's default for new games;
  a view enhancement applies at once (the layout too, for `big_map` and
  `desktop_background`). Each change is saved in `settings.ini`
  (`enhance.<key>=on|off`; "Alle an"/"Alle aus" write `original=` and
  drop the keys' lines). With `--original` or `--enhance=` the changes
  apply to this run and are saved all the same; the screen says that
  the next start's command line goes over the file again. Switching off
  `enhancements_screen` itself leaves the list open, and it says how to
  get it back.

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
| 10a0:074d | The oil slick's random walk counts off the map as flat sea, so a slick by the map's edge goes off it, and its oil's markers (1088:0324) and their clearing at the end (10a0:0915) are written outside the marker layer | Writes no marker off the map ([simulation.md](simulation.md#events-10103084)) |
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
| 10c8:1810 from 1000:2466, 1048:01bd, 1068:1016, 1068:0e68 | A film that doesn't open (a missing or broken file) raises an exception out of the handler; the TV's then never closes by itself ([films.md](films.md)) | Ends the film at once |
| 1068:0288 | A scenario past 7 leaves the end form's reason text as it was | Leaves it empty |

## Simulation logic

| Where | Bug | Kept |
|---|---|---|
| 1078:280b | Removing a building site leaves its location effect on the map ([simulation.md](simulation.md#open-questions)) | `site_effect` |
| 1078:280b | Removing a sea bridge adds up a cost that is never charged | `bridge_demolition_cost`: `Strassenabrisskosten` a square, checked and booked as for a road |
| 1008:0f37 | Every price class's shop price level goes into the same byte (`+0xf`), so the last class wins; it also computes copies of the shops' places by class (1078:0780) and then drops them ([settlement.md](settlement.md#open-questions)) | yes; not fixed: what was meant is unclear. The "level" by price class comes out as the price class itself (the places times their class over the places), and writing it to `+0x10..`, which the classes' quality reads (0 today), would change every plot's ratings |
| 1060:0637 | Switching disasters off on the options form sets `1110:133c` for good, so the score stays lowered after they're switched on again ([options.md](options.md)) | yes; not fixed: looks meant, a mark that the game was played without disasters for a while (easy sets it from the start), so switching them off through a risky stretch costs score |
| 1008:0907 | Divides by `MaxPreis` where the range seems meant | `price_class_range`: the tenth of `MinPreis`..`MaxPreis`, as for the rents |
| Settlement | Buildings with owner 0 add into the slots next to theirs ([settlement.md](settlement.md#the-months-sums-11104aea)) | `ownerless_buildings`: into nothing |
| Settlement step 0x28 (1078:014c, 1078:0172) | The guests of houses with owner 0 are counted in player 8's interest, the `Longint` before the guests | `ownerless_buildings` |
| Settlement, year end | Only the human pays advertising, insurance, cost of living and renovation; computers pay a sixth of their interest. This may be meant as a handicap, not a bug | yes; not fixed: most likely the computers' handicap (the code marks it as player 1's), and making them pay would change the balance a lot |
| 1010:129e | A palm that dies stays counted on its plot (`+0x354`) | `palm_removal` |
| 10a8:21de | Checks the area of the plot the computer is leaving, not the one it moves to | `computer_area_change` |
| 10a8:388b | A category with more than 40 types writes them on into the next row of the computers' lists ([computer.md](computer.md#state)) | `computer_type_lists`: stops at 40 |
| 1098:0002 | Returns an undefined value; the port keeps the ship's heading | n/a |
| Harbour ships (1098:04f9, from 1098:109f) | The second detour checks the ship's own square instead of the squares ahead ([people.md](people.md#open-questions)) | `ship_detour`: its own square and the four ahead |
| Harbour ships (1098:109f) | A ship with `+0x26` left marks the square ahead without going there | yes; not fixed: what was meant is unclear. The ship does get there (`sail`, at its step's end), so the marks only run a square ahead of it, like claiming the square |
| 1090:2373 | Works out a stage's offset for its spectators, then looks for them on the stage's own square | `stage_spectators` |
| 1090:2c1f | A plane's second approach is accepted when either coordinate matches, not both | `plane_approach` |
| 1078:09f6 | A corner rising from the sea between plots merges the first pair by their numbers cut to a byte | `plot_merge_numbers` |
| 1080:3216 | Building any road (the computers' too) turns the "new people" animation option (`1110:0056`) on, whatever the player chose | yes; not fixed: not a bug. `1110:0056` isn't a player's option (the options form has none) but a flag that `plan_road`, `draw_columns` (1050:0e68) and the overview map's scroll timers clear while they're busy and set when done, so no people spawn meanwhile; the road only sets it back |
| 1078:2396 (computers) | The computers' roads use the ground the human's last road drag started on (`1110:704e`): after a drag from water they look for bridges ([computer.md](computer.md#levelling-and-roads)) | `computer_road_ground`: the ground of their start square |
| 1090:0014, 1090:124f | The tools' pictures go into the people's records from 1 on: for a road drag or a building of more than 35 squares they overwrite real people (square, offset, picture, kind 6) ([player-tools.md](player-tools.md#the-pictures-over-the-island-10900014-1090124f-1090155b)) | no: the view keeps the pictures |
| 10b0:03fa, 10b0:12c7 | The land only follows a moved corner where a neighbour is exactly two steps off, so existing cliffs get steeper | `levelling_cliffs`: two or more |
| 10b0:12c7 | Lowering checks the buildings (and clears palms, adding their cost) of squares it reaches after an error already stopped it | `levelling_errors` |
| 10b0:069f, 10b0:1907 | A palm cleared by moving the land only loses its building: its part, owner, plot count and location effect stay | `palm_removal` |
| 10b0:069f, 10b0:1907 | For player 1 the money check replaces any earlier error with 0xe | `levelling_errors` |
| 1078:2178 | After a road is built only buildings whose square shows part 0 get their connection again, so houses with another look never do | `road_connections`: the first square of any look |
| 1038:1a75 | The info box offers its buttons on squares of nobody (owner 0) too: the human renovates, re-prices or sells nobody's buildings and bribes on nobody's sites | `nobodys_buildings`: the buttons, and the commands, for the player's own only |
| 1028:1973 | A computer bids when its limit is above the price, though its bid (5 % more) may be above its limit ([auction.md](auction.md#a-step-10281973)) | `auction_computer_limit` |
| 1028:0c82 | Passing (or the close button) hands the item to a computer at a random raise, or to nobody, even when someone (the player too) leads; after the third call the close button gives the item away again before `Timer1` does | `auction_pass`: a leading bid stands when no computer takes the item; nothing once it's sold |
| 1028:26e4 | Enter bids without checking the player's money, and even after the third call, when the mouse can't | `auction_enter_bid` |
| 1028:08e7 | Player 8 gets nothing for an item (`make_available` has no case for 8); item 6 gives nothing but its message, and an unknown item nothing at all, for the price | `prize_player8` for player 8; item 6 and unknown items not fixed: nothing says what they should give |
| 10a8:0167 | A computer's bid overflows when its range (what it can pay or three times the price, less the price) passes about 2 150 000, and wraps | `computer_bid_overflow` |
| 10b0:3c5c | The atoll's small hills stray from the ring by `ATVariation2` but are centred by half of `ATVariation1` | `atoll_hills` |
| 10b0:4506 | A river's 1500th try at a start is taken without looking at its corners, so a river always starts, maybe on the flat or in the sea | `river_start`: no river then |
| 10b0:0e0c, 10b0:2053 | The generator's raising and lowering write the working copy back after an error, with the corners moved before it: rivers leave cliffs | `generator_levelling` |
| 10b0:48d7 | A river's distance on the flat is an `Integer` that wraps: ways that tie to the map's edge look on round the other side | `river_flat_way`: the last way still open at the edge |
| 10b0:424f | A river square of another ground (sand to grass on the flat, water at a bend) or on a slope that falls another way gets no piece: a gap in the river | yes; not fixed: `FLUSSPOS.DAT` has no pieces for those grounds and slopes, so nothing says what should go there |
| 1088:868c, 1010:3ab4 | A new game leaves the people records' last bytes, record 0, the planes' unused track rows and the events' 13th value as the heap had them | no: 0 |
| 1048:2a19 (`World::keep_unsaved`) | Not saved, so a loaded game takes them from the game in memory (from the file menu, the game that was running): `RandSeed`, the oil slick (`1110:0088`), the people's `6fcc`, `6fce`, `6ff2`, `7040` and `3c42`, the auction's limits, joined flag and timers, and the computers' road candidates past the file's player count ([savegame.md](savegame.md#what-a-saved-game-loses)) | yes; not fixed yet: a fix means carrying that state in the port's chunk, a change of the saved file rather than of the simulation |
| 1068:30d0, 1068:304f | `save_map` writes the building sites from the list's head and `load_map` puts each in front: a reload turns them round | `load_order` |
| 1068:2d49 | Loading builds each plot's lists of houses and facilities backwards from the houses' list, not in the order the game had them | `load_order` |
| 1048:358f | A line shorter than three characters keeps the previous line's bytes, so one after a `***` line also ends a list of `HAUSNAME.TXT` | yes; not fixed: the data files are read before there's a world with fixes, and it changes nothing for the CD's files |
| 10a0:1f65 | Piranhas ordered for a building that isn't a pool do nothing, without the error 0x1a the other kinds give: the loop over the pools always ends at 6, and the check is for 5 ([sabotage.md](sabotage.md)) | `sabotage_pool_error` |
| 10a0:1f65, 1028:3079 | A sabotage kind past the nine (only a command can ask for one) reads past `SABOTAGE.TXT`'s table | no: a free kind with no targets; nothing happens |
| 1048:0a54 | When the news archive first wraps, the empty 26th entry moves into the 25th, so the 27th message goes there and the 26th and 27th stay the wrong way round ([fernseher.md](fernseher.md#posting-a-message-10480a54)) | `news_archive_order` |
| 1048:1b3e | While the TV's speaker speaks, its lights draw `Random(5)`, `Random(20)` and `Random(3)` from the world's `RandSeed` every tick, so the game's numbers depend on the speech option and on how long the speech plays | `tv_lights`: the view's TV draws its own. Kept, the view's TV tells the world (`Action::Speaker`), which draws them in its tick (`World::tv_lights`) ([sound.md](sound.md#the-tvs-speaker)) |

## UI quirks

The widgets' are described in
[widgets.md](widgets.md#quirks-kept-in-the-port), the start menu's in
[startup.md](startup.md#the-start-menu-tstartform) ("Quirks").

| Where | Quirk | Kept |
|---|---|---|
| 10c0:0f10 | The scroll bars' arrows step only when the mouse is released; nothing auto-repeats | `scroll_bars`: they step when pressed, and repeat while held (after 400 ms, every 50 ms, while the mouse is on them; `ScrollBar::repeat`, from the forms' timers) |
| 10c0:15f7 | A scroll arrow released over another part of the bar stays drawn pressed | `scroll_bars` |
| 10c0:0fe3 | A scroll bar press calls `Click` whenever it repaints, even when the position didn't change | `scroll_bars`: only for a change (the release too) |
| 1020:0d92, 1020:151a | The main screen's scroll thumbs are 20 pixels long, but only their first 16 grab them: a press on the last four pages forward ([map.md](map.md#the-scroll-bars)) | `scroll_bars`: all 20 grab |
| 1020:0fa2, 1020:172a | Dragging a main screen's scroll thumb moves the bar by each mouse move scaled by the bar's length less 48 (the thumb travels the length less 51), adds half the divisor also to a move back (so it rounds towards zero) and ignores moves over the arrows: the thumb creeps away from the mouse, faster going back | `scroll_bars`: the thumb goes where the mouse holds it |
| 1020:1e9b | A main screen's scroll arrow or trough held steps again in every clock tick from the first after the press (a click of up to 100 ms can step twice), wherever the mouse has gone: holding the trough pages on past the mouse to the bar's end | `scroll_bars`: from the third tick, and only while the mouse is on the part held, so paging stops at the mouse |
| 1020:0d92, 1020:151a | The main screen's scroll bars take only exactly `[ssLeft]`: with Shift or Ctrl held, or on a double click's second press, a press does nothing, so quick clicks on an arrow step every other time | `modifier_clicks` |
| 1020:0d92, 1020:151a | A press at 16, the first pixel past the 16-pixel back arrow, counts as the arrow (`< 0x11`) | yes; not fixed: one pixel, under the thumb's first only at the bar's start |
| 10c0:0998 | A button's mouse up reports "handled" whenever it was pressed, clicked or not | yes; not fixed: nothing shows it, since only the pressed button takes the release |
| 10c0:1c1b | A click on an edit field while it has focus takes the focus away | `edit_field` |
| 10c0:0431 | A list box ignores clicks with Shift held | `modifier_clicks` |
| 1030:23d3 | `draw_text` keeps the breaking space on the line and measures it again on the next, doesn't measure a line's last character for alignment, and before the first space puts the whole rest of the string on an overlong line | yes; not fixed: it's how every text in the game looks, and the texts were fitted to their boxes with it; a fix would rewrap all of them |
| 1058:02cf | Switching pages on the start menu can leave a different scenario ticked than the one that starts | `start_menu_scenario`: the scenario page ticks scenario 1, which starts |
| 1058:41fb | Letting go of the right mouse button also clicks a button held down with the left | `mouse_buttons` |
| 1028:0dde | The auction form is centred by its picture's height, not its own, so it sits 40 pixels low | `form_positions` |
| 1028:15ef | A paddle's animation puts the bare background back under it (rows 167–234, below the item's picture, which ends at row 166), so a paddle of colour 7 or 8 cuts the bottom rows off the auctioneer's waving hand (282, 149) until its next frame | yes; not fixed: a few pixels for a moment, and only while the hand waves |
| 1038:3983, 1040:10e5 | The lists of saved games come out in the reverse of the directory's order | `saved_games_order`: the directory's order, which the port's stand-in has sorted by name |
| 1040:073c | Any press while the file menu's name field is focused unfocuses it, so clicking "Speichern" focuses it again instead of saving; only Enter saves a typed name | `edit_field`: a press on a button leaves the field focused, so "Speichern" saves |
| 1040:0192 | A name picked from the file menu's list goes into the field without moving its cursor | `edit_field`: the cursor goes after it |
| 1028:1ce0 | After a click on the title bar the title is drawn one row higher than the form opened with | `auction_title` |
| 1090:124f, 1080:31ed | The road drag's end only follows the mouse in the clock's tick with the animations and new people on: otherwise the road is one square, and it always ends where the mouse was at the last tick ([player-tools.md](player-tools.md)) | `road_drag`: the road ends where the mouse is let go (the picture still needs the animations) |
| 1090:0014 | For a square without a building the demolish tool's picture shows the area of the building it or the people code (`1110:6ff2`) last found | partly: the port's picture remembers its own |
| 1020:1984 | The tools only act on exactly `[ssLeft]`: with Shift or Ctrl held, or on the second press of a double click, a click does nothing | `modifier_clicks` |
| 1010:2be1 | Picking any tool closes the build menu, with its sound, even when it's closed (its `Visible` is always set) | `build_menu`: only when it's shown |
| 1010:2c8b | The road's button selects building type 1 for the build tools, until a submenu selects another | yes; not fixed: nothing reads it before a submenu sets its own |
| 1018:3290 | With the build menu focused only F1–F12, 1–9 and Pause reach the main screen: the arrow keys and 0 do nothing | `build_menu`: every key it doesn't use (Left, Right, Escape, Enter, Space) |
| 1060:2404 | The options form takes a press of any mouse button, so the right button ticks its boxes too | `mouse_buttons` |
| 1030:3ec4 | The credits (`TCreditform`) take a press of any mouse button, so the right button presses and clicks their close button | `mouse_buttons` |
| 1030:3c42 | The credits make a "Schließen" button (at 248, 452) but never show it, so it takes no click: only the close button in the corner closes them | yes; not fixed: the picture's last line of names lies under it, and the corner closes them |
| 1070:2b3a | The money menu opens two pixels short of its menu bar's bottom (the first rectangle's bottom − 1); after a drag it's the bottom + 1 ([geldmenu.md](geldmenu.md#opening-and-closing)) | `form_positions`: the bottom + 1 |
| 1070:2e6c | The money menu is placed one column further right than its width allows, so its last column is off the screen | `form_positions` |
| 1070:0820 | Any mouse button drags the money menu by its title bar, and rolls it up meanwhile | `mouse_buttons`: the left one |
| 1070:39bf | With the money menu focused, `1`–`6` pick its pages, so `5` (its button's key) and `1`–`4`, `6` don't reach the button bar | yes; not fixed: the pages' keys look meant, as the form's own shortcuts while it has the focus |
| 1008:4f64 | The year's end shows the money menu without pressing the button bar's button 4 | `money_menu_button` |
| 1070:151b | A chart line drawn from right to left along its longer axis gets only its first dot (the charts never draw one) | yes; not fixed: never seen |
| 10b8:37f5, 10b8:39b1 | A click near the overview map's edges puts the main screen's scroll position past its scroll bars' ends (`wo` from -10 to 490, `ns` from -13 to 489), with the thumbs drawn past the arrows ([overview-map.md](overview-map.md)) | `overview_scrolling`: kept within the ends (`Hauptschirm::set_scroll`) |
| 10b8:2785 | The overview map's horizontal thumb travels the trough's full width, so near the right end it covers 4 pixels of the right arrow, which taking it off leaves behind | `overview_scrolling`: 4 pixels less |
| 10b8:24a1, 10b8:2e99 | Letting go of the overview map's scroll bar or menu turns the "new people" option on whatever the player chose (its arrows turn it off while held) | yes; not fixed: not a bug, as for 1080:3216 above: `1110:0056` is the busy flag that stops people spawning while the map scrolls, not a player's option (and it's the world's) |
| 10b8:04b1, 10b8:07df, 10b8:0a0a | On the overview map, grass below height 3, sand from height 6 and buildings of owner 0 (whose colour comes from the never-written `1110:0073`) get a colour from outside `Ueberma2.BMP` | the port draws nothing there, leaving the sea's tile or the square drawn before; `overview_colours`: the lowest grass's and highest sand's colours, and nobody's buildings as the land |
| 10b8:3d2b | The overview map's close button is drawn back from the small title bar's bitmap in either size (the same pixels) | yes; not fixed: the pixels are the same |
| 10b8:1684, 1030:185d | The overview map's redraw after a location change covers its radius in squares, though the radius is in cells of two squares | no: the port redraws every changed cell ([overview-map.md](overview-map.md)) |
| 1038:34fc | The compass's drag isn't let go on the mouse up, so a left drag that starts elsewhere and comes over it moves it | `form_drags` |
| 1028:2f4e, 1028:394c | The sabotage window's Sabotieren only hides it: the tool was already the sabotage tool, and clicks on the island sabotage with the picked kind while the window is open too. It sets `1110:54fa` (saved), which nothing reads | yes; not fixed: works as a picker either way; what `1110:54fa` was for is unclear |
| 1028:2ecc, 1028:2f4e | The sabotage window's drag state stays after the mouse comes up, until the next press in it | `form_drags` |
| 1030:108b | The prices with decimals (the info box's and the overview's price per visit) put the thousands marks one place off: "1234,50", "12.34567,89" | `infobox_text`: "1.234,50" |
| 1038:014b | Off the map the info box doesn't look the building up, so after a building site it names the site's building instead of the ocean | `infobox_text` |
| 1080:02d9 | The overview clears the place of the selected building's price with the background from row 0x43, not its own | `overview_list` |
| 1080:1c3b | The overview's columns 299 and 300 belong to the list and the scroll bar | `overview_list`: to the bar |
| 1080:202b | The overview's keys Up and Down draw the kind from the list's top first, then Up puts the selection in the top row (when it isn't the first) and Down in the bottom row past row 10 | `overview_list`: the list scrolls only as far as the selection needs |
| 1038:24d6 | After a button's command the info box is drawn again but keeps its size | `infobox_size` |
| 1048:1794 | The TV's button that went down is never cleared: a press elsewhere and a release on that button still clicks it | `tv_form` |
| 1048:1794, 1048:26f6 | The TV's first button scrolls back to the view origin saved with the news, which 1020:0787 moves 10 rows on, so the view doesn't come back where it was | `tv_form` |
| 1048:1b3e | A TV the news opened closes 6 s after it opened if its page has no film, even while the player browses | `tv_form`: once browsed it stays until closed |
| 1048:12e9 | The TV's title is centred over 336 pixels from the bar's left edge, not over the bar after the close button | `tv_form` |
| 1020:38d4 | The quit question is kept on the screen by its 16-pixel title bar while dragged, so it can be dropped with its bottom off the screen | `form_drags` |
| 1068:01df | "Beenden" on the end form quits without saving `Last.map`, unlike the quit question's "OK" | yes; not fixed: the intent is unclear, as a finished game may well not be meant to come back as the last game |
| 1068:0288, 1068:1016 | A cheater who wins sees "not reached", the cheating text, the losing film and music, and no "Weiterspielen" | yes; not fixed: design, cheating is punished |
| 1040:1d49 | The tenth rank of the high scores shows "..." instead of 10 | yes; not fixed: looks meant, the "..." is a string of its own (1040:1d45) |
| 1040:1ca7 | "Löschen" clears the shown high score table without asking | yes; not fixed: design; a question would be a new dialog |
| 1040:31bc | The high scores' check boxes are pressed as radio buttons: a ticked one ignores a click | yes; not fixed: design, one table is shown at a time |
| 1040:3dc7 | The tooltip's timeout (`1110:009c`) is set to 10 and never read | yes; not fixed: nothing to see, and what it was meant to time is unclear |
| 1048:01bd | The TV's `Panel1` is 332 × 256 and its films 335 × 257: their right 3 columns and bottom row are never seen ([films.md](films.md)) | yes; not fixed: the DFM's layout, and the edges hardly show anything |
| 1000:3938 | A key on the background form sets its timer's interval to 1 in any state, so besides ending the intro's film it cuts the waits for the title and the start menu short | yes; not fixed: it works as a skip, which a player pressing a key wants |
| 1000:2466 (state 110) | A desktop up to 20 pixels smaller than 640×480, 800×600, 1024×768, 1152×870 or 1280×1024 is laid out as that size, so what sits at the right or bottom edge (the money menu, the compass, the load form) and the centred forms' right and bottom are cut off by up to 20 pixels ([layout.md](layout.md#the-screen-size)) | `desktop_background`: laid out for the size it is |
| 1000:2466 (states 430, 450, 1130) | Any other desktop size has no title and no background picture: the intro's title and the desktop around the main screen stay black ([layout.md](layout.md#the-screen-size)) | `desktop_background`: the pictures of the size it would snap to, else the biggest that fits, centred |
| 1020:1bd9 | The main screen's title bar moves the form on any left drag over it, also one that started off every form, by the last grab ([layout.md](layout.md#the-main-screen)) | `form_drags` |
| DFMs, Windows | The `fsStayOnTop` forms (TV, compass, info box, auction, pause, options, quit question, high scores, file menu, tooltip) always lie over the others, wherever those are clicked: the end form can lie under the TV and compass, and the TV over the island can't be put behind the main screen ([layout.md](layout.md#stacking)) | `window_stacking` |
| 1070:2bd7, 10b8:3fa2, 1018:2c96, 1038:1f0a, 1048:085a, 1028:056f, 1040:3bac, 1060:0e80, 1068:0194, 1010:2819 | Closing the money menu, overview map, build menu, info box, TV, auction, pause, options or end form ("Weiterspielen"), and loading a game from the file menu, raise the main screen over the other forms that aren't `fsStayOnTop`, which then lie hidden under it where they overlap it ([layout.md](layout.md#stacking)) | `window_stacking` |
| 1000:2241, 1010:2a5a | A press on the background raises `THintergrund` over the main screen and the other forms that aren't `fsStayOnTop`; its `OnActivate` raises them back in `window_forms`' order, the main screen on top, but not while the info box or the end form is shown, which leaves them hidden behind the background until one is raised ([layout.md](layout.md#stacking)) | `window_stacking` |
| 1020:1ce9, 1110:1572, 1050:2700, 1050:21b7 | The main screen and its map view are 640 × 479 and 20 columns by 27 half rows on every screen: a bigger desktop shows more background around them, not more island ([layout.md](layout.md#the-main-screen)) | `big_map` |
| 10b8:1349, 1110:3d80 | The overview map's frame for the main screen's view is a 40 × 25 box in every rotation; turned a quarter that view covers about 50 × 20 of the map ([overview-map.md](overview-map.md#the-frame)) | yes (`big_map` draws its own frame, the right shape, only for its bigger view) |
| 1018:2fa2 | The build menu's first position is `(screen_height - 480) div 2 + 59`, one row above the island (the main screen is 479 high, centred with `div 2` of each half) | `form_positions` |
| 1110:54fc users | The sabotage window and the file menu open where the DFM put them (300, 167 and 305, 139), so on a desktop bigger than 640×480 they aren't over the centred main screen ([layout.md](layout.md#the-other-forms)) | `form_positions`: moved with the centred main screen |

## Sound

[sound.md](sound.md) has the details.

| Where | Quirk | Kept |
|---|---|---|
| 10a0:2824 | The auctioneer's three calls (`zum1.wav`..`zum3.wav`, sounds 0x1a–0x1c) are looked for in `SOUND\`, but they're in `SPRADEUn\`: WaveMix gets null waves and they're never heard | `auction_calls`: from `SPRADEU1\` (the three directories have the same files) |
| 1090:5293, 10a0:30b6 | A sound's time (`1110:723c`) only counts down in `people_tick`, which runs with the "animations" option on: with it off, each sound with a time plays once and then never again (until "Neues Spiel") | `sound_times`: every tick |
| 1018:37de, 1018:367e | The playlist starts at track 0 while the intro's `holiday.mid` plays, so the first track after it is the second (`dub.mid`), and `anotrian.mid` only comes after the whole list | `music`: the first ticked track comes first |
| 1018:367e | A track's end is only noticed when the next day begins and the position hasn't moved for a whole day: up to a day of silence between tracks, and none changes while the game is paused | `music`: the next starts as one ends, paused or not (the day's check stays for a track that doesn't open) |
| 1010:26fa | Loading a game from the file menu with the music on opens its file again without playing it: silence until a day or two later the next track starts | `music`: it plays on from the track's start |
| 1048:0a54 | A message without speech only forgets a speech still open from the last one: it plays on, and the effects stay off, until the TV closes | `tv_sound`: it closes it |
| 1048:1b3e | The speech starts by the page on the TV, not the message it belongs to: browsing with the keys to a page without speech in the first 3 s keeps it from starting, and the TV then never closes by itself | `tv_sound`: by the posted message's |
| 1048:01bd, 1048:034f | A news film after a speech plays at the speech volume: the sound volume comes back only when the speaker closes, after the film | `tv_sound` |
| 1048:01bd, 1048:077a | A news film gives up WaveMix's wave device (10a0:2518) and only closing the TV takes it back, and only with "Geräusche" on: switching the sounds on later leaves the effects silent until something else takes it back | `tv_sound`: the film's end and the TV's closing take it back either way |

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
  stops the game for everyone until the TV closes (after its film, whose
  length and skipping are one player's, [films.md](films.md)), and the
  archive keeps the human's view origin. The news options are each player's view
  setting (`optionen::Settings::news`, saved with the view); for one
  player the port pauses as the original does. Intended for multiplayer,
  not built yet: news never pause the game, since each player can dismiss
  their own TV ([fernseher.md](fernseher.md#multiplayer));
- the animation options;
- the human's view, used when people are placed. Only its rotation and
  origin count, not the window or desktop size: the main screen's map is
  624×404 at every size, and the squares the people code looks at
  (`1110:5536`, 11 × 61 from the view's origin) and `people::sees` are
  fixed to it ([layout.md](layout.md#the-world-and-the-screen-size)), so
  players with different window sizes don't diverge by that. A scroll or
  turn places the people again and spawns new ones for the view
  (1088:7fb1): the command `Action::ViewMoved` carries the viewpoint, so
  every peer does the same, but the people follow whoever moved last;
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
