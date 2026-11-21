//! The components of the MPT 1327 receiver: `mpt.channel`, `mpt.disc`, `mpt.ffsk`, `mpt.frame`, `mpt.msg` and `mpt.tables`.
//!
//! Standard: MPT 1327, MPT 1317 (the FFSK signalling).

use super::code::{self, Check, SYNC};
use super::msg::{decode, MptMessage, MptType};
use crate::front::Level;
use crate::grant::Grant;
use crate::typed::*;
use crate::{set_status, typed_format, typed_port};
use num_complex::Complex32;
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_modem::follow::{CentredChannel, SpectralConfig, SpectralFollow};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Choice, ConfigError, Event, LockState, Results, Setting, SettingError, SettingId, SettingSet,
    Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port,
    RateRule, Timed,
};
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;

/// The demodulator's rate: 20 samples per bit at 1200 bit/s.
pub const FFSK_RATE: f64 = 24000.0;
const SPB: u64 = 20;

/// The half-width of the channel filter centred on the carrier, Hz.
pub const CHANNEL_HALF_WIDTH_HZ: f64 = 4500.0;

/// `mpt.channel`: the control channel mixed to 0 Hz and decimated by round(fs / 24 kHz), then a fourth-order Butterworth of
/// [`CHANNEL_HALF_WIDTH_HZ`] centred on the carrier that the spectral follow-up (open loop) finds; until the follow-up settles
/// the channel passes unfiltered. The discriminator values keep their frequencies. It reports the level every 0.2 s.
#[derive(Default)]
pub struct MptChannel {
    nco: Option<Nco>,
    decim: Option<Decimator<Complex32>>,
    narrow: Option<Narrow>,
    out_rate: f64,
    mixed: Vec<Complex32>,
    acc: f64,
    n: usize,
}

/// The follow-up and the filter it centres.
struct Narrow {
    follow: SpectralFollow,
    channel: CentredChannel,
    /// Samples into the follow-up's current FFT frame.
    fill: usize,
}

impl Narrow {
    /// Filters `x` in place, moving the filter after each look at the sample the look ends on.
    fn process(&mut self, x: &mut [Complex32]) {
        let n = self.follow.fft_len();
        let mut i = 0;
        while i < x.len() {
            let k = (n - self.fill).min(x.len() - i);
            let piece = &mut x[i..i + k];
            let look = self.follow.process(piece);
            self.channel.process(piece);
            if let Some(l) = look {
                self.channel.set_estimate(l.correction_hz, l.settled);
            }
            self.fill = (self.fill + k) % n;
            i += k;
        }
    }
}

impl Component for MptChannel {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpt.channel",
            vec![Port::iq("in")],
            vec![Port::iq("out"), typed_port::<Level>("level")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let d = if f.rate <= FFSK_RATE {
            1
        } else {
            (f.rate / FFSK_RATE).round().max(1.0) as usize
        };
        self.out_rate = f.rate / d as f64;
        self.nco = Some(Nco::new(f.rate, -f.signal_offset_hz()));
        self.decim =
            (d > 1).then(|| Decimator::new(f.rate, d, 6000.0f64.min(0.3 * self.out_rate), 60.0));
        self.narrow = if CHANNEL_HALF_WIDTH_HZ < 0.45 * self.out_rate {
            Some(Narrow {
                follow: SpectralFollow::new(SpectralConfig::open_loop(self.out_rate))?,
                channel: CentredChannel::new(self.out_rate, CHANNEL_HALF_WIDTH_HZ)?,
                fill: 0,
            })
        } else {
            None
        };
        self.acc = 0.0;
        self.n = 0;
        Ok(vec![
            Format {
                rate: self.out_rate,
                centre_hz: f.tuned_hz,
                ..f.clone()
            },
            typed_format::<Level>(f),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        let Some(nco) = self.nco.as_mut() else {
            return;
        };
        self.mixed.resize(iq.len(), Complex32::new(0.0, 0.0));
        nco.mix(iq, &mut self.mixed);
        let first = io.output(0).first();
        let out = io.iq_out(0);
        let start = out.len();
        match self.decim.as_mut() {
            Some(d) => d.process(&self.mixed, out),
            None => out.extend_from_slice(&self.mixed),
        }
        if let Some(n) = self.narrow.as_mut() {
            n.process(&mut out[start..]);
        }
        let every = (self.out_rate * 0.2) as usize;
        let (rate, mut acc, mut n) = (self.out_rate, self.acc, self.n);
        let mut reports = std::mem::take(io.items_out::<Level>(1));
        let out = io.iq_out(0);
        for (i, z) in out[start..].iter().enumerate() {
            acc += z.norm_sqr() as f64;
            n += 1;
            if n >= every {
                let p = acc / n as f64;
                let db = if p > 0.0 { 10.0 * p.log10() } else { -200.0 };
                reports.push(Timed {
                    time: (first + (start + i) as u64) as f64 / rate,
                    value: Level { dbfs: db as f32 },
                });
                acc = 0.0;
                n = 0;
            }
        }
        self.acc = acc;
        self.n = n;
        *io.items_out::<Level>(1) = reports;
    }
}

/// `mpt.disc`: the FM discriminator, in Hz scaled by 1/3000.
#[derive(Default)]
pub struct MptDisc {
    last: Complex32,
    k: f32,
}

impl Component for MptDisc {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpt.disc",
            vec![Port::iq("in")],
            vec![Port::real("out").same_format_as(0)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.last = Complex32::new(0.0, 0.0);
        self.k = (inputs[0].rate / TAU / 3000.0) as f32;
        Ok(vec![inputs[0].with_kind(Kind::Real)])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        let mut last = self.last;
        let k = self.k;
        let out = io.real_out(0);
        out.extend(iq.iter().map(|&z| {
            let d = z * last.conj();
            last = z;
            atan2(d.im, d.re) * k
        }));
        self.last = last;
    }
}

/// `mpt.ffsk`: DC removal (the mean since the start, then a 2 s average, so a carrier error is removed at once) and
/// minimum-shift demodulation around 1500 Hz of audio at 24 kHz. The output is the soft value of the bit that ends at each 24 kHz sample (+90 degrees for each
/// bit at 1800 Hz, -90 degrees at 1200 Hz).
pub struct MptFfsk {
    mean: f64,
    n: u64,
    lpf: Fir<Complex32>,
    delay: [Complex32; SPB as usize],
    rot: [Complex32; 16],
}

impl Default for MptFfsk {
    fn default() -> Self {
        // 31-tap Hamming-windowed sinc, 900 Hz at 24 kHz, unity DC gain.
        let n = 31;
        let fc = 900.0 / FFSK_RATE;
        let mut taps: Vec<f64> = (0..n)
            .map(|i| {
                let m = i as f64 - 15.0;
                let s = if m == 0.0 {
                    2.0 * fc
                } else {
                    (TAU * fc * m).sin() / (PI * m)
                };
                s * (0.54 - 0.46 * (TAU * i as f64 / (n - 1) as f64).cos())
            })
            .collect();
        let sum: f64 = taps.iter().sum();
        taps.iter_mut().for_each(|t| *t /= sum);
        let taps: Vec<f32> = taps.iter().map(|&t| t as f32).collect();
        let rot = std::array::from_fn(|i| {
            let p = -TAU * i as f64 / 16.0;
            Complex32::new(p.cos() as f32, p.sin() as f32)
        });
        MptFfsk {
            mean: 0.0,
            n: 0,
            lpf: Fir::new(&taps),
            delay: [Complex32::new(0.0, 0.0); SPB as usize],
            rot,
        }
    }
}

impl MptFfsk {
    #[inline]
    fn demod(&mut self, x: f32) -> f32 {
        self.mean += (x as f64 - self.mean) / (self.n as f64 + 1.0).min(2.0 * FFSK_RATE);
        let y = x - self.mean as f32;
        let z = self.rot[(self.n % 16) as usize] * y;
        let f = self.lpf.process_sample(z);
        let slot = (self.n % SPB) as usize;
        let old = self.delay[slot];
        self.delay[slot] = f;
        self.n += 1;
        (f * old.conj()).im
    }
    /// Demodulates audio at 24 kHz, appending the soft values.
    pub fn run(&mut self, audio: &[f32], out: &mut Vec<f32>) {
        out.extend(audio.iter().map(|&v| self.demod(v)));
    }
}

impl Component for MptFfsk {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpt.ffsk",
            vec![Port::real("in").rate(RateRule::Exact(FFSK_RATE))],
            vec![Port::real("out").same_format_as(0)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = MptFfsk::default();
        Ok(vec![inputs[0].clone()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let mut out = std::mem::take(io.real_out(0));
        self.run(x, &mut out);
        *io.real_out(0) = out;
    }
}

/// How a codeword was framed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framed {
    /// A CCSC found by its SYNC.
    Ccsc,
    /// The codeword after a SYNC.
    AfterSync,
    /// Tolerant framing (check bits, then the grid).
    Tolerant,
}

/// `mpt.codeword`: one 64-bit codeword with how it checked and how it was framed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MptCodeword {
    /// Signal time of its end, s.
    pub time: f64,
    /// The 64 bits (repaired when one bit was wrong).
    pub bits: u64,
    /// Good, repaired or failed.
    pub check: Check,
    /// How it was found.
    pub framed: Framed,
    /// A control channel system codeword.
    pub ccsc: bool,
    /// Found in inverted polarity.
    pub inverted: bool,
    /// Tolerant framing is active.
    pub tolerant: bool,
    /// Locked (a good codeword within the last 3 s).
    pub locked: bool,
}

impl Item for MptCodeword {
    const KIND: &'static str = "mpt.codeword";
}

sdrkit_core::choice_enum! {
    /// How codewords are found.
    pub enum Framing {
        /// Only the SYNC word starts a codeword.
        Standard = ("standard", "Standard sync word"),
        /// The sync word is not checked.
        Tolerant = ("tolerant", "Ignore sync word (French networks)"),
        /// Standard, tolerant added after 8 s without an exact SYNC.
        Auto = ("auto", "Automatic"),
    }
}

/// `mpt.frame`: SYNC search per sample phase, standard and tolerant framing, the check and single-error repair.
pub struct MptFramer {
    mode: Framing,
    tolerant_on: bool,
    n: u64,
    soft: SampleRing<f32>,
    regs: [u128; SPB as usize],
    eye: [f32; SPB as usize],
    suppress_until: u64,
    pending: Option<(u64, bool)>,
    last_good: Option<u64>,
    exact_sync_seen: bool,
    votes: (u32, u32),
    tol_inv: bool,
    grid: Option<u64>,
    sync_run: Option<(u64, u64, bool)>,
    tol_run: Option<(u64, u64)>,
    candidate: Option<MptCodeword>,
    last_emit_end: Option<u64>,
}

impl Default for MptFramer {
    fn default() -> Self {
        MptFramer::new(Framing::Auto)
    }
}

const LOCK_SAMPLES: u64 = 3 * 24000;

impl MptFramer {
    /// A framer with this framing.
    pub fn new(mode: Framing) -> MptFramer {
        MptFramer {
            mode,
            tolerant_on: mode == Framing::Tolerant,
            n: 0,
            soft: SampleRing::new(4096).expect("a ring of 4096 values"),
            regs: [0; SPB as usize],
            eye: [0.0; SPB as usize],
            suppress_until: 0,
            pending: None,
            last_good: None,
            exact_sync_seen: false,
            votes: (0, 0),
            tol_inv: false,
            grid: None,
            sync_run: None,
            tol_run: None,
            candidate: None,
            last_emit_end: None,
        }
    }
    /// Changes the framing at once (the counters stay).
    pub fn set_mode(&mut self, mode: Framing) {
        self.mode = mode;
        self.tolerant_on = match mode {
            Framing::Standard => false,
            Framing::Tolerant => true,
            Framing::Auto => self.tolerant_on,
        };
    }
    fn locked(&self) -> bool {
        self.last_good
            .is_some_and(|g| self.n.saturating_sub(g) <= LOCK_SAMPLES)
    }
    fn best_phase(&self, centre: u64) -> u64 {
        let mut best = centre;
        let mut be = f32::MIN;
        for d in [
            0i64, -1, 1, -2, 2, -3, 3, -4, 4, -5, 5, -6, 6, -7, 7, -8, 8, -9, 9, -10,
        ] {
            let at = (centre as i64 + d) as u64;
            let e = self.eye[(at % SPB) as usize];
            if e > be {
                be = e;
                best = at;
            }
        }
        best
    }
    fn word_ending(&self, end: u64) -> u64 {
        let mut w = 0u64;
        for k in 0..64u64 {
            let s = self.soft.get(end - SPB * (63 - k)).unwrap_or(0.0);
            w = (w << 1) | (s < 0.0) as u64;
        }
        w
    }
    fn emit(&mut self, cw: MptCodeword, out: &mut Vec<Timed<MptCodeword>>) {
        out.push(Timed {
            time: cw.time,
            value: cw,
        });
    }
    fn make(
        &self,
        end: u64,
        bits: u64,
        check: Check,
        framed: Framed,
        inverted: bool,
    ) -> MptCodeword {
        MptCodeword {
            time: end as f64 / FFSK_RATE,
            bits,
            check,
            framed,
            ccsc: if framed == Framed::Tolerant {
                bits >> 63 == 0 && (code::tail(bits) ^ SYNC).count_ones() <= 4 && code::valid(bits)
            } else {
                code::is_ccsc(bits)
            },
            inverted,
            tolerant: self.tolerant_on,
            locked: self.locked(),
        }
    }
    /// Takes soft values at 24 kHz and appends the codewords found.
    pub fn run(&mut self, soft: &[f32], out: &mut Vec<Timed<MptCodeword>>) {
        for &s in soft {
            let n = self.n;
            self.soft.push(s);
            let ph = (n % SPB) as usize;
            self.regs[ph] = (self.regs[ph] << 1) | (s < 0.0) as u128;
            self.eye[ph] += (s.abs() - self.eye[ph]) / 32.0;
            if self.mode == Framing::Auto
                && !self.tolerant_on
                && !self.exact_sync_seen
                && n >= 8 * 24000
            {
                self.tolerant_on = true;
            }
            // A codeword after a SYNC.
            if let Some((centre, inv)) = self.pending {
                if n >= centre + 64 * SPB + 10 {
                    self.pending = None;
                    let start = self.best_phase(centre);
                    let end = start + 64 * SPB;
                    let mut w = self.word_ending(end);
                    if inv {
                        w = !w;
                    }
                    let (bits, check) = code::check_and_repair(w);
                    let dup = self.last_emit_end.is_some_and(|e| e.abs_diff(end) < 30);
                    if !dup {
                        let cw = self.make(end, bits, check, Framed::AfterSync, inv);
                        if check != Check::Failed {
                            self.last_good = Some(n);
                        }
                        self.last_emit_end = Some(end);
                        self.emit(cw, out);
                    }
                }
            }
            // The tolerant grid.
            if let Some(centre) = self.grid {
                if n >= centre + 10 {
                    let end = self.best_phase(centre);
                    let raw = self.word_ending(end);
                    let ones = raw.count_ones();
                    if code::valid(raw) && (8..=56).contains(&ones) {
                        self.vote(raw);
                        let inv = self.polarity();
                        let bits = if inv { !raw } else { raw };
                        if let Some(c) = self.candidate.take() {
                            self.emit(c, out);
                        }
                        let cw = self.make(end, bits, Check::Good, Framed::Tolerant, inv);
                        self.last_good = Some(n);
                        self.last_emit_end = Some(end);
                        self.emit(cw, out);
                        self.grid = Some(end + 64 * SPB);
                    } else {
                        self.grid = None;
                        self.candidate = None;
                    }
                }
            }
            if n < 80 * SPB {
                self.n += 1;
                continue;
            }
            let reg = self.regs[ph];
            // Standard framing: the run of samples whose last 16 bits are SYNC spans the open eye. Its middle is the centre.
            if self.mode != Framing::Tolerant {
                let low = reg as u16;
                let hit = low == SYNC || low == !SYNC;
                if let Some((a, b, inv)) = self.sync_run {
                    let same = hit && (low != SYNC) == inv;
                    if same && n == b + 1 && n - a < SPB {
                        self.sync_run = Some((a, n, inv));
                    } else {
                        self.sync_run = None;
                        // A real SYNC holds over several samples of the open eye. A single sample is noise in a closed eye.
                        if b - a >= 2 {
                            self.take_sync((a + b) / 2, inv, out);
                        }
                    }
                } else if hit && n >= self.suppress_until {
                    self.exact_sync_seen = true;
                    self.sync_run = Some((n, n, low != SYNC));
                }
            }
            // Tolerant framing: any word with valid check bits starts the grid. It is kept when the next word on the grid is
            // valid too (a random word passes the check one time in 65536).
            if self.tolerant_on
                && self.grid.is_none()
                && self.pending.is_none()
                && self.sync_run.is_none()
            {
                let raw = reg as u64;
                let ok = (8..=56).contains(&raw.count_ones()) && code::valid(raw);
                if let Some((a, b)) = self.tol_run {
                    if ok && n == b + 1 && n - a < SPB {
                        self.tol_run = Some((a, n));
                    } else {
                        self.tol_run = None;
                        let end = (a + b) / 2;
                        let raw = self.regs[(end % SPB) as usize] as u64;
                        self.vote(raw);
                        let inv = self.polarity();
                        let bits = if inv { !raw } else { raw };
                        self.candidate =
                            Some(self.make(end, bits, Check::Good, Framed::Tolerant, inv));
                        self.grid = Some(end + 64 * SPB);
                    }
                } else {
                    let near = self.last_emit_end.is_some_and(|e| n.abs_diff(e) < 30 * SPB);
                    if ok && !near {
                        self.tol_run = Some((n, n));
                    }
                }
            }
            self.n += 1;
        }
    }
    fn take_sync(&mut self, centre: u64, inv: bool, out: &mut Vec<Timed<MptCodeword>>) {
        let reg = self.regs[(centre % SPB) as usize];
        let mut w = reg as u64;
        if inv {
            w = !w;
        }
        let mut accept = false;
        if code::is_ccsc(w) {
            let dup = self.last_emit_end.is_some_and(|e| e.abs_diff(centre) < 30);
            if !dup {
                let cw = self.make(centre, w, Check::Good, Framed::Ccsc, inv);
                self.last_good = Some(self.n);
                self.last_emit_end = Some(centre);
                self.emit(cw, out);
            }
            accept = true;
        } else {
            let mut pre = (reg >> 16) as u16;
            if inv {
                pre = !pre;
            }
            if self.locked() || (pre ^ 0xAAAA).count_ones() <= 2 {
                accept = true;
            }
        }
        if accept {
            self.pending = Some((centre, inv));
            self.suppress_until = centre + 70 * SPB;
        }
    }
    fn vote(&mut self, raw: u64) {
        if raw >> 63 == 1 {
            self.votes.0 += 1;
        } else {
            self.votes.1 += 1;
        }
        // The majority decides. A tie keeps the last decision.
        if self.votes.0 > self.votes.1 {
            self.tol_inv = false;
        } else if self.votes.1 > self.votes.0 {
            self.tol_inv = true;
        }
    }
    fn polarity(&self) -> bool {
        self.tol_inv
    }
    /// Tolerant framing is active.
    pub fn tolerant_active(&self) -> bool {
        self.tolerant_on
    }
}

impl Component for MptFramer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpt.frame",
            vec![Port::real("in")],
            vec![typed_port::<MptCodeword>("out")],
        )
        .with_settings(FramerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = MptFramer::new(self.mode);
        Ok(vec![typed_format::<MptCodeword>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let mut out = std::mem::take(io.items_out::<MptCodeword>(0));
        self.run(x, &mut out);
        *io.items_out::<MptCodeword>(0) = out;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FramerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FramerSetting::Framing => self.set_mode(FRAMING.read(id, v)?),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(FramerSetting::Framing, Value::Choice(self.mode.index()))])
    }
}

/// How codewords are found.
pub const FRAMING: Setting<field::Choice<Framing>> = Setting::choice("framing", "Framing", Framing::Auto)
    .help("How codewords are found: by the standard SYNC word, by their check bits alone (networks with another sync word), or automatically.");

sdrkit_core::settings! {
    /// The settings of `mpt.frame`.
    pub enum FramerSetting {
        /// How codewords are found.
        Framing = FRAMING,
    }
}

/// `mpt.msg`: reads the good address codewords.
#[derive(Default)]
pub struct MptDecoder;

impl Component for MptDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpt.msg",
            vec![Port::typed::<MptCodeword>("in")],
            vec![typed_port::<MptMessage>("out")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_format::<MptMessage>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let cws = io.items::<MptCodeword>(0);
        let out = io.items_out::<MptMessage>(0);
        for c in cws {
            let cw = c.value;
            if cw.check == Check::Failed || cw.ccsc {
                continue;
            }
            if let Some(m) = decode(cw.bits, cw.time) {
                out.push(Timed {
                    time: c.time,
                    value: m,
                });
            }
        }
    }
}

/// A good codeword within this long keeps the lock, s.
const LOCK_S: f64 = 3.0;

sdrkit_core::choice_enum! {
    /// What a channel of the table is used for.
    pub enum ChannelUse {
        /// A control channel.
        Control = ("control", "Control channel"),
        /// A traffic channel named by a go-to-channel message.
        Traffic = ("traffic", "Traffic (GTC)"),
    }
}

/// One row of the channel table.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelRow {
    /// The channel number.
    pub channel: i32,
    /// What it is used for.
    pub usage: ChannelUse,
    /// Messages naming it.
    pub seen: u32,
    /// Signal time of the last one.
    pub last_time: f64,
    /// Last called (GTC).
    pub last_called: String,
    /// Last caller (GTC).
    pub last_caller: String,
    /// The row changed since it was last published.
    dirty: bool,
}

/// The frequency of channel `n`: f_cc + (n - n_cc) × spacing, `None` when it cannot be placed.
pub fn channel_frequency(n: i32, n_cc: i32, f_cc: f64, spacing_hz: f64) -> Option<f64> {
    if n < 0 || n_cc < 0 || spacing_hz <= 0.0 || f_cc <= 0.0 {
        return None;
    }
    let f = f_cc + (n - n_cc) as f64 * spacing_hz;
    (f > 0.0).then_some(f)
}

#[derive(Clone, Debug, PartialEq)]
struct MessageRow(MptMessage);

impl Shaped for MessageRow {
    type Row = Mpt1327MessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let m = &self.0;
        out.put(&Mpt1327MessageRow {
            time_s: m.time,
            r#type: m.kind,
            caller: &m.caller,
            called: &m.called,
            channel: (m.channel >= 0).then_some(i64::from(m.channel)),
            meaning: &m.meaning,
        });
    }
}

/// The control channel's own number.
pub const CONTROL_CHANNEL: Setting<field::I64> = Setting::new("control_channel", "Control channel number", "", -1)
    .range(-1.0, 1023.0, 1.0)
    .help("The control channel's own number, for the frequency column (-1: the first channel announced as a control channel).");
/// The spacing between channel numbers.
pub const SPACING: Setting<field::F64> =
    Setting::new("spacing_khz", "Channel spacing", "kHz", 12.5)
        .range(0.001, 1000.0, 0.001)
        .help("The spacing between channel numbers, for the frequency column.");

sdrkit_core::settings! {
    /// The settings of `mpt.tables`.
    pub enum TablesSetting {
        /// The control channel's own number.
        ControlChannel = CONTROL_CHANNEL,
        /// The channel spacing.
        Spacing = SPACING,
    }
}

/// `mpt.tables`: the counters, the lock, the message list, the channel table, the system identity, the events and the voice
/// grants.
pub struct MptTables {
    p: Published,
    list: LogRing<MessageRow>,
    channels: BTreeMap<i32, ChannelRow>,
    control_channel: i32,
    first_announced: i32,
    spacing_khz: f64,
    f_cc: f64,
    configured: bool,
    last_good_time: Option<f64>,
    now: f64,
    good: u64,
    repaired: u64,
    bad: u64,
    ccscs: u64,
    messages: u64,
    system: Option<u32>,
    sync_check_off: bool,
    level_dbfs: f32,
    logged_count: u64,
    events: Vec<Event>,
    grants: Vec<Timed<Grant>>,
    key: String,
    summary: String,
}

impl Default for MptTables {
    fn default() -> Self {
        MptTables {
            p: Published::new(Mpt1327Fields::SHAPE.fields)
                .with_tables(&[MPT1327_MESSAGES.def(), MPT1327_CHANNELS.def()]),
            list: LogRing::new(200),
            channels: BTreeMap::new(),
            control_channel: -1,
            first_announced: -1,
            spacing_khz: 12.5,
            f_cc: 0.0,
            configured: false,
            last_good_time: None,
            now: 0.0,
            good: 0,
            repaired: 0,
            bad: 0,
            ccscs: 0,
            messages: 0,
            system: None,
            sync_check_off: false,
            level_dbfs: -200.0,
            logged_count: 0,
            events: Vec::new(),
            grants: Vec::new(),
            key: String::new(),
            summary: String::new(),
        }
    }
}

impl MptTables {
    fn n_cc(&self) -> i32 {
        if self.control_channel >= 0 {
            self.control_channel
        } else {
            self.first_announced
        }
    }
    fn frequency(&self, ch: i32) -> Option<f64> {
        channel_frequency(ch, self.n_cc(), self.f_cc, self.spacing_khz * 1e3)
    }
    /// Marks every channel row for publishing (the frequencies moved).
    fn all_dirty(&mut self) {
        self.channels.values_mut().for_each(|r| r.dirty = true);
    }
    fn take_message(&mut self, m: &MptMessage) {
        self.messages += 1;
        self.list.push(MessageRow(m.clone()));
        let mut channel = String::new();
        if m.channel >= 0 {
            // Writing into a `String` cannot fail.
            let _ = write!(channel, "{}", m.channel);
        }
        self.events.push(MPT1327_MESSAGE.event(
            m.time,
            self.f_cc,
            &Mpt1327MessageEvent {
                r#type: m.kind,
                source: (m.caller != "-").then_some(m.caller.as_str()),
                target: (m.called != "-").then_some(m.called.as_str()),
                channel: (m.channel >= 0).then_some(channel.as_str()),
                text: &m.meaning,
                family: keys::Family::Voice,
            },
        ));
        if m.channel >= 0 {
            let r = self
                .channels
                .entry(m.channel)
                .or_insert_with(|| ChannelRow {
                    channel: m.channel,
                    usage: ChannelUse::Control,
                    seen: 0,
                    last_time: 0.0,
                    last_called: String::new(),
                    last_caller: String::new(),
                    dirty: true,
                });
            r.seen += 1;
            r.last_time = m.time;
            r.dirty = true;
            if m.kind == MptType::Gtc {
                r.usage = ChannelUse::Traffic;
                r.last_called.clone_from(&m.called);
                r.last_caller.clone_from(&m.caller);
            } else {
                r.usage = ChannelUse::Control;
            }
            if m.kind == MptType::Bcast && self.first_announced < 0 && m.announces_control {
                self.first_announced = m.channel;
                self.all_dirty();
            }
        }
        if m.kind == MptType::Gtc {
            self.grants.push(Timed {
                time: m.time,
                value: Grant {
                    protocol: "MPT1327",
                    talkgroup: m.called.clone(),
                    caller: m.caller.clone(),
                    freq_hz: self.frequency(m.channel).unwrap_or(0.0),
                    channel: m.channel,
                    group: true,
                    time: m.time,
                    ..Default::default()
                },
            });
        }
    }
    fn publish(&mut self) {
        let locked = self.last_good_time.is_some_and(|t| self.now - t <= LOCK_S);
        let lock_state = if !self.configured {
            LockState::Off
        } else if locked {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let r = self.p.results_mut();
        let sys = SystemText(self.system);
        if !self.configured {
            set_status(
                r,
                lock_state,
                Some(self.level_dbfs),
                &mut self.summary,
                format_args!("MPT1327: waiting for samples"),
            );
        } else if self.messages == 0 && !locked {
            set_status(
                r,
                lock_state,
                Some(self.level_dbfs),
                &mut self.summary,
                format_args!(
                    "MPT1327: searching for a control channel ({:.1} dBFS)",
                    self.level_dbfs
                ),
            );
        } else {
            set_status(
                r,
                lock_state,
                Some(self.level_dbfs),
                &mut self.summary,
                format_args!("MPT1327: system {sys}, {} messages", self.messages),
            );
        }
        if self.messages != self.logged_count {
            self.logged_count = self.messages;
            r.log(format!("MPT1327: {} messages, system {sys}", self.messages));
        }
        let (n_cc, f_cc, spacing_hz) = (self.n_cc(), self.f_cc, self.spacing_khz * 1e3);
        let p = &mut self.p;
        p.put(&Mpt1327Fields {
            locked,
            codewords_good: self.good as i64,
            codewords_repaired: self.repaired as i64,
            codewords_bad: self.bad as i64,
            ccscs: self.ccscs as i64,
            messages: self.messages as i64,
            system_identity: self.system.map(i64::from),
            sync_check_off: self.sync_check_off,
            level_dbfs: f64::from(self.level_dbfs),
        });
        log_rows(&self.list, &MPT1327_MESSAGES, self.now, p);
        for c in self.channels.values_mut().filter(|c| c.dirty) {
            c.dirty = false;
            self.key.clear();
            // Writing into a `String` cannot fail.
            let _ = write!(self.key, "{:05}", c.channel);
            MPT1327_CHANNELS.put(
                p,
                &self.key,
                &Mpt1327ChannelRow {
                    channel: i64::from(c.channel),
                    freq_hz: channel_frequency(c.channel, n_cc, f_cc, spacing_hz).map(Hertz),
                    r#use: c.usage,
                    seen: i64::from(c.seen),
                    last_called: (!c.last_called.is_empty()).then_some(c.last_called.as_str()),
                    last_caller: (!c.last_caller.is_empty()).then_some(c.last_caller.as_str()),
                },
            );
        }
    }
    /// The channel table, by channel number.
    pub fn channels(&self) -> impl Iterator<Item = &ChannelRow> {
        self.channels.values()
    }
}

/// The system identity for people ("-" when none was heard).
struct SystemText(Option<u32>);

impl std::fmt::Display for SystemText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(s) => write!(f, "{s}"),
            None => f.write_str("-"),
        }
    }
}

impl Component for MptTables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpt.tables",
            vec![
                Port::typed::<MptCodeword>("codewords"),
                Port::typed::<MptMessage>("messages"),
                Port::typed::<Level>("level"),
            ],
            vec![
                typed_port::<Grant>("grants"),
                Port::new("events", Kind::Messages).rate_from(sdrkit_graph::RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TablesSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let (cc, sp) = (self.control_channel, self.spacing_khz);
        *self = MptTables::default();
        self.control_channel = cc;
        self.spacing_khz = sp;
        self.f_cc = inputs[0].tuned_hz;
        self.configured = true;
        self.publish();
        Ok(vec![
            typed_format::<Grant>(&inputs[0]),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for c in io.items::<MptCodeword>(0) {
            let cw = &c.value;
            self.now = self.now.max(c.time);
            self.sync_check_off = cw.tolerant;
            match cw.check {
                Check::Failed => self.bad += 1,
                Check::Repaired => {
                    self.good += 1;
                    self.repaired += 1;
                }
                Check::Good => self.good += 1,
            }
            if cw.check != Check::Failed {
                self.last_good_time = Some(c.time);
                if cw.ccsc {
                    self.ccscs += 1;
                    self.system = Some(code::ccsc_system(cw.bits));
                }
            }
        }
        for m in io.items::<MptMessage>(1) {
            self.now = self.now.max(m.time);
            self.take_message(&m.value);
        }
        if let Some(l) = io.items::<Level>(2).last() {
            self.level_dbfs = l.value.dbfs;
            self.now = self.now.max(l.time);
        }
        io.items_out::<Grant>(0).append(&mut self.grants);
        io.messages_out(1).append(&mut self.events);
        if io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::ControlChannel => {
                self.control_channel = CONTROL_CHANNEL.read(id, v)? as i32
            }
            TablesSetting::Spacing => self.spacing_khz = SPACING.read(id, v)?,
        }
        self.all_dirty();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                TablesSetting::ControlChannel,
                Value::I64(i64::from(self.control_channel)),
            ),
            (TablesSetting::Spacing, Value::F64(self.spacing_khz)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
