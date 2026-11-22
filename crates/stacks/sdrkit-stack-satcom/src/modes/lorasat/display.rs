//! The LoRa satellite display: the frame log, the Doppler curve of the pass, the detector counters and the frame events.

use super::doppler_limits;
use super::rx::{DemodRow, LsFrame, LsItem};
use crate::display::{set_series, Clock};
use crate::typed::{
    LoraSatDemodulatorRow, LoraSatDemodulatorSyncRow, LoraSatFields, LoraSatFrameEvent,
    LoraSatFrameRow, LoraSatSfRow, LORA_SAT_DEMODULATORS, LORA_SAT_DEMODULATOR_SYNCS,
    LORA_SAT_DOPPLER, LORA_SAT_FRAME, LORA_SAT_FRAMES, LORA_SAT_FRAMES_PER_SF,
};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    ConfigError, Event, LockState, ResultsDecl, Schema, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Kind, Outbox, Port, RateFrom, RunContext, Timed};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp, Tally};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The most frames kept.
pub const FRAMES_CAP: usize = 400;
/// The most points of the Doppler curve kept.
pub const CURVE_CAP: usize = 600;
/// A good frame keeps the lock state at Locked this long, seconds.
const LOCK_HOLD_S: f64 = 30.0;
/// A frame of the curve this recent provides the carrier rate of the next frame, seconds.
const CURVE_WINDOW_S: (f64, f64) = (0.2, 20.0);

sdrkit_core::choice_enum! {
    /// The CRC of a frame.
    pub enum CrcState {
        /// No CRC was sent.
        None = ("none", "None"),
        /// The CRC checks.
        Ok = ("ok", "Good"),
        /// The CRC fails.
        Bad = ("bad", "Bad"),
    }
}

/// What the display publishes.
pub fn results_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(LoraSatFields::SHAPE.fields)
                .with_table(&LORA_SAT_FRAMES)
                .with_table(&LORA_SAT_DEMODULATORS)
                .with_table(&LORA_SAT_DEMODULATOR_SYNCS)
                .with_table(&LORA_SAT_FRAMES_PER_SF)
                .series(
                    LORA_SAT_DOPPLER.key,
                    LORA_SAT_DOPPLER.label,
                    LORA_SAT_DOPPLER.x_unit,
                    LORA_SAT_DOPPLER.y_unit,
                ),
        )
    })
    .clone()
}

/// The pass planner's name of the satellite overhead.
pub const PASS_SATELLITE: Setting<field::Text> =
    Setting::new("pass_satellite", "Satellite overhead", "", "")
        .help("The pass planner's (the host's) name of the satellite overhead.");

sdrkit_core::settings! {
    /// The settings of [`LoraSatDisplay`].
    pub enum LoraSatDisplaySetting {
        /// Satellite overhead.
        PassSatellite = PASS_SATELLITE,
    }
}

/// One frame of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// When it was heard.
    pub at: Stamp,
    /// Spreading factor.
    pub sf: u8,
    /// Bandwidth, Hz.
    pub bw_hz: f64,
    /// Coding rate denominator (4/cr).
    pub cr: u8,
    /// A CRC was sent.
    pub crc_present: bool,
    /// The CRC checks.
    pub crc_ok: bool,
    /// Sync word.
    pub sync: u8,
    /// SNR, dB.
    pub snr_db: f64,
    /// Carrier offset from the dial at the first data symbol, Hz.
    pub offset_hz: f64,
    /// Carrier rate, Hz/s, when it is known.
    pub rate_hz_s: Option<f64>,
    /// Absolute carrier frequency, Hz.
    pub freq_hz: f64,
    /// Satellite overhead.
    pub satellite: String,
    /// Payload.
    pub payload: Arc<[u8]>,
    /// Payload as printable text.
    pub text: String,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Shaped for FrameRow {
    type Row = LoraSatFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let crc = if !self.crc_present {
            CrcState::None
        } else if self.crc_ok {
            CrcState::Ok
        } else {
            CrcState::Bad
        };
        out.put(&LoraSatFrameRow {
            heard_at: self.at.wall.map(Time),
            sf: i64::from(self.sf),
            bw_hz: Hertz(self.bw_hz),
            cr: i64::from(self.cr),
            crc,
            sync: i64::from(self.sync),
            bytes: self.payload.len() as i64,
            snr_db: self.snr_db,
            doppler_hz: Hertz(self.offset_hz),
            doppler_rate_hz_s: self.rate_hz_s,
            freq_hz: Hertz(self.freq_hz),
            satellite: (!self.satellite.is_empty()).then_some(self.satellite.as_str()),
            payload: self.payload.clone(),
            text: &self.text,
        });
    }
}

/// A point of the Doppler curve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    /// Signal time, seconds.
    pub t: f64,
    /// Carrier offset from the dial, Hz.
    pub offset_hz: f64,
    /// Carrier rate, Hz/s, when it is known.
    pub rate_hz_s: Option<f64>,
}

impl Shift for CurvePoint {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// What the display keeps over a restart and saves.
#[derive(Serialize, Deserialize)]
pub struct LsTables {
    /// The frame log, newest last.
    pub frames: LogRing<FrameRow>,
    /// The Doppler curve of good frames.
    pub curve: LogRing<CurvePoint>,
    /// Good frames by spreading factor.
    pub per_sf: Tally<u8>,
    /// Good frames.
    pub good: u64,
    /// Frames whose CRC failed.
    pub bad: u64,
}

impl Default for LsTables {
    fn default() -> Self {
        LsTables {
            frames: LogRing::new(FRAMES_CAP),
            curve: LogRing::new(CURVE_CAP),
            per_sf: Tally::new(16),
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for LsTables {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.curve.shift_times(by_s);
    }
}

impl Tables for LsTables {
    const FORMAT: StateFormat = StateFormat::new("satcom.lora_sat", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.curve.clear();
        self.per_sf.clear();
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.curve.adopt(saved.curve);
        self.per_sf = saved.per_sf;
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

/// The display decoder (`satcom.lora_sat_display`).
pub struct LoraSatDisplay {
    tables: LsTables,
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    satellite: String,
    demods: Vec<DemodRow>,
    preset: Option<&'static str>,
    allowance: Option<(f64, f64)>,
    /// The frame tables changed since the last report.
    dirty: bool,
}

impl Default for LoraSatDisplay {
    fn default() -> Self {
        LoraSatDisplay {
            tables: LsTables::default(),
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            satellite: String::new(),
            demods: Vec::new(),
            preset: None,
            allowance: None,
            dirty: true,
        }
    }
}

/// The payload as printable ASCII, with a dot for any other byte.
fn printable(b: &[u8]) -> String {
    b.iter()
        .map(|&c| {
            if (0x20..0x7F).contains(&c) {
                c as char
            } else {
                '.'
            }
        })
        .collect()
}

impl LoraSatDisplay {
    /// The tables kept.
    pub fn tables(&self) -> &LsTables {
        &self.tables
    }

    fn frame(&mut self, item_t: f64, f: &LsFrame, events: &mut Vec<Event>) {
        let t = self.clock.at(item_t);
        let fr = &f.frame;
        let off0 = fr.channel_hz + fr.cfo_hz - f.dial_hz;
        // The rate: the slope of the Doppler curve from a recent good frame (the steadier measure), else the rate the
        // Doppler-tracking detector measured over the frame. The timing loop of a detector without tracking does not measure
        // the rate well, so without both the rate is not known.
        let rate = match self.tables.curve.newest_first().next() {
            Some(p) if fr.good() && (CURVE_WINDOW_S.0..CURVE_WINDOW_S.1).contains(&(t - p.t)) => {
                Some((off0 - p.offset_hz) / (t - p.t))
            }
            _ => f.tracked_rate_hz_s,
        };
        self.dirty = true;
        let row = FrameRow {
            at: self.clock.stamp(t),
            sf: fr.sf,
            bw_hz: fr.bw,
            cr: fr.cr,
            crc_present: fr.crc_present,
            crc_ok: fr.crc_ok,
            sync: fr.sync,
            snr_db: fr.snr_db,
            offset_hz: off0,
            rate_hz_s: rate,
            freq_hz: f.centre_hz + fr.channel_hz + fr.cfo_hz,
            satellite: self.satellite.clone(),
            payload: Arc::from(fr.payload.as_slice()),
            text: printable(&fr.payload),
        };
        if fr.good() {
            self.tables.good += 1;
            self.tables.per_sf.add(fr.sf);
            self.tables.curve.push(CurvePoint {
                t,
                offset_hz: off0,
                rate_hz_s: rate,
            });
            let source = (!self.satellite.is_empty()).then_some(self.satellite.as_str());
            events.push(LORA_SAT_FRAME.event(
                t,
                row.freq_hz,
                &LoraSatFrameEvent {
                    source,
                    text: &row.text,
                    snr_db: Some(fr.snr_db).filter(|s| s.is_finite()),
                    family: Family::Satellite,
                    sf: i64::from(fr.sf),
                    payload: row.payload.clone(),
                    doppler_hz: Hertz(off0),
                    doppler_rate_hz_s: rate,
                },
            ));
        } else {
            self.tables.bad += 1;
        }
        self.tables.frames.push(row);
    }
}

impl Decoder for LoraSatDisplay {
    type Input = Timed<LsItem>;
    fn name(&self) -> &'static str {
        "satcom.lora_sat_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        LoraSatDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(
            LoraSatDisplaySetting::PassSatellite,
            Value::Text(self.satellite.as_str().into()),
        )])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match LoraSatDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LoraSatDisplaySetting::PassSatellite => {
                self.satellite = PASS_SATELLITE.read(id, v)?.to_string()
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
    fn process(&mut self, x: &[Timed<LsItem>], _first: u64, out: &mut Outbox) {
        let events = out.messages(0);
        for it in x {
            match &it.value {
                LsItem::Frame(f) => self.frame(it.time, f, events),
                LsItem::Status {
                    demods,
                    preset,
                    allowance,
                } => {
                    self.clock.at(it.time);
                    self.demods.clone_from(demods);
                    self.preset = preset.map(|p| p.name);
                    self.allowance = Some(*allowance);
                }
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let t = &self.tables;
        let (dmax, rmax) = self.allowance.unwrap_or_else(|| doppler_limits(1.0));
        p.put(&LoraSatFields {
            frames_ok: t.good as i64,
            frames_bad: t.bad as i64,
            satellite: &self.satellite,
            preset: self.preset,
            demodulators: self.demods.len() as i64,
            doppler_max_hz: Hertz(dmax),
            doppler_rate_max_hz_s: rmax,
        });
        let dirty = std::mem::take(&mut self.dirty);
        if dirty {
            shaped::log_rows(&t.frames, &LORA_SAT_FRAMES, now, p);
            p.clear_rows(&LORA_SAT_FRAMES_PER_SF);
            let mut key = String::new();
            for (sf, n) in t.per_sf.iter() {
                key.clear();
                let _ = write!(key, "{sf:02}");
                LORA_SAT_FRAMES_PER_SF.put(
                    p,
                    &key,
                    &LoraSatSfRow {
                        sf: i64::from(*sf),
                        frames: n as i64,
                    },
                );
            }
        }
        shaped::indexed_rows(
            p,
            &LORA_SAT_DEMODULATORS,
            &self.demods,
            |d: &DemodRow, out| {
                out.put(&LoraSatDemodulatorRow {
                        sf: i64::from(d.sf),
                        bw_hz: Hertz(d.bw_hz),
                        preambles: d.stats.preambles as i64,
                        sync_failed: d.stats.sync_failed as i64,
                        header_failed: d.stats.header_failed as i64,
                        frames: d.stats.frames as i64,
                        crc_failed: d.stats.crc_failed as i64,
                    },
                );
            },
        );
        let syncs: usize = self.demods.iter().map(|d| d.syncs.len()).sum();
        if p.results()
            .decl()
            .table_id(LORA_SAT_DEMODULATOR_SYNCS.def().key)
            .and_then(|id| p.results().table(id))
            .is_none_or(|tb| tb.len() != syncs)
        {
            p.clear_rows(&LORA_SAT_DEMODULATOR_SYNCS);
            let (mut key, mut demod) = (String::new(), String::new());
            for (i, d) in self.demods.iter().enumerate() {
                demod.clear();
                let _ = write!(demod, "{i:03}");
                for s in &d.syncs {
                    key.clear();
                    let _ = write!(key, "{demod}/{s}");
                    LORA_SAT_DEMODULATOR_SYNCS.put(
                        p,
                        &key,
                        &LoraSatDemodulatorSyncRow {
                            demodulator: &demod,
                            sync: i64::from(*s),
                        },
                    );
                }
            }
        }
        let t = &self.tables;
        let r = p.results_mut();
        set_series(r, LORA_SAT_DOPPLER.key, true, || {
            t.curve.iter().map(|c| [c.t as f32, c.offset_hz as f32])
        });
        let last_good = t.curve.newest_first().next().map(|c| c.t);
        let state = if last_good.is_some_and(|g| now - g < LOCK_HOLD_S) {
            2
        } else {
            0
        };
        let mut summary = String::new();
        let _ = if t.good + t.bad == 0 {
            write!(
                summary,
                "listening, {} demodulators, Doppler up to +-{:.1} kHz",
                self.demods.len(),
                dmax / 1e3
            )
        } else {
            write!(summary, "{} frames, {} CRC errors", t.good, t.bad)
        };
        if !self.satellite.is_empty() {
            let _ = write!(summary, ", pass of {}", self.satellite);
        }
        let lock_state = LockState::from_level(state);
        let newest = t.frames.newest_first().next();
        let offset = newest.map(|f| f.offset_hz);
        let snr = newest.map(|f| f.snr_db as f32);
        r.update_status(|st| {
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
