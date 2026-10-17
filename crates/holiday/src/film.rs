//! A film as MCI's `avivideo` plays it for a `TMediaPlayer` whose
//! `Display` is a panel: the AVI's frames at their rate, at their size
//! from the panel's top left (cut off at its edges), and its sound on the
//! wave device. The game only reads `Position = Length`, which
//! [`Film::at_end`] gives (`docs/films.md`).

use std::path::Path;
use std::sync::Arc;

use formats::avi::Avi;
use formats::{cinepak, msadpcm};
use game::sound::Mci;
use vcl::Rect;

pub struct Film {
    avi: Avi,
    decoder: cinepak::Decoder,
    /// Frames decoded so far.
    decoded: usize,
    /// The device's position and length, in ms.
    pub mci: Mci,
    /// The sound, 16-bit mono, and its rate.
    pub sound: Option<(Arc<[i16]>, u32)>,
    /// The panel it's shown in, on the screen.
    pub panel: Rect,
}

impl Film {
    /// Opens the film at `path` to show in `panel`: its headers, the first
    /// frame and the whole sound decoded.
    ///
    /// # Errors
    /// If the file isn't there, isn't an AVI, or isn't Cinepak.
    pub fn open(path: &Path, panel: Rect) -> Result<Film, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let avi = Avi::parse(bytes).map_err(|e| e.to_string())?;
        let f = &avi.video.format;
        if &f.compression != b"cvid" {
            return Err(format!("not Cinepak: {}", String::from_utf8_lossy(&f.compression)));
        }
        let (w, h) = (
            usize::try_from(f.width.unsigned_abs()).map_err(|e| e.to_string())?,
            usize::try_from(f.height.unsigned_abs()).map_err(|e| e.to_string())?,
        );
        let sound = avi.audio.as_ref().and_then(|a| {
            let f = &a.format;
            // Only MS ADPCM mono, as every film has; anything else is silent.
            let format = msadpcm::Format::new(f.format_tag, f.channels, f.block_align, f.bits_per_sample, &f.extra)
                .ok()
                .filter(|m| m.channels == 1)?;
            Some((msadpcm::decode(&format, &avi.audio_bytes()).into(), f.samples_per_sec))
        });
        let mut mci = Mci::default();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        mci.open(&name, Some(u32::try_from(avi.duration_ms()).unwrap_or(u32::MAX)));
        let mut film = Film {
            decoder: cinepak::Decoder::new(w, h),
            avi,
            decoded: 0,
            mci,
            sound,
            panel,
        };
        film.decode_to(0);
        Ok(film)
    }

    /// Decodes up to frame `n`.
    fn decode_to(&mut self, n: usize) {
        let last = n.min(self.avi.frames().saturating_sub(1));
        while self.decoded <= last {
            if let Err(e) = self.decoder.decode(self.avi.frame(self.decoded)) {
                // The frame is left as far as it decoded.
                eprintln!("film frame {}: {e}", self.decoded);
            }
            self.decoded += 1;
        }
    }

    /// `Play`.
    pub fn play(&mut self) {
        self.mci.play();
        let _ = self.mci.take_events();
    }

    /// `ms` pass: the position moves on, and the frame shown with it.
    pub fn advance(&mut self, ms: u32) {
        self.mci.advance(ms);
        let header = self.avi.video.header;
        let frame = u64::from(self.mci.position()) * u64::from(header.rate) / (1000 * u64::from(header.scale.max(1)));
        self.decode_to(usize::try_from(frame).unwrap_or(usize::MAX));
    }

    /// `Position = Length`.
    #[must_use]
    pub fn at_end(&self) -> bool {
        self.mci.at_end()
    }

    /// What shows on a `screen`-sized screen: the frame at the panel's top
    /// left, cut off at the panel's and the screen's edges, as the
    /// rectangle and its RGB pixels. `None` if none of it shows.
    #[must_use]
    pub fn picture(&self, screen: (i32, i32)) -> Option<(Rect, Vec<u8>)> {
        let (w, h) = (
            i32::try_from(self.decoder.width()).ok()?,
            i32::try_from(self.decoder.height()).ok()?,
        );
        let p = self.panel;
        let left = p.left.max(0);
        let top = p.top.max(0);
        let right = (p.left + w.min(p.width())).min(screen.0);
        let bottom = (p.top + h.min(p.height())).min(screen.1);
        if right <= left || bottom <= top {
            return None;
        }
        let rgb = self.decoder.rgb();
        let row = usize::try_from(w).ok()? * 3;
        let (x0, x1) = (
            usize::try_from(left - p.left).ok()?,
            usize::try_from(right - p.left).ok()?,
        );
        let mut out = Vec::with_capacity((x1 - x0) * 3 * usize::try_from(bottom - top).ok()?);
        for y in top - p.top..bottom - p.top {
            let start = usize::try_from(y).ok()? * row;
            out.extend_from_slice(&rgb[start + x0 * 3..start + x1 * 3]);
        }
        Some((Rect::new(left, top, right - left, bottom - top), out))
    }
}
