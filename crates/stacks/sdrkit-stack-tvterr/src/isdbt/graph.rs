//! The ISDB-T receiver as components: `isdbt.sync` (from samples to the carriers of locked symbols), `isdbt.equalise`,
//! `isdbt.deinterleave`, `isdbt.fec` (the three layers), `isdbt.merge` (one transport stream with its signal time) and
//! `isdbt.report` (the results).

use super::chain::{BitDelay, LayerCounters, LayerRx, LayerTi};
use super::eq::{Cells, EqState, Equaliser};
use super::layout::{Band, Layout};
use super::results::{self as f, code_rate_of, modulation_of, LayerName};
use super::sync::{IsdbtSync, Segments, Symbol, SyncState};
use super::tmcc::{LayerParams, Tmcc};
use super::{
    frame_seconds, ifft_rate, layer_rate, EdgeChoice, Guard, IsdbtSetting, NarrowSetting,
    SegmentCount, Width, EDGE_PILOT, SEGMENTS, WIDTH,
};
use crate::status::{quality_of, write_status, StatusLine, StepColour};
use crate::typed::{
    IsdbtFields, IsdbtLayerRow, IsdbtNarrowFields, ISDBT_LAYERS, ISDBT_SERIES, ISDBT_TMCC,
};
use num_complex::Complex32;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::results::Series;
use sdrkit_core::{
    Choice, ConfigError, EventDef, LockState, Record, Results, SeriesId, SettingError, SettingId,
    SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Latency,
    Port, RateFrom, SlicePool, Timed,
};
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};
use std::sync::Arc;

macro_rules! item {
    ($t:ty, $k:expr) => {
        impl Item for $t {
            const KIND: &'static str = $k;
        }
    };
}
item!(Symbol, "isdbt.carriers");
item!(SyncState, "isdbt.sync");
item!(Cells, "isdbt.cells");
item!(EqState, "isdbt.eq");
item!(LayerCells, "isdbt.layer_cells");
item!(LayerPackets, "isdbt.layer_packets");
item!(FecState, "isdbt.fec");

impl Item for Tmcc {
    const KIND: &'static str = "isdbt.tmcc";
    fn event_def() -> Option<&'static EventDef> {
        Some(&ISDBT_TMCC)
    }
    fn write_fields(&self, fields: &mut Record) {
        f::write_tmcc(self, fields);
    }
}

fn typed<T: Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

/// An output port of typed items, which have no sample rate.
fn items<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

fn width_values(width: Width) -> Values {
    values_of([(IsdbtSetting::Width, Value::Choice(width.index()))])
}

/// `isdbt.sync`: from radio samples to carriers of locked symbols. Output `tmcc` carries the TMCC each time it changes. A
/// narrow-band sync (ISDB-TSB) also takes the segment count and the upper-edge pilot rule.
pub struct SyncComp {
    width: Width,
    narrow: Option<(SegmentCount, EdgeChoice)>,
    sync: Option<IsdbtSync>,
    syms: Vec<Symbol>,
    last_tmcc: Option<Tmcc>,
}

impl Default for SyncComp {
    fn default() -> Self {
        SyncComp::new(Width::W6)
    }
}

impl SyncComp {
    /// For a channel width.
    pub fn new(width: Width) -> SyncComp {
        SyncComp {
            width,
            narrow: None,
            sync: None,
            syms: Vec::new(),
            last_tmcc: None,
        }
    }

    /// A narrow-band sync for a raster width, a segment count and an upper-edge pilot rule.
    pub fn narrow(width: Width, segments: SegmentCount, edge: EdgeChoice) -> SyncComp {
        SyncComp {
            narrow: Some((segments, edge)),
            ..SyncComp::new(width)
        }
    }
}

impl Component for SyncComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "isdbt.sync",
            vec![Port::iq("in")],
            vec![
                items::<Symbol>("carriers"),
                items::<SyncState>("state"),
                items::<Tmcc>("tmcc"),
            ],
        )
        .with_settings(if self.narrow.is_some() {
            NarrowSetting::schema()
        } else {
            IsdbtSetting::schema()
        })
    }
    fn values(&self) -> Values {
        match self.narrow {
            Some((n, e)) => values_of([
                (NarrowSetting::Width, Value::Choice(self.width.index())),
                (NarrowSetting::Segments, Value::Choice(n.index())),
                (NarrowSetting::EdgePilot, Value::Choice(e.index())),
            ]),
            None => width_values(self.width),
        }
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A retune keeps the I/Q orientation found last.
        let mirrored = self
            .sync
            .as_ref()
            .is_some_and(|s| s.state().stage >= 1 && s.state().mirrored);
        let (segments, edge) = match self.narrow {
            Some((n, e)) => (n.segments(), e.edge()),
            None => (Segments::Full, Default::default()),
        };
        let mut sync = IsdbtSync::new(inputs[0].rate, self.width, segments, edge)?;
        sync.prefer_mirrored(mirrored);
        self.sync = Some(sync);
        self.last_tmcc = None;
        Ok(vec![
            typed::<Symbol>(),
            typed::<SyncState>(),
            typed::<Tmcc>(),
        ])
    }
    /// The lock, the mode and the TMCC are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        if let Some(sync) = &mut self.sync {
            sync.forget();
        }
        self.last_tmcc = None;
    }
    fn process(&mut self, io: &mut Io) {
        let Some(sync) = &mut self.sync else { return };
        let x = io.iq(0);
        self.syms.clear();
        sync.feed(x, &mut self.syms);
        let t_in = io.input(0).first() as f64 / io.input_format(0).rate.max(1.0);
        for s in self.syms.drain(..) {
            let time = s.time;
            if Some(s.tmcc) != self.last_tmcc {
                self.last_tmcc = Some(s.tmcc);
                io.items_out::<Tmcc>(2).push(Timed {
                    time,
                    value: s.tmcc,
                });
            }
            io.items_out::<Symbol>(0).push(Timed { time, value: s });
        }
        if sync.take_dirty() {
            let mut st = sync.state().clone();
            st.width_mhz = sync.width();
            io.items_out::<SyncState>(1).push(Timed {
                time: t_in,
                value: st,
            });
            // The log lines went out with the state.
            if let Some(s) = &mut self.sync {
                s.clear_log();
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        if let Some((n, e)) = &mut self.narrow {
            match NarrowSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
                NarrowSetting::Width => self.width = WIDTH.read(id, v)?,
                NarrowSetting::Segments => *n = SEGMENTS.read(id, v)?,
                NarrowSetting::EdgePilot => *e = EDGE_PILOT.read(id, v)?,
            }
            return Ok(Applied::Reformat);
        }
        match IsdbtSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IsdbtSetting::Width => self.width = WIDTH.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
}

/// `isdbt.equalise`: channel estimate, differential detection, noise per cell. The cells of a symbol leave
/// [`DELAY`](super::eq::DELAY) symbols after it arrives; the end of the input lets out those still waiting. Its latency is
/// that many symbol periods: the period of the symbols it last took, and before any that of mode 3 with a guard of 1/8 in
/// 6 MHz (1.134 ms).
pub struct EqComp {
    eq: Equaliser,
    out: Vec<(Cells, Option<EqState>)>,
    /// The symbol period, s.
    period_s: f64,
    /// Time and frame position of the last symbol taken.
    last: Option<(f64, usize)>,
}

/// The symbol period of mode 3 with a guard of 1/8 in a 6 MHz channel, s.
const PERIOD_MODE3_S: f64 = 1.008e-3 * 1.125;

impl Default for EqComp {
    fn default() -> Self {
        EqComp {
            eq: Equaliser::default(),
            out: Vec::new(),
            period_s: PERIOD_MODE3_S,
            last: None,
        }
    }
}

impl EqComp {
    /// Sends the symbols the equaliser let out.
    fn emit(&mut self, io: &mut Io) {
        for (c, state) in self.out.drain(..) {
            if let Some(st) = state {
                io.items_out::<EqState>(1).push(Timed {
                    time: c.time,
                    value: st,
                });
            }
            io.items_out::<Cells>(0).push(Timed {
                time: c.time,
                value: c,
            });
        }
    }
}

impl Component for EqComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "isdbt.equalise",
            vec![Port::typed::<Symbol>("in")],
            vec![items::<Cells>("out"), items::<EqState>("state")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn latency(&self) -> Latency {
        Latency {
            group_delay_s: super::eq::DELAY as f64 * self.period_s,
            first_valid: 0,
        }
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = EqComp::default();
        Ok(vec![typed::<Cells>(), typed::<EqState>()])
    }
    /// The channel estimate is of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        *self = EqComp::default();
    }
    fn process(&mut self, io: &mut Io) {
        let out = &mut self.out;
        for s in io.items::<Symbol>(0) {
            let (time, pos) = (s.value.time, s.value.frame_pos);
            if let Some((t, _)) = self.last.filter(|l| l.1 + 1 == pos && time > l.0) {
                self.period_s = time - t;
            }
            self.last = Some((time, pos));
            self.eq
                .process(&s.value, &mut |c, st| out.push((c, st.cloned())));
        }
        self.emit(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        let out = &mut self.out;
        self.eq.flush(&mut |c, st| out.push((c, st.cloned())));
        self.emit(io);
    }
}

/// Cells of one symbol per layer, de-interleaved in frequency and time.
#[derive(Clone, Debug, Default)]
pub struct LayerCells {
    /// Signal time.
    pub time: f64,
    /// Index in the frame.
    pub frame_pos: usize,
    /// Epoch.
    pub epoch: u32,
    /// The TMCC in force.
    pub tmcc: Tmcc,
    /// Mode.
    pub mode: u8,
    /// Per layer A, B, C (empty when unused).
    pub layers: [Arc<[(Complex32, f32)]>; 3],
}

/// `isdbt.deinterleave`: frequency de-interleaving per group, then time de-interleaving per layer.
#[derive(Default)]
pub struct DeintComp {
    epoch: u32,
    layout: Option<Layout>,
    ti: Vec<Option<LayerTi>>,
    tmp: Vec<(Complex32, f32)>,
    layer: Vec<(Complex32, f32)>,
    pool: SlicePool<(Complex32, f32)>,
    empty: Arc<[(Complex32, f32)]>,
}

impl Component for DeintComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "isdbt.deinterleave",
            vec![Port::typed::<Cells>("in")],
            vec![items::<LayerCells>("out")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.layout = None;
        self.ti.clear();
        Ok(vec![typed::<LayerCells>()])
    }
    /// The cells in the interleavers are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.layout = None;
        self.ti.clear();
    }
    fn process(&mut self, io: &mut Io) {
        for c in io.items::<Cells>(0) {
            let v = &c.value;
            if self.layout.is_none() || v.epoch != self.epoch {
                self.epoch = v.epoch;
                self.layout = Some(Layout::new(v.mode, &v.tmcc.current, v.band));
                self.ti = v
                    .tmcc
                    .current
                    .layers
                    .iter()
                    .map(|l| l.used().then(|| LayerTi::new(v.mode, l)))
                    .collect();
                self.tmp = vec![(Complex32::new(0.0, 0.0), f32::INFINITY); v.cells.len()];
            }
            let lay = self
                .layout
                .as_ref()
                .expect("the layout is built above when it is missing");
            lay.deinterleave(&v.cells, &mut self.tmp);
            let mut layers: [Arc<[(Complex32, f32)]>; 3] =
                std::array::from_fn(|_| self.empty.clone());
            let mut at = 0;
            for li in 0..3 {
                let l = &v.tmcc.current.layers[li];
                if !l.used() {
                    continue;
                }
                let n = l.segments as usize * lay.d;
                let cells = &mut self.layer;
                cells.clear();
                cells.extend_from_slice(&self.tmp[at..at + n]);
                at += n;
                if let Some(ti) = &mut self.ti[li] {
                    ti.push(cells);
                }
                layers[li] = self.pool.exact(n, |out| out.copy_from_slice(cells));
            }
            io.items_out::<LayerCells>(0).push(Timed {
                time: c.time,
                value: LayerCells {
                    time: v.time,
                    frame_pos: v.frame_pos,
                    epoch: v.epoch,
                    tmcc: v.tmcc,
                    mode: v.mode,
                    layers,
                },
            });
        }
    }
}

/// Packets decoded during one symbol, with their layer and whether Reed-Solomon accepted them.
#[derive(Clone, Debug, Default)]
pub struct LayerPackets {
    /// Signal time.
    pub time: f64,
    /// The packets in the order they completed: the first `count` of the slice.
    pub packets: Arc<[LayerPacket]>,
    /// The number of packets.
    pub count: usize,
}

impl LayerPackets {
    /// The packets.
    pub fn packets(&self) -> &[LayerPacket] {
        &self.packets[..self.count.min(self.packets.len())]
    }
}

/// One transport packet of a layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerPacket {
    /// The layer: 0, 1 or 2 for A, B or C.
    pub layer: u8,
    /// The packet.
    pub bytes: [u8; TS],
    /// Reed-Solomon accepted it.
    pub ok: bool,
}

impl Default for LayerPacket {
    fn default() -> Self {
        LayerPacket {
            layer: 0,
            bytes: [0; TS],
            ok: false,
        }
    }
}

/// The FEC counters of the three layers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FecState {
    /// Per layer A, B, C.
    pub layers: [Option<(LayerParams, LayerCounters)>; 3],
    /// Mode.
    pub mode: u8,
}

/// `isdbt.fec`: per layer soft demapping, bit de-interleaving, Viterbi, byte de-interleaving, energy-dispersal removal and RS.
#[derive(Default)]
pub struct FecComp {
    epoch: u32,
    rx: Vec<Option<LayerRx>>,
    models: [BitDelay; 3],
    out: Vec<([u8; TS], bool)>,
    packets: Vec<LayerPacket>,
    pool: SlicePool<LayerPacket>,
    started: bool,
}

impl Component for FecComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "isdbt.fec",
            vec![Port::typed::<LayerCells>("in")],
            vec![items::<LayerPackets>("out"), items::<FecState>("state")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.epoch = 0;
        self.rx.clear();
        self.models = Default::default();
        self.started = false;
        Ok(vec![typed::<LayerPackets>(), typed::<FecState>()])
    }
    /// The counts are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.epoch = 0;
        self.rx.clear();
        self.models = Default::default();
        self.started = false;
    }
    fn process(&mut self, io: &mut Io) {
        for c in io.items::<LayerCells>(0) {
            let v = &c.value;
            if !self.started || v.epoch != self.epoch {
                // Keep the bit-delay model each layer settled on.
                for (li, r) in self.rx.iter().enumerate() {
                    if let Some(r) = r {
                        self.models[li] = r.c.model;
                    }
                }
                self.started = true;
                self.epoch = v.epoch;
                self.rx = v
                    .tmcc
                    .current
                    .layers
                    .iter()
                    .enumerate()
                    .map(|(li, l)| l.used().then(|| LayerRx::new(v.mode, *l, self.models[li])))
                    .collect();
            }
            self.packets.clear();
            for li in 0..3 {
                if let Some(rx) = &mut self.rx[li] {
                    self.out.clear();
                    rx.push_symbol(&v.layers[li], v.frame_pos, &mut self.out);
                    self.packets
                        .extend(self.out.iter().map(|(p, ok)| LayerPacket {
                            layer: li as u8,
                            bytes: *p,
                            ok: *ok,
                        }));
                }
            }
            if !self.packets.is_empty() {
                let packets = &self.packets;
                let pk = LayerPackets {
                    time: v.time,
                    packets: self
                        .pool
                        .at_least(packets.len(), |out| out.copy_from_slice(packets)),
                    count: packets.len(),
                };
                io.items_out::<LayerPackets>(0).push(Timed {
                    time: c.time,
                    value: pk,
                });
            }
            if v.frame_pos % 17 == 16 {
                let mut st = FecState {
                    mode: v.mode,
                    ..Default::default()
                };
                for li in 0..3 {
                    if let Some(rx) = &self.rx[li] {
                        st.layers[li] = Some((rx.params(), rx.c));
                    }
                }
                io.items_out::<FecState>(1).push(Timed {
                    time: c.time,
                    value: st,
                });
            }
        }
    }
}

/// `isdbt.merge`: the layers' packets as one transport stream in the order they complete, each with its signal time.
#[derive(Default)]
pub struct MergeComp {
    ts: TsOut,
}

impl Component for MergeComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "isdbt.merge",
            vec![Port::typed::<LayerPackets>("in")],
            vec![stream::port("ts").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(stream::kind(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let packets = io.items::<LayerPackets>(0);
        let out = io.items_out::<TsPacket>(0);
        for p in packets {
            for pk in p.value.packets() {
                self.ts.push(out, &pk.bytes, p.value.time, !pk.ok);
            }
        }
    }
}

/// The required SNR of a modulation and code rate in a Gaussian channel, dB, from the DVB-T figures (ETSI EN 300 744 table
/// A.1). DQPSK is QPSK plus 3 dB.
pub fn required_snr(p: &LayerParams) -> f64 {
    let t = [
        [3.1, 4.9, 5.9, 6.9, 7.7],
        [8.8, 11.1, 12.5, 13.5, 13.9],
        [14.4, 16.5, 18.0, 19.3, 20.1],
    ];
    let r = p.code_rate.min(4) as usize;
    match p.modulation {
        0 => t[0][r] + 3.0,
        1 => t[0][r],
        2 => t[1][r],
        _ => t[2][r],
    }
}

/// The plots of the report, in the order of [`ISDBT_SERIES`].
struct SeriesIds([SeriesId; 5]);

/// Fills `s` with `points` and says whether it changed, keeping its memory.
fn fill_series(s: &mut Series, line: bool, points: &[[f32; 2]]) -> bool {
    if s.line == line && s.points == points {
        return false;
    }
    s.line = line;
    s.points.clear();
    s.points.extend_from_slice(points);
    true
}

/// Reports of good and bad words that the quality keeps.
const HISTORY: usize = 40;

/// `isdbt.report`: the results of the receiver.
pub struct Report {
    p: Published,
    series: SeriesIds,
    width: Width,
    summaries: [String; 4],
    note: Option<&'static str>,
    sync: SyncState,
    fec: FecState,
    eq: EqState,
    hist: std::collections::VecDeque<(u64, u64)>,
    last: (u64, u64),
    flowing: bool,
    quality: f64,
}

impl Default for Report {
    fn default() -> Self {
        Report::new(Width::W6)
    }
}

impl Report {
    /// For a channel width.
    pub fn new(width: Width) -> Report {
        Report::with_declaration(width, "ISDB-T", None, f::report_declaration())
    }

    /// The report of a narrow-band (ISDB-TSB) receiver: it also publishes the segment count in use and `note`.
    pub fn narrow(width: Width, note: &'static str) -> Report {
        Report::with_declaration(
            width,
            "ISDB-TSB",
            Some(note),
            f::narrow_report_declaration(),
        )
    }

    fn with_declaration(
        width: Width,
        name: &'static str,
        note: Option<&'static str>,
        decl: sdrkit_core::ResultsDecl,
    ) -> Report {
        let decl = Arc::new(decl);
        let ids = std::array::from_fn(|i| {
            decl.series_id(ISDBT_SERIES[i].key)
                .unwrap_or_else(|| unreachable!("the report declares every plot"))
        });
        Report {
            p: Published::from_decl(decl),
            series: SeriesIds(ids),
            width,
            summaries: ["searching", "syncing", "TMCC decoded", "Locked"]
                .map(|stage| format!("{stage} - {name}")),
            note,
            sync: SyncState {
                symbol: -1,
                ..Default::default()
            },
            fec: FecState::default(),
            eq: EqState::default(),
            hist: std::collections::VecDeque::with_capacity(HISTORY + 1),
            last: (0, 0),
            flowing: false,
            quality: 0.0,
        }
    }
}

impl Component for Report {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "isdbt.report",
            vec![
                Port::typed::<SyncState>("sync"),
                Port::typed::<EqState>("eq"),
                Port::typed::<FecState>("fec"),
            ],
        )
        .with_settings(IsdbtSetting::schema())
    }
    fn values(&self) -> Values {
        width_values(self.width)
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        Ok(vec![])
    }
    /// Every result is of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.reset();
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match IsdbtSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IsdbtSetting::Width => self.width = WIDTH.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        for s in io.items::<SyncState>(0) {
            for l in &s.value.log {
                self.p.results_mut().log(l.clone());
            }
            if s.value.stage < 2 && self.sync.stage == 2 {
                self.fec = FecState::default();
                self.flowing = false;
            }
            self.sync.clone_from(&s.value);
            changed = true;
        }
        if let Some(e) = io.items::<EqState>(1).last() {
            self.eq.clone_from(&e.value);
            changed = true;
        }
        for fs in io.items::<FecState>(2) {
            self.fec = fs.value;
            let (mut g, mut b) = (0u64, 0u64);
            for (_, c) in self.fec.layers.iter().flatten() {
                g += c.rs_clean + c.rs_corrected;
                b += c.rs_failed;
            }
            let d = (g.saturating_sub(self.last.0), b.saturating_sub(self.last.1));
            self.last = (g, b);
            self.hist.push_back(d);
            while self.hist.len() > HISTORY {
                self.hist.pop_front();
            }
            changed = true;
        }
        if changed {
            self.publish();
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

impl Report {
    /// The report of a receiver that has seen no signal.
    fn reset(&mut self) {
        self.sync = SyncState {
            symbol: -1,
            ..Default::default()
        };
        self.fec = FecState::default();
        self.eq = EqState::default();
        self.hist.clear();
        self.last = (0, 0);
        self.flowing = false;
        self.quality = 0.0;
        for name in [LayerName::A, LayerName::B, LayerName::C] {
            self.p.remove_row(&ISDBT_LAYERS, name.key());
        }
        self.publish();
    }

    fn publish(&mut self) {
        let s = &self.sync;
        let p = &mut self.p;
        let w = if s.width_mhz > 0.0 {
            s.width_mhz
        } else {
            self.width.start_mhz()
        };
        let mode_found = s.stage >= 1 && s.mode > 0;
        let tmcc_ok = s.stage == 2;
        let layers = self.fec.layers;
        let any_locked = layers.iter().flatten().any(|(_, c)| c.locked);
        let (mut good, mut bad) = (0u64, 0u64);
        for (_, c) in layers.iter().flatten() {
            good += c.rs_clean + c.rs_corrected;
            bad += c.rs_failed;
        }
        let flowing = any_locked && good > 0;
        let detection = match (s.stage, flowing) {
            (2, true) => 3,
            (2, false) => 2,
            (1, _) => 1,
            _ => 0,
        };
        let snr = if tmcc_ok { self.eq.snr_db } else { 0.0 };
        let band = Band::of(s.segments as usize, Default::default());
        // The layer that carries most data sets the required SNR of the quality.
        let (hg, hb) = self
            .hist
            .iter()
            .fold((0u64, 0u64), |a, x| (a.0 + x.0, a.1 + x.1));
        let mut net = 0.0;
        let tmcc = s.tmcc.as_ref().filter(|_| tmcc_ok);
        match tmcc {
            Some(t) => {
                // The layers decoded: for one segment of a wider signal, layer A alone.
                let cur = t.current.within(s.segments as usize).unwrap_or(t.current);
                let main = cur
                    .layers
                    .iter()
                    .filter(|l| l.used())
                    .max_by(|a, b| {
                        let v = |l: &LayerParams| {
                            let (n, dn) = l.rate();
                            l.segments as f64 * l.bits() as f64 * n as f64 / dn as f64
                        };
                        v(a).total_cmp(&v(b))
                    })
                    .copied();
                if let Some(m) = main {
                    let margin = snr - required_snr(&m);
                    let share = if hg + hb > 0 {
                        hg as f64 / (hg + hb) as f64
                    } else {
                        0.0
                    };
                    let k = if any_locked { share * share } else { 0.5 };
                    let sc = (25.0 + 12.5 * margin).clamp(0.0, 100.0) * k;
                    self.quality += 0.25 * (sc - self.quality);
                }
                for (li, l) in cur.layers.iter().enumerate() {
                    let name = LayerName::of(li);
                    if !l.used() {
                        p.remove_row(&ISDBT_LAYERS, name.key());
                        continue;
                    }
                    let rate = layer_rate(s.mode, s.guard, w, l);
                    net += rate;
                    let c = layers[li].map(|x| x.1).unwrap_or_default();
                    ISDBT_LAYERS.put(
                        p,
                        name.key(),
                        &IsdbtLayerRow {
                            layer: name,
                            partial: li == 0 && cur.partial,
                            segments: l.segments as i64,
                            modulation: modulation_of(l),
                            code_rate: code_rate_of(l),
                            interleave_length: super::layout::ti_length(s.mode, l.interleave)
                                as i64,
                            rate_bit_s: rate,
                            packets: c.packets as i64,
                            rs_clean: c.rs_clean as i64,
                            rs_corrected: c.rs_corrected as i64,
                            rs_failed: c.rs_failed as i64,
                            rs_locked: c.locked,
                            viterbi_margin: c.viterbi_margin as f64,
                        },
                    );
                }
            }
            None => self.quality = 0.0,
        }
        p.put(&IsdbtFields {
            detection,
            width: Hertz(w * 1e6),
            width_auto: s.width_auto || self.width == Width::Auto,
            ifft_rate: Hertz(ifft_rate(w)),
            mode: mode_found.then_some(s.mode as i64),
            guard: mode_found.then(|| Guard::of(s.guard)),
            fft_size: mode_found.then(|| band.fft_size(s.mode) as i64),
            carriers: mode_found.then(|| band.carriers(s.mode) as i64),
            carrier_spacing: mode_found
                .then(|| Hertz(ifft_rate(w) / Band::FULL.fft_size(s.mode) as f64)),
            frame_length: mode_found.then(|| Seconds(frame_seconds(s.mode, s.guard, w))),
            offset_hz: mode_found.then_some(Hertz(s.cfo_hz)),
            clock_ppm: mode_found.then_some(s.clock_ppm),
            snr_db: tmcc.map(|_| snr),
            frames_ok: good as i64,
            frames_bad: bad as i64,
            carrier_shift: mode_found.then_some(s.shift as i64),
            frame_symbol: tmcc_ok.then_some(s.symbol as i64),
            since_tmcc: tmcc.map(|_| Seconds(s.since_tmcc_s)),
            mirrored: s.mirrored,
            system_id: tmcc.map(|t| t.system as i64),
            switch_countdown: tmcc.map(|t| t.countdown as i64),
            emergency_alarm: tmcc.map(|t| t.emergency),
            partial_reception: tmcc.map(|t| t.current.partial),
            packets_ok_pct: if good + bad > 0 {
                100.0 * good as f64 / (good + bad) as f64
            } else {
                0.0
            },
            net_rate_bit_s: net,
            quality_pct: self.quality,
            quality: quality_of(self.quality, tmcc.is_some()),
            step_mode: StepColour::of(mode_found, false),
            step_tmcc: StepColour::of(tmcc_ok, mode_found),
            step_layers: StepColour::of(any_locked, tmcc_ok),
            step_ts: StepColour::of(flowing, false),
        });
        if let Some(note) = self.note {
            p.put(&IsdbtNarrowFields {
                band_segments: s.segments as i64,
                note,
            });
        }
        let r = p.results_mut();
        if flowing != self.flowing {
            r.log(if flowing {
                "transport stream locked (Viterbi + Reed-Solomon)"
            } else {
                "transport stream sync lost"
            });
            self.flowing = flowing;
        }
        let lock_state = match detection {
            3 => LockState::Locked,
            1 | 2 => LockState::Tracking,
            _ => LockState::Searching,
        };
        write_status(
            r,
            &StatusLine {
                lock_state,
                snr_db: tmcc_ok.then_some(snr as f32),
                offset_hz: mode_found.then_some(s.cfo_hz),
                level_dbfs: None,
                summary: &self.summaries[detection.clamp(0, 3) as usize],
            },
        );

        let e = &self.eq;
        for (id, (points, line)) in self.series.0.iter().zip([
            (&e.constellation, false),
            (&e.pilots, false),
            (&e.channel_db, true),
            (&e.channel_phase, true),
            (&e.impulse, true),
        ]) {
            r.update_series(*id, |ser| fill_series(ser, line, points));
        }
    }
}
