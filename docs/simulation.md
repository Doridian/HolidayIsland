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
to scroll while the mouse is held on a scroll bar arrow (step 6).

| # | Does | Address | Size | Side |
|---|---|---|---|---|
| 1 | Re-enables the 17 forms in `1110:646c` except 10 and 11, unless `infobox2+0x1e6` is set or `schlussform` is visible | 1010:2a0c | 78 | view |
| 2 | Status bar message timeout: if `1110:009a` > 0, counts it down; at 0 clears the status bar (1030:22de with `''`). Messages set it to 20 (1030:232d, 1030:235a) | inline | | view |
| 3 | With tooltips on (`1110:0060`): the help bubble (`helpform`, 1040:3dc7 / 1040:3ead) for the money menu's buttons (1070:0294), the overview map's (10b8:02e9) and the button bar (1018:394e) under the mouse | inline | | view |
| 4 | Unless paused (`1110:0067`): the game clock, [below](#the-clock-step-4) | inline | | world |
| 5 | `1110:009e := (1110:009e + 1) mod 30`, then `building_slice(1110:009e)` | 1078:0002 | 330 | world |
| 6 | Scrolls the view by the timer's `Tag`: 1–4 `scroll_ns` by −1, −16, 16, 1; 5–8 `scroll_wo` by −1, −18, 18, 1 | 1020:0cf7, 1020:147f | | view |
| 7 | During an earthquake (`1110:0087` = 1): moves the form by `Random(7) - 3` in x and y; on the first tick of each hour also `quake_damage` | inline; 10a0:0d1c | 192 | view; world |
| 8 | With the TV (`fernseher`) visible: its tick, which plays the news film and closes it after 3 s (`GetTickCount`) | 1048:1b3e | 1107 | view |
| 9 | Without animations (`1110:0054` = 0) just refreshes `Hauptbild`. With them: `people_tick`, and with `1110:0056` the rest of the animation group, [below](#animations-step-9) | 1090:5293 | ~3850 | world and view |

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
  245 bytes: counts down the building sites in the list at `1110:6f08`
  and finishes the ones at 0, 1078:203d) and remember the day.
- Otherwise: the step counter goes up. Past 12 on the 1st of a month it
  goes back to 0, and in January that is the new year (`year_end`,
  1008:4f64, 686 bytes). In January at step 5 (on the 2nd), 1008:0512
  (199 bytes). Then, every time, `settlement_step(step)` (1008:1bf7).

So the step runs from 0 (the 1st) to about 150 over the month, five a
day. `settlement_step` (1008:1bf7, **13165 bytes**, the `ABRECHNE` unit)
is the monthly settlement spread over the month: a chain of `if step = n`
for even `n` from 2 to about 0x50 (plus a few odd ones), each doing one
part of the books for all players (clearing the month's sums at
`1110:4aea`.., guests, rents, costs, interest, the computer players' turns
through 1070:1419 and 1010:2493, the occupancy map at 0x48 ...). Its main
callees are 1008:0de9, 1008:0d10, 1008:15dd, 1008:097f, 1008:09c8,
1008:06b6, 1070:1419, 1070:2617, 1010:2493, 1010:3006. Money goes through
`add_money` (1078:01e7).

### Animations (step 9)

The "animations" are the people and vehicles on the island: 1601 records
of 0x28 bytes at `*1110:6fd6` (saved, unit 1088:8a07), using
`ANIMDAT*.DAT`. They walk, enter buildings and are counted, so they are
part of the world, although the original lets the player switch them off
(`1110:0054`, `0055`, `0056`).

| Routine | Address | Size | Side |
|---|---|---|---|
| `people_tick`: counts down `1110:723c[1..42]`, then for each entity 0x24..0x640: 1098:109f (not a function in Ghidra yet) and `person_step` (1090:2ce5, 9498 bytes), and with `1110:0055` the movement along `ANIMDAT2` paths | 1090:5293 | ~3850 | world |
| `spawn_people`: new guests and cars, `Random` | 1090:2373 | 2220 | world |
| Erase the sprites | 1088:385d | 103 | view |
| With a building tool (`1110:6056` in 6..21): the tool's preview, before and after | 1090:124f, 1090:155b | 780, 731 | view |
| Draw the sprites | 1088:38c4 (1088:3a19, 4285) | 219 | view |
| `Hauptbild.Refresh`, then more drawing | 1090:1836, 1090:1919 | 227, 256 | view |

`building_slice` (step 5, 1078:0002) spreads work over 30 ticks: each
building whose `(x + y) mod 30` is the tick's number probably sends out
a person (1090:1a25, not a function in Ghidra yet) with a chance from `HAUSDATA.DAT` (`Random(100)`).

## State

What the driver and the ported parts use, all saved in the map file:

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
| `1110:654a`, `659c`, `656e` | The auction's price, start price, item | same (unit 1028:2506) |
| `1110:0054`, `0055`, `0056`, `6056` | Animation options, the selected tool | same |
| `1110:008c`, `1110:1479` | Difficulty, number of players | same |
| `*1110:5524`, `*1110:5528` | The location map and the plot map, [below](#the-location-map-and-plots) | same |
| `*1110:6f0c` | The plots (count `1110:552c`) | list (unit 1068:313e) |
| `*1110:6f10` | The houses (count `1110:6f1a`) | list (unit 1068:2e05) |
| `*1110:6f24` | 0x88 bytes of per-player values; `+0x2f + player` the reputation | same |
| `*1110:705a`, `*1110:705e` | Sabotage slots, championships | same |
| `1110:64bf` | `MaxLageeinfluss` | same |
| `1110:4a06` | `RandSeed`: **not** saved; `Randomize` (1108:1e21) seeds it from the DOS clock when the game's data is loaded (1030:38a3) | |

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
the list at `*1110:6f0c` (0x364-byte records, `+0` the number, `+0x354`
the palms planted, `+0x360` the next), saved by 1068:313e.

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

The championship messages scroll the view to the building first. Not
ported yet: `replace_building` (10a0:0026, which also updates the
building's record and sends people away), `send_surfers` (10a0:10c6,
five `Random(2)`s and the people), `remove_people_at` (1088:2d2a) and the
go-slow's building site record. They're stubs, so a championship's
building doesn't fill yet and the random numbers part from the
original's while surfers would be sent.

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
  `TAuktionsform`'s timer starts the auction).
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
money menu's books, 1070:1419). Not ported yet: the flood's demolition
(10a0:0b49 knocks down everything at sea level and counts player 1's
buildings as damage), the oil slick (10a0:074d and 10a0:0915: a random
sea square with up to 2000 `Random(250)`s and a random walk of oil, the
location map × 50 % within 30 cells), the books, and `make_available`
(the build menu's list of types is still a form's). While an oil slick
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
scheduler (1008:5307), `vegetation_tick` with the location map,
`beach_life_tick`, `effects_tick` (with stubs for the buildings and
people it changes), `events_tick` (with stubs for the flood's
demolition, the oil slick and new building types), and the news' random
text. The tables the simulation reads (`HAUSNR.DAT`, `HAUSDATA.DAT`) are
in `World::tables`, loaded from the CD by whoever starts the game.

What the view shows comes out of `World::tick` in `Tick`: the squares
that changed (`redraw`; the view redraws the whole island for them rather
than the columns 1050:21f9 picks, and has no overview map yet), a flood
(`redraw_all`), the news (`messages`) and a started auction (`auction`).
The view draws the date and money on the info bar when they change; the
news, the TV, auctions, the status bar timeout, tooltips, scrolling by
the timer, music and the shaking aren't ported.

Stubs (named after the table above, each with its address and a TODO):
`construction_tick`, `year_end`, `january_second`, `settlement_step`,
`building_slice`, `quake_damage`, `people_tick`, `spawn_people`, and
inside the ported routines `replace_building`, `send_surfers`,
`remove_people_at`, `delay_construction`, `flood_start`'s demolition,
`oil_slick_start`, `oil_slick_end` and `make_available`. Where a stub
stands for code that draws random numbers, the port's numbers part from
the original's once it runs.

## Open questions

- Several things that change the world wait on the player's UI: the TV
  pauses the game (`1110:0067`) until its film ends, and the animation
  options (`1110:0054`..`0056`) decide whether people move at all. For
  multiplayer these need to become world state everyone agrees on.
- `1110:0070` stops all money changes; nothing found sets it. `1110:133c`,
  which a scenario sets on easy, is unknown.
- `settlement_step`'s individual steps and the people records still need
  mapping before they can be ported. Many of the stubs above wait on the
  people.
- The news (1048:0a54) pauses the game through the TV for the human
  player, depending on their options per kind of news; the archive also
  stores that player's view origin. Both need a multiplayer answer.
- The houses' `+0x10` value, which sets their beach life, isn't
  identified (guests?), nor is `1110:0072` for sure (insurance?).
- 1010:1129 counts palms on the square's plot through a nil pointer when
  the square has none; the CD's maps have plots on all land, but a
  generated island may not. The port skips the count then.
