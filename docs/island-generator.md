# The island generator

A normal game (not a scenario, not a saved game) plays on an island the
game makes from the start menu's settings: shape, relief, land and
rivers ([startup.md](startup.md#the-start-menu-tstartform)). State 1210
of `THintergrund`'s timer calls `generate_island_busy` (1030:390e), which
sets the cursor `crNoDrop` (-13, not the hourglass) around
`generate_island` (10b0:52d4), whose stages move the progress window
(`game::wasnunform`). The rest of a
new game's world comes from the game's data, loaded in state 1220
([below](#the-rest-of-a-new-game)).

Ported in [`game::generator`](../crates/game/src/generator.rs) (the
island) and [`game::hintergrund::new_world`](../crates/game/src/hintergrund.rs)
(the whole world). The `holiday` binary seeds it from the clock, as
`Randomize` would.

## `generate_island` (10b0:52d4)

For a game that's loaded (`hintergrund+0x190` or `+0x191`) it only clears
the map (10b0:3030). Otherwise, moving the progress window's bar
(1040:376e) between the steps, and with two 0x4fb2-byte blocks for the
hills (`*1110:7226`, `*1110:722a`):

| Step | Routine | Does |
|---|---|---|
| 1 | `clear_map` (10b0:3030) | All sea: heights 0, forms 16, ground 2 (water), no buildings, parts, owners, occupancy; the plot map 0, `plot_count` 0, the location map 127 |
| 2 | by `island_shape` (`1110:008a`) | 0 the atoll (10b0:3c5c), 1 a group of islands (10b0:41b5 with 20), 2 one island (10b0:412c) |
| 3 | `smooth_land` (10b0:4fd1) | For each corner of 1..249, `i` outside, `j` inside: four times its height less its four neighbours' at least 3 lowers it (10b0:1290), below -2 raises it (10b0:0130); then the same backwards |
| 4 | `shape_squares` (10b0:354c) | Squares 1..249, `j` outside: no corner above the sea is flat water (form 16, ground 2); otherwise the form from the corners (`1110:1224`, as 10b0:33bc) and `reset_ground` (10b0:3199) |
| 5 | `decorate` (10b0:4dae) | Squares of sand, grass (looks: flat `Random(6)` gives 0, 0, 0, 1, 2, 3; sloping `Random(2)`) or sand to grass: with `Random(100) < 1110:0094` (20) a palm, part `look * HAUSNR[12][6] + Random(HAUSNR[12][0])` |
| 6 | `rivers` (10b0:4d79) | `rivers` (`1110:0092`) rivers, [below](#rivers) |
| 7 | `number_plots` (10b0:29c0) | The plot map, [below](#plots) |
| 8 | `location_from_island` (10b0:52a4) | Every square, `i` outside: a river (ground above 0x36) multiplies the location map by 112 %, sloping water (ground 2, form not 16) by 108 %, sand at heights below 2 by 101 %, each within 3 cells (1030:1daa); a building (the palms) adds its effect (1030:1d10) |

`MAPERSTE.DAT` has 11 tables of 5 × 5 `Integer`s (read as `Longint`s,
kept as words; 10b0:5788 allocates them, 10b0:56ce reads them), indexed
by relief (row) and land (column): `IGMinLeere` (`*1110:71f2`),
`IGMinRadius`, `GIRadius`, `GILeere1`, `GILeere2`, `ATAnzahl1`,
`ATAnzahl2`, `ATGroesse1`, `ATGroesse2`, `ATVariation1`, `ATVariation2`
(`*1110:721a`).

### Hills (10b0:3904)

Everything is made of hills. `hill(i, j, size)`, with `r := min(size,
50)`, does nothing unless `r < i, j < 250 - r`. Otherwise:

1. The mask (`*1110:7226`, `array[-50..50, -50..50] of Integer`) is
   cleared within `r`, and the heights there are kept (`*1110:722a`).
2. Eight `Random(r) + 1`, `v0` to `v7`, make four corners (row, column
   offsets): `(-v0, -v1)`, `(v2, -v3)`, `(v6, v7)`, `(-v4, v5)`.
3. `hill_line` (10b0:375a) draws the outline's sides: top left to top
   right, bottom left to bottom right, top left to bottom left, top
   right to bottom right. A side steps along its longer axis one at a
   time and adds the slope to the other coordinate, then rounds it
   ([floating point](#floating-point)).
4. Each column is filled from its first mark down to its last; a column
   whose first mark is in the bottom row (or that has none) stays.
5. Every corner inside the mask is raised with `gen_try_raise`
   (10b0:0130) while it's lower than its height before plus 1 and below
   20. `gen_try_raise` raises a corner only while its height is at most
   `i - 2`, `j - 2`, `248 - i` and `248 - j`; `gen_raise_corner`
   (10b0:0002) then raises it and, recursively, each neighbour (`i + 1`,
   `j + 1`, `i - 1`, `j - 1`) that is then two steps lower. So hills are
   plateaus one step above the land they're put on, with slopes around,
   and stay away from the map's edge.

`hill_cluster(i, j, radius, chance, largest)` (10b0:404c) goes through
the square around `(i, j)`, `i` outside: each corner within the circle
(`di² + dj² <= radius²` in `Longint`s, 1030:0f8d) gets a hill if
`Random(chance)` is 0, of size `Random(min(largest, 39) - 4) + 4`.

- **Group of islands** (10b0:41b5): 20 clusters, each at `Random(200) +
  25`, `Random(200) + 25`, radius `IGMinRadius + Random(10)`, chance
  `IGMinLeere + Random(15)`, largest 15.
- **One island** (10b0:412c): a cluster at (125, 125), radius `GIRadius`,
  chance `GILeere1`, largest 35; then one at (`Random(50) + 100`,
  `Random(50) + 100`), radius `GIRadius - 30`, chance `GILeere2`, largest
  20.
- **Atoll** (10b0:3c5c): a ring of 12 points, each `Random` in a box of
  the table at `1110:3cf4` (`i` from, `j` from, `i` to, `j` to: from
  (180..220, 180..220) round to (125..180, 190..249)), closed by the
  first. For each stretch, `ATAnzahl1` hills at a point `t = Random(101)`
  percent along it, each coordinate `(to - from) * t div 100 + from +
  Random(ATVariation1) - ATVariation1 div 2`, size `Random(ATGroesse1) +
  1`; then `ATAnzahl2` the same with `ATVariation2` and `ATGroesse2` but
  half of `ATVariation1` (`fix.atoll_hills`: of `ATVariation2`; the
  CD's tables differ only for some relief and land). Then `Random(30) + 20` hills of size
  `Random(3) + 2` in the lagoon (the 13th box, 55..195), at `from +
  Random(101) * (to - from) div 100`. The products are 16-bit (`MUL`,
  low word) on the ring, `Longint` in the lagoon.

### Floating point

The outline's sides are the generator's only floating point, and they
use the FPU (through `WIN87EM`), not the 6-byte `Real`: the difference
of the coordinates is loaded as a `Longint` (`FILD`), divided
(1108:055f: `FXCH`, `FDIVP`, or the RTL's safe division on a Pentium with
the `FDIV` bug, which gives the same quotient), stored as a `Double`;
the start coordinate is loaded (`FILD`) and the slope added (`FADD`) and
stored (`FSTP`) each step; `Round` (1108:0ea0) is `FRNDINT` and
`FISTP` with the control word's rounding, to nearest with halves to even.
The control word is the one `__FPMATH`'s initialisation leaves
(1108:0dc1), 64-bit precision, so each quotient and sum is rounded twice:
to the register's 64-bit mantissa, then to the `Double`'s 53 bits.

The port ([`generator::x87`](../crates/game/src/generator/x87.rs)) does
the same on integers: values are `i128` counts of 2^-100 (every operand
is a difference of at most a few hundred, and rounding only coarsens, so
every value is exact), each result rounded to 64 then 53 significant
bits, halves to even, and `Round` halves to even.

### Rivers

`river` (10b0:4d19):

1. `river_start` (10b0:4506): `Random(249) + 1` for `i`, then `j`,
   until the square's lowest corner is at least 4, but the 1500th try
   is taken unchecked, so a river always starts (`fix.river_start`: no
   river after 1500 tries).
2. `river_source` (10b0:4618): the direction `Random(4)` (0 `j + 1`, 1
   `i + 1`, 2 `j - 1`, 3 `i - 1`); the square and the next one that way
   are levelled to the lower of their lowest corners (10b0:459a; it
   fails below 3), and become the source pieces `0x57 + 2 * direction`
   and `0x58 + 2 * direction` without buildings. The river goes on from
   the square after them. If the source fails, there's no river.
3. `river_step` (10b0:4a7d) until it fails or the river's square has no
   corner above the sea: the height it comes from is the corner `(i - 1,
   j - 1)` going 0 or 1, `(i, j)` going 2 or 3. Below the square's top
   corner, the square is levelled to it (10b0:2e03) and the way on
   picked by `flat_way` (10b0:48d7); otherwise it's sloped a step down
   the way it goes (10b0:2e6f). Unless that fails or two of the squares
   beside are rivers already (10b0:47ff), the square gets its piece
   (10b0:424f: by the way in and out, its form and ground; `FLUSSPOS.DAT`
   row `piece`, ground `piece + 0x37`, no building) and the river moves
   on.
4. A river that failed is taken out (10b0:4cb1): from the square before,
   along river squares beside (`j - 1`, `i - 1`, `j + 1`, `i + 1`), each
   gets its ground back (10b0:3199).

`flat_way` looks along the three ways but back, a square further each
round: a way whose square has a larger corner sum than the lowest drops
out (off 1..249 or beside a river counts as 0xffff), until one is left;
of ties, the last of `i - 1`, `j - 1`, `j + 1`, `i + 1` wins. Its distance
is an `Integer`, so ways that tie up to the map's edge (over the sea,
say) look on past it until one comes back onto the map from the other
side, some 65000 rounds later; that happens on about one island in a
thousand. `fix.river_flat_way` stops at the edge, where the last of the
ways still open wins.

The levelling uses its own raising and lowering, `gen_raise_checked`
(10b0:0e0c) and `gen_lower_checked` (10b0:2053), through
`gen_set_corner` (10b0:2d70, 0x16 off 1..249). They are the player's
tool (10b0:069f, 10b0:1907, [terrain](../crates/game/src/terrain.rs))
without costs, markers, plots or the road and `layer_551c` checks: a
palm in the way goes for nothing, a river stops them (0x10). Unlike the
tool they write the working copy back after an error, so the corners
moved before it stay moved (`fix.generator_levelling`: nothing moves on
an error).

These fixes change how the island is made, so a new game's world gets
its fixes before the generator runs (`hintergrund::new_world`).

### Plots

`number_plots` (10b0:29c0) sets every plot map entry (`[i div 2][j]`, a
pair of rows) to -1, then goes down the columns (`j` outside, `i`
inside): an even row's sea corner with its entry still -1 gets 0; an
even row's land corner with its entry -1, or an odd row's whose entry
is 0 (its even partner is sea), starts a new plot. `plot_fill`
(10b0:23c3) walks the land joined to it depth first, with a stack in
three 35001-byte arrays: from each corner along `j - 1`, `j + 1` (either
row of the pair), `i - 2`, `i + 2` (the pair above or below) to land
whose entry is -1. `plot_count` is the number of plots. The records are
made in state 1220 (1068:2bba).

## The rest of a new game

What a normal game's world is made of, in the order the original sets
it up, which is the order of the random numbers:

| When | Routine | Sets |
|---|---|---|
| State 100 | `Randomize` (1108:1e21) | `RandSeed`: the port's seed |
| | 1010:000e | `MODATYP.DAT`'s values |
| | 1010:0414 | The names: `NAMEN.TXT`'s first line for player 0 (`keiner`), then for players 2–8 from the list (up to an empty line) at `Random(count)`, stepping by `1110:147a[Random(10)]` (53 to 89) round it |
| | 10b0:5788 | `MAPERSTE.DAT` |
| Start | 1058:021a | The menu's name for player 1; colours, opponents, difficulty are the menu's globals |
| State 1210 | 10b0:52d4 | The island |
| State 1220 | 1030:2e48 | The tables (`HAUSNR.DAT`, ...) again |
| | 1010:2fa9 | The button bar's buttons (`1110:651a`..`6530`) up but the pointer's: the view's (`Tools::new`) |
| | 1030:000d | The date, 1 January 1996 |
| | 1008:57dc | `ABRECHNE.DAT`; the accounting step and day 0 |
| | 1088:868c | The people: records 1 to 0x640 cleared with `+0x1a` 100 and kind 6 for 1 to 0x23, `+0x14` 1 in all; `layer_551c`, `layer_5520` 255; `ANIMDAT*.DAT`, `NEINTRAG.DAT`; `last` 0x2f |
| | 10a8:388b | The computers' type lists (`COMPBAU.DAT`) and `init_computers` (10a8:3558, `Random`s) |
| | 1078:3271 | Players 2–8: `Startgeld` as money and loan. The difficulty: easy doubles `MaxSchulden`, hard halves it; `Startgeld` becomes half of it; disasters off on easy; `1110:133c` on easy. Player 1: `Startgeld` as money and loan. No guests, no refused loans. `1110:6fb0`, `6fb2` (1078:075f, `Random`s); empty lists; the plots (1068:2bba: `plot_count` records, numbered 1 up, each put in front, ratings 50; 1068:25fd counts each land corner in its plot's `+0x352` and each palm in `+0x354`); the sums `*1110:6f20` cleared with the economy at 127 (1078:08b2); the players' block (1078:0918) |
| | 1070:2e6c | The books: the history's rings, this year's sums, kinds 0x6c–0x6e money, `-loan`, their difference |
| | 1010:3ab4 | `EREIGNIS.DAT` |
| | 10a0:0f6c | No sabotage, no championships |
| State 1500 | 1020:2501 | The main screen at its first scroll position (`wo` 240, `ns` 237), without `game_start_setup` |

The computers get `MODATYP.DAT`'s start money before the difficulty
changes it, the human after. The economy (1078:0692) is `Round(Sin(day
/ 6fb0) * 90 + 127 + Sin(day / 6fb2) * 37)`, worked out in `Real`s and on
the FPU, but only ever for day 0, which gives 127.

Not the world's: the button bar's buttons (`1110:651a` to `6530`,
1010:2fa9, `Tools::new`), the playlist (`1110:6ef2` to `6efe`,
1018:37de, `optionen::Settings::playlist`), the auctioneer's frames
(`1110:65b4` to `65b9`, 1028:2449, which the world's auction keeps,
[auction.md](auction.md)), the sabotage window's kind and flag
(`1110:65ba`, `54fa`, 1028:361f, `Sabowin`) and what only the forms
keep: the view sets them up. Heap blocks the original doesn't clear (the people records' last
bytes and record 0, the planes' unused track rows, the events' 13th
value) are 0 in the port.

## Open questions

- Whether `WIN87EM` or the FPU's own initialisation leaves 64-bit
  precision, as assumed: with 53 bits the outline's numbers are rounded
  once instead of twice. With these operands that changes a result only
  where the 64-bit rounding lands exactly halfway between two `Double`s.
- Whether anything between `Randomize` (state 100) and the generator
  draws random numbers, such as the start menu's sounds; nothing found
  does.
