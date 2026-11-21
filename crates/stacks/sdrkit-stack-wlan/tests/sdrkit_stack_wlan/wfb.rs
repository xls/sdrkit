//! `wfb`: the HT-SIG CRC, the HT-mixed PHY over MCS, guard intervals, offsets, clock error, an echo and noise, the frame
//! layouts and the loss count, and the stack on its test signal at 20 and 40 Msps.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields as _;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, Output, Results, SettingSet, StackConfig, Values};
use sdrkit_dev_test::Harness;
use sdrkit_proto_wlan_phy::ofdm::OfdmReceiver;
use sdrkit_stack_wlan::ht::{HtDecoder, HtResult, HtSig, Mcs, PilotPattern};
use sdrkit_stack_wlan::typed::WfbFields;
use sdrkit_stack_wlan::wfb::frames::{self, Kind, LinkId, LossCount};
use sdrkit_stack_wlan::wfb::gen::{self, TestFrame};
use sdrkit_stack_wlan::wfb::{WfbSetting, WFB};

#[test]
fn the_ht_sig_crc_of_the_worked_example_is_39() {
    let s = HtSig {
        mcs: 1,
        length: 300,
        smoothing: true,
        not_sounding: true,
        ..HtSig::default()
    };
    assert_eq!(s.crc(), 0x39);
}

#[test]
fn the_frame_layouts_and_the_loss_count() {
    let m = frames::wfb_data(0x0A1B2C, 3, 1234, 5, &[0u8; 1466]);
    let f = frames::parse(&m).unwrap();
    assert_eq!(
        f.link,
        LinkId::Wfb {
            link: 0x0A1B2C,
            port: 3
        }
    );
    assert_eq!(f.link.text(), "link 0A1B2C port 3");
    assert_eq!(
        f.kind,
        Kind::Data {
            block: 1234,
            fragment: 5
        }
    );
    let o = frames::openhd(1, 5, 0x0000_0002_0000_0011, &[0u8; 40]);
    let f = frames::parse(&o).unwrap();
    assert_eq!(
        (f.link.text(), f.kind),
        (
            "air port 5".to_string(),
            Kind::Frame {
                nonce: 0x0000_0002_0000_0011
            }
        )
    );
    let mut short = frames::wfb_data(1, 1, 1, 1, &[0u8; 10]);
    assert!(frames::parse(&short).is_none());
    short[0] = 0x88;
    assert!(frames::parse(&short).is_none());
    let mut l = LossCount::default();
    for f in (0..12).filter(|&f| f != 5) {
        l.take(100, f);
    }
    l.take(102, 0);
    l.take(102, 1);
    assert_eq!(l.lost, 13);
}

/// A wfb-ng frame of `len` payload bytes with its FCS.
fn frame(mcs: u8, short_gi: bool, len: usize) -> TestFrame {
    let mut m = frames::wfb_data(0x0A1B2C, 3, 77, 1, &vec![0x5Au8; len]);
    let c = sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC.checksum(&m);
    m.extend(c.to_le_bytes());
    TestFrame {
        psdu: m,
        mcs: Mcs(mcs),
        short_gi,
    }
}

/// The PPDU of `f` at 20 Msps after 400 samples of quiet, moved by `offset_hz`, its clock off by `ppm`, with an echo of
/// `echo` samples at half the amplitude, in noise of SNR `snr_db` per sample.
fn channel(
    f: &TestFrame,
    offset_hz: f64,
    ppm: f64,
    echo: usize,
    snr_db: f64,
    seed: u64,
) -> Vec<Complex32> {
    channel_with(f, PilotPattern::Fixed, offset_hz, ppm, echo, snr_db, seed)
}

/// As [`channel`], with the pilots of `pattern`.
fn channel_with(
    f: &TestFrame,
    pattern: PilotPattern,
    offset_hz: f64,
    ppm: f64,
    echo: usize,
    snr_db: f64,
    seed: u64,
) -> Vec<Complex32> {
    let rate = 20e6 * (1.0 + ppm * 1e-6);
    let s = gen::ppdu(f, 0x5D, rate, pattern).unwrap();
    let mut x = vec![Complex32::default(); 400];
    x.extend(&s);
    x.extend(vec![Complex32::default(); 600]);
    if echo > 0 {
        let d = x.clone();
        for i in echo..x.len() {
            x[i] += d[i - echo] * 0.5;
        }
    }
    let sigma = (10f64.powf(-snr_db / 10.0) / 2.0).sqrt() as f32;
    let mut rng = Rng::new(seed);
    for (i, v) in x.iter_mut().enumerate() {
        let p = std::f64::consts::TAU * offset_hz * i as f64 / 20e6;
        *v = *v * Complex32::new(p.cos() as f32, p.sin() as f32)
            + Complex32::new(sigma * rng.normal_f32(), sigma * rng.normal_f32());
    }
    x
}

/// The MPDUs that the receiver and the HT decoder get from `x`.
fn decode(x: &[Complex32]) -> Vec<Vec<u8>> {
    let mut rx = OfdmReceiver::new();
    let mut ht = HtDecoder::new();
    let mut got = Vec::new();
    rx.process_with(
        x,
        |_| {},
        |v| match ht.decode(v) {
            HtResult::NotHt => false,
            HtResult::Frame(f) => {
                got.push(f.mpdu.to_vec());
                true
            }
            _ => true,
        },
    );
    got
}

#[test]
fn every_mcs_and_guard_interval_decodes_at_three_offsets() {
    for mcs in 0..8 {
        for short_gi in [false, true] {
            let f = frame(mcs, short_gi, 300);
            for offset in [0.0, 90e3, -210e3] {
                for pattern in [PilotPattern::Fixed, PilotPattern::Rotating] {
                    let got = decode(&channel_with(&f, pattern, offset, 0.0, 0, 30.0, 3));
                    assert_eq!(
                        got.len(),
                        1,
                        "MCS {mcs} short GI {short_gi} offset {offset} {pattern:?}"
                    );
                    assert_eq!(got[0], f.psdu[..f.psdu.len() - 4]);
                }
            }
        }
    }
}

#[test]
fn a_clock_error_and_an_echo_inside_the_guard_interval_decode() {
    for mcs in [0, 4, 7] {
        let f = frame(mcs, false, 400);
        assert_eq!(
            decode(&channel(&f, 30e3, 15.0, 0, 30.0, 5)).len(),
            1,
            "MCS {mcs} 15 ppm"
        );
        assert_eq!(
            decode(&channel(&f, 30e3, 0.0, 3, 30.0, 6)).len(),
            1,
            "MCS {mcs} 150 ns echo"
        );
    }
}

#[test]
fn mcs_1_frames_of_300_bytes_decode_by_snr() {
    let f = frame(1, false, 300 - 24 - 9 - 4);
    let hits = |snr: f64| -> usize {
        (0..20)
            .filter(|&k| decode(&channel(&f, 20e3, 0.0, 0, snr, 100 + k)).len() == 1)
            .count()
    };
    let (a, b, c, d) = (hits(8.0), hits(10.0), hits(5.5), hits(4.0));
    eprintln!("MCS 1, 300 bytes: {a} of 20 at 8 dB, {b} at 10 dB, {c} at 5.5 dB, {d} at 4 dB");
    assert!(a >= 15, "8 dB: {a} of 20");
    assert!(b >= 19, "10 dB: {b} of 20");
    // One frame of 20 decodes at 5.5 dB with these seeds; none at 4 dB.
    assert!(c <= 1, "5.5 dB: {c} of 20");
    assert_eq!(d, 0, "4 dB: {d} of 20");
}

#[test]
fn non_ht_frames_are_left_to_the_non_ht_receiver() {
    let mut x = vec![Complex32::default(); 400];
    let mut m = frames::wfb_data(1, 2, 3, 4, &[0x11; 200]);
    let c = sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC.checksum(&m);
    m.extend(c.to_le_bytes());
    sdrkit_proto_wlan_phy::ofdm::build_ppdu(
        &m,
        sdrkit_proto_wlan_phy::ofdm::Rate::R6,
        0x33,
        &mut x,
    )
    .unwrap();
    x.extend(vec![Complex32::default(); 400]);
    let mut rx = OfdmReceiver::new();
    let mut ht = HtDecoder::new();
    let (mut offered, mut non_ht) = (0, 0);
    rx.process_with(
        &x,
        |_| non_ht += 1,
        |v| {
            offered += 1;
            !matches!(ht.decode(v), HtResult::NotHt)
        },
    );
    assert_eq!((offered, non_ht), (1, 1));
}

/// Feeds `x` at `rate` to the stack with the signal `offset_hz` from the centre.
fn run(x: &[Complex32], rate: f64, offset_hz: f64) -> Results {
    let mut rx =
        (WFB.make_receiver)(&StackConfig::new(Values::defaults(&WfbSetting::schema()))).unwrap();
    let c = 5805e6;
    rx.configure(&InputInfo::new(rate, c - offset_hz, c).recording())
        .unwrap();
    let mut out = Output::default();
    for p in x.chunks((rate / 100.0) as usize) {
        rx.feed(p, &mut out);
        out.clear();
    }
    out.clear();
    rx.finish(&mut out);
    Owner::results(&*rx).clone()
}

/// One cycle of the test signal at `rate`, moved by `offset_hz`.
fn one_cycle(rate: f64, offset_hz: f64) -> Vec<Complex32> {
    let mut s = gen::scene(rate).unwrap();
    let mut x = vec![Complex32::default(); (rate * gen::PERIOD_S) as usize];
    sdrkit_core::Generator::generate(&mut s, &mut x);
    let mut rng = Rng::new(2);
    for (i, v) in x.iter_mut().enumerate() {
        let p = std::f64::consts::TAU * offset_hz * i as f64 / rate;
        *v = *v * Complex32::new(p.cos() as f32, p.sin() as f32)
            + Complex32::new(0.01 * rng.normal_f32(), 0.01 * rng.normal_f32());
    }
    x
}

#[test]
fn the_test_signal_gives_all_39_frames_of_four_links_and_the_loss_at_20_and_40_msps() {
    for (rate, offset) in [(20e6, 0.0), (20e6, 3.1e6), (20e6, -3.1e6), (40e6, 0.0)] {
        let r = run(&one_cycle(rate, offset), rate, offset);
        let f = WfbFields::read(r.fields()).unwrap();
        assert_eq!(
            (f.frames_ok, f.links_heard, f.frames_lost),
            (gen::FRAMES as i64, 4, gen::LOST as i64),
            "{rate} {offset}"
        );
    }
}

#[test]
fn below_20_msps_the_stack_says_it_needs_20_msps() {
    let mut rx = Harness::new(&WFB).receiver().unwrap();
    assert!(rx
        .configure(&InputInfo::new(10e6, 5805e6, 5805e6).recording())
        .is_err());
    assert!(Owner::results(&*rx)
        .status()
        .summary
        .contains("needs 20 Msps"));
    let mut rng = Rng::new(3);
    let x: Vec<Complex32> = (0..4_000_000)
        .map(|_| Complex32::new(0.05 * rng.normal_f32(), 0.05 * rng.normal_f32()))
        .collect();
    let r = run(&x, 20e6, 0.0);
    assert_eq!(r.fields().at(&WfbFields::FRAMES_OK), Some(0));
}
