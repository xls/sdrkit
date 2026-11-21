//! [`RunContext`]: the run information a graph passes to its components through [`Component::attach`](crate::Component::attach):
//! whether the samples are live, the station context, the helper threads, the host's audio codec plug-ins, storage, clock, random
//! source, network and streams, and the test signals of the stacks.
//!
//! Slow work that must not block the decoding thread runs on a [`Pool`] of helper threads. The host sets the pool
//! ([`Graph::set_context`](crate::Graph)); a graph without one uses [`Pool::shared`]. Components never start threads of their
//! own.

use crate::components::{HostStreams, MakeGenerator};
use sdrkit_core::codec::CodecSet;
use sdrkit_core::net::Transport;
use sdrkit_core::{Clock, MemStorage, Random, Station, Storage};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// The run information a component receives beyond its formats.
#[derive(Clone)]
pub struct RunContext {
    /// The samples come from a radio as they are received (heavy work must then go to helper threads); `false` for a recording
    /// or a generator, which waits for the decoder, so work runs in line and results do not depend on timing.
    pub live: bool,
    /// The receiving station.
    pub station: Arc<Station>,
    /// The transmitter is on: the host's transmit switch is on and the radio can transmit
    /// ([`Receiver::set_transmit`](sdrkit_core::Receiver::set_transmit)). A component makes no transmission while it
    /// is false.
    pub transmit: bool,
    /// The helper threads.
    pub pool: Pool,
    /// The audio codec plug-ins the host registered (empty when it returned none). A component that decodes speech takes its own
    /// copy in [`Component::attach`](crate::Component::attach) and runs each plug-in in a
    /// [`GuardedCodec`](crate::codec::GuardedCodec). If a plug-in fails, the component turns it off in its own copy with
    /// [`CodecSet::disable`] and shows the notice. The host supplies the set with
    /// [`Graph::set_codecs`](crate::Graph::set_codecs) or [`StackGraphBuilder::with_codecs`](crate::StackGraphBuilder::with_codecs).
    pub codecs: Arc<CodecSet>,
    /// The host's files and folders. A component reads code tables and writes recordings through it.
    pub storage: Arc<dyn Storage>,
    /// The host's wall clock.
    pub clock: Clock,
    /// The host's random source.
    pub random: Random,
    /// The host's network, for a network sink. `None` without one.
    pub transport: Option<Arc<dyn Transport>>,
    /// The host's streams of this graph, for the host and transmit endpoints. `None` without a host.
    pub streams: Option<Arc<dyn HostStreams>>,
    /// The test signals of the stacks, for a stack's signal as a source. `None` without them.
    pub generators: Option<MakeGenerator>,
}

impl std::fmt::Debug for RunContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunContext")
            .field("live", &self.live)
            .field("station", &self.station)
            .field("pool", &self.pool)
            .finish_non_exhaustive()
    }
}

impl Default for RunContext {
    /// A context for tests and for a graph that runs without a host: not live, the default station, the shared pool, no
    /// codecs, empty storage in memory, a clock fixed at 0, the random source of seed 0, and no network, host streams or test
    /// signals.
    fn default() -> Self {
        RunContext {
            live: false,
            station: Arc::new(Station::default()),
            transmit: false,
            pool: Pool::shared(),
            codecs: Arc::new(CodecSet::default()),
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(0.0),
            random: Random::fixed(0),
            transport: None,
            streams: None,
            generators: None,
        }
    }
}

type Job = Box<dyn FnOnce() + Send>;

/// One [`Pool::map`]: the items that wait, the results by index, and the count of finished items.
struct MapJob<T, R, F> {
    items: Vec<Mutex<Option<T>>>,
    results: Vec<Mutex<Option<std::thread::Result<R>>>>,
    next: AtomicUsize,
    done: Mutex<usize>,
    all_done: Condvar,
    f: F,
}

impl<T, R, F: Fn(usize, T) -> R> MapJob<T, R, F> {
    /// Takes items until none is left.
    fn work(&self) {
        loop {
            let i = self.next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = self.items.get(i) else {
                return;
            };
            let Some(t) = item.lock().unwrap_or_else(|p| p.into_inner()).take() else {
                continue;
            };
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.f)(i, t)));
            *self.results[i].lock().unwrap_or_else(|p| p.into_inner()) = Some(r);
            let mut done = self.done.lock().unwrap_or_else(|p| p.into_inner());
            *done += 1;
            if *done == self.items.len() {
                self.all_done.notify_all();
            }
        }
    }
}

struct Inner {
    threads: usize,
    /// The most helper threads of one [`Pool::map`].
    workers: AtomicUsize,
    queue: Mutex<Option<Sender<Job>>>,
    started: OnceLock<()>,
    /// Runs on each helper thread when it starts.
    on_start: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// A small pool of helper threads for work too slow for the decoding thread. Cloning shares the pool.
#[derive(Clone)]
pub struct Pool(Arc<Inner>);

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("threads", &self.0.threads)
            .finish()
    }
}

/// A job that panicked on a helper thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobPanicked;

impl std::fmt::Display for JobPanicked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a helper-thread job panicked")
    }
}

impl std::error::Error for JobPanicked {}

/// The result of a job, collected later without waiting ([`Ticket::try_take`]).
#[derive(Debug)]
pub struct Ticket<T> {
    rx: Receiver<Result<T, JobPanicked>>,
    done: bool,
}

impl<T> Ticket<T> {
    /// The result when the job has finished; `None` while it runs (and after the result was taken).
    pub fn try_take(&mut self) -> Option<Result<T, JobPanicked>> {
        if self.done {
            return None;
        }
        match self.rx.try_recv() {
            Ok(r) => {
                self.done = true;
                Some(r)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.done = true;
                Some(Err(JobPanicked))
            }
        }
    }
    /// Waits for the result (for tests and for shutting down; never on the decoding thread).
    pub fn wait(mut self) -> Result<T, JobPanicked> {
        if self.done {
            return Err(JobPanicked);
        }
        self.done = true;
        self.rx.recv().unwrap_or(Err(JobPanicked))
    }
}

impl Pool {
    /// A pool of `threads` helper threads (at least 1), started on the first job.
    pub fn new(threads: usize) -> Pool {
        Pool::make(threads, None)
    }
    /// A pool like [`Pool::new`] whose helper threads each call `on_start` first. The owner of the pool sets the priority
    /// of its threads there.
    pub fn with_start(threads: usize, on_start: impl Fn() + Send + Sync + 'static) -> Pool {
        Pool::make(threads, Some(Arc::new(on_start)))
    }
    fn make(threads: usize, on_start: Option<Arc<dyn Fn() + Send + Sync>>) -> Pool {
        Pool(Arc::new(Inner {
            on_start,
            threads: threads.max(1),
            workers: AtomicUsize::new(threads.max(1)),
            queue: Mutex::new(None),
            started: OnceLock::new(),
        }))
    }
    /// The process-wide pool used when the host supplies none: one thread fewer than the cores, from 1 to 4.
    pub fn shared() -> Pool {
        static SHARED: OnceLock<Pool> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
                Pool::new(cores.saturating_sub(1).clamp(1, 4))
            })
            .clone()
    }
    /// How many threads it has.
    pub fn threads(&self) -> usize {
        self.0.threads
    }
    /// The most helper threads that one [`Pool::map`] uses besides the calling thread.
    pub fn workers(&self) -> usize {
        self.0.workers.load(Ordering::Relaxed)
    }
    /// Sets the most helper threads that one [`Pool::map`] uses, from 0 to [`Pool::threads`]. The owner of the pool
    /// calls it; a component does not.
    pub fn set_workers(&self, workers: usize) {
        self.0
            .workers
            .store(workers.min(self.0.threads), Ordering::Relaxed);
    }
    /// Runs `f` on every item and returns the results in the order of the items. The calling thread works too, and
    /// helper threads that are free take items. With no free helper the call runs in line. The results do not depend on
    /// the number of threads. A panic of `f` continues on the calling thread.
    pub fn map<T, R>(&self, items: Vec<T>, f: impl Fn(usize, T) -> R + Send + Sync + 'static) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
    {
        let n = items.len();
        let helpers = self.workers().min(n.saturating_sub(1));
        if helpers == 0 {
            return items.into_iter().enumerate().map(|(i, t)| f(i, t)).collect();
        }
        let job = Arc::new(MapJob {
            items: items.into_iter().map(|t| Mutex::new(Some(t))).collect(),
            results: (0..n).map(|_| Mutex::new(None)).collect(),
            next: AtomicUsize::new(0),
            done: Mutex::new(0),
            all_done: Condvar::new(),
            f,
        });
        self.start();
        if let Ok(q) = self.0.queue.lock() {
            if let Some(q) = q.as_ref() {
                for _ in 0..helpers {
                    let job = Arc::clone(&job);
                    let _ = q.send(Box::new(move || job.work()));
                }
            }
        }
        job.work();
        let mut done = job.done.lock().unwrap_or_else(|p| p.into_inner());
        while *done < n {
            done = job.all_done.wait(done).unwrap_or_else(|p| p.into_inner());
        }
        drop(done);
        job.results
            .iter()
            .map(|r| {
                match r.lock().unwrap_or_else(|p| p.into_inner()).take() {
                    Some(Ok(v)) => v,
                    Some(Err(panic)) => std::panic::resume_unwind(panic),
                    None => unreachable!("every item has a result"),
                }
            })
            .collect()
    }
    fn start(&self) {
        self.0.started.get_or_init(|| {
            let (tx, rx) = channel::<Job>();
            let rx = Arc::new(Mutex::new(rx));
            for i in 0..self.0.threads {
                let rx = rx.clone();
                let on_start = self.0.on_start.clone();
                let _ = std::thread::Builder::new()
                    .name(format!("sdrkit-helper-{i}"))
                    .spawn(move || {
                        if let Some(f) = on_start {
                            f();
                        }
                        loop {
                        let job = {
                            let Ok(guard) = rx.lock() else { return };
                            match guard.recv() {
                                Ok(j) => j,
                                Err(_) => return,
                            }
                        };
                        job();
                        }
                    });
            }
            if let Ok(mut q) = self.0.queue.lock() {
                *q = Some(tx);
            }
        });
    }
    /// Runs `job` on a helper thread and returns a ticket for its result. A job that panics returns [`JobPanicked`] (the thread
    /// carries on).
    pub fn spawn<T: Send + 'static>(&self, job: impl FnOnce() -> T + Send + 'static) -> Ticket<T> {
        self.start();
        let (tx, rx) = channel();
        let wrapped: Job = Box::new(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                .map_err(|_| JobPanicked);
            let _ = tx.send(r);
        });
        // When no thread could be started the job is dropped with its sender, and the ticket reports it as failed.
        if let Ok(q) = self.0.queue.lock() {
            if let Some(q) = q.as_ref() {
                let _ = q.send(wrapped);
            }
        }
        Ticket { rx, done: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_map_returns_the_results_in_order_on_any_number_of_workers() {
        let work = |i: usize, x: u64| {
            // Items of unequal length, so that they finish out of order.
            let mut h = x;
            for _ in 0..(1000 * (17 - i % 17)) {
                h = h.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            }
            (i, h)
        };
        let items: Vec<u64> = (0..500).collect();
        let pool = Pool::new(8);
        pool.set_workers(0);
        let inline = pool.map(items.clone(), work);
        assert!(inline.iter().enumerate().all(|(k, r)| r.0 == k));
        for workers in [1, 8] {
            pool.set_workers(workers);
            assert_eq!(pool.workers(), workers);
            for _ in 0..20 {
                assert!(pool.map(items.clone(), work) == inline, "{workers} workers");
            }
        }
        assert_eq!(pool.map(Vec::<u64>::new(), work), []);
        // More helpers are asked for than threads exist while every thread is busy: the caller does the work.
        let busy = Pool::new(1);
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let g = Arc::clone(&gate);
        let held = busy.spawn(move || {
            let mut open = g.0.lock().unwrap();
            while !*open {
                open = g.1.wait(open).unwrap();
            }
        });
        assert!(busy.map(items.clone(), work) == inline);
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        held.wait().unwrap();
    }

    #[test]
    fn a_panic_in_a_map_reaches_the_caller() {
        let pool = Pool::new(2);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pool.map(vec![1, 2, 3, 4], |_, x: i32| assert!(x != 3, "three"))
        }));
        assert!(r.is_err());
        assert_eq!(pool.map(vec![1, 2], |_, x: i32| x * 2), [2, 4]);
    }

    #[test]
    fn jobs_run_on_helpers_and_a_panic_is_contained() {
        let pool = Pool::new(2);
        let t = pool.spawn(|| (0..1000u64).sum::<u64>());
        assert_eq!(t.wait(), Ok(499_500));
        let mut bad = pool.spawn(|| -> u32 { panic!("bad job") });
        let r = loop {
            if let Some(r) = bad.try_take() {
                break r;
            }
            std::thread::yield_now();
        };
        assert_eq!(r, Err(JobPanicked));
        assert!(bad.try_take().is_none());
        // The pool still works after a panic.
        assert_eq!(pool.spawn(|| 7).wait(), Ok(7));
        assert!(Pool::shared().threads() >= 1);
    }
}
