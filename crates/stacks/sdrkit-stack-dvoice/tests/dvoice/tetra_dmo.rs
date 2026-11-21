//! TETRA DMO: the test call through the receiver over rates, offsets, clocks and noise, with clear speech, air interface and
//! end-to-end encryption, and noise alone.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::codec::{AudioCodec, CodecError, CodecSet, Pcm};
use sdrkit_core::{FourCc, GenConfig, InputInfo, Output, Owner, Receiver};
use sdrkit_dev_test::{Awgn, ClockError, FrequencyOffset};
use sdrkit_stack_dvoice::tetra_dmo::{render_slots, test_bursts, TestCall, TETRA_DMO_RATE};
use serde_json::Value as Json;
use std::sync::{Arc, Mutex};

/// The test call at `rate` (the channel at the centre), its sent traffic bits, after 0.1 s of silence.
fn call(c: &TestCall, rate: f64) -> (Vec<Complex32>, Vec<Vec<u8>>) {
    let (slots, sent) = test_bursts(c, 3);
    let x72 = render_slots(&slots);
    let mut r =
        sdrkit_algo_dsp::resample::Resampler::with_quality(TETRA_DMO_RATE, rate, 0.46, 70.0)
            .unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (0.1 * rate) as usize];
    if (rate - TETRA_DMO_RATE).abs() < 1.0 {
        x.extend_from_slice(&x72);
    } else {
        r.process(&x72, &mut x);
    }
    (x, sent)
}

/// A codec plug-in that keeps the payload of every speech frame it is given.
struct Capture(Arc<Mutex<Vec<Vec<u8>>>>);
impl AudioCodec for Capture {
    fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
        if !erasure {
            self.0.lock().unwrap().push(frame.to_vec());
        }
        pcm.rate_hz = 8000;
        pcm.channels = 1;
        pcm.samples.extend(std::iter::repeat_n(0, 240));
        Ok(())
    }
}

/// Runs `x` at `rate` through a new receiver, the channel `channel_hz` from the centre; returns the view, the payloads of the
/// speech frames the codec was given and the receiver.
fn receive(x: &[Complex32], rate: f64, channel_hz: f64) -> (Json, Vec<Vec<u8>>, Box<dyn Receiver>) {
    let frames = Arc::new(Mutex::new(Vec::new()));
    let keep = Arc::clone(&frames);
    let mut codecs = CodecSet::default();
    codecs.register(
        FourCc::new("acel"),
        None,
        Arc::new(move |_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Capture(Arc::clone(&keep))) as Box<dyn AudioCodec>)
        }),
    );
    let mut rx = receiver_with_codecs("tetra_dmo", &[], codecs);
    rx.configure(&InputInfo::new(rate, 420e6 - channel_hz, 420e6).recording())
        .expect("configure");
    let mut out = Output::default();
    let zeros = vec![Complex32::new(0.0, 0.0); (0.5 * rate) as usize];
    for c in x.chunks(4800).chain(zeros.chunks(4800)) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let got = frames.lock().unwrap().clone();
    (view(Owner::results(rx.as_ref())), got, rx)
}

fn num(r: &dyn Receiver, key: &str) -> f64 {
    field(Owner::results(r), key).as_f64().unwrap_or(0.0)
}

/// The speech frames delivered, and how many of them lie within 8 bits of a frame that was sent.
fn speech(payloads: &[Vec<u8>], sent: &[Vec<u8>]) -> (usize, usize) {
    let frames: Vec<Vec<u8>> = sent
        .iter()
        .flat_map(|b| [b[..137].to_vec(), b[216..353].to_vec()])
        .collect();
    // A `tchs137` payload holds one bit a byte.
    let got: Vec<Vec<u8>> = payloads
        .iter()
        .map(|b| b.iter().map(|&x| x & 1).collect())
        .collect();
    let close = got
        .iter()
        .filter(|g| {
            frames
                .iter()
                .any(|f| f.iter().zip(g.iter()).filter(|(a, b)| a != b).count() <= 8)
        })
        .count();
    (got.len(), close)
}

fn check_call(
    v: &Json,
    rx: &dyn Receiver,
    out: &[Vec<u8>],
    sent: &[Vec<u8>],
    what: &str,
    src: u64,
    dst: u64,
) {
    assert!(num(rx, "dsb") >= 4.0, "{what}: {} DSB", num(rx, "dsb"));
    let c = calls(v);
    assert!(
        c.iter().any(|c| c["src"] == src && c["dst"] == dst),
        "{what}: {}",
        show(&c)
    );
    assert_eq!(
        field(Owner::results(rx), "mni").as_str(),
        Some("262/1001"),
        "{what}"
    );
    let delivered = num(rx, "traffic_bursts") - num(rx, "traffic_refused");
    assert!(
        delivered >= 32.0,
        "{what}: {delivered} traffic bursts delivered"
    );
    let (frames, close) = speech(out, sent);
    assert!(frames >= 64, "{what}: {frames} speech frames");
    assert!(
        close * 10 >= frames * 8,
        "{what}: {close} of {frames} frames within 8 bits"
    );
}

#[test]
fn the_test_call_over_rates_offsets_clocks_and_noise() {
    let base = TestCall::default();
    let cases: Vec<(&str, f64, f64, f64, f64)> = vec![
        ("72 ksps", 72e3, 0.0, 0.0, 0.0),
        ("240 ksps", 240e3, 0.0, 0.0, 0.0),
        ("2.4 Msps, 40 kHz off centre", 2.4e6, 40e3, 0.0, 0.0),
        ("+800 Hz", 72e3, 800.0, 0.0, 0.0),
        ("-1800 Hz", 72e3, -1800.0, 0.0, 0.0),
        ("+100 ppm", 72e3, 0.0, 100.0, 0.0),
        ("sigma 0.02", 72e3, 0.0, 0.0, 0.02),
        ("sigma 0.05", 72e3, 0.0, 0.0, 0.05),
    ];
    for (what, rate, offset, ppm, sigma) in cases {
        let (x, sent) = call(&base, rate);
        let mut y = Vec::new();
        ClockError::new(rate, ppm).process(&x, &mut y);
        FrequencyOffset::new(rate, offset).process(&mut y);
        if sigma > 0.0 {
            Awgn::with_sigma(sigma, 5).process(&mut y);
        }
        let channel = if offset.abs() > 10e3 { offset } else { 0.0 };
        let (v, out, rx) = receive(&y, rate, channel);
        check_call(&v, rx.as_ref(), &out, &sent, what, 0x1A2B3C, 0x4D5E6F);
    }
}

#[test]
fn communication_type_1_and_an_individual_call() {
    let c = TestCall {
        comm_type: 1,
        group: false,
        destination: 0x000042,
        ..TestCall::default()
    };
    let (x, sent) = call(&c, 72e3);
    let (v, out, rx) = receive(&x, 72e3, 0.0);
    check_call(&v, rx.as_ref(), &out, &sent, "type 1", 0x1A2B3C, 0x42);
    assert_eq!(num(rx.as_ref(), "comm_type"), 1.0);
}

#[test]
fn air_interface_encryption() {
    let c = TestCall {
        aie: 1,
        ..TestCall::default()
    };
    let (x, _) = call(&c, 72e3);
    let (v, out, rx) = receive(&x, 72e3, 0.0);
    assert!(num(rx.as_ref(), "dsb") >= 4.0);
    assert!(out.is_empty());
    assert_eq!(num(rx.as_ref(), "aie_state"), 1.0);
    assert!(num(rx.as_ref(), "traffic_refused") >= 32.0);
    let calls = calls(&v);
    assert!(
        calls.iter().any(|c| c["encryption"]["state"] != "clear"),
        "{}",
        show(&calls)
    );
}

#[test]
fn end_to_end_encryption() {
    let c = TestCall {
        e2e: true,
        ..TestCall::default()
    };
    let (x, _) = call(&c, 72e3);
    let (_, out, rx) = receive(&x, 72e3, 0.0);
    assert!(num(rx.as_ref(), "dsb") >= 4.0);
    assert!(out.is_empty());
}

#[test]
fn noise_gives_no_dsb() {
    let x = Awgn::with_sigma(0.05, 9).noise((72e3 * 4.0) as usize);
    let (_, _, rx) = receive(&x, 72e3, 0.0);
    assert_eq!(num(rx.as_ref(), "dsb"), 0.0);
}

#[test]
fn the_test_signal_with_the_default_settings() {
    let d = desc("tetra_dmo");
    let rate = d.tuning.rate_hz;
    let mut g =
        (d.make_generator.unwrap())(&GenConfig::new(rate, values("tetra_dmo", &[]))).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 4.0) as usize];
    g.generate(&mut x);
    let (v, _, rx) = receive(&x, rate, -d.tuning.tune_offset_hz);
    assert!(num(rx.as_ref(), "dsb") >= 4.0);
    assert!(!calls(&v).is_empty());
    assert!(field(Owner::results(rx.as_ref()), "note")
        .as_str()
        .is_some_and(|t| t.contains("Not confirmed by a public document")));
}

#[test]
fn the_data_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/dvoice/tetra_dmo.json");
    let text = std::fs::read_to_string(&path).expect("the data file");
    let mut plans = sdrkit_data_plans::Plans::new();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    let file = sdrkit_data_plans::data::parse("tetra_dmo.json", &text).expect("a plan file");
    assert_eq!(file.groups.len(), 1);
    assert_eq!(file.groups[0].bands.len(), 1);
    assert_eq!(
        (
            file.groups[0].bands[0].start_hz,
            file.groups[0].bands[0].end_hz
        ),
        (30e6, 1000e6)
    );
}

#[test]
fn rate_retune_clear_and_saved_state() {
    let mut rx = receiver("tetra_dmo", &[]);
    assert!(rx.configure(&InputInfo::new(48e3, 420e6, 420e6)).is_err());
    let (x, _) = call(&TestCall::default(), 72e3);
    let (v, _, mut rx) = receive(&x, 72e3, 0.0);
    let n = calls(&v).len();
    assert!(n > 0);
    let saved = rx.save_state().expect("a saved state");
    rx.retuned(&InputInfo::new(72e3, 421e6, 421e6).recording());
    let mut out = Output::default();
    rx.feed(&vec![Complex32::new(0.0, 0.0); 7200], &mut out);
    assert_eq!(calls(&view(Owner::results(rx.as_ref()))).len(), n);
    let id = rx.commands().id("clear").expect("a clear command");
    let args = rx.commands().defs()[id.index()].new_args();
    sdrkit_core::owner::invoke(rx.as_mut(), id, &args).expect("clear runs");
    rx.feed(&vec![Complex32::new(0.0, 0.0); 7200], &mut out);
    assert!(calls(&view(Owner::results(rx.as_ref()))).is_empty());
    let mut back = receiver("tetra_dmo", &[]);
    back.configure(&InputInfo::new(72e3, 420e6, 420e6).recording())
        .unwrap();
    back.restore_state(&saved).expect("the state is taken");
    back.feed(&vec![Complex32::new(0.0, 0.0); 7200], &mut out);
    assert_eq!(calls(&view(Owner::results(back.as_ref()))).len(), n);
}
