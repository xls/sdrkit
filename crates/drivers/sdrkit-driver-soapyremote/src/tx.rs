//! Transmit: a control connection and make of its own, and a transmit stream (UDP or TCP, as the receive stream) that the
//! driver sends within the window of the server's acknowledgements. A worker thread feeds the stream from a bounded sample
//! queue.
//!
//! Nothing reaches the transmit side of the radio before [`Transmitter::arm`]. Arm applies rate, filter, carrier and gain, then
//! sets up and activates the stream. Disarm stops the worker, deactivates and closes the stream.
//!
//! Standard: the SoapyRemote protocol, RPC version 0x0400 (`rpc::VERSION`).

use crate::format::{pack_cf32, pack_cs16};
use crate::plan;
use crate::radio::{self, any_local, setup_call, Ctx};
use crate::rpc::{call, optional, Conn, Kwargs, Packer, Range, RpcError, Unpacker, TX};
use crate::stream::{read_headers, DgHeader, HeaderBuf, END_BURST, HEADER_LEN, UNDERFLOW};
use crate::wire::{self, Wire};
use crate::{lock, SoapyError, StreamProt};
use core::net::SocketAddr;
use sdrkit_core::net::{DatagramOptions, NetError};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Hz, Results, Setting, SettingId, SettingSet, Values};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::Notices;
use sdrkit_driver_core::{
    tx_schema, Doing, DriverError, Transmitter, TxCapabilities, TxLimits, TxSetting, TxState,
    TxStats, TX_BANDWIDTH, TX_FREQUENCY, TX_POWER, TX_RATE,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Datagrams the sample queue holds.
const QUEUE_DATAGRAMS: usize = 64;
/// The MTU of the transmit stream.
const TX_MTU: usize = 1500;
/// The receive buffer size requested from the server for its transmit stream.
const TX_WINDOW: usize = 4 << 20;
/// On TCP the status path is read after this many messages.
const TCP_STATUS_EVERY: u32 = 64;

/// The transmit output, from 1.
pub const TX_OUTPUT: Setting<field::I64> = Setting::new("txoutput", "Output", "", 1)
    .range(1.0, 8.0, 1.0)
    .help(
        "The transmit output of the radio, from 1. An output the radio does not have is refused.",
    );
/// The clock error of the transmitter in ppm. Its key differs from the receive correction's: an open radio holds both in
/// one schema.
pub const TX_PPM: Setting<field::F64> =
    Setting::new("tx_ppm", "Transmit frequency correction", "ppm", 0.0)
        .range(-200.0, 200.0, 0.01)
        .help("The clock error in ppm, positive when the clock runs fast.");

sdrkit_core::settings! {
    /// The own transmit settings of the SoapyRemote driver. They follow [`TxSetting`].
    pub enum SoapyTxSetting {
        /// [`TX_OUTPUT`].
        Output = TX_OUTPUT,
        /// [`TX_PPM`].
        Ppm = TX_PPM,
    }
}

/// The power range (min, max, step) of a transmit channel whose gain range is `gain`: 0 dB at the highest gain. A
/// channel without a gain range has no power setting.
fn power(gain: Option<Range>) -> (f64, f64, f64) {
    match gain {
        Some(g) if g.max > g.min => (g.min - g.max, 0.0, if g.step > 0.0 { g.step } else { 0.25 }),
        _ => (0.0, 0.0, 0.0),
    }
}

/// The limits of a transmit channel with these tables: the span of its frequencies, its rates, the span of its filter
/// widths if it has any, and the power range of its gain range.
pub(crate) fn limits(
    freqs: &[Range],
    rates: &[Range],
    bandwidths: &[Range],
    gain: Option<Range>,
) -> TxLimits {
    let (min_freq_hz, max_freq_hz) = plan::span(freqs).unwrap_or((0.0, 0.0));
    TxLimits {
        min_freq_hz,
        max_freq_hz,
        rates: plan::rate_limits(rates),
        bandwidth_hz: plan::span(bandwidths),
        power_db: power(gain),
        start_power_db: None,
        common_rates_hz: Vec::new(),
        full_output_dbm: Vec::new(),
    }
}

/// The id of an own transmit setting in the transmit schema.
fn tx_id(s: SoapyTxSetting) -> SettingId {
    SettingId(TxSetting::SETTINGS.len() as u16 + s.id().0)
}

/// The channel of the output that `values` selects.
fn output_index(values: &Values, outputs: usize) -> Result<i32, DriverError> {
    match usize::try_from(values.get(&TX_OUTPUT)) {
        Ok(n) if (1..=outputs).contains(&n) => Ok(n as i32 - 1),
        _ => Err(DriverError::Refused {
            id: tx_id(SoapyTxSetting::Output),
        }),
    }
}

#[derive(Debug, Default)]
struct Counters {
    sent: AtomicU64,
    buffers: AtomicU64,
    padded: AtomicU64,
    underruns: AtomicU64,
    underrun_samples: AtomicU64,
    underflows: AtomicU64,
}

struct Worker {
    prod: rtrb::Producer<Complex32>,
    stop: Arc<AtomicBool>,
    flush: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), DriverError>>>,
}

impl Worker {
    fn stop(mut self) -> Option<DriverError> {
        self.stop.store(true, Ordering::Release);
        self.thread.take()?.join().ok().and_then(Result::err)
    }
    fn failed(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }
}

struct Facts {
    outputs: usize,
    rates: Vec<Range>,
    gain: Option<Range>,
    bandwidths: Vec<Range>,
    has_correction: bool,
    cs16: bool,
}

struct Core {
    ctl: Option<Conn>,
    ctx: Arc<Ctx>,
    facts: Facts,
    channel: i32,
    values: Values,
    state: TxState,
    rate: Hz,
    stream_id: Option<i32>,
    worker: Option<Worker>,
    counters: Arc<Counters>,
    transmitting_since: Option<Instant>,
    sent_freq: Option<Hz>,
    sent_gain: Option<f64>,
    sent_bw: Option<Hz>,
    sent_ppm: f64,
    radio_corrects: bool,
    elements: usize,
    notices: Notices,
    error: Option<DriverError>,
}

/// What the transmitter and the source share. The source closes it when it stops.
pub(crate) struct TxShared {
    core: Mutex<Core>,
    closed: AtomicBool,
    notices: Notices,
}

impl TxShared {
    /// Disarms and closes.
    pub fn close(&self) {
        lock(&self.core).close();
        self.closed.store(true, Ordering::Release);
    }
    /// It is closed.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

/// The SoapyRemote transmitter.
pub(crate) struct SoapyTransmitter {
    shared: Arc<TxShared>,
    caps: TxCapabilities,
}

fn dir_ch(n: u32, ch: i32) -> Packer {
    let mut p = Packer::call(n);
    p.dir_ch(TX, ch);
    p
}

impl SoapyTransmitter {
    /// Opens the transmitter of the radio `remote` on the server of `ctx` (a control connection and make of its own), disarmed.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the server cannot be reached, the radio cannot transmit, or the output is not one it has.
    pub fn open(
        ctx: &Arc<Ctx>,
        remote: &Kwargs,
        values: &Values,
    ) -> Result<(SoapyTransmitter, Arc<TxShared>), DriverError> {
        let t = &ctx.timing;
        let mut c = radio::connect(ctx, &Abort::new())?;
        let fail = |c: &mut Conn, made: bool, e: DriverError| {
            if made && !c.is_broken() {
                let _ = c.call_void(&Packer::call(call::UNMAKE), t.close_call);
            }
            radio::hang_up(c, t.close_call);
            e
        };
        match c.call(&Packer::call(call::SERVER_ID), t.call, |u| u.string()) {
            Ok(_) | Err(RpcError::Exception(_)) => {}
            Err(e) => return Err(fail(&mut c, false, e.driver_error(call::SERVER_ID))),
        }
        if let Err(e) = radio::make(&mut c, remote, remote.is_empty(), t.make) {
            return Err(fail(&mut c, false, e));
        }
        macro_rules! opt {
            ($n:expr, $p:expr, $f:expr) => {
                match optional(c.call(&$p, t.call, $f)) {
                    Ok(v) => v,
                    Err(e) => return Err(fail(&mut c, true, e.driver_error($n))),
                }
            };
        }
        let mut p = Packer::call(call::NUM_CHANNELS);
        p.char(TX);
        let outputs = opt!(call::NUM_CHANNELS, p, |u: &mut Unpacker| u.i32())
            .unwrap_or(0)
            .max(0) as usize;
        if outputs == 0 {
            return Err(fail(&mut c, true, DriverError::CannotTransmit));
        }
        let channel = match output_index(values, outputs) {
            Ok(ch) => ch,
            Err(e) => return Err(fail(&mut c, true, e)),
        };
        let mut rates = opt!(
            call::SAMPLE_RATE_RANGE,
            dir_ch(call::SAMPLE_RATE_RANGE, channel),
            |u: &mut Unpacker| u.range_list()
        )
        .unwrap_or_default();
        if rates.is_empty() {
            rates = opt!(
                call::LIST_SAMPLE_RATES,
                dir_ch(call::LIST_SAMPLE_RATES, channel),
                |u: &mut Unpacker| u.f64_list()
            )
            .unwrap_or_default()
            .into_iter()
            .map(|r| Range::new(r, r, 0.0))
            .collect();
        }
        let gain = opt!(
            call::GAIN_RANGE,
            dir_ch(call::GAIN_RANGE, channel),
            |u: &mut Unpacker| u.range()
        );
        let freqs = opt!(
            call::FREQUENCY_RANGE,
            dir_ch(call::FREQUENCY_RANGE, channel),
            |u: &mut Unpacker| u.range_list()
        )
        .unwrap_or_default();
        let has_correction = opt!(
            call::HAS_FREQ_CORRECTION,
            dir_ch(call::HAS_FREQ_CORRECTION, channel),
            |u: &mut Unpacker| u.bool()
        )
        .unwrap_or(false);
        let formats = opt!(
            call::STREAM_FORMATS,
            dir_ch(call::STREAM_FORMATS, channel),
            |u: &mut Unpacker| u.string_list()
        );
        let bandwidths = opt!(
            call::BANDWIDTH_RANGE,
            dir_ch(call::BANDWIDTH_RANGE, channel),
            |u: &mut Unpacker| u.range_list()
        )
        .unwrap_or_default();
        let full_duplex = opt!(
            call::FULL_DUPLEX,
            dir_ch(call::FULL_DUPLEX, channel),
            |u: &mut Unpacker| u.bool()
        )
        .unwrap_or(false);
        let driver = opt!(
            call::DRIVER_KEY,
            Packer::call(call::DRIVER_KEY),
            |u: &mut Unpacker| u.string()
        )
        .unwrap_or_default();
        let cs16 = formats
            .as_ref()
            .is_none_or(|f| f.iter().any(|s| s.eq_ignore_ascii_case("CS16")));
        let bpe = if cs16 { 4 } else { 8 };
        let elements = plan::elements_per_datagram(TX_MTU, bpe);
        let limits = limits(&freqs, &rates, &bandwidths, gain);
        let caps = TxCapabilities {
            name: format!("SoapyRemote {} {} TX{}", ctx.label, driver, channel + 1),
            settings: tx_schema(SoapyTxSetting::SETTINGS, &limits),
            limits,
            outputs: (1..=outputs).map(|i| format!("tx{i}")).collect(),
            cyclic: false,
            full_duplex,
            buffer_samples: elements,
        };
        let notices = Notices::default();
        let core = Core {
            ctl: Some(c),
            ctx: ctx.clone(),
            facts: Facts {
                outputs,
                rates,
                gain,
                bandwidths,
                has_correction,
                cs16,
            },
            channel,
            values: values.clone(),
            state: TxState::Disarmed,
            rate: 0.0,
            stream_id: None,
            worker: None,
            counters: Arc::default(),
            transmitting_since: None,
            sent_freq: None,
            sent_gain: None,
            sent_bw: None,
            sent_ppm: 0.0,
            radio_corrects: true,
            elements,
            notices: notices.clone(),
            error: None,
        };
        let shared = Arc::new(TxShared {
            core: Mutex::new(core),
            closed: AtomicBool::new(false),
            notices,
        });
        Ok((
            SoapyTransmitter {
                shared: shared.clone(),
                caps,
            },
            shared,
        ))
    }
}

fn not_now(state: Doing) -> DriverError {
    DriverError::NotNow { state }
}

impl Core {
    fn conn(&mut self) -> Result<&mut Conn, DriverError> {
        match self.ctl.as_mut() {
            Some(c) if !c.is_broken() => Ok(c),
            _ => Err(DriverError::Net(NetError::Closed)),
        }
    }

    fn void(&mut self, p: &Packer) -> Result<(), RpcError> {
        let t = self.ctx.timing.call;
        match self.ctl.as_mut() {
            Some(c) => c.call_void(p, t),
            None => Err(RpcError::Broken),
        }
    }

    /// A call that the radio can refuse: its refusal is [`DriverError::Refused`] of `id`.
    fn set(&mut self, n: u32, p: &Packer, id: SettingId) -> Result<(), DriverError> {
        match self.void(p) {
            Ok(()) => Ok(()),
            Err(RpcError::Exception(_)) => Err(DriverError::Refused { id }),
            Err(e) => Err(e.driver_error(n)),
        }
    }

    /// The rate to send: the transmit rate, else the rate of the receiver, else 1 Msps.
    fn rate_for(&self, values: &Values) -> Hz {
        let asked = match (values.get(&TX_RATE), self.ctx.flow.asked()) {
            (r, _) if r > 0.0 => r,
            (_, r) if r > 0.0 => r,
            _ => 1e6,
        };
        plan::choose_rate_up(&self.facts.rates, asked)
    }

    /// Applies the rate (if `with_rate`), the filter, the carrier and the gain.
    fn apply(&mut self, values: &Values, with_rate: bool) -> Result<(), DriverError> {
        let ch = self.channel;
        if with_rate {
            let r = self.rate_for(values);
            let mut p = dir_ch(call::SET_SAMPLE_RATE, ch);
            p.f64(r);
            self.set(call::SET_SAMPLE_RATE, &p, TxSetting::Rate.id())?;
            let t = self.ctx.timing.call;
            let back = optional(
                self.conn()?
                    .call(&dir_ch(call::GET_SAMPLE_RATE, ch), t, |u| u.f64()),
            )
            .map_err(|e| e.driver_error(call::GET_SAMPLE_RATE))?
            .filter(|v| *v > 0.0)
            .unwrap_or(r);
            self.rate = back;
        }
        if !self.facts.bandwidths.is_empty() {
            let asked = values.get(&TX_BANDWIDTH);
            let want = if asked > 0.0 { asked } else { 0.95 * self.rate };
            if self.sent_bw.is_none_or(|b| (b - want).abs() > 0.5) {
                if let Some(bw) = plan::choose_bandwidth(&self.facts.bandwidths, want) {
                    let mut p = dir_ch(call::SET_BANDWIDTH, ch);
                    p.f64(bw);
                    match self.void(&p) {
                        Ok(()) => {}
                        Err(RpcError::Exception(_)) if asked > 0.0 => {
                            return Err(DriverError::Refused {
                                id: TxSetting::Bandwidth.id(),
                            })
                        }
                        Err(RpcError::Exception(_)) => {}
                        Err(e) => return Err(e.driver_error(call::SET_BANDWIDTH)),
                    }
                }
                self.sent_bw = Some(want);
            }
        }
        let ppm = values.get(&TX_PPM);
        if self.facts.has_correction && (ppm - self.sent_ppm).abs() > 1e-9 {
            let mut p = dir_ch(call::SET_FREQ_CORRECTION, ch);
            p.f64(ppm);
            match self.void(&p) {
                Ok(()) => self.radio_corrects = true,
                Err(RpcError::Exception(_)) => self.radio_corrects = false,
                Err(e) => return Err(e.driver_error(call::SET_FREQ_CORRECTION)),
            }
            self.sent_ppm = ppm;
        }
        let centre = values.get(&TX_FREQUENCY);
        let f = if self.facts.has_correction && self.radio_corrects {
            centre
        } else {
            plan::corrected(centre, ppm)
        }
        .max(0.0);
        if self.sent_freq != Some(f) {
            let mut p = dir_ch(call::SET_FREQUENCY, ch);
            p.f64(f).kwargs(&Kwargs::new());
            self.set(call::SET_FREQUENCY, &p, TxSetting::Frequency.id())?;
            self.sent_freq = Some(f);
        }
        match self.facts.gain {
            Some(g) if g.max > g.min => {
                let gain = g.max + values.get(&TX_POWER);
                if self.sent_gain != Some(gain) {
                    let mut p = dir_ch(call::SET_GAIN, ch);
                    p.f64(gain);
                    self.set(call::SET_GAIN, &p, TxSetting::Power.id())?;
                    self.sent_gain = Some(gain);
                }
            }
            _ => self.notices.log(format!(
                "SoapyRemote {}: the radio gives no transmit gain range; the gain was left as it is",
                self.ctx.label
            )),
        }
        Ok(())
    }

    fn configure(&mut self, values: &Values) -> Result<(), DriverError> {
        if self.state == TxState::Closed {
            return Err(not_now(Doing::Closed));
        }
        let ch = output_index(values, self.facts.outputs)?;
        if self.state.transmitting() {
            if ch != self.channel
                || (self.rate_for(values) - self.rate_for(&self.values)).abs() > 1.0
            {
                return Err(not_now(Doing::Transmitting));
            }
            self.apply(values, false)?;
            self.values = values.clone();
            return Ok(());
        }
        if ch != self.channel {
            self.channel = ch;
            (self.sent_freq, self.sent_gain, self.sent_bw) = (None, None, None);
        }
        self.values = values.clone();
        Ok(())
    }

    fn arm(&mut self) -> Result<(), DriverError> {
        match self.state {
            TxState::Closed => return Err(not_now(Doing::Closed)),
            TxState::Armed | TxState::Cyclic => return Ok(()),
            TxState::Disarmed => {}
        }
        let values = self.values.clone();
        self.apply(&values, true)?;
        let ctx = self.ctx.clone();
        let server_ip = ctx.transport.resolve(&ctx.host, ctx.port)?.ip();
        let fmt = if self.facts.cs16 { "CS16" } else { "CF32" };
        let prot = if ctx.tcp.load(Ordering::Relaxed) {
            StreamProt::Tcp
        } else {
            StreamProt::Udp
        };
        let mut p = setup_call(TX, fmt, self.channel, TX_MTU, TX_WINDOW, prot);
        let t = ctx.timing.setup;
        let parts = if prot == StreamProt::Tcp {
            p.string("").string("");
            let abort = Abort::new();
            let (id, data, status) =
                wire::setup_tcp(self.conn()?, &p, &ctx.transport, server_ip, t, &abort, None)?;
            self.stream_id = Some(id);
            SenderParts { data, status }
        } else {
            let any = any_local(server_ip);
            let data = ctx.transport.bind(&DatagramOptions {
                recv_buffer: Some(1 << 20),
                ..DatagramOptions::at(any)
            })?;
            let status = ctx.transport.bind(&DatagramOptions::at(any))?;
            p.string(&data.local_addr().port().to_string())
                .string(&status.local_addr().port().to_string());
            let (id, port) = self
                .conn()?
                .call(&p, t, |u| Ok((u.i32()?, u.string()?)))
                .map_err(|e| e.driver_error(call::SETUP_STREAM))?;
            self.stream_id = Some(id);
            let port: u16 = port
                .trim()
                .parse()
                .map_err(|_| DriverError::driver(SoapyError::StreamPort))?;
            let server = SocketAddr::new(server_ip, port);
            SenderParts {
                data: Wire::udp(data, server),
                status: Wire::udp(status, server),
            }
        };
        let id = self.stream_id.unwrap_or_default();
        let mut p = Packer::call(call::ACTIVATE_STREAM);
        p.i32(id).i32(0).i64(0).i32(0);
        let t = ctx.timing.call;
        match self.conn()?.call(&p, t, |u| u.i32()) {
            Ok(0) => {}
            Ok(code) => {
                self.close_stream();
                return Err(DriverError::Device { code });
            }
            Err(e) => return Err(e.driver_error(call::ACTIVATE_STREAM)),
        }
        let (prod, cons) = rtrb::RingBuffer::new(self.elements * QUEUE_DATAGRAMS);
        let stop = Arc::new(AtomicBool::new(false));
        let flush = Arc::new(AtomicBool::new(false));
        let w = Sender::start(
            parts,
            cons,
            stop.clone(),
            flush.clone(),
            self.counters.clone(),
            self.elements,
            self.facts.cs16,
            &ctx.timing,
            self.rate,
        )?;
        let thread = std::thread::Builder::new()
            .name("sdrkit-soapy-tx".into())
            .spawn(move || w.run())
            .map_err(|_| DriverError::Thread)?;
        self.worker = Some(Worker {
            prod,
            stop,
            flush,
            thread: Some(thread),
        });
        self.state = TxState::Armed;
        self.transmitting_since = Some(Instant::now());
        self.error = None;
        Ok(())
    }

    fn close_stream(&mut self) {
        let wait = self.ctx.timing.close_call;
        if let Some(id) = self.stream_id.take() {
            if let Some(c) = self.ctl.as_mut().filter(|c| !c.is_broken()) {
                let mut p = Packer::call(call::DEACTIVATE_STREAM);
                p.i32(id).i32(0).i64(0);
                let _ = c.call(&p, wait, |u| u.i32());
            }
            if let Some(c) = self.ctl.as_mut().filter(|c| !c.is_broken()) {
                let mut p = Packer::call(call::CLOSE_STREAM);
                p.i32(id);
                let _ = c.call_void(&p, wait);
            }
        }
    }

    fn disarm(&mut self) {
        if let Some(w) = self.worker.take() {
            if let Some(e) = w.stop() {
                self.notices
                    .warn(format!("SoapyRemote {}: {e}", self.ctx.label));
            }
        }
        self.close_stream();
        if self.state != TxState::Closed {
            self.state = TxState::Disarmed;
        }
        self.transmitting_since = None;
    }

    fn close(&mut self) {
        self.disarm();
        if let Some(mut c) = self.ctl.take() {
            let wait = self.ctx.timing.close_call;
            if !c.is_broken() {
                let _ = c.call_void(&Packer::call(call::UNMAKE), wait);
            }
            radio::hang_up(&mut c, wait);
        }
        self.state = TxState::Closed;
    }

    /// Looks at the worker: a failed one ends the transmission with its error.
    fn check(&mut self) -> Result<(), DriverError> {
        if self.worker.as_ref().is_some_and(Worker::failed) {
            let e = self
                .worker
                .take()
                .and_then(Worker::stop)
                .unwrap_or(DriverError::Net(NetError::Closed));
            self.close_stream();
            self.state = TxState::Disarmed;
            self.transmitting_since = None;
            self.error = Some(e.clone());
            return Err(e);
        }
        match &self.error {
            Some(e) if self.state == TxState::Disarmed => Err(e.clone()),
            _ => Ok(()),
        }
    }
}

/// The sender's flow-control state: it can send while (next - acknowledged) is below the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SendWindow {
    /// The next sequence to send.
    pub next: u32,
    /// The latest acknowledged sequence (the receiver's next expected).
    pub acked: u32,
    /// The window of the latest acknowledgement. `None` before the first.
    pub window: Option<u32>,
}

impl SendWindow {
    /// One more datagram can go out.
    pub fn may_send(&self) -> bool {
        self.window
            .is_some_and(|w| self.next.wrapping_sub(self.acked) < w)
    }
    /// Takes an acknowledgement of `acked` with a window of `window` packets (at least 1).
    pub fn ack(&mut self, acked: u32, window: u32) {
        self.acked = acked;
        self.window = Some(window.max(1));
    }
}

/// The latest acknowledgement of the server.
#[derive(Debug, Default)]
struct Acks {
    /// Acknowledgements so far.
    count: u64,
    /// The sequence of the latest one.
    acked: u32,
    /// Its window (0 before the first).
    window: u32,
}

/// The paths of a transmit stream.
struct SenderParts {
    data: Wire,
    status: Wire,
}

/// The worker that sends datagrams within the window of the server.
struct Sender {
    data: Wire,
    status: Wire,
    bufs: [HeaderBuf; 2],
    /// Messages sent since the status path was last read (TCP reads it every [`TCP_STATUS_EVERY`]).
    since_status: u32,
    acks: Acks,
    /// The first error code of the status socket (0: none).
    fault: i32,
    cons: rtrb::Consumer<Complex32>,
    stop: Arc<AtomicBool>,
    flush: Arc<AtomicBool>,
    counters: Arc<Counters>,
    elements: usize,
    cs16: bool,
    slice: Duration,
    waits: u32,
    rate: Hz,
}

impl Sender {
    #[allow(clippy::too_many_arguments)]
    fn start(
        parts: SenderParts,
        cons: rtrb::Consumer<Complex32>,
        stop: Arc<AtomicBool>,
        flush: Arc<AtomicBool>,
        counters: Arc<Counters>,
        elements: usize,
        cs16: bool,
        timing: &crate::Timing,
        rate: Hz,
    ) -> Result<Sender, DriverError> {
        Ok(Sender {
            data: parts.data,
            status: parts.status,
            bufs: [[0; 64]; 2],
            since_status: 0,
            acks: Acks::default(),
            fault: 0,
            cons,
            stop,
            flush,
            counters,
            elements,
            cs16,
            slice: timing.slice,
            waits: timing.tx_waits,
            rate,
        })
    }

    /// Reads the acknowledgements on the data path and the reports on the status path that wait, without waiting. On TCP,
    /// where each poll waits a little, the acknowledgements are read only when `closed` (the window is shut) and the status
    /// every [`TCP_STATUS_EVERY`] messages.
    fn read_headers(&mut self, closed: bool) {
        let tcp = self.data.is_tcp();
        let [dbuf, sbuf] = &mut self.bufs;
        if !tcp || closed {
            let acks = &mut self.acks;
            read_headers(&mut self.data, dbuf, |h| {
                acks.acked = h.seq;
                acks.window = h.elems.max(1) as u32;
                acks.count += 1;
            });
        }
        if tcp && !closed && self.since_status < TCP_STATUS_EVERY {
            return;
        }
        self.since_status = 0;
        let (fault, counters) = (&mut self.fault, &self.counters);
        read_headers(&mut self.status, sbuf, |h| match h.elems {
            code if code >= 0 => {}
            UNDERFLOW => {
                counters.underflows.fetch_add(1, Ordering::Relaxed);
            }
            code if *fault == 0 => *fault = code,
            _ => {}
        });
    }

    /// The window as the latest acknowledgement leaves it.
    fn window(&self, win: &mut SendWindow) {
        if self.acks.window > 0 {
            win.ack(self.acks.acked, self.acks.window);
        }
    }

    /// Waits one slice for a new acknowledgement. It returns `true` if one came.
    fn wait_ack(&mut self) -> bool {
        let before = self.acks.count;
        let end = Instant::now() + self.slice;
        while Instant::now() < end && !self.stop.load(Ordering::Acquire) {
            self.read_headers(true);
            if self.acks.count != before {
                return true;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        false
    }

    fn run(mut self) -> Result<(), DriverError> {
        let bpe = if self.cs16 { 4 } else { 8 };
        let mut out = vec![0u8; HEADER_LEN + self.elements * bpe];
        let mut iq = vec![Complex32::new(0.0, 0.0); self.elements];
        let mut win = SendWindow::default();
        let mut waited = 0u32;
        let mut in_burst = false;
        // When the queue ran dry in the middle of a burst, and whether that gap already counts as an underrun.
        let mut dry: Option<(Instant, bool)> = None;
        let datagram_time = Duration::from_secs_f64(self.elements as f64 / self.rate.max(1.0));
        loop {
            if self.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            self.read_headers(!win.may_send());
            if self.fault != 0 {
                return Err(DriverError::Device { code: self.fault });
            }
            self.window(&mut win);
            if !win.may_send() {
                if self.wait_ack() {
                    waited = 0;
                } else {
                    waited += 1;
                    if waited > self.waits {
                        return Err(DriverError::driver(SoapyError::TxStalled));
                    }
                }
                continue;
            }
            let flushing = self.flush.load(Ordering::Acquire);
            let have = self.cons.slots();
            if have == 0 || (have < self.elements && !flushing) {
                if flushing && have == 0 {
                    self.flush.store(false, Ordering::Release);
                    in_burst = false;
                    dry = None;
                }
                if in_burst && have == 0 {
                    let (since, counted) = dry.get_or_insert((Instant::now(), false));
                    if !*counted && since.elapsed() > datagram_time {
                        // The radio has sent everything it had: an underrun, once per gap.
                        *counted = true;
                        self.counters.underruns.fetch_add(1, Ordering::Relaxed);
                    }
                }
                std::thread::sleep(Duration::from_micros(500));
                continue;
            }
            if let Some((since, true)) = dry.take() {
                let gap = (since.elapsed().saturating_sub(datagram_time)).as_secs_f64() * self.rate;
                self.counters
                    .underrun_samples
                    .fetch_add(gap as u64, Ordering::Relaxed);
            }
            let k = have.min(self.elements);
            let Ok(chunk) = self.cons.read_chunk(k) else {
                continue;
            };
            let (a, b) = chunk.as_slices();
            iq[..a.len()].copy_from_slice(a);
            iq[a.len()..k].copy_from_slice(b);
            chunk.commit_all();
            let last = flushing && self.cons.slots() == 0;
            let bytes = HEADER_LEN + k * bpe;
            let h = DgHeader {
                bytes: bytes as u32,
                seq: win.next,
                elems: k as i32,
                flags: if last { END_BURST } else { 0 },
                time: 0,
            };
            out[..HEADER_LEN].copy_from_slice(&h.to_bytes());
            if self.cs16 {
                pack_cs16(&iq[..k], &mut out[HEADER_LEN..bytes]);
            } else {
                pack_cf32(&iq[..k], &mut out[HEADER_LEN..bytes]);
            }
            let stop = &self.stop;
            match self
                .data
                .send(&out[..bytes], &mut || stop.load(Ordering::Acquire))
            {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && !self.data.is_tcp() => {
                    continue
                }
                Err(_) if self.data.is_tcp() && stop.load(Ordering::Acquire) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {}
                Err(e) => return Err(DriverError::Net(NetError::from(e))),
            }
            win.next = win.next.wrapping_add(1);
            self.since_status += 1;
            self.counters.sent.fetch_add(k as u64, Ordering::Relaxed);
            self.counters.buffers.fetch_add(1, Ordering::Relaxed);
            in_burst = !last;
            if last {
                self.flush.store(false, Ordering::Release);
            }
        }
    }
}

impl Transmitter for SoapyTransmitter {
    fn capabilities(&self) -> &TxCapabilities {
        &self.caps
    }
    fn configure(&mut self, values: &Values) -> Result<(), DriverError> {
        lock(&self.shared.core).configure(values)
    }
    fn arm(&mut self) -> Result<(), DriverError> {
        let mut c = lock(&self.shared.core);
        let r = c.arm();
        if r.is_err() {
            c.disarm();
        }
        r
    }
    fn disarm(&mut self) {
        lock(&self.shared.core).disarm();
    }
    fn push(&mut self, iq: &[Complex32], wait: Duration) -> Result<usize, DriverError> {
        let end = Instant::now() + wait;
        let mut done = 0;
        loop {
            {
                let mut c = lock(&self.shared.core);
                c.check()?;
                if c.state != TxState::Armed {
                    return Err(not_now(Doing::Disarmed));
                }
                let Some(w) = c.worker.as_mut() else {
                    return Err(not_now(Doing::Disarmed));
                };
                let n = (iq.len() - done).min(w.prod.slots());
                if n > 0 {
                    if let Ok(mut chunk) = w.prod.write_chunk_uninit(n) {
                        let (a, b) = chunk.as_mut_slices();
                        for (d, s) in a.iter_mut().chain(b.iter_mut()).zip(&iq[done..done + n]) {
                            let m = s.norm();
                            d.write(if m > 1.0 { s / m } else { *s });
                        }
                        // SAFETY: every one of the n slots was just written.
                        unsafe { chunk.commit_all() };
                        done += n;
                    }
                }
            }
            if done == iq.len() || Instant::now() >= end {
                return Ok(done);
            }
            std::thread::sleep(Duration::from_micros(500));
        }
    }
    fn flush(&mut self) -> Result<(), DriverError> {
        let mut c = lock(&self.shared.core);
        c.check()?;
        if c.state != TxState::Armed {
            return Err(not_now(Doing::Disarmed));
        }
        let e = c.elements;
        let counters = c.counters.clone();
        let Some(w) = c.worker.as_mut() else {
            return Err(not_now(Doing::Disarmed));
        };
        let queued = w.prod.buffer().capacity() - w.prod.slots();
        let pad = (e - queued % e) % e;
        let mut padded = 0;
        for _ in 0..pad {
            if w.prod.push(Complex32::new(0.0, 0.0)).is_err() {
                break;
            }
            padded += 1;
        }
        counters.padded.fetch_add(padded as u64, Ordering::Relaxed);
        w.flush.store(true, Ordering::Release);
        Ok(())
    }
    fn play_cyclic(&mut self, _iq: &[Complex32]) -> Result<(), DriverError> {
        Err(DriverError::driver(SoapyError::NoCyclic))
    }
    fn state(&self) -> TxState {
        let mut c = lock(&self.shared.core);
        let _ = c.check();
        c.state
    }
    fn stats(&self) -> TxStats {
        let c = lock(&self.shared.core);
        let k = &c.counters;
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        TxStats {
            sent: g(&k.sent),
            buffers: g(&k.buffers),
            queued: c
                .worker
                .as_ref()
                .map_or(0, |w| w.prod.buffer().capacity() - w.prod.slots()),
            padded: g(&k.padded),
            underruns: g(&k.underruns) + g(&k.underflows),
            underrun_samples: g(&k.underrun_samples),
            transmitting_since: c.transmitting_since,
        }
    }
    fn poll(&mut self, results: &mut Results) {
        self.shared.notices.drain_into(results);
    }
    fn close(&mut self) {
        self.shared.close();
    }
}

impl Drop for SoapyTransmitter {
    fn drop(&mut self) {
        self.shared.close();
    }
}
