//! The decoder of the coders: FEC, error estimate, frame repeat and mute, parameter reconstruction, enhancement, smoothing
//! and synthesis.

use crate::enhance::Enhancer;
use crate::model::{Params, FRAME};
use crate::synth::Synth;
use crate::{dstar, full, half, Coder, FrameKind, FrameStatus};

/// Full-rate error-rate weight (Equation 96).
const FULL_EPS_WEIGHT: f64 = 0.000365;
/// Half-rate error-rate weight (Equation 56).
const HALF_EPS_WEIGHT: f64 = 0.001064;
/// Full-rate mute limit on the error rate: the text of section 7.8 (epsilon_R > 0.0875), not flow chart 9.
pub const FULL_MUTE_RATE: f64 = 0.0875;
/// Half-rate mute limit on the error rate (clause 5.7).
const HALF_MUTE_RATE: f64 = 0.096;
/// The consecutive invalid half-rate frame that is muted instead of repeated (clause 5.7).
const HALF_MUTE_RUN: u32 = 4;
/// The consecutive repeated D-STAR frame that is muted instead.
const DSTAR_MUTE_RUN: u32 = 4;
/// The consecutive invalid full-rate frame that is muted instead of repeated when it is a lost frame: the count of flow
/// chart 9. The text of sections 7.7 and 7.8 gives no count, and the error rate does not rise for a frame that is not
/// received, so without it a run of lost frames would repeat without end. Received frames keep the text rules.
pub const FULL_LOST_MUTE_RUN: u32 = 4;
/// Whether the half-rate decoder runs adaptive smoothing, which TIA-102.BABA-1 names but does not define: it does not.
pub const HALF_RATE_SMOOTHING: bool = false;
/// Half-amplitude of the comfort noise of a muted frame (section 7.8).
const MUTE_NOISE: f64 = 5.0;

/// The full-rate repeat test of section 7.7 (the text, not flow chart 9): b0 above 207, or epsilon_0 >= 2 and
/// epsilon_T >= 10 + 40 epsilon_R.
pub fn full_rate_repeats(b0: u32, eps0: u32, eps_t: u32, eps_r: f64) -> bool {
    b0 > full::B0_MAX || (eps0 >= 2 && eps_t as f64 >= 10.0 + 40.0 * eps_r)
}

/// The half-rate repeat test of clause 5.6 on the error counts: epsilon_0 >= 4, or epsilon_0 >= 2 and epsilon_T >= 6.
fn half_rate_repeats(eps0: u32, eps_t: u32) -> bool {
    eps0 >= 4 || (eps0 >= 2 && eps_t >= 6)
}

/// A voice decoder for one coder. It keeps the state between frames; it allocates nothing after construction.
pub struct Decoder {
    coder: Coder,
    synth: Synth,
    enh: Enhancer,
    eps_r: f64,
    prev_bar: Params,
    full_amp: full::AmpState,
    pub(crate) half_amp: half::AmpState,
    invalid_run: u32,
    tone: bool,
    noise: u32,
    buf: [f64; FRAME],
}

impl Decoder {
    /// A decoder in the initial state of the coder.
    pub fn new(coder: Coder) -> Self {
        let synth = Synth::new();
        Decoder {
            coder,
            prev_bar: *synth.last(),
            synth,
            enh: Enhancer::default(),
            eps_r: 0.0,
            full_amp: full::AmpState::default(),
            half_amp: match coder {
                Coder::Dstar => dstar::BOOKS.start(),
                _ => half::BOOKS.start(),
            },
            invalid_run: 0,
            tone: false,
            noise: 1,
            buf: [0.0; FRAME],
        }
    }

    /// The coder.
    pub fn coder(&self) -> Coder {
        self.coder
    }

    /// The model parameters that the last synthesised frame used (after enhancement and smoothing).
    pub fn params(&self) -> &Params {
        self.synth.last()
    }

    /// The running error rate epsilon_R. The D-STAR coder has none: 0.
    pub fn error_rate(&self) -> f64 {
        self.eps_r
    }

    /// Decodes one frame of channel bits, one bit per byte (the low bit counts) in the order sent, [`Coder::frame_bits`] of
    /// them, into 160 samples. A frame of another length is muted.
    pub fn decode(&mut self, bits: &[u8], out: &mut [i16; FRAME]) -> FrameStatus {
        if bits.len() != self.coder.frame_bits() {
            return self.mute(0, out);
        }
        match self.coder {
            Coder::FullRate => {
                let mut sb = [0u8; full::SPEECH_BITS];
                let mut eps = [0u8; 7];
                full::decode_channel(bits, &mut sb, &mut eps);
                self.full_frame(&sb, &eps, out)
            }
            Coder::HalfRate => {
                let mut sb = [0u8; half::SPEECH_BITS];
                let mut eps = [0u8; 2];
                half::decode_channel(bits, &mut sb, &mut eps);
                self.half_frame(&sb, &eps, out)
            }
            Coder::Dstar => {
                let mut sb = [0u8; dstar::SPEECH_BITS];
                let e = dstar::decode_channel(bits, &mut sb);
                self.dstar_frame(&sb, e, out)
            }
        }
    }

    /// Decodes one frame of error-corrected speech bits, one bit per byte: the vectors u0..u7 (full rate, 88 bits),
    /// u0..u3 (half rate, 49 bits) or u0..u2 (D-STAR, 48 bits) in turn, each most significant bit first. A frame of
    /// another length is muted.
    pub fn decode_speech(&mut self, bits: &[u8], out: &mut [i16; FRAME]) -> FrameStatus {
        if bits.len() != self.coder.speech_bits() {
            return self.mute(0, out);
        }
        match self.coder {
            Coder::FullRate => {
                let mut sb = [0u8; full::SPEECH_BITS];
                for (s, &b) in sb.iter_mut().zip(bits) {
                    *s = b & 1;
                }
                self.full_frame(&sb, &[0; 7], out)
            }
            Coder::HalfRate => {
                let mut sb = [0u8; half::SPEECH_BITS];
                for (s, &b) in sb.iter_mut().zip(bits) {
                    *s = b & 1;
                }
                self.half_frame(&sb, &[0; 2], out)
            }
            Coder::Dstar => {
                let mut sb = [0u8; dstar::SPEECH_BITS];
                for (s, &b) in sb.iter_mut().zip(bits) {
                    *s = b & 1;
                }
                self.dstar_frame(&sb, dstar::Errors::default(), out)
            }
        }
    }

    /// Conceals one frame that was not received (an erasure of the channel) by the repeat and mute rules of the coder,
    /// into 160 samples. The frame counts as invalid: the previous parameters are synthesised again, and the frame is
    /// muted instead when it is the fourth consecutive invalid frame (half rate, clause 5.7; full rate,
    /// [`FULL_LOST_MUTE_RUN`]; D-STAR, the fourth repeat) or when the running error rate is above the mute limit. The
    /// error rate is not updated: no bits were received.
    pub fn lost(&mut self, out: &mut [i16; FRAME]) -> FrameStatus {
        let limit = match self.coder {
            Coder::FullRate => FULL_MUTE_RATE,
            Coder::HalfRate => HALF_MUTE_RATE,
            Coder::Dstar => f64::INFINITY,
        };
        if self.eps_r > limit {
            self.invalid_run = self.invalid_run.saturating_add(1);
            return self.mute(0, out);
        }
        self.invalid(0, out)
    }

    fn full_frame(&mut self, sb: &[u8; full::SPEECH_BITS], eps: &[u8; 7], out: &mut [i16; FRAME]) -> FrameStatus {
        let eps_t: u32 = eps.iter().map(|&e| e as u32).sum();
        self.eps_r = 0.95 * self.eps_r + FULL_EPS_WEIGHT * eps_t as f64;
        if self.eps_r > FULL_MUTE_RATE {
            return self.mute(eps_t, out);
        }
        let b0 = full::b0_from(sb);
        if full_rate_repeats(b0, eps[0] as u32, eps_t, self.eps_r) {
            self.invalid_run = self.invalid_run.saturating_add(1);
            return self.repeat(eps_t, out);
        }
        self.invalid_run = 0;
        let l = full::l_of(full::w0_of(b0));
        let mut v = full::Values::default();
        full::unpack(sb, l, &mut v);
        let mut p = Params::silent(0.0, 0);
        full::dequantise_frame(&v, &mut self.full_amp, &mut p);
        self.enh.enhance(&mut p);
        self.enh.smooth(&mut p, self.eps_r, eps_t, eps[4] as u32);
        self.synthesise(&p, out);
        FrameStatus {
            kind: FrameKind::Voice,
            errors: eps_t,
        }
    }

    fn half_frame(&mut self, sb: &[u8; half::SPEECH_BITS], eps: &[u8; 2], out: &mut [i16; FRAME]) -> FrameStatus {
        let eps_t = eps[0] as u32 + eps[1] as u32;
        self.eps_r = 0.95 * self.eps_r + HALF_EPS_WEIGHT * eps_t as f64;
        if self.eps_r > HALF_MUTE_RATE {
            return self.mute(eps_t, out);
        }
        if half_rate_repeats(eps[0] as u32, eps_t) {
            return self.invalid(eps_t, out);
        }
        let mut p = Params::silent(0.0, 0);
        let kind;
        if half::is_tone(sb) {
            let t = half::tone_from(sb);
            if half::tone_params(t, &mut p).is_none() {
                return self.invalid(eps_t, out);
            }
            self.tone = true;
            kind = FrameKind::Tone {
                id: t.id,
                amplitude: t.ad,
            };
        } else {
            self.tone = false;
            let mut b = [0u32; 9];
            half::unpack(sb, &mut b);
            match half::kind_of(b[0]) {
                half::Kind::Erasure | half::Kind::Invalid => return self.invalid(eps_t, out),
                half::Kind::Voice => {
                    half::voice_params(&half::BOOKS, b[0], b[1], &mut p);
                    let mut next = self.half_amp;
                    half::dequantise_amps(&half::BOOKS, &b, &self.half_amp, &mut next, &mut p);
                    self.half_amp = next;
                    kind = FrameKind::Voice;
                }
                half::Kind::Silence => {
                    p = Params::silent(half::SILENCE_W0, half::SILENCE_L);
                    let mut next = self.half_amp;
                    half::dequantise_amps(&half::BOOKS, &b, &self.half_amp, &mut next, &mut p);
                    kind = FrameKind::Silence;
                }
            }
            self.enh.enhance(&mut p);
            if HALF_RATE_SMOOTHING {
                self.enh.smooth(&mut p, self.eps_r, eps_t, 0);
            }
        }
        self.invalid_run = 0;
        self.synthesise(&p, out);
        FrameStatus { kind, errors: eps_t }
    }

    /// One D-STAR frame. A frame with too many errors ([`dstar::Errors::repeats`]) is repeated. A tone frame gives its
    /// tone at the fixed level [`dstar::TONE_AD`] (silence for an index that names no tone) and puts the amplitude state
    /// back to its start. Every other frame is a voice frame and updates the state.
    fn dstar_frame(&mut self, sb: &[u8; dstar::SPEECH_BITS], e: dstar::Errors, out: &mut [i16; FRAME]) -> FrameStatus {
        if e.repeats() {
            return self.invalid(e.corrected, out);
        }
        self.invalid_run = 0;
        if dstar::is_tone(sb) {
            let t = dstar::tone_from(sb);
            self.half_amp = dstar::BOOKS.start();
            // An index that names no tone leaves the parameters of silence.
            let mut p = Params::silent(half::SILENCE_W0, half::SILENCE_L);
            let _ = dstar::tone_params(t, &mut p);
            self.tone = true;
            self.synthesise(&p, out);
            return FrameStatus {
                kind: FrameKind::Tone {
                    id: t.id,
                    amplitude: t.level,
                },
                errors: e.corrected,
            };
        }
        self.tone = false;
        let mut b = [0u32; 9];
        dstar::unpack(sb, &mut b);
        let mut p = Params::silent(0.0, 0);
        half::voice_params(&dstar::BOOKS, b[0], b[1], &mut p);
        let mut next = self.half_amp;
        half::dequantise_amps(&dstar::BOOKS, &b, &self.half_amp, &mut next, &mut p);
        self.half_amp = next;
        self.enh.enhance(&mut p);
        self.synthesise(&p, out);
        FrameStatus {
            kind: FrameKind::Voice,
            errors: e.corrected,
        }
    }

    /// D-STAR silence: a frame of no harmonic sound through the synthesis, so that the frame before it ends without a
    /// step and every later frame of silence is zeros. The amplitude state goes back to its start.
    fn dstar_silence(&mut self, out: &mut [i16; FRAME]) {
        self.half_amp = dstar::BOOKS.start();
        self.tone = false;
        self.synthesise(&Params::silent(half::SILENCE_W0, half::SILENCE_L), out);
    }

    /// A frame that is not valid: repeated, or muted when it is the fourth in a row (for full rate, only a lost frame
    /// comes here).
    fn invalid(&mut self, eps_t: u32, out: &mut [i16; FRAME]) -> FrameStatus {
        self.invalid_run = self.invalid_run.saturating_add(1);
        let run = match self.coder {
            Coder::FullRate => FULL_LOST_MUTE_RUN,
            Coder::HalfRate => HALF_MUTE_RUN,
            Coder::Dstar => DSTAR_MUTE_RUN,
        };
        if self.invalid_run >= run {
            self.mute(eps_t, out)
        } else {
            self.repeat(eps_t, out)
        }
    }

    /// Synthesises the previous frame's parameters again (Equations 99 to 104).
    fn repeat(&mut self, eps_t: u32, out: &mut [i16; FRAME]) -> FrameStatus {
        let p = self.prev_bar;
        self.synthesise(&p, out);
        FrameStatus {
            kind: FrameKind::Repeated,
            errors: eps_t,
        }
    }

    /// Comfort noise, uniform in [-5, 5], without synthesis (section 7.8). The D-STAR coder gives silence instead.
    fn mute(&mut self, eps_t: u32, out: &mut [i16; FRAME]) -> FrameStatus {
        if self.coder == Coder::Dstar {
            self.dstar_silence(out);
            return FrameStatus {
                kind: FrameKind::Muted,
                errors: eps_t,
            };
        }
        for o in out.iter_mut() {
            self.noise = self.noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let u = (self.noise >> 8) as f64 / (1u32 << 24) as f64;
            *o = ((2.0 * u - 1.0) * MUTE_NOISE).round() as i16;
        }
        FrameStatus {
            kind: FrameKind::Muted,
            errors: eps_t,
        }
    }

    fn synthesise(&mut self, p: &Params, out: &mut [i16; FRAME]) {
        // Tone frames are synthesised without the random phase of Equation 141.
        self.synth.run(p, !self.tone, &mut self.buf);
        self.prev_bar = *self.synth.last();
        for (o, &x) in out.iter_mut().zip(self.buf.iter()) {
            *o = if x.is_finite() { x.round().clamp(-32768.0, 32767.0) as i16 } else { 0 };
        }
    }
}
