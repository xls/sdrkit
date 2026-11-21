//! A recorded elementary stream. The test runs when `SDRKIT_MPEG2_ES` names a file of access units, each as a length
//! (32 bits, little endian), a time stamp (64 bits, `i64::MIN` for none), a byte that is 1 for a damaged unit, and the
//! bytes. It checks the picture size and the number of frames against the time span, compares the two forms of the
//! arithmetic, prints the decoding time per frame, and writes some frames as PNG files to `SDRKIT_MPEG2_OUT`.
//! Run it optimised: `cargo test --profile shipped -p sdrkit-codec-mpeg2video --test recorded -- --nocapture`.

use sdrkit_codec_image::{png, PixelFormat, Raster};
use sdrkit_codec_mpeg2video::{Decoder, Frame, PictureKind};
use std::time::Instant;

struct Unit {
    pts: Option<i64>,
    damaged: bool,
    bytes: Vec<u8>,
}

fn units(file: &[u8]) -> Vec<Unit> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 13 <= file.len() {
        let len = u32::from_le_bytes(file[at..at + 4].try_into().unwrap()) as usize;
        let pts = i64::from_le_bytes(file[at + 4..at + 12].try_into().unwrap());
        out.push(Unit {
            pts: (pts != i64::MIN).then_some(pts),
            damaged: file[at + 12] != 0,
            bytes: file[at + 13..at + 13 + len].to_vec(),
        });
        at += 13 + len;
    }
    out
}

/// The frame as BGRA samples, with the colours of ITU-R BT.601.
fn bgra(f: &Frame) -> Raster {
    let (w, h) = (f.width as usize, f.height as usize);
    let mut data = Vec::with_capacity(4 * w * h);
    for y in 0..h {
        for x in 0..w {
            // The chroma rows of an interlaced frame belong to the fields in turn.
            let cy = if f.interlaced {
                (y / 4) * 2 + y % 2
            } else {
                y / 2
            };
            let c = cy.min(h / 2 - 1) * f.chroma_stride + x / 2;
            let l = 1.164 * (f.y[y * f.stride + x] as f32 - 16.0);
            let (cb, cr) = (f.cb[c] as f32 - 128.0, f.cr[c] as f32 - 128.0);
            let (r, g, b) = (l + 1.596 * cr, l - 0.392 * cb - 0.813 * cr, l + 2.017 * cb);
            data.extend([b, g, r, 255.0].map(|v| v.round().clamp(0.0, 255.0) as u8));
        }
    }
    Raster::from_vec(PixelFormat::Bgra, w as u32, h as u32, data).expect("a raster")
}

fn hash(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h = (*h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[test]
fn a_recorded_stream_decodes() {
    let Ok(path) = std::env::var("SDRKIT_MPEG2_ES") else {
        return;
    };
    let out = std::env::var("SDRKIT_MPEG2_OUT").ok();
    let units = units(&std::fs::read(&path).expect("the stream reads"));
    assert!(!units.is_empty());
    let mut digests = Vec::new();
    for portable in [false, true] {
        let mut dec = Decoder::new();
        dec.set_portable(portable);
        let mut digest = 0xcbf2_9ce4_8422_2325u64;
        let mut shown: Vec<(Option<i64>, PictureKind, u32, bool)> = Vec::new();
        let mut size = None;
        let mut clean_errors = 0;
        let mut sink = |f: &Frame| {
            size = Some((
                f.width,
                f.height,
                f.aspect,
                f.frame_rate,
                f.interlaced,
                f.top_field_first,
            ));
            hash(&mut digest, f.y);
            hash(&mut digest, f.cb);
            hash(&mut digest, f.cr);
            let pick = shown.len() % 25 == 12
                || shown.len() < 3
                || (f.concealed > 0 && f.concealed < 1620);
            if let (Some(dir), false, true) = (&out, portable, pick) {
                let file = format!("{dir}/frame-{:04}-{:?}.png", shown.len(), f.kind);
                std::fs::write(file, png::encode(&bgra(f)).expect("a PNG file"))
                    .expect("the file writes");
            }
            shown.push((f.pts, f.kind, f.concealed, f.damaged));
        };
        for (n, u) in units.iter().enumerate() {
            let before = dec.stats();
            dec.decode(&u.bytes, u.pts, &mut sink);
            let after = dec.stats();
            // The last access unit of a recording ends where the recording ends.
            if !u.damaged && n + 1 < units.len() {
                clean_errors += after.slice_errors - before.slice_errors;
            }
        }
        dec.flush(&mut sink);
        let stats = dec.stats();
        let frames = shown.len();
        let (first, last) = (
            shown.iter().find_map(|s| s.0).unwrap(),
            shown.iter().rev().find_map(|s| s.0).unwrap(),
        );
        let (w, h, aspect, rate, interlaced, tff) = size.expect("a frame");
        let damaged = units.iter().filter(|u| u.damaged).count();
        let span = (last - first) as f64 / 90_000.0;
        let kinds = |k| shown.iter().filter(|s| s.1 == k).count();
        println!(
            "{}: {} access units ({} damaged), {frames} frames {w}x{h} {}:{} at {}/{} per second, interlaced {interlaced}, \
             top field first {tff}; I {} P {} B {}; time span {span:.3} s; {:?}; slice errors in undamaged units {clean_errors}; \
             digest {digest:016x}",
            if portable { "portable" } else { "default" },
            units.len(),
            damaged,
            aspect.0,
            aspect.1,
            rate.0,
            rate.1,
            kinds(PictureKind::I),
            kinds(PictureKind::P),
            kinds(PictureKind::B),
            stats,
        );
        assert_eq!((w, h), (720, 576));
        // Every frame is whole frame times after the one before, also where the recording lost pictures or the
        // first field of a frame.
        let step = 90_000 * rate.1 as i64 / rate.0 as i64;
        let steps: Vec<i64> = shown
            .windows(2)
            .map(|p| p[1].0.unwrap() - p[0].0.unwrap())
            .collect();
        assert!(
            steps.iter().all(|d| *d > 0 && d % step == 0),
            "the time stamps rise by whole frames"
        );
        let lost = (last - first) / step + 1 - frames as i64;
        let marked = shown.iter().filter(|s| s.3).count();
        println!("  {lost} frames lost, {marked} frames marked as damaged");
        assert!(lost <= 4 * damaged as i64, "{lost} frames lost");
        // A frame with a concealed part is marked, the first frames are not, and the marks end.
        assert!(shown.iter().all(|s| s.3 || s.2 == 0));
        assert!(!shown[0].3);
        assert!(marked < 15 * damaged.max(1), "{marked} frames marked");
        assert_eq!(clean_errors, 0);
        digests.push(digest);
    }
    assert_eq!(
        digests[0], digests[1],
        "the two forms give the same samples"
    );
    // The time of decoding alone: the two forms in turn, the best of ten passes that do nothing with the frames.
    let mut best = [f64::MAX; 2];
    for pass in 0..20 {
        let mut dec = Decoder::new();
        dec.set_portable(pass % 2 == 1);
        let mut count = 0;
        let start = Instant::now();
        for u in &units {
            dec.decode(&u.bytes, u.pts, &mut |_| count += 1);
        }
        dec.flush(&mut |_| count += 1);
        best[pass % 2] = best[pass % 2].min(start.elapsed().as_secs_f64() * 1e3 / count as f64);
    }
    println!(
        "decoding time per frame: default {:.3} ms, portable {:.3} ms",
        best[0], best[1]
    );
}
