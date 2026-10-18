//! `TAuktionsform`, the auction form (segment `1028`): the view's part.
//! The world runs the auction ([`crate::auction`]); this draws the form
//! from what the world's ticks and commands report
//! ([`crate::auction::Change`]) and turns the player's clicks and keys
//! into [`Action`]s, which the caller hands the world as commands.
//!
//! The form is three `TImage`s the game draws into itself, one
//! [`Surface`] each here, which the caller puts into the VCL form at the
//! bounds [`LAYOUT`] gives: `leiste` (the title bar, with the close
//! button and the bar to drag the form by), `AuktionsImage` (the
//! auctioneer, the item's picture and the bidders' paddles) and
//! `knopfimage` (the price and the two buttons).

// The original's names: `x`, `y`, sizes `w`, `h`, the colour `c`.
#![allow(clippy::many_single_char_names)]

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::auction::{Action, Change, SOLD};
use crate::enhance::{Enhancement, Enhancements};
use crate::hauptschirm::InfoTexts;
use crate::money::format_money;
use crate::text::{Align, Font};
use crate::widgets::ShiftState;
use crate::world::World;

/// The form's size: `ClientWidth` and `ClientHeight` as 1028:0dde sets
/// them.
pub const WIDTH: i32 = 340;
pub const HEIGHT: i32 = 316;
/// `AuktionsImage`'s height, by which 1028:0dde centres the form.
const IMAGE_HEIGHT: i32 = 235;
/// The form's height while it's dragged by its title bar, and after a
/// double click on it. // 1028:1b77
const TITLE_HEIGHT: i32 = 16;

/// The images' bounds in the form. // 1028:0dde
pub const LAYOUT: [(&str, Rect); 3] = [
    ("leiste", Rect::new(0, 0, 340, 16)),
    ("AuktionsImage", Rect::new(0, 16, 340, 235)),
    ("knopfimage", Rect::new(0, 251, 340, 65)),
];

/// The bitmaps the form is drawn from, from `GRAFIK`, in the order
/// [`Auktionsform::new`] takes them.
pub const BITMAP_FILES: [&str; 7] = [
    "auktbild.BMP", // AuktionsImage's background, 1028:0d9b
    "auktleis.BMP", // the title bar, 1028:0da8
    "auktfens.BMP", // knopfimage's background, 1028:0db5
    "auktanim.BMP", // the auctioneer's frames, 1028:0dc2
    "aukthand.BMP", // the paddles, 1028:0dd1
    "auktknop.BMP", // a button held down, 1028:1e14
    "closepre.bmp", // the close button held down, 1110:54bc
];
const BILD: usize = 0;
const LEISTE: usize = 1;
const FENSTER: usize = 2;
const ANIM: usize = 3;
const HAND: usize = 4;
const KNOPF: usize = 5;
const CLOSEPRE: usize = 6;

/// The rectangles 1028:01d4 tests a click against (inclusive), from the
/// table at `1110:1654`: the bid and pass buttons in `knopfimage`, the
/// bar to drag by and the close button in `leiste`.
const BID_BUTTON: (i32, i32, i32, i32) = (197, 7, 300, 33); // 1110:166c
const PASS_BUTTON: (i32, i32, i32, i32) = (197, 35, 300, 61); // 1110:1674
const DRAG_BAR: (i32, i32, i32, i32) = (16, 0, 340, 16); // 1110:167c
const CLOSE_BUTTON: (i32, i32, i32, i32) = (0, 0, 16, 16); // 1110:1684

/// The title's width: the drag bar's (`1110:1680 - 1110:167c`).
const TITLE_WIDTH: i32 = 340 - 16;

/// Where the item's picture goes in `AuktionsImage`, 125 × 125.
/// // 1028:0dde
const PICTURE: (i32, i32) = (54, 42);

/// The auctioneer's parts in `auktanim.BMP` and `AuktionsImage`: where
/// they go, their size, and the row and step of their frames.
/// // 1028:0221
const MOUTH: ((i32, i32), (i32, i32), i32, i32) = ((240, 77), (20, 18), 0, 20);
const EYES: ((i32, i32), (i32, i32), i32, i32) = ((237, 53), (24, 8), 18, 24);
const BROWS: ((i32, i32), (i32, i32), i32, i32) = ((234, 37), (32, 15), 26, 32);
/// The hammer's seven frames and the hand's four, in `auktanim.BMP`.
/// // 1110:168c, 1110:16a8
const HAMMER_FRAMES: [(i32, i32); 7] = [(0, 41), (54, 41), (108, 41), (0, 75), (54, 75), (108, 75), (0, 109)];
const HAMMER: ((i32, i32), (i32, i32)) = ((194, 124), (54, 34));
const HAND_FRAMES: [(i32, i32); 4] = [(54, 109), (106, 109), (0, 143), (52, 143)];
const HAND_AT: ((i32, i32), (i32, i32)) = ((282, 149), (52, 30));

/// A bid's paddle: 24 × 68, the mask at the left of `aukthand.BMP` and
/// each colour's 24 further on, going up from the bottom of
/// `AuktionsImage` to row 167 and down again, 4 rows a frame, at least
/// 25 ms each. // 1028:15ef
const PADDLE: (i32, i32) = (24, 68);
const PADDLE_TOP: i32 = 167;
const PADDLE_FRAME_MS: u32 = 25;
/// 17 frames up from row 235, 18 down from 167.
const PADDLE_UP: i32 = 17;
const PADDLE_FRAMES: i32 = 35;

/// The keys `FormKeyDown` knows. // en 1028:3620
#[cfg(test)]
const VK_RETURN: u16 = 0x0d;
const VK_ESCAPE: u16 = 0x1b;
const VK_SPACE: u16 = 0x20;
const VK_F4: u16 = 0x73;

/// What the player holds down on the form. // 1110:6599
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pressed {
    None,
    Bid,
    Pass,
    Drag,
    Close,
}

fn hit((l, t, r, b): (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    (l..=r).contains(&x) && (t..=b).contains(&y)
}

fn blank(name: &str) -> Surface {
    let r = LAYOUT
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(Rect::default(), |(_, r)| *r);
    Surface::new(
        usize::try_from(r.width()).unwrap_or(0),
        usize::try_from(r.height()).unwrap_or(0),
        0,
    )
}

/// Copies `src`'s top left over all of `dst`.
fn copy(dst: &mut Surface, src: &Surface) {
    let all = dst.bounds();
    dst.blit(0, 0, src, all, Rop::Copy, all);
}

/// An amount as the form shows it: the money, a space, the currency.
/// // 1030:11a0, 1028:0dcf
fn money(amount: i32, info: &InfoTexts) -> Vec<u8> {
    let mut text = format_money(amount, info.currency_factor, info.land);
    text.push(b' ');
    text.extend_from_slice(&info.currency);
    text.truncate(255);
    text
}

/// The auction form's state and images.
#[derive(Debug, Clone)]
pub struct Auktionsform {
    bitmaps: Vec<Surface>,
    /// `AUKTION.TXT`'s twelve texts. // *1110:65b0
    texts: Vec<Vec<u8>>,
    pub leiste: Surface,
    pub image: Surface,
    pub knopf: Surface,
    /// The form is shown.
    pub visible: bool,
    /// The form's top left on the screen, and its height.
    pub left: i32,
    pub top: i32,
    pub height: i32,
    pressed: Pressed,
    /// The title bar was double-clicked: the form stays rolled up to it.
    /// // 1110:659a
    rolled_up: bool,
    /// Where in the form the drag took hold. // 1110:6046
    grip: (i32, i32),
    /// A bid's paddle going up and down: its colour and how far it is.
    paddle: Option<(u16, u32)>,
    /// The player's view enhancements (`auction_title`,
    /// `form_positions`).
    pub enhance: Enhancements,
}

impl Auktionsform {
    /// The form, hidden; `bitmaps` are those of [`BITMAP_FILES`], `texts`
    /// `AUKTION.TXT`'s (`formats::data::auktion`).
    ///
    /// # Panics
    /// If there aren't as many bitmaps as files.
    #[must_use]
    pub fn new(bitmaps: Vec<Surface>, texts: Vec<Vec<u8>>) -> Self {
        assert_eq!(bitmaps.len(), BITMAP_FILES.len(), "one bitmap per file");
        Auktionsform {
            bitmaps,
            texts,
            leiste: blank("leiste"),
            image: blank("AuktionsImage"),
            knopf: blank("knopfimage"),
            visible: false,
            left: 0,
            top: 0,
            height: HEIGHT,
            pressed: Pressed::None,
            rolled_up: false,
            grip: (0, 0),
            paddle: None,
            enhance: Enhancements::ORIGINAL,
        }
    }

    fn text(&self, n: usize) -> &[u8] {
        self.texts.get(n).map_or(&[], |t| t)
    }

    /// Draws `AUKTION.TXT`'s text `n` on `knopfimage` at `x`, `y`, centred in
    /// `width`, in colour 10.
    fn knopf_text(&mut self, font: &Font, n: usize, (x, y): (i32, i32), width: i32) {
        let text = self.text(n).to_vec();
        font.draw(&mut self.knopf, &text, (x, y), width, Align::Center, 10);
    }

    /// The title bar, with the title one row lower when the form opens
    /// (`row` 1) than after a click on the bar (0). // 1028:0dde, 1028:1ce0
    fn draw_title(&mut self, font: &Font, row: i32) {
        copy(&mut self.leiste, &self.bitmaps[LEISTE]);
        let title = self.text(10).to_vec();
        font.draw(&mut self.leiste, &title, (0, row), TITLE_WIDTH, Align::Center, 10);
    }

    /// The pass button's two lines.
    fn draw_pass(&mut self, font: &Font) {
        self.knopf_text(font, 0, (200, 35), 100);
        self.knopf_text(font, 1, (200, 46), 100);
    }

    /// The bid button: the next bid and "Mehr bieten", or "Auktion
    /// starten" while the auction waits for the player.
    fn draw_bid(&mut self, world: &World, font: &Font, info: &InfoTexts, bidding: bool) {
        if bidding {
            let next = money(world.auction.price / 20, info);
            font.draw(&mut self.knopf, &next, (200, 6), 100, Align::Center, 10);
            self.knopf_text(font, 2, (200, 17), 100);
        } else {
            self.knopf_text(font, 0, (200, 6), 100);
            self.knopf_text(font, 11, (200, 17), 100);
        }
    }

    /// The start price and the buttons. // 1028:0dde, 1028:2208
    fn draw_start(&mut self, world: &World, font: &Font, info: &InfoTexts, bidding: bool) {
        copy(&mut self.knopf, &self.bitmaps[FENSTER]);
        self.knopf_text(font, 8, (46, 8), 128);
        let price = money(world.auction.price, info);
        font.draw(&mut self.knopf, &price, (46, 22), 128, Align::Center, 10);
        self.draw_bid(world, font, info, bidding);
        self.draw_pass(font);
    }

    /// Opens the form as 1028:0dde does: centred by `AuktionsImage`'s
    /// height on a `screen` of that size, the title, the background with
    /// the item's `picture` (`FILENAME.DAT`'s for the item, 125 × 125), the
    /// start price and the buttons.
    pub fn open(
        &mut self,
        world: &World,
        font: &Font,
        info: &InfoTexts,
        picture: Option<&Surface>,
        screen: (i32, i32),
    ) {
        // BUG(original): centred by `AuktionsImage`'s height, not the form's, so it sits 40 pixels low.
        let height = if self.enhance.has(Enhancement::FormPositions) {
            HEIGHT
        } else {
            IMAGE_HEIGHT
        };
        self.top = (screen.1 - height) / 2;
        self.left = (screen.0 - WIDTH) / 2;
        self.height = HEIGHT;
        copy(&mut self.image, &self.bitmaps[BILD]);
        self.draw_title(font, 1);
        if let Some(p) = picture {
            let all = self.image.bounds();
            self.image
                .blit(PICTURE.0, PICTURE.1, p, Rect::new(0, 0, 125, 125), Rop::Copy, all);
        }
        self.draw_start(world, font, info, !world.options.auction_waits);
        self.pressed = Pressed::None;
        self.paddle = None;
        self.visible = true;
    }

    /// The price, the bidder and the calls, unless the item is sold.
    /// // 1028:05a3
    fn draw_price(&mut self, world: &World, font: &Font, info: &InfoTexts) {
        let a = &world.auction;
        if a.sold.is_some() {
            return;
        }
        copy(&mut self.knopf, &self.bitmaps[FENSTER]);
        self.draw_pass(font);
        let bidding = !world.options.auction_waits || a.joined;
        self.draw_bid(world, font, info, bidding);
        if !bidding {
            return;
        }
        self.knopf_text(font, 3, (46, 8), 128);
        let price = money(a.price, info);
        font.draw(&mut self.knopf, &price, (46, 20), 128, Align::Center, 10);
        let mut by = self.text(4).to_vec();
        by.push(b' '); // 1028:05a1
        by.extend_from_slice(Self::name(world, a.bidder));
        by.truncate(255);
        font.draw(&mut self.knopf, &by, (46, 32), 128, Align::Center, 10);
        if (1..=3).contains(&a.going) {
            self.knopf_text(font, 4 + usize::from(a.going), (46, 44), 128);
        }
    }

    /// Player `p`'s name, `1110:0902 + 256 p`.
    fn name(world: &World, p: i16) -> &[u8] {
        usize::try_from(p)
            .ok()
            .and_then(|p| world.player_names.get(p))
            .map_or(&[], |n| n)
    }

    /// "*name* erhielt den Zuschlag" on the bare background. // 1028:1973
    fn draw_sold(&mut self, world: &World, font: &Font) {
        copy(&mut self.knopf, &self.bitmaps[FENSTER]);
        let mut text = Self::name(world, world.auction.bidder).to_vec();
        text.push(b' '); // 1028:1971
        text.extend_from_slice(self.text(9));
        text.truncate(255);
        font.draw(&mut self.knopf, &text, (46, 10), 128, Align::Center, 10);
    }

    /// Shows what the world did to the auction.
    pub fn update(&mut self, world: &World, changes: &[Change], font: &Font, info: &InfoTexts) {
        for change in changes {
            match *change {
                Change::Opened => {}
                // 1028:2208, 1028:26e4
                Change::Joined => self.draw_start(world, font, info, true),
                Change::Bid(_) => {
                    self.draw_price(world, font, info);
                    self.paddle = Some((world.auction.colour, 0));
                    self.draw_paddle();
                }
                Change::Going(SOLD) => self.draw_sold(world, font),
                Change::Going(_) => self.draw_price(world, font, info),
                Change::Closed => self.visible = false, // 1028:0476
            }
        }
    }

    /// The auctioneer as the world's last step left him. // 1028:0221
    pub fn draw_auctioneer(&mut self, world: &World) {
        let f = &world.auction.frames;
        let anim = &self.bitmaps[ANIM];
        let all = self.image.bounds();
        for ((x, y), (w, h), row, step, frame) in [
            (MOUTH.0, MOUTH.1, MOUTH.2, MOUTH.3, f.mouth),
            (EYES.0, EYES.1, EYES.2, EYES.3, f.eyes),
            (BROWS.0, BROWS.1, BROWS.2, BROWS.3, f.brows),
        ] {
            let from = Rect::new(i32::from(frame) * step, row, w, h);
            self.image.blit(x, y, anim, from, Rop::Copy, all);
        }
        for (((x, y), (w, h)), frames, frame) in [
            (HAMMER, &HAMMER_FRAMES[..], f.hammer),
            (HAND_AT, &HAND_FRAMES[..], f.hand),
        ] {
            let (sx, sy) = frames.get(usize::from(frame)).copied().unwrap_or(frames[0]);
            self.image.blit(x, y, anim, Rect::new(sx, sy, w, h), Rop::Copy, all);
        }
    }

    /// The paddle's frame for how far it is: the background under it put
    /// back, then the mask and the colour. The background is the bare
    /// one: rows 167 to 234, below the item's picture (rows 42 to 166),
    /// but the bottom of the auctioneer's waving hand (282, 149) for the
    /// paddles of colours 7 and 8. // 1028:15ef
    fn draw_paddle(&mut self) {
        let Some((colour, ms)) = self.paddle else {
            return;
        };
        let frame = i32::try_from(ms / PADDLE_FRAME_MS)
            .unwrap_or(i32::MAX)
            .min(PADDLE_FRAMES - 1);
        let y = if frame < PADDLE_UP {
            IMAGE_HEIGHT - 4 * frame
        } else {
            PADDLE_TOP + 4 * (frame - PADDLE_UP)
        };
        let c = i32::from(colour);
        let x = c * 40 - 20;
        let all = self.image.bounds();
        let (w, h) = PADDLE;
        // BUG(original): the bare background goes back under the paddle, wiping the bottom rows of the auctioneer's waving hand for colours 7 and 8.
        let under = Rect::new(x, PADDLE_TOP, w, h);
        self.image
            .blit(x, PADDLE_TOP, &self.bitmaps[BILD], under, Rop::Copy, all);
        let hand = &self.bitmaps[HAND];
        self.image.blit(x, y, hand, Rect::new(0, 0, w, h), Rop::And, all);
        self.image.blit(x, y, hand, Rect::new(c * w, 0, w, h), Rop::Invert, all);
    }

    /// Moves a bid's paddle on by `ms`; at the end the price is drawn
    /// again. Says whether the form changed.
    pub fn advance(&mut self, ms: u32, world: &World, font: &Font, info: &InfoTexts) -> bool {
        let Some((colour, done)) = self.paddle else {
            return false;
        };
        let done = done.saturating_add(ms);
        self.paddle = Some((colour, done));
        self.draw_paddle();
        if done >= PADDLE_FRAME_MS * PADDLE_FRAMES.unsigned_abs() {
            self.paddle = None;
            self.draw_price(world, font, info);
        }
        true
    }

    /// Whether `x`, `y` (in the form) is on the form as it's shown.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.visible && (0..WIDTH).contains(&x) && (0..self.height).contains(&y)
    }

    /// The mouse goes down at `x`, `y` in the form, with `shift`.
    /// // 1028:1b77, 1028:1e23
    pub fn mouse_down(
        &mut self,
        x: i32,
        y: i32,
        shift: ShiftState,
        world: &World,
        font: &Font,
        info: &InfoTexts,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        if y < TITLE_HEIGHT {
            if hit(CLOSE_BUTTON, x, y) {
                self.pressed = Pressed::Close;
                let all = self.leiste.bounds();
                self.leiste
                    .blit(0, 0, &self.bitmaps[CLOSEPRE], Rect::new(0, 0, 16, 16), Rop::Copy, all);
                actions.extend([Action::Hold(true), Action::Pass]); // 1028:0c82
            }
            // Column 16 is on both.
            if hit(DRAG_BAR, x, y) {
                self.pressed = Pressed::Drag;
                self.grip = (x, y);
                self.height = TITLE_HEIGHT;
                self.rolled_up = shift.contains(ShiftState::DOUBLE);
                if !actions.contains(&Action::Hold(true)) {
                    actions.push(Action::Hold(true));
                }
            }
            return actions;
        }
        let (kx, ky) = (x, y - LAYOUT[2].1.top);
        let a = &world.auction;
        if ky < 0 || a.sold.is_some() {
            return actions;
        }
        let bidding = !world.options.auction_waits || a.joined;
        if hit(BID_BUTTON, kx, ky) {
            // The original's form is player 1's.
            if bidding && a.price.wrapping_add(a.price / 20) > a.limits[0] {
                // TODO: the error (FEHLER.TXT 0xe, 1030:232d) on the status
                // bar, which isn't ported.
            } else {
                self.pressed = Pressed::Bid;
                self.press(BID_BUTTON);
                if bidding {
                    let next = money(a.price / 20, info);
                    font.draw(&mut self.knopf, &next, (201, 6), 100, Align::Center, 10);
                    self.knopf_text(font, 2, (201, 18), 100);
                } else {
                    self.knopf_text(font, 0, (201, 6), 100);
                    self.knopf_text(font, 11, (201, 18), 100);
                }
                actions.push(Action::Hold(true));
            }
        }
        if hit(PASS_BUTTON, kx, ky) {
            self.pressed = Pressed::Pass;
            self.press(PASS_BUTTON);
            self.knopf_text(font, 0, (201, 35), 100);
            self.knopf_text(font, 1, (201, 46), 100);
            actions.extend([Action::Hold(true), Action::Pass]); // 1028:0c82
        }
        actions
    }

    /// A button drawn held down, `auktknop.BMP` over it.
    fn press(&mut self, (l, t, _, _): (i32, i32, i32, i32)) {
        let all = self.knopf.bounds();
        self.knopf
            .blit(l, t, &self.bitmaps[KNOPF], Rect::new(0, 0, 102, 26), Rop::Copy, all);
    }

    /// The mouse moves to `x`, `y` on a `screen` of that size: with the
    /// left button down on the title bar, the form follows, kept on the
    /// screen. // 1028:1c4d
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> bool {
        if self.pressed != Pressed::Drag || !shift.contains(ShiftState::LEFT) {
            return false;
        }
        let (sx, sy) = (x + self.left, y + self.top);
        self.left = (sx - self.grip.0).max(0).min(screen.0 - WIDTH);
        self.top = (sy - self.grip.1).max(0).min(screen.1 - self.height);
        true
    }

    /// The mouse comes up at `x`, `y` in the form (the control that took
    /// the mouse down still has it). // 1028:1ce0, 1028:2208
    pub fn mouse_up(&mut self, x: i32, y: i32, world: &World, font: &Font) -> Vec<Action> {
        let mut actions = Vec::new();
        let pressed = self.pressed;
        if matches!(pressed, Pressed::Drag | Pressed::Close) {
            if pressed == Pressed::Drag && !self.rolled_up {
                self.height = HEIGHT;
            }
            if hit(CLOSE_BUTTON, x, y) && pressed == Pressed::Close && world.auction.open {
                actions.push(Action::Close); // 1028:0476
            }
            // BUG(original): the title goes back one row higher than the form opened with.
            let row = i32::from(self.enhance.has(Enhancement::AuctionTitle));
            self.draw_title(font, row);
        } else {
            let (kx, ky) = (x, y - LAYOUT[2].1.top);
            if hit(BID_BUTTON, kx, ky) && pressed == Pressed::Bid {
                actions.push(Action::Bid);
            }
            if hit(PASS_BUTTON, kx, ky) && pressed == Pressed::Pass && world.auction.open {
                actions.push(Action::Close); // 1028:0476
            }
        }
        if pressed != Pressed::None {
            actions.push(Action::Hold(false));
        }
        self.pressed = Pressed::None;
        actions
    }

    /// `FormKeyDown`: Escape, Space and Alt+F4 leave the auction.
    /// v1.0's Enter bid (or started a waiting auction) without the money
    /// check and after the third call too (1028:26e4); the later build
    /// took it out. // en 1028:3620
    #[must_use]
    pub fn key_down(&self, key: u16, shift: ShiftState) -> Option<Action> {
        if shift.0 == 0 || shift == ShiftState::SHIFT {
            match key {
                VK_ESCAPE | VK_SPACE => Some(Action::Leave),
                _ => None,
            }
        } else if shift == ShiftState::ALT && key == VK_F4 {
            Some(Action::Leave)
        } else {
            None
        }
    }

    /// The form's bounds on the screen.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.left, self.top, WIDTH, self.height)
    }

    /// Each image with the name of the `TImage` it goes into.
    #[must_use]
    pub fn images(&self) -> [(&'static str, &Surface); 3] {
        [
            ("leiste", &self.leiste),
            ("AuktionsImage", &self.image),
            ("knopfimage", &self.knopf),
        ]
    }
}

/// `FILENAME.DAT`'s name for a message's picture as a file in `GRAFIK`:
/// the names are padded with spaces before the extension, which DOS
/// ignores.
#[must_use]
pub fn picture_file(name: &[u8]) -> String {
    let name: Vec<u8> = name.iter().copied().filter(|&c| c != b' ').collect();
    String::from_utf8_lossy(&name).to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_names() {
        assert_eq!(picture_file(b"surfwm  .bmp"), "SURFWM.BMP");
    }

    #[test]
    fn keys() {
        let form = Auktionsform::new(vec![Surface::new(1, 1, 0); BITMAP_FILES.len()], Vec::new());
        assert_eq!(form.key_down(VK_ESCAPE, ShiftState::SHIFT), Some(Action::Leave));
        assert_eq!(form.key_down(VK_RETURN, ShiftState::default()), None);
        assert_eq!(form.key_down(VK_F4, ShiftState::ALT), Some(Action::Leave));
        assert_eq!(form.key_down(VK_RETURN, ShiftState::CTRL), None);
    }
}
