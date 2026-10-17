//! The films in `orig/FILMHI` and `orig/FILMLO`: their formats, and the
//! decoders' output against `FFmpeg`'s (`ffmpeg` on the `PATH`), pixel for
//! pixel and sample for sample. Skipped without the files; the comparison
//! is skipped without `ffmpeg`. By default it checks the first frames of
//! a few films and their whole sound; `HOLIDAY_FILMS_ALL=1` checks every
//! frame of every film (slow in a debug build).

use std::path::{Path, PathBuf};
use std::process::Command;

use formats::avi::Avi;
use formats::{cinepak, msadpcm};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../orig")
}

fn films(sub: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir().join(sub)) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("avi")))
        .collect();
    v.sort();
    v
}

fn ffmpeg(args: &[&str], file: &Path) -> Option<Vec<u8>> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(args)
        .arg("-")
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

fn audio(avi: &Avi) -> Option<Vec<i16>> {
    let a = avi.audio.as_ref()?;
    let f = &a.format;
    let format = msadpcm::Format::new(f.format_tag, f.channels, f.block_align, f.bits_per_sample, &f.extra).unwrap();
    Some(msadpcm::decode(&format, &avi.audio_bytes()))
}

#[test]
fn film_formats() {
    let all: Vec<PathBuf> = [films("FILMHI"), films("FILMLO")].concat();
    if all.is_empty() {
        eprintln!("skipping: no films in {}", dir().display());
        return;
    }
    assert_eq!(all.len(), 46);
    for path in &all {
        let name = path.file_name().unwrap().to_string_lossy().to_ascii_uppercase();
        let avi = Avi::parse(std::fs::read(path).unwrap()).unwrap();
        let v = &avi.video;
        assert_eq!(&v.format.compression, b"cvid", "{name}");
        assert_eq!(v.format.bit_count, 24, "{name}");
        assert_eq!(avi.palette_changes, 0, "{name}");
        assert_eq!(avi.frames(), usize::try_from(v.header.length).unwrap(), "{name}");
        let size = (v.format.width, v.format.height);
        assert!(size == (432, 320) || size == (335, 257), "{name}: {size:?}");
        // 25 or 12 frames a second (`FILMLO`), `sunflow.avi` 15 in both.
        let fps = (v.header.rate, v.header.scale);
        assert!([(25, 1), (12, 1), (15, 1)].contains(&fps), "{name}: {fps:?}");
        match &avi.audio {
            // `ABking.avi` has no sound.
            None => assert_eq!(name, "ABKING.AVI"),
            Some(a) => {
                let f = &a.format;
                assert_eq!(
                    (
                        f.format_tag,
                        f.channels,
                        f.samples_per_sec,
                        f.block_align,
                        f.bits_per_sample
                    ),
                    (2, 1, 22050, 512, 4),
                    "{name}"
                );
                let format =
                    msadpcm::Format::new(f.format_tag, f.channels, f.block_align, f.bits_per_sample, &f.extra).unwrap();
                assert_eq!(format.samples_per_block, 1012);
                assert_eq!(format.coefficients, msadpcm::STANDARD_COEFFICIENTS);
                // The sound is as long as the film, give or take a block.
                let samples = audio(&avi).unwrap().len();
                let ms = u64::try_from(samples).unwrap() * 1000 / 22050;
                assert!(
                    ms.abs_diff(avi.duration_ms()) < 100,
                    "{name}: {ms} vs {}",
                    avi.duration_ms()
                );
            }
        }
    }
}

#[test]
fn decodes_as_ffmpeg_does() {
    let all = std::env::var_os("HOLIDAY_FILMS_ALL").is_some();
    let mut paths: Vec<PathBuf> = [films("FILMHI"), films("FILMLO")].concat();
    if paths.is_empty() {
        eprintln!("skipping: no films in {}", dir().display());
        return;
    }
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("skipping: no ffmpeg");
        return;
    }
    if !all {
        // A 432 × 320 film, one of the TV's 335 × 257 ones and a `FILMLO`
        // one.
        paths.retain(|p| {
            let s = p.to_string_lossy().to_ascii_uppercase();
            s.ends_with("FILMHI/SUNFLOW.AVI") || s.ends_with("FILMHI/MISS.AVI") || s.ends_with("FILMLO/PLEITE.AVI")
        });
    }
    for path in paths {
        let name = path.display().to_string();
        let avi = Avi::parse(std::fs::read(&path).unwrap()).unwrap();
        let (w, h) = (
            usize::try_from(avi.video.format.width).unwrap(),
            usize::try_from(avi.video.format.height).unwrap(),
        );
        let frames = if all { avi.frames() } else { avi.frames().min(40) };
        let n = frames.to_string();
        let reference = ffmpeg(
            &[
                "-map",
                "0:v",
                "-frames:v",
                &n,
                "-fps_mode",
                "passthrough",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
            ],
            &path,
        )
        .expect("ffmpeg decodes the video");
        // `FFmpeg` passes over the empty (dropped) frames; they repeat the
        // frame before.
        let shown: Vec<usize> = (0..frames).filter(|&i| !avi.frame(i).is_empty()).collect();
        let size = w * h * 3;
        assert_eq!(reference.len(), size * shown.len(), "{name}: the frames FFmpeg gives");
        let mut d = cinepak::Decoder::new(w, h);
        let mut decoded = 0;
        for (&i, want) in shown.iter().zip(reference.chunks(size)) {
            while decoded <= i {
                d.decode(avi.frame(decoded)).unwrap();
                decoded += 1;
            }
            let got = d.rgb();
            if got != want {
                let at = got.iter().zip(want).position(|(a, b)| a != b).unwrap();
                panic!("{name}: frame {i} differs at pixel ({}, {})", at / 3 % w, at / 3 / w);
            }
        }
        if let Some(samples) = audio(&avi) {
            let reference = ffmpeg(&["-map", "0:a", "-f", "s16le", "-acodec", "pcm_s16le"], &path)
                .expect("ffmpeg decodes the sound");
            let want: Vec<i16> = reference
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&b| i16::from_le_bytes(b))
                .collect();
            assert_eq!(samples.len(), want.len(), "{name}: samples");
            assert!(samples == want, "{name}: the sound differs");
        }
    }
}
