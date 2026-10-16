//! The window, input and display. The game hands over an 8-bit framebuffer
//! and a palette each frame; the GPU does the palette lookup and scales the
//! picture to the window by a whole factor where it fits, letterboxed.

mod display;
mod keys;

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

pub use display::Error;

/// How often the game is updated and redrawn.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// What the game shows: palette indices, top row first, and 256 RGB colours.
pub struct Frame<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
    pub palette: &'a [[u8; 3]; 256],
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
}

struct Runner<A> {
    app: A,
    title: String,
    window: Option<(Arc<Window>, display::Display)>,
    last: Instant,
    mouse: (i32, i32),
    error: Option<Error>,
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
}

impl<A: App> ApplicationHandler for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let f = self.app.frame();
        let attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(PhysicalSize::new(f.width, f.height));
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
            }
        }
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
                if let Some((_, d)) = &mut self.window {
                    d.resize(size);
                }
            }
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
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(vk) = keys::virtual_key(code)
                {
                    self.app.input(match event.state {
                        ElementState::Pressed => Input::KeyDown(vk),
                        ElementState::Released => Input::KeyUp(vk),
                    });
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

/// Opens a window and runs `app` until it finishes or the event loop ends.
///
/// # Errors
/// If no window or GPU device can be set up.
pub fn run<A: App>(title: &str, app: A) -> Result<(), Error> {
    let event_loop = EventLoop::new().map_err(|e| Error::Window(e.to_string()))?;
    let mut runner = Runner {
        app,
        title: title.to_owned(),
        window: None,
        last: Instant::now(),
        mouse: (0, 0),
        error: None,
    };
    event_loop
        .run_app(&mut runner)
        .map_err(|e| Error::Window(e.to_string()))?;
    runner.error.map_or(Ok(()), Err)
}
