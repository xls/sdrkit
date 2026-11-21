//! DECT NR+: packets at offsets, back to back over two paths, from a 4 Msps capture off the middle, in noise, noise alone,
//! the refused rate and the data file.

use sdrkit_algo_dsp::resample::resample_span;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, InputInfo, Owner, Receiver, StackConfig, Values};
use sdrkit_stack_ism::nrplus::gen::{packet, test_fields};
use sdrkit_stack_ism::nrplus::phy::RATE_HZ;
use sdrkit_stack_ism::nrplus::{self, TEST_DIAL_HZ};
use sdrkit_stack_ism::typed::{NrPlusFields, NrPlusPacketRow, NRPLUS_PACKETS};

fn frames_ok(r: &sdrkit_core::Results) -> i64 {
    NrPlusFields::read(r.fields()).map_or(0, |f| f.frames_ok)
}

fn receiver(rate: f64) -> Box<dyn Receiver> {
    let mut rx = nrplus::receiver(&StackConfig::new(Values::defaults(&nrplus::schema()))).unwrap();
    rx.configure(&InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording())
        .unwrap();
    Box::new(rx)
}

fn noise(x: &mut [Complex32], sigma: f32, seed: u64) {
    let mut r = Rng::new(seed);
    for v in x {
        *v += Complex32::new(sigma * r.normal_f32(), sigma * r.normal_f32());
    }
}

/// The texts of the packets read from `x` at `rate`.
fn texts(rate: f64, x: &[Complex32]) -> Vec<String> {
    let mut rx = receiver(rate);
    let mut out = sdrkit_core::Output::default();
    for p in x.chunks(20_000) {
        rx.feed(p, &mut out);
    }
    rx.finish(&mut out);
    let mut v: Vec<String> = NRPLUS_PACKETS
        .rows::<NrPlusPacketRow>(rx.results())
        .map(|(_, r)| r.text.to_string())
        .collect();
    v.reverse();
    v
}

/// Packets of the three test fields at 1.728 Msps, `gap` samples apart, each moved by `offset_hz`, with an echo of `echo`
/// (amplitude) `delay` samples later.
fn packets(n: usize, gap: usize, offset_hz: f64, echo: f32, delay: usize) -> Vec<Complex32> {
    let mut rng = Rng::new(2);
    let fields = test_fields();
    let mut x = vec![Complex32::default(); 2000];
    for k in 0..n {
        let (f, m) = fields[k % 3];
        let p = packet(&f, m, &mut rng);
        let at = x.len();
        x.resize(at + p.len() + gap, Complex32::default());
        for (i, v) in p.iter().enumerate() {
            x[at + i] += v;
            x[at + i + delay] += v * echo;
        }
    }
    x.resize(x.len() + 2000, Complex32::default());
    for (i, v) in x.iter_mut().enumerate() {
        let ph = (std::f64::consts::TAU * offset_hz * i as f64 / RATE_HZ) as f32;
        *v *= Complex32::from_polar(1.0, ph);
    }
    x
}

#[test]
fn packets_at_offsets_are_read() {
    for off in [0.0, 12e3, -40e3] {
        let mut x = packets(6, 3000, off, 0.0, 0);
        noise(&mut x, 0.02, 1);
        let t = texts(RATE_HZ, &x);
        assert_eq!(t.len(), 6, "offset {off}: {t:?}");
        assert_eq!(
            t[1],
            "Type 2 format 0, to 1234, length 4 subslots, MCS 2, HARQ 1 rv 0, feedback format 0, closed-loop MIMO"
        );
        assert_eq!(t[0], "Type 1 format 0, length 4 subslots, MCS 2, power 5");
        assert!(t[2].ends_with(", beamformed"));
    }
}

#[test]
fn four_packets_back_to_back_over_two_paths() {
    let mut x = packets(4, 3, 0.0, 0.5, 3);
    noise(&mut x, 0.02, 3);
    assert_eq!(texts(RATE_HZ, &x).len(), 4);
}

#[test]
fn a_4_msps_capture_300_khz_off_the_middle() {
    let x = packets(6, 3000, 0.0, 0.0, 0);
    let mut y = Vec::new();
    resample_span(&x, RATE_HZ, 4e6, &mut y);
    for (i, v) in y.iter_mut().enumerate() {
        let ph = (std::f64::consts::TAU * 300e3 * i as f64 / 4e6) as f32;
        *v *= Complex32::from_polar(1.0, ph);
    }
    // The radio sits 300 kHz below the channel.
    let mut rx = nrplus::receiver(&StackConfig::new(Values::defaults(&nrplus::schema()))).unwrap();
    rx.configure(&InputInfo::new(4e6, TEST_DIAL_HZ - 300e3, TEST_DIAL_HZ).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    for p in y.chunks(40_000) {
        rx.feed(p, &mut out);
    }
    rx.finish(&mut out);
    let n = frames_ok(rx.results());
    assert_eq!(n, 6, "{n}");
    let (_, row) = NRPLUS_PACKETS
        .rows::<NrPlusPacketRow>(rx.results())
        .next()
        .unwrap();
    assert!(row.r#type.starts_with("control field type"));
}

#[test]
fn snr_and_noise_alone() {
    // Unit power packets; noise of power 10^(-snr/10) per sample.
    let mut good = Vec::new();
    for snr in [0.0f32, 3.0] {
        let mut x = packets(20, 2000, 0.0, 0.0, 0);
        noise(&mut x, (10f32.powf(-snr / 10.0) / 2.0).sqrt(), 7);
        good.push(texts(RATE_HZ, &x).len());
    }
    assert!(good[1] == 20 && good[0] >= 6, "{good:?}");
    let mut x = vec![Complex32::default(); 4_000_000];
    noise(&mut x, 0.1, 9);
    assert!(texts(4e6, &x).is_empty());
}

#[test]
fn a_low_rate_is_refused_and_the_data_file() {
    let mut rx = nrplus::receiver(&StackConfig::new(Values::defaults(&nrplus::schema()))).unwrap();
    assert!(rx
        .configure(&InputInfo::new(1e6, TEST_DIAL_HZ, TEST_DIAL_HZ))
        .is_err());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/plans/dect_nrplus.json");
    let text = std::fs::read_to_string(path).unwrap();
    let file = sdrkit_data_plans::data::parse("dect_nrplus.json", &text).unwrap();
    assert_eq!(file.groups[0].bands.len(), 1);
}

#[test]
fn a_retune_keeps_the_time_and_the_lock() {
    let x = packets(6, 3000, 0.0, 0.0, 0);
    let mut rx = receiver(RATE_HZ);
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    let quiet = vec![Complex32::default(); (RATE_HZ * 3.0) as usize];
    rx.feed(&quiet, &mut out);
    rx.retuned(&InputInfo::new(RATE_HZ, TEST_DIAL_HZ + 1e6, TEST_DIAL_HZ + 1e6).recording());
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let r = rx.results();
    assert_eq!(frames_ok(r), 12);
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
    let newest = NRPLUS_PACKETS.rows::<NrPlusPacketRow>(r).next().unwrap().1;
    assert!(newest.time_s > 3.0);
}

#[test]
fn a_change_of_frequency_keeps_the_logs_and_drops_the_measurements() {
    let x = packets(6, 3000, 0.0, 0.0, 0);
    let mut rx = receiver(RATE_HZ);
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let (frames, devices) = {
        let before = NrPlusFields::read(rx.results().fields()).unwrap();
        assert!(before.frames_ok > 0 && before.snr_db.is_some() && before.level_dbfs.is_some());
        (before.frames_ok, before.devices)
    };
    rx.forget();
    let after = NrPlusFields::read(rx.results().fields()).unwrap();
    assert_eq!((after.frames_ok, after.devices), (frames, devices));
    assert_eq!(
        (after.snr_db, after.cfo_hz, after.level_dbfs),
        (None, None, None)
    );
    let rows = NRPLUS_PACKETS.rows::<NrPlusPacketRow>(rx.results()).count();
    assert_eq!(rows as i64, frames);
}
