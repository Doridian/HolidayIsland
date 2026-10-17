# The monthly settlement (1008:1bf7)

Ported in [`game::settlement`](../crates/game/src/settlement.rs), the
books' month in [`game::accounts`](../crates/game/src/accounts.rs).
`settlement_step(step)` (1008:1bf7, 13165 bytes, the `ABRECHNE` unit) is
called by the accounting scheduler (1008:5307,
[simulation.md](simulation.md#accounting-10085307)) with the low word of
its step, which starts at 0 on the 1st of each month and goes up five
times a day. Each step does one part of the month for every player, so
the month's books are done by about the 28th. Steps the chain below
doesn't name (the odd ones, 0, past 0x8b ...) give the computer players
a turn.

The guests come in three stages: each house's demand from its plot and
its owner, the guests the plot's airports and harbours let in shared
among its houses, then the guests' wishes (by the class of their rent)
shared among the shops and other buildings. Money is paid three times a
month: costs on the 1st, the houses' rent around the 7th, the shops'
takings around the 16th.

## The tables

All of them are saved with the map (units 1008:539f and 1010:0d4c), so
a scenario can change them.

- `ABRECHNE.DAT` (1008:55ee, `formats::data::abrechne`): 38 factors of
  four `Longint`s each (weight `w`, divisor `d`, low, high; the four
  columns are four arrays, `*1110:4c6e` to `*1110:4c7a`), `MinMiete`,
  `MaxMiete` and their difference (`1110:4a8a`..`4a92`), and `MinPreis`,
  `MaxPreis` (`1110:4a96`, `1110:4ac2`, 11 `Longint`s each in the map; the
  11th `MaxPreis` is the first of the sums below).
- `MODATYP.DAT`'s `Werbungskosten` (`1110:64c6`), `Lebenshaltungskosten`
  (`64fa`), `Klassenmischung` (`*1110:6502`, 10 × 10 `Integer`s) and its
  divisor (`64fe`), `Verfallwahrscheinlichkeit` (`650e`, 3 bytes).
- `1110:004c`, a byte only read from the map: the base demand.

The factors work through three helpers, all `Longint` arithmetic:

| Routine | Does |
|---|---|
| `scale(value, n)` (1008:0d10) | `value` on factor `n`'s scale from low to high, as 0–100: `(value - low) · 100 div (high - low)`, or just clamped when the scale is 0–100 |
| `weigh(acc, value, n)` (1008:0de9) | `acc` moved by a rating (50 neutral): `(acc · (d - w) + acc · w · value div 50) div d`, clamped to 0–100 |
| `approach(acc, value, n)` (1008:0ea1) | `acc + ((value - acc) · w + d div 2) div d`, clamped |

and a few more:

| Routine | Does |
|---|---|
| 1008:097f | A rent's class, 0–9: `(rent - MinMiete) · 10 div (MaxMiete - MinMiete)` |
| 1008:0829, 1008:0863 | The fair rent for a house's stars, `MinMiete + stars · range div 100`, and how fair its rent is: `r = fair · 50 div rent`, `r` from 50 on, else `(r - 25) · 2`, clamped to 0–100; 100 for no rent |
| 1008:0709, 1008:0776 | The same for a facility's price per visitor and day, by its class's `MinPreis`/`MaxPreis` |
| 1008:0907 | A facility's price class, 0–9: `(price - MinPreis) · 10 div MaxPreis - MinPreis`. The original divides by `MaxPreis`, not by the range, and takes `MinPreis` off again |
| 1008:06b6 | `a · 50 div b` clamped to 0–100 when `b > 0`, else 0: guests per place |
| 1078:0780 | The class mixing: `out[k] = Σc mix[k][c] · in[c] div divisor`, the remainders carried on through all 100 terms so nobody gets lost |

`ABRECHNE.DAT`'s headings name the factors: 0–2 the houses' stars
(condition, connection, location), 3–7 their surroundings (the plot's
quality, its quality for the class, reputation, advertising,
economy), 8–9 their occupancy (rent, surroundings); 10–12 the other
buildings' stars, 13–17 the shops' surroundings (guests per place, the
price class's guests per place, reputation, advertising, economy), 18–21
the rest's, 22–23 their occupancy (price, surroundings); 24–31 a plot's
quality (greenery, shop prices, leisure, sport, nature, energy, water,
rubbish), 32–35 its quality by class (shop prices, restaurants, shops,
rentals), 36–37 a player's reputation (prices, places).

The facility classes (`TKommerzsachen`, 1010:2493) are: 0 restaurants
(`Gastronomie`), 1 shops, 2 rentals (`Auto`), 3 sport, 4 leisure, 5
nature, 6 transport (airports, harbours), 7 energy, 8 water, 9 rubbish,
10 none. Classes 0–2 are the record kind 1 ("shops" here), 3–10 kind 2.

## The month's sums (`1110:4aea`)

Twelve `array[1..8] of Longint` from `1110:4aee` (saved), in this order:
the takings of houses (`HAUSDATA.DAT` kind 0), hotels (kind 1),
restaurants (kind 2) and the other buildings; their running costs
(`HAUSPREI.DAT` column 1) in the same order; advertising; the rest
(insurance and the cost of living, negative); interest; the month's
total. A building of nobody (owner 0) adds into player 8's slot of the
sum before (and into the 11th `MaxPreis` for the houses' rent), as in
the original.

## The books (`TGeldmenuform`, 1070:1419)

The human's sums go into the money menu's books, kinds by
`GELDMENU.TXT`'s lines: 0x5f–0x62 the takings (houses, hotels,
restaurants, other buildings), 0x63–0x66 their costs, 0x67 new
buildings, 0x68 advertising, 0x69 interest, 0x6a everything else
(`Sonstiges`), 0x6b the year's total; last year's 13 kinds below.
Every booking step also refreshes the money menu (1070:2617), which
works out the total (1070:1452) if the menu is shown: `Tick::books`.

Step 0x8b ends the month in the books' history (1070:210a): four rings
of 120 months of a `Longint` per player (`*[1110:6f04]+0x24b`, `0xf0a`
bytes each: the months, the highest and lowest value as the charts'
scale, the number of months, the first month). They hold the money less
the loan, the net worth (both widening a scale that stays as far below
0 as above), the guests (1078:014c, a scale rounded up to even) and the
reputation. A full ring drops its oldest month. The last twelve months
of guests (1070:1345) decide scenarios 2 and 6 and a third of the score.

## The steps

Plots, houses and facilities are walked in their lists' order; "per
plot" means each plot's own lists of houses or facilities.

| Step | Does |
|---|---|
| 2 | The players' sums := 0 |
| 4 | Costs: player 1's advertising (beds · advertising level `*1110:6f24+p-1` · `Werbungskosten`, 1008:05d9), insurance (`assets div 120` if `1110:0072`, 1008:062c) plus the cost of living, and interest (the computers a sixth of theirs). Each player's `interest - rest - advertising` comes off (`add_money`, so computers pay less on harder levels); books 0x68, 0x69, 0x6a |
| 6 | Per plot: its beds by class of rent (`+0x26 + 4c`) and in all (`+0x22`) |
| 8 | Per plot, 1008:0f37: greenery `+3` (`(palms - built) · 50 div area + 50`, 50 without an area), the shops' price level `+0xf`, its quality `+4` (factors 24–31) and its quality by class `+5..+0xe` (32–35); without beds the quality is the greenery and the classes 50 |
| 0xa | Per plot: demand `+0x1a` and admitted `+0x1e` := 0 |
| 0xc | Each house's stars `+1` from `HAUSDATA.DAT` column 4 by condition, connection and location (1030:180d) |
| 0xe | Each house's rent rating `+0x16` (1008:0863) |
| 0x10 | Per plot, each house's demand `+0xc`: `beds · b div 100` (rounded, at most the beds), `b` the base demand weighed by its surroundings and its rent rating |
| 0x12 | Per plot: the houses' demand into `+0x1a` |
| 0x14 | Per plot: admitted `+0x1e` := min(demand, the airports' and harbours' places `+0xde`) |
| 0x16 | Per plot: each house's guests `+0x10` := its demand if everyone gets in, else `demand · admitted div plot demand` with the remainders carried on, at most its beds |
| 0x18 | Nothing (a loop over the houses that compares the owner with 1) |
| 0x1a, 0x1c | Rent (guests · rent `+0x12`) and running costs into the sums |
| 0x1d, 0x47 | `TUmenuform` refreshes (1080:0123): `Tick::umenu` |
| 0x1e | Each house's squares show its guests in percent of its beds; areas whose tens changed are redrawn on the overview map (10b8:1534) and with the occupancy pointers (1050:264d): `Tick::occupancy` |
| 0x20 | Rent less costs paid to everyone; books 0x5f, 0x63, 0x60, 0x64 |
| 0x22, 0x24 | Per plot: guests by class (`+0xf2 + 4c`) and in all (`+0xee`) |
| 0x26 | The island's guests (`*1110:6f20 + 0xce`, by class from `+0xd2`) |
| 0x28 | Each player's guests (`1110:6f90`, 1078:0172); nobody's count for player 8's interest |
| 0x32, 0x34 | Per plot: the guests copied to `+0x1ba` and mixed by class (1078:0780): their wishes |
| 0x36 | Each facility's stars (factors 10–12) |
| 0x38 | Each facility's price per visitor and day (`price · 100 div (places · 30)`), its rating `+0x1a` and price class `+0x14` |
| 0x39 | Per plot (1008:09c8 clears first): the shops' places by class and price class (`+0x4e + 0x2c·class`, `+0x52..`), the others' by class (`+0xc6 + 4·class`) |
| 0x3a | Per plot, each facility's demand `+0x10`: the base demand weighed by its price rating, then by its surroundings (guests per place ...), in percent of its places |
| 0x3c, 0x3e | Per plot: the facilities' demand by class (and price class) into `+0x1e6..` / `+0x25e + 4·class` |
| 0x40 | Per plot and shop class: from price class 9 down, `served := min(wishes of the class + carry, demand)`, the rest carried down (`+0x2b6..`) |
| 0x42 | Per plot, classes 3–9: served := min(guests, places) for transport, energy, water, rubbish; min(guests, demand) for sport, leisure, nature (`+0x32a + 4·class`) |
| 0x44 | Per plot, each facility but the championships': its visitors `+0xc` := its demand if its class is served in full, else its share of those served (remainders carried in `1110:4cb0`, `1110:4d1c`), at most its places; no demand, no change |
| 0x46 | Each facility's fill `+0x15`: visitors in percent of its places |
| 0x48 | The occupancy on its squares, as in 0x1e |
| 0x4a, 0x4c | Takings (`(fill · price + 50) div 100`) and running costs into the sums |
| 0x4e | Takings less costs paid; books 0x61, 0x65, 0x62, 0x66 |
| 0x50, 0x52 | Each house's and facility's condition drops by 1 with its decay chance (`Random(100)`, drawn only above 0) |
| 0x58 | Each player's beds (`*1110:6f24 + 0x3c + 4p`) and places (`+ 0x5c + 4p`) |
| 0x5a | Each player's price rating (`+ 4 + 4p`): their facilities' ratings weighted by places, 50 without |
| 0x5c | Each player's places per bed (`+ 0x27 + p`: `5 · places div beds`, at most 100, 50 without beds) and reputation (`+ 0x2f + p`) approaching both (factors 36, 37) |
| 0x64 | A bankrupt computer player is out (`+0x7f + p`), and event 10 is made certain (1010:3006: chance `0x3fffc`) with `assets div 5 · 3 + 1000` and the player; the human's net worth past 100 000, 1 000 000, 10 000 000 makes event 12, 16 or 18 certain, the first still waiting (chance 1), unless they cheated or the money is frozen. The events point at the human's view |
| 0x78 | The rentals send their people, [below](#the-rentals-people) |
| 0x82 | The computers' (and nobody's) houses: rent up a step when full (1010:1d62: +4, 8, 20, 40 below 80, 200, 400, 1561, else 1600), else down when rated below 100 (1010:1e05) |
| 0x84 | Their facilities: price up when full (1010:1ae2, at most `HAUSPREI.DAT` column 2 for transport and full championships, else three times it), else down when rated below 100 (1010:1c86) |
| 0x8b | The books' month (1070:210a), above |
| others | With `1110:0057` (on at first) the computer players, [below](#the-computer-players) |

### The rentals' people

Step 0x78 goes through the facilities: the bicycles (0x1a, kinds 0x11 or
0x12), jeeps (0x0a, 0x15/0x16), car showroom (0x1b, 0x13/0x14) and surf
school (0x6d, surfers 0x20/0x21) draw `Random(2)` for the kind. Then
(1008:15dd, 1008:16be) the count of their people out is `layer_551c` at
their square (255 is 0): while `(fill - 1) div 20` is at least the count,
one more goes out to the nearest free road on the plot (1010:1f8f) or
open sea (1010:20e0) within 10 rings, counted if `add_person` took it
(1088:2c04); when it's below the count less one, the first comes back
(1088:2c3b).

### The computer players

10a8:302e sets the computers' status text (`1110:70f2`, saved, not
shown) to `Gar nix`, counts `1110:70c2` up and gives player `70c2 mod
opponents + 2` a turn (10a8:2d5a): a state machine (`*1110:70ca`, a byte
each) that builds (10a8:2463 and the planning code around it), manages
its loans, sells through events (1010:3006 with event 9) and demolishes,
with `Random`s. Ported in [`game::computer`](../crates/game/src/computer.rs),
described in [computer.md](computer.md), with raising and lowering
land ([`game::terrain`](../crates/game/src/terrain.rs)) and building
roads ([`game::roads`](../crates/game/src/roads.rs)). Their price changes (steps 0x82, 0x84) and
renovations ([year_end](simulation.md#the-new-year-10084f64)) are ported.

## What the records' fields are

| Record | Field | What |
|---|---|---|
| House | `+1` | Stars, 0–100 |
| | `+0xc` | Demand (`Longint`) |
| | `+0x10` | Guests (`Integer`) |
| | `+0x12` | Rent per guest and month (`Longint`) |
| | `+0x16` | Rent rating |
| Facility | `+1` | Stars |
| | `+0xc` | Visitors (`Longint`) |
| | `+0x10` | Demand (`Longint`) |
| | `+0x14` | Price class |
| | `+0x15` | Fill in percent |
| | `+0x16` | Price (`Longint`) |
| | `+0x1a` | Price rating |
| Plot | `+3`, `+4`, `+5..+0xe`, `+0xf` | Greenery, quality, quality by class, shop price level |
| | `+0x10..+0x19` | Read as the shops' price level by class, never written |
| | `+0x352` | Probably the area |
| Player block | `+p - 1` | Advertising level, 0–7 |
| | `+4 + 4p`, `+0x27 + p` | Price rating, places per bed |
| | `+0x3c + 4p`, `+0x5c + 4p` | Beds, places |

## Open questions

- What `1110:004c` (the base demand) and `*1110:6f20 + 0x19a` (the
  economy) are set by: the settlement only reads them.
- 1008:0f37 writes each price class's shop price level into the same
  byte (`+0xf`), so `+0x10..` is never written and the last class wins;
  and it mixes copies of the shops' places by class (1078:0780) that it
  then drops. Both look like bugs; the port keeps them.
- `MaxPreis` divides in 1008:0907 where the range seems meant.
- Division by zero (a type without beds or places, a factor with divisor
  0) stops the original; the port's divisions give 0.
- Only the human pays advertising, insurance and the cost of living, and
  the computers pay a sixth of their interest (marked in the code as
  player 1's).
- The events 10, 12, 16 and 18 and the computers' status text are
  saved, but what the scheduled events say isn't checked yet.
