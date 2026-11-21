//! The TV standard detector of TV (auto): a pure, deterministic function of a capture that says which TV standard the channel
//! carries and its parameters, with the evidence for it.

use crate::band::{AvgSpectrum, BandFft};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::DEFAULT_MIN_FLOOR;
use sdrkit_algo_dsp::stats::plain_kurtosis_iq;
use sdrkit_algo_modem::ofdm::LagCorrelator;
use sdrkit_core::{Complex32, SignalType};

sdrkit_core::choice_enum! {
    /// The standards the detector distinguishes.
    pub enum Standard {
        /// No TV signal.
        None = ("none", "No TV signal"),
        /// A signal, standard unclear.
        Unknown = ("unknown", "Unknown"),
        /// DVB-T.
        DvbT = ("dvbt", "DVB-T"),
        /// DVB-T2.
        DvbT2 = ("dvbt2", "DVB-T2"),
        /// ISDB-T.
        IsdbT = ("isdbt", "ISDB-T"),
        /// ATSC 1.0.
        Atsc = ("atsc", "ATSC"),
        /// ATSC 3.0.
        Atsc3 = ("atsc3", "ATSC 3.0"),
        /// DTMB.
        Dtmb = ("dtmb", "DTMB"),
        /// DVB-C.
        DvbC = ("dvbc", "DVB-C"),
        /// DVB-S or S2.
        DvbS = ("dvbs", "DVB-S/S2"),
        /// Analogue TV.
        Analog = ("analog", "Analog"),
    }
}

impl Standard {
    /// The id of the stack that receives it (`None` for no stack).
    pub fn stack_id(self) -> Option<&'static str> {
        match self {
            Standard::DvbT => Some("dvbt"),
            Standard::DvbT2 => Some("dvb_terrestrial"),
            Standard::IsdbT => Some(crate::isdbt::ID),
            Standard::Atsc => Some(crate::atsc::ID),
            Standard::Atsc3 => Some(crate::atsc3::ID),
            Standard::Dtmb => Some(crate::dtmb::ID),
            Standard::DvbC => Some("dvbc"),
            Standard::DvbS => Some("dvbs"),
            Standard::Analog => Some("atv"),
            Standard::None | Standard::Unknown => None,
        }
    }
    /// The signal type that the host maps to a stack (`None` for a standard without one).
    pub fn signal_type(self) -> Option<SignalType> {
        match self {
            Standard::DvbT => Some(SignalType::DvbT),
            Standard::DvbT2 => Some(SignalType::DvbT2),
            Standard::IsdbT => Some(SignalType::IsdbT),
            Standard::Atsc => Some(SignalType::Atsc1),
            Standard::Dtmb => Some(SignalType::Dtmb),
            _ => None,
        }
    }
    /// Its name in the headline ("DVB-T2").
    pub fn name(self) -> &'static str {
        match self {
            Standard::None => "no TV signal",
            Standard::Unknown => "unknown",
            Standard::DvbT => "DVB-T",
            Standard::DvbT2 => "DVB-T2",
            Standard::IsdbT => "ISDB-T",
            Standard::Atsc => "ATSC",
            Standard::Atsc3 => "ATSC 3.0",
            Standard::Dtmb => "DTMB",
            Standard::DvbC => "DVB-C",
            Standard::DvbS => "DVB-S/S2",
            Standard::Analog => "Analog",
        }
    }
}

sdrkit_core::choice_enum! {
    /// How sure the verdict is, from the least to the most sure.
    pub enum Verdict {
        /// Nothing.
        NoSignal = ("no_signal", "No signal"),
        /// Unsure.
        Unsure = ("unsure", "Unsure"),
        /// Likely.
        Likely = ("likely", "Likely"),
        /// Sure.
        Sure = ("sure", "Sure"),
    }
}

impl PartialOrd for Verdict {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Verdict {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use sdrkit_core::Choice;
        self.index().cmp(&other.index())
    }
}

/// What the detector found.
#[derive(Clone, Debug, PartialEq)]
pub struct Detection {
    /// The standard.
    pub standard: Standard,
    /// The verdict.
    pub verdict: Verdict,
    /// Confidence 0..1.
    pub confidence: f64,
    /// Channel width in MHz (0 when not known).
    pub width_mhz: f64,
    /// FFT size (OFDM), or 0.
    pub fft: usize,
    /// Guard as a fraction of the useful symbol (OFDM), or 0.
    pub guard: f64,
    /// ISDB-T mode (1..3) or 0.
    pub isdbt_mode: u8,
    /// ATSC 3.0 guard in samples, or 0.
    pub guard_samples: usize,
    /// DTMB header ("PN945"), or empty.
    pub dtmb_header: String,
    /// Symbol rate of a single carrier, Msym/s.
    pub symbol_rate_msps: f64,
    /// Constellation word of a single carrier.
    pub constellation: String,
    /// Analogue system ("B/G").
    pub analog_system: String,
    /// The channel's centre relative to the capture's centre, Hz.
    pub centre_offset_hz: f64,
    /// The headline.
    pub headline: String,
    /// Evidence lines.
    pub evidence: Vec<String>,
}

impl Detection {
    fn new() -> Detection {
        Detection {
            standard: Standard::None,
            verdict: Verdict::NoSignal,
            confidence: 0.0,
            width_mhz: 0.0,
            fft: 0,
            guard: 0.0,
            isdbt_mode: 0,
            guard_samples: 0,
            dtmb_header: String::new(),
            symbol_rate_msps: 0.0,
            constellation: String::new(),
            analog_system: String::new(),
            centre_offset_hz: 0.0,
            headline: String::new(),
            evidence: Vec::new(),
        }
    }
}

/// An OFDM (or DTMB) candidate.
#[derive(Clone, Debug)]
struct Cand {
    std: Standard,
    width: f64,
    fft: usize,
    mode: u8,
    // Useful symbol (or the PN core lag) in seconds.
    tu: f64,
    guards: Vec<f64>,
    share: f64,
    dtmb: Option<(&'static str, f64, f64)>, // header, frame period s, window s
    t2_only: bool,
    z: f64,
    lag: usize,
}

fn guard_name(g: f64) -> String {
    for (v, n) in [
        (0.25, "1/4"),
        (19.0 / 128.0, "19/128"),
        (0.125, "1/8"),
        (19.0 / 256.0, "19/256"),
        (1.0 / 16.0, "1/16"),
        (1.0 / 32.0, "1/32"),
        (1.0 / 128.0, "1/128"),
    ] {
        if (g - v).abs() < 1e-6 {
            return n.into();
        }
    }
    format!("{g:.4}")
}

fn fft_name(n: usize) -> String {
    format!("{}K", n / 1024)
}

/// The occupied band: (low Hz, high Hz, plateau − floor dB, edge softness). Powers count as at least [`DEFAULT_MIN_FLOOR`]
/// (−120 dB of full scale), so digital silence beside the signal is a floor and not minus infinity.
fn occupied(p: &[f64], rate: f64) -> Option<(f64, f64, f64, f64)> {
    let n = p.len();
    let min = f64::from(DEFAULT_MIN_FLOOR);
    let db: Vec<f64> = p.iter().map(|v| 10.0 * v.max(min).log10()).collect();
    let w = ((0.004 * n as f64) as usize).max(1);
    let mut sm = vec![0.0; n];
    for i in 0..n {
        let lo = i.saturating_sub(w);
        let hi = (i + w + 1).min(n);
        sm[i] = db[lo..hi].iter().sum::<f64>() / (hi - lo) as f64;
    }
    let e = (0.02 * n as f64) as usize;
    let inner = &sm[e..n - e];
    let mut sorted = inner.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted[(0.08 * sorted.len() as f64) as usize];
    let top = sorted[(0.98 * sorted.len() as f64) as usize];
    // The strongest wide stretch: start at the maximum of a 5 % moving average.
    let mw = ((0.05 * n as f64) as usize).max(1);
    let mut best = (e, f64::MIN);
    for i in e..n - e - mw {
        let v = sm[i..i + mw].iter().sum::<f64>();
        if v > best.1 {
            best = (i + mw / 2, v);
        }
    }
    let mut thr = floor + 0.5 * (top - floor);
    let gap = (0.025 * n as f64) as usize;
    let grow = |thr: f64| {
        let (mut lo, mut hi) = (best.0, best.0);
        let mut miss = 0;
        let mut i = best.0;
        while i > e {
            i -= 1;
            if sm[i] > thr {
                lo = i;
                miss = 0;
            } else {
                miss += 1;
                if miss > gap {
                    break;
                }
            }
        }
        miss = 0;
        i = best.0;
        while i + 1 < n - e {
            i += 1;
            if sm[i] > thr {
                hi = i;
                miss = 0;
            } else {
                miss += 1;
                if miss > gap {
                    break;
                }
            }
        }
        (lo, hi)
    };
    let (mut lo, mut hi) = grow(thr);
    let mut inside: Vec<f64> = sm[lo..=hi].to_vec();
    inside.sort_by(|a, b| a.total_cmp(b));
    let plateau = inside[inside.len() / 2];
    thr = floor + 0.5 * (plateau - floor);
    (lo, hi) = grow(thr);
    if plateau - floor < 4.0 {
        return None;
    }
    let width_at = |frac: f64| {
        let t = floor + frac * (plateau - floor);
        let (a, b) = grow(t);
        (b - a + 1) as f64
    };
    let soft = (width_at(0.2) - width_at(0.8)) / width_at(0.5).max(1.0);
    let bw = rate / n as f64;
    Some((
        (lo as f64 - n as f64 / 2.0) * bw,
        (hi as f64 + 1.0 - n as f64 / 2.0) * bw,
        plateau - floor,
        soft,
    ))
}

/// Narrow lines: (bin, dB above the local median), strongest first. The median counts as at least [`DEFAULT_MIN_FLOOR`].
fn lines(p: &[f64]) -> Vec<(usize, f64)> {
    let n = p.len();
    let mut out = Vec::new();
    for i in 2..n - 2 {
        if !(p[i] > p[i - 1] && p[i] >= p[i + 1]) {
            continue;
        }
        let lo = i.saturating_sub(64);
        let hi = (i + 65).min(n);
        let mut v: Vec<f64> = (lo..hi).step_by(4).map(|k| p[k]).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        let med = v[v.len() / 2].max(f64::from(DEFAULT_MIN_FLOOR));
        let db = 10.0 * (p[i] / med).log10();
        if db > 14.0 {
            out.push((i, db));
        }
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.truncate(32);
    out
}

/// Detects the standard of a capture at `rate`, RF centre `rf_hz`. `satellite` when the input is from a dish.
pub fn detect(x: &[Complex32], rate: f64, rf_hz: f64, satellite: bool) -> Detection {
    let mut d = Detection::new();
    if x.len() < 65_536 {
        d.headline = "too few samples".into();
        d.evidence
            .push(format!("{} samples, at least 65536 are needed", x.len()));
        return d;
    }
    let satellite = satellite || (940e6..=2200e6).contains(&rf_hz) || rf_hz > 2.5e9;
    // 1. Spectrum.
    let mut sp = AvgSpectrum::new(BandFft::N4096);
    sp.add_all(x);
    let p = sp.mean();
    let n = p.len();
    let bw = rate / n as f64;
    let ls = lines(&p);
    d.evidence.push(format!(
        "spectrum: {} frames of 4096, {} narrow lines",
        sp.frames,
        ls.len()
    ));
    // 2. Analogue.
    if !satellite {
        for &(i, db) in &ls {
            if db < 22.0 {
                continue;
            }
            for (sep, sys, w) in [
                (4.5e6, "M/N", 6.0),
                (5.5e6, "B/G", 8.0),
                (6.0e6, "I", 8.0),
                (6.5e6, "D/K or L", 8.0),
            ] {
                let j = i as f64 + sep / bw;
                if ls.iter().any(|&(k, _)| (k as f64 - j).abs() * bw <= 60e3) {
                    d.standard = Standard::Analog;
                    d.verdict = Verdict::Sure;
                    d.confidence = 0.95;
                    d.analog_system = sys.into();
                    d.width_mhz = w;
                    d.evidence.push(format!(
                        "vision carrier {db:.0} dB over the picture, sound carrier {:.1} MHz above",
                        sep / 1e6
                    ));
                    return finish(d);
                }
            }
        }
    }
    // 3. Occupied band.
    let Some((lo, hi, contrast, soft)) = occupied(&p, rate) else {
        d.evidence.push("no signal above the noise floor".into());
        return finish(d);
    };
    let occ = hi - lo;
    d.centre_offset_hz = 0.5 * (lo + hi);
    d.evidence.push(format!(
        "occupied {:.2} MHz from {:+.2} to {:+.2} MHz, {contrast:.1} dB over the floor, edge softness {soft:.3}",
        occ / 1e6,
        lo / 1e6,
        hi / 1e6
    ));
    // 8. ATSC 1.0: a pilot line near the lower edge.
    if !satellite && (4.6e6..=6.4e6).contains(&occ) {
        let found = ls.iter().find(|&&(i, db)| {
            db >= 14.0 && ((i as f64 - n as f64 / 2.0) * bw - lo).abs() <= 0.45e6
        });
        if let Some(&(_, db)) = found {
            d.standard = Standard::Atsc;
            d.verdict = Verdict::Sure;
            d.confidence = 0.95;
            d.width_mhz = 6.0;
            d.evidence.push(format!(
                "a pilot line {db:.0} dB near the lower edge: 8-VSB"
            ));
            return finish(d);
        }
    }
    // 4. Autocorrelation of the whole capture.
    let m = x.len().min(1 << 22);
    let nf = (2 * m).next_power_of_two();
    let mut buf = vec![Complex32::new(0.0, 0.0); nf];
    buf[..m].copy_from_slice(&x[..m]);
    for v in buf.iter_mut() {
        if !(v.re.is_finite() && v.im.is_finite()) {
            *v = Complex32::new(0.0, 0.0);
        }
    }
    Fft::forward(nf).process(&mut buf);
    for v in buf.iter_mut() {
        *v = Complex32::new(v.norm_sqr(), 0.0);
    }
    Fft::inverse(nf).process(&mut buf);
    let r0 = buf[0].re.max(1e-30);
    let max_lag = m / 2;
    let r: Vec<f32> = (0..max_lag).map(|l| buf[l].norm() / r0).collect();
    let mut tail: Vec<f32> = r[64.min(max_lag)..].to_vec();
    tail.sort_by(|a, b| a.total_cmp(b));
    let noise = 1.25 * tail.get(tail.len() / 2).copied().unwrap_or(1e-9).max(1e-9) as f64;
    let mut cands = Vec::new();
    let dvb_g = vec![
        0.25,
        19.0 / 128.0,
        0.125,
        19.0 / 256.0,
        1.0 / 16.0,
        1.0 / 32.0,
        1.0 / 128.0,
    ];
    for w in [8.0, 7.0, 6.0, 5.0, 1.7] {
        for k in 0..6 {
            let fft = 1024usize << k;
            let tu = if w == 1.7 {
                71.0 / 131.0 * 1e-6 * fft as f64
            } else {
                7.0 / (8.0 * w) * 1e-6 * fft as f64
            };
            let t2_only = w == 1.7 || fft == 1024 || fft >= 16384;
            let share = if fft >= 16384 { 0.97 } else { 0.952 };
            cands.push(Cand {
                std: Standard::DvbT,
                width: w,
                fft,
                mode: 0,
                tu,
                guards: dvb_g.clone(),
                share,
                dtmb: None,
                t2_only,
                z: 0.0,
                lag: 0,
            });
        }
    }
    for w in [6.0, 7.0, 8.0] {
        for mode in 1..=3u8 {
            let tu = 252e-6 * (1 << (mode - 1)) as f64 * 6.0 / w;
            let fft = 2048usize << (mode - 1);
            cands.push(Cand {
                std: Standard::IsdbT,
                width: w,
                fft,
                mode,
                tu,
                guards: vec![0.25, 0.125, 1.0 / 16.0, 1.0 / 32.0],
                share: 0.929,
                dtmb: None,
                t2_only: false,
                z: 0.0,
                lag: 0,
            });
        }
    }
    let a3_guards = [
        192usize, 384, 512, 768, 1024, 1536, 2048, 2432, 3072, 3648, 4096, 4864,
    ];
    for (nn, w) in [(0usize, 6.0), (2, 6.0), (5, 7.0), (8, 8.0)] {
        let fs3 = (nn as f64 + 16.0) * 0.384e6;
        for fft in [8192usize, 16384, 32768] {
            let gs: Vec<f64> = a3_guards
                .iter()
                .filter(|&&g| g < fft / 2)
                .map(|&g| g as f64 / fft as f64)
                .collect();
            cands.push(Cand {
                std: Standard::Atsc3,
                width: w,
                fft,
                mode: nn as u8,
                tu: fft as f64 / fs3,
                guards: gs,
                share: 0.8438 * fs3 / (w * 1e6),
                dtmb: None,
                t2_only: false,
                z: 0.0,
                lag: 0,
            });
        }
    }
    for (w, fsym) in [(8.0, 7.56e6), (6.0, 5.67e6)] {
        for (hdr, core, frame, win) in [
            ("PN420", 255.0, 4200.0, 165.0),
            ("PN945", 511.0, 4725.0, 434.0),
        ] {
            cands.push(Cand {
                std: Standard::Dtmb,
                width: w,
                fft: 0,
                mode: 0,
                tu: core / fsym,
                guards: vec![],
                share: 0.99,
                dtmb: Some((hdr, frame / fsym, win / fsym)),
                t2_only: false,
                z: 0.0,
                lag: 0,
            });
        }
    }
    let cap = m as f64 / rate;
    let mut ok: Vec<Cand> = Vec::new();
    for mut c in cands {
        if c.tu * 1.02 > cap / 2.0 {
            continue;
        }
        let l0 = c.tu * rate;
        let span = (l0 * 1500e-6).ceil() as usize + 1;
        let (a, b) = (
            (l0 as usize).saturating_sub(span),
            ((l0 as usize) + span).min(max_lag - 1),
        );
        if a >= b {
            continue;
        }
        let Some((lag, v)) = (a..=b)
            .map(|l| (l, r[l]))
            .max_by(|p, q| p.1.total_cmp(&q.1))
        else {
            continue;
        };
        c.z = v as f64 / noise;
        c.lag = lag;
        let own = c.share * c.width * 1e6;
        if c.z >= 6.0 && (occ - own).abs() <= 0.25 * own {
            ok.push(c);
        }
    }
    ok.sort_by(|a, b| b.z.total_cmp(&a.z));
    ok.truncate(6);
    // 5. Guard by folding.
    let mut scored: Vec<(f64, Cand, f64, f64)> = Vec::new(); // score, cand, guard, K
    for c in &ok {
        let l = c.lag;
        let periods: Vec<(f64, f64)> = if let Some((_, frame, win)) = c.dtmb {
            vec![(frame * rate, win * rate)]
        } else {
            c.guards
                .iter()
                .map(|&g| (l as f64 * (1.0 + g), l as f64 * g))
                .collect()
        };
        for (gi, &(per, win)) in periods.iter().enumerate() {
            let pl = per.round() as usize;
            let wl = win.round().max(1.0) as usize;
            if pl < 2 || pl + l >= m || pl > 400_000 {
                continue;
            }
            // The lag correlation over a window of one guard, summed over every period at the same place in the period.
            let Ok(mut lc) = LagCorrelator::new(l, wl, 1.0, 1.0) else {
                continue;
            };
            let mut fold = vec![Complex32::new(0.0, 0.0); pl];
            for (i, &v) in x[..m].iter().enumerate() {
                let (_, p) = lc.push(v);
                if i + 1 >= l + wl {
                    fold[i % pl] += p;
                }
            }
            let sums: Vec<f32> = fold.iter().map(|v| v.norm()).collect();
            let peak = sums.iter().cloned().fold(0f32, f32::max) as f64;
            let mut so = sums.clone();
            so.sort_by(|a, b| a.total_cmp(b));
            let med = so[so.len() / 2].max(1e-30) as f64;
            let k = peak / med;
            let own = c.share * c.width * 1e6;
            let fit = (occ - own).abs() / own;
            let kf = if k >= 3.0 {
                1.0
            } else if k >= 2.0 {
                0.5
            } else {
                0.05
            };
            let wf = if fit <= 0.08 {
                1.0
            } else if fit <= 0.15 {
                0.5
            } else {
                0.15
            };
            let sharp = if c.std == Standard::Dtmb { 0.065 } else { 0.03 };
            let ef = if soft <= sharp || k >= 6.0 { 1.0 } else { 0.05 };
            let g = c.guards.get(gi).copied().unwrap_or(0.0);
            scored.push((c.z * kf * wf * ef, c.clone(), g, k));
        }
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    if let Some((score, c, g, k)) = scored.first().cloned().filter(|s| s.0 >= 5.0) {
        d.evidence.push(format!(
            "cyclic structure at lag {} ({:.1} us): z {:.1}, guard concentration {k:.1}, score {score:.1}",
            c.lag,
            c.lag as f64 / rate * 1e6,
            c.z
        ));
        let mut std = c.std;
        if std == Standard::DvbT {
            let t2_guard = [19.0 / 128.0, 19.0 / 256.0, 1.0 / 128.0]
                .iter()
                .any(|v| (v - g).abs() < 1e-9);
            if c.t2_only || t2_guard || p1_present(x, rate, c.width) {
                std = Standard::DvbT2;
                d.evidence
                    .push("a P1 symbol or a guard only DVB-T2 has: DVB-T2".into());
            }
        }
        d.standard = std;
        d.width_mhz = c.width;
        d.fft = c.fft;
        d.guard = g;
        d.isdbt_mode = if std == Standard::IsdbT { c.mode } else { 0 };
        if std == Standard::Atsc3 {
            d.guard_samples = (g * c.fft as f64).round() as usize;
        }
        if let Some((h, _, _)) = c.dtmb {
            d.dtmb_header = h.into();
        }
        let own = c.share * c.width * 1e6;
        let fit = (occ - own).abs() / own;
        if c.z >= 12.0 && k >= 3.0 && fit <= 0.08 {
            d.verdict = Verdict::Sure;
            d.confidence = 0.97;
        } else {
            d.verdict = Verdict::Likely;
            d.confidence = 0.8;
        }
        if std == Standard::DvbT && cap < 0.27 && d.verdict == Verdict::Sure {
            d.verdict = Verdict::Likely;
            d.confidence = 0.8;
        }
        if let Some(r) = scored
            .iter()
            .find(|s| s.1.std != c.std && s.0 >= 0.8 * score)
        {
            d.verdict = Verdict::Unsure;
            d.confidence = 0.5;
            d.evidence.push(format!(
                "runner-up {} with score {:.1}",
                r.1.std.name(),
                r.0
            ));
        }
        return finish(d);
    }
    // 9. Single carrier.
    single_carrier(x, rate, occ, satellite, &mut d);
    finish(d)
}

/// Looks for DVB-T2 P1 symbols (1K FFT with the C-A-B structure at 542 T and 482 T).
fn p1_present(x: &[Complex32], rate: f64, width: f64) -> bool {
    let t = 7.0 / (8.0 * width) * 1e-6 * rate; // samples per elementary period
    let lag = (1024.0 * t).round() as usize;
    let win = (542.0 * t).round() as usize;
    if x.len() < 2 * lag + win + 1 || lag == 0 {
        return false;
    }
    let shift = std::f64::consts::TAU / (1024.0 * t);
    let n = x.len() - lag - 1;
    let step = (t.round() as usize).max(1);
    let mut vals = Vec::new();
    // The window sum runs in f64 and is summed again exactly every 4 windows, so it cannot drift over a long capture.
    let term = |i: usize| {
        let ph = shift * i as f64;
        let v = x[i] * x[i + lag].conj();
        let (sn, cs) = ph.sin_cos();
        (
            v.re as f64 * cs - v.im as f64 * sn,
            v.re as f64 * sn + v.im as f64 * cs,
        )
    };
    let (mut sr, mut si) = (0f64, 0f64);
    let mut since = 0usize;
    for i in 0..n {
        let (a, b) = term(i);
        sr += a;
        si += b;
        if i >= win {
            let (a, b) = term(i - win);
            sr -= a;
            si -= b;
        }
        since += 1;
        if since >= 4 * win && i >= win {
            (sr, si) = (0.0, 0.0);
            for j in i + 1 - win..=i {
                let (a, b) = term(j);
                sr += a;
                si += b;
            }
            since = 0;
        }
        if i % step == 0 && i >= win {
            vals.push((sr * sr + si * si).sqrt() as f32);
        }
    }
    if vals.is_empty() {
        return false;
    }
    let mut so = vals.clone();
    so.sort_by(|a, b| a.total_cmp(b));
    let med = so[so.len() / 2].max(1e-30);
    let mut peaks = 0;
    let mut last = usize::MAX - 10;
    for (i, &v) in vals.iter().enumerate() {
        if v > 8.0 * med && i.wrapping_sub(last) > 4 {
            peaks += 1;
            last = i;
        }
    }
    peaks >= 1
}

fn single_carrier(x: &[Complex32], rate: f64, occ: f64, satellite: bool, d: &mut Detection) {
    // Symbol rate from the strongest line of |x|^2.
    let n = x.len().min(1 << 20).next_power_of_two() / 2;
    let mut b: Vec<Complex32> = x[..n]
        .iter()
        .map(|v| Complex32::new(v.norm_sqr(), 0.0))
        .collect();
    let mean = b.iter().map(|v| v.re).sum::<f32>() / n as f32;
    b.iter_mut().for_each(|v| v.re -= mean);
    Fft::forward(n).process(&mut b);
    let bwb = rate / n as f64;
    let (lo, hi) = (
        ((occ / 1.55) / bwb) as usize,
        (((occ / 0.98) / bwb) as usize).min(n / 2 - 1),
    );
    let mut rs = occ / 1.2;
    let mut line = false;
    if lo < hi {
        let (k, v) = (lo..=hi)
            .map(|k| (k, b[k].norm()))
            .max_by(|p, q| p.1.total_cmp(&q.1))
            .expect("the bin range lo..=hi is not empty");
        let blk = (k.saturating_sub(128)..(k + 128).min(n / 2))
            .map(|i| b[i].norm())
            .collect::<Vec<_>>();
        let mut so = blk.clone();
        so.sort_by(|a, c| a.total_cmp(c));
        if v > 6.0 * so[so.len() / 2] {
            line = true;
            let (a, c) = (b[k - 1].norm() as f64, b[k + 1].norm() as f64);
            let dd = 0.5 * (a - c) / (a - 2.0 * v as f64 + c).min(-1e-30);
            rs = (k as f64 + dd) * bwb;
        }
    }
    // Fourth moment (noise not removed: a rough PSK/QAM split).
    let kurt = plain_kurtosis_iq(x).unwrap_or(0.0);
    d.symbol_rate_msps = rs / 1e6;
    d.evidence.push(format!(
        "single carrier: symbol rate {:.3} Msym/s ({}), fourth moment {kurt:.2}",
        rs / 1e6,
        if line { "line" } else { "assumed" }
    ));
    if kurt > 1.6 {
        d.standard = Standard::Unknown;
        d.verdict = Verdict::Unsure;
        d.confidence = 0.5;
        d.evidence
            .push("Gaussian-like samples: not a single carrier".into());
        return;
    }
    let qam = kurt > 1.2;
    d.constellation = if qam {
        "16-QAM or higher"
    } else {
        "QPSK or PSK"
    }
    .into();
    if line
        && !satellite
        && ((rs - 7.56e6).abs() < 0.004 * 7.56e6 || (rs - 5.67e6).abs() < 0.004 * 5.67e6)
    {
        d.standard = Standard::Dtmb;
        d.verdict = Verdict::Likely;
        d.confidence = 0.8;
        d.width_mhz = if rs > 7e6 { 8.0 } else { 6.0 };
        d.dtmb_header = "PN595".into();
        return;
    }
    if satellite {
        d.standard = Standard::DvbS;
        d.verdict = if qam {
            Verdict::Unsure
        } else {
            Verdict::Likely
        };
        d.confidence = if qam { 0.5 } else { 0.8 };
    } else if qam {
        d.standard = Standard::DvbC;
        d.verdict = if line {
            Verdict::Likely
        } else {
            Verdict::Unsure
        };
        d.confidence = if line { 0.8 } else { 0.5 };
    } else {
        d.standard = Standard::DvbS;
        d.verdict = Verdict::Unsure;
        d.confidence = 0.5;
    }
}

fn finish(mut d: Detection) -> Detection {
    let v = match d.verdict {
        Verdict::Sure => "sure",
        Verdict::Likely => "likely",
        Verdict::Unsure => "unsure",
        Verdict::NoSignal => "no signal",
    };
    let tail = format!("({v}, {:.0} %)", d.confidence * 100.0);
    d.headline = match d.standard {
        Standard::None => {
            if d.headline.is_empty() {
                "no TV signal found".into()
            } else {
                d.headline.clone()
            }
        }
        Standard::Unknown => "a signal, but the standard is unclear".into(),
        Standard::DvbT | Standard::DvbT2 => {
            format!(
                "{}, {}, GI {}, {} MHz {tail}",
                d.standard.name(),
                fft_name(d.fft),
                guard_name(d.guard),
                d.width_mhz
            )
        }
        Standard::IsdbT => format!(
            "ISDB-T, mode {}, GI {}, {} MHz {tail}",
            d.isdbt_mode,
            guard_name(d.guard),
            d.width_mhz
        ),
        Standard::Atsc3 => format!(
            "ATSC 3.0, {}, GI {}, {} MHz {tail}",
            fft_name(d.fft),
            d.guard_samples,
            d.width_mhz
        ),
        Standard::Dtmb => format!("DTMB, {}, {} MHz {tail}", d.dtmb_header, d.width_mhz),
        Standard::Atsc => format!("ATSC, 8-VSB, 6 MHz {tail}"),
        Standard::DvbS | Standard::DvbC => {
            format!(
                "{}, {:.3} Msym/s, {} {tail}",
                d.standard.name(),
                d.symbol_rate_msps,
                d.constellation
            )
        }
        Standard::Analog => format!("Analog, system {} {tail}", d.analog_system),
    };
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digital_silence_is_the_least_floor() {
        let (n, rate) = (4096usize, 8e6);
        let hz = |i: usize| (i as f64 - n as f64 / 2.0) * rate / n as f64;
        // Nothing at all: no band, no line.
        let zeros = vec![0.0; n];
        assert!(occupied(&zeros, rate).is_none());
        assert!(lines(&zeros).is_empty());
        // A flat channel 6 MHz wide at -30 dB of full scale with digital silence beside it: the floor is -120 dB.
        let mut p: Vec<f64> = (0..n)
            .map(|i| if hz(i).abs() <= 3e6 { 1e-3 } else { 0.0 })
            .collect();
        let (lo, hi, contrast, _) = occupied(&p, rate).expect("a band");
        assert!((contrast - 90.0).abs() < 0.01, "{contrast}");
        assert!(
            (lo + 3e6).abs() < 50e3 && (hi - 3e6).abs() < 50e3,
            "{lo} {hi}"
        );
        // A line in the silence stands over the least floor, not over nothing.
        p[100] = 1e-9;
        let l = lines(&p);
        assert!(
            l.iter()
                .any(|&(i, db)| i == 100 && (db - 30.0).abs() < 0.01),
            "{l:?}"
        );
    }
}
