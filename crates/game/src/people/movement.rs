//! The people and vehicles move: `people_tick` (1090:5293) runs for each
//! record from the money on (0x24..=0x640) `person_animate` (1098:109f:
//! everything that stays in place or sails, the helicopter, the taxiing
//! planes), `person_step` (1090:2ce5: the planes landing and taking off)
//! and the walkers' steps along their `ANIMDAT2.DAT` paths. Nothing here
//! draws: the original only plays sounds, which go to the view in
//! [`Tick::sounds`], and the pictures it works out are the records'
//! sprites and offsets, which the view draws.

use std::sync::Arc;

use super::{
    APPROACHES, BOAT, FIRST_AIRPORT, FIRST_MONEY, FLAT, FREE, HELICOPTER, LAST, LEAVING, LOOKS, MONEY, PLANE,
    PLANE_HALF, PLANE_TRACKS, Person, SHIP, SHIP_SPRITES, TAXIING_OFFSETS_35, TAXIING_OFFSETS_36, TAXIING_OFFSETS_37,
    TAXIING_OFFSETS_38, TAXIING_SPRITES, WALKERS, WATER, nearest_corner,
};
use crate::darstellung::{FORM_LIFT, ground_height};
use crate::events::UNSHAKEN;
use crate::simulation::Tick;
use crate::world::{NO_BUILDING, Pos, World, on_map};

/// The `ANIMDAT3.DAT` tracks (and the helicopter's): each one's last row,
/// and the row a plane starts it at. // 1110:3620
pub(super) const TRACK_ENDS: [(i16, i16); 16] = [
    (7, 4),
    (15, 8),
    (7, 4),
    (15, 8),
    (7, 3),
    (15, 7),
    (7, 3),
    (15, 7),
    (7, 4),
    (14, 0),
    (7, 3),
    (7, 3),
    (14, 0),
    (7, 4),
    (14, 0),
    (14, 0),
];

/// How a vehicle's square changes at the end of a track (planes, the
/// helicopter) or of an `ANIMDAT4.DAT` route (ships, surfers), by the track
/// or the heading. // 1110:3660
const MOVES: [Pos; 16] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (0, 0),
    (1, 0),
    (0, -1),
    (0, 0),
    (0, 1),
    (0, 0),
    (0, 0),
];

/// Which way to turn towards a target: by `1` if it's at a lower `i`, `2`
/// at a higher `i`, `4` at a lower `j`, `8` at a higher `j`; 255 for one
/// that's reached. // 1110:3792
const TURNS: [u8; 16] = [255, 0, 4, 255, 2, 1, 3, 255, 6, 7, 5, 255, 255, 255, 255, 255];

/// A plane's track for each step of its holding circle (states 0..=0x18 and
/// 0x24..=0x3c). // 1110:3778 (also read as 1110:3754 + state)
const CIRCLE: [u8; 25] = [
    2, 2, 2, 1, 1, 1, 0, 0, 0, 7, 7, 7, 6, 6, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 3,
];

/// The row a ship or surfer starts an `ANIMDAT4.DAT` route at, by heading.
/// // 1110:3c64
const ROUTE_STARTS: [i16; 8] = [8, 8, 8, 8, 7, 7, 7, 7];

/// The headings a harbour ship tries when the way ahead is blocked, from
/// its own: six turns for each of `Random(8)`. // 1110:3c74
const DETOURS: [[i16; 6]; 8] = [
    [1, -1, 2, -2, 3, -3],
    [2, -2, 1, -1, 3, -3],
    [3, -3, 1, -1, 2, -2],
    [-1, 2, -2, 3, -3, 1],
    [-1, 2, 1, -2, 3, -3],
    [1, 3, -2, 2, -3, -1],
    [2, -2, -1, 1, 3, -3],
    [-3, 3, -2, 2, -1, 1],
];

/// Where a ship on the open sea drifts every 28 ticks: towards the top of
/// the screen, whatever the view's rotation. // 1110:3cd4
const DRIFT: [Pos; 4] = [(-1, 1), (-1, -1), (1, -1), (1, 1)];

/// The squares next to a walker's in its path's four directions, as the
/// end of a square uses them (literally, not from 1110:3c2e).
const NEIGHBOURS: [Pos; 4] = [(1, 0), (-1, 0), (0, -1), (0, 1)];

/// The paths that turn off at a junction (path byte 3 = 1) when there's no
/// road ahead in their direction: the path, the direction, the new path.
/// // 1090:5293
const JUNCTIONS: [(i16, u8, i16); 16] = [
    (0, 0, 0x18),
    (1, 1, 0x19),
    (2, 2, 0x1a),
    (3, 3, 0x1b),
    (0xc, 0, 0x1c),
    (0xd, 1, 0x1d),
    (0xe, 2, 0x1e),
    (0xf, 3, 0x1f),
    (0x10, 0, 0x20),
    (0x11, 1, 0x21),
    (0x12, 3, 0x22),
    (0x13, 2, 0x23),
    (0x14, 3, 0x24),
    (0x15, 2, 0x25),
    (0x16, 0, 0x26),
    (0x17, 1, 0x27),
];

/// A crossing in a walker's path (path byte 3 = 2..=9): the road pieces it
/// may turn on, the direction checked for a road first (where it's
/// checked), the one checked for someone in the way, and the turn
/// ([`World::turn_walker`]'s direction and path). // 1090:5293
struct Crossing {
    grounds: [u8; 6],
    road: Option<u8>,
    blocked: u8,
    turn: u8,
    path: i16,
}

const CROSSINGS: [Crossing; 8] = [
    Crossing {
        grounds: [0xc, 0xd, 0x10, 0x1b, 0x1c, 0x1f],
        road: None,
        blocked: 3,
        turn: 0,
        path: 7,
    },
    Crossing {
        grounds: [0xc, 0xf, 0x10, 0x1b, 0x1e, 0x1f],
        road: Some(0),
        blocked: 2,
        turn: 0,
        path: 4,
    },
    Crossing {
        grounds: [0xe, 0xf, 0x10, 0x1d, 0x1e, 0x1f],
        road: None,
        blocked: 2,
        turn: 1,
        path: 0xb,
    },
    Crossing {
        grounds: [0xd, 0xe, 0x10, 0x1c, 0x1d, 0x1f],
        road: Some(1),
        blocked: 3,
        turn: 1,
        path: 8,
    },
    Crossing {
        grounds: [0xe, 0xd, 0x10, 0x1d, 0x1c, 0x1f],
        road: None,
        blocked: 0,
        turn: 2,
        path: 9,
    },
    Crossing {
        grounds: [0xd, 0xc, 0x10, 0x1c, 0x1b, 0x1f],
        road: Some(2),
        blocked: 1,
        turn: 2,
        path: 6,
    },
    Crossing {
        grounds: [0xc, 0xf, 0x10, 0x1b, 0x1e, 0x1f],
        road: None,
        blocked: 1,
        turn: 3,
        path: 5,
    },
    Crossing {
        grounds: [0xe, 0xf, 0x10, 0x1d, 0x1e, 0x1f],
        road: Some(3),
        blocked: 0,
        turn: 3,
        path: 10,
    },
];

/// The path and step a walker goes on with at the end of a square, by
/// the row's byte 2 from 4 on. // 1090:5293
const EXITS: [(i16, i32); 16] = [
    (1, 0xf),
    (0, 0xf),
    (3, 0xf),
    (2, 0xe),
    (0xd, 0xf),
    (0xc, 0xf),
    (0xf, 0xf),
    (0xe, 0xf),
    (0x11, 0xe),
    (0x10, 0xe),
    (0x13, 0xe),
    (0x12, 0xf),
    (0x15, 0xf),
    (0x14, 0xf),
    (0x17, 0xf),
    (0x16, 0xf),
];

/// The paths another walker on the square ahead may be on for a walker to
/// join it, by the direction it goes: the path pieces with a sixth for
/// the pieces 0x12, 0x21 (and the like) and one for the others. The last
/// two of each are the paths (the first) or the junction pieces (the
/// second), [`World::walker_blocked`]. // 1088:5386
const JOINABLE: [([u8; 5], u8, u8, [u8; 2]); 4] = [
    ([0x17, 0x11, 0xb, 5, 1], 6, 0xd, [0x12, 0x21]),
    ([0x16, 0x10, 9, 7, 0], 10, 0xc, [0x14, 0x23]),
    ([0x14, 0x12, 7, 5, 3], 8, 0xf, [0x13, 0x22]),
    ([0x15, 0x13, 0xb, 9, 2], 4, 0xe, [0x11, 0x20]),
];

/// An in-place animation (1098:109f): its picture from its frame (`+0x18`)
/// and the first picture.
#[derive(Debug, Clone, Copy)]
enum Frames {
    /// One picture per frame.
    Each,
    /// One per two frames.
    Half,
    /// Over and over, every `n` frames.
    Cycle(i16),
}

/// The kinds 1098:109f counts up in place until their time (`+0x1a`) is
/// up: their frames and first picture.
const IN_PLACE: [(u8, Frames, i16); 36] = [
    (0x1a, Frames::Each, 0xe3),
    (0x1b, Frames::Half, 0xef),
    (0x1c, Frames::Each, 0x103),
    (0x1d, Frames::Each, 0x10b),
    (0x1e, Frames::Each, 0x113),
    (0x1f, Frames::Each, 0x11b),
    (0x26, Frames::Each, 0x153),
    (0x27, Frames::Half, 0x167),
    (0x28, Frames::Cycle(8), 0x16c),
    (0x29, Frames::Each, 0x174),
    (0x30, Frames::Each, 0x1b0),
    (0x31, Frames::Each, 0x1b6),
    (0x32, Frames::Each, 0x1bf),
    (0x33, Frames::Each, 0x1c6),
    (0x34, Frames::Each, 0x1cd),
    (0x3b, Frames::Cycle(2), 0x1f6),
    (0x3c, Frames::Half, 0x1f8),
    (0x3d, Frames::Half, 0x202),
    (0x3e, Frames::Half, 0x20c),
    (0x3f, Frames::Cycle(10), 0x215),
    (0x40, Frames::Half, 0x21f),
    (0x41, Frames::Cycle(10), 0x229),
    (0x42, Frames::Cycle(10), 0x233),
    (0x43, Frames::Cycle(10), 0x23d),
    (0x44, Frames::Cycle(10), 0x247),
    (0x45, Frames::Each, 0x251),
    (0x47, Frames::Each, 0x25b),
    (0x48, Frames::Each, 0x265),
    (0x49, Frames::Each, 0x26f),
    (0x4a, Frames::Each, 0x279),
    (0x4b, Frames::Each, 0x283),
    (0x4c, Frames::Each, 0x28d),
    (0x4d, Frames::Cycle(10), 0x297),
    (0x4e, Frames::Cycle(10), 0x2a1),
    (0x4f, Frames::Cycle(10), 0x297),
    (0x50, Frames::Cycle(10), 0x297),
];

/// The minigolf's players (kinds 0x47..=0x4c) hit the ball on this frame.
const PUTT: i16 = 9;

/// The plane states (`+8`, 1090:2ce5) that wait a hundred ticks on the
/// runway.
const PARKED: [i16; 2] = [0x1b, 0x23];
/// The planes skidding off the runway on soft soap.
const SKIDDING: [i16; 2] = [0x3d, 0x3e];

/// A square's centre in the tracks' and routes' rows (x, y), where the
/// vehicles turn and pick their next track or route.
const CENTRE: (i16, i16) = (0x1f, 0xf);

/// The step's low word, the row the original reads.
#[allow(clippy::cast_possible_truncation)]
fn row(p: &Person) -> i16 {
    p.step() as i16
}

/// A table entry, 0 or the default outside the table (the original reads
/// past it).
fn at<T: Copy + Default>(table: &[T], index: impl TryInto<usize>) -> T {
    index
        .try_into()
        .ok()
        .and_then(|k| table.get(k))
        .copied()
        .unwrap_or_default()
}

/// `p`'s time is up: its frame (`+0x18`, sign-extended) reached its time
/// (`+0x1a`).
fn time_up(p: &Person) -> bool {
    i32::from(p.path()) >= p.step()
}

/// 1098:0497: a heading back into 0..=7, from -8..=15.
fn wrap_heading(heading: i16) -> i16 {
    if heading >= 8 {
        heading - 8
    } else if heading < 0 {
        heading + 8
    } else {
        heading
    }
}

/// 1098:04ca: off the map's inner squares 1..=249.
fn off_inner((i, j): Pos) -> bool {
    !(1..=249).contains(&i) || !(1..=249).contains(&j)
}

fn add((i, j): Pos, (di, dj): Pos) -> Pos {
    (i.wrapping_add(di), j.wrapping_add(dj))
}

impl super::People {
    /// `1110:3c2e`'s direction `d` (saved with the people), `(0, 0)` past
    /// its five.
    fn direction(&self, d: u8) -> Pos {
        let word = |k: usize| {
            self.directions
                .get(k * 2..k * 2 + 2)
                .map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
        };
        let d = usize::from(d);
        (word(2 * d), word(2 * d + 1))
    }

    /// The path a walker takes onto a square with the road piece `ground`
    /// going in direction `d`: `NEINTRAG.DAT`'s row `ground - 10`.
    fn entry(&self, ground: u8, d: u8) -> i16 {
        let index = (i32::from(ground) - 10) * 4 + i32::from(d);
        i16::from(at(&self.entries, index))
    }
}

impl World {
    fn person(&self, n: i16) -> Person {
        self.people.get(n).cloned().unwrap_or_default()
    }

    fn edit(&mut self, n: i16, f: impl FnOnce(&mut Person)) {
        if let Some(p) = self.people.get_mut(n) {
            f(p);
        }
    }

    /// A marker layer on `pos`, 255 off the map (the original reads past
    /// the layer).
    fn marker(&self, pos: Pos) -> u8 {
        self.island.layer_551c.get(pos).unwrap_or(FREE)
    }

    fn marker_2(&self, pos: Pos) -> u8 {
        self.island.layer_5520.get(pos).unwrap_or(FREE)
    }

    /// A square's form as the player's view turns it. // 1088:4bfa
    pub(super) fn form_look(&self, pos: Pos) -> u8 {
        let form = usize::from(self.form_or_flat(pos));
        self.tables
            .form_korrektur
            .get(self.viewpoint.rotation())
            .and_then(|row| row.get(form))
            .copied()
            .unwrap_or(0)
    }

    /// The height of the corner the view measures `pos` from, 0 off the
    /// map. // 1088:4c27
    fn view_corner(&self, pos: Pos) -> u8 {
        if on_map(pos) {
            ground_height(&self.island, self.viewpoint.rotation(), pos) // 1050:075e
        } else {
            0
        }
    }

    /// How a plane's picture is lifted by its square: the highest corner
    /// and the form. // 1090:2ce5
    fn plane_ground(&self, pos: Pos) -> i16 {
        let lift = at(&FORM_LIFT, self.form_or_flat(pos));
        let height = i16::from(self.island.highest_corner(pos)); // 1000:3e51
        #[allow(clippy::cast_possible_truncation)] // FORM_LIFT is within ±18.
        let lift = lift as i16;
        (height - 1).wrapping_mul(9).wrapping_add(lift)
    }

    /// The people and vehicles move, each record from the money on in
    /// turn: [`World::person_animate`], [`World::person_step`], and while
    /// people move the walkers along their paths. // 1090:5293
    ///
    /// The original first counts down `1110:723c[1..42]`, how long each of
    /// the sounds still plays (`play_sound`, 10a0:30b6, plays a sound only
    /// at 0 and sets its time from `1110:3dce`): that's the view's, which
    /// plays the sounds in [`Tick::sounds`].
    pub(crate) fn people_tick(&mut self, tick: &mut Tick) {
        for n in FIRST_MONEY..=LAST {
            self.person_animate(n, tick); // 1098:109f
            self.person_step(n, tick); // 1090:2ce5
            if self.options.people_move && WALKERS.contains(&self.people.kind(n)) {
                self.walk(n, tick);
            }
        }
    }

    /// A walker's step along its path, by what its row's byte 3 says
    /// happens there. // 1090:5293
    fn walk(&mut self, n: i16, tick: &mut Tick) {
        self.edit(n, |p| p.set_step(p.step().wrapping_add(1)));
        let p = self.person(n);
        let ground = self.ground_or_water(p.pos());
        let row = i32::from(row(&p));
        match self.people.path_byte(p.path(), row, 3) {
            0 | 10..=13 => self.place_walker(n), // 1088:4c55
            1 => self.walker_at_junction(n, tick),
            what @ 2..=9 => {
                let c = &CROSSINGS[usize::from(what - 2)];
                if c.grounds.contains(&ground) {
                    let open = match c.road {
                        Some(d) if !self.road_ahead(n, d) => true,
                        _ => !self.walker_blocked(n, c.blocked),
                    };
                    if open {
                        self.turn_walker(c.turn, c.path, n); // 1088:6769
                    }
                }
                self.place_walker(n);
            }
            0xff => {
                let facing = self.people.path_byte(p.path(), row, 2);
                match facing {
                    0..=3 => {
                        if self.road_ahead(n, facing) {
                            let next = add(p.pos(), NEIGHBOURS[usize::from(facing)]);
                            let path = self.people.entry(self.ground_or_water(next), facing);
                            self.edit(n, |p| {
                                p.set_path(path);
                                p.set_pos(next);
                            });
                        } else {
                            self.add_person(0x19, p.pos(), tick);
                            self.remove_person(n); // 1088:2df2
                        }
                        self.edit(n, |p| p.set_step(0));
                    }
                    4..=0x13 => {
                        let (path, step) = EXITS[usize::from(facing - 4)];
                        self.edit(n, |p| {
                            p.set_path(path);
                            p.set_step(step);
                        });
                    }
                    _ => {}
                }
                self.place_walker(n);
            }
            _ => {}
        }
    }

    /// A walker at a junction: turns off where there's no road ahead,
    /// then claims the square ahead or, while someone's in the way there,
    /// waits (its step goes back) and maybe makes a sound. // 1090:5293
    fn walker_at_junction(&mut self, n: i16, tick: &mut Tick) {
        let path = self.person(n).path();
        if let Some(&(_, d, turn)) = JUNCTIONS.iter().find(|j| j.0 == path)
            && !self.road_ahead(n, d)
        {
            self.free_square(n); // 1088:571e
            self.edit(n, |p| {
                p.set_path(turn);
                p.set_step(0);
            });
        }
        let d = match self.person(n).path() {
            0 | 9 | 10 | 0xc | 0x10 | 0x16 => 0,
            1 | 5 | 6 | 0xd | 0x11 | 0x17 => 1,
            2 | 4 | 0xb | 0xe | 0x13 | 0x15 => 2,
            3 | 7 | 8 | 0xf | 0x12 | 0x14 => 3,
            _ => return,
        };
        if !self.walker_blocked(n, d) {
            self.claim_ahead(d, n); // 1088:5f3e
        } else if self.person(n).step() != 0 {
            self.edit(n, |p| p.set_step(p.step().wrapping_sub(1)));
            if self.visible(self.person(n).pos()) {
                self.walker_sound(n, tick); // 1088:68f1
            }
        }
    }

    /// Whether there's road in direction `d` (1110:3c2e) of walker `n`:
    /// a road piece (ground 10..=0x3a), or a bridge (buildings 11, 0x34).
    /// // 1088:4e18
    fn road_ahead(&self, n: i16, d: u8) -> bool {
        let next = add(self.person(n).pos(), self.people.direction(d));
        let ground = self.ground_or_water(next);
        if ground < 10 {
            return false;
        }
        ground <= 0x3a || [0xb, 0x34].contains(&self.building_or_none(next))
    }

    /// Whether someone is in walker `n`'s way in direction `d`: the square
    /// ahead is taken in either marker layer, and not by walkers on paths
    /// it may join ([`JOINABLE`]). // 1088:5386
    fn walker_blocked(&self, n: i16, d: u8) -> bool {
        let next = add(self.person(n).pos(), self.people.direction(d));
        if self.marker(next) == FREE && self.marker_2(next) == FREE {
            return false;
        }
        let d = match self.person(n).path() {
            0 | 9 | 10 | 0xc | 0x10 | 0x16 => 0,
            1 | 5 | 6 | 0xd | 0x11 | 0x17 => 1,
            2 | 4 | 0xb | 0xe | 0x13 | 0x15 => 2,
            3 | 7 | 8 | 0xf | 0x12 | 0x14 => 3,
            _ => return false,
        };
        let (paths, junction, other, pieces) = JOINABLE[d];
        let ground = self.ground_or_water(next);
        let sixth = if pieces.contains(&ground) { junction } else { other };
        let [a, b, c, e, f] = paths;
        !self.walker_may_join(next, [a, b, sixth, c, e, f])
    }

    /// Whether a walker may go onto `pos` with whoever's there: the one
    /// marker layer is free and the other holds one of `paths`.
    /// // 1088:4f4f
    fn walker_may_join(&self, pos: Pos, paths: [u8; 6]) -> bool {
        let (m1, m2) = (self.marker(pos), self.marker_2(pos));
        let mut join = false;
        if m2 == FREE && paths.contains(&m1) {
            join = true;
        }
        if m1 == FREE && paths.contains(&m2) {
            join = true;
        }
        join
    }

    /// Walker `n` claims the square ahead in direction `d` for the path it
    /// will take there (`NEINTRAG.DAT` by the road piece), in the first free
    /// marker layer, and frees its own square. // 1088:5f3e
    fn claim_ahead(&mut self, d: u8, n: i16) {
        let p = self.person(n);
        let next = add(p.pos(), self.people.direction(d));
        if self.marker(next) == FREE || self.marker_2(next) == FREE {
            let ground = self.ground_or_water(next);
            if ground > 9 {
                let path = self.people.entry(ground, d).to_le_bytes()[0];
                if self.marker(next) == FREE {
                    self.set_marker(next, path);
                } else {
                    self.set_marker_2(next, path);
                }
            }
        }
        let pos = p.pos();
        self.set_marker(pos, FREE);
        self.set_marker_2(pos, FREE);
        // The original then frees whichever layer holds its path, which
        // it just freed.
        let path = i16::from(self.marker(pos));
        if path == p.path() {
            self.set_marker(pos, FREE);
        } else if i16::from(self.marker_2(pos)) == p.path() {
            self.set_marker_2(pos, FREE);
        }
    }

    /// Walker `n` frees its square: each marker layer that holds its path,
    /// then both. // 1088:571e
    fn free_square(&mut self, n: i16) {
        let p = self.person(n);
        let pos = p.pos();
        if i16::from(self.marker(pos)) == p.path() {
            self.set_marker(pos, FREE);
        }
        if i16::from(self.marker_2(pos)) == p.path() {
            self.set_marker_2(pos, FREE);
        }
        self.set_marker(pos, FREE);
        self.set_marker_2(pos, FREE);
    }

    /// Walker `n` marks its square with its path, in the first free layer.
    /// // 1088:5e73
    fn mark_square(&mut self, n: i16) {
        let p = self.person(n);
        let path = p.path().to_le_bytes()[0];
        if self.marker(p.pos()) == FREE {
            self.set_marker(p.pos(), path);
        } else if self.marker_2(p.pos()) == FREE {
            self.set_marker_2(p.pos(), path);
        }
    }

    /// Walker `n` turns onto `path` at a crossing: always where there's no
    /// road in direction `d` (60 % of the time, `Random(100)`, for the
    /// paths 5, 7, 9 and 11), else half the time (`Random(100)`) if it
    /// has its square to itself. // 1088:6769
    fn turn_walker(&mut self, d: u8, path: i16, n: i16) {
        let turn = if self.road_ahead(n, d) {
            let p = self.person(n);
            let (m1, m2) = (self.marker(p.pos()), self.marker_2(p.pos()));
            let alone = (m1 == FREE && i16::from(m2) == p.path()) || (m2 == FREE && i16::from(m1) == p.path());
            alone && self.random.random(100) < 0x32
        } else if [5, 7, 9, 0xb].contains(&path) {
            self.random.random(100) < 0x3c
        } else {
            true
        };
        if turn {
            self.free_square(n); // 1088:571e
            self.edit(n, |p| p.set_path(path));
            self.mark_square(n); // 1088:5e73
        }
    }

    /// A walker the player sees waiting: one time in a hundred
    /// (`Random(100)`) it leaves; its sound by its kind, some by
    /// `Random(3)`. // 1088:68f1
    fn walker_sound(&mut self, n: i16, tick: &mut Tick) {
        if self.random.random(100) == 1 {
            self.remove_person(n); // 1088:2df2
        }
        match self.people.kind(n) {
            0x11 => tick.sounds.push(4),
            0x12 => tick.sounds.push(0x2b),
            0x13 | 0x14 => tick.sounds.push(3),
            0x15 | 0x16 => match self.random.random(3) {
                0 => tick.sounds.push(2),
                1 => tick.sounds.push(0xf),
                2 => tick.sounds.push(0x2c),
                _ => {}
            },
            _ => {}
        }
    }

    /// A sound from record `n`'s square, if the player sees it.
    fn sound_from(&self, n: i16, sound: u8, tick: &mut Tick) {
        self.sound_at(self.person(n).pos(), sound, tick);
    }
}

/// 1098:109f, the animations.
impl World {
    /// Record `n`'s animation while people move (ships, boats, the
    /// things at the buildings' doors, surfers, harbour ships, the
    /// helicopter, the taxiing planes), then, whether or not they move,
    /// the money rising, the oil, and the kinds 0x19, 0x25, 0x2a and 0x46
    /// counting down. // 1098:109f, about 10 KB
    pub(super) fn person_animate(&mut self, n: i16, tick: &mut Tick) {
        let kind = self.people.kind(n);
        if self.options.people_move {
            match kind {
                SHIP => self.animate_ship(n, tick),
                BOAT | 4 => {
                    if self.count_frame(n) {
                        self.remove_person(n); // 1088:2df2
                        if kind == BOAT {
                            self.people.boats = self.people.boats.wrapping_sub(1);
                        }
                        let pos = self.person(n).pos();
                        if kind == BOAT {
                            self.set_marker(pos, FREE);
                        }
                    } else {
                        let p = self.person(n);
                        let sum = p.path().wrapping_add(p.pos().0).wrapping_add(p.pos().1);
                        let k = if kind == BOAT { sum % 0xc + 10 } else { sum % 10 };
                        let sprite = at(&SHIP_SPRITES, k).wrapping_add(0x18);
                        self.edit(n, |p| p.set_sprite(sprite));
                    }
                }
                0x20 | 0x21 => self.animate_surfer(n),
                0x22..=0x24 => self.animate_harbour_ship(n, tick),
                HELICOPTER => self.animate_helicopter(n, tick),
                0x35..=0x3a => self.animate_taxiing(n, kind),
                _ => {
                    if let Some(&(_, frames, first)) = IN_PLACE.iter().find(|a| a.0 == kind) {
                        self.animate_in_place(n, kind, frames, first, tick);
                    }
                }
            }
        }
        // The end of 1098:109f, whether or not people move.
        let kind = self.people.kind(n);
        match kind {
            0x2a | 0x46 => {
                if self.count_frame(n) {
                    self.remove_person(n);
                }
            }
            0x19 | LEAVING => {
                let first = if kind == 0x19 { 0xdb } else { 0x14b };
                if self.count_frame(n) {
                    self.remove_person(n);
                } else {
                    self.edit(n, |p| p.set_sprite(p.path().wrapping_add(first)));
                }
            }
            MONEY => {
                self.edit(n, |p| p.set_offset((p.offset().0, p.offset().1.wrapping_sub(1))));
                if self.person(n).offset().1 == 0 {
                    self.remove_person(n);
                }
            }
            0x2b..=0x2d => {
                if self.count_frame(n) {
                    self.edit(n, |p| p.set_path(0));
                }
                self.edit(n, |p| {
                    let sprite = (i16::from(kind) - 0x2b) * 3 + 399 + p.path() / 10;
                    p.set_sprite(sprite);
                });
            }
            _ => {}
        }
    }

    /// The next frame (`+0x18`); whether the time is up.
    fn count_frame(&mut self, n: i16) -> bool {
        self.edit(n, |p| p.set_path(p.path().wrapping_add(1)));
        time_up(&self.person(n))
    }

    /// Someone in place at a building, frame by frame until the time is up.
    /// The zoo's (0x1d) first frame and the minigolf players' putt have a
    /// sound where the player sees them. // 1098:109f
    fn animate_in_place(&mut self, n: i16, kind: u8, frames: Frames, first: i16, tick: &mut Tick) {
        if kind == 0x1d && self.visible(self.person(n).pos()) && self.person(n).path() == 0 {
            tick.sounds.push(7);
        }
        self.edit(n, |p| p.set_path(p.path().wrapping_add(1)));
        if (0x47..=0x4c).contains(&kind) && self.person(n).path() == PUTT {
            self.sound_from(n, 0x29, tick);
        }
        if time_up(&self.person(n)) {
            self.remove_person(n); // 1088:2df2
            return;
        }
        self.edit(n, |p| {
            let frame = p.path();
            let picture = match frames {
                Frames::Each => frame,
                Frames::Half => frame / 2,
                Frames::Cycle(k) => frame % k,
            };
            p.set_sprite(picture.wrapping_add(first));
            if kind == 0x1a && frame == 7 {
                p.set_offset((-11, -13));
            }
            if kind == 0x34 && frame == 6 {
                p.set_offset((8, 3));
            }
        });
    }

    /// A ship on the open sea (kind 2): rocks until its time is up, and
    /// every 28 frames drifts a square towards the top of the screen if
    /// that's open sea (with a sound where the player sees it), else it
    /// goes. // 1098:109f
    fn animate_ship(&mut self, n: i16, tick: &mut Tick) {
        let gone = |w: &mut World| {
            w.people.ships = w.people.ships.wrapping_sub(1);
            w.remove_person(n); // 1088:2df2
            let pos = w.person(n).pos();
            w.set_marker(pos, FREE);
        };
        if self.count_frame(n) {
            self.remove_person(n);
            self.people.ships = self.people.ships.wrapping_sub(1);
            let pos = self.person(n).pos();
            self.set_marker(pos, FREE);
            return;
        }
        let p = self.person(n);
        let sprite = at(&SHIP_SPRITES, p.path() % 0x1c + 0x16).wrapping_add(0x18);
        self.edit(n, |p| p.set_sprite(sprite));
        if p.path() % 0x1c != 0 {
            return;
        }
        let next = add(p.pos(), DRIFT[self.viewpoint.rotation()]);
        if off_inner(next) {
            gone(self);
        } else if self.ground_or_water(next) == WATER
            && self.form_or_flat(next) == FLAT
            && self.marker_or_zero(next) == FREE
        {
            self.sound_from(n, 1, tick);
            self.set_marker(p.pos(), FREE);
            self.edit(n, |p| p.set_pos(next));
            self.set_marker(next, SHIP);
        } else {
            gone(self);
        }
    }

    /// A route's next row for a ship or surfer (`+0x1a`): past row 15 it
    /// moves a square in its heading (`+8`), and the marker layer goes
    /// with it. // 1098:109f
    fn sail(&mut self, n: i16) {
        self.edit(n, |p| p.set_step(p.step().wrapping_add(1)));
        let p = self.person(n);
        if p.step() > 0xf {
            self.edit(n, |p| p.set_step(0));
            self.set_marker(p.pos(), FREE);
            let next = add(p.pos(), at(&MOVES, p.heading()));
            self.edit(n, |p| p.set_pos(next));
            self.set_marker(next, p.kind());
        }
    }

    /// Whether `p`'s row of its `ANIMDAT4.DAT` route is the square's centre.
    fn route_ends(&self, p: &Person) -> bool {
        let t = &self.tables;
        (t.animdat4(p.heading(), row(p), 0), t.animdat4(p.heading(), row(p), 1)) == CENTRE
    }

    /// Which way to turn from `p` towards its target (`+0x1e`): a heading,
    /// or 255 when it's there. // 1110:3792
    fn turn_towards(p: &Person) -> u8 {
        let ((i, j), (ti, tj)) = (p.pos(), p.target());
        let mask = usize::from(ti < i) + 2 * usize::from(i < ti) + 4 * usize::from(tj < j) + 8 * usize::from(j < tj);
        TURNS[mask]
    }

    /// A surfer (kinds 0x20, 0x21) rides its route; at its end it turns
    /// towards its target, or picks another one near it when it's there or
    /// the sea ahead isn't open, and leaves when it finds none.
    /// // 1098:109f
    fn animate_surfer(&mut self, n: i16) {
        self.sail(n);
        let p = self.person(n);
        if self.route_ends(&p) {
            self.surfer_turns(n);
            if !self.open_sea_ahead(n) {
                self.pick_target(0x14, n, 5); // 1098:03b3
                self.surfer_turns(n);
                if !self.open_sea_ahead(n) {
                    self.remove_person(n); // 1088:2df2
                }
            }
        }
        self.place_surfer(n); // 1088:6ee7
    }

    fn surfer_turns(&mut self, n: i16) {
        match Self::turn_towards(&self.person(n)) {
            255 => self.pick_target(0x14, n, 5), // 1098:03b3
            heading => self.head(n, i16::from(heading)),
        }
    }

    /// A new heading, from its route's first row. // 1110:3c64
    fn head(&mut self, n: i16, heading: i16) {
        let start = i32::from(at(&ROUTE_STARTS, heading));
        self.edit(n, |p| {
            p.set_heading(heading);
            p.set_step(start);
        });
    }

    /// Whether `pos` is open sea: flat water without a building or a
    /// marker. // 1098:01c0
    fn open_sea(&self, pos: Pos) -> bool {
        self.ground_or_water(pos) == WATER
            && self.form_or_flat(pos) == FLAT
            && self.marker_or_zero(pos) == FREE
            && self.building_or_none(pos) == NO_BUILDING
    }

    /// Whether the square ahead of `n` in its heading is open sea.
    /// // 1098:0357
    fn open_sea_ahead(&self, n: i16) -> bool {
        let p = self.person(n);
        self.open_sea(add(p.pos(), at(&MOVES, p.heading())))
    }

    /// Up to `tries + 1` random squares within `size` around `n` (a
    /// `Random(size)` for `i`, then `j`) for an open sea target; `n` leaves
    /// when the last one isn't. // 1098:03b3
    fn pick_target(&mut self, tries: i16, n: i16, size: u16) {
        let half = (size.cast_signed() - 1) / 2;
        let mut k = 0;
        while k <= tries {
            let p = self.person(n);
            let i = p
                .pos()
                .0
                .wrapping_add(self.random.random(size).cast_signed())
                .wrapping_sub(half);
            let j = p
                .pos()
                .1
                .wrapping_add(self.random.random(size).cast_signed())
                .wrapping_sub(half);
            if self.open_sea((i, j)) {
                k = tries;
                self.edit(n, |p| p.set_target((i, j)));
            } else if k == tries {
                self.remove_person(n); // 1088:2df2
            }
            if k == tries {
                break;
            }
            k += 1;
        }
    }

    /// A ship's or surfer's picture (from `first`, by the look of its
    /// route's row as the view turns it) and where it goes on its square:
    /// the row of the route the view's rotation turns its heading into.
    fn place_on_route(&mut self, n: i16, first: Option<i16>) {
        let r = self.viewpoint.rotation();
        let t = Arc::clone(&self.tables);
        let p = self.person(n);
        if let Some(first) = first {
            let look = at(&LOOKS[r], t.animdat4(p.heading(), row(&p), 2));
            self.edit(n, |p| p.set_sprite(look.wrapping_add(first)));
        }
        let p = self.person(n);
        let group = at(&LOOKS[r], p.heading());
        let (w, h) = (t.animdat1(p.sprite(), 1) / 2, t.animdat1(p.sprite(), 0) / 2);
        let x = t.animdat4(group, row(&p), 0).wrapping_sub(w);
        let y = t.animdat4(group, row(&p), 1).wrapping_sub(h);
        self.edit(n, |p| p.set_offset((x, y)));
    }

    /// A surfer's picture and place. // 1088:6ee7
    fn place_surfer(&mut self, n: i16) {
        let first = match self.people.kind(n) {
            0x20 => Some(0x123),
            0x21 => Some(299),
            _ => None,
        };
        self.place_on_route(n, first);
    }

    /// A harbour ship's picture and place. // 1088:6cba
    fn place_harbour_ship(&mut self, n: i16) {
        let first = match self.people.kind(n) {
            0x22 => Some(0x133),
            0x23 => Some(0x13b),
            0x24 => Some(0x143),
            _ => None,
        };
        self.place_on_route(n, first);
    }

    /// A ship for a harbour (kinds 0x22..=0x24) sails from its corner of
    /// the map to the harbour (`+0x1e`), waits there a hundred ticks
    /// (`+0x18`) with a sound, and sails back to the corner (`+0x22`),
    /// where it leaves. A ship in its way makes it take a detour
    /// (1098:04f9) for some squares (`+0x26`). // 1098:109f
    fn animate_harbour_ship(&mut self, n: i16, tick: &mut Tick) {
        let harbour_ship = |w: &World| (0x22..=0x24).contains(&w.people.kind(n));
        if self.person(n).path() == 0 {
            self.sail(n);
            let p = self.person(n);
            if self.route_ends(&p) {
                if p.word_26() == 0 {
                    match Self::turn_towards(&p) {
                        255 => {
                            if p.pos() == p.target() {
                                self.edit(n, |p| {
                                    p.set_target(p.home());
                                    p.set_path(100);
                                });
                                if let Some(heading) = self.ship_heading(n) {
                                    self.edit(n, |p| p.set_heading(heading));
                                }
                                self.sound_from(n, 0x23, tick);
                            }
                            let p = self.person(n);
                            if p.pos() == p.home() {
                                self.remove_person(n); // 1088:2df2
                            }
                        }
                        heading => self.head(n, i16::from(heading)),
                    }
                    let heading = self.person(n).heading();
                    if self.ship_blocked(n, heading, true) {
                        self.detour(n); // 1098:04f9
                    }
                } else if self.ship_blocked(n, p.heading(), false) {
                    self.remove_person(n);
                } else {
                    let next = add(p.pos(), at(&MOVES, p.heading()));
                    if off_inner(next) {
                        self.remove_person(n);
                    } else {
                        // It marks the square ahead, without going there.
                        self.set_marker(p.pos(), FREE);
                        self.set_marker(next, p.kind());
                        self.edit(n, |p| p.set_word_26(p.word_26().wrapping_sub(1)));
                    }
                }
            }
        } else {
            self.edit(n, |p| p.set_path(p.path().wrapping_sub(1)));
            if harbour_ship(self) {
                self.place_harbour_ship(n); // 1088:6cba
            }
        }
        if harbour_ship(self) {
            self.place_harbour_ship(n); // 1088:6cba
        }
    }

    /// Which way a ship leaves its harbour: 2 with the harbour (building
    /// types 0x58..=0x6b) at `i ± 1`, 0 at `j ± 1`; otherwise it leaves.
    /// The original then returns an undefined value; the port keeps the
    /// heading. // 1098:0002
    fn ship_heading(&mut self, n: i16) -> Option<i16> {
        let (i, j) = self.person(n).pos();
        let harbour = |w: &World, pos| (0x58..0x6c).contains(&w.building_or_none(pos));
        if harbour(self, (i + 1, j)) || harbour(self, (i - 1, j)) {
            return Some(2);
        }
        if harbour(self, (i, j - 1)) || harbour(self, (i, j + 1)) {
            return Some(0);
        }
        self.remove_person(n); // 1088:2df2
        None
    }

    /// Whether the way ahead in `heading` isn't open sea (with `markers`,
    /// also when it's marked); a heading outside 0..=7 makes the ship
    /// leave and counts as open. // 1098:0217, 1098:02c3
    fn ship_blocked(&mut self, n: i16, heading: i16, markers: bool) -> bool {
        if !(0..=7).contains(&heading) {
            self.remove_person(n); // 1088:2df2
            return false;
        }
        let next = add(self.person(n).pos(), at(&MOVES, heading));
        let open = self.ground_or_water(next) == WATER
            && self.form_or_flat(next) == FLAT
            && self.building_or_none(next) == NO_BUILDING
            && (!markers || self.marker_or_zero(next) == FREE);
        !open
    }

    /// A blocked harbour ship looks for a way round: the first of six
    /// headings (`DETOURS` by `Random(8)`) with ten squares of flat water
    /// ahead, for nine squares; else (with another `Random(8)`) the first
    /// with water on its own square, which the original checks five times
    /// instead of the squares ahead, for four; else it leaves. Then one
    /// time in fifty (`Random(50)`) it leaves anyway. // 1098:04f9
    fn detour(&mut self, n: i16) {
        let water =
            |w: &World, pos: Pos| w.ground_or_water(pos) == WATER && w.form_or_flat(pos) == FLAT && !off_inner(pos);
        let mut found = false;
        let turns = DETOURS[usize::from(self.random.random(8))];
        for turn in turns {
            let p = self.person(n);
            let heading = wrap_heading(p.heading().wrapping_add(turn));
            let (di, dj) = at(&MOVES, heading);
            let clear = (0..=9).all(|k: i16| water(self, add(p.pos(), (di.wrapping_mul(k), dj.wrapping_mul(k)))));
            if clear {
                self.edit(n, |p| {
                    p.set_word_26(9);
                    p.set_heading(heading);
                });
                found = true;
                break;
            }
        }
        let turns = DETOURS[usize::from(self.random.random(8))];
        if !found {
            for turn in turns {
                let p = self.person(n);
                if water(self, p.pos()) {
                    self.edit(n, |p| {
                        p.set_word_26(4);
                        p.set_heading(wrap_heading(p.heading().wrapping_add(turn)));
                    });
                    found = true;
                    break;
                }
            }
            if !found {
                self.remove_person(n); // 1088:2df2
            }
        }
        if self.random.random(0x32) == 1 {
            self.remove_person(n);
        }
    }

    /// The sightseeing helicopter, by its state (`+0x1e`): 3 waits
    /// (`+0x18`), 0 climbs to 220 with a sound and picks a number of
    /// laps (`Random(5) + 3`), 2 flies its tracks (`+8`) round, a turn
    /// after each `+0x22` of them, until the laps (`+0x20`) are done, 1
    /// lands, and 4 waits and leaves. // 1098:109f
    fn animate_helicopter(&mut self, n: i16, tick: &mut Tick) {
        let p = self.person(n);
        match p.target().0 {
            0 => {
                self.sound_from(n, 0x1d, tick);
                if p.word_26() < 0xdc {
                    self.edit(n, |p| p.set_word_26(p.word_26().wrapping_add(3)));
                } else {
                    self.edit(n, |p| {
                        p.set_target((2, 8));
                        p.set_home((p.home().0, 0));
                    });
                    let laps = self.random.random(5).cast_signed() + 3;
                    self.edit(n, |p| {
                        p.set_path(laps);
                        p.set_home((laps, p.home().1));
                    });
                }
            }
            1 => {
                if p.word_26() > 0 {
                    self.edit(n, |p| {
                        p.set_word_26(p.word_26().wrapping_sub(3));
                        if p.word_26() < 1 {
                            p.set_word_26(0);
                            p.set_target((4, p.target().1));
                            p.set_path(0x14);
                        }
                    });
                }
            }
            2 => {
                self.fly_track(n);
                let p = self.person(n);
                if self.track_ends(&p, p.heading()) {
                    self.edit(n, |p| {
                        p.set_home((p.home().0, p.home().1.wrapping_add(1)));
                        if p.home().1 == p.home().0 {
                            p.set_home((p.home().0, 0));
                            p.set_heading(wrap_heading(p.heading().wrapping_add(1))); // 1098:0497
                            p.set_target((p.target().0, p.target().1.wrapping_sub(1)));
                            if p.target().1 == 0 {
                                p.set_target((1, p.target().1));
                            }
                        }
                        p.set_step(i32::from(at(&TRACK_ENDS, p.heading()).1));
                    });
                }
            }
            3 => {
                self.edit(n, |p| {
                    p.set_path(p.path().wrapping_sub(1));
                    if p.path() == 0 {
                        p.set_target((0, p.target().1));
                    }
                });
            }
            4 => {
                self.edit(n, |p| p.set_path(p.path().wrapping_sub(1)));
                if self.person(n).path() == 0 {
                    self.remove_person(n); // 1088:2df2
                    let pos = self.person(n).pos();
                    self.set_marker(pos, FREE);
                }
            }
            _ => {}
        }
        self.place_plane(n); // 1088:75de
    }

    /// The next row of the track `+8` (helicopter) or `+0x18` (planes):
    /// past its last row the record and its second half move a square.
    fn fly(&mut self, n: i16, track: i16) {
        let p = self.person(n);
        if p.step() > i32::from(at(&TRACK_ENDS, track).0) {
            let next = add(p.pos(), at(&MOVES, track));
            self.edit(n, |p| {
                p.set_step(0);
                p.set_pos(next);
            });
            self.edit(n + PLANE_HALF, |q| q.set_pos(next));
        }
    }

    fn fly_track(&mut self, n: i16) {
        self.edit(n, |p| p.set_step(p.step().wrapping_add(1)));
        let track = self.person(n).heading();
        self.fly(n, track);
    }

    /// Whether `p`'s row of `ANIMDAT3.DAT` track `track` is the square's
    /// centre.
    fn track_ends(&self, p: &Person, track: i16) -> bool {
        let row = i32::from(row(p));
        (
            self.people.track_word(track, row, 0),
            self.people.track_word(track, row, 2),
        ) == CENTRE
    }

    /// A plane taxiing to its runway (kinds 0x35..=0x3a): every time its
    /// frame reaches its time it takes a step (`+0x20`), each few steps a
    /// square along the runway (`+0x1e` squares left); its picture by the
    /// view's rotation and the frame. // 1098:109f
    fn animate_taxiing(&mut self, n: i16, kind: u8) {
        let k = usize::from(kind - 0x35);
        if self.count_frame(n) {
            // (along j or i, steps per square)
            let (along_j, steps) = match kind {
                0x35 => (true, Some(4)),
                0x36 => (false, Some(4)),
                0x37 => (true, Some(2)),
                0x38 => (false, Some(2)),
                0x39 => (true, None),
                _ => (false, None),
            };
            let advance = |p: &mut Person| {
                let (i, j) = p.pos();
                p.set_pos(if along_j {
                    (i, j.wrapping_sub(1))
                } else {
                    (i.wrapping_sub(1), j)
                });
                p.set_target((p.target().0.wrapping_sub(1), p.target().1));
            };
            if let Some(steps) = steps {
                self.edit(n, |p| p.set_target((p.target().0, p.target().1.wrapping_sub(1))));
                match kind {
                    0x35 => self.turn_taxiing(n, &TAXIING_OFFSETS_35), // 1098:0745
                    0x36 => self.turn_taxiing(n, &TAXIING_OFFSETS_36), // 1098:0a14
                    0x37 => self.turn_taxiing(n, &TAXIING_OFFSETS_37), // 1098:0ce3
                    _ => self.turn_taxiing(n, &TAXIING_OFFSETS_38),    // 1098:0e6d
                }
                self.edit(n, |p| {
                    if p.target().1 == 0 {
                        advance(p);
                        p.set_target((p.target().0, steps));
                    }
                });
            } else {
                self.edit(n, advance);
            }
            if self.person(n).target().0 == 0 {
                self.remove_person(n); // 1088:2df2
            }
            self.edit(n, |p| p.set_path(1));
        }
        let first = TAXIING_SPRITES[self.viewpoint.rotation()][k];
        self.edit(n, |p| p.set_sprite(first.wrapping_add(p.path())));
    }
}

/// 1090:2ce5, the planes.
impl World {
    /// A plane (kinds 7, 8, 9) flies its tracks (`+0x18`, rows `+0x1a`) by
    /// its state (`+8`) while people move: from its corner of the map
    /// towards the airport (0x19, 0x1f), round a holding circle while the
    /// runway (`layer_551c` at `+0x22`: 255 free, 0 taken) is busy (0..=0x18,
    /// 0x24..=0x3c), down onto it (0x1a, 0x20), along it (0x1c, 0x21), a
    /// hundred ticks parked (0x1b, 0x23), up again (0x1d, 0x22) and off to
    /// the nearest corner (0x1e), where it leaves. Soft soap on the runway
    /// (sabotage 5) makes it skid off (0x3d, 0x3e) and wreck the square
    /// beyond. Then its picture and its second half's. // 1090:2ce5,
    /// 9498 bytes
    #[allow(clippy::too_many_lines)] // One routine of the original's.
    pub(super) fn person_step(&mut self, n: i16, tick: &mut Tick) {
        if !self.options.people_move {
            return;
        }
        let kind = self.people.kind(n);
        if !(PLANE..PLANE + 3).contains(&kind) {
            return;
        }
        let r = self.viewpoint.rotation();
        let base = (i16::from(kind) - 7) * 8 + 0x6b;
        // The picture by the look of the track's row, or of the track itself.
        let row_look = |w: &World, n: i16| {
            let p = w.person(n);
            base.wrapping_add(at(&LOOKS[r], w.people.track_word(p.path(), i32::from(row(&p)), 4)))
        };
        let track_look = |w: &World, n: i16| base.wrapping_add(at(&LOOKS[r], w.person(n).path()));
        // A new state, track and row, and its picture.
        let set = |w: &mut World, n: i16, state: i16, track: i16, step: Option<i32>, by_row: bool| {
            let step = step.unwrap_or_else(|| i32::from(at(&TRACK_ENDS, track).1));
            w.edit(n, |p| {
                p.set_heading(state);
                p.set_path(track);
                p.set_step(step);
            });
            let sprite = if by_row { row_look(w, n) } else { track_look(w, n) };
            w.edit(n, |p| p.set_sprite(sprite));
        };
        let leave = |w: &mut World, n: i16| {
            let corner = nearest_corner(w.person(n).pos()); // 1098:00ef
            w.edit(n, |p| p.set_target(corner));
            set(w, n, 0x1e, 3, Some(0), true);
        };
        // At the end of an approach or of the holding circle: away again
        // unless it's the airport's, onto a free runway (taking it), or
        // round the circle.
        let arrive = |w: &mut World, n: i16, landing: i16, by_row: bool, circle: i16| {
            if !w.on_approach(n) {
                leave(w, n);
                return;
            }
            let home = w.person(n).home();
            if w.marker(home) == FREE {
                w.set_marker(home, 0);
                let track = if landing == 0x1a { 0 } else { 2 };
                set(w, n, landing, track, None, by_row);
            } else {
                set(w, n, circle, i16::from(CIRCLE[0]), None, false);
            }
        };

        self.edit(n, |p| p.set_step(p.step().wrapping_add(1)));
        let mut crashed = false;
        let p = self.person(n);
        let state = p.heading();
        if PARKED.contains(&state) {
            if p.step() > 100 {
                let (state, track) = if state == 0x1b { (0x1d, 10) } else { (0x22, 0xd) };
                self.edit(n, |p| {
                    p.set_heading(state);
                    p.set_path(track);
                    p.set_step(i32::from(at(&TRACK_ENDS, track).1));
                });
                let corner = nearest_corner(self.person(n).pos()); // 1098:00ef
                self.edit(n, |p| p.set_target(corner));
                let sprite = row_look(self, n);
                self.edit(n, |p| p.set_sprite(sprite));
            }
        } else {
            let past = |p: &Person, track: i16| p.step() > i32::from(at(&TRACK_ENDS, track).0);
            match state {
                0..=0x1a | 0x1d..=0x20 | 0x22 | 0x24..=0x3c => self.fly(n, p.path()),
                0x3d => crashed = past(&p, 0),
                0x3e => crashed = past(&p, 4),
                0x1c if past(&p, 9) => self.edit(n, |p| {
                    p.set_heading(0x1b);
                    p.set_step(0);
                }),
                0x21 if past(&p, 0xc) => self.edit(n, |p| {
                    p.set_heading(0x23);
                    p.set_step(0);
                }),
                _ => {}
            }
            let p = self.person(n);
            let state = p.heading();
            if self.track_ends(&p, p.path()) {
                match state {
                    0..=0x18 => {
                        let state = state + 1;
                        self.edit(n, |p| p.set_heading(state));
                        if state == 0x18 {
                            arrive(self, n, 0x1a, false, 0);
                        } else {
                            set(self, n, state, at(&CIRCLE, state).into(), None, false);
                        }
                    }
                    0x24..=0x3c => {
                        let state = state + 1;
                        self.edit(n, |p| p.set_heading(state));
                        if state == 0x3c {
                            arrive(self, n, 0x20, false, 0x24);
                        } else {
                            set(self, n, state, at(&CIRCLE, state - 0x24).into(), None, false);
                        }
                    }
                    0x19 | 0x1e | 0x1f => match Self::turn_towards(&p) {
                        255 => match state {
                            0x19 => arrive(self, n, 0x1a, true, 0),
                            0x1e => self.remove_person(n), // 1088:2df2
                            _ => arrive(self, n, 0x20, true, 0x24),
                        },
                        track => set(self, n, state, track.into(), None, false),
                    },
                    0x1a | 0x20 => self.touch_down(n, tick),
                    0x1d | 0x22 => {
                        if self.take_off(n, true, tick) {
                            crashed = true;
                        }
                    }
                    0x1c => self.edit(n, |p| {
                        p.set_heading(0x1b);
                        p.set_step(0);
                    }),
                    0x21 => self.edit(n, |p| {
                        p.set_heading(0x23);
                        p.set_step(0);
                    }),
                    _ => {}
                }
            } else {
                match state {
                    0x1a | 0x20 => {
                        if self.land(n, state, tick) {
                            crashed = true;
                        }
                    }
                    0x1c => {
                        self.edit(n, |p| p.set_path(9));
                        let sprite = row_look(self, n);
                        self.edit(n, |p| p.set_sprite(sprite));
                    }
                    0x21 => {
                        self.edit(n, |p| p.set_path(0xc));
                        let sprite = row_look(self, n);
                        self.edit(n, |p| p.set_sprite(sprite));
                    }
                    0x1d | 0x22 => {
                        self.take_off(n, false, tick);
                    }
                    _ => {}
                }
            }
        }
        // LAB_1090_4dfa
        let p = self.person(n);
        if PARKED.contains(&p.heading()) || crashed {
            if crashed && SKIDDING.contains(&p.heading()) {
                self.end_soft_soap(p.pos()); // 10a0:0e17
                self.set_marker(p.home(), FREE);
                let (i, j) = p.pos();
                let wreck = if p.heading() == 0x3d {
                    (i.wrapping_sub(1), j)
                } else {
                    (i, j.wrapping_sub(1))
                };
                self.edit(n, |p| p.set_pos(wreck));
                self.add_person(0x19, wreck, tick);
                let _ = self.demolish(0, wreck, tick); // 1078:280b
                self.remove_person(n); // 1088:2df2
            }
            return;
        }
        self.place_flying(n);
    }

    /// A plane's picture on its track, lifted by its height (`+0x26`) over
    /// the view's corner and the form, and its second half's. // 1090:2ce5
    fn place_flying(&mut self, n: i16) {
        let r = self.viewpoint.rotation();
        let t = Arc::clone(&self.tables);
        let p = self.person(n);
        let (w, h) = (t.animdat1(p.sprite(), 1) / 2, t.animdat1(p.sprite(), 0) / 2);
        let track = at(&PLANE_TRACKS[r], p.path());
        let row = i32::from(row(&p));
        let (x, y) = (
            self.people.track_word(track, row, 0),
            self.people.track_word(track, row, 2),
        );
        let lift = at(&FORM_LIFT, self.form_look(p.pos())); // 1088:4bfa
        #[allow(clippy::cast_possible_truncation)] // FORM_LIFT is within ±18.
        let lift = lift as i16;
        let corner = i16::from(self.view_corner(p.pos())); // 1088:4c27
        let dy = y
            .wrapping_sub(p.word_26())
            .wrapping_sub(5)
            .wrapping_add((corner - 1).wrapping_mul(9).wrapping_add(lift))
            .wrapping_sub(h);
        self.edit(n, |p| p.set_offset((x.wrapping_sub(w), dy)));
        let base = (i16::from(p.kind()) - 7) * 8 + 0x83;
        let look = at(&LOOKS[r], self.people.track_word(p.path(), row, 4));
        self.edit(n + PLANE_HALF, |q| {
            q.set_sprite(base.wrapping_add(look));
            q.set_offset((x.wrapping_sub(w), y.wrapping_sub(h)));
        });
    }

    /// Whether the plane's home square (`+0x22`) is one of its airport's
    /// two approaches (1110:36a0; the original takes the second if either
    /// coordinate matches). // 1090:2c1f
    fn on_approach(&self, n: i16) -> bool {
        let home = self.person(n).home();
        let b = self.building_or_none(home);
        let Some(a) = b
            .checked_sub(FIRST_AIRPORT)
            .and_then(|k| APPROACHES.get(usize::from(k)))
        else {
            return false;
        };
        let (i, j) = self.building_origin(home); // 1030:154c
        (a[0].wrapping_add(i) == home.0 && a[1].wrapping_add(j) == home.1)
            || a[5].wrapping_add(i) == home.0
            || a[6].wrapping_add(j) == home.1
    }

    /// Whether the plane's square shows one of its airport's runway parts
    /// (`1110:36a0` columns `first` and `first + 5`); `false` off an
    /// airport (the original reads past its table).
    fn on_runway(&self, n: i16, first: usize) -> bool {
        let pos = self.person(n).pos();
        let b = self.building_or_none(pos);
        let part = i16::from(self.part_or_zero(pos));
        b.checked_sub(FIRST_AIRPORT)
            .and_then(|k| APPROACHES.get(usize::from(k)))
            .is_some_and(|a| part == a[first] || part == a[first + 5])
    }

    /// A plane coming down at the end of a track: 3 lower, or on the
    /// ground (with a sound). // 1090:2ce5
    fn touch_down(&mut self, n: i16, tick: &mut Tick) {
        let p = self.person(n);
        let ground = self.plane_ground(p.pos());
        if ground < p.word_26().wrapping_sub(3) {
            self.edit(n, |p| p.set_word_26(p.word_26().wrapping_sub(3)));
        } else {
            self.sound_from(n, 0x12, tick);
            self.landed(n, ground);
        }
    }

    /// On the ground: both halves' `+0x15` cleared, at the square's height.
    fn landed(&mut self, n: i16, ground: i16) {
        self.edit(n, |p| set_flag_15(p, 0));
        self.edit(n + PLANE_HALF, |q| set_flag_15(q, 0));
        self.edit(n, |p| p.set_word_26(ground));
    }

    /// A plane coming down within a track: 3 lower; on the ground off an
    /// airport it's gone (someone leaves); on its runway it rolls (0x1c,
    /// 0x21), or skids on soft soap (0x3d, 0x3e). Says whether it's gone.
    /// // 1090:2ce5
    fn land(&mut self, n: i16, state: i16, tick: &mut Tick) -> bool {
        let p = self.person(n);
        let ground = self.plane_ground(p.pos());
        if ground < p.word_26().wrapping_sub(3) {
            self.edit(n, |p| p.set_word_26(p.word_26().wrapping_sub(3)));
            return false;
        }
        if !(0x28..=0x33).contains(&self.building_or_none(p.pos())) {
            self.add_person(0x19, p.pos(), tick);
            self.remove_person(n); // 1088:2df2
            return true;
        }
        self.sound_from(n, 0x12, tick);
        self.landed(n, ground);
        if !self.on_runway(n, 2) {
            return false;
        }
        let soap = self.sabotage_at(5, p.pos()); // 10a0:0ec8
        let (state, track) = match (state, soap) {
            (0x1a, false) => (0x1c, 9),
            (0x1a, true) => (0x3d, 0),
            (_, false) => (0x21, 0xc),
            (_, true) => (0x3e, 2),
        };
        self.edit(n, |p| {
            p.set_heading(state);
            p.set_step(0);
            p.set_path(track);
        });
        let r = self.viewpoint.rotation();
        let p = self.person(n);
        let base = (i16::from(p.kind()) - 7) * 8 + 0x6b;
        let look = at(&LOOKS[r], self.people.track_word(track, 0, 4));
        self.edit(n, |p| p.set_sprite(base.wrapping_add(look)));
        false
    }

    /// A plane taking off: 3 higher, at 220 off to its corner (0x1e); on
    /// its runway's last part it frees the runway and its square and leaves
    /// the ground (with a sound). At the end of a track (`at_end`) one that
    /// isn't on an airport is gone (someone leaves). Says whether it's
    /// gone. // 1090:2ce5
    fn take_off(&mut self, n: i16, at_end: bool, tick: &mut Tick) -> bool {
        let p = self.person(n);
        let ground = self.plane_ground(p.pos());
        if ground < p.word_26().wrapping_sub(3) {
            self.edit(n, |p| p.set_word_26(p.word_26().wrapping_add(3)));
            if self.person(n).word_26() > 0xdb {
                self.edit(n, |p| {
                    p.set_word_26(0xdc);
                    p.set_heading(0x1e);
                    p.set_step(0);
                    p.set_path(3);
                });
                let r = self.viewpoint.rotation();
                let base = (i16::from(p.kind()) - 7) * 8 + 0x6b;
                let look = at(&LOOKS[r], self.people.track_word(3, 0, 4));
                self.edit(n, |p| p.set_sprite(base.wrapping_add(look)));
            }
            return false;
        }
        if at_end && !(0x28..=0x33).contains(&self.building_or_none(p.pos())) {
            self.add_person(0x19, p.pos(), tick);
            self.remove_person(n); // 1088:2df2
            return true;
        }
        if !self.on_runway(n, 3) {
            return false;
        }
        self.set_marker(p.home(), FREE);
        self.set_marker(p.pos(), FREE);
        self.edit(n, |p| set_flag_15(p, 1));
        self.edit(n + PLANE_HALF, |q| set_flag_15(q, 1));
        self.edit(n, |p| p.set_word_26(p.word_26().wrapping_add(3)));
        self.sound_from(n, 0x18, tick);
        false
    }

    /// The building on `pos` (or `pos` itself for a palm, beach life or
    /// nothing, `1110:1358`) has sabotage of `kind` on it, in any of the
    /// slots. // 10a0:0ec8
    fn sabotage_at(&self, kind: u8, pos: Pos) -> bool {
        let at = self.sabotage_square(pos);
        self.sabotage.iter().any(|s| s.pos == at && s.kind == kind)
    }

    /// The square sabotage on `pos` is kept at.
    fn sabotage_square(&self, pos: Pos) -> Pos {
        let b = self.island.buildings.get(pos).unwrap_or(NO_BUILDING);
        if UNSHAKEN.contains(&b) {
            pos
        } else {
            self.building_origin(pos) // 1030:154c
        }
    }

    /// The soft soap (sabotage 5) on the building on `pos` ends: its
    /// countdown goes to 1. // 10a0:0e17
    fn end_soft_soap(&mut self, pos: Pos) {
        let at = self.sabotage_square(pos);
        if let Some(s) = self.sabotage.iter_mut().find(|s| s.pos == at && s.kind == 5) {
            s.countdown = 1;
        }
    }
}

fn set_flag_15(p: &mut Person, value: u8) {
    let mut flags = p.flags();
    flags[1] = value;
    p.set_flags(flags);
}

#[cfg(test)]
mod tests {
    use vcl::random::Random;

    use super::super::{FIRST, GENERAL_END, NOBODY};
    use super::*;
    use crate::world::{Island, Tables};

    /// Flat grass with the sea around (0..10 and 60..), ANIMDAT1 pictures of
    /// 10 × 20 and the people's tables empty.
    fn world() -> World {
        let mut island = Island::empty();
        for i in 0..=250 {
            for j in 0..=250 {
                island.forms.set((i, j), FLAT);
                let land = (10..60).contains(&i) && (10..60).contains(&j);
                island.ground.set((i, j), if land { 1 } else { WATER });
                if land {
                    island.heights.set((i, j), 2);
                }
            }
        }
        let mut w = World {
            island,
            tables: Arc::new(Tables {
                hausnr: vec![[0; 8]; 0x80],
                animdat1: vec![[10, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]; 0x300],
                animdat4: vec![vec![[0; 3]; 16]; 8],
                ..Tables::default()
            }),
            random: Random::new(7),
            ..World::default()
        };
        w.people.last = GENERAL_END - 1;
        w
    }

    fn put(w: &mut World, n: i16, kind: u8, pos: Pos) -> &mut Person {
        let p = w.people.get_mut(n).unwrap();
        p.set_kind(kind);
        p.set_pos(pos);
        p
    }

    fn ticks(w: &mut World, n: usize) -> Tick {
        let mut tick = Tick::default();
        for _ in 0..n {
            w.people_tick(&mut tick);
        }
        tick
    }

    #[test]
    fn in_place_until_the_time_is_up() {
        let mut w = world();
        put(&mut w, FIRST, 0x27, (20, 20)).set_step(10);
        ticks(&mut w, 5);
        let p = w.person(FIRST);
        assert_eq!((p.kind(), p.path(), p.sprite()), (0x27, 5, 5 / 2 + 0x167));
        ticks(&mut w, 5);
        assert_eq!(w.people.kind(FIRST), NOBODY);
        // Without people moving, they stay.
        put(&mut w, FIRST, 0x27, (20, 20)).set_step(10);
        w.options.people_move = false;
        ticks(&mut w, 20);
        assert_eq!(w.people.kind(FIRST), 0x27);
    }

    #[test]
    fn the_money_rises_and_the_leaving_go() {
        let mut w = world();
        w.options.people_move = false;
        w.float_money(-100, (20, 20));
        put(&mut w, FIRST, LEAVING, (21, 20)).set_step(8);
        ticks(&mut w, 7);
        let p = w.person(FIRST);
        assert_eq!((p.kind(), p.sprite()), (LEAVING, 7 + 0x14b));
        assert_eq!(w.person(FIRST_MONEY).offset().1, 0xf - 7);
        ticks(&mut w, 1);
        assert_eq!(w.people.kind(FIRST), NOBODY);
        ticks(&mut w, 7);
        assert_eq!(w.people.kind(FIRST_MONEY), NOBODY);
    }

    #[test]
    fn ships_and_boats_go_when_their_time_is_up() {
        let mut w = world();
        let mut tick = Tick::default();
        w.add_person(BOAT, (5, 5), &mut tick);
        w.people.boats = 1;
        assert_eq!(w.island.layer_551c.at((5, 5)), BOAT);
        // A boat rocks for 255 ticks.
        ticks(&mut w, 254);
        assert_eq!(w.people.kind(FIRST), BOAT);
        ticks(&mut w, 1);
        assert_eq!(w.people.kind(FIRST), NOBODY);
        assert_eq!(w.people.boats, 0);
        assert_eq!(w.island.layer_551c.at((5, 5)), FREE);
    }

    #[test]
    fn ships_drift_up_the_screen() {
        let mut w = world();
        let mut tick = Tick::default();
        w.add_person(SHIP, (5, 100), &mut tick);
        w.people.ships = 1;
        let path = w.person(FIRST).path();
        let to_go = 0x1c - path % 0x1c;
        ticks(&mut w, usize::try_from(to_go).unwrap());
        // Every 28 frames a square towards the top of the screen.
        assert_eq!(w.person(FIRST).pos(), (4, 101));
        assert_eq!(w.island.layer_551c.at((4, 101)), SHIP);
        assert_eq!(w.island.layer_551c.at((5, 100)), FREE);
        // Land ahead: it goes.
        w.island.ground.set((3, 102), 1);
        ticks(&mut w, 0x1c);
        assert_eq!(w.people.kind(FIRST), NOBODY);
        assert_eq!(w.people.ships, 0);
    }

    #[test]
    fn taxiing_planes_roll_down_the_runway() {
        let mut w = world();
        // Kind 0x39: a square along j each time its frame reaches its time,
        // four squares in all.
        let p = put(&mut w, FIRST, 0x39, (30, 30));
        p.set_step(4);
        p.set_target((4, 0));
        ticks(&mut w, 4);
        let p = w.person(FIRST);
        assert_eq!((p.pos(), p.target().0, p.path()), ((30, 29), 3, 1));
        assert_eq!(p.sprite(), TAXIING_SPRITES[0][4] + 1);
        ticks(&mut w, 3 * 3);
        assert_eq!(w.people.kind(FIRST), NOBODY);
    }

    #[test]
    fn the_helicopter_waits_climbs_and_flies() {
        let mut w = world();
        let mut tick = Tick::default();
        w.island.buildings.set((30, 30), 0x17);
        w.add_person(HELICOPTER, (30, 30), &mut tick);
        let n = super::super::FIRST_PLANE;
        assert_eq!(w.person(n).target().0, 3);
        ticks(&mut w, 0x14);
        assert_eq!(w.person(n).target().0, 0);
        // Up to 220 and over, three a tick, then the laps by Random(5).
        ticks(&mut w, 74);
        let mut r = w.random;
        let laps = r.random(5).cast_signed() + 3;
        ticks(&mut w, 1);
        let p = w.person(n);
        assert_eq!(
            (p.word_26(), p.target(), p.path(), p.home()),
            (222, (2, 8), laps, (laps, 0))
        );
    }

    #[test]
    fn planes_wait_on_the_runway_then_take_off() {
        let mut w = world();
        let n = super::super::FIRST_PLANE;
        let p = put(&mut w, n, PLANE, (40, 40));
        p.set_heading(0x1b);
        p.set_step(0);
        ticks(&mut w, 100);
        assert_eq!(w.person(n).heading(), 0x1b);
        ticks(&mut w, 1);
        let p = w.person(n);
        assert_eq!((p.heading(), p.path(), p.step()), (0x1d, 10, 3));
        assert_eq!(p.target(), (0, 0));
    }

    #[test]
    fn soft_soap_on_the_runway() {
        let mut w = world();
        w.sabotage[2].kind = 5;
        w.sabotage[2].pos = (12, 13);
        w.sabotage[2].countdown = 50;
        assert!(w.sabotage_at(5, (12, 13)));
        assert!(!w.sabotage_at(4, (12, 13)));
        assert!(!w.sabotage_at(5, (12, 14)));
        w.end_soft_soap((12, 13));
        assert_eq!(w.sabotage[2].countdown, 1);
    }

    /// A road along `i` from (20, 30) to (25, 30), and ANIMDAT2 path 2
    /// taking four rows to cross a square going `i + 1`: three plain rows,
    /// then the end of the square.
    fn road() -> World {
        let mut w = world();
        for i in 20..=25 {
            w.island.ground.set((i, 30), 10);
        }
        for row in 0..3 {
            w.people.paths[2 * 0xbc + row * 4..][..4].copy_from_slice(&[1, 2, 0, 0]);
        }
        w.people.paths[2 * 0xbc + 3 * 4..][..4].copy_from_slice(&[1, 2, 0, 0xff]);
        // NEINTRAG: road piece 10 going i + 1 takes path 2.
        w.people.entries[0] = 2;
        w
    }

    #[test]
    fn walkers_follow_the_road_and_leave_at_its_end() {
        let mut w = road();
        let p = put(&mut w, FIRST, 0x11, (20, 30));
        p.set_path(2);
        p.set_step(-1);
        ticks(&mut w, 4);
        let p = w.person(FIRST);
        assert_eq!((p.pos(), p.path(), p.step()), ((21, 30), 2, 0));
        ticks(&mut w, 4 * 4);
        assert_eq!(w.person(FIRST).pos(), (25, 30));
        // No road ahead: someone leaving (0x19) instead.
        ticks(&mut w, 4);
        assert_eq!(w.people.kind(FIRST), NOBODY);
        assert_eq!(w.people.kind(FIRST + 1), 0x19);
        assert_eq!(w.person(FIRST + 1).pos(), (25, 30));
    }

    #[test]
    fn walkers_join_others_going_their_way() {
        let mut w = road();
        put(&mut w, FIRST, 0x11, (20, 30)).set_path(0);
        assert!(!w.walker_blocked(FIRST, 0));
        w.island.layer_551c.set((21, 30), 0x17);
        assert!(!w.walker_blocked(FIRST, 0));
        w.island.layer_551c.set((21, 30), 2);
        assert!(w.walker_blocked(FIRST, 0));
        w.island.layer_5520.set((21, 30), 0x17);
        assert!(w.walker_blocked(FIRST, 0));
        // Claiming the square ahead marks it in the free layer.
        w.island.layer_551c.set((21, 30), FREE);
        w.island.layer_5520.set((21, 30), FREE);
        w.island.layer_551c.set((20, 30), 0);
        w.claim_ahead(0, FIRST);
        assert_eq!(w.island.layer_551c.at((21, 30)), 2);
        assert_eq!(w.island.layer_551c.at((20, 30)), FREE);
    }

    #[test]
    fn harbour_ships_detour_round_the_land() {
        let mut w = world();
        let p = put(&mut w, FIRST, 0x22, (5, 5));
        p.set_heading(4);
        w.detour(FIRST);
        // The sea is open all round: the first detour of six works.
        let p = w.person(FIRST);
        assert!(p.kind() == NOBODY || p.word_26() == 9);
    }
}
