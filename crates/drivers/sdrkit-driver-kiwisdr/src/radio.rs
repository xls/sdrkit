//! One KiwiSDR connection run by the shared driver base: the handshake, the settings, the read loop, the reconnect pauses
//! and the waterfall connection.

use crate::info::{Meters, SMeter, WaterfallLine};
use crate::proto::{
    self, agc_line, ar_ok_line, auth_line, check_fs, covers, fill_samples, khz3, manual_gain,
    mod_line, parse_msg, parse_snd, parse_wf, passband, refusal, rssi_dbm, tag, url_encode,
    Address, Class, SeqStep, SeqTracker, Snd, Tag,
};
use crate::{
    capabilities, class_of, lock, Awaited, KiwiError, KiwiSetting, Known, Passband, Probe, Stall,
    LOCATION, PASSBAND, PASSWORD, SWAP_IQ, TLIMIT_PASSWORD, USERS, USER_NAME, WATERFALL, WF_ZOOM,
};
use sdrkit_core::{Clock, Hz, NetError, Results, SettingSet, Transport, Values};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::ws::{self, ReadError, Reader, Writer};
use sdrkit_driver_core::{
    own_id, Capabilities, DriverError, RadioSetting, AGC, BIAS_TEE, CENTRE, GAIN, RATE, RATE_EXACT,
};
use sdrkit_graph::stream::SampleType;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The timings of the driver.
#[derive(Clone, Debug)]
pub struct Timing {
    /// The pause after the first loss (5 s).
    pub first_pause: Duration,
    /// The longest pause (120 s).
    pub max_pause: Duration,
    /// No IQ for this long ends the stream (8 s).
    pub stall: Duration,
    /// The hand-shake deadline from the auth (8 s). The waterfall waits as long for `wf_setup`.
    pub handshake: Duration,
    /// How often the base tries to reconnect (1 s).
    pub reconnect_gap: Duration,
    /// The connect timeout (4 s).
    pub connect: Duration,
    /// The receive timeout during the hand-shake (2 s).
    pub header_read: Duration,
    /// The receive timeout while streaming (1 s).
    pub stream_read: Duration,
    /// The keep-alive interval (1 s).
    pub keepalive: Duration,
    /// The listener list interval (10 s).
    pub users_every: Duration,
    /// A connection that streamed this long resets the pause (30 s).
    pub long_session: Duration,
    /// A limit token this recent makes a close the time limit (10 s).
    pub limit_window: Duration,
    /// A fixed WebSocket mask, for tests. `None`: a new random mask per frame.
    pub fixed_mask: Option<[u8; 4]>,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            first_pause: Duration::from_secs(5),
            max_pause: Duration::from_secs(120),
            stall: Duration::from_secs(8),
            handshake: Duration::from_secs(8),
            reconnect_gap: Duration::from_secs(1),
            connect: Duration::from_secs(4),
            header_read: Duration::from_secs(2),
            stream_read: Duration::from_secs(1),
            keepalive: Duration::from_secs(1),
            users_every: Duration::from_secs(10),
            long_session: Duration::from_secs(30),
            limit_window: Duration::from_secs(10),
            fixed_mask: None,
        }
    }
}

/// The settings that take effect only at the next connect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RestartPart {
    password: String,
    tlimit_password: String,
    user_name: String,
    location: String,
    waterfall: bool,
}

/// The settings of a radio, read from its values.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Options {
    pub password: String,
    pub tlimit_password: String,
    pub user_name: String,
    pub location: String,
    pub passband: Passband,
    pub agc: bool,
    pub gain: f64,
    pub swap_iq: bool,
    pub users: bool,
    pub waterfall: bool,
    pub wf_zoom: u16,
    pub frequency: Hz,
    pub bias_tee: bool,
    pub rate: Hz,
    pub rate_exact: bool,
}

impl Options {
    pub fn from_values(v: &Values) -> Options {
        Options {
            password: v.get(&PASSWORD).to_owned(),
            tlimit_password: v.get(&TLIMIT_PASSWORD).to_owned(),
            user_name: v.get(&USER_NAME).to_owned(),
            location: v.get(&LOCATION).to_owned(),
            passband: v.get(&PASSBAND),
            agc: v.get(&AGC),
            gain: v.get(&GAIN),
            swap_iq: v.get(&SWAP_IQ),
            users: v.get(&USERS),
            waterfall: v.get(&WATERFALL),
            wf_zoom: u16::try_from(v.get(&WF_ZOOM)).unwrap_or(0),
            frequency: v.get(&CENTRE),
            bias_tee: v.get(&BIAS_TEE),
            rate: v.get(&RATE),
            rate_exact: v.get(&RATE_EXACT),
        }
    }

    pub fn restart_part(&self) -> RestartPart {
        RestartPart {
            password: self.password.clone(),
            tlimit_password: self.tlimit_password.clone(),
            user_name: self.user_name.clone(),
            location: self.location.clone(),
            waterfall: self.waterfall,
        }
    }

    /// The frequency the receiver is tuned to.
    pub fn dial(&self) -> Hz {
        self.frequency
    }

    /// Refuses what no KiwiSDR takes: a name or location that is too long, the bias tee, an exact rate that is not `fs`.
    fn check(&self, fs: Option<f64>) -> Result<(), DriverError> {
        let refuse = |s: KiwiSetting| DriverError::Refused { id: own_id(s) };
        if self.user_name.trim().chars().count() > crate::MAX_USER_NAME {
            return Err(refuse(KiwiSetting::UserName));
        }
        if self.location.trim().chars().count() > crate::MAX_LOCATION {
            return Err(refuse(KiwiSetting::Location));
        }
        if self.bias_tee {
            return Err(DriverError::Refused {
                id: RadioSetting::BiasTee.id(),
            });
        }
        if let Some(fs) = fs {
            if self.rate_exact && self.rate > 0.0 && ((self.rate - fs) / fs).abs() > 1e-4 {
                return Err(DriverError::Refused {
                    id: RadioSetting::Rate.id(),
                });
            }
        }
        Ok(())
    }
}

// ---- The reconnect pauses ----

#[derive(Debug, Default)]
struct SessionState {
    first_done: bool,
    pause: Duration,
    retry: Option<(Instant, DriverError)>,
    stop: Option<DriverError>,
    fs: Option<f64>,
    lost_at: Option<Instant>,
    lost_once: bool,
}

/// One open of the driver: the reconnect pauses and the rate figure.
#[derive(Debug)]
pub(crate) struct Session {
    pub timing: Timing,
    st: Mutex<SessionState>,
}

impl Session {
    pub fn new(timing: Timing) -> Self {
        Session {
            timing,
            st: Mutex::new(SessionState::default()),
        }
    }

    fn st(&self) -> MutexGuard<'_, SessionState> {
        lock(&self.st)
    }

    fn class_floor(&self, class: Class, pause: Duration) -> Duration {
        match class {
            Class::Busy | Class::Hold => {
                pause.max((self.timing.first_pause * 12).min(self.timing.max_pause))
            }
            Class::Retry | Class::Password | Class::Final => pause,
        }
    }

    fn wait(&self, st: &mut SessionState, e: &DriverError, pause: Duration) {
        let class = class_of(e);
        st.pause = self.class_floor(class, pause);
        st.retry = Some((Instant::now() + st.pause, e.clone()));
        if matches!(class, Class::Password | Class::Final) {
            st.stop = Some(e.clone());
        }
    }

    /// A streaming connection was lost with `e` after it streamed for `streamed`.
    pub fn lost(&self, e: &DriverError, streamed: Duration) {
        let mut st = self.st();
        let t = &self.timing;
        let pause = if !st.lost_once || streamed >= t.long_session {
            t.first_pause
        } else {
            (st.pause * 2).min(t.max_pause).max(t.first_pause)
        };
        st.lost_once = true;
        st.lost_at = Some(Instant::now());
        self.wait(&mut st, e, pause);
    }

    /// A reconnect failed with `e`.
    fn failed(&self, e: &DriverError) {
        let mut st = self.st();
        let t = &self.timing;
        let pause = (st.pause * 2).min(t.max_pause).max(t.first_pause);
        self.wait(&mut st, e, pause);
    }
}

/// What every part of one open shares.
pub(crate) struct Ctx {
    pub addr: Address,
    pub label: String,
    pub transport: Arc<dyn Transport>,
    pub clock: Clock,
    pub known: Arc<Known>,
    pub session: Arc<Session>,
}

impl std::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl Ctx {
    fn info(&self) -> MutexGuard<'_, crate::ServerInfo> {
        lock(&self.known.info)
    }
    fn meters(&self) -> &Meters {
        &self.known.meters
    }
    /// The time stamp of the request paths, whole seconds since 1970.
    fn stamp(&self) -> u64 {
        self.clock.now().max(0.0) as u64
    }
}

fn dropped() -> DriverError {
    DriverError::Net(NetError::Closed)
}

/// The opener of the base: the reconnect pauses around [`KiwiRadio::connect`].
pub(crate) fn open_radio(
    ctx: &Arc<Ctx>,
    values: &Values,
    abort: &Arc<Abort>,
    notices: &Notices,
) -> Result<KiwiRadio, DriverError> {
    let s = &ctx.session;
    let (first, lost_at) = {
        let mut st = s.st();
        if let Some(stop) = &st.stop {
            return Err(stop.clone());
        }
        if st.first_done {
            if let Some((until, e)) = &st.retry {
                if Instant::now() < *until {
                    return Err(e.clone());
                }
            }
            (false, st.lost_at)
        } else {
            st.first_done = true;
            (true, None)
        }
    };
    match KiwiRadio::connect(ctx, values, abort, notices) {
        Ok(r) => {
            if let Some(at) = lost_at.filter(|_| !first) {
                notices.log(format!(
                    "KiwiSDR: reconnected after {:.1} s",
                    at.elapsed().as_secs_f64()
                ));
            }
            Ok(r)
        }
        Err(e) => {
            {
                let mut i = ctx.info();
                i.connected = false;
                i.reason = Some(e.clone());
            }
            if !first {
                s.failed(&e);
            }
            Err(e)
        }
    }
}

// ---- The handshake ----

struct Conn {
    reader: Reader,
    writer: Arc<Writer>,
    sample_rate: Option<String>,
}

/// Receive timeouts in a row that a frame can take between its bytes.
const FRAME_TIMEOUTS: u32 = 10;

/// Connects, upgrades on `path`, authenticates and reads messages until `until` comes.
fn handshake(
    ctx: &Ctx,
    path: &str,
    auth: &str,
    has_pw: bool,
    abort: &Arc<Abort>,
    until: Awaited,
) -> Result<Conn, DriverError> {
    let t = &ctx.session.timing;
    let mut conn = net::connect(
        &ctx.transport,
        &ctx.addr.host,
        ctx.addr.port,
        false,
        t.connect,
        abort,
    )?;
    net::recv_buffer(conn.as_mut(), &[1 << 20, 256 << 10]);
    let link = ws::Link::new(conn, false)?;
    link.set_read_timeout(Some(t.header_read))?;
    link.register(abort);
    let up = ws::Upgrade {
        path,
        host: &ctx.label,
        extra: &[],
        key: None,
        mask: t.fixed_mask,
        wait: t.header_read,
        max_message: ws::MAX_MESSAGE,
        frame_wait: t.header_read * FRAME_TIMEOUTS,
    };
    let (mut reader, writer) = ws::handshake(link, &up, &|| abort.fired())?;
    writer.text(auth).map_err(|_| dropped())?;
    let deadline = Instant::now() + t.handshake;
    let mut limit_seen = false;
    let mut sample_rate = None;
    loop {
        if abort.fired() {
            return Err(DriverError::Net(NetError::Aborted));
        }
        if Instant::now() >= deadline {
            return Err(DriverError::driver(KiwiError::Incomplete {
                awaited: until,
            }));
        }
        // Each read waits at most the receive timeout, and never past the deadline.
        let left = deadline.saturating_duration_since(Instant::now());
        reader
            .link()
            .set_read_timeout(Some(t.header_read.min(left).max(Duration::from_millis(1))))?;
        let m = match reader.poll_message() {
            Ok(None) => continue,
            Ok(Some(m)) => m.bytes(),
            Err(_) if limit_seen => return Err(DriverError::driver(KiwiError::TimeLimit)),
            Err(_) => return Err(dropped()),
        };
        if tag(m) != Tag::Msg {
            continue;
        }
        let mut done = false;
        for tk in parse_msg(m).unwrap_or_default() {
            if let Some(e) = refusal(&tk, has_pw) {
                return Err(DriverError::driver(e));
            }
            if matches!(tk.name.as_str(), "inactivity_timeout" | "ip_limit") {
                limit_seen = true;
            }
            ctx.info().note(&tk);
            match (tk.name.as_str(), until) {
                ("audio_rate", Awaited::SampleRate) => {
                    if let Some(line) = ar_ok_line(&tk.value) {
                        writer.text(&line).map_err(|_| dropped())?;
                    }
                }
                ("sample_rate", Awaited::SampleRate) => {
                    sample_rate = Some(tk.value.clone());
                    done = true;
                }
                ("wf_setup", Awaited::WfSetup) => done = true,
                _ => {}
            }
        }
        if done {
            return Ok(Conn {
                reader,
                writer,
                sample_rate,
            });
        }
    }
}

/// Connects with `password`, reads up to the IQ rate, then closes.
pub(crate) fn probe(ctx: &Ctx, password: &str) -> Result<Probe, DriverError> {
    let abort = Abort::new();
    let c = handshake(
        ctx,
        &format!("/kiwi/{}/SND", ctx.stamp()),
        &auth_line(password, ""),
        !password.is_empty(),
        &abort,
        Awaited::SampleRate,
    )?;
    let res = check_fs(c.sample_rate.as_deref().unwrap_or(""));
    c.writer.close();
    res.map_err(DriverError::driver)?;
    let i = ctx.info();
    Ok(Probe {
        version: i.version_maj.zip(i.version_min),
        channels: i.rx_chans,
    })
}

// ---- The radio ----

/// What the stream reads from the control side at each frame.
#[derive(Debug, Default)]
pub(crate) struct Live {
    swap: AtomicBool,
    users: AtomicBool,
    users_now: AtomicBool,
}

/// The waterfall connection.
struct Waterfall {
    stop: Arc<AtomicBool>,
    abort: Arc<Abort>,
    shared: Arc<WfShared>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Debug, Default)]
struct WfShared {
    /// The zoom line currently requested.
    zoom_line: Mutex<String>,
    /// The writer of the waterfall once it is set up, and the zoom line last sent on it.
    writer: Mutex<Option<(Arc<Writer>, String)>>,
}

impl WfShared {
    fn want(&self, line: String) {
        lock(&self.zoom_line).clone_from(&line);
        let mut w = lock(&self.writer);
        if let Some((wr, sent)) = w.as_mut() {
            if *sent != line && wr.text(&line).is_ok() {
                *sent = line;
            }
        }
    }
}

fn zoom_line(zoom: u16, dial: Hz, range: (Hz, Hz)) -> String {
    let cf = if zoom == 0 {
        (range.0 - proto::MIN_DIAL_HZ + range.1) / 2.0
    } else {
        dial
    };
    format!("SET zoom={zoom} cf={}", khz3(cf))
}

/// One connection to a KiwiSDR.
pub struct KiwiRadio {
    ctx: Arc<Ctx>,
    writer: Arc<Writer>,
    reader: Option<Reader>,
    /// The rate this connection reported.
    fs: f64,
    /// The rate figure of the session: kept unless the rate moves by more than 0.01 %.
    fs_figure: f64,
    dial: Hz,
    sent_mod: Option<(i64, u32)>,
    sent_agc: Option<Option<i32>>,
    live: Arc<Live>,
    opts: Options,
    notices: Notices,
    stamp: u64,
    auth: String,
    wf: Option<Waterfall>,
    closed: bool,
    listeners: Vec<crate::Listener>,
    line: Option<u32>,
}

impl std::fmt::Debug for KiwiRadio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KiwiRadio")
            .field("label", &self.ctx.label)
            .field("fs", &self.fs)
            .finish()
    }
}

impl KiwiRadio {
    fn connect(
        ctx: &Arc<Ctx>,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<KiwiRadio, DriverError> {
        let o = Options::from_values(values);
        o.check(None)?;
        let stamp = ctx.stamp();
        let auth = auth_line(&o.password, &o.tlimit_password);
        let c = handshake(
            ctx,
            &format!("/kiwi/{stamp}/SND"),
            &auth,
            !o.password.is_empty(),
            abort,
            Awaited::SampleRate,
        )?;
        let fail = |e: DriverError| {
            c.writer.close();
            e
        };
        let fs = check_fs(c.sample_rate.as_deref().unwrap_or(""))
            .map_err(|e| fail(DriverError::driver(e)))?;
        let mut lines = vec![format!("SET ident_user={}", url_encode(o.user_name.trim()))];
        if !o.location.trim().is_empty() {
            lines.push(format!("SET geoloc={}", url_encode(o.location.trim())));
        }
        lines.extend(
            [
                "SET compression=0",
                "SET squelch=0 max=0",
                "SET genattn=0",
                "SET gen=0 mix=-1",
            ]
            .map(String::from),
        );
        for line in &lines {
            c.writer.text(line).map_err(|_| fail(dropped()))?;
        }
        let fs_figure = {
            let mut st = ctx.session.st();
            let fig = match st.fs {
                Some(old) if ((fs - old) / old).abs() <= 1e-4 => old,
                Some(_) => {
                    notices.log(format!(
                        "KiwiSDR: the receiver now reports an IQ rate of {fs} Hz"
                    ));
                    notices.restart();
                    fs
                }
                None => fs,
            };
            st.fs = Some(fig);
            fig
        };
        let version = {
            let mut i = ctx.info();
            i.sample_rate = Some(fs);
            i.reason = None;
            i.version()
        };
        notices.log(match version {
            Some(v) => format!("KiwiSDR: connected to {}, version {v}", ctx.label),
            None => format!("KiwiSDR: connected to {}", ctx.label),
        });
        let live = Arc::new(Live::default());
        live.swap.store(o.swap_iq, Ordering::Release);
        live.users.store(o.users, Ordering::Release);
        Ok(KiwiRadio {
            ctx: ctx.clone(),
            writer: c.writer,
            reader: Some(c.reader),
            fs,
            fs_figure,
            dial: o.dial(),
            sent_mod: None,
            sent_agc: None,
            live,
            opts: o,
            notices: notices.clone(),
            stamp,
            auth,
            wf: None,
            closed: false,
            listeners: Vec::new(),
            line: None,
        })
    }

    fn send(&self, line: &str) -> Result<(), DriverError> {
        self.writer.text(line).map_err(|_| dropped())
    }

    fn stop_waterfall(&mut self) {
        if let Some(mut wf) = self.wf.take() {
            wf.stop.store(true, Ordering::Release);
            if let Some((w, _)) = lock(&wf.shared.writer).take() {
                w.close();
            }
            wf.abort.fire();
            if let Some(t) = wf.thread.take() {
                let _ = t.join();
            }
        }
    }

    fn start_waterfall(&mut self) {
        let stop = Arc::new(AtomicBool::new(false));
        let abort = Abort::new();
        let shared = Arc::new(WfShared::default());
        let range = self.ctx.info().covered();
        *lock(&shared.zoom_line) = zoom_line(self.opts.wf_zoom, self.dial, range);
        let wf = WfThread {
            ctx: self.ctx.clone(),
            auth: self.auth.clone(),
            stamp: self.stamp,
            has_pw: !self.opts.password.is_empty(),
            stop: stop.clone(),
            abort: abort.clone(),
            shared: shared.clone(),
            notices: self.notices.clone(),
        };
        let thread = std::thread::Builder::new()
            .name("sdrkit-kiwi-wf".into())
            .spawn(move || wf.run())
            .ok();
        self.wf = Some(Waterfall {
            stop,
            abort,
            shared,
            thread,
        });
    }
}

impl Radio for KiwiRadio {
    fn readings(&mut self, results: &mut Results) {
        let i = self.ctx.info();
        crate::write_readings(&i, self.ctx.meters(), results);
        if i.listeners != self.listeners {
            self.listeners.clone_from(&i.listeners);
            crate::write_listeners(&self.listeners, results);
        }
        let newest = i.waterfall.back().map(|l| l.seq);
        if newest != self.line {
            self.line = newest;
            crate::write_waterfall(&i, results);
        }
    }

    type Stream = KiwiStream;

    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError> {
        let o = Options::from_values(values);
        o.check(Some(self.fs))?;
        let range = self.ctx.info().covered();
        let dial = o.dial();
        if !covers(range, dial) {
            return Err(DriverError::Refused {
                id: RadioSetting::Centre.id(),
            });
        }
        let p = passband(o.passband, self.fs).ok_or(DriverError::Refused {
            id: own_id(KiwiSetting::Passband),
        })?;
        self.live.swap.store(o.swap_iq, Ordering::Release);
        self.live.users.store(o.users, Ordering::Release);
        if live && o.users && !self.opts.users {
            self.live.users_now.store(true, Ordering::Release);
        }
        let m = (dial.round() as i64, p);
        if self.sent_mod != Some(m) {
            self.send(&mod_line(dial, p))?;
            self.sent_mod = Some(m);
        }
        let agc = (!o.agc).then(|| manual_gain(o.gain));
        if self.sent_agc != Some(agc) {
            self.send(&agc_line(agc))?;
            self.sent_agc = Some(agc);
        }
        self.dial = dial.round();
        if let Some(wf) = &self.wf {
            wf.shared.want(zoom_line(o.wf_zoom, dial, range));
        }
        self.opts = o;
        Ok(String::new())
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        Options::from_values(values).check(Some(self.fs))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<KiwiStream, DriverError> {
        let mut reader = self.reader.take().ok_or_else(dropped)?;
        reader.link().register(abort);
        let t = self.ctx.session.timing.clone();
        reader.link().set_read_timeout(Some(t.stream_read))?;
        reader.frame_wait = t.stream_read * FRAME_TIMEOUTS;
        self.send("SET keepalive")?;
        let now = Instant::now();
        if self.opts.users {
            self.send("SET GET_USERS")?;
        }
        if self.opts.waterfall {
            self.start_waterfall();
        }
        {
            let mut i = self.ctx.info();
            i.connected = true;
            i.reason = None;
        }
        // The throughput window counts the stream only, not the messages of the hand-shake.
        let bytes0 = reader.payload_in;
        Ok(KiwiStream {
            reader,
            st: StreamState {
                ctx: self.ctx.clone(),
                writer: self.writer.clone(),
                fs: self.fs,
                fs_figure: self.fs_figure,
                live: self.live.clone(),
                seq: SeqTracker::default(),
                smeter: SMeter::default(),
                has_pw: !self.opts.password.is_empty(),
                timing: t,
                started: now,
                last_keepalive: now,
                last_users: now,
                last_iq: now,
                gps_tagged: None,
                msgs_since_iq: 0,
                audio_since_iq: 0,
                last_any: now,
                bad: 0,
                limit_at: None,
                win_start: now,
                win_bytes: bytes0,
                win_iq: 0,
                notices: self.notices.clone(),
            },
        })
    }

    fn stream_ended(&mut self) {
        self.stop_waterfall();
    }

    fn close(&mut self) {
        self.stop_waterfall();
        if self.closed {
            return;
        }
        self.closed = true;
        self.writer.close();
        self.reader = None;
        self.ctx.info().connected = false;
    }

    fn rate_hz(&self) -> Hz {
        self.fs_figure
    }

    fn centre_hz(&self) -> Hz {
        self.dial
    }

    fn capabilities(&self) -> Capabilities {
        let (lo, hi) = self.ctx.info().covered();
        capabilities(&self.ctx.label, self.fs_figure, lo, hi)
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cs16
    }

    fn status(&self) -> String {
        crate::status_line(&self.ctx.info(), self.ctx.meters())
    }
}

// ---- The stream ----

/// The read loop of one connection.
pub struct KiwiStream {
    reader: Reader,
    st: StreamState,
}

struct StreamState {
    ctx: Arc<Ctx>,
    writer: Arc<Writer>,
    fs: f64,
    fs_figure: f64,
    live: Arc<Live>,
    seq: SeqTracker,
    smeter: SMeter,
    has_pw: bool,
    timing: Timing,
    started: Instant,
    last_keepalive: Instant,
    last_users: Instant,
    last_iq: Instant,
    gps_tagged: Option<Instant>,
    msgs_since_iq: u64,
    audio_since_iq: u64,
    last_any: Instant,
    bad: u64,
    limit_at: Option<Instant>,
    win_start: Instant,
    win_bytes: u64,
    win_iq: u64,
    notices: Notices,
}

fn zeros(dst: &mut [MaybeUninit<u8>], _from: usize) {
    dst.fill(MaybeUninit::new(0));
}

impl StreamState {
    fn send(&mut self, line: &str) -> Result<(), DriverError> {
        self.writer.text(line).map_err(|_| dropped())
    }

    fn bad(&mut self) {
        self.bad += 1;
        Meters::add(&self.ctx.meters().bad, 1);
    }

    fn end_reason(&self, e: ReadError) -> DriverError {
        if e != ReadError::Closed {
            return e.into();
        }
        let (limit, inactivity) = {
            let i = self.ctx.info();
            (i.limit_min(), i.inactivity_min)
        };
        let recent = self
            .limit_at
            .is_some_and(|t| t.elapsed() <= self.timing.limit_window);
        let ran_out = inactivity.is_some_and(|m| {
            m > 0
                && self.started.elapsed()
                    >= Duration::from_secs(u64::from(m) * 60)
                        .saturating_sub(Duration::from_secs(60))
        });
        DriverError::driver(if recent || ran_out {
            KiwiError::TimeLimit
        } else {
            KiwiError::Closed { limit_min: limit }
        })
    }

    fn stall_reason(&self) -> DriverError {
        let kind = if self.msgs_since_iq == 0 || self.last_any.elapsed() >= self.timing.stall {
            Stall::Silent
        } else if self.audio_since_iq > 0 {
            Stall::AudioOnly
        } else {
            Stall::MessagesOnly
        };
        DriverError::driver(KiwiError::Stalled {
            kind,
            seconds: self.timing.stall.as_secs(),
        })
    }

    /// The timers before each read: keep-alive, listener list, S-meter, stall, throughput.
    fn tick(&mut self, bytes: u64) -> Result<(), DriverError> {
        let now = Instant::now();
        if now.duration_since(self.last_keepalive) >= self.timing.keepalive {
            self.send("SET keepalive")?;
            self.last_keepalive = now;
        }
        let users = self.live.users.load(Ordering::Acquire);
        if self.live.users_now.swap(false, Ordering::AcqRel)
            || (users && now.duration_since(self.last_users) >= self.timing.users_every)
        {
            self.send("SET GET_USERS")?;
            self.last_users = now;
        }
        if let Some((d, m)) = self.smeter.due(now) {
            self.ctx.meters().set_rssi(d, m);
        }
        if now.duration_since(self.last_iq) >= self.timing.stall {
            return Err(self.stall_reason());
        }
        let secs = now.duration_since(self.win_start).as_secs_f64();
        if secs >= 1.0 {
            let b = bytes - self.win_bytes;
            self.ctx.meters().set_link(
                b as f64 * 8.0 / secs / 1e6,
                self.win_iq as f64 / secs / self.fs,
            );
            self.win_start = now;
            self.win_bytes = bytes;
            self.win_iq = 0;
        }
        Ok(())
    }

    fn handle(&mut self, m: &[u8], sink: &mut Sink) -> Result<(), DriverError> {
        self.msgs_since_iq += 1;
        self.last_any = Instant::now();
        match tag(m) {
            Tag::Msg => self.handle_msg(m),
            Tag::Snd => {
                match parse_snd(m) {
                    // A compressed frame is audio, never IQ: bad, and counted for the stall reason.
                    Err(proto::BadFrame::CompressedIq) => {
                        self.bad();
                        self.audio_since_iq += 1;
                    }
                    Err(_) => self.bad(),
                    Ok(Snd::Audio { .. }) => {
                        self.audio_since_iq += 1;
                        Meters::add(&self.ctx.meters().not_iq, 1);
                    }
                    Ok(Snd::Iq(f)) => self.handle_iq(&f, sink),
                }
                Ok(())
            }
            Tag::Wf | Tag::Ext => {
                Meters::add(&self.ctx.meters().not_iq, 1);
                Ok(())
            }
            Tag::Bad => {
                self.bad();
                Ok(())
            }
        }
    }

    fn handle_msg(&mut self, m: &[u8]) -> Result<(), DriverError> {
        for tk in parse_msg(m).unwrap_or_default() {
            if let Some(e) = refusal(&tk, self.has_pw) {
                return Err(DriverError::driver(e));
            }
            match tk.name.as_str() {
                "inactivity_timeout" | "ip_limit" => self.limit_at = Some(Instant::now()),
                "audio_rate" => {
                    if let Some(line) = ar_ok_line(&tk.value) {
                        self.send(&line)?;
                    }
                }
                "sample_rate" => {
                    if let Ok(fs) = check_fs(&tk.value) {
                        if ((fs - self.fs_figure) / self.fs_figure).abs() > 1e-4 {
                            self.notices.log(format!(
                                "KiwiSDR: the receiver now reports an IQ rate of {fs} Hz"
                            ));
                            self.notices.restart();
                        }
                    }
                }
                _ => {}
            }
            self.ctx.info().note(&tk);
        }
        Ok(())
    }

    fn handle_iq(&mut self, f: &proto::IqFrame<'_>, sink: &mut Sink) {
        if f.odd {
            self.bad();
        }
        let n = f.len();
        let meters = self.ctx.meters();
        match self.seq.step(f.seq) {
            SeqStep::Repeat => {
                self.bad();
                return;
            }
            SeqStep::Gap(d) => {
                Meters::add(&meters.missed, u64::from(d));
                Meters::add(&meters.discontinuities, 1);
                sink.write_with(fill_samples(d, n, self.fs), zeros);
            }
            SeqStep::First | SeqStep::InOrder | SeqStep::Restart => {}
        }
        // A change of the swap takes effect at the frame boundary.
        let swap = self.live.swap.load(Ordering::Acquire);
        if f.is_native(swap) {
            sink.write(f.data);
        } else {
            sink.write_with(n, |dst, from| f.write_cs16(from, swap, dst));
        }
        let now = Instant::now();
        self.last_iq = now;
        self.msgs_since_iq = 0;
        self.audio_since_iq = 0;
        self.win_iq += n as u64;
        Meters::add(&meters.frames, 1);
        if let Some((d, m)) = self.smeter.reading(rssi_dbm(f.smeter), now) {
            meters.set_rssi(d, m);
        }
        if f.overflow() {
            meters.overloaded(now);
        }
        let valid = f.gps.valid();
        meters.gps_valid.store(valid, Ordering::Relaxed);
        if valid
            && self
                .gps_tagged
                .is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1))
        {
            meters.set_gps_unix(proto::gps_to_unix(
                f.gps.seconds,
                f.gps.nanos,
                self.ctx.clock.now(),
            ));
            self.gps_tagged = Some(now);
        }
    }
}

impl KiwiStream {
    fn run_inner(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        loop {
            if ctx.stopping() || sink.closed() {
                return Ok(());
            }
            self.st.tick(self.reader.payload_in)?;
            match self.reader.poll_message() {
                Ok(None) => {}
                Ok(Some(m)) => self.st.handle(m.bytes(), sink)?,
                Err(e) => return Err(self.st.end_reason(e)),
            }
        }
    }
}

impl RadioStream for KiwiStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let res = self.run_inner(sink, ctx);
        let bad = std::mem::take(&mut self.st.bad);
        if bad > 0 {
            ctx.notices
                .log(format!("KiwiSDR: {bad} malformed frames ignored"));
        }
        self.st.ctx.info().connected = false;
        if ctx.stopping() {
            return Ok(());
        }
        let e = res.err().unwrap_or_else(dropped);
        self.st.ctx.session.lost(&e, self.st.started.elapsed());
        self.st.ctx.info().reason = Some(e.clone());
        self.st.writer.close();
        Err(e)
    }
}

// ---- The waterfall ----

/// The thread of the waterfall connection.
struct WfThread {
    ctx: Arc<Ctx>,
    auth: String,
    stamp: u64,
    has_pw: bool,
    stop: Arc<AtomicBool>,
    abort: Arc<Abort>,
    shared: Arc<WfShared>,
    notices: Notices,
}

impl WfThread {
    fn run(self) {
        let ctx = &self.ctx;
        let c = match handshake(
            ctx,
            &format!("/kiwi/{}/W/F", self.stamp),
            &self.auth,
            self.has_pw,
            &self.abort,
            Awaited::WfSetup,
        ) {
            Ok(c) => c,
            Err(e) => {
                if !self.stop.load(Ordering::Acquire) {
                    self.notices
                        .log(format!("KiwiSDR: the waterfall is not available: {e}"));
                    ctx.info().waterfall_problem = Some(e);
                }
                return;
            }
        };
        let Conn {
            mut reader, writer, ..
        } = c;
        let t = &ctx.session.timing;
        if reader.link().set_read_timeout(Some(t.stream_read)).is_err() {
            return;
        }
        reader.frame_wait = t.stream_read * FRAME_TIMEOUTS;
        let zoom = lock(&self.shared.zoom_line).clone();
        for line in [
            zoom.as_str(),
            "SET maxdb=-10 mindb=-110",
            "SET wf_speed=1",
            "SET wf_comp=0",
            "SET keepalive",
        ] {
            if writer.text(line).is_err() {
                return;
            }
        }
        *lock(&self.shared.writer) = Some((writer.clone(), zoom));
        ctx.info().waterfall_problem = None;
        let mut last_keepalive = Instant::now();
        while !self.stop.load(Ordering::Acquire) {
            if last_keepalive.elapsed() >= t.keepalive {
                if writer.text("SET keepalive").is_err() {
                    break;
                }
                last_keepalive = Instant::now();
            }
            match reader.poll_message() {
                Ok(None) => {}
                Ok(Some(m)) => {
                    if let Ok(f) = parse_wf(m.bytes()) {
                        let mut i = ctx.info();
                        let bw = i
                            .bandwidth_hz
                            .filter(|b| *b > 1e6 && *b < 1e9)
                            .unwrap_or(proto::MAX_TOP_HZ);
                        let mut line = if i.waterfall.len() >= crate::info::WATERFALL_ROWS {
                            i.waterfall.pop_front().unwrap_or_default()
                        } else {
                            WaterfallLine::default()
                        };
                        line.start_hz = f.start_hz(bw) + i.freq_offset_hz;
                        line.span_hz = f.span_hz(bw);
                        line.zoom = f.zoom;
                        line.seq = f.seq;
                        line.levels_db.clear();
                        line.levels_db
                            .extend(f.pixels.iter().map(|&p| proto::pixel_db(p)));
                        i.add_line(line);
                    }
                }
                Err(_) => break,
            }
        }
        lock(&self.shared.writer).take();
        writer.close();
    }
}
