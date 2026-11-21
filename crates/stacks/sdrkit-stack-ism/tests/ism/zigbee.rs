//! `zigbee`: the header vectors, the generator round trip, bad FCS, the adjacent channel, noise, settings, tuning requests and
//! saved state.

use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_LEVEL_DBFS, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    Complex32, HostRequest, InputInfo, Output, Receiver, Results, StackConfig, Value,
};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_dev_test::Harness;
use sdrkit_stack_ism::family::from_hex;
use sdrkit_stack_ism::mac154::Addr;
use sdrkit_stack_ism::typed::{PanDeviceRow, ZigbeeNetworkRow};
use sdrkit_stack_ism::zigbee::{self, describe, ZIGBEE};
use sdrkit_util_bits::crc::catalog::CRC16_KERMIT;

#[test]
fn header_vectors() {
    let exp = read_expected("ism").unwrap();
    let v = exp["byte_vectors"]["zigbee_frames"].as_array().unwrap();
    let ack = from_hex(v[0]["hex"].as_str().unwrap()).unwrap();
    assert_eq!(describe(&ack).unwrap().text, "Ack #90");
    let br = describe(&from_hex(v[1]["hex"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(br.text, "Beacon request");
    assert_eq!((br.mac.dst_pan, br.mac.seq), (Some(0xFFFF), Some(0x2A)));
    let data = describe(&from_hex(v[2]["hex"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(data.pan, Some(0x1A62));
    assert!(data.encrypted);
    assert!(
        data.text
            .starts_with("Zigbee NWK data 0x7A12 > 0x0000, radius 30"),
        "{}",
        data.text
    );
    assert!(data.text.contains("protocol 2") && data.text.contains("encrypted"));
    let b = describe(&from_hex(v[3]["hex"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(b.pan, Some(0x1A62));
    assert_eq!(b.mac.src, Addr::Short(0));
    assert_eq!(
        b.beacon,
        Some((true, true, Some(2), Some(0x0102030405060718)))
    );
    for want in [
        "NWK version 2",
        "router slots",
        "end-device slots",
        "depth 0",
        "update id 3",
    ] {
        assert!(b.text.contains(want), "{}", b.text);
    }
}

/// The 802.15.4 FCS is CRC-16/KERMIT, check value 0x2189 (IEEE 802.15.4-2020 clause 7.2.10), and every generated frame passes it.
#[test]
fn the_fcs_is_the_catalogue_crc16_kermit() {
    assert_eq!(CRC16_KERMIT.checksum(b"123456789"), 0x2189);
    for (psdu, _, _) in zigbee::schedule() {
        assert!(CRC16_KERMIT.verify(&psdu, sdrkit_util_bits::crc::Trailer::Little));
    }
}

fn field(r: &Results, k: &str) -> f64 {
    crate::util::number(r, k)
}

/// The frames the schedule sends in `seconds` at `rate` (each frame and its gap).
fn sent_in(seconds: f64, rate: f64) -> usize {
    let sched = zigbee::schedule();
    let (mut t, mut n) = (0.0005f64, 0);
    for k in 0.. {
        let (psdu, _, gap) = &sched[k % sched.len()];
        let pairs = zigbee::ppdu_chip_pairs(psdu, false, false);
        let len = zigbee::oqpsk(&pairs, rate).unwrap().len() as f64 / rate;
        if t + len > seconds {
            break;
        }
        n += 1;
        t += len + gap;
    }
    n
}

/// Generates with `h` and runs the receiver to the end of the input.
fn run(h: Harness) -> sdrkit_dev_test::Run {
    let x = h.generate().unwrap();
    h.feed(&x).unwrap()
}

fn rows<'a>(r: &'a Results, t: &str) -> Vec<&'a sdrkit_core::Record> {
    crate::util::table(r, t)
        .rows_newest_first()
        .map(|(_, c)| c)
        .collect()
}

#[test]
fn generator_round_trip() {
    for (off, ppm) in [(50e3, 40.0), (-50e3, -40.0)] {
        let h = Harness::new(&ZIGBEE)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 10.0)
            .set(&GEN_OFFSET_HZ, off)
            .set(&GEN_CLOCK_PPM, ppm)
            .seconds(0.2);
        let run = run(h);
        let r = &run.results;
        let sent = sent_in(0.2, 4e6);
        let good = field(r, "frames_ok");
        assert!(good >= 0.99 * sent as f64, "{good} of {sent} at {off}");
        let nets = rows(r, "networks");
        assert!(
            nets.iter()
                .any(|c| c.at(&ZigbeeNetworkRow::PAN) == Some("0x1A62")),
            "{nets:?}"
        );
        assert!(
            nets.iter()
                .any(|c| c.at(&ZigbeeNetworkRow::PAN) == Some("0x3C2D")),
            "{nets:?}"
        );
        let devs = rows(r, "devices");
        let bulb: Vec<_> = devs
            .iter()
            .filter(|c| c.at(&PanDeviceRow::EXTENDED) == Some("00:17:88:01:0A:0B:0C:0D"))
            .collect();
        assert_eq!(bulb.len(), 1, "{devs:?}");
        assert_eq!(bulb[0].at(&PanDeviceRow::SHORT), Some("0x1234"));
        assert_eq!(
            bulb[0].at(&PanDeviceRow::VENDOR),
            Some("Philips Lighting BV")
        );
        assert!(
            !devs
                .iter()
                .any(|c| c.at(&PanDeviceRow::SHORT) == Some("0x1234")
                    && c.at(&PanDeviceRow::EXTENDED).is_none()),
            "not merged: {devs:?}"
        );
        let loops = (good as usize / 13) as f64;
        assert!(field(r, "encrypted_frames") >= 3.0 * loops);
    }
}

/// One case of the chip-value figures: offset Hz, clock ppm, and (SNR in 2 MHz, least share in %) for each level.
type ChipCase<'a> = (f64, f64, &'a [(f64, f64)]);

/// The cases of the chip-value figures that fall short: the share of frames decoded over `trials` trials of 0.25 s (noise
/// seeds 20 and up).
fn chip_shortfalls(cases: &[ChipCase], trials: u64) -> Vec<String> {
    let secs = 0.25;
    let mut short = Vec::new();
    for &(off, ppm, levels) in cases {
        for &(snr, pct) in levels {
            let mut good = 0.0;
            for seed in 0..trials {
                let h = Harness::new(&ZIGBEE)
                    .set(&GEN_NOISE, NoiseKind::Snr)
                    .set(&GEN_SNR_DB, snr)
                    .set(&GEN_OFFSET_HZ, off)
                    .set(&GEN_CLOCK_PPM, ppm)
                    .seed(20 + seed)
                    .seconds(secs);
                good += field(&run(h).results, "frames_ok");
            }
            let sent = trials as f64 * sent_in(secs, 4e6) as f64;
            let got = 100.0 * good / sent;
            println!("{off} Hz {ppm} ppm {snr} dB: {good} of {sent} ({got:.1} %)");
            if got < pct {
                short.push(format!("{off} Hz {ppm} ppm {snr} dB: {got:.1} % < {pct} %"));
            }
        }
    }
    short
}

/// The chip value is the sine of the phase step weighted by the chip's strength. The figures of the row this form reaches (SNR
/// in 2 MHz, 4 trials): with +48 kHz at least 99.7 and 88 % of the frames at 3 and 1.5 dB; with a 50 ppm clock 98, 78 and
/// 18 % at 3, 1.5 and 0 dB.
#[test]
fn chip_values_near_the_threshold() {
    let short = chip_shortfalls(
        &[
            (48e3, 0.0, &[(3.0, 99.7), (1.5, 88.0)]),
            (0.0, 50.0, &[(3.0, 98.0), (1.5, 78.0), (0.0, 18.0)]),
        ],
        4,
    );
    assert!(short.is_empty(), "{short:#?}");
}

/// The carrier offset of a frame is measured on its preamble: the frequencies of the lowest and of the highest event move
/// with the carrier, within 5 kHz of +48 kHz and of -90 kHz at 20 dB in 2 MHz (at +150 kHz the reading is 6 to 9 kHz low).
#[test]
fn the_carrier_offset_is_measured() {
    let ends = |off: f64| {
        let h = Harness::new(&ZIGBEE)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 20.0)
            .set(&GEN_OFFSET_HZ, off)
            .seconds(0.25);
        let f: Vec<f64> = run(h).events.iter().map(|e| e.freq_hz).collect();
        assert!(!f.is_empty(), "{off}: no event");
        (
            f.iter().copied().fold(f64::MAX, f64::min),
            f.iter().copied().fold(f64::MIN, f64::max),
        )
    };
    let on_tune = ends(0.0);
    for off in [48e3, -90e3] {
        let got = ends(off);
        let (low, high) = (got.0 - on_tune.0, got.1 - on_tune.1);
        assert!(
            (low - off).abs() < 5e3 && (high - off).abs() < 5e3,
            "{off}: {low} and {high}"
        );
    }
}

/// All the row's figures over 16 trials (1216 frames a level; over 4 trials the 99.7 % points allow no lost frame of 304 and
/// the 1.5 % point is 5 frames, and both fail on some noise seeds): at 3, 1.5, 0 and -1.5 dB at least 99.7, 90, 31 and 1.5 %; with +48 kHz 99.7, 88 and 30 % at the
/// first three levels; with a 50 ppm clock 98, 78 and 18 %; with -90 kHz 99, 84 and 25 %.
#[test]
fn chip_values_reach_the_row() {
    let short = chip_shortfalls(
        &[
            (
                0.0,
                0.0,
                &[(3.0, 99.7), (1.5, 90.0), (0.0, 31.0), (-1.5, 1.5)],
            ),
            (48e3, 0.0, &[(3.0, 99.7), (1.5, 88.0), (0.0, 30.0)]),
            (0.0, 50.0, &[(3.0, 98.0), (1.5, 78.0), (0.0, 18.0)]),
            (-90e3, 0.0, &[(3.0, 99.0), (1.5, 84.0), (0.0, 25.0)]),
        ],
        16,
    );
    assert!(short.is_empty(), "{short:#?}");
}

/// Every event the test signal makes is declared, and no event says frequency 0.
#[test]
fn events_are_declared_and_carry_their_frequency() {
    let d = sdrkit_stack_ism::typed::shape("zigbee").results_decl();
    let run = run(Harness::new(&ZIGBEE).seconds(0.2));
    assert!(!run.events.is_empty());
    assert!(crate::util::declared(&d, &run.events));
    for e in &run.events {
        assert!(e.freq_hz > 2.4e9, "{} at {} Hz", e.kind(), e.freq_hz);
    }
}

#[test]
fn bad_fcs_counts_and_adds_nothing() {
    let run = run(Harness::new(&ZIGBEE)
        .set(&zigbee::GEN_BAD_FCS, true)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 20.0)
        .seconds(0.1));
    let r = &run.results;
    assert_eq!(field(r, "frames_ok"), 0.0);
    assert!(field(r, "frames_bad") > 10.0, "{:?}", r.fields());
    assert!(rows(r, "networks").is_empty() && rows(r, "devices").is_empty());
}

#[test]
fn adjacent_channel_transmitter() {
    let run = run(Harness::new(&ZIGBEE)
        .rate(16e6)
        .set(&zigbee::GEN_INTERFERER_DB, 30.0)
        .set(&GEN_LEVEL_DBFS, -33.0)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 20.0)
        .seconds(0.1));
    let sent = sent_in(0.1, 16e6);
    let good = field(&run.results, "frames_ok");
    assert!(good >= 0.95 * sent as f64, "{good} of {sent}");
}

#[test]
fn noise_returns_no_frames() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let x: Vec<Complex32> = (0..4_000_000 * 3)
        .map(|_| rng.complex_normal(0.005))
        .collect();
    let run = Harness::new(&ZIGBEE).feed(&x).unwrap();
    assert_eq!(field(&run.results, "frames_ok"), 0.0);
}

fn defaults() -> StackConfig {
    StackConfig::new(sdrkit_core::Values::defaults(&(ZIGBEE.settings)()))
}

/// A value of the wrong kind or out of range is refused.
#[test]
fn a_bad_setting_is_refused() {
    let mut g = zigbee::receiver(&defaults()).unwrap();
    assert!(crate::util::set(&mut g, "channel", Value::I64(30)).is_err());
    assert!(crate::util::set(&mut g, "scan", Value::Text("often".into())).is_err());
    assert!(crate::util::set(&mut g, "dwell", Value::Duration(0.0)).is_err());
    assert!(crate::util::set(&mut g, "channel", Value::I64(15)).is_ok());
}

#[test]
fn below_the_minimum_rate_the_status_says_why() {
    let mut g = zigbee::receiver(&defaults()).unwrap();
    assert!(g.configure(&InputInfo::new(2e6, 2405e6, 2405e6)).is_err());
    assert_eq!(
        g.results().status().summary,
        "Zigbee / 802.15.4 needs a sample rate of at least 4 Msps"
    );
}

#[test]
fn the_channel_setting_asks_the_host_to_tune() {
    let cfg = crate::util::stack_with(&ZIGBEE, &[("channel", Value::I64(20))]);
    let mut g = zigbee::receiver(&cfg).unwrap();
    g.configure(&InputInfo::new(4e6, 2405e6, 2405e6).recording())
        .unwrap();
    let mut out = Output::default();
    g.feed(&vec![Complex32::default(); 400_000], &mut out);
    assert!(out.requests.is_empty(), "{:?}", out.requests);
    crate::util::set(&mut g, "channel", Value::I64(15)).unwrap();
    out.clear();
    g.feed(&vec![Complex32::default(); 400_000], &mut out);
    assert_eq!(
        out.requests,
        vec![HostRequest::retune(2425e6, "Zigbee channel")]
    );
    crate::util::set(&mut g, "scan", Value::Bool(true)).unwrap();
    out.clear();
    g.feed(&vec![Complex32::default(); 400_000], &mut out);
    match &out.requests[..] {
        [HostRequest::Hop {
            channels, dwell_s, ..
        }] => {
            assert_eq!(channels.len(), 16);
            assert_eq!(*dwell_s, 5.0);
        }
        r => panic!("{r:?}"),
    }
}

#[test]
fn saved_state_comes_back_and_clear_forgets() {
    let h = Harness::new(&ZIGBEE).seconds(0.2);
    let x = h.generate().unwrap();
    let mut rx: Box<dyn Receiver> = h.receiver().unwrap();
    rx.configure(&InputInfo::new(4e6, 2405e6, 2405e6).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&x, &mut out);
    let devices = crate::util::table(rx.results(), "devices").len();
    assert!(devices >= 5);
    let state = rx.save_state().expect("a saved state");
    let mut back = h.receiver().unwrap();
    back.configure(&InputInfo::new(4e6, 2405e6, 2405e6).recording())
        .unwrap();
    back.restore_state(&state).unwrap();
    back.feed(&vec![Complex32::default(); 400_000], &mut out);
    assert_eq!(crate::util::table(back.results(), "devices").len(), devices);
    crate::util::clear(back.as_mut());
    back.feed(&vec![Complex32::default(); 400_000], &mut out);
    assert_eq!(crate::util::table(back.results(), "devices").len(), 0);
}

/// A change of frequency keeps the networks, the devices and the counts, and drops the SNR of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_networks_and_drops_the_snr() {
    let h = Harness::new(&ZIGBEE).seconds(0.2);
    let mut r = zigbee::receiver(&defaults()).unwrap();
    let t = &ZIGBEE.tuning;
    r.configure(&InputInfo::new(t.rate_hz, t.default_hz, t.default_hz).recording())
        .unwrap();
    crate::util::feed_all(&mut r, &h.generate().unwrap());
    let networks = crate::util::table(r.results(), "networks").len();
    let frames = field(r.results(), "frames_ok");
    assert!(networks >= 1 && frames > 0.0);
    assert!(crate::util::has(r.results(), "snr_db"));
    r.forget();
    assert_eq!(crate::util::table(r.results(), "networks").len(), networks);
    assert_eq!(field(r.results(), "frames_ok"), frames);
    assert!(!crate::util::has(r.results(), "snr_db"));
    assert!(!crate::util::has(r.results(), "offset_hz"));
    assert_eq!(r.results().status().level_dbfs, None);
}
