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
//!   [`fixes_chunk`]) and a player changes one by command
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
//! Adding one: a line in [`Fix`]'s or [`Enhancement`]'s list below, then
//! where the code branches on it, keep the original's path visible and
//! marked `// BUG(original): ...`, and name the key in the bug's row of
//! `docs/original-bugs.md`.
//!
//! [`World::fixes`]: crate::world::World::fixes
//! [`Action::SetFix`]: crate::command::Action::SetFix

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
    /// A short German name for the in-game list (`crate::enhanceform`),
    /// in the game's language: a few words, as plain as they can be.
    pub label: &'static str,
    /// One line for a list of them.
    pub description: &'static str,
}

/// Defines a kind of enhancement: the enum `$one` with a variant per
/// line, its `ALL` and `info`, and `$set`, a set of them as bits (up to
/// 64), with `ORIGINAL` (all off) and `ALL_ON`.
macro_rules! enhancements {
    ($(#[$m:meta])* $one:ident, $(#[$sm:meta])* $set:ident, $side:expr;
     $( $(#[$vm:meta])* $variant:ident = $key:literal, $label:literal, $desc:literal; )*) => {
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
                    $( $one::$variant => Info { key: $key, side: $side, label: $label, description: $desc }, )*
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
    SiteEffect = "site_effect", "Baustellenabriss nimmt Wirkung weg",
        "Demolishing a building site takes its effect off the location map";
    /// Demolishing a bridge over the sea costs `Strassenabrisskosten` a
    /// square, which the original adds up and never charges (1078:276f).
    BridgeDemolitionCost = "bridge_demolition_cost", "Brückenabriss im Meer kostet",
        "Demolishing a sea bridge costs the road demolition cost per square, as the original adds up but never charges";
    /// A facility's price class divides by the price range, not by
    /// `MaxPreis` (1008:0907).
    PriceClassRange = "price_class_range", "Preisklasse nach Min- und Maxpreis",
        "A facility's price class is its place between MinPreis and MaxPreis, not divided by MaxPreis";
    /// Buildings of nobody (owner 0) earn and cost nobody: their takings,
    /// costs and guests no longer go into player 8's slots of the month's
    /// sums and interest (settlement, 1078:0172).
    OwnerlessBuildings = "ownerless_buildings", "Herrenlose Gebäude zahlen an keinen",
        "Buildings of nobody no longer pay their takings, costs and guests into player 8's sums and interest";
    /// A palm that dies or that moving the land clears comes off its
    /// plot's count, and a cleared one off the location map, its owner
    /// and part (1010:129e, 10b0:069f, 10b0:1907).
    PalmRemoval = "palm_removal", "Tote Palmen werden abgezogen",
        "Palms that die or are cleared by moving the land come off their plot's count and the location map";
    /// A computer looking for another area checks the area of the plot
    /// it moves to, not the one it leaves (10a8:21de).
    ComputerAreaChange = "computer_area_change", "Computer prüft das neue Gebiet",
        "A computer changing area checks the size of the plot it moves to, not the one it leaves";
    /// The computers' type lists keep to their 40 places a category
    /// (10a8:388b, 10a8:0523).
    ComputerTypeLists = "computer_type_lists", "Computer-Baulisten bleiben begrenzt",
        "The computers' building lists stop at 40 types a category instead of running into the next";
    /// A blocked harbour ship's second detour checks the squares ahead,
    /// not its own square five times (1098:04f9).
    ShipDetour = "ship_detour", "Schiffe weichen richtig aus",
        "A blocked harbour ship's second detour looks at the squares ahead, not its own square";
    /// The stage spectators are looked for where they stand, so a stage
    /// doesn't get new ones every time (1090:2373).
    StageSpectators = "stage_spectators", "Bühnen finden ihre Zuschauer",
        "Stages look for their spectators where they stand, not on the stage's first square";
    /// A plane's second approach needs both coordinates to match
    /// (1090:2c1f).
    PlaneApproach = "plane_approach", "Flugzeuge landen richtig",
        "A plane's second approach needs both coordinates to match, not either";
    /// A corner rising from the sea between plots merges them by their
    /// whole numbers, not cut to a byte (1078:09f6).
    PlotMergeNumbers = "plot_merge_numbers", "Grundstücke verschmelzen richtig",
        "Land rising between plots merges them by their full numbers, not their numbers cut to a byte";
    /// The computers' roads start from their own start square's ground,
    /// not the human's last road drag's (1078:2396).
    ComputerRoadGround = "computer_road_ground", "Computerstraßen nach eigenem Boden",
        "The computers' roads go by the ground they start on, not the human's last road drag";
    /// Moving the land takes neighbours along that are two steps or more
    /// off, so cliffs don't get steeper (10b0:03fa, 10b0:12c7).
    LevellingCliffs = "levelling_cliffs", "Planieren ohne steilere Klippen",
        "Moving the land takes along neighbours two or more steps off, so existing cliffs don't get steeper";
    /// Raising and lowering the land report the first error: lowering
    /// stops looking at buildings after one, and the money check doesn't
    /// replace it (10b0:069f, 10b0:12c7).
    LevellingErrors = "levelling_errors", "Planieren meldet den ersten Fehler",
        "Raising and lowering the land report the first error, not a later building or the money";
    /// After a road is built, every building near it gets its connection
    /// again, by its first square, whatever its look (1078:2178).
    RoadConnections = "road_connections", "Neue Straßen verbinden alle Gebäude",
        "A new road reconnects every building near it, not only those whose first square shows part 0";
    /// The info box's buttons (renovate, prices, sell, bribe) are only
    /// for the player's own squares, not nobody's (1038:1a75).
    NobodysBuildings = "nobodys_buildings", "Infobox-Knöpfe nur für eigene Gebäude",
        "The info box's buttons work on the player's own buildings only, not on nobody's";
    /// A computer bids only when its bid (5 % more) is within its limit
    /// (1028:1973).
    AuctionComputerLimit = "auction_computer_limit", "Computer bieten nur im Limit",
        "A computer bids at an auction only when its bid is within its limit, not just the price";
    /// Passing at an auction leaves a leading bid standing and doesn't
    /// give the item away a second time after the third call (1028:0c82).
    AuctionPass = "auction_pass", "Passen lässt das Höchstgebot stehen",
        "Passing at an auction keeps the leading bid and doesn't sell the item again after the third call";
    /// Enter bids at an auction with the same checks as the mouse: the
    /// money, and not after the third call (1028:26e4).
    AuctionEnterBid = "auction_enter_bid", "Gebot mit Enter wie mit der Maus",
        "Enter bids at an auction only with the money and before the third call, as the mouse does";
    /// Player 8 gets the building type it wins at an auction
    /// (`make_available`, 10a0:0691).
    PrizePlayer8 = "prize_player8", "Spieler 8 bekommt Ersteigertes",
        "Player 8 gets the building it wins at an auction, as players 2 to 7 do";
    /// A computer's highest bid is worked out without overflowing
    /// (10a8:0167).
    ComputerBidOverflow = "computer_bid_overflow", "Computer-Limit ohne Überlauf",
        "A rich computer's auction limit no longer overflows and wraps";
    /// The atoll's small hills are centred by half of their own
    /// variation (10b0:3c5c).
    AtollHills = "atoll_hills", "Atoll-Hügel richtig verteilt",
        "The atoll's small hills are centred on the ring by their own variation, not the big hills'";
    /// A river's 1500th try at a start is checked like the others; with
    /// none found there's no river (10b0:4506).
    RiverStart = "river_start", "Flüsse entspringen nur auf Hügeln",
        "The island generator's rivers only start on the hills, never on the flat or in the sea";
    /// The generator's raising and lowering change nothing on an error,
    /// as the player's tool (10b0:0e0c, 10b0:2053).
    GeneratorLevelling = "generator_levelling", "Inselbau: Flüsse ohne Klippen",
        "The island generator's land changes are undone on an error, so its rivers leave no cliffs";
    /// A river on the flat stops looking for its way at the map's edge,
    /// not round the other side (10b0:48d7).
    RiverFlatWay = "river_flat_way", "Flüsse suchen nur bis zum Rand",
        "A river on the flat looks for its way up to the map's edge, not round the other side";
    /// Loading a game keeps the order of the building sites and of the
    /// plots' houses and facilities (1068:304f, 1068:2d49).
    LoadOrder = "load_order", "Laden behält die Reihenfolge",
        "Loading a game keeps the building sites and the plots' building lists in their saved order";
    /// Piranhas ordered for a building that isn't a pool show error 0x1a
    /// (10a0:1f65).
    SabotagePoolError = "sabotage_pool_error", "Piranhas nur in Pools",
        "Piranhas ordered for a building that isn't a pool say so, as the other sabotage kinds do";
    /// The news archive keeps its messages in order when it first wraps
    /// (1048:0a54).
    NewsArchiveOrder = "news_archive_order", "Nachrichtenarchiv bleibt geordnet",
        "The news archive keeps its 26th and 27th messages in order when it first fills";
    /// The TV's lights don't draw from the game's random numbers, so the
    /// speech doesn't change the game (1048:1b3e).
    TvLights = "tv_lights", "Fernsehlichter ohne Spielzufall",
        "The TV's blinking lights no longer use the game's random numbers, so the speech doesn't change the game";
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
    WindowStacking = "window_stacking", "Fenster stapeln wie unter Windows",
        "Forms stack like windows: a click raises one (the main screen too), only modal forms stay on top";
    /// On a screen bigger than 640 × 480 the main screen fills it and its
    /// map view shows more of the island, with the scroll bars' ends, the
    /// overview map's frame and the mouse following the bigger view; the
    /// world still sees the original's view in its middle
    /// (`docs/layout.md`, "`big_map`").
    BigMap = "big_map", "Großes Fenster zeigt mehr Insel",
        "A bigger screen shows more island: the main screen fills it and its map view grows";
    /// The scroll bars' arrows step when pressed and repeat while held,
    /// an arrow let go off itself is drawn up again, and a press only
    /// counts as a change when the position changed (10c0:0f10,
    /// 10c0:15f7, 10c0:0fe3). The main screen's bars take their thumb by
    /// all of it, keep it under the mouse while it's dragged, and repeat
    /// only while the mouse stays on the part held, after a short wait
    /// (1020:0d92, 1020:0fa2, 1020:1e9b).
    ScrollBars = "scroll_bars", "Bildlaufleisten wie unter Windows",
        "Scroll bar arrows step when pressed and repeat while held; thumbs follow the mouse; paging stops at the mouse";
    /// A click on a focused text field keeps it focused, "Speichern"
    /// saves a name being typed, and a name picked from the file menu's
    /// list puts the cursor after it (10c0:1c1b, 1040:073c, 1040:0192).
    EditField = "edit_field", "Textfelder behalten den Fokus",
        "Text fields keep the focus when clicked, so the file menu's Speichern saves a typed name";
    /// Shift, Ctrl and a double click's second press don't stop a click
    /// on a list, with a tool or on the main screen's scroll bars
    /// (10c0:0431, 1020:1984, 1020:0d92, 1020:151a).
    ModifierClicks = "modifier_clicks", "Klicks auch mit Umschalt und Strg",
        "Clicks on lists, with the tools and on the main scroll bars work with Shift or Ctrl held and on a double click";
    /// Only the left button clicks the start menu's buttons, ticks the
    /// options' boxes, drags the money menu and presses the credits'
    /// close button (1058:41fb, 1060:2404, 1070:0820, 1030:3ec4).
    MouseButtons = "mouse_buttons", "Nur die linke Maustaste klickt",
        "The right mouse button no longer clicks start menu buttons, ticks options, drags the money menu or closes the credits";
    /// Switching the start menu to its scenarios ticks the scenario that
    /// starts (1058:02cf).
    StartMenuScenario = "start_menu_scenario", "Startmenü hakt das Szenario an",
        "The start menu's scenario page ticks the scenario that will start";
    /// The auction form's title stays on the row it opened with after a
    /// click on its title bar (1028:1ce0).
    AuctionTitle = "auction_title", "Auktionstitel springt nicht",
        "The auction form's title doesn't jump a row up after a click on its title bar";
    /// Forms open where they belong: the auction centred, the money menu
    /// whole on the screen at its full height, the build menu level with
    /// the island, the sabotage window and the file menu over the main
    /// screen (1028:0dde, 1070:2b3a, 1070:2e6c, 1018:2fa2, `1110:54fc`).
    FormPositions = "form_positions", "Fenster öffnen am richtigen Platz",
        "Forms open where they belong: the auction centred, the money menu whole, the others over the main screen";
    /// The lists of saved games are in the names' order (1038:3983,
    /// 1040:10e5).
    SavedGamesOrder = "saved_games_order", "Spielstände nach Namen sortiert",
        "The lists of saved games are sorted by name, not backwards";
    /// A dragged road ends where the mouse is let go, also with the
    /// animations or new people off (1090:124f, 1080:31ed).
    RoadDrag = "road_drag", "Straße endet beim Loslassen",
        "A dragged road ends where the mouse is let go, also with the animations off";
    /// The build menu's close sound only plays when it's open, and the
    /// keys it doesn't use go on to the main screen (1010:2be1,
    /// 1018:3290).
    BuildMenu = "build_menu", "Baumenü gibt Tasten weiter",
        "The build menu passes on the keys it doesn't use and makes no closing sound when it's closed";
    /// The money menu that the year's end opens presses its button
    /// (1008:4f64).
    MoneyMenuButton = "money_menu_button", "Jahresbilanz drückt den Geldknopf",
        "The year's end presses the money menu's button when it opens the menu";
    /// The overview map keeps the main screen within its scroll bars and
    /// its horizontal thumb off the right arrow (10b8:37f5, 10b8:2785).
    OverviewScrolling = "overview_scrolling", "Übersichtskarte scrollt richtig",
        "Clicks on the overview map keep the main view within its scroll bars; its thumb stays off the arrow";
    /// The overview map colours low grass, high sand and nobody's
    /// buildings with the bitmap's nearest colours (10b8:04b1, 10b8:07df,
    /// 10b8:0a0a).
    OverviewColours = "overview_colours", "Übersichtskarte zeigt alle Farben",
        "The overview map has a colour for low grass, high sand and nobody's buildings instead of none";
    /// A form's drag only starts on its title bar and ends when the mouse
    /// comes up, and the quit question stays whole on the screen
    /// (1038:34fc, 1028:2ecc, 1020:1bd9, 1020:38d4).
    FormDrags = "form_drags", "Fensterziehen endet beim Loslassen",
        "Form drags end when the mouse comes up and only move the form grabbed; the quit question stays on screen";
    /// The info box's and the overview's prices mark the thousands in the
    /// right place, and off the map the box names the sea (1030:108b,
    /// 1038:014b).
    InfoboxText = "infobox_text", "Infobox: Tausenderpunkte und Meer",
        "The info box puts the thousands marks in the right place and names the sea off the map";
    /// The info box takes the size of what it shows after a button's
    /// command (1038:24d6).
    InfoboxSize = "infobox_size", "Infobox passt ihre Größe an",
        "The info box grows or shrinks to what it shows after a button";
    /// The buildings' overview clears its price with its own background,
    /// leaves its scroll bar's columns to the bar, and Up and Down only
    /// scroll as far as the selection needs (1080:02d9, 1080:1c3b,
    /// 1080:202b).
    OverviewList = "overview_list", "Gebäudeübersicht scrollt richtig",
        "The buildings' overview keeps the selection in view with Up and Down and draws its price cleanly";
    /// The TV's buttons click only on a press and release on them, its
    /// first button takes the view back where it was, browsing keeps it
    /// open and its title is centred over its bar (1048:1794, 1048:26f6,
    /// 1048:1b3e, 1048:12e9).
    TvForm = "tv_form", "Fernsehknöpfe wie gedacht",
        "The TV's buttons, back button and title work as meant, and browsing keeps it open";
    /// Any desktop size lays out for itself, with the nearest background
    /// and title picture centred on it (1000:2466).
    DesktopBackground = "desktop_background", "Hintergrundbild für jede Größe",
        "Any window size gets the nearest background picture, centred, and nothing is cut off at the edges";
    /// The auctioneer's calls are heard, from `SPRADEU1\` (10a0:2824).
    AuctionCalls = "auction_calls", "Der Auktionator ist zu hören",
        "The auctioneer's three calls are heard";
    /// A sound's time runs down every tick, also with the animations off
    /// (1090:5293).
    SoundTimes = "sound_times", "Geräusche auch ohne Animationen",
        "Sounds play again after their time with the animations off too";
    /// The music plays the playlist from its first track after the
    /// intro's, goes on to the next track as soon as one ends, and plays
    /// on after a game is loaded (1018:37de, 1018:367e, 1010:26fa).
    Music = "music", "Musik ohne Pausen, auch nach Laden",
        "The music starts at the playlist's first track, has no silence between tracks and plays on after loading";
    /// The TV's speech belongs to its message: a message without one
    /// stops the last, browsing doesn't keep it from starting, and a film
    /// after it plays at the sounds' volume and gives the sounds back
    /// when it ends (1048:0a54, 1048:1b3e, 1048:01bd, 1048:034f).
    TvSound = "tv_sound", "Fernsehton passt zur Nachricht",
        "The TV's speech belongs to its message, and its films don't leave the sounds silent or at the speech volume";
    /// The options' "INFO" opens the list of these enhancements, to switch
    /// them in the game, with the credits a button away; the original's
    /// opens the credits (1060:01ab, `crate::enhanceform`).
    EnhancementsScreen = "enhancements_screen", "INFO zeigt diese Liste",
        "The options' INFO button opens this list of enhancements (the credits are a button away)";
    /// With `big_map`'s main screen filling the window, its own title bar
    /// (the game's name, the close and minimize widgets) goes, as the
    /// window has one: the info bar goes to the top and the map view
    /// gets the room (`docs/layout.md`, "`no_main_title`").
    NoMainTitle = "no_main_title", "Kein zweiter Titelbalken",
        "When the main screen fills the window it drops its own title bar; the map gets the room";
    /// The mouse wheel scrolls: the island (Shift: across), the overview
    /// map, and the lists of the file menu, the load form, the buildings'
    /// overview and this list. The original has no wheel.
    MouseWheel = "mouse_wheel", "Mausrad scrollt",
        "The mouse wheel scrolls the island (with Shift across), the overview map and the lists";
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
}

impl Default for Config {
    /// Everything on.
    fn default() -> Self {
        Config {
            view: Enhancements::all_on(),
            fixes: Fixes::all_on(),
        }
    }
}

impl Config {
    /// Everything off: the original.
    pub const ORIGINAL: Config = Config {
        view: Enhancements::ORIGINAL,
        fixes: Fixes::ORIGINAL,
    };

    /// Everything on, or off with `original`, then each of `overrides`
    /// (`key`, on) in order. Also gives the keys no enhancement has. A switch's text
    /// (`on`, `0`, ...) is the caller's (`platform::settings::switch`).
    pub fn resolve<'a>(original: bool, overrides: impl IntoIterator<Item = (&'a str, bool)>) -> (Config, Vec<String>) {
        let mut c = if original { Config::ORIGINAL } else { Config::default() };
        let mut unknown = Vec::new();
        for (key, on) in overrides {
            if let Some(f) = Fix::from_key(key) {
                c.fixes.set(f, on);
            } else if let Some(e) = Enhancement::from_key(key) {
                c.view.set(e, on);
            } else {
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

/// The prefix of a world fix's key in a saved game's chunk.
const SAVED_PREFIX: &str = "fix.";

/// The port's chunk for a saved game with `fixes`: each fix the port
/// knows as `fix.<key>=1` or `=0`.
#[must_use]
pub fn fixes_chunk(fixes: Fixes) -> Vec<u8> {
    let lines: Vec<(String, &str)> = Fix::ALL
        .iter()
        .map(|&f| {
            (
                format!("{SAVED_PREFIX}{}", f.info().key),
                if fixes.has(f) { "1" } else { "0" },
            )
        })
        .collect();
    formats::savegame::make_port_chunk(lines.iter().map(|(k, v)| (k.as_str(), *v)))
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
            assert_ne!(i.description, "");
            assert!(!i.label.is_empty() && i.label.chars().count() <= 40, "{}", i.key);
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
    fn saved_fixes() {
        let defaults = Fixes::all_on();
        // An original file: the defaults.
        assert_eq!(fixes_in(&[], defaults), defaults);
        assert_eq!(fixes_in(b"junk", Fixes::ORIGINAL), Fixes::ORIGINAL);
        // The port's chunk, through and back.
        for fixes in [Fixes::ORIGINAL, Fixes::all_on()] {
            assert_eq!(fixes_in(&fixes_chunk(fixes), Fixes::ORIGINAL), fixes);
        }
        // Keys it doesn't know are skipped.
        let chunk = formats::savegame::make_port_chunk([("fix.from_the_future", "1"), ("other", "x")]);
        assert_eq!(fixes_in(&chunk, Fixes::ORIGINAL), Fixes::ORIGINAL);
    }
}
