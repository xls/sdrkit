//! Marine VHF voice (`marinevhf`): narrow-FM voice on every marine channel in the captured span. A finder tracks the carriers on
//! the 25 kHz raster, a bank demodulates each one, a player chooses what plays (scan and priority channel), and a log keeps
//! the calls. The host gives the channel plan.

use crate::report::Clock;
use crate::vhfplan::{Plan, PLAN};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{EventKind, Fields, Hertz, Seconds, Table, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, HostRequest, LockState, Record, Results,
    Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError,
    Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Kind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_channels::{
    Blocks, Call, CallBook, Carrier, Carriers, Chan, ChanBlock, Finder, FinderConfig, PlayState,
    Player, ScanPlan, ScanState,
};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

pub use sdrkit_proto_channels::{AUDIO_RATE, BLOCK};
/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The raster.
pub const RASTER: f64 = 25e3;
/// The tune offset.
pub const TUNE_OFFSET_HZ: f64 = 12.5e3;

/// The channel to play.
pub const CHANNEL_HZ: Setting<field::Hz> = Setting::new("channel_hz", "Channel", "Hz", 0.0)
    .range(0.0, 1e9, 0.0)
    .zero_means("Automatic")
    .help("The frequency of the channel to play; either half of a duplex pair of the plan plays both. 0 plays the channel that is active.");
/// The squelch.
pub const SQUELCH: Setting<field::F64> =
    Setting::new("squelch", "Squelch", "%", 50.0).range(0.0, 100.0, 1.0);
/// De-emphasis.
pub const DEEMPHASIS: Setting<field::Bool> = Setting::new("deemphasis", "De-emphasis", "", true);
/// The priority channel.
pub const PRIORITY_HZ: Setting<field::Hz> = Setting::new("priority_hz", "Priority channel", "Hz", 156.8e6)
    .range(0.0, 1e9, 0.0)
    .zero_means("None")
    .help("A channel that takes over while it carries a call. The default is 156.8 MHz, the distress, safety and calling frequency of the ITU Radio Regulations (Appendix 15).");
/// The scan.
pub const SCAN: Setting<field::Bool> = Setting::new("scan", "Scan", "", false);
/// The scan list.
pub const SCAN_HZ: Setting<field::Text> = Setting::new("scan_hz", "Scan list", "", "")
    .help("Frequencies in Hz separated by spaces, commas or semicolons; a frequency of a duplex pair of the plan scans both halves. Empty scans every voice channel of the plan.");
/// The scan dwell.
pub const SCAN_DWELL_S: Setting<field::Duration> =
    Setting::new("scan_dwell_s", "Dwell", "s", 0.4).range(0.1, 3.0, 0.0);
/// The scan hang time.
pub const SCAN_HANG_S: Setting<field::Duration> =
    Setting::new("scan_hang_s", "Hang", "s", 2.0).range(0.0, 6.0, 0.0);
/// The most carriers tracked.
pub const CARRIERS_SETTING: Setting<field::I64> =
    Setting::new("carriers", "Carriers", "", 10).range(1.0, 24.0, 1.0);
/// Only the channels heard.
pub const SHOW_HEARD: Setting<field::Bool> =
    Setting::new("show_heard", "Only channels heard", "", false)
        .help("Lists only the channels that are active now or had a call.");
/// The wall-clock time of the first sample of a recording.
pub const TIME_BASE: Setting<field::Time> = Setting::new("time_base", "Time base", "", 0.0)
    .range(0.0, 1e11, 0.0)
    .help("UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.");

sdrkit_core::settings! {
    /// The settings of the marine VHF stack.
    pub enum VhfSetting {
        /// The channel plan.
        Plan = PLAN,
        /// The channel to play.
        Channel = CHANNEL_HZ,
        /// The squelch.
        Squelch = SQUELCH,
        /// De-emphasis.
        Deemphasis = DEEMPHASIS,
        /// The priority channel.
        Priority = PRIORITY_HZ,
        /// The scan.
        Scan = SCAN,
        /// The scan list.
        ScanList = SCAN_HZ,
        /// The scan dwell.
        ScanDwell = SCAN_DWELL_S,
        /// The scan hang time.
        ScanHang = SCAN_HANG_S,
        /// The most carriers tracked.
        Carriers = CARRIERS_SETTING,
        /// Only the channels heard.
        ShowHeard = SHOW_HEARD,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`CarrierFinder`].
    pub enum FinderSetting {
        /// The most carriers tracked.
        Carriers = CARRIERS_SETTING,
    }
}

sdrkit_core::settings! {
    /// The settings of [`CarrierBank`].
    pub enum BankSetting {
        /// The squelch.
        Squelch = SQUELCH,
        /// De-emphasis.
        Deemphasis = DEEMPHASIS,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Player`].
    pub enum PlayerSetting {
        /// The channel plan.
        Plan = PLAN,
        /// The channel to play.
        Channel = CHANNEL_HZ,
        /// The priority channel.
        Priority = PRIORITY_HZ,
        /// The scan.
        Scan = SCAN,
        /// The scan list.
        ScanList = SCAN_HZ,
        /// The scan dwell.
        ScanDwell = SCAN_DWELL_S,
        /// The scan hang time.
        ScanHang = SCAN_HANG_S,
    }
}

sdrkit_core::settings! {
    /// The settings of [`CallLog`].
    pub enum LogSetting {
        /// The channel plan.
        Plan = PLAN,
        /// The scan list.
        ScanList = SCAN_HZ,
        /// Only the channels heard.
        ShowHeard = SHOW_HEARD,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

/// Ends every open call.
pub const CLOSE_ALL: Command = Command::new("close_all", "Close all").help("Ends every open call.");

sdrkit_core::commands! {
    /// The commands of [`CallLog`].
    pub enum LogCommand {
        /// Forget every call.
        Clear = objcmd::CLEAR,
        /// End every open call.
        CloseAll = CLOSE_ALL,
    }
}

/// The finder on the marine raster: 25 kHz cells summed over 20 kHz, a carrier 9 dB above its floor for 0.12 s, quiet below
/// 5 dB, unmodulated lines rejected.
fn finder_config() -> FinderConfig {
    FinderConfig {
        cell_half_hz: 10e3,
        width_half_hz: 12.5e3,
        line_share: Some(0.8),
        ..FinderConfig::new(RASTER, 0.0)
    }
}

/// The carrier finder (`marinevhf.finder`): the shared finder on the 25 kHz raster.
pub struct CarrierFinder {
    finder: Finder,
    outs: Vec<Timed<Carriers>>,
}

impl Default for CarrierFinder {
    fn default() -> Self {
        CarrierFinder {
            finder: Finder::new(finder_config()),
            outs: Vec::new(),
        }
    }
}

impl Component for CarrierFinder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marinevhf.finder",
            vec![Port::iq("in"), Port::typed::<PlayState>("playing")],
            vec![Port::typed::<Carriers>("carriers").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(FinderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![self.finder.configure(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(p) = io.items::<PlayState>(1).last() {
            self.finder.set_playing(p.value.playing.map(|c| c.hz));
        }
        let mut outs = std::mem::take(&mut self.outs);
        self.finder.process(io.iq(0), &mut outs);
        io.items_out::<Carriers>(0).append(&mut outs);
        self.outs = outs;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FinderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FinderSetting::Carriers => {
                self.finder.config_mut().max = CARRIERS_SETTING.read(id, v)? as usize;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            FinderSetting::Carriers,
            Value::I64(self.finder.config().max as i64),
        )])
    }
}

struct Chain {
    freq: f64,
    chan: Channelizer,
    disc: Discriminator,
    hp: Biquad,
    lp: Biquad,
    hi: Biquad,
    de_y: f32,
    x: Vec<Complex32>,
    a: Vec<f32>,
    block: Vec<f32>,
    vp: f64,
    np: f64,
    rp: f64,
    above: u32,
    below: u32,
    open: bool,
    hist: VecDeque<ChanBlock>,
    t0: f64,
    blocks: u64,
}

/// The carrier bank (`marinevhf.bank`): per carrier the channel at 48 kHz (8 kHz low-pass), the NFM receiver (+-5 kHz full scale,
/// 300 to 3000 Hz, optional 750 us de-emphasis with gain 4.8, limited to +-1), the quieting squelch per 10 ms block and the 30 ms
/// look-ahead.
pub struct CarrierBank {
    rate: f64,
    chains: BTreeMap<u64, Chain>,
    squelch: f64,
    deemph: bool,
    /// Samples since the configure (block times count from it, as the finder's do).
    count: u64,
}

impl Default for CarrierBank {
    fn default() -> Self {
        CarrierBank {
            rate: 0.0,
            chains: BTreeMap::new(),
            squelch: 50.0,
            deemph: true,
            count: 0,
        }
    }
}

impl CarrierBank {
    fn chain(&self, c: &Carrier, t0: f64) -> Option<Chain> {
        let mut cfg = ChannelConfig::new(self.rate, c.offset_hz, 16e3, AUDIO_RATE);
        cfg.exact_rate = true;
        cfg.atten_db = 60.0;
        let chan = Channelizer::new(cfg).ok()?;
        Some(Chain {
            freq: c.freq_hz,
            chan,
            disc: Discriminator::new(AUDIO_RATE, 5000.0)
                .expect("the rate and deviation are positive"),
            hp: Biquad::new(BiquadCoeffs::highpass(AUDIO_RATE, 300.0, 0.707)),
            lp: Biquad::new(BiquadCoeffs::lowpass(AUDIO_RATE, 3000.0, 0.707)),
            hi: Biquad::new(BiquadCoeffs::highpass(AUDIO_RATE, 5500.0, 0.707)),
            de_y: 0.0,
            x: Vec::new(),
            a: Vec::new(),
            block: Vec::with_capacity(BLOCK),
            vp: 0.0,
            np: 0.0,
            rp: 0.0,
            above: 0,
            below: 0,
            open: false,
            hist: VecDeque::new(),
            t0,
            blocks: 0,
        })
    }
}

impl Component for CarrierBank {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marinevhf.bank",
            vec![Port::iq("in"), Port::typed::<Carriers>("carriers")],
            vec![Port::typed::<Blocks>("blocks").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(BankSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.chains.clear();
        self.count = 0;
        Ok(vec![Format::new(Kind::typed::<Blocks>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let t_in = self.count as f64 / self.rate;
        self.count += x.len() as u64;
        // The carrier list of this pass (the newest).
        if let Some(cl) = io.items::<Carriers>(1).last() {
            let ids: Vec<u64> = cl.value.0.iter().map(|c| c.id).collect();
            self.chains.retain(|id, _| ids.contains(id));
            for c in &cl.value.0 {
                if !self.chains.contains_key(&c.id) {
                    if let Some(ch) = self.chain(c, t_in) {
                        self.chains.insert(c.id, ch);
                    }
                }
            }
        }
        let thr = -8.0 + 0.1 * self.squelch;
        let de_a = (1.0 - (-1.0 / (750e-6 * AUDIO_RATE)).exp()) as f32;
        let mut out = Vec::new();
        for ch in self.chains.values_mut() {
            ch.x.clear();
            ch.chan.process(x, &mut ch.x);
            ch.a.clear();
            ch.disc.process(&ch.x, &mut ch.a);
            for (k, &v) in ch.a.iter().enumerate() {
                let h = ch.hi.process_sample(v);
                let vb = ch.lp.process_sample(ch.hp.process_sample(v));
                ch.vp += f64::from(vb * vb);
                ch.np += f64::from(h * h);
                ch.rp += f64::from(ch.x[k].norm_sqr());
                let s = if self.deemph {
                    ch.de_y += de_a * (vb - ch.de_y);
                    ch.de_y * 4.8
                } else {
                    vb
                };
                ch.block.push(s.clamp(-1.0, 1.0));
                if ch.block.len() == BLOCK {
                    let level = 10.0 * (ch.rp / BLOCK as f64).max(1e-30).log10();
                    // No input at all (a gap of zeros, the end of a file): nothing to open on, whatever the filters ring.
                    let measure = if level < -150.0 {
                        f64::NEG_INFINITY
                    } else {
                        (10.0 * (ch.vp / ch.np.max(1e-30)).log10()).min(5.0)
                    };
                    ch.vp = 0.0;
                    ch.np = 0.0;
                    ch.rp = 0.0;
                    // Opens after 2 blocks above the threshold, stays while above threshold - 2 dB, closes 15 blocks after.
                    if !ch.open {
                        ch.above = if measure > thr { ch.above + 1 } else { 0 };
                        if ch.above >= 2 {
                            ch.open = true;
                            ch.below = 0;
                        }
                    } else if measure > thr - 2.0 {
                        ch.below = 0;
                    } else {
                        ch.below += 1;
                        if ch.below >= 15 {
                            ch.open = false;
                            ch.above = 0;
                        }
                    }
                    let b = ChanBlock {
                        key: key(ch.freq),
                        freq_hz: ch.freq,
                        time: ch.t0 + ch.blocks as f64 * BLOCK as f64 / AUDIO_RATE,
                        open: ch.open,
                        level_dbfs: level,
                        measure_db: measure,
                        audio: std::mem::take(&mut ch.block),
                    };
                    ch.blocks += 1;
                    ch.block = Vec::with_capacity(BLOCK);
                    ch.hist.push_back(b);
                    // The audio runs 3 blocks late: a block is let through when it, or any of the 3 after it, is open.
                    let late = if ch.hist.len() > 3 {
                        let any_open = ch.hist.iter().any(|h| h.open);
                        ch.hist.pop_front().map(|first| (first, any_open))
                    } else {
                        None
                    };
                    if let Some((mut first, gate)) = late {
                        if !gate {
                            first.audio.iter_mut().for_each(|v| *v = 0.0);
                        }
                        first.open = first.open || (gate && ch.hist.back().is_some_and(|h| h.open));
                        out.push(first);
                    }
                }
            }
        }
        if !out.is_empty() {
            io.items_out::<Blocks>(0).push(Timed {
                time: t_in,
                value: Blocks(out),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BankSetting::Squelch => self.squelch = SQUELCH.read(id, v)?,
            BankSetting::Deemphasis => self.deemph = DEEMPHASIS.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BankSetting::Squelch, Value::F64(self.squelch)),
            (BankSetting::Deemphasis, Value::Bool(self.deemph)),
        ])
    }
}

/// The marine plan as the player sees it: the scan list, the picked channel and the priority channel.
struct VhfScanPlan {
    plan: Arc<Plan>,
    channel_hz: f64,
    priority_hz: f64,
    scan_hz: Vec<f64>,
}

fn key(f: f64) -> i64 {
    (f / 100.0).round() as i64
}

fn chan(f: f64) -> Chan {
    Chan { key: key(f), hz: f }
}

/// The frequencies of the scan list text: numbers in Hz separated by spaces, commas or semicolons. `None` when one is not
/// a positive number.
fn parse_scan(text: &str) -> Option<Vec<f64>> {
    text.split([',', ' ', ';'])
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0))
        .collect()
}

/// The listening frequencies of the scan list `list` (every voice channel of the plan when the list is empty).
fn scan_frequencies(plan: &Plan, list: &[f64]) -> Vec<f64> {
    if list.is_empty() {
        plan.voice_channels().flat_map(|c| c.listen_hz()).collect()
    } else {
        list.iter().flat_map(|&f| plan.listen_hz(f)).collect()
    }
}

/// The number at the start of a channel label ("22A" is 22), 0 for none.
fn label_number(label: &str) -> u32 {
    let digits: String = label.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(0)
}

impl ScanPlan for VhfScanPlan {
    fn scan_list(&self) -> Vec<Chan> {
        scan_frequencies(&self.plan, &self.scan_hz)
            .into_iter()
            .map(chan)
            .collect()
    }
    fn picked(&self) -> Option<Vec<Chan>> {
        (self.channel_hz > 0.0).then(|| {
            self.plan
                .listen_hz(self.channel_hz)
                .into_iter()
                .map(chan)
                .collect()
        })
    }
    fn priority(&self) -> Option<Chan> {
        (self.priority_hz > 0.0).then(|| chan(self.plan.listen_hz(self.priority_hz)[0]))
    }
    fn is_priority(&self, c: Chan) -> bool {
        self.priority_hz > 0.0
            && self
                .plan
                .listen_hz(self.priority_hz)
                .into_iter()
                .any(|f| key(f) == c.key)
    }
}

/// The player (`marinevhf.player`): what plays, the scan and the priority watch.
pub struct VhfPlayer {
    plan: Arc<Plan>,
    plan_text: String,
    channel_hz: f64,
    priority_hz: f64,
    scan_text: String,
    scan_hz: Vec<f64>,
    player: Player,
    /// The audio of one pass.
    audio: Vec<f32>,
}

impl Default for VhfPlayer {
    fn default() -> Self {
        let plan = Arc::new(Plan::default());
        let priority_hz = PRIORITY_HZ.default;
        let mut p = VhfPlayer {
            plan: plan.clone(),
            plan_text: String::new(),
            channel_hz: 0.0,
            priority_hz,
            scan_text: String::new(),
            scan_hz: Vec::new(),
            player: Player::new(Box::new(VhfScanPlan {
                plan,
                channel_hz: 0.0,
                priority_hz,
                scan_hz: Vec::new(),
            })),
            audio: Vec::new(),
        };
        p.set_plan();
        p
    }
}

impl VhfPlayer {
    fn set_plan(&mut self) {
        self.player.plan = Box::new(VhfScanPlan {
            plan: self.plan.clone(),
            channel_hz: self.channel_hz,
            priority_hz: self.priority_hz,
            scan_hz: self.scan_hz.clone(),
        });
    }
}

impl Component for VhfPlayer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marinevhf.player",
            vec![
                Port::typed::<Blocks>("blocks"),
                Port::typed::<Carriers>("carriers"),
            ],
            vec![
                Port::audio("out").rate_from(RateFrom::Fixed(AUDIO_RATE)),
                Port::typed::<PlayState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(PlayerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.player.configure(finder_config().held(&inputs[1]));
        Ok(vec![
            Format::new(Kind::Audio, AUDIO_RATE),
            Format::new(Kind::typed::<PlayState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut audio = std::mem::take(&mut self.audio);
        audio.clear();
        let changed =
            self.player
                .process(io.items::<Blocks>(0), io.items::<Carriers>(1), &mut audio);
        io.audio_out(0).samples.extend_from_slice(&audio);
        self.audio = audio;
        // The state goes out at every report, and at once when it changed (the single-tuner scan moves the radio by it).
        if io.tick() || changed {
            io.items_out::<PlayState>(1).push(Timed {
                time: self.player.clock(),
                value: self.player.state(),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PlayerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PlayerSetting::Plan => {
                let t = PLAN.read(id, v)?;
                self.plan =
                    Arc::new(Plan::parse(t).ok_or(SettingError::Malformed { id, at: None })?);
                t.clone_into(&mut self.plan_text);
            }
            PlayerSetting::Channel => self.channel_hz = CHANNEL_HZ.read(id, v)?,
            PlayerSetting::Priority => self.priority_hz = PRIORITY_HZ.read(id, v)?,
            PlayerSetting::Scan => self.player.scan = SCAN.read(id, v)?,
            PlayerSetting::ScanList => {
                let t = SCAN_HZ.read(id, v)?;
                self.scan_hz = parse_scan(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.scan_text);
            }
            PlayerSetting::ScanDwell => self.player.dwell = SCAN_DWELL_S.read(id, v)?,
            PlayerSetting::ScanHang => self.player.hang = SCAN_HANG_S.read(id, v)?,
        }
        self.set_plan();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (PlayerSetting::Plan, Value::Text(self.plan_text.clone())),
            (PlayerSetting::Channel, Value::Hz(self.channel_hz)),
            (PlayerSetting::Priority, Value::Hz(self.priority_hz)),
            (PlayerSetting::Scan, Value::Bool(self.player.scan)),
            (PlayerSetting::ScanList, Value::Text(self.scan_text.clone())),
            (PlayerSetting::ScanDwell, Value::Duration(self.player.dwell)),
            (PlayerSetting::ScanHang, Value::Duration(self.player.hang)),
        ])
    }
}

/// One call in the log: the row of the `calls` table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LoggedCall(pub Call);

impl Shift for LoggedCall {
    fn shift_times(&mut self, by_s: f64) {
        self.0.shift_times(by_s);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Fields, tables and events.

/// The most calls the log keeps.
pub const CALL_CAP: usize = 600;
/// The result fields of the marine VHF stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VhfFields<'a> {
    /// Carriers the finder tracks.
    #[field(label = "Carriers")]
    pub carriers: i64,
    /// Carriers active now.
    #[field(label = "Active channels")]
    pub active_channels: i64,
    /// The channel that plays.
    #[field(label = "Playing")]
    pub playing: Option<&'a str>,
    /// The frequency that plays.
    #[field(label = "Playing frequency", unit = "Hz")]
    pub playing_hz: Option<Hertz>,
    /// What the player does.
    #[field(label = "Scan")]
    pub scan_state: ScanState,
    /// The frequency the single-tuner scan requested from the radio.
    #[field(label = "Radio on", unit = "Hz")]
    pub tune_hz: Option<Hertz>,
    /// Calls since the start.
    #[field(label = "Calls")]
    pub calls: i64,
    /// The capture holds more than one channel.
    #[field(label = "Whole span")]
    pub multi_channel: bool,
    /// Reports made.
    #[field(label = "Reports")]
    pub reports: i64,
}

/// A row of the call table of the marine VHF stack. The row key is the place of the call in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VhfCallRow<'a> {
    /// The number of a call.
    #[field(label = "#")]
    pub serial: i64,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// When a call started (wall-clock time, when known).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time a call started.
    #[field(label = "Start", unit = "s")]
    pub start: Seconds,
    /// How long a call lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// The highest level of a call or channel.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
    /// The mean level of a call.
    #[field(label = "Mean", unit = "dBFS")]
    pub mean_dbfs: f64,
    /// The call or channel is open.
    #[field(label = "Open")]
    pub open: bool,
}

/// The calls, newest first.
pub static CALL_TABLE: Table<VhfCallRow<'static>> = Table::new("calls", "Calls", CALL_CAP);

/// A row of the channel table of the marine VHF stack. The row key is the channel frequency in units of the raster.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VhfChannelRow<'a> {
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The channel carries a signal now.
    #[field(label = "Active")]
    pub active: bool,
    /// The call or channel is open.
    #[field(label = "Open")]
    pub open: bool,
    /// The channel plays.
    #[field(label = "Playing")]
    pub now_playing: bool,
    /// The channel is in the scan list.
    #[field(label = "In scan")]
    pub in_scan: bool,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Calls on the channel.
    #[field(label = "Calls")]
    pub calls: i64,
    /// The time the channel was open in all.
    #[field(label = "Time open", unit = "s")]
    pub total: Seconds,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The highest level of a call or channel.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: Option<f64>,
}

/// The channels of the plan.
pub static CHANNEL_TABLE: Table<VhfChannelRow<'static>> = Table::new("channels", "Channels", 400);

/// The fields of the event of a call on a marine VHF channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VhfCallEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The signal time a call started.
    #[field(label = "Start", unit = "s")]
    pub start: Seconds,
    /// How long a call lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// The highest level of a call or channel.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
}

/// A call ended.
pub static CALL: EventKind<VhfCallEvent<'static>> = EventKind::new("marinevhf.call", "Call");

impl Shaped for LoggedCall {
    type Row = VhfCallRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let Call {
            serial,
            label,
            freq_hz,
            at,
            duration_s,
            peak_dbfs,
            mean_dbfs,
            open,
            ..
        } = &self.0;
        out.put(&VhfCallRow {
            serial: *serial as i64,
            channel: label,
            freq_hz: Hertz(*freq_hz),
            time: at.wall.map(Time),
            start: Seconds(at.t),
            duration: Seconds(*duration_s),
            peak_dbfs: *peak_dbfs,
            mean_dbfs: *mean_dbfs,
            open: *open,
        });
    }
}

/// What the call log keeps across sessions: the calls and the serial of the last one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VhfLists {
    /// The calls, oldest first.
    pub calls: LogRing<LoggedCall>,
    /// The serial of the last call.
    pub serial: u64,
}

impl Default for VhfLists {
    fn default() -> Self {
        VhfLists {
            calls: LogRing::new(CALL_CAP),
            serial: 0,
        }
    }
}

impl Shift for VhfLists {
    fn shift_times(&mut self, by_s: f64) {
        self.calls.shift_times(by_s);
    }
}

impl Tables for VhfLists {
    const FORMAT: StateFormat = StateFormat::new("marine.vhf_lists", 1);
    fn clear(&mut self) {
        self.calls.clear();
        self.serial = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.calls.adopt(saved.calls);
        for c in self.calls.iter_mut() {
            c.0.open = false;
        }
        self.serial = self.serial.max(saved.serial);
    }
}

/// The call log (`marinevhf.log`): the calls, the channel rows, the results and the radio requests of the single-tuner scan.
pub struct CallLog {
    p: Published,
    plan: Plan,
    plan_text: String,
    time_base: f64,
    book: CallBook,
    lists: VhfLists,
    carriers: Vec<Carrier>,
    play: PlayState,
    clock: Clock,
    host_clock: sdrkit_core::Clock,
    live: bool,
    /// The first item time of the current input: item times count from it.
    first_time: Option<f64>,
    reports: u64,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    span_hz: f64,
    scan_text: String,
    scan_hz: Vec<f64>,
    /// The channel the single-tuner scan last requested from the radio.
    asked_hz: Option<f64>,
    /// The plan changed since the channel rows were published last.
    rows_stale: bool,
    /// List only the channels that are active or were heard.
    show_heard: bool,
    /// The blocks of one pass in time order: (time, item, block).
    order: Vec<(f64, usize, usize)>,
}

impl Default for CallLog {
    fn default() -> Self {
        let mut p =
            Published::new(VhfFields::SHAPE.fields).with_tables(&[&CALL_TABLE, &CHANNEL_TABLE]);
        p.results_mut().set_status(&Status {
            summary: "Marine VHF: waiting for samples".into(),
            ..Status::default()
        });
        CallLog {
            p,
            plan: Plan::default(),
            plan_text: String::new(),
            time_base: 0.0,
            book: CallBook::default(),
            lists: VhfLists::default(),
            carriers: Vec::new(),
            play: PlayState::default(),
            clock: Clock::default(),
            host_clock: sdrkit_core::Clock::fixed(0.0),
            live: false,
            first_time: None,
            reports: 0,
            events: Vec::new(),
            requests: Vec::new(),
            span_hz: 0.0,
            scan_text: String::new(),
            scan_hz: Vec::new(),
            asked_hz: None,
            rows_stale: true,
            show_heard: false,
            order: Vec::new(),
        }
    }
}

impl CallLog {
    /// The calls, oldest first.
    pub fn calls(&self) -> impl Iterator<Item = &Call> {
        self.lists.calls.iter().map(|c| &c.0)
    }

    /// Takes the ended call `c`: its event, its log line and its final row.
    fn ended(&mut self, c: Call) {
        self.events.push(CALL.event(
            c.at.t,
            c.freq_hz,
            &VhfCallEvent {
                source: None,
                target: None,
                channel: &c.label,
                family: Family::Vessel,
                start: Seconds(c.at.t),
                duration: Seconds(c.duration_s),
                peak_dbfs: c.peak_dbfs,
            },
        ));
        self.p
            .results_mut()
            .log(format!("Marine VHF: ch {} {:.1} s", c.label, c.duration_s));
        let serial = c.serial;
        if let Some(last) = self
            .lists
            .calls
            .iter_mut()
            .rev()
            .find(|x| x.0.serial == serial)
        {
            last.0 = c;
        }
    }

    /// Takes block `b` of signal time `time` (on the clock that runs across inputs).
    fn block(&mut self, b: &ChanBlock, time: f64) {
        let (clock, plan) = (&self.clock, &self.plan);
        let step = self.book.block(
            b,
            time,
            &mut self.lists.serial,
            || clock.stamp(time),
            |c| match plan.label_of(b.freq_hz) {
                Some(label) => {
                    label.clone_into(&mut c.label);
                    c.number = label_number(label);
                }
                None => c.label = "?".into(),
            },
        );
        if let Some(c) = step.started {
            self.lists.calls.push(LoggedCall(c));
        }
        if let Some(c) = step.ended {
            self.ended(c);
        }
    }

    /// Ends every open call (the stream ended).
    pub fn close_all(&mut self) {
        for c in self.book.end_all() {
            self.ended(c);
        }
    }

    /// The listening frequencies of the scan list (every voice channel of the plan when the list is empty).
    fn scan_keys(&self) -> Vec<i64> {
        scan_frequencies(&self.plan, &self.scan_hz)
            .into_iter()
            .map(key)
            .collect()
    }

    fn report(&mut self) {
        self.reports += 1;
        let now = self.clock.now();
        let any_open = self.book.any_open();
        let lock_state = if self.clock.now() > 0.0 || !self.carriers.is_empty() {
            if any_open {
                LockState::Locked
            } else {
                LockState::Searching
            }
        } else {
            LockState::Off
        };
        let active = self.carriers.iter().filter(|c| c.active).count();
        let playing = self
            .play
            .playing
            .and_then(|c| self.plan.label_of(c.hz).map(str::to_owned));
        let scan_keys = self.scan_keys();
        let p = &mut self.p;
        p.put(&VhfFields {
            carriers: self.carriers.len() as i64,
            active_channels: active as i64,
            playing: playing.as_deref(),
            playing_hz: self.play.playing.map(|c| c.hz).map(Hertz),
            scan_state: self.play.scan_state,
            tune_hz: self.play.tune_hz.map(Hertz),
            calls: self.lists.serial as i64,
            multi_channel: self.span_hz > 2.0 * RASTER,
            reports: self.reports as i64,
        });
        shaped::log_rows(&self.lists.calls, &CALL_TABLE, now, p);
        let mut id = String::new();
        let mut keep: Vec<i64> = Vec::new();
        for c in &self.plan.channels {
            for f in c.listen_hz() {
                let k = key(f);
                let st = self.book.stats(k).cloned().unwrap_or_default();
                let carrier = self.carriers.iter().find(|x| key(x.freq_hz) == k);
                let active = carrier.is_some_and(|x| x.active);
                if self.show_heard && !active && st.calls == 0 {
                    continue;
                }
                keep.push(k);
                let total = st.total + self.book.open_call(k).map_or(0.0, |c| c.duration_s);
                id.clear();
                let _ = write!(id, "{k}");
                CHANNEL_TABLE.put(
                    p,
                    &id,
                    &VhfChannelRow {
                        channel: &c.label,
                        freq_hz: Hertz(f),
                        active,
                        open: st.open,
                        now_playing: self.play.playing.is_some_and(|p| p.key == k),
                        in_scan: scan_keys.contains(&k),
                        level_dbfs: st.level,
                        snr_db: carrier.map(|x| x.snr_db),
                        calls: st.calls as i64,
                        total: Seconds(total),
                        last_heard: st.last.and_then(|l| self.clock.wall(l)).map(Time),
                        peak_dbfs: st.peak,
                    },
                );
            }
        }
        if self.rows_stale || self.show_heard {
            self.rows_stale = false;
            let table = p.results().decl().table_id(CHANNEL_TABLE.key);
            let gone: Vec<String> = table
                .and_then(|t| p.results().table(t))
                .map(|t| {
                    t.rows_newest_first()
                        .filter(|(k, _)| k.parse::<i64>().map_or(true, |k| !keep.contains(&k)))
                        .map(|(k, _)| k.to_owned())
                        .collect()
                })
                .unwrap_or_default();
            for k in &gone {
                p.remove_row(&CHANNEL_TABLE, k);
            }
        }
        let ncalls = self.lists.serial;
        let summary = match (&playing, lock_state) {
            (_, LockState::Off) => "Marine VHF: waiting for samples".into(),
            (Some(l), _) => {
                format!("Marine VHF: listening to ch {l}, {active} active, {ncalls} calls")
            }
            (None, _) => format!("Marine VHF: {active} channels active, {ncalls} calls"),
        };
        p.results_mut().set_status(&Status {
            lock_state,
            summary,
            ..Status::default()
        });
    }
}

impl Component for CallLog {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marinevhf.log",
            vec![
                Port::typed::<Blocks>("blocks"),
                Port::typed::<Carriers>("carriers"),
                Port::typed::<PlayState>("play"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(LogSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A reset drops the carriers (they belong to the old plan). The call log stays, and the clock runs on.
        self.span_hz = inputs
            .get(1)
            .and_then(|f| finder_config().held(f))
            .map_or(0.0, |(lo, hi)| hi - lo);
        self.carriers.clear();
        self.close_all();
        self.clock.restart(self.live, self.host_clock.now());
        self.first_time = None;
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, io: &mut Io) {
        let first = io
            .items::<Carriers>(1)
            .first()
            .map(|c| c.time)
            .into_iter()
            .chain(
                io.items::<Blocks>(0)
                    .iter()
                    .flat_map(|b| b.value.0.first().map(|c| c.time)),
            )
            .reduce(f64::min);
        if let Some(t) = first {
            self.first_time.get_or_insert(t);
        }
        let t0 = self.first_time.unwrap_or(0.0);
        if let Some(c) = io.items::<Carriers>(1).last() {
            self.carriers.clone_from(&c.value.0);
            self.clock.at(c.time - t0);
        }
        if let Some(p) = io.items::<PlayState>(2).last() {
            self.play = p.value.clone();
        }
        // The single-tuner scan requests the host to move the radio.
        if self.play.tune_hz != self.asked_hz {
            self.asked_hz = self.play.tune_hz;
            if let Some(f) = self.asked_hz {
                let label = self.plan.label_of(f).unwrap_or_default().to_owned();
                self.requests.push(HostRequest::Retune {
                    tuned_hz: f,
                    reason: format!(
                        "Marine VHF scan, ch {label} ({})",
                        self.play.scan_state.words()
                    ),
                });
            }
        }
        let mut order = std::mem::take(&mut self.order);
        order.clear();
        let items = io.items::<Blocks>(0);
        for (i, b) in items.iter().enumerate() {
            order.extend(b.value.0.iter().enumerate().map(|(j, cb)| (cb.time, i, j)));
        }
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(time, i, j) in &order {
            let t = self.clock.at(time - t0);
            self.block(&items[i].value.0[j], t);
        }
        self.order = order;
        io.messages_out(0).append(&mut self.events);
        let now = self.clock.now();
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: now,
                value: r,
            });
        }
        if io.tick() {
            self.report();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match LogSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LogSetting::Plan => {
                let t = PLAN.read(id, v)?;
                self.plan = Plan::parse(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.plan_text);
                self.rows_stale = true;
            }
            LogSetting::ScanList => {
                let t = SCAN_HZ.read(id, v)?;
                self.scan_hz = parse_scan(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.scan_text);
            }
            LogSetting::ShowHeard => {
                self.show_heard = SHOW_HEARD.read(id, v)?;
                self.report();
            }
            LogSetting::TimeBase => {
                let t = TIME_BASE.read(id, v)?;
                self.time_base = t;
                self.clock.set_time_base((t > 0.0).then_some(t));
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (LogSetting::Plan, Value::Text(self.plan_text.clone())),
            (LogSetting::ScanList, Value::Text(self.scan_text.clone())),
            (LogSetting::ShowHeard, Value::Bool(self.show_heard)),
            (LogSetting::TimeBase, Value::Time(self.time_base)),
        ])
    }
    fn commands(&self) -> &Commands {
        LogCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match LogCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            LogCommand::Clear => {
                self.lists.clear();
                self.book.clear();
                self.report();
            }
            LogCommand::CloseAll => self.close_all(),
        }
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

/// The marine VHF components.
pub fn register(r: &mut Registry) {
    r.add("marinevhf.finder", || Box::new(CarrierFinder::default()));
    r.add("marinevhf.bank", || Box::new(CarrierBank::default()));
    r.add("marinevhf.player", || Box::new(VhfPlayer::default()));
    r.add("marinevhf.log", || Box::new(CallLog::default()));
}

/// The marine VHF graph.
pub fn builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("finder", CarrierFinder::default())
        .add("bank", CarrierBank::default())
        .add("player", VhfPlayer::default())
        .add("log", CallLog::default())
        .input("finder.in")
        .input("bank.in")
        .connect("finder.carriers", "bank.carriers")
        .connect("bank.blocks", "player.blocks")
        .connect("finder.carriers", "player.carriers")
        .connect("bank.blocks", "log.blocks")
        .connect("finder.carriers", "log.carriers")
        .connect("player.state", "log.play")
        .feedback("player.state", "finder.playing")
        .output("player.out")
        .output("log.events")
        .output("log.requests")
        .status_from("log")
        .clock(0.25)
        .reconfigure_on_retune()
}

/// The marine VHF stack graph: the settings routed to the components that own them, and the call log's commands.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape("marinevhf"), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "Marine VHF needs a sample rate of at least 250 ksps",
        )
        .route(PLAN.key(), &["player.plan", "log.plan"])?
        .route(SCAN_HZ.key(), &["player.scan_hz", "log.scan_hz"])?
        .route(CHANNEL_HZ.key(), &["player.channel_hz"])?
        .route(PRIORITY_HZ.key(), &["player.priority_hz"])?
        .route(SCAN.key(), &["player.scan"])?
        .route(SCAN_DWELL_S.key(), &["player.scan_dwell_s"])?
        .route(SCAN_HANG_S.key(), &["player.scan_hang_s"])?
        .route(SQUELCH.key(), &["bank.squelch"])?
        .route(DEEMPHASIS.key(), &["bank.deemphasis"])?
        .route(CARRIERS_SETTING.key(), &["finder.carriers"])?
        .route(TIME_BASE.key(), &["log.time_base"])?
        .route(SHOW_HEARD.key(), &["log.show_heard"])?
        .command("clear", sdrkit_graph::CommandTarget::of("log", "clear"))
        .command(
            "close_all",
            sdrkit_graph::CommandTarget::of("log", "close_all"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The marine VHF graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder().catalog(catalog).build()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// One transmission.
#[derive(Clone, Debug)]
pub struct Transmission {
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// When it starts, s.
    pub start_s: f64,
    /// How long it lasts, s.
    pub dur_s: f64,
    /// The transmission's tone. 0 or below for speech-like tones (three formant-like tones under a syllabic envelope).
    pub tone_hz: f64,
}

impl Transmission {
    /// The audio at `t` seconds into the transmission, full scale 1 (before any pre-emphasis).
    fn audio(&self, t: f64) -> f64 {
        if self.tone_hz > 0.0 {
            return (TAU * self.tone_hz * t).sin();
        }
        // About four syllables a second, never quite silent.
        let env = 0.55 + 0.45 * (TAU * 4.0 * t).sin().abs();
        env * (0.5 * (TAU * 300.0 * t).sin()
            + 0.3 * (TAU * 900.0 * t + 1.0).sin()
            + 0.2 * (TAU * 1700.0 * t + 2.0).sin())
    }
    /// The 750 us pre-emphasis gain for this transmission's tone, unity at 1 kHz (about 4.8 below the corner).
    fn preemphasis(&self) -> f64 {
        let g = |f: f64| (1.0 + (TAU * f * 750e-6).powi(2)).sqrt();
        if self.tone_hz > 0.0 {
            g(self.tone_hz) / g(1000.0)
        } else {
            1.0
        }
    }
}

/// Options of the marine VHF test signal.
#[derive(Clone, Debug)]
pub struct VhfGen {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The stream's centre (absolute), Hz.
    pub centre_hz: f64,
    /// The transmissions.
    pub tx: Vec<Transmission>,
    /// Use the 750 us pre-emphasis.
    pub preemphasis: bool,
}

impl Default for VhfGen {
    fn default() -> Self {
        VhfGen {
            rate: 1e6,
            centre_hz: 156.55e6 + 12.5e3,
            tx: vec![
                Transmission {
                    freq_hz: 156.8e6,
                    start_s: 1.0,
                    dur_s: 2.0,
                    tone_hz: 1000.0,
                },
                Transmission {
                    freq_hz: 156.3e6,
                    start_s: 3.5,
                    dur_s: 2.0,
                    tone_hz: 700.0,
                },
                Transmission {
                    freq_hz: 156.65e6,
                    start_s: 6.0,
                    dur_s: 2.0,
                    tone_hz: 1300.0,
                },
            ],
            preemphasis: false,
        }
    }
}

/// The marine VHF generator: narrow FM (+-5 kHz peak deviation for full-scale audio) of a tone (or speech-like tones) on each
/// transmission's channel, with 750 us pre-emphasis when requested. Each transmission has unit power. The signal is clean: the
/// engine adds the impairments.
pub struct VhfGenerator {
    o: VhfGen,
    freqs: Vec<f64>,
    phases: Vec<f64>,
    n: u64,
}

impl VhfGenerator {
    /// A generator of the transmissions in `o`.
    pub fn new(o: VhfGen) -> Self {
        let freqs: Vec<f64> = o.tx.iter().map(|t| t.freq_hz).collect();
        VhfGenerator {
            phases: vec![0.0; o.tx.len()],
            freqs,
            o,
            n: 0,
        }
    }
    /// A generator of `o` whose first sample is at `t0_s` seconds of the programme (a radio that was retuned part way through).
    pub fn starting_at(o: VhfGen, t0_s: f64) -> Self {
        let mut g = VhfGenerator::new(o);
        g.n = (t0_s * g.o.rate).round() as u64;
        g
    }
}

impl Generator for VhfGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let rate = self.o.rate;
        for s in out.iter_mut() {
            let t = self.n as f64 / rate;
            self.n += 1;
            let mut v = Complex32::default();
            for (k, tx) in self.o.tx.iter().enumerate() {
                if t < tx.start_s || t >= tx.start_s + tx.dur_s {
                    continue;
                }
                // Audio at 60 % of full scale produces 3 kHz peak deviation. Pre-emphasis can raise it to the 5 kHz limit.
                let pre = if self.o.preemphasis {
                    tx.preemphasis()
                } else {
                    1.0
                };
                let dev = (3000.0 * pre * tx.audio(t - tx.start_s)).clamp(-5000.0, 5000.0);
                let f = self.freqs[k] - self.o.centre_hz + dev;
                self.phases[k] = (self.phases[k] + TAU * f / rate) % TAU;
                let p = self.phases[k];
                v += Complex32::new(p.cos() as f32, p.sin() as f32);
            }
            *s = v;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(signal_info())
    }
}

/// What the marine VHF test signal states: each transmission has unit power, and its SNR is measured in 12.5 kHz.
pub fn signal_info() -> SignalInfo {
    SignalInfo::new(12_500.0)
}

/// The generator the descriptor makes: the three test transmissions at the given rate.
pub fn from_config(cfg: &GenConfig) -> Box<dyn Generator> {
    let o = VhfGen {
        rate: cfg.rate_hz,
        ..Default::default()
    };
    Box::new(VhfGenerator::new(o))
}
