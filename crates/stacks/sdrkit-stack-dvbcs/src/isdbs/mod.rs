//! ISDB-S (`isdbs`): Japanese BS satellite television, TC8PSK, QPSK and BPSK, TMCC, up to 8 transport streams per
//! transponder, with the transport stream layer of `sdrkit-proto-mpegts` after the decoder.
//!
//! Standard: ITU-R BO.1408-1; ARIB STD-B20.

pub mod rx;
pub mod tx;

use crate::typed::{IsdbsFields, IsdbsStreamRow, ISDBS_STREAMS};
use crate::TS_LAYER;
use rx::{coarse_freq, frame_phases, sync_metric, sync_patterns, DecodeStats, Decoder};
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_fec::rs::Rs;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::results::{LockState, SeriesId};
use sdrkit_core::SignalType;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, Receiver, ResultsDecl, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_proto_dvbs2::inner::{encoder, PunctureRate};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_proto_dvbs2::receive::{item_format, timing, write_status, Timing};
use sdrkit_proto_dvbs2::transmit::{payload, signal_info, TxShaper, TEST_SIGNAL};
use sdrkit_proto_mpegts::layer::{self, DECODE_VIDEO};
use sdrkit_proto_mpegts::monitor::TsMonitorComponent;
use sdrkit_proto_mpegts::outer::rs as outer_code;
use sdrkit_proto_mpegts::source::{Fill, Numbered, NumberedSource, PacketSource};
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::sync::Arc;
use tx::*;

/// The stack id.
pub const ID: &str = "isdbs";

/// The lowest input rate.
pub const MIN_RATE: f64 = 39e6;

/// Input samples per graph pass.
pub const PASS: usize = 65536;

/// One frame of symbols, carrier corrected.
#[derive(Clone, Debug, PartialEq)]
pub struct IsdbsFrame {
    /// The symbols.
    pub syms: Vec<Complex32>,
    /// First frame of a super-frame (W2).
    pub w2: bool,
    /// Es/N0 from W1, dB.
    pub esn0_db: f64,
}

impl sdrkit_graph::Item for IsdbsFrame {
    const KIND: &'static str = "isdbs.frame";
}

impl sdrkit_graph::Item for Tmcc {
    const KIND: &'static str = "isdbs.tmcc";
}

sdrkit_core::choice_enum! {
    /// The slot layout of the test signal.
    pub enum GenLayout {
        /// TC8PSK in 36 slots, QPSK 3/4 in 8, BPSK in 4.
        Mixed = ("0", "TC8PSK 36, QPSK 3/4 8, BPSK 4"),
        /// TC8PSK in 48 slots.
        Tc8psk = ("1", "TC8PSK 48"),
        /// QPSK 1/2 in 48 slots.
        Qpsk = ("2", "QPSK 1/2 48"),
    }
}

/// The stream to decode: its relative number in the TMCC, or -1 for the stream with the most slots.
pub const STREAM_SETTING: Setting<field::I64> = Setting::new("stream", "Stream", "", -1)
    .range(-1.0, 7.0, 1.0)
    .help("-1: the stream with the most slots.");
/// The slot layout of the test signal.
pub const GEN_LAYOUT: Setting<field::Choice<GenLayout>> =
    Setting::choice("gen_layout", "Layout", GenLayout::Mixed)
        .section(TEST_SIGNAL)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the ISDB-S stack.
    pub enum IsdbsSetting {
        /// The stream to decode.
        Stream = STREAM_SETTING,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The slot layout of the test signal.
        GenLayout = GEN_LAYOUT,
    }
}

sdrkit_core::settings! {
    /// The settings of [`IsdbsDecode`].
    pub enum DecodeSetting {
        /// The stream to decode.
        Stream = STREAM_SETTING,
    }
}

/// The BS channel of an IF: "BS-1  1049.48 MHz" or "custom".
pub fn bs_channel(if_hz: f64) -> String {
    for k in 0..12 {
        let f = 1049.48e6 + 38.36e6 * k as f64;
        if (f - if_hz).abs() < 1e3 {
            return format!("BS-{}  {:.2} MHz", 2 * k + 1, f / 1e6);
        }
    }
    "custom".into()
}

/// The state the demodulator sends to the report on each tick of the graph clock.
#[derive(Clone, Debug, Default)]
pub struct DemodState {
    /// Frame sync.
    pub frame_sync: bool,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// Es/N0, dB.
    pub esn0_db: Option<f64>,
    /// Frames.
    pub frames: u64,
    /// Sync losses.
    pub sync_losses: u64,
    /// Recent symbols.
    pub constellation: Vec<Complex32>,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl sdrkit_graph::Item for DemodState {
    const KIND: &'static str = "isdbs.demod_state";
}

/// The state the decoder sends to the report on each tick of the graph clock.
#[derive(Clone, Debug, Default)]
pub struct DecodeState {
    /// The counters.
    pub stats: DecodeStats,
    /// The TMCC in use.
    pub tmcc: Option<Tmcc>,
    /// The stream decoded.
    pub selected: Option<u8>,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl sdrkit_graph::Item for DecodeState {
    const KIND: &'static str = "isdbs.decode_state";
}

/// `isdbs.demod`: matched filter and timing, the carrier from the eighth power, frame sync on W1 and W2 or W3, the phase from
/// W1 and the bursts. Out: one item per frame, and the state.
pub struct IsdbsDemod {
    sync: Option<Timing<32>>,
    fs: f64,
    syms: Vec<Complex32>,
    base: u64,
    locked: Option<(usize, f64)>,
    misses: u32,
    pats: ([f32; 20], [f32; 20]),
    tmp: Vec<Complex32>,
    st: DemodState,
}

impl Default for IsdbsDemod {
    fn default() -> Self {
        IsdbsDemod {
            sync: None,
            fs: 0.0,
            syms: Vec::new(),
            base: 0,
            locked: None,
            misses: 0,
            pats: sync_patterns(),
            tmp: Vec::new(),
            st: DemodState::default(),
        }
    }
}

impl Component for IsdbsDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "isdbs.demod",
            vec![Port::iq("in")],
            vec![
                Port::typed::<IsdbsFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<DemodState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = IsdbsDemod::default();
        self.fs = inputs[0].rate;
        self.sync = Some(timing(
            self.fs / RATE,
            0.35,
            TIMING_BN_T,
            TIMING_BN_T,
            TIMING_PPM,
            false,
        )?);
        Ok(vec![
            item_format::<IsdbsFrame>(&inputs[0]),
            Format::new(Kind::typed::<DemodState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let t0 = io.input_format(0).time_of(io.input(0).first());
        let Some(s) = self.sync.as_mut() else {
            return;
        };
        s.process(io.iq(0), &mut self.syms);
        loop {
            match self.locked {
                None => {
                    if self.syms.len() < 2 * FRAME + 400 {
                        break;
                    }
                    let f = coarse_freq(&self.syms[..65536.min(self.syms.len())]);
                    self.tmp.clear();
                    for (i, &v) in self.syms[..2 * FRAME + 400].iter().enumerate() {
                        let ph = -2.0 * PI * f * (self.base + i as u64) as f64;
                        self.tmp
                            .push(v * Complex32::new(ph.cos() as f32, ph.sin() as f32));
                    }
                    let (a, b) = &self.pats;
                    let best = (0..FRAME)
                        .map(|p| (p, sync_metric(&self.tmp, p, a, b).0))
                        .max_by(|x, y| x.1.total_cmp(&y.1))
                        .unwrap_or((0, 0.0));
                    if best.1 > 0.5 && sync_metric(&self.tmp, best.0 + FRAME, a, b).0 > 0.5 {
                        self.locked = Some((best.0, f));
                        self.misses = 0;
                        self.st.frame_sync = true;
                        self.st
                            .log
                            .push(format!("frame sync, carrier {:+.1} kHz", f * RATE / 1e3));
                    } else {
                        self.syms.drain(..FRAME);
                        self.base += FRAME as u64;
                    }
                }
                Some((p, mut f)) => {
                    if self.syms.len() < p + 2 * FRAME + 400 {
                        break;
                    }
                    // Search +-4 for the frame start (a clock slip).
                    let (a, b) = self.pats;
                    let derot = |v: Complex32, i: usize| {
                        let ph = -2.0 * PI * f * (self.base + i as u64) as f64;
                        v * Complex32::new(ph.cos() as f32, ph.sin() as f32)
                    };
                    let mut best = (p, 0f32, false);
                    for q in p.saturating_sub(4)..=p + 4 {
                        let seg: Vec<Complex32> =
                            (0..200).map(|i| derot(self.syms[q + i], q + i)).collect();
                        let (m, w2) = sync_metric(&seg, 0, &a, &b);
                        if m > best.1 {
                            best = (q, m, w2);
                        }
                    }
                    let (p, m, w2) = best;
                    if m < 0.3 {
                        self.misses += 1;
                        if self.misses >= 3 {
                            self.locked = None;
                            self.st.frame_sync = false;
                            self.st.sync_losses += 1;
                            self.st.log.push("frame sync lost".into());
                            continue;
                        }
                    } else {
                        self.misses = 0;
                    }
                    let mut y: Vec<Complex32> =
                        (0..FRAME).map(|i| derot(self.syms[p + i], p + i)).collect();
                    let (th, bursts) = frame_phases(&y, &a, 0.0);
                    // Es/N0 from W1.
                    let mut acc = Complex32::new(0.0, 0.0);
                    let rotw = Complex32::new(th.cos() as f32, -th.sin() as f32);
                    for i in 0..20 {
                        acc += y[12 + i] * rotw * a[i];
                    }
                    let amp = acc.re / 20.0;
                    let mut noise = 0f32;
                    for i in 0..20 {
                        noise += (y[12 + i] * rotw - Complex32::new(amp * a[i], 0.0)).norm_sqr();
                    }
                    let esn0 = power_to_db((amp * amp) as f64 / (noise as f64 / 20.0).max(1e-9));
                    // Phase: W1 for the header, then interpolated between bursts.
                    for v in y[..HEADER].iter_mut() {
                        *v *= rotw;
                    }
                    let mut prev = th;
                    for (r, &ph) in bursts.iter().enumerate() {
                        let s0 = HEADER + r * (RUN + 4);
                        for j in 0..RUN + 4 {
                            let t = (j as f64 + 0.5) / (RUN + 4) as f64;
                            let phi = prev + (ph - prev) * t;
                            y[s0 + j] *= Complex32::new(phi.cos() as f32, -phi.sin() as f32);
                        }
                        prev = ph;
                    }
                    // Frequency from the phase drift over the frame.
                    let slope = (bursts[191] - th) / (FRAME - 32) as f64;
                    f += 0.5 * slope / (2.0 * PI);
                    let st = &mut self.st;
                    st.frames += 1;
                    st.cfo_hz = f * RATE;
                    st.esn0_db = Some(st.esn0_db.map_or(esn0, |e| e + 0.2 * (esn0 - e)));
                    if io.tick() || st.constellation.is_empty() {
                        st.constellation.clear();
                        st.constellation
                            .extend(y[HEADER..].iter().step_by(97).take(400).copied());
                    }
                    io.items_out::<IsdbsFrame>(0).push(Timed {
                        time: t0,
                        value: IsdbsFrame {
                            syms: y,
                            w2,
                            esn0_db: esn0,
                        },
                    });
                    let next = p + FRAME;
                    let drop = next.saturating_sub(8);
                    self.syms.drain(..drop);
                    self.base += drop as u64;
                    self.locked = Some((next - drop, f));
                }
            }
        }
        if io.tick() {
            let st = DemodState {
                log: std::mem::take(&mut self.st.log),
                ..self.st.clone()
            };
            io.items_out::<DemodState>(1).push(Timed {
                time: t0,
                value: st,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// The noise bandwidth per symbol of the symbol timing loop. It is fixed: the frame sync after it needs a steady clock, and the
/// loop pulls in within the first frames.
const TIMING_BN_T: f64 = 0.002;
/// How far the symbol clock can be from 28.86 Msym/s, ppm. It covers the clocks of the radio and the transmitter.
const TIMING_PPM: f64 = 300.0;

/// `isdbs.decode`: the header Viterbi, the TMCC, the frame trellis, the byte de-interleaver, energy dispersal and RS(204,188).
/// Output `ts` is the transport stream of the stream decoded.
#[derive(Default)]
pub struct IsdbsDecode {
    dec: Decoder,
    ts: TsOut,
}

impl Component for IsdbsDecode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "isdbs.decode",
            vec![Port::typed::<IsdbsFrame>("frames")],
            vec![
                stream::port("ts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Tmcc>("tmcc").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<DecodeState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DecodeSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let stream = self.dec.stream;
        self.dec = Decoder::default();
        self.dec.stream = stream;
        Ok(vec![
            inputs[0].with_kind(stream::kind()),
            Format::new(Kind::typed::<Tmcc>(), 0.0),
            Format::new(Kind::typed::<DecodeState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for fr in io.items::<IsdbsFrame>(0) {
            let k = (2.0 * 10f64.powf(fr.value.esn0_db.clamp(0.0, 30.0) / 10.0)) as f32;
            let before = self.dec.stats.tmcc_good;
            let time = fr.time;
            let out = &mut self.ts;
            let ts = io.items_out::<TsPacket>(0);
            // A packet that does not start with the sync byte is not sent, and the monitor counts it as lost.
            self.dec.frame(&fr.value.syms, fr.value.w2, k, &mut |p, _| {
                out.push(ts, &p, time, false);
            });
            if self.dec.stats.tmcc_good != before {
                if let Some(t) = self.dec.current.clone() {
                    io.items_out::<Tmcc>(1).push(Timed { time, value: t });
                }
            }
        }
        if io.tick() {
            let st = DecodeState {
                stats: self.dec.stats.clone(),
                tmcc: self.dec.current.clone(),
                selected: self.dec.selected(),
                log: std::mem::take(&mut self.dec.log),
            };
            io.items_out::<DecodeState>(2).push(Timed {
                time: 0.0,
                value: st,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecodeSetting::Stream => self.dec.stream = STREAM_SETTING.read(id, v)? as i32,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            DecodeSetting::Stream,
            Value::I64(i64::from(self.dec.stream)),
        )])
    }
}

sdrkit_core::choice_enum! {
    /// What the ISDB-S receiver does now.
    pub enum RxState {
        /// Looking for frames.
        Searching = ("searching", "Searching"),
        /// Frame sync holds.
        FrameSync = ("frame_sync", "Frame sync"),
        /// The TMCC is decoded.
        Tmcc = ("tmcc", "TMCC decoded"),
        /// Transport packets come out.
        TransportStream = ("ts", "Transport stream"),
    }
}

const CONSTELLATION: SeriesId = SeriesId(0);

/// The plot of the stack.
pub const SERIES: [SeriesDef; 1] = [SeriesDef {
    key: "constellation",
    label: "Constellation",
    x_unit: "",
    y_unit: "",
}];

/// What `isdbs.report` publishes.
fn report_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(IsdbsFields::SHAPE.fields)
        .with_table(ISDBS_STREAMS.def())
        .with_series(&SERIES[0])
}

/// What the ISDB-S stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// `isdbs.report`: the results of the receiver from the states of the demodulator and the decoder.
pub struct IsdbsReport {
    published: Published,
    demod: DemodState,
    decode: DecodeState,
    key: String,
    summary: String,
}

impl Default for IsdbsReport {
    fn default() -> Self {
        IsdbsReport {
            published: Published::from_decl(Arc::new(report_declaration())),
            demod: DemodState::default(),
            decode: DecodeState::default(),
            key: String::new(),
            summary: String::with_capacity(128),
        }
    }
}

impl IsdbsReport {
    fn publish(&mut self) {
        let (dm, dc) = (&self.demod, &self.decode);
        let s = &dc.stats;
        let delivering = s.rs_clean + s.rs_corrected > 0;
        let state = if delivering {
            RxState::TransportStream
        } else if dc.tmcc.is_some() {
            RxState::Tmcc
        } else if dm.frame_sync {
            RxState::FrameSync
        } else {
            RxState::Searching
        };
        let p = &mut self.published;
        p.put(&IsdbsFields {
            state,
            frame_sync: dm.frame_sync,
            offset_hz: (state != RxState::Searching).then_some(Hertz(dm.cfo_hz)),
            esn0_db: dm.esn0_db,
            frames: dm.frames as i64,
            superframes: s.superframes as i64,
            sync_losses: dm.sync_losses as i64,
            tmcc_good: s.tmcc_good as i64,
            tmcc_bad: s.tmcc_bad as i64,
            change_counter: dc.tmcc.as_ref().map(|t| t.change as i64),
            rs_clean: s.rs_clean as i64,
            rs_corrected: s.rs_corrected as i64,
            rs_failed: s.rs_failed as i64,
            stream: dc.selected.map(|v| v as i64),
            ews: dc.tmcc.as_ref().is_some_and(|t| t.ews),
            ts_lock: dm.frame_sync && delivering,
        });
        if let Some(t) = &dc.tmcc {
            let slots = t.slots();
            for k in 0..8u8 {
                if t.ts_ids[k as usize] == 0xFFFF {
                    continue;
                }
                let mine = || (0..SLOTS).filter(|&i| t.ts_of_slot[i] == k);
                let first = mine().next().map(|i| slots[i].0);
                let mixed = mine().any(|i| Some(slots[i].0) != first);
                let carrying = mine().filter(|&i| slots[i].1).count();
                let rate = carrying as f64 * 188.0 * 8.0 * RATE / FRAME as f64;
                self.key.clear();
                let _ = write!(self.key, "{k}");
                let row = IsdbsStreamRow {
                    stream: i64::from(k),
                    ts_id: i64::from(t.ts_ids[k as usize]),
                    slots: mine().count() as i64,
                    modulation: first.filter(|_| !mixed).map(Mc::choice),
                    mixed,
                    bitrate_bps: rate,
                    decoded: Some(k) == dc.selected,
                };
                ISDBS_STREAMS.put(p, &self.key, &row);
            }
        }
        let lock_state = match state {
            RxState::TransportStream => LockState::Locked,
            RxState::FrameSync | RxState::Tmcc => LockState::Tracking,
            RxState::Searching => LockState::Searching,
        };
        self.summary.clear();
        let _ = write!(
            self.summary,
            "ISDB-S {}  Es/N0 {:.1} dB  offset {:+.1} kHz  RS ok {} bad {}",
            state.label(),
            dm.esn0_db.unwrap_or(0.0),
            dm.cfo_hz / 1e3,
            s.rs_clean + s.rs_corrected,
            s.rs_failed,
        );
        let r = p.results_mut();
        write_status(
            r,
            lock_state,
            dm.esn0_db.map(|v| v as f32),
            None,
            Some(dm.cfo_hz),
            &self.summary,
        );
        r.update_series(CONSTELLATION, |x| {
            x.fill(false, dm.constellation.iter().map(|z| [z.re, z.im]))
        });
    }
}

impl Component for IsdbsReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "isdbs.report",
            vec![
                Port::typed::<DemodState>("demod"),
                Port::typed::<DecodeState>("decode"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut fresh = false;
        for st in io.items::<DemodState>(0) {
            for l in &st.value.log {
                self.published.results_mut().log(l.clone());
            }
            self.demod.clone_from(&st.value);
            fresh = true;
        }
        for st in io.items::<DecodeState>(1) {
            for l in &st.value.log {
                self.published.results_mut().log(l.clone());
            }
            self.decode.clone_from(&st.value);
            fresh = true;
        }
        if fresh {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The streams are of the transponder of the frequency before.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.table(ISDBS_STREAMS.key) {
            self.published.clear_rows(ISDBS_STREAMS.def());
        }
    }
    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.published.results())
    }
}

/// The period of the results, seconds of signal.
pub const REPORT_S: f64 = 0.05;

/// The graph of the ISDB-S receiver at its default settings.
pub fn builder() -> GraphBuilder {
    let b = GraphBuilder::new()
        .add("demod", IsdbsDemod::default())
        .add("decode", IsdbsDecode::default())
        .add("monitor", TsMonitorComponent::new(1))
        .add("report", IsdbsReport::default())
        .input("demod.in")
        .connect("demod.frames", "decode.frames")
        .connect("decode.ts", "monitor.in")
        .connect("demod.state", "report.demod")
        .connect("decode.state", "report.decode");
    layer::attach(b, "monitor.out", &TS_LAYER)
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .status_from("report")
}

/// The ISDB-S receiver with the settings of `cfg`: the graph in fixed passes of [`PASS`] samples.
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
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "too low for ISDB-S (39 Msample/s or more)")
        .fixed_passes(PASS)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .route("stream", &["decode.stream"])?
        .route(layer::DECODE_VIDEO_ROUTE.0, layer::DECODE_VIDEO_ROUTE.1)?
        .with_settings(&cfg.settings)
        .build()
}

/// The slot groups of a layout of the test signal.
pub fn layout(n: GenLayout) -> Vec<(Mc, u8)> {
    match n {
        GenLayout::Tc8psk => vec![(Mc::Tc8psk, 48)],
        GenLayout::Qpsk => vec![(Mc::Qpsk(PunctureRate::R12), 48)],
        GenLayout::Mixed => vec![
            (Mc::Tc8psk, 36),
            (Mc::Qpsk(PunctureRate::R34), 8),
            (Mc::Bpsk12, 4),
        ],
    }
}

/// The generator's parameters.
#[derive(Clone, Debug)]
pub struct IsdbsGenParams {
    /// Sample rate.
    pub fs: f64,
    /// The groups.
    pub groups: Vec<(Mc, u8)>,
}

/// The ISDB-S generator: each group is its own transport stream (0x4010 + n), the first carrying the test multiplex and the others
/// numbered packets on PIDs 0x0021, 0x0022 and on.
pub struct IsdbsGenerator {
    p: IsdbsGenParams,
    tmcc: Tmcc,
    sources: Vec<Box<dyn PacketSource>>,
    enc: Encoder,
    shaper: Option<TxShaper>,
    rs: Rs,
    prbs: Vec<u8>,
    bursts: Vec<u8>,
    frames: Vec<Vec<[u8; 203]>>,
    frame_i: usize,
    syms: Vec<Complex32>,
}

impl IsdbsGenerator {
    /// A generator with these parameters.
    ///
    /// # Errors
    ///
    /// The [`LayoutError`] of a layout that does not fill 48 slots in whole units in at most four modulations.
    pub fn new(p: IsdbsGenParams) -> Result<Self, LayoutError> {
        let groups = Tmcc::sorted(p.groups.clone());
        check_layout(&groups)?;
        let mut ts_of_slot = [0u8; SLOTS];
        let mut ids = [0xFFFFu16; 8];
        let mut s = 0;
        for (k, (_, n)) in groups.iter().enumerate() {
            for _ in 0..*n {
                ts_of_slot[s] = k as u8;
                s += 1;
            }
            ids[k] = 0x4010 + k as u16;
        }
        let sources = (0..groups.len())
            .map(|k| -> Box<dyn PacketSource> {
                if k == 0 {
                    payload(false)
                } else {
                    Box::new(NumberedSource::new(Numbered::new(
                        0x20 + k as u16,
                        Fill::N7I13,
                    )))
                }
            })
            .collect();
        let tmcc = Tmcc {
            change: 0,
            groups,
            ts_of_slot,
            ts_ids: ids,
            ews: false,
            uplink: 0,
        };
        Ok(IsdbsGenerator {
            tmcc,
            sources,
            enc: encoder(),
            shaper: TxShaper::new(p.fs, RATE, 0.35).ok(),
            rs: outer_code(),
            prbs: sf_prbs(),
            bursts: burst_bits(),
            frames: Vec::new(),
            frame_i: 0,
            syms: Vec::new(),
            p,
        })
    }
    /// The generator of a generator configuration. The schema of its values must be that of [`IsdbsSetting`].
    pub fn from_config(cfg: &GenConfig) -> Self {
        let n = cfg.settings.get(&GEN_LAYOUT);
        let p = IsdbsGenParams {
            fs: cfg.rate_hz,
            groups: layout(n),
        };
        // The built-in layouts are valid.
        Self::new(p).unwrap_or_else(|e| unreachable!("layout {n:?}: {e}"))
    }
    fn superframe(&mut self) {
        let slots = self.tmcc.slots();
        // Packets per frame and slot, RS coded, dispersed.
        let mut orig = vec![vec![[0u8; 203]; SLOTS]; SF];
        for (f, of) in orig.iter_mut().enumerate() {
            for s in 0..SLOTS {
                if !slots[s].1 {
                    continue;
                }
                let src = &mut self.sources[self.tmcc.ts_of_slot[s] as usize];
                let mut w = [0u8; 204];
                let mut p = [0u8; 188];
                src.next_packet(&mut p);
                w[..188].copy_from_slice(&p);
                // A packet is 188 bytes and the code RS(204,188), so the lengths always match.
                let _ = self.rs.encode(&p, &mut w[188..]);
                for j in 0..203 {
                    of[s][j] = w[1 + j] ^ self.prbs[(f * SLOTS + s) * 204 + 1 + j];
                }
            }
        }
        // Interleave across the 8 frames.
        let mut tx = vec![vec![[0u8; 203]; SLOTS]; SF];
        for f in 0..SF {
            for s in 0..SLOTS {
                for j in 0..203 {
                    let n = 8 * j + f;
                    tx[n / 203][s][n % 203] = orig[f][s][j];
                }
            }
        }
        self.frames = tx
            .into_iter()
            .map(|fr| (0..SLOTS).filter(|&s| slots[s].1).map(|s| fr[s]).collect())
            .collect();
        self.frame_i = 0;
    }
}

impl Generator for IsdbsGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.fs
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let Some(mut shaper) = self.shaper.take() else {
            return 0;
        };
        while shaper.ready() < out.len() {
            if self.frame_i == 0 || self.frame_i >= SF {
                self.superframe();
            }
            let f = self.frame_i;
            let blocks = tmcc_blocks(&self.tmcc);
            let h = header_bits(f, &blocks[f]);
            let slots = self.tmcc.slots();
            encode_frame(
                &mut self.enc,
                &h,
                &slots,
                &self.frames[f],
                &self.bursts,
                &mut self.syms,
            );
            shaper.push(&self.syms);
            self.frame_i += 1;
        }
        let n = shaper.pop(out);
        self.shaper = Some(shaper);
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let carrying = self.tmcc.slots().iter().filter(|s| s.1).count();
        let bits = carrying as f64 * 188.0 * 8.0 * RATE / FRAME as f64;
        Some(signal_info(RATE, 0.35, bits))
    }
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, sdrkit_core::ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(
    cfg: &GenConfig,
) -> Result<Box<dyn sdrkit_core::Generator>, sdrkit_core::ConfigError> {
    Ok(Box::new(IsdbsGenerator::from_config(cfg)))
}

/// The ISDB-S stack.
pub static ISDBS: sdrkit_core::StackDescriptor = sdrkit_core::StackDescriptor {
    id: ID,
    name: "ISDB-S (BS)",
    group: "TV",
    blurb: "Japanese BS satellite television (ISDB-S): TC8PSK, QPSK, BPSK and up to eight transport streams.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 950e6,
        max_hz: 2150e6,
        no_offset_tuning: true,
        ..sdrkit_core::Tuning::anywhere(1049.48e6, 48e6, MIN_RATE, 34.5e6)
    },
    submodes: &[],
    settings: IsdbsSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::IsdbS],
    auto_gain: sdrkit_core::AutoGain::Radio,
    verify: Some(&crate::verify::ISDBS_HOOKS),
    benchmarks: None,
};
