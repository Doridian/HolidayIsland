//! The wire format: the session's messages as bytes, written by hand so
//! that the same message is always the same bytes, on every target.
//!
//! A message is the protocol's version (`u16`), a tag byte and the
//! message's fields. Integers are little-endian and fixed-size; a list
//! or a byte string is its length (`u32`) and then its items; an
//! `Option` is a byte 0 or 1 and then the value; text is UTF-8 bytes. A
//! world's fix goes by its key (`fix.view_people`'s `view_people`), a
//! saved game (the snapshot, `Action::Load`) as the file's bytes.
//!
//! Under the messages, a frame: one byte, then a piece of a message. 0 is
//! a whole message, 1 a piece with more to come, 2 the last piece. A
//! transport takes messages up to some size ([`FRAME`] is safe for every
//! WebRTC implementation); the snapshot is bigger.

use std::sync::Arc;

use formats::savegame::SaveGame;
use game::auction;
use game::command::Action;
use game::enhance::Fix;
use game::geldmenu;
use game::news::Speaker;
use game::optionen::WorldOption;
use game::people::Viewpoint;
use game::seats::{Client, ClientCommand, SeatChange};

/// The protocol's version. Peers of other versions don't play together:
/// the host refuses them.
/// Version 2: the snapshot carries a hash per game file
/// (`crate::hash::data_hashes`) and the end says whether the host quit
/// ([`Ending`]).
pub const VERSION: u16 = 2;

/// The biggest frame the session sends: 16 KiB, which browsers' data
/// channels all take.
pub const FRAME: usize = 16 * 1024;

/// A message between the host and a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Client → host, first: the seat it wants (none: any computer's) or
    /// the client it was, to come back as.
    Hello { seat: Option<u8>, rejoin: Option<Client> },
    /// Host → client: its id and seat. The snapshot follows once the
    /// seat change is due.
    Welcome { client: Client, seat: u8 },
    /// Host → client: it can't play, and why.
    Reject(String),
    /// Client → host: what the client does.
    Command(Action),
    /// Host → client: the world before tick `tick`, as a saved game, and
    /// the hashes of what the host's world took from its game files
    /// (`crate::hash::data_hashes`).
    Snapshot { tick: u64, data: Vec<u64>, save: Vec<u8> },
    /// Host → clients: the go for `tick`, with the commands to apply
    /// before it, in order (none is a go too).
    Go { tick: u64, commands: Vec<ClientCommand> },
    /// Either way: the world's hash after tick `tick`.
    Hash { tick: u64, hash: u64 },
    /// Client → host: leaving for good.
    Bye,
    /// Host → clients: the game is over for the clients, and why.
    End(Ending),
}

/// Why the host ended the game for its clients ([`Message::End`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// The host's player ended their game on purpose: back to the start
    /// menu, or out of the program. A client tells it from a lost host.
    Quit,
    /// Something else, with why.
    Other(String),
}

/// Why bytes didn't read as a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Another version of the protocol.
    Version(u16),
    /// Not a message of this version.
    Malformed,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Version(v) => write!(f, "protocol version {v}, this game speaks {VERSION}"),
            Error::Malformed => write!(f, "a malformed message"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("a message under 4 GiB"));
    }
    fn bytes(&mut self, b: &[u8]) {
        self.len(b.len());
        self.0.extend_from_slice(b);
    }
    fn opt_u8(&mut self, v: Option<u8>) {
        self.bool(v.is_some());
        self.u8(v.unwrap_or(0));
    }
    fn opt_u16(&mut self, v: Option<u16>) {
        self.bool(v.is_some());
        self.u16(v.unwrap_or(0));
    }
    fn pos(&mut self, (i, j): (i16, i16)) {
        self.i16(i);
        self.i16(j);
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        if self.0.len() < n {
            return Err(Error::Malformed);
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let mut a = [0; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, Error> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Malformed),
        }
    }
    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn i16(&mut self) -> Result<i16, Error> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn len(&mut self) -> Result<usize, Error> {
        let n = usize::try_from(self.u32()?).map_err(|_| Error::Malformed)?;
        // No list item is less than a byte: a length past the end is bad.
        if n > self.0.len() {
            return Err(Error::Malformed);
        }
        Ok(n)
    }
    fn bytes(&mut self) -> Result<Vec<u8>, Error> {
        let n = self.len()?;
        Ok(self.take(n)?.to_vec())
    }
    fn text(&mut self) -> Result<String, Error> {
        String::from_utf8(self.bytes()?).map_err(|_| Error::Malformed)
    }
    fn opt_u8(&mut self) -> Result<Option<u8>, Error> {
        let some = self.bool()?;
        let v = self.u8()?;
        Ok(some.then_some(v))
    }
    fn opt_u16(&mut self) -> Result<Option<u16>, Error> {
        let some = self.bool()?;
        let v = self.u16()?;
        Ok(some.then_some(v))
    }
    fn pos(&mut self) -> Result<(i16, i16), Error> {
        Ok((self.i16()?, self.i16()?))
    }
}

/// `message` as bytes.
#[must_use]
pub fn encode(message: &Message) -> Vec<u8> {
    let mut w = Writer::default();
    w.u16(VERSION);
    match message {
        Message::Hello { seat, rejoin } => {
            w.u8(0);
            w.opt_u8(*seat);
            w.opt_u16(*rejoin);
        }
        Message::Welcome { client, seat } => {
            w.u8(1);
            w.u16(*client);
            w.u8(*seat);
        }
        Message::Reject(why) => {
            w.u8(2);
            w.bytes(why.as_bytes());
        }
        Message::Command(action) => {
            w.u8(3);
            action_to(&mut w, action);
        }
        Message::Snapshot { tick, data, save } => {
            w.u8(4);
            w.u64(*tick);
            w.len(data.len());
            for &d in data {
                w.u64(d);
            }
            w.bytes(save);
        }
        Message::Go { tick, commands } => {
            w.u8(5);
            w.u64(*tick);
            w.len(commands.len());
            for c in commands {
                w.u16(c.client);
                action_to(&mut w, &c.action);
            }
        }
        Message::Hash { tick, hash } => {
            w.u8(6);
            w.u64(*tick);
            w.u64(*hash);
        }
        Message::Bye => w.u8(7),
        Message::End(ending) => {
            w.u8(8);
            match ending {
                Ending::Quit => w.u8(0),
                Ending::Other(why) => {
                    w.u8(1);
                    w.bytes(why.as_bytes());
                }
            }
        }
    }
    w.0
}

/// The message `bytes` hold.
///
/// # Errors
/// If they're of another version, or aren't a message.
pub fn decode(bytes: &[u8]) -> Result<Message, Error> {
    let mut r = Reader(bytes);
    let version = r.u16()?;
    if version != VERSION {
        return Err(Error::Version(version));
    }
    let message = match r.u8()? {
        0 => Message::Hello {
            seat: r.opt_u8()?,
            rejoin: r.opt_u16()?,
        },
        1 => Message::Welcome {
            client: r.u16()?,
            seat: r.u8()?,
        },
        2 => Message::Reject(r.text()?),
        3 => Message::Command(action_from(&mut r)?),
        4 => {
            let tick = r.u64()?;
            let n = r.len()?;
            let mut data = Vec::with_capacity(n);
            for _ in 0..n {
                data.push(r.u64()?);
            }
            Message::Snapshot {
                tick,
                data,
                save: r.bytes()?,
            }
        }
        5 => {
            let tick = r.u64()?;
            let n = r.len()?;
            let mut commands = Vec::with_capacity(n);
            for _ in 0..n {
                let client = r.u16()?;
                commands.push(ClientCommand {
                    client,
                    action: action_from(&mut r)?,
                });
            }
            Message::Go { tick, commands }
        }
        6 => Message::Hash {
            tick: r.u64()?,
            hash: r.u64()?,
        },
        7 => Message::Bye,
        8 => Message::End(match r.u8()? {
            0 => Ending::Quit,
            1 => Ending::Other(r.text()?),
            _ => return Err(Error::Malformed),
        }),
        _ => return Err(Error::Malformed),
    };
    if !r.0.is_empty() {
        return Err(Error::Malformed);
    }
    Ok(message)
}

#[allow(clippy::too_many_lines)] // One action after the other.
fn action_to(w: &mut Writer, action: &Action) {
    match action {
        Action::Auction(a) => {
            w.u8(0);
            match a {
                auction::Action::Hold(down) => {
                    w.u8(0);
                    w.bool(*down);
                }
                auction::Action::Bid => w.u8(1),
                auction::Action::Pass => w.u8(2),
                auction::Action::Close => w.u8(3),
                auction::Action::Leave => w.u8(4),
            }
        }
        Action::Load(save) => {
            w.u8(1);
            w.bytes(&save.to_bytes());
        }
        Action::Build { building, at } => {
            w.u8(2);
            w.u8(*building);
            w.pos(*at);
        }
        Action::Level { raise, corner, at } => {
            w.u8(3);
            w.bool(*raise);
            w.pos(*corner);
            w.pos(*at);
        }
        Action::Demolish(at) => {
            w.u8(4);
            w.pos(*at);
        }
        Action::RoadDragStart(at) => {
            w.u8(5);
            w.pos(*at);
        }
        Action::RoadDragEnd { from, to } => {
            w.u8(6);
            w.pos(*from);
            w.pos(*to);
        }
        Action::RoadDragCancel => w.u8(7),
        Action::Speed(speed) => {
            w.u8(8);
            w.i32(*speed);
        }
        Action::Pause(on) => {
            w.u8(9);
            w.bool(*on);
        }
        Action::AuctionTimeout(ticks) => {
            w.u8(10);
            w.u16(*ticks);
        }
        Action::Cheat => w.u8(11),
        Action::SetOption(option, on) => {
            w.u8(12);
            w.u8(match option {
                WorldOption::PeopleMove => 0,
                WorldOption::AuctionWaits => 1,
                WorldOption::Disasters => 2,
                WorldOption::PeopleSpawn => 3,
            });
            w.bool(*on);
        }
        Action::MoneyMenu(a) => {
            w.u8(13);
            match a {
                geldmenu::Action::Shown(on) => {
                    w.u8(0);
                    w.bool(*on);
                }
                geldmenu::Action::TakeLoan => w.u8(1),
                geldmenu::Action::RepayLoan => w.u8(2),
                geldmenu::Action::Advertising { more } => {
                    w.u8(3);
                    w.bool(*more);
                }
                geldmenu::Action::Renovation { more } => {
                    w.u8(4);
                    w.bool(*more);
                }
                geldmenu::Action::Insurance(on) => {
                    w.u8(5);
                    w.bool(*on);
                }
            }
        }
        Action::Sabotage { kind, at } => {
            w.u8(14);
            w.u8(*kind);
            w.pos(*at);
        }
        Action::Renovate(at) => {
            w.u8(15);
            w.pos(*at);
        }
        Action::Price { at, raise } => {
            w.u8(16);
            w.pos(*at);
            w.bool(*raise);
        }
        Action::Bribe(at) => {
            w.u8(17);
            w.pos(*at);
        }
        Action::Sell(at) => {
            w.u8(18);
            w.pos(*at);
        }
        Action::ShowTv(n) => {
            w.u8(19);
            w.i16(*n);
        }
        Action::TvPause(on) => {
            w.u8(20);
            w.bool(*on);
        }
        Action::Speaker(s) => {
            w.u8(21);
            w.u8(match s {
                Speaker::Off => 0,
                Speaker::Open => 1,
                Speaker::Reading => 2,
            });
        }
        Action::SetFix(fix, on) => {
            w.u8(22);
            w.bytes(fix.info().key.as_bytes());
            w.bool(*on);
        }
        Action::Seat(change) => {
            w.u8(23);
            match change {
                SeatChange::Join { client, seat } => {
                    w.u8(0);
                    w.u16(*client);
                    w.u8(*seat);
                }
                SeatChange::Leave(c) => {
                    w.u8(1);
                    w.u16(*c);
                }
                SeatChange::Disconnect(c) => {
                    w.u8(2);
                    w.u16(*c);
                }
                SeatChange::Rejoin(c) => {
                    w.u8(3);
                    w.u16(*c);
                }
            }
        }
        Action::ViewMoved(v) => {
            w.u8(24);
            w.u8(v.rotation);
            w.pos(v.origin);
            w.i16(v.parity);
        }
    }
}

fn action_from(r: &mut Reader) -> Result<Action, Error> {
    Ok(match r.u8()? {
        0 => Action::Auction(match r.u8()? {
            0 => auction::Action::Hold(r.bool()?),
            1 => auction::Action::Bid,
            2 => auction::Action::Pass,
            3 => auction::Action::Close,
            4 => auction::Action::Leave,
            _ => return Err(Error::Malformed),
        }),
        1 => {
            let bytes = r.bytes()?;
            Action::Load(Arc::new(SaveGame::parse(&bytes).map_err(|_| Error::Malformed)?))
        }
        2 => Action::Build {
            building: r.u8()?,
            at: r.pos()?,
        },
        3 => Action::Level {
            raise: r.bool()?,
            corner: r.pos()?,
            at: r.pos()?,
        },
        4 => Action::Demolish(r.pos()?),
        5 => Action::RoadDragStart(r.pos()?),
        6 => Action::RoadDragEnd {
            from: r.pos()?,
            to: r.pos()?,
        },
        7 => Action::RoadDragCancel,
        8 => Action::Speed(r.i32()?),
        9 => Action::Pause(r.bool()?),
        10 => Action::AuctionTimeout(r.u16()?),
        11 => Action::Cheat,
        12 => {
            let option = match r.u8()? {
                0 => WorldOption::PeopleMove,
                1 => WorldOption::AuctionWaits,
                2 => WorldOption::Disasters,
                3 => WorldOption::PeopleSpawn,
                _ => return Err(Error::Malformed),
            };
            Action::SetOption(option, r.bool()?)
        }
        13 => Action::MoneyMenu(match r.u8()? {
            0 => geldmenu::Action::Shown(r.bool()?),
            1 => geldmenu::Action::TakeLoan,
            2 => geldmenu::Action::RepayLoan,
            3 => geldmenu::Action::Advertising { more: r.bool()? },
            4 => geldmenu::Action::Renovation { more: r.bool()? },
            5 => geldmenu::Action::Insurance(r.bool()?),
            _ => return Err(Error::Malformed),
        }),
        14 => Action::Sabotage {
            kind: r.u8()?,
            at: r.pos()?,
        },
        15 => Action::Renovate(r.pos()?),
        16 => Action::Price {
            at: r.pos()?,
            raise: r.bool()?,
        },
        17 => Action::Bribe(r.pos()?),
        18 => Action::Sell(r.pos()?),
        19 => Action::ShowTv(r.i16()?),
        20 => Action::TvPause(r.bool()?),
        21 => Action::Speaker(match r.u8()? {
            0 => Speaker::Off,
            1 => Speaker::Open,
            2 => Speaker::Reading,
            _ => return Err(Error::Malformed),
        }),
        22 => {
            let key = r.text()?;
            let fix = Fix::from_key(&key).ok_or(Error::Malformed)?;
            Action::SetFix(fix, r.bool()?)
        }
        23 => Action::Seat(match r.u8()? {
            0 => SeatChange::Join {
                client: r.u16()?,
                seat: r.u8()?,
            },
            1 => SeatChange::Leave(r.u16()?),
            2 => SeatChange::Disconnect(r.u16()?),
            3 => SeatChange::Rejoin(r.u16()?),
            _ => return Err(Error::Malformed),
        }),
        24 => Action::ViewMoved(Viewpoint {
            rotation: r.u8()?,
            origin: r.pos()?,
            parity: r.i16()?,
        }),
        _ => return Err(Error::Malformed),
    })
}

/// A whole frame.
const WHOLE: u8 = 0;
/// A piece of a message, more to come.
const MORE: u8 = 1;
/// The last piece of a message.
const LAST: u8 = 2;

/// `message`'s frames, none bigger than `max` bytes (at least 2).
#[must_use]
pub fn frames(message: &[u8], max: usize) -> Vec<Vec<u8>> {
    let room = max.max(2) - 1;
    if message.len() <= room {
        let mut f = Vec::with_capacity(message.len() + 1);
        f.push(WHOLE);
        f.extend_from_slice(message);
        return vec![f];
    }
    let pieces: Vec<&[u8]> = message.chunks(room).collect();
    let last = pieces.len() - 1;
    pieces
        .into_iter()
        .enumerate()
        .map(|(n, piece)| {
            let mut f = Vec::with_capacity(piece.len() + 1);
            f.push(if n == last { LAST } else { MORE });
            f.extend_from_slice(piece);
            f
        })
        .collect()
}

/// Puts one peer's frames back together into messages.
#[derive(Debug, Default)]
pub struct Assembler {
    partial: Vec<u8>,
}

impl Assembler {
    /// The message `frame` completes, if it does.
    ///
    /// # Errors
    /// If it isn't a frame.
    pub fn push(&mut self, frame: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let (&kind, data) = frame.split_first().ok_or(Error::Malformed)?;
        match kind {
            WHOLE if self.partial.is_empty() => Ok(Some(data.to_vec())),
            MORE => {
                self.partial.extend_from_slice(data);
                Ok(None)
            }
            LAST if !self.partial.is_empty() => {
                self.partial.extend_from_slice(data);
                Ok(Some(std::mem::take(&mut self.partial)))
            }
            _ => Err(Error::Malformed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::seats::HOST;

    fn every_action() -> Vec<Action> {
        let p = (12, -3);
        let mut out = vec![
            Action::Auction(auction::Action::Hold(true)),
            Action::Auction(auction::Action::Bid),
            Action::Auction(auction::Action::Pass),
            Action::Auction(auction::Action::Close),
            Action::Auction(auction::Action::Leave),
            Action::Build { building: 9, at: p },
            Action::Level {
                raise: true,
                corner: (1, 2),
                at: p,
            },
            Action::Demolish(p),
            Action::RoadDragStart(p),
            Action::RoadDragEnd { from: p, to: (4, 5) },
            Action::RoadDragCancel,
            Action::Speed(-77),
            Action::Pause(true),
            Action::AuctionTimeout(300),
            Action::Cheat,
            Action::MoneyMenu(geldmenu::Action::Shown(true)),
            Action::MoneyMenu(geldmenu::Action::TakeLoan),
            Action::MoneyMenu(geldmenu::Action::RepayLoan),
            Action::MoneyMenu(geldmenu::Action::Advertising { more: false }),
            Action::MoneyMenu(geldmenu::Action::Renovation { more: true }),
            Action::MoneyMenu(geldmenu::Action::Insurance(true)),
            Action::Sabotage { kind: 3, at: p },
            Action::Renovate(p),
            Action::Price { at: p, raise: false },
            Action::Bribe(p),
            Action::Sell(p),
            Action::ShowTv(-1),
            Action::TvPause(false),
            Action::Speaker(Speaker::Reading),
            Action::Seat(SeatChange::Join { client: 7, seat: 3 }),
            Action::Seat(SeatChange::Leave(7)),
            Action::Seat(SeatChange::Disconnect(7)),
            Action::Seat(SeatChange::Rejoin(7)),
            Action::ViewMoved(Viewpoint {
                rotation: 2,
                origin: p,
                parity: 1,
            }),
        ];
        for o in [
            WorldOption::PeopleMove,
            WorldOption::AuctionWaits,
            WorldOption::Disasters,
            WorldOption::PeopleSpawn,
        ] {
            out.push(Action::SetOption(o, true));
        }
        out.extend(Fix::ALL.iter().map(|&f| Action::SetFix(f, true)));
        out
    }

    #[test]
    fn every_message_reads_back() {
        let mut messages = vec![
            Message::Hello {
                seat: Some(2),
                rejoin: None,
            },
            Message::Hello {
                seat: None,
                rejoin: Some(4),
            },
            Message::Welcome { client: 3, seat: 2 },
            Message::Reject("no seat".to_owned()),
            Message::Snapshot {
                tick: 99,
                data: vec![5, !0],
                save: vec![1, 2, 3],
            },
            Message::Go {
                tick: 1 << 40,
                commands: every_action()
                    .into_iter()
                    .map(|action| ClientCommand { client: HOST, action })
                    .collect(),
            },
            Message::Go {
                tick: 0,
                commands: Vec::new(),
            },
            Message::Hash { tick: 10, hash: !0 },
            Message::Bye,
            Message::End(Ending::Quit),
            Message::End(Ending::Other("the host's game stopped".to_owned())),
        ];
        messages.extend(every_action().into_iter().map(Message::Command));
        for m in messages {
            let bytes = encode(&m);
            assert_eq!(decode(&bytes), Ok(m.clone()), "{m:?}");
            assert_eq!(encode(&m), bytes, "the same bytes again");
            assert_eq!(decode(&bytes[..bytes.len() - 1]).ok(), None, "cut short: {m:?}");
        }
    }

    #[test]
    fn other_versions_and_junk_are_refused() {
        let mut bytes = encode(&Message::Bye);
        bytes[0] = bytes[0].wrapping_add(1);
        assert_eq!(decode(&bytes), Err(Error::Version(VERSION + 1)));
        assert_eq!(decode(&[]), Err(Error::Malformed));
        let mut bytes = encode(&Message::Bye);
        bytes.push(0);
        assert_eq!(decode(&bytes), Err(Error::Malformed));
        let mut bytes = encode(&Message::Command(Action::Cheat));
        bytes[3] = 200;
        assert_eq!(decode(&bytes), Err(Error::Malformed));
    }

    #[test]
    fn frames_put_back_together() {
        let message: Vec<u8> = (0..1000u32).map(|i| u8::try_from(i * 7 % 256).unwrap()).collect();
        for max in [2, 3, 100, 999, 1000, 1001, 5000] {
            let fs = frames(&message, max);
            assert!(fs.iter().all(|f| f.len() <= max), "{max}");
            let mut a = Assembler::default();
            let mut got = Vec::new();
            for f in &fs {
                if let Some(m) = a.push(f).unwrap() {
                    got.push(m);
                }
            }
            assert_eq!(got, vec![message.clone()], "{max}");
        }
        assert!(Assembler::default().push(&[LAST, 1]).is_err());
        assert!(Assembler::default().push(&[]).is_err());
    }
}
