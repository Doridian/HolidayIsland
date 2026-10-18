//! The lockstep session over the in-memory transport, on a scenario:
//! the host and clients that join in the middle of the game, one with a
//! transport that takes only small messages; their commands; a dropped
//! connection and the client coming back; a client leaving; a desync,
//! made on purpose, reported by both sides; the host ending the game.
//! After every step the peers' worlds hash the same. Skipped when
//! `orig/` isn't there.

mod common;

use common::{orig, scenario};
use formats::savegame::SaveGame;
use game::command::Action;
use game::enhance::{Fixes, SINGLE_PLAYER_ONLY};
use game::geldmenu;
use game::hintergrund::{self, GameData};
use game::lang::Language;
use game::savegame::MapView;
use game::seats::{Client, HOST};
use game::startform::Settings;
use game::world::World;
use net::hash::world_hash;
use net::memory::{Hub, MemoryTransport};
use net::{Config, Event, Session, world_from_snapshot};
use vcl::random::Random;

const ROOM: &str = "island";

/// A peer: its session, its world once it has one, and what its session
/// said.
struct Peer {
    session: Session<MemoryTransport>,
    world: Option<World>,
    events: Vec<Event>,
}

impl Peer {
    fn client(&self) -> Option<Client> {
        self.session.me().map(|(c, _)| c)
    }

    fn world(&self) -> &World {
        self.world.as_ref().expect("a world")
    }

    /// Takes in what came; a snapshot becomes the world, read over
    /// `base`.
    fn poll(&mut self, base: &World) {
        let events = self.session.poll(self.world.as_ref());
        for e in &events {
            if let Event::Snapshot { data, save, .. } = e {
                self.world = Some(world_from_snapshot(base, data, save).unwrap());
            }
        }
        self.events.extend(events);
    }

    /// One step, if the session gives one: its commands, then the tick.
    fn step(&mut self) -> bool {
        let Some(world) = &mut self.world else { return false };
        let Some(step) = self.session.next_step(world) else {
            return false;
        };
        for c in step.commands {
            world.apply_client(c);
        }
        world.tick();
        self.session.finish_step(world);
        true
    }

    fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

fn client(transport: MemoryTransport, config: Config) -> Peer {
    Peer {
        session: Session::join(transport, ROOM, config).unwrap(),
        world: None,
        events: Vec::new(),
    }
}

/// The host and its clients.
struct Game {
    hub: Hub,
    base: World,
    host: Peer,
    clients: Vec<Peer>,
}

impl Game {
    fn new(fixes: Fixes, config: Config) -> Game {
        Game::hosting(scenario(fixes), config)
    }

    /// The host's game is `world`, from the start.
    fn hosting(world: World, config: Config) -> Game {
        let hub = Hub::new();
        let host = Peer {
            session: Session::host(hub.transport(), ROOM, config).unwrap(),
            world: Some(world.clone()),
            events: Vec::new(),
        };
        Game {
            hub,
            base: world,
            host,
            clients: Vec::new(),
        }
    }

    /// `n` rounds: the host takes in what came and runs a tick, then each
    /// client takes in what came and runs up to the host's tick. After
    /// each the worlds of the peers that play hash the same.
    fn run(&mut self, n: usize) {
        for _ in 0..n {
            self.host.poll(&self.base);
            assert!(self.host.step(), "the host always runs");
            let tick = self.host.session.tick();
            for c in &mut self.clients {
                c.poll(&self.base);
                while c.session.tick() < tick && c.step() {}
            }
            let hash = world_hash(self.host.world());
            for (n, c) in self.clients.iter().enumerate() {
                if c.world.is_some() && !c.session.ended() {
                    assert_eq!(c.session.tick(), tick, "client {n} keeps up");
                    assert_eq!(world_hash(c.world()), hash, "client {n} at tick {tick:?}");
                }
            }
        }
    }

    fn join(&mut self, transport: MemoryTransport, config: Config) -> usize {
        let peer = client(transport, config);
        self.clients.push(peer);
        self.clients.len() - 1
    }

    fn host_world(&self) -> &World {
        self.host.world()
    }
}

fn joined(events: &[Event]) -> Vec<(Client, u8, bool)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Joined { client, seat, rejoin } => Some((*client, *seat, *rejoin)),
            _ => None,
        })
        .collect()
}

#[test]
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // One game, step by step.
fn three_peers_join_drop_rejoin_and_desync() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let config = Config {
        hash_every: 1,
        ..Config::default()
    };
    let mut g = Game::new(Fixes::ORIGINAL, config);
    g.run(20);

    // A joins: it gets the first computer's seat, 2, and a snapshot.
    let a = g.join(g.hub.transport(), config);
    g.run(10);
    assert_eq!(joined(&g.host.take_events()), [(1, 2, false)]);
    let events = g.clients[a].take_events();
    assert!(matches!(events[0], Event::Welcome { client: 1, seat: 2 }), "{events:?}");
    assert!(events.iter().any(|e| matches!(e, Event::Snapshot { .. })));
    let w = g.host_world();
    assert_eq!(w.seats.humans(), 2);
    assert!(!w.is_computer(2) && w.is_computer(3));
    assert!(SINGLE_PLAYER_ONLY.iter().all(|&f| w.fixes.has(f)));
    assert_eq!(g.clients[a].session.tick(), g.host.session.tick());

    // Their commands: the host's for seat 1, A's for seat 2.
    let more = Action::MoneyMenu(geldmenu::Action::Advertising { more: true });
    let (one, two) = (g.host_world().advertising_level(1), g.host_world().advertising_level(2));
    g.host.session.submit(more.clone());
    g.clients[a].session.submit(more.clone());
    g.clients[a].session.submit(more.clone());
    // A client's speed is ignored: only the host's counts.
    let speed = g.host_world().clock.speed;
    g.clients[a].session.submit(Action::Speed(speed + 9));
    g.run(10);
    let w = g.clients[a].world();
    assert_eq!(w.advertising_level(1), (one + 1).min(7));
    assert_eq!(w.advertising_level(2), (two + 2).min(7));
    assert_eq!(w.clock.speed, speed);
    g.run(200);

    // B joins in the middle of the game, over a transport that takes
    // only small messages: the snapshot comes in pieces.
    let b = g.join(g.hub.transport().with_max_message(1000), config);
    g.run(10);
    assert_eq!(joined(&g.host.take_events()), [(2, 3, false)]);
    let b_client = g.clients[b].client().unwrap();
    assert!(!g.host_world().is_computer(3));
    assert_eq!(g.host_world().seats.humans(), 3);
    g.run(100);

    // B's connection drops: its seat is the computer's.
    let b_peer = g.clients[b].session.transport().id();
    g.hub.disconnect(b_peer);
    g.run(10);
    assert!(g.host.take_events().contains(&Event::Left {
        client: b_client,
        for_good: false
    }));
    assert!(g.clients[b].take_events().iter().any(|e| matches!(e, Event::Ended(_))));
    assert!(g.host_world().is_computer(3));
    assert_eq!(g.host_world().seats.humans(), 3, "B keeps its place");
    g.run(100);

    // B comes back as the client it was, over a new connection.
    g.clients.remove(b);
    let b = g.join(
        g.hub.transport(),
        Config {
            rejoin: Some(b_client),
            ..config
        },
    );
    g.run(10);
    assert_eq!(joined(&g.host.take_events()), [(b_client, 3, true)]);
    assert!(!g.host_world().is_computer(3));
    assert_eq!(g.clients[b].client(), Some(b_client));
    g.run(50);

    // A third client asks for a seat the game hasn't.
    let c = g.join(
        g.hub.transport(),
        Config {
            seat: Some(9),
            ..config
        },
    );
    g.run(3);
    assert!(
        g.clients[c]
            .take_events()
            .iter()
            .any(|e| matches!(e, Event::Rejected(_)))
    );
    assert!(g.clients[c].session.ended());
    g.clients.remove(c);

    // A desync, on purpose: A's world goes another way. Both sides say
    // so, and neither stops.
    g.clients[a].world.as_mut().unwrap().random = Random::new(1);
    let host = g.host.session.tick();
    for _ in 0..5 {
        g.host.poll(&g.base);
        assert!(g.host.step());
        for c in &mut g.clients {
            c.poll(&g.base);
            while c.session.tick() < g.host.session.tick() && c.step() {}
        }
    }
    g.host.poll(&g.base);
    let a_client = g.clients[a].client();
    assert!(
        g.host
            .take_events()
            .iter()
            .any(|e| matches!(e, Event::Desync { client, .. } if *client == a_client))
    );
    assert!(
        g.clients[a]
            .take_events()
            .iter()
            .any(|e| matches!(e, Event::Desync { client: None, .. }))
    );
    assert!(g.host.session.tick() > host && !g.clients[a].session.ended());
    // A goes; B is still the host's world.
    g.clients.remove(a);
    let b = 0;
    g.run(20);

    // B leaves for good: it's no longer one of the game's clients.
    g.clients[b].session.leave();
    g.run(10);
    assert!(g.host.take_events().contains(&Event::Left {
        client: b_client,
        for_good: true
    }));
    assert!(g.host_world().seats.member(b_client).is_none());
    assert!(g.host_world().is_computer(3));

    // The host ends the game: the clients hear why.
    let d = g.join(g.hub.transport(), config);
    g.run(10);
    g.host.session.leave();
    for c in &mut g.clients {
        c.poll(&g.base);
    }
    let events = g.clients[d].take_events();
    assert!(
        events.iter().any(|e| matches!(e, Event::Ended(net::End::HostQuit))),
        "{events:?}"
    );
    assert_eq!(g.host_world().seats.host(), HOST);
}

/// With every fix on, and a longer input delay: a joiner plays along
/// from its snapshot, which is the host's world bit for bit.
#[test]
fn a_snapshot_is_the_world() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    for fixes in [Fixes::ORIGINAL, Fixes::all_on()] {
        let mut world = scenario(fixes);
        // The game as it starts on the screen (the binary's set-up hands
        // the views their people), then a while on.
        let _ = world.split_off_view_people();
        for _ in 0..300 {
            world.tick();
        }
        let save = SaveGame::parse(&world.snapshot().to_bytes()).unwrap();
        let data = net::hash::data_hashes(&world);
        let mut back = world_from_snapshot(&world, &data, &save).unwrap();
        assert_eq!(game::savegame::differences(&back, &world), Vec::<&str>::new());
        assert_eq!(world_hash(&back), world_hash(&world));
        // A joiner's game hands its view the people the same way: there
        // are none left in the world to hand over.
        let _ = back.split_off_view_people();
        assert_eq!(world_hash(&back), world_hash(&world));
        // A game folder of the other language: fine, and the world keeps
        // the host's folder language.
        let mut other = world.clone();
        other.folder_language = match world.folder_language {
            game::lang::Language::De => game::lang::Language::En,
            game::lang::Language::En => game::lang::Language::De,
        };
        let back = world_from_snapshot(&other, &data, &save).unwrap();
        assert_eq!(world_hash(&back), world_hash(&world));
        // Other tables: refused, saying which.
        let mut prices = (*world.tables).clone();
        prices.hausprei[1][0] += 1;
        other.tables = std::sync::Arc::new(prices);
        let e = world_from_snapshot(&other, &data, &save).unwrap_err();
        assert!(e.contains("HAUSPREI.DAT"), "{e}");
    }
    let config = Config {
        delay: 6,
        hash_every: 1,
        ..Config::default()
    };
    let mut g = Game::new(Fixes::all_on(), config);
    g.run(30);
    g.join(g.hub.transport(), config);
    g.join(
        g.hub.transport(),
        Config {
            seat: Some(1),
            ..config
        },
    );
    g.run(300);
    let w = g.host_world();
    assert_eq!(w.seats.humans(), 3);
    assert_eq!(w.seats.member(2).map(|m| m.seat), Some(1), "co-op in seat 1");
    assert!(g.clients.iter().all(|c| c.session.tick() == g.host.session.tick()));
}

/// A client's save of a network game hosts again: whoever hosts it plays
/// seat 1, the file's other clients are away (their seats the
/// computers') until they come back as the clients they were, and the
/// game plays on, every world the host's.
#[test]
#[allow(clippy::many_single_char_names)] // Two games, step by step.
fn a_clients_save_hosts_again() {
    if !orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", orig().display());
        return;
    }
    let config = Config {
        hash_every: 1,
        ..Config::default()
    };
    let mut g = Game::new(Fixes::ORIGINAL, config);
    g.run(10);
    let a = g.join(g.hub.transport(), config);
    let b = g.join(g.hub.transport(), config);
    g.run(100);
    let (a_client, b_client) = (g.clients[a].client().unwrap(), g.clients[b].client().unwrap());
    assert_eq!(g.host_world().seats.humans(), 3);
    // The host quits; A saves its copy as it stands, its view in the
    // original's fields.
    g.host.session.leave();
    for c in &mut g.clients {
        c.poll(&g.base);
    }
    assert!(g.clients[a].session.ended());
    let saved = g.clients[a].world().clone();
    let save = SaveGame::parse(&saved.to_map(&MapView::default()).to_bytes()).unwrap();

    // Someone loads it from the start menu (a fresh game, the host alone)
    // and hosts it.
    let loaded = g.base.load_map(&save).unwrap();
    assert_eq!(loaded.seats.host(), HOST);
    for c in [a_client, b_client] {
        let m = loaded.seats.member(c).expect("the file's clients are there");
        assert!(!m.present, "client {c} is away");
        assert!(loaded.is_computer(m.seat), "its seat {} is the computer's", m.seat);
    }
    assert!(loaded.seats.member(HOST).unwrap().present);
    assert!(
        SINGLE_PLAYER_ONLY.iter().all(|&f| loaded.fixes.has(f)),
        "still a game of several"
    );
    let mut h = Game::hosting(loaded, config);
    h.base = g.base.clone();
    h.run(10);
    // A comes back as the client it was, in its seat; someone new takes
    // B's seat in co-op with B, who's away.
    let b_seat = h.host_world().seats.member(b_client).unwrap().seat;
    let a2 = h.join(
        h.hub.transport(),
        Config {
            rejoin: Some(a_client),
            ..config
        },
    );
    h.join(
        h.hub.transport(),
        Config {
            seat: Some(b_seat),
            ..config
        },
    );
    h.run(100);
    let w = h.host_world();
    let a_seat = w.seats.member(a_client).unwrap().seat;
    assert!(w.seats.member(a_client).unwrap().present);
    assert!(!w.is_computer(a_seat) && !w.is_computer(b_seat));
    assert_eq!(h.clients[a2].client(), Some(a_client));
    assert!(h.clients.iter().all(|c| !c.session.ended()));
}

/// Reads `name` from the folder `root`, whatever the case of its letters.
fn read_in(root: &std::path::Path, name: &str) -> Option<Vec<u8>> {
    let mut path = root.to_path_buf();
    for part in name.split('/') {
        let found = std::fs::read_dir(&path)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?;
        path = found.path();
    }
    std::fs::read(path).ok()
}

/// A host with the German CD's files and a client with the English
/// release's (`orig-en/`) play together: the client's world is read over
/// one set up from its own folder in English, and is the host's German
/// world bit for bit, its sabotage table the German one. Skipped without
/// both folders.
#[test]
fn a_german_host_and_an_english_client() {
    let en_dir = orig().join("../orig-en");
    if !orig().join("SZENARIO").exists() || read_in(&en_dir, "DATA/MODATYP.DAT").is_none() {
        eprintln!("skipping: needs orig/ and orig-en/");
        return;
    }
    let de = GameData::load(|n| read_in(&orig(), n), Language::De).unwrap();
    let en = GameData::load(|n| read_in(&en_dir, n), Language::En).unwrap();
    let config = Config {
        hash_every: 1,
        ..Config::default()
    };
    let mut world = scenario(Fixes::ORIGINAL);
    world.tables = std::sync::Arc::clone(&de.tables);
    world.language = Language::De;
    world.folder_language = Language::De;
    let mut g = Game::hosting(world, config);
    // The client's own world, as `network::joined_world` sets it up: from
    // the English folder, in English.
    g.base = hintergrund::set_up_begin(&Settings::default(), &en, 0, Fixes::ORIGINAL);
    assert_eq!(g.base.folder_language, Language::En);
    g.run(10);
    let c = g.join(g.hub.transport(), config);
    g.run(300);
    let w = g.clients[c].world();
    assert_eq!((w.language, w.folder_language), (Language::De, Language::De));
    assert_eq!(w.sabotage_kind(game::sabotage::SHARK_DIVER).targets[0], 254);
    assert!(
        g.clients[c]
            .take_events()
            .iter()
            .chain(&g.host.take_events())
            .all(|e| !matches!(e, Event::Desync { .. }))
    );
}
