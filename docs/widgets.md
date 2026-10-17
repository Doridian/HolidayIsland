# The widget toolkit

The forms don't use VCL controls for their buttons, check boxes, scroll bars
and text fields. Each form draws into one `TImage` (the start screen's
`Bild`), and its `OnMouseDown`/`OnMouseMove`/`OnMouseUp`/`OnKeyDown` handlers
pass the events on to widgets of the game's own toolkit. The toolkit is
segment `10c0` (about 7.5 KB); the port is `game::widgets`.

## Objects, not classes

The widgets are Turbo Pascal `object`s, not Delphi classes:

- Each widget is a record embedded in the form instance (`TStartform` has
  about 45, set up in `1058:27e6`).
- The VMT's offset in `DGROUP` is stored at +9. Constructors take it as a
  hidden parameter (`0` when a descendant's constructor calls its parent's).
- The VMT is `size, -size, 0, 0` followed by far pointers to the virtual
  methods. Every widget shares these slots:

  | Slot   | Method                                    | Base            |
  | ------ | ----------------------------------------- | --------------- |
  | +0x08  | `Click`, the owner's notification         | empty, `00b3`   |
  | +0x0c  | `Show(image)`: draw unless visible        | `00ba`          |
  | +0x10  | `Hide(background, image)`: erase if visible | `00e4`        |
  | +0x14  | `Draw(image)`                             | per type        |
  | +0x18  | `Erase(background, image)`: copy the rect back from a bitmap | `011d` |
  | +0x1c… | per type: mouse handlers or sound hooks   |                 |

- The form units declare a descendant per instance just to override `Click`
  (the start screen's VMTs `1110:1ada`… point into `1058`). The drawing (text
  over the bitmaps) and the sounds (`play_sound`, `10a0:30b6`) are overrides
  too, shared by every instance, in segment `1010` (`ui_button_draw` and
  friends).

The port has one struct per type and a `Widget` trait for `Show`, `Hide` and
`Draw`. The event methods return an `Outcome` (repaint? `Click`? which sound?)
instead of calling virtual methods, and bitmaps are indices into a `Skin`
rather than pointers to the form's `TBitmap` variables.

## Types

All share the base fields: +0 top, +2 left, +4 width, +6 height, +8 visible.
Widgets start hidden, and hidden widgets ignore the mouse. Hit-testing
(`widget_hit`) includes the right and bottom edges.

| Type        | Constructor | Draw (`1010`) | Behaviour |
| ----------- | ----------- | ------------- | --------- |
| Label       | `02a2` | `152d` | Text at (left, top + 2), wrapped to the width. |
| Gauge       | `0181` | `01dc` (`10c0`) | A "full" bitmap over an "empty" one, `width * percent div 100` wide. |
| Button      | `083b` | `1432` | Normal or pressed bitmap, centred caption that moves 1 px when pressed. Press on mouse down (sound 8), `Click` on mouse up if still inside (sound 10). |
| Radio button | `0559` | `1572`, `16b5` | Square bitmap, off or on, text to the right. A click only switches it on (sound 9, `Click`); the form switches the others off with `radio_deselect`. The start screen's colour choice is eight of these. |
| Check box   | `072c` | `15ea` | A radio button that toggles, with the text in a second colour when checked. |
| Scroll bar  | `09eb` | `0aec` (`10c0`) | Six bitmaps: two arrows, both pressed, thumb, trough. Horizontal when `height <= width`. Longint min/max/position, steps 1 and 10. |
| List box    | `0316` | `1838` | Up to 100 `string[255]` rows over a background, centred. Left click selects (sound 9), double click on the selected row calls `Click`. Not on the start screen. |
| Edit        | `1930` | `172f` | A `string[255]` with a byte cursor, drawn centred with a `|` at the cursor while focused. |

## Quirks kept in the port

- Scroll bars step on the arrows when the mouse is *released* over the arrow
  that was pressed, page on the trough at once on press, and drag the thumb
  only while the mouse stays inside the bar. Nothing auto-repeats. With
  `enhance.scroll_bars` the arrows step when pressed.
- A scroll arrow released over another part of the bar is let go without a
  repaint, so it stays drawn pressed (with `enhance.scroll_bars` it's
  drawn up).
- Scroll bar presses call `Click` whenever they repaint, even when only an
  arrow went down or a page step hit the end (with `enhance.scroll_bars`
  only for a change of the position).
- A button's mouse up reports "handled" whenever it was pressed, clicked or
  not.
- An edit field unfocuses on any click while focused, including one on the
  field itself (with `enhance.edit_field` a click on the field keeps it
  focused). Its drawing has no background, so every change erases the
  rect from the form's background bitmap first. Letters are lower case
  unless Shift is held; the German OEM keys give ä, ö, ü, ß (Ä, Ö, Ü, ?), and
  Enter calls `Click` and unfocuses.
- A list box only reacts to exactly `[ssLeft]` or `[ssLeft, ssDouble]`; a
  click with Shift held does nothing (with `enhance.modifier_clicks` Shift,
  Alt and Ctrl don't matter).

## Finding the VMTs

`DGROUP` is stored in the NE file as *iterated* data (runs of
`count, length, bytes`), and Ghidra's NE loader maps those bytes as they
are, so Ghidra's view of initialised data in `1110` is wrong: the VMTs at
`1110:1492` and so on show unrelated bytes there. To read them, expand the
segment's records and follow its relocation chains (the segment words of the
method pointers are relocations into code segments).
