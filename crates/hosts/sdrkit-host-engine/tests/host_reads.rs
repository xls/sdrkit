//! The reads of a host beside a running session: how long each read takes, and what the reads cost the session.

mod common;

use common::*;
use sdrkit_core::publish::{field, TableDef};
use sdrkit_core::{
    Choice, Complex32, ConfigError, Event, EventDef, Facilities, Field, InputInfo, MemStorage,
    Output, Owner, Published, Random, Receiver, Results, ResultsDecl, SettingError, SettingId,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_host_engine::{Engine, Input, OwnerRef, RadioId, SessionId, Setup, DROPPED, SAMPLES};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The latencies of one kind of read, in milliseconds, each with the time it was made at.
#[derive(Default)]
struct Latencies(Vec<(f64, f64)>);

impl Latencies {
    fn time<R>(&mut self, start: Instant, f: impl FnOnce() -> R) -> R {
        let t = Instant::now();
        let r = f();
        self.0
            .push(((t - start).as_secs_f64(), t.elapsed().as_secs_f64() * 1e3));
        r
    }

    fn max(&self) -> f64 {
        self.0.iter().map(|x| x.1).fold(0.0, f64::max)
    }

    fn p99(&self) -> f64 {
        let mut v: Vec<f64> = self.0.iter().map(|x| x.1).collect();
        v.sort_by(f64::total_cmp);
        v.get(v.len() * 99 / 100).copied().unwrap_or(0.0)
    }

    /// The largest latency of each second.
    fn per_second(&self) -> Vec<f64> {
        let mut out = Vec::new();
        for &(at, ms) in &self.0 {
            let i = at as usize;
            if out.len() <= i {
                out.resize(i + 1, 0.0);
            }
            out[i] = f64::max(out[i], ms);
        }
        out
    }
}

/// What a host reads of a session on each frame, each kind timed on its own.
#[derive(Default)]
struct Reads {
    session: Latencies,
    stack: Latencies,
    spectrum: Latencies,
    events: Latencies,
    values: Latencies,
    audio: Latencies,
    /// The results of the engine, the radios, a service and a component.
    other: Latencies,
    other_copy: Results,
    session_copy: Results,
    stack_copy: Results,
    bins: Vec<f32>,
    taken: Vec<Event>,
    sound: Vec<f32>,
}

impl Reads {
    /// One frame of a host: the results of the session and of its stack, the spectrum, the events, every setting value
    /// and the audio.
    fn frame(&mut self, e: &Engine, id: SessionId, start: Instant) {
        let (s, k) = (OwnerRef::Session(id), OwnerRef::Stack(id));
        let copy = &mut self.session_copy;
        self.session.time(start, || {
            e.owner(s, |o| copy.clone_from(o.results())).unwrap();
        });
        let copy = &mut self.stack_copy;
        self.stack.time(start, || {
            e.owner(k, |o| copy.clone_from(o.results())).unwrap();
        });
        self.spectrum.time(start, || {
            e.spectrum(id, &mut self.bins).unwrap();
        });
        self.events.time(start, || {
            e.take_events(s, &mut self.taken).unwrap();
            e.take_events(OwnerRef::Engine, &mut self.taken).unwrap();
        });
        self.taken.clear();
        for r in [s, k] {
            let n = e.owner(r, |o| o.schema().len()).unwrap();
            for i in 0..n {
                self.values.time(start, || {
                    e.owner(r, |o| {
                        o.values()
                            .and_then(|v| v.value(sdrkit_core::SettingId(i as u16)).cloned())
                    })
                    .unwrap()
                });
            }
        }
        self.sound.resize(1 << 15, 0.0);
        self.audio.time(start, || {
            e.audio(id, &mut self.sound).unwrap();
        });
    }

    fn kinds(&self) -> [(&'static str, &Latencies); 7] {
        [
            ("other owners", &self.other),
            ("session results", &self.session),
            ("stack results", &self.stack),
            ("spectrum", &self.spectrum),
            ("events", &self.events),
            ("setting value", &self.values),
            ("audio", &self.audio),
        ]
    }

    /// The number of reads that took over `ms` milliseconds.
    fn slow(&self, ms: f64) -> usize {
        self.kinds()
            .iter()
            .map(|k| k.1 .0.iter().filter(|x| x.1 > ms).count())
            .sum()
    }

    fn print(&self) {
        for (name, l) in self.kinds() {
            let per: Vec<String> = l.per_second().iter().map(|x| format!("{x:.1}")).collect();
            println!(
                "READ {name:16} n={:6} p99={:.3} ms max={:.3} ms per second: {}",
                l.0.len(),
                l.p99(),
                l.max(),
                per.join(" ")
            );
        }
    }
}

fn set(e: &Engine, r: OwnerRef, key: &str, v: Value) {
    e.owner(r, |o| {
        let id = o.schema().id(key).unwrap();
        sdrkit_core::owner::apply(o, id, &v)
    })
    .unwrap()
    .unwrap();
}

/// An engine with the DVB-T stack on the system clock.
fn dvbt_engine() -> Engine {
    let mut components = sdrkit_graph::Registry::standard();
    sdrkit_stack_dvbt::register(&mut components);
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(sdrkit_core::net::FakeTransport::new()),
        stacks: vec![&sdrkit_stack_dvbt::DVBT],
        families: Vec::new(),
        components,
    })
    .unwrap()
}

/// Prints the latency of every read of a host at 15 frames a second beside a DVB-T session on its test signal in a 7 MHz
/// channel (2K 16-QAM, or 8K 64-QAM with `SDRKIT_GEN_8K`) for `SDRKIT_MEASURE_S` seconds (30 if not set), and the samples
/// the session read and lost. `SDRKIT_NO_READER` runs the session with no read.
#[test]
#[ignore = "a measurement, not a check"]
fn measure_the_reads_of_a_host_beside_dvbt() {
    use sdrkit_stack_dvbt::params::{Bandwidth, CodeRate, Constellation, Guard, Mode};
    let secs: f64 = std::env::var("SDRKIT_MEASURE_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30.0);
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    let heavy = std::env::var_os("SDRKIT_GEN_8K").is_some();
    let (mode, qam) = if heavy {
        (Mode::K8, Constellation::Qam64)
    } else {
        (Mode::K2, Constellation::Qam16)
    };
    for (key, x) in [
        ("gen_mode", mode.index()),
        ("gen_guard", Guard::G8.index()),
        ("gen_constellation", qam.index()),
        ("gen_rate_hp", CodeRate::R23.index()),
        ("gen_bandwidth", Bandwidth::B7.index()),
    ] {
        set(&e, OwnerRef::Radio(radio), key, Value::Choice(x));
    }
    let reader = std::env::var_os("SDRKIT_NO_READER").is_none();
    let values = defaults(&e, e.find("stack.dvbt").unwrap());
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    let s = OwnerRef::Session(id);
    set(&e, s, "spectrum", Value::Bool(true));
    let start = Instant::now();
    let mut reads = Reads::default();
    let frame = Duration::from_millis(66);
    let mut next = start;
    let mut heard = 0usize;
    let mut sound = vec![0.0f32; 1 << 15];
    while start.elapsed().as_secs_f64() < secs {
        if reader {
            reads.frame(&e, id, start);
        }
        while let Ok(Some(a)) = e.audio(id, &mut sound) {
            heard += a.frames;
            if a.frames == 0 {
                break;
            }
        }
        next += frame;
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    reads.print();
    // Each read that took over 5 ms: when, and how long.
    for (name, l) in reads.kinds() {
        let late: Vec<String> =
            l.0.iter()
                .filter(|x| x.1 > 5.0)
                .map(|x| format!("{:.2}s:{:.0}", x.0, x.1))
                .collect();
        println!("LATE {name}: {}", late.join(" "));
    }
    println!(
        "SESSION audio frames={heard} audio lost={} samples={} dropped={} state={:?} summary={:?}",
        number(&e, s, sdrkit_host_engine::AUDIO_LOST.key),
        number(&e, s, SAMPLES.key),
        number(&e, s, DROPPED.key),
        e.owner(s, |o| o.results().status().lock_state).unwrap(),
        e.owner(s, |o| o.results().status().summary.clone())
            .unwrap(),
    );
}

/// The number of the block that the pacer was fed last.
const N: Field<field::I64> = Field::new("n", "Block", "");
/// 30 per block that the pacer is fed, numbered from 1 over the whole session.
static TICK: EventDef = EventDef::new("pacer.tick", "Tick", &[&N]);
const TICKS: i64 = 30;
/// 16 rows. Each block writes its number into one of them.
static ROWS: TableDef = TableDef {
    key: "rows",
    label: "Rows",
    columns: &[&N],
    cap: 16,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The blocks that the pacers were fed.
static FEEDS: AtomicU64 = AtomicU64::new(0);
/// While set, a pacer stays in the block it works on. `HELD` says that one does.
static HOLD: AtomicBool = AtomicBool::new(false);
static HELD: AtomicBool = AtomicBool::new(false);
/// A pacer stays in its block of this number and in every later one, until the number is raised. `HELD_AT` is the number
/// of the block where it stays.
static STOP_AT: AtomicI64 = AtomicI64::new(i64::MAX);
static HELD_AT: AtomicI64 = AtomicI64::new(0);
/// The tests of this file that run a pacer run one after the other.
static ALONE: Mutex<()> = Mutex::new(());

/// A stack that does a fixed amount of work on each block, counts the blocks and raises one event per block.
struct Pacer {
    results: Published,
    values: Values,
    n: i64,
}

impl Receiver for Pacer {
    fn configure(&mut self, _: &InputInfo) -> Result<(), ConfigError> {
        Ok(())
    }
    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        let t = Instant::now();
        let mut power = 0.0;
        // About 10 ms: longer than a read of the host may take.
        for _ in 0..640 {
            power += iq.iter().map(|s| s.norm_sqr()).sum::<f32>();
        }
        std::hint::black_box(power);
        self.n += 1;
        self.results.set(&N, self.n);
        let key = format!("r{}", self.n % 16);
        self.results.row(&ROWS, &key, vec![Value::I64(self.n)]);
        for i in 1..=TICKS {
            let number = (self.n - 1) * TICKS + i;
            out.events.push(TICK.event(0.0, 0.0).set(&N, number));
        }
        FEEDS.fetch_add(1, Ordering::Relaxed);
        // A test holds the block, for a minute at most.
        let held = || HOLD.load(Ordering::Acquire) || self.n >= STOP_AT.load(Ordering::Acquire);
        while held() && t.elapsed() < Duration::from_secs(60) {
            HELD_AT.store(self.n, Ordering::Release);
            HELD.store(true, Ordering::Release);
            std::thread::sleep(Duration::from_millis(1));
        }
        HELD.store(false, Ordering::Release);
    }
}

impl Owner for Pacer {
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

/// The pacer stack, at `RATE` samples a second.
static PACER: StackDescriptor = StackDescriptor {
    id: "pacer",
    name: "Pacer",
    tuning: Tuning::anywhere(100e6, RATE, 1e3, 10e3),
    submodes: &[],
    make_receiver: |cfg| {
        let decl = ResultsDecl::new().with_fields(&[&N]).event(&TICK);
        Ok(Box::new(Pacer {
            results: Published::from_decl(Arc::new(decl)).with_tables(&[&ROWS]),
            values: cfg.settings.clone(),
            n: 0,
        }))
    },
    ..METER_DESC
};
/// About 15 blocks a second.
const RATE: f64 = 250e3;

/// An engine with the pacer stack and the FM stack, and a pacer session on the tone signal with its spectrum on.
fn pacer_engine() -> (Engine, RadioId, SessionId) {
    pacer_engine_paced(false)
}

/// As [`pacer_engine`]; with `benchmark` the tone plays as fast as the pacer takes it, so a block always waits for the
/// session.
fn pacer_engine_paced(benchmark: bool) -> (Engine, RadioId, SessionId) {
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(sdrkit_core::net::FakeTransport::new()),
        stacks: vec![&PACER, &TONE, &sdrkit_stack_fm::DESCRIPTOR],
        families: Vec::new(),
        components: sdrkit_stack_fm::graph::registry(),
    })
    .unwrap();
    let radio = tone_radio(&e, RATE);
    if benchmark {
        set(&e, OwnerRef::Radio(radio), "benchmark", Value::Bool(true));
    }
    let values = Values::defaults(&(PACER.settings)());
    let id = e
        .start_stack("pacer", None, &values, Input::Radio(radio), None)
        .unwrap();
    set(&e, OwnerRef::Session(id), "spectrum", Value::Bool(true));
    wait_for(10.0, || {
        (number(&e, OwnerRef::Session(id), N.key) > 0.0).then_some(())
    })
    .expect("the pacer is fed");
    (e, radio, id)
}

/// Waits up to 10 s for the pacers to take `n` more blocks. Returns false if they did not.
fn pacers_go_on(n: u64) -> bool {
    let at = FEEDS.load(Ordering::Relaxed);
    wait_for(10.0, || {
        (FEEDS.load(Ordering::Relaxed) >= at + n).then_some(())
    })
    .is_some()
}

/// A host thread that reads a session and every other owner in a tight loop does not slow the session, and a host that
/// stops inside a read does not hold it up. The pacer takes every block as soon as it can, so its rate shows any wait
/// of the session thread. Phases with the reads alternate with control phases in which the same thread spins without
/// a read, so the processor load, the cores and the clock speed are alike in both. Each phase with the reads is set
/// against the mean of the control phases before and after it, and the median of these ratios counts: the scheduler
/// slows single phases, reads that cost the session time slow every phase.
#[test]
fn a_reader_in_a_tight_loop_does_not_slow_the_session() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, radio, id) = pacer_engine_paced(true);
    // A session of a graph, for the reads of a stack with components, tables and audio.
    let synth = e.find("driver.synth").unwrap();
    let fm_radio = e
        .open_radio("synth", "synth:fm", &defaults(&e, synth))
        .unwrap();
    let fm_values = Values::defaults(&(sdrkit_stack_fm::DESCRIPTOR.settings)());
    let fm = e
        .start_stack("fm", None, &fm_values, Input::Radio(fm_radio), None)
        .unwrap();
    set(&e, OwnerRef::Session(fm), "spectrum", Value::Bool(true));
    let name = e.stack_graph("fm").unwrap().components[0].id.clone();
    let component = OwnerRef::Component(fm, e.component(fm, &name).unwrap());
    let others = [
        OwnerRef::Engine,
        OwnerRef::Radio(radio),
        OwnerRef::Radio(fm_radio),
        e.find("service.record").unwrap(),
        component,
    ];
    // A host that stops inside a read of each owner: the session takes its blocks meanwhile.
    for r in [OwnerRef::Session(id), OwnerRef::Stack(id)]
        .into_iter()
        .chain(others)
    {
        assert!(
            e.owner(r, |_| pacers_go_on(20)).unwrap(),
            "a read of {r:?} held up the session"
        );
    }
    const PHASES: usize = 13;
    const PHASE: Duration = Duration::from_millis(750);
    let stop = AtomicBool::new(false);
    let reading = AtomicBool::new(false);
    // The blocks a second of each phase. The reader reads in the odd ones.
    let mut rates: Vec<f64> = Vec::new();
    let (reads, slow_pauses) = std::thread::scope(|scope| {
        // The pauses over 5 ms of a thread that calls nothing, while the reader reads: what the scheduler takes.
        let idle = scope.spawn(|| {
            let (mut last, mut n) = (Instant::now(), 0usize);
            while !stop.load(Ordering::Relaxed) {
                let now = Instant::now();
                if reading.load(Ordering::Relaxed) && now - last > Duration::from_millis(5) {
                    n += 1;
                }
                last = now;
            }
            n
        });
        let reader = scope.spawn(|| {
            let mut reads = Reads::default();
            let start = Instant::now();
            while !stop.load(Ordering::Relaxed) {
                if !reading.load(Ordering::Relaxed) {
                    std::hint::spin_loop();
                    continue;
                }
                reads.frame(&e, id, start);
                reads.frame(&e, fm, start);
                for r in others {
                    let copy = &mut reads.other_copy;
                    reads.other.time(start, || {
                        e.owner(r, |o| copy.clone_from(o.results())).unwrap();
                    });
                }
            }
            reads
        });
        for i in 0..PHASES {
            let on = i % 2 == 1;
            reading.store(on, Ordering::Relaxed);
            // The block that runs at the switch belongs to neither phase.
            std::thread::sleep(Duration::from_millis(100));
            let (n, t) = (FEEDS.load(Ordering::Relaxed), Instant::now());
            std::thread::sleep(PHASE);
            let rate = (FEEDS.load(Ordering::Relaxed) - n) as f64 / t.elapsed().as_secs_f64();
            rates.push(rate);
        }
        stop.store(true, Ordering::Relaxed);
        (reader.join().unwrap(), idle.join().unwrap())
    });
    reads.print();
    let mut ratios: Vec<f64> = (1..PHASES)
        .step_by(2)
        .map(|i| rates[i] / ((rates[i - 1] + rates[i + 1]) / 2.0))
        .collect();
    ratios.sort_by(f64::total_cmp);
    let ratio = ratios[ratios.len() / 2];
    let lost = number(&e, OwnerRef::Session(id), DROPPED.key);
    let slow_reads = reads.slow(5.0);
    println!(
        "PACE blocks/s by phase {rates:.1?}; ratios {ratios:.3?}; reads over 5 ms {slow_reads}, idle pauses over 5 ms \
         {slow_pauses}; lost {lost}"
    );
    assert!(reads.session.0.len() > 1000, "the reader ran");
    assert!(
        ratio >= 0.8,
        "the reads slowed the session to {ratio:.3} of its rate: {rates:.1?}"
    );
    assert_eq!(lost, 0.0, "the session lost samples");
    // A read that waited for the session would wait at every block, a hundred times a second. The reader and the idle
    // thread spin alike, so the scheduler stops them alike.
    assert!(
        slow_reads <= 3 * slow_pauses + 20,
        "{slow_reads} reads took over 5 ms, against {slow_pauses} pauses of a thread that calls nothing"
    );
}

#[test]
fn a_host_that_reads_before_the_session_kept_1024_events_gets_every_event() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, _, id) = pacer_engine();
    let mut numbers = Vec::new();
    let mut events = Vec::new();
    let mut at = number(&e, OwnerRef::Session(id), N.key) as i64;
    for _ in 0..4 {
        // The host reads after at most 30 more blocks: 900 events, fewer than the 1024 that the session keeps for the
        // host. The pacer waits in the next block meanwhile, and the control thread passes the events of the blocks
        // before it on.
        at += 30;
        STOP_AT.store(at, Ordering::Release);
        wait_for(60.0, || (HELD_AT.load(Ordering::Acquire) == at).then_some(())).expect("the pacer waits");
        assert!(e.settle(Duration::from_secs(30)));
        e.take_events(OwnerRef::Session(id), &mut events).unwrap();
        numbers.extend(
            events
                .drain(..)
                .filter(|x| x.is(&TICK))
                .filter_map(|x| x.get(&N)),
        );
    }
    STOP_AT.store(i64::MAX, Ordering::Release);
    assert!(numbers.len() > 3 * 30 * TICKS as usize, "{} events", numbers.len());
    assert!(
        numbers.windows(2).all(|w| w[1] == w[0] + 1),
        "an event is missing: {numbers:?}"
    );
}

#[test]
fn a_read_returns_while_the_session_works_on_a_block() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, radio, id) = pacer_engine();
    HOLD.store(true, Ordering::Release);
    wait_for(10.0, || HELD.load(Ordering::Acquire).then_some(())).expect("the block is held");
    let mut reads = Reads::default();
    let start = Instant::now();
    reads.frame(&e, id, start);
    for r in [OwnerRef::Engine, OwnerRef::Radio(radio)] {
        e.owner(r, |o| o.results().change_counter()).unwrap();
    }
    let took = start.elapsed();
    // The session is still in the block that it worked on before the first read.
    let held = HELD.load(Ordering::Acquire);
    HOLD.store(false, Ordering::Release);
    assert!(held, "the reads took {took:?} and waited for the block");
    assert!(took < Duration::from_secs(1), "the reads took {took:?}");
}

#[test]
fn the_rows_changed_since_a_counter_rebuild_the_table_of_each_copy() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, _, id) = pacer_engine();
    for r in [OwnerRef::Stack(id), OwnerRef::Session(id)] {
        let mut since = 0;
        let mut mine = std::collections::BTreeMap::new();
        // Reads until the copy holds every row, each read as the host makes them, for a minute at most.
        let start = Instant::now();
        while mine.len() < 16 && start.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(20));
            e.owner(r, |o| {
                let res = o.results();
                let table = res
                    .decl()
                    .table_id(ROWS.key)
                    .and_then(|t| res.table(t))
                    .unwrap();
                for (key, row) in table.rows_changed_since(since) {
                    mine.insert(key.to_owned(), row.clone());
                }
                assert!(res.change_counter() >= since, "the counter went back");
                since = res.change_counter();
                let all: std::collections::BTreeMap<_, _> = table
                    .rows_changed_since(0)
                    .map(|(k, row)| (k.to_owned(), row.clone()))
                    .collect();
                assert_eq!(mine, all);
            })
            .unwrap();
        }
        assert_eq!(mine.len(), 16);
    }
}
