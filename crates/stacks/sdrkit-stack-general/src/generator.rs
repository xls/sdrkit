//! The test signal generator: one clean station modulated by a steady tone or by speech-like noise, at any rate, with a
//! CTCSS tone or a DCS code on the FM when requested. Noise, offsets and the level come from the generator settings of the framework.

use crate::listen::settings::parse_dcs;
use crate::settings::{
    GenAudio, DEMOD, GEN_AUDIO, GEN_CTCSS, GEN_CTCSS_DEV, GEN_DCS, GEN_MOD, GEN_NOISE_ONLY,
    GEN_TONE, GEN_TUNE_OFFSET,
};
use crate::types::Demod;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::{Am, Fm};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::voice::Voice;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo, Values};
use sdrkit_proto_tones::ctcss::ctcss_sequence;
use sdrkit_proto_tones::dcs::DcsEncoder;
use sdrkit_proto_tones::gen::ToneSequence;
use sdrkit_util_ids::tones::dcs::Dcs;

/// The bandwidth that the SNR of the noise of the framework is measured in.
pub const SNR_BANDWIDTH_HZ: f64 = 12_500.0;
/// The deviation of the DCS bit stream, Hz.
pub const DCS_DEVIATION_HZ: f64 = 600.0;
/// How long the sub-audio sequences last before they start again, s.
const SUB_AUDIO_S: f64 = 1000.0;
/// Samples made per internal piece.
const PIECE: usize = 4096;
/// The RMS value of the voice audio before its limit at full scale, where the peak deviation or depth is reached.
pub const VOICE_RMS: f64 = 0.4;
/// The random stream of the voice, apart from the streams of the impairments.
const VOICE_STREAM: u64 = 0x60_1CE0;

/// The generator's parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// Sample rate (Hz).
    pub rate_hz: f64,
    /// The modulation.
    pub demod: Demod,
    /// No station (for squelch tests): the output is silent, and the framework's noise runs on.
    pub noise_only: bool,
    /// The position of the dial frequency in the samples (Hz): minus the tune offset of the stack.
    pub station_hz: f64,
    /// Tone (Hz).
    pub tone_hz: f64,
    /// Modulation fraction: FM deviation m x 5 kHz (NFM) or m x 75 kHz (WFM), AM depth m.
    pub m: f64,
    /// A CTCSS tone (Hz, 0 for none) and its deviation (Hz).
    pub ctcss: (f64, f64),
    /// A DCS code and its polarity (`true` is inverted), at 600 Hz deviation.
    pub dcs: Option<(Dcs, bool)>,
    /// What modulates FM and AM: the tone, or speech-like noise ([`Voice`]) at [`VOICE_RMS`] of full scale.
    pub audio: GenAudio,
    /// The seed of the voice.
    pub seed: u64,
}

impl GenParams {
    /// The defaults at `rate`: NFM at -250 kHz, 1 kHz at m 0.5, no sub-audio, seed 0.
    pub fn new(rate_hz: f64) -> Self {
        GenParams {
            rate_hz,
            demod: Demod::Nfm,
            noise_only: false,
            station_hz: -250e3,
            tone_hz: 1000.0,
            m: 0.5,
            ctcss: (0.0, 600.0),
            dcs: None,
            audio: GenAudio::Tone,
            seed: 0,
        }
    }
    /// From the settings of the stack: `demod` and the options of the test signal, with seed 0.
    pub fn from_values(rate_hz: f64, v: &Values) -> Self {
        GenParams {
            rate_hz,
            demod: v.get(&DEMOD),
            noise_only: v.get(&GEN_NOISE_ONLY),
            station_hz: -v.get(&GEN_TUNE_OFFSET),
            tone_hz: v.get(&GEN_TONE),
            m: v.get(&GEN_MOD),
            ctcss: (v.get(&GEN_CTCSS), v.get(&GEN_CTCSS_DEV)),
            dcs: parse_gen_dcs(v.get(&GEN_DCS)).ok().flatten(),
            audio: v.get(&GEN_AUDIO),
            seed: 0,
        }
    }
    /// The voice modulates the signal: [`GenAudio::Voice`] on FM or AM.
    pub fn voice(&self) -> bool {
        self.audio == GenAudio::Voice && (self.demod.is_fm() || self.demod == Demod::Am)
    }
    /// The mean power of the clean signal while it transmits: 1 (a unit carrier), or for AM 1 + m^2 / 2 with the tone and
    /// 1 + m^2 [`VOICE_RMS`]^2 / 4 with the voice (half of the time speaks, with the mean square of a half sine).
    pub fn power(&self) -> f64 {
        match self.demod {
            Demod::Am if self.voice() => 1.0 + self.m * self.m * VOICE_RMS * VOICE_RMS / 4.0,
            Demod::Am => 1.0 + self.m * self.m / 2.0,
            _ => 1.0,
        }
    }
}

/// Reads a DCS code of the test signal: empty for none, else three octal digits with `N` or `I` for the polarity ("023N").
///
/// # Errors
///
/// The byte position of the code when it does not parse.
pub fn parse_gen_dcs(text: &str) -> Result<Option<(Dcs, bool)>, usize> {
    let lead = text.len() - text.trim_start().len();
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let inverted = t.ends_with(['I', 'i']);
    parse_dcs(t.trim_end_matches(['N', 'n', 'I', 'i']))
        .map(|c| Some((c, inverted)))
        .ok_or(lead)
}

/// The generator: a unit carrier at `station_hz`, modulated as `demod` says. It never allocates after [`GeneralGenerator::new`].
pub struct GeneralGenerator {
    p: GenParams,
    station: bool,
    /// The modulating tone (FM and AM).
    tone: Nco,
    /// The voice, in place of the tone on FM and AM.
    voice: Option<Voice>,
    /// The carrier (or, for USB and LSB, the tone on its side of the suppressed carrier).
    carrier: Nco,
    fm: Option<Fm>,
    am: Option<Am>,
    ctcss: Option<ToneSequence>,
    dcs: Option<DcsEncoder>,
    audio: Vec<f32>,
    sub: Vec<f32>,
    base: Vec<Complex32>,
}

impl GeneralGenerator {
    /// A generator with these parameters.
    pub fn new(p: GenParams) -> Self {
        let rate = p.rate_hz;
        let side = match p.demod {
            Demod::Usb => p.tone_hz,
            Demod::Lsb => -p.tone_hz,
            _ => 0.0,
        };
        let fm = p
            .demod
            .is_fm()
            .then(|| Fm::new(rate, 1.0).expect("a positive rate"));
        let am = (p.demod == Demod::Am)
            .then(|| Am::new(p.m.clamp(0.0, 10.0)).expect("a depth of 0 to 10"));
        let ctcss = (p.demod.is_fm() && p.ctcss.0 > 0.0)
            .then(|| ctcss_sequence(rate, p.ctcss.0, p.ctcss.1 as f32, SUB_AUDIO_S, None).ok())
            .flatten();
        let dcs = p.dcs.filter(|_| p.demod.is_fm()).and_then(|(c, inv)| {
            DcsEncoder::new(rate, c, inv, DCS_DEVIATION_HZ as f32, SUB_AUDIO_S, false).ok()
        });
        let voice = p
            .voice()
            .then(|| Voice::new(rate, VOICE_RMS, Rng::stream(p.seed, VOICE_STREAM)).ok())
            .flatten();
        GeneralGenerator {
            tone: Nco::new(rate, p.tone_hz),
            voice,
            carrier: Nco::new(rate, p.station_hz + side),
            fm,
            am,
            ctcss,
            dcs,
            station: !p.noise_only,
            audio: vec![0.0; PIECE],
            sub: vec![0.0; PIECE],
            base: vec![Complex32::default(); PIECE],
            p,
        }
    }
    /// From an engine's generator settings and seed.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Self::new(GenParams {
            seed: cfg.seed,
            ..GenParams::from_values(cfg.rate_hz, &cfg.settings)
        })
    }
    /// The parameters.
    pub fn params(&self) -> &GenParams {
        &self.p
    }
    /// Switches the station on or off (the framework's noise runs on): for sequences such as noise, station, noise. With
    /// `noise_only` the station stays off.
    pub fn set_station(&mut self, on: bool) {
        self.station = on && !self.p.noise_only;
    }
    /// Fills the first `n` samples of the audio with the voice, else with the tone, in `[-1, 1]`.
    fn fill_audio(&mut self, n: usize) {
        match self.voice.as_mut() {
            Some(v) => v.fill(&mut self.audio[..n]),
            None => {
                for a in self.audio[..n].iter_mut() {
                    *a = self.tone.next_sample().im;
                }
            }
        }
    }
    fn piece(&mut self, out: &mut [Complex32]) {
        let n = out.len();
        let dev = self.p.m
            * if self.p.demod == Demod::Wfm {
                75e3
            } else {
                5e3
            };
        if self.fm.is_some() {
            // The frequency in Hz: the tone or the voice, then the sub-audio tone or code (the sequences restart when they
            // end).
            self.fill_audio(n);
            for a in self.audio[..n].iter_mut() {
                *a = (dev * *a as f64) as f32;
            }
            if let Some(seq) = self.ctcss.as_mut() {
                fill_tones(seq, &mut self.sub[..n]);
                add(&mut self.audio[..n], &self.sub[..n]);
            }
            if let Some(enc) = self.dcs.as_mut() {
                fill_dcs(enc, &mut self.sub[..n]);
                add(&mut self.audio[..n], &self.sub[..n]);
            }
            if let Some(fm) = self.fm.as_mut() {
                let _ = fm.process(&self.audio[..n], &mut self.base[..n]);
            }
        } else if self.am.is_some() {
            self.fill_audio(n);
            if let Some(am) = &self.am {
                let _ = am.process(&self.audio[..n], &mut self.base[..n]);
            }
        } else {
            self.base[..n]
                .iter_mut()
                .for_each(|z| *z = Complex32::new(1.0, 0.0));
        }
        for (o, b) in out.iter_mut().zip(&self.base[..n]) {
            let c = self.carrier.next_sample();
            *o = if self.station {
                *b * c
            } else {
                Complex32::default()
            };
        }
    }
}

/// Adds `b` to `a`, sample by sample.
fn add(a: &mut [f32], b: &[f32]) {
    for (x, y) in a.iter_mut().zip(b) {
        *x += *y;
    }
}

/// Fills `out` from a tone sequence, starting it again when it ends.
fn fill_tones(s: &mut ToneSequence, out: &mut [f32]) {
    let mut k = s.fill(out);
    while k < out.len() {
        s.rewind();
        k += s.fill(&mut out[k..]).max(1);
    }
}

/// Fills `out` from a DCS encoder. It lasts 1000 s, far longer than any test, and returns zeros after that.
fn fill_dcs(s: &mut DcsEncoder, out: &mut [f32]) {
    s.fill(out);
}

impl Generator for GeneralGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate_hz
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(SNR_BANDWIDTH_HZ).with_power(self.p.power()))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for c in out.chunks_mut(PIECE) {
            self.piece(c);
        }
        out.len()
    }
}
