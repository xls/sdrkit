//! Argos (`argos`): PTT-A2 uplink bursts (decoded, other modulations listed) and the A-DCS downlink (HDLC frames).
//!
//! Standard: Argos user documentation (PTT-A2), ISO/IEC 13239 (HDLC).

pub mod a2;
pub mod downlink;
pub mod gen;

use crate::display::{set_series, slope_of, Clock};
use crate::modes::sarsat::baseband;
use crate::modes::sarsat::fgb::{self, FgbFinder};
use crate::typed::{
    ArgosBurstRow, ArgosDownlinkFrameEvent, ArgosFields, ArgosFrameRow, ArgosPlatformRow,
    ArgosPttEvent, ARGOS_BURSTS, ARGOS_DOPPLER, ARGOS_DOWNLINK_FRAME, ARGOS_FRAMES,
    ARGOS_PLATFORMS, ARGOS_PLATFORM_ROW, ARGOS_PTT, ARGOS_REMOVE,
};
use downlink::{Coding, DlFrame, Downlink};
use sdrkit_algo_dsp::channel::Channelizer;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Schema;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState,
    Receiver, ResultsDecl, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_core::{Command, Record};
use sdrkit_graph::{
    no_values, values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Item, Kind, Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "argos";
/// The stack name.
pub const NAME: &str = "Argos";
/// The least input rate.
pub const MIN_RATE: f64 = 128e3;
/// Seconds between two reports.
pub const REPORT_S: f64 = 0.25;
/// The most bursts the burst log keeps.
pub const BURST_CAP: usize = 300;
/// The most platforms the platform table keeps.
pub const PLATFORM_CAP: usize = 200;
/// The most downlink frames the frame log keeps.
pub const FRAME_CAP: usize = 300;
/// The most points of the Doppler track.
pub const TRACK_CAP: usize = 1200;
/// The most burst carriers a platform keeps for its drift.
pub const CARRIERS_CAP: usize = 64;
/// Something decoded this recently (seconds) keeps the lock state locked.
pub const RECENT_S: f64 = 300.0;
/// Above this dial the automatic link choice is the downlink, Hz.
pub const DOWNLINK_ABOVE_HZ: f64 = 430e6;

/// What the receiver passes to the display.
#[derive(Clone, Debug)]
pub enum ArgosItem {
    /// An uplink burst.
    Burst(Box<BurstRow>),
    /// A downlink frame with a good FCS.
    Frame(DlFrame),
    /// The downlink's state at a tick.
    Downlink {
        /// The carrier loop holds.
        locked: bool,
        /// The carrier, Hz from the dial.
        carrier_hz: f64,
        /// The carrier line over the noise, dB.
        line_db: f64,
        /// Bits received.
        bits: u64,
        /// Frames with a bad FCS in the coding in use.
        fcs_fail: u64,
        /// The coding that shows flags.
        coding: Option<Coding>,
        /// (signal time, carrier Hz) since the last tick.
        track: Vec<(f64, f64)>,
    },
    /// The input level at a tick, dBFS.
    Level(f64),
}
impl Item for ArgosItem {
    const KIND: &'static str = "satcom.argos_item";
}

/// One uplink burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BurstRow {
    /// When it was heard.
    pub at: Stamp,
    /// 1 A2 decoded, 2 carrier and modulation but not A2.
    pub kind: u8,
    /// The carrier, Hz from the dial.
    pub carrier_hz: f64,
    /// C/N in 2.4 kHz, dB.
    pub snr_db: f64,
    /// The level, dBFS.
    pub level_dbfs: f64,
    /// The unmodulated carrier, ms.
    pub carrier_ms: f64,
    /// The whole burst, ms.
    pub total_ms: f64,
    /// The quality of the coherent soft bits (1 without noise, 0.64 for noise alone).
    pub quality: f64,
    /// The A2 message, when the burst is A2.
    pub message: Option<a2::A2>,
}

impl Shift for BurstRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

/// The longest uplink burst as the finder sees it, seconds: 160 ms of carrier and 304 bits, a start up to 3 frames early
/// and the 7 frames that end a burst.
const MAX_UPLINK_BURST_S: f64 = 0.160 + 304.0 / a2::BIT_RATE + 10.0 * 4096.0 / fgb::RATE + 0.1;

/// The receiver: the uplink burst finder and the downlink, on the same input.
pub struct ArgosRx {
    rate: f64,
    mix: Option<Channelizer>,
    finder: FgbFinder,
    dl: Option<Downlink>,
    tmp: Vec<Complex32>,
    bursts: Vec<fgb::FgbBurst>,
    power: f64,
    count: u64,
    next_index: u64,
}

impl Default for ArgosRx {
    fn default() -> Self {
        let mut f = FgbFinder::new();
        f.decode = false;
        f.keep = true;
        f.run_line = true;
        f.max_burst_s = MAX_UPLINK_BURST_S;
        ArgosRx {
            rate: 0.0,
            mix: None,
            finder: f,
            dl: None,
            tmp: Vec::new(),
            bursts: Vec::new(),
            power: 0.0,
            count: 0,
            next_index: 0,
        }
    }
}

impl ArgosRx {
    fn start(&mut self, f: &Format) -> Result<(), ConfigError> {
        self.mix = Some(baseband(f.rate, f.signal_offset_hz(), fgb::RATE)?);
        self.dl = Some(Downlink::new(f.rate, f.signal_offset_hz())?);
        self.finder.reset();
        self.finder.index = 0;
        Ok(())
    }

    fn uplink(&mut self, iq: &[Complex32], out: &mut Vec<Timed<ArgosItem>>) {
        let Some(mix) = &mut self.mix else {
            return;
        };
        self.tmp.clear();
        mix.process(iq, &mut self.tmp);
        self.bursts.clear();
        self.finder.process(&self.tmp, &mut self.bursts);
        for b in self.bursts.drain(..) {
            let Some((x, start)) = &b.samples else {
                continue;
            };
            let d = a2::demodulate(x, b.carrier_hz, *start as usize);
            let t = b.start as f64 / fgb::RATE;
            let row = BurstRow {
                at: Stamp::at(t),
                kind: if d.message.is_some() { 1 } else { 2 },
                carrier_hz: d.carrier_hz,
                // C/N in 2.4 kHz from the line over the noise in a 31.25 Hz bin.
                snr_db: b.cn_db - power_to_db(2400.0 / (fgb::RATE / 4096.0)),
                level_dbfs: b.level_dbfs,
                carrier_ms: d.carrier_ms,
                total_ms: d.total_ms,
                quality: d.quality,
                message: d.message,
            };
            out.push(Timed {
                time: t,
                value: ArgosItem::Burst(Box::new(row)),
            });
        }
    }
}

impl Decoder for ArgosRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.argos_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<ArgosItem>("out").rate_from(RateFrom::Fixed(0.0))]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.next_index = 0;
        self.start(f)?;
        let mut o = Format::new(Kind::typed::<ArgosItem>(), 0.0);
        o.centre_hz = f.centre_hz;
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o])
    }
    fn retune(&mut self, f: &Format) {
        // A refusal keeps the old chain, which then finds nothing at the new dial.
        let _ = self.start(f);
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        for s in iq {
            self.power += s.norm_sqr() as f64;
        }
        self.count += iq.len() as u64;
        self.next_index = first + iq.len() as u64;
        let items = out.items::<ArgosItem>(0);
        self.uplink(iq, items);
        if let Some(dl) = &mut self.dl {
            dl.process(iq, 0.0);
            for f in dl.frames.drain(..) {
                items.push(Timed {
                    time: f.time,
                    value: ArgosItem::Frame(f),
                });
            }
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        let t = self.next_index as f64 / self.rate.max(1.0);
        let l = if self.count > 0 {
            power_to_db(self.power / self.count as f64)
        } else {
            -200.0
        };
        self.power = 0.0;
        self.count = 0;
        let items = out.items::<ArgosItem>(0);
        items.push(Timed {
            time: t,
            value: ArgosItem::Level(l),
        });
        if let Some(dl) = &mut self.dl {
            items.push(Timed {
                time: t,
                value: ArgosItem::Downlink {
                    locked: dl.locked,
                    carrier_hz: dl.carrier_hz(),
                    line_db: dl.line_db,
                    bits: dl.bits,
                    fcs_fail: dl.fcs_fail,
                    coding: dl.coding(),
                    track: std::mem::take(&mut dl.track),
                },
            });
        }
    }
}

sdrkit_core::choice_enum! {
    /// The link the display shows.
    pub enum Link {
        /// From the dial: the downlink above 430 MHz.
        Auto = ("auto", "Automatic (from the dial)"),
        /// The platforms' uplink.
        Uplink = ("uplink", "Uplink"),
        /// The satellite's downlink.
        Downlink = ("downlink", "Downlink"),
    }
}

sdrkit_core::choice_enum! {
    /// The modulation of an uplink burst.
    pub enum BurstKind {
        /// An A2 message.
        A2 = ("a2", "A2"),
        /// Another modulation.
        Other = ("other", "Other"),
    }
}

/// The link the display shows (setting).
pub const LINK_SETTING: Setting<field::Choice<Link>> =
    Setting::choice("link", "Link", Link::Auto).help("Automatic: the downlink above 430 MHz.");

sdrkit_core::settings! {
    /// The settings of [`ArgosDisplay`].
    pub enum ArgosDisplaySetting {
        /// Link.
        Link = LINK_SETTING,
    }
}

sdrkit_core::settings! {
    /// The settings of the Argos stack. Noise, offset, drift and clock error of the test signal come from the standard `gen_*`
    /// settings.
    pub enum ArgosSetting {
        /// Link.
        Link = LINK_SETTING,
        /// Downlink of the test signal.
        GenDownlink = gen::GEN_DOWNLINK,
        /// Gap of the test signal.
        GenGapS = gen::GEN_GAP_S,
        /// Step of the test signal.
        GenStepHz = gen::GEN_STEP_HZ,
        /// Other modulation of the test signal.
        GenOther = gen::GEN_OTHER,
        /// Largest Doppler of the test signal.
        GenPassDopplerHz = gen::GEN_PASS_DOPPLER_HZ,
        /// Doppler time constant of the test signal.
        GenPassTauS = gen::GEN_PASS_TAU_S,
        /// Closest approach of the test signal.
        GenPassTcaS = gen::GEN_PASS_TCA_S,
        /// BPSK downlink of the test signal.
        GenBpsk = gen::GEN_BPSK,
        /// NRZI downlink of the test signal.
        GenNrzi = gen::GEN_NRZI,
    }
}

/// What the display publishes.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(ArgosFields::SHAPE.fields)
                .with_table(&ARGOS_BURSTS)
                .with_table(&ARGOS_PLATFORMS)
                .with_table(&ARGOS_FRAMES)
                .series(
                    ARGOS_DOPPLER.key,
                    ARGOS_DOPPLER.label,
                    ARGOS_DOPPLER.x_unit,
                    ARGOS_DOPPLER.y_unit,
                ),
        )
    })
    .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Hexadecimal digits as bytes, the first digit the high half of the first byte. An odd count ends with a half byte.
fn hex_bytes(hex: &str) -> Arc<[u8]> {
    let mut out = Vec::with_capacity(hex.len().div_ceil(2));
    let mut text = String::with_capacity(hex.len() + 1);
    text.push_str(hex);
    if text.len() % 2 == 1 {
        text.push('0');
    }
    if sdrkit_core::base64::unhex_into(&text, &mut out).is_err() {
        out.clear();
    }
    Arc::from(out)
}

impl Shaped for BurstRow {
    type Row = ArgosBurstRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let m = self.message.as_ref();
        out.put(&ArgosBurstRow {
            at: Seconds(self.at.t),
            heard_at: self.at.wall.map(Time),
            kind: if self.kind == 1 {
                BurstKind::A2
            } else {
                BurstKind::Other
            },
            offset_hz: Hertz(self.carrier_hz),
            snr_db: self.snr_db,
            carrier_ms: self.carrier_ms,
            total_ms: self.total_ms,
            quality: self.quality,
            platform: m.map(|m| m.id_hex.as_str()),
            data_bits: m.map(|m| m.data_bits as i64),
            data: m.map(|m| hex_bytes(&m.data_hex)),
        });
    }
}

/// One platform: its last data and the carrier of its recent bursts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Platform {
    /// The id, 7 hex digits.
    pub id_hex: String,
    /// The last user data in hexadecimal digits.
    pub last_data: String,
    /// (signal time, carrier Hz) of each recent burst, oldest first.
    pub carriers: Vec<(f64, f64)>,
}

impl Platform {
    /// The least-squares drift of the carrier over the recent bursts, Hz/s (0 with fewer than two bursts).
    pub fn drift_hz_s(&self) -> f64 {
        if self.carriers.len() < 2 {
            0.0
        } else {
            slope_of(&self.carriers)
        }
    }
}

impl Shift for Platform {
    fn shift_times(&mut self, by_s: f64) {
        for c in &mut self.carriers {
            c.0 += by_s;
        }
    }
}

impl Shaped for Platform {
    type Row = ArgosPlatformRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ArgosPlatformRow {
            platform: &self.id_hex,
            data: hex_bytes(&self.last_data),
            drift_hz_s: self.drift_hz_s(),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
}

/// One downlink frame of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// When it ended.
    pub at: Stamp,
    /// The coding it came in.
    pub coding: Coding,
    /// Its length without the FCS.
    pub bytes: usize,
    /// Its bytes.
    pub data: Arc<[u8]>,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Shaped for FrameRow {
    type Row = ArgosFrameRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ArgosFrameRow {
            at: Seconds(self.at.t),
            coding: self.coding,
            bytes: self.bytes as i64,
            frame: self.data.clone(),
        });
    }
}

/// One point of the downlink's Doppler track.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackPoint {
    /// Signal time, seconds.
    pub t: f64,
    /// The carrier from the dial, Hz.
    pub hz: f64,
}

impl Shift for TrackPoint {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// What the display keeps across sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// The uplink bursts, newest last.
    pub bursts: LogRing<BurstRow>,
    /// The platforms, by id.
    pub platforms: ObjectTable<String, Platform>,
    /// The downlink frames, newest last.
    pub frames: LogRing<FrameRow>,
    /// The downlink's Doppler track.
    pub track: LogRing<TrackPoint>,
    /// A2 messages decoded.
    pub up_good: u64,
    /// Uplink bursts that were not A2.
    pub up_bad: u64,
    /// Downlink frames with a good FCS.
    pub dl_good: u64,
    /// When something last decoded.
    pub last_decoded: Option<f64>,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            bursts: LogRing::new(BURST_CAP),
            platforms: ObjectTable::new(PLATFORM_CAP),
            frames: LogRing::new(FRAME_CAP),
            track: LogRing::new(TRACK_CAP),
            up_good: 0,
            up_bad: 0,
            dl_good: 0,
            last_decoded: None,
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.bursts.shift_times(by_s);
        self.platforms.shift_times(by_s);
        self.frames.shift_times(by_s);
        self.track.shift_times(by_s);
        if let Some(t) = &mut self.last_decoded {
            *t += by_s;
        }
    }
}

impl Tables for Heard {
    const FORMAT: StateFormat = StateFormat::new("satcom.argos", 1);
    fn clear(&mut self) {
        self.bursts.clear();
        self.platforms.clear();
        self.frames.clear();
        self.track.clear();
        self.up_good = 0;
        self.up_bad = 0;
        self.dl_good = 0;
        self.last_decoded = None;
    }
    fn adopt(&mut self, saved: Self) {
        self.bursts.adopt(saved.bursts);
        self.platforms.adopt(saved.platforms);
        self.frames.adopt(saved.frames);
        self.track.adopt(saved.track);
        self.up_good = saved.up_good;
        self.up_bad = saved.up_bad;
        self.dl_good = saved.dl_good;
        self.last_decoded = saved.last_decoded;
    }
}

/// The downlink's state after the last tick.
#[derive(Clone, Debug, Default)]
struct DownlinkState {
    locked: bool,
    carrier_hz: f64,
    line_db: f64,
    bits: u64,
    fcs_fail: u64,
    coding: Option<Coding>,
}

/// The display: the burst log, the platforms, the downlink, the events.
pub struct ArgosDisplay {
    clock: Clock,
    heard: Heard,
    link: Link,
    tuned_hz: f64,
    level: f64,
    samples: bool,
    dl: DownlinkState,
    live: bool,
    host_clock: sdrkit_core::Clock,
    /// The tables changed since the last report.
    dirty: bool,
}

impl Default for ArgosDisplay {
    fn default() -> Self {
        ArgosDisplay {
            clock: Clock::default(),
            heard: Heard::default(),
            link: Link::Auto,
            tuned_hz: 0.0,
            level: -200.0,
            samples: false,
            dl: DownlinkState::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            dirty: true,
        }
    }
}

impl ArgosDisplay {
    fn downlink_mode(&self) -> bool {
        match self.link {
            Link::Uplink => false,
            Link::Downlink => true,
            Link::Auto => self.tuned_hz > DOWNLINK_ABOVE_HZ,
        }
    }

    fn burst(&mut self, mut b: BurstRow, t: f64, out: &mut Outbox) {
        b.at = self.clock.stamp(t);
        self.dirty = true;
        match &b.message {
            Some(m) => {
                self.heard.up_good += 1;
                self.heard.last_decoded = Some(t);
                if let Ok(up) = self.heard.platforms.upsert(&m.id_hex, b.at, || Platform {
                    id_hex: m.id_hex.clone(),
                    ..Default::default()
                }) {
                    let p = up.row;
                    p.last_data.clone_from(&m.data_hex);
                    p.carriers.push((t, b.carrier_hz));
                    if p.carriers.len() > CARRIERS_CAP {
                        p.carriers.remove(0);
                    }
                }
                out.messages(0).push(ARGOS_PTT.event(
                    t,
                    self.tuned_hz + b.carrier_hz,
                    &ArgosPttEvent {
                        source: &m.id_hex,
                        text: &m.data_hex,
                        snr_db: Some(b.snr_db).filter(|s| s.is_finite()),
                        family: Family::Sensor,
                        platform: &m.id_hex,
                        data_bits: m.data_bits as i64,
                        offset_hz: Hertz(b.carrier_hz),
                        quality: b.quality,
                        data: hex_bytes(&m.data_hex),
                    },
                ));
            }
            None => self.heard.up_bad += 1,
        }
        self.heard.bursts.push(b);
    }
}

sdrkit_core::commands! {
    /// The commands of [`ArgosDisplay`].
    pub enum ArgosDisplayCommand {
        /// Forgets one object.
        Remove = ARGOS_REMOVE.command,
    }
}

impl Decoder for ArgosDisplay {
    type Input = Timed<ArgosItem>;
    fn name(&self) -> &'static str {
        "satcom.argos_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        ArgosDisplaySetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match ArgosDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ArgosDisplaySetting::Link => self.link = LINK_SETTING.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(ArgosDisplaySetting::Link, Value::Choice(self.link.index()))])
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.tuned_hz = f.tuned_hz;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.tuned_hz = f.tuned_hz;
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, x: &[Timed<ArgosItem>], _first: u64, out: &mut Outbox) {
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                ArgosItem::Level(l) => {
                    self.level = *l;
                    self.samples = true;
                }
                ArgosItem::Burst(b) => self.burst((**b).clone(), t, out),
                ArgosItem::Frame(f) => {
                    self.heard.dl_good += 1;
                    self.heard.last_decoded = Some(t);
                    out.messages(0).push(ARGOS_DOWNLINK_FRAME.event(
                        t,
                        self.tuned_hz + self.dl.carrier_hz,
                        &ArgosDownlinkFrameEvent {
                            family: Family::Satellite,
                            coding: f.coding,
                            bytes: f.bytes as i64,
                            frame: f.data.clone(),
                        },
                    ));
                    self.dirty = true;
                    self.heard.frames.push(FrameRow {
                        at: self.clock.stamp(t),
                        coding: f.coding,
                        bytes: f.bytes,
                        data: f.data.clone(),
                    });
                }
                ArgosItem::Downlink {
                    locked,
                    carrier_hz,
                    line_db,
                    bits,
                    fcs_fail,
                    coding,
                    track,
                } => {
                    self.samples = true;
                    self.dl = DownlinkState {
                        locked: *locked,
                        carrier_hz: *carrier_hz,
                        line_db: *line_db,
                        bits: *bits,
                        fcs_fail: *fcs_fail,
                        coding: *coding,
                    };
                    for &(tt, hz) in track {
                        let t = self.clock.at(tt);
                        self.heard.track.push(TrackPoint { t, hz });
                    }
                }
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let dlm = self.downlink_mode();
        let h = &self.heard;
        let recent = h.last_decoded.is_some_and(|t| now - t < RECENT_S);
        let state = if !self.samples {
            0
        } else if recent {
            2
        } else {
            1
        };
        let last = h.bursts.newest_first().next();
        let mut summary = String::new();
        let _ = if !self.samples {
            write!(summary, "Argos: waiting for samples")
        } else if dlm {
            if self.dl.locked {
                write!(
                    summary,
                    "Argos downlink: carrier {:+.0} Hz, {} frames",
                    self.dl.carrier_hz, h.dl_good
                )
            } else {
                write!(summary, "Argos downlink: searching for the carrier")
            }
        } else {
            match last {
                Some(b) if now - b.at.t < 30.0 => match &b.message {
                    Some(m) => write!(summary, "Argos PTT {}, {} data bits", m.id_hex, m.data_bits),
                    None => write!(summary, "Argos burst {:+.0} Hz (not A2)", b.carrier_hz),
                },
                _ => write!(summary, "Argos: listening ({:.0} dBFS)", self.level),
            }
        };
        let (good, bad) = if dlm {
            (h.dl_good, self.dl.fcs_fail)
        } else {
            (h.up_good, h.up_bad)
        };
        p.put(&ArgosFields {
            frames_ok: good as i64,
            frames_bad: bad as i64,
            link: if dlm { Link::Downlink } else { Link::Uplink },
            locked: self.dl.locked,
            carrier_line_db: self.dl.line_db,
            downlink_bits: self.dl.bits as i64,
            coding: self.dl.coding,
        });
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&h.bursts, &ARGOS_BURSTS, now, p);
            shaped::rows(&h.platforms, &ARGOS_PLATFORMS, now, p);
            shaped::log_rows(&h.frames, &ARGOS_FRAMES, now, p);
        }
        let r = p.results_mut();
        set_series(r, ARGOS_DOPPLER.key, true, || {
            h.track.iter().map(|q| [q.t as f32, q.hz as f32])
        });
        let lock_state = LockState::from_level(state);
        let offset = if dlm {
            Some(self.dl.carrier_hz)
        } else {
            last.map(|b| b.carrier_hz)
        };
        let snr = if dlm {
            Some(self.dl.line_db as f32)
        } else {
            last.map(|b| b.snr_db as f32)
        };
        let level = Some(self.level as f32);
        r.update_status(|st| {
            let same = st.lock_state == lock_state
                && st.offset_hz == offset
                && st.snr_db == snr
                && st.level_dbfs == level
                && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.offset_hz = offset;
                st.snr_db = snr;
                st.level_dbfs = level;
                st.summary = summary;
            }
            !same
        });
    }
    fn commands(&self) -> &'static [&'static Command] {
        ArgosDisplayCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match ArgosDisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ArgosDisplayCommand::Remove => {
                let p = self.results();
                crate::display::remove(
                    &mut self.heard.platforms,
                    &ARGOS_PLATFORMS,
                    &p,
                    args,
                    &ARGOS_PLATFORM_ROW,
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
    r.add("satcom.argos_receiver", || {
        Box::new(DecoderComponent::new(ArgosRx::default()))
    });
    r.add("satcom.argos_display", || {
        Box::new(DecoderComponent::new(ArgosDisplay::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("rx", DecoderComponent::new(ArgosRx::default()))
        .add("display", DecoderComponent::new(ArgosDisplay::default()))
        .input("rx")
        .connect("rx.out", "display")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "Argos needs a sample rate of at least 128 ksps")
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .route("link", &["display.link"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::ArgosGenerator::from_config(cfg)))
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Argos PTT-A2 platform bursts (other modulations listed) and the A-DCS downlink's HDLC frames. Receive only.",
    tuning: Tuning {
        min_hz: 401.5e6,
        max_hz: 466.1e6,
        default_hz: 401.65e6,
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
    settings: ArgosSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Argos],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

/// The verification of the stack: the uplink platforms, and the downlink through a pass.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut up = VerifyCase::new("test signal, uplink", o, 0.0);
    up.settings.push(CaseSetting::new(&gen::GEN_GAP_S, 1.0));
    up.seconds = up.seconds.max(6.0);
    up.expect_audio = Some(false);
    up.check = Some(check_decoded);
    let mut down = VerifyCase::new("test signal, downlink", o, 0.0);
    down.settings
        .push(CaseSetting::new(&gen::GEN_DOWNLINK, true));
    down.settings
        .push(CaseSetting::new(&gen::GEN_PASS_TCA_S, 3.0));
    down.freq_hz = Some(465.9875e6);
    down.seconds = down.seconds.max(8.0);
    down.expect_audio = Some(false);
    down.check = Some(check_decoded);
    vec![up, down]
}

/// At least one message or frame decoded.
fn check_decoded(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let ok = crate::field(r, ArgosFields::FRAMES_OK.key) as i64;
    vec![CheckLine::new(
        "decoded",
        ok >= 1,
        format!("{ok} messages or frames decoded"),
    )]
}
