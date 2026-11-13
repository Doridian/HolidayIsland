//! A window of the port's own that asks for a file or folder, dropped on
//! it or its path typed, where the system has no file dialog to ask with
//! (a minimal Linux, a sandbox; `holiday`'s `pick`). It draws its text
//! itself, in [`FONT`]: not the system's fonts, which may not be there,
//! nor the game's, which is in the files it asks for.
//!
//! winit makes one event loop per process, so the window's is kept for
//! [`crate::run`] to run the game on ([`take_event_loop`]). Drops reach
//! an X11 window but not a Wayland one (winit 0.30), so where there is an
//! X server (`DISPLAY`) the loop is X11's, and the game's window then too.

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use winit::window::{Window, WindowId};

use crate::{Cursor, Error, Frame, display};

thread_local! {
    static EVENT_LOOP: RefCell<Option<EventLoop<()>>> = const { RefCell::new(None) };
}

/// The event loop the window ran on, if it did.
pub(crate) fn take_event_loop() -> Option<EventLoop<()>> {
    EVENT_LOOP.take()
}

/// The window's size, in points.
const SIZE: (u32, u32) = (800, 300);
/// The space around the text, in pixels of the font.
const MARGIN: u32 = 16;

/// The colours: the background, the title, the text, what's typed, an
/// error.
const PALETTE: [[u8; 3]; 5] = [
    [0x20, 0x20, 0x24],
    [0xF0, 0xC0, 0x20],
    [0xE8, 0xE8, 0xE8],
    [0xA0, 0xA0, 0xA0],
    [0xFF, 0x70, 0x60],
];
const TITLE: u8 = 1;
const TEXT: u8 = 2;
const TYPED: u8 = 3;
const ERROR: u8 = 4;

/// Opens a window titled `title` that shows `lines` and takes a file or
/// folder dropped on it, or a path typed and Enter, until `take` accepts
/// one: what `take` makes of it. `take`'s error is shown under the text,
/// and another may be dropped. `None` if the window is closed.
///
/// # Errors
/// If no window or GPU device can be set up.
pub fn ask_drop<T>(
    title: &str,
    lines: &[&str],
    take: impl FnMut(&Path) -> Result<T, String>,
) -> Result<Option<T>, Error> {
    let mut event_loop = match take_event_loop() {
        Some(l) => l,
        None => new_event_loop()?,
    };
    let mut asker = Asker {
        title,
        lines,
        take,
        window: None,
        typed: String::new(),
        error: None,
        got: None,
        failed: None,
        pixels: Vec::new(),
    };
    let ran = event_loop.run_app_on_demand(&mut asker);
    EVENT_LOOP.set(Some(event_loop));
    ran.map_err(|e| Error::Window(e.to_string()))?;
    asker.failed.map_or(Ok(asker.got), Err)
}

/// An event loop on X11 where there is an X server: the Wayland one gets
/// no drops.
fn new_event_loop() -> Result<EventLoop<()>, Error> {
    let mut builder = EventLoop::builder();
    #[cfg(all(unix, not(target_vendor = "apple"), not(target_os = "android")))]
    if std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11();
    }
    builder.build().map_err(|e| Error::Window(e.to_string()))
}

struct Asker<'a, T, F> {
    title: &'a str,
    lines: &'a [&'a str],
    take: F,
    window: Option<(Arc<Window>, display::Display)>,
    /// The path typed so far.
    typed: String,
    /// What `take` said of the last one.
    error: Option<String>,
    got: Option<T>,
    failed: Option<Error>,
    /// The framebuffer, as big as the window in pixels of the font.
    pixels: Vec<u8>,
}

impl<T, F: FnMut(&Path) -> Result<T, String>> Asker<'_, T, F> {
    fn try_path(&mut self, event_loop: &ActiveEventLoop, path: &Path) {
        match (self.take)(path) {
            Ok(got) => {
                self.got = Some(got);
                self.window = None;
                event_loop.exit();
            }
            Err(e) => {
                self.error = Some(e);
                self.redraw();
            }
        }
    }

    fn redraw(&self) {
        if let Some((w, _)) = &self.window {
            w.request_redraw();
        }
    }

    /// The framebuffer, `width` × `height`: the title, the lines, what's
    /// typed and the error, centred.
    fn render(&mut self, width: u32, height: u32) {
        let fit = |k: u32| (width.saturating_sub(2 * MARGIN) / (8 * k)).max(1) as usize;
        let mut rows: Vec<(String, u32, u8)> = Vec::new();
        let para = |text: &str, k: u32, colour: u8, rows: &mut Vec<(String, u32, u8)>| {
            for line in wrap(text, fit(k)) {
                rows.push((line, k, colour));
            }
        };
        para(self.title, 3, TITLE, &mut rows);
        rows.push((String::new(), 1, TEXT));
        for line in self.lines {
            para(line, 2, TEXT, &mut rows);
        }
        rows.push((String::new(), 1, TEXT));
        para(&format!("> {}_", self.typed), 2, TYPED, &mut rows);
        if let Some(e) = &self.error {
            rows.push((String::new(), 1, TEXT));
            para(e, 2, ERROR, &mut rows);
        }
        let total: u32 = rows.iter().map(|&(_, k, _)| 10 * k).sum();
        self.pixels.clear();
        self.pixels.resize((width * height) as usize, 0);
        let mut y = height.saturating_sub(total) / 2;
        for (text, k, colour) in rows {
            let w = 8 * k * u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
            let x = width.saturating_sub(w) / 2;
            draw(&mut self.pixels, width, height, (x, y), k, colour, &text);
            y += 10 * k;
        }
    }
}

/// `text` in lines of at most `width` characters, broken between words
/// where it can be.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word = word;
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        // A word longer than a line (a path) is broken anywhere.
        while line.chars().count() + word.chars().count() > width {
            let n = width - line.chars().count();
            let at = word.char_indices().nth(n).map_or(word.len(), |(i, _)| i);
            line.push_str(&word[..at]);
            lines.push(std::mem::take(&mut line));
            word = &word[at..];
        }
        line.push_str(word);
    }
    lines.push(line);
    lines
}

/// `text` in [`FONT`] at `at`, each of its pixels `k` × `k`; what isn't
/// printable ASCII as `?`.
fn draw(pixels: &mut [u8], width: u32, height: u32, at: (u32, u32), k: u32, colour: u8, text: &str) {
    for (i, c) in (0..).zip(text.chars()) {
        let c = if (' '..='~').contains(&c) { c } else { '?' };
        let glyph = &FONT[c as usize - 0x20];
        for (row, bits) in (0..).zip(glyph) {
            for col in 0..8 {
                if bits >> col & 1 == 0 {
                    continue;
                }
                for dy in 0..k {
                    for dx in 0..k {
                        let x = at.0 + (i * 8 + col) * k + dx;
                        let y = at.1 + row * k + dy;
                        if x < width && y < height {
                            pixels[(y * width + x) as usize] = colour;
                        }
                    }
                }
            }
        }
    }
}

impl<T, F: FnMut(&Path) -> Result<T, String>> ApplicationHandler for Asker<'_, T, F> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.got.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.title)
            .with_inner_size(LogicalSize::new(SIZE.0, SIZE.1));
        let made = event_loop
            .create_window(attrs)
            .map_err(|e| Error::Window(e.to_string()))
            .and_then(|w| {
                let w = Arc::new(w);
                let d = display::Display::new(event_loop, w.clone())?;
                Ok((w, d))
            });
        match made {
            Ok(w) => self.window = Some(w),
            Err(e) => {
                self.failed = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.window = None;
                event_loop.exit();
            }
            WindowEvent::DroppedFile(path) => self.try_path(event_loop, &path),
            WindowEvent::Resized(size) => {
                if let Some((_, d)) = &mut self.window {
                    d.resize(size);
                }
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match &event.logical_key {
                    Key::Named(NamedKey::Enter) if !self.typed.is_empty() => {
                        let path = std::path::PathBuf::from(self.typed.trim());
                        self.try_path(event_loop, &path);
                        return;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.typed.pop();
                    }
                    _ => {
                        if let Some(text) = &event.text {
                            self.typed.extend(text.chars().filter(|c| !c.is_control()));
                        }
                    }
                }
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                let Some((w, _)) = &self.window else { return };
                let size = w.inner_size();
                // Whole pixels of the font, as big as the display's
                // scale makes them.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let scale = w.scale_factor().round().max(1.0) as u32;
                let (width, height) = ((size.width / scale).max(1), (size.height / scale).max(1));
                self.render(width, height);
                let Some((_, d)) = &mut self.window else { return };
                let frame = Frame {
                    width,
                    height,
                    pixels: &self.pixels,
                    palette: &palette(),
                    overlay: None,
                    cursor: Cursor::Default,
                };
                if let Err(e) = d.draw(&frame) {
                    self.failed = Some(e);
                    self.window = None;
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}

fn palette() -> [[u8; 3]; 256] {
    let mut p = [[0; 3]; 256];
    p[..PALETTE.len()].copy_from_slice(&PALETTE);
    p
}

/// Printable ASCII, `' '` to `'~'`, 8 × 8: a row a byte, top first, its
/// lowest bit the leftmost pixel. font8x8's `font8x8_basic` (Daniel
/// Hepper, after Marcel Sondaar's from the IBM VGA font; public domain).
#[rustfmt::skip]
const FONT: [[u8; 8]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // ' '
    [0x18, 0x3C, 0x3C, 0x18, 0x18, 0x00, 0x18, 0x00], // '!'
    [0x36, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // '"'
    [0x36, 0x36, 0x7F, 0x36, 0x7F, 0x36, 0x36, 0x00], // '#'
    [0x0C, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x0C, 0x00], // '$'
    [0x00, 0x63, 0x33, 0x18, 0x0C, 0x66, 0x63, 0x00], // '%'
    [0x1C, 0x36, 0x1C, 0x6E, 0x3B, 0x33, 0x6E, 0x00], // '&'
    [0x06, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00], // "'"
    [0x18, 0x0C, 0x06, 0x06, 0x06, 0x0C, 0x18, 0x00], // '('
    [0x06, 0x0C, 0x18, 0x18, 0x18, 0x0C, 0x06, 0x00], // ')'
    [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00], // '*'
    [0x00, 0x0C, 0x0C, 0x3F, 0x0C, 0x0C, 0x00, 0x00], // '+'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x06], // ','
    [0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0x00], // '-'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x00], // '.'
    [0x60, 0x30, 0x18, 0x0C, 0x06, 0x03, 0x01, 0x00], // '/'
    [0x3E, 0x63, 0x73, 0x7B, 0x6F, 0x67, 0x3E, 0x00], // '0'
    [0x0C, 0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x3F, 0x00], // '1'
    [0x1E, 0x33, 0x30, 0x1C, 0x06, 0x33, 0x3F, 0x00], // '2'
    [0x1E, 0x33, 0x30, 0x1C, 0x30, 0x33, 0x1E, 0x00], // '3'
    [0x38, 0x3C, 0x36, 0x33, 0x7F, 0x30, 0x78, 0x00], // '4'
    [0x3F, 0x03, 0x1F, 0x30, 0x30, 0x33, 0x1E, 0x00], // '5'
    [0x1C, 0x06, 0x03, 0x1F, 0x33, 0x33, 0x1E, 0x00], // '6'
    [0x3F, 0x33, 0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x00], // '7'
    [0x1E, 0x33, 0x33, 0x1E, 0x33, 0x33, 0x1E, 0x00], // '8'
    [0x1E, 0x33, 0x33, 0x3E, 0x30, 0x18, 0x0E, 0x00], // '9'
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x00], // ':'
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x06], // ';'
    [0x18, 0x0C, 0x06, 0x03, 0x06, 0x0C, 0x18, 0x00], // '<'
    [0x00, 0x00, 0x3F, 0x00, 0x00, 0x3F, 0x00, 0x00], // '='
    [0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00], // '>'
    [0x1E, 0x33, 0x30, 0x18, 0x0C, 0x00, 0x0C, 0x00], // '?'
    [0x3E, 0x63, 0x7B, 0x7B, 0x7B, 0x03, 0x1E, 0x00], // '@'
    [0x0C, 0x1E, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x00], // 'A'
    [0x3F, 0x66, 0x66, 0x3E, 0x66, 0x66, 0x3F, 0x00], // 'B'
    [0x3C, 0x66, 0x03, 0x03, 0x03, 0x66, 0x3C, 0x00], // 'C'
    [0x1F, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1F, 0x00], // 'D'
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x46, 0x7F, 0x00], // 'E'
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x06, 0x0F, 0x00], // 'F'
    [0x3C, 0x66, 0x03, 0x03, 0x73, 0x66, 0x7C, 0x00], // 'G'
    [0x33, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x33, 0x00], // 'H'
    [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 'I'
    [0x78, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E, 0x00], // 'J'
    [0x67, 0x66, 0x36, 0x1E, 0x36, 0x66, 0x67, 0x00], // 'K'
    [0x0F, 0x06, 0x06, 0x06, 0x46, 0x66, 0x7F, 0x00], // 'L'
    [0x63, 0x77, 0x7F, 0x7F, 0x6B, 0x63, 0x63, 0x00], // 'M'
    [0x63, 0x67, 0x6F, 0x7B, 0x73, 0x63, 0x63, 0x00], // 'N'
    [0x1C, 0x36, 0x63, 0x63, 0x63, 0x36, 0x1C, 0x00], // 'O'
    [0x3F, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x0F, 0x00], // 'P'
    [0x1E, 0x33, 0x33, 0x33, 0x3B, 0x1E, 0x38, 0x00], // 'Q'
    [0x3F, 0x66, 0x66, 0x3E, 0x36, 0x66, 0x67, 0x00], // 'R'
    [0x1E, 0x33, 0x07, 0x0E, 0x38, 0x33, 0x1E, 0x00], // 'S'
    [0x3F, 0x2D, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 'T'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x00], // 'U'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00], // 'V'
    [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00], // 'W'
    [0x63, 0x63, 0x36, 0x1C, 0x1C, 0x36, 0x63, 0x00], // 'X'
    [0x33, 0x33, 0x33, 0x1E, 0x0C, 0x0C, 0x1E, 0x00], // 'Y'
    [0x7F, 0x63, 0x31, 0x18, 0x4C, 0x66, 0x7F, 0x00], // 'Z'
    [0x1E, 0x06, 0x06, 0x06, 0x06, 0x06, 0x1E, 0x00], // '['
    [0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00], // '\\'
    [0x1E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1E, 0x00], // ']'
    [0x08, 0x1C, 0x36, 0x63, 0x00, 0x00, 0x00, 0x00], // '^'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF], // '_'
    [0x0C, 0x0C, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00], // '`'
    [0x00, 0x00, 0x1E, 0x30, 0x3E, 0x33, 0x6E, 0x00], // 'a'
    [0x07, 0x06, 0x06, 0x3E, 0x66, 0x66, 0x3B, 0x00], // 'b'
    [0x00, 0x00, 0x1E, 0x33, 0x03, 0x33, 0x1E, 0x00], // 'c'
    [0x38, 0x30, 0x30, 0x3E, 0x33, 0x33, 0x6E, 0x00], // 'd'
    [0x00, 0x00, 0x1E, 0x33, 0x3F, 0x03, 0x1E, 0x00], // 'e'
    [0x1C, 0x36, 0x06, 0x0F, 0x06, 0x06, 0x0F, 0x00], // 'f'
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x1F], // 'g'
    [0x07, 0x06, 0x36, 0x6E, 0x66, 0x66, 0x67, 0x00], // 'h'
    [0x0C, 0x00, 0x0E, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 'i'
    [0x30, 0x00, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E], // 'j'
    [0x07, 0x06, 0x66, 0x36, 0x1E, 0x36, 0x67, 0x00], // 'k'
    [0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00], // 'l'
    [0x00, 0x00, 0x33, 0x7F, 0x7F, 0x6B, 0x63, 0x00], // 'm'
    [0x00, 0x00, 0x1F, 0x33, 0x33, 0x33, 0x33, 0x00], // 'n'
    [0x00, 0x00, 0x1E, 0x33, 0x33, 0x33, 0x1E, 0x00], // 'o'
    [0x00, 0x00, 0x3B, 0x66, 0x66, 0x3E, 0x06, 0x0F], // 'p'
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x78], // 'q'
    [0x00, 0x00, 0x3B, 0x6E, 0x66, 0x06, 0x0F, 0x00], // 'r'
    [0x00, 0x00, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x00], // 's'
    [0x08, 0x0C, 0x3E, 0x0C, 0x0C, 0x2C, 0x18, 0x00], // 't'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x33, 0x6E, 0x00], // 'u'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00], // 'v'
    [0x00, 0x00, 0x63, 0x6B, 0x7F, 0x7F, 0x36, 0x00], // 'w'
    [0x00, 0x00, 0x63, 0x36, 0x1C, 0x36, 0x63, 0x00], // 'x'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x3E, 0x30, 0x1F], // 'y'
    [0x00, 0x00, 0x3F, 0x19, 0x0C, 0x26, 0x3F, 0x00], // 'z'
    [0x38, 0x0C, 0x0C, 0x07, 0x0C, 0x0C, 0x38, 0x00], // '{'
    [0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x18, 0x00], // '|'
    [0x07, 0x0C, 0x0C, 0x38, 0x0C, 0x0C, 0x07, 0x00], // '}'
    [0x6E, 0x3B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // '~'
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps() {
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap("a /long/path/name b", 6), ["a", "/long/", "path/n", "ame b"]);
        assert_eq!(wrap("", 5), [""]);
    }
}
