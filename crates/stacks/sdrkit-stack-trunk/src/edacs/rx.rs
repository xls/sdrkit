//! The EDACS control channel: `edacs.post` (6 kHz low-pass and the soft value), `edacs.frame` (dotting search,
//! alignment and polarity choice, the vote and the check) and `edacs.msg` (the message layer).

use super::code::{self, SYNC};
use super::msg::{read, EdacsMessage};
use crate::front::Eye;
use crate::{typed_format, typed_port};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Port, Timed,
};

/// The control channel's working rate: 8 samples per bit at 9600 bit/s.
pub const RATE: f64 = 76800.0;
/// The channel half-width of the front end, Hz.
pub const HALF_WIDTH_HZ: f64 = 6500.0;
/// The carrier follow-up searches this far from the channel centre, Hz.
pub const SEARCH_HZ: f64 = 9000.0;
/// The carrier follow-up corrects at most this much, Hz.
pub const MAX_CORRECTION_HZ: f64 = 20_000.0;
const SPB: u64 = 8;

/// `edacs.post`: a second-order 6 kHz low-pass (Q 0.7071), then the mean over one bit (8 samples).
pub struct EdacsPost {
    lp: Biquad,
    /// The last bit of low-passed samples.
    h: [f32; SPB as usize],
    k: usize,
}

impl Default for EdacsPost {
    fn default() -> Self {
        EdacsPost {
            lp: Biquad::new(BiquadCoeffs::lowpass(
                RATE,
                6000.0,
                std::f64::consts::FRAC_1_SQRT_2,
            )),
            h: [0.0; SPB as usize],
            k: 0,
        }
    }
}

impl Component for EdacsPost {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "edacs.post",
            vec![Port::real("in")],
            vec![Port::real("out").same_format_as(0)],
        )
        .in_place(0, 0)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = EdacsPost::default();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let (lp, h, k) = (&mut self.lp, &mut self.h, &mut self.k);
        for v in io.pass_through::<f32>(0, 0) {
            h[*k] = lp.process_sample(*v);
            *k = (*k + 1) % h.len();
            *v = h.iter().sum::<f32>() / h.len() as f32;
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// `edacs.frame`: one voted frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdacsFrame {
    /// Signal time of the sync's end, s.
    pub time: f64,
    /// Message A (28 bits).
    pub a: u32,
    /// Message B (0 when it failed its check).
    pub b: u32,
    /// Copy disagreements of A and B.
    pub err_a: u32,
    /// Of B.
    pub err_b: u32,
    /// Good: A passed its check with at most 4 disagreements.
    pub good: bool,
    /// Found in inverted polarity.
    pub inverted: bool,
    /// The data swing, Hz.
    pub swing: f32,
}

impl Item for EdacsFrame {
    const KIND: &'static str = "edacs.frame";
}

/// The frame search and choice.
pub struct EdacsFramer {
    n: u64,
    eye: Eye,
    ring: SampleRing<f32>,
    regs: [u64; SPB as usize],
    hits: Vec<u64>,
    /// (centre, predicted?) to evaluate.
    pending: Option<(u64, bool)>,
    suppress_until: u64,
    /// Syncs evaluated.
    pub syncs: u64,
}

impl Default for EdacsFramer {
    fn default() -> Self {
        EdacsFramer {
            n: 0,
            eye: Eye::default(),
            ring: SampleRing::new(8192).expect("a ring of 8192 samples"),
            regs: [0; SPB as usize],
            hits: Vec::with_capacity(8),
            pending: None,
            suppress_until: 0,
            syncs: 0,
        }
    }
}

fn alternates(x: u16) -> u32 {
    (x ^ 0x5555).count_ones()
}

impl EdacsFramer {
    fn bits_at(&self, c: u64, inv: bool, out: &mut [bool; 288]) {
        for (i, o) in out.iter_mut().enumerate() {
            let at = if i < 48 {
                c - SPB * (47 - i as u64)
            } else {
                c + SPB * (i as u64 - 47)
            };
            *o = (self.ring.get(at).unwrap_or(0.0) > 0.0) != inv;
        }
    }
    fn score(bits: &[bool; 288]) -> (u32, u64, u32, u64, u32) {
        let w = |s: &[bool]| s.iter().fold(0u64, |a, &b| (a << 1) | b as u64);
        let sync_err = (w(&bits[..48]) ^ SYNC).count_ones();
        let (a, ea) = code::vote(w(&bits[48..88]), w(&bits[88..128]), w(&bits[128..168]));
        let (b, eb) = code::vote(w(&bits[168..208]), w(&bits[208..248]), w(&bits[248..288]));
        let mut s = sync_err + ea + eb;
        if !code::valid(a) {
            s += 12;
        }
        if !code::valid(b) {
            s += 12;
        }
        (s, a, ea, b, eb)
    }
    fn evaluate(&mut self, centre: u64, predicted: bool) -> Option<EdacsFrame> {
        self.syncs += 1;
        let mut cands: Vec<u64> = Vec::with_capacity(16);
        if predicted {
            for off in [0i64, -1, 1] {
                for sh in [0i64, -1, 1, -2, 2] {
                    cands.push((centre as i64 + off * SPB as i64 + sh) as u64);
                }
            }
        } else {
            for off in [0i64, -1, 1, 2, -2, 3] {
                cands.push((centre as i64 + off * SPB as i64) as u64);
            }
        }
        let mut best: Option<(u32, u64, bool, u64, u32, u64, u32)> = None;
        let mut bits = [false; 288];
        for &c in &cands {
            for inv in [false, true] {
                self.bits_at(c, inv, &mut bits);
                let (s, a, ea, b, eb) = Self::score(&bits);
                if best.is_none_or(|x| s < x.0) {
                    best = Some((s, c, inv, a, ea, b, eb));
                }
            }
        }
        let (_, c, inv, a, ea, b, eb) = best?;
        let swing = self.eye.range();
        if swing < 400.0 {
            return None;
        }
        let good = code::valid(a) && ea <= 4;
        let f = EdacsFrame {
            time: c as f64 / RATE,
            a: (a >> 12) as u32,
            b: if code::valid(b) { (b >> 12) as u32 } else { 0 },
            err_a: ea,
            err_b: eb,
            good,
            inverted: inv,
            swing,
        };
        if good {
            self.pending = Some((c + 288 * SPB, true));
        }
        Some(f)
    }
    /// Takes post-filtered samples at 76.8 kHz, appends frames.
    pub fn run(&mut self, x: &[f32], out: &mut Vec<Timed<EdacsFrame>>) {
        const MASK: u64 = (1 << 48) - 1;
        for &v in x {
            let n = self.n;
            self.eye.update(v);
            let d = v - self.eye.mid();
            self.ring.push(d);
            let ph = (n % SPB) as usize;
            self.regs[ph] = ((self.regs[ph] << 1) | (d > 0.0) as u64) & MASK;
            if let Some((c, predicted)) = self.pending {
                let need = if predicted {
                    c + 241 * SPB + 2
                } else {
                    c + 245 * SPB
                };
                if n >= need {
                    self.pending = None;
                    if let Some(f) = self.evaluate(c, predicted) {
                        let good = f.good;
                        out.push(Timed {
                            time: f.time,
                            value: f,
                        });
                        if !good {
                            self.suppress_until = n;
                        }
                    }
                }
            }
            if let Some(&h) = self.hits.first() {
                if n > h + 4 {
                    let c = self.hits[self.hits.len() / 2];
                    self.hits.clear();
                    if self.pending.is_none() {
                        self.pending = Some((c, false));
                    }
                    self.suppress_until = c + 250 * SPB;
                }
            }
            if self.pending.is_none()
                && n >= 1000
                && n >= self.suppress_until
                && self.eye.range() >= 400.0
            {
                let r = self.regs[ph];
                let hi = (r >> 32) as u16;
                let lo = r as u16;
                let normal = alternates(hi) + alternates(lo);
                let inverted = alternates(!hi) + alternates(!lo);
                if normal <= 1 || inverted <= 1 {
                    self.hits.push(n);
                }
            }
            self.n += 1;
        }
    }
}

impl Component for EdacsFramer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "edacs.frame",
            vec![Port::real("in")],
            vec![typed_port::<EdacsFrame>("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = EdacsFramer::default();
        Ok(vec![typed_format::<EdacsFrame>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let mut out = std::mem::take(io.items_out::<EdacsFrame>(0));
        self.run(x, &mut out);
        *io.items_out::<EdacsFrame>(0) = out;
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// `edacs.msg`: good frames read through the message layer (layout and ESK settings).
#[derive(Default)]
pub struct EdacsMsg {
    extended: bool,
    esk: bool,
}

/// Extended Addressing instead of Standard / Networked.
pub const EXTENDED: Setting<field::Bool> =
    Setting::new("extended", "Extended Addressing", "", false)
        .help("Extended Addressing instead of Standard / Networked.");
/// Remove the ESK byte.
pub const ESK: Setting<field::Bool> = Setting::new("esk", "ESK", "", false)
    .help("Remove the ESK byte A0 some systems XOR into both messages.");

sdrkit_core::settings! {
    /// The layout settings of `edacs.msg`.
    pub enum MsgSetting {
        /// Extended Addressing.
        Extended = EXTENDED,
        /// Remove the ESK byte.
        Esk = ESK,
    }
}

impl Component for EdacsMsg {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "edacs.msg",
            vec![Port::typed::<EdacsFrame>("in")],
            vec![typed_port::<EdacsMessage>("out")],
        )
        .with_settings(MsgSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_format::<EdacsMessage>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let frames = io.items::<EdacsFrame>(0);
        let out = io.items_out::<EdacsMessage>(0);
        for f in frames {
            if f.value.good {
                out.push(Timed {
                    time: f.time,
                    value: read(f.value.a, f.value.b, self.extended, self.esk, f.time),
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MsgSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MsgSetting::Extended => self.extended = EXTENDED.read(id, v)?,
            MsgSetting::Esk => self.esk = ESK.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (MsgSetting::Extended, Value::Bool(self.extended)),
            (MsgSetting::Esk, Value::Bool(self.esk)),
        ])
    }
}
