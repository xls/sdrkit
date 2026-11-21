//! The measurements of one signal: [`measure`] brings the signal to 0 Hz ([`Channel`]) and measures its spectrum, envelope,
//! bursts, symbol rate, FSK levels, OFDM symbol, chirps, FM pilot, tones and constellation, and decides a modulation class. Each
//! feature adds a [`Why`] row in plain words.

use crate::psd::{
    fir_filter, interp, interp_c, line_spectrum, median, median64, moving_average, pow2_floor,
    quantile, real_line_spectrum, rrc, strongest_line, top_lines, Psd,
};
use crate::regions::Region;
use crate::signature::ModClass;
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::kmeans::{kmeans_1d, Start};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::stats::{plain_kurtosis_less_noise, power_moments};
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// At most this many channel samples are analysed.
pub const MAX_CHANNEL_SAMPLES: usize = 1 << 20;

/// One signal at 0 Hz: what the verifiers work on.
#[derive(Clone, Debug)]
pub struct Channel {
    /// Baseband samples, the signal's centre at 0 Hz, filtered to its width.
    pub samples: Vec<Complex32>,
    /// Their rate.
    pub rate: f64,
    /// The noise power per sample left in them.
    pub noise_power: f64,
    /// Whether each sample is inside a burst.
    pub on: Vec<bool>,
}

/// Where the symbol rate was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateSource {
    /// No symbol rate.
    None,
    /// A line of |x|² (cyclostationarity of the envelope).
    Envelope,
    /// A line of the FM discriminator's transitions.
    Discriminator,
    /// The spacing of the two lines of x² (MSK, GMSK).
    PowerLines,
    /// The chirp length.
    Chirp,
    /// The OFDM symbol (useful part plus guard).
    OfdmSymbol,
}

/// The shape of the spectrum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// A single line.
    Line,
    /// Discrete tones.
    Tones,
    /// Flat across the band (OFDM, a sharp root-raised-cosine).
    FlatTop,
    /// Rounded.
    Rounded,
}

/// A line of (x/|x|)^n.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PowerLine {
    /// The power n.
    pub n: u32,
    /// Its strength over the median of the spectrum (dB).
    pub strength_db: f64,
    /// Its frequency divided by n: the carrier offset it implies (Hz).
    pub offset_hz: f64,
}

/// FSK levels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fsk {
    /// 2 or 4.
    pub levels: u8,
    /// The level frequencies relative to their middle (Hz), lowest first.
    pub deviations_hz: Vec<f64>,
    /// The outer deviation (Hz, half the distance between the outer levels).
    pub outer_dev_hz: f64,
    /// The inner deviation (Hz), for four levels.
    pub inner_dev_hz: Option<f64>,
    /// The rms scatter around the levels over the level spacing.
    pub scatter: f64,
    /// The middle of the levels (Hz from the measured centre): a carrier offset.
    pub centre_hz: f64,
}

/// OFDM parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ofdm {
    /// The useful symbol length (s).
    pub tu_s: f64,
    /// The guard length (s), when found.
    pub tg_s: Option<f64>,
    /// The normalised cyclic-prefix correlation |R(Tu)|/R(0).
    pub cp_corr: f64,
    /// That correlation over the noise of the other lags.
    pub z: f64,
    /// Carrier spacing 1/Tu (Hz).
    pub spacing_hz: f64,
    /// Guard over useful symbol.
    pub guard_fraction: Option<f64>,
}

/// A chirp.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chirp {
    /// The swept bandwidth (Hz).
    pub bw_hz: f64,
    /// One sweep (s).
    pub symbol_s: f64,
    /// The LoRa spreading factor, when 2^SF / BW fits the sweep.
    pub sf: Option<u8>,
    /// Up (+1) or down (−1).
    pub direction: i8,
}

/// The constellation at the symbol instants.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constellation {
    /// 2, 4, 8 (PSK) or 16, 64 (QAM); 0 when unclear.
    pub order: u32,
    /// QAM rather than PSK.
    pub qam: bool,
    /// The phase coherence |mean u^M| for M = 2, 4, 8 (carrier offset taken out).
    pub coherence: [f64; 3],
    /// The same for the symbol-to-symbol differences (π/4-DQPSK shows at M = 4 here only).
    pub diff_coherence: [f64; 3],
    /// The amplitude spread at the symbol instants (rms over mean of |s|).
    pub amplitude_spread: f64,
}

/// Tones in the spectrum.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tones {
    /// How many.
    pub count: usize,
    /// Their common spacing (Hz).
    pub spacing_hz: f64,
}

/// A row of the "why?" view: a feature, its value and why it was read so.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Why {
    /// The feature.
    pub feature: String,
    /// Its value, as text.
    pub value: String,
    /// Why, in plain words.
    pub why: String,
}

/// Everything measured on one signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Features {
    /// The middle of the 99 % band, Hz from the capture's centre.
    pub offset_hz: f64,
    /// The same as a radio frequency.
    pub rf_hz: f64,
    /// Width at −3 dB from the peak (Hz).
    pub width_3db_hz: f64,
    /// Width at −20 dB (Hz).
    pub width_20db_hz: f64,
    /// Width at −26 dB (Hz).
    pub width_26db_hz: f64,
    /// The band holding 99 % of the signal's power (Hz).
    pub width_99_hz: f64,
    /// Signal over noise in the 99 % band (dB).
    pub snr_db: f64,
    /// The signal's power (dBFS).
    pub power_dbfs: f64,
    /// The share of the −20 dB width that is within 3 dB of the peak: about 1 for a flat top.
    pub flat_top: f64,
    /// The share of time the signal is on.
    pub duty: f64,
    /// Bursts seen.
    pub bursts: usize,
    /// The median burst length (s).
    pub burst_s: Option<f64>,
    /// A regular burst period (s): a TDMA frame or slot.
    pub burst_period_s: Option<f64>,
    /// The capture length (s).
    pub capture_s: f64,
    /// E|s|⁴/(E|s|²)² with the noise taken out: 1 constant envelope, about 1.3 filtered PSK, 2 Gaussian or OFDM.
    pub kurtosis: f64,
    /// A carrier line at the centre over the rest of the signal's spectrum (dB).
    pub carrier_line_db: f64,
    /// The strongest lines of (x/|x|)^n, n = 1, 2, 4, 8.
    pub power_lines: [PowerLine; 4],
    /// The modulation class.
    pub class: ModClass,
    /// The symbol rate (Bd).
    pub symbol_rate: Option<f64>,
    /// Where it was read.
    pub rate_source: RateSource,
    /// The strength of the line it was read from (dB).
    pub rate_line_db: f64,
    /// FSK levels.
    pub fsk: Option<Fsk>,
    /// OFDM parameters.
    pub ofdm: Option<Ofdm>,
    /// A chirp.
    pub chirp: Option<Chirp>,
    /// A 19 kHz FM stereo pilot.
    pub fm_pilot: bool,
    /// The spectral shape.
    pub shape: Shape,
    /// The roll-off: 99 % width over the symbol rate, minus 1.
    pub rolloff: Option<f64>,
    /// Discrete tones.
    pub tones: Option<Tones>,
    /// The strongest periodicity across the band (spacing Hz, strength 0 to 1): OFDM pilots, an MFSK grid.
    pub periodicity: Option<(f64, f64)>,
    /// The constellation.
    pub constellation: Option<Constellation>,
    /// The modulation of an analogue FM signal repeats (a test tone): it is not data.
    pub periodic_modulation: bool,
    /// Why each feature was read so.
    pub why: Vec<Why>,
}

impl Features {
    fn why(&mut self, feature: &str, value: String, why: &str) {
        self.why.push(Why {
            feature: feature.into(),
            value,
            why: why.into(),
        });
    }

    /// The one-line summary: "4FSK, 4800 Bd, ±648 / ±1944 Hz, 9.6 kHz wide, continuous".
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = vec![self.class.word().to_string()];
        if let Some(c) = &self.chirp {
            parts.push(format!("{} kHz sweep", fmt_k(c.bw_hz)));
            if let Some(sf) = c.sf {
                parts.push(format!("SF{sf}"));
            }
        }
        if let Some(o) = self.ofdm.as_ref().filter(|_| self.class == ModClass::Ofdm) {
            parts.push(format!("Tu {:.1} µs", o.tu_s * 1e6));
            if let Some(g) = o.tg_s {
                parts.push(format!("Tg {:.1} µs", g * 1e6));
            }
        } else if let Some(r) = self.symbol_rate {
            parts.push(format!("{} Bd", fmt_rate(r)));
        }
        if let Some(f) = &self.fsk {
            match f.inner_dev_hz {
                Some(i) => parts.push(format!("±{:.0} / ±{:.0} Hz", i, f.outer_dev_hz)),
                None => parts.push(format!("±{:.0} Hz", f.outer_dev_hz)),
            }
        }
        if self.fm_pilot {
            parts.push("19 kHz pilot".into());
        }
        parts.push(format!("{} wide", fmt_width(self.width_99_hz)));
        parts.push(if self.duty >= 0.97 {
            "continuous".into()
        } else {
            match self.burst_period_s {
                Some(p) => format!("bursts every {:.1} ms", p * 1e3),
                None => "bursty".into(),
            }
        });
        parts.join(", ")
    }
}

/// A width in words: "12.5 kHz", "1.54 MHz", "800 Hz".
pub fn fmt_width(hz: f64) -> String {
    if hz >= 1e6 {
        format!("{:.2} MHz", hz / 1e6)
    } else if hz >= 1e3 {
        format!("{} kHz", fmt_k(hz))
    } else {
        format!("{:.0} Hz", hz)
    }
}

fn fmt_k(hz: f64) -> String {
    let k = hz / 1e3;
    if k >= 100.0 {
        format!("{k:.0}")
    } else {
        format!("{k:.1}")
    }
}

fn fmt_rate(r: f64) -> String {
    if r >= 1e6 {
        format!("{:.3} M", r / 1e6)
    } else {
        format!("{:.0}", r)
    }
}

/// Brings the region's signal to 0 Hz at about three times its width; returns the samples and their rate.
fn extract(x: &[Complex32], rate: f64, region: &Region, bin_hz: f64) -> (Vec<Complex32>, f64) {
    let w = region.width().max(bin_hz * 4.0);
    let c0 = region.centre();
    let bw = (w * 1.5 + 8.0 * bin_hz).min(rate);
    let min_rate = (bw * 2.0).max(4000.0);
    if min_rate * 1.25 >= rate * 0.75 {
        let mut nco = Nco::new(rate, -c0);
        let n = x.len().min(MAX_CHANNEL_SAMPLES);
        let mut out = vec![Complex32::default(); n];
        nco.mix(&x[..n], &mut out);
        return (out, rate);
    }
    match Channelizer::new(ChannelConfig {
        atten_db: 60.0,
        ..ChannelConfig::new(rate, c0, bw, min_rate)
    }) {
        Ok(mut ch) => {
            let r = ch.output_rate();
            let mut out = Vec::new();
            // Feed only what the analysis keeps.
            let need = ((MAX_CHANNEL_SAMPLES + 256) as f64 * rate / r).ceil() as usize;
            ch.process(&x[..x.len().min(need)], &mut out);
            let skip = 64.min(out.len());
            out.drain(..skip);
            out.truncate(MAX_CHANNEL_SAMPLES);
            (out, r)
        }
        Err(_) => {
            let mut nco = Nco::new(rate, -c0);
            let n = x.len().min(MAX_CHANNEL_SAMPLES);
            let mut out = vec![Complex32::default(); n];
            nco.mix(&x[..n], &mut out);
            (out, rate)
        }
    }
}

/// The instantaneous frequency (Hz) of `y`.
pub fn discriminator(y: &[Complex32], rate: f64) -> Vec<f32> {
    let k = rate / TAU;
    let mut out = Vec::with_capacity(y.len());
    let mut prev = y.first().copied().unwrap_or_default();
    for s in y {
        let d = *s * prev.conj();
        let a = if d.norm_sqr() > 0.0 {
            d.im.atan2(d.re) as f64
        } else {
            0.0
        };
        out.push((a * k) as f32);
        prev = *s;
    }
    out
}

/// 1-D k-means from the spread start, at most 30 rounds: the sorted centres and the rms distance to the nearest centre.
fn level_fit(v: &[f64], k: usize) -> (Vec<f64>, f64) {
    match kmeans_1d(v, k, Start::Spread, 30) {
        Some(c) => {
            let e = v
                .iter()
                .map(|&x| (x - c.centres[c.nearest(x)]).powi(2))
                .sum::<f64>();
            (c.centres, (e / v.len() as f64).sqrt())
        }
        None => (Vec::new(), 0.0),
    }
}

/// Samples `f` once a symbol at the phase where the values spread most (the eye's opening), only where `on`.
fn symbol_samples(f: &[f32], on: &[bool], sps: f64, max: usize) -> Vec<f64> {
    let mut best: (f64, Vec<f64>) = (-1.0, Vec::new());
    for phase in 0..8 {
        let mut v = Vec::new();
        let mut t = phase as f64 * sps / 8.0;
        while t < (f.len() - 1) as f64 && v.len() < max {
            if on[t as usize] {
                v.push(interp(f, t) as f64);
            }
            t += sps;
        }
        if v.len() < 16 {
            continue;
        }
        let m = v.iter().sum::<f64>() / v.len() as f64;
        let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64;
        if var > best.0 {
            best = (var, v);
        }
    }
    best.1
}

/// The FSK levels of the instantaneous frequency `f` sampled at `rs`, if they form two or four clear clusters.
fn fsk_levels(f: &[f32], on: &[bool], rate: f64, rs: f64) -> Option<Fsk> {
    let sps = rate / rs;
    if sps < 1.5 {
        return None;
    }
    let sm = moving_average(f, ((sps * 0.4).round() as usize).max(1));
    let v = symbol_samples(&sm, on, sps, 40_000);
    if v.len() < 64 {
        return None;
    }
    let (c2, e2) = level_fit(&v, 2);
    let gap2 = c2[1] - c2[0];
    if gap2 <= 0.0 {
        return None;
    }
    let r2 = e2 / gap2;
    let (c4, e4) = level_fit(&v, 4);
    let gaps: Vec<f64> = c4.windows(2).map(|w| w[1] - w[0]).collect();
    let mg = gaps.iter().sum::<f64>() / 3.0;
    let even = mg > 0.0 && gaps.iter().all(|g| (g / mg - 1.0).abs() < 0.45);
    let r4 = if mg > 0.0 { e4 / mg } else { 1.0 };
    // Four levels: evenly spaced clusters, tight, and two clusters leave a clear spread (levels ±1 ±3 give about 0.25).
    if even && r4 < 0.3 && r2 > 0.17 {
        let centre = 0.5 * (c4[0] + c4[3]);
        return Some(Fsk {
            levels: 4,
            deviations_hz: c4.iter().map(|c| c - centre).collect(),
            outer_dev_hz: 0.5 * (c4[3] - c4[0]),
            inner_dev_hz: Some(0.5 * (c4[2] - c4[1])),
            scatter: r4,
            centre_hz: centre,
        });
    }
    if r2 < 0.3 {
        let centre = 0.5 * (c2[0] + c2[1]);
        return Some(Fsk {
            levels: 2,
            deviations_hz: c2.iter().map(|c| c - centre).collect(),
            outer_dev_hz: 0.5 * gap2,
            inner_dev_hz: None,
            scatter: r2,
            centre_hz: centre,
        });
    }
    None
}

/// Measures the signal in `region` of the capture `x` (at `rate`, centred on `rf_hz`); `wide` is the capture's spectrum.
pub fn measure(
    x: &[Complex32],
    rate: f64,
    rf_hz: f64,
    region: &Region,
    wide: &Psd,
) -> (Features, Channel) {
    let capture_s = x.len() as f64 / rate;
    let floor_d = wide.floor().max(1e-30);
    let (ch, rc) = extract(x, rate, region, wide.bin_hz());
    let len = ch.len();

    // The channel's spectrum, about 200 bins across the region.
    let w_reg = region.width().max(wide.bin_hz() * 4.0);
    let target = (rc / (w_reg / 200.0)).max(16.0) as usize;
    let nfft = pow2_floor(target)
        .clamp(256, 16384)
        .min(pow2_floor(len / 4).max(64));
    // A bursty signal is measured inside its bursts (a coarse mask of the channel's power).
    let coarse = {
        let pw: Vec<f32> = ch.iter().map(|s| s.norm_sqr()).collect();
        let lw = ((4.0 * rc / w_reg) as usize).clamp(4, ((rc * 0.002) as usize).max(4));
        let ps = moving_average(&pw, lw);
        let n0 = floor_d * (w_reg * 1.5).min(rc);
        let hi = quantile(&ps, 0.99);
        let thr = (n0 * hi).sqrt().max(n0 * 2.0);
        let m: Vec<bool> = ps.iter().map(|v| (*v as f64) > thr).collect();
        let duty = m.iter().filter(|b| **b).count() as f64 / m.len().max(1) as f64;
        (duty < 0.8 && duty > 0.0).then_some(m)
    };
    let fine = Psd::welch_where(&ch, rc, nfft, 256, coarse.as_deref());
    let bh = fine.bin_hz();
    let margin = 0.25 * w_reg + 2.0 * bh;
    let edge = (fine.nfft / 50).max(1);
    let lo_b = (fine.bin_of(-w_reg / 2.0 - margin).floor().max(edge as f64)) as usize;
    let hi_b = (fine.bin_of(w_reg / 2.0 + margin).ceil() as usize).min(fine.nfft - 1 - edge);
    let d = &fine.density;
    let (mut peak, mut peak_b) = (0f64, lo_b);
    for (i, v) in d.iter().enumerate().take(hi_b + 1).skip(lo_b) {
        if (*v as f64) > peak {
            peak = *v as f64;
            peak_b = i;
        }
    }
    let floor = floor_d.max(peak * 1e-12);
    let s: Vec<f64> = (lo_b..=hi_b)
        .map(|i| (d[i] as f64 - floor).max(0.0))
        .collect();
    let total: f64 = s.iter().sum();
    let (mut i_lo, mut i_hi) = (peak_b - lo_b, peak_b - lo_b);
    if total > 0.0 {
        let mut c = 0.0;
        let mut lo_set = false;
        for (i, v) in s.iter().enumerate() {
            c += v;
            if !lo_set && c >= 0.005 * total {
                i_lo = i;
                lo_set = true;
            }
            if c >= 0.995 * total {
                i_hi = i;
                break;
            }
        }
    }
    let w99 = ((i_hi as f64 - i_lo as f64 + 1.0) * bh).max(bh);
    let centre_ch = fine.freq(lo_b as f64 + 0.5 * (i_lo + i_hi) as f64);
    let width_at = |x_db: f64| -> f64 {
        let thr = peak * 10f64.powf(-x_db / 10.0);
        let a = (lo_b..=hi_b)
            .find(|&i| d[i] as f64 >= thr)
            .unwrap_or(peak_b);
        let b = (lo_b..=hi_b)
            .rev()
            .find(|&i| d[i] as f64 >= thr)
            .unwrap_or(peak_b);
        (b as f64 - a as f64 + 1.0) * bh
    };
    let (w3, w20, w26) = (width_at(3.0), width_at(20.0), width_at(26.0));
    let sig_power = total * bh;
    let snr_db = power_to_db(sig_power / (floor * w99)).min(99.0);
    let power_dbfs = power_to_db(sig_power + floor * w99);
    let offset_hz = region.centre() + centre_ch;

    // Bring the 99 % band to 0 Hz and filter it.
    let mut y = vec![Complex32::default(); len];
    Nco::new(rc, -centre_ch).mix(&ch, &mut y);
    let pass = 0.5 * w99 * 1.15 + 2.0 * bh;
    let trans = (0.25 * pass + 2.0 * bh).max(rc / 100.0);
    let mut enbw = rc;
    if pass + trans < 0.475 * rc && len > 64 {
        let taps = sdrkit_algo_dsp::fir::lowpass(rc, pass, pass + trans, 50.0)
            .expect("the filter settings are valid");
        let sum: f64 = taps.iter().map(|t| *t as f64).sum();
        let sq: f64 = taps.iter().map(|t| (*t as f64) * (*t as f64)).sum();
        enbw = rc * sq / (sum * sum).max(1e-30);
        y = fir_filter(&y, &taps);
    }
    let noise = floor * enbw;

    let mut f = Features {
        offset_hz,
        rf_hz: rf_hz + offset_hz,
        width_3db_hz: w3,
        width_20db_hz: w20,
        width_26db_hz: w26,
        width_99_hz: w99,
        snr_db,
        power_dbfs,
        flat_top: (w3 / w20.max(bh)).min(1.0),
        duty: 0.0,
        bursts: 0,
        burst_s: None,
        burst_period_s: None,
        capture_s,
        kurtosis: 0.0,
        carrier_line_db: 0.0,
        power_lines: [PowerLine::default(); 4],
        class: ModClass::Unknown,
        symbol_rate: None,
        rate_source: RateSource::None,
        rate_line_db: 0.0,
        fsk: None,
        ofdm: None,
        chirp: None,
        fm_pilot: false,
        shape: Shape::Rounded,
        rolloff: None,
        tones: None,
        periodicity: None,
        constellation: None,
        periodic_modulation: false,
        why: Vec::new(),
    };
    f.why(
        "centre",
        format!("{:.3} MHz", f.rf_hz / 1e6),
        "the middle of the band holding 99 % of the power",
    );
    f.why(
        "width",
        format!(
            "99 %: {}, −3 dB: {}, −20 dB: {}, −26 dB: {}",
            fmt_width(w99),
            fmt_width(w3),
            fmt_width(w20),
            fmt_width(w26)
        ),
        "measured on the averaged spectrum against the noise floor",
    );
    f.why(
        "SNR",
        format!("{snr_db:.1} dB"),
        "signal power in the 99 % band over the noise floor in the same band",
    );

    let mut chan = Channel {
        samples: Vec::new(),
        rate: rc,
        noise_power: noise,
        on: Vec::new(),
    };
    if snr_db < 3.0 || len < 256 {
        f.class = ModClass::Noise;
        f.why(
            "class",
            "noise".into(),
            "nothing stands 3 dB over the noise floor",
        );
        chan.samples = y;
        chan.on = vec![false; len];
        return (f, chan);
    }

    // Envelope, bursts and duty.
    let p: Vec<f32> = y.iter().map(|s| s.norm_sqr()).collect();
    let lw = ((4.0 / w99 * rc).round() as usize).clamp(4, ((rc * 0.002) as usize).max(4));
    let ps = moving_average(&p, lw);
    let hi = quantile(&ps, 0.95);
    let thr = (noise * hi).sqrt().max(noise * 2.0);
    let mut on: Vec<bool> = ps.iter().map(|v| (*v as f64) > thr).collect();
    // Close gaps shorter than two windows.
    let merge = 2 * lw;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < len {
        if on[i] {
            let a = i;
            while i < len && on[i] {
                i += 1;
            }
            match runs.last_mut() {
                Some(last) if a - last.1 <= merge => last.1 = i,
                _ => runs.push((a, i)),
            }
        } else {
            i += 1;
        }
    }
    for r in &runs {
        for o in &mut on[r.0..r.1] {
            *o = true;
        }
    }
    // The bursts without their edges, where the level and the frequency are still settling.
    let mut core = on.clone();
    let em = ((rc / w99) * 8.0) as usize;
    for r in &runs {
        for c in &mut core[r.0..(r.0 + em).min(r.1)] {
            *c = false;
        }
        for c in &mut core[r.1.saturating_sub(em).max(r.0)..r.1] {
            *c = false;
        }
    }
    if !core.iter().any(|c| *c) {
        core.clone_from(&on);
    }
    let on_count = on.iter().filter(|o| **o).count();
    f.duty = on_count as f64 / len as f64;
    // Runs touching the ends are cut by the capture: not counted for lengths.
    let whole: Vec<&(usize, usize)> = runs.iter().filter(|r| r.0 > 0 && r.1 < len).collect();
    f.bursts = runs.len();
    if f.duty < 0.97 && !whole.is_empty() {
        // The moving average stretches each burst by about its own length (less the part under the threshold).
        let stretch = lw as f64 * (1.0 - 2.0 * (thr / hi.max(thr)).min(0.5));
        let lens: Vec<f64> = whole
            .iter()
            .map(|r| ((r.1 - r.0) as f64 - stretch).max(1.0) / rc)
            .collect();
        f.burst_s = Some(median64(&lens));
        if runs.len() >= 3 {
            let gaps: Vec<f64> = runs
                .windows(2)
                .map(|w| (w[1].0 - w[0].0) as f64 / rc)
                .collect();
            let m = median64(&gaps);
            let dev: Vec<f64> = gaps.iter().map(|g| (g - m).abs()).collect();
            if m > 0.0 && median64(&dev) / m < 0.1 {
                f.burst_period_s = Some(m);
            }
        }
    }
    f.why(
        "duty",
        format!("{:.0} %", f.duty * 100.0),
        "the share of time the smoothed power is over the middle (in dB) of noise and signal",
    );
    if let Some(b) = f.burst_s {
        let per = f
            .burst_period_s
            .map_or(String::new(), |p| format!(", every {:.2} ms", p * 1e3));
        f.why(
            "bursts",
            format!("{} bursts, median {:.2} ms{}", f.bursts, b * 1e3, per),
            "runs of the smoothed power over the threshold",
        );
    }

    // Kurtosis of the envelope while on, with the noise taken out.
    let on_powers = p
        .iter()
        .zip(&on)
        .filter(|(_, o)| **o)
        .map(|(s, _)| *s as f64);
    if let Some((m2, m4)) = power_moments(on_powers) {
        f.kurtosis = plain_kurtosis_less_noise(m2, m4, noise).clamp(0.5, 10.0);
    }
    f.why(
        "envelope kurtosis",
        format!("{:.2}", f.kurtosis),
        "E|s|⁴/(E|s|²)² with the noise removed: 1 constant envelope, about 1.3 filtered PSK, 2 noise-like (OFDM)",
    );

    // Power lines (x/|x|)^n, gated to the bursts.
    let nl = pow2_floor(len).min(1 << 16);
    let start = on.iter().position(|o| *o).unwrap_or(0).min(len - nl);
    let u: Vec<Complex32> = (start..start + nl)
        .map(|i| {
            let s = y[i];
            let m = s.norm();
            if on[i] && m > 0.0 {
                s / m
            } else {
                Complex32::default()
            }
        })
        .collect();
    for (k, n) in [1u32, 2, 4, 8].iter().enumerate() {
        let un: Vec<Complex32> = u.iter().map(|c| c.powu(*n)).collect();
        let sp = line_spectrum(&un, nl);
        let med = median(&sp).max(1e-30);
        let (j, v) = sp
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(j, v)| (j, *v as f64))
            .unwrap_or((0, 0.0));
        let fj = if j > nl / 2 {
            j as f64 - nl as f64
        } else {
            j as f64
        };
        f.power_lines[k] = PowerLine {
            n: *n,
            strength_db: power_to_db(v / med),
            offset_hz: fj * rc / nl as f64 / *n as f64,
        };
    }
    // The carrier line: the bin at 0 Hz of y against the signal's own spectrum around it.
    {
        let sp = line_spectrum(&y[start..start + nl], nl);
        let half = ((0.02 * w99 / rc * nl as f64).ceil() as usize).max(2);
        let c = (0..=half)
            .flat_map(|k| [k, (nl - k) % nl])
            .map(|k| sp[k] as f64)
            .fold(0.0, f64::max);
        let band = ((0.5 * w99 / rc * nl as f64) as usize).max(half + 4);
        let mut around: Vec<f32> = (half + 2..band).flat_map(|k| [sp[k], sp[nl - k]]).collect();
        if around.is_empty() {
            around.push(1e-30);
        }
        let ref_ = quantile(&around, 0.9).max(1e-30);
        f.carrier_line_db = power_to_db(c / ref_);
    }
    f.why(
        "lines",
        f.power_lines
            .iter()
            .map(|l| {
                format!(
                    "x^{}: {:.0} dB at {:+.0} Hz",
                    l.n, l.strength_db, l.offset_hz
                )
            })
            .collect::<Vec<_>>()
            .join("; "),
        "the strongest line of (x/|x|)^n over the median of its spectrum; its frequency / n is the carrier offset",
    );

    // Instantaneous frequency.
    let fr = discriminator(&y, rc);
    let fg: Vec<f32> = fr
        .iter()
        .zip(&on)
        .map(|(v, o)| if *o { *v } else { 0.0 })
        .collect();

    // Symbol rate candidates.
    let ns = pow2_floor(len).min(1 << 18);
    let bursty = f.duty < 0.97;
    let env_lines = {
        let lo = w99 / 2.5;
        let hi = (w99 * 1.1).min(0.47 * rc);
        rate_line(&p, &on, &runs, bursty, rc, lo, hi, ns)
    };
    let disc_lines = {
        // Transitions of the smoothed frequency.
        let fs = moving_average(&fr, ((rc / w99).round() as usize).max(1));
        let mut g: Vec<f32> = Vec::with_capacity(fs.len());
        for i in 0..fs.len() {
            let a = if i > 0 { fs[i - 1] } else { fs[0] };
            g.push((fs[i] - a).abs());
        }
        let lo = w99 / 24.0;
        let hi = (w99 * 1.1).min(0.47 * rc);
        rate_line(&g, &on, &runs, bursty, rc, lo, hi, ns)
    };

    // Is the FM modulation periodic (a test tone)?
    {
        let sp = real_line_spectrum(&fg[..ns.min(fg.len())], ns);
        let total: f64 = sp.iter().skip(1).map(|v| *v as f64).sum();
        let mut idx: Vec<usize> = (1..sp.len()).collect();
        idx.sort_by(|a, b| sp[*b].total_cmp(&sp[*a]));
        let mut taken = 0f64;
        let mut used: Vec<usize> = Vec::new();
        for k in idx {
            if used.len() >= 3 {
                break;
            }
            if used.iter().any(|u| u.abs_diff(k) <= 2) {
                continue;
            }
            used.push(k);
            taken += sp[k.saturating_sub(2)..(k + 3).min(sp.len())]
                .iter()
                .map(|v| *v as f64)
                .sum::<f64>();
        }
        f.periodic_modulation = total > 0.0 && taken / total > 0.5;
    }

    // OFDM: the cyclic prefix.
    if f.kurtosis > 1.4 {
        f.ofdm = ofdm(&y, rc, w99);
        if let Some(o) = &f.ofdm {
            let g = o.tg_s.map_or("no guard found".to_string(), |g| {
                format!("Tg {:.1} µs", g * 1e6)
            });
            f.why(
                "OFDM",
                format!(
                    "Tu {:.1} µs, {}, spacing {:.0} Hz, ρ {:.3} (z {:.0})",
                    o.tu_s * 1e6,
                    g,
                    o.spacing_hz,
                    o.cp_corr,
                    o.z
                ),
                "the autocorrelation peaks at the useful symbol (the cyclic prefix repeats its end); the guard is where the product repeats",
            );
        }
    }

    // Chirps.
    if f.kurtosis < 1.4 {
        f.chirp = chirp(&y, &on, rc, w99);
        if let Some(c) = &f.chirp {
            f.why(
                "chirp",
                format!(
                    "{} sweep of {:.3} ms{}",
                    fmt_width(c.bw_hz),
                    c.symbol_s * 1e3,
                    c.sf.map_or(String::new(), |s| format!(", SF{s}"))
                ),
                "the instantaneous frequency ramps steadily across the band and jumps back",
            );
        }
    }

    // A 19 kHz pilot in a wide FM signal.
    if w99 > 60e3 && rc > 60e3 {
        let nf = pow2_floor(len).min(1 << 17);
        let sp = real_line_spectrum(&fr[..nf], nf);
        let k = 19_000.0 / rc * nf as f64;
        let half = (100.0 / rc * nf as f64).ceil() as usize + 1;
        if let Some(l) = strongest_line(
            &sp,
            (k as usize).saturating_sub(half),
            k as usize + half,
            64,
        ) {
            if l.strength_db >= 12.0 {
                f.fm_pilot = true;
                f.why(
                    "FM pilot",
                    format!("{:.0} dB", l.strength_db),
                    "a line at 19 kHz in the demodulated FM: a stereo pilot",
                );
            }
        }
    }

    // FSK levels.
    if f.kurtosis < 1.1 && f.chirp.is_none() {
        // Of the strongest transition lines, the one whose symbol samples cluster best (a stronger line wins a near tie).
        let mut best: Option<(f64, f64, f64, Fsk)> = None;
        for &(rs, st) in disc_lines.iter().filter(|l| l.1 >= 6.0) {
            if let Some(k) = fsk_levels(&fr, &core, rc, rs) {
                let better = match &best {
                    None => true,
                    Some(b) => {
                        k.scatter < 0.8 * b.3.scatter || (k.scatter < 1.2 * b.3.scatter && st > b.1)
                    }
                };
                if better {
                    best = Some((rs, st, k.scatter, k));
                }
            }
        }
        if let Some((rs, st, _, k)) = best {
            f.symbol_rate = Some(rs);
            f.rate_source = RateSource::Discriminator;
            f.rate_line_db = st;
            f.fsk = Some(k);
        }
    }
    // MSK lines: two lines of x² Rs apart.
    let msk_rate = msk_lines(&u, rc, w99);

    // Tones and periodicity across the band.
    tones(&mut f, &fine, lo_b + i_lo, lo_b + i_hi, floor, peak);

    // Decide the class.
    let tiny = w99 <= 4.0 * bh || f.shape == Shape::Line;
    let class = if tiny && f.kurtosis < 1.25 {
        ModClass::Carrier
    } else if f.chirp.is_some() {
        f.symbol_rate = f.chirp.map(|c| 1.0 / c.symbol_s);
        f.rate_source = RateSource::Chirp;
        ModClass::Chirp
    } else if f.duty < 0.7
        && f.kurtosis > 1.6
        && f.burst_s.is_some_and(|b| b < 5e-3)
        && f.bursts >= 3
    {
        ModClass::Pulse
    } else if f.ofdm.is_some_and(|o| o.z >= 8.0) && f.kurtosis > 1.5 && f.duty >= 0.7 {
        let o = f.ofdm.expect("checked by the condition above");
        if let Some(g) = o.tg_s {
            f.symbol_rate = Some(1.0 / (o.tu_s + g));
            f.rate_source = RateSource::OfdmSymbol;
        }
        ModClass::Ofdm
    } else if f.carrier_line_db >= 12.0 && f.kurtosis > 1.08 && f.fsk.is_none() {
        ModClass::Am
    } else if let Some(k) = f.fsk.clone().filter(|_| !f.periodic_modulation) {
        let rs = f.symbol_rate.unwrap_or(1.0);
        if k.levels == 4 {
            ModClass::Fsk4
        } else if (0.15..=0.36).contains(&(k.outer_dev_hz / rs)) {
            ModClass::Gmsk
        } else {
            ModClass::Fsk2
        }
    } else if let (Some(rs), true) = (msk_rate, f.kurtosis < 1.1 && !f.periodic_modulation) {
        f.symbol_rate = Some(rs);
        f.rate_source = RateSource::PowerLines;
        ModClass::Gmsk
    } else if f.kurtosis < 1.15 {
        if f.fsk.is_some() {
            // Clusters, but from a repeating tone.
            f.fsk = None;
            f.symbol_rate = None;
            f.rate_source = RateSource::None;
        }
        ModClass::Fm
    } else if let Some(&(rs, st)) = env_lines.first().filter(|l| l.1 >= 8.0 && f.kurtosis < 1.9) {
        f.symbol_rate = Some(rs);
        f.rate_source = RateSource::Envelope;
        f.rate_line_db = st;
        let c = constellation(&y, &core, rc, rs, &f.power_lines);
        f.constellation = Some(c);
        match (c.order, c.qam) {
            (_, true) => ModClass::Qam,
            (2, _) => ModClass::Bpsk,
            // π/4-DQPSK: the x⁴ line is Rs/2 away from four times the carrier (which is near 0 Hz here).
            (4, _) if ((f.power_lines[2].offset_hz * 4.0).abs() - rs / 2.0).abs() < 0.06 * rs => {
                ModClass::Pi4Dqpsk
            }
            (4, _) => ModClass::Qpsk,
            (8, _) => ModClass::Psk8,
            _ => ModClass::Unknown,
        }
    } else {
        ModClass::Unknown
    };
    f.class = class;
    if class == ModClass::Pulse {
        // Short pulses spread wider than the average spectrum shows: measure them again on the whole capture, inside the bursts.
        if let Some(b) = pulses(x, rate) {
            f.width_99_hz = b.width_99;
            f.offset_hz = b.centre;
            f.rf_hz = rf_hz + b.centre;
            f.duty = b.duty;
            f.bursts = b.bursts;
            f.burst_s = b.burst_s;
            f.burst_period_s = b.period_s;
            f.why(
                "pulses",
                format!(
                    "{} wide inside the bursts, {} bursts, median {:.1} µs",
                    fmt_width(b.width_99),
                    b.bursts,
                    b.burst_s.unwrap_or(0.0) * 1e6
                ),
                "measured on the whole capture, only where the power rises above the noise",
            );
        }
    }
    if let Some(r) = f.symbol_rate {
        f.rolloff = (f.rate_source != RateSource::OfdmSymbol).then_some(w99 / r - 1.0);
        f.why(
            "symbol rate",
            format!(
                "{:.1} Bd ({:?}, line {:.0} dB)",
                r, f.rate_source, f.rate_line_db
            ),
            "the strongest periodic line where a symbol clock can be",
        );
    }
    if let Some(k) = &f.fsk {
        f.why(
            "FSK levels",
            format!(
                "{} levels at {} Hz, scatter {:.2}",
                k.levels,
                k.deviations_hz
                    .iter()
                    .map(|d| format!("{d:+.0}"))
                    .collect::<Vec<_>>()
                    .join(" / "),
                k.scatter
            ),
            "the demodulated frequency sampled once a symbol falls into clusters",
        );
    }
    if let Some(c) = &f.constellation {
        f.why(
            "constellation",
            format!(
                "order {}, |u²| {:.2}, |u⁴| {:.2}, |u⁸| {:.2}, differential |d⁴| {:.2}, amplitude spread {:.2}",
                c.order,
                c.coherence[0],
                c.coherence[1],
                c.coherence[2],
                c.diff_coherence[1],
                c.amplitude_spread
            ),
            "phase coherence at the symbol instants: high at M = 2 for BPSK, 4 for QPSK, 8 for 8PSK; amplitude rings for QAM",
        );
    }
    let class_why = match class {
        ModClass::Carrier => "a single line, constant envelope",
        ModClass::Chirp => "linear frequency sweeps",
        ModClass::Ofdm => "noise-like envelope with a cyclic prefix",
        ModClass::Pulse => "short bursts with an on-off envelope",
        ModClass::Am => "a carrier line with a varying envelope",
        ModClass::Fsk2 | ModClass::Fsk4 | ModClass::Gmsk => {
            "constant envelope, frequency levels at a symbol clock"
        }
        ModClass::Fm => "constant envelope with no symbol clock",
        ModClass::Bpsk | ModClass::Qpsk | ModClass::Pi4Dqpsk | ModClass::Psk8 | ModClass::Qam => {
            "an envelope line at the symbol rate and phase coherence at the symbol instants"
        }
        _ => "no rule fits",
    };
    f.why("class", class.word().into(), class_why);
    chan.samples = y;
    chan.on = on;
    (f, chan)
}

struct PulseMeasure {
    width_99: f64,
    centre: f64,
    duty: f64,
    bursts: usize,
    burst_s: Option<f64>,
    period_s: Option<f64>,
}

/// Bursts of short pulses on the whole capture: their width inside the bursts, their lengths and period.
fn pulses(x: &[Complex32], rate: f64) -> Option<PulseMeasure> {
    let n = x.len().min(1 << 22);
    let p: Vec<f32> = x[..n].iter().map(|s| s.norm_sqr()).collect();
    let lw = ((rate * 2e-6) as usize).max(2);
    let ps = moving_average(&p, lw);
    let noise = quantile(&ps, 0.3).max(1e-30);
    let hi = quantile(&ps, 0.999);
    if hi < noise * 4.0 {
        return None;
    }
    let thr = (noise * hi).sqrt();
    let on: Vec<bool> = ps.iter().map(|v| (*v as f64) > thr).collect();
    // Bursts: runs with gaps of up to 10 µs closed (the gaps between pulses of one reply).
    let gap = ((rate * 10e-6) as usize).max(2);
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        if on[i] {
            let a = i;
            while i < n && on[i] {
                i += 1;
            }
            match runs.last_mut() {
                Some(l) if a - l.1 <= gap => l.1 = i,
                _ => runs.push((a, i)),
            }
        } else {
            i += 1;
        }
    }
    if runs.len() < 3 {
        return None;
    }
    let mut mask = vec![false; n];
    for r in &runs {
        for m in &mut mask[r.0..r.1] {
            *m = true;
        }
    }
    let quiet: Vec<bool> = mask.iter().map(|m| !m).collect();
    let nfft = 256.min(pow2_floor(n));
    let inb = Psd::welch_where(&x[..n], rate, nfft, 2000, Some(&mask));
    let off = Psd::welch_where(&x[..n], rate, nfft, 2000, Some(&quiet));
    let floor = median(&off.density);
    let s: Vec<f64> = inb
        .density
        .iter()
        .map(|d| (*d as f64 - floor).max(0.0))
        .collect();
    let total: f64 = s.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let (mut a, mut b, mut c) = (0usize, nfft - 1, 0.0);
    let mut lo_set = false;
    for (k, v) in s.iter().enumerate() {
        c += v;
        if !lo_set && c >= 0.005 * total {
            a = k;
            lo_set = true;
        }
        if c >= 0.995 * total {
            b = k;
            break;
        }
    }
    let lens: Vec<f64> = runs
        .iter()
        .filter(|r| r.0 > 0 && r.1 < n)
        .map(|r| (r.1 - r.0) as f64 / rate)
        .collect();
    let starts: Vec<f64> = runs
        .windows(2)
        .map(|w| (w[1].0 - w[0].0) as f64 / rate)
        .collect();
    let m = median64(&starts);
    let dev: Vec<f64> = starts.iter().map(|g| (g - m).abs()).collect();
    Some(PulseMeasure {
        width_99: (b as f64 - a as f64 + 1.0) * inb.bin_hz(),
        centre: inb.freq(0.5 * (a + b) as f64),
        duty: mask.iter().filter(|m| **m).count() as f64 / n as f64,
        bursts: runs.len(),
        burst_s: (!lens.is_empty()).then(|| median64(&lens)),
        period_s: (m > 0.0 && median64(&dev) / m < 0.1).then_some(m),
    })
}

/// The strongest line of `g` between `lo` and `hi` Hz, as (Hz, dB over its neighbours): one long spectrum of a continuous signal
/// (the gaps filled with the mean, so the gating adds no lines), or the average of spectra taken inside the bursts.
#[allow(clippy::too_many_arguments)]
fn rate_line(
    g: &[f32],
    on: &[bool],
    runs: &[(usize, usize)],
    bursty: bool,
    rc: f64,
    lo: f64,
    hi: f64,
    ns: usize,
) -> Vec<(f64, f64)> {
    // One long spectrum, the gaps (and the edges of the bursts, where the discriminator is noise) filled with the mean, so the
    // gating adds few lines of its own; the symbol clock of a TDMA signal runs on across its bursts.
    let n = ns.min(g.len());
    let margin = if bursty {
        ((rc / hi) * 8.0) as usize
    } else {
        0
    };
    let mut keep = on[..n].to_vec();
    if margin > 0 {
        for r in runs {
            for k in &mut keep[r.0.min(n)..(r.0 + margin).min(r.1).min(n)] {
                *k = false;
            }
            let a = r.1.saturating_sub(margin).max(r.0).min(n);
            for k in &mut keep[a..r.1.min(n)] {
                *k = false;
            }
        }
    }
    let (mut sum, mut cnt) = (0f64, 0usize);
    for i in 0..n {
        if keep[i] {
            sum += g[i] as f64;
            cnt += 1;
        }
    }
    let mean = (sum / cnt.max(1) as f64) as f32;
    let h: Vec<f32> = (0..n).map(|i| if keep[i] { g[i] } else { mean }).collect();
    let sp = real_line_spectrum(&h, ns);
    let k = |hz: f64| (hz / rc * ns as f64) as usize;
    let long: Vec<(f64, f64)> = top_lines(&sp, k(lo).max(2), k(hi), 32, 5, 0.03)
        .into_iter()
        .map(|l| (l.bin * rc / ns as f64, l.strength_db))
        .collect();
    if !bursty || long.first().is_some_and(|l| l.1 >= 10.0) {
        return long;
    }
    let lens: Vec<usize> = runs.iter().map(|r| r.1 - r.0).collect();
    let longest = lens.iter().copied().max().unwrap_or(0);
    let nseg = pow2_floor(((longest as f64) * 0.9) as usize).min(1 << 14);
    if nseg < 128 {
        return long;
    }
    let mut acc = vec![0f64; nseg / 2 + 1];
    let mut used = 0;
    for r in runs {
        let mut a = r.0 + (r.1 - r.0) / 20;
        while a + nseg <= r.1 && used < 400 {
            let sp = real_line_spectrum(&g[a..a + nseg], nseg);
            for (x, v) in acc.iter_mut().zip(&sp) {
                *x += *v as f64;
            }
            used += 1;
            a += nseg / 2;
        }
    }
    if used == 0 {
        return long;
    }
    let sp: Vec<f32> = acc.iter().map(|v| *v as f32).collect();
    let k = |hz: f64| (hz / rc * nseg as f64) as usize;
    let mut v: Vec<(f64, f64)> = top_lines(&sp, k(lo).max(2), k(hi), 8, 5, 0.03)
        .into_iter()
        .map(|l| (l.bin * rc / nseg as f64, l.strength_db))
        .collect();
    v.extend(long);
    v
}

fn ofdm(y: &[Complex32], rc: f64, w99: f64) -> Option<Ofdm> {
    let la = pow2_floor(y.len()).min(1 << 19);
    if la < 4096 {
        return None;
    }
    let n = la * 2;
    let mut buf: Vec<Complex32> = (0..n)
        .map(|i| if i < la { y[i] } else { Complex32::default() })
        .collect();
    Fft::forward(n).process(&mut buf);
    for b in &mut buf {
        *b = Complex32::new(b.norm_sqr(), 0.0);
    }
    Fft::inverse(n).process(&mut buf);
    let r0 = buf[0].re as f64;
    if r0 <= 0.0 {
        return None;
    }
    let lmin = ((6.0 * rc / w99).round() as usize).max(32);
    let lmax = (la / 3).min((rc * 0.005) as usize);
    if lmax <= lmin + 10 {
        return None;
    }
    // Normalise each lag by its overlap.
    let rho: Vec<f32> = (lmin..=lmax)
        .map(|l| (buf[l].norm() as f64 / r0 * la as f64 / (la - l) as f64) as f32)
        .collect();
    let max = rho.iter().copied().fold(0f32, f32::max) as f64;
    let noise = 1.25 * median(&rho);
    // Pilots that repeat every symbol or every few symbols correlate at multiples of the symbol period, sometimes more than the
    // cyclic prefix does: the useful symbol is the first strong peak, not the strongest.
    let k = (1..rho.len().saturating_sub(1)).find(|&i| {
        let v = rho[i] as f64;
        rho[i] >= rho[i - 1] && rho[i] >= rho[i + 1] && v >= 0.35 * max && v >= 8.0 * noise
    })?;
    let pk = rho[k] as f64;
    let z = pk / noise.max(1e-12);
    let frac = if k > 0 && k + 1 < rho.len() {
        sdrkit_algo_dsp::peak::parabolic(rho[k - 1] as f64, rho[k] as f64, rho[k + 1] as f64).offset
    } else {
        0.0
    };
    let l = lmin + k;
    let lf = l as f64 + frac;
    let tu = lf / rc;
    // The guard: the product x[n] x*[n+L] is large in each symbol's prefix, once a symbol period.
    let m = (y.len() - l).min(1 << 19);
    let na = pow2_floor(m);
    let a: Vec<f32> = (0..na).map(|i| (y[i] * y[i + l].conj()).norm()).collect();
    let sp = real_line_spectrum(&a, na);
    let lo = (na as f64 / (1.35 * lf)).floor() as usize;
    let hi = (na as f64 / (1.004 * lf)).ceil() as usize;
    let tg = strongest_line(&sp, lo, hi, 16)
        .filter(|ln| ln.strength_db >= 10.0)
        .map(|ln| na as f64 / ln.bin / rc - tu);
    Some(Ofdm {
        tu_s: tu,
        tg_s: tg,
        cp_corr: pk,
        z,
        spacing_hz: 1.0 / tu,
        guard_fraction: tg.map(|g| g / tu),
    })
}

fn chirp(y: &[Complex32], on: &[bool], rc: f64, w99: f64) -> Option<Chirp> {
    // Track the peak of short spectra: a chirp moves it by the same step every hop, except where it wraps or a symbol starts.
    let len = y.len().min(1 << 18);
    let mut best: Option<(f64, Chirp)> = None;
    for nw in [32usize, 64, 128, 256, 512, 1024, 2048] {
        let hop = nw / 2;
        if len < nw * 40 {
            break;
        }
        let mut fft = Fft::forward(nw);
        let mut buf = vec![Complex32::default(); nw];
        let mut track: Vec<Option<f64>> = Vec::new();
        let mut i = 0;
        while i + nw <= len {
            if on[i] && on[i + nw - 1] {
                buf.copy_from_slice(&y[i..i + nw]);
                fft.process(&mut buf);
                let p: Vec<f64> = buf.iter().map(|c| c.norm_sqr() as f64).collect();
                let (k, _) = p
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap_or((0, &0.0));
                let fr =
                    sdrkit_algo_dsp::peak::parabolic(p[(k + nw - 1) % nw], p[k], p[(k + 1) % nw])
                        .offset;
                let mut bin = k as f64 + fr;
                if bin > nw as f64 / 2.0 {
                    bin -= nw as f64;
                }
                track.push(Some(bin * rc / nw as f64));
            } else {
                track.push(None);
            }
            i += hop;
        }
        let mut steps = Vec::new();
        let mut freqs = Vec::new();
        for w in track.windows(2) {
            if let (Some(a), Some(b)) = (w[0], w[1]) {
                let mut d = b - a;
                if d > rc / 2.0 {
                    d -= rc;
                } else if d < -rc / 2.0 {
                    d += rc;
                }
                steps.push(d);
                freqs.push(a);
            }
        }
        if steps.len() < 32 {
            continue;
        }
        let m = median64(&steps);
        // A step of at least half a bin, and most steps alike.
        if m.abs() < 0.5 * rc / nw as f64 {
            continue;
        }
        let good = steps
            .iter()
            .filter(|d| (*d - m).abs() < 0.3 * m.abs())
            .count() as f64
            / steps.len() as f64;
        let mut fs = freqs.clone();
        fs.sort_by(|a, b| a.total_cmp(b));
        let span = fs[fs.len() * 95 / 100] - fs[fs.len() * 5 / 100];
        if good < 0.6 || span < 0.6 * w99 {
            continue;
        }
        let refined: Vec<f64> = steps
            .iter()
            .copied()
            .filter(|d| (*d - m).abs() < 0.3 * m.abs())
            .collect();
        let slope = median64(&refined) / (hop as f64 / rc);
        let bw = w99;
        let t = bw / slope.abs();
        let sf_f = (t * bw).log2();
        let sf = Some(sf_f.round() as u8)
            .filter(|s| (sf_f - *s as f64).abs() < 0.2 && (5..=12).contains(s));
        let c = Chirp {
            bw_hz: bw,
            symbol_s: t,
            sf,
            direction: if slope > 0.0 { 1 } else { -1 },
        };
        if best.as_ref().is_none_or(|b| good > b.0) {
            best = Some((good, c));
        }
    }
    best.map(|b| b.1)
}

fn msk_lines(u: &[Complex32], rc: f64, w99: f64) -> Option<f64> {
    let n = u.len();
    if n < 1024 {
        return None;
    }
    let u2: Vec<Complex32> = u.iter().map(|c| c * c).collect();
    let sp = line_spectrum(&u2, n);
    let med = median(&sp).max(1e-30);
    // The two strongest separated lines.
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|a, b| sp[*b].total_cmp(&sp[*a]));
    let a = idx[0];
    let b = idx
        .iter()
        .copied()
        .find(|k| (*k as i64 - a as i64).unsigned_abs() as usize > 3)?;
    let (sa, sb) = (
        power_to_db(sp[a] as f64 / med),
        power_to_db(sp[b] as f64 / med),
    );
    if sa < 15.0 || sb < 15.0 || (sa - sb).abs() > 6.0 {
        return None;
    }
    let fa = if a > n / 2 {
        a as f64 - n as f64
    } else {
        a as f64
    };
    let fb = if b > n / 2 {
        b as f64 - n as f64
    } else {
        b as f64
    };
    let rs = (fa - fb).abs() * rc / n as f64;
    if rs > w99 / 3.0 && rs < w99 * 1.2 {
        Some(rs)
    } else {
        None
    }
}

fn constellation(
    y: &[Complex32],
    on: &[bool],
    rc: f64,
    rs: f64,
    lines: &[PowerLine; 4],
) -> Constellation {
    let sps = rc / rs;
    let mf = rrc(sps, 0.35, 8);
    let z = if sps >= 2.0 {
        fir_filter(y, &mf)
    } else {
        y.to_vec()
    };
    // Timing: the phase with the most power.
    let mut best = (0.0, 0.0);
    for ph in 0..16 {
        let t0 = ph as f64 * sps / 16.0;
        let mut e = 0.0;
        let mut t = t0;
        let mut k = 0;
        while t < (z.len() - 1) as f64 && k < 4000 {
            e += interp_c(&z, t).norm_sqr() as f64;
            t += sps;
            k += 1;
        }
        if e > best.0 {
            best = (e, t0);
        }
    }
    // Carrier offset from the strongest of the x^4, x^2, x^8 lines.
    let line = [lines[2], lines[1], lines[3]]
        .into_iter()
        .max_by(|a, b| a.strength_db.total_cmp(&b.strength_db))
        .unwrap_or_default();
    let fo = if line.strength_db > 15.0 {
        line.offset_hz
    } else {
        0.0
    };
    let mut s: Vec<Complex32> = Vec::new();
    let mut t = best.1;
    while t < (z.len() - 1) as f64 && s.len() < 20_000 {
        if on[t as usize] {
            let ph = -TAU * fo * t / rc;
            s.push(interp_c(&z, t) * Complex32::new(ph.cos() as f32, ph.sin() as f32));
        }
        t += sps;
    }
    if s.len() < 64 {
        return Constellation {
            order: 0,
            qam: false,
            coherence: [0.0; 3],
            diff_coherence: [0.0; 3],
            amplitude_spread: 0.0,
        };
    }
    let coherence = |v: &[Complex32], m: u32| -> f64 {
        let mut acc = 0.0;
        let mut nb = 0;
        for b in v.chunks(32).filter(|b| b.len() == 32) {
            let mut sum = Complex32::default();
            for c in b {
                let nn = c.norm();
                if nn > 0.0 {
                    sum += (c / nn).powu(m);
                }
            }
            acc += sum.norm() as f64 / 32.0;
            nb += 1;
        }
        if nb == 0 {
            0.0
        } else {
            acc / nb as f64
        }
    };
    let coh = [coherence(&s, 2), coherence(&s, 4), coherence(&s, 8)];
    let d: Vec<Complex32> = s.windows(2).map(|w| w[1] * w[0].conj()).collect();
    let dcoh = [coherence(&d, 2), coherence(&d, 4), coherence(&d, 8)];
    let mags: Vec<f64> = s.iter().map(|c| c.norm() as f64).collect();
    let mm = mags.iter().sum::<f64>() / mags.len() as f64;
    let spread = (mags.iter().map(|m| (m - mm) * (m - mm)).sum::<f64>() / mags.len() as f64).sqrt()
        / mm.max(1e-12);
    let (order, qam) = if coh[0] > 0.5 {
        (2, false)
    } else if coh[1] > 0.25 && spread > 0.2 {
        (16, true)
    } else if coh[1] > 0.45 || dcoh[1] > 0.5 {
        (4, false)
    } else if coh[2] > 0.45 {
        (8, false)
    } else if spread > 0.25 {
        (64, true)
    } else {
        (0, false)
    };
    let _ = PI;
    Constellation {
        order,
        qam,
        coherence: coh,
        diff_coherence: dcoh,
        amplitude_spread: spread,
    }
}

fn tones(f: &mut Features, fine: &Psd, a: usize, b: usize, floor: f64, peak: f64) {
    let d = &fine.density;
    let bh = fine.bin_hz();
    if b <= a + 1 {
        f.shape = Shape::Line;
        return;
    }
    let mut found: Vec<usize> = Vec::new();
    for i in a.max(3)..=b.min(d.len() - 4) {
        let v = d[i] as f64;
        if v < floor * 10.0 || v < peak * 1e-3 {
            continue;
        }
        if d[i] >= d[i - 1]
            && d[i] > d[i + 1]
            && v > 4.0 * d[i - 3] as f64
            && v > 4.0 * d[i + 3] as f64
        {
            found.push(i);
        }
    }
    let line_power: f64 = found
        .iter()
        .map(|&i| (d[i - 1] + d[i] + d[i + 1]) as f64)
        .sum();
    let total: f64 = (a..=b).map(|i| d[i] as f64).sum();
    if found.len() >= 2 {
        let diffs: Vec<f64> = found
            .windows(2)
            .map(|w| (w[1] - w[0]) as f64 * bh)
            .collect();
        f.tones = Some(Tones {
            count: found.len(),
            spacing_hz: median64(&diffs),
        });
    }
    f.shape = if found.len() == 1 && line_power > 0.8 * total {
        Shape::Line
    } else if found.len() >= 2 && line_power > 0.5 * total {
        Shape::Tones
    } else if f.flat_top >= 0.85 {
        Shape::FlatTop
    } else {
        Shape::Rounded
    };
    // The strongest periodicity of the spectrum (in dB) across the band.
    let seg: Vec<f64> = (a..=b).map(|i| power_to_db(d[i] as f64)).collect();
    if seg.len() >= 16 {
        let m = seg.iter().sum::<f64>() / seg.len() as f64;
        let c: Vec<f64> = seg.iter().map(|v| v - m).collect();
        let r0: f64 = c.iter().map(|v| v * v).sum();
        if r0 > 0.0 {
            let mut best = (0usize, 0f64);
            for lag in 3..seg.len() / 2 {
                let r: f64 = c.iter().zip(&c[lag..]).map(|(x, y)| x * y).sum::<f64>() / r0;
                if r > best.1 {
                    best = (lag, r);
                }
            }
            if best.1 > 0.3 {
                f.periodicity = Some((best.0 as f64 * bh, best.1));
            }
        }
    }
    if let Some(t) = &f.tones {
        let (c, s) = (t.count, t.spacing_hz);
        f.why(
            "tones",
            format!("{c} tones, spacing {s:.0} Hz"),
            "narrow peaks 6 dB over their neighbours and 10 dB over the floor",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_reads_well() {
        let mut f = Features {
            offset_hz: 0.0,
            rf_hz: 0.0,
            width_3db_hz: 0.0,
            width_20db_hz: 0.0,
            width_26db_hz: 0.0,
            width_99_hz: 9600.0,
            snr_db: 20.0,
            power_dbfs: -30.0,
            flat_top: 0.5,
            duty: 1.0,
            bursts: 1,
            burst_s: None,
            burst_period_s: None,
            capture_s: 1.0,
            kurtosis: 1.0,
            carrier_line_db: 0.0,
            power_lines: [PowerLine::default(); 4],
            class: ModClass::Fsk4,
            symbol_rate: Some(4800.0),
            rate_source: RateSource::Discriminator,
            rate_line_db: 20.0,
            fsk: Some(Fsk {
                levels: 4,
                deviations_hz: vec![],
                outer_dev_hz: 1944.0,
                inner_dev_hz: Some(648.0),
                scatter: 0.1,
                centre_hz: 0.0,
            }),
            ofdm: None,
            chirp: None,
            fm_pilot: false,
            shape: Shape::Rounded,
            rolloff: None,
            tones: None,
            periodicity: None,
            constellation: None,
            periodic_modulation: false,
            why: vec![],
        };
        assert_eq!(
            f.summary(),
            "4FSK, 4800 Bd, ±648 / ±1944 Hz, 9.6 kHz wide, continuous"
        );
        f.duty = 0.5;
        f.burst_period_s = Some(0.06);
        assert!(f.summary().ends_with("bursts every 60.0 ms"));
    }
}
