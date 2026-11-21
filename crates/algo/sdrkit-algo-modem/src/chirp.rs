//! Chirp spread spectrum (LoRa-style CSS): dechirping, the FFT bins of a symbol with a fractional peak, and a timing loop.
//!
//! A symbol of spreading factor `SF` is `N = 2^SF` chips at the chip rate (the bandwidth). The base up-chirp is
//! `c[n] = e^(j 2 pi (n^2 / (2N) - n/2))`, sweeping from `-bw/2` to `+bw/2`; symbol `s` is the base chirp started `s` chips in,
//! which after multiplication by `conj(c)` is a tone on FFT bin `s`. Down-chirps (the LoRa SFD) are dechirped with `c` itself.
//!
//! [`Dechirp`] reads one symbol (every `os`-th sample of an input oversampled `os` times, already filtered to the bandwidth),
//! removes a frequency offset given in bins, dechirps, and runs the FFT (planned once). It returns the peak bin, the fractional
//! offset of the peak by Candan's estimator (`sdrkit_algo_dsp::peak::candan`), the peak power and the mean power of the other bins; the
//! power of every bin stays available for soft decisions: `N`-ary orthogonal signalling detected non-coherently, so
//! [`crate::fsk::FskLlr`] computes the LLRs of the symbol's bits from the bins (with the stack's own bit map), and
//! [`Dechirp::amplitude_noise`] returns its parameters. [`Dechirp::dechirp_at`] is the burst form: it reads a stored capture at a
//! fractional start through `sdrkit_algo_dsp::interp::Reader`, so timing trials need no copy.
//!
//! A timing error of `d` chips moves the dechirped tone by `d` bins, as a frequency error does; once the preamble has separated the
//! two, [`ChirpTracker`] follows the remaining clock drift with a second-order loop. On a data symbol the Candan fraction does not
//! measure timing: the part after the frequency wrap is turned by `-2 pi d`, which cancels the shift on average over the symbol
//! values. [`ChirpPeak::timing`] measures that phase step instead (after removing the tone's own progression, found from the phase
//! step between the two halves of each segment), which returns the timing alone, whatever the frequency offset; the tracker takes it.
//!
//! ```
//! use sdrkit_algo_modem::chirp::{Dechirp, Direction};
//! let mut d = Dechirp::new(7, 1).unwrap();
//! // Symbol 42: the up-chirp started 42 chips in.
//! let x: Vec<_> = (0..128).map(|n| d.base_chirp((n + 42) % 128)).collect();
//! let r = d.dechirp(&x, Direction::Up, 0.0).unwrap();
//! assert_eq!(r.bin, 42);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::interp::{Cubic, Reader};
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_algo_dsp::peak;
use std::f64::consts::TAU;

/// Which chirp a symbol is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// An up-chirp (data symbols, preamble).
    Up,
    /// A down-chirp (the LoRa start-of-frame delimiter).
    Down,
}

/// The result of dechirping one symbol.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChirpPeak {
    /// The strongest bin (`0..N`).
    pub bin: usize,
    /// The peak's offset from `bin` in bins (`-0.5 .. 0.5`, Candan's estimator over the whole symbol). On an unwrapped chirp
    /// (the preamble, symbol 0) it is the timing error in chips plus the frequency error in bins.
    pub frac: f64,
    /// The timing error in chips (positive: read late), from the phase step at the frequency wrap of an up-chirp (`None` for a
    /// down-chirp, or when the wrap leaves less than two chips on one side, as for symbol 0). A frequency offset does not move it.
    pub timing: Option<f64>,
    /// The tone's frequency offset from `bin` within the two segments either side of the wrap, in bins: timing error plus
    /// frequency error (so `freq - timing` is the residual frequency offset).
    pub freq: f64,
    /// The power of the strongest bin.
    pub power: f32,
    /// The mean power of the other bins (the noise per bin when one symbol is present).
    pub noise: f32,
}

impl ChirpPeak {
    /// Peak power over mean noise per bin, linear.
    pub fn ratio(&self) -> f32 {
        if self.noise > 0.0 {
            self.power / self.noise
        } else {
            f32::INFINITY
        }
    }
}

/// The dechirper of one spreading factor (module documentation).
#[derive(Clone, Debug)]
pub struct Dechirp {
    sf: u8,
    n: usize,
    os: usize,
    chirp: Vec<Complex32>,
    fft: Fft,
    buf: Vec<Complex32>,
    power: Vec<f32>,
    dech: Vec<Complex32>,
    reader: Reader<Cubic>,
}

impl Dechirp {
    /// Spreading factor 5 to 12, input oversampled `os` times (1 to 64).
    pub fn new(sf: u8, os: usize) -> Result<Self, ModemError> {
        Param::SpreadingFactor.check_count(sf as usize, 5, 12)?;
        Param::Oversampling.check_count(os, 1, 64)?;
        let n = 1usize << sf;
        let chirp = (0..n)
            .map(|k| {
                let k = k as f64;
                // Phase in cycles, reduced exactly in f64 before the angle is formed.
                let cyc = (k * k / (2.0 * n as f64) - k / 2.0).rem_euclid(1.0);
                Complex32::new((TAU * cyc).cos() as f32, (TAU * cyc).sin() as f32)
            })
            .collect();
        Ok(Dechirp {
            sf,
            n,
            os,
            chirp,
            fft: Fft::forward(n),
            buf: vec![Complex32::default(); n],
            power: vec![0.0; n],
            dech: vec![Complex32::default(); n],
            reader: Reader::new(Cubic),
        })
    }

    /// The spreading factor.
    pub fn sf(&self) -> u8 {
        self.sf
    }

    /// Chips (and bins) per symbol, `2^SF`.
    pub fn chips(&self) -> usize {
        self.n
    }

    /// Input samples per symbol, `2^SF os`.
    pub fn symbol_len(&self) -> usize {
        self.n * self.os
    }

    /// Chip `k` of the base up-chirp.
    pub fn base_chirp(&self, k: usize) -> Complex32 {
        self.chirp[k % self.n]
    }

    /// The power of every bin from the last call.
    pub fn bins(&self) -> &[f32] {
        &self.power
    }

    /// The complex bins from the last call.
    pub fn spectrum(&self) -> &[Complex32] {
        &self.buf
    }

    /// The amplitude `A` and noise variance per bin `N` of the last symbol for [`crate::fsk::FskLlr::llrs`]: `N` the mean power of
    /// the bins other than the peak, `A^2` the peak power less `N` (at least 5 % of `N`).
    pub fn amplitude_noise(&self, p: &ChirpPeak) -> (f32, f32) {
        let n = p.noise.max(f32::MIN_POSITIVE);
        let a2 = (p.power - n).max(0.05 * n);
        (a2.sqrt(), n)
    }

    fn finish(&mut self, dir: Direction, freq_bins: f64) -> ChirpPeak {
        // Dechirp and remove the offset with one rotator (stepped by multiplication: N <= 4096 steps stay within 1e-4).
        let (s, c) = approx::sin_cos(-TAU * freq_bins / self.n as f64);
        let step = Complex32::new(c, s);
        let mut rot = Complex32::new(1.0, 0.0);
        for (v, &ch) in self.buf.iter_mut().zip(&self.chirp) {
            let base = match dir {
                Direction::Up => ch.conj(),
                Direction::Down => ch,
            };
            *v = *v * base * rot;
            rot *= step;
        }
        self.dech.copy_from_slice(&self.buf);
        self.fft.process(&mut self.buf);
        let mut total = 0f64;
        for (p, v) in self.power.iter_mut().zip(&self.buf) {
            *p = v.norm_sqr();
            total += *p as f64;
        }
        let bin = peak::argmax(&self.power).unwrap_or(0);
        let n = self.n;
        let frac = peak::candan(
            self.buf[(bin + n - 1) % n],
            self.buf[bin],
            self.buf[(bin + 1) % n],
            n,
        );
        let pk = self.power[bin];
        let (timing, freq) = self.wrap_timing(bin, dir);
        ChirpPeak {
            bin,
            frac,
            timing,
            freq,
            power: pk,
            noise: ((total - pk as f64) / (n - 1) as f64) as f32,
        }
    }

    /// Splits the dechirped symbol at the frequency wrap of symbol `bin`: the tone's frequency from the phase step between the
    /// two halves of each segment (the sums of the halves are a half segment apart, and the longer segment counts more), and
    /// the timing from the phase jump between the segments (`-2 pi d` for a read `d` chips late, after removing the tone's own
    /// progression `pi f`).
    fn wrap_timing(&self, bin: usize, dir: Direction) -> (Option<f64>, f64) {
        let n = self.n;
        let w = (n - bin) % n;
        let (s, c) = approx::sin_cos(-TAU * bin as f64 / n as f64);
        let step = Complex32::new(c, s);
        let mut rot = Complex32::new(1.0, 0.0);
        // The sums of the four parts: the halves of the segment before the wrap, then those of the segment after it.
        let ends = [w / 2, w, w + (n - w) / 2, n];
        let mut sums = [Complex32::default(); 4];
        let mut part = 0;
        for (k, &v) in self.dech.iter().enumerate() {
            while k >= ends[part] {
                part += 1;
            }
            sums[part] += v * rot;
            rot *= step;
        }
        // The tone offset of a segment of `len` chips whose halves are `first` and `second`, in bins.
        let offset = |first: Complex32, second: Complex32, len: usize| {
            if len < 2 {
                return 0.0;
            }
            let z = second * first.conj();
            approx::atan2(z.im, z.re) as f64 * n as f64 / (std::f64::consts::PI * len as f64)
        };
        let (la, lb) = (w as f64, (n - w) as f64);
        let (wa, wb) = (la * la * la, lb * lb * lb);
        let f =
            (wa * offset(sums[0], sums[1], w) + wb * offset(sums[2], sums[3], n - w)) / (wa + wb);
        let (a, b) = (sums[0] + sums[1], sums[2] + sums[3]);
        let timing = if dir == Direction::Up && w >= 2 && n - w >= 2 {
            let j = b * a.conj();
            let phi = approx::atan2(j.im, j.re) as f64;
            let d = -(phi - std::f64::consts::PI * f) / TAU;
            Some((d + 0.5).rem_euclid(1.0) - 0.5)
        } else {
            None
        };
        (timing, f)
    }

    /// Dechirps the symbol in `x` (`symbol_len()` samples or more, the symbol starting at `x[0]`), removing a frequency offset of
    /// `freq_bins` bins (one bin is `bw / N`).
    pub fn dechirp(
        &mut self,
        x: &[Complex32],
        dir: Direction,
        freq_bins: f64,
    ) -> Result<ChirpPeak, ModemError> {
        let len = self.symbol_len();
        if x.len() < len {
            return Err(ModemError::InputTooShort { need: len });
        }
        if !freq_bins.is_finite() {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        for (k, v) in self.buf.iter_mut().enumerate() {
            *v = x[k * self.os];
        }
        Ok(self.finish(dir, freq_bins))
    }

    /// The burst form: dechirps the symbol starting at the fractional position `start` of a stored capture (read through a cubic
    /// interpolator; samples outside read as zero), removing `freq_bins`.
    pub fn dechirp_at(
        &mut self,
        x: &[Complex32],
        start: f64,
        dir: Direction,
        freq_bins: f64,
    ) -> Result<ChirpPeak, ModemError> {
        if !(start.is_finite() && freq_bins.is_finite()) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let os = self.os as f64;
        for (k, v) in self.buf.iter_mut().enumerate() {
            *v = self.reader.at(x, start + k as f64 * os);
        }
        Ok(self.finish(dir, freq_bins))
    }
}

/// Follows the symbol timing of a chirp stream from the fractional peak of each dechirped symbol (module documentation).
#[derive(Clone, Debug)]
pub struct ChirpTracker {
    lf: LoopFilter,
    os: f64,
    period: f64,
    next: f64,
}

impl ChirpTracker {
    /// A tracker for symbols of `symbol_len` samples at `os` samples per chip, loop noise bandwidth `bn_t` per symbol, damping 1,
    /// with the clock rate limited to `max_ppm`.
    pub fn new(symbol_len: usize, os: usize, bn_t: f64, max_ppm: f64) -> Result<Self, ModemError> {
        if symbol_len == 0 {
            return Param::SymbolLen.at_least(1.0, 0.0);
        }
        if os == 0 {
            return Param::Oversampling.at_least(1.0, 0.0);
        }
        Param::MaxPpm.check_in(max_ppm, 0.0, f64::INFINITY)?;
        let mut lf = LoopFilter::new(bn_t, 1.0)?;
        lf.set_clamp(max_ppm * 1e-6 * symbol_len as f64);
        Ok(ChirpTracker {
            lf,
            os: os as f64,
            period: symbol_len as f64,
            next: 0.0,
        })
    }

    /// Starts tracking with the next symbol at `start` (samples) and a drift of `drift` samples per symbol (from the preamble).
    pub fn start(&mut self, start: f64, drift: f64) {
        self.lf.reset();
        self.lf.set_integrator(drift);
        self.next = start;
    }

    /// Where the next symbol starts (samples, fractional).
    pub fn next_start(&self) -> f64 {
        self.next
    }

    /// The drift in samples per symbol.
    pub fn drift(&self) -> f64 {
        self.lf.integrator()
    }

    /// The clock error in ppm (positive: symbols longer than nominal).
    pub fn clock_ppm(&self) -> f64 {
        self.lf.integrator() / self.period * 1e6
    }

    /// Takes the timing error (chips, positive late: [`ChirpPeak::timing`], or the fraction of a preamble chirp) of the symbol
    /// just read at [`next_start`](Self::next_start) and moves to the next symbol, which a late read moves earlier.
    pub fn update(&mut self, timing_chips: f64) -> f64 {
        let e = if timing_chips.is_finite() {
            timing_chips.clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let err = -e * self.os;
        let c = self.lf.update(err);
        self.next += self.period + c;
        self.next
    }
}

pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};
    fn run(seconds: f64, sf: u8) -> BenchResult {
        // LoRa at 125 kHz, critically sampled: 125 k samples per second.
        let mut d = Dechirp::new(sf, 1).expect("valid");
        let n = d.symbol_len();
        let syms = (65_536 / n).max(1);
        let x: Vec<Complex32> = (0..n * syms).map(|k| d.base_chirp(k + 17)).collect();
        measure(seconds, n * syms, (n * syms) as f64 / 125e3, || {
            for s in x.chunks_exact(n) {
                let _ = d.dechirp(s, Direction::Up, 0.1);
            }
        })
    }
    vec![
        Benchmark {
            name: "modem.dechirp_sf7_125k",
            run: |s| run(s, 7),
        },
        Benchmark {
            name: "modem.dechirp_sf12_125k",
            run: |s| run(s, 12),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsk::ln_i0;
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;

    /// A LoRa symbol `s` at `os` samples per chip, generated from the instantaneous frequency (wrapping from +bw/2 to -bw/2),
    /// written independently of the dechirper's table.
    fn symbol(sf: u8, os: usize, s: usize, delay: f64, cfo_bins: f64, out: &mut Vec<Complex32>) {
        let n = 1usize << sf;
        let mut phase = 0.0f64;
        for k in 0..n * os {
            // Chip time at the middle of this sample's step; frequency in cycles per chip.
            let tt = (k as f64 + 0.5) / os as f64 - delay;
            let f = (s as f64 + tt).rem_euclid(n as f64) / n as f64 - 0.5 + cfo_bins / n as f64;
            out.push(Complex32::new(
                (TAU * phase).cos() as f32,
                (TAU * phase).sin() as f32,
            ));
            phase = (phase + f / os as f64).rem_euclid(1.0);
        }
    }

    #[test]
    fn every_symbol_lands_on_its_bin() {
        for sf in [5u8, 7, 9, 12] {
            let mut d = Dechirp::new(sf, 2).unwrap();
            let n = 1usize << sf;
            for s in [0usize, 1, n / 3, n - 1] {
                let mut x = Vec::new();
                symbol(sf, 2, s, 0.0, 0.0, &mut x);
                let r = d.dechirp(&x, Direction::Up, 0.0).unwrap();
                assert_eq!(r.bin, s, "SF{sf} symbol {s}");
                assert!(r.ratio() > 100.0);
            }
            // A down-chirp dechirps with the up-chirp.
            let x: Vec<Complex32> = (0..n * 2).map(|k| d.base_chirp(k / 2).conj()).collect();
            assert_eq!(d.dechirp(&x, Direction::Down, 0.0).unwrap().bin, 0);
        }
    }

    #[test]
    fn fractional_offsets_and_timing() {
        let mut d = Dechirp::new(8, 1).unwrap();
        // A frequency offset of 0.3 bin shows as the fraction, and removing it returns 0.
        let mut x = Vec::new();
        symbol(8, 1, 77, 0.0, 0.3, &mut x);
        let r = d.dechirp(&x, Direction::Up, 0.0).unwrap();
        assert_eq!(r.bin, 77);
        assert!((r.frac - 0.3).abs() < 0.03, "{}", r.frac);
        let r = d.dechirp(&x, Direction::Up, 0.3).unwrap();
        assert!(r.frac.abs() < 0.03);
        // Read 0.25 chip late through the burst form (at 4 samples per chip, so the interpolator sees the chirp well sampled):
        // +0.25 bin.
        let mut d4 = Dechirp::new(8, 4).unwrap();
        let mut y = vec![Complex32::default(); 10];
        symbol(8, 4, 77, 0.0, 0.0, &mut y);
        y.extend_from_slice(&[Complex32::default(); 10]);
        // Symbol 0 (an unwrapped chirp, as in the preamble) moves by the full 0.25 bin; a data symbol wraps part-way, and the
        // part after the wrap turns by the timing error, which pulls its fraction towards 0 (but keeps the sign).
        let mut z = vec![Complex32::default(); 10];
        symbol(8, 4, 0, 0.0, 0.0, &mut z);
        z.extend_from_slice(&[Complex32::default(); 10]);
        let r = d4.dechirp_at(&z, 11.0, Direction::Up, 0.0).unwrap();
        assert_eq!(r.bin, 0);
        assert!((r.frac - 0.25).abs() < 0.03, "{}", r.frac);
        let r = d4.dechirp_at(&y, 11.0, Direction::Up, 0.0).unwrap();
        assert_eq!(r.bin, 77);
        // The wrap's phase step measures the timing on every data symbol, with or without a frequency offset.
        for cfo in [0.0, 0.2] {
            for sym in (2..254).step_by(9) {
                let mut w = vec![Complex32::default(); 10];
                symbol(8, 4, sym, 0.0, cfo, &mut w);
                w.extend_from_slice(&[Complex32::default(); 10]);
                let r = d4.dechirp_at(&w, 11.0, Direction::Up, 0.0).unwrap();
                let t = r.timing.unwrap();
                assert!((t - 0.25).abs() < 0.03, "symbol {sym} cfo {cfo}: {t}");
                assert!((r.freq - t - cfo).abs() < 0.03, "{}", r.freq);
            }
        }
    }

    #[test]
    fn timing_and_tone_offset_hold_in_noise() {
        // SF9 at -6 dB in the bandwidth, read 0.2 chip late with a carrier error of 0.15 bin: over the symbols whose wrap is in
        // the middle half, the timing and the tone offset keep their means, and one symbol is right to a few hundredths.
        let (sf, os) = (9u8, 4usize);
        let n = 1usize << sf;
        let mut d = Dechirp::new(sf, os).unwrap();
        let mut awgn = Awgn::with_power(4.0, 5).unwrap();
        let mut rng = Rng::new(9);
        let (mut t, mut f) = (Vec::new(), Vec::new());
        for _ in 0..400 {
            let s = n / 4 + rng.below(n as u64 / 2) as usize;
            let mut x = vec![Complex32::default(); 10];
            symbol(sf, os, s, 0.0, 0.15, &mut x);
            x.extend_from_slice(&[Complex32::default(); 10]);
            // The dechirper reads one sample per chip, so the noise power per sample is the noise power per chip.
            awgn.process(&mut x);
            let r = d.dechirp_at(&x, 10.8, Direction::Up, 0.0).unwrap();
            if r.bin == s {
                t.push(r.timing.unwrap());
                f.push(r.freq);
            }
        }
        assert!(t.len() > 390, "{}", t.len());
        let stats = |v: &[f64]| {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
            (m, sd)
        };
        let (tm, ts) = stats(&t);
        let (fm, fs) = stats(&f);
        assert!((tm - 0.2).abs() < 0.02 && ts < 0.08, "timing {tm} {ts}");
        assert!((fm - 0.35).abs() < 0.03 && fs < 0.12, "offset {fm} {fs}");
    }

    #[test]
    fn tracker_follows_a_clock_error() {
        // SF7 at 4 samples per chip, the transmitter clock 40 ppm slow (symbols 40 ppm longer), 300 symbols.
        let (sf, os) = (7u8, 4usize);
        let n = 1usize << sf;
        let ppm = 40.0;
        let mut rng = Rng::new(2);
        let mut x = Vec::new();
        let mut syms = Vec::new();
        for _ in 0..300 {
            let s = rng.below(n as u64) as usize;
            syms.push(s);
            symbol(sf, os, s, 0.0, 0.0, &mut x);
        }
        // Stretch by resampling the clean signal at (1 - ppm) steps: sample m of the received signal reads x at m (1 - ppm 1e-6).
        let r = Reader::new(Cubic);
        let y: Vec<Complex32> = (0..(x.len() as f64 * (1.0 + ppm * 1e-6)) as usize)
            .map(|m| r.at(&x, m as f64 * (1.0 - ppm * 1e-6)))
            .collect();
        let mut d = Dechirp::new(sf, os).unwrap();
        let mut t = ChirpTracker::new(n * os, os, 0.02, 200.0).unwrap();
        t.start(0.0, 0.0);
        let mut wrong = 0;
        for (k, &s) in syms.iter().enumerate() {
            let p = d
                .dechirp_at(&y, t.next_start(), Direction::Up, 0.0)
                .unwrap();
            if k > 50 && p.bin != s {
                wrong += 1;
            }
            t.update(p.timing.unwrap_or(0.0));
        }
        assert_eq!(wrong, 0);
        assert!((t.clock_ppm() - ppm).abs() < 10.0, "{}", t.clock_ppm());
    }

    /// Symbol error rate of N-ary orthogonal signalling detected non-coherently at Es/N0 `lambda` (linear), by integration:
    /// `1 - integral f_signal(x) (1 - e^-x)^(N-1) dx` over normalised energies.
    fn ser_theory(nbins: usize, lambda: f64) -> f64 {
        let hi = lambda + 20.0 * lambda.sqrt() + 60.0;
        let steps = 200_000;
        let h = hi / steps as f64;
        let mut s = 0.0;
        for i in 0..=steps {
            let x = i as f64 * h;
            let ln_f = -(x + lambda) + ln_i0((2.0 * (lambda * x).sqrt()) as f32) as f64;
            let pc = (1.0 - (-x).exp()).powi(nbins as i32 - 1);
            let w = if i == 0 || i == steps { 0.5 } else { 1.0 };
            s += w * ln_f.exp() * pc;
        }
        1.0 - s * h
    }

    #[test]
    fn symbol_error_rate_follows_theory() {
        // SF7 at the chip rate: SNR in the bandwidth -10 and -9 dB, Es/N0 = 128 SNR.
        let sf = 7u8;
        let n = 1usize << sf;
        let mut d = Dechirp::new(sf, 1).unwrap();
        let mut rng = Rng::new(1);
        for snr_db in [-10.0f64, -9.0] {
            let snr = 10f64.powf(snr_db / 10.0);
            let mut awgn = Awgn::with_power(1.0 / snr, 77).unwrap();
            let trials = 20_000;
            let mut errors = 0;
            let mut x = Vec::with_capacity(n);
            for _ in 0..trials {
                let s = rng.below(n as u64) as usize;
                x.clear();
                x.extend((0..n).map(|k| d.base_chirp(k + s)));
                awgn.process(&mut x);
                if d.dechirp(&x, Direction::Up, 0.0).unwrap().bin != s {
                    errors += 1;
                }
            }
            let got = errors as f64 / trials as f64;
            let want = ser_theory(n, n as f64 * snr);
            assert!(
                got > 0.8 * want && got < 1.25 * want,
                "{snr_db} dB: {got} vs {want}"
            );
        }
    }

    #[test]
    fn soft_bins_give_llrs() {
        let mut d = Dechirp::new(7, 1).unwrap();
        let x: Vec<Complex32> = (0..128).map(|k| d.base_chirp(k + 5)).collect();
        let p = d.dechirp(&x, Direction::Up, 0.0).unwrap();
        let (a, nv) = d.amplitude_noise(&p);
        let llr = crate::fsk::FskLlr::natural(128).unwrap();
        let mut l = [0f32; 7];
        llr.llrs(d.bins(), a, nv.max(1e-3), &mut l).unwrap();
        // 5 = 0000101.
        let bits: Vec<u8> = l.iter().map(|&v| (v < 0.0) as u8).collect();
        assert_eq!(bits, [0, 0, 0, 0, 1, 0, 1]);
        assert!(Dechirp::new(4, 1).is_err());
        assert!(d.dechirp(&x[..10], Direction::Up, 0.0).is_err());
    }
}
