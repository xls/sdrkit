//! NOAA DSB: the TIP and HIRS formats, the shared synchroniser and demodulator, and the receiver on its test signal.

use super::common::{run, Ran};
use sdrkit_algo_modem::pm::{PmConfig, PmDemod};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_DRIFT_HZ_S, GEN_ESN0_DB, GEN_LEVEL_DBFS, GEN_NOISE, GEN_OFFSET_HZ,
};
use sdrkit_algo_sim::modulate::PhaseMod;
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig};
use sdrkit_stack_satimg::gen::test_value;
use sdrkit_stack_satimg::instruments::tip::{
    decode_hirs, encode_hirs, hirs_bytes, test_frame, tip_from_hrpt, tip_header, HirsDecoder,
    HIRS_CHANNELS, HIRS_WIDTH,
};
use sdrkit_stack_satimg::modes::dsb::DESCRIPTOR;
use sdrkit_util_bits::line::{NrzM, Nrzi};

fn noise(n: usize, scale: f32) -> Vec<Complex32> {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
    };
    (0..n)
        .map(|_| Complex32::new(next() + next(), next() + next()) * scale)
        .collect()
}

fn random_bits(n: usize) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    (0..n)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 33) as u8 & 1
        })
        .collect()
}

#[test]
fn a_hirs_element_decodes_from_its_36_bytes() {
    let h: [u8; 36] = [
        0x0A, 0x00, 0x0A, 0x20, 0xC9, 0x0C, 0x88, 0x96, 0x46, 0x42, 0x3E, 0x92, 0x58, 0x95, 0xE4,
        0xC8, 0x27, 0x09, 0x3E, 0x8A, 0x26, 0x52, 0xC2, 0xA2, 0x95, 0x78, 0xAE, 0xE5, 0x90, 0x2D,
        0x49, 0x70, 0x8B, 0xB6, 0x5F, 0x42,
    ];
    let e = decode_hirs(&h);
    assert_eq!((e.encoder, e.element, e.valid), (10, 20, true));
    for (w, &v) in e.words.iter().enumerate() {
        assert_eq!(v, 4096 + 100 * (w as u16 + 1));
    }
    assert_eq!(
        HIRS_CHANNELS,
        [1, 17, 2, 3, 13, 4, 18, 11, 19, 7, 8, 20, 10, 14, 6, 5, 15, 12, 16, 9]
    );
    // The coder gives the same bytes but for the parity bit, which makes the count of ones odd.
    let mut values = [0i16; 20];
    for (w, v) in values.iter_mut().enumerate() {
        *v = 100 * (w as i16 + 1);
    }
    let made = encode_hirs(10, 20, true, &values);
    assert_eq!(made[..35], h[..35]);
    assert_eq!(made[35] & 0xFE, h[35] & 0xFE);
    assert!(decode_hirs(&made).parity_ok);
}

#[test]
fn every_hirs_word_is_sign_and_magnitude() {
    let mut values = [0i16; 20];
    values[0] = -100;
    for encoder in [10u8, 60, 156] {
        let e = decode_hirs(&encode_hirs(encoder, 3, true, &values));
        assert_eq!(e.words[0], 3996);
        assert_eq!(e.words[1], 4096);
    }
}

#[test]
fn a_hirs_line_ends_at_element_55() {
    let mut d = HirsDecoder::default();
    let mut strips = Vec::new();
    for e in 0..56u8 {
        assert!(strips.is_empty());
        let f = test_frame(15, u16::from(e), e, 7, test_value);
        assert_eq!(tip_header(&f).spacecraft, 15);
        assert_eq!(tip_header(&f).minor_frame, u16::from(e));
        d.push_frame(&f, &mut strips);
    }
    assert_eq!(strips.len(), 20);
    for s in &strips {
        let w = HIRS_CHANNELS
            .iter()
            .position(|&c| u16::from(c) == s.channel)
            .unwrap();
        for e in 0..HIRS_WIDTH {
            let want = (4096 + test_value(w as u16, e as u32, 7)) >> 5;
            assert_eq!(u16::from(s.samples[e]), want);
        }
    }
    // The retrace elements are not valid and give nothing.
    for e in 56..64u8 {
        d.push_frame(&test_frame(15, 0, e, 7, test_value), &mut strips);
    }
    assert_eq!(strips.len(), 20);
    assert_eq!(d.stats.rows, 1);
}

#[test]
fn the_minor_frame_counter_has_nine_bits() {
    let f = test_frame(7, 319, 0, 0, test_value);
    assert_eq!(
        (f[0], f[1], f[2] & 15, f[4] & 1, f[5]),
        (0xED, 0xE2, 7, 1, 0x3F)
    );
    assert_eq!(tip_header(&f).minor_frame, 319);
    assert_eq!(hirs_bytes(&f)[0], 1);
}

#[test]
fn tip_frames_come_out_of_an_hrpt_minor_frame() {
    let words: Vec<u16> = (0..11_090u32).map(|i| ((i * 7) % 1024) as u16).collect();
    let t = tip_from_hrpt(&words).unwrap();
    assert_eq!(t[0][0], (words[103] >> 2) as u8);
    assert_eq!(t[4][103], (words[622] >> 2) as u8);
    assert!(tip_from_hrpt(&words[..600]).is_none());
}

/// A stream of `n` frames of `cfg` with the payload bit pattern of `seed`; `spoil(k)` destroys the sync word of frame `k`.
fn frames(
    cfg: &WordSyncConfig,
    n: usize,
    spoil: impl Fn(usize) -> bool,
    invert_from: usize,
) -> Vec<u8> {
    let payload = random_bits(cfg.frame_bits - cfg.word_bits as usize);
    let mut bits = Vec::new();
    for k in 0..n {
        let flip = u8::from(k >= invert_from);
        for i in (0..cfg.word_bits).rev() {
            let b = ((cfg.word >> i) & 1) as u8;
            bits.push(if spoil(k) { (i % 2) as u8 } else { b } ^ flip);
        }
        bits.extend(payload.iter().map(|b| b ^ flip));
    }
    bits
}

const ODD: WordSyncConfig = WordSyncConfig {
    word: 0x5A0F_BE66,
    word_bits: 32,
    frame_bits: 1001,
    acquire_errors: 3,
    track_errors: 10,
    complement: true,
    max_misses: 3,
};

#[test]
fn the_synchroniser_confirms_a_lock_before_it_gives_frames() {
    let mut s = WordSync::new(ODD).unwrap();
    let mut n = 0;
    s.push_bits(&random_bits(20_000_000), |_| n += 1);
    assert_eq!(n, 0);
    // Every second sync word destroyed: a new lock is never confirmed.
    let mut s = WordSync::new(ODD).unwrap();
    s.push_bits(&frames(&ODD, 40, |k| k % 2 == 1, usize::MAX), |_| n += 1);
    assert_eq!(n, 0);
    // A confirmed lock goes on through the same stream, with one miss for each second frame.
    let mut bits = frames(&ODD, 3, |_| false, usize::MAX);
    bits.extend(frames(&ODD, 20, |k| k % 2 == 1, usize::MAX));
    let mut s = WordSync::new(ODD).unwrap();
    let mut missed = 0;
    s.push_bits(&bits, |f| {
        n += 1;
        missed += usize::from(f.missed);
        assert_eq!(f.bytes.len(), 126);
        assert_eq!(f.bytes[..4], [0x5A, 0x0F, 0xBE, 0x66]);
    });
    assert_eq!((n, missed), (23, 10));
    assert!(s.locked());
}

#[test]
fn the_synchroniser_follows_a_slip_and_ends_after_four_misses() {
    // One missed sync word: its frame is still given.
    let mut s = WordSync::new(ODD).unwrap();
    let mut got = Vec::new();
    s.push_bits(&frames(&ODD, 8, |k| k == 4, usize::MAX), |f| {
        got.push(f.missed)
    });
    assert_eq!(got, [false, false, false, false, true, false, false, false]);
    // A half-turn slip of the carrier while tracking: no frame is lost, and the bits stay upright.
    let upright = frames(&ODD, 1, |_| false, usize::MAX);
    let mut s = WordSync::new(ODD).unwrap();
    let mut n = 0;
    s.push_bits(&frames(&ODD, 10, |_| false, 5), |f| {
        n += 1;
        let bits: Vec<u8> = (0..ODD.frame_bits)
            .map(|i| (f.bytes[i / 8] >> (7 - i % 8)) & 1)
            .collect();
        assert_eq!(bits, upright);
    });
    assert_eq!(n, 10);
    assert!(s.inverted());
    // Four misses in a row end the lock.
    let mut s = WordSync::new(ODD).unwrap();
    s.push_bits(
        &frames(&ODD, 12, |k| (4..8).contains(&k), usize::MAX),
        |_| {},
    );
    assert_eq!(s.stats().losses, 1);
    let mut s = WordSync::new(ODD).unwrap();
    s.push_bits(
        &frames(&ODD, 12, |k| (4..7).contains(&k), usize::MAX),
        |_| {},
    );
    assert_eq!(s.stats().losses, 0);
}

#[test]
fn nrz_s_and_nrz_m_decode_their_levels() {
    let mut levels = [0u8, 1, 1, 0, 0, 0, 1];
    NrzM::new(0).decode(&mut levels);
    assert_eq!(levels, [0, 1, 0, 1, 0, 0, 1]);
    let mut levels = [0u8, 1, 1, 0, 0, 0, 1];
    Nrzi::new(0).decode(&mut levels);
    assert_eq!(levels, [1, 0, 1, 0, 1, 1, 0]);
}

/// The lock value of the demodulator after `secs` of a clean PM signal: split-phase or NRZ random data.
fn pm_lock(split: bool, index_deg: f64, roll_off: f64) -> f32 {
    let (rate, symbols) = (64_000.0, 16_000.0);
    let mut m = PhaseMod::new(roll_off, 12.0, rate / symbols, index_deg.to_radians()).unwrap();
    let data = random_bits(40_000);
    let chips: Vec<u8> = if split {
        data.iter().take(20_000).flat_map(|&b| [b, b ^ 1]).collect()
    } else {
        data
    };
    let mut x = Vec::new();
    m.push(&chips, &mut x);
    let mut d = PmDemod::new(PmConfig::new(rate, symbols, roll_off)).unwrap();
    let mut soft = Vec::new();
    d.process(&x, &mut soft);
    assert!(d.tracking());
    d.lock_value()
}

#[test]
fn the_pm_lock_value_tells_a_carrier_from_noise() {
    let mut d = PmDemod::new(PmConfig::new(64_000.0, 16_000.0, 1.0)).unwrap();
    let mut soft = Vec::new();
    d.process(&noise(64_000 + 32_768, 0.5), &mut soft);
    assert!(d.lock_value() < 0.01, "{}", d.lock_value());
    let l = pm_lock(true, 67.5, 1.0);
    assert!((l - 0.17).abs() < 0.04, "{l}");
    let l = pm_lock(false, 60.0, 0.6);
    assert!((l - 0.26).abs() < 0.05, "{l}");
}

#[test]
fn the_satellite_list_is_a_plan_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/weather_satellites.csv");
    let text = std::fs::read_to_string(path).expect("the data file");
    let plan = sdrkit_data_plans::io::from_csv(&text, "x", "X").expect("a plan");
    assert_eq!(plan.id, "weather_satellites");
    assert_eq!(plan.channels.len(), 21);
    assert_eq!(
        plan.modes,
        [
            "noaa_apt",
            "noaa_dsb",
            "goes_gvar",
            "aws",
            "noaa_gac",
            "hrpt",
            "msg_raw",
            "goes_cda",
            "goes_mdl",
            "elektro_l"
        ]
    );
}

fn go(secs: f64, rate: f64, chip_db: f64, offset_hz: f64, drift: f64, ppm: f64) -> Ran {
    let h = Harness::new(&DESCRIPTOR)
        .rate(rate)
        .seconds(secs)
        .set(&GEN_LEVEL_DBFS, -13.0)
        .set(&GEN_NOISE, NoiseKind::EsN0)
        .set(&GEN_ESN0_DB, chip_db)
        .set(&GEN_OFFSET_HZ, offset_hz)
        .set(&GEN_DRIFT_HZ_S, drift)
        .set(&GEN_CLOCK_PPM, ppm);
    let x = h.generate().expect("a signal");
    run::<Strip>(&h, &x, rate, 137.77e6, 137.77e6, 65_536, Some("tip.out"))
}

#[test]
fn the_beacon_gives_frames_and_a_hirs_line() {
    let r = go(13.0, 192e3, 18.0, 1500.0, 15.0, 50.0);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("tip_frames") >= 100.0, "{}", r.num("tip_frames"));
    assert!(r.num("lines") >= 2.0);
    assert_eq!(r.num("spacecraft"), 15.0);
    assert!(
        (r.num("offset_hz") - 1695.0).abs() < 300.0,
        "{}",
        r.num("offset_hz")
    );
    // The last line is a whole line of the test picture.
    let strips = r.of::<Strip>();
    let row = strips.iter().map(|s| s.first_row).max().unwrap();
    let ch1: Vec<&Strip> = strips
        .iter()
        .filter(|s| s.first_row == row && s.channel == 1)
        .collect();
    assert_eq!(ch1.len(), 1);
    let fits = |y: u32| {
        (0..HIRS_WIDTH)
            .all(|e| u16::from(ch1[0].samples[e]) == (4096 + test_value(0, e as u32, y)) >> 5)
    };
    assert!((0..400).any(fits));
}

#[test]
fn noise_gives_no_frame() {
    let h = Harness::new(&DESCRIPTOR).rate(96e3);
    let x = noise((30.0 * 96e3) as usize, 0.2);
    let r = run::<Strip>(&h, &x, 96e3, 137.77e6, 137.77e6, 65_536, Some("tip.out"));
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.num("tip_frames"), 0.0);
    assert!(r.of::<Strip>().is_empty());
}

#[test]
fn the_lock_ends_in_noise_and_comes_back_with_the_signal() {
    let rate = 96e3;
    let h = Harness::new(&DESCRIPTOR)
        .rate(rate)
        .seconds(5.0)
        .set(&GEN_LEVEL_DBFS, -13.0);
    let x = h.generate().expect("a signal");
    let r = run::<Strip>(&h, &x, rate, 137.77e6, 137.77e6, 65_536, None);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    let frames = r.num("tip_frames");
    let r = super::common::feed::<Strip>(r.rx, &noise((3.5 * rate) as usize, 0.2), 65_536, None);
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    // The frames behind the first four missed sync words are still given: then the lock ends.
    assert!(r.num("tip_frames") <= frames + 4.0);
    let frames = r.num("tip_frames");
    let r = super::common::feed::<Strip>(r.rx, &x[..(4.0 * rate) as usize], 65_536, None);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("tip_frames") > frames + 10.0);
}

#[test]
fn the_satellite_list_passes_the_plan_import() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/weather_satellites.csv");
    let text = std::fs::read_to_string(path).expect("the data file");
    let mut plans = sdrkit_data_plans::Plans::default();
    let ids = sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Csv,
        "x",
        "X",
    )
    .expect("the import");
    assert_eq!(ids, ["weather_satellites"]);
}
