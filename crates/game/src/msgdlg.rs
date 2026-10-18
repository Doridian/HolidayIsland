//! The game's message boxes: the original's `MessageDlg` calls (`Dialogs`,
//! en 10d8:07bf, de 10d0:07bf), shown in the game's own look.
//!
//! In the original, `MessageDlg(text, kind, buttons, 0)` opens a modal
//! Windows dialog (`MessageDlgPos`, en 10d8:07ee, of a form
//! `CreateMessageDialog`, en 10d8:00c5, builds): captioned by its kind
//! (`Warning`, `Error`, `Information`, `Confirm`, the string resources
//! 0xf06f–0xf072), the kind's system icon, the message word-wrapped to
//! half the screen's width, and a `TBitBtn` for each button with the
//! EXE's glyphs (`BBOK`, ...), captioned from the string resources
//! 0xf073–0xf07d. Those resources are in the build's language: German in
//! v1.0's `HOLIDAY.EXE` (`Bestätigen`, `&Abbruch`), English in the later
//! build's (`Confirm`, `&Cancel`).
//!
//! **The look is the port's choice, not a copy:** the box is drawn like
//! the game's own question before quitting, "HOLIDAY ISLAND VERLASSEN?"
//! ([`crate::endeform`]): its title bar from `Exitform.BMP` (the close
//! button at the left, the purple and teal pattern carried on to the
//! box's width) with the caption centred in yellow and the message
//! centred under it in the font's blue. The buttons are the file menu's
//! ([`crate::dateimenue`]), pixel for pixel: up as `datwin1c.BMP` has
//! them, held down `datB1pr.bmp`, the caption in black a pixel further
//! right and down when held, sounds 8 and 10. Behind
//! the text it has the high scores' plain grey face (`highscor.bmp`, as
//! the list of enhancements has) instead of the quit question's blue
//! picture, and it's as big as its text and buttons need. It's centred on
//! the screen when it opens and dragged by its title bar. What stays the
//! original's: the message, the kind (its caption), the buttons, their
//! order and what each answers ([`ModalResult`]), Enter for the default
//! button (the first, OK or Yes), Escape for the cancel button, the
//! mnemonic letters, and the close button or Alt+F4 answering `mrCancel`.
//! The texts are in the view's language ([`crate::lang`]).
//!
//! The box is view-side: it only answers, and the caller does what the
//! answer means, through a command if it changes the world. The
//! original's `ShowModal` blocks; the port's caller keeps the box open
//! and gives it all the input until it answers.

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::lang::{Language, Text};
use crate::text::{Align, Font, LINE_HEIGHT};
use crate::widgets::Sound;

/// `TMsgDlgType`, but `mtCustom`, which the game doesn't use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlgType {
    /// `mtWarning`.
    Warning,
    /// `mtError`.
    Error,
    /// `mtInformation`.
    Information,
    /// `mtConfirmation`.
    Confirmation,
}

impl DlgType {
    /// The caption: the string resources 0xf06f to 0xf072 (en
    /// `1118:4120`), in `language` (`msgdlg.*` in `strings.toml`).
    #[must_use]
    pub fn caption(self, language: Language) -> &'static [u8] {
        language.bytes(match self {
            DlgType::Warning => Text::MsgWarning,
            DlgType::Error => Text::MsgError,
            DlgType::Information => Text::MsgInformation,
            DlgType::Confirmation => Text::MsgConfirmation,
        })
    }
}

/// `TMsgDlgBtn`, in its order (the bits of `TMsgDlgButtons`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Yes,
    No,
    Ok,
    Cancel,
    Abort,
    Retry,
    Ignore,
    All,
    Help,
}

/// What a button answers (`TModalResult`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalResult {
    /// `mrOk` (1).
    Ok = 1,
    /// `mrCancel` (2), also the answer of closing the box.
    Cancel = 2,
    /// `mrAbort` (3).
    Abort = 3,
    /// `mrRetry` (4).
    Retry = 4,
    /// `mrIgnore` (5).
    Ignore = 5,
    /// `mrYes` (6).
    Yes = 6,
    /// `mrNo` (7).
    No = 7,
    /// `mrAll` (8).
    All = 8,
}

impl Btn {
    /// The caption, with `&` before the mnemonic: the string resources
    /// 0xf073 to 0xf07d (en `1118:4148`), in `language`.
    #[must_use]
    pub fn caption(self, language: Language) -> &'static [u8] {
        language.bytes(match self {
            Btn::Yes => Text::BtnYes,
            Btn::No => Text::BtnNo,
            Btn::Ok => Text::BtnOk,
            Btn::Cancel => Text::BtnCancel,
            Btn::Abort => Text::BtnAbort,
            Btn::Retry => Text::BtnRetry,
            Btn::Ignore => Text::BtnIgnore,
            Btn::All => Text::BtnAll,
            Btn::Help => Text::BtnHelp,
        })
    }

    /// What it answers: its `TBitBtn` kind's `ModalResult`. "Help" has
    /// none (it would open the help file, which the game has none of).
    #[must_use]
    pub fn result(self) -> Option<ModalResult> {
        Some(match self {
            Btn::Yes => ModalResult::Yes,
            Btn::No => ModalResult::No,
            Btn::Ok => ModalResult::Ok,
            Btn::Cancel => ModalResult::Cancel,
            Btn::Abort => ModalResult::Abort,
            Btn::Retry => ModalResult::Retry,
            Btn::Ignore => ModalResult::Ignore,
            Btn::All => ModalResult::All,
            Btn::Help => return None,
        })
    }

    const ALL: [Btn; 9] = [
        Btn::Yes,
        Btn::No,
        Btn::Ok,
        Btn::Cancel,
        Btn::Abort,
        Btn::Retry,
        Btn::Ignore,
        Btn::All,
        Btn::Help,
    ];
}

/// The bitmaps the box is drawn with, from `GRAFIK`, in this order: the
/// quit question's background (the title bar), the file menu's
/// background (a button up) and its button held down
/// ([`crate::dateimenue::BITMAP_FILES`]), `hintergrund`'s close button
/// held down, and the high scores' background (the face).
pub const BITMAP_FILES: [&str; 5] = [
    "Exitform.BMP",
    "datwin1c.BMP",
    "datB1pr.bmp",
    "closepre.bmp",
    "highscor.bmp",
];
const EXITFORM: usize = 0;
const FILE_MENU: usize = 1;
const BUTTON_PRESSED: usize = 2;
const CLOSE_PRESSED: usize = 3;
const FACE: usize = 4;

/// The quit question's title bar: its rows (with the line under it), the
/// close button at its left, the pattern's columns and its right end.
/// // 1110:18c2, 1110:18ca
pub const TITLE_HEIGHT: i32 = 17;
const CLOSE_SIZE: i32 = 16;
const PATTERN: (i32, i32) = (16, 165);
const TITLE_END: i32 = 185;
/// The file menu's "Abbruch": where it is in `datwin1c.BMP` (its part
/// `1110:189a`, 8, 186), and the size of its outline, which
/// `datB1pr.bmp` covers when it's held down.
const BUTTON_AT: (i32, i32) = (8, 186);
pub const BUTTON_SIZE: (i32, i32) = (77, 17);
/// The gap between the buttons, the text's top and the margins, as the
/// quit question has them. // 1110:18d2, 1110:18ea
const BUTTON_GAP: i32 = 10;
const TEXT_TOP: i32 = 24;
const TEXT_MARGIN: i32 = 8;
const BUTTON_MARGIN: i32 = 10;
/// The box's least width, the quit question's. // 1110:18ba
const MIN_WIDTH: i32 = 185;
/// The high scores' face: its edge columns and rows, and its rows clear
/// of widgets.
const EDGE: i32 = 4;
const CLEAR_ROWS: (i32, i32) = (20, 230);

/// The font's colour rows, the quit question's: the caption yellow, the
/// message blue, the buttons black. // 1020:3aad
const YELLOW: i32 = 1;
const BLUE: i32 = 7;
const BLACK: i32 = 9;

const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_F4: u16 = 0x73;

/// What an input did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// The box looks different (a button, its place).
    pub repaint: bool,
    /// The sound a button plays.
    pub sound: Option<Sound>,
    /// A button answered, or the box was closed.
    pub answer: Option<ModalResult>,
}

/// What holds the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    /// The close button.
    Close,
    /// Button `n`.
    Button(usize),
    /// The title bar, grabbed where from the box's top left.
    Title(i32, i32),
}

/// An open message box.
#[derive(Debug, Clone)]
pub struct MessageDlg {
    kind: DlgType,
    buttons: Vec<Btn>,
    language: Language,
    bitmaps: Vec<Surface>,
    font: Font,
    /// The face with the title and the message, the buttons up.
    background: Surface,
    image: Surface,
    /// The buttons' top left in the box.
    button_at: Vec<(i32, i32)>,
    /// The box's top left on the screen.
    pub left: i32,
    pub top: i32,
    held: Option<Held>,
}

impl MessageDlg {
    /// `MessageDlg(text, kind, buttons, 0)`, centred on a `screen` of
    /// that size, in the view's `language`, in the game's `font`;
    /// `bitmaps` are those of [`BITMAP_FILES`].
    #[must_use]
    pub fn new(
        text: &[u8],
        kind: DlgType,
        buttons: &[Btn],
        language: Language,
        font: &Font,
        bitmaps: Vec<Surface>,
        screen: (i32, i32),
    ) -> Self {
        // `TMsgDlgButtons` is a set: the buttons in its order.
        let buttons: Vec<Btn> = Btn::ALL.iter().copied().filter(|b| buttons.contains(b)).collect();
        let n = i32::try_from(buttons.len()).unwrap_or(0);
        let row = n * BUTTON_SIZE.0 + (n - 1).max(0) * BUTTON_GAP;
        // The original wraps at half the screen's width.
        let lines = wrap(text, (screen.0 / 2).max(1), font);
        let text_width = lines.iter().map(|l| font.width(l)).max().unwrap_or(0);
        let text_height = i32::try_from(lines.len()).unwrap_or(0) * LINE_HEIGHT;
        let width = MIN_WIDTH
            .max(text_width + 2 * TEXT_MARGIN + 4)
            .max(row + 2 * BUTTON_MARGIN);
        let buttons_top = TEXT_TOP + text_height + BUTTON_MARGIN + 1;
        let height = buttons_top + BUTTON_SIZE.1 + 8;
        let mut background = face(&bitmaps, width, height);
        title_bar(&mut background, &bitmaps);
        font.draw(
            &mut background,
            kind.caption(language),
            (CLOSE_SIZE, 1),
            width - CLOSE_SIZE,
            Align::Center,
            YELLOW,
        );
        for (y, line) in (TEXT_TOP..).step_by(LINE_HEIGHT as usize).zip(&lines) {
            font.draw(
                &mut background,
                line,
                (TEXT_MARGIN, y),
                width - 2 * TEXT_MARGIN,
                Align::Center,
                BLUE,
            );
        }
        let first = (width - row) / 2;
        let button_at: Vec<(i32, i32)> = (0..n)
            .map(|i| (first + i * (BUTTON_SIZE.0 + BUTTON_GAP), buttons_top))
            .collect();
        let mut dlg = MessageDlg {
            kind,
            buttons,
            language,
            bitmaps,
            font: font.clone(),
            image: background.clone(),
            background,
            button_at,
            left: 0,
            top: 0,
            held: None,
        };
        for i in 0..dlg.buttons.len() {
            dlg.draw_button(i, false);
        }
        dlg.background.clone_from(&dlg.image);
        dlg.place(screen);
        dlg
    }

    /// The kind it was opened with.
    #[must_use]
    pub fn kind(&self) -> DlgType {
        self.kind
    }

    /// The buttons, in their order.
    #[must_use]
    pub fn buttons(&self) -> &[Btn] {
        &self.buttons
    }

    /// Centres the box on a `screen` of that size (when it opens, and
    /// when the port's screen changes its size).
    pub fn place(&mut self, screen: (i32, i32)) {
        let b = self.bounds();
        self.left = (screen.0 - b.width()) / 2;
        self.top = (screen.1 - b.height()) / 2;
    }

    /// The box on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        let w = i32::try_from(self.image.width).unwrap_or(0);
        let h = i32::try_from(self.image.height).unwrap_or(0);
        Rect::new(self.left, self.top, w, h)
    }

    /// Button `i` in the box.
    fn button_rect(&self, i: usize) -> Rect {
        let (l, t) = self.button_at.get(i).copied().unwrap_or((0, 0));
        Rect::new(l, t, BUTTON_SIZE.0, BUTTON_SIZE.1)
    }

    /// Button `i` on the screen.
    #[must_use]
    pub fn button_bounds(&self, i: usize) -> Rect {
        self.button_rect(i).offset(self.left, self.top)
    }

    /// The close button on the screen.
    #[must_use]
    pub fn close_bounds(&self) -> Rect {
        Rect::new(self.left, self.top, CLOSE_SIZE, CLOSE_SIZE)
    }

    /// The box's picture.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// Button `i` up (the file menu's, from its background) or held down
    /// (`datB1pr.bmp`), the caption as the file menu draws it, a pixel
    /// further right and down when held. // 1040:0a4c (the file menu's
    /// buttons)
    fn draw_button(&mut self, i: usize, pressed: bool) {
        let r = self.button_rect(i);
        let clip = self.image.bounds();
        let (src, from) = if pressed {
            (BUTTON_PRESSED, (0, 0))
        } else {
            (FILE_MENU, BUTTON_AT)
        };
        if let Some(b) = self.bitmaps.get(src) {
            let from = Rect::new(from.0, from.1, BUTTON_SIZE.0, BUTTON_SIZE.1);
            self.image.blit(r.left, r.top, b, from, Rop::Copy, clip);
        }
        let (label, _) = strip_mnemonic(self.buttons[i].caption(self.language));
        let shift = i32::from(pressed);
        self.font.draw(
            &mut self.image,
            &label,
            (r.left + 2 + shift, r.top + 1 + shift),
            BUTTON_SIZE.0,
            Align::Center,
            BLACK,
        );
    }

    /// The close button held down (`closepre.bmp`) or up.
    fn draw_close(&mut self, pressed: bool) {
        let clip = self.image.bounds();
        let all = Rect::new(0, 0, CLOSE_SIZE, CLOSE_SIZE);
        let src = if pressed {
            self.bitmaps.get(CLOSE_PRESSED)
        } else {
            Some(&self.background)
        };
        if let Some(b) = src {
            self.image.blit(0, 0, b, all, Rop::Copy, clip);
        }
    }

    /// The left mouse button goes down at `x`, `y` on the screen: a
    /// button or the close button goes down (sound 8), the title bar
    /// right of the close button grabs the box. Elsewhere, outside too,
    /// nothing: the box is modal.
    pub fn mouse_down(&mut self, x: i32, y: i32) -> Response {
        let (x, y) = (x - self.left, y - self.top);
        let mut r = Response::default();
        let width = self.bounds().width();
        if inside(Rect::new(0, 0, CLOSE_SIZE, CLOSE_SIZE), x, y) {
            self.held = Some(Held::Close);
            self.draw_close(true);
        } else if let Some(i) = (0..self.buttons.len()).find(|&i| inside(self.button_rect(i), x, y)) {
            self.held = Some(Held::Button(i));
            self.draw_button(i, true);
        } else {
            if (0..TITLE_HEIGHT).contains(&y) && (CLOSE_SIZE..width).contains(&x) {
                self.held = Some(Held::Title(x, y));
            }
            return r;
        }
        r.repaint = true;
        r.sound = Some(Sound::Press);
        r
    }

    /// The mouse moves to `x`, `y` on a `screen` of that size: with the
    /// title bar grabbed the box follows, kept on the screen.
    pub fn mouse_move(&mut self, x: i32, y: i32, screen: (i32, i32)) -> Response {
        let mut r = Response::default();
        if let Some(Held::Title(gx, gy)) = self.held {
            let b = self.bounds();
            self.left = (x - gx).clamp(0, (screen.0 - b.width()).max(0));
            self.top = (y - gy).clamp(0, (screen.1 - b.height()).max(0));
            r.repaint = true;
        }
        r
    }

    /// The left mouse button comes up at `x`, `y`: a held button comes up
    /// (sound 10) and, with the mouse still on it, answers; the close
    /// button answers `mrCancel`.
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        let (x, y) = (x - self.left, y - self.top);
        let mut r = Response::default();
        match self.held.take() {
            Some(Held::Close) => {
                self.draw_close(false);
                if inside(Rect::new(0, 0, CLOSE_SIZE, CLOSE_SIZE), x, y) {
                    r.answer = Some(ModalResult::Cancel);
                }
            }
            Some(Held::Button(i)) => {
                self.draw_button(i, false);
                if inside(self.button_rect(i), x, y) {
                    r.answer = self.buttons[i].result();
                }
            }
            Some(Held::Title(..)) | None => return r,
        }
        r.repaint = true;
        r.sound = Some(Sound::Release);
        r
    }

    /// Whether the box has the mouse.
    #[must_use]
    pub fn captured(&self) -> bool {
        self.held.is_some()
    }

    /// A key, with Alt held or not: Enter and Space answer as the default
    /// button (the first: OK or Yes), Escape as the cancel button,
    /// Alt+F4 closes the box (`mrCancel`), and a button's mnemonic letter
    /// clicks it.
    #[must_use]
    pub fn key_down(&self, key: u16, alt: bool) -> Option<ModalResult> {
        match key {
            VK_RETURN | VK_SPACE => self.buttons.first().copied().and_then(Btn::result),
            VK_ESCAPE => self.cancel_button().and_then(Btn::result),
            VK_F4 if alt => Some(ModalResult::Cancel),
            k @ 0x30..=0x5a => {
                let letter = u8::try_from(k).unwrap_or(0);
                self.buttons
                    .iter()
                    .find(|b| mnemonic(b.caption(self.language)).is_some_and(|m| m.eq_ignore_ascii_case(&letter)))
                    .copied()
                    .and_then(Btn::result)
            }
            _ => None,
        }
    }

    /// The button Escape clicks: Delphi 1's `TBitBtn` kinds Cancel and No
    /// are cancel buttons, and `CreateMessageDialog` makes OK one when
    /// there's no Cancel. // en 10d8:00c5
    fn cancel_button(&self) -> Option<Btn> {
        [Btn::Cancel, Btn::No, Btn::Ok]
            .into_iter()
            .find(|b| self.buttons.contains(b))
    }
}

fn inside(r: Rect, x: i32, y: i32) -> bool {
    (r.left..r.right).contains(&x) && (r.top..r.bottom).contains(&y)
}

/// The face of a box `width` × `height`, cut from the high scores'
/// background as the list of enhancements is: below its title bar, the
/// face's clean rows mirrored on down to the bottom edge, its right edge
/// moved to the box's.
fn face(bitmaps: &[Surface], width: i32, height: i32) -> Surface {
    let mut bg = Surface::new(
        usize::try_from(width).unwrap_or(0),
        usize::try_from(height).unwrap_or(0),
        0,
    );
    let clip = bg.bounds();
    let Some(src) = bitmaps.get(FACE) else {
        return bg;
    };
    let (src_w, src_h) = (
        i32::try_from(src.width).unwrap_or(0),
        i32::try_from(src.height).unwrap_or(0),
    );
    let (clear_top, clear_bottom) = (CLEAR_ROWS.0.min(src_h), CLEAR_ROWS.1.min(src_h));
    let span = (clear_bottom - clear_top).max(1);
    let row = |y: i32| {
        if y >= height - EDGE {
            src_h - (height - y)
        } else {
            let k = y % (2 * span);
            if k < span {
                clear_top + k
            } else {
                clear_bottom - 1 - (k - span)
            }
        }
    };
    let body = (width - EDGE).min(src_w - EDGE);
    for y in 0..height {
        let from = row(y);
        bg.blit(0, y, src, Rect::new(0, from, body, 1), Rop::Copy, clip);
        bg.blit(
            width - EDGE,
            y,
            src,
            Rect::new(src_w - EDGE, from, EDGE, 1),
            Rop::Copy,
            clip,
        );
    }
    bg
}

/// The quit question's title bar over the top of `bg`: its close button,
/// its pattern repeated across, and its right end.
fn title_bar(bg: &mut Surface, bitmaps: &[Surface]) {
    let Some(src) = bitmaps.get(EXITFORM) else { return };
    let clip = bg.bounds();
    let width = clip.width();
    let end = 4;
    let mut x = 0;
    while x < width - end {
        let (from, w) = if x < PATTERN.0 {
            (0, PATTERN.0)
        } else {
            (PATTERN.0, PATTERN.1.min(width - end - x))
        };
        bg.blit(x, 0, src, Rect::new(from, 0, w, TITLE_HEIGHT), Rop::Copy, clip);
        x += w;
    }
    bg.blit(
        width - end,
        0,
        src,
        Rect::new(TITLE_END - end, 0, end, TITLE_HEIGHT),
        Rop::Copy,
        clip,
    );
}

/// The caption without its `&`, and where the letter after it (the
/// mnemonic) is.
fn strip_mnemonic(caption: &[u8]) -> (Vec<u8>, Option<usize>) {
    let mut out = Vec::with_capacity(caption.len());
    let mut underline = None;
    let mut amp = false;
    for &ch in caption {
        if ch == b'&' && !amp {
            amp = true;
            continue;
        }
        if amp && underline.is_none() {
            underline = Some(out.len());
        }
        amp = false;
        out.push(ch);
    }
    (out, underline)
}

/// The mnemonic letter of a caption.
fn mnemonic(caption: &[u8]) -> Option<u8> {
    let (label, at) = strip_mnemonic(caption);
    at.and_then(|k| label.get(k).copied())
}

/// `DrawText` with `DT_WORDBREAK` into `width` pixels: lines broken at
/// spaces, a word too long for a line on its own line.
fn wrap(text: &[u8], width: i32, font: &Font) -> Vec<Vec<u8>> {
    let mut lines = Vec::new();
    for paragraph in text.split(|&c| c == b'\r' || c == b'\n') {
        let mut line: Vec<u8> = Vec::new();
        for word in paragraph.split(|&c| c == b' ').filter(|w| !w.is_empty()) {
            let mut next = line.clone();
            if !next.is_empty() {
                next.push(b' ');
            }
            next.extend_from_slice(word);
            if line.is_empty() || font.width(&next) <= width {
                line = next;
            } else {
                lines.push(std::mem::replace(&mut line, word.to_vec()));
            }
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonics() {
        assert_eq!(strip_mnemonic(b"&Cancel"), (b"Cancel".to_vec(), Some(0)));
        assert_eq!(strip_mnemonic(b"Abort"), (b"Abort".to_vec(), None));
        assert_eq!(mnemonic(Btn::Cancel.caption(Language::De)), Some(b'A'));
        assert_eq!(mnemonic(Btn::Ok.caption(Language::En)), Some(b'O'));
    }

    #[test]
    fn captions_follow_the_language() {
        assert_eq!(DlgType::Confirmation.caption(Language::En), b"Confirm");
        assert_eq!(DlgType::Confirmation.caption(Language::De), b"Best\xe4tigen");
        assert_eq!(Btn::Cancel.caption(Language::En), b"&Cancel");
        assert_eq!(Btn::Cancel.caption(Language::De), b"&Abbruch");
    }

    #[test]
    fn the_box_is_cut_to_size() {
        let mut exit = Surface::new(200, 85, 5);
        exit.fill_rect(Rect::new(0, 0, 16, 16), exit.bounds(), 6);
        let mut face_bmp = Surface::new(640, 300, 1);
        face_bmp.fill_rect(Rect::new(636, 0, 4, 300), face_bmp.bounds(), 2);
        face_bmp.fill_rect(Rect::new(0, 296, 640, 4), face_bmp.bounds(), 3);
        let none = || Surface::new(0, 0, 0);
        let bitmaps = vec![exit, none(), none(), none(), face_bmp];
        let mut bg = face(&bitmaps, 300, 120);
        title_bar(&mut bg, &bitmaps);
        assert_eq!((bg.width, bg.height), (300, 120));
        assert_eq!(bg.pixel(3, 3), 6, "the close button");
        assert_eq!(bg.pixel(250, 10), 5, "the title bar across");
        assert_eq!(bg.pixel(10, 40), 1, "the face");
        assert_eq!(bg.pixel(298, 40), 2, "the right edge");
        assert_eq!(bg.pixel(10, 118), 3, "the bottom edge");
    }
}
