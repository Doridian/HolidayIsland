//! The lockstep session: host-authoritative, over a [`Transport`] in a
//! star (every client connected to the host only).
//!
//! - A client sends what it does to the host ([`Session::submit`]).
//! - The host stamps each command with the tick it's carried out on, a
//!   few ticks on (the input delay, [`Config::delay`]), in the order the
//!   commands came, and sends each tick's list to every client: the
//!   **go** for that tick, which may be empty. Its own commands go the
//!   same way.
//! - Every peer runs a tick only once it has the go for it
//!   ([`Session::next_step`]): it applies the list's commands
//!   (`World::apply_client`) and then ticks the world. So the host runs
//!   on its clock, and the clients follow as the goes come, at most the
//!   delay behind when the network is quick enough.
//! - Every [`Config::hash_every`] ticks each peer sends the hash of its
//!   world (`crate::hash`); a mismatch is a desync, reported as
//!   [`Event::Desync`], not a crash.
//! - A client that joins says hello; the host gives it an id and a seat
//!   and schedules its `SeatChange::Join` like any command. Before the
//!   tick the join is carried out on, the host sends the client its
//!   world as a saved game ([`game::world::World::snapshot`]) and the
//!   goes from that tick on, and the client plays along from there. One
//!   that comes back says which client it was, and gets
//!   `SeatChange::Rejoin` and a snapshot the same way.
//! - A client's connection dropping is `SeatChange::Disconnect`, saying
//!   goodbye ([`Session::leave`]) `SeatChange::Leave`. When the host
//!   quits, it says so ([`Ending::Quit`]) and the clients get
//!   [`Event::Ended`] with [`End::HostQuit`]. When the connection to the
//!   host drops, a client waits [`Config::host_grace`] for it to come
//!   back (it then says hello again as the client it was and gets a new
//!   snapshot); after that the host counts as gone for good
//!   ([`End::HostLost`]). There's no host migration: the game is over
//!   for the clients, who may save their own copy of the world.
//!
//! The session never touches the world: the caller applies the steps'
//! commands and ticks it, so the world changes only by commands, as in
//! single player.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use web_time::Instant;

use formats::savegame::SaveGame;
use game::command::Action;
use game::seats::{Client, ClientCommand, HOST, HOST_SEAT, SeatChange};
use game::world::World;

use crate::hash::{data_differences, data_hashes, world_hash};
use crate::transport::{PeerId, Role, Transport, TransportEvent};
use crate::wire::{self, Assembler, Ending, Message};

/// How many of a peer's own hashes it keeps to compare with others'
/// that come late.
const KEEP_HASHES: usize = 256;

/// How long a client waits for the host's connection to come back once
/// it dropped, before the host counts as gone for good and the game is
/// over ([`End::HostLost`]): 10 seconds. Long enough for a short
/// network hiccup (the host's peer connecting again), short enough that
/// the players aren't left looking at a stopped game for long; the world
/// waits meanwhile, as it does for any late go.
pub const HOST_GRACE: Duration = Duration::from_secs(10);

/// The session's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// The input delay: a command is carried out this many ticks after
    /// the host stamps it. 3 ticks (300 ms) by default.
    pub delay: u64,
    /// The peers compare their worlds' hashes every this many ticks:
    /// 10 (a second of the game's clock) by default.
    pub hash_every: u64,
    /// A client: the seat it asks for. None takes the first computer's.
    pub seat: Option<u8>,
    /// A client: the client it was, to come back as.
    pub rejoin: Option<Client>,
    /// A client: how long it waits for a dropped host to come back
    /// ([`HOST_GRACE`] by default).
    pub host_grace: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            delay: 3,
            hash_every: 10,
            seat: None,
            rejoin: None,
            host_grace: HOST_GRACE,
        }
    }
}

/// Why the session ended for this peer ([`Event::Ended`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// A client: the host quit its game (said so, [`Ending::Quit`]).
    HostQuit,
    /// A client: the connection to the host is gone for good: it dropped
    /// and didn't come back within [`Config::host_grace`], or this
    /// peer's transport closed. How.
    HostLost(String),
    /// Anything else (the host's transport closing, another protocol
    /// version, a world that didn't read), with why.
    Other(String),
}

impl std::fmt::Display for End {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            End::HostQuit => write!(f, "the host ended the game"),
            End::HostLost(how) => write!(f, "lost the host: {how}"),
            End::Other(why) => write!(f, "{why}"),
        }
    }
}

/// What happened, from [`Session::poll`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A client: the host took it in, as `client` in `seat`.
    Welcome { client: Client, seat: u8 },
    /// A client: the world to start from, before tick `tick`; the steps
    /// from that tick follow. Read it with [`world_from_snapshot`].
    Snapshot {
        tick: u64,
        data: Vec<u64>,
        save: Box<SaveGame>,
    },
    /// The host: a client is taken in (or back) at `seat`.
    Joined { client: Client, seat: u8, rejoin: bool },
    /// The host: a client's connection dropped, or it left for good.
    Left { client: Client, for_good: bool },
    /// The worlds differ after `tick`: this peer's hash and the other's,
    /// and the client it's from (none: the host's).
    Desync {
        tick: u64,
        client: Option<Client>,
        ours: u64,
        theirs: u64,
    },
    /// A client: the host turned it away.
    Rejected(String),
    /// A client: the connection to the host dropped. The game waits up
    /// to `grace` for it to come back, then ends ([`End::HostLost`]).
    HostLost { grace: Duration },
    /// A client: the host is back within the grace; a new welcome and
    /// snapshot follow, which the client's world is read from again.
    HostBack,
    /// The session is over for this peer: the host quit, the connection
    /// to it is gone for good, or something else ([`End`]).
    Ended(End),
    /// Something to log: a message that didn't read.
    Warning(String),
}

/// One tick's commands, to apply in order before the world's tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub tick: u64,
    pub commands: Vec<ClientCommand>,
}

/// A client as the host sees it.
#[derive(Debug, Clone)]
struct Remote {
    client: Client,
    seat: u8,
    /// The tick its join or rejoin is carried out on, once stamped.
    due: Option<u64>,
    /// Its join or rejoin is among the commands not stamped yet.
    queued: bool,
    /// It has its snapshot and gets the goes.
    playing: bool,
    /// It said goodbye.
    leaving: bool,
}

/// A lockstep session over a transport `T`.
pub struct Session<T: Transport> {
    transport: T,
    role: Role,
    config: Config,
    /// This peer's client and seat, once known.
    me: Option<(Client, u8)>,
    /// The next tick to run; a client has none until its snapshot.
    tick: Option<u64>,
    /// The goes not run yet, by tick.
    schedule: BTreeMap<u64, Vec<ClientCommand>>,
    /// The tick [`Session::next_step`] handed out and
    /// [`Session::finish_step`] hasn't seen end yet.
    stepping: Option<u64>,
    /// This peer's hashes, by tick, and the others' it can't check yet
    /// (with the client they're from; none: the host).
    hashes: BTreeMap<u64, u64>,
    theirs: Vec<(u64, Option<Client>, u64)>,
    assemblers: BTreeMap<PeerId, Assembler>,
    events: Vec<Event>,
    ended: bool,
    /// The host: the next tick to stamp commands for.
    stamp: u64,
    /// The host: the commands not stamped yet, in order.
    pending: Vec<ClientCommand>,
    /// The host: its clients, by peer.
    remotes: BTreeMap<PeerId, Remote>,
    /// The host: the hellos that wait for a world to answer from.
    hellos: Vec<(PeerId, Option<u8>, Option<Client>)>,
    /// The host: the client ids it gave out.
    issued: BTreeSet<Client>,
    /// A client: the host's peer, once connected.
    host: Option<PeerId>,
    /// A client: when the host's connection dropped, while it may still
    /// come back.
    host_lost: Option<Instant>,
}

impl<T: Transport> Session<T> {
    /// The host's session, in `room`: the game's tick 0 is its first step.
    ///
    /// # Errors
    /// If the transport can't connect.
    pub fn host(mut transport: T, room: &str, config: Config) -> Result<Self, String> {
        transport.connect(room, Role::Host)?;
        let mut s = Session::new(transport, Role::Host, config);
        s.me = Some((HOST, HOST_SEAT));
        s.tick = Some(0);
        // The first ticks have no commands: none can be stamped that soon.
        for t in 0..config.delay {
            s.schedule.insert(t, Vec::new());
        }
        s.stamp = config.delay;
        Ok(s)
    }

    /// A client's session, in `room`: it says hello once the host is
    /// connected.
    ///
    /// # Errors
    /// If the transport can't connect.
    pub fn join(mut transport: T, room: &str, config: Config) -> Result<Self, String> {
        transport.connect(room, Role::Client)?;
        Ok(Session::new(transport, Role::Client, config))
    }

    fn new(transport: T, role: Role, config: Config) -> Self {
        Session {
            transport,
            role,
            config: Config {
                hash_every: config.hash_every.max(1),
                ..config
            },
            me: None,
            tick: None,
            schedule: BTreeMap::new(),
            stepping: None,
            hashes: BTreeMap::new(),
            theirs: Vec::new(),
            assemblers: BTreeMap::new(),
            events: Vec::new(),
            ended: false,
            stamp: 0,
            pending: Vec::new(),
            remotes: BTreeMap::new(),
            hellos: Vec::new(),
            issued: BTreeSet::new(),
            host: None,
            host_lost: None,
        }
    }

    /// Whether this is the host's session.
    #[must_use]
    pub fn is_host(&self) -> bool {
        self.role == Role::Host
    }

    /// This peer's client id and seat, once the host said.
    #[must_use]
    pub fn me(&self) -> Option<(Client, u8)> {
        self.me
    }

    /// The next tick to run, once there is a world.
    #[must_use]
    pub fn tick(&self) -> Option<u64> {
        self.tick
    }

    /// The session is over ([`Event::Ended`] or [`Event::Rejected`]).
    #[must_use]
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// The transport, for a test to look at.
    pub fn transport(&mut self) -> &mut T {
        &mut self.transport
    }

    /// How many ticks a client could run now, one after the other:
    /// those it has the go for. The host can always run one.
    #[must_use]
    pub fn ready(&self) -> usize {
        if self.is_host() {
            return 1;
        }
        let Some(mut t) = self.tick else { return 0 };
        let mut n = 0;
        while self.schedule.contains_key(&t) {
            n += 1;
            t += 1;
        }
        n
    }

    /// How far a client is behind the host: how many more ticks it has
    /// the go for than the input delay keeps in hand (the host sends the
    /// go for its tick plus the delay as it runs its tick). A client that
    /// runs those at once runs each tick as soon as the host has.
    #[must_use]
    pub fn behind(&self) -> usize {
        if self.is_host() {
            return 0;
        }
        let delay = usize::try_from(self.config.delay).unwrap_or(usize::MAX);
        self.ready().saturating_sub(delay)
    }

    /// What this peer does: sent to the host, which stamps it.
    pub fn submit(&mut self, action: Action) {
        if self.ended {
            return;
        }
        match self.role {
            Role::Host => self.pending.push(ClientCommand { client: HOST, action }),
            Role::Client => {
                if self.me.is_some()
                    && let Some(host) = self.host
                {
                    self.send(host, &Message::Command(action));
                }
            }
        }
    }

    /// Takes in what came over the network and says what happened. The
    /// host needs its world (`world`) to take clients in; until it has
    /// one, their hellos wait.
    pub fn poll(&mut self, world: Option<&World>) -> Vec<Event> {
        for event in self.transport.poll() {
            match event {
                TransportEvent::Connected(peer) => self.connected(peer),
                TransportEvent::Disconnected(peer) => self.disconnected(peer),
                TransportEvent::Message(peer, frame) => {
                    let assembled = self.assemblers.entry(peer).or_default().push(&frame);
                    match assembled {
                        Ok(Some(bytes)) => match wire::decode(&bytes) {
                            Ok(m) => self.message(peer, m),
                            Err(e) => self.bad_message(peer, &e),
                        },
                        Ok(None) => {}
                        Err(e) => self.bad_message(peer, &e),
                    }
                }
                TransportEvent::Closed(why) => {
                    let end = match self.role {
                        // Nothing comes back over a closed transport.
                        Role::Client => End::HostLost(format!("the connection closed: {why}")),
                        Role::Host => End::Other(format!("the connection closed: {why}")),
                    };
                    self.end(end);
                }
            }
        }
        if self
            .host_lost
            .is_some_and(|since| since.elapsed() >= self.config.host_grace)
        {
            self.host_lost = None;
            self.end(End::HostLost(format!(
                "the connection dropped and didn't come back within {} s",
                self.config.host_grace.as_secs()
            )));
        }
        if let Some(world) = world {
            for (peer, seat, rejoin) in std::mem::take(&mut self.hellos) {
                self.admit(peer, seat, rejoin, world);
            }
        }
        std::mem::take(&mut self.events)
    }

    /// The next tick's commands, if this peer may run it: the host
    /// always, stamping the commands that came for the tick the input
    /// delay on; a client once it has the go. `world` is the world before
    /// the tick: the host sends it to the clients whose seat change is
    /// due on this tick. Apply the commands in order, tick the world and
    /// then call [`Session::finish_step`].
    pub fn next_step(&mut self, world: &World) -> Option<Step> {
        if self.ended || self.stepping.is_some() {
            return None;
        }
        let tick = self.tick?;
        if self.is_host() {
            self.stamp_next();
            self.send_snapshots(tick, world);
        }
        let commands = self.schedule.remove(&tick)?;
        self.stepping = Some(tick);
        Some(Step { tick, commands })
    }

    /// The tick [`Session::next_step`] gave is done, and `world` is the
    /// world after it: its hash, every [`Config::hash_every`] ticks,
    /// goes to the others and is checked against theirs.
    pub fn finish_step(&mut self, world: &World) {
        let Some(tick) = self.stepping.take() else { return };
        self.tick = Some(tick + 1);
        if !tick.is_multiple_of(self.config.hash_every) {
            return;
        }
        let hash = world_hash(world);
        self.hashes.insert(tick, hash);
        while self.hashes.len() > KEEP_HASHES {
            self.hashes.pop_first();
        }
        let m = Message::Hash { tick, hash };
        match self.role {
            Role::Host => {
                for peer in self.playing() {
                    self.send(peer, &m);
                }
            }
            Role::Client => {
                if let Some(host) = self.host {
                    self.send(host, &m);
                }
            }
        }
        self.check_hashes();
    }

    /// Leaves the session: a client says goodbye (its seat goes back to
    /// the computers unless it has co-op partners), the host ends it for
    /// every client, saying it quit ([`Ending::Quit`]).
    pub fn leave(&mut self) {
        if self.ended {
            return;
        }
        match self.role {
            Role::Host => {
                let m = Message::End(Ending::Quit);
                for peer in self.remotes.keys().copied().collect::<Vec<_>>() {
                    self.send(peer, &m);
                }
            }
            Role::Client => {
                if let Some(host) = self.host {
                    self.send(host, &Message::Bye);
                }
            }
        }
        self.ended = true;
    }

    fn end(&mut self, end: End) {
        if !self.ended {
            self.ended = true;
            self.events.push(Event::Ended(end));
        }
    }

    fn send(&mut self, peer: PeerId, message: &Message) {
        let bytes = wire::encode(message);
        let max = self.transport.max_message().min(wire::FRAME);
        for frame in wire::frames(&bytes, max) {
            self.transport.send(peer, &frame);
        }
    }

    /// The host's clients that get the goes.
    fn playing(&self) -> Vec<PeerId> {
        self.remotes
            .iter()
            .filter(|(_, r)| r.playing)
            .map(|(&p, _)| p)
            .collect()
    }

    fn connected(&mut self, peer: PeerId) {
        // A client talks to the host only: the first peer it meets, or
        // the host coming back after its connection dropped, which takes
        // the client back as the client it was.
        if self.role == Role::Client && self.host.is_none() && !self.ended {
            self.host = Some(peer);
            let hello = match self.me {
                Some((client, seat)) => Message::Hello {
                    seat: Some(seat),
                    rejoin: Some(client),
                },
                None => Message::Hello {
                    seat: self.config.seat,
                    rejoin: self.config.rejoin,
                },
            };
            if self.host_lost.take().is_some() {
                self.events.push(Event::HostBack);
            }
            self.send(peer, &hello);
        }
    }

    fn disconnected(&mut self, peer: PeerId) {
        self.assemblers.remove(&peer);
        match self.role {
            Role::Client => {
                if self.host == Some(peer) && !self.ended {
                    self.host = None;
                    if self.config.host_grace.is_zero() {
                        self.end(End::HostLost("the connection dropped".to_owned()));
                    } else {
                        self.host_lost = Some(Instant::now());
                        self.events.push(Event::HostLost {
                            grace: self.config.host_grace,
                        });
                    }
                }
            }
            Role::Host => {
                self.hellos.retain(|h| h.0 != peer);
                if let Some(r) = self.remotes.remove(&peer) {
                    // One that said goodbye is gone already.
                    if !r.leaving {
                        self.pending.push(ClientCommand {
                            client: HOST,
                            action: Action::Seat(SeatChange::Disconnect(r.client)),
                        });
                        self.events.push(Event::Left {
                            client: r.client,
                            for_good: false,
                        });
                    }
                }
            }
        }
    }

    fn bad_message(&mut self, peer: PeerId, e: &wire::Error) {
        match (self.role, e) {
            (Role::Client, wire::Error::Version(_)) if self.host == Some(peer) => {
                self.end(End::Other(format!("the host speaks another version: {e}")));
            }
            (Role::Host, wire::Error::Version(_)) => {
                self.send(peer, &Message::Reject(format!("{e}")));
            }
            _ => self.events.push(Event::Warning(format!("from {peer}: {e}"))),
        }
    }

    fn message(&mut self, peer: PeerId, message: Message) {
        match self.role {
            Role::Host => self.host_message(peer, message),
            Role::Client if Some(peer) == self.host => self.client_message(message),
            Role::Client => {}
        }
    }

    fn host_message(&mut self, peer: PeerId, message: Message) {
        match message {
            Message::Hello { seat, rejoin } => {
                if !self.remotes.contains_key(&peer) && !self.hellos.iter().any(|h| h.0 == peer) {
                    self.hellos.push((peer, seat, rejoin));
                }
            }
            Message::Command(action) => {
                if let Some(r) = self.remotes.get(&peer)
                    && !r.leaving
                {
                    let client = r.client;
                    self.pending.push(ClientCommand { client, action });
                }
            }
            Message::Hash { tick, hash } => {
                if let Some(r) = self.remotes.get(&peer) {
                    self.theirs.push((tick, Some(r.client), hash));
                    self.check_hashes();
                }
            }
            Message::Bye => {
                if let Some(r) = self.remotes.get_mut(&peer)
                    && !r.leaving
                {
                    r.leaving = true;
                    let client = r.client;
                    self.pending.push(ClientCommand {
                        client: HOST,
                        action: Action::Seat(SeatChange::Leave(client)),
                    });
                    self.events.push(Event::Left { client, for_good: true });
                }
            }
            m => self
                .events
                .push(Event::Warning(format!("from {peer}: {} isn't for the host", name(&m)))),
        }
    }

    fn client_message(&mut self, message: Message) {
        match message {
            Message::Welcome { client, seat } => {
                self.me = Some((client, seat));
                self.events.push(Event::Welcome { client, seat });
            }
            Message::Reject(why) => {
                self.ended = true;
                self.events.push(Event::Rejected(why));
            }
            Message::Snapshot { tick, data, save } => match SaveGame::parse(&save) {
                Ok(save) => {
                    self.tick = Some(tick);
                    self.schedule.clear();
                    self.hashes.clear();
                    self.theirs.clear();
                    self.events.push(Event::Snapshot {
                        tick,
                        data,
                        save: Box::new(save),
                    });
                }
                Err(e) => self.end(End::Other(format!("the host's world didn't read: {e}"))),
            },
            Message::Go { tick, commands } => {
                if self.tick.is_some_and(|t| tick >= t) {
                    self.schedule.insert(tick, commands);
                }
            }
            Message::Hash { tick, hash } => {
                self.theirs.push((tick, None, hash));
                self.check_hashes();
            }
            Message::End(Ending::Quit) => self.end(End::HostQuit),
            Message::End(Ending::Other(why)) => self.end(End::Other(format!("the host ended the game: {why}"))),
            m => self.events.push(Event::Warning(format!(
                "the host sent {}, which is a client's",
                name(&m)
            ))),
        }
    }

    /// The host takes the client at `peer` in: back as the client it was,
    /// if that one is the game's (refused while its old connection is
    /// still there), else as a new client in the seat it asked for or the
    /// first computer's.
    fn admit(&mut self, peer: PeerId, seat: Option<u8>, rejoin: Option<Client>, world: &World) {
        if let Some(c) = rejoin
            && self.remotes.values().any(|r| r.client == c)
        {
            self.reject(peer, format!("client {c} is still connected"));
            return;
        }
        let back = rejoin.and_then(|c| {
            let m = world.seats.member(c)?;
            (c != world.seats.host()).then_some(m.seat)
        });
        let (client, seat, change) = if let (Some(c), Some(s)) = (rejoin, back) {
            (c, s, SeatChange::Rejoin(c))
        } else {
            // The seats of clients whose join isn't carried out yet.
            let reserved: Vec<u8> = self
                .remotes
                .values()
                .filter(|r| world.seats.member(r.client).is_none())
                .map(|r| r.seat)
                .collect();
            let seat = match seat {
                Some(s) if (1..=world.player_count.min(8)).contains(&s) => s,
                Some(s) => {
                    self.reject(peer, format!("this game has no seat {s}"));
                    return;
                }
                None => {
                    let free =
                        (2..=world.player_count.min(8)).find(|&s| world.is_computer(s) && !reserved.contains(&s));
                    let Some(s) = free else {
                        self.reject(peer, "no computer's seat is free (ask for one to share)".to_owned());
                        return;
                    };
                    s
                }
            };
            let taken = world.seats.members().iter().map(|m| m.client).max().unwrap_or(HOST);
            let client = taken.max(self.issued.last().copied().unwrap_or(HOST)) + 1;
            (client, seat, SeatChange::Join { client, seat })
        };
        self.issued.insert(client);
        self.pending.push(ClientCommand {
            client: HOST,
            action: Action::Seat(change),
        });
        self.remotes.insert(
            peer,
            Remote {
                client,
                seat,
                due: None,
                queued: true,
                playing: false,
                leaving: false,
            },
        );
        self.send(peer, &Message::Welcome { client, seat });
        self.events.push(Event::Joined {
            client,
            seat,
            rejoin: matches!(change, SeatChange::Rejoin(_)),
        });
    }

    fn reject(&mut self, peer: PeerId, why: String) {
        self.send(peer, &Message::Reject(why));
    }

    /// The host stamps the commands that came for the next tick to stamp
    /// and sends its go. Each command goes through the wire format here
    /// too, so the host applies what the clients read.
    fn stamp_next(&mut self) {
        let tick = self.stamp;
        self.stamp += 1;
        let go = Message::Go {
            tick,
            commands: std::mem::take(&mut self.pending),
        };
        let bytes = wire::encode(&go);
        let Ok(Message::Go { commands, .. }) = wire::decode(&bytes) else {
            unreachable!("a go reads back");
        };
        for r in self.remotes.values_mut() {
            if r.queued {
                r.queued = false;
                r.due = Some(tick);
            }
        }
        for peer in self.playing() {
            self.send(peer, &go);
        }
        self.schedule.insert(tick, commands);
    }

    /// Before `tick`: the clients whose seat change is due on it get the
    /// world and the goes from it on.
    fn send_snapshots(&mut self, tick: u64, world: &World) {
        let due: Vec<PeerId> = self
            .remotes
            .iter()
            .filter(|(_, r)| !r.playing && r.due == Some(tick))
            .map(|(&p, _)| p)
            .collect();
        if due.is_empty() {
            return;
        }
        let snapshot = Message::Snapshot {
            tick,
            data: data_hashes(world),
            save: world.snapshot().to_bytes(),
        };
        let goes: Vec<Message> = self
            .schedule
            .range(tick..)
            .map(|(&t, c)| Message::Go {
                tick: t,
                commands: c.clone(),
            })
            .collect();
        for peer in due {
            self.send(peer, &snapshot);
            for go in &goes {
                self.send(peer, go);
            }
            if let Some(r) = self.remotes.get_mut(&peer) {
                r.playing = true;
            }
        }
    }

    /// Compares the others' hashes with this peer's own, where it has
    /// both.
    fn check_hashes(&mut self) {
        let mut left = Vec::new();
        for (tick, client, theirs) in std::mem::take(&mut self.theirs) {
            if let Some(&ours) = self.hashes.get(&tick) {
                if ours != theirs {
                    self.events.push(Event::Desync {
                        tick,
                        client,
                        ours,
                        theirs,
                    });
                }
            } else if self.tick.is_some_and(|next| tick >= next) {
                // Not run yet here: checked once it is. One run before
                // this peer's snapshot, or too long ago, isn't.
                left.push((tick, client, theirs));
            }
        }
        self.theirs = left;
    }
}

/// A message's name, for warnings.
fn name(m: &Message) -> &'static str {
    match m {
        Message::Hello { .. } => "hello",
        Message::Welcome { .. } => "welcome",
        Message::Reject(_) => "reject",
        Message::Command(_) => "command",
        Message::Snapshot { .. } => "snapshot",
        Message::Go { .. } => "go",
        Message::Hash { .. } => "hash",
        Message::Bye => "bye",
        Message::End(_) => "end",
    }
}

/// The world a client starts from: the snapshot `save` read over `base`,
/// a world set up from this peer's own game files (`World::load_snapshot`),
/// once `data` shows that the tables the world takes from them are the
/// host's ([`crate::hash::data_hashes`]). The folder's language needn't
/// be: a German and an English game play together.
///
/// # Errors
/// If the tables differ from the host's (saying which files), or the
/// snapshot doesn't read.
pub fn world_from_snapshot(base: &World, data: &[u64], save: &SaveGame) -> Result<World, String> {
    let differ = data_differences(&data_hashes(base), data);
    if !differ.is_empty() {
        return Err(format!(
            "this game's files differ from the host's in what the world reads: {}",
            differ.join(", ")
        ));
    }
    base.load_snapshot(save)
        .map_err(|e| format!("the host's world didn't load: {e}"))
}
