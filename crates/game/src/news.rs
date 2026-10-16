//! The news: messages about what happens on the island (1048:0a54). The
//! original logs each in the news archive (`*1110:663a`, the last 25)
//! and, depending on the player's options, shows it on the TV, which
//! pauses the game. The world's part is choosing the text, with a random
//! number; the rest is the view's.

use crate::simulation::Tick;
use crate::world::{Pos, World};

/// A message for the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Message {
    /// The message: its media (`NACHRICH.TXT`'s names) and its kind of
    /// news, which decides whether the TV shows it (`1110:199e`).
    /// // archive +0x12
    pub number: i16,
    /// Which of `NACHRICH.TXT`'s texts it shows: one of the message's three,
    /// `number * 3 + Random(3)`. // archive +0x10
    pub text: i16,
    /// Values the text fills in: a player, another player or a number, an
    /// amount, another amount. // archive +4, +6, +8, +0xc
    pub a: i16,
    pub b: i16,
    pub x: i32,
    pub y: i32,
    /// Where it happened. `None` where the original passes the player's
    /// view origin (`1110:5530`), which is the view's. // archive +0x114
    pub pos: Option<Pos>,
    /// Scroll the view to `pos` before showing it.
    pub centre: bool,
}

impl World {
    /// Posts `message`, picking its text (`Random(3)`); `message.text` is
    /// ignored. // 1048:0a54, 2197 bytes
    ///
    /// The original also keeps it in the news archive with the date and
    /// the view's origin, and shows it on the TV (1048:12e9, which pauses
    /// the game) if the player's options for its kind of news say so.
    /// TODO: the archive (`*1110:663a`, saved in the map) belongs in the
    /// world once something reads it.
    pub fn post_message(&mut self, mut message: Message, tick: &mut Tick) {
        let variant = self.random.random(3).cast_signed();
        message.text = message.number.wrapping_mul(3).wrapping_add(variant);
        tick.messages.push(message);
    }
}
