//! Automatic settings of a keyboard stack by trial decoding. The stack keeps its newest samples and says how to judge them; this
//! part decides when a trial runs, runs it on a helper thread when the input is live (in line otherwise, so that a recording
//! gives the same answer every time), drops the answer of a trial that a reset overtook, and keeps the lock state.
//!
//! Times are counts of input samples, never wall time.

use sdrkit_core::Complex32;
use sdrkit_graph::{Pool, RunContext, Ticket};

/// Samples kept before a trial runs, seconds.
pub const MIN_KEPT_S: f64 = 6.0;
/// Time between two trials, seconds.
pub const EVERY_S: f64 = 4.0;
/// The stretch an automatic trial decodes, seconds.
pub const STRETCH_S: f64 = 8.0;
/// The stretch a trial asked for by command decodes, at most, seconds.
pub const ASKED_S: f64 = 20.0;

/// Where the automatic settings are, as the `auto_state` result reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoState {
    /// Off and not asked.
    NotAsked = 0,
    /// Waiting for samples, or a trial is running.
    Waiting = 1,
    /// An answer is applied.
    Applied = 2,
    /// The last trial found nothing.
    NothingFound = 3,
}

enum Job<A> {
    Done(A),
    Running(Ticket<A>),
}

/// A trial to run: the stretch in seconds, and whether a command asked for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Due {
    /// Seconds of the newest samples to decode.
    pub seconds: f64,
    /// A command asked for it: its answer is applied whether Auto is on or not.
    pub asked: bool,
}

/// The timing and lock state of trial decoding, for answers of type `A`.
pub struct Trials<A: Send + 'static> {
    pool: Option<Pool>,
    live: bool,
    rate: f64,
    n: u64,
    last: Option<u64>,
    job: Option<(Job<A>, bool)>,
    asked: bool,
    lock_at: Option<u64>,
    state: AutoState,
}

impl<A: Send + 'static> Default for Trials<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Send + 'static> Trials<A> {
    /// Nothing kept, not locked, trials in line until [`Trials::attach`] says the input is live.
    pub fn new() -> Self {
        Trials {
            pool: None,
            live: false,
            rate: 8000.0,
            n: 0,
            last: None,
            job: None,
            asked: false,
            lock_at: None,
            state: AutoState::NotAsked,
        }
    }

    /// Takes the helper threads and whether the input is live from the run context.
    pub fn attach(&mut self, ctx: &RunContext) {
        self.pool = Some(ctx.pool.clone());
        self.live = ctx.live;
    }

    /// The input rate is `rate`: forgets everything but a pending request.
    pub fn configure(&mut self, rate: f64) {
        self.rate = rate;
        self.reset();
    }

    /// Forgets the samples counted, the lock and a running trial (its answer is dropped). A request made by command stays.
    pub fn reset(&mut self) {
        self.n = 0;
        self.last = None;
        self.job = None;
        self.lock_at = None;
        if self.state != AutoState::NotAsked {
            self.state = AutoState::Waiting;
        }
    }

    /// `n` more samples are kept.
    pub fn advance(&mut self, n: usize) {
        self.n += n as u64;
    }

    /// Seconds of samples since the last reset.
    pub fn kept_s(&self) -> f64 {
        self.n as f64 / self.rate
    }

    /// Asks for one trial on up to [`ASKED_S`] of the kept samples, once [`MIN_KEPT_S`] are kept.
    pub fn ask(&mut self) {
        self.asked = true;
        self.state = AutoState::Waiting;
    }

    /// The trial to start now, if one is due. With `on` (Auto is on) and not locked: every [`EVERY_S`] once [`MIN_KEPT_S`] are
    /// kept. Locked: only when `rerun_s` is given and that much time has passed since the last trial.
    pub fn due(&self, on: bool, rerun_s: Option<f64>) -> Option<Due> {
        if self.job.is_some() || self.kept_s() < MIN_KEPT_S {
            return None;
        }
        if self.asked {
            return Some(Due {
                seconds: ASKED_S,
                asked: true,
            });
        }
        if !on {
            return None;
        }
        let since = self
            .last
            .map_or(f64::INFINITY, |l| (self.n - l) as f64 / self.rate);
        let wait = if self.locked() {
            rerun_s?.max(EVERY_S)
        } else {
            EVERY_S
        };
        (since >= wait).then_some(Due {
            seconds: STRETCH_S,
            asked: false,
        })
    }

    /// Starts trial `due` on `stretch`, the newest samples: `job` runs on a helper thread when the input is live, else at once.
    pub fn start(
        &mut self,
        due: Due,
        stretch: Vec<Complex32>,
        job: impl FnOnce(Vec<Complex32>) -> A + Send + 'static,
    ) {
        self.last = Some(self.n);
        if due.asked {
            self.asked = false;
        }
        if self.state == AutoState::NotAsked {
            self.state = AutoState::Waiting;
        }
        let run = match &self.pool {
            Some(pool) if self.live => Job::Running(pool.spawn(move || job(stretch))),
            _ => Job::Done(job(stretch)),
        };
        self.job = Some((run, due.asked));
    }

    /// The answer of the trial that ended, with whether a command asked for it. A trial that panicked counts as nothing found.
    pub fn poll(&mut self) -> Option<(A, bool)> {
        let (job, asked) = self.job.take()?;
        match job {
            Job::Done(a) => Some((a, asked)),
            Job::Running(mut t) => match t.try_take() {
                Some(Ok(a)) => Some((a, asked)),
                Some(Err(_)) => {
                    self.state = AutoState::NothingFound;
                    None
                }
                None => {
                    self.job = Some((Job::Running(t), asked));
                    None
                }
            },
        }
    }

    /// A trial is running.
    pub fn running(&self) -> bool {
        self.job.is_some()
    }

    /// An answer is applied: locked from now on.
    pub fn lock(&mut self) {
        self.lock_at = Some(self.n);
        self.state = AutoState::Applied;
    }

    /// The last trial found nothing; a lock stays.
    pub fn nothing(&mut self) {
        if !self.locked() {
            self.state = AutoState::NothingFound;
        }
    }

    /// The lock is lost: trials run again.
    pub fn unlock(&mut self) {
        self.lock_at = None;
        self.state = AutoState::Waiting;
    }

    /// Auto is switched off: no lock, no state.
    pub fn off(&mut self) {
        self.lock_at = None;
        if !self.asked && self.job.is_none() {
            self.state = AutoState::NotAsked;
        }
    }

    /// Auto is switched on.
    pub fn on(&mut self) {
        if self.state == AutoState::NotAsked {
            self.state = AutoState::Waiting;
        }
    }

    /// Whether an answer is applied and holds.
    pub fn locked(&self) -> bool {
        self.lock_at.is_some()
    }

    /// Seconds since the lock, if locked.
    pub fn since_lock_s(&self) -> Option<f64> {
        self.lock_at.map(|l| (self.n - l) as f64 / self.rate)
    }

    /// The state for the results.
    pub fn state(&self) -> AutoState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trials_wait_for_samples_and_keep_their_spacing() {
        let mut t: Trials<u32> = Trials::new();
        t.configure(1000.0);
        t.advance(5999);
        assert_eq!(t.due(true, None), None);
        t.advance(1);
        let d = t.due(true, None).unwrap();
        assert_eq!(d.seconds, STRETCH_S);
        t.start(d, Vec::new(), |_| 7);
        assert_eq!(t.poll(), Some((7, false)));
        t.advance(3999);
        assert_eq!(t.due(true, None), None);
        t.advance(1);
        assert!(t.due(true, None).is_some());
        t.lock();
        t.advance(100_000);
        assert_eq!(t.due(true, None), None);
        assert!(t.due(true, Some(10.0)).is_some());
        assert_eq!(t.due(false, None), None);
        t.ask();
        assert_eq!(t.due(false, None).unwrap().seconds, ASKED_S);
    }

    #[test]
    fn a_reset_drops_the_answer() {
        let mut t: Trials<u32> = Trials::new();
        t.configure(1000.0);
        t.advance(6000);
        let d = t.due(true, None).unwrap();
        t.start(d, Vec::new(), |_| 1);
        t.reset();
        assert_eq!(t.poll(), None);
        assert_eq!(t.state(), AutoState::Waiting);
    }
}
