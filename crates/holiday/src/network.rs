//! A network game from the command line (`docs/networking.md`):
//! `--host=ROOM` hosts the game the player starts, `--join=ROOM` waits
//! for the host's world and plays along in a seat of it. Without either
//! nothing here runs: no connection, no thread, no runtime.
//!
//! The world changes as in single player, by commands; in a network
//! game the view hands its commands to the session (`net::Session`),
//! and the game runs a tick only with the host's go for it, applying the
//! go's commands first (`Game::update`).

use std::path::Path;

use formats::savegame::SaveGame;
use game::hintergrund;
use game::seats::Client;
use game::startform::Settings;
use game::world::World;
use net::matchbox::{IceServers, MatchboxTransport, valid_room};
use net::{Config, Event, Session};

/// The default signaling server: one on this machine.
pub(crate) const DEFAULT_SIGNAL: &str = "ws://localhost:3536";

/// The command line's network options, as given.
#[derive(Debug, Default)]
pub(crate) struct Options {
    pub host: Option<String>,
    pub join: Option<String>,
    pub signal: Option<String>,
    pub ice: IceServers,
    pub seat: Option<u8>,
    pub rejoin: Option<Client>,
}

/// Hosting or joining.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Host,
    Join,
}

/// A network game the command line asked for.
#[derive(Debug, Clone)]
pub(crate) struct NetArgs {
    pub role: Role,
    pub room: String,
    pub signal: String,
    pub ice: IceServers,
    pub config: Config,
}

impl Options {
    /// Takes `arg` if it's a network option: whether it was, or why it
    /// doesn't read.
    pub fn take(&mut self, arg: &str) -> Result<bool, String> {
        if let Some(r) = arg.strip_prefix("--host=") {
            self.host = Some(r.to_owned());
        } else if let Some(r) = arg.strip_prefix("--join=") {
            self.join = Some(r.to_owned());
        } else if let Some(s) = arg.strip_prefix("--signal=") {
            self.signal = Some(s.to_owned());
        } else if let Some(u) = arg.strip_prefix("--stun=") {
            self.ice.stun.push(u.to_owned());
        } else if let Some(u) = arg.strip_prefix("--turn=") {
            self.ice.turn.push(u.to_owned());
        } else if let Some(u) = arg.strip_prefix("--turn-user=") {
            self.ice.username = Some(u.to_owned());
        } else if let Some(p) = arg.strip_prefix("--turn-password=") {
            self.ice.credential = Some(p.to_owned());
        } else if let Some(s) = arg.strip_prefix("--seat=") {
            let seat = s.parse::<u8>().ok().filter(|s| (1..=8).contains(s));
            self.seat = Some(seat.ok_or("--seat= takes 1 to 8")?);
        } else if let Some(c) = arg.strip_prefix("--rejoin=") {
            self.rejoin = Some(
                c.parse()
                    .map_err(|_| "--rejoin= takes the client number the game gave")?,
            );
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    /// The network game asked for, if any.
    ///
    /// # Errors
    /// If the options don't go together.
    pub fn finish(self) -> Result<Option<NetArgs>, String> {
        let (role, room) = match (self.host, self.join) {
            (Some(_), Some(_)) => return Err("--host= and --join= don't go together".to_owned()),
            (Some(r), None) => (Role::Host, r),
            (None, Some(r)) => (Role::Join, r),
            (None, None) => {
                let given = self.signal.is_some()
                    || self.seat.is_some()
                    || self.rejoin.is_some()
                    || self.ice != IceServers::default();
                if given {
                    return Err("the network options need --host= or --join=".to_owned());
                }
                return Ok(None);
            }
        };
        if !valid_room(&room) {
            return Err(format!("{room:?}: a room's name is 1 to 64 letters, digits, - and _"));
        }
        if role == Role::Host && (self.seat.is_some() || self.rejoin.is_some()) {
            return Err("--seat= and --rejoin= are for --join=; the host plays seat 1".to_owned());
        }
        self.ice.check()?;
        let signal = self.signal.unwrap_or_else(|| DEFAULT_SIGNAL.to_owned());
        if !signal.starts_with("ws://") && !signal.starts_with("wss://") {
            return Err(format!("--signal={signal}: a ws:// or wss:// URL"));
        }
        Ok(Some(NetArgs {
            role,
            room,
            signal,
            ice: self.ice,
            config: Config {
                seat: self.seat,
                rejoin: self.rejoin,
                ..Config::default()
            },
        }))
    }
}

/// The session over WebRTC.
pub(crate) type NetSession = Session<MatchboxTransport>;

/// A game's network session. When the game goes (back to the start
/// menu, or the program ends), a client says goodbye and the host ends
/// the session for its clients.
pub(crate) struct NetGame {
    pub session: NetSession,
}

impl Drop for NetGame {
    fn drop(&mut self) {
        let why = if self.session.is_host() {
            "the host left the game"
        } else {
            ""
        };
        self.session.leave(why);
    }
}

impl NetArgs {
    /// Connects to the room: the host's session or the client's.
    ///
    /// # Errors
    /// If the connection can't start.
    pub fn connect(&self) -> Result<NetSession, String> {
        let transport = MatchboxTransport::new(&self.signal, self.ice.clone());
        match self.role {
            Role::Host => Session::host(transport, &self.room, self.config),
            Role::Join => Session::join(transport, &self.room, self.config),
        }
    }
}

/// The world a joining client starts from: the host's snapshot `save`
/// read over a world set up from this game folder `dir`, in the
/// snapshot's world language (the world's text and the sabotage's table
/// are that language's, as on the host).
///
/// # Errors
/// If this folder's files aren't the host's, or the snapshot doesn't
/// read.
pub(crate) fn joined_world(
    dir: &Path,
    data: u64,
    save: &SaveGame,
    fallback: game::lang::Language,
) -> Result<World, String> {
    let language = game::lang::language_in(&save.trailing).unwrap_or(fallback);
    let files = hintergrund::GameData::load(|name| super::read_opt(dir, name), language).map_err(|e| e.to_string())?;
    let base = hintergrund::set_up_begin(&Settings::default(), &files, 0, game::enhance::Fixes::ORIGINAL);
    net::world_from_snapshot(&base, data, save)
}

/// Says on the terminal what the session says, and whether it ended,
/// with why.
pub(crate) fn report(events: &[Event]) -> Option<String> {
    let mut ended = None;
    for e in events {
        match e {
            Event::Welcome { client, seat } => eprintln!(
                "network: joined as client {client} in seat {seat}; after a dropped connection, come back with --rejoin={client}"
            ),
            Event::Snapshot { tick, .. } => eprintln!("network: the host's world came, at tick {tick}"),
            Event::Joined { client, seat, rejoin } => {
                let how = if *rejoin { "is back" } else { "joined" };
                eprintln!("network: client {client} {how}, in seat {seat}");
            }
            Event::Left { client, for_good } => {
                let how = if *for_good { "left" } else { "lost the connection" };
                eprintln!("network: client {client} {how}");
            }
            Event::Desync {
                tick,
                client,
                ours,
                theirs,
            } => {
                let whose = client.map_or_else(|| "the host's".to_owned(), |c| format!("client {c}'s"));
                eprintln!("network: DESYNC after tick {tick}: this world {ours:016x}, {whose} {theirs:016x}");
            }
            Event::Rejected(why) => {
                eprintln!("network: the host turned this game away: {why}");
                ended = Some(why.clone());
            }
            Event::Ended(why) => {
                eprintln!("network: {why}");
                ended = Some(why.clone());
            }
            Event::Warning(w) => eprintln!("network: {w}"),
        }
    }
    ended
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<NetArgs>, String> {
        let mut o = Options::default();
        for a in args {
            assert!(o.take(a)?, "{a}");
        }
        o.finish()
    }

    #[test]
    fn options() {
        assert!(parse(&[]).unwrap().is_none());
        let a = parse(&["--host=isle"]).unwrap().unwrap();
        assert_eq!(
            (a.role, a.room.as_str(), a.signal.as_str()),
            (Role::Host, "isle", DEFAULT_SIGNAL)
        );
        let a = parse(&[
            "--join=isle",
            "--signal=wss://example.org/hi",
            "--seat=3",
            "--rejoin=4",
            "--stun=stun:example.org:3478",
            "--turn=turn:example.org:3478",
            "--turn-user=u",
            "--turn-password=p",
        ])
        .unwrap()
        .unwrap();
        assert_eq!((a.config.seat, a.config.rejoin), (Some(3), Some(4)));
        assert_eq!(a.ice.turn, ["turn:example.org:3478"]);
        assert!(parse(&["--host=a", "--join=b"]).is_err());
        assert!(parse(&["--seat=2"]).is_err());
        assert!(parse(&["--host=a", "--seat=2"]).is_err());
        assert!(parse(&["--join=a/b"]).is_err());
        assert!(parse(&["--join=a", "--signal=http://x"]).is_err());
        assert!(parse(&["--join=a", "--turn=turn:x"]).is_err(), "TURN without a user");
        assert!(parse(&["--join=a", "--seat=9"]).is_err());
        let mut o = Options::default();
        assert_eq!(o.take("--fullscreen"), Ok(false));
    }
}
