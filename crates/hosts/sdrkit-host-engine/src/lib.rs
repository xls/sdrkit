//! The engine of a host: it opens radios, runs sessions on them, runs the services, and exposes every owner (the engine with
//! its catalogs, each radio, session, stack and component, each service, each kind in a catalog) to the host through one
//! handle, [`OwnerRef`].

#![warn(missing_docs)]

mod backends;
mod bank;
mod byname;
mod capture;
mod catalog;
mod devices;
mod drive;
mod engine_owner;
mod error;
mod feed;
mod graph_net;
mod hop;
mod host;
mod mix;
mod owners;
mod published;
mod radio;
mod recording;
mod request;
mod saved;
mod services;
mod session;
mod settings_blob;
mod share;
mod stages;
mod streams;
mod transmit;
mod typed;

pub use catalog::{
    COMPONENT_KINDS, DRIVERS, RADIOS, SERVICES, SESSIONS, STACKS, STACK_SIGNALS, SUBMODES,
};
pub use engine_owner::{
    EngineCommand, EngineSetting, BACKEND_FFT, BACKEND_FIR, BACKEND_LDPC, BACKEND_RESAMPLE,
    BACKEND_RS, BACKEND_VITERBI, BENCHMARKS, BENCH_COMPONENTS, BENCH_CONNECTIONS, BENCH_STAGES,
    BENCH_STATUS, CALLSIGN, CORES_MAX, CORES_USABLE, DEVICE_UNITS, HEIGHT_M, LOCATOR, POSITION,
    REQUESTS_ALLOWED, TIME_ZONE, TRANSMIT_ALLOWED,
};
pub use error::{EngineError, ImportError, RestoreItem};
pub use host::{
    memory_facilities, system_clock, system_facilities, system_random, DirStorage, StdNet,
};
pub use devices::{
    DeviceArrivedEvent, DeviceLeftEvent, DeviceRow, DeviceState, DriverAvailability, DEVICES,
    DEVICE_ARRIVED, DEVICE_LEFT,
};
pub use owners::{FeedId, OwnerRef, RadioId, SessionId, ShareId};
pub use share::{
    ClientJoinedEvent, ClientLeftEvent, ClientRow, GrantArgs, KickArgs, LeftReason, ListenArgs,
    ListenFailedEvent, ListenReply, ListenerRow, RefusedEvent, RefusedReason, ShareCloseArgs,
    ShareCommand, ShareControl, ShareFields, ShareFormat, ShareOpenArgs, ShareOpenReply,
    RequestNote, ShareProtocol, ShareRow, ShareSetting, ShareView, UnlistenArgs, CLIENTS, CLIENT_JOINED, CLIENT_LEFT,
    DEFAULT_ALLOW, DEFAULT_HOST, GRANT, KICK, LISTEN, LISTENERS, LISTEN_FAILED, REFUSED, SHARES,
    SHARE_CLOSE, SHARE_OPEN, UNLISTEN,
};
pub use request::{RequestKind, REQUEST, REQUESTS};
pub use session::{
    Input, SessionSetting, SessionState, AUDIO_IN_LOST, AUDIO_IN_RATE, AUDIO_IN_WANTED, AUDIO_LOST,
    DROPPED, EVENTS_LOST, EVENTS_NOT_DELIVERED, NOT_DELIVERED, PROBLEM, REQUEST_PROBLEM, SAMPLES,
    SPECTRUM, SPECTRUM_FPS, SPECTRUM_SIZE, SPECTRUM_WINDOW, STATE, TRANSMIT_LOST, TUNED, UNLISTED,
};
pub use session::{
    COMPONENTS as SESSION_COMPONENTS, CONNECTIONS as SESSION_CONNECTIONS, CORES_DECLARED,
    CORES_FREE, CORES_NEEDED, CORE_UNITS, GAPS, LOAD_PROBLEM, SHARE, STAGES as SESSION_STAGES,
    STAGE_BEHIND, THREADS,
};
pub use stages::{device, Device, StackNeed};
pub use streams::{AudioInput, AudioRead, IqRead, MediaRead};
pub use transmit::{
    BURST_ATTEMPTS, BURST_ATTENUATION, BURST_REFUSED, BURST_SAMPLES, BURST_SENT, BURST_TX_POWER,
    TRANSMIT, TX_BLOCKED, TX_FAILED, TX_LAST, TX_SENT,
};
pub use typed::object_families;

use catalog::{ComponentKind, StackKind};
use engine_owner::EngineOwner;
use feed::{lock, Feed, Sub};
use owners::Slots;
use radio::Radio;
use request::{Ask, Follower, Requester};
use sdrkit_core::{
    AdapterCatalog, Complex32, ConfigError, Event, EventCatalog, EventDecl, Facilities, GenConfig,
    HostRequest, Owner, OwnerKey, RasterId, Receiver, SeriesId, SettingDecl, SettingError,
    SettingId, StackConfig, StackDescriptor, Transport, TuneTarget, Values,
};
use sdrkit_driver_core::synth::{values_where, GeneratorFactory, Made, SignalInputs};
use sdrkit_driver_core::{DriverKind, Opens};
use sdrkit_graph::components::MakeGenerator;
use sdrkit_graph::{
    Description, Format, Graph, GraphError, GraphPlan, Kind, Registry, RunContext, SampleType,
    ValidateReport,
};
use services::{First, ServiceKind, Services, Tick};
use session::{By, Session};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use streams::Streams;

/// Splits a radio address into the driver key and the driver's own address: `rtl_tcp:10.0.0.5:1234` returns
/// (`rtl_tcp`, `10.0.0.5:1234`). The URL form `rtl_tcp://10.0.0.5:1234` returns the same. A text with no colon returns
/// (`address`, `""`). The scheme `usb:` names a family of USB radios and a selector of one device:
/// `usb:rtl:<selector>` returns the key of the driver of the dongles, `usb:hackrf:<selector>` that of the HackRF.
///
/// ```
/// use sdrkit_host_engine::split_address;
/// assert_eq!(split_address("iiod:192.0.2.1"), ("iiod", "192.0.2.1"));
/// assert_eq!(split_address("iiod://192.0.2.1:30431"), ("iiod", "192.0.2.1:30431"));
/// assert_eq!(split_address("synth"), ("synth", ""));
/// assert_eq!(split_address("usb:rtl:00000001"), ("rtlusb", "00000001"));
/// assert_eq!(split_address("usb:hackrf:"), ("hackrf", ""));
/// assert_eq!(split_address("usb:rtl:@1-2.3"), ("rtlusb", "@1-2.3"));
/// ```
pub fn split_address(address: &str) -> (&str, &str) {
    let address = address.trim();
    let (driver, rest) = address.split_once(':').unwrap_or((address, ""));
    let rest = rest.strip_prefix("//").unwrap_or(rest);
    if driver == "usb" {
        if let Some(family) = sdrkit_driver_registry::split_usb(rest) {
            return family;
        }
    }
    (driver, rest)
}

/// What a host passes to the engine when it makes it.
pub struct Setup {
    /// The storage, clock, random source and codec plug-ins. Every driver, receiver, graph and service gets them.
    pub facilities: Facilities,
    /// The network. Every network driver and every service that sends gets it.
    pub transport: Arc<dyn Transport>,
    /// The stacks the host offers.
    pub stacks: Vec<&'static StackDescriptor>,
    /// The interface descriptions of the families of the stacks. The stacks table and the table of transmit commands
    /// read what a stack transmits from them; a stack without a description shows none.
    pub families: Vec<&'static sdrkit_core::interface::Family>,
    /// The component kinds and adapters that graphs are built from.
    pub components: Registry,
}

impl std::fmt::Debug for Setup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Setup")
            .field("stacks", &self.stacks.len())
            .finish_non_exhaustive()
    }
}

/// The owners of the engine and the order of their locks.
///
/// Lock order, from first to last: the engine owner; `hop_moves`; a table of handles (radios, sessions, followers or
/// hops); one owner (a kind, a radio, the control state of a session, or a service); the run state of that same owner
/// (the receiver of a session, the feed of a radio). A lock is only taken after the locks before it in this order. The control thread also
/// holds the control state of one session while it steps the scanner on that session. A session thread holds its run state
/// and only tries its control state, so it never waits for the host.
pub(crate) struct Shared {
    facilities: Facilities,
    /// The network of the host, for the listeners of a share.
    net: Arc<dyn Transport>,
    /// The USB bus of the host.
    usb: Arc<dyn sdrkit_driver_usb::UsbBus>,
    /// The attached USB devices, as of the last tick.
    usb_state: Mutex<devices::UsbState>,
    shares: Mutex<Slots<share::Share>>,
    /// The transport of the components of a graph: a connection returns at once.
    graph_net: Arc<dyn Transport>,
    /// The test signals of the stacks, for the test-signal driver and for `gen.stack` in a graph.
    generators: MakeGenerator,
    /// The station that the test signals are made for: the station of the engine owner.
    signal_station: Arc<Mutex<sdrkit_core::Station>>,
    /// The stack settings of the sessions that the test signals serve. An entry exists only while the radio of its
    /// session is open: [`Shared::give_signal`] writes and [`Engine::close_radio`] removes under the lock of `radios`.
    signal_settings: Served,
    /// The test signals of the stacks, as the test-signal driver makes them, to check a signal before the driver takes
    /// its values.
    signals: Arc<StackSignals>,
    components: Arc<Registry>,
    adapters: Arc<dyn AdapterCatalog>,
    events: Arc<EventCatalog>,
    /// The keys of the kinds, in the order of their tables, read without a lock.
    keys: Keys,
    stacks: Vec<Mutex<StackKind>>,
    drivers: Vec<Mutex<DriverKind>>,
    component_kinds: Vec<Mutex<ComponentKind>>,
    engine: published::Kept<EngineOwner>,
    /// The events of the engine that wait for the host.
    engine_events: Arc<Mutex<std::collections::VecDeque<Event>>>,
    radios: Mutex<Slots<Radio>>,
    feeds: Mutex<Slots<HostFeed>>,
    sessions: Mutex<Slots<Session>>,
    services: Services,
    followers: Mutex<Vec<Follower>>,
    /// The hops of the sessions: at most one on a radio.
    hops: Mutex<Vec<hop::Hop>>,
    /// Held while a hop moves its radio or ends, so that a hop that ends does not move its radio again.
    hop_moves: Mutex<()>,
    /// The session that the scanner tunes, and what it ran when the scan started.
    scan_home: Mutex<Option<ScanHome>>,
    /// The count of the last spectrum frame that the scan got, and its bins.
    scan_spectrum: Mutex<(u64, Vec<f32>)>,
    sweep_demand: Mutex<Option<drive::ServiceDemand>>,
    /// The spectrum demand of a survey of the scan service on the session that it tunes.
    scan_demand: Mutex<Option<drive::ServiceDemand>>,
    bank: Mutex<bank::BankRun>,
    capture: Mutex<capture::CaptureState>,
    /// The sessions that the engine stopped and closes once their threads have ended.
    closing: Mutex<Vec<SessionId>>,
    /// The rewind buffer of the record service, while no session reads a radio.
    rewind_wait: Mutex<Vec<sdrkit_svc_record::Feed>>,
    /// The engine setting [`TRANSMIT_ALLOWED`], read by the session threads.
    transmit_allowed: Arc<AtomicBool>,
    /// The cores that the sessions may use, and the pool for loops inside a component.
    cores: Arc<stages::Cores>,
    stop: Mutex<bool>,
    /// The ticks that the control thread has completed, and the signal of each.
    ticks: Mutex<u64>,
    ticked: Condvar,
    /// Wakes the control thread before its tick is due. The threads of the radios hold it too.
    wake: Arc<Condvar>,
}

/// A feed that the host writes.
struct HostFeed {
    feed: Arc<Feed>,
    rate_hz: f64,
    centre_hz: f64,
}

/// The kinds of the catalogs, in the order of their tables.
struct Keys {
    stacks: Vec<&'static StackDescriptor>,
    drivers: Vec<&'static str>,
    components: Vec<String>,
}

/// The engine. Dropping it stops every session and closes every radio.
pub struct Engine {
    shared: Arc<Shared>,
    control: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish_non_exhaustive()
    }
}

/// How often the control thread reads the radios and runs the services.
const TICK: Duration = Duration::from_millis(100);
/// The key of the test-signal driver.
const SYNTH: &str = "synth";
/// The audio samples a session keeps for the host: about 1.4 s of stereo at 48 kHz.
const AUDIO_CAP: usize = 1 << 17;
/// The transmit samples a session of a graph keeps for the host: about 0.25 s at 1 MHz.
const TRANSMIT_CAP: usize = 1 << 18;
/// The seconds of transmit samples that a session on a radio that can transmit holds in its transmit stream. A burst
/// up to this length whose end mark is queued is copied whole before it starts and can be sent again: 2.5 s holds the
/// longest Meshtastic LongFast frame (255 bytes, SF 11 at 250 kHz, about 2.2 s). A longer burst is streamed: it goes
/// to the radio as the stack writes it, and is sent once.
const TX_STREAM_S: f64 = 2.5;

/// The transmit samples that a session on a radio that can transmit keeps: [`TX_STREAM_S`] at `rate_hz`, at least
/// [`TRANSMIT_CAP`].
fn transmit_room(rate_hz: f64) -> usize {
    let n = (rate_hz.max(0.0) * TX_STREAM_S).ceil();
    if n.is_finite() && n < usize::MAX as f64 {
        (n as usize).max(TRANSMIT_CAP)
    } else {
        TRANSMIT_CAP
    }
}

/// Samples that the host pushes into a session.
#[derive(Clone, Copy, Debug)]
pub enum Samples<'a> {
    /// Complex 32-bit floats.
    Iq(&'a [Complex32]),
    /// Interleaved little-endian values of a sample type, I then Q, full scale at the type's full scale.
    Raw {
        /// The type of each value.
        sample: SampleType,
        /// The values.
        bytes: &'a [u8],
    },
}

/// A spectrum frame, copied by [`Engine::spectrum`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumFrame {
    /// The bins copied.
    pub bins: usize,
    /// The width of the frame: the sample rate it was made from.
    pub span_hz: f64,
    /// The frequency at its middle bin.
    pub centre_hz: f64,
    /// The frames made per second.
    pub frame_rate: f64,
    /// Seconds of spectrum before this frame: the number of the frames that the session kept before it, divided by
    /// the frame rate.
    pub time_s: f64,
}

/// A tap that the host opened on a session, by the number that [`Engine::open_tap`] returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TapId(pub u32);

/// A part of a tapped stream, copied by [`Engine::read_tap`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapRead {
    /// The kind of the stream: [`Kind::Iq`] (the values are I then Q), [`Kind::Real`] or [`Kind::Audio`].
    pub kind: Kind,
    /// The values copied.
    pub values: usize,
    /// The items per second of the stream.
    pub rate_hz: f64,
    /// The frequency at 0 Hz of the stream.
    pub centre_hz: f64,
    /// Items of the stream that the tap dropped because the host was behind.
    pub dropped: u64,
}

/// A raster, copied by [`Engine::raster`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterRead {
    /// The change counter of the raster. The host requests changes after it.
    pub counter: u64,
    /// Cells in a row.
    pub width: usize,
    /// Rows.
    pub height: usize,
    /// The axis along a row, in the unit that the raster declares for x.
    pub x: AxisRead,
    /// The axis from row to row, in the unit that the raster declares for y.
    pub y: AxisRead,
}

/// An axis of a raster: the value at the centre of the first cell and the change from one cell to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisRead {
    /// The value at the centre of the first cell.
    pub start: f64,
    /// The change from one cell to the next. It can be negative.
    pub step: f64,
}

/// A series, copied by [`Engine::series`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesRead {
    /// The change counter of the series. The host requests changes after it.
    pub counter: u64,
    /// The points make a line. Else they are separate points.
    pub line: bool,
}

/// The owner key of session `id`: `session.s<number>`, the number being [`SessionId::to_u64`].
pub(crate) fn session_key(id: SessionId) -> String {
    format!("session.s{}", id.to_u64())
}

/// The owner key of radio `id`: `radio.r<number>`, the number being [`RadioId::to_u64`].
pub(crate) fn radio_key(id: RadioId) -> String {
    format!("radio.r{}", id.to_u64())
}

/// The session of the key `session.s<number>`, or `None` if `key` is not of that form.
pub(crate) fn parse_session_key(key: &str) -> Option<SessionId> {
    key.strip_prefix("session.s")?
        .parse()
        .ok()
        .map(SessionId::from_u64)
}

/// The radio of the key `radio.r<number>`, or `None` if `key` is not of that form.
pub(crate) fn parse_radio_key(key: &str) -> Option<RadioId> {
    key.strip_prefix("radio.r")?
        .parse()
        .ok()
        .map(RadioId::from_u64)
}

/// The test signals of the stacks, for the test-signal driver. A stack id is `<stack>` or `<stack>/<submode>`.
struct StackSignals {
    stacks: Vec<&'static StackDescriptor>,
    /// The station of the engine, which a generator is made for.
    station: Arc<Mutex<sdrkit_core::Station>>,
    /// The stack settings of the session that each test signal serves, by the stack id of the address of its radio.
    served: Served,
    /// The signals that the checks made, by stack id ([`StackSignals::check`]).
    slots: Mutex<std::collections::HashMap<String, Arc<Mutex<SignalSlot>>>>,
}

/// The test signal of one stack id that a check made, until the driver takes it, and the inputs of the last signal made
/// for the id. A check and a make of the id hold the lock while they build, so the same signal is not built twice.
#[derive(Default)]
struct SignalSlot {
    /// The signal that a check made and its number. The driver takes it if it asks for the same signal.
    held: Option<(u64, Made)>,
    /// The number of the last signal that a check made.
    number: u64,
    /// The inputs of the last signal that the driver took.
    made: Option<SignalInputs>,
}

/// A test signal that a check made and holds for the driver. Dropped before [`Held::keep`], it drops the signal: the
/// open, the start or the change that the check served has failed.
#[must_use]
struct Held(Option<(Arc<Mutex<SignalSlot>>, u64)>);

impl Held {
    /// A check that made no signal.
    fn none() -> Held {
        Held(None)
    }

    /// Leaves the signal for the driver.
    fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let Some((slot, n)) = self.0.take() else {
            return;
        };
        let gone = {
            let mut s = lock(&slot);
            match &s.held {
                Some((k, _)) if *k == n => s.held.take(),
                _ => None,
            }
        };
        drop(gone);
    }
}

/// The stack settings of the session that a test signal serves, and the radio that the session reads.
#[derive(Clone, PartialEq)]
struct ServedBy {
    radio: RadioId,
    values: Values,
}

/// The stack settings of the sessions that the test signals serve, by the stack id of the address of the radio.
type Served = Arc<Mutex<std::collections::HashMap<String, ServedBy>>>;

impl StackSignals {
    /// The stack of `id` if it has a test signal, and the submode that `id` names.
    fn find<'a>(&self, id: &'a str) -> Option<(&'static StackDescriptor, Option<&'a str>)> {
        let (stack, submode) = match id.split_once('/') {
            Some((s, m)) => (s, Some(m)),
            None => (id, None),
        };
        let d = *self.stacks.iter().find(|d| d.id == stack)?;
        d.make_generator.is_some().then_some((d, submode))
    }
}

/// The frequency of the station of a test signal. With a value, the signal is on the air at that frequency only: the
/// radio has it where its samples hold the frequency, and silence with the noise of the signal elsewhere. 0 puts the
/// station where a session of the stack listens, at every centre.
pub const GEN_AT_HZ: sdrkit_core::Setting<sdrkit_core::publish::field::Hz> =
    sdrkit_core::Setting::new("gen_at_hz", "Signal frequency", "Hz", 0.0)
        .help("The frequency of the station of the signal: the radio has the signal only while its samples hold this frequency. 0 puts the station where the session listens, at every centre.")
        .range(0.0, 1e12, 0.0)
        .zero_means("Where the session listens, at every centre");

/// A test signal that is off the air: the samples of `0` are made and replaced by zeros, so the length and the
/// declared power of the signal stay.
struct OffAir(Box<dyn sdrkit_core::Generator>);

impl sdrkit_core::Generator for OffAir {
    fn rate_hz(&self) -> f64 {
        self.0.rate_hz()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = self.0.generate(out);
        let end = n.min(out.len());
        out[..end].fill(Complex32::default());
        n
    }
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        self.0.signal_info()
    }
    fn station_offset_hz(&self, tune_offset_hz: f64) -> f64 {
        self.0.station_offset_hz(tune_offset_hz)
    }
}

/// What the test signal of one stack id is made from.
struct Recipe {
    make: sdrkit_core::MakeGenerator,
    submode: Option<String>,
    schema: Arc<sdrkit_core::Schema>,
    /// The settings of the stack and of the test signal.
    settings: Values,
    /// The tune offset of the stack, Hz.
    own_hz: f64,
    /// The radio centre, Hz.
    centre_hz: f64,
    /// The frequency of the station ([`GEN_AT_HZ`]), if it has one.
    at_hz: Option<f64>,
    station: sdrkit_core::Station,
}

impl Recipe {
    /// The frequency where a session of the stack listens: the radio centre less the tune offset of the stack.
    fn tuned_hz(&self) -> f64 {
        self.centre_hz - self.own_hz
    }

    /// The inputs of a signal at `rate` Hz made from the recipe: the settings, the radio centre if the station has a
    /// frequency of its own, the station, and `tuned_hz`.
    fn inputs(&self, rate: f64, tuned_hz: Option<f64>) -> SignalInputs {
        let mut all = values_where(&self.settings, |_| true);
        if self.at_hz.is_some() {
            all.push((
                sdrkit_driver_core::CENTRE.field.key.to_owned(),
                sdrkit_core::Value::Hz(self.centre_hz),
            ));
        }
        SignalInputs {
            rate_hz: rate,
            values: all,
            station: Some(self.station.clone()),
            tuned_hz,
        }
    }
}

impl StackSignals {
    /// The recipe of the test signal of `id` from the driver's `values`: the stack's settings and the
    /// `gen_*` settings of `values`, over which go the stack settings of the session that the signal serves, and the
    /// `gen_*` settings of that session that differ from their defaults. `served` holds the stack settings of that
    /// session ([`served_values`]). `Ok(None)` if the stack of `id` has no test signal.
    fn recipe(
        &self,
        id: &str,
        values: &Values,
        served: Option<&Values>,
    ) -> Result<Option<Recipe>, SignalError> {
        let Some((d, submode)) = self.find(id) else {
            return Ok(None);
        };
        let Some(make) = d.make_generator else {
            return Ok(None);
        };
        let schema = sdrkit_components::sim::with_generator_settings(&(d.settings)())
            .map_err(SignalError::Schema)?;
        let refused = |e| SignalError::Setting(Arc::clone(&schema), e);
        let mut settings = Values::defaults(&schema);
        for (i, decl) in values.schema().settings().iter().enumerate() {
            let Ok(to) = schema.id(&decl.field.key) else {
                continue;
            };
            // A setting of the driver with the key of a stack setting is not the stack's.
            let gen = sdrkit_components::sim::is_generator_key(&decl.field.key);
            if !gen && schema.setting(to) != Some(decl) {
                continue;
            }
            if let Some(v) = values.value(sdrkit_core::SettingId(i as u16)) {
                settings.set(to, v).map_err(refused)?;
            }
        }
        if let Some(served) = served {
            copy_session(served, &mut settings);
        }
        let radio = |key: &str| {
            let id = values.schema().id(key).ok()?;
            match values.value(id)? {
                sdrkit_core::Value::Hz(hz) => Some(*hz),
                _ => None,
            }
        };
        Ok(Some(Recipe {
            make,
            submode: submode.map(str::to_owned),
            schema,
            settings,
            own_hz: d.tuning.tune_offset_hz,
            centre_hz: radio(sdrkit_driver_core::CENTRE.field.key).unwrap_or(0.0),
            at_hz: radio(GEN_AT_HZ.field.key).filter(|hz| *hz > 0.0),
            station: lock(&self.station).clone(),
        }))
    }

    /// The stack settings of the session that the test signal of `id` serves now.
    fn served_now(&self, id: &str) -> Option<Values> {
        lock(&self.served).get(id).map(|s| s.values.clone())
    }

    /// The generator of `id` at `rate` Hz from its recipe ([`StackSignals::recipe`]) with the driver's `values` and the
    /// stack settings `served`, through the impairments that the `gen_*` settings describe. The generator is made for
    /// the frequency where a session of the stack listens ([`GenConfig::tuned_hz`]), and its inputs hold that frequency
    /// if it read it. The station of the signal is the stack's `tune_offset_hz` below the centre of the samples, where a
    /// session of the stack listens: a generator that makes its station elsewhere
    /// ([`sdrkit_core::Generator::station_offset_hz`]) is moved there. `Ok(None)` if the stack has no test signal.
    fn build(
        &self,
        id: &str,
        values: &Values,
        served: Option<&Values>,
        rate: f64,
    ) -> Result<Option<Made>, SignalError> {
        let Some(r) = self.recipe(id, values, served)? else {
            return Ok(None);
        };
        let seed = u64::try_from(r.settings.get(&sdrkit_algo_sim::chain::GEN_SEED)).unwrap_or(0);
        let cfg = GenConfig::new(rate, r.settings.clone())
            .with_submode_opt(r.submode.clone())
            .with_seed(seed)
            .with_station(r.station.clone())
            .with_tuned_hz(Some(r.tuned_hz()));
        let mut gen = (r.make)(&cfg).map_err(SignalError::Generator)?;
        let inputs = r.inputs(rate, cfg.reads_tuned().then(|| r.tuned_hz()));
        let own = r.own_hz;
        let mut moved = -own - gen.station_offset_hz(own);
        // A station at a fixed frequency moves in the samples with the centre of the radio, and leaves them.
        if let Some(at) = r.at_hz {
            let off = at - r.tuned_hz();
            moved += off;
            if off.abs() >= rate / 2.0 {
                gen = Box::new(OffAir(gen));
            }
        }
        let mut settings = r.settings;
        if moved != 0.0 {
            let offset = &sdrkit_algo_sim::chain::GEN_OFFSET_HZ;
            let at = settings.get(offset) + moved;
            let refused = |e| SignalError::Setting(Arc::clone(&r.schema), e);
            let slot = r.schema.id(offset.field.key).map_err(refused)?;
            settings
                .set(slot, &sdrkit_core::Value::F64(at))
                .map_err(refused)?;
        }
        let gen = sdrkit_components::sim::impaired(gen, &settings).map_err(|e| {
            SignalError::Impairment {
                schema: Arc::clone(&r.schema),
                error: e,
            }
        })?;
        Ok(Some(Made { gen, inputs }))
    }

    /// The inputs of the test signal of `id` with the driver's `values` and the stack settings `served`
    /// ([`GeneratorFactory::inputs`]).
    fn inputs_with(
        &self,
        id: &str,
        values: &Values,
        served: Option<&Values>,
        rate: f64,
    ) -> SignalInputs {
        self.wanted(id, values, served, rate)
            .unwrap_or_else(|| SignalInputs {
                rate_hz: rate,
                values: values_where(values, |_| true),
                station: None,
                tuned_hz: None,
            })
    }

    /// The inputs that the test signal of `id` asks for with the driver's `values`, the stack settings `served` and the
    /// rate `rate`, as the driver compares them ([`SignalInputs::makes_same`]). `None` if the stack has no test signal or
    /// the recipe is refused.
    fn wanted(
        &self,
        id: &str,
        values: &Values,
        served: Option<&Values>,
        rate: f64,
    ) -> Option<SignalInputs> {
        let r = self.recipe(id, values, served).ok()??;
        Some(r.inputs(rate, Some(r.tuned_hz())))
    }

    /// The slot of the signals of `id`.
    fn slot(&self, id: &str) -> Arc<Mutex<SignalSlot>> {
        Arc::clone(lock(&self.slots).entry(id.to_owned()).or_default())
    }

    /// Makes the test signal of `id` with the driver's `values` at their rate and the stack settings `served`, and holds
    /// it for the driver, which takes it when it asks for the same signal ([`GeneratorFactory::make`]). A signal of the
    /// same inputs as the last one that the driver took, or as the one that a check holds, is not made again. Runs on
    /// the calling thread.
    fn check(
        &self,
        id: &str,
        values: &Values,
        served: Option<&Values>,
    ) -> Result<Held, SignalError> {
        let rate = signal_rate(values);
        let slot = self.slot(id);
        let mut s = lock(&slot);
        if let Some(w) = self.wanted(id, values, served, rate) {
            let held = s.held.as_ref().map(|(_, m)| &m.inputs);
            if held
                .into_iter()
                .chain(s.made.as_ref())
                .any(|m| m.makes_same(&w))
            {
                return Ok(Held::none());
            }
        }
        let Some(made) = self.build(id, values, served, rate)? else {
            return Ok(Held::none());
        };
        s.number += 1;
        let n = s.number;
        let old = s.held.replace((n, made));
        drop(s);
        drop(old);
        Ok(Held(Some((slot, n))))
    }

    /// Checks the test signal of `id` that the driver's values and the stack settings `next` make
    /// ([`StackSignals::check`]), if it differs from the signal that `now` makes.
    fn check_change(
        &self,
        id: &str,
        now: (&Values, Option<&Values>),
        next: (&Values, Option<&Values>),
    ) -> Result<Held, SignalError> {
        let was = self.inputs_with(id, now.0, now.1, signal_rate(now.0));
        let will = self.inputs_with(id, next.0, next.1, signal_rate(next.0));
        if was.makes_same(&will) {
            return Ok(Held::none());
        }
        self.check(id, next.0, next.1)
    }

    /// Drops the signal that a check holds for `id` and forgets the signal that the driver took: a check after this
    /// makes the signal again.
    fn release(&self, id: &str) {
        let slot = lock(&self.slots).get(id).cloned();
        let gone = slot.and_then(|s| {
            let mut s = lock(&s);
            s.made = None;
            s.held.take()
        });
        drop(gone);
    }
}

/// The rate setting of a radio in its `values`, Hz. 0 for Automatic and for values without the setting.
fn values_rate(values: &Values) -> f64 {
    values
        .schema()
        .id(sdrkit_driver_core::RATE.field.key)
        .ok()
        .and_then(|id| values.value(id))
        .map_or(0.0, |v| match v {
            sdrkit_core::Value::Hz(hz) | sdrkit_core::Value::F64(hz) => *hz,
            _ => 0.0,
        })
}

/// `values` of a radio with the rate `rate_hz`, inside the limits of the rate setting.
fn with_rate(values: &Values, rate_hz: f64) -> Result<Values, EngineError> {
    let mut v = values.clone();
    let id = EngineError::id_of(v.schema(), sdrkit_driver_core::RATE.field.key)?;
    let limits = v.schema().setting(id).and_then(|s| s.range);
    let rate = limits.map_or(rate_hz, |r| rate_hz.clamp(r.min, r.max));
    EngineError::set(&mut v, id, &sdrkit_core::Value::Hz(rate))?;
    Ok(v)
}

/// The rate of the test-signal driver in its `values`, Hz, as the driver reads it.
fn signal_rate(values: &Values) -> f64 {
    match values_rate(values) {
        hz if hz > 0.0 => hz,
        _ => sdrkit_driver_core::synth::DEFAULT_RATE_HZ,
    }
}

/// The stack id of the test signal at `address` of the test-signal driver: the address without its scheme.
fn signal_id(address: &str) -> &str {
    address.strip_prefix("synth:").unwrap_or(address)
}

/// The stack settings of stack `d` in the settings `values` of a session, as a test signal of the stack takes them.
fn served_values(d: &StackDescriptor, values: &Values) -> Values {
    let mut served = Values::defaults(&(d.settings)());
    copy_where(values, &mut served, |_, _| true);
    served
}

/// Why the test signal of a stack was not made.
#[derive(Debug)]
enum SignalError {
    /// The generator refused its configuration.
    Generator(ConfigError),
    /// The impairments refused the settings of the signal, of `schema`.
    Impairment {
        /// The settings of the stack and of the signal.
        schema: Arc<sdrkit_core::Schema>,
        /// What was refused.
        error: sdrkit_components::sim::SimError,
    },
    /// A value does not fit a setting of the signal, of the schema that the error holds.
    Setting(Arc<sdrkit_core::Schema>, SettingError),
    /// The settings of the stack and of the signal do not make one schema.
    Schema(sdrkit_core::settings::SchemaError),
}

impl SignalError {
    /// The refusal as an error of the configuration.
    fn config(self) -> ConfigError {
        match self {
            SignalError::Generator(e) => e,
            SignalError::Impairment { error, .. } => error.into(),
            SignalError::Setting(schema, error) => {
                let key = refused_key(&schema, &error);
                ConfigError::Stage(sdrkit_core::StageError::new(
                    GraphError::Setting { key, error }.named(&schema),
                ))
            }
            SignalError::Schema(e) => ConfigError::Stage(sdrkit_core::StageError::new(e)),
        }
    }

    /// The refusal of a new value of setting `id`, for the owner of that setting.
    fn of_setting(self, id: SettingId) -> SettingError {
        SettingError::Config {
            id,
            source: Box::new(self.config()),
        }
    }

    /// The refusal of an open or a start. An impairment that needs what the signal does not state is an error of the
    /// setting `gen_noise`; a value that does not fit is an error of its setting.
    fn engine(self) -> EngineError {
        match self {
            SignalError::Setting(schema, e) => EngineError::setting(&schema, e),
            SignalError::Schema(e) => EngineError::Schema(e),
            SignalError::Impairment {
                schema,
                error: error @ sdrkit_components::sim::SimError::SignalLacks { .. },
            } => match schema.id(sdrkit_algo_sim::chain::GEN_NOISE.field.key) {
                Ok(id) => EngineError::setting(
                    &schema,
                    SettingError::Config {
                        id,
                        source: Box::new(error.into()),
                    },
                ),
                Err(_) => EngineError::Config(error.into()),
            },
            e => EngineError::Config(e.config()),
        }
    }
}

impl GeneratorFactory for StackSignals {
    /// The settings of the stack, and [`GEN_AT_HZ`].
    fn settings(&self, id: &str) -> Option<Vec<SettingDecl>> {
        let (d, _) = self.find(id)?;
        let mut all = (d.settings)().settings().to_vec();
        all.push(sdrkit_core::DeclareSetting::decl(&GEN_AT_HZ));
        Some(all)
    }

    /// The generator of the stack at `rate` Hz for the session that the signal serves: the one that a check holds if it
    /// was made from the same inputs, else a new one ([`StackSignals::build`]). Returns `None` if the signal is
    /// refused: the engine refuses such values before they reach the driver ([`Shared::check_signal`], [`SignalGuard`]).
    fn make(&self, id: &str, values: &Values, rate: f64) -> Option<Made> {
        let served = self.served_now(id);
        let slot = self.slot(id);
        let mut s = lock(&slot);
        let wanted = self.wanted(id, values, served.as_ref(), rate);
        let held = s.held.as_ref().map(|(_, m)| &m.inputs);
        let made = if wanted.is_some_and(|w| held.is_some_and(|m| m.makes_same(&w))) {
            s.held.take().map(|(_, m)| m)
        } else {
            self.build(id, values, served.as_ref(), rate).ok().flatten()
        };
        s.made = made.as_ref().map(|m| m.inputs.clone());
        made
    }

    /// The rate, the settings of the recipe, the radio centre if the station has a frequency of its own, the station, and
    /// the frequency where a session of the stack listens.
    fn inputs(&self, id: &str, values: &Values, rate: f64) -> SignalInputs {
        let served = self.served_now(id);
        self.inputs_with(id, values, served.as_ref(), rate)
    }
}

/// The key in `schema` of the setting that `e` refuses first, or an empty text if `e` names none.
fn refused_key(schema: &sdrkit_core::Schema, e: &SettingError) -> String {
    let first = std::cell::Cell::new(None);
    let _ = e.clone().map_ids(|id| {
        if first.get().is_none() {
            first.set(Some(id));
        }
        id
    });
    first
        .get()
        .and_then(|id| schema.setting(id))
        .map_or_else(String::new, |d| d.field.key.to_string())
}

/// An owner whose settings make a test signal: a new value that changes the signal is taken only if the signal can be
/// made with it. `test` gets the values of the owner now and with the new value.
struct SignalGuard<'a> {
    owner: &'a mut dyn sdrkit_core::Owner,
    test: &'a (dyn Fn(&Values, &Values) -> Result<Held, SignalError> + Sync),
}

impl sdrkit_core::Owner for SignalGuard<'_> {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        self.owner.schema()
    }
    fn values(&self) -> Option<&Values> {
        self.owner.values()
    }
    fn apply(&mut self, id: SettingId, value: &sdrkit_core::Value) -> Result<(), SettingError> {
        let mut held = Held::none();
        if let Some(now) = self.owner.values() {
            let mut next = now.clone();
            if next.set(id, value).is_ok() {
                held = (self.test)(now, &next).map_err(|e| e.of_setting(id))?;
            }
        }
        let r = self.owner.apply(id, value);
        if r.is_ok() {
            held.keep();
        }
        r
    }
    fn results_decl(&self) -> &Arc<sdrkit_core::ResultsDecl> {
        self.owner.results_decl()
    }
    fn results(&self) -> &sdrkit_core::Results {
        self.owner.results()
    }
    fn events(&self) -> &[EventDecl] {
        self.owner.events()
    }
    fn commands(&self) -> &sdrkit_core::Commands {
        self.owner.commands()
    }
    fn invoke(
        &mut self,
        id: sdrkit_core::CommandId,
        args: &Values,
    ) -> Result<Option<sdrkit_core::Record>, sdrkit_core::CommandError> {
        self.owner.invoke(id, args)
    }
    fn problem(&mut self, text: &str) {
        self.owner.problem(text);
    }
}

/// True for a setting of the test signal of a stack (a key that starts with `gen_`). A receiver does not read it: the
/// session keeps its value and gives it to the signal.
pub(crate) fn is_signal_setting(key: &str) -> bool {
    key.starts_with("gen_")
}

/// Copies the stack settings of a session (`from`) into the settings of its test signal (`to`): every setting, and a
/// setting of the test signal ([`is_signal_setting`]) only if the session has a value other than its default, which then
/// takes the place of the radio's.
fn copy_session(from: &Values, to: &mut Values) {
    let defaults = Values::defaults(from.schema());
    copy_where(from, to, |key, v| {
        !is_signal_setting(key)
            || defaults
                .schema()
                .id(key)
                .ok()
                .and_then(|id| defaults.value(id))
                != Some(v)
    });
}

/// Copies into `to` each value of `from` whose setting `to` declares the same and that `keep` accepts.
fn copy_where(from: &Values, to: &mut Values, keep: impl Fn(&str, &sdrkit_core::Value) -> bool) {
    for (i, decl) in from.schema().settings().iter().enumerate() {
        let Ok(at) = to.schema().id(&decl.field.key) else {
            continue;
        };
        if to.schema().setting(at) != Some(decl) {
            continue;
        }
        if let Some(v) = from.value(sdrkit_core::SettingId(i as u16)) {
            if keep(&decl.field.key, v) {
                let _ = to.set(at, v);
            }
        }
    }
}

/// Adds `decls` to `out`, except a kind that `out` holds already.
fn add_events(out: &mut Vec<EventDecl>, decls: &[EventDecl]) {
    for d in decls {
        if !out.iter().any(|o| o.kind == d.kind) {
            out.push(d.clone());
        }
    }
}

impl Engine {
    /// Makes the engine with the facilities, network, stacks and components of `setup`, every driver kind of `sdrkit-driver-registry` and every service, each with
    /// the host's facilities. The test-signal driver plays the generator of the stack that its address names
    /// (`synth:<stack>`, or `synth:<stack>/<submode>`), with the driver's `gen_*` settings as its impairments. The event
    /// catalog holds the event kinds that every radio, stack, service and the engine declare.
    ///
    /// # Errors
    ///
    /// [`EngineError::Events`] if the event kinds do not make one catalog, the error of a service that does not open,
    /// [`EngineError::Thread`] if the control thread does not start.
    ///
    /// This engine reaches no USB device: the kinds of USB radios are listed, and an open says that USB is not
    /// available. A host that has USB radios calls [`Engine::with_usb`].
    pub fn new(setup: Setup) -> Result<Engine, EngineError> {
        Engine::with_usb(setup, Arc::new(sdrkit_driver_usb::NoUsb))
    }

    /// As [`Engine::new`], with the USB radios of `usb`: `sdrkit_driver_usb::platform_bus()` for the bus of the
    /// system, in a build with the feature `nusb` of that crate. The engine lists the devices of the bus in its table
    /// `devices` and raises `device.arrived` and `device.left`; listing opens no device.
    ///
    /// # Errors
    ///
    /// As [`Engine::new`].
    pub fn with_usb(
        setup: Setup,
        usb: Arc<dyn sdrkit_driver_usb::UsbBus>,
    ) -> Result<Engine, EngineError> {
        let f = &setup.facilities;
        let gen_stacks = setup.stacks.clone();
        let signal_station = Arc::new(Mutex::new(sdrkit_core::Station::default()));
        let signal_settings = Served::default();
        let gen_station = Arc::clone(&signal_station);
        let generators: MakeGenerator = Arc::new(move |id: &str, rate: f64| {
            let d = gen_stacks.iter().find(|d| d.id == id)?;
            let make = d.make_generator?;
            let rate = if rate > 0.0 { rate } else { d.tuning.rate_hz };
            let cfg = GenConfig::new(rate, Values::defaults(&(d.settings)()))
                .with_station(lock(&gen_station).clone());
            make(&cfg).ok()
        });
        let signals = Arc::new(StackSignals {
            stacks: setup.stacks.clone(),
            station: Arc::clone(&signal_station),
            served: Arc::clone(&signal_settings),
            slots: Mutex::default(),
        });
        let drivers = sdrkit_driver_registry::all(
            Arc::clone(&signals) as Arc<dyn GeneratorFactory>,
            Arc::clone(&f.storage),
            Arc::clone(&setup.transport),
            f.clock.clone(),
            Arc::clone(&usb),
        );
        let descriptors: Vec<_> = drivers.iter().map(|d| d.descriptor()).collect();
        let adapters: Arc<dyn AdapterCatalog> = setup.components.catalog().clone();
        let mut decls: Vec<EventDecl> = sdrkit_driver_core::RADIO_EVENTS
            .iter()
            .map(|d| d.decl())
            .collect();
        add_events(
            &mut decls,
            &[
                REQUEST.decl(),
                TRANSMIT.decl(),
                EVENTS_NOT_DELIVERED.decl(),
                DEVICE_ARRIVED.decl(),
                DEVICE_LEFT.decl(),
                CLIENT_JOINED.decl(),
                CLIENT_LEFT.decl(),
                REFUSED.decl(),
                LISTEN_FAILED.decl(),
            ],
        );
        // The event kinds of a stack come from the interface description of its family, in the order of the stack
        // list, so the start makes no stack. A stack without a description is made here once to read them.
        let mut stack_kinds = Vec::with_capacity(setup.stacks.len());
        for d in &setup.stacks {
            let mut k = StackKind::new(d);
            match setup.families.iter().find_map(|fam| fam.stack_shape(d.id)) {
                Some(shape) => {
                    let these: Vec<EventDecl> = shape.events.iter().map(|e| e.def.decl()).collect();
                    add_events(&mut decls, &these);
                }
                // A stack that cannot be made with its defaults declares no events. Its kind owner returns the error.
                None => {
                    if k.ensure(&adapters, f).is_ok() {
                        add_events(&mut decls, k.events());
                    }
                }
            }
            stack_kinds.push(Mutex::new(k));
        }
        let first = First::make(f, &setup.stacks)?;
        let mut service_events = Vec::new();
        first.events(&mut service_events);
        add_events(&mut decls, &service_events);
        let events = Arc::new(EventCatalog::from_decls(&decls)?);
        let services = Services::new(first, Arc::clone(&events), f, &setup.transport)?;
        let keys = Keys {
            stacks: setup.stacks.clone(),
            drivers: descriptors.iter().map(|d| d.key).collect(),
            components: setup.components.names().map(str::to_owned).collect(),
        };
        let transmit_allowed = Arc::new(AtomicBool::new(false));
        let cores = Arc::new(stages::Cores::new());
        let engine_events = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let shared = Arc::new_cyclic(|weak| Shared {
            engine: published::Kept::new(EngineOwner::new(
                &catalog::Sources {
                    stacks: &setup.stacks,
                    families: &setup.families,
                    drivers: &descriptors,
                    components: &setup.components,
                    usb: usb.as_ref(),
                },
                weak.clone(),
                Arc::clone(&transmit_allowed),
                Arc::clone(&cores),
                Arc::clone(&engine_events),
            )),
            engine_events,
            keys,
            stacks: stack_kinds,
            drivers: drivers
                .into_iter()
                .map(|d| Mutex::new(DriverKind::new(d)))
                .collect(),
            component_kinds: setup
                .components
                .descriptors()
                .map(|d| Mutex::new(ComponentKind::new(d)))
                .collect(),
            facilities: setup.facilities,
            net: Arc::clone(&setup.transport),
            usb,
            usb_state: Mutex::default(),
            shares: Mutex::default(),
            graph_net: Arc::new(graph_net::GraphNet(setup.transport)),
            generators,
            signal_station,
            signal_settings,
            signals,
            components: Arc::new(setup.components),
            adapters,
            events,
            radios: Mutex::default(),
            feeds: Mutex::default(),
            sessions: Mutex::default(),
            services,
            followers: Mutex::default(),
            hops: Mutex::default(),
            hop_moves: Mutex::default(),
            scan_home: Mutex::default(),
            scan_spectrum: Mutex::default(),
            sweep_demand: Mutex::default(),
            scan_demand: Mutex::default(),
            bank: Mutex::default(),
            capture: Mutex::default(),
            closing: Mutex::default(),
            rewind_wait: Mutex::default(),
            transmit_allowed,
            cores,
            stop: Mutex::new(false),
            ticks: Mutex::new(0),
            ticked: Condvar::new(),
            wake: Arc::new(Condvar::new()),
        });
        // The results of a stack as a session shows them, for the check of a scan that names a stack. The stack is made
        // on the first check that names it.
        let weak = Arc::downgrade(&shared);
        shared
            .services
            .scan
            .lock()
            .set_stack_results(Arc::new(move |id: &str| {
                let sh = weak.upgrade()?;
                let i = sh.keys.stacks.iter().position(|d| d.id == id)?;
                let mut k = lock(sh.stacks.get(i)?);
                k.ensure(&sh.adapters, &sh.facilities).ok()?;
                Some(Arc::new(session::merged_decl(k.results_decl())))
            }));
        let station = shared.station();
        *lock(&shared.signal_station) = station;
        shared.engine.lock().note_device();
        // The calibration of the device runs on a thread of its own, so that no call of the host waits for it. Its
        // result becomes results of the engine.
        let weak = Arc::downgrade(&shared);
        let _ = std::thread::Builder::new()
            .name("sdrkit-calibrate-start".into())
            .spawn(move || {
                stages::device();
                if let Some(sh) = weak.upgrade() {
                    sh.engine.lock().note_device();
                }
            });
        let s = Arc::clone(&shared);
        let control = std::thread::Builder::new()
            .name("sdrkit-control".into())
            .spawn(move || control(&s))
            .map_err(|e| EngineError::thread(&e))?;
        Ok(Engine {
            shared,
            control: Some(control),
        })
    }

    /// The catalog of every event kind in the engine. An event that leaves the engine carries its id here.
    pub fn event_catalog(&self) -> &Arc<EventCatalog> {
        &self.shared.events
    }

    /// The owner with the key `key`: `engine`, `stack.<id>`, `driver.<key>`, `component.<kind>`, `service.<name>`,
    /// `radio.r<number>` or `session.s<number>` (the number of [`RadioId::to_u64`] or [`SessionId::to_u64`]).
    ///
    /// # Errors
    ///
    /// [`EngineError::BadKey`] if `key` is not an owner key, [`EngineError::NoSuchKey`] if no owner has it.
    pub fn find(&self, key: &str) -> Result<OwnerRef, EngineError> {
        self.shared.find(key)
    }

    /// The owner key of `r`, as [`Engine::find`] takes it. `None` for a stack or a component of a session.
    pub fn key_of(&self, r: OwnerRef) -> Option<String> {
        let sh = &self.shared;
        let row = |i: u32| usize::try_from(i).ok();
        match r {
            OwnerRef::Engine => Some("engine".into()),
            OwnerRef::Radio(id) => Some(radio_key(id)),
            OwnerRef::Session(id) => Some(session_key(id)),
            OwnerRef::Stack(_) | OwnerRef::Component(..) => None,
            OwnerRef::StackKind(i) => sh
                .keys
                .stacks
                .get(row(i)?)
                .map(|d| format!("stack.{}", d.id)),
            OwnerRef::DriverKind(i) => sh.keys.drivers.get(row(i)?).map(|k| format!("driver.{k}")),
            OwnerRef::ComponentKind(i) => sh
                .keys
                .components
                .get(row(i)?)
                .map(|k| format!("component.{k}")),
            OwnerRef::Service(i) => ServiceKind::at(i).map(|s| s.key().to_owned()),
            OwnerRef::Share(id) => Some(share::share_key(id)),
        }
    }

    /// Runs `f` on the owner `r`. A read takes the newest published copy of the owner and does not wait for a thread
    /// that works on the owner: the session thread for a session, its stack and the components of its graph, the control
    /// thread or another call for the engine, a radio and a service. A setting or a command of the engine, a radio or a
    /// service runs under the lock of that owner, and the reads after it see its effect. A setting of the session reaches the receiver before its next block. A setting or a command of
    /// the stack or of a component is carried out between two blocks, and the call returns its result. `f` must not reach the same owner through the
    /// engine again: that waits for ever.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] if `r` refers to no owner. For a kind, the error of making one with the defaults.
    pub fn owner<R>(
        &self,
        r: OwnerRef,
        f: impl FnOnce(&mut dyn Owner) -> R,
    ) -> Result<R, EngineError> {
        self.shared.owner(r, f)
    }

    /// Waits until the control thread has completed one whole tick that started after this call, at most `timeout`.
    /// Returns false if the time ran out.
    ///
    /// A setting or a command of the host shows in the next read of the host. What the engine does by itself (the
    /// requests of a service or of a rule, the events of a session passed to the services) shows after the tick that
    /// carries it out: a host that must see it waits with this call.
    pub fn settle(&self, timeout: std::time::Duration) -> bool {
        let sh = &self.shared;
        let end = std::time::Instant::now() + timeout;
        let mut n = lock(&sh.ticks);
        // The tick that runs now started before the call, so the one after it counts.
        let want = *n + 2;
        while *n < want {
            let Some(left) = end.checked_duration_since(std::time::Instant::now()) else {
                return false;
            };
            sh.wake.notify_all();
            let (g, _) = sh
                .ticked
                .wait_timeout(n, left)
                .unwrap_or_else(|p| p.into_inner());
            n = g;
        }
        true
    }

    /// Returns the handle of the component named `name` of the graph of session `id`, for [`OwnerRef::Component`].
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] if the session does not exist or runs no graph, [`EngineError::NoSuchKey`] if its graph
    /// has no component of that name.
    pub fn component(
        &self,
        id: SessionId,
        name: &str,
    ) -> Result<sdrkit_graph::ComponentHandle, EngineError> {
        let gone = || EngineError::NoSuchOwner(OwnerRef::Session(id));
        let s = self.shared.session(id).ok_or_else(gone)?;
        s.component(name)
            .ok_or_else(gone)?
            .ok_or_else(|| EngineError::NoSuchKey {
                key: name.to_owned(),
            })
    }

    /// Moves the events of owner `r` that wait into `out`, each with its kind id. The engine, radios, sessions and services
    /// have events. A session's events come after the services saw them.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] if `r` refers to no owner.
    pub fn take_events(&self, r: OwnerRef, out: &mut Vec<Event>) -> Result<(), EngineError> {
        let sh = &self.shared;
        let gone = || EngineError::NoSuchOwner(r);
        match r {
            OwnerRef::Engine => out.extend(lock(&sh.engine_events).drain(..)),
            OwnerRef::Radio(id) => sh.radio(id).ok_or_else(gone)?.take_events(out),
            OwnerRef::Session(id) => {
                lock(&sh.session(id).ok_or_else(gone)?.control).take_events(out, &sh.events);
            }
            OwnerRef::Service(i) => sh
                .services
                .take_events(ServiceKind::at(i).ok_or_else(gone)?, out),
            OwnerRef::StackKind(_) | OwnerRef::DriverKind(_) | OwnerRef::ComponentKind(_) => {}
            OwnerRef::Stack(id) | OwnerRef::Component(id, _) => {
                sh.session(id).ok_or_else(gone)?;
            }
            OwnerRef::Share(id) => sh.share(id).ok_or_else(gone)?.take_events(out),
        }
        Ok(())
    }

    /// Opens the radio at `address` with the driver kind `driver` and the settings `values`, of that kind's schema or of
    /// [`Engine::radio_schema`], and starts its stream. A rate of 0 in `values` is Automatic: the engine sets the rate
    /// that the stack of each session on the radio needs ([`Engine::start_stack`]), and a test signal opens at the
    /// rate of its stack. A rate above 0 stays until the host sets another.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchDriver`]. [`EngineError::NoSuchStack`] for the test signal (`synth:<stack>`) of a stack that
    /// the engine does not have. [`EngineError::RadioOpen`] if a radio of this address is open and the driver opens an
    /// address once. For a test signal that refuses its settings, [`EngineError::Setting`] that names the setting, or
    /// [`EngineError::Config`] if the generator refuses its configuration. Else the error of the driver.
    pub fn open_radio(
        &self,
        driver: &str,
        address: &str,
        values: &Values,
    ) -> Result<RadioId, EngineError> {
        self.shared.open_radio(None, driver, address, values)
    }

    /// The schema of the settings that [`Engine::open_radio`] takes for `address` with the driver kind `driver`: those of
    /// the kind, then those that the radio adds and that are known before it opens. For the test signal of a stack these
    /// are the settings of the stack and of its signal, so the signal starts as they ask.
    ///
    /// # Errors
    ///
    /// As [`Engine::open_radio`] for the driver and the address.
    pub fn radio_schema(
        &self,
        driver: &str,
        address: &str,
    ) -> Result<Arc<sdrkit_core::Schema>, EngineError> {
        let kind = self.shared.driver_kind(driver, address)?;
        Ok(lock(kind).schema_at(address)?)
    }

    /// Stops the stream of radio `id` and closes it. From the moment it is called, the radio takes no new session.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], or [`EngineError::RadioInUse`] while sessions read the radio.
    pub fn close_radio(&self, id: RadioId) -> Result<(), EngineError> {
        let sh = &self.shared;
        // The share of the radio goes first: its session reads the radio.
        sh.close_share_of(id);
        let (radio, last) = {
            let mut t = lock(&sh.radios);
            let r = t
                .get(id.handle())
                .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(id)))?;
            let n = r.feed.close();
            if n > 0 {
                return Err(EngineError::RadioInUse { sessions: n });
            }
            let r = t
                .remove(id.handle())
                .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(id)))?;
            // The settings that a session of this radio gave a test signal go with the radio, under the lock that
            // `Shared::give_signal` holds, so a tick in progress cannot give them again.
            lock(&sh.signal_settings).retain(|_, s| s.radio != id);
            let last = r.driver == SYNTH && !t.iter().any(|(_, o)| o.address == r.address);
            (r, last)
        };
        radio.owner.lock().stop();
        // The signals that the checks made for the address go with its last radio.
        if last {
            sh.signals.release(signal_id(&radio.address));
        }
        sh.engine.lock().results.remove_row(&RADIOS, &radio_key(id));
        sh.usb_closed(id);
        Ok(())
    }

    /// Opens the share of radio `radio`: an owner ([`OwnerRef::Share`], key `share.h<n>`) that serves the samples of the
    /// radio to network clients. Its command `listen` starts a listener for rtl_tcp or SoapyRemote; its settings say who
    /// steers the radio and which addresses may connect. The share runs a session of its own on the radio, which never
    /// sets the rate or the centre of the radio and never stops a session of the host from doing so. A radio has one
    /// share. The engine command `share.open` does the same.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] for a radio that is not open, [`EngineError::ShareOpen`] if the radio has a share.
    pub fn open_share(&self, radio: RadioId) -> Result<ShareId, EngineError> {
        self.shared.open_share(None, radio)
    }

    /// Closes share `id`: its listeners stop and its clients are disconnected. [`Engine::close_radio`] closes the share
    /// of its radio first.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub fn close_share(&self, id: ShareId) -> Result<(), EngineError> {
        self.shared.close_share(None, id)
    }

    /// What share `id` does now, for a screen of the host: the counters of its clients, the level of its radio and
    /// the last requests of its clients.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub fn share_view(&self, id: ShareId) -> Result<share::ShareView, EngineError> {
        self.shared.share_view(id)
    }

    /// Starts a session of stack `stack` (in its variant `submode`, if given) with the settings `values` of the stack's
    /// schema, on `input`.
    ///
    /// On a radio that tunes, the session is tuned to `centre_hz`, or to the default frequency of the stack or variant if
    /// `centre_hz` is `None`. If no other session reads the radio, the radio is first set to that frequency plus the
    /// stack's `tune_offset_hz`, and a radio whose rate is on Automatic to the least of its rates that the stack's rate
    /// fits in. A rate that the host set on the radio stays. A radio that other sessions read moves only if the frequency is outside
    /// the middle 80 % of its capture. On a radio with one fixed frequency (a recording), the session is tuned only to a
    /// `centre_hz` that is given. On a host feed or an inside input, `centre_hz` is not read.
    ///
    /// A session with no frequency (none given and no default, or 0) leaves the centre of its input as it is and listens
    /// the stack's `tune_offset_hz` below it, on a radio, a recording and a host feed. An inside input is a channel
    /// around its station, and the session listens at its centre.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchStack`], [`EngineError::NoSuchOwner`] for a closed or closing radio, [`EngineError::Setting`]
    /// if the radio refuses the rate or the centre or the test signal of the radio refuses a setting of the session,
    /// [`EngineError::Config`] if the stack's receiver or its test signal cannot be made, and with
    /// [`sdrkit_core::ConfigError::RateTooLow`] if the rate that the host set, or the highest rate of the radio, is
    /// below the least rate of the stack, [`EngineError::Thread`].
    pub fn start_stack(
        &self,
        stack: &str,
        submode: Option<&str>,
        values: &Values,
        input: Input,
        centre_hz: Option<f64>,
    ) -> Result<SessionId, EngineError> {
        let sh = &self.shared;
        let desc = sh.stack(stack)?;
        // The operator of the stack says where the radio goes. A frequency from the host is its frequency.
        use sdrkit_core::Operator as _;
        let mut operator = sdrkit_core::TableOperator::of_stack(desc, submode);
        operator.set_rate(desc.rate_hz(values));
        if let Some(hz) = centre_hz {
            operator.set_frequency(hz);
        }
        let hz = match input {
            Input::Radio(r) if sh.radio_tunes(r)? => {
                sh.prepare_radio(r, &operator.demand(), &desc.tuning, desc.name)?;
                Some(operator.frequency())
            }
            Input::Radio(_) => centre_hz,
            Input::Feed(_) | Input::Inside => None,
        };
        // The signal that the check makes waits for the driver until the start succeeds.
        let held = sh.check_signal(input, desc, values)?;
        let rx = sh.make_receiver(desc, submode, values, &sh.station())?;
        let id = sh.start(None, By::Host, rx, input, Some(desc), submode)?;
        if let Some(s) = sh.session(id) {
            let mut c = lock(&s.control);
            c.operator.set_rate(desc.rate_hz(values));
            c.keep_signal_settings(values);
        }
        // The session listens where the operator does. On an input that does not tune it follows the centre of the input.
        match hz {
            Some(hz) => {
                if let Err(e) = sh.tune_to(id, hz) {
                    let _ = sh.end_session(id);
                    return Err(e);
                }
            }
            None => {
                if let Some(s) = sh.session(id) {
                    lock(&s.control).operator.set_frequency(0.0);
                }
            }
        }
        sh.tell_scan(id);
        held.keep();
        Ok(id)
    }

    /// Starts a session of the graph that `plan` describes, on `input`. Its components come from the engine's registry. Its
    /// endpoints reach the session's streams (`host.out`, `tx.sink`), the host's network (`net.sink`) and the stacks' test
    /// signals (`gen.stack`).
    ///
    /// # Errors
    ///
    /// [`EngineError::Graph`] if the plan does not build, [`EngineError::NoSuchOwner`] for a closed or closing radio,
    /// [`EngineError::Thread`].
    pub fn start_graph(&self, mut plan: GraphPlan, input: Input) -> Result<SessionId, EngineError> {
        let sh = &self.shared;
        plan.set_catalog(sh.components.catalog());
        let mut graph = plan.build()?;
        // A graph with a transmit sink has a transmit stream: to the radio if the radio can transmit, else to the host.
        let room = match input {
            _ if !graph.transmits() => 1,
            Input::Radio(r) => sh.transmit_room(r).unwrap_or(TRANSMIT_CAP),
            Input::Feed(_) | Input::Inside => TRANSMIT_CAP,
        };
        let streams = Arc::new(Streams::new(AUDIO_CAP, room));
        graph.set_context(sh.context());
        let mut rx: Box<dyn Receiver> = Box::new(graph);
        rx.set_station(&sh.station());
        let feed = sh.feed_for(input)?;
        sh.start_on(None, By::Host, rx, (input, feed), None, None, streams)
    }

    /// The calibration of this device: the units per second of one core. `None` for the first tens of milliseconds
    /// after the first engine of the process started; the call never waits.
    pub fn device(&self) -> Option<Device> {
        stages::device_if_ready()
    }

    /// What `stack` in `submode` with its default settings needs on this device, by its recorded cost, before a session
    /// starts it: the cores, the threads that the engine would give it, and whether it keeps up with its signal. A
    /// stack with no recorded cost needs 0 cores by this call. `None` while the calibration of the device has not
    /// finished; the call never waits for it.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchStack`], and the error of the stack if it cannot be made.
    pub fn stack_need(
        &self,
        stack: &str,
        submode: Option<&str>,
    ) -> Result<Option<StackNeed>, EngineError> {
        let sh = &self.shared;
        let desc = sh.stack(stack)?;
        let cfg = sdrkit_core::StackConfig::new(Values::defaults(&(desc.settings)()))
            .with_submode_opt(submode.map(str::to_owned));
        let mut rx = (desc.make_receiver)(&cfg)?;
        let stages = rx.staged().map(|s| s.stages()).unwrap_or_default();
        Ok(stages::device_if_ready().map(|d| sh.cores.need(&stages, d)))
    }

    /// Opens a host feed of complex samples at `rate_hz` around `centre_hz`. Sessions read it with [`Input::Feed`], and
    /// the host writes it with [`Engine::push`]. Work that several of its sessions need (a spectrum) is done once.
    pub fn open_feed(&self, rate_hz: f64, centre_hz: f64) -> FeedId {
        let sh = &self.shared;
        let feed = HostFeed {
            feed: Arc::new(Feed::new(
                false,
                Arc::clone(sh.components.catalog()),
                sh.context(),
            )),
            rate_hz,
            centre_hz,
        };
        FeedId::new(lock(&sh.feeds).insert(Arc::new(feed)))
    }

    /// Closes host feed `id`. Its sessions run on and read no more samples.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchFeed`].
    pub fn close_feed(&self, id: FeedId) -> Result<(), EngineError> {
        lock(&self.shared.feeds)
            .remove(id.handle())
            .map(|_| ())
            .ok_or(EngineError::NoSuchFeed { feed: id })
    }

    /// Writes `samples` to host feed `id`, for every session that reads it. Raw samples are converted once. The call
    /// never waits.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchFeed`], [`EngineError::PartSample`] if raw bytes end in a part of a sample, or
    /// [`EngineError::Behind`] if a session of the feed has not used the samples it read before. Nothing is written on
    /// an error.
    pub fn push(&self, id: FeedId, samples: Samples<'_>) -> Result<(), EngineError> {
        let f = self.shared.feed(id)?;
        if let Samples::Raw { sample, bytes } = samples {
            if bytes.len() % (2 * sample.value_bytes()) != 0 {
                return Err(EngineError::PartSample { bytes: bytes.len() });
            }
        }
        f.feed
            .deliver(f.rate_hz, f.centre_hz, true, |buf| {
                buf.clear();
                match samples {
                    Samples::Iq(iq) => buf.extend_from_slice(iq),
                    Samples::Raw { sample, bytes } => {
                        sdrkit_graph::components::iq_from_bytes(sample, bytes, buf);
                    }
                }
                buf.len()
            })
            .map(|_| ())
            .map_err(|feed::Undelivered::Behind| EngineError::Behind)
    }

    /// Stops session `id`. Its thread ends after the block it works on. Its results stay until it is closed.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub fn stop_session(&self, id: SessionId) -> Result<(), EngineError> {
        self.shared.stop_session(id)
    }

    /// Closes the stopped session `id` and frees its thread and its handle.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], or [`EngineError::StillRunning`] if it was not stopped.
    pub fn close_session(&self, id: SessionId) -> Result<(), EngineError> {
        self.shared.close_session(id)
    }

    /// Copies the newest spectrum frame of session `id` into `out`, if a frame came since the last call. The session requests
    /// a spectrum with its setting [`SPECTRUM`]. The frame is made once from the input's stream for every session that
    /// requests the same spectrum.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub fn spectrum(
        &self,
        id: SessionId,
        out: &mut Vec<f32>,
    ) -> Result<Option<SpectrumFrame>, EngineError> {
        let s = self.shared.session_or(id)?;
        let mut f = lock(&s.link.demand.frame);
        if f.made == f.taken {
            return Ok(None);
        }
        f.taken = f.made;
        out.clear();
        out.extend_from_slice(&f.values);
        Ok(Some(SpectrumFrame {
            bins: out.len(),
            span_hz: f.span_hz,
            centre_hz: f.centre_hz,
            frame_rate: f.frame_rate,
            time_s: if f.frame_rate > 0.0 {
                (f.made - 1) as f64 / f.frame_rate
            } else {
                0.0
            },
        }))
    }

    /// Copies the audio of session `id` that waits into `out`: whole frames of one rate and one channel count, at most what
    /// `out` holds. The audio is what the receiver writes and what a `host.out` of its graph takes, at the session's volume.
    /// A channel session of the bank sends its audio to the bank's mix: the host reads the bank's session instead. `None`
    /// if no audio waits.
    /// Audio that the host does not take in time is lost, oldest first, and counted in the session's field [`AUDIO_LOST`].
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], or [`EngineError::InMix`] for a channel session of the bank.
    pub fn audio(&self, id: SessionId, out: &mut [f32]) -> Result<Option<AudioRead>, EngineError> {
        let s = self.shared.session_or(id)?;
        if s.link
            .streams
            .mixed
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(EngineError::InMix);
        }
        let mut r = lock(&s.readers);
        let r = &mut *r;
        Ok(s.link.streams.read_audio(&mut r.audio_at, &mut r.runs, out))
    }

    /// Gives session `id` audio of the host at `rate_hz`, such as a microphone, for the host audio input (`host.in`)
    /// of its graph: interleaved frames of `channels` channels, 1 or 2 (left, then right). A part of a frame at the
    /// end is left out. The engine converts the audio to the rate and the channel count of that input, and the graph
    /// takes it with its next samples: mono audio goes to both channels of a stereo input, and stereo audio becomes
    /// the mean of left and right for a mono input. The channel count and the rate can change from call to call; the
    /// audio from before that the graph has not taken is then left out. The call never waits and allocates nothing,
    /// but it looks the session up under a lock of the engine; a thread that must not wait writes through
    /// [`Engine::audio_input`]. The session keeps 32768 samples (0.68 s of mono or 0.34 s of stereo at 48 kHz): audio
    /// that the graph does not take in time is lost, oldest first, and counted in the field [`AUDIO_IN_LOST`]. The
    /// graph takes audio while it has a host audio input; audio for another session is dropped. A session that
    /// transmits asks for audio with its fields [`AUDIO_IN_RATE`] and [`AUDIO_IN_WANTED`]. One thread at a time
    /// writes the audio of a session.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], [`EngineError::AudioChannels`] for a channel count that is not 1 or 2, or
    /// [`EngineError::Config`] with [`sdrkit_core::ConfigError::RateTooLow`] for a rate that is not above 0 or not
    /// finite.
    pub fn write_audio(
        &self,
        id: SessionId,
        samples: &[f32],
        channels: u16,
        rate_hz: f64,
    ) -> Result<(), EngineError> {
        self.audio_input(id)?.write(samples, channels, rate_hz)
    }

    /// The host audio input of session `id`, for [`AudioInput::write`]. It stays valid after the session closes; the
    /// audio then goes nowhere.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub fn audio_input(&self, id: SessionId) -> Result<AudioInput, EngineError> {
        let s = self.shared.session_or(id)?;
        Ok(AudioInput(Arc::clone(&s.link.streams)))
    }

    /// Copies the samples that the `tx.sink` of session `id` received into `out`, at most what `out` holds. `None` if none wait.
    /// Samples that the host does not take in time are lost, oldest first, and counted in the field [`TRANSMIT_LOST`].
    ///
    /// A session on a radio that can transmit sends its samples to the radio instead. A burst whose end the stack has
    /// written goes out at once. Another burst goes on the air 40 ms after its first samples and is fed as the stack
    /// writes it, whatever its length; the engine disarms at its end mark. If the stack writes less than the radio
    /// plays, the engine ends the burst at once, off the air, as failed. It sends only while the engine
    /// setting [`TRANSMIT_ALLOWED`] is on, else it refuses the burst. If the radio lost a part of a burst, the engine sends
    /// the burst again, at most twice; a burst longer than the session's transmit stream is sent once. Each burst ends
    /// with one [`TRANSMIT`] event, whose field [`BURST_ATTEMPTS`] is how many times the engine sent it, and the session's
    /// fields [`TX_SENT`], [`TX_FAILED`] and [`TX_LAST`] count bursts.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], [`EngineError::ToRadio`] for a session whose samples go to its radio.
    pub fn transmit(
        &self,
        id: SessionId,
        out: &mut [Complex32],
    ) -> Result<Option<IqRead>, EngineError> {
        let s = self.shared.session_or(id)?;
        if s.link.to_radio {
            return Err(EngineError::ToRadio);
        }
        let mut r = lock(&s.readers);
        Ok(s.link.streams.read_transmit(&mut r.transmit_at, out))
    }

    /// Copies the bytes of the first media packet of session `id` after counter `after` (0 for the oldest kept) into `out`.
    /// The session keeps the coded packets of the last ten seconds, at most 32 MiB, and decoded video frames of the last
    /// half second, at most 16 MiB; the field `session.media_lost` counts those that left unread. Pictures are not in it
    /// ([`Engine::picture`]). `None` if no packet follows `after`.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub fn media(
        &self,
        id: SessionId,
        after: u64,
        out: &mut Vec<u8>,
    ) -> Result<Option<MediaRead>, EngineError> {
        Ok(self
            .shared
            .session_or(id)?
            .link
            .streams
            .read_media(after, out))
    }

    /// Copies the first picture of session `id` after counter `after` (0 for the oldest kept) into `out`: compressed bytes,
    /// or raw pixels in the layout of the format's `pixels`. [`MediaRead::picture`] says which picture it is and whether
    /// more of it can come. The session keeps the newest state of each of its last 16 pictures. `None` if no picture
    /// follows `after`.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub fn picture(
        &self,
        id: SessionId,
        after: u64,
        out: &mut Vec<u8>,
    ) -> Result<Option<MediaRead>, EngineError> {
        Ok(self
            .shared
            .session_or(id)?
            .link
            .streams
            .read_picture(after, out))
    }

    /// Opens a tap on output `port` (`"<component>.<port>"`, or `"input"`) of the graph of session `id`: a copy of the stream
    /// at that point, in at most `chunks` pieces that wait for the host. The session waits for the call between two blocks.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], [`EngineError::NoGraph`] if the receiver is not a graph, [`EngineError::Graph`] if the
    /// graph has no such output.
    pub fn open_tap(&self, id: SessionId, port: &str, chunks: usize) -> Result<TapId, EngineError> {
        let s = self.shared.session_or(id)?;
        let at = port.to_owned();
        let reader = s
            .link
            .with_run(move |run| {
                let graph = run
                    .rx
                    .as_any_mut()
                    .and_then(|a| a.downcast_mut::<Graph>())
                    .ok_or(EngineError::NoGraph)?;
                Ok::<_, EngineError>(graph.tap(&at, chunks)?)
            })
            .ok_or(EngineError::Stopped)??;
        let mut r = lock(&s.readers);
        let tap = r.next_tap;
        r.next_tap += 1;
        r.taps.push(streams::Tap {
            id: tap,
            reader,
            chunk: sdrkit_graph::Buffer::default(),
            taken: 0,
        });
        Ok(TapId(tap))
    }

    /// Copies what waits in tap `tap` of session `id` into `out`: the values of one piece, from where the last call stopped,
    /// at most what `out` holds. `None` if nothing waits.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], [`EngineError::NoSuchTap`].
    pub fn read_tap(
        &self,
        id: SessionId,
        tap: TapId,
        out: &mut [f32],
    ) -> Result<Option<TapRead>, EngineError> {
        let s = self.shared.session_or(id)?;
        let mut r = lock(&s.readers);
        let t = r
            .taps
            .iter_mut()
            .find(|t| t.id == tap.0)
            .ok_or(EngineError::NoSuchTap { tap: tap.0 })?;
        Ok(streams::read_tap(t, out))
    }

    /// Closes tap `tap` of session `id`. The graph drops its copy at its next block.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], [`EngineError::NoSuchTap`].
    pub fn close_tap(&self, id: SessionId, tap: TapId) -> Result<(), EngineError> {
        let s = self.shared.session_or(id)?;
        let mut r = lock(&s.readers);
        let i = r
            .taps
            .iter()
            .position(|t| t.id == tap.0)
            .ok_or(EngineError::NoSuchTap { tap: tap.0 })?;
        r.taps.swap_remove(i);
        Ok(())
    }

    /// Copies the points of series `series` of owner `r` into `out` if the series changed after counter `since`. Returns its
    /// change counter and its line flag then, `None` if it did not change.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], [`EngineError::NoSuchSeries`] if the owner declares no such series.
    pub fn series(
        &self,
        r: OwnerRef,
        series: SeriesId,
        since: u64,
        out: &mut Vec<[f32; 2]>,
    ) -> Result<Option<SeriesRead>, EngineError> {
        self.shared.owner(r, |o| {
            let res = o.results();
            let counter = res
                .series_counter(series)
                .ok_or(EngineError::NoSuchSeries { series: series.0 })?;
            if counter <= since {
                return Ok(None);
            }
            let s = res
                .series(series)
                .ok_or(EngineError::NoSuchSeries { series: series.0 })?;
            out.clear();
            out.extend_from_slice(&s.points);
            Ok(Some(SeriesRead {
                counter,
                line: s.line,
            }))
        })?
    }

    /// Copies the cells of raster `raster` of owner `r` into `out`, row by row, if the raster changed after counter `since`.
    /// `None` if it did not change.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], [`EngineError::NoSuchRaster`] if the owner declares no such raster.
    pub fn raster(
        &self,
        r: OwnerRef,
        raster: RasterId,
        since: u64,
        out: &mut Vec<f32>,
    ) -> Result<Option<RasterRead>, EngineError> {
        self.shared.owner(r, |o| {
            let res = o.results();
            let counter = res
                .raster_counter(raster)
                .ok_or(EngineError::NoSuchRaster { raster: raster.0 })?;
            if counter <= since {
                return Ok(None);
            }
            let x = res
                .raster(raster)
                .ok_or(EngineError::NoSuchRaster { raster: raster.0 })?;
            out.clear();
            out.extend_from_slice(&x.values);
            Ok(Some(RasterRead {
                counter,
                width: x.width,
                height: x.height,
                x: AxisRead {
                    start: x.x.start,
                    step: x.x.step,
                },
                y: AxisRead {
                    start: x.y.start,
                    step: x.y.step,
                },
            }))
        })?
    }

    /// The registry of component kinds that the engine builds its graphs with.
    pub fn registry(&self) -> &Registry {
        &self.shared.components
    }

    /// What the graph of `plan` resolves on complex samples at `rate_hz` around `centre_hz` (0 for no input): each port
    /// with its format and each adapter it inserts. The plan stays as it is.
    ///
    /// # Errors
    ///
    /// [`EngineError::Graph`] if the plan does not build or does not configure.
    pub fn validate_graph(
        &self,
        plan: &GraphPlan,
        rate_hz: f64,
        centre_hz: f64,
    ) -> Result<ValidateReport, EngineError> {
        let sh = &self.shared;
        let mut p = plan.describe().plan(&sh.components)?;
        p.set_catalog(sh.components.catalog());
        let mut g = p.build()?;
        g.set_context(sh.context());
        Ok(g.validate(Format::iq(rate_hz, centre_hz, centre_hz))?
            .clone())
    }

    /// The graph of stack `stack` at its default settings, as a description.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchStack`], [`EngineError::Config`] if the stack cannot be made with its defaults,
    /// [`EngineError::NoGraph`] if its receiver is not a graph.
    pub fn stack_graph(&self, stack: &str) -> Result<Description, EngineError> {
        let sh = &self.shared;
        let desc = sh.stack(stack)?;
        let values = Values::defaults(&(desc.settings)());
        let mut rx = sh.make_receiver(desc, None, &values, &sh.station())?;
        rx.as_any_mut()
            .and_then(|a| a.downcast_mut::<Graph>())
            .map(|g| g.describe())
            .ok_or(EngineError::NoGraph)
    }

    /// The report of the derivations of radio `id`: each one made from its stream once, and the sessions that read it.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`], or [`EngineError::Graph`] once, if the derivations of the sessions did not build.
    pub fn derivations(&self, id: RadioId) -> Result<ValidateReport, EngineError> {
        let r = self
            .shared
            .radio(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(id)))?;
        match r.feed.take_problem() {
            Some(e) => Err(e.into()),
            None => Ok(r.feed.report()),
        }
    }

    /// The settings of every owner that has values (the engine, each radio, each session, each service) as a saved
    /// settings blob: bytes that the host stores as they are and gives to [`Engine::import_settings`] in a later run. The
    /// host does not read them. The engine setting [`TRANSMIT_ALLOWED`] is not in the blob: every run starts with
    /// transmit off.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] if an owner was removed while it was read, [`EngineError::Encode`] if a value is
    /// longer than 2^32 - 1 bytes.
    pub fn export_settings(&self) -> Result<Vec<u8>, EngineError> {
        saved::export(&self.shared)
    }

    /// Gives the frequency database the entries that the user cannot change (see
    /// [`sdrkit_svc_freqdb::supplied_entries`]). The list replaces the one before.
    pub fn set_frequency_entries(&self, entries: Vec<sdrkit_svc_freqdb::Entry>) {
        self.shared.services.freqdb.lock().set_supplied(entries);
    }

    /// Gives the scan service the channels that a scan of a stack steps through: for each stack id the name and the
    /// frequency of each channel. The list replaces the one before.
    pub fn set_scan_channels(&self, channels: sdrkit_svc_scan::PlanChannels) {
        self.shared.services.scan.lock().set_plan_channels(channels);
    }

    /// Applies a settings blob that [`Engine::export_settings`] wrote, in this run or an earlier one. An owner that the
    /// engine does not have now and a setting that an owner does not have are skipped. A setting of a named owner that the
    /// blob does not hold takes its default. All or nothing: a value that a schema refuses, or the engine setting
    /// [`TRANSMIT_ALLOWED`], refuses the whole blob and no value changes.
    ///
    /// # Errors
    ///
    /// [`EngineError::Import`]: [`ImportError::Version`] for a blob of a later version of the library,
    /// [`ImportError::NotSettings`] or [`ImportError::Damaged`] for bytes that are not a whole blob, else the owner key and
    /// the setting key at fault.
    pub fn import_settings(&self, blob: &[u8]) -> Result<(), EngineError> {
        saved::import(&self.shared, blob)
    }

    /// The JSON Schema (draft 2020-12) of the settings of owner `r`.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] if `r` refers to no owner.
    pub fn settings_schema(&self, r: OwnerRef) -> Result<String, EngineError> {
        self.shared.owner(r, |o| saved::json_schema(o.schema()))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        *lock(&self.shared.stop) = true;
        self.shared.wake.notify_all();
        if let Some(t) = self.control.take() {
            let _ = t.join();
        }
        let shares: Vec<_> = lock(&self.shared.shares)
            .iter()
            .map(|(_, s)| Arc::clone(s))
            .collect();
        for s in shares {
            s.owner.lock().shut();
        }
        let sessions: Vec<_> = lock(&self.shared.sessions)
            .iter()
            .map(|(_, s)| Arc::clone(s))
            .collect();
        for s in &sessions {
            s.stop();
            s.link.feed.unsubscribe_all();
        }
        for s in &sessions {
            s.join();
        }
        let radios: Vec<_> = lock(&self.shared.radios)
            .iter()
            .map(|(_, r)| Arc::clone(r))
            .collect();
        for r in radios {
            r.owner.lock().stop();
        }
    }
}

impl Shared {
    fn find(&self, key: &str) -> Result<OwnerRef, EngineError> {
        let key = OwnerKey::new(key.to_owned())?;
        let s = key.as_str();
        let none = || EngineError::NoSuchKey { key: s.to_owned() };
        if s == "engine" {
            return Ok(OwnerRef::Engine);
        }
        let (family, rest) = s.split_once('.').ok_or_else(none)?;
        let row = |i: Option<usize>| i.and_then(|i| u32::try_from(i).ok()).ok_or_else(none);
        let number = |prefix: char| {
            rest.strip_prefix(prefix)
                .and_then(|n| n.parse::<u64>().ok())
                .ok_or_else(none)
        };
        match family {
            "stack" => {
                row(self.keys.stacks.iter().position(|d| d.id == rest)).map(OwnerRef::StackKind)
            }
            "driver" => {
                row(self.keys.drivers.iter().position(|k| *k == rest)).map(OwnerRef::DriverKind)
            }
            "component" => row(self.keys.components.iter().position(|k| k == rest))
                .map(OwnerRef::ComponentKind),
            "service" => ServiceKind::ALL
                .iter()
                .find(|k| k.key() == s)
                .map(|k| OwnerRef::Service(k.row()))
                .ok_or_else(none),
            "radio" => {
                let id = RadioId::from_u64(number('r')?);
                self.radio(id).ok_or_else(none)?;
                Ok(OwnerRef::Radio(id))
            }
            "session" => {
                let id = SessionId::from_u64(number('s')?);
                self.session(id).ok_or_else(none)?;
                Ok(OwnerRef::Session(id))
            }
            "share" => {
                let id = ShareId::from_u64(number('h')?);
                self.share(id).ok_or_else(none)?;
                Ok(OwnerRef::Share(id))
            }
            _ => Err(none()),
        }
    }

    fn owner<R>(&self, r: OwnerRef, f: impl FnOnce(&mut dyn Owner) -> R) -> Result<R, EngineError> {
        let gone = || EngineError::NoSuchOwner(r);
        let kind = |i: u32| usize::try_from(i).map_err(|_| gone());
        match r {
            OwnerRef::Engine => Ok(self.engine.host(f)),
            OwnerRef::Radio(id) => {
                let radio = self.radio(id).ok_or_else(gone)?;
                let r = if radio.driver == SYNTH {
                    // A value that the test signal refuses is not set: the signal plays on as it was.
                    let sid = signal_id(&radio.address);
                    let test = |now: &Values, next: &Values| {
                        // A rate of 0 is Automatic: the radio keeps the rate that it has.
                        if values_rate(next) <= 0.0 && values_rate(now) > 0.0 {
                            return Ok(Held::none());
                        }
                        let served = self.signals.served_now(sid);
                        self.signals.check_change(
                            sid,
                            (now, served.as_ref()),
                            (next, served.as_ref()),
                        )
                    };
                    radio.owner.host(|o| {
                        f(&mut SignalGuard {
                            owner: o,
                            test: &test,
                        })
                    })
                } else {
                    radio.owner.host(f)
                };
                // The host set the rate to Automatic: the radio takes the rate of the stack that reads it.
                if radio.take_rate_given_back() {
                    if let Err(e) = self.resolve_rate_again(id) {
                        self.log(format!("{}: {e}", radio_key(id)));
                    }
                }
                Ok(r)
            }
            OwnerRef::Session(id) => {
                let s = self.session(id).ok_or_else(gone)?;
                let tuned = |o: &dyn Owner| o.values().map_or(0.0, |v| v.get(&TUNED));
                // A value of a session that the test signal it reads refuses is not set: the signal plays on as it was.
                let stack = lock(&s.control).stack;
                let signal = match (s.input, stack) {
                    (Input::Radio(on), Some(d)) => self.signal_for(on, d).map(|x| (d, x)),
                    _ => None,
                };
                // The values of the radio are read only when a value is set: a read of the session does not touch the
                // radio.
                let test = |now: &Values, next: &Values| match &signal {
                    Some((d, (sid, radio))) => match radio.owner.host(|o| o.values().cloned()) {
                        Some(radio) => self.signals.check_change(
                            sid,
                            (&radio, Some(&served_values(d, now))),
                            (&radio, Some(&served_values(d, next))),
                        ),
                        None => Ok(Held::none()),
                    },
                    None => Ok(Held::none()),
                };
                // The session takes a new frequency at the first sample of the move of the radio, if one follows: the
                // radio is told first.
                let (r, before, after) = {
                    let mut c = s.owner();
                    c.hold_tunes();
                    let before = tuned(&*c);
                    let r = if signal.is_some() {
                        f(&mut SignalGuard {
                            owner: &mut *c,
                            test: &test,
                        })
                    } else {
                        f(&mut *c)
                    };
                    (r, before, tuned(&*c))
                };
                // A new frequency of the session moves the radio if the band of the radio does not hold it. A scan
                // that tuned the session does not put it back afterwards: the frequency of the caller stays.
                if after.to_bits() != before.to_bits() {
                    lock(&self.scan_home).take_if(|h| h.session == id);
                    lock(&s.control).operator.set_frequency(after);
                    // A frequency that the host set ends a hop of the session.
                    let moved = self.end_hop_by(id, || {
                        self.drop_stack_rate(&s).and_then(|()| self.move_radio(id, &s, 0.0))
                    });
                    if let Err(e) = moved {
                        self.log(format!("{}: {e}", session_key(id)));
                    }
                }
                lock(&s.control).release_tunes();
                // A setting of the stack that gives the sample rate may have changed.
                if let Err(e) = self.follow_rate(&s) {
                    self.log(format!("{}: {e}", session_key(id)));
                }
                Ok(r)
            }
            OwnerRef::Stack(id) => {
                let s = self.session(id).ok_or_else(gone)?;
                if lock(&s.control).stack.is_none() {
                    return Err(gone());
                }
                let r = s.on_stack(f);
                if let Err(e) = self.follow_rate(&s) {
                    self.log(format!("{}: {e}", session_key(id)));
                }
                Ok(r)
            }
            OwnerRef::Component(id, handle) => {
                let s = self.session(id).ok_or_else(gone)?;
                s.on_component(handle, f).ok_or_else(gone)
            }
            OwnerRef::StackKind(i) => {
                let mut k = lock(self.stacks.get(kind(i)?).ok_or_else(gone)?);
                k.ensure(&self.adapters, &self.facilities)?;
                Ok(f(&mut *k))
            }
            OwnerRef::DriverKind(i) => {
                let mut k = lock(self.drivers.get(kind(i)?).ok_or_else(gone)?);
                Ok(f(&mut *k))
            }
            OwnerRef::ComponentKind(i) => {
                let mut k = lock(self.component_kinds.get(kind(i)?).ok_or_else(gone)?);
                k.ensure(&self.components)?;
                Ok(f(&mut *k))
            }
            OwnerRef::Service(i) => Ok(self.services.with(ServiceKind::at(i).ok_or_else(gone)?, f)),
            OwnerRef::Share(id) => Ok(self.share(id).ok_or_else(gone)?.owner.host(f)),
        }
    }

    /// Every owner whose settings a settings blob holds, with its key: the engine, each radio, each session, each service.
    pub(crate) fn owners_with_values(&self) -> Vec<(String, OwnerRef)> {
        let mut out = vec![("engine".to_owned(), OwnerRef::Engine)];
        out.extend(
            lock(&self.radios)
                .iter()
                .map(|(h, _)| RadioId::new(h))
                .map(|id| (radio_key(id), OwnerRef::Radio(id))),
        );
        out.extend(
            lock(&self.sessions)
                .iter()
                .map(|(h, _)| SessionId::new(h))
                .map(|id| (session_key(id), OwnerRef::Session(id))),
        );
        out.extend(
            ServiceKind::ALL
                .iter()
                .map(|k| (k.key().to_owned(), OwnerRef::Service(k.row()))),
        );
        out
    }

    /// The stack id of the test signal that radio `r` plays for a session of stack `d`, and the radio. `None` if `r` is
    /// not a radio of the test-signal driver whose address names `d`.
    fn signal_for(&self, r: RadioId, d: &StackDescriptor) -> Option<(String, Arc<Radio>)> {
        let radio = self.radio(r)?;
        if radio.driver != SYNTH {
            return None;
        }
        let id = signal_id(&radio.address);
        if id.split_once('/').map_or(id, |(s, _)| s) != d.id {
            return None;
        }
        Some((id.to_owned(), radio))
    }

    /// Makes the test signal that a session of stack `d` with the settings `values` on `input` would read, if `input` is
    /// a radio of the test-signal driver whose address names `d`. A start calls it, as the driver plays silence for a
    /// signal that it cannot make. The start keeps the signal that it returns for the driver when it succeeds
    /// ([`StackSignals::check`]).
    ///
    /// # Errors
    ///
    /// [`EngineError::Setting`] that names the setting the signal refuses, or [`EngineError::Config`] if the generator
    /// refuses its configuration.
    fn check_signal(
        &self,
        input: Input,
        d: &StackDescriptor,
        values: &Values,
    ) -> Result<Held, EngineError> {
        let Input::Radio(r) = input else {
            return Ok(Held::none());
        };
        let Some((sid, radio)) = self.signal_for(r, d) else {
            return Ok(Held::none());
        };
        match radio.owner.host(|o| o.values().cloned()) {
            Some(radio) => self
                .signals
                .check(&sid, &radio, Some(&served_values(d, values)))
                .map_err(SignalError::engine),
            None => Ok(Held::none()),
        }
    }

    fn radio(&self, id: RadioId) -> Option<Arc<Radio>> {
        lock(&self.radios).get(id.handle()).cloned()
    }

    fn session(&self, id: SessionId) -> Option<Arc<Session>> {
        lock(&self.sessions).get(id.handle()).cloned()
    }

    /// Session `id`, or [`EngineError::NoSuchOwner`].
    fn session_or(&self, id: SessionId) -> Result<Arc<Session>, EngineError> {
        self.session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))
    }

    /// The descriptor of stack `id`.
    fn stack(&self, id: &str) -> Result<&'static StackDescriptor, EngineError> {
        self.keys
            .stacks
            .iter()
            .find(|d| d.id == id)
            .copied()
            .ok_or_else(|| EngineError::NoSuchStack { id: id.to_owned() })
    }

    fn context(&self) -> RunContext {
        let f = &self.facilities;
        RunContext {
            storage: Arc::clone(&f.storage),
            clock: f.clock.clone(),
            random: f.random.clone(),
            codecs: Arc::clone(&f.codecs),
            transport: Some(Arc::clone(&self.graph_net)),
            generators: Some(Arc::clone(&self.generators)),
            pool: self.cores.pool.clone(),
            ..RunContext::default()
        }
    }

    /// The station that the engine owner's settings give.
    fn station(&self) -> sdrkit_core::Station {
        self.engine.lock().station()
    }

    /// The receiver of stack `desc` in `submode` with the settings `values`, with the host's facilities and `station`.
    fn make_receiver(
        &self,
        desc: &'static StackDescriptor,
        submode: Option<&str>,
        values: &Values,
        station: &sdrkit_core::Station,
    ) -> Result<Box<dyn Receiver>, EngineError> {
        let cfg = StackConfig::new(values.clone())
            .with_submode_opt(submode.map(str::to_owned))
            .with_adapters(Arc::clone(&self.adapters));
        let mut rx = (desc.make_receiver)(&cfg)?;
        rx.set_facilities(&self.facilities);
        rx.set_station(station);
        Ok(rx)
    }

    /// The driver kind `driver`, for a radio at `address`.
    fn driver_kind(&self, driver: &str, address: &str) -> Result<&Mutex<DriverKind>, EngineError> {
        let kind = self
            .keys
            .drivers
            .iter()
            .position(|k| *k == driver)
            .and_then(|i| self.drivers.get(i))
            .ok_or_else(|| EngineError::NoSuchDriver {
                key: driver.to_owned(),
            })?;
        if driver == SYNTH {
            // The test signal of a stack that the engine does not have names the stack in its error.
            let rest = signal_id(address);
            let stack = rest.split_once('/').map_or(rest, |(s, _)| s);
            if !stack.is_empty() {
                self.stack(stack)?;
            }
        }
        Ok(kind)
    }

    /// Opens the radio at `address` with the driver kind `driver` and the settings `values`, starts its stream, and writes
    /// its row into `held`, the engine owner that the caller holds, or into the engine owner after a lock.
    ///
    /// For a kind of USB radio the engine chooses the device that the selector `address` names from the list of the bus,
    /// and opens the driver with the selector of that one device, so that the table `devices` shows which device the
    /// radio is. With the empty selector it tries the free devices in turn and passes over one that another program
    /// holds or that the user may not open. A failure that stays with a device shows in the state of its row.
    fn open_radio(
        &self,
        mut held: Option<&mut EngineOwner>,
        driver: &str,
        address: &str,
        values: &Values,
    ) -> Result<RadioId, EngineError> {
        // A kind of USB radio on a bus that reaches no device: the open says why, whatever the driver.
        if let Some(why) = devices::availability(driver, self.usb.as_ref()).unavailable_why {
            return Err(EngineError::Driver(
                sdrkit_driver_core::DriverError::Unavailable { why },
            ));
        }
        let picked = self.usb_candidates(driver, address);
        if picked.is_empty() {
            return self.open_radio_at(held, driver, address, values);
        }
        let any = address.trim().is_empty();
        // The error of the first device counts: it is the one that the selector names first.
        let mut first = None;
        for p in picked {
            // The listing says that the system lets no program open the device: the driver is not asked.
            let opened = match p.blocked {
                Some(why) => Err(EngineError::Driver(
                    sdrkit_driver_core::DriverError::Unavailable { why },
                )),
                None => self.open_radio_at(held.as_deref_mut(), driver, &p.selector, values),
            };
            match opened {
                Ok(id) => {
                    self.usb_opened(id, &p.path);
                    return Ok(id);
                }
                Err(e) => {
                    let of_device = match &e {
                        EngineError::Driver(d) => {
                            self.usb_failed(&p.path, d);
                            matches!(
                                d,
                                sdrkit_driver_core::DriverError::Busy
                                    | sdrkit_driver_core::DriverError::Unavailable { .. }
                            )
                        }
                        _ => false,
                    };
                    first.get_or_insert(e);
                    if !(any && of_device) {
                        break;
                    }
                }
            }
        }
        Err(first.unwrap_or(EngineError::Stopped))
    }

    /// Opens the radio at `address` with the driver kind `driver`: [`Shared::open_radio`] after the choice of a USB device.
    fn open_radio_at(
        &self,
        held: Option<&mut EngineOwner>,
        driver: &str,
        address: &str,
        values: &Values,
    ) -> Result<RadioId, EngineError> {
        let kind = self.driver_kind(driver, address)?;
        if lock(kind).descriptor().opens == Opens::Once {
            let open = lock(&self.radios)
                .iter()
                .find(|(_, r)| r.driver == driver && r.address == address)
                .map(|(i, _)| RadioId::new(i));
            if let Some(radio) = open {
                return Err(EngineError::RadioOpen {
                    radio,
                    key: radio_key(radio),
                });
            }
        }
        // The rate on Automatic is the engine's to set, at every start of a stack ([`Shared::prepare_radio`]). A test
        // radio opens at the rate of the stack that its address names.
        let auto_rate = values_rate(values) <= 0.0;
        let resolved;
        let values = match self.signals.find(signal_id(address)) {
            Some((d, _)) if driver == SYNTH && auto_rate => {
                resolved = with_rate(values, d.rate_hz(&Values::defaults(&(d.settings)())))?;
                &resolved
            }
            _ => values,
        };
        // A test signal that refuses its values fails the open, as the driver would play silence. The signal that the
        // check makes is the one that the driver plays.
        let signal = if driver == SYNTH {
            self.signals
                .check(signal_id(address), values, None)
                .map_err(SignalError::engine)?
        } else {
            Held::none()
        };
        let open = lock(kind).open(address, values)?;
        let live = open.capabilities().realtime;
        let tx = open.capabilities().transmit.clone();
        // The test signal of a stack is made as it plays: its rows can have the time of the clock.
        let ctx = RunContext {
            present: driver == SYNTH,
            ..self.context()
        };
        let feed = Arc::new(Feed::new(live, Arc::clone(self.components.catalog()), ctx));
        let radio = Radio::start(open, address, feed, auto_rate, Arc::clone(&self.wake))?;
        signal.keep();
        let (d, a) = (radio.driver, radio.address.clone());
        let id = RadioId::new(lock(&self.radios).insert(Arc::new(radio)));
        let note = |e: &mut EngineOwner| {
            if let Some(mut r) = e.results.row_mut(&RADIOS, &radio_key(id)) {
                r.set(&catalog::DRIVER, d);
                r.set(&catalog::ADDRESS, &a);
                // A limit that the radio does not know is 0: the row has no value.
                let known = |hz: f64| (hz > 0.0).then_some(hz);
                r.set(&catalog::TRANSMIT, tx.is_some());
                r.opt(
                    &catalog::TX_MIN_HZ,
                    tx.as_ref().and_then(|t| known(t.min_freq_hz)),
                );
                r.opt(
                    &catalog::TX_MAX_HZ,
                    tx.as_ref().and_then(|t| known(t.max_freq_hz)),
                );
                r.opt(
                    &catalog::TX_MAX_RATE_HZ,
                    tx.as_ref().and_then(|t| known(t.rates.max_hz)),
                );
            }
        };
        match held {
            Some(e) => note(e),
            None => note(&mut self.engine.lock()),
        }
        Ok(id)
    }

    /// Sets `station` on the receiver of every session and for the test signals that are made from now on. The caller
    /// holds the engine owner's lock.
    fn give_station(&self, station: &sdrkit_core::Station) {
        lock(&self.signal_station).clone_from(station);
        let sessions: Vec<_> = lock(&self.sessions)
            .iter()
            .map(|(_, s)| Arc::clone(s))
            .collect();
        for s in sessions {
            let station = station.clone();
            let _ = s.link.with_run(move |run| run.rx.set_station(&station));
        }
    }

    /// Starts a session of `rx` on `input`. `stack` and `submode` name what `rx` runs. The session's row goes into `held`,
    /// the engine owner that the caller holds, or into the engine owner after a lock.
    fn start(
        &self,
        held: Option<&mut EngineOwner>,
        by: By,
        rx: Box<dyn Receiver>,
        input: Input,
        stack: Option<&'static StackDescriptor>,
        submode: Option<&str>,
    ) -> Result<SessionId, EngineError> {
        let feed = self.feed_for(input)?;
        // Only a receiver that transmits, on a radio that can transmit, has a transmit stream.
        let transmit = match input {
            Input::Radio(r) if rx.transmits() => self.transmit_room(r).unwrap_or(1),
            Input::Radio(_) | Input::Feed(_) | Input::Inside => 1,
        };
        let streams = Arc::new(Streams::new(AUDIO_CAP, transmit));
        self.start_on(held, by, rx, (input, feed), stack, submode, streams)
    }

    /// The transmit room of a session on radio `r` at the radio's rate. `None` if `r` is not open or cannot transmit.
    fn transmit_room(&self, r: RadioId) -> Option<usize> {
        let radio = self.radio(r)?;
        let o = radio.owner.lock();
        o.tx_values()?;
        Some(transmit_room(o.tuning().0))
    }

    /// Host feed `id`.
    fn feed(&self, id: FeedId) -> Result<Arc<HostFeed>, EngineError> {
        lock(&self.feeds)
            .get(id.handle())
            .cloned()
            .ok_or(EngineError::NoSuchFeed { feed: id })
    }

    /// The feed that a session on `input` reads: the radio's, the host's, or a feed of its own.
    fn feed_for(&self, input: Input) -> Result<Arc<Feed>, EngineError> {
        Ok(match input {
            Input::Radio(r) => Arc::clone(
                &self
                    .radio(r)
                    .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(r)))?
                    .feed,
            ),
            Input::Feed(id) => Arc::clone(&self.feed(id)?.feed),
            Input::Inside => Arc::new(Feed::new(
                false,
                Arc::clone(self.components.catalog()),
                self.context(),
            )),
        })
    }

    /// Starts a session of `rx` on `source`, with the streams `streams`: what the session's row names as its input, and the
    /// feed it reads. A channel's session names its radio and reads the feed that another session fills from the radio's
    /// stream.
    #[allow(clippy::too_many_arguments)]
    fn start_on(
        &self,
        held: Option<&mut EngineOwner>,
        by: By,
        rx: Box<dyn Receiver>,
        source: (Input, Arc<Feed>),
        stack: Option<&'static StackDescriptor>,
        submode: Option<&str>,
        streams: Arc<Streams>,
    ) -> Result<SessionId, EngineError> {
        let (input, feed) = source;
        let tx_side = match input {
            Input::Radio(r) if streams.has_transmit() => self.radio(r).and_then(|radio| {
                transmit::TxSide::new(
                    radio,
                    Arc::clone(&self.transmit_allowed),
                    self.facilities.clock.clone(),
                )
            }),
            Input::Radio(_) | Input::Feed(_) | Input::Inside => None,
        };
        let session = Arc::new(Session::new(
            rx,
            Arc::clone(&feed),
            streams,
            input,
            stack,
            submode,
            by,
            tx_side.is_some(),
            Arc::clone(&self.transmit_allowed),
            Arc::clone(&self.cores),
        )?);
        let id = SessionId::new(lock(&self.sessions).insert(Arc::clone(&session)));
        let (tx, blocks) = sync_channel(feed.queue());
        session
            .link
            .queue
            .blocks
            .store(feed.queue() as u64, Ordering::Relaxed);
        let subscribed = feed.subscribe(Sub {
            session: id.to_u64(),
            tx,
            dropped: Arc::clone(&session.link.dropped),
            demand: Arc::clone(&session.link.demand),
            queue: Arc::clone(&session.link.queue),
            steers: by != By::Share,
        });
        if !subscribed {
            lock(&self.sessions).remove(id.handle());
            let Input::Radio(r) = input else {
                return Err(EngineError::Stopped);
            };
            return Err(EngineError::NoSuchOwner(OwnerRef::Radio(r)));
        }
        let (s, events) = (Arc::clone(&session), Arc::clone(&self.events));
        let thread = std::thread::Builder::new()
            .name("sdrkit-session".into())
            .spawn(move || {
                // A fault in a receiver ends its session and no other part of the engine: the session shows it.
                let crashed = Arc::clone(&s);
                let run = std::panic::AssertUnwindSafe(|| session::run(s, blocks, events, tx_side));
                if let Err(cause) = std::panic::catch_unwind(run) {
                    let text = cause
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| cause.downcast_ref::<&str>().copied())
                        .unwrap_or("no message");
                    crashed.link.abandon();
                    lock(&crashed.control).crashed(text);
                }
            });
        let thread = match thread {
            Ok(t) => t,
            Err(e) => {
                feed.unsubscribe(id.to_u64());
                lock(&self.sessions).remove(id.handle());
                return Err(EngineError::thread(&e));
            }
        };
        *lock(&session.thread) = Some(thread);
        let note = |e: &mut EngineOwner| {
            if let Some(mut row) = e.results.row_mut(&SESSIONS, &session_key(id)) {
                row.opt(&catalog::STACK, stack.map(|d| d.id));
                match input {
                    Input::Radio(r) => row.set(&catalog::RADIO, &radio_key(r)),
                    Input::Feed(_) | Input::Inside => row.none(&catalog::RADIO),
                };
                row.set(&catalog::STATE, SessionState::Waiting);
            }
        };
        match held {
            Some(e) => note(e),
            None => note(&mut self.engine.lock()),
        }
        self.tell_scan(id);
        Ok(id)
    }

    fn stop_session(&self, id: SessionId) -> Result<(), EngineError> {
        let s = self
            .session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
        self.end_hop(id);
        s.stop();
        s.link.feed.unsubscribe(id.to_u64());
        set_state(&mut self.engine.lock(), id, SessionState::Stopped);
        Ok(())
    }

    fn close_session(&self, id: SessionId) -> Result<(), EngineError> {
        let s = {
            let mut t = lock(&self.sessions);
            let s = t
                .get(id.handle())
                .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
            if lock(&s.control).state != SessionState::Stopped {
                return Err(EngineError::StillRunning);
            }
            t.remove(id.handle())
                .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?
        };
        s.join();
        self.engine
            .lock()
            .results
            .remove_row(&SESSIONS, &session_key(id));
        Ok(())
    }

    /// Stops session `id`. The control thread closes it once its thread has ended, so the caller does not wait for it.
    fn end_session(&self, id: SessionId) -> Result<(), EngineError> {
        self.stop_session(id)?;
        lock(&self.closing).push(id);
        Ok(())
    }

    /// Closes the sessions that [`Shared::end_session`] stopped and whose threads have ended.
    fn close_ended(&self) {
        let ids = std::mem::take(&mut *lock(&self.closing));
        for id in ids {
            match self.session(id) {
                Some(s) if !s.finished() => lock(&self.closing).push(id),
                // A session that the host closed already is gone.
                Some(_) => {
                    let _ = self.close_session(id);
                }
                None => {}
            }
        }
    }

    /// Writes the stack of session `id` into its row of the `sessions` table.
    fn set_session_stack(&self, id: SessionId, stack: Option<&'static str>) {
        let mut e = self.engine.lock();
        if let Some(mut row) = e.results.row_mut(&SESSIONS, &session_key(id)) {
            row.opt(&catalog::STACK, stack);
        }
    }

    /// One tick of the control thread at wall time `now`.
    fn tick(&self, now: f64, work: &mut Work) {
        let radios: Vec<_> = lock(&self.radios)
            .iter()
            .map(|(h, r)| (RadioId::new(h), Arc::clone(r)))
            .collect();
        for (_, r) in radios {
            r.owner.lock().poll(now, &self.events);
        }
        let sessions: Vec<_> = lock(&self.sessions)
            .iter()
            .map(|(h, s)| (SessionId::new(h), Arc::clone(s)))
            .collect();
        let (ctx, offset_min) = {
            let e = self.engine.lock();
            let ctx = sdrkit_svc_events::Context {
                unix_time: now,
                receiver: e.position(),
            };
            (ctx, e.offset_min())
        };
        work.views.clear();
        for (id, s) in &sessions {
            let state = {
                let c = s.owner();
                s.link.take_fresh(&mut work.events, &mut work.requests);
                c.state
            };
            work.views.push((*id, self.scan_view(s)));
            work.states.push((*id, state));
            if !work.events.is_empty() {
                self.services.observe(
                    &work.events,
                    Some(*id),
                    &ctx,
                    offset_min,
                    &self.events,
                    &mut work.tick,
                );
                lock(&s.control).pass_on(&mut work.events);
            }
            for r in work.requests.drain(..) {
                self.handle(Requester::Session(*id), Ask::Host(r));
            }
        }
        {
            let mut e = self.engine.lock();
            for (id, state) in work.states.drain(..) {
                set_state(&mut e, id, state);
            }
        }
        // The scanner knows what each session receives and shows. A session that started since the sessions were read
        // was told at its start and is left as it is.
        if work.views != work.views_told {
            let mut scan = self.services.scan.lock();
            for (id, v) in &work.views {
                if !work.views_told.iter().any(|(i, t)| i == id && t == v) {
                    scan.set_view(&session_key(*id), Some(v.told()));
                }
            }
            for (id, _) in &work.views_told {
                if !work.views.iter().any(|(i, _)| i == id) {
                    scan.set_view(&session_key(*id), None);
                }
            }
            drop(scan);
            work.views_told.clone_from(&work.views);
        }
        self.services
            .tick(now, &ctx, offset_min, &self.events, &mut work.tick);
        for (from, r) in std::mem::take(&mut work.tick.requests) {
            self.handle(Requester::Service(from), Ask::Host(r));
        }
        for a in std::mem::take(&mut work.tick.actions) {
            self.handle(Requester::Service(ServiceKind::Automation), Ask::Action(a));
        }
        self.serve_signals();
        self.step_scan(now);
        self.step_sweep(now);
        self.step_bank(now);
        self.step_identify();
        self.give_records();
        self.step_devices(now);
        self.step_shares(now);
        self.step_hops();
        self.close_ended();
        self.expire_followers(now);
        if !work.tick.problems.is_empty() {
            let mut e = self.engine.lock();
            for p in work.tick.problems.drain(..) {
                e.results.results_mut().log(p);
            }
        }
    }

    /// Gives the test signal of each radio of the test-signal driver the stack settings of the session that it serves:
    /// of the sessions on the radios with its address that run the stack of the address, the one with the lowest
    /// number that has not stopped. The signal is made again when those settings change. Without such a session the
    /// signal has the settings of its radio alone.
    fn serve_signals(&self) {
        for (address, same, wanted) in self.signals_wanted() {
            if self.give_signal(&address, wanted) {
                for r in same {
                    r.owner.lock().renew();
                }
            }
        }
    }

    /// For each address of the open radios of the test-signal driver: the address without its scheme, its radios, and
    /// the stack settings that [`Shared::serve_signals`] gives its signal.
    fn signals_wanted(&self) -> Vec<(String, Vec<Arc<Radio>>, Option<ServedBy>)> {
        let radios: Vec<_> = lock(&self.radios)
            .iter()
            .filter(|(_, r)| r.driver == SYNTH)
            .map(|(h, r)| (RadioId::new(h), Arc::clone(r)))
            .collect();
        if radios.is_empty() {
            return Vec::new();
        }
        let sessions: Vec<_> = lock(&self.sessions)
            .iter()
            .map(|(_, s)| Arc::clone(s))
            .collect();
        let mut out: Vec<(String, Vec<Arc<Radio>>, Option<ServedBy>)> = Vec::new();
        for (_, radio) in &radios {
            let address = signal_id(&radio.address);
            if out.iter().any(|(a, _, _)| a == address) {
                continue;
            }
            let stack = address.split_once('/').map_or(address, |(s, _)| s);
            let same: Vec<_> = radios
                .iter()
                .filter(|(_, r)| r.address == radio.address)
                .collect();
            let wanted = sessions.iter().find_map(|s| {
                let Input::Radio(on) = s.input else {
                    return None;
                };
                if !same.iter().any(|(id, _)| *id == on) {
                    return None;
                }
                let c = s.owner();
                let d = c.stack.filter(|d| d.id == stack)?;
                if c.state == SessionState::Stopped {
                    return None;
                }
                let values = served_values(d, c.values()?);
                Some(ServedBy { radio: on, values })
            });
            let same = same.into_iter().map(|(_, r)| Arc::clone(r)).collect();
            out.push((address.to_owned(), same, wanted));
        }
        out
    }

    /// Gives the test signal of `address` the settings `wanted`, and returns whether its settings changed. Settings
    /// whose radio has closed are dropped: the check and the write hold the lock of the radios, as the close holds it
    /// when it removes the settings of its radio.
    fn give_signal(&self, address: &str, wanted: Option<ServedBy>) -> bool {
        let radios = lock(&self.radios);
        if wanted
            .as_ref()
            .is_some_and(|w| radios.get(w.radio.handle()).is_none())
        {
            return false;
        }
        let mut served = lock(&self.signal_settings);
        if served.get(address) == wanted.as_ref() {
            return false;
        }
        match wanted {
            Some(v) => served.insert(address.to_owned(), v),
            None => served.remove(address),
        };
        true
    }

    /// What session `s` receives and shows, for the scan service. Before its first samples, and while its feed is made
    /// again, the session receives what its input gives ([`Shared::input_tuning`]). Takes the control state of `s`, then
    /// the tables of radios or feeds: the caller holds no lock.
    fn scan_view(&self, s: &Session) -> ScanView {
        let (tuned, offset, stack, results) = {
            let c = s.owner();
            (
                c.values().map_or(0.0, |v| v.get(&TUNED)),
                session::follow_offset(s.input, c.operator.as_ref()),
                c.stack.map(|d| d.id),
                Arc::clone(c.results().decl()),
            )
        };
        let (mut rate, mut centre) = s.link.feed.tuning();
        if !(rate > 0.0 && centre.is_finite()) {
            (rate, centre) = self.input_tuning(s.input);
        }
        ScanView {
            listened_hz: sdrkit_core::listened_hz(tuned, centre, offset),
            span_hz: rate,
            stack,
            results,
        }
    }

    /// The rate and centre that `input` gives before its samples arrive: on a radio, the rate and centre asked of it
    /// (else those it reported); on a host feed, those of the feed; on an inside input, zeros.
    fn input_tuning(&self, input: Input) -> (f64, f64) {
        match input {
            Input::Radio(r) => self.radio(r).map_or((0.0, 0.0), |radio| {
                let o = radio.owner.lock();
                let (rate, centre) = o.tuning();
                let asked = |key: &str| match o.values()?.schema().id(key).ok() {
                    Some(id) => match o.values()?.value(id)? {
                        sdrkit_core::Value::Hz(hz) if *hz > 0.0 => Some(*hz),
                        _ => None,
                    },
                    None => None,
                };
                (
                    asked(sdrkit_driver_core::RATE.field.key).unwrap_or(rate),
                    asked(sdrkit_driver_core::CENTRE.field.key).unwrap_or(centre),
                )
            }),
            Input::Feed(f) => self
                .feed(f)
                .map_or((0.0, 0.0), |h| (h.rate_hz, h.centre_hz)),
            Input::Inside => (0.0, 0.0),
        }
    }

    /// Tells the scan service what session `id` receives and shows now, so that a scan command can name the session as
    /// soon as it has started.
    fn tell_scan(&self, id: SessionId) {
        if let Some(s) = self.session(id) {
            let view = self.scan_view(&s).told();
            self.services
                .scan
                .lock()
                .set_view(&session_key(id), Some(view));
        }
    }

    /// Steps the scan that runs, on the session it
    /// tunes, and carries out its requests. When the scan ends or stops, the session returns to what it ran when the scan
    /// started, unless the host set the frequency of the session in the meantime.
    fn step_scan(&self, now: f64) {
        let running = self
            .services
            .scan
            .lock()
            .session()
            .and_then(crate::parse_session_key);
        let Some((id, s)) = running.and_then(|id| Some((id, self.session(id)?))) else {
            self.drop_demand(&self.scan_demand);
            self.scan_back();
            return;
        };
        // A survey gets every frame of the whole band of the radio, in order, before the step.
        let need = self.services.scan.lock().spectrum_need();
        if let Err(why) = self.follow_need(&self.scan_demand, id, need) {
            self.log(format!("{}: {why}", ServiceKind::Scan.key()));
        }
        if need.is_some() {
            let rate = self.radio_rate(id).unwrap_or(0.0);
            self.take_frames(&self.scan_demand, |centre, bins| {
                self.services
                    .scan
                    .lock()
                    .frame(&sdrkit_svc_scan::SpectrumFrame {
                        time_s: now,
                        centre_hz: centre,
                        span_hz: rate,
                        bins_db: bins,
                    });
            });
        }
        // A new spectrum frame of the session goes to the scan first: it shows which channels around the tuned one have
        // a signal.
        {
            let mut seen = lock(&self.scan_spectrum);
            let (frames, bins) = &mut *seen;
            let frame = {
                let f = lock(&s.link.demand.frame);
                (f.made != *frames && !f.values.is_empty()).then(|| {
                    *frames = f.made;
                    bins.clear();
                    bins.extend_from_slice(&f.values);
                    (f.centre_hz, f.span_hz)
                })
            };
            if let Some((centre_hz, span_hz)) = frame {
                self.services.scan.lock().spectrum(
                    now,
                    &sdrkit_svc_scan::Spectrum {
                        centre_hz,
                        span_hz,
                        power_db: bins,
                    },
                );
            }
        }
        let (step, home) = {
            let c = s.owner();
            let home = ScanHome {
                session: id,
                spectrum: c.values().is_some_and(|v| v.get(&SPECTRUM)),
                tuned_hz: c.values().map_or(0.0, |v| v.get(&TUNED)),
                stack: c.stack,
                submode: c.submode.clone().unwrap_or_default(),
            };
            (self.services.scan.lock().step(now, Some(c.results())), home)
        };
        match step {
            Some(sdrkit_svc_scan::Step::Tune) => {
                {
                    let mut held = lock(&self.scan_home);
                    if held.as_ref().is_none_or(|h| h.session != id) {
                        // The scan reads the spectrum of the session, also when the host shows none.
                        if !home.spectrum {
                            let _ = lock(&s.control).apply(
                                sdrkit_core::SettingSet::id(SessionSetting::Spectrum),
                                &sdrkit_core::Value::Bool(true),
                            );
                        }
                        *held = Some(home);
                    }
                }
                let req = self
                    .services
                    .scan
                    .lock()
                    .target()
                    .map(|t| HostRequest::Tune {
                        session: session_key(id),
                        freq_hz: Some(t.freq_hz),
                        target: TuneTarget::of_stack(t.stack),
                        submode: t.submode.to_owned(),
                        rate_hz: t.rate_hz,
                        bandwidth_hz: t.bandwidth_hz,
                        settings: t.settings.to_vec(),
                    });
                if let Some(r) = req {
                    self.handle(Requester::Service(ServiceKind::Scan), Ask::Host(r));
                }
            }
            Some(sdrkit_svc_scan::Step::Done) => {
                self.drop_demand(&self.scan_demand);
                self.scan_back();
            }
            None => {}
        }
    }
}

impl Shared {
    /// Puts the session that the scan tuned back to what it ran when the scan started, if a scan tuned one.
    fn scan_back(&self) {
        let Some(h) = lock(&self.scan_home).take() else {
            return;
        };
        if let (false, Some(s)) = (h.spectrum, self.session(h.session)) {
            let _ = lock(&s.control).apply(
                sdrkit_core::SettingSet::id(SessionSetting::Spectrum),
                &sdrkit_core::Value::Bool(false),
            );
        }
        // The wide rate of a look of a survey counts no more, also for a session that goes back to no frequency.
        if let Some(s) = self.session(h.session) {
            if let Err(e) = self.drop_stack_rate(&s) {
                self.log(format!("{}: {e}", session_key(h.session)));
            }
        }
        let back = HostRequest::Tune {
            session: session_key(h.session),
            freq_hz: (h.tuned_hz > 0.0).then_some(h.tuned_hz),
            target: h
                .stack
                .map_or(TuneTarget::Running, |d| TuneTarget::Stack(d.id.to_owned())),
            submode: h.submode,
            rate_hz: None,
            bandwidth_hz: None,
            settings: Vec::new(),
        };
        self.handle(Requester::Service(ServiceKind::Scan), Ask::Host(back));
    }
}

/// What a session ran when a scan started on it: the scan puts it back when it ends.
struct ScanHome {
    session: SessionId,
    /// The session requested a spectrum before the scan.
    spectrum: bool,
    tuned_hz: f64,
    stack: Option<&'static StackDescriptor>,
    submode: String,
}

/// What a session receives and shows, as the scan service is told it ([`sdrkit_svc_scan::SessionView`]).
#[derive(Clone)]
struct ScanView {
    listened_hz: f64,
    span_hz: f64,
    stack: Option<&'static str>,
    results: Arc<sdrkit_core::ResultsDecl>,
}

impl PartialEq for ScanView {
    /// Equal numbers and stack, and the same results declaration.
    fn eq(&self, o: &ScanView) -> bool {
        self.listened_hz.to_bits() == o.listened_hz.to_bits()
            && self.span_hz.to_bits() == o.span_hz.to_bits()
            && self.stack == o.stack
            && Arc::ptr_eq(&self.results, &o.results)
    }
}

impl ScanView {
    fn told(&self) -> sdrkit_svc_scan::SessionView {
        sdrkit_svc_scan::SessionView {
            listened_hz: self.listened_hz,
            span_hz: self.span_hz,
            stack: self.stack.unwrap_or_default().to_owned(),
            results: Arc::clone(&self.results),
        }
    }
}

/// What the control thread keeps between ticks, so a tick allocates only when it has more to hold than before.
#[derive(Default)]
struct Work {
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    states: Vec<(SessionId, SessionState)>,
    /// What each session receives and shows now, and what the scanner was told last.
    views: Vec<(SessionId, ScanView)>,
    views_told: Vec<(SessionId, ScanView)>,
    tick: Tick,
}

/// The loop of the control thread: about ten times a second it reads every radio, passes the sessions' events to the
/// services, carries out the requests, runs the services and copies the state of every session into the engine's `sessions`
/// table.
fn control(sh: &Shared) {
    let mut work = Work::default();
    let mut stopped = lock(&sh.stop);
    loop {
        let (g, _) = sh
            .wake
            .wait_timeout(stopped, TICK)
            .unwrap_or_else(|p| p.into_inner());
        stopped = g;
        if *stopped {
            return;
        }
        drop(stopped);
        sh.tick(sh.facilities.clock.now(), &mut work);
        *lock(&sh.ticks) += 1;
        sh.ticked.notify_all();
        stopped = lock(&sh.stop);
    }
}

/// Writes the state of session `id` into its row of the `sessions` table, if the row is there and holds another state.
fn set_state(e: &mut EngineOwner, id: SessionId, state: SessionState) {
    let key = session_key(id);
    let r = e.results.results();
    let held = r
        .decl()
        .table_id(SESSIONS.key)
        .and_then(|t| r.table(t))
        .and_then(|t| t.row(&key))
        .map(|row| row.get(&catalog::STATE));
    if matches!(held, Some(s) if s != Some(state)) {
        if let Some(mut row) = e.results.row_mut(&SESSIONS, &key) {
            row.set(&catalog::STATE, state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::publish::field;
    use sdrkit_core::{DeclareSetting, Schema, Setting, Value};

    const WIDTH: Setting<field::F64> = Setting::new("width", "Width", "", 1.0);
    /// The radio centre of the test signal; 0 takes the frequency where the session listens.
    const CENTRE_AT: Setting<field::Hz> = Setting::new("gen_plan_centre_hz", "Centre", "Hz", 0.0);
    const SPAN: Setting<field::Hz> = Setting::new("gen_span", "Span", "Hz", 1e6);

    fn schema() -> Arc<sdrkit_core::Schema> {
        Arc::new(Schema::of(&[&WIDTH, &CENTRE_AT, &SPAN]))
    }

    fn no_receiver(_: &StackConfig) -> Result<Box<dyn Receiver>, sdrkit_core::ConfigError> {
        Err(sdrkit_core::ConfigError::NoRate)
    }

    /// Silence at the rate of the configuration.
    struct Quiet(f64);
    impl sdrkit_core::Generator for Quiet {
        fn rate_hz(&self) -> f64 {
            self.0
        }
        fn generate(&mut self, out: &mut [Complex32]) -> usize {
            out.fill(Complex32::default());
            out.len()
        }
    }

    /// Silence, placed against [`CENTRE_AT`] or, at 0, against the frequency where the session listens.
    fn quiet(cfg: &GenConfig) -> Result<Box<dyn sdrkit_core::Generator>, sdrkit_core::ConfigError> {
        if cfg.settings.get(&CENTRE_AT) == 0.0 {
            let _ = cfg.tuned_hz();
        }
        Ok(Box::new(Quiet(cfg.rate_hz)))
    }

    static PLAN: StackDescriptor = StackDescriptor {
        id: "plan",
        name: "Plan",
        group: "",
        blurb: "",
        tuning: sdrkit_core::Tuning {
            tune_offset_hz: 100e3,
            ..sdrkit_core::Tuning::anywhere(0.0, 2e6, 1e6, 1e6)
        },
        submodes: &[],
        settings: schema,
        make_receiver: no_receiver,
        make_generator: Some(quiet),
        lock_profile: sdrkit_core::LockProfile::DEFAULT,
        signal_types: &[],
        auto_gain: sdrkit_core::AutoGain::Radio,
        verify: None,
        benchmarks: None,
    };

    fn signals() -> StackSignals {
        StackSignals {
            stacks: vec![&PLAN],
            station: Arc::default(),
            served: Served::default(),
            slots: Mutex::default(),
        }
    }

    /// The values of a test radio of `plan` with `set` by key.
    fn radio(s: &StackSignals, set: &[(&str, Value)]) -> Values {
        let mut decls = vec![
            sdrkit_driver_core::CENTRE.decl(),
            sdrkit_driver_core::RATE.decl(),
            sdrkit_driver_core::GAIN.decl(),
        ];
        decls.extend(
            <sdrkit_algo_sim::chain::GenSetting as sdrkit_core::SettingSet>::SETTINGS
                .iter()
                .map(|d| d.decl()),
        );
        decls.extend(s.settings("plan").unwrap());
        let mut v = Values::defaults(&Arc::new(Schema::new(decls).unwrap()));
        for (k, x) in set {
            let id = v.schema().id(k).unwrap();
            v.set(id, x).unwrap();
        }
        v
    }

    fn input(i: &SignalInputs, key: &str) -> Option<Value> {
        i.values.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn a_signal_that_reads_where_the_session_listens_is_made_again_when_it_moves() {
        let s = signals();
        let v = radio(&s, &[("centre_hz", Value::Hz(921.6e6))]);
        let made = s.make("plan", &v, 2e6).unwrap().inputs;
        assert_eq!(made.tuned_hz, Some(921.5e6));
        assert!(made.makes_same(&s.inputs("plan", &v, 2e6)));
        let w = radio(&s, &[("centre_hz", Value::Hz(915.1e6))]);
        assert!(!made.makes_same(&s.inputs("plan", &w, 2e6)));
        // A signal at a centre of its own does not read it: a retune leaves it.
        let at = |centre: f64| {
            radio(
                &s,
                &[("centre_hz", Value::Hz(centre)), (CENTRE_AT.key(), Value::Hz(1e9))],
            )
        };
        let made = s.make("plan", &at(921.6e6), 2e6).unwrap().inputs;
        assert_eq!(made.tuned_hz, None);
        assert!(made.makes_same(&s.inputs("plan", &at(915.1e6), 2e6)));
    }

    #[test]
    fn a_setting_that_the_signal_does_not_read_leaves_its_inputs() {
        let s = signals();
        let v = |centre: f64, gain: f64| {
            radio(
                &s,
                &[
                    ("centre_hz", Value::Hz(centre)),
                    ("gain", Value::F64(gain)),
                    (CENTRE_AT.key(), Value::Hz(1e9)),
                ],
            )
        };
        let made = s.make("plan", &v(100e6, 10.0), 2e6).unwrap().inputs;
        assert!(made.makes_same(&s.inputs("plan", &v(200e6, 30.0), 2e6)));
        assert!(!made.makes_same(&s.inputs("plan", &radio(&s, &[]), 1e6)));
    }

    #[test]
    fn the_session_gives_its_stack_settings_and_the_test_signal_settings_it_changed() {
        let s = signals();
        let v = radio(&s, &[(CENTRE_AT.key(), Value::Hz(1e9)), ("gen_span", Value::Hz(2e6))]);
        let mut session = Values::defaults(&schema());
        let set = |x: &mut Values, k: &str, val: Value| {
            let id = x.schema().id(k).unwrap();
            x.set(id, &val).unwrap();
        };
        set(&mut session, "width", Value::F64(7.0));
        let serve = |values: &Values| {
            let by = ServedBy {
                radio: RadioId::from_u64(1),
                values: values.clone(),
            };
            lock(&s.served).insert("plan".into(), by);
        };
        serve(&session);
        let i = s.inputs("plan", &v, 2e6);
        assert_eq!(input(&i, "width"), Some(Value::F64(7.0)));
        // The session leaves the test signal settings at their defaults: those of the radio stay.
        assert_eq!(input(&i, CENTRE_AT.key()), Some(Value::Hz(1e9)));
        assert_eq!(input(&i, "gen_span"), Some(Value::Hz(2e6)));
        set(&mut session, CENTRE_AT.key(), Value::Hz(868e6));
        serve(&session);
        let i = s.inputs("plan", &v, 2e6);
        assert_eq!(input(&i, CENTRE_AT.key()), Some(Value::Hz(868e6)));
        assert_eq!(input(&i, "gen_span"), Some(Value::Hz(2e6)));
    }

    /// An engine with the stack `plan` and no network.
    fn plan_engine() -> Engine {
        Engine::new(Setup {
            facilities: Facilities::default(),
            transport: Arc::new(StdNet),
            stacks: vec![&PLAN],
            families: Vec::new(),
            components: Registry::standard(),
        })
        .unwrap()
    }

    #[test]
    fn the_settings_of_a_session_of_a_closed_radio_are_not_given_to_a_test_signal() {
        let engine = plan_engine();
        let at = engine.radio_schema(SYNTH, "plan").unwrap();
        let r = engine
            .open_radio(SYNTH, "plan", &Values::defaults(&at))
            .unwrap();
        let sh = &engine.shared;
        let by = ServedBy {
            radio: r,
            values: Values::defaults(&schema()),
        };
        assert!(sh.give_signal("plan", Some(by.clone())));
        engine.close_radio(r).unwrap();
        assert!(lock(&sh.signal_settings).is_empty());
        // A tick that read the sessions before the close gives nothing after it.
        assert!(!sh.give_signal("plan", Some(by)));
        assert!(lock(&sh.signal_settings).is_empty());
    }
}
