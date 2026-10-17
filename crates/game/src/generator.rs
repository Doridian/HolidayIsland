//! The island generator (segment `10b0`, 10b0:52d4): a new island from
//! the start menu's shape, relief, land and rivers, which state 1210 of
//! `THintergrund`'s timer makes for a normal game (`docs/island-generator.md`).
//!
//! It starts from the sea and raises hills: each a random quadrilateral
//! one step above the land under it, the land around following so that
//! no corner is more than a step above its neighbours. Where the hills go
//! depends on the shape: a ring of them for an atoll, twenty clusters for
//! a group of islands, one big and one smaller cluster for one island.
//! `MAPERSTE.DAT` sets how many, how big and how far apart, by relief
//! and land. Then the land is smoothed, the squares get their forms and
//! grounds, sand and grass their looks and some of them palms, rivers
//! run from the hills to the sea, the land is cut into plots, and the
//! location map gets the water's and the palms' effects.
//!
//! Every random number comes from [`World::random`], in the original's
//! order. The hills' outlines are the one place with floating point: see
//! [`x87`].

// The original's names: squares `(i, j)`, offsets `di`, `dj`.
#![allow(clippy::many_single_char_names)]

mod x87;

use formats::data::MapersteTable;

use crate::enhance::Fix;
use crate::startform::Settings;
use crate::terrain::FORMS;
use crate::world::{NO_BUILDING, PALM, Pos, World, on_map};
use x87::Double;

/// `MAPERSTE.DAT`'s tables, each indexed by relief (the row) and land
/// (the column). // 10b0:56ce, into 1110:71f2 to 1110:721a
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Maperste {
    /// `IGMinLeere`: a group's hills, one chance in this (plus
    /// `Random(15)`) per corner of a cluster. // *1110:71f2
    pub ig_min_leere: MapersteTable,
    /// `IGMinRadius`: a group's cluster radius, plus `Random(10)`.
    /// // *1110:71f6
    pub ig_min_radius: MapersteTable,
    /// `GIRadius`: one island's big cluster radius; the small one's is 30
    /// less. // *1110:71fa
    pub gi_radius: MapersteTable,
    /// `GILeere1`: one chance in this per corner of the big cluster.
    /// // *1110:71fe
    pub gi_leere1: MapersteTable,
    /// `GILeere2`: the same for the small cluster. // *1110:7202
    pub gi_leere2: MapersteTable,
    /// `ATAnzahl1`: an atoll's big hills per stretch of its ring.
    /// // *1110:7206
    pub at_anzahl1: MapersteTable,
    /// `ATAnzahl2`: its small hills per stretch. // *1110:720a
    pub at_anzahl2: MapersteTable,
    /// `ATGroesse1`: the big hills' size, `Random` of it plus 1.
    /// // *1110:720e
    pub at_groesse1: MapersteTable,
    /// `ATGroesse2`: the small hills' size. // *1110:7212
    pub at_groesse2: MapersteTable,
    /// `ATVariation1`: how far the big hills stray from the ring.
    /// // *1110:7216
    pub at_variation1: MapersteTable,
    /// `ATVariation2`: how far the small ones stray. // *1110:721a
    pub at_variation2: MapersteTable,
}

impl Maperste {
    /// The tables in the file's order, as [`formats::data::maperste`]
    /// returns them; `None` unless there are 11.
    #[must_use]
    pub fn new(tables: &[MapersteTable]) -> Option<Self> {
        let [a, b, c, d, e, f, g, h, i, j, k] = <[MapersteTable; 11]>::try_from(tables).ok()?;
        Some(Maperste {
            ig_min_leere: a,
            ig_min_radius: b,
            gi_radius: c,
            gi_leere1: d,
            gi_leere2: e,
            at_anzahl1: f,
            at_anzahl2: g,
            at_groesse1: h,
            at_groesse2: i,
            at_variation1: j,
            at_variation2: k,
        })
    }
}

/// The atoll's ring: twelve boxes a point each is picked in, clockwise
/// around the map, then the box of the hills inside the ring. Each is
/// `i` from, `j` from, `i` to, `j` to. // 1110:3cf4
const ATOLL_BOXES: [[i16; 4]; 13] = [
    [180, 180, 220, 220],
    [190, 125, 249, 180],
    [190, 70, 249, 125],
    [180, 30, 220, 70],
    [125, 1, 180, 60],
    [70, 1, 125, 60],
    [30, 30, 70, 70],
    [1, 70, 60, 125],
    [1, 125, 60, 180],
    [30, 180, 70, 220],
    [70, 190, 125, 249],
    [125, 190, 180, 249],
    [55, 55, 195, 195],
];

/// The group of islands' clusters. // 10b0:52d4
const GROUP_CLUSTERS: i16 = 20;

/// How far a hill reaches at most each way, and the side of its working
/// arrays (`array[-50..50, -50..50] of Integer`, `*1110:7226` and
/// `*1110:722a`). // 10b0:3904
const HILL_REACH: i16 = 50;
const HILL_SIDE: usize = 101;

/// The neighbouring corners the land follows to, in the original's
/// order. // 10b0:0002, 10b0:1162
const NEIGHBOURS: [Pos; 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// The highest a corner goes. // 10b0:0130
const TOP: u8 = 20;

/// A river's ground pieces start here (`FLUSSPOS.DAT` row 0); its two
/// source squares are `0x57 + 2 * direction` and one more.
const RIVER: u8 = 0x37;
const SOURCE: u8 = 0x57;

/// How many places a river's start is looked for in. // 10b0:4506
const RIVER_TRIES: u16 = 1500;

/// The steps one river takes at most. The original has no limit: a river
/// that wandered in circles would never end. The map's squares are
/// enough for any river that ends.
const RIVER_STEPS: usize = 251 * 251;

/// The rounds a flat river looks along at most for its way on
/// (10b0:48d7): its distance is an `Integer`, so after this many rounds
/// it has looked at every square of its row and column and starts over.
const LOOK_AHEAD: u32 = 0x1_0000;

/// The location map's value everywhere before the effects. // 10b0:3030
const LOCATION_START: u8 = 0x7f;

/// The effects on the location map (1030:1daa: radius 3): a river's
/// square, sloping water (the shore) and sand at sea level. // 10b0:517b
const RIVER_EFFECT: i32 = 0x70;
const SHORE_EFFECT: i32 = 0x6c;
const BEACH_EFFECT: i32 = 0x65;
const WATER_EFFECT_RADIUS: i32 = 3;

/// A position in the map's coordinates, moved one square in `direction`:
/// 0 `j + 1`, 1 `i + 1`, 2 `j - 1`, 3 `i - 1`. // 10b0:4560
fn step((i, j): Pos, direction: u8) -> Pos {
    match direction {
        0 => (i, j.wrapping_add(1)),
        1 => (i.wrapping_add(1), j),
        2 => (i, j.wrapping_sub(1)),
        3 => (i.wrapping_sub(1), j),
        _ => (i, j),
    }
}

/// A hill's outline and the heights under it: the locals of 10b0:3904
/// that its nested line drawing shares.
struct Hill {
    /// 1 inside the outline. // *1110:7226
    mask: Vec<i16>,
    /// The heights the hill started on. // *1110:722a
    under: Vec<i16>,
}

impl Hill {
    fn new() -> Hill {
        Hill {
            mask: vec![0; HILL_SIDE * HILL_SIDE],
            under: vec![0; HILL_SIDE * HILL_SIDE],
        }
    }

    fn index((di, dj): Pos) -> Option<usize> {
        let a = usize::try_from(di + HILL_REACH).ok()?;
        let b = usize::try_from(dj + HILL_REACH).ok()?;
        (a < HILL_SIDE && b < HILL_SIDE).then_some(a * HILL_SIDE + b)
    }

    /// The outline at the offset `d`. The original reads one row past
    /// the outline's square when a column has none of it, which decides
    /// nothing; the port reads 0 outside the arrays.
    fn mask(&self, d: Pos) -> i16 {
        Hill::index(d).map_or(0, |n| self.mask[n])
    }

    fn mark(&mut self, d: Pos) {
        if let Some(n) = Hill::index(d) {
            self.mask[n] = 1;
        }
    }

    /// A side of the outline from `(row, column)` `p` to `q`: one mark per
    /// column (or row, if it's steeper), the other coordinate stepped in
    /// `Double`s and rounded. // 10b0:375a
    fn line(&mut self, p: Pos, q: Pos) {
        let ((mut b, mut a), (mut d, mut c)) = (p, q);
        if (c - a).abs() > (d - b).abs() {
            if a > c {
                // 10b0:371e swaps the two points.
                (a, b, c, d) = (c, d, a, b);
            }
            let slope = Double::quotient(i32::from(d - b), i32::from(c - a)); // 1108:055f
            let mut x = Double::from_int(i32::from(b));
            self.mark((b, a));
            for column in a + 1..=c {
                x = x.add(slope);
                self.mark((low_word(x.round()), column)); // 1108:0ea0
            }
        } else {
            if b > d {
                (a, b, c, d) = (c, d, a, b);
            }
            let slope = Double::quotient(i32::from(c - a), i32::from(d - b));
            let mut x = Double::from_int(i32::from(a));
            self.mark((b, a));
            for row in b + 1..=d {
                x = x.add(slope);
                self.mark((row, low_word(x.round())));
            }
        }
    }
}

/// The low word of a `Longint`, as the original uses `Round`'s result.
fn low_word(n: i32) -> i16 {
    i16::from_le_bytes([n.to_le_bytes()[0], n.to_le_bytes()[1]])
}

/// The 41 × 41 working copy the generator's own raising and lowering
/// (10b0:0e0c, 10b0:2053) move a corner on: its heights and buildings,
/// the error and the bounds of the corners moved.
struct Copy {
    centre: Pos,
    heights: Vec<u8>,
    buildings: Vec<u8>,
    error: u16,
    low: (u8, u8),
    high: (u8, u8),
}

/// How far the copy reaches from the corner, each way, and its side.
const COPY_REACH: i16 = 20;
const COPY_SIDE: usize = 41;

impl Copy {
    /// The copy around `centre`: off the map height 0 and no building.
    fn new(w: &World, centre: Pos) -> Copy {
        let mut heights = vec![0; COPY_SIDE * COPY_SIDE];
        let mut buildings = vec![NO_BUILDING; COPY_SIDE * COPY_SIDE];
        let (i, j) = centre;
        for di in -COPY_REACH..=COPY_REACH {
            for dj in -COPY_REACH..=COPY_REACH {
                let pos = (i + di, j + dj);
                if let Some(n) = Copy::index((di, dj))
                    && on_map(pos)
                {
                    heights[n] = w.island.heights.at(pos);
                    buildings[n] = w.island.buildings.at(pos);
                }
            }
        }
        let byte = |v: i16| v.to_le_bytes()[0];
        Copy {
            centre,
            heights,
            buildings,
            error: 0,
            low: (byte(i), byte(j)),
            high: (byte(i), byte(j)),
        }
    }

    /// The copy's index for the offset `d`. The land can't spread
    /// further than a corner's height, below 20, so it never leaves the
    /// copy; the port checks anyway.
    fn index((di, dj): Pos) -> Option<usize> {
        let a = usize::try_from(di + COPY_REACH).ok()?;
        let b = usize::try_from(dj + COPY_REACH).ok()?;
        (a < COPY_SIDE && b < COPY_SIDE).then_some(a * COPY_SIDE + b)
    }

    fn height(&self, d: Pos) -> Option<i16> {
        Copy::index(d).map(|n| i16::from(self.heights[n]))
    }

    /// A palm on the square at `d` goes, for nothing.
    fn clear_palm(&mut self, d: Pos) {
        if let Some(n) = Copy::index(d)
            && self.buildings[n] == PALM
        {
            self.buildings[n] = NO_BUILDING;
        }
    }

    /// The corner at `d` moved: the bounds take it in, as bytes.
    fn spread(&mut self, (di, dj): Pos) {
        let (a, b) = (i32::from(self.centre.0 + di), i32::from(self.centre.1 + dj));
        let byte = |v: i32| v.to_le_bytes()[0];
        self.low = (byte(a.min(i32::from(self.low.0))), byte(b.min(i32::from(self.low.1))));
        self.high = (byte(a.max(i32::from(self.high.0))), byte(b.max(i32::from(self.high.1))));
    }
}

/// The squares around a corner, as offsets, in the order they're checked.
const SQUARES: [Pos; 4] = [(0, 0), (1, 0), (0, 1), (1, 1)];

/// The generator's state: the world it builds the island in and the
/// settings' rows of `MAPERSTE.DAT`.
struct Generator<'a> {
    w: &'a mut World,
    t: &'a Maperste,
    relief: i16,
    land: i16,
}

impl World {
    /// Makes a new island from the start menu's `settings` with
    /// `MAPERSTE.DAT`'s `tables`: every layer, the plot map and its
    /// number of plots ([`World::plots`] isn't made here, see
    /// [`World::create_plots`]) and the location map. The tables must be
    /// loaded: the palms and grounds read them. // 10b0:52d4
    ///
    /// The original also shows the progress window's steps and, for a
    /// game that's loaded rather than made (`hintergrund+0x190` or
    /// `+0x191`), only clears the map.
    pub fn generate_island(&mut self, settings: &Settings, tables: &Maperste) -> u16 {
        let mut g = Generator {
            w: self,
            t: tables,
            relief: settings.relief,
            land: settings.land,
        };
        g.clear(); // 10b0:3030
        match settings.island {
            0 => g.atoll(),               // 10b0:3c5c
            1 => g.group(GROUP_CLUSTERS), // 10b0:41b5
            2 => g.one_island(),          // 10b0:412c
            _ => {}
        }
        g.smooth(); // 10b0:4fd1
        g.shape_squares(); // 10b0:354c
        g.decorate(); // 10b0:4dae
        g.rivers(settings.rivers); // 10b0:4d79
        let plots = g.number_plots(); // 10b0:29c0
        g.location(); // 10b0:52a4
        plots
    }
}

impl Generator<'_> {
    /// The settings' value in `table`. The scroll bars keep relief and
    /// land within 0..=4; the port reads 0 outside.
    fn at(&self, table: &MapersteTable) -> i16 {
        let row = usize::try_from(self.relief).ok().and_then(|r| table.get(r));
        row.and_then(|r| usize::try_from(self.land).ok().and_then(|l| r.get(l)))
            .copied()
            .unwrap_or(0)
    }

    fn random(&mut self, range: i16) -> i16 {
        self.w.random.random(range.cast_unsigned()).cast_signed()
    }

    fn height(&self, pos: Pos) -> u8 {
        self.w.island.heights.get(pos).unwrap_or(0)
    }

    /// The sea everywhere: height 0, flat water, nothing on it, no plots,
    /// the location map at 127. // 10b0:3030
    fn clear(&mut self) {
        let island = &mut self.w.island;
        for i in 0..=250 {
            for j in 0..=250 {
                let pos = (i, j);
                island.heights.set(pos, 0);
                island.forms.set(pos, 16);
                island.buildings.set(pos, NO_BUILDING);
                island.ground.set(pos, 2);
                island.parts.set(pos, 0);
                island.owners.set(pos, 0);
                island.occupancy.set(pos, 0);
                island.plot_map.set(pos, 0);
            }
        }
        for a in 0..=0x7d {
            for b in 0..=0x7d {
                island.location.set((a, b), LOCATION_START);
            }
        }
    }

    /// Raises the corner `(i, j)` a step, and its neighbours that are then
    /// two steps lower, without any checks. // 10b0:0002
    fn lift(&mut self, (i, j): Pos) {
        let heights = &mut self.w.island.heights;
        let Some(h) = heights.get((i, j)) else {
            return;
        };
        heights.set((i, j), h.wrapping_add(1));
        for (di, dj) in NEIGHBOURS {
            let next = (i + di, j + dj);
            let (Some(here), Some(there)) = (self.w.island.heights.get((i, j)), self.w.island.heights.get(next)) else {
                continue;
            };
            if i16::from(here) - i16::from(there) == 2 {
                self.lift(next);
            }
        }
    }

    /// Raises the corner `(i, j)` a step if it's below 20 and its cone
    /// stays on the map: whether it did. // 10b0:0130
    fn try_raise(&mut self, (i, j): Pos) -> bool {
        let h = i16::from(self.height((i, j)));
        if h < i16::from(TOP) && h <= 248 - i && h <= j - 2 && h <= i - 2 && h <= 248 - j {
            self.lift((i, j));
            true
        } else {
            false
        }
    }

    /// Lowers the corner `(i, j)` a step, and its neighbours that are then
    /// two steps higher. // 10b0:1162
    fn sink(&mut self, (i, j): Pos) {
        let heights = &mut self.w.island.heights;
        let Some(h) = heights.get((i, j)) else {
            return;
        };
        heights.set((i, j), h.wrapping_sub(1));
        for (di, dj) in NEIGHBOURS {
            let next = (i + di, j + dj);
            let (Some(here), Some(there)) = (self.w.island.heights.get((i, j)), self.w.island.heights.get(next)) else {
                continue;
            };
            if i16::from(there) - i16::from(here) == 2 {
                self.sink(next);
            }
        }
    }

    /// Lowers the corner `(i, j)` a step unless it's at sea level.
    /// // 10b0:1290
    fn try_lower(&mut self, pos: Pos) {
        if self.height(pos) != 0 {
            self.sink(pos);
        }
    }

    /// A hill on the corner `(i, j)` reaching `size` (at most 50) each
    /// way, if that stays on the map: four random corners, one in each
    /// quarter, make its outline, and every corner inside is raised to a
    /// step above where it was when the hill began. Eight `Random(size)`
    /// for the corners. // 10b0:3904
    fn hill(&mut self, (i, j): Pos, size: i16) {
        let r = size.min(HILL_REACH);
        if !(r < i && r < j && i < 250 - r && j < 250 - r) {
            return;
        }
        let mut hill = Hill::new();
        for di in -r..=r {
            for dj in -r..=r {
                if let Some(n) = Hill::index((di, dj)) {
                    hill.mask[n] = 0;
                    hill.under[n] = i16::from(self.height((i + di, j + dj)));
                }
            }
        }
        let mut v = [0i16; 8];
        for x in &mut v {
            *x = self.random(r).wrapping_add(1);
        }
        // The corners, (row, column) offsets.
        let top_left = (-v[0], -v[1]);
        let bottom_left = (v[2], -v[3]);
        let top_right = (-v[4], v[5]);
        let bottom_right = (v[6], v[7]);
        hill.line(top_left, top_right);
        hill.line(bottom_left, bottom_right);
        hill.line(top_left, bottom_left);
        hill.line(top_right, bottom_right);
        // Fill each column between its first and last mark. A column whose
        // first mark is in the bottom row stays as it is.
        for dj in -r..=r {
            let mut di = -r;
            while hill.mask((di, dj)) == 0 && di <= r {
                di += 1;
            }
            if di >= r {
                continue;
            }
            let first = di;
            di = r;
            while hill.mask((di, dj)) == 0 && di > first {
                di -= 1;
            }
            for d in first..=di {
                hill.mark((d, dj));
            }
        }
        for di in -r..=r {
            for dj in -r..=r {
                let Some(n) = Hill::index((di, dj)) else {
                    continue;
                };
                let pos = (i + di, j + dj);
                loop {
                    let h = self.height(pos);
                    if hill.mask[n] + hill.under[n] <= i16::from(h) || h >= TOP || !self.try_raise(pos) {
                        break;
                    }
                }
            }
        }
    }

    /// A cluster of hills in the circle of `radius` around `(i0, j0)`:
    /// each corner inside has one chance in `chance` (`Random`) of a hill
    /// of size `Random(min(largest, 39) - 4) + 4`. // 10b0:404c
    fn cluster(&mut self, (i0, j0): Pos, radius: i16, chance: i16, largest: i16) {
        let limit = i32::from(radius) * i32::from(radius); // 1030:0f8d
        let largest = largest.min(0x27);
        for i in i0 - radius..=i0 + radius {
            for j in j0 - radius..=j0 + radius {
                let (di, dj) = (i32::from(i - i0), i32::from(j - j0));
                if dj * dj + di * di <= limit && self.random(chance) == 0 {
                    let size = self.random(largest - 4) + 4;
                    if (1..=0x31).contains(&size) {
                        self.hill((i, j), size);
                    }
                }
            }
        }
    }

    /// A group of islands: `n` clusters around random corners, their
    /// radius `IGMinRadius + Random(10)`, one chance in `IGMinLeere +
    /// Random(15)` of a hill of size 4 to 14. // 10b0:41b5
    fn group(&mut self, n: i16) {
        for _ in 0..n {
            let i = self.random(200) + 25;
            let j = self.random(200) + 25;
            let radius = self.at(&self.t.ig_min_radius) + self.random(10);
            let chance = self.at(&self.t.ig_min_leere) + self.random(15);
            self.cluster((i, j), radius, chance, 0xf);
        }
    }

    /// One island: a big cluster in the middle (`GIRadius`, `GILeere1`,
    /// hills up to 34), and a smaller one (30 less, `GILeere2`, up to 19)
    /// around a random corner of the south east. // 10b0:412c
    fn one_island(&mut self) {
        let radius = self.at(&self.t.gi_radius);
        let chance = self.at(&self.t.gi_leere1);
        self.cluster((0x7d, 0x7d), radius, chance, 0x23);
        let i = self.random(0x32) + 100;
        let j = self.random(0x32);
        let chance = self.at(&self.t.gi_leere2);
        self.cluster((i, j + 100), radius - 0x1e, chance, 0x14);
    }

    /// An atoll: a ring through a random point in each of twelve boxes,
    /// hills strewn along each stretch (`ATAnzahl1` big ones and
    /// `ATAnzahl2` small ones, `ATVariation` off the line), then 20 to
    /// 49 little hills in the lagoon. // 10b0:3c5c
    fn atoll(&mut self) {
        let mut ring = [(0i16, 0i16); 13];
        for (point, b) in ring.iter_mut().zip(ATOLL_BOXES) {
            let i = b[0].wrapping_add(self.random(b[2] - b[0]));
            let j = b[1].wrapping_add(self.random(b[3] - b[1]));
            *point = (i, j);
        }
        ring[12] = ring[0];
        for n in 0..12 {
            let (from, to) = (ring[n], ring[n + 1]);
            let big = self.at(&self.t.at_anzahl1);
            for _ in 0..big.max(0) {
                let variation = self.at(&self.t.at_variation1);
                let size = self.at(&self.t.at_groesse1);
                self.ring_hill(from, to, variation, variation, size);
            }
            let small = self.at(&self.t.at_anzahl2);
            for _ in 0..small.max(0) {
                // BUG(original): the small hills stray by ATVariation2 but are centred by half of ATVariation1.
                let half_of = self.at(&self.t.at_variation1);
                let variation = self.at(&self.t.at_variation2);
                let half_of = if self.w.fixes.has(Fix::AtollHills) {
                    variation
                } else {
                    half_of
                };
                let size = self.at(&self.t.at_groesse2);
                self.ring_hill(from, to, half_of, variation, size);
            }
        }
        let lagoon = ATOLL_BOXES[12];
        let n = self.random(0x1e) + 0x14;
        for _ in 0..n {
            let along = |g: &mut Generator<'_>, from: i16, to: i16| {
                let t = i32::from(g.random(0x65));
                low_word(i32::from(from) + t * i32::from(to - from) / 100)
            };
            let i = along(self, lagoon[0], lagoon[2]);
            let j = along(self, lagoon[1], lagoon[3]);
            let size = self.random(3) + 2;
            self.hill((i, j), size);
        }
    }

    /// A hill at a random point (`Random(101)` percent) of the stretch
    /// from `from` to `to`, `Random(variation) - half_of div 2` off it
    /// each way, of size `Random(size) + 1`. // 10b0:3c5c
    fn ring_hill(&mut self, from: Pos, to: Pos, half_of: i16, variation: i16, size: i16) {
        let t = self.random(0x65);
        let along = |g: &mut Generator<'_>, a: i16, b: i16| {
            let half = i32::from(half_of / 2);
            let off = i32::from(g.random(variation));
            // `MUL` keeps the low word of the product; `IDIV` truncates.
            let part = b.wrapping_sub(a).wrapping_mul(t) / 100;
            low_word(i32::from(part.wrapping_add(a)) + off - half)
        };
        let i = along(self, from.0, to.0);
        let j = along(self, from.1, to.1);
        let size = self.random(size) + 1;
        self.hill((i, j), size);
    }

    /// Takes off peaks and fills in pits: a corner more than two steps
    /// above the sum of its four neighbours' (four times its height
    /// against theirs) is lowered, one more than two below raised; once
    /// forwards, then backwards. // 10b0:4fd1
    fn smooth(&mut self) {
        let forwards = (1..=249).flat_map(|i| (1..=249).map(move |j| (i, j)));
        let backwards = (1..=249).rev().flat_map(|i| (1..=249).rev().map(move |j| (i, j)));
        for pos in forwards.chain(backwards) {
            let (i, j) = pos;
            let h = |p| i16::from(self.height(p));
            let laplace = h(pos) * 4 - h((i - 1, j)) - h((i, j - 1)) - h((i + 1, j)) - h((i, j + 1));
            if laplace >= 3 {
                self.try_lower(pos); // 10b0:1290
            } else if laplace < -2 {
                self.try_raise(pos); // 10b0:0130
            }
        }
    }

    /// Every square within the edge gets its form, and its ground from
    /// its corners (10b0:3199); a square with no corner above the sea is
    /// flat water. `j` outside, `i` inside. // 10b0:354c
    fn shape_squares(&mut self) {
        for j in 1..=249 {
            for i in 1..=249 {
                let pos = (i, j);
                if self.w.island.highest_corner(pos) == 0 {
                    self.w.island.forms.set(pos, 16);
                    self.w.island.ground.set(pos, 2);
                    continue;
                }
                let h = |p| i16::from(self.height(p));
                let here = h(pos);
                let corners = [h((i, j - 1)), h((i - 1, j - 1)), h((i - 1, j))];
                let form = FORMS
                    .iter()
                    .position(|f| (0..3).all(|k| here + f[k] == corners[k]))
                    .unwrap_or(FORMS.len());
                self.w.island.forms.set(pos, u8::try_from(form).unwrap_or(19));
                self.w.reset_ground(pos); // 10b0:3199
            }
        }
    }

    /// Sand and grass get their looks (flat: `Random(6)`, half of them
    /// the first; sloping: `Random(2)`), and any empty square of sand,
    /// grass or sand to grass a palm with `Random(100) <` 20 (`1110:0094`)
    /// in one of `HAUSNR.DAT`'s looks for it. `j` outside, `i` inside.
    /// // 10b0:4dae
    fn decorate(&mut self) {
        for j in 1..=249 {
            for i in 1..=249 {
                let pos = (i, j);
                let ground = self.w.island.ground.at(pos);
                if !matches!(ground, 0 | 1 | 4) {
                    continue;
                }
                if ground != 4 {
                    let part = if self.w.island.forms.at(pos) == 16 {
                        match self.w.random.random(6) {
                            0..=2 => 0,
                            3 => 1,
                            4 => 2,
                            _ => 3,
                        }
                    } else {
                        self.w.random.random(2).to_le_bytes()[0]
                    };
                    self.w.island.parts.set(pos, part);
                }
                if self.w.island.buildings.at(pos) == NO_BUILDING && self.w.random.random(100) < PALM_CHANCE {
                    self.w.island.buildings.set(pos, PALM);
                    let looks = self.w.tables.hausnr(PALM, 0);
                    let stage = self.random(looks).to_le_bytes()[0];
                    let stages = self.w.tables.hausnr(PALM, 6).to_le_bytes()[0];
                    let variant = self.w.island.parts.at(pos);
                    self.w
                        .island
                        .parts
                        .set(pos, variant.wrapping_mul(stages).wrapping_add(stage));
                }
            }
        }
    }
}

/// `1110:0094`: the chance in percent of a palm on a square of a new
/// island (20, from `init_game_defaults`, 1000:1ea0).
const PALM_CHANCE: u16 = 0x14;

/// Rivers (segment `10b0`, 10b0:4d19 and its callees).
impl Generator<'_> {
    /// `n` rivers. // 10b0:4d79
    fn rivers(&mut self, n: i16) {
        for _ in 0..n.max(0) {
            self.river(); // 10b0:4d19
        }
    }

    /// A river: from a random square on the hills (its lowest corner at
    /// least 4 up) in a random direction downhill, levelling the land
    /// under it, until it reaches the sea. One that runs into another
    /// river or can't be levelled is taken out again. // 10b0:4d19
    fn river(&mut self) {
        let Some(mut pos) = self.river_start() else {
            return;
        }; // 10b0:4506
        let mut direction = 0;
        if self.river_source(&mut direction, &mut pos) {
            return;
        }
        let mut failed;
        let mut steps = 0;
        loop {
            failed = self.river_step(&mut direction, &mut pos) != 0; // 10b0:4a7d
            if self.w.island.highest_corner(pos) == 0 || failed {
                break;
            }
            steps += 1;
            if steps == RIVER_STEPS {
                // BUG(original): a river that wanders in circles never ends; the port gives up and takes it out.
                failed = true;
                break;
            }
        }
        if failed {
            self.river_erase(direction, pos); // 10b0:4cb1
        }
    }

    /// Where a river starts: a random square (`Random(249) + 1` for `i`,
    /// then `j`) whose lowest corner is at least 4 up, tried 1500 times;
    /// the original takes the last try whatever it is, `fix.river_start`
    /// none. // 10b0:4506
    fn river_start(&mut self) -> Option<Pos> {
        let mut tries = 0;
        loop {
            tries += 1;
            let pos = (self.random(0xf9) + 1, self.random(0xf9) + 1);
            if self.w.island.lowest_corner(pos) >= 4 {
                return Some(pos);
            }
            if tries >= RIVER_TRIES {
                // BUG(original): the 1500th try is taken without looking at its corners, so a river always starts.
                return (!self.w.fixes.has(Fix::RiverStart)).then_some(pos);
            }
        }
    }

    /// The river's source: two squares in the direction `Random(4)`,
    /// levelled to the lower of their lowest corners, if that's at least
    /// 3 and the levelling works. Whether that failed; otherwise `pos` is
    /// the square after them. // 10b0:4618
    fn river_source(&mut self, direction: &mut u8, pos: &mut Pos) -> bool {
        *direction = self.w.random.random(4).to_le_bytes()[0];
        let next = step(*pos, *direction);
        let blocked = self.source_blocked(*pos, next); // 10b0:459a
        if !blocked {
            let island = &mut self.w.island;
            if on_map(*pos) && on_map(next) {
                island.buildings.set(*pos, NO_BUILDING);
                island.buildings.set(next, NO_BUILDING);
                island.ground.set(*pos, SOURCE + *direction * 2);
                island.ground.set(next, SOURCE + *direction * 2 + 1);
            }
            *pos = step(next, *direction);
        }
        blocked
    }

    /// Whether the source can't go on `pos` and `next`: their lowest
    /// corners' lower is below 3, or levelling either square to it fails.
    /// // 10b0:459a
    fn source_blocked(&mut self, pos: Pos, next: Pos) -> bool {
        let low = self.w.island.lowest_corner(pos).min(self.w.island.lowest_corner(next));
        if low < 3 {
            return true;
        }
        let mut error = self.level_square(i16::from(low), pos); // 10b0:2e03
        if error == 0 {
            error = self.level_square(i16::from(low), next);
        }
        error != 0
    }

    /// One step of the river at `pos`, going `direction`: the square is
    /// levelled to the height of the corner it comes from if that isn't
    /// its top (then the river picks its way on along the flat,
    /// 10b0:48d7), or sloped a step down the way it goes. Unless that
    /// fails or the square already has two rivers beside it, it gets its
    /// river piece and the river moves on. 0 when it did. // 10b0:4a7d
    fn river_step(&mut self, direction: &mut u8, pos: &mut Pos) -> u16 {
        let (i, j) = *pos;
        let from = match *direction {
            0 | 1 => self.height((i - 1, j - 1)),
            _ => self.height((i, j)),
        };
        let top = self.w.island.highest_corner(*pos);
        let target = if from < top { from } else { from.saturating_sub(1) };
        let mut way = *direction;
        let error = if target == from {
            let error = self.level_square(i16::from(from), *pos); // 10b0:2e03
            if error == 0 {
                way = self.flat_way(*direction, *pos); // 10b0:48d7
            }
            error
        } else {
            self.slope_square(*direction, from, *pos) // 10b0:2e6f
        };
        if error != 0 {
            return error;
        }
        if self.rivers_beside(*pos) >= 2 {
            return 1;
        }
        self.river_piece(way, *direction, *pos); // 10b0:424f
        *pos = step(*pos, way);
        *direction = way;
        0
    }

    /// Takes a failed river out: from the square before `pos` along the
    /// river squares beside it (`j - 1`, `i - 1`, `j + 1`, `i + 1` first),
    /// each gets its ground back. // 10b0:4cb1
    fn river_erase(&mut self, direction: u8, pos: Pos) {
        let back = (direction + 2) % 4; // 10b0:4bea
        let mut pos = step(pos, back);
        if self.w.island.ground.get(pos).unwrap_or(0) >= RIVER {
            self.w.reset_ground(pos); // 10b0:3199
        }
        // 10b0:4c06
        while let Some(next) = [(0, -1), (-1, 0), (0, 1), (1, 0)]
            .into_iter()
            .map(|(di, dj)| (pos.0 + di, pos.1 + dj))
            .find(|&p| self.w.island.ground.get(p).unwrap_or(0) >= RIVER)
        {
            pos = next;
            let before = self.w.island.ground.at(pos);
            self.w.reset_ground(pos);
            if self.w.island.ground.at(pos) == before {
                // BUG(original): a river square reset_ground leaves alone (one with a building) is found again for ever; rivers have none, and the port stops.
                break;
            }
        }
    }

    /// How many of the squares beside `(i, j)` are rivers. // 10b0:47ff
    fn rivers_beside(&self, (i, j): Pos) -> u8 {
        let river = |p| u8::from(self.w.island.ground.get(p).unwrap_or(0) >= RIVER);
        river((i - 1, j)) + river((i, j - 1)) + river((i + 1, j)) + river((i, j + 1))
    }

    /// The way a river on flat land goes on, not back where it came from:
    /// looking further out each round along the three other ways, the
    /// ways whose square there is higher (by the sum of its corners) than
    /// the lowest drop out, until one is left; a square off the map or
    /// beside a river counts as highest. Of ways still open the last of
    /// `i - 1`, `j - 1`, `j + 1`, `i + 1` wins. // 10b0:48d7
    ///
    /// Ways that tie up to the map's edge (over the sea, say) stay open
    /// past it, where all count as highest; the distance is an `Integer`
    /// that wraps round, so they come back onto the map from the other
    /// side some 65000 rounds later, and the first to come back wins.
    fn flat_way(&self, direction: u8, (i, j): Pos) -> u8 {
        const NONE: u16 = 0xffff;
        // The ways in the original's order of its locals, by the way they
        // go: 2 (`j - 1`), 3 (`i - 1`), 0 (`j + 1`), 1 (`i + 1`). The way
        // back is closed.
        const WAYS: [u8; 4] = [2, 3, 0, 1];
        let back = (direction + 2) % 4;
        let mut open = WAYS.map(|w| w != back);
        let mut value = [NONE; 4];
        // BUG(original): the distance is an `Integer` that wraps, so ways that tie to the map's edge look on round the other side.
        let mut k: i16 = 1;
        let mut rounds = 0;
        loop {
            for (n, &w) in WAYS.iter().enumerate() {
                if open[n] {
                    let square = match w {
                        2 => (i, j.wrapping_sub(k)),
                        3 => (i.wrapping_sub(k), j),
                        0 => (i, j.wrapping_add(k)),
                        _ => (i.wrapping_add(k), j),
                    };
                    value[n] = self.square_sum(square); // 10b0:4885
                }
            }
            let lowest = value[3].min(value[2]).min(value[1]).min(value[0]);
            for n in 0..4 {
                if lowest < value[n] {
                    open[n] = false;
                    value[n] = NONE;
                }
            }
            k = k.wrapping_add(1);
            if open.iter().filter(|&&o| o).count() <= 1 {
                break;
            }
            // BUG(original): the distance is an `Integer` that wraps, so ways that tie to the map's edge look on round the other side.
            if k > 250 && self.w.fixes.has(Fix::RiverFlatWay) {
                // Every way still open is off the map: the last wins.
                break;
            }
            rounds += 1;
            if rounds == LOOK_AHEAD {
                // BUG(original): ways that tie all the way round their row and column are looked along for ever; the port stops after one round.
                break;
            }
        }
        let mut way = direction;
        for (n, w) in [(1, 3), (0, 2), (2, 0), (3, 1)] {
            if open[n] {
                way = w;
            }
        }
        way
    }

    /// The sum of the square `(i, j)`'s corners, or `0xffff` off
    /// 1..=249 or beside a river. // 10b0:4885, 10b0:4786, 10b0:470c
    fn square_sum(&self, (i, j): Pos) -> u16 {
        if !(1..=249).contains(&i) || !(1..=249).contains(&j) {
            return 0xffff;
        }
        let river = |p| self.w.island.ground.get(p).unwrap_or(0) >= RIVER;
        if river((i - 1, j)) || river((i + 1, j)) || river((i, j - 1)) || river((i, j + 1)) {
            return 0xffff;
        }
        let h = |p| u16::from(self.height(p));
        h((i - 1, j - 1)) + h((i, j)) + h((i, j - 1)) + h((i - 1, j))
    }

    /// The river piece for a square entered going `from` and left going
    /// `to`, by its form and ground (0 sand, 1 grass, 4 sand to grass, 2
    /// water); none for any other. It takes the square's building.
    /// // 10b0:424f
    fn river_piece(&mut self, to: u8, from: u8, pos: Pos) {
        let island = &mut self.w.island;
        let (Some(ground), Some(form)) = (island.ground.get(pos), island.forms.get(pos)) else {
            return;
        };
        // Straight on: on the flat by sand or grass, down a slope of
        // `form` by each ground.
        let straight = |flat: [u8; 2], down: u8, slope: [u8; 4]| match (form, ground) {
            (16, 0) => Some(flat[0]),
            (16, 1) => Some(flat[1]),
            (f, 0) if f == down => Some(slope[0]),
            (f, 1) if f == down => Some(slope[1]),
            (f, 4) if f == down => Some(slope[2]),
            (f, 2) if f == down => Some(slope[3]),
            _ => None,
        };
        // A bend, whatever the form: by sand or grass.
        let bend = |sand: u8| match ground {
            0 => Some(sand),
            1 => Some(sand + 1),
            _ => None,
        };
        let piece = match (from, to) {
            (0, 0) => straight([4, 5], 3, [0x19, 0x1a, 0x1b, 0x1f]),
            (1, 1) => straight([6, 7], 0, [0x10, 0x11, 0x12, 0x1c]),
            (2, 2) => straight([4, 5], 1, [0x13, 0x14, 0x15, 0x1d]),
            (3, 3) => straight([6, 7], 2, [0x16, 0x17, 0x18, 0x1e]),
            (0, 1) | (3, 2) => bend(0xc),
            (0, 3) | (1, 2) => bend(0xa),
            (1, 0) | (2, 3) => bend(8),
            (2, 1) | (3, 0) => bend(0xe),
            _ => None,
        };
        // BUG(original): other grounds (sand to grass on the flat, water in
        // a bend) and slopes that don't fall the way the river goes get no
        // piece: the square stays as it is, a gap in the river.
        if let Some(p) = piece {
            island.buildings.set(pos, NO_BUILDING);
            island.ground.set(pos, p + RIVER);
        }
    }

    /// Levels the square `(i, j)` to `height`: its corners `(i, j)`,
    /// `(i - 1, j)`, `(i, j - 1)` and `(i - 1, j - 1)` in turn, until one
    /// fails. The error, 0 when done. // 10b0:2e03
    fn level_square(&mut self, height: i16, (i, j): Pos) -> u16 {
        let mut error = 0;
        for corner in [(i, j), (i - 1, j), (i, j - 1), (i - 1, j - 1)] {
            if error == 0 {
                error = self.set_corner(height, corner); // 10b0:2d70
            }
        }
        error
    }

    /// Slopes the square `(i, j)` a step down in `direction` from
    /// `height`: the corners on that side to `height - 1`, the others to
    /// `height`, until one fails. // 10b0:2e6f
    fn slope_square(&mut self, direction: u8, height: u8, (i, j): Pos) -> u16 {
        let (h, low) = (i16::from(height), i16::from(height) - 1);
        let corners = match direction {
            0 => [(low, (i, j)), (low, (i - 1, j)), (h, (i, j - 1)), (h, (i - 1, j - 1))],
            1 => [(low, (i, j)), (h, (i - 1, j)), (low, (i, j - 1)), (h, (i - 1, j - 1))],
            2 => [(h, (i, j)), (h, (i - 1, j)), (low, (i, j - 1)), (low, (i - 1, j - 1))],
            3 => [(h, (i, j)), (low, (i - 1, j)), (h, (i, j - 1)), (low, (i - 1, j - 1))],
            _ => return 0,
        };
        let mut error = 0;
        for (height, corner) in corners {
            if error == 0 {
                error = self.set_corner(height, corner); // 10b0:2d70
            }
        }
        error
    }

    /// Raises or lowers the corner `(i, j)` step by step to `height`
    /// until a step fails: the error, 0x16 for a corner not within
    /// 1..=249. // 10b0:2d70
    fn set_corner(&mut self, height: i16, pos: Pos) -> u16 {
        let (i, j) = pos;
        if !(1..=249).contains(&i) || !(1..=249).contains(&j) {
            return 0x16;
        }
        let mut error = 0;
        while i16::from(self.height(pos)) < height && error == 0 {
            error = self.raise_checked(pos); // 10b0:0e0c
        }
        while height < i16::from(self.height(pos)) && error == 0 {
            error = self.lower_checked(pos); // 10b0:2053
        }
        error
    }

    /// Raises the corner `(i, j)` a step on a working copy, the land
    /// following, unless a river is on a square it touches (palms go for
    /// nothing): 0, 0xd off the map, 4 at 20, 5 to 8 too close to the
    /// edge for its height, 0x10 a river. Unlike the player's tool
    /// (10b0:069f) it writes the copy back even after an error. // 10b0:0e0c
    fn raise_checked(&mut self, pos: Pos) -> u16 {
        let (i, j) = pos;
        if !on_map(pos) {
            return 0xd;
        }
        let h = i16::from(self.height(pos));
        if h >= i16::from(TOP) {
            4
        } else if h > 248 - i {
            5
        } else if h > j - 2 {
            6
        } else if h > i - 2 {
            7
        } else if h > 248 - j {
            8
        } else {
            let mut copy = Copy::new(self.w, pos);
            self.raise_step(&mut copy, (0, 0)); // 10b0:0ba3
            // BUG(original): the copy is written back after an error too, with the corners raised before it.
            if copy.error == 0 || !self.w.fixes.has(Fix::GeneratorLevelling) {
                self.write_back(&copy);
            }
            copy.error
        }
    }

    /// Raises the copy's corner at `d` unless a river is on one of its
    /// four squares, then the neighbours two steps lower. // 10b0:0ba3
    fn raise_step(&self, copy: &mut Copy, (di, dj): Pos) {
        for (si, sj) in SQUARES {
            if copy.error == 0 {
                copy.error = self.check_square(copy, (di + si, dj + sj)); // 10b0:0b04
            }
        }
        if copy.error != 0 {
            return;
        }
        let Some(n) = Copy::index((di, dj)) else {
            return;
        };
        copy.heights[n] = copy.heights[n].wrapping_add(1);
        copy.spread((di, dj));
        // The original also lists the corners risen from the sea
        // (10b0:0ace), and never reads the list.
        for (ni, nj) in NEIGHBOURS {
            let next = (di + ni, dj + nj);
            if let (Some(h), Some(g)) = (copy.height((di, dj)), copy.height(next))
                && h - g == 2
            {
                self.raise_step(copy, next);
            }
        }
    }

    /// A square of the copy the land rises under: its palm goes; 0x10 if
    /// it's a river (ground 0x37 to 0x5e), else 0. // 10b0:0b04
    fn check_square(&self, copy: &mut Copy, d: Pos) -> u16 {
        copy.clear_palm(d);
        let pos = (copy.centre.0 + d.0, copy.centre.1 + d.1);
        let ground = self.w.island.ground.get(pos).unwrap_or(0);
        if (RIVER..0x5f).contains(&ground) { 0x10 } else { 0 }
    }

    /// Lowers the corner `(i, j)` a step on a working copy, the land
    /// following, unless a river is on a square it touches (palms go):
    /// 0, or 3 off the map or at sea level, 0x10 a river. Written back
    /// even after an error. // 10b0:2053
    fn lower_checked(&mut self, pos: Pos) -> u16 {
        if !on_map(pos) || self.height(pos) == 0 {
            return 3;
        }
        let mut copy = Copy::new(self.w, pos);
        self.lower_step(&mut copy, (0, 0)); // 10b0:1c54
        // BUG(original): written back after an error too, with the corners lowered before it.
        if copy.error == 0 || !self.w.fixes.has(Fix::GeneratorLevelling) {
            self.write_back(&copy);
        }
        copy.error
    }

    /// Lowers the copy's corner at `d`: the palms on its four squares go
    /// whatever happens, then unless a river is on one, the corner sinks
    /// and the neighbours two steps higher follow. // 10b0:1c54
    fn lower_step(&self, copy: &mut Copy, (di, dj): Pos) {
        for (si, sj) in SQUARES {
            copy.clear_palm((di + si, dj + sj));
        }
        for (si, sj) in SQUARES {
            let pos = (copy.centre.0 + di + si, copy.centre.1 + dj + sj);
            let ground = self.w.island.ground.get(pos).unwrap_or(0);
            if copy.error == 0 && (RIVER..0x5f).contains(&ground) {
                copy.error = 0x10;
            }
        }
        if copy.error != 0 {
            return;
        }
        let Some(n) = Copy::index((di, dj)) else {
            return;
        };
        copy.heights[n] = copy.heights[n].wrapping_sub(1);
        copy.spread((di, dj));
        for (ni, nj) in NEIGHBOURS {
            let next = (di + ni, dj + nj);
            if let (Some(h), Some(g)) = (copy.height((di, dj)), copy.height(next))
                && g - h == 2
            {
                self.lower_step(copy, next);
            }
        }
    }

    /// Puts the copy's heights and buildings back, and works out the
    /// forms and grounds of the squares the moved corners touch.
    /// // 10b0:0e0c, 10b0:2053
    fn write_back(&mut self, copy: &Copy) {
        let (i, j) = copy.centre;
        for a in (i - COPY_REACH).max(0)..=(i + COPY_REACH).min(250) {
            for b in (j - COPY_REACH).max(0)..=(j + COPY_REACH).min(250) {
                if let Some(n) = Copy::index((a - i, b - j)) {
                    self.w.island.heights.set((a, b), copy.heights[n]);
                    self.w.island.buildings.set((a, b), copy.buildings[n]);
                }
            }
        }
        self.w.reshape(copy.low, copy.high); // 10b0:33bc
    }
}

/// The plots (10b0:29c0) and the location map (10b0:52a4).
impl Generator<'_> {
    /// Numbers the land's plots: each piece of land, its corners joined
    /// along the map's rows and columns (the plot map's entries are pairs
    /// of rows), is one plot, numbered in the order it's found going
    /// down the columns. The sea is 0. The number of plots. // 10b0:29c0
    fn number_plots(&mut self) -> u16 {
        const UNSET: u16 = 0xffff;
        let island = &mut self.w.island;
        for i in (0..=250).step_by(2) {
            for j in 0..=250 {
                island.plot_map.set((i, j), UNSET);
            }
        }
        let mut plot: u16 = 1;
        for j in 0..=250 {
            for i in 0..=250 {
                let pos = (i, j);
                let land = self.height(pos) != 0;
                let entry = self.w.island.plot_map.get(pos).unwrap_or(0);
                if i % 2 == 0 {
                    if !land && entry == UNSET {
                        self.w.island.plot_map.set(pos, 0);
                    }
                    let entry = self.w.island.plot_map.get(pos).unwrap_or(0);
                    if land && entry == UNSET {
                        self.fill_plot(pos, plot); // 10b0:23c3
                        plot = plot.wrapping_add(1);
                    }
                } else if land && entry == 0 {
                    self.fill_plot(pos, plot);
                    plot = plot.wrapping_add(1);
                }
            }
        }
        plot.wrapping_sub(1)
    }

    /// Gives the land joined to the corner `pos` the plot `plot`, a
    /// depth-first walk with a stack of where it left off (three arrays
    /// of 35001 bytes in the original, which is enough). // 10b0:23c3
    fn fill_plot(&mut self, start: Pos, plot: u16) {
        const UNSET: u16 = 0xffff;
        let mut stack: Vec<(Pos, u8)> = Vec::new();
        let (mut pos, mut state) = (start, 0u8); // 10b0:239a
        loop {
            let (i, j) = pos;
            if state == 0 {
                self.w.island.plot_map.set(pos, plot);
                state = 1;
                continue;
            }
            let island = &self.w.island;
            let h = |p: Pos| island.heights.get(p).unwrap_or(0) != 0;
            let unset = |p: Pos| island.plot_map.get(p) == Some(UNSET);
            let even = i % 2 == 0;
            // The corner to go to from this state, if any.
            let next = match state {
                1 | 2 => {
                    let dj = if state == 1 { -1 } else { 1 };
                    let partner = if even { i + 1 } else { i - 1 };
                    if h((i, j + dj)) && unset((i, j + dj)) {
                        Some((i, j + dj))
                    } else if h((partner, j)) && h((partner, j + dj)) && unset((i, j + dj)) {
                        Some((partner, j + dj))
                    } else {
                        None
                    }
                }
                3 if even => (h((i - 1, j)) && unset((i - 2, j))).then_some((i - 1, j)),
                3 => (h((i - 1, j)) && h((i - 2, j)) && unset((i - 2, j))).then_some((i - 2, j)),
                4 if even => (h((i + 1, j)) && h((i + 2, j)) && unset((i + 2, j))).then_some((i + 2, j)),
                4 => (h((i + 1, j)) && unset((i + 1, j))).then_some((i + 1, j)),
                _ => match stack.pop() {
                    // 10b0:233b
                    Some((p, s)) => {
                        (pos, state) = (p, s);
                        continue;
                    }
                    None => return,
                },
            };
            match next {
                Some(p) => {
                    stack.push((pos, state + 1)); // 10b0:22dd
                    (pos, state) = (p, 0);
                }
                None => state += 1,
            }
        }
    }

    /// The location map: rivers, the shore's sloping water and the sand
    /// at sea level add their effects within 3 cells (1030:1daa), and so
    /// do the buildings (the palms) theirs (1030:1d10). `i` outside, `j`
    /// inside. // 10b0:52a4, 10b0:517b
    fn location(&mut self) {
        for i in 0..=250 {
            for j in 0..=250 {
                let pos = (i, j);
                let w = &mut *self.w;
                let ground = w.island.ground.at(pos);
                if ground > 0x36 {
                    w.change_location(pos, RIVER_EFFECT, WATER_EFFECT_RADIUS, false);
                }
                if ground == 2 && w.island.forms.at(pos) != 16 {
                    w.change_location(pos, SHORE_EFFECT, WATER_EFFECT_RADIUS, false);
                }
                if ground == 0 && w.island.heights.at(pos) < 2 {
                    w.change_location(pos, BEACH_EFFECT, WATER_EFFECT_RADIUS, false);
                }
                let building = w.island.buildings.at(pos);
                if building != NO_BUILDING {
                    w.add_location_effect(pos, building);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vcl::random::Random;

    /// Tables with every cell `v`, except the hills' sizes and counts.
    fn tables() -> Maperste {
        let t = |v: i16| [[v; 5]; 5];
        Maperste {
            ig_min_leere: t(20),
            ig_min_radius: t(10),
            gi_radius: t(84),
            gi_leere1: t(100),
            gi_leere2: t(60),
            at_anzahl1: t(26),
            at_anzahl2: t(22),
            at_groesse1: t(10),
            at_groesse2: t(15),
            at_variation1: t(14),
            at_variation2: t(13),
        }
    }

    fn world(seed: u32) -> World {
        World {
            random: Random::new(seed),
            ..World::default()
        }
    }

    /// No rivers: their levelling may leave steeper steps where it's
    /// stopped (10b0:0e0c writes the copy back after an error).
    fn settings(island: i16) -> Settings {
        Settings {
            island,
            rivers: 0,
            ..Settings::default()
        }
    }

    /// No corner more than a step from its neighbours, none on the edge.
    fn smooth_enough(w: &World) -> usize {
        let mut land = 0;
        for i in 0..=250i16 {
            for j in 0..=250i16 {
                let h = i16::from(w.island.heights.at((i, j)));
                land += usize::from(h > 0);
                if !(2..=248).contains(&i) || !(2..=248).contains(&j) {
                    assert_eq!(h, 0, "{i},{j}");
                    continue;
                }
                for (di, dj) in NEIGHBOURS {
                    let g = i16::from(w.island.heights.at((i + di, j + dj)));
                    assert!((h - g).abs() <= 1, "{i},{j}: {h} next to {g}");
                }
            }
        }
        land
    }

    #[test]
    fn same_seed_same_island() {
        for island in 0..=2 {
            let mut a = world(42);
            let mut b = world(42);
            let mut c = world(43);
            let plots = a.generate_island(&settings(island), &tables());
            assert_eq!(b.generate_island(&settings(island), &tables()), plots);
            assert_eq!(a, b);
            c.generate_island(&settings(island), &tables());
            assert_ne!(a.island, c.island);
            assert!(smooth_enough(&a) > 1000, "shape {island}");
            assert!(plots >= 1);
        }
    }

    #[test]
    fn a_hill() {
        let mut w = world(7);
        let t = tables();
        let mut g = Generator {
            w: &mut w,
            t: &t,
            relief: 2,
            land: 2,
        };
        g.clear();
        g.hill((100, 100), 6);
        assert_eq!(g.w.island.heights.at((100, 100)), 1);
        g.hill((100, 100), 6);
        assert_eq!(g.w.island.heights.at((100, 100)), 2);
        assert_eq!(g.w.island.heights.at((107, 100)), 0);
        // Too close to the edge: nothing.
        g.hill((5, 100), 6);
        assert_eq!(g.w.island.heights.at((5, 100)), 0);
        assert!(smooth_enough(g.w) > 0);
    }

    #[test]
    fn plots_by_pieces_of_land() {
        let mut w = world(1);
        let t = tables();
        let mut g = Generator {
            w: &mut w,
            t: &t,
            relief: 2,
            land: 2,
        };
        g.clear();
        // An island of rows 10 to 13, another of the odd row 21 alone,
        // and a third joined to the first only diagonally.
        for i in 10..=13 {
            for j in 10..=20 {
                g.w.island.heights.set((i, j), 1);
            }
        }
        for j in 30..=35 {
            g.w.island.heights.set((21, j), 1);
        }
        g.w.island.heights.set((14, 21), 1);
        let plots = g.number_plots();
        assert_eq!(plots, 3);
        let island = &g.w.island;
        assert_eq!(island.plot_map.get((10, 10)), Some(1));
        assert_eq!(island.plot_map.get((13, 20)), Some(1));
        assert_eq!(island.plot_map.get((14, 21)), Some(2));
        assert_eq!(island.plot_map.get((21, 30)), Some(3));
        assert_eq!(island.plot_map.get((0, 0)), Some(0));
        assert_eq!(island.plot_map.get((40, 40)), Some(0));
    }

    #[test]
    fn fix_atoll_hills() {
        let atoll = |fixed: bool, t: &Maperste| {
            let mut w = world(42);
            w.fixes.set(Fix::AtollHills, fixed);
            w.generate_island(&settings(0), t);
            smooth_enough(&w);
            w.island
        };
        // The small hills are centred by half of ATVariation2, not 1.
        assert_ne!(atoll(false, &tables()), atoll(true, &tables()));
        let mut same = tables();
        same.at_variation2 = same.at_variation1;
        assert_eq!(atoll(false, &same), atoll(true, &same));
    }

    #[test]
    fn fix_river_start() {
        for fixed in [false, true] {
            let mut w = world(5);
            w.fixes.set(Fix::RiverStart, fixed);
            let t = tables();
            let mut g = Generator {
                w: &mut w,
                t: &t,
                relief: 2,
                land: 2,
            };
            g.clear();
            // All sea: the original starts the river on the last try.
            assert_eq!(g.river_start().is_some(), !fixed);
        }
    }

    #[test]
    fn fix_generator_levelling() {
        for fixed in [false, true] {
            let mut w = world(5);
            w.fixes.set(Fix::GeneratorLevelling, fixed);
            let t = tables();
            let mut g = Generator {
                w: &mut w,
                t: &t,
                relief: 2,
                land: 2,
            };
            g.clear();
            for i in 90..=110 {
                for j in 90..=110 {
                    g.w.island.heights.set((i, j), 2);
                }
            }
            // (101, 100) follows the lowered (100, 100) down, and a river
            // on one of its squares stops it.
            g.w.island.heights.set((101, 100), 3);
            g.w.island.ground.set((102, 100), RIVER + 4);
            assert_eq!(g.lower_checked((100, 100)), 0x10);
            // The original keeps the first corner lowered: a step of two.
            let h = g.w.island.heights.at((100, 100));
            assert_eq!(h, if fixed { 2 } else { 1 });
            assert_eq!(g.w.island.heights.at((101, 100)), 3);
        }
    }

    #[test]
    fn fix_river_flat_way() {
        for fixed in [false, true] {
            let mut w = world(5);
            w.fixes.set(Fix::RiverFlatWay, fixed);
            let t = tables();
            let mut g = Generator {
                w: &mut w,
                t: &t,
                relief: 2,
                land: 2,
            };
            g.clear();
            // Flat land, the middle of the map: the three ways tie up to
            // the edge, which they reach together.
            for i in 0..=250 {
                for j in 0..=250 {
                    g.w.island.heights.set((i, j), 2);
                }
            }
            // Lower at the far end of the way back.
            g.w.island.heights.set((124, 1), 0);
            g.w.island.heights.set((125, 1), 0);
            let way = g.flat_way(0, (125, 125));
            // The original looks on round the map, and the way `j + 1`
            // comes back onto it at the lower square; the fix stops at the
            // edge and takes the last of the ways still open, `i + 1`.
            assert_eq!(way, u8::from(fixed));
        }
    }
}
