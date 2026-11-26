//! The analogue TV modulator: the clean channel (composite video on the vision carrier through the vestigial-sideband filter,
//! the sound carrier, and the A2 or NICAM-728 second sound) at any output rate. It is the test-signal generator of `atv` and
//! the core of its sender, which gives it the pictures ([`AtvGenerator::show`]) and the sound
//! ([`AtvGenerator::push_programme`]) of the host. Noise, offsets, clock errors, echoes and hum come from the framework's
//! `gen_*` settings (`sdrkit-algo-sim`). The same configuration produces the same samples, however the caller cuts them.

use crate::card::{Pattern, Planes};
use crate::gen_video::{internal_rate, SecamIdent, VideoGen, VideoOptions};
use crate::sound_gen::{
    pre_emphasis, programme_due, HostAudio, NicamTx, Programme, SecondSound, A2_DUAL_ID, A2_PILOT,
    A2_SPACING, A2_STEREO_ID, PROGRAMME_STEP,
};
use crate::systems::{Colour, System, SystemId};
use sdrkit_algo_dsp::iir::PreEmphasis;
use sdrkit_algo_dsp::resample::FilterShiftResampler;
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};
use std::f64::consts::TAU;

/// Everything the generator is made with.
#[derive(Clone, Debug)]
pub struct AtvGenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// The system.
    pub system: SystemId,
    /// The colour (`None`: the system's own).
    pub colour: Option<Colour>,
    /// The picture.
    pub pattern: Pattern,
    /// The programme.
    pub sound: Programme,
    /// The programme is the host's audio ([`AtvGenerator::push_programme`]) in place of `sound`, limited to full deviation
    /// after the pre-emphasis. The sound carrier is there unless `sound` is [`Programme::NoCarrier`].
    pub host_sound: bool,
    /// The second sound.
    pub second: SecondSound,
    /// Sync compression of the transmitter, 0 to 0.8.
    pub sync_compression: f64,
    /// Chroma group-delay pre-correction (170 ns).
    pub group_delay: bool,
    /// The modulator already has the Nyquist slope.
    pub nyquist_slope: bool,
    /// 7.5 IRE setup on 525 lines.
    pub setup: bool,
    /// SECAM line identification.
    pub secam_ident: SecamIdent,
    /// Ident text.
    pub ident: String,
    /// Peak carrier amplitude.
    pub level: f64,
    /// Clock at the first sample, seconds.
    pub start_s: f64,
    /// The offset of the channel centre in the output, Hz (0: the channel centre at 0 Hz).
    pub centre_offset_hz: f64,
    /// Teletext in the field blanking (625 lines).
    pub teletext: bool,
}

impl Default for AtvGenParams {
    fn default() -> Self {
        AtvGenParams {
            rate_hz: 10e6,
            system: SystemId::G,
            colour: None,
            pattern: Pattern::Card,
            sound: Programme::ToneGaps,
            host_sound: false,
            second: SecondSound::None,
            sync_compression: 0.0,
            group_delay: true,
            nyquist_slope: false,
            setup: true,
            secam_ident: SecamIdent::Both,
            ident: "TEST CARD".into(),
            level: 0.4,
            start_s: 0.0,
            centre_offset_hz: 0.0,
            teletext: false,
        }
    }
}

impl AtvGenParams {
    /// The colour in use.
    pub fn colour(&self) -> Colour {
        self.colour
            .unwrap_or(System::get(self.system).default_colour)
    }
    /// Whether this combination existed (colour on A or E, NTSC on a positive system, ... are refused).
    pub fn valid(&self) -> bool {
        let s = System::get(self.system);
        s.allows(self.colour())
            && self.rate_hz >= 2e6
            && (!self.second.is_a2()
                || matches!(
                    self.system,
                    SystemId::B | SystemId::G | SystemId::DK | SystemId::H | SystemId::Kp
                ))
    }
}

/// The analogue TV generator.
pub struct AtvGenerator {
    p: AtvGenParams,
    sys: System,
    ok: bool,
    video: VideoGen,
    /// The vestigial-sideband filter, the move of the vision carrier to its place and the change to the output rate.
    vsb: FilterShiftResampler,
    vbuf: Vec<f32>,
    pending: Vec<Complex32>,
    pos: usize,
    // Output-rate sound.
    k: u64,
    dt: f64,
    sound_phase: f64,
    a2_phase: f64,
    pilot_phase: f64,
    nicam_phase: f64,
    pre: [Option<PreEmphasis>; 2],
    prog: [f64; 3],
    prog_next: [f64; 3],
    nicam: Option<NicamTx>,
    vision_hz: f64,
    host_audio: HostAudio,
}

impl AtvGenerator {
    /// Creates a generator. An invalid combination produces silence (see [`AtvGenerator::is_valid`]).
    pub fn new(p: AtvGenParams) -> Self {
        let sys = System::get(p.system);
        let colour = p.colour();
        let ok = p.valid();
        let video = VideoGen::new(&VideoOptions {
            system: p.system,
            colour,
            pattern: p.pattern,
            ident: p.ident.clone(),
            setup: p.setup,
            secam_ident: p.secam_ident,
            group_delay: p.group_delay,
            start_s: p.start_s,
            teletext: p.teletext,
        });
        let fi = internal_rate(p.system);
        // The vestigial-sideband filter, for the main sideband above (or below, turned round) the carrier.
        let vest = sys.vestige_mhz * 1e6;
        // SECAM on M and N has a 5 MHz video band (its subcarriers reach 4.756 MHz) and no sound carrier.
        let video_mhz = if colour == Colour::Secam && matches!(p.system, SystemId::M | SystemId::N)
        {
            5.0
        } else {
            sys.video_mhz
        };
        let top = (video_mhz * 1e6).min(0.5 * fi - 0.6e6);
        let turned = sys.turned_round();
        let nyq = p.nyquist_slope;
        let resp = move |f: f64| -> f64 {
            let f = if turned { -f } else { f };
            let skirt = 0.5e6;
            let upper = if f <= top {
                1.0
            } else if f < top + skirt {
                (std::f64::consts::FRAC_PI_2 * (f - top) / skirt)
                    .cos()
                    .powi(2)
            } else {
                0.0
            };
            let lower = if nyq {
                // A Nyquist slope across the carrier.
                ((f + vest) / (2.0 * vest)).clamp(0.0, 1.0)
            } else if f >= -vest {
                1.0
            } else if f > -vest - skirt {
                (std::f64::consts::FRAC_PI_2 * (-vest - f) / skirt)
                    .cos()
                    .powi(2)
            } else {
                0.0
            };
            upper.min(lower)
        };
        let taps = sdrkit_algo_dsp::fir::design_complex_by_response(
            97,
            fi,
            crate::util::DESIGN_BETA,
            |f| num_complex::Complex64::new(resp(f), 0.0),
        )
        .expect("the generator's IF rate is positive");
        let vision_hz = sys.vision_from_centre_mhz() * 1e6 + p.centre_offset_hz;
        // The content reaches from the vestige to the video top around the carrier.
        let lo = vision_hz - if turned { top + 0.5e6 } else { vest + 0.5e6 };
        let hi = vision_hz + if turned { vest + 0.5e6 } else { top + 0.5e6 };
        let extent = lo.abs().max(hi.abs());
        let pass = extent.min(0.47 * p.rate_hz).min(0.45 * fi);
        let stop = (p.rate_hz - pass).min(0.5 * fi).max(pass + 0.3e6);
        let low = fi.min(p.rate_hz);
        let vsb = FilterShiftResampler::new(&taps, vision_hz, fi, p.rate_hz, pass, stop, 55.0)
            .or_else(|_| {
                FilterShiftResampler::new(&taps, vision_hz, fi, p.rate_hz, 0.42 * low, 0.5 * low, 80.0)
            })
            .expect("rates");
        let rate_true = p.rate_hz;
        let deemph = if sys.am_sound {
            0.0
        } else {
            sys.deemph_us * 1e-6
        };
        let prog_rate = rate_true / PROGRAMME_STEP as f64;
        let nicam = p.second.is_nicam().then(|| {
            NicamTx::new(
                p.second == SecondSound::NicamDual,
                if p.system == SystemId::I { 1.0 } else { 0.4 },
            )
        });
        AtvGenerator {
            sys,
            ok,
            video,
            vsb,
            vbuf: vec![0.0; 4096],
            pending: Vec::new(),
            pos: 0,
            k: 0,
            dt: 1.0 / rate_true,
            sound_phase: 0.0,
            a2_phase: 0.0,
            pilot_phase: 0.0,
            nicam_phase: 0.0,
            pre: [
                pre_emphasis(deemph, prog_rate),
                pre_emphasis(deemph, prog_rate),
            ],
            prog: [0.0; 3],
            prog_next: [0.0; 3],
            nicam,
            vision_hz,
            host_audio: HostAudio::default(),
            p,
        }
    }

    /// Whether the combination is one that existed.
    pub fn is_valid(&self) -> bool {
        self.ok
    }
    /// The parameters.
    pub fn params(&self) -> &AtvGenParams {
        &self.p
    }
    /// The vision carrier position in the output, Hz.
    pub fn vision_hz(&self) -> f64 {
        self.vision_hz
    }
    /// The test card as sent (the last drawn).
    pub fn card(&self) -> &Planes {
        self.video.card()
    }
    /// The system.
    pub fn system(&self) -> &System {
        &self.sys
    }
    /// Shows a picture of the host from the next field on (`None`: the test picture again); see [`VideoGen::show`].
    pub fn show(&mut self, picture: Option<Planes>) {
        self.video.show(picture);
    }
    /// The pictures of the host taken so far.
    pub fn pictures_taken(&self) -> u64 {
        self.video.pictures_taken()
    }
    /// The rate of the programme that [`AtvGenerator::push_programme`] takes: the output rate over [`PROGRAMME_STEP`].
    pub fn programme_rate(&self) -> f64 {
        self.p.rate_hz / PROGRAMME_STEP as f64
    }
    /// The programme samples that the next `n` output samples read.
    pub fn programme_due(&self, n: usize) -> usize {
        programme_due(self.k, n)
    }
    /// Adds host audio at [`AtvGenerator::programme_rate`] for a generator made with `host_sound`.
    pub fn push_programme(&mut self, x: &[f32]) {
        self.host_audio.push(x);
    }

    /// Makes another piece of vision signal at the output rate.
    fn vision_block(&mut self) {
        self.video.fill(&mut self.vbuf);
        let sys = self.sys;
        let s = sys.raster.sync_level;
        let comp = self.p.sync_compression;
        let level = self.p.level;
        let limit = |x: f32| {
            let mut v = x as f64;
            if v < 0.0 {
                v *= 1.0 - comp;
            }
            v.clamp(1.2 * s, 1.3)
        };
        if sys.positive {
            for x in &mut self.vbuf {
                *x = (level * ((limit(*x) - s) / (1.0 - s))) as f32;
            }
        } else {
            let m = 1.0 - sys.peak_white;
            for x in &mut self.vbuf {
                *x = (level * (1.0 - m * (limit(*x) - s) / (1.0 - s))) as f32;
            }
        }
        if self.pos > 0 {
            self.pending.drain(..self.pos);
            self.pos = 0;
        }
        self.vsb.process(&self.vbuf, &mut self.pending);
    }

    fn programme_at(&mut self, t: f64) -> [f64; 3] {
        if self.p.host_sound {
            let x = self.host_audio.take();
            return [x, x, 0.0];
        }
        let p = self.p.sound;
        let tone = |f: f64, t: f64| 0.25 * (TAU * f * t).sin();
        match self.p.second {
            SecondSound::A2Stereo => {
                let l = tone(1000.0, t);
                let r = tone(1500.0, t);
                [(l + r) / 2.0, r, 0.0]
            }
            SecondSound::A2Dual => [tone(1000.0, t), tone(1500.0, t), 0.0],
            _ => [p.value(t), p.value(t), 0.0],
        }
    }

    /// Adds the sound carrier, the A2 carrier and NICAM to the vision samples `x` (at most [`SOUND_CHUNK`]). A first pass
    /// steps the programme and the phases sample by sample; a second makes the carriers and adds them in the order sound,
    /// A2, NICAM.
    fn add_sound(&mut self, x: &mut [Complex32]) {
        let n = x.len();
        let sys = self.sys;
        let level = self.p.level;
        let secam_mn =
            self.p.colour() == Colour::Secam && matches!(self.p.system, SystemId::M | SystemId::N);
        let sound_amp = if self.p.sound == Programme::NoCarrier || secam_mn {
            0.0
        } else {
            level * 10f64.powf(sys.sound_db / 20.0)
        };
        let sound_hz = self.vision_hz + sys.spacing_mhz * 1e6;
        let a2 = self.p.second.is_a2();
        // The second A2 carrier at -20 dB against the peak vision carrier.
        let a2_amp = level * 0.1;
        // NICAM 728 at -20 dB against the peak vision carrier.
        let nicam_amp = (level * 0.1) as f32;
        let nicam_hz =
            self.vision_hz + sys.nicam_hz() * if sys.turned_round() { -1.0 } else { 1.0 };
        let dev = sys.deviation_hz;
        let dt = self.dt;
        // The phase (cycles) and the amplitude of the sound carrier, and the A2 and NICAM carriers, of each sample.
        let mut phase = [0.0f64; SOUND_CHUNK];
        let mut amp = [0.0f32; SOUND_CHUNK];
        let mut a2_out = [Complex32::default(); SOUND_CHUNK];
        let mut nicam_out = [Complex32::default(); SOUND_CHUNK];
        let start_s = self.p.start_s;
        let time = |k: u64| k as f64 * dt + start_s;
        for j in 0..n {
            if self.k.is_multiple_of(PROGRAMME_STEP) {
                let t = time(self.k);
                self.prog = self.prog_next;
                let tn = t + PROGRAMME_STEP as f64 * dt;
                let pr = self.programme_at(tn);
                let emph = |p: &mut Option<PreEmphasis>, x: f64| match p {
                    Some(p) => p.process_sample(x as f32) as f64,
                    None => x,
                };
                let [p0, p1] = &mut self.pre;
                self.prog_next = [emph(p0, pr[0]), emph(p1, pr[1]), 0.0];
                if self.p.host_sound {
                    for x in &mut self.prog_next {
                        *x = x.clamp(-1.0, 1.0);
                    }
                }
            }
            let frac = (self.k % PROGRAMME_STEP) as f64 / PROGRAMME_STEP as f64;
            let p0 = self.prog[0] + (self.prog_next[0] - self.prog[0]) * frac;
            if sound_amp > 0.0 {
                if sys.am_sound {
                    self.sound_phase = (self.sound_phase + sound_hz * dt).fract();
                    amp[j] = (sound_amp * (1.0 + 0.8 * p0)) as f32;
                } else {
                    self.sound_phase = wrap(self.sound_phase + (sound_hz + dev * p0) * dt);
                    amp[j] = sound_amp as f32;
                }
                phase[j] = self.sound_phase;
            }
            if a2 {
                let p1 = self.prog[1] + (self.prog_next[1] - self.prog[1]) * frac;
                let id = match self.p.second {
                    SecondSound::A2Stereo => A2_STEREO_ID,
                    SecondSound::A2Dual => A2_DUAL_ID,
                    _ => 0.0,
                };
                let am = if id > 0.0 {
                    0.5 + 0.5 * (TAU * id * time(self.k)).sin()
                } else {
                    1.0
                };
                self.pilot_phase = (self.pilot_phase + A2_PILOT * dt).fract();
                let pilot = 2.5e3 * am * (TAU * self.pilot_phase).sin();
                let f = sound_hz + A2_SPACING + 50e3 * p1 + pilot;
                self.a2_phase = wrap(self.a2_phase + f * dt);
                let (s, c) = sdrkit_algo_dsp::approx::sin_cos(TAU * self.a2_phase);
                a2_out[j] = Complex32::new(c, s) * a2_amp as f32;
            }
            if let Some(nc) = &mut self.nicam {
                let b = nc.at(time(self.k));
                self.nicam_phase = wrap(self.nicam_phase + nicam_hz * dt);
                let (s, c) = sdrkit_algo_dsp::approx::sin_cos(TAU * self.nicam_phase);
                nicam_out[j] = b * Complex32::new(c, s) * nicam_amp;
            }
            self.k += 1;
        }
        if sound_amp > 0.0 {
            for ((y, &ph), &a) in x.iter_mut().zip(&phase[..n]).zip(&amp[..n]) {
                let (s, c) = sdrkit_algo_dsp::approx::sin_cos(TAU * ph);
                *y += Complex32::new(c, s) * a;
            }
        }
        if a2 {
            for (y, z) in x.iter_mut().zip(&a2_out[..n]) {
                *y += *z;
            }
        }
        if self.nicam.is_some() {
            for (y, z) in x.iter_mut().zip(&nicam_out[..n]) {
                *y += *z;
            }
        }
    }
}

/// The samples [`AtvGenerator::add_sound`] works on at a time.
const SOUND_CHUNK: usize = 256;

impl Generator for AtvGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }

    /// The stated power is the peak power of the vision carrier (the sync tip, or peak white on a positive system), and the
    /// bandwidth is 5 MHz. So `gen_snr_db` is the carrier-to-noise ratio that television uses: the peak vision carrier against
    /// the noise in 5 MHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(5e6).with_power(self.p.level * self.p.level))
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if !self.ok {
            out.fill(Complex32::default());
            return out.len();
        }
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.pending.len() {
                self.vision_block();
                continue;
            }
            let n = (self.pending.len() - self.pos).min(out.len() - i);
            for o in out[i..i + n].chunks_mut(SOUND_CHUNK) {
                let m = o.len();
                o.copy_from_slice(&self.pending[self.pos..self.pos + m]);
                self.pos += m;
                self.add_sound(o);
            }
            i += n;
        }
        out.len()
    }
}

/// `x` less its whole part, in `[0, 1]`: the same value as `x.rem_euclid(1.0)` without its division.
#[inline]
fn wrap(x: f64) -> f64 {
    x - x.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_output_does_not_depend_on_the_cuts() {
        let p = AtvGenParams {
            second: SecondSound::NicamStereo,
            ..Default::default()
        };
        let mut a = AtvGenerator::new(p.clone());
        let mut b = AtvGenerator::new(p);
        let mut x = vec![Complex32::default(); 30_000];
        a.generate(&mut x);
        let mut y = vec![Complex32::default(); 30_000];
        let mut i = 0;
        for n in [1usize, 7, 333, 4096, 9999, 15564] {
            b.generate(&mut y[i..i + n]);
            i += n;
        }
        assert_eq!(x, y);
    }

    #[test]
    fn invalid_combinations_are_refused() {
        assert!(!AtvGenerator::new(AtvGenParams {
            system: SystemId::A,
            colour: Some(Colour::Pal),
            ..Default::default()
        })
        .is_valid());
        assert!(!AtvGenerator::new(AtvGenParams {
            system: SystemId::L,
            colour: Some(Colour::Ntsc),
            ..Default::default()
        })
        .is_valid());
        assert!(AtvGenerator::new(AtvGenParams {
            system: SystemId::M,
            colour: Some(Colour::PalM),
            ..Default::default()
        })
        .is_valid());
        assert!(AtvGenerator::new(AtvGenParams {
            system: SystemId::G,
            colour: Some(Colour::Mono),
            ..Default::default()
        })
        .is_valid());
    }
}
