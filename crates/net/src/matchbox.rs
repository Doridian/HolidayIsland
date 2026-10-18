//! The WebRTC transport, with matchbox: data channels between the host
//! and each client, which find each other through a signaling server
//! (`crates/server`, `docs/networking.md`). The same socket runs natively
//! and in the browser.
//!
//! The socket's message loop is a future that must run all the time.
//! Natively it gets a thread of its own with a small tokio runtime (the
//! WebRTC stack wants tokio); the socket itself stays with the game, and
//! [`Transport::poll`] reads it without blocking. The browser build
//! will spawn the future with `wasm-bindgen-futures` instead, the one
//! part of this that isn't written yet.
//!
//! The room is the signaling server's path: `<signal>/<room>` for a
//! client and `<signal>/<room>~host` for the host, which the server
//! makes the star's middle.

use std::collections::BTreeMap;
use std::sync::mpsc;

use matchbox_socket::{ChannelConfig, MessageLoopFuture, PeerState, RtcIceServerConfig, WebRtcSocket};

use crate::transport::{PeerId, Role, Transport, TransportEvent};

/// The host's suffix on the room's path.
pub const HOST_SUFFIX: &str = "~host";

/// The ICE servers: STUN to find a way through the players' routers,
/// TURN (coturn) to relay for those who can't connect directly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IceServers {
    /// `stun:host:port` URLs.
    pub stun: Vec<String>,
    /// `turn:host:port` or `turns:host:port` URLs.
    pub turn: Vec<String>,
    /// The TURN server's user name and password.
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl IceServers {
    /// Checks the URLs' schemes: a bad URL would only fail later, in the
    /// socket's thread.
    ///
    /// # Errors
    /// The first URL that isn't a STUN or TURN one.
    pub fn check(&self) -> Result<(), String> {
        for url in &self.stun {
            if !url.starts_with("stun:") && !url.starts_with("stuns:") {
                return Err(format!("{url} isn't a stun: URL"));
            }
        }
        for url in &self.turn {
            if !url.starts_with("turn:") && !url.starts_with("turns:") {
                return Err(format!("{url} isn't a turn: or turns: URL"));
            }
        }
        if !self.turn.is_empty() && (self.username.is_none() || self.credential.is_none()) {
            return Err("a TURN server needs a user name and a password".to_owned());
        }
        Ok(())
    }

    fn config(&self) -> RtcIceServerConfig {
        RtcIceServerConfig {
            urls: self.stun.iter().chain(&self.turn).cloned().collect(),
            username: self.username.clone(),
            credential: self.credential.clone(),
        }
    }
}

/// The room's URL on the signaling server `signal` (`ws://host:port` or
/// `wss://...`).
#[must_use]
pub fn room_url(signal: &str, room: &str, role: Role) -> String {
    let base = signal.trim_end_matches('/');
    match role {
        Role::Host => format!("{base}/{room}{HOST_SUFFIX}"),
        Role::Client => format!("{base}/{room}"),
    }
}

/// Whether `room` is a name the transport takes: letters, digits, `-`
/// and `_`, 1 to 64 of them.
#[must_use]
pub fn valid_room(room: &str) -> bool {
    (1..=64).contains(&room.len())
        && room
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A peer of a room on a matchbox signaling server.
pub struct MatchboxTransport {
    signal: String,
    ice: IceServers,
    socket: Option<WebRtcSocket>,
    /// Matchbox's ids by ours.
    ids: BTreeMap<PeerId, matchbox_socket::PeerId>,
    /// Why the message loop ended, once it has.
    done: Option<mpsc::Receiver<String>>,
    closed: bool,
}

impl MatchboxTransport {
    /// A transport through the signaling server at `signal` with the ICE
    /// servers `ice`; it connects on [`Transport::connect`].
    #[must_use]
    pub fn new(signal: &str, ice: IceServers) -> Self {
        MatchboxTransport {
            signal: signal.to_owned(),
            ice,
            socket: None,
            ids: BTreeMap::new(),
            done: None,
            closed: false,
        }
    }

    fn ours(&mut self, id: matchbox_socket::PeerId) -> PeerId {
        let ours = PeerId(id.0.as_u128());
        self.ids.insert(ours, id);
        ours
    }
}

impl Transport for MatchboxTransport {
    fn connect(&mut self, room: &str, role: Role) -> Result<(), String> {
        if self.socket.is_some() {
            return Err("already connected".to_owned());
        }
        if !valid_room(room) {
            return Err(format!("{room:?} isn't a room name: letters, digits, - and _ only"));
        }
        if !self.signal.starts_with("ws://") && !self.signal.starts_with("wss://") {
            return Err(format!("{} isn't a ws:// or wss:// URL", self.signal));
        }
        self.ice.check()?;
        let (socket, message_loop) = WebRtcSocket::builder(room_url(&self.signal, room, role))
            .ice_server(self.ice.config())
            .reconnect_attempts(Some(3))
            .add_channel(ChannelConfig::reliable())
            .build();
        self.done = Some(spawn(message_loop)?);
        self.socket = Some(socket);
        Ok(())
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        let mut out = Vec::new();
        if self.closed {
            return out;
        }
        let Some(socket) = &mut self.socket else { return out };
        let (changes, gone) = match socket.try_update_peers() {
            Ok(changes) => (changes, false),
            Err(_) => (Vec::new(), true),
        };
        let mut messages = Vec::new();
        if let Ok(channel) = socket.get_channel_mut(0) {
            messages = channel.receive();
        }
        for (id, state) in changes {
            let peer = self.ours(id);
            out.push(match state {
                PeerState::Connected => TransportEvent::Connected(peer),
                PeerState::Disconnected => TransportEvent::Disconnected(peer),
            });
        }
        for (id, packet) in messages {
            let peer = self.ours(id);
            out.push(TransportEvent::Message(peer, packet.into_vec()));
        }
        let ended = self.done.as_ref().and_then(|d| match d.try_recv() {
            Ok(why) => Some(why),
            Err(mpsc::TryRecvError::Disconnected) => Some("the connection's thread stopped".to_owned()),
            Err(mpsc::TryRecvError::Empty) => None,
        });
        if let Some(why) = ended.or_else(|| gone.then(|| "the socket closed".to_owned())) {
            self.closed = true;
            out.push(TransportEvent::Closed(why));
        }
        out
    }

    fn send(&mut self, peer: PeerId, data: &[u8]) {
        if let (Some(socket), Some(&id)) = (&mut self.socket, self.ids.get(&peer))
            && let Ok(channel) = socket.get_channel_mut(0)
        {
            let _ = channel.try_send(data.into(), id);
        }
    }

    fn peers(&self) -> Vec<PeerId> {
        self.socket
            .as_ref()
            .map(|s| s.connected_peers().map(|id| PeerId(id.0.as_u128())).collect())
            .unwrap_or_default()
    }

    fn max_message(&self) -> usize {
        crate::wire::FRAME
    }

    fn close(&mut self) {
        if let Some(socket) = &mut self.socket {
            socket.close();
        }
        self.closed = true;
    }
}

impl Drop for MatchboxTransport {
    fn drop(&mut self) {
        self.close();
    }
}

/// Runs the socket's message loop on a thread of its own, on a
/// one-thread tokio runtime; the receiver gets why it ended.
#[cfg(not(target_arch = "wasm32"))]
fn spawn(message_loop: MessageLoopFuture) -> Result<mpsc::Receiver<String>, String> {
    let (tx, rx) = mpsc::channel();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("no runtime for the connection: {e}"))?;
    std::thread::Builder::new()
        .name("matchbox".to_owned())
        .spawn(move || {
            let why = match runtime.block_on(message_loop) {
                Ok(()) => "the signaling server closed the connection".to_owned(),
                Err(e) => format!("{e}"),
            };
            let _ = tx.send(why);
        })
        .map_err(|e| format!("no thread for the connection: {e}"))?;
    Ok(rx)
}

/// The browser build will spawn the message loop with
/// `wasm_bindgen_futures::spawn_local` (`docs/networking.md`).
#[cfg(target_arch = "wasm32")]
fn spawn(_message_loop: MessageLoopFuture) -> Result<mpsc::Receiver<String>, String> {
    Err("the browser build can't connect yet".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_and_urls() {
        assert_eq!(room_url("ws://x:3536/", "abc", Role::Host), "ws://x:3536/abc~host");
        assert_eq!(room_url("ws://x:3536", "abc", Role::Client), "ws://x:3536/abc");
        assert!(valid_room("Game_1-a"));
        assert!(!valid_room("a/b") && !valid_room("") && !valid_room("x~host"));
        let mut ice = IceServers {
            stun: vec!["stun:example.org:3478".to_owned()],
            ..IceServers::default()
        };
        assert_eq!(ice.check(), Ok(()));
        ice.turn.push("turn:example.org:3478".to_owned());
        assert!(ice.check().is_err());
        ice.username = Some("u".to_owned());
        ice.credential = Some("p".to_owned());
        assert_eq!(ice.check(), Ok(()));
        ice.stun.push("http://x".to_owned());
        assert!(ice.check().is_err());
    }
}
