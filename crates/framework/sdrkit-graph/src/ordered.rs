//! [`InOrder`]: independent jobs on the helper threads of a [`Pool`], with the results taken back in the order the jobs were
//! given. A fixed ring of slots bounds the jobs in flight and the finished results that wait for an older one (the reorder
//! window). When every slot is in use, [`InOrder::give`] refuses the job; the caller then skips the work and counts it, or makes
//! room with [`InOrder::wait_one`].
//!
//! A slot holds a payload of the caller's type: the caller fills it, a worker changes it in place, and the caller reads it when
//! it takes the slot back. Payloads and worker states are made once and reused, so a job allocates nothing once its payload has
//! grown to its working size. The one allocation left is the small task that starts a worker on the pool after the workers
//! were idle.
//!
//! The caller keeps a worker state of its own. With no helpers a job runs in line in [`InOrder::give`], and the waiting calls
//! work on queued jobs instead of only waiting. The results never depend on the number of helpers.

use crate::context::{JobPanicked, Pool};
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Free,
    Queued,
    Running,
    Done,
    Failed,
}

struct Ctl<S> {
    slots: Box<[Slot]>,
    /// Queued slots, oldest first.
    queue: VecDeque<usize>,
    /// The worker states that no helper holds.
    states: Vec<S>,
    /// Helper tasks that run or wait on the pool.
    running: usize,
}

type Work<S, T> = dyn Fn(&mut S, &mut T) + Send + Sync;

struct Shared<T, S> {
    payloads: Box<[Mutex<T>]>,
    ctl: Mutex<Ctl<S>>,
    finished: Condvar,
    work: Box<Work<S, T>>,
}

fn lock<V>(m: &Mutex<V>) -> MutexGuard<'_, V> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl<T, S> Shared<T, S> {
    /// Runs job `slot` with `state` and marks it done or failed. A panic of the job does not reach the caller.
    fn run(&self, slot: usize, state: &mut S) -> Slot {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (self.work)(state, &mut lock(&self.payloads[slot]));
        }));
        if r.is_ok() {
            Slot::Done
        } else {
            Slot::Failed
        }
    }

    /// The loop of one helper task: takes queued jobs until none is left.
    fn drain(&self) {
        let mut c = lock(&self.ctl);
        loop {
            let Some(slot) = c.queue.pop_front() else {
                c.running -= 1;
                return;
            };
            let Some(mut state) = c.states.pop() else {
                // Cannot happen: there are never more helper tasks than states.
                c.queue.push_front(slot);
                c.running -= 1;
                return;
            };
            c.slots[slot] = Slot::Running;
            drop(c);
            let end = self.run(slot, &mut state);
            c = lock(&self.ctl);
            c.states.push(state);
            c.slots[slot] = end;
            self.finished.notify_all();
        }
    }
}

/// A bounded window of independent jobs whose results are taken back in the order given (module documentation).
pub struct InOrder<T, S> {
    shared: Arc<Shared<T, S>>,
    pool: Pool,
    helpers: usize,
    /// The caller's worker state.
    own: S,
    /// The oldest slot not taken back.
    head: usize,
    /// Slots given and not taken back.
    len: usize,
}

impl<T: Send + 'static, S: Send + 'static> InOrder<T, S> {
    /// A window of `slots` slots (at least 1) whose jobs run `work` on up to `helpers` helper threads of `pool` at once.
    /// `make_state` makes `helpers + 1` worker states (one is the caller's) and `make_payload` one payload per slot.
    pub fn new(
        pool: &Pool,
        helpers: usize,
        slots: usize,
        mut make_state: impl FnMut() -> S,
        mut make_payload: impl FnMut() -> T,
        work: impl Fn(&mut S, &mut T) + Send + Sync + 'static,
    ) -> InOrder<T, S> {
        let slots = slots.max(1);
        let shared = Shared {
            payloads: (0..slots).map(|_| Mutex::new(make_payload())).collect(),
            ctl: Mutex::new(Ctl {
                slots: vec![Slot::Free; slots].into_boxed_slice(),
                queue: VecDeque::with_capacity(slots),
                states: (0..helpers).map(|_| make_state()).collect(),
                running: 0,
            }),
            finished: Condvar::new(),
            work: Box::new(work),
        };
        InOrder {
            shared: Arc::new(shared),
            pool: pool.clone(),
            helpers,
            own: make_state(),
            head: 0,
            len: 0,
        }
    }

    /// The helper threads that may work at once.
    pub fn helpers(&self) -> usize {
        self.helpers
    }

    /// The slots.
    pub fn slots(&self) -> usize {
        self.shared.payloads.len()
    }

    /// Jobs given and not taken back.
    pub fn len(&self) -> usize {
        self.len
    }

    /// No job is given and not taken back.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every slot is in use: [`give`](Self::give) refuses a job.
    pub fn is_full(&self) -> bool {
        self.len == self.slots()
    }

    /// Gives a job: `fill` writes it into a free payload. False, and `fill` is not called, when every slot is in use. With no
    /// helpers the job runs here before the call returns.
    pub fn give(&mut self, fill: impl FnOnce(&mut T)) -> bool {
        if self.is_full() {
            return false;
        }
        let slot = (self.head + self.len) % self.slots();
        fill(&mut lock(&self.shared.payloads[slot]));
        self.len += 1;
        if self.helpers == 0 {
            let end = self.shared.run(slot, &mut self.own);
            lock(&self.shared.ctl).slots[slot] = end;
            return true;
        }
        let mut c = lock(&self.shared.ctl);
        c.slots[slot] = Slot::Queued;
        c.queue.push_back(slot);
        if c.running < self.helpers {
            c.running += 1;
            drop(c);
            let shared = Arc::clone(&self.shared);
            self.pool.run(Box::new(move || shared.drain()));
        }
        true
    }

    /// Takes back the oldest job when it has finished: `each` reads its payload, with `Err` when the job panicked. False when
    /// no job is given or the oldest still runs.
    pub fn take_one(&mut self, each: impl FnOnce(&mut T, Result<(), JobPanicked>)) -> bool {
        if self.len == 0 {
            return false;
        }
        let end = lock(&self.shared.ctl).slots[self.head];
        if !matches!(end, Slot::Done | Slot::Failed) {
            return false;
        }
        self.finish_head(end, each);
        true
    }

    /// Takes back every finished job up to the first one that still runs, oldest first.
    pub fn take(&mut self, mut each: impl FnMut(&mut T, Result<(), JobPanicked>)) {
        while self.take_one(&mut each) {}
    }

    /// Waits until the oldest job has finished and takes it back. The calling thread works on queued jobs, oldest first,
    /// while it waits. False when no job is given.
    pub fn wait_one(&mut self, each: impl FnOnce(&mut T, Result<(), JobPanicked>)) -> bool {
        if self.len == 0 {
            return false;
        }
        let mut c = lock(&self.shared.ctl);
        loop {
            let end = c.slots[self.head];
            if matches!(end, Slot::Done | Slot::Failed) {
                drop(c);
                self.finish_head(end, each);
                return true;
            }
            if let Some(slot) = c.queue.pop_front() {
                c.slots[slot] = Slot::Running;
                drop(c);
                let end = self.shared.run(slot, &mut self.own);
                c = lock(&self.shared.ctl);
                c.slots[slot] = end;
            } else {
                c = self
                    .shared
                    .finished
                    .wait(c)
                    .unwrap_or_else(|p| p.into_inner());
            }
        }
    }

    /// Waits for every job and takes them all back, oldest first.
    pub fn take_all(&mut self, mut each: impl FnMut(&mut T, Result<(), JobPanicked>)) {
        while self.wait_one(&mut each) {}
    }

    fn finish_head(&mut self, end: Slot, each: impl FnOnce(&mut T, Result<(), JobPanicked>)) {
        let slot = self.head;
        let r = if end == Slot::Done { Ok(()) } else { Err(JobPanicked) };
        each(&mut lock(&self.shared.payloads[slot]), r);
        lock(&self.shared.ctl).slots[slot] = Slot::Free;
        self.head = (self.head + 1) % self.slots();
        self.len -= 1;
    }
}

impl<T, S> Drop for InOrder<T, S> {
    /// Jobs that have not started are dropped; running ones finish on their helper.
    fn drop(&mut self) {
        lock(&self.shared.ctl).queue.clear();
    }
}

impl<T, S> std::fmt::Debug for InOrder<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InOrder")
            .field("helpers", &self.helpers)
            .field("slots", &self.shared.payloads.len())
            .field("len", &self.len)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Work of uneven length, so that jobs finish out of order.
    fn work(_: &mut u64, job: &mut (u64, u64)) {
        let mut h = job.0;
        for _ in 0..(2000 * (13 - job.0 % 13)) {
            h = h.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        }
        job.1 = h;
    }

    fn run(helpers: usize, slots: usize) -> Vec<(u64, u64)> {
        let pool = Pool::new(4);
        let mut q = InOrder::new(&pool, helpers, slots, || 0u64, || (0u64, 0u64), work);
        let mut got = Vec::new();
        for i in 0..300u64 {
            while !q.give(|j| *j = (i, 0)) {
                q.wait_one(|j, r| {
                    r.unwrap();
                    got.push(*j);
                });
            }
            q.take(|j, r| {
                r.unwrap();
                got.push(*j);
            });
        }
        q.take_all(|j, r| {
            r.unwrap();
            got.push(*j);
        });
        assert!(q.is_empty());
        got
    }

    #[test]
    fn results_come_back_in_the_order_given_on_any_number_of_helpers() {
        let inline = run(0, 3);
        assert_eq!(inline.len(), 300);
        assert!(inline.iter().enumerate().all(|(i, j)| j.0 == i as u64));
        for (helpers, slots) in [(1, 1), (1, 4), (2, 3), (4, 8), (4, 2)] {
            assert_eq!(run(helpers, slots), inline, "{helpers} helpers, {slots} slots");
        }
    }

    #[test]
    fn a_full_window_refuses_and_a_finished_job_waits_for_an_older_one() {
        let pool = Pool::new(2);
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let g = Arc::clone(&gate);
        // Job 0 holds its helper until the gate opens; the others run at once.
        let mut q = InOrder::new(&pool, 2, 3, || (), || 0u32, move |_: &mut (), j: &mut u32| {
            if *j == 0 {
                let mut open = lock(&g.0);
                while !*open {
                    open = g.1.wait(open).unwrap_or_else(|p| p.into_inner());
                }
            }
            *j += 100;
        });
        assert!(q.give(|j| *j = 0) && q.give(|j| *j = 1) && q.give(|j| *j = 2));
        assert!(q.is_full());
        assert!(!q.give(|_| unreachable!("a full window does not fill")));
        // Jobs 1 and 2 finish, and job 0 still holds the window: nothing is taken back.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let c = lock(&q.shared.ctl);
            if c.slots.iter().filter(|s| **s == Slot::Done).count() == 2 || std::time::Instant::now() > deadline {
                break;
            }
            drop(c);
            std::thread::yield_now();
        }
        let mut got = Vec::new();
        q.take(|j, _| got.push(*j));
        assert!(got.is_empty() && q.len() == 3);
        *lock(&gate.0) = true;
        gate.1.notify_all();
        q.take_all(|j, _| got.push(*j));
        assert_eq!(got, [100, 101, 102]);
        assert!(q.give(|j| *j = 7));
        q.take_all(|j, _| got.push(*j));
        assert_eq!(got.last(), Some(&107));
    }

    #[test]
    fn a_panicking_job_fails_its_slot_and_the_window_goes_on() {
        let pool = Pool::new(2);
        let quiet = Arc::new(AtomicBool::new(true));
        for helpers in [0, 2] {
            let q2 = Arc::clone(&quiet);
            let mut q = InOrder::new(&pool, helpers, 4, || (), || 0u32, move |_: &mut (), j: &mut u32| {
                assert!(*j != 2 || !q2.load(Ordering::Relaxed), "job two");
                *j *= 10;
            });
            let mut got = Vec::new();
            for i in 0..6 {
                while !q.give(|j| *j = i) {
                    q.wait_one(|j, r| got.push(r.map(|()| *j)));
                }
            }
            q.take_all(|j, r| got.push(r.map(|()| *j)));
            assert_eq!(got, [Ok(0), Ok(10), Err(JobPanicked), Ok(30), Ok(40), Ok(50)]);
        }
    }
}
