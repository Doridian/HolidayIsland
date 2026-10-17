# The news and the TV (`TFernseher`, segment 1048)

The news about the island and the world (`NACHRICH.TXT`), the archive of
the last 26, and the TV that shows them. Ported in
[`game::news`](../crates/game/src/news.rs) (the world's part: posting a
message, the archive, the pause) and
[`game::fernseher`](../crates/game/src/fernseher.rs) (the form). Tested in
`crates/game/tests/orig_fernseher.rs`.

## The data (`init_messages`, 1048:1ffd)

Each game's set-up (state 1220, `load_game_rest` 1030:36e5) runs it:

- `TEXTE\NACHRICH.TXT`: a title (`string[30]`, `*1110:6634`), then three
  lines per message for 90 messages, into three arrays of 90 strings
  (`1110:663e`): message `n`'s text `v` is `lines[v][n]`, and an entry's
  text number `t` is `lines[t mod 3][t div 3]` (`formats::data::nachrich`).
- `DATA\FILENAME.DAT`, `FILMNAME.DAT`, `SOUNNAME.DAT`: per message the
  picture (in `GRAFIK\`), the film (in `FILMHI\`/`FILMLO\`) and the
  speaker's sound (in `SPRADEUn\`), `string[12]` each in 90 records of 0x27
  (`*1110:6630`); `kein` is none. Two pictures don't exist on the CD
  (`Xpiranh`, `Xstrkapu`, messages 3 and 26).
- The archive (`*1110:663a`): 26 empty entries (day -1, `kein Datum`).
- The TV's position, centred: `((width - 352) / 2, (height - 334) / 2)`
  (`1110:664a`, `664c`); nothing posted yet (`1110:667c` = 1), the mouse
  up (`6663` = 1), the page and the last entry 0 (`6638`, `6664`).
- The bitmaps `tvbild` (the TV, 352 × 319), `tvanim` (its two lights),
  `tvknopf` (the buttons held down) and `tvleis` (the title bar).

`1048:249d` frees them all at the end.

## Posting a message (1048:0a54)

`post_message(number, a, b, x, y, j, i, centre)`; callers that have no
square pass the human's view origin (`1110:5530`).

1. `1110:667c` := 0: something was posted.
2. The entry: the first empty one (day -1) of 0..24, else 25, after
   entries 1..25 moved down to 0..24. The first wrap moves the empty 25
   into 24, so the 27th message goes into 24 and the 26th and 27th stay
   the wrong way round for good (kept).
3. `1110:6638` (the TV's page) and `6664` (the last entry) := it; `662e`
   (the first button) := `centre`.
4. The entry (0x11c bytes): `+0` the day count (`1110:653a`, `Longint`),
   `+4` `a`, `+6` `b`, `+8` `x`, `+0xc` `y` (`Longint`s), `+0x10` the
   text `number * 3 + Random(3)`, `+0x12` the number, `+0x14` the date as
   the info bar shows it (`date_text`, 1030:0148, `string`), `+0x114` the
   square (`i`, `j`), `+0x118` the human's view origin.
5. By the message's kind (`1110:199e[number]`): 0 to 3 open the TV if the
   option `1110:005c + kind` is on (events, new buildings, sabotage,
   congratulations, the options form's names), and then, with `centre`,
   first scroll the view to the square (1020:0787); 4 always opens it,
   without scrolling; 255 never. Opening it: `fernseher_show` (1048:12e9)
   on the entry, `1110:667d` := 1 (the TV is the news'), the speaker's
   sound opened if the option `1110:0066` is on and the message has one
   (`tv_busy`, `1110:662d`), sound 0x24, and the time (`GetTickCount`,
   `1110:6676`).

In the port the world does 1 to 4; 5 is the view's: `Tick::news` hands
it each message as it went into the archive (`Posted`: the entry, its
index, its kind of news and `centre`). The options `005c`..`005f` are the
player's view settings (`optionen::Settings::news`, which the options
form changes), saved with the view (`MapView::news`, unit 1000:0c6a):
`Posted::opens_tv` and `Posted::scrolls` decide with them whether the
view's TV opens and the view scrolls.

## The TV opening (`fernseher_show`, 1048:12e9)

Unless nothing was posted yet (then only the button bar's third button
comes back up):

- the pause (`1110:0067` := 1), the button bar's third button down;
- `667d`, `667a` (the speaker started), `6672` (the film's state) := 0;
- the form at its position; for the announcement of a building's sale
  (message 0x15) the people of kind 0x46 on each square of the building
  on the entry's square (`add_person`, `HAUSNR.DAT` columns 3 and 4) and
  the form at the screen's left edge;
- the form 352 × 335: `tvleiimage` (0, 0, 352 × 16) and `tvimage` (0, 16,
  352 × 319);
- the page drawn (1048:087f), the title bar (`tvleis`, the title centred
  in 336 pixels at 0, colour 10), 1038:1ee0 on the info box, the
  form shown, sound 0xd, the time.

The page (1048:087f): `tvbild`, the message's picture (125 × 125 of
`FILENAME.DAT`'s bitmap at (54, 72), loaded each time; a missing file
raises an exception in the original, the port leaves it out), the first
button's picture (`tvknopf` at 0, 0) if the view is at the news' square
(`662e`), the date (at (192, 38), 135 wide, centred, colour 9) and the
text (at (24, 214), 305 wide, centred, colour 10).

The text (1048:03c5) is the line up to its `*`, with `&1` and `&2` the
names of the players `a` and `b` (`1110:0902`), `}1` and `}2` the amounts
`x` and `y` (`format_money`), `$` the currency; each of these takes the
character after it too and puts a space there. The CD's German texts
have none of them.

In the port the world's part (the pause and the people) is the command
`Action::ShowTv(n)`, for the button bar's button and for a message alike:
the view's TV sends it before the next tick, which is when the original's
pause, set in the middle of a tick, takes effect too. For a message the
sale's people therefore come after the rest of the tick rather than in
the middle of `post_message`, where the original adds them; nothing in the
same tick reads them.

## The timer (`fernseher_tick`, 1048:1b3e)

The clock's step 8, every 100 ms while the TV is shown; only for a TV a
message opened (`667d`):

- 3 s after it opened, with the speaker's sound open, the music pauses
  and the speaker starts (`667a` := 1).
- The two lights (`tvanim`, 40 × 16 at (242, 117) and 40 × 20 at (243,
  91)): frame 0, or while the speaker speaks a `Random(5)` frame and, one
  time in `Random(20)`, a `Random(3)` + 1 one: drawn from the world's
  `RandSeed`, so the original's numbers part from a game without the
  speaker.
- When the speaker is done, or 6 s after the TV opened without one: a
  page without a film closes the TV (1048:077a). Else, if the player
  hasn't browsed (`6672` = 0), the film starts (1048:01bd: the main
  screen's info disabled, `Panel1` at (9, 25), 332 wide, `662c` := 1,
  `6672` := 1); when it ends the TV closes.

The port has no speaker (as with the option off) and no film: a film ends
as it starts, so the TV closes 6 s after it opened unless the player
browsed and the page has a film. `Fernseher::timer` takes the caller's
clock in ms.

## Closing (1048:077a)

If the form is shown: its position kept (`664a`, `664c`), its size 0, the
button bar's third button up, the film and the speaker stopped, the form
hidden, sound 0xe, the main screen shown. Then, shown or not, the pause
off (`1110:0067` := 0) and the music on again. `Action::TvPause(false)` in
the port.

## The buttons and keys

`tvimage` (1048:15e6, 1048:1794), by the rectangles at `1110:1a52`:

| Part | Where | Down (with the mouse up before, sound 8) | Up on it, if it went down |
|---|---|---|---|
| 1 | 107–139, 278–310 | | To the news' square, or back to the view origin of the entry (`+0x118`, which 1020:0787 moves 10 rows on) |
| 2 | 143–175 | `tvknopf` at 36 | The page before, down to 0 |
| 3 | 179–211 | `tvknopf` at 72 | The page after, up to the last (`6664`) |
| 4 | 215–247 | `tvknopf` at 108 | Off |

Each release plays sound 10, and the page is drawn again. The part that
went down (`664e`) is never cleared, so a press elsewhere and a release on
a button that went down before still clicks it (kept). Browsing sets the
film's state to 3: the film doesn't start and the TV doesn't close by
itself, unless the page has no film. A press stops a playing film.

`tvleiimage` (1048:288f, 2955, 2922; 1048:1939, 1a4b): a press on the bar
(x 16 to the form's width, y 0 to 15) grabs the form: it shrinks to the
bar while dragged (kept on the screen) and comes back on release, unless
the press was a double click, which leaves it rolled up until the next
press. A press on the close button (0–16) draws `closepre`, and the
release on it closes the TV.

Keys (1048:26f6), without Ctrl and Alt: Escape and Space off, the arrows
left and right the page before and after, Enter the first button (sound 9
each); Alt+F4 off.

## Saved

Unit 1048:25ea/2524 saves `6664`, `667c`, `6638`, the archive, `664a`
(4 bytes), `662e`, `664e`, `6662`, `6663`. In the port the archive,
`6664` and `667c` are the world's (`World::news`); the rest are the TV's
(`Fernseher::load_fields`, `store_fields`, through `MapView::unported`).
The news options `005c`..`005f` (unit 1000:0c6a) are the view's too
(`MapView::news`).

## Multiplayer

- The TV pauses the game for everyone, and the options that open it are
  one player's: the simulation waits on that player's dialog. For one
  player the port pauses exactly as the original. The intended design
  for multiplayer (not built yet): the news options stay each player's
  view setting (they already are), and news never pause the game, since
  each player can dismiss their own TV: in a game with more than one
  human seat the view doesn't send `Action::ShowTv`'s pause (nor
  `TvPause`) for a message, only the world's part without it (the sale's
  people). The TV's button would need the same rule.
- The archive's view origin (`+0x118`) is player 1's view.
- The speaker's lights draw from the world's `RandSeed` (not ported).
