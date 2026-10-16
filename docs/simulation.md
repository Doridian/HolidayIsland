# The simulation

How game time moves, and what runs on each tick. The driver is ported in
[`game::simulation`](../crates/game/src/simulation.rs), the date in
[`game::monate`](../crates/game/src/monate.rs) and the money in
[`game::money`](../crates/game/src/money.rs). The rest of this page is the
map later passes port the subsystems from.

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
   | 1 | `events_tick` | 1010:3084 | 809 | Counts `1110:653a` (days since the start) and runs the scenario's events (`EREIGNIS.DAT`/`EREIGNn.DAT`, 41 records of 0x34 at `*1110:6536`): starts the due ones with a chance (`Random(65000)`, 1000:3d9e), shows their message (1048:0a54, 2197 bytes, the TV), applies effects (1028:08ac), quakes, floods (`1110:0086`, `0087`, `0088`), ends them after their duration |
   | 2 | `effects_tick` | 10a0:117e | 1495 | Counts down the five timed effects at `*1110:705a` (10 bytes each) and the three at `*1110:705e` (8 bytes): sabotage and disasters, which change the location map (1030:185d), demolish (10a0:0026), show messages, change ratings (`*1110:6f24+0x2f`) |
   | 3, 5 | `vegetation_tick` | 1010:129e | 404 | 30 random squares (`Random(250)` twice): a palm grows to its next stage, seeds a neighbour or dies (60%); an empty square of the right ground gets a palm (30%, 1010:1129). Redraws the square (1050:21f9) |
   | 4 | `ratings_tick` | 1008:1b4b | 172 | For each building in the list at `1110:6f10` (0x2c-byte records), one chance in 30 (`Random(30)`) to move its rating (`+0x2b`) a step towards the class its `+0x10` value falls in (1008:1aea, 97 bytes, `Random(5)`) |

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
| `1110:0054`, `0055`, `0056`, `6056` | Animation options, the selected tool | same |
| `1110:008c`, `1110:1479` | Difficulty, number of players | same |
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
scheduler (1008:5307). Stubs (named after the table above, each with its
address and a TODO): `events_tick`, `effects_tick`, `vegetation_tick`,
`ratings_tick`, `construction_tick`, `year_end`, `january_second`,
`settlement_step`, `building_slice`, `quake_damage`, `people_tick`,
`spawn_people` (`events_tick` already counts `1110:653a`). The view draws
the date and money on the info bar when
they change; the status bar timeout, tooltips, scrolling by the timer,
music, the TV and the shaking aren't ported.

## Open questions

- Several things that change the world wait on the player's UI: the TV
  pauses the game (`1110:0067`) until its film ends, and the animation
  options (`1110:0054`..`0056`) decide whether people move at all. For
  multiplayer these need to become world state everyone agrees on.
- `1110:0070` stops all money changes; nothing found sets it. `1110:133c`,
  which a scenario sets on easy, is unknown.
- `settlement_step`'s individual steps, the events' record layout, and
  the people records still need mapping before they can be ported.
