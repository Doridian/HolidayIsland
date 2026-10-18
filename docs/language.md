# Language

Two releases are known: the German v1.0 (`orig/`) and the English UK
release, the later build (`orig-en/`, [versions.md](versions.md)). The
port plays either folder and shows German or English: by default the
folder's language, or the one the player picks.

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
`MISSING_CD`, `INSERT_CD` and their `_EN`; it shows none of them yet
(it doesn't check the CD).

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
`Fehler!!` (en 1040:1330, a saved game list's name past its end), the
computer players' status texts (`game::computer`). The music's titles in
the options (en 1060:14a1–14e2) are names.

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

`game::lang` has the setting and the port's own texts.

```text
language=auto      # settings.ini: the folder's (the default)
language=en        # settings.ini: English whatever the folder
--lang=auto|de|en  # this run, over settings.ini
```

It's one player's view setting, `enhance::Config::language` (with the
enhancements, as the player's settings travel together; `--original`
doesn't touch it). `auto` takes the language of the folder's `land`:
German for `D`, English for `A`, `E` and `F` (the port has no French
texts; `F` keeps its own marks).

What it switches, all of it the view's:

- The view's `land` (`hauptschirm::InfoTexts::land`,
  `lang::Setting::land`): the folder's own with `auto` or when the
  forced language is the folder's, else `D` for German and `E` for
  English. Everything the original formats by `land` reads it: the info
  bar's date and money, the money menu, the info box, the buildings'
  overview, the build menu's prices, the auction, the sabotage window,
  the end form's results and the high score's money text.
- The later build's messages by `land`
  (`dateimenue::invalid_name_text`, `dateimenue::delete_question`);
  the message boxes aren't ported, the invalid name's text goes to the
  log.
- The high score's date: `dd.MM.yy` in German, `dd/MM/yy` in English
  (`highscoreform::date_text`).
- The port's own texts: the list of enhancements (`lang::Text`) and the
  enhancements' labels and descriptions, each in German and English in
  `game::enhance`'s list. The command line (`--help`, `--enhancements`)
  and the log are English only.

What stays the folder's: every text from `TEXTE\` and `UPGRADE\`, the
speech, the films, the currency's name and factor, the forms' layouts
and bitmaps. So a forced language that isn't the folder's mixes: on the
German folder `--lang=en` shows `6th Januar 1996`, `20,000,000 DM` and
an English list of enhancements among German forms, and on the English
folder `--lang=de` shows `6. January 1996` and `8.000.000 £`. That's
accepted: a language's texts come with its folder.

### The world's text

The world never reads the setting. What of it is text is made from the
folder's data, the same way for every player:

- the players' names (`World::player_names`): `NAMEN.TXT`'s name for
  nobody and the computers' names picked from it with the world's
  random numbers (`hintergrund::pick_names`, en 1000:27f3); saved in the
  map file;
- the news archive's dates (`news::Entry::date`): `post_message` writes
  each entry's date as text with the folder's `MONATE.TXT` and `land`
  (`news::DateFormat`, which the world keeps, `World::news`); saved in
  the map file, and shown by the TV as written. So the TV's dates keep
  the folder's format when another language is forced, unlike the info
  bar's;
- `kein Datum` for an empty entry and the computers' status texts,
  which are the EXE's and the same in both builds.

In multiplayer each player can have their own language. The folder's
data must be the same for every peer anyway (the world's tables come
from it), and with it the names and the archive's date format; a peer
with a folder of another language would get another world, so the
world's date format and names have to come with the world from the
host, not from each peer's folder.

### Screens

`language_screens` (crates/holiday) renders the main screen with the
TV, the money menu and the overview map, the info bar alone, an auction,
the options and the list of enhancements, on each folder in its own
language and in the other forced; with `HOLIDAY_PNG` set they go there
as `lang-<folder>[-lang-<forced>]-<screen>.png`. On the English folder
nothing German shows; on the German folder nothing English.
