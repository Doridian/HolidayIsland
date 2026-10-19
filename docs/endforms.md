# The end of a game: quitting, the end form, high scores, tooltips

Four small forms: `TEndeform` asks before quitting, `TSchlussform` shows
the end of a game (and the quit film), `Thighscoreform` the high scores,
and `THelpform` the tooltips. All four are view forms: none of them
changes the world. The high scores are a file (`SOUND\HIGHSCOR.WAV`,
[`formats::highscore`](../crates/formats/src/highscore.rs)), not world
state, so the form keeps the table and the caller writes it.

## `TEndeform`: "HOLIDAY ISLAND VERLASSEN?" (1020:3b5f)

Ported in [`game::endeform`](../crates/game/src/endeform.rs).

Opened from the main screen with Escape or `Q` (alone or with Shift) and
Alt+F4 (1020:2e09), from its close button (`CloseBild`, 1020:0577;
1020:0480 is the old `CloseButtonDblClick`, which nothing uses), and from the file menu's "Beenden" and `Q` (1040:0a4c,
1040:1572). Each caller shows it only if `+0x180` (quitting) is clear.

`FormShow` stops the clock (1020:24a0), disables the other forms
(`disable_forms(3)`), centres the form (185 × 85) and draws
`Exitform.BMP` with `SONSTIGE.TXT`'s texts 1 (title, colour 1), 2 (the
question, colour 7), 3 "OK" and 4 "Abbruch" (colour 9). The parts are a
rectangle table at `1110:18ba` like the file menu's: 0 the form, 1 the
close button, 2 the title bar, 3 the question, 4/5 "Abbruch" (drawn/hit),
6/7 "OK". A pressed button is `BilButPr.BMP` (the close button
`closepre.bmp`) with the text a pixel down and right.

- Mouse down (left only): close button, "Abbruch", "OK" go down with sound
  8; the title bar grabs the form, which shrinks to 16 pixels (the title
  bar) while it's dragged.
- Mouse move: drags it, kept on the screen by its current height (so its
  bottom can go off the screen; with `enhance.form_drags` by its full
  height).
- Mouse up (left only): the button comes up (sound 10); on the button,
  "Abbruch" and the close button hide the form and start the clock
  (1020:24b7); "OK" hides it, sets `+0x180`, saves `MAPS\Last.map`
  (1048:2bae) and shows `TSchlussform`'s quit film (1068:0f52). The title
  bar gets its height back.
- Keys (any shift state): Escape and Space cancel (unless quitting), Enter
  is "OK".
- `FormHide` frees the bitmaps and enables the forms (1020:3d05).

## `TSchlussform`: the end (1068)

Ported in [`game::schlussform`](../crates/game/src/schlussform.rs).

Fields: `Bild` +0x17c (the results page), `Panel1` +0x180 (the film's
display), `Filmplayer` +0x184, `Timer1` +0x188, `MusikPlayer` +0x18c,
`Image1` +0x190 (the film's frame). `+0xf8a` the background music played,
`+0xf8b` the mode (0 end of game, 1 quit), `+0xf8c` a film plays, `+0xf8d`
the end's music plays.

**The end of a game** (`january_second`, 1008:0512, calls 1068:1016):
the clock stops, the background music stops (1018:38c5, remembered in
`+0xf8a`), the help hides, the TV stops (1048:077a), every other form is
disabled (`disable_forms(0xe)`), and `schlussform_init` (1068:09a5) makes
the widgets, draws `schlrahm.bmp` into `Image1` (442 × 330, the film in
`Panel1` at 5, 5, 432 × 320) and loads the texts. Then the music and the
film: a fair win (won, `1110:0071`, and not cheated, `1110:006f`) plays
`gewonn.mid` and the scenario's film (0 `ABrente`, 1 `ABgeld`, 2 `ABleute`,
3 `ABking`, 4 `ABluxus`, 5 `ABsurfer`, 6 `ABoeko`); otherwise
`pleite.mid` and `ABPleite.avi`.

`Timer1` (1068:187f, every second) ends the film when its position
reaches its length, so the last frame stays up to a second, and when the
music is over plays `endeloop.mid` (again and again); the port runs it
once a second too (`Game::end_music_tick`). A key
(`FormKeyDown`, 1068:198a), a click on `Panel1` (1068:199b) or any event
on the background form while this one shows (1000:3d0c) skip the film
(1068:17ad). After the end-of-game film the results page shows
(1068:1340): the form 432 × 320, centred, `schluss.bmp` with:

| Widget | Where | Text (`Schluss.txt` line, 0-based) |
|---|---|---|
| label +0x712 | 16, 0, 416 × 16, colour 1 | 0 "SPIELENDE" |
| label +0x821 | 16, 32, 400 × 32 | 1 |
| label +0x930 | 16, 74 | 2 (fair win) or 3 |
| label +0xa3f | 16, 116 | 4 for a cheater; else 5–12 (won) or 13–20 (lost) by scenario: 0 first, 7 (the tutorial) second, 1–6 after |
| label +0xb4e | 16, 158 | 21 + net worth + 22 |
| label +0xc5d | 16, 200 | 23 + last 12 months' guests + 24 |
| label +0xd6c | 16, 242 | 25 + score (0 without one) + 26 |
| label +0xe7b | 16, 268 | 27, only if the score went in (`1110:006c`) |
| button +0x1a4 | 15, 294, 90 × 22 | 28 "Weiterspielen", only after a fair win |
| button +0x2ba | 119, 294 | 29 "High-Score" |
| button +0x3d0 | 223, 294 | 30 "Beenden" |
| button +0x4e6 | 327, 294 | 31 "Neues Spiel" |
| button +0x5fc | 0, 0, 16 × 16 | the close button |

The joined texts have a space each side of the number. The buttons'
`Click`s (VMTs `1110:2522`..`25b2`): "Weiterspielen" (1068:0180) closes
the form (1068:1593: hide, timer off, the end's music stops, the forms
enabled), brings the main screen to the front, starts the clock and the
background music if it played; "High-Score" (1068:01b3) enables the high
scores, disables this form and shows them (1040:2579); "Beenden" and the
close button (1068:01df, 1068:0218) hide `Bild` and play the quit film;
"Neues Spiel" (1068:0201) closes the form and calls `back_to_start_menu`.

**Quitting** (1068:0f52, from `TEndeform`'s "OK"): the clock stops, the
TV stops, the forms are disabled, the form is made and shown and
`ABEnde.avi` plays (1068:0e68, mode 1). At its end (or skipped) the form
closes, everything is freed (1030:3974: `game_done`) and the program
`Halt`s.

The port hands the films and the music to the caller as file names in the
[`Response`](../crates/game/src/schlussform.rs), and `stop_film` when one
ends; the caller plays the film in `Schlussform::film_panel` and calls
`Schlussform::film_ended` at its end ([films.md](films.md)). The music plays
([sound.md](sound.md#the-music)): the caller asks `music_ended` when the
end's player has reached its length.

## `Thighscoreform`: the high scores (1040:1853–1040:3347)

Ported in [`game::highscoreform`](../crates/game/src/highscoreform.rs).

The table (`*1110:6ee4`, 0x1252 bytes) and `HIGHSCOR.TXT`'s 16 lines
(`*1110:6edc`... `highscor_texts`) are loaded when the form is created
(1040:3183). `1110:6ee8` is the table shown.

- `add_highscore` (1040:1bdf), from `january_second` after a fair win with
  money not frozen: the last rank of the scenario's table becomes the
  player's name (`1110:0a02`, the start menu's), today's date (the clock's,
  `DateToStr`), the score and the net worth's money text (each a
  `string[20]`), and the tables are sorted and saved.
- Sorting (1040:1853): for each of the 7 tables, 10 bubble passes of 9
  comparisons, swapping when an entry's score is less than the next's (so
  ties keep their order). The strings are swapped by `PStrStore`, so the
  bytes after them stay. Then the table is saved.
- `FormShow` (1040:2579, with 1040:2bf0): sorts (and saves), loads the
  bitmaps, makes the widgets, 640 × 300 centred, ticks the scenario's box
  (calling its `Click`, which shows that table), draws `highscor.bmp`, the
  widgets, the title (text 0), "Spielziele/Szenarien" (3) and the column
  headers (11–15, colour 11), and the table.
- The table (1040:1d49): rows 48–207 restored from the background, then
  for each rank (16 pixels apart from y 48) its number (the tenth "..."),
  name, date, score and money, centred in 128-pixel columns, colour 9.
- Widgets: "Schließen" (text 1, 528, 265, 100 × 20) and the close button
  close the form (1040:1bc2, 1040:1ba5 → 1040:1b5c: enables the end form,
  or the options if they opened it, `+0xc8c`; disables and hides this one);
  "Löschen" (text 2, 421, 265) clears the shown table (1040:1ca7: scores 0,
  lengths 0) and saves it. Seven check boxes (texts 4–10, `strakn.bmp`/
  `straknpr.bmp`, colour 9, ticked 10) pick tables 1–6 and 0; they're
  pressed as radio buttons (`radio_mouse_down`), and each one's `Click`
  unticks the others and shows its table.
- Mouse down (1040:31bc) needs the left button; mouse up (1040:3347).
- Opened from the end form (1068:01b3) and the options (1060:0175, which
  sets `+0xc8c` and disables the options).

## `THelpform`: the tooltips (1040:3d02–1040:3ead)

Ported in [`game::helpform`](../crates/game/src/helpform.rs).

`FormCreate` (1040:3d02) loads `Help.bmp`, puts `Bild` 150 × 16 at the top
left and shows the form 0 high. `helpform_show(text, top, left)`
(1040:3dc7): unless it already shows `text`, draws the bitmap and
`SONSTIGE.TXT`'s short text `text` centred (colour 9, y 1), puts the form at
`min(screen_h - 16, top)`, `min(screen_w - width, left)`, disabled, 16 high,
in front; always sets `1110:009c` to 10 (never read). `helpform_hide`
(1040:3ead) sets the height to 0.

Step 3 of `AnimationsTimer` (1020:1e9b), with the tooltips option
(`1110:0060`):

1. The money menu, if it or the bubble is active, it's enabled and
   `+0x696` is clear: its buttons 9–14 (1070:0294 against the cursor) give
   texts `1110:1627 + button` (29–34), the bubble at the button's bottom
   left.
2. The overview map, likewise with `+0x212`: buttons 16–20 (10b8:02e9) give
   `1110:1626 + button` (24–28).
3. The main screen, if it or the bubble is active, it's enabled and
   `+0x1da` is clear (it isn't rolled up by a double click on its title
   bar, [layout.md](layout.md#the-main-screen)), with the cursor in rows 34–59 of the form (the button
   bar) and its width: `button_at_mouse` (1018:394e) finds the button by
   `mouse_x` and the edges at `1110:12c0` (the first whose edges hold it,
   none in the gaps); its text is `1110:163c + button` (the button's
   number), the bubble at the form's top + 59 and the button's left edge.
4. If none of them showed the bubble, it hides.

So when the mouse is over several, the last one wins. The port's main
screen clock asks the three in that order (`Game::tooltips` in the
`holiday` binary): `Geldmenuform::tooltip`, `Uebersichtsmap::help_text_at`
and `helpform::button_bar_tooltip`, with the form's focus for "active"
(the build menu and the sabotage window take it from the main screen).

## The later build

The later build ([versions.md](versions.md)), which the port follows,
changes the end form and the high scores for its six scenarios of
`UPGRADE\`:

- `UPGRADE\SCHLUSS.TXT` (en 1070:0294) has 44 lines: the blocks of
  reasons have 14 lines each, the games 0, 7, 1–6 and then 8–13, so the
  lost block starts at 19 and the rest (net worth ...) at 33. Without
  `UPGRADE\` the port reads `TEXTE\SCHLUSS.TXT` as v1.0 does.
- A fair win of scenarios 8–13 plays `ABFries`, `ABCamper`, `ABBBonly`,
  `ABPRWund`, `ABHurry` or `ABChalle.avi` (en 1070:128d); 7 still has
  none.
- The high score table has 14 games, `0x3aa` bytes a rank (en
  1068:0109, `game::highscoreform::ScoreTable`). Games 0–6 are
  `SOUND\HIGHSCOR.WAV` as before; games 8–13 go to
  `UPGRADE\HIGHSCO2.WAV`, slots 1–6 of a file of the same layout (slot 0
  holds a copy of game 0's), read by en 1068:1adf and written by en
  1068:1e08. Game 7, the tutorial's number, has a table in memory that
  isn't saved: v1.0's overrun is gone. The port keeps the player's
  copies, `HIGHSCOR.WAV` and `HIGHSCO2.WAV`, next to their saves.
- The form (en 1068:29ac) draws `UPGRADE\highsco2.bmp` and has
  thirteen check boxes, `UPGRADE\strakn2.bmp`/`strakn2p.bmp`, 126 × 13:
  games 1–4 at x 3, 5, 6, 8, 9 at x 0x81, 10–13 at x 0x109, each column
  from y 0xe6 every 0x11, and the normal game at 0x1a5, 0xe6. The new
  scenarios' names are `UPGRADE\HIGHSCO2.TXT`'s six lines (en
  1068:2493). "Szenarien" (text 3) is drawn at y 0xd6.

## The port

| Form | Module | View API |
|---|---|---|
| `TEndeform` | `game::endeform` | `Endeform::new(font, bitmaps, sonstige_texts)`, `open(screen)`, `mouse_down/move/up`, `key_down`, `image()`, `bounds()`; `Action` `Close`/`Quit`; `main_screen_opens(key, shift)` |
| `TSchlussform` | `game::schlussform` | `Schlussform::new(font, bitmaps, schluss_lines(..))`, `open_end(Results::of(..), music_on, screen)`, `open_quit(screen)`, `film_ended`, `key_down`, `music_ended`, `mouse_down/up`, `components()`, `bounds()`; `Action` `Continue`/`HighScores`/`NewGame`/`Exit` |
| `Thighscoreform` | `game::highscoreform` | `Highscoreform::new(font, bitmaps, highscor_lines(..), table)`, `add(..)`, `show(scenario, from_options, screen)`, `mouse_down/up`, `image()`, `bounds()`, `table`; `Response::save` |
| `THelpform` | `game::helpform` | `Helpform::new(font, Help.bmp, short_texts)`, `show(text, top, left, screen)`, `hide()`, `image()`, `bounds()`; `button_bar_tooltip(..)`, `MONEY_MENU_TEXTS` (`Geldmenuform::tooltip`), `OVERVIEW_TEXTS` (`Uebersichtsmap::help_text_at`) |

None of them issues a world command. The clock stops while the quit
question and the end form are up (single human: the simulation waits on
these dialogs; see [original-bugs.md](original-bugs.md#design-issues-for-multiplayer)).

With more than one human client (decision 8 in
[port-architecture.md](port-architecture.md#decided)) the quit question,
the end form and the high scores stop nothing: the view keeps ticking
(`Game::clock_stopped` in the `holiday` binary), and the world goes on
after the end of a game. A client's "OK" is their leaving
(`SeatChange::Leave`): a computer plays their seat once its last client
has gone. Each view shows its seat's results
(`Tick::results_for`): every scenario ends for everyone when the first
human seat reaches its goal, and the other seats still in lose (decision 9, [simulation.md](simulation.md#the-scenarios-end-10080512)),
a seat that goes bankrupt gets its loss and spectates, and one human's
cheat counts for everyone (decision 10). The high scores stay each
view's file, entered from its seat's results.

Not ported: `game_done`'s freeing (the port drops the game).

## Open questions

- `FUN_10e8_6258`, the "active" test of step 3, is assumed to be the
  form's `Active`.
- Whether the tutorial is ever scenario 7 (its texts exist in
  `Schluss.txt`; its high score overran v1.0's table, the later build
  keeps a table for it in memory).
