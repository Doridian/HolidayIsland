//! A network game between two copies of the program in one process,
//! through the signaling server, run in the test, over WebRTC on this
//! machine: the host starts a game, the client joins it from the
//! command line's options, takes seat 2 and plays along; its commands
//! act for its seat on both; when the host's game ends, the client goes
//! back to its start menu. Skipped without the game's files in `orig/`.

use std::net::SocketAddr;
use std::time::Instant;

use super::*;

fn start_server() -> SocketAddr {
    let mut server = server::server(SocketAddr::from(([127, 0, 0, 1], 0)));
    let addr = server.bind().expect("a free port");
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("a runtime");
        let _ = runtime.block_on(server.serve());
    });
    addr
}

/// Both programs' frames until `done`, 100 ms of the game's time each,
/// for at most 30 s.
fn frames(host: &mut Holiday, client: &mut Holiday, done: impl Fn(&Holiday, &Holiday) -> bool) {
    let start = Instant::now();
    while !done(host, client) {
        assert!(start.elapsed() < Duration::from_secs(30), "took too long");
        let frame = Duration::from_millis(u64::from(TICK_MS));
        App::update(host, frame);
        App::update(client, frame);
        if client.loading.is_some() {
            client.finish_loading();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn tick_of(h: &Holiday) -> Option<u64> {
    h.game.as_ref()?.net.as_ref()?.session.tick()
}

#[test]
fn a_client_joins_the_hosts_game() {
    let Some(dir) = game_dir() else { return };
    let signal = format!("ws://{}", start_server());
    let args = |role| network::NetArgs {
        role,
        room: "test".to_owned(),
        signal: signal.clone(),
        ice: net::matchbox::IceServers::default(),
        config: net::Config::default(),
    };

    // The host starts a game from the start menu, and hosts it.
    let mut host = fixed_holiday(&dir);
    host.net_args = Some(args(network::Role::Host));
    host.respond(startform::Response {
        action: Some(Action::Start { scenario: false }),
        ..startform::Response::default()
    });
    host.start_pending();
    host.finish_loading();
    assert!(host.game.as_ref().unwrap().net.is_some(), "the host's game is hosted");
    // The client waits for the host's world and joins with it.
    let mut client = fixed_holiday(&dir);
    client.joining = Some(args(network::Role::Join).connect().unwrap());
    frames(&mut host, &mut client, |h, c| {
        tick_of(c).is_some() && tick_of(c) == tick_of(h) && h.game.as_ref().is_some_and(|g| g.world.seats.humans() == 2)
    });
    let (h, c) = (host.game.as_ref().unwrap(), client.game.as_ref().unwrap());
    assert_eq!(c.seat, 2, "the client's view plays seat 2");
    assert_eq!(c.tools.seat, 2);
    assert!(!h.world.is_computer(2));
    assert_eq!(world_hash(&c.world), world_hash(&h.world));

    // The client's command acts for seat 2, on both.
    let level = h.world.advertising_level(2);
    let more = level < 7;
    client.game.as_mut().unwrap().commands.push(Command {
        player: 1,
        action: command::Action::MoneyMenu(game::geldmenu::Action::Advertising { more }),
    });
    frames(&mut host, &mut client, |h, c| {
        let changed = c.game.as_ref().is_some_and(|g| g.world.advertising_level(2) != level);
        changed && tick_of(h) == tick_of(c)
    });
    let (h, c) = (host.game.as_ref().unwrap(), client.game.as_ref().unwrap());
    assert_eq!(h.world.advertising_level(2), if more { level + 1 } else { level - 1 });
    assert_eq!(world_hash(&c.world), world_hash(&h.world));

    // The host's game ends: the client's goes back to the start menu.
    host.back_to_start_menu();
    frames(&mut host, &mut client, |_, c| c.game.is_none());
    assert!(client.start.visible());
    for h in [&host, &client] {
        std::fs::remove_dir_all(&h.saves.dir).ok();
    }
}
