//! COSPAS-SARSAT 406 MHz beacons (`sarsat`): first-generation and second-generation bursts, the beacon log and its events.
//!
//! Standard: C/S T.001 (first generation), C/S T.018 (second generation).

pub mod coherent;
pub mod fgb;
pub mod gen;
pub mod repeat;
pub mod sgb;
pub mod t001;
pub mod t018;

use crate::display::{set_status, Clock};
use crate::typed::{
    SarsatBeaconEvent, SarsatBeaconRow, SarsatBurstRow, SarsatFields, SarsatPresetArgs,
    SARSAT_BEACON, SARSAT_BEACONS, SARSAT_BEACON_ROW, SARSAT_BURSTS, SARSAT_PRESET_APPLY,
    SARSAT_REMOVE,
};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Command, Complex32, ConfigError, GenConfig, Generator, HostRequest, LockProfile,
    LockState, Receiver, Record, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet,
    StackConfig, StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "sarsat";
/// The stack name.
pub const NAME: &str = "COSPAS-SARSAT 406 MHz";
/// The least input rate.
pub const MIN_RATE: f64 = 128_000.0;
/// The least input rate of the second-generation path.
pub const SGB_MIN_RATE: f64 = 160_000.0;
/// The second-generation carrier is this far above the dial (406.050 MHz against the 406.028 MHz dial).
pub const SGB_OFFSET_HZ: f64 = 22_000.0;
/// Seconds between two reports.
pub const REPORT_S: f64 = 0.25;
/// The most bursts the log keeps.
pub const LOG_CAP: usize = 200;
/// The most beacons the beacon table keeps.
pub const BEACON_CAP: usize = 200;
/// A beacon decoded this recently (seconds) keeps the lock state locked.
pub const RECENT_S: f64 = 300.0;
/// The width of the first-generation channel each finder reads, Hz.
const FGB_WIDTH_HZ: f64 = 92_000.0;
/// Finders of the wide search sit this far apart, Hz.
const BANK_STEP_HZ: f64 = 90_000.0;

/// What the receiver passes to the display.
#[derive(Clone, Debug)]
pub enum SarsatItem {
    /// A first-generation burst.
    Fgb {
        /// Carrier offset from the dial, Hz.
        carrier_hz: f64,
        /// Carrier to mean noise in one 31.25 Hz bin, dB.
        cn_db: f64,
        /// The burst's level, dBFS.
        level_dbfs: f64,
        /// The message, when it is reported.
        message: Option<t001::Fgb>,
        /// How the message was reached.
        repair: Repair,
    },
    /// A second-generation burst.
    Sgb {
        /// Carrier offset from the dial, Hz.
        carrier_hz: f64,
        /// SNR of the despread channel bits, dB.
        snr_db: f64,
        /// The message, when it decoded.
        message: Option<t018::Sgb>,
    },
    /// At each tick: the input level and the bank.
    Status {
        /// The input level since the last tick, dBFS.
        level_dbfs: f64,
        /// Samples received since the start.
        samples: u64,
        /// The number of first-generation finders.
        finders: usize,
        /// The width the finders cover, Hz.
        span_hz: f64,
    },
}
/// How a first-generation message was reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Repair {
    /// Bursts whose soft bits were summed (1 for a burst decoded alone).
    pub combined: u32,
    /// A later burst decoded to the same identification.
    pub confirmed: bool,
    /// The distance to the next code word that passes, over the mean reliability of a bit.
    pub margin: f64,
    /// The reliability of the repaired bits over the mean reliability of a bit.
    pub cost: f64,
}

impl Item for SarsatItem {
    const KIND: &'static str = "satcom.sarsat_item";
}

/// Whether the dial lies in the satellites' L-band repeat (the wide search turns itself on there).
pub fn in_lband(dial_hz: f64) -> bool {
    (1543.5e6..=1546.0e6).contains(&dial_hz)
}

sdrkit_core::choice_enum! {
    /// The dial presets.
    pub enum Preset {
        /// The beacons, 406.028 MHz.
        Beacons = ("406.028", "406.028 MHz (beacons)"),
        /// The satellite downlink, 1544.500 MHz.
        Downlink = ("1544.500", "1544.500 MHz (satellite downlink)"),
    }
}

/// The dial frequency of a preset, Hz.
pub fn preset_hz(p: Preset) -> f64 {
    match p {
        Preset::Beacons => 406.028e6,
        Preset::Downlink => 1544.5e6,
    }
}

/// The argument of `preset.apply`: the dial preset.
pub const PRESET: Setting<field::Choice<Preset>> =
    Setting::choice("preset", "Preset", Preset::Beacons);

sdrkit_core::commands! {
    /// The commands of [`SarsatRx`].
    pub enum SarsatRxCommand {
        /// Tunes to a preset.
        PresetApply = SARSAT_PRESET_APPLY.command,
    }
}
/// The bank of burst finders across the input.
pub const WIDE_SEARCH: Setting<field::Bool> = Setting::new("wide_search", "Wide search", "", false).help(
    "A bank of burst finders 90 kHz apart across up to 1.1 MHz of the input. It is on by itself while the dial is \
     between 1543.5 and 1546.0 MHz.",
);

/// Repeat combining of first-generation bursts.
pub const REPEAT_COMBINING: Setting<field::Bool> =
    Setting::new("repeat_combining", "Repeat combining", "", true).help(
        "Sums the soft bits of a first-generation burst that does not decode with those of the same beacon's earlier \
         bursts on the same carrier.",
    );
/// The shortest repeat interval of a first-generation beacon.
pub const REPEAT_MIN_S: Setting<field::Duration> =
    Setting::new("repeat_min_s", "Shortest repeat interval", "s", 47.5)
        .range(1.0, 300.0, 0.0)
        .help("C/S T.001: a beacon repeats every 50 s +-5 %.");
/// The longest repeat interval of a first-generation beacon.
pub const REPEAT_MAX_S: Setting<field::Duration> =
    Setting::new("repeat_max_s", "Longest repeat interval", "s", 52.5)
        .range(1.0, 300.0, 0.0)
        .help("C/S T.001: a beacon repeats every 50 s +-5 %.");

sdrkit_core::settings! {
    /// The settings of [`SarsatRx`].
    pub enum SarsatRxSetting {
        /// Wide search.
        WideSearch = WIDE_SEARCH,
        /// Repeat combining.
        RepeatCombining = REPEAT_COMBINING,
        /// The shortest repeat interval.
        RepeatMinS = REPEAT_MIN_S,
        /// The longest repeat interval.
        RepeatMaxS = REPEAT_MAX_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the COSPAS-SARSAT stack. Noise and other impairments of the test signal come from the standard `gen_*`
    /// settings.
    pub enum SarsatSetting {
        /// Wide search.
        WideSearch = WIDE_SEARCH,
        /// Repeat combining.
        RepeatCombining = REPEAT_COMBINING,
        /// The shortest repeat interval.
        RepeatMinS = REPEAT_MIN_S,
        /// The longest repeat interval.
        RepeatMaxS = REPEAT_MAX_S,
        /// Second generation of the test signal.
        GenSecondGeneration = gen::GEN_SECOND_GENERATION,
        /// Beacon of the test signal.
        GenBeacon = gen::GEN_BEACON,
        /// Phase polarity of the test signal.
        GenReverse = gen::GEN_REVERSE,
        /// Bit rate error of the test signal.
        GenBitratePct = gen::GEN_BITRATE_PCT,
        /// Gap of the test signal.
        GenGapS = gen::GEN_GAP_S,
    }
}

/// The channel of a signal at `offset_hz` in an input at `in_rate`, at 0 Hz and exactly `out_rate`: flat to 0.42 of the output rate,
/// and every input component more than half the output rate from the signal attenuated by 80 dB. Errors when a rate is not
/// positive.
pub fn baseband(in_rate: f64, offset_hz: f64, out_rate: f64) -> Result<Channelizer, ConfigError> {
    let mut cfg = ChannelConfig::new(in_rate, offset_hz, 0.84 * out_rate, out_rate);
    cfg.exact_rate = true;
    Ok(Channelizer::with_stop(cfg, out_rate / 2.0)?)
}

/// The receiver: first-generation finders (one at the dial, or a bank), and the second-generation path.
pub struct SarsatRx {
    wide: bool,
    request: Option<f64>,
    rate: f64,
    dial: f64,
    tuned: f64,
    finders: Vec<(f64, Channelizer, fgb::FgbFinder)>,
    sgb: Option<(Channelizer, sgb::SgbReceiver)>,
    tmp: Vec<Complex32>,
    bursts: Vec<fgb::FgbBurst>,
    sbursts: Vec<sgb::SgbBurst>,
    repeats: repeat::Repeats,
    power: f64,
    count: u64,
    samples: u64,
    next_index: u64,
    recent: Vec<(f64, f64)>,
}

impl SarsatRx {
    /// A receiver with the wide search on or off.
    pub fn new(wide: bool) -> Self {
        SarsatRx {
            wide,
            request: None,
            rate: 0.0,
            dial: 0.0,
            tuned: 0.0,
            finders: Vec::new(),
            sgb: None,
            tmp: Vec::new(),
            bursts: Vec::new(),
            sbursts: Vec::new(),
            repeats: repeat::Repeats::new(repeat::RepeatConfig::default()),
            power: 0.0,
            count: 0,
            samples: 0,
            next_index: 0,
            recent: Vec::new(),
        }
    }

    fn build(&mut self) -> Result<(), ConfigError> {
        // New finders count samples from 0: what the repeats hold is on the old time base.
        self.finders.clear();
        self.repeats.clear();
        self.recent.clear();
        let wide = self.wide || in_lband(self.tuned);
        let mut offs = vec![0.0];
        if wide {
            let span = (0.9 * self.rate).min(1.1e6);
            let k = ((span / 2.0 - FGB_WIDTH_HZ / 2.0) / BANK_STEP_HZ)
                .floor()
                .max(0.0) as i64;
            offs = (-k..=k).map(|i| i as f64 * BANK_STEP_HZ).collect();
        }
        for o in offs {
            let ch = baseband(self.rate, self.dial + o, fgb::RATE)?;
            self.finders.push((o, ch, fgb::FgbFinder::new()));
        }
        self.sgb = if self.rate >= SGB_MIN_RATE {
            let ch = baseband(self.rate, self.dial + SGB_OFFSET_HZ, sgb::RATE)?;
            Some((ch, sgb::SgbReceiver::new()))
        } else {
            None
        };
        Ok(())
    }

    /// The number of first-generation finders.
    pub fn finders(&self) -> usize {
        self.finders.len()
    }

    fn fgb(&mut self, iq: &[Complex32], out: &mut Vec<Timed<SarsatItem>>) {
        let life = self.repeats.config().lifetime_s();
        for (o, ch, f) in &mut self.finders {
            self.tmp.clear();
            ch.process(iq, &mut self.tmp);
            self.bursts.clear();
            f.process(&self.tmp, &mut self.bursts);
            for b in self.bursts.drain(..) {
                let t = b.start as f64 / fgb::RATE;
                let carrier = *o + b.carrier_hz;
                // One burst heard by two finders is taken once.
                if self
                    .recent
                    .iter()
                    .any(|(tt, c)| (t - tt).abs() < 0.5 && (carrier - c).abs() < 3e3)
                {
                    continue;
                }
                self.recent.push((t, carrier));
                if self.recent.len() > 16 {
                    self.recent.remove(0);
                }
                let (decoded, soft) = b.demod.map_or((None, Vec::new()), |d| (d.decoded, d.soft));
                let outcome = self.repeats.burst(t, carrier, decoded, soft);
                if outcome.expect {
                    f.expect(b.carrier_hz, b.start + (life * fgb::RATE) as u64);
                } else if b.weak && outcome.report.is_none() {
                    // A weak line without the bare carrier and sync of a first-generation burst (a line of a
                    // second-generation burst, a spur) is not a burst.
                    continue;
                }
                let (message, repair) = match outcome.report {
                    Some(r) => (
                        Some(r.message),
                        Repair {
                            combined: r.combined,
                            confirmed: r.confirmed,
                            margin: f64::from(r.margin),
                            cost: f64::from(r.cost),
                        },
                    ),
                    None => (None, Repair::default()),
                };
                out.push(Timed {
                    time: t,
                    value: SarsatItem::Fgb {
                        carrier_hz: carrier,
                        cn_db: b.cn_db,
                        level_dbfs: b.level_dbfs,
                        message,
                        repair,
                    },
                });
            }
        }
    }

    fn sgb(&mut self, iq: &[Complex32], out: &mut Vec<Timed<SarsatItem>>) {
        let Some((ch, rx)) = &mut self.sgb else {
            return;
        };
        // Bounded pieces keep the working buffer small.
        for c in iq.chunks(16_384) {
            self.tmp.clear();
            ch.process(c, &mut self.tmp);
            self.sbursts.clear();
            rx.process(&self.tmp, &mut self.sbursts);
            for b in self.sbursts.drain(..) {
                out.push(Timed {
                    time: b.start as f64 / sgb::RATE,
                    value: SarsatItem::Sgb {
                        carrier_hz: b.carrier_hz + SGB_OFFSET_HZ,
                        snr_db: b.snr_db,
                        message: b.message,
                    },
                });
            }
        }
    }
}

impl Decoder for SarsatRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.sarsat_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<SarsatItem>("out").rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        SarsatRxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match SarsatRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SarsatRxSetting::WideSearch => {
                self.wide = WIDE_SEARCH.read(id, v)?;
                if self.rate > 0.0 {
                    self.build().map_err(|e| SettingError::Config {
                        id,
                        source: Box::new(e),
                    })?;
                }
            }
            SarsatRxSetting::RepeatCombining => {
                let mut c = self.repeats.config();
                c.combining = REPEAT_COMBINING.read(id, v)?;
                self.repeats.set_config(c);
            }
            SarsatRxSetting::RepeatMinS => {
                let mut c = self.repeats.config();
                c.min_s = REPEAT_MIN_S.read(id, v)?;
                self.repeats.set_config(c);
            }
            SarsatRxSetting::RepeatMaxS => {
                let mut c = self.repeats.config();
                c.max_s = REPEAT_MAX_S.read(id, v)?;
                self.repeats.set_config(c);
            }
        }
        Ok(())
    }
    fn commands(&self) -> &'static [&'static Command] {
        SarsatRxCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SarsatRxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SarsatRxCommand::PresetApply => {
                let SarsatPresetArgs { preset } = SARSAT_PRESET_APPLY.args(args)?;
                self.request = preset.map(preset_hz);
            }
        }
        Ok(None)
    }
    fn values(&self) -> Values {
        let c = self.repeats.config();
        values_of([
            (SarsatRxSetting::WideSearch, Value::Bool(self.wide)),
            (SarsatRxSetting::RepeatCombining, Value::Bool(c.combining)),
            (SarsatRxSetting::RepeatMinS, Value::Duration(c.min_s)),
            (SarsatRxSetting::RepeatMaxS, Value::Duration(c.max_s)),
        ])
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.dial = f.signal_offset_hz();
        self.tuned = f.tuned_hz;
        self.next_index = 0;
        self.recent.clear();
        self.repeats.clear();
        self.build()?;
        let mut o = Format::new(Kind::typed::<SarsatItem>(), 0.0);
        o.centre_hz = f.centre_hz;
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o, Format::new(Kind::typed::<HostRequest>(), 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        // The finders start again at the new dial; a refusal keeps the old ones, which then find nothing new.
        self.dial = f.signal_offset_hz();
        self.tuned = f.tuned_hz;
        let _ = self.build();
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        if let Some(hz) = self.request.take() {
            out.items::<HostRequest>(1).push(Timed {
                time: first as f64 / self.rate.max(1.0),
                value: HostRequest::Retune {
                    tuned_hz: hz,
                    reason: "COSPAS-SARSAT preset".into(),
                },
            });
        }
        for s in iq {
            self.power += s.norm_sqr() as f64;
        }
        self.count += iq.len() as u64;
        self.samples += iq.len() as u64;
        self.next_index = first + iq.len() as u64;
        let items = out.items::<SarsatItem>(0);
        self.fgb(iq, items);
        self.sgb(iq, items);
    }
    fn tick(&mut self, out: &mut Outbox) {
        let level = if self.count > 0 {
            power_to_db(self.power / self.count as f64)
        } else {
            -200.0
        };
        self.power = 0.0;
        self.count = 0;
        let span = (self.finders.len().max(1) - 1) as f64 * BANK_STEP_HZ + FGB_WIDTH_HZ;
        out.items::<SarsatItem>(0).push(Timed {
            time: self.next_index as f64 / self.rate.max(1.0),
            value: SarsatItem::Status {
                level_dbfs: level,
                samples: self.samples,
                finders: self.finders.len(),
                span_hz: span,
            },
        });
    }
}

/// What the display publishes.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(SarsatFields::SHAPE.fields)
                .with_table(&SARSAT_BURSTS)
                .with_table(&SARSAT_BEACONS)
                .points("Beacons"),
        )
    })
    .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// One burst of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurstRow {
    /// When it was heard.
    pub at: Stamp,
    /// 1 or 2.
    pub generation: u8,
    /// The hex identity.
    pub hex_id: String,
    /// The beacon type.
    pub beacon_type: String,
    /// The country of its MID.
    pub country: String,
    /// The MID.
    pub mid: u32,
    /// The protocol in words.
    pub protocol: String,
    /// The identification in words.
    pub identification: String,
    /// The position, degrees.
    pub position: Option<(f64, f64)>,
    /// The 121.5 MHz homing flag.
    pub homing: bool,
    /// The self-test or test-protocol flag.
    pub self_test: bool,
    /// The SNR, dB.
    pub snr_db: f64,
    /// The level, dBFS (first generation only).
    pub level_dbfs: Option<f64>,
    /// The carrier offset from the dial, Hz.
    pub offset_hz: f64,
    /// Bits the BCH codes corrected.
    pub bits_corrected: usize,
    /// The message as hex.
    pub message_hex: String,
    /// How a first-generation message was reached (none for the second generation).
    pub repair: Option<Repair>,
}

impl Shift for BurstRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Shaped for BurstRow {
    type Row = SarsatBurstRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&SarsatBurstRow {
            at: Seconds(self.at.t),
            heard_at: self.at.wall.map(Time),
            generation: i64::from(self.generation),
            hex_id: &self.hex_id,
            beacon_type: &self.beacon_type,
            country: &self.country,
            protocol: &self.protocol,
            identification: &self.identification,
            position: self.position,
            homing: self.homing,
            snr_db: self.snr_db,
            level_dbfs: self.level_dbfs,
            offset_hz: Hertz(self.offset_hz),
        });
    }
}

/// One beacon of the beacon table: its last burst.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BeaconRow {
    /// Its last burst.
    pub last: BurstRow,
}

impl Shift for BeaconRow {
    fn shift_times(&mut self, by_s: f64) {
        self.last.shift_times(by_s);
    }
}

impl Shaped for BeaconRow {
    type Row = SarsatBeaconRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let b = &self.last;
        let r = b.repair.as_ref();
        out.put(&SarsatBeaconRow {
            hex_id: &b.hex_id,
            generation: i64::from(b.generation),
            beacon_type: &b.beacon_type,
            country: &b.country,
            identification: &b.identification,
            position: b.position,
            combined: r.map(|r| i64::from(r.combined)),
            confirmed: r.map(|r| r.confirmed),
            repair_margin: r.map(|r| r.margin),
            repair_cost: r.map(|r| r.cost),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon) = self.last.position?;
        Some(Point::new(
            sdrkit_util_geo::Position::new(lat, lon),
            format!("{} {}", self.last.beacon_type, self.last.hex_id),
        ))
    }
}

/// What the display keeps across sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// The burst log, newest last.
    pub bursts: LogRing<BurstRow>,
    /// The beacons, by hex identity.
    pub beacons: ObjectTable<String, BeaconRow>,
    /// Bursts heard.
    pub heard: u64,
    /// Bursts decoded.
    pub decoded: u64,
    /// Bursts whose checks failed.
    pub failed: u64,
    /// When the last beacon decoded.
    pub last_decoded: Option<f64>,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            bursts: LogRing::new(LOG_CAP),
            beacons: ObjectTable::new(BEACON_CAP),
            heard: 0,
            decoded: 0,
            failed: 0,
            last_decoded: None,
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.bursts.shift_times(by_s);
        self.beacons.shift_times(by_s);
        if let Some(t) = &mut self.last_decoded {
            *t += by_s;
        }
    }
}

impl Tables for Heard {
    const FORMAT: StateFormat = StateFormat::new("satcom.sarsat", 2);
    fn clear(&mut self) {
        self.bursts.clear();
        self.beacons.clear();
        self.heard = 0;
        self.decoded = 0;
        self.failed = 0;
        self.last_decoded = None;
    }
    fn adopt(&mut self, saved: Self) {
        self.bursts.adopt(saved.bursts);
        self.beacons.adopt(saved.beacons);
        self.heard = saved.heard;
        self.decoded = saved.decoded;
        self.failed = saved.failed;
        self.last_decoded = saved.last_decoded;
    }
}

/// The display: the beacon log, the counters and the events.
pub struct SarsatDisplay {
    clock: Clock,
    heard: Heard,
    centre_hz: f64,
    tuned_hz: f64,
    level: f64,
    samples: u64,
    cfo: f64,
    finders: usize,
    span: f64,
    live: bool,
    host_clock: sdrkit_core::Clock,
    /// The tables changed since the last report.
    dirty: bool,
    /// The summary of the status, kept to be written again.
    summary: String,
}

impl Default for SarsatDisplay {
    fn default() -> Self {
        SarsatDisplay {
            clock: Clock::default(),
            heard: Heard::default(),
            centre_hz: 0.0,
            tuned_hz: 0.0,
            level: -200.0,
            samples: 0,
            cfo: 0.0,
            finders: 1,
            span: FGB_WIDTH_HZ,
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            dirty: true,
            summary: String::new(),
        }
    }
}

impl SarsatDisplay {
    /// The tables held.
    pub fn heard(&self) -> &Heard {
        &self.heard
    }

    fn add(&mut self, row: BurstRow, out: &mut Outbox) {
        let freq = self.tuned_hz + row.offset_hz;
        let txt = format!(
            "{} {} {} {}{}",
            row.hex_id,
            row.beacon_type,
            row.country,
            row.identification,
            match row.position {
                Some((a, b)) => format!(" at {a:.5} {b:.5}"),
                None => String::new(),
            }
        );
        let urgency = if row.self_test {
            Urgency::None
        } else {
            Urgency::Emergency
        };
        out.messages(0).push(SARSAT_BEACON.event(
            row.at.t,
            freq,
            &SarsatBeaconEvent {
                source: &row.hex_id,
                text: &txt,
                snr_db: Some(row.snr_db).filter(|s| s.is_finite()),
                family: Family::Alert,
                urgency,
                position: row.position,
                generation: i64::from(row.generation),
                hex_id: &row.hex_id,
                beacon_type: &row.beacon_type,
                country: &row.country,
                mid: i64::from(row.mid),
                protocol: &row.protocol,
                identification: &row.identification,
                homing: row.homing,
                self_test: row.self_test,
                bits_corrected: row.bits_corrected as i64,
                offset_hz: Hertz(row.offset_hz),
                message_hex: &row.message_hex,
            },
        ));
        self.dirty = true;
        // A beacon table that refuses the time (not finite) keeps the burst in the log only.
        let _ = self
            .heard
            .beacons
            .upsert(&row.hex_id, row.at, || BeaconRow { last: row.clone() });
        if let Some(b) = self.heard.beacons.get_mut(&row.hex_id) {
            b.last = row.clone();
        }
        self.heard.bursts.push(row);
    }

    fn fgb_row(
        &self,
        t: f64,
        carrier_hz: f64,
        cn_db: f64,
        level: f64,
        m: &t001::Fgb,
        repair: Repair,
    ) -> BurstRow {
        BurstRow {
            at: self.clock.stamp(t),
            generation: 1,
            hex_id: m.hex_id.clone(),
            beacon_type: m.beacon_type.clone(),
            country: m.country.clone(),
            mid: m.mid,
            protocol: m.protocol_name.clone(),
            identification: m.identification.clone(),
            position: m.lat.zip(m.lon),
            homing: m.homing_121_5,
            self_test: m.self_test,
            snr_db: cn_db,
            level_dbfs: Some(level),
            offset_hz: carrier_hz,
            bits_corrected: m.bits_corrected,
            message_hex: m.raw_hex.clone(),
            repair: Some(repair),
        }
    }

    fn sgb_row(&self, t: f64, carrier_hz: f64, snr_db: f64, m: &t018::Sgb) -> BurstRow {
        BurstRow {
            at: self.clock.stamp(t),
            generation: 2,
            hex_id: m.hex_id_23.clone(),
            beacon_type: m.beacon_type.clone(),
            country: m.country.clone(),
            mid: m.mid,
            protocol: if m.test_protocol {
                "test protocol".into()
            } else {
                "second generation".into()
            },
            identification: format!("TAC {}, serial {}", m.tac, m.serial),
            position: m.lat.zip(m.lon),
            homing: m.homing_121_5,
            self_test: m.test_protocol,
            snr_db,
            level_dbfs: None,
            offset_hz: carrier_hz,
            bits_corrected: m.bits_corrected,
            message_hex: m.raw_hex.clone(),
            repair: None,
        }
    }
}

sdrkit_core::commands! {
    /// The commands of [`SarsatDisplay`].
    pub enum SarsatDisplayCommand {
        /// Forgets one object.
        Remove = SARSAT_REMOVE.command,
    }
}

impl Decoder for SarsatDisplay {
    type Input = Timed<SarsatItem>;
    fn name(&self) -> &'static str {
        "satcom.sarsat_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, x: &[Timed<SarsatItem>], _first: u64, out: &mut Outbox) {
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                SarsatItem::Status {
                    level_dbfs,
                    samples,
                    finders,
                    span_hz,
                } => {
                    self.level = *level_dbfs;
                    self.samples = self.samples.max(*samples).max(1);
                    self.finders = *finders;
                    self.span = *span_hz;
                }
                SarsatItem::Fgb {
                    carrier_hz,
                    cn_db,
                    level_dbfs,
                    message,
                    repair,
                } => {
                    self.heard.heard += 1;
                    match message {
                        Some(m) => {
                            self.heard.decoded += 1;
                            self.heard.last_decoded = Some(t);
                            self.cfo = *carrier_hz;
                            let row = self.fgb_row(t, *carrier_hz, *cn_db, *level_dbfs, m, *repair);
                            self.add(row, out);
                        }
                        None => self.heard.failed += 1,
                    }
                }
                SarsatItem::Sgb {
                    carrier_hz,
                    snr_db,
                    message,
                } => {
                    self.heard.heard += 1;
                    match message {
                        Some(m) => {
                            self.heard.decoded += 1;
                            self.heard.last_decoded = Some(t);
                            self.cfo = carrier_hz - SGB_OFFSET_HZ;
                            let row = self.sgb_row(t, *carrier_hz, *snr_db, m);
                            self.add(row, out);
                        }
                        None => self.heard.failed += 1,
                    }
                }
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let h = &self.heard;
        let recent = h.last_decoded.is_some_and(|t| now - t < RECENT_S);
        let state = if self.samples == 0 {
            0
        } else if recent {
            2
        } else {
            1
        };
        p.put(&SarsatFields {
            frames_ok: h.decoded as i64,
            frames_bad: h.failed as i64,
            bursts_heard: h.heard as i64,
            finders: self.finders as i64,
            span_hz: Hertz(self.span),
        });
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&h.bursts, &SARSAT_BURSTS, now, p);
            shaped::rows(&h.beacons, &SARSAT_BEACONS, now, p);
            shaped::points(&h.beacons, "sarsat/", p.results_mut());
        }
        let last = h.bursts.newest_first().next();
        let lock_state = LockState::from_level(state);
        let offset = Some(self.cfo);
        let snr = last.map(|b| b.snr_db as f32);
        let level = Some(self.level as f32);
        let summary = &mut self.summary;
        summary.clear();
        let _ = if self.samples == 0 {
            write!(summary, "COSPAS-SARSAT: waiting for samples")
        } else if let (true, Some(b)) = (recent, last) {
            write!(
                summary,
                "Beacon {} {}, {}",
                b.hex_id, b.beacon_type, b.country
            )
        } else if h.heard > 0 && h.decoded == 0 {
            write!(
                summary,
                "COSPAS-SARSAT: {} bursts heard, none decoded",
                h.heard
            )
        } else {
            write!(summary, "COSPAS-SARSAT: listening ({:.0} dBFS)", self.level)
        };
        set_status(p.results_mut(), lock_state, offset, snr, level, summary);
    }
    fn commands(&self) -> &'static [&'static Command] {
        SarsatDisplayCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SarsatDisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SarsatDisplayCommand::Remove => {
                let p = self.results();
                crate::display::remove(
                    &mut self.heard.beacons,
                    &SARSAT_BEACONS,
                    &p,
                    args,
                    &SARSAT_BEACON_ROW,
                )?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn clear(&mut self) {
        self.heard.clear();
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.heard.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

/// Adds the components of this stack to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.sarsat_receiver", || {
        Box::new(DecoderComponent::new(SarsatRx::new(false)))
    });
    r.add("satcom.sarsat_display", || {
        Box::new(DecoderComponent::new(SarsatDisplay::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("rx", DecoderComponent::new(SarsatRx::new(false)))
        .add("display", DecoderComponent::new(SarsatDisplay::default()))
        .input("rx")
        .connect("rx.out", "display")
        .output("display.events")
        .output("rx.requests")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "COSPAS-SARSAT 406 MHz needs a sample rate of at least 128 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .route("wide_search", &["rx.wide_search"])?
        .route("repeat_combining", &["rx.repeat_combining"])?
        .route("repeat_min_s", &["rx.repeat_min_s"])?
        .route("repeat_max_s", &["rx.repeat_max_s"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        )
        .command(
            SARSAT_PRESET_APPLY.command.key,
            sdrkit_graph::CommandTarget::of("rx", SARSAT_PRESET_APPLY.command.key),
        )
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::SarsatGenerator::from_config(cfg)))
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Distress beacons on 406 MHz, first and second generation: identity, protocol, country and position. Receive only.",
    tuning: Tuning {
        min_hz: 406.0e6,
        max_hz: 1546.0e6,
        default_hz: 406.028e6,
        rate_hz: 256e3,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.1e6,
        tune_offset_hz: 20e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(16e3),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: SarsatSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Cospas406],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

/// The verification of the stack: each generation's test beacons through the receiver.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut a = VerifyCase::new("test signal, first generation", o, 0.0);
    a.settings.push(CaseSetting::new(&gen::GEN_GAP_S, 0.8));
    a.seconds = a.seconds.max(4.0);
    a.expect_audio = Some(false);
    a.check = Some(check_decoded);
    let mut b = VerifyCase::new("test signal, second generation", o, 0.0);
    b.settings
        .push(CaseSetting::new(&gen::GEN_SECOND_GENERATION, true));
    b.settings.push(CaseSetting::new(&gen::GEN_GAP_S, 0.6));
    b.seconds = b.seconds.max(4.0);
    b.expect_audio = Some(false);
    b.check = Some(check_decoded);
    vec![a, b]
}

/// At least one beacon decoded and no burst failed its checks.
fn check_decoded(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = |k: &str| crate::field(r, k) as i64;
    let (ok, bad) = (
        n(SarsatFields::FRAMES_OK.key),
        n(SarsatFields::FRAMES_BAD.key),
    );
    vec![CheckLine::new(
        "beacons decoded",
        ok >= 1 && bad == 0,
        format!("{ok} decoded, {bad} failed"),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_receiver_refuses_a_low_rate() {
        use sdrkit_core::{Owner, Receiver};
        let values = Values::defaults(&SarsatSetting::schema());
        let mut rx = receiver(&StackConfig::new(values)).unwrap();
        let r = rx.configure(&sdrkit_core::InputInfo::new(100e3, 406.048e6, 406.028e6));
        assert!(r.is_err());
        assert_eq!(
            rx.results().status().summary,
            "COSPAS-SARSAT 406 MHz needs a sample rate of at least 128 ksps"
        );
    }
}
