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
        let temp = path.with_extension("ini.tmp");
        std::fs::write(&temp, self.to_text())?;
        std::fs::rename(&temp, path)
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
}
