//! The OpenSky identifier (`opensky`): it measures an FM channel block by block (symbol rate, levels, deviation, occupied width,
//! burst period) and says whether the signal fits the description of OpenSky: 4-level FSK at 9600 baud in a 25 kHz channel,
//! in TDMA bursts. Nothing is decoded: the air interface is not published and its traffic is encrypted.

pub mod gen;

use crate::front::NfmFront;
use crate::typed::*;
use crate::typed_format;
use num_complex::Complex32;
use sdrkit_algo_modem::fmmeasure::FmMeasure;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, EventDef, GenConfig, Generator, LockState, Receiver, Record, ResultsDecl, Schema,
    StackConfig, StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Outbox,
    Port, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self as objpub, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "opensky";
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 96e3;
/// The working rate, Hz.
pub const WORK_RATE: f64 = 96_000.0;
/// The channel half-width, Hz.
pub const HALF_WIDTH_HZ: f64 = 12_500.0;
/// The samples of a block.
pub const BLOCK: usize = 1 << 17;
/// The symbol rate of the description, Hz.
pub const OPENSKY_BAUD: f64 = 9600.0;
/// How far the measured rate may lie from it, Hz.
pub const RATE_TOLERANCE_HZ: f64 = 300.0;
/// The least strength of the symbol rate line, dB.
pub const MIN_LINE_DB: f64 = 8.0;
/// The most rows kept.
pub const ROW_CAP: usize = 100;
/// A block with the carrier on for less than this share of its samples holds no signal (noise alone passes the on threshold
/// at its peaks only).
pub const MIN_DUTY: f64 = 0.1;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the description of the signal (4-slot TDMA, 9600 baud 4-level FSK in a 25 kHz channel, on 700, 800 and 900 MHz); no OpenSky signal was analysed. The verdict is a fit to that description, not a proof.";

sdrkit_core::choice_enum! {
    /// What a block says.
    #[derive(Serialize, Deserialize)]
    pub enum Verdict {
        /// The block fits the description of OpenSky.
        Consistent = ("consistent", "Consistent with OpenSky"),
        /// A symbol rate was found and the signal is something else.
        Other = ("other", "Not the OpenSky pattern"),
    }
}

/// The measurements of one block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measures {
    /// Signal time of the end of the block, s.
    pub t: f64,
    /// The symbol rate, Hz.
    pub rate_hz: f64,
    /// The strength of its line, dB.
    pub line_db: f64,
    /// 2 or 4 levels, 0 unclear.
    pub levels: u8,
    /// The deviation, Hz.
    pub deviation_hz: f64,
    /// The burst period, ms (0: continuous or none found).
    pub burst_ms: f64,
    /// The share of samples with the carrier on.
    pub duty: f64,
    /// The occupied width, Hz.
    pub width_hz: f64,
    /// The mean power, dBFS.
    pub level_dbfs: f64,
    /// The verdict.
    pub verdict: Verdict,
}

impl Measures {
    /// The verdict for people.
    pub fn text(&self) -> String {
        match self.verdict {
            Verdict::Consistent => "consistent with OpenSky".to_string(),
            Verdict::Other => format!(
                "measured {:.0} baud, {} levels, {:.1} kHz wide: not the OpenSky pattern",
                self.rate_hz,
                self.levels,
                self.width_hz / 1e3
            ),
        }
    }
}

impl Shift for Measures {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Held {
    /// The blocks with a result, newest last.
    pub rows: LogRing<Measures>,
    /// Blocks analysed.
    pub blocks: u64,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            rows: LogRing::new(ROW_CAP),
            blocks: 0,
        }
    }
}

impl Shift for Held {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.shift_times(by_s);
    }
}

impl Tables for Held {
    const FORMAT: StateFormat = StateFormat::new("trunk.opensky", 1);
    fn clear(&mut self) {
        self.rows.clear();
        self.blocks = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.rows.adopt(saved.rows);
        self.blocks = saved.blocks;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for Measures {
    type Row = OpenskyBlockRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&OpenskyBlockRow {
            time_s: self.t,
            symbol_rate_hz: Hertz(self.rate_hz),
            levels: i64::from(self.levels),
            width_hz: self.width_hz,
            burst_ms: self.burst_ms,
            verdict: self.verdict,
            text: &self.text(),
        });
    }
}

/// A verdict as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct VerdictItem(pub Measures);

impl Item for VerdictItem {
    const KIND: &'static str = "opensky.verdict";
    fn event_def() -> Option<&'static EventDef> {
        Some(OPENSKY_VERDICT.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let m = &self.0;
        OpenskyVerdictEvent {
            verdict: m.verdict,
            symbol_rate_hz: Hertz(m.rate_hz),
            levels: i64::from(m.levels),
            deviation_hz: m.deviation_hz,
            burst_ms: m.burst_ms,
            width_hz: m.width_hz,
            text: &m.text(),
            family: Family::Other,
        }
        .write(f);
    }
}

/// The `opensky.analyse` decoder.
pub struct Analyser {
    front: NfmFront,
    measure: FmMeasure,
    disc: Vec<f32>,
    env: Vec<f32>,
    iq: Vec<Complex32>,
    on: Vec<bool>,
    n: u64,
    held: Held,
    last: Option<Measures>,
    out: Vec<Timed<VerdictItem>>,
    started: bool,
    dirty: bool,
    cleared: bool,
    summary: String,
}

impl Default for Analyser {
    fn default() -> Self {
        let mut front = NfmFront::new(WORK_RATE, HALF_WIDTH_HZ);
        front.keep_channel(true);
        Analyser {
            front,
            measure: FmMeasure::new(),
            disc: Vec::with_capacity(BLOCK + 8192),
            env: Vec::with_capacity(BLOCK + 8192),
            iq: Vec::with_capacity(BLOCK + 8192),
            on: Vec::with_capacity(BLOCK + 8192),
            n: 0,
            held: Held::default(),
            last: None,
            out: Vec::new(),
            started: false,
            dirty: true,
            cleared: false,
            summary: String::new(),
        }
    }
}

impl Analyser {
    /// Analyses one block of [`BLOCK`] samples.
    fn block(&mut self) {
        let n = BLOCK;
        let t = self.n as f64 / WORK_RATE;
        self.held.blocks += 1;
        let peak = self.env[..n].iter().copied().fold(0.0f32, f32::max);
        self.on.clear();
        self.on
            .extend(self.env[..n].iter().map(|&e| peak > 0.0 && e > 0.25 * peak));
        let on_count = self.on.iter().filter(|&&o| o).count();
        let duty = on_count as f64 / n as f64;
        let mean = self.env[..n].iter().map(|&e| f64::from(e)).sum::<f64>() / n as f64;
        let level_dbfs = if mean > 0.0 {
            10.0 * mean.log10()
        } else {
            -200.0
        };
        self.dirty = true;
        if duty < MIN_DUTY {
            return;
        }
        let rate = self
            .measure
            .symbol_rate(&self.disc[..n], &self.on, WORK_RATE, 2000.0, 25_000.0)
            .filter(|r| r.strength_db > MIN_LINE_DB);
        let Some(rate) = rate else {
            return;
        };
        let lv = self.measure.levels(&self.disc[..n], &self.on);
        let width_hz = self
            .measure
            .occupied_width(&self.iq[..n], WORK_RATE)
            .unwrap_or(0.0);
        let burst_ms = if duty < 0.97 {
            self.measure
                .burst_period(&self.env[..n], WORK_RATE)
                .map_or(0.0, |s| s * 1e3)
        } else {
            0.0
        };
        let consistent = (rate.hz - OPENSKY_BAUD).abs() <= RATE_TOLERANCE_HZ && lv.count == 4;
        let m = Measures {
            t,
            rate_hz: rate.hz,
            line_db: rate.strength_db,
            levels: lv.count,
            deviation_hz: lv.deviation,
            burst_ms,
            duty,
            width_hz,
            level_dbfs,
            verdict: if consistent {
                Verdict::Consistent
            } else {
                Verdict::Other
            },
        };
        self.out.push(Timed {
            time: t,
            value: VerdictItem(m.clone()),
        });
        self.held.rows.push(m.clone());
        self.last = Some(m);
    }
}

impl Decoder for Analyser {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "opensky.analyse"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<VerdictItem>("verdicts")]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        Published::new(OpenskyFields::SHAPE.fields).with_tables(&[OPENSKY_BLOCKS.def()])
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.front.setup(f.rate, f.signal_offset_hz())?;
        self.disc.clear();
        self.env.clear();
        self.iq.clear();
        self.n = 0;
        Ok(vec![typed_format::<VerdictItem>(f)])
    }
    fn retune(&mut self, f: &Format) {
        let _ = self.front.setup(f.rate, f.signal_offset_hz());
        self.disc.clear();
        self.env.clear();
        self.iq.clear();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        let before = self.disc.len();
        self.front.run(x, &mut self.disc);
        let made = self.disc.len() - before;
        let ch = self.front.channel();
        self.iq
            .extend_from_slice(&ch[ch.len() - made.min(ch.len())..]);
        self.env
            .extend(self.iq[before..].iter().map(|z| z.norm_sqr()));
        self.n += made as u64;
        while self.disc.len() >= BLOCK {
            self.block();
            self.disc.drain(..BLOCK);
            self.env.drain(..BLOCK);
            self.iq.drain(..BLOCK);
        }
        out.items::<VerdictItem>(0).append(&mut self.out);
    }
    fn report(&mut self, p: &mut Published) {
        let state = if !self.started {
            0
        } else if self.last.is_some() {
            2
        } else {
            1
        };
        let m = self.last.as_ref();
        let text = m.map(Measures::text);
        p.put(&OpenskyFields {
            rx_state: state,
            blocks: self.held.blocks as i64,
            symbol_rate_hz: m.map(|m| Hertz(m.rate_hz)),
            line_db: m.map(|m| m.line_db),
            levels: m.map(|m| i64::from(m.levels)),
            deviation_hz: m.map(|m| m.deviation_hz),
            burst_ms: m.map(|m| m.burst_ms),
            duty: m.map(|m| m.duty),
            width_hz: m.map(|m| m.width_hz),
            level_dbfs: m.map(|m| m.level_dbfs),
            verdict: m.map(|m| m.verdict),
            text: text.as_deref(),
            note: NOT_CONFIRMED,
        });
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        if std::mem::take(&mut self.dirty) {
            objpub::log_rows(
                &self.held.rows,
                &OPENSKY_BLOCKS,
                self.n as f64 / WORK_RATE,
                p,
            );
        }
        let lock_state = match (state, m.map(|m| m.verdict)) {
            (0, _) => LockState::Off,
            (_, Some(Verdict::Consistent)) => LockState::Locked,
            _ => LockState::Searching,
        };
        let level = m.map(|m| m.level_dbfs as f32);
        let r = p.results_mut();
        match m {
            None if state == 0 => crate::set_status(r, lock_state, level, &mut self.summary, format_args!("OpenSky: waiting for samples")),
            None => crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("OpenSky: analysing, no symbol rate found"),
            ),
            Some(m) if m.verdict == Verdict::Consistent => crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!(
                    "OpenSky: consistent ({:.0} baud, {} levels, burst {:.1} ms)",
                    m.rate_hz, m.levels, m.burst_ms
                ),
            ),
            Some(m) => crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!(
                    "OpenSky: measured {:.0} baud, {} levels, {:.1} kHz wide: not the OpenSky pattern",
                    m.rate_hz,
                    m.levels,
                    m.width_hz / 1e3
                ),
            ),
        }
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        self.held.clear();
        self.last = None;
        self.cleared = true;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.n as f64 / WORK_RATE).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.n as f64 / WORK_RATE)?;
        self.last = self.held.rows.newest_first().next().cloned();
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The settings of the `opensky` stack: the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(gen::SETTINGS)))
        .clone()
}

/// The receiver: the analyser with the `clear` command.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("analyse", DecoderComponent::new(Analyser::default()))
        .input("analyse")
        .output("analyse")
        .status_from("analyse")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE, "The OpenSky identifier needs at least 96 ksps")
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`receiver`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// Makes the looping test signal, the channel where the receiver looks.
///
/// # Errors
///
/// [`ConfigError`] if the rate cannot hold the channel.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::OpenSkyGenerator::from_config(
        cfg,
        crate::OPENSKY.tuning.tune_offset_hz,
    )?))
}
