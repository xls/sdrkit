//! HamDRM: the frame that the HB9TLK "DRM mode HAM" document fixes, the format file loaded at run time, the refusal without it,
//! and generator -> receiver round trips with the self-check fixture.

use crate::support::{
    apply, cell_num, cell_text, impair, int, invoke, is_none, num, row_ids, snr, stack_config,
    text, Set,
};
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Results, SettingSet, StackConfig};
use sdrkit_stack_broadcast::drm::mlc::{Mlc, Rate};
use sdrkit_stack_broadcast::drm::params::{gain_phase, Qam};
use sdrkit_stack_broadcast::hamdrm::frame::{
    CAPACITY_PRINTED, CARRIERS, FRAME_SAMPLES_48K, MAP_A_WIDE, MAP_B_WIDE, MAP_E_NARROW,
    MSC_CELLS_PRINTED,
};
use sdrkit_stack_broadcast::hamdrm::{
    self, capacity, CellKind, CellMap, Content, FacValues, FrameInfo, GenOptions, HamFormat,
    HamGenerator, HamMode, HISTORY,
};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use serde_json::json;
use std::sync::Arc;

const DIAL: f64 = 14.233e6;

fn vectors(name: &str) -> String {
    format!(
        "{}/../../../tests/vectors/broadcast/hamdrm/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// The self-check fixture: a format file for the loader and the round trips, not the HamDRM format.
fn fixture_path() -> String {
    vectors("self-check-format.json")
}

fn fixture() -> Arc<HamFormat> {
    Arc::new(HamFormat::load(&fixture_path()).expect("the self-check fixture loads"))
}

fn config(format: Option<&str>) -> StackConfig {
    let settings: Vec<Set> = format
        .map(|p| ("hamdrm_format", json!(p)))
        .into_iter()
        .collect();
    stack_config(hamdrm::HamSetting::schema(), &settings)
}

fn opts(mode: HamMode, wide: bool, qam: Qam, protection: u8) -> GenOptions {
    GenOptions {
        mode,
        wide,
        qam,
        protection,
        transmissions: 1,
        format: Some(fixture()),
        ..Default::default()
    }
}

fn default_opts() -> GenOptions {
    GenOptions {
        format: Some(fixture()),
        ..Default::default()
    }
}

fn generate(o: GenOptions, rate: f64, seconds: f64, extra: &[Set]) -> Vec<Complex32> {
    let g = HamGenerator::new(o, rate).expect("a rate it can reach");
    let mut all: Vec<Set> = snr(35.0).to_vec();
    all.push(("gen_seed", json!(5)));
    all.extend(extra.iter().cloned());
    impair(Box::new(g), seconds, &all)
}

/// A recorded clean signal played as a generator, so that the framework adds the noise over all of it.
struct Replay {
    x: Vec<Complex32>,
    at: usize,
    width: f64,
}

impl sdrkit_core::Generator for Replay {
    fn rate_hz(&self) -> f64 {
        48e3
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            *o = self.x.get(self.at).copied().unwrap_or_default();
            self.at += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        Some(sdrkit_core::SignalInfo::new(self.width))
    }
}

/// `seconds` of the signal of `o` at 48 kHz with nothing sent from `from` for `gap` seconds, and noise over all of it.
fn with_gap(o: GenOptions, seconds: f64, from: f64, gap: f64) -> Vec<Complex32> {
    let mut g = HamGenerator::new(o, 48e3).unwrap();
    let mut x = vec![Complex32::default(); (seconds * 48e3) as usize];
    sdrkit_core::Generator::generate(&mut g, &mut x);
    let at = (from * 48e3) as usize;
    for s in &mut x[at..at + (gap * 48e3) as usize] {
        *s = Complex32::default();
    }
    let mut all: Vec<Set> = snr(35.0).to_vec();
    all.push(("gen_seed", json!(5)));
    impair(
        Box::new(Replay {
            x,
            at: 0,
            width: 2500.0,
        }),
        seconds,
        &all,
    )
}

fn feed_with(x: &[Complex32], rate: f64, piece: usize, cfg: &StackConfig) -> Results {
    let mut rx = hamdrm::make_receiver(cfg).unwrap();
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(piece) {
        rx.feed(c, &mut out);
        out.clear();
    }
    rx.results().clone()
}

fn feed(x: &[Complex32], rate: f64, piece: usize) -> Results {
    feed_with(x, rate, piece, &config(Some(&fixture_path())))
}

/// The newest finished file's column `key`.
fn newest(r: &Results, key: &str) -> String {
    row_ids(r, "history")
        .first()
        .map(|id| cell_text(r, &HISTORY, id, key))
        .unwrap_or_default()
}

fn want_crc() -> String {
    CRC32_ISO_HDLC.checksum(&hamdrm::test_picture()).to_string()
}

/// The length of the first transmission and 1.5 s after it.
fn seconds_for(o: &GenOptions) -> f64 {
    let g = HamGenerator::new(o.clone(), 48e3).unwrap();
    0.3 + g.transmission_frames(0) as f64 * o.mode.frame_s() + 1.5
}

/// The map rows of one mode as the HamDRM document prints them, when a copy of the document's text is in the folder named by
/// `SDRKIT_HAMDRM_DOC` (the repository keeps no copy of third-party pages). `None` when no copy is there.
fn printed_rows(caption: &str) -> Option<Vec<String>> {
    let dir = std::env::var_os("SDRKIT_HAMDRM_DOC")?;
    let t = std::fs::read_to_string(std::path::Path::new(&dir).join("hb9tlk-mode-ham.txt")).ok()?;
    let at = t.find(caption).expect("the caption");
    Some(
        t[at..]
            .lines()
            .skip(2)
            .take_while(|l| !l.trim().is_empty())
            .map(|l| l.trim_end().to_string())
            .collect(),
    )
}

// ---------------------------------------------------------------------------------------------------------------------------
// What the documents fix

/// "Timing": FFT_SIZE 1152, 1024, 640 at 48 kHz, Tg/Tu 1/9, 1/4, 1/2, 15, 15 and 20 symbols, 19200 samples (400 ms) per frame.
#[test]
fn timing_follows_the_document() {
    let want = [(288, 32, 15), (256, 64, 15), (160, 80, 20)];
    for (m, w) in HamMode::ALL.into_iter().zip(want) {
        assert_eq!((m.nfft(), m.cp(), m.nsym()), w, "{m:?}");
        assert_eq!(
            4 * m.nsym() * (m.nfft() + m.cp()),
            FRAME_SAMPLES_48K,
            "{m:?}"
        );
        assert!((m.frame_s() - 0.4).abs() < 1e-12, "{m:?}");
    }
    assert!((HamMode::B.spacing() - 46.875).abs() < 1e-9);
}

/// "Carrier Config": the drawn maps are the document's text, with 57, 51 and 29 carriers, 45 FAC cells, and 704, 530 and 339 MSC
/// cells. The document prints "tot MSC cells 707" for mode A at 2.5 kHz; its bit counts follow from 704 (see the capacity test).
#[test]
fn drawn_maps_are_the_document_text() {
    let cases: [(&str, &[&str], HamMode, usize, usize); 3] = [
        (
            "Robustness mode A / Spectrum occupancy 1",
            &MAP_A_WIDE,
            HamMode::A,
            57,
            704,
        ),
        (
            "Robustness mode B / Spectrum occupancy 1",
            &MAP_B_WIDE,
            HamMode::B,
            51,
            530,
        ),
        (
            "Robustness mode E / Spectrum occupancy 0",
            &MAP_E_NARROW,
            HamMode::E,
            29,
            339,
        ),
    ];
    for (caption, rows, m, carriers, msc) in cases {
        if let Some(printed) = printed_rows(caption) {
            assert_eq!(
                printed,
                rows.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                "{caption}"
            );
        }
        let map = CellMap::drawn(m);
        assert_eq!(map.carriers(), carriers);
        assert_eq!(carriers, CARRIERS[m.index()][m.drawn_wide() as usize]);
        assert_eq!(map.count(CellKind::Fac), 45, "{m:?}");
        assert_eq!(map.count(CellKind::Msc), msc, "{m:?}");
        // Every cell is one kind: the counts add up to the frame.
        let all: usize = [
            CellKind::Msc,
            CellKind::Fac,
            CellKind::Time,
            CellKind::Frequency,
            CellKind::Scattered,
            CellKind::Boosted,
        ]
        .iter()
        .map(|&k| map.count(k))
        .sum();
        assert_eq!(all, carriers * m.nsym());
    }
    assert_eq!(MSC_CELLS_PRINTED[HamMode::B.index()][1], 530);
    assert_eq!(MSC_CELLS_PRINTED[HamMode::E.index()][0], 339);
    assert_eq!(MSC_CELLS_PRINTED[HamMode::A.index()][1], 707);
}

/// WinDRM Docs Release 1.3: the frequency pilots sit at 725, 1475 and 1850 Hz with carrier 0 at 350 Hz. The drawn maps put them
/// on carriers 9, 27, 36 (A), 8, 24, 32 (B) and 5, 15, 20 (E), so column 0 is carrier 2 (A) or 1 (B, E).
#[test]
fn frequency_pilots_sit_at_725_1475_and_1850_hz() {
    let want = [([9, 27, 36], 2), ([8, 24, 32], 1), ([5, 15, 20], 1)];
    for (m, (k, kmin)) in HamMode::ALL.into_iter().zip(want) {
        assert_eq!(m.freq_pilot_carriers(), k, "{m:?}");
        let map = CellMap::drawn(m);
        assert_eq!(map.kmin(), kmin, "{m:?}");
        for (i, &kk) in k.iter().enumerate() {
            let hz = hamdrm::DC_HZ + kk as f64 * m.spacing();
            assert!((hz - [725.0, 1475.0, 1850.0][i]).abs() < 1e-9, "{m:?} {hz}");
        }
    }
}

/// The scattered pilots of the drawn maps of modes A and B are the gain pilots of ES 201 980 table 58 ((x, y, k0) = (4, 5, 2)
/// and (2, 3, 1)) on the carriers that no time or frequency pilot takes. Mode E has (1, 4, 1), which no DRM mode has.
#[test]
fn scattered_pilots_follow_the_drm_gain_pattern_in_a_and_b() {
    for m in HamMode::ALL {
        let map = CellMap::drawn(m);
        for s in 0..map.nsym() {
            for c in 0..map.carriers() {
                let k = map.kmin() + c as i32;
                let kind = map.kind(s, c);
                let on = match m.drm() {
                    Some(r) => gain_phase(r, s, k).is_some(),
                    None => (k - 1 - (s as i32 % 4)).rem_euclid(4) == 0,
                };
                let scattered = matches!(kind, CellKind::Scattered | CellKind::Boosted);
                assert!(
                    !scattered || on,
                    "{m:?} s {s} k {k}: a scattered pilot off the pattern"
                );
                if on {
                    assert!(
                        kind.is_pilot(),
                        "{m:?} s {s} k {k}: the pattern cell is {kind:?}"
                    );
                }
            }
        }
        // The boosted pilots are at the edges only.
        for i in map.cells_of(CellKind::Boosted) {
            let c = i % map.carriers();
            assert!(c < 5 || c + 5 >= map.carriers(), "{m:?} column {c}");
        }
    }
}

/// "MSC capacity": the multilevel code of ES 201 980 clause 7.3 produces the printed bits per frame, with the DRM rates of
/// protection levels 0 and 1 for 16-QAM and 64-QAM and rate 3/5 for 4-QAM, on the MSC cells of the maps (704 in mode A at 2.5
/// kHz) and the printed cell counts of the maps that the document does not draw. Mode E at 2.5 kHz prints 762 and 1087 for
/// 16-QAM where the code produces 726 and 906: the printed 762 swaps two digits of 726, and 1087 repeats the 64-QAM value above
/// it.
#[test]
fn msc_capacity_follows_the_printed_table() {
    let order = [
        (Qam::Q16, 0),
        (Qam::Q64, 0),
        (Qam::Q4, 1),
        (Qam::Q16, 1),
        (Qam::Q64, 1),
    ];
    for m in HamMode::ALL {
        for wide in [false, true] {
            let cells = if wide == m.drawn_wide() {
                CellMap::drawn(m).count(CellKind::Msc)
            } else {
                MSC_CELLS_PRINTED[m.index()][wide as usize]
            };
            for (i, &(q, p)) in order.iter().enumerate() {
                let got = capacity(q, p, cells).unwrap();
                let printed = CAPACITY_PRINTED[m.index()][wide as usize][i];
                match (m, wide, i) {
                    (HamMode::E, true, 0) => assert_eq!((printed, got), (762, 726)),
                    (HamMode::E, true, 3) => assert_eq!((printed, got), (1087, 906)),
                    _ => assert_eq!(got, printed, "{m:?} wide {wide} {q:?} protection {p}"),
                }
            }
        }
    }
    // No 4-QAM at protection 0 ("High Protection is not supported for QAM4"), and no level beyond 1.
    assert!(capacity(Qam::Q4, 0, 530).is_none());
    assert!(capacity(Qam::Q16, 2, 530).is_none());
    // 3/5 is the only rate of table 27 that produces 627 bits on 530 cells.
    let others = [
        (1, 4),
        (1, 3),
        (2, 5),
        (1, 2),
        (4, 7),
        (2, 3),
        (3, 4),
        (4, 5),
    ];
    for (rx, ry) in others {
        let b = Mlc::eep(Qam::Q4, &[Rate::new(rx, ry)], 530)
            .unwrap()
            .info_bits();
        assert_ne!(b, 627, "{rx}/{ry}");
    }
}

/// WinDRM Docs Release 1.3: the default "B/S/16/0/2.5" sends 2617 bps, the fast mode 4362 bps and the slow mode 997 bps. These are
/// mode B 2.5 kHz 16-QAM protection 0, mode A 2.5 kHz 16-QAM protection 1 and mode E 2.3 kHz 4-QAM protection 1.
#[test]
fn windrm_bit_rates_follow_from_the_capacity() {
    let bps = |m: HamMode, wide: bool, q: Qam, p: u8| {
        let cells = CellMap::drawn(m).count(CellKind::Msc);
        assert_eq!(wide, m.drawn_wide());
        (capacity(q, p, cells).unwrap() as f64 / m.frame_s()).floor() as u32
    };
    assert_eq!(bps(HamMode::B, true, Qam::Q16, 0), 2617);
    assert_eq!(bps(HamMode::A, true, Qam::Q16, 1), 4362);
    assert_eq!(bps(HamMode::E, false, Qam::Q4, 1), 997);
}

/// "FAC Config": 40 bits in the order FRAME-ID (2), occupancy, interleaver depth, MSC mode, protection, audio/data, packet id
/// (2), extended MSC mode, 21 bits of label, a dummy bit and the CRC-8; refused when the CRC is wrong.
#[test]
fn fac_bits_follow_the_fac_config() {
    let v = FacValues {
        long_interleave: 1,
        qam64: 1,
        data: 1,
        qam4: 1,
    };
    let f = FrameInfo {
        frame_id: 2,
        wide: true,
        long_interleave: false,
        qam: Qam::Q64,
        protection: 1,
        content: Content::Data { packet_id: 3 },
        label: *b"N0C",
    };
    let b = f.bits(&v);
    assert_eq!(b.len(), 40);
    assert_eq!(&b[..10], &[1, 0, 1, 0, 1, 1, 1, 1, 1, 0]);
    assert_eq!(&b[10..17], &[1, 0, 0, 1, 1, 1, 0]); // 'N' = 0x4E
    assert_eq!(b[31], 0);
    assert_eq!(FrameInfo::from_bits(&b, &v), Some(f));
    let mut bad = b.clone();
    bad[20] ^= 1;
    assert_eq!(FrameInfo::from_bits(&bad, &v), None);
    // The other bit values read back the same frame.
    let w = FacValues {
        long_interleave: 0,
        qam64: 0,
        data: 0,
        qam4: 0,
    };
    assert_eq!(FrameInfo::from_bits(&f.bits(&w), &w), Some(f));
    let q4 = FrameInfo { qam: Qam::Q4, ..f };
    assert_eq!(FrameInfo::from_bits(&q4.bits(&v), &v), Some(q4));
}

/// No rate of ES 201 980 table 27 puts the 40 FAC bits and the tail on the 45 FAC cells: the FAC code comes from the file.
#[test]
fn no_drm_rate_fits_the_fac() {
    let rates = [
        (1, 6),
        (1, 4),
        (3, 10),
        (1, 3),
        (4, 11),
        (2, 5),
        (1, 2),
        (4, 7),
        (3, 5),
        (2, 3),
        (8, 11),
        (3, 4),
        (4, 5),
        (7, 8),
        (8, 9),
    ];
    for (rx, ry) in rates {
        assert!(Mlc::fac(Rate::new(rx, ry), 40, 45).is_err(), "{rx}/{ry}");
        let eep = Mlc::eep(Qam::Q4, &[Rate::new(rx, ry)], 45).map(|m| m.info_bits());
        assert_ne!(eep.ok(), Some(40), "{rx}/{ry}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The format file

#[test]
fn the_fixture_says_that_it_is_not_the_format() {
    let f = fixture();
    assert!(f.note().contains("NOT the HamDRM format"), "{}", f.note());
    for m in HamMode::ALL {
        for wide in [false, true] {
            let map = f.map(m, wide);
            assert_eq!(map.carriers(), CARRIERS[m.index()][wide as usize]);
            assert_eq!(map.count(CellKind::Fac), 45);
            if wide != m.drawn_wide() {
                assert_eq!(
                    map.count(CellKind::Msc),
                    MSC_CELLS_PRINTED[m.index()][wide as usize]
                );
            }
        }
    }
}

#[test]
fn a_malformed_format_is_refused_naming_the_field() {
    let good = std::fs::read_to_string(fixture_path()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&good).unwrap();
    let refused = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut j = json.clone();
        edit(&mut j);
        HamFormat::from_json(&j.to_string()).expect_err("refused")
    };
    let e = refused(&|j| {
        j["maps"].as_object_mut().unwrap().remove("B/2.3");
    });
    assert!(e.to_string().starts_with("maps.B/2.3"), "{e}");
    let e = refused(&|j| j["maps"]["A/2.5"] = j["maps"]["A/2.3"].clone());
    assert!(e.to_string().starts_with("maps.A/2.5"), "{e}");
    let e = refused(&|j| j["maps"]["E/2.5"][3] = serde_json::json!("x"));
    assert!(e.to_string().starts_with("maps.E/2.5"), "{e}");
    let e = refused(&|j| j["time_phases"]["B"][0][1] = serde_json::json!(1024));
    assert!(e.to_string().starts_with("time_phases.B"), "{e}");
    let e = refused(&|j| {
        j["frequency_phases"]["A"].as_array_mut().unwrap().pop();
    });
    assert!(e.to_string().starts_with("frequency_phases.A"), "{e}");
    let e = refused(&|j| {
        j["gain_phases_e"].as_array_mut().unwrap().pop();
    });
    assert!(e.to_string().starts_with("gain_phases_e"), "{e}");
    let e = refused(&|j| j["fac_puncture"] = serde_json::json!([1, 1, 0, 0, 0, 0]));
    assert!(e.to_string().starts_with("fac_puncture"), "{e}");
    let e = refused(&|j| j["fac_values"]["qam4"] = serde_json::json!(2));
    assert!(e.to_string().starts_with("fac_values.qam4"), "{e}");
    let e = refused(&|j| j["packet_length"] = serde_json::json!(0));
    assert!(e.to_string().starts_with("packet_length"), "{e}");
    let e = refused(&|j| j["application"] = serde_json::json!("ftp"));
    assert!(e.to_string().starts_with("application"), "{e}");
    assert!(HamFormat::from_json("{").is_err());
    // The receiver says so in its field `format`, and a live setting is refused.
    let path = std::env::temp_dir().join("sdrkit-hamdrm-malformed-format.json");
    let mut j = json.clone();
    j["packet_length"] = serde_json::json!(0);
    std::fs::write(&path, j.to_string()).unwrap();
    let p = path.to_string_lossy().to_string();
    let r = feed_with(&[Complex32::default(); 4800], 48e3, 4800, &config(Some(&p)));
    let t = text(&r, hamdrm::HamdrmFields::FORMAT.key);
    assert!(t.starts_with("refused: packet_length"), "{t}");
    let mut rx = hamdrm::make_receiver(&config(None)).unwrap();
    rx.configure(&InputInfo::new(48e3, DIAL, DIAL).recording())
        .unwrap();
    assert!(apply(rx.as_mut(), "hamdrm_format", json!(p.as_str())).is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn without_the_format_nothing_is_decoded_and_the_receiver_says_why() {
    let o = GenOptions {
        transmissions: 1,
        ..default_opts()
    };
    let x = generate(o.clone(), 48e3, seconds_for(&o), &[]);
    let r = feed_with(&x, 48e3, 4800, &config(None));
    let t = text(&r, hamdrm::HamdrmFields::FORMAT.key);
    assert!(t.starts_with("not loaded"), "{t}");
    assert!(
        r.status().summary.contains("format"),
        "{}",
        r.status().summary
    );
    assert_eq!(r.status().lock_state, LockState::Searching);
    assert_eq!(int(&r, "frames_ok"), 0);
    assert!(row_ids(&r, "history").is_empty());
    // The generator without the format sends nothing.
    assert!(HamGenerator::new(GenOptions::default(), 48e3).is_none());
    // With the fixture the same signal decodes, and the field names the fixture.
    let r = feed(&x, 48e3, 4800);
    assert!(text(&r, hamdrm::HamdrmFields::FORMAT.key).starts_with("loaded: Self-check fixture"));
    assert_eq!(newest(&r, "crc32"), want_crc());
}

// ---------------------------------------------------------------------------------------------------------------------------
// Round trips with the fixture

#[test]
fn h1_every_mode_occupancy_and_qam() {
    for mode in HamMode::ALL {
        for wide in [false, true] {
            for (qam, p) in [(Qam::Q4, 1), (Qam::Q16, 0), (Qam::Q64, 0), (Qam::Q16, 1)] {
                if mode != HamMode::B && (qam == Qam::Q4 || p == 1) {
                    continue; // mode B covers 4-QAM and protection 1
                }
                let o = opts(mode, wide, qam, p);
                let r = feed(&generate(o.clone(), 48e3, seconds_for(&o), &[]), 48e3, 4800);
                assert_eq!(
                    newest(&r, "crc32"),
                    want_crc(),
                    "{mode:?} {wide} {qam:?} {p}: {:?}",
                    r.fields()
                );
                assert_eq!(newest(&r, "name"), hamdrm::TEST_NAME);
                assert_eq!(newest(&r, "call"), hamdrm::TEST_CALL);
                assert_eq!(newest(&r, "transmissions"), "1");
                assert_eq!(text(&r, "mode"), mode.letter());
                assert_eq!(num(&r, "occupancy"), if wide { 2500.0 } else { 2300.0 });
                assert_eq!(text(&r, "protection"), p.to_string());
                assert_eq!(text(&r, "label"), hamdrm::TEST_CALL);
            }
        }
    }
}

#[test]
fn h1_long_interleaver() {
    let o = GenOptions {
        long_interleave: true,
        transmissions: 1,
        ..default_opts()
    };
    let r = feed(&generate(o.clone(), 48e3, seconds_for(&o), &[]), 48e3, 4800);
    assert_eq!(newest(&r, "crc32"), want_crc(), "{:?}", r.fields());
    assert_eq!(text(&r, "interleave"), "long");
}

#[test]
fn h2_missing_segments_are_listed_and_filled() {
    let o = GenOptions {
        skip_every: Some(4),
        transmissions: 2,
        ..default_opts()
    };
    let g = HamGenerator::new(o.clone(), 48e3).unwrap();
    let nseg = g.segments();
    assert!(nseg >= 4, "{nseg} segments");
    let frame_s = o.mode.frame_s();
    let one = 0.3 + g.transmission_frames(0) as f64 * frame_s;
    let total = one + 2.0 + g.transmission_frames(1) as f64 * frame_s + 1.5;
    let x = generate(o, 48e3, total, &[]);
    // After the first transmission: the missing list is the left-out segments up to the highest one received.
    let first = feed(&x[..(48e3 * (one + 1.5)) as usize], 48e3, 4800);
    let top = (0..nseg).rev().find(|i| (i + 1) % 4 != 0).unwrap();
    let want: Vec<String> = (0..top)
        .filter(|i| (i + 1) % 4 == 0)
        .map(|i| i.to_string())
        .collect();
    let mut missing = row_ids(&first, "missing");
    missing.reverse();
    assert_eq!(missing.join(" "), want.join(" "), "{:?}", first.fields());
    assert_eq!(int(&first, "files_completed"), 0);
    let all = feed(&x, 48e3, 4800);
    assert_eq!(newest(&all, "crc32"), want_crc(), "{:?}", all.fields());
    assert_eq!(newest(&all, "transmissions"), "2");
}

#[test]
fn h3_tuning_errors() {
    for off in [-50.0, 50.0] {
        let o = GenOptions {
            transmissions: 1,
            ..default_opts()
        };
        let x = generate(
            o.clone(),
            48e3,
            seconds_for(&o),
            &[("gen_offset_hz", json!(off))],
        );
        let r = feed(&x, 48e3, 4800);
        assert!(
            (num(&r, "offset_hz") - off).abs() < 5.0,
            "{off}: {}",
            num(&r, "offset_hz")
        );
        assert_eq!(newest(&r, "crc32"), want_crc());
    }
}

#[test]
fn h4_noise_only() {
    let x = impair(
        Box::new(sdrkit_stack_broadcast::silence::Silent(48e3)),
        6.0,
        &[
            ("gen_noise", json!("floor")),
            ("gen_noise_floor_dbfs", json!(-20.0)),
        ],
    );
    let r = feed(&x, 48e3, 4800);
    assert_eq!(r.status().lock_state, LockState::Searching);
    assert_eq!(int(&r, "frames_ok"), 0);
}

/// H5: nothing is sent for 2 s in the first transmission. The receiver locks again within that transmission, lists the
/// segments it lost, and the second transmission completes the file. Every mode.
#[test]
fn h5_recovery_after_a_gap() {
    for mode in HamMode::ALL {
        let o = GenOptions {
            mode,
            transmissions: 2,
            ..default_opts()
        };
        let g = HamGenerator::new(o.clone(), 48e3).unwrap();
        let frame_s = o.mode.frame_s();
        let one = 0.3 + g.transmission_frames(0) as f64 * frame_s;
        let total = one + 2.0 + g.transmission_frames(1) as f64 * frame_s + 1.5;
        let from = 0.3 + 4.0 * frame_s;
        let x = with_gap(o, total, from, 2.0);
        let before = feed(&x[..(48e3 * from) as usize], 48e3, 4800);
        let first = feed(&x[..(48e3 * (one + 1.0)) as usize], 48e3, 4800);
        assert!(
            int(&first, "frames_ok") > int(&before, "frames_ok") + 2,
            "{mode:?}: {:?}",
            first.fields()
        );
        assert_eq!(
            int(&first, "files_completed"),
            0,
            "{mode:?}: {:?}",
            first.fields()
        );
        assert!(
            !row_ids(&first, "missing").is_empty(),
            "{mode:?}: {:?}",
            first.fields()
        );
        let all = feed(&x, 48e3, 4800);
        assert_eq!(
            newest(&all, "crc32"),
            want_crc(),
            "{mode:?}: {:?}",
            all.fields()
        );
        assert_eq!(newest(&all, "transmissions"), "2", "{mode:?}");
        // The completion time is on the input clock: within the second transmission.
        let id = row_ids(&all, "history")[0].clone();
        let done = cell_num(&all, &HISTORY, &id, "completed");
        assert!(
            done > one + 2.0 && done < total - 1.0,
            "{mode:?}: completed at {done} s"
        );
    }
}

#[test]
fn impairments_and_cut_independence() {
    let o = GenOptions {
        transmissions: 1,
        ..default_opts()
    };
    let x = generate(
        o.clone(),
        48e3,
        seconds_for(&o),
        &[("gen_offset_hz", json!(20.0))],
    );
    let a = feed(&x, 48e3, 1000);
    let b = feed(&x, 48e3, 33_333);
    assert_eq!(newest(&a, "crc32"), want_crc());
    assert_eq!(int(&a, "frames_ok"), int(&b, "frames_ok"));
    assert_eq!(num(&a, "offset_hz"), num(&b, "offset_hz"));
}

#[test]
fn clear_command() {
    let o = GenOptions {
        transmissions: 1,
        ..default_opts()
    };
    let x = generate(o.clone(), 48e3, seconds_for(&o), &[]);
    let mut rx = hamdrm::make_receiver(&config(Some(&fixture_path()))).unwrap();
    rx.configure(&InputInfo::new(48e3, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(4800) {
        rx.feed(c, &mut out);
        out.clear();
    }
    assert_eq!(int(rx.results(), "files_completed"), 1);
    assert!(invoke(rx.as_mut(), "clear", &[]).is_ok());
    assert_eq!(int(rx.results(), "files_completed"), 0);
    assert!(row_ids(rx.results(), "history").is_empty());
    assert!(invoke(rx.as_mut(), "clear", &[("now", json!(true))]).is_err());
}

/// A change of frequency keeps the finished files and their count, and forgets the frame values and the file being
/// received.
#[test]
fn a_change_of_frequency_keeps_the_finished_files() {
    let o = opts(HamMode::B, true, Qam::Q16, 0);
    let x = generate(o.clone(), 48e3, seconds_for(&o), &[]);
    let mut rx = hamdrm::make_receiver(&config(Some(&fixture_path()))).unwrap();
    rx.configure(&InputInfo::new(48e3, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(4800) {
        rx.feed(c, &mut out);
        out.clear();
    }
    assert_eq!(newest(rx.results(), "crc32"), want_crc());
    assert_eq!(int(rx.results(), "files_completed"), 1);
    let frames = int(rx.results(), "frames_ok");
    rx.forget();
    let r = rx.results();
    assert_eq!(newest(r, "crc32"), want_crc());
    assert_eq!(int(r, "files_completed"), 1);
    for key in [
        "mode",
        "qam",
        "label",
        "snr_db",
        "offset_hz",
        "file_id",
        "file_name",
    ] {
        assert!(is_none(r, key), "{key}");
    }
    assert!(row_ids(r, "missing").is_empty());
    assert_eq!(int(r, "frames_ok"), 0);
    assert!(text(r, hamdrm::HamdrmFields::FORMAT.key).starts_with("loaded"));
    // The frame counts start from zero: the same signal again counts as much as the first time.
    for c in x.chunks(4800) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let again = int(rx.results(), "frames_ok");
    assert!(again > 0 && again <= frames + 1, "{again} after {frames}");
}
