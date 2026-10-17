# Screen layout and window sizes

How the original lays its forms out for the Windows desktop, and how the
port turns a resizable window into that desktop. In short: the original
picks a screen size once, at startup, from the size of its maximized
background form. The main screen is **always 640×479**, centred on that
screen. A bigger desktop doesn't show more island; it shows more of the
background picture, and leaves room around the main screen for the
floating forms, which are placed against the screen's edges or its centre.
Nothing is hidden or resized at any size; only the background and title
pictures depend on it. The port's `big_map` enhancement changes that:
the main screen fills a bigger screen and shows more island
([below](#big_map-enhancement)).

## The screen size

`THintergrund` is borderless and `wsMaximized` (its DFM), so it covers the
desktop. State 110 of its timer (`hintergrund_timer`, 1000:2466) reads the
form's `Width` and `Height` and snaps them to one of five layouts, with
±20 pixels of slack; any other size is kept as it is. The result is
`screen_width`/`screen_height` (`1110:54fc`/`54fe`), which every later
placement reads. The form's own size (the real desktop) is only used to
centre `FilmPanel` (432×320) for the intro's and the start's films.

| Form width | Form height | Screen size | Title (states 430/440) | Background (states 450, 1130) |
|---|---|---|---|---|
| 780–820 | 580–620 | 800×600 | `tit80x60.BMP` | `bck80x60.BMP` |
| 620–660 | 460–500 | 640×480 | `tit64x48.BMP` | `bck64x48.BMP` |
| 1004–1044 | 748–788 | 1024×768 | `tit10x76.BMP` | `bck10x76.BMP` |
| 1132–1172 | 850–890 | 1152×870 | `tit11x87.BMP` | `bck11x87.BMP` |
| 1260–1300 | 1004–1044 | 1280×1024 | `tit12x10.BMP` | `bck12x10.BMP` |
| any other | | as it is | none | none |

The names are at `1000:233b`..`1000:23b0`. `backgroundimage` is set to the
screen size at 0, 0 and shows the picture unscaled. Ported as
`hintergrund::screen_size`, `hintergrund::layout` and `hintergrund::LAYOUTS`.

Two quirks follow ([original-bugs.md](original-bugs.md#ui-quirks)):

- A desktop up to 20 pixels smaller than a layout is laid out as that
  layout, so whatever sits at the right or bottom edge is cut off by up
  to 20 pixels (790×590 lays out for 800×600: the compass's right edge
  is at 800).
- Any size but the five has no pictures at all: the title stays black, and
  so does the desktop around the main screen (1366×768, 1920×1080, ...).
  The palette then is whatever Windows had; the port uses the 800×600
  pictures' palette.

With `enhance.desktop_background` the port lays out for the desktop's
size as it is, and draws the pictures of the layout it would have
snapped to, else of the biggest that fits, centred on it (else the
smallest; around them the background stays black).

Nothing in the original reacts to a change of the desktop's size later:
no form has an `OnResize` handler (none in the DFMs), and state 110 runs
once.

## The main screen

`THauptschirm` (`hauptschirm_layout`, 1020:1ce9) gets the width of entry 0
of the layout table at `1110:1572` (11 entries of hidden, left, top,
width, height; constant initialised data, nothing writes it) and is
centred: `Left = screen_width div 2 - 640 div 2`, `Top = screen_height div
2 - 479 div 2`. `hauptschirm_place` (1020:1c79) puts the images at the
table's bounds; `hauptschirm_show` (1020:283b) sets its height to 479.

| Entry | Image | Bounds |
|---|---|---|
| 0 | the form | 640 × 479 |
| 1, 3 | (hidden 16×16 slots) | 0,0 and 624,0 |
| 2 | `Ueberschriftbild` (title bar) | 16, 0, 608 × 16 |
| 4 | `InfoImage` | 0, 16, 640 × 18 |
| 5 | `ButtonImage` (23 buttons) | 0, 34, 640 × 25 |
| 6 | `Hauptbild` (the island) | 0, 59, 624 × 404 |
| 7 | `NSScrollbild` | 624, 59, 16 × 404 (1020:0a55) |
| 8 | `Statusimage` | 0, 463, 219 × 16 |
| 9 | `WOScrollbild` | 219, 463, 405 × 16 (1020:11dd) |
| 10 | `eckimage` (compass knob) | 624, 463, 16 × 16 |

The map view, the scroll bars' ranges (`NS_MAX` 476, `WO_MAX` 481), the
drawing's 20 columns and 27 half rows (`darstellung::COLUMNS`,
`HALF_ROWS`, `VIEW_WIDTH`/`VIEW_HEIGHT` 624×405 at `1110:12bc`) and the
button bar are the same at every screen size. Where the code has the
view's size, see [`big_map`](#big_map-enhancement).

| Screen | Main screen's top left |
|---|---|
| 640×480 | 0, 1 |
| 800×600 | 80, 61 |
| 1024×768 | 192, 145 |
| 1152×870 | 256, 196 |
| 1280×1024 | 320, 273 |
| 1366×700 (no layout) | 363, 111 |

The main screen is a window like the others: its title bar drags it.

- `UeberschriftbildMouseDown` (1020:1b8e): `+0x1da` is set to whether the
  press has `ssDouble`; the cursor's offset from the form's top left goes
  to `1110:6046`/`6048` (`drag_grab_x`/`_y`), and the form's height to 16,
  its title bar.
- `UeberschriftbildMouseMove` (1020:1bd9): with `ssLeft` the form follows
  the cursor, kept at `0..screen_width - Width` and `0..screen_height -
  Height` (16 then).
- `UeberschriftbildMouseUp` (1020:1c58): the height goes back to 479
  unless `+0x1da` is set. So a double click leaves the main screen rolled
  up to its title bar until the next click on it. `+0x1da` also keeps the
  button bar's tooltips away (1020:2115,
  [endforms.md](endforms.md#open-questions)).

The move handler doesn't check that the press was on the title bar, so a
left drag that started off every form (on the background) moves the main
screen when it passes over the title bar, by the last grab
([original-bugs.md](original-bugs.md#ui-quirks)). The other forms' title
bars work the same way, each with its own clamp (below). With
`enhance.form_drags` the main screen only follows a press on its title
bar, and the compass's and the sabotage window's drags end when the
mouse comes up.

Ported in `game::hauptschirm`: `Hauptschirm::layout`, `bounds`,
`title_mouse_down/move/up`, `left`, `top`, `height`, `rolled_up`.

The two scroll bars take the mouse as the original's handlers have them
([map.md](map.md#the-scroll-bars)); with `big_map` their stretched
lengths go into the same arithmetic, so the thumb, its parts and the drag
follow the longer bars.

## The other forms

Where each form goes. "Centred" is `(screen_width - Width) div 2`,
`(screen_height - Height) div 2`. Positions are absolute screen
coordinates; only the info box is placed relative to the main screen.

| Form | Where | Placed by |
|---|---|---|
| `TStartform` | centred | 1058:27e6 (`startform_init`), 1058:1a30 (`startform_show`) |
| `TLadenForm` | centred across, at the bottom (`screen_height - Height`) | 1038:3b46 |
| `FilmPanel` (432×320) | centred on the background form, the real desktop | 1000:2466, state 110 |
| `TCreditform` | 640×480, centred; disables the options | 1030:3c42 |
| The list of enhancements (port's) | 640×480, centred, over the options | `game::enhanceform` |
| `TBaumenue` | created at `screen_width - Width - 1`, `(screen_height - 480) div 2 + 59` (with `enhance.form_positions` the main screen's top + 59, level with the island); opening keeps it left of `screen_width - Width - 1` | 1018:2fa2, 1018:2b01 |
| `TAuktionsform` | `(screen_width - 0x154) div 2`, `(screen_height - 0xeb) div 2` (with `enhance.form_positions` by its own height, 316) | 1028:0dde |
| `TSabowin` | the DFM's 300, 167 (with `enhance.form_positions` from the centred main screen's top left) | `Application.CreateForm` |
| `TDateimenue` | the DFM's 305, 139 (the same) | `Application.CreateForm` |
| `TInfobox2` | next to the mouse inside the main screen's client area, flipped left or up at its right or bottom | 1038:1ccf |
| `TKompassform` | top right: `screen_width - Width`, 0 | 1038:2c04 |
| `TEndeform` | centred | 1020:3b5f |
| `TPauseform` | `screen_width div 2 - 0xaf`, `screen_height div 2 - 0x69` | 1040:3a45 |
| `Thighscoreform` | centred | 1040:2bf0, 1040:2579 |
| `THelpform` (tooltip) | at the asked spot, kept left of `screen_width - Width` and above `screen_height - 16` | 1040:3dc7 |
| `TFernseher` | `(screen_width - 0x160) div 2`, `(screen_height - 0x14e) div 2` for a new game; a map file's own position otherwise | 1048:1ffd, 1048:25ea |
| `TOptionenform` | centred across, at the top | 1060:14f1 |
| `TSchlussform` | centred (the quit film's panel and the results) | 1068:09a5, 1068:1340 |
| `TGeldmenuform` | top right: `screen_width -` the table's width, 0 (one column too far right, [geldmenu.md](geldmenu.md); with `enhance.form_positions` whole on the screen) | 1070:2e6c |
| `Tumenuform` | centred | 1080:145f, 1080:0ea8 |
| `TUebersichtsmap2` | top left, 0, 0; its small/large switch (10b8:3e73) keeps the top left | 10b8:4538 |

Every form's title bar drags it, kept on the screen by the same
`max(0, ..)` / `min(screen - size, ..)` clamp as the main screen:
1070:08e0 (money menu), 1020:38d4 (quit question, by its 16-pixel title
bar; with `enhance.form_drags` by its full height), 1048:2955 (TV), 1060:29f0 (options), 1040:09ca (file menu),
10b8:3c9b (overview), 1028:1c4d (auction), 1080:1e54 (buildings'
overview), 1028:2ecc (sabotage), 1038:346f (compass), 1018:0461 (build
menu). `screen_width`'s other readers are these handlers.

So the layout differs by screen size only in where things land:

- At 640×480 everything overlaps the main screen; the main screen fills
  the screen but for its last row.
- At 800×600 and up the forms placed at the screen's edges (money menu,
  compass, build menu, overview, options) sit beside or above the
  main screen, and the centred ones (TV, auction, pause, end forms) over
  its middle.
- The sabotage window and the file menu stay at their DFM positions,
  which were chosen for 640×480: on bigger screens they're up and left of
  the centred main screen, partly off it
  ([original-bugs.md](original-bugs.md#ui-quirks)).

### Saved windows

A map file keeps the windows of the 17 forms in `1110:646c`
(`window_forms`, filled by `init_game_globals`, 1010:064a): the auction,
build menu, file menu, quit question, money menu, tooltip, high scores,
info box, compass, TV, two empty slots, sabotage window, main screen, end
form, buildings' overview and overview map. `save_window_positions`
(1010:0799) writes each one's `Top`, `Left`, `Width`, `Height`, `Visible`
and `Enabled` (not the file menu's; the empty slots as zeros), and
`load_window_positions` (1010:08d7) reads them back and keeps each on the
screen it's loaded on: `Top = min(screen_height - max(16, Height), Top)`
and `Left = min(screen_width - Width, Left)`. Nothing keeps a window off
the top or left edge. That clamp is `savegame::keep_on_screen`. The port
doesn't apply the saved windows yet; it keeps them as read
(`MapView::unported`) and writes them back.

## Stacking

Every form is a top-level window of its own, and Windows stacks them; the
game hardly does. What the binary and the DFMs show:

- **Two tiers.** These DFMs have `FormStyle = fsStayOnTop` (Windows'
  topmost windows): `TAuktionsform`, `TInfobox2`, `TDateimenue`,
  `TEndeform`, `TKompassform`, `TFernseher`, `TOptionenform`,
  `Thighscoreform`, `TPauseform`, `THelpform`, `TCreditform` and
  `TDummyForm`. The others are normal: `THintergrund`, `THauptschirm`,
  `TBaumenue`, `TSabowin`, `TGeldmenuform`, `Tumenuform`,
  `TUebersichtsmap2`, `TSchlussform`, `TStartform`, `TLadenForm`,
  `TWasNunForm`. A topmost window always lies over every normal one.
- **Siblings.** Delphi 1 makes the application's hidden window the owner
  of every form, the background (the main form) too, so none is kept
  over another by ownership: within a tier the last one raised is on top.
- **What raises a form.** Showing it: `TCustomForm.Show` (10f0:56e1) is
  `Visible := True` (10f0:2e80), whose `ShowWindow` activates the window,
  and then `BringToFront` (10e8:201c, `SetZOrder(True)`). A press on it,
  which activates it, unless it's disabled (`disable_forms`, 1010:2970,
  while a modal form is up; a disabled form takes no press at all).
  The focus (`SetFocus`, VMT+0x78), which activates it. And the game's own
  `BringToFront` calls:
  - each form when it opens: the money menu (1070:2b71), build menu
    (1018:2c11), overview map (10b8:414b), info box (1038:1e6b), tooltip
    (1040:3e9e), file menu (1040:0fa7);
  - the main screen when a form closes: the money menu (1070:2bd7),
    overview map (10b8:3fa2), build menu (1018:2c96), info box
    (1038:1f0a); `Show` of the main screen when the TV hides (1048:085a)
    and the auction closes (1028:056f); the focus to it after the pause
    (1040:3bac) and the options (1060:0e80);
  - the main screen on Space in the build menu (1018:3347), money menu
    (1070:39f9) and overview map (10b8:4def), at the start of a game
    (1010:2964), after loading one from the file menu (`game_show_state`,
    1010:2819) and after "Weiterspielen" (1068:0194).
- **The background.** A press where no form is activates `THintergrund`,
  which goes over every normal form. Its `FormActivate` (1000:2241), once
  the main screen has been shown (`1110:00a0`, set by 1020:283e), calls
  1010:2a5a: unless the info box is shown (`infobox2+0x1e6`) or the end
  form is, `Application.Restore` and `ProcessMessages`, then
  `BringToFront` of every form of `window_forms` (`1110:646c`, 0 to 16
  but the empty 10 and 11) in turn and of the main screen last. So
  normally the normal forms come back over the background, the main
  screen on top of them; with the info box or the end form shown they
  stay hidden behind it.

Nothing calls `SendToBack` or `SetWindowPos` directly (the
`SETWINDOWPOS` and `BRINGWINDOWTOTOP` calls are all the VCL's).

So the main screen, as a normal form, goes over the money menu, build
menu, overview map, sabotage window and buildings' overview whenever it's
clicked or a form closes, and over none of the topmost forms. These are
quirks ([original-bugs.md](original-bugs.md#ui-quirks)). The stacking is
worked out from the code and Windows' rules, not watched in a running
original.

### In the port

`holiday::stacking::Stack` keeps the order of the game's windows (the
background, the main screen, the build menu, the auction, the file menu
and the forms of `holiday::forms`). Every repaint and mouse event first
tells it which are shown (`Game::sync_stack`): a form that opened since
goes on top. The windows are painted in its order (`Game::paint`), and a
mouse event that no window holds goes to the top window under the
pointer (`Game::window_at`); a press raises that window
(`Game::press_window`). The tooltip takes no presses, which go to what's
under it. Before this the port painted the forms in a fixed order and
gave a press to the first of a fixed list.

With every enhancement off it does what's above: the two tiers, the
main screen raised when those forms close (the end form too, for
"Weiterspielen") and after a load, the background's raise and 1010:2a5a.
The start of a game needs nothing: the main screen is shown first, so
the forms shown after it lie over it. Space in the build menu, money menu
and overview map raises the main screen in both modes.

### `window_stacking` (enhancement)

On by default ([original-bugs.md](original-bugs.md#options)). The
floating forms then stack like the windows of a window manager:

- a press raises the window it's on, the main screen too, over every
  non-modal form, the `fsStayOnTop` ones among them;
- a form that opens goes on top;
- the pointer's events go to the top window under it;
- the tiers are: the background at the bottom; the main screen and the
  non-modal forms; the modal ones (pause, options, quit question, end
  form, high scores, file menu, the credits, the port's list of
  enhancements, and the info box while it has buttons); the tooltip;
- closing a form, loading a game and a press on the background raise
  nothing;
- the button of a form that's open but lies under another window (in the
  order before the press on the button) raises it, with the compass for
  the overview map's, instead of closing it, like a task bar. Without
  that a TV under the main screen (it lies inside the main screen at
  every size) would be found again only by closing and opening it.

The tests: `stacking::tests` (both modes' orders) and, with the game's
files, `tests::window_stacking` (a press on the main screen at 800×600
over the money menu and the TV, the TV's button; `stacking-original.ppm`
and `stacking-enhanced.ppm` with `HOLIDAY_PPM`).

## The world and the screen size

Nothing simulated reads `screen_width`, `screen_height` or the desktop.
What the world reads of the view is the main screen's rotation, origin
and parity (`World::viewpoint`, [people.md](people.md)): where people
appear, which squares get spectators, builders and taxiing planes (the
11 × 61 squares of `1110:5536` from the view's origin), and which
sounds play (`people::sees`). All of that is sized for the fixed 624×404
map view, so it is the same at every window size. The view's origin is
already a multiplayer issue
([original-bugs.md](original-bugs.md#design-issues-for-multiplayer));
the window's size isn't one.

`big_map`'s bigger map view ([below](#big_map-enhancement)) keeps it
that way: the world's viewpoint, grid and `sees` stay those of the
original's 20 × 27 view, which lies in the middle of the bigger one; the
people around it are only drawn.

## `big_map` (enhancement)

On by default ([original-bugs.md](original-bugs.md#options)). With it
off, everything is as above. With it on and a desktop (the window, not
the snapped screen size) bigger than 640×480 either way
(`hauptschirm::BIG_FROM`), the main screen fills the desktop at 0, 0
and its map view shows as much of the island as fits. At 640×480 it's
the original's main screen, which fills that screen anyway. Filling the
whole screen, rather than leaving a margin, is a choice: the forms float
over the main screen at every size in the original too (at 640×480 all
of them do), and a margin wide enough for the money menu or the
overview map would leave little island on 800×600.

### Where the original has the view's size

The routines that read the map view's size, and what they read:

| Where | What | With `big_map` |
|---|---|---|
| `1110:1572` (1020:1ce9, 1020:1c79, 1020:283b) | The layout table: the form 640 × 479 and its images' bounds | `hauptschirm::layout_for`: the right and bottom edges' images move there, the bars and `Hauptbild` stretch between; `layout_for(640, 479)` is the table |
| 1050:2700 (`draw_view`) | `PUSH 0x13`: columns 0 to 19 | `darstellung::draw_view_in` draws `ViewSize::columns` |
| 1050:0e68 (`draw_columns`) | The bottom from `view_height` (`1110:12be`, 405); `0x1a - rows` for the scrolling's bands | The view's height; the bands aren't used by the port |
| `1110:12bc`, `1110:12be` | `view_width`, `view_height`, 624 × 405, set by `init_game_defaults` | `ViewSize::width`, `height` |
| `DARSTELL.DAT` | 20 entries per column table (`AnsichtSpaltenxKorrektur`, ...) | Each table steps the same every two columns; past the 20th it goes on that way (`darstellung::column_entry`) |
| 1050:21b7 (`in_view`), 1050:245b, 1050:21f9 | `column_low <= 0x13` and `row_high <= h · 9 div 32 + 0x2a` (27 half rows and 15 for tall buildings); the rotations' `0x13`, `0x1a`, `0x19` | `people::sees_in` with the view's columns and half rows, for drawing the people only; the rotations' constants are where the tables start, which the bigger camera shares |
| 1088:30ba | The people's columns and half rows from the origin (`0x1a`, `0x13`, 10, `0xd`) | The same: they come from the tables' start |
| 1050:0144 | The scrolling's strips: `PUSH 0x14` columns, `0x12`, `view_width`/`view_height` | Not ported (the port draws the whole view) |
| 1018:39c2 (`square_at`) | `Hauptbild`'s `Width` and `Height` | Follows by itself: `darstellung::square_at_in` with the view's size |
| 1020:1984 (right click) | `Hauptbild`'s `Width div 2`, `Height div 2` | Follows by itself |
| 1020:2501 | The scroll bars' ranges, 0..476 and 0..481 | Less the bigger view's margins (below) |
| 1020:0787 (`jump_to`), 10b8:4079, 10b8:37f5 | The view's middle as 10 columns and 13 half rows from its origin; the overview's click offsets | The same: they're about the original's view, which stays in the middle |
| 10b8:1349, `1110:3d80` | The overview's frame, a picture 40 × 25 | A box the bigger view's size |
| 1030:2bdc, `1110:5536`, 1090:2373 | The 11 × 61 squares from the view's origin where people appear | The world's: unchanged |

### The view

`darstellung::ViewSize` is the drawing area and its columns and half
rows. For a `Hauptbild` of `w` × `h` (`ViewSize::for_image`) it covers
`w` × `h + 1` pixels (the original's one extra row) with at least 20
columns and 27 half rows, more by a multiple of four of each; the
original's 20 × 27 view (the core) then lies in its middle, offset by an
even number of columns and half rows (`ViewSize::core_offset`). On
1024×768 that's 32 columns by 47 half rows, the core 6 columns and 10
half rows in; on 1280×1024, 40 × 63, 10 and 18 in.

The scroll position, the camera (`Hauptschirm::camera`), what a map
file saves and what the world sees (`Hauptschirm::viewpoint`) stay the
core's. The island is drawn and picked with a second camera
(`Hauptschirm::drawn_camera`): the scroll position moved up and left by
the offset, the way a scroll turns a move on the screen into the map's
directions for the rotation (1020:07dc), without the bars' ends. The
offset is even, so the parity and the turned origin's rounding (1020:06f1)
come out as the original's would there, and the core's part of the
bigger view is the original's view pixel for pixel, square for square
under the mouse, in every rotation (the test below checks both). So a
right click, the TV's and the buildings' overview's jumps and the
overview map's clicks centre on the same squares as in the original.

- **Scroll bars**: the core's ranges shrink by the offset at each end
  (`ns` from `dr` to `476 - dr`, `wo` from `dc` to `481 - dc`, in the
  bars' screen-turned terms), so the bigger view reaches the map's edges
  where the original's does. The bars' troughs stretch (their ends kept,
  the middle repeated); the thumbs travel the longer bars.
- **The people**: drawn with `sees_in` for the bigger view, so the
  people around the core show. The world's `sees` (sounds, which squares
  get spectators, builders, ...) and the 11 × 61 squares where people
  appear stay the core's, so the world doesn't see the window: two games
  from one world, with and without `big_map`, tick the same.
- **The tools**: `Tools::view` is the view's size; the caller passes the
  drawn camera, so the pointer, the build cursor and the clicks pick the
  square under the mouse in the bigger view.
- **The overview map's frame**: a box around the squares at the bigger
  view's corners (`Hauptschirm::overview_frame`), outlined in the frame
  picture's colour, with the picture's mark for the screen's bottom edge
  beside it (`Uebersichtsmap::set_big_frame`). Without the bigger view
  it's the original's picture.
- **The bars**: the title bar's grass, the info bar's planks repeat;
  the date goes to the info bar's right end; the button bar keeps its 23
  buttons at the left; right of them, between two of the wooden pieces
  that part its groups, the flat brown panel the overview map and money
  menu have under their buttons; the status bar keeps its 219 pixels, the lower scroll bar
  takes the rest.
- **Dragging**: the title bar doesn't move a main screen that fills the
  screen; a double click still rolls it up to its title bar, and the
  next press unrolls it. With `no_main_title` (on by default) there's no
  title bar at all ([below](#no_main_title-enhancement)).
- **Stacking**: the main screen that fills the screen lies right over
  the background and under every form, in both stackings
  (`Stack::main_desktop`): raised, it would hide them all.
- **The forms at the top edge** (the money menu, the compass, the
  options, the overview map) are moved just below the main screen's
  button bar when they open over it (59 pixels down, 43 without the
  title bar: `Hauptbild`'s top, `Forms::below_bars`), so the title, info
  and button bars stay free.
- **Resizing** lays the main screen out again (`Hauptschirm::place`):
  it fills the new desktop, or becomes the original's at 640×480. The
  other forms move as they would with the original's main screen
  centred, and are kept on the screen. The scroll position stays, past
  the new bars' ends too until the next scroll, so a resize never moves
  what the world sees.

The tests (`crates/holiday/src/main.rs`): `big_map` (1024×768 and
1280×1024, on and off: the layout, the core pixel for pixel and square
for square in each rotation, the overview's frame, the scroll bars'
ends; with `HOLIDAY_PNG` set to a directory it writes
`bigmap-WxH-on.png` and `-off.png` there), `big_map_world_is_the_same`,
`big_map_resize`, and `stacking::tests::big_map_desktop`.

### `no_main_title` (enhancement)

On by default ([original-bugs.md](original-bugs.md#options)), a key of
its own so that `big_map` can be had with the title bar. When `big_map`'s
main screen fills the window, its own title bar (`Ueberschriftbild` with
the game's name, `CloseBild` and `Kleinbild`) only repeats the window's,
so it goes: the info bar moves to the top, the button bar under it
(18), and `Hauptbild` and the right scroll bar start at 43 instead of 59,
the map view growing by those 16 pixels (`layout_for(w, h, false)`; the
three images get empty rectangles). The status bar, the lower scroll bar
and the compass knob stay at the bottom. On 640×480, where the main
screen is the original's floating window, and with `big_map` off, the
title bar stays.

What follows the title bar's place:

- The forms that open over the bars go to `Hauptbild`'s top, 43.
- The button bar's tooltips (1020:1e9b tests the rows `0x22`..`0x3b` from
  the form's top) are given the form's top as if it had its title bar,
  so they still go just under the button bar.
- The pointer's info box is placed from the mouse in the form
  (1020:04bc), which `Tools::hauptbild` gives with the island's real
  top.
- Nothing drags or rolls up the form: the top row is the info bar.
  Without `CloseBild` the quit question is still a key away (Escape,
  `Q`, Alt+F4), and the window's own buttons close and minimize it.
- The world's view is the same as with the title bar: the original's
  20 × 27 view still lies in the middle of the bigger one.

The test: `no_main_title` (800×600, 1024×768, 1280×1024 and 1366×700
against `big_map` alone; resizing to 640×480 and back; switching it off
in the game; with `HOLIDAY_PNG` set, `notitle-WxH.png` and
`notitle-WxH-forms.png`), and `hauptschirm::tests::layouts_without_the_title_bar`.

## The port

The platform (`crates/platform`) has a resizable window:

- **The desktop** is the window's client area divided by the UI scale,
  rounded down (`platform::desktop`). The game gets it through
  `App::resize`, before the first frame and after every change.
- **The UI scale** is 1 to 4: the display's scale factor rounded, made
  smaller while the desktop would be under 640×480
  (`platform::ui_scale`), or fixed with `--ui-scale=N`. The GPU scales the
  framebuffer up by it, nearest neighbour.
- **Letterboxing**: a desktop smaller than 640×480 (either way) gets a
  640×480 framebuffer, shown scaled down to fit, centred.
- **Fullscreen**: F11 or Alt+Enter (Ctrl+Cmd+F on macOS) toggle borderless
  fullscreen on the current monitor. The platform keeps those keys from
  the game, which costs the original's F11 shortcut for the "Buttonhilfe"
  button (0x13); the button itself still works.
- **Command line**: `--window-size=WxH` (the client area in logical
  pixels), `--fullscreen` / `--windowed`, `--ui-scale=N`. Without a size
  the window opens at the remembered one, else at 800×600 desktop pixels.
- **Settings file**: `settings.ini` in the config directory
  (`$XDG_CONFIG_HOME/holiday-island`, `~/.config/holiday-island`;
  `~/Library/Preferences/Holiday Island`; `%APPDATA%\Holiday Island`),
  `platform::settings::WindowSettings`. It remembers the windowed size
  (logical pixels), the position (where the window system reports one,
  not on Wayland) and whether the window was fullscreen, and is written
  when the game ends. The original reads nothing like it.

The game (`crates/holiday`, `Holiday::layout`) then does what state 110
does: the framebuffer and `THintergrund` get the desktop's size,
`hintergrund::screen_size` picks the screen size the forms are placed
for, `backgroundimage` gets that size and its picture, and `FilmPanel` is
centred on the desktop.

### Live resizing (port extension)

The original lays out once. When the port's window changes size during
the game, `Holiday::layout` runs again, which:

- picks the screen size and the pictures again (state 110, 430/450);
- centres a film that's playing in `FilmPanel` again;
- centres the start menu (1058:1a30) and puts the load form at the bottom
  (1038:3b46) again;
- in a game (`Game::resize`): centres the main screen again
  (`hauptschirm_layout`, 1020:1ce9, unrolling it), moves every other form
  by as much as the main screen moved, so it stays where it was over it,
  and then keeps it on the screen with `load_window_positions`' clamp
  (1010:08d7), plus a `max(0, ..)` so it can't go off the top or left
  edge either. Shown or not, every form is moved: the auction, build
  menu, sabotage window, file menu, pause, options, TV, money menu,
  overview map, compass, info box, buildings' overview, quit question,
  end form, high scores and tooltip.

The world isn't touched, nor is the scroll position: the map view has the
same size at every screen size, so it keeps showing the same part of the
island. With `big_map` the main screen fills the new size instead
([above](#big_map-enhancement)). A form pushed against an edge by a smaller window stays there when
the window grows again.

The tests (`crates/holiday/src/main.rs`): `layouts` draws the main screen
with the TV, money menu, overview map and compass at 640×480, 800×600,
1024×768 and 1366×700 (`layout-*.ppm` with `HOLIDAY_PPM`);
`small_and_snapped_desktops`; `resize_mid_game` (the world, scroll
position and camera unchanged, every shown form on the screen at each
size); `dragging_the_main_screen`.

## Not done

- Applying the saved windows of a map file (1010:08d7) when a game is
  loaded.
- The web build's Fullscreen API and canvas sizing (no web build yet).
