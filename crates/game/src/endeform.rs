//! `TEndeform`, "HOLIDAY ISLAND VERLASSEN?" (unit `1020`, with
//! `THauptschirm`): the question before the game quits. The main screen
//! opens it with Escape, `Q` or Alt+F4 (1020:2e09), its close button and
//! `FormClose` (1020:0480, 1020:0577), and the file menu with "Beenden"
//! or `Q` (1040:0a4c, 1040:1572). It doesn't open again while the game
//! is already quitting.
//!
//! The form is one `TImage`, `EndeImage`, with the background
//! `Exitform.BMP`; its two buttons, the close button and the title bar are
//! drawn on it and found by the rectangles of a table (`1110:18ba`), like
//! the file menu's. "OK" (or Enter) saves `Last.map` and quits through the
//! end film (`TSchlussform`, 1068:0f52); "Abbruch", the close button,
//! Escape or Space let the game go on.
//!
//! Showing it stops the clock and disables the other forms until it
//! closes: the simulation waits on this dialog. Single human: in the
//! original this stops the game for everyone.

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::enhance::{Enhancement, Enhancements};
use crate::text::{Align, Font};
use crate::widgets::{BitmapId, ShiftState, Sound};

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Endeform::new`] takes them.
pub const BITMAP_FILES: [&str; 3] = [
    "Exitform.BMP", // the background, 1110:6616, 1020:3b45
    "BilButPr.BMP", // a button held down, 1110:661a, 1020:3b52
    "closepre.bmp", // hintergrund's, 1110:54bc
];
const BACKGROUND: BitmapId = BitmapId(0);
const BUTTON_PRESSED: BitmapId = BitmapId(1);
const CLOSE_PRESSED: BitmapId = BitmapId(2);

/// The form's parts: left, top, right and bottom, inclusive. Entry 0 is
/// the form, 1 the close button, 2 the title bar, 3 the question; the
/// buttons have one rectangle to draw (`n`) and a pixel smaller one to
/// hit (`n + 1`). // 1110:18ba
pub const RECTS: [(i32, i32, i32, i32); 8] = [
    (0, 0, 184, 84),   // 1110:18ba
    (0, 0, 15, 15),    // 1110:18c2
    (16, 0, 184, 15),  // 1110:18ca
    (8, 24, 176, 50),  // 1110:18d2
    (10, 61, 88, 76),  // 1110:18da, "Abbruch"
    (10, 61, 87, 75),  // 1110:18e2
    (98, 61, 176, 76), // 1110:18ea, "OK"
    (98, 61, 175, 75), // 1110:18f2
];

/// The form's size: entry 0 of [`RECTS`] plus one (1020:3b5f).
pub const WIDTH: i32 = RECTS[0].2 + 1;
pub const HEIGHT: i32 = RECTS[0].3 + 1;
/// Its height while it's dragged by the title bar (1020:37dd).
pub const DRAG_HEIGHT: i32 = 0x10;

/// The image's name in the DFM.
pub const IMAGE: &str = "EndeImage";

const CLOSE: u8 = 1;
const TITLE: u8 = 2;
const QUESTION: u8 = 3;
const CANCEL: u8 = 4;
const OK: u8 = 6;

/// `SONSTIGE.TXT`'s texts: 1 the title, 2 the question, 3 "OK", 4
/// "Abbruch". // 1020:3aad
const TITLE_TEXT: usize = 1;
const QUESTION_TEXT: usize = 2;
const OK_TEXT: usize = 3;
const CANCEL_TEXT: usize = 4;

const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_F4: u16 = 0x73;

/// Whether a key on the main screen opens the form: Escape or `Q`, alone
/// or with Shift, or Alt+F4. The main screen's other keys are its own.
/// // 1020:2e09
#[must_use]
pub fn main_screen_opens(key: u16, shift: ShiftState) -> bool {
    let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
    match mods {
        0 | 1 => key == VK_ESCAPE || key == 0x51,
        2 => key == VK_F4,
        _ => false,
    }
}

/// What the form asks of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// "Abbruch", the close button, Escape or Space: the form hides and
    /// the clock runs again (1020:24b7). `FormHide` enables the other
    /// forms again (1020:3d05).
    Close,
    /// "OK" or Enter: the form hides, `save_map` writes `MAPS\Last.map`
    /// (1048:2bae) and the end film plays before the game exits
    /// ([`crate::schlussform::Schlussform::open_quit`], 1068:0f52). The
    /// form can't open again meanwhile ([`Endeform::quitting`]); the
    /// caller clears it with [`Endeform::done_quitting`] if the game
    /// doesn't exit.
    Quit,
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the handler refreshes the image.
    pub refresh: bool,
    /// The sounds played, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<Sound>,
    pub action: Option<Action>,
    /// The form moved or changed its height (dragged by its title bar).
    pub moved: bool,
}

/// The quit question.
#[derive(Debug, Clone)]
pub struct Endeform {
    font: Font,
    bitmaps: Vec<Surface>,
    empty: Surface,
    /// `SONSTIGE.TXT`'s texts.
    texts: Vec<Vec<u8>>,
    /// `EndeImage`'s picture.
    image: Surface,
    pub visible: bool,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
    /// The form's height: [`HEIGHT`], or [`DRAG_HEIGHT`] while it's
    /// dragged.
    pub height: i32,
    /// "OK" was chosen and the game is quitting: the form doesn't open
    /// again. // +0x180
    quitting: bool,
    /// The part the left button went down on, 0 for none. // 1110:661e
    pressed: u8,
    /// Where the mouse grabbed the title bar, from the form's top left.
    /// // 1110:6046, 1110:6048
    grab: (i32, i32),
    /// The player's view enhancements (`form_drags`).
    pub enhance: Enhancements,
}

impl Endeform {
    /// The form as it was created, not shown. `bitmaps` are those of
    /// [`BITMAP_FILES`], `texts` `SONSTIGE.TXT`'s.
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, texts: Vec<Vec<u8>>) -> Self {
        Endeform {
            font,
            bitmaps,
            empty: Surface::new(0, 0, 0),
            texts,
            image: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            visible: false,
            left: 0,
            top: 0,
            height: HEIGHT,
            quitting: false,
            pressed: 0,
            grab: (0, 0),
            enhance: Enhancements::ORIGINAL,
        }
    }

    /// The image's picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// The form on the screen: its height is [`DRAG_HEIGHT`] while it's
    /// dragged, so only the title bar shows.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// Whether the game is quitting ("OK" was chosen). // +0x180
    #[must_use]
    pub fn quitting(&self) -> bool {
        self.quitting
    }

    /// The quitting is over (the original exits, 1068:17ad, and clears
    /// the flag after `TSchlussform` returns, 1020:3a86).
    pub fn done_quitting(&mut self) {
        self.quitting = false;
    }

    /// The callers' `if not quitting then Show`: shows the form unless
    /// the game is already quitting. Whether it did. // 1020:0480, 1020:0577
    pub fn open(&mut self, screen: (i32, i32)) -> bool {
        if self.quitting {
            return false;
        }
        self.show(screen);
        true
    }

    /// `FormShow`: the form centred on the `screen`, its size from the
    /// table, the background with the texts. The original also stops the
    /// clock (1020:24a0) and disables the other forms (`disable_forms(3)`,
    /// 1010:2970); that's the caller's. // 1020:3b5f
    pub fn show(&mut self, screen: (i32, i32)) {
        // Single human: the clock stands while the one player is asked;
        // the simulation waits on this dialog.
        self.top = (screen.1 - RECTS[0].3) / 2;
        self.left = (screen.0 - RECTS[0].2) / 2;
        self.height = HEIGHT;
        self.pressed = 0;
        self.visible = true;
        let clip = self.image.bounds();
        let background = self.bitmaps.get(BACKGROUND.0).unwrap_or(&self.empty);
        self.image.blit(0, 0, background, clip, Rop::Copy, clip);
        self.draw_texts();
    }

    /// Hides the form. `FormHide` frees its bitmaps and enables the other
    /// forms (1020:3d05); that's the caller's.
    fn hide(&mut self) {
        self.visible = false;
    }

    fn text(&self, n: usize) -> Vec<u8> {
        self.texts.get(n).cloned().unwrap_or_default()
    }

    /// The buttons' texts, the title and the question. // 1020:3aad
    fn draw_texts(&mut self) {
        self.draw_text(OK, false);
        self.draw_text(CANCEL, false);
        let (l, t, r, _) = RECTS[usize::from(TITLE)];
        let title = self.text(TITLE_TEXT);
        self.font
            .draw(&mut self.image, &title, (l, t + 1), r - l, Align::Center, 1);
        let (l, t, r, _) = RECTS[usize::from(QUESTION)];
        let question = self.text(QUESTION_TEXT);
        self.font
            .draw(&mut self.image, &question, (l, t), r - l, Align::Center, 7);
    }

    /// A button's text: up (1020:3603), or pressed a pixel further right
    /// and down (1020:3588).
    fn draw_text(&mut self, part: u8, pressed: bool) {
        let n = match part {
            OK => OK_TEXT,
            CANCEL => CANCEL_TEXT,
            _ => return,
        };
        let (l, t, r, _) = RECTS[usize::from(part)];
        let shift = i32::from(pressed);
        let text = self.text(n);
        self.font.draw(
            &mut self.image,
            &text,
            (l + 2 + shift, t + 1 + shift),
            r - l,
            Align::Center,
            9,
        );
    }

    /// Draws part `part` held down: `closepre.bmp` for the close button,
    /// `BilButPr.bmp` for the others, with the text. // 1020:36ce
    fn draw_pressed(&mut self, part: u8) {
        let (l, t, r, b) = RECTS[usize::from(part)];
        let bitmap = if part == CLOSE { CLOSE_PRESSED } else { BUTTON_PRESSED };
        let clip = self.image.bounds();
        if let Some(src) = self.bitmaps.get(bitmap.0) {
            self.image
                .blit(l, t, src, Rect::new(0, 0, r - l + 1, b - t + 1), Rop::Copy, clip);
        }
        self.draw_text(part, true);
    }

    /// Draws part `part` up: the background's part, with the text.
    /// // 1020:367c, 1020:34ca
    fn draw_up(&mut self, part: u8) {
        let (l, t, r, b) = RECTS[usize::from(part)];
        let clip = self.image.bounds();
        if let Some(src) = self.bitmaps.get(BACKGROUND.0) {
            self.image
                .blit(l, t, src, Rect::new(l, t, r - l + 1, b - t + 1), Rop::Copy, clip);
        }
        self.draw_text(part, false);
    }

    /// Whether `x`, `y` is in part `n`'s rectangle, edges included.
    /// // 1020:347d, 1030:1517
    fn hit(part: u8, x: i32, y: i32) -> bool {
        let (left, top, right, bottom) = RECTS[usize::from(part)];
        (left..=right).contains(&x) && (top..=bottom).contains(&y)
    }

    /// `EndeImageMouseDown`, with `x`, `y` in the image and whether it's
    /// the left button. The close button, "Abbruch" or "OK" goes down
    /// (sound 8), or the title bar is grabbed: the form shrinks to the
    /// title bar until it's let go. // 1020:37dd
    pub fn mouse_down(&mut self, x: i32, y: i32, left_button: bool) -> Response {
        let mut r = Response::default();
        if !left_button {
            return r;
        }
        // Each button's hit rectangle, and the part it draws.
        for (hit, part) in [(CLOSE, CLOSE), (CANCEL + 1, CANCEL), (OK + 1, OK)] {
            if Self::hit(hit, x, y) {
                self.pressed = hit;
                self.draw_pressed(part);
                r.refresh = true;
                r.sounds.push(Sound::Press);
                return r;
            }
        }
        if Self::hit(TITLE, x, y) {
            self.pressed = TITLE;
            self.grab = (x, y);
            self.height = DRAG_HEIGHT;
            r.moved = true;
        }
        r
    }

    /// `EndeImageMouseMove`: drags the form by its title bar while the
    /// left button is held, keeping it on the `screen` by its current
    /// (dragging) height. // 1020:38d4
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        if self.pressed == TITLE && shift.contains(ShiftState::LEFT) {
            let (sx, sy) = (self.left + x, self.top + y);
            self.left = (sx - self.grab.0).max(0).min(screen.0 - WIDTH);
            // BUG(original): the form is kept on the screen by its height while dragged (16), so it can be dropped with its bottom off the screen.
            let height = if self.enhance.has(Enhancement::FormDrags) {
                HEIGHT
            } else {
                self.height
            };
            self.top = (sy - self.grab.1).max(0).min(screen.1 - height);
            r.moved = true;
        }
        r
    }

    /// `EndeImageMouseUp`: with the left button, the button that went
    /// down comes up (sound 10) and does its part if the mouse is still
    /// on it; the title bar lets the form have its height back.
    /// // 1020:395f
    pub fn mouse_up(&mut self, x: i32, y: i32, left_button: bool) -> Response {
        let mut r = Response::default();
        if !left_button {
            return r;
        }
        let pressed = std::mem::take(&mut self.pressed);
        match pressed {
            CLOSE | 5 | 7 => {
                let part = if pressed == CLOSE { CLOSE } else { pressed - 1 };
                self.draw_up(part);
                r.refresh = true;
                r.sounds.push(Sound::Release);
                if Self::hit(pressed, x, y) {
                    r.action = Some(if pressed == 7 { self.quit() } else { self.cancel() });
                }
            }
            TITLE => {
                self.height = HEIGHT;
                r.moved = true;
            }
            _ => {}
        }
        r
    }

    /// `FormKeyDown`, with any shift state: Escape and Space close the
    /// form (unless the game is quitting), Enter quits. // 1020:3d30
    pub fn key_down(&mut self, key: u16) -> Response {
        let mut r = Response::default();
        match key {
            VK_SPACE | VK_ESCAPE if !self.quitting => r.action = Some(self.cancel()),
            VK_RETURN => r.action = Some(self.quit()),
            _ => {}
        }
        r
    }

    fn cancel(&mut self) -> Action {
        self.hide();
        Action::Close
    }

    fn quit(&mut self) -> Action {
        self.hide();
        self.quitting = true;
        Action::Quit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::bmp::Bitmap;

    fn font() -> Font {
        let (width, height) = (12, 14 * 12);
        let bitmap = Bitmap {
            width,
            height,
            palette: [vcl::Rgb::default(); 256],
            pixels: vec![0; width * height],
        };
        Font::new(&bitmap, &[[0; 8]; 32], &[[1; 8]; 32])
    }

    fn form() -> Endeform {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(200, 100, u8::try_from(i + 1).unwrap()))
            .collect();
        let texts = (0..24).map(|i| format!("t{i}").into_bytes()).collect();
        let mut f = Endeform::new(font(), bitmaps, texts);
        assert!(f.open((800, 600)));
        f
    }

    fn click(f: &mut Endeform, x: i32, y: i32) -> Response {
        let _ = f.mouse_down(x, y, true);
        f.mouse_up(x, y, true)
    }

    #[test]
    fn opens_centred() {
        let f = form();
        assert_eq!((f.left, f.top, WIDTH, HEIGHT), (308, 258, 185, 85));
        assert_eq!(f.image.pixel(50, 40), 1, "the background");
    }

    #[test]
    fn buttons() {
        let mut f = form();
        let down = f.mouse_down(20, 65, true);
        assert_eq!(down.sounds, [Sound::Press]);
        assert_eq!(f.image.pixel(14, 74), 2, "drawn pressed");
        let up = f.mouse_up(20, 65, true);
        assert_eq!((up.action, up.sounds), (Some(Action::Close), vec![Sound::Release]));
        assert_eq!(f.image.pixel(14, 74), 1);
        assert!(!f.visible);
        assert!(f.open((800, 600)));
        assert_eq!(click(&mut f, 3, 3).action, Some(Action::Close));
        assert!(f.open((800, 600)));
        // The draw rectangle's last row isn't the hit's.
        assert_eq!(f.mouse_down(100, 76, true), Response::default());
        // Let go elsewhere: nothing but the sound.
        let _ = f.mouse_down(100, 65, true);
        assert_eq!(f.mouse_up(10, 30, true).action, None);
        assert!(f.visible);
        assert_eq!(click(&mut f, 100, 65).action, Some(Action::Quit));
        assert!(f.quitting() && !f.open((800, 600)));
        f.done_quitting();
        assert!(f.open((800, 600)));
    }

    #[test]
    fn keys() {
        let mut f = form();
        assert_eq!(f.key_down(VK_SPACE).action, Some(Action::Close));
        assert!(f.open((800, 600)));
        assert_eq!(f.key_down(0x41).action, None);
        assert_eq!(f.key_down(VK_RETURN).action, Some(Action::Quit));
        assert_eq!(f.key_down(VK_ESCAPE).action, None, "quitting");
        assert!(main_screen_opens(VK_ESCAPE, ShiftState::default()));
        assert!(main_screen_opens(0x51, ShiftState::SHIFT));
        assert!(main_screen_opens(VK_F4, ShiftState::ALT));
        assert!(!main_screen_opens(0x51, ShiftState::CTRL));
        assert!(!main_screen_opens(VK_F4, ShiftState::default()));
    }

    #[test]
    fn drags_by_the_title() {
        let mut f = form();
        let r = f.mouse_down(50, 5, true);
        assert!(r.moved);
        assert_eq!(f.bounds().height(), DRAG_HEIGHT);
        let _ = f.mouse_move(50, 5 + 500, ShiftState::LEFT, (800, 600));
        assert_eq!(f.top, 600 - DRAG_HEIGHT, "kept on the screen by the title bar");
        let _ = f.mouse_up(50, 5, true);
        assert_eq!((f.top, f.height), (584, HEIGHT));
        let _ = f.mouse_down(50, 5, true);
        let _ = f.mouse_move(-1000, 5, ShiftState::LEFT, (800, 600));
        assert_eq!(f.left, 0);
    }

    #[test]
    fn stays_on_the_screen_with_the_fix() {
        let mut f = form();
        f.enhance.set(Enhancement::FormDrags, true);
        let _ = f.mouse_down(50, 5, true);
        let _ = f.mouse_move(50, 5 + 500, ShiftState::LEFT, (800, 600));
        assert_eq!(f.top, 600 - HEIGHT);
    }
}
