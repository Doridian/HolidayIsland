# The simulation

How game time moves, and what runs on each tick. The driver is ported in
[`game::simulation`](../crates/game/src/simulation.rs), the date in
[`game::monate`](../crates/game/src/monate.rs), the money in
[`game::money`](../crates/game/src/money.rs), and the daily subsystems
in the modules their sections below name. The rest of this page is the
map later passes port the remaining subsystems from.

## The driver: `AnimationsTimer` (1020:1e9b)

There is one simulation clock: `THauptschirm`'s `AnimationsTimer`
(`+0x190`, a `TTimer` with `Interval` 100 in the DFM). Its `OnTimer`,
`hauptschirm_animationstimer_timer` (1020:1e9b, 1403 bytes), runs every
100 ms while the main screen is up, and does everything below in this
order. `THintergrund`'s timer only runs the intro and the game start
([startup.md](startup.md)). The timer's `Tag` is reused as the direction
to scroll while the mouse is held on a scroll bar arrow (step 6). The
auction form turns it off while it's open (1020:24a0, 1020:24b7), so the
whole simulation stands then ([auction.md](auction.md)).

| # | Does | Address | Size | Side |
|---|---|---|---|---|
| 1 | Re-enables the 17 forms in `1110:646c` except 10 and 11, unless `infobox2+0x1e6` is set or `schlussform` is visible | 1010:2a0c | 78 | view |
| 2 | Status bar message timeout: if `1110:009a` > 0, counts it down; at 0 clears the status bar (1030:22de with `''`). Messages set it to 20 (1030:232d, 1030:235a) | inline | | view |
| 3 | With tooltips on (`1110:0060`): the help bubble (`helpform`, 1040:3dc7 / 1040:3ead) for the money menu's buttons (1070:0294), the overview map's (10b8:02e9) and the button bar (1018:394e) under the mouse | inline | | view |
| 4 | Unless paused (`1110:0067`): the game clock, [below](#the-clock-step-4) | inline | | world |
| 5 | `1110:009e := (1110:009e + 1) mod 30`, then `building_slice(1110:009e)`, [below](#the-buildings-tick-10780002) | 1078:0002 | 330 | world |
| 6 | Scrolls the view by the timer's `Tag`: 1–4 `scroll_ns` by −1, −16, 16, 1; 5–8 `scroll_wo` by −1, −18, 18, 1 | 1020:0cf7, 1020:147f | | view |
| 7 | During an earthquake (`1110:0087` = 1): moves the form by `Random(7) - 3` in x and y; on the first tick of each hour also `quake_damage` | inline; 10a0:0d1c | 192 | view; world |
| 8 | With the TV (`fernseher`) visible: its tick, which plays the news film and closes it after 3 s (`GetTickCount`) | 1048:1b3e | 1107 | view |
| 9 | Without animations (`1110:0054` = 0) just refreshes `Hauptbild`. With them: `people_tick`, and with `1110:0056` the rest of the animation group (new people, drawing them), [below](#animations-step-9) | 1090:5293 | ~3850 | world and view |

### The clock (step 4)

Two counters on the form, saved in the map (unit 1020:2b1d):
`+0x1d6` counts the ticks of an "hour", `+0x1d8` those of a day. The
game speed `1110:0096` (`Longint`, 4 by default, `100 - position` of the
options' slider, 1060:0619) is the length of an hour in ticks; a day is
six hours. So at the default speed a day takes 2.4 s and a month about
72 s.

1. Both counters go up by one.
2. On the first five ticks of a day, one routine each, by `+0x1d8`:

   | `+0x1d8` | Routine | Address | Size | What |
   |---|---|---|---|---|
   | 1 | `events_tick` | 1010:3084 | 809 | Counts `1110:653a` (days since the start) and runs the scenario's events (`EREIGNIS.DAT`/`EREIGNn.DAT`, 41 records of 0x34 at `*1110:6536`): starts the due ones with a chance (`Random(65000)`, 1000:3d9e), posts their news (1048:0a54), starts auctions (1028:08ac), floods, earthquakes and oil slicks (`1110:0086`, `0087`, `0088`), ends them after their duration. [Below](#events-10103084) |
   | 2 | `effects_tick` | 10a0:117e | 1495 | Counts down the five sabotage slots at `*1110:705a` (10 bytes each) and the three championships at `*1110:705e` (8 bytes), which change the location map (1030:185d), swap buildings (10a0:0026), post news, change reputations (`*1110:6f24+0x2f`). [Below](#sabotage-and-championships-10a0117e) |
   | 3, 5 | `vegetation_tick` | 1010:129e | 404 | 30 random squares (`Random(250)` twice): a young palm grows, a grown one ages, an old one dies (60%); an empty square of sand or grass gets a palm (30%, 1010:1129). [Below](#palms-1010129e) |
   | 4 | `beach_life_tick` | 1008:1b4b | 172 | For each house in the list at `*1110:6f10` (0x2c-byte records), one chance in 30 (`Random(30)`) to move its beach life (`+0x2b`) a step towards the class its `+0x10` value falls in (1008:1aea, 97 bytes, `Random(5)`). [Below](#beach-life-10081b4b) |

3. When `+0x1d8 >= speed * 6` (32-bit, signed): `+0x1d8 := 0` and a new
   day: `date_advance` (1030:00eb), the date's text (1030:0148) onto the
   info bar (1030:228d), and the next music track if the current one has
   ended (1018:367e, 352 bytes, view).
4. When `+0x1d6 >= speed`: `+0x1d6 := 0` and `accounting_tick`
   (1008:5307, 152 bytes), [below](#accounting-10085307).

Both counters start together, so the day and the hour wrap on the same
tick, the day first: there are six accounting ticks a day, and the first
one sees the new date.

### Accounting (1008:5307)

`1110:4a88` (`Word`) is the day the last accounting tick saw, `1110:4a84`
(`Longint`) a step counter. Both are saved (unit 1008:539f).

- If the day changed since the last tick: `construction_tick` (1008:5212,
  [below](#building-sites-10085212)) and remember the day.
- Otherwise: the step counter goes up. Past 12 on the 1st of a month it
  goes back to 0, and in January that is the new year (`year_end`,
  1008:4f64, [below](#the-new-year-10084f64)). In January at step 5 (on
  the 2nd), `january_second` (1008:0512, [below](#the-scenarios-end-10080512)).
  Then, every time, `settlement_step(step)` (1008:1bf7).

So the step runs from 0 (the 1st) to about 150 over the month, five a
day. `settlement_step` (1008:1bf7, 13165 bytes, the `ABRECHNE` unit),
called with the step's low word, is the monthly settlement spread over
the month: a chain of `if step = n` for 48 steps from 2 to 0x8b, each
doing one part of the month for all players (guests, rents, takings,
costs, interest, ratings, reputation, the books), and the computer
players' turns on every other step. Ported in
[`game::settlement`](../crates/game/src/settlement.rs), step by step in
[settlement.md](settlement.md). Money goes through `add_money`
(1078:01e7).

### Building sites (1008:5212)

Ported in [`game::construction`](../crates/game/src/construction.rs).
A building under construction is building type 31 on its squares and a
13-byte record in the list at `*1110:6f08` (saved by unit 1068:32b5):
`+1`, `+3` its first square, `+5` the days left (`Integer`), `+7` the
type it becomes, `+8` (a signed byte) the computer player's budget it's
paid from. New sites go in front (1068:1c7d, with `HAUSDATA.DAT`'s column
0 as the days, 1 while the money is frozen). Loading a map puts each
record in front of the ones before it (1068:304f) while saving walks the
list (1068:30d0), so the file has them in reverse.

`construction_tick` takes a day off each site, and `finish_building`
(1078:203d) finishes the ones at 0 or less, in the list's order:

1. `place_building` (1078:1de9): the first square's part is
   `Random(looks) * HAUSNR.DAT column 5` (low byte) for types 0, 12 and
   0x15 with at least two looks (column 0), 1 for type 0x3b on sand, else
   0. Then for each `j`, for each `i` of the area (byte loops): the
   building site's effect comes off the location map (1030:1d10 with
   `remove`), the building goes on and the part is `first + (i - i0) +
   width * (j - j0)`. Then `register_building` (1078:0eda,
   [below](#the-buildings-records-10780eda)) and the redraw.
2. If a computer player (2 to the player count) owns the square,
   `HAUSDATA.DAT` column 3 comes off its budget `*1110:6512 + owner *
   0x34 + budget * 4 - 0x68` (13 `Longint`s for each of players 2–8).
3. The builders (people of kind 0x26 in the area) leave (1088:6bac).
4. A championship building (0x6f–0x7c) takes the first free
   championship slot (10a0:02ad): 0x6f tennis and 0x71–0x74 surfing for
   60 days, 0x79–0x7a Miss Holiday Island for 30; the full versions take
   the slot without setting its kind and days. Other types in the set at
   `1110:1458` (29, 30, 35, 37, 60, 108, 110, 125, 126) raise the square
   owner's reputation by 30, to at most 100, whoever the owner is.
5. The record comes off the list (1068:2153).

The original also closes the info box if it shows a square of the site
(1038:1ee0). `find_building_site` (1078:1995) finds the record whose area
holds a square showing type 31; the go-slow sabotage uses it.

### The buildings' records (1078:0eda)

Ported in [`game::construction`](../crates/game/src/construction.rs).
`register_building(i, j, building)` does nothing on a square without a
plot (1030:1e62 gives 0): no record, no sums, not even the location
effect. Otherwise, on the plot record (1068:2591):

1. `HAUSNR.DAT` column 5 (the type's number of squares, also the step
   between its looks) goes onto `+0x354` for a park (`HAUSDATA.DAT`
   kind, column 5, 6), the palms' counter, else onto `+0x356` (`Integer`s).
2. The record, by `1110:1341[kind]` (`array[0..10]`; kind -1 reads
   `1110:1340` = 3, kind 11 `1110:134c` = 2): 0 a house (1068:1a08,
   kinds 0 and 1), 1 or 2 a facility (1068:1b3e; 1 for kinds 2, 3, 8, 2
   for 4, 5, 6, 9, 10, 11), 3 none (kinds -1, 7). Both go in front of
   their list (`*1110:6f10`, `*1110:6f14`) and of the plot's list
   (`+0x358`, `+0x35c`, 8-byte nodes; loading a map rebuilds those from
   the lists, 1068:2d49, each building in front of the ones before). A
   house gets condition 127, `HAUSPREI.DAT` column 2 at `+0x12`, 0x50 at
   `+0x16`, `HAUSDATA.DAT` column 4 at `+1`, 0 at `+0xc`, `+0xe`, `+0x10`,
   `+0x2b` (the rest of the 0x2c bytes isn't set); a facility the same
   with the price at `+0x16`, everything else 0. Both get at `+2`
   `Angeschlossenumrechnung[d]` (`1110:6508`, `array[-1..4]`), `d` being
   the first ring (1 to 4) around the building's area with a square on
   the same plot that's a road (ground 10–58) or an airport or harbour
   (kinds 9, 10), else -1 (1030:1706, 1030:15d2).
3. `HAUSDATA.DAT` column 3 (the capacity, it seems: beds for houses)
   goes into a `Longint` sum on the plot and in `*1110:6f20`: for a
   house `+0x22` and `+2`; for record kind 1 `+0x4e + 0x2c·class` and
   `+0x2e + 0x2c·class`; for 2 `+0xc6 + 4·class` and `+0xa6 +
   4·class`. The class (1010:2493) is `1110:134c[kind]` for kinds 2–10
   (2 → 0, 3 → 1, 4 → 3, 5 → 4, 6 → 5, 7 → 10, 8 → 2, 9 and 10 → 6), for
   kind 11 9, 7 or 8 for the rubbish dump (0x1c), the power station
   (0x20) and the sewage works (0x21), else 10.
4. Airports and harbours (kinds 9, 10) count in the plot's `+2` (a
   byte); the first on a plot adds one to `*1110:6f20`'s first `Word`.
5. The building's effect goes onto the location map (1030:1d10).

`unregister_building` (1078:124e, through 1078:1684) undoes all of that
for the building whose first square it gets, removing the first record
at that square (1068:1e6d, 1068:1fe0) and the effect (1030:1d10 with
`remove`). `find_record` (1078:169a) finds a building's record by its
first square, which 1030:154c works out from a square's part: `r :=
part mod squares`, then `(i - r mod width, j - r div width)`.

### Demolishing (1078:280b)

Ported in [`game::construction`](../crates/game/src/construction.rs)
and [`game::roads`](../crates/game/src/roads.rs).
`demolish(j, i, player)`: player 0 (the disasters) skips the check that
the square is the player's. Errors (`FEHLER.TXT`, 1030:232d) only show
for player 1: 0x18 "Geht nicht!", 0x17 "Gehört Ihnen nicht!", 0xe not
enough money, 0x1d "Die Zuschauer protestieren!". By the square's
building:

- Not in the set at `1110:1378` (11 and 52 the bridges, 12 palms, 36
  beach life, 255) and the player's:
  - A building site (31): for each square of the site's area
    (1078:1995): its people leave (1088:6975), building 255, part 0, its
    ground back (10b0:3199), owner 0, occupancy 0, and 1088:6c5a. The
    owner gets `price * days left div building time` back (`HAUSPREI`
    column 0, `HAUSDATA` column 0; player 1 also in the books, kind
    0x6a), and the site goes off the list. The site's effect stays on
    the location map.
  - A full championship building (`1110:1418`): 0x1d.
  - Any other building, for `price div 20` (0xe if player 1 can't pay):
    the house's beach life is cleared (1078:267e, `clear_square` with
    its `Random(1)`), a rental's vehicles go (1078:273a: 0x0a, 0x1a,
    0x1b, 0x6d), the people leave, the building is unregistered, and its
    area (byte loops from its first square) is emptied as above.
- Otherwise by the ground: a road (10–58), the player's, for
  `Strassenabrisskosten` (`1110:64ea`): its people leave, owner 0; a
  road over a river (55–58) goes back to the river piece (55 → 59, 56 →
  61, 57 → 60, 58 → 62) and loses its bridge (building 11), any other
  gets its ground back. Then its flat road neighbours get the pieces
  their neighbours call for (1080:302a, 1080:232a).
- A bridge over the sea (ground 95–100), the player's, for nothing: from
  the square along the bridge by its part (2, 4 towards `i - 1`; 3, 5
  towards `j + 1`; then 0, 4 towards `i + 1`; 1, 5 towards `j - 1`) every
  square of bridge ground becomes water (1078:276f), then the square
  itself, and the roads at both ends are reconnected. 1078:276f adds up
  `Strassenabrisskosten` per square in a sum nothing uses.
- Anything else: 0x18.

The empty squares' ground (10b0:3199): with a corner at sea level water
(2); a corner one or two steps up sand (0), or sand to grass (4) with
part 1 or 0 if a corner is 3 high or more; else grass (1). A part past
the tile's `GROUNDNR.DAT` variants (`1110:1202[ground] + form`) becomes
0.

A road square's piece (1080:232a): a road gets its ground back first,
then `piece[mask] + base`, `mask` having 1, 2, 4, 8 for a road or bridge
at `(i + 1, j)`, `(i, j + 1)`, `(i - 1, j)`, `(i, j - 1)` (`piece` the
jump table at 1080:230a: 10, 11, 10, 0x13, 11, 11, 0x12, 0xd, 10, 0x14,
10, 0xe, 0x11, 0xf, 0xc, 0x10) and `base` by the new ground (1080:22be:
sand 0, grass 0xf, water 0x5f, sand to grass 0x1e). The map's edge
counts as water (1000:3e0d) and flat (1000:3e8d).

### The new year (1008:4f64)

Ported in [`game::accounts`](../crates/game/src/accounts.rs).

1. The money menu's year ends (1070:269d): this year's total (kind 0x6b,
   1070:1452) is worked out, kinds 0x5f–0x6b become last year's 0x52–0x5e
   and are cleared.
2. Every house, then every facility, whose owner `p` has a renovation
   interval (`1110:1333 + p`: `1110:1334`, `array[1..8] of Byte` from the
   map; 0 never) that divides the year (`Integer` mod) is renovated: its
   condition `+7` becomes 127. Only player 1 pays: `(127 - condition) *
   Renovierungskosten (1110:64c2) * price (HAUSPREI column 0) div 1000`,
   through `add_money` and the books' kind 0x6a (1070:1419).
3. `MaxSchulden` goes up by 100 000, 50 000 or 10 000 by the difficulty.

With the option `1110:0064` the original then shows the money menu
(1070:2b11).

### The scenario's end (1008:0512)

Ported in [`game::accounts`](../crates/game/src/accounts.rs). Unless
`1110:0071` (won) is set: if the scenario (`1110:006e`) is won
(1008:0297), `0071` := 1, and unless `1110:006f` (set by a key on the main
screen; cheating?) or the money is frozen, the score (1010:0faa) goes into
the high scores with the net worth. Otherwise, if it's lost (1008:042b),
it ends without a score. Either way the original shows `schlussform`
(1068:1016); the port hands that to the view in `Tick::game_over`.

| Scenario | Won when | Lost when |
|---|---|---|
| 0 (normal) | the year is 2056 | bankrupt (then `*1110:6f24+0x80` := 1) |
| 1 Geld | net worth ≥ 50 000 000 | 2017 |
| 2 Urlauber | the last 12 months' guests ≥ 120 000 | 2017 |
| 3 Koenig | players 2–8 are all out (`*1110:6f24+0x7f+p`), player 1 not | 2017 |
| 4 Luxus | the guests (`+0x10`) of houses with a price (`+0x12`) over 320 ≥ 4000 | 2027 |
| 5 Surfer | the visitors (`+0xc`) of the surf schools (0x6d) ≥ 4000 | 2007 |
| 6 Oeko | the last 12 months' guests ≥ 12 000 and the plots' quality (`+4`) averaged by their `+0x352` ≥ 71 | 2017 |
| 7 (not on the CD) | from 2001, or net worth ≥ 1 000 000 | bankrupt |

- Net worth (1008:0c6e): the buildings' value (at least 0) plus money
  less loan, at most `MaxInt`. Money (1078:0191) is 99 999 999 while it's
  frozen or for a player not in the game. A building's value (1008:0a56)
  is `price * +1 div HAUSDATA column 4` if that column is positive, else
  `price * condition div 127`; a building site's is its price.
- Bankrupt (1008:0002): not out yet, net worth below `-MaxSchulden`, and
  for player 1 last year's books (kind 0x5e, 1070:13f1) negative too.
- The last 12 months (1070:1345): the books' history `*[1110:6f04]+0x24b`
  holds 120 months of each player's guests at `+0x1e14`, with the count
  at `+0x2d1c` and the start at `+0x2d1d` (its third ring,
  [settlement.md](settlement.md#the-books-tgeldmenuform-10701419)).
- The score (1010:0faa): the mean of net worth div 90 000, the 12 months
  div 1800 and the reputation, each clamped to 0..100; then with
  `1110:133c`, and again on easy (20) and normal (10), it drops by that
  much but not below half.

### Earthquakes (10a0:0d1c)

Ported in [`game::events`](../crates/game/src/events.rs). On the first
tick of each hour of an earthquake, 100 squares (`Random(250)` for `i`,
then `j`) whose building isn't in the set at `1110:1358` (12 palms, 36
beach life, 255 none) lose their people (1088:6975) and building
(`demolish(0, ...)`, 1078:280b, [above](#demolishing-1078280b));
player 1's count half their price (`HAUSPREI` column 0) as damage. The
flood (10a0:0b49) uses the same set, also on the building.

### Animations (step 9)

The "animations" are the people and vehicles on the island: 1601 records
of 0x28 bytes at `*1110:6fd6` (saved, unit 1088:8a07), using
`ANIMDAT*.DAT`, described in [people.md](people.md). They walk, enter
buildings and are counted, so they are part of the world, although the
original lets the player switch them off (`1110:0054`, `0055`, `0056`).
Adding, removing and moving them is ported in
[`game::people`](../crates/game/src/people.rs) and
[`game::people::movement`](../crates/game/src/people/movement.rs), drawing
them in [`game::darstellung`](../crates/game/src/darstellung.rs).

| Routine | Address | Size | Side |
|---|---|---|---|
| `people_tick`: counts down `1110:723c[1..42]` (how long each sound still plays: the view's), then for each record 0x24..0x640 `person_animate` (1098:109f, about 10 KB), `person_step` (1090:2ce5, 9498 bytes, the planes) and with `1110:0055` the walkers along their `ANIMDAT2` paths ([people.md](people.md#moving-10905293)); `Random`s for the helicopter, the surfers' and harbour ships' ways and the walkers' turns and sounds | 1090:5293 | ~3850 | world |
| `spawn_people`: new ships and boats, and people at the buildings the player sees, `Random` ([people.md](people.md#new-people-10902373)) | 1090:2373 | 2220 | world |
| Collect the people in view, put back what was under them and keep what's under them now | 1088:385d (1088:3496) | 103 | view |
| With a building tool (`1110:6056` in 6..21): the tool's preview, before and after | 1090:124f, 1090:155b | 780, 731 | view |
| Draw the people, and what's in front of them over them ([people.md](people.md#drawing-them-1088385d)) | 1088:38c4 (1088:3a19, 4285) | 219 | view |
| `Hauptbild.Refresh`, then more drawing | 1090:1836, 1090:1919 | 227, 256 | view |

### The buildings' tick (1078:0002)

Ported in [`game::construction`](../crates/game/src/construction.rs).
`building_slice(n)` spreads its work over 30 ticks: each house whose
`(i + j) mod 30` is `n` (as `Longint`s) draws `Random(100)` and sends
out a person if that's below `+0x10 * 100 div capacity` (`HAUSDATA.DAT`
column 3), probably its occupancy in percent; then each facility whose
`(i * j) mod 30` is `n` the same with its `+0x15` byte. Sending out
(`send_person`, 1090:1a25, 2154 bytes, [people.md](people.md#sending-someone-out-10901a25))
is the people's code: by the building type at the square it adds
someone (1088:0324) if there's none of that kind at its door
(1088:4ae2), some kinds picked with a `Random`. A house type with no
capacity stops the original with a division by zero; the port skips
the house.

## State

What the driver and the ported parts use, saved in the map file but
where the table says otherwise ([savegame.md](savegame.md#what-a-saved-game-loses)
has all that a saved game loses):

| Global | What | Map field |
|---|---|---|
| `[THauptschirm]+0x1d6`, `+0x1d8` | Ticks of the hour, of the day (`Word`) | `[1110:6532]+0x1d6`, `+0x1d8` |
| `1110:0096` | Speed: ticks per hour (`Longint`) | `1110:0096` |
| `1110:009e` | `building_slice`'s tick, 0–29 | `1110:009e` |
| `1110:0067` | Paused, while the TV shows a message (set by 1048:12e9, cleared by 1048:077a) | `1110:0067` |
| `1110:65d6` | The date object (`TMonate`, built by `monate_create`, 1030:000d): `+0` far pointer to the 12 month names (`string[15]`, `MONATE.TXT`), `+4` day, `+5` month (1–12), `+6` year (`Word`, 1996 at first) | `[1110:65d6]+0x4` .. `+0x6` |
| `1110:6f30` | Money, `array[1..8] of Longint` | `1110:6f30` |
| `1110:6f50`, `6f70` | Each player's loan and its monthly interest (1078:0466) | same |
| `1110:4a84`, `4a88` | Accounting step and last day | same |
| `1110:653a` | Days since the start, for the events | same |
| `*1110:6536` | The events | same (unit 1010:3e8a) |
| `1110:0086`, `0087`, `0088` | Flood, earthquake, oil slick going on (`0088` isn't saved) | `0086`, `0087` |
| `1110:0072`, `7050`, `7054`, `7056` | Insured, the disaster's damage, insured for it, the oil slick's square | same |
| `1110:6548`..`65a6`, `1110:0063` | The auction: bidder, price, item, calls, ... ([auction.md](auction.md#state-unit-10282506)); "Auktion wartet" | same (unit 1028:2506) |
| `1110:0902` | The players' names, `array[0..8] of string` (0 `keiner`) | same |
| `1110:0054`, `0055`, `0056`, `6056` | Animation options, the selected tool | same |
| `1110:008c`, `1110:1479` | Difficulty, number of players | same |
| `*1110:5524`, `*1110:5528` | The location map and the plot map, [below](#the-location-map-and-plots) | same |
| `*1110:6f0c` | The plots (count `1110:552c`) | list (unit 1068:313e) |
| `*1110:6f10` | The houses (count `1110:6f1a`) | list (unit 1068:2e05) |
| `*1110:6f24` | 0x88 bytes of per-player values; `+0x2f + player` the reputation | same |
| `*1110:705a`, `*1110:705e` | Sabotage slots, championships | same |
| `1110:64bf` | `MaxLageeinfluss` | same |
| `*1110:6f08` | The building sites (count `1110:6f18`) | list, reversed (unit 1068:32b5) |
| `*1110:6f14` | The facilities (count `1110:6f1c`) | list (unit 1068:32b5) |
| `*1110:6f20` | Sums over the island: plots with an airport or harbour, capacities by class ([above](#the-buildings-records-10780eda)) | same (unit 1078:34ad) |
| `1110:64ea`, `1110:6508` | `Strassenabrisskosten`, `Angeschlossenumrechnung` (`MODATYP.DAT`) | same |
| `*1110:6512` | The computer players' budgets, 13 `Longint`s for each of players 2–8 | same (unit 1010:0d4c) |
| `[1110:6f04]+0x191`, `*[..]+0x24b` | The money menu's books: kinds 0x52–0x6e, the monthly history ([settlement.md](settlement.md#the-books-tgeldmenuform-10701419)) | same (unit 1070:3bf8) |
| `1110:4a8a`..`4ac2`, `*1110:4c6e`..`*1110:4c7a`, `1110:004c` | `ABRECHNE.DAT`: rents, prices, factors; the base demand | same (unit 1008:539f) |
| `1110:4aee` | The month's sums, 12 × `array[1..8] of Longint` | same (unit 1008:539f) |
| `1110:64c6`, `64fa`, `64fe`, `*1110:6502`, `650e` | `Werbungskosten`, `Lebenshaltungskosten`, `Klassenmischung` and its divisor, `Verfallwahrscheinlichkeit` | same (unit 1010:0d4c) |
| `1110:6f90` | Each player's guests this month | same (unit 1078:34ad) |
| `1110:0057`, `1110:70c2` | The computer players play; their turns so far | same |
| `1110:7062`..`1110:70f2`, `1110:6f28`, `1110:634c`, `1110:00a2` | The computer players' state, the bank's no, the human's road drag ([computer.md](computer.md#state)) | same (unit 10a8:32ec) |
| `1110:1334`, `1110:64c2` | Renovation interval per player, `Renovierungskosten` | same |
| `1110:006e`, `006f`, `0071`, `133c` | Scenario, cheated(?), won, the score's unknown flag | same |
| `*1110:6f24+0x7f+p` | Player `p` is out | `*1110:6f24` |
| `*1110:6fd6`, `*1110:6fda`, `*1110:6fde`, `*1110:6fe2`, `1110:3c2e`, `1110:6ff4`, `1110:6fee`, `1110:6ff0` | The people and their tables, [people.md](people.md) | same (unit 1088:8a07) |
| `1110:007c`, `1110:5530`, `1110:5534` | The human's view as the people code reads it | same |
| `1110:723c` | How long each sound still plays (`array[1..42]`), the view's | not saved |
| `1110:4a06` | `RandSeed`: **not** saved; `Randomize` (1108:1e21) seeds it from the DOS clock when the program loads its data (1030:38a3, state 100); a new game's names, island and computers draw from it ([island-generator.md](island-generator.md)) | |

## The date (1030:000d..)

- `date_advance` (1030:00eb): the day goes up; past the month's length the
  day is 1 and the month goes up; past 12 the month is 1 and the year goes
  up. The lengths are a table at `1110:174c`, `array[0..3, 1..12] of
  Byte`, indexed by `year mod 4`: February has 29 days in every fourth
  year (1996, 2000, ...), with no century rule.
- `date_text` (1030:0148): `day` + suffix + `' '` + month name + `' '` +
  year. The suffix depends on `land` (`MODATYP.DAT`): `.` in Germany
  (`1. Januar 1996`), `st`/`nd`/`rd`/`th` in America and England (1, 21, 31
  `st`; 2, 22 `nd`; 3, 23 `rd`), nothing in France.
- 1030:228d draws it on the info bar: restores the bar from x = 400, then
  `draw_text` at (450, 1), 182 wide, right aligned, colour 10.

## Money (1078:01e7)

`add_money(player, amount)`:

- Does nothing if `1110:0070` is set (never, as far as the code shows) or
  `player` isn't 1 to `player_count`.
- Adds with saturation at the `Longint` limits.
- For player 1 shows the new amount on the info bar: `format_money`
  (1030:11a0) + `' '` + the currency (`SONSTIGE.TXT`'s sixth text, `DM`),
  drawn by 1030:2211: restores the bar's first 152 pixels, then
  `draw_text` at (4, 1), 120 wide, right aligned, colour 10, or 11 when
  the text starts with `-`.
- For a computer player paying (`amount < 0`): at difficulty 1 it then
  gets `amount div 6` back, at 2 `amount div 3`. Computers pay less on
  harder levels.

`format_money(n)` (1030:11a0) shows `n * Waehrungsfaktor`
(`SONSTIGE.TXT`, 10): a `-` for negative amounts, the digits worked out so
the product can't overflow, and a separator every three digits (1030:0fdd:
`.` in Germany, `_` in France, `,` otherwise).

Starting a scenario (state 1500, [startup.md](startup.md)) sets the loan
from the difficulty: `MaxSchulden` (`1110:64ce`) is doubled on easy and
halved on hard, `Startgeld` (`1110:64d6`) is half of it, the interest
rate `1110:64d2` is 6, and player 1's money and loan (1078:0466: interest
`loan div 24 * rate div 50`) are the start money.

## Palms (1010:129e)

Ported in [`game::vegetation`](../crates/game/src/vegetation.rs). A palm
is building type 12 on a single square. Its part is
`variant * stages + stage`: `stages` is the palm's part count in
`HAUSNR.DAT` (10), `variant` the ground tile's variant the square had
before (the parts layer holds it for empty ground), and the stage is 8 for
a young palm, 9 for a grown one and 0–7 for the eight looks of an old one.

`vegetation_tick` picks 30 squares, `i := Random(250)`, `j := Random(250)`
(so never row or column 250), and for each:

- A palm, by stage: 8 becomes 9; 9 becomes `Random(8)`; 0–7 dies if
  `Random(100) < 60`: the part goes back to the variant, the building to
  255, and the palm's effect comes off the location map (1030:1dc6). The
  square is redrawn either way (1050:21f9).
- An empty square whose ground is in the set at `1110:13b8` (0, 1, 4: sand,
  grass, sand to grass), whatever its height: `plant_palm` (1010:1129) if
  `Random(100) < 30`. That adds one to the palms counted on the square's
  plot (`+0x354` of the record 1068:2591 finds for 1030:1e62's plot
  number; dying doesn't take it off), sets the building, adds the palm's
  location effect (1030:1d10) and makes the part `part * stages + 8`. It
  redraws the square on the map and the overview map (10b8:1624).

So a square's palm comes and goes over the years, and the island fills
with palms until the deaths catch up with the planting.

### The location map and plots

`*1110:5524` is the location quality (`Lage`), 126 × 126 bytes, one cell
per 2 × 2 squares, `[i div 2][j div 2]`, around 100.
`change_location(i, j, value, radius, remove)` (1030:1ae9; 1030:185d also
redraws that part of the overview map) multiplies every cell within
`radius` (Manhattan distance `d`, in cells) of `(i div 2, j div 2)` by
`f = (value - 100) * (radius + 1 - d) div (radius + 1) + 100` percent, as
`(cell * f + 50) div 100` clamped to 2..255. With `remove` it uses
`(f div 2 + 10000) div f` instead, which undoes the change up to rounding.
A building type's effect (1030:1d10 adds, 1030:1dc6 removes) is centred on
`(i + width div 2, j + depth div 2)` (`HAUSNR.DAT` columns 3 and 4) with
`value` from `HAUSDATA.DAT`'s last column (palms 106) and a radius of
`|value - 100| div 6`, at least 1 and at most `MaxLageeinfluss div 2`
(`1110:64bf`, 30 in the scenarios).

`*1110:5528` gives each square its plot (`Grundstück`): 126 × 251 `Word`s,
`[i div 2][j]`, 0 for none. 1030:1e62 reads it, and for a square with 0
looks at the neighbour its form (0–3) points to. The plots themselves are
the list at `*1110:6f0c` (0x364-byte records, `+0` the number, `+2` its
airports and harbours, `+0x22`, `+0x4e`.., `+0xc6`.. capacities,
`+0x354` the palms planted and park squares, `+0x356` the other
buildings' squares, `+0x358` and `+0x35c` lists of its houses and
facilities, `+0x360` the next), saved by 1068:313e.

## Beach life (1008:1b4b)

Ported in [`game::beach_life`](../crates/game/src/beach_life.rs). The
finished buildings ("houses") are a list at `*1110:6f10` of 0x2c-byte
records (unit 1068:2e05): `+3`, `+5` the building's first square, `+8`
the next record, `+0x10` a value not identified yet (guests?), `+0x13 +
4n` the squares of its beach life `n` = 1..5 and `+0x2b` how many it has.
Beach life is building type 36 (`Strandleben`): people on the beach.

`beach_life_tick` goes through the list, and for each house with
`Random(30) = 0` takes `beach_life_step(class)` (1008:1aea), the class
being 0 for a `+0x10` of 0–2, 1 for 3–10, 2 for 11–30, 3 for 31–60, 4
for 61–110 and 5 otherwise (negative included):

- below the class: `add_beach_life` (1008:19ea): `find_beach_square`
  (1008:1848) looks through rings 1 to 6 around the building (its size
  from `HAUSNR.DAT`), first down the columns to its left and right, then
  along the rows above and below, for a square on the building's plot
  that is flat (form 16) sand (ground 0) one step above the sea (height 1)
  without building or owner (1008:179f). There `place_beach_life`
  (1010:11cd) puts type 36 with part `Random(hausnr[36][0])` (10 looks),
  and the house remembers it as its next square.
- above it: `remove_beach_life` (1008:1a52) clears its last square
  (1010:122f: building 255, part `Random(1)`, owner 0), whatever is on it
  by then.
- at it, `Random(5)`: 0 adds one (if the class is below 5), 1 removes one
  (if above 0), else (if above 0) `change_beach_life` (1008:1a81) gives
  square `Random(n) + 1` the part `Random(hausnr[36][0])`, again whatever
  is on it.

## Sabotage and championships (10a0:117e)

Ported in [`game::sabotage`](../crates/game/src/sabotage.rs), except
the callees named below. `effects_tick` goes through two tables, both
saved (unit 10a0:23c9). For each active entry it does what the day of
its countdown calls for, then counts down (a `Word`): at 0 the entry
ends instead.

The sabotage slots (`*1110:705a`, 5 × 10 bytes: attacker, victim, kind,
`i`, `j`, countdown, active) by kind (`SABOTAGE.TXT`'s order) and day:

| Kind | Day | Does |
|---|---|---|
| 0 piranhas | 120; 0 | location × 60 %, radius 1; undone |
| 1 bomb | 9 | message 0x17 (attacker, victim) |
| 2 stink bomb, 4 rats | 90; 0 | like the piranhas |
| 3 shark diver | 119; 118; 0 | message 4 (attacker); location × 60 %, radius 10; undone |
| 5 soft soap | 2; 1 | countdown := 1; message 0x42. The countdown then goes on to 0 the same day, so the message never comes |
| 6 go-slow | 10 | if the square holds a building site (type 31), 1078:1995 finds it in `*1110:6f08` and its days left go up by its type's building time (`HAUSDATA.DAT` column 0); otherwise the countdown is set to 0 and the sabotage ends |
| 7 character assassination | 10 | the victim's reputation (`*1110:6f24 + 0x2f + player`, 50 at first) goes down by 50, to at least 0 |

The championships (`*1110:705e`, 3 × 8 bytes: kind, `i`, `j`, countdown,
active) only act while their building is on the square, the owner being
the square's:

| Kind | Building | Days |
|---|---|---|
| 0 tennis | 0x6f empty, 0x70 full | 0x37 full (10a0:0026); 0x36 location × 160 %, radius 1; 0x35 owner's reputation + 50 (at most 100); 0x34 message 0x58; 2 location undone; 1 empty |
| 1 surfing | 0x71–0x74 empty, 0x75–0x78 full | 0x38 full; 0x37 location; 0x36–0x31, 0x24, 0x1e, 0x14, 10 surfers (10a0:10c6); 0x2b reputation + 50; 0x2a message 0x57; 3 location undone; 2 the surfers leave (1088:2d2a); 1 empty |
| 2 Miss Holiday Island | 0x79, 0x7a empty, 0x7b, 0x7c full | 0x19 full; 0x18 location; 0x17 reputation + 30; 0x14 message 0x59; 2 location undone; 1 empty |

The championship messages scroll the view to the building first.
`replace_building` (10a0:0026), its first part the spectators leaving an
emptied tennis court (people of kinds 0x4d–0x50) or stage (0x41) (port
in `game::people`): the type goes on the area and into the record
(1078:169a); a full building (`1110:1418`) gets `min(plot+0xee,
capacity)` at `+0xc` and that in percent of the capacity at `+0x15`,
an empty one (`1110:1438`) 0 for both. `send_surfers` (10a0:10c6, five
tries with a `Random(2)` each) and `remove_people_at` (1088:2d2a) are in
`game::people`.

## The news (1048:0a54)

`post_message(number, a, b, x, y, j, i, centre)` writes an entry into the
news archive (`*1110:663a`, 26 records of 0x11c bytes, saved by unit
1048:25ea): the day count, the four values, the text
`number * 3 + Random(3)` (`NACHRICH.TXT` has three texts per message),
the number, the date's text, the square and the view's origin. Then, by
the message's kind of news (`1110:199e[number]`, 0–4) and the player's
options for it (`1110:005c`..`005f`), it scrolls the view to the square
(with `centre`) and opens the TV (1048:12e9), which pauses the game. In
the port the world draws the `Random(3)` and hands the message to the
view in `Tick::messages` (`game::news`); the archive and the TV aren't
ported.

## Events (1010:3084)

Ported in [`game::events`](../crates/game/src/events.rs), except the
callees named below. The events are 41 records of 0x34 bytes at
`*1110:6536` (saved by unit 1010:3e8a): `EREIGNIS.DAT`'s twelve
`Longint`s per row (1010:3ab4; starting scenario *n* reloads them from
`EREIGNn.DAT`, 1010:36f8, not ported yet) and a 13th, `+0x30`, read as a
square (`i`, `j` words). The columns:

| Offset | Column | What |
|---|---|---|
| `+0` | 0 | Chance out of 65000; set to 0 when the event starts, so it never comes again |
| `+4`, `+8` | 1, 2 | The window of days (`1110:653a`) it may start in |
| `+0xc` | 3 | Days running, 0 while it isn't |
| `+0x10` | 4 | Duration |
| `+0x14` | 5 | A price or amount: the message's value, the auction's price |
| `+0x18` | 6 | Start message; the auction's item; the disaster (5 oil slick, 6 earthquake, 0x2b flood) |
| `+0x1c` | 7 | End message; the item for kind 1 |
| `+0x20` | 8 | Not used here |
| `+0x24` | 9 | The building type for kind 6 |
| `+0x28` | 10 | The kind, below |
| `+0x2c` | 11 | A value for the start message |

`events_tick` adds one to the day counter, then for each event in turn:
if it isn't running and the day is in its window (signed `Longint`s),
`event_chance` (1000:3d9e) draws `Random(65000)` and says yes when the
chance is 65536 or more, or at least 1 and above the number plus 1. Then
the chance becomes 0 and, by kind:

- 0: the start message, and `start_auction(start, price)` (1028:08ac:
  `1110:654a` and `659c` the price, `656e` the item, and
  `TAuktionsform`'s timer starts the auction, [auction.md](auction.md)).
- 1: the start message; it runs.
- 2: only if disasters are on (`1110:0065`), and the event is used up
  either way: it runs, and by `start` a flood (`1110:0086` := 1,
  10a0:0b49, the island redrawn), an earthquake (`1110:0087` := 1,
  10a0:0d09, the main screen's position kept for later) or an oil slick
  (`1110:0088` := 1, 10a0:074d, which also gives the message's square).
  Then the start message.
- 3, 5: the start message at the event's square (5 scrolls there); it
  runs.
- 4: the start message.
- 6: the start message with no values, and `make_available(building, 0)`
  (10a0:0691).

The messages without a square of their own get the view's origin. A
running event counts its days, and on the day they reach its duration:

- 1: the end message, and an auction of the `end` item. It keeps
  counting, and never ends again.
- 2: if a flood, earthquake or oil slick is going on (any of them), the end
  message, and the disaster by `start` ends: the flag back to 0 and
  10a0:0cce (flood, with a redraw), 10a0:0ddc (earthquake, with the
  screen put back) or 10a0:0915 (oil slick).
- 3, 5: an auction of the `start` item, and it stops running.

The disasters keep their damage at `1110:7050` (`Longint`), and at their
start copy `1110:0072` (probably the player's insurance) to `1110:7054`;
at the end an insured player 1 gets the damage back (`add_money`, and the
money menu's books, 1070:1419). The flood (10a0:0b49) goes through every
square, `i` then `j` from 0 to 250: one at sea level (lowest corner
below 2) whose building isn't in `1110:1358` is demolished for no one,
player 1's counting half their price as damage; then, still at sea
level, the people on it leave if `layer_551c` or `layer_5520` isn't 255
there (`flood_people`, in `game::people`). Not ported yet: the oil slick (10a0:074d and 10a0:0915: a random
sea square with up to 2000 `Random(250)`s and a random walk of oil, the
location map × 50 % within 30 cells), the books, and `make_available`'s
build menu (still a form's; the computer players' part is ported,
[computer.md](computer.md#state)). While an oil slick
would start, the port's random numbers part from the original's.

## Random

`Random(n)` (1108:1d8c) steps `RandSeed` (`RandSeed * 0x08088405 + 1`,
1108:1de9) and returns `(RandSeed * n) shr 32` with `n` as a `Word`.
The port's `World` has its own `Random` (`vcl::random`), seeded by whoever creates the
game. Every `Random` the world's routines call must come from it, in the
original's order. The earthquake's shaking (step 7) also calls `Random`
in the original, between `building_slice` and `quake_damage`; the port
gives that to the view, so during a quake the port's numbers part from
the original's.

## Port status

Ported: the driver's world steps in their order (`World::tick`), the date,
`add_money`, `set_loan`, `format_money`, the start money, the accounting
scheduler (1008:5307) with the monthly settlement (1008:1bf7, all its
steps and the books' month, [settlement.md](settlement.md)), the
computer players' turns ([computer.md](computer.md)) with the build check
and new building sites, `construction_tick`, `year_end`, `january_second` with the scenarios' goals, the
net worth and the score, `quake_damage`,
`vegetation_tick` with the location map,
`beach_life_tick`, `effects_tick`, `events_tick` (with stubs for the oil slick and new building
types), the buildings' records (`register_building`,
`unregister_building`, `find_record`), `demolish` with the road pieces,
`building_slice` with `send_person`, the flood's
demolition and its people, `replace_building`, the news' random text,
and the people ([people.md](people.md): `people_tick` with
`person_animate` and `person_step`, `spawn_people`, `send_person`,
`add_person` and the removal helpers; their records and tables are
loaded from the map, or set up for a new game). A normal game's world,
the generated island and everything the game's data sets up, is
`hintergrund::new_world` ([island-generator.md](island-generator.md)).
The tables the
simulation reads (`HAUSNR.DAT`, `HAUSDATA.DAT`, `HAUSPREI.DAT`,
`GROUNDNR.DAT`, `ANIMDAT1.DAT`, `ANIMDAT4.DAT` and `DARSTELL.DAT`'s
`AnsichtFormkorrektur`) are in `World::tables`, loaded from the CD by
whoever starts the game. The human's view (rotation, origin, parity),
which the people code reads, is in `World::viewpoint`; the view sets it
before each tick (marked).

What the view shows comes out of `World::tick` in `Tick`: the squares
that changed (`redraw`; the view redraws the whole island for them rather
than the columns 1050:21f9 picks, and has no overview map yet), a flood
(`redraw_all`), the news (`messages`), a started auction (`auction`) and
what the auction form did (`auction_changes`), the people's and the
auction's sounds (`sounds`, not played yet), the end of the game
(`game_over`), the money menu's and `TUmenuform`'s refreshes (`books`,
`umenu`) and the buildings whose occupancy changed (`occupancy`);
finished buildings are in `redraw`. The people's pictures
are in their records, which the view draws over the island after each
tick (`Hauptschirm::show_people`, 1088:385d, 1088:38c4).
The view draws the date and money on the info bar when they change, and
the auction form ([auction.md](auction.md)), whose timers
(`World::auction_tick`) run before the clock's steps each tick, and the
file menu, which stops the clock while it's open ([savegame.md](savegame.md)); the
news, the TV, the status bar timeout, tooltips, scrolling by the timer,
music and the shaking aren't ported. The auction's buttons and loading a
game from the file menu are the only commands so far (`World::apply`).
`World::to_map` saves the world ([savegame.md](savegame.md)).

Stubs (named after the table above, each with its address and a TODO):
inside the ported routines `oil_slick_start`, `oil_slick_end`.
Raising and lowering the land (`game::terrain`) and building roads
(`game::roads`) are ported as world code the computers use
([computer.md](computer.md#levelling-and-roads)). `make_available` only gives the computers the type; the
build menu's part is the view's. 1088:7fb1,
which runs `spawn_people` again after the player scrolls or turns the
view, isn't ported. Demolishing refreshes the money menu (1070:2617) in the original,
which the port leaves to the view. Where a stub
stands for code that draws random numbers, the port's numbers part from
the original's once it runs.

## Open questions

- The auction form stops the simulation for everyone while the human
  bids, and with "Auktion wartet" until they start the auction
  ([auction.md](auction.md#open-questions)).
- Several things that change the world wait on the player's UI: the TV
  pauses the game (`1110:0067`) until its film ends, and the animation
  options (`1110:0054`..`0056`) decide whether people move at all. For
  multiplayer these need to become world state everyone agrees on.
- `1110:0070` stops all money changes; nothing found sets it. `1110:133c`,
  which a scenario sets on easy, is unknown.
- The computers' roads ignore the ground the human's last road drag
  started on (`1110:704e`), which the original lets them use
  ([computer.md](computer.md#levelling-and-roads)).
- The sounds' countdown `1110:723c` (people_tick's first loop) stops a
  sound from playing again while it still plays; the port leaves it to
  the view, which doesn't play the sounds yet.
- The people code reads the human's view (`World::viewpoint`): where
  people appear, their pictures, and the sounds' `Random`s. 1088:7fb1
  also runs `spawn_people` from the UI. Both need a multiplayer answer
  ([people.md](people.md)).
- The news (1048:0a54) pauses the game through the TV for the human
  player, depending on their options per kind of news; the archive also
  stores that player's view origin. Both need a multiplayer answer.
- The houses' `+0x10` is their guests ([settlement.md](settlement.md));
  `1110:0072` is the insurance (a 120th of the buildings' value a month).
- `demolish` reports the error messages for player 1 as a `Result`; a
  player's demolition isn't a command yet, so only the disasters call it.
  The floating cost (1088:0153) is a people record in the original but
  only for the human: it may belong to the view. The port keeps it in
  the world, as the original does.
- A full championship building takes its visitors from its plot's
  guests (`+0xee`), at most its capacity. `*1110:6f20 + 0x19a` (the
  economy) is 127 from a new game on, and nothing changes it
  ([settlement.md](settlement.md#open-questions)).
- Demolishing a building site doesn't take its effect (type 31's 90 %)
  off the location map. A demolished park comes off its plot's
  `+0x354`, a dead palm doesn't. The original's bridge demolition costs
  nothing, though it adds up a cost.
- The plots' lists of houses and facilities hold the buildings' first
  squares in the port, not pointers; the settlement finds the records by
  them.
- The books' totals (kinds 0x5e, 0x6b) are worked out by 1070:1452, which
  the original runs at the year's end and whenever the money menu is
  shown (1070:2617, only with the form's height above 0): a view
  condition in world state. The port only works it out at the year's end.
- What the books' 0x6c–0x6e are (a new game starts them as the human's
  money, `-loan` and their difference), and what `1110:133c` and
  `1110:006f` mean. The plots' `+0x352` is their area in land corners
  (1068:25fd).
- The high score entry also stores a floating-point value made by
  1100:1142/1100:1808 from `1110:0a02` (the date?); that's left to the
  view. A scenario's `*1110:6f24+0x80` out flags come from its map; state
  1500 may change them (not ported).
- Renovations only charge player 1; computer players' buildings get
  renovated for free (their turns don't pay for it either). The
  settlement likewise charges only the human for advertising, insurance
  and the cost of living.
- 1010:1129 counts palms on the square's plot through a nil pointer when
  the square has none; the CD's maps and generated islands have a plot
  on every land corner, but a square whose corners are all in sea rows
  of the plot map takes its neighbour's by its form (1030:1e62), which
  may be none. The port skips the count then.
