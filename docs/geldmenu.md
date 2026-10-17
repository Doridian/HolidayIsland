# The money menu (`TGeldmenuform`, segment 1070)

Ported in [`game::geldmenu`](../crates/game/src/geldmenu.rs): the form
(`Geldmenuform`, the view) and what it changes in the world
(`geldmenu::Action`, through `command::Action::MoneyMenu`). The books it
shows are the world's ([settlement.md](settlement.md#the-books-tgeldmenuform-10701419),
`game::accounts`). Single human: the form, the books, the insurance and
the balance are player 1's.

## Opening and closing

- The button bar's button 4 (or `5`/`G` on the main screen, 1020:2e09)
  shows it at page 0 (1010:2c8b → 1070:2b11 with 0); pressed again it
  closes (1070:2bb4). The year's end shows it at page 1 if the option
  `1110:0064` is on (1008:4f87, before the renovations): `Tick::new_year`,
  with the option the view's (`optionen::Settings::year_balance`).
  Button 4 stays up then; with `enhance.money_menu_button` the year's
  end presses it.
- `geldmenu_show_page` (1070:2b11): another page is drawn first
  (1070:20e7), then `geldmenu_show` (1070:2b3a) closes the info box
  (1038:1ee0), enables the form, keeps `button_clicked` (`+0x20e`, never
  read), works out the year's total (1070:1452), takes the focus, sets the
  height to the first rectangle's bottom − 1 (with
  `enhance.form_positions` + 1, as a drag gives it) and plays 0xd, then
  0x25 on the balance sheet or 0xd again.
- `geldmenu_close` (1070:2bb4): sound 0xe, the button bar's button 4 up
  (1010:2ade), height 0, the main screen focused.
- The form is created once (`geldmenu_init`, 1070:2e6c) at the top right
  (left = screen width − the first rectangle's right, a column too far;
  with `enhance.form_positions` − its width), shown with height
  0. "Shown" means height above 0 everywhere (1070:2617, the timer's
  tooltips use `Visible`, which is always true).
- It isn't modal and the clock runs on. While it's shown every booking
  refreshes it (1070:2617, `Tick::books`).

## Layout

Three `TImage`s, placed and sized from `GELDMENU.DAT` (118 rectangles of
left, top, right, bottom, inclusive; `formats::data::geldmenu_dat`):
rectangles 1–3 place `UeberschriftImage`, `BildImage` and `MenuImage`, 4,
0x11 and 8 size them. The others are buttons and text fields on them
(1070:0b76 says which image: 4–7 the title bar, 8–0x10 the menu bar,
0x11–0x75 the page; 1070:0be5 which background they're erased from:
`geldueb.BMP`, `geldmenu.BMP`, or by page `geldfin.BMP`, `geldbil.BMP`,
`geldstat.BMP`). Texts go through one routine (1070:0dea) with nine
styles: right, centred, left in colour 9, centred in colour 1, the same
erasing first, red (11) right-aligned for negative amounts, and centred a
pixel off for a held button.

| Rectangle | What |
|---|---|
| 5, 6, 7 | The close button, the drag area, the title ("FINANZEN", `SONSTIGE.TXT` 0xe) |
| 9–0xe | The tabs, pages 0–5 (`1110:323e`); the page's tab is drawn from `geldmepr.BMP` |
| 0xf, 0x10 | "Schließen" (`GELDMENU.TXT`'s last line) and its text |
| 0x12, 0x13 | Renovation: left (`1110:329a`: a year more, after five never) and right (`1110:3294`: a year less, from never five) |
| 0x14, 0x15 | Advertising: one level less (at least 0), more (at most 7) |
| 0x17, 0x18 | "Geld leihen", "Kredit tilgen", by `Kreditgroesse` (`1110:64ca`) |
| 0x19–0x1b | The advertising level's text (`MODATYP.TXT`'s `WerbungToText`), its monthly cost, the renovation interval's text (`RenovierungToText`) |
| 0x1c–0x34 | `GELDMENU.TXT`'s texts |
| 0x35–0x51 | The currency next to the amounts |
| 0x52–0x6e | The books' kinds of the same number (page 1; 0x6c–0x6e also on page 0) |
| 0x6f–0x75 | The chart, its title, the scale's top, middle and bottom, its first and last date |

Widgets of the toolkit ([widgets.md](widgets.md)): on page 0 the
insurance's check box (`+0x697`, a radio button's object toggled by
`checkbox_mouse_down`, text "Versicherung", `geldch.BMP`), the labels
"Schutz vor Katastrophen:", "Kosten im Monat:" and the cost (a 120th of
the buildings' value if insured, 1008:062c, with `_` before the
currency); on the menu bar "Gebäudeübersicht" (`+0x24f`), whose `Click`
(1070:01e8) opens `TUmenuform` (1080:0ea8).

## The pages

`geldmenu_draw_page` (1070:1eeb), which every refresh runs: page 0's
widgets hidden on the others (1070:1ba5), the background, the labels
(1070:28d3), the values (1070:270f), the chart (1070:16b7), page 0's
widgets shown (1070:1d1f).

- 0, `Schreibtisch`: advertising, renovation, insurance, the two loan
  buttons, and the balance (kinds 0x6c–0x6e).
- 1, `Bilanz`: last year's kinds 0x52–0x5e and this year's 0x5f–0x6b,
  with the balance.
- 2–5: the books' history rings (1070:210a): money less the loan, net
  worth, guests, reputation. Each player's line from the last player's
  to player 1's, in their colours (`spieler.bmp`, 2 × 2 dots,
  1070:14b4), from month to month across the chart; y is
  `v · (top − bottom) / range / 2 + (max · bottom − min · top) / range /
  2`, `range = max div 2 − min div 2` (no lines when it's 0), in x87
  `Double`s, rounded. Lines step along their longer axis (1070:151b). The
  scale's labels are money (pages 2, 3) or numbers; the first date is the
  month 120 months ago (1030:0263, January if the game is younger) and
  `max(year − 10, 1996)`, the last the date.

## Events

| Image | Event | Does |
|---|---|---|
| Title | down (1070:0820) | Left: the close button pressed (`closepre.bmp`, sound 8). Any button on the drag area (with `enhance.mouse_buttons` the left one): drag, the form rolled up to 16 pixels, for good with a double click (`+0x696`) |
| | move (1070:08e0) | With the left button: the form follows, kept on the screen |
| | up (1070:096d) | The close button up (sound 10), closing if still on it; a drag ends at the full height (the first rectangle's bottom + 1) unless double-clicked |
| Page | down (1070:0300) | Left: the check box (sound 9, its `Click` 1070:01fc sets `1110:0072`), else on page 0 a button pressed (sound 8) |
| | up (`geldmenuform_bild_mouse_up`, 1070:04d3) | Left, page 0: the button up (sound 10), and if still on it its action, then the values redrawn |
| Menu | down (1070:09f7) | Left: "Gebäudeübersicht", or "Schließen" pressed (sound 8), or a tab of another page (sound 9) |
| | up (1070:0add) | "Gebäudeübersicht" up (its `Click`), or "Schließen" up (sound 10), closing if still on it |
| Form | key (1070:39bf) | No modifier or Shift: Enter, Escape close; Space focuses the main screen; Left/Right the previous/next page (`1110:3232`, `1110:3238`); `1`–`6` a page; any other key goes to the main screen's `FormKeyDown`. Alt+F4 closes |

The main screen's timer shows the tabs' tooltips (1020:1e9b, with
`1110:0060`, while the form or the help is active and it isn't rolled
up): `SONSTIGE.TXT`'s short texts 0x1d–0x22 (`1110:1630`), under the tab.

## World and view

The world's part (`World::money_menu_command`):

| `geldmenu::Action` | Original | Checks |
|---|---|---|
| `Shown(bool)` | 1070:2b3a / 1070:2bb4 | Player 1 only: the flag `World::money_menu_shown`; showing works out the year's total and the balance |
| `TakeLoan` | `take_loan(1, Kreditgroesse)` (1078:05c6), the balance (1070:2ab8) | Past `MaxSchulden` the bank says no (`1110:6f28`), nothing changes |
| `RepayLoan` | `repay_loan` (1078:061b), the balance | The money must be there and something owed |
| `Advertising { more }` | `*1110:6f24 + p − 1` ±1 | 0 to 7 |
| `Renovation { more }` | `1110:1334[p]` through the tables | An interval past 5 stays |
| `Insurance(bool)` | `1110:0072` | Player 1 only |

The year's total (1070:1452): kind 0x6b is the sum of 0x5f–0x6a, then
(1070:2ab8) 0x6c, 0x6d, 0x6e are player 1's money (1078:0191), minus the
loan and the difference: shown on pages 0 and 1, and saved; nothing else
reads them. The original runs it when the form is shown, at the year's
end (1070:269d), and on every refresh (1070:2617) while the height is
above 0, which is the view's state the books follow. The port keeps that
condition as `World::money_menu_shown`, set by `Action::Shown` (decided
for the open question in [simulation.md](simulation.md#open-questions)),
and runs the refresh at the end of each `World::tick` and `World::apply`
that booked (`Tick::books`) or loaded a game (`Tick::loaded`, 1070:2630).
The original refreshes right after each booking; only the saved kinds
0x6b–0x6e could tell the difference, when money changes after a booking
in the same tick. The year's end shows the form after the books' year
closed and before the renovations; the port's view does that when it
sees `Tick::new_year` (one command later), so 0x6b may then hold the
renovations already.

Loading a game (`game_show_state`, 1010:26fa) toggles the check box to
the loaded insurance and redraws the menu bar
(`Geldmenuform::loaded`).

The map saves the form's page (`+0x188`), pressed element (`+0x205`),
drag offsets (`+0x206`, `+0x20a`), `+0x20e` and the roll-up (`+0x696`)
with the books (unit 1070:3bf8): the view's, kept in
`savegame::MapView::unported`; the port's form doesn't take them over
from a loaded game yet (it keeps its page).

## Open questions

- The tooltips' `FUN_10e8_6258` is probably `TCustomForm.Active` (or
  `Focused`).
- `1070:2653` clears last and this year's kinds; nothing seen calls it.
