//! The signaling server of the port's network games
//! (`docs/networking.md`), on matchbox's signaling library. The game's
//! peers find each other here and agree on their WebRTC connections;
//! the game's own messages then go between them directly (or through a
//! TURN server), not through this server.
//!
//! The topology is a star per room ([`Star`]): a room is the URL's path,
//! the host connects to `/<room>~host` and the clients to `/<room>`. The
//! host hears of every client of its room (which it then offers a
//! connection to), each client of the host only. A client that comes
//! before the host waits for it. A second host of a room is turned away
//! (its connection closes). When the host leaves, its clients hear so,
//! and stay for a new host.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::extract::ws::Message;
use futures::StreamExt;
use matchbox_protocol::{JsonPeerEvent, PeerId, PeerRequest};
use matchbox_signaling::common_logic::{SignalingChannel, parse_request, try_send};
use matchbox_signaling::{
    ClientRequestError, NoCallbacks, SignalingServer, SignalingServerBuilder, SignalingState, SignalingTopology,
    WsStateMeta,
};

/// The host's suffix on its room's path.
pub const HOST_SUFFIX: &str = "~host";

/// The default port.
pub const PORT: u16 = 3536;

/// The rooms: the host, if it's there, and the clients.
#[derive(Default)]
struct Room {
    host: Option<(PeerId, SignalingChannel)>,
    clients: BTreeMap<PeerId, SignalingChannel>,
}

/// The server's state: every room by name.
#[derive(Clone, Default)]
pub struct StarState(Arc<Mutex<BTreeMap<String, Room>>>);

impl SignalingState for StarState {}

fn event(e: &JsonPeerEvent) -> Message {
    Message::Text(e.to_string().into())
}

impl StarState {
    fn rooms(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Room>> {
        // A thread that panicked with the lock leaves the rooms as they
        // were: still usable.
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `peer` comes into `room`; false if it's a host and the room has
    /// one.
    fn enter(&self, room: &str, peer: PeerId, sender: SignalingChannel, host: bool) -> bool {
        let mut rooms = self.rooms();
        let r = rooms.entry(room.to_owned()).or_default();
        if host {
            if r.host.is_some() {
                return false;
            }
            // The clients that waited: the host offers each a connection.
            for &client in r.clients.keys() {
                let _ = try_send(&sender, event(&JsonPeerEvent::NewPeer(client)));
            }
            r.host = Some((peer, sender));
        } else {
            if let Some((_, h)) = &r.host {
                let _ = try_send(h, event(&JsonPeerEvent::NewPeer(peer)));
            }
            r.clients.insert(peer, sender);
        }
        true
    }

    /// `from` signals `to` (an offer, an answer, a candidate): only
    /// between the host and a client of the same room.
    fn signal(&self, room: &str, from: PeerId, host: bool, to: PeerId, data: serde_json::Value) {
        let rooms = self.rooms();
        let Some(r) = rooms.get(room) else { return };
        let channel = if host {
            r.clients.get(&to)
        } else {
            r.host.as_ref().filter(|(h, _)| *h == to).map(|(_, c)| c)
        };
        if let Some(c) = channel {
            let _ = try_send(c, event(&JsonPeerEvent::Signal { sender: from, data }));
        }
    }

    /// `peer` left `room`: the host hears of a client, the clients of
    /// the host.
    fn leave(&self, room: &str, peer: PeerId, host: bool) {
        let mut rooms = self.rooms();
        let Some(r) = rooms.get_mut(room) else { return };
        let left = event(&JsonPeerEvent::PeerLeft(peer));
        if host {
            if r.host.as_ref().is_some_and(|(h, _)| *h == peer) {
                r.host = None;
                for c in r.clients.values() {
                    let _ = try_send(c, left.clone());
                }
            }
        } else if r.clients.remove(&peer).is_some()
            && let Some((_, h)) = &r.host
        {
            let _ = try_send(h, left);
        }
        if r.host.is_none() && r.clients.is_empty() {
            rooms.remove(room);
        }
    }
}

/// A star per room: the host in the middle (see the crate's docs).
#[derive(Debug, Default)]
pub struct Star;

#[async_trait]
impl SignalingTopology<NoCallbacks, StarState> for Star {
    async fn state_machine(upgrade: WsStateMeta<NoCallbacks, StarState>) {
        let WsStateMeta {
            room,
            peer_id,
            sender,
            mut receiver,
            state,
            ..
        } = upgrade;
        let (room, host) = match room.strip_suffix(HOST_SUFFIX) {
            Some(r) => (r.to_owned(), true),
            None => (room, false),
        };
        if !state.enter(&room, peer_id, sender, host) {
            eprintln!("{peer_id}: room {room} has a host already");
            return;
        }
        eprintln!("{peer_id}: in room {room}{}", if host { " as its host" } else { "" });
        while let Some(request) = receiver.next().await {
            match parse_request(request) {
                Ok(PeerRequest::Signal { receiver, data }) => state.signal(&room, peer_id, host, receiver, data),
                Ok(PeerRequest::KeepAlive)
                | Err(ClientRequestError::Json(_) | ClientRequestError::UnsupportedType(_)) => {}
                Err(_) => break,
            }
        }
        eprintln!("{peer_id}: left room {room}");
        state.leave(&room, peer_id, host);
    }
}

/// The server, at `addr`, not bound yet.
#[must_use]
pub fn server(addr: SocketAddr) -> SignalingServer {
    SignalingServerBuilder::new(addr, Star, StarState::default()).build()
}
