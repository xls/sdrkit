//! The cdma2000 pilot search of one capture: every pilot by the code phase of the pilot PN, tracked through the capture, and the
//! Sync Channel of each pilot despread, decoded and read.

use super::sync::{frame_soft, Capsule, Reassembly, SyncMessage, FRAME_BITS, FRAME_SYMBOLS};
use sdrkit_algo_fec::conv::codes::CDMA2000_K9;
use super::{matched_taps, CHIP_RATE};
use crate::decoder::Capture;
use crate::dsp::{resample_to, ResamplerCache};
use sdrkit_algo_dsp::fir::Convolver;
use sdrkit_algo_fec::conv::{Ends, Viterbi};
use sdrkit_algo_modem::dsss::segmented::SegmentedSearch;
use sdrkit_algo_modem::spreading::cdma2000::{chip as pn_chip, PN_PERIOD};
use sdrkit_algo_modem::spreading::hadamard;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The working rate: two samples a chip.
pub const WORK_RATE: f64 = 2.0 * CHIP_RATE;
/// Samples of a PN period at the working rate.
const PERIOD: usize = 2 * PN_PERIOD;
/// The width of the channel low-pass of the front end (two-sided), Hz.
const CHANNEL_HZ: f64 = 1.3e6;
/// Chips of a coherent segment of the acquisition and the tracking.
const SEGMENT: usize = 1024;
/// The step of the carrier hypotheses, FFT bins of 18.75 Hz (506.25 Hz).
const HYPOTHESIS_BINS: usize = 27;
/// A candidate within this many chips of a stronger one is the same pilot.
const PEAK_GUARD: usize = 4;
/// The tracking searches the predicted start within this many samples.
const TRACK_SPAN: i64 = 4;
/// Pilots without a message below this Ec/Io are dropped, dB.
const MIN_EC_IO_DB: f64 = -24.0;

/// One pilot found in a capture.
#[derive(Clone, Debug, PartialEq)]
pub struct CdmaFound {
    /// The start of the pilot's PN period in the capture, samples at the working rate (0 to one period).
    pub start: f64,
    /// The PN offset relative to the strongest pilot of the capture, units of 64 chips.
    pub relative: u16,
    /// Ec/Io, dB.
    pub ec_io_db: f64,
    /// Pilot level, dBFS.
    pub level_dbfs: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Sample clock error, ppm.
    pub clock_ppm: f64,
    /// The newest good Sync Channel Message.
    pub msg: Option<SyncMessage>,
    /// Good messages.
    pub good: u32,
    /// Capsules that failed their CRC.
    pub bad: u32,
    /// The PN offset derived from another pilot of the capture that has a message.
    pub derived_pn: Option<u16>,
}

/// The searcher: the front end, the code search and the buffers, kept from capture to capture.
pub struct CdmaSearcher {
    cache: ResamplerCache,
    mf: Option<Convolver<Complex32>>,
    search: Option<SegmentedSearch>,
    /// The complex pilot PN as `(I, Q)` values.
    pn: Vec<(f32, f32)>,
    raw: Vec<Complex32>,
    y: Vec<Complex32>,
    yd: Vec<Complex32>,
    even: Vec<Complex32>,
}

impl Default for CdmaSearcher {
    fn default() -> Self {
        let pn: Vec<(f32, f32)> = (0..PN_PERIOD)
            .map(|n| {
                let (i, q) = pn_chip(n);
                (f32::from(i), f32::from(q))
            })
            .collect();
        let code: Vec<Complex32> = pn.iter().map(|&(i, q)| Complex32::new(i, q)).collect();
        CdmaSearcher {
            cache: ResamplerCache::default(),
            mf: Convolver::new(&matched_taps()).ok(),
            search: SegmentedSearch::new(&code, SEGMENT).ok(),
            pn,
            raw: Vec::new(),
            y: Vec::new(),
            yd: Vec::new(),
            even: Vec::new(),
        }
    }
}

/// A candidate of the acquisition: its PN start (sample) and carrier (Hz).
#[derive(Clone, Copy, Debug)]
struct Candidate {
    start: usize,
    freq_hz: f64,
    power: f32,
}

/// The track of one pilot: PN period `p` starts at sample `a + b p`.
#[derive(Clone, Copy, Debug)]
struct Track {
    a: f64,
    b: f64,
}

impl CdmaSearcher {
    /// The pilots of `cap`: carriers within `max_offset_hz`, candidates over `threshold` times the median, at most `max_cells`.
    pub fn search(
        &mut self,
        cap: &Capture,
        max_offset_hz: f64,
        max_cells: usize,
        threshold: f64,
    ) -> Vec<CdmaFound> {
        if cap.rate <= 0.0 || cap.samples.is_empty() {
            return Vec::new();
        }
        let n_out = (cap.samples.len() as f64 * WORK_RATE / cap.rate) as usize;
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            cap.dial_offset_hz,
            WORK_RATE,
            CHANNEL_HZ,
            0.0,
            0.0,
            n_out,
            &mut self.raw,
        );
        self.y.clear();
        self.y.resize(self.raw.len(), Complex32::default());
        match self.mf.as_ref() {
            Some(mf) if mf.centred(&self.raw, &mut self.y).is_ok() => {}
            _ => self.y.copy_from_slice(&self.raw),
        }
        if self.y.len() < 2 * PERIOD + 16 {
            return Vec::new();
        }
        let io = self.y.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / self.y.len() as f64;
        let cands = self.acquire(max_offset_hz, max_cells + 2, threshold);
        let mut kept: Vec<CdmaFound> = Vec::new();
        for c in cands {
            if kept.len() >= max_cells {
                break;
            }
            let near = kept.iter().any(|k| {
                let d = (k.start - c.start as f64).rem_euclid(PERIOD as f64);
                d.min(PERIOD as f64 - d) <= 8.0
            });
            if near {
                continue;
            }
            let Some(f) = self.measure(c, io) else { continue };
            let same_pn = f.msg.as_ref().is_some_and(|m| {
                kept.iter()
                    .any(|k| k.msg.as_ref().is_some_and(|n| n.pilot_pn == m.pilot_pn))
            });
            if same_pn || (f.msg.is_none() && f.ec_io_db < MIN_EC_IO_DB) {
                continue;
            }
            kept.push(f);
        }
        // Offsets relative to the first (strongest) pilot, and PN offsets derived from a pilot with a message.
        if let Some(first) = kept.first().map(|k| k.start) {
            for k in kept.iter_mut() {
                let d = (k.start - first).rem_euclid(PERIOD as f64);
                k.relative = ((d / 128.0).round() as i64).rem_euclid(512) as u16;
            }
        }
        if let Some((rel, pn)) = kept
            .iter()
            .find_map(|k| k.msg.as_ref().map(|m| (k.relative, m.pilot_pn)))
        {
            for k in kept.iter_mut().filter(|k| k.msg.is_none()) {
                k.derived_pn =
                    Some((i64::from(pn) + i64::from(k.relative) - i64::from(rel)).rem_euclid(512) as u16);
            }
        }
        kept
    }

    /// Stage 2: the code phases whose power is at least `threshold` times the median and the largest within 4 chips, strongest
    /// first, at most `most`.
    fn acquire(&mut self, max_offset_hz: f64, most: usize, threshold: f64) -> Vec<Candidate> {
        let Some(search) = self.search.as_mut() else {
            return Vec::new();
        };
        self.even.clear();
        self.even.extend(self.y.iter().step_by(2).take(2 * PN_PERIOD).copied());
        let step_hz = HYPOTHESIS_BINS as f64 * CHIP_RATE / (2 * PN_PERIOD) as f64;
        let half = (max_offset_hz / step_hz).ceil() as usize;
        let Ok(p) = search.search(&self.even, HYPOTHESIS_BINS, half) else {
            return Vec::new();
        };
        let mut sorted = p.power.clone();
        let mid = sorted.len() / 2;
        let median = *sorted.select_nth_unstable_by(mid, |a, b| a.total_cmp(b)).1;
        let n = p.power.len();
        let mut cands: Vec<Candidate> = (0..n)
            .filter(|&d| {
                let v = p.power[d];
                f64::from(v) >= threshold * f64::from(median)
                    && (1..=PEAK_GUARD).all(|k| {
                        v >= p.power[(d + k) % n] && v > p.power[(d + n - k) % n]
                    })
            })
            .map(|d| Candidate {
                start: 2 * d,
                freq_hz: f64::from(p.hypothesis[d]) * step_hz,
                power: p.power[d],
            })
            .collect();
        cands.sort_by(|a, b| b.power.total_cmp(&a.power));
        cands.truncate(most);
        cands
    }

    /// The energy of the PN period that starts at sample `at` of the derotated samples: 32 coherent segments added in power.
    fn energy(&self, at: i64) -> f64 {
        if at < 0 || at as usize + PERIOD > self.yd.len() {
            return 0.0;
        }
        let at = at as usize;
        let mut e = 0.0;
        for seg in 0..PN_PERIOD / SEGMENT {
            let (mut re, mut im) = (0f32, 0f32);
            for n in seg * SEGMENT..(seg + 1) * SEGMENT {
                let v = self.yd[at + 2 * n];
                let (pi, pq) = self.pn[n];
                re += v.re * pi + v.im * pq;
                im += v.im * pi - v.re * pq;
            }
            e += f64::from(re * re + im * im);
        }
        e
    }

    /// Stage 3: the PN period starts through the capture, a line by energy-weighted least squares. `None` with fewer than 4
    /// periods.
    fn track(&self, first: usize) -> Option<Track> {
        let mut pts: Vec<(f64, f64, f64)> = Vec::new();
        let mut p = 0;
        loop {
            let pred = match pts.len() {
                0 => first as f64,
                1 => pts[0].1 + PERIOD as f64 * (p as f64 - pts[0].0),
                _ => {
                    let t = fit(&pts);
                    t.a + t.b * p as f64
                }
            };
            let base = pred.round() as i64;
            if base + TRACK_SPAN + PERIOD as i64 + 2 > self.yd.len() as i64 {
                break;
            }
            let e: Vec<f64> = (-TRACK_SPAN..=TRACK_SPAN).map(|d| self.energy(base + d)).collect();
            let (k, &peak) = e.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
            let frac = if k > 0 && k + 1 < e.len() {
                let (l, r) = (e[k - 1], e[k + 1]);
                let den = l - 2.0 * peak + r;
                if den < 0.0 {
                    0.5 * (l - r) / den
                } else {
                    0.0
                }
            } else {
                0.0
            };
            pts.push((p as f64, (base + k as i64 - TRACK_SPAN) as f64 + frac, peak));
            p += 1;
        }
        (pts.len() >= 4).then(|| fit(&pts))
    }

    /// Stages 3 to 7 for one candidate: track, despread, measure and decode. `io` is the mean power of the filtered samples.
    fn measure(&mut self, c: Candidate, io: f64) -> Option<CdmaFound> {
        let f0 = c.freq_hz / WORK_RATE;
        self.yd.clear();
        self.yd.extend(
            self.y
                .iter()
                .enumerate()
                .map(|(n, v)| v * Complex32::from_polar(1.0, (-TAU * f0 * n as f64) as f32)),
        );
        let mut t = self.track(c.start)?;
        if t.a < 0.0 {
            t.a += t.b;
        }
        let spc = t.b / PN_PERIOD as f64;
        // Stage 4: the pilot and Sync values of each Walsh period.
        let walsh = ((self.yd.len() as f64 - t.a - 2.0) / (64.0 * spc)).floor().max(0.0) as usize;
        let mut zp = Vec::with_capacity(walsh);
        let mut zs = Vec::with_capacity(walsh);
        let w32: Vec<f32> = (0..64).map(|n| f32::from(hadamard(32, n))).collect();
        for k in 0..walsh {
            let (mut p, mut s) = (Complex32::default(), Complex32::default());
            for n in 0..64 {
                let ch = 64 * k + n;
                let x = t.a + ch as f64 * spc;
                let i = x.floor() as usize;
                let fr = (x - i as f64) as f32;
                let v = self.yd[i] * (1.0 - fr) + self.yd[i + 1] * fr;
                let (pi, pq) = self.pn[ch % PN_PERIOD];
                let d = Complex32::new(v.re * pi + v.im * pq, v.im * pi - v.re * pq);
                p += d;
                s += d * w32[n];
            }
            zp.push(p);
            zs.push(s);
        }
        if zp.len() < 128 {
            return None;
        }
        // The carrier refined from the phase advance of the pilot over 8, then 64 Walsh periods.
        let mut dphi = 0.0;
        for lag in [8usize, 64] {
            let z: Complex32 = (0..zp.len().saturating_sub(lag))
                .map(|k| zp[k + lag] * zp[k].conj())
                .sum();
            let step = f64::from(z.im.atan2(z.re)) / lag as f64;
            for (k, (p, s)) in zp.iter_mut().zip(zs.iter_mut()).enumerate() {
                let r = Complex32::from_polar(1.0, (-step * k as f64) as f32);
                *p *= r;
                *s *= r;
            }
            dphi += step;
        }
        let walsh_s = 64.0 * spc / WORK_RATE;
        let offset_hz = c.freq_hz + dphi / TAU / walsh_s;
        // Stage 5: Sync symbols with the pilot of the 16 Walsh periods around them as the phase.
        let symbols = zs.len() / 4;
        let mut soft = Vec::with_capacity(symbols);
        let mut ec = 0.0;
        for m in 0..symbols {
            let sym: Complex32 = zs[4 * m..4 * m + 4].iter().sum();
            let lo = (4 * m).saturating_sub(6);
            let hi = (4 * m + 10).min(zp.len());
            let pilot: Complex32 = zp[lo..hi].iter().sum();
            let scale = 16.0 / (hi - lo) as f32;
            ec += f64::from((pilot * scale).norm_sqr());
            let a = pilot.norm().max(1e-12);
            soft.push((sym * pilot.conj()).re / a);
        }
        // Stage 6: Ec and Ec/Io. A chip of the complex PN has power 2.
        let ec = ec / symbols as f64 / (16.0 * 16.0 * 64.0 * 64.0 * 2.0);
        let ec_io_db = 10.0 * (ec / io.max(1e-30)).max(1e-30).log10();
        let level_dbfs = 10.0 * ec.max(1e-30).log10();
        let rms = (soft.iter().map(|v| v * v).sum::<f32>() / soft.len().max(1) as f32).sqrt();
        if rms > 0.0 {
            soft.iter_mut().for_each(|v| *v /= rms);
        }
        // Stage 7: frames of 128 symbols from the PN period start, one Viterbi stream over all of them.
        let frames = soft.len() / FRAME_SYMBOLS;
        let mut llr = Vec::with_capacity(64 * frames);
        for fr in 0..frames {
            let s = frame_soft(&soft[FRAME_SYMBOLS * fr..FRAME_SYMBOLS * (fr + 1)]);
            llr.extend(s.iter().map(|v| (v * 16.0).round().clamp(-127.0, 127.0) as i8));
        }
        let (mut msg, mut good, mut bad) = (None, 0, 0);
        if frames > 0 {
            if let Ok(mut vit) = Viterbi::new(CDMA2000_K9, 32 * frames) {
                let mut bits = vec![0u8; 32 * frames];
                if vit.decode_only(&llr, Ends::Open, &mut bits).is_ok() {
                    let mut re = Reassembly::default();
                    for fb in bits.chunks(FRAME_BITS) {
                        match re.push(fb) {
                            Some(Capsule::Good(pdu)) => {
                                if let Some(m) = SyncMessage::parse(&pdu) {
                                    msg = Some(m);
                                    good += 1;
                                }
                            }
                            Some(Capsule::Bad) => bad += 1,
                            None => {}
                        }
                    }
                }
            }
        }
        Some(CdmaFound {
            start: t.a,
            relative: 0,
            ec_io_db,
            level_dbfs,
            offset_hz,
            clock_ppm: (t.b / PERIOD as f64 - 1.0) * 1e6,
            msg,
            good,
            bad,
            derived_pn: None,
        })
    }
}

/// The energy-weighted least-squares line through `(period, start, weight)` points.
fn fit(pts: &[(f64, f64, f64)]) -> Track {
    let w: f64 = pts.iter().map(|p| p.2).sum::<f64>().max(1e-300);
    let mx = pts.iter().map(|p| p.2 * p.0).sum::<f64>() / w;
    let my = pts.iter().map(|p| p.2 * p.1).sum::<f64>() / w;
    let sxx: f64 = pts.iter().map(|p| p.2 * (p.0 - mx).powi(2)).sum();
    let sxy: f64 = pts.iter().map(|p| p.2 * (p.0 - mx) * (p.1 - my)).sum();
    let b = if sxx > 0.0 { sxy / sxx } else { PERIOD as f64 };
    Track { a: my - b * mx, b }
}
