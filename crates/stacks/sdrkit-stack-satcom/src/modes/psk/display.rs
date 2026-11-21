//! The PSK display: the receiver's state, the analyser's last result, the newest bits and sync words, the constellation and eye,
//! the log of analyses, and the bytes passed to the host.

use super::bits::Resolution;
use super::rx::{OrderChoice, PskItem, RxStatus};
use super::BITS_CAP;
use crate::display::{set_series, Clock};
use crate::typed::{
    PskDemodBytesEvent, PskDemodFields, PskDemodLockEvent, PskDemodLogRow, PskDemodRateRow,
    PskDemodSyncRow, PskDemodTrialRow, PSK_DEMOD_BYTES, PSK_DEMOD_CONSTELLATION,
    PSK_DEMOD_ENVELOPE, PSK_DEMOD_EYE, PSK_DEMOD_LOCK, PSK_DEMOD_LOG, PSK_DEMOD_POWER,
    PSK_DEMOD_RATES, PSK_DEMOD_SYNCS, PSK_DEMOD_TRIALS,
};
use sdrkit_algo_modem::psk::{Analysis, AnalysisNote, PskOrder, PskPulse};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, ConfigError, Event, LockState, ResultsDecl, Schema, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Kind, Outbox, Port, RateFrom, RunContext, Timed};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

sdrkit_core::choice_enum! {
    /// The pulse shape the analyser found.
    pub enum ShapeFound {
        /// Rectangular pulses.
        Rect = ("rect", "Rectangular"),
        /// Root raised cosine pulses.
        Rrc = ("rrc", "Root raised cosine"),
    }
}

sdrkit_core::choice_enum! {
    /// Why the analyser found no PSK signal.
    pub enum Note {
        /// The capture is too short.
        TooShort = ("too_short", "Too few samples"),
        /// No symbol rate line.
        NoRateLine = ("no_rate_line", "No symbol rate line"),
        /// No trial demodulation could run.
        NoTrial = ("no_trial", "No trial demodulation"),
        /// The best trial has no constellation.
        NoConstellation = ("no_constellation", "No constellation"),
    }
}

impl Note {
    fn of(n: AnalysisNote) -> Note {
        match n {
            AnalysisNote::TooShort => Note::TooShort,
            AnalysisNote::NoRateLine => Note::NoRateLine,
            AnalysisNote::NoTrial => Note::NoTrial,
            AnalysisNote::NoConstellation => Note::NoConstellation,
        }
    }
}

/// The order as a choice of the order setting.
fn order_choice(o: PskOrder) -> OrderChoice {
    match o {
        PskOrder::Bpsk => OrderChoice::Bpsk,
        PskOrder::Qpsk => OrderChoice::Qpsk,
        PskOrder::Oqpsk => OrderChoice::Oqpsk,
        PskOrder::Psk8 => OrderChoice::Psk8,
    }
}

/// The pulse shape as shown ("rectangular", "RRC 0.35").
fn shape_text(s: Option<PskPulse>) -> String {
    match s {
        Some(PskPulse::Rect) => "rectangular".into(),
        Some(PskPulse::Rrc(b)) => format!("RRC {b:.2}"),
        None => String::new(),
    }
}

/// The most lines of the analysis log.
pub const LOG_CAP: usize = 100;
/// The most constellation points kept.
const CONSTELLATION_CAP: usize = 1024;
/// The most sync word positions kept.
pub const SYNCS_CAP: usize = 64;

/// What the display publishes.
pub fn results_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        let mut d = ResultsDecl::new()
            .with_fields(<PskDemodFields as Fields>::SHAPE.fields)
            .with_table(&PSK_DEMOD_RATES)
            .with_table(&PSK_DEMOD_TRIALS)
            .with_table(&PSK_DEMOD_SYNCS)
            .with_table(&PSK_DEMOD_LOG);
        for s in [
            PSK_DEMOD_CONSTELLATION,
            PSK_DEMOD_EYE,
            PSK_DEMOD_ENVELOPE,
            PSK_DEMOD_POWER,
        ] {
            d = d.series(s.key, s.label, s.x_unit, s.y_unit);
        }
        Arc::new(d)
    })
    .clone()
}

/// A line of the log of analyses.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogNote {
    /// When.
    pub at: Stamp,
    /// The text.
    pub text: String,
}

impl Shift for LogNote {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Shaped for LogNote {
    type Row = PskDemodLogRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&PskDemodLogRow {
            at: Seconds(self.at.t),
            text: &self.text,
        });
    }
}

/// What the display keeps across sessions: the log of analyses.
#[derive(Debug, Serialize, Deserialize)]
pub struct PskTables {
    /// The log of analyses.
    pub log: LogRing<LogNote>,
}

impl Default for PskTables {
    fn default() -> Self {
        PskTables {
            log: LogRing::new(LOG_CAP),
        }
    }
}

impl Shift for PskTables {
    fn shift_times(&mut self, by_s: f64) {
        self.log.shift_times(by_s);
    }
}

impl Tables for PskTables {
    const FORMAT: StateFormat = StateFormat::new("satcom.psk_demod", 1);
    fn clear(&mut self) {
        self.log.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.log.adopt(saved.log);
    }
}

/// Hand the bits, packed into bytes, to the host.
pub const SAVE_BITS: Setting<field::Bool> = Setting::new("save_bits", "Save bits", "", false)
    .help("Hands the bits, packed into bytes, to the host (event psk_demod.bytes).");

sdrkit_core::settings! {
    /// The settings of [`PskDisplay`].
    pub enum PskDisplaySetting {
        /// Save bits.
        SaveBits = SAVE_BITS,
    }
}

/// The PSK display.
pub struct PskDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    tuned_hz: f64,
    tables: PskTables,
    /// Lines for the results log, written at the next report.
    pending_log: Vec<String>,
    /// The log table changed since the last report.
    dirty: bool,
    /// The sync word positions changed since the last report.
    syncs_dirty: bool,
    status: Arc<RxStatus>,
    analysis: Option<Arc<Analysis>>,
    bits: VecDeque<u8>,
    bits_first: u64,
    syncs: VecDeque<u64>,
    resolution: Resolution,
    constellation: VecDeque<[f32; 2]>,
    symbols: u64,
    bits_total: u64,
    total_syncs: u64,
    save_bits: bool,
    pending: Vec<u8>,
    text: String,
}

impl Default for PskDisplay {
    fn default() -> Self {
        PskDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            tuned_hz: 0.0,
            tables: PskTables::default(),
            pending_log: Vec::new(),
            dirty: true,
            syncs_dirty: true,
            status: Arc::new(RxStatus::default()),
            analysis: None,
            bits: VecDeque::new(),
            bits_first: 0,
            syncs: VecDeque::new(),
            resolution: Resolution::default(),
            constellation: VecDeque::new(),
            symbols: 0,
            bits_total: 0,
            total_syncs: 0,
            save_bits: false,
            pending: Vec::new(),
            text: String::new(),
        }
    }
}

impl PskDisplay {
    /// Adds a line to the log of analyses and to the results' log.
    fn note(&mut self, t: f64, text: String) {
        self.pending_log.push(text.clone());
        self.dirty = true;
        self.tables.log.push(LogNote {
            at: self.clock.stamp(t),
            text,
        });
    }

    fn analysis(&mut self, t: f64, a: &Arc<Analysis>, events: &mut Vec<Event>) {
        let line = if a.signal {
            let order = a.order.map_or("?", |o| o.name());
            events.push(PSK_DEMOD_LOCK.event(
                t,
                self.tuned_hz + a.carrier_hz,
                &PskDemodLockEvent {
                    text: &format!("{order} at {:.0} Bd", a.baud),
                    snr_db: Some(a.esn0_db).filter(|s| s.is_finite()),
                    family: Family::Satellite,
                    symbol_rate_hz: Hertz(a.baud),
                    order: a.order.map(order_choice),
                },
            ));
            format!(
                "{t:.1} s: {order} at {:.0} Bd ({}), carrier {:+.1} Hz, Es/N0 {:.1} dB",
                a.baud,
                shape_text(a.shape),
                a.carrier_hz,
                a.esn0_db
            )
        } else {
            let why = a
                .note
                .map_or("", |n| &*Note::CHOICES[Note::of(n) as usize].label);
            format!("{t:.1} s: no PSK signal ({why})")
        };
        self.note(t, line);
        self.analysis = Some(a.clone());
    }

    #[allow(clippy::too_many_arguments)]
    fn bits(
        &mut self,
        t: f64,
        bits: &[u8],
        first: u64,
        syncs: &[u64],
        resolution: &Resolution,
        total_syncs: u64,
        events: &mut Vec<Event>,
    ) {
        if self.bits.is_empty() || first < self.bits_first {
            self.bits.clear();
            self.bits_first = first;
            self.syncs.clear();
        }
        self.bits.extend(bits.iter().copied());
        while self.bits.len() > BITS_CAP {
            self.bits.pop_front();
            self.bits_first += 1;
        }
        self.syncs_dirty |= !syncs.is_empty();
        self.syncs.extend(syncs.iter().copied());
        while self.syncs.len() > SYNCS_CAP {
            self.syncs.pop_front();
        }
        self.bits_total += bits.len() as u64;
        if resolution.resolved && !self.resolution.resolved {
            let line = format!(
                "sync word resolved: rotation {:.0} deg{}{}",
                resolution.rotation_deg,
                if resolution.conjugated {
                    ", conjugated"
                } else {
                    ""
                },
                if resolution.rail_shift {
                    ", rail shift"
                } else {
                    ""
                }
            );
            self.note(t, line);
        }
        self.resolution = resolution.clone();
        self.total_syncs = total_syncs;
        if self.save_bits {
            self.pending.extend_from_slice(bits);
            let n = self.pending.len() / 8 * 8;
            if n > 0 {
                let mut bytes = Vec::with_capacity(n / 8);
                for c in self.pending[..n].chunks(8) {
                    bytes.push(c.iter().fold(0u8, |a, &b| (a << 1) | (b & 1)));
                }
                self.pending.drain(..n);
                self.text.clear();
                sdrkit_core::base64::hex_into(&bytes, &mut self.text);
                let freq = self.tuned_hz + self.status.carrier_hz;
                events.push(PSK_DEMOD_BYTES.event(
                    t,
                    freq,
                    &PskDemodBytesEvent {
                        text: &self.text,
                        family: Family::Satellite,
                        bytes: Arc::from(bytes),
                    },
                ));
            }
        }
    }
}

impl PskDisplay {
    /// Writes the rate candidates and trials of analysis `a`, changing only the rows that differ.
    fn write_analysis(&mut self, p: &mut Published, a: &Analysis) {
        let mut key = String::new();
        for (i, (r, db)) in a.rate_candidates.iter().enumerate() {
            key.clear();
            let _ = write!(key, "{i}");
            PSK_DEMOD_RATES.put(
                p,
                &key,
                &PskDemodRateRow {
                    rate_hz: Hertz(*r),
                    strength_db: *db,
                },
            );
        }
        for i in a.rate_candidates.len()..PSK_DEMOD_RATES.cap {
            key.clear();
            let _ = write!(key, "{i}");
            p.remove_row(&PSK_DEMOD_RATES, &key);
        }
        for (i, tr) in a.order_trials.iter().enumerate() {
            key.clear();
            let _ = write!(key, "{i:02}");
            PSK_DEMOD_TRIALS.put(
                p,
                &key,
                &PskDemodTrialRow {
                    order: order_choice(tr.order),
                    symbol_rate_hz: Hertz(tr.baud),
                    evm: tr.evm,
                },
            );
        }
        for i in a.order_trials.len()..PSK_DEMOD_TRIALS.cap {
            key.clear();
            let _ = write!(key, "{i:02}");
            p.remove_row(&PSK_DEMOD_TRIALS, &key);
        }
    }
}

impl Decoder for PskDisplay {
    type Input = Timed<PskItem>;
    fn name(&self) -> &'static str {
        "satcom.psk_demod_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        PskDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(PskDisplaySetting::SaveBits, Value::Bool(self.save_bits))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match PskDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PskDisplaySetting::SaveBits => self.save_bits = SAVE_BITS.read(id, v)?,
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(results_decl())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.tuned_hz = input.tuned_hz;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.tuned_hz = input.tuned_hz;
    }
    fn process(&mut self, x: &[Timed<PskItem>], _first: u64, out: &mut Outbox) {
        let events = out.messages(0);
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                PskItem::Analysis(a) => self.analysis(t, a, events),
                PskItem::Symbols { syms, .. } => {
                    self.symbols += syms.len() as u64;
                    self.constellation.extend(syms.iter().map(|z| [z.re, z.im]));
                    while self.constellation.len() > CONSTELLATION_CAP {
                        self.constellation.pop_front();
                    }
                }
                PskItem::Bits {
                    bits,
                    first,
                    syncs,
                    resolution,
                    total_syncs,
                    ..
                } => self.bits(t, bits, *first, syncs, resolution, *total_syncs, events),
                PskItem::Status(s) => self.status = s.clone(),
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let s = self.status.clone();
        let locked = s.state == 2;
        for line in self.pending_log.drain(..) {
            p.results_mut().log(line);
        }
        self.text.clear();
        self.text
            .extend(self.bits.iter().map(|&b| if b == 1 { '1' } else { '0' }));
        let f = s.found.as_ref();
        let a = self.analysis.clone();
        let a = a.as_deref();
        let shape = a.and_then(|a| a.shape);
        p.put(&PskDemodFields {
            symbol_rate_hz: locked.then_some(Hertz(s.baud)),
            order: s.order.filter(|_| locked).map(order_choice),
            lock: locked.then_some(s.lock),
            carrier_hz: locked.then_some(Hertz(s.carrier_hz)),
            drift_hz_s: locked.then_some(s.doppler_hz_s),
            evm_pct: locked.then_some(s.evm * 100.0),
            esn0_db: locked.then_some(s.esn0_db),
            ebn0_db: locked.then_some(s.ebn0_db),
            symbols: self.symbols as i64,
            bits: self.bits_total as i64,
            frames_ok: self.total_syncs as i64,
            bit_stream: &self.text,
            bits_first: self.bits_first as i64,
            rotation_deg: self.resolution.rotation_deg,
            conjugated: self.resolution.conjugated,
            rail_shift: self.resolution.rail_shift,
            resolved: self.resolution.resolved,
            signal_hz: f.map(|f| Hertz(f.centre_hz)),
            width_3db_hz: f.map(|f| Hertz(f.width_3db_hz)),
            width_10db_hz: f.map(|f| Hertz(f.width_10db_hz)),
            occupied_hz: f.map(|f| Hertz(f.occupied_99_hz)),
            signal_snr_db: f.map(|f| f.snr_db),
            baud_raw_hz: a.map(|a| Hertz(a.baud_raw)),
            snapped: a.map(|a| a.snapped),
            rate_confidence: a.map(|a| a.rate_confidence),
            order_confidence: a.map(|a| a.order_confidence),
            shape: shape.map(|s| match s {
                PskPulse::Rect => ShapeFound::Rect,
                PskPulse::Rrc(_) => ShapeFound::Rrc,
            }),
            roll_off: shape.and_then(|s| match s {
                PskPulse::Rrc(b) => Some(b),
                PskPulse::Rect => None,
            }),
            envelope_db: a.map(|a| a.envelope_db),
            line_x2_db: a.map(|a| a.line_x2_db),
            line_x4_db: a.map(|a| a.line_x4_db),
            line_x8_db: a.map(|a| a.line_x8_db),
            note: a.and_then(|a| a.note).map(Note::of),
        });
        if std::mem::take(&mut self.syncs_dirty) {
            p.clear_rows(&PSK_DEMOD_SYNCS);
            let mut key = String::with_capacity(4);
            for (i, &b) in self.syncs.iter().enumerate() {
                key.clear();
                let _ = write!(key, "{i:02}");
                PSK_DEMOD_SYNCS.put(p, &key, &PskDemodSyncRow { bit: b as i64 });
            }
        }
        if let Some(a) = a {
            self.write_analysis(p, a);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.tables.log, &PSK_DEMOD_LOG, now, p);
        }
        let r = p.results_mut();
        set_series(r, PSK_DEMOD_CONSTELLATION.key, false, || {
            self.constellation.iter().copied()
        });
        if !s.eye.is_empty() {
            let eye = || {
                s.eye
                    .iter()
                    .flat_map(|trace| trace.iter().enumerate().map(|(j, v)| [j as f32, *v]))
            };
            set_series(r, PSK_DEMOD_EYE.key, false, eye);
        }
        if let Some(a) = a {
            set_series(r, PSK_DEMOD_ENVELOPE.key, true, || {
                a.envelope_spectrum
                    .iter()
                    .enumerate()
                    .map(|(i, v)| [i as f32, *v])
            });
            set_series(r, PSK_DEMOD_POWER.key, true, || {
                a.power_spectrum
                    .iter()
                    .enumerate()
                    .map(|(i, v)| [i as f32, *v])
            });
        }
        let lock_state = LockState::from_level(s.state);
        let offset = match s.state {
            2 => Some(s.carrier_hz),
            1 => s.found.as_ref().map(|f| f.centre_hz),
            _ => None,
        };
        let snr = locked.then_some(s.esn0_db as f32);
        let syncs_found = self.total_syncs > 0;
        r.update_status(|st| {
            let mut summary = String::new();
            match s.state {
                2 => {
                    let _ = write!(
                        summary,
                        "PSK: {} {:.0} Bd, Eb/N0 {:.0} dB{}",
                        s.order.map_or("?", |o| o.name()),
                        s.baud,
                        s.ebn0_db,
                        if syncs_found {
                            ", sync words found"
                        } else {
                            ""
                        }
                    );
                }
                1 => summary.push_str("PSK: signal found, analysing"),
                _ => summary.push_str("PSK: searching"),
            }
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
        self.bits.clear();
        self.syncs.clear();
        self.bits_total = 0;
        self.symbols = 0;
        self.dirty = true;
        self.syncs_dirty = true;
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
