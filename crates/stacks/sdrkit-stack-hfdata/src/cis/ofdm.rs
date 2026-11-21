//! The identifiers of the wide CIS signals: the OFDM signals (CIS-45, CIS-60, CIS-93 and CIS-128) from their guard interval, and
//! CIS-3000 (one 8-PSK carrier at 3000 Bd) from the lines of its envelope and of its powers.

use super::{Out, Sub};
use num_complex::Complex64;
use sdrkit_algo_dsp::bandscan::BandScan;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::guardscan::{symbol_timing, GuardScan};
use sdrkit_algo_modem::pskorder::PskOrder;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

const RATE: f64 = super::RATE;

/// A published OFDM signal: name, useful part (ms), symbol rate (Bd), carriers, carrier modulation.
pub const OFDM_TABLE: [(&str, f64, f64, u32, &str); 4] = [
    ("CIS-45 (40 Bd)", 16.0, 40.0, 45, "BPSK"),
    ("CIS-45 (33.3 Bd)", 16.0, 33.333, 45, "BPSK"),
    ("CIS-60", 22.5, 35.555, 60, "8-PSK"),
    ("CIS-93", 32.0, 22.222, 93, "QPSK"),
];

/// The window analysed, samples (2 s).
const WINDOW: usize = 24_000;
/// Samples between analyses (1 s).
const HOP: u64 = 12_000;

/// What an OFDM analysis found.
#[derive(Clone, Debug, Default)]
struct OfdmFound {
    band: Option<(f64, f64)>,
    tu: Option<f64>,
    rate: Option<f64>,
    ts: Option<f64>,
    spacing: Option<f64>,
    carriers: i64,
    centre_off: bool,
    order: Option<u32>,
    m: (f64, f64, f64),
    pilot: bool,
    snr: Option<f64>,
    name: Option<String>,
    reason: String,
}

/// The `ofdm_hdr` and `cis128` receivers.
pub struct Ofdm {
    wide: bool,
    scan: BandScan,
    guard: GuardScan,
    hist: Vec<Complex32>,
    n: u64,
    found: OfdmFound,
    named: Option<String>,
}

impl Ofdm {
    /// The receiver of the CIS OFDM table (`wide` false) or of CIS-128 (`wide` true).
    pub fn new(wide: bool) -> Self {
        Ofdm {
            wide,
            scan: BandScan::new(4096, RATE, 0.15).expect("a valid spectrum"),
            guard: GuardScan::new(WINDOW, 800).expect("a valid size"),
            hist: Vec::new(),
            n: 0,
            found: OfdmFound {
                reason: "searching".to_string(),
                ..OfdmFound::default()
            },
            named: None,
        }
    }

    fn analyse(&mut self) {
        let mut f = OfdmFound::default();
        let noise = self.scan.noise(-5900.0, 5900.0);
        f.pilot = self
            .scan
            .peak(3280.0, 3320.0)
            .is_some_and(|p| p.1 > 20.0 * noise);
        f.band = self.scan.occupied(100.0, 5900.0, noise, 6.0, 70.0, 300.0);
        let Some((lo, hi)) = f.band else {
            f.reason = "no signal".to_string();
            self.found = f;
            return;
        };
        f.snr = Some(self.scan.snr_db(lo, hi, noise));
        let x = &self.hist[self.hist.len() - WINDOW..];
        let Some(tu) = self.guard.useful_length(x, 150, 800, 0.06) else {
            f.reason = "no OFDM guard found".to_string();
            self.found = f;
            return;
        };
        f.tu = Some(tu.samples);
        let Some(t) = symbol_timing(x, tu.samples.round() as usize, RATE, 8.0, 60.0) else {
            f.reason = "no symbol rate".to_string();
            self.found = f;
            return;
        };
        f.rate = Some(t.rate_hz);
        f.ts = Some(t.symbol);
        let spacing = RATE / tu.samples;
        f.spacing = Some(spacing);
        // Carriers at (k + fraction) spacings, the fraction from the phase of the correlation at Tu.
        let frac = (-f64::from(tu.phase) / TAU).rem_euclid(1.0);
        let k0 = ((lo / spacing) - frac).ceil() as i64;
        let k1 = ((hi / spacing) - frac).floor() as i64;
        let freqs: Vec<f64> = (k0..=k1).map(|k| (k as f64 + frac) * spacing).collect();
        let tun = tu.samples.round() as usize;
        let g = (tun / 8).max(8) as f64;
        // The DFT of each symbol starts in the middle of its guard.
        let mut start = t.peak - g / 2.0 + 1.0;
        while start - t.symbol >= 0.0 {
            start -= t.symbol;
        }
        let mut order = PskOrder::new(0.995);
        let mut prev: Vec<Option<Complex64>> = vec![None; freqs.len()];
        let mut power = vec![0.0f64; freqs.len()];
        let mut s = start;
        while (s.round() as usize) + tun <= x.len() {
            let a = s.round() as usize;
            for (i, &fk) in freqs.iter().enumerate() {
                let w = -TAU * fk / RATE;
                let step = Complex64::from_polar(1.0, w);
                let mut rot = Complex64::new(1.0, 0.0);
                let mut acc = Complex64::default();
                for &v in &x[a..a + tun] {
                    acc += Complex64::new(f64::from(v.re), f64::from(v.im)) * rot;
                    rot *= step;
                }
                power[i] += acc.norm_sqr();
                if let Some(p) = prev[i] {
                    let u = acc * p.conj();
                    order.push(Complex32::new(u.re as f32, u.im as f32));
                }
                prev[i] = Some(acc);
            }
            order.decay();
            s += t.symbol;
        }
        let mut sorted = power.clone();
        sorted.sort_by(f64::total_cmp);
        let med = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
        let on: Vec<bool> = power.iter().map(|&p| p > 0.2 * med).collect();
        f.carriers = on.iter().filter(|&&v| v).count() as i64;
        let mid = on.len() / 2;
        f.centre_off = on.len() >= 7
            && on[mid.saturating_sub(3)..(mid + 4).min(on.len())]
                .iter()
                .any(|&v| !v);
        f.order = order.order();
        f.m = (order.m2(), order.m4(), order.m8());
        let close = |a: f64, b: f64| (a - b).abs() <= 0.03 * b;
        let tu_ms = tu.samples / RATE * 1000.0;
        f.name = if self.wide {
            if (110..=140).contains(&f.carriers) && (17.0..=25.0).contains(&t.rate_hz) {
                Some("CIS-128".to_string())
            } else if (110..=140).contains(&f.carriers) && (36.0..=48.0).contains(&t.rate_hz) {
                Some("CIS-128 (6 kHz)".to_string())
            } else {
                None
            }
        } else {
            OFDM_TABLE
                .iter()
                .find(|r| close(tu_ms, r.1) && close(t.rate_hz, r.2))
                .map(|r| r.0.to_string())
        };
        f.reason = match &f.name {
            Some(n) => n.clone(),
            None => "an OFDM signal not in the table".to_string(),
        };
        if f.name.is_some() {
            self.named = f.name.clone();
        }
        self.found = f;
    }
}

impl Sub for Ofdm {
    fn push(&mut self, x: &[Complex32], _out: &mut Out) {
        for &v in x {
            self.scan.push(std::slice::from_ref(&v));
            self.hist.push(v);
            self.n += 1;
            if self.hist.len() > 2 * WINDOW {
                self.hist.drain(..WINDOW);
            }
            if self.n.is_multiple_of(HOP) && self.hist.len() >= WINDOW {
                self.analyse();
            }
        }
    }
    fn state(&self) -> u8 {
        if self.named.is_some() {
            2
        } else if self.found.tu.is_some() {
            1
        } else {
            0
        }
    }
    fn summary(&self) -> String {
        let f = &self.found;
        match (f.tu, f.rate) {
            (Some(tu), Some(r)) => format!(
                "{}: {} carriers {:.2} Hz apart, {r:.2} Bd, useful {:.2} ms",
                f.reason,
                f.carriers,
                RATE / tu,
                tu / RATE * 1000.0
            ),
            _ => f.reason.clone(),
        }
    }
    fn measure<'a>(&'a self, m: &mut super::Measures<'a>) {
        let f = &self.found;
        m.low_hz = f.band.map(|b| b.0.round());
        m.high_hz = f.band.map(|b| b.1.round());
        m.carriers = Some(f.carriers);
        m.spacing_hz = f.spacing.map(|v| (v * 100.0).round() / 100.0);
        m.symbol_rate = f.rate.map(|v| (v * 100.0).round() / 100.0);
        m.useful_ms = f.tu.map(|v| (v / RATE * 1e5).round() / 100.0);
        m.guard_ms =
            f.tu.zip(f.ts)
                .map(|(tu, ts)| ((ts - tu) / RATE * 1e5).round() / 100.0);
        m.modulation = Some(match f.order {
            Some(2) => "BPSK",
            Some(4) => "QPSK",
            Some(8) => "8-PSK",
            _ => "unknown",
        });
        m.m2 = Some((f.m.0 * 100.0).round() / 100.0);
        m.m4 = Some((f.m.1 * 100.0).round() / 100.0);
        m.m8 = Some((f.m.2 * 100.0).round() / 100.0);
        m.centre_off = Some(f.centre_off);
        m.pilot_3300 = Some(f.pilot);
    }
    fn snr_db(&self) -> Option<f64> {
        self.found.snr
    }
    fn identified(&self) -> Option<String> {
        self.named.clone()
    }
}

// ------------------------------------------------------------------------------------------------------------------------------
// CIS-3000

/// The samples of a CIS-3000 window (1 s).
const WINDOW_3000: usize = 12_000;
/// The rate-line transform.
const RATE_FFT: usize = 16_384;
/// The transform of the power lines.
const LINE_FFT: usize = 4096;

/// The `cis3000` receiver.
pub struct Cis3000 {
    scan: BandScan,
    taps: Vec<f32>,
    hist: Vec<Complex32>,
    n: u64,
    windows: u64,
    centre: Option<f64>,
    band: Option<(f64, f64)>,
    rate_acc: Vec<f64>,
    line_acc: Vec<Vec<f64>>,
    rate_line: Option<(f64, f64)>,
    lines: [Option<(f64, f64)>; 3],
    snr: Option<f64>,
    named: bool,
}

impl Cis3000 {
    /// The receiver with a root-raised-cosine matched filter of `roll_off`, 8 symbols long at 4 samples a symbol.
    pub fn new(roll_off: f64) -> Self {
        let taps: Vec<f32> = (0..=32)
            .map(|i| sdrkit_algo_dsp::pulse::rrc((i as f64 - 16.0) / 4.0, roll_off) as f32)
            .collect();
        Cis3000 {
            scan: BandScan::new(4096, RATE, 0.15).expect("a valid spectrum"),
            taps,
            hist: Vec::new(),
            n: 0,
            windows: 0,
            centre: None,
            band: None,
            rate_acc: vec![0.0; RATE_FFT],
            line_acc: vec![vec![0.0; LINE_FFT]; 12],
            rate_line: None,
            lines: [None; 3],
            snr: None,
            named: false,
        }
    }

    fn clear_acc(&mut self) {
        self.rate_acc.iter_mut().for_each(|v| *v = 0.0);
        self.line_acc
            .iter_mut()
            .for_each(|l| l.iter_mut().for_each(|v| *v = 0.0));
        self.windows = 0;
    }

    /// The strongest bin of `acc` between `lo` and `hi` (bins) over the median of that range, in dB, with its bin.
    fn line(acc: &[f64], lo: usize, hi: usize) -> Option<(usize, f64)> {
        let seg = &acc[lo..=hi];
        let mut s = seg.to_vec();
        s.sort_by(f64::total_cmp);
        let med = s[s.len() / 2].max(1e-30);
        let (k, &v) = seg.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
        Some((k + lo, 10.0 * (v / med).max(1e-30).log10()))
    }

    fn analyse(&mut self) {
        let noise = self.scan.noise(-5900.0, 5900.0);
        let band = self.scan.occupied(100.0, 5900.0, noise, 6.0, 60.0, 800.0);
        self.band = band;
        let Some((lo, hi)) = band else {
            self.centre = None;
            return;
        };
        self.snr = Some(self.scan.snr_db(lo, hi, noise));
        let c = 0.5 * (lo + hi);
        if self.centre.is_none_or(|p| (p - c).abs() > 40.0) {
            self.clear_acc();
        }
        self.centre = Some(c);
        let x = &self.hist[self.hist.len() - WINDOW_3000..];
        // To 0 Hz.
        let w = -TAU * c / RATE;
        let y: Vec<Complex32> = x
            .iter()
            .enumerate()
            .map(|(i, &v)| v * Complex32::from_polar(1.0, (w * i as f64).rem_euclid(TAU) as f32))
            .collect();
        // The symbol-rate line of the envelope.
        let mean = y.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / y.len() as f64;
        let mut buf: Vec<Complex32> = y
            .iter()
            .map(|v| Complex32::new((f64::from(v.norm_sqr()) - mean) as f32, 0.0))
            .collect();
        buf.resize(RATE_FFT, Complex32::default());
        Fft::forward(RATE_FFT).process(&mut buf);
        for (a, v) in self.rate_acc.iter_mut().zip(&buf) {
            *a += f64::from(v.norm_sqr());
        }
        let bin = RATE / RATE_FFT as f64;
        self.rate_line = Self::line(
            &self.rate_acc,
            (500.0 / bin) as usize,
            (5000.0 / bin) as usize,
        )
        .map(|(k, db)| (k as f64 * bin, db));
        // The matched filter at 4 samples a symbol, then the lines of the 2nd, 4th and 8th powers at each of the 4 phases.
        let z: Vec<Complex32> = (0..y.len().saturating_sub(self.taps.len()))
            .map(|i| {
                self.taps
                    .iter()
                    .enumerate()
                    .fold(Complex32::default(), |s, (j, &t)| s + y[i + j] * t)
            })
            .collect();
        let mut fft = Fft::forward(LINE_FFT);
        for phase in 0..4 {
            let sym: Vec<Complex32> = z
                .iter()
                .skip(phase)
                .step_by(4)
                .map(|v| v / v.norm().max(1e-20))
                .collect();
            for (pi, pw) in [2i32, 4, 8].into_iter().enumerate() {
                let mut b: Vec<Complex32> = sym.iter().map(|v| v.powi(pw)).collect();
                b.resize(LINE_FFT, Complex32::default());
                fft.process(&mut b);
                for (a, v) in self.line_acc[pi * 4 + phase].iter_mut().zip(&b) {
                    *a += f64::from(v.norm_sqr());
                }
            }
        }
        for pi in 0..3 {
            self.lines[pi] = (0..4)
                .filter_map(|phase| {
                    let acc = &self.line_acc[pi * 4 + phase];
                    let n = acc.len();
                    let (k, db) = Self::line(acc, 0, n - 1)?;
                    let kf = if k >= n / 2 {
                        k as f64 - n as f64
                    } else {
                        k as f64
                    };
                    Some((kf * 3000.0 / LINE_FFT as f64, db))
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));
        }
        self.windows += 1;
        if self.windows.is_multiple_of(8) {
            self.rate_acc.iter_mut().for_each(|v| *v *= 0.5);
            self.line_acc
                .iter_mut()
                .for_each(|l| l.iter_mut().for_each(|v| *v *= 0.5));
        }
        if self.identify() {
            self.named = true;
        }
    }

    fn order(&self) -> Option<u32> {
        [2u32, 4, 8]
            .into_iter()
            .zip(self.lines)
            .find(|(_, l)| l.is_some_and(|l| l.1 > 20.0))
            .map(|(o, _)| o)
    }

    fn identify(&self) -> bool {
        self.rate_line
            .is_some_and(|(f, db)| (2900.0..=3100.0).contains(&f) && db >= 16.0)
            && self.order() == Some(8)
    }
}

impl Sub for Cis3000 {
    fn push(&mut self, x: &[Complex32], _out: &mut Out) {
        for &v in x {
            self.scan.push(std::slice::from_ref(&v));
            self.hist.push(v);
            self.n += 1;
            if self.hist.len() > 2 * WINDOW_3000 {
                self.hist.drain(..WINDOW_3000);
            }
            if self.n.is_multiple_of(WINDOW_3000 as u64) && self.hist.len() >= WINDOW_3000 {
                self.analyse();
            }
        }
    }
    fn state(&self) -> u8 {
        if self.named {
            2
        } else if self.band.is_some() {
            1
        } else {
            0
        }
    }
    fn summary(&self) -> String {
        match (self.centre, self.rate_line) {
            (Some(c), Some((r, db))) => format!(
                "{}: centre {c:.0} Hz, symbol-rate line {r:.0} Hz ({db:.0} dB), order {}",
                if self.identify() {
                    "CIS-3000"
                } else {
                    "a single-carrier signal, not CIS-3000"
                },
                self.order()
                    .map_or("unknown".to_string(), |o| o.to_string())
            ),
            _ => "searching".to_string(),
        }
    }
    fn measure<'a>(&'a self, m: &mut super::Measures<'a>) {
        m.low_hz = self.band.map(|b| b.0.round());
        m.high_hz = self.band.map(|b| b.1.round());
        let fine = self.lines[2].map(|l| l.0 / 8.0).unwrap_or(0.0);
        m.centre_hz = self.centre.map(|c| ((c + fine) * 10.0).round() / 10.0);
        m.symbol_rate = self.rate_line.map(|l| l.0.round());
        m.modulation = Some(match self.order() {
            Some(2) => "BPSK",
            Some(4) => "QPSK",
            Some(8) => "8-PSK",
            _ => "unknown",
        });
        let db = |i: usize| self.lines[i].map(|l| (l.1 * 10.0).round() / 10.0);
        m.line2_db = db(0);
        m.line4_db = db(1);
        m.line8_db = db(2);
        m.rate_line_db = self.rate_line.map(|l| (l.1 * 10.0).round() / 10.0);
    }
    fn snr_db(&self) -> Option<f64> {
        self.snr.filter(|_| self.band.is_some())
    }
    fn identified(&self) -> Option<String> {
        self.named.then(|| "CIS-3000".to_string())
    }
}
