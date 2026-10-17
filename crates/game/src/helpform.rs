//! `THelpform`, the tooltips (segment `1040`): a 150 × 16 bubble,
//! `Help.bmp` with one of `SONSTIGE.TXT`'s short texts centred on it. With
//! the tooltips option on (`1110:0060`), every tick of the main screen's
//! timer (step 3 of `AnimationsTimer`, 1020:1e9b) shows it for the money
//! menu's buttons (1070:0294), the overview map's (10b8:02e9) or the
//! button bar's (1018:394e) under the mouse, and hides it when none is.
//! It's never enabled, so it doesn't take the mouse.
//!
//! [`Helpform::show`] and [`Helpform::hide`] are the bubble; the forms'
//! lookups give the text and where it goes: [`button_bar_tooltip`] for
//! the button bar, and the text tables of the money menu
//! ([`MONEY_MENU_TEXTS`]) and the overview map ([`OVERVIEW_TEXTS`]) for
//! when those are ported.

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::text::{Align, Font};

/// The bubble's bitmap, from `GRAFIK`. // +0x180, 1040:3cf9
pub const BITMAP_FILE: &str = "Help.bmp";
/// The image's name in the DFM.
pub const IMAGE: &str = "Bild";
/// The bubble's size. // 1040:3d02
pub const WIDTH: i32 = 0x96;
pub const HEIGHT: i32 = 0x10;

/// The short text for each of the button bar's 23 buttons. // 1110:163c
pub const BUTTON_TEXTS: [u8; 23] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
];
/// The short text for the money menu's buttons 9 to 14 (`1110:1627 +
/// button`). // 1110:1630
pub const MONEY_MENU_TEXTS: [u8; 6] = [0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22];
/// The short text for the overview map's buttons 16 to 20 (`1110:1626 +
/// button`). // 1110:1636
pub const OVERVIEW_TEXTS: [u8; 5] = [0x18, 0x19, 0x1a, 0x1b, 0x1c];

/// The button bar's buttons' left and right edges in `ButtonImage`.
/// // 1110:12c0
const BUTTON_EDGES: [(i32, i32); 23] = [
    (0, 24),
    (25, 49),
    (50, 74),
    (88, 112),
    (113, 137),
    (138, 162),
    (176, 200),
    (201, 225),
    (226, 250),
    (264, 288),
    (289, 313),
    (314, 338),
    (339, 363),
    (364, 388),
    (389, 413),
    (414, 438),
    (439, 463),
    (464, 488),
    (489, 513),
    (514, 538),
    (539, 563),
    (577, 601),
    (615, 639),
];

/// `button_at_mouse`: the button whose edges hold `x` (`mouse_x`, the
/// last mouse move's x on `ButtonImage` or `Hauptbild`), the first one
/// that does; `None` (23) between the groups. // 1018:394e
#[must_use]
pub fn button_at_mouse(x: i32) -> Option<u8> {
    let mut found = None;
    for (b, &(left, right)) in BUTTON_EDGES.iter().enumerate() {
        if left <= x && x <= right {
            found = u8::try_from(b).ok();
            break;
        }
    }
    found
}

/// Step 3's part for the button bar (1020:1e9b): with the main screen
/// (`form`, on the screen) or the bubble active, the main screen enabled
/// and its `+0x1da` clear (`ready`), and the `cursor` (on the screen) in
/// the button bar's rows, the button under `mouse_x` gives the text, and
/// the bubble goes under the bar at the button's left edge: `(text, top,
/// left)` for [`Helpform::show`].
#[must_use]
pub fn button_bar_tooltip(form: Rect, cursor: (i32, i32), mouse_x: i32, ready: bool) -> Option<(u8, i32, i32)> {
    if !ready {
        return None;
    }
    let (x, y) = cursor;
    let in_bar = form.top + 0x22 <= y && y <= form.top + 0x3b && form.left <= x && x <= form.right;
    if !in_bar {
        return None;
    }
    let b = button_at_mouse(mouse_x)?;
    let left = BUTTON_EDGES[usize::from(b)].0 + form.left;
    Some((BUTTON_TEXTS[usize::from(b)], form.top + 0x3b, left))
}

/// The bubble.
#[derive(Debug, Clone)]
pub struct Helpform {
    font: Font,
    bitmap: Surface,
    /// `SONSTIGE.TXT`'s short texts. // 1110:737a
    texts: Vec<Vec<u8>>,
    /// `Bild`'s picture.
    image: Surface,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
    /// The form's height: 0 hides it, 16 shows it. // +0x24
    pub height: i32,
    /// The text shown. // +0x184
    text: u8,
    /// Set to 10 whenever the bubble is asked for, and never read.
    /// // 1110:009c
    // BUG(original): the bubble's timeout is set but never counted down or read.
    pub timeout: i16,
}

impl Helpform {
    /// `FormCreate`: `Help.bmp`, the image 150 × 16 at the top left and
    /// the form 0 high, shown. `texts` are `SONSTIGE.TXT`'s short texts.
    /// // 1040:3d02
    #[must_use]
    pub fn new(font: Font, bitmap: Surface, texts: Vec<Vec<u8>>) -> Self {
        Helpform {
            font,
            bitmap,
            texts,
            image: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            left: 0,
            top: 0,
            height: 0,
            text: 0,
            timeout: 0,
        }
    }

    /// Whether the bubble shows.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.height != 0
    }

    /// The image's picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// The form on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// `helpform_show`: unless it already shows `text`, the bubble with
    /// short text `text` centred in colour 9, at `top`, `left` but no
    /// further down and right than fits on the `screen`; it goes in
    /// front, disabled. Whether it changed. // 1040:3dc7
    pub fn show(&mut self, text: u8, top: i32, left: i32, screen: (i32, i32)) -> bool {
        self.timeout = 10;
        if self.height != 0 && self.text == text {
            return false;
        }
        self.text = text;
        let clip = self.image.bounds();
        self.image.blit(0, 0, &self.bitmap, clip, Rop::Copy, clip);
        let s = self.texts.get(usize::from(text)).map_or(&[][..], |t| &t[..]);
        self.font.draw(&mut self.image, s, (0, 1), WIDTH, Align::Center, 9);
        self.top = (screen.1 - 0x10).min(top);
        self.left = (screen.0 - WIDTH).min(left);
        self.height = HEIGHT;
        true
    }

    /// `helpform_hide`: the form 0 high. Whether it showed. // 1040:3ead
    pub fn hide(&mut self) -> bool {
        let shown = self.height != 0;
        self.height = 0;
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_bar() {
        assert_eq!(button_at_mouse(0), Some(0));
        assert_eq!(button_at_mouse(24), Some(0));
        assert_eq!(button_at_mouse(80), None);
        assert_eq!(button_at_mouse(639), Some(22));
        let form = Rect::new(80, 60, 640, 480);
        assert_eq!(
            button_bar_tooltip(form, (100, 60 + 0x22), 90, true),
            Some((3, 60 + 0x3b, 168))
        );
        assert_eq!(button_bar_tooltip(form, (100, 60 + 0x3c), 90, true), None);
        assert_eq!(button_bar_tooltip(form, (100, 100), 80, true), None);
        assert_eq!(button_bar_tooltip(form, (100, 100), 90, false), None);
    }
}
