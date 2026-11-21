//! The voice codec 4 frame decoder: frame types, comfort noise, the band merge and the conversion to PCM.

use super::core::CoreDecoder;
use super::enhancer::{Enhancer, EnhancerMode};
use super::frame::{self, Damage, Sid};
use super::highband::{t, HbDecoder, Levels};
use super::layout::{Config, Layout};
use super::lpc::{lsf_to_lpc, synth_step, weight, ORDER};
use super::math::{lin, Lcg, Mdct};
use super::tables::SPLIT_TAPS;
use crate::Error;

/// What the container delivers for one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input<'a> {
    /// The frame bytes. An empty slice means "no data" (DTX).
    Frame(&'a [u8]),
    /// The frame is lost.
    Lost,
}

/// What the decoder made of one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Decoded {
    /// A speech frame.
    Speech,
    /// A SID frame: comfort noise with new parameters.
    Sid,
    /// Comfort noise without new parameters.
    ComfortNoise,
    /// Concealment of a lost frame.
    Concealed,
}

/// The 8 kHz to 16 kHz interpolator with gain 2 (section 3.7.1 step 1).
#[derive(Clone)]
pub(crate) struct Upsampler {
    buf: [f32; 47 + 320],
}

impl Upsampler {
    pub(crate) fn new() -> Self {
        Self {
            buf: [0.0; 47 + 320],
        }
    }

    /// Interpolates `x` (at most 320 samples) into `out` (twice as long).
    pub(crate) fn run(&mut self, x: &[f32], out: &mut [f32]) {
        let n = x.len();
        self.buf[47..47 + n].copy_from_slice(x);
        for m in 0..n {
            let mut even = 0f32;
            let mut odd = 0f32;
            for i in 0..48 {
                let v = self.buf[47 + m - i];
                even += SPLIT_TAPS[2 * i] * v;
                if i < 47 {
                    odd += SPLIT_TAPS[2 * i + 1] * v;
                }
            }
            out[2 * m] = 2.0 * even;
            out[2 * m + 1] = 2.0 * odd;
        }
        self.buf.copy_within(n..n + 47, 0);
    }
}

/// Converts a float sample to PCM with the soft limit above 28000 (section 3.7.1 step 3).
pub(crate) fn to_pcm(v: f32) -> i16 {
    let a = v.abs();
    let mut v = v;
    if !(a <= 28000.0) {
        let t = a.min(1e9) - 28000.0;
        let y = 28000.0 + 4767.0 * t / (t + 4767.0);
        v = if v < 0.0 { -y } else { y };
        if a.is_nan() {
            v = 0.0;
        }
    }
    v.clamp(-32768.0, 32767.0).round() as i16
}

/// The decoder settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DecoderSettings {
    /// The core post-filter (on by default). When it is off, the enhancer post-filter stages are off too.
    pub postfilter: bool,
    /// The decoder-side enhancer (automatic by default).
    pub enhancer: EnhancerMode,
    /// The noise reduction stage of the enhancer (off by default). It adds one hop of delay: 80 samples at 16 kHz, 40 at 8 kHz.
    pub noise_reduction: bool,
}

impl Default for DecoderSettings {
    fn default() -> Self {
        Self {
            postfilter: true,
            enhancer: EnhancerMode::Auto,
            noise_reduction: false,
        }
    }
}

impl DecoderSettings {
    /// The plain decoder: the core post-filter on and the enhancer off. This is the conformance target.
    pub const PLAIN: Self = Self {
        postfilter: true,
        enhancer: EnhancerMode::Off,
        noise_reduction: false,
    };
}

/// The voice codec 4 decoder.
pub struct Decoder {
    lay: Layout,
    settings: DecoderSettings,
    core: CoreDecoder,
    hb: HbDecoder,
    mdct: Mdct,
    up: Upsampler,
    enh: Option<Box<Enhancer>>,
    cn_on: bool,
    have_sid: bool,
    cur: Sid,
    target: Sid,
    cn_mem: [f32; ORDER],
    cn_rng: Lcg,
    lb: [f32; 320],
    fade: [f32; 320],
    hbuf: [f32; 640],
    out16: [f32; 640],
    params: frame::Params,
}

impl Decoder {
    /// Makes a decoder for `cfg` with `settings`.
    ///
    /// Errors: [`Error::BadConfig`] when the configuration has no layout.
    pub fn new(cfg: Config, settings: DecoderSettings) -> Result<Self, Error> {
        let lay = Layout::new(cfg)?;
        let enh = Enhancer::wanted(&lay, settings).then(|| Box::new(Enhancer::new(&lay, settings)));
        Ok(Self {
            core: CoreDecoder::new(&lay),
            lay,
            settings,
            hb: HbDecoder::new(),
            mdct: Mdct::new(),
            up: Upsampler::new(),
            enh,
            cn_on: false,
            have_sid: false,
            cur: Sid::reset(),
            target: Sid::reset(),
            cn_mem: [0.0; ORDER],
            cn_rng: Lcg(0x434E_4721),
            lb: [0.0; 320],
            fade: [0.0; 320],
            hbuf: [0.0; 640],
            out16: [0.0; 640],
            params: frame::Params::default(),
        })
    }

    /// The frame layout.
    pub fn layout(&self) -> &Layout {
        &self.lay
    }

    /// Samples per frame at the codec rate.
    pub fn frame_samples(&self) -> usize {
        self.lay.frame_samples
    }

    /// Decodes one frame into `out` (one frame of PCM). `damage` marks damaged bit classes of a frame.
    ///
    /// A frame of a wrong length, or with class A damaged, is concealed as lost.
    ///
    /// Errors: [`Error::OutputTooShort`] when `out` is shorter than one frame.
    pub fn decode(
        &mut self,
        input: Input<'_>,
        damage: Damage,
        out: &mut [i16],
    ) -> Result<Decoded, Error> {
        let n = self.lay.frame_samples;
        if out.len() < n {
            return Err(Error::OutputTooShort { need: n });
        }
        enum Kind<'a> {
            Lost,
            NoData,
            Sid(&'a [u8], usize),
            Speech(&'a [u8]),
        }
        let lay = &self.lay;
        let kind = match input {
            Input::Lost => Kind::Lost,
            Input::Frame([]) => Kind::NoData,
            Input::Frame(d) if damage.a => {
                let _ = d;
                Kind::Lost
            }
            Input::Frame(d) => {
                if lay.cfg.type_in_band {
                    if d.len() != lay.payload_bytes {
                        Kind::Lost
                    } else if d[0] & 0x80 != 0 {
                        Kind::Sid(d, 1)
                    } else {
                        Kind::Speech(d)
                    }
                } else if d.len() == 5 {
                    Kind::Sid(d, 0)
                } else if d.len() == lay.speech_bytes() {
                    Kind::Speech(d)
                } else {
                    Kind::Lost
                }
            }
        };
        let result = match kind {
            Kind::Lost | Kind::NoData => {
                let nodata = matches!(kind, Kind::NoData);
                if self.cn_on || (nodata && self.have_sid) {
                    self.comfort_frame();
                    Decoded::ComfortNoise
                } else {
                    self.conceal_frame();
                    Decoded::Concealed
                }
            }
            Kind::Sid(d, off) => {
                let idx = frame::sid_unpack(d, off);
                self.target = frame::sid_dequant(&idx);
                if !self.cn_on {
                    self.cur = self.target;
                    for i in 0..ORDER {
                        self.cur.lsf[i] = 0.5 * self.core.prev_lsf[i] + 0.5 * self.target.lsf[i];
                    }
                }
                self.have_sid = true;
                self.cn_on = true;
                self.comfort_frame();
                Decoded::Sid
            }
            Kind::Speech(d) => {
                let p = frame::unpack(&self.lay, d, damage);
                self.speech_frame(&p);
                Decoded::Speech
            }
        };
        for (o, &v) in out[..n].iter_mut().zip(&self.out16[..n]) {
            *o = to_pcm(v);
        }
        Ok(result)
    }

    fn comfort_low(&mut self, out_fade: bool) {
        let nc = self.lay.nc;
        let comfort_weight = self.enh.as_ref().is_some_and(|e| e.comfort_on());
        let buf = if out_fade {
            &mut self.fade
        } else {
            &mut self.lb
        };
        for blk in 0..nc / 40 {
            for i in 0..ORDER {
                self.cur.lsf[i] = 0.9 * self.cur.lsf[i] + 0.1 * self.target.lsf[i];
            }
            self.cur.res = 0.9 * self.cur.res + 0.1 * self.target.res;
            let mut a = lsf_to_lpc(&self.cur.lsf);
            if comfort_weight {
                a = weight(&a, 0.92);
            }
            let g = lin(self.cur.res).sqrt() as f32;
            for n in 0..40 {
                let e = g * self.cn_rng.gauss();
                buf[blk * 40 + n] = synth_step(&a, &mut self.cn_mem, e);
            }
        }
    }

    fn comfort_frame(&mut self) {
        self.comfort_low(false);
        let nc = self.lay.nc;
        if let Some(e) = self.enh.as_mut() {
            e.comfort_low(&self.lay, &mut self.lb[..nc]);
        }
        self.cur.hb = 0.8 * self.cur.hb + 0.2 * self.target.hb;
        self.cur.tilt = 0.8 * self.cur.tilt + 0.2 * self.target.tilt;
        let nb = 4 * self.lay.halves;
        let mut levels: Levels = [[0.0; 6]; 8];
        for lv in levels.iter_mut().take(nb) {
            for (j, v) in lv.iter_mut().enumerate() {
                *v = self.cur.hb + self.cur.tilt * t(j);
            }
        }
        if self.lay.hb_on {
            if let Some(e) = self.enh.as_mut() {
                e.comfort_high(&self.lay, &levels[..nb], &mut self.hb, &mut self.hbuf);
            } else {
                self.hb.synth(&self.mdct, &levels[..nb], &mut self.hbuf);
                self.hb.last = levels[0];
                self.hb.have = true;
            }
        }
        self.merge(false, true);
    }

    fn conceal_frame(&mut self) {
        let pf = self.settings.postfilter && self.enh.is_none();
        let nc = self.lay.nc;
        self.core.conceal(&self.lay, pf, &mut self.lb[..nc]);
        if let Some(e) = self.enh.as_mut() {
            e.low_band(&self.lay, &self.core, true, &mut self.lb[..nc]);
        }
        self.highband(false);
        self.merge(false, false);
    }

    fn speech_frame(&mut self, p: &frame::Params) {
        let after_cn = self.cn_on;
        if after_cn {
            let lsf = if self.have_sid {
                self.target.lsf
            } else {
                self.core.prev_lsf
            };
            self.core.restart(&lsf);
            if let Some(e) = self.enh.as_mut() {
                e.restart();
            }
            self.comfort_low(true);
            self.cn_on = false;
        }
        let pf = self.settings.postfilter && self.enh.is_none();
        let nc = self.lay.nc;
        self.core.decode(&self.lay, p, pf, &mut self.lb[..nc]);
        if let Some(e) = self.enh.as_mut() {
            e.low_band(&self.lay, &self.core, false, &mut self.lb[..nc]);
        }
        self.params = *p;
        self.highband(true);
        if after_cn {
            for i in 0..nc {
                self.lb[i] += self.fade[i] * (1.0 - i as f32 / nc as f32);
            }
        }
        self.merge(true, false);
    }

    fn highband(&mut self, good: bool) {
        if !self.lay.hb_on {
            return;
        }
        let mut levels: Levels = [[0.0; 6]; 8];
        for h in 0..self.lay.halves {
            let p = if good { Some((&self.params, h)) } else { None };
            self.hb
                .half_levels(&self.lay, p, &mut levels[4 * h..4 * h + 4]);
        }
        let nb = 4 * self.lay.halves;
        if let Some(e) = self.enh.as_mut() {
            e.highband(&self.lay, Some(&self.core), &levels[..nb], &mut self.hbuf);
        } else {
            self.hb.synth(&self.mdct, &levels[..nb], &mut self.hbuf);
        }
    }

    fn merge(&mut self, speech: bool, comfort: bool) {
        let nc = self.lay.nc;
        let n = self.lay.frame_samples;
        if self.lay.hb_on {
            self.up.run(&self.lb[..nc], &mut self.out16[..n]);
            for i in 0..n {
                self.out16[i] += self.hbuf[i];
            }
        } else {
            self.out16[..n].copy_from_slice(&self.lb[..nc]);
        }
        if let Some(e) = self.enh.as_mut() {
            e.output(&self.lay, speech, comfort, &mut self.out16[..n]);
        }
    }
}
