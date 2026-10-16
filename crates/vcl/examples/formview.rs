//! Renders a form from the executable with bitmaps loaded into its images,
//! and writes the screen as a PPM.
//!
//! `cargo run -p vcl --example formview -- EXE FORMCLASS OUT.ppm WxH [IMAGE=FILE.BMP]...`
//!
//! The screen palette is the first bitmap's. A maximized form fills the
//! screen; any other form is centred, as the game positions most of them.

use formats::bmp::Bitmap;
use formats::dfm::Object;
use formats::ne::{RT_RCDATA, ResId, resources};
use vcl::form::{Form, Kind};
use vcl::surface::Surface;
use vcl::{Palette, Rect, Rgb};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [exe, class, out, size, images @ ..] = &args[..] else {
        panic!("usage: formview EXE FORMCLASS OUT.ppm WxH [IMAGE=FILE.BMP]...");
    };
    let (w, h) = size.split_once('x').expect("size is WxH");
    let (w, h): (usize, usize) = (w.parse().unwrap(), h.parse().unwrap());

    let exe = std::fs::read(exe).expect("can't read the executable");
    let res = resources(&exe)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == ResId::Id(RT_RCDATA) && r.name == ResId::Name(class.to_ascii_uppercase().into_bytes()))
        .expect("no such form");
    let mut form = Form::load(&Object::parse(res.data).unwrap()).unwrap();

    let mut palette: Option<Palette> = None;
    for arg in images {
        let (name, file) = arg.split_once('=').expect("images are NAME=FILE");
        let bitmap = Bitmap::parse(&std::fs::read(file).expect("can't read the bitmap")).unwrap();
        palette.get_or_insert(bitmap.palette);
        let image = form.component_mut(name).expect("no such component");
        image.visible = true;
        let Kind::Image { picture } = &mut image.kind else {
            panic!("{name} isn't a TImage");
        };
        *picture = Some(Surface::from_bitmap(&bitmap));
    }
    let palette = palette.unwrap_or([Rgb::default(); 256]);

    let screen_w = i32::try_from(w).unwrap();
    let screen_h = i32::try_from(h).unwrap();
    let root = &mut form.components[0];
    root.visible = true;
    root.bounds = if matches!(root.kind, Kind::Form { maximized: true, .. }) {
        Rect::new(0, 0, screen_w, screen_h)
    } else {
        let (fw, fh) = (root.bounds.width(), root.bounds.height());
        Rect::new((screen_w - fw) / 2, (screen_h - fh) / 2, fw, fh)
    };
    // A maximized form's image is sized to the screen at runtime.
    if matches!(form.components[0].kind, Kind::Form { maximized: true, .. }) {
        for c in &mut form.components[1..] {
            if let Kind::Image { picture: Some(p) } = &c.kind {
                c.bounds = p.bounds();
            }
        }
    }

    let mut screen = Surface::new(w, h, 0);
    form.paint(&mut screen, &palette);
    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
    for &i in &screen.pixels {
        let c = palette[usize::from(i)];
        ppm.extend_from_slice(&[c.r, c.g, c.b]);
    }
    std::fs::write(out, ppm).expect("can't write the output");
}
