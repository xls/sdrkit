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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

    fn max(&self) -> f64 {
        self.kinds().iter().map(|k| k.1.max()).fold(0.0, f64::max)
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
};
/// The blocks that the pacers were fed, and the time they worked on them in nanoseconds.
static FEEDS: AtomicU64 = AtomicU64::new(0);
static WORK_NS: AtomicU64 = AtomicU64::new(0);
/// While set, a pacer stays in the block it works on. `HELD` says that one does.
static HOLD: AtomicBool = AtomicBool::new(false);
static HELD: AtomicBool = AtomicBool::new(false);
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
        WORK_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        // A test holds the block, for 5 s at most.
        while HOLD.load(Ordering::Acquire) && t.elapsed() < Duration::from_secs(5) {
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

/// What the pacer session of `e` did in `time`: the blocks a second, the mean time of a block in milliseconds, and the
/// share of its samples that it lost.
fn pace(e: &Engine, id: SessionId, time: Duration) -> (f64, f64, f64) {
    let s = OwnerRef::Session(id);
    let at = || {
        (
            FEEDS.load(Ordering::Relaxed) as f64,
            WORK_NS.load(Ordering::Relaxed) as f64,
            number(e, s, DROPPED.key),
            number(e, s, SAMPLES.key),
        )
    };
    let a = at();
    std::thread::sleep(time);
    let b = at();
    let blocks = (b.0 - a.0).max(1.0);
    (
        blocks / time.as_secs_f64(),
        (b.1 - a.1) / blocks / 1e6,
        (b.2 - a.2) / (b.3 - a.3 + b.2 - a.2).max(1.0),
    )
}

#[test]
fn a_reader_in_a_tight_loop_does_not_slow_the_session() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, radio, id) = pacer_engine();
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
    let quiet = pace(&e, id, Duration::from_secs(4));
    let stop = AtomicBool::new(false);
    let (busy, reads, pause) = std::thread::scope(|scope| {
        // The longest time that the scheduler keeps a thread that calls nothing off its processor, in milliseconds.
        let idle = scope.spawn(|| {
            let (mut last, mut pause) = (Instant::now(), 0.0f64);
            while !stop.load(Ordering::Relaxed) {
                let now = Instant::now();
                pause = pause.max((now - last).as_secs_f64() * 1e3);
                last = now;
            }
            pause
        });
        let reader = scope.spawn(|| {
            let mut reads = Reads::default();
            let start = Instant::now();
            while !stop.load(Ordering::Relaxed) {
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
        let busy = pace(&e, id, Duration::from_secs(10));
        stop.store(true, Ordering::Relaxed);
        (busy, reader.join().unwrap(), idle.join().unwrap())
    });
    reads.print();
    println!("PACE quiet: {quiet:?} with the reader: {busy:?} (blocks/s, ms per block, share lost); pause {pause} ms");
    assert!(reads.session.0.len() > 1000, "the reader ran");
    // A read takes under 5 ms, beside what the scheduler takes from the reader and from a thread that holds a lock.
    let limit = 5.0 + 2.0 * pause;
    assert!(
        reads.max() < limit,
        "the slowest read took {} ms, the limit is {limit} ms",
        reads.max()
    );
    assert!(busy.0 >= quiet.0 * 0.95, "{busy:?} against {quiet:?}");
    assert!(busy.1 <= quiet.1 * 1.25 + 0.1, "{busy:?} against {quiet:?}");
    assert!(busy.2 <= quiet.2 + 0.001, "{busy:?} against {quiet:?}");
}

#[test]
fn a_host_that_reads_once_a_second_gets_every_event() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, _, id) = pacer_engine();
    let mut numbers = Vec::new();
    let mut events = Vec::new();
    for _ in 0..4 {
        std::thread::sleep(Duration::from_secs(1));
        e.take_events(OwnerRef::Session(id), &mut events).unwrap();
        numbers.extend(
            events
                .drain(..)
                .filter(|x| x.is(&TICK))
                .filter_map(|x| x.get(&N)),
        );
    }
    // About 450 events a second: fewer than the 1024 that the session keeps for the host.
    assert!(numbers.len() > 1200, "{} events", numbers.len());
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
fn the_session_works_on_its_blocks_while_a_read_is_open() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, _, id) = pacer_engine();
    for r in [OwnerRef::Stack(id), OwnerRef::Session(id)] {
        let fed = e
            .owner(r, |_| {
                let before = FEEDS.load(Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(500));
                FEEDS.load(Ordering::Relaxed) - before
            })
            .unwrap();
        // About 15 blocks a second.
        assert!(fed >= 3, "{fed} blocks while a read of {r:?} was open");
    }
}

#[test]
fn the_rows_changed_since_a_counter_rebuild_the_table_of_each_copy() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, _, id) = pacer_engine();
    for r in [OwnerRef::Stack(id), OwnerRef::Session(id)] {
        let mut since = 0;
        let mut mine = std::collections::BTreeMap::new();
        for _ in 0..100 {
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
