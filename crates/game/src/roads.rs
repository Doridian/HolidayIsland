//! Roads (segment `1080` and 1078:2178..1078:2396): which piece a road
//! square shows follows the roads and bridges next to it; building a road
//! ([`World::build_road`]) lays a straight one along the longer axis from
//! where it starts towards where it ends, and demolishing one
//! ([`World::demolish`]) puts its neighbours' pieces right.
//!
//! The human's road tool drags a road out: `road_drag_start` (1080:3195)
//! keeps the start square (`1110:6344`, `1110:6346`), its ground
//! (`1110:704e`) and form (`1110:704f`) and sets `1110:634c`, and letting
//! go builds the road (1080:31ed). In the port the start square is the
//! view's (`crate::tools`) and an argument of [`World::build_road`], as is
//! the ground; `1110:634c` (which the computers wait on, and building a
//! road clears) and the ground are the world's
//! ([`World::road_dragging`], [`World::road_drag_ground`]), set by the
//! drag's command (`crate::command`). The computers pass the ground too:
//! the original lets them use whatever ground the human's last drag
//! started on, which only matters when that was water (then they look
//! for bridges). Before any drag it's [`NO_DRAG_GROUND`].

use crate::accounts::BOOK_OTHER;
use crate::construction::NOT_ENOUGH_MONEY;
use crate::enhance::Fix;
use crate::simulation::Tick;
use crate::world::{NO_BUILDING, PALM, Pos, World, on_map};

/// The road pieces' grounds (`STRASPOS.DAT`): 10–54, and 55–58 where a
/// road crosses a river (with building 11, a bridge, on it).
/// // 1078:280b
pub(crate) const ROADS: std::ops::RangeInclusive<u8> = 10..=0x3a;
/// The road crossing a river: grounds 55–58, which become the river
/// pieces 59, 61, 60 and 62 without the road. // 1078:280b, 1080:232a
pub(crate) const CROSSINGS: [(u8, u8); 4] = [(0x37, 0x3b), (0x38, 0x3d), (0x39, 0x3c), (0x3a, 0x3e)];
/// The grounds of bridges over the sea (building type 52): 95–100.
/// // 1078:280b
pub(crate) const BRIDGES: std::ops::RangeInclusive<u8> = 0x5f..=0x64;

/// The building type of a bridge over a river. // 1080:232a
const RIVER_BRIDGE: u8 = 11;

/// A flat road's piece by which neighbours have a road or bridge: 1 for
/// `(i + 1, j)`, 2 for `(i, j + 1)`, 4 for `(i - 1, j)`, 8 for `(i, j - 1)`.
/// // 1080:232a (its jump table at 1080:230a)
const PIECES: [u8; 16] = [
    10, 11, 10, 0x13, 11, 11, 0x12, 0xd, 10, 0x14, 10, 0xe, 0x11, 0xf, 0xc, 0x10,
];

/// A flat square's form. // 1080:302a
pub(crate) const FLAT: u8 = 16;

/// The river pieces a road can cross. // 1110:13f8 (`set of Byte`)
pub(crate) const CROSSABLE: [u8; 4] = [59, 60, 61, 62];

/// The building type of a bridge over the sea. // 1080:261a
const SEA_BRIDGE: u8 = 0x34;

/// The ground a computer's road starts from, as far as the road planner
/// is concerned: sand, what `1110:704e` holds before the human drags a
/// road. See the module's documentation.
pub const NO_DRAG_GROUND: u8 = 0;

/// A seat's road drag: whether it's on, and the ground and form of the
/// square it started on. Seat 1's are the original's
/// [`World::road_dragging`], [`World::road_drag_ground`] and
/// [`World::road_drag_form`]; the other seats' a port extension
/// (`crate::seats::SeatSlots::drag`, [`World::road_drag`]), so two
/// humans dragging at once each have their own. // 1110:634c, 1110:704e,
/// 1110:704f
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RoadDrag {
    pub on: bool,
    pub ground: u8,
    pub form: u8,
}

/// The sound of a road being built, for player 1. // 1080:3216
const ROAD_SOUND: u8 = 0x11;

/// Why [`World::build_road`] built nothing. Only player 1 gets the
/// messages; the others get [`RoadError::NotBuilt`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoadError {
    /// Nothing to build there, no message.
    NotBuilt,
    /// `FEHLER.TXT` 0xe: not enough money for a road as long as the drag.
    NotEnoughMoney,
    /// The building on this square is in the way: its name and
    /// `FEHLER.TXT` 0x1d in the status bar, or message 1 for a type past
    /// 0x7e (1030:235a).
    InTheWay(Pos),
    /// `FEHLER.TXT` 0x1f: a river a road can't cross.
    River,
}

/// What the road planner lays: a road over land, or a bridge over the sea
/// (when the drag starts on water). // 1080:3216
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Road,
    Bridge,
}

impl World {
    /// The ground of `pos`, or 2 (water) for a square on the map's edge or
    /// off it: only 1..=249 count. // 1000:3e0d
    pub(crate) fn ground_or_water(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.ground.at((i, j))
        } else {
            2
        }
    }

    /// The form of `pos`, or [`FLAT`] on the edge or off the map.
    /// // 1000:3e8d
    pub(crate) fn form_or_flat(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.forms.at((i, j))
        } else {
            FLAT
        }
    }

    /// The owner of `pos`, 0 unless both coordinates are 1..=249.
    /// // 1000:3f15
    fn owner_or_zero(&self, (i, j): Pos) -> u8 {
        if (1..=249).contains(&i) && (1..=249).contains(&j) {
            self.island.owners.at((i, j))
        } else {
            0
        }
    }

    /// Builds a road for `player` from `from` towards `to`: a straight one
    /// along the axis on which they're further apart, as far as the land
    /// lets it go ([`World::plan_road`]); `start_ground` is the ground the
    /// drag started on (`1110:704e`, see the module's documentation). Each
    /// new square (one that had no owner) costs `Strassenbaukosten`, which
    /// the computers pay too. Where the road ended. // 1078:2396
    ///
    /// Player 1 also needs the money for a road as long as the drag,
    /// books the cost and sees it rise from `to`; the original redraws the
    /// area on the main screen and the overview map. Single human: here
    /// every human seat does ([`World::is_human`]), and a road ends the
    /// builder's drag, the original's one flag for player 1 and the
    /// computers ([`World::end_road_drag`]).
    ///
    /// # Errors
    /// [`RoadError`] when nothing was built.
    pub fn build_road(
        &mut self,
        player: u8,
        from: Pos,
        to: Pos,
        start_ground: u8,
        tick: &mut Tick,
    ) -> Result<Pos, RoadError> {
        if !on_map(from) || !on_map(to) {
            self.end_road_drag(player);
            return Err(RoadError::NotBuilt);
        }
        if self.is_human(player) {
            let span = |a: i16, b: i16| i32::from(a.max(b)) - i32::from(a.min(b)) + 1;
            let longest = span(from.1, to.1).max(span(from.0, to.0));
            if self.money_of(player) < longest.wrapping_mul(self.tables.road_cost) {
                // `show_error_message(0xe)`, the view's.
                debug_assert_eq!(NOT_ENOUGH_MONEY, 0xe);
                self.end_road_drag(player);
                return Err(RoadError::NotEnoughMoney);
            }
        }
        let Some((end, squares)) = self.plan_road(player, from, to, start_ground, tick) else {
            if !self.is_human(player) {
                return Err(RoadError::NotBuilt);
            }
            let building = self.island.buildings.at(from);
            let ground = self.island.ground.at(from);
            return Err(if ![NO_BUILDING, RIVER_BRIDGE, SEA_BRIDGE].contains(&building) {
                RoadError::InTheWay(from) // 1030:235a
            } else if (0x37..=0x5e).contains(&ground) && !CROSSABLE.contains(&ground) {
                RoadError::River
            } else {
                RoadError::NotBuilt
            });
        };
        self.lay_road(from, end, start_ground);
        // The original redraws the road and a square around it on the
        // overview map (10b8:15f4) and the main screen (1050:264d).
        for a in from.0.min(end.0) - 1..=from.0.max(end.0) + 1 {
            for b in from.1.min(end.1) - 1..=from.1.max(end.1) + 1 {
                if on_map((a, b)) {
                    tick.redraw.push((a, b));
                }
            }
        }
        let cost = squares.wrapping_mul(self.tables.road_cost);
        self.add_money(player, cost.wrapping_neg()); // 1078:01e7
        if self.is_human(player) {
            self.book_for(player, BOOK_OTHER, cost.wrapping_neg()); // 1070:1419
            tick.books = true; // 1070:2617
            self.float_money(cost, to); // 1088:0153
        }
        Ok(end)
    }

    /// Plans and lays the road from `from` towards `to` (bytes, as the
    /// original passes them): from a drag that started on water a bridge,
    /// if the water ends at a ramp facing the start's ramp (forms 0 and 2,
    /// or 1 and 3), else a road if one can start at `from`. Squares that
    /// aren't a road yet and that no one else owns get the road (or
    /// bridge) and `player` as their owner; the road goes on over the
    /// others. Where it ended and how many squares it took that no one
    /// owned, if it built any.
    ///
    /// It also clears `1110:634c` and sets the animations' `1110:0056`
    /// (which it clears while it works), whatever the player chose: in
    /// the port [`Options::people_spawn`](crate::simulation::Options)
    /// goes on whenever anyone builds a road. // 1080:3216
    fn plan_road(&mut self, player: u8, from: Pos, to: Pos, start_ground: u8, tick: &mut Tick) -> Option<(Pos, i32)> {
        self.options.people_spawn = false;
        let byte = |v: i16| v.to_le_bytes()[0];
        let (fi, fj) = (byte(from.0), byte(from.1));
        let (ti, tj) = (byte(to.0), byte(to.1));
        let along_i = (i16::from(tj) - i16::from(fj)).abs() < (i16::from(ti) - i16::from(fi)).abs();
        let pos = |a: u8, b: u8| (i16::from(a), i16::from(b));
        let kind = if start_ground == 2 || start_ground == 3 {
            // How far the water goes.
            let fits = |w: &World, a: u8, b: u8| w.fits(Kind::Bridge, from, start_ground, pos(a, b));
            let (mut a, mut b) = (fi, fj);
            if along_i {
                a = self.scan_road(fi, ti, |w, a| fits(w, a, fj));
            } else {
                b = self.scan_road(fj, tj, |w, b| fits(w, fi, b));
            }
            let ramps = (self.form_or_flat(from), self.form_or_flat(pos(a, b)));
            (matches!(ramps, (0, 2) | (2, 0) | (1, 3) | (3, 1)) && self.can_bridge(from)).then_some(Kind::Bridge)
        } else {
            self.can_road(from, start_ground, from).then_some(Kind::Road)
        };
        let mut result = None;
        if let Some(kind) = kind {
            let mut squares = 0i32;
            let mut end = from;
            let mut built = false;
            let mut visit = |w: &mut World, at: Pos| {
                let ground = w.ground_or_water(at);
                let owner = w.owner_or_zero(at);
                if !ROADS.contains(&ground) && (owner == 0 || owner == player) {
                    // BUG(original): a bridge off the map writes owners past it; the port doesn't.
                    // The original reads and writes past the map for a
                    // bridge that runs off it; the port doesn't.
                    if on_map(at) && w.island.owners.at(at) == 0 {
                        squares = squares.wrapping_add(1);
                    }
                    w.place_piece(kind, at);
                    if on_map(at) {
                        w.island.owners.set(at, player);
                    }
                    built = true;
                }
            };
            if along_i {
                end.1 = from.1;
                self.walk_road(
                    fi,
                    ti,
                    |w, a| w.fits(kind, from, start_ground, pos(a, fj)),
                    |w, a| {
                        visit(w, pos(a, fj));
                        end.0 = pos(a, fj).0;
                    },
                );
            } else {
                end.0 = from.0;
                self.walk_road(
                    fj,
                    tj,
                    |w, b| w.fits(kind, from, start_ground, pos(fi, b)),
                    |w, b| {
                        visit(w, pos(fi, b));
                        end.1 = pos(fi, b).1;
                    },
                );
            }
            if built {
                result = Some((end, squares));
            }
        }
        if self.is_human(player) && result.is_some() {
            tick.sounds.push(ROAD_SOUND); // 10a0:30b6
        }
        self.end_road_drag(player);
        // BUG(original): turns the "new people" option on whatever the player chose.
        self.options.people_spawn = true;
        result
    }

    /// How far from `start` towards `stop` (bytes) the squares `fits`, one
    /// back from the first that doesn't or from one past `stop`.
    /// // 1080:3216
    fn scan_road(&self, start: u8, stop: u8, fits: impl Fn(&World, u8) -> bool) -> u8 {
        let mut a = start;
        // BUG(original): a bridge along a row of water to the map's edge loops for ever; the port stops after one round.
        // The original goes round the byte for ever on a row of water to
        // the map's edge; the port stops after one round.
        for _ in 0..=u8::MAX {
            let on = if start < stop { a <= stop } else { stop <= a };
            if !fits(self, a) || !on {
                break;
            }
            a = if start < stop {
                a.wrapping_add(1)
            } else {
                a.wrapping_sub(1)
            };
        }
        if start < stop {
            a.wrapping_sub(1)
        } else {
            a.wrapping_add(1)
        }
    }

    /// Goes from `start` towards `stop` (bytes) while the squares `fits`,
    /// up to and with `stop`, visiting each. // 1080:3216
    fn walk_road(
        &mut self,
        start: u8,
        stop: u8,
        fits: impl Fn(&World, u8) -> bool,
        mut visit: impl FnMut(&mut World, u8),
    ) {
        let mut a = start;
        for _ in 0..=u8::MAX {
            let on = if start < stop { a <= stop } else { stop <= a };
            if !fits(self, a) || !on {
                break;
            }
            visit(self, a);
            a = if start < stop {
                a.wrapping_add(1)
            } else {
                a.wrapping_sub(1)
            };
        }
    }

    /// Whether the planner's road or bridge goes on over `pos`.
    fn fits(&self, kind: Kind, from: Pos, start_ground: u8, pos: Pos) -> bool {
        match kind {
            Kind::Road => self.can_road(pos, start_ground, from),
            Kind::Bridge => self.can_bridge(pos),
        }
    }

    /// Lays the planner's road or bridge on `pos`.
    fn place_piece(&mut self, kind: Kind, pos: Pos) {
        match kind {
            Kind::Road => self.place_road(pos),     // 1080:2b0c
            Kind::Bridge => self.place_bridge(pos), // 1080:2e6a
        }
    }

    /// Whether a road from `from` goes on over `pos`: no building but a
    /// palm, not water, a drag that started on sand, grass, sand to grass,
    /// a road or a crossable river; then a flat square that isn't a river
    /// (or a crossable river piece across the road's way), or a ramp
    /// (forms 0–3) the road goes up or down. // 1080:2831
    pub(crate) fn can_road(&self, pos: Pos, start_ground: u8, from: Pos) -> bool {
        let building = self.building_or_none(pos); // 1000:3dc9
        if building != NO_BUILDING && building != PALM {
            return false;
        }
        let ground = self.ground_or_water(pos);
        if ground == 2 || ground == 3 {
            return false;
        }
        if !matches!(start_ground, 0 | 1 | 4 | 5 | 10..=0x3e) {
            return false;
        }
        let (along_i, along_j) = (from.1 == pos.1, from.0 == pos.0);
        match self.form_or_flat(pos) {
            FLAT => match ground {
                // Not 0x3a, the last crossing.
                g if g < 0x3a => true,
                0x3b | 0x3c => along_i,
                0x3d | 0x3e => along_j,
                _ => false,
            },
            0 | 2 => along_i,
            1 | 3 => along_j,
            _ => false,
        }
    }

    /// Whether a bridge goes on over `pos`: water, flat or a ramp (forms
    /// 0–3). The map's edge and off it count as flat water. // 1080:2a82
    pub(crate) fn can_bridge(&self, pos: Pos) -> bool {
        matches!(self.ground_or_water(pos), 2 | 3) && matches!(self.form_or_flat(pos), FLAT | 0..=3)
    }

    /// Whether a road can be laid on `pos`: no building but a palm, not
    /// water, flat or a ramp. // 1080:29d8
    pub(crate) fn road_fits(&self, pos: Pos) -> bool {
        matches!(self.building_or_none(pos), NO_BUILDING | PALM)
            && !matches!(self.ground_or_water(pos), 2 | 3)
            && matches!(self.form_or_flat(pos), FLAT | 0..=3)
    }

    /// Lays a road on `pos` (the palm goes): on a flat square the piece
    /// for its neighbours, on a ramp (form 0–3) the ramp's piece (0x15 +
    /// form) in its ground's set; then its flat road neighbours get their
    /// pieces again, and `layer_551c` and `layer_5520` are cleared there.
    /// // 1080:2b0c
    fn place_road(&mut self, pos: Pos) {
        if !self.road_fits(pos) {
            return;
        }
        let form = self.form_or_flat(pos);
        if form == FLAT {
            self.island.parts.set(pos, 0);
            self.island.buildings.set(pos, NO_BUILDING);
            let piece = self.road_piece(pos); // 1080:232a
            self.island.ground.set(pos, piece);
        } else if form <= 3 {
            self.island.parts.set(pos, 0);
            self.island.buildings.set(pos, NO_BUILDING);
            let piece = self.road_set(pos).wrapping_add(0x15 + form); // 1080:22be
            self.island.ground.set(pos, piece);
        }
        self.reconnect_roads(pos); // the same as 1080:302a
        self.island.layer_551c.set(pos, 0xff);
        self.island.layer_5520.set(pos, 0xff);
    }

    /// Lays a bridge over the sea on `pos` if it can take one, then gives
    /// its flat road neighbours their pieces again. // 1080:2e6a
    fn place_bridge(&mut self, pos: Pos) {
        if self.can_bridge(pos) && on_map(pos) {
            let piece = self.bridge_piece(pos); // 1080:261a
            self.island.ground.set(pos, piece);
            self.island.layer_551c.set(pos, 0xff);
            self.island.layer_5520.set(pos, 0xff);
        }
        self.reconnect_roads(pos);
    }

    /// The bridge piece for `pos`, with building 0x34 and the piece's part:
    /// on a ramp (form 0–3) 0x5f + form with part form; flat, 99 (part 4,
    /// along `i`) next to a bridge along `i` or a ramp at `(i ± 1, j)`,
    /// else 100 (part 5). // 1080:261a
    fn bridge_piece(&mut self, (i, j): Pos) -> u8 {
        let ground = self.ground_or_water((i, j));
        let form = self.form_or_flat((i, j));
        let (piece, part) = if form == FLAT {
            let along = |w: &World, pos| {
                matches!(w.part_or_zero(pos), 4 | 0 | 2) && BRIDGES.contains(&w.ground_or_water(pos)) // 1000:3ed1
            };
            if along(self, (i + 1, j)) || along(self, (i - 1, j)) {
                (99, 4)
            } else {
                (100, 5)
            }
        } else if form <= 3 {
            (0x5f + form, form)
        } else {
            return ground;
        };
        self.island.buildings.set((i, j), SEA_BRIDGE);
        self.island.parts.set((i, j), part);
        piece
    }

    /// The road just built from `from` to `end` counts on its plot's
    /// built squares (`+0x356`; not for a bridge from the sea), and every
    /// building whose first square (part 0; with `fix.road_connections`
    /// the first square of any look) is within 8 squares before and
    /// 4 after the road, on both axes, works out how well it's connected
    /// again (1030:1706, 1078:1a66). // 1078:2178
    fn lay_road(&mut self, from: Pos, end: Pos, start_ground: u8) {
        if start_ground != 2 && start_ground != 3 {
            let length = (from.0 - end.0).abs().max((from.1 - end.1).abs());
            // The original writes through a nil pointer for a road off
            // any plot.
            // BUG(original): a road off any plot is counted through a nil pointer; skipped.
            if let Some(p) = self.plot_of(from) {
                p.set_built(p.built().wrapping_add(length).wrapping_add(1));
            }
        }
        let range = |a: i16, b: i16| (a.min(b) - 8).clamp(0, 250)..=(a.max(b) + 4).clamp(0, 250);
        let any_look = self.fixes.has(Fix::RoadConnections);
        for a in range(from.0, end.0) {
            for b in range(from.1, end.1) {
                let building = self.island.buildings.at((a, b));
                // BUG(original): only part 0 counts, so houses with another look never get their connection again.
                let first = if any_look {
                    self.building_origin((a, b)) == (a, b) // 1030:154c
                } else {
                    self.island.parts.at((a, b)) == 0
                };
                if building < 0x7f && first {
                    let value = self.connection(building, (a, b)); // 1030:1706
                    self.set_connection_at((a, b), value); // 1078:1a66
                }
            }
        }
    }

    /// Gives the record of the building on `pos` (found by its first
    /// square) the connection `value`. The original goes on through the
    /// list after the first match, setting every record there. // 1078:1a66
    fn set_connection_at(&mut self, pos: Pos, value: u8) {
        let origin = self.building_origin(pos); // 1030:154c
        let building = self.island.buildings.get(origin).unwrap_or(NO_BUILDING);
        match self.record_kind(building) {
            0 => self
                .houses
                .iter_mut()
                .filter(|h| h.pos() == origin)
                .for_each(|h| h.set_connection(value)),
            1 | 2 => self
                .facilities
                .iter_mut()
                .filter(|f| f.pos() == origin)
                .for_each(|f| f.set_connection(value)),
            _ => {}
        }
    }

    /// The flat road squares next to `(i, j)` get the piece their
    /// neighbours call for, in the order `(i + 1, j)`, `(i - 1, j)`,
    /// `(i, j + 1)`, `(i, j - 1)`. // 1080:302a
    pub(crate) fn reconnect_roads(&mut self, (i, j): Pos) {
        for pos in [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)] {
            if self.form_or_flat(pos) == FLAT && ROADS.contains(&self.ground_or_water(pos)) {
                let piece = self.road_piece(pos);
                self.island.ground.set(pos, piece);
            }
        }
    }

    /// The piece the road on `pos` (on the map, not on its edge) shows: a
    /// plain road gets its ground back first ([`World::reset_ground`]),
    /// and then the piece for its neighbours from [`PIECES`] in that
    /// ground's set of pieces; a crossing keeps its piece, and a river
    /// piece becomes the crossing with a bridge on it. // 1080:232a
    fn road_piece(&mut self, (i, j): Pos) -> u8 {
        let ground = self.ground_or_water((i, j));
        if ground < 0x37 {
            self.reset_ground((i, j)); // 10b0:3199
            let road = |w: &World, pos| {
                let g = w.ground_or_water(pos);
                ROADS.contains(&g) || BRIDGES.contains(&g)
            };
            let neighbours = [(i + 1, j), (i, j + 1), (i - 1, j), (i, j - 1)];
            let mask = neighbours
                .iter()
                .enumerate()
                .filter(|&(_, &pos)| road(self, pos))
                .fold(0, |mask, (bit, _)| mask | (1 << bit));
            PIECES[mask].wrapping_add(self.road_set((i, j)))
        } else if let Some(n) = CROSSINGS.iter().position(|&(_, river)| river == ground) {
            // 59 → 55 with part 0, 61 → 56 with 1, 60 → 57 with 2, 62 →
            // 58 with 3.
            self.island.buildings.set((i, j), RIVER_BRIDGE);
            self.island.parts.set((i, j), u8::try_from(n).unwrap_or(0));
            CROSSINGS[n].0
        } else {
            ground
        }
    }

    /// Where the road pieces for the ground of `pos` start: 0 on sand, 15
    /// on grass, 0x5f on water and 0x1e on sand to grass. // 1080:22be
    fn road_set(&self, pos: Pos) -> u8 {
        match self.ground_or_water(pos) {
            1 => 0xf,
            2 | 3 => 0x5f,
            4 | 5 => 0x1e,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::NO_BUILDING;

    /// Raises the corners of the squares in `0..=n` on both axes to
    /// `height`, so they're dry and flat.
    fn land(w: &mut World, n: i16, height: u8) {
        for i in 0..=n {
            for j in 0..=n {
                w.island.heights.set((i, j), height);
                w.island.forms.set((i, j), FLAT);
            }
        }
    }

    #[test]
    fn pieces_follow_the_neighbours() {
        let mut w = World::default();
        land(&mut w, 20, 4);
        // Grass (the corners are high) with a road from (10, 10) to
        // (10, 12) and a road ending at (11, 10).
        for pos in [(10, 10), (10, 11), (10, 12), (11, 10)] {
            w.island.ground.set(pos, 10);
        }
        w.reconnect_roads((10, 11));
        // (11, 11) isn't a road; (10, 10) has (11, 10) and (10, 11) next
        // to it: 1 + 2.
        assert_eq!(w.island.ground.at((10, 10)), 0x13 + 0xf);
        // (10, 12) has only (10, 11): 8.
        assert_eq!(w.island.ground.at((10, 12)), 10 + 0xf);
        assert_eq!(w.island.ground.at((11, 11)), 0);
        assert_eq!(w.island.buildings.at((10, 10)), NO_BUILDING);
    }

    /// Grass at height 4 on `0..=40`, plot 1 on all of it, and roads at
    /// 7 a square.
    fn world() -> World {
        let mut w = World {
            tables: std::sync::Arc::new(crate::world::Tables {
                road_cost: 7,
                ..crate::world::Tables::default()
            }),
            ..World::default()
        };
        land(&mut w, 40, 4);
        for i in 0..=40 {
            for j in 0..=40 {
                w.island.ground.set((i, j), 1);
                w.island.plot_map.set((i, j), 1);
            }
        }
        w.plots = vec![crate::world::Plot::new(1)];
        w
    }

    #[test]
    fn a_road_across_flat_land() {
        let mut w = world();
        let mut tick = Tick::default();
        // Further apart along j: a road along j, as far as `to`.
        assert_eq!(
            w.build_road(2, (10, 10), (12, 15), NO_DRAG_GROUND, &mut tick),
            Ok((10, 15))
        );
        for j in 10..=15 {
            assert!(ROADS.contains(&w.island.ground.at((10, j))), "{j}");
            assert_eq!(w.island.owners.at((10, j)), 2);
        }
        assert!(!ROADS.contains(&w.island.ground.at((10, 16))));
        assert_eq!(w.finances.money[1], -6 * 7);
        assert_eq!(w.plots[0].built(), 6);
        assert!(w.island.forms.at((10, 10)) == FLAT && tick.redraw.contains(&(9, 9)));
        // Over its own road again it pays only for the new squares.
        assert_eq!(
            w.build_road(2, (10, 15), (10, 17), NO_DRAG_GROUND, &mut tick),
            Ok((10, 17))
        );
        assert_eq!(w.finances.money[1], -8 * 7);
        // Someone else's square is skipped, a building stops it.
        w.island.owners.set((20, 12), 3);
        w.island.buildings.set((20, 14), 5);
        assert_eq!(
            w.build_road(2, (20, 10), (20, 20), NO_DRAG_GROUND, &mut tick),
            Ok((20, 13))
        );
        assert!(!ROADS.contains(&w.island.ground.at((20, 12))));
        assert!(ROADS.contains(&w.island.ground.at((20, 13))));
        // Nothing can start on a building; player 1 hears why.
        assert_eq!(
            w.build_road(1, (20, 14), (20, 20), NO_DRAG_GROUND, &mut tick),
            Err(RoadError::NotEnoughMoney)
        );
        w.finances.money[0] = 1000;
        assert_eq!(
            w.build_road(1, (20, 14), (20, 20), NO_DRAG_GROUND, &mut tick),
            Err(RoadError::InTheWay((20, 14)))
        );
        assert_eq!(
            w.build_road(2, (20, 14), (20, 20), NO_DRAG_GROUND, &mut tick),
            Err(RoadError::NotBuilt)
        );
    }

    #[test]
    fn ramps_and_rivers() {
        let mut w = world();
        let mut tick = Tick::default();
        // A ramp along the road's way gets the ramp piece; one across it
        // stops the road.
        w.island.forms.set((5, 12), 1);
        w.island.forms.set((7, 12), 0);
        assert_eq!(
            w.build_road(2, (5, 10), (5, 14), NO_DRAG_GROUND, &mut tick),
            Ok((5, 14))
        );
        assert_eq!(w.island.ground.at((5, 12)), 0xf + 0x15 + 1);
        assert_eq!(
            w.build_road(2, (7, 10), (7, 14), NO_DRAG_GROUND, &mut tick),
            Ok((7, 11))
        );
        // A river piece across the way becomes a crossing with a bridge.
        w.island.ground.set((9, 12), 0x3d);
        assert_eq!(
            w.build_road(2, (9, 10), (9, 14), NO_DRAG_GROUND, &mut tick),
            Ok((9, 14))
        );
        assert_eq!(w.island.ground.at((9, 12)), 0x38);
        assert_eq!(w.island.buildings.at((9, 12)), RIVER_BRIDGE);
    }

    #[test]
    fn a_bridge_over_the_sea() {
        let mut w = world();
        let mut tick = Tick::default();
        // Water from (20, 10) to (20, 14), ramps at both ends.
        for j in 10..=14 {
            w.island.ground.set((20, j), 2);
        }
        w.island.forms.set((20, 10), 1);
        w.island.forms.set((20, 14), 3);
        assert_eq!(w.build_road(2, (20, 10), (20, 18), 2, &mut tick), Ok((20, 14)));
        assert_eq!(
            (10..=14).map(|j| w.island.ground.at((20, j))).collect::<Vec<_>>(),
            [0x60, 100, 100, 100, 0x62]
        );
        assert_eq!(w.island.buildings.at((20, 12)), SEA_BRIDGE);
        assert_eq!(w.island.parts.at((20, 12)), 5);
        assert_eq!(w.finances.money[1], -5 * 7);
        // A bridge doesn't count on the plot.
        assert_eq!(w.plots[0].built(), 0);
        // Without a ramp facing the start's, no bridge.
        w.island.forms.set((22, 10), 1);
        w.island.ground.set((22, 10), 2);
        assert_eq!(
            w.build_road(2, (22, 10), (22, 18), 2, &mut tick),
            Err(RoadError::NotBuilt)
        );
    }

    #[test]
    fn roads_connect_the_buildings_near_them() {
        let mut w = world();
        let mut hausnr = vec![[0; 8]; 3];
        hausnr[2] = [1, 0, 0, 1, 1, 1, 0, 0];
        let mut hausdata = vec![[0; 10]; 3];
        hausdata[2][5] = 1;
        w.tables = std::sync::Arc::new(crate::world::Tables {
            hausnr,
            hausdata,
            road_cost: 7,
            ..crate::world::Tables::default()
        });
        w.connection_values = [0, 10, 20, 30, 40, 50];
        let mut house = crate::world::House::new((12, 12));
        house.set_building(2);
        w.houses.push(house);
        w.island.buildings.set((12, 12), 2);
        let mut tick = Tick::default();
        assert!(w.build_road(2, (10, 5), (10, 20), NO_DRAG_GROUND, &mut tick).is_ok());
        // Two squares away.
        assert_eq!(w.houses[0].connection(), 30);
    }

    #[test]
    fn crossings_keep_their_piece() {
        let mut w = World::default();
        land(&mut w, 20, 4);
        w.island.ground.set((5, 6), 0x38);
        w.reconnect_roads((5, 5));
        assert_eq!(w.island.ground.at((5, 6)), 0x38);
        // A river piece next door isn't a road.
        w.island.ground.set((5, 4), 0x3c);
        w.reconnect_roads((5, 5));
        assert_eq!(w.island.ground.at((5, 4)), 0x3c);
        assert_eq!(w.road_piece((5, 4)), 0x39);
        assert_eq!(
            (w.island.buildings.at((5, 4)), w.island.parts.at((5, 4))),
            (RIVER_BRIDGE, 2)
        );
    }

    #[test]
    fn fix_road_connections() {
        for fixed in [false, true] {
            let mut hausnr = vec![[0; 8]; 0x80];
            // Two looks of a 2 × 1 house.
            hausnr[40] = [2, 0, 0, 2, 1, 2, 0, 0];
            let mut w = World {
                tables: std::sync::Arc::new(crate::world::Tables {
                    hausnr,
                    hausdata: vec![[0; 10]; 0x80],
                    ..crate::world::Tables::default()
                }),
                ..World::default()
            };
            w.fixes.set(Fix::RoadConnections, fixed);
            // The second look: parts 2 and 3.
            w.island.buildings.set((20, 20), 40);
            w.island.buildings.set((21, 20), 40);
            w.island.parts.set((20, 20), 2);
            w.island.parts.set((21, 20), 3);
            let mut house = crate::world::House::new((20, 20));
            house.set_building(40);
            house.set_connection(99);
            w.houses = vec![house];
            w.lay_road((22, 20), (22, 25), 2);
            // The original only looks at squares showing part 0.
            assert_eq!(w.houses[0].connection() == 99, !fixed);
        }
    }
}
