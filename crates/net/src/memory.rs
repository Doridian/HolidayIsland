//! A transport in one process, for tests: a [`Hub`] holds the rooms, and
//! each [`MemoryTransport`] of it is a peer. Like the WebRTC transport's
//! signaling, a room is a star: the host and each client are connected,
//! the clients not with each other. Messages arrive in order, on the
//! next [`Transport::poll`]. A test can cut a peer off
//! ([`Hub::disconnect`]) as a dropped connection would.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use crate::transport::{PeerId, Role, Transport, TransportEvent};

/// The rooms and the peers' queues.
#[derive(Default)]
struct Rooms {
    next: u128,
    /// The peers in each room: the host, and the clients.
    by_name: BTreeMap<String, (Option<PeerId>, Vec<PeerId>)>,
    /// What each peer will get on its next poll.
    queues: BTreeMap<PeerId, VecDeque<TransportEvent>>,
    /// Each peer's room and role.
    members: BTreeMap<PeerId, (String, Role)>,
    /// Pairs that are connected: (host, client).
    links: Vec<(PeerId, PeerId)>,
}

impl Rooms {
    fn push(&mut self, to: PeerId, event: TransportEvent) {
        self.queues.entry(to).or_default().push_back(event);
    }

    fn link(&mut self, host: PeerId, client: PeerId) {
        self.links.push((host, client));
        self.push(host, TransportEvent::Connected(client));
        self.push(client, TransportEvent::Connected(host));
    }

    fn linked(&self, a: PeerId, b: PeerId) -> bool {
        self.links.iter().any(|&(h, c)| (h, c) == (a, b) || (h, c) == (b, a))
    }

    /// `peer` is gone: its links drop, and it leaves its room.
    fn drop_peer(&mut self, peer: PeerId) {
        let (gone, kept): (Vec<_>, Vec<_>) = self.links.iter().partition(|&&(h, c)| h == peer || c == peer);
        self.links = kept;
        for (h, c) in gone {
            let other = if h == peer { c } else { h };
            self.push(other, TransportEvent::Disconnected(peer));
        }
        if let Some((room, _)) = self.members.remove(&peer)
            && let Some((host, clients)) = self.by_name.get_mut(&room)
        {
            if *host == Some(peer) {
                *host = None;
            }
            clients.retain(|&c| c != peer);
        }
    }
}

/// The rooms of one process's peers.
#[derive(Clone, Default)]
pub struct Hub(Rc<RefCell<Rooms>>);

impl Hub {
    #[must_use]
    pub fn new() -> Hub {
        Hub::default()
    }

    /// A new peer, not in any room yet.
    #[must_use]
    pub fn transport(&self) -> MemoryTransport {
        let id = {
            let mut r = self.0.borrow_mut();
            r.next += 1;
            PeerId(r.next)
        };
        MemoryTransport {
            hub: self.clone(),
            id,
            max_message: usize::MAX,
        }
    }

    /// Cuts `peer` off, as a dropped connection: the others it was
    /// connected to get [`TransportEvent::Disconnected`], and it gets
    /// [`TransportEvent::Closed`].
    pub fn disconnect(&self, peer: PeerId) {
        let mut r = self.0.borrow_mut();
        r.drop_peer(peer);
        r.push(peer, TransportEvent::Closed("cut off".to_owned()));
    }
}

/// A peer of a [`Hub`].
pub struct MemoryTransport {
    hub: Hub,
    id: PeerId,
    max_message: usize,
}

impl MemoryTransport {
    /// This peer's id, as the others see it.
    #[must_use]
    pub fn id(&self) -> PeerId {
        self.id
    }

    /// Messages bigger than `max` are refused (dropped): to test that the
    /// session splits them.
    #[must_use]
    pub fn with_max_message(mut self, max: usize) -> Self {
        self.max_message = max;
        self
    }
}

impl Transport for MemoryTransport {
    fn connect(&mut self, room: &str, role: Role) -> Result<(), String> {
        let mut r = self.hub.0.borrow_mut();
        let entry = r.by_name.entry(room.to_owned()).or_default();
        let (host, clients) = (entry.0, entry.1.clone());
        match role {
            Role::Host => {
                if host.is_some() {
                    return Err(format!("room {room} has a host"));
                }
                if let Some(e) = r.by_name.get_mut(room) {
                    e.0 = Some(self.id);
                }
                for c in clients {
                    r.link(self.id, c);
                }
            }
            Role::Client => {
                if let Some(e) = r.by_name.get_mut(room) {
                    e.1.push(self.id);
                }
                if let Some(h) = host {
                    r.link(h, self.id);
                }
            }
        }
        r.members.insert(self.id, (room.to_owned(), role));
        Ok(())
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        self.hub
            .0
            .borrow_mut()
            .queues
            .get_mut(&self.id)
            .map(|q| q.drain(..).collect())
            .unwrap_or_default()
    }

    fn send(&mut self, peer: PeerId, data: &[u8]) {
        if data.len() > self.max_message {
            return;
        }
        let mut r = self.hub.0.borrow_mut();
        if r.linked(self.id, peer) {
            r.push(peer, TransportEvent::Message(self.id, data.to_vec()));
        }
    }

    fn peers(&self) -> Vec<PeerId> {
        let r = self.hub.0.borrow();
        r.links
            .iter()
            .filter_map(|&(h, c)| {
                if h == self.id {
                    Some(c)
                } else if c == self.id {
                    Some(h)
                } else {
                    None
                }
            })
            .collect()
    }

    fn max_message(&self) -> usize {
        self.max_message
    }

    fn close(&mut self) {
        self.hub.0.borrow_mut().drop_peer(self.id);
    }
}

impl Drop for MemoryTransport {
    fn drop(&mut self) {
        self.close();
    }
}
