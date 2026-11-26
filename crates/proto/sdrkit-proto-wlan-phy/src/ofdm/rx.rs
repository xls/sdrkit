//! The 802.11a/g burst receiver at 20 Msps. It detects a frame by the lag-16 correlation of the short training field, takes
//! timing, carrier offset, channel and noise from the long training symbols, decodes SIGNAL, and equalises and demaps each DATA
//! symbol for [`DataDecoder`](super::DataDecoder). It outputs a frame only when the FCS is good.
//!
//! Standard: IEEE Std 802.11-2020 clause 17.

use super::{
    bin, data_carriers, demap_symbol, ltf_time, DataDecoder, Rate, Signal, GI, LTF, NFFT, N_SD,
    PILOTS, SAMPLE_RATE, SYMBOL_LEN,
};
use crate::{finite, Complex32, Result};
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_fec::conv::{codes, Ends, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::constellation::Constellation;
use sdrkit_algo_modem::ofdm::{ieee80211_pilot_polarity, LagCorrelator};
use sdrkit_util_bits::interleave::Permutation;
use std::f64::consts::TAU;

/// Samples into the cyclic prefix where each FFT window starts, so a late timing estimate stays inside the prefix.
const BACKOFF: u64 = 3;
/// The correlation level of the short training field that counts.
const DETECT: f32 = 0.6;
/// Samples above [`DETECT`] that start a frame.
const RUN: u32 = 32;
/// Samples before the end of a frame where the detector runs again. The correlator is full and a run can be complete before
/// the frame ends.
const DETECT_LEAD: u64 = 128;

/// A PSDU whose FCS is good. It borrows the receiver's buffer for the time of the callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OfdmFrame<'a> {
    /// The MPDU without its FCS.
    pub mpdu: &'a [u8],
    /// The SIGNAL field.
    pub signal: Signal,
    /// The scrambler seed the transmitter used.
    pub seed: u8,
    /// The input sample index where the first long training symbol starts.
    pub sample: u64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The mean signal-to-noise ratio of the subcarriers from the long training field, dB.
    pub snr_db: f64,
    /// The power of the signal in the long training field on its 52 subcarriers, dBFS (dB relative to full scale). Noise and
    /// other signals outside the subcarriers do not count, and the noise inside them is taken out.
    pub level_dbfs: f64,
}

/// A PPDU whose SIGNAL field says 6 Mbit/s, as the receiver holds it before it decodes the DATA field as non-HT: an HT-mixed
/// PPDU sends such a SIGNAL field (L-SIG) in front of its HT fields. It borrows the receiver's buffers for the time of the call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PpduView<'a> {
    /// The input samples from the start of the first long training symbol to the end of the PPDU that SIGNAL gives.
    pub samples: &'a [Complex32],
    /// The input sample index of `samples[0]`.
    pub sample: u64,
    /// The SIGNAL field.
    pub signal: Signal,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The channel of each FFT bin from the long training field (0 outside subcarriers -26 to 26), with the phase taken as 0 at
    /// `sample`.
    pub channel: &'a [Complex32; NFFT],
    /// The noise power of a bin from the long training field.
    pub noise: f32,
    /// The mean signal-to-noise ratio of the subcarriers from the long training field, dB.
    pub snr_db: f64,
    /// The power of the signal in the long training field on its 52 subcarriers, the noise taken out, dBFS.
    pub level_dbfs: f64,
}

/// The receiver's counters since it was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OfdmStats {
    /// Short training fields detected.
    pub preambles: u64,
    /// Detections with no long training field.
    pub no_sync: u64,
    /// SIGNAL fields that passed.
    pub signal_ok: u64,
    /// SIGNAL fields that failed.
    pub signal_bad: u64,
    /// PSDUs with a good FCS.
    pub frames: u64,
    /// PSDUs with a bad FCS.
    pub bad_fcs: u64,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Idle,
    Trigger {
        t: u64,
        coarse_hz: f64,
    },
    Signal {
        d: u64,
        f: f64,
    },
    Data {
        d: u64,
        f: f64,
        signal: Signal,
        end: u64,
    },
}

/// The 802.11a/g receiver (module documentation). The input is complex baseband at 20 Msps, centred on the channel.
pub struct OfdmReceiver {
    lag: LagCorrelator,
    y: Vec<Complex32>,
    base: u64,
    n: u64,
    run: u32,
    busy_until: u64,
    state: State,
    stats: OfdmStats,
    fft: Fft,
    /// Filters whose taps are the real and the imaginary parts of the long training symbol, the last sample first.
    ltf_re: Fir<Complex32>,
    ltf_im: Fir<Complex32>,
    by_re: Vec<Complex32>,
    by_im: Vec<Complex32>,
    h: [Complex32; NFFT],
    /// `1 / h` per bin, 0 where the channel is 0.
    h_inv: [Complex32; NFFT],
    /// `|h|^2 / noise` of each data carrier.
    weights: [f32; N_SD],
    noise: f32,
    snr_db: f64,
    /// The power of the signal in the two long training symbols on the 52 used subcarriers, the noise taken out.
    ltf_power: f64,
    pol: [i8; 127],
    dc: [i32; N_SD],
    sig_perm: Permutation,
    sig_viterbi: Viterbi,
    bpsk: Constellation,
    consts: Vec<(Constellation, Permutation)>,
    data: DataDecoder,
    seg: Vec<Complex32>,
    corr: Vec<f32>,
    scratch: Vec<f32>,
    llr: Vec<f32>,
    llr8: [i8; 48],
    sig_bits: [u8; 18],
}

impl Default for OfdmReceiver {
    fn default() -> Self {
        OfdmReceiver::new()
    }
}

impl OfdmReceiver {
    /// A receiver.
    pub fn new() -> OfdmReceiver {
        let consts = [Rate::R6, Rate::R12, Rate::R24, Rate::R48]
            .iter()
            .map(|r| {
                let c = Constellation::new(r.modulation().points())
                    .unwrap_or_else(|_| unreachable!("the 802.11 constellations are valid"));
                (c, r.interleaver())
            })
            .collect();
        let ltf = ltf_time();
        let part = |f: fn(&Complex32) -> f32| -> Vec<f32> { ltf.iter().rev().map(f).collect() };
        OfdmReceiver {
            ltf_re: Fir::new(&part(|v| v.re)),
            ltf_im: Fir::new(&part(|v| v.im)),
            by_re: Vec::with_capacity(1024),
            by_im: Vec::with_capacity(1024),
            lag: LagCorrelator::new(16, 48, DETECT, DETECT)
                .unwrap_or_else(|_| unreachable!("valid correlator settings")),
            y: Vec::with_capacity(1 << 17),
            base: 0,
            n: 0,
            run: 0,
            busy_until: 0,
            state: State::Idle,
            stats: OfdmStats::default(),
            fft: Fft::forward(NFFT),
            h: [Complex32::default(); NFFT],
            h_inv: [Complex32::default(); NFFT],
            weights: [0.0; N_SD],
            noise: 1.0,
            snr_db: 0.0,
            ltf_power: 0.0,
            pol: ieee80211_pilot_polarity(),
            dc: data_carriers(),
            sig_perm: Rate::R6.interleaver(),
            sig_viterbi: Viterbi::new(codes::WIFI_K7, 24)
                .unwrap_or_else(|_| unreachable!("WIFI_K7 is a valid code")),
            bpsk: Constellation::new(Rate::R6.modulation().points())
                .unwrap_or_else(|_| unreachable!("BPSK is valid")),
            consts,
            data: DataDecoder::new(),
            seg: Vec::with_capacity(1024),
            corr: Vec::with_capacity(1024),
            scratch: vec![0.0; 6 * N_SD],
            llr: Vec::with_capacity(1366 * 288),
            llr8: [0; 48],
            sig_bits: [0; 18],
        }
    }

    /// The counters.
    pub fn stats(&self) -> OfdmStats {
        self.stats
    }

    /// Forgets the signal and any frame in progress. The counters stay.
    pub fn reset(&mut self) {
        self.lag.reset();
        self.run = 0;
        self.state = State::Idle;
        self.busy_until = self.n;
    }

    #[inline]
    fn at(&self, i: u64) -> Complex32 {
        self.y[(i - self.base) as usize]
    }

    /// Runs 20 Msps samples. The receiver calls `on_frame` for each PSDU with a good FCS.
    pub fn process(&mut self, x: &[Complex32], on_frame: impl FnMut(&OfdmFrame<'_>)) {
        self.process_with(x, on_frame, |_| false);
    }

    /// As [`process`](Self::process), and before it decodes the DATA field of a PPDU whose SIGNAL says 6 Mbit/s it calls
    /// `on_ppdu`. When `on_ppdu` returns true the PPDU is taken: it is not decoded as non-HT and its FCS is not counted.
    pub fn process_with(
        &mut self,
        x: &[Complex32],
        mut on_frame: impl FnMut(&OfdmFrame<'_>),
        mut on_ppdu: impl FnMut(&PpduView<'_>) -> bool,
    ) {
        self.y.extend(x.iter().map(|&s| finite(s)));
        let end = self.n + x.len() as u64;
        while self.n < end {
            if self.skip_data(end) {
                continue;
            }
            // The counters stay in registers while no frame is in progress.
            let (mut next, mut run) = (self.n, self.run);
            while next < end {
                let n = next;
                next += 1;
                let (m, p) = self.lag.push(self.y[(n - self.base) as usize]);
                if m > DETECT {
                    run += 1;
                    if run == RUN && matches!(self.state, State::Idle) && n >= self.busy_until {
                        self.stats.preambles += 1;
                        self.state = State::Trigger {
                            t: n,
                            coarse_hz: self.lag.freq_of(p) * SAMPLE_RATE,
                        };
                    }
                } else {
                    run = 0;
                }
                if !matches!(self.state, State::Idle) {
                    (self.n, self.run) = (next, run);
                    self.step(&mut on_frame, &mut on_ppdu);
                    if matches!(self.state, State::Data { .. }) {
                        break;
                    }
                }
            }
            (self.n, self.run) = (next, run);
        }
        self.trim();
    }

    /// Passes over the samples before `end` that lie inside the DATA field of the frame in progress, where nothing is
    /// detected; true when it passed over some. The detector starts empty [`DETECT_LEAD`] samples before the end of the frame.
    fn skip_data(&mut self, end: u64) -> bool {
        let State::Data { end: last, .. } = self.state else {
            return false;
        };
        let resume = last.saturating_sub(DETECT_LEAD);
        if self.n >= resume {
            return false;
        }
        self.n = resume.min(end);
        if self.n == resume {
            self.lag.reset();
            self.run = 0;
        }
        true
    }

    fn trim(&mut self) {
        let keep = match self.state {
            State::Idle => self.n,
            State::Trigger { t, .. } => t,
            State::Signal { d, .. } | State::Data { d, .. } => d,
        }
        .saturating_sub(400)
        .min(self.n.saturating_sub(1200));
        if keep > self.base + (1 << 16) {
            let d = (keep - self.base) as usize;
            self.y.drain(..d);
            self.base = keep;
        }
    }

    /// The FFT of the 64 samples from `start`, with the carrier offset `f` (Hz) removed, the phase taken as 0 at `d`.
    fn fft_at(&mut self, start: u64, d: u64, f: f64) -> [Complex32; NFFT] {
        let w = -TAU * f / SAMPLE_RATE;
        let (s0, c0) = approx::sin_cos(w * (start as f64 - d as f64));
        let (s1, c1) = approx::sin_cos(w);
        let step = Complex32::new(c1, s1);
        let first = Complex32::new(c0, s0);
        // Four phasors that each turn by four steps: a short chain of products.
        let step2 = step * step;
        let step4 = step2 * step2;
        let mut rot = [first, first * step, first * step2, first * step2 * step];
        let at = (start - self.base) as usize;
        let mut x = [Complex32::default(); NFFT];
        let (quads, _) = x.as_chunks_mut::<4>();
        let (input, _) = self.y[at..at + NFFT].as_chunks::<4>();
        for (v, y) in quads.iter_mut().zip(input) {
            for ((v, &y), r) in v.iter_mut().zip(y).zip(&mut rot) {
                *v = y * *r;
                *r *= step4;
            }
        }
        self.fft.process(&mut x);
        x
    }

    fn step(
        &mut self,
        on_frame: &mut impl FnMut(&OfdmFrame<'_>),
        on_ppdu: &mut impl FnMut(&PpduView<'_>) -> bool,
    ) {
        match self.state {
            State::Idle => {}
            State::Trigger { t, coarse_hz } => {
                if self.n < t + 400 {
                    return;
                }
                self.state = State::Idle;
                if self.sync(t, coarse_hz).is_none() {
                    self.stats.no_sync += 1;
                }
            }
            State::Signal { d, f } => {
                if self.n < d + 128 + SYMBOL_LEN as u64 + 2 {
                    return;
                }
                self.state = State::Idle;
                match self.signal(d, f) {
                    Ok(signal) => {
                        self.stats.signal_ok += 1;
                        let n_sym = signal.rate.n_sym(signal.length as usize) as u64;
                        let end = d + 128 + SYMBOL_LEN as u64 * (1 + n_sym);
                        self.busy_until = end;
                        self.state = State::Data { d, f, signal, end };
                    }
                    Err(_) => {
                        self.stats.signal_bad += 1;
                        self.busy_until = d + 128 + SYMBOL_LEN as u64;
                    }
                }
            }
            State::Data { d, f, signal, end } => {
                if self.n < end + 2 {
                    return;
                }
                self.state = State::Idle;
                if signal.rate == Rate::R6 && self.offer(d, f, signal, end, on_ppdu) {
                    return;
                }
                self.data(d, f, signal, on_frame);
            }
        }
    }

    /// Finds the long training field after a detection at `t`. If it finds it, it sets the channel and moves to the SIGNAL state.
    fn sync(&mut self, t: u64, coarse_hz: f64) -> Option<()> {
        let from = t.saturating_sub(100).max(self.base);
        let positions = (t + 260).saturating_sub(from) as usize;
        let w = -TAU * coarse_hz / SAMPLE_RATE;
        let (s1, c1) = approx::sin_cos(w);
        let step = Complex32::new(c1, s1);
        let mut rot = Complex32::new(1.0, 0.0);
        self.seg.clear();
        for i in 0..positions + 2 * NFFT {
            let v = self.at(from + i as u64) * rot;
            self.seg.push(v);
            rot *= step;
            if i % 64 == 63 {
                rot /= rot.norm().max(1e-12);
            }
        }
        // The correlation with the long training symbol: its real and imaginary parts are two filters with real taps.
        let n = self.seg.len();
        self.by_re.resize(n, Complex32::default());
        self.by_im.resize(n, Complex32::default());
        self.ltf_re.reset();
        self.ltf_im.reset();
        self.ltf_re.process(&self.seg, &mut self.by_re);
        self.ltf_im.process(&self.seg, &mut self.by_im);
        self.corr.clear();
        // Output `p + NFFT - 1` of a filter is the correlation of the samples from `p` on.
        let whole = NFFT - 1..NFFT - 1 + positions + NFFT;
        self.corr.extend(
            self.by_re[whole.clone()]
                .iter()
                .zip(&self.by_im[whole])
                .map(|(a, b)| Complex32::new(a.re + b.im, a.im - b.re).norm()),
        );
        let mean = self.corr.iter().sum::<f32>() / self.corr.len().max(1) as f32;
        let mut best = (0f32, 0usize);
        for p in 0..positions {
            let v = self.corr[p] + self.corr[p + NFFT];
            if v > best.0 {
                best = (v, p);
            }
        }
        if best.0 <= 6.0 * mean {
            return None;
        }
        let p = best.1;
        let mut acc = Complex32::default();
        for i in 0..NFFT {
            acc += self.seg[p + i] * self.seg[p + NFFT + i].conj();
        }
        let f =
            coarse_hz - (acc.im as f64).atan2(acc.re as f64) * SAMPLE_RATE / (TAU * NFFT as f64);
        let d = from + p as u64;
        let a = self.fft_at(d - BACKOFF, d, f);
        let b = self.fft_at(d + NFFT as u64 - BACKOFF, d, f);
        let (mut ph, mut pn, mut pl) = (0f32, 0f32, 0f64);
        for (i, &l) in LTF.iter().enumerate() {
            let k = bin(i as i32 - 26);
            if l == 0 {
                self.h[k] = Complex32::default();
                continue;
            }
            self.h[k] = (a[k] + b[k]) * 0.5 * l as f32;
            ph += self.h[k].norm_sqr();
            pn += (a[k] - b[k]).norm_sqr() * 0.5;
            pl += f64::from(a[k].norm_sqr() + b[k].norm_sqr()) * 0.5;
        }
        let ph = ph / 52.0;
        if ph < 1e-20 {
            return None;
        }
        self.noise = (pn / 52.0).max(ph * 1e-5);
        // Parseval for the unscaled transform: the mean power of a symbol is the sum of |X_k|^2 over NFFT^2. Each of the 52
        // bins holds the noise `self.noise` (from the difference of the two symbols): what is left is the signal.
        self.ltf_power =
            (pl - 52.0 * f64::from(self.noise)).max(pl * 1e-3) / (NFFT * NFFT) as f64;
        for (v, h) in self.h_inv.iter_mut().zip(&self.h) {
            *v = if h.norm_sqr() > 1e-20 {
                h.inv()
            } else {
                Complex32::default()
            };
        }
        for (w, &k) in self.weights.iter_mut().zip(&self.dc) {
            *w = self.h[bin(k)].norm_sqr() / self.noise;
        }
        self.snr_db = 10.0 * (ph / self.noise).log10() as f64;
        self.state = State::Signal { d, f };
        Some(())
    }

    /// Equalises one symbol and corrects its phase from the pilots: the data cells.
    fn cells(&mut self, start: u64, d: u64, f: f64, polarity: i8) -> [Complex32; N_SD] {
        let y = self.fft_at(start, d, f);
        let pilots = PILOTS.map(|(k, v)| y[bin(k)] * self.h_inv[bin(k)] * (v * polarity as f32));
        let acc: Complex32 = pilots.iter().sum();
        let phi0 = approx::atan2(acc.im, acc.re);
        let (s0, c0) = approx::sin_cos(-phi0 as f64);
        let r0 = Complex32::new(c0, s0);
        let (mut num, mut den) = (0f32, 0f32);
        for (&(k, _), &p) in PILOTS.iter().zip(&pilots) {
            let e = p * r0;
            num += k as f32 * approx::atan2(e.im, e.re);
            den += (k * k) as f32;
        }
        let slope = num / den;
        // The correction of carrier k is r0 * step^k. Each power of step is a product of two lower powers.
        let (s1, c1) = approx::sin_cos(-slope as f64);
        let step = Complex32::new(c1, s1);
        let mut power = [Complex32::new(1.0, 0.0); 27];
        power[1] = step;
        for k in 2..=26 {
            power[k] = power[k / 2] * power[k - k / 2];
        }
        let mut rot = [r0; 53];
        for k in 1..=26 {
            rot[26 + k] = r0 * power[k];
            rot[26 - k] = r0 * power[k].conj();
        }
        let mut cells = [Complex32::default(); N_SD];
        for (c, &k) in cells.iter_mut().zip(&self.dc) {
            *c = y[bin(k)] * self.h_inv[bin(k)] * rot[(k + 26) as usize];
        }
        cells
    }

    fn signal(&mut self, d: u64, f: f64) -> Result<Signal> {
        let cells = self.cells(d + 128 + GI as u64 - BACKOFF, d, f, self.pol[0]);
        self.llr.clear();
        demap_symbol(
            &self.bpsk,
            &self.sig_perm,
            &cells,
            &self.weights,
            &mut self.scratch,
            &mut self.llr,
        )?;
        Quantizer::default().quantize_auto(&self.llr, &mut self.llr8)?;
        self.sig_viterbi
            .decode_only(&self.llr8, Ends::Zero, &mut self.sig_bits)?;
        Signal::from_bits(&self.sig_bits)
    }

    /// Offers the PPDU at `d` that ends at `end` to `on_ppdu`; true when it took it.
    fn offer(
        &self,
        d: u64,
        f: f64,
        signal: Signal,
        end: u64,
        on_ppdu: &mut impl FnMut(&PpduView<'_>) -> bool,
    ) -> bool {
        let view = PpduView {
            samples: &self.y[(d - self.base) as usize..(end - self.base) as usize],
            sample: d,
            signal,
            offset_hz: f,
            channel: &self.h,
            noise: self.noise,
            snr_db: self.snr_db,
            level_dbfs: sdrkit_algo_dsp::meter::power_to_db(self.ltf_power),
        };
        on_ppdu(&view)
    }

    fn data(&mut self, d: u64, f: f64, signal: Signal, on_frame: &mut impl FnMut(&OfdmFrame<'_>)) {
        let rate = signal.rate;
        let n_sym = rate.n_sym(signal.length as usize);
        let ci = match rate.n_bpsc() {
            1 => 0,
            2 => 1,
            4 => 2,
            _ => 3,
        };
        self.llr.clear();
        for i in 0..n_sym {
            let start = d + 128 + SYMBOL_LEN as u64 * (1 + i as u64) + GI as u64 - BACKOFF;
            let cells = self.cells(start, d, f, self.pol[(i + 1) % 127]);
            let (c, perm) = &self.consts[ci];
            if demap_symbol(
                c,
                perm,
                &cells,
                &self.weights,
                &mut self.scratch,
                &mut self.llr,
            )
            .is_err()
            {
                self.stats.bad_fcs += 1;
                return;
            }
        }
        let snr_db = self.snr_db;
        let level = self.ltf_power;
        let stats = &mut self.stats;
        match self.data.decode(&self.llr, rate, signal.length as usize) {
            Ok((psdu, seed)) if crate::mac::fcs_ok(psdu) => {
                stats.frames += 1;
                on_frame(&OfdmFrame {
                    mpdu: &psdu[..psdu.len() - 4],
                    signal,
                    seed,
                    sample: d,
                    offset_hz: f,
                    snr_db,
                    level_dbfs: sdrkit_algo_dsp::meter::power_to_db(level),
                });
            }
            _ => stats.bad_fcs += 1,
        }
    }
}

impl std::fmt::Debug for OfdmReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OfdmReceiver")
            .field("samples", &self.n)
            .field("stats", &self.stats)
            .finish()
    }
}
