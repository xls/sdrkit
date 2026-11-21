//! Filter banks: many channels out of one wide input for little more than the cost of one.
//!
//! - [`PolyphaseBank`]: an oversampled polyphase FFT analysis bank. `N` bands spaced `B = rate / N` apart cover the whole input; each
//!   is output at `os B` (2 B by default), so a channel anywhere in the input is well inside some band.
//! - [`OverlapSaveBank`]: fast convolution. Each channel has its own centre, filter and output rate (LoRa and LoRaWAN plans,
//!   scanners).
//!
//! Both are exact linear filters (no time aliasing): [`PolyphaseBank`] band `k` at output `m` is the input mixed down by `k B`,
//! filtered by the prototype and sampled at input index `(m + 1) M - 1` (`M = N / os`); [`OverlapSaveBank`] channel output `j` is
//! the input mixed down by the channel's offset, filtered by its lowpass (delay [`OverlapSaveBank::delay`]) and sampled at input
//! index `j D`. Phases are continuous with the input's absolute time.

use crate::fft::Fft;
use crate::fir::{design_beta, kaiser_len, lowpass, windowed_sinc, MAX_TAPS};
use crate::nco::Nco;
use crate::{Complex32, DspError, Param};
use std::f64::consts::TAU;

/// A polyphase FFT analysis filter bank (module docs).
///
/// ```
/// use sdrkit_algo_dsp::bank::PolyphaseBank;
/// use sdrkit_algo_dsp::Complex32;
/// // 16 bands, 8 taps per band, 60 dB, reading bands 0 and 3.
/// let mut b = PolyphaseBank::new(16, 8, 2, 60.0, &[0, 3]).unwrap();
/// let mut out = vec![Vec::new(), Vec::new()];
/// b.process(&vec![Complex32::new(1.0, 0.0); 1600], &mut out).unwrap();
/// assert_eq!(out[0].len(), 200); // one output per N / 2 inputs
/// ```
#[derive(Debug)]
pub struct PolyphaseBank {
    n: usize,
    hop: usize,
    /// Prototype reversed (oldest sample first), each tap twice (re, im lanes).
    wide: Vec<f32>,
    /// Input as interleaved re, im: the last `L` samples, then what has come since.
    hist: Vec<f32>,
    phase: usize,
    /// Absolute index of the next input sample.
    t: u64,
    fft: Fft,
    acc: Vec<f32>,
    work: Vec<Complex32>,
    /// `e^(-j 2 pi q / N)`.
    twiddle: Vec<Complex32>,
    bands: Vec<usize>,
    taps: usize,
    atten_db: f64,
}

impl PolyphaseBank {
    /// `bands` bands (a power of two, 2 to 65536), `taps_per_band` prototype taps per band (2 to 64), oversampling `os` (1, 2 or 4;
    /// the output rate is `os` times the spacing), a prototype designed for `atten_db` (Kaiser), and the bands to read (indices
    /// modulo `bands`; band `k` is centred on `k rate / bands`, so the upper half are the negative frequencies).
    pub fn new(
        bands: usize,
        taps_per_band: usize,
        os: usize,
        atten_db: f64,
        read: &[usize],
    ) -> Result<Self, DspError> {
        if !(bands.is_power_of_two() && (2..=65_536).contains(&bands)) {
            return Param::Bands.not_allowed(bands as f64);
        }
        if !(2..=64).contains(&taps_per_band) {
            return Param::TapsPerBand.range(2.0, 64.0, taps_per_band as f64);
        }
        if ![1, 2, 4].contains(&os) {
            return Param::Oversampling.not_allowed(os as f64);
        }
        if os > bands {
            return Param::Oversampling.conflict(Param::Bands);
        }
        if !(atten_db > 0.0 && atten_db < 200.0) {
            return Param::AttenuationDb.range(0.0, 200.0, atten_db);
        }
        if let Some(&k) = read.iter().find(|&&k| k >= bands) {
            return Param::Band.range(0.0, (bands - 1) as f64, k as f64);
        }
        let l = taps_per_band * bands;
        // -6 dB at the band spacing.
        let proto = windowed_sinc(l, 1.0 / bands as f64, design_beta(atten_db))?;
        let wide: Vec<f32> = proto
            .iter()
            .rev()
            .flat_map(|&v| [v as f32, v as f32])
            .collect();
        let mut hist = Vec::with_capacity(2 * (2 * l + bands));
        hist.resize(2 * l, 0.0);
        Ok(PolyphaseBank {
            n: bands,
            hop: bands / os,
            wide,
            hist,
            phase: 0,
            t: 0,
            fft: Fft::inverse(bands),
            acc: vec![0.0; 2 * bands],
            work: vec![Complex32::default(); bands],
            twiddle: (0..bands)
                .map(|q| {
                    let (s, c) = (-TAU * q as f64 / bands as f64).sin_cos();
                    Complex32::new(c as f32, s as f32)
                })
                .collect(),
            bands: read.to_vec(),
            taps: l,
            atten_db,
        })
    }

    /// The number of bands `N`.
    pub fn bands(&self) -> usize {
        self.n
    }

    /// Inputs per output (`N / os`).
    pub fn hop(&self) -> usize {
        self.hop
    }

    /// The prototype length.
    pub fn num_taps(&self) -> usize {
        self.taps
    }

    /// The prototype's delay in input samples.
    pub fn delay(&self) -> f64 {
        (self.taps - 1) as f64 / 2.0
    }

    /// How far from a band's centre its response is flat (within the design ripple), in units of the band spacing: `1 - w / 2`, `w`
    /// being the Kaiser transition width for the taps and attenuation (in spacings). The stop band starts at `1 + w / 2`, and what
    /// lies there folds onto `os - 1 - w / 2` and beyond at the output, so a 2x bank is flat and alias-free within `+- flat_edge()`
    /// of each centre.
    pub fn flat_edge(&self) -> f64 {
        let w = (self.atten_db + crate::fir::DESIGN_MARGIN_DB - 7.95)
            / (14.36 * (self.taps - 1) as f64)
            * self.n as f64;
        1.0 - w / 2.0
    }

    /// The band whose centre is nearest `f` cycles per sample, and what is left of `f` inside it (cycles per input sample).
    pub fn band_of(&self, f: f64) -> (usize, f64) {
        let k = (f * self.n as f64).round();
        (
            (k as i64).rem_euclid(self.n as i64) as usize,
            f - k / self.n as f64,
        )
    }

    /// Forgets the input (the time count starts again from 0).
    pub fn reset(&mut self) {
        let l = self.taps;
        self.hist.clear();
        self.hist.resize(2 * l, 0.0);
        self.phase = 0;
        self.t = 0;
    }

    /// Feeds input; appends each read band's new samples to `out[i]` (in the order the bands were given). Errors when `out` does not
    /// have one `Vec` per band read. Allocates nothing once the `Vec`s have grown.
    pub fn process(
        &mut self,
        input: &[Complex32],
        out: &mut [Vec<Complex32>],
    ) -> Result<(), DspError> {
        if out.len() != self.bands.len() {
            return Param::Output.length(self.bands.len(), out.len());
        }
        let l2 = 2 * self.taps;
        let mut rest = input;
        while !rest.is_empty() {
            let take = (self.hop - self.phase).min(rest.len());
            if self.hist.len() + 2 * take > self.hist.capacity() {
                let drop = self.hist.len() - l2;
                self.hist.copy_within(drop.., 0);
                self.hist.truncate(l2);
            }
            for v in &rest[..take] {
                self.hist.push(v.re);
                self.hist.push(v.im);
            }
            rest = &rest[take..];
            self.phase += take;
            self.t += take as u64;
            if self.phase < self.hop {
                break;
            }
            self.phase = 0;
            let win = &self.hist[self.hist.len() - l2..];
            let n2 = 2 * self.n;
            self.acc.fill(0.0);
            for (w, h) in win.chunks_exact(n2).zip(self.wide.chunks_exact(n2)) {
                for ((a, &x), &c) in self.acc.iter_mut().zip(w).zip(h) {
                    *a += x * c;
                }
            }
            // acc[j] holds fold position N - 1 - j (module docs): reverse into work.
            for (r, w) in self.work.iter_mut().enumerate() {
                let j = self.n - 1 - r;
                *w = Complex32::new(self.acc[2 * j], self.acc[2 * j + 1]);
            }
            self.fft.process(&mut self.work);
            // Output time: the newest sample, t - 1. Rotate band k by e^(-j 2 pi k (t - 1) / N).
            let tm = ((self.t - 1) % self.n as u64) as usize;
            for (o, &k) in out.iter_mut().zip(&self.bands) {
                o.push(self.work[k] * self.twiddle[(k * tm) % self.n]);
            }
        }
        Ok(())
    }
}

struct Channel {
    bin: i64,
    /// The offset of the centre from the centre of `bin`, Hz: mixed out after the filter.
    resid: f64,
    d: usize,
    /// The design of `taps`: pass edge, stop edge, attenuation.
    design: [f64; 3],
    taps: Vec<f32>,
    /// The filter's spectrum on the `m` bins kept, scaled by `1 / L`, in IFFT order.
    h: Vec<Complex32>,
    ifft: Fft,
    work: Vec<Complex32>,
    nco: Nco,
    out_rate: f64,
}

impl Channel {
    /// Appends to `out` the output of one block, before the mix of the residual: `spec` is the spectrum of the window whose
    /// first sample is at absolute input time `s0`. Returns where the new output starts in `out`.
    fn filter_block(
        &mut self,
        spec: &[Complex32],
        s0: i128,
        l: usize,
        s: usize,
        out: &mut Vec<Complex32>,
    ) -> usize {
        let m = self.work.len();
        for (q, (w, h)) in self.work.iter_mut().zip(&self.h).enumerate() {
            let qq = if q < m / 2 {
                q as i64
            } else {
                q as i64 - m as i64
            };
            let src = (self.bin + qq).rem_euclid(l as i64) as usize;
            *w = spec[src] * h;
        }
        self.ifft.process(&mut self.work);
        let idx = (self.bin as i128 * s0).rem_euclid(l as i128) as f64;
        let (sn, cs) = (-TAU * idx / l as f64).sin_cos();
        let z = Complex32::new(cs as f32, sn as f32);
        let start = out.len();
        out.extend(self.work[(l - s) / self.d..].iter().map(|v| v * z));
        start
    }
}

/// A fast-convolution (overlap-save) bank of independent channels (module docs).
///
/// ```
/// use sdrkit_algo_dsp::bank::OverlapSaveBank;
/// use sdrkit_algo_dsp::Complex32;
/// let mut b = OverlapSaveBank::new(1e6, 4096).unwrap();
/// let ch = b.add_channel(200e3, 50e3, 62.5e3, 60.0).unwrap();
/// assert_eq!(b.output_rate(ch), 125e3);
/// let mut out = vec![Vec::new()];
/// b.process(&vec![Complex32::new(1.0, 0.0); 8192], &mut out).unwrap();
/// // One hop of 2048 inputs per FFT (the first window starts with 2048 samples of zero history): 4 hops of 2048 / 8 outputs.
/// assert_eq!(out[0].len(), 1024);
/// ```
pub struct OverlapSaveBank {
    rate: f64,
    l: usize,
    s: usize,
    fft: Fft,
    /// Input not yet consumed, starting with the `L - S` samples of history.
    hist: Vec<Complex32>,
    spec: Vec<Complex32>,
    block: u64,
    chans: Vec<Channel>,
    /// The spectra of the newest blocks, a ring of `past_cap` blocks of `L` bins ([`OverlapSaveBank::replay`]).
    past: Vec<Complex32>,
    past_cap: usize,
    past_len: usize,
    past_next: usize,
    /// `L` points of scratch for the spectrum of the filter of a new channel.
    full: Vec<Complex32>,
}

impl std::fmt::Debug for OverlapSaveBank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlapSaveBank")
            .field("rate", &self.rate)
            .field("fft_size", &self.l)
            .field("channels", &self.chans.len())
            .finish()
    }
}

impl OverlapSaveBank {
    /// A bank for input at `rate` Hz with FFTs of `fft_size` points (a power of two, 64 to 2^20), hopping by half of it, so each
    /// channel filter may have up to `fft_size / 2 + 1` taps.
    pub fn new(rate: f64, fft_size: usize) -> Result<Self, DspError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        if !(fft_size.is_power_of_two() && (64..=1 << 20).contains(&fft_size)) {
            return Param::FftSize.not_allowed(fft_size as f64);
        }
        let s = fft_size / 2;
        let mut hist = Vec::with_capacity(2 * fft_size);
        hist.resize(fft_size - s, Complex32::default());
        Ok(OverlapSaveBank {
            rate,
            l: fft_size,
            s,
            fft: Fft::forward(fft_size),
            hist,
            spec: vec![Complex32::default(); fft_size],
            block: 0,
            chans: Vec::new(),
            past: Vec::new(),
            past_cap: 0,
            past_len: 0,
            past_next: 0,
            full: Vec::new(),
        })
    }

    /// The FFT size an input at `rate` needs for channel filters with a transition band of `transition_hz` at `atten_db`. An
    /// attenuation that is not finite returns the size for the longest filter.
    pub fn size_for(rate: f64, transition_hz: f64, atten_db: f64) -> usize {
        let taps =
            kaiser_len(atten_db, (transition_hz / rate).clamp(1e-7, 0.49)).unwrap_or(MAX_TAPS);
        (2 * (taps - 1)).next_power_of_two().max(64)
    }

    /// The input samples of one block (half the FFT size): the output of every channel comes in whole blocks.
    pub fn hop(&self) -> usize {
        self.s
    }

    /// Adds a channel centred on `offset_hz` (relative to the input's centre), flat to `pass_hz` either side and attenuated by
    /// `atten_db` from `stop_hz` (a Kaiser lowpass). Its output rate is `rate / D` for the largest power of two `D` that keeps
    /// `stop_hz` below the output's Nyquist. Returns the channel's index. Errors for bad edges, an offset outside the input, or a
    /// filter longer than the FFT allows (raise the FFT size: [`size_for`](Self::size_for)). A channel with the design of one
    /// already in the bank takes its taps without a new design.
    pub fn add_channel(
        &mut self,
        offset_hz: f64,
        pass_hz: f64,
        stop_hz: f64,
        atten_db: f64,
    ) -> Result<usize, DspError> {
        let rate = self.rate;
        if !(pass_hz >= 0.0 && pass_hz.is_finite()) {
            return Param::Pass.at_least(0.0, pass_hz);
        }
        if stop_hz > rate / 2.0 || stop_hz.is_nan() {
            return Param::Stop.range(0.0, rate / 2.0, stop_hz);
        }
        if pass_hz >= stop_hz {
            return Param::Pass.conflict(Param::Stop);
        }
        if offset_hz.abs() > rate / 2.0 || offset_hz.is_nan() {
            return Param::OffsetHz.range(-rate / 2.0, rate / 2.0, offset_hz);
        }
        if !(atten_db > 0.0 && atten_db < 200.0) {
            return Param::AttenuationDb.range(0.0, 200.0, atten_db);
        }
        let design = [pass_hz, stop_hz, atten_db];
        let taps = match self.chans.iter().find(|c| c.design == design) {
            Some(c) => c.taps.clone(),
            None => lowpass(rate, pass_hz, stop_hz, atten_db)?,
        };
        if taps.len() > self.l - self.s + 1 {
            return Param::Taps.range(1.0, (self.l - self.s + 1) as f64, taps.len() as f64);
        }
        let mut d = 1usize;
        while 2 * d <= self.s && 2 * d <= self.l / 4 && rate / (2.0 * 2.0 * d as f64) >= stop_hz {
            d *= 2;
        }
        let m = self.l / d;
        let bin_hz = rate / self.l as f64;
        let bin = (offset_hz / bin_hz).round() as i64;
        let resid = offset_hz - bin as f64 * bin_hz;
        // The filter's spectrum by one FFT of the zero-padded taps, kept on the m bins around 0.
        // The lowpass moved by the residual, so the filter is centred on the exact offset: h[i] e^(j 2 pi resid i / rate).
        self.full.clear();
        self.full.resize(self.l, Complex32::default());
        for (i, (f, &t)) in self.full.iter_mut().zip(&taps).enumerate() {
            let (s, c) = (TAU * resid * i as f64 / rate).sin_cos();
            *f = Complex32::new((t as f64 * c) as f32, (t as f64 * s) as f32);
        }
        self.fft.process(&mut self.full);
        let scale = 1.0 / self.l as f32;
        let full = &self.full;
        let h = (0..m)
            .map(|q| {
                let src = if q < m / 2 { q } else { self.l - (m - q) };
                full[src] * scale
            })
            .collect();
        let out_rate = rate / d as f64;
        let mut nco = Nco::new(out_rate, -resid);
        // The next output is at input time block * S: keep the residual's phase on absolute time.
        let t0 = self.block as f64 * self.s as f64;
        nco.set_phase_cycles((-resid * t0 / rate).rem_euclid(1.0));
        self.chans.push(Channel {
            bin,
            resid,
            d,
            design,
            taps,
            h,
            ifft: Fft::inverse(m),
            work: vec![Complex32::default(); m],
            nco,
            out_rate,
        });
        Ok(self.chans.len() - 1)
    }

    /// Removes channel `ch`. The channels after it move down by one index. Errors for an unknown channel.
    pub fn remove_channel(&mut self, ch: usize) -> Result<(), DspError> {
        if ch >= self.chans.len() {
            return Param::Channel.range(0.0, self.chans.len() as f64 - 1.0, ch as f64);
        }
        self.chans.remove(ch);
        Ok(())
    }

    /// Keeps the spectra of the newest `blocks` blocks (`blocks` times the FFT size in complex values) for
    /// [`replay`](Self::replay). 0 keeps none. What was kept before is forgotten.
    pub fn keep_history(&mut self, blocks: usize) {
        self.past = vec![Complex32::default(); blocks * self.l];
        self.past_cap = blocks;
        self.past_len = 0;
        self.past_next = 0;
    }

    /// The blocks kept now for [`replay`](Self::replay).
    pub fn history_blocks(&self) -> usize {
        self.past_len
    }

    /// The absolute input index of the next output of every channel (before the delay of its filter): output `j` of a block is
    /// at this index plus `j D`.
    pub fn next_output_index(&self) -> u64 {
        self.block * self.s as u64
    }

    /// Appends to `out` the output that channel `ch` would have given over the kept blocks, oldest first. The next output of
    /// [`process`](Self::process) for the channel follows it without a gap. Errors for an unknown channel. Allocates nothing
    /// once `out` has grown.
    pub fn replay(&mut self, ch: usize, out: &mut Vec<Complex32>) -> Result<(), DspError> {
        let (l, s, rate) = (self.l, self.s, self.rate);
        let n = self.chans.len();
        let Some(c) = self.chans.get_mut(ch) else {
            return Param::Channel.range(0.0, n as f64 - 1.0, ch as f64);
        };
        for k in 0..self.past_len {
            let slot = (self.past_next + self.past_cap - self.past_len + k) % self.past_cap;
            let block = self.block - (self.past_len - k) as u64;
            let s0 = block as i128 * s as i128 - (l - s) as i128;
            let start = c.filter_block(&self.past[slot * l..(slot + 1) * l], s0, l, s, out);
            let mut nco = Nco::new(c.out_rate, -c.resid);
            nco.set_phase_cycles((-c.resid * (block * s as u64) as f64 / rate).rem_euclid(1.0));
            nco.mix_in_place(&mut out[start..]);
        }
        Ok(())
    }

    /// The number of channels.
    pub fn len(&self) -> usize {
        self.chans.len()
    }

    /// No channels yet.
    pub fn is_empty(&self) -> bool {
        self.chans.is_empty()
    }

    /// The output rate of channel `ch` (0 for an unknown channel).
    pub fn output_rate(&self, ch: usize) -> f64 {
        self.chans.get(ch).map_or(0.0, |c| c.out_rate)
    }

    /// The decimation of channel `ch` (0 for an unknown channel).
    pub fn decimation(&self, ch: usize) -> usize {
        self.chans.get(ch).map_or(0, |c| c.d)
    }

    /// The lowpass taps of channel `ch`.
    pub fn taps(&self, ch: usize) -> &[f32] {
        self.chans.get(ch).map_or(&[], |c| &c.taps)
    }

    /// The delay of channel `ch`'s filter in input samples.
    pub fn delay(&self, ch: usize) -> f64 {
        self.chans
            .get(ch)
            .map_or(0.0, |c| (c.taps.len() - 1) as f64 / 2.0)
    }

    /// Feeds input; appends each channel's new samples to `out[ch]`. Output comes in whole hops of `fft_size / 2` inputs. Errors when
    /// `out` does not have one `Vec` per channel. Allocates nothing once the `Vec`s have grown.
    pub fn process(
        &mut self,
        input: &[Complex32],
        out: &mut [Vec<Complex32>],
    ) -> Result<(), DspError> {
        if out.len() != self.chans.len() {
            return Param::Output.length(self.chans.len(), out.len());
        }
        let (l, s) = (self.l, self.s);
        let mut rest = input;
        while !rest.is_empty() {
            let take = (l - self.hist.len()).min(rest.len());
            self.hist.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.hist.len() < l {
                break;
            }
            self.spec.copy_from_slice(&self.hist);
            self.fft.process(&mut self.spec);
            if self.past_cap > 0 {
                let at = self.past_next * l;
                self.past[at..at + l].copy_from_slice(&self.spec);
                self.past_next = (self.past_next + 1) % self.past_cap;
                self.past_len = (self.past_len + 1).min(self.past_cap);
            }
            // The window's first sample is at absolute time block * S - (L - S).
            let s0 = self.block as i128 * s as i128 - (l - s) as i128;
            for (c, o) in self.chans.iter_mut().zip(out.iter_mut()) {
                let start = c.filter_block(&self.spec, s0, l, s, o);
                c.nco.mix_in_place(&mut o[start..]);
            }
            self.block += 1;
            // Keep the last L - S samples as the next window's history.
            self.hist.copy_within(s.., 0);
            self.hist.truncate(l - s);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fir::response;
    use crate::testutil::*;
    use num_complex::Complex64;

    #[test]
    fn polyphase_band_output_is_the_mixed_and_filtered_input() {
        let (n, t, os) = (16usize, 8usize, 2usize);
        let mut b = PolyphaseBank::new(n, t, os, 60.0, &[3, 13]).unwrap();
        let proto = windowed_sinc(n * t, 1.0 / n as f64, design_beta(60.0)).unwrap();
        let proto32: Vec<f32> = proto.iter().map(|&v| v as f32).collect();
        // A tone 0.3 of a spacing above band 3.
        let f = (3.0 + 0.3) / n as f64;
        let x = ctone(16_000, f, 1.0);
        let mut out = vec![Vec::new(), Vec::new()];
        for c in x.chunks(1234) {
            b.process(c, &mut out).unwrap();
        }
        assert_eq!(out[0].len(), 16_000 / (n / os));
        let delta = f - 3.0 / n as f64;
        let h = response(&proto32, delta);
        for (m, y) in out[0].iter().enumerate().skip(20) {
            let tt = ((m + 1) * (n / os) - 1) as f64;
            let want = Complex64::from_polar(1.0, TAU * ((delta * tt) % 1.0)) * h;
            assert!(
                (y.re as f64 - want.re).abs() < 2e-3 && (y.im as f64 - want.im).abs() < 2e-3,
                "{m}: {y} vs {want}"
            );
        }
        // Band 13 (-3 spacings) sees nothing of it.
        assert!(db(camp(&out[1][20..], 0.0).max(1e-12)) < -55.0);
        assert!(
            b.flat_edge() > 0.6 && b.flat_edge() < 0.9,
            "{}",
            b.flat_edge()
        );
    }

    #[test]
    fn polyphase_bank_rejects_aliases() {
        let (n, t) = (32usize, 12usize);
        let mut b = PolyphaseBank::new(n, t, 2, 70.0, &[5]).unwrap();
        let edge = b.flat_edge();
        // Inside the flat part: unity gain; beyond the alias-free region (os - flat_edge spacings): below -70 dB.
        for (off, pass) in [
            (0.0, true),
            (0.9 * edge, true),
            (-0.9 * edge, true),
            (2.0 - 0.9 * edge, false),
            (-1.5, false),
        ] {
            b.reset();
            let f = (5.0 + off) / n as f64;
            let mut out = vec![Vec::new()];
            b.process(&ctone(40_000, f, 1.0), &mut out).unwrap();
            // The output rate is 2B: the tone appears at off / 2 cycles per output sample (aliased if outside).
            let fo = (off / 2.0 + 0.5).rem_euclid(1.0) - 0.5;
            let g = db(camp(&out[0][40..], fo));
            if pass {
                assert!(g.abs() < 0.01, "{off}: {g}");
            } else {
                assert!(g < -70.0, "{off}: {g}");
            }
        }
        assert!(PolyphaseBank::new(12, 8, 2, 60.0, &[0]).is_err());
        assert!(PolyphaseBank::new(16, 8, 3, 60.0, &[0]).is_err());
        assert!(PolyphaseBank::new(16, 8, 2, 60.0, &[16]).is_err());
        let mut b = PolyphaseBank::new(16, 8, 2, 60.0, &[1]).unwrap();
        assert!(b.process(&[Complex32::default()], &mut []).is_err());
        assert_eq!(b.band_of(-1.0 / 16.0 + 0.001).0, 15);
    }

    #[test]
    fn overlap_save_channel_is_the_mixed_and_filtered_input() {
        let rate = 1e6;
        let mut b = OverlapSaveBank::new(rate, 4096).unwrap();
        let off = 200e3 + 37.0; // off the bin grid: the residual is mixed out
        let ch = b.add_channel(off, 40e3, 60e3, 60.0).unwrap();
        let d = b.decimation(ch);
        assert_eq!(d, 8);
        let f_hz = off + 13_000.0;
        let x = ctone(100_000, f_hz / rate, 1.0);
        let mut out = vec![Vec::new()];
        for c in x.chunks(3001) {
            b.process(c, &mut out).unwrap();
        }
        assert_eq!(out[0].len(), 100_000 / 2048 * (2048 / d));
        let taps = b.taps(ch).to_vec();
        let h = response(&taps, 13_000.0 / rate);
        for (j, y) in out[0].iter().enumerate().skip(300).step_by(17) {
            let t = (j * d) as f64;
            let want = Complex64::from_polar(1.0, TAU * ((13_000.0 / rate * t) % 1.0)) * h;
            assert!(
                (y.re as f64 - want.re).abs() < 1e-3 && (y.im as f64 - want.im).abs() < 1e-3,
                "{j}: {y} vs {want}"
            );
        }
        // A tone beyond the stop band (it would alias onto the channel) is gone.
        let mut b2 = OverlapSaveBank::new(rate, 4096).unwrap();
        b2.add_channel(off, 40e3, 60e3, 60.0).unwrap();
        let mut out2 = vec![Vec::new()];
        b2.process(&ctone(60_000, (off + 100e3) / rate, 1.0), &mut out2)
            .unwrap();
        let fo = ((100e3 / (rate / d as f64)) + 0.5).rem_euclid(1.0) - 0.5;
        assert!(db(camp(&out2[0][100..], fo)) < -60.0);
        // Errors.
        assert!(b.add_channel(0.0, 50e3, 40e3, 60.0).is_err());
        assert!(b.add_channel(0.0, 10e3, 10.5e3, 80.0).is_err());
        assert!(OverlapSaveBank::new(rate, 1000).is_err());
        assert!(b.process(&[Complex32::default()], &mut []).is_err());
        assert!(
            OverlapSaveBank::size_for(rate, 500.0, 80.0)
                >= 2 * (kaiser_len(80.0, 500.0 / rate).unwrap() - 1)
        );
    }

    #[test]
    fn overlap_save_cuts_do_not_matter() {
        let rate = 48_000.0;
        let x = ctone(20_000, 0.0371, 0.5);
        let run = |chunk: usize| {
            let mut b = OverlapSaveBank::new(rate, 1024).unwrap();
            b.add_channel(1500.0, 1000.0, 2500.0, 50.0).unwrap();
            b.add_channel(-6000.0, 500.0, 1500.0, 50.0).unwrap();
            let mut out = vec![Vec::new(), Vec::new()];
            for c in x.chunks(chunk) {
                b.process(c, &mut out).unwrap();
            }
            out
        };
        assert_eq!(run(20_000), run(7));
    }

    #[test]
    fn overlap_save_replay_continues_without_a_gap() {
        // A channel added late and replayed gives what a channel there from the start gives over the same time.
        let rate = 48_000.0;
        let x = ctone(40_000, 0.0371, 0.5);
        let mut whole = OverlapSaveBank::new(rate, 1024).unwrap();
        whole.add_channel(1800.0, 400.0, 900.0, 60.0).unwrap();
        let mut want = vec![Vec::new()];
        whole.process(&x, &mut want).unwrap();
        let mut late = OverlapSaveBank::new(rate, 1024).unwrap();
        late.keep_history(8);
        late.add_channel(-5000.0, 400.0, 900.0, 60.0).unwrap();
        let mut first = vec![Vec::new()];
        late.process(&x[..20_000], &mut first).unwrap();
        assert_eq!(late.history_blocks(), 8);
        late.remove_channel(0).unwrap();
        assert!(late.remove_channel(0).is_err());
        let ch = late.add_channel(1800.0, 400.0, 900.0, 60.0).unwrap();
        let mut got = Vec::new();
        late.replay(ch, &mut got).unwrap();
        let mut rest = vec![Vec::new()];
        late.process(&x[20_000..], &mut rest).unwrap();
        got.extend_from_slice(&rest[0]);
        // 20 000 inputs make 39 blocks of 512; the 8 kept begin at block 31.
        let d = late.decimation(ch);
        let skip = 31 * 512 / d;
        assert_eq!(got.len(), want[0].len() - skip);
        for (j, (a, b)) in got.iter().zip(&want[0][skip..]).enumerate() {
            assert!((a - b).norm() < 1e-4, "{j}: {a} vs {b}");
        }
        assert!(late.replay(5, &mut got).is_err());
    }
}
