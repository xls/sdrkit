//! A kind of radio and an open radio as owners: the settings every radio has, the results, events and commands, and the owner
//! that wraps a [`Source`].

use crate::address::{self, AddressPart};
use crate::form;
use crate::ring::{RingStats, RingWriter};
use crate::{
    Capabilities, DriverError, RateLimits, Retuned, Source, Started, StreamLayout, TxLimits,
};
use sdrkit_core::command::{Command, CommandSet};
use sdrkit_core::publish::field;
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::settings::{DeclareSetting, Range, SettingDecl, SettingSet};
use sdrkit_core::{
    CommandError, CommandId, Commands, Declare, Event, EventDef, Field, Hz, LockState, Owner,
    Published, RasterDef, Record, Results, ResultsDecl, Schema, SeriesDef, Setting, SettingError,
    SettingId, StateBlob, StateError, TableDef, Value, Values,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::sync::Arc;

/// The frequency of the centre of the samples, after any converter. An open radio limits it to the range that the radio
/// tunes.
pub const CENTRE: Setting<field::Hz> =
    Setting::new(form::CENTRE_FORM.key, form::CENTRE_FORM.label, "Hz", 100e6).range(0.0, 1e12, 1.0);
/// The overall gain, used when [`AGC`] is off. An open radio limits it to the gain range and the step of the radio; a
/// radio without a gain to set has it fixed.
pub const GAIN: Setting<field::F64> = Setting::new(form::GAIN_FORM.key, form::GAIN_FORM.label, "dB", 30.0)
    .range(-100.0, 200.0, 1.0)
    .section(form::GAIN_SECTION);
/// The radio's own automatic gain control. A radio without one has it fixed off.
pub const AGC: Setting<field::Bool> =
    Setting::new(form::AGC_FORM.key, form::AGC_FORM.label, "", true).section(form::GAIN_SECTION);
/// The sample rate. 0 lets the driver choose. An open radio limits it to the rates that the radio has and lists the
/// rates of the radio, or the usual rates inside its range ([`form::USUAL_RATES_HZ`]).
pub const RATE: Setting<field::Hz> = Setting::new(form::RATE_FORM.key, form::RATE_FORM.label, "Hz", 0.0)
    .range(0.0, 1e9, 1.0)
    .section(form::RATE_SECTION)
    .zero_means("Automatic")
    .help("The rate of the samples from the radio. On Automatic the radio takes the least rate at or above what the stack needs; a rate that is set stays, and a stack that needs more is refused. Write 0 to return to Automatic.");
/// The analogue filter. 0 lets the driver choose from the rate. An open radio limits it to the widths of its filter
/// ([`Capabilities::bandwidth_hz`]) and lists them; a radio whose filter cannot be set has it fixed.
pub const BANDWIDTH: Setting<field::Hz> =
    Setting::new(form::BANDWIDTH_FORM.key, form::BANDWIDTH_FORM.label, "Hz", 0.0)
        .range(0.0, 1e9, 1.0)
        .section(form::RATE_SECTION)
        .zero_means("From the sample rate")
        .help("The width of the analogue filter of the radio. Without a width, the driver chooses one that fits the sample rate.");
/// Send [`RATE`] as it is, even where the driver would pick another rate.
pub const RATE_EXACT: Setting<field::Bool> =
    Setting::new(form::RATE_EXACT_FORM.key, form::RATE_EXACT_FORM.label, "", false)
        .section(form::RATE_SECTION)
        .help("Sends the rate as it is, even outside the rates the driver picks.");
/// The bias tee: power on the antenna input. A radio without one has it fixed off.
pub const BIAS_TEE: Setting<field::Bool> =
    Setting::new(form::BIAS_TEE_FORM.key, form::BIAS_TEE_FORM.label, "", false)
        .section(form::ANTENNA_SECTION);
sdrkit_core::settings! {
    /// The settings every radio has, in the order of their sections ([`form::SECTIONS`]). A driver's own settings
    /// follow them in its schema.
    pub enum RadioSetting {
        /// [`CENTRE`].
        Centre = CENTRE,
        /// [`GAIN`].
        Gain = GAIN,
        /// [`AGC`].
        Agc = AGC,
        /// [`RATE`].
        Rate = RATE,
        /// [`BANDWIDTH`].
        Bandwidth = BANDWIDTH,
        /// [`RATE_EXACT`].
        RateExact = RATE_EXACT,
        /// [`BIAS_TEE`].
        BiasTee = BIAS_TEE,
    }
}

/// The id in a radio's schema of `setting`, one of the driver's own settings.
pub fn own_id<S: SettingSet>(setting: S) -> SettingId {
    SettingId(RadioSetting::SETTINGS.len() as u16 + setting.id().0)
}

/// The driver's own setting that `id` names in a radio's schema. `None` for a setting of [`RadioSetting`] or an id outside the
/// set.
pub fn own_setting<S: SettingSet>(id: SettingId) -> Option<S> {
    let first = RadioSetting::SETTINGS.len() as u16;
    id.0.checked_sub(first)
        .and_then(|i| S::from_id(SettingId(i)))
}

/// The carrier of a transmitter: the centre of the transmitted samples. A radio that can transmit limits it, the rate, the
/// filter width and the power to [`Capabilities::transmit`] when it opens.
pub const TX_FREQUENCY: Setting<field::Hz> =
    Setting::new("tx_frequency", "Transmit frequency", "Hz", 100e6).range(0.0, 1e12, 1.0);
/// The rate of the samples pushed to a transmitter. 0 takes the receiver's rate.
pub const TX_RATE: Setting<field::Hz> = Setting::new("tx_rate", "Transmit rate", "Hz", 0.0)
    .range(0.0, 1e9, 1.0)
    .zero_means("The receive rate");
/// The analogue filter of a transmitter. 0 lets the driver choose from the rate.
pub const TX_BANDWIDTH: Setting<field::Hz> =
    Setting::new("tx_bandwidth", "Transmit bandwidth", "Hz", 0.0)
        .range(0.0, 1e9, 1.0)
        .zero_means("From the transmit rate");
/// The output power relative to the full output of the radio: 0 dB is full power, a negative value is below it. The
/// driver sets its attenuator to the opposite value. An open radio limits it to [`TxLimits::power_db`] and starts it at
/// [`TxLimits::start_power_db`]; a radio whose power cannot be set has it fixed.
pub const TX_POWER: Setting<field::F64> = Setting::new("tx_power", "Transmit power", "dB", -10.0)
    .range(-200.0, 0.0, 0.0)
    .common(&[0.0, -3.0, -6.0, -10.0, -20.0, -30.0])
    .help("The output relative to the full output of the radio: 0 dB is full power, -10 dB is a tenth of it.");

sdrkit_core::settings! {
    /// The settings every transmitter has. A driver's own transmit settings follow them.
    pub enum TxSetting {
        /// [`TX_FREQUENCY`].
        Frequency = TX_FREQUENCY,
        /// [`TX_RATE`].
        Rate = TX_RATE,
        /// [`TX_BANDWIDTH`].
        Bandwidth = TX_BANDWIDTH,
        /// [`TX_POWER`].
        Power = TX_POWER,
    }
}

/// The transmit schema of a driver whose own transmit settings are `own`, with the carrier, the rate, the filter width
/// and the power limited to `limits`. A rate or a width of 0 keeps its meaning. A filter that cannot be set has
/// [`TX_BANDWIDTH`] fixed, and a power that cannot be set has [`TX_POWER`] fixed. A limit that is not known leaves the
/// declared range. A power that cannot be set is fixed at 0 dB: the driver applies no attenuation and the radio sends at
/// its own level. The common values are those of `limits` and the declarations inside the limits.
pub fn tx_schema(own: &[&dyn DeclareSetting], limits: &TxLimits) -> Arc<Schema> {
    let list: Vec<&dyn DeclareSetting> = TxSetting::SETTINGS
        .iter()
        .copied()
        .chain(own.iter().copied())
        .collect();
    let mut decls = Schema::of(&list).settings().to_vec();
    let at = |s: TxSetting| s.id().index();
    if limits.max_freq_hz > limits.min_freq_hz {
        limit(
            &mut decls[at(TxSetting::Frequency)],
            limits.min_freq_hz,
            limits.max_freq_hz,
            1.0,
        );
    }
    limit_rate(&mut decls[at(TxSetting::Rate)], &limits.rates);
    common_in_range(
        &mut decls[at(TxSetting::Rate)],
        &rate_list(&limits.common_rates_hz, &limits.rates),
    );
    limit_filter(&mut decls[at(TxSetting::Bandwidth)], limits.bandwidth_hz);
    width_list(&mut decls[at(TxSetting::Bandwidth)], &[]);
    let (min, max, step) = limits.power_db;
    let power = &mut decls[at(TxSetting::Power)];
    if max > min {
        limit(power, min, max, step);
        power.default = Value::F64(limits.start_power_db.unwrap_or(min).clamp(min, max));
        let common = power.common.values.clone();
        common_in_range(power, &common);
    } else {
        power.fixed = true;
        power.default = Value::F64(0.0);
        power.common.values.clear();
    }
    Arc::new(Schema::new(decls).expect("the transmit settings make a valid schema"))
}

/// Limits `decl` to `min` to `max` in steps of `step` and moves its default into the limits. A default of 0 that has a
/// meaning stays 0.
fn limit(decl: &mut SettingDecl, min: f64, max: f64, step: f64) {
    decl.range = Some(Range { min, max, step });
    let (meant, special) = (!decl.zero_means.is_empty(), decl.special_value);
    if let Value::Hz(x) | Value::F64(x) = &mut decl.default {
        if !(meant && *x == special) {
            *x = x.clamp(min, max);
        }
    }
}

/// Sets the common values of `decl` to those of `values` that lie inside its range, in increasing order without
/// repeats.
fn common_in_range(decl: &mut SettingDecl, values: &[f64]) {
    let (min, max) = decl
        .range
        .map_or((f64::NEG_INFINITY, f64::INFINITY), |r| (r.min, r.max));
    let mut v: Vec<f64> = values
        .iter()
        .copied()
        .filter(|x| x.is_finite() && (min..=max).contains(x))
        .collect();
    v.sort_by(f64::total_cmp);
    v.dedup();
    decl.common.values = v;
}

/// The rates to list for a radio with `rates`: those of `common` inside its ranges (any, if it lists none); else its
/// single rates, when it lists only such; else the usual rates ([`form::USUAL_RATES_HZ`]) inside its ranges. What the
/// link carries now ([`RateLimits::steady_hz`]) never shortens the list.
fn rate_list(common: &[Hz], rates: &RateLimits) -> Vec<Hz> {
    let inside = |x: &Hz| rates.ranges.is_empty() || rates.ranges.iter().any(|(a, b)| (a..=b).contains(&x));
    if !common.is_empty() {
        return common.iter().copied().filter(inside).collect();
    }
    if !rates.ranges.is_empty() && rates.ranges.iter().all(|(a, b)| a == b) {
        return rates.ranges.iter().map(|(a, _)| *a).collect();
    }
    form::USUAL_RATES_HZ.iter().copied().filter(inside).collect()
}

/// Lists the widths of a filter that can be set: `common`, or the usual widths ([`form::USUAL_WIDTHS_HZ`]) when it is
/// empty, inside the range of `decl`. A fixed filter lists none.
fn width_list(decl: &mut SettingDecl, common: &[Hz]) {
    if decl.fixed {
        decl.common.values.clear();
        return;
    }
    common_in_range(decl, if common.is_empty() { &form::USUAL_WIDTHS_HZ } else { common });
}

/// Fixes `decl`, a setting that the radio does not have, at `value`, without common values.
fn absent(decl: &mut SettingDecl, value: Value) {
    decl.fixed = true;
    decl.default = value;
    decl.common.values.clear();
}

/// Limits the rate `decl` to the lowest and the highest rate of `rates`, if they are known.
fn limit_rate(decl: &mut SettingDecl, rates: &RateLimits) {
    if rates.max_hz > 0.0 && rates.max_hz >= rates.min_hz {
        limit(decl, rates.min_hz, rates.max_hz, 1.0);
    }
}

/// Limits the filter width `decl` to `width` (min, max). Without a width that can be set, the setting is fixed at its
/// default.
fn limit_filter(decl: &mut SettingDecl, width: Option<(Hz, Hz)>) {
    match width.filter(|(min, max)| *max > 0.0 && max >= min) {
        Some((min, max)) => limit(decl, min, max, 1.0),
        None => decl.fixed = true,
    }
}

/// Ends the connection and connects again with the same settings.
pub const RECONNECT: Command = Command::new("reconnect", "Reconnect");

sdrkit_core::commands! {
    /// The commands every radio has. A driver's own commands follow them.
    pub enum RadioCommand {
        /// [`RECONNECT`].
        Reconnect = RECONNECT,
    }
}

sdrkit_core::choice_enum! {
    /// Where the link to a radio is.
    pub enum Link {
        /// Open, not streaming.
        Idle = ("idle", "Idle"),
        /// Connecting for the first time.
        Connecting = ("connecting", "Connecting"),
        /// Streaming.
        Up = ("up", "Up"),
        /// The link dropped. The driver connects again.
        Away = ("away", "Reconnecting"),
        /// Stopped.
        Stopped = ("stopped", "Stopped"),
    }
}

/// Where the link is.
pub const STATE: Field<field::Choice<Link>> = Field::new("state", "Link", "");
/// The rate the radio provides.
pub const RATE_GIVEN: Field<field::Hz> = Field::new("rate_hz", "Sample rate", "Hz");
/// The centre frequency of the samples.
pub const CENTRE_GIVEN: Field<field::Hz> = Field::new("centre_hz", "Centre", "Hz");
/// Samples written into the ring.
pub const SAMPLES: Field<field::I64> = Field::new("samples", "Samples", "");
/// Samples waiting in the ring, in percent of its size.
pub const BUFFER_PCT: Field<field::F64> = Field::new("buffer_pct", "Buffer", "%");
/// Samples the radio or its link did not deliver.
pub const RADIO_LOST: Field<field::I64> = Field::new("radio_lost", "Lost by the radio", "");
/// Samples missed while the link was down.
pub const LINK_DOWN: Field<field::I64> = Field::new("link_down", "Lost while away", "");
/// The delivered rate in percent of the set rate.
pub const DELIVERED_PCT: Field<field::F64> = Field::new("delivered_pct", "Delivered", "%");
/// The last error of the radio, if any.
pub const PROBLEM: Field<field::Text> = Field::new("problem", "Problem", "");

/// The output of the transmitter at its carrier and its power: an estimate, not measured, from typical figures of the
/// device ([`TxLimits::output_dbm`]). A driver whose transmitter has the figures lists it in its readings. No value
/// without figures for the carrier.
pub const TX_OUTPUT_DBM: Field<field::F64> =
    Field::new("tx_output_dbm", "Output (estimate, not measured)", "dBm");

/// The result fields every radio has.
pub const STANDARD_FIELDS: &[&dyn Declare] = &[
    &STATE,
    &RATE_GIVEN,
    &CENTRE_GIVEN,
    &SAMPLES,
    &keys::DROPPED,
    &BUFFER_PCT,
    &RADIO_LOST,
    &LINK_DOWN,
    &DELIVERED_PCT,
    &PROBLEM,
];

const EVENT_FIELDS: &[&dyn Declare] = &[&keys::SOURCE, &keys::TEXT, &keys::FAMILY];

/// The link to the radio is up.
pub static CONNECTED: EventDef = EventDef::new("radio.connected", "Radio connected", EVENT_FIELDS);
/// The link to the radio dropped.
pub static LOST: EventDef = EventDef::new("radio.lost", "Radio lost", EVENT_FIELDS);
/// The ring was full and samples were dropped.
pub static OVERFLOW: EventDef = EventDef::new("radio.overflow", "Samples dropped", EVENT_FIELDS);
/// A change needs the stream stopped and started again with a new ring.
pub static RESTART: EventDef = EventDef::new("radio.restart", "Restart needed", EVENT_FIELDS);

/// The events of every radio.
pub static RADIO_EVENTS: [&EventDef; 4] = [&CONNECTED, &LOST, &OVERFLOW, &RESTART];

/// A stream that a radio provides or takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamFormat {
    /// The kind of samples ([`Kind::Iq`] for most radios).
    pub kind: Kind,
    /// How one sample is stored in the ring.
    pub sample: SampleType,
    /// The lowest rate of any radio of the kind.
    pub min_rate_hz: f64,
    /// The highest rate of any radio of the kind.
    pub max_rate_hz: f64,
    /// The samples per frame: 1 for one channel, 2 for stereo audio.
    pub channels: u16,
}

/// How many radios of one address a host has open at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opens {
    /// One. The device has one receiver with one rate and one tuning, so a second connection changes what the first
    /// reads. The sessions of a host share the one radio.
    Once,
    /// Any number. Each radio is a receiver of its own: a channel of a server that serves many clients, a file, a
    /// generator.
    Many,
}

/// What a host reads about a kind of radio before it opens one.
pub struct DriverDescriptor {
    /// The lookup key ("rtl_tcp", "iiod", "file", "synth").
    pub key: &'static str,
    /// A name for people.
    pub name: &'static str,
    /// The address forms that the driver opens, each with the driver key as its scheme (`rtl_tcp:<host>:<port>`). A form
    /// can end with a note in parentheses.
    pub addresses: &'static [&'static str],
    /// How many radios of one address a host has open at a time.
    pub opens: Opens,
    /// The parts of an address, in the order a person fills them in. They are the arguments of the `address` command of
    /// the kind, which returns the address composed from them ([`address::compose`]).
    pub address: &'static [AddressPart],
    /// The driver's own settings, in groups. They follow [`RadioSetting`] in the schema, group after group.
    pub settings: &'static [&'static [&'static dyn DeclareSetting]],
    /// The driver's own commands: device actions that are not settings. They follow [`RadioCommand`].
    pub commands: &'static [&'static Command],
    /// The device's readings. They follow [`STANDARD_FIELDS`] in the results.
    pub readings: &'static [&'static dyn Declare],
    /// The streams it produces.
    pub produces: &'static [StreamFormat],
    /// The streams it accepts (a transmitter). Empty if it cannot transmit.
    pub accepts: &'static [StreamFormat],
    /// The device's lists, as tables of the results.
    pub tables: &'static [&'static TableDef],
    /// The device's pictures, as rasters of the results.
    pub rasters: &'static [&'static RasterDef],
    /// The device's plots, as series of the results.
    pub series: &'static [&'static SeriesDef],
    /// The label of the map points of the results (a receiver's place). Empty for a kind without points.
    pub points: &'static str,
    /// The commands of the kind itself, before any radio is open ([`SourceDriver::invoke_kind`](crate::SourceDriver)):
    /// find radios, test an address. The `address` command follows them in the commands of the kind.
    pub kind_commands: &'static [&'static Command],
    /// The lists of the kind itself, as tables of its results (the radios that a search found).
    pub kind_tables: &'static [&'static TableDef],
}

impl std::fmt::Debug for DriverDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key)
    }
}

/// The declarations that a descriptor returns, built once.
#[derive(Debug)]
pub struct KindDecl {
    schema: Arc<Schema>,
    radio_commands: Arc<Commands>,
    radio_results: Arc<ResultsDecl>,
    kind_commands: Arc<Commands>,
    kind_results: Arc<ResultsDecl>,
}

impl DriverDescriptor {
    /// The schema, commands and results declaration of a radio of this kind.
    ///
    /// # Panics
    ///
    /// If two settings, commands, fields, tables, rasters or address parts share a key, or a command of the kind has the
    /// key `address`. That is a fault of the descriptor.
    pub fn decl(&self) -> KindDecl {
        let settings: Vec<&dyn DeclareSetting> = RadioSetting::SETTINGS
            .iter()
            .chain(self.settings.iter().flat_map(|g| g.iter()))
            .copied()
            .collect();
        let commands: Vec<&Command> = RadioCommand::COMMANDS
            .iter()
            .chain(self.commands)
            .copied()
            .collect();
        let fields: Vec<&dyn Declare> = STANDARD_FIELDS
            .iter()
            .chain(self.readings)
            .copied()
            .collect();
        let mut results = Published::new(&fields).results().decl().as_ref().clone();
        for t in self.tables {
            results = results.with_table(t);
        }
        for r in self.rasters {
            results = results.with_raster(r);
        }
        for s in self.series {
            results = results.with_series(s);
        }
        if !self.points.is_empty() {
            results = results.points(self.points);
        }
        for e in RADIO_EVENTS {
            results = results.event(e);
        }
        let mut kind_results = ResultsDecl::new();
        for t in self.kind_tables {
            kind_results = kind_results.with_table(t);
        }
        let kind_commands = self
            .kind_commands
            .iter()
            .map(|c| c.def())
            .chain([address::command(self.address)])
            .collect();
        let kind_commands = match Commands::new(kind_commands) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        };
        KindDecl {
            schema: Arc::new(kind_schema(&settings, self.produces)),
            radio_commands: Arc::new(Commands::of(&commands)),
            radio_results: Arc::new(results),
            kind_commands: Arc::new(kind_commands),
            kind_results: Arc::new(kind_results),
        }
    }

    /// The id in a radio's schema of the `i`-th setting that the open radio adds ([`Source::settings`]).
    pub fn run_time_id(&self, i: usize) -> SettingId {
        let own: usize = self.settings.iter().map(|g| g.len()).sum();
        SettingId((RadioSetting::SETTINGS.len() + own + i) as u16)
    }
}

/// The schema of `settings` for a kind that `produces` these streams: the rate with the usual rates inside the rates of
/// the streams, and the filter width with the usual widths up to the highest rate (those rates, if no usual width is
/// below it).
fn kind_schema(settings: &[&dyn DeclareSetting], produces: &[StreamFormat]) -> Schema {
    let mut decls = Schema::of(settings).settings().to_vec();
    let min = produces.iter().map(|p| p.min_rate_hz).fold(f64::INFINITY, f64::min);
    let max = produces.iter().map(|p| p.max_rate_hz).fold(0.0, f64::max);
    let min = min.min(max);
    let rates = RateLimits {
        min_hz: min,
        max_hz: max,
        ranges: if min == max { vec![(min, max)] } else { Vec::new() },
        steady_hz: None,
    };
    // The range stays as declared: a driver takes a rate outside its streams in its own way, and an open radio narrows it.
    let listed = rate_list(&[], &rates);
    decls[RadioSetting::Rate.id().index()].common.values = listed.clone();
    let widths: Vec<Hz> = form::USUAL_WIDTHS_HZ.iter().copied().filter(|w| *w <= max).collect();
    decls[RadioSetting::Bandwidth.id().index()].common.values =
        if widths.is_empty() { listed } else { widths };
    Schema::new(decls).expect("the settings of a kind make a valid schema")
}

impl KindDecl {
    /// The schema of a radio of the kind, with the defaults.
    pub fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
    /// The commands of a radio of the kind.
    pub fn commands(&self) -> &Arc<Commands> {
        &self.radio_commands
    }
    /// The results declaration of a radio of the kind.
    pub fn results(&self) -> &Arc<ResultsDecl> {
        &self.radio_results
    }
}

/// A kind of radio as an owner: the schema of a radio of the kind with the defaults (it has no values), the commands of the
/// kind ([`DriverDescriptor::kind_commands`], then `address`) and the results of the kind
/// ([`DriverDescriptor::kind_tables`]). The declarations of an open radio are in [`DriverKind::decl`].
pub struct DriverKind {
    driver: Box<dyn crate::SourceDriver>,
    decl: KindDecl,
    results: Published,
}

impl DriverKind {
    /// The kind of `driver`.
    pub fn new(driver: Box<dyn crate::SourceDriver>) -> Self {
        let decl = driver.descriptor().decl();
        let results = Published::from_decl(decl.kind_results.clone());
        DriverKind {
            driver,
            decl,
            results,
        }
    }

    /// The descriptor.
    pub fn descriptor(&self) -> &'static DriverDescriptor {
        self.driver.descriptor()
    }

    /// The declarations of a radio of the kind.
    pub fn decl(&self) -> &KindDecl {
        &self.decl
    }

    /// The schema of the settings that an open of `address` takes: those of the kind ([`Owner::schema`]), then those
    /// that the radio at `address` adds and the driver knows before it opens
    /// ([`SourceDriver::settings_at`](crate::SourceDriver::settings_at)).
    ///
    /// # Errors
    ///
    /// What the driver returns for the address, and [`DriverError::Schema`] if its settings do not make a schema with
    /// those of the kind.
    pub fn schema_at(&self, address: &str) -> Result<Arc<Schema>, DriverError> {
        let added = self.driver.settings_at(address)?;
        if added.is_empty() {
            return Ok(Arc::clone(&self.decl.schema));
        }
        let mut decls = self.decl.schema.settings().to_vec();
        decls.extend(added);
        Schema::new(decls).map(Arc::new).map_err(DriverError::Schema)
    }

    /// Opens the radio at `address` with `values`, of the schema of the kind or of [`DriverKind::schema_at`]. The radio
    /// starts with every value of that schema. It does not stream yet.
    ///
    /// # Errors
    ///
    /// [`DriverError::Setting`] if a value does not fit the schema, else what the driver returns.
    pub fn open(&self, address: &str, values: &Values) -> Result<OpenRadio, DriverError> {
        let values = copy_values(&self.schema_at(address)?, values)?;
        let source = self.driver.open(address, &values)?;
        OpenRadio::new(
            self.driver.descriptor(),
            &self.decl,
            address,
            &values,
            source,
        )
    }
}

impl Owner for DriverKind {
    fn schema(&self) -> &Arc<Schema> {
        &self.decl.schema
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        &self.decl.kind_results
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
    fn commands(&self) -> &Commands {
        &self.decl.kind_commands
    }
    /// Runs command `id` of the kind. The `address` command returns the address composed from its arguments
    /// ([`address::compose`]) and does not call the driver. Every other command goes to
    /// [`SourceDriver::invoke_kind`](crate::SourceDriver).
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let d = self.driver.descriptor();
        let def = self
            .decl
            .kind_commands
            .def(id)
            .ok_or(CommandError::NoSuchId { id })?;
        if id.index() == d.kind_commands.len() {
            let text = address::compose(d.key, d.address, args)?;
            let mut reply = def.new_reply();
            reply.set(&address::ADDRESS, &text);
            return Ok(Some(reply));
        }
        self.driver
            .invoke_kind(id, args, &mut self.results)
            .map_err(|e| CommandError::Failed {
                source: sdrkit_core::StageError::new(e),
            })
    }
}

/// An open radio as an owner. It holds the values of its settings, applies a change to the [`Source`], and publishes the link
/// state, the counters and the device's readings. The host calls [`OpenRadio::poll`] about once a second.
pub struct OpenRadio {
    kind: &'static DriverDescriptor,
    name: String,
    source: Box<dyn Source>,
    kind_schema: Arc<Schema>,
    schema: Arc<Schema>,
    values: Values,
    commands: Arc<Commands>,
    results: Published,
    rings: Vec<Arc<RingStats>>,
    link: Link,
    drop_events: u64,
    restart: bool,
    /// The transmit settings, and the id in [`OpenRadio::schema`] of the first one. `None` for a radio that cannot transmit.
    tx: Option<(Arc<Schema>, u16)>,
    /// Moves on at each change of a transmit setting.
    tx_seq: u64,
}

impl OpenRadio {
    fn new(
        kind: &'static DriverDescriptor,
        decl: &KindDecl,
        address: &str,
        values: &Values,
        source: Box<dyn Source>,
    ) -> Result<Self, DriverError> {
        let mut radio = OpenRadio {
            kind,
            name: address.to_owned(),
            source,
            kind_schema: decl.schema.clone(),
            schema: decl.schema.clone(),
            values: values.clone(),
            commands: decl.radio_commands.clone(),
            results: Published::from_decl(decl.radio_results.clone()),
            rings: Vec::new(),
            link: Link::Idle,
            drop_events: 0,
            restart: false,
            tx: None,
            tx_seq: 0,
        };
        // A value that the device chose at open (a frequency that the server holds) replaces the requested one. A value of a
        // setting that the radio adds is set once the schema has it.
        let polled = radio.source.poll(radio.results.results_mut());
        let stored = polled.store;
        if let Some((id, value)) = stored
            .as_ref()
            .filter(|(id, _)| id.index() < decl.schema.len())
        {
            radio.values.set(*id, value)?;
        }
        radio.narrow()?;
        if let Some((id, value)) = stored.filter(|(id, _)| id.index() >= decl.schema.len()) {
            radio.values.set(id, &value)?;
        }
        radio.results.set(&STATE, Link::Idle);
        radio.publish_tx_output();
        Ok(radio)
    }

    /// True if the centre of the samples carries no carrier of the radio's own converter ([`form::centre_is_clean`]).
    pub fn centre_clean(&self) -> bool {
        form::centre_is_clean(&self.values)
    }

    /// The descriptor of the kind.
    pub fn kind(&self) -> &'static DriverDescriptor {
        self.kind
    }

    /// What the radio can do.
    pub fn capabilities(&self) -> &Capabilities {
        self.source.capabilities()
    }

    /// How the samples lie in the rings.
    pub fn layout(&self) -> StreamLayout {
        self.source.layout()
    }

    /// How one sample is stored in the rings. Make the rings with this type.
    pub fn sample_type(&self) -> SampleType {
        self.source.sample_type()
    }

    /// Starts streaming into `sink`.
    ///
    /// # Errors
    ///
    /// What the driver returns.
    pub fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        self.start_streams(vec![sink])
    }

    /// Starts streaming into one ring per receiver ([`Source::start_streams`]).
    ///
    /// # Errors
    ///
    /// What the driver returns.
    pub fn start_streams(&mut self, sinks: Vec<RingWriter>) -> Result<Started, DriverError> {
        let want = self.source.sample_type();
        if let Some(w) = sinks.iter().find(|w| w.sample_type() != want) {
            return Err(DriverError::WrongSampleType {
                ring: w.sample_type(),
                radio: want,
            });
        }
        let rings: Vec<_> = sinks.iter().map(RingWriter::stats).collect();
        let r = match sinks.len() {
            1 => self
                .source
                .start(sinks.into_iter().next().ok_or(DriverError::Thread)?),
            _ => self.source.start_streams(sinks),
        };
        match &r {
            Ok(s) => {
                self.rings = rings;
                self.drop_events = 0;
                self.results.set(&RATE_GIVEN, s.rate_hz);
                self.results.set(&CENTRE_GIVEN, s.centre_hz);
                self.clear_problem();
            }
            Err(e) => self.problem(e),
        }
        r
    }

    /// Stops streaming. Safe to call twice.
    pub fn stop(&mut self) {
        self.source.stop();
        self.rings.clear();
    }

    /// Opens the transmitter, off air, with the radio's transmit settings ([`OpenRadio::tx_values`]).
    ///
    /// # Errors
    ///
    /// [`DriverError::CannotTransmit`] for a radio that cannot transmit, else what the driver returns.
    pub fn transmitter(&mut self) -> Result<Box<dyn crate::Transmitter>, DriverError> {
        let values = self.tx_values().ok_or(DriverError::CannotTransmit)?;
        self.source.transmitter(&values)
    }

    /// The transmit settings of the radio as values of its transmit schema ([`Source::tx_schema`]). A fixed setting has
    /// its default. `None` for a radio that cannot transmit.
    pub fn tx_values(&self) -> Option<Values> {
        let (schema, first) = self.tx.as_ref()?;
        let mut v = Values::defaults(schema);
        for (i, decl) in schema.settings().iter().enumerate() {
            let id = SettingId(i as u16);
            if decl.fixed {
                continue;
            }
            if let Some(x) = self.values.value(SettingId(first + id.0)) {
                v.set(id, x).ok()?;
            }
        }
        Some(v)
    }

    /// A number that moves on at each change of a transmit setting.
    pub fn tx_seq(&self) -> u64 {
        self.tx_seq
    }

    /// Sets the transmit frequency of the radio to `centre_hz` and, if `rate_hz` is given, its transmit rate. A value
    /// that the radio has already is not set again, so [`OpenRadio::tx_seq`] moves on only at a change. A fixed
    /// setting keeps its value. Does nothing for a radio that cannot transmit.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if a value is outside the limits of the radio. A value before it is set.
    pub fn tx_tune(&mut self, centre_hz: f64, rate_hz: Option<f64>) -> Result<(), SettingError> {
        let Some((schema, first)) = self.tx.as_ref() else {
            return Ok(());
        };
        let (schema, first) = (Arc::clone(schema), *first);
        let wanted = [
            (TxSetting::Frequency, Some(centre_hz)),
            (TxSetting::Rate, rate_hz),
        ];
        for (setting, hz) in wanted {
            let Some(hz) = hz else { continue };
            let id = setting.id();
            if schema.setting(id).is_none_or(|d| d.fixed) {
                continue;
            }
            let own = SettingId(first + id.0);
            let value = Value::Hz(hz);
            if self.values.value(own) != Some(&value) {
                self.values.set(own, &value)?;
                self.tx_seq += 1;
            }
        }
        self.publish_tx_output();
        Ok(())
    }

    /// What the radio keeps across a restart (what it learned about a server), from [`Source::save_state`].
    pub fn save_state(&self) -> Option<StateBlob> {
        self.source.save_state()
    }

    /// Restores the radio from what [`OpenRadio::save_state`] kept.
    ///
    /// # Errors
    ///
    /// What [`Source::restore_state`] returns.
    pub fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.source.restore_state(state)
    }

    /// Brings the results up to date and adds the radio's events to `events`. `now` is the time of the events (seconds since
    /// 1970).
    pub fn poll(&mut self, now: f64, events: &mut Vec<Event>) {
        let polled = self.source.poll(self.results.results_mut());
        if polled.capabilities {
            if let Err(e) = self.narrow() {
                self.problem(&e);
            }
        }
        if let Some((id, value)) = polled.store {
            let _ = self.values.set(id, &value);
        }
        if polled.restart || std::mem::take(&mut self.restart) {
            events.push(self.event(&RESTART, now, ""));
        }
        let link = self.source.link();
        if link != self.link {
            match link {
                Link::Up => {
                    // The link is back: a problem of the link before is gone.
                    self.clear_problem();
                    events.push(self.event(&CONNECTED, now, ""));
                }
                Link::Away => events.push(self.event(&LOST, now, "")),
                _ => {}
            }
            self.link = link;
            self.results.set(&STATE, link);
        }
        let (mut written, mut dropped, mut drops, mut fill) = (0u64, 0u64, 0u64, 0.0f64);
        for r in &self.rings {
            written += r.written();
            dropped += r.cpu_lost();
            drops += r.drop_events();
            fill = fill.max(r.fill());
        }
        if drops > self.drop_events {
            events.push(self.event(&OVERFLOW, now, ""));
            self.drop_events = drops;
        }
        self.results
            .set(&SAMPLES, i64::try_from(written).unwrap_or(i64::MAX));
        self.results
            .set(&keys::DROPPED, i64::try_from(dropped).unwrap_or(i64::MAX));
        self.results.set(&BUFFER_PCT, fill * 100.0);
        if let Some(l) = self.source.loss() {
            self.results
                .set(&RADIO_LOST, i64::try_from(l.radio_lost).unwrap_or(i64::MAX));
            self.results
                .set(&LINK_DOWN, i64::try_from(l.link_down).unwrap_or(i64::MAX));
            self.results.opt(&DELIVERED_PCT, l.delivered_pct);
        }
        self.publish_tx_output();
        let lock_state = match link {
            Link::Up => LockState::Locked,
            Link::Connecting | Link::Away => LockState::Searching,
            Link::Idle | Link::Stopped => LockState::Off,
        };
        self.results.results_mut().update_status(|s| {
            let changed = s.lock_state != lock_state;
            s.lock_state = lock_state;
            changed
        });
    }

    /// Writes [`TX_OUTPUT_DBM`] from the transmit carrier and power, if the kind lists the reading.
    fn publish_tx_output(&mut self) {
        if self.results.results().decl().fields().id(TX_OUTPUT_DBM.key).is_none() {
            return;
        }
        let estimate = self.tx.as_ref().and_then(|(_, first)| {
            let at = |s: TxSetting| SettingId(first + s.id().0);
            let carrier = match self.values.value(at(TxSetting::Frequency))? {
                Value::Hz(x) => *x,
                _ => return None,
            };
            let power = match self.values.value(at(TxSetting::Power))? {
                Value::F64(x) => *x,
                _ => return None,
            };
            self.source
                .capabilities()
                .transmit
                .as_ref()?
                .output_dbm(carrier, power)
        });
        self.results.opt(&TX_OUTPUT_DBM, estimate);
    }

    fn event(&self, def: &'static EventDef, now: f64, text: &str) -> Event {
        def.event(now, self.values.get(&CENTRE))
            .set(&keys::SOURCE, &self.name)
            .set(&keys::TEXT, text)
            .set(&keys::FAMILY, Family::Receiver)
    }

    fn problem(&mut self, e: &DriverError) {
        Owner::problem(self, &e.to_string());
    }

    /// Removes the problem that [`Owner::problem`] wrote: the field [`PROBLEM`], and the summary of the status while it
    /// is still that text.
    fn clear_problem(&mut self) {
        let Some(text) = self.results.results().fields().get(&PROBLEM).map(str::to_owned) else {
            return;
        };
        self.results.none(&PROBLEM);
        self.results.results_mut().update_status(|s| {
            let same = s.summary == text;
            if same {
                s.summary.clear();
            }
            same
        });
    }

    /// Narrows the ranges of frequency, rate, filter width and gain to what the open radio can do, and moves their
    /// defaults and the values held into the ranges. The common values of rate, width and gain are those of
    /// [`Capabilities`] inside the ranges, the usual rates and widths ([`form`]) where the radio lists none. The rate
    /// range runs from the lowest to the highest rate of the radio ([`Capabilities::rates`]); inside it the driver picks
    /// the nearest rate that it runs. The width range is [`Capabilities::bandwidth_hz`]; a radio whose filter cannot be
    /// set has [`BANDWIDTH`] fixed at 0. A value of 0 that has a meaning stays valid. A radio without a gain to set has
    /// [`GAIN`] fixed, one without automatic gain control or a bias tee has [`AGC`] or [`BIAS_TEE`] fixed off. The settings that
    /// the open radio adds ([`Source::settings`]) follow, then its transmit settings ([`Source::tx_schema`]). It returns
    /// [`DriverError::Schema`] if they do not make a schema with those of the kind.
    fn narrow(&mut self) -> Result<(), DriverError> {
        let caps = self.source.capabilities();
        let mut decls: Vec<SettingDecl> = self.kind_schema.settings().to_vec();
        let at = |s: RadioSetting| s.id().index();
        if caps.max_freq_hz > caps.min_freq_hz {
            limit(
                &mut decls[at(RadioSetting::Centre)],
                caps.min_freq_hz,
                caps.max_freq_hz,
                1.0,
            );
        }
        if caps.rates.max_hz > 0.0 {
            limit_rate(&mut decls[at(RadioSetting::Rate)], &caps.rates);
        } else {
            // Rates not known yet: those of the streams of the kind.
            let p = self.kind.produces;
            let lo = p.iter().map(|s| s.min_rate_hz).fold(f64::INFINITY, f64::min);
            let hi = p.iter().map(|s| s.max_rate_hz).fold(0.0, f64::max);
            if hi > 0.0 && lo <= hi {
                limit(&mut decls[at(RadioSetting::Rate)], lo, hi, 1.0);
            }
        }
        common_in_range(
            &mut decls[at(RadioSetting::Rate)],
            &rate_list(&caps.common_rates_hz, &caps.rates),
        );
        limit_filter(&mut decls[at(RadioSetting::Bandwidth)], caps.bandwidth_hz);
        width_list(
            &mut decls[at(RadioSetting::Bandwidth)],
            &caps.common_bandwidths_hz,
        );
        match caps.gain_db.filter(|(min, max, _)| max > min) {
            Some((min, max, step)) => {
                limit(&mut decls[at(RadioSetting::Gain)], min, max, step);
                common_in_range(&mut decls[at(RadioSetting::Gain)], &caps.gain_steps_db);
            }
            None => absent(&mut decls[at(RadioSetting::Gain)], Value::F64(0.0)),
        }
        if !caps.has_agc {
            absent(&mut decls[at(RadioSetting::Agc)], Value::Bool(false));
        }
        if !caps.has_bias_tee {
            absent(&mut decls[at(RadioSetting::BiasTee)], Value::Bool(false));
        }
        decls.extend(self.source.settings());
        self.tx = self.source.tx_schema().map(|t| {
            let first = decls.len() as u16;
            decls.extend_from_slice(t.settings());
            (t, first)
        });
        let schema = Arc::new(Schema::new(decls).map_err(DriverError::Schema)?);
        self.values = copy_values(&schema, &self.values)?;
        self.schema = schema;
        Ok(())
    }
}

impl Owner for OpenRadio {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    /// A transmit setting is kept and reaches the transmitter through its owner ([`OpenRadio::tx_seq`]). Every other
    /// setting goes to the device. A new value of a setting declared `restart` that the device takes requests a restart of
    /// the stream ([`RESTART`] at the next poll).
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        if self.tx.as_ref().is_some_and(|(_, first)| id.0 >= *first) {
            self.values.set(id, value)?;
            self.tx_seq += 1;
            self.publish_tx_output();
            return Ok(());
        }
        let mut next = self.values.clone();
        next.set(id, value)?;
        let restart = self.schema.setting(id).is_some_and(|d| d.restart)
            && self.values.value(id) != next.value(id);
        match self.source.apply(&next) {
            Ok(r) => {
                self.values = next;
                self.restart |= restart;
                // A problem names a state before this change: the device took the change, so it is gone.
                self.clear_problem();
                match r {
                    Retuned::Live(s) => {
                        self.results.set(&RATE_GIVEN, s.rate_hz);
                        self.results.set(&CENTRE_GIVEN, s.centre_hz);
                    }
                    Retuned::NeedsRestart => self.restart = true,
                }
                Ok(())
            }
            Err(DriverError::Setting(e)) => Err(e),
            Err(e) => {
                self.problem(&e);
                Err(SettingError::NotPermitted { id })
            }
        }
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
    fn problem(&mut self, text: &str) {
        self.results.set(&PROBLEM, text);
        self.results.results_mut().problem(text);
    }
    fn commands(&self) -> &Commands {
        &self.commands
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let first = RadioCommand::COMMANDS.len() as u16;
        let r = match RadioCommand::from_id(id) {
            Some(RadioCommand::Reconnect) => self.source.reconnect().map(|()| None),
            None => {
                let own =
                    id.0.checked_sub(first)
                        .filter(|i| usize::from(*i) < self.kind.commands.len())
                        .ok_or(CommandError::NoSuchId { id })?;
                self.source.invoke(CommandId(own), args)
            }
        };
        r.map_err(|e| {
            self.problem(&e);
            CommandError::Failed {
                source: sdrkit_core::StageError::new(e),
            }
        })
    }
}

/// The values of `from` as values of `schema`, setting by setting. A fixed setting keeps its default. A number outside
/// the limits of `schema` moves to the nearest limit.
fn copy_values(schema: &Arc<Schema>, from: &Values) -> Result<Values, DriverError> {
    let mut values = Values::defaults(schema);
    for (i, decl) in schema.settings().iter().enumerate() {
        let id = SettingId(i as u16);
        let held = from.value(id).filter(|_| {
            from.schema()
                .setting(id)
                .is_some_and(|d| d.field.key == decl.field.key)
        });
        if let (Some(v), false) = (held, decl.fixed) {
            match values.set(id, v) {
                // A value from before the limits were known moves to the nearest limit.
                Err(SettingError::OutOfRange { min, max, got, .. }) => {
                    let near = got.clamp(min, max);
                    let moved = match v {
                        Value::Hz(_) => Value::Hz(near),
                        Value::I64(_) => Value::I64(near as i64),
                        _ => Value::F64(near),
                    };
                    values.set(id, &moved)?;
                }
                r => r?,
            }
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::SynthDriver;
    use crate::testing::kind;
    use crate::Polled;
    use sdrkit_core::owner;

    fn synth() -> DriverKind {
        kind(SynthDriver::new(Arc::new(|_: &str, _: &Values, _| None)))
    }

    #[test]
    fn an_open_radio_is_an_owner() {
        let k = synth();
        assert_eq!(k.schema().id("centre_hz"), Ok(RadioSetting::Centre.id()));
        assert!(k.schema().id("pace").is_ok() && k.schema().id("gen_seed").is_ok());
        assert_eq!(
            k.decl().commands().id("reconnect"),
            Ok(RadioCommand::Reconnect.id())
        );
        let mut v = Values::defaults(k.schema());
        v.set(RadioSetting::Rate.id(), &Value::Hz(1e6)).unwrap();
        let mut radio = k.open("", &v).unwrap();
        assert_eq!(radio.sample_type(), SampleType::Cf32);
        let (w, _r) = crate::ring(1 << 16, SampleType::Cs8);
        assert!(matches!(
            radio.start(w),
            Err(DriverError::WrongSampleType { .. })
        ));
        let (w, _r) = crate::ring(1 << 16, SampleType::Cf32);
        assert_eq!(radio.start(w).unwrap().rate_hz, 1e6);
        let mut events = Vec::new();
        radio.poll(10.0, &mut events);
        assert!(events.iter().any(|e| e.is(&CONNECTED)), "{events:?}");
        assert_eq!(radio.results().fields().get(&STATE), Some(Link::Up));
        assert_eq!(radio.results().fields().get(&RATE_GIVEN), Some(1e6));
        // The schema refuses a value before the radio sees it.
        let f = RadioSetting::Centre.id();
        assert!(matches!(
            owner::apply(&mut radio, f, &Value::Hz(-5.0)),
            Err(SettingError::OutOfRange { .. })
        ));
        owner::apply(&mut radio, f, &Value::Hz(433e6)).unwrap();
        assert_eq!(radio.values().unwrap().get(&CENTRE), 433e6);
        assert_eq!(radio.results().fields().get(&CENTRE_GIVEN), Some(433e6));
        // The test signal has no connection to make again, and no command of its own.
        let reconnect = RadioCommand::Reconnect.id();
        let none = Record::new(
            radio
                .commands()
                .def(reconnect)
                .unwrap()
                .args
                .record_decl()
                .clone(),
        );
        assert!(matches!(
            owner::invoke(&mut radio, reconnect, &none),
            Err(CommandError::Failed { .. })
        ));
        assert!(matches!(
            radio.invoke(CommandId(5), &Values::defaults(&Arc::default())),
            Err(CommandError::NoSuchId { .. })
        ));
        radio.stop();
    }

    #[test]
    fn a_descriptor_declares_its_tables_and_rasters() {
        const NAME: Field<field::Text> = Field::new("name", "Name", "");
        static LIST: TableDef = TableDef {
            key: "list",
            label: "List",
            columns: &[&NAME],
            cap: 4,
            marks: sdrkit_core::RowMarks::NONE,
        };
        static PICTURE: RasterDef = RasterDef {
            key: "picture",
            label: "Picture",
            x_unit: "Hz",
            y_unit: "s",
            unit: "dB",
        };
        static D: DriverDescriptor = DriverDescriptor {
            key: "test",
            name: "Test",
            opens: Opens::Many,
            addresses: &[],
            address: &[],
            settings: &[],
            commands: &[],
            readings: &[],
            produces: &[],
            accepts: &[],
            tables: &[&LIST],
            rasters: &[&PICTURE],
            series: &[],
            points: "",
            kind_commands: &[],
            kind_tables: &[],
        };
        let decl = D.decl();
        assert!(decl.results().table_id("list").is_some());
        let r = decl.results().raster_id("picture").unwrap();
        assert_eq!(decl.results().rasters()[usize::from(r.0)].unit, "dB");
    }

    /// A radio that holds a frequency of its own, has no AGC, adds one setting when it opens and keeps a state.
    struct Held {
        caps: Capabilities,
        told: bool,
        state: Vec<u8>,
        clash: bool,
    }

    impl Source for Held {
        fn capabilities(&self) -> &Capabilities {
            &self.caps
        }
        fn start(&mut self, _: RingWriter) -> Result<Started, DriverError> {
            Err(DriverError::Thread)
        }
        fn apply(&mut self, _: &Values) -> Result<Retuned, DriverError> {
            Ok(Retuned::NeedsRestart)
        }
        fn stop(&mut self) {}
        fn sample_type(&self) -> SampleType {
            SampleType::Cs16
        }
        fn link(&self) -> Link {
            Link::Idle
        }
        fn poll(&mut self, _: &mut Results) -> Polled {
            Polled {
                store: (!std::mem::replace(&mut self.told, true))
                    .then(|| (RadioSetting::Centre.id(), Value::Hz(7.1e6))),
                ..Polled::default()
            }
        }
        fn settings(&self) -> Vec<SettingDecl> {
            const LNA: Setting<field::F64> =
                Setting::new("lna_db", "LNA", "dB", 0.0).range(0.0, 30.0, 1.0);
            if self.clash {
                vec![GAIN.decl()]
            } else {
                vec![LNA.decl()]
            }
        }
        fn tx_schema(&self) -> Option<Arc<Schema>> {
            Some(tx_schema(&[], self.caps.transmit.as_ref()?))
        }
        fn save_state(&self) -> Option<StateBlob> {
            Some(StateBlob::new("held", 1, self.state.clone()))
        }
        fn restore_state(&mut self, s: &StateBlob) -> Result<(), StateError> {
            self.state = s.open("held", 1)?.to_vec();
            Ok(())
        }
    }

    const ADDRESS: Field<field::Text> = Field::new("address", "Address", "");
    static FOUND: TableDef = TableDef {
        key: "found",
        label: "Found",
        columns: &[&ADDRESS],
        cap: 8,
        marks: sdrkit_core::RowMarks::NONE,
    };
    const DISCOVER: Command = Command::new("discover", "Discover");
    static SPECTRUM: SeriesDef = SeriesDef {
        key: "spectrum",
        label: "Spectrum",
        x_unit: "Hz",
        y_unit: "dB",
    };
    static HELD: DriverDescriptor = DriverDescriptor {
        key: "held",
        name: "Held",
        opens: Opens::Many,
        addresses: &[],
        address: &[],
        settings: &[],
        commands: &[],
        readings: &[],
        produces: &[StreamFormat {
            kind: Kind::Real,
            sample: SampleType::I16,
            min_rate_hz: 12e3,
            max_rate_hz: 12e3,
            channels: 2,
        }],
        accepts: &[],
        tables: &[],
        rasters: &[],
        series: &[&SPECTRUM],
        points: "Receiver",
        kind_commands: &[&DISCOVER],
        kind_tables: &[&FOUND],
    };

    /// The kind of [`Held`]: whether its own setting clashes with a setting of the kind, the limits of its filter and
    /// the limits of its transmitter.
    #[derive(Default)]
    struct HeldDriver {
        clash: bool,
        filter: Option<(Hz, Hz)>,
        transmit: Option<TxLimits>,
    }

    impl crate::SourceDriver for HeldDriver {
        fn descriptor(&self) -> &'static DriverDescriptor {
            &HELD
        }
        fn open(&self, _: &str, _: &Values) -> Result<Box<dyn Source>, DriverError> {
            Ok(Box::new(Held {
                caps: Capabilities {
                    min_freq_hz: 1e6,
                    max_freq_hz: 30e6,
                    bandwidth_hz: self.filter,
                    transmit: self.transmit.clone(),
                    ..Capabilities::default()
                },
                told: false,
                state: Vec::new(),
                clash: self.clash,
            }))
        }
        fn invoke_kind(
            &self,
            id: CommandId,
            _: &Values,
            results: &mut Published,
        ) -> Result<Option<Record>, DriverError> {
            assert_eq!(id, CommandId(0));
            if let Some(mut row) = results.row_mut(&FOUND, "1") {
                row.set(&ADDRESS, "10.0.0.2");
            }
            Ok(None)
        }
    }

    #[test]
    fn an_open_takes_the_frequency_the_radio_holds_and_its_own_settings() {
        let k = DriverKind::new(Box::new(HeldDriver::default()));
        // A tune of 0 is outside the radio's range, but the radio holds 7.1 MHz.
        let mut v = Values::defaults(k.schema());
        v.set(RadioSetting::Centre.id(), &Value::Hz(0.0)).unwrap();
        let mut radio = k.open("", &v).unwrap();
        assert_eq!(radio.values().unwrap().get(&CENTRE), 7.1e6);
        // No automatic gain: AGC is fixed off.
        let agc = radio.schema().setting(RadioSetting::Agc.id()).unwrap();
        assert!(agc.fixed);
        assert!(!radio.values().unwrap().get(&AGC));
        // The setting that the radio adds follows the settings of the kind.
        let lna = HELD.run_time_id(0);
        assert_eq!(radio.schema().id("lna_db"), Ok(lna));
        assert!(matches!(
            owner::apply(&mut radio, lna, &Value::F64(40.0)),
            Err(SettingError::OutOfRange { .. })
        ));
        owner::apply(&mut radio, lna, &Value::F64(20.0)).unwrap();
        // Saved state goes to the source and back.
        radio
            .restore_state(&StateBlob::new("held", 1, vec![1, 2]))
            .unwrap();
        assert_eq!(radio.save_state().unwrap().open("held", 1).unwrap(), [1, 2]);
        // The descriptor declares a series and map points.
        let r = radio.results_decl();
        assert!(r.series_id("spectrum").is_some());
        assert_eq!(HELD.produces[0].channels, 2);
        // A setting that the radio adds with a key that the kind has refuses the open.
        let clash = DriverKind::new(Box::new(HeldDriver {
            clash: true,
            ..HeldDriver::default()
        }));
        assert!(matches!(clash.open("", &v), Err(DriverError::Schema(_))));
    }

    #[test]
    fn an_open_radio_limits_the_filter_width_to_what_the_radio_has() {
        let k = DriverKind::new(Box::new(HeldDriver {
            filter: Some((200e3, 8e6)),
            ..HeldDriver::default()
        }));
        let id = RadioSetting::Bandwidth.id();
        let mut v = Values::defaults(k.schema());
        let mut radio = k.open("", &v).unwrap();
        let decl = radio.schema().setting(id).unwrap();
        assert_eq!(decl.range.map(|r| (r.min, r.max)), Some((200e3, 8e6)));
        assert!(!decl.fixed);
        // 0 lies outside the limits and stays valid: the width follows the sample rate.
        assert_eq!(radio.values().unwrap().get(&BANDWIDTH), 0.0);
        owner::apply(&mut radio, id, &Value::Hz(1e6)).unwrap();
        assert_eq!(radio.values().unwrap().get(&BANDWIDTH), 1e6);
        owner::apply(&mut radio, id, &Value::Hz(0.0)).unwrap();
        assert_eq!(radio.values().unwrap().get(&BANDWIDTH), 0.0);
        for outside in [100e3, 9e6] {
            assert!(matches!(
                owner::apply(&mut radio, id, &Value::Hz(outside)),
                Err(SettingError::OutOfRange { .. })
            ));
        }
        // A stored width outside the limits moves to the nearest limit at open; 0 and a width inside them stay.
        for (stored, held) in [(20e6, 8e6), (50e3, 200e3), (1e6, 1e6), (0.0, 0.0)] {
            v.set(id, &Value::Hz(stored)).unwrap();
            let radio = k.open("", &v).unwrap();
            assert_eq!(radio.values().unwrap().get(&BANDWIDTH), held, "{stored}");
        }
    }

    #[test]
    fn a_filter_that_cannot_be_set_is_a_fixed_setting() {
        let k = DriverKind::new(Box::new(HeldDriver::default()));
        let id = RadioSetting::Bandwidth.id();
        let mut v = Values::defaults(k.schema());
        v.set(id, &Value::Hz(5e6)).unwrap();
        let mut radio = k.open("", &v).unwrap();
        assert!(radio.schema().setting(id).unwrap().fixed);
        assert_eq!(radio.values().unwrap().get(&BANDWIDTH), 0.0);
        assert!(matches!(
            owner::apply(&mut radio, id, &Value::Hz(1e6)),
            Err(SettingError::Fixed { .. })
        ));
    }

    #[test]
    fn an_open_radio_declares_its_transmit_limits_before_a_transmitter_opens() {
        let limits = TxLimits {
            min_freq_hz: 70e6,
            max_freq_hz: 6e9,
            rates: RateLimits {
                min_hz: 0.5e6,
                max_hz: 20e6,
                ranges: vec![(0.5e6, 20e6)],
                steady_hz: None,
            },
            bandwidth_hz: None,
            power_db: (-60.0, 0.0, 0.5),
            start_power_db: Some(-10.0),
            common_rates_hz: vec![0.25e6, 1e6, 2e6, 30e6],
            full_output_dbm: Vec::new(),
        };
        let k = DriverKind::new(Box::new(HeldDriver {
            transmit: Some(limits.clone()),
            ..HeldDriver::default()
        }));
        let mut radio = k.open("", &Values::defaults(k.schema())).unwrap();
        assert_eq!(radio.capabilities().transmit.as_ref(), Some(&limits));
        let id = |key: &str| radio.schema().id(key).unwrap();
        let (f, rate, width, power) = (
            id("tx_frequency"),
            id("tx_rate"),
            id("tx_bandwidth"),
            id("tx_power"),
        );
        let range = |id| {
            let decl = radio.schema().setting(id).unwrap();
            decl.range.map(|r| (r.min, r.max))
        };
        assert_eq!(range(f), Some((70e6, 6e9)));
        assert_eq!(range(rate), Some((0.5e6, 20e6)));
        assert_eq!(range(power), Some((-60.0, 0.0)));
        assert!(radio.schema().setting(width).unwrap().fixed);
        // The common rates and powers are those inside the limits.
        assert_eq!(radio.schema().common(rate, None), &[1e6, 2e6]);
        assert_eq!(
            radio.schema().common(power, None),
            &[-30.0, -20.0, -10.0, -6.0, -3.0, 0.0]
        );
        // The defaults lie inside the limits: a low but usable power, and a rate of 0 for the receive rate.
        let tx = radio.tx_values().unwrap();
        assert_eq!(tx.get(&TX_POWER), -10.0);
        assert_eq!(tx.get(&TX_RATE), 0.0);
        // The schema refuses a carrier outside the range of the transmitter.
        assert!(matches!(
            owner::apply(&mut radio, f, &Value::Hz(50e6)),
            Err(SettingError::OutOfRange { .. })
        ));
        owner::apply(&mut radio, f, &Value::Hz(433.92e6)).unwrap();
        assert_eq!(radio.tx_values().unwrap().get(&TX_FREQUENCY), 433.92e6);
    }

    #[test]
    fn a_kind_runs_its_own_commands_into_its_own_tables() {
        let mut k = DriverKind::new(Box::new(HeldDriver::default()));
        let id = k.commands().id("discover").unwrap();
        let none = Record::new(k.commands().def(id).unwrap().args.record_decl().clone());
        owner::invoke(&mut k, id, &none).unwrap();
        let r = k.results();
        let t = r.table(r.decl().table_id("found").unwrap()).unwrap();
        assert_eq!(t.row("1").unwrap().get(&ADDRESS), Some("10.0.0.2"));
        // Id 1 is the `address` command of every kind; the kind has no further command.
        assert_eq!(
            k.invoke(CommandId(2), &Values::defaults(&Arc::default())),
            Err(CommandError::NoSuchId { id: CommandId(2) })
        );
        // The declarations of a radio of the kind stay at hand.
        assert!(k.decl().commands().id("reconnect").is_ok());
    }
}
