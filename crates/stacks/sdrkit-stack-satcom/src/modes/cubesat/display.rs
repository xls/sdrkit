//! The CubeSat display: the frame list, the counts by framing, the smart search's carriers, and the frame and KISS events.

use super::demod::{Framing, FramingCounts, Submode};
use super::{CsItem, Named};
use crate::display::Clock;
use crate::typed::{
    CubesatCarrierRow, CubesatFields, CubesatFrameEvent, CubesatFrameRow, CubesatFramingRow,
    CubesatKissEvent, CUBESAT_CARRIERS, CUBESAT_FRAME, CUBESAT_FRAMES, CUBESAT_FRAMES_PER_FRAMING,
    CUBESAT_KISS,
};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, ConfigError, Event, LockState, ResultsDecl, Schema, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Kind, Outbox, Port, RateFrom, RunContext, Timed};
use sdrkit_proto_link::kiss::{self, Command};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp, Tally};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The most frames kept.
pub const FRAMES_CAP: usize = 300;
/// A frame keeps the lock state at Locked this long, seconds.
const LOCK_HOLD_S: f64 = 60.0;

/// What each burst framing receives that no public document confirms.
const NOTES: [(Submode, &str); 9] = [
    (
        Submode::Geoscan,
        "Geoscan: the frame layout (66 bytes, PN9 whitening over every byte, CRC-16/CMS over 64 bytes) and the 4 wrong sync bits allowed are not confirmed by a public document; the 74-byte layout is a guess",
    ),
    (
        Submode::Lucky7,
        "Lucky-7: the frame, the whitening, the CRC range, the kind byte and the correlation rule are not confirmed by a public document",
    ),
    (
        Submode::Spino,
        "Spino: the frame and its header are not confirmed by a public document",
    ),
    (
        Submode::Erminaz,
        "Erminaz: which layout the satellite sends (255 bytes in the dual basis, or 160 bytes in the conventional basis) is not confirmed by a public document",
    ),
    (
        Submode::Usp,
        "USP: the meaning of data bytes 0 and 1 and the 13 wrong sync bits allowed are not confirmed by a public document",
    ),
    (
        Submode::Ngham,
        "NGHam: the size tags, the padding rule, the first root 112 of the code with 16 check bytes and the variant with zero check bytes are not confirmed by a public document",
    ),
    (
        Submode::U482c,
        "U482C: the length field, its flags and the 4 wrong sync bits allowed are not confirmed by a public document; frames without Reed-Solomon are not taken",
    ),
    (
        Submode::Mobitex,
        "Mobitex: the meaning of the control bytes and of the block data, and the BEESAT-1 header, are not confirmed by a public document",
    ),
    (
        Submode::CcsdsConcat,
        "CCSDS concatenated: the 4 wrong sync bits allowed are not confirmed by a public document",
    ),
];

/// The note of `sub`: the parts its framings take that no public document confirms.
pub fn note(sub: Submode) -> String {
    NOTES
        .iter()
        .filter(|(s, _)| sub == Submode::All || *s == sub)
        .map(|(_, t)| *t)
        .collect::<Vec<_>>()
        .join(". ")
        + "."
}

/// What the display publishes.
pub fn results_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(CubesatFields::SHAPE.fields)
                .with_table(&CUBESAT_FRAMES)
                .with_table(&CUBESAT_FRAMES_PER_FRAMING)
                .with_table(&CUBESAT_CARRIERS),
        )
    })
    .clone()
}

sdrkit_core::choice_enum! {
    /// The framing the frame list shows.
    pub enum FramingFilter {
        /// Every framing.
        All = ("all", "Every framing"),
        /// AX.25 AFSK.
        Afsk = ("ax25_afsk", "AX.25 AFSK"),
        /// AX.25 G3RUH.
        G3ruh = ("ax25_g3ruh", "AX.25 G3RUH"),
        /// AX.25 NRZI.
        Nrzi = ("ax25_nrzi", "AX.25 NRZI"),
        /// AX100 Mode 5.
        Mode5 = ("ax100_mode5", "AX100 Mode 5"),
        /// AX100 Mode 6.
        Mode6 = ("ax100_mode6", "AX100 Mode 6"),
        /// CCSDS RS.
        Ccsds = ("ccsds_rs", "CCSDS RS"),
        /// Geoscan.
        Geoscan = ("geoscan", "Geoscan"),
        /// Lucky-7.
        Lucky7 = ("lucky7", "Lucky-7"),
        /// Spino.
        Spino = ("spino", "Spino"),
        /// Erminaz.
        Erminaz = ("erminaz", "Erminaz"),
        /// USP.
        Usp = ("usp", "USP"),
        /// NGHam.
        Ngham = ("ngham", "NGHam"),
        /// U482C.
        U482c = ("u482c", "U482C"),
        /// Mobitex, Mobitex-NX, BEESAT-1 and BEESAT-9.
        Mobitex = ("mobitex", "Mobitex"),
        /// CCSDS concatenated.
        CcsdsConcat = ("ccsds_concat", "CCSDS concatenated"),
    }
}

impl FramingFilter {
    /// The filter lets frames of `f` through.
    fn passes(self, f: Framing) -> bool {
        match self {
            FramingFilter::All => true,
            FramingFilter::Afsk => f == Framing::Afsk,
            FramingFilter::G3ruh => f == Framing::G3ruh,
            FramingFilter::Nrzi => f == Framing::Nrzi,
            FramingFilter::Mode5 => f == Framing::Mode5,
            FramingFilter::Mode6 => f == Framing::Mode6,
            FramingFilter::Ccsds => f == Framing::Ccsds,
            FramingFilter::Geoscan => f == Framing::Geoscan,
            FramingFilter::Lucky7 => f == Framing::Lucky7,
            FramingFilter::Spino => f == Framing::Spino,
            FramingFilter::Erminaz => f == Framing::Erminaz,
            FramingFilter::Usp => f == Framing::Usp,
            FramingFilter::Ngham => f == Framing::Ngham,
            FramingFilter::U482c => f == Framing::U482c,
            FramingFilter::Mobitex => matches!(
                f,
                Framing::Mobitex | Framing::MobitexNx | Framing::Beesat1 | Framing::Beesat9
            ),
            FramingFilter::CcsdsConcat => f == Framing::CcsdsConcat,
        }
    }
}

/// Send each frame to the KISS output.
pub const KISS: Setting<field::Bool> = Setting::new("kiss", "KISS output", "", true);
/// The framing the frame list shows.
pub const FRAMING_FILTER: Setting<field::Choice<FramingFilter>> =
    Setting::choice("framing_filter", "Framing", FramingFilter::All);

sdrkit_core::settings! {
    /// The settings of [`CsDisplay`].
    pub enum CsDisplaySetting {
        /// KISS output.
        Kiss = KISS,
        /// Framing.
        FramingFilter = FRAMING_FILTER,
    }
}

/// One frame of the list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// When it was heard.
    pub at: Stamp,
    /// The framing.
    pub framing: Framing,
    /// The bit rate.
    pub baud: u32,
    /// Reed-Solomon symbols corrected.
    pub rs_corrected: Option<u32>,
    /// The satellite.
    pub satellite: String,
    /// Its NORAD number.
    pub norad: u32,
    /// The name comes from the dial.
    pub guess: bool,
    /// Absolute carrier frequency, Hz.
    pub freq_hz: f64,
    /// AX.25 source.
    pub from: String,
    /// AX.25 destination.
    pub to: String,
    /// AX.25 repeaters.
    pub path: String,
    /// AX.25 frame type.
    pub frame_type: String,
    /// AX.25 PID.
    pub pid: Option<u8>,
    /// The CSP header in words.
    pub csp: String,
    /// The data.
    pub data: Arc<[u8]>,
    /// The payload.
    pub payload: Arc<[u8]>,
    /// A text beacon.
    pub text: Option<String>,
    /// The carrier's SNR, dB.
    pub snr_db: Option<f64>,
    /// What passed.
    #[serde(default)]
    pub validated: String,
    /// The Lucky-7 kind.
    #[serde(default)]
    pub kind: Option<String>,
    /// The sync word was found complemented.
    #[serde(default)]
    pub inverted: bool,
    /// The spacecraft id of a CCSDS header.
    #[serde(default)]
    pub spacecraft: Option<u16>,
    /// The virtual channel of a CCSDS header.
    #[serde(default)]
    pub vc: Option<u8>,
    /// The NGHam flags.
    #[serde(default)]
    pub flags: Option<u8>,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl FrameRow {
    /// The row of the frame table.
    fn shaped(&self) -> CubesatFrameRow<'_> {
        fn text(s: &str) -> Option<&str> {
            (!s.is_empty()).then_some(s)
        }
        // A burst framing without Reed-Solomon, or whose CRC alone decided, reads -1.
        let rs = self.rs_corrected.map(i64::from);
        CubesatFrameRow {
            heard_at: self.at.wall.map(Time),
            framing: self.framing,
            baud: i64::from(self.baud),
            rs_corrected: rs.or(self.framing.is_burst().then_some(-1)),
            satellite: text(&self.satellite),
            norad: i64::from(self.norad),
            guess: self.guess,
            freq_hz: Hertz(self.freq_hz),
            from: text(&self.from),
            to: text(&self.to),
            path: text(&self.path),
            frame_type: text(&self.frame_type),
            pid: self.pid.map(i64::from),
            csp: text(&self.csp),
            data: self.data.clone(),
            payload: self.payload.clone(),
            text: self.text.as_deref(),
            snr_db: self.snr_db,
            validated: text(&self.validated),
            kind: self.kind.as_deref(),
            inverted: self.inverted,
            spacecraft: self.spacecraft.map(i64::from),
            vc: self.vc.map(i64::from),
            flags: self.flags.map(i64::from),
        }
    }
}

impl Shaped for FrameRow {
    type Row = CubesatFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&self.shaped());
    }
}

/// What the display keeps over a restart and saves.
#[derive(Serialize, Deserialize)]
pub struct CsTables {
    /// The frames, newest last.
    pub frames: LogRing<FrameRow>,
    /// Frames by framing.
    pub per_framing: Tally<Framing>,
    /// Frames decoded.
    pub good: u64,
    /// Frames the Reed-Solomon code repaired.
    pub rs_repaired: u64,
    /// Frames sent to the KISS output.
    pub kiss_sent: u64,
}

impl Default for CsTables {
    fn default() -> Self {
        CsTables {
            frames: LogRing::new(FRAMES_CAP),
            per_framing: Tally::new(32),
            good: 0,
            rs_repaired: 0,
            kiss_sent: 0,
        }
    }
}

impl Shift for CsTables {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
    }
}

impl Tables for CsTables {
    const FORMAT: StateFormat = StateFormat::new("satcom.cubesat", 1);
    fn clear(&mut self) {
        *self = CsTables {
            frames: LogRing::new(self.frames.cap()),
            ..CsTables::default()
        };
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.per_framing = saved.per_framing;
        self.good = saved.good;
        self.rs_repaired = saved.rs_repaired;
        self.kiss_sent = saved.kiss_sent;
    }
}

/// The display decoder (`satcom.cubesat_display`).
pub struct CsDisplay {
    tables: CsTables,
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    kiss_on: bool,
    filter: FramingFilter,
    carrier: bool,
    offset_hz: f64,
    carriers: Vec<(f64, f64)>,
    counts: Vec<(Framing, FramingCounts)>,
    note: String,
    kiss_buf: Vec<u8>,
    /// The frame tables changed since the last report.
    dirty: bool,
}

impl Default for CsDisplay {
    fn default() -> Self {
        CsDisplay::new(Submode::All)
    }
}

impl CsDisplay {
    /// The display of `sub`.
    pub fn new(sub: Submode) -> Self {
        CsDisplay {
            tables: CsTables::default(),
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            kiss_on: true,
            filter: FramingFilter::All,
            carrier: false,
            offset_hz: 0.0,
            carriers: Vec::new(),
            counts: Vec::new(),
            note: note(sub),
            kiss_buf: Vec::new(),
            dirty: true,
        }
    }

    fn frame(&mut self, item_t: f64, n: &Named, events: &mut Vec<Event>) {
        let Some(raw) = &n.raw else { return };
        let t = self.clock.at(item_t);
        let row = FrameRow {
            at: self.clock.stamp(t),
            framing: raw.framing,
            baud: raw.baud,
            rs_corrected: raw.rs_corrected,
            satellite: n.satellite.clone(),
            norad: n.norad,
            guess: n.guess,
            freq_hz: n.freq_hz,
            from: n.from.clone(),
            to: n.to.clone(),
            path: n.path.clone(),
            frame_type: n.frame_type.clone(),
            pid: n.pid,
            csp: n.csp.clone(),
            data: Arc::from(raw.data.as_slice()),
            payload: Arc::from(n.payload.as_slice()),
            text: n.text.clone(),
            snr_db: n.snr_db,
            validated: raw.validated.to_string(),
            kind: raw.kind.map(String::from),
            inverted: raw.inverted,
            spacecraft: raw.spacecraft,
            vc: raw.vc,
            flags: raw.flags,
        };
        self.dirty = true;
        let tb = &mut self.tables;
        tb.good += 1;
        tb.per_framing.add(row.framing);
        if raw.rs_corrected.is_some_and(|c| c > 0) {
            tb.rs_repaired += 1;
        }
        let source = (!row.satellite.is_empty()).then_some(row.satellite.as_str());
        if self.kiss_on {
            self.kiss_buf.clear();
            if kiss::encode(0, Command::Data, &raw.data, &mut self.kiss_buf).is_ok() {
                tb.kiss_sent += 1;
                events.push(CUBESAT_KISS.event(
                    t,
                    row.freq_hz,
                    &CubesatKissEvent {
                        source,
                        family: Family::Satellite,
                        kiss_frame: Arc::from(self.kiss_buf.as_slice()),
                    },
                ));
            }
        }
        events.push(CUBESAT_FRAME.event(
            t,
            row.freq_hz,
            &CubesatFrameEvent {
                source,
                text: row.text.as_deref(),
                snr_db: row.snr_db.filter(|s| s.is_finite()),
                family: Family::Satellite,
                framing: row.framing,
                norad: i64::from(row.norad),
                data: row.data.clone(),
            },
        ));
        tb.frames.push(row);
    }
}

impl Decoder for CsDisplay {
    type Input = Timed<CsItem>;
    fn name(&self) -> &'static str {
        "satcom.cubesat_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        CsDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (CsDisplaySetting::Kiss, Value::Bool(self.kiss_on)),
            (
                CsDisplaySetting::FramingFilter,
                Value::Choice(self.filter.index()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match CsDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CsDisplaySetting::Kiss => self.kiss_on = KISS.read(id, v)?,
            CsDisplaySetting::FramingFilter => {
                self.filter = FRAMING_FILTER.read(id, v)?;
                self.dirty = true;
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(results_decl())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Timed<CsItem>], _first: u64, out: &mut Outbox) {
        let events = out.messages(0);
        for it in x {
            match &it.value {
                CsItem::Named(n) => self.frame(it.time, n, events),
                CsItem::Status {
                    carrier,
                    offset_hz,
                    carriers,
                    counts,
                } => {
                    self.clock.at(it.time);
                    self.carrier = *carrier;
                    self.offset_hz = *offset_hz;
                    self.carriers.clone_from(carriers);
                    if *counts != self.counts {
                        self.counts.clone_from(counts);
                        self.dirty = true;
                    }
                }
                CsItem::Raw { .. } => {}
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let tb = &self.tables;
        p.put(&CubesatFields {
            frames_ok: tb.good as i64,
            rs_repaired: tb.rs_repaired as i64,
            kiss_sent: tb.kiss_sent as i64,
            note: self.note.as_str(),
        });
        if std::mem::take(&mut self.dirty) {
            if self.filter == FramingFilter::All {
                shaped::log_rows(&tb.frames, &CUBESAT_FRAMES, now, p);
            } else {
                p.clear_rows(&CUBESAT_FRAMES);
                let mut key = String::new();
                for (seq, f) in tb.frames.iter().enumerate() {
                    if !self.filter.passes(f.framing) {
                        continue;
                    }
                    key.clear();
                    let _ = write!(key, "{seq:06}");
                    CUBESAT_FRAMES.put(p, &key, &f.shaped());
                }
            }
            if tb.per_framing.iter().next().is_none() && self.counts.is_empty() {
                p.clear_rows(&CUBESAT_FRAMES_PER_FRAMING);
            }
            // The frames of each framing, with the counts of the framings that the receiver reports.
            let counted = |framing: &Framing| {
                self.counts
                    .iter()
                    .find(|(f, _)| f == framing)
                    .map(|(_, c)| c)
            };
            let put = |p: &mut Published, framing: Framing| {
                let c = counted(&framing);
                CUBESAT_FRAMES_PER_FRAMING.put(
                    p,
                    &Framing::CHOICES[framing.index() as usize].key,
                    &CubesatFramingRow {
                        framing,
                        frames: tb.per_framing.get(&framing) as i64,
                        sync_hits: c.map(|c| c.sync_hits as i64),
                        decode_failed: c.map(|c| c.decode_failed as i64),
                    },
                );
            };
            for (framing, _) in tb.per_framing.iter() {
                put(p, *framing);
            }
            for (framing, _) in &self.counts {
                if tb.per_framing.iter().all(|(f, _)| f != framing) {
                    put(p, *framing);
                }
            }
        }
        shaped::indexed_rows(
            p,
            &CUBESAT_CARRIERS,
            &self.carriers,
            |(off, snr), out| {
                out.put(&CubesatCarrierRow {
                        offset_hz: Hertz(*off),
                        snr_db: *snr,
                    },
                );
            },
        );
        let tb = &self.tables;
        let newest = tb.frames.newest_first().next();
        let recent = newest.is_some_and(|f| now - f.at.t < LOCK_HOLD_S);
        let state = if recent {
            2
        } else if self.carrier {
            1
        } else {
            0
        };
        let mut summary = String::new();
        let _ = match newest {
            Some(f) if recent => write!(
                summary,
                "{} frames, last {} ({})",
                tb.good,
                if f.satellite.is_empty() {
                    "unknown satellite"
                } else {
                    &f.satellite
                },
                Framing::CHOICES[f.framing.index() as usize]
                    .label
                    .trim_start_matches("AX.25 ")
            ),
            _ if self.carrier => write!(summary, "signal, no frame yet"),
            _ => write!(summary, "listening"),
        };
        let lock_state = LockState::from_level(state);
        let offset = Some(self.offset_hz);
        let snr = newest.and_then(|f| f.snr_db).map(|s| s as f32);
        p.results_mut().update_status(|st| {
            let same = st.lock_state == lock_state
                && st.offset_hz == offset
                && st.snr_db == snr
                && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.offset_hz = offset;
                st.snr_db = snr;
                st.summary = summary;
            }
            !same
        });
    }
    fn clear(&mut self) {
        self.tables.clear();
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.tables.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.tables.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}
