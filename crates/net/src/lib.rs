//! Network multiplayer for the Holiday Island port
//! (`docs/networking.md`): humans take the computers' seats of a game
//! over the network, in deterministic lockstep, with the host's world
//! the authority.
//!
//! - [`transport`]: what the session needs of a network, as a trait
//!   that never blocks; [`matchbox`] (WebRTC data channels, natively and
//!   in the browser) and [`memory`] (one process, for tests) are two.
//! - [`session`]: the lockstep session on top: the host stamps the
//!   clients' commands with the tick they're carried out on and sends
//!   every tick's go; joining clients get a snapshot of the world.
//! - [`wire`]: the messages as bytes, versioned.
//! - [`hash`]: the world's hash, the same on every target, for desync
//!   checks.
//!
//! The world changes only by its commands (`game::command`,
//! `World::apply_client`), on every peer alike; this crate only carries
//! them.

pub mod hash;
#[cfg(feature = "matchbox")]
pub mod matchbox;
pub mod memory;
pub mod session;
pub mod transport;
pub mod wire;

pub use session::{Config, Event, Session, Step, world_from_snapshot};
pub use transport::{PeerId, Role, Transport, TransportEvent};
