//! The game's own widget toolkit (segment `10c0`). The forms don't use VCL
//! controls for their buttons, check boxes and scroll bars: they draw
//! everything into one `TImage` and route its mouse and key events to these
//! widgets.
//!
//! The original is written with Turbo Pascal `object`s, not Delphi classes.
//! Each widget is a record embedded in the form, with its VMT's offset in
//! `DGROUP` stored at +9, and the form units declare a descendant per
//! instance just to override `Click` (VMT+8). The drawing and the sounds are
//! overrides too, shared by every instance and kept in segment `1010`
//! (`ui_button_draw` and friends); they're part of the widgets here.
//!
//! Instead of virtual calls, the event methods return an [`Outcome`]: whether
//! the widget wants repainting, whether the original would call `Click`, and
//! the sound it plays. Bitmaps are the form's `TBitmap` variables in the
//! original (the widget keeps a pointer to the variable); here they're
//! indices into the [`Skin`]'s list.
//!
//! The original draws immediately, in the middle of the event. The order of
//! sound, `Click` and drawing is noted on each method.

use vcl::Rect;
use vcl::surface::{Rop, Surface};

use crate::text::{Align, Font};

/// `play_sound`'s numbers for the sounds the widgets make. // 10a0:30b6
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// A button or scroll arrow goes down.
    Press = 8,
    /// A radio button, check box or list row is picked, or a scroll bar
    /// pages.
    Select = 9,
    /// A button or scroll arrow comes back up.
    Release = 10,
}

impl Sound {
    /// The number `play_sound` takes.
    #[must_use]
    pub fn number(self) -> i16 {
        self as i16
    }
}

/// What an event did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Outcome {
    /// The original method's `Boolean` result. Each method says what it
    /// means.
    pub handled: bool,
    /// The widget changed how it looks: call [`Widget::repaint`].
    pub repaint: bool,
    /// The original calls the widget's `Click` (VMT+8), which the form
    /// overrides.
    pub clicked: bool,
    pub sound: Option<Sound>,
}

/// Delphi's `TShiftState`, as the byte the mouse and key events pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShiftState(pub u8);

impl ShiftState {
    pub const SHIFT: ShiftState = ShiftState(1);
    pub const ALT: ShiftState = ShiftState(2);
    pub const CTRL: ShiftState = ShiftState(4);
    pub const LEFT: ShiftState = ShiftState(8);
    pub const RIGHT: ShiftState = ShiftState(0x10);
    pub const MIDDLE: ShiftState = ShiftState(0x20);
    pub const DOUBLE: ShiftState = ShiftState(0x40);

    #[must_use]
    pub fn contains(self, other: ShiftState) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for ShiftState {
    type Output = ShiftState;
    fn bitor(self, rhs: ShiftState) -> ShiftState {
        ShiftState(self.0 | rhs.0)
    }
}

/// One of the form's bitmaps, by its index in [`Skin::bitmaps`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitmapId(pub usize);

/// What the widgets draw with.
#[derive(Debug, Clone, Copy)]
pub struct Skin<'a> {
    pub font: &'a Font,
    pub bitmaps: &'a [Surface],
}

impl Skin<'_> {
    /// Copies the `width`×`height` top left of a bitmap to `x`, `y`, like
    /// every `BitBlt` in the toolkit. A missing bitmap draws nothing.
    fn blit(&self, dst: &mut Surface, id: BitmapId, (x, y): (i32, i32), width: i32, height: i32) {
        if let Some(src) = self.bitmaps.get(id.0) {
            let clip = dst.bounds();
            dst.blit(x, y, src, Rect::new(0, 0, width, height), Rop::Copy, clip);
        }
    }
}

/// `draw_text`'s alignment argument: 1 centres, 2 right-aligns, anything
/// else is left. // 1030:23d3
#[must_use]
pub fn align(n: i32) -> Align {
    match n {
        1 => Align::Center,
        2 => Align::Right,
        _ => Align::Left,
    }
}

/// The base object's fields: +0 top, +2 left, +4 width, +6 height (all
/// `Integer`) and +8 visible. `widget_init` // 10c0:0002
///
/// Constructors in the original start hidden. Hidden widgets ignore the
/// mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub visible: bool,
}

impl Frame {
    #[must_use]
    pub fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        Frame {
            left,
            top,
            width,
            height,
            visible: false,
        }
    }

    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::new(self.left, self.top, self.width, self.height)
    }

    /// `widget_hit`: visible, and `x`, `y` inside. The right and bottom
    /// edges count as inside, one pixel past the drawn area. // 10c0:006b
    #[must_use]
    pub fn hit(&self, x: i32, y: i32) -> bool {
        self.visible && y >= self.top && x >= self.left && y <= self.top + self.height && x <= self.left + self.width
    }

    /// `widget_erase`: copies the widget's rect from the same place in
    /// `background` (VMT+0x18). // 10c0:011d
    pub fn erase(&self, dst: &mut Surface, background: &Surface) {
        let clip = dst.bounds();
        dst.blit(self.left, self.top, background, self.rect(), Rop::Copy, clip);
    }
}

/// What every widget has: the base object's virtual methods.
pub trait Widget {
    fn frame(&self) -> &Frame;
    fn frame_mut(&mut self) -> &mut Frame;

    /// The widget's draw method (VMT+0x14). It draws whatever the visible
    /// flag says.
    fn draw(&self, dst: &mut Surface, skin: &Skin);

    /// What the original does when [`Outcome::repaint`] is set: draws again
    /// on top of the old picture. Only the edit field erases first.
    fn repaint(&self, dst: &mut Surface, _background: &Surface, skin: &Skin) {
        self.draw(dst, skin);
    }

    /// `widget_show` (VMT+0xc): draws the widget unless it's already
    /// visible, and makes it visible. // 10c0:00ba
    fn show(&mut self, dst: &mut Surface, skin: &Skin) {
        if !self.frame().visible {
            self.draw(dst, skin);
        }
        self.frame_mut().visible = true;
    }

    /// `widget_hide` (VMT+0x10): erases the widget from `background` if
    /// it's visible, and hides it. // 10c0:00e4
    fn hide(&mut self, dst: &mut Surface, background: &Surface) {
        if self.frame().visible {
            self.frame().erase(dst, background);
        }
        self.frame_mut().visible = false;
    }
}

macro_rules! frame_accessors {
    () => {
        fn frame(&self) -> &Frame {
            &self.frame
        }
        fn frame_mut(&mut self) -> &mut Frame {
            &mut self.frame
        }
    };
}

/// A line of text. `label_init` // 10c0:02a2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub frame: Frame,
    /// `string[255]` at +0xb.
    pub text: Vec<u8>,
    /// +0x10b
    pub align: Align,
    /// Colour row, +0x10d.
    pub colour: i32,
}

impl Label {
    #[must_use]
    pub fn new(frame: Frame, text: &[u8], align: Align, colour: i32) -> Self {
        Label {
            frame,
            text: pstring(text),
            align,
            colour,
        }
    }
}

impl Widget for Label {
    frame_accessors!();

    /// `ui_label_draw`: the text 2 pixels below the top, wrapped into the
    /// width. Nothing is cleared behind it. // 1010:152d
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        let f = &self.frame;
        skin.font
            .draw(dst, &self.text, (f.left, f.top + 2), f.width, self.align, self.colour);
    }
}

/// A bitmap filled from the left by a percentage: an "empty" bitmap with
/// part of a "full" one on top. Only used outside the start screen
/// (`1040:367e`). `gauge_init` // 10c0:0181
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gauge {
    pub frame: Frame,
    /// +0xb
    pub empty: BitmapId,
    /// +0xf
    pub full: BitmapId,
    /// `Longint` at +0x13, 0 to 100. The owner sets it.
    pub percent: i32,
}

impl Gauge {
    #[must_use]
    pub fn new(frame: Frame, empty: BitmapId, full: BitmapId) -> Self {
        Gauge {
            frame,
            empty,
            full,
            percent: 0,
        }
    }
}

impl Widget for Gauge {
    frame_accessors!();

    /// `gauge_draw`: `width * percent div 100` pixels of the full bitmap
    /// over the empty one. // 10c0:01dc
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        let f = &self.frame;
        skin.blit(dst, self.empty, (f.left, f.top), f.width, f.height);
        let filled = f.width.wrapping_mul(self.percent) / 100;
        skin.blit(dst, self.full, (f.left, f.top), filled, f.height);
    }
}

/// A push button: a bitmap for up and one for down, with a caption.
/// `button_init` // 10c0:083b
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub frame: Frame,
    /// +0xb
    pub pressed: bool,
    /// +0xc
    pub normal: BitmapId,
    /// +0x10
    pub down: BitmapId,
    /// `string[255]` at +0x14. The start screen sets its captions from
    /// `START.TXT` after construction.
    pub text: Vec<u8>,
    /// Colour row, +0x114.
    pub colour: i32,
}

impl Button {
    #[must_use]
    pub fn new(frame: Frame, normal: BitmapId, down: BitmapId, text: &[u8], colour: i32) -> Self {
        Button {
            frame,
            pressed: false,
            normal,
            down,
            text: pstring(text),
            colour,
        }
    }

    /// `ui_button_mouse_down` (VMT+0x1c) around `button_mouse_down`: a hit
    /// presses the button, redraws it and plays [`Sound::Press`]. It doesn't
    /// look at the pressed flag. `handled` is the hit. // 1010:14c1 10c0:0958
    pub fn mouse_down(&mut self, x: i32, y: i32) -> Outcome {
        let mut out = Outcome::default();
        if self.frame.hit(x, y) {
            self.pressed = true;
            out.handled = true;
            out.repaint = true;
            out.sound = Some(Sound::Press);
        }
        out
    }

    /// `ui_button_mouse_up` (VMT+0x20) around `button_mouse_up`: if the
    /// button is pressed, it comes up and is redrawn, then `Click` runs if
    /// the mouse is still on it, then [`Sound::Release`] plays. `handled`
    /// says it was pressed, clicked or not. // 1010:14f7 10c0:0998
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Outcome {
        let mut out = Outcome::default();
        if self.pressed {
            self.pressed = false;
            out.handled = true;
            out.repaint = true;
            out.clicked = self.frame.hit(x, y);
            out.sound = Some(Sound::Release);
        }
        out
    }
}

impl Widget for Button {
    frame_accessors!();

    /// `button_draw` and `ui_button_draw`: the up or down bitmap, then the
    /// caption centred in the width and vertically for a 14 pixel line;
    /// pressed, it moves a pixel right and down. // 10c0:08cb 1010:1432
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        let f = &self.frame;
        let bitmap = if self.pressed { self.down } else { self.normal };
        skin.blit(dst, bitmap, (f.left, f.top), f.width, f.height);
        let shift = i32::from(self.pressed);
        let y = (f.height - 14) / 2 + f.top + shift;
        skin.font.draw(
            dst,
            &self.text,
            (f.left + shift, y),
            f.width,
            Align::Center,
            self.colour,
        );
    }
}

/// A radio button: a square bitmap, off or on, and a text to its right. It
/// only switches itself on; the form switches the others off.
/// `radio_init` // 10c0:0559
///
/// It's a descendant of the label, with the label's text, alignment and
/// colour. The start screen uses eight of them, each with its own pair of
/// bitmaps, to pick a colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioButton {
    pub frame: Frame,
    pub text: Vec<u8>,
    pub align: Align,
    pub colour: i32,
    /// +0x10f
    pub on: bool,
    /// +0x110
    pub off_bitmap: BitmapId,
    /// +0x114
    pub on_bitmap: BitmapId,
}

impl RadioButton {
    #[must_use]
    pub fn new(label: Label, off_bitmap: BitmapId, on_bitmap: BitmapId) -> Self {
        RadioButton {
            frame: label.frame,
            text: label.text,
            align: label.align,
            colour: label.colour,
            on: false,
            off_bitmap,
            on_bitmap,
        }
    }

    /// `radio_mouse_down`: a hit on a button that's off switches it on,
    /// plays [`Sound::Select`], calls `Click` and redraws. `handled` says
    /// it switched on. // 10c0:0666
    pub fn mouse_down(&mut self, x: i32, y: i32) -> Outcome {
        if self.frame.hit(x, y) && !self.on {
            self.select()
        } else {
            Outcome::default()
        }
    }

    /// `radio_select`: switches on, even if it already is, with the sound,
    /// `Click` and a redraw. // 10c0:06c7
    pub fn select(&mut self) -> Outcome {
        self.on = true;
        Outcome {
            handled: true,
            repaint: true,
            clicked: true,
            sound: Some(Sound::Select),
        }
    }

    /// `radio_deselect`: switches off quietly, with a redraw if it was on
    /// and no `Click`. // 10c0:06fe
    pub fn deselect(&mut self) -> Outcome {
        let mut out = Outcome::default();
        if self.on {
            self.on = false;
            out.handled = true;
            out.repaint = true;
        }
        out
    }

    /// `radio_draw` and the text drawn by `ui_radio_draw`: the bitmap is
    /// `height` square; the text starts 16 pixels right of it, `width -
    /// height - 3` wide, centred for a 12 pixel line. // 10c0:05d6 1010:1572
    fn draw_in(&self, dst: &mut Surface, skin: &Skin, colour: i32) {
        let f = &self.frame;
        let bitmap = if self.on { self.on_bitmap } else { self.off_bitmap };
        skin.blit(dst, bitmap, (f.left, f.top), f.height, f.height);
        let at = (f.left + f.height + 16, (f.height - 12) / 2 + f.top);
        skin.font
            .draw(dst, &self.text, at, f.width - f.height - 3, self.align, colour);
    }
}

impl Widget for RadioButton {
    frame_accessors!();

    /// `ui_radio_draw` (and its twin `ui_radio_draw2`). // 1010:1572 1010:16b5
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        self.draw_in(dst, skin, self.colour);
    }
}

/// A check box: a radio button that toggles, with the text in another colour
/// when it's on. `checkbox_init` // 10c0:072c
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckBox {
    pub radio: RadioButton,
    /// Colour row of the text when checked, +0x118.
    pub checked_colour: i32,
}

impl CheckBox {
    #[must_use]
    pub fn new(label: Label, off_bitmap: BitmapId, on_bitmap: BitmapId, checked_colour: i32) -> Self {
        CheckBox {
            radio: RadioButton::new(label, off_bitmap, on_bitmap),
            checked_colour,
        }
    }

    #[must_use]
    pub fn checked(&self) -> bool {
        self.radio.on
    }

    /// `checkbox_mouse_down`: a hit toggles it. `handled` is the hit.
    /// // 10c0:0797
    pub fn mouse_down(&mut self, x: i32, y: i32) -> Outcome {
        if self.radio.frame.hit(x, y) {
            self.toggle()
        } else {
            Outcome::default()
        }
    }

    /// `checkbox_toggle`: flips it, plays [`Sound::Select`], calls `Click`
    /// and redraws. // 10c0:07fa
    pub fn toggle(&mut self) -> Outcome {
        self.radio.on = !self.radio.on;
        Outcome {
            handled: true,
            repaint: true,
            clicked: true,
            sound: Some(Sound::Select),
        }
    }
}

impl Widget for CheckBox {
    fn frame(&self) -> &Frame {
        &self.radio.frame
    }
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.radio.frame
    }

    /// `ui_checkbox_draw`. // 1010:15ea
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        let colour = if self.radio.on {
            self.checked_colour
        } else {
            self.radio.colour
        };
        self.radio.draw_in(dst, skin, colour);
    }
}

/// The parts of a scroll bar, from the top or left. The original keeps the
/// pressed one at +0x38 as 0–4, 5 for none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    BackArrow,
    PageBack,
    Thumb,
    PageForward,
    ForwardArrow,
}

/// A scroll bar's six bitmaps, in the constructor's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollBitmaps {
    /// Up or left arrow, +0xc.
    pub back: BitmapId,
    /// Down or right arrow, +0x10.
    pub forward: BitmapId,
    /// +0x14
    pub back_pressed: BitmapId,
    /// +0x18
    pub forward_pressed: BitmapId,
    /// +0x1c
    pub thumb: BitmapId,
    /// The trough between the arrows, +0x20.
    pub track: BitmapId,
}

/// A scroll bar: two arrows, a trough and a square thumb, all as thick as
/// the bar. `scrollbar_init` // 10c0:09eb
///
/// The arrows step on release, the trough pages on press, and the thumb
/// drags, but only while the mouse stays inside the bar. Nothing repeats
/// while a button is held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollBar {
    pub frame: Frame,
    /// +0xb, set when `height <= width`.
    pub horizontal: bool,
    pub bitmaps: ScrollBitmaps,
    /// `Longint`s at +0x24, +0x28 and +0x2c. The original starts them at
    /// 0, 100 and 1; the owner sets them.
    pub min: i32,
    pub max: i32,
    pub position: i32,
    /// +0x30, the arrows' step: 1.
    pub small_step: i32,
    /// +0x34, the trough's step: 10.
    pub large_step: i32,
    /// +0x38
    pub pressed: Option<Part>,
    /// Where in the thumb it was grabbed, along the bar (+0x39 across a
    /// horizontal bar, +0x3b down a vertical one).
    pub grab: i32,
}

impl ScrollBar {
    #[must_use]
    pub fn new(frame: Frame, bitmaps: ScrollBitmaps) -> Self {
        ScrollBar {
            frame,
            horizontal: frame.height <= frame.width,
            bitmaps,
            min: 0,
            max: 100,
            position: 1,
            small_step: 1,
            large_step: 10,
            pressed: None,
            grab: 0,
        }
    }

    /// The arrows and thumb are squares this big.
    fn thickness(&self) -> i32 {
        if self.horizontal {
            self.frame.height
        } else {
            self.frame.width
        }
    }

    fn length(&self) -> i32 {
        if self.horizontal {
            self.frame.width
        } else {
            self.frame.height
        }
    }

    /// How far the thumb can travel: the length less the two arrows and the
    /// thumb.
    fn travel(&self) -> i32 {
        self.length() - 3 * self.thickness()
    }

    /// The thumb's distance from the back arrow, in `Longint` arithmetic
    /// (`sys_long_mul`, `sys_long_div`). Only called when `max != min`.
    fn thumb_offset(&self) -> i32 {
        let range = self.max.wrapping_sub(self.min);
        self.travel()
            .wrapping_mul(self.position.wrapping_sub(self.min))
            .checked_div(range)
            .unwrap_or(0)
    }

    /// The mouse's distance along the bar from its top or left.
    fn along(&self, x: i32, y: i32) -> i32 {
        if self.horizontal {
            x - self.frame.left
        } else {
            y - self.frame.top
        }
    }

    /// Which part is `along` pixels down or across: each part's far edge
    /// still belongs to it. Shared by `scrollbar_mouse_down` and
    /// `scrollbar_mouse_up`.
    fn part_at(&self, along: i32) -> Part {
        let thick = self.thickness();
        let thumb = self.thumb_offset();
        if along <= thick {
            Part::BackArrow
        } else if along <= thick + thumb {
            Part::PageBack
        } else if along <= 2 * thick + thumb {
            Part::Thumb
        } else if along > self.length() - thick {
            Part::ForwardArrow
        } else {
            Part::PageForward
        }
    }

    /// Moves towards `min` by `by`, stopping there. The sign of `by` isn't
    /// checked, in this and [`ScrollBar::forward`].
    fn back(&mut self, by: i32) {
        self.position = self.position.wrapping_sub(by).max(self.min);
    }

    fn forward(&mut self, by: i32) {
        self.position = self.position.wrapping_add(by).min(self.max);
    }

    /// `scrollbar_press`: an arrow just shows pressed ([`Sound::Press`]);
    /// the trough moves a large step at once and isn't left pressed
    /// ([`Sound::Select`]); the thumb only starts a drag and doesn't
    /// repaint. // 10c0:0e21
    fn press(&mut self, part: Part, out: &mut Outcome) {
        self.pressed = Some(part);
        match part {
            Part::BackArrow | Part::ForwardArrow => {
                out.repaint = true;
                out.sound = Some(Sound::Press);
            }
            Part::PageBack | Part::PageForward => {
                if part == Part::PageBack {
                    self.back(self.large_step);
                } else {
                    self.forward(self.large_step);
                }
                self.pressed = None;
                out.repaint = true;
                out.sound = Some(Sound::Select);
            }
            Part::Thumb => {}
        }
    }

    /// `scrollbar_release`: released over the arrow that was pressed, it
    /// steps ([`Sound::Release`]). Whatever the part, nothing is pressed
    /// any more. // 10c0:0f10
    fn release(&mut self, part: Part, out: &mut Outcome) {
        if self.pressed == Some(part) {
            match part {
                Part::BackArrow => self.back(self.small_step),
                Part::ForwardArrow => self.forward(self.small_step),
                _ => {}
            }
            if matches!(part, Part::BackArrow | Part::ForwardArrow) {
                out.repaint = true;
                out.sound = Some(Sound::Release);
            }
        }
        self.pressed = None;
    }

    /// `scrollbar_mouse_down`: presses the part under the mouse. Nothing
    /// happens when `min == max`. A repaint also calls `Click`, even when
    /// only an arrow went down or a page step hit the end; `handled` is
    /// the repaint. // 10c0:0fe3
    pub fn mouse_down(&mut self, x: i32, y: i32) -> Outcome {
        let mut out = Outcome::default();
        if self.frame.hit(x, y) && self.max != self.min {
            let along = self.along(x, y);
            self.press(self.part_at(along), &mut out);
            // Measured after a page step has moved the thumb.
            self.grab = along - (self.thickness() + self.thumb_offset());
        }
        out.clicked = out.repaint;
        out.handled = out.repaint;
        out
    }

    /// `scrollbar_mouse_move`: drags the thumb, if that's what was pressed
    /// and the mouse is inside the bar, rounding to the nearest position.
    /// A changed position repaints and calls `Click`; `handled` is the
    /// change. // 10c0:13b0
    ///
    /// The original divides by the thumb's travel and fails with a runtime
    /// error when it's 0; here the position stays.
    pub fn mouse_move(&mut self, x: i32, y: i32) -> Outcome {
        let mut out = Outcome::default();
        if self.pressed == Some(Part::Thumb) && self.frame.hit(x, y) && self.max != self.min {
            let travel = self.travel();
            let mut at = self.along(x, y) - self.grab - self.thickness();
            if at < 0 {
                at = 0;
            } else if travel < at {
                at = travel;
            }
            let range = self.max.wrapping_sub(self.min);
            let scaled = at.wrapping_mul(range).wrapping_add(travel / 2);
            if let Some(offset) = scaled.checked_div(travel) {
                let old = self.position;
                self.position = offset.wrapping_add(self.min);
                out.repaint = self.position != old;
            }
        }
        out.clicked = out.repaint;
        out.handled = out.repaint;
        out
    }

    /// `scrollbar_mouse_up`: inside the bar, steps if the mouse is over the
    /// arrow that was pressed, and lets go. Outside (or with `min ==
    /// max`), lets go of whatever was pressed, with [`Sound::Release`]. A
    /// repaint calls `Click`; `handled` is the repaint. // 10c0:15f7
    ///
    /// Released inside the bar but off the pressed arrow, the arrow is let
    /// go without a repaint, so it stays drawn pressed until something
    /// else redraws the bar.
    pub fn mouse_up(&mut self, x: i32, y: i32) -> Outcome {
        let mut out = Outcome::default();
        if self.frame.hit(x, y) && self.max != self.min {
            let part = self.part_at(self.along(x, y));
            self.release(part, &mut out);
        } else if self.frame.visible && self.pressed.is_some() {
            self.pressed = None;
            out.repaint = true;
            out.sound = Some(Sound::Release);
        }
        out.clicked = out.repaint;
        out.handled = out.repaint;
        out
    }
}

impl Widget for ScrollBar {
    frame_accessors!();

    /// `scrollbar_draw`: the trough, the arrows (pressed or not) and, unless
    /// `min == max`, the thumb, each from its bitmap's top left. The thumb
    /// can't be told apart from the arrows when pressed. // 10c0:0aec
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        let f = &self.frame;
        let b = &self.bitmaps;
        let thick = self.thickness();
        // From the start along the bar to a point `along` pixels down it.
        let at = |along: i32| {
            if self.horizontal {
                (f.left + along, f.top)
            } else {
                (f.left, f.top + along)
            }
        };
        let trough = self.length() - 2 * thick;
        if self.horizontal {
            skin.blit(dst, b.track, at(thick), trough, thick);
        } else {
            skin.blit(dst, b.track, at(thick), thick, trough);
        }
        let back = if self.pressed == Some(Part::BackArrow) {
            b.back_pressed
        } else {
            b.back
        };
        skin.blit(dst, back, at(0), thick, thick);
        let forward = if self.pressed == Some(Part::ForwardArrow) {
            b.forward_pressed
        } else {
            b.forward
        };
        skin.blit(dst, forward, at(self.length() - thick), thick, thick);
        if self.max != self.min {
            skin.blit(dst, b.thumb, at(thick + self.thumb_offset()), thick, thick);
        }
    }
}

/// The most the list box holds: its buffer is 100 `string[255]`s.
pub const LIST_CAPACITY: usize = 100;

/// A list of centred lines over a background bitmap, one of which can be
/// selected. Rows count from 1, and 0 means none. Not used on the start
/// screen (`1038:3c0e`). `listbox_init` // 10c0:0316
///
/// The owner fills `items` (at most [`LIST_CAPACITY`]) and scrolls by
/// setting `top`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListBox {
    pub frame: Frame,
    /// +0xb
    pub background: BitmapId,
    /// Colour rows, +0xf and +0x11 for the selected row.
    pub colour: i32,
    pub selected_colour: i32,
    /// +0x13
    pub row_height: i32,
    /// How many rows fit, `height div row_height`, +0x15.
    pub rows: i32,
    /// +0x17, with the count at +0x1d.
    pub items: Vec<Vec<u8>>,
    /// +0x1b
    pub selected: i32,
    /// The first row shown, +0x1f.
    pub top: i32,
}

impl ListBox {
    #[must_use]
    pub fn new(frame: Frame, background: BitmapId, colour: i32, selected_colour: i32, row_height: i32) -> Self {
        ListBox {
            frame,
            background,
            colour,
            selected_colour,
            row_height,
            rows: frame.height.checked_div(row_height).unwrap_or(0),
            items: Vec::new(),
            selected: 0,
            top: 1,
        }
    }

    fn count(&self) -> i32 {
        i32::try_from(self.items.len()).unwrap_or(i32::MAX)
    }

    /// `listbox_can_scroll_up` // 10c0:03cc
    #[must_use]
    pub fn can_scroll_up(&self) -> bool {
        self.top > 1
    }

    /// `listbox_can_scroll_down`: true until the last row is in view.
    /// // 10c0:03e7
    #[must_use]
    pub fn can_scroll_down(&self) -> bool {
        self.count() - self.rows >= self.top
    }

    /// `listbox_has_selection` // 10c0:0409
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selected >= 1 && self.selected <= self.count()
    }

    /// `listbox_mouse_down`, with the click's `TShiftState`. A plain left
    /// click selects the row under the mouse (none below the last), with a
    /// repaint and [`Sound::Select`]; `handled` says the selection changed.
    /// A double click on the selected row plays the sound and calls
    /// `Click`, and returns `handled` false. Any other shift state, such as
    /// a click with Shift held, does nothing. // 10c0:0431
    pub fn mouse_down(&mut self, x: i32, y: i32, shift: ShiftState) -> Outcome {
        let mut out = Outcome::default();
        if !self.frame.hit(x, y) {
            return out;
        }
        let Some(offset) = (y - self.frame.top).checked_div(self.row_height) else {
            return out;
        };
        let row = offset + self.top;
        if shift == ShiftState::LEFT | ShiftState::DOUBLE {
            if row == self.selected {
                out.sound = Some(Sound::Select);
                out.clicked = true;
            }
        } else if shift == ShiftState::LEFT {
            let row = if row > self.count() { 0 } else { row };
            if row != self.selected {
                self.selected = row;
                out.handled = true;
                out.repaint = true;
                out.sound = Some(Sound::Select);
            }
        }
        out
    }
}

impl Widget for ListBox {
    frame_accessors!();

    /// `listbox_draw` and `ui_listbox_draw`: the background, then the rows
    /// from `top` that fit, centred. // 10c0:04ed 1010:1838
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        let f = &self.frame;
        skin.blit(dst, self.background, (f.left, f.top), f.width, f.height);
        // `min_longint` // 1030:0f53
        let last = self.count().min(self.top + self.rows - 1);
        let mut y = f.top;
        for row in self.top..=last {
            let index = usize::try_from(row - 1).ok();
            if let Some(text) = index.and_then(|i| self.items.get(i)) {
                let colour = if row == self.selected {
                    self.selected_colour
                } else {
                    self.colour
                };
                skin.font.draw(dst, text, (f.left, y), f.width, Align::Center, colour);
            }
            y += self.row_height;
        }
    }
}

/// A one-line text field, typed into while focused. `edit_init`
/// // 10c0:1930
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub frame: Frame,
    /// Colour rows, +0xb and +0xd while focused.
    pub colour: i32,
    pub focus_colour: i32,
    /// +0xf
    pub focused: bool,
    /// `string[255]` at +0x11.
    pub text: Vec<u8>,
    /// Characters before the cursor, +0x111. Starts at the end.
    pub cursor: u8,
    /// +0x112
    pub max_len: u8,
}

/// The cursor drawn into the text. // 1010:172d
const CURSOR: u8 = b'|';

impl Edit {
    #[must_use]
    pub fn new(frame: Frame, text: &[u8], max_len: u8, colour: i32, focus_colour: i32) -> Self {
        let text = pstring(text);
        Edit {
            frame,
            colour,
            focus_colour,
            focused: false,
            cursor: u8::try_from(text.len()).unwrap_or(u8::MAX),
            text,
            max_len,
        }
    }

    /// `edit_mouse_down`: a click on the field focuses it. While it's
    /// focused, any click, on the field or not, unfocuses it, without
    /// `Click`. Either way it repaints; `handled` says so. // 10c0:1c1b
    pub fn mouse_down(&mut self, x: i32, y: i32) -> Outcome {
        let mut out = Outcome::default();
        if self.frame.hit(x, y) && !self.focused {
            self.focused = true;
        } else if self.focused && self.frame.visible {
            self.focused = false;
        } else {
            return out;
        }
        out.handled = true;
        out.repaint = true;
        out
    }

    /// `edit_insert_char`: inserts at the cursor if there's room.
    /// // 10c0:1cc1
    fn insert(&mut self, c: u8) {
        if self.text.len() < usize::from(self.max_len) {
            let at = usize::from(self.cursor).min(self.text.len());
            self.cursor = self.cursor.wrapping_add(1);
            self.text.insert(at, c);
            self.text.truncate(255);
        }
    }

    /// `edit_key_down`, with the virtual key code and shift state, while
    /// focused. Letters are lower case unless Shift is held. The German
    /// keyboard's OEM keys give ä, ö, ü and ß (with Shift Ä, Ö, Ü and ?),
    /// in the game's Windows-1252 font. Digits and space type themselves
    /// (Shift or not), and so do the keypad digits. Backspace, Delete,
    /// Home, End and the left and right arrows edit and move. Enter calls
    /// `Click` and unfocuses. Each of these repaints, even when nothing
    /// changed, and returns `handled`; other keys, and any key while not
    /// focused, do nothing. // 10c0:19d7
    pub fn key_down(&mut self, key: u16, shift: ShiftState) -> Outcome {
        let mut out = Outcome::default();
        if !self.focused {
            return out;
        }
        let shifted = shift.contains(ShiftState::SHIFT);
        let pick = |normal: u8, with_shift: u8| if shifted { with_shift } else { normal };
        let len = u8::try_from(self.text.len()).unwrap_or(u8::MAX);
        match key {
            // 'A'..'Z'
            0x41..=0x5a => {
                let c = u8::try_from(key).unwrap_or(0);
                self.insert(pick(c + 0x20, c));
            }
            // VK_OEM_7, VK_OEM_3, VK_OEM_1, VK_OEM_4
            0xde => self.insert(pick(0xe4, 0xc4)),
            0xc0 => self.insert(pick(0xf6, 0xd6)),
            0xba => self.insert(pick(0xfc, 0xdc)),
            0xdb => self.insert(pick(0xdf, b'?')),
            0x30..=0x39 | 0x20 => self.insert(u8::try_from(key).unwrap_or(0)),
            // VK_NUMPAD0..9
            0x60..=0x69 => self.insert(u8::try_from(key - 0x30).unwrap_or(0)),
            // VK_BACK
            8 => {
                if self.cursor != 0 {
                    let at = usize::from(self.cursor - 1);
                    if at < self.text.len() {
                        self.text.remove(at);
                    }
                    self.cursor -= 1;
                }
            }
            // VK_HOME, VK_END
            0x24 => self.cursor = 0,
            0x23 => self.cursor = len,
            // VK_DELETE
            0x2e => {
                if self.cursor < len {
                    self.text.remove(usize::from(self.cursor));
                }
            }
            // VK_LEFT, VK_RIGHT
            0x25 => self.cursor = self.cursor.saturating_sub(1),
            0x27 => {
                if self.cursor < len {
                    self.cursor += 1;
                }
            }
            // VK_RETURN
            0x0d => {
                out.clicked = true;
                self.focused = false;
            }
            _ => return out,
        }
        out.handled = true;
        out.repaint = true;
        out
    }
}

impl Widget for Edit {
    frame_accessors!();

    /// `ui_edit_draw`: the text centred in the width, for a 12 pixel line,
    /// with a `|` at the cursor while focused. The base `edit_draw` draws
    /// nothing, so nothing clears behind it. // 1010:172f 10c0:19c0
    fn draw(&self, dst: &mut Surface, skin: &Skin) {
        let f = &self.frame;
        let at = (f.left, (f.height - 12) / 2 + f.top);
        if self.focused {
            let mut text = self.text.clone();
            text.insert(usize::from(self.cursor).min(text.len()), CURSOR);
            text.truncate(255);
            skin.font
                .draw(dst, &text, at, f.width, Align::Center, self.focus_colour);
        } else {
            skin.font.draw(dst, &self.text, at, f.width, Align::Center, self.colour);
        }
    }

    /// Typing and focus changes erase the field from the background
    /// first (VMT+0x18), since its drawing doesn't cover the old text.
    fn repaint(&self, dst: &mut Surface, background: &Surface, skin: &Skin) {
        self.frame.erase(dst, background);
        self.draw(dst, skin);
    }
}

/// A Pascal `string[255]`: anything longer is cut.
fn pstring(text: &[u8]) -> Vec<u8> {
    text[..text.len().min(255)].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::bmp::Bitmap;

    /// A font whose glyphs change nothing: a white mask and black colours.
    fn font() -> Font {
        let (width, height) = (12, 14 * 12);
        let mut pixels = vec![0; width * height];
        pixels[..width * 14].fill(0xff);
        let bitmap = Bitmap {
            width,
            height,
            palette: [vcl::Rgb::default(); 256],
            pixels,
        };
        Font::new(&bitmap, &[[0; 8]; 32], &[[1; 8]; 32])
    }

    fn shown(mut frame: Frame) -> Frame {
        frame.visible = true;
        frame
    }

    #[test]
    fn hit_includes_far_edges() {
        let mut f = Frame::new(10, 20, 5, 4);
        assert!(!f.hit(10, 20), "hidden");
        f.visible = true;
        assert!(f.hit(10, 20) && f.hit(15, 24));
        assert!(!f.hit(9, 20) && !f.hit(16, 20) && !f.hit(10, 25));
    }

    #[test]
    fn show_and_hide() {
        let font = font();
        let bitmaps = [Surface::new(4, 4, 7)];
        let skin = Skin {
            font: &font,
            bitmaps: &bitmaps,
        };
        let mut dst = Surface::new(8, 8, 0);
        let mut b = Button::new(Frame::new(2, 2, 4, 4), BitmapId(0), BitmapId(0), b"", 1);
        b.show(&mut dst, &skin);
        assert!(b.frame.visible);
        assert_eq!(dst.pixel(2, 2), 7);
        let background = Surface::new(8, 8, 3);
        b.hide(&mut dst, &background);
        assert!(!b.frame.visible);
        assert_eq!((dst.pixel(2, 2), dst.pixel(5, 5), dst.pixel(1, 1)), (3, 3, 0));
    }

    #[test]
    fn button_clicks_when_released_inside() {
        let mut b = Button::new(shown(Frame::new(0, 0, 10, 10)), BitmapId(0), BitmapId(1), b"", 1);
        assert_eq!(b.mouse_up(5, 5), Outcome::default());
        let down = b.mouse_down(5, 5);
        assert!(b.pressed && down.handled && down.repaint && !down.clicked);
        assert_eq!(down.sound, Some(Sound::Press));
        let up = b.mouse_up(5, 5);
        assert!(!b.pressed && up.clicked && up.handled);
        assert_eq!(up.sound, Some(Sound::Release));
        // Dragged off before letting go: released, but no click.
        let _ = b.mouse_down(5, 5);
        let up = b.mouse_up(50, 5);
        assert!(!b.pressed && up.handled && up.repaint && !up.clicked);
    }

    #[test]
    fn radio_only_switches_on() {
        let label = Label::new(shown(Frame::new(0, 0, 50, 10)), b"Rot", Align::Left, 9);
        let mut r = RadioButton::new(label, BitmapId(0), BitmapId(1));
        let on = r.mouse_down(3, 3);
        assert!(r.on && on.clicked && on.repaint);
        assert_eq!(on.sound, Some(Sound::Select));
        assert_eq!(r.mouse_down(3, 3), Outcome::default());
        assert!(r.on);
        let off = r.deselect();
        assert!(!r.on && off.repaint && !off.clicked && off.sound.is_none());
        assert_eq!(r.deselect(), Outcome::default());
    }

    #[test]
    fn checkbox_toggles() {
        let label = Label::new(shown(Frame::new(0, 0, 50, 10)), b"", Align::Left, 9);
        let mut c = CheckBox::new(label, BitmapId(0), BitmapId(1), 10);
        assert!(c.mouse_down(1, 1).clicked && c.checked());
        assert!(c.mouse_down(1, 1).clicked && !c.checked());
        assert_eq!(c.mouse_down(60, 1), Outcome::default());
    }

    /// A horizontal bar 100 wide and 10 high: arrows at 0–10 and 90–100,
    /// a thumb travel of 70.
    fn bar() -> ScrollBar {
        let ids = ScrollBitmaps {
            back: BitmapId(0),
            forward: BitmapId(1),
            back_pressed: BitmapId(2),
            forward_pressed: BitmapId(3),
            thumb: BitmapId(4),
            track: BitmapId(5),
        };
        let mut s = ScrollBar::new(shown(Frame::new(0, 0, 100, 10)), ids);
        s.position = 0;
        s
    }

    #[test]
    fn scrollbar_parts() {
        let mut s = bar();
        assert!(s.horizontal);
        s.position = 50;
        // Thumb at 10 + 70 * 50 / 100 = 45, so 45..=55 is the thumb.
        assert_eq!(s.thumb_offset(), 35);
        let parts: Vec<Part> = [0, 10, 11, 45, 46, 55, 56, 90, 91, 100]
            .iter()
            .map(|&a| s.part_at(a))
            .collect();
        let expected = [
            Part::BackArrow,
            Part::BackArrow,
            Part::PageBack,
            Part::PageBack,
            Part::Thumb,
            Part::Thumb,
            Part::PageForward,
            Part::PageForward,
            Part::ForwardArrow,
            Part::ForwardArrow,
        ];
        assert_eq!(parts, expected);
        let vertical = ScrollBar::new(Frame::new(0, 0, 10, 100), s.bitmaps);
        assert!(!vertical.horizontal);
    }

    #[test]
    fn scrollbar_arrows_step_on_release() {
        let mut s = bar();
        let down = s.mouse_down(95, 5);
        assert_eq!(s.pressed, Some(Part::ForwardArrow));
        assert_eq!(s.position, 0);
        assert!(down.repaint && down.clicked);
        assert_eq!(down.sound, Some(Sound::Press));
        let up = s.mouse_up(96, 5);
        assert_eq!((s.position, s.pressed), (1, None));
        assert!(up.repaint && up.clicked);
        assert_eq!(up.sound, Some(Sound::Release));
        // At the end, the step clamps but still clicks.
        s.position = 0;
        let _ = s.mouse_down(5, 5);
        assert!(s.mouse_up(5, 5).clicked);
        assert_eq!(s.position, 0);
        // Released over the trough: let go, but no repaint.
        let _ = s.mouse_down(5, 5);
        assert_eq!(s.mouse_up(50, 5), Outcome::default());
        assert_eq!(s.pressed, None);
        // Released outside: let go with a repaint.
        let _ = s.mouse_down(5, 5);
        let up = s.mouse_up(500, 5);
        assert!(up.repaint && s.pressed.is_none());
        assert_eq!(s.position, 0);
    }

    #[test]
    fn scrollbar_pages_on_press() {
        let mut s = bar();
        let out = s.mouse_down(50, 5);
        assert_eq!((s.position, s.pressed), (10, None));
        assert_eq!(out.sound, Some(Sound::Select));
        // Thumb at 10 + 70 * 95 / 100 = 76..=86.
        s.position = 95;
        let _ = s.mouse_down(88, 5);
        assert_eq!(s.position, 100);
        let _ = s.mouse_down(50, 5);
        assert_eq!(s.position, 90);
        // With nowhere to go, the bar ignores the mouse.
        s.max = 0;
        s.min = 0;
        assert_eq!(s.mouse_down(50, 5), Outcome::default());
    }

    #[test]
    fn scrollbar_drags_thumb() {
        let mut s = bar();
        // Grab the thumb (10..=20) 4 pixels in.
        let down = s.mouse_down(14, 5);
        assert_eq!((s.pressed, s.grab), (Some(Part::Thumb), 4));
        assert_eq!(down, Outcome::default());
        // Thumb's left edge to 45: (35 * 100 + 35) / 70 = 50.
        let out = s.mouse_move(49, 5);
        assert!(out.repaint && out.clicked);
        assert_eq!(s.position, 50);
        assert_eq!(s.mouse_move(49, 5), Outcome::default());
        // Past the end, clamped.
        let _ = s.mouse_move(99, 5);
        assert_eq!(s.position, 100);
        // Outside the bar, nothing.
        let _ = s.mouse_move(20, 50);
        assert_eq!(s.position, 100);
        let _ = s.mouse_up(20, 5);
        let _ = s.mouse_move(14, 5);
        assert_eq!(s.position, 100);
    }

    #[test]
    fn scrollbar_draws() {
        let font = font();
        let bitmaps: Vec<Surface> = (1..=6).map(|c| Surface::new(100, 100, c)).collect();
        let skin = Skin {
            font: &font,
            bitmaps: &bitmaps,
        };
        let mut s = bar();
        s.position = 50;
        s.pressed = Some(Part::BackArrow);
        let mut dst = Surface::new(100, 10, 0);
        s.draw(&mut dst, &skin);
        let row: Vec<u8> = (0..100).map(|x| dst.pixel(x, 5)).collect();
        assert_eq!(row[..10], [3; 10], "back arrow pressed");
        assert_eq!(row[10..45], [6; 35], "track");
        assert_eq!(row[45..55], [5; 10], "thumb");
        assert_eq!(row[55..90], [6; 35]);
        assert_eq!(row[90..], [2; 10], "forward arrow");
    }

    #[test]
    fn listbox_selects() {
        let mut l = ListBox::new(shown(Frame::new(0, 100, 50, 40)), BitmapId(0), 9, 10, 10);
        assert_eq!(l.rows, 4);
        l.items = (0..6).map(|i| vec![b'a' + i]).collect();
        l.top = 2;
        let out = l.mouse_down(5, 115, ShiftState::LEFT);
        assert_eq!(l.selected, 3);
        assert!(out.handled && out.repaint && !out.clicked);
        assert!(!l.mouse_down(5, 115, ShiftState::LEFT).handled);
        let double = l.mouse_down(5, 115, ShiftState::LEFT | ShiftState::DOUBLE);
        assert!(double.clicked && !double.handled);
        assert!(!l.mouse_down(5, 125, ShiftState::LEFT | ShiftState::DOUBLE).clicked);
        let _ = l.mouse_down(5, 115, ShiftState::LEFT | ShiftState::SHIFT);
        assert_eq!(l.selected, 3);
        // Below the last row: none.
        l.top = 6;
        let _ = l.mouse_down(5, 115, ShiftState::LEFT);
        assert!(!l.has_selection());
        assert!(l.can_scroll_up());
        l.top = 2;
        assert!(l.can_scroll_down());
        l.top = 3;
        assert!(!l.can_scroll_down(), "rows 3 to 6 in view");
    }

    #[test]
    fn edit_types() {
        let mut e = Edit::new(shown(Frame::new(0, 0, 100, 16)), b"ab", 4, 9, 11);
        assert_eq!(
            e.key_down(0x41, ShiftState::default()),
            Outcome::default(),
            "not focused"
        );
        assert!(e.mouse_down(5, 5).repaint && e.focused);
        let _ = e.key_down(0x43, ShiftState::default());
        let _ = e.key_down(0xde, ShiftState::SHIFT);
        let _ = e.key_down(0x44, ShiftState::default());
        assert_eq!(e.text, b"abc\xc4", "full at 4");
        assert_eq!(e.cursor, 4);
        let _ = e.key_down(0x25, ShiftState::default());
        let _ = e.key_down(8, ShiftState::default());
        assert_eq!((e.text.as_slice(), e.cursor), (&b"ab\xc4"[..], 2));
        let _ = e.key_down(0x24, ShiftState::default());
        let _ = e.key_down(0x2e, ShiftState::default());
        assert_eq!((e.text.as_slice(), e.cursor), (&b"b\xc4"[..], 0));
        let _ = e.key_down(0x63, ShiftState::default());
        assert_eq!(e.text, b"3b\xc4");
        assert!(!e.key_down(0x70, ShiftState::default()).handled, "F1");
        let enter = e.key_down(0x0d, ShiftState::default());
        assert!(enter.clicked && !e.focused);
        // A click while focused unfocuses, even on the field.
        let _ = e.mouse_down(5, 5);
        assert!(e.mouse_down(5, 5).handled && !e.focused);
        let _ = e.mouse_down(5, 5);
        assert!(e.mouse_down(500, 5).handled && !e.focused);
        assert!(!e.mouse_down(500, 5).handled);
    }
}
