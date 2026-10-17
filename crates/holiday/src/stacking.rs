//! Which window lies over which (`docs/layout.md`, "Stacking").
//!
//! In the original every form is a top-level window of its own, and
//! Windows stacks them. Every form is owned by the application's hidden
//! window, the background (`THintergrund`) too, so they are siblings,
//! with two tiers: the forms whose DFM has `FormStyle = fsStayOnTop` lie
//! over all the others. Within a tier the last one raised is on top, and
//! a form is raised when it's shown (`ShowWindow` activates it;
//! `TCustomForm.Show`, 10f0:56e1, also calls `BringToFront`, 10e8:201c),
//! when a press activates it, and when the game calls `BringToFront` or
//! gives it the focus. The game raises the main screen itself when a form
//! closes, and its background's `OnActivate` raises every form again
//! ([`Stack::background_pressed`]).
//!
//! [`Stack`] keeps that order. With the `window_stacking` enhancement
//! there's one tier for the non-modal forms, the main screen among them,
//! and the modal forms and the tooltip over it; nothing is raised but by
//! a press, a form opening or a form's own "focus the main screen" key.

use game::enhance::{Enhancement, Enhancements};

/// A window of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Win {
    /// `THintergrund`, the desktop behind everything.
    Background,
    /// `THauptschirm`.
    Main,
    /// `TBaumenue`.
    Menu,
    Auction,
    Money,
    Overview,
    Sabowin,
    Umenu,
    Infobox,
    Tv,
    Compass,
    Pause,
    Options,
    /// The quit question, `TEndeform`.
    Ende,
    /// The end of the game, `TSchlussform`.
    Schluss,
    High,
    /// The tooltip, `THelpform`.
    Help,
    /// `TDateimenue`.
    FileMenu,
}

/// Every window, where it starts, bottom first. Shown forms are raised,
/// so this only orders the ones a game starts with.
pub(crate) const ALL: [Win; 18] = [
    Win::Background,
    Win::Main,
    Win::Menu,
    Win::Auction,
    Win::Money,
    Win::Overview,
    Win::Sabowin,
    Win::Umenu,
    Win::Infobox,
    Win::Tv,
    Win::Compass,
    Win::Pause,
    Win::Options,
    Win::Ende,
    Win::Schluss,
    Win::High,
    Win::Help,
    Win::FileMenu,
];

/// `window_forms` (`1110:646c`) in its order, without the empty slots 10
/// and 11: what 1010:2a5a raises one after the other.
const WINDOW_FORMS: [Win; 15] = [
    Win::Auction,
    Win::Menu,
    Win::FileMenu,
    Win::Ende,
    Win::Money,
    Win::Help,
    Win::High,
    Win::Infobox,
    Win::Compass,
    Win::Tv,
    Win::Sabowin,
    Win::Main,
    Win::Schluss,
    Win::Umenu,
    Win::Overview,
];

/// The forms whose closing raises the main screen in the original:
/// `BringToFront` of the main screen in 1070:2bd7 (money menu), 10b8:3fa2
/// (overview map), 1018:2c96 (build menu) and 1038:1f0a (info box),
/// `Show` of it in 1048:085a (TV) and 1028:056f (auction), and the focus
/// (VMT+0x78) after the pause (1040:3bac) and the options (1060:0e80),
/// and `BringToFront` after the end form's "Weiterspielen" (1068:0194).
const CLOSING_RAISES_MAIN: [Win; 9] = [
    Win::Money,
    Win::Overview,
    Win::Menu,
    Win::Infobox,
    Win::Tv,
    Win::Auction,
    Win::Pause,
    Win::Options,
    Win::Schluss,
];

impl Win {
    /// `FormStyle = fsStayOnTop` in the form's DFM.
    fn stay_on_top(self) -> bool {
        matches!(
            self,
            Win::Auction
                | Win::Infobox
                | Win::FileMenu
                | Win::Ende
                | Win::Compass
                | Win::Tv
                | Win::Options
                | Win::High
                | Win::Pause
                | Win::Help
        )
    }

    /// Disables the other forms while it's shown (`disable_forms`,
    /// 1010:2970, and the file menu, 1040:12be).
    fn modal(self) -> bool {
        matches!(
            self,
            Win::Pause | Win::Options | Win::Ende | Win::Schluss | Win::High | Win::FileMenu
        )
    }
}

/// The windows from the bottom up.
#[derive(Debug, Clone)]
pub(crate) struct Stack {
    order: Vec<Win>,
    /// The windows shown at the last [`Stack::sync`].
    shown: Vec<Win>,
    /// The `window_stacking` enhancement.
    pub enhanced: bool,
    /// The order before the press the mouse is in, until it comes up.
    pub before_press: Option<Vec<Win>>,
    /// `big_map`'s main screen fills the screen: it lies right over the
    /// background, under every other window, like a desktop, in either
    /// stacking. Raised over the others it would hide them all.
    pub main_desktop: bool,
}

impl Stack {
    pub fn new(enhancements: Enhancements) -> Stack {
        Stack {
            order: ALL.to_vec(),
            shown: Vec::new(),
            before_press: None,
            enhanced: enhancements.has(Enhancement::WindowStacking),
            main_desktop: false,
        }
    }

    /// `w` goes to the top of its tier.
    pub fn raise(&mut self, w: Win) {
        self.order.retain(|&o| o != w);
        self.order.push(w);
    }

    /// The tier `w` is in: a higher tier lies over a lower one.
    fn tier(&self, w: Win, infobox_modal: bool) -> u8 {
        if self.main_desktop {
            return match w {
                Win::Background => 0,
                Win::Main => 1,
                w => 2 + self.form_tier(w, infobox_modal),
            };
        }
        self.form_tier(w, infobox_modal)
    }

    /// [`Stack::tier`] without `big_map`'s desktop.
    fn form_tier(&self, w: Win, infobox_modal: bool) -> u8 {
        if self.enhanced {
            match w {
                Win::Background => 0,
                Win::Help => 3,
                Win::Infobox if infobox_modal => 2,
                w if w.modal() => 2,
                _ => 1,
            }
        } else {
            // BUG(original): the fsStayOnTop forms (the TV, compass, info box, auction, ...) always lie over the others, the main screen, build menu, money menu and end form among them, wherever they're clicked.
            u8::from(w.stay_on_top())
        }
    }

    /// Every window, the bottom one first. `infobox_modal`: the info box
    /// has buttons and disables the others.
    pub fn order(&self, infobox_modal: bool) -> Vec<Win> {
        let mut order = self.order.clone();
        order.sort_by_key(|&w| self.tier(w, infobox_modal));
        order
    }

    /// Catches up with which windows are shown now: those that opened
    /// since go on top, in the order they lay. In the original a form
    /// that closed may raise the main screen ([`CLOSING_RAISES_MAIN`]).
    pub fn sync(&mut self, shown: impl Fn(Win) -> bool) {
        let now: Vec<Win> = self.order.iter().copied().filter(|&w| shown(w)).collect();
        let closed = self.shown.iter().filter(|w| !now.contains(w));
        if !self.enhanced && closed.into_iter().any(|w| CLOSING_RAISES_MAIN.contains(w)) {
            // BUG(original): closing the money menu, overview map, build menu, info box, TV, auction, pause, options or end form raises the main screen over the other forms of its tier, which then lie under it.
            self.raise(Win::Main);
        }
        for &w in &now {
            if !self.shown.contains(&w) {
                self.raise(w);
            }
        }
        self.shown = now;
    }

    /// Whether `w` was shown at the last [`Stack::sync`].
    pub fn was_shown(&self, w: Win) -> bool {
        self.shown.contains(&w)
    }

    /// A loaded game is shown (`game_show_state`, 1010:26fa, ends with
    /// `BringToFront` of the main screen, 1010:2819).
    pub fn loaded(&mut self) {
        if !self.enhanced {
            // BUG(original): loading a game from the file menu raises the main screen over the money menu, build menu and the other forms of its tier.
            self.raise(Win::Main);
        }
    }

    /// A press on the background, where no form is. In the original that
    /// activates `THintergrund`, which goes over the forms of its tier;
    /// its `FormActivate` (1000:2241), once the main screen has been
    /// shown (`1110:00a0`, 1020:283e), raises every form of
    /// `window_forms` in turn and the main screen last (1010:2a5a), unless
    /// the info box is shown (`infobox2+0x1e6`) or the end form is.
    pub fn background_pressed(&mut self, in_game: bool, infobox_shown: bool, schluss_shown: bool) {
        if self.enhanced {
            return;
        }
        // BUG(original): a press on the background raises it over the main screen and the other forms that aren't fsStayOnTop; the game raises them back in `window_forms`' order (the main screen on top), but not while the info box or the end form is shown, which leaves them hidden behind the background.
        self.raise(Win::Background);
        if in_game && !infobox_shown && !schluss_shown {
            for w in WINDOW_FORMS {
                self.raise(w);
            }
            self.raise(Win::Main);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(enhanced: bool) -> Stack {
        let mut e = Enhancements::ORIGINAL;
        e.set(Enhancement::WindowStacking, enhanced);
        Stack::new(e)
    }

    fn above(s: &Stack, a: Win, b: Win) -> bool {
        let o = s.order(false);
        o.iter().position(|&w| w == a) > o.iter().position(|&w| w == b)
    }

    #[test]
    fn original_tiers() {
        let mut s = stack(false);
        let shown = [Win::Background, Win::Main, Win::Money, Win::Tv];
        s.sync(|w| shown.contains(&w));
        // A press on the main screen raises it over the money menu, not
        // over the TV.
        s.raise(Win::Main);
        assert!(above(&s, Win::Main, Win::Money));
        assert!(above(&s, Win::Tv, Win::Main));
        s.raise(Win::Money);
        assert!(above(&s, Win::Money, Win::Main));
        // Closing the TV raises the main screen again.
        s.sync(|w| shown[..3].contains(&w));
        assert!(above(&s, Win::Main, Win::Money));
        // The background over everything but the fsStayOnTop forms while
        // the info box is shown.
        s.background_pressed(true, true, false);
        assert!(above(&s, Win::Background, Win::Main) && above(&s, Win::Background, Win::Money));
        assert!(above(&s, Win::Infobox, Win::Background));
        // And otherwise the main screen on top again.
        s.background_pressed(true, false, false);
        assert!(above(&s, Win::Main, Win::Money) && above(&s, Win::Money, Win::Background));
        assert!(above(&s, Win::Overview, Win::Money));
    }

    #[test]
    fn enhanced_stacking() {
        let mut s = stack(true);
        let shown = [Win::Background, Win::Main, Win::Money, Win::Tv];
        s.sync(|w| shown.contains(&w));
        assert!(above(&s, Win::Tv, Win::Money) && above(&s, Win::Money, Win::Main));
        // The main screen goes over the TV.
        s.raise(Win::Main);
        assert!(above(&s, Win::Main, Win::Tv));
        // Closing a form leaves the others as they are.
        s.raise(Win::Money);
        s.sync(|w| shown[..3].contains(&w));
        assert!(above(&s, Win::Money, Win::Main));
        // A new form opens on top; the background stays at the bottom.
        s.sync(|w| shown[..3].contains(&w) || w == Win::Overview);
        assert!(above(&s, Win::Overview, Win::Money) && above(&s, Win::Overview, Win::Main));
        s.background_pressed(true, false, false);
        assert_eq!(s.order(false)[0], Win::Background);
        // Modal forms stay on top.
        s.sync(|w| shown.contains(&w) || w == Win::Pause);
        s.raise(Win::Main);
        assert!(above(&s, Win::Pause, Win::Main));
        assert!(
            s.order(true).iter().position(|&w| w == Win::Infobox) > s.order(true).iter().position(|&w| w == Win::Main)
        );
    }

    #[test]
    fn big_map_desktop() {
        for enhanced in [false, true] {
            let mut s = stack(enhanced);
            s.main_desktop = true;
            let shown = [Win::Background, Win::Main, Win::Money, Win::Tv];
            s.sync(|w| shown.contains(&w));
            // A press on the main screen, a form closing or a press on
            // the background leave it under the forms, over the
            // background.
            s.raise(Win::Main);
            s.sync(|w| shown[..3].contains(&w));
            s.background_pressed(true, false, false);
            assert_eq!(s.order(false)[..2], [Win::Background, Win::Main]);
            assert!(above(&s, Win::Money, Win::Main));
        }
    }
}
