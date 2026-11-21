//! Olivia and Contestia: Walsh-Hadamard coded characters, scrambled and spread over the bit planes of Gray-coded MFSK symbols
//! with half-overlapping tone bursts. The receiver searches every time phase and frequency offset with fast Hadamard transforms
//! and integrates over four blocks. The generator sends the clean signal with Hann-shaped bursts; the sender shapes them with
//! the published window.
//!
//! Standard: P. Jalocha SP9VRC, "Olivia MFSK" (published description, 2005), and the Contestia variant.

use crate::decoder::HeardCommand;
use crate::decoder::{self, put_text, Heard, HfGenerator, Modulator, GEN_MESSAGE};
use crate::finder::CentreFinder;
use crate::mfsk::{put_keyboard, Keyboard, CENTRE, FIND_SIGNAL, TEST_TEXT};
use crate::util::{walsh_chip, walsh_decode};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::Record;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Schema, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port};
use sdrkit_util_bits::gray::{gray, ungray};
use sdrkit_util_text::store::TextStore;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// Olivia or Contestia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 7-bit characters, 64-chip words.
    Olivia,
    /// 6-bit upper-case characters, 32-chip words.
    Contestia,
}

impl Kind {
    /// Bits per character.
    pub fn bits(self) -> usize {
        match self {
            Kind::Olivia => 7,
            Kind::Contestia => 6,
        }
    }
    /// Chips per character = symbols per block.
    pub fn chips(self) -> usize {
        1 << (self.bits() - 1)
    }
    fn scrambler(self) -> (u64, usize, usize) {
        match self {
            Kind::Olivia => (0xE257_E6D0_2915_74EC, 64, 13),
            Kind::Contestia => (0xEDB8_8320, 32, 5),
        }
    }
    /// The name.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Olivia => "Olivia",
            Kind::Contestia => "Contestia",
        }
    }
}

/// The bit of the scrambling sequence for chip `t` of character `j` (a 1 negates the chip).
fn scramble(kind: Kind, j: usize, t: usize) -> bool {
    let (seq, len, mul) = kind.scrambler();
    let idx = (j * mul + t) % len;
    (seq >> idx) & 1 == 1
}

/// A character's value (Contestia maps to its 6-bit alphabet).
pub fn char_value(kind: Kind, c: u8) -> u32 {
    match kind {
        Kind::Olivia => (c & 0x7F) as u32,
        Kind::Contestia => {
            let c = c.to_ascii_uppercase();
            match c {
                b' ' => 59,
                b'\r' => 60,
                8 => 61,
                b'\n' | 0 => 0,
                33..=90 => (c - 32) as u32,
                _ => (b'?' - 32) as u32,
            }
        }
    }
}

/// The character of a value.
pub fn value_char(kind: Kind, v: u32) -> Option<u8> {
    match kind {
        Kind::Olivia => (v != 0).then_some(v as u8),
        Kind::Contestia => match v {
            0 => None,
            59 => Some(b' '),
            60 => Some(b'\r'),
            61 => Some(8),
            1..=58 => Some(v as u8 + 32),
            _ => None,
        },
    }
}

/// The tones of one block: `chars` (n of them, the bits per symbol) in place j = 0 to n - 1, each as its value's Walsh word
/// (chip t of a value v below half the chips is -1 when v AND (chips - 1 - t) has an odd count of ones; a larger value is the
/// word of v - half negated), scrambled, spread over the bit planes (a negative chip t of place j sets bit (j + t) mod n of
/// symbol t), and each symbol Gray coded. `tone` takes the tones in the order sent.
pub fn block_tones(kind: Kind, chars: &[u8], mut tone: impl FnMut(u32)) {
    let n = chars.len();
    let s = kind.chips();
    let mut syms = [0u32; 64];
    for (j, &c) in chars.iter().enumerate() {
        let v = char_value(kind, c);
        let half = s as u32;
        let (row, neg) = if v >= half {
            (v - half, true)
        } else {
            (v, false)
        };
        for (t, sym) in syms[..s].iter_mut().enumerate() {
            // The word's chips run from the last Walsh-Hadamard chip to the first.
            let mut chip = walsh_chip(row, (s - 1 - t) as u32, s.trailing_zeros());
            if neg {
                chip = -chip;
            }
            if scramble(kind, j, t) {
                chip = -chip;
            }
            if chip < 0.0 {
                *sym |= 1 << ((j + t) % n);
            }
        }
    }
    for &sym in &syms[..s] {
        tone(gray(sym));
    }
}

/// The geometry of a tones/bandwidth choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// Tones.
    pub tones: usize,
    /// Bits per symbol.
    pub n: usize,
    /// Bandwidth, Hz.
    pub bw: f64,
    /// FFT length = burst length at 8 kHz.
    pub ns: usize,
}

impl Geometry {
    /// For `tones` and `bw` (Hz).
    pub fn new(tones: usize, bw: f64) -> Self {
        let n = tones.trailing_zeros() as usize;
        let k = (bw / 125.0).log2().round() as i32;
        let ns = 1usize << (n as i32 + 7 - k).max(5);
        Geometry { tones, n, bw, ns }
    }
    /// Tone spacing = baud, Hz.
    pub fn spacing(&self) -> f64 {
        self.bw / self.tones as f64
    }
}

sdrkit_core::choice_enum! {
    /// The number of tones.
    pub enum Tones {
        /// 2.
        T2 = ("2", "2"),
        /// 4.
        T4 = ("4", "4"),
        /// 8.
        T8 = ("8", "8"),
        /// 16.
        T16 = ("16", "16"),
        /// 32.
        T32 = ("32", "32"),
        /// 64.
        T64 = ("64", "64"),
        /// 128.
        T128 = ("128", "128"),
        /// 256.
        T256 = ("256", "256"),
    }
}

impl Tones {
    /// The number.
    pub fn count(self) -> usize {
        2 << self.index()
    }
}

sdrkit_core::choice_enum! {
    /// The bandwidth.
    pub enum Bandwidth {
        /// 125 Hz.
        B125 = ("125", "125 Hz"),
        /// 250 Hz.
        B250 = ("250", "250 Hz"),
        /// 500 Hz.
        B500 = ("500", "500 Hz"),
        /// 1000 Hz.
        B1000 = ("1000", "1000 Hz"),
        /// 2000 Hz.
        B2000 = ("2000", "2000 Hz"),
    }
}

impl Bandwidth {
    /// The bandwidth in Hz.
    pub fn hz(self) -> f64 {
        125.0 * f64::from(1u32 << self.index())
    }
}

/// The `tones` setting.
pub const TONES: Setting<field::Choice<Tones>> =
    Setting::choice("tones", "Tones", Tones::T32).restart();
/// The `bandwidth` setting.
pub const BANDWIDTH: Setting<field::Choice<Bandwidth>> =
    Setting::choice("bandwidth", "Bandwidth", Bandwidth::B1000).restart();

sdrkit_core::settings! {
    /// The settings of Olivia and Contestia.
    pub enum OliviaSetting {
        /// The tones.
        Tones = TONES,
        /// The bandwidth.
        Bandwidth = BANDWIDTH,
        /// The audio centre.
        Centre = CENTRE,
        /// The centre follows the signal.
        FindSignal = FIND_SIGNAL,
    }
}

sdrkit_core::settings! {
    /// The test-signal settings of Olivia and Contestia.
    pub enum OliviaGenSetting {
        /// The text sent.
        Message = GEN_MESSAGE,
    }
}

/// The transmitter.
pub struct OliviaModulator {
    kind: Kind,
    g: Geometry,
    rate: f64,
    centre: f64,
    text: Vec<u8>,
    ti: usize,
    syms: std::collections::VecDeque<u32>,
    t: f64,
    k: i64,
    bursts: [(f64, f64, f64); 2],
    rng: Rng,
}

impl OliviaModulator {
    /// Sends `text` again and again at `centre` Hz. `seed` sets the phase jumps between the tone bursts.
    pub fn new(kind: Kind, g: Geometry, rate: f64, centre: f64, text: &str, seed: u64) -> Self {
        OliviaModulator {
            kind,
            g,
            rate,
            centre,
            text: text.as_bytes().to_vec(),
            ti: 0,
            syms: Default::default(),
            t: 0.0,
            k: -1,
            bursts: [(0.0, 0.0, 0.0); 2],
            rng: Rng::new(seed),
        }
    }

    fn block(&mut self) {
        let chars: Vec<u8> = (0..self.g.n)
            .map(|j| self.text[(self.ti + j) % self.text.len()])
            .collect();
        self.ti += self.g.n;
        block_tones(self.kind, &chars, |t| self.syms.push_back(t));
    }

    fn next_symbol(&mut self) -> u32 {
        if self.syms.is_empty() {
            self.block();
        }
        self.syms
            .pop_front()
            .expect("a block has at least one symbol")
    }
}

impl Modulator for OliviaModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let ns = self.g.ns as f64;
        let half = ns / 2.0;
        let lo = self.centre - self.g.bw / 2.0 + self.g.spacing() / 2.0;
        let step = 8000.0 / self.rate;
        for o in out {
            let k = (self.t / half).floor() as i64;
            while self.k < k {
                self.k += 1;
                let tone = self.next_symbol();
                let f = lo + tone as f64 * self.g.spacing();
                let (pf, pstart, pph) = self.bursts[1];
                let jump = if self.rng.bit() == 1 {
                    PI / 2.0
                } else {
                    -PI / 2.0
                };
                let ph = pph + TAU * pf * (self.k as f64 * half - pstart) / 8000.0 + jump;
                self.bursts = [self.bursts[1], (f, self.k as f64 * half, ph)];
            }
            let mut v = Complex32::default();
            for &(f, start, ph) in &self.bursts {
                let u = self.t - start;
                if (0.0..ns).contains(&u) {
                    let w = (PI * u / ns).sin().powi(2);
                    let p = ph + TAU * f * u / 8000.0;
                    v += Complex32::new((w * p.cos()) as f32, (w * p.sin()) as f32);
                }
            }
            *o = v;
            self.t += step;
        }
    }
}

/// The receiver.
pub struct OliviaDecoder {
    kind: Kind,
    g: Geometry,
    centre: f64,
    find: bool,
    dial: f64,
    nco: Nco,
    mixed: Vec<Complex32>,
    hist: SampleRing<Complex32>,
    spec: WindowedFft,
    hop: usize,
    hops: u64,
    margin: usize,
    noffs: usize,
    /// Soft bits: [hop ring][offset][bit plane].
    soft: Vec<f32>,
    ring: usize,
    sig: Vec<f32>,
    noise: Vec<f32>,
    chars: Vec<u8>,
    /// The ratio signal / sqrt(noise) of each block on its own: [phase][offset][block], as `chars`.
    block_ratio: Vec<f32>,
    chips: Vec<f32>,
    work: Vec<f32>,
    energies: Vec<f32>,
    last_out: Option<u64>,
    best: (usize, usize),
    ratio: f32,
    locked: bool,
    store: TextStore,
    heard: Heard,
    status: Status,
    label: String,
    tones: Tones,
    bandwidth: Bandwidth,
    finder: CentreFinder,
    t_in: u64,
}

const DELAY_BLOCKS: usize = 3;
/// The least lock ratio (signal / sqrt(noise) of the best phase and offset) of [`lock_ratio`].
const LOCK_RATIO: f32 = 4.0;
/// The signal / sqrt(noise) that one block must exceed for its characters to be shown. A block of digital silence gives 0; a
/// block with k chips of signal and the rest empty (the edge of a burst holds 1 to 3) gives at most sqrt(k); noise alone gives
/// about 2.4 to 3, so a weak signal keeps every block it had.
const BLOCK_RATIO: f32 = 2.0;

/// The lock ratio for `n` characters per block. On noise alone the best phase and offset reach about 2.1 + 2.1 / sqrt(n) in
/// a minute (4.2 for 2 tones, 3.6 for 4 tones): a block of fewer characters averages less. The lock is 2.1 + 2.5 / sqrt(n),
/// at least [`LOCK_RATIO`].
fn lock_ratio(n: usize) -> f32 {
    (2.1 + 2.5 / (n as f32).sqrt()).max(LOCK_RATIO)
}

/// The ratio signal / sqrt(noise) of a correlation; 0 without signal.
fn ratio(sig: f32, noise: f32) -> f32 {
    if sig > 0.0 {
        sig / noise.max(1e-20).sqrt()
    } else {
        0.0
    }
}

impl OliviaDecoder {
    /// A receiver of 32 tones in 1000 Hz at the default settings.
    pub fn new(kind: Kind) -> Self {
        Self::with(kind, Geometry::new(32, 1000.0))
    }

    /// A receiver of one geometry.
    pub fn with(kind: Kind, g: Geometry) -> Self {
        let mut d = OliviaDecoder {
            kind,
            g,
            centre: 1500.0,
            find: true,
            dial: 0.0,
            nco: Nco::new(8000.0, -1500.0),
            mixed: Vec::new(),
            hist: SampleRing::new(g.ns).expect("a ring of at least one sample"),
            spec: WindowedFft::new(g.ns, g.ns, Window::Hann)
                .expect("a window that fits the transform"),
            hop: 0,
            hops: 0,
            margin: 0,
            noffs: 0,
            soft: Vec::new(),
            ring: 0,
            sig: Vec::new(),
            noise: Vec::new(),
            chars: Vec::new(),
            block_ratio: Vec::new(),
            chips: vec![0.0; kind.chips()],
            work: vec![0.0; kind.chips()],
            energies: Vec::new(),
            last_out: None,
            best: (0, 0),
            ratio: 0.0,
            locked: false,
            store: decoder::store(8000, 0),
            heard: Heard::new(kind.name()),
            status: Status::default(),
            label: String::new(),
            tones: TONES.default,
            bandwidth: BANDWIDTH.default,
            finder: CentreFinder::new(g.bw as f32),
            t_in: 0,
        };
        d.rebuild();
        d
    }

    fn rebuild(&mut self) {
        let g = self.g;
        let binw = 8000.0 / g.ns as f64;
        let lowest_bin = ((self.centre - g.bw / 2.0) / binw).floor().max(0.0) as usize;
        self.margin = 8.min(lowest_bin);
        self.noffs = 2 * self.margin + 1;
        let s = self.kind.chips();
        self.ring = 2 * s;
        let phases = 2 * s;
        self.soft = vec![0.0; self.ring * self.noffs * g.n];
        self.sig = vec![0.0; phases * self.noffs];
        self.noise = vec![1e-12; phases * self.noffs];
        self.chars = vec![0; phases * self.noffs * (DELAY_BLOCKS + 1) * g.n];
        self.block_ratio = vec![0.0; phases * self.noffs * (DELAY_BLOCKS + 1)];
        self.energies = vec![0.0; g.tones];
        self.nco = Nco::new(8000.0, -self.centre);
        self.hist.reset();
        self.hop = 0;
        self.hops = 0;
        self.last_out = None;
        self.locked = false;
        self.ratio = 0.0;
        put_text(
            &mut self.label,
            format_args!("{} {}/{}", self.kind.name(), self.g.tones, self.g.bw),
        );
    }

    /// Tuning error, Hz.
    pub fn offset_hz(&self) -> f64 {
        (self.best.1 as f64 - self.margin as f64) * 8000.0 / self.g.ns as f64
    }

    fn hop(&mut self, out: &mut Outbox) {
        let g = self.g;
        let ns = g.ns as i32;
        self.spec.run_power(self.hist.last(g.ns));
        let slot = (self.hops % self.ring as u64) as usize;
        for o in 0..self.noffs {
            let off = o as i32 - self.margin as i32;
            let mut sum = 0.0;
            for k in 0..g.tones {
                let bin = 2 * k as i32 + 1 - g.tones as i32 + off;
                let e = self.spec.power()[bin.rem_euclid(ns) as usize];
                let e2 = e * e;
                self.energies[k] = e2;
                sum += e2;
            }
            let base = (slot * self.noffs + o) * g.n;
            for b in 0..g.n {
                self.soft[base + b] = 0.0;
            }
            if sum > 0.0 {
                for k in 0..g.tones {
                    let p = self.energies[k] / sum;
                    let sym = ungray(k as u32);
                    for b in 0..g.n {
                        self.soft[base + b] += if (sym >> b) & 1 == 1 { p } else { -p };
                    }
                }
            }
        }
        self.hops += 1;
        let s = self.kind.chips();
        if self.hops < (2 * s) as u64 {
            return;
        }
        // Decode the block that ends at this hop, at every offset.
        let phase = (self.hops % (2 * s) as u64) as usize;
        let blk = ((self.hops / (2 * s) as u64) % (DELAY_BLOCKS as u64 + 1)) as usize;
        for o in 0..self.noffs {
            let mut sig = 0.0;
            let mut noise = 0.0;
            for j in 0..g.n {
                for t in 0..s {
                    let h = (self.hops - 1 - 2 * (s - 1 - t) as u64) % self.ring as u64;
                    let v = self.soft[(h as usize * self.noffs + o) * g.n + (j + t) % g.n];
                    let mut chip = -v;
                    if scramble(self.kind, j, t) {
                        chip = -chip;
                    }
                    self.chips[s - 1 - t] = chip;
                }
                let (v, best, rest) = walsh_decode(&self.chips, &mut self.work);
                sig += best;
                noise += rest / (s - 1) as f32;
                let ci = ((phase * self.noffs + o) * (DELAY_BLOCKS + 1) + blk) * g.n + j;
                self.chars[ci] = v as u8;
            }
            let i = phase * self.noffs + o;
            self.block_ratio[i * (DELAY_BLOCKS + 1) + blk] =
                ratio(sig / g.n as f32, noise / g.n as f32);
            self.sig[i] += 0.25 * (sig / g.n as f32 - self.sig[i]);
            self.noise[i] += 0.25 * (noise / g.n as f32 - self.noise[i]);
        }
        // The best phase and offset.
        let mut best = (0usize, 0usize, 0.0f32);
        for ph in 0..2 * s {
            for o in 0..self.noffs {
                let i = ph * self.noffs + o;
                let r = ratio(self.sig[i], self.noise[i]);
                if r > best.2 {
                    best = (ph, o, r);
                }
            }
        }
        self.best = (best.0, best.1);
        self.ratio = best.2;
        let locked = self.ratio >= lock_ratio(g.n);
        if locked && !self.locked {
            self.heard.logs.push(format!(
                "{} locked at {:.0} Hz",
                self.label,
                self.centre + self.offset_hz()
            ));
        }
        self.locked = locked;
        // At the winning phase, the characters decided four blocks earlier, when that block on its own exceeds BLOCK_RATIO: the
        // integration holds the lock for some blocks past the ends of a burst, and a block of silence or with a sliver of the
        // burst carries no characters.
        if locked && phase == best.0 && self.last_out.is_none_or(|l| self.hops >= l + s as u64) {
            self.last_out = Some(self.hops);
            let old = (blk + 1) % (DELAY_BLOCKS + 1);
            let own = self.block_ratio[(phase * self.noffs + best.1) * (DELAY_BLOCKS + 1) + old];
            if self.hops >= ((DELAY_BLOCKS + 1) * 2 * s) as u64 && own > BLOCK_RATIO {
                for j in 0..g.n {
                    let ci = ((phase * self.noffs + best.1) * (DELAY_BLOCKS + 1) + old) * g.n + j;
                    if let Some(c) = value_char(self.kind, self.chars[ci] as u32) {
                        let time = self.t_in as f64 / 8000.0;
                        let snr = self.snr_db();
                        let f = self.dial + self.centre + self.offset_hz();
                        self.heard
                            .char(&mut self.store, out, c, time, 100, 0, f, snr);
                    }
                }
            }
        }
    }

    fn snr_db(&self) -> f32 {
        10.0 * self.ratio.max(1e-6).log10()
    }
}

impl sdrkit_graph::Decoder for OliviaDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        match self.kind {
            Kind::Olivia => "hfkbd.olivia",
            Kind::Contestia => "hfkbd.contestia",
        }
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        OliviaSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match OliviaSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OliviaSetting::Tones => self.tones = TONES.read(id, value)?,
            OliviaSetting::Bandwidth => self.bandwidth = BANDWIDTH.read(id, value)?,
            OliviaSetting::Centre => {
                self.centre = CENTRE.read(id, value)?;
                self.rebuild();
                return Ok(());
            }
            OliviaSetting::FindSignal => {
                self.find = FIND_SIGNAL.read(id, value)?;
                return Ok(());
            }
        }
        self.g = Geometry::new(self.tones.count(), self.bandwidth.hz());
        self.hist = SampleRing::new(self.g.ns).expect("a ring of at least one sample");
        self.spec = WindowedFft::new(self.g.ns, self.g.ns, Window::Hann)
            .expect("a window that fits the transform");
        self.finder = CentreFinder::new(self.g.bw as f32);
        self.rebuild();
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (OliviaSetting::Tones, Value::Choice(self.tones.index())),
            (
                OliviaSetting::Bandwidth,
                Value::Choice(self.bandwidth.index()),
            ),
            (OliviaSetting::Centre, Value::Hz(self.centre)),
            (OliviaSetting::FindSignal, Value::Bool(self.find)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results(match self.kind {
            Kind::Olivia => "olivia",
            Kind::Contestia => "contestia",
        })
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        let dial = input.tuned_hz;
        self.dial = dial;
        self.rebuild();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        let dial = input.tuned_hz;
        self.dial = dial;
        self.rebuild();
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
        let (snr, offset) = (self.snr_db(), self.offset_hz());
        let st = &mut self.status;
        st.lock_state = if self.locked {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = Some(snr);
        st.offset_hz = Some(offset);
        let what = if self.locked { "locked" } else { "searching" };
        put_text(&mut st.summary, format_args!("{} {what}", self.kind.name()));
        res.results_mut().set_status(st);
        put_keyboard(
            res,
            &Keyboard {
                variant: &self.label,
                centre_hz: self.centre,
                offset_hz: self.offset_hz(),
                snr_db: self.snr_db(),
                store: &self.store,
                quality: if self.locked { 1.0 } else { 0.0 },
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
        self.rebuild();
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
    }
}

impl OliviaDecoder {
    fn piece(&mut self, x: &[Complex32], out: &mut Outbox) {
        self.mixed.clear();
        self.mixed.resize(x.len(), Complex32::default());
        self.nco.mix(x, &mut self.mixed);
        let hop = self.g.ns / 4;
        let mixed = std::mem::take(&mut self.mixed);
        for &s in &mixed {
            self.hist.push(s);
            self.t_in += 1;
            self.hop += 1;
            if self.hop == hop {
                self.hop = 0;
                self.hop(out);
            }
        }
        self.mixed = mixed;
        // The signal finder decides at the end of the piece, after the decoder has seen it.
        let busy = self.locked && self.store.plausible();
        if self.find {
            if let Some(c) = self.finder.feed(x, self.centre as f32, !busy) {
                self.heard.logs.push(format!("signal found at {c:.0} Hz"));
                self.centre = c as f64;
                self.rebuild();
            }
        }
    }
}

/// The generator. It reads the settings `tones`, `bandwidth`, `centre` and `gen_message`.
pub fn generator(kind: Kind, cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let g = Geometry::new(v.get(&TONES).count(), v.get(&BANDWIDTH).hz());
    let centre = v.get(&CENTRE);
    let text = decoder::message(cfg, TEST_TEXT);
    let m = OliviaModulator::new(kind, g, cfg.rate_hz, centre, &text, cfg.seed);
    let baud = 8000.0 / g.ns as f64;
    // Two Hann bursts overlap by half: the mean power of a unit burst train is 1/2.
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(g.bw).with_symbol_rate(baud).with_power(0.5),
    ))
}

/// The symbol window of the sender over x from -pi to pi (the published Olivia window).
pub fn tx_window(x: f64) -> f64 {
    1.0 + 1.191_378_572_3 * x.cos()
        - 0.079_301_855_8 * (2.0 * x).cos()
        - 0.217_144_202_6 * (3.0 * x).cos()
        - 0.001_452_607_6 * (4.0 * x).cos()
}

/// The largest sum of two symbol windows half a window apart: the gain that makes two overlapping symbols peak at 1.
fn tx_window_peak() -> f64 {
    (0..=1024)
        .map(|i| {
            let x = -PI + PI * f64::from(i) / 1024.0;
            tx_window(x) + tx_window(x + PI)
        })
        .fold(0.0, f64::max)
}

/// The tone grid of a sender at an audio centre: the transform bins are 8000 / Ns Hz apart; the lowest tone is at bin b0 =
/// (integer part of (centre - D) Ns / 8000) + 1, with D = bandwidth (1 - 0.5 / tones) / 2 for Olivia and bandwidth / 2 for
/// Contestia; tone k is `step` bins above it, `step` the bins of one spacing (bandwidth / tones).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxGrid {
    /// The transform length.
    pub ns: usize,
    /// The bin of tone 0.
    pub b0: i64,
    /// Bins between adjacent tones.
    pub step: i64,
    /// Tones.
    pub tones: usize,
}

impl TxGrid {
    /// The grid of `kind` and `g` at `centre` Hz.
    pub fn new(kind: Kind, g: Geometry, centre: f64) -> Self {
        let bin = 8000.0 / g.ns as f64;
        let d = match kind {
            Kind::Olivia => g.bw * (1.0 - 0.5 / g.tones as f64) / 2.0,
            Kind::Contestia => g.bw / 2.0,
        };
        TxGrid {
            ns: g.ns,
            b0: ((centre - d) / bin).floor() as i64 + 1,
            step: (g.spacing() / bin).round() as i64,
            tones: g.tones,
        }
    }
    /// The frequency of tone `k`, Hz.
    pub fn tone_hz(&self, k: usize) -> f64 {
        (self.b0 + self.step * k as i64) as f64 * 8000.0 / self.ns as f64
    }
    /// The middle of the tones, Hz.
    pub fn middle_hz(&self) -> f64 {
        (self.tone_hz(0) + self.tone_hz(self.tones - 1)) / 2.0
    }
}

/// The silence before the first block of a burst, samples at 8 kHz.
const TX_LEAD: usize = 800;
/// The blocks of NUL after the block of the last text character.
const TX_TAIL_BLOCKS: usize = 5;
/// The seed of the phase steps between symbols.
const TX_PHASE_SEED: u64 = 0x0E11_A5ED;

/// The coder of Olivia and Contestia: samples at 8 kHz around the middle of the tone grid ([`TxGrid`]). A burst is 0.1 s of
/// silence, then blocks of n characters (n the bits per symbol) in `kind.chips()` symbols each ([`block_tones`]): the first
/// block starts with NUL. After the text, NUL characters complete its block and 5 blocks of NUL follow. Each symbol is a tone of
/// Ns samples shaped by [`tx_window`], one every Ns / 2 samples (half overlap); its phase starts where the previous tone would
/// be at that time, plus or minus 90 degrees at random. Two overlapping symbols peak at 1 at most.
pub struct OliviaCoder {
    kind: Kind,
    g: Geometry,
    grid: TxGrid,
    chars: Vec<u8>,
    tones: Vec<u32>,
    tail: Vec<Complex32>,
    burst: Vec<Complex32>,
    /// The phase at its start and the frequency (cycles per sample) of the last symbol.
    last: Option<(f64, f64)>,
    gain: f64,
    rng: Rng,
}

impl OliviaCoder {
    /// A coder of `kind` with `g` around the audio centre `centre`.
    pub fn new(kind: Kind, g: Geometry, centre: f64) -> Self {
        OliviaCoder {
            kind,
            g,
            grid: TxGrid::new(kind, g, centre),
            chars: Vec::with_capacity(g.n),
            tones: Vec::with_capacity(64),
            tail: vec![Complex32::default(); g.ns / 2],
            burst: vec![Complex32::default(); g.ns],
            last: None,
            gain: 1.0 / tx_window_peak(),
            rng: Rng::new(TX_PHASE_SEED),
        }
    }

    /// The tone grid.
    pub fn grid(&self) -> TxGrid {
        self.grid
    }

    /// One symbol of tone `k`: half a window of samples out.
    fn symbol(&mut self, k: u32, out: &mut Vec<Complex32>) {
        let ns = self.g.ns;
        let h = ns / 2;
        let rel = (2 * self.grid.step * i64::from(k)
            - self.grid.step * (self.grid.tones as i64 - 1)) as f64
            / (2.0 * ns as f64);
        let ph = match self.last {
            None => 0.0,
            Some((p, f)) => {
                let jump = if self.rng.bit() == 1 {
                    PI / 2.0
                } else {
                    -PI / 2.0
                };
                (p + TAU * f * h as f64 + jump).rem_euclid(TAU)
            }
        };
        self.last = Some((ph, rel));
        for (m, b) in self.burst.iter_mut().enumerate() {
            let w = tx_window(-PI + TAU * m as f64 / ns as f64) * self.gain;
            let p = ph + TAU * rel * m as f64;
            *b = Complex32::new((w * p.cos()) as f32, (w * p.sin()) as f32);
        }
        out.extend(self.tail.iter().zip(&self.burst[..h]).map(|(a, b)| a + b));
        self.tail.copy_from_slice(&self.burst[h..]);
    }

    /// The block of the characters collected, completed with NUL.
    fn block(&mut self, out: &mut Vec<Complex32>) {
        self.chars.resize(self.g.n, 0);
        let mut tones = std::mem::take(&mut self.tones);
        tones.clear();
        block_tones(self.kind, &self.chars, |t| tones.push(t));
        self.chars.clear();
        for &t in &tones {
            self.symbol(t, out);
        }
        self.tones = tones;
    }

    fn byte(&mut self, b: u8, out: &mut Vec<Complex32>) {
        self.chars.push(b);
        if self.chars.len() == self.g.n {
            self.block(out);
        }
    }
}

impl crate::sender::WaveCoder for OliviaCoder {
    fn centre_hz(&self) -> f64 {
        self.grid.middle_hz()
    }
    fn begin(&mut self, out: &mut Vec<Complex32>) {
        out.resize(out.len() + TX_LEAD, Complex32::default());
        self.byte(0, out);
    }
    /// A character above 255 is sent as `?`.
    fn char(&mut self, c: char, out: &mut Vec<Complex32>) {
        self.byte(u8::try_from(c).unwrap_or(b'?'), out);
    }
    fn end(&mut self, out: &mut Vec<Complex32>) {
        if !self.chars.is_empty() {
            self.block(out);
        }
        for _ in 0..TX_TAIL_BLOCKS * self.g.n {
            self.byte(0, out);
        }
        out.extend_from_slice(&self.tail);
    }
}

sdrkit_core::settings! {
    /// The settings of the Olivia and Contestia senders.
    pub enum OliviaTxSetting {
        /// The tones.
        Tones = TONES,
        /// The bandwidth.
        Bandwidth = BANDWIDTH,
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

/// The Olivia or Contestia side of the sender.
#[derive(Clone, Debug)]
pub struct OliviaModem {
    kind: Kind,
    tones: Tones,
    bandwidth: Bandwidth,
    centre: f64,
    demod: crate::front::Demod,
    common: crate::sender::Common,
}

impl OliviaModem {
    /// The sender of `kind` at the default settings.
    pub fn new(kind: Kind) -> Self {
        OliviaModem {
            kind,
            tones: TONES.default,
            bandwidth: BANDWIDTH.default,
            centre: CENTRE.default,
            demod: crate::front::DEMOD.default,
            common: crate::sender::Common::default(),
        }
    }
    fn geometry(&self) -> Geometry {
        Geometry::new(self.tones.count(), self.bandwidth.hz())
    }
    /// The tone grid at the settings now.
    pub fn grid(&self) -> TxGrid {
        TxGrid::new(self.kind, self.geometry(), self.centre)
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match OliviaTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OliviaTxSetting::Tones => self.tones = TONES.read(id, v)?,
            OliviaTxSetting::Bandwidth => self.bandwidth = BANDWIDTH.read(id, v)?,
            OliviaTxSetting::Centre => self.centre = CENTRE.read(id, v)?,
            OliviaTxSetting::Demod => self.demod = crate::front::DEMOD.read(id, v)?,
            OliviaTxSetting::Level => self.common.level = crate::sender::TX_LEVEL.read(id, v)?,
            OliviaTxSetting::TuneLimit => {
                self.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (OliviaTxSetting::Tones, Value::Choice(self.tones.index())),
            (
                OliviaTxSetting::Bandwidth,
                Value::Choice(self.bandwidth.index()),
            ),
            (OliviaTxSetting::Centre, Value::Hz(self.centre)),
            (OliviaTxSetting::Demod, Value::Choice(self.demod.index())),
            (OliviaTxSetting::Level, Value::F64(self.common.level)),
            (
                OliviaTxSetting::TuneLimit,
                Value::Duration(self.common.tune_s),
            ),
        ])
    }
    /// Olivia: bytes 0 to 255 as typed (sent as the byte AND 127); a character above 255 refuses the text. Contestia: every
    /// character; lower case is sent as upper case and a character outside its alphabet as `?`.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        if self.kind == Kind::Olivia {
            if let Some(c) = text.chars().find(|&c| u8::try_from(c).is_err()) {
                return Err(crate::sender::TxError::Character(c));
            }
        }
        out.extend(text.chars());
        Ok(())
    }
    fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
        crate::sender::lower_of(self.demod)?;
        Ok(crate::sender::Coding::Wave(Box::new(OliviaCoder::new(
            self.kind,
            self.geometry(),
            self.centre,
        ))))
    }
    /// The tones and half a spacing each side.
    fn band(&self) -> (f64, f64) {
        let g = self.grid();
        let half = self.geometry().spacing() / 2.0;
        (g.tone_hz(0) - half, g.tone_hz(g.tones - 1) + half)
    }
}

/// The Olivia side of the sender.
#[derive(Clone, Debug)]
pub struct OliviaTxModem(pub OliviaModem);

impl Default for OliviaTxModem {
    fn default() -> Self {
        OliviaTxModem(OliviaModem::new(Kind::Olivia))
    }
}

/// The Contestia side of the sender.
#[derive(Clone, Debug)]
pub struct ContestiaTxModem(pub OliviaModem);

impl Default for ContestiaTxModem {
    fn default() -> Self {
        ContestiaTxModem(OliviaModem::new(Kind::Contestia))
    }
}

/// The modem trait for a wrapper of [`OliviaModem`].
macro_rules! olivia_modem {
    ($t:ty, $name:literal) => {
        impl crate::sender::Modem for $t {
            const NAME: &'static str = $name;
            fn schema() -> Arc<Schema> {
                OliviaTxSetting::schema()
            }
            fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
                self.0.apply(id, v)
            }
            fn values(&self) -> Values {
                self.0.values()
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
                self.0.coder()
            }
            fn centre_hz(&self) -> f64 {
                self.0.centre
            }
            fn band(&self) -> (f64, f64) {
                self.0.band()
            }
            fn lower(&self) -> bool {
                crate::sender::lower_of(self.0.demod).unwrap_or(false)
            }
        }
    };
}

olivia_modem!(OliviaTxModem, "hfkbd.oliviatx");
olivia_modem!(ContestiaTxModem, "hfkbd.contestiatx");

/// The Olivia sender component.
pub fn olivia_sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<OliviaTxModem>::default())
}

/// The Contestia sender component.
pub fn contestia_sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<ContestiaTxModem>::default())
}

/// The Olivia generator.
pub fn olivia_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    generator(Kind::Olivia, cfg)
}

/// The Contestia generator.
pub fn contestia_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    generator(Kind::Contestia, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry() {
        let g = Geometry::new(32, 1000.0);
        assert_eq!(g.ns, 512);
        assert_eq!(g.spacing(), 31.25);
        assert_eq!(Geometry::new(8, 250.0).bw, 250.0);
    }

    #[test]
    fn contestia_alphabet() {
        for c in b"CQ DE K0TST 0123456789?" {
            let v = char_value(Kind::Contestia, *c);
            assert_eq!(value_char(Kind::Contestia, v), Some(*c));
        }
        assert_eq!(
            value_char(Kind::Contestia, char_value(Kind::Contestia, b'a')),
            Some(b'A')
        );
    }
}
