//! Voice channels (`voice_channels`): every carrier inside the capture at once. A finder tracks the carriers on a raster, a
//! bank runs a voice channel receiver on each one, a player chooses what plays and runs the scan, and a log keeps the calls,
//! the channel rows and the requests to the radio. The channel plan is a file that the host names in the `plan` setting.
//! Without one every carrier is a carrier off the plan, labelled by its frequency.

use crate::plans::{self, Channel, Plan, Profile, Service, SIDEBAND_TUNE_OFFSET_HZ};
use crate::rx::{finder_threshold_db, Demod, RxBlock, RxConfig, ToneRead, VoiceRx};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, Event, HostRequest, LockState, Record, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, StackConfig, StateBlob, StateError,
    Status, Storage, StorePath, Value, Values,
};
use sdrkit_data_plans::PlanError;
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Kind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_channels::{
    Blocks, Call, CallBook, Carrier, Carriers, Chan, ChanBlock, Finder, FinderConfig, PlayState,
    Player, ScanPlan, SignalClock,
};
use crate::typed::{
    CallRow, ChannelRow, VoiceChannelsCallEvent, VoiceChannelsFields, CALLS, CHANNELS,
    VOICE_CHANNELS_CALL,
};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_proto_tones::components::{SubAudioEvent, SUBAUDIO_EVENT};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

pub use sdrkit_proto_channels::{AUDIO_RATE, BLOCK};

/// The stack id.
pub const ID: &str = "voice_channels";
/// The keys of carriers off the plan start here. A key below it is the index of a plan channel.
pub const OFF_PLAN: i64 = 1_000_000;
/// The most carriers off the plan that get a receiver.
pub const OFF_PLAN_MAX: usize = 24;
/// The span kept for the receiver of a new carrier, s.
pub const PREROLL_S: f64 = 0.25;
/// The blocks of every channel go out this long after their signal, s: the blocks that a new receiver makes from the
/// stored span are then in time for the player.
pub const HOLD_S: f64 = PREROLL_S + 0.05;
/// The part of the rate the finder searches on each side of the centre.
const SPAN: f64 = 0.46;

sdrkit_core::choice_enum! {
    /// The demodulator setting.
    pub enum DemodChoice {
        /// The demodulator of the plan channel, else of the submode.
        Auto = ("auto", "Automatic"),
        /// Narrow FM.
        Fm = ("fm", "FM"),
        /// AM.
        Am = ("am", "AM"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
    }
}

impl DemodChoice {
    fn forced(self) -> Option<Demod> {
        match self {
            DemodChoice::Auto => None,
            DemodChoice::Fm => Some(Demod::Fm),
            DemodChoice::Am => Some(Demod::Am),
            DemodChoice::Usb => Some(Demod::Usb),
            DemodChoice::Lsb => Some(Demod::Lsb),
        }
    }
}

/// The channel plan: a plan file in the host's storage.
pub const PLAN: Setting<field::Text> = Setting::new("plan", "Channel plan", "", "")
    .help("The path of a plan file in the host's storage, in a format that plans.import takes (docs/schema/plans.json). Empty for no plan: every carrier is then labelled by its frequency. A change forgets the carriers and the channel statistics.");
/// The raster step of the carrier finder.
pub const RASTER_STEP_HZ: Setting<field::Hz> =
    Setting::new("raster_step_hz", "Raster step", "Hz", 0.0)
        .range(0.0, 200e3, 0.0)
        .help("The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes.");
/// A frequency of the raster.
pub const RASTER_ORIGIN_HZ: Setting<field::Hz> =
    Setting::new("raster_origin_hz", "Raster origin", "Hz", 0.0)
        .range(0.0, 300e9, 0.0)
        .help("A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz.");
/// The width of a channel off the plan.
pub const CHANNEL_WIDTH_HZ: Setting<field::Hz> =
    Setting::new("channel_width_hz", "Channel width", "Hz", 0.0)
        .range(0.0, 50e3, 0.0)
        .help("The width of a channel that no plan describes. 0 takes the width that the stack or its submode fixes.");
/// The channel to play.
pub const CHANNEL: Setting<field::Text> = Setting::new("channel", "Channel", "", "auto")
    .help("A channel label of the plan, or auto for the strongest open channel. A label that the plan does not have is as auto.");
/// The demodulator.
pub const DEMOD: Setting<field::Choice<DemodChoice>> =
    Setting::choice("demod", "Demodulator", DemodChoice::Auto)
        .help("Automatic uses the demodulator of the plan channel, else the one of the submode: FM, upper sideband, or for CB the one the signal shows.");
/// The squelch.
pub const SQUELCH: Setting<field::F64> = Setting::new("squelch", "Squelch", "%", 50.0)
    .range(0.0, 100.0, 1.0)
    .help("0 opens at the weakest signal.");
/// De-emphasis of FM audio.
pub const DEEMPHASIS: Setting<field::Bool> = Setting::new("deemphasis", "De-emphasis", "", true);
/// The scan.
pub const SCAN: Setting<field::Bool> = Setting::new("scan", "Scan", "", false);
/// The scan list.
pub const SCAN_CHANNELS: Setting<field::Text> = Setting::new("scan_channels", "Channels", "", "")
    .help("Channel labels of the plan. Empty scans every channel of the plan. Without a plan the scan has no channels.");
/// The scan dwell.
pub const SCAN_DWELL_S: Setting<field::Duration> =
    Setting::new("scan_dwell_s", "Dwell", "s", 0.4).range(0.1, 5.0, 0.0);
/// The scan hang time.
pub const SCAN_HANG_S: Setting<field::Duration> =
    Setting::new("scan_hang_s", "Hang", "s", 2.0).range(0.0, 6.0, 0.0);
/// The most carriers followed.
pub const CARRIERS_SETTING: Setting<field::I64> =
    Setting::new("carriers", "Carriers at once", "", 10).range(1.0, 24.0, 1.0);
/// Only the channels heard.
pub const SHOW_HEARD: Setting<field::Bool> = Setting::new("show_heard", "Only heard", "", false)
    .help("Lists only the channels that are active now or had a call.");
/// The wall-clock time of the first sample of a recording.
pub const TIME_BASE: Setting<field::Time> = Setting::new("time_base", "Time of the first sample", "", 0.0)
    .range(0.0, 1e11, 0.0)
    .help("UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.");
/// The carrier-to-noise ratio of the test signal.
pub const GEN_CNR_DB: Setting<field::F64> =
    Setting::new("gen_cnr_db", "Test signal C/N", "dB", 30.0)
        .range(0.0, 80.0, 0.0)
        .help("The carrier-to-noise ratio of the test signal in 10 kHz.");
/// The scene of the test signal.
pub const GEN_SCENE: Setting<field::Choice<crate::scene::GenScene>> =
    Setting::choice("gen_scene", "Test signal scene", crate::scene::GenScene::Auto)
        .help("The transmissions of the test signal, at fixed offsets from the centre of the capture. Automatic takes the scene of the submode, and the mixed scene when the generator is given no submode.");

sdrkit_core::settings! {
    /// The settings of the voice channel stack.
    pub enum VoiceSetting {
        /// The channel plan.
        Plan = PLAN,
        /// The raster step.
        RasterStep = RASTER_STEP_HZ,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN_HZ,
        /// The width of a channel off the plan.
        ChannelWidth = CHANNEL_WIDTH_HZ,
        /// The channel to play.
        Channel = CHANNEL,
        /// The demodulator.
        Demod = DEMOD,
        /// The squelch.
        Squelch = SQUELCH,
        /// De-emphasis.
        Deemphasis = DEEMPHASIS,
        /// The scan.
        Scan = SCAN,
        /// The scan list.
        ScanChannels = SCAN_CHANNELS,
        /// The scan dwell.
        ScanDwell = SCAN_DWELL_S,
        /// The scan hang time.
        ScanHang = SCAN_HANG_S,
        /// The most carriers followed.
        Carriers = CARRIERS_SETTING,
        /// Only the channels heard.
        ShowHeard = SHOW_HEARD,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
        /// The carrier-to-noise ratio of the test signal.
        GenCnr = GEN_CNR_DB,
        /// The scene of the test signal.
        GenScene = GEN_SCENE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`CarrierFinder`].
    pub enum FinderSetting {
        /// The channel plan.
        Plan = PLAN,
        /// The raster step.
        RasterStep = RASTER_STEP_HZ,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN_HZ,
        /// The squelch.
        Squelch = SQUELCH,
        /// The most carriers followed.
        Carriers = CARRIERS_SETTING,
    }
}

sdrkit_core::settings! {
    /// The settings of [`CarrierBank`].
    pub enum BankSetting {
        /// The channel plan.
        Plan = PLAN,
        /// The raster step.
        RasterStep = RASTER_STEP_HZ,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN_HZ,
        /// The width of a channel off the plan.
        ChannelWidth = CHANNEL_WIDTH_HZ,
        /// The demodulator.
        Demod = DEMOD,
        /// The squelch.
        Squelch = SQUELCH,
        /// De-emphasis.
        Deemphasis = DEEMPHASIS,
    }
}

sdrkit_core::settings! {
    /// The settings of [`ChannelPlayer`].
    pub enum PlayerSetting {
        /// The channel plan.
        Plan = PLAN,
        /// The raster step.
        RasterStep = RASTER_STEP_HZ,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN_HZ,
        /// The channel to play.
        Channel = CHANNEL,
        /// The scan.
        Scan = SCAN,
        /// The scan list.
        ScanChannels = SCAN_CHANNELS,
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
        /// The raster step.
        RasterStep = RASTER_STEP_HZ,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN_HZ,
        /// The scan.
        Scan = SCAN,
        /// The scan list.
        ScanChannels = SCAN_CHANNELS,
        /// Only the channels heard.
        ShowHeard = SHOW_HEARD,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`CallLog`].
    pub enum LogCommand {
        /// Forget every call and the channel statistics.
        Clear = objcmd::CLEAR,
    }
}

/// Why a plan file is not loaded, for the status.
fn fault_text(e: &SettingError) -> String {
    match e {
        SettingError::Storage { source, .. } => source.to_string(),
        _ => "not a plan file".into(),
    }
}

/// A plan file that a text setting names: its path in the host's storage and what was read from it.
#[derive(Clone)]
pub(crate) struct PlanFile<T> {
    /// The setting, and its id once it was applied.
    pub(crate) path: String,
    id: Option<SettingId>,
    /// The host's storage, from the run context.
    storage: Option<Arc<dyn Storage>>,
    load: fn(&str) -> Result<T, PlanError>,
    pub(crate) plan: Option<Arc<T>>,
    /// Why the plan file is not loaded.
    pub(crate) fault: Option<String>,
}

impl<T> PlanFile<T> {
    /// No file; `load` reads the text of one.
    pub(crate) fn new(load: fn(&str) -> Result<T, PlanError>) -> PlanFile<T> {
        PlanFile {
            path: String::new(),
            id: None,
            storage: None,
            load,
            plan: None,
            fault: None,
        }
    }

    /// The plan of the file `path` in `storage`.
    fn read(&self, storage: &dyn Storage, id: SettingId, path: &str) -> Result<T, SettingError> {
        let malformed = SettingError::Malformed { id, at: None };
        let Ok(file) = StorePath::new(path) else {
            return Err(malformed);
        };
        let bytes = storage
            .read(&file)
            .map_err(|source| SettingError::Storage {
                id,
                path: file,
                source,
            })?;
        (self.load)(&String::from_utf8_lossy(&bytes)).map_err(|_| malformed)
    }

    /// Sets the plan file from the value `v` of `setting`. A file that cannot be read yet is read again when the host's
    /// storage comes; until then the stack runs without a plan. Returns whether the plan changed.
    ///
    /// # Errors
    ///
    /// [`SettingError::Malformed`] if the text is not a storage path or the file is not a plan file. The plan in use
    /// stays.
    pub(crate) fn set_path(
        &mut self,
        setting: &Setting<field::Text>,
        id: SettingId,
        v: &Value,
    ) -> Result<bool, SettingError> {
        let path = setting.read(id, v)?.trim();
        self.id = Some(id);
        if path.is_empty() {
            self.path.clear();
            self.fault = None;
            return Ok(self.plan.take().is_some());
        }
        if path == self.path && self.plan.is_some() {
            return Ok(false);
        }
        if StorePath::new(path).is_err() {
            return Err(SettingError::Malformed { id, at: None });
        }
        match self.storage.as_deref().map(|s| self.read(s, id, path)) {
            Some(Ok(p)) => {
                self.plan = Some(Arc::new(p));
                self.fault = None;
            }
            // The file may come later, or with the host's storage: the status says why no plan is loaded.
            Some(Err(e @ SettingError::Storage { .. })) => {
                self.plan = None;
                self.fault = Some(fault_text(&e));
            }
            Some(Err(e)) => return Err(e),
            None => {
                self.plan = None;
                self.fault = None;
            }
        }
        path.clone_into(&mut self.path);
        Ok(true)
    }

    /// Takes the host's storage and reads a plan file that waits for it. Returns whether the plan changed.
    pub(crate) fn attach(&mut self, ctx: &RunContext) -> bool {
        self.storage = Some(Arc::clone(&ctx.storage));
        let (Some(id), true) = (self.id, self.plan.is_none() && !self.path.is_empty()) else {
            return false;
        };
        match self.read(&*ctx.storage, id, &self.path) {
            Ok(p) => {
                self.plan = Some(Arc::new(p));
                self.fault = None;
                true
            }
            Err(e) => {
                self.fault = Some(fault_text(&e));
                false
            }
        }
    }

    /// What the status says of a plan that is not loaded. Empty with a plan.
    pub(crate) fn status(&self) -> String {
        match (&self.fault, &self.plan) {
            (Some(why), _) => format!("; the channel plan {} is not loaded: {why}", self.path),
            (None, None) => "; no channel plan loaded".into(),
            (None, Some(_)) => String::new(),
        }
    }
}

/// What one component knows of the channels: what its stack fixes, the plan file that the `plan` setting names, and the
/// raster settings.
#[derive(Clone)]
struct PlanView {
    profile: Profile,
    file: PlanFile<Plan>,
    /// The raster settings. 0 is automatic.
    step_hz: f64,
    origin_hz: f64,
    width_hz: f64,
}

impl PlanView {
    fn new(profile: Profile) -> PlanView {
        PlanView {
            profile,
            file: PlanFile::new(plans::load),
            step_hz: 0.0,
            origin_hz: 0.0,
            width_hz: 0.0,
        }
    }

    /// The channels of the plan. Empty without a plan.
    fn channels(&self) -> &[Channel] {
        self.file.plan.as_ref().map_or(&[], |p| &p.channels)
    }

    /// The channel with key `key`.
    fn channel(&self, key: i64) -> Option<&Channel> {
        usize::try_from(key)
            .ok()
            .and_then(|i| self.channels().get(i))
    }

    /// The raster step: the setting, else the narrowest channel of the plan, else the step of the profile. A sideband
    /// service keeps the step of the profile whatever the plan.
    fn step(&self) -> f64 {
        if self.step_hz > 0.0 {
            return self.step_hz;
        }
        self.file
            .plan
            .as_ref()
            .filter(|_| !self.profile.sideband)
            .and_then(|p| p.step_hz())
            .unwrap_or(self.profile.step_hz)
    }

    /// A frequency of the raster: the setting, else the first channel of the plan, else 0 Hz.
    fn origin(&self) -> f64 {
        if self.origin_hz > 0.0 {
            return self.origin_hz;
        }
        self.file
            .plan
            .as_ref()
            .and_then(|p| p.origin_hz())
            .unwrap_or(0.0)
    }

    /// The width of a channel off the plan.
    fn width(&self) -> f64 {
        if self.width_hz > 0.0 {
            self.width_hz
        } else {
            self.profile.width_hz
        }
    }

    /// Takes the host's storage. Returns whether the plan changed.
    fn attach(&mut self, ctx: &RunContext) -> bool {
        self.file.attach(ctx)
    }

    /// Applies the plan or raster setting `which`. Returns whether the channels or the raster changed.
    fn apply(&mut self, which: Shared, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        let (to, new) = match which {
            Shared::Plan => return self.file.set_path(&PLAN, id, v),
            Shared::Step => (&mut self.step_hz, RASTER_STEP_HZ.read(id, v)?),
            Shared::Origin => (&mut self.origin_hz, RASTER_ORIGIN_HZ.read(id, v)?),
            Shared::Width => (&mut self.width_hz, CHANNEL_WIDTH_HZ.read(id, v)?),
        };
        let changed = *to != new;
        *to = new;
        Ok(changed)
    }
}

/// The settings that every component of the stack has.
#[derive(Clone, Copy)]
enum Shared {
    Plan,
    Step,
    Origin,
    Width,
}

/// The finder on the raster of `view`: the thresholds follow the squelch, the plan channels are known channels (not in a
/// sideband service), carriers stay through a small retune.
fn finder_config(view: &PlanView, squelch: f64, max: usize) -> FinderConfig {
    let on = finder_threshold_db(squelch);
    let known = if view.profile.sideband {
        Vec::new()
    } else {
        view.channels().iter().map(|c| c.hz).collect()
    };
    let step = view.step();
    let search = view.profile.search;
    FinderConfig {
        span: SPAN,
        on_db: on,
        off_db: on - 4.0,
        known,
        resolution_hz: (step / 4.0).min(1000.0),
        cell_half_hz: search.cell_half * step,
        width_half_hz: search.width_half * step,
        report_after_s: search.report_after_s,
        estimate_s: search.estimate_s,
        keep_s: search.keep_s,
        skirt_ratio: 10f64.powf(search.skirt_db / 10.0),
        min_snr_db: search.min_snr_db,
        max,
        keep_on_retune: true,
        ..FinderConfig::new(step, view.origin())
    }
}

/// The channels of the scan list `text` (labels; every channel when empty). Channels that are not in use and channels
/// without speech are never scanned. Without a plan the list is empty.
fn scan_list(plan: Option<&Plan>, text: &str) -> Vec<Chan> {
    let Some(plan) = plan else {
        return Vec::new();
    };
    let labels: Vec<&str> = text
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let all: Vec<usize> = if labels.is_empty() {
        (0..plan.channels.len()).collect()
    } else {
        labels.iter().filter_map(|l| plan.by_label(l)).collect()
    };
    all.into_iter()
        .filter(|&i| !plan.channels[i].skipped && !plan.channels[i].no_speech)
        .map(|i| Chan {
            key: i as i64,
            hz: plan.channels[i].hz,
        })
        .collect()
}

/// Where the radio goes to hear the channel at `hz` alone: beside the channel, so that the carrier of the radio is off it.
fn step_centre_hz(view: &PlanView, hz: f64) -> f64 {
    if view.profile.sideband {
        hz + SIDEBAND_TUNE_OFFSET_HZ
    } else {
        hz + view.step() / 2.0
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The finder.

/// The carrier finder (`voice_channels.finder`): the shared finder on the raster of the plan or of the settings.
pub struct CarrierFinder {
    view: PlanView,
    finder: Finder,
    squelch: f64,
    max: usize,
    input: Option<Format>,
    /// The time of the last list, and the time the finder's own clock started at.
    last: f64,
    base: f64,
    outs: Vec<Timed<Carriers>>,
}

impl CarrierFinder {
    /// A finder for `service`.
    pub fn new(service: Service) -> CarrierFinder {
        Self::with_profile(plans::profile(service))
    }

    /// A finder for carriers of `profile`.
    pub fn with_profile(profile: Profile) -> CarrierFinder {
        let view = PlanView::new(profile);
        CarrierFinder {
            finder: Finder::new(finder_config(&view, 50.0, 10)),
            view,
            squelch: 50.0,
            max: 10,
            input: None,
            last: 0.0,
            base: 0.0,
            outs: Vec::new(),
        }
    }

    /// Starts again on the plan and the raster in force. The carriers go; the time runs on.
    fn replan(&mut self) {
        *self.finder.config_mut() = finder_config(&self.view, self.squelch, self.max);
        if let Some(f) = &self.input {
            self.base = self.last;
            self.finder.configure(f);
            self.finder.drop_carriers();
        }
    }
}

impl Component for CarrierFinder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "voice_channels.finder",
            vec![Port::iq("in"), Port::typed::<PlayState>("playing")],
            vec![Port::typed::<Carriers>("carriers").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(FinderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.input = Some(inputs[0].clone());
        self.base = 0.0;
        self.last = 0.0;
        Ok(vec![self.finder.configure(&inputs[0])])
    }
    fn attach(&mut self, ctx: &RunContext) {
        if self.view.attach(ctx) {
            self.replan();
        }
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(p) = io.items::<PlayState>(1).last() {
            self.finder.set_playing(p.value.playing.map(|c| c.hz));
        }
        let mut outs = std::mem::take(&mut self.outs);
        self.finder.process(io.iq(0), &mut outs);
        for o in &mut outs {
            o.time += self.base;
            self.last = o.time;
        }
        io.items_out::<Carriers>(0).append(&mut outs);
        self.outs = outs;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let setting = FinderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        match setting {
            FinderSetting::Plan | FinderSetting::RasterStep | FinderSetting::RasterOrigin => {
                let which = match setting {
                    FinderSetting::Plan => Shared::Plan,
                    FinderSetting::RasterStep => Shared::Step,
                    _ => Shared::Origin,
                };
                if self.view.apply(which, id, v)? {
                    self.replan();
                }
            }
            FinderSetting::Squelch => {
                self.squelch = SQUELCH.read(id, v)?;
                let on = finder_threshold_db(self.squelch);
                let c = self.finder.config_mut();
                c.on_db = on;
                c.off_db = on - 4.0;
            }
            FinderSetting::Carriers => {
                self.max = CARRIERS_SETTING.read(id, v)? as usize;
                self.finder.config_mut().max = self.max;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                FinderSetting::Plan,
                Value::Text(self.view.file.path.clone()),
            ),
            (FinderSetting::RasterStep, Value::Hz(self.view.step_hz)),
            (FinderSetting::RasterOrigin, Value::Hz(self.view.origin_hz)),
            (FinderSetting::Squelch, Value::F64(self.squelch)),
            (FinderSetting::Carriers, Value::I64(self.max as i64)),
        ])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The bank.

/// One 10 ms block of one channel as the bank's second output gives it: what the call log and a signalling decoder need.
#[derive(Clone, Debug)]
pub struct VoiceBlock {
    /// The channel key: the index of a plan channel, or [`OFF_PLAN`] and up for a carrier off the plan.
    pub key: i64,
    /// The channel frequency, Hz.
    pub freq_hz: f64,
    /// The signal time of the block, s.
    pub time: f64,
    /// The squelch is open.
    pub open: bool,
    /// The carrier finder holds the carrier active.
    pub active: bool,
    /// The channel level, dBFS.
    pub level_dbfs: f64,
    /// The demodulator in use.
    pub demod: Demod,
    /// The tone or code the channel carries now.
    pub tone: Option<ToneRead>,
    /// The voice band of the discriminator (300 to 3000 Hz, before de-emphasis) while the squelch is open. Empty without FM
    /// and while it is closed.
    pub flat: Vec<f32>,
}

/// The blocks of one pass and the tone changes of each channel (with the channel frequency, Hz).
#[derive(Clone, Debug, Default)]
pub struct VoiceBlocks {
    /// The blocks, in the order of the [`Blocks`] of the same pass.
    pub blocks: Vec<VoiceBlock>,
    /// The changes of the sub-audible tones.
    pub tones: Vec<(f64, SubAudioEvent)>,
}
impl Item for VoiceBlocks {
    const KIND: &'static str = "voice_channels.blocks";
}

/// Words that another component of the graph adds to the summary of the call log ("2 bursts").
#[derive(Clone, Debug, Default)]
pub struct SummaryNote(pub String);
impl Item for SummaryNote {
    const KIND: &'static str = "voice_channels.note";
}

struct Chain {
    key: i64,
    /// The channel frequency (the plan's, or the raster cell of a carrier off the plan).
    freq: f64,
    /// Where the channeliser sits, absolute, Hz.
    tuned: f64,
    /// The channeliser stays on the channel frequency.
    fixed: bool,
    /// The raster cell of the carrier, Hz.
    cell: f64,
    active: bool,
    chan: Channelizer,
    rx: VoiceRx,
    /// Channel samples to drop first, so that the blocks of every chain start on the same 10 ms raster.
    skip: usize,
    /// The number of the next block on that raster.
    next: i64,
    /// The blocks not yet given out: a block goes out when every chain has made its block of that number.
    queue: VecDeque<(ChanBlock, VoiceBlock)>,
}

/// The carrier bank (`voice_channels.bank`): per carrier the channel at 48 kHz and a voice channel receiver. A new carrier's
/// receiver is first fed the last 0.25 s of the span.
pub struct CarrierBank {
    view: PlanView,
    rate: f64,
    centre: f64,
    chains: BTreeMap<u64, Chain>,
    squelch: f64,
    deemph: bool,
    demod: DemodChoice,
    count: u64,
    preroll: VecDeque<Complex32>,
    x: Vec<Complex32>,
    rx_out: Vec<RxBlock>,
}

impl CarrierBank {
    /// A bank for `service`.
    pub fn new(service: Service) -> CarrierBank {
        Self::with_profile(plans::profile(service))
    }

    /// A bank for carriers of `profile`.
    pub fn with_profile(profile: Profile) -> CarrierBank {
        CarrierBank {
            view: PlanView::new(profile),
            rate: 0.0,
            centre: 0.0,
            chains: BTreeMap::new(),
            squelch: 50.0,
            deemph: true,
            demod: DemodChoice::Auto,
            count: 0,
            preroll: VecDeque::new(),
            x: Vec::new(),
            rx_out: Vec::new(),
        }
    }

    fn channelizer(&self, tuned: f64) -> Option<Channelizer> {
        let mut cfg = ChannelConfig::new(self.rate, tuned - self.centre, 16e3, AUDIO_RATE);
        cfg.exact_rate = true;
        cfg.atten_db = 60.0;
        Channelizer::new(cfg).ok()
    }

    /// The receiver chain of the new carrier `c`, or `None` for a carrier that gets none: a second cell of a signal that has
    /// a receiver, or one carrier off the plan too many.
    fn chain(&self, c: &Carrier, t0: f64) -> Option<Chain> {
        let view = &self.view;
        let profile = view.profile;
        let sideband = profile.sideband;
        let step = view.step();
        // On a raster that names the channels, the cell names the channel, whatever sideband the signal is on.
        let at = if profile.on_raster {
            c.freq_hz
        } else {
            c.centroid_hz
        };
        let hit = view
            .file
            .plan
            .as_ref()
            .filter(|_| self.centre > 0.0)
            .and_then(|p| p.match_carrier(at, sideband));
        let holder = |i: usize| self.chains.values().find(|ch| ch.key == i as i64);
        let on_plan = match hit {
            Some(i) => match holder(i) {
                None => Some(i),
                // One signal over two cells, or more speech of one sideband: the channel has its receiver.
                Some(h) if sideband || (at - h.tuned).abs() < 1.5 * step => return None,
                Some(_) => None,
            },
            None => None,
        };
        if on_plan.is_none() {
            // More cells of a signal that has a receiver: more speech of one sideband, or the rest of a signal wider than
            // a cell.
            let part = self.chains.values().any(|ch| {
                if sideband {
                    // The frequency of a channel off the plan is an estimate that can be 1 kHz off.
                    let slack = if ch.key >= OFF_PLAN { 1000.0 } else { 0.0 };
                    (-800.0 - slack..=3600.0 + slack).contains(&(c.centroid_hz - ch.freq))
                } else {
                    !profile.on_raster
                        && ch.key >= OFF_PLAN
                        && (c.centroid_hz - ch.tuned).abs() < 0.5 * view.width()
                }
            });
            if part {
                return None;
            }
        }
        let (key, freq, demod, width) =
            match on_plan.and_then(|i| Some((i, view.channel(i as i64)?))) {
                // A channel without speech is watched with the FM squelch.
                Some((i, ch)) if ch.no_speech => (i as i64, ch.hz, Some(Demod::Fm), ch.width_hz),
                Some((i, ch)) => (i as i64, ch.hz, ch.demod.or(profile.demod), ch.width_hz),
                None => {
                    let off = self.chains.values().filter(|ch| ch.key >= OFF_PLAN).count();
                    if off >= OFF_PLAN_MAX {
                        return None;
                    }
                    // An estimate: the first cell of a sideband signal that the finder reports holds the speech from 500 to
                    // 1500 Hz above the channel frequency.
                    let freq = if sideband {
                        c.freq_hz - (1000.0 / step).round() * step
                    } else {
                        c.freq_hz
                    };
                    let key = OFF_PLAN + (freq / 100.0).round() as i64;
                    (key, freq, profile.demod, view.width())
                }
            };
        let fixed = sideband || profile.on_raster;
        let tuned = if fixed { freq } else { c.centroid_hz };
        let next = (t0 * AUDIO_RATE / BLOCK as f64 - 1e-6).ceil() as i64;
        let rx = VoiceRx::new(RxConfig {
            cutoff_hz: profile.cutoff_hz(width),
            plan: demod,
            forced: self.demod.forced(),
            squelch: self.squelch,
            deemphasis: self.deemph,
            fm_squelch: !sideband,
            tones: !sideband,
        });
        Some(Chain {
            key,
            freq,
            tuned,
            fixed,
            cell: c.freq_hz,
            active: c.active,
            chan: self.channelizer(tuned)?,
            rx,
            skip: ((next as f64 * BLOCK as f64 / AUDIO_RATE - t0) * AUDIO_RATE).round() as usize,
            next,
            queue: VecDeque::new(),
        })
    }

    fn retune_all(&mut self) {
        for ch in self.chains.values_mut() {
            ch.rx.set(self.squelch, self.deemph, self.demod.forced());
        }
    }
}

/// Runs `x` through `ch`. Its blocks go to its queue, its tone changes to `voice`.
fn run_chain(
    ch: &mut Chain,
    x: &[Complex32],
    scratch: &mut Vec<Complex32>,
    rx_out: &mut Vec<RxBlock>,
    voice: &mut VoiceBlocks,
) {
    scratch.clear();
    ch.chan.process(x, scratch);
    let drop = ch.skip.min(scratch.len());
    ch.skip -= drop;
    rx_out.clear();
    ch.rx.process(&scratch[drop..], ch.active, rx_out);
    for b in rx_out.drain(..) {
        let time = ch.next as f64 * BLOCK as f64 / AUDIO_RATE;
        ch.next += 1;
        // A tone belongs to a channel that is heard as FM.
        let tone = ch.rx.tone().filter(|_| b.demod == Demod::Fm).cloned();
        ch.queue.push_back((
            ChanBlock {
                key: ch.key,
                freq_hz: ch.freq,
                time,
                open: b.open,
                level_dbfs: b.level_dbfs,
                measure_db: b.measure_db,
                audio: b.audio,
            },
            VoiceBlock {
                key: ch.key,
                freq_hz: ch.freq,
                time,
                open: b.open,
                active: ch.active,
                level_dbfs: b.level_dbfs,
                demod: b.demod,
                tone,
                flat: if b.open { b.flat } else { Vec::new() },
            },
        ));
    }
    for e in ch.rx.take_tone_events() {
        voice.tones.push((ch.freq, e));
    }
}

impl Component for CarrierBank {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "voice_channels.bank",
            vec![Port::iq("in"), Port::typed::<Carriers>("carriers")],
            vec![
                Port::typed::<Blocks>("blocks").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<VoiceBlocks>("voice").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(BankSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        // A small retune keeps the receivers, as the finder keeps their carriers.
        let keep = self.rate == f.rate
            && f.centre_hz > 0.0
            && self.centre > 0.0
            && (f.centre_hz - self.centre).abs() < 0.45 * f.rate;
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.count = 0;
        self.preroll.clear();
        if keep {
            for ch in self.chains.values_mut() {
                ch.chan.set_offset(ch.tuned - f.centre_hz);
                ch.next = 0;
                ch.queue.clear();
            }
        } else {
            self.chains.clear();
        }
        Ok(vec![
            Format::new(Kind::typed::<Blocks>(), 0.0),
            Format::new(Kind::typed::<VoiceBlocks>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        if self.view.attach(ctx) {
            self.chains.clear();
        }
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let t_in = self.count as f64 / self.rate;
        self.count += x.len() as u64;
        let mut blocks = Vec::new();
        let mut voice = VoiceBlocks::default();
        let mut scratch = std::mem::take(&mut self.x);
        let mut rx_out = std::mem::take(&mut self.rx_out);
        if let Some(cl) = io.items::<Carriers>(1).last() {
            let step = self.view.step();
            self.chains
                .retain(|id, _| cl.value.0.iter().any(|c| c.id == *id));
            for c in &cl.value.0 {
                // The finder gives the number of a quiet carrier to a signal that starts in the next cell: a carrier on
                // another cell gets a new receiver.
                let moved = self
                    .chains
                    .get(&c.id)
                    .is_some_and(|ch| (c.freq_hz - ch.cell).abs() > 1.0);
                if moved {
                    self.chains.remove(&c.id);
                }
                if let Some(ch) = self.chains.get_mut(&c.id) {
                    ch.active = c.active;
                    if !ch.fixed && c.active && (c.centroid_hz - ch.tuned).abs() > 0.06 * step {
                        ch.tuned = c.centroid_hz;
                        ch.chan.set_offset(ch.tuned - self.centre);
                    }
                    continue;
                }
                let before = self.preroll.len() as f64 / self.rate;
                if let Some(mut ch) = self.chain(c, t_in - before) {
                    let (a, b) = self.preroll.as_slices();
                    run_chain(&mut ch, a, &mut scratch, &mut rx_out, &mut voice);
                    run_chain(&mut ch, b, &mut scratch, &mut rx_out, &mut voice);
                    self.chains.insert(c.id, ch);
                }
            }
        }
        for ch in self.chains.values_mut() {
            run_chain(ch, x, &mut scratch, &mut rx_out, &mut voice);
        }
        // The player takes one block of each channel for a 10 ms step: a block goes out when every chain has made it, and
        // when it is as old as the stored span.
        let due =
            ((self.count as f64 / self.rate - HOLD_S) * AUDIO_RATE / BLOCK as f64).floor() as i64;
        let ready = self
            .chains
            .values()
            .map(|ch| ch.next)
            .min()
            .unwrap_or(0)
            .min(due + 1);
        for ch in self.chains.values_mut() {
            let behind = (ch.next - ready).max(0) as usize;
            let n = ch.queue.len().saturating_sub(behind);
            for (b, v) in ch.queue.drain(..n) {
                blocks.push(b);
                voice.blocks.push(v);
            }
        }
        self.x = scratch;
        self.rx_out = rx_out;
        let keep = (PREROLL_S * self.rate) as usize;
        self.preroll.extend(x.iter().copied());
        let extra = self.preroll.len().saturating_sub(keep);
        self.preroll.drain(..extra);
        if !blocks.is_empty() {
            io.items_out::<Blocks>(0).push(Timed {
                time: t_in,
                value: Blocks(blocks),
            });
        }
        if !voice.blocks.is_empty() || !voice.tones.is_empty() {
            io.items_out::<VoiceBlocks>(1).push(Timed {
                time: t_in,
                value: voice,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let setting = BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        match setting {
            BankSetting::Plan
            | BankSetting::RasterStep
            | BankSetting::RasterOrigin
            | BankSetting::ChannelWidth => {
                let which = match setting {
                    BankSetting::Plan => Shared::Plan,
                    BankSetting::RasterStep => Shared::Step,
                    BankSetting::RasterOrigin => Shared::Origin,
                    _ => Shared::Width,
                };
                if self.view.apply(which, id, v)? {
                    self.chains.clear();
                }
            }
            BankSetting::Demod => self.demod = DEMOD.read(id, v)?,
            BankSetting::Squelch => self.squelch = SQUELCH.read(id, v)?,
            BankSetting::Deemphasis => self.deemph = DEEMPHASIS.read(id, v)?,
        }
        self.retune_all();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BankSetting::Plan, Value::Text(self.view.file.path.clone())),
            (BankSetting::RasterStep, Value::Hz(self.view.step_hz)),
            (BankSetting::RasterOrigin, Value::Hz(self.view.origin_hz)),
            (BankSetting::ChannelWidth, Value::Hz(self.view.width_hz)),
            (BankSetting::Demod, Value::Choice(self.demod.index())),
            (BankSetting::Squelch, Value::F64(self.squelch)),
            (BankSetting::Deemphasis, Value::Bool(self.deemph)),
        ])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The player.

/// The plan as the player sees it.
struct VoiceScanPlan {
    plan: Option<Arc<Plan>>,
    sideband: bool,
    channel: String,
    scan_channels: String,
}

impl ScanPlan for VoiceScanPlan {
    fn scan_list(&self) -> Vec<Chan> {
        scan_list(self.plan.as_deref(), &self.scan_channels)
    }
    /// The picked channel. A label that the plan does not have leaves the choice to the player.
    fn picked(&self) -> Option<Vec<Chan>> {
        let plan = self.plan.as_ref()?;
        let i = plan.by_label(&self.channel)?;
        Some(vec![Chan {
            key: i as i64,
            hz: plan.channels[i].hz,
        }])
    }
    fn muted(&self, c: Chan) -> bool {
        usize::try_from(c.key)
            .ok()
            .and_then(|i| self.plan.as_ref()?.channels.get(i))
            .is_some_and(|ch| ch.no_speech)
    }
    fn always_single_tuner(&self) -> bool {
        self.sideband
    }
}

/// The player (`voice_channels.player`): what plays and the scan. A sideband service scans by stepping the radio.
pub struct ChannelPlayer {
    view: PlanView,
    channel: String,
    scan_channels: String,
    player: Player,
    carriers: Option<Format>,
    audio: Vec<f32>,
}

impl ChannelPlayer {
    /// A player for `service`.
    pub fn new(service: Service) -> ChannelPlayer {
        Self::with_profile(plans::profile(service))
    }

    /// A player for carriers of `profile`.
    pub fn with_profile(profile: Profile) -> ChannelPlayer {
        let view = PlanView::new(profile);
        let mut p = ChannelPlayer {
            player: Player::new(Box::new(VoiceScanPlan {
                plan: None,
                sideband: view.profile.sideband,
                channel: "auto".into(),
                scan_channels: String::new(),
            })),
            view,
            channel: "auto".into(),
            scan_channels: String::new(),
            carriers: None,
            audio: Vec::new(),
        };
        p.set_plan();
        p
    }

    fn set_plan(&mut self) {
        self.player.lag = HOLD_S;
        // The radio needs time to settle after a step of a scan that always steps it.
        self.player.settle = if self.view.profile.sideband { 0.5 } else { 0.0 };
        self.player.plan = Box::new(VoiceScanPlan {
            plan: self.view.file.plan.clone(),
            sideband: self.view.profile.sideband,
            channel: self.channel.clone(),
            scan_channels: self.scan_channels.clone(),
        });
    }

    /// Starts again on the plan and the raster in force: the channel states go.
    fn replan(&mut self) {
        self.set_plan();
        let held = self
            .carriers
            .as_ref()
            .and_then(|f| finder_config(&self.view, 50.0, 1).held(f));
        self.player.configure(held);
    }
}

impl Component for ChannelPlayer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "voice_channels.player",
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
        self.carriers = Some(inputs[1].clone());
        self.player
            .configure(finder_config(&self.view, 50.0, 1).held(&inputs[1]));
        Ok(vec![
            Format::new(Kind::Audio, AUDIO_RATE),
            Format::new(Kind::typed::<PlayState>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        if self.view.attach(ctx) {
            self.replan();
        }
    }
    fn process(&mut self, io: &mut Io) {
        let mut audio = std::mem::take(&mut self.audio);
        audio.clear();
        let changed =
            self.player
                .process(io.items::<Blocks>(0), io.items::<Carriers>(1), &mut audio);
        io.audio_out(0).samples.extend_from_slice(&audio);
        self.audio = audio;
        if io.tick() || changed {
            io.items_out::<PlayState>(1).push(Timed {
                time: self.player.clock(),
                value: self.player.state(),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let setting = PlayerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        match setting {
            PlayerSetting::Plan | PlayerSetting::RasterStep | PlayerSetting::RasterOrigin => {
                let which = match setting {
                    PlayerSetting::Plan => Shared::Plan,
                    PlayerSetting::RasterStep => Shared::Step,
                    _ => Shared::Origin,
                };
                if self.view.apply(which, id, v)? {
                    self.replan();
                }
            }
            // The label is looked up in the plan in force each time the player chooses.
            PlayerSetting::Channel => CHANNEL.read(id, v)?.trim().clone_into(&mut self.channel),
            PlayerSetting::Scan => self.player.scan = SCAN.read(id, v)?,
            PlayerSetting::ScanChannels => SCAN_CHANNELS
                .read(id, v)?
                .clone_into(&mut self.scan_channels),
            PlayerSetting::ScanDwell => self.player.dwell = SCAN_DWELL_S.read(id, v)?,
            PlayerSetting::ScanHang => self.player.hang = SCAN_HANG_S.read(id, v)?,
        }
        self.set_plan();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                PlayerSetting::Plan,
                Value::Text(self.view.file.path.clone()),
            ),
            (PlayerSetting::RasterStep, Value::Hz(self.view.step_hz)),
            (PlayerSetting::RasterOrigin, Value::Hz(self.view.origin_hz)),
            (PlayerSetting::Channel, Value::Text(self.channel.clone())),
            (PlayerSetting::Scan, Value::Bool(self.player.scan)),
            (
                PlayerSetting::ScanChannels,
                Value::Text(self.scan_channels.clone()),
            ),
            (PlayerSetting::ScanDwell, Value::Duration(self.player.dwell)),
            (PlayerSetting::ScanHang, Value::Duration(self.player.hang)),
        ])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Fields, tables and events.

/// The most calls the log keeps.
pub const CALL_CAP: usize = 600;

/// What the voice channel stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
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

impl Shaped for LoggedCall {
    type Row = CallRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let c = &self.0;
        out.put(&CallRow {
            serial: c.serial as i64,
            time_utc: c.at.wall.map(Time),
            key: c.key,
            number: i64::from(c.number),
            label: &c.label,
            hz: Hertz(c.freq_hz),
            start_s: Seconds(c.at.t),
            duration_s: Seconds(c.duration_s),
            peak_dbfs: c.peak_dbfs,
            mean_dbfs: c.mean_dbfs,
            demod: &c.demod,
            tone: &c.tone,
            open: c.open,
        });
    }
}

/// What the call log keeps across sessions: the calls and the serial of the last one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallLists {
    /// The calls, oldest first.
    pub calls: LogRing<LoggedCall>,
    /// The serial of the last call.
    pub serial: u64,
}

impl Default for CallLists {
    fn default() -> Self {
        CallLists {
            calls: LogRing::new(CALL_CAP),
            serial: 0,
        }
    }
}

impl Shift for CallLists {
    fn shift_times(&mut self, by_s: f64) {
        self.calls.shift_times(by_s);
    }
}

impl Tables for CallLists {
    const FORMAT: StateFormat = StateFormat::new("voice.channel_calls", 1);
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

// ---------------------------------------------------------------------------------------------------------------------------------
// The log.

/// What a channel does now.
#[derive(Clone, Debug)]
struct Live {
    hz: f64,
    seen: f64,
    active: bool,
    demod: Demod,
    tone: Option<ToneRead>,
    last_tone: String,
}

/// What a stack makes of the call log.
#[derive(Clone, Debug)]
pub struct LogStyle {
    /// The name of the stack or the service, for messages.
    pub title: Cow<'static, str>,
    /// The service, for a stack that has services.
    pub service: Option<Service>,
    /// Makes the event of an ended call.
    pub call: fn(&Call) -> Event,
    /// The stack takes a channel plan: the status says when none is loaded.
    pub plan: bool,
}

impl LogStyle {
    /// The call log of the voice channel stack for `service`.
    pub fn of(service: Service) -> LogStyle {
        LogStyle {
            title: service.title(),
            service: Some(service),
            call: call_event,
            plan: true,
        }
    }
}

/// The event of the ended call `c` of the voice channel stack.
fn call_event(c: &Call) -> Event {
    VOICE_CHANNELS_CALL.event(
        c.at.t,
        c.freq_hz,
        &VoiceChannelsCallEvent {
            serial: c.serial as i64,
            key: c.key,
            number: i64::from(c.number),
            label: &c.label,
            start_s: Seconds(c.at.t),
            duration_s: Seconds(c.duration_s),
            peak_dbfs: c.peak_dbfs,
            mean_dbfs: c.mean_dbfs,
            demod: &c.demod,
            tone: &c.tone,
        },
    )
}

/// The call log (`voice_channels.log`): the calls, the channel rows, the results, and the requests to the radio for the
/// steps of a scan that the capture does not hold.
pub struct CallLog {
    p: Published,
    view: PlanView,
    style: LogStyle,
    /// The words of the `note` input.
    note: String,
    time_base: f64,
    book: CallBook,
    lists: CallLists,
    carriers: Vec<Carrier>,
    live: BTreeMap<i64, Live>,
    play: PlayState,
    clock: SignalClock,
    host_clock: sdrkit_core::Clock,
    is_live: bool,
    first_time: Option<f64>,
    reports: u64,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    /// The capture: its rate, its centre and the channels it holds.
    rate: f64,
    centre: f64,
    held: Option<(f64, f64)>,
    scan: bool,
    scan_channels: String,
    asked_hz: Option<f64>,
    rows_stale: bool,
    show_heard: bool,
    order: Vec<(f64, usize, usize)>,
}

impl CallLog {
    /// A log for `service`.
    pub fn new(service: Service) -> CallLog {
        Self::with_profile(plans::profile(service), LogStyle::of(service))
    }

    /// A log for carriers of `profile`, as `style` says.
    pub fn with_profile(profile: Profile, style: LogStyle) -> CallLog {
        let view = PlanView::new(profile);
        let mut p = Published::new(VoiceChannelsFields::SHAPE.fields).with_tables(&[CALLS.def(), CHANNELS.def()]);
        p.results_mut().set_status(&Status {
            summary: format!("{}: waiting for samples", style.title),
            ..Status::default()
        });
        CallLog {
            p,
            view,
            style,
            note: String::new(),
            time_base: 0.0,
            book: CallBook::default(),
            lists: CallLists::default(),
            carriers: Vec::new(),
            live: BTreeMap::new(),
            play: PlayState::default(),
            clock: SignalClock::default(),
            host_clock: sdrkit_core::Clock::fixed(0.0),
            is_live: false,
            first_time: None,
            reports: 0,
            events: Vec::new(),
            requests: Vec::new(),
            rate: 0.0,
            centre: 0.0,
            held: None,
            scan: false,
            scan_channels: String::new(),
            asked_hz: None,
            rows_stale: true,
            show_heard: false,
            order: Vec::new(),
        }
    }

    /// The calls, oldest first.
    pub fn calls(&self) -> impl Iterator<Item = &Call> {
        self.lists.calls.iter().map(|c| &c.0)
    }

    /// The label and the number of channel `key` at `hz`: the plan's, or the frequency of a carrier off the plan.
    fn label(&self, key: i64, hz: f64) -> (String, u32) {
        match self.view.channel(key) {
            Some(c) => (c.label.clone(), c.number),
            None => (format!("{:.4}", hz / 1e6), 0),
        }
    }

    /// Takes the ended call `c`: its event, its log line and its final row.
    fn ended(&mut self, c: Call) {
        self.events.push((self.style.call)(&c));
        let mut line = format!(
            "{} {} ({:.4} MHz, {}",
            self.style.title,
            if c.key >= OFF_PLAN {
                "carrier"
            } else {
                &c.label
            },
            c.freq_hz / 1e6,
            c.demod
        );
        if !c.tone.is_empty() {
            let _ = write!(line, ", tone {}", c.tone);
        }
        let _ = write!(line, "): {:.1} s, peak {:.0} dB", c.duration_s, c.peak_dbfs);
        self.p.results_mut().log(line);
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
    fn block(&mut self, b: &VoiceBlock, time: f64) {
        let (label, number) = self.label(b.key, b.freq_hz);
        let clock = &self.clock;
        let as_block = ChanBlock {
            key: b.key,
            freq_hz: b.freq_hz,
            time: b.time,
            open: b.open,
            level_dbfs: b.level_dbfs,
            measure_db: 0.0,
            audio: Vec::new(),
        };
        let step = self.book.block(
            &as_block,
            time,
            &mut self.lists.serial,
            || clock.stamp(time),
            |c| {
                c.label = label;
                c.number = number;
            },
        );
        let live = self.live.entry(b.key).or_insert_with(|| {
            self.rows_stale = true;
            Live {
                hz: b.freq_hz,
                seen: time,
                active: b.active,
                demod: b.demod,
                tone: None,
                last_tone: String::new(),
            }
        });
        live.seen = time;
        live.active = b.active;
        live.demod = b.demod;
        live.tone.clone_from(&b.tone);
        if let Some(t) = &b.tone {
            live.last_tone.clone_from(&t.label);
        }
        if b.open {
            if let Some(c) = self.book.open_call_mut(b.key) {
                let name = &Demod::CHOICES[usize::from(b.demod.index())].label;
                if c.demod != *name {
                    c.demod = name.to_string();
                }
                if let Some(t) = &b.tone {
                    c.tone.clone_from(&t.label);
                }
            }
        }
        if let Some(mut c) = step.started {
            c.demod = Demod::CHOICES[usize::from(b.demod.index())]
                .label
                .to_string();
            self.lists.calls.push(LoggedCall(c));
        }
        if let Some(c) = step.ended {
            self.ended(c);
        }
    }

    /// Ends every open call.
    pub fn close_all(&mut self) {
        for c in self.book.end_all() {
            self.ended(c);
        }
    }

    /// Starts again on the plan and the raster in force: the carriers and the channel statistics go, the call log stays.
    fn replan(&mut self) {
        self.close_all();
        self.book.clear();
        self.live.clear();
        self.carriers.clear();
        self.rows_stale = true;
        self.held = None;
    }

    /// The scan steps the radio: a sideband service, or a scan list with a channel outside the capture.
    fn single_tuner(&self) -> bool {
        if !self.scan {
            return false;
        }
        self.view.profile.sideband
            || self.held.is_some_and(|(lo, hi)| {
                scan_list(self.view.file.plan.as_deref(), &self.scan_channels)
                    .iter()
                    .any(|c| c.hz < lo - 1.0 || c.hz > hi + 1.0)
            })
    }

    fn report(&mut self) {
        self.reports += 1;
        let now = self.clock.now();
        let plan = self.view.file.plan.clone();
        let channels: &[Channel] = plan.as_ref().map_or(&[], |p| &p.channels);
        let profile = self.view.profile;
        let any_open = self.book.any_open();
        let lock_state = if now > 0.0 || !self.carriers.is_empty() {
            if any_open {
                LockState::Locked
            } else {
                LockState::Searching
            }
        } else {
            LockState::Off
        };
        // A channel whose carrier went has no blocks: it is not active, and a carrier off the plan without a call goes.
        let stale: Vec<i64> = self
            .live
            .iter()
            .filter(|(_, l)| now - l.seen > 0.5)
            .map(|(k, _)| *k)
            .collect();
        for k in stale {
            let heard = self.book.stats(k).is_some_and(|s| s.calls > 0);
            if k >= OFF_PLAN && !heard {
                self.live.remove(&k);
                self.rows_stale = true;
            } else if let Some(l) = self.live.get_mut(&k) {
                l.active = false;
                l.tone = None;
            }
        }
        let active = self.live.values().filter(|l| l.active).count();
        let open = self.book.all_stats().filter(|(_, s)| s.open).count();
        let playing = self.play.playing.map(|c| self.label(c.key, c.hz).0);
        let in_scan: Vec<i64> = scan_list(plan.as_deref(), &self.scan_channels)
            .iter()
            .map(|c| c.key)
            .collect();
        let single = self.single_tuner();
        let step = self.view.step();
        let minute = self.clock.wall(now).map(|w| ((w / 60.0) as u64 % 60) as u8);
        let p = &mut self.p;
        p.put(&VoiceChannelsFields {
            submode: self.style.service,
            plan: plan.as_ref().map(|p| p.name.as_str()),
            plan_channels: channels.len() as i64,
            raster_step_hz: Hertz(step),
            carriers: self.carriers.len() as i64,
            active_channels: active as i64,
            open_channels: open as i64,
            playing: playing.as_deref(),
            playing_hz: self.play.playing.map(|c| Hertz(c.hz)),
            scan: self.scan,
            scan_state: self.play.scan_state,
            single_tuner: single,
            tune_hz: self.play.tune_hz.map(Hertz),
            multi_channel: self.rate >= 100e3,
            calls: self.lists.serial as i64,
            input_rate_hz: Hertz(self.rate),
            centre_hz: Hertz(self.centre),
            signal_time_s: Seconds(now),
            reports: self.reports as i64,
        });
        // The row of an open call shows the call as it is now.
        for k in self.live.keys() {
            let Some(c) = self.book.open_call(*k) else {
                continue;
            };
            let entry = self.lists.calls.iter_mut().rev();
            if let Some(e) = entry.take(32).find(|e| e.0.serial == c.serial) {
                e.0.clone_from(c);
            }
        }
        shaped::log_rows(&self.lists.calls, &CALLS, now, p);
        let mut id = String::new();
        let mut keep: Vec<i64> = Vec::new();
        let off: Vec<i64> = self.live.range(OFF_PLAN..).map(|(k, _)| *k).collect();
        for k in (0..channels.len() as i64).chain(off) {
            let c = usize::try_from(k).ok().and_then(|i| channels.get(i));
            let live = self.live.get(&k);
            let st = self.book.stats(k).cloned().unwrap_or_default();
            let active = live.is_some_and(|l| l.active);
            if self.show_heard && !active && st.calls == 0 {
                continue;
            }
            keep.push(k);
            let total = st.total + self.book.open_call(k).map_or(0.0, |c| c.duration_s);
            id.clear();
            let _ = write!(id, "{k:07}");
            let hz = c.map_or_else(|| live.map_or(0.0, |l| l.hz), |c| c.hz);
            let label = match c {
                Some(c) => Cow::Borrowed(c.label.as_str()),
                None => Cow::Owned(format!("{:.4}", hz / 1e6)),
            };
            let plan_demod = c
                .and_then(|c| c.demod.or(c.no_speech.then_some(Demod::Fm)))
                .or(profile.demod)
                .unwrap_or(Demod::Am);
            let tone = live.and_then(|l| l.tone.as_ref());
            CHANNELS.put(
                p,
                &id,
                &ChannelRow {
                    key: k,
                    number: c.map_or(0, |c| i64::from(c.number)),
                    label: &label,
                    name: c.map_or("", |c| c.name.as_str()),
                    hz: Hertz(hz),
                    input_hz: c.and_then(|c| c.input_hz).map(Hertz),
                    active,
                    open: st.open,
                    playing: self.play.playing.is_some_and(|p| p.key == k),
                    in_scan: in_scan.contains(&k),
                    on_air: c.is_none_or(|c| minute.is_none_or(|m| c.on_air(m))),
                    level_dbfs: st.level.filter(|_| active || st.open),
                    demod: live.map_or(plan_demod, |l| l.demod),
                    tone: tone.map_or("", |t| t.label.as_str()),
                    tone_alias: tone.map_or("", |t| t.alias.as_str()),
                    last_tone: live.map_or("", |l| l.last_tone.as_str()),
                    calls: st.calls as i64,
                    total_s: Seconds(total),
                    last_heard_s: st
                        .last
                        .map(|l| Seconds(if st.open { 0.0 } else { (now - l).max(0.0) })),
                    last_heard_utc: st.last.and_then(|l| self.clock.wall(l)).map(Time),
                    peak_dbfs: st.peak,
                },
            );
        }
        if std::mem::take(&mut self.rows_stale) || self.show_heard {
            let table = p.results().decl().table_id(CHANNELS.key);
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
                p.remove_row(&CHANNELS, k);
            }
        }
        let ncalls = self.lists.serial;
        let title = &self.style.title;
        let mut summary = match (&playing, lock_state) {
            (_, LockState::Off) => format!("{title}: waiting for samples"),
            (Some(l), _) => {
                let hz = self.play.playing.map_or(0.0, |c| c.hz);
                format!(
                    "{title}: listening to {l} ({:.4} MHz), {active} active, {ncalls} calls",
                    hz / 1e6
                )
            }
            (None, _) => format!("{title}: {active} channels active, {ncalls} calls"),
        };
        if lock_state != LockState::Off && !self.note.is_empty() {
            let _ = write!(summary, ", {}", self.note);
        }
        if self.style.plan {
            summary.push_str(&self.view.file.status());
        }
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
            "voice_channels.log",
            vec![
                Port::typed::<VoiceBlocks>("voice"),
                Port::typed::<Carriers>("carriers"),
                Port::typed::<PlayState>("play"),
                Port::typed::<SummaryNote>("note").optional("the summary has no words added"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(LogSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The carriers belong to the old capture. The call log stays, and the clock runs on.
        let f = inputs.get(1);
        self.rate = f.map_or(0.0, |f| f.bandwidth_hz / (2.0 * SPAN));
        self.centre = f.map_or(0.0, |f| f.centre_hz);
        self.held = f.and_then(|f| finder_config(&self.view, 50.0, 1).held(f));
        self.carriers.clear();
        self.live.clear();
        self.rows_stale = true;
        self.close_all();
        self.clock.restart(self.is_live, self.host_clock.now());
        self.first_time = None;
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.is_live = ctx.live;
        self.host_clock = ctx.clock.clone();
        if self.view.attach(ctx) {
            self.replan();
        }
    }
    fn process(&mut self, io: &mut Io) {
        if self.held.is_none() && self.rate > 0.0 {
            // The plan or the raster changed in place: the capture is as it was.
            let f = Format {
                centre_hz: self.centre,
                tuned_hz: self.centre,
                bandwidth_hz: 2.0 * SPAN * self.rate,
                ..Format::new(Kind::typed::<Carriers>(), 0.0)
            };
            self.held = finder_config(&self.view, 50.0, 1).held(&f);
        }
        let first = io
            .items::<Carriers>(1)
            .first()
            .map(|c| c.time)
            .into_iter()
            .chain(
                io.items::<VoiceBlocks>(0)
                    .iter()
                    .flat_map(|b| b.value.blocks.first().map(|c| c.time)),
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
        if let Some(n) = io.items::<SummaryNote>(3).last() {
            self.note.clone_from(&n.value.0);
        }
        // The stepping scan requests the host to move the radio.
        if self.play.tune_hz != self.asked_hz {
            self.asked_hz = self.play.tune_hz;
            if let Some(f) = self.asked_hz {
                let label = self
                    .view
                    .file
                    .plan
                    .as_ref()
                    .and_then(|p| Some(p.channels[p.by_frequency(f, 1.0, false)?].label.clone()))
                    .unwrap_or_default();
                self.requests.push(HostRequest::Retune {
                    tuned_hz: step_centre_hz(&self.view, f),
                    reason: format!(
                        "{} scan, {label} ({})",
                        self.style.title,
                        self.play.scan_state.words()
                    ),
                });
            }
        }
        let mut order = std::mem::take(&mut self.order);
        order.clear();
        let items = io.items::<VoiceBlocks>(0);
        for (i, b) in items.iter().enumerate() {
            order.extend(
                b.value
                    .blocks
                    .iter()
                    .enumerate()
                    .map(|(j, vb)| (vb.time, i, j)),
            );
        }
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(time, i, j) in &order {
            let t = self.clock.at(time - t0);
            self.block(&items[i].value.blocks[j], t);
        }
        self.order = order;
        let now = self.clock.now();
        for item in items {
            for (hz, e) in &item.value.tones {
                let mut event = SUBAUDIO_EVENT.event(now, *hz);
                e.write_fields(&mut event.fields);
                self.events.push(event);
            }
        }
        io.messages_out(0).append(&mut self.events);
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
        let setting = LogSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        match setting {
            LogSetting::Plan | LogSetting::RasterStep | LogSetting::RasterOrigin => {
                let which = match setting {
                    LogSetting::Plan => Shared::Plan,
                    LogSetting::RasterStep => Shared::Step,
                    _ => Shared::Origin,
                };
                if self.view.apply(which, id, v)? {
                    self.replan();
                }
            }
            LogSetting::Scan => self.scan = SCAN.read(id, v)?,
            LogSetting::ScanChannels => SCAN_CHANNELS
                .read(id, v)?
                .clone_into(&mut self.scan_channels),
            LogSetting::ShowHeard => {
                self.show_heard = SHOW_HEARD.read(id, v)?;
                self.rows_stale = true;
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
            (LogSetting::Plan, Value::Text(self.view.file.path.clone())),
            (LogSetting::RasterStep, Value::Hz(self.view.step_hz)),
            (LogSetting::RasterOrigin, Value::Hz(self.view.origin_hz)),
            (LogSetting::Scan, Value::Bool(self.scan)),
            (
                LogSetting::ScanChannels,
                Value::Text(self.scan_channels.clone()),
            ),
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
                self.live.clear();
                self.rows_stale = true;
                self.report();
            }
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

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph.

/// The service of submode `id`. No submode is CB.
pub fn service_of(id: Option<&str>) -> Option<Service> {
    match id {
        None | Some("") => Some(Service::Cb),
        Some(id) => Service::CHOICES
            .iter()
            .position(|c| c.key == id)
            .and_then(|i| Service::from_index(i as u16)),
    }
}

/// The lowest input rate of `service`, Hz.
pub fn min_rate_hz(service: Service) -> f64 {
    match service {
        Service::Cb => 100e3,
        Service::HfVoice => 48e3,
        _ => 250e3,
    }
}

/// The voice channel components, each for CB.
pub fn register(r: &mut Registry) {
    r.add("voice_channels.finder", || {
        Box::new(CarrierFinder::new(Service::Cb))
    });
    r.add("voice_channels.bank", || {
        Box::new(CarrierBank::new(Service::Cb))
    });
    r.add("voice_channels.player", || {
        Box::new(ChannelPlayer::new(Service::Cb))
    });
    r.add("voice_channels.log", || Box::new(CallLog::new(Service::Cb)));
}

/// The graph of `service`.
pub fn builder(service: Service) -> GraphBuilder {
    receiver_builder(plans::profile(service), LogStyle::of(service))
}

/// The graph of the multi-carrier receiver for carriers of `profile`, with a call log of `style`.
pub fn receiver_builder(profile: Profile, style: LogStyle) -> GraphBuilder {
    GraphBuilder::new()
        .add("finder", CarrierFinder::with_profile(profile))
        .add("bank", CarrierBank::with_profile(profile))
        .add("player", ChannelPlayer::with_profile(profile))
        .add("log", CallLog::with_profile(profile, style))
        .input("finder.in")
        .input("bank.in")
        .connect("finder.carriers", "bank.carriers")
        .connect("bank.blocks", "player.blocks")
        .connect("finder.carriers", "player.carriers")
        .connect("bank.voice", "log.voice")
        .connect("finder.carriers", "log.carriers")
        .connect("player.state", "log.play")
        .feedback("player.state", "finder.playing")
        .output("player.out")
        .output("log.events")
        .output("log.requests")
        .status_from("log")
        .clock(0.1)
        .reconfigure_on_retune()
}

/// The graph of `service`.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(service: Service, catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder(service).catalog(catalog).build()
}

/// The stack graph of the submode of `cfg`: the settings routed to the components that own them, and the commands of the
/// call log.
///
/// # Errors
///
/// [`GraphError`] if the submode is not one of the stack or a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let service = service_of(cfg.submode.as_deref()).ok_or_else(|| GraphError::Config {
        component: ID.into(),
        error: ConfigError::Setting(SettingError::Unknown {
            key: format!("submode {}", cfg.submode.clone().unwrap_or_default()),
        }),
    })?;
    let low = min_rate_hz(service);
    let all = ["finder", "bank", "player", "log"];
    let to = |key: &str, of: &[&str]| -> Vec<String> {
        of.iter().map(|c| format!("{c}.{key}")).collect()
    };
    let mut b = StackGraph::of(crate::typed::shape(ID), graph(service, &Catalog::of(cfg)?)?)
        .min_rate(
            low,
            &format!(
                "Voice channels: the sample rate is too low ({:.0} kHz at least)",
                low / 1e3
            ),
        );
    let routes: [(&str, &[&str]); 15] = [
        (PLAN.key(), &all),
        (RASTER_STEP_HZ.key(), &all),
        (RASTER_ORIGIN_HZ.key(), &all),
        (CHANNEL_WIDTH_HZ.key(), &["bank"]),
        (CHANNEL.key(), &["player"]),
        (DEMOD.key(), &["bank"]),
        (SQUELCH.key(), &["finder", "bank"]),
        (DEEMPHASIS.key(), &["bank"]),
        (SCAN.key(), &["player", "log"]),
        (SCAN_CHANNELS.key(), &["player", "log"]),
        (SCAN_DWELL_S.key(), &["player"]),
        (SCAN_HANG_S.key(), &["player"]),
        (CARRIERS_SETTING.key(), &["finder"]),
        (SHOW_HEARD.key(), &["log"]),
        (TIME_BASE.key(), &["log"]),
    ];
    for (key, of) in routes {
        let paths = to(key, of);
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        b = b.route(key, &paths)?;
    }
    b.command("clear", sdrkit_graph::CommandTarget::of("log", "clear"))
        .with_settings(&cfg.settings)
        .build()
}
