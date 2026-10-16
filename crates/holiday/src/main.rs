//! Holiday Island. For now this shows the main form with the title picture
//! and then the background, like the end of the intro (`docs/startup.md`,
//! states 430–450).
//!
//! `holiday [GAME_DIR]`, where `GAME_DIR` holds `HOLIDAY.EXE` and `GRAFIK`
//! (default `orig`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use formats::bmp::Bitmap;
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use platform::{App, Frame, Input};
use vcl::form::{Form, Kind};
use vcl::surface::Surface;
use vcl::{Palette, Rect};

/// The layout `hintergrund_timer` picks for an 800×600 screen.
const SCREEN: (usize, usize) = (800, 600);
/// How long the title stays up (state 440's interval).
const TITLE_TIME: Duration = Duration::from_secs(3);
const VK_ESCAPE: u16 = 0x1b;

struct Intro {
    form: Form,
    background: Bitmap,
    palette: Palette,
    rgb: [[u8; 3]; 256],
    screen: Surface,
    elapsed: Duration,
    showing_title: bool,
    quit: bool,
}

impl Intro {
    fn show(&mut self, bitmap: &Bitmap) {
        self.palette = bitmap.palette;
        self.rgb = bitmap.palette.map(|c| [c.r, c.g, c.b]);
        if let Some(image) = self.form.component_mut("backgroundimage")
            && let Kind::Image { picture } = &mut image.kind
        {
            *picture = Some(Surface::from_bitmap(bitmap));
        }
        self.screen.pixels.fill(0);
        self.form.paint(&mut self.screen, &self.palette);
    }
}

impl App for Intro {
    fn update(&mut self, elapsed: Duration) {
        self.elapsed += elapsed;
        if self.showing_title && self.elapsed >= TITLE_TIME {
            self.showing_title = false;
            let background = self.background.clone();
            self.show(&background);
        }
    }

    fn input(&mut self, input: Input) {
        if matches!(input, Input::Close | Input::KeyDown(VK_ESCAPE)) {
            self.quit = true;
        }
    }

    fn frame(&self) -> Frame<'_> {
        Frame {
            width: u32::try_from(self.screen.width).expect("screen width"),
            height: u32::try_from(self.screen.height).expect("screen height"),
            pixels: &self.screen.pixels,
            palette: &self.rgb,
        }
    }

    fn finished(&self) -> bool {
        self.quit
    }
}

fn read(dir: &Path, name: &str) -> Vec<u8> {
    let path = dir.join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("can't read {}: {e}", path.display()))
}

fn bitmap(dir: &Path, name: &str) -> Bitmap {
    Bitmap::parse(&read(dir, name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("orig"), PathBuf::from);
    let exe = read(&dir, "HOLIDAY.EXE");
    let res = resources(&exe)
        .expect("HOLIDAY.EXE isn't an NE executable")
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(b"THINTERGRUND".to_vec()))
        .expect("no THintergrund form");
    let mut form = Form::load(&Object::parse(res.data).expect("bad form")).expect("bad form");

    // What states 110 and 400 do to the form and its image.
    let (w, h) = (i32::try_from(SCREEN.0).unwrap(), i32::try_from(SCREEN.1).unwrap());
    form.components[0].bounds = Rect::new(0, 0, w, h);
    form.components[0].visible = true;
    if let Some(image) = form.component_mut("backgroundimage") {
        image.bounds = Rect::new(0, 0, w, h);
        image.visible = true;
    }

    let title = bitmap(&dir, "GRAFIK/TIT80X60.BMP");
    let mut intro = Intro {
        form,
        background: bitmap(&dir, "GRAFIK/BCK80X60.BMP"),
        palette: title.palette,
        rgb: [[0; 3]; 256],
        screen: Surface::new(SCREEN.0, SCREEN.1, 0),
        elapsed: Duration::ZERO,
        showing_title: true,
        quit: false,
    };
    intro.show(&title);
    if let Err(e) = platform::run("Holiday Island", intro) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
