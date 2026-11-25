//! The hop of a radio: one session has its radio visit a list of frequencies in turn, a dwell on each
//! ([`HostRequest::Hop`](sdrkit_core::HostRequest::Hop)).
//!
//! The thread of the radio counts the dwell in samples, so a step lasts its dwell of signal time at any load and pace. At
//! the end of a step it passes no more samples and wakes the control thread, which tunes the session to the next
//! frequency through its operator ([`Shared::tune_to`]). The samples that the radio wrote until the new centre was in force
//! are left out as one gap made on purpose: the stacks of the radio are told of it, and nobody counts it as lost.

use crate::feed::lock;
use crate::radio::Radio;
use crate::{EngineError, Input, OwnerRef, RadioId, SessionId, Shared};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Where the thread of a radio is in a hop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Phase {
    /// No hop: the samples pass.
    Off,
    /// A step runs: `left` more samples pass, then the step is over.
    Step {
        /// The samples of the step still to pass.
        left: u64,
    },
    /// The step is over, or the radio is about to move: no sample passes until the radio has moved.
    Held,
    /// The radio has moved: the samples before the place `at` of the ring are left out, then a step of `dwell_s`
    /// seconds starts, or the hop ends if there is no dwell.
    Moved {
        /// The place in the ring ([`sdrkit_driver_core::RingReader::position`]) of the first sample at the new
        /// frequency.
        at: u64,
        /// The dwell of the step, seconds of signal. `None` for the last move of a hop.
        dwell_s: Option<f64>,
    },
}

/// The phase of the hop of one radio, shared by the control thread and the thread of the radio.
#[derive(Debug)]
pub(crate) struct HopGate {
    phase: Mutex<Phase>,
    /// Signalled when the phase leaves [`Phase::Held`].
    moved: Condvar,
}

impl Default for HopGate {
    fn default() -> Self {
        HopGate {
            phase: Mutex::new(Phase::Off),
            moved: Condvar::new(),
        }
    }
}

impl HopGate {
    /// The phase now.
    pub(crate) fn phase(&self) -> Phase {
        *lock(&self.phase)
    }

    fn set(&self, p: Phase) {
        *lock(&self.phase) = p;
        self.moved.notify_all();
    }

    /// Stops the samples before the radio moves.
    pub(crate) fn hold(&self) {
        self.set(Phase::Held);
    }

    /// The radio moved: the samples before ring place `at` are left out, then a step of `dwell_s` starts, or the hop
    /// ends if there is no dwell.
    pub(crate) fn moved(&self, at: u64, dwell_s: Option<f64>) {
        self.set(Phase::Moved { at, dwell_s });
    }

    /// Ends the hop: the samples pass again.
    pub(crate) fn off(&self) {
        self.set(Phase::Off);
    }

    /// True if a step is over and the radio waits for its next move.
    pub(crate) fn held(&self) -> bool {
        self.phase() == Phase::Held
    }

    /// The thread of the radio reached ring place `at` of [`Phase::Moved`]: a step of `samples` starts, or the hop
    /// ends for `None`. Does nothing if the phase changed meanwhile.
    pub(crate) fn start_step(&self, at: u64, samples: Option<u64>) {
        let mut p = lock(&self.phase);
        if matches!(*p, Phase::Moved { at: a, .. } if a == at) {
            *p = match samples {
                None => Phase::Off,
                Some(0) => Phase::Held,
                Some(left) => Phase::Step { left },
            };
        }
    }

    /// `n` samples of the step passed. Returns true if that ended the step.
    pub(crate) fn passed(&self, n: u64) -> bool {
        let mut p = lock(&self.phase);
        let Phase::Step { left } = *p else {
            return false;
        };
        let left = left.saturating_sub(n);
        *p = if left == 0 {
            Phase::Held
        } else {
            Phase::Step { left }
        };
        left == 0
    }

    /// Waits up to `timeout` while the phase is [`Phase::Held`].
    pub(crate) fn wait_moved(&self, timeout: Duration) {
        let p = lock(&self.phase);
        let _ = self
            .moved
            .wait_timeout_while(p, timeout, |p| *p == Phase::Held);
    }

    /// The stream of the radio started again in a new ring: a place of the old ring counts from the start of the new one.
    pub(crate) fn restarted(&self) {
        let mut p = lock(&self.phase);
        if let Phase::Moved { dwell_s, .. } = *p {
            *p = Phase::Moved { at: 0, dwell_s };
        }
    }
}

/// The hop of one session.
#[derive(Clone, Debug)]
pub(crate) struct Hop {
    pub(crate) session: SessionId,
    radio: RadioId,
    channels: Vec<f64>,
    dwell_s: f64,
    stop_on_lock: bool,
    /// The index in `channels` of the step that runs.
    step: usize,
}

impl Hop {
    /// True if the request with `channels`, `dwell_s` and `stop_on_lock` asks for this hop.
    fn same(&self, channels: &[f64], dwell_s: f64, stop_on_lock: bool) -> bool {
        self.channels == channels && self.dwell_s == dwell_s && self.stop_on_lock == stop_on_lock
    }
}

impl Shared {
    /// Starts the hop of session `id` over `channels`, `dwell_s` seconds of signal on each, at the first channel. A hop
    /// that the session runs with the same channels, dwell and `stop_on_lock` goes on where it is; another one takes its
    /// place. With `stop_on_lock` the hop ends on a channel where the stack reports lock at the end of the step.
    ///
    /// # Errors
    ///
    /// [`EngineError::Unserved`] if the session reads no radio or a radio that does not tune, and
    /// [`EngineError::NotSteering`] if another session steers the radio: another session hops it, or another session
    /// that can steer reads it too.
    pub(crate) fn start_hop(
        &self,
        id: SessionId,
        channels: Vec<f64>,
        dwell_s: f64,
        stop_on_lock: bool,
    ) -> Result<(), EngineError> {
        let unserved = EngineError::Unserved {
            kind: crate::RequestKind::Hop,
        };
        let s = self
            .session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
        let Input::Radio(r) = s.input else {
            return Err(unserved);
        };
        if s.by == crate::session::By::Share || !self.radio_tunes(r)? {
            return Err(unserved);
        }
        let radio = self
            .radio(r)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(r)))?;
        let moves = lock(&self.hop_moves);
        let hop = {
            let mut hops = lock(&self.hops);
            if hops.iter().any(|h| h.radio == r && h.session != id) {
                return Err(EngineError::NotSteering);
            }
            let own = hops.iter().position(|h| h.session == id);
            if own.is_none() && radio.feed.readers() > 1 {
                return Err(EngineError::NotSteering);
            }
            if own.is_some_and(|i| hops[i].same(&channels, dwell_s, stop_on_lock)) {
                return Ok(());
            }
            hops.retain(|h| h.session != id);
            let hop = Hop {
                session: id,
                radio: r,
                channels,
                dwell_s,
                stop_on_lock,
                step: 0,
            };
            hops.push(hop.clone());
            hop
        };
        let moved = self.hop_to(&radio, &hop);
        drop(moves);
        if let Err(e) = moved {
            self.end_hop(id);
            return Err(e);
        }
        Ok(())
    }

    /// Ends the hop of session `id`, if it has one: the radio stays where it is and its samples pass again.
    pub(crate) fn end_hop(&self, id: SessionId) {
        let _moves = lock(&self.hop_moves);
        if let Some(radio) = self.take_hop(id) {
            radio.hop.off();
        }
    }

    /// Ends the hop of session `id`, if it has one, with the move of its radio that `tune` makes: the samples that the
    /// radio wrote before the move are left out, as at a step. Without a hop, `tune` runs alone.
    pub(crate) fn end_hop_by<E>(
        &self,
        id: SessionId,
        tune: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), E> {
        let _moves = lock(&self.hop_moves);
        let Some(radio) = self.take_hop(id) else {
            return tune();
        };
        radio.hop.hold();
        let r = tune();
        let at = radio.owner.lock().ring_written();
        radio.hop.moved(at, None);
        r
    }

    /// Removes the hop of session `id` and returns its radio.
    fn take_hop(&self, id: SessionId) -> Option<Arc<Radio>> {
        let h = {
            let mut hops = lock(&self.hops);
            let i = hops.iter().position(|h| h.session == id)?;
            hops.swap_remove(i)
        };
        self.radio(h.radio)
    }

    /// True if a session other than `id` hops radio `r`: only that session moves the radio.
    pub(crate) fn hopped_by_other(&self, r: RadioId, id: SessionId) -> bool {
        lock(&self.hops)
            .iter()
            .any(|h| h.radio == r && h.session != id)
    }

    /// Moves each hop whose step is over to its next channel. A hop ends when its session or radio is gone, when the
    /// session has stopped, when the stack reports lock on a hop with `stop_on_lock`, and when the move is refused.
    pub(crate) fn step_hops(&self) {
        let hops: Vec<Hop> = lock(&self.hops).clone();
        for mut h in hops {
            let (Some(s), Some(radio)) = (self.session(h.session), self.radio(h.radio)) else {
                self.end_hop(h.session);
                continue;
            };
            let (stopped, locked) = {
                let c = s.owner();
                let lock_state = sdrkit_core::Owner::results(&*c).status().lock_state;
                (
                    c.state == crate::SessionState::Stopped,
                    lock_state == sdrkit_core::LockState::Locked,
                )
            };
            if stopped {
                self.end_hop(h.session);
                continue;
            }
            if !radio.hop.held() {
                continue;
            }
            if h.stop_on_lock && locked {
                self.end_hop(h.session);
                continue;
            }
            h.step = (h.step + 1) % h.channels.len();
            let moved = {
                // A hop that another thread ends meanwhile is not moved.
                let _moves = lock(&self.hop_moves);
                let still = match lock(&self.hops).iter_mut().find(|x| x.session == h.session) {
                    Some(x) => {
                        x.step = h.step;
                        true
                    }
                    None => false,
                };
                if !still {
                    continue;
                }
                self.hop_to(&radio, &h)
            };
            if let Err(e) = moved {
                self.end_hop(h.session);
                self.log(format!("{}: {e}", crate::session_key(h.session)));
            }
        }
    }

    /// Tunes the session of `h` to the channel of its step while the samples of the radio wait, then lets them pass from
    /// the first sample that the radio wrote after the move. The caller holds `hop_moves`.
    fn hop_to(&self, radio: &Radio, h: &Hop) -> Result<(), EngineError> {
        radio.hop.hold();
        self.tune_to(h.session, h.channels[h.step])?;
        let at = radio.owner.lock().ring_written();
        radio.hop.moved(at, Some(h.dwell_s));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_passes_its_samples_then_holds_until_the_radio_moved() {
        let g = HopGate::default();
        assert_eq!(g.phase(), Phase::Off);
        assert!(!g.passed(10), "no step runs");
        g.hold();
        g.moved(500, Some(0.25));
        // A place of another move does not start the step.
        g.start_step(400, Some(100));
        assert_eq!(
            g.phase(),
            Phase::Moved {
                at: 500,
                dwell_s: Some(0.25)
            }
        );
        g.start_step(500, Some(100));
        assert!(!g.passed(60));
        assert_eq!(g.phase(), Phase::Step { left: 40 });
        assert!(g.passed(40));
        assert!(g.held());
        // The stream starts again in a new ring during a move: the place counts from its start.
        g.moved(9_000, Some(0.25));
        g.restarted();
        assert_eq!(
            g.phase(),
            Phase::Moved {
                at: 0,
                dwell_s: Some(0.25)
            }
        );
        // The last move ends the hop once its place is reached.
        g.moved(700, None);
        g.start_step(700, None);
        assert_eq!(g.phase(), Phase::Off);
    }
}
