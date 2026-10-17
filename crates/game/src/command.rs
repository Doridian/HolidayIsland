//! Commands: what a player does to the world. The view never changes the
//! world itself; it hands the world a [`Command`], which
//! [`World::apply`] carries out between two ticks
//! (`docs/port-architecture.md`). So far the auction form's buttons and
//! keys, loading a game from the file menu, and the main screen's tools:
//! building, raising and lowering the land, demolishing and the road
//! (`docs/player-tools.md`).

use std::sync::Arc;

use formats::savegame::SaveGame;

use crate::accounts::BOOK_OTHER;
use crate::auction;
use crate::roads::RoadError;
use crate::simulation::{Status, Tick};
use crate::world::{Pos, World, on_map};

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
    /// A build tool's click (the build menus' tools 9 to 20, palms too):
    /// `building` with its first square at `at`. // 1028:3cb1
    Build { building: u8, at: Pos },
    /// The raise (6) or lower (7) tool's click: one step of `corner`, the
    /// money rising from `at`, the square under the mouse.
    /// // 1028:3b7c, 1028:3a68
    Level { raise: bool, corner: Pos, at: Pos },
    /// The demolish tool's (8) click. // 1028:3c90
    Demolish(Pos),
    /// The road tool's (16) press: the drag starts at `at`. // 1080:3195
    RoadDragStart(Pos),
    /// Its release: the road from `from` towards `to`, if the drag is
    /// still on. // 1080:31ed
    RoadDragEnd { from: Pos, to: Pos },
    /// A button of the button bar ends the drag (1010:2b60, 1010:2c8b).
    RoadDragCancel,
}

/// The sound of a tool that did its work. // 1028:3b7c, 1028:3cb1
const TOOL_SOUND: u8 = 0x1f;

impl World {
    /// Carries out `command`, between two ticks, and returns what it did
    /// for the view, like a tick.
    pub fn apply(&mut self, command: Command) -> Tick {
        let mut tick = Tick::default();
        let player = command.player;
        match command.action {
            Action::Auction(a) => self.auction_command(player, a, &mut tick),
            Action::Load(map) => match self.load_map(&map) {
                Ok(world) => {
                    *self = world;
                    tick.loaded = true;
                    tick.redraw_all = true;
                }
                // BUG(original): with I/O checks off, a short or missing file stops the reads at the error and half loads the game; the port loads nothing.
                Err(_) => tick.load_failed = true,
            },
            Action::Build { building, at } => self.build_tool(player, building, at, &mut tick),
            Action::Level { raise, corner, at } => self.level_tool(player, raise, corner, at, &mut tick),
            Action::Demolish(at) => {
                // 1078:280b shows its errors itself, to player 1.
                if let Err(e) = self.demolish(player, at, &mut tick) {
                    tick.status.push((player, Status::Error(e)));
                }
            }
            Action::RoadDragStart(at) => self.road_drag_start(at),
            Action::RoadDragEnd { from, to } => self.road_drag_end(player, from, to, &mut tick),
            Action::RoadDragCancel => self.road_dragging = false,
        }
        tick
    }

    /// The message for the building on `pos`, which is in the way.
    /// // 1030:235a
    fn in_the_way(&self, player: u8, pos: Pos, tick: &mut Tick) {
        let building = self.island.buildings.get(pos).unwrap_or(crate::world::NO_BUILDING);
        tick.status.push((player, Status::InTheWay(building)));
    }

    /// `player` builds `building` at `at` if [`World::can_build`] lets
    /// them, with the budget 11; else the error. // 1028:3cb1
    fn build_tool(&mut self, player: u8, building: u8, at: Pos, tick: &mut Tick) {
        let (error, cost, in_the_way) = self.can_build_at(at, building, player); // 1030:04e3
        match error {
            0 => {
                tick.sounds.push(TOOL_SOUND); // 10a0:30b6
                self.start_building(cost, 0xb, player, building, at, tick); // 1078:1d21
            }
            1 => self.in_the_way(player, in_the_way, tick), // 1030:235a
            e => tick.status.push((player, Status::Error(e))), // 1030:232d
        }
    }

    /// One step of `corner` up or down for `player`, who pays for it: the
    /// books' "other" get the cost and it rises from `at`. The original
    /// also takes the tool's pointer off the island (`*1110:6fd6 + 0x17 :=
    /// 0`, the view's). Single human: the books are player 1's, and only
    /// player 1 needs the money (`crate::terrain`). // 1028:3b7c, 1028:3a68
    fn level_tool(&mut self, player: u8, raise: bool, corner: Pos, at: Pos, tick: &mut Tick) {
        let ((error, cost), in_the_way) = if raise {
            self.raise_corner(player, corner, tick) // 10b0:069f
        } else {
            self.lower_corner(player, corner, tick) // 10b0:1907
        };
        match error {
            0 => {
                self.add_money(player, cost.wrapping_neg()); // 1078:01e7
                if player == 1 {
                    self.book(BOOK_OTHER, cost.wrapping_neg()); // 1070:1419
                    tick.books = true; // 1070:2617
                    self.float_money(cost, at); // 1088:0153
                }
                tick.sounds.push(TOOL_SOUND); // 10a0:30b6
            }
            1 => self.in_the_way(player, in_the_way, tick), // 1030:235a
            e => tick
                .status
                .push((player, Status::Error(u8::try_from(e).unwrap_or(u8::MAX)))), // 1030:232d
        }
    }

    /// The road tool goes down on `at`: the drag is on, and its ground and
    /// form are kept, if `at` is on the map. The square itself is the
    /// view's (`1110:6344`). // 1080:3195
    fn road_drag_start(&mut self, at: Pos) {
        if on_map(at) {
            self.road_dragging = true;
            self.road_drag_ground = self.ground_or_water(at); // 1000:3e0d
            self.road_drag_form = self.form_or_flat(at); // 1000:3e8d
        }
    }

    /// The road tool comes up: with the drag on, the road from `from`
    /// towards `to` ([`World::build_road`], from the drag's ground), and
    /// its messages. // 1080:31ed
    fn road_drag_end(&mut self, player: u8, from: Pos, to: Pos, tick: &mut Tick) {
        if !self.road_dragging {
            return;
        }
        match self.build_road(player, from, to, self.road_drag_ground, tick) {
            Ok(_) | Err(RoadError::NotBuilt) => {}
            Err(RoadError::NotEnoughMoney) => tick.status.push((player, Status::Error(0xe))),
            Err(RoadError::InTheWay(pos)) => self.in_the_way(player, pos, tick),
            Err(RoadError::River) => tick.status.push((player, Status::Error(0x1f))),
        }
    }
}
