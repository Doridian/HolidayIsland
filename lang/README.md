# Languages

Every text the game shows, in every language the port has: German
(`de/`) and English (`en/`). The original game takes its text from the
game folder (`TEXTE\`, `UPGRADE\`) and a few strings from `HOLIDAY.EXE`;
the port takes all of it from here, so the language a player picks
decides every text, whichever release their game folder is. Pictures,
films and sounds (the speech included) aren't here: they still come from
the player's game folder. See [docs/language.md](../docs/language.md).

The files are compiled into the game (`include_str!` in
`crates/game/src/lang.rs`), so a build needs no game folder for its
text, and changing a file here needs a rebuild.

## What a language has

```
<code>/
  strings.toml        the EXE's strings, the formats, the port's own texts
  TEXTE/*.TXT         the 23 text files of the CD's TEXTE\
  UPGRADE/*.TXT       the "Erweiterte Version"'s texts: SZENARIO, SCHLUSS,
                      SONSTIGE (24 texts plus the file menu's Löschen) and
                      HIGHSCO2 (the high score names of scenarios 8 to 13)
  DATA/BAUMENU.DAT    the label layouts the English release changed for its
  DATA/GELDMENU.DAT   longer words: positions and alignments of the build
  DATA/INFOBOX.DAT    menu's, the money menu's and the info box's labels
```

- **`TEXTE/` and `UPGRADE/`** are the original's files, one for one: same
  names, same lines in the same order, as UTF-8 with LF line ends. The
  game reads them line by line and by line count, so a line's place is
  its meaning: translate a line in place, never add or remove one, and
  keep blank lines and the `_TXT…_` labels. The game reads them back as
  Windows-1252 with CRLF, so only characters Windows-1252 has can show
  (`ä ö ü ß é £ € „ “ –` are fine; the game's font may still lack some).
  Each line has a length limit of the original's (often a `string[30]`),
  and the screen's room is tighter still: check the screens.
- **The German v1.0 CD's files** are `de/TEXTE/` and `de/DATA/`, and the
  English release's are `en/TEXTE/`, `en/UPGRADE/` and `en/DATA/`, as
  released: typos and all (`Wirklich loeschen?`). The only change is the
  NUL byte some German files end with, which is dropped. A test
  (`crates/game/tests/lang_data.rs`) checks them against `orig/` and
  `orig-en/` byte for byte when those are there.
- **`de/UPGRADE/`** is the port's own: Sunflowers shipped the expanded
  edition only in English. Its files take v1.0's German texts as they are
  and add the new ones in German, with a header that says so.
- Lines at the top of a file that start with `//` are the port's
  comments and aren't part of the file. Nowhere else.
- **`strings.toml`**: `key = "value"` under `[section]` headings, a
  subset of TOML (basic strings with `\"`, `\\`, `\n`, `\uXXXX`; `#`
  comments). Its sections:
  - `[language]`: the language's own name.
  - `[format]`: the date's pattern (`{day}{suffix} {month} {year}`, the
    month names are `TEXTE/MONATE.TXT`'s), the day's suffix (`.` or
    `st|nd|rd|th|…`, one for every day or 31), the thousands and decimal
    marks, and the high score table's date (`dd.MM.yy`). The currency's
    name and factor are `TEXTE/SONSTIGE.TXT`'s, as in the original
    (`DM` times 10, `£` times 4).
  - `[msgdlg]`: the message boxes' captions and buttons, `&` before the
    letter of the key.
  - `[file_menu]`, `[news]`, `[computer]`: the EXE's other strings. Some
    the English release left in German (`kein Datum`, the computers'
    status texts); they never show, and stay as released.
  - `[port]`: the port's own texts (the list of enhancements).
  - `[enhance.<key>]`: each enhancement's `label` (a few words) and
    `description` (one line), by its key in `crates/game/src/enhance.rs`.

Every language has every file and every key. `cargo test -p game`
checks it (`lang::tests::every_language_is_complete`), and that every
file loads with the game's loaders (`lang_data::every_file_loads`).

## Adding a language

1. Copy `en/` to a folder named by the language's code (`fr/`, two
   lower-case letters).
2. Translate every file in place, line for line, and every value of
   `strings.toml` (keep the keys). Put a `//` header at the top of the
   text files saying whose translation it is. Adjust the label layouts in
   `DATA/` if the words don't fit.
3. In `crates/game/src/lang.rs`: add the language to `Language` (its
   variant, `ALL`, `code`), a `static` with its files like `DE` and `EN`,
   and its arm in `Language::data`; say in `Language::of_land` which of
   `MODATYP.DAT`'s countries it is, if any.
4. `cargo test --workspace`, then look at the screens (`HOLIDAY_PNG=dir
   cargo test -p holiday language_screens` writes them).

The world's text (the computers' names from `NAMEN.TXT`, the news
archive's dates) is made in the language a game was created in and saved
with it, so a language added later can't change a saved game.
