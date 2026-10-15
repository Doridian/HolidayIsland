# Game data on the CD

A first survey, not reverse-engineered yet. File names are 8.3 upper case on
the CD, but `HOLIDAY.EXE` spells them in mixed case (`Hausdata.dat`, `Grafik\`,
`Filmhi\`), so the port needs case-insensitive file lookup. All text is German,
Windows-1252.

| Dir | Files | What |
|---|---|---|
| `DATA\` | 39 `*.DAT`, 364 KB | Game tables, all plain text, mostly numbers and names in fixed-width columns (`HAUSDATA.DAT`: buildings, `HAUSPREI.DAT`: prices, `FILENAME.DAT`: bitmap names, `*POS.DAT`: positions, `EREIGN*.DAT`: events, `ANIMDAT*.DAT`: animations, `FONTDAT*.DAT`: fonts). |
| `TEXTE\` | 23 `*.TXT` | UI texts, one string per line (`AUKTION.TXT`, `BAUMENU.TXT`, `NACHRICH.TXT`, ...). |
| `GRAFIK\` | 322 `*.BMP`, 19 MB | Uncompressed 8-bit Windows 3.x BMPs. Tiles are mostly 128×125, icons 16×16. The installer can copy it to the hard disk (`SETUP.LST`). |
| `MAPS\` | `TUTORIAL.MAP` | The tutorial island. Same format as the scenarios and presumably the save games (`*.map`, `Last.map`). |
| `SZENARIO\` | 6 `*.HIS`, ~770 KB each | Scenarios: `GELD`, `KOENIG`, `LUXUS`, `OEKO`, `SURFER`, `URLAUBER`. Start with a Pascal string `1.000` (a version) and map dimensions. |
| `MUSIK\` | 10 `*.MID` | Type 1 Standard MIDI, played through MCI. |
| `SOUND\`, `SOUNDLO\` | 40 `*.WAV` each | Effects, 8-bit mono PCM at 22 kHz (`SOUND`) or 11 kHz (`SOUNDLO`). Except `HIGHSCOR.WAV` and the installer-only `HH_HIGH.WAV`: those are the **high-score table** under a `.WAV` name (player names such as `Spieler1neu`, dates such as `30.10.1996`, in a binary layout not decoded yet). |
| `SPRADEU1\`–`SPRADEU3\` | 271 `*.WAV`, 32 MB | German speech ("Sprache Deutsch"), 8-bit mono 22 kHz PCM. |
| `FILMHI\`, `FILMLO\` | 23 `*.AVI` each | Cutscenes: Cinepak (`cvid`) 432×320 with MS ADPCM 22 kHz mono audio. `FILMLO` (46 MB vs 94 MB) is a lower-bitrate set of the same videos. |

`HOLIDAY.EXE` also carries data of its own, in its resources: the 23 DFM forms
(component layout, captions, event bindings), cursors, the icon, and the VCL's
button glyphs. Like the CD's files, those aren't committed. The port reads them
from the user's EXE.
