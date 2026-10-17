//! Commands: what a player does to the world. The view never changes the
//! world itself; it hands the world a [`Command`], which
//! [`World::apply`] carries out between two ticks
//! (`docs/port-architecture.md`). So far the auction form's buttons and
//! keys, and loading a game from the file menu.

use std::sync::Arc;

use formats::savegame::SaveGame;

use crate::auction;
use crate::simulation::Tick;
use crate::world::World;

/// `player` does `action`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Command {
    /// 1 for the human in the original.
    pub player: u8,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    /// On the auction form ([`World::auction_command`]).
    Auction(auction::Action),
    /// The file menu's "Laden": `load_map` reads the parsed file over the
    /// world in the middle of the game, and what the file doesn't hold
    /// stays as this game left it ([`World::load_map`]). Single human: in
    /// the original only the one player loads; in a multiplayer game it
    /// would be the host's, and replaces everyone's world. // 1040:0f27
    Load(Arc<SaveGame>),
}

impl World {
    /// Carries out `command`, between two ticks, and returns what it did
    /// for the view, like a tick.
    pub fn apply(&mut self, command: Command) -> Tick {
        let mut tick = Tick::default();
        match command.action {
            Action::Auction(a) => self.auction_command(command.player, a, &mut tick),
            Action::Load(map) => match self.load_map(&map) {
                Ok(world) => {
                    *self = world;
                    tick.loaded = true;
                    tick.redraw_all = true;
                }
                // BUG(original): with I/O checks off, a short or missing file stops the reads at the error and half loads the game; the port loads nothing.
                Err(_) => tick.load_failed = true,
            },
        }
        tick
    }
}
