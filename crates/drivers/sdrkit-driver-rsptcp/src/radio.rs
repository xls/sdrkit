//! One connection to an rsp_tcp server: connect, greeting, commands, and the read loop.

use crate::model::{split_gain, Antenna, Model};
use crate::proto::{self, cmd, notch, ExtendedTail, Head, SampleFormat, MAGIC_RTL};
use crate::{
    lock, plan_rate, AddrState, CommandSet, OpenState, Options, RspError, RspSetting, ServerInfo,
    Throughput, Timing, Window, THROUGHPUT,
};
use sdrkit_core::net::{is_local, timed_out, Conn, NetError, Transport};
use sdrkit_core::{Hz, Results, SettingSet, Value, Values};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::rtl_tcp::{nearest_gain, Greeting};
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::{
    own_id, Capabilities, DriverError, Protocol, RadioSetting, RateLimits, AGC, BIAS_TEE, CENTRE,
    GAIN,
};
use sdrkit_graph::stream::SampleType;
use std::io::{self, ErrorKind, Read, Write};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The largest read.
const READ: usize = 1 << 20;

/// Where a server is and how the driver waits on it.
pub(crate) struct At {
    pub transport: Arc<dyn Transport>,
    pub host: String,
    pub port: u16,
    pub label: String,
    pub timing: Timing,
}

/// Reads exactly `buf.len()` bytes by `deadline`.
fn read_by(s: &mut dyn Conn, buf: &mut [u8], deadline: Instant) -> io::Result<()> {
    let mut got = 0;
    while got < buf.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(ErrorKind::TimedOut.into());
        }
        s.set_read_timeout(Some(left)).map_err(io::Error::from)?;
        match s.read(&mut buf[got..]) {
            Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
            Ok(n) => got += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn refused(s: RspSetting) -> DriverError {
    DriverError::Refused { id: own_id(s) }
}

/// One connection.
pub struct RspRadio {
    label: String,
    sock: Box<dyn Conn>,
    info: ServerInfo,
    local: bool,
    /// The last value sent of each command byte. Cleared at each connect.
    sent: [Option<u32>; 0x27],
    bias_on: bool,
    refout_on: bool,
    opts: Options,
    rate: u32,
    centre: Hz,
    gmax: f64,
    addr: Arc<AddrState>,
    open: Arc<OpenState>,
    timing: Timing,
    closed: bool,
}

impl RspRadio {
    /// Connects to the server at `at`, reads its greeting and sets the gain mode.
    ///
    /// # Errors
    ///
    /// [`DriverError::Net`] if the server cannot be reached, [`DriverError::Busy`] if it sends no greeting,
    /// [`DriverError::Protocol`] if it is not an rsp_tcp or rtl_tcp server, [`RspError`] for a greeting the settings refuse.
    pub(crate) fn connect(
        at: &At,
        addr: Arc<AddrState>,
        open: Arc<OpenState>,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<RspRadio, DriverError> {
        let local = is_local(&at.host) && !at.timing.local_as_remote;
        let timeout = Duration::from_secs(if is_local(&at.host) { 10 } else { 8 });
        let mut tries = 0;
        let mut sock = loop {
            tries += 1;
            match net::connect(&at.transport, &at.host, at.port, false, timeout, abort) {
                Ok(s) => break s,
                Err(NetError::TimedOut) if tries == 1 && !abort.fired() => continue,
                Err(e) => return Err(e.into()),
            }
        };
        net::recv_buffer(sock.as_mut(), &[16 << 20, 8 << 20, 4 << 20]);
        let opts = Options::from_values(values);
        let info = match read_greeting(sock.as_mut(), &at.label, &opts, &addr, at.timing, notices) {
            Ok(i) => i,
            Err(e) => {
                sock.shutdown();
                return Err(e);
            }
        };
        {
            let mut first = lock(&open.format);
            match *first {
                Some(was) if was != info.format => {
                    sock.shutdown();
                    return Err(DriverError::driver(RspError::FormatChanged {
                        was,
                        now: info.format,
                    }));
                }
                _ => *first = Some(info.format),
            }
        }
        *lock(&addr.info) = Some(info.clone());
        // A new connection: the throughput totals start again.
        *lock(&addr.window) = Window::default();
        let gmax = info.model.map_or(49.6, |m| m.gmax(false));
        let mut r = RspRadio {
            label: at.label.clone(),
            sock,
            info,
            local,
            sent: [None; 0x27],
            bias_on: false,
            refout_on: false,
            opts,
            rate: 0,
            centre: values.get(&CENTRE),
            gmax,
            addr,
            open,
            timing: at.timing,
            closed: false,
        };
        let res = if r.info.extended {
            let agc = values.get(&AGC);
            r.send(cmd::GAIN_MODE, 1)
                .and_then(|_| r.send(cmd::AGC, u32::from(agc)))
        } else {
            r.send(cmd::GAIN_MODE, 1)
                .and_then(|_| r.send(cmd::RTL_AGC, 0))
        };
        if let Err(e) = res {
            r.close();
            return Err(e);
        }
        Ok(r)
    }

    /// Sends one command. The radio is guarded by the lock of the base, held only for a configure or a close, so the 5 bytes of
    /// one command never mix with those of another.
    fn send(&mut self, c: u8, v: u32) -> Result<(), DriverError> {
        self.sock
            .write_all(&proto::command(c, v))
            .map_err(|_| DriverError::Net(NetError::Closed))?;
        self.sent[usize::from(c)] = Some(v);
        Ok(())
    }

    fn send_changed(&mut self, c: u8, v: u32) -> Result<(), DriverError> {
        if self.sent[usize::from(c)] != Some(v) {
            self.send(c, v)?;
        }
        Ok(())
    }

    fn model(&self) -> Model {
        self.info.model.unwrap_or(Model::Unknown(0))
    }

    fn note(&self, notes: &mut Vec<String>, what: &'static str, text: String) {
        if self.open.first(what) {
            notes.push(text);
        }
    }

    /// Refuses a value that needs a control the server does not offer.
    fn check_controls(&self, o: &Options, v: &Values) -> Result<(), DriverError> {
        let c = self.info.controls();
        let model = self.model();
        if o.antenna != Antenna::First && (!c.antenna || o.antenna_index(model).is_none()) {
            return Err(refused(RspSetting::Antenna));
        }
        for (on, offered, s) in [
            (o.rfnotch, c.fm_notch, RspSetting::RfNotch),
            (o.dabnotch, c.dab_notch, RspSetting::DabNotch),
            (o.amnotch, c.am_notch, RspSetting::AmNotch),
            (o.refout, c.refout, RspSetting::RefOut),
            (o.lnastate >= 0, self.info.extended, RspSetting::LnaState),
            (o.ifgr >= 0, self.info.extended, RspSetting::Ifgr),
        ] {
            if on && !offered {
                return Err(refused(s));
            }
        }
        if self.info.extended && v.get(&BIAS_TEE) && !c.bias {
            return Err(DriverError::Refused {
                id: RadioSetting::BiasTee.id(),
            });
        }
        if self.info.extended && o.lnastate >= 0 {
            let f = v.get(&CENTRE).max(0.0);
            let hiz = c.antenna && o.antenna_index(model) == model.hiz_input();
            if o.lnastate as usize >= model.states(hiz, f) {
                return Err(refused(RspSetting::LnaState));
            }
        }
        Ok(())
    }

    fn plan(&self, values: &Values) -> Result<(Options, u32), DriverError> {
        let mut o = Options::from_values(values);
        if o.pinned.is_none() {
            o.pinned = *lock(&self.addr.pinned);
        }
        let rate = plan_rate(&o, values)?;
        self.check_controls(&o, values)?;
        Ok((o, rate))
    }
}

/// Reads the greeting and decides between the RSP commands and the rtl_tcp commands.
fn read_greeting(
    sock: &mut dyn Conn,
    label: &str,
    opts: &Options,
    addr: &AddrState,
    timing: Timing,
    notices: &Notices,
) -> Result<ServerInfo, DriverError> {
    let deadline = Instant::now() + timing.greeting;
    // A server that sends no greeting serves another client.
    let mut b = [0u8; 12];
    read_by(sock, &mut b, deadline).map_err(|_| DriverError::Busy)?;
    let head = Head::parse(&b).ok_or(DriverError::Protocol(Protocol::NotThisRadio))?;
    let compat = ServerInfo {
        extended: false,
        magic: head.magic,
        version: None,
        model: None,
        caps: 0,
        format: SampleFormat::U8,
        tuner: head.w1,
        gains: head.w2,
    };
    let not_extended = || DriverError::driver(RspError::NotExtended);
    let fallback = |why: String| -> Result<ServerInfo, DriverError> {
        if opts.protocol == CommandSet::Extended {
            return Err(not_extended());
        }
        if !addr.fallback_logged.swap(true, Ordering::Relaxed) {
            notices.log(format!(
                "rsp_tcp: {label} greets as rsp_tcp but not with the extended greeting ({why}): using rtl_tcp commands only"
            ));
        }
        Ok(compat.clone())
    };
    if !head.is_rsp() || opts.protocol == CommandSet::Compat {
        if opts.protocol == CommandSet::Extended {
            return Err(not_extended());
        }
        notices.log(format!(
            "rsp_tcp: {label} greets as {} (tuner type {}, {} gains): using rtl_tcp commands",
            head.magic_text(),
            head.w1,
            head.w2
        ));
        return Ok(compat);
    }
    if !head.version_ok() {
        return fallback(format!("version word 0x{:08X}", head.w1));
    }
    let mut t = [0u8; 12];
    read_by(sock, &mut t, deadline).map_err(|_| DriverError::Busy)?;
    let tail = ExtendedTail::parse(&t);
    let Some(format) = SampleFormat::from_word(tail.format) else {
        return fallback(format!("sample format {}", tail.format));
    };
    let model = Model::from_hw(tail.hw);
    let (ma, mi) = proto::version(head.w1);
    notices.log(format!(
        "rsp_tcp: {label} greets as SDRplay {} (rsp_tcp v{ma}.{mi}, {}, capabilities 0x{:X})",
        model.name(),
        format.name(),
        head.w2
    ));
    Ok(ServerInfo {
        extended: true,
        magic: head.magic,
        version: Some((ma, mi)),
        model: Some(model),
        caps: head.w2,
        format,
        tuner: head.w1,
        gains: head.w2,
    })
}

impl Radio for RspRadio {
    type Stream = RspStream;

    fn configure(&mut self, values: &Values, _live: bool) -> Result<String, DriverError> {
        let (o, rate) = self.plan(values)?;
        let mut notes = Vec::new();
        let f = values
            .get(&CENTRE)
            .max(0.0)
            .round()
            .min(f64::from(u32::MAX));
        let model = self.model();
        let ppm = o.ppm as i32;
        let bias = values.get(&BIAS_TEE);
        let agc = values.get(&AGC);
        if self.info.extended {
            let c = self.info.controls();
            let antenna = if c.antenna {
                o.antenna_index(model).unwrap_or(0)
            } else {
                0
            };
            let hiz = model.hiz_input() == Some(antenna) && c.antenna;
            // In this order: AGC, set point, rate, antenna, ppm, frequency, notches, clock output, bias-T, LNA, IF.
            self.send_changed(cmd::AGC, u32::from(agc))?;
            if agc {
                self.send_changed(cmd::AGC_SETPOINT, o.agcsetpoint as i32 as u32)?;
                // The AGC of the radio moves the IF gain reduction: send it again when the AGC goes off.
                self.sent[usize::from(cmd::IFGR)] = None;
            }
            self.send_changed(cmd::RATE, rate)?;
            if c.antenna {
                self.send_changed(cmd::ANTENNA, antenna)?;
            }
            self.send_changed(cmd::PPM, ppm as u32)?;
            self.send_changed(cmd::FREQ, f as u32)?;
            if model.has_notch() && c.any_notch() {
                let mut mask = 0;
                if o.rfnotch {
                    mask |= notch::FM;
                }
                if o.dabnotch {
                    mask |= notch::DAB;
                }
                if o.amnotch {
                    mask |= notch::AM;
                }
                self.send_changed(cmd::NOTCH, mask)?;
            }
            if c.refout && (o.refout || self.refout_on) {
                self.send_changed(cmd::REFOUT, u32::from(o.refout))?;
                self.refout_on = o.refout;
            }
            if c.bias && (bias || self.bias_on) {
                self.send_changed(cmd::BIAS, u32::from(bias))?;
                self.bias_on = bias;
            }
            if c.bias && bias && matches!(model, Model::RspDuo) {
                self.note(
                    &mut notes,
                    "duobias",
                    "rsp_tcp: on the RSPduo, bias-T powers the Tuner 2 input only".into(),
                );
            }
            let gmax = model.gmax(hiz);
            // While the AGC of the radio runs, the gain number sets only the LNA state.
            let gain = values.get(&GAIN);
            let st = split_gain(
                model,
                hiz,
                f,
                gain,
                o.split,
                o.lnastate as i32,
                o.ifgr as i32,
            );
            self.send_changed(cmd::LNA, st.lna_state)?;
            if !agc {
                self.send_changed(cmd::IFGR, st.ifgr)?;
            }
            self.gmax = gmax;
            if hiz && f > 30e6 {
                self.note(
                    &mut notes,
                    "hiz",
                    "rsp_tcp: the Hi-Z input is specified for 1 kHz to 30 MHz; use antenna A or B above that".into(),
                );
            }
        } else {
            // As the rtl_tcp client for another radio: rate, ppm, frequency, bias tee, gain.
            self.send_changed(cmd::RATE, rate)?;
            self.send_changed(cmd::PPM, ppm as u32)?;
            self.send_changed(cmd::FREQ, f as u32)?;
            if bias || self.bias_on {
                self.send_changed(cmd::RTL_BIAS, u32::from(bias))?;
                self.bias_on = bias;
            }
            if !agc {
                let r820t = Greeting {
                    magic: MAGIC_RTL,
                    tuner: 5,
                    gains: 29,
                };
                let target = (values.get(&GAIN) * 10.0).round() as i32;
                let v = nearest_gain(r820t.gain_table(), target);
                self.send_changed(cmd::GAIN, v as u32)?;
            }
        }
        self.opts = o;
        self.centre = f;
        if self.rate != rate {
            self.rate = rate;
            let mut w = lock(&self.addr.window);
            (w.start, w.bytes, w.asked) = (None, 0, Hz::from(rate));
        }
        let (lo, hi) = model.freq_range();
        if !(lo..=hi).contains(&f) {
            let spec = if self.info.extended && model == Model::Rsp1 {
                "the SDRplay RSP1 is specified for 10 kHz to 2 GHz"
            } else {
                "SDRplay RSPs are specified for 1 kHz to 2 GHz"
            };
            self.note(
                &mut notes,
                "range",
                format!(
                    "rsp_tcp: {spec}; the server may not reach {} kHz, or the signal may be weak or absent",
                    (f / 1e3).round()
                ),
            );
        }
        let bits = Hz::from(rate) * self.info.format.bytes() as f64 * 8.0;
        if bits > crate::LINK_WARN_BITS {
            self.note(
                &mut notes,
                "link",
                format!(
                    "rsp_tcp at {} Msps needs about {:.0} Mbit/s; a wired gigabit link is safest, and lost samples show in the status line",
                    sdrkit_driver_core::fmt_m(Hz::from(rate)),
                    bits / 1e6
                ),
            );
        }
        Ok(notes.join("; "))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<RspStream, DriverError> {
        let mut sock = self.sock.try_clone()?;
        abort.register(sock.as_ref());
        let _ = sock.set_read_timeout(Some(self.timing.stall));
        Ok(RspStream {
            sock,
            label: self.label.clone(),
            local: self.local,
            timing: self.timing,
            format: self.info.format,
            addr: self.addr.clone(),
            spare: vec![0u8; READ],
        })
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        self.plan(values).map(|_| ())
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let mut off = Vec::with_capacity(2);
        if self.bias_on {
            off.push(if self.info.extended {
                cmd::BIAS
            } else {
                cmd::RTL_BIAS
            });
        }
        if self.refout_on {
            off.push(cmd::REFOUT);
        }
        for c in off {
            let _ = self.sock.write_all(&proto::command(c, 0));
        }
        self.bias_on = false;
        self.refout_on = false;
        self.sock.shutdown();
    }

    fn rate_hz(&self) -> Hz {
        Hz::from(self.rate)
    }

    fn centre_hz(&self) -> Hz {
        self.centre
    }

    fn sample_type(&self) -> SampleType {
        self.info.format.sample_type()
    }

    fn readings(&mut self, results: &mut Results) {
        self.info.write(results);
        if let Some(t) = *lock(&self.addr.last) {
            results.fields_mut().set(&THROUGHPUT, t.mbit_s);
        }
    }

    fn capabilities(&self) -> Capabilities {
        let pinned = self.opts.pinned.or(*lock(&self.addr.pinned));
        let rates = match pinned {
            Some(p) => {
                let p = Hz::from(p);
                RateLimits {
                    min_hz: p,
                    max_hz: p,
                    ranges: vec![(p, p)],
                    steady_hz: None,
                }
            }
            None => RateLimits {
                min_hz: crate::MIN_RATE,
                max_hz: crate::MAX_RATE,
                ..Default::default()
            },
        };
        let model = self.model();
        let (lo, hi) = if self.info.extended {
            model.freq_range()
        } else {
            (1e3, 2e9)
        };
        let c = self.info.controls();
        Capabilities {
            name: self.info.list_name(&self.label),
            rates,
            min_freq_hz: lo,
            max_freq_hz: hi,
            gain_db: Some(if self.info.extended {
                (0.0, self.gmax, 1.0)
            } else {
                (0.0, 49.6, 0.1)
            }),
            bandwidth_hz: None,
            has_agc: self.info.extended,
            transmit: None,
            has_bias_tee: if self.info.extended { c.bias } else { true },
            realtime: true,
            common_rates_hz: Vec::new(),
            common_bandwidths_hz: Vec::new(),
            gain_steps_db: Vec::new(),
        }
    }

    fn status(&self) -> String {
        match *lock(&self.addr.last) {
            Some(t) => format!(
                "rsp_tcp: {:.1} Mbit/s, {}",
                t.mbit_s,
                sdrkit_driver_core::arrive_text(t.achieved_hz, t.asked_hz)
            ),
            None => String::new(),
        }
    }
}

/// The read loop of one connection. It reads the bytes straight into the ring, whole samples only.
pub struct RspStream {
    sock: Box<dyn Conn>,
    label: String,
    local: bool,
    timing: Timing,
    format: SampleFormat,
    addr: Arc<AddrState>,
    /// Where a read goes when the ring has no room for it.
    spare: Vec<u8>,
}

impl RspStream {
    /// Counts bytes into the throughput window. At the end of a window, it keeps the window and runs the pin detector.
    fn count(&mut self, n: usize, sink: &Sink, ctx: &StreamCtx) {
        let now = Instant::now();
        let bps = self.format.bytes() as f64;
        let reading = {
            let mut w = lock(&self.addr.window);
            w.total += n as u64;
            match w.start {
                None => {
                    (w.start, w.bytes) = (Some(now), 0);
                    None
                }
                Some(s) => {
                    w.bytes += n as u64;
                    let secs = now.duration_since(s).as_secs_f64();
                    if secs >= self.timing.window.as_secs_f64() {
                        let t = Throughput {
                            mbit_s: w.bytes as f64 * 8.0 / secs / 1e6,
                            achieved_hz: w.bytes as f64 / bps / secs,
                            asked_hz: w.asked,
                            total_bytes: w.total,
                            lost_bytes: sink.radio_lost() * self.format.bytes() as u64,
                        };
                        (w.start, w.bytes) = (Some(now), 0);
                        Some(t)
                    } else {
                        None
                    }
                }
            }
        };
        let Some(t) = reading else { return };
        *lock(&self.addr.last) = Some(t);
        let pinned = *lock(&self.addr.pinned);
        let pin = lock(&self.addr.detector).reading(
            t.achieved_hz,
            t.asked_hz,
            t.total_bytes,
            pinned.map(Hz::from),
        );
        if let Some(p) = pin {
            let p = p.round() as u32;
            *lock(&self.addr.pinned) = Some(p);
            let m = sdrkit_driver_core::fmt_m(Hz::from(p));
            ctx.notices.warn(format!(
                "the rsp_tcp server sends {m} Msps whatever it is asked: the radio is set to that rate"
            ));
            ctx.notices
                .store(own_id(RspSetting::ServerRate), Value::Hz(Hz::from(p)));
            ctx.notices.restart();
        }
    }
}

impl RadioStream for RspStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let lost = DriverError::Net(NetError::Closed);
        let mut silent_since: Option<Instant> = None;
        let mut said_waiting = false;
        loop {
            if ctx.stopping() {
                return Ok(());
            }
            match sink.receive(READ, &mut self.spare, |dst| self.sock.read(dst)) {
                Ok(0) => return Err(lost),
                Ok(n) => {
                    silent_since = None;
                    said_waiting = false;
                    self.count(n, sink, ctx);
                }
                Err(e) if timed_out(&e) => {
                    let stall = self.timing.stall;
                    if !self.local {
                        return Err(DriverError::Net(NetError::TimedOut));
                    }
                    let since = *silent_since.get_or_insert_with(|| Instant::now() - stall);
                    if !said_waiting {
                        said_waiting = true;
                        ctx.notices.log(format!(
                            "rsp_tcp: no samples from {} for {} s, still waiting",
                            self.label,
                            stall.as_secs_f64()
                        ));
                    }
                    if since.elapsed() >= self.timing.local_stall {
                        return Err(DriverError::Net(NetError::TimedOut));
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => return Err(lost),
            }
        }
    }
}
