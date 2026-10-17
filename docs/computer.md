# The computer players (segment `10a8`)

Ported in [`game::computer`](../crates/game/src/computer.rs), the build
check and the building sites in
[`game::construction`](../crates/game/src/construction.rs), the loans in
[`game::money`](../crates/game/src/money.rs). The computers get a turn on
each of the monthly settlement's free steps
([settlement.md](settlement.md#the-computer-players)), one of them at a
time: `computer_players_tick` (10a8:302e) sets the status text to `Gar
nix`, counts the turns (`1110:70c2`) and gives player `turns mod
opponents + 2` (a signed `Longint` remainder) its turn.

Everything they do changes only the world and draws from `Random` in the
order below. They play like the human through the same routines
(`can_build`, `start_building`, `demolish`, the loans), with three
differences: they don't need the money to build, they don't pay
advertising, insurance or the cost of living
([settlement.md](settlement.md)), and on the harder levels `add_money`
gives back a sixth or a third of what they pay. They never launch
sabotage: the only caller of the sabotage launcher (10a0:1f65) is the
human's sabotage form (1028:3d93).

## State

Saved with the map (unit 10a8:32ec; `1110:00a2` in 1000:0c6a). The
original saves all seven players' entries whatever the player count, so
the CD's maps carry heap garbage for the players they don't have. The
candidate lists are saved for players 2 to the player count only.

| Global | What |
|---|---|
| `1110:7062`, `7072` | Turns waited, turns before demolishing (`array[1..8] of Word`) |
| `1110:7082`, `7092`, `709a` | The levelling's phase, its height, who owns the square that stopped it |
| `*1110:70a2` | The square it works at: its area, then its site, then a road's end (`array[2..8]` of `i`, `j`) |
| `*1110:70a6` | The centre: a road near the area, the site is looked for around it |
| `*1110:70aa`, `*1110:70ae` | The next road's start and direction (0 `j + 1`, 1 `i + 1`, 2 `j - 1`, 3 `i - 1`) |
| `*1110:70b2` .. `*1110:70be` | `Random(7)`, 1, `Random(2) + 7` and the first again at the start; never read |
| `*1110:70c6`, `*1110:70ca` | The building's state (0–11) and the turn's (0–5) |
| `*1110:70ce`, `*1110:70da` | The category (a `COMPBAU.DAT` row) and the type it builds |
| `*1110:70d2` | It builds in a new area: no road is needed near the site |
| `*1110:70d6` | The area has no road yet: the first roads start around its transport building |
| `*1110:70de` | 12 counters each; 2, 4, 8 and 10 are used |
| `*1110:70e2`, `*1110:70e6` | Candidate lists (1030:0c82: three byte lists of up to 254 entries, the direction, `i` and `j`): an area's first road starts, the built roads' ends |
| `*1110:70ea` | Per category a count and 40 types, 13 rows, 11 used; the original reads and writes past a row's end |
| `*1110:70ee` | `COMPBAU.DAT`: 13 rows of 7 bytes |
| `1110:70f2` | The status text, a `string` (shown nowhere) |
| `*1110:6512` | The budgets: 13 `Longint`s for each of players 2–8, the capacity (`HAUSDATA.DAT` column 3) of its building sites per category; a site adds it (1078:15d9), finishing takes it off (1078:203d) |
| `1110:6f28` | `array[1..8] of Boolean`: the bank said no |
| `1110:634c` | The human drags out a road (1080:3195); the computers don't build roads meanwhile, building one clears it ([player-tools.md](player-tools.md)) |

When the game's data is loaded (1030:3745), 10a8:388b builds the type
lists (for each category 0–10 the types 0–0x7e whose row matches and
whose `HAUSDATA.DAT` column 6 is 0, not one of a kind) and 10a8:3558
starts each computer player 2..count: `Random(200) + 25` for `i` then `j`
as its square and centre, `Random(7)`, `Random(2)`, thinking (2) and
looking for its first area (9), type 255, the budgets 0, the lists
empty, the turns 0. A map then replaces all of it; the port's
`World::build_type_lists` and `World::init_computers` are for worlds
that don't come from a map.

A `COMPBAU.DAT` row (10a8:03a6) matches a type by `HAUSDATA.DAT`
column 4, one of two kinds (column 5) and one of four types, 255
matching anything in each group. On the CD the rows are: 0 transport
(kinds 9, 10), 1 type 32 (energy), 2 type 33 (water), 3 type 28
(rubbish), then the kinds 6, 5, 3, 4, 2, 8 and 0 or 1 (houses, row 10).

An event that makes a type available (10a0:0691 kind 0) puts it on each
matching list too (10a8:0523), one of a kind or not, without a check for
the row's end; the hotels 0x60, 0x64, 0x68 with the three types after
them and 0x2c, 0x2e, 0x30, 0x32 with the next. Kinds 2–7 of 10a0:0691
make that player build the type next (10a8:2cc1: turn state 1, building
state 2).

## The turn (10a8:2d5a)

Nothing for a player who is out. By the turn's state:

| State | Status | Does |
|---|---|---|
| 0, 1 | | The building's step, below (1 is a type an event ordered) |
| 2 | `WLTNaechstenSchrittueberlegen` | Loans (10a8:2a60); then, unless the bank said no and it's in the red, `Random(1000)` 0 demolish, else build; otherwise `Random(100)` 0 demolish, else wait |
| 3 | `WLTGebaeudeverkaufen` | Nothing; never set |
| 4 | `WLTAbwarten` | After 11 turns, think |
| 5 | `WLTGebaeudeabreissen` | Every other turn: the worst building (10a8:2ade), and if there is one, `Random(100) < 20` sells it through event 9 (1010:3006) with twice `HAUSPREI.DAT` column 2 for a type of price 10 or less, else a quarter of the price, and the player; otherwise its people leave and it's demolished (1078:280b). Then think; without one, wait |

The loans: while it's in the red and the bank hasn't said no, it borrows
`Kreditgroesse` (1078:05c6: if the loan would pass `MaxSchulden` the bank
says no, else money and loan go up); while it has more than that and owes
as much, it pays that back (1078:061b, which clears the bank's no). The
interest is worked out again each time (1078:04ce).

The worst building: of its houses whose rent (guests × rent) is below
their running costs (`HAUSPREI.DAT` column 1), the one with the fewest
guests in percent of its beds (`Integer` arithmetic); then of its
facilities (not one of a kind) whose takings (`fill × price div 100`) are
below their costs, one less full; below 100 % only.

## Building (10a8:2463)

| State | Status | Does |
|---|---|---|
| 0 | `WLBGebaeudeartauswaehlen` | Category (10a8:0987), then 1 |
| 1 | `WLBGebaeudewaehlen` | Type: `Random(count)` of the category's list; then 2, or for a category past 10 back to 0 and wait |
| 2 | `WLBGebaeudeBauPunktsuchen` | Counts tries; past `min(squares, 8) * 4` (`HAUSNR.DAT` column 5) roads (10) and think (unless ordered); else a road near the area (10a8:13f6), then 3 |
| 3 | `WLBGebaeudeBauPunktsuchen2` | A site around it (10a8:1454): 4, else 2 |
| 4 | `WLBGebaeudebauen` | Build (10a8:163b): done, think and choose again (0), or in a new area build its roads (10); on the first try 0x14 levels the site (5), 0xc evens out its heights (6), else 2 |
| 5, 6 | `WLBBauplatzebenmachen`, `WLBBauplatzhoeheausgleichen` | Level the site (10a8:173b), then 4 |
| 7 | `WLBStrassebauen` | Unless the human drags a road: a road (10a8:1b0a); built, think and choose again (or for an ordered type look for a site again, 2), else 10 |
| 8 | `WLBInselwechseln` | Another area (10a8:21de); found or after 100 tries, a new area |
| 9 | `WLBStartInselsuchen` | The first area (10a8:207b), then a new area |
| 10 | `WLBStrassenBauPunktsuchen` | 20 tries for a road start (10a8:1f44), then 7; then another area (8) |
| 11 | `WLBNichtstun` | Nothing |

A new area: the category is transport (0), new area and first roads set,
then 1.

- The category (10a8:0987), by the plot of its square, nothing without
  a plot: in a new area transport. Else the first that fits of: the
  airports' and harbours' places (`+0xde`) up to the beds (`+0x22`);
  energy (`+0xe2`), water (`+0xe6`), rubbish (`+0xea`), each plus
  `Random(50)`, up to three fifths of the beds; the greenery (`+3`) plus
  `Random(20)` up to 51 (category 4); nature (`+0xda`, 4), leisure
  (`+0xd6`, 5), shops (`+0x7a`, 6), sport (`+0xd2`, 7), rentals (`+0xa6`,
  9), restaurants (`+0x4e`, 8), each plus `Random(40)`, up to three
  fifths of the beds; else houses (10). Each counts the budget of its
  category: what's being built.
- A road near the area (10a8:0e30): 100 steps of a random walk from its
  square, `Random(5) - 2` on `i` then `j`, kept on the map, until a road
  (ground 10–58) on the same plot, or any square of it in a new area.
- A site (10a8:1309): 51 tries of `centre - size - 7 + Random(17)`, `i`
  then `j`, until one on the centre's plot whose area is on the map, has
  no building but palms and beach life, plain ground (below 10) and for a
  harbour (0x58–0x6b) its highest corner at 1; and unless it's a new
  area, with one of the player's roads within 3 squares of every column
  from `i - 1` to `i + width` (or else of every row), for houses a
  location of 180 or more, and for kinds 9 and 11 none of the player's
  houses within 2 squares.
- Building (10a8:163b) through `can_build` (1030:04e3, below), ignoring
  error 0xe; then `start_building` (1078:1d21) with the category as the
  budget. For a new area's transport building the squares within 2 of
  it that are free, road-flat (forms 0–3, 16) and above the sea become
  the first roads' starts, in all four directions (10a8:14b2).
- Levelling (10a8:173b): first the height, the mean of the area's
  lowest corner (from `1110:00a2` down) and highest, if it's 1–9 and
  suits `HAUSDATA.DAT` column 7 (0 up to 2, 1 from 3, 2 any); then each
  turn the first corner in a scan from `Random(width + 1)`,
  `Random(depth + 1)` (stepping by 17, mod the size) that is someone
  else's (done, the owner kept) or not at the height: it's raised or
  lowered (10b0:2c83), until an error or none is left.
- Roads (10a8:1b0a): from the start five squares on in its direction
  (1078:2396 plans and lays the road). Built, the end becomes its square
  and a start for more in the same direction, and for its two sides where
  it's level; for an area's first road the start's back and sides too.
- Road starts (10a8:1f44, 10a8:1ded): `Random(count)` of the first
  roads' starts (or the roads' ends), taken off the list; in an area's
  first roads its square is levelled towards its direction (10b0:53fe);
  then the next square must take a road (10a8:064b: 0 free; 1 water, 2 a
  river but 59–62, 3 a building, 4 a road, 5 too steep, levelled if it
  can be, 6 off the map). Up to 10 a turn.
- Areas: the first (10a8:207b) by up to 100 steps of `Random(4)` (`i +`,
  `i -`, `j +`, `j -`) and `Random(20)`, kept within 0..250, until a plot
  of 200 squares (`+0x352`); another (10a8:21de) by up to 50 steps with
  `Random(15)` until another plot, as long as the plot it left has 200
  squares (it checks the old plot, it seems by mistake).

## The build check (1030:04e3)

`can_build(i, j, building, player, var cost)`, also the human's: 0x16 if
the area isn't within 0..250; the cost is the price (`HAUSPREI.DAT`
column 0), and player 1 gets 0xe without the money. Then square by square
(rows of `j`, the first error wins): for a type on any land (column 7 is
2) but a palm, each square that isn't level adds `Fundamentbaukosten`
(0xe for player 1 without the money); ground 10–54 (road) 2, 55–58 (road
over a river) 0xf, 59 and up 0x10; a building but a palm 1 (the square
kept at `1110:64ba` for the message); a highest corner other than the
first square's 0xc; the ground for column 7 (0 sand 0x11, 1 grass 0x12, 2
not water 0xb, 3 water 0x13; 4–7 water along the first row or column and
sand elsewhere or the other way round); the form for column 8 (0 flat
0x14, 1 not 8–11 (10), 2 at most 3 (0x15), 4–7 one side sloping, 8 no
palm 0x1b).

A building site (1078:1d21): a palm is planted (1010:1129), anything else
becomes a site (1078:1b5e: for each square, in byte loops, a palm comes
off its plot's count and the location map, the square gets type 31, its
effect, `Random(HAUSNR.DAT[31][0])` as its look and the player as its
owner), whose record (1068:1c7d) goes in front of the list with
`HAUSDATA.DAT` column 0's days and the category as its budget, which
gets the type's capacity. The cost is paid; for player 1 the books' new
buildings (0x67) get the price and the cost rises from the square, and
a type that's one of a kind leaves the build menu.

## Levelling and roads

Raising and lowering the land is ported in
[`game::terrain`](../crates/game/src/terrain.rs), the road planner in
[`game::roads`](../crates/game/src/roads.rs), both world code that the
human's tools call too, through commands
([player-tools.md](player-tools.md)).

- A corner moves one step at a time (10b0:069f up, 10b0:1907 down) on a
  41 × 41 working copy of the heights and buildings around it
  (`*1110:721e`, `*1110:7222`). Each step checks the four squares around
  the corner: a building stops it (1; a palm goes for
  `Palmenabrisskosten`), a road (2), a river (0x10), and when raising
  `layer_551c` (0x1c). Neighbours exactly two steps off follow,
  recursively (i + 1, j + 1, i - 1, j - 1). Raising also needs the corner
  below 20 (4) and as far from the map's edge as it is high (5–8). The
  cost adds up `LandausWasserhebekosten` (from 0), `Landhebekosten`,
  `LandinWassersenkkosten` (to 0) or `Landsenkkosten` per corner; only
  player 1 must have it (0xe), and the computers drop it. On an error
  nothing changes and a marker (person kind 0x2a) appears at the square
  that caused it. Otherwise the copy goes back, the corners that rose
  from the sea get a plot (1078:09f6: new, grown or merged, 1068:2698,
  1068:27a5), a corner sunk to 0 shrinks its plot or removes it
  (1078:0d00, 1068:2a2b, the last plot taking its number), and the
  touched squares get their forms from the corners (10b0:33bc, the table
  at `1110:1224`, 19 if none fits) and plain ground its tile back.
- `level_corner` (10b0:2c83) is one step towards a height;
  `set_corner_height` (10b0:2b9b) steps until it's there or fails;
  `level_road_square` (10b0:53fe) sets the square's two corners towards
  the road's direction to the height of one behind, and if the square
  isn't level then, both a step below (if its lowest corner is lower) or
  above.
- `build_road` (1078:2396) for player `p` from `from` towards `to`:
  `plan_road` (1080:3216) goes along the longer axis while each square
  takes a road (1080:2831: no building but a palm, not water, flat and not
  a river, or a crossable river piece or a ramp along the road's way) and
  lays it on squares that aren't roads and that no one else owns
  (1080:2b0c: the piece for its neighbours, or a ramp's `0x15 + form`),
  making them `p`'s; from a drag that started on water it builds a bridge
  between facing ramps instead (1080:2a82, 1080:2e6a, 1080:261a). Then
  1078:2178 counts the road on its plot (`+0x356`) and works out the
  connection of the buildings near it again. Every new square (no owner
  before) costs `Strassenbaukosten` (`1110:64ee`, `MODATYP.DAT`, not in
  the map), which the computers pay too. Player 1 also needs the money for
  the drag's length and gets the messages (0xe, 0x1f, 1030:235a).
- The human's road drag: its start square (`1110:6344`) is the view's
  and an argument of `build_road`; its flag (`1110:634c`, which the
  computers wait on and building a road clears) and its start's ground
  and form (`1110:704e`, `704f`) are the world's, set by the drag's
  command. The computers' roads read the ground too, as in the original:
  after the human's drag from water they look for bridges. Before any
  drag it's sand (0), or what the map saved.

None of this draws random numbers, but the markers are people
(1088:0324). The bugs kept are in
[original-bugs.md](original-bugs.md).

## Auction bids (10a8:0373)

When the auction form opens (1028:0dde) every player's limit is worked
out once, at the start price (10a8:0221, 10a8:0167 with `Random(1000)`):
see [auction.md](auction.md#the-computers-bids-10a80373). A computer
bids only for a building or a bankrupt player's property, or while its
area isn't waiting for its first roads, and up to three times the price
or what it can pay (money less loan plus `MaxSchulden`).

## Open questions

- What `1110:00a2` (20) is: the levelling's lowest corner starts there,
  so probably the highest a corner can be.
- `*1110:70b2`..`*1110:70be` are set at the start and saved, but nothing
  reads them; perhaps left over from planned behaviour (sabotage?).
- The turn's state 3 (selling) is never set; the selling happens in
  state 5.
- 10a8:21de checks the area of the plot it leaves, not the new one's.
- A computer in the red with the bank's no only waits (or demolishes one
  time in a hundred); nothing else pays its debts.
- `1110:634c` and `1110:704e` are the human's UI state in the world: for
  multiplayer the computers' wait on another player's road tool, and
  their roads by another player's drag, need a decision.
- Building any road turns the animations' `1110:0056` on (1080:3216),
  whatever the player chose; the port keeps it.
