//! `TKompassform`, the compass (segment `1038`): a small form at the top
//! right of the screen with four buttons that turn the main screen's view,
//! one for each side the island is seen from. The knob in the main
//! screen's bottom right corner (`eckimage`, a widget at
//! `THauptschirm+0x40c`) shows and hides it (1020:3214, 1020:0456); it's
//! kept here with the form.
//!
//! The form is one `TImage`, `Bild`, 60 × 76: `komp.bmp` with the
//! toolkit's widgets on it (four radio buttons and a close button). The
//! title row drags it like the other forms'. Its keys go to the main
//! screen. Turning is the view's: a button comes back as
//! [`Response::view`], for the caller to turn the main screen
//! (`set_view`, 1010:254d). Nothing here changes the world.

use vcl::Rect;
use vcl::surface::Surface;

use crate::enhance::{Enhancement, Enhancements};
use crate::text::{Align, Font};
use crate::widgets::{BitmapId, Button, CheckBox, Frame, Label, Outcome, RadioButton, ShiftState, Skin, Widget};

/// The bitmaps, from `GRAFIK`, in the order [`Kompassform::new`] takes
/// them: the form's (1038:2c04), then the close button's and the knob's
/// (1020:2501).
pub const BITMAP_FILES: [&str; 13] = [
    "komp.bmp",     // the background, +0x180
    "kompsu.bmp",   // view 0, +0x184
    "kompsupr.bmp", // +0x188
    "kompos.bmp",   // view 3, +0x18c
    "kompospr.bmp", // +0x190
    "kompno.bmp",   // view 2, +0x194
    "kompnopr.bmp", // +0x198
    "kompwe.bmp",   // view 1, +0x19c
    "kompwepr.bmp", // +0x1a0
    "close.bmp",    // 1110:54b8
    "closepre.bmp", // 1110:54bc
    "kompkn.bmp",   // the knob, THauptschirm+0x1a8
    "kompknpr.bmp", // pressed, +0x1ac
];
const BACKGROUND: usize = 0;
const CLOSE: BitmapId = BitmapId(9);
const CLOSE_PRESSED: BitmapId = BitmapId(10);
const KNOB: BitmapId = BitmapId(11);
const KNOB_PRESSED: BitmapId = BitmapId(12);

/// The form's size. // 1038:2c04
pub const WIDTH: i32 = 0x3c;
pub const HEIGHT: i32 = 0x4c;
/// The height while its title row is dragged.
const TITLE_HEIGHT: i32 = 0x10;

/// The radio buttons, in the order the form keeps and tries them
/// (+0x1a4, +0x2bc, +0x3d4, +0x4ec): the view each turns to, its top
/// and left, and its bitmaps, off and on. // 1038:2c04
const RADIOS: [(u8, i32, i32, usize); 4] = [
    (0, 0x35, 0x14, 1), // south, at the bottom
    (3, 0x24, 0x25, 3), // east, at the right
    (2, 0x13, 0x14, 5), // north, at the top
    (1, 0x24, 3, 7),    // west, at the left
];

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// `Bild` changed.
    pub refresh: bool,
    /// The knob in the main screen's corner changed: draw it again
    /// ([`Kompassform::draw_knob`]).
    pub knob: bool,
    /// The sounds played, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<u8>,
    /// Turn the main screen's view to this: `set_view` (1010:254d).
    pub view: Option<u8>,
    /// The form was shown, hidden or moved.
    pub moved: bool,
}

impl Response {
    fn outcome(&mut self, o: Outcome) {
        if let Some(s) = o.sound {
            self.sounds.push(u8::try_from(s.number()).unwrap_or(0));
        }
        self.refresh |= o.repaint;
    }
}

/// The compass.
#[derive(Debug, Clone)]
pub struct Kompassform {
    font: Font,
    bitmaps: Vec<Surface>,
    /// `Bild`'s picture.
    pub image: Surface,
    /// The form's top left on the screen, and its height: 0 while hidden.
    pub left: i32,
    pub top: i32,
    pub height: i32,
    radios: [RadioButton; 4],
    /// +0x604
    close: Button,
    /// The main screen's knob (`THauptschirm+0x40c`).
    knob: CheckBox,
    /// The title row is held. // +0x71a
    dragging: bool,
    /// Where it was grabbed, from the form's top left. // +0x71b
    grab: (i32, i32),
    /// The height before the drag shrank it. // +0x723
    saved_height: i32,
    /// The last press on the title row was a double click: the form stays
    /// shrunk. // +0x725
    double: bool,
    /// The player's view enhancements (`form_drags`).
    pub enhance: Enhancements,
}

fn label(left: i32, top: i32, size: i32) -> Label {
    Label::new(Frame::new(left, top, size, size), b"", Align::Left, 9)
}

impl Kompassform {
    /// The form as the game creates it (1038:2c04): `komp.bmp` and the
    /// four buttons drawn, all off, hidden at the top right of a screen
    /// `screen_width` wide; and the knob (1020:2501), off. `bitmaps` are
    /// those of [`BITMAP_FILES`].
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, screen_width: i32) -> Self {
        let radios = RADIOS
            .map(|(_, top, left, off)| RadioButton::new(label(left, top, 0x14), BitmapId(off), BitmapId(off + 1)));
        let close = Button::new(Frame::new(0, 0, 0x10, 0x10), CLOSE, CLOSE_PRESSED, b"", 9);
        let mut knob = CheckBox::new(label(0, 0, 0x10), KNOB, KNOB_PRESSED, 9);
        knob.radio.frame.visible = true;
        let mut k = Kompassform {
            font,
            bitmaps,
            image: Surface::new(0x3c, 0x4c, 0),
            left: screen_width - WIDTH,
            top: 0,
            height: 0,
            radios,
            close,
            knob,
            dragging: false,
            grab: (0, 0),
            saved_height: HEIGHT,
            double: false,
            enhance: Enhancements::ORIGINAL,
        };
        if let Some(b) = k.bitmaps.get(BACKGROUND) {
            let clip = k.image.bounds();
            k.image.draw(0, 0, b, clip);
        }
        let skin = Skin {
            font: &k.font,
            bitmaps: &k.bitmaps,
        };
        for r in &mut k.radios {
            r.show(&mut k.image, &skin);
        }
        k
    }

    /// Whether the form is shown.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.height > 0
    }

    /// The form on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// Whether the knob is pressed (the form open).
    #[must_use]
    pub fn knob_on(&self) -> bool {
        self.knob.checked()
    }

    /// Draws the knob into the main screen's `eckimage`. // 1010:16b5
    pub fn draw_knob(&self, corner: &mut Surface) {
        let skin = Skin {
            font: &self.font,
            bitmaps: &self.bitmaps,
        };
        self.knob.radio.draw(corner, &skin);
    }

    /// The button for `view` on, the others off, all drawn.
    /// // 1038:3256 (and 1038:314f)
    fn set_view(&mut self, view: u8) {
        let skin = Skin {
            font: &self.font,
            bitmaps: &self.bitmaps,
        };
        for (r, &(v, ..)) in self.radios.iter_mut().zip(&RADIOS) {
            r.on = v == view & 3;
            r.draw(&mut self.image, &skin);
        }
    }

    /// The main screen turned with the keys (`hauptschirm_key_turn`,
    /// 1020:2d31) or a game was shown (`game_show_state`, only while the
    /// form is shown): the buttons show `view`. // 1038:3256
    pub fn update(&mut self, view: u8) -> bool {
        self.set_view(view);
        true
    }

    /// Shows the form: the close button and the buttons for `view`.
    /// // 1038:314f
    fn show(&mut self, view: u8) {
        let skin = Skin {
            font: &self.font,
            bitmaps: &self.bitmaps,
        };
        self.close.show(&mut self.image, &skin);
        self.set_view(view);
        self.height = HEIGHT;
    }

    /// `eckimage`'s `OnMouseDown` at `x`, `y`: the knob toggles, and the
    /// form shows with the main screen's `view` or hides. // 1020:3214,
    /// 1020:0456
    pub fn knob_mouse_down(&mut self, x: i32, y: i32, view: u8) -> Response {
        let mut r = Response::default();
        let o = self.knob.mouse_down(x, y);
        r.outcome(o);
        r.knob = o.repaint;
        if o.clicked {
            if self.knob.checked() {
                self.show(view);
            } else {
                self.height = 0; // 1038:3243
            }
            r.moved = true;
            r.refresh = true;
        }
        r
    }

    /// `BildMouseDown` at `x`, `y` in the form, `cursor` the mouse on the
    /// screen: a button turns the view, the close button goes down, or the
    /// title row right of it is grabbed. // 1038:3329
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState, cursor: (i32, i32)) -> Response {
        let mut r = Response::default();
        self.dragging = false;
        #[allow(clippy::needless_range_loop)]
        for n in 0..self.radios.len() {
            let o = self.radios[n].mouse_down(x, y);
            if o.handled {
                r.outcome(o);
                if o.clicked {
                    // 1038:29e3, 1038:2a41, 1038:2a9f, 1038:2afd: the
                    // view, and the other buttons off.
                    r.view = Some(RADIOS[n].0);
                    for (k, other) in self.radios.iter_mut().enumerate() {
                        if k != n {
                            r.outcome(other.deselect());
                        }
                    }
                }
                self.draw_widgets();
                return r;
            }
        }
        let o = self.close.mouse_down(x, y); // 1010:14c1
        if o.handled {
            r.outcome(o);
            self.draw_widgets();
            return r;
        }
        if x > 0xf && x <= WIDTH && (0..0x10).contains(&y) {
            self.dragging = true;
            self.grab = (cursor.0 - self.left, cursor.1 - self.top);
            if !self.double {
                self.saved_height = self.height;
                self.height = TITLE_HEIGHT;
                r.moved = true;
            }
            self.double = shift.contains(ShiftState::DOUBLE);
        }
        r
    }

    /// `BildMouseMove`: the grabbed form follows the left button's drag,
    /// on the `screen`. The grab isn't let go on the mouse up, only by the
    /// next press. // 1038:346f
    pub fn mouse_move(&mut self, shift: ShiftState, cursor: (i32, i32), screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        // BUG(original): the drag flag stays set after the mouse up, so a left drag that starts elsewhere and comes over the compass moves it.
        if self.dragging && shift.contains(ShiftState::LEFT) {
            self.left = (cursor.0 - self.grab.0).max(0).min(screen.0 - WIDTH);
            self.top = (cursor.1 - self.grab.1).max(0).min(screen.1 - self.height);
            r.moved = true;
        }
        r
    }

    /// `BildMouseUp` at `x`, `y`: the close button comes up and, over it,
    /// hides the form and lets the knob up; after a drag the form gets its
    /// height back unless the press was a double click. With `form_drags`
    /// the drag ends. // 1038:34fc
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        let dragging = self.dragging;
        if self.enhance.has(Enhancement::FormDrags) {
            self.dragging = false;
        }
        let o = self.close.mouse_up(x, y); // 1010:14f7
        if o.handled {
            r.outcome(o);
            if o.clicked {
                // 1038:2b5b
                self.height = 0;
                let _ = self.knob.radio.deselect();
                r.knob = true;
                r.moved = true;
            }
            self.draw_widgets();
        } else if dragging && !self.double {
            self.height = self.saved_height;
            r.moved = true;
        }
        r
    }

    fn draw_widgets(&mut self) {
        let skin = Skin {
            font: &self.font,
            bitmaps: &self.bitmaps,
        };
        for w in &self.radios {
            w.draw(&mut self.image, &skin);
        }
        if self.close.frame.visible {
            self.close.draw(&mut self.image, &skin);
        }
    }
}
