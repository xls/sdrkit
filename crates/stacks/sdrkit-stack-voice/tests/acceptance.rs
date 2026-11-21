//! Acceptance tests of the stacks of this crate and of the MDC-1200 burst codec.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::keys;
use sdrkit_core::{
    Complex32, GenConfig, Generator, InputInfo, LockState, Output, Receiver, StackConfig,
    StackDescriptor, Value, Values,
};
use sdrkit_proto_link::modem::{AfskHeard as Heard, AfskReceiver};
use sdrkit_proto_tones::mdc::{self, BurstDecoder, Decoded, Packet};
use sdrkit_stack_voice::common::{self, Stage, RATE};
use sdrkit_stack_voice::elt121::{EltGenerator, SweepDetector};
use sdrkit_stack_voice::hfpacket::{self, PacketGenerator};
use sdrkit_stack_voice::inversion::{self, InversionDetector, InversionGenerator, Inverter};
use sdrkit_stack_voice::typed;
use sdrkit_stack_voice::{ELT121, HFPACKET, VOICE_INVERSION};
use std::f64::consts::TAU;

// ---------------------------------------------------------------------------------------------------------------------
// Helpers.

/// `n` samples of `g`.
fn take(g: &mut dyn Generator, n: usize) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); n];
    assert_eq!(g.generate(&mut x), n);
    x
}

/// `x` moved by `hz` at `rate`.
fn shifted(x: &[Complex32], hz: f64, rate: f64) -> Vec<Complex32> {
    x.iter()
        .enumerate()
        .map(|(i, &s)| {
            let (sin, cos) = (TAU * hz * i as f64 / rate).sin_cos();
            s * Complex32::new(cos as f32, sin as f32)
        })
        .collect()
}

/// Adds white noise to a 12 kHz stream so that the signal power (over its non-zero samples) over the noise power in 3 kHz is
/// `snr_db`.
fn with_snr(x: &[Complex32], snr_db: f64, seed: u64) -> Vec<Complex32> {
    let on: Vec<f64> = x
        .iter()
        .map(|s| f64::from(s.norm_sqr()))
        .filter(|&p| p > 0.0)
        .collect();
    let power = on.iter().sum::<f64>() / on.len().max(1) as f64;
    let noise = 4.0 * power / 10f64.powf(snr_db / 10.0);
    let mut rng = Rng::new(seed);
    x.iter().map(|&s| s + rng.complex_normal(noise)).collect()
}

/// White noise of mean power `power`.
fn noise(n: usize, power: f64, seed: u64) -> Vec<Complex32> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| rng.complex_normal(power)).collect()
}

/// Real white noise of `rms`.
fn real_noise(n: usize, rms: f64, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| (rms * rng.normal()) as f32).collect()
}

/// The frequency to tune `desc` to: its default, else the starting dial that a host reads from the data folder.
fn start_hz(desc: &StackDescriptor) -> f64 {
    if desc.tuning.default_hz > 0.0 {
        return desc.tuning.default_hz;
    }
    let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
    let rows = sdrkit_presets::radio_dials::dials(&data).unwrap();
    sdrkit_presets::default_dials::dial_hz(&rows, desc.id, None).unwrap()
}

/// The default settings of `desc` with its generator settings.
fn defaults(desc: &StackDescriptor) -> Values {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)()).unwrap();
    Values::defaults(&schema)
}

/// The receiver of `desc` at 250 ksps with the dial 20 kHz below the centre.
fn receiver(desc: &StackDescriptor) -> Box<dyn Receiver> {
    let mut rx = (desc.make_receiver)(&StackConfig::new(defaults(desc))).unwrap();
    let dial = start_hz(desc);
    rx.configure(&InputInfo::new(250e3, dial + common::TUNE_OFFSET_HZ, dial).recording())
        .unwrap();
    rx
}

/// `seconds` of the test signal of `desc` at 250 ksps (the generator puts it 20 kHz below the centre), with a little noise.
fn stack_signal(desc: &StackDescriptor, seconds: f64) -> Vec<Complex32> {
    let mut g = (desc.make_generator.unwrap())(&GenConfig::new(250e3, defaults(desc)).with_seed(3))
        .unwrap();
    let mut x = take(g.as_mut(), (seconds * 250e3) as usize);
    // 30 dB in 3 kHz for a signal of power 0.25.
    let mut rng = Rng::new(9);
    let n0 = 0.25 / 1000.0 * (250e3 / 3e3);
    x.iter_mut().for_each(|s| *s += rng.complex_normal(n0));
    x
}

/// Feeds `x` to `rx` in chunks of `chunk` samples. Returns the events.
fn feed(rx: &mut dyn Receiver, x: &[Complex32], chunk: usize) -> Vec<sdrkit_core::Event> {
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(chunk) {
        rx.feed(c, &mut out);
        events.append(&mut out.events);
        out.clear();
    }
    events
}

fn stage(rx: &dyn Receiver) -> Option<Stage> {
    rx.results().fields().at(&typed::DetectorFields::STATE)
}

/// The count of rows of the table `key` of `rx`.
fn row_count(rx: &dyn Receiver, key: &str) -> usize {
    let r = rx.results();
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.rows_newest_first().count())
}

/// The receiver of `desc` fed with `seconds` of its test signal.
fn fed(desc: &StackDescriptor, seconds: f64) -> Box<dyn Receiver> {
    let mut rx = receiver(desc);
    feed(rx.as_mut(), &stack_signal(desc, seconds), 50_000);
    rx
}

/// Runs `check` on one receiver of `desc` fed with `seconds` of its signal after a retune 5 kHz up that the host did
/// not order, and on a second one after the forget call of the host alone.
fn retune_then_forget(desc: &StackDescriptor, seconds: f64, check: impl Fn(&dyn Receiver, &str)) {
    let mut rx = fed(desc, seconds);
    let dial = start_hz(desc) + 5e3;
    rx.retuned(&InputInfo::new(250e3, dial + common::TUNE_OFFSET_HZ, dial).recording());
    // An empty feed merges the results of the components.
    rx.feed(&[], &mut Output::default());
    check(rx.as_ref(), "retune");
    let mut rx = fed(desc, seconds);
    rx.forget();
    check(rx.as_ref(), "forget");
}

/// The shared fields after a change of frequency: no level, no quality, no lock.
fn forgot_the_signal(rx: &dyn Receiver, step: &str) {
    let f = rx.results().fields();
    assert_eq!(stage(rx), Some(Stage::Searching), "{step}");
    assert_eq!(f.at(&typed::DetectorFields::LEVEL_DBFS), None, "{step}");
    assert_eq!(f.at(&typed::DetectorFields::QUALITY_DB), None, "{step}");
    assert_ne!(
        rx.results().status().lock_state,
        LockState::Locked,
        "{step}"
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// MDC-1200.

fn pack_msb(bits: &[u8]) -> Vec<u8> {
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b))
        .collect()
}

#[test]
fn mdc_check_values() {
    assert_eq!(mdc::check_value(b"123456789"), 0xDE76);
    assert_eq!(mdc::check_value(&[0x01, 0x80, 0x12, 0x34]), 0x3E2E);
    assert_eq!(
        mdc::block_bytes([0x01, 0x80, 0x12, 0x34]),
        [0x01, 0x80, 0x12, 0x34, 0x2E, 0x3E, 0x00, 0x65, 0x80, 0xA8, 0x62, 0xDD, 0x88, 0x08]
    );
    assert_eq!(
        pack_msb(&mdc::block_bits([0x01, 0x80, 0x12, 0x34])),
        [0x8E, 0x94, 0x2A, 0x2E, 0x06, 0x99, 0x22, 0x04, 0x00, 0x1C, 0x18, 0xA2, 0x2C, 0x88]
    );
    assert_eq!(mdc::check_value(&[0x01, 0x00, 0xBE, 0xEF]), 0x5CC3);
    assert_eq!(
        mdc::block_bytes([0x01, 0x00, 0xBE, 0xEF]),
        [0x01, 0x00, 0xBE, 0xEF, 0xC3, 0x5C, 0x00, 0x65, 0x00, 0x06, 0x49, 0x4A, 0x87, 0x1D]
    );
    assert_eq!(
        pack_msb(&mdc::block_bits([0x01, 0x00, 0xBE, 0xEF])),
        [0x92, 0x86, 0x0A, 0x01, 0x1A, 0x89, 0x3C, 0x35, 0x3E, 0x20, 0x38, 0x8E, 0x30, 0x96]
    );
    assert_eq!(mdc::check_value(&[0xDE, 0xAD, 0xBE, 0xEF]), 0xE6EA);
    assert_eq!(
        pack_msb(&mdc::block_bits([0xDE, 0xAD, 0xBE, 0xEF])),
        [0x56, 0x71, 0xE8, 0xA3, 0x9C, 0xCE, 0xFE, 0x70, 0xF6, 0x0D, 0x3A, 0xE8, 0xF4, 0xBC]
    );
    // The generator sends these bits, and the sync as tone decisions is the listed value.
    let bits = mdc::burst_bits(&Packet::new(0x01, 0x80, 0x1234), 7);
    assert_eq!(bits.len(), 208);
    assert_eq!(
        pack_msb(&bits[96..]),
        pack_msb(&mdc::block_bits([0x01, 0x80, 0x12, 0x34]))
    );
    let mut before = 1;
    let mut decisions = 0u64;
    for &b in &bits[56..96] {
        decisions = (decisions << 1) | u64::from(b != before);
        before = b;
    }
    assert_eq!(decisions, mdc::SYNC_DECISIONS);
    assert_eq!(
        mdc::burst_bits(&Packet::double(0x35, 0, 1, [1, 2, 3, 4]), 7).len(),
        320
    );
}

/// The packets decoded from `audio` at `rate`.
fn mdc_decode(audio: &[f32], rate: f64) -> Vec<Decoded> {
    let mut d = BurstDecoder::new(rate).unwrap();
    let mut got = Vec::new();
    d.process(audio, |p| got.push(p));
    got
}

/// A burst of `p` at `rate` after `before` samples of silence, with a tail.
fn mdc_burst(p: &Packet, rate: f64, amplitude: f32, before: usize) -> Vec<f32> {
    let mut x = vec![0.0; before];
    x.extend(mdc::burst_audio(p, rate, amplitude, 7));
    x.resize(x.len() + (0.1 * rate) as usize, 0.0);
    x
}

#[test]
fn mdc_clean_bursts_decode_once_each() {
    let mut rng = Rng::new(5);
    for i in 0..40u32 {
        let p = Packet::new(
            if i % 4 == 0 {
                0x01
            } else {
                (rng.below(0x30) + 2) as u8
            },
            rng.below(256) as u8,
            rng.below(65536) as u16,
        );
        let got = mdc_decode(
            &mdc_burst(&p, 48e3, 0.3, 100 + rng.below(400) as usize),
            48e3,
        );
        assert_eq!(got.len(), 1, "burst {i}: {got:?}");
        assert_eq!(got[0].packet, p);
    }
}

#[test]
fn mdc_noisy_bursts() {
    let mut right = 0;
    let mut rng = Rng::new(11);
    for i in 0..120u64 {
        let p = Packet::new(0x01, 0x80, rng.below(65536) as u16);
        let mut x = mdc_burst(&p, 48e3, 0.3, 200 + rng.below(1000) as usize);
        let rms = 0.02 + 0.2 * i as f64 / 119.0;
        for (v, n) in x.iter_mut().zip(real_noise(1 << 16, rms, 100 + i)) {
            *v += n;
        }
        let got = mdc_decode(&x, 48e3);
        if got.len() == 1 && got[0].packet == p {
            right += 1;
        }
    }
    assert!(right >= 102, "{right} of 120");
}

#[test]
fn mdc_other_rates_and_clock_errors() {
    let p = Packet::new(0x01, 0x00, 0xBEEF);
    for rate in [8e3, 16e3] {
        let got = mdc_decode(&mdc_burst(&p, rate, 0.3, 333), rate);
        assert_eq!(got.len(), 1, "{rate}");
        assert_eq!(got[0].packet, p);
    }
    for error in [-0.005, -0.001, 0.001, 0.005] {
        let got = mdc_decode(&mdc_burst(&p, 48e3 * (1.0 + error), 0.3, 250), 48e3);
        assert_eq!(got.len(), 1, "{error}");
        assert_eq!(got[0].packet, p);
    }
    assert!(BurstDecoder::new(7000.0).is_err());
}

#[test]
fn mdc_double_packet() {
    let p = Packet::double(0x35, 0x12, 0x4321, [0xDE, 0xAD, 0xBE, 0xEF]);
    let got = mdc_decode(&mdc_burst(&p, 48e3, 0.3, 77), 48e3);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].packet, p);
    assert!(got[0].packet.text().ends_with("(double packet)"));
    assert_eq!(
        Packet::new(1, 0x80, 0).text(),
        "PTT ID (start of transmission)"
    );
}

/// Speech-like audio at 48 kHz for `seconds` with the peak `peak`.
fn speech_48k(seconds: f64, peak: f32, seed: u64) -> Vec<f32> {
    let s = inversion::speech(seconds, seed);
    let mut out = Vec::with_capacity(4 * s.len());
    for w in s.windows(2) {
        for k in 0..4 {
            out.push((w[0] + (w[1] - w[0]) * k as f32 / 4.0) * peak / 0.5);
        }
    }
    out
}

#[test]
fn mdc_start_and_end_bursts_between_speech() {
    let start = Packet::new(0x01, 0x80, 0x1A2B);
    let end = Packet::new(0x01, 0x00, 0x1A2B);
    let mut x = speech_48k(2.0, 0.4, 1);
    x.extend(mdc::burst_audio(&start, 48e3, 0.3, 7));
    x.resize(x.len() + 9600, 0.0);
    x.extend(mdc::burst_audio(&end, 48e3, 0.3, 7));
    x.extend(speech_48k(2.0, 0.4, 2));
    let got = mdc_decode(&x, 48e3);
    assert_eq!(
        got.iter().map(|d| d.packet).collect::<Vec<_>>(),
        [start, end]
    );
}

#[test]
fn mdc_speech_and_noise_give_no_report() {
    for seed in 0..12 {
        let mut x = speech_48k(10.0, 0.4, 20 + seed);
        for (v, n) in x.iter_mut().zip(real_noise(480_000, 0.1, 40 + seed)) {
            *v += n;
        }
        assert!(mdc_decode(&x, 48e3).is_empty(), "seed {seed}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// ELT 121.5 MHz.

/// The detector after `seconds` of `g` at 12 kHz with the carrier at `offset_hz` and `snr_db` in 3 kHz.
fn elt_run(mut g: EltGenerator, seconds: f64, offset_hz: f64, snr_db: f64) -> SweepDetector {
    g.seconds = 600.0;
    let n = (seconds * g.rate) as usize;
    let x = take(&mut g, n);
    let x = with_snr(&shifted(&x, offset_hz, RATE), snr_db, 7);
    let mut d = SweepDetector::default();
    let mut changes = Vec::new();
    d.process(&x, &mut changes);
    d
}

#[test]
fn elt_is_confirmed_at_its_rates_duties_and_levels() {
    let d = elt_run(EltGenerator::new(RATE), 6.0, 0.0, 30.0);
    assert!(d.confirmed());
    assert!((d.rate_hz() - 3.0).abs() < 0.3, "{}", d.rate_hz());
    assert!(d.top_hz() > 1300.0 && d.bottom_hz() < 600.0);
    for rate in [2.0, 2.5, 3.5, 4.0] {
        // The clock runs 100 ppm fast.
        let mut g = EltGenerator::new(RATE * (1.0 + 100e-6));
        g.sweep_hz = rate;
        g.seconds = 600.0;
        let x = take(&mut g, 6 * RATE as usize);
        let x = with_snr(&shifted(&x, 800.0, RATE), 25.0, 3);
        let mut d = SweepDetector::default();
        d.process(&x, &mut Vec::new());
        assert!(d.confirmed(), "{rate} sweeps/s");
        assert!((d.rate_hz() - rate).abs() < 0.35, "{rate}: {}", d.rate_hz());
    }
    for duty in [0.35, 0.5, 0.66, 1.0] {
        let mut g = EltGenerator::new(RATE);
        g.duty = duty;
        assert!(elt_run(g, 6.0, -1500.0, 25.0).confirmed(), "duty {duty}");
    }
    for snr in [20.0, 10.0] {
        assert!(
            elt_run(EltGenerator::new(RATE), 6.0, 0.0, snr).confirmed(),
            "{snr} dB"
        );
    }
}

/// Narrow FM of speech at 12 kHz, amplitude 0.5.
fn fm_speech(seconds: f64, invert_hz: f64, seed: u64) -> Vec<Complex32> {
    let mut g = InversionGenerator::new(RATE, invert_hz, seconds, 2500.0, seed).unwrap();
    take(&mut g, (seconds * RATE) as usize)
}

/// A packet burst at 12 kHz, again and again.
fn packet_burst(seconds: f64) -> Vec<Complex32> {
    let mut g = PacketGenerator::new(RATE, 4, 0.3, 0.3, hfpacket::MARK_HZ, hfpacket::SPACE_HZ);
    take(&mut g, (seconds * RATE) as usize)
}

/// A carrier of amplitude 0.5 at 12 kHz with the envelope `1 + m(t)`.
fn am(seconds: f64, m: impl Fn(f64) -> f64) -> Vec<Complex32> {
    (0..(seconds * RATE) as usize)
        .map(|i| Complex32::new((0.5 * (1.0 + m(i as f64 / RATE))) as f32, 0.0))
        .collect()
}

#[test]
fn elt_is_not_confirmed_by_other_signals() {
    let mut upward = EltGenerator::new(RATE);
    (upward.top_hz, upward.bottom_hz) = (300.0, 1600.0);
    assert!(
        !elt_run(upward, 6.0, 0.0, 25.0).confirmed(),
        "an upward sweep"
    );
    let cases: [(&str, Vec<Complex32>); 5] = [
        ("a steady tone", am(6.0, |t| 0.9 * (TAU * 1000.0 * t).sin())),
        ("speech on FM", fm_speech(6.0, 0.0, 4)),
        ("a packet burst", packet_burst(6.0)),
        ("a plain carrier", am(6.0, |_| 0.0)),
        ("noise", noise(72_000, 0.01, 1)),
    ];
    for (name, x) in cases {
        let x = if name == "noise" {
            x
        } else {
            with_snr(&x, 20.0, 5)
        };
        let mut d = SweepDetector::default();
        let mut changes = Vec::new();
        d.process(&x, &mut changes);
        assert!(!d.confirmed() && changes.is_empty(), "{name}");
    }
}

#[test]
fn elt_through_the_stack() {
    let mut rx = receiver(&ELT121);
    let events = feed(rx.as_mut(), &stack_signal(&ELT121, 8.0), 50_000);
    assert_eq!(stage(rx.as_ref()), Some(Stage::Confirmed));
    let f = rx.results().fields();
    let rate = f.at(&typed::Elt121Fields::SWEEP_RATE_HZ).unwrap();
    assert!((rate - 3.0).abs() < 0.4, "{rate}");
    assert_eq!(f.at(&typed::Elt121Fields::CONFIRMED), Some(true));
    let status = rx.results().status();
    assert_eq!(status.lock_state, LockState::Locked);
    assert!(
        status.summary.contains("detection only"),
        "{}",
        status.summary
    );
    assert!(events.iter().any(|e| e.is(&typed::ELT121_FOUND)));
}

/// A change of frequency keeps the event list and forgets the tone, its sweeps and the level.
#[test]
fn elt_forgets_the_tone_and_keeps_the_events() {
    let rx = fed(&ELT121, 8.0);
    let f = rx.results().fields();
    assert_eq!(f.at(&typed::Elt121Fields::CONFIRMED), Some(true));
    assert!(f.at(&typed::DetectorFields::LEVEL_DBFS).is_some());
    let events = row_count(rx.as_ref(), "events");
    assert!(events >= 1);
    retune_then_forget(&ELT121, 8.0, |rx, step| {
        forgot_the_signal(rx, step);
        let f = rx.results().fields();
        assert_eq!(f.at(&typed::Elt121Fields::CONFIRMED), Some(false), "{step}");
        assert_eq!(f.at(&typed::Elt121Fields::SWEEP_RATE_HZ), None, "{step}");
        assert_eq!(f.at(&typed::Elt121Fields::DEPTH_PCT), None, "{step}");
        assert_eq!(f.at(&typed::DetectorFields::GOOD), Some(0), "{step}");
        assert_eq!(row_count(rx, "events"), events, "{step}");
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// HF packet.

/// The frames decoded from `x`: good ones as text, and the receiver.
fn packet_decode(x: &[Complex32]) -> (Vec<String>, AfskReceiver) {
    let mut rx =
        AfskReceiver::new(RATE, hfpacket::BAUD, hfpacket::MARK_HZ, hfpacket::SPACE_HZ).unwrap();
    let mut got = Vec::new();
    rx.process(x, |h| {
        if let Heard::Frame(b) = h {
            if let Ok(f) = sdrkit_proto_link::ax25::Frame::parse(b) {
                got.push(f.to_string());
            }
        }
    });
    (got, rx)
}

/// One burst of `frames` frames at 12 kHz sampled at `rate`, then silence.
fn one_burst(frames: usize, rate: f64) -> Vec<Complex32> {
    let mut g = PacketGenerator::new(
        rate,
        frames,
        0.3,
        0.5,
        hfpacket::MARK_HZ,
        hfpacket::SPACE_HZ,
    );
    let n = g.period() as usize;
    take(&mut g, n)
}

#[test]
fn packet_clean_frames() {
    let (got, _) = packet_decode(&one_burst(4, RATE));
    assert_eq!(got.len(), 4, "{got:?}");
    for (i, line) in got.iter().enumerate() {
        assert_eq!(
            line,
            &format!("TEST-1>BEACON:sdrkit HF packet test frame {}", i + 1)
        );
    }
}

#[test]
fn packet_with_offsets_clock_error_and_noise() {
    for offset in [-40.0, 0.0, 35.0] {
        let x = with_snr(&shifted(&one_burst(8, RATE), offset, RATE), 25.0, 2);
        let (got, rx) = packet_decode(&x);
        assert!(got.len() >= 7, "{offset} Hz: {}", got.len());
        if offset != 0.0 {
            assert!(
                (rx.tone_offset_hz() - offset).abs() < 20.0,
                "{}",
                rx.tone_offset_hz()
            );
        }
    }
    let x = with_snr(&one_burst(8, RATE * (1.0 + 300e-6)), 25.0, 3);
    assert!(packet_decode(&x).0.len() >= 7, "clock +300 ppm");
    let x = with_snr(&one_burst(8, RATE), 18.0, 4);
    assert!(packet_decode(&x).0.len() >= 7, "18 dB");
    for snr in [12.0, 8.0] {
        let n = packet_decode(&with_snr(&one_burst(8, RATE), snr, 5))
            .0
            .len();
        println!("HF packet at {snr} dB: {n} of 8 frames");
    }
}

#[test]
fn packet_is_not_found_in_other_signals() {
    let mut elt = EltGenerator::new(RATE);
    let cases = [
        ("an ELT", with_snr(&take(&mut elt, 72_000), 20.0, 1)),
        ("speech on FM", with_snr(&fm_speech(6.0, 0.0, 2), 20.0, 2)),
        ("noise", noise(120_000, 0.01, 3)),
    ];
    for (name, x) in cases {
        let (got, rx) = packet_decode(&x);
        assert!(got.is_empty(), "{name}");
        if name == "noise" {
            assert!(rx.signal_blocks() <= 3, "{name}: {}", rx.signal_blocks());
        }
    }
}

#[test]
fn packet_through_the_stack() {
    let mut rx = receiver(&HFPACKET);
    let events = feed(rx.as_mut(), &stack_signal(&HFPACKET, 8.0), 50_000);
    let f = rx.results().fields();
    assert!(f.at(&typed::DetectorFields::GOOD).unwrap() >= 3);
    assert_eq!(stage(rx.as_ref()), Some(Stage::Confirmed));
    assert_eq!(f.at(&typed::HfpacketFields::STATIONS), Some(1));
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    let frames: Vec<_> = events
        .iter()
        .filter(|e| e.is(&typed::HFPACKET_FRAME))
        .collect();
    assert!(frames.len() >= 3);
    assert_eq!(frames[0].get(&keys::SOURCE), Some("TEST-1"));
    assert_eq!(
        frames[0].at(&typed::HfpacketFrameRow::DESTINATION),
        Some("BEACON")
    );
    assert!(
        frames[0].text("text").ends_with("[UI]"),
        "{}",
        frames[0].text("text")
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e.is(&typed::HFPACKET_STATION))
            .count(),
        1
    );
    // The command clear empties the lists.
    let id = rx.commands().id("clear").unwrap();
    rx.invoke(id, &Values::defaults(&Default::default()))
        .unwrap();
    let f = rx.results().fields();
    assert_eq!(f.at(&typed::DetectorFields::GOOD), Some(0));
    assert_eq!(f.at(&typed::HfpacketFields::STATIONS), Some(0));
}

/// A change of frequency keeps the frames, their counts, the stations and the last frame, and forgets the signal.
#[test]
fn packet_keeps_the_frames_and_forgets_the_signal() {
    let rx = fed(&HFPACKET, 8.0);
    let f = rx.results().fields();
    let good = f.at(&typed::DetectorFields::GOOD).unwrap();
    let bad = f.at(&typed::DetectorFields::BAD);
    let last = f.at(&typed::HfpacketFields::LAST_FRAME).map(str::to_string);
    assert!(good >= 3 && last.is_some());
    assert!(f.at(&typed::DetectorFields::LEVEL_DBFS).is_some());
    let frames = row_count(rx.as_ref(), "frames");
    let events = row_count(rx.as_ref(), "events");
    assert!(frames >= 3 && events > frames);
    retune_then_forget(&HFPACKET, 8.0, |rx, step| {
        forgot_the_signal(rx, step);
        let f = rx.results().fields();
        assert_eq!(f.at(&typed::DetectorFields::GOOD), Some(good), "{step}");
        assert_eq!(f.at(&typed::DetectorFields::BAD), bad, "{step}");
        assert_eq!(f.at(&typed::HfpacketFields::STATIONS), Some(1), "{step}");
        assert_eq!(
            f.at(&typed::HfpacketFields::LAST_FRAME),
            last.as_deref(),
            "{step}"
        );
        assert_eq!(row_count(rx, "frames"), frames, "{step}");
        assert_eq!(row_count(rx, "events"), events, "{step}");
    });
}

#[test]
fn packet_on_the_lower_sideband() {
    let desc = &HFPACKET;
    let mut values = defaults(desc);
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)()).unwrap();
    values
        .set(schema.id("sideband").unwrap(), &Value::Choice(1))
        .unwrap();
    let mut rx = (desc.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(250e3, 14.12e6, 14.1e6).recording())
        .unwrap();
    // The mirror image of the test signal about the dial.
    let x: Vec<Complex32> = {
        let mut g = PacketGenerator::new(250e3, 4, 0.3, 0.3, 1600.0, 1800.0);
        let x: Vec<Complex32> = take(&mut g, 2_000_000).iter().map(|s| s.conj()).collect();
        shifted(&x, -20e3, 250e3)
    };
    feed(rx.as_mut(), &x, 65_536);
    assert!(
        rx.results()
            .fields()
            .at(&typed::DetectorFields::GOOD)
            .unwrap()
            >= 3
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Voice inversion.

/// The detector and its discriminator audio after `x`.
fn inversion_run(x: &[Complex32]) -> (InversionDetector, Vec<f32>) {
    let mut d = InversionDetector::default();
    let mut audio = Vec::new();
    d.process(x, &mut audio, &mut Vec::new());
    (d, audio)
}

#[test]
fn inversion_is_found_with_its_carrier() {
    for carrier in [2800.0, 3000.0, 3300.0, 3500.0] {
        let x = with_snr(&shifted(&fm_speech(10.0, carrier, 1), 120.0, RATE), 28.0, 6);
        let (d, _) = inversion_run(&x);
        let found = d.carrier_hz().unwrap_or(0.0);
        assert!((found - carrier).abs() <= 200.0, "{carrier}: found {found}");
    }
    for snr in [28.0, 22.0] {
        // 200 Hz low, the clock 150 ppm fast.
        let mut g =
            InversionGenerator::new(RATE * (1.0 + 150e-6), 3000.0, 12.0, 2500.0, 2).unwrap();
        let x = with_snr(&shifted(&take(&mut g, 120_000), -200.0, RATE), snr, 8);
        let found = inversion_run(&x).0.carrier_hz().unwrap_or(0.0);
        assert!((found - 3000.0).abs() <= 200.0, "{snr} dB: found {found}");
    }
}

#[test]
fn clear_speech_and_other_signals_are_not_inverted() {
    let (d, _) = inversion_run(&with_snr(&fm_speech(10.0, 0.0, 1), 28.0, 6));
    assert!(d.carrier_hz().is_none());
    assert!(d.clear_speech() && d.activity());
    let cases = [
        ("a plain carrier", with_snr(&am(10.0, |_| 0.0), 22.0, 1)),
        ("a packet burst", with_snr(&packet_burst(10.0), 22.0, 2)),
        ("noise", noise(120_000, 0.01, 3)),
    ];
    for (name, x) in cases {
        assert!(inversion_run(&x).0.carrier_hz().is_none(), "{name}");
    }
}

/// The long-term spectrum of `x` (12 kHz) in 100 Hz bands from 300 to 2900 Hz, dB.
fn band_spectrum(x: &[f32]) -> Vec<f64> {
    let mut bands = vec![0.0f64; 26];
    for frame in x.as_chunks::<240>().0 {
        for (b, power) in bands.iter_mut().enumerate() {
            for f in [325.0 + 100.0 * b as f64, 375.0 + 100.0 * b as f64] {
                let (mut re, mut im) = (0.0, 0.0);
                for (n, &v) in frame.iter().enumerate() {
                    let w = 0.5 - 0.5 * (TAU * n as f64 / 240.0).cos();
                    let (s, c) = (TAU * f * n as f64 / RATE).sin_cos();
                    re += w * f64::from(v) * c;
                    im += w * f64::from(v) * s;
                }
                *power += re * re + im * im;
            }
        }
    }
    bands.iter().map(|p| 10.0 * p.max(1e-30).log10()).collect()
}

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    sab / (saa * sbb).sqrt()
}

#[test]
fn the_descrambled_audio_has_the_spectrum_of_the_clear_speech() {
    for carrier in [3000.0, 3300.0] {
        let clear = inversion::speech(10.0, 2);
        let x = with_snr(&fm_speech(10.0, carrier, 1), 28.0, 6);
        let mut d = InversionDetector::default();
        let mut inverter = Inverter::new(None, 0.5);
        let (mut scrambled, mut out) = (Vec::new(), Vec::new());
        for c in x.chunks(1200) {
            let from = scrambled.len();
            d.process(c, &mut scrambled, &mut Vec::new());
            inverter.set_carrier(d.carrier_hz());
            inverter.process(&scrambled[from..], &mut out);
        }
        // The part after the carrier was found.
        let clear_bands = band_spectrum(&clear[60_000..]);
        let fixed = correlation(&band_spectrum(&out[60_000..]), &clear_bands);
        let raw = correlation(&band_spectrum(&scrambled[60_000..]), &clear_bands);
        assert!(
            fixed > 0.7 && fixed > raw + 0.25,
            "{carrier}: {fixed} against {raw}"
        );
    }
}

#[test]
fn a_set_carrier_gives_back_the_waveform() {
    let clear = inversion::speech(10.0, 2);
    let (_, audio) = inversion_run(&with_snr(&fm_speech(10.0, 3000.0, 1), 40.0, 6));
    let mut out = Vec::new();
    Inverter::new(Some(3000.0), 0.5).process(&audio, &mut out);
    let spectrum = correlation(
        &band_spectrum(&out[12_000..]),
        &band_spectrum(&clear[12_000..]),
    );
    assert!(spectrum > 0.9, "{spectrum}");
    let best = (0..400)
        .map(|delay| {
            let (a, b) = (
                &clear[12_000..100_000],
                &out[12_000 + delay..100_000 + delay],
            );
            let dot: f64 = a.iter().zip(b).map(|(x, y)| f64::from(x * y)).sum();
            let (pa, pb): (f64, f64) = (
                a.iter().map(|x| f64::from(x * x)).sum(),
                b.iter().map(|x| f64::from(x * x)).sum(),
            );
            dot / (pa * pb).sqrt()
        })
        .fold(f64::MIN, f64::max);
    assert!(best > 0.5, "{best}");
}

#[test]
fn inversion_through_the_stack() {
    let mut rx = receiver(&VOICE_INVERSION);
    let x = stack_signal(&VOICE_INVERSION, 12.0);
    let mut out = Output::default();
    let mut audio = 0;
    let mut found = false;
    for c in x.chunks(50_000) {
        rx.feed(c, &mut out);
        audio += out.audio().len();
        assert!(out.audio_format().is_none_or(|f| f == (48e3, 1)));
        found |= out
            .events
            .iter()
            .any(|e| e.is(&typed::VOICE_INVERSION_FOUND));
        out.clear();
    }
    assert_eq!(stage(rx.as_ref()), Some(Stage::Confirmed));
    let f = rx.results().fields();
    let carrier = f.at(&typed::VoiceInversionFields::CARRIER_HZ).unwrap();
    assert!((carrier - 3000.0).abs() <= 200.0, "{carrier}");
    assert_eq!(f.at(&typed::VoiceInversionFields::INVERTED), Some(true));
    assert!(found);
    assert!(
        (audio as f64 - 12.0 * 48e3).abs() < 4800.0,
        "{audio} audio samples"
    );
    assert!(rx.results().status().summary.contains("descrambled"));
}

/// A change of frequency keeps the event list and forgets the carrier, the fits and the level.
#[test]
fn inversion_forgets_the_carrier_and_keeps_the_events() {
    let rx = fed(&VOICE_INVERSION, 12.0);
    let f = rx.results().fields();
    assert_eq!(f.at(&typed::VoiceInversionFields::INVERTED), Some(true));
    assert!(f.at(&typed::DetectorFields::LEVEL_DBFS).is_some());
    let events = row_count(rx.as_ref(), "events");
    assert!(events >= 1);
    retune_then_forget(&VOICE_INVERSION, 12.0, |rx, step| {
        forgot_the_signal(rx, step);
        let f = rx.results().fields();
        assert_eq!(
            f.at(&typed::VoiceInversionFields::INVERTED),
            Some(false),
            "{step}"
        );
        assert_eq!(
            f.at(&typed::VoiceInversionFields::CARRIER_HZ),
            None,
            "{step}"
        );
        assert_eq!(
            f.at(&typed::VoiceInversionFields::FIT_HEARD),
            None,
            "{step}"
        );
        assert_eq!(row_count(rx, "events"), events, "{step}");
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// All stacks.

#[test]
fn noise_alone_gives_nothing() {
    for desc in [&HFPACKET, &ELT121, &VOICE_INVERSION] {
        let mut rx = receiver(desc);
        let events = feed(rx.as_mut(), &noise(1_500_000, 0.01, 4), 65_536);
        assert!(events.is_empty(), "{}", desc.id);
        assert_eq!(stage(rx.as_ref()), Some(Stage::Searching), "{}", desc.id);
        assert_eq!(rx.results().status().lock_state, LockState::Searching);
    }
}

#[test]
fn a_rate_below_the_lowest_is_refused() {
    for desc in [&HFPACKET, &ELT121, &VOICE_INVERSION] {
        let mut rx = (desc.make_receiver)(&StackConfig::new(defaults(desc))).unwrap();
        let e = rx
            .configure(&InputInfo::new(100e3, 14.12e6, 14.1e6))
            .unwrap_err();
        assert!(e.to_string().contains("250"), "{}: {e}", desc.id);
    }
}

#[test]
fn settings_refuse_values_outside_their_range() {
    let schema = (VOICE_INVERSION.settings)();
    let id = schema.id("carrier_hz").unwrap();
    assert!(schema.check(id, &Value::Hz(5000.0)).is_err());
    assert!(schema.check(id, &Value::Hz(3000.0)).is_ok());
    let mut rx =
        (VOICE_INVERSION.make_receiver)(&StackConfig::new(defaults(&VOICE_INVERSION))).unwrap();
    assert!(rx.apply(id, &Value::Hz(1000.0)).is_err());
    assert!(rx.apply(id, &Value::Hz(0.0)).is_ok());
    assert!(rx.apply(id, &Value::Hz(2700.0)).is_ok());
    let schema = (HFPACKET.settings)();
    assert!(schema
        .check(schema.id("sideband").unwrap(), &Value::Choice(2))
        .is_err());
    let schema = (ELT121.settings)();
    assert!(schema
        .check(schema.id("gen_duty_pct").unwrap(), &Value::F64(5.0))
        .is_err());
}

#[test]
fn results_follow_the_signal_time() {
    for desc in [&HFPACKET, &ELT121] {
        let x = stack_signal(desc, 4.0);
        let run = |chunk: usize| {
            let mut rx = receiver(desc);
            let events = feed(rx.as_mut(), &x, chunk);
            let f = rx.results().fields();
            (
                events.len(),
                f.at(&typed::DetectorFields::GOOD),
                f.at(&typed::DetectorFields::SIGNAL_TIME_S),
                stage(rx.as_ref()),
            )
        };
        assert_eq!(run(10_000), run(250_000), "{}", desc.id);
    }
}
