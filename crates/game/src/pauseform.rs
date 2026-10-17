//! `TPauseform`, the pause (segment `1040`): the button bar's second
//! button (button 1, or the keys `2`, Pause and `P`) creates it and shows
//! it (1010:2c8b, 1040:3a45): the picture `Pause1.BMP` in the middle of
//! the screen. A click on the picture (`PausebildClick`, 1040:3bf4) or any
//! key (`FormKeyDown`, 1040:3c05) hides and frees it again (1040:3bac).
//!
//! The pause is the main screen's clock standing (`AnimationsTimer` off,
//! 1020:24a0, and on again, 1020:24b7) while the other forms are disabled,
//! not a flag of the world (the TV's pause is `1110:0067`): the form gives
//! the world no commands. Single human: in the original this stops the
//! game for the one player; in a multiplayer game a pause would have to be
//! a command that stops the world for everybody, or not stop it at all.

use vcl::Rect;
use vcl::surface::{Rop, Surface};

/// The bitmap the form shows, from `GRAFIK`. // 1040:3a3a
pub const BITMAP_FILES: [&str; 1] = ["Pause1.BMP"];

/// The form's size, which 1040:3a45 sets over the DFM's, and
/// `Pausebild`'s, which fills it.
pub const WIDTH: i32 = 0x15e;
pub const HEIGHT: i32 = 0xd2;

/// `Pausebild`'s bounds in the form. // 1040:3a45
pub const LAYOUT: [(&str, Rect); 1] = [("Pausebild", Rect::new(0, 0, WIDTH, HEIGHT))];

/// What the form asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The pause ends: the form hides, the button bar's button comes up,
    /// the other forms are enabled again, the main screen gets the focus
    /// (its VMT+0x78) and the clock runs again. // 1040:3bac
    Close,
}

/// The pause form.
#[derive(Debug, Clone)]
pub struct Pauseform {
    bitmaps: Vec<Surface>,
    /// `Pausebild`'s picture.
    image: Surface,
    /// The form is shown.
    pub visible: bool,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
}

impl Pauseform {
    /// The form, not shown; `bitmaps` are those of [`BITMAP_FILES`].
    #[must_use]
    pub fn new(bitmaps: Vec<Surface>) -> Self {
        Pauseform {
            bitmaps,
            image: Surface::new(0x15e, 0xd2, 0),
            visible: false,
            left: 0,
            top: 0,
        }
    }

    /// The button bar's button creates the form and shows it: in the middle
    /// of a `screen` of that size, with the top left of `Pause1.BMP` in its
    /// picture. The original first stops the clock, hides the button help
    /// and disables the other forms; that's the caller's. // 1040:3a45
    pub fn show(&mut self, screen: (i32, i32)) {
        self.top = screen.1 / 2 - 0x69;
        self.left = screen.0 / 2 - 0xaf;
        let clip = self.image.bounds();
        if let Some(b) = self.bitmaps.first() {
            self.image.blit(0, 0, b, clip, Rop::Copy, clip);
        }
        self.visible = true;
    }

    /// `PausebildClick`: a click on the picture ends the pause. // 1040:3bf4
    #[must_use]
    pub fn click(&self) -> Action {
        Action::Close
    }

    /// `FormKeyDown`: so does any key. // 1040:3c05
    #[must_use]
    pub fn key_down(&self, _key: u16) -> Action {
        Action::Close
    }

    /// `Pausebild`'s picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// Each image with the name of the `TImage` it goes into.
    #[must_use]
    pub fn images(&self) -> [(&'static str, &Surface); 1] {
        [("Pausebild", &self.image)]
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, HEIGHT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centred() {
        let mut f = Pauseform::new(vec![Surface::new(400, 300, 7)]);
        f.show((800, 600));
        assert_eq!(f.bounds(), Rect::new(225, 195, WIDTH, HEIGHT));
        assert_eq!(f.image().pixel(349, 209), 7);
        assert_eq!(f.key_down(0x41), Action::Close);
        assert_eq!(f.click(), Action::Close);
    }
}
