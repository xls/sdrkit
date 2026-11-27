//! A source of sample blocks for sessions (a radio, or samples that the host pushes). One feed fans each block out to its
//! sessions by reference, and runs the derivations that its sessions ask for (a spectrum) once for all of them.

use sdrkit_core::{Complex32, ConfigError, Output, Values};
use sdrkit_driver_core::Gap;
use sdrkit_graph::{
    no_values, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io, Port,
    RunContext, SpectrumSpec, ValidateReport,
};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// The longest time that a feed which waits for room goes without a new try. A session that takes a block wakes the feed
/// sooner; the bound is for a session whose thread ended with no notice.
const RETRY: Duration = Duration::from_millis(10);

/// The samples of one block that a radio feed reads from its ring.
pub(crate) const BLOCK: usize = 16_384;

/// Locks `m`. A lock whose holder panicked still returns its data: every value behind these locks stays whole between two
/// statements.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// A block of samples with the rate and centre they were received at. Sessions read it by reference.
#[derive(Debug, Default)]
pub(crate) struct Block {
    iq: Vec<Complex32>,
    len: usize,
    pub(crate) rate_hz: f64,
    pub(crate) centre_hz: f64,
    /// The places where samples are missing, in order. [`Gap::at`] is the count of samples of the block before the place.
    gaps: Vec<Gap>,
    /// When the feed sent the block.
    pub(crate) sent: Option<std::time::Instant>,
    /// The moves of the radio of the feed ([`Feed::move_asked`]) whose first sample is in this block or came before it.
    pub(crate) moves: u64,
}

impl Block {
    pub(crate) fn samples(&self) -> &[Complex32] {
        &self.iq[..self.len]
    }

    /// The places where samples are missing, in order.
    pub(crate) fn gaps(&self) -> &[Gap] {
        &self.gaps
    }

    /// A block of `iq` with the gaps `gaps`, for tests.
    #[cfg(test)]
    pub(crate) fn of(iq: &[Complex32], gaps: &[Gap]) -> Block {
        Block {
            iq: iq.to_vec(),
            len: iq.len(),
            gaps: gaps.to_vec(),
            ..Block::default()
        }
    }
}

/// A derivation that one session or service requests from the feed, and the newest spectrum frame made for it. A demand with a
/// queue keeps every frame, each with the centre it was made at, until its reader takes them.
#[derive(Debug, Default)]
pub(crate) struct Demand {
    pub(crate) spec: Mutex<Option<SpectrumSpec>>,
    pub(crate) frame: Mutex<Frame>,
    pub(crate) queue: Option<Mutex<FrameQueue>>,
}

/// The frames that wait for the reader of a demand, and the buffers of frames it read, for use again.
#[derive(Debug, Default)]
pub(crate) struct FrameQueue {
    /// Each frame with the centre of the samples it was made from.
    pub(crate) full: std::collections::VecDeque<(f64, Vec<f32>)>,
    pub(crate) free: Vec<Vec<f32>>,
}

/// The frames a queue of a demand holds. The oldest go first.
const QUEUE_CAP: usize = 64;

impl Demand {
    /// A demand of no spectrum yet that keeps every frame.
    pub(crate) fn queued_empty() -> Self {
        Demand {
            spec: Mutex::new(None),
            frame: Mutex::default(),
            queue: Some(Mutex::default()),
        }
    }
}

/// The newest spectrum frame of a demand.
#[derive(Debug, Default)]
pub(crate) struct Frame {
    pub(crate) values: Vec<f32>,
    /// The width of the frame.
    pub(crate) span_hz: f64,
    /// The centre of the samples it was made from.
    pub(crate) centre_hz: f64,
    /// The frames made per second.
    pub(crate) frame_rate: f64,
    /// Frames written so far.
    pub(crate) made: u64,
    /// The value of `made` when the host last read a frame.
    pub(crate) taken: u64,
}

/// One session that reads a feed.
pub(crate) struct Sub {
    pub(crate) session: u64,
    pub(crate) tx: SyncSender<Arc<Block>>,
    /// Samples the session lost because it was behind.
    pub(crate) dropped: Arc<AtomicU64>,
    pub(crate) demand: Arc<Demand>,
    /// The traffic of the queue of the session.
    pub(crate) queue: Arc<QueueMeter>,
    /// The session can set the rate and the centre of the radio. The session of a share cannot: it takes what the
    /// radio gives, and does not count where the engine asks whether a session holds the radio where it is.
    pub(crate) steers: bool,
}

/// The traffic of the queue between a feed and one session. The feed writes the counts of what it sent, the session
/// those of what it took.
#[derive(Debug, Default)]
pub(crate) struct QueueMeter {
    pub(crate) sent: AtomicU64,
    pub(crate) taken: AtomicU64,
    /// The bound of the queue in blocks, and the samples of the last block.
    pub(crate) blocks: AtomicU64,
    pub(crate) block_len: AtomicU64,
    /// Samples that wait after each take: their sum, the takes and the most.
    pub(crate) held_sum: AtomicU64,
    pub(crate) held_n: AtomicU64,
    pub(crate) held_max: AtomicU64,
    /// Nanoseconds that a block waited: the sum, the blocks and the longest.
    pub(crate) wait_sum: AtomicU64,
    pub(crate) wait_n: AtomicU64,
    pub(crate) wait_worst: AtomicU64,
    /// The times the feed found the queue full, and that the session found it empty.
    pub(crate) full: AtomicU64,
    pub(crate) empty: AtomicU64,
}

impl QueueMeter {
    /// Counts the block `b` that the session takes.
    pub(crate) fn take(&self, b: &Block) {
        let n = b.samples().len() as u64;
        let taken = self.taken.fetch_add(n, Ordering::Relaxed) + n;
        let held = self.sent.load(Ordering::Relaxed).saturating_sub(taken);
        self.held_sum.fetch_add(held, Ordering::Relaxed);
        self.held_n.fetch_add(1, Ordering::Relaxed);
        self.held_max.fetch_max(held, Ordering::Relaxed);
        if let Some(at) = b.sent {
            let ns = at.elapsed().as_nanos() as u64;
            self.wait_sum.fetch_add(ns, Ordering::Relaxed);
            self.wait_n.fetch_add(1, Ordering::Relaxed);
            self.wait_worst.fetch_max(ns, Ordering::Relaxed);
        }
    }
}

/// Why a block of a host feed was not delivered.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Undelivered {
    /// The slowest session has not used its earlier blocks.
    Behind,
}

struct FeedRun {
    subs: Vec<Sub>,
    /// The feed's radio is closing: it takes no new sessions.
    closing: bool,
    pool: Vec<Arc<Block>>,
    graph: Option<sdrkit_graph::Graph>,
    /// The rate and centre the graph was configured for.
    graph_input: (f64, f64),
    out: Output,
    /// Demands of services, which read the feed with no session of their own.
    extra: Vec<Arc<Demand>>,
    /// The rate and centre of the newest block, or of the samples that wait for the first reader of a feed that is not
    /// live.
    waiting: (f64, f64),
}

/// A source of blocks: its sessions, its block pool and its derivation graph.
pub(crate) struct Feed {
    run: Mutex<FeedRun>,
    changed: AtomicBool,
    live: bool,
    catalog: Arc<Catalog>,
    ctx: RunContext,
    report: Mutex<ValidateReport>,
    problem: Mutex<Option<GraphError>>,
    /// The centre of the samples that the derivations work on, as the bits of an `f64`.
    centre: Arc<AtomicU64>,
    /// The blocks that the queue of a session of this feed holds.
    queue: AtomicUsize,
    /// Held for the whole of a delivery, and taken before `run`: the blocks stay in order while a delivery waits with
    /// `run` free. The value is the list of the sessions that have not taken the block in hand.
    late: Mutex<Vec<u64>>,
    /// Signalled when a session of this feed took a block or left, and when the waits are cancelled.
    room: Condvar,
    /// A delivery may be waiting for room.
    waits: AtomicBool,
    /// Deliveries do not wait: the source of the feed stops.
    no_wait: AtomicBool,
    /// The moves of the radio of the feed to another centre that were ordered, and those whose first sample the thread
    /// of the radio has reached.
    moves_asked: AtomicU64,
    moves_done: AtomicU64,
}

/// What became of a block that the feed offered to a session.
enum Offer {
    Taken,
    /// The queue of the session is full.
    Full,
    /// The thread of the session has ended.
    Gone,
}

impl Feed {
    /// A feed whose samples arrive in real time if `live`. Its derivations use the adapters of `catalog` and run in `ctx`.
    pub(crate) fn new(live: bool, catalog: Arc<Catalog>, ctx: RunContext) -> Self {
        Feed {
            run: Mutex::new(FeedRun {
                subs: Vec::new(),
                closing: false,
                pool: Vec::new(),
                graph: None,
                graph_input: (0.0, 0.0),
                out: Output::default(),
                extra: Vec::new(),
                waiting: (0.0, 0.0),
            }),
            changed: AtomicBool::new(false),
            queue: AtomicUsize::new(crate::session::QUEUE),
            late: Mutex::default(),
            room: Condvar::new(),
            waits: AtomicBool::new(false),
            no_wait: AtomicBool::new(false),
            moves_asked: AtomicU64::new(0),
            moves_done: AtomicU64::new(0),
            live,
            catalog,
            ctx,
            report: Mutex::new(ValidateReport::default()),
            problem: Mutex::new(None),
            centre: Arc::default(),
        }
    }

    /// A move of the radio of the feed to another centre is ordered. Returns the count of the moves ordered.
    pub(crate) fn move_asked(&self) -> u64 {
        self.moves_asked.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// The moves ordered so far. A session takes a new frequency at the first block that has this many
    /// ([`Block::moves`]).
    pub(crate) fn moves_asked(&self) -> u64 {
        self.moves_asked.load(Ordering::Acquire)
    }

    /// The moves whose first sample the thread of the radio has reached.
    pub(crate) fn moves_done(&self) -> u64 {
        self.moves_done.load(Ordering::Acquire)
    }

    /// The thread of the radio reached the first sample of every move ordered up to `asked`: the next block has it.
    pub(crate) fn moves_done_to(&self, asked: u64) {
        self.moves_done.fetch_max(asked, Ordering::AcqRel);
    }

    pub(crate) fn live(&self) -> bool {
        self.live
    }

    /// True if the signal of the feed is made as it plays (a test signal): [`RunContext::present`].
    pub(crate) fn present(&self) -> bool {
        self.ctx.present
    }

    /// Adds a session. False if the feed's radio is closing: the session is not added.
    pub(crate) fn subscribe(&self, sub: Sub) -> bool {
        let mut run = lock(&self.run);
        if run.closing {
            return false;
        }
        run.subs.push(sub);
        self.demand_changed();
        drop(run);
        self.room.notify_all();
        true
    }

    /// Adds a demand of a service.
    pub(crate) fn add_demand(&self, d: Arc<Demand>) {
        lock(&self.run).extra.push(d);
        self.demand_changed();
        self.room.notify_all();
    }

    /// Removes a demand of a service.
    pub(crate) fn remove_demand(&self, d: &Arc<Demand>) {
        lock(&self.run).extra.retain(|x| !Arc::ptr_eq(x, d));
        self.demand_changed();
    }

    /// The blocks that the queue of a session of this feed holds.
    pub(crate) fn queue(&self) -> usize {
        self.queue.load(Ordering::Relaxed)
    }

    /// Sets the blocks that the queue of a session started from now on holds. At least [`QUEUE`](crate::session::QUEUE).
    pub(crate) fn set_queue(&self, blocks: usize) {
        self.queue
            .store(blocks.max(crate::session::QUEUE), Ordering::Relaxed);
    }

    /// The rate and centre of the newest block, or of the samples that wait for the first reader.
    pub(crate) fn tuning(&self) -> (f64, f64) {
        let run = lock(&self.run);
        if run.graph_input.0 > 0.0 {
            run.graph_input
        } else {
            run.waiting
        }
    }

    /// Returns the number of sessions that read the feed and can steer its radio ([`Sub::steers`]).
    pub(crate) fn readers(&self) -> usize {
        lock(&self.run).subs.iter().filter(|s| s.steers).count()
    }

    /// Waits up to `timeout` for a session or a service to read the feed, whose samples have the rate and centre
    /// `tuning`. Returns true if one reads it, false after the timeout or [`Feed::cancel_waits`]. A feed that is not live
    /// calls it before it takes samples from its source, so that no sample goes to a feed that nobody reads.
    pub(crate) fn wait_for_reader(&self, tuning: (f64, f64), timeout: Duration) -> bool {
        let read = |run: &FeedRun| !run.subs.is_empty() || !run.extra.is_empty();
        let mut run = lock(&self.run);
        if read(&run) {
            return true;
        }
        run.waiting = tuning;
        if self.no_wait.load(Ordering::SeqCst) {
            return false;
        }
        let run = match self.room.wait_timeout(run, timeout) {
            Ok((g, _)) => g,
            Err(p) => p.into_inner().0,
        };
        read(&run)
    }

    /// The source of the feed ended (a recording played to its end). Each session takes the blocks in its queue, finishes
    /// its stack and ends.
    pub(crate) fn input_ended(&self) {
        lock(&self.run).subs.clear();
        self.room.notify_all();
    }

    /// Marks the feed as closing if no session reads it, and returns the number of sessions that read it.
    pub(crate) fn close(&self) -> usize {
        let mut run = lock(&self.run);
        run.closing = run.subs.is_empty();
        run.subs.len()
    }

    pub(crate) fn unsubscribe(&self, session: u64) {
        lock(&self.run).subs.retain(|s| s.session != session);
        self.demand_changed();
        self.room.notify_all();
    }

    pub(crate) fn unsubscribe_all(&self) {
        lock(&self.run).subs.clear();
        self.room.notify_all();
    }

    /// Tells the feed that a session took a block from its queue. A delivery that waits for room tries again.
    pub(crate) fn took(&self) {
        if self.waits.load(Ordering::SeqCst) {
            // With the lock, the delivery is either in its wait or before its next try: the signal is not lost.
            drop(lock(&self.run));
            self.room.notify_all();
        }
    }

    /// With `on`, a delivery that waits for room gives up, and no delivery waits: the source of the feed stops. A session
    /// that had no room then loses the block and counts it. Without `on`, deliveries wait again.
    pub(crate) fn cancel_waits(&self, on: bool) {
        self.no_wait.store(on, Ordering::SeqCst);
        if on {
            drop(lock(&self.run));
            self.room.notify_all();
        }
    }

    /// Marks the demands as changed. The next block builds the derivation graph again.
    pub(crate) fn demand_changed(&self) {
        self.changed.store(true, Ordering::Release);
    }

    /// The report of the derivation graph: each derivation it made and the demands that read it.
    pub(crate) fn report(&self) -> ValidateReport {
        lock(&self.report).clone()
    }

    /// Why the derivation graph did not build, if it did not. The error is given once.
    pub(crate) fn take_problem(&self) -> Option<GraphError> {
        lock(&self.problem).take()
    }

    /// Fills a free block with `fill` (which returns the number of samples it wrote), sends it to every session and runs
    /// the derivations on it. What happens to a session whose queue is full depends on the feed:
    ///
    /// - A live feed does not wait: the session loses the block, and its count of dropped samples grows.
    /// - A feed that is not live (a file, a generator, the channel of a bank on such a radio) waits until every session
    ///   has room, so no session loses a sample and the slowest session sets the pace of the feed and of all its sessions.
    ///   The wait holds no lock that another call needs: sessions join and leave, and the feed answers its other calls. The
    ///   wait ends for a session that leaves or whose thread ended, and for all of them at [`Feed::cancel_waits`].
    /// - If `host` is set, the host fills the feed and waits for the sessions itself: the block is not filled while the
    ///   slowest session has [`QUEUE`](crate::session::QUEUE) blocks that it has not used.
    pub(crate) fn deliver(
        &self,
        rate_hz: f64,
        centre_hz: f64,
        host: bool,
        fill: impl FnOnce(&mut Vec<Complex32>) -> usize,
    ) -> Result<usize, Undelivered> {
        self.deliver_gaps(rate_hz, centre_hz, host, |iq, _| fill(iq))
    }

    /// [`Feed::deliver`] for a source that knows where samples are missing: `fill` also writes each such place, in order,
    /// with [`Gap::at`] the samples of the block before it.
    pub(crate) fn deliver_gaps(
        &self,
        rate_hz: f64,
        centre_hz: f64,
        host: bool,
        fill: impl FnOnce(&mut Vec<Complex32>, &mut Vec<Gap>) -> usize,
    ) -> Result<usize, Undelivered> {
        let mut late = lock(&self.late);
        let mut guard = lock(&self.run);
        let run = &mut *guard;
        if self.changed.swap(false, Ordering::AcqRel) {
            self.rebuild(run);
        }
        // Every session takes every block of a host feed: the blocks still in use are the backlog of the slowest one.
        if host && run.pool.iter().filter(|b| Arc::strong_count(b) > 1).count() >= self.queue() {
            return Err(Undelivered::Behind);
        }
        let i = match run.pool.iter_mut().position(|b| Arc::get_mut(b).is_some()) {
            Some(i) => i,
            None => {
                run.pool.push(Arc::new(Block::default()));
                run.pool.len() - 1
            }
        };
        let Some(block) = Arc::get_mut(&mut run.pool[i]) else {
            return Ok(0);
        };
        block.gaps.clear();
        let n = fill(&mut block.iq, &mut block.gaps);
        block.len = n.min(block.iq.len());
        block.rate_hz = rate_hz;
        block.centre_hz = centre_hz;
        block.moves = self.moves_done();
        if block.len == 0 {
            return Ok(0);
        }
        let n = block.len;
        block.sent = Some(std::time::Instant::now());
        let wait = !self.live && !host && !self.no_wait.load(Ordering::SeqCst);
        late.clear();
        // Set before the first try, so that a session which makes room after a failed try sees it.
        self.waits.store(wait, Ordering::SeqCst);
        for s in &run.subs {
            if let Offer::Full = Self::offer(s, &run.pool[i]) {
                s.queue.full.fetch_add(1, Ordering::Relaxed);
                if wait {
                    late.push(s.session);
                } else {
                    s.dropped.fetch_add(n as u64, Ordering::Relaxed);
                }
            }
            s.queue.block_len.store(n as u64, Ordering::Relaxed);
        }
        if !late.is_empty() {
            let block = Arc::clone(&run.pool[i]);
            guard = self.wait_for_room(guard, &mut late, &block);
        }
        self.waits.store(false, Ordering::SeqCst);
        let run = &mut *guard;
        let changed = run.graph_input != (rate_hz, centre_hz);
        run.graph_input = (rate_hz, centre_hz);
        run.waiting = (rate_hz, centre_hz);
        if let Some(g) = run.graph.as_mut() {
            if changed {
                self.centre.store(centre_hz.to_bits(), Ordering::Relaxed);
                let configured = g.configure_input(Format::iq(rate_hz, centre_hz, centre_hz));
                self.note(configured.map(|()| g.report().clone()));
            }
            run.out.clear();
            g.process(run.pool[i].samples(), &mut run.out);
        }
        Ok(n)
    }

    /// Offers `block` to the session `s` and keeps the count of what was sent.
    fn offer(s: &Sub, block: &Arc<Block>) -> Offer {
        let n = block.len as u64;
        // The count goes up before the send, so that the session never takes more than was sent.
        s.queue.sent.fetch_add(n, Ordering::Relaxed);
        let offer = match s.tx.try_send(Arc::clone(block)) {
            Ok(()) => return Offer::Taken,
            Err(TrySendError::Disconnected(_)) => Offer::Gone,
            Err(TrySendError::Full(_)) => Offer::Full,
        };
        s.queue.sent.fetch_sub(n, Ordering::Relaxed);
        offer
    }

    /// Waits until each session of `late` has taken `block`, has left the feed or has ended. The lock of `guard` is free
    /// during each wait. After [`Feed::cancel_waits`] the sessions still in `late` lose the block and count it.
    fn wait_for_room<'a>(
        &'a self,
        mut guard: MutexGuard<'a, FeedRun>,
        late: &mut Vec<u64>,
        block: &Arc<Block>,
    ) -> MutexGuard<'a, FeedRun> {
        while !late.is_empty() {
            if self.no_wait.load(Ordering::SeqCst) {
                for s in guard.subs.iter().filter(|s| late.contains(&s.session)) {
                    s.dropped.fetch_add(block.len as u64, Ordering::Relaxed);
                }
                late.clear();
                break;
            }
            guard = match self.room.wait_timeout(guard, RETRY) {
                Ok((g, _)) => g,
                Err(p) => p.into_inner().0,
            };
            let subs = &guard.subs;
            late.retain(|id| {
                let sub = subs.iter().find(|s| s.session == *id);
                sub.is_some_and(|s| matches!(Self::offer(s, block), Offer::Full))
            });
        }
        guard
    }

    fn note(&self, r: Result<ValidateReport, GraphError>) {
        match r {
            Ok(report) => {
                *lock(&self.report) = report;
                *lock(&self.problem) = None;
            }
            Err(e) => {
                *lock(&self.report) = ValidateReport::default();
                *lock(&self.problem) = Some(e);
            }
        }
    }

    /// Builds the derivation graph from the demands of the sessions: one consumer per demand, so that equal demands read one
    /// derivation.
    fn rebuild(&self, run: &mut FeedRun) {
        run.graph = None;
        let input = run.graph_input;
        run.graph_input = (0.0, 0.0);
        let mut b = GraphBuilder::new().catalog(&self.catalog);
        let mut any = false;
        let demands = run.subs.iter().map(|s| &s.demand).chain(&run.extra);
        for (i, demand) in demands.enumerate() {
            if let Some(spec) = *lock(&demand.spec) {
                let id = format!("spectrum{i}");
                b = b
                    .add(
                        &id,
                        SpectrumDemand {
                            spec,
                            demand: Arc::clone(demand),
                            centre: Arc::clone(&self.centre),
                            span_hz: 0.0,
                        },
                    )
                    .input(&id);
                any = true;
            }
        }
        if !any {
            self.note(Ok(ValidateReport::default()));
            return;
        }
        match b.build() {
            Ok(mut g) => {
                g.set_context(RunContext {
                    live: self.live,
                    ..self.ctx.clone()
                });
                run.graph = Some(g);
                // The next block configures the graph again, at the same input.
                run.graph_input = (input.0, f64::NAN);
            }
            Err(e) => self.note(Err(e)),
        }
    }
}

/// The consumer of one session's spectrum: it keeps the newest frame for the host.
struct SpectrumDemand {
    spec: SpectrumSpec,
    demand: Arc<Demand>,
    /// The centre of the samples now, as the bits of an `f64`.
    centre: Arc<AtomicU64>,
    /// The width of the frames.
    span_hz: f64,
}

impl Component for SpectrumDemand {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("engine.spectrum", vec![Port::spectrum("in", self.spec)])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.span_hz = inputs[0].bandwidth_hz;
        Ok(Vec::new())
    }

    fn process(&mut self, io: &mut Io) {
        let frame_rate = io.input_format(0).rate;
        let frames = io.spectrum(0);
        let Some(last) = frames.len().checked_sub(1) else {
            return;
        };
        if let Some(q) = &self.demand.queue {
            let centre = f64::from_bits(self.centre.load(Ordering::Relaxed));
            let mut q = lock(q);
            for i in 0..frames.len() {
                if q.full.len() == QUEUE_CAP {
                    if let Some((_, b)) = q.full.pop_front() {
                        q.free.push(b);
                    }
                }
                let mut b = q.free.pop().unwrap_or_default();
                b.clear();
                b.extend_from_slice(frames.frame(i));
                q.full.push_back((centre, b));
            }
            return;
        }
        // The host may be copying the frame. Then this frame is skipped and the next one is kept.
        if let Ok(mut f) = self.demand.frame.try_lock() {
            f.values.clear();
            f.values.extend_from_slice(frames.frame(last));
            f.made += 1;
            f.span_hz = self.span_hz;
            f.centre_hz = f64::from_bits(self.centre.load(Ordering::Relaxed));
            f.frame_rate = frame_rate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{sync_channel, Receiver};
    use std::time::Instant;

    /// The blocks that the queue of a test session holds.
    const ROOM: usize = 2;
    /// The samples of a test block.
    const LEN: usize = 100;
    /// The bound for a call that must not wait for a session.
    const PROMPT: Duration = Duration::from_secs(2);

    fn feed(live: bool) -> Arc<Feed> {
        let catalog = Arc::new(Catalog::default());
        Arc::new(Feed::new(live, catalog, RunContext::default()))
    }

    /// Adds session `id` to `feed` and returns its end of the queue, with its counts.
    fn join(feed: &Feed, id: u64) -> (Receiver<Arc<Block>>, Arc<AtomicU64>, Arc<QueueMeter>) {
        let (tx, rx) = sync_channel(ROOM);
        let (dropped, queue) = (Arc::<AtomicU64>::default(), Arc::<QueueMeter>::default());
        assert!(feed.subscribe(Sub {
            session: id,
            tx,
            dropped: Arc::clone(&dropped),
            demand: Arc::default(),
            queue: Arc::clone(&queue),
            steers: true,
        }));
        (rx, dropped, queue)
    }

    /// Delivers block `k`: every sample has the value `k`.
    fn send(feed: &Feed, k: usize) {
        let n = feed.deliver(48e3, 100e6, false, |iq| {
            iq.clear();
            iq.resize(LEN, Complex32::new(k as f32, 0.0));
            LEN
        });
        assert_eq!(n, Ok(LEN));
    }

    /// Takes blocks as a session does, `pause` apart, until the feed has no more. Returns the number of each block.
    fn read(
        feed: &Feed,
        rx: Receiver<Arc<Block>>,
        meter: &QueueMeter,
        pause: Duration,
    ) -> Vec<usize> {
        let mut got = Vec::new();
        while let Ok(b) = rx.recv_timeout(Duration::from_millis(500)) {
            meter.take(&b);
            feed.took();
            assert!(b.samples().iter().all(|s| *s == b.samples()[0]));
            got.push(b.samples()[0].re as usize);
            drop(b);
            std::thread::sleep(pause);
        }
        got
    }

    #[test]
    fn a_slow_session_of_a_feed_that_is_not_live_gets_every_sample_in_order() {
        let feed = feed(false);
        let (rx, dropped, meter) = join(&feed, 1);
        let got = std::thread::scope(|s| {
            let (f, m) = (&*feed, &*meter);
            let reader = s.spawn(move || read(f, rx, m, Duration::from_millis(3)));
            (0..60).for_each(|k| send(&feed, k));
            reader.join().unwrap()
        });
        assert_eq!(got, (0..60).collect::<Vec<_>>());
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
        let sent = meter.sent.load(Ordering::Relaxed);
        assert_eq!(sent, 60 * LEN as u64);
        assert_eq!(sent, meter.taken.load(Ordering::Relaxed));
        // The feed found the queue full and waited.
        assert!(meter.full.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn a_slow_session_of_a_live_feed_loses_blocks_and_counts_them() {
        let feed = feed(true);
        let (rx, dropped, meter) = join(&feed, 1);
        let start = Instant::now();
        (0..60).for_each(|k| send(&feed, k));
        // The feed did not wait for the session, which took nothing.
        assert!(start.elapsed() < PROMPT);
        let got = read(&feed, rx, &meter, Duration::ZERO);
        assert_eq!(got, [0, 1]);
        assert_eq!(dropped.load(Ordering::Relaxed), 58 * LEN as u64);
        assert_eq!(meter.sent.load(Ordering::Relaxed), 2 * LEN as u64);
    }

    #[test]
    fn the_slowest_session_sets_the_pace_and_every_session_gets_every_block() {
        let feed = feed(false);
        let (fast_rx, fast_dropped, fast_meter) = join(&feed, 1);
        let (slow_rx, slow_dropped, slow_meter) = join(&feed, 2);
        let (fast, slow) = std::thread::scope(|s| {
            let (f, fm, sm) = (&*feed, &*fast_meter, &*slow_meter);
            let fast = s.spawn(move || read(f, fast_rx, fm, Duration::ZERO));
            let slow = s.spawn(move || read(f, slow_rx, sm, Duration::from_millis(3)));
            (0..40).for_each(|k| send(&feed, k));
            (fast.join().unwrap(), slow.join().unwrap())
        });
        assert_eq!(fast, (0..40).collect::<Vec<_>>());
        assert_eq!(slow, fast);
        assert_eq!(fast_dropped.load(Ordering::Relaxed), 0);
        assert_eq!(slow_dropped.load(Ordering::Relaxed), 0);
    }

    /// Fills the queue of the session of `rx`, then starts a delivery that waits. Returns the thread of the delivery
    /// once it waits.
    fn stall<'s>(
        s: &'s std::thread::Scope<'s, '_>,
        feed: &'s Feed,
    ) -> std::thread::ScopedJoinHandle<'s, Duration> {
        (0..ROOM).for_each(|k| send(feed, k));
        let waiter = s.spawn(move || {
            let start = Instant::now();
            send(feed, ROOM);
            send(feed, ROOM + 1);
            start.elapsed()
        });
        std::thread::sleep(Duration::from_millis(100));
        assert!(!waiter.is_finished(), "the feed waits for the session");
        waiter
    }

    #[test]
    fn a_feed_that_waits_answers_its_other_calls_and_goes_on_when_the_session_leaves() {
        let feed = feed(false);
        let (rx, dropped, _) = join(&feed, 1);
        std::thread::scope(|s| {
            let waiter = stall(s, &feed);
            let start = Instant::now();
            // The calls of a retune, of a change of a setting and of a new session.
            assert_eq!(feed.readers(), 1);
            let _ = feed.tuning();
            feed.demand_changed();
            feed.add_demand(Arc::default());
            let (other, _, _) = join(&feed, 2);
            feed.unsubscribe(2);
            drop(other);
            assert!(start.elapsed() < PROMPT);
            assert!(!waiter.is_finished());
            // The session never takes a block. It is closed: the feed goes on at once.
            let start = Instant::now();
            feed.unsubscribe(1);
            waiter.join().unwrap();
            assert!(start.elapsed() < PROMPT);
        });
        // A session that left did not lose the block: it did not want it.
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
        assert_eq!(rx.try_iter().count(), ROOM);
    }

    #[test]
    fn a_feed_that_waits_goes_on_when_the_thread_of_the_session_ends_or_all_sessions_leave() {
        let feed = feed(false);
        let (rx, _, _) = join(&feed, 1);
        std::thread::scope(|s| {
            let waiter = stall(s, &feed);
            // The thread ended with no notice to the feed.
            let start = Instant::now();
            drop(rx);
            waiter.join().unwrap();
            assert!(start.elapsed() < PROMPT);
        });
        let (_rx, _, _) = join(&feed, 2);
        std::thread::scope(|s| {
            let waiter = stall(s, &feed);
            // The engine shuts down.
            let start = Instant::now();
            feed.unsubscribe_all();
            waiter.join().unwrap();
            assert!(start.elapsed() < PROMPT);
        });
    }

    #[test]
    fn a_feed_whose_source_stops_does_not_wait_and_counts_what_the_session_lost() {
        let feed = feed(false);
        let (rx, dropped, _) = join(&feed, 1);
        std::thread::scope(|s| {
            let waiter = stall(s, &feed);
            let start = Instant::now();
            feed.cancel_waits(true);
            waiter.join().unwrap();
            assert!(start.elapsed() < PROMPT);
        });
        // The block in hand and the one after it.
        assert_eq!(dropped.load(Ordering::Relaxed), 2 * LEN as u64);
        // The source starts again: the feed waits again.
        feed.cancel_waits(false);
        std::thread::scope(|s| {
            let waiter = s.spawn(|| send(&feed, 9));
            std::thread::sleep(Duration::from_millis(100));
            assert!(!waiter.is_finished());
            assert_eq!(rx.recv().unwrap().samples()[0].re, 0.0);
            feed.took();
            waiter.join().unwrap();
        });
        assert_eq!(dropped.load(Ordering::Relaxed), 2 * LEN as u64);
    }
}
