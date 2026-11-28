//! A hop of a radio: a stack asks the engine to visit a list of frequencies, a dwell on each. On the test-signal radio,
//! the fake rtl_tcp server and a scripted dongle, each step lasts its dwell of signal time to the sample, the stack is told
//! the centre of a step before its samples, a step is a gap made on purpose and nothing counts as lost. A second reader
//! follows the hop and does not steer it, a share does not stop it, and the hop ends on a fixed frequency, on lock, and
//! with its session. The rtl_tcp server of these tests is stepped by the test ([`Stepper`]), so no clock paces it.

mod common;

use common::*;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Clock, Complex32, ConfigError, Facilities, HostRequest, InputInfo, LockState, MemStorage,
    Output, Owner, Published, Random, Receiver, Results, ResultsDecl, Setting, SettingError,
    SettingId, SettingSet, StackDescriptor, Transport, Value, Values,
};
use sdrkit_driver_core::rtl_tcp::cmd;
use sdrkit_driver_core::rtl_tcp::fake::SteppedRtl;
use sdrkit_host_engine::{Engine, Input, OwnerRef, RadioId, SessionId, Setup};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The number of the log that a probe writes.
const TAG: Setting<field::I64> = Setting::new("tag", "Tag", "", 0).range(0.0, 1e6, 1.0);
/// The channels of the hop, MHz, separated by commas.
const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "MHz", "");
/// The dwell of the hop.
const DWELL_S: Setting<field::F64> =
    Setting::new("dwell_s", "Dwell", "s", 0.2).range(0.01, 10.0, 0.0);
/// A value above the last one sends the hop.
const ROUND: Setting<field::I64> = Setting::new("round", "Round", "", 0).range(0.0, 1e6, 1.0);
/// A new frequency above 0 sends a retune there.
const FIXED_HZ: Setting<field::Hz> =
    Setting::new("fixed_hz", "Fixed", "Hz", 0.0).range(0.0, 1e10, 0.0);
/// Above 0, the request of [`FIXED_HZ`] is a tune of the session itself that states this width around the frequency.
const WIDTH_HZ: Setting<field::Hz> =
    Setting::new("width_hz", "Width", "Hz", 0.0).range(0.0, 1e9, 0.0);
/// The probe reports lock while the centre is here.
const LOCK_HZ: Setting<field::Hz> =
    Setting::new("lock_hz", "Lock", "Hz", 0.0).range(0.0, 1e10, 0.0);
/// The hop ends on lock.
const STOP_ON_LOCK: Setting<field::Bool> = Setting::new("stop_on_lock", "Stop on lock", "", false);
/// While the test holds the probe ([`hold`]), it waits at each block, so that blocks wait in its queue.
const HOLD_EACH: Setting<field::Bool> = Setting::new("hold_each", "Hold at each block", "", false);
/// While the test holds the probe, it waits at the first block after each change of the centre.
const HOLD_MOVED: Setting<field::Bool> = Setting::new("hold_moved", "Hold when moved", "", false);
/// The probe holds ([`HOLD_MOVED`]) at the first block after each change of the frequency too.
const HOLD_TUNED: Setting<field::Bool> = Setting::new("hold_tuned", "Hold when tuned", "", false);
/// The level of the samples at a centre is that of the fake rtl_tcp server, not that of the test signal.
const RTL_LEVELS: Setting<field::Bool> =
    Setting::new("rtl_levels", "Levels of the server", "", false);

sdrkit_core::settings! {
    /// The settings of the probe.
    pub enum ProbeSetting {
        Tag = TAG,
        Channels = CHANNELS,
        DwellS = DWELL_S,
        Round = ROUND,
        FixedHz = FIXED_HZ,
        WidthHz = WIDTH_HZ,
        LockHz = LOCK_HZ,
        StopOnLock = STOP_ON_LOCK,
        HoldEach = HOLD_EACH,
        HoldMoved = HOLD_MOVED,
        HoldTuned = HOLD_TUNED,
        RtlLevels = RTL_LEVELS,
    }
}

/// What a probe was told, in order.
#[derive(Clone, Debug, PartialEq)]
enum Note {
    /// A configure or a retune: the centre of the samples from now on.
    Centre(f64),
    /// A configure or a retune: the frequency that the stack listens to from now on.
    Tuned(f64),
    /// Samples are missing here.
    Gap(u64),
    /// Samples at the centre: their count, and those whose level is not that of the centre.
    Feed { centre: f64, n: u64, bad: u64 },
}

/// The logs of the probes, by tag.
static LOGS: Mutex<Vec<(i64, Vec<Note>)>> = Mutex::new(Vec::new());
/// Signalled at each note.
static LOGGED: Condvar = Condvar::new();
/// The tags of the probes that the test holds.
static HELD: Mutex<Vec<i64>> = Mutex::new(Vec::new());
/// Signalled when the test holds or frees a probe.
static FREED: Condvar = Condvar::new();

/// Holds the probe `tag` (`on`) or frees it: a held probe waits at its points of hold ([`HOLD_EACH`], [`HOLD_MOVED`]).
fn hold(tag: i64, on: bool) {
    let mut h = HELD.lock().unwrap();
    h.retain(|t| *t != tag);
    if on {
        h.push(tag);
    }
    drop(h);
    FREED.notify_all();
}

/// A point of hold of the probe `tag`: it waits while the test holds it, at most a minute.
fn hold_point(tag: i64) {
    let h = HELD.lock().unwrap();
    let _ = FREED
        .wait_timeout_while(h, Duration::from_secs(60), |h| h.contains(&tag))
        .unwrap();
}

fn note(tag: i64, n: Note) {
    let mut logs = LOGS.lock().unwrap();
    let i = match logs.iter().position(|(t, _)| *t == tag) {
        Some(i) => i,
        None => {
            logs.push((tag, Vec::new()));
            logs.len() - 1
        }
    };
    let log = &mut logs[i].1;
    if let (
        Some(Note::Feed { centre, n, bad }),
        Note::Feed {
            centre: c,
            n: m,
            bad: b,
        },
    ) = (log.last_mut(), &n)
    {
        if *centre == *c {
            *n += m;
            *bad += b;
            return;
        }
    }
    log.push(n);
    drop(logs);
    LOGGED.notify_all();
}

/// Waits until `f` gives a value for the log of probe `tag`, at most `timeout`, and returns it. `f` is asked at each note
/// and every 10 ms.
fn wait_log<T>(tag: i64, timeout: Duration, mut f: impl FnMut(&[Note]) -> Option<T>) -> Option<T> {
    let end = Instant::now() + timeout;
    let mut logs = LOGS.lock().unwrap();
    loop {
        let log = logs
            .iter()
            .find(|(t, _)| *t == tag)
            .map_or(&[][..], |(_, l)| l);
        if let Some(v) = f(log) {
            return Some(v);
        }
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        logs = LOGGED
            .wait_timeout(logs, left.min(Duration::from_millis(10)))
            .unwrap()
            .0;
    }
}

/// The samples that the probe was fed and those left out before them.
fn accounted(notes: &[Note]) -> u64 {
    notes
        .iter()
        .map(|n| match *n {
            Note::Feed { n, .. } => n,
            Note::Gap(g) => g,
            _ => 0,
        })
        .sum()
}

fn log(tag: i64) -> Vec<Note> {
    let logs = LOGS.lock().unwrap();
    logs.iter()
        .find(|(t, _)| *t == tag)
        .map(|(_, l)| l.clone())
        .unwrap_or_default()
}

/// The magnitude of the test signal of the probe with the radio centre at `hz`.
fn level_of(hz: f64) -> f32 {
    (0.1 + (hz - 100e6) / 1e6 * 0.05) as f32
}

/// A stack that logs what it is told and sends the requests that its settings ask for.
struct Probe {
    results: Published,
    values: Values,
    centre: f64,
    tuned: f64,
    /// The centre changed since the last block.
    moved: bool,
    sent_round: i64,
    sent_fixed: f64,
}

impl Probe {
    fn tag(&self) -> i64 {
        self.values.get(&TAG)
    }

    fn told(&mut self, input: &InputInfo) {
        if input.centre_hz != self.centre {
            self.centre = input.centre_hz;
            self.moved = true;
            note(self.tag(), Note::Centre(input.centre_hz));
        }
        if input.tuned_hz != self.tuned {
            self.tuned = input.tuned_hz;
            self.moved |= self.values.get(&HOLD_TUNED);
            note(self.tag(), Note::Tuned(input.tuned_hz));
        }
    }
}

impl Receiver for Probe {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.centre = f64::NAN;
        self.told(input);
        Ok(())
    }
    fn retuned(&mut self, input: &InputInfo) {
        self.told(input);
    }
    fn discontinuity(&mut self, lost: u64) {
        note(self.tag(), Note::Gap(lost));
    }
    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        let moved = std::mem::take(&mut self.moved);
        let level = if self.values.get(&RTL_LEVELS) {
            sdrkit_driver_core::rtl_tcp::fake::level_of(self.centre)
        } else {
            level_of(self.centre)
        };
        let bad = iq
            .iter()
            .filter(|s| (s.norm() - level).abs() > 0.02)
            .count() as u64;
        let n = iq.len() as u64;
        note(
            self.tag(),
            Note::Feed {
                centre: self.centre,
                n,
                bad,
            },
        );
        let round = self.values.get(&ROUND);
        if round > self.sent_round {
            self.sent_round = round;
            let channels = self
                .values
                .get(&CHANNELS)
                .split(',')
                .filter_map(|c| c.trim().parse::<f64>().ok())
                .map(|mhz| mhz * 1e6)
                .collect();
            out.requests.push(HostRequest::Hop {
                channels,
                dwell_s: self.values.get(&DWELL_S),
                stop_on_lock: self.values.get(&STOP_ON_LOCK),
            });
        }
        let fixed = self.values.get(&FIXED_HZ);
        if fixed > 0.0 && fixed != self.sent_fixed {
            self.sent_fixed = fixed;
            let width = self.values.get(&WIDTH_HZ);
            out.requests.push(if width > 0.0 {
                HostRequest::Tune {
                    session: String::new(),
                    freq_hz: Some(fixed),
                    target: sdrkit_core::TuneTarget::Running,
                    submode: String::new(),
                    rate_hz: None,
                    bandwidth_hz: Some(width),
                    settings: Vec::new(),
                }
            } else {
                HostRequest::retune(fixed, "fixed")
            });
        }
        let lock = self.values.get(&LOCK_HZ);
        let state = if lock > 0.0 && self.centre == lock {
            LockState::Locked
        } else {
            LockState::Searching
        };
        self.results.results_mut().update_status(|s| {
            let changed = s.lock_state != state;
            s.lock_state = state;
            changed
        });
        // After its requests: a held probe still sends them.
        if self.values.get(&HOLD_EACH) || (moved && self.values.get(&HOLD_MOVED)) {
            hold_point(self.tag());
        }
    }
}

impl Owner for Probe {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.values.set(id, value)
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
}

/// The test signal of the probe: a constant of the level of the radio centre.
struct Level {
    rate: f64,
    level: f32,
}

impl sdrkit_core::Generator for Level {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::new(self.level, 0.0));
        out.len()
    }
}

static PROBE: StackDescriptor = StackDescriptor {
    id: "probe",
    name: "Probe",
    submodes: &[],
    signal_types: &[],
    settings: ProbeSetting::schema,
    make_receiver: |cfg| {
        Ok(Box::new(Probe {
            results: Published::from_decl(Arc::new(ResultsDecl::new())),
            values: cfg.settings.clone(),
            centre: 0.0,
            tuned: 0.0,
            moved: false,
            sent_round: 0,
            sent_fixed: 0.0,
        }))
    },
    make_generator: Some(|cfg| {
        Ok(Box::new(Level {
            rate: cfg.rate_hz,
            level: level_of(cfg.tuned_hz().unwrap_or(100e6)),
        }))
    }),
    benchmarks: None,
    ..METER_DESC
};

/// An engine with the probe on `net` and `usb`.
fn engine_with(net: Arc<dyn Transport>, usb: Arc<dyn sdrkit_driver_usb::UsbBus>) -> Engine {
    Engine::with_usb(
        Setup {
            facilities: Facilities {
                storage: Arc::new(MemStorage::new()),
                clock: Clock::fixed(1.7e9),
                random: Random::fixed(7),
                codecs: Arc::default(),
            },
            transport: net,
            stacks: vec![&PROBE],
            families: Vec::new(),
            components: sdrkit_graph::Registry::standard(),
        },
        usb,
    )
    .unwrap()
}

fn engine() -> Engine {
    engine_with(
        Arc::new(FakeTransport::new()),
        Arc::new(sdrkit_driver_usb::NoUsb),
    )
}

/// The rate of the test-signal radio.
const RATE: f64 = 1e6;

/// The test-signal radio of the probe at [`RATE`], centred on 101 MHz, as fast as the stacks take the samples.
fn synth(e: &Engine) -> RadioId {
    let mut v = defaults(e, e.find("driver.synth").unwrap());
    for (key, value) in [
        ("rate", Value::Hz(RATE)),
        ("centre_hz", Value::Hz(101e6)),
        ("benchmark", Value::Bool(true)),
        ("quantize", Value::Bool(false)),
    ] {
        v.set(v.schema().id(key).unwrap(), &value).unwrap();
    }
    e.open_radio("synth", "synth:probe", &v).unwrap()
}

/// The settings of a probe with the log `tag` and `settings`.
fn probe(e: &Engine, tag: i64, settings: &[(&str, Value)]) -> Values {
    let mut v = defaults(e, e.find("stack.probe").unwrap());
    v.set(v.schema().id("tag").unwrap(), &Value::I64(tag))
        .unwrap();
    for (key, value) in settings {
        v.set(v.schema().id(key).unwrap(), value).unwrap();
    }
    v
}

/// Sets setting `key` of the stack of session `s`.
fn set(e: &Engine, s: SessionId, key: &str, v: Value) {
    e.owner(OwnerRef::Session(s), |o| {
        let id = o.schema().id(key).unwrap();
        sdrkit_core::owner::apply(o, id, &v)
    })
    .unwrap()
    .unwrap();
}

/// The samples that a probe was fed at one centre, in a row.
#[derive(Clone, Debug, PartialEq)]
struct Step {
    centre: f64,
    samples: u64,
    /// Samples whose level is not that of the centre.
    bad: u64,
    /// A gap came between the samples before and these.
    after_gap: bool,
    /// The probe was told the centre before these samples.
    told: bool,
}

/// The log as runs of samples at one centre.
fn steps(notes: &[Note]) -> Vec<Step> {
    let (mut out, mut gap, mut told) = (Vec::<Step>::new(), false, f64::NAN);
    for n in notes {
        match *n {
            Note::Centre(c) => told = c,
            Note::Tuned(_) => {}
            Note::Gap(_) => gap = true,
            Note::Feed { centre, n, bad } => {
                match out.last_mut() {
                    Some(s) if s.centre == centre => {
                        s.samples += n;
                        s.bad += bad;
                    }
                    _ => out.push(Step {
                        centre,
                        samples: n,
                        bad,
                        after_gap: gap,
                        told: told == centre,
                    }),
                }
                gap = false;
            }
        }
    }
    out
}

/// Waits until the probe `tag` has at least `n` runs of samples, and returns them.
fn wait_steps(tag: i64, n: usize) -> Vec<Step> {
    wait_for(30.0, || {
        let s = steps(&log(tag));
        (s.len() >= n).then_some(s)
    })
    .unwrap_or_else(|| {
        panic!(
            "probe {tag} got to fewer than {n} steps: {:?}",
            steps(&log(tag))
        )
    })
}

/// Waits until the last run of the probe `tag` is at `centre` with at least `samples` samples, and returns the runs.
fn wait_stay(tag: i64, centre: f64, samples: u64) -> Vec<Step> {
    wait_for(30.0, || {
        let s = steps(&log(tag));
        s.last()
            .is_some_and(|l| l.centre == centre && l.samples >= samples)
            .then_some(s)
    })
    .unwrap_or_else(|| {
        panic!(
            "probe {tag} did not stay at {centre}: {:?}",
            steps(&log(tag))
        )
    })
}

/// Checks the runs `s` of a hop over `channels` (Hz), each of `dwell` samples, from run `first` on, at channel
/// `from`: each starts with its centre told, the centres follow the channels in turn, and every run but the last has the
/// dwell to the sample. Each run after the first starts after a gap: the samples of the time that the radio took to
/// move, left out on purpose.
fn check_hop(s: &[Step], first: usize, channels: &[f64], from: usize, dwell: u64) {
    let hop = &s[first..];
    for (i, step) in hop.iter().enumerate() {
        assert_eq!(
            step.centre,
            channels[(from + i) % channels.len()],
            "step {i}: {s:?}"
        );
        assert!(step.told && (i == 0 || step.after_gap), "step {i}: {s:?}");
        if i + 1 < hop.len() {
            assert_eq!(step.samples, dwell, "step {i}: {s:?}");
        }
    }
}

/// The samples of one dwell at `rate`.
fn dwell_at(dwell_s: f64, rate: f64) -> u64 {
    (dwell_s * rate).round() as u64
}

/// Checks that session `s` and radio `r` lost nothing and that the session was told of gaps.
fn nothing_lost(e: &Engine, s: SessionId, r: RadioId) {
    // The count of gaps comes with the measurement of the load, once a second.
    let gaps = wait_for(10.0, || {
        let n = number(e, OwnerRef::Session(s), "session.gaps");
        (n > 0.0).then_some(n)
    });
    assert!(gaps.is_some(), "the session was told of no gap");
    assert_eq!(number(e, OwnerRef::Session(s), "session.dropped"), 0.0);
    assert_eq!(number(e, OwnerRef::Radio(r), "dropped"), 0.0);
    assert_eq!(number(e, OwnerRef::Radio(r), "radio_lost"), 0.0);
}

#[test]
fn a_hop_steps_through_its_channels_for_its_dwell_of_signal_time() {
    let e = engine();
    let r = synth(&e);
    let v = probe(&e, 1, &[("channels", Value::Text("100,102,104".into()))]);
    let a = e
        .start_stack("probe", None, &v, Input::Radio(r), Some(101e6))
        .unwrap();
    let dwell = dwell_at(0.2, RATE);
    let mhz = |v: &[f64]| v.iter().map(|f| f * 1e6).collect::<Vec<_>>();
    set(&e, a, "round", Value::I64(1));
    let s = wait_steps(1, 5);
    assert_eq!(s[0].centre, 101e6);
    check_hop(&s, 1, &mhz(&[100.0, 102.0, 104.0]), 0, dwell);
    // The same request again: the hop goes on where it is.
    set(&e, a, "round", Value::I64(2));
    let s = wait_steps(1, s.len() + 3);
    check_hop(&s, 1, &mhz(&[100.0, 102.0, 104.0]), 0, dwell);
    // Another request takes its place, at its first channel.
    set(&e, a, "channels", Value::Text("106,108".into()));
    let before = steps(&log(1)).len();
    set(&e, a, "round", Value::I64(3));
    let s = wait_steps(1, before + 5);
    let first = s
        .iter()
        .rposition(|x| x.centre == 104e6 || x.centre == 100e6 || x.centre == 102e6)
        .unwrap()
        + 1;
    check_hop(&s, first, &mhz(&[106.0, 108.0]), 0, dwell);
    // A fixed frequency ends the hop there.
    set(&e, a, "fixed_hz", Value::Hz(101e6));
    let ended = wait_stay(1, 101e6, 4 * dwell);
    assert!(ended.last().unwrap().after_gap, "{ended:?}");
    // Every sample had the level of the centre that the probe was told.
    assert!(ended.iter().all(|x| x.bad == 0), "{ended:?}");
    nothing_lost(&e, a, r);
    e.stop_session(a).unwrap();
    e.close_session(a).unwrap();
    e.close_radio(r).unwrap();
}

#[test]
fn a_second_reader_follows_the_hop_and_neither_it_nor_a_share_steers() {
    let e = engine();
    let r = synth(&e);
    let _share = e.open_share(r).unwrap();
    let channels = ("channels", Value::Text("100,102,104".into()));
    let a = e
        .start_stack(
            "probe",
            None,
            &probe(&e, 11, std::slice::from_ref(&channels)),
            Input::Radio(r),
            Some(101e6),
        )
        .unwrap();
    set(&e, a, "round", Value::I64(1));
    wait_steps(11, 3);
    let b = e
        .start_stack(
            "probe",
            None,
            &probe(&e, 12, &[channels]),
            Input::Radio(r),
            Some(101e6),
        )
        .unwrap();
    // The second reader asks for a hop of its own: refused.
    set(&e, b, "round", Value::I64(1));
    let why = wait_for(30.0, || {
        match field(&e, OwnerRef::Session(b), "session.request_problem") {
            Some(Value::Text(t)) => Some(t),
            _ => None,
        }
    })
    .expect("the hop of the second reader was refused");
    assert!(why.contains("steers"), "{why}");
    // A fixed frequency of the second reader does not end the hop of the first.
    set(&e, b, "fixed_hz", Value::Hz(101.2e6));
    let n = steps(&log(11)).len();
    let s = wait_steps(11, n + 4);
    let dwell = dwell_at(0.2, RATE);
    check_hop(&s, 1, &[100e6, 102e6, 104e6], 0, dwell);
    // The second reader is told every centre of the hop, after a gap.
    let sb = wait_steps(12, 4);
    for step in &sb[1..] {
        assert!([100e6, 102e6, 104e6].contains(&step.centre), "{sb:?}");
        assert!(step.told && step.after_gap, "{sb:?}");
    }
    // The hop ends with the session that steers: the radio stays.
    let at = steps(&log(12)).len();
    e.stop_session(a).unwrap();
    let stayed = wait_for(30.0, || {
        let s = steps(&log(12));
        s.last()
            .is_some_and(|l| s.len() >= at && l.samples >= 4 * dwell)
            .then_some(s)
    })
    .unwrap_or_else(|| panic!("the radio did not stay: {:?}", steps(&log(12))));
    assert!(stayed.iter().all(|x| x.bad == 0), "{stayed:?}");
    assert!(e.settle(std::time::Duration::from_secs(5)));
    assert_eq!(number(&e, OwnerRef::Session(b), "session.dropped"), 0.0);
    e.close_session(a).unwrap();
    e.stop_session(b).unwrap();
    e.close_session(b).unwrap();
    e.close_radio(r).unwrap();
}

#[test]
fn a_hop_with_stop_on_lock_ends_on_the_channel_where_the_stack_locks() {
    let e = engine();
    let r = synth(&e);
    let v = probe(
        &e,
        21,
        &[
            ("channels", Value::Text("100,102,104".into())),
            ("lock_hz", Value::Hz(102e6)),
            ("stop_on_lock", Value::Bool(true)),
        ],
    );
    let a = e
        .start_stack("probe", None, &v, Input::Radio(r), Some(101e6))
        .unwrap();
    set(&e, a, "round", Value::I64(1));
    let dwell = dwell_at(0.2, RATE);
    let s = wait_stay(21, 102e6, 4 * dwell);
    assert_eq!(
        s.iter().map(|x| x.centre).collect::<Vec<_>>(),
        [101e6, 100e6, 102e6],
        "{s:?}"
    );
    nothing_lost(&e, a, r);
    e.stop_session(a).unwrap();
    e.close_session(a).unwrap();
    e.close_radio(r).unwrap();
}

/// The frequencies of the commands that the client sent the stepped server, without repeats.
fn freq_commands(rtl: &SteppedRtl) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (c, v) in rtl.cmds() {
        if c == cmd::FREQ && out.last() != Some(&f64::from(v)) {
            out.push(f64::from(v));
        }
    }
    out
}

/// Steps a [`SteppedRtl`] for radio `r` and the probe `tag`: one block of 20 ms of signal at a time, each once the radio
/// has read the one before. After a frequency command the next block waits until the radio reports that centre, so the
/// retune has ended before a sample of the new frequency comes. While the probe is free, a block waits too until the probe
/// has taken in all but 1.8 s (at most 1.6 million) of the samples sent, so that its queue never overflows; it goes on
/// when a command comes, and when a second passes with no note of the probe, which then has taken in all that it can
/// get. A test that holds the probe ([`Stepper::hold`]) bounds what it sends meanwhile. The samples that a move leaves out
/// at most (0.56 s) go after a frequency command without a wait: the probe learns of them only after them.
struct Stepper<'a> {
    e: &'a Engine,
    a: SessionId,
    r: RadioId,
    rtl: &'a SteppedRtl,
    tag: i64,
    /// The commands of the server seen so far.
    seen: usize,
    /// The samples sent.
    sent: u64,
    /// The frequency commands seen, in order.
    freqs: Vec<f64>,
    /// The samples up to which blocks go without a wait for the probe.
    free_until: u64,
    /// The test holds the probe.
    held: bool,
}

impl Drop for Stepper<'_> {
    /// Frees the probe, so that a test that fails ends.
    fn drop(&mut self) {
        hold(self.tag, false);
    }
}

impl<'a> Stepper<'a> {
    fn new(e: &'a Engine, a: SessionId, r: RadioId, rtl: &'a SteppedRtl, tag: i64) -> Self {
        Stepper {
            e,
            a,
            r,
            rtl,
            tag,
            seen: 0,
            sent: 0,
            freqs: Vec::new(),
            free_until: 0,
            held: false,
        }
    }

    /// Holds the probe (`on`) or frees it ([`hold`]).
    fn hold(&mut self, on: bool) {
        self.held = on;
        hold(self.tag, on);
    }

    /// The state of the radio and the session, for a failure: connections, samples that the radio lost, samples that
    /// the queue of the session dropped, and the log of the probe.
    fn state(&self) -> String {
        format!(
            "connections {}, radio lost {}, session dropped {}, commands {:?}: {:?}",
            self.rtl.connections(),
            number(self.e, OwnerRef::Radio(self.r), "radio_lost"),
            number(self.e, OwnerRef::Session(self.a), "session.dropped"),
            self.rtl.cmds(),
            log(self.tag)
        )
    }

    /// Sends one block.
    fn step(&mut self) {
        assert!(
            self.rtl.wait_read(Duration::from_secs(60)),
            "the radio did not read the samples sent: {}",
            self.state()
        );
        let rate = self.rtl.rate_hz().max(225e3);
        let block = (rate * 0.02) as usize;
        let cmds = self.rtl.cmds();
        let new: Vec<f64> = cmds[self.seen..]
            .iter()
            .filter(|c| c.0 == cmd::FREQ)
            .map(|c| f64::from(c.1))
            .collect();
        self.seen = cmds.len();
        if let Some(&hz) = new.last() {
            let (e, r) = (self.e, self.r);
            wait_for(60.0, || {
                (number(e, OwnerRef::Radio(r), "centre_hz") == hz).then_some(())
            })
            .unwrap_or_else(|| panic!("the radio did not report {hz} Hz: {}", self.state()));
            self.freqs.extend(new);
            self.free_until = self.sent + (0.56 * rate) as u64 + block as u64;
        }
        if !self.held && self.sent >= self.free_until {
            self.wait_probe((1.8 * rate).min(1.6e6) as u64);
        }
        self.rtl.send(block);
        self.sent += block as u64;
    }

    /// Waits until the probe has taken in all but `window` of the samples sent, a command comes, or a second passes
    /// with no note of the probe.
    fn wait_probe(&self, window: u64) {
        let (tag, sent, seen) = (self.tag, self.sent, self.seen);
        let command = || self.rtl.cmds().len() > seen;
        for _ in 0..60 {
            let at = accounted(&log(tag));
            if sent < at + window || command() {
                return;
            }
            let noted = wait_log(tag, Duration::from_secs(1), |l| {
                (accounted(l) != at || command()).then_some(())
            });
            if noted.is_none() {
                return;
            }
        }
        panic!(
            "probe {tag} took in too few of {sent} samples: {}",
            self.state()
        );
    }

    /// Sends blocks until `done` gives a value, and returns it.
    fn until<T>(&mut self, mut done: impl FnMut(&Self) -> Option<T>) -> T {
        loop {
            if let Some(v) = done(self) {
                return v;
            }
            assert!(
                (self.sent as f64) < 600.0 * self.rtl.rate_hz().max(225e3),
                "probe {} did not get there in 10 minutes of signal: {}",
                self.tag,
                self.state()
            );
            self.step();
        }
    }

    /// Sends blocks until the probe has at least `n` runs of samples, and returns them.
    fn steps(&mut self, n: usize) -> Vec<Step> {
        let tag = self.tag;
        self.until(|_| {
            let s = steps(&log(tag));
            (s.len() >= n).then_some(s)
        })
    }

    /// Sends blocks until the last run of the probe is at `centre` with at least `samples` samples, and returns the runs.
    fn stay(&mut self, centre: f64, samples: u64) -> Vec<Step> {
        let tag = self.tag;
        self.until(|_| {
            let s = steps(&log(tag));
            s.last()
                .is_some_and(|l| l.centre == centre && l.samples >= samples)
                .then_some(s)
        })
    }

    /// Sends blocks until the probe was told that it listens to `hz`.
    fn tuned(&mut self, hz: f64) {
        let tag = self.tag;
        self.until(|_| log(tag).contains(&Note::Tuned(hz)).then_some(()));
    }

    /// Sends blocks until the probe was told the centre of each frequency command: one change of the centre for each
    /// command to a frequency other than the one before.
    fn told_last(&mut self) {
        let tag = self.tag;
        self.until(|st| {
            let mut moves = st.freqs.clone();
            moves.dedup();
            let told = log(tag)
                .iter()
                .filter(|n| matches!(n, Note::Centre(_)))
                .count();
            (told >= moves.len()).then_some(())
        });
    }
}

/// Opens the radio of the stepped server `rtl` with `radio` on the engine `e` and starts the probe `tag` with `settings`
/// at `centre`.
fn stepped_probe(
    e: &Engine,
    rtl: &SteppedRtl,
    radio: &Values,
    tag: i64,
    settings: &[(&str, Value)],
    centre: f64,
) -> (SessionId, RadioId) {
    let r = e.open_radio("rtl_tcp", &rtl.addr(), radio).unwrap();
    let a = e
        .start_stack(
            "probe",
            None,
            &probe(e, tag, settings),
            Input::Radio(r),
            Some(centre),
        )
        .unwrap();
    (a, r)
}

/// An engine on the stepped server `rtl` and the default settings of its radio.
fn stepped_engine(rtl: &SteppedRtl) -> (Engine, Values) {
    let e = engine_with(rtl.transport(), Arc::new(sdrkit_driver_usb::NoUsb));
    let radio = defaults(&e, e.find("driver.rtl_tcp").unwrap());
    (e, radio)
}

/// Ends session `a` and closes radio `r`; the queue of the session dropped no sample.
fn end(e: &Engine, a: SessionId, r: RadioId) {
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(number(e, OwnerRef::Session(a), "session.dropped"), 0.0);
    e.stop_session(a).unwrap();
    e.close_session(a).unwrap();
    e.close_radio(r).unwrap();
}

#[test]
fn a_hop_of_the_fake_rtl_tcp_server_moves_it_in_turn_and_loses_nothing() {
    let rtl = SteppedRtl::start(Duration::ZERO);
    let (e, radio) = stepped_engine(&rtl);
    let settings = [
        ("channels", Value::Text("100,103,106".into())),
        ("dwell_s", Value::F64(0.1)),
    ];
    let (a, r) = stepped_probe(&e, &rtl, &radio, 31, &settings, 101e6);
    let mut st = Stepper::new(&e, a, r, &rtl, 31);
    set(&e, a, "round", Value::I64(1));
    let s = st.steps(6);
    let channels = [100e6, 103e6, 106e6];
    // The dwell counts the samples at the rate that the radio reports.
    let rate = number(&e, OwnerRef::Radio(r), "rate_hz");
    assert!(rate > 0.0);
    check_hop(&s, 1, &channels, 0, dwell_at(0.1, rate));
    let sent = freq_commands(&rtl);
    // The radio opened at its default frequency, then went to that of the session, then hopped.
    let hop = &sent[sent.iter().position(|f| *f == 101e6).unwrap() + 1..];
    assert!(hop.len() >= 5, "{sent:?}");
    for (i, f) in hop.iter().enumerate() {
        assert_eq!(*f, channels[i % 3], "{sent:?}");
    }
    nothing_lost(&e, a, r);
    end(&e, a, r);
}

#[test]
fn a_hop_of_a_scripted_dongle_moves_it_in_turn_and_loses_nothing() {
    let e = engine_with(Arc::new(FakeTransport::new()), Arc::new(dongle_bus()));
    assert!(e.settle(std::time::Duration::from_secs(5)));
    let kind = e.find("driver.rtlusb").unwrap();
    let r = e.open_radio("rtlusb", "", &defaults(&e, kind)).unwrap();
    let v = probe(
        &e,
        41,
        &[
            ("channels", Value::Text("100,103,106".into())),
            ("dwell_s", Value::F64(0.1)),
        ],
    );
    let a = e
        .start_stack("probe", None, &v, Input::Radio(r), Some(101e6))
        .unwrap();
    set(&e, a, "round", Value::I64(1));
    let s = wait_steps(41, 6);
    let rate = number(&e, OwnerRef::Radio(r), "rate_hz");
    assert!(rate > 0.0);
    // The dongle flushes at each retune: that gap is made on purpose too, inside the step.
    check_hop(&s, 1, &[100e6, 103e6, 106e6], 0, dwell_at(0.1, rate));
    nothing_lost(&e, a, r);
    e.stop_session(a).unwrap();
    e.close_session(a).unwrap();
    e.close_radio(r).unwrap();
}

/// The time that the tuner of the stepped server takes to move: the samples of that time after a frequency command, and
/// those on their way, are those of the frequency before.
const TUNE_TAKES: Duration = Duration::from_millis(10);

/// Starts a probe with the log `tag` on the stepped server `rtl` at 100 MHz, with `settings`, and hops it over 100, 103
/// and 106 MHz once the probe has 0.3 s of samples.
fn hop_slow_tuner<'a>(
    e: &'a Engine,
    rtl: &'a SteppedRtl,
    radio: &Values,
    tag: i64,
    settings: &[(&str, Value)],
) -> (Stepper<'a>, SessionId, RadioId) {
    let mut all = vec![
        ("channels", Value::Text("100,103,106".into())),
        ("dwell_s", Value::F64(0.1)),
        ("rtl_levels", Value::Bool(true)),
    ];
    all.extend_from_slice(settings);
    let (a, r) = stepped_probe(e, rtl, radio, tag, &all, 100e6);
    let mut st = Stepper::new(e, a, r, rtl, tag);
    // The first samples of the stream come before the tuner is at its first frequency.
    let rate = rtl.rate_hz().max(225e3);
    st.stay(100e6, (0.3 * rate) as u64);
    set(e, a, "round", Value::I64(1));
    // The probe sends the hop at its next block; the hop has begun once the radio is at its second channel.
    st.until(|_| log(tag).contains(&Note::Centre(103e6)).then_some(()));
    (st, a, r)
}

#[test]
fn a_step_starts_at_the_first_sample_that_the_radio_captured_at_the_new_frequency() {
    let rtl = SteppedRtl::start(TUNE_TAKES);
    let (e, radio) = stepped_engine(&rtl);
    let (mut st, a, r) = hop_slow_tuner(&e, &rtl, &radio, 51, &[]);
    let s = st.steps(8);
    // The runs of the hop: from the second run on, no sample has the level of another frequency. The server sent
    // samples of the frequency before for 10 ms after each command, and none of them reached the stack.
    let first = s.iter().position(|x| x.centre == 103e6).unwrap();
    let rate = number(&e, OwnerRef::Radio(r), "rate_hz");
    check_hop(&s, first, &[100e6, 103e6, 106e6], 1, dwell_at(0.1, rate));
    for step in &s[first..] {
        assert_eq!(step.bad, 0, "{s:?}");
    }
    assert_eq!(number(&e, OwnerRef::Radio(r), "radio_lost"), 0.0);
    end(&e, a, r);
}

/// Checks the log of a probe on the stepped server: after the first run, the stack is told a new centre and the
/// frequency of that centre together, the next samples are at that centre, and they have its level. At least `least`
/// changes are checked.
fn told_with_the_samples(notes: &[Note], least: usize) {
    let mut told = None;
    let mut checked = 0;
    for (i, n) in notes.iter().enumerate() {
        match *n {
            Note::Centre(c) if i > 0 => {
                assert_eq!(notes.get(i + 1), Some(&Note::Tuned(c)), "{notes:?}");
            }
            Note::Tuned(hz) => told = Some(hz),
            Note::Feed { centre, bad, .. } => {
                if let Some(hz) = told.take() {
                    assert_eq!(centre, hz, "{notes:?}");
                    if checked > 0 {
                        assert_eq!(bad, 0, "{notes:?}");
                    }
                    checked += 1;
                }
            }
            _ => {}
        }
    }
    assert!(checked >= least, "{notes:?}");
}

/// Runs the hop of `st` in rounds until the probe has `runs` runs: in each, the test holds the probe until the radio has
/// moved `moves` times and once more, then frees it until it was told the last centre. The engine moves the radio only
/// after the dwell of a step, so the move after the last of the `moves` shows that the first sample of each is in the
/// stream.
fn hold_through_moves(st: &mut Stepper<'_>, moves: usize, runs: usize) {
    let tag = st.tag;
    while steps(&log(tag)).len() < runs {
        st.hold(true);
        let from = st.freqs.len();
        st.until(|st| (st.freqs.len() > from + moves).then_some(()));
        st.hold(false);
        st.told_last();
    }
}

#[test]
fn a_stack_that_takes_two_changes_of_frequency_at_once_is_told_each_at_its_first_sample() {
    let rtl = SteppedRtl::start(TUNE_TAKES);
    let (e, radio) = stepped_engine(&rtl);
    // The probe waits at the first block of each step until the radio has moved twice more: the two changes of
    // frequency wait for it together.
    let (mut st, a, r) = hop_slow_tuner(&e, &rtl, &radio, 53, &[("hold_moved", Value::Bool(true))]);
    hold_through_moves(&mut st, 2, 7);
    told_with_the_samples(&log(53), 5);
    end(&e, a, r);
}

#[test]
fn a_retune_without_a_hop_starts_at_the_first_sample_of_the_new_frequency() {
    let rtl = SteppedRtl::start(TUNE_TAKES);
    let (e, radio) = stepped_engine(&rtl);
    let (a, r) = stepped_probe(
        &e,
        &rtl,
        &radio,
        54,
        &[("rtl_levels", Value::Bool(true))],
        100e6,
    );
    let mut st = Stepper::new(&e, a, r, &rtl, 54);
    st.stay(100e6, 60_000);
    // The stack asks for 103 MHz, then the host sets 106 MHz.
    set(&e, a, "fixed_hz", Value::Hz(103e6));
    st.stay(103e6, 20_000);
    set(&e, a, "tuned_hz", Value::Hz(106e6));
    let s = st.stay(106e6, 20_000);
    assert_eq!(s.len(), 3, "{s:?}");
    for step in &s[1..] {
        assert!(step.told && step.after_gap && step.bad == 0, "{s:?}");
    }
    told_with_the_samples(&log(54), 2);
    end(&e, a, r);
}

#[test]
fn a_stack_request_moves_the_radio_only_when_the_capture_does_not_hold_its_width() {
    let rtl = SteppedRtl::start(TUNE_TAKES);
    let (e, mut radio) = stepped_engine(&rtl);
    radio
        .set(radio.schema().id("rate").unwrap(), &Value::Hz(2.4e6))
        .unwrap();
    let (a, r) = stepped_probe(
        &e,
        &rtl,
        &radio,
        56,
        &[("rtl_levels", Value::Bool(true))],
        100e6,
    );
    let mut st = Stepper::new(&e, a, r, &rtl, 56);
    st.stay(100e6, 40_000);
    assert_eq!(number(&e, OwnerRef::Radio(r), "rate_hz"), 2.4e6);
    let sent = freq_commands(&rtl);
    // At 2.4 Msps the capture holds 0.96 MHz either side of the centre. A request without a width inside it, and one
    // whose width fits (100.3 MHz with 0.5 MHz), move the session only.
    set(&e, a, "fixed_hz", Value::Hz(100.2e6));
    st.tuned(100.2e6);
    set(&e, a, "width_hz", Value::Hz(0.5e6));
    set(&e, a, "fixed_hz", Value::Hz(100.3e6));
    st.tuned(100.3e6);
    let at = steps(&log(56)).last().unwrap().samples;
    let s = st.stay(100e6, at + 40_000);
    assert_eq!(s.len(), 1, "{s:?}");
    assert_eq!(freq_commands(&rtl), sent);
    // 100.7 MHz with 1.3 MHz reaches 1.35 MHz from the centre: the radio moves there, and the stack is told at the
    // first sample of the new centre.
    set(&e, a, "width_hz", Value::Hz(1.3e6));
    set(&e, a, "fixed_hz", Value::Hz(100.7e6));
    let s = st.stay(100.7e6, 20_000);
    assert_eq!(s.len(), 2, "{s:?}");
    assert!(s[1].told && s[1].after_gap && s[1].bad == 0, "{s:?}");
    let notes = log(56);
    let i = notes
        .iter()
        .position(|n| *n == Note::Centre(100.7e6))
        .unwrap();
    assert_eq!(notes.get(i + 1), Some(&Note::Tuned(100.7e6)), "{notes:?}");
    assert_eq!(freq_commands(&rtl).last(), Some(&100.7e6));
    // The host tunes inside the capture: the session listens there and the radio stays.
    let sent = freq_commands(&rtl);
    set(&e, a, "tuned_hz", Value::Hz(100.9e6));
    st.tuned(100.9e6);
    let at = steps(&log(56)).last().unwrap().samples;
    let s = st.stay(100.7e6, at + 40_000);
    assert_eq!(s.len(), 2, "{s:?}");
    assert_eq!(freq_commands(&rtl), sent);
    end(&e, a, r);
}

#[test]
fn a_stack_that_is_behind_is_told_the_new_frequency_at_its_first_sample() {
    let rtl = SteppedRtl::start(TUNE_TAKES);
    let (e, radio) = stepped_engine(&rtl);
    // The probe waits at each block until the radio has moved: blocks of a step still wait in its queue when the
    // engine moves the radio.
    let (mut st, a, r) = hop_slow_tuner(&e, &rtl, &radio, 52, &[("hold_each", Value::Bool(true))]);
    hold_through_moves(&mut st, 1, 8);
    told_with_the_samples(&log(52), 6);
    end(&e, a, r);
}

#[test]
fn a_hop_inside_one_capture_tells_each_frequency_at_the_first_sample_of_its_step() {
    let rtl = SteppedRtl::start(Duration::ZERO);
    let (e, radio) = stepped_engine(&rtl);
    // The channels lie 20 kHz apart: the radio stays where it is. The probe waits at the start of each step until
    // the radio has passed two more dwells, so the blocks and the changes of frequency wait for it.
    let settings = [
        ("channels", Value::Text("100,100.02,100.04".into())),
        ("dwell_s", Value::F64(0.1)),
        ("hold_tuned", Value::Bool(true)),
        ("hold_moved", Value::Bool(true)),
    ];
    let (a, r) = stepped_probe(&e, &rtl, &radio, 55, &settings, 100e6);
    let mut st = Stepper::new(&e, a, r, &rtl, 55);
    set(&e, a, "round", Value::I64(1));
    let tuned = |n: &[Note]| n.iter().filter(|x| matches!(x, Note::Tuned(_))).count();
    while tuned(&log(55)) < 7 {
        st.hold(true);
        let to = st.sent + 2 * dwell_at(0.1, rtl.rate_hz().max(225e3));
        st.until(|st| (st.sent >= to).then_some(()));
        st.hold(false);
        let k = tuned(&log(55));
        st.until(|_| (tuned(&log(55)) >= k + 2).then_some(()));
    }
    let notes = log(55);
    let rate = number(&e, OwnerRef::Radio(r), "rate_hz");
    // Each frequency of the hop is told once, in turn; then come the gap of the step, if samples were left out, and
    // the samples of its dwell, to the sample.
    let told: Vec<usize> = (0..notes.len())
        .filter(|i| matches!(notes[*i], Note::Tuned(_)))
        .collect();
    let channels = [100e6, 100.02e6, 100.04e6];
    let dwell = dwell_at(0.1, rate);
    for (k, i) in told[1..told.len() - 1].iter().enumerate() {
        assert_eq!(notes[*i], Note::Tuned(channels[(k + 1) % 3]), "{notes:?}");
        let step: Vec<&Note> = notes[i + 1..told[k + 2]]
            .iter()
            .skip_while(|n| matches!(n, Note::Gap(_)))
            .collect();
        assert!(
            matches!(step[..], [Note::Feed { n, .. }] if *n == dwell),
            "{notes:?}"
        );
    }
    end(&e, a, r);
}
