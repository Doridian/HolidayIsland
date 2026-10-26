//! The languages the game is shown in: German and English
//! (`docs/language.md`), with all their text in the repository's
//! `lang/` folder (`lang/README.md`), embedded into the build.
//!
//! The original has no language setting: its texts are the game
//! folder's (`TEXTE\*.TXT`, `UPGRADE\*.TXT`), and the few places in
//! `HOLIDAY.EXE` that differ by language go by `MODATYP.DAT`'s `land`
//! (`D`, `A`, `E`, `F`; `load_modatyp`, en 1000:23ed): the date's suffix
//! (`date_text`, en 1030:12b0), the thousands and decimal marks
//! (`group_digits`, en 1030:2145; `insert_decimal_marks`, en 1030:21f3)
//! and the later build's two file menu messages (en 1040:0eef,
//! en 1040:110c). The VCL's message box texts are compiled into each
//! build in its language.
//!
//! The port carries both languages itself. Each has, in `lang/<code>/`:
//! the text files the game reads (`TEXTE/`, `UPGRADE/`, as the original
//! has them, with the label layouts of `DATA/BAUMENU.DAT`,
//! `GELDMENU.DAT` and `INFOBOX.DAT`), and `strings.toml` with the EXE's
//! strings, the formats ([`Format`]) and the port's own texts
//! ([`Text`], the enhancements' labels). [`Language::file`] gives a
//! text file as the original's loaders read it: Windows-1252 with CRLF.
//!
//! The player's [`Setting`] (`language=auto|de|en` in `settings.ini`,
//! `--lang=`) is one player's view setting: `auto` follows the folder's
//! `land`. The language decides every text of that player's view. The
//! world has a language of its own ([`crate::world::World::language`]):
//! the one a game was created in, for the text the world makes and
//! saves (the computers' names, the news archive's dates).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use formats::data::Land;

/// A language the game can be shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum Language {
    #[default]
    De,
    En,
}

impl Language {
    /// Every language, in `lang/`'s order.
    pub const ALL: [Language; 2] = [Language::De, Language::En];

    /// The language of a folder whose `MODATYP.DAT` says `land`: German
    /// for `D` (and anything the game doesn't know), English for the
    /// rest. The port has no French texts; `F` gets English.
    #[must_use]
    pub fn of_land(land: Land) -> Language {
        match land {
            Land::Deutschland => Language::De,
            Land::Amerika | Land::England | Land::Frankreich => Language::En,
        }
    }

    /// The language's code: its folder in `lang/`, `language=` in
    /// `settings.ini`, `language=` in a saved game's port chunk.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Language::De => "de",
            Language::En => "en",
        }
    }

    /// The language with `code` (any case).
    #[must_use]
    pub fn from_code(code: &str) -> Option<Language> {
        Language::ALL
            .into_iter()
            .find(|l| l.code().eq_ignore_ascii_case(code.trim()))
    }

    fn data(self) -> &'static Data {
        match self {
            Language::De => &DE,
            Language::En => &EN,
        }
    }

    /// The text file `path` (`TEXTE/NAMEN.TXT`, `UPGRADE/SZENARIO.TXT`,
    /// `DATA/GELDMENU.DAT`; any case, `\` or `/`) in this language, as the
    /// game's loaders read it: Windows-1252, CRLF line ends, without the
    /// port's header comments (lines that start with `//` at the top).
    /// None for a file the language data doesn't have.
    #[must_use]
    pub fn file(self, path: &str) -> Option<Vec<u8>> {
        let path = path.replace('\\', "/");
        let (_, text) = self
            .data()
            .files
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&path))?;
        Some(file_bytes(text))
    }

    /// The names of the text files this language has, as in `lang/`.
    #[must_use]
    pub fn file_names(self) -> Vec<&'static str> {
        self.data().files.iter().map(|(name, _)| *name).collect()
    }

    /// `text` in this language.
    #[must_use]
    pub fn text(self, text: Text) -> &'static str {
        &self.data().texts[text as usize].0
    }

    /// `text` in this language, in the game's font ([`encode`]).
    #[must_use]
    pub fn bytes(self, text: Text) -> &'static [u8] {
        &self.data().texts[text as usize].1
    }

    /// The string `key` (`enhance.big_map.label`) of `strings.toml`.
    #[must_use]
    pub fn get(self, key: &str) -> Option<&'static str> {
        self.data().strings.get(key).map(String::as_str)
    }

    /// Every key of this language's `strings.toml`.
    pub fn keys(self) -> impl Iterator<Item = &'static str> {
        self.data().strings.keys().map(String::as_str)
    }

    /// `MONATE.TXT`'s twelve months in this language.
    #[must_use]
    pub fn months(self) -> &'static [Vec<u8>] {
        &self.data().months
    }

    /// How this language writes dates and numbers.
    #[must_use]
    pub fn format(self) -> &'static Format {
        &self.data().format
    }
}

/// The player's choice: `language=` in `settings.ini`, `--lang=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Setting {
    /// The game folder's language (`MODATYP.DAT`'s `land`).
    #[default]
    Auto,
    /// This language, whatever the folder's.
    Is(Language),
}

impl Setting {
    /// `auto` or a language's code (any case); none for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Setting> {
        if text.trim().eq_ignore_ascii_case("auto") {
            Some(Setting::Auto)
        } else {
            Language::from_code(text).map(Setting::Is)
        }
    }

    /// The language on a folder whose `MODATYP.DAT` says `folder`.
    #[must_use]
    pub fn language(self, folder: Land) -> Language {
        match self {
            Setting::Auto => Language::of_land(folder),
            Setting::Is(language) => language,
        }
    }
}

/// How a language writes dates and numbers: `strings.toml`'s
/// `[format]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Format {
    /// The date's pattern, with `{day}`, `{suffix}`, `{month}`, `{year}`.
    date: Vec<u8>,
    /// The suffix after each day, 1 to 31.
    day_suffixes: Vec<Vec<u8>>,
    /// The thousands mark (`group_digits`, en 1030:2145).
    pub thousands: u8,
    /// The decimal mark (`insert_decimal_marks`, en 1030:21f3).
    pub decimal: u8,
    /// The high score table's date, with `dd`, `MM`, `yy`.
    highscore_date: Vec<u8>,
}

impl Format {
    fn parse(strings: &BTreeMap<String, String>) -> Format {
        let get = |key: &str| strings.get(&format!("format.{key}")).map_or("", String::as_str);
        let mark = |key: &str, default: u8| encode(get(key)).first().copied().unwrap_or(default);
        let mut day_suffixes: Vec<Vec<u8>> = get("day_suffixes").split('|').map(encode).collect();
        if day_suffixes.len() == 1 {
            day_suffixes = vec![day_suffixes[0].clone(); 31];
        }
        Format {
            date: encode(get("date")),
            day_suffixes,
            thousands: mark("thousands", b','),
            decimal: mark("decimal", b'.'),
            highscore_date: encode(get("highscore_date")),
        }
    }

    /// The suffix after `day` (1 to 31).
    #[must_use]
    pub fn day_suffix(&self, day: u8) -> &[u8] {
        usize::from(day)
            .checked_sub(1)
            .and_then(|d| self.day_suffixes.get(d))
            .map_or(&[][..], Vec::as_slice)
    }

    /// A date as the info bar shows it: `6. Januar 1996`,
    /// `6th January 1996` (`date_text`, en 1030:12b0, 1030:0148).
    #[must_use]
    pub fn date_text(&self, day: u8, month: &[u8], year: &[u8]) -> Vec<u8> {
        let day_text = day.to_string();
        fill(
            &self.date,
            &[
                (b"{day}", day_text.as_bytes()),
                (b"{suffix}", self.day_suffix(day)),
                (b"{month}", month),
                (b"{year}", year),
            ],
        )
    }

    /// The high score table's date (Delphi's `DateToStr`, 1008:0512, in
    /// the short date format of the language's Windows): `04.09.96`,
    /// `04/09/96`.
    #[must_use]
    pub fn highscore_date(&self, day: u32, month: u32, year: i32) -> Vec<u8> {
        let (d, m, y) = (
            format!("{day:02}"),
            format!("{month:02}"),
            format!("{:02}", year.rem_euclid(100)),
        );
        fill(
            &self.highscore_date,
            &[(b"dd", d.as_bytes()), (b"MM", m.as_bytes()), (b"yy", y.as_bytes())],
        )
    }
}

/// `pattern` with each of `fields`' names replaced by its value.
fn fill(pattern: &[u8], fields: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut rest = pattern;
    'outer: while let Some(&b) = rest.first() {
        for (name, value) in fields {
            if let Some(after) = rest.strip_prefix(*name) {
                out.extend_from_slice(value);
                rest = after;
                continue 'outer;
            }
        }
        out.push(b);
        rest = &rest[1..];
    }
    out
}

/// Defines [`Text`]: the strings of `strings.toml` the code names, each
/// by its key.
macro_rules! texts {
    ($( $(#[$m:meta])* $variant:ident = $key:literal, )*) => {
        /// A string of `strings.toml` the code uses, by its key: the EXE's
        /// strings and the port's own texts. The enhancements' labels and
        /// descriptions are looked up by their keys (`crate::enhance`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Text {
            $( $(#[$m])* $variant, )*
        }

        impl Text {
            /// Every one, in the enum's order.
            pub const ALL: &'static [Text] = &[$(Text::$variant),*];

            /// Its key in `strings.toml`.
            #[must_use]
            pub fn key(self) -> &'static str {
                match self {
                    $( Text::$variant => $key, )*
                }
            }
        }
    };
}

texts! {
    /// The message boxes' captions (`crate::msgdlg`).
    MsgWarning = "msgdlg.warning",
    MsgError = "msgdlg.error",
    MsgInformation = "msgdlg.information",
    MsgConfirmation = "msgdlg.confirmation",
    /// Their buttons.
    BtnYes = "msgdlg.yes",
    BtnNo = "msgdlg.no",
    BtnOk = "msgdlg.ok",
    BtnCancel = "msgdlg.cancel",
    BtnAbort = "msgdlg.abort",
    BtnRetry = "msgdlg.retry",
    BtnIgnore = "msgdlg.ignore",
    BtnAll = "msgdlg.all",
    BtnHelp = "msgdlg.help",
    /// The file menu's messages (`crate::dateimenue`).
    InvalidName = "file_menu.invalid_name",
    DeleteQuestion = "file_menu.delete_question",
    /// An empty news archive entry's date (`crate::news`).
    NoDate = "news.no_date",
    /// The media name for none (`crate::fernseher`).
    NoMedium = "news.no_medium",
    /// The computers' status texts (`crate::computer`).
    ComputerIdle = "computer.idle",
    ComputerThinking = "computer.thinking",
    ComputerDemolishing = "computer.demolishing",
    ComputerWaiting = "computer.waiting",
    ComputerSelling = "computer.selling",
    ComputerNothing = "computer.nothing",
    ComputerStartArea = "computer.start_area",
    ComputerChangeArea = "computer.change_area",
    ComputerChooseCategory = "computer.choose_category",
    ComputerChooseType = "computer.choose_type",
    ComputerFindRoad = "computer.find_road",
    ComputerFindSite = "computer.find_site",
    ComputerBuild = "computer.build",
    ComputerLevel = "computer.level",
    ComputerEvenOut = "computer.even_out",
    ComputerRoad = "computer.road",
    ComputerRoadStart = "computer.road_start",
    /// The enhancements' list (`crate::enhanceform`): its title.
    EnhanceTitle = "port.enhance_title",
    /// Its two lists' headings and the lines under them.
    EnhanceFixes = "port.enhance_fixes",
    EnhanceView = "port.enhance_view",
    EnhanceFixesNote = "port.enhance_fixes_note",
    EnhanceViewNote = "port.enhance_view_note",
    /// Its buttons.
    EnhanceAllOn = "port.enhance_all_on",
    EnhanceAllOff = "port.enhance_all_off",
    EnhanceCredits = "port.enhance_credits",
    /// What it says about saving.
    EnhanceSaved = "port.enhance_saved",
    EnhanceNotSaved = "port.enhance_not_saved",
    EnhanceCommandLine = "port.enhance_command_line",
    EnhanceScreenOff = "port.enhance_screen_off",
    /// Before the mouse is over a row.
    EnhanceHint = "port.enhance_hint",
    /// The host's option of how long an auction waits for the human
    /// seats (`crate::simulation::Options::auction_timeout`), its label
    /// and what it does.
    AuctionTimeout = "port.auction_timeout",
    AuctionTimeoutDescription = "port.auction_timeout_description",
    /// A network game's end for a client (`docs/networking.md`): the
    /// host quit, the connection to the host is gone, or the session
    /// ended otherwise; then the offer to save the client's copy.
    NetHostQuit = "port.net_host_quit",
    NetHostLost = "port.net_host_lost",
    NetEnded = "port.net_ended",
    NetSaveQuestion = "port.net_save_question",
    /// A client waiting for the host's world, and for a dropped host to
    /// come back.
    NetWaitingHost = "port.net_waiting_host",
    NetHostDropped = "port.net_host_dropped",
    /// The browser build's file menu button that saves the game as a
    /// download (`crate::dateimenue`), and a saved game dropped onto the
    /// window: the question before it replaces the game in progress, and
    /// what a file that isn't one gets.
    Download = "port.download",
    DropLoadQuestion = "port.drop_load_question",
    DropNotAGame = "port.drop_not_a_game",
}

impl Text {
    /// The text in `language`.
    #[must_use]
    pub fn get(self, language: Language) -> &'static str {
        language.text(self)
    }

    /// The text in `language` in the game's font ([`encode`]).
    #[must_use]
    pub fn bytes(self, language: Language) -> Vec<u8> {
        language.bytes(self).to_vec()
    }
}

/// One language's data, parsed from the embedded files.
struct Data {
    /// `TEXTE/…`, `UPGRADE/…`, `DATA/…`: the path in `lang/<code>/` and
    /// the UTF-8 text.
    files: &'static [(&'static str, &'static str)],
    /// `strings.toml`, by `section.key`.
    strings: BTreeMap<String, String>,
    /// Each [`Text`], in its order, as UTF-8 and in the game's font.
    texts: Vec<(String, Vec<u8>)>,
    format: Format,
    /// `TEXTE/MONATE.TXT`, parsed.
    months: Vec<Vec<u8>>,
}

impl Data {
    fn new(files: &'static [(&'static str, &'static str)], strings: &str) -> Data {
        let strings = parse_strings(strings).unwrap_or_else(|e| panic!("strings.toml: {e}"));
        let texts = Text::ALL
            .iter()
            .map(|t| {
                let s = strings.get(t.key()).cloned().unwrap_or_else(|| t.key().to_owned());
                let bytes = encode(&s);
                (s, bytes)
            })
            .collect();
        let months = files
            .iter()
            .find(|(name, _)| *name == "TEXTE/MONATE.TXT")
            .and_then(|(_, text)| formats::data::monate(&file_bytes(text)).ok())
            .unwrap_or_default();
        Data {
            files,
            months,
            format: Format::parse(&strings),
            strings,
            texts,
        }
    }
}

/// The files of `lang/<code>/`, embedded.
macro_rules! lang_files {
    ($lang:literal: $($file:literal),* $(,)?) => {
        &[$(($file, include_str!(concat!("../../../lang/", $lang, "/", $file)))),*]
    };
}

/// The text files every language has, in `lang/<code>/`: every
/// `TEXTE\*.TXT` of the CD, `UPGRADE\`'s texts and the label layouts
/// that differ by translation (`docs/language.md`).
macro_rules! every_file {
    ($lang:literal) => {
        lang_files!($lang:
            "TEXTE/AUKTION.TXT", "TEXTE/AUSWAHL.TXT", "TEXTE/BAUMENU.TXT", "TEXTE/FEHLER.TXT",
            "TEXTE/GELDMENU.TXT", "TEXTE/HAUPTSCH.TXT", "TEXTE/HAUSNAME.TXT", "TEXTE/HIGHSCOR.TXT",
            "TEXTE/INFOBOX.TXT", "TEXTE/MODATYP.TXT", "TEXTE/MONATE.TXT", "TEXTE/NACHRICH.TXT",
            "TEXTE/NAMEN.TXT", "TEXTE/OPTIONEN.TXT", "TEXTE/SABOTAGE.TXT", "TEXTE/SABOTEXT.TXT",
            "TEXTE/SCHLUSS.TXT", "TEXTE/SONSTIGE.TXT", "TEXTE/START.TXT", "TEXTE/SZENARIO.TXT",
            "TEXTE/UEBERMAP.TXT", "TEXTE/UMENU.TXT", "TEXTE/WASNUN.TXT",
            "UPGRADE/HIGHSCO2.TXT", "UPGRADE/SCHLUSS.TXT", "UPGRADE/SONSTIGE.TXT", "UPGRADE/SZENARIO.TXT",
            "DATA/BAUMENU.DAT", "DATA/GELDMENU.DAT", "DATA/INFOBOX.DAT",
        )
    };
}

static DE: LazyLock<Data> =
    LazyLock::new(|| Data::new(every_file!("de"), include_str!("../../../lang/de/strings.toml")));
static EN: LazyLock<Data> =
    LazyLock::new(|| Data::new(every_file!("en"), include_str!("../../../lang/en/strings.toml")));

/// A file of `lang/` as the game's loaders read it: without the header
/// comments, Windows-1252, CRLF.
fn file_bytes(text: &str) -> Vec<u8> {
    let mut body = text;
    while body.starts_with("//") {
        body = body.split_once('\n').map_or("", |(_, rest)| rest);
    }
    encode(&body.replace('\n', "\r\n"))
}

/// The Windows-1252 characters of 0x80 to 0x9f; the five it leaves out
/// are the C1 controls of the same number.
const CP1252_HIGH: [char; 32] = [
    '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}', //
    '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}', //
    '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}', //
    '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
];

/// `text` in the game's Windows-1252 font; a character it doesn't have
/// is a `?`.
#[must_use]
pub fn encode(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| {
            if let Some(i) = CP1252_HIGH.iter().position(|&h| h == c) {
                0x80 + u8::try_from(i).unwrap_or(0x1f)
            } else {
                match u8::try_from(u32::from(c)) {
                    Ok(b) if !(0x80..0xa0).contains(&b) => b,
                    _ => b'?',
                }
            }
        })
        .collect()
}

/// Windows-1252 `bytes` as text.
#[must_use]
pub fn decode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x80..0xa0).contains(&b) {
                CP1252_HIGH[usize::from(b - 0x80)]
            } else {
                char::from(b)
            }
        })
        .collect()
}

/// Parses `strings.toml`, the subset of TOML it's written in: `#`
/// comments, `[section]` headings, and `key = "value"` lines with a
/// basic string (`\"`, `\\`, `\n`, `\t`, `\uXXXX`). The keys come out as
/// `section.key`.
///
/// # Errors
/// The first line that isn't one of those, or a key given twice.
pub fn parse_strings(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut section = String::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        let err = |what: &str| format!("line {}: {what}", n + 1);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[') {
            let name = name.strip_suffix(']').ok_or_else(|| err("no ] after the section"))?;
            if !valid_key(name, true) {
                return Err(err("bad section name"));
            }
            name.clone_into(&mut section);
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(|| err("not key = \"value\""))?;
        let key = key.trim();
        if !valid_key(key, false) {
            return Err(err("bad key"));
        }
        let (value, rest) = basic_string(value.trim()).ok_or_else(|| err("bad string"))?;
        let rest = rest.trim();
        if !(rest.is_empty() || rest.starts_with('#')) {
            return Err(err("text after the string"));
        }
        let full = if section.is_empty() {
            key.to_owned()
        } else {
            format!("{section}.{key}")
        };
        if out.insert(full, value).is_some() {
            return Err(err("the key is there twice"));
        }
    }
    Ok(out)
}

fn valid_key(key: &str, dots: bool) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || (dots && b == b'.'))
}

/// A TOML basic string at the start of `s`, and what follows it.
fn basic_string(s: &str) -> Option<(String, &str)> {
    let mut chars = s.strip_prefix('"')?.char_indices();
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &s[i + 2..])),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'u' => {
                    let hex: String = (0..4).filter_map(|_| chars.next().map(|(_, c)| c)).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                _ => return None,
            },
            c => out.push(c),
        }
    }
    None
}

/// The world's language a saved game's port chunk (`trailing`) names
/// (`language=de`); none in an original file or an older port's.
#[must_use]
pub fn language_in(trailing: &[u8]) -> Option<Language> {
    formats::savegame::port_chunk(trailing)?
        .into_iter()
        .find(|(k, _)| k == "language")
        .and_then(|(_, v)| Language::from_code(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings() {
        assert_eq!(Setting::parse("auto"), Some(Setting::Auto));
        assert_eq!(Setting::parse(" EN "), Some(Setting::Is(Language::En)));
        assert_eq!(Setting::parse("de"), Some(Setting::Is(Language::De)));
        assert_eq!(Setting::parse("fr"), None);
        assert_eq!(Setting::default(), Setting::Auto);
    }

    #[test]
    fn auto_follows_the_folder() {
        assert_eq!(Setting::Auto.language(Land::Deutschland), Language::De);
        assert_eq!(Setting::Auto.language(Land::England), Language::En);
        assert_eq!(Setting::Auto.language(Land::Amerika), Language::En);
        assert_eq!(Setting::Is(Language::De).language(Land::England), Language::De);
        assert_eq!(Setting::Is(Language::En).language(Land::Deutschland), Language::En);
    }

    /// Every language has every key and every file the others have, and
    /// every key the code names.
    #[test]
    fn every_language_is_complete() {
        for language in Language::ALL {
            for t in Text::ALL {
                assert!(language.get(t.key()).is_some(), "{}: no {}", language.code(), t.key());
            }
            for info in crate::enhance::all() {
                for part in ["label", "description"] {
                    let key = format!("enhance.{}.{part}", info.key);
                    assert!(
                        language.get(&key).is_some_and(|s| !s.is_empty()),
                        "{}: no {key}",
                        language.code()
                    );
                }
            }
            for other in Language::ALL {
                for key in other.keys() {
                    assert!(
                        language.get(key).is_some(),
                        "{} has {key}, {} hasn't",
                        other.code(),
                        language.code()
                    );
                }
                assert_eq!(language.file_names(), other.file_names());
            }
            // Every file is there and isn't empty; the strings are all
            // in the game's font.
            for name in language.file_names() {
                assert!(language.file(name).is_some_and(|f| !f.is_empty()), "{name}");
            }
            for key in language.keys() {
                let text = language.get(key).unwrap_or_default();
                assert_eq!(decode(&encode(text)), text, "{key} isn't all Windows-1252");
            }
            let f = language.format();
            assert_eq!(f.day_suffixes.len(), 31);
            assert!(!f.date.is_empty() && !f.highscore_date.is_empty());
        }
    }

    #[test]
    fn files_read_as_the_original_has_them() {
        let data = Language::De.file("texte\\monate.txt").unwrap();
        assert!(data.starts_with(b"Januar\r\nFebruar\r\nM\xe4rz\r\n"));
        assert_eq!(Language::En.file("TEXTE/NOPE.TXT"), None);
        // The port's header comment isn't part of the file.
        let up = Language::De.file("UPGRADE/SZENARIO.TXT").unwrap();
        assert!(up.starts_with(b"Money-Maker\r\n"));
        assert!(
            Language::En
                .file("UPGRADE/SZENARIO.TXT")
                .unwrap()
                .starts_with(b"Money Maker\r\n")
        );
    }

    #[test]
    fn formats() {
        let (de, en) = (Language::De.format(), Language::En.format());
        assert_eq!(de.date_text(6, b"Januar", b"1996"), b"6. Januar 1996");
        assert_eq!(en.date_text(22, b"February", b"1997"), b"22nd February 1997");
        assert_eq!(en.date_text(13, b"May", b"1997"), b"13th May 1997");
        assert_eq!(en.date_text(31, b"May", b"1997"), b"31st May 1997");
        assert_eq!(
            (de.thousands, de.decimal, en.thousands, en.decimal),
            (b'.', b',', b',', b'.')
        );
        assert_eq!(de.highscore_date(4, 9, 1996), b"04.09.96");
        assert_eq!(en.highscore_date(4, 9, 2001), b"04/09/01");
    }

    #[test]
    fn texts() {
        assert_eq!(Text::EnhanceTitle.get(Language::De), "ERWEITERUNGEN");
        assert_eq!(Text::EnhanceSaved.bytes(Language::De)[0], 0xc4, "Ä");
        assert_eq!(Language::De.bytes(Text::MsgConfirmation), b"Best\xe4tigen");
        assert_eq!(Language::En.bytes(Text::DeleteQuestion), b"Are you sure?");
        assert_eq!(Language::De.bytes(Text::DeleteQuestion), b"Wirklich loeschen?");
    }

    #[test]
    fn windows_1252() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(encode(&decode(&all)), all);
        assert_eq!(encode("€ „x“ – £ ß"), b"\x80 \x84x\x93 \x96 \xa3 \xdf");
        assert_eq!(encode("\u{3b1}"), b"?");
    }

    #[test]
    fn strings_toml() {
        let s = parse_strings("# c\nx = \"a\\\"b\" # c\n[s.t]\ny = \"\\u00e4\\n\"\n").unwrap();
        assert_eq!(s["x"], "a\"b");
        assert_eq!(s["s.t.y"], "ä\n");
        assert!(parse_strings("x = 1").is_err());
        assert!(parse_strings("x = \"a\"\nx = \"b\"").is_err());
        assert!(parse_strings("x = \"a\" y").is_err());
        assert!(parse_strings("[a\n").is_err());
    }

    #[test]
    fn saved_language() {
        let chunk = formats::savegame::make_port_chunk([("fix.x", "1"), ("language", "en")]);
        assert_eq!(language_in(&chunk), Some(Language::En));
        assert_eq!(language_in(&[]), None);
        assert_eq!(
            language_in(&formats::savegame::make_port_chunk([("language", "xx")])),
            None
        );
    }
}
