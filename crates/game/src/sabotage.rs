//! Sabotage and the championships: the timed effects `effects_tick`
//! counts down once a day (segment `10a0`).

use crate::construction::{BUILDING_SITE, EMPTY_CHAMPIONSHIPS, FULL_CHAMPIONSHIPS, Record};
use crate::news::Message;
use crate::simulation::Tick;
use crate::world::{Pos, World};

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
        // The original would read through a nil pointer if no site's area
        // held the square.
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
    /// [`Facility::value_c`](crate::world::Facility::value_c) and that in
    /// percent of the capacity as how full it is; an empty one 0 for both.
    /// // 10a0:0026
    ///
    /// The original writes through a nil pointer when the building has no
    /// record, and would write those fields into a house's record; the
    /// buildings are facilities, and the port leaves the records alone
    /// otherwise.
    pub(crate) fn replace_building(&mut self, building: u8, pos: Pos, tick: &mut Tick) {
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
                    f.set_value_c(spectators);
                    f.set_fill(percent.to_le_bytes()[0]);
                } else if EMPTY_CHAMPIONSHIPS.contains(&building) {
                    let f = &mut self.facilities[n];
                    f.set_value_c(0);
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
        assert_eq!((f.building(), f.value_c(), f.fill()), (0x70, 3000, 60));
        w.replace_building(0x6f, pos, &mut tick);
        let f = &w.facilities[0];
        assert_eq!((f.building(), f.value_c(), f.fill()), (0x6f, 0, 0));
        assert_eq!(w.island.buildings.at((40, 41)), 0x6f);
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
