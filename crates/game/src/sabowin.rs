//! `TSabowin`, the sabotage window (segment `1028`): the button bar's
//! button 21 shows it and picks the sabotage tool (1010:2c8b). It lists
//! `SABOTAGE.TXT`'s nine kinds of sabotage; the one picked (`1110:65ba`)
//! is shown with its picture, its description, what it can be used on and
//! its price. The tool's click on the island then orders sabotage of that
//! kind ([`crate::command::Action::Sabotage`], [`crate::sabotage`]).
//!
//! The form isn't modal and doesn't stop the clock: the tool works while
//! it's shown, and "Sabotieren" (or Enter) only hides it, keeping the
//! tool. "Schliessen", the close button, Escape, Space and Alt+F4 hide it
//! and take the pointer. The form is one `TImage`, `Saboimage`, with the
//! background `samenu.bmp`; its parts are the rectangles of a table
//! (`1110:16b8`).
//!
//! This is the view: [`crate::tools::Tools`] keeps the form and passes it
//! the mouse and the keys (`Tools::sabowin_mouse_down` and so on), since
//! its buttons change the tool. `docs/sabotage.md` has the whole of it.

// The original's names: rectangles' `l`, `t`, `r`, `b`.
#![allow(clippy::many_single_char_names)]

use formats::data;
use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::enhance::{Enhancement, Enhancements};
use crate::hauptschirm::InfoTexts;
use crate::money::format_money;
use crate::text::{Align, Font};
use crate::widgets::ShiftState;

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Sabowin::new`] takes them; the kinds' pictures come after them, by
/// [`picture_files`].
pub const BITMAP_FILES: [&str; 3] = [
    "samenu.bmp",   // the background, 1110:65c4, 1028:348c
    "sabupr.BMP",   // a button held down, 1110:65cc, 1028:3497
    "closepre.bmp", // the close button held down, `hintergrund`'s, 1110:54bc
];
const BACKGROUND: usize = 0;
const BUTTON_PRESSED: usize = 1;
const CLOSE_PRESSED: usize = 2;

/// The `GRAFIK` files of the kinds' pictures (`SABOTAGE.TXT`'s second
/// line of each), which the original loads each time it draws one
/// (1028:3079).
#[must_use]
pub fn picture_files(kinds: &[data::Sabotage]) -> Vec<String> {
    kinds
        .iter()
        .map(|k| String::from_utf8_lossy(&k.bitmap).trim().to_string())
        .collect()
}

/// The form's parts: left, top, right and bottom. The hit tests include
/// the right and bottom edges (1030:1517), the blits and texts don't.
/// 0 is the form, 1 the title bar, 2 the close button, 3 the picture, 4
/// "Schliessen", 6 "Sabotieren", 12 the list, 13 the kind's name, 14 its
/// description; 5, 7 and 8 to 11 aren't used. // 1110:16b8
pub const RECTS: [(i32, i32, i32, i32); 15] = [
    (0, 0, 411, 217),     // 1110:16b8
    (16, 0, 411, 16),     // 1110:16c0
    (0, 0, 15, 15),       // 1110:16c8
    (115, 46, 240, 171),  // 1110:16d0
    (244, 191, 320, 207), // 1110:16d8
    (244, 191, 320, 207), // 1110:16e0
    (325, 191, 401, 207), // 1110:16e8
    (325, 191, 401, 207), // 1110:16f0
    (46, 26, 74, 42),     // 1110:16f8
    (46, 26, 74, 42),     // 1110:1700
    (46, 190, 74, 206),   // 1110:1708
    (46, 190, 74, 206),   // 1110:1710
    (11, 46, 110, 185),   // 1110:1718
    (115, 172, 239, 186), // 1110:1720
    (245, 46, 399, 186),  // 1110:1728
];
const TITLE: u8 = 1;
const CLOSE: u8 = 2;
const PICTURE: u8 = 3;
const SCHLIESSEN: u8 = 4;
const SABOTIEREN: u8 = 6;
const LIST: u8 = 0xc;
const NAME: u8 = 0xd;
const DESCRIPTION: u8 = 0xe;

/// The kind on each row of the list, and the row of each kind: the
/// same order. // 1110:173a, 1110:1730
const ROW_KIND: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
const KIND_ROW: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
/// The list's rows, 14 pixels apart (a line of text). // 1028:2d67
const ROW_HEIGHT: i32 = 14;

/// The form's size as `FormShow` sets it (`RECTS[0]`; the DFM's 392 ×
/// 250 is never seen), and its height while it's dragged by its title
/// bar. // 1028:34a2, 1028:2d67
pub const WIDTH: i32 = 411;
pub const HEIGHT: i32 = 217;
pub const TITLE_HEIGHT: i32 = 16;
/// The DFM's position.
// BUG(original): the DFM's place on the screen, which isn't over the centred main screen on a desktop bigger than 640×480.
pub const LEFT: i32 = 300;
pub const TOP: i32 = 167;

/// `VK_*` codes the form reads.
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_UP: u16 = 0x26;
const VK_DOWN: u16 = 0x28;
const VK_F4: u16 = 0x73;

/// The sounds (`play_sound`, 10a0:30b6).
pub const SOUND_SHOW: u8 = 0xd;
pub const SOUND_HIDE: u8 = 0xe;
const SOUND_PRESS: u8 = 8;
const SOUND_SELECT: u8 = 9;
const SOUND_RELEASE: u8 = 10;

/// How the form goes away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Close {
    /// "Sabotieren" or Enter: hidden, the sabotage tool stays.
    Hide,
    /// "Schliessen", the close button, Escape, Space or Alt+F4: hidden,
    /// button 21 up, the pointer's down and the pointer is the tool.
    Pointer,
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// The image changed and the handler refreshes it.
    pub refresh: bool,
    /// The sounds played, in order.
    pub sounds: Vec<u8>,
    /// The form moved, or its height changed (dragging by the title bar
    /// shrinks it to the bar).
    pub moved: bool,
    /// "Sabotieren" (or Enter) makes 21 the tool (`1110:6056 := 0x15`,
    /// which it already is while the form is shown) and sets
    /// `1110:54fa`, which nothing reads.
    pub sabotage_tool: bool,
    pub close: Option<Close>,
}

/// What the form draws from besides its bitmaps.
#[derive(Debug, Clone)]
pub struct Texts {
    /// `SABOTEXT.TXT`'s three: the title, "Schliessen", "Sabotieren".
    /// // *1110:7356, *1110:735a, *1110:735e
    pub sabotext: [Vec<u8>; 3],
    /// `SABOTAGE.TXT`'s labels "Anwendbar auf:" and "Kosten:".
    /// // *1110:65d2
    pub labels: [Vec<u8>; 2],
    /// `SABOTAGE.TXT`'s kinds. // *1110:65bc
    pub kinds: Vec<data::Sabotage>,
    /// For the price.
    pub info: InfoTexts,
}

/// The sabotage window.
#[derive(Debug, Clone)]
pub struct Sabowin {
    font: Font,
    bitmaps: Vec<Surface>,
    /// The kinds' pictures, by kind ([`picture_files`]).
    pictures: Vec<Surface>,
    texts: Texts,
    empty: Surface,
    /// `Saboimage`'s picture.
    image: Surface,
    /// The form's top left on the screen, and its height.
    pub left: i32,
    pub top: i32,
    pub height: i32,
    /// The form's `Visible`.
    pub visible: bool,
    /// The kind picked, 0 when the game starts (1028:361f). // 1110:65ba
    pub kind: u8,
    /// The part the mouse went down on, 0 for none; the mouse's release
    /// leaves it. // 1110:65d0
    pressed: u8,
    /// Where the mouse grabbed the title bar, from the form's top left.
    /// // 1110:6046, 1110:6048
    grab: (i32, i32),
    /// The player's view enhancements (`form_drags`).
    pub enhance: Enhancements,
}

/// Whether `x`, `y` is in part `n`, edges included. // 1028:2ad7, 1030:1517
fn hit(n: u8, x: i32, y: i32) -> bool {
    let (left, top, right, bottom) = RECTS[usize::from(n)];
    (left..=right).contains(&x) && (top..=bottom).contains(&y)
}

impl Sabowin {
    /// The form as it was created, hidden, at the DFM's position, with
    /// the first kind picked. `bitmaps` are those of [`BITMAP_FILES`],
    /// `pictures` the kinds' ([`picture_files`]).
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, pictures: Vec<Surface>, texts: Texts) -> Self {
        Sabowin {
            font,
            bitmaps,
            pictures,
            texts,
            empty: Surface::new(0, 0, 0),
            image: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            left: LEFT,
            top: TOP,
            height: HEIGHT,
            visible: false,
            kind: 0,
            pressed: 0,
            grab: (0, 0),
            enhance: Enhancements::ORIGINAL,
        }
    }

    /// `Saboimage`'s picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// Whether `x`, `y` (in the form) is on the form as it's shown.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.visible && (0..WIDTH).contains(&x) && (0..self.height).contains(&y)
    }

    /// `TCustomForm.Show`: a hidden form gets `FormShow`, its full height,
    /// the background and everything on it (1028:3079), and sound 0xd.
    /// // 1028:34a2
    pub fn show(&mut self) -> Response {
        let mut r = Response::default();
        if self.visible {
            return r;
        }
        self.visible = true;
        self.height = HEIGHT; // 1028:34a2: ClientHeight
        let clip = self.image.bounds();
        let background = self.bitmaps.get(BACKGROUND).unwrap_or(&self.empty);
        let (l, t, rt, b) = RECTS[0];
        self.image
            .blit(l, t, background, Rect::new(0, 0, rt - l, b - t), Rop::Copy, clip);
        self.draw();
        r.refresh = true;
        r.moved = true;
        r.sounds.push(SOUND_SHOW);
        r
    }

    /// `TCustomForm.Hide`: a shown form gets `FormHide`, sound 0xe.
    /// // 1028:35ee
    pub fn hide(&mut self) -> Response {
        let mut r = Response::default();
        if self.visible {
            self.visible = false;
            r.sounds.push(SOUND_HIDE);
        }
        r
    }

    fn text(&self, n: usize) -> Vec<u8> {
        self.texts.sabotext.get(n).cloned().unwrap_or_default()
    }

    /// A button's text: up at two pixels right and one down
    /// (1028:2b9f), pressed one more each way (1028:2b24).
    fn button_text(&mut self, n: u8, pressed: bool) {
        let text = match n {
            SCHLIESSEN => self.text(1),
            SABOTIEREN => self.text(2),
            _ => return,
        };
        let (l, t, r, _) = RECTS[usize::from(n)];
        let shift = i32::from(pressed);
        self.font.draw(
            &mut self.image,
            &text,
            (l + 2 + shift, t + 1 + shift),
            r - l,
            Align::Center,
            9,
        );
    }

    /// Part `n` from `bitmap` at the part's place, or from (0, 0).
    fn blit_part(&mut self, n: u8, bitmap: usize, from_origin: bool) {
        let (l, t, r, b) = RECTS[usize::from(n)];
        let clip = self.image.bounds();
        let src = self.bitmaps.get(bitmap).unwrap_or(&self.empty);
        let (x, y) = if from_origin { (0, 0) } else { (l, t) };
        self.image
            .blit(l, t, src, Rect::new(x, y, r - l, b - t), Rop::Copy, clip);
    }

    /// Part `n` held down: `closepre.bmp` for the close button,
    /// `sabupr.BMP` and the text for the buttons. // 1028:2c62
    fn draw_pressed(&mut self, n: u8) {
        if n == CLOSE {
            self.blit_part(n, CLOSE_PRESSED, true);
        } else {
            self.blit_part(n, BUTTON_PRESSED, true);
        }
        self.button_text(n, true);
    }

    /// Part `n` up: the background's part (1028:33d0) and the text.
    /// // 1028:2c18
    fn draw_up(&mut self, n: u8) {
        self.blit_part(n, BACKGROUND, false);
        self.button_text(n, false);
    }

    /// The buttons' texts, the title, the list with the picked kind in
    /// colour 11, and the kind: its name, description, what it can be
    /// used on, its price and its picture. The list's and the buttons'
    /// texts go over what's there; the name and description parts are
    /// cleared first. // 1028:3079
    fn draw(&mut self) {
        self.button_text(SCHLIESSEN, false);
        self.button_text(SABOTIEREN, false);
        let (l, t, r, _) = RECTS[usize::from(TITLE)];
        let title = self.text(0);
        self.font.draw(&mut self.image, &title, (l, t), r - l, Align::Center, 1);
        let (l, t, r, _) = RECTS[usize::from(LIST)];
        for (row, kind) in (0..).zip(0..=8u8) {
            let name = self
                .texts
                .kinds
                .get(usize::from(kind))
                .map(|k| k.name.clone())
                .unwrap_or_default();
            let colour = if kind == self.kind { 0xb } else { 9 };
            self.font.draw(
                &mut self.image,
                &name,
                (l, t + row * ROW_HEIGHT),
                r - l,
                Align::Left,
                colour,
            );
        }
        // BUG(original): a kind past the nine reads past the table; the port draws nothing for it.
        let kind = self.texts.kinds.get(usize::from(self.kind)).cloned();
        let kind = kind.unwrap_or(data::Sabotage {
            name: Vec::new(),
            bitmap: Vec::new(),
            description: Vec::new(),
            targets_text: Vec::new(),
            cost: 0,
            targets: [0xff; 6],
        });
        self.blit_part(NAME, BACKGROUND, false); // 1028:33d0
        let (l, t, r, _) = RECTS[usize::from(NAME)];
        self.font
            .draw(&mut self.image, &kind.name, (l, t), r - l, Align::Center, 9);
        self.blit_part(DESCRIPTION, BACKGROUND, false);
        let (l, t, r, b) = RECTS[usize::from(DESCRIPTION)];
        let w = r - l;
        self.font
            .draw(&mut self.image, &kind.description, (l, t), w, Align::Center, 9);
        let labels = self.texts.labels.clone();
        self.font
            .draw(&mut self.image, &labels[0], (l, b - 0x38), w, Align::Center, 9);
        self.font
            .draw(&mut self.image, &kind.targets_text, (l, b - 0x2a), w, Align::Center, 9);
        self.font
            .draw(&mut self.image, &labels[1], (l, b - 0x1c), w, Align::Center, 9);
        let info = &self.texts.info;
        let mut price = format_money(kind.cost, info.currency_factor, info.land); // 1030:11a0
        price.push(b' '); // 1028:3077
        price.extend_from_slice(&info.currency); // 1110:7352
        self.font
            .draw(&mut self.image, &price, (l, b - 0xe), w, Align::Center, 9);
        let (l, t, r, b) = RECTS[usize::from(PICTURE)];
        let clip = self.image.bounds();
        if let Some(picture) = self.pictures.get(usize::from(self.kind)) {
            self.image
                .blit(l, t, picture, Rect::new(0, 0, r - l, b - t), Rop::Copy, clip);
        }
    }

    /// `SaboimageMouseDown`, with `x`, `y` in the image and whether it's
    /// the left button. Any button lets go of the part that was pressed;
    /// the left one presses the close button, "Schliessen" or
    /// "Sabotieren" (sound 8), grabs the title bar (the form shrinks to
    /// it), or picks the kind on the list's row (sound 9 anywhere on the
    /// list). // 1028:2d67
    pub fn mouse_down(&mut self, x: i32, y: i32, left_button: bool) -> Response {
        let mut r = Response::default();
        self.pressed = 0;
        if !left_button {
            return r;
        }
        if let Some(n) = [CLOSE, SCHLIESSEN, SABOTIEREN].into_iter().find(|&n| hit(n, x, y)) {
            self.pressed = n;
            self.draw_pressed(n);
            r.refresh = true;
            r.sounds.push(SOUND_PRESS);
        } else if hit(TITLE, x, y) {
            self.pressed = TITLE;
            self.grab = (x, y);
            self.height = TITLE_HEIGHT; // ClientHeight := 16
            r.moved = true;
        } else if hit(LIST, x, y) {
            self.pressed = LIST;
            let row = (y - RECTS[usize::from(LIST)].1) / ROW_HEIGHT;
            if let Some(&kind) = usize::try_from(row).ok().and_then(|row| ROW_KIND.get(row)) {
                self.kind = kind;
                self.draw();
                r.refresh = true;
            }
            r.sounds.push(SOUND_SELECT);
        }
        r
    }

    /// `SaboimageMouseMove`: with the title bar grabbed and the left
    /// button held, the form follows the mouse, kept on the `screen`.
    /// // 1028:2ecc
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        if self.pressed == TITLE && shift.contains(ShiftState::LEFT) {
            let (sx, sy) = (self.left + x, self.top + y); // GetCursorPos
            self.left = (sx - self.grab.0).max(0).min(screen.0 - WIDTH);
            self.top = (sy - self.grab.1).max(0).min(screen.1 - self.height);
            r.moved = true;
        }
        r
    }

    /// `SaboimageMouseUp`, left button only: a pressed button comes up
    /// (sound 10) and, if the mouse is still on it, hides the form, the
    /// pointer taking over for the close button and "Schliessen".
    /// "Sabotieren" makes 21 the tool even off the button. The title bar
    /// gives the form its height back. // 1028:2f4e
    pub fn mouse_up(&mut self, x: i32, y: i32, left_button: bool) -> Response {
        let mut r = Response::default();
        if !left_button {
            return r;
        }
        // BUG(original): the press stays in 1110:65d0 until the next one, so a later release (or a move with the left button held) still drags.
        match self.pressed {
            n @ (CLOSE | SCHLIESSEN | SABOTIEREN) => {
                self.draw_up(n);
                r.refresh = true;
                r.sounds.push(SOUND_RELEASE);
                if n == SABOTIEREN {
                    // BUG(original): "Sabotieren" only hides the form: the tool already sabotages with the picked kind, also while the form is open; 1110:54fa, which it sets, is never read.
                    r.sabotage_tool = true;
                }
                if hit(n, x, y) {
                    r.close = Some(if n == SABOTIEREN { Close::Hide } else { Close::Pointer });
                }
            }
            TITLE => {
                self.height = HEIGHT;
                r.moved = true;
            }
            _ => {}
        }
        if self.enhance.has(Enhancement::FormDrags) {
            self.pressed = 0;
        }
        r
    }

    /// `FormKeyDown`. Without Ctrl and Alt: Escape and Space close the
    /// form for the pointer, the arrows pick the kind above or below,
    /// Enter is "Sabotieren". Alt+F4 closes it for the pointer, and the
    /// original sets the key to 0xff, so it doesn't close the game.
    /// // 1028:394c
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Response {
        let mut r = Response::default();
        let mods = shift.0 & (ShiftState::SHIFT.0 | ShiftState::ALT.0 | ShiftState::CTRL.0);
        if mods == 0 || mods == ShiftState::SHIFT.0 {
            let row = KIND_ROW.get(usize::from(self.kind)).copied().unwrap_or(0xff);
            match key {
                VK_ESCAPE | VK_SPACE => r.close = Some(Close::Pointer),
                VK_UP if row != 0 => {
                    self.kind -= 1;
                    self.draw();
                    r.refresh = true;
                }
                VK_DOWN if row < 8 => {
                    self.kind += 1;
                    self.draw();
                    r.refresh = true;
                }
                VK_RETURN => {
                    self.draw_up(SABOTIEREN);
                    r.refresh = true;
                    r.sounds.push(SOUND_RELEASE);
                    r.sabotage_tool = true;
                    r.close = Some(Close::Hide);
                }
                _ => {}
            }
        } else if mods == ShiftState::ALT.0 && key == VK_F4 {
            r.close = Some(Close::Pointer);
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::bmp::Bitmap;

    fn font() -> Font {
        let (width, height) = (12, 14 * 12);
        let mut pixels = vec![0; width * height];
        pixels[..width * 14].fill(0xff);
        let bitmap = Bitmap {
            width,
            height,
            palette: [vcl::Rgb::default(); 256],
            pixels,
        };
        Font::new(&bitmap, &[[0; 8]; 32], &[[1; 8]; 32])
    }

    fn form() -> Sabowin {
        let bitmaps = (1..=3).map(|i| Surface::new(420, 220, i)).collect();
        let pictures = (0..9).map(|i| Surface::new(125, 125, 20 + i)).collect();
        let kinds = (0..9)
            .map(|i| data::Sabotage {
                name: format!("k{i}").into_bytes(),
                bitmap: b"x.bmp".to_vec(),
                description: b"d".to_vec(),
                targets_text: b"t".to_vec(),
                cost: 1000 * i,
                targets: [0xff; 6],
            })
            .collect();
        let texts = Texts {
            sabotext: [b"T".to_vec(), b"S".to_vec(), b"O".to_vec()],
            labels: [b"A".to_vec(), b"K".to_vec()],
            kinds,
            info: InfoTexts {
                months: Vec::new(),
                land: formats::data::Land::Deutschland,
                currency_factor: 1,
                currency: b"DM".to_vec(),
            },
        };
        Sabowin::new(font(), bitmaps, pictures, texts)
    }

    #[test]
    fn shows_the_picked_kind() {
        let mut f = form();
        let r = f.show();
        assert_eq!(r.sounds, [SOUND_SHOW]);
        assert_eq!(f.image.pixel(200, 100), 20, "kind 0's picture");
        assert_eq!(f.image.pixel(5, 200), 1, "the background");
        assert_eq!(f.show().sounds, Vec::<u8>::new(), "already shown");
        // The fourth row.
        let r = f.mouse_down(20, 46 + 3 * 14 + 2, true);
        assert_eq!(r.sounds, [SOUND_SELECT]);
        assert_eq!(f.kind, 3);
        assert_eq!(f.image.pixel(200, 100), 23);
        // Below the last row: only the sound.
        let r = f.mouse_down(20, 184, true);
        assert_eq!((r.refresh, f.kind), (false, 3));
        let _ = f.key_down(VK_DOWN, ShiftState::default());
        assert_eq!(f.kind, 4);
        let _ = f.key_down(VK_UP, ShiftState::SHIFT);
        let _ = f.key_down(VK_UP, ShiftState::default());
        assert_eq!(f.kind, 2);
        f.kind = 8;
        let _ = f.key_down(VK_DOWN, ShiftState::default());
        assert_eq!(f.kind, 8);
        f.kind = 0;
        let _ = f.key_down(VK_UP, ShiftState::default());
        assert_eq!(f.kind, 0);
        assert_eq!(f.hide().sounds, [SOUND_HIDE]);
        assert_eq!(f.hide().sounds, Vec::<u8>::new());
    }

    #[test]
    fn buttons() {
        let mut f = form();
        let _ = f.show();
        // "Sabotieren": pressed, then released on it.
        let r = f.mouse_down(330, 195, true);
        assert_eq!(r.sounds, [SOUND_PRESS]);
        assert_eq!(f.image.pixel(330, 205), 2, "sabupr.BMP");
        let r = f.mouse_up(330, 195, true);
        assert_eq!(
            (r.sounds, r.sabotage_tool, r.close),
            (vec![SOUND_RELEASE], true, Some(Close::Hide))
        );
        assert_eq!(f.image.pixel(330, 205), 1);
        // Released off it: the tool, but the form stays.
        let _ = f.mouse_down(330, 195, true);
        let r = f.mouse_up(10, 100, true);
        assert_eq!((r.sabotage_tool, r.close), (true, None));
        for (x, y) in [(250, 200), (3, 3)] {
            let _ = f.mouse_down(x, y, true);
            let r = f.mouse_up(x, y, true);
            assert_eq!((r.sabotage_tool, r.close), (false, Some(Close::Pointer)));
        }
        // A right press lets go.
        let _ = f.mouse_down(3, 3, true);
        let _ = f.mouse_down(3, 3, false);
        assert_eq!(f.mouse_up(3, 3, true).close, None);
        assert_eq!(f.key_down(VK_ESCAPE, ShiftState::default()).close, Some(Close::Pointer));
        assert_eq!(f.key_down(VK_RETURN, ShiftState::default()).close, Some(Close::Hide));
        assert_eq!(f.key_down(VK_F4, ShiftState::ALT).close, Some(Close::Pointer));
        assert_eq!(f.key_down(VK_ESCAPE, ShiftState::CTRL).close, None);
    }

    #[test]
    fn dragged_by_the_title_bar() {
        let mut f = form();
        let _ = f.show();
        let _ = f.mouse_down(100, 5, true);
        assert_eq!(f.height, TITLE_HEIGHT);
        let r = f.mouse_move(90, 0, ShiftState::LEFT, (800, 600));
        assert!(r.moved);
        assert_eq!((f.left, f.top), (290, 162));
        let _ = f.mouse_move(5000, 5000, ShiftState::LEFT, (800, 600));
        assert_eq!((f.left, f.top), (800 - WIDTH, 600 - TITLE_HEIGHT));
        let _ = f.mouse_up(0, 0, true);
        assert_eq!(f.height, HEIGHT);
    }

    #[test]
    fn the_drag_ends_with_the_fix() {
        for fix in [false, true] {
            let mut f = form();
            f.enhance.set(Enhancement::FormDrags, fix);
            let _ = f.show();
            let _ = f.mouse_down(100, 5, true);
            let _ = f.mouse_up(100, 5, true);
            // A later left drag over the form.
            assert_eq!(f.mouse_move(90, 0, ShiftState::LEFT, (800, 600)).moved, !fix);
        }
    }
}
