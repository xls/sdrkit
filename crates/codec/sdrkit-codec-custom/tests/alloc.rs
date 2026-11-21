//! Makes sure that the steady-state paths allocate nothing: each coder runs several frames after warm-up under an allocator
//! that counts. (The counting allocator needs `unsafe impl GlobalAlloc`. The library itself forbids `unsafe`.)

use sdrkit_codec_custom::{picture, voice3, voice4};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    let before = ALLOCS.load(Ordering::Relaxed);
    f();
    ALLOCS.load(Ordering::Relaxed) - before
}

fn speech(n: usize, seed: u32) -> Vec<i16> {
    let mut s = seed;
    (0..n)
        .map(|i| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let t = i as f64 / 16000.0;
            let env = if (i / 4000) % 3 == 2 { 0.0 } else { 1.0 };
            (env * (3000.0 * (2.0 * std::f64::consts::PI * 180.0 * t).sin())
                + ((s >> 20) as f64 - 2048.0) * 0.5) as i16
        })
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    let pcm = speech(64000, 3);
    for (rate, ms, profile, ib) in [
        (
            voice4::SampleRate::Hz16000,
            voice4::FrameLength::Ms40,
            voice4::Profile::P6_0,
            true,
        ),
        (
            voice4::SampleRate::Hz16000,
            voice4::FrameLength::Ms20,
            voice4::Profile::Hq16,
            false,
        ),
        (
            voice4::SampleRate::Hz8000,
            voice4::FrameLength::Ms20,
            voice4::Profile::P8_0,
            false,
        ),
    ] {
        let cfg = voice4::Config {
            rate,
            frame: ms,
            profile,
            type_in_band: ib,
        };
        let mut enc = voice4::Encoder::new(
            cfg,
            voice4::EncoderSettings {
                dtx: true,
                noise_suppressor: true,
            },
        )
        .unwrap();
        let mut dec = voice4::Decoder::new(cfg, voice4::DecoderSettings::PLAIN).unwrap();
        let n = enc.frame_samples();
        let mut out = [0u8; 128];
        let mut pcm_out = vec![0i16; n];
        let mut run = |frames: std::ops::Range<usize>| {
            for f in frames {
                let e = enc.encode(&pcm[f * n..(f + 1) * n], &mut out).unwrap();
                let input = if f % 9 == 4 {
                    voice4::Input::Lost
                } else {
                    voice4::Input::Frame(&out[..e.len()])
                };
                dec.decode(input, voice4::Damage::default(), &mut pcm_out)
                    .unwrap();
            }
        };
        run(0..4);
        let frames = 64000 / n;
        assert_eq!(count(|| run(4..frames)), 0, "voice4 {cfg:?}");
    }
    for v in [
        voice3::Variant::V3(voice3::V3Rate::R12),
        voice3::Variant::V3(voice3::V3Rate::R32),
        voice3::Variant::V1,
    ] {
        let mut enc = voice3::Encoder::new(v);
        let mut dec = voice3::Decoder::new(v);
        let mut out = [0u8; 80];
        let mut pcm_out = [0i16; 320];
        let mut run = |frames: std::ops::Range<usize>| {
            for f in frames {
                let nb = enc.encode(&pcm[f * 320..(f + 1) * 320], &mut out).unwrap();
                dec.decode(
                    if f % 7 == 3 { None } else { Some(&out[..nb]) },
                    &mut pcm_out,
                )
                .unwrap();
            }
        };
        run(0..2);
        assert_eq!(count(|| run(2..100)), 0, "{v:?}");
    }
    let rgb: Vec<u8> = (0..96 * 64 * 3).map(|i| ((i * 7) % 251) as u8).collect();
    let mut enc = picture::Encoder::new();
    let mut dec = picture::Decoder::new();
    let mut stream = Vec::new();
    let mut pic = Vec::new();
    let hd = enc.encode(&rgb, 96, 64, 100_000, &mut stream).unwrap();
    dec.decode(&hd, &stream, &mut pic).unwrap();
    let n = count(|| {
        let hd = enc.encode(&rgb, 96, 64, 100_000, &mut stream).unwrap();
        dec.decode(&hd, &stream, &mut pic).unwrap();
    });
    assert_eq!(n, 0, "picture");
}
