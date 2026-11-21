//! The test signals of the CIS stack, one per submode. The 2-FSK and MFSK signals are tone sequences; the PSK and OFDM signals
//! are made once at 24 kHz and played at any rate. Each repeats without end; the content comes from fixed seeds, so a test can
//! make the same symbols again.

use super::fsk::{valid, EOM};
use super::{submode_of, Submode, GEN};
use crate::serial::gen::test_dial;
use crate::teleprinter::ToneSeqGen;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The BEE variant.
    pub enum BeeVariant {
        /// Idle at 36 Bd, traffic at 50 Bd, 200 Hz shift.
        V36 = ("36-50", "CIS-36-50"),
        /// Idle at 50 Bd, traffic at 100 Bd, 250 Hz shift.
        V50 = ("50-100", "CIS-50-50"),
    }
}

sdrkit_core::choice_enum! {
    /// The CIS-12 order.
    pub enum Order {
        /// DBPSK.
        Two = ("2", "DBPSK"),
        /// DQPSK.
        Four = ("4", "DQPSK"),
    }
}

sdrkit_core::choice_enum! {
    /// The CIS-36 tone rate.
    pub enum ToneRate {
        /// 10 Bd.
        R10 = ("10", "10 Bd"),
        /// 20 Bd.
        R20 = ("20", "20 Bd"),
        /// 40 Bd.
        R40 = ("40", "40 Bd"),
    }
}

sdrkit_core::choice_enum! {
    /// The OFDM signal.
    pub enum OfdmRow {
        /// CIS-45 at 40 Bd.
        Cis45a = ("cis45_40", "CIS-45 (40 Bd)"),
        /// CIS-45 at 33.3 Bd.
        Cis45b = ("cis45_33", "CIS-45 (33.3 Bd)"),
        /// CIS-60.
        Cis60 = ("cis60", "CIS-60"),
        /// CIS-93.
        Cis93 = ("cis93", "CIS-93"),
    }
}

/// The BEE variant of the test signal.
pub const GEN_VARIANT: Setting<field::Choice<BeeVariant>> =
    Setting::choice("gen_variant", "BEE variant", BeeVariant::V36)
        .section(GEN)
        .advanced();
/// The BEE test signal puts the 1 bits on the lower tone.
pub const GEN_LOWER_ONE: Setting<field::Bool> =
    Setting::new("gen_lower_one", "BEE 1 on the lower tone", "", true)
        .section(GEN)
        .advanced();
/// The centre of the 2-FSK test signals; 0 takes 1700 Hz for BEE and 1500 Hz for CIS-14.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new("gen_centre_hz", "Centre", "Hz", 0.0)
    .range(0.0, 5000.0, 1.0)
    .help("0 takes 1700 Hz for BEE and 1500 Hz for CIS-14.")
    .section(GEN)
    .advanced();
/// The shift of the CIS-14 test signal.
pub const GEN_SHIFT_HZ: Setting<field::Hz> =
    Setting::new("gen_shift_hz", "CIS-14 shift", "Hz", 200.0)
        .range(60.0, 1100.0, 1.0)
        .section(GEN)
        .advanced();
/// The order of the CIS-12 test signal.
pub const GEN_ORDER: Setting<field::Choice<Order>> =
    Setting::choice("gen_order", "CIS-12 order", Order::Four)
        .section(GEN)
        .advanced();
/// The tone rate of the CIS-36 test signal.
pub const GEN_RATE: Setting<field::Choice<ToneRate>> =
    Setting::choice("gen_rate", "CIS-36 tone rate", ToneRate::R20)
        .section(GEN)
        .advanced();
/// The OFDM signal of the `ofdm_hdr` test signal.
pub const GEN_OFDM: Setting<field::Choice<OfdmRow>> =
    Setting::choice("gen_ofdm", "OFDM signal", OfdmRow::Cis45a)
        .section(GEN)
        .advanced();

/// A small random generator for the test content.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
}

/// The 35 valid 3:4 characters.
pub fn valid_chars() -> Vec<u8> {
    (0u8..128).filter(|&c| valid(c)).collect()
}

/// The 44-bit start word of the BEE test signal (21 ones).
pub const START_WORD: u64 = 0xDE5_7814_E8A2;

/// The BEE test message: the 10 key characters and the 120 traffic characters.
pub fn bee_message() -> (Vec<u8>, Vec<u8>) {
    let v = valid_chars();
    let mut r = Lcg(77);
    let key = (0..10).map(|_| v[r.next() as usize % v.len()]).collect();
    let chars = (0..120).map(|_| v[r.next() as usize % v.len()]).collect();
    (key, chars)
}

/// The bits of `chars` (7 bits, most significant first).
fn char_bits(chars: &[u8]) -> Vec<u8> {
    chars
        .iter()
        .flat_map(|&c| (0..7).rev().map(move |i| (c >> i) & 1))
        .collect()
}

/// The traffic bits of a BEE message: the start word, the key twice, the characters, 77 four times.
pub fn bee_bits(with_start: bool) -> Vec<u8> {
    let (key, chars) = bee_message();
    let mut b = Vec::new();
    if with_start {
        b.extend((0..44).rev().map(|i| ((START_WORD >> i) & 1) as u8));
        b.extend(char_bits(&key));
        b.extend(char_bits(&key));
    }
    b.extend(char_bits(&chars));
    b.extend(char_bits(&[EOM; 4]));
    b
}

/// The BEE test signal: 2 s of reversals at the idle rate, 0.6 s at the traffic rate, the message.
fn bee(cfg: &GenConfig) -> Box<dyn Generator> {
    let s = &cfg.settings;
    let (idle, traffic, shift) = match s.get(&GEN_VARIANT) {
        BeeVariant::V36 => (36.0, 50.0, 200.0),
        BeeVariant::V50 => (50.0, 100.0, 250.0),
    };
    let c = match s.get(&GEN_CENTRE_HZ) {
        v if v > 0.0 => v,
        _ => 1700.0,
    };
    let lower_one = s.get(&GEN_LOWER_ONE);
    let tone = |b: u8| {
        let hi = (b == 1) != lower_one;
        Some(if hi { c + shift / 2.0 } else { c - shift / 2.0 })
    };
    let mut seq = Vec::new();
    for i in 0..(2.0 * idle) as usize {
        seq.push((1.0 / idle, tone((i % 2) as u8 ^ 1)));
    }
    for i in 0..(0.6 * traffic) as usize {
        seq.push((1.0 / traffic, tone((i % 2) as u8 ^ 1)));
    }
    for b in bee_bits(true) {
        seq.push((1.0 / traffic, tone(b)));
    }
    Box::new(ToneSeqGen::new(cfg.rate_hz, &seq, shift + 2.0 * traffic))
}

/// A 14-bit CIS-14 frame (bit 0 first): the state bits (each 2-bit value with its first-sent bit least significant), word `a` on
/// places 2, 4, ... 10 and word `b` on places 3, 5, ... 11 (most significant first), the parity bits on places 12 and 13.
pub fn cis14_bits(state: u8, a: u8, b: u8, parity: u8) -> [u8; 14] {
    let mut f = [0u8; 14];
    f[0] = state & 1;
    f[1] = (state >> 1) & 1;
    for k in 0..5 {
        f[2 + 2 * k] = (a >> (4 - k)) & 1;
        f[3 + 2 * k] = (b >> (4 - k)) & 1;
    }
    f[12] = parity & 1;
    f[13] = (parity >> 1) & 1;
    f
}

/// The 14-bit frame `i` of the CIS-14 test signal: state 2, `a = i mod 32`, `b = 7 i mod 32`, parity `i mod 4`.
pub fn cis14_frame(i: usize) -> [u8; 14] {
    cis14_bits(2, (i % 32) as u8, ((7 * i) % 32) as u8, (i % 4) as u8)
}

fn cis14(cfg: &GenConfig) -> Box<dyn Generator> {
    let s = &cfg.settings;
    let shift = s.get(&GEN_SHIFT_HZ);
    let c = match s.get(&GEN_CENTRE_HZ) {
        v if v > 0.0 => v,
        _ => 1500.0,
    };
    let seq: Vec<(f64, Option<f64>)> = (0..600)
        .flat_map(cis14_frame)
        .map(|b| {
            (
                1.0 / 96.0,
                Some(if b == 1 {
                    c + shift / 2.0
                } else {
                    c - shift / 2.0
                }),
            )
        })
        .collect();
    Box::new(ToneSeqGen::new(cfg.rate_hz, &seq, shift + 200.0))
}

/// The CIS-36 tones (1 to 36) of the test signal: random traffic tones, a control tone (1, 12, 24 or 36) once in 60 symbols.
pub fn cis36_tones(n: usize) -> Vec<usize> {
    let mut r = Lcg(36);
    let traffic: Vec<usize> = (1..=36).filter(|t| ![1, 12, 24, 36].contains(t)).collect();
    (0..n)
        .map(|i| {
            if i % 60 == 59 {
                [1, 12, 24, 36][(i / 60) % 4]
            } else {
                traffic[r.next() as usize % traffic.len()]
            }
        })
        .collect()
}

fn cis36(cfg: &GenConfig) -> Box<dyn Generator> {
    let rate = match cfg.settings.get(&GEN_RATE) {
        ToneRate::R10 => 10.0,
        ToneRate::R20 => 20.0,
        ToneRate::R40 => 40.0,
    };
    let seq: Vec<(f64, Option<f64>)> = cis36_tones((30.0 * rate) as usize)
        .into_iter()
        .map(|t| (1.0 / rate, Some(1000.0 + 40.0 * (t - 1) as f64)))
        .collect();
    Box::new(ToneSeqGen::new(cfg.rate_hz, &seq, 1500.0))
}

/// The rate the PSK and OFDM test signals are made at.
const BASE: f64 = 24_000.0;

/// The steps (0 to `order` - 1) of the 12 carriers of each CIS-12 test symbol.
pub fn cis12_steps(order: u32, n: usize) -> Vec<[u8; 12]> {
    let mut r = Lcg(12);
    (0..n)
        .map(|_| std::array::from_fn(|_| (r.next() % order) as u8))
        .collect()
}

fn cis12_signal(order: u32) -> Vec<Complex32> {
    let sym = 200;
    let amp = (1.0 / 13.0f64).sqrt();
    let mut x = Vec::new();
    let mut phase = [0.0f64; 12];
    let pilot = |n: usize| {
        Complex32::from_polar(
            amp as f32,
            (TAU * 3300.0 * n as f64 / BASE).rem_euclid(TAU) as f32,
        )
    };
    for n in 0..12_000 {
        x.push(pilot(n));
    }
    for steps in cis12_steps(order, 1200) {
        for k in 0..12 {
            phase[k] += TAU * f64::from(steps[k]) / f64::from(order);
        }
        for _ in 0..sym {
            let n = x.len();
            let t = n as f64 / BASE;
            let mut v = pilot(n);
            for (k, p) in phase.iter().enumerate() {
                let f = super::cis12::carrier(k);
                v += Complex32::from_polar(amp as f32, (TAU * f * t + p).rem_euclid(TAU) as f32);
            }
            x.push(v);
        }
    }
    x
}

/// An OFDM signal: `nsym` symbols of `carriers` carriers (`off` not sent) of differential PSK of `order` on whole multiples of
/// `1 / tu_s` around `centre_hz`, symbols `ts_s` long (the guard a cyclic prefix), and a 3300 Hz pilot at twice a carrier's
/// amplitude when `pilot`.
#[allow(clippy::too_many_arguments)]
fn ofdm_signal(
    nsym: usize,
    carriers: usize,
    off: Option<usize>,
    order: u32,
    tu_s: f64,
    ts_s: f64,
    centre_hz: f64,
    pilot: bool,
) -> Vec<Complex32> {
    let sp = 1.0 / tu_s;
    let k0 = (centre_hz / sp).round() as i64 - (carriers / 2) as i64;
    let nc = carriers - usize::from(off.is_some());
    let norm = 1.0 / ((nc as f64 + if pilot { 4.0 } else { 0.0 }).sqrt());
    let mut r = Lcg(45);
    let mut phase = vec![0.0f64; carriers];
    let total = (nsym as f64 * ts_s * BASE) as usize;
    let mut x = Vec::with_capacity(total);
    let ng = ts_s - tu_s;
    let mut j = usize::MAX;
    let mut rot: Vec<(Complex32, Complex32)> = Vec::new();
    for n in 0..total {
        let t = n as f64 / BASE;
        let sj = (t / ts_s).floor() as usize;
        if sj != j {
            j = sj;
            for p in phase.iter_mut() {
                *p += TAU * f64::from(r.next() % order) / f64::from(order);
            }
            // The time from the start of the useful part; the prefix is the end of the useful part.
            let tau = t - sj as f64 * ts_s - ng;
            rot = (0..carriers)
                .map(|k| {
                    let f = (k0 + k as i64) as f64 * sp;
                    (
                        Complex32::from_polar(
                            norm as f32,
                            (TAU * f * tau + phase[k]).rem_euclid(TAU) as f32,
                        ),
                        Complex32::from_polar(1.0, (TAU * f / BASE) as f32),
                    )
                })
                .collect();
        }
        let mut v = Complex32::default();
        for (k, (a, s)) in rot.iter_mut().enumerate() {
            if Some(k) != off {
                v += *a;
            }
            *a *= *s;
        }
        if pilot {
            v += Complex32::from_polar(
                (2.0 * norm) as f32,
                (TAU * 3300.0 * t).rem_euclid(TAU) as f32,
            );
        }
        x.push(v);
    }
    x
}

fn cis3000_signal() -> Vec<Complex32> {
    let sps = 8usize;
    let nsym = 60_000;
    let mut r = Lcg(3000);
    let syms: Vec<Complex32> = (0..nsym)
        .map(|_| Complex32::from_polar(1.0, TAU as f32 * (r.next() % 8) as f32 / 8.0))
        .collect();
    let span = 8 * sps;
    let taps: Vec<f32> = (0..=2 * span)
        .map(|i| sdrkit_algo_dsp::pulse::rrc((i as f64 - span as f64) / sps as f64, 0.15) as f32)
        .collect();
    let n = nsym * sps;
    let mut x = vec![Complex32::default(); n];
    for (i, &s) in syms.iter().enumerate() {
        for (j, &t) in taps.iter().enumerate() {
            let k = (i * sps + j) as i64 - span as i64;
            let k = k.rem_euclid(n as i64) as usize;
            x[k] += s * t;
        }
    }
    let p = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / n as f64;
    let g = (1.0 / p.max(1e-30)).sqrt() as f32;
    for (i, v) in x.iter_mut().enumerate() {
        *v *=
            g * Complex32::from_polar(1.0, (TAU * 1800.0 * i as f64 / BASE).rem_euclid(TAU) as f32);
    }
    x
}

/// A signal made at 24 kHz, played at any rate (cubic interpolation) above the dial of the crate's test signals, repeated.
pub struct Playback {
    x: Arc<Vec<Complex32>>,
    rate: f64,
    dial: f64,
    pos: f64,
    n: u64,
    bandwidth: f64,
}

impl Playback {
    /// `x` (at 24 kHz, the dial at 0 Hz) played at `rate`; `bandwidth` is the signal width reported.
    pub fn new(x: Vec<Complex32>, rate: f64, bandwidth: f64) -> Self {
        Playback {
            x: Arc::new(x),
            rate,
            dial: test_dial(rate),
            pos: 0.0,
            n: 0,
            bandwidth,
        }
    }
}

impl Generator for Playback {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.bandwidth))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let len = self.x.len();
        if len < 4 {
            out.iter_mut().for_each(|o| *o = Complex32::default());
            return out.len();
        }
        let step = BASE / self.rate;
        for o in out.iter_mut() {
            let i = self.pos.floor() as usize;
            let f = (self.pos - i as f64) as f32;
            let at = |k: usize| self.x[(i + len + k - 1) % len];
            let (p0, p1, p2, p3) = (at(0), at(1), at(2), at(3));
            // Catmull-Rom.
            let v = p1
                + (p2 - p0) * (0.5 * f)
                + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * (0.5 * f * f)
                + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * (0.5 * f * f * f);
            let ph = (TAU * self.dial * self.n as f64 / self.rate).rem_euclid(TAU);
            *o = v * Complex32::from_polar(1.0, ph as f32);
            self.n += 1;
            self.pos += step;
            if self.pos >= len as f64 {
                self.pos -= len as f64;
            }
        }
        out.len()
    }
}

/// The test signal of the submode.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let rate = cfg.rate_hz;
    Ok(match submode_of(cfg.submode.as_deref(), s) {
        Submode::Bee => bee(cfg),
        Submode::Cis14 => cis14(cfg),
        Submode::Cis36 => cis36(cfg),
        Submode::Cis12 => {
            let order = match s.get(&GEN_ORDER) {
                Order::Two => 2,
                Order::Four => 4,
            };
            Box::new(Playback::new(cis12_signal(order), rate, 3000.0))
        }
        Submode::OfdmHdr => {
            let (tu_ms, baud, carriers, order) = match s.get(&GEN_OFDM) {
                OfdmRow::Cis45a => (16.0, 40.0, 45, 2),
                OfdmRow::Cis45b => (16.0, 100.0 / 3.0, 45, 2),
                OfdmRow::Cis60 => (22.5, 320.0 / 9.0, 60, 8),
                OfdmRow::Cis93 => (32.0, 200.0 / 9.0, 93, 4),
            };
            let x = ofdm_signal(
                1200,
                carriers,
                None,
                order,
                tu_ms / 1000.0,
                1.0 / baud,
                1650.0,
                true,
            );
            Box::new(Playback::new(x, rate, 3400.0))
        }
        Submode::Cis128 => {
            let x = ofdm_signal(600, 128, Some(64), 4, 1.0 / 24.0, 1.0 / 21.0, 1650.0, false);
            Box::new(Playback::new(x, rate, 3200.0))
        }
        Submode::Cis3000 => Box::new(Playback::new(cis3000_signal(), rate, 3500.0)),
    })
}
