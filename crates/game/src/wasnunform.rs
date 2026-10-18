//! `TWasNunForm`, the progress window while a game starts (en segment
//! `1028`, de `1040`; VMT en 1028:3684). State 1200 of the background
//! form's timer creates and shows it, states 1210, 1220 and 1500 move it
//! on, and 1500 hides and frees it before the main screen shows
//! (`hintergrund_timer`, en 1010:0591, `docs/startup.md`).
//!
//! It's 200 × 100, centred on the screen, borderless: `WasNun.bmp` with a
//! gauge (`gauge_init`, en 10c8:0181: `Prozent.bmp` filled from the left
//! with `Prozentp.bmp`, at 50, 60, 100 × 20) and a line of
//! `WASNUN.TXT` centred over it, at y 21 in the font's colour 9. Each
//! step sets a page of the text with the gauge empty
//! (`wasnun_set_page`, en 1028:39e3) or the gauge to a percentage with
//! the text as it is (en 1028:3a09, which also lets the messages through
//! with the timer stopped, `Application.ProcessMessages`); both redraw
//! the whole picture (en 1028:3a73).
//!
//! | Step | Page | Gauge |
//! |---|---|---|
//! | State 1200: shown (en 1028:3b3c) | none | 0 |
//! | 1210, a new island (`generate_island`, en 10b8:52d4) | 1 | 5, 26, 32, 48, 51, 70, 74, 100 between its stages |
//! | 1210, a scenario or a saved game | 3 | 100 once the map is cleared |
//! | 1220 (`load_game_data`, en 1038:3615) | 2 | 1, 7, 50 (and more within each loader), 100 |
//! | 1500 | 0 | 100, `load_map`, 100 |
//!
//! The form's DFM gives it the cursor `crNoDrop`, which states 1210 and
//! 1220 also set for the whole screen (`Screen.Cursor := crNoDrop`,
//! en 1038:35f1, en 1038:33c8).

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::text::{Align, Font};

/// The bitmaps, from `GRAFIK`: the picture and the gauge's empty and
/// full bitmaps. // en 1028:37d6, en 1028:37e1, en 1028:37ed
pub const BITMAP_FILES: [&str; 3] = ["WasNun.bmp", "Prozent.bmp", "Prozentp.bmp"];

/// The text's file in `TEXTE`, four lines. // en 1028:373f
pub const TEXT_FILE: &str = "WasNun.txt";

/// `Bild`'s size, which is the form's. // en 1028:37fa
pub const WIDTH: i32 = 200;
pub const HEIGHT: i32 = 100;

/// The gauge's place in the picture. // en 1028:37fa
const GAUGE: Rect = Rect::new(0x32, 0x3c, 100, 0x14);

/// The text's top and colour row. // en 1028:3a73
const TEXT_TOP: i32 = 0x15;
const TEXT_COLOUR: i32 = 9;

/// The pages of `WASNUN.TXT` the steps show.
pub mod page {
    /// "Laden der Karte" / "Loading the map...": state 1500.
    pub const LOAD_MAP: usize = 0;
    /// "Erstellen der neuen Karte": state 1210 for a new island.
    pub const NEW_MAP: usize = 1;
    /// "Initialisierung des Spiels": state 1220.
    pub const INIT_GAME: usize = 2;
    /// "Initialisierung der Karte": state 1210 for a scenario or a saved
    /// game.
    pub const INIT_MAP: usize = 3;
}

/// The progress window.
#[derive(Debug, Clone)]
pub struct WasNunForm {
    bitmaps: Vec<Surface>,
    /// `WASNUN.TXT`'s four lines. // en 1028:374a
    texts: Vec<Vec<u8>>,
    /// The text shown. // +0x182
    text: Vec<u8>,
    /// The gauge's percentage. // +0x180
    percent: i32,
    /// `Bild`'s picture.
    image: Surface,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
}

impl WasNunForm {
    /// The form as `TWasNunForm.Create` and en 1028:3b3c make it and show
    /// it: centred on a `screen` of that size, no text, the gauge empty.
    /// `bitmaps` are those of [`BITMAP_FILES`], `texts` the lines of
    /// [`TEXT_FILE`]. // en 1028:37fa, en 1028:3b3c
    #[must_use]
    pub fn show(bitmaps: Vec<Surface>, texts: Vec<Vec<u8>>, font: &Font, screen: (i32, i32)) -> Self {
        let mut form = WasNunForm {
            bitmaps,
            texts,
            text: Vec::new(),
            percent: 0,
            image: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            left: (screen.0 - WIDTH) / 2,
            top: (screen.1 - HEIGHT) / 2,
        };
        form.draw(0, Vec::new(), font);
        form
    }

    /// `wasnun_set_page`: page `n` of the text, the gauge empty.
    /// // en 1028:39e3
    pub fn set_page(&mut self, n: usize, font: &Font) {
        let text = self.texts.get(n).cloned().unwrap_or_default();
        self.draw(0, text, font);
    }

    /// The gauge at `percent`, the text as it is. // en 1028:3a09
    pub fn set_percent(&mut self, percent: i32, font: &Font) {
        let text = std::mem::take(&mut self.text);
        self.draw(percent, text, font);
    }

    /// The gauge's percentage.
    #[must_use]
    pub fn percent(&self) -> i32 {
        self.percent
    }

    /// The text shown.
    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.text
    }

    /// The picture: `WasNun.bmp`, the gauge filled to `percent` and the
    /// `text` centred. // en 1028:3a73
    fn draw(&mut self, percent: i32, text: Vec<u8>, font: &Font) {
        self.percent = percent;
        self.text = text;
        let all = self.image.bounds();
        if let Some(b) = self.bitmaps.first() {
            self.image.blit(0, 0, b, b.bounds(), Rop::Copy, all);
        }
        // `gauge_draw` (en 10c8:01dc): `width * percent div 100` of the
        // full bitmap over the empty one.
        let (l, t, w, h) = (GAUGE.left, GAUGE.top, GAUGE.width(), GAUGE.height());
        if let Some(empty) = self.bitmaps.get(1) {
            self.image.blit(l, t, empty, Rect::new(0, 0, w, h), Rop::Copy, all);
        }
        if let Some(full) = self.bitmaps.get(2) {
            let filled = w.wrapping_mul(percent) / 100;
            self.image.blit(l, t, full, Rect::new(0, 0, filled, h), Rop::Copy, all);
        }
        font.draw(
            &mut self.image,
            &self.text,
            (0, TEXT_TOP),
            WIDTH,
            Align::Center,
            TEXT_COLOUR,
        );
    }

    /// Centres the form on a `screen` of that size again (the port's
    /// screen can change its size meanwhile).
    pub fn place(&mut self, screen: (i32, i32)) {
        self.left = (screen.0 - WIDTH) / 2;
        self.top = (screen.1 - HEIGHT) / 2;
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
}
