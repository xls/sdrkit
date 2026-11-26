//! The shared base of the network radios: open with retries, the radio thread, reconnect with the last settings, settings that
//! arrive while the radio is away, a live change or a restart, and the sample-loss count. A driver returns an [`Opener`] and a
//! [`Radio`]. The radio's lock is held for a configure or a close, never across an open.

use crate::loss::LossDetector;
use crate::net::Abort;
use crate::{
    Capabilities, DriverError, Link, LossReport, Polled, Retuned, RingWriter, Source, Started,
    CENTRE, RATE,
};
use sdrkit_core::{
    Hz, Results, SettingDecl, SettingId, SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::stream::SampleType;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// An open radio's control side, as the base drives it.
pub trait Radio: Send + 'static {
    /// The read loop this radio streams with.
    type Stream: RadioStream;
    /// Applies `values`. `live`: the stream runs, so the radio sends only what changed. Not live: no stream runs (at open,
    /// and for a rate change on a radio that cannot change the rate while it streams). It returns a line for the log, or "".
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the radio refuses.
    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError>;
    /// Starts streaming. The connections the stream waits on are registered with `abort`, so the base can stop it.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the stream cannot start.
    fn stream(&mut self, abort: &Arc<Abort>) -> Result<Self::Stream, DriverError>;
    /// The stream ended (it was interrupted for a non-live configure, or lost): free what it held.
    fn stream_ended(&mut self) {}
    /// Closes the radio. Must be safe to call twice.
    fn close(&mut self);
    /// The rate the radio runs now (what it confirmed, or what was sent when it confirms nothing).
    fn rate_hz(&self) -> Hz;
    /// The centre frequency it is tuned to now.
    fn centre_hz(&self) -> Hz;
    /// What it can do.
    fn capabilities(&self) -> Capabilities;
    /// How one sample is stored in the ring. It changes only at an open, with a setting that sets the sample format.
    fn sample_type(&self) -> SampleType;
    /// The rate can change while streaming. When not, a rate change stops the stream, configures, and starts it again.
    fn live_rate_change(&self) -> bool {
        true
    }
    /// Refuses a change before the stream is touched.
    ///
    /// # Errors
    ///
    /// [`DriverError`] for a change the radio cannot take.
    fn check(&self, values: &Values) -> Result<(), DriverError> {
        let _ = values;
        Ok(())
    }
    /// A line for the status strip.
    fn status(&self) -> String {
        String::new()
    }
    /// Writes the device's readings ([`DriverDescriptor::readings`](crate::DriverDescriptor)) into `results`. The base calls
    /// it when the host polls. The radio can keep what it wrote last, to write only what changed. The default writes none.
    fn readings(&mut self, results: &mut Results) {
        let _ = results;
    }
    /// The settings that the radio reveals when it opens ([`Source::settings`]). The default adds none.
    fn settings(&self) -> Vec<SettingDecl> {
        Vec::new()
    }
    /// What the radio keeps across a restart ([`Source::save_state`]). The default keeps nothing.
    fn save_state(&self) -> Option<StateBlob> {
        None
    }
    /// Restores the radio from what [`Radio::save_state`] kept.
    ///
    /// # Errors
    ///
    /// [`StateError`] if the state is not the radio's. The default returns [`StateError::NoState`].
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let _ = state;
        Err(StateError::NoState)
    }
    /// Writes `gain_db` as the manual gain of receiver `rx` for the held gain of the driver ([`crate::gain`]). The stream calls
    /// it through [`StreamCtx::write_gain`], never during a configure. The default refuses.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the radio refuses or the link fails.
    fn write_held_gain(&mut self, rx: usize, gain_db: f64) -> Result<(), DriverError> {
        let _ = (rx, gain_db);
        Err(DriverError::Refused {
            id: crate::RadioSetting::Gain.id(),
        })
    }
}

/// A radio's blocking read loop.
pub trait RadioStream: Send + 'static {
    /// Reads and writes samples into `sink` until the stream ends: the link dropped (`Err` with the reason), or the base stopped it
    /// through the stream's [`Abort`] (the result is then not looked at).
    ///
    /// # Errors
    ///
    /// [`DriverError`] for the reason the link dropped.
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError>;
}

/// Makes a radio's connection: connect, make sure that it is the right kind of radio, read what it is. The base then
/// configures it. The connections it waits on are registered with the [`Abort`], so a stop can cut it short.
pub type Opener<R> =
    Arc<dyn Fn(&Values, &Arc<Abort>, &Notices) -> Result<R, DriverError> + Send + Sync>;

/// What a radio's threads tell the host: log lines (the oldest go past 256), the last warning, a setting to store, a restart.
#[derive(Clone, Debug, Default)]
pub struct Notices(Arc<Mutex<Pending>>);

#[derive(Debug, Default)]
struct Pending {
    lines: VecDeque<String>,
    warning: Option<String>,
    store: Option<(SettingId, Value)>,
    restart: bool,
}

impl Notices {
    fn lock(&self) -> MutexGuard<'_, Pending> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Adds a log line.
    pub fn log(&self, line: impl Into<String>) {
        let mut q = self.lock();
        if q.lines.len() >= 256 {
            q.lines.pop_front();
        }
        q.lines.push_back(line.into());
    }
    /// Adds a log line that the status shows too.
    pub fn warn(&self, line: impl Into<String>) {
        let line = line.into();
        self.lock().warning = Some(line.clone());
        self.log(line);
    }
    /// Asks the host to store `value` as the value of setting `id`.
    pub fn store(&self, id: SettingId, value: Value) {
        self.lock().store = Some((id, value));
    }
    /// Asks the host to stop the stream and start it again.
    pub fn restart(&self) {
        self.lock().restart = true;
    }
    /// Writes what is queued into `results` and returns the requests.
    pub fn drain_into(&self, results: &mut Results) -> Polled {
        let mut q = self.lock();
        for line in q.lines.drain(..) {
            results.log(line);
        }
        if let Some(w) = q.warning.take() {
            results.update_status(|s| {
                s.summary = w;
                true
            });
        }
        Polled {
            capabilities: false,
            restart: std::mem::take(&mut q.restart),
            store: q.store.take(),
        }
    }
}

/// Writes a held gain through the radio, if the radio is free: `None` when it is busy.
type GainWrite = Arc<dyn Fn(usize, f64) -> Option<Result<(), DriverError>> + Send + Sync>;

/// What a stream's read loop may look at.
#[derive(Clone)]
pub struct StreamCtx {
    stopping: Arc<AtomicBool>,
    /// A planned restart asks the stream to end at its next boundary.
    ending: Arc<AtomicBool>,
    /// Where log lines and requests to the host go.
    pub notices: Notices,
    gain: Option<GainWrite>,
}

impl std::fmt::Debug for StreamCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamCtx")
            .field("stopping", &self.stopping())
            .finish_non_exhaustive()
    }
}

impl StreamCtx {
    /// A stop or a planned restart was requested: the loop should end at its next boundary, with nothing more requested
    /// from the radio.
    pub fn stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire) || self.ending.load(Ordering::Acquire)
    }

    /// Writes `gain_db` to receiver `rx` through [`Radio::write_held_gain`]. It never waits: `None` while the radio is busy
    /// (a configure, a retune, a close) or gone.
    pub fn write_gain(&self, rx: usize, gain_db: f64) -> Option<Result<(), DriverError>> {
        self.gain.as_ref().and_then(|w| w(rx, gain_db))
    }
}

/// Where a stream writes its samples: the ring's writer, with the radio-side loss detector watching every write.
pub struct Sink {
    writer: RingWriter,
    loss: Arc<Mutex<LossDetector>>,
    notices: Notices,
    epoch: Instant,
    offered: u64,
    advice: &'static str,
    last_warn: Option<Instant>,
}

impl Sink {
    fn new(
        writer: RingWriter,
        loss: Arc<Mutex<LossDetector>>,
        notices: Notices,
        epoch: Instant,
        advice: &'static str,
    ) -> Self {
        Sink {
            writer,
            loss,
            notices,
            epoch,
            offered: 0,
            advice,
            last_warn: None,
        }
    }

    /// Writes the whole native samples of `bytes` that fit (a live radio never waits) and returns how many were written. The rest
    /// count as loss on the reader's side.
    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let n = bytes.len() / crate::sample::size(self.writer.sample_type());
        if n == 0 {
            return 0;
        }
        let k = self.writer.write(bytes);
        self.offered(n);
        k
    }

    /// As [`Sink::write`], filling the ring in place: see [`RingWriter::write_with`].
    pub fn write_with(
        &mut self,
        n: usize,
        fill: impl FnMut(&mut [std::mem::MaybeUninit<u8>], usize),
    ) -> usize {
        if n == 0 {
            return 0;
        }
        let k = self.writer.write_with(n, fill);
        self.offered(n);
        k
    }

    /// As [`Sink::write`], reading from the device straight into the ring: see [`RingWriter::receive`]. It returns the bytes
    /// that `read` returned.
    ///
    /// # Errors
    /// The error of `read`.
    pub fn receive(
        &mut self,
        max: usize,
        spare: &mut [u8],
        read: impl FnOnce(&mut [u8]) -> std::io::Result<usize>,
    ) -> std::io::Result<usize> {
        let got = self.writer.receive(max, spare, read)?;
        if got.samples > 0 {
            self.offered(got.samples);
        }
        Ok(got.bytes)
    }

    /// As [`Sink::receive`] for a message body of `n` bytes: it calls `read` until all of them are read, and offers the whole
    /// samples of the body as one write. A part of a sample at the end of the body is dropped.
    ///
    /// # Errors
    /// The error of `read` (`Interrupted` is tried again), or `UnexpectedEof` if it returns 0. The samples of the body count
    /// as not offered then.
    pub fn receive_exact(
        &mut self,
        n: usize,
        spare: &mut [u8],
        mut read: impl FnMut(&mut [u8]) -> std::io::Result<usize>,
    ) -> std::io::Result<()> {
        let (mut left, mut samples) = (n, 0);
        let result = loop {
            if left == 0 {
                break Ok(());
            }
            match self.writer.receive(left, spare, &mut read) {
                Ok(got) if got.bytes == 0 => break Err(std::io::ErrorKind::UnexpectedEof.into()),
                Ok(got) => {
                    left -= got.bytes;
                    samples += got.samples;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => break Err(e),
            }
        };
        self.writer.discard_part();
        if result.is_ok() && samples > 0 {
            self.offered(samples);
        }
        result
    }

    fn offered(&mut self, len: usize) {
        self.offered += len as u64;
        let t = self.epoch.elapsed().as_secs_f64();
        let found = self
            .loss
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write(t, self.offered, len);
        if let Some(l) = found {
            let quiet = self
                .last_warn
                .is_some_and(|w| w.elapsed() < Duration::from_secs(10));
            if !quiet {
                self.last_warn = Some(Instant::now());
                let rate = self
                    .loss
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .rate();
                let ms = if rate > 0.0 {
                    l.samples as f64 / rate * 1e3
                } else {
                    0.0
                };
                self.notices.warn(format!(
                    "the radio did not deliver about {} samples ({ms:.0} ms); {}",
                    l.samples, self.advice
                ));
            }
        }
    }

    /// Counts `samples` that the radio or its link lost before the ring (a refill that failed, a part of a buffer that did
    /// not come) as radio-side loss, and marks the place in the ring ([`RingWriter::mark_gap`]) so the reader can mark a
    /// discontinuity. They count as offered, so the loss detector does not count them again.
    pub fn lost(&mut self, samples: u64) {
        if samples == 0 {
            return;
        }
        self.offered += samples;
        self.loss
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .add_gap(self.epoch.elapsed().as_secs_f64(), samples);
        self.writer.mark_gap(samples);
    }

    /// Marks a gap of `samples` that the driver made on purpose: a retune that flushes the device, or the time a
    /// half-duplex radio transmits. The place is marked in the ring ([`RingWriter::mark_deliberate_gap`]) so the reader
    /// can mark a discontinuity, and the samples count as offered. Neither the radio nor the reader counts them as lost.
    pub fn deliberate_gap(&mut self, samples: u64) {
        if samples == 0 {
            return;
        }
        self.offered += samples;
        self.writer.mark_deliberate_gap(samples);
    }

    /// The consumer was dropped or has requested a stop.
    pub fn closed(&self) -> bool {
        self.writer.closed()
    }

    /// Radio-side samples lost so far (for throughput figures).
    pub fn radio_lost(&self) -> u64 {
        self.loss
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .totals()
            .0
    }
}

/// What a driver adds to its runner beyond the radio. The runner calls these also when no radio is open (after a stop, or
/// while the link is away). Each has a default that does nothing.
pub trait RunnerHooks: std::fmt::Debug + Send + Sync + 'static {
    /// Called by [`Source::apply`] before it changes anything. If it returns true, the runner changes nothing and returns
    /// [`Retuned::NeedsRestart`]. It sees the new `values`, and the hook keeps the old ones.
    fn restart_needed(&self, values: &Values) -> bool {
        let _ = values;
        false
    }
    /// Samples and events that the radio lost and the stream did not count. The runner adds them to its loss report.
    fn extra_loss(&self) -> (u64, u64) {
        (0, 0)
    }
    /// A status line that replaces the line of the radio at each poll when it is `Some`.
    fn status(&self) -> Option<String> {
        None
    }
    /// Called at the end of [`Source::stop`].
    fn stopped(&self) {}
}

/// The base's timing and wording.
#[derive(Clone, Debug)]
pub struct RunnerConfig {
    /// The hooks of the driver, if it has any.
    pub hooks: Option<Arc<dyn RunnerHooks>>,
    /// A name for log lines ("rtl_tcp 10.0.0.2:1234").
    pub name: String,
    /// Advice shown with a radio-side loss.
    pub advice: &'static str,
    /// Open attempts at start (1 + retries).
    pub open_attempts: u32,
    /// The gap between open attempts.
    pub retry_gap: Duration,
    /// The wait after a loss before reconnecting (a stop on its way is not a loss).
    pub lost_wait: Duration,
    /// The gap between reconnect attempts.
    pub reconnect_gap: Duration,
    /// How long a retune waits for a stream restart (a rate change on a radio that cannot change rate while streaming).
    pub restart_timeout: Duration,
    /// How long a stop or a planned restart lets the stream end by itself (it then closes its buffer with the protocol) before
    /// its connection is shut down. Zero: shut down at once.
    pub stop_grace: Duration,
}

impl RunnerConfig {
    /// The usual timings: 4 open attempts 0.5 s apart, a wait of 100 ms after a loss, a reconnect every second.
    pub fn new(name: impl Into<String>, advice: &'static str) -> Self {
        RunnerConfig {
            hooks: None,
            name: name.into(),
            advice,
            open_attempts: 4,
            retry_gap: Duration::from_millis(500),
            lost_wait: Duration::from_millis(100),
            reconnect_gap: Duration::from_secs(1),
            restart_timeout: Duration::from_secs(20),
            stop_grace: Duration::ZERO,
        }
    }
}

/// Advice for a network radio's losses.
pub const NETWORK_ADVICE: &str = "use a wired connection or a lower sample rate";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Open, not streaming yet.
    Idle,
    Up,
    /// Lost; reconnecting.
    Away,
    Stopped,
}

type RestartReply = mpsc::Sender<Result<Started, DriverError>>;

struct State {
    wanted: Values,
    pending: bool,
    phase: Phase,
    restart: Option<RestartReply>,
    asked_rate: Hz,
    rate: Hz,
    centre: Hz,
}

struct Shared<R> {
    slot: Mutex<Option<R>>,
    state: Mutex<State>,
    stopping: Arc<AtomicBool>,
    open_abort: Mutex<Arc<Abort>>,
    stream_abort: Mutex<Arc<Abort>>,
    notices: Notices,
    loss: Arc<Mutex<LossDetector>>,
    /// The loss detector's clock.
    epoch: Instant,
    link_down: AtomicU64,
    /// See [`StreamCtx::stopping`].
    ending: Arc<AtomicBool>,
    /// The streams whose loop has ended, counted.
    ended: AtomicU64,
    /// The counters of the ring that the stream writes to, for the DC removal of its reader ([`crate::dc::follow`]).
    ring: Mutex<Option<Arc<crate::ring::RingStats>>>,
}

impl<R> Shared<R> {
    fn now_s(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }
    /// A fresh abort handle in `which`; fired at once if a stop is under way (the stop may have fired the old one already).
    fn fresh(&self, which: &Mutex<Arc<Abort>>) -> Arc<Abort> {
        let mut g = which
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *g = Abort::new();
        if self.stopping() {
            g.fire();
        }
        g.clone()
    }
    /// Sleeps up to `d`, waking early for a stop. Returns true if stopping.
    fn nap(&self, d: Duration) -> bool {
        let end = Instant::now() + d;
        while Instant::now() < end {
            if self.stopping() {
                return true;
            }
            std::thread::sleep(
                Duration::from_millis(10).min(end.saturating_duration_since(Instant::now())),
            );
        }
        self.stopping()
    }
}

/// An open network radio run by the base. Drivers return it as their [`Source`].
pub struct StreamRunner<R: Radio> {
    shared: Arc<Shared<R>>,
    opener: Opener<R>,
    cfg: RunnerConfig,
    thread: Option<JoinHandle<()>>,
    caps: Capabilities,
    settings: Vec<SettingDecl>,
    caps_changed: bool,
    /// A restart the radio reported while its slot was busy, told with the next refreshed capabilities.
    restart_pending: bool,
    sample: SampleType,
}

/// An open is tried again only while less than this has passed since its first try.
pub const OPEN_RETRY_WITHIN: Duration = Duration::from_secs(5);

fn open_once<R: Radio>(
    opener: &Opener<R>,
    values: &Values,
    abort: &Arc<Abort>,
    notices: &Notices,
) -> Result<(R, String), DriverError> {
    let mut r = opener(values, abort, notices)?;
    match r.check(values).and_then(|()| r.configure(values, false)) {
        Ok(note) => Ok((r, note)),
        Err(e) => {
            r.close();
            Err(e)
        }
    }
}

impl<R: Radio> StreamRunner<R> {
    /// Opens the radio with `values`: up to `open_attempts` tries, `retry_gap` apart, while the failure can pass
    /// ([`DriverError::retryable`]) and less than [`OPEN_RETRY_WITHIN`] has passed since the first try, so that an address
    /// that does not answer fails after one wait. It does not start streaming.
    ///
    /// # Errors
    ///
    /// The last [`DriverError`] of the opener.
    pub fn open(
        cfg: RunnerConfig,
        opener: Opener<R>,
        values: &Values,
    ) -> Result<Self, DriverError> {
        let notices = Notices::default();
        let mut attempt = 0;
        let began = Instant::now();
        let (radio, note) = loop {
            attempt += 1;
            match open_once(&opener, values, &Abort::new(), &notices) {
                Ok(v) => break v,
                Err(e)
                    if e.retryable()
                        && attempt < cfg.open_attempts.max(1)
                        && began.elapsed() < OPEN_RETRY_WITHIN =>
                {
                    notices.log(format!("{}: {e}; trying again", cfg.name));
                    std::thread::sleep(cfg.retry_gap);
                }
                Err(e) => return Err(e),
            }
        };
        if !note.is_empty() {
            notices.warn(note.clone());
        }
        let caps = radio.capabilities();
        let settings = radio.settings();
        let sample = radio.sample_type();
        let state = State {
            wanted: values.clone(),
            pending: false,
            phase: Phase::Idle,
            restart: None,
            asked_rate: values.get(&RATE),
            rate: radio.rate_hz(),
            centre: radio.centre_hz(),
        };
        let shared = Arc::new(Shared {
            loss: Arc::new(Mutex::new(LossDetector::new(radio.rate_hz()))),
            slot: Mutex::new(Some(radio)),
            state: Mutex::new(state),
            stopping: Arc::new(AtomicBool::new(false)),
            open_abort: Mutex::new(Abort::new()),
            stream_abort: Mutex::new(Abort::new()),
            notices,
            epoch: Instant::now(),
            link_down: AtomicU64::new(0),
            ending: Arc::new(AtomicBool::new(false)),
            ended: AtomicU64::new(0),
            ring: Mutex::new(None),
        });
        Ok(StreamRunner {
            shared,
            opener,
            cfg,
            thread: None,
            caps,
            settings,
            caps_changed: false,
            restart_pending: false,
            sample,
        })
    }

    fn started(&self, note: String) -> Started {
        if !note.is_empty() {
            self.shared.notices.warn(note);
        }
        let st = self.shared.state();
        Started {
            rate_hz: st.rate,
            centre_hz: st.centre,
        }
    }

    /// Takes the capabilities and run-time settings of `r`, and notes a change for the next poll.
    fn refresh(&mut self, r: &R) {
        let caps = r.capabilities();
        if caps != self.caps {
            self.caps = caps;
            self.caps_changed = true;
        }
        let settings = r.settings();
        if settings != self.settings {
            self.settings = settings;
            self.caps_changed = true;
        }
    }
}

impl<R: Radio> Source for StreamRunner<R> {
    fn capabilities(&self) -> &Capabilities {
        &self.caps
    }

    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        if self.thread.is_some() {
            return Err(DriverError::NotNow {
                state: crate::Doing::Running,
            });
        }
        let sh = self.shared.clone();
        let stream = {
            let mut slot = sh
                .slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if slot.is_none() && sh.state().phase == Phase::Stopped {
                // A stop closed the radio. A start after it opens the radio again with the settings it had: the engine
                // stops and starts a radio that changed its rate.
                sh.stopping.store(false, Ordering::Release);
                let wanted = sh.state().wanted.clone();
                let abort = sh.fresh(&sh.open_abort);
                let (r, note) = open_once(&self.opener, &wanted, &abort, &sh.notices)?;
                if !note.is_empty() {
                    sh.notices.log(note);
                }
                self.sample = r.sample_type();
                *slot = Some(r);
            }
            let r = slot.as_mut().ok_or(DriverError::NotNow {
                state: crate::Doing::Closed,
            })?;
            if r.sample_type() != sink.sample_type() {
                return Err(DriverError::WrongSampleType {
                    ring: sink.sample_type(),
                    radio: r.sample_type(),
                });
            }
            let abort = sh.fresh(&sh.stream_abort);
            let s = r.stream(&abort)?;
            let mut st = sh.state();
            st.phase = Phase::Up;
            st.rate = r.rate_hz();
            st.centre = r.centre_hz();
            sh.loss
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .restarted(st.rate);
            s
        };
        let stats = sink.stats();
        crate::dc::follow(&stats, &sh.state().wanted);
        *sh.ring
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stats);
        let sink = Sink::new(
            sink,
            sh.loss.clone(),
            sh.notices.clone(),
            sh.epoch,
            self.cfg.advice,
        );
        let (opener, cfg) = (self.opener.clone(), self.cfg.clone());
        let t = std::thread::Builder::new()
            .name("sdrkit-radio".into())
            .spawn(move || radio_thread(sh, opener, cfg, stream, sink))
            .map_err(|_| DriverError::Thread)?;
        self.thread = Some(t);
        Ok(self.started(String::new()))
    }

    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        let tune = values;
        let sh = self.shared.clone();
        if let Some(h) = &self.cfg.hooks {
            if sh.state().phase != Phase::Stopped && h.restart_needed(values) {
                // The next open (the start after the restart) takes the new values.
                sh.state().wanted = tune.clone();
                return Ok(Retuned::NeedsRestart);
            }
        }
        let mut slot = sh
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut st = sh.state();
        let rate_change = (tune.get(&RATE) - st.asked_rate).abs() > 1.0;
        let phase = st.phase;
        match (slot.as_mut(), phase) {
            (_, Phase::Stopped) => Err(DriverError::NotNow {
                state: crate::Doing::Stopped,
            }),
            (Some(r), Phase::Idle) => {
                drop(st);
                r.check(tune)?;
                sh.state().wanted = tune.clone();
                let note = r.configure(tune, false)?;
                let mut st = sh.state();
                st.asked_rate = tune.get(&RATE);
                (st.rate, st.centre) = (r.rate_hz(), r.centre_hz());
                drop(st);
                self.refresh(r);
                drop(slot);
                Ok(Retuned::Live(self.started(note)))
            }
            (Some(r), Phase::Up) => {
                drop(st);
                r.check(tune)?;
                sh.state().wanted = tune.clone();
                if rate_change && !r.live_rate_change() {
                    let (tx, rx) = mpsc::channel();
                    sh.state().restart = Some(tx);
                    drop(slot);
                    // The stream ends at its next boundary and closes its buffer; one that does not end in time is cut off.
                    let old = sh
                        .stream_abort
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    let before = sh.ended.load(Ordering::Acquire);
                    sh.ending.store(true, Ordering::Release);
                    let end = Instant::now() + self.cfg.stop_grace;
                    while sh.ended.load(Ordering::Acquire) == before && Instant::now() < end {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    if sh.ended.load(Ordering::Acquire) == before {
                        old.fire();
                    }
                    let res = rx
                        .recv_timeout(self.cfg.restart_timeout)
                        .map_err(|_| DriverError::Net(sdrkit_core::NetError::TimedOut))?;
                    if let Ok(g) = sh.slot.try_lock() {
                        if let Some(r) = g.as_ref() {
                            self.refresh(r);
                        }
                    }
                    return res.map(Retuned::Live);
                }
                let note = r.configure(tune, true)?;
                // A retune or a change of gain moves the DC offset.
                if let Some(stats) = sh
                    .ring
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_ref()
                {
                    crate::dc::follow(stats, tune);
                }
                let mut st = sh.state();
                st.asked_rate = tune.get(&RATE);
                (st.rate, st.centre) = (r.rate_hz(), r.centre_hz());
                sh.loss
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .retuned(st.rate);
                drop(st);
                self.refresh(r);
                drop(slot);
                Ok(Retuned::Live(self.started(note)))
            }
            _ => {
                // Away (or reopening): store, apply when the radio is back.
                st.wanted = tune.clone();
                st.pending = true;
                st.asked_rate = tune.get(&RATE);
                st.centre = tune.get(&CENTRE);
                drop(st);
                drop(slot);
                Ok(Retuned::Live(self.started(
                    "the radio is away: the change is applied when it is back".into(),
                )))
            }
        }
    }

    fn stop(&mut self) {
        let sh = &self.shared;
        sh.stopping.store(true, Ordering::Release);
        sh.open_abort
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fire();
        // The stream ends at its next boundary and closes its buffer itself, within the grace.
        if let Some(t) = &self.thread {
            let end = Instant::now() + self.cfg.stop_grace;
            while !t.is_finished() && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        // Close first (a driver may still have something to send), then interrupt the read.
        let radio = sh
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(mut r) = radio {
            r.stream_ended();
            r.close();
        }
        sh.stream_abort
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fire();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(mut r) = sh
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            r.close();
        }
        sh.state().phase = Phase::Stopped;
        if let Some(h) = &self.cfg.hooks {
            h.stopped();
        }
    }

    fn sample_type(&self) -> SampleType {
        self.sample
    }

    fn link(&self) -> Link {
        match self.shared.state().phase {
            Phase::Idle => Link::Idle,
            Phase::Up => Link::Up,
            Phase::Away => Link::Away,
            Phase::Stopped => Link::Stopped,
        }
    }

    fn poll(&mut self, results: &mut Results) -> Polled {
        let sh = self.shared.clone();
        let mut slot = sh.slot.try_lock().ok();
        let refreshed = slot.is_some();
        let mut radio = slot.as_mut().and_then(|g| g.as_mut());
        let mut line = String::new();
        if let Some(r) = radio.as_deref_mut() {
            r.readings(results);
            line = r.status();
        }
        let line = self
            .cfg
            .hooks
            .as_ref()
            .and_then(|h| h.status())
            .unwrap_or(line);
        if !line.is_empty() {
            results.update_status(|s| {
                let changed = s.summary != line;
                s.summary = line;
                changed
            });
        }
        // The notices after the status line, so a warning replaces it for this poll; the capabilities after the notices,
        // so a restart noted there is reported with the capabilities it changed.
        let mut polled = self.shared.notices.drain_into(results);
        if let Some(r) = radio.as_deref() {
            self.refresh(r);
        }
        // A restart noted while the radio's slot was busy waits for a poll that could read its capabilities.
        if polled.restart && !refreshed {
            self.restart_pending = true;
            polled.restart = false;
        } else if self.restart_pending && refreshed {
            self.restart_pending = false;
            polled.restart = true;
        }
        polled.capabilities = std::mem::take(&mut self.caps_changed);
        polled
    }

    fn settings(&self) -> Vec<SettingDecl> {
        self.settings.clone()
    }

    fn save_state(&self) -> Option<StateBlob> {
        lock(&self.shared.slot).as_ref().and_then(R::save_state)
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        match lock(&self.shared.slot).as_mut() {
            Some(r) => r.restore_state(state),
            None => Err(StateError::NoState),
        }
    }

    fn reconnect(&mut self) -> Result<(), DriverError> {
        if self.thread.is_none() {
            return Err(DriverError::NotNow {
                state: crate::Doing::Stopped,
            });
        }
        self.shared
            .stream_abort
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fire();
        Ok(())
    }

    fn loss(&self) -> Option<LossReport> {
        let d = self
            .shared
            .loss
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (radio_lost, radio_events, last) = d.totals();
        let now = self.shared.now_s();
        let pct = d.delivered_pct();
        let rep = LossReport {
            rate_hz: d.rate(),
            radio_lost,
            radio_events,
            since_radio_loss: last.map(|t| Duration::from_secs_f64((now - t).max(0.0))),
            link_down: self.shared.link_down.load(Ordering::Relaxed),
            delivered_pct: pct,
            rate_low: pct.is_some_and(|p| p < 97.0),
        };
        let mut rep = rep;
        if let Some(h) = &self.cfg.hooks {
            let (lost, events) = h.extra_loss();
            rep.radio_lost += lost;
            rep.radio_events += events;
        }
        Some(rep)
    }
}

/// Locks `m`, also after a thread panicked while it held the lock.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl<R: Radio> Drop for StreamRunner<R> {
    fn drop(&mut self) {
        self.stop();
    }
}

fn radio_thread<R: Radio>(
    sh: Arc<Shared<R>>,
    opener: Opener<R>,
    cfg: RunnerConfig,
    first: R::Stream,
    mut sink: Sink,
) {
    let weak = Arc::downgrade(&sh);
    let gain: GainWrite = Arc::new(move |rx, gain_db| {
        let sh = weak.upgrade()?;
        let mut slot = sh.slot.try_lock().ok()?;
        slot.as_mut().map(|r| r.write_held_gain(rx, gain_db))
    });
    let ctx = StreamCtx {
        stopping: sh.stopping.clone(),
        ending: sh.ending.clone(),
        notices: sh.notices.clone(),
        gain: Some(gain),
    };
    let mut next = Some(first);
    let mut first_stream = true;
    'outer: loop {
        let Some(mut stream) = next.take() else {
            break;
        };
        // A stream after the first (a restart, a reconnect) starts the DC estimate again; the start set it for the first.
        if !std::mem::take(&mut first_stream) {
            crate::dc::follow(&sink.writer.stats(), &sh.state().wanted);
        }
        let result = stream.run(&mut sink, &ctx);
        // The old stream closes its connection before anything else goes to the radio: a connection left open and unread
        // would hold the radio's buffer for a client that no longer reads it. Then the abort lets go of its handles on it.
        drop(stream);
        sh.stream_abort
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fire();
        sh.ending.store(false, Ordering::Release);
        sh.ended.fetch_add(1, Ordering::AcqRel);
        if sh.stopping() {
            break;
        }
        let mut reason = match &result {
            Ok(()) => "the stream ended".to_string(),
            Err(e) => e.to_string(),
        };
        // A planned restart: a rate change on a radio that cannot change rate while streaming.
        let restart = sh.state().restart.take();
        if let Some(reply) = restart {
            let mut slot = sh
                .slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(r) = slot.as_mut() {
                r.stream_ended();
                let tune = sh.state().wanted.clone();
                let abort = sh.fresh(&sh.stream_abort);
                match r
                    .configure(&tune, false)
                    .and_then(|note| Ok((r.stream(&abort)?, note)))
                {
                    Ok((s, note)) => {
                        next = Some(s);
                        let mut st = sh.state();
                        st.asked_rate = tune.get(&RATE);
                        (st.rate, st.centre) = (r.rate_hz(), r.centre_hz());
                        sh.loss
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .retuned(st.rate);
                        if !note.is_empty() {
                            sh.notices.warn(note);
                        }
                        let _ = reply.send(Ok(Started {
                            rate_hz: st.rate,
                            centre_hz: st.centre,
                        }));
                        continue;
                    }
                    Err(e) if e.retryable() => {
                        let st = sh.state();
                        sh.notices.warn(format!("{e}; reconnecting"));
                        let _ = reply.send(Ok(Started {
                            rate_hz: tune.get(&RATE),
                            centre_hz: st.centre,
                        }));
                        reason = e.to_string();
                    }
                    Err(e) => {
                        sh.notices
                            .warn(format!("{}: {e}; the radio is stopped", cfg.name));
                        let _ = reply.send(Err(e));
                        r.close();
                        *slot = None;
                        break;
                    }
                }
            }
        }
        // Lost.
        sh.state().phase = Phase::Away;
        sh.notices.warn(format!("{}: {reason}", cfg.name));
        if sh.nap(cfg.lost_wait) {
            break;
        }
        let lost_at = Instant::now();
        let old = sh
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(mut r) = old {
            r.stream_ended();
            r.close();
        }
        let mut last_reason = String::new();
        loop {
            if sh.stopping() {
                break 'outer;
            }
            let tune = {
                let mut st = sh.state();
                st.pending = false;
                st.wanted.clone()
            };
            let abort = sh.fresh(&sh.open_abort);
            let opened =
                open_once(&opener, &tune, &abort, &sh.notices).and_then(|(mut r, note)| {
                    let sa = sh.fresh(&sh.stream_abort);
                    match r.stream(&sa) {
                        Ok(s) => Ok((r, s, note)),
                        Err(e) => {
                            r.close();
                            Err(e)
                        }
                    }
                });
            match opened {
                Ok((mut r, s, note)) => {
                    if sh.stopping() {
                        r.close();
                        break 'outer;
                    }
                    if r.sample_type() != sink.writer.sample_type() {
                        // The ring cannot change its sample type: the radio stops.
                        let e = DriverError::WrongSampleType {
                            ring: sink.writer.sample_type(),
                            radio: r.sample_type(),
                        };
                        sh.notices
                            .warn(format!("{}: {e}; the radio is stopped", cfg.name));
                        r.close();
                        break 'outer;
                    }
                    next = Some(s);
                    let mut slot = sh
                        .slot
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let r = slot.insert(r);
                    let (pending, wanted) = {
                        let mut st = sh.state();
                        st.phase = Phase::Up;
                        (std::mem::take(&mut st.pending), st.wanted.clone())
                    };
                    if pending {
                        // A retune arrived while the radio was opening: apply it now.
                        let restart = (wanted.get(&RATE) - tune.get(&RATE)).abs() > 1.0
                            && !r.live_rate_change();
                        let res = if restart {
                            // The stream just opened has not run: it closes its buffer before the radio is configured.
                            drop(next.take());
                            sh.stream_abort
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .fire();
                            r.stream_ended();
                            let sa = sh.fresh(&sh.stream_abort);
                            r.check(&wanted)
                                .and_then(|()| r.configure(&wanted, false))
                                .and_then(|n| Ok((r.stream(&sa)?, n)))
                                .map(|(s, n)| {
                                    next = Some(s);
                                    n
                                })
                        } else {
                            r.check(&wanted).and_then(|()| r.configure(&wanted, true))
                        };
                        if let Err(e) = res {
                            sh.notices.warn(format!(
                                "{}: the new settings were not applied: {e}",
                                cfg.name
                            ));
                        }
                        if next.is_none() {
                            // No stream opened with the new settings: the radio is opened anew.
                            if sh.nap(cfg.reconnect_gap) {
                                break 'outer;
                            }
                            continue;
                        }
                    }
                    if !note.is_empty() {
                        sh.notices.warn(note);
                    }
                    let mut st = sh.state();
                    st.asked_rate = if pending {
                        wanted.get(&RATE)
                    } else {
                        tune.get(&RATE)
                    };
                    (st.rate, st.centre) = (r.rate_hz(), r.centre_hz());
                    let secs = lost_at.elapsed().as_secs_f64();
                    let gap = (secs * st.rate).round() as u64;
                    {
                        let mut d = sh
                            .loss
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        d.add_gap(sh.now_s(), gap);
                        d.restarted(st.rate);
                    }
                    sh.link_down.fetch_add(gap, Ordering::Relaxed);
                    sh.notices.log(format!(
                        "{}: reconnected after {secs:.1} s, about {gap} samples lost",
                        cfg.name
                    ));
                    break;
                }
                Err(e) => {
                    let text = e.to_string();
                    if text != last_reason {
                        sh.notices
                            .log(format!("{}: reconnect failed: {text}", cfg.name));
                        last_reason = text;
                    }
                    if sh.nap(cfg.reconnect_gap) {
                        break 'outer;
                    }
                }
            }
        }
    }
    sh.state().phase = Phase::Stopped;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;
    use crate::RadioSetting;
    use sdrkit_core::publish::field;
    use sdrkit_core::Field;
    use sdrkit_core::SettingSet;
    use std::sync::atomic::AtomicU32;

    const TEMP: Field<field::F64> = Field::new("temp_c", "Temperature", "°C");

    /// What the fake radio does on each open: its sample type, and whether `check` refuses.
    struct Plan {
        opens: AtomicU32,
        sample_from: u32,
        refuse_from: u32,
        checks: AtomicU32,
    }

    struct Fake {
        sample: SampleType,
        refuse: bool,
        plan: Arc<Plan>,
    }

    struct Drops;

    impl RadioStream for Drops {
        fn run(&mut self, _: &mut Sink, _: &StreamCtx) -> Result<(), DriverError> {
            // Every stream drops after a moment.
            std::thread::sleep(Duration::from_millis(5));
            Err(DriverError::Net(sdrkit_core::NetError::Closed))
        }
    }

    impl Radio for Fake {
        type Stream = Drops;
        fn configure(&mut self, _: &Values, _: bool) -> Result<String, DriverError> {
            Ok(String::new())
        }
        fn stream(&mut self, _: &Arc<Abort>) -> Result<Drops, DriverError> {
            Ok(Drops)
        }
        fn close(&mut self) {}
        fn rate_hz(&self) -> Hz {
            1e6
        }
        fn centre_hz(&self) -> Hz {
            100e6
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities::default()
        }
        fn sample_type(&self) -> SampleType {
            self.sample
        }
        fn check(&self, _: &Values) -> Result<(), DriverError> {
            self.plan.checks.fetch_add(1, Ordering::Relaxed);
            if self.refuse {
                Err(DriverError::Refused {
                    id: RadioSetting::Gain.id(),
                })
            } else {
                Ok(())
            }
        }
        fn readings(&mut self, results: &mut Results) {
            results.fields_mut().set(&TEMP, 41.5);
        }
    }

    fn opener(plan: &Arc<Plan>) -> Opener<Fake> {
        let plan = plan.clone();
        Arc::new(move |_, _, _| {
            let n = plan.opens.fetch_add(1, Ordering::Relaxed);
            Ok(Fake {
                sample: if n >= plan.sample_from {
                    SampleType::Cs16
                } else {
                    SampleType::Cs8
                },
                refuse: n >= plan.refuse_from,
                plan: plan.clone(),
            })
        })
    }

    fn values() -> Values {
        testing::values(
            &crate::synth::SynthDriver::new(Arc::new(|_: &str, _: &Values, _| None)),
            &[],
        )
    }

    fn quick() -> RunnerConfig {
        RunnerConfig {
            lost_wait: Duration::from_millis(1),
            reconnect_gap: Duration::from_millis(5),
            ..RunnerConfig::new("fake", NETWORK_ADVICE)
        }
    }

    fn wait_for(f: impl Fn() -> bool) {
        let end = Instant::now() + Duration::from_secs(5);
        while !f() {
            assert!(Instant::now() < end, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn an_open_that_fails_after_a_long_wait_is_not_tried_again() {
        let opens = Arc::new(AtomicU32::new(0));
        let seen = opens.clone();
        let opener: Opener<Fake> = Arc::new(move |_, _, _| {
            seen.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(OPEN_RETRY_WITHIN);
            Err(DriverError::Net(sdrkit_core::NetError::TimedOut))
        });
        let cfg = RunnerConfig {
            open_attempts: 4,
            ..quick()
        };
        let e = StreamRunner::open(cfg, opener, &values()).err();
        assert!(matches!(
            e,
            Some(DriverError::Net(sdrkit_core::NetError::TimedOut))
        ));
        assert_eq!(opens.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_start_after_a_stop_opens_the_radio_again() {
        let plan = Arc::new(Plan {
            opens: AtomicU32::new(0),
            sample_from: u32::MAX,
            refuse_from: u32::MAX,
            checks: AtomicU32::new(0),
        });
        let mut r = StreamRunner::open(quick(), opener(&plan), &values()).unwrap();
        let (w, _rd) = crate::ring(1 << 12, SampleType::Cs8);
        r.start(w).unwrap();
        r.stop();
        assert_eq!(r.link(), Link::Stopped);
        let opens = plan.opens.load(Ordering::Relaxed);
        let (w, _rd) = crate::ring(1 << 12, SampleType::Cs8);
        r.start(w).unwrap();
        assert_eq!(plan.opens.load(Ordering::Relaxed), opens + 1);
        assert_ne!(r.link(), Link::Stopped);
        r.stop();
    }

    #[test]
    fn a_restart_that_the_hooks_ask_for_opens_with_the_new_values() {
        #[derive(Debug)]
        struct Always;
        impl RunnerHooks for Always {
            fn restart_needed(&self, _: &Values) -> bool {
                true
            }
        }
        let plan = Arc::new(Plan {
            opens: AtomicU32::new(0),
            sample_from: u32::MAX,
            refuse_from: u32::MAX,
            checks: AtomicU32::new(0),
        });
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (inner, s) = (opener(&plan), seen.clone());
        let opener: Opener<Fake> = Arc::new(move |v, a, n| {
            lock(&s).push(v.get(&CENTRE));
            inner(v, a, n)
        });
        let cfg = RunnerConfig {
            hooks: Some(Arc::new(Always)),
            ..quick()
        };
        let mut r = StreamRunner::open(cfg, opener, &values()).unwrap();
        let (w, _rd) = crate::ring(1 << 12, SampleType::Cs8);
        r.start(w).unwrap();
        let mut v = values();
        v.set(RadioSetting::Centre.id(), &Value::Hz(433e6)).unwrap();
        assert!(matches!(r.apply(&v), Ok(Retuned::NeedsRestart)));
        r.stop();
        let (w, _rd) = crate::ring(1 << 12, SampleType::Cs8);
        r.start(w).unwrap();
        r.stop();
        assert_eq!(lock(&seen).last(), Some(&433e6));
    }

    #[test]
    fn a_reopen_with_another_sample_type_stops_the_radio() {
        let plan = Arc::new(Plan {
            opens: AtomicU32::new(0),
            sample_from: 1,
            refuse_from: u32::MAX,
            checks: AtomicU32::new(0),
        });
        let mut r = StreamRunner::open(quick(), opener(&plan), &values()).unwrap();
        let (w, _rd) = crate::ring(1 << 12, SampleType::Cs8);
        r.start(w).unwrap();
        wait_for(|| r.link() == Link::Stopped);
        assert!(plan.opens.load(Ordering::Relaxed) >= 2);
        r.stop();
    }

    #[test]
    fn saved_settings_are_checked_at_a_reconnect_and_readings_reach_the_results() {
        let plan = Arc::new(Plan {
            opens: AtomicU32::new(0),
            sample_from: u32::MAX,
            refuse_from: 1,
            checks: AtomicU32::new(0),
        });
        let mut r = StreamRunner::open(quick(), opener(&plan), &values()).unwrap();
        let (w, _rd) = crate::ring(1 << 12, SampleType::Cs8);
        r.start(w).unwrap();
        // The radio drops, and every reopen is refused by `check`: it stays away.
        wait_for(|| plan.opens.load(Ordering::Relaxed) >= 3);
        assert_eq!(r.link(), Link::Away);
        assert!(plan.checks.load(Ordering::Relaxed) >= 3);
        r.stop();
        let decl = sdrkit_core::Published::new(&[&TEMP])
            .results()
            .decl()
            .clone();
        let mut results = Results::new(decl);
        let plan = Arc::new(Plan {
            opens: AtomicU32::new(0),
            sample_from: u32::MAX,
            refuse_from: u32::MAX,
            checks: AtomicU32::new(0),
        });
        let mut r = StreamRunner::open(quick(), opener(&plan), &values()).unwrap();
        r.poll(&mut results);
        assert_eq!(results.fields().get(&TEMP), Some(41.5));
        r.stop();
    }
}
