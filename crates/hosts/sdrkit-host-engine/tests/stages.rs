//! The threads of a session: a test stack of three stages whose cost the test sets. The output is the same for every
//! placement, a stack that needs one and a half cores loses samples on one thread and none on three, and the engine cuts
//! and merges as the load changes. A stack that hands over to a receive chain with other stages is placed again at once.

mod common;

use common::{defaults, engine, engine_on, fake_rtl, number, wait_for, FakeRtl};
use sdrkit_core::{
    Clock, Complex32, ConfigError, Facilities, InputInfo, MemStorage, Output, Owner, Published,
    Random, Receiver, Results, ResultsDecl, SettingError, SettingId, StackDescriptor, Staged, Tuning,
    Value, Values,
};
use sdrkit_graph::stream::TagKind;
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Graph, GraphPlan, Io, Port, PortRef, RateFrom,
    Stage,
};
use sdrkit_host_engine::{Engine, Input, OwnerRef, Samples, SessionId, Setup, SAMPLES};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The tests of this file measure time: one runs at a time.
static ALONE: Mutex<()> = Mutex::new(());

/// The cost of each stage in thousandths of a core at the rate of the stack.
static COST: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
/// The discontinuity tags that the first component saw.
static GAPS_SEEN: AtomicU64 = AtomicU64::new(0);

fn set_cost(milli: [u64; 3]) {
    for (c, m) in COST.iter().zip(milli) {
        c.store(m, Ordering::Relaxed);
    }
}

/// Scales each sample by a factor that depends on every sample before it, and works for its cost.
struct Spin {
    stage: usize,
    rate: f64,
    phase: f32,
}

impl Component for Spin {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("work.spin", vec![Port::iq("in")], vec![Port::iq("out")])
            .in_place(0, 0)
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = i[0].rate;
        Ok(vec![i[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let t0 = Instant::now();
        if self.stage == 0 {
            let gaps = io.input(0).tags().iter();
            let gaps = gaps.filter(|t| matches!(t.kind, TagKind::Discontinuity { .. }));
            GAPS_SEEN.fetch_add(gaps.count() as u64, Ordering::Relaxed);
        }
        let x = io.pass_through::<Complex32>(0, 0);
        let n = x.len();
        for s in x {
            self.phase = (self.phase + 0.001 + s.re * 1e-4).fract();
            *s *= 1.0 + self.phase;
        }
        let cost = COST[self.stage].load(Ordering::Relaxed) as f64 / 1000.0;
        let need = Duration::from_secs_f64(cost * n as f64 / self.rate.max(1.0));
        while t0.elapsed() < need {
            std::hint::spin_loop();
        }
    }
}

/// Writes every hundredth sample as sound.
struct Tail {
    count: u64,
}

impl Component for Tail {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "work.tail",
            vec![Port::iq("in")],
            vec![Port::audio("out").rate_from(RateFrom::Divided {
                input: 0,
                by: 100.try_into().unwrap(),
            })],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::audio(i[0].rate / 100.0, 1)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let out = io.audio_out(0);
        for s in x {
            if self.count.is_multiple_of(100) {
                out.samples.push(s.re + s.im);
            }
            self.count += 1;
        }
    }
}

/// The test stack: three stages, `front`, `mid` and `back`, with the declared costs `declared`.
fn work(declared: [f64; 3]) -> GraphPlan {
    let mut p = GraphPlan::new();
    let spin = |p: &mut GraphPlan, id: &str, stage| {
        let c = Spin {
            stage,
            rate: 1.0,
            phase: 0.0,
        };
        p.add_component(id, Box::new(c)).unwrap()
    };
    let (a, b, c) = (spin(&mut p, "a", 0), spin(&mut p, "b", 1), spin(&mut p, "c", 2));
    let tail = p.add_component("tail", Box::new(Tail { count: 0 })).unwrap();
    p.input(PortRef::new(a, 0)).unwrap();
    p.connect(PortRef::new(a, 0), PortRef::new(b, 0)).unwrap();
    p.connect(PortRef::new(b, 0), PortRef::new(c, 0)).unwrap();
    p.connect(PortRef::new(c, 0), PortRef::new(tail, 0)).unwrap();
    p.output(PortRef::new(tail, 0)).unwrap();
    p.stage(a, b, Stage::new("mid").ahead_bytes(1 << 20));
    p.stage(b, c, Stage::new("back").ahead_bytes(1 << 20));
    let names = ["front", "mid", "back"];
    // The declared costs are in calibration units: cores of this device times what one of its cores runs.
    let core = sdrkit_host_engine::device().fast_units;
    let units = declared.map(|c| c * core);
    p.costs(names.iter().map(|n| n.to_string()).zip(units).collect());
    p
}

fn set_cores(e: &Engine, cores: i64) {
    e.owner(OwnerRef::Engine, |o| {
        let id = o.schema().id("cores_max").unwrap();
        sdrkit_core::owner::apply(o, id, &Value::I64(cores))
    })
    .unwrap()
    .unwrap();
}

fn session_number(e: &Engine, id: SessionId, key: &str) -> f64 {
    number(e, OwnerRef::Session(id), key)
}

/// One cell of a load table of the session.
fn cell(e: &Engine, id: SessionId, table: &str, row: &str, column: &str) -> Option<Value> {
    e.owner(OwnerRef::Session(id), |o| {
        let r = o.results();
        let t = r.table(r.decl().table_id(table)?)?;
        let col = t.columns().id(column)?;
        t.row(row)?.value(col).cloned()
    })
    .unwrap()
}

/// The rows of the table of connections: key, from, to, cut, lost, items per second.
fn connections(e: &Engine, id: SessionId) -> Vec<(String, String, String, bool, f64, f64)> {
    e.owner(OwnerRef::Session(id), |o| {
        let r = o.results();
        let Some(t) = r.decl().table_id("session.connections").and_then(|t| r.table(t)) else {
            return Vec::new();
        };
        let col = |k: &str| t.columns().id(k).unwrap();
        let (from, to, cut, lost, items) = (col("from"), col("to"), col("cut"), col("lost"), col("items_per_s"));
        let text = |v: Option<&Value>| match v {
            Some(Value::Text(s)) => s.to_string(),
            _ => String::new(),
        };
        let num = |v: Option<&Value>| match v {
            Some(Value::I64(n)) => *n as f64,
            Some(Value::F64(x)) => *x,
            _ => f64::NAN,
        };
        t.rows_changed_since(0)
            .map(|(k, row)| {
                (
                    k.to_owned(),
                    text(row.value(from)),
                    text(row.value(to)),
                    row.value(cut) == Some(&Value::Bool(true)),
                    num(row.value(lost)),
                    num(row.value(items)),
                )
            })
            .collect()
    })
    .unwrap()
}

fn signal(n: usize) -> Vec<Complex32> {
    let mut seed = 7u32;
    (0..n)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = f32::from((seed >> 16) as u16) / 65536.0 - 0.5;
            Complex32::new(a, 0.25 - a * a)
        })
        .collect()
}

/// Runs 10 s of signal at 1 MHz through the test stack from a host feed, as fast as the session takes it, and returns
/// the sound and the most threads that the session had.
fn offline(e: &Engine, declared: [f64; 3]) -> (Vec<f32>, f64) {
    const RATE: f64 = 1e6;
    let feed = e.open_feed(RATE, 100e6);
    let id = e.start_graph(work(declared), Input::Feed(feed)).unwrap();
    let x = signal(10 * RATE as usize);
    let mut sound = Vec::new();
    let mut buffer = vec![0.0f32; 1 << 14];
    let mut threads = 0f64;
    let mut read = |sound: &mut Vec<f32>| {
        let mut any = false;
        while let Some(a) = e.audio(id, &mut buffer).unwrap() {
            sound.extend_from_slice(&buffer[..a.frames]);
            any = true;
        }
        any
    };
    for (k, block) in x.chunks(16_384).enumerate() {
        let t = Instant::now();
        while e.push(feed, Samples::Iq(block)).is_err() {
            read(&mut sound);
            assert!(t.elapsed() < Duration::from_secs(30), "the session takes no samples");
            std::thread::yield_now();
        }
        read(&mut sound);
        if k % 16 == 0 {
            threads = threads.max(session_number(e, id, "session.threads"));
        }
    }
    // The session measures its load every quarter second.
    let placed = wait_for(5.0, || Some(session_number(e, id, "session.threads")).filter(|&t| t > 0.0));
    threads = threads.max(placed.unwrap_or(0.0));
    let want = x.len() / 100;
    let t = Instant::now();
    while sound.len() < want && t.elapsed() < Duration::from_secs(30) {
        if !read(&mut sound) {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    assert_eq!(session_number(e, id, "session.audio_lost"), 0.0);
    assert_eq!(session_number(e, id, "session.dropped"), 0.0);
    e.stop_session(id).unwrap();
    e.close_session(id).unwrap();
    e.close_feed(feed).unwrap();
    (sound, threads)
}

#[test]
fn the_output_is_the_same_for_every_placement_of_the_stages() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    set_cost([0, 0, 0]);
    let e = engine();
    set_cores(&e, 1);
    let (whole, threads) = offline(&e, [0.5, 0.5, 0.5]);
    assert_eq!(whole.len(), 100_000);
    assert_eq!(threads, 1.0, "one core: one thread");
    set_cores(&e, 8);
    // No cut, a cut before the second stage, a cut before the third, and both: by the declared costs. The engine also
    // moves the cuts while the session runs, because a session that is fed as fast as it reads keeps its thread busy.
    for (declared, least) in [
        ([0.1, 0.1, 0.1], 1.0),
        ([0.5, 0.5, 0.1], 2.0),
        ([0.1, 0.5, 0.5], 2.0),
        ([0.5, 0.5, 0.5], 3.0),
    ] {
        let (cut, threads) = offline(&e, declared);
        eprintln!("declared {declared:?}: {} samples of sound, {threads} threads at most", cut.len());
        assert!(threads >= least, "declared {declared:?}: {threads} threads");
        assert_eq!(cut.len(), whole.len(), "declared {declared:?}");
        let same = cut.iter().zip(&whole).all(|(a, b)| a.to_bits() == b.to_bits());
        assert!(same, "declared {declared:?}: the sound differs");
    }
}

/// An engine and a radio that delivers in real time for it: a session that is behind loses samples.
fn live_engine() -> (Engine, FakeRtl) {
    let rtl = fake_rtl();
    (engine_on(rtl.net()), rtl)
}

/// Starts the test stack on the radio `rtl`, which sends at 2.048 MHz, and returns the session.
fn live(e: &Engine, rtl: &FakeRtl, declared: [f64; 3]) -> SessionId {
    let kind = e.find("driver.rtl_tcp").unwrap();
    let radio = e.open_radio("rtl_tcp", &rtl.addr(), &defaults(e, kind)).unwrap();
    e.start_graph(work(declared), Input::Radio(radio)).unwrap()
}

#[test]
fn a_stack_of_one_and_a_half_cores_loses_samples_on_one_thread_and_none_on_three() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    set_cost([500, 500, 500]);
    // One core: the session starts, says that it is short of cores, and loses samples.
    let (e, rtl) = live_engine();
    set_cores(&e, 1);
    GAPS_SEEN.store(0, Ordering::Relaxed);
    let id = live(&e, &rtl, [0.5, 0.5, 0.5]);
    std::thread::sleep(Duration::from_secs(8));
    let dropped = session_number(&e, id, "session.dropped");
    let need = session_number(&e, id, "session.cores_needed");
    let share = session_number(&e, id, "session.share");
    let gaps = session_number(&e, id, "session.gaps");
    let problem = common::field(&e, OwnerRef::Session(id), "session.load_problem");
    eprintln!(
        "one thread: {dropped} samples lost, needs {need:.2} cores, share {share:.2}, {gaps} gaps, {problem:?}"
    );
    assert_eq!(session_number(&e, id, "session.threads"), 1.0);
    assert!(dropped > 1e6, "{dropped} samples lost in 8 s");
    // On one thread every loss is at the queue of the session, and the column adds up to the count of the session.
    // The thread of the session ends soon after the stop; its last copy holds the final counts.
    e.stop_session(id).unwrap();
    let settled = wait_for(5.0, || {
        let rows = connections(&e, id);
        let dropped = session_number(&e, id, "session.dropped");
        let queue = rows.iter().find(|r| r.0 == "queue")?.4;
        let lost: f64 = rows.iter().map(|r| r.4).sum();
        (queue == dropped && lost == dropped).then_some(dropped)
    });
    let rows = connections(&e, id);
    eprintln!("one thread, stopped: {settled:?} lost; connections {rows:?}");
    assert!(settled.is_some_and(|d| d > 1e6), "the queue holds the loss: {rows:?}");
    // The stages work for 1.5 cores; time that the machine takes from the thread counts on top.
    assert!(need > 1.3, "needs {need} cores");
    assert!(share > 0.9, "share {share}");
    assert_eq!(problem, Some(Value::Text("needs about 1.5 cores, 1 free".into())));
    // The count of the session is a second old at most.
    let seen = GAPS_SEEN.load(Ordering::Relaxed) as f64;
    assert!(gaps > 0.0 && seen >= gaps && seen - gaps < 60.0, "{gaps} gaps told, {seen} seen");
    drop(e);

    // Four cores: a thread per stage from the start, and no sample is lost.
    let (e, rtl) = live_engine();
    set_cores(&e, 4);
    GAPS_SEEN.store(0, Ordering::Relaxed);
    let id = live(&e, &rtl, [0.5, 0.5, 0.5]);
    std::thread::sleep(Duration::from_secs(8));
    let dropped = session_number(&e, id, "session.dropped");
    let need = session_number(&e, id, "session.cores_needed");
    let threads = session_number(&e, id, "session.threads");
    let fill = cell(&e, id, "session.stages", "mid", "queue_fill");
    let mid = cell(&e, id, "session.stages", "mid", "share");
    let b = cell(&e, id, "session.components", "b", "share");
    eprintln!(
        "three threads: {dropped} samples lost, needs {need:.2} cores, {threads} threads, \
         stage mid: share {mid:?}, queue fill {fill:?}; component b: share {b:?}"
    );
    assert_eq!(threads, 3.0);
    assert_eq!(dropped, 0.0);
    assert!((1.3..1.7).contains(&need), "needs {need} cores");
    assert!(matches!(mid, Some(Value::F64(s)) if (0.4..0.65).contains(&s)), "{mid:?}");
    assert!(matches!(b, Some(Value::F64(s)) if (0.4..0.65).contains(&s)), "{b:?}");
    assert_eq!(cell(&e, id, "session.stages", "mid", "cut"), Some(Value::Bool(true)));
    assert_eq!(cell(&e, id, "session.components", "c", "thread"), Some(Value::I64(2)));
    let problem = common::field(&e, OwnerRef::Session(id), "session.load_problem");
    assert!(!matches!(problem, Some(Value::Text(_))), "{problem:?}");
    assert_eq!(GAPS_SEEN.load(Ordering::Relaxed), 0);
    // The queue of the session takes the 2.048 MHz of the radio; the two cut connections show their cut.
    let rows = connections(&e, id);
    eprintln!("three threads: connections {rows:?}");
    let queue = rows.iter().find(|r| r.0 == "queue").expect("the queue");
    assert!((1.5e6..2.5e6).contains(&queue.5), "{queue:?}");
    let mut cut: Vec<_> = rows.iter().filter(|r| r.0 != "queue" && r.3).map(|r| (r.1.as_str(), r.2.as_str())).collect();
    cut.sort_unstable();
    assert_eq!(cut, [("a", "b"), ("b", "c")]);
    assert!(rows.iter().all(|r| r.4 == 0.0), "{rows:?}");
}

#[test]
fn a_loss_behind_a_slow_stage_counts_at_the_connection_into_that_stage() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    // The middle stage needs 1.5 cores and has a thread of its own: no cut makes it keep up.
    set_cost([100, 1500, 100]);
    let (e, rtl) = live_engine();
    set_cores(&e, 4);
    let id = live(&e, &rtl, [0.1, 1.5, 0.1]);
    std::thread::sleep(Duration::from_secs(6));
    let running = connections(&e, id);
    let cut = |rows: &[(String, String, String, bool, f64, f64)]| {
        rows.iter().any(|r| (r.1.as_str(), r.2.as_str(), r.3) == ("a", "b", true))
    };
    assert!(cut(&running), "the connection into the slow stage is cut: {running:?}");
    // The last copy, after the stage threads ended, holds the final counts.
    e.stop_session(id).unwrap();
    let settled = wait_for(5.0, || {
        let rows = connections(&e, id);
        let dropped = session_number(&e, id, "session.dropped");
        let lost: f64 = rows.iter().map(|r| r.4).sum();
        (lost == dropped && dropped > 0.0).then_some((dropped, rows))
    });
    let (dropped, rows) = settled.expect("the rows add up to the loss of the session");
    eprintln!("a slow stage: {dropped} samples lost; connections {rows:?}");
    let into_mid = rows.iter().find(|r| (r.1.as_str(), r.2.as_str()) == ("a", "b")).expect("a to b");
    // The queue before the slow stage held the session up: most of the loss counts there.
    assert!(into_mid.4 > 0.5 * dropped, "{into_mid:?} of {dropped}");
}

#[test]
fn a_stage_that_becomes_heavy_gets_a_thread_and_a_light_stack_goes_back_to_one() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    set_cost([50, 50, 50]);
    let (e, rtl) = live_engine();
    set_cores(&e, 4);
    let id = live(&e, &rtl, [0.1, 0.1, 0.1]);
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(session_number(&e, id, "session.threads"), 1.0, "a light stack has one thread");
    assert_eq!(session_number(&e, id, "session.dropped"), 0.0, "lost while light");
    // The second and the third stage become heavy: together they need more than one core.
    set_cost([50, 700, 600]);
    let t = Instant::now();
    let cut = wait_for(6.0, || (session_number(&e, id, "session.threads") >= 2.0).then_some(()));
    let took = t.elapsed().as_secs_f64();
    eprintln!("at the cut: {} lost", session_number(&e, id, "session.dropped"));
    assert!(cut.is_some(), "no cut within 6 s");
    std::thread::sleep(Duration::from_secs(3));
    let threads = session_number(&e, id, "session.threads");
    let dropped = session_number(&e, id, "session.dropped");
    eprintln!("heavy: a cut after {took:.2} s, {threads} threads, {dropped} samples lost");
    assert!(took < 4.0, "the cut took {took} s");
    assert_eq!(dropped, 0.0, "samples lost around the cut");
    // Light again: the stages go back to one thread.
    set_cost([50, 50, 50]);
    let t = Instant::now();
    let merged = wait_for(15.0, || (session_number(&e, id, "session.threads") == 1.0).then_some(()));
    eprintln!("light: one thread after {:.2} s", t.elapsed().as_secs_f64());
    assert!(merged.is_some(), "the stages did not go back to one thread within 15 s");
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(session_number(&e, id, "session.dropped"), 0.0);
    assert_eq!(session_number(&e, id, "session.audio_lost").min(0.0), 0.0);
}

/// The samples of each receive chain of the hand-over stack before it hands over.
const PHASE: usize = 1_000_000;
/// The receive chains of the hand-over stack: the names of their stages, their declared cores, and the threads that the
/// engine gives them by that cost on eight cores. The first and the third have three stages each.
const CHAINS: [(&[&str], &[f64], f64); 3] = [
    (&["front", "mid", "back"], &[0.5, 0.5, 0.5], 3.0),
    (&["demod", "decode"], &[0.1, 0.1], 1.0),
    (&["one", "two", "three"], &[0.5, 0.5, 0.1], 2.0),
];
/// The chain of each phase: a change of the count, a change of the names at an equal count, and back to the first.
const ORDER: [usize; 4] = [0, 1, 2, 0];

/// The names of the stages of a chain and their declared cores, by name.
fn by_name(c: (&[&str], &[f64], f64)) -> (Vec<String>, Vec<f64>) {
    let mut rows: Vec<(String, f64)> = c.0.iter().map(|n| n.to_string()).zip(c.1.iter().copied()).collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().unzip()
}

/// A receive chain of one spin per stage and a tail.
fn chain(names: &[&str], declared: &[f64]) -> Graph {
    let mut p = GraphPlan::new();
    let mut spins = Vec::new();
    for (stage, name) in names.iter().enumerate() {
        let c = Spin {
            stage,
            rate: 1.0,
            phase: 0.0,
        };
        spins.push(p.add_component(&format!("s{stage}"), Box::new(c)).unwrap());
        if stage > 0 {
            p.connect(PortRef::new(spins[stage - 1], 0), PortRef::new(spins[stage], 0))
                .unwrap();
            p.stage(spins[stage - 1], spins[stage], Stage::new(name).ahead_bytes(1 << 20));
        }
    }
    let tail = p.add_component("tail", Box::new(Tail { count: 0 })).unwrap();
    p.input(PortRef::new(spins[0], 0)).unwrap();
    p.connect(PortRef::new(*spins.last().unwrap(), 0), PortRef::new(tail, 0))
        .unwrap();
    p.output(PortRef::new(tail, 0)).unwrap();
    p.first_stage(names[0]);
    let core = sdrkit_host_engine::device().fast_units;
    p.costs(names.iter().map(|n| n.to_string()).zip(declared.iter().map(|c| c * core)).collect());
    p.build().unwrap()
}

/// A stack that decodes with one receive chain at a time and hands over to the next after [`PHASE`] samples, at that
/// sample whatever the blocks are. At a hand-over it ends the placement of the chain it leaves, as the stack contract
/// asks.
struct Handover {
    chains: Vec<Graph>,
    phase: usize,
    left: usize,
    values: Values,
    results: Published,
}

impl Handover {
    fn active(&mut self) -> &mut Graph {
        &mut self.chains[ORDER[self.phase]]
    }
}

impl Owner for Handover {
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

impl Receiver for Handover {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.chains.iter_mut().try_for_each(|c| Receiver::configure(c, input))
    }
    fn feed(&mut self, mut iq: &[Complex32], out: &mut Output) {
        while !iq.is_empty() {
            let n = self.left.min(iq.len());
            self.active().feed(&iq[..n], out);
            (iq, self.left) = (&iq[n..], self.left - n);
            if self.left == 0 && self.phase + 1 < ORDER.len() {
                let st = self.active();
                st.drain(out);
                drop(Staged::cut(st, 0));
                (self.phase, self.left) = (self.phase + 1, PHASE);
            }
        }
    }
    fn finish(&mut self, out: &mut Output) {
        Graph::finish(self.active(), out);
    }
    fn staged(&mut self) -> Option<&mut dyn Staged> {
        Some(self.active())
    }
}

static HANDOVER: StackDescriptor = StackDescriptor {
    id: "handover",
    name: "Hand-over",
    tuning: Tuning::anywhere(100e6, 1e6, 1e3, 10e3),
    submodes: &[],
    make_receiver: |cfg| {
        Ok(Box::new(Handover {
            chains: CHAINS.iter().map(|(n, d, _)| chain(n, d)).collect(),
            phase: 0,
            left: PHASE,
            values: cfg.settings.clone(),
            results: Published::from_decl(Arc::new(ResultsDecl::new())),
        }))
    },
    ..common::METER_DESC
};

/// What the session showed when the stages of a chain were first in its table: the names of the stages, the threads, the
/// declared cores of each stage, and the seconds from the end of the chain before.
type Placed = (Vec<String>, f64, Vec<f64>, f64);

/// Runs the hand-over stack from a host feed on `cores` cores and returns its sound and what it placed. With `pause` the
/// feed waits at each hand-over until the new stages are placed, and the list has one entry per phase; without, the
/// blocks flow through every hand-over, and the list has the placement at the end.
fn handed_over(cores: i64, pause: bool) -> (Vec<f32>, Vec<Placed>) {
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(sdrkit_core::net::FakeTransport::new()),
        stacks: vec![&HANDOVER],
        families: Vec::new(),
        components: sdrkit_graph::Registry::default(),
    })
    .unwrap();
    set_cores(&e, cores);
    let feed = e.open_feed(1e6, 100e6);
    let values = Values::defaults(&(HANDOVER.settings)());
    let id = e.start_stack("handover", None, &values, Input::Feed(feed), None).unwrap();
    let placed_now = || {
        let (names, cores) = e.owner(OwnerRef::Session(id), |o| {
            let r = o.results();
            let t = r.table(r.decl().table_id("session.stages")?)?;
            let declared = t.columns().id("declared")?;
            // The rows of a table come in no fixed order: by name.
            let mut rows: Vec<(String, f64)> = t
                .rows_changed_since(0)
                .map(|(k, row)| match row.value(declared) {
                    Some(Value::F64(x)) => (k.to_owned(), *x),
                    _ => (k.to_owned(), f64::NAN),
                })
                .collect();
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            Some(rows.into_iter().unzip())
        })
        .unwrap()?;
        // Read after the names: no load change follows a new placement within a second.
        Some((names, session_number(&e, id, "session.threads"), cores))
    };
    let x = signal(ORDER.len() * PHASE);
    let mut sound = Vec::new();
    let mut buffer = vec![0.0f32; 1 << 14];
    let mut read = |sound: &mut Vec<f32>| {
        let mut any = false;
        while let Some(a) = e.audio(id, &mut buffer).unwrap() {
            sound.extend_from_slice(&buffer[..a.frames]);
            any = true;
        }
        any
    };
    let mut placed = Vec::new();
    for (k, part) in x.chunks(PHASE).enumerate() {
        if !pause {
            for block in part.chunks(16_384) {
                let t = Instant::now();
                while e.push(feed, Samples::Iq(block)).is_err() {
                    read(&mut sound);
                    assert!(t.elapsed() < Duration::from_secs(30), "the session takes no samples");
                    std::thread::yield_now();
                }
                read(&mut sound);
            }
            continue;
        }
        // The session has taken the chain before: the stack has handed over.
        let taken = wait_for(10.0, || {
            (session_number(&e, id, SAMPLES.key) >= (k * PHASE) as f64).then_some(())
        });
        assert!(taken.is_some(), "phase {k}: the session did not take the samples before");
        let t = Instant::now();
        let want = by_name(CHAINS[ORDER[k]]).0;
        let seen = wait_for(5.0, || placed_now().filter(|p: &(Vec<String>, f64, Vec<f64>)| p.0 == want));
        let (names, threads, cores) = seen.unwrap_or_else(|| panic!("phase {k}: {:?}", placed_now()));
        placed.push((names, threads, cores, t.elapsed().as_secs_f64()));
        for block in part.chunks(16_384) {
            let t = Instant::now();
            while e.push(feed, Samples::Iq(block)).is_err() {
                read(&mut sound);
                assert!(t.elapsed() < Duration::from_secs(30), "the session takes no samples");
                std::thread::yield_now();
            }
            read(&mut sound);
        }
    }
    let want = x.len() / 100;
    let t = Instant::now();
    while sound.len() < want && t.elapsed() < Duration::from_secs(30) {
        if !read(&mut sound) {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    if !pause {
        let t = Instant::now();
        let want = by_name(CHAINS[ORDER[ORDER.len() - 1]]).0;
        let seen = wait_for(5.0, || placed_now().filter(|p: &(Vec<String>, f64, Vec<f64>)| p.0 == want));
        let (names, threads, cores) = seen.unwrap_or_else(|| panic!("at the end: {:?}", placed_now()));
        placed.push((names, threads, cores, t.elapsed().as_secs_f64()));
    }
    assert_eq!(session_number(&e, id, "session.audio_lost"), 0.0);
    assert_eq!(session_number(&e, id, "session.dropped"), 0.0);
    e.stop_session(id).unwrap();
    e.close_session(id).unwrap();
    e.close_feed(feed).unwrap();
    (sound, placed)
}

#[test]
fn a_stack_that_changes_its_stages_is_placed_again_at_once_and_its_output_does_not_change() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    set_cost([0, 0, 0]);
    let (whole, one) = handed_over(1, true);
    let (cut, eight) = handed_over(8, true);
    for (k, (p1, p8)) in one.iter().zip(&eight).enumerate() {
        eprintln!("phase {k}: one core {p1:?}; eight cores {p8:?}");
        let (names, declared) = by_name(CHAINS[ORDER[k]]);
        let threads = CHAINS[ORDER[k]].2;
        for (p, threads) in [(p1, 1.0), (p8, threads)] {
            assert_eq!(p.0, names, "phase {k}");
            // The snapshot that first shows the new names shows the placement by their declared cost.
            assert_eq!(p.1, threads, "phase {k}: {p:?}");
            let same = p.2.iter().zip(&declared).all(|(a, b)| (a - b).abs() < 1e-9);
            assert!(same && p.2.len() == declared.len(), "phase {k}: declared {:?}", p.2);
            assert!(p.3 < 1.0, "phase {k}: placed after {} s", p.3);
        }
    }
    assert_eq!(whole.len(), ORDER.len() * PHASE / 100);
    assert_eq!(cut.len(), whole.len());
    let same = cut.iter().zip(&whole).all(|(a, b)| a.to_bits() == b.to_bits());
    assert!(same, "the sound of eight cores differs from that of one");
    // Blocks that flow through each hand-over: a chain is left and taken up again within a look or two, and its stages
    // are cut while blocks wait. The sound is the same, and the last chain has its threads.
    let (flowing, end) = handed_over(8, false);
    eprintln!("flowing: {end:?}");
    assert_eq!(flowing.len(), whole.len());
    let same = flowing.iter().zip(&whole).all(|(a, b)| a.to_bits() == b.to_bits());
    assert!(same, "the sound of a feed that flows through the hand-overs differs from that of one core");
    assert_eq!(end.last().map(|p| p.1), Some(CHAINS[ORDER[ORDER.len() - 1]].2), "{end:?}");
}

#[test]
fn the_two_standard_stack_is_placed_again_at_each_hand_over() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    set_cost([0, 0, 0]);
    // A stand-in for DVB-T2 that never locks: two stages of half a core each, so two threads.
    let stand_in = (&["t2_front", "t2_back"][..], &[0.5, 0.5][..], 2.0);
    sdrkit_stack_dvbt::terrestrial::set_t2_factory(|_| {
        Ok(Box::new(chain(&["t2_front", "t2_back"], &[0.5, 0.5])))
    });
    let mut components = sdrkit_graph::Registry::default();
    sdrkit_stack_dvbt::register(&mut components);
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(sdrkit_core::net::FakeTransport::new()),
        stacks: vec![&sdrkit_stack_dvbt::DVB_TERRESTRIAL],
        families: Vec::new(),
        components,
    })
    .unwrap();
    set_cores(&e, 8);
    let desc = &sdrkit_stack_dvbt::DVB_TERRESTRIAL;
    let values = Values::defaults(&(desc.settings)());
    let rate = desc.rate_hz(&values);
    let feed = e.open_feed(rate, 600e6);
    let id = e.start_stack(desc.id, None, &values, Input::Feed(feed), None).unwrap();
    let placed = || {
        let names = e.owner(OwnerRef::Session(id), |o| {
            let r = o.results();
            let t = r.table(r.decl().table_id("session.stages")?)?;
            let mut names: Vec<String> = t.rows_changed_since(0).map(|(k, _)| k.to_owned()).collect();
            names.sort_unstable();
            Some(names)
        });
        Some((names.unwrap()?, session_number(&e, id, "session.threads")))
    };
    let t2_names = by_name(stand_in).0;
    let mut pushed = 0usize;
    let mut push = |seconds: f64| {
        let n = (seconds * rate) as usize + 16_384;
        for block in signal(n).chunks(16_384) {
            let t = Instant::now();
            while e.push(feed, Samples::Iq(block)).is_err() {
                assert!(t.elapsed() < Duration::from_secs(60), "the session takes no samples");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        pushed += n;
        let taken = wait_for(60.0, || (session_number(&e, id, SAMPLES.key) >= pushed as f64).then_some(()));
        assert!(taken.is_some(), "the session did not take the samples");
    };
    // DVB-T2 first, then DVB-T after its slice, then DVB-T2 again.
    let first = wait_for(5.0, || placed().filter(|p| p.0 == t2_names));
    assert_eq!(first.map(|p| p.1), Some(2.0), "DVB-T2 first");
    push(sdrkit_stack_dvbt::terrestrial::T2_SLICE_S);
    let t = Instant::now();
    let dvbt = wait_for(5.0, || placed().filter(|p| !p.0.is_empty() && p.0 != t2_names));
    eprintln!("DVB-T after {:.3} s: {dvbt:?}", t.elapsed().as_secs_f64());
    let dvbt = dvbt.expect("the stages of DVB-T");
    assert_eq!(dvbt.0, ["equalise", "fec", "ofdm", "ts"]);
    push(sdrkit_stack_dvbt::terrestrial::T_SLICE_S);
    let t = Instant::now();
    let back = wait_for(5.0, || placed().filter(|p| p.0 == t2_names));
    eprintln!("DVB-T2 again after {:.3} s: {back:?}", t.elapsed().as_secs_f64());
    assert_eq!(back.map(|p| p.1), Some(2.0), "DVB-T2 again");
    e.stop_session(id).unwrap();
    e.close_session(id).unwrap();
    e.close_feed(feed).unwrap();
}
