//! Multitone paging (`multitone`): a detector of Multitone transmissions, not a decoder. For every transmission the stack
//! measures what is there (carrier, deviation, tones, FSK) and returns no message text.

pub mod analysis;

use crate::fmtx::{AudioFm, Looping};
use crate::front::{CarrierState, PagerDisc, PagerFront};
use crate::typed::*;
use crate::{set_status, typed_format, typed_port};
use analysis::{measure, Burst, Transmission};
use num_complex::Complex32;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, GenConfig,
    Generator, LockState, Record, Results, ResultsDecl, SignalInfo, StackConfig, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError, Io,
    Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_proto_pager::word;
use sdrkit_util_objects::commands::CLEAR;
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

const MAX_PIECE: usize = 6 * 48000;

/// `mt.segment`: carrier up .. carrier gone (the paging front end's detector), the last 0.31 s trimmed, pieces shorter
/// than 0.2 s dropped, at most 6 s per piece. Each piece is measured.
pub struct MtSegment {
    fifo: VecDeque<f32>,
    h: [f32; 2],
    piece: Vec<f32>,
    start: f64,
    peak_snr: f64,
    active: bool,
    n: u64,
    serial: u64,
}

impl Default for MtSegment {
    fn default() -> Self {
        MtSegment {
            fifo: VecDeque::with_capacity(8192),
            h: [0.0; 2],
            piece: Vec::with_capacity(MAX_PIECE),
            start: 0.0,
            peak_snr: 0.0,
            active: false,
            n: 0,
            serial: 0,
        }
    }
}

impl MtSegment {
    fn finish(
        &mut self,
        trim: bool,
        txs: &mut Vec<Timed<Transmission>>,
        bursts: &mut Vec<Timed<Burst>>,
    ) {
        let mut len = self.piece.len();
        if trim {
            len = len.saturating_sub((0.31 * 48000.0) as usize);
        }
        if len >= (0.2 * 48000.0) as usize {
            let (mut tx, burst) = measure(&self.piece[..len], self.start, self.peak_snr);
            if let Some(mut b) = burst {
                self.serial += 1;
                b.id = self.serial;
                tx.burst = b.id;
                bursts.push(Timed {
                    time: self.start,
                    value: b,
                });
            }
            txs.push(Timed {
                time: self.start,
                value: tx,
            });
        }
        self.piece.clear();
        self.peak_snr = 0.0;
    }
}

impl Component for MtSegment {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mt.segment",
            vec![Port::real("in"), Port::typed::<CarrierState>("carrier")],
            vec![
                typed_port::<Transmission>("tx"),
                typed_port::<Burst>("bursts"),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let s = self.serial;
        *self = MtSegment::default();
        self.serial = s;
        Ok(vec![
            typed_format::<Transmission>(&inputs[0]),
            typed_format::<Burst>(&inputs[0]),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for &v in io.real(0) {
            let y = (v + self.h[0] + self.h[1]) / 3.0;
            self.h[1] = self.h[0];
            self.h[0] = v;
            self.fifo.push_back(y);
        }
        let mut txs = std::mem::take(io.items_out::<Transmission>(0));
        let mut bursts = std::mem::take(io.items_out::<Burst>(1));
        for c in io.items::<CarrierState>(1) {
            if self.fifo.len() < 480 {
                break;
            }
            let present = c.value.present;
            if present {
                if !self.active {
                    self.active = true;
                    self.start = self.n as f64 / 48000.0;
                }
                self.peak_snr = self.peak_snr.max(c.value.snr_db as f64);
                for _ in 0..480 {
                    let v = self.fifo.pop_front().unwrap_or(0.0);
                    self.piece.push(v);
                }
                if self.piece.len() >= MAX_PIECE {
                    self.finish(false, &mut txs, &mut bursts);
                    self.start = (self.n + 480) as f64 / 48000.0;
                }
            } else {
                self.fifo.drain(..480);
                if self.active {
                    self.active = false;
                    self.finish(true, &mut txs, &mut bursts);
                }
            }
            self.n += 480;
        }
        *io.items_out::<Transmission>(0) = txs;
        *io.items_out::<Burst>(1) = bursts;
    }
}

/// A cluster of the tones heard.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cluster {
    /// Its id: the row key of its row.
    pub id: u64,
    /// Mean frequency, Hz.
    pub hz: f64,
    /// How often.
    pub count: u32,
    /// Seconds in all.
    pub seconds: f64,
    /// Last time, s.
    pub last: f64,
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

sdrkit_core::commands! {
    /// The commands of `mt.tables`.
    pub enum MtCommand {
        /// Empties the lists.
        Clear = CLEAR,
    }
}

/// The tones of a transmission for people: "{Hz} Hz {s} s" for each.
fn tones_text(t: &Transmission) -> String {
    let mut s = String::new();
    for (i, tone) in t.tones.iter().enumerate() {
        let sep = if i == 0 { "" } else { "; " };
        // Writing into a `String` cannot fail.
        let _ = write!(s, "{sep}{:.1} Hz {:.2} s", tone.hz, tone.seconds);
    }
    s
}

/// `mt.tables`: the transmissions, the bursts, the tone clusters, the fields, the events and the state.
pub struct MtTables {
    p: Published,
    txs: VecDeque<Transmission>,
    bursts: VecDeque<Burst>,
    clusters: Vec<Cluster>,
    clusters_dirty: bool,
    carrier: bool,
    last_carrier: f64,
    now: f64,
    serial: u64,
    cluster_serial: u64,
    valid_words: u64,
    other_words: u64,
    level_dbfs: f32,
    freq_hz: f64,
    events: Vec<Event>,
    key: String,
    summary: String,
}

impl Default for MtTables {
    fn default() -> Self {
        MtTables {
            p: Published::from_decl(std::sync::Arc::new(declaration())),
            txs: VecDeque::new(),
            bursts: VecDeque::new(),
            clusters: Vec::new(),
            clusters_dirty: false,
            carrier: false,
            last_carrier: -10.0,
            now: 0.0,
            serial: 0,
            cluster_serial: 0,
            valid_words: 0,
            other_words: 0,
            level_dbfs: -200.0,
            freq_hz: 0.0,
            events: Vec::new(),
            key: String::new(),
            summary: String::new(),
        }
    }
}

impl MtTables {
    fn take_tx(&mut self, t: &Transmission) {
        for tone in &t.tones {
            match self
                .clusters
                .iter_mut()
                .find(|c| (c.hz - tone.hz).abs() <= 6.0)
            {
                Some(c) => {
                    c.hz = (c.hz * f64::from(c.count) + tone.hz) / f64::from(c.count + 1);
                    c.count += 1;
                    c.seconds += tone.seconds;
                    c.last = t.time;
                }
                None => {
                    if self.clusters.len() >= MULTITONE_HEARD.cap {
                        if let Some(i) =
                            (0..self.clusters.len()).min_by_key(|&i| self.clusters[i].count)
                        {
                            let gone = self.clusters.remove(i);
                            self.p.remove_row(&MULTITONE_HEARD, &gone.id.to_string());
                        }
                    }
                    self.cluster_serial += 1;
                    self.clusters.push(Cluster {
                        id: self.cluster_serial,
                        hz: tone.hz,
                        count: 1,
                        seconds: tone.seconds,
                        last: t.time,
                    });
                }
            }
            self.clusters_dirty = true;
        }
        self.clusters.sort_by_key(|c| std::cmp::Reverse(c.count));
        let text = tones_text(t);
        if t.burst == 0 {
            self.p.results_mut().log(format!(
                "Multitone: {}, {:.1} s: {text}",
                t.kind.label(),
                t.duration
            ));
        }
        self.events.push(MULTITONE_TRANSMISSION.event(
            t.time,
            self.freq_hz,
            &MultitoneTransmissionEvent {
                kind: t.kind,
                duration: Seconds(t.duration),
                snr_db: t.snr_db,
                deviation_hz: Hertz(t.deviation_hz),
                text: &text,
                family: keys::Family::Pager,
            },
        ));
        self.serial += 1;
        let tx = format!("{:08}", self.serial);
        MULTITONE_TRANSMISSIONS.put(
            &mut self.p,
            &tx,
            &MultitoneTransmissionRow {
                time_s: t.time,
                duration: Seconds(t.duration),
                snr_db: t.snr_db,
                deviation_hz: Hertz(t.deviation_hz),
                kind: t.kind,
                period_ms: (t.period_ms > 0.0).then_some(t.period_ms),
                burst: (t.burst > 0).then_some(t.burst as i64),
            },
        );
        for (i, tone) in t.tones.iter().enumerate() {
            MULTITONE_TONES.put(
                &mut self.p,
                &format!("{tx}/{i}"),
                &MultitoneToneRow {
                    transmission: &tx,
                    hz: Hertz(tone.hz),
                    seconds: Seconds(tone.seconds),
                    start: Seconds(tone.start),
                },
            );
        }
        for (i, line) in t.lines.iter().enumerate() {
            MULTITONE_LINES.put(
                &mut self.p,
                &format!("{tx}/{i}"),
                &MultitoneLineRow {
                    transmission: &tx,
                    hz: Hertz(line.hz),
                    db: line.db,
                },
            );
        }
        if self.txs.len() >= MULTITONE_TRANSMISSIONS.cap {
            self.txs.pop_front();
        }
        self.txs.push_back(t.clone());
    }
    fn take_burst(&mut self, b: &Burst, t: f64) {
        if b.bch_run >= 2 {
            self.valid_words += b.bch_valid as u64;
            self.other_words += (b.bch_total - b.bch_valid) as u64;
        } else {
            self.other_words += b.bch_total as u64;
        }
        self.p.results_mut().log(format!(
            "Multitone: {} data {:.0} baud, {} bits, {} valid BCH words (best run {}), 0x605A x{}{}",
            b.kind.label(),
            b.baud,
            b.bits,
            b.bch_valid,
            b.bch_run,
            b.w605a,
            if b.pocsag_sync.is_some() { ", POCSAG sync" } else { "" }
        ));
        self.events.push(MULTITONE_BURST.event(
            t,
            self.freq_hz,
            &MultitoneBurstEvent {
                kind: b.kind,
                baud: b.baud,
                bits: b.bits as i64,
                bch_valid: b.bch_valid as i64,
                text: &b.verdict,
                family: keys::Family::Pager,
            },
        ));
        MULTITONE_BURSTS.put(
            &mut self.p,
            &format!("{:08}", b.id),
            &MultitoneBurstRow {
                burst: b.id as i64,
                kind: b.kind,
                baud: b.baud,
                bits: b.bits as i64,
                bch_valid: b.bch_valid as i64,
                bch_total: b.bch_total as i64,
                verdict: &b.verdict,
            },
        );
        if self.bursts.len() >= MULTITONE_BURSTS.cap {
            self.bursts.pop_front();
        }
        self.bursts.push_back(b.clone());
    }
    fn publish(&mut self) {
        let on = self.carrier || self.now - self.last_carrier < 2.0;
        let lock_state = if on {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let level = Some(self.level_dbfs);
        let r = self.p.results_mut();
        match self.txs.back() {
            None => set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("Multitone: listening, level {:.1} dBFS", self.level_dbfs),
            ),
            Some(t) => set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!(
                    "Multitone: {} transmissions, last: {}{}",
                    self.txs.len(),
                    t.kind.label(),
                    if self.bursts.is_empty() {
                        ""
                    } else {
                        ", FSK data seen"
                    }
                ),
            ),
        }
        let p = &mut self.p;
        p.put(&MultitoneFields {
            carrier: self.carrier,
            transmissions: self.txs.len() as i64,
            bursts: self.bursts.len() as i64,
            valid_words: self.valid_words as i64,
            other_words: self.other_words as i64,
            level_dbfs: f64::from(self.level_dbfs),
        });
        if std::mem::take(&mut self.clusters_dirty) {
            for c in &self.clusters {
                self.key.clear();
                // Writing into a `String` cannot fail.
                let _ = write!(self.key, "{}", c.id);
                MULTITONE_HEARD.put(
                    p,
                    &self.key,
                    &MultitoneHeardRow {
                        hz: Hertz(c.hz),
                        count: i64::from(c.count),
                        seconds: Seconds(c.seconds),
                        last_s: c.last,
                    },
                );
            }
        }
    }
    /// The transmissions, oldest first.
    pub fn transmissions(&self) -> impl Iterator<Item = &Transmission> {
        self.txs.iter()
    }
    /// The bursts, oldest first.
    pub fn bursts(&self) -> impl Iterator<Item = &Burst> {
        self.bursts.iter()
    }
    /// The tone clusters, the most frequent first.
    pub fn clusters(&self) -> &[Cluster] {
        &self.clusters
    }
}

impl Component for MtTables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mt.tables",
            vec![
                Port::typed::<Transmission>("tx"),
                Port::typed::<Burst>("bursts"),
                Port::typed::<CarrierState>("carrier"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A retune forgets the signal and keeps the lists.
        self.carrier = false;
        self.now = 0.0;
        self.last_carrier = -10.0;
        self.freq_hz = inputs.first().map_or(0.0, |f| f.tuned_hz);
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for b in io.items::<Burst>(1) {
            self.take_burst(&b.value, b.time);
        }
        for t in io.items::<Transmission>(0) {
            self.take_tx(&t.value);
        }
        for c in io.items::<CarrierState>(2) {
            self.now = c.time;
            self.carrier = c.value.present;
            if c.value.present {
                self.last_carrier = c.time;
            }
            self.level_dbfs = c.value.input_dbfs;
        }
        io.messages_out(0).append(&mut self.events);
        if io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn commands(&self) -> &Commands {
        MtCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match MtCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MtCommand::Clear => {
                self.txs.clear();
                self.bursts.clear();
                self.clusters.clear();
                for t in [
                    MULTITONE_TRANSMISSIONS.def(),
                    MULTITONE_TONES.def(),
                    MULTITONE_LINES.def(),
                    MULTITONE_HEARD.def(),
                    MULTITONE_BURSTS.def(),
                ] {
                    self.p.clear_rows(t);
                }
            }
        }
        Ok(None)
    }
}

/// The receiver graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", PagerFront::new(false))
        .add("disc", PagerDisc::default())
        .add("segment", MtSegment::default())
        .add("tables", MtTables::default())
        .input("front")
        .connect("front.out", "disc")
        .connect("disc", "segment.in")
        .connect("front.carrier", "segment.carrier")
        .connect("segment.tx", "tables.tx")
        .connect("segment.bursts", "tables.bursts")
        .connect("front.carrier", "tables.carrier")
        .output("tables")
        .clock(0.25)
        .status_from("tables")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(250e3, "the Multitone detector needs at least 250 ksps")
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// The stack id.
pub const ID: &str = "multitone";

/// The settings of the Multitone stack: none.
pub fn schema() -> std::sync::Arc<sdrkit_core::Schema> {
    std::sync::Arc::default()
}

/// What the test signal holds. The data formats are made up for the test, not the format of Multitone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// The 10 s cycle: noise, tones, noise, an FSK burst, noise.
    Cycle,
    /// Noise only.
    Noise,
    /// The tone sequence only.
    Tones,
    /// The FSK burst only.
    Data,
    /// A standard POCSAG batch.
    Pocsag,
    /// Audio FFSK as measured on real recordings.
    AudioFfsk,
}

/// The test signal.
#[derive(Clone, Debug)]
pub struct MultitoneSignal {
    /// Sample rate.
    pub rate: f64,
    /// The channel's place in the capture, Hz from the centre.
    pub offset_hz: f64,
    /// Content.
    pub content: Content,
    /// FSK baud: 1200 (512 and 2400 as options).
    pub baud: f64,
    /// Inverted data polarity.
    pub invert: bool,
    /// The seed of the data words.
    pub seed: u64,
}

impl Default for MultitoneSignal {
    fn default() -> Self {
        MultitoneSignal {
            rate: 250e3,
            offset_hz: 0.0,
            content: Content::Cycle,
            baud: 1200.0,
            invert: false,
            seed: 1,
        }
    }
}

/// The 12 words of the burst (data with the BCH check).
pub fn burst_words(seed: u64) -> Vec<u32> {
    let mut r = Rng::new(seed ^ 0xB5A3);
    (0..12)
        .map(|_| word::encode((r.next_u64() & 0x1F_FFFF) as u32))
        .collect()
}

impl MultitoneSignal {
    /// The deviation (Hz) at 48 kHz and whether the carrier is on.
    pub fn track(&self) -> (Vec<f32>, Vec<bool>) {
        let fs = 48000.0;
        let mut dev: Vec<f32> = Vec::new();
        let mut on: Vec<bool> = Vec::new();
        let gap = |dev: &mut Vec<f32>, on: &mut Vec<bool>, s: f64| {
            let n = (s * fs) as usize;
            dev.extend(std::iter::repeat_n(0.0, n));
            on.extend(std::iter::repeat_n(false, n));
        };
        let tones = |dev: &mut Vec<f32>, on: &mut Vec<bool>| {
            for (f, len, pause) in [(238.3, 0.5, 0.2), (474.6, 0.5, 0.2), (1000.0, 0.4, 0.0)] {
                let n = (len * fs) as usize;
                for i in 0..n {
                    dev.push((2500.0 * (TAU * f * i as f64 / fs).sin()) as f32);
                    on.push(true);
                }
                let p = (pause * fs) as usize;
                dev.extend(std::iter::repeat_n(0.0, p));
                on.extend(std::iter::repeat_n(true, p));
            }
        };
        let fsk = |dev: &mut Vec<f32>,
                   on: &mut Vec<bool>,
                   bits: &[bool],
                   baud: f64,
                   peak: f64,
                   total_s: f64| {
            let spb = fs / baud;
            let k = 1.0 - (-TAU * 1.6 * baud / fs).exp();
            let (mut y1, mut y2) = (0.0f64, 0.0f64);
            let n = ((total_s * fs) as usize).max((bits.len() as f64 * spb) as usize);
            for i in 0..n {
                let bi = (i as f64 / spb) as usize;
                let b = if bi < bits.len() {
                    bits[bi]
                } else {
                    bi.is_multiple_of(2)
                };
                let x = if b != self.invert { peak } else { -peak };
                y1 += k * (x - y1);
                y2 += k * (y1 - y2);
                dev.push(y2 as f32);
                on.push(true);
            }
        };
        let burst_bits = || {
            let mut b: Vec<bool> = (0..32).map(|i| i % 2 == 0).collect();
            let mut push = |w: u32| b.extend((0..32).rev().map(|i| (w >> i) & 1 == 1));
            push(0xB5A3_C961);
            for w in burst_words(self.seed) {
                push(w);
            }
            b
        };
        match self.content {
            Content::Cycle => {
                gap(&mut dev, &mut on, 1.0);
                tones(&mut dev, &mut on);
                gap(&mut dev, &mut on, 1.0);
                fsk(&mut dev, &mut on, &burst_bits(), self.baud, 4000.0, 1.0);
                let rest = 10.0 - dev.len() as f64 / fs;
                gap(&mut dev, &mut on, rest.max(0.5));
            }
            Content::Noise => gap(&mut dev, &mut on, 4.0),
            Content::Tones => {
                gap(&mut dev, &mut on, 0.5);
                tones(&mut dev, &mut on);
                gap(&mut dev, &mut on, 1.0);
            }
            Content::Data => {
                gap(&mut dev, &mut on, 0.5);
                fsk(&mut dev, &mut on, &burst_bits(), self.baud, 4000.0, 1.0);
                gap(&mut dev, &mut on, 1.0);
            }
            Content::Pocsag => {
                gap(&mut dev, &mut on, 0.5);
                let page = [sdrkit_proto_pager::pocsag::PocsagTx {
                    ric: 1234567,
                    function: 3,
                    content: sdrkit_proto_pager::pocsag::Content::Alpha("Test page"),
                }];
                let mut words = Vec::new();
                sdrkit_proto_pager::pocsag::encode_batches(
                    &page,
                    sdrkit_proto_pager::pocsag::NumericSet::Itu,
                    &mut words,
                )
                .expect("a valid test page");
                let mut tx = Vec::new();
                sdrkit_proto_pager::pocsag::transmission_bits(
                    &words,
                    sdrkit_proto_pager::pocsag::PREAMBLE_BITS,
                    &mut tx,
                );
                let bits: Vec<bool> = tx.iter().map(|&b| b == 0).collect();
                fsk(&mut dev, &mut on, &bits, 1200.0, 4500.0, 0.0);
                gap(&mut dev, &mut on, 1.0);
            }
            Content::AudioFfsk => {
                gap(&mut dev, &mut on, 0.5);
                let (lo, hi) = (292.65, 585.3);
                let mut bits: Vec<bool> = vec![true; 24];
                for w in [0x605Au16, 0x605A, 0x7B91, 0x310E, 0xC47F, 0xFFFF] {
                    bits.extend((0..16).rev().map(|i| (w >> i) & 1 == 1));
                }
                bits.extend(std::iter::repeat_n(true, 16));
                let mut ph = 0.0f64;
                for &b in &bits {
                    let (f, cycles) = if b { (hi, 1.0) } else { (lo, 0.5) };
                    let n = (cycles / f * fs).round() as usize;
                    for _ in 0..n {
                        dev.push((3000.0 * (TAU * ph).sin()) as f32);
                        on.push(true);
                        ph = (ph + f / fs).fract();
                    }
                }
                gap(&mut dev, &mut on, 1.0);
            }
        }
        (dev, on)
    }
    /// The streaming generator.
    pub fn generator(&self) -> MultitoneGenerator {
        let (dev, on) = self.track();
        MultitoneGenerator {
            rate: self.rate,
            tx: AudioFm::new(48000.0, self.rate, 1.0, self.offset_hz).ok(),
            offset_hz: self.offset_hz,
            audio: Looping::new(dev),
            on,
            n: 0,
            looping: true,
        }
    }
}

/// The streaming generator. It sends unit power while the carrier is on.
pub struct MultitoneGenerator {
    rate: f64,
    tx: Option<AudioFm>,
    /// Where the station is made, Hz from the centre of the samples.
    offset_hz: f64,
    audio: Looping,
    on: Vec<bool>,
    n: u64,
    looping: bool,
}

impl MultitoneGenerator {
    /// From the stack's generator config.
    pub fn from_config(cfg: &GenConfig) -> MultitoneGenerator {
        MultitoneSignal {
            rate: cfg.rate_hz,
            seed: cfg.seed,
            ..Default::default()
        }
        .generator()
    }
    /// Stops after one pass.
    pub fn once(mut self) -> Self {
        self.looping = false;
        self
    }
}

impl Generator for MultitoneGenerator {
    /// The station is made at the offset of the signal, 0 Hz for the test signal of the stack. The framework places it on
    /// the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        self.offset_hz
    }
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let len = self.on.len() as u64;
        let Some(tx) = self.tx.as_mut() else { return 0 };
        if len == 0 {
            return 0;
        }
        let total = (len as f64 * self.rate / 48000.0) as u64;
        let n = if self.looping {
            out.len()
        } else {
            out.len().min(total.saturating_sub(self.n) as usize)
        };
        let audio = &mut self.audio;
        tx.fill(&mut out[..n], |a| audio.fill(a));
        for z in out[..n].iter_mut() {
            let k = ((self.n as f64 * 48000.0 / self.rate) as u64 % len) as usize;
            if !self.on[k] {
                *z = Complex32::new(0.0, 0.0);
            }
            self.n += 1;
        }
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(16e3))
    }
}
