//! Stage boundaries of a graph: the output is the same for every placement of cuts.

use sdrkit_core::{Complex32, InputInfo, Output, Receiver, Staged};
use sdrkit_graph::{components::*, Graph, GraphBuilder, Stage};

fn am() -> Graph {
    GraphBuilder::new()
        .add("chan", Channel::new(10e3, 24e3).unwrap())
        .add("demod", AmDemod::default())
        .add("audio", AudioResampler::new(48e3).unwrap())
        .input("chan")
        .connect("chan", "demod")
        .connect("demod", "audio")
        .output("audio")
        .stage("chan", "demod", Stage::new("demod"))
        .stage("demod", "audio", Stage::new("sound").ahead_bytes(1 << 16))
        .build()
        .unwrap()
}

fn signal(n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let t = i as f32 / 96e3;
            let a = 1.0 + 0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            Complex32::from_polar(a, 2.0 * std::f32::consts::PI * 1000.0 * t)
        })
        .collect()
}

/// Runs the signal through the graph cut at `mask`, with one thread per cut, and returns the audio. `poke` reaches for a
/// component in the middle of the run, which takes every component home.
fn run(mask: u32, poke: bool) -> (Vec<f32>, Vec<sdrkit_core::StageLoad>) {
    let mut g = am();
    g.configure(&InputInfo::new(96e3, 0.0, 0.0).recording()).unwrap();
    let threads: Vec<_> = g
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(threads.len(), mask.count_ones() as usize);
    let x = signal(96_000);
    let mut out = Output::default();
    let mut loads = Vec::new();
    for (k, block) in x.chunks(1000).enumerate() {
        g.feed(block, &mut out);
        if poke && k == 40 {
            assert!(g.component_mut("demod").is_some());
        }
        if k == 90 {
            g.stage_loads(&mut loads);
        }
    }
    g.finish(&mut out);
    drop(g.cut(0));
    for t in threads {
        t.join().unwrap();
    }
    (out.audio().to_vec(), loads)
}

#[test]
fn the_output_is_the_same_for_every_placement_of_cuts() {
    let (whole, loads) = run(0, false);
    assert!(whole.len() > 40_000, "{}", whole.len());
    assert_eq!(loads.len(), 3);
    assert_eq!(loads[1].name, "demod");
    assert!(loads.iter().all(|l| !l.cut && l.busy_ns > 0));
    for mask in 1..4 {
        let (cut, loads) = run(mask, false);
        assert!(cut == whole, "cut at {mask:#b}");
        for (s, l) in loads.iter().enumerate().skip(1) {
            assert_eq!(l.cut, mask & (1 << (s - 1)) != 0, "stage {s} at {mask:#b}");
        }
        let (poked, _) = run(mask, true);
        assert!(poked == whole, "cut at {mask:#b}, with every component taken home in the run");
    }
}

/// The connections of the graph cut at `mask` after a run of the signal, read before and after the threads end.
fn connections(mask: u32) -> (Vec<sdrkit_core::ConnectionLoad>, Vec<sdrkit_core::ConnectionLoad>) {
    let mut g = am();
    g.configure(&InputInfo::new(96e3, 0.0, 0.0).recording()).unwrap();
    let threads: Vec<_> = g
        .cut(mask)
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    let mut out = Output::default();
    for block in signal(96_000).chunks(1000) {
        g.feed(block, &mut out);
    }
    // A later queue gets its first unit from the thread before it: wait for a push at every cut.
    let mut during = Vec::new();
    let t0 = std::time::Instant::now();
    loop {
        g.connection_loads(&mut during);
        let pushed = during.iter().all(|c| !c.cut || c.held_samples > 0);
        if pushed || t0.elapsed() > std::time::Duration::from_secs(5) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    g.finish(&mut out);
    drop(g.cut(0));
    for t in threads {
        t.join().unwrap();
    }
    let mut after = Vec::new();
    g.connection_loads(&mut after);
    (during, after)
}

#[test]
fn a_connection_counts_the_same_items_under_every_cut_and_shows_where_it_is_cut() {
    let (_, whole) = connections(0);
    assert!(whole.len() >= 2, "{whole:?}");
    assert_eq!(whole.iter().filter(|c| c.boundary).count(), 2, "{whole:?}");
    assert!(whole.iter().all(|c| c.items > 0 && c.bytes >= c.items && !c.cut));
    assert!(whole.iter().all(|c| c.wait_samples > 0 && c.held_max >= c.held_bytes));
    let first = &whole[0];
    assert_eq!((first.from.as_str(), first.to.as_str()), ("chan", "demod"));
    assert!(first.format.starts_with("iq "), "{}", first.format);
    for mask in 1..4 {
        let (during, after) = connections(mask);
        let counts = |v: &[sdrkit_core::ConnectionLoad]| {
            v.iter().map(|c| (c.from.clone(), c.to.clone(), c.items, c.bytes)).collect::<Vec<_>>()
        };
        assert_eq!(counts(&after), counts(&whole), "cut at {mask:#b}");
        for c in &during {
            let cut = c.boundary && mask & (1 << (c.stage - 1)) != 0;
            assert_eq!(c.cut, cut, "{} to {} at {mask:#b}", c.from, c.to);
            if cut {
                assert!(c.capacity_bytes > 0 && c.held_samples > 0, "{c:?}");
            }
        }
    }
}

/// Notes where its input carries a discontinuity tag, as (index of the sample, missing samples).
struct GapNotes(std::sync::Arc<std::sync::Mutex<Vec<(u64, u64)>>>);

impl sdrkit_graph::Component for GapNotes {
    fn info(&self) -> sdrkit_graph::ComponentInfo {
        sdrkit_graph::ComponentInfo::sink("test.gap_notes", vec![sdrkit_graph::Port::iq("in")])
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn configure(
        &mut self,
        _: &[sdrkit_graph::Format],
    ) -> Result<Vec<sdrkit_graph::Format>, sdrkit_core::ConfigError> {
        Ok(Vec::new())
    }
    fn process(&mut self, io: &mut sdrkit_graph::Io) {
        for t in io.input(0).tags() {
            if let sdrkit_graph::stream::TagKind::Discontinuity { lost } = t.kind {
                self.0.lock().unwrap().push((t.at, lost));
            }
        }
    }
}

#[test]
fn a_discontinuity_reaches_the_first_component_at_its_sample() {
    let notes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut g = GraphBuilder::new()
        .add("first", GapNotes(notes.clone()))
        .input("first")
        .build()
        .unwrap();
    g.configure(&InputInfo::new(48e3, 0.0, 0.0).recording()).unwrap();
    let mut out = Output::default();
    let x = signal(300);
    g.feed(&x[..100], &mut out);
    // 4096 samples are missing after the first 100, and 7 after 50 more that the caller still holds.
    g.discontinuity(4096);
    g.discontinuity_after(50, 7);
    g.feed(&x[100..300], &mut out);
    assert_eq!(*notes.lock().unwrap(), [(100, 4096), (150, 7)]);
}

/// Scales its samples by the count of its configures, and requests a new configure in its 40th pass.
struct Rescale {
    configures: f32,
    passes: u32,
}

impl sdrkit_graph::Component for Rescale {
    fn info(&self) -> sdrkit_graph::ComponentInfo {
        use sdrkit_graph::Port;
        sdrkit_graph::ComponentInfo::processor("test.rescale", vec![Port::iq("in")], vec![Port::iq("out")])
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn configure(
        &mut self,
        i: &[sdrkit_graph::Format],
    ) -> Result<Vec<sdrkit_graph::Format>, sdrkit_core::ConfigError> {
        self.configures += 1.0;
        CONFIGURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(vec![i[0].clone()])
    }
    fn process(&mut self, io: &mut sdrkit_graph::Io) {
        self.passes += 1;
        if self.passes == 40 {
            io.request_reformat();
        }
        let x = io.iq(0);
        let k = self.configures;
        io.iq_out(0).extend(x.iter().map(|s| s * k));
    }
}

#[test]
fn a_new_configure_that_a_component_requests_takes_effect_at_the_same_pass_under_every_cut() {
    let run = |mask: u32| {
        CONFIGURES.store(0, std::sync::atomic::Ordering::Relaxed);
        let mut g = GraphBuilder::new()
            .add("chan", Channel::new(10e3, 24e3).unwrap())
            .add("rescale", Rescale { configures: 0.0, passes: 0 })
            .add("demod", AmDemod::default())
            .add("audio", AudioResampler::new(48e3).unwrap())
            .input("chan")
            .connect("chan", "rescale")
            .connect("rescale", "demod")
            .connect("demod", "audio")
            .output("audio")
            .stage("chan", "rescale", Stage::new("scale").ahead_bytes(1 << 16))
            .stage("demod", "audio", Stage::new("sound").ahead_bytes(1 << 16))
            .build()
            .unwrap();
        g.configure(&InputInfo::new(96e3, 0.0, 0.0).recording()).unwrap();
        let threads: Vec<_> = g
            .cut(mask)
            .into_iter()
            .map(|w| std::thread::spawn(move || w.run()))
            .collect();
        let x = signal(96_000);
        let mut out = Output::default();
        for block in x.chunks(1000) {
            g.feed(block, &mut out);
        }
        g.finish(&mut out);
        drop(g.cut(0));
        for t in threads {
            t.join().unwrap();
        }
        (out.audio().to_vec(), CONFIGURES.load(std::sync::atomic::Ordering::Relaxed))
    };
    let whole = run(0);
    // One configure at the start and the one that the component requested.
    assert_eq!(whole.1, 2);
    for mask in 1..4 {
        assert!(run(mask) == whole, "cut at {mask:#b}");
    }
}

/// The configures of [`Rescale`] in the run that is under way.
static CONFIGURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
