//! The receiver chain of the railway CIR data channel: the IQ front end ([`Front`]: the channel at 48 kHz, the FM
//! discriminator, the carrier offset and the level) and the audio decoder ([`AudioDecoder`]: a 300 to 4000 Hz band-pass,
//! the FFSK tones of 1200 Hz (bit 1) and 1800 Hz (bit 0) at 1200 bit/s through the shared M-FSK demodulator with its
//! symbol clock, and the [`Framer`]).

use super::air::{Counts, Found, Framer};
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::RationalResampler;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::fsk::{Analytic, MfskConfig, MfskDemod};
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The audio rate, Hz.
pub const AUDIO_HZ: f64 = 48_000.0;
/// The bit rate, bit/s.
pub const BAUD: f64 = 1200.0;
/// The tone of a 1 bit, Hz.
pub const MARK_HZ: f64 = 1200.0;
/// The tone of a 0 bit, Hz.
pub const SPACE_HZ: f64 = 1800.0;
/// The half width of the channel, Hz (room for a transmitter error of 8 kHz).
pub const CHANNEL_HZ: f64 = 14_000.0;
/// The same frame found again within this time is dropped, s.
pub const REPEAT_S: f64 = 0.15;
/// The largest clock error the symbol clock follows, ppm.
pub const MAX_PPM: f64 = 5000.0;

/// The channel at the tune offset, mixed to 0 Hz and resampled to 48 kHz, then the FM discriminator.
pub struct Front {
    osc: Nco,
    rs: Option<RationalResampler<Complex32>>,
    mixed: Vec<Complex32>,
    chan: Vec<Complex32>,
    prev: Complex32,
    mean: f64,
    power: f64,
    started: bool,
}

impl Front {
    /// A front end at `rate_hz` for the channel `offset_hz` from the centre of the samples.
    pub fn new(rate_hz: f64, offset_hz: f64) -> Front {
        let r = rate_hz.round() as u64;
        let g = gcd(AUDIO_HZ as u64, r.max(1));
        let (l, m) = ((AUDIO_HZ as u64 / g) as usize, (r / g) as usize);
        let rs = (l != m).then(|| RationalResampler::with_spec(l, m, CHANNEL_HZ / AUDIO_HZ, 0.5, 60.0).ok()).flatten();
        Front {
            osc: Nco::new(rate_hz, -offset_hz),
            rs,
            mixed: Vec::new(),
            chan: Vec::new(),
            prev: Complex32::new(1.0, 0.0),
            mean: 0.0,
            power: 0.0,
            started: false,
        }
    }

    /// The carrier offset, Hz: the mean of the discriminator over about 0.1 s.
    pub fn offset_hz(&self) -> f64 {
        self.mean * AUDIO_HZ / TAU
    }

    /// The level, dB: the mean power over about 0.05 s.
    pub fn level_db(&self) -> f64 {
        10.0 * self.power.max(1e-30).log10()
    }

    /// Appends the discriminator output (radians per sample at 48 kHz) of `x` to `audio`.
    pub fn process(&mut self, x: &[Complex32], audio: &mut Vec<f32>) {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.osc.mix_in_place(&mut self.mixed);
        self.chan.clear();
        match self.rs.as_mut() {
            Some(r) => r.process(&self.mixed, &mut self.chan),
            None => self.chan.extend_from_slice(&self.mixed),
        }
        for &z in &self.chan {
            let d = z * self.prev.conj();
            self.prev = z;
            let f = f64::from(d.im).atan2(f64::from(d.re));
            let p = f64::from(z.norm_sqr());
            if !self.started {
                self.started = true;
                self.power = p;
            }
            self.mean += (f - self.mean) / 4800.0;
            self.power += (p - self.power) / 2400.0;
            audio.push(f as f32);
        }
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// A frame the audio decoder found.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The frame bytes.
    pub bytes: Vec<u8>,
    /// The bits corrected.
    pub corrections: u32,
    /// The air bits were inverted.
    pub inverted: bool,
    /// The audio sample at which the frame ended.
    pub sample: u64,
}

/// FFSK audio at 48 kHz to frames.
pub struct AudioDecoder {
    bandpass: Fir<f32>,
    demod: MfskDemod,
    framer: Framer,
    energies: Vec<f32>,
    filtered: Vec<f32>,
    n: u64,
    recent: VecDeque<(u64, Vec<u8>)>,
}

impl Default for AudioDecoder {
    fn default() -> Self {
        // A band-pass of 300 to 4000 Hz with no gain at DC: the difference of two low-passes of unity DC gain.
        let hi = design_by_taps(257, 4000.0 / AUDIO_HZ, Window::Hamming).expect("a low-pass");
        let lo = design_by_taps(257, 300.0 / AUDIO_HZ, Window::Hamming).expect("a low-pass");
        let bp: Vec<f32> = hi.iter().zip(&lo).map(|(a, b)| a - b).collect();
        let mut cfg = MfskConfig::from_hz(&[SPACE_HZ, MARK_HZ], BAUD, AUDIO_HZ);
        cfg.max_ppm = MAX_PPM;
        let demod = MfskDemod::new(cfg)
            .expect("the FFSK tones")
            .with_analytic(Analytic::for_band(AUDIO_HZ, 300.0).expect("a Hilbert transformer"));
        AudioDecoder {
            bandpass: Fir::new(&bp),
            demod,
            framer: Framer::default(),
            energies: Vec::new(),
            filtered: Vec::new(),
            n: 0,
            recent: VecDeque::new(),
        }
    }
}

impl AudioDecoder {
    /// The counts of the framer.
    pub fn counts(&self) -> Counts {
        self.framer.counts
    }

    /// Forgets the signal and the recent frames; the counts stay.
    pub fn reset(&mut self) {
        let counts = self.framer.counts;
        *self = AudioDecoder::default();
        self.framer.counts = counts;
    }

    /// Takes audio at 48 kHz (any scale); calls `heard` with each new frame.
    pub fn push(&mut self, audio: &[f32], mut heard: impl FnMut(Heard)) {
        self.filtered.resize(audio.len(), 0.0);
        self.bandpass.process(audio, &mut self.filtered);
        self.energies.clear();
        let _ = self.demod.process_real(&self.filtered, &mut self.energies);
        self.n += audio.len() as u64;
        let now = self.n;
        let window = (REPEAT_S * AUDIO_HZ) as u64;
        for e in self.energies.as_chunks::<2>().0 {
            let bit = u8::from(e[1] > e[0]);
            let Some(Found::Frame(bytes, corrections, inverted)) = self.framer.push(bit) else {
                continue;
            };
            self.recent.retain(|(t, _)| now.saturating_sub(*t) <= AUDIO_HZ as u64);
            if self.recent.iter().any(|(t, b)| *b == bytes && now.saturating_sub(*t) <= window) {
                continue;
            }
            self.recent.push_back((now, bytes.clone()));
            heard(Heard {
                bytes,
                corrections,
                inverted,
                sample: now,
            });
        }
    }
}
