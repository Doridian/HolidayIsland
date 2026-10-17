//! The list of the enhancements (`crate::enhance`), a port extension: the
//! options' "INFO" opens it with `enhancements_screen` on, where the
//! original opens the credits ([`crate::creditform`], which a button here
//! opens). It's a modal form like the options, 640 × 480 like the
//! credits, centred on the screen, drawn with the game's own bitmaps,
//! font and widgets.
//!
//! Two lists of check boxes, each with a scroll bar: "Spielkorrekturen",
//! the world's fixes ([`Fix`]), and "Anzeige und Ton", the player's view
//! enhancements ([`Enhancement`]). Each row is a key's German label
//! ([`crate::enhance::Info::label`]); the one under the mouse has its key
//! and description shown below the lists. Buttons switch every one on or
//! off (the original), open the credits and close the form.
//!
//! A fix comes back as a command ([`Action::SetFix`], through
//! `World::apply` like any other) and as a [`Change`] for the caller,
//! which makes it the player's default for new games too; a view
//! enhancement is only a [`Change`], which the caller applies at once.
//! The caller saves both in its `settings.ini`. The form keeps no state
//! of its own beyond what it shows: [`Enhanceform::show`] takes the
//! world's fixes and the player's set.
//!
//! [`Action::SetFix`]: crate::command::Action::SetFix

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::command;
use crate::enhance::{Enhancement, Enhancements, Fix, Fixes, Info};
use crate::text::{Align, Font, LINE_HEIGHT};
use crate::widgets::{
    BitmapId, Button, CheckBox, Frame, Label, Outcome, ScrollBar, ScrollBitmaps, ShiftState, Skin, Sound, Widget,
};

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`Enhanceform::new`] takes them: the high scores' background (which
/// the form's is made from), the options' check box, the buildings'
/// overview's scroll bar, the credits' button and `hintergrund`'s close
/// button.
pub const BITMAP_FILES: [&str; 13] = [
    "highscor.bmp",
    "optch.bmp",
    "optchpr.bmp",
    "menuscu.bmp",
    "menuscup.bmp",
    "menuscd.bmp",
    "menuscdp.bmp",
    "menuscli.bmp",
    "menuscrl.bmp",
    "stkn.bmp",
    "stknpr.bmp",
    "close.bmp",
    "closepre.bmp",
];
const BACKGROUND: BitmapId = BitmapId(0);
const CHECK: BitmapId = BitmapId(1);
const CHECKED: BitmapId = BitmapId(2);
const SCROLL: ScrollBitmaps = ScrollBitmaps {
    back: BitmapId(3),
    back_pressed: BitmapId(4),
    forward: BitmapId(5),
    forward_pressed: BitmapId(6),
    thumb: BitmapId(7),
    track: BitmapId(8),
};
const BUTTON: BitmapId = BitmapId(9);
const BUTTON_PRESSED: BitmapId = BitmapId(10);
const CLOSE: BitmapId = BitmapId(11);
const CLOSE_PRESSED: BitmapId = BitmapId(12);

/// The form's size: the credits'.
pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 480;

/// The two lists' left edges, and how wide each is with its scroll bar.
const LIST_LEFT: [i32; 2] = [12, 328];
const LIST_WIDTH: i32 = 300;
/// A row's check box with its label, and the scroll bar right of them.
const ROW_WIDTH: i32 = 280;
const BAR_WIDTH: i32 = 16;
const LIST_TOP: i32 = 60;
const ROW_HEIGHT: i32 = 18;
/// Rows a list shows at once.
const ROWS: i32 = 18;
const LIST_HEIGHT: i32 = ROWS * ROW_HEIGHT;
/// The description of the row under the mouse (two lines), and the notes
/// (two lines) under it.
const INFO_TOP: i32 = 390;
const NOTES_TOP: i32 = 420;
const TEXT_LEFT: i32 = 12;
const TEXT_WIDTH: i32 = WIDTH - 2 * TEXT_LEFT;
/// The buttons along the bottom.
const BUTTON_TOP: i32 = 452;
const BUTTON_WIDTH: i32 = 0x90;
const BUTTON_HEIGHT: i32 = 0x16;
const BUTTON_LEFT: [i32; 4] = [12, 169, 327, 484];

/// The font's colour rows the form uses.
const YELLOW: i32 = 1;
const BLUE: i32 = 7;
const BLACK: i32 = 9;
const RED: i32 = 11;

/// The form's own texts, in the game's language. `Schließen` is
/// `SONSTIGE.TXT`'s, which the caller hands to [`Enhanceform::new`].
const TITLE: &str = "ERWEITERUNGEN";
const HEADINGS: [&str; 2] = ["Spielkorrekturen", "Anzeige und Ton"];
const SUBHEADINGS: [&str; 2] = ["Für dieses Spiel und neue Spiele", "Ihre Einstellungen für jedes Spiel"];
const ALL_ON: &str = "Alle an";
const ALL_OFF: &str = "Alle aus (Original)";
const CREDITS: &str = "Mitwirkende";
const NOTE_SAVED: &str = "Änderungen gelten sofort und werden in settings.ini gespeichert.";
const NOTE_NOT_SAVED: &str = "Änderungen gelten sofort, aber nur bis zum Beenden (keine settings.ini).";
const NOTE_COMMAND_LINE: &str = "Beim nächsten Start gehen --original und --enhance wieder vor.";
const NOTE_SCREEN_OFF: &str = "INFO zeigt nun die Mitwirkenden (zurück: enhance.enhancements_screen=on).";
const HINT: &str = "Die Maus über einer Zeile zeigt hier, was sie ändert.";

/// The keys of [`Enhanceform::key_down`].
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_F4: u16 = 0x73;

/// `text` in the game's Windows-1252 font: the Latin-1 letters as they
/// are, anything else a `?`.
fn german(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

/// What [`Enhanceform::show`] says under the lists about saving.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Notes {
    /// The changes go into `settings.ini`.
    pub saved: bool,
    /// `--original` or `--enhance=` was given, which go over
    /// `settings.ini` when the program starts.
    pub command_line: bool,
}

/// What the form asks of the game besides its commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The close button, "Schließen", Escape, Enter or Alt+F4: the form
    /// hides and the options are enabled again.
    Close,
    /// "Mitwirkende": the credits open over the form, which is disabled
    /// until they close.
    Credits,
}

/// A switch the player changed, for the caller to apply to the player's
/// settings and save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A world fix; the form also sends its command. The caller makes it
    /// the default for new games.
    Fix(Fix, bool),
    /// A view enhancement, which the caller applies at once.
    View(Enhancement, bool),
    /// "Alle an" (true) or "Alle aus (Original)" (false): every one, the
    /// world's fixes by command as well.
    All(bool),
}

/// What an event did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the image is drawn again.
    pub refresh: bool,
    /// The widgets' sounds, in order (`play_sound`, 10a0:30b6).
    pub sounds: Vec<Sound>,
    /// The world's fixes that changed, as the player's commands.
    pub commands: Vec<command::Action>,
    pub changes: Vec<Change>,
    pub action: Option<Action>,
}

/// A row's switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Fix(Fix),
    View(Enhancement),
}

impl Key {
    fn info(self) -> Info {
        match self {
            Key::Fix(f) => f.info(),
            Key::View(e) => e.info(),
        }
    }
}

/// One of the two lists.
#[derive(Debug, Clone)]
struct Pane {
    left: i32,
    keys: Vec<Key>,
    rows: Vec<CheckBox>,
    bar: ScrollBar,
}

impl Pane {
    fn new(left: i32, keys: Vec<Key>) -> Pane {
        let rows = keys
            .iter()
            .map(|k| {
                let label = Label::new(
                    Frame::new(left, 0, ROW_WIDTH, 16),
                    &german(k.info().label),
                    Align::Left,
                    BLACK,
                );
                CheckBox::new(label, CHECK, CHECKED, BLACK)
            })
            .collect();
        let mut bar = ScrollBar::new(
            Frame::new(left + LIST_WIDTH - BAR_WIDTH, LIST_TOP, BAR_WIDTH, LIST_HEIGHT),
            SCROLL,
        );
        let rows_n = i32::try_from(keys.len()).unwrap_or(i32::MAX);
        bar.min = 0;
        bar.max = (rows_n - ROWS).max(0);
        bar.position = 0;
        bar.large_step = ROWS - 1;
        bar.frame.visible = true;
        Pane { left, keys, rows, bar }
    }

    /// The first row shown.
    fn top(&self) -> usize {
        usize::try_from(self.bar.position).unwrap_or(0)
    }

    /// The rows from the bar's position on in their places, the others
    /// hidden (a hidden widget takes no press).
    fn place_rows(&mut self) {
        let top = self.top();
        for (i, row) in self.rows.iter_mut().enumerate() {
            let shown = i >= top && i < top + ROWS as usize;
            let f = &mut row.radio.frame;
            f.visible = shown;
            f.top = LIST_TOP + i32::try_from(i.saturating_sub(top)).unwrap_or(0) * ROW_HEIGHT;
        }
    }

    fn rect(&self) -> Rect {
        Rect::new(self.left, LIST_TOP, LIST_WIDTH, LIST_HEIGHT)
    }

    /// The row under `x`, `y`, if one is shown there.
    fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        if !(self.left..self.left + ROW_WIDTH).contains(&x) || !(LIST_TOP..LIST_TOP + LIST_HEIGHT).contains(&y) {
            return None;
        }
        let i = self.top() + usize::try_from((y - LIST_TOP) / ROW_HEIGHT).ok()?;
        (i < self.rows.len()).then_some(i)
    }
}

/// The list of the enhancements.
#[derive(Debug, Clone)]
pub struct Enhanceform {
    font: Font,
    bitmaps: Vec<Surface>,
    /// The form's background with its fixed texts: the title, the
    /// lists' headings and frames.
    background: Surface,
    image: Surface,
    pub visible: bool,
    /// The form's top left on the screen.
    pub left: i32,
    pub top: i32,
    panes: [Pane; 2],
    /// The close button, "Alle an", "Alle aus (Original)", "Mitwirkende"
    /// and "Schließen".
    buttons: [Button; 5],
    fixes: Fixes,
    view: Enhancements,
    notes: Notes,
    /// The row under the mouse: its list and index.
    hover: Option<(usize, usize)>,
    /// The player's view enhancements (`scroll_bars`).
    pub enhance: Enhancements,
}

const CLOSE_BUTTON: usize = 0;
const ALL_ON_BUTTON: usize = 1;
const ALL_OFF_BUTTON: usize = 2;
const CREDITS_BUTTON: usize = 3;
const SCHLIESSEN_BUTTON: usize = 4;

impl Enhanceform {
    /// The form, not shown. `bitmaps` are those of [`BITMAP_FILES`],
    /// `schliessen` `SONSTIGE.TXT`'s "Schließen".
    #[must_use]
    pub fn new(font: Font, mut bitmaps: Vec<Surface>, schliessen: &[u8]) -> Self {
        // The scroll bar's trough is 200 high, the bars longer: it's
        // repeated down.
        if let Some(track) = bitmaps.get(SCROLL.track.0) {
            let mut long = Surface::new(BAR_WIDTH as usize, LIST_HEIGHT as usize, 0);
            let step = i32::try_from(track.height).unwrap_or(0).max(1);
            let clip = long.bounds();
            for y in (0..LIST_HEIGHT).step_by(usize::try_from(step).unwrap_or(1)) {
                long.blit(0, y, track, track.bounds(), Rop::Copy, clip);
            }
            bitmaps[SCROLL.track.0] = long;
        }
        let background = Self::background(&font, &bitmaps);
        let button = |n: usize, text: &[u8]| {
            Button::new(
                Frame::new(BUTTON_LEFT[n], BUTTON_TOP, BUTTON_WIDTH, BUTTON_HEIGHT),
                BUTTON,
                BUTTON_PRESSED,
                text,
                BLACK,
            )
        };
        Enhanceform {
            buttons: [
                Button::new(Frame::new(0, 0, 16, 16), CLOSE, CLOSE_PRESSED, b"", BLACK),
                button(0, &german(ALL_ON)),
                button(1, &german(ALL_OFF)),
                button(2, &german(CREDITS)),
                button(3, schliessen),
            ],
            panes: Self::panes(),
            font,
            bitmaps,
            image: background.clone(),
            background,
            visible: false,
            left: 0,
            top: 0,
            fixes: Fixes::ORIGINAL,
            view: Enhancements::ORIGINAL,
            notes: Notes::default(),
            hover: None,
            enhance: Enhancements::ORIGINAL,
        }
    }

    fn panes() -> [Pane; 2] {
        [
            Pane::new(LIST_LEFT[0], Fix::ALL.iter().map(|&f| Key::Fix(f)).collect()),
            Pane::new(LIST_LEFT[1], Enhancement::ALL.iter().map(|&e| Key::View(e)).collect()),
        ]
    }

    /// The form's background: the high scores', its title bar and the
    /// clean top of its face, the face mirrored on down to the bottom
    /// edge; then the title, the lists' headings and a frame round each
    /// list in the colour of the form's edge.
    fn background(font: &Font, bitmaps: &[Surface]) -> Surface {
        let mut bg = Surface::new(WIDTH as usize, HEIGHT as usize, 0);
        let clip = bg.bounds();
        if let Some(src) = bitmaps.get(BACKGROUND.0) {
            let src_height = i32::try_from(src.height).unwrap_or(0);
            // The rows 20 to 230 of the face are clear of its widgets.
            let (clear_top, clear_bottom) = (20, 230.min(src_height));
            let edge = 4;
            for y in 0..HEIGHT {
                let from = if y < clear_bottom {
                    y
                } else if y >= HEIGHT - edge {
                    src_height - (HEIGHT - y)
                } else {
                    let span = (clear_bottom - clear_top).max(1);
                    let k = (y - clear_bottom) % (2 * span);
                    if k < span {
                        clear_bottom - 1 - k
                    } else {
                        clear_top + (k - span)
                    }
                };
                bg.blit(0, y, src, Rect::new(0, from, WIDTH, 1), Rop::Copy, clip);
            }
        }
        let edge_colour = bg.pixel(0, (HEIGHT / 2) as usize);
        font.draw(&mut bg, &german(TITLE), (16, 2), WIDTH - 32, Align::Center, YELLOW);
        for (n, &left) in LIST_LEFT.iter().enumerate() {
            font.draw(&mut bg, &german(HEADINGS[n]), (left, 22), LIST_WIDTH, Align::Left, BLUE);
            font.draw(
                &mut bg,
                &german(SUBHEADINGS[n]),
                (left, 38),
                LIST_WIDTH,
                Align::Left,
                BLACK,
            );
            let r = Rect::new(left - 3, LIST_TOP - 3, LIST_WIDTH + 6, LIST_HEIGHT + 6);
            for line in [
                Rect::new(r.left, r.top, r.width(), 1),
                Rect::new(r.left, r.bottom - 1, r.width(), 1),
                Rect::new(r.left, r.top, 1, r.height()),
                Rect::new(r.right - 1, r.top, 1, r.height()),
            ] {
                bg.fill_rect(line, clip, edge_colour);
            }
        }
        bg
    }

    /// Opens the form centred on a `screen` of that size, with the
    /// world's `fixes` and the player's `view` set ticked, the lists at
    /// their tops and `notes` under them.
    pub fn show(&mut self, fixes: Fixes, view: Enhancements, notes: Notes, screen: (i32, i32)) -> Response {
        self.fixes = fixes;
        self.view = view;
        self.notes = notes;
        self.hover = None;
        self.panes = Self::panes();
        self.left = (screen.0 - WIDTH) / 2;
        self.top = (screen.1 - HEIGHT) / 2;
        for p in &mut self.panes {
            for (row, key) in p.rows.iter_mut().zip(&p.keys) {
                row.radio.on = match *key {
                    Key::Fix(f) => fixes.has(f),
                    Key::View(e) => view.has(e),
                };
            }
            p.place_rows();
        }
        let clip = self.image.bounds();
        self.image.blit(0, 0, &self.background, clip, Rop::Copy, clip);
        let Enhanceform {
            font,
            bitmaps,
            image,
            buttons,
            ..
        } = self;
        let skin = Skin { font, bitmaps };
        for b in buttons.iter_mut() {
            b.frame.visible = false;
            b.pressed = false;
            b.show(image, &skin);
        }
        for p in 0..self.panes.len() {
            self.draw_pane(p);
        }
        self.draw_info();
        self.visible = true;
        Response {
            refresh: true,
            ..Response::default()
        }
    }

    /// The image.
    #[must_use]
    pub fn image(&self) -> &Surface {
        &self.image
    }

    /// The form on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, HEIGHT)
    }

    /// The world's fixes as the form has them.
    #[must_use]
    pub fn fixes(&self) -> Fixes {
        self.fixes
    }

    /// The player's view enhancements as the form has them.
    #[must_use]
    pub fn view(&self) -> Enhancements {
        self.view
    }

    /// The first row list `pane` shows.
    #[must_use]
    pub fn scrolled(&self, pane: usize) -> usize {
        self.panes.get(pane).map_or(0, Pane::top)
    }

    /// List `pane`'s scroll bar's place in the form.
    #[must_use]
    pub fn bar_frame(&self, pane: usize) -> Option<Frame> {
        self.panes.get(pane).map(|p| p.bar.frame)
    }

    /// List `pane` scrolled to row `top` (as far as it goes).
    pub fn scroll_to(&mut self, pane: usize, top: usize) {
        if let Some(p) = self.panes.get_mut(pane) {
            p.bar.position = i32::try_from(top).unwrap_or(i32::MAX).min(p.bar.max);
            p.place_rows();
            self.draw_pane(pane);
        }
    }

    /// Where the check box of `key`'s row is in the form, while it's
    /// shown.
    #[must_use]
    pub fn row_of(&self, key: &str) -> Option<(i32, i32)> {
        self.panes.iter().find_map(|p| {
            let i = p.keys.iter().position(|k| k.info().key == key)?;
            let f = &p.rows[i].radio.frame;
            f.visible.then_some((f.left + 4, f.top + 4))
        })
    }

    /// Where button `n` (0 close, 1 "Alle an", 2 "Alle aus", 3
    /// "Mitwirkende", 4 "Schließen") is in the form.
    #[must_use]
    pub fn button_at(&self, n: usize) -> Option<(i32, i32)> {
        self.buttons.get(n).map(|b| (b.frame.left + 4, b.frame.top + 4))
    }

    /// List `p` drawn again: its face from the background, the rows
    /// shown and the scroll bar.
    fn draw_pane(&mut self, p: usize) {
        let Enhanceform {
            font,
            bitmaps,
            image,
            background,
            panes,
            ..
        } = self;
        let pane = &panes[p];
        let r = pane.rect();
        let clip = image.bounds();
        image.blit(r.left, r.top, background, r, Rop::Copy, clip);
        let skin = Skin { font, bitmaps };
        for row in pane.rows.iter().filter(|row| row.radio.frame.visible) {
            row.draw(image, &skin);
        }
        pane.bar.draw(image, &skin);
    }

    /// The description of the row under the mouse, and the notes.
    fn draw_info(&mut self) {
        let r = Rect::new(TEXT_LEFT, INFO_TOP, TEXT_WIDTH, BUTTON_TOP - INFO_TOP - 2);
        let clip = self.image.bounds();
        self.image.blit(r.left, r.top, &self.background, r, Rop::Copy, clip);
        let font = &self.font;
        let (text, colour) = match self.hover {
            Some((p, i)) => {
                let info = self.panes[p].keys[i].info();
                (format!("{}: {}", info.key, info.description), BLUE)
            }
            None => (HINT.to_owned(), BLACK),
        };
        font.draw(
            &mut self.image,
            &german(&text),
            (TEXT_LEFT, INFO_TOP),
            TEXT_WIDTH,
            Align::Left,
            colour,
        );
        let mut notes = vec![if self.notes.saved { NOTE_SAVED } else { NOTE_NOT_SAVED }];
        if !self.view.has(Enhancement::EnhancementsScreen) {
            notes.push(NOTE_SCREEN_OFF);
        } else if self.notes.command_line && self.notes.saved {
            notes.push(NOTE_COMMAND_LINE);
        }
        for (n, note) in (0..).zip(notes) {
            let colour = if note == NOTE_SCREEN_OFF { RED } else { BLACK };
            let at = (TEXT_LEFT, NOTES_TOP + n * LINE_HEIGHT);
            font.draw(&mut self.image, &german(note), at, TEXT_WIDTH, Align::Left, colour);
        }
    }

    fn draw_button(&mut self, n: usize) {
        let Enhanceform {
            font,
            bitmaps,
            image,
            buttons,
            ..
        } = self;
        buttons[n].draw(image, &Skin { font, bitmaps });
    }

    /// Row `i` of list `p` switched to `on`: the form's sets, the command
    /// for a fix, the change for the caller.
    fn switched(&mut self, p: usize, i: usize, on: bool, r: &mut Response) {
        match self.panes[p].keys[i] {
            Key::Fix(f) => {
                self.fixes.set(f, on);
                // Single human: whoever switches a fix switches it for
                // everybody; in a multiplayer game it would be the host's.
                r.commands.push(command::Action::SetFix(f, on));
                r.changes.push(Change::Fix(f, on));
            }
            Key::View(e) => {
                self.view.set(e, on);
                r.changes.push(Change::View(e, on));
                if e == Enhancement::EnhancementsScreen {
                    self.draw_info();
                }
            }
        }
    }

    /// "Alle an" or "Alle aus (Original)": every row, a command for each
    /// fix that changes.
    fn all(&mut self, on: bool, r: &mut Response) {
        let fixes = if on { Fixes::all_on() } else { Fixes::ORIGINAL };
        for &f in Fix::ALL {
            if self.fixes.has(f) != fixes.has(f) {
                r.commands.push(command::Action::SetFix(f, on));
            }
        }
        self.fixes = fixes;
        self.view = if on {
            Enhancements::all_on()
        } else {
            Enhancements::ORIGINAL
        };
        for pane in &mut self.panes {
            for row in &mut pane.rows {
                row.radio.on = on;
            }
        }
        r.changes.push(Change::All(on));
        r.sounds.push(Sound::Select);
        for p in 0..self.panes.len() {
            self.draw_pane(p);
        }
        self.draw_info();
    }

    /// A scroll bar's [`Outcome`]: the sound, and on its `Click` the
    /// rows follow; the list drawn again.
    fn bar_outcome(&mut self, p: usize, out: Outcome, r: &mut Response) {
        r.sounds.extend(out.sound);
        if out.clicked {
            self.panes[p].place_rows();
            if let Some((hp, _)) = self.hover
                && hp == p
            {
                self.hover = None;
                self.draw_info();
            }
        }
        if out.repaint || out.clicked {
            self.draw_pane(p);
        }
        r.refresh = true;
    }

    /// A press at `x`, `y` in the form, with the left button only: a
    /// button goes down, a row's check box toggles, or a scroll bar is
    /// pressed.
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        let mut r = Response::default();
        if !shift.contains(ShiftState::LEFT) {
            return r;
        }
        for n in 0..self.buttons.len() {
            let out = self.buttons[n].mouse_down(x, y);
            if out.handled {
                r.sounds.extend(out.sound);
                self.draw_button(n);
                r.refresh = true;
                return r;
            }
        }
        for p in 0..self.panes.len() {
            let hit = (0..self.panes[p].rows.len()).find(|&i| self.panes[p].rows[i].radio.frame.hit(x, y));
            if let Some(i) = hit {
                let out = self.panes[p].rows[i].toggle();
                r.sounds.extend(out.sound);
                let on = self.panes[p].rows[i].checked();
                self.switched(p, i, on, &mut r);
                self.draw_pane(p);
                r.refresh = true;
                return r;
            }
            let out = self.panes[p].bar.mouse_down(x, y, self.enhance);
            if out.handled {
                self.bar_outcome(p, out, &mut r);
                return r;
            }
        }
        r
    }

    /// The mouse moves to `x`, `y`: a scroll bar's thumb follows it, and
    /// the row under it has its description shown.
    pub fn mouse_move(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        for p in 0..self.panes.len() {
            let out = self.panes[p].bar.mouse_move(x, y);
            if out.handled {
                self.bar_outcome(p, out, &mut r);
                return r;
            }
        }
        let hover = (0..self.panes.len()).find_map(|p| Some((p, self.panes[p].row_at(x, y)?)));
        if hover != self.hover {
            self.hover = hover;
            self.draw_info();
            r.refresh = true;
        }
        r
    }

    /// A release at `x`, `y`: a button that was down comes up (and does
    /// what it's for if the mouse is still on it), or a scroll bar lets
    /// go.
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Response {
        let mut r = Response::default();
        for n in 0..self.buttons.len() {
            let out = self.buttons[n].mouse_up(x, y);
            if !out.handled {
                continue;
            }
            self.draw_button(n);
            r.refresh = true;
            r.sounds.extend(out.sound);
            if out.clicked {
                match n {
                    CLOSE_BUTTON | SCHLIESSEN_BUTTON => {
                        self.visible = false;
                        r.action = Some(Action::Close);
                    }
                    ALL_ON_BUTTON => self.all(true, &mut r),
                    ALL_OFF_BUTTON => self.all(false, &mut r),
                    CREDITS_BUTTON => r.action = Some(Action::Credits),
                    _ => {}
                }
            }
            return r;
        }
        for p in 0..self.panes.len() {
            let out = self.panes[p].bar.mouse_up(x, y, self.enhance);
            if out.handled {
                self.bar_outcome(p, out, &mut r);
                return r;
            }
        }
        r
    }

    /// `scroll_bars`' repeat of a scroll arrow held down
    /// ([`ScrollBar::repeat`]), `ms` after the last call.
    pub fn repeat(&mut self, ms: u32) -> Response {
        let mut r = Response::default();
        for p in 0..self.panes.len() {
            let out = self.panes[p].bar.repeat(ms, self.enhance);
            if out.handled {
                self.bar_outcome(p, out, &mut r);
            }
        }
        r
    }

    /// The mouse wheel at `x`, `y` in the form (`mouse_wheel`): the list
    /// under it scrolls `notches` up ([`ScrollBar::wheel`]).
    pub fn wheel(&mut self, x: i32, y: i32, notches: i32) -> Response {
        let mut r = Response::default();
        let on =
            |p: &Pane| (p.left..p.left + LIST_WIDTH).contains(&x) && (LIST_TOP..LIST_TOP + LIST_HEIGHT).contains(&y);
        if let Some(p) = self.panes.iter().position(on) {
            let out = self.panes[p].bar.wheel(notches);
            if out.handled {
                self.bar_outcome(p, out, &mut r);
            }
        }
        r
    }

    /// A key: Escape, Enter and Alt+F4 close the form, as the options'.
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Response {
        let mut r = Response::default();
        let plain = shift.0 == 0 || shift == ShiftState::SHIFT;
        if (plain && matches!(key, VK_ESCAPE | VK_RETURN)) || (shift == ShiftState::ALT && key == VK_F4) {
            self.visible = false;
            r.action = Some(Action::Close);
            r.refresh = true;
        }
        r
    }
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

    fn form() -> Enhanceform {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(640, 300, u8::try_from(i + 1).unwrap()))
            .collect();
        let mut f = Enhanceform::new(font(), bitmaps, b"Schlie\xdfen");
        let _ = f.show(
            Fixes::all_on(),
            Enhancements::all_on(),
            Notes {
                saved: true,
                command_line: false,
            },
            (800, 600),
        );
        f
    }

    fn click(f: &mut Enhanceform, at: impl FnOnce(&Enhanceform) -> (i32, i32)) -> Response {
        let (x, y) = at(f);
        let mut down = f.mouse_down(x, y, ShiftState::LEFT);
        let up = f.mouse_up(x, y);
        down.commands.extend(up.commands);
        down.changes.extend(up.changes);
        down.action = down.action.or(up.action);
        down
    }

    #[test]
    fn shows_every_key_ticked() {
        let f = form();
        assert_eq!(f.bounds(), Rect::new(80, 60, WIDTH, HEIGHT));
        assert_eq!(f.panes[0].rows.len(), Fix::ALL.len());
        assert_eq!(f.panes[1].rows.len(), Enhancement::ALL.len());
        assert!(f.panes.iter().flat_map(|p| &p.rows).all(CheckBox::checked));
        // The first rows are shown, the rest hidden until scrolled to.
        assert!(f.row_of("site_effect").is_some() && f.row_of("window_stacking").is_some());
        assert!(f.row_of("tv_lights").is_none());
    }

    #[test]
    fn a_fix_is_a_command_and_a_change() {
        let mut f = form();
        let r = click(&mut f, |f| f.row_of("ship_detour").unwrap());
        assert_eq!(r.commands, [command::Action::SetFix(Fix::ShipDetour, false)]);
        assert_eq!(r.changes, [Change::Fix(Fix::ShipDetour, false)]);
        assert_eq!(r.sounds, [Sound::Select]);
        assert!(!f.fixes().has(Fix::ShipDetour));
        // A view enhancement is only a change.
        let r = click(&mut f, |f| f.row_of("big_map").unwrap());
        assert_eq!(r.commands, []);
        assert_eq!(r.changes, [Change::View(Enhancement::BigMap, false)]);
        assert!(!f.view().has(Enhancement::BigMap));
    }

    #[test]
    fn scrolls_to_the_last_rows() {
        let mut f = form();
        // The bar's lower arrow, a release on it: one row down.
        let bar = f.panes[0].bar.frame;
        let arrow = (bar.left + 4, bar.top + bar.height - 4);
        let _ = click(&mut f, |_| arrow);
        assert_eq!(f.scrolled(0), 1);
        // The trough pages; it stops at the last page.
        for _ in 0..5 {
            let _ = click(&mut f, |_| (bar.left + 4, bar.top + bar.height - 40));
        }
        assert_eq!(f.scrolled(0), Fix::ALL.len() - ROWS as usize);
        let last = f.row_of("tv_lights").unwrap();
        assert!(f.row_of("site_effect").is_none());
        let r = click(&mut f, |_| last);
        assert_eq!(r.changes, [Change::Fix(Fix::TvLights, false)]);
    }

    #[test]
    fn all_off_and_on() {
        let mut f = form();
        let _ = click(&mut f, |f| f.row_of("ship_detour").unwrap());
        let r = click(&mut f, |f| f.button_at(ALL_OFF_BUTTON).unwrap());
        assert_eq!(r.changes, [Change::All(false)]);
        assert_eq!(r.commands.len(), Fix::ALL.len() - 1, "the fixes that were on");
        assert_eq!((f.fixes(), f.view()), (Fixes::ORIGINAL, Enhancements::ORIGINAL));
        let r = click(&mut f, |f| f.button_at(ALL_ON_BUTTON).unwrap());
        assert_eq!(r.changes, [Change::All(true)]);
        assert_eq!(r.commands.len(), Fix::ALL.len());
        assert_eq!(f.view(), Enhancements::all_on());
    }

    #[test]
    fn buttons_and_keys() {
        let mut f = form();
        assert_eq!(
            click(&mut f, |f| f.button_at(CREDITS_BUTTON).unwrap()).action,
            Some(Action::Credits)
        );
        assert!(f.visible);
        assert_eq!(f.mouse_down(10, 10, ShiftState::RIGHT), Response::default());
        assert_eq!(
            click(&mut f, |f| f.button_at(SCHLIESSEN_BUTTON).unwrap()).action,
            Some(Action::Close)
        );
        let mut f = form();
        assert_eq!(f.key_down(VK_ESCAPE, ShiftState::default()).action, Some(Action::Close));
        let mut f = form();
        assert_eq!(f.key_down(b'A'.into(), ShiftState::default()).action, None);
        assert_eq!(f.key_down(VK_F4, ShiftState::ALT).action, Some(Action::Close));
    }
}
