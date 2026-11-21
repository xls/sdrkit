//! VDL Mode 3 (`vdl3`): TDMA D8PSK bursts on a 25 kHz channel. Bursts are found by their four training sequences and told
//! apart by sequence and length; their Golay (24, 12) words and the Reed-Solomon (72, 62) word of a data burst are decoded.
//! Data octets and their printable text are shown; voice bursts are detected, not decoded. The fields are shown as raw values.
//! Standard: ICAO Annex 10 Volume III Part I, 6.4 (VDL Mode 3).

pub mod gen;
pub mod phy;
pub mod rx;

use crate::typed::{Vdl3BurstRow, Vdl3Fields, VDL3_BURSTS};
use phy::{BurstKind, OctetOrder, Training};
use rx::{Counts, Decoded, Vdl3Rx, EVM_GOOD, WORK_RATE};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver, Setting,
    SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, StateBlob, StateError,
    Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kept, Outbox,
    Port, StackGraph,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "vdl3";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The radio is tuned this far above the channel, off its DC spike.
pub const TUNE_OFFSET_HZ: f64 = 100e3;
/// The frequency of the test signal, Hz. The session gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 136.9e6;
/// The most bursts in the table.
pub const BURST_CAP: usize = 400;
/// The time without a good burst after which the state falls back to "bursts seen", s.
pub const DECODING_HOLD_S: f64 = 30.0;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: the bit order of the Reed-Solomon octets (each octet least \
     significant bit first, the 62 data octets before the 10 check octets, a setting); the meaning of the system data, header \
     and handoff words, which are shown as raw values; the slot timing. No recording of VDL Mode 3 exists to check against.";

/// The training threshold.
pub const SYNC_THRESHOLD: Setting<field::F64> =
    Setting::new("sync_threshold", "Training threshold", "", 0.8)
        .range(0.6, 0.95, 0.01)
        .help("The normalised differential correlation that starts a burst.");
/// The largest phase error kept.
pub const EVM_MAX: Setting<field::F64> =
    Setting::new("evm_max", "Largest phase error", "% of pi/4", 30.0)
        .range(15.0, 40.0, 1.0)
        .help("A burst whose RMS phase error is larger is taken for noise and dropped.");
/// The bit order of the Reed-Solomon octets.
pub const RS_BIT_ORDER: Setting<field::Choice<OctetOrder>> = Setting::choice(
    "rs_bit_order",
    "Reed-Solomon octet bit order",
    OctetOrder::LsbFirst,
)
.help("The order of the bits of each octet of the data word on air.")
.advanced();
/// A second station in the test signal.
pub const GEN_ADJACENT: Setting<field::Bool> =
    Setting::new("gen_adjacent", "Test signal adjacent station", "", false)
        .help("A second station 25 kHz above the channel at the same level.")
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `vdl3` stack.
    pub enum Vdl3Setting {
        /// The training threshold.
        SyncThreshold = SYNC_THRESHOLD,
        /// The largest phase error kept.
        EvmMax = EVM_MAX,
        /// The bit order of the Reed-Solomon octets.
        RsBitOrder = RS_BIT_ORDER,
        /// A second station in the test signal.
        GenAdjacent = GEN_ADJACENT,
    }
}

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum Vdl3DecoderSetting {
        /// The training threshold.
        SyncThreshold = SYNC_THRESHOLD,
        /// The largest phase error kept.
        EvmMax = EVM_MAX,
        /// The bit order of the Reed-Solomon octets.
        RsBitOrder = RS_BIT_ORDER,
    }
}

/// A row of the burst table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurstRow {
    /// Signal time of the end of the training, s.
    pub t: f64,
    /// The kind.
    pub kind: BurstKind,
    /// The training sequence.
    pub training: Training,
    /// Symbols after the training.
    pub symbols: u32,
    /// Every Golay word decoded.
    pub golay_ok: bool,
    /// Bits corrected by Golay.
    pub golay_corrected: u32,
    /// The information words, 3 hex digits each.
    pub info: String,
    /// The Reed-Solomon word was good (V/D).
    pub rs_ok: bool,
    /// Octets corrected by Reed-Solomon.
    pub rs_corrected: u32,
    /// The data octets (62) or the voice octets as received (72), hex.
    pub data_hex: String,
    /// The printable text of the data.
    pub text: String,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Es/N0, dB.
    pub snr_db: f64,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// RMS phase error, % of pi/4.
    pub evm_pct: f64,
    /// The summary.
    pub summary: String,
}

impl Shift for BurstRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The printable ASCII of data octets: trailing zeros removed, at least 4 octets and 90 % printable, else empty.
pub fn printable(octets: &[u8]) -> String {
    let end = octets.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let d = &octets[..end];
    let ok = d.iter().filter(|b| (0x20..0x7F).contains(*b)).count();
    if d.len() < 4 || ok * 10 < d.len() * 9 {
        return String::new();
    }
    d.iter()
        .map(|&b| {
            if (0x20..0x7F).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

impl BurstRow {
    /// The row of a decoded burst at signal time `t`.
    pub fn of(d: &Decoded, t: f64) -> BurstRow {
        let p = &d.payload;
        let info = p
            .words
            .iter()
            .map(|w| format!("{w:03X}"))
            .collect::<Vec<_>>()
            .join(" ");
        let vd = matches!(d.kind, BurstKind::VdData | BurstKind::VdVoice);
        let data_hex = if vd {
            p.octets.iter().map(|b| format!("{b:02X}")).collect()
        } else {
            String::new()
        };
        let text = if d.kind == BurstKind::VdData {
            printable(&p.octets)
        } else {
            String::new()
        };
        let mut summary = match d.kind {
            BurstKind::VdData if text.is_empty() => format!("header {info}, data"),
            BurstKind::VdData => format!("header {info}, data \"{text}\""),
            BurstKind::VdVoice => format!("header {info}, vocoder frames (not decoded)"),
            _ => format!("system data {info}"),
        };
        if d.evm >= EVM_GOOD {
            summary.push_str(" (weak)");
        }
        if !p.golay_ok {
            summary.push_str(" (Golay failed)");
        }
        BurstRow {
            t,
            kind: d.kind,
            training: d.training,
            symbols: d.symbols as u32,
            golay_ok: p.golay_ok,
            golay_corrected: p.golay_corrected,
            info,
            rs_ok: p.rs_ok,
            rs_corrected: p.rs_corrected,
            data_hex,
            text,
            level_dbfs: d.level_dbfs,
            snr_db: d.snr_db,
            cfo_hz: d.cfo_hz,
            evm_pct: 100.0 * d.evm,
            summary,
        }
    }
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    /// The bursts, newest last.
    pub bursts: LogRing<BurstRow>,
    /// Training sequences found.
    pub trainings: u64,
    /// Good bursts.
    pub good: u64,
    /// Bad bursts (a Golay word failed, the phase error over 25 %, or the data word failed).
    pub bad: u64,
    /// Bursts of each kind, in [`BurstKind`] order.
    pub per_kind: [u64; 8],
}

impl Default for Store {
    fn default() -> Self {
        Store {
            bursts: LogRing::new(BURST_CAP),
            trainings: 0,
            good: 0,
            bad: 0,
            per_kind: [0; 8],
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.bursts.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("avdata.vdl3", 1);
    fn clear(&mut self) {
        *self = Store {
            bursts: LogRing::new(BURST_CAP),
            ..Store::default()
        };
    }
    fn adopt(&mut self, saved: Self) {
        self.bursts.adopt(saved.bursts);
        self.trainings = saved.trainings;
        self.good = saved.good;
        self.bad = saved.bad;
        self.per_kind = saved.per_kind;
    }
}

impl Shaped for BurstRow {
    type Row = Vdl3BurstRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Vdl3BurstRow {
            time_s: self.t,
            kind: self.kind,
            training: self.training,
            symbols: i64::from(self.symbols),
            golay_ok: self.golay_ok,
            golay_corrected: i64::from(self.golay_corrected),
            info: &self.info,
            rs_ok: self.rs_ok,
            rs_corrected: i64::from(self.rs_corrected),
            data_hex: &self.data_hex,
            data_text: &self.text,
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            cfo_hz: self.cfo_hz,
            evm_pct: self.evm_pct,
            summary: &self.summary,
        });
    }
}

/// The `avdata.vdl3` decoder: the receiver and the burst table.
pub struct Vdl3Decoder {
    rx: Option<Vdl3Rx>,
    rate: f64,
    input: u64,
    /// Signal time of the receiver's first sample, s.
    t0: f64,
    threshold: f64,
    evm_max: f64,
    order: OctetOrder,
    store: Store,
    counts: Counts,
    rows: Vec<BurstRow>,
    last_good_t: Option<f64>,
    last: Option<(f64, f64, f64)>,
    started: bool,
    dirty: bool,
    cleared: bool,
    summary: String,
}

impl Default for Vdl3Decoder {
    fn default() -> Self {
        Vdl3Decoder {
            rx: None,
            rate: 0.0,
            input: 0,
            t0: 0.0,
            threshold: SYNC_THRESHOLD.default,
            evm_max: EVM_MAX.default / 100.0,
            order: OctetOrder::LsbFirst,
            store: Store::default(),
            counts: Counts::default(),
            rows: Vec::new(),
            last_good_t: None,
            last: None,
            started: false,
            dirty: true,
            cleared: false,
            summary: String::new(),
        }
    }
}

impl Vdl3Decoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn build(&mut self, f: &Format) -> Result<(), ConfigError> {
        let mut rx = Vdl3Rx::new(f.rate, f.signal_offset_hz())?;
        rx.threshold = self.threshold;
        rx.evm_max = self.evm_max;
        rx.order = self.order;
        self.rx = Some(rx);
        self.t0 = self.now();
        Ok(())
    }

    fn state(&self) -> i64 {
        let now = self.now();
        if self.last_good_t.is_some_and(|t| now - t <= DECODING_HOLD_S) {
            2
        } else if self.store.trainings > 0 {
            1
        } else {
            0
        }
    }
}

impl Decoder for Vdl3Decoder {
    type Input = sdrkit_core::Complex32;
    fn name(&self) -> &'static str {
        "avdata.vdl3"
    }
    fn outputs(&self) -> Vec<Port> {
        Vec::new()
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        Vdl3DecoderSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                Vdl3DecoderSetting::SyncThreshold,
                Value::F64(self.threshold),
            ),
            (Vdl3DecoderSetting::EvmMax, Value::F64(self.evm_max * 100.0)),
            (
                Vdl3DecoderSetting::RsBitOrder,
                Value::Choice(self.order.index()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match Vdl3DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Vdl3DecoderSetting::SyncThreshold => self.threshold = SYNC_THRESHOLD.read(id, v)?,
            Vdl3DecoderSetting::EvmMax => self.evm_max = EVM_MAX.read(id, v)? / 100.0,
            Vdl3DecoderSetting::RsBitOrder => self.order = RS_BIT_ORDER.read(id, v)?,
        }
        if let Some(rx) = self.rx.as_mut() {
            rx.threshold = self.threshold;
            rx.evm_max = self.evm_max;
            rx.order = self.order;
        }
        Ok(())
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.input = 0;
        self.build(f)?;
        Ok(Vec::new())
    }
    fn retune(&mut self, f: &Format) {
        let _ = self.build(f);
    }
    fn process(&mut self, x: &[sdrkit_core::Complex32], first: u64, _out: &mut Outbox) {
        self.started = true;
        let t_rx = self.t0;
        self.input = first + x.len() as u64;
        let Some(rx) = self.rx.as_mut() else {
            return;
        };
        let mut rows = std::mem::take(&mut self.rows);
        let before = self.counts;
        rx.process(x, &mut self.counts, |d| {
            rows.push(BurstRow::of(d, t_rx + d.end_of_training / WORK_RATE));
        });
        self.store.trainings += self.counts.trainings - before.trainings;
        for r in rows.drain(..) {
            let good = r.golay_ok
                && r.evm_pct < 100.0 * EVM_GOOD
                && (r.kind != BurstKind::VdData || r.rs_ok);
            if good {
                self.store.good += 1;
                self.last_good_t = Some(r.t);
            } else {
                self.store.bad += 1;
            }
            self.store.per_kind[r.kind.index() as usize] += 1;
            self.last = Some((r.level_dbfs, r.snr_db, r.cfo_hz));
            self.store.bursts.push(r);
            self.dirty = true;
        }
        self.rows = rows;
    }
    fn report(&mut self, p: &mut Published) {
        let as_count = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let state = self.state();
        let s = &self.store;
        let k = s.per_kind.map(as_count);
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        let newest = s.bursts.newest_first().next().map(|b| b.summary.clone());
        p.put(&Vdl3Fields {
            rx_state: state,
            trainings: as_count(s.trainings),
            frames_ok: as_count(s.good),
            frames_bad: as_count(s.bad),
            m_uplink: k[0],
            m_uplink_3t: k[1],
            handoff: k[2],
            poll_response: k[3],
            m_downlink: k[4],
            net_entry: k[5],
            vd_data: k[6],
            vd_voice: k[7],
            level_dbfs: self.last.map(|l| l.0),
            snr_db: self.last.map(|l| l.1),
            offset_hz: self.last.map(|l| Hertz(l.2)),
            text: newest.as_deref(),
            note: NOTE,
        });
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.store.bursts, &VDL3_BURSTS, self.now(), p);
        }
        let lock = match (self.started, state) {
            (false, _) => LockState::Off,
            (_, 2) => LockState::Locked,
            (_, 1) => LockState::Tracking,
            _ => LockState::Searching,
        };
        self.summary.clear();
        self.summary.push_str(&match state {
            2 => format!(
                "VDL Mode 3: decoding, {} good bursts, {} bad",
                self.store.good, self.store.bad
            ),
            1 => format!(
                "VDL Mode 3: bursts seen, {} training sequences",
                self.store.trainings
            ),
            _ if self.started => "VDL Mode 3: listening".to_string(),
            _ => "VDL Mode 3: waiting for samples".to_string(),
        });
        let level = self.last.map(|l| l.0 as f32);
        let summary = &self.summary;
        p.results_mut().update_status(|st| {
            let changed = st.lock_state != lock || st.level_dbfs != level || st.summary != *summary;
            if changed {
                st.lock_state = lock;
                st.level_dbfs = level;
                st.summary.clone_from(summary);
            }
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        self.store.clear();
        self.last_good_t = None;
        self.last = None;
        self.cleared = true;
        self.dirty = true;
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(VDL3_BURSTS.key) {
            self.store.bursts.clear();
            self.dirty = true;
        }
        if !kept.field("trainings") {
            self.store.trainings = 0;
        }
        if !kept.field("frames_ok") {
            self.store.good = 0;
            self.store.bad = 0;
        }
        if !kept.field("m_uplink") {
            self.store.per_kind = Default::default();
        }
        self.last_good_t = None;
        self.last = None;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now())?;
        self.dirty = true;
        Ok(())
    }
}

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("vdl3", DecoderComponent::new(Vdl3Decoder::default()))
        .input("vdl3")
        .status_from("vdl3")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "VDL Mode 3 needs a sample rate of at least 250 kHz",
        )
        .route(SYNC_THRESHOLD.key(), &["vdl3.sync_threshold"])?
        .route(EVM_MAX.key(), &["vdl3.evm_max"])?
        .route(RS_BIT_ORDER.key(), &["vdl3.rs_bit_order"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("avdata.vdl3", || {
        Box::new(DecoderComponent::new(Vdl3Decoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::Vdl3Generator::from_config(
        cfg,
        TUNE_OFFSET_HZ,
    )?))
}

fn verify_cases(o: &sdrkit_core::verify::VerifyOptions) -> Vec<sdrkit_core::verify::VerifyCase> {
    let mut c = sdrkit_core::verify::VerifyCase::new("test signal", o, 500.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| vec![crate::verify::at_least(r, "frames_ok", 20.0, "good bursts")]);
    vec![c]
}

/// The engine's checks of the test signal.
pub static HOOKS: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The VDL Mode 3 stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "VDL Mode 3",
    group: "Aviation",
    blurb: "VDL Mode 3 TDMA bursts on a 25 kHz channel: burst kinds by training sequence and length, Golay words, \
            Reed-Solomon data with its printable text; voice bursts detected, not decoded.",
    tuning: Tuning {
        tune_offset_hz: TUNE_OFFSET_HZ,
        ..Tuning::anywhere(TEST_SIGNAL_HZ, 1e6, MIN_RATE_HZ, 25e3)
    },
    submodes: &[],
    settings: Vdl3Setting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
