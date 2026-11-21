//! The STL test link, written from the public documents: an FM carrier with the stereo multiplex of ITU-R BS.450 and RDS
//! of IEC 62106 (the FM test transmitter at the link's deviation), the same carrier with programme audio and no pilot,
//! or band-limited Gaussian noise in the shape of a digital link. Each is the clean signal at unit power.

use super::settings::*;
use crate::generator::{FmGenerator, GenParams};
use crate::settings::{
    GEN_LEFT_AMP, GEN_LEFT_HZ, GEN_PI, GEN_PREEMPHASIS, GEN_PS, GEN_PTY, GEN_RDS, GEN_RIGHT_AMP,
    GEN_RIGHT_HZ, GEN_RT,
};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo, Values};

/// What the test link sends.
#[derive(Clone, Debug, PartialEq)]
pub struct StlParams {
    /// The link.
    pub link: GenLink,
    /// The FM carrier of a composite or mono link; its `stereo` and `rds` follow `link`.
    pub fm: GenParams,
    /// The occupied width of the digital-like link, Hz.
    pub width_hz: f64,
    /// The seed of the digital-like noise.
    pub seed: u64,
}

impl StlParams {
    /// A link of kind `link` at `rate_hz` with the test transmitter's defaults.
    pub fn new(link: GenLink, rate_hz: f64) -> Self {
        StlParams {
            link,
            fm: GenParams {
                rate_hz,
                ..Default::default()
            },
            width_hz: GEN_WIDTH.default,
            seed: 1,
        }
    }

    /// Parameters from the stack's settings (the `gen_*` settings).
    pub fn from_values(rate_hz: f64, v: &Values) -> Self {
        let d = GenParams::default();
        StlParams {
            link: v.get(&GEN_LINK),
            fm: GenParams {
                rate_hz,
                deviation_hz: v.get(&GEN_DEVIATION),
                sca: v.get(&GEN_SCA),
                left_hz: v.get(&GEN_LEFT_HZ),
                left_amp: v.get(&GEN_LEFT_AMP),
                right_hz: v.get(&GEN_RIGHT_HZ),
                right_amp: v.get(&GEN_RIGHT_AMP),
                rds: v.get(&GEN_RDS),
                ps: v.get(&GEN_PS).to_owned(),
                rt: v.get(&GEN_RT).to_owned(),
                pi: u16::from_str_radix(v.get(&GEN_PI).trim(), 16).unwrap_or(d.pi),
                pty: v.get(&GEN_PTY) as u8,
                preemphasis_s: v.get(&GEN_PREEMPHASIS).tau_s(),
                ..d
            },
            width_hz: v.get(&GEN_WIDTH),
            seed: 1,
        }
    }
}

/// Band-limited complex Gaussian noise of unit power: white noise through a low-pass filter of half the width.
#[derive(Debug)]
struct BandNoise {
    noise: Awgn,
    filter: FirDecimator<Complex32>,
    gain: f32,
    buf: Vec<Complex32>,
    out: Vec<Complex32>,
}

impl BandNoise {
    fn new(rate: f64, width_hz: f64, seed: u64) -> Option<Self> {
        let pass = (width_hz / 2.0).min(0.45 * rate);
        let stop = (pass * 1.05).min(0.5 * rate);
        let taps = sdrkit_algo_dsp::fir::lowpass(rate, pass, stop, 70.0).ok()?;
        // The filter keeps the share `sum of taps squared` of white noise of unit power.
        let kept: f64 = taps.iter().map(|&t| (t as f64) * (t as f64)).sum();
        Some(BandNoise {
            noise: Awgn::with_power(1.0, seed).ok()?,
            filter: FirDecimator::new(&taps, 1),
            gain: (1.0 / kept.sqrt()) as f32,
            buf: Vec::new(),
            out: Vec::new(),
        })
    }

    fn fill(&mut self, out: &mut [Complex32]) {
        self.buf.clear();
        self.buf.resize(out.len(), Complex32::default());
        self.noise.process(&mut self.buf);
        self.out.clear();
        self.filter.process(&self.buf, &mut self.out);
        for (o, v) in out.iter_mut().zip(&self.out) {
            *o = *v * self.gain;
        }
    }
}

/// The source of the test link.
#[derive(Debug)]
enum Source {
    Fm(Box<FmGenerator>),
    Noise(BandNoise),
    Silent,
}

/// The STL test link generator.
#[derive(Debug)]
pub struct StlGenerator {
    p: StlParams,
    source: Source,
}

impl StlGenerator {
    /// A generator for these parameters.
    pub fn new(p: StlParams) -> Self {
        let source = match p.link {
            GenLink::Composite | GenLink::Mono => {
                let composite = p.link == GenLink::Composite;
                Source::Fm(Box::new(FmGenerator::new(GenParams {
                    stereo: composite,
                    rds: composite && p.fm.rds,
                    ..p.fm.clone()
                })))
            }
            GenLink::Digital => BandNoise::new(p.fm.rate_hz, p.width_hz, p.seed)
                .map_or(Source::Silent, Source::Noise),
        };
        StlGenerator { p, source }
    }

    /// A generator from the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Self::new(StlParams::from_values(cfg.rate_hz, &cfg.settings))
    }

    /// The parameters.
    pub fn params(&self) -> &StlParams {
        &self.p
    }
}

impl Generator for StlGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.fm.rate_hz
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        match &mut self.source {
            Source::Fm(g) => {
                g.generate(out);
            }
            Source::Noise(n) => n.fill(out),
            Source::Silent => out.fill(Complex32::default()),
        }
        out.len()
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        match &self.source {
            Source::Fm(g) => g.signal_info(),
            Source::Noise(_) => Some(SignalInfo::new(self.p.width_hz).with_power(1.0)),
            Source::Silent => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power(x: &[Complex32]) -> f64 {
        x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64
    }

    #[test]
    fn every_link_has_unit_power() {
        for link in [GenLink::Composite, GenLink::Mono, GenLink::Digital] {
            let mut g = StlGenerator::new(StlParams::new(link, 1e6));
            let mut x = vec![Complex32::default(); 200_000];
            g.generate(&mut x);
            let p = power(&x[10_000..]);
            assert!((p - 1.0).abs() < 0.03, "{link:?}: {p}");
            assert_eq!(g.signal_info().unwrap().power, 1.0);
        }
        let g = StlGenerator::new(StlParams::new(GenLink::Digital, 1e6));
        assert_eq!(g.signal_info().unwrap().bandwidth_hz, 220e3);
    }
}
