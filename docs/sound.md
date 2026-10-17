# Sound and music

The effects are WAVs mixed by `WAVEMIX.DLL`; the music is MIDI from
`MUSIK\` through `TMediaPlayer` (MCI's `sequencer`); the TV's speaker is a
WAV from `SPRADEUn\` through another `TMediaPlayer` (MCI's `waveaudio`).
All three are the view's: the world only says which sound number a tick
makes (`Tick::sounds`) and hears of the speaker through a command.

The films' sound is in [films.md](films.md). Ported in
[`game::sound`](../crates/game/src/sound.rs) (which wave
`play_sound` plays and when, the music's player and playlist, the MCI
players' positions), [`game::fernseher`](../crates/game/src/fernseher.rs)
(the speaker), [`platform::audio`](../crates/platform/src/audio/mod.rs)
(the WaveMix model, the renderer, the `cpal` output, the SoundFont lookup)
and [`holiday::sound`](../crates/holiday/src/sound.rs) (the wiring).

## The options

| Global | What | Where |
|---|---|---|
| `1110:0062` | The installation has sound: `SETUP.DAT`'s fifth line isn't `N` (`init_paths`, 1000:17bc). Without it nothing of WaveMix runs and the options show the sound volume as 0 | `Effects::enabled`; the binary reads `SETUP.DAT` if the game directory has one, else `--sound=hi\|lo\|off` (default `hi`) |
| `1110:51e0` | `Sound\` or, for `L`, `SoundLo\` (22 kHz or 11 kHz files; WaveMix mixes both at 11 kHz) | `holiday::sound::Sound::new` |
| `1110:0061` | "Geräusche": `play_sound` plays | `Settings::sounds` |
| `1110:005b` | "Musik": the background music plays. `music_start`/`music_stop` set it; the end of a game clears it | `Settings::music` |
| `1110:6ef2..6ef8`, `6efa` | The playlist and the track | `Settings::playlist`, `track` |
| `1110:0066` | "Sprachausgabe": the TV reads the news | `Settings::speech` |
| `1110:6330`, `6334`, `6338` | Music, sound and speech volume, 0–100, set at start from `waveOutGetVolume / 655` (the port: 100). `midiOutSetVolume` / `waveOutSetVolume` get `volume * 655` for both channels (1018:35f0, 10a0:2453, 10a0:24bc) | `game::sound::device_level` |

A scenario's start (state 1500) turns the music and the speech on.
`game_start_setup` (1010:2820, scenarios and saved games) sets both
volumes and stops and closes the music if it's off.

## The effects: `play_sound` (10a0:30b6)

`init_sound` (10a0:2f5e, from 1030:38a3 at start and again for "Neues
Spiel"): `WaveMixConfigureInit` with one output channel and the INI's
rate (11.025 kHz), `WaveMixOpenWave` of the 42 files (`sound_file_name`,
10a0:2824, into `1110:728e`), `WaveMixOpenChannel(0, WMIX_OPENALL)` (all
eight), and the session activated (`sound_activate`, 10a0:254f).

`play_sound(n)`, with sound and "Geräusche" on: 0x2b plays 3 and 0x2c
plays 2; unless `1110:723c[n]` is above 0, `WaveMixPlay` of wave `n` on
channel 0 with `WMIX_USELRUCHANNEL`, no loops, and `1110:723c[n]` :=
`1110:3dce[n]`. `people_tick` (1090:5293) counts each down by one at its
start; it runs only with the "animations" option on (with
`enhance.sound_times` the port counts down every tick).

| n | File | Ticks | Played by |
|---|---|---|---|
| 1 | `waal.wav` | 50 | |
| 2 | `chikli8b.wav` | 50 | a waiting walker (also 0x2c) |
| 3 | `hupe.wav` | 50 | a waiting walker (also 0x2b) |
| 4 | `hupe3.wav` | 50 | a waiting walker |
| 5 | `abris18b.wav` | 0 | |
| 6 | `elektriz.wav` | 50 | |
| 7 | `loewe8b.wav` | 50 | the zoo's animal (kind 0x1d), its first frame |
| 8 | `butthi8b.wav` | 0 | a button or arrow goes down |
| 9 | `buttmi8b.wav` | 0 | a box ticked, a row picked, a key on a form |
| 10 | `buttlo8b.wav` | 0 | a button or arrow comes up |
| 11 | `clicla8b.wav` | 0 | |
| 12 | `duck8b.wav` | 0 | an error on the status bar (1030:232d) |
| 13 | `auf8b.wav` | 0 | a form opens |
| 14 | `zu8b.wav` | 0 | a form closes |
| 15 | `bike38b.wav` | 50 | a waiting walker |
| 16 | `brandung.wav` | 50 | the beach (one time in 500) |
| 17 | `strasb8b.wav` | 0 | a road built |
| 18 | `brems8b.wav` | 50 | |
| 19 | `hammer.wav` | 25 | builders |
| 20 | `aukha8b.wav` | 0 | the auctioneer's hammer |
| 21–23 | `crashku1.wav`, `exploku1.wav`, `exploku2.wav` | 0 | |
| 24 | `fliegst2.wav` | 50 | |
| 25 | `elefant.wav` | 50 | the people (kind 0x1c) |
| 26–28 | `zum1.wav`..`zum3.wav` | 0 | the auctioneer's three calls: not in `SOUND\` (they're in `SPRADEUn\`), so never heard; with `enhance.auction_calls` from `SPRADEU1\` |
| 29 | `hublop8b.wav` | 50 | |
| 30 | `klaerku.wav` | 50 | the people (kind 0x1b) |
| 31 | `klonga8b.wav` | 0 | a tool did its work (build, raise, lower) |
| 32 | `kranku.wav` | 25 | builders |
| 33 | `kreisse.wav` | 50 | builders |
| 34 | `moew2.wav` | 50 | the beach (one time in 500) |
| 35 | `nebel48b.wav` | 0 | |
| 36 | `newskurz.wav` | 0 | the TV opens for a message |
| 37 | `payedit2.wav` | 50 | the balance sheet opens |
| 38 | `pressl.wav` | 25 | builders |
| 39 | `saege.wav` | 25 | builders |
| 40 | `putt.wav` | 25 | the minigolfers' putt |
| 41 | `hole-in.wav` | 25 | |
| 42 | `disco.wav` | 25 | the people (kind 0x3b) |

"The people" are `add_person`'s and `spawn_people`'s sounds, only where
the player sees them ([people.md](people.md)); empty rows are numbers the
ported code doesn't use yet or that come from people's tables. The world
pushes the numbers into `Tick::sounds`, `Tick::sound_countdown` says where
in them `people_tick` counted down, and the forms return theirs in their
responses; the view plays them in order.

`sound_deactivate` (10a0:2518: `WaveMixFlushChannel(0, WMIX_ALL)`,
`WaveMixActivate(FALSE)`) gives the wave device to a film or the speaker;
`sound_activate` (10a0:254f) takes it back. Callers: the intro's film
(1000:2466: off in state 100, on in 500) and `titel.avi` (off in 1111, on
in 1120), the end's films (1068:12b7, 1068:0ec9,
back at 1068:1869), the TV's film (1048:01bd) and speaker (1048:1b3e,
back in 1048:034f, 077a, 15e6, 1939, 26b7). While the session is inactive
`WaveMixPlay` fails, silently; `play_sound`'s times still start.

### WAVEMIX.DLL

Read from the DLL in Ghidra (its own segment `1000`), modelled in
[`platform::audio::wavemix`](../crates/platform/src/audio/wavemix.rs) and
tested without a device:

- `WaveMixOpenWave` (1000:227e) converts a file to the session's format
  (1000:21a0): down by the whole ratio of the rates, each sample the
  truncated average of its group but the last (1000:1c1c); up by stepping
  `(next - this) / ratio` (1000:1d3e). `SOUND\`'s 22 kHz waves become
  11 kHz ones like `SOUNDLO\`'s.
- `WaveMixPlay` (1000:1042) fails for a null wave (a file that didn't
  open) or an inactive session; `ShowDevices=0` keeps it from showing a
  message box. `WMIX_USELRUCHANNEL` takes the open channel chosen least
  recently (a count of plays; ties to the lowest), playing or not, so the
  eight channels take turns; without `WMIX_CLEARQUEUE` the wave queues
  behind what that channel still plays.
- The mix (`cmixit` 1000:01b2; the 386 version 1000:31b4 does the same):
  one source copied, several summed around 0x80 and clipped to 0–255,
  silence 0x80.

Not modelled: the device's blocks (`WaveBlocks`, `WaveBlockLen`) and
their latency, and the "remix" of blocks already written: a wave starts at
the next sample mixed. The port's output adds its own latency.

## The music

`hintergrund.MusikMediaPlayer` (+0x188). MCI's sequencer is modelled by
`game::sound::Mci`: a position in ms that runs with the clock while it
plays, up to the file's length (from `rustysynth`'s parse); that's all
the game reads of it.

- The intro (states 422, 424) and "Neues Spiel" (1010:25dc) open and play
  `holiday.mid`, whatever the options say, once.
- Each game's set-up: `music_init_playlist` (1018:37de): every track on
  the list, track 0, `1110:6efc` := 0x3039. So the first track after the
  intro's is track 1; with `enhance.music` track 0.
- After each day (1020:2285): `music_next_track` (1018:367e): with the
  music on, if the position equals the one after the last day, the next
  ticked track after the current one (wrapping, the same one if none is
  ticked) is stopped, ejected, closed, opened and played; then the
  position is kept. So a track's end shows a day later, and the paused
  game never changes track. With `enhance.music` the next track starts
  as soon as one ends (each frame, paused or not).
- The options: "Musik" `music_start` (1018:3850: the track opens and
  plays, 0x3039) or `music_stop` (1018:38c5: stop, close); a track's radio
  button (1060:02b7): with the music on stop, eject and close, the track,
  and with the music on open and play. Tracks (`1110:31d6`):
  `anotrian.mid`, `dub.mid`, `floatin'.mid`, `samba.mid`, `ukanchi.mid`,
  `cuban.mid`, `holiday.mid`.
- A game loaded from the file menu (`game_show_state`, 1010:26fa): with the
  music off it closes, else its file opens again, without playing (with
  `enhance.music` it plays).
- The end of a game (`TSchlussform.MusikPlayer`, +0x18c,
  [endforms.md](endforms.md)): the background music stops (and the option
  goes off), `gewonn.mid` or `pleite.mid` plays, then `endeloop.mid` each
  time it has ended (`Timer1`); "Weiterspielen" starts the background music
  again if it played.

### Getting a SoundFont

MIDI is rendered with `rustysynth` from a SoundFont (`.sf2`), which the
game doesn't ship and the repository doesn't hold. The music is plain
General MIDI (channels 1–12, drums on 10, no GS/XG SysEx), written on a
Roland GS Sound Canvas: `dub.mid` picks drum kit 25 (the TR-808) and
`holiday.mid` kit 1. `rustysynth` takes a program change on channel 10 as
a kit (bank 128; a kit the font lacks falls back to 128:0), as GS does, so
a GS SoundFont comes closest.

`tools/get-soundfont.sh` downloads **GeneralUser GS** v2.0.3 by
S. Christian Collins (GS-compatible, 13 drum kits; its licence allows
redistributing it with software) and its `LICENSE.txt` into
`${XDG_DATA_HOME:-~/.local/share}/holiday-island/soundfont/` (macOS:
`~/Library/Application Support/Holiday Island/soundfont/`), checking its
size and SHA-256.

The binary takes the first of:

1. `--soundfont=FILE.sf2`;
2. `HOLIDAY_SOUNDFONT`;
3. any `.sf2` in that `soundfont` directory;
4. the system's: `/usr/share/soundfonts/default.sf2`, `FluidR3_GM.sf2`,
   `/usr/share/sounds/sf2/` (`default-GM`, `FluidR3_GM`, `TimGM6mb`), then
   any `.sf2` in `/usr/share/soundfonts`, `/usr/share/sounds/sf2`,
   `/usr/local/share/soundfonts`, `/usr/share/sf2`,
   `$XDG_DATA_HOME/soundfonts` and `~/Library/Audio/Sounds/Banks`.

Without one it logs one line naming `tools/get-soundfont.sh` and the music
is silent; the players run on, so the tracks change as they would. A
SoundFont of the Sound Canvas (SC-55) the user has works through
`--soundfont`.

## The TV's speaker

`TFernseher.MediaPlayer2` (+0x18c). [fernseher.md](fernseher.md) has the
TV; this is its sound.

- A message that opens the TV (`post_message`, 1048:0a54): with
  "Sprachausgabe" and a `SOUNNAME.DAT` name other than `kein`, the file
  `SPRADEU<1 + text mod 3>\<name>` (the speech of the text variant shown)
  is opened and `tv_busy` (`1110:662d`) := 1; else `tv_busy` := 0,
  leaving a speech still open playing (with `enhance.tv_sound` it's
  closed). Then sound 0x24.
- `fernseher_tick` (1048:1b3e): 3 s after the TV opened, if the page's
  name isn't `kein` (with `enhance.tv_sound` the posted message's): with `tv_busy` the effects go off (10a0:2518), the
  wave volume goes to the speech volume (10a0:24bc) and the speaker plays;
  `1110:667a` := 1. The lights flicker while `tv_busy`, started and
  `Position <> Length`. With `tv_busy` the TV closes (or its film plays)
  when `Position = Length`; without, 6 s after it opened.
- Closing the speaker (stopping the film, hiding the TV, a press on the TV
  or its title bar): `Close`, the effects on (10a0:254f), the sound volume
  back, `tv_busy` := 0. Hiding the TV then turns the effects on again with
  "Geräusche" on (with `enhance.tv_sound` either way, and a news film's
  end too).
- The auction's `Timer2` (1028:1940) doesn't open the form while
  `tv_busy`: it tries again a second later.

The port: the TV asks for these in `fernseher::Response::media`; the
view's `Mci` gives `Position = Length` to `Fernseher::timer`. The world
needs the speaker in two places, so the TV sends
`Action::Speaker(Off | Open | Reading)`, applied at once (it's a global
the original sets in place): the auction waits while it isn't `Off`, and
while it's `Reading` the world draws the lights' `Random(5)`,
`Random(20)`, `Random(3)` in its tick, between the earthquake and the
people, as the original does, and hands them to the view
(`Tick::tv_lights`). So the speech option and how long the speech plays
change the game's random numbers; with `fix.tv_lights` the world draws
nothing and the view's TV blinks with its own. A speech file that doesn't open (the original raises
an exception in `post_message`) leaves the TV without a speaker.

## The output

`platform::audio::Engine` renders, without a device: the WaveMix mix
(11.025 kHz, 8-bit, linearly interpolated to the device rate) and the
speaker (at its file's rate) at the wave volume, plus the MIDI at the
music volume. `Audio::open` puts it behind a `cpal` stream on the default
device (feature `audio-device`, on by default; it needs ALSA's headers on
Linux to build). Without a device, or with `--no-audio`, `Audio` is
silent and does nothing; the players' positions still run, so the game
behaves the same.

## Not done

- The films' timing as MCI's: see [films.md](films.md#not-done).
- The device latency of WaveMix's blocks.
- `WAVEMIX.INI`'s other rates (22 and 44 kHz for some devices): the port
  mixes at 11.025 kHz, the default and the INI's `[default]`.
- The system's volumes at start (the port starts at 100) and putting the
  system's wave volume back at the end (`sound_done`, 10a0:2577).
- `WaveMixActivate` on losing the focus: the game doesn't call it either.
- An authentic FM mode: a typical 1996 home PC played the music on its
  sound card's OPL3 (Sound Blaster 16) rather than a Sound Canvas. An
  OPL3 emulation with the Windows FM patches would be a later option.
