# Language

Two releases are known: the German v1.0 (`orig/`) and the English UK
release, the later build (`orig-en/`, [versions.md](versions.md)). The
port plays either folder and shows German or English, from its own
language data ([lang/](../lang/README.md)): by default the folder's
language, or the one the player picks.

## How the original picks its language

It has no language setting. Nearly all of its text is the folder's:
`TEXTE\*.TXT` (and `UPGRADE\`'s), the speech (`SPRADEUn\` or
`Sprache<n>\`), the films' sound tracks, the currency's name and factor
(`SONSTIGE.TXT`: the game counts in its own units, shown times
`Waehrungsfaktor`, 10 for `DM`, 4 for `£`) and the
layouts of the labels for the longer English words (`BAUMENU.DAT`,
`GELDMENU.DAT`, `INFOBOX.DAT`). The rest decides in one of three ways.

### `land`

`MODATYP.DAT`'s second line is the country, `D` in the German folder
and `E` in the English one. `load_modatyp` (en 1000:23ed, de 1010:000e)
turns it into `land` (en `1118:66e2`, de `1110:64be`): `A` 1, `E` 2,
`F` 3, anything else 0. It's read once at the start and isn't in a
saved game (the map file's copy of `MODATYP.DAT` starts at the byte
after it, `1110:64bf`), so a German save loaded with English data
formats its dates and money the English way (its players' names and
news stay as they were saved). Every reader of it in the later build:

| en | de | What `land` picks |
|---|---|---|
| en 1030:12b0 `date_text` | 1030:0148 | The day's suffix: `A`/`E` `st`, `nd`, `rd`, `th` (CS:12a0–12a9), `D` `.` (CS:12ac), `F` none: `6. Januar 1996`, `6th January 1996` |
| en 1030:2145 `group_digits` | 1030:0fdd | The thousands mark: `D` `.`, `F` `_`, `A`/`E` `,`: `20.000.000 DM`, `8,000,000 £` |
| en 1030:21f3 `insert_decimal_marks` | 1030:108b | The decimal and thousands marks of a price with pence: `D` `,` and `.`, `F` `.` and `_`, `A`/`E` `.` and `,` |
| en 1040:0eef `dateimenue_save` | (1040:0e03: German only) | The invalid name's message: `Dateiname ungültig` (en 1040:0eb0), `Non valable nom de fichier` (en 1040:0ec3), `Invalid Filename` (en 1040:0ede) |
| en 1040:110c `dateimenue_delete` | (new) | The question before "Löschen": `Wirklich loeschen?` (en 1040:10d7), `Vous êtes sûr?` (en 1040:10ea), `Are you sure?` (en 1040:10f9) |

`A` and `E` always go the same way. The German v1.0 has the first
three; the two file menu messages are the later build's. The French
texts suggest a French release was planned or made; none is known.

### Compiled into the build

`init_paths` (en 1000:196d) has the two messages that end the game
before it starts in German, French and English, and shows the English:
`Please start first SETUP.EXE on the HOLIDAY ISLAND CD.` (en 1000:1649)
and `Please put the HOLIDAY ISLAND CD in the CD-Rom Drive and start
HOLIDAY ISLAND again.` (en 1000:1918). `back_to_start_menu`
(en 1010:3582) asks for the CD in English (en 1010:3540; the German one,
en 1010:3502, is unused). v1.0 has only the German ones (1000:15b0,
1000:175b). The port has both as `formats::setup::MISSING_SETUP`,
`MISSING_CD`, `INSERT_CD` and their `_EN`, but shows none of them: the
port never checks for the CD or `SETUP.DAT`, so they aren't in the
language data either.

The message boxes' own captions and buttons (`Confirm`, `&Cancel`, ...)
are the VCL's string resources, in the build's language: German in
v1.0's `HOLIDAY.EXE`, English in the later build's. The port has both
(`[msgdlg]` in `strings.toml`, `game::msgdlg`) and shows the view's.

The DFMs differ too: v1.0's forms are captioned with their names
(`Baumenue`, `Hauptschirm`, ...), the later build's all `Holiday
Island`. The forms have no Windows title bar in the game, so nothing
shows it; the port reads the DFMs from the folder's `HOLIDAY.EXE`
anyway.

The start menu has a published `SprachButtonClick` (en 1058:2791,
de 1058:1a29) that does nothing and has no component: a language button
that was never made.

Left in German in both builds, and never on the screen: `kein Datum`
(en 1048:1fc5, an empty news archive entry's date, which the TV never
browses to), `kein` (en 1048:0a45, a message without a medium),
`Fehler!!` (en 1040:1330, a saved game list's name past its end; not
ported), the computer players' status texts (`game::computer`). The
port's English `strings.toml` keeps them German, as released. The
music's titles in the options (en 1060:14a1–14e2) are names, the same in
both builds, and stay in the code (`optionen::TRACK_NAMES`), like the
credits' names.

### Windows

The high score table's date is Delphi's `DateToStr` (1008:0512): the
short date format of the Windows the game runs on, `dd.MM.yy` on a
German one, `dd/MM/yy` on a British one.

### German in the English release

On the screens checked (below) nothing. In its data, the English
`NACHRICH.TXT` keeps the German placeholders of messages 34 to 36
(`#Die Straße geht kaputt.*`, `#Der Sabotagebeauftragte haut mit dem
Geld ab*`), as the German file has them: their kind is 255 (no TV) and
no ported code posts them, but the TV would show one if it were in the
archive. `SONSTIGE.TXT` has a German comment line the loader skips, and
`MODATYP.DAT`'s labels are German.

## The port

The port carries both languages itself, in the repository's `lang/`
folder ([lang/README.md](../lang/README.md)), compiled into the build
(`game::lang`). Each language has every `TEXTE\` text file and
`UPGRADE\`'s texts as the original has them (UTF-8 in the repository,
Windows-1252 with CRLF for the loaders, `Language::file`), the label
layouts the English release changed for its longer words
(`DATA\BAUMENU.DAT`, `GELDMENU.DAT`, `INFOBOX.DAT`), and `strings.toml`
with the EXE's strings, the formats and the port's own texts. German is
the v1.0 CD's, English the English release's; the expanded edition's
texts exist only in English, so `lang/de/UPGRADE/` is the port's
translation, marked as such in each file's header.

Embedded rather than read from a folder next to the binary: a build is
one file that needs nothing but the game folder, the web build gets the
text without a file system, and the tests run without `orig/`.

```text
language=auto      # settings.ini: the folder's (the default)
language=en        # settings.ini: English whatever the folder
--lang=auto|de|en  # this run, over settings.ini
```

It's one player's view setting, `enhance::Config::language` (with the
enhancements, as the player's settings travel together; `--original`
doesn't touch it). `auto` takes the language of the folder's `land`
(`MODATYP.DAT`, the one thing the port still reads for it): German for
`D`, English for `A`, `E` and `F` (the port has no French).

The language decides every text of the player's view:

- every text the game reads from `TEXTE\` and `UPGRADE\` (with the
  folder's `UPGRADE\`, the language's `UPGRADE\` texts; without it, its
  `TEXTE\` ones, so the German CD still plays as v1.0);
- the currency's name and factor (`SONSTIGE.TXT`: `DM` times 10, `£`
  times 4), so an English game counts in pounds on the German folder;
- the formats (`[format]`): the date's pattern and day suffixes, the
  thousands and decimal marks (`group_digits`, `decimal_money`), the
  high score table's date. They replace the original's `land` switches;
  the view's `InfoTexts::language` carries the language to every form
  that formats money or dates;
- the label layouts of the build menu, the money menu and the info box;
- the EXE's strings: the message boxes (`[msgdlg]`), the file menu's
  messages (`[file_menu]`; the later build's French ones for `F` aren't
  in the port);
- the port's own texts (`[port]`, `lang::Text`) and the enhancements'
  labels and descriptions (`[enhance.<key>]`). The command line
  (`--help`, `--enhancements`) and the log are English only.

What stays the folder's: the speech (whichever of `SPRADEUn\` and
`Sprache<n>\` is there, so a forced language that isn't the folder's
plays the folder's speech), the films with their sound, the pictures
(a few have words drawn in: the credits, some buttons), the data tables
of `DATA\` and the scenarios' maps (with their computers' names). On the
German folder `--lang=en` shows `6th January 1996` and `8,000,000 £`,
and on the English folder `--lang=de` `6. Januar 1996` and
`20.000.000 DM`, with every form's text in that language.

### The world's text

The world doesn't read the player's setting. It has a language of its
own, `World::language`: the language a game was created in (the
creating player's), which every peer and every reload shares. It's in
the world's hash and saved in the port's chunk after the map file's
fields (`language=de`, `lang::language_in`). A file without it (an
original's, an older port's) takes the game folder's language
(`World::folder_language`, from `MODATYP.DAT`'s `land`, which loading a
file keeps). A scenario is a new game: its world is in the player's
language too. The world makes in it:

- the players' names (`World::player_names`): the language's
  `NAMEN.TXT`, its name for nobody and the computers' names picked from
  it with the world's random numbers (`hintergrund::pick_names`,
  en 1000:27f3, `GameData::load`); saved in the map file. A scenario's
  names are its map file's;
- the news archive's dates (`news::Entry::date`): `post_message` writes
  each entry's date as text with the language's months and `[format]`,
  as the original does; saved in the map file as written. The TV reads
  the date back from the text (`monate::Date::parse`, the world's
  language first, then the others) and writes it in its player's
  language (`news::Entry::date_in`): `6. Januar 1996` in a German world
  shows as `6th January 1996` to an English player. A text no language
  writes for a date (a file's own) shows as it is;
- `kein Datum` for an empty entry (`news.no_date`, which the TV shows in
  its player's language too) and the computers' status texts
  (`[computer]`), the same in both languages;
- the sabotage's costs and the piranhas' pools from `SABOTAGE.TXT`
  (`sabotage::table` of the world's language, not the folder's or the
  player's). The two releases' files differ in one number the game never
  reads (the shark diver's target, 254 in German, 255 in English), so
  the world plays the same in either language (`lang_data` checks it),
  and German and English players share a network game.

The view's formatting (the info bar's date and money, the forms, the
TV's dates and money) follows each player's language. In multiplayer each player
can have their own, and the world's text comes with the world.

### Screens

`language_screens` (crates/holiday) renders the start menu's scenario
page, the main screen with the TV's message, the money menu and the
overview map, the info bar alone, an auction, the options, the list of
enhancements and a message box, on each folder in its own language and
in the other forced; with `HOLIDAY_PNG` set they go there as
`langfiles-<folder>[-lang-<forced>]-<screen>.png`. On the German folder
forced to English nothing German shows, and on the English folder
forced to German nothing English, the expanded edition's scenarios in
the port's German included.
