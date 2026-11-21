//! The paging test signal: a loop of POCSAG transmissions at 512, 1200 and 2400 bit/s and one FLEX frame at each speed. The
//! words come from the `sdrkit-proto-pager` encoders, so FLEX pages carry their fragment checks, signatures and numeric checks,
//! and the long address is one of set 1-2. The symbols are frequency modulated with shaped transitions and streamed, since
//! a cycle at 2 Msps does not fit in memory. Noise, offsets and clock errors come from the framework's `gen_*` settings.

use num_complex::Complex32;
use sdrkit_algo_sim::modulate::Fm;
use sdrkit_core::{GenConfig, Generator, SignalInfo};
use sdrkit_proto_pager::flex::{self, Fiw, FlexTx, Speed as FlexSpeed, PHASE_WORDS};
use sdrkit_proto_pager::pocsag::{
    self, Content as PocsagContent, NumericSet, PocsagTx, PREAMBLE_BITS,
};
use sdrkit_proto_pager::Kind;
use std::f64::consts::PI;

/// The bits of FLEX sync 1 (bit sync and the words A, B and A inverted) before the frame information word.
const SYNC1_BITS: usize = flex::BIT_SYNC_BITS + flex::SYNC1_BITS as usize;

/// A transmission: symbols as (level from -3 to +3, length in seconds), sent with `full_dev` Hz for level 3.
#[derive(Clone, Debug, PartialEq)]
pub struct Tx {
    /// The symbols.
    pub symbols: Vec<(f32, f64)>,
    /// Hz for level 3.
    pub full_dev: f64,
}

/// A piece of the loop.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    /// A transmission.
    Tx(Tx),
    /// No carrier, s.
    Silence(f64),
}

/// A page of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct TestPage {
    /// The RIC or capcode.
    pub address: u32,
    /// The POCSAG function code (0 to 3).
    pub function: u8,
    /// Tone, numeric or text.
    pub kind: Kind,
    /// The message.
    pub text: String,
}

impl TestPage {
    /// A page.
    pub fn new(address: u32, function: u8, kind: Kind, text: &str) -> TestPage {
        TestPage {
            address,
            function,
            kind,
            text: text.into(),
        }
    }
}

/// The POCSAG transmission of `pages`: the preamble and the batches, a bit 1 sent as -3 (the lower frequency). Errors for a
/// page that the encoder refuses.
pub fn pocsag_tx(
    pages: &[TestPage],
    baud: f64,
    full_dev: f64,
    invert: bool,
) -> Result<Tx, sdrkit_proto_pager::Error> {
    Ok(pocsag_symbols(&pocsag_words(pages)?, baud, full_dev, invert))
}

/// The code words of the POCSAG batches that carry `pages`, sync words included, with the numeric set of ITU-R M.584-2.
/// Errors for a page that the encoder refuses.
pub fn pocsag_words(pages: &[TestPage]) -> Result<Vec<u32>, sdrkit_proto_pager::Error> {
    let tx: Vec<PocsagTx> = pages
        .iter()
        .map(|p| PocsagTx {
            ric: p.address,
            function: p.function,
            content: match p.kind {
                Kind::Numeric => PocsagContent::Numeric(&p.text),
                Kind::Alpha => PocsagContent::Alpha(&p.text),
                _ => PocsagContent::Tone,
            },
        })
        .collect();
    let mut words = Vec::new();
    pocsag::encode_batches(&tx, NumericSet::Itu, &mut words)?;
    Ok(words)
}

/// The symbols of a POCSAG transmission of `words`: the preamble of 576 reversals and the words, first bit first, at
/// `baud`. A bit 1 is sent as -3 (the lower frequency), or as +3 when `invert`.
pub fn pocsag_symbols(words: &[u32], baud: f64, full_dev: f64, invert: bool) -> Tx {
    let mut bits = Vec::new();
    pocsag::transmission_bits(words, PREAMBLE_BITS, &mut bits);
    let t = 1.0 / baud;
    let s = if invert { -3.0 } else { 3.0 };
    Tx {
        symbols: bits
            .into_iter()
            .map(|b| (if b == 1 { -s } else { s }, t))
            .collect(),
        full_dev,
    }
}

/// A FLEX frame: sync 1 (a bit 1 sent as the lower frequency), the frame information word, sync 2 and the phases that carry
/// `pages` (page `i` on phase `i` modulo the number of phases). The frame information word and the blocks send a 1 as the
/// upper frequency. `bad_fiw` damages the frame information word beyond repair. Errors for a page that the encoder refuses.
pub fn flex_tx(
    speed: FlexSpeed,
    frame: u8,
    pages: &[TestPage],
    full_dev: f64,
    invert: bool,
    bad_fiw: bool,
) -> Result<Tx, sdrkit_proto_pager::Error> {
    let inv = if invert { -1.0 } else { 1.0 };
    let mut head = Vec::new();
    flex::header_bits(
        speed,
        Fiw {
            cycle: 0,
            frame,
            rest: 0,
        },
        &mut head,
    );
    if bad_fiw {
        for (k, b) in head[SYNC1_BITS..].iter_mut().enumerate() {
            if [0, 2, 3, 13, 14].contains(&k) {
                *b ^= 1;
            }
        }
    }
    let s1 = 1.0 / 1600.0;
    // 64 symbols of dotting first, for the receiver's level and timing.
    let mut symbols: Vec<(f32, f64)> = (0..64)
        .map(|i| (if i % 2 == 0 { -3.0 * inv } else { 3.0 * inv }, s1))
        .collect();
    symbols.extend(head.iter().enumerate().map(|(k, &b)| {
        let upper = if k < SYNC1_BITS { b == 0 } else { b == 1 };
        (if upper { 3.0 * inv } else { -3.0 * inv }, s1)
    }));
    let np = speed.phases();
    let mut phases = vec![[0u32; PHASE_WORDS]; np];
    for (p, words) in phases.iter_mut().enumerate() {
        let mine: Vec<FlexTx> = pages
            .iter()
            .enumerate()
            .filter(|(i, _)| i % np == p)
            .map(|(_, x)| FlexTx {
                capcode: x.address,
                kind: x.kind,
                text: &x.text,
            })
            .collect();
        flex::encode_phase(&mine, words)?;
    }
    let mut levels = Vec::new();
    flex::data_symbols(speed, &phases, &mut levels)?;
    let td = 1.0 / speed.baud() as f64;
    symbols.extend(levels.into_iter().map(|l| (l as f32 * inv, td)));
    Ok(Tx { symbols, full_dev })
}

/// What the loop holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// POCSAG and FLEX.
    Mix,
    /// POCSAG only.
    Pocsag,
    /// FLEX only.
    Flex,
}

/// The pages of the cycle at one speed.
pub fn cycle_pages(speed: &str) -> Vec<TestPage> {
    let p = TestPage::new;
    match speed {
        "POCSAG 512" => vec![
            p(1234561, 0, Kind::Numeric, "555-0123 U 45"),
            p(1234563, 3, Kind::Alpha, "Test page 512: hello"),
        ],
        "POCSAG 1200" => vec![
            p(1234561, 0, Kind::Numeric, "12345 U 678"),
            p(1234562, 2, Kind::Tone, ""),
            p(1234567, 3, Kind::Alpha, "Test page 1200: hello"),
        ],
        "POCSAG 2400" => vec![
            p(1234561, 0, Kind::Numeric, "2400-9876"),
            p(1234563, 3, Kind::Alpha, "Test page 2400: hello"),
        ],
        s => {
            let short = s.trim_start_matches("FLEX ");
            vec![
                p(
                    1234567,
                    0,
                    Kind::Alpha,
                    &format!("Test page {short}: hello"),
                ),
                p(1234568, 0, Kind::Numeric, "555-0123 U 45"),
                p(1234569, 0, Kind::Tone, ""),
                p(1234570, 0, Kind::Alpha, &format!("Second page {short}")),
                p(2_201_234, 0, Kind::Alpha, &format!("Long address {short}")),
            ]
        }
    }
}

/// The speeds of the cycle, in order.
pub const SPEEDS: [&str; 7] = [
    "POCSAG 512",
    "POCSAG 1200",
    "POCSAG 2400",
    "FLEX 1600/2",
    "FLEX 3200/2",
    "FLEX 3200/4",
    "FLEX 6400/4",
];

/// The paging test signal.
#[derive(Clone, Debug)]
pub struct PagerSignal {
    /// Sample rate.
    pub rate: f64,
    /// What the loop holds.
    pub content: Content,
    /// Inverted polarity.
    pub invert: bool,
    /// One speed only.
    pub only: Option<String>,
    /// Full deviation of POCSAG, Hz.
    pub pocsag_dev: f64,
    /// Full deviation of FLEX, Hz.
    pub flex_dev: f64,
    /// Silence after each transmission, s.
    pub gap_s: f64,
    /// Segments to send instead of the cycle.
    pub custom: Option<Vec<Segment>>,
}

impl Default for PagerSignal {
    fn default() -> Self {
        PagerSignal {
            rate: 2e6,
            content: Content::Mix,
            invert: false,
            only: None,
            pocsag_dev: 4500.0,
            flex_dev: 4800.0,
            gap_s: 0.5,
            custom: None,
        }
    }
}

impl PagerSignal {
    /// The segments of one cycle.
    pub fn segments(&self) -> Vec<Segment> {
        if let Some(c) = &self.custom {
            return c.clone();
        }
        let mut out = Vec::new();
        for (i, s) in SPEEDS.iter().enumerate() {
            if self.only.as_deref().is_some_and(|o| o != *s) {
                continue;
            }
            let is_flex = i >= 3;
            if (is_flex && self.content == Content::Pocsag)
                || (!is_flex && self.content == Content::Flex)
            {
                continue;
            }
            let pages = cycle_pages(s);
            let tx = if is_flex {
                let speed = FlexSpeed::ALL[i - 3];
                flex_tx(
                    speed,
                    [3, 40, 77, 114][i - 3],
                    &pages,
                    self.flex_dev,
                    self.invert,
                    false,
                )
            } else {
                pocsag_tx(
                    &pages,
                    [512.0, 1200.0, 2400.0][i],
                    self.pocsag_dev,
                    self.invert,
                )
            };
            out.push(Segment::Tx(tx.expect("the cycle's pages are valid")));
            out.push(Segment::Silence(self.gap_s));
        }
        out
    }
    /// The streaming generator.
    pub fn generator(&self) -> PagerGenerator {
        PagerGenerator::new(self.clone())
    }
    /// The duration of one cycle, s.
    pub fn cycle_s(&self) -> f64 {
        self.segments().iter().map(length).sum()
    }
}

fn length(s: &Segment) -> f64 {
    match s {
        Segment::Tx(t) => t.symbols.iter().map(|x| x.1).sum::<f64>(),
        Segment::Silence(d) => *d,
    }
}

/// Where a transmission is: the time since its start, the current symbol and its start time.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SymClock {
    pub(crate) t: f64,
    pub(crate) k: usize,
    pub(crate) k_start: f64,
}

/// The frequency (Hz) at the clock's time: the symbol's level, with a raised-sine change over 30 % of a symbol at each end.
pub(crate) fn shaped_freq(clk: &mut SymClock, tx: &Tx) -> f64 {
    while clk.k + 1 < tx.symbols.len() && clk.t >= clk.k_start + tx.symbols[clk.k].1 {
        clk.k_start += tx.symbols[clk.k].1;
        clk.k += 1;
    }
    let (lv, len) = tx.symbols[clk.k];
    let w = 0.3 * len;
    let mut l = lv as f64;
    let d0 = clk.t - clk.k_start;
    let d1 = clk.k_start + len - clk.t;
    let s = |tau: f64| 0.5 + 0.5 * (PI * tau / (2.0 * w)).sin();
    if d0 < w && clk.k > 0 {
        let p = tx.symbols[clk.k - 1].0 as f64;
        l = p + (l - p) * s(d0);
    } else if d1 < w && clk.k + 1 < tx.symbols.len() {
        let nx = tx.symbols[clk.k + 1].0 as f64;
        l += (nx - l) * s(-d1);
    }
    l / 3.0 * tx.full_dev
}

/// The streaming generator of a [`PagerSignal`]. It sends unit power while it transmits.
pub struct PagerGenerator {
    sig: PagerSignal,
    segs: Vec<Segment>,
    seg: usize,
    clk: SymClock,
    seg_len: f64,
    fm: Fm,
    freq: Vec<f32>,
    gain: Vec<f32>,
    looping: bool,
}

impl PagerGenerator {
    fn new(sig: PagerSignal) -> PagerGenerator {
        let segs = sig.segments();
        let fm = Fm::new(sig.rate, 1.0)
            .expect("1 Hz of deviation for each unit fits any rate above 2 Hz");
        let mut g = PagerGenerator {
            sig,
            segs,
            seg: 0,
            clk: SymClock::default(),
            seg_len: 0.0,
            fm,
            freq: Vec::new(),
            gain: Vec::new(),
            looping: true,
        };
        g.enter();
        g
    }
    /// The stack's generator: the cycle, looping, the channel at the centre.
    pub fn from_config(cfg: &GenConfig) -> PagerGenerator {
        PagerSignal {
            rate: cfg.rate_hz,
            ..Default::default()
        }
        .generator()
    }
    /// Stops after one cycle instead of looping.
    pub fn once(mut self) -> Self {
        self.looping = false;
        self
    }
    fn enter(&mut self) {
        self.clk = SymClock::default();
        self.seg_len = self.segs.get(self.seg).map_or(0.0, length);
    }
}

impl Generator for PagerGenerator {
    /// The station is made at 0 Hz. The framework places it on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.sig.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let dt = 1.0 / self.sig.rate;
        self.freq.clear();
        self.gain.clear();
        let mut n = 0;
        for _ in 0..out.len() {
            while self.clk.t >= self.seg_len {
                self.seg += 1;
                if self.seg >= self.segs.len() {
                    if !self.looping || self.segs.is_empty() {
                        break;
                    }
                    self.seg = 0;
                }
                self.enter();
            }
            if self.seg >= self.segs.len() {
                break;
            }
            match &self.segs[self.seg] {
                Segment::Tx(tx) => {
                    let f = shaped_freq(&mut self.clk, tx);
                    let ramp = (self.clk.t / 1e-3)
                        .min((self.seg_len - self.clk.t) / 1e-3)
                        .clamp(0.0, 1.0);
                    self.freq.push(f as f32);
                    self.gain.push(ramp as f32);
                }
                Segment::Silence(_) => {
                    self.freq.push(0.0);
                    self.gain.push(0.0);
                }
            }
            self.clk.t += dt;
            n += 1;
        }
        // The two slices have the same length, so the modulator cannot fail.
        let _ = self.fm.process(&self.freq, &mut out[..n]);
        for (z, g) in out[..n].iter_mut().zip(&self.gain) {
            *z *= *g;
        }
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(16e3))
    }
}
