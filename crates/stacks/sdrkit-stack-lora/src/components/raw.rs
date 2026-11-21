//! `loraraw.log`: the frames of `loraraw`. It drops a frame a second detector found again, keeps the frame log, the decoders table
//! and the counters, and builds the frames the user sends.

use super::{count, show_status};
use crate::commands::RESET;
use crate::components::channels::RawPlan;
use crate::mesh::types::{TxKind, TxRequest};
use crate::results::{CrcState, Modulation};
use crate::settings::{RawLogSetting, CLOCK0, TX_REPEAT};
use crate::typed::{
    LoraPacketEvent, RawDecoderRow, RawFields, RawFrameRow, RawSendArgs, LORA_PACKET, RAW_DECODERS,
    RAW_FRAMES, RAW_SEND,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::ResultsDecl;
use sdrkit_core::{
    ConfigError, LockState, Record, Results, SettingError, SettingId, SettingSet, StateBlob,
    StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Port, RateFrom,
    RunContext, Timed,
};
use sdrkit_proto_lora_phy::bank::DetectorState;
use sdrkit_proto_lora_phy::components::BankState;
use sdrkit_proto_lora_phy::rx::LoraFrame;
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    fingerprint, Dedup, Format as StateFormat, LogRing, Seen, Shift, Tables,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::Arc;

/// Frames kept in the log.
pub const LOG_CAP: usize = 500;

/// One row of the frame log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RawRow {
    /// Computer clock (0 when times follow the signal).
    pub wall: f64,
    /// Signal time.
    pub time: f64,
    /// SF.
    pub sf: u8,
    /// Bandwidth, Hz.
    pub bw: f64,
    /// CR.
    pub cr: u8,
    /// CRC present.
    pub crc_present: bool,
    /// CRC right.
    pub crc_ok: bool,
    /// Implicit header.
    pub implicit: bool,
    /// Sync word.
    pub sync: u8,
    /// Length.
    pub len: usize,
    /// Corrected codewords.
    pub corrected: u32,
    /// SNR, dB.
    pub snr_db: f64,
    /// Level, dBFS.
    pub level_db: f64,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// Frequency from the dial (channel offset and carrier offset), Hz.
    pub freq_hz: f64,
    /// Payload bytes.
    #[serde(with = "shared_bytes")]
    pub payload: Arc<[u8]>,
    /// The payload as text for people.
    pub text: String,
}

/// The saved form of shared bytes: a plain byte list.
mod shared_bytes {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S: Serializer>(b: &Arc<[u8]>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(b)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Arc<[u8]>, D::Error> {
        Vec::<u8>::deserialize(d).map(Into::into)
    }
}

impl Shift for RawRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for RawRow {
    type Row = RawFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&RawFrameRow {
            time_s: self.time,
            sf: i64::from(self.sf),
            bw: Hertz(self.bw),
            cr: i64::from(self.cr),
            len: count(self.len),
            crc: if !self.crc_present {
                CrcState::None
            } else if self.crc_ok {
                CrcState::Ok
            } else {
                CrcState::Bad
            },
            snr_db: self.snr_db,
            level_dbfs: self.level_db,
            cfo: Hertz(self.cfo_hz),
            payload: Arc::clone(&self.payload),
            text: &self.text,
        });
    }
}

/// Text of a payload: bytes 32 to 126 as they are, others ".".
pub fn text_of(b: &[u8]) -> String {
    b.iter()
        .map(|&c| {
            if (32..127).contains(&c) {
                c as char
            } else {
                '.'
            }
        })
        .collect()
}

/// What `loraraw` heard: the log and the counters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawHeard {
    /// The log, oldest first.
    pub log: LogRing<RawRow>,
    /// Good frames (good CRC or none).
    pub good: u64,
    /// Bad frames.
    pub bad: u64,
    /// Good frames per SF.
    pub per_sf: [u64; 8],
    /// Good frames at 125, 250 and 500 kHz.
    pub per_bw: [u64; 3],
}

impl Default for RawHeard {
    fn default() -> Self {
        RawHeard {
            log: LogRing::new(LOG_CAP),
            good: 0,
            bad: 0,
            per_sf: [0; 8],
            per_bw: [0; 3],
        }
    }
}

impl Shift for RawHeard {
    fn shift_times(&mut self, by_s: f64) {
        self.log.shift_times(by_s);
    }
}

impl Tables for RawHeard {
    const FORMAT: StateFormat = StateFormat::new("lora.raw_log", 1);
    fn clear(&mut self) {
        self.log.clear();
        self.good = 0;
        self.bad = 0;
        self.per_sf = [0; 8];
        self.per_bw = [0; 3];
    }
    fn adopt(&mut self, saved: Self) {
        self.log.adopt(saved.log);
        self.good = saved.good;
        self.bad = saved.bad;
        self.per_sf = saved.per_sf;
        self.per_bw = saved.per_bw;
    }
}

/// The `loraraw.log` component.
pub struct RawLog {
    /// What was heard.
    pub heard: RawHeard,
    recent: Dedup<u64>,
    plan: RawPlan,
    preambles: u64,
    header_failed: u64,
    last_good: f64,
    now: f64,
    level: Option<f64>,
    /// The signal to noise ratio and the carrier offset of the last frame, until a change of frequency.
    last: Option<(f64, f64)>,
    clock0: f64,
    det: Vec<DetectorState>,
    p: Published,
    tx_enabled: bool,
    tx_repeat: f64,
    tx_payload: Option<Vec<u8>>,
    tx_next: f64,
    next_id: u64,
    sent: u64,
    text: String,
    key: String,
}

impl Default for RawLog {
    fn default() -> Self {
        RawLog {
            heard: RawHeard::default(),
            recent: Dedup::new(0.25, 12),
            plan: RawPlan::default(),
            preambles: 0,
            header_failed: 0,
            last_good: -1e9,
            now: 0.0,
            level: None,
            last: None,
            clock0: 0.0,
            det: Vec::new(),
            p: Published::from_decl(Arc::new(
                ResultsDecl::new()
                    .with_fields(RawFields::SHAPE.fields)
                    .with_table(RAW_FRAMES.def())
                    .with_table(RAW_DECODERS.def()),
            )),
            tx_enabled: false,
            tx_repeat: 0.0,
            tx_payload: None,
            tx_next: 0.0,
            next_id: 1,
            sent: 0,
            text: String::new(),
            key: String::new(),
        }
    }
}

impl RawLog {
    /// Good frames at spreading factor `sf`.
    pub fn per_sf(&self, sf: u8) -> u64 {
        sf.checked_sub(5)
            .and_then(|i| self.heard.per_sf.get(i as usize))
            .copied()
            .unwrap_or(0)
    }

    fn publish(&mut self) {
        let h = &self.heard;
        let state = self.now - self.last_good < 5.0;
        let last = self.last;
        let t = &mut self.text;
        t.clear();
        if h.good + h.bad == 0 {
            t.push_str(&self.plan.line);
        } else {
            let _ = write!(t, "{} frames, {} CRC errors", h.good, h.bad);
        }
        let p = &mut self.p;
        show_status(
            p,
            if state {
                LockState::Locked
            } else {
                LockState::Searching
            },
            last.map(|f| f.0),
            last.map(|f| f.1),
            self.level,
            t,
        );
        let (sf, bw) = (h.per_sf.map(count), h.per_bw.map(count));
        p.put(&RawFields {
            level_dbfs: self.level,
            offset_hz: last.map(|f| Hertz(f.1)),
            frames_ok: count(h.good),
            frames_bad: count(h.bad),
            preambles: count(self.preambles),
            header_failed: count(self.header_failed),
            searching: &self.plan.line,
            dial: Hertz(self.plan.dial_hz),
            tx_sent: count(self.sent),
            sf5: sf[0],
            sf6: sf[1],
            sf7: sf[2],
            sf8: sf[3],
            sf9: sf[4],
            sf10: sf[5],
            sf11: sf[6],
            sf12: sf[7],
            bw125: bw[0],
            bw250: bw[1],
            bw500: bw[2],
            snr_db: last.map(|f| f.0),
        });
        shaped::log_rows(&h.log, &RAW_FRAMES, self.now, p);
        let key = &mut self.key;
        for c in &self.plan.bank.channels {
            for d in &c.detectors {
                let st = self.det.iter().find(|s| s.tag == d.tag);
                let s = st.map(|s| s.stats).unwrap_or_default();
                key.clear();
                let _ = write!(key, "{:03}", d.tag);
                t.clear();
                for (i, w) in d.syncs.iter().enumerate() {
                    let _ = write!(t, "{}0x{w:02X}", if i > 0 { " " } else { "" });
                }
                RAW_DECODERS.put(
                    p,
                    key,
                    &RawDecoderRow {
                        sf: i64::from(d.sf),
                        bw: Hertz(c.bw),
                        syncs: t.as_str(),
                        in_band: st.is_some_and(|s| s.in_band),
                        preambles: count(s.preambles),
                        frames: count(s.frames),
                        crc_errors: count(s.crc_failed),
                        header_failed: count(s.header_failed),
                        sync_failed: count(s.sync_failed),
                    },
                );
            }
        }
    }

    fn transmit(&mut self, io: &mut Io) {
        if !self.tx_enabled || self.now < self.tx_next {
            return;
        }
        let Some(payload) = self.tx_payload.clone() else {
            return;
        };
        let req = TxRequest {
            id: self.next_id,
            payload,
            lora: Some(self.plan.tx.clone()),
            afsk: None,
            freq_hz: self.plan.tx_hz,
            not_before: self.now,
            cw: 3,
            what: TxKind::Raw,
            message_key: None,
        };
        self.next_id += 1;
        self.sent += 1;
        io.items_out::<TxRequest>(1).push(Timed {
            time: self.now,
            value: req,
        });
        if self.tx_repeat > 0.0 {
            self.tx_next = self.now + self.tx_repeat;
        } else {
            self.tx_payload = None;
        }
    }
}

impl Component for RawLog {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "loraraw.log",
            vec![
                Port::typed::<LoraFrame>("frames"),
                Port::typed::<BankState>("state"),
                Port::typed::<RawPlan>("plan"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<TxRequest>("tx").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(RawLogSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<TxRequest>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.tx_enabled = ctx.transmit;
        // A frame that repeats ends with the transmitter. It does not start again by itself.
        if !ctx.transmit {
            self.tx_payload = None;
        }
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(pl) = io.items::<RawPlan>(2).last() {
            let pl = pl.value.clone();
            if pl.line != self.plan.line {
                self.p
                    .results_mut()
                    .log(format!("LoRa (raw): searching {}", pl.line));
            }
            if pl.bank != self.plan.bank {
                self.p.clear_rows(&RAW_DECODERS);
            }
            self.plan = pl;
        }
        for s in io.items::<BankState>(1) {
            self.now = s.value.time;
            self.level = Some(s.value.level_db);
            self.preambles = s.value.detectors.iter().map(|d| d.stats.preambles).sum();
            self.header_failed = s
                .value
                .detectors
                .iter()
                .map(|d| d.stats.header_failed)
                .sum();
            self.det.clone_from(&s.value.detectors);
        }
        for f in io.items::<LoraFrame>(0) {
            let fr = &f.value;
            // Another detector reporting the same payload within 0.25 s locked on the same frame.
            if self.recent.seen(fingerprint(&fr.payload), f.time) {
                continue;
            }
            let good = fr.good();
            let h = &mut self.heard;
            self.last = Some((fr.snr_db, fr.cfo_hz));
            if good {
                h.good += 1;
                self.last_good = f.time;
                if let Some(c) = fr
                    .sf
                    .checked_sub(5)
                    .and_then(|i| h.per_sf.get_mut(i as usize))
                {
                    *c += 1;
                }
                let bi = match (fr.bw / 1e3).round() as u64 {
                    125 => Some(0),
                    250 => Some(1),
                    500 => Some(2),
                    _ => None,
                };
                if let Some(i) = bi {
                    h.per_bw[i] += 1;
                }
            } else {
                h.bad += 1;
            }
            let row = RawRow {
                wall: if self.clock0 > 0.0 {
                    self.clock0 + f.time
                } else {
                    0.0
                },
                time: f.time,
                sf: fr.sf,
                bw: fr.bw,
                cr: fr.cr,
                crc_present: fr.crc_present,
                crc_ok: fr.crc_ok,
                implicit: fr.implicit,
                sync: fr.sync,
                len: fr.payload.len(),
                corrected: fr.corrected,
                snr_db: fr.snr_db,
                level_db: fr.level_db,
                cfo_hz: fr.cfo_hz,
                freq_hz: fr.channel_hz - self.plan.dial_hz + fr.cfo_hz,
                payload: fr.payload.as_slice().into(),
                text: text_of(&fr.payload),
            };
            if good {
                io.messages_out(0).push(LORA_PACKET.event(
                    f.time,
                    fr.channel_hz + fr.cfo_hz,
                    &LoraPacketEvent {
                        net: crate::mesh::types::Protocol::Lora,
                        modulation: Modulation::Lora,
                        snr_db: fr.snr_db,
                        sf: Some(i64::from(fr.sf)),
                        payload: Arc::clone(&row.payload),
                        crc_ok: good,
                        source: None,
                        target: None,
                        devaddr: None,
                        iq_inverted: None,
                        crc: Some(fr.crc_present),
                        bw: Some(Hertz(fr.bw)),
                        cr: Some(i64::from(fr.cr)),
                        implicit: Some(fr.implicit),
                        sync: Some(i64::from(fr.sync)),
                        level_dbfs: Some(fr.level_db),
                        family: keys::Family::Other,
                    },
                ));
            }
            self.heard.log.push(row);
        }
        self.transmit(io);
        if io.tick() {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RawLogSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RawLogSetting::TxRepeat => self.tx_repeat = TX_REPEAT.read(id, v)?,
            RawLogSetting::Clock0 => self.clock0 = CLOCK0.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (RawLogSetting::TxRepeat, Value::Duration(self.tx_repeat)),
            (RawLogSetting::Clock0, Value::F64(self.clock0)),
        ])
    }

    fn commands(&self) -> &Commands {
        RawLogCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match RawLogCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RawLogCommand::Clear => {
                self.heard.clear();
                self.last = None;
                self.p.clear_rows(&RAW_FRAMES);
            }
            RawLogCommand::Reset => {
                self.heard.good = 0;
                self.heard.bad = 0;
                self.heard.per_sf = [0; 8];
                self.heard.per_bw = [0; 3];
            }
            RawLogCommand::Send => {
                let RawSendArgs { payload: b } = RAW_SEND.args(args)?;
                if b.is_empty() || b.len() > 255 {
                    let id = args
                        .schema()
                        .id(RawSendArgs::PAYLOAD.field.key)
                        .unwrap_or(SettingId(0));
                    return Err(CommandError::BadArgument {
                        arg: id,
                        error: SettingError::NotPermitted { id },
                    });
                }
                if self.tx_enabled {
                    self.tx_payload = Some(b.to_vec());
                    self.tx_next = self.now;
                } else {
                    let line = format!("LoRa (raw): {}", crate::mesh::types::TRANSMIT_OFF);
                    self.p.results_mut().log(line);
                }
            }
        }
        // What is shown changes at once, not at the next report.
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn forget(&mut self, kept: &Kept) {
        let h = &mut self.heard;
        if !kept.table(RAW_FRAMES.key) {
            h.log.clear();
            self.p.clear_rows(&RAW_FRAMES);
        }
        if !kept.field("frames_ok") {
            h.good = 0;
        }
        if !kept.field("frames_bad") {
            h.bad = 0;
        }
        if !kept.field("sf5") {
            h.per_sf = [0; 8];
        }
        if !kept.field("bw125") {
            h.per_bw = [0; 3];
        }
        if !kept.field("tx_sent") {
            self.sent = 0;
        }
        self.level = None;
        self.last = None;
        self.last_good = -1e9;
        self.p.clear_rows(&RAW_DECODERS);
        self.publish();
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.now).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let now = self.now;
        self.heard.restore_state(state, now)
    }
}

sdrkit_core::commands! {
    /// The commands of `loraraw.log`.
    pub enum RawLogCommand {
        /// Clear.
        Clear = CLEAR,
        /// Reset.
        Reset = RESET,
        /// Send a frame.
        Send = RAW_SEND.command,
    }
}
