//! Helpers of the tests: slots of audio built from the encoders with the family's SNR convention, noise and impairments from
//! `sdrkit-algo-sim`, and the receiver fed in pieces.

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::publish::keys;
use sdrkit_core::{
    Choice, Complex32, Event, InputInfo, Output, Receiver, Results, StackConfig, Value, Values,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_wsjt::encode::{audio, Transmission};
use sdrkit_stack_wsjt::receiver;
use sdrkit_stack_wsjt::runner::{decode_slot, DecodeState};
use sdrkit_stack_wsjt::rx::slot::{Decode, Slot};
use sdrkit_stack_wsjt::slots::TimeChoice;
use sdrkit_stack_wsjt::typed::DECODE;
use sdrkit_stack_wsjt::{Mode, Sub};
use std::sync::Arc;

/// Noise sigma per real sample at 8 kHz.
pub const SIGMA: f64 = 0.05;

/// The start of the recordings and of the test slots: 2026-01-01T00:00:00Z.
pub const START: f64 = 1_767_225_600.0;

/// The amplitude of a tone of `snr` dB in 2500 Hz.
pub fn amp(snr: f64) -> f32 {
    (2.0 * 10f64.powf(snr / 10.0) * SIGMA * SIGMA * 2500.0 / 4000.0).sqrt() as f32
}

/// `n` samples of real Gaussian noise of standard deviation `sigma`, seeded.
pub fn noise(n: usize, sigma: f64, seed: u64) -> Vec<f32> {
    let mut a = vec![0f32; n];
    Awgn::with_power(sigma * sigma, seed)
        .unwrap()
        .process_real(&mut a);
    a
}

/// Adds complex noise to `iq` (rate `rate_hz`) so that a signal of unit power has `snr_db` in 2500 Hz.
pub fn add_noise(iq: &mut [Complex32], snr_db: f64, rate_hz: f64, seed: u64) {
    let level = NoiseLevel::SnrDb {
        snr_db,
        bandwidth_hz: 2500.0,
    };
    Awgn::complex(level, 1.0, rate_hz, seed)
        .unwrap()
        .process(iq);
}

/// Moves `iq` by `hz`.
pub fn shift(iq: &mut [Complex32], rate_hz: f64, hz: f64) {
    sdrkit_algo_sim::offset::FrequencyOffset::new(rate_hz, hz, 0.0)
        .unwrap()
        .process(iq);
}

/// `iq` as a receiver whose sample clock runs `ppm` fast sees it.
pub fn clock_error(iq: &[Complex32], ppm: f64) -> Vec<Complex32> {
    let mut out = Vec::new();
    sdrkit_algo_sim::clock::ClockError::new(ppm)
        .unwrap()
        .process(iq, &mut out);
    out
}

/// A station: text, Hz, SNR dB, DT s, upside down.
pub type St<'a> = (&'a str, f64, f64, f64, bool);

/// `seconds` of 8 kHz audio holding the stations in every slot of `sub` from 0 s, plus noise (sigma 0.05 when `noise`).
pub fn audio_of(sub: Sub, stations: &[St], seconds: f64, with_noise: bool, seed: u64) -> Vec<f32> {
    let n = (seconds * 8000.0) as usize;
    let mut a = if with_noise {
        noise(n, SIGMA, seed)
    } else {
        vec![0f32; n]
    };
    let slots = (seconds / sub.period).ceil() as usize + 1;
    for &(text, hz, snr, dt, mir) in stations {
        let s = audio(
            &Transmission {
                sub,
                hz,
                amp: amp(snr),
                mirrored: mir,
            },
            text,
            8000.0,
        )
        .unwrap();
        for k in 0..slots {
            let start = ((k as f64 * sub.period + sub.t0() + dt) * 8000.0).round() as i64;
            for (i, v) in s.iter().enumerate() {
                let j = start + i as i64;
                if j >= 0 && (j as usize) < n {
                    a[j as usize] += v;
                }
            }
        }
    }
    a
}

/// The slots of `audio` (first sample = a slot start) for `sub`, as the cutter makes them at the end of a recording.
pub fn slots(sub: Sub, audio: &[f32]) -> Vec<Slot> {
    let (w0, w1) = sub.window();
    let len = audio.len() as f64 / 8000.0;
    let mut out = Vec::new();
    let mut k = 0i64;
    loop {
        let s0 = k as f64 * sub.period;
        if s0 + sub.t0() + sub.tx_s() / 2.0 > len {
            break;
        }
        let a = (s0 + w0).max(0.0);
        let b = (s0 + w1).min(len);
        let (i0, i1) = (
            (a * 8000.0) as usize,
            ((b * 8000.0) as usize).min(audio.len()),
        );
        out.push(Slot {
            sub,
            utc: s0,
            audio: Arc::new(audio[i0..i1].to_vec()),
            rate: 8000.0,
            t_first: a - s0,
            search: false,
            dial_hz: 0.0,
            index: k,
            early: false,
            seq: 0,
        });
        k += 1;
    }
    out
}

/// Decodes every slot with one decoder state.
pub fn decode_all(sub: Sub, audio: &[f32]) -> Vec<Decode> {
    decode_with(sub, audio, DecodeState::default())
}

/// Decodes every slot with `state`.
pub fn decode_with(sub: Sub, audio: &[f32], mut state: DecodeState) -> Vec<Decode> {
    slots(sub, audio)
        .iter()
        .flat_map(|s| decode_slot(s, &mut state).decodes)
        .collect()
}

/// Audio as IQ (the real signal: its positive-frequency half is the upper sideband).
pub fn iq_of(a: &[f32]) -> Vec<Complex32> {
    a.iter().map(|&x| Complex32::new(x, 0.0)).collect()
}

/// The settings of `mode` for a recording that starts at [`START`], with these extra settings. A refused setting fails the
/// test.
pub fn settings(mode: Option<Mode>, extra: &[(&str, Value)]) -> Values {
    let schema = sdrkit_stack_wsjt::settings::schema_for(mode);
    let mut s = Values::defaults(&schema);
    let mut put = |k: &str, v: &Value| {
        let id = schema.id(k).unwrap_or_else(|e| panic!("{k}: {e}"));
        s.set(id, v).unwrap_or_else(|e| panic!("{k}: {e}"));
    };
    put("time_source", &Value::Choice(TimeChoice::Start.index()));
    put("time_start", &Value::Text("2026-01-01T00:00:00Z".into()));
    for (k, v) in extra {
        put(k, v);
    }
    s
}

/// A receiver of `mode` with these extra settings, configured for a recording at `rate` with the dial at `dial_offset` from the
/// centre. A refused setting fails the test.
pub fn receiver(
    mode: Option<Mode>,
    extra: &[(&str, Value)],
    rate: f64,
    dial_offset: f64,
) -> StackGraph {
    let cfg = StackConfig::new(settings(mode, extra));
    let mut rx = receiver::receiver(mode, &cfg).unwrap();
    rx.configure(&InputInfo::new(rate, 14e6, 14e6 + dial_offset).recording())
        .unwrap();
    rx
}

/// Feeds IQ in pieces, finishes, and returns the events.
pub fn feed(rx: &mut StackGraph, iq: &[Complex32], piece: usize) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in iq.chunks(piece) {
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
        out.clear();
    }
    rx.finish(&mut out);
    ev.append(&mut out.events);
    ev
}

/// The texts of decode events.
pub fn texts(ev: &[Event]) -> Vec<String> {
    ev.iter()
        .filter(|e| e.is(&DECODE))
        .map(|e| e.get(&keys::TEXT).unwrap_or_default().to_string())
        .collect()
}

/// A number of the record `r` by its key.
pub fn record_num(r: &sdrkit_core::Record, key: &str) -> f64 {
    let id = r
        .decl()
        .id(key)
        .unwrap_or_else(|| panic!("{key} is not declared"));
    match r.value(id) {
        Some(Value::F64(v) | Value::Hz(v) | Value::Duration(v) | Value::Time(v)) => *v,
        Some(Value::I64(v)) => *v as f64,
        other => panic!("{key}: {other:?}"),
    }
}

/// A number of an event by its key.
pub fn num(e: &Event, key: &str) -> f64 {
    record_num(&e.fields, key)
}

/// A field of the results as a number, by its key.
pub fn field_f64(r: &Results, key: &str) -> f64 {
    record_num(r.fields(), key)
}

/// The value of the field `key` of the results.
pub fn field<'a>(r: &'a Results, key: &str) -> Option<&'a Value> {
    r.fields().value(r.decl().field_id(key)?)
}

/// The rows of the table `key` of the results.
pub fn table<'a>(r: &'a Results, key: &str) -> &'a sdrkit_core::Table {
    r.table(
        r.decl()
            .table_id(key)
            .unwrap_or_else(|| panic!("no table {key}")),
    )
    .unwrap_or_else(|| panic!("no table {key}"))
}

/// The key of the choice value `v` of field `key` of the results.
pub fn choice_key<'a>(r: &'a Results, key: &str) -> &'a str {
    let id = r.decl().field_id(key).unwrap();
    match r.fields().value(id) {
        Some(Value::Choice(i)) => {
            &r.fields().decl().field(id).unwrap().choices[usize::from(*i)].key
        }
        other => panic!("{key}: {other:?}"),
    }
}

/// Counts the stations decoded (by text) and the false decodes.
pub fn score(d: &[Decode], sent: &[&str]) -> (usize, usize) {
    let ok = d.iter().filter(|x| sent.contains(&x.text.as_str())).count();
    (ok, d.len() - ok)
}

/// A seeded uniform random number source for test layouts.
pub fn rng(seed: u64) -> sdrkit_algo_sim::rng::Rng {
    sdrkit_algo_sim::rng::Rng::new(seed)
}
