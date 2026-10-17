# Bugs in the original game

The port reproduces the original's behaviour, bugs included, so that it
plays and computes the same. The plan is to make each of these fixable later
behind an option. This file lists every bug and quirk found so far, so that
none gets lost.

Each place in the code that keeps one is marked with a `// BUG(original):`
comment, so `grep -rn "BUG(original)" crates` finds them all. Add a row
here whenever you keep a bug or work around one.

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
| 1010:1129 | Planting a palm on a square with no plot counts it through a nil pointer | Skips the count |
| 1090:1a25 area | A house type with no capacity divides by zero | Skips the house |
| 1078:280b and the other building routines | Division by zero or a nil record in some cases | Skips that step |
| Settlement (1008:1bf7) | Division by zero for a type with no beds or places, or a factor whose divisor is 0 | The division gives 0 |
| Computer loans (10a8) | A `Kreditgroesse` of 0 or less loops forever | Skips the loans |
| 1030:23d3 `draw_text` | A word longer than a line, after the first space, writes past the 10 entries for line ends | Stops at nine lines |
| People (1088/1090) | Some tables are read past their end, and markers are written outside their layers for a home of -1 | Reads 0, or skips the write |
| 1080:3216 `plan_road` | A bridge dragged from water along a row of water to the map's edge wraps its byte coordinate and loops for ever (the edge and off the map count as flat water), writing owners off the map | Stops after one round of the byte; writes nothing off the map |
| 10b0:069f, 10b0:1907 | Raising or lowering a corner reads and writes past the 41 × 41 working copy where the land would spread further | The spreading stops at the copy's edge |
| 10b0:33bc | Working out the forms reads heights before row 0 or column 0 and writes forms off the map | Reads the layer's neighbouring bytes, writes nothing off the map |
| 1068:27a5, 1078:09f6, 1078:0d00, 1078:2178 | A plot number without a record is used through a nil pointer | Skips that plot's update |
| 10b0:2b9b | Returns an unset error for a corner not within 1..249 | Returns 0 (callers ignore it) |
| 10b0:53fe | Leaves the reference height and corners unset for a direction past 3 (nothing passes one) | Returns false |

## Simulation logic

| Where | Bug | Kept |
|---|---|---|
| 1078:280b | Removing a building site leaves its location effect on the map | yes |
| 1078:280b | Removing a sea bridge adds up a cost that is never charged | yes |
| 1008:0f37 | Every price class's shop price level goes into the same byte (`+0xf`), so the last class wins; it also computes copies of the shops' places by class (1078:0780) and then drops them | yes |
| 1008:0907 | Divides by `MaxPreis` where the range seems meant | yes |
| Settlement | Buildings with owner 0 add into the slots next to theirs | yes |
| Settlement, year end | Only the human pays advertising, insurance, cost of living and renovation; computers pay a sixth of their interest. This may be meant as a handicap, not a bug | yes |
| 10a8:21de | Checks the area of the plot the computer is leaving, not the one it moves to | yes |
| 1098:0002 | Returns an undefined value; the port keeps the ship's heading | n/a |
| Harbour ships (1098:109f) | The second detour checks the ship's own square instead of the squares ahead | yes |
| 1090:2c1f | A plane's second approach is accepted when either coordinate matches, not both | yes |
| 1078:09f6 | A corner rising from the sea between plots merges the first pair by their numbers cut to a byte | yes |
| 1080:3216 | Building any road (the computers' too) turns the "new people" animation option (`1110:0056`) on, whatever the player chose | yes |
| 1078:2396 (computers) | The computers' roads use the ground the human's last road drag started on (`1110:704e`): after a drag from water they'd look for bridges | no: they pass sand, what it holds before any drag |
| 10b0:03fa, 10b0:12c7 | The land only follows a moved corner where a neighbour is exactly two steps off, so existing cliffs get steeper | yes |
| 10b0:12c7 | Lowering checks the buildings (and clears palms, adding their cost) of squares it reaches after an error already stopped it | yes |
| 10b0:069f, 10b0:1907 | A palm cleared by moving the land only loses its building: its part, owner, plot count and location effect stay | yes |
| 10b0:069f, 10b0:1907 | For player 1 the money check replaces any earlier error with 0xe | yes |
| 1078:2178 | After a road is built only buildings whose square shows part 0 get their connection again, so houses with another look never do | yes |

## UI quirks

See [widgets.md](widgets.md#quirks-kept-in-the-port) and
[startup.md](startup.md) ("Quirks").
- The scroll bars step only when the mouse is released.
- A scroll arrow can stay drawn pressed.
- A click on an edit field while it has focus takes the focus away.
- A list box ignores clicks with Shift held.
- Switching pages on the start menu can leave a different scenario ticked
  than the one that starts.
- Letting go of the right mouse button also clicks a button held down with
  the left.

## Design issues for multiplayer

These aren't bugs for a single player, but the original mixes the human's UI
state into the simulation. See the open questions in
[simulation.md](simulation.md#open-questions) and [people.md](people.md):
- the TV pause;
- the animation options;
- the human's view, used when people are placed;
- the road drag state.
