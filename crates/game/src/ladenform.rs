//! `TLadenForm`, the load form (segment `1038`): the start menu's "Altes
//! Spiel fortsetzen" opens it (1058:0c7d) to pick a saved game from
//! `MAPS\`. Like the start menu it's one `TImage`, `Bild`, with widgets
//! of the game's toolkit drawn into it: the list of saved games, its two
//! arrows, "Laden" and the close button.
//!
//! Picking a game sets `load_map_name` (`1110:1322`) and starts
//! `hintergrund`'s states 1000–1500 with `+0x190` set, which set up a new
//! game and load the file over it ([`crate::hintergrund::load_saved_game`]).
//! The original lists `MAPS\*.map` itself; here the caller lists the
//! files and [`saved_games`] makes the list's entries of them.

use vcl::surface::Surface;

use crate::enhance::{Enhancement, Enhancements};
use crate::text::Font;
use crate::widgets::{BitmapId, Button, Frame, LIST_CAPACITY, ListBox, Outcome, ShiftState, Skin, Sound, Widget};

/// The form's size, which 1038:3b46 sets (the DFM says 427×273), and
/// `Bild`'s. // 1038:3b46
pub const WIDTH: i32 = 0x92;
pub const HEIGHT: i32 = 0xd2;

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`LadenForm::new`] takes them.
pub const BITMAP_FILES: [&str; 10] = [
    "datwin2a.bmp", // the background, +0x5f9, 1038:3b38
    // The rest are `hintergrund`'s, which state 100 loads.
    "close.bmp",    // 1110:54b8
    "closepre.bmp", // 1110:54bc
    "datB1.bmp",    // 1110:54c0
    "datB1pr.bmp",  // 1110:54c4
    "datListe.bmp", // 1110:54c8
    "datDow.bmp",   // 1110:54cc
    "datDowPr.bmp", // 1110:54d0
    "datUp.bmp",    // 1110:54d4
    "datUpPr.bmp",  // 1110:54d8
];
const BACKGROUND: BitmapId = BitmapId(0);
const CLOSE: BitmapId = BitmapId(1);
const CLOSE_PRESSED: BitmapId = BitmapId(2);
const BUTTON: BitmapId = BitmapId(3);
const BUTTON_PRESSED: BitmapId = BitmapId(4);
const LIST: BitmapId = BitmapId(5);
const DOWN: BitmapId = BitmapId(6);
const DOWN_PRESSED: BitmapId = BitmapId(7);
const UP: BitmapId = BitmapId(8);
const UP_PRESSED: BitmapId = BitmapId(9);

/// The extension the lists look for and the name gets back.
/// // 1038:397d, 1038:3a90
pub const EXTENSION: &[u8] = b".map";

/// The entries of a list of saved games, from the names of the files in
/// `MAPS\` in the directory's order: those that end in `.map` (in any
/// case), without it. Each entry goes in front of those before it, so
/// the list comes out backwards. The list box holds 100; the original
/// writes any more past its buffer, the port drops them. Used by the load
/// form and the file menu (1040:10e5). With `saved_games_order`
/// (`enhance`) they stay in the directory's order. // 1038:3983, 1038:389d, 1038:3a13
#[must_use]
pub fn saved_games(files: &[Vec<u8>], enhance: Enhancements) -> Vec<Vec<u8>> {
    // `FindFirst(MAPS\*.map)`, then `Delete(name, Length(name) - 3, 4)`.
    let mut names: Vec<Vec<u8>> = files
        .iter()
        .filter(|f| f.len() > EXTENSION.len() && f[f.len() - EXTENSION.len()..].eq_ignore_ascii_case(EXTENSION))
        .map(|f| f[..f.len() - EXTENSION.len()].to_vec())
        .collect();
    // BUG(original): the list comes out in the reverse of the directory's order.
    if !enhance.has(Enhancement::SavedGamesOrder) {
        names.reverse();
    }
    // BUG(original): past 100 files the names are written beyond the list box's buffer; the port keeps 100.
    names.truncate(LIST_CAPACITY);
    names
}

/// What the form does that leaves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// "Laden", or a double click on the selected game: the file to load
    /// from `MAPS\`, the entry and `.map`. The original hides the start
    /// menu, starts
    /// `hintergrund`'s states 1000–1500 with `+0x190` set and closes the
    /// form. // 1038:3a95
    Load(Vec<u8>),
    /// The close button: the start menu is enabled again and the form
    /// closes. // 1038:3817
    Close,
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the handler refreshes `Bild`.
    pub refresh: bool,
    pub sound: Option<Sound>,
    pub action: Option<Action>,
}

/// The widgets with a `Click` or that take the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    List,
    Up,
    Down,
    Load,
    Close,
}

/// The load form.
#[derive(Debug, Clone)]
pub struct LadenForm {
    font: Font,
    bitmaps: Vec<Surface>,
    empty: Surface,
    /// `Bild`'s picture.
    bild: Surface,
    list: ListBox, // +0x180
    up: Button,    // +0x1a1
    down: Button,  // +0x2b7
    load: Button,  // +0x3cd
    close: Button, // +0x4e3
    /// The player's view enhancements (`modifier_clicks`).
    pub enhance: Enhancements,
}

impl LadenForm {
    /// The form as 1038:3b46 sets it up and shows it: the widgets, the
    /// list of `games` ([`saved_games`]) from the top, the background in
    /// `Bild`, the arrows only where the list can scroll. `bitmaps` are
    /// those of [`BITMAP_FILES`], `load_text` is `SONSTIGE.TXT`'s
    /// "Laden". // 1038:3b46, 1038:3a13
    #[must_use]
    pub fn new(font: Font, bitmaps: Vec<Surface>, load_text: &[u8], games: Vec<Vec<u8>>) -> Self {
        let mut list = ListBox::new(Frame::new(0x18, 0x2e, 0x60, 0x6c), LIST, 9, 10, 0xf);
        list.items = games;
        list.top = 1;
        let mut f = LadenForm {
            font,
            bitmaps,
            empty: Surface::new(0, 0, 0),
            bild: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            list,
            up: Button::new(Frame::new(0x39, 0x19, 0x1c, 0x11), UP, UP_PRESSED, b"", 9),
            down: Button::new(Frame::new(0x39, 0x9f, 0x1c, 0x11), DOWN, DOWN_PRESSED, b"", 9),
            load: Button::new(Frame::new(0x22, 0xba, 0x4e, 0x11), BUTTON, BUTTON_PRESSED, load_text, 9),
            close: Button::new(Frame::new(0, 0, 0x10, 0x10), CLOSE, CLOSE_PRESSED, b"", 9),
            enhance: Enhancements::ORIGINAL,
        };
        let LadenForm {
            font,
            bitmaps,
            empty,
            bild,
            list,
            up,
            down,
            load,
            close,
            ..
        } = &mut f;
        let clip = bild.bounds();
        bild.draw(0, 0, bitmaps.get(BACKGROUND.0).unwrap_or(empty), clip);
        let skin = Skin { font, bitmaps };
        list.show(bild, &skin);
        if list.can_scroll_up() {
            up.show(bild, &skin);
        }
        if list.can_scroll_down() {
            down.show(bild, &skin);
        }
        load.show(bild, &skin);
        close.show(bild, &skin);
        f
    }

    /// Where the form goes on a `screen_width`×`screen_height` screen: at
    /// the bottom, centred across. // 1038:3b46
    #[must_use]
    pub fn position(screen_width: i32, screen_height: i32) -> (i32, i32) {
        ((screen_width - WIDTH) / 2, screen_height - HEIGHT)
    }

    /// `Bild`'s picture.
    #[must_use]
    pub fn bild(&self) -> &Surface {
        &self.bild
    }

    /// The list's entries.
    #[must_use]
    pub fn games(&self) -> &[Vec<u8>] {
        &self.list.items
    }

    /// The selected entry, from 1, 0 for none.
    #[must_use]
    pub fn selected(&self) -> i32 {
        self.list.selected
    }

    /// `BildMouseDown`, with `x`, `y` in `Bild` and the event's shift
    /// state: the list, the close button, "Laden" and the arrows, until one
    /// takes it. Any mouse button presses the buttons. // 1038:3e24
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        // 1038:36a2: the list's mouse down, with the shift state.
        let out = self.list.mouse_down(x, y, shift, self.enhance);
        if out.handled || out.clicked {
            return self.finish(Which::List, out);
        }
        for which in [Which::Close, Which::Load, Which::Down, Which::Up] {
            let out = self.button(which).mouse_down(x, y);
            if out.handled {
                return self.finish(which, out);
            }
        }
        Response::default()
    }

    /// `BildMouseUp`: "Laden", the close button and the arrows, until one
    /// takes it. // 1038:3ef5
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        for which in [Which::Load, Which::Close, Which::Down, Which::Up] {
            let out = self.button(which).mouse_up(x, y);
            if out.handled {
                return self.finish(which, out);
            }
        }
        Response::default()
    }

    fn button(&mut self, which: Which) -> &mut Button {
        match which {
            Which::Up => &mut self.up,
            Which::Down => &mut self.down,
            Which::Load => &mut self.load,
            Which::Close | Which::List => &mut self.close,
        }
    }

    /// Repaints the widget if it asks for it, then runs its `Click`.
    fn finish(&mut self, which: Which, out: Outcome) -> Response {
        if out.repaint {
            self.redraw(which);
        }
        let action = if out.clicked { self.click(which) } else { None };
        Response {
            refresh: out.handled || out.repaint,
            sound: out.sound,
            action,
        }
    }

    fn redraw(&mut self, which: Which) {
        let LadenForm {
            font, bitmaps, bild, ..
        } = self;
        let skin = Skin { font, bitmaps };
        match which {
            Which::List => self.list.draw(bild, &skin),
            Which::Up => self.up.draw(bild, &skin),
            Which::Down => self.down.draw(bild, &skin),
            Which::Load => self.load.draw(bild, &skin),
            Which::Close => self.close.draw(bild, &skin),
        }
    }

    /// The widgets' `Click` overrides.
    fn click(&mut self, which: Which) -> Option<Action> {
        match which {
            // 1038:3682 (a double click on the selected game), 1038:3836.
            Which::List | Which::Load => self.list.has_selection().then(|| self.load_name()),
            Which::Close => Some(Action::Close),
            Which::Up => {
                self.scroll(-1);
                None
            }
            Which::Down => {
                self.scroll(1);
                None
            }
        }
    }

    /// The mouse wheel over the form (`mouse_wheel`, a port extension):
    /// the list moves [`crate::widgets::WHEEL_STEPS`] rows a notch,
    /// `notches` up, as the arrows move it (they hide and show as they
    /// do).
    pub fn wheel(&mut self, notches: i32) -> Response {
        let top = self.list.top;
        let by = if notches > 0 { -1 } else { 1 };
        for _ in 0..notches
            .unsigned_abs()
            .saturating_mul(crate::widgets::WHEEL_STEPS.unsigned_abs())
        {
            self.scroll(by);
        }
        Response {
            refresh: self.list.top != top,
            ..Response::default()
        }
    }

    /// The up (`by` -1, 1038:36d5) and down (1038:3776) arrows: the list
    /// moves a row if it can, the arrow hides once it can't any more, and
    /// the other arrow shows if the list can go its way.
    fn scroll(&mut self, by: i32) {
        let can = |l: &ListBox| if by < 0 { l.can_scroll_up() } else { l.can_scroll_down() };
        let can_back = |l: &ListBox| if by < 0 { l.can_scroll_down() } else { l.can_scroll_up() };
        let LadenForm {
            font,
            bitmaps,
            empty,
            bild,
            list,
            up,
            down,
            ..
        } = self;
        let background = bitmaps.get(BACKGROUND.0).unwrap_or(empty);
        let skin = Skin { font, bitmaps };
        let (this, other) = if by < 0 { (up, down) } else { (down, up) };
        if can(list) {
            list.top += by;
            list.draw(bild, &skin);
        }
        if !can(list) {
            this.hide(bild, background);
        }
        if can_back(list) {
            other.show(bild, &skin);
        }
    }

    /// The selected game's file: `load_map_name := items[selected] +
    /// '.map'`. `load_map_name` is a `string[12]`, which the 8.3 names
    /// the original finds always fit; the port doesn't cut a longer host
    /// file name, which it couldn't find again. // 1038:3a95, 1110:1322
    fn load_name(&self) -> Action {
        let index = usize::try_from(self.list.selected - 1).unwrap_or(0);
        let mut name = self.list.items.get(index).cloned().unwrap_or_default();
        name.extend_from_slice(EXTENSION);
        Action::Load(name)
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

    fn form(n: usize) -> LadenForm {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(200, 220, u8::try_from(i + 1).unwrap()))
            .collect();
        let games = (0..n).map(|i| format!("GAME{i}").into_bytes()).collect();
        LadenForm::new(font(), bitmaps, b"Laden", games)
    }

    #[test]
    fn lists_saved_games_backwards() {
        let files = [&b"LAST.MAP"[..], b"notes.txt", b"Spiel1.map", b".map", b"x.MAPS"].map(<[u8]>::to_vec);
        assert_eq!(
            saved_games(&files, Enhancements::ORIGINAL),
            [b"Spiel1".to_vec(), b"LAST".to_vec()]
        );
        let many: Vec<_> = (0..120).map(|i| format!("S{i}.MAP").into_bytes()).collect();
        let games = saved_games(&many, Enhancements::ORIGINAL);
        assert_eq!(games.len(), 100);
        assert_eq!(games[0], b"S119");
        // With the fix, in the given (sorted) order.
        let mut e = Enhancements::ORIGINAL;
        e.set(Enhancement::SavedGamesOrder, true);
        assert_eq!(saved_games(&files, e), [b"LAST".to_vec(), b"Spiel1".to_vec()]);
        assert_eq!(saved_games(&many, e)[0], b"S0");
    }

    #[test]
    fn picks_a_game() {
        let mut f = form(3);
        // Three games fit: no arrows.
        assert!(!f.up.frame.visible && !f.down.frame.visible && f.load.frame.visible);
        assert_eq!(f.bild.pixel(70, 100), 6, "the list's background");
        // "Laden" with nothing selected does nothing.
        let r = f.mouse_down(0x30, 0xc0, ShiftState::LEFT);
        assert!(r.refresh && r.sound == Some(Sound::Press));
        assert_eq!(f.mouse_up(0x30, 0xc0).action, None);
        // The second row, then "Laden".
        let r = f.mouse_down(0x30, 0x2e + 0xf + 3, ShiftState::LEFT);
        assert_eq!((r.refresh, r.sound), (true, Some(Sound::Select)));
        assert_eq!(f.selected(), 2);
        let _ = f.mouse_down(0x30, 0xc0, ShiftState::LEFT);
        assert_eq!(f.mouse_up(0x30, 0xc0).action, Some(Action::Load(b"GAME1.map".to_vec())));
        // A double click on the selected row loads it at once.
        let r = f.mouse_down(0x30, 0x2e + 0xf + 3, ShiftState::LEFT | ShiftState::DOUBLE);
        assert_eq!(r.action, Some(Action::Load(b"GAME1.map".to_vec())));
        // The close button.
        let _ = f.mouse_down(3, 3, ShiftState::LEFT);
        assert_eq!(f.mouse_up(3, 3).action, Some(Action::Close));
    }

    #[test]
    fn wheel_scrolls() {
        let mut f = form(12);
        assert!(f.wheel(-1).refresh);
        assert_eq!(f.list.top, 4);
        assert!(f.up.frame.visible && f.down.frame.visible);
        // Twelve games, seven rows: row 6 is the last top.
        let _ = f.wheel(-5);
        assert_eq!(f.list.top, 6);
        assert!(!f.down.frame.visible);
        assert!(!f.wheel(-1).refresh);
        let _ = f.wheel(1);
        assert_eq!(f.list.top, 3);
        let _ = f.wheel(1);
        assert_eq!(f.list.top, 1);
        assert!(!f.up.frame.visible);
    }

    #[test]
    fn arrows_scroll() {
        let mut f = form(9);
        assert!(!f.up.frame.visible && f.down.frame.visible);
        let down: (i32, i32) = (0x39 + 4, 0x9f + 4);
        for _ in 0..2 {
            let _ = f.mouse_down(down.0, down.1, ShiftState::LEFT);
            let _ = f.mouse_up(down.0, down.1);
        }
        // Seven rows show: two steps reach the end, and the arrow hides.
        assert_eq!(f.list.top, 3);
        assert!(f.up.frame.visible && !f.down.frame.visible);
        assert_eq!(f.bild.pixel(0x3d, 0xa3), 1, "erased to the background");
        let up = (0x39 + 4, 0x19 + 4);
        let _ = f.mouse_down(up.0, up.1, ShiftState::LEFT);
        let _ = f.mouse_up(up.0, up.1);
        assert_eq!(f.list.top, 2);
        assert!(f.up.frame.visible && f.down.frame.visible);
    }

    #[test]
    fn position() {
        assert_eq!(LadenForm::position(800, 600), (327, 390));
    }
}
