//! The forms' windows a map file keeps (`docs/layout.md`, "Saved
//! windows"): `load_window_positions` (1010:08d7, en 1000:2cb6) puts
//! them back when a scenario or a saved game is loaded, from the start
//! menu or the file menu, and `save_window_positions` (1010:0799) writes
//! them. They're the view's: where a player's forms are, which of them
//! are open. The world isn't touched, but for the TV's pause, which the
//! TV's closing ends by command as ever; whether the money menu is open,
//! which the books follow, the world takes from the file itself
//! (`game::savegame::money_menu_shown`).
//!
//! For each form of `window_forms` (`1110:646c`) in turn, the file menu
//! and the two empty slots skipped, the original
//!
//! 1. sets `Top`, `Left`, `Width` and `Height` to the saved ones, but for
//!    the quit question, the compass and the end form
//!    ([`savegame::KEEP_PLACE`]);
//! 2. keeps the form on the screen it lays out for
//!    ([`savegame::keep_on_screen`]);
//! 3. shows it (`TCustomForm.Show`: `Visible`, `BringToFront`, and its
//!    `OnShow` if it was hidden) if the file has it shown, but never the
//!    TV or the buildings' overview, which it hides instead (1048:077a,
//!    1080:0e92 and 1080:1b24) if they're shown. A form the file has
//!    hidden stays as it is;
//! 4. sets `Enabled`, which the clock's first step (1010:2a0c) sets again
//!    for all of them: the port has no `Enabled`, the modal forms decide
//!    what takes the mouse.
//!
//! Most forms the original closes by their height, not `Visible`: the
//! build menu, the money menu, the tooltip, the info box, the compass and
//! the overview map are always shown and 0 high when closed, so their
//! saved height opens or closes them, with what the file's own units put
//! into them (the build menu's, the money menu's, the overview map's).

use game::enhance::Enhancement;
use game::hauptschirm::{self, Hauptschirm};
use game::savegame::{self, Fields, KEEP_PLACE, NEVER_SHOWN, Window, Windows, window};
use vcl::Rect;

use super::Game;
use crate::stacking::Win;

/// The window of `window_forms`' slot `form`, none for the empty slots.
/// // 1110:646c
fn win(form: usize) -> Option<Win> {
    Some(match form {
        window::AUCTION => Win::Auction,
        window::BUILD_MENU => Win::Menu,
        window::FILE_MENU => Win::FileMenu,
        window::QUIT_QUESTION => Win::Ende,
        window::MONEY_MENU => Win::Money,
        window::TOOLTIP => Win::Help,
        window::HIGH_SCORES => Win::High,
        window::INFO_BOX => Win::Infobox,
        window::COMPASS => Win::Compass,
        window::TV => Win::Tv,
        window::SABOTAGE => Win::Sabowin,
        window::MAIN_SCREEN => Win::Main,
        window::END_FORM => Win::Schluss,
        window::BUILDINGS_OVERVIEW => Win::Umenu,
        window::OVERVIEW_MAP => Win::Overview,
        _ => return None,
    })
}

/// The forms the original closes by their height, always shown
/// (`Visible`): saved, they have `Visible` set and their height 0 while
/// closed. The main screen is shown at its full height.
const BY_HEIGHT: [usize; 7] = [
    window::BUILD_MENU,
    window::MONEY_MENU,
    window::TOOLTIP,
    window::INFO_BOX,
    window::COMPASS,
    window::MAIN_SCREEN,
    window::OVERVIEW_MAP,
];

/// Where a game is loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Loading {
    /// The start menu, a scenario or a saved game (states 1000–1500):
    /// `hauptschirm_show` (1020:283b) gives the main screen its full
    /// height after.
    Start,
    /// The file menu's "Laden" (1040:0f27), over the running game.
    FileMenu,
}

impl Game {
    /// The form `form`'s top left, if it's one the port has.
    fn window_place(&mut self, form: usize) -> Option<(&mut i32, &mut i32)> {
        let f = &mut self.forms;
        Some(match form {
            window::AUCTION => (&mut self.auction.left, &mut self.auction.top),
            window::BUILD_MENU => (&mut self.tools.menu.left, &mut self.tools.menu.top),
            window::QUIT_QUESTION => (&mut f.ende.left, &mut f.ende.top),
            window::MONEY_MENU => (&mut f.money.left, &mut f.money.top),
            window::TOOLTIP => (&mut f.help.left, &mut f.help.top),
            window::HIGH_SCORES => (&mut f.high.left, &mut f.high.top),
            window::INFO_BOX => (&mut f.infobox.left, &mut f.infobox.top),
            window::COMPASS => (&mut f.compass.left, &mut f.compass.top),
            window::TV => (&mut f.tv.left, &mut f.tv.top),
            window::SABOTAGE => {
                let s = self.tools.sabowin.as_mut()?;
                (&mut s.left, &mut s.top)
            }
            window::MAIN_SCREEN => (&mut self.screen.left, &mut self.screen.top),
            window::END_FORM => (&mut f.schluss.left, &mut f.schluss.top),
            window::BUILDINGS_OVERVIEW => (&mut f.umenu.left, &mut f.umenu.top),
            window::OVERVIEW_MAP => (&mut f.overview.left, &mut f.overview.top),
            _ => return None,
        })
    }

    /// The form `form` on the screen as it is now, and whether it's
    /// shown.
    fn window_now(&self, form: usize) -> Option<(Rect, bool)> {
        let f = &self.forms;
        let w = win(form)?;
        let bounds = match form {
            window::AUCTION => self.auction.bounds(),
            window::BUILD_MENU => self.tools.menu.bounds(),
            window::FILE_MENU => return None,
            window::QUIT_QUESTION => f.ende.bounds(),
            window::MONEY_MENU => f.money.bounds(),
            window::TOOLTIP => f.help.bounds(),
            window::HIGH_SCORES => f.high.bounds(),
            window::INFO_BOX => f.infobox.bounds(),
            window::COMPASS => f.compass.bounds(),
            window::TV => f.tv.bounds(),
            window::SABOTAGE => self.tools.sabowin.as_ref()?.bounds(),
            window::MAIN_SCREEN => self.screen.bounds(),
            window::END_FORM => f.schluss.bounds(),
            window::BUILDINGS_OVERVIEW => f.umenu.bounds(),
            _ => f.overview.bounds(),
        };
        Some((bounds, self.shown(w)))
    }

    /// The main screen fills the screen (`big_map`): it isn't where a
    /// file can put it.
    fn main_fills(&self) -> bool {
        self.screen.size() != (hauptschirm::WIDTH, hauptschirm::HEIGHT)
    }

    /// `load_window_positions` for the view's forms, from the windows of
    /// the file the game was loaded from (`view_rest`), on the screen the
    /// game lays out for; a file without them changes nothing. The forms
    /// it showed, in its order, for [`Game::stack_loaded`].
    /// // 1010:08d7, en 1000:2cb6
    ///
    /// With `saved_windows` the main screen stays where the port put it,
    /// and the others keep their places over it, as resizing moves them,
    /// whole on the screen. With `loaded_forms` the end form, the info
    /// box and the tooltip aren't shown again. With `loaded_pause` the TV
    /// closes, shown or not, which ends its pause.
    #[allow(clippy::too_many_lines)] // One form after the other, as the original.
    pub(crate) fn load_windows(&mut self, loading: Loading) -> Vec<Win> {
        let Some(saved) = Windows::from_fields(&self.view_rest) else {
            return Vec::new();
        };
        let size = self.size;
        let enhance = self.screen.enhance;
        let fit = enhance.has(Enhancement::SavedWindows);
        let ghosts = !enhance.has(Enhancement::LoadedForms);
        // How far the saved main screen is off the centred one, which
        // `saved_windows` moves the others by.
        let main = saved.0[window::MAIN_SCREEN];
        let centred = Hauptschirm::position(size.0, size.1);
        let delta = (centred.0 - i32::from(main.left), centred.1 - i32::from(main.top));
        let main_fills = self.main_fills();
        let mut shown = Vec::new();
        for (form, &w) in saved.0.iter().enumerate() {
            let Some(which) = win(form).filter(|&w| w != Win::FileMenu) else {
                continue;
            };
            // 1. and 2.: the saved place, kept on the screen; the forms of
            // `KEEP_PLACE` keep theirs, and are kept on the screen too.
            let keep = KEEP_PLACE.contains(&form);
            let (bounds, moved) = if keep {
                let Some((now, _)) = self.window_now(form) else {
                    continue;
                };
                (now, (0, 0))
            } else {
                (w.rect(), if fit { delta } else { (0, 0) })
            };
            let placed = Rect::new(
                bounds.left + moved.0,
                bounds.top + moved.1,
                bounds.width(),
                bounds.height(),
            );
            let (mut left, mut top) = savegame::keep_on_screen(placed, size);
            // BUG(original): a map file's windows go where they were on the screen it was saved on, the main screen too, and nothing keeps them off the left or top edge.
            if fit {
                (left, top) = (left.max(0), top.max(0));
            }
            let main_stays = form == window::MAIN_SCREEN && (fit || main_fills);
            if !main_stays && let Some((l, t)) = self.window_place(form) {
                (*l, *t) = (left, top);
            }
            if !keep {
                self.saved_size(form, w, loading, main_fills);
            }
            // 3.: shown, or for the TV and the buildings' overview hidden.
            if form == window::TV {
                // BUG(original): a game saved with the TV open loads paused (1110:0067) without its TV, until it's opened and closed again.
                if self.forms.tv.visible || enhance.has(Enhancement::LoadedPause) {
                    let r = self.forms.tv.close(); // 1048:077a
                    self.tv_response(r);
                }
            } else if form == window::BUILDINGS_OVERVIEW {
                if self.forms.umenu.visible {
                    // 1080:0e92, 1080:1b24: hidden and its lists freed;
                    // opening it sets it up again.
                    let r = self.forms.umenu.hide();
                    self.umenu_response(r);
                }
            } else if w.shown() && !NEVER_SHOWN.contains(&form) {
                self.show_saved(form, w, ghosts);
                shown.push(which);
            }
        }
        shown
    }

    /// The height the file gives form `form`: the forms closed by their
    /// height open or close by it, the main screen keeps it only when
    /// loaded from the file menu (rolled up to its title bar if it was).
    /// The others are sized when they're shown. // 1010:08d7
    fn saved_size(&mut self, form: usize, w: Window, loading: Loading, main_fills: bool) {
        let height = i32::from(w.height).max(0);
        let f = &mut self.forms;
        match form {
            window::BUILD_MENU => self.tools.menu.height = height,
            window::MONEY_MENU => f.money.height = height,
            window::OVERVIEW_MAP => {
                // `game_start` (10b8:3fab) gives a shown one its size.
                f.overview.width = i32::from(w.width);
                f.overview.height = height;
            }
            window::MAIN_SCREEN if loading == Loading::FileMenu && !main_fills => {
                // The port keeps its title bar at least.
                self.screen.height = height.clamp(hauptschirm::TITLE_HEIGHT, hauptschirm::HEIGHT);
            }
            // A tooltip and an info box closed by the file; one it has
            // open is shown ([`Game::show_saved`]).
            window::TOOLTIP if height == 0 => {
                f.help.hide();
            }
            window::INFO_BOX if height == 0 && f.infobox.visible => {
                let r = f.infobox.hide();
                self.infobox_response(r);
            }
            _ => {}
        }
    }

    /// `TCustomForm.Show` of form `form`, which the file `w` has shown.
    /// The forms closed by their height have it from the file; the
    /// sabotage window's `FormShow` draws it. The end form, an open info
    /// box and tooltip show what they last drew (`ghosts`). The auction,
    /// the quit question and the high scores can't be saved shown: their
    /// forms disable the file menu's button, and `Last.map` is saved
    /// after the quit question closes. // 1010:0a82
    fn show_saved(&mut self, form: usize, w: Window, ghosts: bool) {
        let f = &mut self.forms;
        match form {
            window::SABOTAGE => {
                self.tools.show_sabowin(); // 1028:34a2
                self.show_tools();
            }
            // BUG(original): the end form, the info box and the tooltip a file has shown come back as they were last drawn, another game's or empty.
            window::END_FORM if ghosts && !f.schluss.visible() => f.schluss.show_saved(),
            window::INFO_BOX if w.height > 0 => {
                if ghosts {
                    f.infobox.visible = true;
                    f.infobox.width = i32::from(w.width);
                    f.infobox.height = i32::from(w.height);
                } else if f.infobox.visible {
                    let r = f.infobox.hide();
                    self.infobox_response(r);
                }
            }
            window::TOOLTIP if w.height > 0 => {
                if ghosts {
                    f.help.height = i32::from(w.height);
                } else {
                    f.help.hide();
                }
            }
            _ => {}
        }
    }

    /// A scenario's or a saved game's start (state 1500), once the view's
    /// enhancements are set: `load_map`'s windows (1010:08d7), then what
    /// the file's units and `game_start_setup` (1010:2820) do with the
    /// forms they opened or closed: the overview map's fields and layout
    /// (10b8:50de, 10b8:3fab), the TV's (1048:25ea), the money menu's and
    /// its page (1070:3bf8, 1070:2630); then the order.
    pub(crate) fn start_from_file(&mut self) {
        let shown = self.load_windows(Loading::Start);
        let rest = self.view_rest.clone();
        let f = &mut self.forms;
        f.overview.load(&rest);
        f.overview.game_start(&self.world);
        f.tv.load_fields(&rest);
        f.money.load_fields(&rest);
        let _ = f.money.loaded(&self.world);
        self.stack_loaded(&shown);
    }

    /// The order after a load: the forms `load_windows` showed raised in
    /// its order (`Show`'s `BringToFront`), then the main screen
    /// (`game_show_state`, 1010:2819, and `game_start_setup`, en
    /// 1010:39be). With `window_stacking` nothing is raised: what opened
    /// goes on top.
    pub(crate) fn stack_loaded(&mut self, shown: &[Win]) {
        self.sync_stack();
        if !self.stack.enhanced {
            let raised: Vec<Win> = shown.iter().copied().filter(|&w| self.shown(w)).collect();
            for w in raised {
                self.stack.raise(w);
            }
        }
        self.stack.loaded();
    }

    /// `save_window_positions` (1010:0799): each form's window into the
    /// view's `fields` for `save_map`. A form the port has in other terms
    /// keeps what the file the game came from had (its size while it's
    /// hidden); a new game's are the forms' own. With `big_map` the main
    /// screen is saved as the original's, centred, so that the original
    /// can load the file.
    pub(crate) fn save_windows(&self, fields: &mut Fields) {
        let loaded = Windows::from_fields(fields);
        let mut out = loaded.unwrap_or_default();
        // The form that disabled the others (`disable_forms`, 1010:2970):
        // the file menu while it saves, the quit question, the end form,
        // the high scores, an info box with buttons; the pause and the
        // options aren't among the forms.
        let f = &self.forms;
        let disabler = if self.file_menu.visible {
            Some(window::FILE_MENU)
        } else if f.ende.visible {
            Some(window::QUIT_QUESTION)
        } else if f.high.visible {
            Some(window::HIGH_SCORES)
        } else if f.schluss.visible() {
            Some(window::END_FORM)
        } else if f.infobox.visible && f.infobox.modal {
            Some(window::INFO_BOX)
        } else if f.modal() {
            Some(usize::MAX)
        } else {
            None
        };
        for (form, w) in out.0.iter_mut().enumerate() {
            let Some((mut bounds, shown)) = self.window_now(form) else {
                continue;
            };
            if form == window::MAIN_SCREEN && self.main_fills() {
                let (left, top) = Hauptschirm::position(self.size.0, self.size.1);
                bounds = Rect::new(left, top, hauptschirm::WIDTH, hauptschirm::HEIGHT);
            }
            let by_height = BY_HEIGHT.contains(&form);
            let short = |v: i32| i16::try_from(v).unwrap_or(if v < 0 { i16::MIN } else { i16::MAX });
            if loaded.is_none() {
                (w.width, w.height) = (short(bounds.width()), short(bounds.height()));
                w.visible = 1;
            }
            (w.left, w.top) = (short(bounds.left), short(bounds.top));
            if shown {
                (w.width, w.height) = (short(bounds.width()), short(bounds.height()));
            } else if by_height {
                w.height = 0;
            }
            if !by_height {
                w.visible = u8::from(shown);
            }
            w.enabled = u8::from(disabler.is_none_or(|d| d == form));
        }
        out.store(fields);
        if fields.get("1110:645a", 0).is_none() {
            // The forms' `Enabled` before the file menu disabled them
            // (1010:2970), all of them (the clock's first step enables
            // them), but the empty slots.
            let mut before = [1u8; 17];
            before[10] = 0;
            before[11] = 0;
            fields.set("1110:645a", 0, &before);
        }
    }
}
