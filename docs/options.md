# The pause and the options

The main screen's button bar has the pause as its button 1 (the keys `2`,
Pause and `P`) and the options as its button 2 (`3` and `O`). Both are
forms that `button_bar_click` (1010:2c8b) creates when the button goes
down (`TApplication.CreateForm`-like, 10f0:24ba, into `1110:7236` and
`1110:6edc`) and frees again when they close (`Release`, 10f0:54bd). Both
stop the clock while they're open. Ported in
[`game::pauseform`](../crates/game/src/pauseform.rs) and
[`game::optionen`](../crates/game/src/optionen.rs).

## The pause (`TPauseform`, segment 1040)

`pauseform_show` (1040:3a45) stops the clock (`hauptschirm_stop_clock`,
1020:24a0: the main screen's `AnimationsTimer` off), hides the button help
(1040:3ead) and disables the other forms (`disable_forms(0xb)`). The form
is 350 × 210 in the middle of the screen (top `height / 2 - 105`, left
`width / 2 - 175`), `Pausebild` fills it, and the top left of
`GRAFIK\Pause1.BMP` is copied into the picture.

A click on the picture (`PausebildClick`, 1040:3bf4) or any key
(`FormKeyDown`, 1040:3c05, which doesn't look at the key) closes it
(1040:3bac): the form hides, button 1 comes up (`button_bar_up(1)`), the
forms are enabled again, the bitmap is freed, the main screen gets the
focus (its VMT+0x78) and the clock runs again (1020:24b7). `FormClose`
(1040:3c16) frees the form (`caFree`).

So the pause is the clock standing in the view, not a world flag (the
TV's pause is `1110:0067`, [`Clock::paused`](../crates/game/src/simulation.rs)).
The form gives the world no commands.

## The options (`TOptionenform`, segment 1060)

`optionenform_show` (1060:0789) disables the other forms
(`disable_forms(10)`), hides the button help and stops the clock, makes
the form (1060:14f1: fifteen bitmaps from `GRAFIK`, the widgets, 342 × 377
at the top of the screen, centred across, and the texts, 1060:0ed0), sets
the widgets from the globals, draws `optionen.bmp` and shows every widget.
`optionenform_close` (1060:0e80) hides it, enables the forms, lets button
2 up, frees the bitmaps (1060:231c), starts the clock and focuses the main
screen.

The texts are the first 26 lines of `TEXTE\OPTIONEN.TXT` (the file has
more, which nothing reads): the title, the widgets' texts and the eight
labels. The tracks' names are the form's own (1060:14a1..).

| Widget (instance offset) | Global | Click | Key | Whose |
|---|---|---|---|---|
| Close button (`+0x1c8`), "OK" (`+0x2de`) | | 1060:0151, 1060:0163: close | Escape, Enter, Alt+F4 | view |
| "TOP" (`+0x3f4`) | | 1060:0175: `Thighscoreform` over the form, which is disabled | `T` | view |
| "INFO" (`+0x50a`) | | 1060:01ab: `TCreditform` (`credits.bmp`); with `enhance.enhancements_screen` the port's [list of enhancements](#the-list-of-enhancements-port-extension) | Space | view |
| "Musik" | `005b` | 1060:01dc: start (1018:3850) or stop (1018:38c5) | `M` | view |
| The seven tracks' check boxes | `6ef2..6ef8` | 1060:01fa: the track playing stays ticked | `1`–`7` | view |
| The tracks' radio buttons | `6efa` | 1060:02b7: an unticked track goes off again; else the others off, the music stops, the new file, and it plays if the music is on | keypad `1`–`7` | view |
| Music volume, 0–100 | `6330` | 1060:04e2: `midiOutSetVolume` (1018:35f0) | ← → | view |
| "Geräusche" | `0061` | 1060:04f9 | `G` | view |
| Sound volume | `6334` | 1060:050b: `waveOutSetVolume` (10a0:2453), only with sound (shown 0 without) | Ctrl+← → | view |
| "Sprachausgabe" | `0066` | 1060:0522 | `S` | view |
| Speech volume | `6338` | 1060:0534 (10a0:24bc uses it) | Alt+← → | view |
| "Auktionen", "Neue Gebäude", "Sabotage", "Gratulationen" | `005c..005f` | 1060:054d.. | F1–F4 | view: which news the TV shows (`post_message`, 1048:0a54) |
| "Besitzer", "Auslastung", "Keine" | `0058`, `0059` | 1060:0656, 1060:06ba, 1060:071e: the others off, the island redrawn | F5–F7 | view (the camera) |
| "Jahresbilanz" | `0064` | 1060:0595 (`year_end`, 1008:4f64) | F9 | view |
| "Auktion wartet" | `0063` | 1060:05a7 | F10 | world, `Action::SetOption` |
| "Buttonhilfe" | `0060` | 1060:05b9 | F11 | view |
| "Animationen" | `0055` | 1060:05cb: the people's `+0x16` cleared or set (1088:7ee3, 1088:7f4a), the island redrawn or `people_refresh` | F12 | world, `Action::SetOption` |
| Speed, 80–99 | `0096` | 1060:0619: speed `:= 100 - position` | `-` `+` | world, `Action::Speed` |
| "Katastrophen" | `0065` | 1060:0637: off also sets `1110:133c`, played without disasters (the score's 10 points off) | `K` | world, `Action::SetOption` |

Presses (1060:2404), of any mouse button (with `enhance.mouse_buttons`
the left one only), go to the widgets in the order the form makes them
until one takes it; otherwise a press on the title bar right of the close
button grabs the form, which rolls up to its 16-pixel title bar until the
mouse comes up (1060:2b17), and stays rolled up after a double click, like
the auction form and the build menu. Moves (1060:29f0) drag the scroll
bars' thumbs, then the form, kept on the screen.

The keys (1060:2c72) are those of the table, without Ctrl or Alt unless it
says so; a key ticks a box or selects a radio button with its sound and
`Click`, the arrows move a scroll bar one step without a sound, and the
image is refreshed after any key.

In the port the player's settings are [`optionen::Settings`], which the
caller keeps (with the camera's pointers and the music) and hands to the
form when it opens; the form reports their changes
([`optionen::Change`]). The world's four come back as commands.

## The credits (`TCreditform`, segment 1030)

"INFO" creates the form (1060:01ab, `TApplication.CreateForm`) and shows
it (`creditform_show`, 1030:3c42): `credits.bmp`, `stkn.bmp` and
`stknpr.bmp` from `GRAFIK`, `Bild` 640 × 480 at the form's top left with
`credits.bmp` in it (the names, a title bar and a close button's look are
in the picture), the form as big and centred on the screen, the close
button (16 × 16 at 0, 0, `close.bmp`) shown, and the options disabled.
It also makes a "Schließen" button (`SONSTIGE.TXT`'s 14th text,
`stkn.bmp`, 144 × 22 at 248, 452), but never shows it, and a hidden
widget takes no press. Both buttons have the VMT `1110:3d5c`, whose
`Click` (1030:3c0f) is `creditform_close` (1030:3e6f): hide, enable the
options, free the bitmaps, `Release`. `BildMouseDown` (1030:3ec4) and
`BildMouseUp` (1030:3f22) offer the close button, then "Schließen", the
event, of any mouse button (with `enhance.mouse_buttons` the left one),
and refresh `Bild` if one took it; `FormClose` (1030:3f80) frees the
form. It has no keys, and no drag. Ported as
[`game::creditform`](../crates/game/src/creditform.rs). The form is
`fsStayOnTop`; while it's up only it takes the mouse, over the options or
the list of enhancements.

## The list of enhancements (port extension)

With `enhance.enhancements_screen` (on by default) "INFO" opens the
port's list of the [enhancements](original-bugs.md#options) instead of
the credits: [`game::enhanceform`](../crates/game/src/enhanceform.rs).
It's modal like the options (and stays on top like them in either
stacking), 640 × 480 like the credits and centred, made of the game's
own pieces: the high scores' background (its face mirrored down to
480), the options' check boxes, the buildings' overview's scroll bar
(its trough repeated, as it's 200 high), the credits' buttons, the
game's font, and texts in the player's language (German shown here;
`game::lang`, [language.md](language.md)):

- "Spielkorrekturen" (the world's fixes, "für dieses Spiel und neue
  Spiele") on the left, "Anzeige und Ton" (the view's) on the right:
  each key's check box and label, 18 rows at a time, scrolled by
  its bar. The row under the mouse has its key and description
  shown under the lists.
- The notes under it: whether the changes are saved in `settings.ini`;
  with `--original` or `--enhance=` that the command line goes over the
  file again at the next start; with `enhancements_screen` off that
  "INFO" now opens the credits, and the line that brings the list back.
- "Alle an", "Alle aus (Original)", "Mitwirkende" (the credits, over the
  list) and "Schließen" (`SONSTIGE.TXT`'s), the close button, Escape,
  Enter or Alt+F4.

A tick on a fix is the human's `Action::SetFix` (`Command { player: 1 }`,
applied by `World::apply` before the next tick, so the world's hash
changes) and becomes the player's default for new games; a view
enhancement is applied at once, the layout too. Each is saved in
`settings.ini` (`EnhanceSettings::save`) as `enhance.<key>=on|off`;
"Alle an" and "Alle aus" write `original=false|true` and drop the keys'
lines. Single human: whoever switches a fix switches it for everybody; in
a multiplayer game it would be the host's.

## Open questions

- The map saves the player's settings (`005b`..`0066`) with the view:
  `Settings::with_map` reads them from a file, and a save writes what
  the options say now (`MapView::of`: the pointers and the news options
  as `MapView`'s own fields, the rest through `Settings::store`).
- The volumes start as the system's wave volume in the original
  (10a0:2f5e); the port starts them at 100.
- The high scores are `Thighscoreform` ([endforms.md](endforms.md)).
- After "Animationen" goes on, the original runs `people_refresh`
  (1088:7fb1) from the view, which places the people for the view and
  calls `spawn_people` with the world's random numbers. The port runs it
  in the world with `Action::SetOption`, for the view the world last
  had ([people.md](people.md#after-the-view-moves-10887fb1)).
