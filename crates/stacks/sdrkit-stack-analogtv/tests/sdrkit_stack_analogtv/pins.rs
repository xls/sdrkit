//! Pins of the pictures and the reports of the `atv` and `fpv` receivers on their test signals: a hash of every picture
//! published and of every report, from the first sample. A change of a pin is a change of the picture or of the lock.

use crate::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, Generator, InputInfo, Output, SettingSet, StackConfig};
use sdrkit_stack_analogtv::fpv::{FpvGenParams, FpvGenerator};
use sdrkit_stack_analogtv::settings::FpvSetting;
use sdrkit_stack_analogtv::systems::SystemId;
use sdrkit_stack_analogtv::AtvGenParams;

/// The count of the pictures, their hash and the hash of the reports.
type Pin = (u64, u64, u64);
/// A run that returns its pin and the colour system at its end, with `true` when the picture had colour.
type Case = Box<dyn Fn() -> (Pin, String) + Send + Sync>;

/// FNV-1a, 64 bits.
struct Hash(u64);

impl Hash {
    fn new() -> Hash {
        Hash(0xcbf2_9ce4_8422_2325)
    }
    fn add(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// Feeds `seconds` of `g` to `rx` in pieces of 50 000 samples: the hash of the pictures with their count, and the hash of
/// the status, the log and the plain fields after each piece that changed them. After `stop_s` the signal is noise only.
fn pin(
    g: &mut dyn Generator,
    rx: &mut sdrkit_graph::StackGraph,
    seconds: f64,
    stop_s: f64,
) -> (Pin, String) {
    let rate = g.rate_hz();
    let mut quiet = noise(0.004, 3);
    let mut buf = vec![Complex32::default(); 50_000];
    let mut out = Output::default();
    let (mut pic, mut rep, mut count) = (Hash::new(), Hash::new(), 0u64);
    let mut counter = Owner::results(rx).change_counter();
    for piece in 0..(seconds * rate / 5e4) as u64 {
        if (piece as f64) * 5e4 / rate >= stop_s {
            buf.fill(Complex32::default());
            quiet.process(&mut buf);
        } else {
            g.generate(&mut buf);
        }
        out.clear();
        sdrkit_core::Receiver::feed(rx, &buf, &mut out);
        for p in pictures(&out) {
            pic.add(&p.data);
            count += 1;
        }
        let res = Owner::results(rx);
        if res.change_counter() != counter {
            counter = res.change_counter();
            rep.add(&piece.to_le_bytes());
            rep.add(format!("{:?}", res.status()).as_bytes());
            rep.add(format!("{:?}", res.log_lines()).as_bytes());
            rep.add(format!("{:?}", detail(res, 0.0)).as_bytes());
        }
    }
    let end = detail(Owner::results(rx), 0.0);
    (
        (count, pic.0, rep.0),
        format!("{} {}", end.colour_system, end.colour),
    )
}

fn atv_pin(p: AtvGenParams, settings: &[(&str, &str)], seconds: f64, stop_s: f64) -> (Pin, String) {
    let rate = p.rate_hz;
    let mut g = impaired(
        Box::new(sdrkit_stack_analogtv::AtvGenerator::new(p)),
        Imp::cnr(40.0),
    );
    let mut rx = atv(rate, settings);
    pin(g.as_mut(), &mut rx, seconds, stop_s)
}

fn fpv_pin(p: FpvGenParams, as_atv: bool, seconds: f64) -> (Pin, String) {
    let rate = p.rate_hz;
    let mut g = impaired(Box::new(FpvGenerator::new(p)), Imp::cnr(30.0));
    let mut rx = if as_atv {
        atv(rate, &[("modulation", "fm")])
    } else {
        let text = fpv_channels_text();
        let v = values_of(&FpvSetting::schema(), &[("channels", text.as_str())]);
        let mut rx = sdrkit_stack_analogtv::fpv::receiver(&StackConfig::new(v)).unwrap();
        sdrkit_core::Receiver::configure(&mut rx, &InputInfo::new(rate, 5806e6, 5806e6)).unwrap();
        rx
    };
    pin(g.as_mut(), &mut rx, seconds, f64::MAX)
}

fn system(s: SystemId) -> AtvGenParams {
    AtvGenParams {
        system: s,
        ..params(10e6)
    }
}

#[test]
fn pictures_and_reports_are_pinned() {
    // The `fpv` stack has no field for colour on: its text ends in `false`.
    let cases: Vec<(&str, Case, Pin, &str)> = vec![
        (
            "atv pal",
            Box::new(|| atv_pin(system(SystemId::G), &[], 1.5, f64::MAX)),
            (34, 0xee0698546ec22cd4, 0x19da9211d9dbabab),
            "PAL true",
        ),
        (
            "atv ntsc",
            Box::new(|| atv_pin(system(SystemId::M), &[], 1.5, f64::MAX)),
            (41, 0xbeeebf30e69b0d0e, 0x00d3b6c668fe5881),
            "NTSC true",
        ),
        (
            "atv secam",
            Box::new(|| atv_pin(system(SystemId::L), &[], 1.8, f64::MAX)),
            (42, 0xe63e2488faec9cfd, 0xf33438ab17748900),
            "SECAM true",
        ),
        (
            "atv pal, signal lost",
            Box::new(|| atv_pin(system(SystemId::G), &[], 2.0, 1.2)),
            (27, 0x87be2c092b262629, 0x95156ee748e953a4),
            "mono false",
        ),
        (
            "atv pal, bob and noise reduction",
            Box::new(|| {
                atv_pin(
                    system(SystemId::G),
                    &[("deinterlace", "bob"), ("noise_reduction", "medium")],
                    1.2,
                    f64::MAX,
                )
            }),
            (54, 0xa1b6480fbb88d087, 0xcd2472b2823d208f),
            "PAL true",
        ),
        (
            "atv fm video",
            Box::new(|| {
                fpv_pin(
                    FpvGenParams {
                        sound_hz: 0.0,
                        ..Default::default()
                    },
                    true,
                    1.5,
                )
            }),
            (35, 0xc0b11861a63100d9, 0x3158cebe04970d75),
            "PAL true",
        ),
        (
            "fpv pal",
            Box::new(|| fpv_pin(FpvGenParams::default(), false, 1.2)),
            (28, 0x6d35d4d4a2b22676, 0x1c656ca72ce343dc),
            "PAL false",
        ),
        (
            "fpv ntsc",
            Box::new(|| {
                fpv_pin(
                    FpvGenParams {
                        ntsc: true,
                        ..Default::default()
                    },
                    false,
                    1.2,
                )
            }),
            (33, 0x58b84d5aa6343b6c, 0xa5172c244fe5343f),
            "NTSC false",
        ),
    ];
    let got: Vec<_> = std::thread::scope(|s| {
        let h: Vec<_> = cases.iter().map(|c| s.spawn(|| (c.1)())).collect();
        h.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut bad = false;
    for (c, g) in cases.iter().zip(&got) {
        let (g, colour) = g;
        if c.2 != *g || c.3 != colour {
            bad = true;
            eprintln!(
                "PIN {}: ({}, {:#018x}, {:#018x}) {colour}",
                c.0, g.0, g.1, g.2
            );
        }
    }
    assert!(!bad, "a picture or a report changed");
}
