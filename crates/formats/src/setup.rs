//! `SETUP.DAT`, which the installer writes into the game's directory, and the
//! directories `init_paths` (1000:17bc) derives from it.
//!
//! The file has five lines:
//! 1. the base directory, the CD's root (`D:\`);
//! 2. `0` if the speech (`SPRADEU1`–`3`) was copied to the hard disk;
//! 3. `0` if `GRAFIK` was copied to the hard disk;
//! 4. `L` for the small videos (`FILMLO`), anything else for `FILMHI`;
//! 5. `N` for no sound, `L` for the 8-bit sound in `SOUNDLO`, anything
//!    else for `SOUND`.
//!
//! The comparisons are exact: `l` isn't `L`. `MAPS`, `SOUND`/`SOUNDLO` and
//! `MUSIK` are always read from the game's directory. All paths are
//! `string[144]`.

use crate::pascal::TextReader;

/// The file `init_paths` opens, relative to the current directory. Without
/// it the game shows [`MISSING_SETUP`] and quits.
pub const FILE_NAME: &[u8] = b"setup.dat";

/// Capacity of the directory strings.
pub const PATH_LEN: usize = 144;

/// "Please run SETUP.EXE from the HOLIDAY ISLAND CD first." Each build
/// has the message in its own language; the later (English) build keeps
/// the German and a French one too, but shows the English
/// (`init_paths`, en 1000:196d). The port picks by the player's language
/// (`game::lang`, `docs/language.md`). // 1000:15b0, en 1000:15c4
pub const MISSING_SETUP: &[u8] = b"Bitte starten Sie zun\xe4chst SETUP.EXE von der HOLIDAY ISLAND CD.";
/// The later build's [`MISSING_SETUP`]. // en 1000:1649
pub const MISSING_SETUP_EN: &[u8] = b"Please start first SETUP.EXE on the HOLIDAY ISLAND CD.";

/// "Please put the HOLIDAY ISLAND CD in the CD-ROM drive and start HOLIDAY
/// ISLAND again.", as [`MISSING_SETUP`]. // 1000:175b, en 1000:185e
pub const MISSING_CD: &[u8] =
    b"Bitte legen Sie die HOLIDAY ISLAND CD in das CD-Rom Laufwerk und starten Sie HOLIDAY ISLAND neu.";
/// The later build's [`MISSING_CD`]. // en 1000:1918
pub const MISSING_CD_EN: &[u8] =
    b"Please put the HOLIDAY ISLAND CD in the CD-Rom Drive and start HOLIDAY ISLAND again.";

/// The later build's question when going back to the start menu finds no
/// CD (`back_to_start_menu`, with a retry), shown in English like
/// [`MISSING_SETUP_EN`]; the German one is in the build too.
/// // en 1010:3582, en 1010:3502
pub const INSERT_CD: &[u8] = b"Bitte legen Sie die HOLIDAY ISLAND CD in das CD-Rom Laufwerk.";
/// The English [`INSERT_CD`]. // en 1010:3540
pub const INSERT_CD_EN: &[u8] = b"Please put the HOLIDAY ISLAND CD in the CD-Rom Drive.";
/// The film in the film directory whose presence counts as the CD in the
/// drive when going back to the start menu (`back_to_start_menu`'s
/// `FileExists(film_dir + 'surfwm.avi')`, en 1010:35e4). // en 1010:34f7
pub const INSERT_CD_FILM: &str = "surfwm.avi";

/// The files whose presence counts as "the CD is in the drive": one in the
/// data directory, then these in the film directory, checked in this order.
pub const CD_CHECK_DATA_FILE: &[u8] = b"Modatyp.dat";
pub const CD_CHECK_FILMS: [&[u8]; 21] = [
    b"abende.avi",
    b"abgeld.avi",
    b"abking.avi",
    b"ableute.avi",
    b"abluxus.avi",
    b"aboeko.avi",
    b"abpleite.avi",
    b"absurfer.avi",
    b"haitauch.avi",
    b"miss.avi",
    b"oel.avi",
    b"piranhas.avi",
    b"pleite.avi",
    b"ratten.avi",
    b"saufbier.avi",
    b"spreng.avi",
    b"stink.avi",
    b"surfwm.avi",
    b"tennis.avi",
    b"tuschel.avi",
    b"abrente.avi",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Film {
    /// `FILMLO`.
    Lo,
    /// `FILMHI`.
    Hi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// No sound: `1110:0062` is false.
    Off,
    /// `SOUNDLO`: `1110:54dc` is true.
    Lo,
    /// `SOUND`.
    Hi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    /// Line 1, cut to [`PATH_LEN`].
    pub base_dir: Vec<u8>,
    pub speech_on_cd: bool,
    pub grafik_on_cd: bool,
    pub film: Film,
    pub sound: Sound,
}

/// The asset directories, each ending in `\`. Those without the base
/// directory are relative to the game's directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub data: Vec<u8>,     // 1110:4de2
    pub texte: Vec<u8>,    // 1110:4e74
    pub maps: Vec<u8>,     // 1110:50bc
    pub szenario: Vec<u8>, // 1110:514e
    pub sound: Vec<u8>,    // 1110:51e0
    pub musik: Vec<u8>,    // 1110:4f06
    /// `SPRADEU1`–`3`. // 1110:5304, 1110:5395, 1110:5426
    pub spradeu: [Vec<u8>; 3],
    pub grafik: Vec<u8>, // 1110:4f98
    pub film: Vec<u8>,   // 1110:502a
}

fn path(parts: &[&[u8]]) -> Vec<u8> {
    let mut p = parts.concat();
    p.truncate(PATH_LEN);
    p
}

impl Setup {
    /// Reads `SETUP.DAT` like `init_paths`. Missing lines read as empty.
    #[must_use]
    pub fn parse(data: &[u8]) -> Self {
        let mut f = TextReader::new(data);
        let mut base_dir = f.read_line();
        base_dir.truncate(PATH_LEN);
        let speech_on_cd = f.read_line() != b"0";
        let grafik_on_cd = f.read_line() != b"0";
        let film = if f.read_line() == b"L" { Film::Lo } else { Film::Hi };
        let sound = match &f.read_line()[..] {
            b"N" => Sound::Off,
            b"L" => Sound::Lo,
            _ => Sound::Hi,
        };
        Setup {
            base_dir,
            speech_on_cd,
            grafik_on_cd,
            film,
            sound,
        }
    }

    /// The directories `init_paths` sets.
    #[must_use]
    pub fn paths(&self) -> Paths {
        let base = &self.base_dir[..];
        let on_cd = |cd: bool, dir: &[u8]| if cd { path(&[base, dir]) } else { dir.to_vec() };
        Paths {
            data: path(&[base, b"Data\\"]),
            texte: path(&[base, b"Texte\\"]),
            maps: b"Maps\\".to_vec(),
            szenario: path(&[base, b"Szenario\\"]),
            // With the sound off this stays `Sound\`.
            sound: if self.sound == Sound::Lo {
                b"SoundLo\\".to_vec()
            } else {
                b"Sound\\".to_vec()
            },
            musik: b"Musik\\".to_vec(),
            spradeu: [b"spradeu1\\", b"spradeu2\\", b"spradeu3\\"].map(|d| on_cd(self.speech_on_cd, d)),
            grafik: on_cd(self.grafik_on_cd, b"Grafik\\"),
            film: match self.film {
                Film::Lo => path(&[base, b"Filmlo\\"]),
                Film::Hi => path(&[base, b"Filmhi\\"]),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let setup = Setup::parse(b"D:\\\r\n0\r\n1\r\nL\r\nL\r\n");
        assert_eq!(setup.base_dir, b"D:\\");
        assert!(!setup.speech_on_cd && setup.grafik_on_cd);
        assert_eq!((setup.film, setup.sound), (Film::Lo, Sound::Lo));
        let paths = setup.paths();
        assert_eq!(paths.data, b"D:\\Data\\");
        assert_eq!(paths.sound, b"SoundLo\\");
        assert_eq!(paths.spradeu[2], b"spradeu3\\");
        assert_eq!(paths.grafik, b"D:\\Grafik\\");
        assert_eq!(paths.film, b"D:\\Filmlo\\");

        let setup = Setup::parse(b"E:\\\r\n");
        assert!(setup.speech_on_cd && setup.grafik_on_cd);
        assert_eq!((setup.film, setup.sound), (Film::Hi, Sound::Hi));
        assert_eq!(Setup::parse(b"\r\n\r\n\r\n\r\nN").sound, Sound::Off);
    }

    #[test]
    fn long_base_dir() {
        let mut data = vec![b'x'; 200];
        data.extend_from_slice(b"\r\n");
        let paths = Setup::parse(&data).paths();
        assert_eq!(paths.data, vec![b'x'; PATH_LEN]);
    }
}
