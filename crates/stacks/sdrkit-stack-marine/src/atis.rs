//! ATIS on inland waterways: the identity burst a VHF radio sends after each transmission, in the DSC code. The receiver plays the
//! voice and decodes the bursts on seven offset branches, both polarities.
//!
//! Standard: ETSI EN 300 698-1 Annex B, ITU-R Report M.1159

use crate::dsc::call::station_name;
use crate::dsc::decoder::{DscDecoder, RawCall};
use crate::dsc::symbols;
use crate::dsp::FixedRateFrontEnd;
use crate::fsk::{AfskSlicer, CHANNEL_RATE};
use crate::report::{Clock, Hearing};
use crate::vhfplan::{Plan, PLAN};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir;
use sdrkit_algo_dsp::floor::MinFollower;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Arguments, CommandKind, EventKind, Fields, Hertz, Table, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, Record, Results, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder,
    GraphError, Io, Item, Kind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_ids::mid;
use sdrkit_util_ids::names::{IdKind, IdNames};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The branch offsets, Hz.
pub const BRANCHES: [f64; 7] = [0.0, -500.0, 500.0, -1000.0, 1000.0, -1600.0, 1600.0];
/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 48e3;
/// The tune offset (the radio above the dial).
pub const TUNE_OFFSET_HZ: f64 = 150e3;
/// The ATIS bit rate.
pub const BAUD: f64 = 1200.0;
/// The most messages listed.
pub const MESSAGE_CAP: usize = 300;
/// The most stations listed.
pub const STATION_CAP: usize = 200;
/// The time between two reports, seconds.
pub const REPORT_S: f64 = 0.25;
/// Copies of one burst found within this time are one message, seconds.
const GROUP_S: f64 = 0.25;

/// The first letter of a call sign by MID, with how sure it is ("" sure, "likely"). Standard: RR Appendix 42.
pub fn first_letter(mid: u16) -> Option<(char, &'static str)> {
    Some(match mid {
        244..=246 => ('P', ""),
        211 | 218 => ('D', ""),
        205 => ('O', ""),
        226..=228 => ('F', ""),
        269 => ('H', ""),
        253 => ('L', ""),
        203 => ('O', ""),
        243 => ('H', ""),
        267 => ('O', ""),
        270 => ('O', ""),
        264 => ('Y', ""),
        207 => ('L', ""),
        261 => ('S', "likely"),
        219 | 220 => ('O', ""),
        265 | 266 => ('S', "likely"),
        257..=259 => ('L', ""),
        230 => ('O', ""),
        _ => return None,
    })
}

/// An interpreted identity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    /// Ten digits, `~` where unreadable.
    pub digits: String,
    /// The format is 121 and five symbols arrived.
    pub valid: bool,
    /// Some digits are unreadable.
    pub partial: bool,
    /// The identity starts with 9.
    pub standard_z: bool,
    /// The MID (the three digits after the 9).
    pub mid: Option<u16>,
    /// The country of the MID.
    pub country: String,
    /// The nine digits after the 9.
    pub mmsi: String,
    /// The call sign the digits spell.
    pub callsign: String,
    /// How sure the first letter is: "" sure, "likely", "unknown".
    pub letter_note: String,
}

/// Interprets the symbols of a call of format 121. Standard: EN 300 698-1 Annex B.
pub fn interpret(format: Option<u8>, message: &[Option<u8>]) -> Identity {
    let mut id = Identity::default();
    if format != Some(121) || message.len() < 5 {
        return id;
    }
    id.digits = symbols::digits(&message[..5]);
    id.valid = true;
    id.partial = id.digits.contains('~');
    let d: Vec<char> = id.digits.chars().collect();
    id.standard_z = d[0] == '9';
    id.mmsi = d[1..].iter().collect();
    let mid_s: String = d[1..4].iter().collect();
    if let Ok(m) = mid_s.parse::<u16>() {
        id.mid = Some(m);
        id.country = mid::country(m).unwrap_or("").into();
    }
    if id.standard_z {
        let x12: String = d[4..6].iter().collect();
        let second = x12
            .parse::<u8>()
            .ok()
            .filter(|v| (1..=26).contains(v))
            .map(|v| (b'A' + v - 1) as char)
            .unwrap_or('?');
        let num: String = d[6..10].iter().collect();
        let (first, note) = match id.mid.and_then(first_letter) {
            Some((c, n)) => (c, n),
            None => ('?', "unknown"),
        };
        id.callsign = format!("{first}{second}{num}");
        id.letter_note = note.into();
    }
    id
}

/// Builds the ten digits from a call sign of letter, letter, four digits and a MID, or 9 and nine MMSI digits. Returns `None` for
/// any other text.
pub fn build(callsign_or_mmsi: &str, mid: Option<u16>) -> Option<String> {
    let s = callsign_or_mmsi.trim().to_ascii_uppercase();
    if s.len() == 9 && s.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!("9{s}"));
    }
    let c: Vec<char> = s.chars().collect();
    if c.len() != 6
        || !c[0].is_ascii_uppercase()
        || !c[1].is_ascii_uppercase()
        || !c[2..].iter().all(|x| x.is_ascii_digit())
    {
        return None;
    }
    let mid = mid?;
    let x12 = c[1] as u8 - b'A' + 1;
    let num: String = c[2..].iter().collect();
    Some(format!("9{mid:03}{x12:02}{num}"))
}

/// The bits of an ATIS burst for ten digits: `dots` dot bits, phasing, format 121 twice, five symbols, EOS 127, the check.
pub fn burst_bits(digits: &str, dots: usize) -> Vec<u8> {
    symbols::call_bits(dots, 121, &symbols::number_symbols(digits), 127)
}

/// A decoded ATIS message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AtisMessage {
    /// Its number since the start.
    pub serial: u64,
    /// When it was heard.
    pub at: Stamp,
    /// The ten digits.
    pub digits: String,
    /// The nine digits after the 9.
    pub mmsi: String,
    /// The country of the MID.
    pub country: String,
    /// The call sign the digits spell.
    pub callsign: String,
    /// How sure the first letter is.
    pub letter_note: String,
    /// The name from the name table.
    pub name: String,
    /// The check symbol agrees.
    pub check_ok: bool,
    /// The identity starts with 9.
    pub standard_z: bool,
    /// Symbols taken from the second copy or rebuilt.
    pub repaired: usize,
    /// Symbols unreadable in both copies.
    pub erasures: usize,
    /// The tones were swapped.
    pub inverted: bool,
    /// The channel level over its floor, dB.
    pub level_db: f64,
    /// The offset of the branch that found it, Hz.
    pub branch_hz: f64,
}

/// A station heard (the table keeps when and how often).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AtisStation {
    /// The ten digits.
    pub digits: String,
    /// The nine digits after the 9.
    pub mmsi: String,
    /// The country of the MID.
    pub country: String,
    /// The call sign.
    pub callsign: String,
    /// The name from the name table.
    pub name: String,
    /// The best level over the floor, dB.
    pub best_level_db: f64,
}

/// A station holds no time of its own: the table's [`Seen`] has them.
impl Shift for AtisStation {
    fn shift_times(&mut self, _: f64) {}
}

impl Shift for AtisMessage {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

/// A call one branch found.
#[derive(Clone, Debug)]
pub struct BranchCall {
    /// The call.
    pub raw: RawCall,
    /// The offset of the branch, Hz.
    pub branch_hz: f64,
    /// Its end, signal seconds since the configure.
    pub time: f64,
}
impl Item for BranchCall {
    const KIND: &'static str = "atis.call";
}

/// A tick report of a branch or the level meter.
#[derive(Clone, Copy, Debug, Default)]
pub struct AtisLevel {
    /// A decoder of the branch holds the phasing.
    pub locked: bool,
    /// The smoothed channel power.
    pub level: f64,
    /// The floor of the channel power.
    pub floor: f64,
}
impl Item for AtisLevel {
    const KIND: &'static str = "atis.level";
}

/// One offset branch (`atis.branch`): shifted by its offset before the discriminator, then the AFSK slicer and both polarities.
pub struct Branch {
    offset: f64,
    nco: Nco,
    disc: Discriminator,
    slicer: AfskSlicer,
    dec: [DscDecoder; 2],
    mixed: Vec<Complex32>,
    audio: Vec<f32>,
    soft: Vec<f32>,
}

impl Branch {
    /// A branch at `offset_hz`.
    pub fn new(offset_hz: f64) -> Self {
        Branch {
            offset: offset_hz,
            nco: Nco::new(CHANNEL_RATE, -offset_hz),
            disc: Discriminator::new(CHANNEL_RATE, 3000.0)
                .expect("the rate and deviation are positive"),
            slicer: AfskSlicer::balanced(),
            dec: [DscDecoder::new(false), DscDecoder::new(true)],
            mixed: Vec::new(),
            audio: Vec::new(),
            soft: Vec::new(),
        }
    }
}

impl Component for Branch {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atis.branch",
            vec![Port::iq("in")],
            vec![
                Port::typed::<BranchCall>("calls").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<AtisLevel>("level").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = Branch::new(self.offset);
        Ok(vec![
            Format::new(Kind::typed::<BranchCall>(), 0.0),
            Format::new(Kind::typed::<AtisLevel>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.nco.mix_in_place(&mut self.mixed);
        self.audio.clear();
        self.disc.process(&self.mixed, &mut self.audio);
        self.soft.clear();
        self.slicer.process(&self.audio, &mut self.soft);
        for v in &mut self.soft {
            *v = -*v;
        }
        for d in &mut self.dec {
            d.push(&self.soft);
            for raw in d.take_calls() {
                let time = raw.end_bit as f64 / BAUD;
                io.items_out::<BranchCall>(0).push(Timed {
                    time,
                    value: BranchCall {
                        raw,
                        branch_hz: self.offset,
                        time,
                    },
                });
            }
        }
        if io.tick() {
            let locked = self.dec.iter().any(|d| d.locked());
            io.items_out::<AtisLevel>(1).push(Timed {
                time: 0.0,
                value: AtisLevel {
                    locked,
                    ..Default::default()
                },
            });
        }
    }
}

/// The samples of one level block: 10 ms.
const LEVEL_BLOCK: usize = 240;

/// The channel level (`atis.level`): the mean power of each 10 ms block smoothed by 0.1 per block, and a floor that follows the
/// lowest block at once and rises by 0.05 % per block. A block of digital silence leaves the floor where it is. The output format
/// carries the channel's frequency.
pub struct LevelMeter {
    count: u64,
    acc: f64,
    n: usize,
    level: f64,
    floor: MinFollower,
}

impl Default for LevelMeter {
    fn default() -> Self {
        LevelMeter {
            count: 0,
            acc: 0.0,
            n: 0,
            level: 0.0,
            floor: MinFollower::new(1.0, db(1.0005) as f32).expect("the floor rise is valid"),
        }
    }
}

impl Component for LevelMeter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atis.level",
            vec![Port::iq("in")],
            vec![Port::typed::<AtisLevel>("level").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = LevelMeter::default();
        let mut o = Format::new(Kind::typed::<AtisLevel>(), 0.0);
        o.tuned_hz = inputs[0].tuned_hz;
        o.centre_hz = inputs[0].tuned_hz;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.iq(0) {
            self.count += 1;
            self.acc += f64::from(s.norm_sqr());
            self.n += 1;
            if self.n == LEVEL_BLOCK {
                let p = self.acc / LEVEL_BLOCK as f64;
                self.level = if self.level == 0.0 {
                    p
                } else {
                    self.level + 0.1 * (p - self.level)
                };
                if p > 0.0 {
                    self.floor.push(p as f32);
                }
                self.acc = 0.0;
                self.n = 0;
            }
        }
        if io.tick() {
            let time = self.count as f64 / CHANNEL_RATE;
            io.items_out::<AtisLevel>(0).push(Timed {
                time,
                value: AtisLevel {
                    locked: false,
                    level: self.level,
                    floor: self.floor.floor().map_or(0.0, f64::from),
                },
            });
        }
    }
}

/// Listen to the voice.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", true)
    .help("The voice of the channel on the audio output.");

sdrkit_core::settings! {
    /// The settings of [`Voice`].
    pub enum VoiceSetting {
        /// Listen to the voice.
        Listen = LISTEN,
    }
}

/// The voice (`atis.voice`): narrow FM at 8 kHz on the output (mono).
pub struct Voice {
    disc: Discriminator,
    dec: FirDecimator<f32>,
    audio: Vec<f32>,
    low: Vec<f32>,
    listen: bool,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            disc: Discriminator::new(CHANNEL_RATE, 5000.0)
                .expect("the rate and deviation are positive"),
            dec: FirDecimator::new(
                &fir::lowpass(CHANNEL_RATE, 3000.0, 3800.0, 50.0)
                    .expect("the filter settings are valid"),
                3,
            ),
            audio: Vec::new(),
            low: Vec::new(),
            listen: true,
        }
    }
}

impl Component for Voice {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atis.voice",
            vec![Port::iq("in").rate(sdrkit_graph::RateRule::Exact(CHANNEL_RATE))],
            vec![Port::audio("out").rate_from(RateFrom::Fixed(CHANNEL_RATE / 3.0))],
        )
        .with_settings(VoiceSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.disc.reset();
        Ok(vec![Format::new(Kind::Audio, CHANNEL_RATE / 3.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.audio.clear();
        self.disc.process(x, &mut self.audio);
        self.low.clear();
        self.dec.process(&self.audio, &mut self.low);
        if self.listen {
            io.audio_out(0)
                .samples
                .extend(self.low.iter().map(|&v| v.clamp(-1.0, 1.0)));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match VoiceSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            VoiceSetting::Listen => self.listen = LISTEN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(VoiceSetting::Listen, Value::Bool(self.listen))])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Fields, tables and events.

/// The result fields of the ATIS stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtisFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Checked DSC calls of another format.
    #[field(label = "Other calls")]
    pub other_calls: i64,
    /// Stations in the table.
    #[field(label = "Stations")]
    pub stations: i64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Reports made.
    #[field(label = "Reports")]
    pub reports: i64,
}

/// A row of the ATIS message table. The row key is the place of the message in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtisMessageRow<'a> {
    /// When (wall-clock time, when known).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The ten digits.
    #[field(label = "Digits")]
    pub digits: &'a str,
    /// The call sign of a station heard.
    #[field(shared = keys::CALLSIGN, role = Sender)]
    pub callsign: &'a str,
    /// The nine digits after the 9.
    #[field(label = "MMSI")]
    pub mmsi: &'a str,
    /// The country of the MID.
    #[field(label = "Country")]
    pub country: &'a str,
    /// The name from the name table.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The check symbol agrees.
    #[field(label = "Check")]
    pub check_ok: bool,
    /// The identity starts with 9.
    #[field(label = "Starts with 9")]
    pub standard_z: bool,
    /// The tones were swapped.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// Symbols taken from the second copy or rebuilt.
    #[field(label = "Repaired")]
    pub repaired: i64,
    /// Symbols unreadable in both copies.
    #[field(label = "Erasures")]
    pub erasures: i64,
    /// The level over the floor.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The offset of the branch that found the message.
    #[field(label = "Branch", unit = "Hz")]
    pub branch_hz: Hertz,
}

/// The messages, newest first.
pub static MESSAGE_TABLE: Table<AtisMessageRow<'static>> =
    Table::new("messages", "Messages", MESSAGE_CAP)
        .role(sdrkit_core::TableRole::Messages)
        .label(&AtisMessageRow::NAME);

/// A row of the ATIS station table. The row key is the digits of the identity.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtisStationRow<'a> {
    /// The ten digits.
    #[field(label = "Digits")]
    pub digits: &'a str,
    /// The call sign of a station heard.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The nine digits after the 9.
    #[field(label = "MMSI")]
    pub mmsi: &'a str,
    /// The country of the MID.
    #[field(label = "Country")]
    pub country: &'a str,
    /// The name from the name table.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The best level over the floor.
    #[field(label = "Best level", unit = "dB")]
    pub best_level_db: f64,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The stations heard.
pub static STATION_TABLE: Table<AtisStationRow<'static>> =
    Table::new("stations", "Stations", STATION_CAP)
        .role(sdrkit_core::TableRole::Nodes)
        .label(&AtisStationRow::NAME);

/// The fields of the event of an ATIS message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtisMessageEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: Option<&'a str>,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The call sign of a station heard.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The nine digits after the 9.
    #[field(label = "MMSI")]
    pub mmsi: &'a str,
    /// The country of the MID.
    #[field(label = "Country")]
    pub country: &'a str,
    /// The check symbol agrees.
    #[field(label = "Check")]
    pub check_ok: bool,
}

/// An ATIS message was decoded.
pub static MESSAGE: EventKind<AtisMessageEvent<'static>> =
    EventKind::new("atis.message", "ATIS message");

impl Shaped for AtisMessage {
    type Row = AtisMessageRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AtisMessageRow {
            time: self.at.wall.map(Time),
            digits: &self.digits,
            callsign: &self.callsign,
            mmsi: &self.mmsi,
            country: &self.country,
            name: &self.name,
            check_ok: self.check_ok,
            standard_z: self.standard_z,
            inverted: self.inverted,
            repaired: self.repaired as i64,
            erasures: self.erasures as i64,
            level_db: self.level_db,
            branch_hz: Hertz(self.branch_hz),
        });
    }
}

impl Shaped for AtisStation {
    type Row = AtisStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AtisStationRow {
            digits: &self.digits,
            callsign: &self.callsign,
            mmsi: &self.mmsi,
            country: &self.country,
            name: &self.name,
            best_level_db: self.best_level_db,
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// What the display keeps across sessions: the messages, the stations and the totals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtisLists {
    /// The messages, oldest first.
    pub messages: LogRing<AtisMessage>,
    /// The stations, by their ten digits.
    pub stations: ObjectTable<String, AtisStation>,
    /// Messages with a good check since the start.
    pub good: u64,
    /// Messages with a failed check since the start.
    pub failed: u64,
    /// Checked calls of another format since the start.
    pub other: u64,
    /// The serial of the last message.
    pub serial: u64,
}

impl Default for AtisLists {
    fn default() -> Self {
        AtisLists {
            messages: LogRing::new(MESSAGE_CAP),
            stations: ObjectTable::new(STATION_CAP),
            good: 0,
            failed: 0,
            other: 0,
            serial: 0,
        }
    }
}

impl Shift for AtisLists {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
        self.stations.shift_times(by_s);
    }
}

impl Tables for AtisLists {
    const FORMAT: StateFormat = StateFormat::new("marine.atis_lists", 1);
    fn clear(&mut self) {
        self.messages.clear();
        self.stations.clear();
        self.good = 0;
        self.failed = 0;
        self.other = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.stations.adopt(saved.stations);
        self.good = saved.good;
        self.failed = saved.failed;
        self.other = saved.other;
        self.serial = self.serial.max(saved.serial);
    }
}

/// The name table.
pub const NAMES: Setting<field::Text> = Setting::new("names", "Name table", "", "")
    .help("Ship and station names by MMSI to import, one record per line.");
/// The wall-clock time of the first sample of a recording.
pub const TIME_BASE: Setting<field::Time> = Setting::new("time_base", "Time base", "", 0.0)
    .range(0.0, 1e11, 0.0)
    .help("UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.");

sdrkit_core::settings! {
    /// The settings of [`AtisDisplay`].
    pub enum DisplaySetting {
        /// The name table.
        Names = NAMES,
        /// The channel plan.
        Plan = PLAN,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

sdrkit_core::settings! {
    /// The settings of the ATIS stack.
    pub enum AtisSetting {
        /// Listen to the voice.
        Listen = LISTEN,
        /// The name table.
        Names = NAMES,
        /// The channel plan.
        Plan = PLAN,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

const STATION_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of("stations");

/// The arguments of the command `remove` of the ATIS stack.
#[derive(Arguments)]
pub struct AtisRemoveArgs<'a> {
    /// The row key of the station.
    #[arg(shared = STATION_ROW)]
    pub row: &'a str,
}

/// Forgets one station.
pub const REMOVE: CommandKind<AtisRemoveArgs<'static>> = objcmd::remove_kind();

sdrkit_core::commands! {
    /// The commands of [`AtisDisplay`].
    pub enum DisplayCommand {
        /// Forget every message and station.
        Clear = objcmd::CLEAR,
        /// Forget one station.
        Remove = REMOVE.command,
    }
}

/// The ATIS interpreter and results (`atis.display`).
pub struct AtisDisplay {
    p: Published,
    clock: Clock,
    host_clock: sdrkit_core::Clock,
    live: bool,
    lists: AtisLists,
    pending: Vec<BranchCall>,
    last_msg: f64,
    last_tone: f64,
    level: AtisLevel,
    freq_hz: f64,
    names: IdNames,
    names_text: String,
    plan: Plan,
    plan_text: String,
    time_base: f64,
    reports: u64,
    events: Vec<Event>,
}

impl Default for AtisDisplay {
    fn default() -> Self {
        let mut p =
            Published::new(AtisFields::SHAPE.fields).with_tables(&[&MESSAGE_TABLE, &STATION_TABLE]);
        p.results_mut().set_status(&Status {
            lock_state: Hearing::Searching.lock_state(),
            summary: "ATIS: no signal".into(),
            ..Status::default()
        });
        AtisDisplay {
            p,
            clock: Clock::default(),
            host_clock: sdrkit_core::Clock::fixed(0.0),
            live: false,
            lists: AtisLists::default(),
            pending: Vec::new(),
            last_msg: f64::NEG_INFINITY,
            last_tone: f64::NEG_INFINITY,
            level: AtisLevel::default(),
            freq_hz: 0.0,
            names: IdNames::new(IdKind::Mmsi),
            names_text: String::new(),
            plan: Plan::default(),
            plan_text: String::new(),
            time_base: 0.0,
            reports: 0,
            events: Vec::new(),
        }
    }
}

impl AtisDisplay {
    /// The lists.
    pub fn lists(&self) -> &AtisLists {
        &self.lists
    }

    /// Takes the copies of bursts that waited long enough (all of them with `flush`): one message per burst and transmitter.
    fn resolve(&mut self, now: f64, flush: bool) {
        self.pending.sort_by(|a, b| a.time.total_cmp(&b.time));
        while let Some(first) = self.pending.first() {
            if !flush && now - first.time < GROUP_S {
                break;
            }
            let t0 = first.time;
            let split = self
                .pending
                .iter()
                .position(|c| c.time - t0 >= GROUP_S)
                .unwrap_or(self.pending.len());
            let group: Vec<BranchCall> = self.pending.drain(..split).collect();
            // Checked copies win. Two checked copies with different numbers are two transmitters.
            let mut checked: Vec<&BranchCall> = group.iter().filter(|c| c.raw.check_ok).collect();
            checked.sort_by(|a, b| a.branch_hz.abs().total_cmp(&b.branch_hz.abs()));
            let mut seen: Vec<String> = Vec::new();
            let mut out = Vec::new();
            for c in checked {
                let d = symbols::digits(&c.raw.message.iter().take(5).copied().collect::<Vec<_>>());
                let key = format!("{:?}{d}", c.raw.format);
                if !seen.contains(&key) {
                    seen.push(key);
                    out.push(c.clone());
                }
            }
            if out.is_empty() {
                if let Some(f) = group
                    .iter()
                    .find(|c| c.raw.message.iter().any(|s| s.is_some()))
                {
                    out.push(f.clone());
                }
            }
            for c in out {
                self.message(c);
            }
        }
    }

    fn message(&mut self, c: BranchCall) {
        if c.raw.format != Some(121) {
            if c.raw.check_ok {
                self.lists.other += 1;
            }
            return;
        }
        let id = interpret(c.raw.format, &c.raw.message);
        if !c.raw.check_ok {
            self.lists.failed += 1;
            if id.digits.chars().all(|x| x == '~') {
                return;
            }
        } else {
            self.lists.good += 1;
            self.last_msg = c.time;
        }
        self.lists.serial += 1;
        let at = self.clock.stamp(c.time);
        let name = station_name(&self.names, &id.mmsi).unwrap_or_default();
        let level_db = db(self.level.level / self.level.floor.max(1e-30));
        let m = AtisMessage {
            serial: self.lists.serial,
            at,
            digits: id.digits.clone(),
            mmsi: id.mmsi.clone(),
            country: id.country.clone(),
            callsign: id.callsign.clone(),
            letter_note: id.letter_note.clone(),
            name: name.clone(),
            check_ok: c.raw.check_ok,
            standard_z: id.standard_z,
            repaired: c.raw.repaired,
            erasures: c.raw.erasures,
            inverted: c.raw.inverted,
            level_db,
            branch_hz: c.branch_hz,
        };
        let channel = self.plan.label_of(self.freq_hz).map(str::to_owned);
        self.events.push(MESSAGE.event(
            c.time,
            self.freq_hz,
            &AtisMessageEvent {
                source: &m.digits,
                target: None,
                channel: channel.as_deref(),
                family: Family::Vessel,
                callsign: &m.callsign,
                mmsi: &m.mmsi,
                country: &m.country,
                check_ok: m.check_ok,
            },
        ));
        self.p.results_mut().log(format!(
            "ATIS {} {} {}{}",
            m.digits,
            m.callsign,
            m.country,
            if m.check_ok { "" } else { " (check failed)" }
        ));
        if c.raw.check_ok {
            if let Ok(u) = self.lists.stations.upsert(&m.digits, at, || AtisStation {
                digits: m.digits.clone(),
                mmsi: m.mmsi.clone(),
                country: m.country.clone(),
                callsign: m.callsign.clone(),
                name: name.clone(),
                best_level_db: level_db,
            }) {
                u.row.best_level_db = u.row.best_level_db.max(level_db);
            }
        }
        self.lists.messages.push(m);
    }

    fn report(&mut self) {
        self.reports += 1;
        let now = self.clock.now();
        let state = if now - self.last_msg <= 60.0 {
            Hearing::Decoding
        } else if now - self.last_tone <= 30.0 {
            Hearing::Signal
        } else {
            Hearing::Searching
        };
        let l = self.level;
        let snr = (l.floor > 0.0).then(|| db(l.level / l.floor));
        let level = (l.level > 0.0).then(|| db(l.level));
        let lists = &self.lists;
        let p = &mut self.p;
        p.put(&AtisFields {
            frames_ok: lists.good as i64,
            frames_bad: lists.failed as i64,
            other_calls: lists.other as i64,
            stations: lists.stations.len() as i64,
            snr_db: snr,
            level_dbfs: level,
            reports: self.reports as i64,
        });
        shaped::log_rows(&lists.messages, &MESSAGE_TABLE, now, p);
        shaped::rows(&lists.stations, &STATION_TABLE, now, p);
        let n = lists.good;
        let summary = if n > 0 {
            let last = lists
                .messages
                .iter()
                .rev()
                .find(|m| m.check_ok)
                .map(|m| m.digits.clone())
                .unwrap_or_default();
            format!(
                "ATIS: {n} messages from {} station(s), last {last}",
                lists.stations.len()
            )
        } else if lists.failed > 0 {
            format!("ATIS: {} burst(s) with a failed check", lists.failed)
        } else if state == Hearing::Signal || l.level > 4.0 * l.floor {
            "ATIS: waiting for a message".into()
        } else {
            "ATIS: no signal".into()
        };
        p.results_mut().set_status(&Status {
            lock_state: state.lock_state(),
            snr_db: snr.map(|v| v as f32),
            level_dbfs: level.map(|v| v as f32),
            summary,
            ..Status::default()
        });
    }
}

impl Component for AtisDisplay {
    fn info(&self) -> ComponentInfo {
        let mut ins: Vec<Port> = (0..7)
            .map(|i| Port::typed::<BranchCall>(&format!("calls{i}")))
            .collect();
        ins.extend((0..7).map(|i| Port::typed::<AtisLevel>(&format!("lock{i}"))));
        ins.push(Port::typed::<AtisLevel>("level"));
        ComponentInfo::processor(
            "atis.display",
            ins,
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DisplaySetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A new input: its item times start again at 0, the clock runs on.
        self.clock.restart(self.live, self.host_clock.now());
        self.pending.clear();
        self.freq_hz = inputs.get(14).map_or(0.0, |f| f.tuned_hz);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, io: &mut Io) {
        for i in 0..7 {
            for c in io.items::<BranchCall>(i) {
                let mut c = c.value.clone();
                c.time = self.clock.at(c.time);
                self.pending.push(c);
            }
            if io
                .items::<AtisLevel>(7 + i)
                .last()
                .is_some_and(|l| l.value.locked)
            {
                self.last_tone = self.clock.now();
            }
        }
        if let Some(l) = io.items::<AtisLevel>(14).last() {
            self.level = l.value;
            self.clock.at(l.time);
        }
        if io.tick() {
            self.resolve(self.clock.now(), false);
            self.report();
        }
        io.messages_out(0).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DisplaySetting::Names => {
                let t = NAMES.read(id, v)?;
                self.names.import(t);
                t.clone_into(&mut self.names_text);
            }
            DisplaySetting::Plan => {
                let t = PLAN.read(id, v)?;
                self.plan = Plan::parse(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.plan_text);
            }
            DisplaySetting::TimeBase => {
                let t = TIME_BASE.read(id, v)?;
                self.time_base = t;
                self.clock.set_time_base((t > 0.0).then_some(t));
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DisplaySetting::Names, Value::Text(self.names_text.clone())),
            (DisplaySetting::Plan, Value::Text(self.plan_text.clone())),
            (DisplaySetting::TimeBase, Value::Time(self.time_base)),
        ])
    }
    fn commands(&self) -> &Commands {
        DisplayCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match DisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DisplayCommand::Clear => self.lists.clear(),
            DisplayCommand::Remove => {
                let table = self
                    .p
                    .results()
                    .decl()
                    .table_id(STATION_TABLE.key)
                    .expect("the declaration holds the station table");
                objcmd::remove(
                    &mut self.lists.stations,
                    table,
                    &mut self.p,
                    args,
                    &STATION_ROW,
                )?;
            }
        }
        self.report();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.clock.now())?;
        self.report();
        Ok(())
    }
}

/// The ATIS components.
pub fn register(r: &mut Registry) {
    r.add("atis.branch", || Box::new(Branch::new(0.0)));
    r.add("atis.level", || Box::new(LevelMeter::default()));
    r.add("atis.voice", || Box::new(Voice::default()));
    r.add("atis.display", || Box::new(AtisDisplay::default()));
}

/// The ATIS graph: the front end to 24 kHz, the voice, the level meter and the seven branches into the display.
///
/// # Errors
///
/// [`GraphError`] if a component refuses its constants.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let mut b = GraphBuilder::new()
        .add(
            "frontend",
            FixedRateFrontEnd::new(0.0, 16e3, CHANNEL_RATE).map_err(|error| {
                GraphError::Setting {
                    key: "frontend.offset".into(),
                    error,
                }
            })?,
        )
        .add("voice", Voice::default())
        .add("level", LevelMeter::default())
        .add("display", AtisDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "voice.in")
        .connect("frontend.out", "level.in")
        .connect("level.level", "display.level");
    for (i, off) in BRANCHES.iter().enumerate() {
        b = b
            .add(&format!("branch{i}"), Branch::new(*off))
            .connect("frontend.out", &format!("branch{i}.in"))
            .connect(&format!("branch{i}.calls"), &format!("display.calls{i}"))
            .connect(&format!("branch{i}.level"), &format!("display.lock{i}"));
    }
    Ok(b.output("display.events")
        .output("voice.out")
        .status_from("display")
        .clock(REPORT_S)
        .reconfigure_on_retune())
}

/// The ATIS stack graph: the settings routed to the components that own them, and the display's commands.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = StackGraph::of(crate::typed::shape("atis"), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "ATIS needs a sample rate of at least 48 ksps")
        .route(LISTEN.key(), &["voice.listen"])?
        .command("clear", sdrkit_graph::CommandTarget::of("display", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        );
    for s in DisplaySetting::SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("display.{k}")])?;
    }
    g.with_settings(&cfg.settings).build()
}

/// The ATIS graph with no settings applied.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder()?.catalog(catalog).build()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// Options of the ATIS test signal.
#[derive(Clone, Debug)]
pub struct AtisGen {
    /// The sample rate.
    pub rate: f64,
    /// Identities, one per transmission (ten digits each). An empty list produces silence.
    pub identities: Vec<String>,
    /// The voice before each burst, seconds (0 for none).
    pub voice_s: f64,
    /// The dot bits before the phasing (0 leaves them out).
    pub dots: usize,
    /// Phase modulation index for a full-scale tone (rad). Flat FM uses index x 1.7 kHz.
    pub index: f64,
    /// Flat FM instead of phase modulation.
    pub flat_fm: bool,
    /// The time without carrier after each burst, seconds.
    pub gap_s: f64,
    /// The offset of the channel in the stream, Hz.
    pub channel_hz: f64,
    /// The tones swapped (an inverted receiver).
    pub invert: bool,
}

impl Default for AtisGen {
    fn default() -> Self {
        AtisGen {
            rate: 48e3,
            identities: default_identities(),
            voice_s: 1.2,
            dots: 20,
            index: 1.0,
            flat_fm: false,
            gap_s: 1.0,
            channel_hz: 0.0,
            invert: false,
        }
    }
}

/// The default five identities: PC8075 / 244, DB1234 / 211, OT5678 / 205, 9 + 207072225, HB0042 / 269.
pub fn default_identities() -> Vec<String> {
    [
        ("PC8075", Some(244)),
        ("DB1234", Some(211)),
        ("OT5678", Some(205)),
        ("207072225", None),
        ("HB0042", Some(269)),
    ]
    .iter()
    .filter_map(|(c, m)| build(c, *m))
    .collect()
}

/// What a slot of the schedule sends.
#[derive(Clone, Debug)]
enum SlotKind {
    /// No carrier.
    Gap,
    /// Voice-like tones.
    Voice,
    /// The carrier alone.
    Carrier,
    /// The burst of these bits.
    Burst(Vec<u8>),
}

/// One slot of the schedule: start, duration, what.
type Slot = (f64, f64, SlotKind);

/// The ATIS generator: voice-like tones on narrow FM, then the burst on a phase-modulated carrier, then a gap. The carrier has
/// unit power.
pub struct AtisGenerator {
    o: AtisGen,
    sched: Vec<Slot>,
    total: f64,
    n: u64,
    phase: f64,
    audio: f64,
    prev_mod: f64,
}

impl AtisGenerator {
    /// From options.
    pub fn new(o: AtisGen) -> Self {
        let mut sched = Vec::new();
        let mut t = 0.0;
        for id in &o.identities {
            if o.voice_s > 0.0 {
                sched.push((t, o.voice_s, SlotKind::Voice));
                t += o.voice_s;
            }
            sched.push((t, 0.04, SlotKind::Carrier));
            t += 0.04;
            let mut bits = burst_bits(id, o.dots);
            if o.invert {
                for b in &mut bits {
                    *b ^= 1;
                }
            }
            let d = bits.len() as f64 / BAUD;
            sched.push((t, d, SlotKind::Burst(bits)));
            t += d;
            sched.push((t, o.gap_s, SlotKind::Gap));
            t += o.gap_s;
        }
        AtisGenerator {
            o,
            sched,
            total: t,
            n: 0,
            phase: 0.0,
            audio: 0.0,
            prev_mod: 0.0,
        }
    }
    /// One cycle of the identities, seconds.
    pub fn cycle_s(&self) -> f64 {
        self.total
    }
    /// The number of transmissions per cycle.
    pub fn transmissions(&self) -> usize {
        self.o.identities.len()
    }
}

impl Generator for AtisGenerator {
    /// The station is made at 0 Hz; the framework places it on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.sched.is_empty() || self.total <= 0.0 {
            out.fill(Complex32::default());
            self.n += out.len() as u64;
            return out.len();
        }
        let rate = self.o.rate;
        for s in out.iter_mut() {
            let t = (self.n as f64 / rate).rem_euclid(self.total);
            self.n += 1;
            let k = self.sched.iter().rposition(|x| x.0 <= t).unwrap_or(0);
            let (t0, dur, kind) = &self.sched[k];
            let local = t - t0;
            let mut carrier = true;
            let mut f_inst = 0.0;
            match kind {
                SlotKind::Gap => carrier = false,
                SlotKind::Voice => {
                    // Voice-like tones (flat FM, about 2.5 kHz peak).
                    let m = 0.5 * (TAU * 450.0 * local).sin()
                        + 0.3 * (TAU * 900.0 * local).sin()
                        + 0.2 * (TAU * 1350.0 * local).sin();
                    f_inst = 2500.0 * m;
                }
                SlotKind::Carrier => {}
                SlotKind::Burst(bits) => {
                    let i = ((local / dur * bits.len() as f64) as usize).min(bits.len() - 1);
                    let af = if bits[i] == 1 { 1300.0 } else { 2100.0 };
                    self.audio += TAU * af / rate;
                    let m = self.audio.sin();
                    if self.o.flat_fm {
                        f_inst = self.o.index * 1700.0 * m;
                    } else {
                        // Phase modulation: the phase is index x m(t). Its frequency is the derivative.
                        let ph = self.o.index * m;
                        f_inst = (ph - self.prev_mod) * rate / TAU;
                        self.prev_mod = ph;
                    }
                }
            }
            if !matches!(kind, SlotKind::Burst(_)) {
                self.prev_mod = 0.0;
            }
            self.phase = (self.phase + TAU * (f_inst + self.o.channel_hz) / rate).rem_euclid(TAU);
            *s = if carrier {
                Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32)
            } else {
                Complex32::default()
            };
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(signal_info())
    }
}

/// What the ATIS test signal states: a carrier of unit power, its C/N measured in 12.5 kHz, 1200 bits a second.
pub fn signal_info() -> SignalInfo {
    SignalInfo::new(12_500.0).with_bit_rate(BAUD)
}

/// The generator the descriptor makes.
pub fn from_config(cfg: &GenConfig) -> Box<dyn Generator> {
    Box::new(AtisGenerator::new(AtisGen {
        rate: cfg.rate_hz,
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_facts() {
        let d = build("PC8075", Some(244)).unwrap();
        assert_eq!(d, "9244038075");
        assert_eq!(symbols::number_symbols(&d), vec![92, 44, 3, 80, 75]);
        assert_eq!(symbols::ecc(121, &[92, 44, 3, 80, 75], 127), 110);
        assert_eq!(build("207072225", None).unwrap(), "9207072225");
        assert!(build("P1234", Some(244)).is_none());
        assert!(build("PC80X5", Some(244)).is_none());
        assert_eq!(default_identities().len(), 5);
        let id = interpret(
            Some(121),
            &[Some(92), Some(44), Some(3), Some(80), Some(75)],
        );
        assert_eq!(
            (
                id.callsign.as_str(),
                id.mmsi.as_str(),
                id.country.as_str(),
                id.standard_z
            ),
            ("PC8075", "244038075", "Netherlands", true)
        );
        let id = interpret(Some(121), &[Some(92), Some(44), None, Some(80), Some(75)]);
        assert!(id.partial && id.digits == "9244~~8075");
        assert!(!interpret(Some(120), &[Some(92); 5]).valid);
        assert!(!interpret(Some(121), &[Some(92); 3]).valid);
        let id = interpret(
            Some(121),
            &[Some(12), Some(34), Some(56), Some(78), Some(90)],
        );
        assert!(id.valid && !id.standard_z);
        let id = interpret(
            Some(121),
            &[Some(93), Some(66), Some(3), Some(80), Some(75)],
        );
        assert_eq!(id.callsign, "?C8075");
        assert_eq!(
            interpret(Some(121), &[Some(92), Some(7), Some(7), Some(22), Some(25)]).callsign,
            "LG2225"
        );
        // All four variants (with and without dots, both polarities) decode to one checked call of format 121.
        for dots in [20, 0] {
            for inv in [false, true] {
                let mut b = burst_bits(&d, dots);
                b.extend((0..20).map(|i| (i % 2) as u8));
                let soft: Vec<f32> = b
                    .iter()
                    .map(|&x| if (x == 1) != inv { 1.0 } else { -1.0 })
                    .collect();
                let mut dec = DscDecoder::new(inv);
                dec.push(&soft);
                let c = dec.take_calls();
                assert_eq!(c.len(), 1);
                assert!(c[0].check_ok && c[0].format == Some(121));
                assert_eq!(interpret(c[0].format, &c[0].message).digits, d);
            }
        }
    }

    #[test]
    fn a_generator_without_identities_sends_silence() {
        let mut g = AtisGenerator::new(AtisGen {
            identities: Vec::new(),
            ..Default::default()
        });
        let mut x = vec![Complex32::new(1.0, 1.0); 480];
        assert_eq!(g.generate(&mut x), 480);
        assert!(x.iter().all(|v| *v == Complex32::default()));
    }
}
