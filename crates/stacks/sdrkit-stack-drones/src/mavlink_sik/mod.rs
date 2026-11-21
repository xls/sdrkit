//! MAVLink on SiK radios (`mavlink_sik`): the bursts of telemetry radios anywhere in the capture at every SiK air rate that the
//! sample rate allows, the SiK packets with and without Golay error correction, the MAVLink v1 and v2 frames in them, and the
//! position of each vehicle. Hops are not followed.
//! Standard: MAVLink Developer Guide (frames, checksum); the SiK packet layout is not in a public document.

pub mod generator;
pub mod mavlink;
pub mod sik;

use crate::family::{min_rate_message, wall, REPORT_S};
use crate::fsklink::{band_config, bytes_of, Burst, FskFront};
use crate::links::{
    LinkCommand,
    Heard, LinkHeld, NewLink, VehicleSeen,
    RECENT_S,
};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_algo_modem::bandburst::BandDetector;
use sdrkit_algo_modem::fsksync::Polarity;
use crate::typed::{LinkFields, FRAMES, LINKS};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::value::Choice as _;
use sdrkit_core::Record;
use sdrkit_core::command::CommandSet as _;
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState,
    Receiver, ResultsDecl, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kind, Outbox,
    Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::Stamp;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "mavlink_sik";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The link name in the tables.
pub const LINK_NAME: &str = "SiK";
/// The pattern searched: the last two preamble bytes and the sync word.
pub const PATTERN_BYTES: [u8; 4] = [0xAA, 0xAA, 0x2D, 0xD4];
/// The most bytes read after the sync word (an error-corrected packet of 252 data bytes).
pub const MAX_AIR_BYTES: usize = 1 + 12 + 84 * 6;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the SiK packet layout (net id, length, CRC-16/CMS over net id, length and data; the error-corrected form with Golay (23,12) words and the SiK CRC) and the trailer of window, command, bonus and resend bits. Only the 7 MAVLink messages HEARTBEAT, SYS_STATUS, GPS_RAW_INT, ATTITUDE, GLOBAL_POSITION_INT, VFR_HUD and STATUSTEXT are known, so frames of other messages are not accepted. Not tried on a real signal.";

sdrkit_core::choice_enum! {
    /// The air rate searched.
    pub enum AirRate {
        /// Every rate that fits the sample rate.
        Auto = ("auto", "Auto"),
        /// 2 kbit/s.
        R2 = ("2", "2"),
        /// 4 kbit/s.
        R4 = ("4", "4"),
        /// 8 kbit/s.
        R8 = ("8", "8"),
        /// 16 kbit/s.
        R16 = ("16", "16"),
        /// 19 kbit/s.
        R19 = ("19", "19"),
        /// 24 kbit/s.
        R24 = ("24", "24"),
        /// 32 kbit/s.
        R32 = ("32", "32"),
        /// 48 kbit/s.
        R48 = ("48", "48"),
        /// 64 kbit/s.
        R64 = ("64", "64"),
        /// 96 kbit/s.
        R96 = ("96", "96"),
        /// 128 kbit/s.
        R128 = ("128", "128"),
        /// 192 kbit/s.
        R192 = ("192", "192"),
        /// 250 kbit/s.
        R250 = ("250", "250"),
    }
}

impl AirRate {
    /// The rate, bit/s; `None` for auto.
    pub fn bps(self) -> Option<f64> {
        let k = match self {
            AirRate::Auto => return None,
            AirRate::R2 => 2,
            AirRate::R4 => 4,
            AirRate::R8 => 8,
            AirRate::R16 => 16,
            AirRate::R19 => 19,
            AirRate::R24 => 24,
            AirRate::R32 => 32,
            AirRate::R48 => 48,
            AirRate::R64 => 64,
            AirRate::R96 => 96,
            AirRate::R128 => 128,
            AirRate::R192 => 192,
            AirRate::R250 => 250,
        };
        Some(f64::from(k) * 1e3)
    }
}

/// The air rate searched.
pub const AIR_RATE: Setting<field::Choice<AirRate>> = Setting::choice("air_rate", "Air speed", AirRate::Auto)
    .help("The air rate of the radios, kbit/s. Auto searches every rate that the sample rate allows (at least 2.5 samples a bit).");
/// The burst threshold.
pub const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Burst threshold", "dB", 9.0)
        .range(6.0, 20.0, 1.0)
        .help("How far above the noise floor of a frequency bin a burst must rise.");

sdrkit_core::settings! {
    /// The settings of `mavlink_sik`.
    pub enum SikSetting {
        /// The air rate.
        AirRate = AIR_RATE,
        /// The burst threshold.
        ThresholdDb = THRESHOLD_DB,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("mavlink_sik").results_decl()
}

/// One SiK packet decoded from a burst, with where it was heard.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// The packet.
    pub packet: sik::Packet,
    /// The air rate, bit/s.
    pub rate: f64,
    /// The bytes after the sync word that the packet took.
    pub bytes: Vec<u8>,
    /// The frequency, Hz from the input centre.
    pub offset_hz: f64,
    /// The stream index of the first bit after the sync word.
    pub sample: u64,
}

/// The rates to try for a burst of width `width_hz`: `only`, or every air rate, those whose GFSK width (about twice the rate)
/// is nearest the burst's first.
pub fn rates(width_hz: f64, only: Option<f64>, out: &mut Vec<f64>) {
    out.clear();
    match only {
        Some(r) => out.push(r),
        None => {
            out.extend(sik::AIR_RATES);
            let w = width_hz.max(1e3);
            out.sort_by(|a, b| {
                let da = (w / (2.0 * a)).ln().abs();
                let db = (w / (2.0 * b)).ln().abs();
                da.total_cmp(&db)
            });
        }
    }
}

/// The SiK reader of bursts: the front, the pattern, the bit and byte buffers and the Golay code.
pub struct SikReader {
    front: FskFront,
    code: SmallCode,
    pattern: Vec<u8>,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    rates: Vec<f64>,
}

impl SikReader {
    /// A reader for input at `fs`.
    pub fn new(fs: f64) -> SikReader {
        let mut pattern = Vec::with_capacity(32);
        crate::fsklink::bits_of(&PATTERN_BYTES, false, &mut pattern);
        SikReader {
            front: FskFront::new(fs),
            code: sik::golay(),
            pattern,
            bits: Vec::with_capacity(8 * MAX_AIR_BYTES),
            bytes: Vec::with_capacity(MAX_AIR_BYTES),
            rates: Vec::with_capacity(sik::AIR_RATES.len()),
        }
    }

    /// The input rate.
    pub fn rate_hz(&self) -> f64 {
        self.front.rate_hz()
    }

    /// The first SiK packet of burst `b` at rate `only` or at any rate that fits.
    pub fn read(&mut self, b: &Burst, only: Option<f64>) -> Option<Decoded> {
        rates(b.width_hz, only, &mut self.rates);
        for i in 0..self.rates.len() {
            let r = self.rates[i];
            if !self.front.demod(b, r, 0.0, None) {
                continue;
            }
            let Some(hit) = self.front.find(&self.pattern, 1, Polarity::Normal) else {
                continue;
            };
            self.front.bits(&hit, 8 * MAX_AIR_BYTES, &mut self.bits);
            bytes_of(&self.bits, false, &mut self.bytes);
            let p = sik::parse_plain(&self.bytes).or_else(|| sik::parse_ecc(&self.code, &self.bytes));
            if let Some(p) = p {
                let n = p.air_len.min(self.bytes.len());
                return Some(Decoded {
                    rate: r,
                    bytes: self.bytes[..n].to_vec(),
                    offset_hz: b.offset_hz + self.front.cfo_hz(&hit),
                    sample: self.front.sample_of(b, &hit, 0),
                    packet: p,
                });
            }
        }
        None
    }
}

/// The `mavlink_sik.rx` decoder: bursts, SiK packets, MAVLink streams and the tables.
pub struct SikDecoder {
    air_rate: AirRate,
    threshold_db: f64,
    rate: f64,
    centre_hz: f64,
    origin_s: f64,
    fresh: bool,
    det: Option<BandDetector>,
    reader: Option<SikReader>,
    pool: Vec<Burst>,
    used: usize,
    streams: Vec<(u16, mavlink::Stream)>,
    held: LinkHeld,
    snr_db: Option<f64>,
    now: f64,
    last_frame: f64,
    live: bool,
    clock: Option<Clock>,
    removed: Vec<String>,
    events: Vec<Timed<VehicleSeen>>,
    summary: String,
    dirty: bool,
    cleared: bool,
    started: bool,
}

impl Default for SikDecoder {
    fn default() -> Self {
        SikDecoder {
            air_rate: AirRate::Auto,
            threshold_db: THRESHOLD_DB.default,
            rate: 0.0,
            centre_hz: 0.0,
            origin_s: 0.0,
            fresh: true,
            det: None,
            reader: None,
            pool: Vec::new(),
            used: 0,
            streams: Vec::new(),
            held: LinkHeld::default(),
            snr_db: None,
            now: 0.0,
            last_frame: f64::NEG_INFINITY,
            live: false,
            clock: None,
            removed: Vec::new(),
            events: Vec::new(),
            summary: String::with_capacity(160),
            dirty: true,
            cleared: false,
            started: false,
        }
    }
}

impl SikDecoder {
    /// The tables and counters.
    pub fn held(&self) -> &LinkHeld {
        &self.held
    }

    fn build(&mut self) -> Result<(), ConfigError> {
        self.fresh = true;
        self.det = Some(BandDetector::new(band_config(self.rate, self.threshold_db)).map_err(ConfigError::from)?);
        if self.reader.as_ref().is_none_or(|r| r.rate_hz() != self.rate) {
            self.reader = Some(SikReader::new(self.rate));
        }
        Ok(())
    }

    fn stamp(&self, t: f64) -> Stamp {
        match self.clock.as_ref().and_then(|c| wall(self.live, c)) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    fn freq(&self, offset_hz: f64) -> f64 {
        if self.centre_hz > 0.0 {
            self.centre_hz + offset_hz
        } else {
            offset_hz
        }
    }

    fn burst(&mut self, i: usize) {
        self.held.counters.bursts += 1;
        let b = std::mem::take(&mut self.pool[i]);
        let got = self
            .reader
            .as_mut()
            .and_then(|r| r.read(&b, self.air_rate.bps()));
        let snr = f64::from(b.peak_db);
        self.pool[i] = b;
        match got {
            Some(d) => self.packet(&d, snr),
            None => self.held.counters.without_frame += 1,
        }
    }

    fn packet(&mut self, d: &Decoded, snr: f64) {
        let t = self.origin_s + d.sample as f64 / self.rate.max(1.0);
        let at = self.stamp(t);
        let freq = self.freq(d.offset_hz);
        let net = d.packet.net_id;
        let k = match self.streams.iter().position(|(n, _)| *n == net) {
            Some(k) => k,
            None => {
                self.streams.push((net, mavlink::Stream::new()));
                self.streams.len() - 1
            }
        };
        let mut frames = Vec::new();
        self.streams[k].1.push(&d.packet.data, |f| frames.push(f));
        self.snr_db = Some(snr);
        self.last_frame = t;
        self.dirty = true;
        if frames.is_empty() {
            let tr = d.packet.trailer;
            let detail = format!(
                "{} bytes, window {}{}{}{}, {} kbit/s",
                d.packet.data.len(),
                tr.window,
                if tr.command { ", command" } else { "" },
                if tr.bonus { ", bonus" } else { "" },
                if tr.resend { ", resend" } else { "" },
                d.rate / 1e3
            );
            let kind = if d.packet.ecc { "SiK packet (ECC)" } else { "SiK packet" };
            self.take(&Heard {
                t,
                link: LINK_NAME,
                id: format!("net {net}"),
                kind: kind.to_string(),
                detail,
                bytes: &d.bytes,
                freq_hz: freq,
                snr_db: snr,
                encrypted: false,
                position: None,
            }, at);
            return;
        }
        for f in &frames {
            let desc = mavlink::describe(f);
            let mut detail = desc.text;
            let _ = write!(
                detail,
                "; v{}, seq {}, {} kbit/s{}",
                f.version,
                f.seq,
                d.rate / 1e3,
                if d.packet.ecc { ", ECC" } else { "" }
            );
            self.take(&Heard {
                t,
                link: LINK_NAME,
                id: format!("net {net}, sys {} / {}", f.sys, f.comp),
                kind: f.name(),
                detail,
                bytes: &d.bytes,
                freq_hz: freq,
                snr_db: snr,
                encrypted: false,
                position: desc.position,
            }, at);
        }
    }

    fn take(&mut self, h: &Heard<'_>, at: Stamp) {
        let taken = self.held.take(h, at);
        if taken.first_position {
            self.events.push(Timed {
                time: h.t,
                value: VehicleSeen(NewLink {
                    link: h.link.to_string(),
                    id: h.id.clone(),
                    position: h.position.map(|(a, b, _)| (a, b)),
                }),
            });
        }
    }

    fn summary(&self, out: &mut String) {
        out.clear();
        let c = &self.held.counters;
        let span = self.rate / 2e6;
        // Writing into a `String` cannot fail.
        let _ = if !self.started {
            write!(out, "MAVLink (SiK): waiting for samples")
        } else if c.frames == 0 {
            write!(out, "MAVLink (SiK): searching ±{span:.2} MHz, {} bursts, no packet yet", c.bursts)
        } else {
            write!(
                out,
                "MAVLink (SiK): {} links, {} frames; searching ±{span:.2} MHz",
                self.held.links.len(),
                c.frames
            )
        };
    }

    fn covered(&self) -> String {
        let fits: Vec<String> = sik::AIR_RATES
            .iter()
            .filter(|&&r| self.rate >= crate::fsklink::RATE_MARGIN * r)
            .map(|r| format!("{}", r / 1e3))
            .collect();
        format!(
            "±{:.2} MHz around the centre; air rates {} kbit/s",
            self.rate / 2e6,
            fits.join(", ")
        )
    }
}

impl Decoder for SikDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "mavlink_sik.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<VehicleSeen>("vehicles")]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        SikSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (SikSetting::AirRate, Value::Choice(self.air_rate.index())),
            (SikSetting::ThresholdDb, Value::F64(self.threshold_db)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match SikSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SikSetting::AirRate => self.air_rate = AIR_RATE.read(id, v)?,
            SikSetting::ThresholdDb => {
                self.threshold_db = THRESHOLD_DB.read(id, v)?;
                if let Some(d) = self.det.as_mut() {
                    d.set_threshold_db(self.threshold_db);
                }
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(LinkFields::SHAPE.fields).with_tables(&[LINKS.def(), FRAMES.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre_hz = input.centre_hz;
        self.build()?;
        Ok(vec![Format {
            rate: 0.0,
            frame_len: 0,
            media: None,
            ..input.with_kind(Kind::typed::<VehicleSeen>())
        }])
    }
    fn retune(&mut self, input: &Format) {
        self.centre_hz = input.centre_hz;
        let _ = self.build();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = Some(ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        if std::mem::take(&mut self.fresh) {
            self.origin_s = first as f64 / self.rate.max(1.0);
        }
        self.used = 0;
        if let Some(d) = self.det.as_mut() {
            let (pool, used) = (&mut self.pool, &mut self.used);
            d.process(x, |b| {
                if *used == pool.len() {
                    pool.push(Burst::default());
                }
                pool[*used].take(b);
                *used += 1;
            });
        }
        for i in 0..self.used {
            self.burst(i);
        }
        if !self.events.is_empty() {
            out.items::<VehicleSeen>(0).append(&mut self.events);
        }
    }
    fn commands(&self) -> &'static [&'static Command] {
        LinkCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match LinkCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            LinkCommand::Remove => {
                let table = declaration()
                    .table_id(LINKS.key)
                    .unwrap_or(sdrkit_core::TableId(0));
                self.held.remove(args, table, &mut self.removed)?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn report(&mut self, p: &mut Published) {
        let recent = self.now - self.last_frame <= RECENT_S;
        for k in self.removed.drain(..) {
            p.remove_row(&LINKS, &k);
        }
        if std::mem::take(&mut self.cleared) {
            p.results_mut().clear_log();
            for t in [LINKS.def(), FRAMES.def()] {
                p.clear_rows(t);
            }
        }
        let covered = self.covered();
        p.put(&self.held.fields(recent, &covered, self.snr_db, NOT_CONFIRMED));
        if std::mem::take(&mut self.dirty) {
            self.held.rows(self.now, true, p);
        }
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let mut summary = std::mem::take(&mut self.summary);
        self.summary(&mut summary);
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            if changed {
                s.lock_state = lock;
                s.summary.clone_from(&summary);
            }
            changed
        });
        self.summary = summary;
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        self.streams.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The `mavlink_sik` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(SikDecoder::default()))
        .input("rx")
        .output("rx.vehicles")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("MAVLink (SiK)", MIN_RATE_HZ))
        .route(AIR_RATE.key(), &["rx.air_rate"])?
        .route(THRESHOLD_DB.key(), &["rx.threshold_db"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("rx", "remove"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator::from_config(cfg)?))
}

/// The `mavlink_sik` stack. Its default frequency is the centre of the test signal; the session gives the real one.
pub static MAVLINK_SIK: StackDescriptor = StackDescriptor {
    id: ID,
    name: "MAVLink (SiK)",
    group: crate::GROUP,
    blurb: "Telemetry radios of drones: SiK packets at every air rate, with and without error correction, the MAVLink messages in them and the position of each vehicle.",
    tuning: Tuning {
        no_offset_tuning: true,
        ..Tuning::anywhere(generator::TEST_CENTRE_HZ, 8e6, MIN_RATE_HZ, 8e6)
    },
    submodes: &[],
    settings: SikSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::MAVLINK_SIK_VERIFY),
    benchmarks: Some(benchmarks),
};

/// The benchmark of the receiver: 1 s of the test signal at 8 Msps.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "mavlink_sik.receiver_8m",
        run: |s| crate::bench_receiver(s, &MAVLINK_SIK, 1.0),
    }]
}
