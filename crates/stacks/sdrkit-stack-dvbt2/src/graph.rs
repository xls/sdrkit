//! The DVB-T2 components: `dvbt2.receiver`, samples to the cells of the chosen data PLP and its common PLP, and
//! `dvbt2.fec`, those cells to BBFRAMEs, the PLP pair for the mode adaptation of `ipcast.bbframe`, and the results.
//!
//! Standard: ETSI EN 302 755 V1.4.1.

use crate::fec::{IfCells, T2Fec};
use crate::l1::L1Post;
use crate::params::{k_bch, CodeRate, FecFrame, Modulation, Profile};
use crate::results::{Detection, PayloadType, PlpType, TimeIl};
use crate::rx::{RxSettings, RxStatus, Stage, T2Receiver};
use crate::settings::{BANDWIDTH, PLP};
use crate::typed::{Dvbt2Fields, Dvbt2PlpRow, PLPS};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, ConfigError, LockState, Results, ResultsDecl, SettingError, SettingId, SettingSet,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, PacketPool, Port,
    RateFrom, RateRule, TagKind, Timed,
};
use sdrkit_proto_ipcast::components::PlpPair;
use std::fmt::Write;
use std::sync::Arc;

sdrkit_core::settings! {
    /// The settings of [`RxComp`].
    pub enum RxCompSetting {
        /// The channel width.
        Bandwidth = BANDWIDTH,
        /// The data PLP.
        Plp = PLP,
    }
}

/// The PLP setting value of a PLP choice: -1 for the first data PLP.
fn plp_value(plp: Option<u8>) -> i64 {
    plp.map_or(-1, i64::from)
}

/// The PLP choice of a PLP setting value.
fn plp_of(v: i64) -> Option<u8> {
    u8::try_from(v).ok()
}

/// What the receive chain knows of the signal, for the results (the item `dvbt2.state`). The FEC counts of the
/// status are not filled.
#[derive(Clone, Debug, Default)]
pub struct RxState {
    /// The status, with the log lines since the item before.
    pub status: RxStatus,
    /// The L1-post of the last T2-frame, when its PLPs differ from those of the item before. `None` keeps them.
    pub post: Option<Arc<L1Post>>,
}

impl sdrkit_graph::Item for RxState {
    const KIND: &'static str = "dvbt2.state";
}

/// `dvbt2.receiver`: from samples at the elementary rate of the channel width to the equalised cells of each
/// Interleaving Frame of the data PLP and its common PLP ([`IfCells`] on `cells`), and what it knows of the signal
/// ([`RxState`] on `state`) when that changes.
pub struct RxComp {
    set: RxSettings,
    rx: Option<T2Receiver>,
    seen: (u64, Stage, u64),
    cells: Vec<IfCells>,
    /// The L1-post that the last [`RxState`] with one carried.
    told: Option<Arc<L1Post>>,
}

impl Default for RxComp {
    fn default() -> Self {
        RxComp::new(RxSettings::default())
    }
}

impl RxComp {
    /// A component with these settings.
    pub fn new(set: RxSettings) -> RxComp {
        RxComp {
            set,
            rx: None,
            seen: (0, Stage::Searching, 0),
            cells: Vec::new(),
            told: None,
        }
    }
}

impl Component for RxComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt2.receiver",
            vec![Port::iq("in").rate(RateRule::Exact(self.set.bandwidth.rate()))],
            vec![
                Port::typed::<IfCells>("cells").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<RxState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(RxCompSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let need = self.set.bandwidth.rate();
        let got = inputs.first().map_or(0.0, |f| f.rate);
        if got != need {
            return Err(ConfigError::WrongRate { need, got });
        }
        self.rx = Some(T2Receiver::deferred(self.set));
        self.seen = (0, Stage::Searching, 0);
        self.told = None;
        Ok(vec![
            Format::new(Kind::typed::<IfCells>(), 0.0),
            Format::new(Kind::typed::<RxState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let Some(rx) = self.rx.as_mut() else { return };
        let meter = io.meter();
        {
            let _timer = meter.section("ofdm");
            rx.feed(io.iq(0), &mut |_| {});
        }
        rx.take_cells(&mut self.cells);
        let out = io.items_out::<IfCells>(0);
        for c in self.cells.drain(..) {
            out.push(Timed {
                time: c.time,
                value: c,
            });
        }
        let st = rx.status();
        let now = (st.frames, st.stage, st.p1_count);
        if now != self.seen || !st.log.is_empty() {
            self.seen = now;
            let time = io.input(0).first() as f64 / io.input_format(0).rate;
            // The results read the PLPs of the L1-post and their blocks in the frame.
            let same =
                |a: &L1Post, b: &L1Post| a.config == b.config && a.dynamic.plps == b.dynamic.plps;
            let post = match (rx.post(), self.told.as_deref()) {
                (Some(now), Some(told)) if same(now, told) => None,
                (Some(now), _) => {
                    self.told = Some(Arc::new(now.clone()));
                    self.told.clone()
                }
                (None, _) => None,
            };
            let mut status = rx.status().clone();
            status.log = rx.take_log();
            let value = RxState { status, post };
            io.items_out::<RxState>(1).push(Timed { time, value });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RxCompSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RxCompSetting::Bandwidth => {
                self.set.bandwidth = BANDWIDTH.read(id, v)?;
                Ok(Applied::Reformat)
            }
            RxCompSetting::Plp => {
                self.set.plp = plp_of(PLP.read(id, v)?);
                if let Some(rx) = self.rx.as_mut() {
                    rx.set_plp(self.set.plp);
                }
                Ok(Applied::Live)
            }
        }
    }

    fn values(&self) -> Values {
        values_of([
            (
                RxCompSetting::Bandwidth,
                Value::Choice(self.set.bandwidth.index()),
            ),
            (RxCompSetting::Plp, Value::I64(plp_value(self.set.plp))),
        ])
    }

    /// The signal belongs to the multiplex of the frequency before.
    fn forget(&mut self, _: &Kept) {
        if self.rx.is_some() {
            self.rx = Some(T2Receiver::deferred(self.set));
        }
        self.seen = (0, Stage::Searching, 0);
        self.told = None;
    }
}

/// `dvbt2.fec`: from the cells of each Interleaving Frame ([`IfCells`] on `cells`) to the descrambled BBFRAMEs (one
/// per `Bytes` packet) of the data PLP and its common PLP, and a [`PlpPair`] on `plps` when either changes. It
/// publishes the results of the stack from the [`RxState`] on `state` and its own counts. A BBFRAME that the BCH
/// decoder refuses is left out, and the next one carries a `Discontinuity` tag.
pub struct FecComp {
    max_iter: u32,
    fec: T2Fec,
    pool: PacketPool,
    lost: bool,
    pair: PlpPair,
    status: RxStatus,
    post: Option<Arc<L1Post>>,
    counted: crate::fec::FecCounts,
    report: Report,
    workers: Option<sdrkit_graph::Pool>,
}

impl Default for FecComp {
    fn default() -> Self {
        FecComp::new(RxSettings::default().max_iter)
    }
}

impl FecComp {
    /// A component whose decoders run at most `max_iter` LDPC iterations.
    pub fn new(max_iter: u32) -> FecComp {
        FecComp {
            max_iter,
            fec: T2Fec::new(max_iter),
            pool: PacketPool::default(),
            lost: false,
            pair: PlpPair::default(),
            status: RxStatus::default(),
            post: None,
            counted: crate::fec::FecCounts::default(),
            report: Report::new(),
            workers: None,
        }
    }

    /// Starts again from the empty state.
    fn clear(&mut self) {
        self.fec = T2Fec::new(self.max_iter);
        self.lost = false;
        self.pair = PlpPair::default();
        self.status = RxStatus::default();
        self.post = None;
        self.counted = crate::fec::FecCounts::default();
        self.report.publish(&RxStatus::default(), None);
    }
}

impl Component for FecComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt2.fec",
            vec![
                Port::typed::<IfCells>("cells"),
                Port::typed::<RxState>("state"),
            ],
            vec![
                Port::bytes("bbframes").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<PlpPair>("plps").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.clear();
        Ok(vec![
            Format::new(Kind::Bytes, 0.0),
            Format::new(Kind::typed::<PlpPair>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &sdrkit_graph::RunContext) {
        self.workers = Some(ctx.pool.clone());
    }

    fn process(&mut self, io: &mut Io) {
        let meter = io.meter();
        let (pool, lost) = (&mut self.pool, &mut self.lost);
        let mut frames = std::mem::take(io.bytes_out(0));
        let mut pairs = std::mem::take(io.items_out::<PlpPair>(1));
        self.fec.counts.start_mean();
        for c in io.items::<IfCells>(0) {
            if c.value.pair != self.pair {
                self.pair = c.value.pair;
                pairs.push(Timed {
                    time: c.time,
                    value: self.pair,
                });
            }
            let _timer = meter.section("fec");
            self.fec.decode(&c.value, self.workers.as_ref(), &mut |b| {
                if !b.ok {
                    *lost = true;
                    return;
                }
                if std::mem::take(lost) {
                    io.output(0)
                        .tag_here(frames.len(), TagKind::Discontinuity { lost: 0 });
                }
                frames.push(pool.make(b.bytes.len(), b.time, |d| d.copy_from_slice(b.bytes)));
            });
        }
        *io.bytes_out(0) = frames;
        let mut told = false;
        for s in io.items::<RxState>(1) {
            told = true;
            let state = &s.value;
            for line in &state.status.log {
                self.report.p.results_mut().log(line.clone());
            }
            let iterations = self.status.iterations;
            self.status.clone_from(&state.status);
            self.status.log.clear();
            self.status.iterations = iterations;
            if state.post.is_some() {
                self.post.clone_from(&state.post);
            }
            let pair = PlpPair {
                plp: state.status.plp,
                common: state.status.common,
            };
            if pair != self.pair {
                self.pair = pair;
                pairs.push(Timed {
                    time: s.time,
                    value: pair,
                });
            }
        }
        *io.items_out::<PlpPair>(1) = pairs;
        let n = self.fec.counts;
        if told || n != self.counted {
            self.counted = n;
            (self.status.blocks_ok, self.status.blocks_bad) = (n.ok, n.bad);
            self.status.bch_corrected = n.corrected;
            if let Some(mean) = n.mean_iterations() {
                self.status.iterations = mean;
            }
            self.report.publish(&self.status, self.post.as_deref());
        }
    }

    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }

    /// The PLPs and every count belong to the multiplex of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.clear();
    }

    fn results(&self) -> Option<&Results> {
        Some(self.report.p.results())
    }
}

/// The results of the receiver, written from its status. Each write changes only what changed.
struct Report {
    p: Published,
    /// The row key being written.
    key: String,
    /// The PLPs that have a row.
    rows: [bool; 256],
    /// What the status line was written from.
    summary_of: Option<(Stage, Option<crate::pilots::FrameShape>, Option<u8>)>,
    summary: String,
}

impl Report {
    fn new() -> Report {
        Report {
            p: Published::from_decl(Arc::new(
                ResultsDecl::new()
                    .with_fields(Dvbt2Fields::SHAPE.fields)
                    .with_table(&PLPS),
            )),
            key: String::with_capacity(4),
            rows: [false; 256],
            summary_of: None,
            summary: String::with_capacity(48),
        }
    }

    fn publish(&mut self, st: &RxStatus, post: Option<&L1Post>) {
        let flowing = st.stage == Stage::Locked && st.blocks_ok > 0;
        let shape = st.shape;
        let lite = st.profile == Some(Profile::Lite);
        let pre = st.pre.as_ref();
        let frame_s = st.frame_length_s;
        let mut net = None;
        let mut now = [false; 256];
        if let Some(post) = post {
            for c in &post.config.plps {
                let blocks = post
                    .dynamic
                    .plps
                    .iter()
                    .find(|d| d.plp_id == c.plp_id)
                    .map_or(0, |d| d.num_blocks);
                let rate = CodeRate::from_code(c.cod, lite);
                let frame = FecFrame::from_code(c.fec_type);
                if Some(c.plp_id) == st.plp {
                    if let (Some(r), Some(fr), Some(t)) = (rate, frame, frame_s) {
                        let bits =
                            k_bch(fr, r).unwrap_or(80).saturating_sub(80) * usize::from(blocks);
                        net = Some(bits as f64 / c.p_i() as f64 / t);
                    }
                }
                now[usize::from(c.plp_id)] = true;
                self.key.clear();
                // Writing to a `String` cannot fail.
                let _ = write!(self.key, "{}", c.plp_id);
                let row = Dvbt2PlpRow {
                    plp_id: i64::from(c.plp_id),
                    plp_type: PlpType::of(c.plp_type),
                    payload: PayloadType::of(c.payload_type),
                    modulation: Modulation::from_plp_code(c.modulation),
                    code_rate: rate,
                    fec_frame_bits: frame.map(|f| f.n_ldpc() as i64),
                    rotated: c.rotation,
                    fec_blocks: i64::from(blocks),
                    group_id: i64::from(c.group_id),
                    time_interleaving: match (c.time_il_type, c.time_il_length) {
                        (_, 0) => TimeIl::Off,
                        (false, _) => TimeIl::Blocks,
                        (true, _) => TimeIl::Frames,
                    },
                    time_il_length: i64::from(c.time_il_length),
                };
                PLPS.put(&mut self.p, &self.key, &row);
            }
        }
        let p = &mut self.p;
        for (id, (&was, &is)) in self.rows.iter().zip(&now).enumerate() {
            if was && !is {
                self.key.clear();
                // Writing to a `String` cannot fail.
                let _ = write!(self.key, "{id}");
                p.remove_row(&PLPS, &self.key);
            }
        }
        self.rows = now;
        let offset = (st.stage != Stage::Searching).then_some(st.offset_hz);
        p.put(&Dvbt2Fields {
            detection: match (st.stage, flowing) {
                (Stage::Locked, true) => Detection::Flowing,
                (Stage::Locked, false) => Detection::L1,
                (Stage::P1, _) => Detection::P1,
                (Stage::Searching, _) => Detection::Searching,
            },
            fft: shape.map(|s| s.fft),
            guard: shape.map(|s| s.guard),
            pilot_pattern: shape.map(|s| s.pp),
            extended_carriers: shape.map(|s| s.extended),
            miso: shape.map(|s| s.miso),
            profile: st.profile,
            l1_modulation: pre.and_then(|pre| Modulation::from_l1_code(pre.l1_mod)),
            cell_id: pre.map(|pre| i64::from(pre.cell_id)),
            network_id: pre.map(|pre| i64::from(pre.network_id)),
            t2_system_id: pre.map(|pre| i64::from(pre.t2_system_id)),
            frames_per_superframe: pre.map(|pre| i64::from(pre.num_t2_frames)),
            data_symbols: shape.map(|s| s.l_data as i64),
            frame_length: frame_s.map(Seconds),
            t2_frames: count(st.frames),
            l1_errors: count(st.l1_errors),
            plp: st.plp.map(i64::from),
            common_plp: st.common.map(i64::from),
            ldpc_iterations: f64::from(st.iterations),
            net_rate_bit_s: net,
            problem: st.problem,
            offset_hz: offset.map(Hertz),
            snr_db: st.snr_db.map(f64::from),
            frames_ok: count(st.blocks_ok),
            frames_bad: count(st.blocks_bad),
            bits_corrected: count(st.bch_corrected),
        });
        let of = (st.stage, shape, st.plp);
        if self.summary_of != Some(of) {
            self.summary_of = Some(of);
            self.summary.clear();
            // Writing to a `String` cannot fail.
            let _ = write_summary(&mut self.summary, st.stage, shape, st.plp);
        }
        let lock_state = match (st.stage, flowing) {
            (Stage::Locked, true) => LockState::Locked,
            (Stage::Searching, _) => LockState::Searching,
            _ => LockState::Tracking,
        };
        let summary = &self.summary;
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock_state
                || s.snr_db != st.snr_db
                || s.offset_hz != offset
                || s.summary != *summary;
            if changed {
                s.lock_state = lock_state;
                s.snr_db = st.snr_db;
                s.offset_hz = offset;
                s.summary.clone_from(summary);
            }
            changed
        });
    }
}

/// Writes the status line of the receiver.
fn write_summary(
    out: &mut String,
    stage: Stage,
    shape: Option<crate::pilots::FrameShape>,
    plp: Option<u8>,
) -> std::fmt::Result {
    match (stage, shape) {
        (Stage::Locked, Some(s)) => {
            write!(
                out,
                "DVB-T2 {} {} {}",
                s.fft.name(),
                s.guard.name(),
                s.pp.name()
            )?;
            match plp {
                Some(id) => write!(out, ", PLP {id}"),
                None => Ok(()),
            }
        }
        (Stage::P1, _) => out.write_str("DVB-T2 P1 found"),
        _ => out.write_str("searching for DVB-T2"),
    }
}

/// A counter as a field value.
fn count(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}
