//! Where the port keeps the player's files: saved games (and later the
//! high scores) in the platform's data directory, and its settings
//! ([`crate::settings`]) in the config directory. The
//! original writes its saves into the game's own `MAPS\` directory; the
//! port leaves the game's files alone and only reads from there.

use std::path::PathBuf;

/// The directory the port's own files go in: `$XDG_DATA_HOME/holiday-island`
/// (or `~/.local/share/holiday-island`) on Linux and the other Unixes,
/// `~/Library/Application Support/Holiday Island` on macOS and
/// `%APPDATA%\Holiday Island` on Windows. `None` if the environment
/// doesn't say where the home directory is.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        var("APPDATA").map(|d| d.join("Holiday Island"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Application Support/Holiday Island"))
    } else {
        var("XDG_DATA_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("holiday-island"))
    }
}

/// Where saved games go: `MAPS` in [`data_dir`], like the game's own
/// `MAPS\`.
#[must_use]
pub fn save_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("MAPS"))
}

/// The directory the port's settings go in: `$XDG_CONFIG_HOME/holiday-island`
/// (or `~/.config/holiday-island`) on Linux and the other Unixes,
/// `~/Library/Preferences/Holiday Island` on macOS and
/// `%APPDATA%\Holiday Island` on Windows. `None` if the environment
/// doesn't say where the home directory is.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        var("APPDATA").map(|d| d.join("Holiday Island"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Preferences/Holiday Island"))
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".config")))
            .map(|d| d.join("holiday-island"))
    }
}
