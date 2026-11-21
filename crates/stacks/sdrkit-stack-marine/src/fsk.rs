//! The FSK front of DSC, NAVTEX and ATIS: the FSK pair finder on the 24 kHz channel, the MF/HF slicer and the VHF AFSK slicer.
//! Both slicers give one soft value per symbol, positive for the higher tone.
//!
//! Standard: ITU-R M.493-15 section 2, ITU-R M.625-4 section 2

use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::freq::centroid;
use sdrkit_algo_modem::fsk::{Analytic, FskLlr, MfskConfig, MfskDemod, TrackedFskLlr};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The channel rate of the marine front end.
pub const CHANNEL_RATE: f64 = 24_000.0;
/// The rate the MF/HF slicer works at after its decimator.
pub const SLICER_RATE: f64 = 2_400.0;
/// The largest soft value a slicer outputs.
pub const SOFT_LIMIT: f32 = 50.0;
/// Symbols over which the tone amplitude and the noise of the soft values are averaged.
const LLR_MEMORY: f32 = 16.0;

/// The soft value of one symbol from its energies `[low, high]`: the LLR of the higher tone (positive when it was sent), held
/// within [`SOFT_LIMIT`]. A value that is not finite returns 0.
fn soft_of(llr: &mut TrackedFskLlr, energies: &[f32]) -> f32 {
    let mut l = [0f32];
    if llr.symbol(energies, &mut l).is_err() {
        return 0.0;
    }
    // The LLR is positive for tone 0, the lower tone.
    let v = -l[0];
    if v.is_finite() {
        v.clamp(-SOFT_LIMIT, SOFT_LIMIT)
    } else {
        0.0
    }
}

fn tracked() -> TrackedFskLlr {
    // Two tones with natural labels and a memory of 16 symbols: both are valid, so the constructors cannot fail.
    match FskLlr::natural(2).and_then(|l| TrackedFskLlr::new(l, LLR_MEMORY)) {
        Ok(t) => t,
        Err(_) => unreachable!("two natural labels and a positive memory are valid"),
    }
}

/// The MF/HF slicer: the 24 kHz channel mixed by the FSK centre, low-passed and decimated to 2400 Hz, then the shared M-FSK
/// demodulator with its symbol clock on the two tones at plus and minus half the shift.
#[derive(Clone, Debug)]
pub struct MfSlicer {
    nco: Nco,
    dec: FirDecimator<Complex32>,
    demod: MfskDemod,
    llr: TrackedFskLlr,
    mixed: Vec<Complex32>,
    low: Vec<Complex32>,
    energies: Vec<f32>,
    centre: f64,
    shift: f64,
    baud: f64,
    prev: Complex32,
    tones: [Option<f64>; 2],
}

impl MfSlicer {
    /// A slicer for a 24 kHz input, FSK centre `centre_hz`, shift `shift_hz` and `baud` symbols a second. An error when the shift
    /// or the baud does not fit the 2400 Hz slicer rate.
    pub fn new(
        centre_hz: f64,
        shift_hz: f64,
        baud: f64,
    ) -> Result<Self, sdrkit_algo_modem::ModemError> {
        let taps = fir::lowpass(CHANNEL_RATE, 400.0, 1100.0, 60.0)?;
        let half = shift_hz / 2.0;
        let demod = MfskDemod::new(MfskConfig::from_hz(&[-half, half], baud, SLICER_RATE))?;
        Ok(MfSlicer {
            nco: Nco::new(CHANNEL_RATE, -centre_hz),
            dec: FirDecimator::new(&taps, (CHANNEL_RATE / SLICER_RATE) as usize),
            demod,
            llr: tracked(),
            mixed: Vec::new(),
            low: Vec::new(),
            energies: Vec::new(),
            centre: centre_hz,
            shift: shift_hz,
            baud,
            prev: Complex32::default(),
            tones: [None; 2],
        })
    }

    /// The NAVTEX and MF/HF DSC slicer: 170 Hz shift, 100 Bd, centred on the dial.
    pub fn standard() -> Self {
        match Self::new(0.0, 170.0, 100.0) {
            Ok(s) => s,
            Err(_) => unreachable!("170 Hz at 100 Bd fits the slicer rate"),
        }
    }

    /// Follows the found FSK centre (Hz from the dial).
    pub fn set_centre(&mut self, hz: f64) {
        if (hz - self.centre).abs() > 0.5 {
            let d = hz - self.centre;
            self.centre = hz;
            self.nco.set_freq(-hz);
            for t in self.tones.iter_mut().flatten() {
                *t -= d;
            }
        }
    }

    /// The centre in use (Hz from the dial).
    pub fn centre(&self) -> f64 {
        self.centre
    }

    /// The shift in use (Hz).
    pub fn shift(&self) -> f64 {
        self.shift
    }

    /// The tone frequencies measured on the signal (low, high), Hz from the dial: the average instantaneous frequency of the samples
    /// that sit on each tone.
    pub fn tones(&self) -> Option<(f64, f64)> {
        match self.tones {
            [Some(a), Some(b)] => Some((a + self.centre, b + self.centre)),
            _ => None,
        }
    }

    /// The symbol rate the clock measures (Hz).
    pub fn symbol_rate(&self) -> f64 {
        self.baud / (1.0 + self.demod.clock_ppm() * 1e-6)
    }

    /// Appends one soft value per symbol of `x` (positive for the higher tone).
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.nco.mix_in_place(&mut self.mixed);
        self.low.clear();
        self.dec.process(&self.mixed, &mut self.low);
        let q = self.shift / 4.0;
        for &v in &self.low {
            let f = f64::from((v * self.prev.conj()).arg()) * SLICER_RATE / TAU;
            self.prev = v;
            let side = if f > q && f < 3.0 * q {
                1
            } else if f < -q && f > -3.0 * q {
                0
            } else {
                continue;
            };
            let t = &mut self.tones[side];
            *t = Some(t.map_or(f, |m| m + 0.002 * (f - m)));
        }
        self.energies.clear();
        self.demod.process(&self.low, &mut self.energies);
        for e in self.energies.as_chunks::<2>().0 {
            out.push(soft_of(&mut self.llr, e));
        }
    }

    /// Forgets the signal: the clock, the filters and the soft-value estimates. The centre stays.
    pub fn reset(&mut self) {
        self.dec.reset();
        self.demod.reset();
        self.llr.reset();
        self.tones = [None; 2];
    }
}

/// The VHF AFSK slicer: real discriminator output at 24 kHz made analytic, then the shared M-FSK demodulator on 1300 and 2100 Hz at
/// 1200 Bd. With balancing, each tone's energy is divided by its own average, for a phase-modulated carrier on which the higher
/// tone has more deviation.
#[derive(Clone, Debug)]
pub struct AfskSlicer {
    demod: MfskDemod,
    llr: TrackedFskLlr,
    energies: Vec<f32>,
    balanced: bool,
    avg: [f32; 2],
    last_energy: [f32; 2],
}

/// The low tone of VHF DSC (Y, 1).
pub const AFSK_LOW_HZ: f64 = 1300.0;
/// The high tone of VHF DSC (B, 0).
pub const AFSK_HIGH_HZ: f64 = 2100.0;
/// The VHF DSC symbol rate.
pub const AFSK_BAUD: f64 = 1200.0;

impl Default for AfskSlicer {
    fn default() -> Self {
        let cfg = MfskConfig::from_hz(&[AFSK_LOW_HZ, AFSK_HIGH_HZ], AFSK_BAUD, CHANNEL_RATE);
        let demod = match (MfskDemod::new(cfg), Analytic::for_band(CHANNEL_RATE, 300.0)) {
            (Ok(d), Ok(a)) => d.with_analytic(a),
            _ => unreachable!("1300 and 2100 Hz at 1200 Bd fit 24 kHz"),
        };
        AfskSlicer {
            demod,
            llr: tracked(),
            energies: Vec::new(),
            balanced: false,
            avg: [0.0; 2],
            last_energy: [0.0; 2],
        }
    }
}

impl AfskSlicer {
    /// A slicer that balances the two tones by their average energies.
    pub fn balanced() -> Self {
        AfskSlicer {
            balanced: true,
            ..AfskSlicer::default()
        }
    }

    /// Appends one soft value per symbol (positive for 2100 Hz) for real audio `x` at 24 kHz.
    pub fn process(&mut self, x: &[f32], out: &mut Vec<f32>) {
        self.energies.clear();
        if self.demod.process_real(x, &mut self.energies).is_err() {
            return;
        }
        for k in 0..self.energies.len() / 2 {
            let mut e = [self.energies[2 * k], self.energies[2 * k + 1]];
            self.last_energy = e;
            if self.balanced {
                let d = (e[1] - e[0]) / (e[1] + e[0] + 1e-20);
                let side = if d > 0.3 {
                    Some(1)
                } else if d < -0.3 {
                    Some(0)
                } else {
                    None
                };
                if let Some(s) = side {
                    let a = &mut self.avg[s];
                    *a = if *a == 0.0 {
                        e[s]
                    } else {
                        *a + 0.05 * (e[s] - *a)
                    };
                }
                if self.avg[0] > 0.0 && self.avg[1] > 0.0 {
                    e[0] /= self.avg[0];
                    e[1] /= self.avg[1];
                }
            }
            out.push(soft_of(&mut self.llr, &e));
        }
    }

    /// The tone energy of the last symbol (both tones).
    pub fn energy(&self) -> f32 {
        self.last_energy[0] + self.last_energy[1]
    }

    /// Forgets the signal. Balancing stays as it was set.
    pub fn reset(&mut self) {
        self.demod.reset();
        self.llr.reset();
        self.avg = [0.0; 2];
        self.last_energy = [0.0; 2];
    }
}

/// What the FSK finder sees.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct FskSeen {
    /// A pair stands above the noise.
    pub found: bool,
    /// The FSK centre, Hz from the dial.
    pub centre_hz: f64,
    /// The lower tone, Hz from the dial.
    pub low_hz: f64,
    /// The higher tone, Hz from the dial.
    pub high_hz: f64,
    /// The measured shift, Hz.
    pub shift_hz: f64,
    /// Level of the tones above the noise per bin, dB.
    pub level_db: f64,
    /// Power from the low box to the high box above the noise, against the noise in 3 kHz, dB.
    pub snr_db: f64,
}

/// The FFT size of the finder.
pub const FINDER_N: usize = 8192;
/// The bins the audio spectrum has.
pub const AUDIO_BINS: usize = 342;

/// The FSK pair finder: an 8192-point Hann spectrum of the 24 kHz stream every 4096 samples, averaged with factor 0.25.
pub struct FskFinder {
    fft: Fft,
    buf: Vec<Complex32>,
    work: Vec<Complex32>,
    window: Vec<f32>,
    /// The averaged power, ordered from the lowest frequency to the highest (index `FINDER_N / 2` is 0 Hz).
    avg: Vec<f32>,
    scratch: Vec<f32>,
    fill: usize,
    shift: f64,
    frames: u64,
    /// The latest result.
    pub seen: FskSeen,
}

const DC: isize = (FINDER_N / 2) as isize;
const BIN_HZ: f64 = CHANNEL_RATE / FINDER_N as f64;

impl FskFinder {
    /// A finder for a pair `shift_hz` apart.
    pub fn new(shift_hz: f64) -> Self {
        FskFinder {
            fft: Fft::forward(FINDER_N),
            buf: vec![Complex32::default(); FINDER_N],
            work: vec![Complex32::default(); FINDER_N],
            window: Window::Hann.coefficients(FINDER_N),
            avg: vec![0.0; FINDER_N],
            scratch: Vec::with_capacity(FINDER_N),
            fill: 0,
            shift: shift_hz,
            frames: 0,
            seen: FskSeen::default(),
        }
    }

    /// Feeds samples of the 24 kHz channel. Returns true when a new estimate is ready.
    pub fn process(&mut self, x: &[Complex32]) -> bool {
        let mut new = false;
        for &s in x {
            self.buf[self.fill] = s;
            self.fill += 1;
            if self.fill == FINDER_N {
                self.analyse();
                self.buf.copy_within(FINDER_N / 2.., 0);
                self.fill = FINDER_N / 2;
                new = true;
            }
        }
        new
    }

    fn bin(&self, hz: f64) -> usize {
        (DC + (hz / BIN_HZ).round() as isize).clamp(0, FINDER_N as isize - 1) as usize
    }

    fn sum(&self, centre: usize, half: usize) -> f32 {
        let lo = centre.saturating_sub(half);
        let hi = (centre + half + 1).min(FINDER_N);
        self.avg[lo..hi].iter().sum()
    }

    fn analyse(&mut self) {
        for ((w, &b), &h) in self.work.iter_mut().zip(&self.buf).zip(&self.window) {
            *w = b * h;
        }
        self.fft.process(&mut self.work);
        let a = if self.frames == 0 { 1.0 } else { 0.25 };
        let scale = 1.0 / (FINDER_N as f32 * FINDER_N as f32);
        for (i, v) in self.avg.iter_mut().enumerate() {
            let p = self.work[(i + FINDER_N / 2) % FINDER_N].norm_sqr() * scale;
            *v += a * (p - *v);
        }
        self.frames += 1;
        // Noise per bin: the median over -3 .. +3 kHz.
        let k3 = (3000.0 / BIN_HZ) as usize;
        let span = &self.avg[FINDER_N / 2 - k3..=FINDER_N / 2 + k3];
        let noise = percentile(span, 0.5, &mut self.scratch).max(1e-20);
        // The pair: boxes of +-17 bins at +-shift/2 around each candidate centre within +-600 Hz.
        let half = self.shift / 2.0;
        let mut best = (0.0f64, 0.0f32);
        let step = (600.0 / BIN_HZ) as isize;
        for k in -step..=step {
            let c = k as f64 * BIN_HZ;
            let s = self.sum(self.bin(c - half), 17) + self.sum(self.bin(c + half), 17);
            if s > best.1 {
                best = (c, s);
            }
        }
        let level_db = 10.0 * f64::from(best.1 / 70.0 / noise).max(1e-12).log10();
        let found = level_db > 5.0;
        let lo = self.place(best.0 - half, noise);
        let hi = self.place(best.0 + half, noise);
        // SNR: power from the low box to the high box above the noise, against the noise in 3 kHz (Hann noise bandwidth 1.5 bins).
        let (k_lo, k_hi) = (
            self.bin(lo).saturating_sub(17),
            (self.bin(hi) + 17).min(FINDER_N - 1),
        );
        let p: f64 = self.avg[k_lo..=k_hi]
            .iter()
            .map(|v| f64::from(v - noise).max(0.0))
            .sum();
        let n3k = f64::from(noise) * 3000.0 / BIN_HZ;
        let snr_db = 10.0 * (p / 1.5 / n3k.max(1e-30)).max(1e-12).log10();
        self.seen = FskSeen {
            found,
            centre_hz: if found { (lo + hi) / 2.0 } else { best.0 },
            low_hz: lo,
            high_hz: hi,
            shift_hz: hi - lo,
            level_db,
            snr_db,
        };
    }

    /// A tone place near `hz`: the largest 5-bin-smoothed bin within +-40 Hz, refined by the centroid of the power above the noise
    /// within +-12 Hz of it.
    fn place(&self, hz: f64, noise: f32) -> f64 {
        let k0 = self.bin(hz);
        let r = (40.0 / BIN_HZ).round() as usize;
        let mut bk = (k0, f32::MIN);
        for k in k0.saturating_sub(r).max(2)..=(k0 + r).min(FINDER_N - 3) {
            let sm = self.sum(k, 2);
            if sm > bk.1 {
                bk = (k, sm);
            }
        }
        let w = (12.0 / BIN_HZ).round() as usize;
        let k =
            centroid(&self.avg, bk.0.saturating_sub(w), bk.0 + w + 1, noise).unwrap_or(bk.0 as f64);
        (k - DC as f64) * BIN_HZ
    }

    /// The audio spectrum: [`AUDIO_BINS`] bins from -500 to +3500 Hz (the largest FFT bin inside each), as (Hz, dB) points.
    pub fn audio_spectrum(&self, out: &mut Vec<[f32; 2]>) {
        out.clear();
        for i in 0..AUDIO_BINS {
            let f0 = -500.0 + 4000.0 * i as f64 / AUDIO_BINS as f64;
            let f1 = -500.0 + 4000.0 * (i + 1) as f64 / AUDIO_BINS as f64;
            let k0 = (DC + (f0 / BIN_HZ).floor() as isize).max(0) as usize;
            let k1 = ((DC + (f1 / BIN_HZ).ceil() as isize).max(0) as usize).min(FINDER_N);
            let m = self.avg[k0.min(k1)..k1]
                .iter()
                .copied()
                .fold(0.0f32, f32::max);
            out.push([f0 as f32, 10.0 * m.max(1e-20).log10()]);
        }
    }

    /// Starts again.
    pub fn reset(&mut self) {
        self.avg.iter_mut().for_each(|v| *v = 0.0);
        self.fill = 0;
        self.frames = 0;
        self.seen = FskSeen::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn the_slicer_recovers_bits_with_a_clock_error() {
        let rate = CHANNEL_RATE;
        let baud = 100.0 * (1.0 + 300e-6);
        let mut rng = Rng::new(3);
        let bits: Vec<u8> = (0..400).map(|_| (rng.next_u64() & 1) as u8).collect();
        let mut x = Vec::new();
        let mut ph = 0.0f64;
        let spb = rate / baud;
        let n = (bits.len() as f64 * spb) as usize;
        for i in 0..n {
            let b = bits[((i as f64) / spb) as usize];
            let f = if b == 1 { 85.0 } else { -85.0 } + 30.0;
            ph += TAU * f / rate;
            x.push(Complex32::new(ph.cos() as f32, ph.sin() as f32));
        }
        let mut s = MfSlicer::new(30.0, 170.0, 100.0).unwrap();
        let mut out = Vec::new();
        for c in x.chunks(777) {
            s.process(c, &mut out);
        }
        let hard: Vec<u8> = out.iter().map(|&d| u8::from(d > 0.0)).collect();
        let mut best = usize::MAX;
        for lag in 0..6 {
            let e = (50..350)
                .filter(|&k| hard.get(k + lag).copied() != Some(bits[k]))
                .count();
            best = best.min(e);
        }
        assert!(best <= 2, "{best} errors");
        assert!((s.symbol_rate() - baud).abs() < 1.0, "{}", s.symbol_rate());
    }

    #[test]
    fn the_finder_finds_a_pair() {
        let mut f = FskFinder::new(170.0);
        let mut x = Vec::new();
        let mut rng = Rng::new(2);
        let mut ph = 0.0f64;
        let bits: Vec<bool> = (0..200).map(|_| rng.next_u64() & 1 == 1).collect();
        for i in 0..40_000 {
            let b = bits[i / 240];
            let fr = 120.0 + if b { 85.0 } else { -85.0 };
            ph += TAU * fr / CHANNEL_RATE;
            x.push(Complex32::new(
                ph.cos() as f32 * 0.1 + (rng.normal() * 0.01) as f32,
                ph.sin() as f32 * 0.1 + (rng.normal() * 0.01) as f32,
            ));
        }
        f.process(&x);
        assert!(f.seen.found);
        assert!((f.seen.centre_hz - 120.0).abs() < 10.0, "{:?}", f.seen);
        assert!((f.seen.shift_hz - 170.0).abs() < 10.0);
        let mut sp = Vec::new();
        f.audio_spectrum(&mut sp);
        assert_eq!(sp.len(), AUDIO_BINS);
    }

    /// A steady real tone through the VHF AFSK slicer produces a steady soft value: the slicer sees no image of the tone at
    /// twice its frequency.
    #[test]
    fn afsk_soft_values_do_not_ripple_on_a_steady_tone() {
        for (f, sign) in [(AFSK_LOW_HZ, -1.0f32), (AFSK_HIGH_HZ, 1.0)] {
            let x: Vec<f32> = (0..24_000)
                .map(|n| (TAU * f * n as f64 / CHANNEL_RATE).cos() as f32)
                .collect();
            let mut s = AfskSlicer::default();
            let mut out = Vec::new();
            s.process(&x, &mut out);
            let v = &out[100..];
            let lo = v.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let m = v.iter().map(|x| x.abs()).sum::<f32>() / v.len() as f32;
            assert!(hi - lo <= 0.02 * m, "{f}: {lo} {hi} {m}");
            assert!(v.iter().all(|x| x.signum() == sign), "{f}");
        }
    }
}
