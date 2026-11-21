//! The DVB-S and DVB-S2 satellite receiver: the front end with its carrier search, the sync stage, the DVB-S2 FEC, the
//! report and the test-signal generator, with the stream adaptation and IP of `sdrkit-proto-ipcast` after the FEC.
//! [`attach`] adds the DVB-S2 receiver to the graph of a stack, with a transport stream output, a data-field output of
//! every good BBFRAME, or both. [`typed`] holds its shared interface `dvbs2`, and [`register`] adds its components.
//!
//! Standard: ETSI EN 300 421; ETSI EN 302 307-1.

#![warn(missing_docs)]
#![forbid(unsafe_code)]
// Index loops read more clearly than iterator chains in the signal processing here.
#![allow(clippy::needless_range_loop)]

pub mod fec;
pub mod front;
pub mod gen;
pub mod inner;
pub mod modcod;
pub mod pl;
pub mod receive;
pub mod s1encode;
pub mod s2encode;
pub mod settings;
pub mod spectrum;
pub mod sync;
pub mod transmit;
pub mod typed;

pub use modcod::CodeRate;
pub use settings::DvbsSetting;

use crate::receive::{item_format, write_status, Lost};
use crate::typed::{
    with_dvbs2_receiver, Dvbs2CarrierRow, Dvbs2Fields, Dvbs2StreamModcodRow, Dvbs2StreamRow,
    DVBS2_CARRIERS, DVBS2_STREAMS, DVBS2_STREAM_MODCODS,
};
use fec::{FrameDecoder, FrameResult};
use front::{Acquisition, Carrier, Front, FrontConfig};
use modcod::{ModCod, Modulation};
use sdrkit_core::interface::{Hertz, StackShape};
use sdrkit_core::publish::Published;
use sdrkit_core::results::{LockState, SeriesId};
use sdrkit_core::{
    Choice, Complex32, ConfigError, ResultsDecl, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, GraphBuilder, Io, Kind, Packet,
    PacketPool, Pool, Port, RateFrom, RunContext, SlicePool, Stage, Timed,
};
use sdrkit_proto_ipcast::bbheader::{bb_descramble, BbHeader, StreamFormat, System, BBHEADER_LEN};
use sdrkit_proto_ipcast::components::{BbFrameAdapt, SystemChoice};
use sdrkit_proto_ipcast::field::DataField;
use sdrkit_proto_mpegts::outer::{OuterStats, SyncRule};
use settings::{
    parse_modcods, parse_pick, Fec2Setting, FrameLengthRule, FrontSetting, LnbSupply, ModcodRule,
    PilotsRule, Polarisation, ReportSetting, StandardChoice, SyncSetting, CARRIER_PICK,
    FRAME_LENGTH, LNB_SUPPLY, MODCOD as MODCOD_SETTING, MODCODS, MODCOD_RULE, PILOTS_RULE,
    PL_SCRAMBLING, ROLL_OFF, STANDARD as STANDARD_SETTING, SYMBOL_RATE, TX_POLARISATION,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;
use sync::{Received, S1Soft, S2Frame, Standard, Sync, SyncReport};

/// Symbols between the front end and the sync stage.
#[derive(Clone, Debug)]
pub struct SymChunk {
    /// Symbols at one per symbol, unit mean power, from the front end's pool.
    pub syms: Arc<[Complex32]>,
    /// The front end (re)started before these.
    pub restart: bool,
}

impl sdrkit_graph::Item for SymChunk {
    const KIND: &'static str = "dvbs.symbols";
}

/// The packet sync rule of DVB-S and DVB-C: RS words of 204 bytes with dispersal groups, at least `min_places` words looked at,
/// 80 % of them with a sync byte.
pub fn sync_rule(min_places: usize, allow_inverted: bool) -> SyncRule {
    SyncRule {
        spacing: sdrkit_proto_mpegts::outer::RS_N,
        min_places,
        min_fraction: 0.8,
        groups: true,
        allow_inverted,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The front end.

/// The state the front end sends to the report on each tick of the graph clock.
#[derive(Clone, Debug, Default)]
pub struct FrontState {
    /// Carriers of the survey.
    pub carriers: Vec<Carrier>,
    /// The acquisition.
    pub acq: Option<Acquisition>,
    /// Input spectrum (512 points, dB).
    pub spectrum: Vec<f32>,
    /// Symbol rate the loop runs at.
    pub rate_now: f64,
    /// Loop offset, ppm.
    pub loop_ppm: f64,
    /// Input level, dBFS.
    pub level_dbfs: f64,
    /// Seconds without a carrier.
    pub no_carrier_s: f64,
    /// The input rate, Hz.
    pub input_rate: f64,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl sdrkit_graph::Item for FrontState {
    const KIND: &'static str = "dvbs.front_state";
}

/// The state the sync stage sends to the front end when it changes: frames are followed, the stage is locked, and the hunts
/// that failed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncFeedback {
    /// DVB-S2 frames are followed.
    pub followed: bool,
    /// The sync stage is locked.
    pub locked: bool,
    /// Hunts that failed.
    pub hunts: u64,
}

impl sdrkit_graph::Item for SyncFeedback {
    const KIND: &'static str = "dvbs.sync_feedback";
}

/// `dvbs.front`: the survey, the mixer, decimators and the matched filter with timing recovery. Input `feedback` takes the
/// [`SyncFeedback`] of the sync stage.
#[derive(Default)]
pub struct DvbsFront {
    cfg: FrontConfig,
    input_rate: f64,
    front: Option<Front>,
    hunts: u64,
    hunts_seen: u64,
    locked: bool,
    pick_text: String,
    roll_off: settings::RollOffChoice,
    out: Vec<Complex32>,
    pool: SlicePool<Complex32>,
}

impl Component for DvbsFront {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbs.front",
            vec![
                Port::iq("in"),
                Port::typed::<SyncFeedback>("feedback")
                    .optional("no rate dither after failed hunts"),
            ],
            vec![
                Port::typed::<SymChunk>("symbols").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FrontState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.front = Some(Front::new(inputs[0].rate, self.cfg)?);
        self.input_rate = inputs[0].rate;
        self.hunts = 0;
        self.hunts_seen = 0;
        self.locked = false;
        Ok(vec![
            item_format::<SymChunk>(&inputs[0]),
            Format::new(Kind::typed::<FrontState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(fb) = io.items::<SyncFeedback>(1).last() {
            self.locked = fb.value.locked;
            self.hunts = fb.value.hunts;
        }
        let t0 = io.input_format(0).time_of(io.input(0).first());
        let Some(f) = self.front.as_mut() else {
            return;
        };
        f.hold_timing(self.locked);
        // The rate dither after three failed hunts.
        if f.symbols < 1000 {
            self.hunts_seen = self.hunts;
        }
        if self.hunts >= self.hunts_seen + 3
            && !self.locked
            && f.acq.is_some()
            && f.symbols >= 150_000
        {
            self.hunts_seen = self.hunts;
            if !f.next_dither() {
                f.restart();
            }
        }
        self.out.clear();
        f.process(io.iq(0), &mut self.out);
        let restart = std::mem::replace(&mut f.restarted, false);
        if !self.out.is_empty() || restart {
            let out = &self.out;
            let syms = self.pool.exact(out.len(), |o| o.copy_from_slice(out));
            io.items_out::<SymChunk>(0).push(Timed {
                time: t0,
                value: SymChunk { syms, restart },
            });
        }
        if io.tick() {
            let st = FrontState {
                carriers: f.carriers.clone(),
                acq: f.acq,
                spectrum: f.spectrum.clone(),
                rate_now: f.rate_now(),
                loop_ppm: f.loop_ppm(),
                level_dbfs: f.level_dbfs,
                no_carrier_s: f.no_carrier_s,
                input_rate: self.input_rate,
                log: std::mem::take(&mut f.log),
            };
            io.items_out::<FrontState>(1).push(Timed {
                time: t0,
                value: st,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::SymbolRate => self.cfg.manual_rs = SYMBOL_RATE.read(id, v)?,
            FrontSetting::RollOff => {
                self.roll_off = ROLL_OFF.read(id, v)?;
                self.cfg.rolloff = self.roll_off.value();
            }
            FrontSetting::CarrierPick => {
                let t = CARRIER_PICK.read(id, v)?;
                self.cfg.pick_hz = parse_pick(id, t)?;
                self.pick_text = t.to_owned();
            }
        }
        if let Some(f) = self.front.as_mut() {
            f.cfg = self.cfg;
            f.restart();
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (FrontSetting::SymbolRate, Value::Hz(self.cfg.manual_rs)),
            (FrontSetting::RollOff, Value::Choice(self.roll_off.index())),
            (
                FrontSetting::CarrierPick,
                Value::Text(self.pick_text.clone()),
            ),
        ])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The sync stage.

/// `dvbs.sync`: the DVB-S2 PLHEADER hunt and frame follower, and the DVB-S hypotheses with the QPSK carrier loop. Input `lost`
/// receives the [`Lost`] items of the DVB-S decoder. Output `feedback` reports to the front end whether frames are followed and
/// how many hunts failed.
pub struct DvbsSync {
    sync: Sync,
    standard: StandardChoice,
    sent: SyncFeedback,
}

impl Default for DvbsSync {
    fn default() -> Self {
        DvbsSync {
            sync: Sync::new(Standard::Auto, 0),
            standard: StandardChoice::Auto,
            sent: SyncFeedback::default(),
        }
    }
}

impl sdrkit_graph::Item for SyncReport {
    const KIND: &'static str = "dvbs.sync_state";
}

impl Component for DvbsSync {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbs.sync",
            vec![
                Port::typed::<SymChunk>("in"),
                Port::typed::<Lost>("lost").optional("the DVB-S stream is never reported lost"),
            ],
            vec![
                Port::typed::<S2Frame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<S1Soft>("soft").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SyncFeedback>("feedback").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SyncReport>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(SyncSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.sync = Sync::new(self.sync.standard, self.sync.gold);
        self.sent = SyncFeedback::default();
        Ok(vec![
            item_format::<S2Frame>(&inputs[0]),
            item_format::<S1Soft>(&inputs[0]),
            Format::new(Kind::typed::<SyncFeedback>(), 0.0),
            Format::new(Kind::typed::<SyncReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.items::<Lost>(1).is_empty() {
            self.sync.lost();
        }
        let mut t = 0.0;
        for c in io.items::<SymChunk>(0) {
            if c.value.restart {
                self.sync.restart();
            }
            t = c.time;
            self.sync.push(&c.value.syms);
        }
        for f in self.sync.frames_out.drain(..) {
            io.items_out::<S2Frame>(0).push(Timed { time: t, value: f });
        }
        for s in self.sync.soft_out.drain(..) {
            io.items_out::<S1Soft>(1).push(Timed { time: t, value: s });
        }
        let r = &self.sync.report;
        let now = SyncFeedback {
            followed: r.locked && r.standard == Some(Received::Dvbs2),
            locked: r.locked,
            hunts: r.failed_hunts,
        };
        if now != self.sent {
            io.items_out::<SyncFeedback>(2).push(Timed {
                time: t,
                value: now,
            });
            self.sent = now;
        }
        if io.tick() {
            let mut st = self.sync.report.clone();
            st.log_lines = std::mem::take(&mut self.sync.log);
            io.items_out::<SyncReport>(3)
                .push(Timed { time: t, value: st });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SyncSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SyncSetting::Standard => {
                self.standard = STANDARD_SETTING.read(id, v)?;
                self.sync.standard = match self.standard {
                    StandardChoice::Auto => Standard::Auto,
                    StandardChoice::Dvbs => Standard::S,
                    StandardChoice::Dvbs2 => Standard::S2,
                };
            }
            SyncSetting::PlScrambling => self.sync.gold = PL_SCRAMBLING.read(id, v)? as u32,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (SyncSetting::Standard, Value::Choice(self.standard.index())),
            (
                SyncSetting::PlScrambling,
                Value::I64(i64::from(self.sync.gold)),
            ),
        ])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The DVB-S2 FEC frame.

sdrkit_core::choice_enum! {
    /// What an input stream of DVB-S2 carries.
    pub enum Carries {
        /// A transport stream.
        TransportStream = ("ts", "Transport stream"),
        /// GSE.
        Gse = ("gse", "GSE"),
        /// GSE with the high efficiency mode.
        GseHem = ("gse_hem", "GSE-HEM"),
        /// Transport packets in a generic stream (sync byte 47h).
        TsInGeneric = ("ts_generic", "Transport packets in a generic stream"),
        /// Packets of another kind.
        Packets = ("packets", "Packets"),
    }
}

/// One input stream of a DVB-S2 carrier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamRow {
    /// What it carries.
    pub carries: Carries,
    /// The user packet length, bytes (0 for a continuous stream).
    pub packet_bytes: u16,
    /// The sync byte of the user packets.
    pub sync: u8,
    /// CCM.
    pub ccm: bool,
    /// Frames.
    pub frames: u64,
    /// Frames of each MODCOD, by MODCOD (1 to 28).
    pub modcods: [u64; 29],
    /// Data field bits.
    pub bits: u64,
}

impl Default for StreamRow {
    fn default() -> Self {
        StreamRow {
            carries: Carries::TransportStream,
            packet_bytes: 0,
            sync: 0,
            ccm: true,
            frames: 0,
            modcods: [0; 29],
            bits: 0,
        }
    }
}

/// The state the DVB-S2 FEC sends to the report on each tick of the graph clock.
#[derive(Clone, Debug, Default)]
pub struct Fec2State {
    /// BCH words good.
    pub bch_ok: u64,
    /// BCH words that failed.
    pub bch_bad: u64,
    /// Frames followed but not decoded (no code for the MODCOD).
    pub undecoded: u64,
    /// Frames skipped by the MODCOD rule or the frame length rule.
    pub other_frames: u64,
    /// Frames whose pilots differ from the setting `pilots`.
    pub pilot_mismatch: u64,
    /// BBFRAMEs whose BCH word was good and whose BBHEADER was not (CRC-8, or a DFL beyond the frame).
    pub bbheader_bad: u64,
    /// LDPC iterations, recent mean.
    pub ldpc_iter: f64,
    /// The roll-off the BBHEADERs signal.
    pub rolloff: Option<f64>,
    /// The input streams by ISI.
    pub streams: BTreeMap<u8, StreamRow>,
    /// All input streams are single streams.
    pub single: bool,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl sdrkit_graph::Item for Fec2State {
    const KIND: &'static str = "dvbs.fec2_state";
}

/// What the outputs need of a decoded frame besides its bytes.
struct FrameInfo {
    time: f64,
    modcod: u8,
    short: bool,
    pilots: bool,
}

/// The most helper threads of the engine's pool that one batch of frames uses.
const MAX_HELPERS: usize = 4;
/// The most signal time between the first and the last frame of a batch, seconds.
const MAX_WAIT_S: f64 = 0.05;

/// A frame that waits in a batch: what the outputs need, the frames lost before it, the input of the decoder, a decoder,
/// and after the decoding its result and BBFRAME bytes.
struct Job {
    info: FrameInfo,
    lost_before: u64,
    modcod: ModCod,
    syms: Arc<[Complex32]>,
    noise_var: f32,
    dec: FrameDecoder,
    bytes: Vec<u8>,
    result: Option<FrameResult>,
}

impl Job {
    /// Decodes the frame with the decoder of the job.
    fn decode(&mut self) {
        self.result = self.dec.decode(
            &self.modcod,
            self.info.short,
            &self.syms,
            self.noise_var,
            &mut self.bytes,
        );
    }
}

/// What the frames taken in one pass add to the outputs.
#[derive(Default)]
struct Taken {
    out: Vec<Packet>,
    /// The packets of `out` that came with the pass.
    first: usize,
    tags: Vec<usize>,
    fields: Vec<Timed<DataField>>,
}

/// `dvbs.fec2`: soft demapping, bit de-interleaving, LDPC and BCH (`sdrkit-algo-fec`), BB descrambling. Output `out`: one descrambled
/// BBFRAME per packet, for `ipcast.bbframe`. A frame whose BCH word failed is not sent, and the next frame has a `Discontinuity`
/// tag. Output `field` ([`Dvbs2Fec::with_field`] only): one [`DataField`] per good BBFRAME of every input stream.
///
/// The settings choose which MODCODs and which frame length are decoded; other frames are counted and skipped before the LDPC
/// decoder.
///
/// The frames wait in a batch of one more than the helper threads that the engine's pool offers ([`Pool::workers`], at
/// most [`MAX_HELPERS`]); a full batch, a batch that spans [`MAX_WAIT_S`] of signal, the report tick and the end of the
/// stream decode the batch with [`Pool::map`], and the results are taken in the order of the frames. So the output is the same for every size of the pool. A stack marks
/// the edge from the sync stage to this component as a stage boundary, so that the engine can run it on a thread of its own.
#[derive(Default)]
pub struct Dvbs2Fec {
    /// The engine's pool.
    helpers: Option<Pool>,
    /// The frames that wait for the decoder, in their order.
    batch: Vec<Job>,
    /// Decoders and byte buffers for the next frames.
    spare: Vec<(FrameDecoder, Vec<u8>)>,
    pool: PacketPool,
    st: Fec2State,
    lost: bool,
    field: bool,
    lost_run: u64,
    /// Frames lost since the last frame taken into a batch.
    lost_next: u64,
    field_pool: PacketPool,
    rule: ModcodRule,
    modcod: i64,
    modcods_text: String,
    modcods: Vec<u8>,
    length: FrameLengthRule,
    pilots: PilotsRule,
}

impl Dvbs2Fec {
    /// The FEC with the output `field` of data fields.
    pub fn with_field() -> Self {
        Dvbs2Fec {
            field: true,
            ..Default::default()
        }
    }

    /// Whether the rules take a frame of MODCOD `id`.
    fn taken(&self, id: u8, short: bool) -> bool {
        let modcod_ok = match self.rule {
            ModcodRule::Any => true,
            ModcodRule::Fixed => i64::from(id) == self.modcod,
            ModcodRule::Set => self.modcods.contains(&id),
        };
        let length_ok = match self.length {
            FrameLengthRule::Any => true,
            FrameLengthRule::Normal => !short,
            FrameLengthRule::Short => short,
        };
        modcod_ok && length_ok
    }

    /// A frame lost before the next good one (or a relock): the next `out` packet has a `Discontinuity` tag, the next data field
    /// counts it.
    fn lose(&mut self) {
        self.lost = true;
        self.lost_run += 1;
    }

    /// Counts a frame of the sync stage and adds it to the batch, unless a rule skips it.
    fn admit(&mut self, time: f64, f: &S2Frame) {
        if f.restart {
            self.lost_next += 1;
        }
        let Some(m) = ModCod::new(f.modcod) else {
            return;
        };
        if !self.taken(f.modcod, f.short) {
            self.st.other_frames += 1;
            return;
        }
        let pilots_ok = match self.pilots {
            PilotsRule::Any => true,
            PilotsRule::On => f.pilots,
            PilotsRule::Off => !f.pilots,
        };
        if !pilots_ok {
            if self.st.pilot_mismatch == 0 {
                self.st.log.push(format!(
                    "pilots {} where the setting expects {}",
                    if f.pilots { "on" } else { "off" },
                    if f.pilots { "off" } else { "on" }
                ));
            }
            self.st.pilot_mismatch += 1;
        }
        let noise_var = 10f32.powf(-(f.esn0_db.clamp(-10.0, 40.0) as f32) / 10.0);
        let (dec, bytes) = self.spare.pop().unwrap_or_default();
        self.batch.push(Job {
            info: FrameInfo {
                time,
                modcod: f.modcod,
                short: f.short,
                pilots: f.pilots,
            },
            lost_before: std::mem::take(&mut self.lost_next),
            modcod: m,
            syms: f.syms.clone(),
            noise_var,
            dec,
            bytes,
            result: None,
        });
    }

    /// The frames that one batch holds: one more than the helper threads of the pool.
    fn batch_size(&self) -> usize {
        1 + self
            .helpers
            .as_ref()
            .map_or(0, |p| p.workers().min(MAX_HELPERS))
    }

    /// Decodes the frames of the batch and adds what they decoded to `t`, in their order.
    fn flush(&mut self, t: &mut Taken) {
        let mut jobs = std::mem::take(&mut self.batch);
        match &self.helpers {
            Some(pool) if jobs.len() > 1 => {
                jobs = pool.map(jobs, |_, mut j| {
                    j.decode();
                    j
                });
            }
            _ => jobs.iter_mut().for_each(Job::decode),
        }
        for j in jobs {
            let Job {
                info,
                lost_before,
                dec,
                bytes,
                result,
                ..
            } = j;
            let bytes = self.take(&info, lost_before, result, bytes, t);
            self.spare.push((dec, bytes));
        }
    }

    /// Adds what the frame `w` decoded to `t`, after the `lost_before` frames lost before it, and returns its byte buffer.
    fn take(
        &mut self,
        w: &FrameInfo,
        lost_before: u64,
        result: Option<FrameResult>,
        mut bytes: Vec<u8>,
        t: &mut Taken,
    ) -> Vec<u8> {
        for _ in 0..lost_before {
            self.lose();
        }
        match result {
            Some(r) => {
                self.st.ldpc_iter += 0.1 * (r.iterations as f64 - self.st.ldpc_iter);
                if r.bch_ok {
                    self.st.bch_ok += 1;
                    bb_descramble(&mut bytes);
                    self.deliver(w, &bytes, t);
                } else {
                    self.st.bch_bad += 1;
                    self.lose();
                }
            }
            None => {
                if self.st.undecoded == 0 {
                    self.st.log.push(format!(
                        "{}{}: frames followed, not decoded (no code of this size)",
                        ModCod::new(w.modcod).map(|m| m.name()).unwrap_or_default(),
                        if w.short { " short" } else { "" }
                    ));
                }
                self.st.undecoded += 1;
            }
        }
        bytes
    }

    /// Reads the BBHEADER of the descrambled BBFRAME `frame`, counts its stream and adds the frame to `t`.
    fn deliver(&mut self, w: &FrameInfo, frame: &[u8], t: &mut Taken) {
        let time = w.time;
        let header = BbHeader::parse(frame).ok().filter(|h| {
            let dfl = usize::from(h.dfl);
            dfl.is_multiple_of(8) && BBHEADER_LEN + dfl / 8 <= frame.len()
        });
        let Some(h) = header else {
            self.st.bbheader_bad += 1;
            self.lose();
            return;
        };
        self.st.rolloff = h
            .rolloff(System::DvbS2)
            .map(|r| r as f64)
            .or(self.st.rolloff);
        self.st.single &= h.single_stream;
        let row = self.st.streams.entry(h.stream_id).or_default();
        row.carries = match h.format {
            StreamFormat::Transport => Carries::TransportStream,
            _ if h.carries_gse() => Carries::Gse,
            StreamFormat::Gse => Carries::GseHem,
            _ if h.sync == 0x47 && h.upl == 1504 => Carries::TsInGeneric,
            _ => Carries::Packets,
        };
        row.packet_bytes = h.upl / 8;
        row.sync = h.sync;
        row.ccm = h.ccm;
        row.frames += 1;
        if let Some(n) = row.modcods.get_mut(usize::from(w.modcod)) {
            *n += 1;
        }
        row.bits += h.dfl as u64;
        if self.field {
            let n = usize::from(h.dfl) / 8;
            let mut raw_header = [0u8; BBHEADER_LEN];
            raw_header.copy_from_slice(&frame[..BBHEADER_LEN]);
            t.fields.push(Timed {
                time,
                value: DataField {
                    raw_header,
                    header: h,
                    data: self.field_pool.make(n, time, |d| {
                        d.copy_from_slice(&frame[BBHEADER_LEN..BBHEADER_LEN + n])
                    }),
                    modcod: w.modcod,
                    short: w.short,
                    pilots: w.pilots,
                    lost_before: std::mem::take(&mut self.lost_run),
                    time,
                },
            });
        }
        self.lost_run = 0;
        if self.lost {
            t.tags.push(t.out.len() - t.first);
            self.lost = false;
        }
        t.out.push(
            self.pool
                .make(frame.len(), time, |d| d.copy_from_slice(frame)),
        );
    }

    /// One pass: adds the frames of the pass to the batch, decodes each batch that is full or spans [`MAX_WAIT_S`], and the
    /// batch left at the report tick or with `all` (the end of the stream), and writes the outputs.
    fn run(&mut self, io: &mut Io, all: bool) {
        let out = std::mem::take(io.bytes_out(0));
        let mut t = Taken {
            first: out.len(),
            out,
            ..Default::default()
        };
        let size = self.batch_size();
        for f in io.items::<S2Frame>(0) {
            self.admit(f.time, &f.value);
            let span = match (self.batch.first(), self.batch.last()) {
                (Some(a), Some(b)) => b.info.time - a.info.time,
                _ => 0.0,
            };
            if self.batch.len() >= size || span >= MAX_WAIT_S {
                self.flush(&mut t);
            }
        }
        let tick = io.tick() || all;
        if tick {
            self.flush(&mut t);
        }
        *io.bytes_out(0) = t.out;
        for k in t.tags {
            io.output(0)
                .tag_here(k, sdrkit_graph::TagKind::Discontinuity { lost: 0 });
        }
        if self.field {
            io.items_out::<DataField>(2).extend(t.fields);
        }
        if tick {
            let st = Fec2State {
                log: std::mem::take(&mut self.st.log),
                ..self.st.clone()
            };
            io.items_out::<Fec2State>(1).push(Timed {
                time: 0.0,
                value: st,
            });
        }
    }
}

impl Component for Dvbs2Fec {
    fn info(&self) -> ComponentInfo {
        let mut outputs = vec![
            Port::bytes("out").rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<Fec2State>("state").rate_from(RateFrom::Fixed(0.0)),
        ];
        if self.field {
            outputs.push(Port::typed::<DataField>("field").rate_from(RateFrom::Fixed(0.0)));
        }
        ComponentInfo::processor("dvbs.fec2", vec![Port::typed::<S2Frame>("in")], outputs)
            .manual_tags()
            .with_settings(Fec2Setting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.helpers = Some(ctx.pool.clone());
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The frames of the signal before are given up.
        for j in self.batch.drain(..) {
            self.spare.push((j.dec, j.bytes));
        }
        self.st = Fec2State {
            single: true,
            ..Default::default()
        };
        self.lost = false;
        self.lost_run = 0;
        self.lost_next = 0;
        let mut formats = vec![
            Format {
                rate: 0.0,
                ..inputs[0].with_kind(Kind::Bytes)
            },
            Format::new(Kind::typed::<Fec2State>(), 0.0),
        ];
        if self.field {
            formats.push(Format::new(Kind::typed::<DataField>(), 0.0));
        }
        Ok(formats)
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io, false);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io, true);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match Fec2Setting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Fec2Setting::ModcodRule => self.rule = MODCOD_RULE.read(id, v)?,
            Fec2Setting::Modcod => self.modcod = MODCOD_SETTING.read(id, v)?,
            Fec2Setting::Modcods => {
                let t = MODCODS.read(id, v)?;
                self.modcods = parse_modcods(id, t)?;
                self.modcods_text = t.to_owned();
            }
            Fec2Setting::FrameLength => self.length = FRAME_LENGTH.read(id, v)?,
            Fec2Setting::Pilots => self.pilots = PILOTS_RULE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (Fec2Setting::ModcodRule, Value::Choice(self.rule.index())),
            (Fec2Setting::Modcod, Value::I64(self.modcod)),
            (Fec2Setting::Modcods, Value::Text(self.modcods_text.clone())),
            (Fec2Setting::FrameLength, Value::Choice(self.length.index())),
            (Fec2Setting::Pilots, Value::Choice(self.pilots.index())),
        ])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The results.

sdrkit_core::choice_enum! {
    /// Where the roll-off came from.
    pub enum RollOffSource {
        /// The usual value, assumed.
        Assumed = ("assumed", "Assumed"),
        /// Measured from the spectrum.
        Measured = ("measured", "Measured"),
        /// Signalled in the BBHEADER.
        Signalled = ("signalled", "Signalled"),
    }
}

sdrkit_core::choice_enum! {
    /// The size of a DVB-S2 FEC frame.
    pub enum FrameSize {
        /// 64 800 bits.
        Normal = ("normal", "Normal (64800)"),
        /// 16 200 bits.
        Short = ("short", "Short (16200)"),
    }
}

sdrkit_core::choice_enum! {
    /// A note on the signal for the user.
    pub enum SignalNote {
        /// No carrier for 10 s: the LNB polarisation or band, or the dish, is wrong.
        NoCarrier = ("no_carrier", "No carrier found. Most often the LNB is on the other polarisation (13 V vertical, 18 V horizontal) or the other band (22 kHz tone), or the dish is off target."),
        /// Frames are followed but not decoded: no code of this size.
        NotDecoded = ("not_decoded", "Frames followed, not decoded (no code of this size)"),
    }
}

const CONSTELLATION: SeriesId = SeriesId(0);
const SPECTRUM: SeriesId = SeriesId(1);
const MER_HISTORY: SeriesId = SeriesId(2);

/// What `dvbs.report` publishes: the shapes of [`with_dvbs2_receiver`], for a stack whose results are declared without
/// a description.
pub fn report_declaration() -> ResultsDecl {
    with_dvbs2_receiver(StackShape::new("dvbs2")).results_decl()
}

/// Seconds of signal after the last good frame during which the receiver still counts as delivering data.
pub const DATA_HOLD_S: f64 = 3.0;

/// The state of the receiver for a stack that takes its output: what `dvbs.report` sends on its output `link` at each report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkState {
    /// 0 searching, 1 carrier and symbols, 2 PL frames locked and decoding, 3 data coming out (a good frame in the last
    /// [`DATA_HOLD_S`] seconds).
    pub state: u8,
    /// Es/N0 from the PLHEADERs, dB.
    pub esn0_db: Option<f64>,
    /// MER, dB.
    pub mer_db: Option<f64>,
    /// The carrier offset from the centre, Hz.
    pub offset_hz: f64,
    /// The input level, dBFS.
    pub level_dbfs: f64,
    /// The input rate, Hz.
    pub input_rate: f64,
    /// The symbol rate in use, Hz (0 before a carrier).
    pub symbol_rate: f64,
    /// The MODCOD of the frames followed.
    pub modcod: Option<u8>,
    /// The signal in words: "DVB-S2 QPSK 9/10 pilots".
    pub signal: String,
    /// BCH words good and failed.
    pub bch_ok: u64,
    /// BCH words failed.
    pub bch_bad: u64,
    /// Seconds of signal.
    pub time_s: f64,
}

impl sdrkit_graph::Item for LinkState {
    const KIND: &'static str = "dvbs.link_state";
}

/// `dvbs.report`: the results of the receiver from the states of the front end, the sync stage and both FEC paths. Output
/// `link` carries a [`LinkState`] at each report for a stack that builds on the receiver.
pub struct DvbsReport {
    published: Published,
    front: FrontState,
    sync: SyncReport,
    fec2: Fec2State,
    rs: OuterStats,
    manual_rs: f64,
    gold: u32,
    tx_pol: Polarisation,
    lnb: LnbSupply,
    carrier_keys: Vec<i64>,
    carrier_keys_now: Vec<i64>,
    key: String,
    stream_key: String,
    summary: String,
    now: f64,
    last_ok: u64,
    last_ok_time: Option<f64>,
    bits_at: Option<(f64, u64)>,
    net_rate: f64,
    link: LinkState,
}

impl Default for DvbsReport {
    fn default() -> Self {
        DvbsReport {
            published: Published::from_decl(Arc::new(report_declaration())),
            front: FrontState::default(),
            sync: SyncReport::default(),
            fec2: Fec2State {
                single: true,
                ..Default::default()
            },
            rs: OuterStats::default(),
            manual_rs: 0.0,
            gold: 0,
            tx_pol: Polarisation::Unset,
            lnb: LnbSupply::Unset,
            carrier_keys: Vec::new(),
            carrier_keys_now: Vec::new(),
            key: String::new(),
            stream_key: String::new(),
            summary: String::with_capacity(160),
            now: 0.0,
            last_ok: 0,
            last_ok_time: None,
            bits_at: None,
            net_rate: 0.0,
            link: LinkState::default(),
        }
    }
}

impl DvbsReport {
    /// Follows the good frames and the data field bits over signal time.
    fn follow_data(&mut self) {
        let f2 = &self.fec2;
        if f2.bch_ok != self.last_ok {
            self.last_ok = f2.bch_ok;
            self.last_ok_time = Some(self.now);
        }
        let bits: u64 = f2.streams.values().map(|r| r.bits).sum();
        match self.bits_at {
            Some((t, b)) if self.now - t >= 1.0 => {
                self.net_rate = bits.saturating_sub(b) as f64 / (self.now - t);
                self.bits_at = Some((self.now, bits));
            }
            None => self.bits_at = Some((self.now, bits)),
            _ => {}
        }
    }

    fn publish(&mut self) {
        self.follow_data();
        let delivering = self
            .last_ok_time
            .is_some_and(|t| self.now - t <= DATA_HOLD_S);
        let (fr, s, f2, rs) = (&self.front, &self.sync, &self.fec2, self.rs);
        let s2 = s.standard == Some(Received::Dvbs2);
        let s1 = s.standard == Some(Received::Dvbs);
        let acq = fr.acq;
        let rs_now = fr.rate_now;
        let offset = acq.map_or(0.0, |a| a.carrier.offset_hz)
            + s.freq_cps * rs_now * if s.inverted { -1.0 } else { 1.0 };
        let (ro, src) = match (s2, f2.rolloff, acq) {
            (true, Some(r), _) => (r, RollOffSource::Signalled),
            (_, _, Some(a)) => (
                a.rolloff,
                if a.rolloff_source == 0 {
                    RollOffSource::Assumed
                } else {
                    RollOffSource::Measured
                },
            ),
            _ => (0.35, RollOffSource::Assumed),
        };
        let m = ModCod::new(s.modcod);
        let (modulation, rate) = match (s1, m) {
            (true, _) => (Some(Modulation::Qpsk), s.rate.map(CodeRate::of_punctured)),
            (false, Some(m)) if s2 => (Some(m.modulation), m.code_rate()),
            _ => (None, None),
        };
        let ebn0 = s.esn0_db.and_then(|e| {
            if s2 {
                m.map(|m| e - 10.0 * (m.modulation.bits() as f64 * m.rate_value()).log10())
            } else if s1 {
                s.rate
                    .map(|r| e - 10.0 * (2.0 * r.value() * 188.0 / 204.0).log10())
            } else {
                None
            }
        });
        let (ok, bad) = if s1 {
            (rs.clean + rs.corrected, rs.failed)
        } else {
            (f2.bch_ok, f2.bch_bad)
        };
        let delivered = if s1 { rs.delivered } else { f2.bch_ok };
        let ts_lock = s.locked && ok > 0 && delivered > 0;
        let note = if fr.no_carrier_s >= 10.0 && acq.is_none() {
            Some(SignalNote::NoCarrier)
        } else if f2.undecoded > 0 {
            Some(SignalNote::NotDecoded)
        } else {
            None
        };
        let p = &mut self.published;
        p.put(&Dvbs2Fields {
            standard: s.standard,
            signal_note: note,
            symbol_rate_hz: (rs_now > 0.0).then_some(Hertz(rs_now)),
            spectrum_rate: acq.map(|a| Hertz(a.carrier.rate_hz)),
            clock_ppm: fr.loop_ppm,
            manual_rate: self.manual_rs > 0.0,
            spectrum_snr_db: acq.map(|a| a.carrier.snr_db),
            misfit: acq.map(|a| a.carrier.misfit),
            edge_low: acq.map(|a| Hertz(a.carrier.lo_hz)),
            edge_high: acq.map(|a| Hertz(a.carrier.hi_hz)),
            offset_hz: acq.map(|_| Hertz(offset)),
            inverted: s.inverted && s.locked,
            roll_off: (ro * 100.0).round() / 100.0,
            roll_off_source: src,
            modulation,
            code_rate: rate,
            modcod: s2.then_some(s.modcod as i64),
            frame: s2.then_some(if s.short {
                FrameSize::Short
            } else {
                FrameSize::Normal
            }),
            pilots: s2 && s.pilots,
            vcm: s.vcm,
            single_stream: f2.streams.len() <= 1 && f2.single,
            pl_scrambling_code: i64::from(self.gold),
            plframes: s.frames as i64,
            dummy_frames: s.dummies as i64,
            unsupported_frames: f2.undecoded as i64,
            other_modcod_frames: f2.other_frames as i64,
            pilot_mismatch: f2.pilot_mismatch as i64,
            bbheader_bad: f2.bbheader_bad as i64,
            net_bit_rate: self.net_rate,
            esn0_db: s.esn0_db,
            mer_db: s.mer_db,
            ebn0_db: ebn0,
            margin_to_qef_db: if s2 {
                s.esn0_db.zip(m).map(|(e, m)| e - m.qef_esn0(s.short))
            } else {
                None
            },
            ldpc_iterations: f2.ldpc_iter,
            bch_ok: f2.bch_ok as i64,
            bch_bad: f2.bch_bad as i64,
            rs_clean: rs.clean as i64,
            rs_corrected: rs.corrected as i64,
            rs_failed: rs.failed as i64,
            carrier_lock: acq.is_some(),
            timing_lock: rs_now > 0.0,
            frame_lock: s.locked,
            fec_lock: ok > 0,
            ts_lock,
            level_dbfs: fr.level_dbfs,
            lnb_mismatch: matches!(
                (self.tx_pol, self.lnb),
                (Polarisation::Vertical, LnbSupply::Horizontal)
                    | (Polarisation::Horizontal, LnbSupply::Vertical)
            ),
        });
        // The carriers: one row each, by offset in kHz. Rows of carriers that left go.
        let key = &mut self.key;
        let (old, now) = (&mut self.carrier_keys, &mut self.carrier_keys_now);
        now.clear();
        for c in &fr.carriers {
            let k = (c.offset_hz / 1e3).round() as i64;
            now.push(k);
            key.clear();
            let _ = write!(key, "{k}");
            DVBS2_CARRIERS.put(
                p,
                key,
                &Dvbs2CarrierRow {
                    offset: Hertz(c.offset_hz),
                    symbol_rate: Hertz(c.rate_hz),
                    roll_off: c.rolloff,
                    snr_db: c.snr_db,
                    level_db: c.level_db,
                    decoded: c.decoded,
                },
            );
        }
        for k in old.iter().filter(|k| !now.contains(k)) {
            key.clear();
            let _ = write!(key, "{k}");
            p.remove_row(DVBS2_CARRIERS.def(), key);
        }
        std::mem::swap(old, now);
        let stream = &mut self.stream_key;
        for (isi, row) in &f2.streams {
            key.clear();
            let _ = write!(key, "{isi}");
            DVBS2_STREAMS.put(
                p,
                key,
                &Dvbs2StreamRow {
                    isi: i64::from(*isi),
                    carries: row.carries,
                    packet_bytes: (row.packet_bytes > 0).then_some(i64::from(row.packet_bytes)),
                    sync_byte: (row.packet_bytes > 0).then_some(i64::from(row.sync)),
                    ccm: row.ccm,
                    frames: row.frames as i64,
                    bits: row.bits as i64,
                },
            );
            stream.clone_from(key);
            for (id, &n) in row.modcods.iter().enumerate().filter(|(_, n)| **n > 0) {
                let Some(m) = ModCod::new(id as u8) else {
                    continue;
                };
                key.clear();
                let _ = write!(key, "{isi}/{id}");
                DVBS2_STREAM_MODCODS.put(
                    p,
                    key,
                    &Dvbs2StreamModcodRow {
                        stream,
                        modcod: id as i64,
                        modulation: m.modulation,
                        code_rate: m.code_rate(),
                        frames: n as i64,
                        share_pct: 100.0 * n as f64 / row.frames.max(1) as f64,
                    },
                );
            }
        }
        let lock_state = if ts_lock {
            LockState::Locked
        } else if acq.is_some() {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let sm = &mut self.summary;
        sm.clear();
        match s.standard {
            None => {
                let _ = write!(
                    sm,
                    "DVB-S/S2: {}  {:.3} Msym/s  SNR {:.1} dB",
                    if acq.is_some() {
                        "carrier found"
                    } else {
                        "searching"
                    },
                    rs_now / 1e6,
                    acq.map_or(0.0, |a| a.carrier.snr_db)
                );
            }
            Some(std) => {
                let _ = write!(
                    sm,
                    "{} {} {}{}{}  {:.3} Msym/s  Es/N0 {:.1} dB  ok {ok}  bad {bad}{}",
                    std.label(),
                    modulation.map_or("", |m| m.name()),
                    rate.map_or("", |r| r.label()),
                    if s2 && s.short { " short" } else { "" },
                    if s2 && s.pilots { " pilots" } else { "" },
                    rs_now / 1e6,
                    s.esn0_db.unwrap_or(0.0),
                    if ts_lock { "  TS lock" } else { "" }
                );
            }
        }
        let link = &mut self.link;
        link.state = if delivering {
            3
        } else if s.locked && s2 {
            2
        } else if acq.is_some() {
            1
        } else {
            0
        };
        link.esn0_db = s.esn0_db;
        link.mer_db = s.mer_db;
        link.offset_hz = offset;
        link.level_dbfs = fr.level_dbfs;
        link.input_rate = fr.input_rate;
        link.symbol_rate = if acq.is_some() { rs_now } else { 0.0 };
        link.modcod = s2.then_some(s.modcod);
        link.signal.clear();
        if s2 {
            let _ = write!(
                link.signal,
                "DVB-S2 {} {}{}{}",
                modulation.map_or("", |m| m.name()),
                rate.map_or("", |r| r.label()),
                if s.short { " short" } else { "" },
                if s.pilots { " pilots" } else { "" }
            );
        }
        link.bch_ok = f2.bch_ok;
        link.bch_bad = f2.bch_bad;
        link.time_s = self.now;
        let r = p.results_mut();
        write_status(
            r,
            lock_state,
            s.esn0_db.map(|v| v as f32),
            Some(fr.level_dbfs as f32),
            Some(offset),
            &self.summary,
        );
        r.update_series(CONSTELLATION, |x| {
            x.fill(false, s.constellation.iter().map(|z| [z.re, z.im]))
        });
        r.update_series(SPECTRUM, |x| {
            x.fill(
                true,
                fr.spectrum.iter().enumerate().map(|(i, &v)| [i as f32, v]),
            )
        });
        r.update_series(MER_HISTORY, |x| {
            x.fill(
                true,
                s.mer_history
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| [i as f32, v]),
            )
        });
    }
}

impl Component for DvbsReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbs.report",
            vec![
                Port::typed::<FrontState>("front"),
                Port::typed::<SyncReport>("sync"),
                Port::typed::<Fec2State>("fec2"),
                Port::typed::<OuterStats>("fec1").optional("no DVB-S counters"),
            ],
            vec![Port::typed::<LinkState>("link").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ReportSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<LinkState>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut fresh = false;
        let results = self.published.results_mut();
        for x in io.items::<FrontState>(0) {
            self.now = self.now.max(x.time);
            for l in &x.value.log {
                results.log(l.clone());
            }
        }
        if let Some(x) = io.items::<FrontState>(0).last() {
            self.front.clone_from(&x.value);
            fresh = true;
        }
        for x in io.items::<SyncReport>(1) {
            for l in &x.value.log_lines {
                results.log(l.clone());
            }
            self.sync.clone_from(&x.value);
            fresh = true;
        }
        for x in io.items::<Fec2State>(2) {
            for l in &x.value.log {
                results.log(l.clone());
            }
            self.fec2.clone_from(&x.value);
            fresh = true;
        }
        if let Some(x) = io.items::<OuterStats>(3).last() {
            self.rs = x.value;
            fresh = true;
        }
        if fresh {
            self.publish();
        }
        if io.tick() {
            io.items_out::<LinkState>(0).push(Timed {
                time: self.now,
                value: self.link.clone(),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ReportSetting::TxPolarisation => self.tx_pol = TX_POLARISATION.read(id, v)?,
            ReportSetting::LnbSupply => self.lnb = LNB_SUPPLY.read(id, v)?,
            ReportSetting::SymbolRate => self.manual_rs = SYMBOL_RATE.read(id, v)?,
            ReportSetting::PlScrambling => self.gold = PL_SCRAMBLING.read(id, v)? as u32,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                ReportSetting::TxPolarisation,
                Value::Choice(self.tx_pol.index()),
            ),
            (ReportSetting::LnbSupply, Value::Choice(self.lnb.index())),
            (ReportSetting::SymbolRate, Value::Hz(self.manual_rs)),
            (
                ReportSetting::PlScrambling,
                Value::I64(i64::from(self.gold)),
            ),
        ])
    }
    /// The carriers and the input streams are of the transponder of the frequency before.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        for t in [
            DVBS2_CARRIERS.def(),
            DVBS2_STREAMS.def(),
            DVBS2_STREAM_MODCODS.def(),
        ] {
            if !kept.table(t.key) {
                self.published.clear_rows(t);
            }
        }
    }
    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The DVB-S2 receiver as a component of another stack.

/// What a stack takes from the DVB-S2 receiver that [`attach`] adds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dvbs2Parts {
    /// The transport stream of the chosen input stream: output `s2_bbframe.ts` ([`TsPacket`](sdrkit_proto_mpegts::stream::TsPacket) items).
    pub ts: bool,
    /// The data field of every good BBFRAME: output `s2_fec.field` ([`DataField`] items).
    pub field: bool,
}

/// The name of the front end in a graph made by [`attach`]. Its input `in` takes the samples.
pub const S2_FRONT: &str = "s2_front";
/// The name of the sync stage.
pub const S2_SYNC: &str = "s2_sync";
/// The name of the FEC. With [`Dvbs2Parts::field`] its output `field` carries the data fields.
pub const S2_FEC: &str = "s2_fec";
/// The name of the mode adaptation (with [`Dvbs2Parts::ts`]). Its output `ts` carries the transport packets.
pub const S2_BBFRAME: &str = "s2_bbframe";
/// The name of the graph stage that begins at the FEC, in a graph made by [`attach`] and in a stack that marks the same
/// boundary.
pub const FEC_STAGE: &str = "fec";
/// The name of the report. It publishes the fields of [`report_declaration`] and sends a [`LinkState`] on `link` at each report.
pub const S2_REPORT: &str = "s2_report";

/// Adds the DVB-S2 receiver to `b`: front end, sync stage (DVB-S2 only), FEC and report, and with `parts.ts` the mode
/// adaptation. The graph input is `s2_front.in`. The edge from the sync stage to the FEC is the boundary of the graph stage
/// [`FEC_STAGE`]; the front end and the sync stage are in the first stage. The stack connects `s2_report.link` and the
/// outputs that `parts` names, declares [`report_declaration`] in its results, and routes its settings with [`routes`].
pub fn attach(b: GraphBuilder, parts: Dvbs2Parts) -> GraphBuilder {
    let fec = if parts.field {
        Dvbs2Fec::with_field()
    } else {
        Dvbs2Fec::default()
    };
    let mut b = b
        .add(S2_FRONT, DvbsFront::default())
        .add(S2_SYNC, DvbsSync::default())
        .add(S2_FEC, fec)
        .add(S2_REPORT, DvbsReport::default())
        .input("s2_front.in")
        .connect("s2_front.symbols", "s2_sync.in")
        .feedback("s2_sync.feedback", "s2_front.feedback")
        .connect("s2_sync.frames", "s2_fec.in")
        .stage(S2_SYNC, S2_FEC, Stage::new(FEC_STAGE))
        .connect("s2_front.state", "s2_report.front")
        .connect("s2_sync.state", "s2_report.sync")
        .connect("s2_fec.state", "s2_report.fec2")
        .set(
            "s2_sync.standard",
            Value::Choice(StandardChoice::Dvbs2.index()),
        );
    if parts.ts {
        b = b
            .add(S2_BBFRAME, BbFrameAdapt::default())
            .connect("s2_fec.out", "s2_bbframe.in")
            .set(
                "s2_bbframe.system",
                Value::Choice(SystemChoice::DvbS2.index()),
            );
    }
    b
}

/// The routes of the receiver's settings in a graph made by [`attach`], as (stack setting key, targets). The stack's schema
/// holds these keys: `symbol_rate`, `roll_off`, `carrier_offset` (a text, as [`settings::CARRIER_PICK`]), `modcod_rule`,
/// `modcod`, `modcods`, `frame_length`, `pilots`, `pl_scrambling`, and `isi` when `parts.ts`.
pub fn routes(parts: Dvbs2Parts) -> Vec<(&'static str, &'static [&'static str])> {
    let mut r: Vec<(&'static str, &'static [&'static str])> = vec![
        (
            "symbol_rate",
            &["s2_front.symbol_rate", "s2_report.symbol_rate"],
        ),
        ("roll_off", &["s2_front.roll_off"]),
        ("carrier_offset", &["s2_front.carrier_pick"]),
        ("modcod_rule", &["s2_fec.modcod_rule"]),
        ("modcod", &["s2_fec.modcod"]),
        ("modcods", &["s2_fec.modcods"]),
        ("frame_length", &["s2_fec.frame_length"]),
        ("pilots", &["s2_fec.pilots"]),
        (
            "pl_scrambling",
            &["s2_sync.pl_scrambling", "s2_report.pl_scrambling"],
        ),
    ];
    if parts.ts {
        r.push(("isi", &["s2_bbframe.stream"]));
    }
    r
}

/// Adds the components of the receiver to a graph registry: `dvbs.front`, `dvbs.sync`, `dvbs.fec2` and `dvbs.report`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("dvbs.front", || Box::new(DvbsFront::default()));
    r.add("dvbs.sync", || Box::new(DvbsSync::default()));
    r.add("dvbs.fec2", || Box::new(Dvbs2Fec::default()));
    r.add("dvbs.report", || Box::new(DvbsReport::default()));
}

/// The highest symbol rate the front end takes at input rate `rate`: `rate` / (1.3 x 1.35), accepted up to 1.02 times that.
pub fn highest_symbol_rate(rate: f64) -> f64 {
    1.02 * rate / (1.3 * 1.35)
}

/// The least input rate for symbol rate `symbol_rate` (the inverse of [`highest_symbol_rate`]).
pub fn least_input_rate(symbol_rate: f64) -> f64 {
    symbol_rate * 1.3 * 1.35 / 1.02
}
