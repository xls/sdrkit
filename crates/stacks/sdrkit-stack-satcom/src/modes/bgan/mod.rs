//! Inmarsat BGAN (`bgan`): the broadcast signalling of every forward carrier of a capture of the L-band forward link, and a
//! census of the carriers. Standards: ETSI TS 102 744-2-1 (physical layer, with the tables of its Annex C) and TS 102 744-3-1
//! (Bearer Control layer).
//!
//! The stack reads broadcast signalling only: bulletin boards, broadcast AVP lists, return schedules and the headers of the other
//! broadcast SDUs. PDUs addressed to a terminal and connection PDUs are counted, never read or kept; no user data is decoded.

pub mod codec;
pub mod demod;
pub mod gen;
pub mod pdu;
pub mod rx;
pub mod tables;

use crate::display::set_status;
use sdrkit_util_objects::shaped;
use crate::typed::{
    BganBeamRow, BganCarrierRow, BganFields, BganLogRow, BganSignallingEvent, BGAN_BEAMS,
    BGAN_CARRIERS, BGAN_LOG, BGAN_SIGNALLING,
};
use codec::IqLabels;
use demod::{DemodOptions, DemodState};
use rx::{BganRx, Kind, SatPos};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::value::Choice;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::HostRequest;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind as PortKind, Outbox, Port, RateFrom, Registry, StackGraph, Timed,
};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "bgan";
/// The stack name.
pub const NAME: &str = "Inmarsat BGAN";
/// The least input rate: a 168 kBd carrier needs 2.2 samples per symbol after the channel filter, and the band around it.
pub const MIN_RATE: f64 = 600e3;
/// Seconds between two reports.
pub const REPORT_S: f64 = 0.25;
/// What the stack says about its sources.
pub const NOTE_TEXT: &str = "Synthetic signals only: never checked on a real BGAN carrier. No public document states the bit \
     order of the unique-word hex (setting uw_order) or how the I and Q columns of 38 of the published tables map to the bits \
     of a symbol (setting iq_labels). The thresholds of the stack, the code rate of blocks that no ForwardBearerCodeRate \
     names (taken as block 0's), the layouts of the ReturnSchedule flags and of the SpotBeamMap header, and the satellite \
     names are not confirmed by a public document. F80T1Q-1B has no published tables: frame structure only.";

sdrkit_core::choice_enum! {
    /// The bit order of the unique-word hex.
    pub enum UwOrder {
        /// The first symbol is the most significant bit.
        MsbFirst = ("msb_first", "First symbol is the most significant bit"),
        /// The first symbol is the least significant bit.
        LsbFirst = ("lsb_first", "First symbol is the least significant bit"),
    }
}

sdrkit_core::choice_enum! {
    /// How the columns of the tables labelled with I and Q map to the bits of a symbol.
    pub enum IqLabelChoice {
        /// I1 I0 Q1 Q0 are b3 b2 b1 b0; I0 Q0 are b1 b0.
        InOrder = ("in_order", "In column order"),
        /// I1 I0 Q1 Q0 are b2 b3 b0 b1; I0 Q0 are b0 b1.
        Swapped = ("swapped", "Swapped within each pair"),
    }
}

sdrkit_core::choice_enum! {
    /// The bearer to look for.
    pub enum BearerChoice {
        /// Any.
        Auto = ("auto", "Any bearer"),
        /// F80T0.25Q-1B.
        B1 = ("1", "F80T0.25Q-1B"),
        /// F80T1Q-1B.
        B2 = ("2", "F80T1Q-1B"),
        /// F80T1Q-4B.
        B3 = ("3", "F80T1Q-4B"),
        /// F80T1X-4B.
        B4 = ("4", "F80T1X-4B"),
        /// FR80T2.5X4/16-5B.
        B5 = ("5", "FR80T2.5X4/16-5B"),
        /// FR80T2.5X16-5B.
        B6 = ("6", "FR80T2.5X16-5B"),
        /// FR80T2.5X32-6B.
        B7 = ("7", "FR80T2.5X32-6B"),
        /// FR80T2.5X64-7B.
        B8 = ("8", "FR80T2.5X64-7B"),
        /// F80T4.5X-8B.
        B9 = ("9", "F80T4.5X-8B"),
        /// FR80T5X4/16-9B.
        B10 = ("10", "FR80T5X4/16-9B"),
        /// FR80T5X16-9B.
        B11 = ("11", "FR80T5X16-9B"),
        /// FR80T5X32-11B.
        B12 = ("12", "FR80T5X32-11B"),
        /// FR80T5X64-13B.
        B13 = ("13", "FR80T5X64-13B"),
    }
}

/// Decode every FEC block (else block 0 only).
pub const DECODE_ALL: Setting<field::Bool> =
    Setting::new("decode_all", "Decode every FEC block", "", true)
        .help("Off: block 0 of each frame only.");
/// The bearer to look for.
pub const BEARER: Setting<field::Choice<BearerChoice>> =
    Setting::choice("bearer", "Bearer", BearerChoice::Auto);
/// The bit order of the unique words.
pub const UW_ORDER: Setting<field::Choice<UwOrder>> =
    Setting::choice("uw_order", "Unique word bit order", UwOrder::MsbFirst)
        .advanced()
        .help("No public document states which end of the printed hex is sent first.");
/// The labelling of the I and Q columns.
pub const IQ_LABELS: Setting<field::Choice<IqLabelChoice>> =
    Setting::choice("iq_labels", "I and Q columns", IqLabelChoice::InOrder)
        .advanced()
        .help("How the columns of the 38 tables labelled with I and Q map to the bits of a symbol; no public document states it.");
/// Satellite names by orbital longitude.
pub const SATELLITES: Setting<field::Text> = Setting::new("satellites", "Satellite names", "", "").help(
    "One a line (or separated by ;): longitude in degrees east, name. A SatelliteLocation within 1.5 degrees takes the name; \
     without one the longitude is shown.",
);

/// Asks the host to walk the band.
pub const SCAN: Setting<field::Bool> = Setting::new("scan", "Scan the band", "", false).help(
    "Asks the host to visit the band from scan_from_hz to scan_to_hz in steps of the sample rate, 7 s on each. The carrier \
     rows stay with their last state.",
);
/// The lower end of the scan.
pub const SCAN_FROM: Setting<field::Hz> = Setting::new("scan_from_hz", "Scan from", "Hz", 1518e6)
    .range(1518e6, 1559e6, 1e3)
    .advanced();
/// The upper end of the scan.
pub const SCAN_TO: Setting<field::Hz> = Setting::new("scan_to_hz", "Scan to", "Hz", 1559e6)
    .range(1518e6, 1559e6, 1e3)
    .advanced();
/// Seconds on each step of a scan.
pub const SCAN_DWELL_S: f64 = 7.0;

sdrkit_core::settings! {
    /// The settings of the stack.
    pub enum BganSetting {
        /// Decode every block.
        DecodeAll = DECODE_ALL,
        /// The bearer.
        Bearer = BEARER,
        /// Unique word bit order.
        UwOrder = UW_ORDER,
        /// I and Q columns.
        IqLabels = IQ_LABELS,
        /// Satellite names.
        Satellites = SATELLITES,
        /// Scan the band.
        Scan = SCAN,
        /// Scan from.
        ScanFrom = SCAN_FROM,
        /// Scan to.
        ScanTo = SCAN_TO,
        /// Test signal bearers.
        GenBearer = gen::GEN_BEARER,
        /// Test signal subtype.
        GenSubtype = gen::GEN_SUBTYPE,
        /// Test signal C/N0.
        GenCarrierCn0 = gen::GEN_CARRIER_CN0,
        /// Test signal offset.
        GenOffsetHz = gen::GEN_OFFSET_HZ,
        /// Test signal clock error.
        GenClockPpm = gen::GEN_CLOCK_PPM,
        /// Test signal drift.
        GenDrift = gen::GEN_DRIFT,
        /// Test signal fading rate.
        GenFadeHz = gen::GEN_FADE_HZ,
        /// Test signal fading depth.
        GenFadeDepth = gen::GEN_FADE_DEPTH,
        /// Test signal spot beam.
        GenSpot = gen::GEN_SPOT,
    }
}

sdrkit_core::settings! {
    /// The settings of [`BganComponent`].
    pub enum BganRxSetting {
        /// Decode every block.
        DecodeAll = DECODE_ALL,
        /// The bearer.
        Bearer = BEARER,
        /// Unique word bit order.
        UwOrder = UW_ORDER,
        /// I and Q columns.
        IqLabels = IQ_LABELS,
        /// Satellite names.
        Satellites = SATELLITES,
        /// Scan the band.
        Scan = SCAN,
        /// Scan from.
        ScanFrom = SCAN_FROM,
        /// Scan to.
        ScanTo = SCAN_TO,
    }
}

fn decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(BganFields::SHAPE.fields)
                .with_table(&BGAN_CARRIERS)
                .with_table(&BGAN_BEAMS)
                .with_table(&BGAN_LOG),
        )
    })
    .clone()
}

/// A satellite name list: (degrees east, name).
pub fn parse_satellites(text: &str) -> Vec<(f64, String)> {
    text.split(['\n', ';'])
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let (lon, name) = l.split_once(',')?;
            Some((
                lon.trim().parse().ok()?,
                name.trim().trim_matches('"').to_string(),
            ))
        })
        .collect()
}

/// The text of a satellite: its name within 1.5 degrees, else "satellite N at L".
pub fn satellite_text(p: &SatPos, names: &[(f64, String)]) -> String {
    let near = names
        .iter()
        .filter(|(l, _)| (l - p.lon_deg).abs() <= 1.5)
        .min_by(|a, b| (a.0 - p.lon_deg).abs().total_cmp(&(b.0 - p.lon_deg).abs()));
    match near {
        Some((_, n)) => format!("{n} ({})", rx::lon_text(p.lon_deg)),
        None => format!("satellite {} at {}", p.id, rx::lon_text(p.lon_deg)),
    }
}

/// The receiver and display in one component (`satcom.bgan_receiver`).
pub struct BganComponent {
    rx: Option<BganRx>,
    decode_all: bool,
    bearer: BearerChoice,
    uw_order: UwOrder,
    iq_labels: IqLabelChoice,
    satellites: String,
    names: Vec<(f64, String)>,
    scan: bool,
    scan_from: f64,
    scan_to: f64,
    /// A scan request waits for the next samples.
    scan_pending: bool,
    rate: f64,
    tuned_hz: f64,
    level: f64,
    summary: String,
}

impl Default for BganComponent {
    fn default() -> Self {
        BganComponent {
            rx: None,
            decode_all: true,
            bearer: BearerChoice::Auto,
            uw_order: UwOrder::MsbFirst,
            iq_labels: IqLabelChoice::InOrder,
            satellites: String::new(),
            names: Vec::new(),
            scan: false,
            scan_from: 1518e6,
            scan_to: 1559e6,
            scan_pending: false,
            rate: 0.0,
            tuned_hz: 0.0,
            level: 0.0,
            summary: String::new(),
        }
    }
}

impl BganComponent {
    fn options(&self) -> DemodOptions {
        let forced = match self.bearer {
            BearerChoice::Auto => None,
            b => tables::BEARERS.get(Choice::index(b) as usize - 1),
        };
        DemodOptions {
            decode_all: self.decode_all,
            uw_msb_first: self.uw_order == UwOrder::MsbFirst,
            labels: match self.iq_labels {
                IqLabelChoice::InOrder => IqLabels::InOrder,
                IqLabelChoice::Swapped => IqLabels::Swapped,
            },
            forced,
        }
    }

    /// The scan request: the steps of the sample rate over the band.
    fn scan_request(&self) -> Option<HostRequest> {
        if !self.scan || self.rate <= 0.0 || self.scan_to <= self.scan_from {
            return None;
        }
        let mut channels = Vec::new();
        let mut f = self.scan_from + self.rate / 2.0;
        while f - self.rate / 2.0 < self.scan_to {
            channels.push(f);
            f += self.rate;
        }
        Some(HostRequest::Hop {
            channels,
            dwell_s: SCAN_DWELL_S,
            stop_on_lock: false,
        })
    }

    fn state_text(s: DemodState) -> &'static str {
        match s {
            DemodState::Searching => "searching",
            DemodState::Sync => "frame sync",
            DemodState::Decoded => "signalling decoded",
        }
    }
}

impl Decoder for BganComponent {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.bgan_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::new("events", PortKind::Messages).rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        BganRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (BganRxSetting::DecodeAll, Value::Bool(self.decode_all)),
            (
                BganRxSetting::Bearer,
                Value::Choice(Choice::index(self.bearer)),
            ),
            (
                BganRxSetting::UwOrder,
                Value::Choice(Choice::index(self.uw_order)),
            ),
            (
                BganRxSetting::IqLabels,
                Value::Choice(Choice::index(self.iq_labels)),
            ),
            (
                BganRxSetting::Satellites,
                Value::Text(self.satellites.as_str().into()),
            ),
            (BganRxSetting::Scan, Value::Bool(self.scan)),
            (BganRxSetting::ScanFrom, Value::Hz(self.scan_from)),
            (BganRxSetting::ScanTo, Value::Hz(self.scan_to)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match BganRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BganRxSetting::DecodeAll => self.decode_all = DECODE_ALL.read(id, v)?,
            BganRxSetting::Bearer => self.bearer = BEARER.read(id, v)?,
            BganRxSetting::UwOrder => self.uw_order = UW_ORDER.read(id, v)?,
            BganRxSetting::IqLabels => self.iq_labels = IQ_LABELS.read(id, v)?,
            BganRxSetting::Satellites => {
                self.satellites = SATELLITES.read(id, v)?.to_string();
                self.names = parse_satellites(&self.satellites);
                return Ok(());
            }
            BganRxSetting::Scan => {
                self.scan = SCAN.read(id, v)?;
                self.scan_pending = self.scan;
                return Ok(());
            }
            BganRxSetting::ScanFrom => {
                self.scan_from = SCAN_FROM.read(id, v)?;
                self.scan_pending = self.scan;
                return Ok(());
            }
            BganRxSetting::ScanTo => {
                self.scan_to = SCAN_TO.read(id, v)?;
                self.scan_pending = self.scan;
                return Ok(());
            }
        }
        let o = self.options();
        if let Some(r) = self.rx.as_mut() {
            r.set_options(o);
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.tuned_hz = f.tuned_hz;
        let dial = if f.tuned_hz > 0.0 { f.centre_hz } else { 0.0 };
        self.rx = Some(BganRx::new(f.rate, dial, self.options()));
        Ok(vec![
            Format::new(PortKind::Messages, 0.0),
            Format::new(PortKind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn retune(&mut self, f: &Format) {
        self.tuned_hz = f.tuned_hz;
        if let Some(r) = self.rx.as_mut() {
            r.retune(if f.tuned_hz > 0.0 { f.centre_hz } else { 0.0 });
        }
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let Some(r) = self.rx.as_mut() else {
            return;
        };
        let p = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len().max(1) as f64;
        self.level = 0.9 * self.level + 0.1 * p;
        r.process(x);
        let t = r.now();
        for line in r.take_fresh() {
            out.messages(0).push(BGAN_SIGNALLING.event(
                t,
                self.tuned_hz,
                &BganSignallingEvent {
                    text: line.as_str(),
                },
            ));
        }
        if self.scan_pending {
            self.scan_pending = false;
            if let Some(value) = self.scan_request() {
                out.items::<HostRequest>(1).push(Timed { time: t, value });
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let Some(r) = self.rx.as_ref() else {
            return;
        };
        let now = r.now();
        let in_sync = r
            .carriers
            .iter()
            .filter(|c| c.state >= DemodState::Sync)
            .count();
        let decoded = r
            .carriers
            .iter()
            .filter(|c| c.state == DemodState::Decoded)
            .count();
        let ok: u64 = r.carriers.iter().map(|c| c.blocks_ok).sum();
        let bad: u64 = r.carriers.iter().map(|c| c.blocks_bad).sum();
        let networks: Vec<&str> = r.networks.iter().map(String::as_str).collect();
        let sats: Vec<String> = r
            .satellites
            .iter()
            .map(|s| satellite_text(s, &self.names))
            .collect();
        p.put(&BganFields {
            carriers: r.carriers.len() as i64,
            in_sync: in_sync as i64,
            decoded: decoded as i64,
            gmr2: r.count(Kind::Gmr2) as i64,
            networks: networks.join(", ").as_str(),
            satellites_heard: sats.join(", ").as_str(),
            blocks_ok: ok as i64,
            blocks_bad: bad as i64,
            connection_pdus: r.carriers.iter().map(|c| c.connection_pdus).sum::<u64>() as i64,
            addressed_pdus: r.carriers.iter().map(|c| c.addressed_pdus).sum::<u64>() as i64,
            level_dbfs: 10.0 * self.level.max(1e-20).log10(),
            note: NOTE_TEXT,
        });
        let names = &self.names;
        shaped::indexed_rows(p, &BGAN_CARRIERS, &r.carriers, |c, out| {
            let sat = c.satellite.as_ref().map(|s| satellite_text(s, names));
            out.put(&BganCarrierRow {
                    freq_hz: (r.tuned_hz > 0.0).then_some(Hertz(c.freq_hz)),
                    offset_hz: Hertz(c.offset_hz),
                    width_hz: Hertz(c.width_hz),
                    snr_db: c.snr_db,
                    kind: c.kind.text(),
                    state: Self::state_text(c.state),
                    bearer: (!c.bearer.is_empty()).then_some(c.bearer.as_str()),
                    subtype: (!c.subtype.is_empty()).then_some(c.subtype.as_str()),
                    symbol_rate_hz: (c.kind == Kind::Bgan).then_some(Hertz(c.symbol_rate)),
                    cfo_hz: (c.frames > 0).then_some(Hertz(c.cfo_hz)),
                    cn0_dbhz: (c.frames > 0).then_some(c.cn0_dbhz),
                    frames: c.frames as i64,
                    blocks_ok: c.blocks_ok as i64,
                    blocks_bad: c.blocks_bad as i64,
                    load_pct: 100.0 * c.load(),
                    structure_only: c.structure_only,
                    spot_beam: c.spot.map(i64::from),
                    rnc: c.rnc.map(i64::from),
                    bct_id: c.bct_id.map(i64::from),
                    f_bearer: c.f_bearer.map(i64::from),
                    beam_type: c.beam_type.map(pdu::beam_type_name),
                    network: (!c.network.is_empty()).then_some(c.network.as_str()),
                    satellite: sat.as_deref(),
                    last_seen: Seconds((now - c.last_seen).max(0.0)),
                },
            );
        });
        let beams: Vec<(u8, rx::Beam)> = r.beams.iter().map(|(k, v)| (*k, v.clone())).collect();
        shaped::indexed_rows(p, &BGAN_BEAMS, &beams, |(spot, b), out| {
            out.put(&BganBeamRow {
                    spot_beam: i64::from(*spot),
                    beam_type: b.beam_type.map(pdu::beam_type_name),
                    primary_hz: b.primary_hz.map(Hertz),
                    bearer_type: b.bearer_type.map(tables::fbearer_type_name),
                },
            );
        });
        let log: Vec<&(f64, String)> = r.log.iter().rev().collect();
        shaped::indexed_rows(p, &BGAN_LOG, &log, |(at, text), out| {
            out.put(&BganLogRow {
                    at: Seconds(*at),
                    text: text.as_str(),
                },
            );
        });
        let summary = &mut self.summary;
        summary.clear();
        let _ = write!(
            summary,
            "broadcast signalling only: {} carriers, {} in sync, {} decoded",
            r.carriers.len(),
            in_sync,
            decoded
        );
        if !networks.is_empty() {
            let _ = write!(summary, ", network {}", networks.join(", "));
        }
        if !sats.is_empty() {
            let _ = write!(summary, ", {}", sats.join(", "));
        }
        let g = r.count(Kind::Gmr2);
        if g > 0 {
            let _ = write!(summary, ", {g} GMR-2 class");
        }
        let lock = if decoded > 0 {
            LockState::Locked
        } else if in_sync > 0 || !r.carriers.is_empty() {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let best = r
            .carriers
            .iter()
            .filter(|c| c.frames > 0)
            .map(|c| c.cn0_dbhz - 10.0 * c.symbol_rate.max(1.0).log10())
            .fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.max(x))));
        set_status(
            p.results_mut(),
            lock,
            None,
            best.map(|x| x as f32),
            None,
            summary,
        );
    }
    fn clear(&mut self) {
        if let Some(r) = self.rx.as_mut() {
            r.log.clear();
            for c in &mut r.carriers {
                c.blocks_ok = 0;
                c.blocks_bad = 0;
                c.blocks_used = 0;
                c.frames = 0;
            }
        }
    }
}

/// Adds the components of this stack to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.bgan_receiver", || {
        Box::new(DecoderComponent::new(BganComponent::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("rx", DecoderComponent::new(BganComponent::default()))
        .input("rx")
        .output("rx.events")
        .output("rx.requests")
        .status_from("rx")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "BGAN needs a sample rate of at least 0.6 Msps")
        .route("decode_all", &["rx.decode_all"])?
        .route("bearer", &["rx.bearer"])?
        .route("uw_order", &["rx.uw_order"])?
        .route("iq_labels", &["rx.iq_labels"])?
        .route("satellites", &["rx.satellites"])?
        .route("scan", &["rx.scan"])?
        .route("scan_from_hz", &["rx.scan_from_hz"])?
        .route("scan_to_hz", &["rx.scan_to_hz"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::BganGenerator::from_config(cfg)))
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Inmarsat BGAN forward carriers of the L band: every carrier found and classed, frames and FEC blocks decoded with the published tables, and the broadcast signalling read (bulletin boards, networks, satellites, spot beams, return schedules). Broadcast signalling only: no user data. Synthetic signals only. Receive only.",
    tuning: Tuning {
        min_hz: 1518e6,
        max_hz: 1559e6,
        default_hz: 1545e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 1.6e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(2e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: BganSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

/// The verification of the stack: the group test signal decodes.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, a group of carriers", o, 0.0);
    // The generator sets the C/N0 of each carrier and adds its own noise.
    c.snr_db = None;
    c.offset_hz = 0.0;
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(check_group);
    vec![c]
}

fn check_group(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let decoded = crate::field(r, BganFields::DECODED.key);
    let ok = crate::field(r, BganFields::BLOCKS_OK.key);
    let gmr = crate::field(r, BganFields::GMR2.key);
    vec![
        CheckLine::new(
            "decoded",
            decoded >= 5.0,
            format!("{decoded} carriers decoded"),
        ),
        CheckLine::new("blocks", ok >= 40.0, format!("{ok} blocks good")),
        CheckLine::new("gmr2", gmr >= 1.0, format!("{gmr} GMR-2 class carriers")),
    ]
}

/// The benchmark of the receiver on the group at 2 Msps.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "bgan.receiver_2m",
        run: |s| crate::bench_receiver(s, &DESCRIPTOR, None, 4.0),
    }]
}
