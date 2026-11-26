//! Lightning position by time difference of arrival (`lightning_tdoa`): every station times sferics on a disciplined sample
//! clock; one station, the server, collects the detections of all stations over UDP and locates each strike with a 95 %
//! error ellipse.
//!
//! | Module | Contents |
//! |---|---|
//! | [`timer`] | the sferic timer: impulses timed at the half-peak point of the leading edge, with a carrier notch in front |
//! | [`station`] | one station: the timer, the timing-transmitter timer, the clocks of each source, the packets it sends |
//! | [`server`] | the server: the checks of the datagrams, the station table, common-view alignment, matching and fixes |
//! | [`packet`] | the datagrams, the replay guard and the rate limit |
//! | [`gen`] | the test signal |

pub mod gen;
pub mod packet;
pub mod server;
pub mod station;
pub mod timer;

use crate::family::{self, REPORT_S};
use packet::{network_allowed, Packet, MAX_NAME, MIN_KEY};
use sdrkit_algo_locate::clock::TimeSource;
use sdrkit_algo_locate::Propagation;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::net::{host_port, Datagram, DatagramOptions, Transport};
use crate::typed::{
    LightningStrikeEvent, LightningTdoaDatagramArgs, LightningTdoaFields, LightningTdoaStationRow,
    LightningTdoaStrikeRow, LightningTdoaTimeMarkArgs, LIGHTNING_STRIKE, LIGHTNING_TDOA_DATAGRAM,
    LIGHTNING_TDOA_STATIONS, LIGHTNING_TDOA_STRIKES, LIGHTNING_TDOA_TIME_MARK,
};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::Setting;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Choice, Clock, Complex32, ConfigError, Event, LockProfile, LockState, Record,
    ResultsDecl, Schema, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Status,
    Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, RunContext, StackGraph,
};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::shaped::{rows_of, RowOut, Shaped};
use sdrkit_util_objects::Seen;
use server::{NetStation, Server, ServerConfig, StrikeRow};
use station::{HostTime, Station, StationConfig, TimingTx};
use std::fmt::Write as _;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

pub use gen::{make_generator, TdoaGenerator, TdoaSignal};

/// The stack id.
pub const ID: &str = "lightning_tdoa";
/// The least input rate, Hz: 10 µs a sample.
pub const MIN_RATE: f64 = 100e3;
/// The rate the stack asks for, Hz.
pub const ASK_RATE: f64 = 1e6;
/// The dial of the test signal, Hz.
pub const TEST_DIAL_HZ: f64 = 100e3;
/// The timing transmitter is used when its carrier lies within this fraction of the rate from the tuned centre.
pub const TIMING_SPAN: f64 = 0.45;
/// The most datagrams read in one call.
const READS_PER_CALL: usize = 256;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: the figures and rules of the timing sources (their 1-sigmas and \
     biases, the fit windows, the rate prior of 50 ppm, the refusal of outlying marks), the sferic timer (the 10 µs power \
     average, the floor of 1 ms means, the end 3 dB under the threshold after 100 µs, 5 ms as too long, the 1 ms merge), \
     the MSK timer, the 200 Bd symbol rate of the timing transmitters, the ground-wave velocity 0.9993 c and the hop \
     model, the common-view alignment, the matching bounds, the solver's start grid and limits, and the packet format, \
     which is sdrkit's own. The framework gives no hardware timestamp of a sample, so GPS and GNSS time comes only from the \
     host through the time_mark command.";

sdrkit_core::choice_enum! {
    /// What a station does in the network.
    pub enum Role {
        /// Times sferics and shows its clock; sends nothing.
        Local = ("local", "Local"),
        /// Sends detections, boundary readings and hellos to the server.
        Client = ("client", "Client"),
        /// Receives, checks, matches and locates.
        Server = ("server", "Server"),
    }
}

sdrkit_core::choice_enum! {
    /// How the computer clock is kept.
    pub enum SystemClock {
        /// Not synchronised.
        System = ("system", "Not synchronised"),
        /// NTP.
        Ntp = ("ntp", "NTP"),
        /// PTP.
        Ptp = ("ptp", "PTP"),
    }
}

impl SystemClock {
    /// The timing source of the computer clock.
    pub fn source(self) -> TimeSource {
        match self {
            SystemClock::System => TimeSource::System,
            SystemClock::Ntp => TimeSource::Ntp,
            SystemClock::Ptp => TimeSource::Ptp,
        }
    }
}

/// The role of the station.
pub const ROLE: Setting<field::Choice<Role>> = Setting::choice("role", "Role", Role::Local)
    .help("Local times sferics and sends nothing. A client sends to the server. The server collects and locates.");
/// The server address of a client.
pub const SERVER: Setting<field::Text> =
    Setting::new("server", "Server address (client)", "", "").help("host or host:port.");
/// The UDP port.
pub const PORT: Setting<field::I64> =
    Setting::new("port", "UDP port", "", packet::DEFAULT_PORT as i64).range(1.0, 65_535.0, 1.0);
/// The network key.
pub const NETWORK_KEY: Setting<field::Text> = Setting::new(
    "network_key",
    "Network key (secret)",
    "",
    "",
)
.help("12 characters or more; shorter keeps the network off. The host keeps it; it is never sent.");
/// The station name.
pub const STATION_NAME: Setting<field::Text> = Setting::new(
    "station_name",
    "Station name",
    "",
    "",
)
.help("1 to 24 printable characters. Its hash is the station id.");
/// The server's radio counts as a station.
pub const SERVER_IS_STATION: Setting<field::Bool> = Setting::new(
    "server_is_station",
    "The server's radio counts as a station",
    "",
    true,
);
/// How the computer clock is kept.
pub const SYSTEM_CLOCK: Setting<field::Choice<SystemClock>> =
    Setting::choice("system_clock", "Computer clock", SystemClock::Ntp);
/// The error of the computer clock.
pub const SYSTEM_SIGMA_US: Setting<field::F64> =
    Setting::new("system_sigma_us", "Computer clock error", "us", 0.0)
        .range(1.0, 1e6, 1.0)
        .zero_means("the typical figure: PTP 100 µs, NTP 5 ms, not synchronised 200 ms");
/// Use the radio's GPS time.
pub const USE_RADIO_TIME: Setting<field::Bool> =
    Setting::new("use_radio_time", "Use the radio's GPS time", "", true)
        .help("GPS marks that the host gives (time_mark) are used.");
/// The id of the timing transmitter.
pub const TIMING_ID: Setting<field::I64> =
    Setting::new("timing_id", "Timing transmitter id", "", 1)
        .range(1.0, 255.0, 1.0)
        .help("The same at every station of a network.");
/// The label of the timing transmitter.
pub const TIMING_LABEL: Setting<field::Text> =
    Setting::new("timing_label", "Timing transmitter", "", "");
/// The carrier of the timing transmitter.
pub const TIMING_HZ: Setting<field::Hz> =
    Setting::new("timing_hz", "Timing transmitter frequency", "Hz", 0.0)
        .range(3e3, 30e6, 1.0)
        .zero_means("no timing transmitter")
        .help("Used when it lies within 0.45 times the rate of the tuned centre.");
/// The symbol rate of the timing transmitter.
pub const TIMING_BAUD: Setting<field::F64> =
    Setting::new("timing_baud", "Timing transmitter symbol rate", "Bd", 200.0)
        .range(10.0, 2000.0, 1.0)
        .help("No public document gives the rate of each transmitter; 200 Bd is assumed.");
/// The latitude of the timing transmitter.
pub const TIMING_LAT: Setting<field::F64> =
    Setting::new("timing_lat", "Timing transmitter latitude", "°", 0.0).range(-90.0, 90.0, 0.0001);
/// The longitude of the timing transmitter.
pub const TIMING_LON: Setting<field::F64> =
    Setting::new("timing_lon", "Timing transmitter longitude", "°", 0.0)
        .range(-180.0, 180.0, 0.0001);
/// The start level of the sferic timer.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Sferic threshold", "dB", 12.0)
        .range(6.0, 30.0, 0.5)
        .help("A sferic starts when the 10 µs power average rises this far over the floor.");
/// The ground-wave velocity over the speed of light.
pub const VELOCITY_FACTOR: Setting<field::F64> = Setting::new(
    "velocity_factor",
    "Ground-wave velocity / c",
    "",
    0.9993,
)
.range(0.99, 1.0, 0.0001)
.help("An assumption: 0.1 % error moves fixes by about 0.3 km at 400 km.");
/// Ionosphere hops on long paths.
pub const HOPS: Setting<field::Bool> =
    Setting::new("hops", "Ionosphere hops for long paths", "", false)
        .help("Paths of 1500 km or more travel by hops between the ground and the reflecting layer.");
/// The height of the reflecting layer.
pub const HOP_HEIGHT_KM: Setting<field::F64> =
    Setting::new("hop_height_km", "Reflection height", "km", 80.0).range(60.0, 100.0, 1.0);
/// Fine stations for a fix.
pub const MIN_STATIONS: Setting<field::I64> =
    Setting::new("min_stations", "Fine stations for a fix", "", 4)
        .range(3.0, 64.0, 1.0)
        .help("Three-station fixes, which nothing checks, only at 3.");
/// The UTC of sample 0 of a file.
pub const START_UTC: Setting<field::F64> = Setting::new(
    "start_utc",
    "UTC of sample 0 (files; 0 = host clock)",
    "s",
    0.0,
)
.range(0.0, 1e11, 1.0);
/// The mean strike rate of the test signal.
pub const GEN_STRIKE_RATE: Setting<field::F64> = Setting::new(
    "gen_strike_rate",
    "Test signal: strikes a second",
    "1/s",
    2.0,
)
.range(0.0, 50.0, 0.1)
.section("Test signal")
.help("Strikes come at random gaps, at least 2 ms apart, within 3° of latitude and 4° of longitude of 52.5 N 13.4 E.")
.advanced();
/// The test signal holds the timing transmitter.
pub const GEN_TIMING_TX: Setting<field::Bool> =
    Setting::new("gen_timing_tx", "Test signal: timing transmitter", "", false)
        .section("Test signal")
        .help("A 200 Bd MSK transmitter at 52.0 N 10.0 E, amplitude 0.05.")
        .advanced();
/// The offset of the test signal's timing transmitter.
pub const GEN_TIMING_OFFSET_HZ: Setting<field::Hz> = Setting::new(
    "gen_timing_offset_hz",
    "Test signal: timing transmitter offset",
    "Hz",
    50_000.0,
)
.range(-1e6, 1e6, 1.0)
.section("Test signal")
.help("From the tuned centre; at most the rate / 2.5.")
.advanced();
/// The latitude of the test signal's receiving station.
pub const GEN_STATION_LAT: Setting<field::F64> = Setting::new(
    "gen_station_lat",
    "Test signal: station latitude",
    "°",
    gen::TEST_STATION.0,
)
.range(-90.0, 90.0, 0.0001)
.section("Test signal")
.help("Where the test signal is heard, when the host gives no station position.")
.advanced();
/// The longitude of the test signal's receiving station.
pub const GEN_STATION_LON: Setting<field::F64> = Setting::new(
    "gen_station_lon",
    "Test signal: station longitude",
    "°",
    gen::TEST_STATION.1,
)
.range(-180.0, 180.0, 0.0001)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of [`TdoaDecoder`].
    pub enum DecoderSetting {
        /// The role.
        Role = ROLE,
        /// The server address.
        Server = SERVER,
        /// The UDP port.
        Port = PORT,
        /// The network key.
        NetworkKey = NETWORK_KEY,
        /// The station name.
        StationName = STATION_NAME,
        /// The server's radio counts.
        ServerIsStation = SERVER_IS_STATION,
        /// The computer clock.
        SystemClock = SYSTEM_CLOCK,
        /// Its error.
        SystemSigma = SYSTEM_SIGMA_US,
        /// The radio's GPS time.
        UseRadioTime = USE_RADIO_TIME,
        /// The timing transmitter id.
        TimingId = TIMING_ID,
        /// Its label.
        TimingLabel = TIMING_LABEL,
        /// Its carrier.
        TimingHz = TIMING_HZ,
        /// Its symbol rate.
        TimingBaud = TIMING_BAUD,
        /// Its latitude.
        TimingLat = TIMING_LAT,
        /// Its longitude.
        TimingLon = TIMING_LON,
        /// The sferic threshold.
        Threshold = THRESHOLD,
        /// The ground-wave velocity.
        VelocityFactor = VELOCITY_FACTOR,
        /// Hops.
        Hops = HOPS,
        /// The reflection height.
        HopHeight = HOP_HEIGHT_KM,
        /// Fine stations for a fix.
        MinStations = MIN_STATIONS,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the lightning position stack.
    pub enum TdoaSetting {
        /// The role.
        Role = ROLE,
        /// The server address.
        Server = SERVER,
        /// The UDP port.
        Port = PORT,
        /// The network key.
        NetworkKey = NETWORK_KEY,
        /// The station name.
        StationName = STATION_NAME,
        /// The server's radio counts.
        ServerIsStation = SERVER_IS_STATION,
        /// The computer clock.
        SystemClock = SYSTEM_CLOCK,
        /// Its error.
        SystemSigma = SYSTEM_SIGMA_US,
        /// The radio's GPS time.
        UseRadioTime = USE_RADIO_TIME,
        /// The timing transmitter id.
        TimingId = TIMING_ID,
        /// Its label.
        TimingLabel = TIMING_LABEL,
        /// Its carrier.
        TimingHz = TIMING_HZ,
        /// Its symbol rate.
        TimingBaud = TIMING_BAUD,
        /// Its latitude.
        TimingLat = TIMING_LAT,
        /// Its longitude.
        TimingLon = TIMING_LON,
        /// The sferic threshold.
        Threshold = THRESHOLD,
        /// The ground-wave velocity.
        VelocityFactor = VELOCITY_FACTOR,
        /// Hops.
        Hops = HOPS,
        /// The reflection height.
        HopHeight = HOP_HEIGHT_KM,
        /// Fine stations for a fix.
        MinStations = MIN_STATIONS,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The strike rate of the test signal.
        GenStrikeRate = GEN_STRIKE_RATE,
        /// The timing transmitter of the test signal.
        GenTimingTx = GEN_TIMING_TX,
        /// Its offset.
        GenTimingOffset = GEN_TIMING_OFFSET_HZ,
        /// The latitude of the test signal's station.
        GenStationLat = GEN_STATION_LAT,
        /// Its longitude.
        GenStationLon = GEN_STATION_LON,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Restarts the detectors.
pub const RESET: Command = Command::new("reset", "Reset").help(
    "Empties the tables and counters and learns the floor of the sferic timer again; the clock discipline is kept.",
);
sdrkit_core::commands! {
    /// The commands of [`TdoaDecoder`].
    pub enum TdoaCommand {
        /// Restarts the detectors.
        Reset = RESET,
        /// Feeds the server a datagram.
        Datagram = LIGHTNING_TDOA_DATAGRAM.command,
        /// A GPS or GNSS time mark.
        TimeMark = LIGHTNING_TDOA_TIME_MARK.command,
    }
}

impl Shaped for StrikeRow {
    type Row = LightningTdoaStrikeRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let r = self.rounded();
        out.put(&LightningTdoaStrikeRow {
            utc_ns: r.utc_ns,
            lat: r.lat,
            lon: r.lon,
            major_km: r.major_km,
            minor_km: r.minor_km,
            az_deg: r.az_deg,
            stations: r.stations as i64,
            also_heard: r.also_heard as i64,
            rms_us: r.rms_us,
            chi2: r.chi2,
            dof: r.dof as i64,
            unchecked: r.unchecked,
            ambiguous: r.ambiguous,
            utc: r.utc,
            time: r.utc.then_some(sdrkit_core::interface::Time(r.utc_ns as f64 * 1e-9)),
        });
    }
}

impl StrikeRow {
    /// The values of the strike as the table and the event show them: positions to 1e-5°, the ellipse to 1 m and 0.1°,
    /// the rms to 1 ns and the chi-square to 0.01.
    fn rounded(&self) -> StrikeRow {
        StrikeRow {
            lat: (self.lat * 1e5).round() / 1e5,
            lon: (self.lon * 1e5).round() / 1e5,
            major_km: (self.major_km * 1e3).round() / 1e3,
            minor_km: (self.minor_km * 1e3).round() / 1e3,
            az_deg: (self.az_deg * 10.0).round() / 10.0,
            rms_us: (self.rms_us * 1e3).round() / 1e3,
            chi2: (self.chi2 * 100.0).round() / 100.0,
            used: Vec::new(),
            heard: Vec::new(),
            ..*self
        }
    }

    /// The event `lightning.strike` of the strike at signal time `time` on `freq_hz`.
    fn event(&self, time: f64, freq_hz: f64) -> Event {
        let r = self.rounded();
        LIGHTNING_STRIKE.event(
            time,
            freq_hz,
            &LightningStrikeEvent {
                utc_ns: r.utc_ns,
                lat: r.lat,
                lon: r.lon,
                major_km: r.major_km,
                minor_km: r.minor_km,
                az_deg: r.az_deg,
                stations: r.stations as i64,
                also_heard: r.also_heard as i64,
                rms_us: r.rms_us,
                chi2: r.chi2,
                dof: r.dof as i64,
                unchecked: r.unchecked,
                ambiguous: r.ambiguous,
                utc: r.utc,
            },
        )
    }
}

/// A row of the station table at a server clock.
struct StationRow<'a> {
    s: &'a NetStation,
    now_ns: i64,
}

impl Shaped for StationRow<'_> {
    type Row = LightningTdoaStationRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let s = self.s;
        out.put(&LightningTdoaStationRow {
            id: &format!("{:08x}", s.id),
            name: &s.name,
            lat: s.position.lat_deg,
            lon: s.position.lon_deg,
            time_source: s.source.map_or("none", TimeSource::name),
            sigma_us: (s.clock_sigma_s * 1e8).round() / 100.0,
            age_s: (self.now_ns.saturating_sub(s.last_ns) as f64 * 1e-8).round() / 10.0,
            detections: i64::try_from(s.detections).unwrap_or(i64::MAX),
            used: i64::try_from(s.used).unwrap_or(i64::MAX),
            fine: s.fine(),
            tied_to_utc: s.tied_to_utc,
            cv_correction_us: s.correction_s.map(|c| (c * 1e8).round() / 100.0),
        });
    }
}

/// The settings of the decoder, as applied.
#[derive(Clone, Debug)]
struct Config {
    role: Role,
    server: String,
    port: i64,
    network_key: String,
    station_name: String,
    server_is_station: bool,
    system_clock: SystemClock,
    system_sigma_us: f64,
    use_radio_time: bool,
    timing_id: i64,
    timing_label: String,
    timing_hz: f64,
    timing_baud: f64,
    timing_lat: f64,
    timing_lon: f64,
    threshold_db: f64,
    velocity_factor: f64,
    hops: bool,
    hop_height_km: f64,
    min_stations: i64,
    start_utc: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            role: ROLE.default,
            server: String::new(),
            port: PORT.default,
            network_key: String::new(),
            station_name: String::new(),
            server_is_station: SERVER_IS_STATION.default,
            system_clock: SYSTEM_CLOCK.default,
            system_sigma_us: SYSTEM_SIGMA_US.default,
            use_radio_time: USE_RADIO_TIME.default,
            timing_id: TIMING_ID.default,
            timing_label: String::new(),
            timing_hz: TIMING_HZ.default,
            timing_baud: TIMING_BAUD.default,
            timing_lat: TIMING_LAT.default,
            timing_lon: TIMING_LON.default,
            threshold_db: THRESHOLD.default,
            velocity_factor: VELOCITY_FACTOR.default,
            hops: HOPS.default,
            hop_height_km: HOP_HEIGHT_KM.default,
            min_stations: MIN_STATIONS.default,
            start_utc: START_UTC.default,
        }
    }
}

impl Config {
    fn propagation(&self) -> Propagation {
        Propagation {
            velocity_factor: self.velocity_factor,
            hops: self.hops,
            hop_height_m: self.hop_height_km * 1e3,
        }
    }

    fn timing(&self) -> Option<TimingTx> {
        (self.timing_hz > 0.0).then(|| TimingTx {
            id: self.timing_id.clamp(1, 255) as u8,
            label: self.timing_label.clone(),
            freq_hz: self.timing_hz,
            baud: self.timing_baud,
            position: Position::new(self.timing_lat, self.timing_lon),
        })
    }

    fn name_ok(&self) -> bool {
        (1..=MAX_NAME).contains(&self.station_name.len())
            && self.station_name.bytes().all(|c| (0x20..=0x7e).contains(&c))
    }

    fn host_bias_s(&self) -> f64 {
        if self.system_sigma_us > 0.0 {
            self.system_sigma_us * 1e-6
        } else {
            self.system_clock.source().typical_sigma_s()
        }
    }
}

/// The lightning position decoder: complex samples in, strike events out.
pub struct TdoaDecoder {
    cfg: Config,
    rate: f64,
    centre_hz: f64,
    configured: bool,
    station: Option<Station>,
    server: Option<Server>,
    clock: Clock,
    live: bool,
    position: Option<Position>,
    height_m: f64,
    transport: Option<Arc<dyn Transport>>,
    socket: Option<Box<dyn Datagram>>,
    net_key: String,
    network: String,
    start_utc: f64,
    sent: u64,
    send_errors: u64,
    rx: Vec<u8>,
    datagrams: Vec<Vec<u8>>,
    packets: Vec<Packet>,
    shown: u64,
    cleared: bool,
    status: Status,
}

impl Default for TdoaDecoder {
    fn default() -> Self {
        TdoaDecoder {
            cfg: Config::default(),
            rate: 0.0,
            centre_hz: 0.0,
            configured: false,
            station: None,
            server: None,
            clock: Clock::fixed(0.0),
            live: false,
            position: None,
            height_m: 0.0,
            transport: None,
            socket: None,
            net_key: String::new(),
            network: String::new(),
            start_utc: 0.0,
            sent: 0,
            send_errors: 0,
            rx: vec![0; packet::MAX_LEN + 1],
            datagrams: Vec::new(),
            packets: Vec::new(),
            shown: u64::MAX,
            cleared: false,
            status: Status::default(),
        }
    }
}

impl TdoaDecoder {
    /// The UTC of sample 0 of a recording: the setting, else the host clock, else the start of the test signals.
    fn file_start_utc(&self) -> f64 {
        if self.cfg.start_utc > 0.0 {
            self.cfg.start_utc
        } else {
            let now = self.clock.now();
            if now > 0.0 {
                now.floor()
            } else {
                family::TEST_START_UTC
            }
        }
    }

    /// The server clock now, ns: the station's best clock, else the host clock.
    fn now_ns(&self) -> i64 {
        self.station
            .as_ref()
            .and_then(Station::now_ns)
            .unwrap_or_else(|| {
                let s = if self.live {
                    self.clock.now()
                } else {
                    self.start_utc + self.station.as_ref().map_or(0.0, Station::now_s)
                };
                (s * 1e9) as i64
            })
    }

    /// Makes the station for the input format, keeping nothing.
    fn build_station(&mut self) -> Result<(), ConfigError> {
        let rate = self.rate;
        let timing = self.cfg.timing().and_then(|tx| {
            let off = tx.freq_hz - self.centre_hz;
            (off.abs() <= TIMING_SPAN * rate && tx.baud <= rate / 4.0).then_some((tx, off))
        });
        let source = self.cfg.system_clock.source();
        let bias_s = self.cfg.host_bias_s();
        let host = if self.live {
            HostTime::Live { source, bias_s }
        } else {
            HostTime::File {
                start_utc_ns: (self.start_utc * 1e9) as i64,
                source,
                bias_s,
            }
        };
        let mut st = Station::new(StationConfig {
            rate,
            threshold_db: self.cfg.threshold_db,
            timing,
            host,
        })
        .map_err(|_| ConfigError::RateTooLow {
            need: MIN_RATE,
            got: rate,
        })?;
        st.set_position(self.position, self.height_m);
        st.set_name(self.own_name().as_deref());
        self.station = Some(st);
        Ok(())
    }

    /// The name of this station: the setting when it is valid; the server's own station is "server" without one.
    fn own_name(&self) -> Option<String> {
        if self.cfg.name_ok() {
            Some(self.cfg.station_name.clone())
        } else if self.cfg.role == Role::Server {
            Some("server".to_owned())
        } else {
            None
        }
    }

    /// Makes the server when the role is server.
    fn build_server(&mut self) {
        self.server = if self.cfg.role == Role::Server {
            Server::new(ServerConfig {
                network_key: self.cfg.network_key.clone(),
                propagation: self.cfg.propagation(),
                min_stations: self.cfg.min_stations.clamp(3, 64) as usize,
                timing: self.cfg.timing(),
            })
            .ok()
        } else {
            None
        };
        self.shown = u64::MAX;
    }

    /// Opens or closes the socket for the network settings. Runs outside `process`: opening may resolve a host name.
    fn open_network(&mut self) {
        let c = &self.cfg;
        let key = format!(
            "{}|{}|{}|{}|{}|{}",
            c.role.key(),
            c.server,
            c.port,
            c.station_name,
            c.network_key.chars().count(),
            self.transport.is_some()
        );
        if key == self.net_key && (self.socket.is_some() || !self.network.is_empty()) {
            return;
        }
        self.net_key = key;
        self.socket = None;
        self.network.clear();
        let name_needed = c.role == Role::Client;
        let named = !name_needed || c.name_ok();
        let allowed = c.network_key.chars().count() >= MIN_KEY
            && named
            && (1..=65_535).contains(&c.port)
            && (c.role != Role::Client || network_allowed(&c.network_key, &c.station_name, c.port));
        let reason = match (c.role, &self.transport) {
            (Role::Local, _) => Some("off: local role".to_owned()),
            _ if !allowed => Some(format!(
                "off: the network needs a key of {MIN_KEY} characters or more{}",
                if name_needed {
                    " and a station name of 1 to 24 printable characters"
                } else {
                    ""
                }
            )),
            (_, None) => Some("off: the host gives no network".to_owned()),
            _ => None,
        };
        if let Some(r) = reason {
            self.network = r;
            return;
        }
        let Some(t) = self.transport.clone() else {
            return;
        };
        let port = c.port as u16;
        match c.role {
            Role::Client => match host_port(&c.server, Some(port)) {
                Ok((host, p)) => match t.datagram(host, p) {
                    Ok(s) => {
                        self.network = format!("on: sending to {}", sdrkit_core::net::show(host, p));
                        self.socket = Some(s);
                    }
                    Err(e) => self.network = format!("off: {e}"),
                },
                Err(_) => self.network = "off: no server address".to_owned(),
            },
            Role::Server => {
                let local = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
                match t.bind(&DatagramOptions::at(local)) {
                    Ok(s) => {
                        self.network = format!("on: listening on port {port}");
                        self.socket = Some(s);
                    }
                    Err(e) => self.network = format!("off: {e}"),
                }
            }
            Role::Local => {}
        }
    }

    /// Sends, receives, matches and emits after the station took new samples.
    fn network_step(&mut self, out: &mut Outbox, now_ns: i64) {
        let Some(st) = self.station.as_mut() else {
            return;
        };
        match self.cfg.role {
            Role::Local => st.discard(),
            Role::Client => {
                self.datagrams.clear();
                if self.socket.is_some() {
                    st.datagrams(now_ns, &self.cfg.network_key, &mut self.datagrams);
                } else {
                    st.discard();
                }
                if let Some(sock) = &mut self.socket {
                    for d in &self.datagrams {
                        match sock.send(d) {
                            Ok(()) => self.sent += 1,
                            Err(_) => self.send_errors += 1,
                        }
                    }
                }
            }
            Role::Server => {
                self.packets.clear();
                st.packets(now_ns, &mut self.packets);
                let own = self.cfg.server_is_station;
                let Some(server) = self.server.as_mut() else {
                    return;
                };
                if own {
                    for p in &self.packets {
                        let _ = server.accept(p, now_ns);
                    }
                }
                if let Some(sock) = &mut self.socket {
                    for _ in 0..READS_PER_CALL {
                        match sock.try_recv_from(&mut self.rx) {
                            Ok(Some((n, from))) => {
                                let _ = server.datagram(
                                    &self.rx[..n.min(self.rx.len())],
                                    &from.ip().to_string(),
                                    now_ns,
                                );
                            }
                            _ => break,
                        }
                    }
                }
                if server.run(now_ns) {
                    let t = st.now_s();
                    let centre = self.centre_hz;
                    for row in server.take_new() {
                        out.messages(0).push(row.event(t, centre));
                    }
                }
            }
        }
    }

    /// Writes the server's tables and map points.
    fn write_server(&mut self, p: &mut Published, now_s: f64) {
        let now_ns = self.now_ns();
        let Some(server) = &self.server else {
            if std::mem::take(&mut self.cleared) {
                p.clear_rows(&LIGHTNING_TDOA_STRIKES);
                p.clear_rows(&LIGHTNING_TDOA_STATIONS);
                p.results_mut().clear_points();
            }
            return;
        };
        let newest = server.strikes().front().map_or(0, |r| r.seq);
        if newest != self.shown || std::mem::take(&mut self.cleared) {
            self.shown = newest;
            rows_of(
                server.strikes().iter().rev(),
                |r: &StrikeRow, w| write!(w, "{:08}", r.seq),
                &LIGHTNING_TDOA_STRIKES,
                now_s,
                p,
            );
            let r = p.results_mut();
            r.clear_points();
            let table = r.decl().table_id(LIGHTNING_TDOA_STRIKES.def().key);
            let mut id = String::new();
            let mut label = String::new();
            for s in server.strikes() {
                id.clear();
                label.clear();
                let _ = write!(id, "strike/{:08}", s.seq);
                let _ = write!(
                    label,
                    "{} stations, {:.1} x {:.1} km at {:.0}°",
                    s.stations, s.major_km, s.minor_km, s.az_deg
                );
                r.update_point(&id, s.lat, s.lon, &label, None);
                // The row key is the point id without "strike/".
                r.link_point(&id, table.map(|t| (t, &id["strike/".len()..])));
            }
        }
        let rows: Vec<StationRow<'_>> = server
            .stations()
            .iter()
            .map(|s| StationRow { s, now_ns })
            .collect();
        rows_of(
            rows.iter(),
            |r: &StationRow<'_>, w| write!(w, "{:08x}", r.s.id),
            &LIGHTNING_TDOA_STATIONS,
            now_s,
            p,
        );
    }
}

/// A count as a field value.
fn count(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A refused command argument.
fn refused(args: &Values, key: &str) -> CommandError {
    let id = args.schema().id(key).unwrap_or(SettingId(0));
    CommandError::BadArgument {
        arg: id,
        error: SettingError::NotPermitted { id },
    }
}

impl Decoder for TdoaDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "lightning.tdoa"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        let c = &mut self.cfg;
        match s {
            DecoderSetting::Role => c.role = ROLE.read(id, value)?,
            DecoderSetting::Server => SERVER.read(id, value)?.clone_into(&mut c.server),
            DecoderSetting::Port => c.port = PORT.read(id, value)?,
            DecoderSetting::NetworkKey => {
                NETWORK_KEY.read(id, value)?.clone_into(&mut c.network_key);
            }
            DecoderSetting::StationName => {
                STATION_NAME.read(id, value)?.clone_into(&mut c.station_name);
            }
            DecoderSetting::ServerIsStation => {
                c.server_is_station = SERVER_IS_STATION.read(id, value)?;
            }
            DecoderSetting::SystemClock => c.system_clock = SYSTEM_CLOCK.read(id, value)?,
            DecoderSetting::SystemSigma => c.system_sigma_us = SYSTEM_SIGMA_US.read(id, value)?,
            DecoderSetting::UseRadioTime => c.use_radio_time = USE_RADIO_TIME.read(id, value)?,
            DecoderSetting::TimingId => c.timing_id = TIMING_ID.read(id, value)?,
            DecoderSetting::TimingLabel => {
                TIMING_LABEL.read(id, value)?.clone_into(&mut c.timing_label);
            }
            DecoderSetting::TimingHz => c.timing_hz = TIMING_HZ.read(id, value)?,
            DecoderSetting::TimingBaud => c.timing_baud = TIMING_BAUD.read(id, value)?,
            DecoderSetting::TimingLat => c.timing_lat = TIMING_LAT.read(id, value)?,
            DecoderSetting::TimingLon => c.timing_lon = TIMING_LON.read(id, value)?,
            DecoderSetting::Threshold => c.threshold_db = THRESHOLD.read(id, value)?,
            DecoderSetting::VelocityFactor => c.velocity_factor = VELOCITY_FACTOR.read(id, value)?,
            DecoderSetting::Hops => c.hops = HOPS.read(id, value)?,
            DecoderSetting::HopHeight => c.hop_height_km = HOP_HEIGHT_KM.read(id, value)?,
            DecoderSetting::MinStations => c.min_stations = MIN_STATIONS.read(id, value)?,
            DecoderSetting::StartUtc => c.start_utc = START_UTC.read(id, value)?,
        }
        if !self.configured {
            return Ok(());
        }
        match s {
            DecoderSetting::Threshold => {
                if let Some(st) = &mut self.station {
                    st.set_threshold(self.cfg.threshold_db);
                }
            }
            DecoderSetting::StationName => {
                let name = self.own_name();
                if let Some(st) = &mut self.station {
                    st.set_name(name.as_deref());
                }
                self.open_network();
            }
            DecoderSetting::Server | DecoderSetting::Port | DecoderSetting::NetworkKey => {
                if s == DecoderSetting::NetworkKey {
                    self.build_server();
                }
                self.open_network();
            }
            DecoderSetting::Role => {
                self.build_server();
                let name = self.own_name();
                if let Some(st) = &mut self.station {
                    st.set_name(name.as_deref());
                }
                self.open_network();
            }
            DecoderSetting::VelocityFactor
            | DecoderSetting::Hops
            | DecoderSetting::HopHeight
            | DecoderSetting::MinStations => self.build_server(),
            DecoderSetting::TimingId
            | DecoderSetting::TimingHz
            | DecoderSetting::TimingBaud
            | DecoderSetting::TimingLat
            | DecoderSetting::TimingLon => {
                self.build_server();
                let _ = self.build_station();
            }
            DecoderSetting::SystemClock | DecoderSetting::SystemSigma | DecoderSetting::StartUtc => {
                if !self.live {
                    self.start_utc = self.file_start_utc();
                }
                let _ = self.build_station();
            }
            DecoderSetting::ServerIsStation
            | DecoderSetting::UseRadioTime
            | DecoderSetting::TimingLabel => {}
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let c = &self.cfg;
        values_of([
            (DecoderSetting::Role, Value::Choice(c.role.index())),
            (DecoderSetting::Server, Value::Text(c.server.clone())),
            (DecoderSetting::Port, Value::I64(c.port)),
            (DecoderSetting::NetworkKey, Value::Text(c.network_key.clone())),
            (DecoderSetting::StationName, Value::Text(c.station_name.clone())),
            (DecoderSetting::ServerIsStation, Value::Bool(c.server_is_station)),
            (DecoderSetting::SystemClock, Value::Choice(c.system_clock.index())),
            (DecoderSetting::SystemSigma, Value::F64(c.system_sigma_us)),
            (DecoderSetting::UseRadioTime, Value::Bool(c.use_radio_time)),
            (DecoderSetting::TimingId, Value::I64(c.timing_id)),
            (DecoderSetting::TimingLabel, Value::Text(c.timing_label.clone())),
            (DecoderSetting::TimingHz, Value::Hz(c.timing_hz)),
            (DecoderSetting::TimingBaud, Value::F64(c.timing_baud)),
            (DecoderSetting::TimingLat, Value::F64(c.timing_lat)),
            (DecoderSetting::TimingLon, Value::F64(c.timing_lon)),
            (DecoderSetting::Threshold, Value::F64(c.threshold_db)),
            (DecoderSetting::VelocityFactor, Value::F64(c.velocity_factor)),
            (DecoderSetting::Hops, Value::Bool(c.hops)),
            (DecoderSetting::HopHeight, Value::F64(c.hop_height_km)),
            (DecoderSetting::MinStations, Value::I64(c.min_stations)),
            (DecoderSetting::StartUtc, Value::F64(c.start_utc)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        TdoaCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TdoaCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TdoaCommand::Reset => {
                self.clear();
                if let Some(st) = &mut self.station {
                    st.reset_detectors();
                }
            }
            TdoaCommand::Datagram => {
                let LightningTdoaDatagramArgs { bytes } = LIGHTNING_TDOA_DATAGRAM.args(args)?;
                let now = self.now_ns();
                let Some(server) = &mut self.server else {
                    return Err(refused(args, "bytes"));
                };
                if self.cfg.network_key.chars().count() < MIN_KEY {
                    return Err(refused(args, "bytes"));
                }
                server
                    .datagram(&bytes, "command", now)
                    .map_err(|_| refused(args, "bytes"))?;
            }
            TdoaCommand::TimeMark => {
                let a: LightningTdoaTimeMarkArgs = LIGHTNING_TDOA_TIME_MARK.args(args)?;
                let source = a.source.to_ascii_lowercase();
                let sample = u64::try_from(a.sample).map_err(|_| refused(args, "sample"))?;
                let utc_ns = a.utc_ns;
                let sigma = a.sigma_ns.unwrap_or(0.0) * 1e-9;
                let bias = a.bias_ns.unwrap_or(0.0) * 1e-9;
                let use_gps = self.cfg.use_radio_time;
                let Some(st) = &mut self.station else {
                    return Err(refused(args, "source"));
                };
                let mark = match source.as_str() {
                    "gps" if use_gps => st.gps_mark(sample, utc_ns),
                    "gnss" => st.gnss_mark(sample, utc_ns, sigma, bias),
                    _ => return Err(refused(args, "source")),
                };
                st.mark(mark)
                    .map_err(|_| refused(args, "utc_ns"))?;
            }
        }
        Ok(None)
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
        self.position = ctx.station.position;
        self.height_m = ctx.station.height_m.unwrap_or(0.0);
        self.transport.clone_from(&ctx.transport);
        let live_changed = self.live != ctx.live;
        self.live = ctx.live;
        if let Some(st) = &mut self.station {
            st.set_position(self.position, self.height_m);
        }
        if self.configured {
            if live_changed {
                if !self.live {
                    self.start_utc = self.file_start_utc();
                }
                let _ = self.build_station();
            }
            self.net_key.clear();
            self.open_network();
        }
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre_hz = f.centre_hz;
        self.start_utc = self.file_start_utc();
        self.build_station()?;
        self.build_server();
        self.configured = true;
        self.net_key.clear();
        self.open_network();
        self.shown = u64::MAX;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_hz = f.centre_hz;
        let _ = self.build_station();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        let Some(st) = self.station.as_mut() else {
            return;
        };
        st.process(x, first);
        if self.live && !x.is_empty() {
            let utc_ns = (self.clock.now() * 1e9) as i64;
            st.host_mark(first + x.len() as u64 - 1, x.len(), utc_ns);
        }
        let now = self.now_ns();
        self.network_step(out, now);
    }
    fn finish(&mut self, out: &mut Outbox) {
        if let Some(st) = &mut self.station {
            st.flush();
        }
        let now = self.now_ns();
        self.network_step(out, now);
        // The input ended: the detections still waiting are matched now.
        let late = now.saturating_add(3_000_000_000);
        if let (Some(server), Some(st)) = (&mut self.server, &self.station) {
            if server.run(late) {
                let t = st.now_s();
                let centre = self.centre_hz;
                for row in server.take_new() {
                    out.messages(0).push(row.event(t, centre));
                }
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let Some(st) = &self.station else {
            return;
        };
        let now_s = st.now_s();
        let best = st.now_stamp();
        let clocks = st.clocks();
        let timer = st.timer();
        let best_clock = best.map(|b| clocks.clock(b.0));
        let fine = st.fine();
        let per = |s: TimeSource| {
            let c = clocks.clock(s);
            c.usable(now_s)
                .then(|| c.stamp(now_s).map(|x| x.sigma_s * 1e6))
                .flatten()
        };
        let t = st.last_timing().filter(|_| st.timing().is_some());
        let server = self.server.as_ref();
        let sc = server.map(Server::counters);
        let mc = server.map(Server::match_counters);
        p.put(&LightningTdoaFields {
            role: self.cfg.role.key(),
            pulses: count(st.pulses()),
            last_peak_db: st.last_peak_db().map(|d| (d * 10.0).round() / 10.0),
            floor_db: timer.floor().map(|f| 10.0 * f.max(1e-30).log10()),
            time_source: best.map_or("none", |b| b.0.name()),
            clock_sigma_us: best.map(|b| b.1.sigma_s * 1e6),
            clock_ppm: best_clock.and_then(|c| c.ppm()),
            clock_ppm_sigma: best_clock.and_then(|c| c.ppm_sigma()),
            fine,
            sigma_gps_us: per(TimeSource::Gps),
            sigma_gnss_us: per(TimeSource::Gnss),
            sigma_cv_us: per(TimeSource::CommonView),
            sigma_ptp_us: per(TimeSource::Ptp),
            sigma_ntp_us: per(TimeSource::Ntp),
            sigma_system_us: per(TimeSource::System),
            common_view: best.is_some_and(|b| b.0 == TimeSource::CommonView),
            timing_locked: t.map(|r| r.locked),
            timing_snr_db: t.map(|r| (r.snr_db * 10.0).round() / 10.0),
            timing_sigma_us: t.map(|r| r.sigma_s * 1e6),
            timing_carrier_hz: t.map(|r| r.carrier_error_hz),
            network: self.network.as_str(),
            sent: count(self.sent),
            send_errors: count(self.send_errors),
            packets_ok: sc.map(|c| count(c.ok)),
            refused_mac: sc.map(|c| count(c.mac)),
            replays: sc.map(|c| count(c.replays)),
            stale: sc.map(|c| count(c.stale)),
            malformed: sc.map(|c| count(c.malformed)),
            rate_limited: sc.map(|c| count(c.rate_limited)),
            groups: mc.map(|c| count(c.seeds)),
            fixes: mc.map(|c| count(c.fixes)),
            rejected: mc.map(|c| count(c.refused)),
            unmatched: mc.map(|c| count(c.unmatched + c.too_few)),
            coarse_only: mc.map(|c| count(c.coarse_only)),
            too_long: count(timer.too_long()),
            note: NOTE,
        });
        let lock = if !timer.is_valid() {
            LockState::Searching
        } else if fine {
            LockState::Locked
        } else {
            LockState::Tracking
        };
        let pulses = st.pulses();
        let source = best.map_or("no clock", |b| b.0.name());
        let sigma_us = best.map_or(0.0, |b| b.1.sigma_s * 1e6);
        let role = self.cfg.role.key();
        let fixes = mc.map(|c| c.fixes);
        let stations = server.map_or(0, |s| s.stations().len());
        self.write_server(p, now_s);
        family::status(&mut self.status, lock, None, None, None, |s| {
            if lock == LockState::Searching {
                let _ = write!(s, "Lightning ({role}): learning the noise floor");
                return;
            }
            let _ = write!(s, "Lightning ({role}): {pulses} sferics, {source}");
            if sigma_us > 0.0 {
                let _ = write!(s, " ±{sigma_us:.1} µs");
            }
            if let Some(f) = fixes {
                let _ = write!(s, "; {f} fixes from {stations} stations");
            }
        });
        p.results_mut().set_status(&self.status);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        if let Some(st) = &mut self.station {
            st.clear();
        }
        if let Some(s) = &mut self.server {
            s.clear();
        }
        self.sent = 0;
        self.send_errors = 0;
        self.cleared = true;
        self.shown = u64::MAX;
    }
}

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("tdoa", DecoderComponent::new(TdoaDecoder::default()))
        .input("tdoa")
        .output("tdoa.events")
        .status_from("tdoa")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE,
            "Lightning position needs a sample rate of at least 100 ksps",
        )
;
    for s in DecoderSetting::schema().settings() {
        let k = &s.field.key;
        g = g.route(k, &[&format!("tdoa.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command(RESET.key, CommandTarget::of("tdoa", RESET.key))
        .command("datagram", CommandTarget::of("tdoa", "datagram"))
        .command("time_mark", CommandTarget::of("tdoa", "time_mark"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("lightning.tdoa", || {
        Box::new(DecoderComponent::new(TdoaDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal with the timing transmitter", o, 0.0)
        .with(&GEN_TIMING_TX, true)
        .with(&TIMING_HZ, TEST_DIAL_HZ + GEN_TIMING_OFFSET_HZ.default)
        .with(&TIMING_LAT, gen::TIMING_TX.0)
        .with(&TIMING_LON, gen::TIMING_TX.1);
    c.seconds = o.seconds.max(8.0);
    c.expect_audio = Some(false);
    if !o.clean {
        c.snr_db = Some(30.0);
    }
    c.check = Some(|r| {
        let n = family::field(r, LightningTdoaFields::PULSES.key);
        let cv = family::field(r, LightningTdoaFields::COMMON_VIEW.key);
        vec![
            CheckLine::new(LightningTdoaFields::PULSES.key, n >= 2.0, format!("{n}, at least 2 expected")),
            CheckLine::new(
                LightningTdoaFields::COMMON_VIEW.key,
                cv > 0.0,
                "the common-view clock stamps the detections",
            ),
        ]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// Lightning position (network).
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Lightning position (network)",
    group: "Science, time and utility",
    blurb: "Each station times sferics to the microsecond on a disciplined sample clock; one station, the server, collects \
            the detections of all and locates each strike by time difference of arrival, with a 95 % error ellipse.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 3e3,
        max_hz: 30e6,
        default_hz: TEST_DIAL_HZ,
        rate_hz: ASK_RATE,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.5e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: true,
        radio_filter_hz: Some(0.5e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: TdoaSetting::schema,
    make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(make_generator),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
