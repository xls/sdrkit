//! S-band CCSDS: the derived frame sizes, the data file of links, the code (255,239), and each link of the data file on its
//! test signal, with impairments for one link of each kind.

use super::common::{assert_declared, run, set, table_rows, Ran};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_DRIFT_HZ_S, GEN_ESN0_DB, GEN_INVERTED, GEN_LEVEL_DBFS, GEN_NOISE,
    GEN_OFFSET_HZ,
};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Choice, InputInfo, LockState, SettingSet, Values};
use sdrkit_dev_test::Harness;
use sdrkit_graph::Component;
use sdrkit_stack_satimg::components::table::{Check, FrameBytes, FrameTableComponent};
use sdrkit_stack_satimg::modes::sband::{
    test_frame, Randomiser, RsChoice, SbandLink, SbandModulation, SbandSetting, CONV,
    DESCRIPTOR, FRAME_BITS, LINK_NAME, MODULATION, NRZM, RANDOMISER, ROLL_OFF, RS, RS_BASIS,
    RS_INTERLEAVE, SYMBOL_RATE, SYNC_WORD,
};
use sdrkit_stack_satimg::modes::xrit::RsBasis;
use sdrkit_util_bits::xorshift::Xorshift64;

/// One row of the data file: the name, then the settings of the link.
struct Row {
    name: String,
    settings: Vec<CaseSetting>,
    link: SbandLink,
}

fn choice<C: Choice>(key: &str) -> C {
    let i = C::CHOICES
        .iter()
        .position(|c| c.key == key)
        .unwrap_or_else(|| panic!("no choice {key}"));
    C::from_index(i as u16).unwrap()
}

/// The rows of `data/sband_links.csv`.
fn rows() -> Vec<Row> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/sband_links.csv");
    let text = std::fs::read_to_string(path).expect("the data file");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let head: Vec<&str> = lines.next().expect("a heading").split(',').collect();
    assert_eq!(
        head,
        [
            "link_name",
            "centre_mhz",
            "modulation",
            "symbol_rate",
            "roll_off",
            "conv",
            "frame_bits",
            "sync_word",
            "nrzm",
            "randomiser",
            "rs",
            "rs_interleave",
            "rs_basis"
        ]
    );
    let schema = SbandSetting::schema();
    lines
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            assert_eq!(c.len(), head.len(), "{l}");
            assert!(c[1].is_empty() || c[1].parse::<f64>().is_ok(), "{l}");
            let settings = vec![
                CaseSetting::new(&LINK_NAME, c[0]),
                CaseSetting::new(&MODULATION, choice::<SbandModulation>(c[2])),
                CaseSetting::new(&SYMBOL_RATE, c[3].parse().unwrap()),
                CaseSetting::new(&ROLL_OFF, c[4].parse().unwrap()),
                CaseSetting::new(&CONV, c[5] == "true"),
                CaseSetting::new(&FRAME_BITS, c[6].parse().unwrap()),
                CaseSetting::new(&SYNC_WORD, c[7]),
                CaseSetting::new(&NRZM, c[8] == "true"),
                CaseSetting::new(&RANDOMISER, choice::<Randomiser>(c[9])),
                CaseSetting::new(&RS, choice::<RsChoice>(c[10])),
                CaseSetting::new(&RS_INTERLEAVE, c[11].parse().unwrap()),
                CaseSetting::new(&RS_BASIS, choice::<RsBasis>(c[12])),
            ];
            let mut v = Values::defaults(&schema);
            CaseSetting::apply_all(&settings, &mut v).expect("the settings of the row");
            let link = SbandLink::from_values(&v).expect("a link");
            Row {
                name: c[0].to_string(),
                settings,
                link,
            }
        })
        .collect()
}

fn row(name: &str) -> Row {
    rows()
        .into_iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row {name}"))
}

#[test]
fn the_data_file_has_46_links() {
    let r = rows();
    assert_eq!(r.len(), 46);
    let coded = r.iter().filter(|r| r.link.conv).count();
    assert!(coded > 0 && coded < 46);
}

#[test]
fn frame_sizes_follow_from_the_frame_length() {
    for (name, n, fill, depth) in [
        ("AIM Dump", 248, 7, 5),
        ("DSCOVR TLM Link", 252, 3, 5),
        ("SciSat-1 Dump", 239, 16, 5),
        ("Crew Dragon S-Band TLM", 246, 9, 1),
    ] {
        let l = row(name).link;
        assert_eq!(
            (l.word_len(), l.virtual_fill(), l.interleave),
            (n, fill, depth),
            "{name}"
        );
        assert_eq!(4 + depth * n, l.frame_bytes(), "{name}");
    }
    let l = row("Jason-3 S-Band Link").link;
    assert_eq!((l.check_bytes(), l.data_bytes()), (16, 5 * 239));
}

/// The frame table of `link`, configured.
fn table(link: &SbandLink) -> FrameTableComponent {
    let mut c = FrameTableComponent::new(link.table());
    c.configure(&[]).expect("the component");
    c
}

#[test]
fn a_255_239_word_corrects_8_bytes() {
    let link = row("SMAP S-Band Link").link;
    let f = test_frame(&link, 7, &mut Xorshift64::new(3));
    assert_eq!(f.len(), 1279);
    let spoil = |n: usize| {
        let mut g = f.clone();
        // Byte 4 + i belongs to code word i mod 5: n errors in each.
        for w in 0..5 {
            for j in 0..n {
                g[4 + w + 5 * (2 + 20 * j)] ^= 0x5A;
            }
        }
        g
    };
    let mut c = table(&link);
    let (row8, good) = c.push_frame(&spoil(8), 0.0);
    assert_eq!(row8.check, Check::Good);
    let good = good.unwrap();
    assert_eq!(good.len(), 4 + 5 * 239);
    assert_eq!(good[4..9], [0x44, 0x85, 0, 0, 7]);
    let (row9, good) = c.push_frame(&spoil(9), 0.0);
    assert_eq!((row9.check, good), (Check::Failed, None));
}

#[test]
fn the_randomiser_inside_the_code_is_removed_after_it() {
    let link = row("Falcon 9 S-Band TLM").link;
    let f = test_frame(&link, 0x0A0B0C, &mut Xorshift64::new(1));
    let (r, good) = table(&link).push_frame(&f, 0.0);
    assert_eq!(r.check, Check::Good);
    assert_eq!(good.unwrap()[4..9], [0x44, 0x85, 0x0A, 0x0B, 0x0C]);
    // On the air the data bytes are randomised: FF 48 over 44 85.
    assert_eq!(f[4..6], [0x44 ^ 0xFF, 0x85 ^ 0x48]);
}

/// The seconds a link has to give 3 good frames: 14 frame periods, at least 0.3 s and at most 2 s, and 0.05 s more.
fn window(l: &SbandLink) -> f64 {
    (14.0 * l.frame_period_s()).clamp(0.3, 2.0) + 0.05
}

/// The impairments of a run.
#[derive(Clone, Copy, Default)]
struct Imp {
    offset_hz: f64,
    clock_ppm: f64,
    drift_hz_s: f64,
    inverted: bool,
}

/// Runs the link of `r` for `secs` at Es/N0 `esn0` dB with the impairments `imp`, at 4 samples a symbol and 500 ksps at
/// the least.
fn go(r: &Row, secs: f64, esn0: f64, imp: Imp) -> Ran {
    let rate = (4.0 * r.link.symbol_rate).max(500e3);
    let h = Harness::new(&DESCRIPTOR)
        .with_settings(&r.settings)
        .rate(rate)
        .seconds(secs)
        .set(&GEN_LEVEL_DBFS, -13.0)
        .set(&GEN_NOISE, NoiseKind::EsN0)
        .set(&GEN_ESN0_DB, esn0)
        .set(&GEN_OFFSET_HZ, imp.offset_hz)
        .set(&GEN_CLOCK_PPM, imp.clock_ppm)
        .set(&GEN_DRIFT_HZ_S, imp.drift_hz_s)
        .set(&GEN_INVERTED, imp.inverted);
    let x = h.generate().expect("a signal");
    let piece = ((rate * 0.01) as usize).max(4096);
    run::<FrameBytes>(&h, &x, rate, 2215e6, 2215e6, piece, Some("frames.frames"))
}

/// Every good frame is a test frame of the link: 44 85, then a counter that rises by one.
fn check_frames(r: &Row, ran: &Ran, at_least: usize, what: &str) {
    let frames = ran.of::<FrameBytes>();
    assert!(
        frames.len() >= at_least,
        "{} {what}: {} frames ({} bad), state {:?}, {}",
        r.name,
        frames.len(),
        ran.num("frames_bad"),
        ran.results.status().lock_state,
        ran.results.status().summary
    );
    let len = 4 + r.link.data_bytes();
    for f in &frames {
        assert_eq!(f.bytes.len(), len, "{}", r.name);
        assert_eq!(f.bytes[..4], r.link.sync_word.to_be_bytes(), "{}", r.name);
        assert_eq!(f.bytes[4..6], [0x44, 0x85], "{} {what}", r.name);
    }
    let counts: Vec<u32> = frames
        .iter()
        .map(|f| u32::from_be_bytes([0, f.bytes[6], f.bytes[7], f.bytes[8]]))
        .collect();
    assert!(
        counts.windows(2).all(|w| w[1] > w[0]),
        "{} {what}: {counts:?}",
        r.name
    );
}

/// Each link of the data file in turn, at Es/N0 10 dB when coded and 15 dB when not, 0.002 of the symbol rate off.
fn every_link(part: usize, of: usize) {
    for (i, r) in rows().iter().enumerate() {
        if i % of != part {
            continue;
        }
        let esn0 = if r.link.conv { 10.0 } else { 15.0 };
        let imp = Imp {
            offset_hz: 0.002 * r.link.symbol_rate,
            ..Imp::default()
        };
        let ran = go(r, window(&r.link), esn0, imp);
        check_frames(r, &ran, 3, "in its window");
        assert_eq!(
            (ran.num("spacecraft"), ran.num("vcid")),
            (18.0, 5.0),
            "{}",
            r.name
        );
    }
}

#[test]
fn every_link_of_the_data_file_gives_frames_1() {
    every_link(0, 4);
}

#[test]
fn every_link_of_the_data_file_gives_frames_2() {
    every_link(1, 4);
}

#[test]
fn every_link_of_the_data_file_gives_frames_3() {
    every_link(2, 4);
}

#[test]
fn every_link_of_the_data_file_gives_frames_4() {
    every_link(3, 4);
}

/// One link of each kind with its Es/N0 floor, dB.
const KINDS: [(&str, f64); 5] = [
    ("FORMOSAT-5 S-Band dump", 9.0),
    ("Jason-3 S-Band Link", 7.0),
    ("SWOT S-Band TLM", 7.0),
    ("Hinode S-Band dump", 9.0),
    ("Falcon 9 S-Band TLM", 12.0),
];

/// The seconds of a run with impairments: 20 frame periods, at least 0.5 s and at most 2.5 s.
fn long(l: &SbandLink) -> f64 {
    (20.0 * l.frame_period_s()).clamp(0.5, 2.5)
}

fn kind_with(k: usize) {
    let (name, floor) = KINDS[k];
    let r = row(name);
    let rs = r.link.symbol_rate;
    let psk = matches!(
        r.link.modulation,
        SbandModulation::Bpsk | SbandModulation::Qpsk | SbandModulation::Oqpsk
    );
    let mut cases = vec![
        (
            "+0.5 % offset",
            Imp {
                offset_hz: 0.005 * rs,
                ..Imp::default()
            },
        ),
        (
            "-1 % offset",
            Imp {
                offset_hz: -0.01 * rs,
                ..Imp::default()
            },
        ),
        (
            "+50 ppm clock",
            Imp {
                clock_ppm: 50.0,
                ..Imp::default()
            },
        ),
        (
            "500 Hz/s drift",
            Imp {
                drift_hz_s: 500.0,
                ..Imp::default()
            },
        ),
        (
            "0.15 Rs off centre",
            Imp {
                offset_hz: 0.15 * rs,
                ..Imp::default()
            },
        ),
    ];
    if psk {
        cases.push((
            "inverted",
            Imp {
                inverted: true,
                ..Imp::default()
            },
        ));
    }
    for (what, imp) in cases {
        let ran = go(&r, long(&r.link), floor + 8.0, imp);
        check_frames(&r, &ran, 3, what);
    }
    let ran = go(
        &r,
        long(&r.link),
        floor,
        Imp {
            offset_hz: 0.001 * rs,
            ..Imp::default()
        },
    );
    check_frames(&r, &ran, 3, "at the floor");
}

#[test]
fn formosat_5_bpsk_with_impairments() {
    kind_with(0);
}

#[test]
fn jason_3_coded_qpsk_with_impairments() {
    kind_with(1);
}

#[test]
fn swot_coded_oqpsk_with_impairments() {
    kind_with(2);
}

#[test]
fn hinode_pm_with_impairments() {
    kind_with(3);
}

#[test]
fn falcon_9_fsk_with_impairments() {
    kind_with(4);
}

#[test]
fn the_defaults_lock_and_publish_what_they_declare() {
    let h = Harness::new(&DESCRIPTOR)
        .rate(4e6)
        .seconds(0.6)
        .set(&GEN_LEVEL_DBFS, -13.0);
    let x = h.generate().expect("a signal");
    let ran = run::<FrameBytes>(&h, &x, 4e6, 2215e6, 2215e6, 65_536, Some("frames.frames"));
    assert_eq!(ran.results.status().lock_state, LockState::Locked);
    assert!(ran.results.status().summary.starts_with("S-band link, "));
    assert!(ran.num("frames_ok") >= 20.0, "{}", ran.num("frames_ok"));
    assert_eq!(ran.text("link"), "S-band link");
    assert!(ran
        .text("note")
        .to_lowercase()
        .contains("not confirmed by a public document"));
    assert_declared(&ran, &sdrkit_stack_satimg::typed::shape("sband_ccsds").results_decl());
    // Clear empties the tables; the synchronisation stays.
    let mut rx = ran.rx;
    super::common::invoke(&mut *rx, "clear", &[]).expect("the command");
    let r = super::common::feed::<FrameBytes>(rx, &x[..400_000], 65_536, None);
    let rows = table_rows(&r.results);
    assert!(rows.iter().filter(|(t, _, _)| t == "frames").count() < 20);
}

#[test]
fn noise_gives_no_frame() {
    for name in [
        "FORMOSAT-5 S-Band dump",
        "Jason-3 S-Band Link",
        "Hinode S-Band dump",
    ] {
        let r = row(name);
        let ran = go(&r, 1.0, -30.0, Imp::default());
        assert_eq!(ran.num("frames_ok"), 0.0, "{name}");
        assert!(
            matches!(
                ran.results.status().lock_state,
                LockState::Searching | LockState::Tracking
            ),
            "{name}"
        );
        assert!(ran.num("frame_lock") == 0.0, "{name}");
    }
}

#[test]
fn a_rate_below_the_least_is_refused() {
    let mut rx = Harness::new(&DESCRIPTOR).receiver().unwrap();
    assert!(rx
        .configure(&InputInfo::new(2.5e6, 2215e6, 2215e6).recording())
        .is_err());
    assert!(rx
        .configure(&InputInfo::new(2.8e6, 2215e6, 2215e6).recording())
        .is_ok());
}

#[test]
fn a_bad_sync_word_is_refused() {
    let h = Harness::new(&DESCRIPTOR).set(&SYNC_WORD, "1ACFFC1");
    assert!(h.receiver().is_err());
    let mut rx = Harness::new(&DESCRIPTOR).receiver().unwrap();
    assert!(set(&mut *rx, "sync_word", "XYZ").is_err());
    assert!(set(&mut *rx, "sync_word", "04B3E375").is_ok());
}
