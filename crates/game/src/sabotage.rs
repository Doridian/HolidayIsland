//! Sabotage and the championships (segment `10a0`): ordering sabotage
//! with the sabotage tool (10a0:1f65, from 1028:3d93; the form is
//! [`crate::sabowin`]), and the timed effects `effects_tick` counts down
//! once a day. `docs/sabotage.md` has the whole of it.

use formats::data;

use crate::accounts::BOOK_OTHER;
use crate::construction::{BUILDING_SITE, EMPTY_CHAMPIONSHIPS, FULL_CHAMPIONSHIPS, Record};
use crate::enhance::Fix;
use crate::events::UNSHAKEN;
use crate::news::Message;
use crate::simulation::{Status, Tick};
use crate::world::{Pos, World, on_map};

/// What the world needs of one of `SABOTAGE.TXT`'s kinds of sabotage.
/// // *1110:65bc, 0x232 bytes each
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SabotageKind {
    /// What it costs. // +0x222
    pub cost: i32,
    /// The building types it can be used on, 255 for none; only the
    /// piranhas' are read (pools). // +0x226
    pub targets: [i16; 6],
}

/// The world's part of `SABOTAGE.TXT`'s kinds. // 1028:361f
#[must_use]
pub fn kinds(kinds: &[data::Sabotage]) -> Vec<SabotageKind> {
    kinds
        .iter()
        .map(|k| SabotageKind {
            cost: k.cost,
            targets: k.targets,
        })
        .collect()
}

/// The sabotage kinds, `SABOTAGE.TXT`'s order.
pub const PIRANHAS: u8 = 0;
pub const BOMB: u8 = 1;
pub const STINK_BOMB: u8 = 2;
pub const SHARK_DIVER: u8 = 3;
pub const RATS: u8 = 4;
pub const SOFT_SOAP: u8 = 5;
pub const GO_SLOW: u8 = 6;
pub const CHARACTER_ASSASSINATION: u8 = 7;
pub const ROAD: u8 = 8;

/// The message for sabotage that's found out, by kind. // 10a0:1b31
const CAUGHT_MESSAGES: [i16; 9] = [0x1b, 0x1d, 0x3e, 0x20, 0x3f, 0x40, 0x1e, 0x1f, 0x41];
/// The message for sabotage that failed or found no free slot.
/// // 10a0:1d5e
const FAILED_MESSAGE: i16 = 0x1c;
/// The people kind 10a0:1000 puts on every square of a blown-up building.
const EXPLOSION: u8 = 0xdb;
/// The people kind of the shark diver. // 10a0:19d2
const DIVER: u8 = 3;
/// `HAUSDATA.DAT`'s column 5 for an airport, where soft soap goes.
const AIRPORT: i16 = 9;
/// The sea square a shark diver goes on: form 16 (flat) and ground 2.
const SEA_FORM: u8 = 0x10;
const SEA_GROUND: u8 = 2;
/// The ground of the road pieces the road sabotage takes.
const ROAD_PIECES: std::ops::RangeInclusive<u8> = 10..=0x3a;

impl crate::world::Tables {
    /// Sabotage kind `kind`; past the table a free kind with no targets.
    #[must_use]
    pub fn sabotage_kind(&self, kind: u8) -> SabotageKind {
        // BUG(original): a kind past the nine reads past the table (only a command can ask for one); the port takes a free kind with no targets.
        self.sabotage.get(usize::from(kind)).copied().unwrap_or_default()
    }
}

impl World {
    /// The sabotage tool's click: `player` orders sabotage of `kind` on
    /// the square `at` (the building there, for most kinds) if it's on
    /// the map, not the player's own and, for player 1, paid for, and if
    /// `kind` can be used there ([`World::order_sabotage`]). Else the
    /// error on the status bar (`FEHLER.TXT` 0x16, 0x19, 0xe, 0x1a).
    /// // 10a0:1f65
    pub(crate) fn sabotage_command(&mut self, player: u8, kind: u8, at: Pos, tick: &mut Tick) {
        let mut error = |n| tick.status.push((player, Status::Error(n))); // 1030:232d
        if !on_map(at) {
            return error(0x16);
        }
        if self.island.owners.at(at) == player {
            return error(0x19);
        }
        let cost = self.tables.sabotage_kind(kind).cost;
        // Single human: only player 1's money is checked (the computers never sabotage).
        if player == 1 && self.money_of(1) < cost {
            return error(0xe);
        }
        let building = self.island.buildings.at(at);
        let free = UNSHAKEN.contains(&building); // 1110:1358
        let origin = || self.building_origin(at); // 1030:154c
        let order = match kind {
            PIRANHAS if free => None,
            PIRANHAS => {
                if self.tables.sabotage_kind(kind).targets.contains(&i16::from(building)) {
                    Some((120, origin()))
                } else if self.fixes.has(Fix::SabotagePoolError) {
                    None
                } else {
                    // BUG(original): the loop over the pools always ends at 6, so its check for 5 never shows error 0x1a for a building that isn't a pool: nothing happens.
                    return;
                }
            }
            BOMB if !free => Some((10, origin())),
            STINK_BOMB | RATS if !free => Some((90, origin())),
            SHARK_DIVER if self.island.forms.at(at) == SEA_FORM && self.island.ground.at(at) == SEA_GROUND => {
                Some((120, at))
            }
            SOFT_SOAP if !free && self.tables.hausdata(building, 5) == AIRPORT => Some((90, origin())),
            GO_SLOW if building == BUILDING_SITE => Some((10, at)),
            CHARACTER_ASSASSINATION if !free => Some((10, at)),
            ROAD if ROAD_PIECES.contains(&self.island.ground.at(at)) => Some((10, at)),
            k if k > ROAD => return,
            _ => None,
        };
        match order {
            Some((days, pos)) => self.order_sabotage(days, pos, kind, player, tick), // 10a0:1d5e
            None => error(0x1a),
        }
    }

    /// Whether the sabotage tool's picture shows `kind` as one that can be
    /// used on `at` (the building's first square, or the square): on the
    /// map, not player 1's, with player 1's money, and the kind fits
    /// there, as the click checks it (here the piranhas' check works).
    /// Single human: player 1's. // 1090:0014
    #[must_use]
    pub fn sabotage_fits(&self, kind: u8, at: Pos) -> bool {
        let k = self.tables.sabotage_kind(kind);
        if !on_map(at) || self.island.owners.at(at) == 1 || k.cost > self.money_of(1) {
            return false;
        }
        let building = self.island.buildings.at(at);
        let free = UNSHAKEN.contains(&building);
        match kind {
            PIRANHAS => !free && k.targets.contains(&i16::from(building)),
            BOMB | STINK_BOMB | RATS | CHARACTER_ASSASSINATION => !free,
            SHARK_DIVER => self.island.forms.at(at) == SEA_FORM && self.island.ground.at(at) == SEA_GROUND,
            SOFT_SOAP => !free && self.tables.hausdata(building, 5) == AIRPORT,
            GO_SLOW => building == BUILDING_SITE,
            ROAD => ROAD_PIECES.contains(&self.island.ground.at(at)),
            _ => false,
        }
    }

    /// `player` pays for sabotage of `kind` on `pos` (player 1 into the
    /// books' "other", the money rising from `pos`). Then 30 % of the time
    /// it fails (`Random(100) < 30`): half of those quietly (message
    /// 0x1c), the other half found out ([`World::sabotage_caught`]; the
    /// second `Random(100) < 50`). Else it takes the first free slot, with
    /// the owner of `pos` as the victim and `days` to run, and has its
    /// first effect ([`World::sabotage_starts`]); with no free slot it
    /// fails (0x1c). // 10a0:1d5e
    fn order_sabotage(&mut self, days: u16, pos: Pos, kind: u8, player: u8, tick: &mut Tick) {
        let cost = self.tables.sabotage_kind(kind).cost;
        self.add_money(player, cost.wrapping_neg()); // 1078:01e7
        // Single human: only player 1 has books and sees the money rise.
        if player == 1 {
            self.book(BOOK_OTHER, cost.wrapping_neg()); // 1070:1419
            tick.books = true; // 1070:2617
            self.float_money(cost, pos); // 1088:0153
        }
        let victim = self.island.owners.at(pos);
        if self.random.random(100) < 30 {
            if self.random.random(100) < 50 {
                self.post_message(self.sabotage_message(FAILED_MESSAGE, pos, player), tick);
            } else {
                self.sabotage_caught(pos, kind, player, tick); // 10a0:1b31
            }
            return;
        }
        match self.sabotage.iter_mut().find(|s| !s.active) {
            Some(slot) => {
                *slot = Sabotage {
                    attacker: player,
                    victim,
                    kind,
                    pos,
                    countdown: days,
                    active: true,
                };
                self.sabotage_starts(pos, kind, player, tick); // 10a0:19d2
            }
            None => self.post_message(self.sabotage_message(FAILED_MESSAGE, pos, player), tick),
        }
    }

    /// The sabotage was found out: `player`'s reputation goes down by 50
    /// (to 0 at the least) and the news says so. // 10a0:1b31
    fn sabotage_caught(&mut self, pos: Pos, kind: u8, player: u8, tick: &mut Tick) {
        let r = self.reputation(player);
        self.set_reputation(player, r.saturating_sub(50));
        let number = CAUGHT_MESSAGES[usize::from(kind)];
        self.post_message(self.sabotage_message(number, pos, player), tick);
    }

    /// The news about sabotage on `pos` by `player`, against its owner.
    fn sabotage_message(&self, number: i16, pos: Pos, player: u8) -> Message {
        Message {
            number,
            a: i16::from(player),
            b: i16::from(self.island.owners.at(pos)),
            pos: Some(pos),
            ..Message::default()
        }
    }

    /// The sabotage's first effect, the day it's ordered: most kinds are
    /// in the news, the bomb and the road blow up what's on `pos`, the
    /// shark diver goes into the water, soft soap waits for a plane.
    /// // 10a0:19d2
    fn sabotage_starts(&mut self, pos: Pos, kind: u8, player: u8, tick: &mut Tick) {
        let number = match kind {
            PIRANHAS => 0x21,
            STINK_BOMB => 0x2f,
            RATS => 0x30,
            GO_SLOW => 0x18,
            CHARACTER_ASSASSINATION => 0x19,
            BOMB | ROAD => return self.blow_up(pos, tick), // 10a0:1000
            SHARK_DIVER => return self.add_person(DIVER, pos, tick), // 1088:0324
            _ => return,
        };
        self.post_message(self.sabotage_message(number, pos, player), tick);
    }

    /// The bomb (or the road sabotage) goes off on `pos`: the people there
    /// leave, what's there is demolished for no one, and every square of
    /// the building that was there gets an explosion (people kind 0xdb).
    /// // 10a0:1000
    fn blow_up(&mut self, pos: Pos, tick: &mut Tick) {
        let building = self.island.buildings.at(pos);
        self.remove_people_from(pos, tick); // 1088:6975
        // For no one, so it never refuses.
        let _ = self.demolish(0, pos, tick); // 1078:280b
        self.people_map_changed(); // 1088:6c5a
        if !UNSHAKEN.contains(&building) {
            let (w, d) = (self.tables.hausnr(building, 3), self.tables.hausnr(building, 4));
            for a in 0..w {
                for c in 0..d {
                    self.add_person(EXPLOSION, (pos.0 + a, pos.1 + c), tick); // 1088:0324
                }
            }
        }
    }
}

/// One of the five sabotage slots. // *1110:705a, 10 bytes each
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Sabotage {
    /// The player who did it, for the message. // +0
    pub attacker: u8,
    /// The player it's aimed at. // +1
    pub victim: u8,
    /// `SABOTAGE.TXT`'s kind: 0 piranhas, 1 a bomb, 2 a stink bomb, 3 a
    /// shark diver, 4 rats, 5 soft soap, 6 a go-slow on a building site,
    /// 7 character assassination, 8 a road. // +2
    pub kind: u8,
    /// Where. // +3, +5
    pub pos: Pos,
    /// Days left. // +7
    pub countdown: u16,
    /// // +9
    pub active: bool,
}

impl Sabotage {
    fn parse(r: &[u8]) -> Self {
        Sabotage {
            attacker: r[0],
            victim: r[1],
            kind: r[2],
            pos: (word(r, 3), word(r, 5)),
            countdown: word(r, 7).cast_unsigned(),
            active: r[9] != 0,
        }
    }

    /// The record as the map file stores it.
    pub(crate) fn record(&self) -> [u8; 10] {
        let mut r = [0; 10];
        r[0] = self.attacker;
        r[1] = self.victim;
        r[2] = self.kind;
        r[3..5].copy_from_slice(&self.pos.0.to_le_bytes());
        r[5..7].copy_from_slice(&self.pos.1.to_le_bytes());
        r[7..9].copy_from_slice(&self.countdown.to_le_bytes());
        r[9] = u8::from(self.active);
        r
    }
}

/// The three championships: tennis (0), surfing (1) and Miss Holiday
/// Island (2), each on its building. // *1110:705e, 8 bytes each
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Championship {
    /// Which one. // +0
    pub kind: u8,
    /// The building. // +1, +3
    pub pos: Pos,
    /// Days left. // +5
    pub countdown: u16,
    /// // +7
    pub active: bool,
}

impl Championship {
    fn parse(r: &[u8]) -> Self {
        Championship {
            kind: r[0],
            pos: (word(r, 1), word(r, 3)),
            countdown: word(r, 5).cast_unsigned(),
            active: r[7] != 0,
        }
    }

    /// The record as the map file stores it.
    pub(crate) fn record(self) -> [u8; 8] {
        let mut r = [0; 8];
        r[0] = self.kind;
        r[1..3].copy_from_slice(&self.pos.0.to_le_bytes());
        r[3..5].copy_from_slice(&self.pos.1.to_le_bytes());
        r[5..7].copy_from_slice(&self.countdown.to_le_bytes());
        r[7] = u8::from(self.active);
        r
    }
}

fn word(r: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([r[at], r[at + 1]])
}

/// The sabotage slots from the map file's `*1110:705a` (unit 10a0:23c9).
#[must_use]
pub fn parse_sabotage(data: &[u8]) -> Option<[Sabotage; 5]> {
    (data.len() == 5 * 10).then(|| std::array::from_fn(|n| Sabotage::parse(&data[10 * n..])))
}

/// The championships from the map file's `*1110:705e` (unit 10a0:23c9).
#[must_use]
pub fn parse_championships(data: &[u8]) -> Option<[Championship; 3]> {
    (data.len() == 3 * 8).then(|| std::array::from_fn(|n| Championship::parse(&data[8 * n..])))
}

/// The tennis championship's building, empty and full. // 10a0:117e
const TENNIS: [u8; 2] = [0x6f, 0x70];
/// The surfing championship's buildings: four empty, four full.
const SURFING: std::ops::RangeInclusive<u8> = 0x71..=0x78;
/// Miss Holiday Island's: two empty, two full.
const MISS: std::ops::RangeInclusive<u8> = 0x79..=0x7c;

impl World {
    /// Each active sabotage and championship moves on a day: on certain
    /// days of its countdown it has an effect, then the countdown goes down
    /// by one, and one that was at 0 ends. // 10a0:117e
    pub(crate) fn effects_tick(&mut self, tick: &mut Tick) {
        for n in 0..self.sabotage.len() {
            let s = self.sabotage[n];
            if s.active {
                self.sabotage_day(n, tick);
                let s = &mut self.sabotage[n];
                if s.countdown == 0 {
                    s.active = false;
                } else {
                    s.countdown -= 1;
                }
            }
        }
        for n in 0..self.championships.len() {
            let c = self.championships[n];
            if c.active {
                self.championship_day(c, tick);
                let c = &mut self.championships[n];
                if c.countdown == 0 {
                    c.active = false;
                } else {
                    c.countdown -= 1;
                }
            }
        }
    }

    /// What sabotage slot `n` does on the day its countdown is at.
    /// // 10a0:117e
    fn sabotage_day(&mut self, n: usize, tick: &mut Tick) {
        let s = self.sabotage[n];
        let message = |number| Message {
            number,
            a: i16::from(s.attacker),
            b: i16::from(s.victim),
            pos: Some(s.pos),
            ..Message::default()
        };
        match (s.kind, s.countdown) {
            // Piranhas, a stink bomb, rats: the place is worse for a while.
            (0, 120) | (2 | 4, 90) => self.change_location(s.pos, 60, 1, false), // 1030:185d
            (0 | 2 | 4, 0) => self.change_location(s.pos, 60, 1, true),
            (1, 9) => self.post_message(message(0x17), tick),
            // The shark diver: a wider area.
            (3, 119) => self.post_message(Message { b: 0, ..message(4) }, tick),
            (3, 118) => self.change_location(s.pos, 60, 10, false),
            (3, 0) => self.change_location(s.pos, 60, 10, true),
            // Soft soap: the message would be on the day after the 2nd, but
            // the countdown goes from 2 straight to 0.
            (5, 2) => self.sabotage[n].countdown = 1,
            (5, 1) => self.post_message(Message { b: 0, ..message(0x42) }, tick),
            (6, 10) => {
                if self.island.buildings.get(s.pos) == Some(BUILDING_SITE) {
                    self.delay_construction(s.pos);
                } else {
                    self.sabotage[n].countdown = 0;
                }
            }
            (7, 10) => {
                let r = self.reputation(s.victim);
                self.set_reputation(s.victim, r.saturating_sub(50));
            }
            _ => {}
        }
    }

    /// What a championship does on the day its countdown is at, if its
    /// building is still there: the building fills, the place gets
    /// better, the owner's reputation goes up, the news reports it, and at
    /// the end it all goes back. // 10a0:117e
    fn championship_day(&mut self, c: Championship, tick: &mut Tick) {
        let Some(building) = self.island.buildings.get(c.pos) else {
            return;
        };
        let owner = self.island.owners.get(c.pos).unwrap_or(0);
        let message = |number| Message {
            number,
            a: i16::from(owner),
            pos: Some(c.pos),
            centre: true,
            ..Message::default()
        };
        match c.kind {
            0 if TENNIS.contains(&building) => match c.countdown {
                0x37 => self.replace_building(TENNIS[1], c.pos, tick),
                0x36 => self.change_location(c.pos, 160, 1, false),
                0x35 => self.raise_reputation(owner, 50),
                0x34 => self.post_message(message(0x58), tick),
                2 => self.change_location(c.pos, 160, 1, true),
                1 => self.replace_building(TENNIS[0], c.pos, tick),
                _ => {}
            },
            1 if SURFING.contains(&building) => match c.countdown {
                0x38 => self.replace_building(building + 4, c.pos, tick),
                0x37 => self.change_location(c.pos, 160, 1, false),
                0x31..=0x36 | 0x24 | 0x1e | 0x14 | 10 => self.send_surfers(c.pos, tick), // 10a0:10c6
                0x2b => self.raise_reputation(owner, 50),
                0x2a => self.post_message(message(0x57), tick),
                3 => self.change_location(c.pos, 160, 1, true),
                2 => {
                    let _ = self.remove_people_at(c.pos); // 1088:2d2a
                }
                1 => self.replace_building(building.wrapping_sub(4), c.pos, tick),
                _ => {}
            },
            2 if MISS.contains(&building) => match c.countdown {
                0x19 => self.replace_building(building + 2, c.pos, tick),
                0x18 => self.change_location(c.pos, 160, 1, false),
                0x17 => self.raise_reputation(owner, 30),
                0x14 => self.post_message(message(0x59), tick),
                2 => self.change_location(c.pos, 160, 1, true),
                1 => self.replace_building(building.wrapping_sub(2), c.pos, tick),
                _ => {}
            },
            _ => {}
        }
    }

    /// Raises `player`'s reputation by `by`, up to 100. // 10a0:117e
    fn raise_reputation(&mut self, player: u8, by: u8) {
        let r = self.reputation(player);
        self.set_reputation(player, r.saturating_add(by).min(100));
    }

    /// The building site at `pos` takes as long again as its type's
    /// building time (`HAUSDATA.DAT`'s first column) more.
    /// // 10a0:117e, sabotage kind 6
    fn delay_construction(&mut self, pos: Pos) {
        // BUG(original): a square no site's area holds is read through a nil pointer; skipped.
        if let Some(n) = self.find_building_site(pos) {
            let site = &mut self.building_sites[n];
            site.days = site.days.wrapping_add(self.tables.hausdata(site.building, 0));
        }
    }

    /// A finished championship building (0x6f–0x7c) takes the first free
    /// championship slot, if there is one. The empty buildings set its kind
    /// and days: tennis (0x6f) and surfing (0x71–0x74) 60, Miss Holiday
    /// Island (0x79, 0x7a) 30; the full ones (finished as such only from a
    /// map) take the slot with the kind and days it had before.
    /// // 10a0:02ad
    pub(crate) fn start_championship(&mut self, building: u8, pos: Pos) {
        let Some(c) = self.championships.iter_mut().find(|c| !c.active) else {
            return;
        };
        c.pos = pos;
        c.active = true;
        match building {
            0x6f => (c.kind, c.countdown) = (0, 0x3c),
            0x71..=0x74 => (c.kind, c.countdown) = (1, 0x3c),
            0x79 | 0x7a => (c.kind, c.countdown) = (2, 0x1e),
            _ => {}
        }
    }
}

impl World {
    /// A championship's building at `pos` becomes `building`, a full or
    /// an empty one: the spectators of an emptied one leave, the type goes
    /// on the new one's area (its size from `HAUSNR.DAT`) and into its
    /// record (1078:169a). A full one's record gets the plot's `+0xee`
    /// (spectators?), at most its capacity (`HAUSDATA.DAT` column 3), as
    /// [`Facility::visitors`](crate::world::Facility::visitors) and that in
    /// percent of the capacity as how full it is; an empty one 0 for both.
    /// // 10a0:0026
    ///
    /// The original writes through a nil pointer when the building has no
    /// record, and would write those fields into a house's record; the
    /// buildings are facilities, and the port leaves the records alone
    /// otherwise.
    pub(crate) fn replace_building(&mut self, building: u8, pos: Pos, tick: &mut Tick) {
        // BUG(original): a building with no record is written through a nil pointer; the port leaves the records alone.
        self.send_spectators_away(building); // 10a0:0026
        let record = self.find_record(pos); // 1078:169a
        let t = std::sync::Arc::clone(&self.tables);
        // Byte loops, as in place_building.
        let (i0, j0) = (pos.0.to_le_bytes()[0], pos.1.to_le_bytes()[0]);
        let i1 = t.hausnr(building, 3).wrapping_add(pos.0).wrapping_sub(1).to_le_bytes()[0];
        let j1 = t.hausnr(building, 4).wrapping_add(pos.1).wrapping_sub(1).to_le_bytes()[0];
        for i in i0..=i1 {
            for j in j0..=j1 {
                let square = (i16::from(i), i16::from(j));
                if crate::world::on_map(square) {
                    self.island.buildings.set(square, building);
                    tick.redraw.push(square); // 1050:21f9
                }
            }
        }
        match record {
            Some(Record::House(n)) => self.houses[n].set_building(building),
            Some(Record::Facility(n)) => {
                self.facilities[n].set_building(building);
                if FULL_CHAMPIONSHIPS.contains(&building) {
                    let capacity = i32::from(t.hausdata(building, 3));
                    let at = self.facilities[n].pos();
                    let spectators = self.plot_of(at).map_or(0, |p| p.longint(0xee)).min(capacity);
                    let percent = spectators.wrapping_mul(100).checked_div(capacity).unwrap_or(0);
                    let f = &mut self.facilities[n];
                    f.set_visitors(spectators);
                    f.set_fill(percent.to_le_bytes()[0]);
                } else if EMPTY_CHAMPIONSHIPS.contains(&building) {
                    let f = &mut self.facilities[n];
                    f.set_visitors(0);
                    f.set_fill(0);
                }
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use vcl::random::Random;

    use super::*;
    use crate::world::LocationMap;

    fn sabotage(kind: u8, countdown: u16) -> Sabotage {
        Sabotage {
            attacker: 2,
            victim: 1,
            kind,
            pos: (100, 60),
            countdown,
            active: true,
        }
    }

    /// Runs `effects_tick` `days` times and returns the messages.
    fn days(w: &mut World, days: usize) -> Vec<Message> {
        let mut tick = Tick::default();
        for _ in 0..days {
            w.effects_tick(&mut tick);
        }
        tick.messages
    }

    #[test]
    fn countdown() {
        let mut w = World::default();
        w.sabotage[3] = sabotage(8, 2);
        days(&mut w, 2);
        assert_eq!(
            w.sabotage[3],
            Sabotage {
                countdown: 0,
                ..sabotage(8, 2)
            }
        );
        days(&mut w, 1);
        assert!(!w.sabotage[3].active);
        assert_eq!(w.sabotage[3].countdown, 0);
    }

    #[test]
    fn piranhas() {
        let mut w = World::default();
        w.sabotage[0] = sabotage(0, 121);
        days(&mut w, 1);
        assert_eq!(w.island.location, LocationMap::filled(100));
        days(&mut w, 1);
        assert_eq!(w.island.location.get((50, 30)), Some(60));
        assert_eq!(w.island.location.get((49, 30)), Some(80));
        days(&mut w, 120);
        assert_eq!(w.island.location, LocationMap::filled(100));
        assert!(!w.sabotage[0].active);
    }

    #[test]
    fn character_assassination() {
        let mut w = World::default();
        w.set_reputation(1, 70);
        w.sabotage[0] = sabotage(7, 10);
        w.sabotage[1] = sabotage(7, 10);
        days(&mut w, 1);
        assert_eq!(w.reputation(1), 0);
        assert_eq!(w.player_block[0x30], 0);
    }

    #[test]
    fn messages() {
        let mut w = World {
            random: Random::new(7),
            ..World::default()
        };
        w.sabotage[0] = sabotage(1, 9);
        // Soft soap never gets to its message.
        w.sabotage[1] = sabotage(5, 2);
        let messages = days(&mut w, 3);
        let mut r = Random::new(7);
        let text = 0x17 * 3 + r.random(3).cast_signed();
        assert_eq!(
            messages,
            [Message {
                number: 0x17,
                text,
                a: 2,
                b: 1,
                pos: Some((100, 60)),
                ..Message::default()
            }]
        );
        assert_eq!(w.random, r);
        assert!(!w.sabotage[1].active);
    }

    #[test]
    fn go_slow_without_a_site() {
        let mut w = World::default();
        w.sabotage[0] = sabotage(6, 10);
        days(&mut w, 1);
        assert!(!w.sabotage[0].active);
    }

    #[test]
    fn go_slow() {
        let mut hausnr = vec![[0; 8]; 41];
        hausnr[40][3..5].copy_from_slice(&[2, 2]);
        let mut hausdata = vec![[0; 10]; 41];
        hausdata[40][0] = 30;
        let mut w = World {
            tables: std::sync::Arc::new(crate::world::Tables {
                hausnr,
                hausdata,
                hausprei: Vec::new(),
                groundnr: Vec::new(),
                ..crate::world::Tables::default()
            }),
            ..World::default()
        };
        w.building_sites.push(crate::world::BuildingSite {
            pos: (99, 60),
            days: 5,
            building: 40,
            ..Default::default()
        });
        w.island.buildings.set((100, 60), BUILDING_SITE);
        w.sabotage[0] = sabotage(6, 10);
        days(&mut w, 1);
        assert_eq!(w.building_sites[0].days, 35);
        assert_eq!(w.sabotage[0].countdown, 9);
    }

    #[test]
    fn replacing_fills_the_record() {
        let mut hausnr = vec![[0; 8]; 0x71];
        hausnr[0x6f] = [1, 0, 0, 3, 3, 9, 0, 0];
        hausnr[0x70] = [1, 0, 0, 3, 3, 9, 0, 0];
        let mut hausdata = vec![[0; 10]; 0x71];
        hausdata[0x6f][5] = 5;
        hausdata[0x70][3] = 5000;
        hausdata[0x70][5] = 5;
        let mut w = World {
            tables: std::sync::Arc::new(crate::world::Tables {
                hausnr,
                hausdata,
                hausprei: Vec::new(),
                groundnr: Vec::new(),
                ..crate::world::Tables::default()
            }),
            ..World::default()
        };
        let pos = (40, 40);
        for i in 40..43 {
            for j in 40..43 {
                w.island.buildings.set((i, j), 0x6f);
                w.island.plot_map.set((i, j), 2);
            }
        }
        let mut plot = crate::world::Plot::new(2);
        plot.set_longint(0xee, 3000);
        w.plots.push(plot);
        w.facilities.push(crate::world::Facility::new(0x6f, pos));
        let mut tick = Tick::default();
        w.replace_building(0x70, pos, &mut tick);
        assert_eq!(w.island.buildings.at((42, 42)), 0x70);
        assert_eq!(tick.redraw.len(), 9);
        let f = &w.facilities[0];
        assert_eq!((f.building(), f.visitors(), f.fill()), (0x70, 3000, 60));
        w.replace_building(0x6f, pos, &mut tick);
        let f = &w.facilities[0];
        assert_eq!((f.building(), f.visitors(), f.fill()), (0x6f, 0, 0));
        assert_eq!(w.island.buildings.at((40, 41)), 0x6f);
    }

    #[test]
    fn fix_sabotage_pool_error() {
        for fixed in [false, true] {
            let mut w = World::default();
            w.fixes.set(Fix::SabotagePoolError, fixed);
            w.island.buildings.set((40, 40), 5);
            let mut tick = Tick::default();
            // Piranhas for a building that isn't a pool: the original says
            // nothing.
            w.sabotage_command(2, PIRANHAS, (40, 40), &mut tick);
            let error = (2, crate::simulation::Status::Error(0x1a));
            assert_eq!(tick.status, if fixed { vec![error] } else { vec![] });
            assert!(w.sabotage.iter().all(|s| !s.active));
        }
    }

    #[test]
    fn tennis() {
        let mut w = World::default();
        let pos = (40, 40);
        w.island.buildings.set(pos, TENNIS[0]);
        w.island.owners.set(pos, 3);
        w.set_reputation(3, 60);
        w.championships[1] = Championship {
            kind: 0,
            pos,
            countdown: 0x36,
            active: true,
        };
        let messages = days(&mut w, 3);
        assert_eq!(w.island.location.get((20, 20)), Some(160));
        assert_eq!(w.reputation(3), 100);
        assert_eq!(messages.len(), 1);
        assert_eq!((messages[0].number, messages[0].a, messages[0].centre), (0x58, 3, true));
        days(&mut w, 0x34 - 2);
        // Up to rounding: 160 % * 63 % in the middle.
        let mut back = LocationMap::filled(100);
        back.set((20, 20), 101);
        assert_eq!(w.island.location, back);
        // Without the building nothing happens, but the days still count.
        w.island.buildings.set(pos, 1);
        w.championships[1].countdown = 0x34;
        assert_eq!(days(&mut w, 1), []);
        assert_eq!(w.championships[1].countdown, 0x33);
    }
}
