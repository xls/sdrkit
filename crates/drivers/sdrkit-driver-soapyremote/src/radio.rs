//! One open radio behind a SoapyRemote server: the open sequence, the configuration (at open and live), the set-up of the
//! receive stream (UDP or TCP) and the close sequence. The runner of the driver core drives it.
//!
//! Standard: the SoapyRemote protocol, RPC version 0x0400 (`rpc::VERSION`).

use crate::format::{choose_format, cs16_shift, Format, WireChoice};
use crate::log::{LogConn, LogSetup};
use crate::plan::{self, Control, Element, GainPlan, RunTime};
use crate::rpc::{
    self, arg, call, optional, Conn, Kwargs, Packer, Range, RpcError, Unpacker, RX, SERIAL,
};
use crate::stream::{Flow, RxShared, RxSockets, RxStream, StreamStats};
use crate::wire::{self, Wire, PROT_KEY};
use crate::{
    lock, Mtu, SoapyError, SoapyRemoteSetting, StreamProt, Timing, ANTENNA, CLOCK_SOURCE, FORMAT,
    MTU, PPM, REMOTE, SERVER_LOG, TIME_SOURCE, TRANSPORT, WINDOW,
};
use crate::{
    write_items, ARRIVING, DATAGRAM, DRIVER, HARDWARE, HARDWARE_INFO, LATE, LOG_DROPPED,
    LOST_DATAGRAMS, MALFORMED, MBIT_S, NATIVE_FORMAT, NATIVE_SCALE, OVERFLOWS, PER_DATAGRAM,
    RECONNECTS, RPC_VERSION, RX_CHANNELS, SENSORS, SERVER_ID, STREAM_ERRORS, TX_CHANNELS,
    WINDOW_BYTES, WINDOW_PACKETS, WIRE_FORMAT,
};
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use sdrkit_core::net::{DatagramOptions, NetError, Transport};
use sdrkit_core::Results;
use sdrkit_core::{Hz, SettingDecl, SettingSet, Value, Values};
use sdrkit_driver_core::linkwatch::LinkWatch;
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::runner::{Notices, Radio};
use sdrkit_driver_core::{
    own_id, Capabilities, DriverError, RadioSetting, TxLimits, AGC, BANDWIDTH, BIAS_TEE, CENTRE,
    GAIN, RATE, RATE_EXACT,
};
use sdrkit_graph::stream::SampleType;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The smallest message size of a TCP stream: the size the protocol's own client uses.
pub(crate) const TCP_MTU: usize = 4096;

/// The message size of a stream with `mtu` over `prot`.
pub(crate) fn stream_mtu(mtu: usize, prot: StreamProt) -> usize {
    match prot {
        StreamProt::Udp => mtu,
        StreamProt::Tcp => mtu.max(TCP_MTU),
    }
}

/// The rate requested when the setting specifies none.
pub(crate) const DEFAULT_RATE: Hz = 2.048e6;

/// What the radio told the driver at open.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Facts {
    /// Driver key (call 100).
    pub driver_key: String,
    /// Hardware key (call 101).
    pub hardware_key: String,
    /// Hardware info (call 102).
    pub hardware_info: Kwargs,
    /// Receive channels (call 202).
    pub rx_channels: i32,
    /// Transmit channels (call 202, direction 0).
    pub tx_channels: i32,
    /// The limits of transmit channel 0: its frequency, rate, filter and overall gain ranges (calls 502, 907 or 902,
    /// 1004 and 707, direction 0). `None` without a transmitter.
    pub tx: Option<TxLimits>,
    /// Native format and full scale (call 305).
    pub native: Option<(String, f64)>,
    /// Stream formats (call 304). `None` when not offered.
    pub formats: Option<Vec<String>>,
    /// Rate ranges (call 907, or 902).
    pub rates: Vec<Range>,
    /// Frequency ranges (call 805).
    pub freqs: Vec<Range>,
    /// Overall gain range (call 707).
    pub gain: Option<Range>,
    /// Gain elements with their ranges (calls 700, 708).
    pub elements: Vec<Element>,
    /// Has automatic gain mode (call 709).
    pub has_gain_mode: bool,
    /// Has frequency correction (call 503).
    pub has_correction: bool,
    /// Antennas (call 500).
    pub antennas: Vec<String>,
    /// The bias-tee setting key of the radio, and its value at open.
    pub bias: Option<(String, bool)>,
    /// Clock sources (call 1002).
    pub clocks: Vec<String>,
    /// The current clock source (call 1004).
    pub clock: Option<String>,
    /// Time sources (call 1005).
    pub times: Vec<String>,
    /// The current time source (call 1007).
    pub time: Option<String>,
    /// Bandwidth ranges (call 906, or 905).
    pub bandwidths: Vec<Range>,
    /// Sensors (call 1200) with their values at open (call 1201).
    pub sensors: Vec<(String, String)>,
    /// The serial, from the hardware info or the arguments.
    pub serial: String,
    /// The settings of the radio's driver (calls 1402, 1401), then the gain elements as settings of their own.
    pub run_time: Vec<RunTime>,
}

impl Facts {
    /// The range of the gain setting.
    pub fn gain_range(&self) -> Option<Range> {
        self.gain.map(|g| plan::overall_range(g, &self.elements))
    }
    /// The RF stage (RFGR, IFGR) of a gain-reduction radio.
    pub fn rf_stage(&self) -> Option<(Range, Range)> {
        plan::rf_stage(&self.elements).map(|(r, i)| (r.range, i.range))
    }
    /// The capabilities these facts give.
    pub fn capabilities(&self, label: &str) -> Capabilities {
        let (lo, hi) = plan::span(&self.freqs).unwrap_or((0.0, 0.0));
        let name = if self.hardware_key.is_empty() && self.driver_key.is_empty() {
            format!("SoapyRemote {label}")
        } else if self.hardware_key.is_empty() || self.hardware_key == self.driver_key {
            format!("SoapyRemote {label}: {}", self.driver_key)
        } else {
            format!(
                "SoapyRemote {label}: {} ({})",
                self.driver_key, self.hardware_key
            )
        };
        Capabilities {
            name,
            rates: plan::rate_limits(&self.rates),
            min_freq_hz: lo,
            max_freq_hz: hi,
            gain_db: self.gain_range().map(|r| (r.min, r.max, r.step)),
            bandwidth_hz: plan::span(&self.bandwidths),
            has_agc: self.has_gain_mode,
            transmit: self.tx.clone(),
            has_bias_tee: self.bias.is_some(),
            realtime: true,
            common_rates_hz: plan::points(&self.rates),
            common_bandwidths_hz: plan::points(&self.bandwidths),
            gain_steps_db: Vec::new(),
        }
    }

    /// The id and the value of the RF gain state setting, for a radio that has one.
    pub fn rf_state(&self, values: &Values) -> Option<(sdrkit_core::SettingId, i64)> {
        let i = self
            .run_time
            .iter()
            .position(|r| r.control == Control::RfState)?;
        let id = crate::DESCRIPTOR.run_time_id(i);
        match values.value(id) {
            Some(Value::I64(v)) => Some((id, *v)),
            _ => Some((id, -1)),
        }
    }

    /// Checks that the radio accepts the values of the settings that are known only once it is open.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] with the setting the radio cannot take.
    pub fn check(&self, values: &Values) -> Result<(), DriverError> {
        let refused = |s: SoapyRemoteSetting| Err(DriverError::Refused { id: own_id(s) });
        if let Some((id, rf)) = self.rf_state(values) {
            if rf >= 0 && !self.rf_stage().is_some_and(|(r, _)| in_range(r, rf as f64)) {
                return Err(DriverError::Refused { id });
            }
        }
        let listed =
            |want: &str, list: &[String]| want.is_empty() || list.iter().any(|a| a == want);
        if !listed(values.get(&ANTENNA), &self.antennas) {
            return refused(SoapyRemoteSetting::Antenna);
        }
        if !listed(values.get(&CLOCK_SOURCE), &self.clocks) {
            return refused(SoapyRemoteSetting::ClockSource);
        }
        if !listed(values.get(&TIME_SOURCE), &self.times) {
            return refused(SoapyRemoteSetting::TimeSource);
        }
        if values.get(&BIAS_TEE) && self.bias.is_none() {
            return Err(DriverError::Refused {
                id: RadioSetting::BiasTee.id(),
            });
        }
        if values.get(&RATE_EXACT) && !plan::choose_rate(&self.rates, asked_rate(values)).exact {
            return Err(DriverError::Refused {
                id: RadioSetting::Rate.id(),
            });
        }
        Ok(())
    }
}

fn in_range(r: Range, v: f64) -> bool {
    v >= r.min.min(r.max) && v <= r.max.max(r.min)
}

/// The rate requested by `values`.
pub(crate) fn asked_rate(values: &Values) -> Hz {
    match values.get(&RATE) {
        r if r > 0.0 => r,
        _ => DEFAULT_RATE,
    }
}

/// The settings that take effect when the radio opens. A change of one of them needs a restart.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Options {
    /// The Remote driver arguments.
    pub remote: Kwargs,
    /// The Remote driver text was empty.
    pub remote_empty: bool,
    /// The IQ format.
    pub format: WireChoice,
    /// The MTU.
    pub mtu: Mtu,
    /// The requested receive buffer size, in bytes.
    pub window: usize,
    /// The transport of the stream.
    pub transport: StreamProt,
    /// Forward the server's log.
    pub serverlog: bool,
    /// The antenna ("": the radio's default).
    pub antenna: String,
    /// The clock source ("": as the radio is).
    pub clock: String,
    /// The time source ("": as the radio is).
    pub time: String,
}

impl Options {
    /// The size of a stream message: the MTU, and at least [`TCP_MTU`] on TCP.
    pub fn message_size(&self) -> usize {
        stream_mtu(self.mtu.bytes(), self.transport)
    }

    /// Reads the settings that take effect at open.
    pub fn from_values(values: &Values) -> Options {
        let remote = values.get(&REMOTE);
        Options {
            remote: plan::parse_remote(remote),
            remote_empty: remote.trim().is_empty(),
            format: values.get(&FORMAT),
            mtu: values.get(&MTU),
            window: (values.get(&WINDOW) * 1_048_576.0).round() as usize,
            transport: values.get(&TRANSPORT),
            serverlog: values.get(&SERVER_LOG),
            antenna: values.get(&ANTENNA).to_owned(),
            clock: values.get(&CLOCK_SOURCE).to_owned(),
            time: values.get(&TIME_SOURCE).to_owned(),
        }
    }
}

/// What the readings of an open radio show.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ServerInfo {
    /// The server id (call 20).
    pub server_id: String,
    /// The RPC version of its replies.
    pub version: u32,
    /// What the radio told at open.
    pub facts: Facts,
    /// The format on the network.
    pub format: Option<Format>,
    /// The MTU.
    pub mtu: usize,
    /// The requested receive buffer size, in bytes.
    pub window: usize,
    /// The window in packets.
    pub window_packets: u32,
    /// Elements per full datagram.
    pub elements_per_datagram: usize,
}

/// Sensor names with their values, and when they were read.
pub(crate) type Sensors = (Vec<(String, String)>, Option<Instant>);

/// What one open radio shares across its connections (the first open and every reconnect).
pub(crate) struct Ctx {
    /// The network.
    pub transport: Arc<dyn Transport>,
    /// Server host.
    pub host: String,
    /// Server port.
    pub port: u16,
    /// `host:port`.
    pub label: String,
    /// Timings.
    pub timing: Timing,
    /// Stream counters.
    pub stats: Arc<StreamStats>,
    /// Throughput.
    pub flow: Arc<Flow>,
    /// The server information of the current connection.
    pub info: Mutex<ServerInfo>,
    /// The current control connection (sensor reads use it).
    pub ctl: Mutex<Option<Arc<Mutex<Conn>>>>,
    /// The sensors' values and when they were read.
    pub sensors: Mutex<Sensors>,
    /// Opens so far.
    pub opens: AtomicU64,
    /// A stream delivered once (later stream set-ups are reconnects, which the runner retries).
    pub streamed: AtomicBool,
    /// The Remote driver arguments of the last open (the transmitter opens the same radio).
    pub remote: Mutex<Kwargs>,
    /// The format of the first open. A reconnect must give the same one.
    pub format: Mutex<Option<Format>>,
    /// The stream transport of the last open (the transmitter streams the same way).
    pub tcp: AtomicBool,
    /// The radio was set up once: a frequency outside what a server holds is refused from the second set-up on.
    pub set_up: AtomicBool,
    /// The live reading of the link of the stream.
    pub link: Arc<LinkWatch>,
}

impl Ctx {
    /// A context for the server `host:port`.
    pub fn new(transport: Arc<dyn Transport>, host: &str, port: u16, timing: Timing) -> Ctx {
        Ctx {
            transport,
            host: host.to_owned(),
            port,
            label: sdrkit_core::net::show(host, port),
            timing,
            stats: Arc::default(),
            flow: Arc::default(),
            info: Mutex::default(),
            ctl: Mutex::new(None),
            sensors: Mutex::new((Vec::new(), None)),
            opens: AtomicU64::new(0),
            streamed: AtomicBool::new(false),
            remote: Mutex::default(),
            format: Mutex::new(None),
            tcp: AtomicBool::new(false),
            set_up: AtomicBool::new(false),
            link: Arc::default(),
        }
    }
}

/// Connects to the server: one more try after a silent timeout.
///
/// # Errors
///
/// [`DriverError::Net`] if no connection is made.
pub(crate) fn connect(ctx: &Ctx, abort: &Abort) -> Result<Conn, DriverError> {
    let timeout = ctx.timing.connect_timeout(&ctx.host);
    let mut tries = 0;
    loop {
        tries += 1;
        match net::connect(&ctx.transport, &ctx.host, ctx.port, false, timeout, abort) {
            Ok(s) => return Ok(Conn::new(s)),
            Err(NetError::TimedOut) if tries == 1 && !abort.fired() => {}
            Err(e) => return Err(DriverError::Net(e)),
        }
    }
}

/// Sends hang up and closes, waiting at most `wait` for the reply.
pub(crate) fn hang_up(conn: &mut Conn, wait: Duration) {
    if !conn.is_broken() {
        let _ = conn.call_void(&Packer::call(call::HANGUP), wait);
    }
    conn.shutdown();
}

/// The start of call 300 for direction `dir`, format `format`, channel `ch`: everything before the two port strings.
pub(crate) fn setup_call(
    dir: i8,
    format: &str,
    ch: i32,
    mtu: usize,
    window_bytes: usize,
    prot: StreamProt,
) -> Packer {
    let mut args = Kwargs::new();
    args.insert("remote:mtu".into(), mtu.to_string());
    args.insert(PROT_KEY.into(), prot.name().into());
    args.insert("remote:window".into(), window_bytes.to_string());
    let mut p = Packer::call(call::SETUP_STREAM);
    p.char(dir).string(format).size_list(&[ch]).kwargs(&args);
    p
}

/// Makes the radio `remote` on `conn` (call 1).
///
/// # Errors
///
/// [`SoapyError::NoRadio`] or [`SoapyError::NoMatch`] if the server has no such radio, [`DriverError::Busy`] if another
/// program holds it, or the error of the call.
pub(crate) fn make(
    conn: &mut Conn,
    remote: &Kwargs,
    remote_empty: bool,
    wait: Duration,
) -> Result<(), DriverError> {
    let mut p = Packer::call(call::MAKE);
    p.kwargs(&plan::with_no_deeper(remote));
    match conn.call_void(&p, wait) {
        Ok(()) => Ok(()),
        Err(RpcError::Exception(text)) if text.to_lowercase().contains("no match") => {
            Err(DriverError::driver(if remote_empty {
                SoapyError::NoRadio
            } else {
                SoapyError::NoMatch
            }))
        }
        Err(RpcError::Exception(text)) if plan::is_busy(&text) => Err(DriverError::Busy),
        Err(e) => Err(e.driver_error(call::MAKE)),
    }
}

fn dir_ch(n: u32, dir: i8) -> Packer {
    let mut p = Packer::call(n);
    p.dir_ch(dir, 0);
    p
}

/// A frequency that the server reads back this far from the one sent is held by the server, Hz. A tuner's own steps
/// are smaller.
const HELD_TOLERANCE_HZ: f64 = 1e3;
/// The part of the capture around its centre that a session can tune to: the edges are the radio's filter.
const USABLE: f64 = 0.9;

/// The local address of the same family as `server`, any port.
pub(crate) fn any_local(server: IpAddr) -> SocketAddr {
    match server {
        IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    }
}

/// What was last sent, so a live change sends only what changed.
#[derive(Clone, Debug, Default)]
struct Sent {
    asked_rate: Option<Hz>,
    bw_wanted: Option<Hz>,
    ppm: f64,
    radio_corrects: bool,
    freq: Option<Hz>,
    gain_mode: Option<bool>,
    gain: Option<GainPlan>,
    bias: bool,
    bias_on: bool,
    /// The centre that the server read back after the last frequency, and whether it holds the radio elsewhere.
    held: Option<(Hz, bool)>,
    /// The value last sent of each run-time setting, by its index in [`Facts::run_time`]. It starts with the values at open.
    run_time: Vec<Value>,
}

/// One connection to a radio behind a SoapyRemote server.
pub(crate) struct SoapyRadio {
    ctl: Arc<Mutex<Conn>>,
    ctx: Arc<Ctx>,
    /// What the radio told at open.
    pub facts: Facts,
    opts: Options,
    format: Format,
    shift: u32,
    server_ip: IpAddr,
    log: Option<LogConn>,
    notices: Notices,
    stream_id: Option<i32>,
    stream_abort: Option<Arc<Abort>>,
    sent: Sent,
    rate: Hz,
    centre: Hz,
    closed: bool,
}

/// Opens the radio: connect, the server id, make, and what the radio is. The runner then configures it.
///
/// # Errors
///
/// [`DriverError`] if the server cannot be reached, has no such radio, or refuses.
pub(crate) fn open(
    ctx: &Arc<Ctx>,
    values: &Values,
    abort: &Arc<Abort>,
    notices: &Notices,
) -> Result<SoapyRadio, DriverError> {
    let opts = Options::from_values(values);
    let server_ip = ctx.transport.resolve(&ctx.host, ctx.port)?.ip();
    let mut conn = connect(ctx, abort)?;
    let log = opts.serverlog.then(|| {
        LogConn::start(LogSetup {
            transport: ctx.transport.clone(),
            host: ctx.host.clone(),
            port: ctx.port,
            label: ctx.label.clone(),
            timing: ctx.timing.clone(),
            notices: notices.clone(),
            stats: ctx.stats.clone(),
        })
    });
    let t = ctx.timing.clone();
    // The server id. A server that does not know the call answers with an EXCEPTION.
    let server_id = match conn.call(&Packer::call(call::SERVER_ID), t.call, |u| u.string()) {
        Ok(id) => id,
        Err(RpcError::Exception(_)) => String::new(),
        Err(e) => {
            conn.shutdown();
            return Err(e.driver_error(call::SERVER_ID));
        }
    };
    let version = conn.peer_version.unwrap_or(rpc::VERSION);
    if let Err(e) = make(&mut conn, &opts.remote, opts.remote_empty, t.make) {
        hang_up(&mut conn, t.close_call);
        return Err(e);
    }
    let mut radio = SoapyRadio {
        ctl: Arc::new(Mutex::new(conn)),
        ctx: ctx.clone(),
        facts: Facts::default(),
        opts,
        format: Format::Cs16,
        shift: 0,
        server_ip,
        log,
        notices: notices.clone(),
        stream_id: None,
        stream_abort: None,
        sent: Sent::default(),
        rate: 0.0,
        centre: values.get(&CENTRE),
        closed: false,
    };
    if let Err(e) = radio.read_radio().and_then(|()| radio.choose_format()) {
        radio.close();
        return Err(e);
    }
    *lock(&ctx.info) = ServerInfo {
        server_id,
        version,
        facts: radio.facts.clone(),
        format: Some(radio.format),
        mtu: radio.opts.message_size(),
        window: radio.opts.window,
        ..ServerInfo::default()
    };
    *lock(&ctx.sensors) = (radio.facts.sensors.clone(), Some(Instant::now()));
    *lock(&ctx.ctl) = Some(radio.ctl.clone());
    lock(&ctx.remote).clone_from(&radio.opts.remote);
    ctx.tcp
        .store(radio.opts.transport == StreamProt::Tcp, Ordering::Relaxed);
    if ctx.opens.fetch_add(1, Ordering::Relaxed) > 0 {
        ctx.stats.reconnects.fetch_add(1, Ordering::Relaxed);
    }
    Ok(radio)
}

impl SoapyRadio {
    fn conn(&self) -> MutexGuard<'_, Conn> {
        lock(&self.ctl)
    }

    fn timing(&self) -> &Timing {
        &self.ctx.timing
    }

    /// A call. A broken connection also ends the stream (the runner then reconnects).
    fn call<T>(
        &self,
        p: &Packer,
        timeout: Duration,
        f: impl FnOnce(&mut Unpacker) -> Result<T, rpc::Unexpected>,
    ) -> Result<T, RpcError> {
        let r = self.conn().call(p, timeout, f);
        if let Err(e) = &r {
            if !matches!(e, RpcError::Exception(_)) {
                if let Some(a) = &self.stream_abort {
                    a.fire();
                }
            }
        }
        r
    }

    fn void(&self, p: &Packer) -> Result<(), RpcError> {
        self.call(p, self.timing().call, |u| u.void())
    }

    /// A call that the radio can refuse: its refusal is [`DriverError::Refused`] of `id`.
    fn set(&self, n: u32, p: &Packer, id: sdrkit_core::SettingId) -> Result<(), DriverError> {
        match self.void(p) {
            Ok(()) => Ok(()),
            Err(RpcError::Exception(_)) => Err(DriverError::Refused { id }),
            Err(e) => Err(e.driver_error(n)),
        }
    }

    /// An optional query. A broken connection is an error.
    fn opt<T>(
        &self,
        n: u32,
        p: &Packer,
        f: impl FnOnce(&mut Unpacker) -> Result<T, rpc::Unexpected>,
    ) -> Result<Option<T>, DriverError> {
        optional(self.call(p, self.timing().call, f)).map_err(|e| e.driver_error(n))
    }

    /// Reads what the radio is.
    fn read_radio(&mut self) -> Result<(), DriverError> {
        let mut f = Facts::default();
        let none = Packer::call;
        f.driver_key = self
            .opt(call::DRIVER_KEY, &none(call::DRIVER_KEY), |u| u.string())?
            .unwrap_or_default();
        f.hardware_key = self
            .opt(call::HARDWARE_KEY, &none(call::HARDWARE_KEY), |u| {
                u.string()
            })?
            .unwrap_or_default();
        f.hardware_info = self
            .opt(call::HARDWARE_INFO, &none(call::HARDWARE_INFO), |u| {
                u.kwargs()
            })?
            .unwrap_or_default();
        let mut p = Packer::call(call::NUM_CHANNELS);
        p.char(RX);
        f.rx_channels = self.opt(call::NUM_CHANNELS, &p, |u| u.i32())?.unwrap_or(1);
        if f.rx_channels <= 0 {
            return Err(DriverError::driver(SoapyError::NoReceiver));
        }
        f.native = self.opt(call::NATIVE_FORMAT, &dir_ch(call::NATIVE_FORMAT, RX), |u| {
            Ok((u.string()?, u.f64()?))
        })?;
        f.formats = self.opt(
            call::STREAM_FORMATS,
            &dir_ch(call::STREAM_FORMATS, RX),
            |u| u.string_list(),
        )?;
        f.rates = self.rate_table(RX)?;
        f.freqs = self
            .opt(
                call::FREQUENCY_RANGE,
                &dir_ch(call::FREQUENCY_RANGE, RX),
                |u| u.range_list(),
            )?
            .unwrap_or_default();
        f.gain = self.opt(call::GAIN_RANGE, &dir_ch(call::GAIN_RANGE, RX), |u| {
            u.range()
        })?;
        let names = self
            .opt(call::LIST_GAINS, &dir_ch(call::LIST_GAINS, RX), |u| {
                u.string_list()
            })?
            .unwrap_or_default();
        for name in names {
            let mut p = dir_ch(call::GAIN_RANGE_ELEMENT, RX);
            p.string(&name);
            let range = self
                .opt(call::GAIN_RANGE_ELEMENT, &p, |u| u.range())?
                .unwrap_or_default();
            f.elements.push(Element { name, range });
        }
        // An empty overall range with no element that spans more is a radio without gain: no gain is sent.
        let spans = |r: &Range| r.max > r.min;
        if f.gain.as_ref().is_some_and(|g| !spans(g)) && !f.elements.iter().any(|e| spans(&e.range))
        {
            f.gain = None;
        }
        let mut stages = Vec::new();
        for e in plan::stages(&f.elements) {
            let mut p = dir_ch(call::GET_GAIN_ELEMENT, RX);
            p.string(&e.name);
            let at_open = self.opt(call::GET_GAIN_ELEMENT, &p, |u| u.f64())?;
            stages.push(plan::stage_setting(e, at_open));
        }
        f.has_gain_mode = self
            .opt(call::HAS_GAIN_MODE, &dir_ch(call::HAS_GAIN_MODE, RX), |u| {
                u.bool()
            })?
            .unwrap_or(false);
        f.has_correction = self
            .opt(
                call::HAS_FREQ_CORRECTION,
                &dir_ch(call::HAS_FREQ_CORRECTION, RX),
                |u| u.bool(),
            )?
            .unwrap_or(false);
        f.antennas = self
            .opt(call::LIST_ANTENNAS, &dir_ch(call::LIST_ANTENNAS, RX), |u| {
                u.string_list()
            })?
            .unwrap_or_default();
        let infos = self
            .opt(call::SETTING_INFO, &none(call::SETTING_INFO), |u| {
                u.arg_info_list()
            })?
            .unwrap_or_default();
        for info in infos.iter().filter(|i| !i.key.is_empty()) {
            let mut p = Packer::call(call::READ_SETTING);
            p.string(&info.key);
            let value = self.opt(call::READ_SETTING, &p, |u| u.string())?;
            if plan::is_bias_key(&info.key) {
                if f.bias.is_none() {
                    let v = value
                        .as_deref()
                        .filter(|v| !v.is_empty())
                        .unwrap_or(&info.value);
                    f.bias = Some((info.key.clone(), plan::truthy(v)));
                }
            } else {
                f.run_time
                    .extend(plan::module_setting(info, value.as_deref()));
            }
        }
        f.run_time.extend(stages);
        if let Some((rf, _)) = plan::rf_stage(&f.elements) {
            f.run_time.push(plan::rf_state_setting(rf.range));
        }
        if let Some(c) = self.opt(
            call::LIST_CLOCK_SOURCES,
            &none(call::LIST_CLOCK_SOURCES),
            |u| u.string_list(),
        )? {
            f.clocks = c;
            f.clock = self.opt(call::GET_CLOCK_SOURCE, &none(call::GET_CLOCK_SOURCE), |u| {
                u.string()
            })?;
        }
        if let Some(c) = self.opt(
            call::LIST_TIME_SOURCES,
            &none(call::LIST_TIME_SOURCES),
            |u| u.string_list(),
        )? {
            f.times = c;
            f.time = self.opt(call::GET_TIME_SOURCE, &none(call::GET_TIME_SOURCE), |u| {
                u.string()
            })?;
        }
        f.bandwidths = match self.opt(
            call::BANDWIDTH_RANGE,
            &dir_ch(call::BANDWIDTH_RANGE, RX),
            |u| u.range_list(),
        )? {
            Some(r) => r,
            None => self
                .opt(
                    call::LIST_BANDWIDTHS,
                    &dir_ch(call::LIST_BANDWIDTHS, RX),
                    |u| u.f64_list(),
                )?
                .unwrap_or_default()
                .into_iter()
                .map(|b| Range::new(b, b, 0.0))
                .collect(),
        };
        let sensors = self
            .opt(call::LIST_SENSORS, &none(call::LIST_SENSORS), |u| {
                u.string_list()
            })?
            .unwrap_or_default();
        for s in sensors {
            let mut p = Packer::call(call::READ_SENSOR);
            p.string(&s);
            let v = self
                .opt(call::READ_SENSOR, &p, |u| u.string())?
                .unwrap_or_default();
            f.sensors.push((s, v));
        }
        let mut p = Packer::call(call::NUM_CHANNELS);
        p.char(rpc::TX);
        f.tx_channels = self
            .opt(call::NUM_CHANNELS, &p, |u| u.i32())?
            .unwrap_or(0)
            .max(0);
        if f.tx_channels > 0 {
            let gain = self.opt(call::GAIN_RANGE, &dir_ch(call::GAIN_RANGE, rpc::TX), |u| {
                u.range()
            })?;
            let freqs = self
                .opt(
                    call::FREQUENCY_RANGE,
                    &dir_ch(call::FREQUENCY_RANGE, rpc::TX),
                    |u| u.range_list(),
                )?
                .unwrap_or_default();
            let rates = self.rate_table(rpc::TX)?;
            let widths = self
                .opt(
                    call::BANDWIDTH_RANGE,
                    &dir_ch(call::BANDWIDTH_RANGE, rpc::TX),
                    |u| u.range_list(),
                )?
                .unwrap_or_default();
            f.tx = Some(crate::tx::limits(&freqs, &rates, &widths, gain));
        }
        f.serial = arg(&f.hardware_info, SERIAL)
            .or_else(|| arg(&self.opts.remote, SERIAL))
            .unwrap_or_default()
            .to_owned();
        self.sent.bias = f.bias.as_ref().is_some_and(|b| b.1);
        self.sent.radio_corrects = true;
        self.facts = f;
        // The antenna takes effect at open.
        if !self.opts.antenna.is_empty() {
            if !self.facts.antennas.contains(&self.opts.antenna) {
                return Err(DriverError::Refused {
                    id: own_id(SoapyRemoteSetting::Antenna),
                });
            }
            let mut p = dir_ch(call::SET_ANTENNA, RX);
            p.string(&self.opts.antenna);
            self.set(call::SET_ANTENNA, &p, own_id(SoapyRemoteSetting::Antenna))?;
        }
        Ok(())
    }

    /// Chooses the format on the network. A reconnect must give the format of the first open.
    fn choose_format(&mut self) -> Result<(), DriverError> {
        let native = self.facts.native.clone();
        let (fmt, note) = choose_format(
            self.opts.format,
            native.as_ref().map(|n| n.0.as_str()),
            self.facts.formats.as_deref(),
        );
        let first = *lock(&self.ctx.format).get_or_insert(fmt);
        if first != fmt {
            return Err(DriverError::driver(SoapyError::FormatChanged {
                was: first.sample_type(),
                now: fmt.sample_type(),
            }));
        }
        if let Some(n) = note {
            self.notices.warn(n);
        }
        self.format = fmt;
        self.shift = cs16_shift(
            fmt,
            native.as_ref().map(|n| n.0.as_str()),
            native.as_ref().map_or(0.0, |n| n.1),
        );
        Ok(())
    }

    /// The rate table of a direction: call 907, or 902 as single rates.
    fn rate_table(&self, dir: i8) -> Result<Vec<Range>, DriverError> {
        if let Some(r) = self.opt(
            call::SAMPLE_RATE_RANGE,
            &dir_ch(call::SAMPLE_RATE_RANGE, dir),
            |u| u.range_list(),
        )? {
            return Ok(r);
        }
        Ok(self
            .opt(
                call::LIST_SAMPLE_RATES,
                &dir_ch(call::LIST_SAMPLE_RATES, dir),
                |u| u.f64_list(),
            )?
            .unwrap_or_default()
            .into_iter()
            .map(|r| Range::new(r, r, 0.0))
            .collect())
    }

    fn deactivate(&self, id: i32, timeout: Duration) -> Result<i32, RpcError> {
        let mut p = Packer::call(call::DEACTIVATE_STREAM);
        p.i32(id).i32(0).i64(0);
        self.call(&p, timeout, |u| u.i32())
    }

    fn activate(&self, id: i32) -> Result<(), DriverError> {
        let mut p = Packer::call(call::ACTIVATE_STREAM);
        p.i32(id).i32(0).i64(0).i32(0);
        match self.call(&p, self.timing().call, |u| u.i32()) {
            Ok(0) => Ok(()),
            Ok(code) => Err(DriverError::Device { code }),
            Err(e) => Err(e.driver_error(call::ACTIVATE_STREAM)),
        }
    }

    /// The clock and time sources, at open only.
    fn sources(&mut self) -> Result<(), DriverError> {
        for (want, cur, n, id) in [
            (
                self.opts.clock.clone(),
                self.facts.clock.clone(),
                call::SET_CLOCK_SOURCE,
                SoapyRemoteSetting::ClockSource,
            ),
            (
                self.opts.time.clone(),
                self.facts.time.clone(),
                call::SET_TIME_SOURCE,
                SoapyRemoteSetting::TimeSource,
            ),
        ] {
            if want.is_empty() || cur.as_deref() == Some(want.as_str()) {
                continue;
            }
            let mut p = Packer::call(n);
            p.string(&want);
            self.set(n, &p, own_id(id))?;
            if n == call::SET_CLOCK_SOURCE {
                self.facts.clock = Some(want);
            } else {
                self.facts.time = Some(want);
            }
        }
        Ok(())
    }

    /// The rate and the filter.
    fn rate_and_filter(
        &mut self,
        values: &Values,
        asked: Hz,
        rate_change: bool,
        notes: &mut Vec<String>,
    ) -> Result<(), DriverError> {
        let rate_id = RadioSetting::Rate.id();
        if rate_change {
            let choice = plan::choose_rate(&self.facts.rates, asked);
            let mut p = dir_ch(call::SET_SAMPLE_RATE, RX);
            p.f64(choice.rate);
            self.set(call::SET_SAMPLE_RATE, &p, rate_id)?;
            self.sent.asked_rate = Some(asked);
            self.ctx.flow.set_asked(asked);
            let back = optional(self.call(
                &dir_ch(call::GET_SAMPLE_RATE, RX),
                self.timing().call,
                |u| u.f64(),
            ))
            .map_err(|e| e.driver_error(call::GET_SAMPLE_RATE))?
            .filter(|r| *r > 0.0)
            .unwrap_or(choice.rate);
            self.rate = back;
            self.ctx.flow.set_rate(back);
            self.ctx.link.restarted();
            if back < 0.97 * asked {
                notes.extend(choice.note);
            }
        }
        if !self.facts.bandwidths.is_empty() {
            let asked_bw = values.get(&BANDWIDTH);
            let wanted = if asked_bw > 0.0 {
                asked_bw
            } else {
                0.95 * self.rate
            };
            if rate_change || self.sent.bw_wanted.is_none_or(|b| (b - wanted).abs() > 0.5) {
                self.sent.bw_wanted = Some(wanted);
                if let Some(bw) = plan::choose_bandwidth(&self.facts.bandwidths, wanted) {
                    let mut p = dir_ch(call::SET_BANDWIDTH, RX);
                    p.f64(bw);
                    match self.void(&p) {
                        Ok(()) => {}
                        Err(RpcError::Exception(_)) if asked_bw > 0.0 => {
                            return Err(DriverError::Refused {
                                id: RadioSetting::Bandwidth.id(),
                            })
                        }
                        Err(RpcError::Exception(_)) => {}
                        Err(e) => return Err(e.driver_error(call::SET_BANDWIDTH)),
                    }
                }
            }
        }
        Ok(())
    }

    /// The frequency correction, the frequency, the gain and the bias tee.
    fn rest(&mut self, values: &Values, notes: &mut Vec<String>) -> Result<(), DriverError> {
        let ppm = values.get(&PPM);
        if self.facts.has_correction && (ppm - self.sent.ppm).abs() > 1e-9 {
            let mut p = dir_ch(call::SET_FREQ_CORRECTION, RX);
            p.f64(ppm);
            match self.void(&p) {
                Ok(()) => self.sent.radio_corrects = true,
                Err(RpcError::Exception(_)) => self.sent.radio_corrects = false,
                Err(e) => return Err(e.driver_error(call::SET_FREQ_CORRECTION)),
            }
            self.sent.ppm = ppm;
        }
        let centre = values.get(&CENTRE);
        let tuned = if self.facts.has_correction && self.sent.radio_corrects {
            centre
        } else {
            plan::corrected(centre, ppm)
        };
        let f = tuned.max(0.0);
        if self.sent.freq != Some(f) {
            let mut p = dir_ch(call::SET_FREQUENCY, RX);
            p.f64(f).kwargs(&Kwargs::new());
            self.set(call::SET_FREQUENCY, &p, RadioSetting::Centre.id())?;
            self.sent.freq = Some(f);
            notes.extend(plan::frequency_note(&self.facts.freqs, centre));
            // A server that does not let clients tune answers with the frequency it holds.
            let back = optional(self.call(
                &dir_ch(call::GET_FREQUENCY, RX),
                self.timing().call,
                |u| u.f64(),
            ))
            .map_err(|e| e.driver_error(call::GET_FREQUENCY))?
            .filter(|b| b.is_finite() && *b > 0.0)
            .unwrap_or(f);
            self.sent.held = Some((back, (back - f).abs() > HELD_TOLERANCE_HZ));
        }
        self.centre = match self.sent.held {
            Some((back, true)) => {
                // The session tunes inside the capture, if the wanted frequency lies in it. The first set-up of the radio
                // takes the frequency that the host had before a stack named its own, so it refuses nothing.
                let first = !self.ctx.set_up.swap(true, Ordering::AcqRel);
                if !first && (centre - back).abs() > USABLE * self.rate / 2.0 {
                    self.sent.freq = None;
                    return Err(DriverError::driver(SoapyError::HeldElsewhere {
                        centre_hz: back.round() as u64,
                        rate_hz: self.rate.round() as u64,
                        wanted_hz: centre.round() as u64,
                    }));
                }
                back
            }
            _ => centre,
        };
        let agc = values.get(&AGC);
        if self.facts.has_gain_mode && self.sent.gain_mode != Some(agc) {
            let mut p = dir_ch(call::SET_GAIN_MODE, RX);
            p.bool(agc);
            self.set(call::SET_GAIN_MODE, &p, RadioSetting::Agc.id())?;
            self.sent.gain_mode = Some(agc);
        }
        let mut gain_sent = false;
        if let (false, Some(range)) = (agc, self.facts.gain) {
            let rf = self.facts.rf_state(values).map_or(-1, |(_, v)| v);
            let rfgr = (rf >= 0).then_some(rf as i32);
            let gp = plan::gain_plan(range, &self.facts.elements, values.get(&GAIN), rfgr);
            if self.sent.gain.as_ref() != Some(&gp) {
                match &gp {
                    GainPlan::Overall(x) => {
                        let mut p = dir_ch(call::SET_GAIN, RX);
                        p.f64(*x);
                        self.set(call::SET_GAIN, &p, RadioSetting::Gain.id())?;
                    }
                    GainPlan::Elements(list) => {
                        for (name, x) in list {
                            let mut p = dir_ch(call::SET_GAIN_ELEMENT, RX);
                            p.string(name).f64(*x);
                            self.set(call::SET_GAIN_ELEMENT, &p, RadioSetting::Gain.id())?;
                        }
                    }
                }
                self.sent.gain = Some(gp);
                gain_sent = true;
            }
        }
        if let Some((key, _)) = self.facts.bias.clone() {
            let want = values.get(&BIAS_TEE);
            if want != self.sent.bias {
                let mut p = Packer::call(call::WRITE_SETTING);
                p.string(&key).string(if want { "true" } else { "false" });
                self.set(call::WRITE_SETTING, &p, RadioSetting::BiasTee.id())?;
                self.sent.bias = want;
                self.sent.bias_on = want;
            }
        }
        self.run_time(values, gain_sent)
    }

    /// Sends the run-time settings that changed. A gain element whose value is not its value at open is sent again after the
    /// gain was sent (`gain_sent`), because the gain overwrites it. A refusal is [`DriverError::Refused`] of the setting.
    fn run_time(&mut self, values: &Values, gain_sent: bool) -> Result<(), DriverError> {
        if self.sent.run_time.len() != self.facts.run_time.len() {
            self.sent.run_time = self
                .facts
                .run_time
                .iter()
                .map(|r| r.decl.default.clone())
                .collect();
        }
        for i in 0..self.facts.run_time.len() {
            let id = crate::DESCRIPTOR.run_time_id(i);
            let rt = &self.facts.run_time[i];
            let Some(v) = values
                .value(id)
                .filter(|v| v.kind() == Some(rt.decl.field.kind))
            else {
                continue;
            };
            let changed = self.sent.run_time[i] != *v;
            let (n, p) = match &rt.control {
                Control::Module { key, .. } => {
                    let Some(text) = rt.wire(v).filter(|_| changed) else {
                        continue;
                    };
                    let mut p = Packer::call(call::WRITE_SETTING);
                    p.string(key).string(&text);
                    (call::WRITE_SETTING, p)
                }
                // Sent with the gain.
                Control::RfState => continue,
                Control::Stage { element, at_open } => {
                    let Value::F64(x) = v else { continue };
                    if !(changed || gain_sent && (x - at_open).abs() > 1e-9) {
                        continue;
                    }
                    let mut p = dir_ch(call::SET_GAIN_ELEMENT, RX);
                    p.string(element).f64(*x);
                    (call::SET_GAIN_ELEMENT, p)
                }
            };
            let v = v.clone();
            self.set(n, &p, id)?;
            self.sent.run_time[i] = v;
        }
        Ok(())
    }

    /// Sets up the receive stream once: sockets or connections, call 300, the first acknowledgement, activate.
    fn setup_stream(&mut self, abort: &Arc<Abort>) -> Result<RxStream, DriverError> {
        let window_bytes = self.opts.window;
        let mtu = self.opts.message_size();
        let window = plan::window_packets(window_bytes, mtu);
        let fmt = self.format;
        let p = setup_call(RX, fmt.name(), 0, mtu, window_bytes, self.opts.transport);
        let sockets = match self.opts.transport {
            StreamProt::Udp => {
                let any = any_local(self.server_ip);
                let data = self.ctx.transport.bind(&DatagramOptions {
                    recv_buffer: Some(window_bytes),
                    ..DatagramOptions::at(any)
                })?;
                let status = self.ctx.transport.bind(&DatagramOptions::at(any))?;
                let mut p = p;
                p.string(&data.local_addr().port().to_string())
                    .string(&status.local_addr().port().to_string());
                let (id, sv_port) = self
                    .call(&p, self.timing().setup, |u| Ok((u.i32()?, u.string()?)))
                    .map_err(|e| e.driver_error(call::SETUP_STREAM))?;
                self.stream_id = Some(id);
                let sv_port: u16 = sv_port
                    .trim()
                    .parse()
                    .map_err(|_| DriverError::driver(SoapyError::StreamPort))?;
                let server = SocketAddr::new(self.server_ip, sv_port);
                RxSockets {
                    data: Wire::udp(data, server),
                    status: Wire::udp(status, server),
                }
            }
            StreamProt::Tcp => {
                let mut p = p;
                p.string("").string("");
                let (id, data, status) = wire::setup_tcp(
                    &mut self.conn(),
                    &p,
                    &self.ctx.transport,
                    self.server_ip,
                    self.timing().setup,
                    abort,
                    Some(window_bytes),
                )?;
                self.stream_id = Some(id);
                RxSockets { data, status }
            }
        };
        self.stream_abort = Some(abort.clone());
        let mut rx = RxStream::new(
            sockets,
            fmt.bytes(),
            self.shift,
            window,
            self.ctx.timing.clone(),
            abort.clone(),
            RxShared {
                stats: self.ctx.stats.clone(),
                flow: self.ctx.flow.clone(),
                notices: self.notices.clone(),
                label: self.ctx.label.clone(),
                link: self.ctx.link.clone(),
            },
        )?;
        rx.send_ack();
        if let Some(id) = self.stream_id {
            self.activate(id)?;
        }
        let mut info = lock(&self.ctx.info);
        info.window_packets = window;
        info.elements_per_datagram = plan::elements_per_datagram(mtu, fmt.bytes());
        Ok(rx)
    }

    /// Deactivates and closes the current stream, if any (each step waits `wait`).
    fn close_stream(&mut self, wait: Duration) {
        if let Some(id) = self.stream_id.take() {
            if !self.conn().is_broken() {
                let _ = self.deactivate(id, wait);
            }
            if !self.conn().is_broken() {
                let mut p = Packer::call(call::CLOSE_STREAM);
                p.i32(id);
                let _ = self.call(&p, wait, |u| u.void());
            }
        }
    }
}

impl Radio for SoapyRadio {
    type Stream = RxStream;

    fn readings(&mut self, results: &mut Results) {
        let info = lock(&self.ctx.info);
        let f = &info.facts;
        let stats = &self.ctx.stats;
        let g = |c: &AtomicU64| c.load(Ordering::Relaxed) as i64;
        let flow = *lock(&self.ctx.flow.last);
        {
            let mut r = results.fields_mut();
            r.set(&SERVER_ID, &info.server_id);
            r.set(&RPC_VERSION, &plan::show_version(info.version));
            r.set(&DRIVER, &f.driver_key);
            r.set(&HARDWARE, &f.hardware_key);
            r.set(&crate::SERIAL, &f.serial);
            r.set(&RX_CHANNELS, i64::from(f.rx_channels));
            r.set(&TX_CHANNELS, i64::from(f.tx_channels));
            r.opt(&WIRE_FORMAT, info.format.map(Format::name));
            r.opt(&NATIVE_FORMAT, f.native.as_ref().map(|n| n.0.as_str()));
            r.opt(&NATIVE_SCALE, f.native.as_ref().map(|n| n.1));
            r.set(&DATAGRAM, info.mtu as i64);
            r.set(&WINDOW_BYTES, info.window as i64);
            r.set(&WINDOW_PACKETS, i64::from(info.window_packets));
            r.set(&PER_DATAGRAM, info.elements_per_datagram as i64);
            r.opt(&MBIT_S, flow.map(|t| t.mbit_s));
            r.opt(&ARRIVING, flow.map(|t| t.achieved_hz));
            r.set(&OVERFLOWS, g(&stats.overflows).max(g(&stats.ssi_overflows)));
            r.set(&LOST_DATAGRAMS, g(&stats.lost_datagrams));
            r.set(&LATE, g(&stats.late));
            r.set(&MALFORMED, g(&stats.malformed) + g(&stats.truncated));
            r.set(&STREAM_ERRORS, g(&stats.errors));
            r.set(&RECONNECTS, g(&stats.reconnects));
            r.set(&LOG_DROPPED, g(&stats.log_dropped));
        }
        let sensors = lock(&self.ctx.sensors);
        write_items(
            results,
            &SENSORS,
            sensors.0.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        );
        write_items(
            results,
            &HARDWARE_INFO,
            f.hardware_info
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str())),
        );
    }

    fn settings(&self) -> Vec<SettingDecl> {
        self.facts.run_time.iter().map(|r| r.decl.clone()).collect()
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        self.facts.check(values)
    }

    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError> {
        if self.closed || self.conn().is_broken() {
            return Err(DriverError::Net(NetError::Closed));
        }
        self.facts.check(values)?;
        let mut notes = Vec::new();
        let asked = asked_rate(values);
        let rate_change = self.sent.asked_rate.is_none_or(|a| (a - asked).abs() > 1.0);
        if !live {
            self.sources()?;
        }
        let around = match (live && rate_change, self.stream_id) {
            (true, Some(id)) => {
                match self.deactivate(id, self.timing().call) {
                    Ok(_) | Err(RpcError::Exception(_)) => {}
                    Err(e) => return Err(e.driver_error(call::DEACTIVATE_STREAM)),
                }
                Some(id)
            }
            _ => None,
        };
        let mut res = self.rate_and_filter(values, asked, rate_change, &mut notes);
        if res.is_ok() {
            res = self.rest(values, &mut notes);
        }
        if let Some(id) = around {
            // Activated again whatever failed: a refused setting must not leave the receiver silent.
            let act = self.activate(id);
            if res.is_ok() {
                res = act;
            }
        }
        res.map(|()| notes.join("; "))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<RxStream, DriverError> {
        let attempts = if self.ctx.streamed.load(Ordering::Relaxed) {
            1
        } else {
            self.timing().stream_attempts.max(1)
        };
        let mut last = None;
        for attempt in 1..=attempts {
            if abort.fired() {
                break;
            }
            self.close_stream(self.timing().call);
            match self.setup_stream(abort) {
                Ok(mut rx) => match rx.wait_first(self.timing().first_data) {
                    Ok(()) => {
                        self.ctx.streamed.store(true, Ordering::Relaxed);
                        return Ok(rx);
                    }
                    Err(e) => last = Some(e),
                },
                Err(e) if !e.retryable() => return Err(e),
                Err(e) => last = Some(e),
            }
            if attempt < attempts {
                if let Some(e) = &last {
                    self.notices
                        .log(format!("SoapyRemote {}: {e}; trying again", self.ctx.label));
                }
                let end = Instant::now() + self.timing().retry_gap;
                while Instant::now() < end && !abort.fired() {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        Err(last.unwrap_or(DriverError::Net(NetError::Aborted)))
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let wait = self.timing().close_call;
        if self.sent.bias_on {
            if let Some((key, _)) = self.facts.bias.clone() {
                if !self.conn().is_broken() {
                    let mut p = Packer::call(call::WRITE_SETTING);
                    p.string(&key).string("false");
                    let _ = self.call(&p, wait, |u| u.void());
                }
            }
            self.sent.bias_on = false;
        }
        self.close_stream(wait);
        {
            let mut c = self.conn();
            if !c.is_broken() {
                let _ = c.call_void(&Packer::call(call::UNMAKE), wait);
            }
            hang_up(&mut c, wait);
        }
        if let Some(mut l) = self.log.take() {
            l.close();
        }
        let mut cur = lock(&self.ctx.ctl);
        if cur.as_ref().is_some_and(|c| Arc::ptr_eq(c, &self.ctl)) {
            *cur = None;
        }
    }

    fn rate_hz(&self) -> Hz {
        self.rate
    }

    fn centre_hz(&self) -> Hz {
        self.centre
    }

    fn capabilities(&self) -> Capabilities {
        self.facts.capabilities(&self.ctx.label)
    }

    fn sample_type(&self) -> SampleType {
        self.format.sample_type()
    }

    fn status(&self) -> String {
        let mut v = Vec::new();
        match self.sent.held {
            Some((back, true)) => v.push(format!(
                "follows the server, which holds the radio at {} MHz, {} Msps",
                sdrkit_driver_core::fmt_m(back),
                sdrkit_driver_core::fmt_m(self.rate)
            )),
            Some((_, false)) => v.push("steers the radio".to_owned()),
            None => {}
        }
        v.extend(sdrkit_driver_core::linkwatch::drop_text(
            self.ctx.link.part(),
        ));
        let s = self.ctx.stats.status();
        if !s.is_empty() {
            v.push(s);
        }
        v.join(", ")
    }
}

impl Drop for SoapyRadio {
    fn drop(&mut self) {
        self.close();
    }
}
