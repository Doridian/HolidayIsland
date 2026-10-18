//! What the session needs of a network: messages of bytes to and from
//! peers, and which peers come and go. A [`Transport`] never blocks: the
//! game loop calls [`Transport::poll`] once a frame and gets what came
//! since, and sending queues the message. The WebRTC transport
//! (`crate::matchbox`) and the in-memory one of the tests
//! (`crate::memory`) are two; a WebSocket relay could be a third, behind
//! the same trait (`docs/networking.md`).
//!
//! Each peer's messages arrive whole, in the order they were sent, and
//! none is lost while the peer stays connected: the reliable, ordered
//! channel. A transport may limit how big one message is
//! ([`Transport::max_message`]); the session splits bigger ones
//! (`crate::wire`).

use std::fmt;

/// A peer of the transport: who a message is from or for. The
/// transport hands them out; one that reconnects is a new peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(pub u128);

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// Which end of the star this peer is: the host, which every client
/// connects to, or a client, which connects to the host only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Client,
}

/// What [`Transport::poll`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportEvent {
    /// A peer can now be sent to.
    Connected(PeerId),
    /// A peer's connection dropped; nothing more comes from it.
    Disconnected(PeerId),
    /// A message from a peer.
    Message(PeerId, Vec<u8>),
    /// The transport itself is gone (the signaling server refused or
    /// lost us), with why.
    Closed(String),
}

/// A connection to a room of peers.
pub trait Transport {
    /// Starts connecting to `room` as `role`. Returns at once: the
    /// peers come as [`TransportEvent::Connected`] from
    /// [`Transport::poll`].
    ///
    /// # Errors
    /// If the transport can't start (a bad address, say).
    fn connect(&mut self, room: &str, role: Role) -> Result<(), String>;

    /// What happened since the last call. Never blocks.
    fn poll(&mut self) -> Vec<TransportEvent>;

    /// Queues `data` for `peer`. A peer that isn't connected drops it.
    fn send(&mut self, peer: PeerId, data: &[u8]);

    /// The peers connected now.
    fn peers(&self) -> Vec<PeerId>;

    /// Queues `data` for every connected peer.
    fn broadcast(&mut self, data: &[u8]) {
        for peer in self.peers() {
            self.send(peer, data);
        }
    }

    /// The biggest message the transport takes in one piece.
    fn max_message(&self) -> usize {
        usize::MAX
    }

    /// Closes the connection to every peer.
    fn close(&mut self);
}
