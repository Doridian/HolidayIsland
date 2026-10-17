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
    /// One line for a list of them.
    pub description: &'static str,
}

/// Defines a kind of enhancement: the enum `$one` with a variant per
/// line, its `ALL` and `info`, and `$set`, a set of them as bits (up to
/// 64), with `ORIGINAL` (all off) and `ALL_ON`.
macro_rules! enhancements {
    ($(#[$m:meta])* $one:ident, $(#[$sm:meta])* $set:ident, $side:expr;
     $( $(#[$vm:meta])* $variant:ident = $key:literal, $desc:literal; )*) => {
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
                    $( $one::$variant => Info { key: $key, side: $side, description: $desc }, )*
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
    /// A fix that changes the simulation. None yet.
    Fix,
    /// The world's fixes that are on ([`World::fixes`]). The default is
    /// [`Fixes::ORIGINAL`], so a `World` built in a test is the original's
    /// unless it asks for a fix.
    ///
    /// [`World::fixes`]: crate::world::World::fixes
    Fixes,
    Side::World;
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
    WindowStacking = "window_stacking",
        "Forms stack like windows: a click raises one (the main screen too), only modal forms stay on top";
    /// On a screen bigger than 640 × 480 the main screen fills it and its
    /// map view shows more of the island, with the scroll bars' ends, the
    /// overview map's frame and the mouse following the bigger view; the
    /// world still sees the original's view in its middle
    /// (`docs/layout.md`, "`big_map`").
    BigMap = "big_map",
        "A bigger screen shows more island: the main screen fills it and its map view grows";
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
