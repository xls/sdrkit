//! The SELCAL test signal: speech-like audio, then the calls AM-CP at 4.0 s, DH-KS at 10.5 s and AM-CP at 17.0 s (pairs of
//! 1 s, 0.2 s apart, each tone at half amplitude), in a loop of 22 s, carried as FM (1.0 = 5 kHz), AM or a sideband. The signal
//! is clean: the engine adds the noise.
//! Standard: ARINC 714 (pulses of 1 s, 0.2 s apart).

use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::RationalResampler;
use sdrkit_algo_sim::modulate::{Am, Fm, Sideband, Ssb};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_util_ids::tones::selcal::Code;
use std::f64::consts::TAU;

/// The four tones of a code ("AM-CP"), or `None` for a code that is not four tone designators in two pairs.
pub fn tones(code: &str) -> Option<[f64; 4]> {
    Code::parse(code).map(|c| c.tones().map(|f| f as f64))
}

/// The amplitude of the carrier.
pub const AMPLITUDE: f64 = 0.1;

/// How the audio is carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    /// FM, 1.0 = 5 kHz deviation.
    Fm,
    /// AM. Each tone at `depth` of the carrier (40 %).
    Am,
    /// Upper sideband, suppressed carrier (the audio above the dial).
    Usb,
    /// Lower sideband.
    Lsb,
}

/// The scene.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// The channel's offset from the centre of the samples.
    pub signal_offset_hz: f64,
    /// FM or AM.
    pub carry: Carry,
    /// The calls: (start of pair 1, code).
    pub calls: Vec<(f64, String)>,
    /// The loop length, s.
    pub period_s: f64,
    /// Speech between the calls.
    pub voice: bool,
    /// Seed.
    pub seed: u64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 2e6,
            signal_offset_hz: -50e3,
            carry: Carry::Fm,
            calls: vec![
                (4.0, "AM-CP".into()),
                (10.5, "DH-KS".into()),
                (17.0, "AM-CP".into()),
            ],
            period_s: 22.0,
            voice: true,
            seed: 1,
        }
    }
}

/// The audio rate the modulators work at, before the rise to the output rate.
const AUDIO_RATE_HZ: f64 = 32e3;

/// The modulator of the carry.
enum Modulator {
    Fm(Fm),
    Am(Am),
    Ssb(Ssb),
}

/// The SELCAL generator.
pub struct SelcalGenerator {
    p: GenParams,
    /// Audio samples made.
    n: u64,
    /// The audio rate (the output rate over a whole number).
    fa: f64,
    rng: Rng,
    bp1: Biquad,
    bp2: Biquad,
    modulator: Modulator,
    up: Option<RationalResampler<Complex32>>,
    shift: Nco,
    audio: Vec<f32>,
    base: Vec<Complex32>,
    ready: Vec<Complex32>,
    used: usize,
}

impl SelcalGenerator {
    /// A generator for the scene.
    pub fn new(p: GenParams) -> Self {
        let fs = p.rate_hz;
        let rise = ((fs / AUDIO_RATE_HZ).round() as usize).max(1);
        let fa = fs / rise as f64;
        let modulator = match p.carry {
            Carry::Fm => {
                Modulator::Fm(Fm::new(fa, 5000.0).expect("5 kHz deviation fits the audio rate"))
            }
            Carry::Am => Modulator::Am(Am::new(0.8).expect("a depth of 0.8 is valid")),
            Carry::Usb | Carry::Lsb => {
                let side = if p.carry == Carry::Usb {
                    Sideband::Upper
                } else {
                    Sideband::Lower
                };
                Modulator::Ssb(
                    Ssb::new(side, fa, 150.0, 60.0).expect("the audio rate holds the SSB filter"),
                )
            }
        };
        SelcalGenerator {
            n: 0,
            fa,
            rng: Rng::new(p.seed),
            bp1: Biquad::new(BiquadCoeffs::highpass(fa, 300.0, 0.707)),
            bp2: Biquad::new(BiquadCoeffs::lowpass(fa, 2800.0, 0.707)),
            modulator,
            up: (rise > 1).then(|| {
                RationalResampler::with_spec(rise, 1, 0.3, 0.7, 70.0)
                    .expect("the filter settings are valid")
            }),
            shift: Nco::new(fs, p.signal_offset_hz),
            audio: Vec::new(),
            base: Vec::new(),
            ready: Vec::new(),
            used: 0,
            p,
        }
    }

    /// A generator from the stack's `gen_*` settings and the rate.
    pub fn from_config(cfg: &GenConfig) -> Self {
        SelcalGenerator::new(GenParams {
            rate_hz: cfg.rate_hz,
            seed: cfg.seed,
            ..Default::default()
        })
    }

    fn audio(&mut self, t: f64) -> f64 {
        let tl = t % self.p.period_s;
        for (start, code) in &self.p.calls {
            let Some(tn) = tones(code) else { continue };
            let pair = if tl >= *start && tl < start + 1.0 {
                Some((tn[0], tn[1]))
            } else if tl >= start + 1.2 && tl < start + 2.2 {
                Some((tn[2], tn[3]))
            } else {
                None
            };
            if let Some((a, b)) = pair {
                return 0.5 * (TAU * a * t).sin() + 0.5 * (TAU * b * t).sin();
            }
            if tl >= *start - 0.3 && tl < start + 2.5 {
                return 0.0;
            }
        }
        if !self.p.voice {
            return 0.0;
        }
        let w = self.rng.normal_f32();
        let v = self.bp2.process_sample(self.bp1.process_sample(w)) as f64;
        v * 0.3 * (0.35 + 0.65 * (TAU * 2.7 * t).sin().abs())
    }

    /// Makes 1024 audio samples, modulates them and raises them to the output rate.
    fn refill(&mut self) {
        self.ready.drain(..self.used);
        self.used = 0;
        self.audio.clear();
        for _ in 0..1024 {
            let t = self.n as f64 / self.fa;
            let a = self.audio(t) as f32;
            self.audio.push(a);
            self.n += 1;
        }
        self.base.resize(self.audio.len(), Complex32::default());
        let done = match &mut self.modulator {
            Modulator::Fm(m) => m.process(&self.audio, &mut self.base),
            Modulator::Am(m) => m.process(&self.audio, &mut self.base),
            Modulator::Ssb(m) => m.process(&self.audio, &mut self.base),
        };
        done.expect("the audio and the output have the same length");
        match &mut self.up {
            Some(up) => up.process(&self.base, &mut self.ready),
            None => self.ready.extend_from_slice(&self.base),
        }
    }
}

impl Generator for SelcalGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate_hz
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        let mut s = SignalInfo::new(6e3);
        s.power = AMPLITUDE * AMPLITUDE;
        Some(s)
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.used == self.ready.len() {
                self.refill();
            }
            let n = (self.ready.len() - self.used).min(out.len() - i);
            self.shift
                .mix(&self.ready[self.used..self.used + n], &mut out[i..i + n]);
            self.used += n;
            i += n;
        }
        let amp = AMPLITUDE as f32;
        out.iter_mut().for_each(|v| *v *= amp);
        out.len()
    }
}
