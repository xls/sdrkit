//! Synthetic signals from published parameters, for the evaluation set and the tests: PSK and QAM, two- and four-level FSK with
//! frame syncs, bursts, OFDM with a cyclic prefix, chirps, AM and FM.

use crate::psd::rrc;
use crate::signature::FrameSync;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// A small deterministic random source (xorshift).
#[derive(Clone, Debug)]
pub struct Rand(u64);

impl Rand {
    /// Seeded.
    pub fn new(seed: u64) -> Self {
        Rand(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniform in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// A standard normal value.
    pub fn gauss(&mut self) -> f64 {
        let u1 = self.uniform().max(1e-300);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
    /// An integer below `n`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
}

/// Adds complex white noise so the signal (of power `signal_power`) has `snr_db` in `bandwidth_hz`.
pub fn add_noise(
    x: &mut [Complex32],
    signal_power: f64,
    snr_db: f64,
    bandwidth_hz: f64,
    rate: f64,
    seed: u64,
) {
    let n0 = signal_power / 10f64.powf(snr_db / 10.0) / bandwidth_hz;
    let sigma = (n0 * rate / 2.0).sqrt();
    let mut r = Rand::new(seed ^ 0xABCD);
    for s in x {
        *s += Complex32::new((sigma * r.gauss()) as f32, (sigma * r.gauss()) as f32);
    }
}

/// Moves `x` by `hz`.
pub fn shift(x: &mut [Complex32], hz: f64, rate: f64) {
    for (i, s) in x.iter_mut().enumerate() {
        let p = TAU * hz * i as f64 / rate;
        *s *= Complex32::new(p.cos() as f32, p.sin() as f32);
    }
}

/// Mean power.
pub fn power(x: &[Complex32]) -> f64 {
    x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len().max(1) as f64
}

fn shape(symbols: &[Complex32], sps: f64, beta: f64, n: usize) -> Vec<Complex32> {
    // Upsample by linear placement at fractional positions, then filter with an RRC at the output rate.
    let h = rrc(sps, beta, 10);
    let half = h.len() / 2;
    let mut out = vec![Complex32::default(); n];
    for (k, s) in symbols.iter().enumerate() {
        let t = k as f64 * sps;
        let c = t.round() as i64;
        for (j, hv) in h.iter().enumerate() {
            let idx = c + j as i64 - half as i64;
            if idx >= 0 && (idx as usize) < n {
                out[idx as usize] += *s * (*hv * sps as f32);
            }
        }
    }
    out
}

/// PSK or square QAM with a root-raised-cosine pulse: `order` 2, 4, 8 (PSK) or 16, 64 (QAM); `pi4` makes π/4-DQPSK.
pub fn psk(
    rate: f64,
    rs: f64,
    order: u32,
    beta: f64,
    n: usize,
    seed: u64,
    pi4: bool,
) -> Vec<Complex32> {
    let sps = rate / rs;
    let count = (n as f64 / sps).ceil() as usize + 1;
    let mut r = Rand::new(seed);
    let mut phase = 0.0f64;
    let symbols: Vec<Complex32> = (0..count)
        .map(|k| {
            if pi4 {
                let d = [1.0, 3.0, -1.0, -3.0][r.below(4) as usize] * std::f64::consts::FRAC_PI_4;
                phase += d;
                let _ = k;
                Complex32::new(phase.cos() as f32, phase.sin() as f32)
            } else if order <= 8 {
                let p = TAU * r.below(order as u64) as f64 / order as f64
                    + if order == 4 { TAU / 8.0 } else { 0.0 };
                Complex32::new(p.cos() as f32, p.sin() as f32)
            } else {
                let m = (order as f64).sqrt() as u64;
                let lv = |v: u64| (2.0 * v as f64 - (m as f64 - 1.0)) / (m as f64 - 1.0);
                Complex32::new(lv(r.below(m)) as f32, lv(r.below(m)) as f32)
            }
        })
        .collect();
    shape(&symbols, sps, beta, n)
}

/// FSK: symbols drawn from `levels` (in units of `dev_unit_hz`, for example ±1 ±3 with 648 Hz), shaped by a raised-cosine-like
/// frequency pulse (a moving average over one symbol of the root-raised-cosine filtered levels), with `sync` inserted every
/// `sync_every` symbols when given, and on only during `on` (burst length, period) when given.
#[allow(clippy::too_many_arguments)]
pub fn fsk(
    rate: f64,
    rs: f64,
    levels: &[i8],
    dev_unit_hz: f64,
    n: usize,
    seed: u64,
    sync: Option<(&FrameSync, usize)>,
    on: Option<(f64, f64)>,
) -> Vec<Complex32> {
    let sps = rate / rs;
    let count = (n as f64 / sps).ceil() as usize + 1;
    let mut r = Rand::new(seed);
    let mut sym: Vec<f64> = (0..count)
        .map(|_| levels[r.below(levels.len() as u64) as usize] as f64)
        .collect();
    if let Some((s, every)) = sync {
        let mut k = 7;
        while k + s.symbols.len() < count {
            for (j, v) in s.symbols.iter().enumerate() {
                sym[k + j] = *v as f64;
            }
            k += every.max(s.symbols.len() + 1);
        }
    }
    let freq = shape(
        &sym.iter()
            .map(|v| Complex32::new(*v as f32, 0.0))
            .collect::<Vec<_>>(),
        sps,
        0.2,
        n,
    );
    let mut ph = 0.0f64;
    let mut out = Vec::with_capacity(n);
    for (i, fv) in freq.iter().enumerate() {
        ph += TAU * fv.re as f64 * dev_unit_hz / rate;
        let t = i as f64 / rate;
        let a = match on {
            Some((len, period)) => {
                if (t % period) < len {
                    1.0
                } else {
                    0.0
                }
            }
            None => 1.0,
        };
        out.push(Complex32::new((a * ph.cos()) as f32, (a * ph.sin()) as f32));
    }
    out
}

/// GMSK with BT 0.3 (approximated by a Gaussian frequency pulse), NRZI-free: random bits at `rs`.
pub fn gmsk(rate: f64, rs: f64, n: usize, seed: u64) -> Vec<Complex32> {
    let sps = rate / rs;
    let mut r = Rand::new(seed);
    let count = (n as f64 / sps).ceil() as usize + 2;
    let bits: Vec<f64> = (0..count)
        .map(|_| if r.below(2) == 1 { 1.0 } else { -1.0 })
        .collect();
    // Gaussian pulse of BT 0.3 convolved with a rectangle of one symbol.
    let bt = 0.3;
    let sigma = (2f64.ln()).sqrt() / (TAU * bt) * sps;
    let span = (3.0 * sps) as i64;
    let g: Vec<f64> = (-span..=span)
        .map(|k| (-(k as f64).powi(2) / (2.0 * sigma * sigma)).exp())
        .collect();
    let gs: f64 = g.iter().sum();
    let mut nrz = vec![0f64; n];
    for (i, v) in nrz.iter_mut().enumerate() {
        *v = bits[(i as f64 / sps) as usize];
    }
    let mut ph = 0.0f64;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut acc = 0.0;
        for (j, gv) in g.iter().enumerate() {
            let idx = i as i64 + j as i64 - span;
            if idx >= 0 && (idx as usize) < n {
                acc += nrz[idx as usize] * gv;
            }
        }
        let f = acc / gs * rs / 4.0;
        ph += TAU * f / rate;
        out.push(Complex32::new(ph.cos() as f32, ph.sin() as f32));
    }
    out
}

/// OFDM with `nfft` carriers at `rate` (one sample a carrier spacing), `used` carriers around the centre with random QPSK, and a
/// cyclic prefix of `guard` samples.
pub fn ofdm(nfft: usize, used: usize, guard: usize, symbols: usize, seed: u64) -> Vec<Complex32> {
    let mut r = Rand::new(seed);
    let mut fft = sdrkit_algo_dsp::fft::Fft::inverse(nfft);
    let mut out = Vec::with_capacity(symbols * (nfft + guard));
    for _ in 0..symbols {
        let mut buf = vec![Complex32::default(); nfft];
        for k in 0..used {
            let bin = (k as i64 - used as i64 / 2).rem_euclid(nfft as i64) as usize;
            if bin == 0 {
                continue;
            }
            let p = TAU * (r.below(4) as f64 + 0.5) / 4.0;
            buf[bin] = Complex32::new(p.cos() as f32, p.sin() as f32);
        }
        fft.process(&mut buf);
        let s = 1.0 / (used as f32).sqrt();
        for b in &mut buf {
            *b *= s;
        }
        out.extend_from_slice(&buf[nfft - guard..]);
        out.extend_from_slice(&buf);
    }
    out
}

/// LoRa-style up-chirps: spreading factor `sf`, bandwidth `bw`, at `rate`, random symbols.
pub fn chirps(rate: f64, bw: f64, sf: u8, n: usize, seed: u64) -> Vec<Complex32> {
    let m = 1usize << sf;
    let ts = m as f64 / bw;
    let sps = ts * rate;
    let mut r = Rand::new(seed);
    let mut out = Vec::with_capacity(n);
    let mut ph = 0.0f64;
    let mut sym = r.below(m as u64) as f64;
    let mut next = sps;
    for i in 0..n {
        if i as f64 >= next {
            sym = r.below(m as u64) as f64;
            next += sps;
        }
        let t = (i as f64 - (next - sps)) / rate;
        let mut f = -bw / 2.0 + bw * (sym / m as f64) + bw / ts * t;
        if f > bw / 2.0 {
            f -= bw;
        }
        ph += TAU * f / rate;
        out.push(Complex32::new(ph.cos() as f32, ph.sin() as f32));
    }
    out
}

/// AM by a tone.
pub fn am(rate: f64, tone_hz: f64, depth: f64, n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let e = 1.0 + depth * (TAU * tone_hz * i as f64 / rate).sin();
            Complex32::new(e as f32, 0.0)
        })
        .collect()
}

/// FM by a tone.
pub fn fm(rate: f64, tone_hz: f64, dev_hz: f64, n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let t = i as f64 / rate;
            let p = dev_hz / tone_hz * (TAU * tone_hz * t).sin();
            Complex32::new(p.cos() as f32, p.sin() as f32)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signals_have_power() {
        assert!(power(&psk(48e3, 4800.0, 4, 0.35, 4800, 1, false)) > 0.1);
        assert!((power(&fm(48e3, 1e3, 3e3, 1000)) - 1.0).abs() < 1e-3);
        assert!(power(&ofdm(256, 200, 32, 4, 1)) > 0.5);
        assert!((power(&chirps(500e3, 125e3, 9, 10_000, 1)) - 1.0).abs() < 1e-3);
        assert!((power(&gmsk(96e3, 9600.0, 10_000, 1)) - 1.0).abs() < 1e-3);
    }
}
