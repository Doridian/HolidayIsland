//! Forms and their components, built from the DFMs in the executable.

use std::fmt;

use formats::dfm::{Object, Value};

use crate::color::Color;
use crate::surface::Surface;
use crate::{Palette, Rect};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A component class the subset doesn't have.
    UnknownClass(Vec<u8>),
    /// A property with a value of the wrong type, or an unknown colour.
    BadProperty { component: Vec<u8>, property: Vec<u8> },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        match self {
            Error::UnknownClass(c) => write!(f, "unknown component class {}", s(c)),
            Error::BadProperty { component, property } => {
                write!(f, "bad value for {}.{}", s(component), s(property))
            }
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Form {
        color: Color,
        stay_on_top: bool,
        maximized: bool,
    },
    Image {
        /// `Picture.Bitmap`; the game loads all of them at runtime.
        picture: Option<Surface>,
    },
    Panel {
        color: Color,
        /// `BevelOuter` isn't `bvNone`. (`bvLowered` isn't used.)
        raised: bool,
    },
    Timer {
        interval: u32,
    },
    MediaPlayer {
        /// The component the video plays in.
        display: Option<Vec<u8>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub name: Vec<u8>,
    /// The class name in the DFM, e.g. `TStartform` for the form.
    pub class: Vec<u8>,
    pub kind: Kind,
    /// Relative to the parent; for a form, on the screen (the client area, as
    /// every form is borderless).
    pub bounds: Rect,
    pub visible: bool,
    pub enabled: bool,
    pub tag: i32,
    /// Event (`OnMouseDown`) and handler method name (`BildMouseDown`).
    pub events: Vec<(Vec<u8>, Vec<u8>)>,
    /// Index of the parent in [`Form::components`]; `None` for the form.
    pub parent: Option<usize>,
}

impl Component {
    /// `TWinControl`s have their own window, so they cover the graphic
    /// controls (`TImage`) of the same parent whatever the creation order.
    #[must_use]
    pub fn is_windowed(&self) -> bool {
        matches!(
            self.kind,
            Kind::Form { .. } | Kind::Panel { .. } | Kind::MediaPlayer { .. }
        )
    }

    /// Whether the component is drawn at all.
    fn is_control(&self) -> bool {
        !matches!(self.kind, Kind::Timer { .. })
    }

    /// The handler for `event`, if one is assigned.
    #[must_use]
    pub fn handler(&self, event: &str) -> Option<&[u8]> {
        self.events
            .iter()
            .find(|(e, _)| e == event.as_bytes())
            .map(|(_, h)| &h[..])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// The form first, then its components depth first in DFM (creation)
    /// order.
    pub components: Vec<Component>,
}

struct Props<'a> {
    object: &'a Object,
}

impl Props<'_> {
    fn bad(&self, property: &str) -> Error {
        Error::BadProperty {
            component: self.object.name.clone(),
            property: property.as_bytes().to_vec(),
        }
    }

    fn int(&self, name: &str, default: i32) -> Result<i32, Error> {
        self.object
            .property(name)
            .map_or(Ok(default), |v| v.as_int().ok_or_else(|| self.bad(name)))
    }

    fn bool(&self, name: &str, default: bool) -> Result<bool, Error> {
        self.object
            .property(name)
            .map_or(Ok(default), |v| v.as_bool().ok_or_else(|| self.bad(name)))
    }

    fn ident(&self, name: &str) -> Result<Option<&[u8]>, Error> {
        self.object
            .property(name)
            .map(|v| match v {
                Value::Ident(s) => Ok(&s[..]),
                _ => Err(self.bad(name)),
            })
            .transpose()
    }

    fn color(&self, name: &str) -> Result<Color, Error> {
        match self.object.property(name) {
            None => Ok(Color::BTN_FACE),
            Some(Value::Ident(s)) => Color::from_ident(s).ok_or_else(|| self.bad(name)),
            Some(Value::Int(n)) => Ok(Color(n.cast_unsigned())),
            Some(_) => Err(self.bad(name)),
        }
    }
}

impl Form {
    /// Builds a form from its DFM, with every property the DFM doesn't set at
    /// the VCL's default.
    ///
    /// # Errors
    /// If the DFM uses a class or value outside the subset.
    pub fn load(dfm: &Object) -> Result<Self, Error> {
        let mut form = Form { components: Vec::new() };
        form.add(dfm, None)?;
        Ok(form)
    }

    fn add(&mut self, object: &Object, parent: Option<usize>) -> Result<(), Error> {
        let p = Props { object };
        let left = p.int("Left", 0)?;
        let top = p.int("Top", 0)?;
        let (kind, bounds, visible) = if parent.is_none() {
            let kind = Kind::Form {
                color: p.color("Color")?,
                stay_on_top: p.ident("FormStyle")? == Some(b"fsStayOnTop"),
                maximized: p.ident("WindowState")? == Some(b"wsMaximized"),
            };
            let bounds = Rect::new(left, top, p.int("ClientWidth", 0)?, p.int("ClientHeight", 0)?);
            (kind, bounds, p.bool("Visible", false)?)
        } else {
            let kind = match &object.class[..] {
                b"TImage" => Kind::Image { picture: None },
                b"TPanel" => Kind::Panel {
                    color: p.color("Color")?,
                    raised: p.ident("BevelOuter")? != Some(b"bvNone"),
                },
                b"TTimer" => Kind::Timer {
                    interval: u32::try_from(p.int("Interval", 1000)?).map_err(|_| p.bad("Interval"))?,
                },
                b"TMediaPlayer" => Kind::MediaPlayer {
                    display: p.ident("Display")?.map(<[u8]>::to_vec),
                },
                other => return Err(Error::UnknownClass(other.to_vec())),
            };
            // TMediaPlayer's default size is 253×30; the others are stored.
            let bounds = Rect::new(left, top, p.int("Width", 0)?, p.int("Height", 0)?);
            (kind, bounds, p.bool("Visible", true)?)
        };
        let events = object
            .properties
            .iter()
            .filter(|(name, _)| name.starts_with(b"On"))
            .map(|(name, v)| match v {
                Value::Ident(h) => Ok((name.clone(), h.clone())),
                _ => Err(p.bad("On…")),
            })
            .collect::<Result<_, _>>()?;
        let index = self.components.len();
        self.components.push(Component {
            name: object.name.clone(),
            class: object.class.clone(),
            kind,
            bounds,
            visible,
            enabled: p.bool("Enabled", true)?,
            tag: p.int("Tag", 0)?,
            events,
            parent,
        });
        for child in &object.children {
            self.add(child, Some(index))?;
        }
        Ok(())
    }

    /// The form itself.
    #[must_use]
    pub fn form(&self) -> &Component {
        &self.components[0]
    }

    /// The component called `name`.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<usize> {
        self.components.iter().position(|c| c.name == name.as_bytes())
    }

    #[must_use]
    pub fn component(&self, name: &str) -> Option<&Component> {
        self.find(name).map(|i| &self.components[i])
    }

    pub fn component_mut(&mut self, name: &str) -> Option<&mut Component> {
        self.find(name).map(|i| &mut self.components[i])
    }

    /// Draws the form onto the screen if it's visible.
    pub fn paint(&self, screen: &mut Surface, palette: &Palette) {
        if self.form().visible {
            self.paint_component(0, 0, 0, screen.bounds(), screen, palette);
        }
    }

    fn paint_component(&self, index: usize, x: i32, y: i32, clip: Rect, screen: &mut Surface, palette: &Palette) {
        let comp = &self.components[index];
        let rect = comp.bounds.offset(x, y);
        let clip = clip.intersect(&rect);
        if clip.is_empty() {
            return;
        }
        match &comp.kind {
            Kind::Form { color, .. } => screen.fill_rect(rect, clip, color.index(palette)),
            Kind::Panel { color, raised } => {
                screen.fill_rect(rect, clip, color.index(palette));
                if *raised {
                    bevel(screen, rect, clip, palette);
                }
            }
            // TImage draws the picture at its top left, unstretched.
            Kind::Image { picture: Some(p) } => screen.draw(rect.left, rect.top, p, clip),
            // The media players' buttons aren't drawn: every one the game
            // shows is hidden.
            Kind::Image { picture: None } | Kind::Timer { .. } | Kind::MediaPlayer { .. } => {}
        }
        let children = || {
            self.components
                .iter()
                .enumerate()
                .filter(move |(_, ch)| ch.parent == Some(index) && ch.visible && ch.is_control())
        };
        for windowed in [false, true] {
            for (child, _) in children().filter(|(_, ch)| ch.is_windowed() == windowed) {
                self.paint_component(child, rect.left, rect.top, clip, screen, palette);
            }
        }
    }
}

/// A one-pixel raised bevel.
fn bevel(screen: &mut Surface, r: Rect, clip: Rect, palette: &Palette) {
    let light = Color::BTN_HIGHLIGHT.index(palette);
    let dark = Color::BTN_SHADOW.index(palette);
    screen.fill_rect(Rect::new(r.left, r.top, r.width(), 1), clip, light);
    screen.fill_rect(Rect::new(r.left, r.top, 1, r.height()), clip, light);
    screen.fill_rect(Rect::new(r.left, r.bottom - 1, r.width(), 1), clip, dark);
    screen.fill_rect(Rect::new(r.right - 1, r.top, 1, r.height()), clip, dark);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    fn object(class: &str, name: &str, props: Vec<(&str, Value)>, children: Vec<Object>) -> Object {
        Object {
            class: class.as_bytes().to_vec(),
            name: name.as_bytes().to_vec(),
            properties: props.into_iter().map(|(n, v)| (n.as_bytes().to_vec(), v)).collect(),
            children,
        }
    }

    #[test]
    fn windowed_controls_cover_images() {
        let ident = |s: &str| Value::Ident(s.as_bytes().to_vec());
        let dfm = object(
            "TTest",
            "Test",
            vec![
                ("ClientWidth", Value::Int(4)),
                ("ClientHeight", Value::Int(1)),
                ("Color", ident("clBlack")),
                ("Visible", Value::Bool(true)),
            ],
            vec![
                object(
                    "TPanel",
                    "Panel",
                    vec![
                        ("Width", Value::Int(2)),
                        ("Height", Value::Int(1)),
                        ("Color", ident("clWhite")),
                        ("BevelOuter", ident("bvNone")),
                    ],
                    vec![],
                ),
                object(
                    "TImage",
                    "Bild",
                    vec![
                        ("Left", Value::Int(1)),
                        ("Width", Value::Int(3)),
                        ("Height", Value::Int(1)),
                        ("OnMouseDown", ident("BildMouseDown")),
                    ],
                    vec![],
                ),
                object("TTimer", "Timer1", vec![("Interval", Value::Int(10))], vec![]),
            ],
        );
        let mut form = Form::load(&dfm).unwrap();
        assert_eq!(form.component("Timer1").unwrap().kind, Kind::Timer { interval: 10 });
        assert_eq!(
            form.component("Bild").unwrap().handler("OnMouseDown"),
            Some(&b"BildMouseDown"[..])
        );
        let Kind::Image { picture } = &mut form.component_mut("Bild").unwrap().kind else {
            panic!("not an image");
        };
        *picture = Some(Surface::new(2, 1, 7));

        let mut palette = [Rgb::default(); 256];
        palette[1] = Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        };
        let mut screen = Surface::new(5, 1, 9);
        form.paint(&mut screen, &palette);
        // Black form, white panel over the image, image, form, and off-form.
        assert_eq!(screen.pixels, [1, 1, 7, 0, 9]);
    }

    #[test]
    fn unknown_class() {
        let dfm = object("TForm", "F", vec![], vec![object("TButton", "B", vec![], vec![])]);
        assert_eq!(Form::load(&dfm), Err(Error::UnknownClass(b"TButton".to_vec())));
    }
}
