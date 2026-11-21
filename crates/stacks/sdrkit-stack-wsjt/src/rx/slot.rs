//! What every decoder shares: the slot and decode records, the slot's spectrum and narrow basebands cut from it, spectrograms,
//! soft decisions, SNR, subtraction of decoded signals and the call / locator extraction.

use crate::mode::{Mode, Sub};
use num_complex::Complex32;
use sdrkit_algo_dsp::fft::{Fft, FftPlans};
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_fec::ldpc::{BpDecoder, LdpcCode, Osd};
use sdrkit_algo_modem::fsk::FskLlr;
use std::f64::consts::PI;
use std::sync::Arc;

/// One slot of audio of one mode (stream kind `wsjt.slot`).
#[derive(Clone, Debug)]
pub struct Slot {
    /// The mode, sub-mode and period.
    pub sub: Sub,
    /// The UTC of the slot start, seconds since 1970 (or since the given start).
    pub utc: f64,
    /// The audio, 8000 samples a second.
    pub audio: Arc<Vec<f32>>,
    /// Samples a second of `audio`.
    pub rate: f64,
    /// The time of the first sample against the slot start, seconds (negative: before it).
    pub t_first: f64,
    /// A slot-search window (starts searched from -0.05 to +0.55 of the period around the nominal start).
    pub search: bool,
    /// The dial frequency the slot was heard on, Hz (0: not known).
    pub dial_hz: f64,
    /// The slot index (UTC seconds / period).
    pub index: i64,
    /// An early cut: the audio ends before the window does (the slot is still being received).
    pub early: bool,
    /// The number the cutter gave the slot, counting from 1 (0: none).
    pub seq: u64,
}

sdrkit_core::choice_enum! {
    /// How a decode was made.
    #[derive(Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum How {
        /// From this slot alone.
        #[default]
        Alone = ("alone", "This slot"),
        /// With the soft bits of earlier slots added.
        Combined = ("combined", "(combined)"),
        /// With the bits of a message heard on the band assumed.
        APriori = ("a_priori", "(a-priori)"),
    }
}

/// One decode (stream kind `wsjt.decode`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Decode {
    /// The slot start, UTC seconds.
    pub slot: f64,
    /// The mode id.
    pub mode: Mode,
    /// The sub-mode label (`JT65A`, `Q65-60A` ...).
    pub label: String,
    /// SNR in 2500 Hz, dB.
    pub snr: i32,
    /// Start against the nominal start, seconds.
    pub dt: f64,
    /// Audio frequency of tone 0, Hz.
    pub hz: f64,
    /// The message.
    pub text: String,
    /// Heard upside down.
    pub mirrored: bool,
    /// A CQ (every WSPR decode counts as one).
    pub cq: bool,
    /// The calling station.
    pub call: String,
    /// Its locator.
    pub grid: String,
    /// WSPR and FST4W: the power sent, dBm; else -1.
    pub dbm: i32,
    /// 0 for the first pass.
    pub pass: u32,
    /// The dial frequency, Hz (0: not known).
    pub dial: f64,
    /// Decoded through the deep stage (OSD, a-priori bits).
    #[serde(default)]
    pub deep: bool,
    /// How it was decoded.
    #[serde(default)]
    pub how: How,
    /// Decoded from the first part of a slot still being received.
    #[serde(default)]
    pub early: bool,
    /// The band of the dial, from the band edges of the display; `None` when the dial is in no band.
    #[serde(default)]
    pub band: Option<String>,
}

impl Slot {
    /// The DT range searched in this slot: the mode's, or -0.05 to +0.55 of the period in a slot-search window.
    pub fn dt_range(&self) -> (f64, f64) {
        if self.search {
            (-0.05 * self.sub.period, 0.55 * self.sub.period)
        } else {
            self.sub.dt_range()
        }
    }
    /// The audio sample index of a time (seconds from the slot start).
    pub fn sample_of(&self, t: f64) -> f64 {
        (t - self.t_first) * self.rate
    }
    /// The time (seconds from the slot start) of an audio sample index.
    pub fn time_of(&self, i: f64) -> f64 {
        self.t_first + i / self.rate
    }
}

impl Decode {
    /// A decode of `text`, with the call and locator taken from it.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        slot: &Slot,
        text: String,
        snr: f64,
        dt: f64,
        hz: f64,
        mirrored: bool,
        pass: u32,
    ) -> Decode {
        let wspr = matches!(slot.sub.mode, Mode::Wspr | Mode::Fst4w);
        let (call, grid) = call_and_grid(&text, wspr);
        let cq = wspr || text.starts_with("CQ ");
        Decode {
            slot: slot.utc,
            mode: slot.sub.mode,
            label: slot.sub.label(),
            snr: (snr.round() as i32).max(-35),
            dt,
            hz,
            text,
            mirrored,
            cq,
            call,
            grid,
            dbm: -1,
            pass,
            dial: slot.dial_hz,
            deep: false,
            how: How::Alone,
            early: slot.early,
            band: None,
        }
    }
}

/// The calling station and its locator, from a message.
pub fn call_and_grid(text: &str, wspr: bool) -> (String, String) {
    let w: Vec<&str> = text.split_whitespace().collect();
    let clean = |c: &str| -> String {
        let c = c.trim_matches(|ch| ch == '<' || ch == '>');
        if c == "..." {
            String::new()
        } else {
            c.to_string()
        }
    };
    let is_grid = |g: &str| crate::proto::msg77::is_grid4(g) || crate::proto::msg77::is_grid6(g);
    if wspr {
        let call = w.first().map(|c| clean(c)).unwrap_or_default();
        let grid = w
            .get(1)
            .filter(|g| is_grid(g))
            .map(|g| g.to_string())
            .unwrap_or_default();
        return (call, grid);
    }
    if w.len() >= 4 && w[1] == "RR73;" {
        return (clean(w[3]), String::new());
    }
    if w.is_empty() {
        return (String::new(), String::new());
    }
    let (call_i, grid_i) = if w[0] == "CQ" || w[0] == "QRZ" || w[0] == "DE" {
        // CQ [modifier] CALL [GRID]
        if w.len() >= 3 && !is_grid(w[2]) || (w.len() >= 4) {
            (2, 3)
        } else {
            (1, 2)
        }
    } else {
        (1, 2)
    };
    let call = w.get(call_i).map(|c| clean(c)).unwrap_or_default();
    let last = w.last().copied().unwrap_or("");
    let grid = if w.len() > grid_i && is_grid(last) && last != "RR73" {
        last.to_string()
    } else {
        String::new()
    };
    // A single word (telemetry) has no call.
    if w.len() < 2 {
        return (String::new(), String::new());
    }
    (call, grid)
}

/// A planner and the transforms made with it, by size and direction.
type Plans = (FftPlans, Vec<((usize, bool), Fft)>);

thread_local! {
    /// The thread's planner (shared twiddles) and the transforms made with it.
    static PLANS: std::cell::RefCell<Plans> = std::cell::RefCell::new((FftPlans::new(), Vec::new()));
}

/// Runs `f` with the thread's transform of `n` points, made once per thread, size and direction by the shared planner.
fn with_plan<R>(n: usize, inverse: bool, f: impl FnOnce(&mut Fft) -> R) -> R {
    PLANS.with(|p| {
        let mut p = p.borrow_mut();
        let (plans, made) = &mut *p;
        let i = match made.iter().position(|(k, _)| *k == (n, inverse)) {
            Some(i) => i,
            None => {
                let fft = if inverse {
                    plans.inverse(n)
                } else {
                    plans.forward(n)
                };
                made.push(((n, inverse), fft));
                made.len() - 1
            }
        };
        f(&mut made[i].1)
    })
}

/// An FFT plan of `n` points (a copy of the thread's transform).
pub fn fft_plan(n: usize, inverse: bool) -> Fft {
    with_plan(n, inverse, |f| f.clone())
}

/// Runs an FFT of `buf.len()` points in place with the thread's transform.
pub fn fft_in_place(buf: &mut [Complex32], inverse: bool) {
    with_plan(buf.len(), inverse, |f| f.process(buf))
}

/// The smallest number at least `x` whose only prime factors are 2, 3 and 5.
pub fn smooth_at_least(x: usize) -> usize {
    let mut n = x.max(1);
    loop {
        let mut m = n;
        for p in [2, 3, 5] {
            while m.is_multiple_of(p) {
                m /= p;
            }
        }
        if m == 1 {
            return n;
        }
        n += 1;
    }
}

/// The spectrum of a slot's audio, for cutting narrow basebands from it.
pub struct SlotSpectrum {
    x: Vec<Complex32>,
    /// FFT size.
    pub n: usize,
    /// The audio rate.
    pub rate: f64,
}

impl SlotSpectrum {
    /// The spectrum of `audio` zero-padded to `n` points.
    pub fn new(audio: &[f32], n: usize) -> SlotSpectrum {
        let mut x: Vec<Complex32> = audio
            .iter()
            .take(n)
            .map(|&v| Complex32::new(if v.is_finite() { v } else { 0.0 }, 0.0))
            .collect();
        x.resize(n, Complex32::default());
        fft_in_place(&mut x, false);
        SlotSpectrum { x, n, rate: 8000.0 }
    }

    /// The complex baseband of `f_lo..f_hi` (audio Hz) with raised-cosine skirts of `skirt` Hz, shifted so that `centre_hz` lands
    /// near 0 Hz, as `m` samples over the whole slot (rate `m / n` of the audio rate). Returns the shift actually applied (the
    /// nearest bin to `centre_hz`). The samples are scaled so that a cosine of amplitude a has magnitude a.
    pub fn baseband(
        &self,
        centre_hz: f64,
        f_lo: f64,
        f_hi: f64,
        skirt: f64,
        m: usize,
        out: &mut Vec<Complex32>,
    ) -> f64 {
        let df = self.rate / self.n as f64;
        let kc = (centre_hz / df).round() as i64;
        out.clear();
        out.resize(m, Complex32::default());
        let k0 = ((f_lo - skirt) / df).floor().max(1.0) as i64;
        let k1 = (((f_hi + skirt) / df).ceil() as i64).min(self.n as i64 / 2 - 1);
        let scale = 2.0 / self.n as f32;
        for k in k0..=k1 {
            let f = k as f64 * df;
            let w = if f < f_lo {
                0.5 * (1.0 + (PI * (f_lo - f) / skirt).cos())
            } else if f > f_hi {
                0.5 * (1.0 + (PI * (f - f_hi) / skirt).cos())
            } else {
                1.0
            };
            let idx = (k - kc).rem_euclid(m as i64) as usize;
            out[idx] += self.x[k as usize] * (w as f32 * scale);
        }
        fft_in_place(out, true);
        kc as f64 * df
    }
}

/// Power spectra of symbol-long pieces of the audio, every `step` samples, each zero-padded to `nfft` (rectangular window, or
/// Hann when `hann`). Row `i` starts at sample `i * step`.
pub struct Spectrogram {
    /// Bins per row (`nfft / 2`).
    pub bins: usize,
    /// Rows.
    pub rows: usize,
    /// Samples between rows.
    pub step: usize,
    /// Hz per bin.
    pub df: f64,
    /// Powers, row-major.
    pub p: Vec<f32>,
}

impl Spectrogram {
    /// The spectrogram of `audio`: windows of `nsym` samples, `step` apart, FFTs of `nfft`.
    pub fn new(
        audio: &[f32],
        nsym: usize,
        step: usize,
        nfft: usize,
        hann: bool,
        rate: f64,
    ) -> Spectrogram {
        let rows = if audio.len() >= nsym {
            (audio.len() - nsym) / step + 1
        } else {
            0
        };
        let bins = nfft / 2;
        let mut p = vec![0f32; rows * bins];
        let mut fft = fft_plan(nfft, false);
        let mut buf = vec![Complex32::default(); nfft];
        let win: Vec<f32> = (0..nsym)
            .map(|i| {
                if hann {
                    (0.5 - 0.5 * (2.0 * PI * (i as f64 + 0.5) / nsym as f64).cos()) as f32
                } else {
                    1.0
                }
            })
            .collect();
        for r in 0..rows {
            let s = &audio[r * step..r * step + nsym];
            for (i, b) in buf.iter_mut().enumerate() {
                *b = if i < nsym {
                    let v = s[i];
                    Complex32::new(if v.is_finite() { v * win[i] } else { 0.0 }, 0.0)
                } else {
                    Complex32::default()
                };
            }
            fft.process(&mut buf);
            let row = &mut p[r * bins..(r + 1) * bins];
            for (o, b) in row.iter_mut().zip(&buf[..bins]) {
                *o = b.norm_sqr();
            }
        }
        Spectrogram {
            bins,
            rows,
            step,
            df: rate / nfft as f64,
            p,
        }
    }

    /// The power at `row`, `bin` (0 outside).
    #[inline]
    pub fn at(&self, row: i64, bin: i64) -> f32 {
        if row < 0 || bin < 0 || row as usize >= self.rows || bin as usize >= self.bins {
            0.0
        } else {
            self.p[row as usize * self.bins + bin as usize]
        }
    }
}

pub use sdrkit_algo_modem::fsk::ln_i0;

/// Bit LLRs of M-FSK symbols from their tone amplitudes, by the shared non-coherent demapper:
/// `amps[s * m + t]` is the magnitude of tone `t` of symbol `s`; `label[t]` the bits of tone `t` (most significant first, `bits`
/// per symbol). Appends `bits` LLRs per symbol to `out`, clamped to +-25. Nothing is appended when the amplitudes are not finite.
pub fn fsk_llrs(amps: &[f32], m: usize, bits: usize, label: &[u8], out: &mut Vec<f32>) {
    let labels: Vec<u16> = label.iter().take(m).map(|&l| l as u16).collect();
    let Ok(demap) = FskLlr::new(&labels) else {
        return;
    };
    debug_assert_eq!(demap.bits(), bits);
    let energies: Vec<f32> = amps.iter().map(|a| a * a).collect();
    let Some((a, n)) = FskLlr::estimate(&energies, m) else {
        return;
    };
    let start = out.len();
    out.resize(start + energies.len() / m * bits, 0.0);
    if demap.llrs(&energies, a, n, &mut out[start..]).is_err() {
        out.truncate(start);
        return;
    }
    for v in &mut out[start..] {
        *v = v.clamp(-25.0, 25.0);
    }
}

/// The belief-propagation and ordered-statistics decoders of one LDPC code, sized once for a slot.
pub struct LdpcDecoders {
    bp: BpDecoder,
    osd: Option<Osd>,
    post: Vec<f32>,
}

/// What [`LdpcDecoders::decode`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeWord {
    /// Code bits whose value differs from the hard decision of the input LLR.
    pub errors: usize,
    /// Found by OSD (the deep stage).
    pub osd: bool,
}

impl LdpcDecoders {
    /// The decoders of `code`.
    pub fn new(code: &LdpcCode) -> Self {
        LdpcDecoders {
            bp: BpDecoder::new(code),
            osd: Osd::new(code).ok(),
            post: vec![0.0; code.n()],
        }
    }

    /// As [`LdpcDecoders::decode`], with belief propagation that stops early once `patience` iterations in a row do not lower
    /// the number of unsatisfied checks (0: never).
    pub fn decode_patient(
        &mut self,
        llr: &[f32],
        iters: u32,
        patience: u32,
        osd: bool,
        out: &mut [u8],
    ) -> Option<CodeWord> {
        self.bp.set_early_stop(patience);
        let r = self.decode(llr, iters, osd, out);
        self.bp.set_early_stop(0);
        r
    }

    /// Decodes `llr` (positive means 0) into `out`: belief propagation with at most `iters` iterations, then, when `osd` is set and
    /// it failed, OSD of order 2 on the posterior LLRs. `None` when neither produces a code word.
    pub fn decode(
        &mut self,
        llr: &[f32],
        iters: u32,
        osd: bool,
        out: &mut [u8],
    ) -> Option<CodeWord> {
        let mut used_osd = false;
        if self.bp.decode(llr, iters, out).is_err() {
            let o = self.osd.as_mut().filter(|_| osd)?;
            self.post.clear();
            self.post.extend_from_slice(self.bp.posterior());
            o.decode(&self.post, 2, out).ok()?;
            used_osd = true;
        }
        let errors = out
            .iter()
            .zip(llr)
            .filter(|(&b, &l)| (b == 1) != (l < 0.0))
            .count();
        Some(CodeWord {
            errors,
            osd: used_osd,
        })
    }
}

/// The SNR in 2500 Hz from the powers of the sent tones against the mean power of the other tones of the same symbols.
pub fn snr_2500(p_sent: f64, p_other: f64, spacing_hz: f64) -> f64 {
    let r = (p_sent / p_other.max(1e-30) - 1.0).max(1e-6);
    (10.0 * r.log10() + 10.0 * (spacing_hz / 2500.0).log10()).max(-35.0)
}

/// Subtracts a decoded signal from `audio`: `copy[i]` is the clean complex copy (unit amplitude times the
/// envelope) of the sample at `start + i`; the complex amplitude is measured over a sliding window of `window` samples.
#[allow(clippy::needless_range_loop)]
pub fn subtract(audio: &mut [f32], start: i64, copy: &[Complex32], window: usize) {
    let n = copy.len();
    let w = window.max(1);
    // z = audio * conj(ref), and |ref|^2, as running sums.
    let mut z = vec![Complex32::default(); n];
    let mut e = vec![0f32; n];
    for i in 0..n {
        let j = start + i as i64;
        if j >= 0 && (j as usize) < audio.len() {
            let a = audio[j as usize];
            z[i] = copy[i].conj() * a;
            e[i] = copy[i].norm_sqr();
        }
    }
    let mut cz = vec![Complex32::default(); n + 1];
    let mut ce = vec![0f64; n + 1];
    for i in 0..n {
        cz[i + 1] = cz[i] + z[i];
        ce[i + 1] = ce[i] + e[i] as f64;
    }
    let half = w / 2;
    for i in 0..n {
        let j = start + i as i64;
        if j < 0 || j as usize >= audio.len() {
            continue;
        }
        let a = i.saturating_sub(half);
        let b = (i + half + 1).min(n);
        let den = (ce[b] - ce[a]) as f32;
        if den <= 1e-9 {
            continue;
        }
        let amp = (cz[b] - cz[a]) * (2.0 / den);
        audio[j as usize] -= (amp * copy[i]).re;
    }
}

/// The clean complex copy of a tone sequence at `rate` for subtraction: `exp(j phase)` times the envelope, tone 0 at `f0_hz`,
/// the first sample at `t_start` seconds after the audio's first sample (the copy starts at the nearest sample).
pub fn clean_copy(
    sub: &Sub,
    tones: &[u8],
    f0_hz: f64,
    mirrored: bool,
    rate: f64,
) -> Vec<Complex32> {
    let mut spec = sub.fsk();
    if mirrored {
        spec.spacing_hz = -spec.spacing_hz;
    }
    let mut out = Vec::new();
    crate::encode::waveform(&spec, tones, f0_hz, rate, &mut out);
    out
}

/// The latitude and longitude (east positive) of the middle of a locator's square (4 characters) or sub-square (6 characters),
/// from `sdrkit-util-geo`.
pub fn locator_centre(g: &str) -> Option<(f64, f64)> {
    let sq = sdrkit_util_geo::maidenhead::decode(g).ok()?;
    if !matches!(sq.chars, 4 | 6) {
        return None;
    }
    let c = sq.centre();
    Some((c.lat_deg, c.lon_deg))
}

/// The median of a slice (the upper one of an even count), by the shared quantile.
pub fn median(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    percentile(v, 0.5, &mut Vec::with_capacity(v.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_and_grids_of_the_spec() {
        let c = |t: &str| call_and_grid(t, false);
        assert_eq!(c("CQ K1ABC FN42"), ("K1ABC".into(), "FN42".into()));
        assert_eq!(c("CQ DX K1ABC FN42"), ("K1ABC".into(), "FN42".into()));
        assert_eq!(c("W9XYZ K1ABC R FN42"), ("K1ABC".into(), "FN42".into()));
        assert_eq!(c("W9XYZ K1ABC -12"), ("K1ABC".into(), String::new()));
        assert_eq!(
            c("K1ABC RR73; W9XYZ <KH1/KH7Z> -08"),
            ("KH1/KH7Z".into(), String::new())
        );
        assert_eq!(
            c("K1ABC RR73; W9XYZ <...> -08"),
            (String::new(), String::new())
        );
        assert_eq!(c("TNX BOB 73 GL"), ("BOB".into(), String::new()));
        assert_eq!(c("7123456789ABCDEF01"), (String::new(), String::new()));
        assert_eq!(c("CQ PJ4/K1ABC"), ("PJ4/K1ABC".into(), String::new()));
        assert_eq!(c("G4ABC DL1XYZ RR73"), ("DL1XYZ".into(), String::new()));
        let w = |t: &str| call_and_grid(t, true);
        assert_eq!(w("K1ABC FN42 37"), ("K1ABC".into(), "FN42".into()));
        assert_eq!(w("PJ4/K1ABC 30"), ("PJ4/K1ABC".into(), String::new()));
        assert_eq!(w("<K1ABC> FN42AX 37"), ("K1ABC".into(), "FN42AX".into()));
        assert_eq!(w("<...> FN42AX 37"), (String::new(), "FN42AX".into()));
    }

    #[test]
    fn a_baseband_holds_its_tone() {
        let audio: Vec<f32> = (0..16000)
            .map(|i| (0.5 * (2.0 * PI * 1003.0 * i as f64 / 8000.0).cos()) as f32)
            .collect();
        let s = SlotSpectrum::new(&audio, 16000);
        let mut bb = Vec::new();
        let shift = s.baseband(1000.0, 990.0, 1020.0, 5.0, 400, &mut bb);
        assert!((shift - 1000.0).abs() < 1e-9);
        let mid = bb[200];
        assert!((mid.norm() - 0.5).abs() < 0.02, "{}", mid.norm());
    }
}
