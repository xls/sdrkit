//! Modulation quality meter (`mer`): of one blind M-PSK carrier (BPSK, QPSK, 8-PSK) the symbol rate, the order, the carrier
//! offset, and MER and EVM after matched filtering, timing and phase recovery (MER and EVM as ETSI TR 101 290 defines them, with
//! a fitted gain). It measures and decodes nothing. Hard decisions make the reading too good below about 8 dB MER; QAM, APSK,
//! FSK and OFDM are not measured; the carrier must be alone in the channel. It reads the signal as this radio receives it, not
//! a type-approval measurement. The blind chain is confirmed by no public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{MerEventRow, MerFields, MerModulation, MER_EVENTS};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::{CatmullRom, Reader};
use sdrkit_algo_dsp::peak::{line_fit, log_parabolic};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas, PhaseDetector};
use sdrkit_algo_modem::level::DataAided;
use sdrkit_algo_modem::timing::{Gardner, SymbolSync, SymbolSyncConfig};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::f64::consts::{FRAC_PI_4, TAU};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "mer";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 96e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 100e3;
/// The pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 45e3;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.1;
/// The bins of the moving mean of the rough carrier.
pub const MOVING_BINS: usize = 64;
/// Candidates of the symbol rate.
pub const CANDIDATES: usize = 3;
/// A line of the M-th power is this many times the mean around it.
pub const LINE_RATIO: f64 = 25.0;
/// The fewest symbols of a measurement, and of its measured part.
pub const MIN_SYMBOLS: (usize, usize) = (400, 200);
/// The symbols of the start phase.
pub const PHASE_SYMBOLS: usize = 300;
/// The loop bandwidths per symbol of the timing and the carrier.
pub const LOOP_BN: (f64, f64) = (0.005, 0.018);
/// The phases of a symbol tried for the timing on the fitted clock.
pub const PHASES: usize = 32;
/// The span of the matched filter, symbols: +-8, so that the tails of a small roll-off add no ISI.
pub const MATCHED_SPAN: f64 = 16.0;
/// A row when the MER moves this far, dB, or after this time, s.
pub const ROW_RULE: (f64, f64) = (3.0, 30.0);

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures one blind M-PSK carrier (BPSK, QPSK, 8-PSK): symbol rate, order, carrier offset, MER and \
EVM after matched filtering, timing and phase recovery; decodes nothing. Hard decisions make the reading too good below about \
8 dB MER, so a low value means poor. QAM, APSK, FSK and OFDM are not measured; the carrier must be alone in the channel. The \
reading includes this radio's phase noise and clock. The blind chain is confirmed by no public document.";

/// The block length.
pub const BLOCK_S: Setting<field::F64> =
    Setting::new("block_s", "Block length", "s", 2.0).range(0.5, 10.0, 0.0);
/// The lowest symbol rate.
pub const MIN_BAUD: Setting<field::F64> =
    Setting::new("min_baud", "Lowest symbol rate", "Bd", 300.0).range(50.0, 10_000.0, 0.0);
/// The lowest MER shown.
pub const MIN_MER_DB: Setting<field::F64> =
    Setting::new("min_mer_db", "Lowest MER shown", "dB", 4.0).range(0.0, 20.0, 0.0);
/// The roll-offs tried.
pub const ROLLOFFS: Setting<field::Text> =
    Setting::new("rolloffs", "Roll-offs tried", "", "0.2, 0.35, 0.5")
        .help("Roll-offs separated by ',', each 0.1 to 1.0.");

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The block.
        BlockS = BLOCK_S,
        /// The lowest rate.
        MinBaud = MIN_BAUD,
        /// The lowest MER.
        MinMerDb = MIN_MER_DB,
        /// The roll-offs.
        Rolloffs = ROLLOFFS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `mer` stack.
    pub enum MerSetting {
        /// The block.
        BlockS = BLOCK_S,
        /// The lowest rate.
        MinBaud = MIN_BAUD,
        /// The lowest MER.
        MinMerDb = MIN_MER_DB,
        /// The roll-offs.
        Rolloffs = ROLLOFFS,
        /// The phases of the test signal.
        GenPsk = gen::GEN_PSK,
        /// Its symbol rate.
        GenSymbolRate = gen::GEN_SYMBOL_RATE,
        /// Its roll-off.
        GenRolloff = gen::GEN_ROLLOFF,
        /// Its points.
        GenRotated = gen::GEN_ROTATED,
    }
}

/// One measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reading {
    m: u32,
    mer: f64,
    rate: f64,
    fc: f64,
    rolloff: f64,
    symbols: usize,
    level: f64,
}

/// The power spectrum of `v`, Hann over its length and zero-padded to `nf`, in FFT order.
fn spectrum(v: impl ExactSizeIterator<Item = Complex32>, nf: usize) -> Vec<f64> {
    let n = v.len();
    let w = Window::Hann.coefficients(n);
    let mut buf = vec![Complex32::default(); nf];
    for ((b, z), &wi) in buf.iter_mut().zip(v).zip(&w) {
        *b = z * wi;
    }
    Fft::forward(nf).process(&mut buf);
    buf.iter().map(|z| f64::from(z.norm_sqr())).collect()
}

/// The signed index of bin `k` of `nf`.
fn signed(k: usize, nf: usize) -> i64 {
    if k >= nf / 2 {
        k as i64 - nf as i64
    } else {
        k as i64
    }
}

/// The refined frequency of bin `k` of the spectrum `p` (FFT order), Hz.
fn refined(p: &[f64], k: usize, b: f64) -> f64 {
    let nf = p.len();
    let d = log_parabolic(p[(k + nf - 1) % nf], p[k], p[(k + 1) % nf]).offset;
    (signed(k, nf) as f64 + d) * b
}

/// The symbols after a carrier loop of order `M`, turned back so that the points lie at `2 pi k / M`.
fn carrier_loop<P: PhaseDetector>(det: P, z: &[Complex32], pre: f64) -> Vec<Complex32> {
    let Ok(mut cs) = CarrierSync::new(det, CarrierConfig::new(LOOP_BN.1)) else {
        return Vec::new();
    };
    let (a, b) = (
        Complex32::from_polar(1.0, pre as f32),
        Complex32::from_polar(1.0, -pre as f32),
    );
    z.iter().map(|&s| cs.push(s * a) * b).collect()
}

/// The symbols of the matched-filter output `y` on a straight clock: `sync` finds the symbol instants, a least-squares line
/// through those from an eighth of the way in gives the symbol period, and the phase of the most energy of [`PHASES`] tried
/// (refined by the log parabola) places it; `y` is read there (Catmull-Rom). `None` with fewer than 16 instants.
fn clocked_symbols(
    sync: &mut SymbolSync<Complex32, Gardner, CatmullRom>,
    y: &[Complex32],
) -> Option<Vec<Complex32>> {
    let mut sym = Vec::new();
    let mut at = Vec::new();
    for v in y.chunks(1) {
        sync.process(v, &mut sym);
        if sym.len() > at.len() {
            at.push(sync.last_instant());
        }
    }
    let n = at.len();
    if n < 16 {
        return None;
    }
    let ks: Vec<f64> = (n / 8..n).map(|k| k as f64).collect();
    let clock = line_fit(&ks, &at[n / 8..], None).ok()?;
    let step = clock.slope;
    let read = Reader::new(CatmullRom);
    let energy = |d: f64| {
        (n / 8..n)
            .map(|k| f64::from(read.at(y, clock.at(k as f64) + d).norm_sqr()))
            .sum::<f64>()
    };
    let e: Vec<f64> = (0..PHASES)
        .map(|j| energy(step * (j as f64 / PHASES as f64 - 0.5)))
        .collect();
    let j = (0..PHASES).max_by(|&a, &b| e[a].total_cmp(&e[b]))?;
    let p = log_parabolic(e[(j + PHASES - 1) % PHASES], e[j], e[(j + 1) % PHASES]).offset;
    let d = step * ((j as f64 + p) / PHASES as f64 - 0.5);
    Some((0..n).map(|k| read.at(y, clock.at(k as f64) + d)).collect())
}

/// The nearest of the `m` points at `2 pi k / m`.
fn decide(y: Complex32, m: u32) -> Complex32 {
    let k = (f64::from(y.im).atan2(f64::from(y.re)) * f64::from(m) / TAU).round();
    Complex32::from_polar(1.0, (TAU * k / f64::from(m)) as f32)
}

/// The measurement of `mer`.
#[derive(Default)]
pub struct Mer {
    front: Option<Front>,
    rate: f64,
    block: usize,
    min_baud: f64,
    min_mer: f64,
    rolloffs: Vec<f64>,
    x: Vec<Complex32>,
    t: f64,
    reading: Option<Reading>,
    last_row: Option<(f64, u32, f64)>,
    log: Log<MerEventRow>,
}

impl Mer {
    /// MER, symbols of the candidate at `fc` Hz and `rb` Bd of order `m`, for the roll-off `beta`.
    fn measure(
        &self,
        x: &[Complex32],
        fc: f64,
        rb: f64,
        m: u32,
        beta: f64,
    ) -> Option<(f64, usize)> {
        let r = self.rate;
        let q = ((r / (8.0 * rb)).floor() as usize).max(1);
        let step = Complex32::from_polar(1.0, (-TAU * fc / r) as f32);
        let mut ph = Complex32::new(1.0, 0.0);
        let mut mixed = Vec::with_capacity(x.len());
        for (i, &z) in x.iter().enumerate() {
            mixed.push(z * ph);
            ph *= step;
            if i % 1024 == 0 {
                ph /= ph.norm();
            }
        }
        let d: Vec<Complex32> = mixed
            .chunks_exact(q)
            .map(|c| c.iter().sum::<Complex32>() / q as f32)
            .collect();
        let s = r / q as f64 / rb;
        let h = taps(Shape::Rrc { beta }, s, MATCHED_SPAN, Norm::Energy).ok()?;
        if d.len() < 8 * (h.len() / 2) || d.len() < h.len() {
            return None;
        }
        let mut full = vec![Complex32::default(); d.len()];
        Fir::new(&h).process(&d, &mut full);
        let y = &full[h.len() - 1..];
        let mut cfg = SymbolSyncConfig::new(s);
        cfg.bn_t = LOOP_BN.0;
        cfg.pulse = Shape::Rc { beta };
        let mut sync = SymbolSync::new(Gardner, CatmullRom, cfg).ok()?;
        let sym = clocked_symbols(&mut sync, y)?;
        let n = sym.len();
        if n < MIN_SYMBOLS.0 {
            return None;
        }
        let from = n / 8;
        let acc: Complex32 = sym[from..(from + PHASE_SYMBOLS).min(n)]
            .iter()
            .filter(|z| z.norm() > 0.0)
            .map(|z| (z / z.norm()).powu(m))
            .sum();
        let start = f64::from(acc.im).atan2(f64::from(acc.re)) / f64::from(m);
        let amp = sym.iter().map(|z| z.norm()).sum::<f32>() / n as f32;
        let turn = Complex32::from_polar(1.0 / amp.max(1e-12), -start as f32);
        let z: Vec<Complex32> = sym.iter().map(|&v| v * turn).collect();
        let out = match m {
            2 => carrier_loop(Costas::<2>, &z, 0.0),
            4 => carrier_loop(Costas::<4>, &z, FRAC_PI_4),
            _ => carrier_loop(Costas::<8>, &z, 0.0),
        };
        let part = &out[n / 4..];
        if part.len() < MIN_SYMBOLS.1 {
            return None;
        }
        let mut da = DataAided::new();
        for &v in part {
            da.push(v, decide(v, m));
        }
        Some((da.estimate()?.snr_db, part.len()))
    }

    fn analyse(&mut self, c: &mut Ctx) {
        let n = self.block;
        self.t += n as f64 / self.rate;
        let x = &self.x[..n];
        let power = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / n as f64;
        self.reading = None;
        if power < 1e-14 {
            c.tally.bad(self.t);
            return;
        }
        let level = 10.0 * power.log10();
        let g = (1.0 / power.sqrt()) as f32;
        let x: Vec<Complex32> = x.iter().map(|&z| z * g).collect();
        c.tally.energy(self.t);
        let nf = n.next_power_of_two();
        let b = self.rate / nf as f64;
        let p = spectrum(x.iter().copied(), nf);
        let median = |v: &[f64]| {
            let mut s = v.to_vec();
            let k = s.len() / 2;
            let (_, m, _) = s.select_nth_unstable_by(k, f64::total_cmp);
            *m
        };
        let m = median(&p);
        let shifted: Vec<f64> = (0..nf).map(|i| p[(i + nf / 2) % nf]).collect();
        let mut prefix = vec![0.0; nf + 1];
        for i in 0..nf {
            prefix[i + 1] = prefix[i] + shifted[i];
        }
        let (mut wsum, mut fsum) = (0.0, 0.0);
        for i in 0..nf {
            let (a, z) = (
                i.saturating_sub(MOVING_BINS / 2),
                (i + MOVING_BINS / 2).min(nf),
            );
            let w = (prefix[z] - prefix[a]) / (z - a) as f64 - 1.5 * m;
            if w > 0.0 {
                wsum += w;
                fsum += w * (i as f64 - (nf / 2) as f64) * b;
            }
        }
        if wsum <= 0.0 {
            return;
        }
        let rough = fsum / wsum;
        let env_mean = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / n as f64;
        let e = spectrum(
            x.iter()
                .map(|z| Complex32::new((f64::from(z.norm_sqr()) - env_mean) as f32, 0.0)),
            nf,
        );
        let (lo, hi) = (
            2usize.max((self.min_baud / b).floor() as usize),
            ((self.rate / 3.0 / b).floor() as usize).min(nf / 2 - 2),
        );
        if hi <= lo + 2 {
            return;
        }
        let med_e = median(&e[lo..=hi]);
        let mut cands: Vec<usize> = Vec::new();
        for _ in 0..CANDIDATES {
            let Some(k) = (lo..=hi)
                .filter(|&k| cands.iter().all(|&j| j.abs_diff(k) >= 20))
                .max_by(|&i, &j| e[i].total_cmp(&e[j]))
            else {
                break;
            };
            if e[k] < 8.0 * med_e {
                break;
            }
            cands.push(k);
        }
        let mut powers: Vec<(u32, Vec<f64>)> = Vec::new();
        for mm in [2u32, 4, 8] {
            powers.push((mm, spectrum(x.iter().map(|z| z.powu(mm)), nf)));
        }
        let fold = |f: f64| (f + self.rate / 2.0).rem_euclid(self.rate) - self.rate / 2.0;
        let mut best: Option<Reading> = None;
        for k in cands {
            let rb = refined(&e, k, b);
            let mut found = None;
            for (mm, z) in &powers {
                let expect = fold(f64::from(*mm) * rough);
                let h = 4i64.max((f64::from(*mm) * 0.12 * rb / b).floor() as i64);
                let c0 = (expect / b).round() as i64;
                let at = |j: i64| j.rem_euclid(nf as i64) as usize;
                let pk = (c0 - h..=c0 + h)
                    .max_by(|&i, &j| z[at(i)].total_cmp(&z[at(j)]))
                    .unwrap_or(c0);
                let around: Vec<f64> = (c0 - h..=c0 + h)
                    .filter(|j| (j - pk).abs() > 5)
                    .map(|j| z[at(j)])
                    .collect();
                let mean = around.iter().sum::<f64>() / around.len().max(1) as f64;
                if z[at(pk)] > LINE_RATIO * mean {
                    let line = refined(z, at(pk), b);
                    found = Some((
                        *mm,
                        rough + fold(line - f64::from(*mm) * rough) / f64::from(*mm),
                    ));
                    break;
                }
            }
            let Some((mm, fc)) = found else { continue };
            let dist = |k: usize| fold(signed(k, nf) as f64 * b - fc).abs();
            let (mut outer, mut inner) = (0.0, 0.0);
            for (k, &pk) in p.iter().enumerate() {
                let w = (pk - m).max(0.0);
                let d = dist(k);
                if d < rb {
                    outer += w;
                }
                if d < 0.2 * rb {
                    inner += w;
                }
            }
            if outer <= 0.0 || inner > 0.7 * outer {
                continue;
            }
            for &beta in &self.rolloffs {
                let Some((mer, symbols)) = self.measure(&x, fc, rb, mm, beta) else {
                    continue;
                };
                if best.is_none_or(|b| mer > b.mer) {
                    best = Some(Reading {
                        m: mm,
                        mer,
                        rate: rb,
                        fc,
                        rolloff: beta,
                        symbols,
                        level,
                    });
                }
            }
        }
        let Some(r) = best.filter(|r| r.mer >= self.min_mer) else {
            c.tally.bad(self.t);
            return;
        };
        c.tally.good(self.t);
        self.reading = Some(r);
        if self.last_row.is_none_or(|(t, mm, mer)| {
            mm != r.m || (r.mer - mer).abs() > ROW_RULE.0 || self.t - t >= ROW_RULE.1
        }) {
            self.last_row = Some((self.t, r.m, r.mer));
            self.log.push(MerEventRow {
                time_s: Seconds(self.t),
                modulation: modulation(r.m),
                mer_db: r.mer,
                evm_pct: evm(r.mer),
                symbol_rate_bd: r.rate,
                carrier_offset_hz: Hertz(r.fc),
                level_db: r.level,
            });
        }
    }
}

fn modulation(m: u32) -> MerModulation {
    match m {
        2 => MerModulation::Bpsk,
        4 => MerModulation::Qpsk,
        _ => MerModulation::Psk8,
    }
}

fn evm(mer_db: f64) -> f64 {
    100.0 / 10f64.powf(mer_db / 20.0)
}

impl TxTest for Mer {
    const ID: &'static str = ID;
    const NAME: &'static str = "mer.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.rate = front.rate();
        self.front = Some(front);
        self.block = (v.get(&BLOCK_S) * self.rate).round() as usize;
        self.min_baud = v.get(&MIN_BAUD);
        self.min_mer = v.get(&MIN_MER_DB);
        self.rolloffs = v
            .get(&ROLLOFFS)
            .split(',')
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .filter(|b| (0.1..=1.0).contains(b))
            .collect();
        self.x.clear();
        self.t = self.t.max(SKIP_S);
        self.reading = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(front) = self.front.as_mut() else {
            return;
        };
        self.x.extend_from_slice(front.process(x));
        while self.block > 0 && self.x.len() >= self.block {
            self.analyse(c);
            self.x.drain(..self.block);
        }
    }
    fn clear(&mut self) {
        self.reading = None;
        self.last_row = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&MER_EVENTS, p);
        let r = self.reading;
        p.put(&MerFields {
            locked: (self.t > SKIP_S).then_some(r.is_some()),
            modulation: r.map(|r| modulation(r.m)),
            mer_db: r.map(|r| r.mer),
            evm_pct: r.map(|r| evm(r.mer)),
            symbol_rate_bd: r.map(|r| r.rate),
            carrier_offset_hz: r.map(|r| Hertz(r.fc)),
            rolloff: r.map(|r| r.rolloff),
            symbols: r.map(|r| r.symbols as i64),
        });
        match r {
            Some(r) => format!(
                "{}-PSK at {:.0} Bd, MER {:.1} dB, offset {:+.0} Hz",
                r.m, r.rate, r.mer, r.fc
            ),
            None => "no PSK carrier".to_string(),
        }
    }
}

const KEYS: [&str; 4] = ["block_s", "min_baud", "min_mer_db", "rolloffs"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("QPSK of 9600 Bd", o, 2100.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let b = crate::instrument::number(r, MerFields::SYMBOL_RATE_BD.key);
        vec![CheckLine::new(
            MerFields::SYMBOL_RATE_BD.key,
            (b - 9600.0).abs() < 20.0,
            format!("{b:.1} Bd, 9600 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Mer,
    settings: MerSetting,
    keys: KEYS,
    name: "Modulation quality meter",
    blurb: "Of one M-PSK carrier: order, symbol rate, carrier offset, MER and EVM, and the best roll-off.",
    tuning: (0.1e6, 6e9, 250e3, 90e3, true),
    rate_text: "The modulation quality meter needs at least 96 ksps",
    generator: |cfg| gen::MerGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
