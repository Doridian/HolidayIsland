//! Commands: what a player does to the world. The view never changes the
//! world itself; it hands the world a [`Command`], which
//! [`World::apply`] carries out between two ticks
//! (`docs/port-architecture.md`). So far only the auction form's buttons
//! and keys are commands.

use crate::auction;
use crate::simulation::Tick;
use crate::world::World;

/// `player` does `action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Command {
    /// 1 for the human in the original.
    pub player: u8,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// On the auction form ([`World::auction_command`]).
    Auction(auction::Action),
}

impl World {
    /// Carries out `command`, between two ticks, and returns what it did
    /// for the view, like a tick.
    pub fn apply(&mut self, command: Command) -> Tick {
        let mut tick = Tick::default();
        match command.action {
            Action::Auction(a) => self.auction_command(command.player, a, &mut tick),
        }
        tick
    }
}
