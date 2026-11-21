//! The beacon's signal chain: the line finder, the mixer and decimator to the work rate, the carrier PLL (its phase error is the
//! phase modulation), the subcarrier finder, the Costas loop, the symbol clock and the bit streams (NRZ and bi-phase).

use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_modem::carrier::ToneLoop;
use sdrkit_core::Complex32;
use std::f64::consts::PI;

/// A narrow line of the RF spectrum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Line {
    /// Offset from the centre, Hz.
    pub offset_hz: f64,
    /// Level, dBFS (of the line's amplitude).
    pub level_dbfs: f64,
    /// Over the noise in its bin, dB.
    pub over_noise_db: f64,
}

/// Finds the narrow lines of `x` (Hann FFT of its length, a power of two); returns lines strongest first and the noise density
/// (dBFS/Hz).
pub fn find_lines(x: &[Complex32], fs: f64, max: usize) -> (Vec<Line>, f64, Vec<f32>) {
    let n = x.len();
    let mut buf: Vec<Complex32> = x
        .iter()
        .enumerate()
        .map(|(i, &v)| v * (0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()) as f32)
        .collect();
    Fft::forward(n).process(&mut buf);
    // Power, scaled so a tone of amplitude a reads a^2 (Hann coherent gain 0.5).
    let scale = 1.0 / (0.5 * n as f64).powi(2);
    let p: Vec<f64> = buf.iter().map(|v| v.norm_sqr() as f64 * scale).collect();
    let mut sorted = p.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median = sorted[n / 2];
    // Noise per bin of the Hann window (ENBW 1.5 bins); the median of an exponential is ln 2 of its mean.
    let noise_bin = median / std::f64::consts::LN_2;
    let bin = fs / n as f64;
    let noise_density = sdrkit_algo_dsp::meter::power_to_db(noise_bin * n as f64 / (1.5 * fs));
    let mut lines = Vec::new();
    for k in 0..n {
        let v = p[k];
        if v < 30.0 * noise_bin || v < p[(k + n - 1) % n] || v < p[(k + 1) % n] {
            continue;
        }
        // Narrow: well above the median of +-64 bins.
        let (ym, yp) = (
            p[(k + n - 1) % n].max(1e-30).ln(),
            p[(k + 1) % n].max(1e-30).ln(),
        );
        let y0 = v.ln();
        let den = ym - 2.0 * y0 + yp;
        let d = if den.abs() > 1e-12 {
            0.5 * (ym - yp) / den
        } else {
            0.0
        };
        let kk = if k >= n / 2 {
            k as f64 - n as f64
        } else {
            k as f64
        } + d.clamp(-0.5, 0.5);
        // Level: the three bins around the peak sum to the tone's power (Hann).
        let pw = p[(k + n - 1) % n] + v + p[(k + 1) % n];
        lines.push(Line {
            offset_hz: kk * bin,
            level_dbfs: sdrkit_algo_dsp::meter::power_to_db(pw / 1.5),
            over_noise_db: sdrkit_algo_dsp::meter::power_to_db(v / noise_bin),
        });
    }
    lines.sort_by(|a, b| b.level_dbfs.total_cmp(&a.level_dbfs));
    // Merge lines closer than 3 bins.
    let mut out: Vec<Line> = Vec::new();
    for l in lines {
        if !out
            .iter()
            .any(|o| (o.offset_hz - l.offset_hz).abs() < 3.0 * bin)
        {
            out.push(l);
        }
        if out.len() >= max {
            break;
        }
    }
    // RF spectrum picture: 256 points, dB over noise.
    let mut pic = Vec::with_capacity(256);
    let step = n / 256;
    for j in 0..256 {
        let k0 = (j * step + n / 2) % n;
        let m = (0..step).map(|i| p[(k0 + i) % n]).fold(0.0, f64::max);
        pic.push(sdrkit_algo_dsp::meter::power_to_db(m / noise_bin) as f32);
    }
    (out, noise_density, pic)
}

/// The carrier loop: mixer, decimation to the work rate, and a second-order PLL (`sdrkit_algo_modem::carrier::ToneLoop`) whose phase
/// error is the phase modulation.
#[derive(Clone, Debug)]
pub struct CarrierPll {
    fs: f64,
    /// Decimation.
    pub dec: usize,
    acc: Complex32,
    nacc: usize,
    nco: Nco,
    mixed: Vec<Complex32>,
    /// The mixer frequency, Hz.
    pub mix_hz: f64,
    pll: Option<ToneLoop>,
    /// Lock indicator (smoothed cosine of the error).
    pub lock: f64,
    /// Mean square of the phase modulation, radians squared (smoothed).
    pub pm_rms: f64,
}

impl CarrierPll {
    /// For input at `fs`, the carrier at `hz`, loop noise bandwidth `bn` Hz. If the rates do not suit a loop, the loop is off
    /// and [`CarrierPll::process`] produces no output.
    pub fn new(fs: f64, hz: f64, bn: f64) -> CarrierPll {
        let dec = ((fs / 200e3).floor() as usize).max(1);
        let fw = fs / dec as f64;
        CarrierPll {
            fs,
            dec,
            acc: Complex32::new(0.0, 0.0),
            nacc: 0,
            nco: Nco::new(fs.max(1.0), -hz),
            mixed: Vec::new(),
            mix_hz: hz,
            pll: ToneLoop::new(fw, 0.0, bn, 0.05 * fw, 20.0 * bn).ok(),
            lock: 0.0,
            pm_rms: 0.0,
        }
    }
    /// The work rate.
    pub fn work_rate(&self) -> f64 {
        self.fs / self.dec as f64
    }
    /// The carrier frequency now, Hz.
    pub fn freq_hz(&self) -> f64 {
        self.mix_hz + self.pll.as_ref().map_or(0.0, |p| p.freq_hz())
    }
    /// Runs samples; appends the phase modulation (radians) at the work rate.
    pub fn process(&mut self, x: &[Complex32], pm: &mut Vec<f32>) {
        let Some(pll) = self.pll.as_mut() else {
            return;
        };
        self.mixed.clear();
        self.mixed.resize(x.len(), Complex32::new(0.0, 0.0));
        self.nco.mix(x, &mut self.mixed);
        for &m in &self.mixed {
            self.acc += m;
            self.nacc += 1;
            if self.nacc < self.dec {
                continue;
            }
            let v = self.acc;
            self.acc = Complex32::new(0.0, 0.0);
            self.nacc = 0;
            if !(v.re.is_finite() && v.im.is_finite()) {
                continue;
            }
            let y = v * pll.push(v).conj();
            let n = y.norm();
            if n <= 0.0 {
                continue;
            }
            let e = approx::atan2(y.im, y.re);
            self.lock += 1e-4 * ((y.re / n) as f64 - self.lock);
            self.pm_rms += 1e-4 * ((e * e) as f64 - self.pm_rms);
            pm.push(e);
        }
    }
}

/// The subcarrier centre of a phase-modulation spectrum (`s` linear, bins 0 to rate/2): the centre of the strongest symmetric pair
/// of humps (or a single hump), with the number of humps and their spacing.
pub fn subcarrier(s: &[f64], rate: f64) -> Option<(f64, u32, f64, f64)> {
    let n = s.len();
    let bin = rate / 2.0 / n as f64;
    let mut sm = Vec::new();
    sdrkit_algo_dsp::stats::centred_mean(s, 2, &mut sm);
    let floor = sdrkit_proto_dvbs2::spectrum::quantile(&sm, 0.3);
    let k_max = (12_000.0 / bin) as usize;
    let lo = ((2000.0 / bin) as usize).max(k_max + 1);
    let mut best = (0usize, 0f64);
    for c in lo..n.saturating_sub(k_max + 1) {
        let mut v = 0.0;
        for k in 1..k_max {
            v += sm[c - k].min(sm[c + k]);
        }
        v += sm[c];
        if v > best.1 {
            best = (c, v);
        }
    }
    if best.1 <= 0.0 {
        return None;
    }
    let c = best.0;
    // Humps: the strongest bins either side.
    let left = (c.saturating_sub(k_max)..c).max_by(|&a, &b| sm[a].total_cmp(&sm[b]))?;
    let right = (c + 1..(c + k_max).min(n)).max_by(|&a, &b| sm[a].total_cmp(&sm[b]))?;
    let two = sm[left] > 2.0 * sm[c] && sm[right] > 2.0 * sm[c];
    let peak = sm[left].max(sm[right]).max(sm[c]);
    if peak < 10.0 * floor {
        return None;
    }
    let snr = sdrkit_algo_dsp::meter::power_to_db(peak / floor);
    let spacing = (right - left) as f64 * bin;
    // Centre: midpoint of the humps when two, else the peak.
    let centre = if two {
        0.5 * (left + right) as f64 * bin
    } else {
        c as f64 * bin
    };
    Some((
        centre,
        if two { 2 } else { 1 },
        if two { spacing } else { 0.0 },
        snr,
    ))
}

/// The usual bit rates the symbol clock snaps to (within 1.5 %).
pub const ROUND_RATES: [f64; 16] = [
    100.0, 150.0, 200.0, 250.0, 300.0, 500.0, 600.0, 1000.0, 1200.0, 2000.0, 2400.0, 4000.0,
    4800.0, 8000.0, 9600.0, 19200.0,
];

/// The subcarrier demodulator: mixer, filter, Costas loop, chip clock (a transition-tracking loop) and chip decisions.
#[derive(Clone, Debug)]
pub struct SubDemod {
    rate: f64,
    sc_phase: f64,
    sc_step: f64,
    costas_th: f64,
    costas_w: f64,
    lp: Vec<f32>,
    lpos: usize,
    lsum: Complex32,
    lbuf: Vec<Complex32>,
    l2sum: Complex32,
    l2buf: Vec<Complex32>,
    /// Chip rate, Hz (0 until known).
    pub chip_rate: f64,
    clk: f64,
    clk_step: f64,
    acc: f32,
    acc_n: u32,
    prev: f32,
    /// Decided chips (+1 / -1 as 1 / 0).
    pub chips: Vec<u8>,
    /// Baseband for the clock search.
    pub base: Vec<f32>,
    /// The clock has followed transitions (smoothed agreement).
    pub clock_lock: f64,
}

impl SubDemod {
    /// At work rate `rate`, subcarrier `fsc`, filter about `bw` Hz wide.
    pub fn new(rate: f64, fsc: f64, bw: f64) -> SubDemod {
        let len = ((rate / bw.max(1000.0)) as usize).clamp(2, 512);
        SubDemod {
            rate,
            sc_phase: 0.0,
            sc_step: 2.0 * PI * fsc / rate,
            costas_th: 0.0,
            costas_w: 0.0,
            lp: Vec::new(),
            lpos: 0,
            lsum: Complex32::new(0.0, 0.0),
            lbuf: vec![Complex32::new(0.0, 0.0); len],
            l2sum: Complex32::new(0.0, 0.0),
            l2buf: vec![Complex32::new(0.0, 0.0); len.div_ceil(2)],
            chip_rate: 0.0,
            clk: 0.0,
            clk_step: 0.0,
            acc: 0.0,
            acc_n: 0,
            prev: 0.0,
            chips: Vec::new(),
            base: Vec::new(),
            clock_lock: 0.0,
        }
    }
    /// The subcarrier frequency the Costas loop follows.
    pub fn subcarrier_hz(&self) -> f64 {
        (self.sc_step + self.costas_w) * self.rate / (2.0 * PI)
    }
    /// Sets the chip rate.
    pub fn set_chip_rate(&mut self, r: f64) {
        self.chip_rate = r;
        self.clk_step = r / self.rate;
    }
    /// Runs phase-modulation samples.
    pub fn process(&mut self, pm: &[f32]) {
        let _ = &self.lp;
        let n = self.lbuf.len();
        for &v in pm {
            let ph = self.sc_phase + self.costas_th;
            let (s, c) = approx::sin_cos(ph);
            let m = Complex32::new(v * c, -v * s);
            self.sc_phase += self.sc_step;
            if self.sc_phase > PI {
                self.sc_phase -= 2.0 * PI;
            }
            self.lsum += m - self.lbuf[self.lpos];
            self.lbuf[self.lpos] = m;
            self.lpos = (self.lpos + 1) % n;
            let y1 = self.lsum / n as f32;
            let n2 = self.l2buf.len();
            let p2 = self.lpos % n2;
            self.l2sum += y1 - self.l2buf[p2];
            self.l2buf[p2] = y1;
            let y = self.l2sum / n2 as f32;
            // Costas (BPSK): the data is on one axis.
            let e = (y.re * y.im) as f64 / ((y.norm_sqr() as f64) + 1e-9);
            self.costas_w += 2e-7 * e;
            self.costas_th += self.costas_w + 2e-3 * e;
            let b = y.re;
            self.base.push(b);
            if self.base.len() > (self.rate * 2.0) as usize {
                self.base.drain(..self.base.len() / 2);
            }
            if self.chip_rate <= 0.0 {
                continue;
            }
            // Chip clock: phase 0 at chip boundaries; a transition pulls the phase toward 0.
            let old = self.clk;
            self.clk += self.clk_step;
            if (b > 0.0) != (self.prev > 0.0) {
                let err = if self.clk > 0.5 {
                    self.clk - 1.0
                } else {
                    self.clk
                };
                self.clk -= 0.05 * err;
                self.clk_step -= 0.0005 * err * self.clk_step;
                self.clock_lock += 0.01 * ((1.0 - 4.0 * err.abs()) - self.clock_lock);
            }
            self.prev = b;
            // The middle half of the chip.
            if (0.25..0.75).contains(&self.clk) {
                self.acc += b;
                self.acc_n += 1;
            }
            if self.clk >= 1.0 {
                self.clk -= 1.0;
                self.chips.push((self.acc > 0.0) as u8);
                self.acc = 0.0;
                self.acc_n = 0;
            }
            let _ = old;
        }
    }
    /// The chip clock's rate now.
    pub fn clock_rate(&self) -> f64 {
        self.clk_step * self.rate
    }
}

/// The chip rate of a baseband: the strongest line of its edge energy between 50 Hz and a quarter of the rate, snapped to a round
/// rate (or twice one) within 1.5 %.
pub fn chip_rate(base: &[f32], rate: f64) -> Option<f64> {
    let n = base.len().next_power_of_two() / 2;
    if n < 4096 {
        return None;
    }
    let x = &base[base.len() - n..];
    let mut buf: Vec<Complex32> = (1..n)
        .map(|i| Complex32::new((x[i] - x[i - 1]).powi(2), 0.0))
        .collect();
    buf.push(Complex32::new(0.0, 0.0));
    let mean = buf.iter().map(|v| v.re).sum::<f32>() / n as f32;
    buf.iter_mut().for_each(|v| v.re -= mean);
    Fft::forward(n).process(&mut buf);
    let bin = rate / n as f64;
    let lo = (50.0 / bin) as usize;
    let hi = ((25_000.0 / bin) as usize).min(n / 4);
    let k = (lo..hi).max_by(|&a, &b| buf[a].norm().total_cmp(&buf[b].norm()))?;
    let f = k as f64 * bin;
    for r in ROUND_RATES {
        for m in [1.0, 2.0] {
            if ((f - r * m) / (r * m)).abs() < 0.015 {
                return Some(r * m);
            }
        }
    }
    Some(f)
}

/// Bits from chips: NRZ (the chips themselves) and bi-phase (pairs of opposite chips, paired at the better offset). Returns
/// whether the stream is bi-phase.
pub fn biphase(chips: &[u8]) -> (bool, usize) {
    let mut best = (0usize, 0f64);
    for off in 0..2 {
        let pairs = (chips.len() - off) / 2;
        if pairs == 0 {
            continue;
        }
        let opp = (0..pairs)
            .filter(|&i| chips[off + 2 * i] != chips[off + 2 * i + 1])
            .count();
        let f = opp as f64 / pairs as f64;
        if f > best.1 {
            best = (off, f);
        }
    }
    (best.1 > 0.9, best.0)
}
