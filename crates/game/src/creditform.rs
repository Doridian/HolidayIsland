//! `TCreditform`, the credits (segment `1030`): the options' "INFO" (or
//! Space) creates it and shows it (1060:01ab, `creditform_show`,
//! 1030:3c42). It's one `TImage`, `Bild`, 640 × 480 like the form,
//! centred on the screen, showing `credits.bmp`, whose picture has the
//! names, a title bar and a close button's look. The form disables the
//! options while it's up; its close button closes it again (1030:3c0f,
//! 1030:3e6f) and enables them.
//!
//! `creditform_show` makes two buttons of the game's toolkit (the widget
//! VMT `1110:3d5c`, whose `Click` is 1030:3c0f): the close button at the
//! top left, which it shows, and "Schließen" (`SONSTIGE.TXT`'s 14th text)
//! at 248, 452, which it never shows. A hidden widget ignores the mouse,
//! so only the close button closes the form. The form has no keys.

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::enhance::{Enhancement, Enhancements};
use crate::text::Font;
use crate::widgets::{BitmapId, Button, Frame, ShiftState, Skin, Sound, Widget};

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Creditform::new`] takes them. // 1030:3c42
pub const BITMAP_FILES: [&str; 5] = [
    "credits.bmp", // the picture, +0x180, 1030:3c21
    "stkn.bmp",    // "Schließen", +0x184, 1030:3c2d
    "stknpr.bmp",  // held down, +0x188, 1030:3c36
    // `hintergrund`'s, which state 100 loads.
    "close.bmp",    // 1110:54b8
    "closepre.bmp", // 1110:54bc
];
const PICTURE: BitmapId = BitmapId(0);
const BUTTON: BitmapId = BitmapId(1);
const BUTTON_PRESSED: BitmapId = BitmapId(2);
const CLOSE: BitmapId = BitmapId(3);
const CLOSE_PRESSED: BitmapId = BitmapId(4);

/// The form's size, and its image's. // 1030:3c42
pub const WIDTH: i32 = 0x280;
pub const HEIGHT: i32 = 0x1e0;
/// The image's name in the DFM.
pub const IMAGE: &str = "Bild";

/// What the form asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The close button: the form hides, the options are enabled again,
    /// the bitmaps are freed and the form released. // 1030:3c0f, 1030:3e6f
    Close,
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the handler refreshes the image.
    pub refresh: bool,
    /// The buttons' sounds, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<Sound>,
    pub action: Option<Action>,
}

/// The credits.
#[derive(Debug, Clone)]
pub struct Creditform {
    font: Font,
    bitmaps: Vec<Surface>,
    /// `Bild`'s picture.
    image: Surface,
    pub visible: bool,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
    /// The close button, +0x2a2.
    close: Button,
    /// "Schließen", +0x18c: made, never shown.
    schliessen: Button,
    /// The player's view enhancements (`mouse_buttons`).
    pub enhance: Enhancements,
}

impl Creditform {
    /// The form, not shown. `bitmaps` are those of [`BITMAP_FILES`],
    /// `schliessen` `SONSTIGE.TXT`'s 14th text (`*1110:734e + 0xd00`).
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, schliessen: &[u8]) -> Self {
        Creditform {
            font,
            bitmaps,
            image: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            visible: false,
            left: 0,
            top: 0,
            close: close_button(),
            schliessen: Button::new(
                Frame::new(0xf8, 0x1c4, 0x90, 0x16),
                BUTTON,
                BUTTON_PRESSED,
                schliessen,
                9,
            ),
            enhance: Enhancements::ORIGINAL,
        }
    }

    /// `creditform_show`: new buttons, `Bild` 640 × 480 at the form's top
    /// left with `credits.bmp`, the form as big and centred on a `screen`
    /// of that size, and the close button shown. The original also
    /// disables the options (the caller's). // 1030:3c42
    pub fn show(&mut self, screen: (i32, i32)) -> Response {
        self.close = close_button();
        self.schliessen.frame.visible = false;
        self.schliessen.pressed = false;
        let clip = self.image.bounds();
        self.image.fill_rect(clip, clip, 0);
        if let Some(picture) = self.bitmaps.get(PICTURE.0) {
            self.image.blit(0, 0, picture, clip, Rop::Copy, clip);
        }
        self.top = (screen.1 - HEIGHT) / 2;
        self.left = (screen.0 - WIDTH) / 2;
        let Creditform {
            font,
            bitmaps,
            image,
            close,
            ..
        } = self;
        // BUG(original): "Schließen" (+0x18c) is made but never shown, so it can't be clicked; only the close button closes the credits. Kept: the picture's last line of names lies under it.
        close.show(image, &Skin { font, bitmaps });
        self.visible = true;
        Response {
            refresh: true,
            ..Response::default()
        }
    }

    /// `Bild`'s picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// The form on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, HEIGHT)
    }

    /// `BildMouseDown`: the close button, then "Schließen", are offered
    /// the press until one takes it, whichever mouse button it is (with
    /// `mouse_buttons` the left one only); `Bild` is refreshed if one
    /// did. // 1030:3ec4
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        let mut r = Response::default();
        // BUG(original): the handler doesn't look at the mouse button, so the right button presses the close button too.
        if self.enhance.has(Enhancement::MouseButtons) && !shift.contains(ShiftState::LEFT) {
            return r;
        }
        for button in [&mut self.close, &mut self.schliessen] {
            let out = button.mouse_down(x, y);
            if out.handled {
                r.sounds.extend(out.sound);
                r.refresh = true;
                break;
            }
        }
        self.redraw();
        r
    }

    /// `BildMouseUp`: the same for the release; a click on either button
    /// closes the form (1030:3c0f). // 1030:3f22
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        for button in [&mut self.close, &mut self.schliessen] {
            let out = button.mouse_up(x, y);
            if out.handled {
                r.sounds.extend(out.sound);
                r.refresh = true;
                if out.clicked {
                    self.visible = false;
                    r.action = Some(Action::Close);
                }
                break;
            }
        }
        self.redraw();
        r
    }

    /// The shown buttons drawn as they are now.
    fn redraw(&mut self) {
        let Creditform {
            font,
            bitmaps,
            image,
            close,
            schliessen,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        for b in [close, schliessen] {
            if b.frame.visible {
                b.draw(image, &skin);
            }
        }
    }
}

/// The close button as `creditform_show` makes it: 16 × 16 at the top
/// left, no text. // 1030:3c42
fn close_button() -> Button {
    Button::new(Frame::new(0, 0, 0x10, 0x10), CLOSE, CLOSE_PRESSED, b"", 9)
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::bmp::Bitmap;

    fn font() -> Font {
        let bitmap = Bitmap {
            width: 12,
            height: 14 * 12,
            palette: [vcl::Rgb::default(); 256],
            pixels: vec![0; 12 * 14 * 12],
        };
        Font::new(&bitmap, &[[0; 8]; 32], &[[1; 8]; 32])
    }

    fn form() -> Creditform {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(640, 480, u8::try_from(i + 1).unwrap()))
            .collect();
        Creditform::new(font(), bitmaps, b"Schlie\xdfen")
    }

    #[test]
    fn shows_centred_and_closes_by_its_button() {
        let mut f = form();
        let _ = f.show((800, 600));
        assert!(f.visible);
        assert_eq!(f.bounds(), Rect::new(80, 60, WIDTH, HEIGHT));
        // The picture, and the close button over its corner.
        assert_eq!(f.image().pixel(320, 240), 1);
        assert_eq!(f.image().pixel(4, 4), 4);
        // The hidden "Schließen" takes no click.
        let (x, y) = (0xf8 + 10, 0x1c4 + 5);
        assert_eq!(f.mouse_down(x, y, ShiftState::LEFT), Response::default());
        assert_eq!(f.mouse_up(x, y).action, None);
        assert!(f.visible);
        // The close button does.
        let r = f.mouse_down(4, 4, ShiftState::LEFT);
        assert_eq!(r.sounds, [Sound::Press]);
        assert_eq!(f.image().pixel(4, 4), 5, "drawn pressed");
        let r = f.mouse_up(4, 4);
        assert_eq!(r.action, Some(Action::Close));
        assert!(!f.visible);
    }

    #[test]
    fn right_button_with_the_fix() {
        let mut f = form();
        let _ = f.show((640, 480));
        assert_eq!(f.bounds().left, 0);
        let _ = f.mouse_down(4, 4, ShiftState::RIGHT);
        assert_eq!(f.mouse_up(4, 4).action, Some(Action::Close), "the original's");
        let _ = f.show((640, 480));
        f.enhance.set(Enhancement::MouseButtons, true);
        assert!(!f.mouse_down(4, 4, ShiftState::RIGHT).refresh);
        assert_eq!(f.mouse_up(4, 4).action, None);
    }
}
