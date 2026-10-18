# Game data on the CD

File names are 8.3 upper case on
the CD, but `HOLIDAY.EXE` spells them in mixed case (`Hausdata.dat`, `Grafik\`,
`Filmhi\`), so the port needs case-insensitive file lookup. All text is
Windows-1252, German on the German CD and English in the English
release ([language.md](language.md)).

| Dir | Files | What |
|---|---|---|
| `DATA\` | 39 `*.DAT`, 364 KB | Game tables, all plain text, mostly numbers and names in fixed-width columns (`HAUSDATA.DAT`: buildings, `HAUSPREI.DAT`: prices, `FILENAME.DAT`: bitmap names, `*POS.DAT`: positions, `EREIGN*.DAT`: events, `ANIMDAT*.DAT`: animations, `FONTDAT*.DAT`: the cell and advance of each character in `GRAFIK\FONT.BMP`, the only font the game draws with; see `game::text`). |
| `TEXTE\` | 23 `*.TXT` | UI texts, one string per line (`AUKTION.TXT`, `BAUMENU.TXT`, `NACHRICH.TXT`, ...). |
| `GRAFIK\` | 322 `*.BMP`, 19 MB | Uncompressed 8-bit Windows 3.x BMPs (`formats::bmp`). Tiles are mostly 128×125, icons 16×16. 310 share one palette; the five title pictures (`TIT*.BMP`) have their own. `GFXGROU2.BMP` is a cut-off copy of `GFXGROUN.BMP` that the game never loads. The installer can copy it to the hard disk (`SETUP.LST`). |
| `MAPS\` | `TUTORIAL.MAP` | The tutorial island. Same format as the scenarios and the saved games (`*.map`, `Last.map`), which the original writes here ([savegame.md](savegame.md)). |
| `SZENARIO\` | 6 `*.HIS`, ~770 KB each | Scenarios: `GELD`, `KOENIG`, `LUXUS`, `OEKO`, `SURFER`, `URLAUBER`. Start with a Pascal string `1.000` (a version) and map dimensions. |
| `MUSIK\` | 10 `*.MID` | Type 1 Standard MIDI, played through MCI. |
| `SOUND\`, `SOUNDLO\` | 40 `*.WAV` each | Effects, 8-bit mono PCM at 22 kHz (`SOUND`) or 11 kHz (`SOUNDLO`). Except `HIGHSCOR.WAV` and the installer-only `HH_HIGH.WAV`: those are the **high-score table** under a `.WAV` name (see below). |
| `SPRADEU1\`–`SPRADEU3\` | 271 `*.WAV`, 32 MB | German speech ("Sprache Deutsch"), 8-bit mono 22 kHz PCM. |
| `FILMHI\`, `FILMLO\` | 23 `*.AVI` each | Cutscenes: Cinepak (`cvid`) 432×320 with MS ADPCM 22 kHz mono audio. `FILMLO` (46 MB vs 94 MB) is a lower-bitrate set of the same videos. |
| `UPGRADE\` | 24 files | Only with the later build (the English release, [versions.md](versions.md)): six more scenarios (`*.HIS`, `EREIGN7`–`12.DAT`), and files that replace the CD's (`SONSTIGE`, `SZENARIO`, `SCHLUSS.TXT`, `HAUSPREI.DAT`, `CREDITS.BMP`) or are new (`HIGHSCO2.*`, `STARTSZ1`, `STRAKN2*`, `DATWIN1D.BMP`). The English install's names are in mixed case (`Hausname.txt`, `Sprache1\`), which the port's case-insensitive lookup takes. |

`HOLIDAY.EXE` also carries data of its own, in its resources: the 23 DFM forms
(component layout, captions, event bindings), cursors, the icon, and the VCL's
button glyphs. Like the CD's files, those aren't committed. The port reads them
from the user's EXE.

## How the game reads the tables

Every `DATA\*.DAT` and `TEXTE\*.TXT` file is a Delphi `Text` file, read by
a loader routine with `Read`, `Readln`, `Eof` and `StrToInt`. The loaders
define the formats, so `crates/formats` ports each one (`formats::data`,
with the routine's address in its doc comment) on top of `formats::pascal`,
which reproduces those RTL routines. Things the loaders have in common:

- They read a fixed number of rows. The files often have notes after them,
  which the game never reads.
- Headings and comments are skipped by line count, not by content. Many
  headings are the original Pascal declarations
  (`Klassenmischung : array [0..BesteKlasse, 0..BesteKlasse] of Integer`),
  which give the parsed fields their names.
- Numbers are either read with `Read` (blank-separated, may span lines) or
  cut out of the line in fixed-width columns (`Copy(line, 1, 5)`, usually 5
  characters, 3 or 10 in some files). Values go into `Integer`, `Byte` or
  `Longint` without range checks.
- Most loaders halt the game when the file is too short. The parsers
  return an error instead.
- The directories are `SETUP.DAT`'s base path plus `Data\` and `Texte\`
  (1000:17bc).
- `ANIMDAT3.DAT`'s group sizes aren't in the file: they're the first words
  of the table at `1110:3620` (initialised data; Ghidra shows it wrong), 7,
  15, 7, 15, 7, 15, 7, 15, 7, 14, 7, 7, 14, 7, 14, 14 last rows. The parser
  takes them as an argument.

Still open:
- `HAUPTSCH.TXT` isn't named anywhere in the EXE and may be unused.
- `SETUP.DAT` (written by the installer) only picks the directories and
  the sound/video quality: `formats::setup`, see [startup.md](startup.md).
- The high-score table (`HIGHSCOR.WAV`) is `formats::highscore`: 10 ranks ×
  7 games of name, date, score and a money text (1040:24a8).

## Maps, scenarios and saved games

`MAPS\*.MAP`, `SZENARIO\*.HIS` and saved games share one format: a dump of
the game's globals. `load_map` (1048:2a19) reads a `string[5]` version
(`1.000`, never checked), then calls 26 unit routines that `BlockRead` their
variables one after the other, with no tags or lengths. `formats::savegame`
follows the same sequence of reads (a table generated from the decompiled
routines) and keeps each read as a field named after the address it goes
to. All seven files on the CD parse to their exact length and write back
identically. Most of it is fixed-size; the variable parts are four counted
record lists (1068:32b5), the window positions of the forms (1010:08d7)
and 0x600 bytes per computer player (10a8:32ec).

The biggest blocks are nine 63001-byte map layers (251 × 251). A save also
carries copies of tables from `DATA\` (`ABRECHNE`, `MODATYP`, `ANIMDAT2/3`,
`NEINTRAG`, `COMPBAU`, `DARSTELL`, `EREIGNIS`), so a scenario can override
them. What the fields mean comes with porting the code that uses them;
[savegame.md](savegame.md) has each unit, saving, loading and what a
saved game loses.
