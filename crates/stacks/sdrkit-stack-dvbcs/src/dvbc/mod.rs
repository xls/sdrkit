//! DVB-C (`dvbc`): EN 300 429 and ITU-T J.83 annexes A and C, 16- to 256-QAM, with the transport stream layer of
//! `sdrkit-proto-mpegts` after the outer decoder.
//!
//! Standard: ETSI EN 300 429.

pub mod demod;
pub mod gen;
pub mod qam;

use crate::typed::DvbcFields;
use crate::TS_LAYER;
use sdrkit_proto_dvbs2::receive::{item_format, write_status, Lost};

use demod::{CoreReport, DvbcConfig, DvbcCore, Stage, MAIN, TAPS};
use qam::Qam;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::results::{LockState, SeriesId};
use sdrkit_core::SignalType;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Receiver, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, StackConfig, Value, Values,
};
use sdrkit_graph::components::{SpectrumSize, SpectrumWindow};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Kind, PacketPool, Port, RateFrom, SlicePool, SpectrumSpec, StackGraph, Timed,
};
use sdrkit_proto_dvbs2::transmit::{GEN_PAYLOAD, TEST_SIGNAL};
use sdrkit_proto_mpegts::layer::{self, DECODE_VIDEO};
use sdrkit_proto_mpegts::monitor::TsMonitorComponent;
use sdrkit_proto_mpegts::outer::{sync_search, Dispersal, OuterDecoder, OuterStats, RS_N as RSW};
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};
use std::fmt::Write as _;
use std::sync::Arc;

/// A run of bytes between a demodulator and its outer decoder, with the signal time of its first byte.
#[derive(Clone, Debug)]
pub struct ByteChunk {
    /// The bytes, from the demodulator's packet pool.
    pub bytes: sdrkit_graph::Packet,
    /// The first byte starts a new alignment (byte 0 is a sync byte position).
    pub restart: bool,
    /// Signal time the chunk spans, seconds.
    pub span_s: f64,
}

impl sdrkit_graph::Item for ByteChunk {
    const KIND: &'static str = "dvb.bytes";
}

/// The stack id.
pub const ID: &str = "dvbc";

/// The lowest input rate.
pub const MIN_RATE: f64 = 7e6;

/// The period of the results, seconds of signal. It is short so that a recording of a fraction of a second has its results.
pub const REPORT_S: f64 = 0.05;

/// Input samples per graph pass.
pub const PASS: usize = 8192;

/// The cable raster: channel centres 114 + 8 k MHz (k = 0 to 93), the nearest to `hz`, limited to 114 and 858 MHz.
pub fn cable_raster(hz: f64) -> f64 {
    let k = ((hz / 1e6 - 114.0) / 8.0).round().clamp(0.0, 93.0);
    (114.0 + 8.0 * k) * 1e6
}

sdrkit_core::choice_enum! {
    /// The QAM order the receiver uses.
    pub enum QamChoice {
        /// Found by trial.
        Auto = ("auto", "Automatic"),
        /// 16-QAM.
        Q16 = ("16", "16-QAM"),
        /// 32-QAM.
        Q32 = ("32", "32-QAM"),
        /// 64-QAM.
        Q64 = ("64", "64-QAM"),
        /// 128-QAM.
        Q128 = ("128", "128-QAM"),
        /// 256-QAM.
        Q256 = ("256", "256-QAM"),
    }
}

impl QamChoice {
    /// The order, `None` for automatic.
    pub fn qam(self) -> Option<Qam> {
        match self {
            QamChoice::Auto => None,
            QamChoice::Q16 => Some(Qam::Q16),
            QamChoice::Q32 => Some(Qam::Q32),
            QamChoice::Q64 => Some(Qam::Q64),
            QamChoice::Q128 => Some(Qam::Q128),
            QamChoice::Q256 => Some(Qam::Q256),
        }
    }

    /// The choice of order `q`, automatic for `None`.
    pub fn of(q: Option<Qam>) -> QamChoice {
        match q {
            None => QamChoice::Auto,
            Some(Qam::Q16) => QamChoice::Q16,
            Some(Qam::Q32) => QamChoice::Q32,
            Some(Qam::Q64) => QamChoice::Q64,
            Some(Qam::Q128) => QamChoice::Q128,
            Some(Qam::Q256) => QamChoice::Q256,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The annex of ITU-T J.83 in use.
    pub enum Annex {
        /// Annex A, 8 MHz channels, roll-off 0.15.
        A = ("a", "A (8 MHz)"),
        /// Annex C, 6 MHz channels, roll-off 0.13.
        C = ("c", "C (6 MHz)"),
    }
}

/// The symbol rate. 0 measures it from the signal. A manual rate that does not lock is replaced by the measured one.
pub const SYMBOL_RATE: Setting<field::Hz> = Setting::new("symbol_rate", "Symbol rate", "Hz", 0.0)
    .range(0.0, 9.5e6, 0.0)
    .help("0: measured from the signal. A manual rate that does not lock gives way to the measured one.");
/// The QAM order.
pub const QAM_ORDER: Setting<field::Choice<QamChoice>> =
    Setting::choice("qam", "QAM", QamChoice::Auto);
/// ITU-T J.83 annex C.
pub const ANNEX_C: Setting<field::Bool> = Setting::new("annex_c", "Annex C", "", false)
    .help("ITU-T J.83 annex C (Japan, 6 MHz channels): roll-off 0.13 instead of 0.15.");
/// The QAM order of the test signal.
pub const GEN_QAM: Setting<field::Choice<Qam>> = Setting::choice("gen_qam", "QAM", Qam::Q64)
    .section(TEST_SIGNAL)
    .advanced();
/// Annex C for the test signal.
pub const GEN_ANNEX_C: Setting<field::Bool> = Setting::new("gen_annex_c", "Annex C", "", false)
    .section(TEST_SIGNAL)
    .advanced();
/// The symbol rate of the test signal.
pub const GEN_SYMBOL_RATE: Setting<field::Hz> =
    Setting::new("gen_symbol_rate", "Symbol rate", "Hz", 0.0)
        .range(0.0, 9.5e6, 0.0)
        .section(TEST_SIGNAL)
        .help("0: 6.952 Msym/s (annex A) or 5.274 Msym/s (annex C).")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the DVB-C stack.
    pub enum DvbcSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The QAM order.
        Qam = QAM_ORDER,
        /// Annex C.
        AnnexC = ANNEX_C,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The QAM order of the test signal.
        GenQam = GEN_QAM,
        /// Annex C for the test signal.
        GenAnnexC = GEN_ANNEX_C,
        /// The symbol rate of the test signal.
        GenSymbolRate = GEN_SYMBOL_RATE,
        /// The payload of the test signal.
        GenPayload = GEN_PAYLOAD,
    }
}

sdrkit_core::settings! {
    /// The settings of [`DvbcDemod`].
    pub enum DemodSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The QAM order.
        Qam = QAM_ORDER,
        /// Annex C.
        AnnexC = ANNEX_C,
    }
}

/// The state the demodulator sends to the report on each tick of the graph clock.
#[derive(Clone, Debug, Default)]
pub struct DvbcState {
    /// The measurements of the core.
    pub core: CoreReport,
    /// The stage of the core.
    pub stage: Option<Stage>,
    /// Annex C is in use.
    pub annex_c: bool,
    /// Recent symbols.
    pub constellation: Arc<[Complex32]>,
    /// Equaliser tap magnitudes, dB, one per tap. The main tap is [`MAIN`].
    pub taps: Arc<[f32]>,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl sdrkit_graph::Item for DvbcState {
    const KIND: &'static str = "dvbc.state";
}

/// `dvbc.demod`: iq in, aligned bytes and the state out. Input `lost` takes the [`Lost`] items of the outer decoder.
pub struct DvbcDemod {
    core: Option<DvbcCore>,
    cfg: DvbcConfig,
    span: f64,
    packets: PacketPool,
    symbols: SlicePool<Complex32>,
    taps: SlicePool<f32>,
}

impl DvbcDemod {
    /// A demodulator with these settings.
    ///
    /// # Errors
    ///
    /// [`SettingError::OutOfRange`] if the manual symbol rate is out of the range of the setting.
    pub fn new(cfg: DvbcConfig) -> Result<Self, SettingError> {
        DemodSetting::SymbolRate.check(&Value::Hz(cfg.manual_rs))?;
        Ok(DvbcDemod {
            core: None,
            cfg,
            span: 0.0,
            packets: PacketPool::default(),
            symbols: SlicePool::default(),
            taps: SlicePool::default(),
        })
    }
}

impl Default for DvbcDemod {
    fn default() -> Self {
        DvbcDemod {
            core: None,
            cfg: DvbcConfig::default(),
            span: 0.0,
            packets: PacketPool::default(),
            symbols: SlicePool::default(),
            taps: SlicePool::default(),
        }
    }
}

impl Component for DvbcDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbc.demod",
            vec![
                Port::iq("in"),
                Port::typed::<Lost>("lost").optional("the stream is never reported lost"),
            ],
            vec![
                Port::typed::<ByteChunk>("bytes").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<DvbcState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DemodSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let fs = inputs[0].rate;
        self.core = Some(DvbcCore::new(fs, self.cfg));
        self.span = 0.0;
        Ok(vec![
            item_format::<ByteChunk>(&inputs[0]),
            Format::new(Kind::typed::<DvbcState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(core) = self.core.as_mut() else {
            return;
        };
        if !io.items::<Lost>(1).is_empty() {
            core.lost_stream();
        }
        let x = io.iq(0);
        let t0 = io.input_format(0).time_of(io.input(0).first());
        let fs = io.input_format(0).rate;
        core.process(x);
        core.update_report();
        self.span += x.len() as f64 / fs;
        if !core.out_bytes.is_empty() {
            let bytes = &core.out_bytes;
            let chunk = ByteChunk {
                bytes: self
                    .packets
                    .make(bytes.len(), t0, |b| b.copy_from_slice(bytes)),
                restart: std::mem::replace(&mut core.out_restart, false),
                span_s: self.span,
            };
            core.out_bytes.clear();
            self.span = 0.0;
            io.items_out::<ByteChunk>(0).push(Timed {
                time: t0,
                value: chunk,
            });
        }
        if io.tick() {
            let cons = &core.constellation;
            let st = DvbcState {
                core: core.report,
                stage: Some(core.stage),
                annex_c: self.cfg.annex_c,
                constellation: self.symbols.exact(cons.len(), |o| o.copy_from_slice(cons)),
                taps: self.taps.exact(TAPS, |o| core.tap_db(o)),
                log: std::mem::take(&mut core.log),
            };
            io.items_out::<DvbcState>(1).push(Timed {
                time: t0,
                value: st,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DemodSetting::SymbolRate => self.cfg.manual_rs = SYMBOL_RATE.read(id, v)?,
            DemodSetting::Qam => self.cfg.qam = QAM_ORDER.read(id, v)?.qam(),
            DemodSetting::AnnexC => self.cfg.annex_c = ANNEX_C.read(id, v)?,
        }
        if let Some(c) = self.core.as_mut() {
            c.set_config(self.cfg);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DemodSetting::SymbolRate, Value::Hz(self.cfg.manual_rs)),
            (
                DemodSetting::Qam,
                Value::Choice(QamChoice::of(self.cfg.qam).index()),
            ),
            (DemodSetting::AnnexC, Value::Bool(self.cfg.annex_c)),
        ])
    }
}

/// `dvbc.outer`: the outer decoder with byte re-alignment and the lost-stream rule. Output `ts` is the transport stream,
/// `lost` reports to the demodulator that the stream is lost, and `stats` carries the counters.
pub struct DvbcOuter {
    outer: OuterDecoder,
    ts: TsOut,
    active: bool,
    shift: usize,
    carry: u8,
    collecting: Option<Vec<u8>>,
    pending: Vec<u8>,
}

impl Default for DvbcOuter {
    fn default() -> Self {
        DvbcOuter {
            outer: OuterDecoder::new(Dispersal::DvbGroups),
            ts: TsOut::default(),
            active: false,
            shift: 0,
            carry: 0,
            collecting: None,
            pending: Vec::new(),
        }
    }
}

impl DvbcOuter {
    fn decode(&mut self, bytes: &[u8], time: f64, out: &mut Vec<Timed<TsPacket>>) -> u32 {
        let (outer, ts) = (&mut self.outer, &mut self.ts);
        let mut fails = 0;
        for piece in bytes.chunks(RSW) {
            // A word that does not start with the sync byte is not a packet, and the monitor counts it as lost.
            outer.push(piece, &mut |w| {
                ts.push(out, &w.packet, time, false);
            });
            fails = fails.max(outer.run_failed());
            if outer.run_failed() >= 48 {
                break;
            }
        }
        fails
    }
}

impl Component for DvbcOuter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbc.outer",
            vec![Port::typed::<ByteChunk>("in")],
            vec![
                stream::port("ts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Lost>("lost").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<OuterStats>("stats").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = DvbcOuter::default();
        Ok(vec![
            inputs[0].with_kind(stream::kind()),
            Format::new(Kind::typed::<Lost>(), 0.0),
            Format::new(Kind::typed::<OuterStats>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut lost = false;
        let chunks = io.items::<ByteChunk>(0);
        let ts = io.items_out::<TsPacket>(0);
        for c in chunks {
            if c.value.restart {
                self.outer.restart();
                self.active = true;
                self.shift = 0;
                self.carry = 0;
                self.collecting = None;
            }
            if !self.active {
                continue;
            }
            // Bytes shifted by the current bit alignment.
            self.pending.clear();
            if self.shift == 0 {
                self.pending.extend_from_slice(c.value.bytes.bytes());
            } else {
                for &b in c.value.bytes.bytes() {
                    self.pending
                        .push((self.carry << self.shift) | (b >> (8 - self.shift)));
                    self.carry = b & ((1 << self.shift) - 1);
                }
            }
            let time = c.time;
            if let Some(mut buf) = self.collecting.take() {
                buf.extend_from_slice(&self.pending);
                if buf.len() < 24 * RSW {
                    self.collecting = Some(buf);
                    continue;
                }
                let bits: Vec<u8> = buf
                    .iter()
                    .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1))
                    .collect();
                match sync_search(&bits, &sdrkit_proto_dvbs2::sync_rule(20, false)) {
                    Some(hit) => {
                        self.outer.stats.alignments += 1;
                        self.outer.restart();
                        let start = hit.start;
                        let mut re = Vec::with_capacity(bits.len() / 8);
                        let mut i = start;
                        while i + 8 <= bits.len() {
                            re.push(bits[i..i + 8].iter().fold(0u8, |a, &b| (a << 1) | b));
                            i += 8;
                        }
                        // The new alignment of later bytes: the total bit shift against the incoming bytes.
                        self.shift = (self.shift + hit.start % 8) % 8;
                        self.carry = bits[i..].iter().fold(0u8, |a, &b| (a << 1) | b);
                        self.decode(&re, time, ts);
                    }
                    None => {
                        lost = true;
                        self.active = false;
                    }
                }
                continue;
            }
            let pending = std::mem::take(&mut self.pending);
            let fails = self.decode(&pending, time, ts);
            self.pending = pending;
            if self.outer.run_failed() >= 48 {
                lost = true;
                self.active = false;
            } else if fails >= 12 {
                self.collecting = Some(Vec::with_capacity(25 * RSW));
            }
        }
        if lost {
            io.items_out::<Lost>(1).push(Timed {
                time: 0.0,
                value: Lost,
            });
        }
        if io.tick() || lost {
            io.items_out::<OuterStats>(2).push(Timed {
                time: 0.0,
                value: self.outer.stats,
            });
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

const CONSTELLATION: SeriesId = SeriesId(0);
const SPECTRUM: SeriesId = SeriesId(1);
const EQUALISER: SeriesId = SeriesId(2);
const MER_HISTORY: SeriesId = SeriesId(3);

/// The input spectrum of the report's plot.
pub const SPECTRUM_SPEC: SpectrumSpec = SpectrumSpec {
    size: sdrkit_graph::SizeRule::Bins(SpectrumSize::S1024),
    window: SpectrumWindow::Hann,
    average: 8,
    frames: sdrkit_graph::FrameRule::PerSecond(1.0 / REPORT_S),
    scale: sdrkit_graph::components::SpectrumScale::Db,
};

/// The most reports that the MER history keeps.
const MER_HISTORY_LEN: usize = 240;

/// The plots of the stack, in order.
pub const SERIES: [SeriesDef; 4] = [
    SeriesDef {
        key: "constellation",
        label: "Constellation",
        x_unit: "",
        y_unit: "",
    },
    SeriesDef {
        key: "spectrum",
        label: "Spectrum",
        x_unit: "bin",
        y_unit: "dB",
    },
    SeriesDef {
        key: "equaliser",
        label: "Equaliser taps",
        x_unit: "us",
        y_unit: "dB",
    },
    SeriesDef {
        key: "mer_history",
        label: "MER history",
        x_unit: "report",
        y_unit: "dB",
    },
];

/// What `dvbc.report` publishes: its fields and plots.
fn report_declaration() -> ResultsDecl {
    let mut d = ResultsDecl::new().with_fields(DvbcFields::SHAPE.fields);
    for s in &SERIES {
        d = d.with_series(s);
    }
    d
}

/// What the DVB-C stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// `dvbc.report`: the results of the receiver, from the state of the demodulator, the counters of the outer decoder and the
/// spectrum of the input.
pub struct DvbcReport {
    published: Published,
    state: DvbcState,
    outer: OuterStats,
    mer_hist: Vec<f32>,
    run_start_failed: u64,
    realigned: u64,
    summary: String,
}

impl Default for DvbcReport {
    fn default() -> Self {
        DvbcReport {
            published: Published::from_decl(Arc::new(report_declaration())),
            state: DvbcState::default(),
            outer: OuterStats::default(),
            mer_hist: Vec::with_capacity(MER_HISTORY_LEN + 1),
            run_start_failed: 0,
            realigned: 0,
            summary: String::with_capacity(160),
        }
    }
}

impl DvbcReport {
    fn publish(&mut self) {
        let c = &self.state.core;
        let o = self.outer;
        let annex_c = self.state.annex_c;
        let stage = self.state.stage.unwrap_or(Stage::Spectrum);
        let locked = stage == Stage::Locked;
        if !locked {
            self.run_start_failed = o.failed;
        }
        if o.alignments > self.realigned {
            self.realigned = o.alignments;
            self.published
                .results_mut()
                .log("bytes re-aligned without losing the lock");
        }
        let failing = o.failed > self.run_start_failed;
        let ts_lock = locked && !failing && o.delivered > 0;
        let p = &mut self.published;
        for l in self.state.log.drain(..) {
            p.results_mut().log(l);
        }
        let inverted = c.inverted && locked;
        let m = c.qam.map_or(0, |q| q.bits());
        let carrier = stage != Stage::Spectrum;
        p.put(&DvbcFields {
            stage,
            note: c.note,
            qam: c.qam,
            symbol_rate_hz: (c.symbol_rate > 0.0).then_some(Hertz(c.symbol_rate)),
            measured_rate: (c.measured_rate > 0.0).then_some(Hertz(c.measured_rate)),
            manual_rate: c.manual_used,
            width: (c.width_hz > 0.0).then_some(Hertz(c.width_hz)),
            annex: if annex_c { Annex::C } else { Annex::A },
            roll_off: if annex_c { 0.13 } else { 0.15 },
            inverted,
            clock_ppm: c.clock_ppm,
            offset_hz: carrier.then_some(Hertz(c.cfo_hz)),
            mer_db: c.mer_db,
            level_dbfs: c.level_dbfs,
            pre_rs: o.pre_rs,
            rs_clean: o.clean as i64,
            rs_corrected: o.corrected as i64,
            rs_failed: o.failed as i64,
            realigned: o.alignments as i64,
            net_bitrate_bps: locked.then_some(c.symbol_rate * m as f64 * 188.0 / 204.0),
            spectrum_lock: c.stages[0],
            timing_lock: c.stages[1],
            equaliser_lock: c.stages[2],
            carrier_lock: c.stages[3],
            sync_lock: c.stages[4],
            ts_lock,
            searches: c.searches as i64,
            frames_ok: (o.clean + o.corrected) as i64,
            frames_bad: o.failed as i64,
        });
        if let (Some(mer), true) = (
            c.mer_db,
            matches!(stage, Stage::Trial | Stage::Locked | Stage::Dd),
        ) {
            self.mer_hist.push(mer as f32);
            if self.mer_hist.len() > MER_HISTORY_LEN {
                self.mer_hist.remove(0);
            }
        }
        let lock_state = if ts_lock {
            LockState::Locked
        } else if stage == Stage::Spectrum {
            LockState::Searching
        } else {
            LockState::Tracking
        };
        let s = &mut self.summary;
        s.clear();
        let _ = write!(
            s,
            "DVB-C{} {}  {:.4} Msym/s  MER {:.1} dB  offset {:+.0} Hz  RS ok {} bad {}{}{}",
            if annex_c { " (annex C)" } else { "" },
            c.qam.map_or("QAM not known", Qam::name),
            c.symbol_rate / 1e6,
            c.mer_db.unwrap_or(0.0),
            c.cfo_hz,
            o.clean + o.corrected,
            o.failed,
            if ts_lock { "  TS lock" } else { "" },
            if inverted { "  inverted" } else { "" }
        );
        let r = p.results_mut();
        write_status(
            r,
            lock_state,
            c.mer_db.map(|m| m as f32),
            Some(c.level_dbfs as f32),
            Some(c.cfo_hz),
            &self.summary,
        );
        let rs = c.symbol_rate.max(1.0);
        let st = &self.state;
        r.update_series(CONSTELLATION, |s| {
            s.fill(false, st.constellation.iter().map(|z| [z.re, z.im]))
        });
        r.update_series(EQUALISER, |s| {
            s.fill(
                true,
                st.taps
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| [((i as f64 - MAIN as f64) * 0.5 / rs * 1e6) as f32, v]),
            )
        });
        let hist = &self.mer_hist;
        r.update_series(MER_HISTORY, |s| {
            s.fill(true, hist.iter().enumerate().map(|(i, &v)| [i as f32, v]))
        });
    }

    /// Plots the newest frame of `frames`, 0 dB at its peak.
    fn plot_spectrum(&mut self, frames: &sdrkit_graph::Frames) {
        let n = frames.len();
        if n == 0 {
            return;
        }
        let line = &frames.values[(n - 1) * frames.bins..n * frames.bins];
        let peak = line.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        self.published.results_mut().update_series(SPECTRUM, |s| {
            s.fill(
                true,
                line.iter().enumerate().map(|(i, &v)| [i as f32, v - peak]),
            )
        });
    }
}

impl Component for DvbcReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "dvbc.report",
            vec![
                Port::typed::<DvbcState>("state"),
                Port::typed::<OuterStats>("stats"),
                Port::spectrum("spectrum", SPECTRUM_SPEC).optional("no spectrum plot"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The plots keep their size once full, so their memory is taken now.
        let r = self.published.results_mut();
        for (id, n) in [
            (CONSTELLATION, demod::CONSTELLATION_LEN),
            (SPECTRUM, 1024),
            (EQUALISER, TAPS),
            (MER_HISTORY, MER_HISTORY_LEN),
        ] {
            r.update_series(id, |s| {
                s.points.reserve(n);
                false
            });
        }
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut fresh = false;
        if let Some(s) = io.items::<OuterStats>(1).last() {
            self.outer = s.value;
            fresh = true;
        }
        if let Some(s) = io.items::<DvbcState>(0).last() {
            let mut log = std::mem::take(&mut self.state.log);
            self.state = s.value.clone();
            log.append(&mut self.state.log);
            self.state.log = log;
            fresh = true;
        }
        if fresh {
            self.publish();
        }
        if io.is_connected(2) {
            self.plot_spectrum(io.spectrum(2));
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.published.results())
    }
}

/// The graph of the DVB-C receiver at its default settings.
pub fn builder() -> GraphBuilder {
    let b = GraphBuilder::new()
        .add("demod", DvbcDemod::default())
        .add("outer", DvbcOuter::default())
        .add("monitor", TsMonitorComponent::new(1))
        .add("report", DvbcReport::default())
        .input("demod.in")
        .input("report.spectrum")
        .connect("demod.bytes", "outer.in")
        .feedback("outer.lost", "demod.lost")
        .connect("outer.ts", "monitor.in")
        .connect("demod.state", "report.state")
        .connect("outer.stats", "report.stats");
    layer::attach(b, "monitor.out", &TS_LAYER)
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .status_from("report")
}

/// Where each stack setting goes.
const ROUTES: &[(&str, &[&str])] = &[
    ("symbol_rate", &["demod.symbol_rate"]),
    ("qam", &["demod.qam"]),
    ("annex_c", &["demod.annex_c"]),
    layer::DECODE_VIDEO_ROUTE,
];

/// The DVB-C receiver with the settings of `cfg`: the graph in fixed passes of [`PASS`] samples.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = builder()
        .catalog(&Catalog::of(cfg)?)
        .declare(&declaration())
        .build()?;
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "too low for DVB-C (7 Msps or more)")
        .fixed_passes(PASS)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for (key, targets) in ROUTES {
        b = b.route(key, targets)?;
    }
    b.with_settings(&cfg.settings).build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, sdrkit_core::ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(
    cfg: &sdrkit_core::GenConfig,
) -> Result<Box<dyn sdrkit_core::Generator>, sdrkit_core::ConfigError> {
    Ok(Box::new(gen::DvbcGenerator::from_config(cfg)))
}

/// The DVB-C stack.
pub static DVBC: sdrkit_core::StackDescriptor = sdrkit_core::StackDescriptor {
    id: ID,
    name: "DVB-C",
    group: "TV",
    blurb: "Cable television (DVB-C, J.83 annex A and C), 16- to 256-QAM: services, guide, teletext and subtitles.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 47e6,
        max_hz: 1002e6,
        radio_filter_hz: Some(8e6),
        no_offset_tuning: true,
        ..sdrkit_core::Tuning::anywhere(346e6, 10e6, MIN_RATE, 8e6)
    },
    submodes: &[],
    settings: DvbcSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::DvbC],
    auto_gain: sdrkit_core::AutoGain::Radio,
    verify: Some(&crate::verify::DVBC_HOOKS),
    benchmarks: Some(crate::benchmarks),
};

/// Shown with the tuner.
pub const HINT: &str =
    "DVB-C needs a cable feed: connect the radio to the cable TV outlet (through a DC block and an attenuator), not to an antenna.";
