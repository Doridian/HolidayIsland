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
`Interval` until the next tick. The other components it uses: a media player
for the videos (`+0x17c`), a full-screen image (`+0x180`), a media player for
the music (`+0x188`) and the control the video plays in (`+0x18c`).

| Tag | Does | Next | Interval (ms) |
|---|---|---|---|
| 100 | Loads nine bitmaps for the file menu's buttons (`close.bmp`, `closepre.bmp`, `datB1.bmp`, ...) from `GRAFIK` | 110 | 1 |
| 110 | Centres the video control (432×320) and picks the screen size (below); sizes the image to it at 0,0 | 300 | 1 |
| 300, 400 | 300 waits a tick. 400 shows the form, image and video control, then opens and plays `sunflow.avi`. If the open fails it goes to 420 | 410 | 200 |
| 410 | Waits until the video's position reaches its length | 420 | 1 |
| 420 | Stops and closes the video and hides the form and image; something happens to the video control (`10e8:2258`) | 422 | 10 |
| 422 | Opens `holiday.mid` from `MUSIK` | 424 | 1 |
| 424 | Plays it | 430 | 1000 |
| 430 | Loads the title picture `tit<size>.BMP` into the image | 440 | 1 |
| 440 | Brings the image to the front | 450 | 3000 |
| 450 | Loads the background `bck<size>.BMP` | 500 | 1 |
| 500 | Stops the timer, creates `TStartform` (`startform`) and shows it | | |

States 700 and 1000–1500 (`titel.avi`, the `TWasNunForm`, loading
`MAPS\` or one of the six `SZENARIO\*.HIS` and `EREIGN1-6.DAT`) start a game
from the start menu; they come with the start menu's port.

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

## Back to the start menu (1010:25dc)

The other place that creates `TStartform`. It saves the game to
`Maps\Last.map`, hides the forms in `1110:646c` except 10 and 11, restarts
`holiday.mid`, and shows a new `TStartform`.
