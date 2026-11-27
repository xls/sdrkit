//! DGPS beacons on 283.5 to 325 kHz: differential GPS corrections in RTCM SC-104 version 2, sent by MSK at 50, 100 or 200 bits a
//! second. Three demodulators run side by side, and the bit rate whose words pass their parity is the one decoded.
//! Standard: ITU-R M.823-3, RTCM 10402.3.

pub mod rtcm;

use crate::fields::put_status;
use crate::typed::{
    DgpsAlmanacRow, DgpsCorrectionRow, DgpsFields, DgpsMessageRow, DgpsStationRow, DGPS_ALMANAC,
    DGPS_CORRECTIONS, DGPS_MESSAGES, DGPS_STATION,
};
use rtcm::{
    base_position, decode, encode_almanac, encode_base_station, encode_corrections, encode_text,
    frame_words, header_words, AlmanacEntry, Correction, FramerStats, RtcmFrame, RtcmFramer,
    RtcmMessage,
};
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::SimError;
use sdrkit_components::dsp::{Channel, PowerMeterSink};
use sdrkit_components::modem::demod::MskDemod;
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig,
    Generator, LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, StageError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Item, Kept, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_util_geo::Position;
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "dgps";
/// The least radio rate.
pub const MIN_RATE_HZ: f64 = 8e3;
/// The channel's rate (an exact rate: 20 samples a bit at 200 bps).
pub const CHANNEL_RATE_HZ: f64 = 4e3;
/// The channel's width.
pub const CHANNEL_WIDTH_HZ: f64 = 1e3;
/// The bit rates the beacons use. The modulation is MSK, and a one is the higher frequency.
/// Standard: ITU-R M.823-3 Annex 1, 2.
pub const BIT_RATES: [f64; 3] = [50.0, 100.0, 200.0];
/// The `i8` units for each unit of natural-log LLR on the demodulators' output.
pub const LLR_SCALE: f64 = 8.0;
/// The time constant of the carrier-offset removal in front of each demodulator, s. The data average out over a few seconds.
const OFFSET_TAU_S: f64 = 2.0;

/// The link as the selector sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DgpsLink {
    /// The bit rate chosen.
    pub bitrate: f64,
    /// That rate's framer counters.
    pub stats: FramerStats,
}

impl Item for DgpsLink {
    const KIND: &'static str = "dgps.link";
}

sdrkit_core::choice_enum! {
    /// The bit rate that [`RtcmSelect`] passes on.
    pub enum BitRateChoice {
        /// The rate with the most good words of late.
        Auto = ("0", "Automatic"),
        /// 50 bits a second.
        B50 = ("50", "50 bps"),
        /// 100 bits a second.
        B100 = ("100", "100 bps"),
        /// 200 bits a second.
        B200 = ("200", "200 bps"),
    }
}

impl BitRateChoice {
    /// The index of the rate in [`BIT_RATES`], or `None` for [`BitRateChoice::Auto`].
    pub fn rate_index(self) -> Option<usize> {
        match self {
            BitRateChoice::Auto => None,
            BitRateChoice::B50 => Some(0),
            BitRateChoice::B100 => Some(1),
            BitRateChoice::B200 => Some(2),
        }
    }
}

pub const BITRATE: Setting<field::Choice<BitRateChoice>> =
    Setting::choice("bitrate", "Bit rate", BitRateChoice::Auto);

sdrkit_core::settings! {
    /// The settings of [`RtcmSelect`].
    pub enum RtcmSelectSetting {
        /// The bit rate passed on.
        Bitrate = BITRATE,
    }
}

/// The bit rate selector (`avnav.rtcm_select`): frames and counters of the three rates in, the chosen rate's frames and link
/// out.
///
/// At each tick of the graph's clock, the score of each rate takes the good words less the bad words since the last tick (for
/// each 30 bits of the rate) and keeps 0.9 of its old value, so the choice is the same whatever the cuts of the input.
pub struct RtcmSelect {
    bitrate: BitRateChoice,
    score: [f64; 3],
    pending: [f64; 3],
    last_good: [u64; 3],
    stats: [FramerStats; 3],
    chosen: usize,
}

impl Default for RtcmSelect {
    fn default() -> Self {
        RtcmSelect {
            bitrate: BITRATE.default,
            score: [0.0; 3],
            pending: [0.0; 3],
            last_good: [0; 3],
            stats: [FramerStats::default(); 3],
            chosen: 1,
        }
    }
}

impl Component for RtcmSelect {
    fn info(&self) -> ComponentInfo {
        let mut inputs: Vec<Port> = BIT_RATES
            .iter()
            .map(|r| Port::typed::<RtcmFrame>(&format!("f{r}")))
            .collect();
        inputs.extend(
            BIT_RATES
                .iter()
                .map(|r| Port::typed::<FramerStats>(&format!("s{r}"))),
        );
        ComponentInfo::processor(
            "avnav.rtcm_select",
            inputs,
            vec![
                Port::typed::<RtcmFrame>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<DgpsLink>("link").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(RtcmSelectSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = RtcmSelect {
            bitrate: self.bitrate,
            ..Default::default()
        };
        Ok(vec![
            Format {
                rate: 0.0,
                ..inputs[0].with_kind(Kind::typed::<RtcmFrame>())
            },
            Format {
                rate: 0.0,
                ..inputs[0].with_kind(Kind::typed::<DgpsLink>())
            },
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut time = None;
        for i in 0..3 {
            if let Some(s) = io.items::<FramerStats>(3 + i).last() {
                let t = s.time;
                let s = s.value;
                let new = s.good_words.saturating_sub(self.last_good[i]) as f64;
                let bad = s.bad_words.saturating_sub(self.stats[i].bad_words) as f64;
                self.last_good[i] = s.good_words;
                self.pending[i] += (new - bad) * 30.0 / BIT_RATES[i];
                self.stats[i] = s;
                time = Some(t);
            }
        }
        if io.tick() {
            for (s, p) in self.score.iter_mut().zip(self.pending.iter_mut()) {
                *s = 0.9 * *s + std::mem::take(p);
            }
        }
        self.chosen = match self.bitrate.rate_index() {
            Some(i) => i,
            None => {
                let best = (0..3)
                    .max_by(|&a, &b| self.score[a].total_cmp(&self.score[b]))
                    .unwrap_or(1);
                if self.score[best] > self.score[self.chosen] + 0.5 {
                    best
                } else {
                    self.chosen
                }
            }
        };
        let frames = io.items::<RtcmFrame>(self.chosen);
        io.items_out::<RtcmFrame>(0).extend_from_slice(frames);
        if let Some(t) = time {
            let link = DgpsLink {
                bitrate: BIT_RATES[self.chosen],
                stats: self.stats[self.chosen],
            };
            io.items_out::<DgpsLink>(1).push(Timed {
                time: t,
                value: link,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RtcmSelectSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RtcmSelectSetting::Bitrate => self.bitrate = BITRATE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            RtcmSelectSetting::Bitrate,
            Value::Choice(self.bitrate.index()),
        )])
    }
}

sdrkit_core::choice_enum! {
    /// The station health of an RTCM frame header.
    /// Standard: RTCM 10402.3 table 4-1.
    pub enum StationHealth {
        /// UDRE scale factor 1.
        Scale1 = ("scale_1", "UDRE scale 1"),
        /// UDRE scale factor 0.75.
        Scale075 = ("scale_0.75", "UDRE scale 0.75"),
        /// UDRE scale factor 0.5.
        Scale05 = ("scale_0.5", "UDRE scale 0.5"),
        /// UDRE scale factor 0.3.
        Scale03 = ("scale_0.3", "UDRE scale 0.3"),
        /// UDRE scale factor 0.2.
        Scale02 = ("scale_0.2", "UDRE scale 0.2"),
        /// UDRE scale factor 0.1.
        Scale01 = ("scale_0.1", "UDRE scale 0.1"),
        /// The transmission is not monitored.
        NotMonitored = ("not_monitored", "not monitored"),
        /// The station is not working.
        NotWorking = ("not_working", "not working"),
    }
}

impl StationHealth {
    /// The health of a header's 3-bit field. Only the low three bits count.
    pub fn of(bits: u8) -> StationHealth {
        StationHealth::from_index(u16::from(bits & 7)).unwrap_or(StationHealth::NotWorking)
    }
}

fn new_published() -> Published {
    let mut p = crate::typed::store(
        &[DgpsFields::SHAPE],
        &[
            &DGPS_STATION,
            &DGPS_MESSAGES,
            &DGPS_CORRECTIONS,
            &DGPS_ALMANAC,
        ],
    );
    put_status(&mut p, LockState::Searching, None, None, None, "Searching");
    p
}

/// The DGPS report (`avnav.dgps_report`): frames and the link in, results out. Its command `clear` forgets the station.
pub struct DgpsReport {
    published: Published,
    frames: u64,
    types: BTreeMap<u8, u64>,
    corrections: BTreeMap<u8, Correction>,
    station_id: Option<u16>,
    health: Option<StationHealth>,
    zcount_s: Option<f64>,
    /// The position and the height of the base station.
    base: Option<((f64, f64), f64)>,
    /// The satellites flagged unhealthy, by PRN, after the first message of type 5.
    unhealthy: Option<String>,
    /// The last text message.
    message: Option<String>,
    link: Option<DgpsLink>,
    last_frame: f64,
    now: f64,
    text: String,
}

impl Default for DgpsReport {
    fn default() -> Self {
        DgpsReport {
            published: new_published(),
            frames: 0,
            types: BTreeMap::new(),
            corrections: BTreeMap::new(),
            station_id: None,
            health: None,
            zcount_s: None,
            base: None,
            unhealthy: None,
            message: None,
            link: None,
            last_frame: f64::MIN,
            now: 0.0,
            text: String::with_capacity(64),
        }
    }
}

impl DgpsReport {
    fn frame(&mut self, time: f64, f: &RtcmFrame) {
        self.frames += 1;
        let n = {
            let c = self.types.entry(f.msg_type).or_insert(0);
            *c += 1;
            *c
        };
        self.station_id = Some(f.station_id);
        self.health = Some(StationHealth::of(f.health));
        self.zcount_s = Some(f.zcount_s);
        self.last_frame = time;
        let key = &mut self.text;
        let p = &mut self.published;
        key.clear();
        // Writing into a String cannot fail.
        let _ = write!(key, "{:02}", f.msg_type);
        DGPS_MESSAGES.put(
            p,
            key,
            &DgpsMessageRow {
                msg_type: i64::from(f.msg_type),
                count: n as i64,
            },
        );
        match decode(f) {
            RtcmMessage::Corrections { corrections, .. } => {
                for c in corrections {
                    self.corrections.insert(c.prn, c);
                    key.clear();
                    let _ = write!(key, "{:02}", c.prn);
                    DGPS_CORRECTIONS.put(
                        p,
                        key,
                        &DgpsCorrectionRow {
                            prn: i64::from(c.prn),
                            prc_m: c.prc_m,
                            rrc_m_s: c.rrc_m_s,
                            iod: i64::from(c.iod),
                            udre: i64::from(c.udre),
                        },
                    );
                }
            }
            RtcmMessage::BaseStation { x_m, y_m, z_m } => {
                if let Some(pos) = base_position(x_m, y_m, z_m) {
                    self.base = Some(((pos.lat_deg, pos.lon_deg), pos.alt_m));
                    key.clear();
                    let _ = write!(key, "DGPS {}", f.station_id);
                    p.results_mut()
                        .update_point("station", pos.lat_deg, pos.lon_deg, key, None);
                    // The point shows the row of the station.
                    key.clear();
                    let _ = write!(key, "{}", f.station_id);
                    let table = p.results().decl().table_id(DGPS_STATION.key);
                    p.results_mut()
                        .link_point("station", table.map(|t| (t, key.as_str())));
                }
            }
            RtcmMessage::Almanac { beacons } => {
                for b in &beacons {
                    key.clear();
                    let _ = write!(key, "{}", b.station_id);
                    DGPS_ALMANAC.put(
                        p,
                        key,
                        &DgpsAlmanacRow {
                            station_id: i64::from(b.station_id),
                            freq_hz: Hertz((b.freq_khz * 1e3).round()),
                            bit_rate_hz: Hertz(f64::from(b.bitrate)),
                            position: (b.lat_deg, b.lon_deg),
                            range_km: f64::from(b.range_km),
                            health: i64::from(b.health),
                        },
                    );
                }
            }
            RtcmMessage::ConstellationHealth { satellites } => {
                let list = self.unhealthy.get_or_insert_with(String::new);
                list.clear();
                for s in satellites.iter().filter(|s| s.warning) {
                    if !list.is_empty() {
                        list.push_str(", ");
                    }
                    let _ = write!(list, "{}", s.prn);
                }
            }
            RtcmMessage::Text { text } => {
                p.results_mut().log(format!("{time:.1} s: text \"{text}\""));
                self.message = Some(text);
            }
            RtcmMessage::Null | RtcmMessage::Other { .. } => {}
        }
        self.put_fields();
    }

    /// Writes the result fields from the state.
    fn put_fields(&mut self) {
        let stats = self.link.map(|l| l.stats).unwrap_or_default();
        self.published.put(&DgpsFields {
            station_id: self.station_id.map(i64::from),
            health: self.health,
            zcount: self.zcount_s.map(Seconds),
            symbol_rate_hz: self.link.map(|l| Hertz(l.bitrate)),
            locked: stats.locked,
            good_words: stats.good_words as i64,
            bad_words: stats.bad_words as i64,
            frames_ok: self.frames as i64,
            base_station: self.base.map(|(p, _)| p),
            base_height_m: self.base.map(|(_, h)| h),
            text: self.message.as_deref(),
            unhealthy: self.unhealthy.as_deref(),
            corrections: self.corrections.len() as i64,
        });
        // The station table: one row, keyed by the station id of the frames.
        if let Some(id) = self.station_id {
            let key = &mut self.text;
            key.clear();
            // Writing into a String cannot fail.
            let _ = write!(key, "{id}");
            DGPS_STATION.put(
                &mut self.published,
                key,
                &DgpsStationRow {
                    station_id: i64::from(id),
                    health: self.health,
                    position: self.base.map(|(p, _)| p),
                    height_m: self.base.map(|(_, h)| h),
                },
            );
        }
    }

    fn publish(&mut self) {
        let recent = self.now - self.last_frame < 10.0;
        self.text.clear();
        match (recent, self.station_id) {
            (true, Some(id)) => {
                // Writing into a String cannot fail.
                let _ = write!(
                    self.text,
                    "Station {id}, {:.0} bps, {} corrections",
                    self.link.map_or(0.0, |l| l.bitrate),
                    self.corrections.len()
                );
            }
            _ => self.text.push_str("Searching"),
        }
        let lock_state = if recent {
            LockState::Locked
        } else if self.link.is_some_and(|l| l.stats.locked) {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        put_status(
            &mut self.published,
            lock_state,
            None,
            None,
            None,
            &self.text,
        );
    }
}

impl Component for DgpsReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "avnav.dgps_report",
            vec![
                Port::typed::<RtcmFrame>("frames"),
                Port::typed::<DgpsLink>("link"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.put_fields();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<RtcmFrame>(0) {
            self.now = self.now.max(f.time);
            self.frame(f.time, &f.value);
        }
        if let Some(l) = io.items::<DgpsLink>(1).last() {
            self.now = self.now.max(l.time);
            self.link = Some(l.value);
            self.put_fields();
        }
        if io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                *self = DgpsReport::default();
                self.put_fields();
            }
        }
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        let p = &mut self.published;
        self.corrections.clear();
        p.clear_rows(&DGPS_CORRECTIONS);
        if !kept.table(DGPS_MESSAGES.key) {
            self.types.clear();
            p.clear_rows(&DGPS_MESSAGES);
        }
        if !kept.table(DGPS_ALMANAC.key) {
            p.clear_rows(&DGPS_ALMANAC);
        }
        if !kept.field("frames_ok") {
            self.frames = 0;
        }
        if !kept.field("text") {
            self.message = None;
        }
        p.results_mut().clear_points();
        p.clear_rows(&DGPS_STATION);
        self.station_id = None;
        self.health = None;
        self.zcount_s = None;
        self.base = None;
        self.unhealthy = None;
        self.link = None;
        self.last_frame = f64::MIN;
        self.put_fields();
        self.publish();
    }
}

/// The DGPS graph: the channel to three demodulators and framers (one for each bit rate), the selector, the report, and a
/// power meter.
///
/// # Errors
///
/// A [`GraphError`] if the graph cannot be built.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let mut b = GraphBuilder::new()
        .add(
            "chan",
            GraphError::checked("chan", Channel::new(CHANNEL_WIDTH_HZ, CHANNEL_RATE_HZ))?,
        )
        .set("chan.wide", Value::Hz(CHANNEL_WIDTH_HZ))
        .set("chan.exact", Value::Bool(true))
        .add("level", PowerMeterSink::default())
        .add("select", RtcmSelect::default())
        .add("report", DgpsReport::default())
        .input("chan")
        .connect("chan", "level");
    for r in BIT_RATES {
        let (m, f) = (format!("msk{r}"), format!("framer{r}"));
        b = b
            .add(&m, MskDemod::default())
            .set(&format!("{m}.baud"), Value::Hz(r))
            .set(&format!("{m}.invert"), Value::Bool(true))
            .set(&format!("{m}.offset_tau_s"), Value::F64(OFFSET_TAU_S))
            .set(&format!("{m}.llr_scale"), Value::F64(LLR_SCALE))
            .add(&f, RtcmFramer::default())
            .connect("chan", &m)
            .connect(&format!("{m}.out"), &f)
            .connect(&format!("{f}.out"), &format!("select.f{r}"))
            .connect(&format!("{f}.stats"), &format!("select.s{r}"));
    }
    b.connect("select.out", "report.frames")
        .connect("select.link", "report.link")
        .output("select.out")
        .clock(0.25)
        .status_from("report")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

sdrkit_core::choice_enum! {
    /// The bit rate of the test signal.
    pub enum GenBitRate {
        /// 50 bits a second.
        B50 = ("50", "50"),
        /// 100 bits a second.
        B100 = ("100", "100"),
        /// 200 bits a second.
        B200 = ("200", "200"),
    }
}

impl GenBitRate {
    /// Bits a second.
    pub fn hz(self) -> f64 {
        match self {
            GenBitRate::B50 => 50.0,
            GenBitRate::B100 => 100.0,
            GenBitRate::B200 => 200.0,
        }
    }
}

pub const GEN_BITRATE: Setting<field::Choice<GenBitRate>> =
    Setting::choice("gen_bitrate", "Test signal: bit rate", GenBitRate::B200)
        .section("Test signal")
        .advanced();
pub const GEN_STATION: Setting<field::I64> =
    Setting::new("gen_station", "Test signal: station id", "", 333)
        .range(0.0, 1023.0, 1.0)
        .section("Test signal")
        .advanced();
pub const GEN_LAT: Setting<field::F64> =
    Setting::new("gen_lat", "Test signal: latitude", "°", -33.86)
        .range(-90.0, 90.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_LON: Setting<field::F64> =
    Setting::new("gen_lon", "Test signal: longitude", "°", 151.21)
        .range(-180.0, 180.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_TEXT: Setting<field::Text> =
    Setting::new("gen_text", "Test signal: text", "", "SDRKIT TEST BEACON")
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the DGPS stack.
    pub enum DgpsSetting {
        /// The bit rate decoded.
        Bitrate = BITRATE,
        /// The bit rate of the test signal.
        GenBitrate = GEN_BITRATE,
        /// The station id of the test signal.
        GenStation = GEN_STATION,
        /// The latitude of the test signal's base station.
        GenLat = GEN_LAT,
        /// The longitude of the test signal's base station.
        GenLon = GEN_LON,
        /// The text message of the test signal.
        GenText = GEN_TEXT,
    }
}

fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "DGPS needs a sample rate of at least 8 kHz")
        .route("bitrate", &["select.bitrate"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("report", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The DGPS receiver.
pub fn receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(stack_graph(cfg)?))
}

/// What a DGPS test signal sends.
#[derive(Clone, Debug)]
pub struct DgpsSignal {
    /// Bits a second.
    pub bitrate: f64,
    /// Station id.
    pub station: u16,
    /// Base station latitude, degrees.
    pub lat: f64,
    /// Base station longitude, degrees.
    pub lon: f64,
    /// Text of the type 16 message.
    pub text: String,
    /// Corrections sent (type 9).
    pub corrections: Vec<Correction>,
}

impl DgpsSignal {
    /// The default test content: six corrections, the base station, the text and an almanac of one beacon.
    pub fn standard(bitrate: f64, station: u16, lat: f64, lon: f64, text: &str) -> Self {
        let corrections = [
            (2u8, -5.12),
            (5, 12.34),
            (12, -0.5),
            (15, 23.1),
            (21, -7.7),
            (29, 1.02),
        ]
        .iter()
        .enumerate()
        .map(|(i, &(prn, prc))| Correction {
            prn,
            udre: (i % 3) as u8,
            prc_m: prc,
            rrc_m_s: 0.002 * i as f64,
            iod: 10 + i as u8,
        })
        .collect();
        DgpsSignal {
            bitrate,
            station,
            lat,
            lon,
            text: text.into(),
            corrections,
        }
    }
}

/// The DGPS test signal: an MSK carrier at 0 Hz and unit power, sending a cycle of RTCM frames.
pub struct DgpsGenerator {
    rate: f64,
    sig: DgpsSignal,
    cpm: Cpm,
    rotation: Complex32,
    levels: Vec<f32>,
    ready: Vec<Complex32>,
    used: usize,
    d29: bool,
    d30: bool,
    seq: u8,
    cycle: usize,
    bits_sent: u64,
}

impl DgpsGenerator {
    /// At `rate` samples a second.
    ///
    /// # Errors
    ///
    /// The [`SimError`] of the MSK modulator for a rate below two samples a bit.
    pub fn new(rate: f64, sig: DgpsSignal, seed: u64) -> Result<Self, SimError> {
        let phase = sdrkit_algo_sim::rng::Rng::new(seed).uniform() * TAU;
        let cpm = Cpm::msk(rate / sig.bitrate)?;
        Ok(DgpsGenerator {
            cpm,
            rotation: Complex32::new(phase.cos() as f32, phase.sin() as f32),
            levels: Vec::with_capacity(1024),
            ready: Vec::new(),
            used: 0,
            rate,
            sig,
            d29: false,
            d30: false,
            seq: 0,
            cycle: 0,
            bits_sent: 0,
        })
    }
    /// From the stack's settings.
    ///
    /// # Errors
    ///
    /// What [`DgpsGenerator::new`] returns.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, SimError> {
        let s = &cfg.settings;
        let sig = DgpsSignal::standard(
            s.get(&GEN_BITRATE).hz(),
            s.get(&GEN_STATION).clamp(0, 1023) as u16,
            s.get(&GEN_LAT),
            s.get(&GEN_LON),
            s.get(&GEN_TEXT),
        );
        DgpsGenerator::new(cfg.rate_hz, sig, cfg.seed)
    }
    /// Modulates the next frame of the cycle into `ready`.
    fn next_frame(&mut self) {
        let e = Position {
            lat_deg: self.sig.lat,
            lon_deg: self.sig.lon,
            alt_m: 50.0,
        }
        .to_ecef();
        let (t, data) = match self.cycle % 6 {
            0 => (3u8, encode_base_station(e.x_m, e.y_m, e.z_m)),
            1 => (9, encode_corrections(&self.sig.corrections[..3])),
            2 => (9, encode_corrections(&self.sig.corrections[3..])),
            3 => (16, encode_text(&self.sig.text)),
            4 => (
                7,
                encode_almanac(&[AlmanacEntry {
                    lat_deg: self.sig.lat,
                    lon_deg: self.sig.lon,
                    range_km: 250,
                    freq_khz: 304.0,
                    health: 0,
                    station_id: self.sig.station + 400,
                    bitrate: self.sig.bitrate as u16,
                    fsk: false,
                }]),
            ),
            _ => (6, vec![]),
        };
        self.cycle += 1;
        let z = (self.bits_sent as f64 / self.sig.bitrate) % 3600.0;
        let h = header_words(t, self.sig.station, z, self.seq, data.len(), 0);
        self.seq = (self.seq + 1) % 8;
        self.levels.clear();
        for w in frame_words(h, &data, &mut self.d29, &mut self.d30) {
            // A one turns the phase forward (the higher frequency).
            self.levels.extend(
                (0..30)
                    .rev()
                    .map(|k| if (w >> k) & 1 == 1 { 1.0 } else { -1.0 }),
            );
        }
        self.bits_sent += self.levels.len() as u64;
        self.ready.clear();
        self.used = 0;
        self.cpm.push(&self.levels, &mut self.ready);
    }
}

impl Generator for DgpsGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(1.5 * self.sig.bitrate)
                .with_symbol_rate(self.sig.bitrate)
                .with_bit_rate(self.sig.bitrate),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.used == self.ready.len() {
                self.next_frame();
                continue;
            }
            let k = (self.ready.len() - self.used).min(out.len() - i);
            for (o, &z) in out[i..i + k]
                .iter_mut()
                .zip(&self.ready[self.used..self.used + k])
            {
                *o = z * self.rotation;
            }
            self.used += k;
            i += k;
        }
        out.len()
    }
}

/// Makes the DGPS generator.
///
/// # Errors
///
/// [`ConfigError::Stage`] if the rate is too low for the bit rate.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        DgpsGenerator::from_config(cfg).map_err(|e| ConfigError::Stage(StageError::new(e)))?,
    ))
}
