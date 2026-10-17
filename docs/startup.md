# Startup

What happens from `entry` until the start menu (`TStartform`) is up. Field
offsets like `+0x184` are into the `THintergrund` object; what the VCL calls
do is inferred from how they're used until the VCL subset is mapped.

## `entry` (1000:0011)

1. `InitTask`, then the VCL/RTL unit initializations. No game unit has an
   initialization section.
2. Quits at once if `GetModuleUsage` is above 1, i.e. another copy is running.
3. `Application.Title := 'Holiday Island'` (probably).
4. `Application.CreateForm` for `THintergrund` (`hintergrund`, the main form),
   then `init_paths`, then 17 more forms in this order: `TAuktionsform`,
   `TBaumenue`, `TDateimenue`, `TDummyForm`, `TEndeform`, `TGeldmenuform`,
   `THelpform`, `Thighscoreform`, `TInfobox2`, `TKompassform`, `TLadenForm`,
   `TFernseher`, `TSabowin`, `THauptschirm`, `TSchlussform`, `Tumenuform`,
   `TUebersichtsmap2`. The symbol file names the global each one goes into.
5. `Application.Run`.

`TStartform`, `TOptionenform`, `TCreditform`, `TPauseform` and `TWasNunForm`
aren't created here; the game creates them when it needs them.

## `init_paths` (1000:17bc)

Ported as [`formats::setup`](../crates/formats/src/setup.rs).

- Without `setup.dat` in the current directory: a message box ("Bitte
  starten Sie zunächst SETUP.EXE ...") and `Halt`. Before showing it, it stops
  `hintergrund`'s timer.
- Reads the five lines of `SETUP.DAT` into the directory globals and the
  sound flags (`sound_enabled`, `sound_lo`).
- The CD check: `Modatyp.dat` in the data directory and 21 AVIs in the film
  directory must all exist (`FileExists`), or it's another message box ("Bitte
  legen Sie die HOLIDAY ISLAND CD ...") and `Halt`. Nothing else about the CD
  is checked.

## The intro: `THintergrund`'s timer (1000:2466)

`THintergrund` has a timer at `+0x184`. Its handler is a state machine on the
timer's `Tag`; each state does its work, sets the next `Tag` and the
`Interval` until the next tick. The other components it uses:
`FilmMediaPlayer` (`+0x17c`), `backgroundimage` (`+0x180`), `MusikMediaPlayer`
(`+0x188`) and `FilmPanel` (`+0x18c`), which the video plays in. `Timer1` is
`+0x184`.

| Tag | Does | Next | Interval (ms) |
|---|---|---|---|
| 100 | Loads nine bitmaps for the file menu's buttons (`close.bmp`, `closepre.bmp`, `datB1.bmp`, ...) from `GRAFIK` | 110 | 1 |
| 110 | Centres the video control (432×320) and picks the screen size (below); sizes the image to it at 0,0 | 300 | 1 |
| 300, 400 | 300 waits a tick. 400 shows the form, image and video control, then opens and plays `sunflow.avi`. If the open fails it goes to 420 | 410 | 200 |
| 410 | Waits until the video's position reaches its length; a key or a click ends it ([films.md](films.md)) | 420 | 1 |
| 420 | Stops and closes the video and hides the form and image; something happens to the video control (`10e8:2258`) | 422 | 10 |
| 422 | Opens `holiday.mid` from `MUSIK` | 424 | 1 |
| 424 | Plays it | 430 | 1000 |
| 430 | Loads the title picture `tit<size>.BMP` into the image | 440 | 1 |
| 440 | Brings the image to the front | 450 | 3000 |
| 450 | Loads the background `bck<size>.BMP` | 500 | 1 |
| 500 | Stops the timer, disables the form (`tcontrol_set_enabled`), creates `TStartform` (`startform`), sets it up and shows it ([below](#the-start-menu-tstartform)) | | |

State 700 (from the start menu's unreachable `MaperstButtonClick`) stops
the timer, generates an island (1030:390e) and shows the menu again. States
1000–1500 start a game; see [below](#starting-a-game-states-10001500).

### Screen size

State 110 snaps the form's size to one of five layouts, with ±20 pixels of
slack. Any other size is used as it is, and then states 430 and 450 load
no picture.

| Form size | Layout | Pictures |
|---|---|---|
| 780–820 × 580–620 | 800×600 | `tit80x60.BMP`, `bck80x60.BMP` |
| 620–660 × 460–500 | 640×480 | `tit64x48.BMP`, `bck64x48.BMP` |
| 1004–1044 × 748–788 | 1024×768 | `tit10x76.BMP`, `bck10x76.BMP` |
| 1132–1172 × 850–890 | 1152×870 | `tit11x87.BMP`, `bck11x87.BMP` |
| 1260–1300 × 1004–1044 | 1280×1024 | `tit12x10.BMP`, `bck12x10.BMP` |

The result is in `screen_width`/`screen_height` (`1110:54fc`/`54fe`).

## Starting a game: states 1000–1500

The start menu's Start sets `Timer1.Tag` to 1000 with `+0x191` set on the
scenario page and `+0x190` cleared. `+0x190` is the other way in: it loads
the saved game named at `1110:1322` (`Last.map` unless something put
another name there) from `MAPS\`, which the load form sets
([savegame.md](savegame.md#loading-from-the-start-menu-tladenform),
`hintergrund::load_saved_game`). Ported in
[`game::hintergrund`](../crates/game/src/hintergrund.rs): a scenario's
world from its map file (`load_world`) and a normal game's from the
generator and the game's data (`new_world`,
[island-generator.md](island-generator.md)); the `holiday` binary plays
`titel.avi` ([films.md](films.md)) and skips the progress window.

| Tag | Does | Next | Interval (ms) |
|---|---|---|---|
| 1000 | Frees the menu's bitmaps (1058:3b61) and the menu. For a scenario, keeps the menu's name (in `1110:6356`), colours (`1110:634e`) and difficulty (`1110:1330`), which the scenario's file would overwrite | 1111 | 1 |
| 1111 | Enables the form, its image and the video panel, shows `dummyform`, opens `titel.avi` from the film directory and plays it; if the open failed, goes to 1120 | 1113 | 200 |
| 1113 | Waits until the video's position reaches its length | 1120 | 1 |
| 1120 | Stops and closes the video, hides the panel, disables the form and image, reactivates WaveMix and reloads the font (`text_done`, `text_init`) | 1200 | 10 |
| 1130, 1140 | Reload the background `bck<size>.BMP` and show it. Nothing in the start path goes there | 1200 | 10 |
| 1200 | Creates `TWasNunForm`, the progress window (1040:33e9), shows it and disables the form | 1210 | 10 |
| 1210 | With the timer off: generates a new island from the start menu's settings (1030:390e, hourglass cursor around 10b0:52d4, [island-generator.md](island-generator.md)); for a scenario or a saved game it only clears the map | 1220 | 10 |
| 1220 | With the timer off: loads the data tables (1030:2e48: `HAUSPOS.DAT`, `HAUSNR.DAT`, the other `*POS.DAT`, prices, names...), the bitmaps (1030:307a: the frame's and the island's, see [map.md](map.md)) and the rest (1030:36e5: months, events, sabotage, the forms' data...), moving the progress bar to 100 | 1500 | 2 |
| 1500 | Sets up the main screen (`hauptschirm_init`, 1020:2501) and stops the timer. With `+0x190`: `load_map` of the saved game, then `game_start_setup` (1010:2820). With `+0x191`: `load_map` of the scenario's `SZENARIO\*.his`, puts the kept name, colours and difficulty back, sets the starting money from the difficulty, the scenario's number of computer players and its events (`EREIGNn.DAT`, 1010:36f8), then `game_start_setup`. Finally closes the progress window and shows the main screen (1020:283b: starts its `AnimationsTimer`, height 479) | | |

So a normal game plays on the island state 1210 generated, and a scenario
or a saved game replaces it with the file's. `game_start_setup` turns the
map file's view position into the main screen's scroll position
(1020:074d), draws the scroll bars and the button bar, and draws the island
(1050:2700).

| Scenario | File | Computer players | Events |
|---|---|---|---|
| 1 | `Geld.his` | 1 | `ereign1.dat` |
| 2 | `Urlauber.his` | 3 | `ereign2.dat` |
| 3 | `Koenig.his` | 7 | `ereign3.dat` |
| 4 | `Luxus.his` | 2 | `ereign4.dat` |
| 5 | `Surfer.his` | 3 | `ereign5.dat` |
| 6 | `Oeko.his` | 0 | `ereign6.dat` |

A normal game's world is ported: the island (10b0:52d4) and what the
game's data sets up around it in states 100 and 1220
([island-generator.md](island-generator.md#the-rest-of-a-new-game)). Not
ported yet: the progress window, a scenario's events
(`EREIGNn.DAT`, 1010:36f8) and players' flags, and what
`game_start_setup` does to the view. A scenario's start money is ported
([simulation.md](simulation.md#money-107801e7)).

## Back to the start menu (1010:25dc)

The other place that creates `TStartform`, from the file menu's "Neues
Spiel". It saves the game to `Maps\Last.map`, hides the forms in
`1110:646c` except 10 and 11, frees the game (1030:3974), reloads the data
(1030:38a3), restarts `holiday.mid`, and shows a new `TStartform`, which
`startform_init` sets from the globals the game left (`Settings::from_world`).
The `holiday` binary saves `Last.map`, opens the sounds again, plays
`holiday.mid` and shows a new start menu; the rest of the reload isn't
ported. The intro's music is in [sound.md](sound.md#the-music).

## The start menu: `TStartform`

Ported as [`game::startform`](../crates/game/src/startform.rs); the
`holiday` binary shows it over the background.

State 500 does `startform := TStartform.Create(Application)`, then
`startform_init` (1058:27e6), activates WaveMix (`10a0:254f`) and calls
`startform_show` (1058:1a30).

- `startform_init` loads 26 bitmaps from `GRAFIK` (`start1.bmp`, the scroll
  bar's six, the button's two, the check box's two and the eight colours
  with their selected versions), builds the widgets, and sets the form and
  its one image, `Bild`, to 500×334 (the DFM says 424×352). It centres the
  form on the screen, reads the texts (`startform_load_texts`, 1058:1ed0:
  `START.TXT` and `SZENARIO.TXT`, line by line) and sets the widgets from
  the globals below.
- `startform_show` centres the form again, draws `start1.bmp` into `Bild`,
  shows the normal page's widgets, refreshes and shows the form.
- `Bild`'s `OnMouseDown` (only with the left button), `OnMouseMove` (only
  with exactly the left button) and `OnMouseUp` (any button) offer the event
  to the widgets in a fixed order until one takes it; the name field comes
  last, so clicking another widget doesn't unfocus it. `FormKeyDown` quits
  on Alt+F4 and passes every other key to the name field.

The menu has two pages. The normal page sets up a random island; the mode
button swaps the background for `startsze.bmp` and the island settings for
the six scenarios. What's on both pages:

| Widget | Where | Does |
|---|---|---|
| Close button (`close.bmp`, the background form's) | 0,0 | Hides the form, `game_done`, `Application.Terminate` (1058:0cbb) |
| Name (edit, 12 characters) | 74,174 | Starts as `last_player_name` (`1110:0a02`, "Holiday") |
| Difficulty (scroll bar 0–2) | 322,194 | `difficulty` (`1110:008c`) |
| Eight colour boxes (radio buttons) | 74,194 | The player takes the colour from whoever had it (`take_colour`, `player_colours`, `1110:0074`). Yellow is ticked at the start whatever the player's colour is |
| "Szenarien" / "Standardspiel" | 18,306 | Switches pages (1058:02cf), and `scenario` (`1110:006e`) to 1 or 0 |
| "Altes Spiel fortsetzen" | 178,306 | Disables the form and opens `TLadenForm` (1058:0c7d) |
| "Start" | 338,306 | Copies the name into `player_names[1]` and `last_player_name`, hides the form and sets `hintergrund`'s `Timer1.Tag` to 1000 with `+0x191` set on the scenario page (1058:021a) |

The normal page:

| Widget | Global |
|---|---|
| Opponents (scroll bar 0–7, with the number beside it) | `computer_opponents` (`1110:1478`), and `player_count` one more |
| Relief, flat to hilly (0–4) | `relief` (`1110:008e`) |
| Land, little to much (0–4) | `land_amount` (`1110:0090`) |
| Rivers, none to many (0–10) | `rivers` (`1110:0092`) |
| Atoll, Inselgruppe, Eine Insel (check boxes used as radio buttons) | `island_shape` (`1110:008a`): 0, 1, 2 |

The scenario page has six check boxes used as radio buttons, which set
`scenario` and put the scenario's description under them.

The scroll bars' `Click`s store their positions whenever they repaint. The
defaults come from `init_game_defaults` (1000:1ea0) and the initialised
data: a group of islands, relief 2, land 2, rivers 5, easy, seven
opponents.

Quirks:

- The check boxes of the island shapes and the scenarios are pressed like
  radio buttons, so a click only ticks them.
- Switching pages sets `scenario` to 1 (or 0), but the boxes and the
  description stay as they were: after going back and forth, a different
  scenario can be ticked than the one that starts.
- Letting go of the right mouse button also releases, and clicks, a button
  held down with the left.
- Never shown: the credits as labels ("Moritz Baer", ...; the picture has
  them), an empty label and a "Version: Deutsch" button with an empty
  `Click`. The published `MaperstButtonClick` (hide the form and go to
  state 700) and `SprachButtonClick` (empty) have no component in the DFM.

"Altes Spiel fortsetzen" opens `TLadenForm`
([savegame.md](savegame.md#loading-from-the-start-menu-tladenform)). The
widgets' sounds play ([sound.md](sound.md)). Start leads to [states 1000–1500](#starting-a-game-states-10001500).
