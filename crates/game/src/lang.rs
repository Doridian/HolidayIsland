//! The language the player sees: German or English (`docs/language.md`).
//!
//! The original has no language setting. Its texts are the game folder's
//! (`TEXTE\*.TXT`, the speech, the films' sound), and the few places in
//! `HOLIDAY.EXE` that differ by language go by `MODATYP.DAT`'s `land`
//! (`D`, `A`, `E`, `F`; `load_modatyp`, en 1000:23ed): the date's suffix
//! (`date_text`, en 1030:12b0), the thousands and decimal marks
//! (`group_digits`, en 1030:2145; `insert_decimal_marks`, en 1030:21f3)
//! and the later build's two file menu messages (en 1040:0eef,
//! en 1040:110c). A few more texts are compiled into each build in its
//! language (the CD and `SETUP.EXE` messages of en 1000:196d and
//! en 1010:3582).
//!
//! The port's [`Setting`] (`language=auto|de|en` in `settings.ini`,
//! `--lang=`) is one player's view setting: `auto` follows the folder's
//! `land`, so German data shows German and English data English. A
//! forced language switches what the EXE and the port decide: the
//! view's `land` for the dates and numbers ([`Language::land`]), the
//! EXE's messages and the port's own texts ([`Text`], the enhancements'
//! labels). The folder's texts stay as they are. The world never sees
//! it: what of the world is text (the players' names from `NAMEN.TXT`,
//! the news archive's dates) is made from the folder's data, the same
//! for every player.

use formats::data::Land;

/// A language the port shows its own texts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    #[default]
    De,
    En,
}

impl Language {
    /// The language of a folder whose `MODATYP.DAT` says `land`: German
    /// for `D` (and anything the game doesn't know), English for the
    /// rest. The port has no French texts; `F` keeps its own `land` for
    /// the numbers and gets the English texts.
    #[must_use]
    pub fn of_land(land: Land) -> Language {
        match land {
            Land::Deutschland => Language::De,
            Land::Amerika | Land::England | Land::Frankreich => Language::En,
        }
    }

    /// The `land` the view formats dates and numbers with, on a folder
    /// whose `MODATYP.DAT` says `folder`: the folder's own when it's this
    /// language, else `D` for German and `E` for English.
    #[must_use]
    pub fn land(self, folder: Land) -> Land {
        if Language::of_land(folder) == self {
            folder
        } else {
            match self {
                Language::De => Land::Deutschland,
                Language::En => Land::England,
            }
        }
    }

    /// The index of this language's column in the port's tables.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Language::De => 0,
            Language::En => 1,
        }
    }

    /// `de` in German, `en` in English.
    #[must_use]
    pub fn pick<T>(self, de: T, en: T) -> T {
        match self {
            Language::De => de,
            Language::En => en,
        }
    }

    /// The setting's name (`de`, `en`).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Language::De => "de",
            Language::En => "en",
        }
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
    /// `auto`, `de` or `en` (any case); none for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Setting> {
        match text.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Setting::Auto),
            "de" => Some(Setting::Is(Language::De)),
            "en" => Some(Setting::Is(Language::En)),
            _ => None,
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

    /// The view's `land` on that folder ([`Language::land`]).
    #[must_use]
    pub fn land(self, folder: Land) -> Land {
        self.language(folder).land(folder)
    }
}

/// `text` in the game's Windows-1252 font: the Latin-1 letters as they
/// are, anything else a `?`.
#[must_use]
pub fn latin1(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

/// The port's own texts on the screen. The enhancements' labels and
/// descriptions are in their list (`crate::enhance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// The enhancements' list (`crate::enhanceform`): its title.
    EnhanceTitle,
    /// Its two lists' headings and the lines under them.
    EnhanceFixes,
    EnhanceView,
    EnhanceFixesNote,
    EnhanceViewNote,
    /// Its buttons.
    EnhanceAllOn,
    EnhanceAllOff,
    EnhanceCredits,
    /// What it says about saving.
    EnhanceSaved,
    EnhanceNotSaved,
    EnhanceCommandLine,
    EnhanceScreenOff,
    /// Before the mouse is over a row.
    EnhanceHint,
}

/// Each [`Text`] in German and English, in the enum's order.
const TEXTS: [[&str; 2]; 13] = [
    ["ERWEITERUNGEN", "ENHANCEMENTS"],
    ["Spielkorrekturen", "Game fixes"],
    ["Anzeige und Ton", "Screen and sound"],
    ["Für dieses Spiel und neue Spiele", "For this game and new games"],
    ["Ihre Einstellungen für jedes Spiel", "Your settings for every game"],
    ["Alle an", "All on"],
    ["Alle aus (Original)", "All off (original)"],
    ["Mitwirkende", "Credits"],
    [
        "Änderungen gelten sofort und werden in settings.ini gespeichert.",
        "Changes apply at once and are saved in settings.ini.",
    ],
    [
        "Änderungen gelten sofort, aber nur bis zum Beenden (keine settings.ini).",
        "Changes apply at once, but only until you quit (no settings.ini).",
    ],
    [
        "Beim nächsten Start gehen --original und --enhance wieder vor.",
        "At the next start --original and --enhance come first again.",
    ],
    [
        "INFO zeigt nun die Mitwirkenden (zurück: enhance.enhancements_screen=on).",
        "CREDITS (INFO) now opens the credits (back: enhance.enhancements_screen=on).",
    ],
    [
        "Die Maus über einer Zeile zeigt hier, was sie ändert.",
        "Hold the mouse over a row to see here what it changes.",
    ],
];

impl Text {
    /// The text in `language`.
    #[must_use]
    pub fn get(self, language: Language) -> &'static str {
        TEXTS[self as usize][language.index()]
    }

    /// The text in `language` in the game's font ([`latin1`]).
    #[must_use]
    pub fn bytes(self, language: Language) -> Vec<u8> {
        latin1(self.get(language))
    }
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
        for land in [Land::Deutschland, Land::Amerika, Land::England, Land::Frankreich] {
            assert_eq!(Setting::Auto.land(land), land, "the folder's own");
        }
    }

    #[test]
    fn forced_languages() {
        let en = Setting::Is(Language::En);
        let de = Setting::Is(Language::De);
        assert_eq!(en.land(Land::Deutschland), Land::England);
        assert_eq!(en.land(Land::Amerika), Land::Amerika);
        assert_eq!(en.land(Land::Frankreich), Land::Frankreich);
        assert_eq!(de.land(Land::England), Land::Deutschland);
        assert_eq!(de.land(Land::Deutschland), Land::Deutschland);
        // The view's language is always its land's.
        for s in [Setting::Auto, en, de] {
            for land in [Land::Deutschland, Land::Amerika, Land::England, Land::Frankreich] {
                assert_eq!(Language::of_land(s.land(land)), s.language(land));
            }
        }
    }

    #[test]
    fn texts() {
        assert_eq!(Text::EnhanceTitle.get(Language::De), "ERWEITERUNGEN");
        assert_eq!(Text::EnhanceHint.get(Language::En), TEXTS[12][1]);
        assert_eq!(Text::EnhanceSaved.bytes(Language::De)[0], 0xc4, "Ä");
        assert!(TEXTS.iter().flatten().all(|t| t.chars().all(|c| u32::from(c) < 0x100)));
    }
}
