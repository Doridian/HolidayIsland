//! The WebRTC transport through the signaling server (`crates/server`),
//! all on this machine: the server runs in the test, and the host and
//! two clients connect through it on localhost, with no STUN or TURN
//! server. Then a session over it: a client joins a scenario's game and
//! plays along (skipped when `orig/` isn't there).

#![cfg(feature = "matchbox")]

mod common;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use game::enhance::Fixes;
use net::hash::world_hash;
use net::matchbox::{IceServers, MatchboxTransport};
use net::{Config, Event, PeerId, Role, Session, Transport, TransportEvent, world_from_snapshot};

/// How long the peers get to find each other.
const PATIENCE: Duration = Duration::from_secs(30);

/// The signaling server, in the test, on a free port of localhost.
fn start_server() -> SocketAddr {
    let mut server = server::server(SocketAddr::from(([127, 0, 0, 1], 0)));
    let addr = server.bind().expect("a free port");
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("a runtime");
        let _ = runtime.block_on(server.serve());
    });
    addr
}

fn transport(addr: SocketAddr) -> MatchboxTransport {
    MatchboxTransport::new(&format!("ws://{addr}"), IceServers::default())
}

#[test]
fn the_host_and_two_clients_connect() {
    assert_eq!(net::matchbox::HOST_SUFFIX, server::HOST_SUFFIX);
    let addr = start_server();
    // A client first: it waits for the host.
    let mut a = transport(addr);
    a.connect("room1", Role::Client).unwrap();
    let mut host = transport(addr);
    host.connect("room1", Role::Host).unwrap();
    let mut b = transport(addr);
    b.connect("room1", Role::Client).unwrap();
    let start = Instant::now();
    let mut host_peers = BTreeSet::new();
    let (mut a_host, mut b_host) = (None, None);
    while (host_peers.len() < 2 || a_host.is_none() || b_host.is_none()) && start.elapsed() < PATIENCE {
        for e in host.poll() {
            if let TransportEvent::Connected(p) = e {
                host_peers.insert(p);
            }
        }
        for (t, found) in [(&mut a, &mut a_host), (&mut b, &mut b_host)] {
            for e in t.poll() {
                if let TransportEvent::Connected(p) = e {
                    *found = Some(p);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(host_peers.len(), 2, "the host meets both clients");
    let (a_host, b_host) = (a_host.expect("A meets the host"), b_host.expect("B meets the host"));
    assert_eq!(a_host, b_host, "the clients meet the host only");

    // A second host of the room is turned away.
    let mut usurper = transport(addr);
    usurper.connect("room1", Role::Host).unwrap();
    let start = Instant::now();
    let mut usurper_closed = false;
    while !usurper_closed && start.elapsed() < PATIENCE {
        usurper_closed = usurper.poll().iter().any(|e| matches!(e, TransportEvent::Closed(_)));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(usurper_closed, "a second host is turned away");

    // Messages both ways, in order, the biggest a frame takes.
    let big: Vec<u8> = (0..net::wire::FRAME).map(|i| u8::try_from(i % 251).unwrap()).collect();
    host.broadcast(&big);
    host.broadcast(b"second");
    a.send(a_host, b"from a");
    b.send(b_host, b"from b");
    let start = Instant::now();
    let mut got: Vec<(char, Vec<u8>)> = Vec::new();
    while got.len() < 6 && start.elapsed() < PATIENCE {
        for (who, t) in [('h', &mut host), ('a', &mut a), ('b', &mut b)] {
            for e in t.poll() {
                if let TransportEvent::Message(_, m) = e {
                    got.push((who, m));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    for who in ['a', 'b'] {
        let mine: Vec<&Vec<u8>> = got.iter().filter(|(w, _)| *w == who).map(|(_, m)| m).collect();
        assert_eq!(mine, [&big, &b"second".to_vec()], "{who}");
    }
    let host_got: BTreeSet<&[u8]> = got
        .iter()
        .filter(|(w, _)| *w == 'h')
        .map(|(_, m)| m.as_slice())
        .collect();
    assert_eq!(host_got, BTreeSet::from([&b"from a"[..], &b"from b"[..]]));

    // A goes: the host hears of it.
    drop(a);
    let start = Instant::now();
    let mut gone: Option<PeerId> = None;
    while gone.is_none() && start.elapsed() < PATIENCE {
        for e in host.poll() {
            if let TransportEvent::Disconnected(p) = e {
                gone = Some(p);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(gone.is_some_and(|p| host_peers.contains(&p)), "the host hears A went");
}

/// A session over WebRTC: a client joins the host's scenario and plays
/// along, the same world.
#[test]
fn a_session_over_webrtc() {
    if !common::orig().join("SZENARIO").exists() {
        eprintln!("skipping: {} not found", common::orig().display());
        return;
    }
    let addr = start_server();
    let config = Config::default();
    let base = common::scenario(Fixes::ORIGINAL);
    let mut host_world = base.clone();
    let mut host = Session::host(transport(addr), "room2", config).unwrap();
    let mut client = Session::join(transport(addr), "room2", config).unwrap();
    let mut client_world = None;
    let start = Instant::now();
    let mut compared = 0;
    while compared < 30 && start.elapsed() < PATIENCE * 2 {
        let _ = host.poll(Some(&host_world));
        let step = host.next_step(&host_world).unwrap();
        for c in step.commands {
            host_world.apply_client(c);
        }
        host_world.tick();
        host.finish_step(&host_world);
        for e in client.poll(client_world.as_ref()) {
            match e {
                Event::Snapshot { data, save, .. } => {
                    client_world = Some(world_from_snapshot(&base, data, &save).unwrap());
                }
                Event::Desync { .. } | Event::Ended(_) | Event::Rejected(_) => panic!("{e:?}"),
                _ => {}
            }
        }
        if let Some(w) = &mut client_world {
            while client.tick() < host.tick()
                && let Some(step) = client.next_step(w)
            {
                for c in step.commands {
                    w.apply_client(c);
                }
                w.tick();
                client.finish_step(w);
            }
            if client.tick() == host.tick() {
                assert_eq!(world_hash(w), world_hash(&host_world));
                compared += 1;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(compared >= 30, "the client played along");
    assert!(!host_world.is_computer(2), "the client plays seat 2");
}
