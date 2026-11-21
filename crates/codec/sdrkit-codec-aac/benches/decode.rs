//! Decoding speed of HE-AAC v2 and HE-AAC frames, timed with `sdrkit_core::bench::measure` (the fastest of several runs).
//! Run with `cargo bench -p sdrkit-codec-aac`; each line prints frames per second and the time of one frame.

use sdrkit_codec_aac::register;
use sdrkit_core::bench::measure;
use sdrkit_core::codec::{CodecSet, Pcm};
use sdrkit_core::media::{Codec, MediaFormat, Packaging};

fn frames(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 7 <= data.len() {
        let len = (usize::from(data[at + 3] & 3) << 11)
            | (usize::from(data[at + 4]) << 3)
            | usize::from(data[at + 5] >> 5);
        out.push(&data[at..at + len]);
        at += len;
    }
    out
}

fn main() {
    for (name, file, codec) in [
        ("HE-AAC v2, 48 kHz", "hev2_48k.aac", Codec::HeAacV2),
        ("HE-AAC stereo, 48 kHz", "he_48k_stereo.aac", Codec::HeAac),
        ("AAC-LC stereo, 48 kHz", "lc_48k_stereo.aac", Codec::Aac),
    ] {
        let data =
            std::fs::read(format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let frames = frames(&data);
        let mut set = CodecSet::default();
        register(&mut set);
        let entry = set.find(codec.fourcc(), "").unwrap();
        let mut dec = (entry.factory)(&MediaFormat::new(codec, Packaging::Adts)).unwrap();
        let mut pcm = Pcm::default();
        let r = measure(0.5, frames.len(), 0.0, || {
            for f in &frames {
                pcm.clear();
                dec.decode(f, false, &mut pcm).unwrap();
            }
        });
        println!(
            "{name:<24} {:>9.0} frames/s  ({:.3} ms/frame)",
            r.items_per_second,
            1e3 / r.items_per_second
        );
    }
}
