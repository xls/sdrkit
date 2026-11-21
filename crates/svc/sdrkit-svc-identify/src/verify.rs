//! The verification of the best candidates with detectors of their own: frame syncs, the symbol clock at the exact rate, the
//! cyclic prefix at the exact symbol length, chirps, burst timing, deviation and the FM pilot. A stack can add a [`Detector`].

use crate::features::{discriminator, Channel, Features};
use crate::psd::{interp, median, moving_average};
use crate::signature::{FrameSync, ModClass, Signature};
use sdrkit_algo_dsp::meter::power_to_db;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// What a check found.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Check {
        /// The candidate's own detector found it.
        Confirmed = ("confirmed", "Confirmed"),
        /// It must have been found and was not.
        Failed = ("failed", "Not found"),
        /// Nothing can be said (too weak, too short).
        Inconclusive = ("inconclusive", "Inconclusive"),
    }
}

/// The result of verifying one candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verification {
    /// The overall result.
    pub check: Check,
    /// One line per check made.
    pub notes: Vec<String>,
    /// How much a confirmation counts (1 for a long frame sync or an exact measurement, less for a short sync that data can
    /// imitate).
    pub weight: f64,
}

impl Verification {
    fn one(check: Check, note: String) -> Self {
        Verification {
            check,
            notes: vec![note],
            weight: 1.0,
        }
    }
    /// Merges several checks: confirmed if any confirmed, else failed if any failed, else inconclusive.
    pub fn merge(parts: Vec<Verification>) -> Option<Verification> {
        if parts.is_empty() {
            return None;
        }
        let check = if parts.iter().any(|p| p.check == Check::Confirmed) {
            Check::Confirmed
        } else if parts.iter().any(|p| p.check == Check::Failed) {
            Check::Failed
        } else {
            Check::Inconclusive
        };
        let weight = parts
            .iter()
            .filter(|p| p.check == check)
            .map(|p| p.weight)
            .fold(0.0, f64::max);
        Some(Verification {
            check,
            notes: parts.into_iter().flat_map(|p| p.notes).collect(),
            weight,
        })
    }
}

/// A cheap detector for one or more signal types. It runs on the signal at 0 Hz ([`Channel`]) after the features are known.
pub trait Detector: Send + Sync {
    /// A name for the notes.
    fn name(&self) -> &str;
    /// Checks the candidate; `None` when this detector has nothing to say about it.
    fn verify(&self, sig: &Signature, f: &Features, ch: &Channel) -> Option<Verification>;
}

/// What a frame sync search found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SyncFound {
    /// Non-overlapping matches.
    pub hits: usize,
    /// Matches of the reversed pattern (what chance returns).
    pub control: usize,
    /// The best normalised correlation.
    pub best: f64,
    /// The symbols came upside down.
    pub inverted: bool,
}

/// Searches the instantaneous frequency of `ch` for `sync` at `rate` symbols a second, at eight timing phases.
pub fn sync_search(ch: &Channel, rate: f64, sync: &FrameSync) -> SyncFound {
    let fr = discriminator(&ch.samples, ch.rate);
    sync_search_freq(&fr, &ch.on, ch.rate, rate, sync)
}

/// [`sync_search`] on an instantaneous frequency already computed.
pub fn sync_search_freq(
    fr: &[f32],
    on: &[bool],
    fs: f64,
    rate: f64,
    sync: &FrameSync,
) -> SyncFound {
    let sps = fs / rate;
    let l = sync.symbols.len();
    if sps < 1.5 || l < 4 || fr.len() < (sps * (l as f64 + 4.0)) as usize {
        return SyncFound::default();
    }
    let sm = moving_average(fr, ((sps * 0.4).round() as usize).max(1));
    let thr = if l >= 16 { 0.85 } else { 0.93 };
    let pat: Vec<f64> = sync.symbols.iter().map(|s| *s as f64).collect();
    let rev: Vec<f64> = pat.iter().rev().copied().collect();
    let mut best = SyncFound::default();
    for phase in 0..8 {
        let mut v: Vec<f64> = Vec::new();
        let mut t = phase as f64 * sps / 8.0;
        while t < (sm.len() - 1) as f64 {
            v.push(if on[t as usize] {
                interp(&sm, t) as f64
            } else {
                f64::NAN
            });
            t += sps;
        }
        // The levels' middle is the carrier offset: take it out.
        let good: Vec<f32> = v
            .iter()
            .filter(|x| !x.is_nan())
            .map(|x| *x as f32)
            .collect();
        let m = median(&good);
        for x in &mut v {
            *x -= m;
        }
        if sync.nrzi {
            // 1 keeps the level (+1), 0 changes it (−1).
            let mut d = vec![f64::NAN; v.len()];
            for k in 1..v.len() {
                d[k] = if v[k].is_nan() || v[k - 1].is_nan() {
                    f64::NAN
                } else if (v[k] > 0.0) == (v[k - 1] > 0.0) {
                    1.0
                } else {
                    -1.0
                };
            }
            v = d;
        }
        let (hits, b, inv) = count_hits(&v, &pat, thr);
        let (control, _, _) = count_hits(&v, &rev, thr);
        if hits > best.hits || (hits == best.hits && b > best.best) {
            best = SyncFound {
                hits,
                control,
                best: b,
                inverted: inv,
            };
        }
    }
    best
}

fn count_hits(v: &[f64], pat: &[f64], thr: f64) -> (usize, f64, bool) {
    let l = pat.len();
    let pm = pat.iter().sum::<f64>() / l as f64;
    let pc: Vec<f64> = pat.iter().map(|p| p - pm).collect();
    let pn = pc.iter().map(|p| p * p).sum::<f64>().sqrt();
    if pn == 0.0 || v.len() < l {
        return (0, 0.0, false);
    }
    let (mut hits, mut best, mut inv, mut neg) = (0usize, 0f64, false, 0usize);
    let mut k = 0;
    while k + l <= v.len() {
        let w = &v[k..k + l];
        if w.iter().any(|x| x.is_nan()) {
            k += 1;
            continue;
        }
        let wm = w.iter().sum::<f64>() / l as f64;
        let mut num = 0.0;
        let mut wn = 0.0;
        for (x, p) in w.iter().zip(&pc) {
            let d = x - wm;
            num += d * p;
            wn += d * d;
        }
        let c = if wn > 0.0 {
            num / (wn.sqrt() * pn)
        } else {
            0.0
        };
        if c.abs() > best {
            best = c.abs();
        }
        if c.abs() >= thr {
            hits += 1;
            if c < 0.0 {
                neg += 1;
            }
            k += l;
        } else {
            k += 1;
        }
    }
    if hits > 0 && neg * 2 > hits {
        inv = true;
    }
    (hits, best, inv)
}

/// Frame syncs.
pub struct SyncDetector;

impl Detector for SyncDetector {
    fn name(&self) -> &str {
        "frame sync"
    }
    fn verify(&self, sig: &Signature, f: &Features, ch: &Channel) -> Option<Verification> {
        if sig.frame_sync.is_empty() || sig.symbol_rates.is_empty() {
            return None;
        }
        let fr = discriminator(&ch.samples, ch.rate);
        let mut found: Option<(String, f64, SyncFound)> = None;
        for &rate in &sig.symbol_rates {
            for s in &sig.frame_sync {
                let r = sync_search_freq(&fr, &ch.on, ch.rate, rate, s);
                if found.as_ref().is_none_or(|b| r.hits > b.2.hits) {
                    found = Some((s.name.clone(), rate, r));
                }
            }
        }
        let (name, rate, r) = found?;
        let short = sig.frame_sync.iter().all(|s| s.symbols.len() < 16);
        let need = if short { 4 } else { 2 };
        let ok = r.hits >= need && r.hits >= 3 * r.control + need;
        let note = format!(
            "{name} at {rate:.0} Bd: {} found (reversed pattern {}), best match {:.2}{}",
            r.hits,
            r.control,
            r.best,
            if r.inverted { ", inverted" } else { "" }
        );
        let mut v = Verification::one(
            if ok {
                Check::Confirmed
            } else if f.snr_db >= 12.0 && f.capture_s >= 0.3 {
                Check::Failed
            } else {
                Check::Inconclusive
            },
            note,
        );
        if short {
            v.weight = 0.5;
        }
        Some(v)
    }
}

/// The cyclic prefix at the exact useful symbol lengths.
pub struct CyclicPrefixDetector;

/// The normalised correlation of `y` at lag `l` over the median of lags around it.
pub fn lag_z(y: &[sdrkit_core::Complex32], l: usize) -> f64 {
    let n = y.len().min(1 << 18);
    if l * 2 >= n || l < 8 {
        return 0.0;
    }
    let at = |lag: usize| -> f64 {
        let mut acc = sdrkit_core::Complex32::default();
        let mut p = 0.0f64;
        let step = ((n - lag) / 65_536).max(1);
        let mut i = 0;
        while i + lag < n {
            acc += y[i] * y[i + lag].conj();
            p += y[i].norm_sqr() as f64;
            i += step;
        }
        acc.norm() as f64 / p.max(1e-30)
    };
    let c = at(l);
    let others: Vec<f32> = [0.71, 0.77, 0.83, 0.89, 1.13, 1.19, 1.27, 1.33]
        .iter()
        .map(|k| at(((l as f64) * k) as usize) as f32)
        .collect();
    c / median(&others).max(1e-6)
}

impl Detector for CyclicPrefixDetector {
    fn name(&self) -> &str {
        "cyclic prefix"
    }
    fn verify(&self, sig: &Signature, _f: &Features, ch: &Channel) -> Option<Verification> {
        if sig.ofdm_tu_s.is_empty() {
            return None;
        }
        let mut best = (0.0, 0.0);
        for tu in &sig.ofdm_tu_s {
            let l = (tu * ch.rate).round() as usize;
            let z = lag_z(&ch.samples, l);
            if z > best.0 {
                best = (z, *tu);
            }
        }
        let note = format!(
            "cyclic prefix at Tu {:.1} µs: {:.1} times the other lags",
            best.1 * 1e6,
            best.0
        );
        Some(Verification::one(
            if best.0 >= 8.0 {
                Check::Confirmed
            } else if best.0 < 3.0 {
                Check::Failed
            } else {
                Check::Inconclusive
            },
            note,
        ))
    }
}

/// The symbol clock at the exact rate: a line of the envelope (PSK) or of the frequency transitions (FSK) there and not beside.
pub struct SymbolClockDetector;

fn tone_power(g: &[f32], fs: f64, hz: f64) -> f64 {
    let n = g.len().min(1 << 18);
    let m = g[..n].iter().map(|v| *v as f64).sum::<f64>() / n.max(1) as f64;
    // A rotating phasor instead of a sine and cosine per sample, renormalised now and then.
    let (ws, wc) = (-TAU * hz / fs).sin_cos();
    let (mut pr, mut pi) = (1.0f64, 0.0f64);
    let (mut re, mut im) = (0f64, 0f64);
    for (i, v) in g[..n].iter().enumerate() {
        let x = *v as f64 - m;
        re += x * pr;
        im += x * pi;
        let r = pr * wc - pi * ws;
        pi = pr * ws + pi * wc;
        pr = r;
        if i % 1024 == 1023 {
            let k = 1.0 / (pr * pr + pi * pi).sqrt();
            pr *= k;
            pi *= k;
        }
    }
    re * re + im * im
}

impl Detector for SymbolClockDetector {
    fn name(&self) -> &str {
        "symbol clock"
    }
    fn verify(&self, sig: &Signature, f: &Features, ch: &Channel) -> Option<Verification> {
        if sig.symbol_rates.is_empty() || !sig.frame_sync.is_empty() || f.duty < 0.9 {
            return None;
        }
        let fsk = matches!(
            sig.main_class,
            ModClass::Fsk2 | ModClass::Fsk4 | ModClass::Gmsk
        );
        let g: Vec<f32> = if fsk {
            let fr = discriminator(&ch.samples, ch.rate);
            let sm = moving_average(&fr, 3);
            sm.windows(2).map(|w| (w[1] - w[0]).abs()).collect()
        } else {
            ch.samples.iter().map(|s| s.norm_sqr()).collect()
        };
        let n = g.len().min(1 << 18);
        if n < 1024 {
            return None;
        }
        let res = ch.rate / n as f64;
        let mut best = (f64::MIN, 0.0);
        for &r in &sig.symbol_rates {
            if r >= ch.rate / 2.0 {
                continue;
            }
            let on = tone_power(&g[..n], ch.rate, r);
            let side: Vec<f32> = [-7.3, -5.1, -3.2, 3.3, 5.2, 7.1]
                .iter()
                .map(|k| tone_power(&g[..n], ch.rate, r + k * res * 4.0) as f32)
                .collect();
            let s = power_to_db(on / median(&side).max(1e-30));
            if s > best.0 {
                best = (s, r);
            }
        }
        if best.0 == f64::MIN {
            return None;
        }
        Some(Verification::one(
            if best.0 >= 10.0 {
                Check::Confirmed
            } else if best.0 < 3.0 && f.snr_db > 12.0 {
                Check::Failed
            } else {
                Check::Inconclusive
            },
            format!(
                "symbol clock at {:.0} Bd: {:.0} dB over its neighbours",
                best.1, best.0
            ),
        ))
    }
}

/// Measurements a signature fixes exactly: chirp spreading factor, burst timing, FM pilot.
pub struct MeasureDetector;

impl Detector for MeasureDetector {
    fn name(&self) -> &str {
        "measurements"
    }
    fn verify(&self, sig: &Signature, f: &Features, _ch: &Channel) -> Option<Verification> {
        let mut parts = Vec::new();
        if !sig.chirp_sf.is_empty() {
            parts.push(match f.chirp.and_then(|c| c.sf) {
                Some(sf) if sig.chirp_sf.contains(&sf) => {
                    Verification::one(Check::Confirmed, format!("chirps of SF{sf}"))
                }
                Some(sf) => {
                    Verification::one(Check::Failed, format!("chirps of SF{sf}, not one it uses"))
                }
                None => Verification::one(Check::Inconclusive, "no spreading factor read".into()),
            });
        }
        if let (Some(p), Some(m)) = (sig.period_s, f.burst_period_s) {
            let ok = (m / p - 1.0).abs() < 0.1 || (m / (p / 2.0) - 1.0).abs() < 0.1;
            parts.push(Verification::one(
                if ok { Check::Confirmed } else { Check::Failed },
                format!(
                    "bursts every {:.2} ms (expected {:.2} ms)",
                    m * 1e3,
                    p * 1e3
                ),
            ));
        }
        if sig.fm_pilot {
            parts.push(Verification::one(
                if f.fm_pilot {
                    Check::Confirmed
                } else {
                    Check::Failed
                },
                format!(
                    "19 kHz pilot {}",
                    if f.fm_pilot { "found" } else { "not found" }
                ),
            ));
        }
        Verification::merge(parts)
    }
}

/// The detectors every identifier starts with.
pub fn builtin_detectors() -> Vec<Arc<dyn Detector>> {
    vec![
        Arc::new(SyncDetector),
        Arc::new(CyclicPrefixDetector),
        Arc::new(SymbolClockDetector),
        Arc::new(MeasureDetector),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::dmr_syncs;

    #[test]
    fn finds_a_sync_in_clean_symbols() {
        let fs = 48_000.0;
        let rate = 4800.0;
        let s = &dmr_syncs()[0];
        // Random levels with the sync every 144 symbols, as a stepped frequency.
        let mut r = crate::synth::Rand::new(3);
        let mut sym: Vec<f64> = (0..2000)
            .map(|_| [-3.0, -1.0, 1.0, 3.0][r.below(4) as usize])
            .collect();
        let mut k = 10;
        while k + 24 < sym.len() {
            for (j, v) in s.symbols.iter().enumerate() {
                sym[k + j] = *v as f64;
            }
            k += 144;
        }
        let fr: Vec<f32> = (0..(2000 * 10))
            .map(|i| (sym[i / 10] * 648.0) as f32)
            .collect();
        let on = vec![true; fr.len()];
        let found = sync_search_freq(&fr, &on, fs, rate, s);
        assert!(found.hits >= 12, "{found:?}");
        assert!(found.control <= 1);
        // Inverted symbols are found as inverted.
        let neg: Vec<f32> = fr.iter().map(|v| -v).collect();
        let found = sync_search_freq(&neg, &on, fs, rate, s);
        assert!(found.hits >= 12 && found.inverted);
    }
}
