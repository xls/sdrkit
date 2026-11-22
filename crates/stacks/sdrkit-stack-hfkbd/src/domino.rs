//! DominoEX and THOR: 18-tone incremental frequency keying (IFK+). DominoEX sends its nibble varicode directly. THOR sends the
//! MFSK varicode through a K = 7 or K = 15 code and a 4 x 4 x depth interleaver. The receiver follows the symbol clock, decodes
//! the tone steps and locks. The generator sends the clean signal.
//!
//! Standard: M. Greenman ZL1BPU, "DominoEX" (published description and varicode), and the THOR description.

use crate::decoder::HeardCommand;
use crate::decoder::{self, put_text, Heard, HfGenerator, Modulator, GEN_MESSAGE};
use crate::fec::{SoftViterbi, MFSK_K7, THOR_K15};
use crate::finder::CentreFinder;
use crate::mfsk::{
    interleaver, put_keyboard, varicode_bits, Keyboard, CENTRE, FIND_SIGNAL, TEST_TEXT,
};
use crate::util::Tone;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_fec::conv::{ConvCode, Encoder};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::publish::Published;
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::Record;
use sdrkit_core::{
    Choice as _, ChoiceDecl, Complex32, ConfigError, GenConfig, Generator, LockState, Schema,
    SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port};
use sdrkit_util_bits::interleave::DelayLanes;
use sdrkit_util_text::store::TextStore;
use sdrkit_util_text::varicode::{dominoex, mfsk as mfsk_vari};
use std::sync::Arc;

/// The DominoEX varicode symbols (nibbles) of a byte, in the order sent.
pub fn nibbles(b: u8) -> Vec<u8> {
    dominoex::code(b).symbols().to_vec()
}

/// A DominoEX or THOR variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IfkVariant {
    /// "11", "16", "25x4" ...
    pub tag: &'static str,
    /// Its own sample rate.
    pub rate: f64,
    /// Samples per symbol.
    pub symlen: usize,
    /// Tone spacing in units of the baud.
    pub factor: usize,
    /// THOR: constraint length (7 or 15).
    pub k: u32,
    /// THOR: interleaver depth.
    pub depth: usize,
    /// DominoEX has it.
    pub domino: bool,
}

impl IfkVariant {
    /// Symbols per second.
    pub fn baud(&self) -> f64 {
        self.rate / self.symlen as f64
    }
    /// Tone spacing, Hz.
    pub fn spacing(&self) -> f64 {
        self.baud() * self.factor as f64
    }
    /// 18 x spacing.
    pub fn bandwidth(&self) -> f64 {
        18.0 * self.spacing()
    }
}

/// Every variant (the first six are DominoEX's too).
pub const VARIANTS: [IfkVariant; 10] = [
    IfkVariant {
        tag: "4",
        rate: 8000.0,
        symlen: 2048,
        factor: 2,
        k: 7,
        depth: 10,
        domino: true,
    },
    IfkVariant {
        tag: "5",
        rate: 11025.0,
        symlen: 2048,
        factor: 2,
        k: 7,
        depth: 10,
        domino: true,
    },
    IfkVariant {
        tag: "8",
        rate: 8000.0,
        symlen: 1024,
        factor: 2,
        k: 7,
        depth: 10,
        domino: true,
    },
    IfkVariant {
        tag: "11",
        rate: 11025.0,
        symlen: 1024,
        factor: 1,
        k: 7,
        depth: 10,
        domino: true,
    },
    IfkVariant {
        tag: "16",
        rate: 8000.0,
        symlen: 512,
        factor: 1,
        k: 7,
        depth: 10,
        domino: true,
    },
    IfkVariant {
        tag: "22",
        rate: 11025.0,
        symlen: 512,
        factor: 1,
        k: 7,
        depth: 10,
        domino: true,
    },
    IfkVariant {
        tag: "25x4",
        rate: 8000.0,
        symlen: 320,
        factor: 4,
        k: 15,
        depth: 50,
        domino: false,
    },
    IfkVariant {
        tag: "50x1",
        rate: 8000.0,
        symlen: 160,
        factor: 1,
        k: 15,
        depth: 50,
        domino: false,
    },
    IfkVariant {
        tag: "50x2",
        rate: 8000.0,
        symlen: 160,
        factor: 2,
        k: 15,
        depth: 50,
        domino: false,
    },
    IfkVariant {
        tag: "100",
        rate: 8000.0,
        symlen: 80,
        factor: 1,
        k: 15,
        depth: 50,
        domino: false,
    },
];

/// Which family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// DominoEX: nibble varicode, no FEC.
    DominoEx,
    /// THOR: MFSK varicode, convolutional code, interleaver.
    Thor,
}

impl Family {
    fn prefix(self) -> &'static str {
        match self {
            Family::DominoEx => "DominoEX ",
            Family::Thor => "THOR ",
        }
    }
    /// The variants of this family.
    pub fn variants(self) -> Vec<IfkVariant> {
        VARIANTS
            .iter()
            .copied()
            .filter(|v| self == Family::Thor || v.domino)
            .collect()
    }
    /// "DominoEX 11".
    pub fn name(self, v: &IfkVariant) -> String {
        format!("{}{}", self.prefix(), v.tag)
    }
    /// A variant by its name ("THOR 16") or tag ("16").
    pub fn variant(self, name: &str) -> Option<IfkVariant> {
        let tag = name.strip_prefix(self.prefix()).unwrap_or(name).trim();
        self.variants()
            .into_iter()
            .find(|v| v.tag.eq_ignore_ascii_case(tag))
    }
    /// The default variant.
    pub fn default_variant(self) -> IfkVariant {
        match self {
            Family::DominoEx => VARIANTS[3],
            Family::Thor => VARIANTS[4],
        }
    }
}

/// The variants of `f` as choices ("DominoEX 11").
fn variant_choices(f: Family) -> Vec<ChoiceDecl> {
    f.variants()
        .iter()
        .map(|v| {
            let n = f.name(v);
            ChoiceDecl {
                key: n.clone().into(),
                label: n.into(),
            }
        })
        .collect()
}

/// The `variant` setting of DominoEX.
pub const DOMINO_VARIANT: CatalogueSetting = CatalogueSetting {
    key: "variant",
    label: "Variant",
    help: "",
    choices: || variant_choices(Family::DominoEx),
    default: "DominoEX 11",
    advanced: false,
    role: None,
};

/// The `variant` setting of THOR.
pub const THOR_VARIANT: CatalogueSetting = CatalogueSetting {
    key: "variant",
    label: "Variant",
    help: "",
    choices: || variant_choices(Family::Thor),
    default: "THOR 16",
    advanced: false,
    role: None,
};

sdrkit_core::settings! {
    /// The settings of DominoEX.
    pub enum DominoSetting {
        /// The variant.
        Variant = DOMINO_VARIANT,
        /// The audio centre.
        Centre = CENTRE,
        /// The centre follows the signal.
        FindSignal = FIND_SIGNAL,
    }
}

sdrkit_core::settings! {
    /// The settings of THOR.
    pub enum ThorSetting {
        /// The variant.
        Variant = THOR_VARIANT,
        /// The audio centre.
        Centre = CENTRE,
        /// The centre follows the signal.
        FindSignal = FIND_SIGNAL,
    }
}

sdrkit_core::settings! {
    /// The test-signal settings of DominoEX and THOR.
    pub enum IfkGenSetting {
        /// The text sent.
        Message = GEN_MESSAGE,
    }
}

impl Family {
    /// The `variant` setting of the family.
    fn variant_setting(self) -> &'static CatalogueSetting {
        match self {
            Family::DominoEx => &DOMINO_VARIANT,
            Family::Thor => &THOR_VARIANT,
        }
    }
}

/// THOR's code of a variant.
fn thor_code(v: &IfkVariant) -> ConvCode {
    if v.k == 15 {
        THOR_K15
    } else {
        MFSK_K7
    }
}

/// THOR's symbol of the next four code bits of `code` (the first taken is the most significant), through the interleaver `il`.
pub fn thor_symbol(code: &mut std::collections::VecDeque<u8>, il: &mut DelayLanes<f32>) -> u32 {
    let mut v = [0.0f32; 4];
    for (x, c) in v.iter_mut().zip(code.drain(..4)) {
        *x = f32::from(c);
    }
    il.process(&mut v);
    let mut s = 0;
    for (i, &b) in v.iter().enumerate() {
        if b > 0.5 {
            s |= 1 << (3 - i);
        }
    }
    s
}

/// The IFK+ transmitter.
pub struct IfkModulator {
    v: IfkVariant,
    fam: Family,
    rate: f64,
    centre: f64,
    text: Vec<u8>,
    ti: usize,
    syms: std::collections::VecDeque<u32>,
    enc: Encoder,
    il: DelayLanes<f32>,
    code: std::collections::VecDeque<u8>,
    data: std::collections::VecDeque<u8>,
    tone: u32,
    osc: Tone,
    left: f64,
}

impl IfkModulator {
    /// Sends `text` again and again at `centre` Hz.
    pub fn new(fam: Family, v: IfkVariant, rate: f64, centre: f64, text: &str) -> Self {
        let il = interleaver(4, v.depth, false);
        let mut syms = std::collections::VecDeque::new();
        if fam == Family::Thor {
            // The interleaver starts filled with the code of zero bits, then 16 zero symbols and an idle character.
            syms.extend(std::iter::repeat_n(0, 16));
        }
        let mut data = std::collections::VecDeque::new();
        if fam == Family::Thor {
            data.extend(varicode_bits(&[0]));
        }
        IfkModulator {
            v,
            fam,
            rate,
            centre,
            text: text.as_bytes().to_vec(),
            ti: 0,
            syms,
            enc: Encoder::new(thor_code(&v)),
            il,
            code: Default::default(),
            data,
            tone: 0,
            osc: Tone::new(rate),
            left: 0.0,
        }
    }

    fn next_symbol(&mut self) -> u32 {
        if let Some(s) = self.syms.pop_front() {
            return s;
        }
        match self.fam {
            Family::DominoEx => {
                let b = self.text[self.ti % self.text.len()];
                self.ti += 1;
                self.syms.extend(nibbles(b).iter().map(|&n| n as u32));
                self.syms.pop_front().unwrap_or(0)
            }
            Family::Thor => {
                while self.code.len() < 4 {
                    if self.data.is_empty() {
                        let b = self.text[self.ti % self.text.len()];
                        self.ti += 1;
                        self.data.extend(varicode_bits(&[b]));
                    }
                    let bit = self.data.pop_front().unwrap_or(0);
                    let p = self.enc.push_bit(bit);
                    self.code.push_back(p & 1);
                    self.code.push_back((p >> 1) & 1);
                }
                thor_symbol(&mut self.code, &mut self.il)
            }
        }
    }
}

impl Modulator for IfkModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let lo = self.centre - self.v.bandwidth() / 2.0 + self.v.spacing() / 2.0;
        for o in out {
            if self.left <= 0.0 {
                let s = self.next_symbol();
                self.tone = (self.tone + 2 + s) % 18;
                self.left += self.v.symlen as f64 / self.v.rate * self.rate;
            }
            *o = self.osc.next(lo + self.tone as f64 * self.v.spacing(), 1.0);
            self.left -= 1.0;
        }
    }
}

const HOPS: usize = 16;
const SUB: usize = 4;
/// DominoEX: the most characters kept before the lock.
const PENDING: usize = 32;
/// A window holds a tone when its strongest sub-bin is at least this many times the root of the mean power of the others
/// (`r` in `hop`). Noise alone gives about 1.8 to 2.7, a symbol at -3 dB in 2.5 kHz 3.5 or more; silence and
/// the edge of a burst give less than 2.
const TONE_RATIO: f32 = 2.5;

/// The DominoEX / THOR receiver.
pub struct IfkDecoder {
    fam: Family,
    v: IfkVariant,
    centre: f64,
    find: bool,
    dial: f64,
    rs: Option<Resampler<Complex32>>,
    rsbuf: Vec<Complex32>,
    nco: Nco,
    mixed: Vec<Complex32>,
    hist: SampleRing<Complex32>,
    spec: WindowedFft,
    hop: usize,
    hop_n: u64,
    last_dec: u64,
    peak_avg: [f32; HOPS],
    phase: usize,
    symbols: u64,
    lo_q: i32,
    hi_q: i32,
    prev_q: i32,
    /// Whether the window of the last decision held a tone.
    prev_tone: bool,
    prev_mags: Vec<f32>,
    snr: f32,
    ratio: f32,
    // DominoEX
    // THOR
    deint: DelayLanes<f32>,
    vit: SoftViterbi,
    ncode: u64,
    prev_code: f32,
    vmetric: f32,
    conf: f32,
    vari: mfsk_vari::Decoder,
    nibbles: dominoex::Decoder,
    /// DominoEX: the characters read since the last erasure while not locked, shown when the lock comes.
    pending: Vec<u8>,
    locked: bool,
    store: TextStore,
    heard: Heard,
    status: Status,
    label: String,
    finder: CentreFinder,
    t_in: u64,
}

impl IfkDecoder {
    /// A receiver of `fam` with these settings.
    pub fn new(fam: Family) -> Self {
        Self::with_variant(fam, fam.default_variant())
    }

    /// A receiver of one variant.
    pub fn with_variant(fam: Family, v: IfkVariant) -> Self {
        let n = SUB * v.symlen;
        IfkDecoder {
            fam,
            v,
            centre: 1500.0,
            find: true,
            dial: 0.0,
            rs: (v.rate != 8000.0).then(|| {
                Resampler::new(8000.0, v.rate).expect("the variant has a fixed positive rate")
            }),
            rsbuf: Vec::new(),
            nco: Nco::new(v.rate, -1500.0),
            mixed: Vec::new(),
            hist: SampleRing::new(v.symlen).expect("a ring of at least one sample"),
            spec: WindowedFft::new(n, v.symlen, Window::Rectangular)
                .expect("a window that fits the transform"),
            hop: 0,
            hop_n: 0,
            last_dec: 0,
            peak_avg: [0.0; HOPS],
            phase: 0,
            symbols: 0,
            lo_q: 0,
            hi_q: 0,
            prev_q: 0,
            prev_tone: false,
            prev_mags: Vec::new(),
            snr: 0.0,
            ratio: 1.0,
            deint: interleaver(4, v.depth, true),
            vit: SoftViterbi::new(thor_code(&v), if v.k == 15 { 180 } else { 45 }),
            ncode: 0,
            prev_code: 0.0,
            vmetric: 0.0,
            conf: 0.0,
            vari: mfsk_vari::Decoder::new(),
            nibbles: dominoex::Decoder::new(),
            pending: Vec::with_capacity(PENDING),
            locked: false,
            store: decoder::store(8000, 0),
            heard: Heard::new(match fam {
                Family::DominoEx => "DominoEX",
                Family::Thor => "THOR",
            }),
            status: Status::default(),
            label: fam.name(&v),
            finder: CentreFinder::new(v.bandwidth() as f32),
            t_in: 0,
        }
    }

    fn reset_signal(&mut self) {
        let store = std::mem::replace(&mut self.store, decoder::store(1, 1));
        let heard = std::mem::replace(&mut self.heard, Heard::new(""));
        let (centre, find, dial, t_in) = (self.centre, self.find, self.dial, self.t_in);
        *self = Self::with_variant(self.fam, self.v);
        self.store = store;
        self.heard = heard;
        self.centre = centre;
        self.find = find;
        self.dial = dial;
        self.t_in = t_in;
        self.nco = Nco::new(self.v.rate, -centre);
        // The window of sub-bins: the 18 tones and 9 each side, inside 80 to 3900 Hz.
        let sub_hz = self.v.baud() / SUB as f64;
        let half = 18.0 * self.v.spacing();
        let lo = (-half).max(80.0 - centre);
        let hi = half.min(3900.0 - centre);
        self.lo_q = (lo / sub_hz).ceil() as i32;
        self.hi_q = (hi / sub_hz).floor() as i32;
    }

    fn q_per_tone(&self) -> i32 {
        (SUB * self.v.factor) as i32
    }

    fn hop(&mut self, out: &mut Outbox) {
        self.spec.run_power(self.hist.last(self.v.symlen));
        let n = (SUB * self.v.symlen) as i32;
        let (mut best, mut bq, mut sum, mut cnt) = (0.0f32, 0i32, 0.0f32, 0);
        for q in self.lo_q..=self.hi_q {
            let p = self.spec.power()[q.rem_euclid(n) as usize];
            sum += p;
            cnt += 1;
            if p > best {
                best = p;
                bq = q;
            }
        }
        let sharp = if sum > 0.0 { best / sum } else { 0.0 };
        let h = (self.hop_n % HOPS as u64) as usize;
        self.peak_avg[h] += 0.1 * (sharp - self.peak_avg[h]);
        if h == self.phase && self.hop_n >= self.last_dec + HOPS as u64 / 2 {
            self.last_dec = self.hop_n;
            let others = (sum - best) / (cnt - 1).max(1) as f32;
            let r = (best / others.max(1e-20)).sqrt();
            self.ratio += (r - self.ratio) / 12.0;
            self.snr = 20.0 * self.ratio.max(1e-6).log10();
            self.decide(bq, r >= TONE_RATIO, out);
            let bp = (0..HOPS)
                .max_by(|&x, &y| self.peak_avg[x].total_cmp(&self.peak_avg[y]))
                .expect("the range is not empty");
            if bp != self.phase && self.peak_avg[bp] > 1.03 * self.peak_avg[self.phase] {
                let d = (bp + HOPS - self.phase) % HOPS;
                self.phase = if d <= HOPS / 2 {
                    (self.phase + 1) % HOPS
                } else {
                    (self.phase + HOPS - 1) % HOPS
                };
            }
        }
        self.hop_n += 1;
    }

    /// One symbol decision at sub-bin `q`; `tone`: the window held a tone. A DominoEX symbol is the step from the tone before:
    /// a window without a tone (silence, the edge of a burst) is an erasure, and so is the step from it.
    fn decide(&mut self, q: i32, tone: bool, out: &mut Outbox) {
        self.symbols += 1;
        let qt = self.q_per_tone();
        let step = (q - self.prev_q) as f32 / qt as f32;
        let prev_q = self.prev_q;
        self.prev_q = q;
        let st = step.round() as i32;
        let erased = st.abs() as f32 > 17.5 || (step - st as f32).abs() > 0.45;
        let m = st.rem_euclid(18);
        let erased = erased || m < 2 || !tone || !self.prev_tone;
        self.prev_tone = tone;
        match self.fam {
            Family::DominoEx => {
                let locked = self.ratio >= 6.0;
                if locked != self.locked {
                    self.locked = locked;
                    self.pending.reverse();
                    while let Some(b) = self.pending.pop() {
                        if locked {
                            self.out(b, out);
                        }
                    }
                    if locked {
                        self.heard
                            .logs
                            .push(format!("{} locked at {:.0} Hz", self.label, self.centre));
                    }
                }
                if erased {
                    self.pending.clear();
                    return;
                }
                let sym = (m - 2) as u32;
                self.nibble(sym, out);
            }
            Family::Thor => {
                // Soft bits from the power at each symbol's expected position.
                let n = (SUB * self.v.symlen) as i32;
                let mut soft = [0.0f32; 4];
                let mut tot = 0.0;
                {
                    for s in 0..16i32 {
                        let mut best = 0.0f32;
                        for d in [-18, 0, 18] {
                            let qq = prev_q + (s + 2 + d) * qt;
                            if qq < self.lo_q || qq > self.hi_q {
                                continue;
                            }
                            for j in -1..=1 {
                                best = best.max(self.spec.power()[(qq + j).rem_euclid(n) as usize]);
                            }
                        }
                        let m = best.sqrt();
                        tot += m;
                        for (i, x) in soft.iter_mut().enumerate() {
                            *x += if (s >> (3 - i)) & 1 == 1 { m } else { -m };
                        }
                    }
                }
                if tot > 0.0 {
                    for x in &mut soft {
                        *x /= tot;
                    }
                }
                self.deint.process(&mut soft);
                for &cur in &soft {
                    let k = self.ncode;
                    self.ncode += 1;
                    let prev = self.prev_code;
                    self.prev_code = cur;
                    if k.is_multiple_of(2) {
                        continue;
                    }
                    let r = self.vit.push(prev, cur);
                    let g = self.vit.gain / (prev.abs() + cur.abs()).max(1e-6);
                    self.vmetric += 0.05 * (g - self.vmetric);
                    if let Some(bit) = r {
                        self.thor_bit(bit, out);
                    }
                }
                // The confidence: +2 when good, -(2 + half of it) when bad. Locked at 40.
                if self.vmetric > 0.6 {
                    self.conf = (self.conf + 2.0).min(100.0);
                } else {
                    self.conf = (self.conf - (2.0 + self.conf / 2.0)).max(0.0);
                }
                let locked = self.conf >= 40.0;
                if locked && !self.locked {
                    self.heard
                        .logs
                        .push(format!("{} locked at {:.0} Hz", self.label, self.centre));
                }
                self.locked = locked;
            }
        }
    }

    /// One DominoEX symbol. A character read before the lock waits in `pending`: the lock needs about a dozen symbols, and the
    /// start of a burst is only four characters long.
    fn nibble(&mut self, sym: u32, out: &mut Outbox) {
        if let Some(Ok(dominoex::Symbol::Primary(b))) = self.nibbles.push(sym as u8) {
            if self.locked {
                self.out(b, out);
            } else if self.pending.len() < PENDING {
                self.pending.push(b);
            }
        }
    }

    fn thor_bit(&mut self, bit: u8, out: &mut Outbox) {
        // The secondary character channel is not shown.
        if let (Some(Ok(mfsk_vari::Symbol::Primary(b))), true) = (self.vari.push(bit), self.locked)
        {
            self.out(b, out);
        }
    }

    fn out(&mut self, b: u8, out: &mut Outbox) {
        if b == 0 {
            return;
        }
        let time = self.t_in as f64 / 8000.0;
        self.heard.char(
            &mut self.store,
            out,
            b,
            time,
            100,
            0,
            self.dial + self.centre,
            self.snr,
        );
    }
}

impl sdrkit_graph::Decoder for IfkDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        match self.fam {
            Family::DominoEx => "hfkbd.dominoex",
            Family::Thor => "hfkbd.thor",
        }
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        match self.fam {
            Family::DominoEx => DominoSetting::schema(),
            Family::Thor => ThorSetting::schema(),
        }
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        // The two families declare the same settings in the same order.
        match DominoSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DominoSetting::Variant => {
                let i = self.fam.variant_setting().read(id, value)?;
                let v = self
                    .fam
                    .variants()
                    .get(i)
                    .copied()
                    .ok_or(SettingError::NotAChoice {
                        id,
                        index: i as u16,
                    })?;
                if v != self.v {
                    self.v = v;
                    self.reset_signal();
                    self.finder = CentreFinder::new(v.bandwidth() as f32);
                }
            }
            DominoSetting::Centre => {
                self.centre = CENTRE.read(id, value)?;
                self.reset_signal();
            }
            DominoSetting::FindSignal => self.find = FIND_SIGNAL.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let i = self
            .fam
            .variants()
            .iter()
            .position(|v| *v == self.v)
            .unwrap_or(0);
        let all = [
            Value::Choice(i as u16),
            Value::Hz(self.centre),
            Value::Bool(self.find),
        ];
        match self.fam {
            Family::DominoEx => values_of(
                [
                    DominoSetting::Variant,
                    DominoSetting::Centre,
                    DominoSetting::FindSignal,
                ]
                .into_iter()
                .zip(all),
            ),
            Family::Thor => values_of(
                [
                    ThorSetting::Variant,
                    ThorSetting::Centre,
                    ThorSetting::FindSignal,
                ]
                .into_iter()
                .zip(all),
            ),
        }
    }
    fn results(&self) -> Published {
        crate::typed::results(match self.fam {
            Family::DominoEx => "dominoex",
            Family::Thor => "thor",
        })
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.reset_signal();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset_signal();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        // In pieces that end where the signal finder decides, so the result does not depend on how the input is cut.
        let mut rest = x;
        while !rest.is_empty() {
            let n = rest.len().min(self.finder.until_frame());
            let (piece, r) = rest.split_at(n);
            rest = r;
            self.piece(piece, out);
        }
    }
    fn report(&mut self, res: &mut Published) {
        let st = &mut self.status;
        st.lock_state = if self.locked {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = Some(self.snr);
        st.offset_hz = Some(0.0);
        let what = if self.locked { "locked" } else { "searching" };
        put_text(&mut st.summary, format_args!("{} {what}", self.label));
        res.results_mut().set_status(st);
        put_keyboard(
            res,
            &Keyboard {
                variant: &self.label,
                centre_hz: self.centre,
                offset_hz: 0.0,
                snr_db: self.snr,
                store: &self.store,
                quality: self.conf / 100.0,
            },
            &mut self.heard,
        );
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset_signal();
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
    }
}

impl IfkDecoder {
    fn piece(&mut self, x: &[Complex32], out: &mut Outbox) {
        if self.lo_q == self.hi_q {
            self.reset_signal();
        }

        let input: &[Complex32] = match &mut self.rs {
            Some(r) => {
                self.rsbuf.clear();
                r.process(x, &mut self.rsbuf);
                &self.rsbuf
            }
            None => x,
        };
        self.mixed.clear();
        self.mixed.resize(input.len(), Complex32::default());
        self.nco.mix(input, &mut self.mixed);
        let hop = (self.v.symlen / HOPS).max(1);
        let mixed = std::mem::take(&mut self.mixed);
        for &s in &mixed {
            self.hist.push(s);
            self.hop += 1;
            if self.hop == hop {
                self.hop = 0;
                self.hop(out);
            }
        }
        self.mixed = mixed;
        self.t_in += x.len() as u64;
        let _ = &self.prev_mags;
        // The signal finder decides at the end of the piece, after the decoder has seen it.
        let busy = self.locked && self.store.plausible();
        if self.find {
            if let Some(c) = self.finder.feed(x, self.centre as f32, !busy) {
                self.heard.logs.push(format!("signal found at {c:.0} Hz"));
                self.centre = c as f64;
                self.reset_signal();
            }
        }
    }
}

/// The generator of a family. It reads the settings `variant`, `centre` and `gen_message`.
pub fn generator(fam: Family, cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = fam
        .variants()
        .get(usize::from(cfg.settings.entry(fam.variant_setting())))
        .copied()
        .unwrap_or(fam.default_variant());
    let centre = cfg.settings.get(&CENTRE);
    let m = IfkModulator::new(
        fam,
        v,
        cfg.rate_hz,
        centre,
        &decoder::message(cfg, TEST_TEXT),
    );
    let baud = v.rate / v.symlen as f64;
    let bits = if fam == Family::Thor { 2.0 } else { 4.0 };
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(v.bandwidth())
            .with_symbol_rate(baud)
            .with_bit_rate(baud * bits),
    ))
}

/// The DominoEX generator.
pub fn domino_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    generator(Family::DominoEx, cfg)
}

/// The THOR generator.
pub fn thor_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    generator(Family::Thor, cfg)
}

sdrkit_core::settings! {
    /// The settings of the DominoEX sender.
    pub enum DominoTxSetting {
        /// The variant.
        Variant = DOMINO_VARIANT,
        /// The audio centre.
        Centre = CENTRE,
        /// USB or LSB.
        Demod = crate::front::DEMOD,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
    }
}

sdrkit_core::settings! {
    /// The settings of the THOR sender.
    pub enum ThorTxSetting {
        /// The variant.
        Variant = THOR_VARIANT,
        /// The audio centre.
        Centre = CENTRE,
        /// USB or LSB.
        Demod = crate::front::DEMOD,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
    }
}

/// The characters before the text of a burst: NUL, CR, STX, CR.
const START: [u8; 4] = [0, b'\r', 2, b'\r'];
/// THOR: zero symbols sent before the code, without it.
const THOR_ZEROS: usize = 16;

/// The coder of DominoEX and THOR. 18 tones; tone k (0 to 17) at centre + (k - 8.5) spacing, phase continuous. IFK+: the tone
/// number is 0 before the first symbol and each symbol s (0 to 15) moves it to (previous + 2 + s) mod 18. DominoEX sends each
/// byte's nibbles of the DominoEX varicode as symbols; a burst is NUL, CR, STX, CR, the text and 16 spaces. THOR (the K = 7
/// variants) sends the MFSK varicode through the K = 7 code and the interleaver of 4 x 4 x depth, each 4 code bits one symbol;
/// a burst is 16 zero symbols outside the code, then NUL, CR, STX, CR, the text and 16 + 4 depth spaces through the code, then
/// a zero bit when a symbol is half full.
pub struct IfkCoder {
    fam: Family,
    v: IfkVariant,
    centre: f64,
    tone: u32,
    enc: Encoder,
    il: DelayLanes<f32>,
    code: std::collections::VecDeque<u8>,
}

impl IfkCoder {
    /// A coder of variant `v` of family `fam` around the audio centre `centre`.
    pub fn new(fam: Family, v: IfkVariant, centre: f64) -> Self {
        IfkCoder {
            fam,
            v,
            centre,
            tone: 0,
            enc: Encoder::new(thor_code(&v)),
            il: interleaver(4, v.depth, false),
            code: std::collections::VecDeque::new(),
        }
    }
    /// The spaces after the text.
    pub fn tail_spaces(&self) -> usize {
        match self.fam {
            Family::DominoEx => 16,
            Family::Thor => 16 + 4 * self.v.depth,
        }
    }
    fn symbol(&mut self, s: u32, out: &mut Vec<crate::sender::Element>) {
        self.tone = (self.tone + 2 + s) % 18;
        let hz = self.centre + (f64::from(self.tone) - 8.5) * self.v.spacing();
        out.push(crate::sender::Element::tone(
            hz,
            self.v.symlen as f64 / self.v.rate,
        ));
    }
    fn data_bit(&mut self, bit: u8, out: &mut Vec<crate::sender::Element>) {
        let p = self.enc.push_bit(bit);
        self.code.push_back(p & 1);
        self.code.push_back((p >> 1) & 1);
        if self.code.len() >= 4 {
            let s = thor_symbol(&mut self.code, &mut self.il);
            self.symbol(s, out);
        }
    }
    fn byte(&mut self, b: u8, out: &mut Vec<crate::sender::Element>) {
        match self.fam {
            Family::DominoEx => {
                for &n in dominoex::code(b).symbols() {
                    self.symbol(u32::from(n), out);
                }
            }
            Family::Thor => {
                for bit in mfsk_vari::code(b).iter() {
                    self.data_bit(bit, out);
                }
            }
        }
    }
}

impl crate::sender::Coder for IfkCoder {
    fn begin(&mut self, out: &mut Vec<crate::sender::Element>) {
        self.tone = 0;
        if self.fam == Family::Thor {
            for _ in 0..THOR_ZEROS {
                self.symbol(0, out);
            }
        }
        for b in START {
            self.byte(b, out);
        }
    }
    fn char(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        if let Ok(b) = u8::try_from(c) {
            self.byte(b, out);
        }
    }
    fn end(&mut self, out: &mut Vec<crate::sender::Element>) {
        for _ in 0..self.tail_spaces() {
            self.byte(b' ', out);
        }
        if !self.code.is_empty() {
            self.data_bit(0, out);
        }
    }
}

/// The DominoEX or THOR side of the sender.
#[derive(Clone, Debug)]
pub struct IfkModem {
    fam: Family,
    v: IfkVariant,
    centre: f64,
    demod: crate::front::Demod,
    common: crate::sender::Common,
}

impl IfkModem {
    /// The sender of `fam` at the default settings.
    pub fn new(fam: Family) -> Self {
        IfkModem {
            fam,
            v: fam.default_variant(),
            centre: CENTRE.default,
            demod: crate::front::DEMOD.default,
            common: crate::sender::Common::default(),
        }
    }
}

/// The DominoEX side of the sender.
#[derive(Clone, Debug)]
pub struct DominoModem(IfkModem);

impl Default for DominoModem {
    fn default() -> Self {
        DominoModem(IfkModem::new(Family::DominoEx))
    }
}

/// The THOR side of the sender.
#[derive(Clone, Debug)]
pub struct ThorModem(IfkModem);

impl Default for ThorModem {
    fn default() -> Self {
        ThorModem(IfkModem::new(Family::Thor))
    }
}

impl IfkModem {
    /// Sets the variant from the value `v` of setting `id`.
    fn set_variant(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let i = self.fam.variant_setting().read(id, v)?;
        self.v = self
            .fam
            .variants()
            .get(i)
            .copied()
            .ok_or(SettingError::NotAChoice {
                id,
                index: i as u16,
            })?;
        Ok(())
    }
    fn index(&self) -> u16 {
        self.fam
            .variants()
            .iter()
            .position(|x| *x == self.v)
            .unwrap_or(0) as u16
    }
    /// Bytes 0 to 255 (ISO 8859-1) as typed; CR LF in the text is one LF.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        let start = out.len();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\r' && chars.peek() == Some(&'\n') {
                continue;
            }
            if u8::try_from(c).is_err() {
                out.truncate(start);
                return Err(crate::sender::TxError::Character(c));
            }
            out.push(c);
        }
        Ok(())
    }
    fn coder(&self) -> Result<Box<dyn crate::sender::Coder>, crate::sender::TxError> {
        crate::sender::lower_of(self.demod)?;
        if self.fam == Family::Thor && self.v.k != 7 {
            return Err(crate::sender::TxError::Variant);
        }
        Ok(Box::new(IfkCoder::new(self.fam, self.v, self.centre)))
    }
}

/// The modem trait for a wrapper of [`IfkModem`] with the settings enum `$set`.
macro_rules! ifk_modem {
    ($t:ty, $set:ident, $name:literal) => {
        impl crate::sender::Modem for $t {
            const NAME: &'static str = $name;
            fn schema() -> Arc<Schema> {
                $set::schema()
            }
            fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
                let m = &mut self.0;
                match $set::from_id(id).ok_or(SettingError::NoSuchId { id })? {
                    $set::Variant => m.set_variant(id, v)?,
                    $set::Centre => m.centre = CENTRE.read(id, v)?,
                    $set::Demod => m.demod = crate::front::DEMOD.read(id, v)?,
                    $set::Level => m.common.level = crate::sender::TX_LEVEL.read(id, v)?,
                    $set::TuneLimit => {
                        m.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
                    }
                }
                Ok(())
            }
            fn values(&self) -> Values {
                let m = &self.0;
                values_of([
                    ($set::Variant, Value::Choice(m.index())),
                    ($set::Centre, Value::Hz(m.centre)),
                    ($set::Demod, Value::Choice(m.demod.index())),
                    ($set::Level, Value::F64(m.common.level)),
                    ($set::TuneLimit, Value::Duration(m.common.tune_s)),
                ])
            }
            fn common(&self) -> crate::sender::Common {
                self.0.common
            }
            fn accept(
                &self,
                text: &str,
                out: &mut Vec<char>,
            ) -> Result<(), crate::sender::TxError> {
                self.0.accept(text, out)
            }
            fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
                self.0.coder().map(crate::sender::Coding::Elements)
            }
            fn centre_hz(&self) -> f64 {
                self.0.centre
            }
            /// The 18 tones and half a spacing each side.
            fn band(&self) -> (f64, f64) {
                let half = 9.0 * self.0.v.spacing();
                (self.0.centre - half, self.0.centre + half)
            }
            fn lower(&self) -> bool {
                crate::sender::lower_of(self.0.demod).unwrap_or(false)
            }
        }
    };
}

ifk_modem!(DominoModem, DominoTxSetting, "hfkbd.dominoextx");
ifk_modem!(ThorModem, ThorTxSetting, "hfkbd.thortx");

/// The DominoEX sender component.
pub fn domino_sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<DominoModem>::default())
}

/// The THOR sender component.
pub fn thor_sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<ThorModem>::default())
}
