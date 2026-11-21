//! The work of the control thread for the services that tune a session or read its stream: the sweep and the survey of
//! the scan service read the spectrum frames of the radio's shared derivation.

use crate::feed::{lock, Demand};
use crate::{Input, OwnerRef, SessionId, Shared, TUNED};
use sdrkit_core::{owner, Choice, Owner, Value};
use sdrkit_driver_core::CENTRE;
use sdrkit_graph::components::{SpectrumScale, SpectrumSize, SpectrumWindow};
use sdrkit_graph::{FrameRule, SizeRule, SpectrumSpec};
use sdrkit_svc_sweep::{RadioInfo, SpectrumFrame, SpectrumNeed, Step};
use std::sync::{Arc, Mutex};

/// The queued spectrum demand of a service on the feed of the session it works on.
pub(crate) struct ServiceDemand {
    session: SessionId,
    need: SpectrumNeed,
    demand: Arc<Demand>,
}

/// The spectrum that `need` requests, as the graph's spectrum component makes it: Blackman-Harris, frames back to back,
/// dB with a tone of magnitude 1 at 0 dB. `None` if the graph offers no size of that many bins.
fn spec_of(need: &SpectrumNeed) -> Option<SpectrumSpec> {
    let size = (0..)
        .map_while(SpectrumSize::from_index)
        .find(|s| s.bins() == need.bins)?;
    Some(SpectrumSpec {
        size: SizeRule::Bins(size),
        window: SpectrumWindow::BlackmanHarris,
        average: need.average.max(1),
        frames: FrameRule::BackToBack,
        scale: SpectrumScale::Db,
    })
}

impl Shared {
    /// Steps the sweep that runs at wall time `now`: tunes the radio of its session, passes it the frames of the shared
    /// derivation, and tunes back when it is done.
    pub(crate) fn step_sweep(&self, now: f64) {
        let target = self
            .services
            .sweep
            .lock()
            .session()
            .and_then(crate::parse_session_key);
        let Some(id) = target.filter(|id| self.session(*id).is_some()) else {
            self.drop_demand(&self.sweep_demand);
            return;
        };
        let Some(info) = self.radio_info(id) else {
            return;
        };
        let step = self.services.sweep.lock().step(now, &info);
        match step {
            Ok(s) => self.apply_sweep(id, s),
            Err(e) => self.services.sweep.lock().problem(&e.to_string()),
        }
        let need = self.services.sweep.lock().spectrum();
        if let Err(why) = self.follow_need(&self.sweep_demand, id, need) {
            self.services.sweep.lock().problem(&why);
        }
        let rate = info.rate_hz;
        self.take_frames(&self.sweep_demand, |centre, bins| {
            let frame = SpectrumFrame {
                time_s: now,
                centre_hz: centre,
                span_hz: rate,
                bins_db: bins,
            };
            let step = self.services.sweep.lock().feed(&frame);
            match step {
                Ok(s) => self.apply_sweep(id, s),
                Err(e) => self.services.sweep.lock().problem(&e.to_string()),
            }
        });
    }

    /// Passes every frame that waits in the queue of the demand in `slot` to `f`, oldest first, each with the centre
    /// of the samples it was made from.
    pub(crate) fn take_frames(
        &self,
        slot: &Mutex<Option<ServiceDemand>>,
        mut f: impl FnMut(f64, &[f32]),
    ) {
        let Some(demand) = lock(slot).as_ref().map(|d| Arc::clone(&d.demand)) else {
            return;
        };
        let Some(queue) = demand.queue.as_ref() else {
            return;
        };
        loop {
            let Some((centre, bins)) = lock(queue).full.pop_front() else {
                break;
            };
            f(centre, &bins);
            lock(queue).free.push(bins);
        }
    }

    /// The sample rate of the radio that session `id` reads. `None` for a session that the host feeds.
    pub(crate) fn radio_rate(&self, id: SessionId) -> Option<f64> {
        let Input::Radio(r) = self.session(id)?.input else {
            return None;
        };
        Some(self.radio(r)?.feed.tuning().0)
    }

    /// The radio information of session `id` for the sweep: the tuning range that its driver returns, its rate and where the
    /// session listens. `None` for a session that the host feeds.
    fn radio_info(&self, id: SessionId) -> Option<RadioInfo> {
        let s = self.session(id)?;
        let Input::Radio(r) = s.input else {
            return None;
        };
        let radio = self.radio(r)?;
        let (rate_hz, centre_hz) = radio.feed.tuning();
        let (min_hz, max_hz) = {
            let (lo, hi) = radio.owner.lock().freq_range();
            if hi > lo {
                (lo, hi)
            } else {
                (centre_hz, centre_hz)
            }
        };
        let (tuned, offset) = {
            let c = s.owner();
            let offset = crate::session::follow_offset(s.input, c.operator.as_ref());
            (c.values().map_or(0.0, |v| v.get(&TUNED)), offset)
        };
        Some(RadioInfo {
            min_hz,
            max_hz,
            rate_hz,
            can_sweep: false,
            tuned_hz: sdrkit_core::listened_hz(tuned, centre_hz, offset),
        })
    }

    /// Carries out step `s` of the sweep on session `id`.
    fn apply_sweep(&self, id: SessionId, s: Option<Step>) {
        let tune = |hz: f64| {
            let Some(Input::Radio(r)) = self.session(id).map(|s| s.input) else {
                return;
            };
            let done = self.owner(OwnerRef::Radio(r), |o| {
                let c = o.schema().id(CENTRE.field.key)?;
                owner::apply(o, c, &Value::Hz(hz))
            });
            if let Ok(Err(e)) = done {
                self.log(format!("{}: {e}", crate::radio_key(r)));
            }
        };
        match s {
            Some(Step::Tune { centre_hz, .. }) => tune(centre_hz),
            Some(Step::Done { return_hz }) => {
                tune(return_hz);
                self.drop_demand(&self.sweep_demand);
            }
            Some(Step::RadioSweep { .. }) => {
                let next = self.services.sweep.lock().radio_sweep_failed();
                self.apply_sweep(id, next);
            }
            Some(Step::Wait { .. }) | None => {}
        }
    }

    /// Makes the demand in `slot` the queued back-to-back spectrum `need` on the feed of session `id`, or removes it
    /// for no need. A need that is held already changes nothing.
    ///
    /// # Errors
    ///
    /// Why the spectrum component refuses the need. The need is held, so it is refused once.
    pub(crate) fn follow_need(
        &self,
        slot: &Mutex<Option<ServiceDemand>>,
        id: SessionId,
        need: Option<SpectrumNeed>,
    ) -> Result<(), String> {
        let mut held = lock(slot);
        let same = match (&*held, need) {
            (Some(d), Some(n)) => d.session == id && d.need == n,
            (None, None) => true,
            _ => false,
        };
        if same {
            return Ok(());
        }
        if let Some(d) = held.take() {
            if let Some(s) = self.session(d.session) {
                s.link.feed.remove_demand(&d.demand);
            }
        }
        let Some(n) = need else {
            return Ok(());
        };
        let Some(s) = self.session(id) else {
            return Ok(());
        };
        let spec = spec_of(&n);
        let demand = Arc::new(Demand {
            spec: Mutex::new(spec),
            ..Demand::queued_empty()
        });
        let out = match spec.map(|x| x.values(n.rate_hz)) {
            Some(Ok(_)) => {
                s.link.feed.add_demand(Arc::clone(&demand));
                Ok(())
            }
            Some(Err(e)) => Err(format!("The spectrum of {} bins was refused: {e}", n.bins)),
            None => Err(format!("No spectrum has {} bins", n.bins)),
        };
        *held = Some(ServiceDemand {
            session: id,
            need: n,
            demand,
        });
        out
    }

    /// Removes the demand in `slot` from the feed of its session.
    pub(crate) fn drop_demand(&self, slot: &Mutex<Option<ServiceDemand>>) {
        if let Some(d) = lock(slot).take() {
            if let Some(s) = self.session(d.session) {
                s.link.feed.remove_demand(&d.demand);
            }
        }
    }

    /// Writes `line` into the engine's log.
    pub(crate) fn log(&self, line: String) {
        self.engine.lock().results.results_mut().log(line);
    }
}
