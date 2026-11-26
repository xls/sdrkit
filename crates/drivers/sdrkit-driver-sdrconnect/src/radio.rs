//! One SDRconnect connection run on the shared driver base: connect and open, configure, the read loop and the close.

use crate::proto::{self, event, prop, Binary, Envelope, Tuner, SAMPLE_BYTES};
use crate::{
    caps_for, lock, Entry, Options, RateSource, Report, SdrconnectError, Shared, Throughput, Timing,
};
use crate::{
    ANTENNA_IN_USE, API_VERSION, CAN_CONTROL, DEVICE, IGNORED, LNA_MAX, LNA_MIN, LNA_STATE,
    MALFORMED, MBIT_S, OVERLOAD, RADIO_STARTED, SERVER_CENTRE, SERVER_RATE, SIGNAL_POWER,
    SIGNAL_SNR,
};
use sdrkit_core::net::{NetError, Transport};
use sdrkit_core::Results;
use sdrkit_core::{Hz, SettingDecl, Values};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::ws::{self, tls, Message, ReadError, Reader, Writer};
use sdrkit_driver_core::{Capabilities, DriverError, CENTRE, RATE};
use sdrkit_graph::stream::SampleType;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The properties requested at open.
const OPEN_PROPERTIES: &[&str] = &[
    prop::API,
    prop::DEVICE,
    prop::CAN_CONTROL,
    prop::STARTED,
    prop::RATE,
    prop::CENTRE,
    prop::LNA_MIN,
    prop::LNA_MAX,
    prop::LNA,
    prop::ANTENNAS,
    prop::ANTENNA,
    prop::OVERLOAD,
];

/// How long a report of an earlier requested rate is not treated as a change made by SDRconnect.
const RATE_GRACE: Duration = Duration::from_secs(1);

/// What one text message did.
#[derive(Clone, Debug, Default, PartialEq)]
struct Applied {
    /// The property it set, for the tuner in use.
    property: Option<String>,
}

/// Applies one text message to the entry. Malformed and ignored messages are counted.
fn apply_text(entry: &Shared, tuner: Tuner, text: &str, report: &mut Report) -> Applied {
    let mut e = lock(entry);
    let env = match proto::parse(text) {
        Ok(env) => env,
        Err(_) => {
            e.malformed += 1;
            return Applied::default();
        }
    };
    if env.event_type != event::PROPERTY_CHANGED && env.event_type != event::GET_PROPERTY_RESPONSE {
        e.ignored += 1;
        return Applied::default();
    }
    if env.device != tuner {
        e.ignored += 1;
        return Applied::default();
    }
    if set_property(&mut e, &env, report) {
        Applied {
            property: Some(env.property),
        }
    } else {
        Applied::default()
    }
}

/// Sets the entry's copy of one property. Returns false for a value that does not parse (counted as malformed) or a property
/// the driver does not use (counted as ignored).
fn set_property(e: &mut Entry, env: &Envelope, report: &mut Report) -> bool {
    let v = env.value.as_str();
    let p = &mut e.props;
    let ok = match env.property.as_str() {
        prop::CENTRE => proto::parse_num(v).map(|f| p.centre = Some(f)).is_some(),
        prop::RATE => proto::parse_num(v)
            .filter(|r| *r > 0.0)
            .map(|r| p.rate = Some(r))
            .is_some(),
        prop::LNA => proto::parse_u32(v).map(|n| p.lna = Some(n)).is_some(),
        prop::LNA_MIN | prop::LNA_MAX => match proto::parse_u32(v) {
            Some(n) => {
                let (lo, hi) = report.lna_range.unwrap_or((n, n));
                report.lna_range = Some(if env.property == prop::LNA_MIN {
                    (n, hi.max(n))
                } else {
                    (lo.min(n), n)
                });
                true
            }
            None => false,
        },
        prop::ANTENNAS => {
            report.antennas = Some(proto::split_antennas(v));
            true
        }
        prop::ANTENNA => {
            p.antenna = Some(v.trim().to_string());
            true
        }
        prop::STARTED => proto::parse_bool(v).map(|b| p.started = Some(b)).is_some(),
        prop::CAN_CONTROL => proto::parse_bool(v)
            .map(|b| p.can_control = Some(b))
            .is_some(),
        prop::OVERLOAD => proto::parse_bool(v).map(|b| p.overload = Some(b)).is_some(),
        prop::POWER => proto::parse_num(v).map(|d| p.power = Some(d)).is_some(),
        prop::SNR => proto::parse_num(v).map(|d| p.snr = Some(d)).is_some(),
        prop::DEVICE => {
            // The secondary tuner of an RSPduo reports an empty name.
            report.device = Some(v.trim().to_string()).filter(|d| !d.is_empty());
            true
        }
        prop::API => {
            report.api_version = Some(v.trim().to_string());
            true
        }
        _ => {
            e.ignored += 1;
            return false;
        }
    };
    if !ok {
        e.malformed += 1;
    }
    ok
}

/// What the driver sent and runs on one connection, shared by the control side and the read loop.
#[derive(Debug, Default)]
pub(crate) struct Live {
    /// The driver's rate: the rate it requested, or the rate SDRconnect reported when it changed the rate by itself.
    pub rate: Hz,
    /// The driver's centre.
    pub centre: Hz,
    /// The rate requested before the last one, and the time of the last request (its reports are not a change).
    prev_rate: Option<(Hz, Instant)>,
    /// The rate that SDRconnect did not take. It is not asked again on this connection.
    refused_rate: Option<Hz>,
    /// The centre SDRconnect moved to by itself, once logged.
    moved_logged: Option<u64>,
    /// IQ came on this connection.
    pub iq_seen: bool,
    /// The driver started the radio.
    pub started_radio: bool,
}

/// Where a server is and how the driver waits on it.
pub(crate) struct At {
    pub transport: Arc<dyn Transport>,
    pub host: String,
    pub port: u16,
    pub label: String,
    pub timing: Timing,
}

fn dropped() -> DriverError {
    DriverError::Net(NetError::Closed)
}

/// The radio of one SDRconnect connection, as the base drives it.
pub(crate) struct SdrRadio {
    label: String,
    writer: Arc<Writer>,
    reader: Option<Reader>,
    entry: Shared,
    live: Arc<Mutex<Live>>,
    opts: Options,
    timing: Timing,
    notices: Notices,
    /// The "no control" note was shown on this connection.
    no_control_noted: AtomicBool,
    closed: bool,
}

impl SdrRadio {
    /// Connects, runs the TLS and WebSocket handshakes, chooses the tuner and reads the properties.
    pub(crate) fn connect(
        at: &At,
        entry: Shared,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<SdrRadio, DriverError> {
        let (label, timing) = (at.label.as_str(), &at.timing);
        let known = lock(&entry).report.clone();
        let opts = Options::from_values(values)?;
        if let Some(r) = &known {
            opts.check(r)?;
        }
        // A pinned certificate is the driver's own trust: TLS then runs over the plain connections of the transport.
        let transport: Arc<dyn Transport> = match (opts.wss, opts.pin) {
            (true, Some(p)) => Arc::new(tls::RustlsTransport::new(
                at.transport.clone(),
                &tls::Trust::Pinned(p),
            )?),
            _ => at.transport.clone(),
        };
        let mut conn = net::connect(
            &transport,
            &at.host,
            at.port,
            opts.wss,
            timing.connect,
            abort,
        )?;
        net::recv_buffer(conn.as_mut(), &[16 << 20, 8 << 20, 4 << 20]);
        conn.set_read_timeout(Some(timing.tick))?;
        conn.set_write_timeout(Some(Duration::from_secs(5)))?;
        let link = ws::Link::new(conn, opts.wss)?;
        let up = ws::Upgrade {
            path: "/",
            host: label,
            extra: &[],
            key: timing.fixed_key,
            mask: timing.fixed_mask,
            wait: timing.handshake,
            max_message: crate::MAX_MESSAGE,
            frame_wait: timing.stall,
        };
        let stop = || abort.fired();
        let (mut reader, writer) = ws::handshake(link, &up, &stop)?;
        let t = opts.tuner;
        let send = |ev: &str, p: &str, v: &str, d: Option<Tuner>| {
            writer
                .text(&proto::request(ev, p, v, d))
                .map_err(|_| dropped())
        };
        send(t.enable_event(), "", "true", None)?;
        send(t.other().enable_event(), "", "false", None)?;
        for p in OPEN_PROPERTIES {
            send(event::GET_PROPERTY, p, "", Some(t))?;
        }
        // Read the answers.
        {
            let mut e = lock(&entry);
            e.props = Default::default();
            e.last = None;
        }
        let mut report = known.clone().unwrap_or_default();
        let deadline = Instant::now() + timing.handshake;
        let mut texts = 0;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match reader.read(left, &stop) {
                Ok(Message::Text(b)) => {
                    texts += 1;
                    apply_text(&entry, t, &String::from_utf8_lossy(b), &mut report);
                }
                Ok(Message::Binary(_)) => {}
                Err(ReadError::Timeout) => break,
                Err(e) => return Err(e.into()),
            }
            let e = lock(&entry);
            if e.props.centre.is_some()
                && e.props.rate.is_some()
                && report.lna_range.is_some()
                && report.antennas.is_some()
                && report.api_version.is_some()
            {
                break;
            }
        }
        let fail = |e: DriverError| {
            writer.close();
            e
        };
        if texts == 0 {
            return Err(fail(DriverError::driver(SdrconnectError::NoProperties)));
        }
        if t == Tuner::Secondary {
            if let Some(v) = report.version().filter(|v| *v < proto::DUAL_TUNER_API) {
                return Err(fail(DriverError::driver(SdrconnectError::ApiTooOld {
                    got: v,
                    need: proto::DUAL_TUNER_API,
                })));
            }
        }
        // The settings again, now against what this server reported.
        opts.check(&report).map_err(fail)?;
        lock(&entry).report = Some(report.clone());
        notices.log(format!(
            "SDRconnect: connected to {} at {label} (API {}, {} tuner)",
            report.device.as_deref().unwrap_or("the radio"),
            report.api_version.as_deref().unwrap_or("before 1.0.3"),
            t.name()
        ));
        reader.frame_wait = timing.stall;
        Ok(SdrRadio {
            label: label.to_string(),
            writer,
            reader: Some(reader),
            entry,
            live: Arc::new(Mutex::new(Live::default())),
            opts,
            timing: timing.clone(),
            notices: notices.clone(),
            no_control_noted: AtomicBool::new(false),
            closed: false,
        })
    }

    fn set(&self, property: &str, value: &str) -> Result<(), DriverError> {
        self.writer
            .text(&proto::request(
                event::SET_PROPERTY,
                property,
                value,
                Some(self.opts.tuner),
            ))
            .map_err(|_| dropped())
    }

    /// At open: reads the answers to what was sent, for up to the `confirm` wait, until `done` says they are all in.
    fn await_answers(&mut self, done: &dyn Fn(&Entry) -> bool) {
        let Some(mut reader) = self.reader.take() else {
            return;
        };
        let deadline = Instant::now() + self.timing.confirm;
        let mut report = lock(&self.entry).report.clone().unwrap_or_default();
        while !done(&lock(&self.entry)) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match reader.read(left, &|| false) {
                Ok(Message::Text(b)) => {
                    apply_text(
                        &self.entry,
                        self.opts.tuner,
                        &String::from_utf8_lossy(b),
                        &mut report,
                    );
                }
                Ok(Message::Binary(_)) => {}
                Err(_) => break,
            }
        }
        lock(&self.entry).report = Some(report);
        self.reader = Some(reader);
    }
}

impl Radio for SdrRadio {
    type Stream = SdrStream;

    /// The antenna setting, once SDRconnect listed the antennas of the radio.
    fn settings(&self) -> Vec<SettingDecl> {
        let e = lock(&self.entry);
        match e.report.as_ref().and_then(|r| r.antennas.as_deref()) {
            Some(list) if !list.is_empty() => vec![crate::antenna_setting(list)],
            _ => Vec::new(),
        }
    }

    fn readings(&mut self, results: &mut Results) {
        let e = lock(&self.entry);
        let (p, r) = (&e.props, e.report.as_ref());
        let lna = r.and_then(|r| r.lna_range);
        let mut f = results.fields_mut();
        f.opt(&SIGNAL_POWER, p.power);
        f.opt(&SIGNAL_SNR, p.snr);
        f.opt(&OVERLOAD, p.overload);
        f.opt(&RADIO_STARTED, p.started);
        f.opt(&CAN_CONTROL, p.can_control);
        f.opt(&LNA_STATE, p.lna.map(i64::from));
        f.opt(&LNA_MIN, lna.map(|(lo, _)| i64::from(lo)));
        f.opt(&LNA_MAX, lna.map(|(_, hi)| i64::from(hi)));
        f.opt(&ANTENNA_IN_USE, p.antenna.as_deref());
        f.opt(&DEVICE, r.and_then(|r| r.device.as_deref()));
        f.opt(&API_VERSION, r.and_then(|r| r.api_version.as_deref()));
        f.opt(&SERVER_CENTRE, p.centre);
        f.opt(&SERVER_RATE, p.rate);
        f.opt(&MBIT_S, e.last.map(|t| t.mbit_s));
        f.set(&IGNORED, i64::try_from(e.ignored).unwrap_or(i64::MAX));
        f.set(&MALFORMED, i64::try_from(e.malformed).unwrap_or(i64::MAX));
    }

    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError> {
        self.check(values)?;
        let o = Options::from_values(values)?;
        let mut notes = Vec::new();
        if o.tuner != self.opts.tuner {
            notes.push("SDRconnect: the tuner changes when the radio restarts".to_string());
        }
        let (want_rate, want_centre) = (values.get(&RATE), values.get(&CENTRE));
        let props = lock(&self.entry).props.clone();
        let control = props.can_control != Some(false);
        let mut asked_rate = None;
        let mut asked_centre = None;
        if !control {
            if !self.no_control_noted.swap(true, Ordering::Relaxed) {
                notes.push(
                    "SDRconnect does not give this client control of the radio: it stays where SDRconnect has it".into(),
                );
            }
        } else {
            // The rate.
            if o.rate == RateSource::Stack && want_rate > 0.0 {
                let want = want_rate.round();
                let refused = lock(&self.live)
                    .refused_rate
                    .is_some_and(|r| (r - want).abs() <= 1.0);
                if !refused && props.rate.is_none_or(|r| (r - want).abs() > 1.0) {
                    self.set(prop::RATE, &format!("{want}"))?;
                    asked_rate = Some(want);
                }
            }
            // The centre, the LNA state, the antenna.
            let want = want_centre.max(0.0).round();
            if props.centre.is_none_or(|f| (f - want).abs() > 1.0) {
                self.set(prop::CENTRE, &format!("{want}"))?;
                asked_centre = Some(want);
            }
            if let Some(n) = o.lna.filter(|n| props.lna != Some(*n)) {
                self.set(prop::LNA, &n.to_string())?;
            }
            if let Some(a) = o
                .antenna
                .as_ref()
                .filter(|a| props.antenna.as_ref() != Some(a))
            {
                self.set(prop::ANTENNA, a)?;
            }
        }
        {
            let mut l = lock(&self.live);
            if let Some(r) = asked_rate {
                if (r - l.rate).abs() > 0.5 && l.rate > 0.0 {
                    l.prev_rate = Some((l.rate, Instant::now()));
                }
                l.rate = r;
            } else if l.rate <= 0.0
                || !control
                || o.rate == RateSource::Server
                || l.refused_rate.is_some()
            {
                l.rate = props.rate.unwrap_or(0.0);
            }
            l.centre = match (asked_centre, control) {
                (Some(f), _) => f,
                (None, true) => want_centre.max(0.0).round(),
                (None, false) => props.centre.unwrap_or(want_centre),
            };
            l.moved_logged = None;
        }
        if live {
            // The stream reads the answers. SDRconnect sends none when it keeps its rate (an RSPduo with two tuners runs
            // 2 Msps at most), and samples counted at the rate that was asked play at the wrong speed.
            if let Some(r) = asked_rate {
                let deadline = Instant::now() + self.timing.confirm;
                let taken = |e: &Entry| e.props.rate.is_some_and(|x| (x - r).abs() <= 1.0);
                while !taken(&lock(&self.entry)) && Instant::now() < deadline {
                    std::thread::sleep(self.timing.tick);
                }
            }
        } else if asked_rate.is_some() || asked_centre.is_some() {
            // At open: take what SDRconnect answers.
            self.await_answers(&|e: &Entry| {
                asked_rate.is_none_or(|r| e.props.rate.is_some_and(|x| (x - r).abs() <= 1.0))
                    && asked_centre
                        .is_none_or(|f| e.props.centre.is_some_and(|x| (x - f).abs() <= 1.0))
            });
        }
        if asked_rate.is_some() || (!live && asked_centre.is_some()) {
            let props = lock(&self.entry).props.clone();
            let mut l = lock(&self.live);
            if let (Some(asked), Some(r)) = (asked_rate, props.rate) {
                if (asked - r).abs() > 1.0 {
                    notes.push(format!(
                        "SDRconnect runs {} Msps, {} Msps were asked",
                        sdrkit_driver_core::fmt_m(r),
                        sdrkit_driver_core::fmt_m(asked)
                    ));
                    l.rate = r;
                    l.prev_rate = None;
                    l.refused_rate = Some(asked);
                }
            }
        }
        if lock(&self.live).rate <= 0.0 {
            return Err(DriverError::driver(SdrconnectError::NoRate));
        }
        self.opts = Options {
            tuner: self.opts.tuner,
            ..o
        };
        Ok(notes.join(". "))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<SdrStream, DriverError> {
        let mut reader = self.reader.take().ok_or_else(dropped)?;
        reader.frame_wait = self.timing.stall;
        reader.link().register(abort);
        self.writer
            .text(&proto::request(event::IQ_STREAM_ENABLE, "", "true", None))
            .map_err(|_| dropped())?;
        let started = lock(&self.entry).props.started;
        if started == Some(false) {
            let mut l = lock(&self.live);
            if !l.started_radio {
                self.writer
                    .text(&proto::request(
                        event::DEVICE_STREAM_ENABLE,
                        "",
                        "true",
                        None,
                    ))
                    .map_err(|_| dropped())?;
                l.started_radio = true;
                self.notices
                    .log("SDRconnect: the radio was stopped in SDRconnect, starting it");
            }
        }
        let now = Instant::now();
        Ok(SdrStream {
            reader,
            st: StreamState {
                label: self.label.clone(),
                writer: self.writer.clone(),
                entry: self.entry.clone(),
                live: self.live.clone(),
                tuner: self.opts.tuner,
                timing: self.timing.clone(),
                last_poll: now,
                questions: [prop::POWER, prop::SNR]
                    .map(|p| proto::request(event::GET_PROPERTY, p, "", Some(self.opts.tuner))),
                win_start: None,
                win_bytes: 0,
                iq_seen: false,
            },
        })
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let _ = self
            .writer
            .text(&proto::request(event::IQ_STREAM_ENABLE, "", "false", None));
        if lock(&self.live).started_radio {
            let _ = self.writer.text(&proto::request(
                event::DEVICE_STREAM_ENABLE,
                "",
                "false",
                None,
            ));
        }
        self.writer.close();
        self.reader = None;
    }

    fn rate_hz(&self) -> Hz {
        lock(&self.live).rate
    }

    fn centre_hz(&self) -> Hz {
        lock(&self.live).centre
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cs16
    }

    fn capabilities(&self) -> Capabilities {
        caps_for(&self.label, &lock(&self.entry), &self.opts)
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        let o = Options::from_values(values)?;
        match &lock(&self.entry).report {
            Some(r) => o.check(r),
            None => Ok(()),
        }
    }
}

impl Drop for SdrRadio {
    fn drop(&mut self) {
        self.close();
    }
}

/// The read loop of one SDRconnect connection.
pub(crate) struct SdrStream {
    reader: Reader,
    st: StreamState,
}

/// What the read loop keeps besides the reader.
struct StreamState {
    label: String,
    writer: Arc<Writer>,
    entry: Shared,
    live: Arc<Mutex<Live>>,
    tuner: Tuner,
    timing: Timing,
    last_poll: Instant,
    /// The requests for the signal readings, made once.
    questions: [String; 2],
    win_start: Option<Instant>,
    win_bytes: u64,
    /// IQ came on this connection.
    iq_seen: bool,
}

impl StreamState {
    /// Asks for the signal readings once per poll interval.
    fn poll(&mut self) -> Result<(), DriverError> {
        if self.last_poll.elapsed() < self.timing.poll {
            return Ok(());
        }
        self.last_poll = Instant::now();
        for q in &self.questions {
            self.writer.text(q).map_err(|_| dropped())?;
        }
        Ok(())
    }

    fn on_text(&mut self, msg: &[u8], ctx: &StreamCtx) {
        let mut report = lock(&self.entry).report.clone().unwrap_or_default();
        let before = report.clone();
        let a = apply_text(
            &self.entry,
            self.tuner,
            &String::from_utf8_lossy(msg),
            &mut report,
        );
        if report != before {
            lock(&self.entry).report = Some(report);
        }
        let props = lock(&self.entry).props.clone();
        match a.property.as_deref() {
            Some(prop::RATE) => {
                let Some(r) = props.rate else { return };
                let mut l = lock(&self.live);
                if let Some((old, at)) = l.prev_rate {
                    if at.elapsed() < RATE_GRACE && (r - old).abs() <= 0.5 {
                        return;
                    }
                }
                if (r - l.rate).abs() <= 0.5 {
                    return;
                }
                l.rate = r;
                l.prev_rate = None;
                drop(l);
                let m = sdrkit_driver_core::fmt_m(r);
                ctx.notices
                    .warn(format!("SDRconnect changed its sample rate to {m} Msps"));
                ctx.notices.restart();
            }
            Some(prop::CENTRE) => {
                let Some(f) = props.centre else { return };
                let mut l = lock(&self.live);
                let fv = f.max(0.0).round() as u64;
                if (f - l.centre).abs() > 1.0 && l.moved_logged != Some(fv) {
                    l.moved_logged = Some(fv);
                    ctx.notices.log(format!(
                        "SDRconnect moved the centre to {} MHz, sdrkit keeps tuning {} MHz",
                        sdrkit_driver_core::fmt_m(f),
                        sdrkit_driver_core::fmt_m(l.centre)
                    ));
                }
            }
            _ => {}
        }
    }

    fn on_binary(&mut self, msg: &[u8], sink: &mut Sink, ctx: &StreamCtx) {
        let payload = match proto::binary(msg) {
            Binary::Iq { tuner, payload } if tuner == self.tuner => payload,
            Binary::Short => {
                lock(&self.entry).malformed += 1;
                return;
            }
            _ => {
                lock(&self.entry).ignored += 1;
                return;
            }
        };
        let n = payload.len() / SAMPLE_BYTES;
        if payload.len() % SAMPLE_BYTES != 0 {
            lock(&self.entry).malformed += 1;
        }
        if n == 0 {
            return;
        }
        if !self.iq_seen {
            self.iq_seen = true;
            lock(&self.live).iq_seen = true;
            ctx.notices.log(format!(
                "SDRconnect: IQ is arriving from the {} tuner",
                self.tuner.name()
            ));
        }
        sink.write(&payload[..n * SAMPLE_BYTES]);
        self.count(n * SAMPLE_BYTES);
    }

    /// Counts IQ into the throughput window. The entry is locked only at the end of a window.
    fn count(&mut self, bytes: usize) {
        let now = Instant::now();
        let Some(start) = self.win_start else {
            self.win_start = Some(now);
            self.win_bytes = 0;
            return;
        };
        self.win_bytes += bytes as u64;
        let secs = now.duration_since(start).as_secs_f64();
        if secs < self.timing.window.as_secs_f64() {
            return;
        }
        let t = Throughput {
            mbit_s: self.win_bytes as f64 * 8.0 / secs / 1e6,
        };
        self.win_start = Some(now);
        self.win_bytes = 0;
        lock(&self.entry).last = Some(t);
    }
}

impl RadioStream for SdrStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let stop = || ctx.stopping();
        let tick = self.st.timing.tick;
        let stall = self.st.timing.stall;
        let mut quiet_since = Instant::now();
        loop {
            if ctx.stopping() {
                return Ok(());
            }
            self.st.poll()?;
            // Read in short waits, so the poll timer keeps running while the stream is quiet.
            match self.reader.read(tick, &stop) {
                Ok(Message::Text(b)) => {
                    quiet_since = Instant::now();
                    self.st.on_text(b, ctx);
                }
                Ok(Message::Binary(b)) => {
                    quiet_since = Instant::now();
                    self.st.on_binary(b, sink, ctx);
                }
                Err(ReadError::Stopped) => return Ok(()),
                Err(_) if ctx.stopping() => return Ok(()),
                Err(ReadError::Timeout) => {
                    if quiet_since.elapsed() < stall {
                        continue;
                    }
                    if !self.st.iq_seen {
                        ctx.notices.log(format!(
                            "SDRconnect: no IQ from {} in {} s (is the radio started in SDRconnect?)",
                            self.st.label,
                            stall.as_secs_f64()
                        ));
                    }
                    return Err(DriverError::Net(NetError::TimedOut));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}
