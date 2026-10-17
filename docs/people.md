# The people and vehicles

Everything that walks, drives, flies or floats on the island, and the
money rising from a square, is a record in one table. Adding and
removing people is ported in [`game::people`](../crates/game/src/people.rs);
the per-tick movement (`people_tick`, 1090:5293, and `person_step`,
1090:2ce5) isn't yet. [simulation.md](simulation.md#animations-step-9)
has where they run in the timer.

## The records (`*1110:6fd6`)

1601 records of 0x28 bytes, saved with the map with their tables by unit
1088:8a07 (`load_people`; 1088:8950 saves them):

| Saved | What |
|---|---|
| `1110:3c2e` | Ten `Integer`s, `1, 0, -1, 0, 0, -1, 0, 1, 0, 0` at first (directions?) |
| `*1110:6fd6` | The records, 0xfa28 bytes |
| `*1110:6fda` | `ANIMDAT2.DAT`, the walkers' paths: 40 groups of 47 rows of 4 bytes (x, y, facing, what happens there) |
| `*1110:6fde` | `NEINTRAG.DAT`, 101 rows of 4 bytes |
| `*1110:6fe2` | `ANIMDAT3.DAT`, the planes' tracks: 16 groups of 31 rows of 3 `Integer`s (x, y, ?) |
| `1110:6ff4` | The last record in use of the general range |
| `1110:6fee`, `6ff0` | How many ships (kind 2) and boats (kind 3) are on the sea; the maps' counts match their records |

The tables are loaded from the CD first (`load_animdat`, 1088:80a7), so
the map's copies replace them; `ANIMDAT1.DAT` (the pictures: height and
width first) and `ANIMDAT4.DAT` (the planes' routes) come only from the
CD and are in `World::tables`.

The records come in fixed ranges:

| Records | What |
|---|---|
| 0–0x23 | Kind 6 in the CD's maps, not known yet; `people_map_changed` (1088:6c5a, also 1088:6c8b) clears their kinds and `1110:6fce`, `1110:6ff2` |
| 0x24–0x2e | The money rising from a square (`float_money`, 1088:0153) |
| 0x2f–0x577 | Everyone else: the first free record up to `1110:6ff4`, which grows by one when it was that one (to at most 0x578). Nobody is added while it's below 0x2f |
| 0x578–0x5dc | Planes (kinds 7, 8, 9) and the sightseeing helicopter (0x2e) |
| 0x5dc–0x640 | Their pictures' second halves, 100 records on (kind + 3, 0x2f) |

A record (the names in the port in brackets):

| Offset | What |
|---|---|
| `+0`, `+2` | The square (`pos`) |
| `+4`, `+6` | The picture's offset in pixels (`offset`) |
| `+8` | Heading or route (`heading`): 2 for a walker, 0x19 or 0x1f for a plane, `Random(8)` for the helicopter |
| `+0xa` | The picture, an `ANIMDAT1.DAT` row (`sprite`) |
| `+0x14`–`+0x16` | Flags (`flags`); `+0x15` 0 lets a plane be cleared off a demolished building, `+0x16` is cleared and set for most kinds by 1088:7ee3 and 1088:7f4a |
| `+0x17` | The kind, 0 for a free record (`kind`) |
| `+0x18` | A walker's `ANIMDAT2.DAT` path, a ship's animation phase (`path`) |
| `+0x1a` | `Longint`: a walker's step along its path, a plane's along its track; a countdown for most others; the money's amount (`step`) |
| `+0x1e`, `+0x20` | Where it's going; for a taxiing plane the runway and a lane (`target`) |
| `+0x22`, `+0x24` | Where it's from: a surfer's beach, a walker's square, a ship's map corner (`home`) |
| `+0x26` | 0xdc (220) for a new plane: its height? (`word_26`) |

`+0xc`–`+0x13` aren't written by anything ported.

The two marker layers `*1110:551c` and `*1110:5520` (255 = nothing) say
what's on a square for the people: ships, boats, surfers and oil mark
their square with their kind, walkers with their path, the helicopter
with 1, and a beach counts its surfers out at sea.

## The kinds

From the buildings that send them (`HAUSNAME.TXT`'s names):

| Kind | What, from where |
|---|---|
| 1 | The money rising from a square |
| 2, 3 | Ships and boats on the open sea ([`spawn_people`](#new-people-10902373)) |
| 4 | From the tennis court (7) |
| 6 | Records 0–0x23 |
| 7, 8, 9 | Planes for the airports (0x28–0x33), from the map's corner nearest to their approach, from a height of 220; second halves 10, 11, 12 |
| 0x11–0x16 | Walkers along `ANIMDAT2.DAT` paths, their path by the road piece they start on |
| 0x19 | Leaves, with one of two sounds |
| 0x1a, 0x1b | From the power station (0x20) and the sewage works (0x21) |
| 0x1c–0x1f | From the zoo (0x45), `Random(4)` |
| 0x20, 0x21 | Surfers, sent out by a surfing championship ([`send_surfers`](#removing-people)) |
| 0x22–0x24 | Ships for the harbours (0x58–0x6b), from the nearest map corner, `Random(3)` |
| 0x25 | Someone leaving a demolished building's square |
| 0x26 | Builders on a building site (31) |
| 0x27–0x29, 0x30–0x32, 0x34, 0x3b–0x40, 0x42–0x45 | At the doors of the Haifischbar (0x14), Spielhalle (0x48), Juwelier (0x39), the parks (0x57, 0x4e, 0x50), Wasserpark (0x46), Disco (0x4a), Paradieso (0x37), Treppina (0x26), Queen's Hotel (0x38), Wellenbad (0x40), Bananorama (0x27), Beachvolleyball (0x3a), Bolzplatz (6), Casino (0x49, `Random(2)`) |
| 0x2a, 0x46 | A picture by the square's form (`AnsichtFormkorrektur`); not sent by anything ported |
| 0x2b–0x2d | Oil (the oil slick, 10a0:074d, not ported) |
| 0x2e | The sightseeing helicopter (`H-Rundflüge`, 0x17), when the square's `551c` is free |
| 0x33 | Boats at the Bigport's jetties (0x68–0x6b) |
| 0x35–0x3a | Planes taxiing at the airports, on a runway (`1110:6fcc`) |
| 0x41 | Spectators at Miss Holiday Island's stage (0x7b, 0x7c) |
| 0x47–0x4c | From the minigolf (0x41), `Random(6)` |
| 0x4d–0x50 | Spectators at a full tennis court (0x70) |

## Adding someone (1088:0324)

`add_person(kind, i, j)`, 10464 bytes, mostly one case per kind. Planes
and the helicopter go into the first free record of 0x578..0x5dc and only
while people move (`1110:0055`). The helicopter takes a `Random(8)` route,
its first step from `1110:3622 + 4 · route`; a plane's heading and
approaches come from `1110:36a0` by airport type, two headings pick one
of two approaches with `Random(2)`. Both work out their pictures with
1088:75de.

Everyone else goes into the first free record of 0x2f..`1110:6ff4`
(below 0x578). The kind's case only runs while people move; after it, the
kinds 0x19 and 0x25 (with a `Random(2)` sound), 0x2a, 0x46 and the oil
are filled whether or not they do. The record is used up even when
nothing was written into it (people not moving, a walker's square taken).
Many cases go through 1088:0025, which fills all fields; the others leave
the fields they don't write as the last occupant left them.

Where someone appears depends on the human's view: a building's door is
its first square (1030:154c) plus `1110:34b0[rotation, n]` (n for a door
1–3 squares in), the jetties' and the stage's offsets are by rotation
(`1110:31e0`, `1110:3a58`), and the pictures by rotation (`1110:33f0`,
`1110:3356`, `1110:3430`, `1110:34e0`, 1098:0745..0e6d). A sound plays
only where the player sees the square (1050:245b), and a few kinds pick
theirs with a `Random` then, so what the player looks at changes the
random numbers. The port keeps the view's rotation, origin and parity in
`World::viewpoint` (`1110:007c`, `1110:5530`, `1110:5534`, saved with the
map), which the view sets before each tick; sounds go to the view in
`Tick::sounds`.

## New people (1090:2373)

`spawn_people`, each tick while people are animated and spawn
(`1110:0054`, `1110:0056`), only while they move (`1110:0055`):

1. A boat, then a ship: `Random(250)` for `i`, then `j`; on flat water
   with fewer than ten and none of the kind there, one comes and the count
   goes up.
2. For the 11 × 61 squares of `1110:5536` from the view's origin (`[e][l]
   = ((l + 1) div 2 + e, l div 2 - e)`, filled by 1030:2bdc and saved with
   the map; they reach below the screen), each with both coordinates
   1..249, by its building: a full tennis court's first square gets the
   four spectators (looked for at its second door); the stage its
   spectators; a building site's first square builders (`Random(50) = 1`);
   an airport (`Random(100) = 1`, some with a `Random(2)` between two
   parts) a taxiing plane if the square shows the right part; a harbour a
   sound (`Random(500) = 1` and seen, then `Random(2)`), and the Bigport's
   first squares a jetty boat (`Random(10) = 1`).

The view also calls it: 1088:7fb1, after scrolling, turning the view and
the options, runs 1088:7a14 and `spawn_people` with the drawing. The port
leaves that out, so the random numbers part from the original's after
those.

## Sending someone out (1090:1a25)

`building_slice` (1078:0002) picks a building; `send_person` sends by its
type the kind in the table above, if none of that kind is at the square
(or at its door, from the first square only, for most); airports and
harbours draw `Random(35)` / `Random(30)` and the kind with `Random(3)`;
the harbours only if nobody is on the square.

## Removing people

- `remove_person` (1088:2df2): planes and the helicopter with their
  second halves; walkers and surfers take one off their home's count in
  `551c` (255 at 0), walkers also free the square their path step leads to
  (1088:5841); anyone else on the map clears both layers on their square.
- `remove_people_from` (1088:6975): everyone on the square leaves; if a
  building (not a palm) is there, each of its squares gets someone leaving
  (0x25) and free layers, and everyone on them goes but those leaving, the
  money, kind 0x19 and planes with `+0x15` set. Demolishing, the
  earthquake and the flood use it.
- `remove_workers` (1088:6bac): the builders in a finished building's area.
- `remove_people_at` (1088:2d2a): walkers and surfers whose home is the
  square (a surfing championship's end, a rental's demolition).
- `send_spectators_away` (10a0:0026's first part), `flood_people`
  (10a0:0b49's second part).
- `send_surfers` (10a0:10c6): five tries; the nearest open sea within 15
  rings (1010:20e0: columns, then rows), `Random(2)` for the kind, and the
  beach counts each that came.

## Open questions

- What records 0–0x23 (kind 6), `NEINTRAG.DAT`, `1110:3c2e`, the flags
  and `+0x26` are; whether 0x35–0x3a are planes taxiing, and 7–9 planes
  landing (the height of 220 and the airports suggest it).
- The world reads the human's view in several places, and 1088:7fb1 runs
  `spawn_people` from the UI. For several players these need an answer.
- 1088:0324 reads its tables past their ends for building types that
  don't send that kind (nothing ported does that); the port says 0.
- Walkers' and surfers' homes can be -1 (a beach already taken), and the
  original then reads and writes outside the marker layer; the port
  doesn't.
