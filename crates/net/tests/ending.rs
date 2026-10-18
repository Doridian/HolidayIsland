//! The end of a network game for the clients, and players of both
//! languages in one game, over the in-memory transport, on worlds made
//! without the game's files (an empty island), so these run anywhere:
//! - the host quits: the clients hear it was on purpose;
//! - the host's connection drops: after the grace period the host is
//!   gone for good, and within it the host can come back, the client
//!   reading its world again;
//! - a German host and an English client: the world, its sabotage table
//!   with it, stays the host's language's, the same on both, and only a
//!   table that really differs is refused, by name.

use std::time::Duration;

use formats::savegame::SaveGame;
use game::lang::Language;
use game::sabotage::SHARK_DIVER;
use game::seats::HOST_SEAT;
use game::world::{Tables, World};
use net::hash::{data_hashes, world_hash};
use net::memory::{Hub, MemoryTransport};
use net::{Config, End, Event, Session, world_from_snapshot};

const ROOM: &str = "isle";

/// A world without game files: an empty island in `language`, from a
/// folder of `folder`, as a snapshot reads it (so that the next one
/// reads back the same), running.
fn world(language: Language, folder: Language) -> World {
    let mut w = World {
        language,
        folder_language: folder,
        ..World::default()
    };
    let save = SaveGame::parse(&w.snapshot().to_bytes()).unwrap();
    w = w.load_snapshot(&save).unwrap();
    w.clock.paused = false;
    w
}

/// A peer: its session, its world once it has one, what it was told.
struct Peer {
    session: Session<MemoryTransport>,
    world: Option<World>,
    events: Vec<Event>,
}

impl Peer {
    fn host(hub: &Hub, world: World, config: Config) -> Peer {
        Peer {
            session: Session::host(hub.transport(), ROOM, config).unwrap(),
            world: Some(world),
            events: Vec::new(),
        }
    }

    /// A client in co-op in seat 1 (an empty island has no computers).
    fn client(hub: &Hub, config: Config) -> Peer {
        let config = Config {
            seat: Some(HOST_SEAT),
            ..config
        };
        Peer {
            session: Session::join(hub.transport(), ROOM, config).unwrap(),
            world: None,
            events: Vec::new(),
        }
    }

    /// Takes in what came; a snapshot is the world, read over `base`.
    fn poll(&mut self, base: &World) {
        let events = self.session.poll(self.world.as_ref());
        for e in &events {
            if let Event::Snapshot { data, save, .. } = e {
                self.world = Some(world_from_snapshot(base, data, save).unwrap());
            }
        }
        self.events.extend(events);
    }

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

    fn world(&self) -> &World {
        self.world.as_ref().expect("a world")
    }

    fn ended(&self) -> Option<&End> {
        self.events.iter().find_map(|e| match e {
            Event::Ended(end) => Some(end),
            _ => None,
        })
    }
}

/// `n` rounds of the host running a tick and the client following; the
/// client's world, once it has one and runs, hashes as the host's.
fn run(host: &mut Peer, client: &mut Peer, base: &World, n: usize) {
    for _ in 0..n {
        host.poll(base);
        assert!(host.step());
        client.poll(base);
        while client.session.tick() < host.session.tick() && client.step() {}
        if client.world.is_some() && client.session.tick() == host.session.tick() {
            assert_eq!(world_hash(client.world()), world_hash(host.world()));
        }
    }
}

fn config() -> Config {
    Config {
        hash_every: 1,
        ..Config::default()
    }
}

#[test]
fn the_host_quitting_ends_the_game_on_purpose() {
    let hub = Hub::new();
    let base = world(Language::De, Language::De);
    let mut host = Peer::host(&hub, base.clone(), config());
    let mut client = Peer::client(&hub, config());
    run(&mut host, &mut client, &base, 20);
    assert!(client.world.is_some());
    host.session.leave();
    client.poll(&base);
    assert_eq!(client.ended(), Some(&End::HostQuit));
    assert!(client.session.ended());
}

#[test]
fn a_lost_host_is_gone_after_the_grace_period() {
    // No grace: at once.
    let hub = Hub::new();
    let base = world(Language::De, Language::De);
    let quick = Config {
        host_grace: Duration::ZERO,
        ..config()
    };
    let mut host = Peer::host(&hub, base.clone(), quick);
    let mut client = Peer::client(&hub, quick);
    run(&mut host, &mut client, &base, 10);
    hub.disconnect(host.session.transport().id());
    client.poll(&base);
    assert!(matches!(client.ended(), Some(End::HostLost(_))), "{:?}", client.events);

    // A short grace: the client waits, its world standing, then gives up.
    let hub = Hub::new();
    let grace = Duration::from_millis(50);
    let short = Config {
        host_grace: grace,
        ..config()
    };
    let mut host = Peer::host(&hub, base.clone(), short);
    let mut client = Peer::client(&hub, short);
    run(&mut host, &mut client, &base, 10);
    hub.disconnect(host.session.transport().id());
    client.poll(&base);
    assert!(client.events.contains(&Event::HostLost { grace }));
    assert_eq!(client.ended(), None);
    // It runs the goes it has (the input delay's), then waits.
    let mut ran = 0;
    while client.step() {
        ran += 1;
    }
    assert!(ran <= 3, "{ran}");
    let tick = client.session.tick();
    client.poll(&base);
    assert!(!client.step(), "no goes come");
    assert_eq!(client.session.tick(), tick);
    std::thread::sleep(grace * 2);
    client.poll(&base);
    assert!(matches!(client.ended(), Some(End::HostLost(_))), "{:?}", client.events);
}

#[test]
fn a_host_back_within_the_grace_sends_its_world_again() {
    let hub = Hub::new();
    let base = world(Language::De, Language::De);
    let long = Config {
        host_grace: Duration::from_secs(60),
        ..config()
    };
    let mut host = Peer::host(&hub, base.clone(), long);
    let mut client = Peer::client(&hub, long);
    run(&mut host, &mut client, &base, 30);
    let me = client.session.me().unwrap();
    hub.disconnect(host.session.transport().id());
    client.poll(&base);
    assert!(client.events.iter().any(|e| matches!(e, Event::HostLost { .. })));
    // The host comes back in the room, its game where it was (its own
    // session closed with the transport).
    host.poll(&base);
    assert!(host.session.ended());
    let world = host.world.take().unwrap();
    let mut host = Peer::host(&hub, world, long);
    client.events.clear();
    run(&mut host, &mut client, &base, 30);
    assert!(client.events.contains(&Event::HostBack), "{:?}", client.events);
    assert_eq!(client.session.me(), Some(me), "back as the client it was");
    assert_eq!(client.ended(), None);
    assert_eq!(client.session.tick(), host.session.tick());
    assert_eq!(world_hash(client.world()), world_hash(host.world()));
}

#[test]
fn a_german_host_and_an_english_client_play_together() {
    let hub = Hub::new();
    // The host's game folder and world are German; the client's folder
    // is English, and so is the world it would set up itself.
    let host_world = world(Language::De, Language::De);
    let client_base = world(Language::En, Language::En);
    let mut host = Peer::host(&hub, host_world.clone(), config());
    let mut client = Peer::client(&hub, config());
    for _ in 0..30 {
        host.poll(&host_world);
        assert!(host.step());
        client.poll(&client_base);
        while client.session.tick() < host.session.tick() && client.step() {}
        if client.world.is_some() {
            assert_eq!(world_hash(client.world()), world_hash(host.world()));
        }
    }
    let w = client.world();
    assert_eq!((w.language, w.folder_language), (Language::De, Language::De));
    // The world's sabotage table is the German one on both.
    assert_eq!(w.sabotage_kind(SHARK_DIVER).targets[0], 254);
    assert_eq!(host.world().sabotage_kind(SHARK_DIVER), w.sabotage_kind(SHARK_DIVER));
    assert!(client.events.iter().all(|e| !matches!(e, Event::Desync { .. })));
    assert!(host.events.iter().all(|e| !matches!(e, Event::Desync { .. })));
}

/// The two languages' sabotage tables differ only where the world never
/// reads (the shark diver's targets), and a world's is its own
/// language's, not its folder's.
#[test]
fn the_sabotage_table_is_the_worlds_language() {
    let de = game::sabotage::table(Language::De);
    let en = game::sabotage::table(Language::En);
    assert_eq!(de.len(), 9);
    let differ: Vec<usize> = (0..9).filter(|&k| de[k] != en[k]).collect();
    assert_eq!(differ, [usize::from(SHARK_DIVER)]);
    assert_eq!(de[3].cost, en[3].cost);
    assert_eq!((de[3].targets[0], en[3].targets[0]), (254, 255));
    let w = world(Language::En, Language::De);
    assert_eq!(w.sabotage_kind(SHARK_DIVER), en[3]);
    // A file loaded into it in the middle of the game takes its own
    // language, and with it its table.
    let german = world(Language::De, Language::De);
    let loaded = w
        .load_map(&SaveGame::parse(&german.snapshot().to_bytes()).unwrap())
        .unwrap();
    assert_eq!(loaded.sabotage_kind(SHARK_DIVER), de[3]);
}

/// Only what the world reads from the game folder must be the host's:
/// another folder language is fine, another table is refused, by name.
#[test]
fn only_world_tables_must_match() {
    let host = world(Language::De, Language::De);
    let save = SaveGame::parse(&host.snapshot().to_bytes()).unwrap();
    let data = data_hashes(&host);
    let english = world(Language::En, Language::En);
    let back = world_from_snapshot(&english, &data, &save).unwrap();
    assert_eq!(world_hash(&back), world_hash(&host));
    let mut other = english.clone();
    other.tables = std::sync::Arc::new(Tables {
        road_cost: host.tables.road_cost + 1,
        ..(*host.tables).clone()
    });
    let e = world_from_snapshot(&other, &data, &save).unwrap_err();
    assert!(e.contains("MODATYP.DAT"), "{e}");
    assert!(!e.contains("HAUSPREI"), "{e}");
}
