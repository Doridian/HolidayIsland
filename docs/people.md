# The people and vehicles

Everything that walks, drives, flies or floats on the island, and the
money rising from a square, is a record in one table. Adding and
removing people is ported in [`game::people`](../crates/game/src/people.rs),
the per-tick movement ([below](#moving-10905293)) in
[`game::people::movement`](../crates/game/src/people/movement.rs), and
drawing them ([below](#drawing-them-1088385d)) in
[`game::darstellung`](../crates/game/src/darstellung.rs).
[simulation.md](simulation.md#animations-step-9) has where they run in
the timer.

## The records (`*1110:6fd6`)

1601 records of 0x28 bytes, saved with the map with their tables by unit
1088:8a07 (`load_people`; 1088:8950 saves them):

| Saved | What |
|---|---|
| `1110:3c2e` | Ten `Integer`s, `1, 0, -1, 0, 0, -1, 0, 1, 0, 0` at first (directions?) |
| `*1110:6fd6` | The records, 0xfa28 bytes |
| `*1110:6fda` | `ANIMDAT2.DAT`, the walkers' paths: 40 groups of 47 rows of 4 bytes (x, y, facing, what happens there) |
| `*1110:6fde` | `NEINTRAG.DAT`, 101 rows of 4 bytes |
| `*1110:6fe2` | `ANIMDAT3.DAT`, the planes' tracks: 16 groups of 31 rows of 3 `Integer`s (x, y, the way it faces); each group's length is in `1110:3620` |
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
| `+0xc`–`+0x13` | The view's: the picture's rectangle on the screen (x, y, width, height), 1088:3496 |
| `+0x14`–`+0x16` | Flags (`flags`); `+0x14` is cleared by the drawing; `+0x15` is 1 for a plane in the air (it isn't covered by the buildings in front and stays when a building is demolished) and 0 on the ground; `+0x16` 1 is drawn, cleared and set for most kinds by 1088:7ee3 and 1088:7f4a |
| `+0x17` | The kind, 0 for a free record (`kind`) |
| `+0x18` | A walker's `ANIMDAT2.DAT` path, a plane's `ANIMDAT3.DAT` track, the frame of everyone animated in place, a harbour ship's wait (`path`) |
| `+0x1a` | `Longint`: a walker's step along its path, a plane's row on its track, a ship's or surfer's on its `ANIMDAT4.DAT` route; the time (in frames) for those animated in place; the money's amount (`step`) |
| `+0x1e`, `+0x20` | Where it's going; for a taxiing plane the squares left and the steps to the next; the helicopter's state and laps left (`target`) |
| `+0x22`, `+0x24` | Where it's from: a surfer's beach, a walker's square, a ship's map corner, a plane's runway (its `layer_551c` is 0 while the runway is taken); the helicopter's tracks per lap and count (`home`) |
| `+0x26` | A plane's or the helicopter's height (220 in the air); a harbour ship's squares of detour (`word_26`) |

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
| 7, 8, 9 | Planes for the airports (0x28–0x33), from the map's corner nearest to their approach, at a height of 220; they land, park and take off again ([below](#the-planes-10902ce5)); second halves (the shadow?) 10, 11, 12 |
| 0x11–0x16 | "Walkers": the cars and bikes on the roads, along `ANIMDAT2.DAT` paths, their path by the road piece they start on |
| 0x19 | Leaves, with one of two sounds |
| 0x1a, 0x1b | From the power station (0x20) and the sewage works (0x21) |
| 0x1c–0x1f | From the zoo (0x45), `Random(4)` |
| 0x20, 0x21 | Surfers, sent out by a surfing championship ([`send_surfers`](#removing-people)) |
| 0x22–0x24 | Ships for the harbours (0x58–0x6b), from the nearest map corner, `Random(3)` |
| 0x25 | Someone leaving a demolished building's square |
| 0x26 | Builders on a building site (31) |
| 0x27–0x29, 0x30–0x32, 0x34, 0x3b–0x40, 0x42–0x45 | At the doors of the Haifischbar (0x14), Spielhalle (0x48), Juwelier (0x39), the parks (0x57, 0x4e, 0x50), Wasserpark (0x46), Disco (0x4a), Paradieso (0x37), Treppina (0x26), Queen's Hotel (0x38), Wellenbad (0x40), Bananorama (0x27), Beachvolleyball (0x3a), Bolzplatz (6), Casino (0x49, `Random(2)`) |
| 0x2a, 0x46 | A picture by the square's form (`AnsichtFormkorrektur`); not sent by anything ported |
| 0x2b–0x2d | Oil, from the oil slick (10a0:074d, [simulation.md](simulation.md#events-10103084)), `Random(3)` |
| 0x2e | The sightseeing helicopter (`H-Rundflüge`, 0x17), when the square's `551c` is free |
| 0x33 | Boats at the Bigport's jetties (0x68–0x6b) |
| 0x35–0x3a | Planes taxiing along an airport's runway (`1110:6fcc`), a square at a time |
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
   spectators (looked for on its own square, where they never are, so
   each pass sends more; `fix.stage_spectators` looks where they stand,
   `1110:3a58`); a building site's first square builders (`Random(50) = 1`);
   an airport (`Random(100) = 1`, some with a `Random(2)` between two
   parts) a taxiing plane if the square shows the right part; a harbour a
   sound (`Random(500) = 1` and seen, then `Random(2)`), and the Bigport's
   first squares a jetty boat (`Random(10) = 1`).

The view also calls it, through `people_refresh` ([below](#after-the-view-moves-10887fb1)).

## After the view moves (1088:7fb1)

`people_refresh` (181 bytes) runs at the end of every scroll of the main
screen (`hauptschirm_scroll`, 1020:07dc: the keys, a right click, the
scroll bars and their held repeat, the wheel) and of a turn (`set_view`,
1010:254d), both of which first clear records 0–0x23 with 1088:6c8b
(the same as 1088:6c5a), and when the options switch people moving on (1060:05cb). It
places everyone again for the view's rotation (1088:7a14, 1231 bytes,
records 0 to 0x640, no random numbers):

| Kinds | Placed again |
|---|---|
| 7, 8, 9 | The plane's picture and its second half's (1088:70ab): by the look of its track's row, a parked one (0x1b, 0x23) by the first row of track 10 or 13 |
| 0x11–0x16 | The walker's picture and offset (1088:4c55) |
| 0x1a, 0x1b, 0x28, 0x31, 0x3b, 0x3f, 0x44, 0x45, 0x47–0x4c | The square: the building's first square plus its first door (`1110:34b0`) |
| 0x1c–0x1f, 0x32, 0x34, 0x3c, 0x3d, 0x40, 0x4d–0x50 | Its second door |
| 0x3e | Its third door |
| 0x20, 0x21 / 0x22–0x24 | The surfer's / harbour ship's picture and offset (1088:6ee7, 1088:6cba) |
| 0x2a, 0x46 | The picture by the square's form (1088:4bfa) |
| 0x2e | The helicopter (1088:75de) |
| 0x33 | The square: the jetty's offset (`1110:31e0`) |
| 0x35–0x38 | The taxiing plane's offset (1098:0745...) and picture (`1110:3356` + `+0x18`) |
| 0x39, 0x3a | The picture |
| 0x41 | The square: the stage's offset (`1110:3a58`) |

Then, while people are animated and new ones come (`1110:0054`,
`1110:0056`), `spawn_people` for the new view, and the drawing (the
people and the tool's pictures). The port makes it a command,
`Action::ViewMoved(viewpoint)`, which `Game::view_moved` hands the
world at once, as the original does it: the world clears the records
0–0x23, takes the view's `Viewpoint` (the original's 20 × 27 view, with
`big_map` the middle of the bigger one) and runs `people_refresh`. The view data travels with the command, so a
replay or another peer places and spawns the same people. Switching
people moving on (`Action::SetOption`) runs it with the view the world
last had.

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
- `remove_first_at_home` (1088:2c3b): like `remove_people_at`, but only
  the first, and the square it was on gets a free `551c` afterwards.
- The rentals' people (the settlement's step 0x78, 1008:15dd and
  1008:16be, [settlement.md](settlement.md#the-rentals-people)): one more
  goes out to the nearest road on the rental's plot (`find_road`,
  1010:1f8f, 10 rings in `find_sea`'s order) or the open sea (1010:20e0,
  10 rings), or one comes back.

## Moving (1090:5293)

`people_tick` first counts down `1110:723c[1..42]`, how long each sound
still plays (`play_sound`, 10a0:30b6, plays a sound only at 0 and sets
its time from `1110:3dce`): that's the view's, and the port leaves it to
whoever plays `Tick::sounds`. Then for each record from 0x24 to 0x640:
`person_animate` (1098:109f), `person_step` (1090:2ce5) and, while
people move (`1110:0055`), a walker's step. Nothing here draws; the sounds
play where the player sees the square (1050:245b), and the walkers' one
`Random(100)` (and `Random(3)` for some) only then.

### The animations (1098:109f)

While people move, by kind:

- Most of those at the buildings' doors (0x1a–0x1f, 0x26–0x29,
  0x30–0x34, 0x3b–0x45, 0x47–0x50): the frame `+0x18` counts up; once it
  reaches the time `+0x1a` they leave, else the picture is the frame (or
  half of it, or the frame mod 2, 8 or 10) on from the kind's first. The
  zoo's 0x1d makes a sound on its first frame, the minigolfers on their
  ninth; 0x1a and 0x34 move their picture on one frame.
- Ships (2), boats (3) and kind 4 rock until their time is up (ships
  live 700 ticks and more, boats 255) and take one off the count; every 28
  frames a ship drifts a square up the screen (`1110:3cd4` by the view's
  rotation) if that's open sea, with a sound, else it leaves.
- Surfers (0x20, 0x21) and harbour ships (0x22–0x24) ride `ANIMDAT4.DAT`
  routes (`+8` the route, `+0x1a` the row; 16 rows a square, then they move
  by `1110:3660`, their marker with them). At a route's centre row (31,
  15) they turn towards their target (`1110:3792`) and start the next
  route at `1110:3c64`. A surfer at its target, or with no open sea ahead,
  picks a new target within 5 squares (1098:03b3: 21 tries of two
  `Random(5)`s) and leaves if it finds none. A harbour ship sails from its
  corner to the harbour, waits 100 ticks there with a sound, turns away
  from the harbour (1098:0002) and sails back to the corner, where it
  leaves; one blocked by another takes a detour (1098:04f9, `Random(8)`
  twice and `Random(50)`).
- The helicopter (0x2e) by its state `+0x1e`: 3 waits 20 ticks, 0 climbs
  to 220 with a sound and draws its laps (`Random(5) + 3`), 2 flies its
  tracks round, a turn (`+8`) after each of them, 1 lands, 4 waits 20 ticks
  and leaves; 1088:75de places it.
- Taxiing planes (0x35–0x3a) roll a step every three ticks (four the
  first time), a square every four steps (0x35, 0x36), two (0x37, 0x38) or
  each (0x39, 0x3a), until the runway's squares (`+0x1e`) are done.

Whether or not people move, the money rises a pixel a tick and goes at
0, the oil cycles its frames, and kinds 0x19, 0x25, 0x2a and 0x46 count
their frames and leave.

### The planes (1090:2ce5)

Only while people move. A plane (7, 8, 9) flies `ANIMDAT3.DAT` tracks
(`+0x18`, its rows `+0x1a`, `1110:3620` their lengths and start rows):
past a track's last row it moves a square (`1110:3660`). Its state `+8`:

- 0x19 and 0x1f: from the corner towards the runway (`+0x1e`), turning at
  each centre row; there, if it's its airport's approach (1090:2c1f) and
  the runway is free (`layer_551c` at `+0x22` 255), it takes the runway
  (0) and comes down (0x1a, 0x20); if it's taken it flies a holding circle
  (0..=0x18, 0x24..=0x3c, the tracks from `1110:3778`) and tries again at
  its end; off its airport it leaves (0x1e, to the nearest corner, where
  it's gone).
- 0x1a and 0x20 come down 3 a tick to the square's height (with a sound);
  within a track, off an airport it's gone (someone leaves, 0x19); on the
  runway's parts (`1110:36a0` columns 2, 7) it rolls (0x1c, 0x21), or with
  soft soap on the runway (sabotage 5, 10a0:0ec8) skids (0x3d, 0x3e).
- 0x1c and 0x21 roll to the end of their track, then park (0x1b, 0x23)
  for 100 ticks, then take off (0x1d, 0x22, to the nearest corner).
- 0x1d and 0x22 climb 3 a tick from the runway's last parts (columns 3,
  8; freeing the runway, with a sound) up to 220, then leave (0x1e).
- 0x3d and 0x3e: at the end of the track the soft soap ends (10a0:0e17),
  the runway is freed, and the plane crashes into the square past the
  runway: someone leaves and the square is demolished (1078:280b).

Then its picture: its track turned by the view (`1110:3430`), lifted by
its height over the view's corner and the form, and its second half's.

### The walkers

While people move, a walker's step `+0x1a` counts up along its
`ANIMDAT2.DAT` path, and the row's fourth byte says what happens:

- 0, 10–13: it just moves (1088:4c55 places it).
- 1, a junction: a path without road ahead (1088:4e18) turns off
  (0..3, 0xc..0x17 to 0x18..0x27); then it claims the square ahead
  (1088:5f3e: the path `NEINTRAG.DAT` gives for that road piece, in the
  first free marker layer) unless someone's in the way there (1088:5386:
  a marker not of a path it may join, 1088:4f4f), when it waits a step
  (and makes a sound where seen, 1088:68f1, which also sends it away one
  time in a hundred).
- 2–9, a crossing: on the right road pieces, with the way free, it may
  turn onto another path (1088:6769, `Random(100)` < 60 or < 50).
- 255, the end of a square: by the row's third byte, 0–3 it goes on to
  the next square in that direction with the path `NEINTRAG.DAT` gives,
  or where the road ends it's gone (someone leaves, 0x19); 4–0x13 it goes
  on with another path within the square (a turn).

## Drawing them (1088:385d)

The clock's tick draws the people after `spawn_people`, only while new
people come (`1110:0056`), all in the view: 1088:385d takes the records
from 0x24 on that are drawn (`+0x16` = 1) and seen (1050:245b), works out
where each picture goes (1088:30ba: the square's column pair and half
row from the view's origin, rotation and parity, plus the offset, lifted
by the highest corner for those at the buildings, else by the view's
corner and the form) and keeps what's under it (1088:3496); 1088:3727
sorts them by their picture's top, the planes 2000 and their second
halves 1000 lower. 1088:38c4 draws them (1088:3a19): the `ANIMDAT1.DAT`
row's mask (columns 4, 5) with `SRCAND`, the picture (2, 3) with
`SRCINVERT`, both from bitmap column 10 and height × width columns 0, 1;
the money is its amount and the currency in the font's colour 1. Then,
for those on the ground (`+0x15` = 0), what's in front of them is drawn
over them, cut to the picture: the foundations of the three squares in
front (`1110:37a2`, not for kind 0x3b) and the buildings of 29 squares in
front, but those whose `HAUSPOS.DAT` row's last column is 1 (or 2, for
walkers). The port draws the island first and the people over it,
instead of keeping what's under them.

## Open questions

- What `+0x14` is; what the planes' second halves are (a shadow on the
  ground, it seems). Records 0–0x23 (kind 6) are the main screen's tools'
  pictures: the pointer and the squares a building or road would take
  (1090:124f, [player-tools.md](player-tools.md)), which the port keeps
  in the view.
- Who sends the walkers out: nothing ported adds kinds 0x11–0x16
  (1088:7a14 only places them again).
- 1098:0002 returns an undefined value when a harbour ship's square has no
  harbour next to it; the port keeps the ship's heading (it leaves anyway).
- The harbour ships' second detour (1098:04f9) checks the ship's own square
  five times rather than the squares ahead (`fix.ship_detour` checks its
  own and the four ahead); one with `+0x26` left marks the square ahead
  without going there, which looks meant (it gets there at its step's
  end) and is kept.
- 1090:2c1f takes a plane's second approach if either coordinate matches
  (`fix.plane_approach`: both).
- The world reads the human's view in several places, and 1088:7fb1 runs
  `spawn_people` for the view that moved (`Action::ViewMoved`). With
  several players the people would be placed for whoever moved last;
  that needs an answer.
- 1088:0324 reads its tables past their ends for building types that
  don't send that kind (nothing ported does that); the port says 0.
- Walkers' and surfers' homes can be -1 (a beach already taken), and the
  original then reads and writes outside the marker layer; the port
  doesn't.
