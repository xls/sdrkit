//! The two components of the DVB-C2 stack.
//!
//! `dvbc2.receiver`: samples in; out the equalised cells of the data slices for the decoding stage (`cells`), its
//! measurements at each clock tick (`report`), and the request to tune to the tuning position of a slice that the samples do
//! not hold (`requests`).
//!
//! `dvbc2.fec`, the stage `fec` of the stack: the cells in; out the descrambled BBFRAMEs (`bbframes`, one per `Bytes` packet,
//! MATYPE-2 naming the PLP) of the chosen PLP when it carries a transport stream or GSE and of the common PLP of its group,
//! the generic stream of the chosen PLP (`generic`): the user packets of a GFPS or the data fields of a GCS, and the pair of
//! the chosen PLP and its common PLP for the merge of `ipcast.bbframe` (`plps`), sent before the first BBFRAME of a new pair
//! (the BBFRAMEs of the pair before that the same pass decoded are dropped). It publishes the
//! results of the stack: the measurements and counters, the L1 fields, and the tables of data slices, PLPs and notches.

use super::decode::{BbFrame, C2Cells, C2Decoder};
use super::l1::L1Part2;
use super::rx::{C2Receiver, RxConfig, State, Status};
use super::settings::*;
use super::tables::{CodeRate, ModCod, Qam};
use sdrkit_proto_dvbs2::receive::write_status;
use sdrkit_proto_ipcast::adapt::UpDeframer;
use sdrkit_proto_ipcast::bbheader::{BbHeader, BBHEADER_LEN};
use sdrkit_proto_ipcast::components::PlpPair;
use crate::typed::{
    Dvbc2Fields, Dvbc2NotchRow, Dvbc2PlpRow, Dvbc2SliceRow, DVBC2_NOTCHES, DVBC2_PLPS, DVBC2_SLICES,
};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, ConfigError, HostRequest, LockState, Results, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Packet, PacketPool, Port, RateFrom, TagKind,
    Timed,
};
use std::fmt::Write as _;

/// The lowest input rate of any raster (the 6 MHz raster; the 8 MHz raster needs 8 Msps).
pub const MIN_RATE: f64 = 6e6;

sdrkit_core::choice_enum! {
    /// DSLICE_TYPE.
    pub enum SliceKind {
        /// Type 1.
        Type1 = ("type1", "Type 1"),
        /// Type 2.
        Type2 = ("type2", "Type 2"),
    }
}

sdrkit_core::choice_enum! {
    /// FEC_HEADER_TYPE of a type 2 slice.
    pub enum HeaderKind {
        /// No FECFrame header (type 1).
        None = ("none", "None"),
        /// Robust: 32 QPSK cells.
        Robust = ("robust", "Robust"),
        /// High efficiency: 16 16-QAM cells.
        Efficient = ("efficient", "High efficiency"),
    }
}

sdrkit_core::choice_enum! {
    /// PLP_TYPE.
    pub enum PlpType {
        /// Common PLP.
        Common = ("common", "Common"),
        /// Grouped data PLP.
        Grouped = ("grouped", "Grouped data"),
        /// Normal data PLP.
        Normal = ("normal", "Normal data"),
        /// A reserved value.
        Reserved = ("reserved", "Reserved"),
    }
}

sdrkit_core::choice_enum! {
    /// PLP_PAYLOAD_TYPE.
    pub enum PayloadType {
        /// Generic fixed-length packetized stream.
        Gfps = ("gfps", "GFPS"),
        /// Generic continuous stream.
        Gcs = ("gcs", "GCS"),
        /// Generic stream encapsulation.
        Gse = ("gse", "GSE"),
        /// Transport stream.
        Ts = ("ts", "TS"),
        /// A reserved value.
        Reserved = ("reserved", "Reserved"),
    }
}

/// The receiver's settings as a configuration (without the rate and centre, which come from the input).
fn config_of(v: &Values) -> RxConfig {
    let opt = |x: i64| u8::try_from(x).ok();
    RxConfig {
        plp: opt(v.get(&PLP)),
        slice: opt(v.get(&SLICE)),
        centre_known: v.get(&CENTRE_KNOWN),
        raster: v.get(&RASTER),
        iterations: v.get(&LDPC_ITERATIONS) as u32,
        timing_metric: v.get(&TIMING_METRIC) as f32,
        preamble_ratio: v.get(&PREAMBLE_RATIO) as f32,
        index_metric: v.get(&INDEX_METRIC) as f32,
        index_span: v.get(&INDEX_SPAN) as usize,
        packets: super::packets::Thresholds {
            candidate: v.get(&HEADER_CANDIDATE) as f32,
            agree: v.get(&HEADER_AGREE) as u32,
            header_misses: v.get(&HEADER_MISSES) as u32,
            block_misses: v.get(&BLOCK_MISSES) as u32,
        },
        preamble_misses: v.get(&PREAMBLE_MISSES) as u32,
        ti_full_width: v.get(&TI_FULL_WIDTH),
        bundle_hold: v.get(&BUNDLE_HOLD) as usize,
        w_rm: v.get(&RM_SCRAMBLING) as u32,
        ..RxConfig::default()
    }
}


/// What `dvbc2.receiver` reports at each clock tick: its measurements, the carrier spacing of its raster and its log lines.
#[derive(Clone, Debug, Default)]
pub struct C2Report {
    /// The measurements and counters of the front.
    pub status: Status,
    /// The carrier spacing, Hz.
    pub spacing: f64,
    /// The log lines since the last report.
    pub log: Vec<String>,
}

impl Item for C2Report {
    const KIND: &'static str = "dvbc2.report";
}

/// `dvbc2.receiver`.
pub struct C2Comp {
    values: Values,
    rx: Option<C2Receiver>,
    /// The frequency of the last tuning request.
    requested: Option<f64>,
}

impl Default for C2Comp {
    fn default() -> Self {
        C2Comp {
            values: Values::defaults(&RxSetting::schema()),
            rx: None,
            requested: None,
        }
    }
}

impl C2Comp {
    /// Sends the measurements and log lines of the front to `dvbc2.fec`.
    fn report(&mut self, io: &mut Io) {
        let Some(rx) = self.rx.as_mut() else { return };
        let value = C2Report {
            log: rx.take_log(),
            status: rx.status.clone(),
            spacing: rx.config().raster.spacing(),
        };
        io.items_out::<C2Report>(1).push(Timed { time: 0.0, value });
    }
}

impl Component for C2Comp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbc2.receiver",
            vec![Port::iq("in")],
            vec![
                Port::typed::<C2Cells>("cells").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<C2Report>("report").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(RxSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let need = self.values.get(&RASTER).min_rate();
        if f.rate < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: f.rate,
            });
        }
        let cfg = RxConfig {
            rate_hz: f.rate,
            centre_hz: f.centre_hz,
            ..config_of(&self.values)
        };
        self.rx = Some(C2Receiver::new(cfg));
        self.requested = None;
        Ok(vec![
            Format::new(Kind::typed::<C2Cells>(), 0.0),
            Format::new(Kind::typed::<C2Report>(), 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let Some(rx) = self.rx.as_mut() else { return };
        let x = io.iq(0);
        rx.feed(x, io.items_out::<C2Cells>(0));
        // One request for each tuning position that the samples do not hold, inside the range of the stack.
        let retune = rx.status.retune_hz;
        if retune != self.requested {
            self.requested = retune;
            let t = &super::DVBC2.tuning;
            if let Some(hz) = retune.filter(|hz| (t.min_hz..=t.max_hz).contains(hz)) {
                let value = HostRequest::retune(hz, "DVB-C2 tuning position of the data slice");
                io.items_out::<HostRequest>(2).push(Timed { time: 0.0, value });
            }
        }
        if io.tick() {
            self.report(io);
        }
    }

    fn finish(&mut self, io: &mut Io) {
        self.report(io);
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        s.check(v)?;
        self.values.set(id, v).map_err(|_| SettingError::NoSuchId { id })?;
        let next = config_of(&self.values);
        if let Some(rx) = self.rx.as_mut() {
            match s {
                RxSetting::Plp | RxSetting::Slice => rx.set_choice(next.plp, next.slice),
                RxSetting::LdpcIterations => rx.set_iterations(next.iterations),
                _ => {
                    let c = rx.config();
                    *rx = C2Receiver::new(RxConfig {
                        rate_hz: c.rate_hz,
                        centre_hz: c.centre_hz,
                        ..next
                    });
                }
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        self.values.clone()
    }
}

/// The rows of the L1 tables now published, by key.
#[derive(Default)]
struct Rows {
    key: String,
    slices_now: Vec<String>,
    plps_now: Vec<String>,
    notches_now: usize,
}

/// `dvbc2.fec`.
pub struct C2Fec {
    dec: C2Decoder,
    /// The last report of `dvbc2.receiver`.
    front: C2Report,
    pool: PacketPool,
    lost: bool,
    /// The pair of PLPs last sent to the merge.
    pair: PlpPair,
    p: Published,
    rows: Rows,
    summary: String,
    l1_seen: Option<L1Part2>,
    /// The chosen slice and PLP, the window centre and the number of codings read from headers that the tables show.
    view_seen: (Option<(u8, u8)>, Option<usize>, usize),
    last_ok: u64,
    gfps: UpDeframer,
    generic_packets: u64,
    generic_bytes: u64,
}

impl Default for C2Fec {
    fn default() -> Self {
        C2Fec {
            dec: C2Decoder::default(),
            front: C2Report {
                status: Status {
                    state: Some(State::Searching),
                    ..Status::default()
                },
                spacing: RxConfig::default().raster.spacing(),
                log: Vec::new(),
            },
            pool: PacketPool::default(),
            lost: false,
            pair: PlpPair::default(),
            p: Published::new(Dvbc2Fields::SHAPE.fields).with_tables(&[
                DVBC2_SLICES.def(),
                DVBC2_PLPS.def(),
                DVBC2_NOTCHES.def(),
            ]),
            rows: Rows {
                key: String::with_capacity(8),
                ..Rows::default()
            },
            summary: String::with_capacity(96),
            l1_seen: None,
            view_seen: (None, None, 0),
            last_ok: 0,
            gfps: UpDeframer::new(),
            generic_packets: 0,
            generic_bytes: 0,
        }
    }
}

impl C2Fec {
    fn publish(&mut self) {
        let spacing = self.front.spacing;
        let st: &Status = &self.front.status;
        let ds = &self.dec.status;
        let p = &mut self.p;
        let state = st.state.unwrap_or(State::Searching);
        let l1 = st.l1.as_ref();
        let hz = |carrier: f64| Hertz(carrier * spacing);
        let count = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
        p.put(&Dvbc2Fields {
            state,
            guard: st.guard,
            offset_hz: (state != State::Searching).then_some(Hertz(st.cfo_hz)),
            clock_ppm: st.clock_ppm,
            window_carrier: st.window_carrier.map(|k| k as i64),
            window_hz: st.window_carrier.map(|k| hz(k as f64)),
            esn0_db: st.esn0_db,
            mer_db: ds.mer_db,
            network_id: l1.map(|l| i64::from(l.network_id)),
            system_id: l1.map(|l| i64::from(l.system_id)),
            start_frequency_hz: l1.map(|l| hz(l.k_min() as f64)),
            bandwidth_hz: l1.map(|l| hz((l.k_max() - l.k_min()) as f64)),
            change_counter: l1.map(|l| i64::from(l.change_counter)),
            ews: l1.map(|l| l.ews),
            c2_version: l1.map(|l| i64::from(l.version)),
            reserved_tones: l1.map(|l| l.reserved_tones),
            blocks_ok: count(ds.blocks_ok),
            blocks_bad: count(ds.blocks_bad),
            l1_bad: count(st.l1_bad),
            sync_losses: count(st.sync_losses + ds.sync_losses),
            pre_ber: ds.pre_ber,
            mean_iterations: ds.iterations,
            c2_frames: count(st.frames),
            chosen_slice: st.chosen.map(|c| i64::from(c.0)),
            chosen_plp: st.chosen.map(|c| i64::from(c.1)),
            common_plp: st.common.map(i64::from),
            bundled_slices: (st.bundled_slices > 1).then_some(st.bundled_slices as i64),
            retune_hz: st.retune_hz.map(Hertz),
            outside_slice: st.outside.map(|o| i64::from(o.0)),
            outside_hz: st.outside.map(|o| Hertz(o.1)),
            generic_packets: count(self.generic_packets),
            generic_bytes: count(self.generic_bytes),
            note: super::NOTE,
        });
        let view = (st.chosen, st.window_carrier, ds.codings.len());
        if st.l1.as_ref() != self.l1_seen.as_ref() || view != self.view_seen || !ds.codings.is_empty() {
            Self::tables(p, &mut self.rows, st, &ds.codings, spacing);
            self.l1_seen = st.l1.clone();
            self.view_seen = view;
        }
        let flowing = ds.blocks_ok > self.last_ok;
        self.last_ok = ds.blocks_ok;
        let lock = match state {
            State::Data if flowing => LockState::Locked,
            State::Searching => LockState::Searching,
            _ => LockState::Tracking,
        };
        let s = &mut self.summary;
        s.clear();
        let _ = write!(s, "DVB-C2 {}", state.label());
        if let Some(g) = st.guard {
            let _ = write!(s, ", guard {}", g.label());
        }
        if let Some((sl, plp)) = st.chosen {
            let _ = write!(s, ", slice {sl} PLP {plp}");
        }
        if let Some(m) = ds.mer_db {
            let _ = write!(s, ", MER {m:.1} dB");
        }
        let _ = write!(s, ", blocks {} good {} bad", ds.blocks_ok, ds.blocks_bad);
        write_status(
            p.results_mut(),
            lock,
            ds.mer_db.map(|m| m as f32),
            None,
            (state != State::Searching).then_some(st.cfo_hz),
            &self.summary,
        );
    }

    /// Puts the rows of the L1 of `st` into the tables and removes the rows of slices, PLPs and notches it no longer has.
    fn tables(p: &mut Published, rows: &mut Rows, st: &Status, codings: &[(u8, ModCod)], spacing: f64) {
        let Rows { key, slices_now, plps_now, notches_now } = rows;
        let Some(l1) = st.l1.as_ref() else { return };
        let dx = l1.guard.d_x();
        let start = l1.k_min();
        let window = st
            .window_carrier
            .map(|k| (k.saturating_sub(super::rx::HALF_WINDOW), k + super::rx::HALF_WINDOW));
        let mut slices = Vec::new();
        let mut plps = Vec::new();
        for s in &l1.slices {
            key.clear();
            let _ = write!(key, "{}", s.id);
            slices.push(key.clone());
            let ds = s.carriers(start, dx);
            let row = Dvbc2SliceRow {
                slice_id: i64::from(s.id),
                slice_type: if s.type2 { SliceKind::Type2 } else { SliceKind::Type1 },
                header_type: match (s.type2, s.efficient_header) {
                    (false, _) => HeaderKind::None,
                    (true, false) => HeaderKind::Robust,
                    (true, true) => HeaderKind::Efficient,
                },
                tune_hz: Hertz(s.tune_carrier(start, dx) as f64 * spacing),
                start_hz: ds.map(|d| Hertz(d.0 as f64 * spacing)),
                end_hz: ds.map(|d| Hertz(d.1 as f64 * spacing)),
                ti_depth: s.depth() as i64,
                plp_count: s.plps.len() as i64,
                in_window: matches!((ds, window), (Some(d), Some(w)) if d.0 >= w.0 && d.1 <= w.1),
                width_hz: ds.map(|d| Hertz((d.1 - d.0 + 1) as f64 * spacing)),
            };
            DVBC2_SLICES.put(p, key, &row);
            for q in &s.plps {
                key.clear();
                let _ = write!(key, "{}/{}", s.id, q.id);
                plps.push(key.clone());
                // A type 2 slice names the coding in each header: the last one seen for this PLP.
                let (long, qam, rate) = if s.type2 {
                    match codings.iter().find(|h| h.0 == q.id) {
                        Some((_, m)) => (Some(m.long), Some(m.qam), Some(m.rate)),
                        None => (None, None, None),
                    }
                } else {
                    (Some(q.long), Qam::from_code(q.plp_mod), CodeRate::from_code(q.plp_cod))
                };
                let row = Dvbc2PlpRow {
                    plp_slice: i64::from(s.id),
                    plp_id: i64::from(q.id),
                    plp_type: match q.plp_type {
                        0 => PlpType::Common,
                        1 => PlpType::Grouped,
                        2 => PlpType::Normal,
                        _ => PlpType::Reserved,
                    },
                    payload: match q.payload {
                        0 => PayloadType::Gfps,
                        1 => PayloadType::Gcs,
                        2 => PayloadType::Gse,
                        3 => PayloadType::Ts,
                        _ => PayloadType::Reserved,
                    },
                    fec_frame_bits: long.map(|l| if l { 64_800 } else { 16_200 }),
                    modulation: qam,
                    code_rate: rate,
                    ts_id: (!q.psi_reprocessing).then_some(i64::from(q.ts_id)),
                    on_id: (!q.psi_reprocessing).then_some(i64::from(q.on_id)),
                    chosen: st.chosen == Some((s.id, q.id)),
                    group: (q.plp_type <= 1).then_some(i64::from(q.group)),
                    bundled: q.bundled,
                };
                DVBC2_PLPS.put(p, key, &row);
            }
        }
        for old in slices_now.iter().filter(|k| !slices.contains(k)) {
            p.remove_row(DVBC2_SLICES.def(), old);
        }
        for old in plps_now.iter().filter(|k| !plps.contains(k)) {
            p.remove_row(DVBC2_PLPS.def(), old);
        }
        *slices_now = slices;
        *plps_now = plps;
        for (i, n) in l1.notches.iter().enumerate() {
            key.clear();
            let _ = write!(key, "{i}");
            let c = n.carriers(start, dx);
            let row = Dvbc2NotchRow {
                notch_start_hz: c.map(|c| Hertz(c.0 as f64 * spacing)),
                notch_end_hz: c.map(|c| Hertz(c.1 as f64 * spacing)),
            };
            DVBC2_NOTCHES.put(p, key, &row);
        }
        for i in l1.notches.len()..*notches_now {
            key.clear();
            let _ = write!(key, "{i}");
            p.remove_row(DVBC2_NOTCHES.def(), key);
        }
        *notches_now = l1.notches.len();
    }

    /// Decodes the cells of the pass and, at the end of the stream, sends out the frames still held. A new data path, a new
    /// receiver and a loss of the signal break the stream: the next packet has a `Discontinuity` tag.
    fn run(&mut self, io: &mut Io, finish: bool) {
        let C2Fec { dec, pool, lost, pair, gfps, generic_packets, generic_bytes, .. } = self;
        let items = io.items::<C2Cells>(0);
        let mut pass = PassOut {
            frames: std::mem::take(io.bytes_out(0)),
            generic: std::mem::take(io.bytes_out(1)),
            tags: Vec::new(),
            pool,
            lost,
            gfps,
            packets: generic_packets,
            bytes: generic_bytes,
        };
        let mut pairs = Vec::new();
        for it in items {
            match &it.value {
                C2Cells::Path(p) => {
                    let next = PlpPair { plp: Some(p.plp), common: p.common };
                    if next != *pair {
                        *pair = next;
                        // What the pass decoded so far belongs to the pair before.
                        pass.frames.clear();
                        pass.generic.clear();
                        pass.tags.clear();
                        pairs.push(Timed { time: it.time, value: next });
                    }
                    pass.broken();
                }
                C2Cells::Reset | C2Cells::Lost => pass.broken(),
                _ => {}
            }
            dec.push(&it.value, it.time, &mut |f| pass.take(f));
        }
        if finish {
            dec.finish(&mut |f| pass.take(f));
        }
        let PassOut { frames, generic, tags, .. } = pass;
        *io.bytes_out(0) = frames;
        *io.bytes_out(1) = generic;
        io.items_out::<PlpPair>(2).append(&mut pairs);
        for (port, at) in tags {
            io.output(port).tag_here(at, TagKind::Discontinuity { lost: 0 });
        }
        for r in io.items::<C2Report>(1) {
            for l in &r.value.log {
                self.p.results_mut().log(l.clone());
            }
            self.front.status.clone_from(&r.value.status);
            self.front.spacing = r.value.spacing;
        }
    }
}

/// The output packets of one pass of `dvbc2.fec`: the buffers of its ports `bbframes` and `generic`, and the places of
/// their loss tags. It holds no result value.
struct PassOut<'a> {
    frames: Vec<Packet>,
    generic: Vec<Packet>,
    /// (output, packet index) of each `Discontinuity` tag.
    tags: Vec<(usize, usize)>,
    pool: &'a mut PacketPool,
    lost: &'a mut bool,
    gfps: &'a mut UpDeframer,
    packets: &'a mut u64,
    bytes: &'a mut u64,
}

impl PassOut<'_> {
    /// The stream broke here: the next packet is tagged.
    fn broken(&mut self) {
        *self.lost = true;
        self.gfps.reset();
    }

    /// Takes one BBFRAME: a transport stream or GSE frame goes to `frames`, the user packets of a GFPS and the data field of
    /// a GCS go to `generic`.
    fn take(&mut self, f: BbFrame<'_>) {
        if !f.ok {
            self.broken();
            return;
        }
        let copy = |d: &mut [u8]| {
            d.copy_from_slice(f.bytes);
            // MATYPE-2 names the PLP, so that the mode adaptation follows the PLP setting also for a single input
            // stream (whose MATYPE-2 carries no PLP).
            if let (Ok(mut h), Some(head)) = (BbHeader::parse(d), d.first_chunk_mut::<BBHEADER_LEN>()) {
                h.stream_id = f.plp;
                h.write(head);
            }
        };
        // A loss is tagged on the output that the next packet goes to.
        let generic_out = f.payload <= 1 && !f.common;
        if std::mem::take(self.lost) {
            self.tags.push(if generic_out { (1, self.generic.len()) } else { (0, self.frames.len()) });
        }
        let (pool, generic) = (&mut *self.pool, &mut self.generic);
        match f.payload {
            // The common PLP carries a transport stream.
            _ if f.common => self.frames.push(pool.make(f.bytes.len(), f.time, copy)),
            // GFPS: the user packets, their sync byte back in place.
            0 => {
                let (gp, gb) = (&mut *self.packets, &mut *self.bytes);
                let _ = self.gfps.push_frame(f.bytes, &mut |u| {
                    *gp += 1;
                    *gb += u.bytes.len() as u64;
                    generic.push(pool.make(u.bytes.len(), f.time, |d| d.copy_from_slice(u.bytes)));
                });
            }
            // GCS: the data field.
            1 => {
                let df = BbHeader::parse(f.bytes)
                    .ok()
                    .and_then(|h| h.data_field_len().ok())
                    .and_then(|n| f.bytes.get(BBHEADER_LEN..BBHEADER_LEN + n));
                if let Some(df) = df {
                    *self.packets += 1;
                    *self.bytes += df.len() as u64;
                    generic.push(pool.make(df.len(), f.time, |d| d.copy_from_slice(df)));
                }
            }
            _ => self.frames.push(pool.make(f.bytes.len(), f.time, copy)),
        }
    }
}

impl Component for C2Fec {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbc2.fec",
            vec![Port::typed::<C2Cells>("in"), Port::typed::<C2Report>("report")],
            vec![
                Port::bytes("bbframes").rate_from(RateFrom::Fixed(0.0)),
                Port::bytes("generic").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<PlpPair>("plps").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dec = C2Decoder::default();
        self.front = C2Fec::default().front;
        self.lost = false;
        self.pair = PlpPair::default();
        self.l1_seen = None;
        self.view_seen = (None, None, 0);
        self.gfps.reset();
        self.publish();
        Ok(vec![
            Format::new(Kind::Bytes, 0.0),
            Format::new(Kind::Bytes, 0.0),
            Format::new(Kind::typed::<PlpPair>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        self.run(io, false);
        if io.tick() {
            self.publish();
        }
    }

    fn finish(&mut self, io: &mut Io) {
        self.run(io, true);
        self.publish();
    }

    fn values(&self) -> Values {
        no_values()
    }

    /// The L1 tables are of the C2 system of the frequency before.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        for t in [DVBC2_SLICES.def(), DVBC2_PLPS.def(), DVBC2_NOTCHES.def()] {
            if !kept.table(t.key) {
                self.p.clear_rows(t);
            }
        }
        self.rows.slices_now.clear();
        self.rows.plps_now.clear();
        self.rows.notches_now = 0;
        self.l1_seen = None;
        self.view_seen = (None, None, 0);
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
