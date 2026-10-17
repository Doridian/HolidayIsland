//! Prints an AVI's streams, and with frame numbers writes those frames as
//! PPM files.
//!
//! `cargo run -p formats --example avidump -- orig/FILMHI/TITEL.AVI [FRAME... OUT_PREFIX]`

use formats::avi::Avi;
use formats::cinepak::Decoder;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: avidump FILE.AVI [FRAME... OUT_PREFIX]");
        std::process::exit(2);
    };
    let avi = Avi::parse(std::fs::read(path).expect("read the file")).expect("an AVI file");
    let v = &avi.video;
    println!(
        "video: {} x {}, {} bits, {}, {} frames at {}/{} a second, {} ms",
        v.format.width,
        v.format.height,
        v.format.bit_count,
        String::from_utf8_lossy(&v.format.compression),
        avi.frames(),
        v.header.rate,
        v.header.scale,
        avi.duration_ms()
    );
    match &avi.audio {
        Some(a) => println!(
            "audio: tag {}, {} channel(s), {} Hz, {} bits, blocks of {}, {} bytes",
            a.format.format_tag,
            a.format.channels,
            a.format.samples_per_sec,
            a.format.bits_per_sample,
            a.format.block_align,
            avi.audio_bytes().len()
        ),
        None => println!("audio: none"),
    }
    let Some((prefix, frames)) = args[1..].split_last() else {
        return;
    };
    let wanted: Vec<usize> = frames.iter().map(|f| f.parse().expect("a frame number")).collect();
    let (w, h) = (
        usize::try_from(v.format.width).unwrap_or(0),
        usize::try_from(v.format.height).unwrap_or(0),
    );
    let mut d = Decoder::new(w, h);
    let last = wanted.iter().copied().max().unwrap_or(0);
    for n in 0..=last.min(avi.frames().saturating_sub(1)) {
        if let Err(e) = d.decode(avi.frame(n)) {
            eprintln!("frame {n}: {e}");
        }
        if wanted.contains(&n) {
            let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
            ppm.extend(d.rgb());
            let out = format!("{prefix}{n}.ppm");
            std::fs::write(&out, ppm).expect("write the frame");
            println!("{out}");
        }
    }
}
