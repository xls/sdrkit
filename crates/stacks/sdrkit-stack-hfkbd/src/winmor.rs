//! WINMOR, detect only: the pilot of the retired HF e-mail modem. Each frame of a WINMOR station begins with a 1500 Hz carrier
//! keyed in DBPSK at 93.75 Bd whose phase is reversed every symbol, then one symbol that keeps the phase (the frame sync), then
//! four 4-FSK symbols at 46.875 Bd that carry the frame type. The receive chain keeps 1050 to 1950 Hz, finds the alternation,
//! measures the tuning error from the squared signal of the 20 symbols before the trigger (the squared frame-type tones fall in the same band), finds the symbol timing and the frame sync, and reports the four frame-type tones as raw indices 0 to 7.
//! Frames are not read: the symbol mapping, the Reed-Solomon code and the callsign packing are not in the specification.
//!
//! Standard: R. Muething KN6KB, WINMOR protocol specification, revision 1.0.6 (released to the public domain), sections 5 and
//! 6.1. No part of it is confirmed by a public document of an issuing body.

use crate::decoder::{self, InputPort, Modulator};
use crate::typed::{WinmorFields, WinmorPilotEvent, WinmorPilotRow, WINMOR_PILOT, WINMOR_PILOTS};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::fir::{bandpass_complex, ComplexFir};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::Time;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Clock, Complex32, ConfigError, GenConfig, Generator, LockState, Setting, SignalInfo, Status,
    Values,
};
use sdrkit_graph::{no_values, Format, Kept, Outbox, Port, RunContext};
use std::f64::consts::TAU;

/// The rate the receiver works at.
const RATE: f64 = 12_000.0;
/// Samples a pilot symbol (93.75 Bd).
const N: usize = 128;
/// Samples a frame-type symbol (46.875 Bd).
const N_FSK: usize = 256;
/// The pilot, Hz.
const PILOT: f64 = 1500.0;
/// The eight frame-type tones, Hz: two groups of four on the 46.875 Hz grid.
pub const TONES: [f64; 8] = [
    1312.5, 1359.375, 1406.25, 1453.125, 1546.875, 1593.75, 1640.625, 1687.5,
];
/// Samples kept.
const KEEP: usize = 128 * N;
/// The band the detector keeps, Hz: the pilot and the frame-type tones at a tuning error of up to 200 Hz.
const BAND: (f64, f64) = (1050.0, 1950.0);

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Detect only: frames are not read. Not confirmed by a public document: the \
pilot (1500 Hz, DBPSK at 93.75 Bd, reversed every symbol), the frame sync, the four 4-FSK frame-type symbols at 46.875 Bd and \
their eight tones (1359.375 Hz taken for the 1358.375 Hz of the specification).";

#[derive(Clone, Debug)]
struct Report {
    time: f64,
    error: f64,
    symbols: i64,
    tones: [u8; 4],
}

/// The WINMOR detector.
pub struct WinmorDecoder {
    decim: FirDecimator<Complex32>,
    band: ComplexFir,
    hist: Vec<Complex32>,
    base: u64,
    n: u64,
    corr: Complex32,
    power: f32,
    run: usize,
    hold: u64,
    pending: Option<u64>,
    fft: WindowedFft,
    rows: Vec<Report>,
    new_rows: usize,
    serial: u64,
    cleared: bool,
    pilots: i64,
    last_error: Option<f64>,
    clock: Option<Clock>,
    utc0: Option<f64>,
    n_in: u64,
    rate_in: f64,
    dial: f64,
    status: Status,
}

impl Default for WinmorDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl WinmorDecoder {
    /// The detector.
    pub fn new() -> Self {
        WinmorDecoder {
            decim: FirDecimator::design(24_000.0, 2, 2500.0, 2500.0, 60.0)
                .expect("a valid decimator"),
            band: ComplexFir::new(
                &bandpass_complex(RATE, BAND.0, BAND.1, 200.0, 60.0).expect("a valid band-pass"),
            ),
            hist: Vec::new(),
            base: 0,
            n: 0,
            corr: Complex32::default(),
            power: 0.0,
            run: 0,
            hold: 0,
            pending: None,
            fft: WindowedFft::new(4096, 4096, Window::Rectangular).expect("a valid transform"),
            rows: Vec::new(),
            new_rows: 0,
            serial: 0,
            cleared: false,
            pilots: 0,
            last_error: None,
            clock: None,
            utc0: None,
            n_in: 0,
            rate_in: 24_000.0,
            dial: 0.0,
            status: Status::default(),
        }
    }

    fn reset(&mut self) {
        self.hist.clear();
        self.base = self.n;
        self.corr = Complex32::default();
        self.power = 0.0;
        self.run = 0;
        self.hold = 0;
        self.pending = None;
        self.decim.reset();
        self.band.reset();
    }

    /// The kept sample `i` (absolute).
    fn at(&self, i: u64) -> Complex32 {
        if i < self.base || i >= self.n {
            return Complex32::default();
        }
        self.hist[(i - self.base) as usize]
    }

    /// The product of kept sample `i` with the conjugate of the sample one symbol before.
    fn lag(&self, i: u64) -> Complex32 {
        if i < N as u64 {
            Complex32::default()
        } else {
            self.at(i) * self.at(i - N as u64).conj()
        }
    }

    /// The sum of the kept samples `[a, a + len)` mixed down by `f`.
    fn tone_sum(&self, f: f64, a: u64, len: usize) -> Complex32 {
        let w = -TAU * f / RATE;
        let mut rot = Complex32::from_polar(1.0, (w * a as f64).rem_euclid(TAU) as f32);
        let step = Complex32::from_polar(1.0, w as f32);
        let mut acc = Complex32::default();
        for i in 0..len as u64 {
            acc += self.at(a + i) * rot;
            rot *= step;
        }
        acc
    }

    /// The analysis 34 symbols after a trigger at `t`: the report, if the pilot holds.
    fn analyse(&mut self, t: u64) -> Option<Report> {
        // The frequency: the squared signal loses the BPSK. The 20 symbols before the trigger hold the pilot and no frame-type
        // symbol, whose squared tones would fall in the same band.
        let start = t.checked_sub(20 * N as u64)?;
        let mut sq: Vec<Complex32> = (start..t).map(|i| self.at(i) * self.at(i)).collect();
        sq.resize(4096, Complex32::default());
        let p = self.fft.run_power(&sq).to_vec();
        let bin = RATE / 4096.0;
        let mean = p.iter().map(|&v| f64::from(v)).sum::<f64>() / p.len() as f64;
        let (lo, hi) = ((2600.0 / bin) as usize, (3400.0 / bin) as usize);
        let (k, &v) = p[lo..=hi]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))?;
        if f64::from(v) < 30.0 * mean {
            return None;
        }
        let k = k + lo;
        let (l, r) = (f64::from(p[k - 1]), f64::from(p[k + 1]));
        let den = l - 2.0 * f64::from(v) + r;
        let d = if den.abs() > 1e-30 {
            (0.5 * (l - r) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let error = (k as f64 + d) * bin / 2.0 - PILOT;
        if error.abs() > 200.0 {
            return None;
        }
        let f = PILOT + error;
        // The symbol timing over the 8 symbols before the trigger.
        let sym = |s: u64| self.tone_sum(f, s + 20, 88);
        let t0 = t.checked_sub(8 * N as u64)?;
        // The offset whose symbols reverse most strongly; the count of reversals there gates the report.
        let mut best: Option<(u64, f32, i32)> = None;
        for o in 0..32u64 {
            let s0 = t0 + 4 * o;
            let v: Vec<Complex32> = (0..7).map(|j| sym(s0 + j * N as u64)).collect();
            let turns: Vec<f32> = v.windows(2).map(|w| -(w[1] * w[0].conj()).re).collect();
            let strength: f32 = turns.iter().sum();
            if best.is_none_or(|b| strength > b.1) {
                let score = turns.iter().map(|&t| if t > 0.0 { 1 } else { -1 }).sum();
                best = Some((s0, strength, score));
            }
        }
        let (s0, _, score) = best?;
        if score < 4 {
            return None;
        }
        // Back over the pilot while its level holds above half, then forward to the frame sync.
        let level = sym(s0).norm();
        let mut first = s0;
        for _ in 0..40 {
            let Some(prev) = first.checked_sub(N as u64) else {
                break;
            };
            if prev < self.base
                || sym(prev).norm() < 0.5 * level
                || (sym(first) * sym(prev).conj()).re >= 0.0
            {
                break;
            }
            first = prev;
        }
        let mut s = s0;
        let mut sync = None;
        for _ in 0..40 {
            let next = s + N as u64;
            if next + N as u64 > self.n {
                break;
            }
            if (sym(next) * sym(s).conj()).re > 0.0 {
                sync = Some(next);
                break;
            }
            s = next;
        }
        let sync = sync?;
        let symbols = ((sync - first) / N as u64) as i64;
        let mut tones = [0u8; 4];
        for (j, t) in tones.iter_mut().enumerate() {
            let a = sync + N as u64 + (j * N_FSK) as u64;
            if a + N_FSK as u64 > self.n {
                return None;
            }
            let e: Vec<f32> = TONES
                .iter()
                .map(|&ft| self.tone_sum(ft + error, a, N_FSK).norm_sqr())
                .collect();
            *t = e
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(0, |(i, _)| i as u8);
        }
        Some(Report {
            time: self.utc0.unwrap_or(0.0) + self.n_in as f64 / self.rate_in,
            error,
            symbols,
            tones,
        })
    }

    fn sample(&mut self, x: Complex32, out: &mut Outbox) {
        self.hist.push(x);
        self.n += 1;
        if self.hist.len() > 2 * KEEP {
            let cut = self.hist.len() - KEEP;
            self.hist.drain(..cut);
            self.base += cut as u64;
        }
        // The alternation over the last 20 symbols.
        let w = 20 * N as u64;
        let newest = self.n - 1;
        self.corr += self.lag(newest);
        self.power += x.norm_sqr();
        if newest >= w {
            self.corr -= self.lag(newest - w);
            let old = self.at(newest - w);
            self.power -= old.norm_sqr();
        }
        if self.n.is_multiple_of(4096) {
            // Sum again now and then, so rounding does not build up.
            let a = self.n.saturating_sub(w).max(self.base + N as u64);
            self.corr = (a..self.n).map(|i| self.lag(i)).sum();
            self.power = (a..self.n).map(|i| self.at(i).norm_sqr()).sum();
        }
        if self.power > 0.0 && self.corr.norm() / self.power > 0.75 {
            self.run += 1;
        } else {
            self.run = 0;
        }
        if self.hold > 0 {
            self.hold -= 1;
        } else if self.run >= 12 * N {
            self.pending = Some(self.n);
            self.hold = 45 * N as u64;
        }
        if let Some(t) = self.pending {
            if self.n >= t + 34 * N as u64 {
                self.pending = None;
                if let Some(r) = self.analyse(t) {
                    self.report_pilot(r, out);
                }
            }
        }
    }

    fn report_pilot(&mut self, r: Report, out: &mut Outbox) {
        let tones = format!(
            "{} {} {} {}",
            r.tones[0], r.tones[1], r.tones[2], r.tones[3]
        );
        let e = (r.error * 10.0).round() / 10.0;
        out.messages(1).push(WINMOR_PILOT.event(
            r.time,
            self.dial,
            &WinmorPilotEvent {
                text: &format!(
                    "pilot {} symbols, tones {tones} (raw), tuning error {e:+.1} Hz",
                    r.symbols
                ),
                error_hz: e,
                pilot_symbols: r.symbols,
                tones: &tones,
            },
        ));
        self.pilots += 1;
        self.last_error = Some(e);
        self.rows.push(r);
        self.new_rows += 1;
        if self.rows.len() > 200 {
            self.rows.remove(0);
        }
    }
}

impl sdrkit_graph::Decoder for WinmorDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.winmor"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        crate::typed::results("winmor")
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = Some(ctx.clock.clone());
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.rate_in = input.rate;
        self.decim = FirDecimator::design(input.rate, 2, 2500.0, 2500.0, 60.0).map_err(|_| {
            ConfigError::RateNotMultiple {
                got: input.rate,
                of: 24_000.0,
            }
        })?;
        self.reset();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        if self.utc0.is_none() {
            let now = self.clock.as_ref().map_or(0.0, Clock::now);
            self.utc0 = Some(now - self.n_in as f64 / self.rate_in);
        }
        for &v in x {
            self.n_in += 1;
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            if let Some(y) = self.decim.push(v) {
                let y = self.band.process_sample(y);
                self.sample(y, out);
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            res.clear_rows(&WINMOR_PILOTS);
        }
        let start = self.rows.len().saturating_sub(self.new_rows);
        for (k, r) in self.rows.iter().skip(start).enumerate() {
            let tones = format!(
                "{} {} {} {}",
                r.tones[0], r.tones[1], r.tones[2], r.tones[3]
            );
            WINMOR_PILOTS.put(
                res,
                &format!("{}", self.serial + k as u64),
                &WinmorPilotRow {
                    time: Time(r.time),
                    error_hz: (r.error * 10.0).round() / 10.0,
                    pilot_symbols: r.symbols,
                    tones: &tones,
                },
            );
        }
        self.serial += self.new_rows as u64;
        self.new_rows = 0;
        res.put(&WinmorFields {
            note: NOTE_TEXT,
            pilots: self.pilots,
            error_hz: self.last_error,
        });
        self.status.lock_state = if self.pilots > 0 {
            LockState::Locked
        } else {
            LockState::Searching
        };
        self.status.offset_hz = self.last_error;
        self.status.summary = format!("detect only: {} WINMOR pilots", self.pilots);
        res.results_mut().set_status(&self.status);
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.new_rows = 0;
        self.pilots = 0;
        self.last_error = None;
        self.cleared = true;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        self.last_error = None;
        if !kept.table(WINMOR_PILOTS.key) {
            self.clear();
        }
    }
}

/// A part of the WINMOR test signal: its start and length (seconds within the 5 s cycle) and what it sends.
#[derive(Clone, Copy, Debug)]
enum Part {
    /// A half-sine pulse of sign `sign` on the carrier `f`.
    Pulse {
        start: f64,
        len: f64,
        f: f64,
        sign: f64,
    },
    /// A steady tone `f`.
    Tone { start: f64, len: f64, f: f64 },
}

impl Part {
    fn span(&self) -> (f64, f64) {
        match *self {
            Part::Pulse { start, len, .. } | Part::Tone { start, len, .. } => (start, len),
        }
    }
}

/// The modulator of a WINMOR frame start, repeated every 5 s: 0.75 s of silence, the pilot of `pilot` half-sine symbols, the
/// frame sync, four 4-FSK symbols on the tones `tones` (indices 0 to 7), 8 more 4-FSK symbols, then silence. `offset_hz` moves
/// it all.
pub struct WinmorModulator {
    rate: f64,
    n: u64,
    parts: Vec<Part>,
    phase: f64,
}

impl WinmorModulator {
    /// The modulator at `rate`.
    pub fn new(rate: f64, pilot: usize, tones: [u8; 4], offset_hz: f64) -> Self {
        let ts = N as f64 / RATE;
        let tf = N_FSK as f64 / RATE;
        let f = PILOT + offset_hz;
        let mut parts = Vec::new();
        let mut t = 0.75;
        let mut sign = 1.0;
        for _ in 0..pilot {
            parts.push(Part::Pulse {
                start: t,
                len: ts,
                f,
                sign,
            });
            sign = -sign;
            t += ts;
        }
        // The frame sync keeps the phase of the last pilot symbol.
        parts.push(Part::Pulse {
            start: t,
            len: ts,
            f,
            sign: -sign,
        });
        t += ts;
        let more = [2u8, 7, 0, 5, 3, 6, 1, 4];
        for &k in tones.iter().chain(more.iter()) {
            parts.push(Part::Tone {
                start: t,
                len: tf,
                f: TONES[usize::from(k & 7)] + offset_hz,
            });
            t += tf;
        }
        WinmorModulator {
            rate,
            n: 0,
            parts,
            phase: 0.0,
        }
    }
}

impl Modulator for WinmorModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        for o in out.iter_mut() {
            let t = (self.n as f64 / self.rate) % 5.0;
            self.n += 1;
            let part = self.parts.iter().find(|p| {
                let (s, l) = p.span();
                t >= s && t < s + l
            });
            *o = match part {
                Some(&Part::Pulse {
                    start,
                    len,
                    f,
                    sign,
                }) => {
                    self.phase = (self.phase + TAU * f / self.rate).rem_euclid(TAU);
                    let env = (std::f64::consts::PI * (t - start) / len).sin() * sign;
                    Complex32::from_polar(env as f32, self.phase as f32)
                }
                Some(&Part::Tone { f, .. }) => {
                    self.phase = (self.phase + TAU * f / self.rate).rem_euclid(TAU);
                    Complex32::from_polar(1.0, self.phase as f32)
                }
                None => {
                    self.phase = (self.phase + TAU * PILOT / self.rate).rem_euclid(TAU);
                    Complex32::default()
                }
            };
        }
    }
}

/// The frame-type tones of the test signal, as four indices 0 to 7.
pub const GEN_TONES: Setting<field::Text> =
    Setting::new("gen_tones", "Frame-type tones", "", "1 6 3 4")
        .help("Four indices 0 to 7.")
        .section(decoder::GEN_SECTION)
        .advanced();
/// The tuning error of the test signal.
pub const GEN_ERROR_HZ: Setting<field::Hz> =
    Setting::new("gen_error_hz", "Tuning error", "Hz", 0.0)
        .range(-200.0, 200.0, 0.1)
        .section(decoder::GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of WINMOR.
    pub enum WinmorGenSetting {
        /// Frame-type tones.
        GenTones = GEN_TONES,
        /// Tuning error.
        GenErrorHz = GEN_ERROR_HZ,
    }
}

/// The four tone indices of a text of numbers 0 to 7 (missing or bad ones are 0).
pub fn parse_tones(s: &str) -> [u8; 4] {
    let mut t = [0u8; 4];
    for (i, v) in s.split_whitespace().take(4).enumerate() {
        t[i] = v.parse::<u8>().unwrap_or(0) & 7;
    }
    t
}

/// The test signal: a 30-symbol pilot and the frame-type tones, every 5 s.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let tones = parse_tones(cfg.settings.get(&GEN_TONES));
    let off = cfg.settings.get(&GEN_ERROR_HZ);
    Ok(decoder::placed(
        cfg.rate_hz,
        WinmorModulator::new(cfg.rate_hz, 30, tones, off),
        SignalInfo::new(500.0).with_symbol_rate(93.75),
    ))
}

/// The input of the WINMOR receiver.
pub const INPUT: InputPort = InputPort::Wide;
