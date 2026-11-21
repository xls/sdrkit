//! `picture_burst` through the transmitter, the test channel and the receiver (Part E section 12 and Part A section 8).

mod common;

use common::{resample, Noise};
use sdrkit_core::Complex32;
use sdrkit_stack_custom::picture_burst::rx::{decode, psnr};
use sdrkit_stack_custom::picture_burst::{burst, data_symbols, header_bits, make, parse_header, slots, test_picture, FS, HEADER_BITS};
use std::f64::consts::TAU;
use std::sync::OnceLock;

/// The test channel: clock error, lead and tail, carrier, noise at an SNR in 190 kHz.
fn channel(x: &[Complex32], snr_db: f64, cfo: f64, ppm: f64, lead: usize, tail: usize, seed: u64) -> Vec<Complex32> {
    // Sample i of the output is the burst at i / (1 + e), to first order in e.
    let c = resample(x, -ppm, 0.0);
    let mut y = vec![Complex32::new(0.0, 0.0); lead];
    y.extend(c);
    y.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), tail));
    for (i, v) in y.iter_mut().enumerate() {
        let a = TAU * (cfo * i as f64 / FS).fract() + 0.7;
        *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
    }
    Noise::new(seed).add(&mut y, FS / 190_000.0 / 10f64.powf(snr_db / 10.0));
    y
}

fn big() -> &'static (Vec<u8>, Vec<Complex32>) {
    static B: OnceLock<(Vec<u8>, Vec<Complex32>)> = OnceLock::new();
    B.get_or_init(|| {
        let pic = test_picture(1920, 1080);
        let m = make(&pic, 1920, 1080, 0, "LY1ABC").unwrap();
        (pic, m.samples)
    })
}

struct Case {
    snr: f64,
    cfo: f64,
    ppm: f64,
    lead: usize,
    layers: usize,
    psnr: f64,
}

fn run(c: &Case, seed: u64) -> sdrkit_stack_custom::picture_burst::rx::Decoded {
    let (pic, x) = big();
    let y = channel(x, c.snr, c.cfo, c.ppm, c.lead, 200_000, seed);
    let d = decode(&y).expect("a burst");
    assert_eq!(d.header.callsign, "LY1ABC");
    assert!(d.layers_complete >= c.layers, "{} layers, {:?}", d.layers_complete, d.good);
    let p = psnr(pic, d.picture.as_ref().expect("a picture"));
    assert!(p >= c.psnr, "PSNR {p}");
    d
}

#[test]
fn burst_length_power_and_a_clean_decode() {
    let pic = test_picture(320, 180);
    let m = make(&pic, 320, 180, 0, "LY1TST").unwrap();
    assert_eq!(m.samples.len(), 6_315_508);
    let p = common::power(&m.samples);
    assert!((0.85..1.15).contains(&p), "{p}");
    let d = decode(&m.samples).unwrap();
    assert_eq!(d.layers_complete, 8);
    assert!(d.cfo_hz.abs() < 5.0 && d.clock_ppm.unwrap().abs() < 1.0);
    assert!(psnr(&pic, d.picture.as_ref().unwrap()) > 40.0);
}

#[test]
fn main_case() {
    let d = run(&Case { snr: 8.0, cfo: 3100.0, ppm: 12.0, lead: 442_000, layers: 7, psnr: 36.0 }, 1);
    assert!((d.snr_db - 8.0).abs() < 1.0, "{}", d.snr_db);
    assert!((d.cfo_hz - 3100.0).abs() < 20.0, "{}", d.cfo_hz);
    assert!((d.clock_ppm.unwrap() - 12.0).abs() < 1.0, "{:?}", d.clock_ppm);
}

#[test]
fn case_a_on_the_first_sample() {
    run(&Case { snr: 8.0, cfo: -9700.0, ppm: -18.0, lead: 0, layers: 7, psnr: 36.0 }, 2);
}

#[test]
fn case_b() {
    run(&Case { snr: 8.0, cfo: 9700.0, ppm: 18.0, lead: 123_457, layers: 7, psnr: 36.0 }, 3);
}

#[test]
fn clock_errors_of_50_ppm() {
    run(&Case { snr: 8.0, cfo: 2000.0, ppm: 50.0, lead: 20_000, layers: 7, psnr: 36.0 }, 4);
    run(&Case { snr: 8.0, cfo: -2000.0, ppm: -50.0, lead: 20_000, layers: 7, psnr: 36.0 }, 5);
}

#[test]
fn case_c_at_minus_8_db() {
    run(&Case { snr: -8.0, cfo: 4000.0, ppm: 7.0, lead: 31_000, layers: 2, psnr: 19.0 }, 6);
}

#[test]
fn case_d_at_minus_12_db() {
    run(&Case { snr: -12.0, cfo: -1200.0, ppm: -3.0, lead: 5000, layers: 1, psnr: 17.0 }, 7);
}

#[test]
fn profiles_1_to_3() {
    let pic = test_picture(1920, 1080);
    for (p, snr, cfo, ppm, layers, min_psnr) in [(1, 3.0, 800.0, 2.0, 5, 25.0), (2, 26.0, -2500.0, 4.0, 8, 40.0), (3, 8.0, 6000.0, -9.0, 4, 42.0)] {
        let m = make(&pic, 1920, 1080, p, "DL2XYZ/P").unwrap();
        let y = channel(&m.samples, snr, cfo, ppm, 40_000, 200_000, 10 + p as u64);
        let d = decode(&y).expect("a burst");
        assert_eq!(d.header.callsign, "DL2XYZ/P");
        assert!(d.layers_complete >= layers, "profile {p}: {:?}", d.good);
        let q = psnr(&pic, d.picture.as_ref().unwrap());
        assert!(q >= min_psnr, "profile {p}: PSNR {q}");
    }
}

#[test]
fn no_sync_on_noise() {
    let mut x = vec![Complex32::new(0.0, 0.0); 2_000_000];
    Noise::new(9).add(&mut x, 1.0);
    assert!(decode(&x).is_none());
}

/// The recorded bursts of section 13.1 in both sample forms (profile 0, the 1920 x 1080 test picture, LY1TST, SNR +20, +8
/// and -3 dB in 190 kHz): the header, at least 8, 7 and 4 layers, and a PSNR against the codec's input picture of at least
/// 46.75, 40.45 and 25.05 dB.
#[test]
fn the_recorded_bursts() {
    let Some(root) = common::recordings() else { return };
    let dir = common::folder_with(&root, "_P0_m3dB_cf32.sigmf-data");
    let (_, _, pic) = common::read_ppm(&common::recording(&common::folder_with(&root.join("codecs"), "_1920x1080_input.ppm"), "_1920x1080_input.ppm"));
    for (stem, layers, min_psnr) in [("_P0_20dB", 8, 46.75), ("_P0_8dB", 7, 40.45), ("_P0_m3dB", 4, 25.05)] {
        for (name, rate, y) in common::both_forms(&dir, stem) {
            assert_eq!(rate, FS, "{name}");
            let d = decode(&y).unwrap_or_else(|| panic!("{name}: no burst"));
            assert_eq!(d.header.callsign, "LY1TST", "{name}");
            let q = d.picture.as_ref().map_or(0.0, |p| psnr(&pic, p));
            assert!(d.layers_complete >= layers, "{name}: {} layers, {:?}", d.layers_complete, d.good);
            assert!(q >= min_psnr, "{name}: PSNR {q:.1} dB");
        }
    }
}

/// The clean bursts and slot vectors of section 13.2 (profiles 0 and 3, 320 x 180, LY1TST): the header bits, the slot
/// symbols made from the stream file (to 1e-4), the burst (to 1e-5), and a decode of the clean burst with every layer
/// complete and the stream equal to the file.
#[test]
fn the_slot_vectors() {
    let Some(root) = common::recordings() else { return };
    let dir = common::folder_with(&root, "_320x180_input.ppm");
    for (p, hex) in [
        (0, "13197144D4000002800B4B6200E040180C02070002C801B2C078E05550358000EA70DD5340"),
        (3, "13197144D4000002800B4B6D00E0100808800B38580000000000000000000000EA70D62280"),
    ] {
        let bits: Vec<u8> = (0..HEADER_BITS).map(|i| (u8::from_str_radix(&hex[i / 4..i / 4 + 1], 16).unwrap() >> (3 - i % 4)) & 1).collect();
        let h = parse_header(&bits).unwrap_or_else(|| panic!("profile {p}: header"));
        assert_eq!(h.callsign, "LY1TST");
        assert_eq!(header_bits(&h), bits, "profile {p}: header bits");
        let stream = std::fs::read(common::recording(&dir, &format!("_P{p}_320x180_LY1TST_stream.{}", dir.file_name().unwrap().to_string_lossy().to_lowercase()))).unwrap();
        let s = slots(&data_symbols(&h, &stream));
        let want = common::read_cf32(&common::recording(&dir, &format!("_P{p}_320x180_LY1TST_slots_cf32.raw")));
        assert_eq!(s.len(), want.len(), "profile {p}: slot count");
        let e = s.iter().zip(&want).map(|(a, b)| (a - b).norm()).fold(0f32, f32::max);
        assert!(e < 1e-4, "profile {p}: slot error {e}");
        let x = burst(&s);
        let want = common::read_cf32(&common::recording(&dir, &format!("_P{p}_320x180_LY1TST_clean_632k_cf32.raw")));
        assert_eq!(x.len(), want.len(), "profile {p}: burst length");
        let e = x.iter().zip(&want).map(|(a, b)| (a - b).norm()).fold(0f32, f32::max);
        assert!(e < 1e-5, "profile {p}: burst error {e}");
        let d = decode(&want).unwrap_or_else(|| panic!("profile {p}: no burst"));
        assert_eq!(d.layers_complete, h.layers.len(), "profile {p}: {:?}", d.good);
        assert_eq!(d.stream[..], stream[..d.stream.len().min(stream.len())], "profile {p}: stream");
        assert!(d.cfo_hz.abs() < 5.0 && d.clock_ppm.unwrap().abs() < 1.0, "profile {p}: {} Hz {:?} ppm", d.cfo_hz, d.clock_ppm);
    }
}

#[test]
fn iq_file_round_trip_at_1_msps() {
    let pic = test_picture(960, 540);
    let m = make(&pic, 960, 540, 0, "LY1ABC").unwrap();
    let mut y = sdrkit_algo_dsp::sinc_table::SincConverter::convert(&m.samples, FS, 1e6).unwrap();
    let peak = y.iter().map(|v| v.re.abs().max(v.im.abs())).fold(0f32, f32::max);
    for v in &mut y {
        *v *= 0.8 / peak;
    }
    // Written as raw complex float32 and read back.
    let bytes: Vec<u8> = y.iter().flat_map(|v| [v.re.to_le_bytes(), v.im.to_le_bytes()].concat()).collect();
    let mut z: Vec<Complex32> = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| Complex32::new(f32::from_le_bytes([c[0], c[1], c[2], c[3]]), f32::from_le_bytes([c[4], c[5], c[6], c[7]])))
        .collect();
    Noise::new(21).add(&mut z, 2.0 * 0.03 * 0.03);
    let mut back = Vec::new();
    sdrkit_algo_dsp::resample::Resampler::new(1e6, FS).unwrap().process(&z, &mut back);
    let d = decode(&back).expect("a burst");
    assert_eq!((d.header.callsign.as_str(), d.header.width), ("LY1ABC", 960));
    assert!(d.layers_complete >= 7, "{:?} snr {} ppm {:?} cfo {}", d.good, d.snr_db, d.clock_ppm, d.cfo_hz);
    assert!(psnr(&pic, d.picture.as_ref().unwrap()) > 30.0);
}

#[test]
fn the_transmitter_checks_and_shrinks() {
    use sdrkit_stack_custom::picture_burst::stack::{fit, tx_burst, TxError};
    use sdrkit_stack_custom::picture_burst::{callsign, MakeError};
    let (w, h, _) = fit(3000, 1500, &vec![128u8; 3000 * 1500 * 3]);
    assert_eq!((w, h), (1920, 960));
    assert_eq!(callsign("ABCDEFGHIJKLMNO"), Err(MakeError::Callsign));
    assert_eq!(callsign("AB_C"), Err(MakeError::Callsign));
    assert_eq!(tx_burst("TEST1", 0, "", 300e3), Err(TxError::Rate));
    assert!(matches!(tx_burst("TEST1", 9, "", 1.2e6), Err(TxError::Make(_))));
    assert_eq!(tx_burst("TEST1", 3, "/nonexistent/picture.ppm", 1.2e6), Err(TxError::Picture));
    let x = tx_burst("TEST1", 3, "", 1.2e6).unwrap();
    let peak = x.iter().map(|v| v.re.abs().max(v.im.abs())).fold(0f32, f32::max);
    assert!((0.5..0.95).contains(&peak), "{peak}");
}

