//! The port's own settings file, separate from anything the original
//! reads: the window's size and position and whether it was fullscreen,
//! remembered from one run to the next. It lives in the platform's
//! config directory ([`crate::storage::config_dir`]) as `settings.ini`,
//! one `key=value` per line:
//!
//! ```text
//! window_width=800
//! window_height=600
//! window_x=120
//! window_y=80
//! fullscreen=false
//! ```
//!
//! Sizes are in logical pixels (the desktop's points, before the
//! display's scale factor), positions in physical pixels as the window
//! system reports them. Unknown keys and lines that don't parse are
//! skipped, missing ones keep their defaults.
//!
//! The enhancements' switches ([`EnhanceSettings`], `game::enhance`) are
//! the player's to write by hand; the port only reads them:
//!
//! ```text
//! original=false
//! enhance.window_stacking=off
//! ```
//!
//! Writing the window's state keeps every other line of the file.

use std::path::{Path, PathBuf};

/// What the settings file remembers about the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowSettings {
    /// The windowed client area's size, in logical pixels.
    pub size: Option<(u32, u32)>,
    /// The window's outer top left, where the window system has one.
    pub position: Option<(i32, i32)>,
    pub fullscreen: bool,
}

impl WindowSettings {
    /// The settings in `text`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut s = WindowSettings::default();
        let (mut width, mut height, mut x, mut y) = (None, None, None, None);
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "window_width" => width = value.parse::<u32>().ok().filter(|&w| w > 0),
                "window_height" => height = value.parse::<u32>().ok().filter(|&h| h > 0),
                "window_x" => x = value.parse::<i32>().ok(),
                "window_y" => y = value.parse::<i32>().ok(),
                "fullscreen" => s.fullscreen = value == "true",
                _ => {}
            }
        }
        s.size = width.zip(height);
        s.position = x.zip(y);
        s
    }

    /// The file's text.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some((w, h)) = self.size {
            lines.push(format!("window_width={w}"));
            lines.push(format!("window_height={h}"));
        }
        if let Some((x, y)) = self.position {
            lines.push(format!("window_x={x}"));
            lines.push(format!("window_y={y}"));
        }
        lines.push(format!("fullscreen={}", self.fullscreen));
        lines.join("\n") + "\n"
    }

    /// The settings in the file at `path`; the defaults if there's none
    /// or it can't be read.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).map_or_else(|_| WindowSettings::default(), |t| WindowSettings::parse(&t))
    }

    /// Writes the settings to `path`, making its directory.
    ///
    /// # Errors
    /// If the directory or the file can't be written.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // The other lines (the enhancements, comments) stay as they are.
        let others: String = std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.split_once('=').is_some_and(|(k, _)| WINDOW_KEYS.contains(&k.trim())))
            .flat_map(|l| [l, "\n"])
            .collect();
        let temp = path.with_extension("ini.tmp");
        std::fs::write(&temp, others + &self.to_text())?;
        std::fs::rename(&temp, path)
    }
}

/// The keys [`WindowSettings`] writes.
const WINDOW_KEYS: [&str; 5] = ["window_width", "window_height", "window_x", "window_y", "fullscreen"];

/// The enhancements' switches in the settings file: `original=true` (all
/// off) and `enhance.<key>=on|off` (also `1`/`0`, `true`/`false`,
/// `yes`/`no`), in the file's order. The keys mean nothing here; the game
/// resolves them (`game::enhance::Config::resolve`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnhanceSettings {
    pub original: bool,
    pub overrides: Vec<(String, bool)>,
    /// Lines with an `enhance.` key whose value isn't a switch.
    pub bad: Vec<String>,
}

impl EnhanceSettings {
    /// The switches in `text`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut s = EnhanceSettings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key == "original" {
                s.original = switch(value).unwrap_or(false);
            } else if let Some(k) = key.strip_prefix("enhance.") {
                match switch(value) {
                    Some(on) => s.overrides.push((k.to_owned(), on)),
                    None => s.bad.push(line.to_owned()),
                }
            }
        }
        s
    }

    /// The switches in the file at `path`; none if there's no file.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).map_or_else(|_| EnhanceSettings::default(), |t| EnhanceSettings::parse(&t))
    }
}

/// A switch's value: `on`/`off`, `1`/`0`, `true`/`false`, `yes`/`no`.
#[must_use]
pub fn switch(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Some(true),
        "off" | "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

/// Where the settings file is: `settings.ini` in the config directory.
#[must_use]
pub fn settings_file() -> Option<PathBuf> {
    crate::storage::config_dir().map(|d| d.join("settings.ini"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = WindowSettings {
            size: Some((1024, 700)),
            position: Some((-8, 30)),
            fullscreen: true,
        };
        assert_eq!(WindowSettings::parse(&s.to_text()), s);
        assert_eq!(WindowSettings::parse(""), WindowSettings::default());
        let odd = WindowSettings::parse("window_width=0\nwindow_height=5\n# x\nfullscreen=false\nwindow_x=3\n");
        assert_eq!(odd, WindowSettings::default());
    }

    #[test]
    fn enhancements() {
        let e = EnhanceSettings::parse("window_width=3\noriginal=1\nenhance.window_stacking=on\nenhance.x=maybe\n");
        assert!(e.original);
        assert_eq!(e.overrides, [("window_stacking".to_owned(), true)]);
        assert_eq!(e.bad, ["enhance.x=maybe"]);
        assert_eq!(EnhanceSettings::parse(""), EnhanceSettings::default());
    }

    #[test]
    fn saving_keeps_the_other_lines() {
        let dir = std::env::temp_dir().join(format!("holiday-settings-{}", std::process::id()));
        let path = dir.join("settings.ini");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            "# mine\nwindow_width=1\nenhance.window_stacking=off\nfullscreen=true\n",
        )
        .unwrap();
        let s = WindowSettings {
            size: Some((800, 600)),
            ..WindowSettings::default()
        };
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(WindowSettings::parse(&text), s);
        assert!(text.starts_with("# mine\nenhance.window_stacking=off\n"));
        assert_eq!(text.matches("window_width").count(), 1);
    }
}
