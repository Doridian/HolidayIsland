# The films

The cutscenes are AVIs in `FILMHI\` or `FILMLO\`, played by Delphi's
`TMediaPlayer` through MCI's `avivideo` driver (Video for Windows 1.1e,
`VFW1.1E\` on the CD) into a `TPanel` (`Display`). Ported in
[`formats::avi`](../crates/formats/src/avi.rs) (the demuxer),
[`formats::cinepak`](../crates/formats/src/cinepak.rs) and
[`formats::msadpcm`](../crates/formats/src/msadpcm.rs) (the decoders),
[`holiday::film`](../crates/holiday/src/film.rs) (the player),
`platform::Overlay` and `platform::audio::Engine::play_film` (the
picture and the sound), with the games' parts in `game::fernseher`,
`game::schlussform` and `holiday`'s `main.rs` and `forms.rs`.

## Which directory

`SETUP.DAT`'s fourth line: `L` puts `Filmlo\` into `film_dir`
(`1110:502a`), anything else `Filmhi\` (`init_paths`, 1000:17bc;
[startup.md](startup.md)). The CD check wants 21 of the films there
(`formats::setup::CD_CHECK_FILMS`): not `sunflow.avi` or `titel.avi`.
The port takes `--films=hi|lo`, else `SETUP.DAT`'s line if the game
directory has one, else `FILMHI`.

## The files

Checked for all 46 files (`crates/formats/tests/orig_films.rs`):

- Video: Cinepak (`cvid`), `biBitCount` 24, no palette and no palette
  changes (`##pc`). 432 × 320 for the intro, the start and the end films,
  335 × 257 for the TV's. `FILMHI` at 25 frames a second, `FILMLO` at 12
  (the same films with fewer frames and a lower data rate), `sunflow.avi`
  at 15 in both (the same file). Some frames are empty chunks (dropped
  frames: the picture stays). The chunks are interleaved in `rec ` lists
  with `JUNK` padding, and there is an `idx1`.
- Sound: MS ADPCM (`WAVE_FORMAT_ADPCM`, tag 2), mono, 22050 Hz, 4 bits,
  blocks of 512 bytes (1012 samples), the standard seven coefficient
  pairs, in chunks of about a second. As long as the film, give or take
  a block. `ABking.avi` has no sound stream at all, in both directories.

## Which films play when

| Film | When | Player, panel | Ends |
|---|---|---|---|
| `sunflow.avi` | The intro: `hintergrund_timer` state 400 (1000:2466), after the screen layout (110) | `FilmMediaPlayer` (+0x17c) in `FilmPanel` (+0x18c), 432 × 320, centred (state 110) | State 410 sees `Position = Length` (it runs every 1 ms); a key (`hintergrund_skip_film`, 1000:3938) or a click on the panel or the background image (1000:39b7, 1000:3a36) goes to 420 at once. 420 stops and closes it; then `holiday.mid`, the title and the start menu ([startup.md](startup.md)) |
| `titel.avi` | Every game's start, after the start menu's Start or the load form: states 1111 to 1120 | The same | State 1113, or a key or a click, goes to 1120: stop, close, then the progress window and the game |
| A news film | `fernseher_tick` (1048:1b3e), when the speech is over (or 6 s after a message opened the TV without one), if the page has a film (`FILMNAME.DAT`) and the player hasn't browsed: `fernseher_play_film` (1048:01bd) | `TFernseher.MediaPlayer1` in `Panel1`, put at (9, 0x19), 0x14c × 0x100 | The clock's next step 8 that sees `Position = Length` stops it (1048:034f) and closes the TV (1048:077a), which ends the pause. A press on the TV, its keys (1048:26f6) or closing it stop it early |
| `ABrente`, `ABgeld`, `ABleute`, `ABking`, `ABluxus`, `ABsurfer`, `ABoeko` | A fair win of scenario 0 to 6 (`schlussform_show_end`, 1068:1016) | `TSchlussform.Filmplayer` (+0x184) in `Panel1` (+0x180), (5, 5), 0x1b0 × 0x140 | `Timer1` (1068:187f, once a second) sees `Position = Length`, or a key (1068:198a), a click on the panel (1068:199b) or a key on the background form (1000:3d0c): `schlussform_film_ended` (1068:17ad) shows the results |
| `ABPleite.avi` | A lost game, or a cheated win | The same | The same |
| `ABEnde.avi` | Quitting: the quit question's "OK" (1020:3a1b) or the end form's "Beenden" and close button (1068:0e68) | The same | The same, then the game exits (`Halt`) |

The news films, by message (`FILMNAME.DAT`'s line, from 0): 4 and 32
`haitauch.avi`, 22 `pleite.avi`, 23 and 29 `spreng.avi`, 24 and 30
`saufbier.avi`, 25 and 31 `tuschel.avi`, 27 and 33 `piranhas.avi`, 47
and 62 `stink.avi`, 48 and 63 `ratten.avi`, 49 and 64 `oel.avi`, 87
`surfwm.avi`, 88 `tennis.avi`, 89 `miss.avi`; the others have `kein`.
The options' "INFO" (`TCreditform`, [options.md](options.md)) is a
bitmap, not a film; there is no other film.

## Where they're drawn

`TMediaPlayer.Open` with `Display` set hands MCI the panel's window
(`MCI_WINDOW`); the game never sets `DisplayRect`, so the driver draws
each frame at the film's own size from the panel's top left, not
stretched, cut off at the panel's edges. The panels are the films' sizes,
but the TV's: 332 × 256 for its 335 × 257 films, so their right 3
columns and bottom row are never seen (kept). The TV's panel moves with
the TV and is cut off by it while it's rolled up; the port does the same
(`Game::film_panel`).

On the 256-colour display the game asks for, the driver (`DrawDib`)
reduces the 24-bit frames to the system palette, which on such a display
changes how they look. The port shows them in true colour, as on a
display with more colours: `platform::Frame::overlay`, a second texture
the shader draws over the 8-bit framebuffer.

## Sound

The film's sound goes to the wave device, which under Windows 3.1 one
client has at a time, so each film first has `WaveMix` give it up
(`sound_deactivate`, 10a0:2518) and takes it back after
(`sound_activate`, 10a0:254f): the intro in state 100 and 500 (so the
effects are off from the start to the start menu), `titel.avi` in 1111
and 1120, the end's films in 1068:12b7 / 1068:0ec9 and 1068:1869, the
TV's in 1048:01bd and only when the TV closes (1048:077a, with
"Geräusche" on; kept, [original-bugs.md](original-bugs.md#sound)). The
volume is the wave device's: after the TV's speech that's still the
speech volume until the speaker closes after the film (kept).

The port renders the film's ADPCM, decoded at open, on the wave device's
mix at the wave volume (`Engine::play_film`), interpolated to the
device's rate like the speaker.

## Music and the clock

The MIDI music is on another device and plays on: `holiday.mid` starts
only after the intro's film (states 422, 424) and is still playing (or
not) during `titel.avi`; the background music during the TV's films;
`gewonn.mid` or `pleite.mid` starts with the end's film
([sound.md](sound.md#the-music)).

The world doesn't see the films. The TV pauses the game while it's open
(`Action::ShowTv`, `Action::TvPause(false)` when it closes), so the film
only keeps the pause on longer; the end form stops the clock; the intro
and `titel.avi` come before there's a game. The film's position is the
view's clock (`game::sound::Mci` in ms, `Position = Length` at the
film's length), and its end comes back to the forms as before:
`Fernseher::timer`'s `film_done`, `Schlussform::film_ended`.

## Not done

- The 256-colour look: the frames aren't reduced to a palette.
- MCI's own timing: `avivideo` counts in frames and drops or waits for
  frames as the machine keeps up; the port shows the frame for the
  clock's position and decodes every frame up to it. Picture and sound
  are timed apart (the view's clock, the device), so they can drift by
  the output's latency.
- A film that doesn't open raises an exception in the original; the port
  ends it at once ([original-bugs.md](original-bugs.md#crashes-and-hangs)).
- Seeking, `idx1` and palette changes: nothing uses them.
