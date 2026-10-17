//! The news: messages about what happens on the island (1048:0a54) and the
//! news archive they go into (`*1110:663a`), which the TV
//! ([`crate::fernseher`]) shows and browses. `docs/fernseher.md` has the
//! whole unit.
//!
//! The archive is the world's: `post_message` writes it, the map file
//! saves it (unit 1048:25ea) and the computers' and events' news all go
//! there. The options that say which kinds of news open the TV
//! (`1110:005c`..`005f`) are the player's view's
//! ([`crate::optionen::Settings::news`]; the map file saves them with the
//! view, [`crate::savegame::MapView::news`]): the view decides with
//! [`Posted::opens_tv`] whether a message opens its TV, and asks the world
//! for the pause that goes with it ([`crate::command::Action::ShowTv`]).
//! Single human: one player's TV pauses everyone, as in the original (see
//! `docs/original-bugs.md`, "Design issues for multiplayer", for the
//! intended multiplayer behaviour).

use crate::simulation::Tick;
use crate::world::{Pos, World};

/// A message for the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Message {
    /// The message: its media (`NACHRICH.TXT`'s names) and its kind of
    /// news, which decides whether the TV shows it (`1110:199e`).
    /// // archive +0x12
    pub number: i16,
    /// Which of `NACHRICH.TXT`'s texts it shows: one of the message's three,
    /// `number * 3 + Random(3)`. // archive +0x10
    pub text: i16,
    /// Values the text fills in: a player, another player or a number, an
    /// amount, another amount. // archive +4, +6, +8, +0xc
    pub a: i16,
    pub b: i16,
    pub x: i32,
    pub y: i32,
    /// Where it happened. `None` where the original passes the player's
    /// view origin (`1110:5530`), which the archive then holds. // archive +0x114
    pub pos: Option<Pos>,
    /// Scroll the view to `pos` before showing it.
    pub centre: bool,
}

/// The archive's entries. // 1048:1ffd, 0x1cd8 bytes
pub const ARCHIVE_ENTRIES: usize = 26;
/// An entry's size in the map file. // 1048:0a54
pub const ENTRY_SIZE: usize = 0x11c;
/// An empty entry's day. // 1048:1ffd
pub const NO_DAY: i32 = -1;
/// An empty entry's date. // 1048:1fc5
pub const NO_DATE: &[u8] = b"kein Datum";
/// The message whose TV puts kind 0x46 people on its square's building
/// (1048:12e9): `NACHRICH.TXT`'s 22nd, the announcement of a building's
/// sale ([`crate::auction::BUILDING_SALE`]).
pub const BUILDING_SALE_NEWS: i16 = 0x15;
/// The people 1048:12e9 puts on that building: a picture by the square's
/// form (`docs/people.md`).
const SALE_SIGN: u8 = 0x46;

/// Each message's kind of news, which picks the option (`1110:005c` + kind)
/// that says whether it opens the TV; kind 4 always does, 255 never.
/// // 1110:199e
pub const KINDS: [u8; 90] = [
    0, 0, 0, 255, 2, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 2, 2, 2, 2, //
    2, 2, 2, 2, 255, 255, 255, 0, 0, 4, 0, 0, 0, 4, 4, 3, 3, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 4, 4, 4,
];

/// An entry of the news archive. // *1110:663a, 0x11c bytes
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entry {
    /// The day count when it was posted (`1110:653a`), [`NO_DAY`] for an
    /// empty entry. // +0, Longint
    pub day: i32,
    /// The message's values. // +4, +6, +8, +0xc
    pub a: i16,
    pub b: i16,
    pub x: i32,
    pub y: i32,
    /// `NACHRICH.TXT`'s text, `number * 3 + Random(3)`. // +0x10
    pub text: i16,
    /// The message. // +0x12
    pub number: i16,
    /// The date as the info bar shows it (`date_text`, 1030:0148), a
    /// `string`. // +0x14
    pub date: Vec<u8>,
    /// Where it happened. // +0x114, `i` then `j`
    pub pos: Pos,
    /// The human's view origin when it was posted (`1110:5530`), where the
    /// TV's first button scrolls back to. Single human: one player's view
    /// in the world's archive. // +0x118
    pub origin: Pos,
}

impl Default for Entry {
    /// An empty entry. // 1048:1ffd
    fn default() -> Self {
        Entry {
            day: NO_DAY,
            a: 0,
            b: 0,
            x: 0,
            y: 0,
            text: 0,
            number: 0,
            date: NO_DATE.to_vec(),
            pos: (0, 0),
            origin: (0, 0),
        }
    }
}

fn word_at(r: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([r[at], r[at + 1]])
}

fn longint_at(r: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]])
}

impl Entry {
    /// Whether the entry is empty: its day is -1. // 1048:0a54
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.day == NO_DAY
    }

    /// The entry from its record in the map file.
    #[must_use]
    pub fn from_record(r: &[u8]) -> Option<Entry> {
        if r.len() != ENTRY_SIZE {
            return None;
        }
        let len = usize::from(r[0x14]);
        Some(Entry {
            day: longint_at(r, 0),
            a: word_at(r, 4),
            b: word_at(r, 6),
            x: longint_at(r, 8),
            y: longint_at(r, 0xc),
            text: word_at(r, 0x10),
            number: word_at(r, 0x12),
            date: r[0x15..0x15 + len].to_vec(),
            pos: (word_at(r, 0x114), word_at(r, 0x116)),
            origin: (word_at(r, 0x118), word_at(r, 0x11a)),
        })
    }

    /// The record the map file holds. What follows the date's characters
    /// is whatever memory held in the original; zeros here.
    #[must_use]
    pub fn to_record(&self) -> Vec<u8> {
        let mut r = vec![0; ENTRY_SIZE];
        r[0..4].copy_from_slice(&self.day.to_le_bytes());
        r[4..6].copy_from_slice(&self.a.to_le_bytes());
        r[6..8].copy_from_slice(&self.b.to_le_bytes());
        r[8..0xc].copy_from_slice(&self.x.to_le_bytes());
        r[0xc..0x10].copy_from_slice(&self.y.to_le_bytes());
        r[0x10..0x12].copy_from_slice(&self.text.to_le_bytes());
        r[0x12..0x14].copy_from_slice(&self.number.to_le_bytes());
        let date = &self.date[..self.date.len().min(255)];
        r[0x14] = u8::try_from(date.len()).unwrap_or(u8::MAX);
        r[0x15..0x15 + date.len()].copy_from_slice(date);
        r[0x114..0x116].copy_from_slice(&self.pos.0.to_le_bytes());
        r[0x116..0x118].copy_from_slice(&self.pos.1.to_le_bytes());
        r[0x118..0x11a].copy_from_slice(&self.origin.0.to_le_bytes());
        r[0x11a..0x11c].copy_from_slice(&self.origin.1.to_le_bytes());
        r
    }
}

/// What the date of an entry is written with: `MONATE.TXT` and the
/// country from `MODATYP.DAT` (`date_text`, 1030:0148). They come from the
/// CD, not the map file, like [`crate::world::Tables`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct DateFormat {
    /// `MONATE.TXT`'s twelve. // [1110:65d6]
    pub months: Vec<Vec<u8>>,
    /// `Land` (`MODATYP.DAT`): 0 D, 1 A, 2 E, 3 F. // 1110:64be
    pub land: u8,
}

impl DateFormat {
    fn land(&self) -> formats::data::Land {
        use formats::data::Land;
        match self.land {
            1 => Land::Amerika,
            2 => Land::England,
            3 => Land::Frankreich,
            _ => Land::Deutschland,
        }
    }
}

/// The news archive and what goes with it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct News {
    /// The entries, the oldest first. // *1110:663a
    pub entries: Vec<Entry>,
    /// The entry the last message went into: the last the TV browses to.
    /// // 1110:6664
    pub last: i16,
    /// No message was posted yet: the TV doesn't open. // 1110:667c
    pub nothing_yet: bool,
    /// What the entries' dates are written with. Not in the map file.
    pub date_format: DateFormat,
    /// The TV's speaker, as the view's TV reports it
    /// ([`crate::command::Action::Speaker`]). Not in the map file.
    pub speaker: Speaker,
}

/// The TV's speaker (`MediaPlayer2`, `SPRADEUn\`), which the world waits
/// on: the auction doesn't open while it's open (`1110:662d`,
/// 1028:1940), and while it reads, the TV's lights take random numbers
/// from the world's `RandSeed` each tick (1048:1b3e). The view's TV has
/// the speaker and tells the world. Waits on a view; see
/// `docs/original-bugs.md`'s multiplayer section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Speaker {
    /// No speaker (`1110:662d` = 0).
    #[default]
    Off,
    /// A message's speech is open (`1110:662d` = 1), before it starts
    /// (`1110:667a` = 0) or after it ended (`Position = Length`).
    Open,
    /// It reads: the lights flicker.
    Reading,
}

impl Default for News {
    /// As `init_messages` (1048:1ffd), which each game's set-up runs (state
    /// 1220), leaves it.
    fn default() -> Self {
        News {
            entries: vec![Entry::default(); ARCHIVE_ENTRIES],
            last: 0,
            nothing_yet: true,
            date_format: DateFormat::default(),
            speaker: Speaker::Off,
        }
    }
}

impl News {
    /// The entry `n`, if the archive has it.
    #[must_use]
    pub fn entry(&self, n: i16) -> Option<&Entry> {
        usize::try_from(n).ok().and_then(|n| self.entries.get(n))
    }

    /// The archive's bytes in the map file. // 1048:2524
    #[must_use]
    pub fn archive_bytes(&self) -> Vec<u8> {
        self.entries.iter().flat_map(Entry::to_record).collect()
    }

    /// The archive from its bytes in the map file.
    #[must_use]
    pub fn from_archive_bytes(data: &[u8]) -> Option<Vec<Entry>> {
        if data.len() != ARCHIVE_ENTRIES * ENTRY_SIZE {
            return None;
        }
        data.chunks(ENTRY_SIZE).map(Entry::from_record).collect()
    }
}

/// A message as it went into the archive, for the view: the TV shows the
/// entry as it was written, even if later messages of the same tick move
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posted {
    /// The entry it went into (`1110:6638`, `1110:6664`).
    pub index: i16,
    pub entry: Entry,
    /// Its kind of news ([`KINDS`]), 255 for a message outside the table.
    pub kind: u8,
    /// The message's `centre`, which the TV's first button shows
    /// (`1110:662e`) whether the TV opens or not.
    pub centre: bool,
}

impl Posted {
    /// Whether the player's news options `options` (`1110:005c`..`005f`,
    /// [`crate::optionen::Settings::news`]) open the TV on it (1048:12e9):
    /// kinds 0 to 3 by their option, 4 always, the rest never.
    /// // 1048:0a54
    #[must_use]
    pub fn opens_tv(&self, options: [bool; 4]) -> bool {
        match self.kind {
            0..=3 => options[usize::from(self.kind)],
            4 => true,
            _ => false,
        }
    }

    /// And the view scrolls to its square first (1020:0787): a message to
    /// centre, of kind 0 to 3, whose option is on. // 1048:0a54
    #[must_use]
    pub fn scrolls(&self, options: [bool; 4]) -> bool {
        self.centre && self.kind <= 3 && self.opens_tv(options)
    }
}

impl World {
    /// Posts `message`, picking its text (`Random(3)`); `message.text` is
    /// ignored. // 1048:0a54, 2197 bytes
    ///
    /// The message goes into the first empty entry of the archive; with
    /// none among the first 25, the entries move up one and it goes into
    /// the last. The view gets it in [`Tick::messages`] and [`Tick::news`].
    /// Whether the TV opens on it is the player's options' ([`Posted::opens_tv`]),
    /// so the rest of 1048:12e9 is a command the view's TV sends before the
    /// next tick ([`crate::command::Action::ShowTv`]: the pause and
    /// [`World::tv_people`]), as the original's pause only takes effect
    /// there; the scrolling, the TV's picture and its sounds are the view's.
    pub fn post_message(&mut self, mut message: Message, tick: &mut Tick) {
        let news = &mut self.news;
        news.nothing_yet = false;
        // BUG(original): the first wrap leaves entry 24 empty, so the next message goes there and the 26th and 27th stay in the wrong order for good.
        let mut n = 0;
        let index = loop {
            if news.entries[n].is_empty() || n == ARCHIVE_ENTRIES - 1 {
                if n == ARCHIVE_ENTRIES - 1 {
                    news.entries.remove(0);
                    let last = news.entries[ARCHIVE_ENTRIES - 2].clone();
                    news.entries.push(last);
                }
                break n;
            }
            n += 1;
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // 0..=25
        let index16 = index as i16;
        news.last = index16;
        let variant = self.random.random(3).cast_signed();
        message.text = message.number.wrapping_mul(3).wrapping_add(variant);
        let format = &self.news.date_format;
        let entry = Entry {
            day: self.days,
            a: message.a,
            b: message.b,
            x: message.x,
            y: message.y,
            text: message.text,
            number: message.number,
            date: self.date.text(&format.months, format.land()), // 1030:0148
            pos: message.pos.unwrap_or(self.viewpoint.origin),
            // Single human: the view origin of player 1.
            origin: self.viewpoint.origin,
        };
        self.news.entries[index] = entry.clone();
        // BUG(original): a message outside 0..89 reads its kind past the table; the port opens no TV.
        let kind = usize::try_from(message.number)
            .ok()
            .and_then(|n| KINDS.get(n))
            .copied()
            .unwrap_or(255);
        tick.messages.push(message);
        tick.news.push(Posted {
            index: index16,
            entry,
            kind,
            centre: message.centre,
        });
    }

    /// The world's part of the TV opening on entry `n` (1048:12e9), from
    /// the button bar's third button (1010:2c8b) or for a message whose
    /// kind the player's options show (1048:0a54): the game pauses
    /// (`1110:0067`), and [`World::tv_people`]. Nothing until the first
    /// message was posted (`1110:667c`). Single human: the human's TV
    /// pauses the game for everyone; the simulation waits on a dialog.
    pub(crate) fn tv_shown(&mut self, n: i16, tick: &mut Tick) {
        if self.news.nothing_yet {
            return;
        }
        self.clock.paused = true;
        self.tv_people(n, tick);
    }

    /// The view's TV says what its speaker does ([`Speaker`]).
    pub(crate) fn set_speaker(&mut self, speaker: Speaker) {
        self.news.speaker = speaker;
    }

    /// The TV's lights while its speaker reads, which the original draws
    /// in `fernseher_tick` (1048:1b3e), between the earthquake's damage
    /// and the people, from the game's `RandSeed`: the lower one's frame
    /// `Random(5)`, and one time in `Random(20)` the upper one's
    /// `Random(3) + 1`. The view draws them ([`Tick::tv_lights`]).
    // BUG(original): the TV's lights draw from the game's random numbers, so how long the speaker reads (and the speech option) changes the game.
    pub(crate) fn tv_lights(&mut self, tick: &mut Tick) {
        if self.news.speaker != Speaker::Reading {
            return;
        }
        let lower = self.random.random(5);
        let upper = (self.random.random(20) == 1).then(|| self.random.random(3) + 1);
        tick.tv_lights = Some((lower, upper));
    }

    /// The pause while the TV is open: set by 1048:12e9, cleared by
    /// 1048:077a, which the original does even when the TV wasn't open.
    /// Single human: the simulation waits on the human's dialog.
    pub(crate) fn tv_pause(&mut self, on: bool) {
        self.clock.paused = on;
    }

    /// What opening the TV on entry `n` does to the island (1048:12e9):
    /// the announcement of a building's sale puts kind 0x46 people on
    /// each square of the building on its square.
    pub(crate) fn tv_people(&mut self, n: i16, tick: &mut Tick) {
        let Some(entry) = self.news.entry(n) else {
            return;
        };
        if entry.number != BUILDING_SALE_NEWS {
            return;
        }
        let (i, j) = self.building_origin(entry.pos); // 1030:154c
        let building = self.building_or_none((i, j)); // 1000:3dc9
        let (width, depth) = (self.tables.hausnr(building, 3), self.tables.hausnr(building, 4));
        for e in 0..width {
            for f in 0..depth {
                self.add_person(SALE_SIGN, (i.wrapping_add(e), j.wrapping_add(f)), tick); // 1088:0324
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(number: i16) -> Message {
        Message {
            number,
            a: 1,
            pos: Some((10, 20)),
            ..Message::default()
        }
    }

    #[test]
    fn records() {
        let e = Entry {
            day: 7,
            a: 2,
            b: -3,
            x: 100_000,
            y: -5,
            text: 13,
            number: 4,
            date: b"3. Mai 1996".to_vec(),
            pos: (1, 2),
            origin: (3, 4),
        };
        let r = e.to_record();
        assert_eq!(r.len(), ENTRY_SIZE);
        assert_eq!(Entry::from_record(&r), Some(e));
        let news = News::default();
        let bytes = news.archive_bytes();
        assert_eq!(bytes.len(), 0x1cd8);
        assert_eq!(News::from_archive_bytes(&bytes).unwrap(), news.entries);
    }

    #[test]
    fn the_archive_fills_and_moves_up() {
        let mut w = World::default();
        let mut tick = Tick::default();
        for k in 0..25 {
            w.days = k;
            w.post_message(message(0), &mut tick);
            assert_eq!(w.news.last, i16::try_from(k).unwrap());
        }
        let days = |w: &World| w.news.entries.iter().map(|e| e.day).collect::<Vec<_>>();
        assert_eq!(days(&w)[24..], [24, NO_DAY]);
        // The 26th goes into the last entry, after the others moved up;
        // the 27th into the 25th, which that left empty.
        w.days = 25;
        w.post_message(message(0), &mut tick);
        assert_eq!(w.news.last, 25);
        assert_eq!(days(&w)[..2], [1, 2]);
        assert_eq!(days(&w)[23..], [24, NO_DAY, 25]);
        w.days = 26;
        w.post_message(message(0), &mut tick);
        assert_eq!(w.news.last, 24);
        assert_eq!(days(&w)[23..], [24, 26, 25]);
        w.days = 27;
        w.post_message(message(0), &mut tick);
        assert_eq!(w.news.last, 25);
        assert_eq!(days(&w)[..2], [2, 3]);
        assert_eq!(days(&w)[22..], [24, 26, 25, 27]);
        assert_eq!(tick.news.len(), 28);
        assert!(tick.news.iter().all(|p| !p.opens_tv([false; 4])));
        assert!(!w.clock.paused);
    }

    #[test]
    fn the_options_open_the_tv() {
        let mut w = World::default();
        let mut tick = Tick::default();
        // Kind 0 with its option on, centred: the TV and the scrolling.
        w.post_message(
            Message {
                centre: true,
                ..message(0)
            },
            &mut tick,
        );
        let on = [true; 4];
        assert!(tick.news[0].opens_tv(on) && tick.news[0].scrolls(on));
        // The view's TV pauses the game.
        assert!(!w.clock.paused);
        // Kind 2 with its option off: neither.
        let off2 = [true, true, false, true];
        w.post_message(
            Message {
                centre: true,
                ..message(4)
            },
            &mut tick,
        );
        assert!(!tick.news[1].opens_tv(off2) && !tick.news[1].scrolls(off2) && tick.news[1].centre);
        // Kind 4 always opens it, and never scrolls; 255 never.
        let off = [false; 4];
        w.post_message(
            Message {
                centre: true,
                ..message(5)
            },
            &mut tick,
        );
        assert!(tick.news[2].opens_tv(off) && !tick.news[2].scrolls(on));
        w.post_message(message(3), &mut tick);
        assert!(!tick.news[3].opens_tv(on));
        // Without a position, the view origin.
        w.viewpoint.origin = (77, 88);
        w.post_message(
            Message {
                pos: None,
                ..message(3)
            },
            &mut tick,
        );
        assert_eq!(w.news.entries[4].pos, (77, 88));
        assert_eq!(w.news.entries[4].origin, (77, 88));
        assert_eq!(w.news.entries[0].date, b"1.  1996");
    }

    #[test]
    fn no_tv_before_the_first_message() {
        let mut w = World::default();
        let mut tick = Tick::default();
        w.tv_shown(0, &mut tick);
        assert!(!w.clock.paused);
        w.post_message(message(0), &mut tick);
        w.tv_shown(0, &mut tick);
        assert!(w.clock.paused);
        w.tv_pause(false);
        assert!(!w.clock.paused);
    }
}
