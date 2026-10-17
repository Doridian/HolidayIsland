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
| People (1088:0324, 1088:2df2, 1088/1090/1098) | Some tables are read past their end, and markers are written outside their layers for a home of -1 or a square off the map ([people.md](people.md#open-questions)) | Reads 0 (or "free"), or skips the write |
| Tables read for an index out of range: the month's length, a player's reputation (`*1110:6f24+0x2f`), player 0's advertising level and loan (1078:057a), kind 3's decay (step 0x52), the books (1070:1419) | Reads or writes the bytes next to the table | Reads 0 (31 for a month), writes nothing |
| 1080:3216 `plan_road` | A bridge dragged from water along a row of water to the map's edge wraps its byte coordinate and loops for ever (the edge and off the map count as flat water), writing owners off the map | Stops after one round of the byte; writes nothing off the map |
| 10b0:069f, 10b0:1907 | Raising or lowering a corner reads and writes past the 41 × 41 working copy where the land would spread further | The spreading stops at the copy's edge |
| 10b0:33bc | Working out the forms reads heights before row 0 or column 0 and writes forms off the map | Reads the layer's neighbouring bytes, writes nothing off the map |
| 1068:27a5, 1078:09f6, 1078:0d00, 1078:2178, 10a8:0987, 10a8:21de | A plot number without a record is used through a nil pointer | Skips that plot's update, or reads 0 |
| 10b0:2b9b | Returns an unset error for a corner not within 1..249 | Returns 0 (callers ignore it) |
| 10b0:53fe | Leaves the reference height and corners unset for a direction past 3 (nothing passes one) | Returns false |
| 1050:245b | Leaves its locals unset for a view rotation past 3 | Returns false |
| 1008:0297, 10a8:173b | Return an unset byte for a scenario past 7, or a levelling state past the known ones | Return false |
| Data files: `HAUSNAME.TXT` (1048:358f), `ANIMDAT2.DAT`, `ANIMDAT3.DAT` (1088:80a7), `NAMEN.TXT` (1010:0414), the high scores | A missing `***` end marker loops for ever; a group or list longer than its room overruns the next memory; a high score file of another size isn't checked | The file fails to load |

## Simulation logic

| Where | Bug | Kept |
|---|---|---|
| 1078:280b | Removing a building site leaves its location effect on the map ([simulation.md](simulation.md#open-questions)) | yes |
| 1078:280b | Removing a sea bridge adds up a cost that is never charged | yes |
| 1008:0f37 | Every price class's shop price level goes into the same byte (`+0xf`), so the last class wins; it also computes copies of the shops' places by class (1078:0780) and then drops them ([settlement.md](settlement.md#open-questions)) | yes |
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
| 1078:2396 (computers) | The computers' roads use the ground the human's last road drag started on (`1110:704e`): after a drag from water they'd look for bridges ([computer.md](computer.md#levelling-and-roads)) | no: they pass sand, what it holds before any drag |
| 10b0:03fa, 10b0:12c7 | The land only follows a moved corner where a neighbour is exactly two steps off, so existing cliffs get steeper | yes |
| 10b0:12c7 | Lowering checks the buildings (and clears palms, adding their cost) of squares it reaches after an error already stopped it | yes |
| 10b0:069f, 10b0:1907 | A palm cleared by moving the land only loses its building: its part, owner, plot count and location effect stay | yes |
| 10b0:069f, 10b0:1907 | For player 1 the money check replaces any earlier error with 0xe | yes |
| 1078:2178 | After a road is built only buildings whose square shows part 0 get their connection again, so houses with another look never do | yes |
| 1048:358f | A line shorter than three characters keeps the previous line's bytes, so one after a `***` line also ends a list of `HAUSNAME.TXT` | yes |

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

## Design issues for multiplayer

These aren't bugs for a single player, but the original mixes the human's UI
state into the simulation. See the open questions in
[simulation.md](simulation.md#open-questions) and [people.md](people.md):
- the TV pause;
- the animation options;
- the human's view, used when people are placed;
- the road drag state.
