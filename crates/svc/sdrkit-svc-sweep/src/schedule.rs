//! Repeating sweeps: a sweep starts a fixed time after the start of the one before, or at once if that one ran longer.

/// When sweeps repeat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Schedule {
    /// Seconds from one sweep's start to the next (0: back to back).
    pub every_s: f64,
    /// How many sweeps in all (`None`: until stopped).
    pub count: Option<u32>,
}

impl Schedule {
    /// One sweep only.
    pub fn once() -> Self {
        Schedule {
            every_s: 0.0,
            count: Some(1),
        }
    }
    #[cfg(test)]
    /// Back to back until stopped.
    pub fn continuous() -> Self {
        Schedule {
            every_s: 0.0,
            count: None,
        }
    }
}

/// What to do after a sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// Start the next sweep now.
    Now,
    /// Sleep until this time, s.
    At(f64),
    /// No more sweeps.
    Finished,
}

/// Keeps track of a schedule.
#[derive(Clone, Debug)]
pub struct Scheduler {
    schedule: Schedule,
    done: u32,
    last_start_s: f64,
}

impl Scheduler {
    /// A scheduler.
    pub fn new(schedule: Schedule) -> Self {
        Scheduler {
            schedule,
            done: 0,
            last_start_s: 0.0,
        }
    }
    /// A sweep starts.
    pub fn started(&mut self, now_s: f64) {
        self.last_start_s = now_s;
    }
    /// A sweep ended at `now_s`: what next.
    pub fn finished(&mut self, now_s: f64) -> Next {
        self.done += 1;
        if let Some(c) = self.schedule.count {
            if self.done >= c {
                return Next::Finished;
            }
        }
        let at = self.last_start_s + self.schedule.every_s;
        if at <= now_s {
            Next::Now
        } else {
            Next::At(at)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules() {
        let mut s = Scheduler::new(Schedule::once());
        s.started(0.0);
        assert_eq!(s.finished(1.0), Next::Finished);
        let mut s = Scheduler::new(Schedule {
            every_s: 10.0,
            count: Some(3),
        });
        s.started(0.0);
        assert_eq!(s.finished(2.0), Next::At(10.0));
        s.started(10.0);
        assert_eq!(s.finished(25.0), Next::Now);
        s.started(25.0);
        assert_eq!(s.finished(26.0), Next::Finished);
        let mut s = Scheduler::new(Schedule::continuous());
        s.started(0.0);
        assert_eq!(s.finished(1.0), Next::Now);
    }
}
