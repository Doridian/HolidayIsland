//! `TStartform`, the start menu (segment `1058`). `hintergrund_timer`
//! creates it in state 500 once the intro is over (`docs/startup.md`).
//!
//! The form is one `TImage`, `Bild`, with about 45 widgets of the game's own
//! toolkit drawn into it: the settings for a new island (opponents, relief,
//! land, rivers, island shape), the player's colour, name and difficulty,
//! and the buttons. A second page swaps the background for `startsze.bmp`
//! and shows the six scenarios instead of the island settings.
//!
//! The original writes the settings straight into globals as the player
//! clicks; here they're a [`Settings`] the form owns. Clicks that leave the
//! menu come back as an [`Action`].

use vcl::surface::Surface;

use crate::enhance::{Enhancement, Enhancements};
use crate::text::Font;
use crate::widgets::{
    BitmapId, Button, CheckBox, Edit, Frame, Label, Outcome, RadioButton, ScrollBar, ScrollBitmaps, ShiftState, Skin,
    Sound, Widget, align,
};

/// The form's size. The DFM says 424×352, but `startform_init` sets the form
/// and `Bild` to this. // 1058:27e6
pub const WIDTH: i32 = 500;
pub const HEIGHT: i32 = 334;

/// The bitmaps the form draws with, from `GRAFIK`, in the order
/// [`StartForm::new`] takes them.
pub const BITMAP_FILES: [&str; 30] = [
    "start1.bmp",   // +0x3460, the background
    "scrli.bmp",    // +0x180
    "scrre.bmp",    // +0x184
    "scrlipr.bmp",  // +0x188
    "scrrepr.bmp",  // +0x18c
    "scrolli.bmp",  // +0x190
    "scrpanel.bmp", // +0x194
    "stkn.bmp",     // +0x198
    "stknpr.bmp",   // +0x19c
    "strakn.bmp",   // +0x1a0
    "straknpr.bmp", // +0x1a4
    "stgelb.bmp",   // +0x1a8
    "storan.bmp",   // +0x1ac
    "strot.bmp",    // +0x1b0
    "stlila.bmp",   // +0x1b4
    "strosa.bmp",   // +0x1b8
    "stcyan.bmp",   // +0x1bc
    "stblau.bmp",   // +0x1c0
    "stgrue.bmp",   // +0x1c4
    "stgelbpr.bmp", // +0x1c8
    "storanpr.bmp", // +0x1cc
    "strotpr.bmp",  // +0x1d0
    "stlilapr.bmp", // +0x1d4
    "strosapr.bmp", // +0x1d8
    "stcyanpr.bmp", // +0x1dc
    "stblaupr.bmp", // +0x1e0
    "stgruepr.bmp", // +0x1e4
    // The scenario page's background, loaded into +0x3460 by the mode
    // button. // 1058:02cf
    "startsze.bmp",
    // The close button's, which belong to `hintergrund` (state 100 loads
    // them for the file menu). // 1110:54b8 1110:54bc
    "close.bmp",
    "closepre.bmp",
];

const START1: BitmapId = BitmapId(0);
const SCROLL: ScrollBitmaps = ScrollBitmaps {
    back: BitmapId(1),
    forward: BitmapId(2),
    back_pressed: BitmapId(3),
    forward_pressed: BitmapId(4),
    thumb: BitmapId(5),
    track: BitmapId(6),
};
const BUTTON: BitmapId = BitmapId(7);
const BUTTON_PRESSED: BitmapId = BitmapId(8);
const CHECK: BitmapId = BitmapId(9);
const CHECKED: BitmapId = BitmapId(10);
/// The eight colours' boxes, yellow, orange, red, purple, pink, cyan, blue
/// and green, and then the same selected.
const COLOUR: usize = 11;
const COLOUR_SELECTED: usize = 19;
const STARTSZE: BitmapId = BitmapId(27);
const CLOSE: BitmapId = BitmapId(28);
const CLOSE_PRESSED: BitmapId = BitmapId(29);

/// The credits under "Ein Spiel von:", which the form never shows.
/// // 1058:27bb 1058:27c7 1058:27d7
const CREDITS: [&[u8]; 3] = [b"Moritz Baer", b"Niels Kokkelink", b"Rainer Michael"];

/// `VK_F4`
const VK_F4: u16 = 0x73;

/// What the start menu decides about the next game. In the original these
/// are globals, which the menu sets as the player clicks and the game reads
/// when it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The island's shape: 0 atoll, 1 a group of islands, 2 one island.
    /// // 1110:008a
    pub island: i16,
    /// Relief, 0 (flat) to 4 (hilly). // 1110:008e
    pub relief: i16,
    /// How much land, 0 to 4. // 1110:0090
    pub land: i16,
    /// How many rivers, 0 to 10. // 1110:0092
    pub rivers: i16,
    /// Difficulty, 0 (easy) to 2 (hard). // 1110:008c
    pub difficulty: i16,
    /// Computer opponents, 0 to 7. // 1110:1478
    pub opponents: u8,
    /// Players: the opponents and the human. // 1110:1479
    pub players: u8,
    /// Each player's colour, 1 to 8; the human is the first. Picking a
    /// colour swaps it with whoever had it. // 1110:0074
    pub colours: [u8; 8],
    /// The scenario, 1 to 6, or 0 for a normal game. // 1110:006e
    pub scenario: u8,
    /// The player's name: the first entry of the player names and the one
    /// the menu offers next time. // 1110:0102 1110:0a02
    pub name: Vec<u8>,
}

impl Default for Settings {
    /// The values `init_game_defaults` and the initialised data give them.
    /// // 1000:1ea0
    fn default() -> Self {
        Settings {
            island: 1,
            relief: 2,
            land: 2,
            rivers: 5,
            difficulty: 0,
            opponents: 7,
            players: 8,
            colours: [1, 2, 3, 4, 5, 6, 7, 8],
            scenario: 0,
            name: b"Holiday".to_vec(),
        }
    }
}

impl Settings {
    /// The globals `startform_init` reads when "Neues Spiel" goes back to
    /// the start menu (1010:25dc), as `world` left them: a loaded game's
    /// file has put its own there. The name is player 1's. // 1058:27e6
    #[must_use]
    pub fn from_world(world: &crate::world::World) -> Self {
        let [island, relief, land, rivers] = world.island_settings;
        Settings {
            island,
            relief,
            land,
            rivers,
            difficulty: world.difficulty,
            opponents: world.player_count.saturating_sub(1),
            players: world.player_count,
            colours: world.player_colours,
            scenario: world.scenario,
            name: world.player_names.get(1).cloned().unwrap_or_default(),
        }
    }
}

/// A click that leaves the start menu. The port doesn't do these yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// "Start" (`startform_start_click`): stores the name, hides the form
    /// and sets `hintergrund`'s `Timer1.Tag` to 1000, with
    /// `hintergrund+0x190` cleared and `+0x191` set to `scenario`, which
    /// starts the game (states 1000–1500). // 1058:021a
    Start { scenario: bool },
    /// "Altes Spiel fortsetzen" (`startform_load_click`): disables the form
    /// and opens `TLadenForm` to load a saved game. // 1058:0c7d
    Load,
    /// The close button (`startform_close_click`) or Alt+F4
    /// (`startform_formkeydown`): `game_done` and `Application.Terminate`.
    /// // 1058:0cbb 1058:4381
    Quit,
}

/// What an event did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// Something changed and the handler refreshes `Bild` (`10e8:22c6`).
    pub refresh: bool,
    /// The sound the widget plays (`play_sound`, 10a0:30b6).
    pub sound: Option<Sound>,
    pub action: Option<Action>,
}

/// The texts from `START.TXT` and `SZENARIO.TXT` that aren't in a widget.
#[derive(Debug, Clone)]
struct Texts {
    /// The mode button's caption on the normal page: "Szenarien". // 1110:66ce
    scenarios: Vec<u8>,
    /// Its caption on the scenario page: "Standardspiel". // 1110:67ce
    normal_game: Vec<u8>,
    /// The scenarios' descriptions. // 1110:68ce..1110:6dce
    descriptions: [Vec<u8>; 6],
}

/// The widgets, with their offsets in the form.
#[derive(Debug, Clone)]
struct Widgets {
    relief: ScrollBar,      // +0x1e8
    land: ScrollBar,        // +0x226
    opponents: ScrollBar,   // +0x264
    rivers: ScrollBar,      // +0x2a2
    difficulty: ScrollBar,  // +0x3577
    start: Button,          // +0x2e0
    mode: Button,           // +0x3f6
    load: Button,           // +0x50c
    close: Button,          // +0x622
    version: Button,        // +0x334a, never shown
    title: Label,           // +0x738
    credits_heading: Label, // +0x847
    credits: [Label; 3],    // +0x956, +0xa65, +0xb74, never shown
    // +0xc83 is a label with no text that's never shown either (at 0, 0x68,
    // 0x1a9 wide); the port leaves it out.
    opponents_heading: Label,  // +0xd92
    opponents_count: Label,    // +0xea1
    relief_heading: Label,     // +0xfb0
    relief_low: Label,         // +0x10bf
    relief_high: Label,        // +0x11ce
    land_heading: Label,       // +0x12dd
    land_low: Label,           // +0x13ec
    land_high: Label,          // +0x14fb
    rivers_heading: Label,     // +0x160a
    rivers_low: Label,         // +0x1719
    rivers_high: Label,        // +0x1828
    island_heading: Label,     // +0x1937
    colour_heading: Label,     // +0x1a46
    name_heading: Label,       // +0x1b55
    difficulty_heading: Label, // +0x1c64
    difficulty_low: Label,     // +0x1d73
    difficulty_high: Label,    // +0x1e82
    /// Check boxes that act as radio buttons, by [`Settings::island`]:
    /// atoll (+0x1f91), group (+0x21c5) and one island (+0x20ab).
    islands: [CheckBox; 3],
    /// +0x22df, 0x118 apart.
    colours: [RadioButton; 8],
    /// The scenario's description, +0x2b9f.
    description: Label,
    /// Check boxes that act as radio buttons, +0x2cae, 0x11a apart.
    scenarios: [CheckBox; 6],
    name: Edit, // +0x3464
}

/// The widgets with a `Click` or that take the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Relief,
    Land,
    Opponents,
    Rivers,
    Difficulty,
    Start,
    Mode,
    Load,
    Close,
    Version,
    Island(usize),
    Colour(usize),
    Scenario(usize),
    Name,
}

/// The scroll bars in the order the mouse handlers try them.
const SCROLL_DOWN_ORDER: [Which; 5] = [
    Which::Land,
    Which::Relief,
    Which::Opponents,
    Which::Difficulty,
    Which::Rivers,
];
const SCROLL_MOVE_ORDER: [Which; 5] = [
    Which::Land,
    Which::Relief,
    Which::Opponents,
    Which::Rivers,
    Which::Difficulty,
];
const BUTTON_ORDER: [Which; 5] = [Which::Start, Which::Version, Which::Mode, Which::Load, Which::Close];
/// The island check boxes in the order `startform_bildmousedown` tries
/// them: one island, atoll, group.
const ISLAND_ORDER: [usize; 3] = [2, 0, 1];

/// The images the widgets draw with.
#[derive(Debug, Clone)]
struct Assets {
    font: Font,
    bitmaps: Vec<Surface>,
    /// What `+0x3460` holds: `start1.bmp`, or `startsze.bmp` on the
    /// scenario page. Widgets erase themselves from it.
    background: BitmapId,
    /// Stands in for a missing bitmap.
    empty: Surface,
}

impl Assets {
    fn skin(&self) -> Skin<'_> {
        Skin {
            font: &self.font,
            bitmaps: &self.bitmaps,
        }
    }

    fn background(&self) -> &Surface {
        self.bitmaps.get(self.background.0).unwrap_or(&self.empty)
    }
}

/// The start menu.
#[derive(Debug, Clone)]
pub struct StartForm {
    assets: Assets,
    /// `Bild`'s picture.
    bild: Surface,
    w: Widgets,
    texts: Texts,
    /// On the scenario page. // 1110:6ece
    scenario_mode: bool,
    settings: Settings,
    /// The player's view enhancements (`edit_field`, `scroll_bars`,
    /// `mouse_buttons`, `start_menu_scenario`).
    pub enhance: Enhancements,
}

fn label(left: i32, top: i32, width: i32, height: i32, colour: i32, alignment: i32, text: &[u8]) -> Label {
    Label::new(Frame::new(left, top, width, height), text, align(alignment), colour)
}

/// A check box that the start menu uses as a radio button.
fn check(left: i32, top: i32, width: i32, text: &[u8]) -> CheckBox {
    CheckBox::new(label(left, top, width, 0x10, 9, 0, text), CHECK, CHECKED, 10)
}

fn scroll(top: i32) -> ScrollBar {
    ScrollBar::new(Frame::new(0x142, top, 0x60, 0x10), SCROLL)
}

fn button(left: i32, top: i32) -> Button {
    Button::new(Frame::new(left, top, 0x90, 0x16), BUTTON, BUTTON_PRESSED, b"", 9)
}

/// `Str(n)`. // 1100:0764
fn decimal(n: u8) -> Vec<u8> {
    n.to_string().into_bytes()
}

/// A `Longint` stored into an `Integer`: the low word.
#[allow(clippy::cast_possible_truncation)]
fn low_word(n: i32) -> i16 {
    n as i16
}

/// A `Longint` stored into a `Byte`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn low_byte(n: i32) -> u8 {
    n as u8
}

impl Widgets {
    fn scroll_mut(&mut self, which: Which) -> Option<&mut ScrollBar> {
        Some(match which {
            Which::Relief => &mut self.relief,
            Which::Land => &mut self.land,
            Which::Opponents => &mut self.opponents,
            Which::Rivers => &mut self.rivers,
            Which::Difficulty => &mut self.difficulty,
            _ => return None,
        })
    }

    fn button_mut(&mut self, which: Which) -> Option<&mut Button> {
        Some(match which {
            Which::Start => &mut self.start,
            Which::Mode => &mut self.mode,
            Which::Load => &mut self.load,
            Which::Close => &mut self.close,
            Which::Version => &mut self.version,
            _ => return None,
        })
    }

    fn get(&self, which: Which) -> &dyn Widget {
        match which {
            Which::Relief => &self.relief,
            Which::Land => &self.land,
            Which::Opponents => &self.opponents,
            Which::Rivers => &self.rivers,
            Which::Difficulty => &self.difficulty,
            Which::Start => &self.start,
            Which::Mode => &self.mode,
            Which::Load => &self.load,
            Which::Close => &self.close,
            Which::Version => &self.version,
            Which::Island(i) => &self.islands[i],
            Which::Colour(i) => &self.colours[i],
            Which::Scenario(i) => &self.scenarios[i],
            Which::Name => &self.name,
        }
    }

    /// What `startform_show` shows, in its order. // 1058:1a30
    fn shown_at_start(&mut self) -> Vec<&mut dyn Widget> {
        let mut v: Vec<&mut dyn Widget> = vec![
            &mut self.relief,
            &mut self.land,
            &mut self.opponents,
            &mut self.rivers,
            &mut self.start,
            &mut self.difficulty,
            &mut self.difficulty_heading,
            &mut self.difficulty_low,
            &mut self.difficulty_high,
            &mut self.mode,
            &mut self.load,
            &mut self.close,
            &mut self.title,
            &mut self.credits_heading,
            &mut self.colour_heading,
            &mut self.name_heading,
            &mut self.opponents_heading,
            &mut self.opponents_count,
            &mut self.relief_heading,
            &mut self.relief_low,
            &mut self.relief_high,
            &mut self.land_heading,
            &mut self.land_low,
            &mut self.land_high,
            &mut self.rivers_heading,
            &mut self.rivers_low,
            &mut self.rivers_high,
            &mut self.island_heading,
        ];
        let [atoll, group, one] = &mut self.islands;
        v.extend([atoll as &mut dyn Widget, one, group]);
        v.push(&mut self.name);
        v.extend(self.colours.iter_mut().map(|c| c as &mut dyn Widget));
        v
    }

    /// The normal page's island settings, which the scenario page hides,
    /// in the mode button's order. // 1058:02cf
    fn island_settings(&mut self) -> Vec<&mut dyn Widget> {
        let [atoll, group, one] = &mut self.islands;
        vec![
            &mut self.opponents_heading,
            &mut self.opponents_count,
            &mut self.relief_heading,
            &mut self.relief_low,
            &mut self.relief_high,
            &mut self.land_heading,
            &mut self.land_low,
            &mut self.land_high,
            &mut self.rivers_heading,
            &mut self.rivers_low,
            &mut self.rivers_high,
            &mut self.island_heading,
            atoll,
            one,
            group,
            &mut self.relief,
            &mut self.land,
            &mut self.opponents,
            &mut self.rivers,
        ]
    }

    /// The scenario page's own widgets.
    fn scenario_page(&mut self) -> Vec<&mut dyn Widget> {
        let mut v: Vec<&mut dyn Widget> = vec![&mut self.description];
        v.extend(self.scenarios.iter_mut().map(|c| c as &mut dyn Widget));
        v
    }

    /// What the mode button draws again on both pages, over the new
    /// background. // 1058:02cf
    fn on_both_pages(&mut self) -> Vec<&mut dyn Widget> {
        let mut v: Vec<&mut dyn Widget> = vec![
            &mut self.start,
            &mut self.mode,
            &mut self.load,
            &mut self.close,
            &mut self.title,
            &mut self.credits_heading,
            &mut self.colour_heading,
            &mut self.name_heading,
            &mut self.name,
            &mut self.difficulty,
            &mut self.difficulty_heading,
            &mut self.difficulty_low,
            &mut self.difficulty_high,
        ];
        v.extend(self.colours.iter_mut().map(|c| c as &mut dyn Widget));
        v
    }
}

impl StartForm {
    /// `startform_init`: builds the widgets, loads the texts and sets the
    /// widgets from `settings`. // 1058:27e6
    ///
    /// `bitmaps` are [`BITMAP_FILES`] in that order (a missing one draws
    /// nothing), `start` and `szenario` the lines of `START.TXT` and
    /// `SZENARIO.TXT` (`formats::data::start_txt`, `szenario_txt`).
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(font: Font, bitmaps: Vec<Surface>, start: &[Vec<u8>], szenario: &[Vec<u8>], settings: Settings) -> Self {
        // `startform_load_texts` reads the lines in order; 20 ("Abbruch")
        // and 23 are skipped. // 1058:1ed0
        let s = |line: usize| start.get(line - 1).cloned().unwrap_or_default();
        let z = |line: usize| szenario.get(line - 1).cloned().unwrap_or_default();

        // 20 pixels apart from 0x4a.
        let colour = |i: usize| {
            let left = 0x4a + 0x14 * i32::try_from(i).unwrap_or(0);
            RadioButton::new(
                label(left, 0xc2, 0x10, 0x10, 9, 0, b""),
                BitmapId(COLOUR + i),
                BitmapId(COLOUR_SELECTED + i),
            )
        };
        let mut start_button = button(0x152, 0x132);
        start_button.text = s(3);
        let mut load = button(0xb2, 0x132);
        load.text = s(22);
        let mut version = button(8, 0x8e);
        version.text = s(21);
        let mut mode = button(0x12, 0x132);
        let texts = Texts {
            scenarios: s(24),
            normal_game: z(13),
            descriptions: [z(2), z(4), z(6), z(8), z(10), z(12)],
        };
        mode.text.clone_from(&texts.scenarios);

        let mut w = Widgets {
            difficulty: scroll(0xc2),
            rivers: scroll(0x118),
            land: scroll(0x104),
            relief: scroll(0xf0),
            opponents: scroll(0xdc),
            start: start_button,
            mode,
            load,
            close: Button::new(Frame::new(0, 0, 0x10, 0x10), CLOSE, CLOSE_PRESSED, b"", 9),
            title: label(0, 0x3c, 500, 0x10, 1, 1, &s(1)),
            credits_heading: label(0x12, 0x55, 0x8c, 0x10, 10, 0, &s(2)),
            credits: [0x1c, 0x34, 0x4c].map(|top| label(0xac, top, 0x8c, 0x18, 9, 0, b"")),
            opponents_heading: label(0xaa, 0xdc, 0xb4, 0x10, 9, 0, &s(5)),
            opponents_count: label(0x130, 0xdc, 0x10, 0x10, 9, 1, &decimal(settings.opponents)),
            relief_heading: label(0xaa, 0xf0, 0x55, 0x10, 9, 0, &s(10)),
            relief_low: label(0x104, 0xf0, 0x34, 0x10, 9, 2, &s(11)),
            relief_high: label(0x1ac, 0xf0, 0x3c, 0x10, 9, 0, &s(12)),
            land_heading: label(0xaa, 0x104, 0x55, 0x10, 9, 0, &s(13)),
            land_low: label(0x104, 0x104, 0x34, 0x10, 9, 2, &s(14)),
            land_high: label(0x1ac, 0x104, 0x3c, 0x10, 9, 0, &s(15)),
            rivers_heading: label(0xaa, 0x118, 0x55, 0x10, 9, 0, &s(16)),
            rivers_low: label(0x104, 0x118, 0x34, 0x10, 9, 2, &s(17)),
            rivers_high: label(0x1ac, 0x118, 0x3c, 0x10, 9, 0, &s(18)),
            difficulty_heading: label(300, 0xae, 0x8c, 0x10, 9, 1, &s(25)),
            difficulty_low: label(0x104, 0xc2, 0x34, 0x10, 9, 2, &s(26)),
            difficulty_high: label(0x1ac, 0xc2, 0x3c, 0x10, 9, 0, &s(27)),
            island_heading: label(0x12, 0xdc, 0x76, 0x10, 9, 0, &s(6)),
            colour_heading: label(4, 0xc2, 0x41, 0x10, 9, 2, &s(4)),
            name_heading: label(4, 0xae, 0x41, 0x10, 9, 2, &s(19)),
            islands: [
                check(0x12, 0xf0, 0x76, &s(7)),
                check(0x12, 0x104, 0x76, &s(8)),
                check(0x12, 0x118, 0x76, &s(9)),
            ],
            name: Edit::new(Frame::new(0x4a, 0xae, 0x9c, 0x10), &settings.name, 12, 9, 11),
            colours: std::array::from_fn(colour),
            description: label(0x10, 0x104, 0x1d4, 0x25, 10, 1, &texts.descriptions[0]),
            scenarios: [
                check(0x12, 0xdc, 0x82, &z(1)),
                check(0x12, 0xf0, 0x82, &z(3)),
                check(0xb3, 0xdc, 0x82, &z(5)),
                check(0xb3, 0xf0, 0x82, &z(7)),
                check(0x153, 0xdc, 0x82, &z(9)),
                check(0x153, 0xf0, 0x82, &z(11)),
            ],
            version,
        };
        for (l, text) in w.credits.iter_mut().zip(CREDITS) {
            l.text = text.to_vec();
        }

        // The ranges and positions.
        let ranges = [
            (&mut w.difficulty, 2, settings.difficulty),
            (&mut w.rivers, 10, settings.rivers),
            (&mut w.land, 4, settings.land),
            (&mut w.relief, 4, settings.relief),
            (&mut w.opponents, 7, i16::from(settings.opponents)),
        ];
        for (bar, max, position) in ranges {
            bar.min = 0;
            bar.max = max;
            bar.position = i32::from(position);
        }
        // Only these three values tick a box; the colour box is always the
        // first, yellow, whatever the player's colour is, and the scenario
        // box always the first.
        if let Some(island) = usize::try_from(settings.island).ok().and_then(|i| w.islands.get_mut(i)) {
            island.radio.on = true;
        }
        w.colours[0].on = true;
        w.scenarios[0].radio.on = true;

        let empty = Surface::new(0, 0, 0);
        StartForm {
            assets: Assets {
                font,
                bitmaps,
                background: START1,
                empty,
            },
            bild: Surface::new(WIDTH as usize, HEIGHT as usize, 0),
            w,
            texts,
            scenario_mode: false,
            settings,
            enhance: Enhancements::ORIGINAL,
        }
    }

    /// Where the form goes on a `screen_width`×`screen_height` screen:
    /// centred, as `startform_init` and `startform_show` place it.
    /// // 1058:1a30
    #[must_use]
    pub fn position(screen_width: i32, screen_height: i32) -> (i32, i32) {
        ((screen_width - WIDTH) / 2, (screen_height - HEIGHT) / 2)
    }

    /// `startform_show`: draws the background into `Bild` and shows the
    /// normal page's widgets. The original centres the form and calls
    /// `Show` too; that's up to the caller ([`StartForm::position`]).
    /// // 1058:1a30
    pub fn show(&mut self) {
        let StartForm { assets, bild, w, .. } = self;
        let clip = bild.bounds();
        bild.draw(0, 0, assets.background(), clip);
        let skin = assets.skin();
        for widget in w.shown_at_start() {
            widget.show(bild, &skin);
        }
    }

    /// `Bild`'s picture.
    #[must_use]
    pub fn bild(&self) -> &Surface {
        &self.bild
    }

    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// On the scenario page. // 1110:6ece
    #[must_use]
    pub fn scenario_mode(&self) -> bool {
        self.scenario_mode
    }

    /// `startform_bildmousedown`, with `x`, `y` in `Bild` and the event's
    /// shift state. Only with the left button down, it offers the press to
    /// the widgets in turn until one takes it. The edit field comes last,
    /// so a click on another widget doesn't unfocus it. // 1058:3cfd
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        if !shift.contains(ShiftState::LEFT) {
            return Response::default();
        }
        for which in SCROLL_DOWN_ORDER {
            if let Some(bar) = self.w.scroll_mut(which) {
                let out = bar.mouse_down(x, y, self.enhance);
                if out.handled {
                    return self.finish(which, out);
                }
            }
        }
        for which in BUTTON_ORDER {
            if let Some(b) = self.w.button_mut(which) {
                let out = b.mouse_down(x, y);
                if out.handled {
                    return self.finish(which, out);
                }
            }
        }
        // The islands and scenarios are check boxes, pressed like radio
        // buttons (`radio_mouse_down`), so a click only ticks them.
        let radios = ISLAND_ORDER
            .map(Which::Island)
            .into_iter()
            .chain((0..8).map(Which::Colour))
            .chain((0..6).map(Which::Scenario));
        for which in radios {
            let radio = match which {
                Which::Island(i) => &mut self.w.islands[i].radio,
                Which::Colour(i) => &mut self.w.colours[i],
                Which::Scenario(i) => &mut self.w.scenarios[i].radio,
                _ => continue,
            };
            let out = radio.mouse_down(x, y);
            if out.handled {
                return self.finish(which, out);
            }
        }
        let out = self.w.name.mouse_down(x, y, self.enhance);
        if out.handled {
            return self.finish(Which::Name, out);
        }
        Response::default()
    }

    /// `startform_bildmousemove`: drags a scroll bar's thumb, but only when
    /// the shift state is exactly the left button. // 1058:412f
    pub fn mouse_move(&mut self, x: i32, y: i32, shift: ShiftState) -> Response {
        if shift != ShiftState::LEFT {
            return Response::default();
        }
        for which in SCROLL_MOVE_ORDER {
            if let Some(bar) = self.w.scroll_mut(which) {
                let out = bar.mouse_move(x, y);
                if out.handled {
                    return self.finish(which, out);
                }
            }
        }
        Response::default()
    }

    /// `scroll_bars`' repeat of a scroll bar's arrow held down (a port
    /// extension, [`ScrollBar::repeat`]), `ms` after the last call.
    pub fn repeat(&mut self, ms: u32) -> Response {
        for which in SCROLL_DOWN_ORDER {
            let e = self.enhance;
            if let Some(bar) = self.w.scroll_mut(which) {
                let out = bar.repeat(ms, e);
                if out.handled {
                    return self.finish(which, out);
                }
            }
        }
        Response::default()
    }

    /// Whether a scroll bar's arrow is held down, for the caller to call
    /// [`StartForm::repeat`].
    #[must_use]
    pub fn scroll_held(&mut self) -> bool {
        SCROLL_DOWN_ORDER
            .iter()
            .any(|&w| self.w.scroll_mut(w).is_some_and(|b| b.pressed.is_some()))
    }

    /// `startform_bildmouseup`: the scroll bars, then the buttons, until
    /// one takes it. It doesn't look at which button came up, so letting go
    /// of the right button also releases (and clicks) a button held down
    /// with the left; with `mouse_buttons` only the `left` one does.
    /// // 1058:41fb
    pub fn mouse_up(&mut self, x: i32, y: i32, left: bool) -> Response {
        // BUG(original): doesn't look at which button came up, so letting go of the right one clicks a button held with the left.
        if !left && self.enhance.has(Enhancement::MouseButtons) {
            return Response::default();
        }
        for which in SCROLL_DOWN_ORDER {
            if let Some(bar) = self.w.scroll_mut(which) {
                let out = bar.mouse_up(x, y, self.enhance);
                if out.handled {
                    return self.finish(which, out);
                }
            }
        }
        for which in BUTTON_ORDER {
            if let Some(b) = self.w.button_mut(which) {
                let out = b.mouse_up(x, y);
                if out.handled {
                    return self.finish(which, out);
                }
            }
        }
        Response::default()
    }

    /// `startform_formkeydown`: Alt+F4 (with no other key or button held)
    /// quits; every other key goes to the name field. // 1058:4381
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Response {
        if shift == ShiftState::ALT && key == VK_F4 {
            return Response {
                action: Some(Action::Quit),
                ..Response::default()
            };
        }
        let out = self.w.name.key_down(key, shift);
        if out.handled {
            self.finish(Which::Name, out)
        } else {
            Response::default()
        }
    }

    /// Repaints the widget if it asks for it, then runs its `Click`.
    fn finish(&mut self, which: Which, out: Outcome) -> Response {
        if out.repaint {
            self.repaint(which);
        }
        let action = if out.clicked { self.click(which) } else { None };
        Response {
            refresh: out.handled,
            sound: out.sound,
            action,
        }
    }

    fn repaint(&mut self, which: Which) {
        let StartForm { assets, bild, w, .. } = self;
        w.get(which).repaint(bild, assets.background(), &assets.skin());
    }

    /// The widgets' `Click` overrides.
    fn click(&mut self, which: Which) -> Option<Action> {
        match which {
            // 1058:014d
            Which::Relief => self.settings.relief = low_word(self.w.relief.position),
            // 1058:015e
            Which::Land => self.settings.land = low_word(self.w.land.position),
            // 1058:01f8
            Which::Rivers => self.settings.rivers = low_word(self.w.rivers.position),
            // 1058:0209
            Which::Difficulty => self.settings.difficulty = low_word(self.w.difficulty.position),
            Which::Opponents => self.opponents_click(),
            // 1058:021a
            Which::Start => {
                self.settings.name.clone_from(&self.w.name.text);
                return Some(Action::Start {
                    scenario: self.scenario_mode,
                });
            }
            Which::Mode => self.mode_click(),
            // 1058:0c7d
            Which::Load => return Some(Action::Load),
            // 1058:0cbb
            Which::Close => return Some(Action::Quit),
            // 1058:02b0: empty. The button is never shown anyway.
            Which::Version | Which::Name => {}
            Which::Island(i) => self.island_click(i),
            Which::Colour(i) => self.colour_click(i),
            Which::Scenario(i) => self.scenario_click(i),
        }
        None
    }

    /// `startform_opponents_click`: stores the count and writes it next to
    /// the bar, erasing the old number first. // 1058:016f
    fn opponents_click(&mut self) {
        self.settings.opponents = low_byte(self.w.opponents.position);
        self.settings.players = self.settings.opponents.wrapping_add(1);
        let StartForm { assets, bild, w, .. } = self;
        w.opponents_count.frame.erase(bild, assets.background());
        w.opponents_count.text = decimal(self.settings.opponents);
        w.opponents_count.draw(bild, &assets.skin());
    }

    /// The island check boxes' `Click`s: store the shape and untick the
    /// other two. // 1058:0cdd 1058:0d23 1058:0d6a
    fn island_click(&mut self, i: usize) {
        self.settings.island = i16::try_from(i).unwrap_or(0);
        for other in [0, 1, 2] {
            if other != i {
                let out = self.w.islands[other].radio.deselect();
                if out.repaint {
                    self.repaint(Which::Island(other));
                }
            }
        }
    }

    /// The colour boxes' `Click`s: give the player colour `i + 1` and untick
    /// the other seven. // 1058:0ded..1058:13ce
    fn colour_click(&mut self, i: usize) {
        take_colour(&mut self.settings.colours, u8::try_from(i + 1).unwrap_or(0));
        for other in 0..8 {
            if other != i {
                let out = self.w.colours[other].deselect();
                if out.repaint {
                    self.repaint(Which::Colour(other));
                }
            }
        }
    }

    /// The scenario boxes' `Click`s: store the scenario, untick the others
    /// and show its description. // 1058:14a5..1058:1900
    fn scenario_click(&mut self, i: usize) {
        self.settings.scenario = u8::try_from(i + 1).unwrap_or(0);
        for other in 0..6 {
            if other != i {
                let out = self.w.scenarios[other].radio.deselect();
                if out.repaint {
                    self.repaint(Which::Scenario(other));
                }
            }
        }
        let StartForm {
            assets, bild, w, texts, ..
        } = self;
        w.description.text.clone_from(&texts.descriptions[i]);
        w.description.frame.erase(bild, assets.background());
        w.description.draw(bild, &assets.skin());
    }

    /// `startform_mode_click`: switches between the normal page and the
    /// scenarios. // 1058:02cf
    ///
    /// Each switch starts on scenario 1 (or a normal game), but the boxes
    /// and the description stay as they were, so after going back and
    /// forth a different scenario can be ticked than the one that starts.
    /// With `start_menu_scenario` the scenario page ticks scenario 1 and
    /// shows its description.
    fn mode_click(&mut self) {
        let StartForm {
            assets,
            bild,
            w,
            texts,
            scenario_mode,
            settings,
            ..
        } = self;
        let clip = bild.bounds();
        if *scenario_mode {
            // BUG(original): the boxes and description stay as they were, so a different scenario can be ticked than the one that starts.
            *scenario_mode = false;
            settings.scenario = 0;
            for widget in w.scenario_page() {
                widget.hide(bild, assets.background());
            }
            assets.background = START1;
            bild.draw(0, 0, assets.background(), clip);
            let skin = assets.skin();
            for widget in w.island_settings() {
                widget.show(bild, &skin);
            }
            w.mode.text.clone_from(&texts.scenarios);
        } else {
            *scenario_mode = true;
            settings.scenario = 1;
            for widget in w.island_settings() {
                widget.hide(bild, assets.background());
            }
            w.mode.text.clone_from(&texts.normal_game);
            assets.background = STARTSZE;
            bild.draw(0, 0, assets.background(), clip);
            let skin = assets.skin();
            for widget in w.scenario_page() {
                widget.show(bild, &skin);
            }
        }
        let skin = assets.skin();
        for widget in w.on_both_pages() {
            widget.draw(bild, &skin);
        }
        if self.scenario_mode && self.enhance.has(Enhancement::StartMenuScenario) {
            let out = self.w.scenarios[0].radio.select();
            if out.repaint {
                self.repaint(Which::Scenario(0));
            }
            self.scenario_click(0);
        }
    }
}

/// `take_colour`: the human (the first entry) takes `colour`, and whoever
/// had it gets the human's old one. // 1058:0db1
fn take_colour(colours: &mut [u8; 8], colour: u8) {
    for i in 1..8 {
        if colours[i] == colour {
            colours[i] = colours[0];
            colours[0] = colour;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::bmp::Bitmap;

    /// A font whose glyphs change nothing: a white mask and black colours.
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

    fn texts(n: usize, prefix: &str) -> Vec<Vec<u8>> {
        (1..=n).map(|i| format!("{prefix}{i}").into_bytes()).collect()
    }

    /// A form whose bitmaps are filled with their index plus one, shown.
    fn form() -> StartForm {
        let bitmaps = (0..BITMAP_FILES.len())
            .map(|i| Surface::new(504, 336, u8::try_from(i + 1).unwrap()))
            .collect();
        let mut f = StartForm::new(font(), bitmaps, &texts(27, "s"), &texts(13, "z"), Settings::default());
        f.show();
        f
    }

    fn click(f: &mut StartForm, x: i32, y: i32) -> Response {
        let down = f.mouse_down(x, y, ShiftState::LEFT);
        let up = f.mouse_up(x, y, true);
        if up.refresh { up } else { down }
    }

    #[test]
    fn starts_from_the_settings() {
        let f = form();
        assert!(f.w.islands[1].checked(), "a group of islands");
        assert!(!f.w.islands[0].checked() && !f.w.islands[2].checked());
        assert!(f.w.colours[0].on && f.w.scenarios[0].checked());
        assert_eq!(f.w.opponents.position, 7);
        assert_eq!(f.w.opponents_count.text, b"7");
        assert_eq!(f.w.start.text, b"s3");
        assert_eq!(f.w.mode.text, b"s24");
        assert_eq!(f.w.description.text, b"z2");
        assert!(f.w.relief.frame.visible && !f.w.scenarios[0].frame().visible);
        assert!(!f.w.version.frame.visible && !f.w.credits[0].frame.visible);
        // The background, and the close button's bitmap at the top left.
        assert_eq!(f.bild.pixel(250, 100), 1);
        assert_eq!(f.bild.pixel(3, 3), 29);
        assert_eq!(StartForm::position(800, 600), (150, 133));
    }

    #[test]
    fn colours_swap() {
        let mut colours = [1, 2, 3, 4, 5, 6, 7, 8];
        take_colour(&mut colours, 3);
        assert_eq!(colours, [3, 2, 1, 4, 5, 6, 7, 8]);
        take_colour(&mut colours, 2);
        assert_eq!(colours, [2, 3, 1, 4, 5, 6, 7, 8]);
        take_colour(&mut colours, 2);
        assert_eq!(colours, [2, 3, 1, 4, 5, 6, 7, 8]);

        let mut f = form();
        let r = f.mouse_down(0x72 + 4, 0xc2 + 4, ShiftState::LEFT);
        assert_eq!(r.sound, Some(Sound::Select));
        assert!(r.refresh);
        assert_eq!(f.settings.colours, [3, 2, 1, 4, 5, 6, 7, 8]);
        assert!(f.w.colours[2].on && !f.w.colours[0].on);
        // Drawn with the selected bitmap.
        assert_eq!(f.bild.pixel(0x72, 0xc2), u8::try_from(COLOUR_SELECTED + 2 + 1).unwrap());
    }

    #[test]
    fn island_boxes_act_as_radio_buttons() {
        let mut f = form();
        let _ = click(&mut f, 0x12 + 2, 0xf0 + 2);
        assert_eq!(f.settings.island, 0);
        assert!(f.w.islands[0].checked() && !f.w.islands[1].checked());
        // Clicking it again doesn't untick it.
        assert_eq!(f.mouse_down(0x12 + 2, 0xf0 + 2, ShiftState::LEFT), Response::default());
        assert!(f.w.islands[0].checked());
        let _ = click(&mut f, 0x12 + 2, 0x118 + 2);
        assert_eq!(f.settings.island, 2);
    }

    #[test]
    fn opponents_bar() {
        let mut f = form();
        // The back arrow steps when it's released.
        let down = f.mouse_down(0x142 + 4, 0xdc + 4, ShiftState::LEFT);
        assert_eq!(down.sound, Some(Sound::Press));
        assert_eq!(f.settings.opponents, 7);
        let up = f.mouse_up(0x142 + 4, 0xdc + 4, true);
        assert_eq!(up.sound, Some(Sound::Release));
        assert_eq!((f.settings.opponents, f.settings.players), (6, 7));
        assert_eq!(f.w.opponents_count.text, b"6");
        // The trough pages at once, by 10, stopping at 0.
        let _ = f.mouse_down(0x142 + 20, 0xdc + 4, ShiftState::LEFT);
        assert_eq!(f.settings.opponents, 0);
        // The others just store the position.
        let _ = click(&mut f, 0x142 + 0x5c, 0xf0 + 4);
        assert_eq!(f.settings.relief, 3);
        let _ = click(&mut f, 0x142 + 0x5c, 0xc2 + 4);
        assert_eq!(f.settings.difficulty, 1);
    }

    #[test]
    fn held_arrows_repeat_with_the_fix() {
        let mut f = form();
        f.enhance.set(Enhancement::ScrollBars, true);
        // The opponents' back arrow: a step at once, more while held.
        let _ = f.mouse_down(0x142 + 4, 0xdc + 4, ShiftState::LEFT);
        assert!(f.scroll_held());
        assert_eq!(f.settings.opponents, 6);
        let r = f.repeat(crate::widgets::REPEAT_DELAY_MS + 2 * crate::widgets::REPEAT_MS);
        assert!(r.refresh);
        assert_eq!(
            (f.settings.opponents, f.w.opponents_count.text.as_slice()),
            (3, b"3".as_slice())
        );
        let _ = f.mouse_up(0x142 + 4, 0xdc + 4, true);
        assert!(!f.scroll_held());
        assert!(!f.repeat(1000).refresh);
        // BUG(original): no repeat.
        f.enhance = Enhancements::ORIGINAL;
        let _ = f.mouse_down(0x142 + 4, 0xdc + 4, ShiftState::LEFT);
        assert!(!f.repeat(1000).refresh);
    }

    #[test]
    fn needs_the_left_button() {
        let mut f = form();
        assert_eq!(
            f.mouse_down(0x152 + 4, 0x132 + 4, ShiftState::RIGHT),
            Response::default()
        );
        assert!(!f.w.start.pressed);
        // But any button's release lets go.
        let _ = f.mouse_down(0x152 + 4, 0x132 + 4, ShiftState::LEFT | ShiftState::RIGHT);
        let up = f.mouse_up(0x152 + 4, 0x132 + 4, true);
        assert_eq!(up.action, Some(Action::Start { scenario: false }));
        // Dragging needs exactly the left button.
        let _ = f.mouse_down(0x142 + 45, 0xf0 + 4, ShiftState::LEFT);
        assert_eq!(f.w.relief.pressed, Some(crate::widgets::Part::Thumb));
        let before = f.w.relief.position;
        assert!(
            !f.mouse_move(0x142 + 80, 0xf0 + 4, ShiftState::LEFT | ShiftState::SHIFT)
                .refresh
        );
        assert_eq!(f.w.relief.position, before);
        assert!(f.mouse_move(0x142 + 80, 0xf0 + 4, ShiftState::LEFT).refresh);
        assert_eq!(f.settings.relief, 4);
    }

    #[test]
    fn scenario_page() {
        let mut f = form();
        let r = click(&mut f, 0x12 + 4, 0x132 + 4);
        assert!(r.action.is_none() && f.scenario_mode);
        assert_eq!(f.settings.scenario, 1);
        assert_eq!(f.w.mode.text, b"z13");
        assert!(!f.w.relief.frame.visible && !f.w.islands[0].frame().visible);
        assert!(f.w.scenarios[5].frame().visible && f.w.description.frame.visible);
        assert_eq!(f.bild.pixel(250, 100), 28, "startsze.bmp");
        // Clicks on the hidden settings do nothing.
        assert_eq!(click(&mut f, 0x142 + 4, 0xf0 + 4), Response::default());

        let _ = click(&mut f, 0x153 + 2, 0xdc + 2);
        assert_eq!(f.settings.scenario, 5);
        assert!(f.w.scenarios[4].checked() && !f.w.scenarios[0].checked());
        assert_eq!(f.w.description.text, b"z10");

        let r = click(&mut f, 0x152 + 4, 0x132 + 4);
        assert_eq!(r.action, Some(Action::Start { scenario: true }));

        // Back and forth: scenario 1 again, with box 5 still ticked.
        let _ = click(&mut f, 0x12 + 4, 0x132 + 4);
        assert!(!f.scenario_mode);
        assert_eq!(f.settings.scenario, 0);
        assert_eq!(f.bild.pixel(250, 100), 1);
        assert!(f.w.relief.frame.visible);
        let _ = click(&mut f, 0x12 + 4, 0x132 + 4);
        assert_eq!(f.settings.scenario, 1);
        assert!(f.w.scenarios[4].checked());
    }

    #[test]
    fn name_and_buttons() {
        let mut f = form();
        // Focus the name, type, and the edit field keeps focus while other
        // widgets take clicks.
        let r = f.mouse_down(0x4a + 4, 0xae + 4, ShiftState::LEFT);
        assert!(r.refresh && f.w.name.focused);
        let _ = f.key_down(0x41, ShiftState::SHIFT);
        let _ = click(&mut f, 0x12 + 2, 0xf0 + 2);
        assert!(f.w.name.focused);
        assert!(f.key_down(0x42, ShiftState::default()).refresh);
        assert_eq!(f.w.name.text, b"HolidayAb");
        let r = click(&mut f, 0x152 + 4, 0x132 + 4);
        assert_eq!(r.action, Some(Action::Start { scenario: false }));
        assert_eq!(f.settings.name, b"HolidayAb");

        assert_eq!(click(&mut f, 0xb2 + 4, 0x132 + 4).action, Some(Action::Load));
        assert_eq!(click(&mut f, 4, 4).action, Some(Action::Quit));
        assert_eq!(f.key_down(VK_F4, ShiftState::ALT).action, Some(Action::Quit));
        assert_eq!(f.key_down(VK_F4, ShiftState::ALT | ShiftState::SHIFT).action, None);
        // Released off the button: no click.
        let _ = f.mouse_down(4, 4, ShiftState::LEFT);
        let r = f.mouse_up(40, 40, true);
        assert!(r.refresh && r.action.is_none());
    }

    fn fixed(e: Enhancement) -> StartForm {
        let mut f = form();
        f.enhance.set(e, true);
        f
    }

    #[test]
    fn scenario_page_with_the_fix() {
        let mut f = fixed(Enhancement::StartMenuScenario);
        let _ = click(&mut f, 0x12 + 4, 0x132 + 4);
        let _ = click(&mut f, 0x153 + 2, 0xdc + 2);
        assert!(f.w.scenarios[4].checked());
        let _ = click(&mut f, 0x12 + 4, 0x132 + 4);
        let _ = click(&mut f, 0x12 + 4, 0x132 + 4);
        // Scenario 1 starts, and its box is the ticked one.
        assert_eq!(f.settings.scenario, 1);
        assert!(f.w.scenarios[0].checked() && !f.w.scenarios[4].checked());
        assert_eq!(f.w.description.text, b"z2");
    }

    #[test]
    fn right_button_with_the_fix() {
        for (e, clicks) in [(Enhancements::ORIGINAL, true), (Enhancements::all_on(), false)] {
            let mut f = form();
            f.enhance = e;
            let _ = f.mouse_down(0x152 + 4, 0x132 + 4, ShiftState::LEFT | ShiftState::RIGHT);
            let up = f.mouse_up(0x152 + 4, 0x132 + 4, false);
            assert_eq!(up.action.is_some(), clicks);
        }
    }
}
