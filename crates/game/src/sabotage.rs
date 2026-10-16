//! Sabotage and the championships: the timed effects `effects_tick`
//! counts down once a day (segment `10a0`).

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

/// The building site's type. // 1078:1995
const BUILDING_SITE: u8 = 31;
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
                0x31..=0x36 | 0x24 | 0x1e | 0x14 | 10 => self.send_surfers(c.pos),
                0x2b => self.raise_reputation(owner, 50),
                0x2a => self.post_message(message(0x57), tick),
                3 => self.change_location(c.pos, 160, 1, true),
                2 => self.remove_people_at(c.pos),
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
}

/// The parts of the effects that aren't ported yet. Where the original
/// draws random numbers in them, the port's numbers part from the
/// original's once they run.
#[allow(clippy::unused_self)]
impl World {
    /// The building site at `pos` takes as long again as its type's
    /// building time (`HAUSDATA.DAT`'s first column) more: 1078:1995 finds
    /// it in the building sites (`*1110:6f08`, 13-byte records: `+1`, `+3`
    /// the square, `+5` the days left, `+7` the type, `+9` the next).
    /// // 10a0:117e, sabotage kind 6
    fn delay_construction(&mut self, pos: Pos) {
        // TODO: port once the building sites (`*1110:6f08`) are loaded.
        let _ = pos;
    }

    /// Puts building type `building` on the area of the one at `pos` (its
    /// size from `HAUSNR.DAT`), updates its record (1078:169a) and, for
    /// some types, sends the people inside away (1088:2df2). // 10a0:0026,
    /// 623 bytes
    fn replace_building(&mut self, building: u8, pos: Pos, tick: &mut Tick) {
        // TODO: port 10a0:0026 once the people (`*1110:6fd6`) and the
        // building records (1078:169a) are.
        let _ = (building, pos, tick);
    }

    /// Five tries to send a surfer from the beach at `pos` into the sea
    /// (1010:20e0, `Random(2)`, 1088:2c04), counted in `layer_551c`.
    /// // 10a0:10c6, 168 bytes
    fn send_surfers(&mut self, pos: Pos) {
        // TODO: port 10a0:10c6 with the people.
        let _ = pos;
    }

    /// Takes the people at `pos` off the island. // 1088:2d2a, 200 bytes
    fn remove_people_at(&mut self, pos: Pos) {
        // TODO: port 1088:2d2a with the people.
        let _ = pos;
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
