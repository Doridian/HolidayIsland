//! The main screen's tools: the button bar's tool buttons (1010:2ac7 to
//! 1010:2c8b), what a click on the island does with the tool
//! (1028:3a68 to 1028:3dd0, from `HauptbildMouseDown`, 1020:1984), the
//! road tool's drag (1080:3195, 1080:31ed), the tools' pictures over the
//! island (1090:0014, 1090:124f, 1090:155b) and the status bar's
//! messages (1030:232d, 1030:235a). `docs/player-tools.md` has the whole
//! of it.
//!
//! This is the view: one human's tools. What a tool does to the island is
//! a [`command::Action`], which the caller hands the world as player 1's
//! command; the world's answer comes back in a [`Tick`].
//!
//! The tools are the buttons' numbers (`1110:6056`; the tables at
//! `1110:14e0` and `1110:14f2` that give a tool's button are the same
//! numbers): 6 raises a corner of the land, 7 lowers one, 8 demolishes,
//! 9 to 20 but 16 open a submenu of the build menu and build its type,
//! 16 drags out a road, 21 sabotages with the kind picked on
//! `TSabowin` ([`crate::sabowin`], which its button shows) and 22 is the
//! pointer, whose click opens the info box (`TInfobox2`,
//! [`crate::infobox`], through [`Tools::info_box`]). Buttons 0 to 5 open
//! forms, which are the caller's: the file menu (0), the pause (1), the
//! options (2), the TV (3), the money menu (4) and the overview map (5).

use formats::data::Strings;

use crate::baumenue::{Baumenue, Response};
use crate::command::Action;
use crate::darstellung::{self, Camera, Tiles, ToolSprite, ViewSize};
use crate::enhance::{Enhancement, Enhancements};
use crate::hauptschirm::InfoTexts;
use crate::infobox;
use crate::roads::CROSSABLE;
use crate::sabowin::{self, Close, Sabowin};
use crate::savegame::Fields;
use crate::simulation::{Status, Tick};
use crate::text::Font;
use crate::widgets::ShiftState;
use crate::world::{NO_BUILDING, PALM, Pos, World, on_map};

pub const RAISE: u8 = 6;
pub const LOWER: u8 = 7;
pub const DEMOLISH: u8 = 8;
pub const ROAD: u8 = 0x10;
pub const SABOTAGE: u8 = 0x15;
pub const POINTER: u8 = 0x16;
/// The button bar's buttons, and "none" (`1110:5fbe` when the mouse is
/// over no button).
pub const BUTTONS: usize = 23;
pub const NO_BUTTON: u8 = 0x17;

/// Whether `tool` is one of the build menu's submenus. // 1010:2b60
#[must_use]
pub fn is_menu(tool: u8) -> bool {
    (9..=0x14).contains(&tool) && tool != ROAD
}

/// The building types whose squares the demolish tool's picture leaves
/// out: palms, beach life and none. // 1110:1358 (`set of Byte`)
const FREE_SQUARES: [u8; 3] = [PALM, 36, NO_BUILDING];

/// The `ANIMDAT1.DAT` rows of a square of the tools' pictures, by its
/// form: one that can take it (`1110:120b`, the ninth ground's first
/// `GROUNDNR.DAT` row) and one that can't. // 1090:0014
const SQUARE_FITS: i16 = 76;
const SQUARE_DOESNT: i16 = 0x17c;

/// The pointers' frames and pictures: the raise and lower tools' arrows
/// and the demolish tool's, eight frames each, and their offsets.
/// // 1090:124f
const RAISE_POINTER: (i16, (i16, i16)) = (0x10, (0x19, 6));
const LOWER_POINTER: (i16, (i16, i16)) = (8, (0x19, 6));
const DEMOLISH_POINTER: (i16, (i16, i16)) = (0x3c, (0x10, 0));
const SABOTAGE_POINTER: (i16, (i16, i16)) = (0x44, (0x16, 0));

/// The main screen's tool state and the build menu.
#[allow(clippy::struct_excessive_bools)] // The original's flags.
#[derive(Debug, Clone)]
pub struct Tools {
    /// The tool. // 1110:6056
    pub tool: u8,
    /// The button the mouse is over, or was clicked. // 1110:5fbe
    pub button: u8,
    /// The buttons held down (24 bytes, the last never used).
    /// // 1110:651a
    pub down: [bool; BUTTONS + 1],
    /// The square under the mouse, worked out each tick. // 1110:605a
    pub square: Pos,
    /// The road tool's drag: where it started and where it is.
    /// // 1110:6344, 1110:6348
    pub drag_start: Pos,
    pub drag_end: Pos,
    /// The pointers' frame, 0–7. // 1110:6fd4
    frame: i16,
    /// The building the demolish tool's picture last showed: the
    /// original's `1110:6ff2`, which the people code also writes.
    last_building: u8,
    /// The tool's pictures over the island, in the order they're drawn:
    /// the squares, then the pointer.
    pub sprites: Vec<ToolSprite>,
    /// The build menu.
    pub menu: Baumenue,
    /// The build menu has the keyboard's focus (it takes it when it
    /// opens, 1018:2b01; the main screen when it closes, 1018:2c71).
    pub menu_focused: bool,
    /// `FEHLER.TXT`. // 1110:734a
    fehler: Strings,
    /// The status bar's message, when the world's tick or command left one.
    pub status: Option<Vec<u8>>,
    /// The button bar needs drawing again (1010:2af5).
    pub buttons_changed: bool,
    /// Sounds to play, in order (`play_sound`).
    pub sounds: Vec<u8>,
    /// The sabotage window, once the caller has given it its data; until
    /// then the tool sabotages with kind 0. // 1110:65c0
    pub sabowin: Option<Sabowin>,
    /// The sabotage window has the keyboard's focus: it takes it when it's
    /// shown and gives it back when it's hidden; a click on another form
    /// takes it too (the caller's).
    pub sabowin_focused: bool,
    /// The last press of the pointer was a double click's: its info box
    /// stays open when the button comes up. The main screen's
    /// `1110:64b8`.
    pub pointer_double: bool,
    /// What the pointer asks of the info box (`TInfobox2`), for the
    /// caller to take.
    pub info_box: Option<infobox::Request>,
    /// The main screen's map view, which the mouse is picked in
    /// (`Hauptbild`'s size, 1018:39c2): the original's, or `big_map`'s,
    /// which the caller sets ([`crate::hauptschirm::Hauptschirm::view_size`])
    /// and then passes the matching camera
    /// ([`crate::hauptschirm::Hauptschirm::drawn_camera`]).
    pub view: ViewSize,
    /// The player's view enhancements (`build_menu`, `modifier_clicks`,
    /// `road_drag`).
    pub enhance: Enhancements,
}

/// One step towards `stop` along a line of squares (the road planner's
/// and the preview's `for` loops), bounded.
fn line(start: i16, stop: i16) -> impl Iterator<Item = i16> {
    let sign = if start < stop { 1 } else { -1 };
    (0..=start.abs_diff(stop)).map(move |n| start + sign * i16::try_from(n).unwrap_or(0))
}

impl Tools {
    /// The tools as a game starts: the pointer, its button held down.
    /// // 1030:36e5, 1010:2fa9
    #[must_use]
    pub fn new(menu: Baumenue, fehler: Strings) -> Self {
        let mut down = [false; BUTTONS + 1];
        down[usize::from(POINTER)] = true;
        Tools {
            tool: POINTER,
            button: NO_BUTTON,
            down,
            square: (0, 0),
            drag_start: (0, 0),
            drag_end: (0, 0),
            frame: 0,
            last_building: 0,
            sprites: Vec::new(),
            menu,
            menu_focused: false,
            fehler,
            status: None,
            buttons_changed: true,
            sounds: Vec::new(),
            sabowin: None,
            sabowin_focused: false,
            pointer_double: false,
            info_box: None,
            view: ViewSize::ORIGINAL,
            enhance: Enhancements::ORIGINAL,
        }
    }

    /// The tool, the buttons and the build menu's fields from the map a
    /// game starts from (its view's fields), and the build menu drawn for
    /// the tool (`game_start_setup`, 1010:2820, 1018:2a17). A new island
    /// has none of them and keeps [`Tools::new`]'s.
    pub fn start(&mut self, fields: &Fields, world: &World, font: &Font, tiles: &Tiles, info: &InfoTexts, view: u8) {
        let read = |dest: &str, at: usize| {
            fields
                .get(dest, 0)
                .and_then(|d| d.get(at..at + 2))
                .map(|w| i16::from_le_bytes([w[0], w[1]]))
        };
        if let Some(tool) = read("1110:6056", 0) {
            self.tool = tool.to_le_bytes()[0];
        } else if fields.get("1110:651a", 0).is_some() {
            // A map without a tool saved 0 there.
            self.tool = 0;
        }
        if let Some(b) = read("1110:6058", 0) {
            self.menu.building = b;
        }
        self.square = (read("1110:605a", 0).unwrap_or(0), read("1110:605a", 2).unwrap_or(0));
        self.drag_start = (read("1110:6344", 0).unwrap_or(0), read("1110:6344", 2).unwrap_or(0));
        self.drag_end = (read("1110:6348", 0).unwrap_or(0), read("1110:6348", 2).unwrap_or(0));
        if let Some(d) = fields.get("1110:651a", 0) {
            for (b, &v) in self.down.iter_mut().zip(d) {
                *b = v != 0;
            }
        }
        if let Some(b) = fields.get("1110:5fbe", 0).and_then(|d| d.first()) {
            self.button = *b;
        }
        self.menu.load(fields);
        let (tool, button) = (self.tool, self.button);
        self.menu.restore_menu(tool, button, world, font, tiles, info, view); // 1018:2a17
        self.buttons_changed = true;
    }

    /// The tool's fields into the view's `fields` of the map file.
    pub fn save(&self, fields: &mut Fields) {
        let pos = |(i, j): Pos| [i.to_le_bytes(), j.to_le_bytes()].concat();
        fields.set("1110:6056", 0, &u16::from(self.tool).to_le_bytes());
        fields.set("1110:6058", 0, &self.menu.building.to_le_bytes());
        fields.set("1110:605a", 0, &pos(self.square));
        fields.set("1110:6344", 0, &pos(self.drag_start));
        fields.set("1110:6348", 0, &pos(self.drag_end));
        fields.set("1110:5fbe", 0, &[self.button]);
        let down: Vec<u8> = self.down.iter().map(|&d| u8::from(d)).collect();
        fields.set("1110:651a", 0, &down);
        self.menu.save(fields);
    }

    fn button_down(&mut self, b: u8) {
        if let Some(d) = self.down.get_mut(usize::from(b)) {
            *d = true;
        }
        self.buttons_changed = true; // 1010:2ac7
    }

    fn button_up(&mut self, b: u8) {
        if let Some(d) = self.down.get_mut(usize::from(b)) {
            *d = false;
        }
        self.buttons_changed = true; // 1010:2ade
    }

    /// The pointer takes over from `button`'s tool. // 1010:2bc4
    fn pointer_instead_of(&mut self, button: u8) {
        self.tool = POINTER;
        self.button_up(button);
        self.button_down(POINTER);
    }

    /// The old tool's button comes up and the build menu closes before
    /// another tool's button goes down. // 1010:2be1
    ///
    /// The original closes the build menu if its form's `Visible`
    /// (`+0x29`) is set, which it is from its creation on, so always;
    /// with `build_menu` only when it's shown.
    fn leave_tool(&mut self, old: u8) {
        self.button_up(old); // 1110:14f2[old]
        if old == SABOTAGE {
            self.hide_sabowin();
        }
        // BUG(original): the build menu's `Visible` is always set, so it's closed (with its sound) even when it is.
        if self.menu.height > 0 || !self.enhance.has(Enhancement::BuildMenu) {
            self.close_menu();
        }
    }

    fn close_menu(&mut self) {
        self.menu.close(); // 1018:2c71
        self.menu_focused = false;
    }

    #[allow(clippy::too_many_arguments)]
    fn open_menu(
        &mut self,
        tool: u8,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
        screen: (i32, i32),
    ) {
        let button = self.button;
        self.menu.open(tool, button, world, font, tiles, info, view, screen); // 1018:2b01
        self.menu_focused = true;
    }

    /// A tool's button clicked again: the pointer takes over, but a
    /// submenu whose form was closed opens again. The road drag ends.
    /// // 1010:2b60
    fn release_tool(
        &mut self,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
        screen: (i32, i32),
    ) -> Vec<Action> {
        if !is_menu(self.tool) {
            self.tool = POINTER;
            self.button_down(POINTER);
        } else if !self.menu.open {
            let tool = self.tool;
            self.open_menu(tool, world, font, tiles, info, view, screen);
            self.button_down(tool); // 1010:2b4c, 1110:14e0[tool]
        } else {
            self.tool = POINTER;
            self.button_down(POINTER);
            self.close_menu();
        }
        drag_cancel(world)
    }

    /// `button_bar_click` for button `b` (`1110:5fbe`): a button that's up
    /// goes down and its tool is picked (a submenu's opens the build menu,
    /// the sabotage tool's shows `TSabowin`), one that's down comes up and
    /// the pointer takes over. Buttons 0 to 5 open forms, which are the
    /// caller's (`crate::pauseform`, `crate::optionen`, `crate::fernseher`,
    /// `crate::geldmenu`, `crate::uebersichtsmap`); here they do nothing.
    /// // 1010:2c8b
    #[allow(clippy::too_many_arguments)]
    pub fn click(
        &mut self,
        b: u8,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
        screen: (i32, i32),
    ) -> Vec<Action> {
        if !(RAISE..NO_BUTTON).contains(&b) {
            return Vec::new();
        }
        self.button = b;
        self.sounds.push(9); // 10a0:30b6
        let mut actions = Vec::new();
        if self.down[usize::from(b)] {
            self.button_up(b);
            if b == POINTER {
                self.button_down(POINTER);
                self.tool = POINTER;
            } else {
                if b == SABOTAGE {
                    self.hide_sabowin();
                }
                actions = self.release_tool(world, font, tiles, info, view, screen);
                if b == 0x11 {
                    actions.extend(drag_cancel(world));
                }
            }
        } else {
            self.button_down(b);
            let old = self.tool;
            self.leave_tool(old);
            self.tool = b;
            if is_menu(b) {
                self.open_menu(b, world, font, tiles, info, view, screen);
            } else if b == SABOTAGE {
                self.show_sabowin();
            } else if b == ROAD {
                // BUG(original): the road's button selects building type 1, which no tool reads before a submenu selects another.
                self.drag_start = self.square;
                self.drag_end = self.square;
                self.menu.building = 1;
            }
        }
        actions
    }

    /// The left button went down on `Hauptbild` at `x`, `y` with `shift`
    /// (only exactly `[ssLeft]` counts): the tool's work on the square
    /// there, as commands. A right click is the caller's (it centres the
    /// view, 1020:1984). // 1020:1984, 1028:3dd0
    pub fn press(
        &mut self,
        x: i32,
        y: i32,
        shift: ShiftState,
        world: &World,
        tiles: &Tiles,
        cam: &Camera,
    ) -> Vec<Action> {
        let double = ShiftState::LEFT | ShiftState::DOUBLE;
        if self.tool == POINTER && (shift == ShiftState::LEFT || shift == double) {
            // The pointer's info box (1020:04bc): with [ssLeft] until the
            // button comes up, with [ssLeft, ssDouble] with its buttons
            // and open. The square under the mouse (`cursor_square`), and
            // the mouse in the main screen's form.
            self.pointer_double = shift == double; // 1110:64b8
            let main = crate::hauptschirm::LAYOUT[3].1; // Hauptbild, at 0, 59 with `big_map` too
            self.info_box = Some(infobox::Request::Show {
                square: darstellung::square_at_in(world, tiles, cam, &self.view, x, y), // 1018:3d8d
                mouse: (x + main.left, y + main.top),
                modal: self.pointer_double,
            });
            return Vec::new();
        }
        // BUG(original): only exactly [ssLeft]: with Shift or Ctrl held, or on a double click's second press, the tools do nothing.
        let shift = if self.enhance.has(Enhancement::ModifierClicks) {
            ShiftState(shift.without_keys().0 & !ShiftState::DOUBLE.0)
        } else {
            shift
        };
        if shift != ShiftState::LEFT {
            return Vec::new();
        }
        let at = darstellung::square_at_in(world, tiles, cam, &self.view, x, y); // 1018:3d8d
        match self.tool {
            RAISE | LOWER => {
                // 1018:3dad: the corner above the square 16 pixels up, by
                // the view's rotation.
                let (i, j) = darstellung::square_at_in(world, tiles, cam, &self.view, x, y - 16);
                let corner = match cam.view & 3 {
                    0 => (i, j),
                    1 => (i - 1, j),
                    2 => (i - 1, j - 1),
                    _ => (i, j - 1),
                };
                vec![Action::Level {
                    raise: self.tool == RAISE,
                    corner,
                    at: (i, j),
                }] // 1028:3b7c, 1028:3a68
            }
            DEMOLISH => vec![Action::Demolish(at)], // 1028:3c90
            ROAD => {
                // 1080:3195: the drag starts on the map only.
                self.square = at;
                if on_map(at) {
                    self.drag_start = at;
                    self.drag_end = at;
                    vec![Action::RoadDragStart(at)]
                } else {
                    Vec::new()
                }
            }
            t if is_menu(t) => match u8::try_from(self.menu.building) {
                // 1028:3cb1
                Ok(building) => vec![Action::Build { building, at }],
                // BUG(original): an empty submenu's type -1 is built as 255, past the tables; the port builds nothing.
                Err(_) => Vec::new(),
            },
            // 1028:3d93: the square under the mouse and the form's kind.
            SABOTAGE => vec![Action::Sabotage {
                kind: self.sabotage_kind(),
                at,
            }],
            // The pointer's `tool_click` does nothing; its info box is
            // above (1020:04bc).
            _ => Vec::new(),
        }
    }

    /// A button came up on `Hauptbild`: the road tool builds its road
    /// with the left one, if the drag is still on; the pointer's info box
    /// closes unless a double click opened it (any button). With
    /// `road_drag` the road ends at the square under the `mouse` (in
    /// `Hauptbild`), not where the last tick's picture had it.
    /// // 1020:1b31, 1080:31ed, 1020:050b
    #[must_use]
    pub fn release(
        &mut self,
        left: bool,
        world: &World,
        tiles: &Tiles,
        cam: &Camera,
        mouse: (i32, i32),
    ) -> Vec<Action> {
        if self.tool == POINTER && !self.pointer_double {
            self.info_box = Some(infobox::Request::Hide);
        }
        if self.tool == ROAD && left && world.road_dragging && self.enhance.has(Enhancement::RoadDrag) {
            // The original's end is the last tick's (`preview_squares`).
            self.square = darstellung::square_at_in(world, tiles, cam, &self.view, mouse.0, mouse.1); // 1018:3d8d
            self.drag_end = self.square;
        }
        if self.tool == ROAD && left {
            vec![Action::RoadDragEnd {
                from: self.drag_start,
                to: self.drag_end,
            }]
        } else {
            Vec::new()
        }
    }

    /// What the clock's tick does for the tools, with the animations and
    /// new people on: the square under the mouse (`mouse` in
    /// `Hauptbild`), the road drag's end, and the tool's pictures.
    /// // 1090:124f, 1090:0014
    ///
    /// The original keeps the pictures in the people's records 0 (the
    /// pointer) and 1 on (the squares), which are the world's; the port
    /// keeps them here. The caller runs it only with the animations and
    /// new people on, as the clock's tick does.
    pub fn preview(&mut self, world: &World, tiles: &Tiles, cam: &Camera, mouse: (i32, i32)) {
        // BUG(original): records past 0x23 belong to real people, and a road drag or building of more than 35 squares overwrites them (their square, offset, picture and kind 6); the port leaves the people alone.
        self.sprites.clear();
        let next_frame = |frame: &mut i16| {
            *frame += 1;
            if *frame > 7 {
                *frame = 0;
            }
            *frame
        };
        match self.tool {
            RAISE | LOWER => {
                self.square = darstellung::square_at_in(world, tiles, cam, &self.view, mouse.0, mouse.1 - 16); // 1018:3dad
                let (first, offset) = if self.tool == RAISE {
                    RAISE_POINTER
                } else {
                    LOWER_POINTER
                };
                let frame = next_frame(&mut self.frame);
                self.sprites.push(ToolSprite {
                    pos: self.square,
                    offset,
                    sprite: first + frame,
                    pointer: true,
                });
            }
            t if (DEMOLISH..POINTER).contains(&t) => {
                self.square = darstellung::square_at_in(world, tiles, cam, &self.view, mouse.0, mouse.1); // 1018:3d8d
                self.preview_squares(world, tiles, cam); // 1090:0014
                if t == DEMOLISH || t == SABOTAGE {
                    let (first, offset) = if t == DEMOLISH {
                        DEMOLISH_POINTER
                    } else {
                        SABOTAGE_POINTER
                    };
                    let frame = next_frame(&mut self.frame);
                    self.sprites.push(ToolSprite {
                        pos: self.square,
                        offset,
                        sprite: first + frame,
                        pointer: true,
                    });
                }
            }
            _ => {}
        }
    }

    /// A square of the tool's picture, as one that can or can't take it.
    fn square_sprite(&mut self, world: &World, tiles: &Tiles, cam: &Camera, pos: Pos, fits: bool) {
        let form = i16::from(darstellung::form_look(world, tiles, cam, pos)); // 1088:4bfa
        self.sprites.push(ToolSprite {
            pos,
            offset: (0, 0),
            sprite: if fits { SQUARE_FITS } else { SQUARE_DOESNT } + form,
            pointer: false,
        });
    }

    /// The squares the tool would take: the road being dragged (or the
    /// bridge from water) as far as it goes, else the square or building
    /// under the mouse, in the colour that says whether it can be built
    /// there (for the road the money for one square, for a building
    /// [`World::can_build`], player 1's). // 1090:0014
    fn preview_squares(&mut self, world: &World, tiles: &Tiles, cam: &Camera) {
        if self.tool == ROAD && world.road_dragging {
            // BUG(original): only this tick moves the drag's end: with the animations or new people off the road is one square, and it ends where the mouse was at the last tick.
            self.drag_end = self.square;
            let (from, to) = (self.drag_start, self.drag_end);
            let along_i = (to.1 - from.1).abs() < (to.0 - from.0).abs();
            let squares: Vec<Pos> = if along_i {
                line(from.0, to.0).map(|i| (i, from.1)).collect()
            } else {
                line(from.1, to.1).map(|j| (from.0, j)).collect()
            };
            let ground = world.road_drag_ground;
            for pos in squares {
                if ground == 2 || ground == 3 {
                    if !world.can_bridge(pos) {
                        break;
                    }
                    self.square_sprite(world, tiles, cam, pos, true);
                } else {
                    if !world.can_road(pos, ground, from) {
                        break;
                    }
                    if matches!(world.building_or_none(pos), NO_BUILDING | PALM) {
                        self.square_sprite(world, tiles, cam, pos, true);
                    }
                }
            }
            return;
        }
        let mut at = (self.square.0.clamp(0, 0xfa), self.square.1.clamp(0, 0xfa)); // 1110:6ff6
        let building_here = world.building_or_none(self.square);
        // BUG(original): without a building here the demolish tool shows the area of the one last found, which the people code (`1110:6ff2`) also sets; the port remembers only its own.
        let picks_building = self.tool == DEMOLISH || self.tool == SABOTAGE;
        if picks_building && building_here < NO_BUILDING && building_here != PALM {
            at = world.building_origin(self.square); // 1030:154c
            self.last_building = world.building_or_none(at);
        }
        let fits = match self.tool {
            DEMOLISH => true,
            SABOTAGE => world.sabotage_fits(self.sabotage_kind(), at),
            ROAD => {
                self.last_building = 0;
                let ground = world.island.ground.at(at);
                // Single human: the money is player 1's.
                !(!on_map(at)
                    || (!world.can_bridge(at) && !world.road_fits(at))
                    || world.money_of(1) < world.tables.road_cost
                    || ((0x37..=0x5e).contains(&ground) && !CROSSABLE.contains(&ground)))
            }
            _ => {
                let building = self.menu.building.to_le_bytes()[0];
                self.last_building = building;
                // Single human: player 1's money.
                world.can_build(at, building, 1).0 == 0 // 1030:04e3
            }
        };
        let b = self.last_building;
        let (w, d) = (world.tables.hausnr(b, 3), world.tables.hausnr(b, 4));
        for a in 0..w {
            for c in 0..d {
                if picks_building && FREE_SQUARES.contains(&b) {
                    return;
                }
                self.square_sprite(world, tiles, cam, (at.0 + a, at.1 + c), fits);
            }
        }
    }

    /// Draws the tool's pictures over the island picture `dst`. // 1090:155b
    pub fn draw_preview(&self, dst: &mut vcl::surface::Surface, world: &World, tiles: &Tiles, cam: &Camera) {
        for s in &self.sprites {
            darstellung::draw_tool_sprite(dst, world, tiles, cam, s);
        }
    }

    /// What the build menu's click or key means for the tools, and a key
    /// to pass on to the main screen. // 1018:04e7, 1018:3290
    #[allow(clippy::too_many_arguments)]
    pub fn menu_response(
        &mut self,
        response: Response,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
        screen: (i32, i32),
    ) -> (Vec<Action>, Option<u16>) {
        match response {
            Response::Nothing => {}
            Response::Close => {
                if self.menu.height > 0 || self.menu.open {
                    self.close_menu();
                }
                self.menu_focused = false;
            }
            Response::Cancel => {
                let button = self.menu.button;
                self.pointer_instead_of(button); // 1010:2bc4
                if self.menu.height > 0 || self.menu.open {
                    self.close_menu();
                }
                self.menu_focused = false;
            }
            Response::Button(b) => return (self.click(b, world, font, tiles, info, view, screen), None),
            Response::MainScreenKey(key) => return (Vec::new(), Some(key)),
            Response::FocusMainScreen => self.menu_focused = false,
        }
        (Vec::new(), None)
    }

    /// What a tick or command of the world means for the tools: the
    /// player's messages on the status bar, a submenu that lost its
    /// selected type, the pointer after a type there's only one of.
    #[allow(clippy::too_many_arguments)]
    pub fn world_changed(
        &mut self,
        tick: &Tick,
        player: u8,
        world: &World,
        font: &Font,
        tiles: &Tiles,
        info: &InfoTexts,
        view: u8,
        screen: (i32, i32),
    ) {
        for &(p, status) in &tick.status {
            if p == player
                && let Some(text) = self.status_text(status)
            {
                self.status = Some(text);
                self.sounds.push(0xc);
            }
        }
        for &menu in &tick.build_menu_removed {
            self.menu.removed(menu); // 1018:26b9
        }
        if tick.pointer_tool {
            let _ = self.click(POINTER, world, font, tiles, info, view, screen); // 1010:2c8b
        }
    }

    /// The status bar's text for `status`, none for a palm in the way.
    /// // 1030:232d, 1030:235a
    #[must_use]
    pub fn status_text(&self, status: Status) -> Option<Vec<u8>> {
        let error = |n: u8| {
            usize::from(n)
                .checked_sub(1)
                .and_then(|k| self.fehler.get(k))
                .cloned()
                .unwrap_or_default()
        };
        match status {
            Status::Error(n) => Some(error(n)),
            Status::InTheWay(b) if b >= 0x7f => Some(error(1)),
            Status::InTheWay(PALM) => None,
            Status::InTheWay(b) => {
                let mut text = self.menu.short_name(b);
                text.push(b' '); // 1030:2358
                text.extend_from_slice(&error(0x1e));
                text.truncate(255);
                Some(text)
            }
        }
    }
}

/// The road drag ends, if it's on (a command, as the computers wait on
/// it). // 1010:2b60
fn drag_cancel(world: &World) -> Vec<Action> {
    if world.road_dragging {
        vec![Action::RoadDragCancel]
    } else {
        Vec::new()
    }
}

/// The sabotage tool's form (`TSabowin`, [`crate::sabowin`]): shown and
/// hidden with the tool, and its buttons and keys change the tool.
impl Tools {
    /// The kind of sabotage the form has picked, 0 without the form.
    /// // 1110:65ba
    #[must_use]
    pub fn sabotage_kind(&self) -> u8 {
        self.sabowin.as_ref().map_or(0, |f| f.kind)
    }

    /// `TCustomForm.Show(sabowin)`, which takes the keyboard. // 1010:2c8b
    fn show_sabowin(&mut self) {
        if let Some(f) = &mut self.sabowin {
            let r = f.show(); // 1028:34a2
            self.sounds.extend(r.sounds);
            self.sabowin_focused = true;
        }
    }

    /// `TCustomForm.Hide(sabowin)`. // 1010:2be1, 1010:2c8b, 1028:35ee
    fn hide_sabowin(&mut self) {
        if let Some(f) = &mut self.sabowin {
            let r = f.hide();
            self.sounds.extend(r.sounds);
        }
        self.sabowin_focused = false;
    }

    /// What the form's event means for the tools: its sounds, the tool,
    /// and the form hidden, for the pointer (button 21 up, the pointer's
    /// down) or keeping the sabotage tool. The response goes back for the
    /// caller's refresh and move.
    fn sabowin_response(&mut self, r: sabowin::Response) -> sabowin::Response {
        self.sounds.extend(r.sounds.iter().copied());
        if r.sabotage_tool {
            self.tool = SABOTAGE; // 1028:2f4e, 1028:394c
        }
        match r.close {
            Some(Close::Hide) => self.hide_sabowin(),
            Some(Close::Pointer) => {
                self.hide_sabowin();
                self.pointer_instead_of(SABOTAGE); // 1010:2ade, 1010:2ac7
            }
            None => {}
        }
        r
    }

    /// `SaboimageMouseDown` at `x`, `y` in the form. // 1028:2d67
    pub fn sabowin_mouse_down(&mut self, x: i32, y: i32, left_button: bool) -> sabowin::Response {
        let r = self.sabowin.as_mut().map(|f| f.mouse_down(x, y, left_button));
        self.sabowin_response(r.unwrap_or_default())
    }

    /// `SaboimageMouseMove` at `x`, `y` in the form, on a `screen` of that
    /// size. // 1028:2ecc
    pub fn sabowin_mouse_move(&mut self, x: i32, y: i32, shift: ShiftState, screen: (i32, i32)) -> sabowin::Response {
        let r = self.sabowin.as_mut().map(|f| f.mouse_move(x, y, shift, screen));
        self.sabowin_response(r.unwrap_or_default())
    }

    /// `SaboimageMouseUp` at `x`, `y` in the form. // 1028:2f4e
    pub fn sabowin_mouse_up(&mut self, x: i32, y: i32, left_button: bool) -> sabowin::Response {
        let r = self.sabowin.as_mut().map(|f| f.mouse_up(x, y, left_button));
        self.sabowin_response(r.unwrap_or_default())
    }

    /// The form's `FormKeyDown`, while it has the keyboard. // 1028:394c
    pub fn sabowin_key_down(&mut self, key: u16, shift: ShiftState) -> sabowin::Response {
        let r = self.sabowin.as_mut().map(|f| f.key_down(key, shift));
        self.sabowin_response(r.unwrap_or_default())
    }
}
