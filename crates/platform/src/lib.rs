//! The window, input and display. The game hands over an 8-bit framebuffer
//! and a palette each frame, and a true-colour picture over part of it
//! (a film); the GPU does the palette lookup and scales the picture to
//! the window by a whole factor where it fits, letterboxed.
//! [`audio`] plays the sound.
//!
//! The window is resizable. Its client area divided by the UI scale (1
//! to [`MAX_UI_SCALE`], from the display's scale factor unless it's
//! given) is the desktop the game lays out for ([`App::resize`]), as the
//! original laid out for the Windows desktop. F11 or Alt+Enter (and
//! Ctrl+Cmd+F on macOS) toggle borderless fullscreen; those keys don't
//! reach the game. The window's size, position and fullscreen state are
//! remembered in the port's settings file ([`settings`]).

pub mod audio;
mod display;
mod keys;
pub mod settings;
pub mod storage;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

pub use display::Error;
use settings::WindowSettings;

/// The smallest desktop the game lays out for, the original's minimum.
/// A smaller window gets a framebuffer this big, letterboxed.
pub const MIN_DESKTOP: (u32, u32) = (640, 480);
/// The largest UI scale.
pub const MAX_UI_SCALE: u32 = 4;
/// The window's client area when neither the command line nor the
/// settings file gives one, in desktop pixels: the original's
/// recommended 800×600.
pub const DEFAULT_DESKTOP: (u32, u32) = (800, 600);

/// How often the game is updated and redrawn.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// What the game shows: palette indices, top row first, and 256 RGB colours.
pub struct Frame<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
    pub palette: &'a [[u8; 3]; 256],
    /// A true-colour picture on top.
    pub overlay: Option<Overlay<'a>>,
}

/// RGB pixels, top row first, `width` × `height` at `x`, `y` in the
/// framebuffer (a film MCI draws into a window, past the palette).
#[derive(Debug, Clone, Copy)]
pub struct Overlay<'a> {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub rgb: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// Input in framebuffer coordinates, which may lie outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        x: i32,
        y: i32,
        button: Button,
    },
    MouseUp {
        x: i32,
        y: i32,
        button: Button,
    },
    /// A Windows virtual-key code (`VK_*`), as `OnKeyDown` gets it.
    KeyDown(u16),
    KeyUp(u16),
    /// The window's close button.
    Close,
}

pub trait App {
    /// Advances the game by `elapsed`.
    fn update(&mut self, elapsed: Duration);
    fn input(&mut self, input: Input);
    fn frame(&self) -> Frame<'_>;
    /// Whether the game has quit.
    fn finished(&self) -> bool;
    /// The desktop the game lays out for is now `width` × `height`: the
    /// window's client area divided by the UI scale. Called once before
    /// the first frame and after every change. It may be smaller than
    /// [`MIN_DESKTOP`]; the frame may be bigger than it, and is then
    /// letterboxed.
    fn resize(&mut self, width: u32, height: u32);
}

/// How the window opens; what isn't given comes from the settings file.
#[derive(Debug, Clone, Default)]
pub struct WindowOptions {
    /// The client area's size in logical pixels (`--window-size=WxH`).
    pub size: Option<(u32, u32)>,
    /// Fullscreen or not (`--fullscreen`, `--windowed`).
    pub fullscreen: Option<bool>,
    /// A fixed UI scale (`--ui-scale=N`) instead of the display's.
    pub ui_scale: Option<u32>,
    /// The settings file the window's state is read from and written back
    /// to when the game ends; none to remember nothing.
    pub settings_file: Option<PathBuf>,
}

/// The UI scale for a client area `client` (physical pixels) on a display
/// with `scale_factor`: `fixed` if given (1 to [`MAX_UI_SCALE`]), else
/// the scale factor rounded, made smaller while the desktop it leaves
/// would be smaller than [`MIN_DESKTOP`].
#[must_use]
pub fn ui_scale(client: (u32, u32), scale_factor: f64, fixed: Option<u32>) -> u32 {
    if let Some(n) = fixed {
        return n.clamp(1, MAX_UI_SCALE);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut scale = (scale_factor.round().clamp(1.0, f64::from(MAX_UI_SCALE))) as u32;
    while scale > 1 && (client.0 / scale < MIN_DESKTOP.0 || client.1 / scale < MIN_DESKTOP.1) {
        scale -= 1;
    }
    scale
}

/// The desktop a client area `client` gives at UI scale `scale`.
#[must_use]
pub fn desktop(client: (u32, u32), scale: u32) -> (u32, u32) {
    let scale = scale.max(1);
    (client.0 / scale, client.1 / scale)
}

/// Whether `code` with `modifiers` toggles fullscreen: F11, Alt+Enter, and
/// Ctrl+Cmd+F on macOS.
fn fullscreen_key(code: KeyCode, modifiers: ModifiersState) -> bool {
    match code {
        KeyCode::F11 => true,
        KeyCode::Enter | KeyCode::NumpadEnter => modifiers.alt_key(),
        KeyCode::KeyF => cfg!(target_os = "macos") && modifiers.control_key() && modifiers.super_key(),
        _ => false,
    }
}

struct Runner<A> {
    app: A,
    title: String,
    options: WindowOptions,
    window: Option<(Arc<Window>, display::Display)>,
    last: Instant,
    mouse: (i32, i32),
    error: Option<Error>,
    /// The desktop last handed to [`App::resize`].
    desktop: (u32, u32),
    modifiers: ModifiersState,
    /// Keys whose press toggled fullscreen, whose release the game
    /// doesn't get either.
    swallowed: Vec<KeyCode>,
    /// What the settings file gets at the end: the windowed size and
    /// position, and whether the window is fullscreen.
    remembered: WindowSettings,
}

impl<A: App> Runner<A> {
    fn to_frame(&self, pos: PhysicalPosition<f64>) -> (i32, i32) {
        let Some((window, _)) = &self.window else {
            return (0, 0);
        };
        let f = self.app.frame();
        let v = display::View::new(window.inner_size(), f.width, f.height);
        #[allow(clippy::cast_possible_truncation)]
        let at = |p: f64, offset: f32| ((p - f64::from(offset)) / f64::from(v.scale)).floor() as i32;
        (at(pos.x, v.offset[0]), at(pos.y, v.offset[1]))
    }

    /// Works out the desktop from the window's size and tells the game if
    /// it changed.
    fn update_desktop(&mut self) {
        let Some((window, _)) = &self.window else { return };
        let size = window.inner_size();
        let client = (size.width, size.height);
        let scale = ui_scale(client, window.scale_factor(), self.options.ui_scale);
        let desktop = desktop(client, scale);
        if desktop != self.desktop && desktop.0 > 0 && desktop.1 > 0 {
            self.desktop = desktop;
            self.app.resize(desktop.0, desktop.1);
            window.request_redraw();
        }
    }

    fn toggle_fullscreen(&mut self) {
        let Some((window, _)) = &self.window else { return };
        let full = window.fullscreen().is_none();
        window.set_fullscreen(full.then_some(Fullscreen::Borderless(None)));
        self.remembered.fullscreen = full;
    }

    /// A key, unless it toggles fullscreen.
    fn key(&mut self, code: KeyCode, pressed: bool, repeat: bool) {
        if pressed && fullscreen_key(code, self.modifiers) {
            if !repeat {
                self.toggle_fullscreen();
            }
            if !self.swallowed.contains(&code) {
                self.swallowed.push(code);
            }
            return;
        }
        if !pressed && let Some(i) = self.swallowed.iter().position(|&c| c == code) {
            self.swallowed.swap_remove(i);
            return;
        }
        if let Some(vk) = keys::virtual_key(code) {
            self.app
                .input(if pressed { Input::KeyDown(vk) } else { Input::KeyUp(vk) });
        }
    }
}

impl<A: App> ApplicationHandler for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let saved = self
            .options
            .settings_file
            .as_deref()
            .map(WindowSettings::load)
            .unwrap_or_default();
        // By default the desktop is 800×600: at a fixed UI scale that many
        // pixels times the scale, else that many points.
        let size = self.options.size.or(saved.size).unwrap_or_else(|| {
            let Some(n) = self.options.ui_scale else {
                return DEFAULT_DESKTOP;
            };
            let factor = event_loop.primary_monitor().map_or(1.0, |m| m.scale_factor()).max(0.25);
            let n = f64::from(n.clamp(1, MAX_UI_SCALE));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let points = |px: u32| (f64::from(px) * n / factor).ceil() as u32;
            (points(DEFAULT_DESKTOP.0), points(DEFAULT_DESKTOP.1))
        });
        let fullscreen = self.options.fullscreen.unwrap_or(saved.fullscreen);
        self.remembered = WindowSettings {
            size: Some(size),
            position: saved.position,
            fullscreen,
        };
        let mut attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(size.0, size.1))
            .with_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        if let Some((x, y)) = saved.position {
            attrs = attrs.with_position(PhysicalPosition::new(x, y));
        }
        let result = event_loop
            .create_window(attrs)
            .map_err(|e| Error::Window(e.to_string()))
            .and_then(|w| {
                let w = Arc::new(w);
                let d = display::Display::new(event_loop, w.clone())?;
                Ok((w, d))
            });
        match result {
            Ok(w) => self.window = Some(w),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
        }
        self.update_desktop();
        self.last = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let button = |b| match b {
            MouseButton::Right => Some(Button::Right),
            MouseButton::Middle => Some(Button::Middle),
            MouseButton::Left => Some(Button::Left),
            _ => None,
        };
        match event {
            WindowEvent::CloseRequested => self.app.input(Input::Close),
            WindowEvent::Resized(size) => {
                if let Some((w, d)) = &mut self.window {
                    d.resize(size);
                    if w.fullscreen().is_none() && size.width > 0 && size.height > 0 {
                        let logical: LogicalSize<u32> = size.to_logical(w.scale_factor());
                        self.remembered.size = Some((logical.width, logical.height));
                    }
                }
                self.update_desktop();
            }
            WindowEvent::ScaleFactorChanged { .. } => self.update_desktop(),
            WindowEvent::Moved(position) => {
                if let Some((w, _)) = &self.window
                    && w.fullscreen().is_none()
                {
                    self.remembered.position = Some((position.x, position.y));
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = self.to_frame(position);
                if (x, y) != self.mouse {
                    self.mouse = (x, y);
                    self.app.input(Input::MouseMove { x, y });
                }
            }
            WindowEvent::MouseInput { state, button: b, .. } => {
                if let Some(button) = button(b) {
                    let (x, y) = self.mouse;
                    self.app.input(match state {
                        ElementState::Pressed => Input::MouseDown { x, y, button },
                        ElementState::Released => Input::MouseUp { x, y, button },
                    });
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed, event.repeat);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some((_, d)) = &mut self.window
                    && let Err(e) = d.draw(&self.app.frame())
                {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            _ => {}
        }
        if self.app.finished() {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.last + FRAME {
            self.app.update(now - self.last);
            self.last = now;
            if let Some((w, _)) = &self.window {
                w.request_redraw();
            }
        }
        if self.app.finished() {
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.last + FRAME));
    }
}

/// Opens a window as `options` say and runs `app` until it finishes or
/// the event loop ends, then remembers the window in the settings file.
///
/// # Errors
/// If no window or GPU device can be set up.
pub fn run<A: App>(title: &str, options: WindowOptions, app: A) -> Result<(), Error> {
    let event_loop = EventLoop::new().map_err(|e| Error::Window(e.to_string()))?;
    let mut runner = Runner {
        app,
        title: title.to_owned(),
        options,
        window: None,
        last: Instant::now(),
        mouse: (0, 0),
        error: None,
        desktop: (0, 0),
        modifiers: ModifiersState::empty(),
        swallowed: Vec::new(),
        remembered: WindowSettings::default(),
    };
    event_loop
        .run_app(&mut runner)
        .map_err(|e| Error::Window(e.to_string()))?;
    if let Some(path) = &runner.options.settings_file
        && runner.remembered.size.is_some()
        && let Err(e) = runner.remembered.save(path)
    {
        eprintln!("{}: {e}", path.display());
    }
    runner.error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_scales() {
        assert_eq!(ui_scale((1600, 1200), 2.0, None), 2);
        assert_eq!(ui_scale((1600, 1200), 1.0, None), 1);
        // Too small for 640×480 at 2: 1.
        assert_eq!(ui_scale((1200, 900), 2.0, None), 1);
        assert_eq!(ui_scale((3840, 2160), 3.0, None), 3);
        assert_eq!(ui_scale((3840, 2160), 9.0, None), 4);
        assert_eq!(ui_scale((800, 600), 1.0, Some(2)), 2);
        assert_eq!(ui_scale((800, 600), 1.0, Some(7)), 4);
        assert_eq!(desktop((1366, 700), 1), (1366, 700));
        assert_eq!(desktop((2733, 1401), 2), (1366, 700));
    }

    #[test]
    fn fullscreen_keys() {
        assert!(fullscreen_key(KeyCode::F11, ModifiersState::empty()));
        assert!(fullscreen_key(KeyCode::Enter, ModifiersState::ALT));
        assert!(!fullscreen_key(KeyCode::Enter, ModifiersState::empty()));
        assert!(!fullscreen_key(KeyCode::KeyF, ModifiersState::empty()));
    }
}
