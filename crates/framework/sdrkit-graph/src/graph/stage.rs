//! Stages of a graph. A stack marks edges of its graph as stage boundaries. The host can cut the graph at a boundary:
//! the components after the cut then run on a thread of the host, and whole passes cross the cut through a bounded
//! queue. The graph starts no thread.
//!
//! One unit of a queue is one pass: every buffer that crosses the cut, with its tags and the clock tick. The components
//! after the cut see the same calls as on one thread, so the output is the same for every placement of cuts.
//!
//! The queue is a type of its own. The ring of a radio carries raw samples of one type and drops when it is full, and a
//! tap drops when its reader is behind. A queue between stages carries typed buffers with tags, and its writer waits.

use super::{collect_outputs, Graph, GraphError, Node};
use crate::codec::MediaAudio;
use crate::component::{Component, ComponentInfo, Io, Latency, TagPolicy};
use crate::media::MediaFormat;
use crate::meter::{ns_of, Counters, StreamCount, SAMPLE_EVERY};
use crate::stream::{Buffer, Format, TagKind};
use crate::tap::TapWriter;
use sdrkit_core::codec::CodecSet;
use sdrkit_core::{
    Commands, Complex32, ComponentLoad, ConfigError, ConnectionLoad, Output, Results, StageDecl,
    StageLoad, StageRun, Staged, Values,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The bytes that wait before a stage that states no bound.
pub const AHEAD_BYTES: usize = 8 << 20;
/// The most stages of one graph.
pub const MAX_STAGES: usize = 32;

/// A stage of a graph, as the stack declares it at a boundary ([`GraphBuilder::stage`](crate::GraphBuilder::stage)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stage {
    pub(crate) name: String,
    pub(crate) ahead_bytes: usize,
}

impl Stage {
    /// A stage `name` with the default read-ahead bound, [`AHEAD_BYTES`].
    pub fn new(name: &str) -> Stage {
        Stage {
            name: name.to_owned(),
            ahead_bytes: AHEAD_BYTES,
        }
    }
    /// Sets the read-ahead bound of the stage: the most bytes of units that wait before it. The stage may hold that much
    /// in advance when it has a thread of its own. One unit always fits.
    pub fn ahead_bytes(mut self, bytes: usize) -> Stage {
        self.ahead_bytes = bytes;
        self
    }
}

/// A marked edge: the components by index, and the stage that begins at the input of `to`.
#[derive(Clone, Debug)]
pub(crate) struct StageMark {
    pub from: usize,
    pub to: usize,
    pub stage: Stage,
}

/// Why a stage boundary cannot be where the stack marked it.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StageProblem {
    /// A fed-back edge joins two stages. Both ends of a feedback edge belong to one stage.
    #[error("the feedback edge from \"{from}\" to \"{to}\" crosses the boundary")]
    Feedback {
        /// The component that writes the edge.
        from: String,
        /// The component that reads it.
        to: String,
    },
    /// A component after the boundary reads the input of the graph.
    #[error("\"{component}\" reads the input of the graph")]
    Input {
        /// The component.
        component: String,
    },
    /// Two boundaries at the same depth name their stage differently.
    #[error("the same stage is also named \"{other}\"")]
    Name {
        /// The other name.
        other: String,
    },
    /// More than [`MAX_STAGES`] stages.
    #[error("the graph has too many stages")]
    TooMany,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// What one pass needs besides the component: the streams and the limits of the graph.
pub(crate) struct Pass<'a> {
    pub streams: &'a mut [Buffer],
    pub formats: &'a [Format],
    pub consumers: &'a [usize],
    pub outputs: &'a [usize],
    pub taps: &'a [(usize, TapWriter)],
    pub max_items: usize,
    pub dropped_items: &'a mut u64,
    pub reformats: &'a mut Vec<usize>,
    pub tick: bool,
    pub finish: bool,
    pub iq: &'a [Complex32],
    pub pass_s: f64,
    pub metering: bool,
    /// The counters of each stream, and the number of the pass.
    pub flow: &'a [Arc<StreamCount>],
    pub seq: u64,
}

/// Runs component `i` for one pass: its `process`, or its `finish` at the end of the stream.
pub(crate) fn run_node(i: usize, n: &mut Node, p: &mut Pass<'_>) {
    let streams = &mut *p.streams;
    for &s in &n.outputs {
        let mut b = std::mem::take(&mut streams[s]);
        b.advance();
        n.scratch.push(b);
    }
    let mut items_in = 0;
    let t0 = p.metering.then(Instant::now);
    if let Some(t0) = t0 {
        // The waits of the inputs are measured in one pass of [`SAMPLE_EVERY`].
        let sampled = p.seq.is_multiple_of(SAMPLE_EVERY);
        let t0_ns = if sampled { ns_of(t0) } else { 0 };
        for (k, &s) in n.inputs.iter().enumerate() {
            let len = if s == 0 { p.iq.len() } else { streams[s].len() };
            items_in += len;
            let Some(c) = n.in_count.get(k).filter(|_| s != 0) else {
                continue;
            };
            if len == 0 {
                c.empty();
            }
            if let Some(end) = sampled.then(|| p.flow.get(s)?.end_of(p.seq)).flatten() {
                c.waited(t0_ns.saturating_sub(end));
            }
        }
    }
    // In place: the input's items move into the output when nothing else reads them.
    let mut moved = None;
    if let Some((pi, po)) = n.info.in_place {
        let si = n.inputs[pi];
        if p.consumers[si] == 1
            && si != 0
            && !n.feedback[pi]
            && !p.outputs.contains(&si)
            && !p.taps.iter().any(|(t, _)| *t == si)
            && p.formats[si].kind == n.kinds[po]
        {
            let src = &mut streams[si];
            std::mem::swap(&mut src.data, &mut n.scratch[po].data);
            src.moved = n.scratch[po].data.len();
            moved = Some((pi, po));
        }
    }
    {
        let mut io = Io {
            bufs: streams,
            formats: p.formats,
            inputs: &n.inputs,
            outputs: &n.outputs,
            outs: &mut n.scratch,
            moved,
            tick: p.tick,
            input0: p.iq,
            unconnected: n.unconnected,
            pass_s: p.pass_s,
            reformat: false,
            schema: &n.info.settings,
            max_items: p.max_items,
            meter: p.metering.then_some(&n.meter),
        };
        if p.finish {
            n.comp.finish(&mut io);
        } else {
            n.comp.process(&mut io);
        }
        let reformat = io.reformat;
        if let Some(t0) = t0 {
            let t1 = Instant::now();
            let ns = t1.saturating_duration_since(t0).as_nanos() as u64;
            let mut items_out = 0;
            let stamp = p.seq.is_multiple_of(SAMPLE_EVERY).then(|| ns_of(t1));
            for (b, &s) in n.scratch.iter().zip(&n.outputs) {
                items_out += b.len();
                if let Some(c) = p.flow.get(s) {
                    c.wrote(b.len() as u64, b.data.len_bytes() as u64);
                    if let Some(at) = stamp {
                        c.stamp(p.seq, at);
                    }
                }
            }
            n.meter.call(ns, items_in as u64, items_out as u64);
        }
        if reformat {
            p.reformats.push(i);
        }
    }
    for (o, b) in n.scratch.iter_mut().enumerate() {
        if !n.kinds[o].is_sampled() && b.len() > p.max_items {
            let lost = b.len() - p.max_items;
            if let Some(c) = p.flow.get(n.outputs[o]) {
                c.full.fetch_add(1, Ordering::Relaxed);
            }
            b.data.truncate(p.max_items);
            *p.dropped_items += lost as u64;
            let at = b.end().saturating_sub(1);
            b.push_tag(at, TagKind::Discontinuity { lost: lost as u64 });
        }
    }
    if n.info.tags == TagPolicy::Auto && !n.carries.is_empty() {
        // Tags follow the streams forward: not from a fed-back input, and not from a stream without a
        // rate onto one with a rate (there is no position to map them to).
        let no = n.outputs.len();
        for (k, &si) in n.inputs.iter().enumerate() {
            if n.feedback[k] {
                continue;
            }
            let timed = p.formats[si].kind.is_sampled();
            for o in 0..no {
                if timed || !n.kinds[o].is_sampled() {
                    n.carries[k * no + o].forward(&streams[si], &mut n.scratch[o]);
                }
            }
        }
    }
    for &s in n.outputs.iter().rev() {
        streams[s] = n.scratch.pop().expect("one buffer per output");
    }
}

/// One pass on its way over a cut.
struct Unit {
    /// The number of the pass.
    seq: u64,
    bufs: Vec<(usize, Buffer)>,
    tick: bool,
    finish: bool,
    pass_s: f64,
    bytes: usize,
}

#[derive(Default)]
struct Queue {
    units: VecDeque<Unit>,
    bytes: usize,
    /// Buffers that the reader has used, for the writer to fill again.
    spare: Vec<(usize, Buffer)>,
    /// The reader stops after its pass and takes no unit.
    stop: bool,
}

#[derive(Default)]
struct Slot {
    crew: Option<Box<Crew>>,
    /// The thread has put the crew back.
    returned: bool,
    /// The placement has ended: the thread returns.
    closed: bool,
    /// The thread ended in a panic and the crew is gone.
    lost: bool,
}

/// The queue before one stage thread and the place where the thread takes and returns its components.
struct Link {
    q: Mutex<Queue>,
    not_empty: Condvar,
    not_full: Condvar,
    slot: Mutex<Slot>,
    slot_cv: Condvar,
    cap_bytes: usize,
    /// The first stage of the thread.
    stage: usize,
    units_max: AtomicU64,
    behind: AtomicU64,
    /// The times the reader found the queue empty, and the bytes held after each push: their sum, count and most.
    empty: AtomicU64,
    held_sum: AtomicU64,
    held_n: AtomicU64,
    held_max: AtomicU64,
    /// Seconds of signal that the thread has worked on, as the bits of an `f64`.
    signal_s: AtomicU64,
}

impl Link {
    /// Adds `unit`. Waits while the queue holds its bound of bytes, unless the reader is stopping or `give_up` is set.
    fn push(&self, unit: Unit, give_up: &AtomicBool) {
        let mut q = lock(&self.q);
        let mut waited = false;
        while q.bytes >= self.cap_bytes
            && !q.units.is_empty()
            && !q.stop
            && !give_up.load(Ordering::Acquire)
        {
            if !waited {
                waited = true;
                self.behind.fetch_add(1, Ordering::Relaxed);
            }
            q = self
                .not_full
                .wait_timeout(q, Duration::from_millis(5))
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        q.bytes += unit.bytes;
        q.units.push_back(unit);
        self.units_max
            .fetch_max(q.units.len() as u64, Ordering::Relaxed);
        let held = q.bytes as u64;
        self.held_sum.fetch_add(held, Ordering::Relaxed);
        self.held_n.fetch_add(1, Ordering::Relaxed);
        self.held_max.fetch_max(held, Ordering::Relaxed);
        drop(q);
        self.not_empty.notify_one();
    }

    /// The next unit. `None` when the reader shall stop.
    fn pop(&self) -> Option<Unit> {
        let mut q = lock(&self.q);
        if q.units.is_empty() && !q.stop {
            self.empty.fetch_add(1, Ordering::Relaxed);
        }
        loop {
            if q.stop {
                return None;
            }
            if let Some(u) = q.units.pop_front() {
                q.bytes -= u.bytes;
                drop(q);
                self.not_full.notify_one();
                return Some(u);
            }
            q = self.not_empty.wait(q).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// Waits until the reader shall stop.
    fn wait_stop(&self) {
        let mut q = lock(&self.q);
        while !q.stop {
            q = self.not_empty.wait(q).unwrap_or_else(|p| p.into_inner());
        }
    }

    fn set_stop(&self, stop: bool) {
        lock(&self.q).stop = stop;
        self.not_empty.notify_all();
        self.not_full.notify_all();
    }

    /// An empty buffer for stream `s` that continues after `sent`.
    fn fresh(&self, s: usize, sent: &Buffer, format: &Format) -> Buffer {
        let spare = {
            let mut q = lock(&self.q);
            q.spare
                .iter()
                .position(|(k, _)| *k == s)
                .map(|k| q.spare.swap_remove(k).1)
        };
        let mut b = spare.unwrap_or_else(|| Buffer::new(&format.kind));
        b.data.clear();
        b.tags.clear();
        b.moved = 0;
        b.first = sent.end();
        b
    }

    fn recycle(&self, bufs: &mut Vec<(usize, Buffer)>) {
        let mut q = lock(&self.q);
        for (s, b) in bufs.drain(..) {
            if q.spare.len() < 64 {
                q.spare.push((s, b));
            }
        }
    }
}

/// The results of the components that are away, as their threads last copied them.
#[derive(Default)]
struct Snapshots {
    results: Vec<Option<Results>>,
    dirty: Vec<usize>,
}

/// What the threads of one placement share with the graph.
#[derive(Default)]
struct Shared {
    /// The output of the passes that the last stage has finished, in order.
    mailbox: Mutex<Output>,
    snapshots: Mutex<Snapshots>,
    /// A thread needs the graph to take every component back: a component requested a new configure, or a thread failed.
    attention: AtomicBool,
    /// The pass and the component of each request of a new configure.
    reformats: Mutex<Vec<(u64, usize)>>,
}

/// What the last stage thread needs to move the outputs of the graph into an [`Output`].
struct Outlet {
    media: Vec<Option<Arc<MediaFormat>>>,
    consumed: Vec<bool>,
    media_audio: MediaAudio,
    codecs: Arc<CodecSet>,
    out: Output,
}

/// The components of one stage thread with their streams.
struct Crew {
    nodes: Vec<(usize, Node)>,
    streams: Vec<Buffer>,
    /// The streams that the components of the crew write.
    owned: Vec<usize>,
    /// The streams that go on to the next thread.
    forward: Vec<usize>,
    formats: Vec<Format>,
    consumers: Vec<usize>,
    active: Vec<bool>,
    outputs: Vec<usize>,
    max_items: usize,
    dropped_items: u64,
    metering: bool,
    flow: Vec<Arc<StreamCount>>,
    outlet: Option<Outlet>,
    /// The change counter of each component's results at the last copy.
    seen: Vec<u64>,
    since_copy: f64,
    reformats: Vec<usize>,
    used: Vec<(usize, Buffer)>,
    /// The streams of the pass that runs.
    arrived: Vec<usize>,
}

/// The body of one stage thread ([`StageRun`]).
pub(crate) struct StageWorker {
    link: Arc<Link>,
    next: Option<Arc<Link>>,
    shared: Arc<Shared>,
}

/// Marks the crew as lost when the thread ends in a panic.
struct LostGuard<'a>(&'a Link, &'a Shared);

impl Drop for LostGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            lock(&self.0.slot).lost = true;
            self.0.slot_cv.notify_all();
            self.1.attention.store(true, Ordering::Release);
        }
    }
}

impl StageRun for StageWorker {
    fn run(self: Box<Self>) {
        let _guard = LostGuard(&self.link, &self.shared);
        loop {
            let mut crew = {
                let mut slot = lock(&self.link.slot);
                loop {
                    if slot.closed {
                        return;
                    }
                    if !slot.returned {
                        if let Some(c) = slot.crew.take() {
                            break c;
                        }
                    }
                    slot = self
                        .link
                        .slot_cv
                        .wait(slot)
                        .unwrap_or_else(|p| p.into_inner());
                }
            };
            while let Some(unit) = self.link.pop() {
                if self.pass(&mut crew, unit) {
                    // A component requested a new configure: the graph does it with every component at home.
                    self.shared.attention.store(true, Ordering::Release);
                    self.link.wait_stop();
                    break;
                }
            }
            let mut slot = lock(&self.link.slot);
            slot.crew = Some(crew);
            slot.returned = true;
            drop(slot);
            self.link.slot_cv.notify_all();
        }
    }

    fn stage(&self) -> usize {
        self.link.stage
    }
}

impl StageWorker {
    /// Runs one pass of the crew. True if a component requested a new configure.
    fn pass(&self, crew: &mut Crew, mut unit: Unit) -> bool {
        let (seq, tick, finish, pass_s) = (unit.seq, unit.tick, unit.finish, unit.pass_s);
        crew.arrived.clear();
        for (s, b) in unit.bufs.drain(..) {
            crew.streams[s] = b;
            crew.arrived.push(s);
        }
        {
            let mut p = Pass {
                streams: &mut crew.streams,
                formats: &crew.formats,
                consumers: &crew.consumers,
                outputs: &crew.outputs,
                taps: &[],
                max_items: crew.max_items,
                dropped_items: &mut crew.dropped_items,
                reformats: &mut crew.reformats,
                tick,
                finish,
                iq: &[],
                pass_s,
                metering: crew.metering,
                flow: &crew.flow,
                seq,
            };
            for (i, n) in &mut crew.nodes {
                if crew.active[*i] {
                    run_node(*i, n, &mut p);
                }
            }
        }
        if let Some(next) = &self.next {
            let owned = &crew.owned;
            forward(
                &mut crew.streams,
                &crew.formats,
                &crew.forward,
                |s| owned.contains(&s),
                next,
                (seq, tick, finish, pass_s),
                &self.shared.attention,
            );
        } else if let Some(o) = crew.outlet.as_mut() {
            collect_outputs(
                &crew.outputs,
                &crew.formats,
                &crew.streams,
                &o.media,
                |s| o.consumed[s],
                &mut o.media_audio,
                &o.codecs,
                &mut o.out,
            );
            lock(&self.shared.mailbox).append(&mut o.out);
        }
        // The buffers that this pass used up go back to their writer.
        for &s in &crew.arrived {
            if !crew.forward.contains(&s) {
                let mut b = std::mem::take(&mut crew.streams[s]);
                b.advance();
                crew.used.push((s, b));
            }
        }
        self.link.recycle(&mut crew.used);
        let done = f64::from_bits(self.link.signal_s.load(Ordering::Relaxed)) + pass_s;
        self.link.signal_s.store(done.to_bits(), Ordering::Relaxed);
        crew.since_copy += pass_s;
        if crew.since_copy >= Graph::MERGE_INTERVAL_S || tick || pass_s == 0.0 {
            crew.since_copy = 0.0;
            self.copy_results(crew);
        }
        if crew.reformats.is_empty() {
            return false;
        }
        lock(&self.shared.reformats).extend(crew.reformats.drain(..).map(|i| (seq, i)));
        true
    }

    /// Copies the results that changed since the last copy for the graph to merge.
    fn copy_results(&self, crew: &mut Crew) {
        let mut snaps = None;
        for (k, (i, n)) in crew.nodes.iter().enumerate() {
            let Some(r) = n.comp.results() else { continue };
            let c = r.change_counter();
            if crew.seen[k] == c {
                continue;
            }
            crew.seen[k] = c;
            let s = snaps.get_or_insert_with(|| lock(&self.shared.snapshots));
            match &mut s.results[*i] {
                Some(copy) if Arc::ptr_eq(copy.decl(), r.decl()) => copy.copy_from(r),
                slot => {
                    let mut copy = Results::new(Arc::clone(r.decl()));
                    copy.copy_from(r);
                    *slot = Some(copy);
                }
            }
            if !s.dirty.contains(i) {
                s.dirty.push(*i);
            }
        }
    }
}

/// Sends the streams `forward` of one finished pass to `next`. A stream that `owned` names gets an empty buffer that
/// continues after the one sent.
fn forward(
    streams: &mut [Buffer],
    formats: &[Format],
    forward: &[usize],
    owned: impl Fn(usize) -> bool,
    next: &Link,
    (seq, tick, finish, pass_s): (u64, bool, bool, f64),
    give_up: &AtomicBool,
) {
    let mut bufs = Vec::with_capacity(forward.len());
    let mut bytes = 1;
    for &s in forward {
        let b = if owned(s) {
            let fresh = next.fresh(s, &streams[s], &formats[s]);
            std::mem::replace(&mut streams[s], fresh)
        } else {
            std::mem::take(&mut streams[s])
        };
        bytes += b.bytes();
        bufs.push((s, b));
    }
    next.push(
        Unit {
            seq,
            bufs,
            tick,
            finish,
            pass_s,
            bytes,
        },
        give_up,
    );
}

/// A component that is away on a stage thread, as the graph sees it: its declarations and the last copy of its results.
struct Parked {
    info: ComponentInfo,
    values: Values,
    results: Option<Results>,
    commands: Commands,
    latency: Latency,
}

impl Component for Parked {
    fn info(&self) -> ComponentInfo {
        self.info.clone()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(Vec::new())
    }
    fn process(&mut self, _: &mut Io) {}
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn latency(&self) -> Latency {
        self.latency
    }
    fn results(&self) -> Option<&Results> {
        self.results.as_ref()
    }
    fn commands(&self) -> &Commands {
        &self.commands
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// One placement of cuts: the queues, and what the threads share with the graph.
pub(crate) struct Deployment {
    mask: u32,
    /// The thread of each stage: 0 is the thread that feeds the graph.
    group_of_stage: Vec<usize>,
    /// The thread of each component, while the components are away.
    group_of_node: Vec<usize>,
    /// The queue before thread `g` is `links[g - 1]`.
    links: Vec<Arc<Link>>,
    shared: Arc<Shared>,
    /// The components of the stage threads are with their threads.
    away: bool,
    seq: u64,
    /// The streams that cross into the first stage thread.
    forward: Vec<usize>,
    /// The number of validations of the graph when the components left.
    epoch: u64,
}

impl Drop for Deployment {
    fn drop(&mut self) {
        for l in &self.links {
            lock(&l.slot).closed = true;
            l.slot_cv.notify_all();
            l.set_stop(true);
        }
    }
}

impl Deployment {
    pub(crate) fn away(&self) -> bool {
        self.away
    }
    /// True if component `i` runs on the thread that feeds the graph.
    pub(crate) fn here(&self, i: usize) -> bool {
        self.group_of_node[i] == 0
    }
    pub(crate) fn attention(&self) -> bool {
        self.shared.attention.load(Ordering::Acquire)
    }
}

impl Graph {
    /// Works out the stage of every component from the marked edges. `stage_of[i]` is the number of marked edges on the
    /// longest way from the input to component `i`.
    pub(crate) fn assign_stages(&mut self) -> Result<(), GraphError> {
        let mut producer = vec![usize::MAX; self.streams.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            for &s in &n.outputs {
                producer[s] = i;
            }
        }
        // The component of the plan that an input reads, through the adapters that the graph inserted.
        let source = |mut s: usize| loop {
            let p = *producer.get(s)?;
            if p == usize::MAX {
                return None;
            }
            if p < self.user_nodes {
                return Some(p);
            }
            s = *self.nodes[p].inputs.first()?;
        };
        let stage_err = |stage: &str, problem| GraphError::Stage {
            stage: stage.to_owned(),
            problem,
        };
        let mut stage_of = vec![0usize; self.nodes.len()];
        let mut decls: Vec<Option<&Stage>> = vec![None];
        for &i in &self.order {
            let n = &self.nodes[i];
            let mut level = 0;
            for (p, &s) in n.inputs.iter().enumerate() {
                if n.feedback[p] || (p < 64 && n.unconnected & (1 << p) != 0) || s == 0 {
                    continue;
                }
                let Some(&from) = producer.get(s).filter(|&&f| f != usize::MAX) else {
                    continue;
                };
                let mark = source(s).and_then(|u| {
                    self.marks.iter().find(|m| m.from == u && m.to == i)
                });
                let l = stage_of[from] + usize::from(mark.is_some());
                if let Some(m) = mark {
                    if l >= MAX_STAGES {
                        return Err(stage_err(&m.stage.name, StageProblem::TooMany));
                    }
                    if decls.len() <= l {
                        decls.resize(l + 1, None);
                    }
                    match decls[l] {
                        Some(d) if d.name != m.stage.name => {
                            return Err(stage_err(
                                &m.stage.name,
                                StageProblem::Name {
                                    other: d.name.clone(),
                                },
                            ))
                        }
                        Some(_) => {}
                        None => decls[l] = Some(&m.stage),
                    }
                }
                level = level.max(l);
            }
            stage_of[i] = level;
        }
        let name_of = |l: usize| -> String {
            match (l, decls.get(l).copied().flatten()) {
                (_, Some(d)) => d.name.clone(),
                (0, None) => self.first_stage.clone(),
                (l, None) => format!("stage{l}"),
            }
        };
        for (i, n) in self.nodes.iter().enumerate() {
            for (p, &s) in n.inputs.iter().enumerate() {
                if p < 64 && n.unconnected & (1 << p) != 0 {
                    continue;
                }
                if s == 0 && stage_of[i] > 0 {
                    return Err(stage_err(
                        &name_of(stage_of[i]),
                        StageProblem::Input {
                            component: n.id.clone(),
                        },
                    ));
                }
                let from = producer.get(s).copied().unwrap_or(usize::MAX);
                if n.feedback[p] && from != usize::MAX && stage_of[from] != stage_of[i] {
                    return Err(stage_err(
                        &name_of(stage_of[i].max(stage_of[from])),
                        StageProblem::Feedback {
                            from: self.nodes[from].id.clone(),
                            to: n.id.clone(),
                        },
                    ));
                }
            }
        }
        let stages = (0..decls.len())
            .map(|l| {
                let name = name_of(l);
                StageDecl {
                    cost: self
                        .costs
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|&(_, c)| c),
                    ahead_bytes: decls[l].map_or(AHEAD_BYTES, |d| d.ahead_bytes),
                    name,
                }
            })
            .collect::<Vec<_>>();
        self.stage_of = stage_of;
        if stages != self.stages || self.stage_set == sdrkit_core::StageSetId::default() {
            // Other stages: a placement of the old ones ends, and its stage threads return.
            self.deploy = None;
            self.stages = stages;
            self.stage_set = sdrkit_core::StageSetId::fresh();
        }
        Ok(())
    }

    /// Takes every component back from its stage thread and finishes the passes that wait in the queues, in order, on
    /// this thread. Their output is kept for the next [`Graph::process`].
    pub(crate) fn home(&mut self) {
        let Some(mut d) = self.deploy.take() else {
            return;
        };
        if !d.away {
            self.deploy = Some(d);
            return;
        }
        d.away = false;
        for l in &d.links {
            l.set_stop(true);
        }
        let mut lost = false;
        for l in &d.links {
            let crew = {
                let mut slot = lock(&l.slot);
                loop {
                    if slot.lost {
                        break None;
                    }
                    if let Some(c) = slot.crew.take() {
                        slot.returned = false;
                        break Some(c);
                    }
                    slot = l.slot_cv.wait(slot).unwrap_or_else(|p| p.into_inner());
                }
            };
            let Some(mut crew) = crew else {
                lost = true;
                continue;
            };
            for (i, n) in crew.nodes.drain(..) {
                self.nodes[i] = n;
            }
            for &s in &crew.owned {
                self.streams[s] = std::mem::take(&mut crew.streams[s]);
            }
            self.dropped_items += crew.dropped_items;
            if let Some(o) = crew.outlet.take() {
                self.media_audio = o.media_audio;
            }
        }
        self.backlog.append(&mut lock(&d.shared.mailbox));
        self.backlog_full = true;
        let mut pending: Vec<(usize, Unit)> = Vec::new();
        for (k, l) in d.links.iter().enumerate() {
            let mut q = lock(&l.q);
            q.bytes = 0;
            q.spare.clear();
            pending.extend(q.units.drain(..).map(|u| (k + 1, u)));
            drop(q);
            l.set_stop(false);
        }
        let mut asked = std::mem::take(&mut *lock(&d.shared.reformats));
        d.shared.attention.store(false, Ordering::Release);
        *lock(&d.shared.snapshots) = Snapshots::default();
        if lost {
            // The components of a failed thread are gone: the graph stops and its status says why.
            self.configured = false;
            self.stopped = Some(GraphError::StageFailed);
            self.deploy = None;
            self.refresh_results();
            return;
        }
        // The oldest passes wait at the latest cuts.
        pending.sort_by_key(|(_, u)| u.seq);
        asked.sort_by_key(|&(seq, i)| (seq, self.order.iter().position(|&o| o == i)));
        let epoch = d.epoch;
        let group_of_node = std::mem::take(&mut d.group_of_node);
        // The placement stays for the next pass, unless the graph is validated again while it catches up.
        self.deploy = Some(d);
        let mut asked = asked.into_iter().peekable();
        for (group, unit) in pending {
            let mut due: Vec<usize> = Vec::new();
            while let Some(&(_, i)) = asked.peek().filter(|&&(seq, _)| seq < unit.seq) {
                due.push(i);
                asked.next();
            }
            self.reformat_now(due);
            if self.validations != epoch || !self.configured {
                // The streams of the waiting passes are no longer those of the graph.
                self.dropped_items += 1;
                continue;
            }
            self.replay(unit, group, &group_of_node);
        }
        let due: Vec<usize> = asked.map(|(_, i)| i).collect();
        self.reformat_now(due);
    }

    /// Configures again the components `due` and those that requested it in the last replayed pass.
    fn reformat_now(&mut self, mut due: Vec<usize>) {
        due.append(&mut self.reformats);
        if !due.is_empty() {
            self.reformats = due;
            self.apply_reformats();
        }
    }

    /// Runs the pass `unit`, which waited before thread `group`, through the rest of the graph on this thread.
    fn replay(&mut self, mut unit: Unit, group: usize, group_of_node: &[usize]) {
        let mut saved: Vec<(usize, Buffer)> = Vec::with_capacity(unit.bufs.len());
        for (s, b) in unit.bufs.drain(..) {
            saved.push((s, std::mem::replace(&mut self.streams[s], b)));
        }
        {
            let Graph {
                nodes,
                order,
                streams,
                formats,
                consumers,
                outputs,
                max_items,
                dropped_items,
                reformats,
                active,
                metering,
                flow,
                ..
            } = self;
            let mut p = Pass {
                streams,
                formats,
                consumers,
                outputs,
                taps: &[],
                max_items: *max_items,
                dropped_items,
                reformats,
                tick: unit.tick,
                finish: unit.finish,
                iq: &[],
                pass_s: unit.pass_s,
                metering: *metering,
                flow,
                seq: unit.seq,
            };
            for &i in order.iter() {
                if active[i] && group_of_node[i] >= group {
                    run_node(i, &mut nodes[i], &mut p);
                }
            }
        }
        let mut out = std::mem::take(&mut self.backlog);
        self.collect(&mut out);
        self.backlog = out;
        for (s, b) in saved {
            self.streams[s] = b;
        }
    }

    /// Hands the components of the stage threads to their threads, if the graph can run divided now.
    pub(crate) fn scatter(&mut self) {
        let Some(mut d) = self.deploy.take() else {
            return;
        };
        if d.away || !self.configured || !self.taps.is_empty() || self.stopped.is_some() {
            self.deploy = Some(d);
            return;
        }
        let groups = d.links.len() + 1;
        let group_of_node: Vec<usize> = self
            .stage_of
            .iter()
            .map(|&s| d.group_of_stage.get(s).copied().unwrap_or(0))
            .collect();
        let mut producer = vec![0usize; self.streams.len()];
        let mut last_reader = vec![0usize; self.streams.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            for &s in &n.outputs {
                producer[s] = group_of_node[i];
            }
            for (p, &s) in n.inputs.iter().enumerate() {
                if p >= 64 || n.unconnected & (1 << p) == 0 {
                    last_reader[s] = last_reader[s].max(group_of_node[i]);
                }
            }
        }
        // The last thread moves the outputs of the graph to the host.
        for &s in &self.outputs {
            last_reader[s] = groups - 1;
        }
        let n_streams = self.streams.len();
        let crossing = |g: usize| -> Vec<usize> {
            (1..n_streams)
                .filter(|&s| producer[s] < g && last_reader[s] >= g)
                .collect()
        };
        d.forward = crossing(1);
        let mut snaps = Snapshots {
            results: Vec::new(),
            dirty: Vec::new(),
        };
        snaps.results.resize_with(self.nodes.len(), || None);
        *lock(&d.shared.snapshots) = snaps;
        for g in 1..groups {
            let mut crew = Box::new(Crew {
                nodes: Vec::new(),
                streams: Vec::new(),
                owned: Vec::new(),
                forward: if g + 1 < groups {
                    crossing(g + 1)
                } else {
                    Vec::new()
                },
                formats: self.formats.clone(),
                consumers: self.consumers.clone(),
                active: self.active.clone(),
                outputs: self.outputs.clone(),
                max_items: self.max_items,
                dropped_items: 0,
                metering: self.metering,
                flow: self.flow.clone(),
                outlet: None,
                seen: Vec::new(),
                since_copy: 0.0,
                reformats: Vec::new(),
                used: Vec::with_capacity(64),
                arrived: Vec::new(),
            });
            crew.streams.resize_with(self.streams.len(), Buffer::default);
            for k in 0..self.order.len() {
                let i = self.order[k];
                if group_of_node[i] != g {
                    continue;
                }
                let n = &self.nodes[i];
                let results = n.comp.results().map(|r| {
                    let mut copy = Results::new(Arc::clone(r.decl()));
                    copy.copy_from(r);
                    copy
                });
                let stub = Node {
                    id: n.id.clone(),
                    info: n.info.clone(),
                    comp: Box::new(Parked {
                        info: n.info.clone(),
                        values: n.values.clone(),
                        results,
                        commands: n.comp.commands().clone(),
                        latency: n.comp.latency(),
                    }),
                    values: n.values.clone(),
                    inputs: n.inputs.clone(),
                    feedback: n.feedback.clone(),
                    unconnected: n.unconnected,
                    outputs: n.outputs.clone(),
                    kinds: n.kinds.clone(),
                    carries: Vec::new(),
                    scratch: Vec::new(),
                    meter: Arc::clone(&n.meter),
                    in_count: n.in_count.clone(),
                };
                let n = std::mem::replace(&mut self.nodes[i], stub);
                for &s in &n.outputs {
                    crew.owned.push(s);
                    crew.streams[s] = std::mem::take(&mut self.streams[s]);
                }
                crew.seen
                    .push(n.comp.results().map_or(0, Results::change_counter));
                crew.nodes.push((i, n));
            }
            if g + 1 == groups {
                let consumed = (0..self.streams.len())
                    .map(|s| self.nodes.iter().any(|n| n.inputs.contains(&s)))
                    .collect();
                crew.outlet = Some(Outlet {
                    media: self.media.clone(),
                    consumed,
                    media_audio: std::mem::take(&mut self.media_audio),
                    codecs: Arc::clone(&self.ctx.codecs),
                    out: Output::default(),
                });
            }
            let l = &d.links[g - 1];
            let mut slot = lock(&l.slot);
            slot.crew = Some(crew);
            slot.returned = false;
            drop(slot);
            l.slot_cv.notify_all();
        }
        d.group_of_node = group_of_node;
        d.epoch = self.validations;
        d.away = true;
        self.deploy = Some(d);
    }

    /// The end of a pass of the components that stayed on this thread: the pass goes on to the first stage thread, and
    /// what the last stage thread has finished goes into `out`.
    pub(crate) fn after_home_pass(&mut self, tick: bool, finish: bool, pass_s: f64, out: &mut Output) {
        let Some(d) = self.deploy.as_mut() else {
            return;
        };
        // The pass keeps its number across the cut, so that a wait is measured on both sides of the same pass.
        d.seq = self.passes;
        forward(
            &mut self.streams,
            &self.formats,
            &d.forward,
            |_| true,
            &d.links[0],
            (d.seq, tick, finish, pass_s),
            &d.shared.attention,
        );
        if !self.reformats.is_empty() {
            let seq = d.seq;
            lock(&d.shared.reformats).extend(self.reformats.drain(..).map(|i| (seq, i)));
            d.shared.attention.store(true, Ordering::Release);
            self.reformat_here = true;
        }
        out.append(&mut lock(&d.shared.mailbox));
    }

    /// Copies the results that the stage threads left since the last merge into the places of their components.
    pub(crate) fn take_snapshots(&mut self) {
        let Some(d) = self.deploy.as_ref().filter(|d| d.away) else {
            return;
        };
        let mut s = lock(&d.shared.snapshots);
        let Snapshots { results, dirty } = &mut *s;
        for i in dirty.drain(..) {
            let (Some(copy), Some(parked)) = (
                results[i].as_ref(),
                self.nodes[i]
                    .comp
                    .any_mut()
                    .and_then(|a| a.downcast_mut::<Parked>()),
            ) else {
                continue;
            };
            match &mut parked.results {
                Some(r) if Arc::ptr_eq(r.decl(), copy.decl()) => r.copy_from(copy),
                slot => {
                    let mut r = Results::new(Arc::clone(copy.decl()));
                    r.copy_from(copy);
                    *slot = Some(r);
                }
            }
        }
    }
}

impl Graph {
    /// Makes the counters of the streams and of the input ports afresh, with the writer of each stream.
    pub(crate) fn reset_flow(&mut self) {
        self.flow.clear();
        self.flow.resize_with(self.streams.len(), Arc::default);
        self.flow_src.clear();
        self.flow_src.resize(self.streams.len(), (usize::MAX, 0));
        for (i, n) in self.nodes.iter_mut().enumerate() {
            for (o, &s) in n.outputs.iter().enumerate() {
                if let Some(x) = self.flow_src.get_mut(s) {
                    *x = (i, o);
                }
            }
            n.in_count.clear();
            n.in_count.resize_with(n.inputs.len(), Arc::default);
        }
    }
}

impl Drop for Graph {
    fn drop(&mut self) {
        if crate::meter::recording() {
            self.home();
            let mut l = crate::meter::GraphLoad {
                decls: self.stages.clone(),
                ..Default::default()
            };
            self.component_loads(&mut l.components);
            self.stage_loads(&mut l.stages);
            self.connection_loads(&mut l.connections);
            crate::meter::note(l);
        }
    }
}

/// Writes `text` into `dst` and keeps the buffer of `dst`.
fn put_text(dst: &mut String, text: &str) {
    if dst != text {
        dst.clear();
        dst.push_str(text);
    }
}

impl Staged for Graph {
    fn stages(&self) -> Vec<StageDecl> {
        self.stages.clone()
    }

    fn stage_set(&self) -> sdrkit_core::StageSetId {
        self.stage_set
    }

    fn cuts(&self) -> u32 {
        self.deploy.as_ref().map_or(0, |d| d.mask)
    }

    fn cut(&mut self, mask: u32) -> Vec<Box<dyn StageRun>> {
        self.home();
        self.deploy = None;
        let boundaries = self.stages.len().saturating_sub(1);
        let mask = mask & (1u32 << boundaries).wrapping_sub(1);
        if mask == 0 {
            return Vec::new();
        }
        let mut group_of_stage = vec![0; self.stages.len()];
        let mut links = Vec::new();
        for (s, group) in group_of_stage.iter_mut().enumerate().skip(1) {
            if mask & (1 << (s - 1)) != 0 {
                links.push(Arc::new(Link {
                    q: Mutex::default(),
                    not_empty: Condvar::new(),
                    not_full: Condvar::new(),
                    slot: Mutex::default(),
                    slot_cv: Condvar::new(),
                    cap_bytes: self.stages[s].ahead_bytes,
                    stage: s,
                    units_max: AtomicU64::new(0),
                    behind: AtomicU64::new(0),
                    empty: AtomicU64::new(0),
                    held_sum: AtomicU64::new(0),
                    held_n: AtomicU64::new(0),
                    held_max: AtomicU64::new(0),
                    signal_s: AtomicU64::new(0f64.to_bits()),
                }));
            }
            *group = links.len();
        }
        let shared = Arc::new(Shared::default());
        let workers = (0..links.len())
            .map(|k| {
                Box::new(StageWorker {
                    link: Arc::clone(&links[k]),
                    next: links.get(k + 1).cloned(),
                    shared: Arc::clone(&shared),
                }) as Box<dyn StageRun>
            })
            .collect();
        self.deploy = Some(Deployment {
            mask,
            group_of_stage,
            group_of_node: Vec::new(),
            links,
            shared,
            away: false,
            seq: 0,
            forward: Vec::new(),
            epoch: 0,
        });
        workers
    }

    fn gather(&mut self) {
        self.home();
    }

    fn poll(&mut self, out: &mut Output) {
        self.take_backlog(out);
        if let Some(d) = self.deploy.as_ref().filter(|d| d.away) {
            out.append(&mut lock(&d.shared.mailbox));
        }
    }

    fn drain(&mut self, out: &mut Output) {
        self.home();
        self.take_backlog(out);
    }

    fn stage_loads(&self, out: &mut Vec<StageLoad>) {
        out.resize_with(self.stages.len(), StageLoad::default);
        for (s, decl) in self.stages.iter().enumerate() {
            let busy_ns = self
                .nodes
                .iter()
                .zip(&self.stage_of)
                .filter(|(_, &st)| st == s)
                .map(|(n, _)| n.meter.ns.load(Ordering::Relaxed))
                .sum();
            let load = &mut out[s];
            if load.name != decl.name {
                load.name.clone_from(&decl.name);
            }
            (load.busy_ns, load.signal_s, load.cut, load.thread) = (busy_ns, self.signal_s, false, 0);
            (load.queue_bytes, load.queue_cap_bytes) = (0, 0);
            (load.queue_units, load.queue_units_max, load.behind) = (0, 0, 0);
            if let Some(d) = &self.deploy {
                let g = d.group_of_stage[s];
                load.thread = g as u32;
                if g > 0 {
                    let l = &d.links[g - 1];
                    load.signal_s = f64::from_bits(l.signal_s.load(Ordering::Relaxed));
                    if l.stage == s {
                        let q = lock(&l.q);
                        load.cut = true;
                        load.queue_bytes = q.bytes;
                        load.queue_units = q.units.len();
                        load.queue_cap_bytes = l.cap_bytes;
                        load.queue_units_max = l.units_max.load(Ordering::Relaxed) as usize;
                        load.behind = l.behind.load(Ordering::Relaxed);
                    }
                }
            }
        }
    }

    fn connection_loads(&self, out: &mut Vec<ConnectionLoad>) {
        use std::fmt::Write;
        let load = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let mut k = 0;
        for &i in &self.order {
            let n = &self.nodes[i];
            for (p, &s) in n.inputs.iter().enumerate() {
                if s == 0 || (p < 64 && n.unconnected & (1 << p) != 0) {
                    continue;
                }
                let (Some(&(w, o)), Some(f), Some(c)) =
                    (self.flow_src.get(s), self.flow.get(s), n.in_count.get(p))
                else {
                    continue;
                };
                let Some(writer) = self.nodes.get(w) else { continue };
                if out.len() == k {
                    out.push(ConnectionLoad::default());
                }
                let row = &mut out[k];
                k += 1;
                put_text(&mut row.from, &writer.id);
                put_text(&mut row.output, writer.info.outputs.get(o).map_or("", |x| x.name.as_str()));
                put_text(&mut row.to, &n.id);
                put_text(&mut row.input, n.info.inputs.get(p).map_or("", |x| x.name.as_str()));
                row.format.clear();
                let fmt = &self.formats[s];
                let _ = write!(row.format, "{}", fmt.kind);
                if fmt.rate > 0.0 {
                    let _ = write!(row.format, " {}/s", fmt.rate);
                }
                (row.items, row.bytes) = (load(&f.items), load(&f.bytes));
                (row.held_bytes, row.held_sum, row.held_samples, row.held_max) =
                    (load(&f.held), load(&f.held_sum), load(&f.held_n), load(&f.held_max));
                // A stream without a rate holds at most the bound of items of the graph; a sampled stream has no bound.
                row.capacity_bytes = if fmt.kind.is_sampled() || row.items == 0 {
                    0
                } else {
                    self.max_items as u64 * (row.bytes / row.items)
                };
                (row.wait_ns_sum, row.wait_samples, row.wait_worst_ns) =
                    (load(&c.wait_sum), load(&c.wait_n), load(&c.wait_worst));
                (row.full, row.empty) = (load(&f.full), load(&c.empty));
                let (sw, sr) = (
                    self.stage_of.get(w).copied().unwrap_or(0),
                    self.stage_of.get(i).copied().unwrap_or(0),
                );
                (row.boundary, row.stage, row.cut) = (sw != sr, sr, false);
                let Some(d) = &self.deploy else { continue };
                let (gw, gr) = (
                    d.group_of_stage.get(sw).copied().unwrap_or(0),
                    d.group_of_stage.get(sr).copied().unwrap_or(0),
                );
                if gw == gr || gr == 0 {
                    continue;
                }
                // Across a cut the queue before the thread of the reader holds the item.
                let l = &d.links[gr - 1];
                row.cut = true;
                row.held_bytes = lock(&l.q).bytes as u64;
                (row.held_sum, row.held_samples, row.held_max) =
                    (load(&l.held_sum), load(&l.held_n), load(&l.held_max));
                row.capacity_bytes = l.cap_bytes as u64;
                (row.full, row.empty) = (load(&l.behind), load(&l.empty));
            }
        }
        out.truncate(k);
    }

    fn component_loads(&self, out: &mut Vec<ComponentLoad>) {
        let mut k = 0;
        let mut put = |id: &str, section: &'static str, stage, c: [u64; 5]| {
            if out.len() == k {
                out.push(ComponentLoad::default());
            }
            let row = &mut out[k];
            if row.component != id {
                id.clone_into(&mut row.component);
            }
            (row.section, row.stage) = (section, stage);
            [row.busy_ns, row.worst_ns, row.calls, row.items_in, row.items_out] = c;
            k += 1;
        };
        for &i in &self.order {
            let n = &self.nodes[i];
            let m: &Counters = &n.meter;
            let stage = self.stage_of.get(i).copied().unwrap_or(0);
            let load = |a: &AtomicU64| a.load(Ordering::Relaxed);
            let whole = [load(&m.ns), load(&m.worst_ns), load(&m.calls), load(&m.items_in), load(&m.items_out)];
            put(&n.id, "", stage, whole);
            for s in lock(&m.sections).iter() {
                put(&n.id, s.name, stage, [s.ns, s.worst_ns, s.calls, 0, 0]);
            }
        }
        out.truncate(k);
    }
}
