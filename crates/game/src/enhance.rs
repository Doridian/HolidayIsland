//! Enhancements: fixes of the original's bugs and port extensions, each
//! a switch with a stable key, on by default (`docs/original-bugs.md`,
//! "Options"). With every switch off the port does what the original
//! does.
//!
//! There are two kinds:
//!
//! - **World fixes** ([`Fix`], a set of them is [`Fixes`]) change the
//!   simulation. They are the world's ([`World::fixes`]): every player
//!   has the same, they are in its hash, a saved game keeps them (the
//!   port's chunk after the original's fields, [`fixes_in`],
//!   [`world_chunk`]) and a player changes one by command
//!   ([`Action::SetFix`]).
//! - **View enhancements** ([`Enhancement`], a set is [`Enhancements`])
//!   change one player's screen, sound or input. They are that player's
//!   settings (the port's `settings.ini`).
//!
//! [`Config::resolve`] works out both from the master switch
//! (`--original`, `original=true`) and per-key overrides
//! (`enhance.<key>=on|off`, `--enhance=<key>=on|off`). The world's fixes
//! it gives are the player's defaults: a new game starts with them, and
//! a game from a file without the port's chunk (any original file)
//! takes them.
//!
//! Adding one: a line in [`Fix`]'s or [`Enhancement`]'s list below (its
//! key), its label and description in every language's `strings.toml`
//! (`[enhance.<key>]`, `lang/README.md`), then
//! where the code branches on it, keep the original's path visible and
//! marked `// BUG(original): ...`, and name the key in the bug's row of
//! `docs/original-bugs.md`.
//!
//! [`World::fixes`]: crate::world::World::fixes
//! [`Action::SetFix`]: crate::command::Action::SetFix

use crate::lang::{Language, Setting};

/// Whose an enhancement is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The world's: changes the simulation, the same for every player.
    World,
    /// One player's view.
    View,
}

/// What the lists below say about an enhancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    /// The stable key (`enhance.<key>` in `settings.ini`, `--enhance=`,
    /// `fix.<key>` in a saved game). Never changed once released.
    pub key: &'static str,
    pub side: Side,
}

impl Info {
    /// The short name for the in-game list (`crate::enhanceform`) in
    /// `language`: a few words, as plain as they can be
    /// (`enhance.<key>.label` in `strings.toml`).
    #[must_use]
    pub fn label(&self, language: Language) -> &'static str {
        language.get(&format!("enhance.{}.label", self.key)).unwrap_or(self.key)
    }

    /// One line for a list of them in `language`
    /// (`enhance.<key>.description`).
    #[must_use]
    pub fn description(&self, language: Language) -> &'static str {
        language
            .get(&format!("enhance.{}.description", self.key))
            .unwrap_or(self.key)
    }
}

/// Defines a kind of enhancement: the enum `$one` with a variant per
/// line, its `ALL` and `info`, and `$set`, a set of them as bits (up to
/// 64), with `ORIGINAL` (all off) and `ALL_ON`.
macro_rules! enhancements {
    ($(#[$m:meta])* $one:ident, $(#[$sm:meta])* $set:ident, $side:expr;
     $( $(#[$vm:meta])* $variant:ident = $key:literal; )*) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $one {
            $( $(#[$vm])* $variant, )*
        }

        impl $one {
            /// Every one, in the list's order.
            pub const ALL: &'static [$one] = &[$($one::$variant),*];

            #[must_use]
            pub fn info(self) -> Info {
                match self {
                    $( $one::$variant => Info { key: $key, side: $side }, )*
                }
            }

            /// The one with `key`.
            #[must_use]
            pub fn from_key(key: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|e| e.info().key == key)
            }

            fn bit(self) -> u64 {
                1 << Self::ALL.iter().position(|&e| e == self).unwrap_or(0)
            }
        }

        $(#[$sm])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $set(u64);

        impl $set {
            /// All off: the original.
            pub const ORIGINAL: $set = $set(0);

            /// All on, the default.
            #[must_use]
            pub fn all_on() -> $set {
                $set($one::ALL.iter().fold(0, |bits, e| bits | e.bit()))
            }

            /// Whether `e` is on.
            #[must_use]
            pub fn has(self, e: $one) -> bool {
                self.0 & e.bit() != 0
            }

            pub fn set(&mut self, e: $one, on: bool) {
                if on {
                    self.0 |= e.bit();
                } else {
                    self.0 &= !e.bit();
                }
            }

            /// The ones that are on.
            pub fn iter(self) -> impl Iterator<Item = $one> {
                $one::ALL.iter().copied().filter(move |&e| self.has(e))
            }
        }
    };
}

enhancements! {
    /// A fix that changes the simulation (`docs/original-bugs.md`,
    /// "Simulation logic").
    Fix,
    /// The world's fixes that are on ([`World::fixes`]). The default is
    /// [`Fixes::ORIGINAL`], so a `World` built in a test is the original's
    /// unless it asks for a fix.
    ///
    /// [`World::fixes`]: crate::world::World::fixes
    Fixes,
    Side::World;
    /// Demolishing a building site takes its effect off the location map
    /// (1078:280b).
    SiteEffect = "site_effect";
    /// Demolishing a bridge over the sea costs `Strassenabrisskosten` a
    /// square, which the original adds up and never charges (1078:276f).
    BridgeDemolitionCost = "bridge_demolition_cost";
    /// A facility's price class divides by the price range, not by
    /// `MaxPreis` (1008:0907).
    PriceClassRange = "price_class_range";
    /// Buildings of nobody (owner 0) earn and cost nobody: their takings,
    /// costs and guests no longer go into player 8's slots of the month's
    /// sums and interest (settlement, 1078:0172).
    OwnerlessBuildings = "ownerless_buildings";
    /// A palm that dies or that moving the land clears comes off its
    /// plot's count, and a cleared one off the location map, its owner
    /// and part (1010:129e, 10b0:069f, 10b0:1907).
    PalmRemoval = "palm_removal";
    /// A computer looking for another area checks the area of the plot
    /// it moves to, not the one it leaves (10a8:21de).
    ComputerAreaChange = "computer_area_change";
    /// The computers' type lists keep to their 40 places a category
    /// (10a8:388b, 10a8:0523).
    ComputerTypeLists = "computer_type_lists";
    /// A blocked harbour ship's second detour checks the squares ahead,
    /// not its own square five times (1098:04f9).
    ShipDetour = "ship_detour";
    /// The stage spectators are looked for where they stand, so a stage
    /// doesn't get new ones every time (1090:2373).
    StageSpectators = "stage_spectators";
    /// A plane's second approach needs both coordinates to match
    /// (1090:2c1f).
    PlaneApproach = "plane_approach";
    /// A corner rising from the sea between plots merges them by their
    /// whole numbers, not cut to a byte (1078:09f6).
    PlotMergeNumbers = "plot_merge_numbers";
    /// The computers' roads start from their own start square's ground,
    /// not the human's last road drag's (1078:2396).
    ComputerRoadGround = "computer_road_ground";
    /// Moving the land takes neighbours along that are two steps or more
    /// off, so cliffs don't get steeper (10b0:03fa, 10b0:12c7).
    LevellingCliffs = "levelling_cliffs";
    /// Raising and lowering the land report the first error: lowering
    /// stops looking at buildings after one, and the money check doesn't
    /// replace it (10b0:069f, 10b0:12c7).
    LevellingErrors = "levelling_errors";
    /// After a road is built, every building near it gets its connection
    /// again, by its first square, whatever its look (1078:2178).
    RoadConnections = "road_connections";
    /// The info box's buttons (renovate, prices, sell, bribe) are only
    /// for the player's own squares, not nobody's (1038:1a75).
    NobodysBuildings = "nobodys_buildings";
    /// A computer bids only when its bid (5 % more) is within its limit
    /// (1028:1973).
    AuctionComputerLimit = "auction_computer_limit";
    /// Passing at an auction leaves a leading bid standing and doesn't
    /// give the item away a second time after the third call (1028:0c82).
    AuctionPass = "auction_pass";
    /// Player 8 gets the building type it wins at an auction
    /// (`make_available`, 10a0:0691).
    PrizePlayer8 = "prize_player8";
    /// A computer's highest bid is worked out without overflowing
    /// (10a8:0167).
    ComputerBidOverflow = "computer_bid_overflow";
    /// The atoll's small hills are centred by half of their own
    /// variation (10b0:3c5c).
    AtollHills = "atoll_hills";
    /// A river's 1500th try at a start is checked like the others; with
    /// none found there's no river (10b0:4506).
    RiverStart = "river_start";
    /// The generator's raising and lowering change nothing on an error,
    /// as the player's tool (10b0:0e0c, 10b0:2053).
    GeneratorLevelling = "generator_levelling";
    /// A river on the flat stops looking for its way at the map's edge,
    /// not round the other side (10b0:48d7).
    RiverFlatWay = "river_flat_way";
    /// Loading a game keeps the order of the building sites and of the
    /// plots' houses and facilities (1068:304f, 1068:2d49).
    LoadOrder = "load_order";
    /// Piranhas ordered for a building that isn't a pool show error 0x1a
    /// (10a0:1f65).
    SabotagePoolError = "sabotage_pool_error";
    /// The news archive keeps its messages in order when it first wraps
    /// (1048:0a54).
    NewsArchiveOrder = "news_archive_order";
    /// The TV's lights don't draw from the game's random numbers, so the
    /// speech doesn't change the game (1048:1b3e).
    TvLights = "tv_lights";
    /// The people only a screen needs (spectators, builders, taxiing
    /// planes, the jetties' boats, the harbours' sounds) are each
    /// player's own, with their own random numbers, and the world's
    /// people no longer follow anyone's view: no view squares, no view
    /// rotation, no sounds or random numbers by what a player sees
    /// (1090:2373, 1088:7fb1, `docs/people.md`). Off is the original,
    /// which only one human can play ([`SINGLE_PLAYER_ONLY`]).
    ViewPeople = "view_people";
    /// The shark's diver, which is a boat (kind 3), counts with the boats
    /// it's taken off when its time is up (10a0:19d2, 1098:109f).
    SharkBoatCount = "shark_boat_count";
    /// A saved game keeps what the original's loses, in the port's chunk
    /// (`state.` keys, [`crate::savegame`]): `RandSeed`, an oil slick
    /// going on, the people's lane, `6fce`, last building, beach and
    /// `3c42`, the auction's limits, joined flag and timers, the
    /// computers' road candidates past the file's player count, and,
    /// with `view_people`, the world's door people's buildings and its
    /// last record. A game loaded from such a file goes on as the one
    /// that was saved (1048:2a19, 1048:2bae).
    SavedState = "saved_state";
}

enhancements! {
    /// An enhancement of one player's view.
    Enhancement,
    /// The view enhancements that are on. The default is
    /// [`Enhancements::ORIGINAL`].
    Enhancements,
    Side::View;
    /// The floating forms stack like windows: a click raises one, the
    /// main screen too, a new one opens on top, the pointer goes to the
    /// top one, and only the modal forms stay above the others
    /// (`docs/layout.md`, "Stacking").
    WindowStacking = "window_stacking";
    /// On a screen bigger than 640 × 480 the main screen fills it and its
    /// map view shows more of the island, with the scroll bars' ends, the
    /// overview map's frame and the mouse following the bigger view; the
    /// world still sees the original's view in its middle
    /// (`docs/layout.md`, "`big_map`").
    BigMap = "big_map";
    /// The scroll bars' arrows step when pressed and repeat while held,
    /// an arrow let go off itself is drawn up again, and a press only
    /// counts as a change when the position changed (10c0:0f10,
    /// 10c0:15f7, 10c0:0fe3). The main screen's bars take their thumb by
    /// all of it, keep it under the mouse while it's dragged, and repeat
    /// only while the mouse stays on the part held, after a short wait
    /// (1020:0d92, 1020:0fa2, 1020:1e9b).
    ScrollBars = "scroll_bars";
    /// A click on a focused text field keeps it focused, "Speichern"
    /// saves a name being typed, and a name picked from the file menu's
    /// list puts the cursor after it (10c0:1c1b, 1040:073c, 1040:0192).
    EditField = "edit_field";
    /// Shift, Ctrl and a double click's second press don't stop a click
    /// on a list, with a tool or on the main screen's scroll bars
    /// (10c0:0431, 1020:1984, 1020:0d92, 1020:151a).
    ModifierClicks = "modifier_clicks";
    /// Only the left button clicks the start menu's buttons, ticks the
    /// options' boxes, drags the money menu and presses the credits'
    /// close button (1058:41fb, 1060:2404, 1070:0820, 1030:3ec4).
    MouseButtons = "mouse_buttons";
    /// Switching the start menu to its scenarios ticks the scenario that
    /// starts (1058:02cf).
    StartMenuScenario = "start_menu_scenario";
    /// The auction form's title stays on the row it opened with after a
    /// click on its title bar (1028:1ce0).
    AuctionTitle = "auction_title";
    /// Forms open where they belong: the auction centred, the money menu
    /// whole on the screen at its full height, the build menu level with
    /// the island, the sabotage window and the file menu over the main
    /// screen (1028:0dde, 1070:2b3a, 1070:2e6c, 1018:2fa2, `1110:54fc`).
    FormPositions = "form_positions";
    /// The lists of saved games are in the names' order (1038:3983,
    /// 1040:10e5).
    SavedGamesOrder = "saved_games_order";
    /// A dragged road ends where the mouse is let go, also with the
    /// animations or new people off (1090:124f, 1080:31ed).
    RoadDrag = "road_drag";
    /// The build menu's close sound only plays when it's open, and the
    /// keys it doesn't use go on to the main screen (1010:2be1,
    /// 1018:3290).
    BuildMenu = "build_menu";
    /// The money menu that the year's end opens presses its button
    /// (1008:4f64).
    MoneyMenuButton = "money_menu_button";
    /// The overview map keeps the main screen within its scroll bars and
    /// its horizontal thumb off the right arrow (10b8:37f5, 10b8:2785).
    OverviewScrolling = "overview_scrolling";
    /// The overview map colours low grass, high sand and nobody's
    /// buildings with the bitmap's nearest colours (10b8:04b1, 10b8:07df,
    /// 10b8:0a0a).
    OverviewColours = "overview_colours";
    /// A form's drag only starts on its title bar and ends when the mouse
    /// comes up, and the quit question stays whole on the screen
    /// (1038:34fc, 1028:2ecc, 1020:1bd9, 1020:38d4).
    FormDrags = "form_drags";
    /// The info box's and the overview's prices mark the thousands in the
    /// right place, and off the map the box names the sea (1030:108b,
    /// 1038:014b).
    InfoboxText = "infobox_text";
    /// The info box takes the size of what it shows after a button's
    /// command (1038:24d6).
    InfoboxSize = "infobox_size";
    /// The buildings' overview clears its price with its own background,
    /// leaves its scroll bar's columns to the bar, and Up and Down only
    /// scroll as far as the selection needs (1080:02d9, 1080:1c3b,
    /// 1080:202b).
    OverviewList = "overview_list";
    /// The TV's buttons click only on a press and release on them, its
    /// first button takes the view back where it was, browsing keeps it
    /// open and its title is centred over its bar (1048:1794, 1048:26f6,
    /// 1048:1b3e, 1048:12e9).
    TvForm = "tv_form";
    /// Any desktop size lays out for itself, with the nearest background
    /// and title picture centred on it (1000:2466).
    DesktopBackground = "desktop_background";
    /// The auctioneer's calls are heard, from `SPRADEU1\` (10a0:2824).
    AuctionCalls = "auction_calls";
    /// A sound's time runs down every tick, also with the animations off
    /// (1090:5293).
    SoundTimes = "sound_times";
    /// The music plays the playlist from its first track after the
    /// intro's, goes on to the next track as soon as one ends, and plays
    /// on after a game is loaded (1018:37de, 1018:367e, 1010:26fa).
    Music = "music";
    /// The TV's speech belongs to its message: a message without one
    /// stops the last, browsing doesn't keep it from starting, and a film
    /// after it plays at the sounds' volume and gives the sounds back
    /// when it ends (1048:0a54, 1048:1b3e, 1048:01bd, 1048:034f).
    TvSound = "tv_sound";
    /// The options' "INFO" opens the list of these enhancements, to switch
    /// them in the game, with the credits a button away; the original's
    /// opens the credits (1060:01ab, `crate::enhanceform`).
    EnhancementsScreen = "enhancements_screen";
    /// With `big_map`'s main screen filling the window, its own title bar
    /// (the game's name, the close and minimize widgets) goes, as the
    /// window has one: the info bar goes to the top and the map view
    /// gets the room (`docs/layout.md`, "`no_main_title`").
    NoMainTitle = "no_main_title";
    /// The mouse wheel scrolls: the island (Shift: across), the overview
    /// map, and the lists of the file menu, the load form, the buildings'
    /// overview and this list. The original has no wheel.
    MouseWheel = "mouse_wheel";
    /// With `fix.view_people`, the screen's own people (spectators,
    /// builders, taxiing planes, the jetties' boats) come all over the
    /// island, not only where the view looks (1090:2373).
    PeopleEverywhere = "people_everywhere";
}

/// The world fixes that may only be off with one human in the game: off,
/// the world reads that human's view, which several players don't share.
/// There is no multiplayer yet; [`Fixes::for_humans`] is how a game with
/// several humans would keep them on.
pub const SINGLE_PLAYER_ONLY: &[Fix] = &[Fix::ViewPeople];

impl Fixes {
    /// The fixes a game with `humans` human players runs with: these, but
    /// with several humans those of [`SINGLE_PLAYER_ONLY`] on.
    #[must_use]
    pub fn for_humans(mut self, humans: usize) -> Fixes {
        if humans > 1 {
            for &f in SINGLE_PLAYER_ONLY {
                self.set(f, true);
            }
        }
        self
    }
}

/// Every enhancement's [`Info`], the world's first.
pub fn all() -> impl Iterator<Item = Info> {
    Fix::ALL
        .iter()
        .map(|f| f.info())
        .chain(Enhancement::ALL.iter().map(|e| e.info()))
}

/// What a player's settings switch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Config {
    pub view: Enhancements,
    /// The world's fixes a new game starts with, and a game from a file
    /// without them takes.
    pub fixes: Fixes,
    /// The player's language (`language=`, `--lang=`, [`crate::lang`]).
    /// Not an enhancement: the original follows its folder too, which is
    /// `auto`, and `--original` leaves it.
    pub language: Setting,
}

impl Default for Config {
    /// Everything on.
    fn default() -> Self {
        Config {
            view: Enhancements::all_on(),
            fixes: Fixes::all_on(),
            language: Setting::Auto,
        }
    }
}

impl Config {
    /// Everything off: the original.
    pub const ORIGINAL: Config = Config {
        view: Enhancements::ORIGINAL,
        fixes: Fixes::ORIGINAL,
        language: Setting::Auto,
    };

    /// Everything on, or off with `original`, then each of `overrides`
    /// (`key`, on) in order. Also gives the keys no enhancement has; a
    /// [`RETIRED`] key is skipped without a word. A switch's text (`on`,
    /// `0`, ...) is the caller's (`platform::settings::switch`).
    pub fn resolve<'a>(original: bool, overrides: impl IntoIterator<Item = (&'a str, bool)>) -> (Config, Vec<String>) {
        let mut c = if original { Config::ORIGINAL } else { Config::default() };
        let mut unknown = Vec::new();
        for (key, on) in overrides {
            if let Some(f) = Fix::from_key(key) {
                c.fixes.set(f, on);
            } else if let Some(e) = Enhancement::from_key(key) {
                c.view.set(e, on);
            } else if !RETIRED.contains(&key) {
                unknown.push(key.to_owned());
            }
        }
        (c, unknown)
    }

    /// Whether `key` is on; none for a key no enhancement has.
    #[must_use]
    pub fn is_on(&self, key: &str) -> Option<bool> {
        Fix::from_key(key)
            .map(|f| self.fixes.has(f))
            .or_else(|| Enhancement::from_key(key).map(|e| self.view.has(e)))
    }
}

/// Keys of enhancements the port no longer has because the later build
/// of the game fixes their bug itself, so the port always behaves that
/// way (`docs/versions.md`). An old `settings.ini`, command line or saved
/// game that names one still loads; the key is ignored.
pub const RETIRED: &[&str] = &[
    // Enter at an auction (1028:26e4): en 1028:3620 has no Enter.
    "auction_enter_bid",
];

/// The prefix of a world fix's key in a saved game's chunk.
const SAVED_PREFIX: &str = "fix.";

/// The port's chunk for a saved game of a world with `fixes` and the
/// world's `language` (`crate::world::World::language`): each fix the
/// port knows as `fix.<key>=1` or `=0`, then `language=<code>`
/// ([`crate::lang::language_in`]).
#[must_use]
pub fn world_chunk(fixes: Fixes, language: Language) -> Vec<u8> {
    world_chunk_with(fixes, language, &[])
}

/// [`world_chunk`] with `more` lines after the language: the world's
/// state the original's file loses, with `fix.saved_state`
/// (`crate::savegame`'s `state.` keys).
#[must_use]
pub fn world_chunk_with(fixes: Fixes, language: Language, more: &[(String, String)]) -> Vec<u8> {
    let mut lines: Vec<(String, &str)> = Fix::ALL
        .iter()
        .map(|&f| {
            (
                format!("{SAVED_PREFIX}{}", f.info().key),
                if fixes.has(f) { "1" } else { "0" },
            )
        })
        .collect();
    lines.push(("language".to_owned(), language.code()));
    formats::savegame::make_port_chunk(
        lines
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .chain(more.iter().map(|(k, v)| (k.as_str(), v.as_str()))),
    )
}

/// The fixes a saved game's `trailing` bytes keep. A fix the file doesn't
/// name (an original file has no chunk; an older port's lacks the fixes
/// added since) gets its value in `defaults`. Keys the port doesn't know
/// (a newer port's) are skipped.
#[must_use]
pub fn fixes_in(trailing: &[u8], defaults: Fixes) -> Fixes {
    let mut fixes = defaults;
    for (key, value) in formats::savegame::port_chunk(trailing).unwrap_or_default() {
        if let Some(f) = key.strip_prefix(SAVED_PREFIX).and_then(Fix::from_key)
            && let Some(on) = match value.as_str() {
                "1" => Some(true),
                "0" => Some(false),
                _ => None,
            }
        {
            fixes.set(f, on);
        }
    }
    fixes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_fit() {
        let keys: Vec<_> = all().map(|i| i.key).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len());
        assert!(Fix::ALL.len() <= 64 && Enhancement::ALL.len() <= 64);
        for i in all() {
            assert!(
                i.key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            );
            for language in [Language::De, Language::En] {
                assert_ne!(i.description(language), "");
                let label = i.label(language);
                assert!(!label.is_empty() && label.chars().count() <= 40, "{}", i.key);
            }
        }
    }

    #[test]
    fn resolving() {
        let stacking = Enhancement::WindowStacking;
        let (c, unknown) = Config::resolve(false, []);
        assert!(c.view.has(stacking) && unknown.is_empty());
        assert_eq!(c, Config::default());
        let (c, _) = Config::resolve(true, []);
        assert_eq!(c, Config::ORIGINAL);
        assert!(!c.view.has(stacking));
        let (c, unknown) = Config::resolve(true, [("window_stacking", true), ("nonsense", false)]);
        assert!(c.view.has(stacking));
        assert_eq!(c.is_on("window_stacking"), Some(true));
        assert_eq!(c.is_on("nonsense"), None);
        assert_eq!(unknown, ["nonsense"]);
        // A retired key is ignored, without a word.
        let (c, unknown) = Config::resolve(false, [("auction_enter_bid", false)]);
        assert_eq!((c, unknown.len()), (Config::default(), 0));
        assert!(RETIRED.iter().all(|k| all().all(|i| i.key != *k)));
        // Later overrides win.
        let (c, _) = Config::resolve(false, [("window_stacking", false), ("window_stacking", true)]);
        assert!(c.view.has(stacking));
        let mut e = Enhancements::ORIGINAL;
        e.set(stacking, true);
        assert_eq!(e.iter().collect::<Vec<_>>(), [stacking]);
        e.set(stacking, false);
        assert_eq!(e, Enhancements::ORIGINAL);
    }

    #[test]
    fn single_player_only() {
        assert!(!Fixes::ORIGINAL.for_humans(1).has(Fix::ViewPeople));
        assert!(Fixes::ORIGINAL.for_humans(2).has(Fix::ViewPeople));
        assert!(!Fixes::ORIGINAL.for_humans(2).has(Fix::TvLights));
        assert_eq!(Fixes::all_on().for_humans(3), Fixes::all_on());
    }

    #[test]
    fn saved_fixes() {
        let defaults = Fixes::all_on();
        // An original file: the defaults.
        assert_eq!(fixes_in(&[], defaults), defaults);
        assert_eq!(fixes_in(b"junk", Fixes::ORIGINAL), Fixes::ORIGINAL);
        // The port's chunk, through and back.
        for fixes in [Fixes::ORIGINAL, Fixes::all_on()] {
            assert_eq!(fixes_in(&world_chunk(fixes, Language::En), Fixes::ORIGINAL), fixes);
            assert_eq!(
                crate::lang::language_in(&world_chunk(fixes, Language::En)),
                Some(Language::En)
            );
        }
        // Keys it doesn't know are skipped, a retired one too.
        let chunk = formats::savegame::make_port_chunk([
            ("fix.from_the_future", "1"),
            ("other", "x"),
            ("fix.auction_enter_bid", "0"),
        ]);
        assert_eq!(fixes_in(&chunk, Fixes::ORIGINAL), Fixes::ORIGINAL);
    }
}
