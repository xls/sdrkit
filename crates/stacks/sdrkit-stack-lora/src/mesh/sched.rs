//! The transmit scheduler: requests wait until no detector on their setting is inside a frame (or, for AX.25 on FM, until
//! no carrier is heard), then a random back-off of 0 to 2^cw slots (a LoRa slot is 2.5 symbol times plus 0.2 ms, an FM
//! slot 10 ms), and the duty-cycle limit of the band. Nothing goes out when the radio cannot transmit.

use super::duty::DutyCycle;
use super::types::{Failure, TxRequest};
use sdrkit_algo_sim::rng::Rng;
use std::collections::VecDeque;

/// What the scheduler reports.
#[derive(Clone, Debug, PartialEq)]
pub enum SchedEvent {
    /// A request went on the air at `at` for `air_s`.
    Sent {
        /// The request.
        id: u64,
        /// Its own message.
        message_key: Option<u64>,
        /// Signal time.
        at: f64,
        /// Air time.
        air_s: f64,
    },
    /// The duty-cycle limit holds a request.
    WaitingForDutyCycle {
        /// Its own message.
        message_key: Option<u64>,
    },
    /// A request cannot be sent.
    Failed {
        /// Its own message.
        message_key: Option<u64>,
        /// Why.
        reason: Failure,
    },
}

#[derive(Clone, Debug)]
struct Pending {
    req: TxRequest,
    ready_at: Option<f64>,
    told_duty: bool,
}

/// The scheduler of one engine.
#[derive(Clone, Debug)]
pub struct TxScheduler {
    queue: VecDeque<Pending>,
    /// The duty-cycle tracker (its `on` is the setting `duty_limit`).
    pub duty: DutyCycle,
    rng: Rng,
    /// The transmitter is on. While it is off, each request fails.
    pub on: bool,
    /// Requests sent.
    pub sent: u64,
    /// The end of the current own transmission.
    pub busy_until: f64,
}

/// At most this many requests wait.
pub const QUEUE_CAP: usize = 64;

impl TxScheduler {
    /// A scheduler. `seed` makes the back-offs repeatable.
    pub fn new(seed: u64) -> Self {
        TxScheduler {
            queue: VecDeque::new(),
            duty: DutyCycle::new(),
            rng: Rng::new(seed),
            on: true,
            sent: 0,
            busy_until: 0.0,
        }
    }
    /// Requests waiting.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
    /// Queues a request.
    pub fn push(&mut self, req: TxRequest) {
        if self.queue.len() >= QUEUE_CAP {
            self.queue.pop_front();
        }
        self.queue.push_back(Pending {
            req,
            ready_at: None,
            told_duty: false,
        });
    }
    /// Drops a queued request (a cancelled rebroadcast).
    pub fn cancel(&mut self, id: u64) {
        self.queue.retain(|p| p.req.id != id);
    }
    /// Fails each queued request whose frequency `listens` refuses. A request that waits while the session leaves its
    /// channel is not sent.
    pub fn keep(&mut self, listens: impl Fn(f64) -> bool, ev: &mut Vec<SchedEvent>) {
        self.queue.retain(|p| {
            let keep = listens(p.req.freq_hz);
            if !keep {
                ev.push(SchedEvent::Failed {
                    message_key: p.req.message_key,
                    reason: Failure::NoChannel,
                });
            }
            keep
        });
    }
    /// The requests due at `now` with the channel `busy` or not. Returns what goes on the air and what happened.
    pub fn poll(
        &mut self,
        now: f64,
        busy: bool,
        out: &mut Vec<TxRequest>,
        ev: &mut Vec<SchedEvent>,
    ) {
        if !self.on {
            for p in self.queue.drain(..) {
                ev.push(SchedEvent::Failed {
                    message_key: p.req.message_key,
                    reason: Failure::TransmitOff,
                });
            }
            return;
        }
        if now < self.busy_until {
            return;
        }
        let Some(p) = self.queue.front_mut() else {
            return;
        };
        if p.req.not_before > now {
            return;
        }
        let air = p.req.air_s();
        let earliest = self.duty.earliest(now, p.req.freq_hz, air);
        if earliest > now {
            if !p.told_duty {
                p.told_duty = true;
                ev.push(SchedEvent::WaitingForDutyCycle {
                    message_key: p.req.message_key,
                });
            }
            return;
        }
        if busy {
            p.ready_at = None;
            return;
        }
        let slot = p
            .req
            .lora
            .as_ref()
            .map_or(0.01, |l| 2.5 * l.symbol_s() + 0.2e-3);
        let ready = *p.ready_at.get_or_insert_with(|| {
            let w = 1u64 << p.req.cw.min(7);
            now + self.rng.below(w + 1) as f64 * slot
        });
        if ready > now {
            return;
        }
        let p = self.queue.pop_front().expect("a front");
        self.duty.record(now, air, p.req.freq_hz);
        self.busy_until = now + air;
        self.sent += 1;
        ev.push(SchedEvent::Sent {
            id: p.req.id,
            message_key: p.req.message_key,
            at: now,
            air_s: air,
        });
        out.push(p.req);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::types::TxKind;
    use sdrkit_proto_lora_phy::LoraParams;

    fn req(id: u64, freq: f64) -> TxRequest {
        TxRequest {
            id,
            payload: vec![0; 200],
            lora: Some(LoraParams::new(11, 250e3, 5, 16, 0x2B)),
            afsk: None,
            freq_hz: freq,
            not_before: 0.0,
            cw: 3,
            what: TxKind::Text,
            message_key: Some(id),
        }
    }

    #[test]
    fn waits_for_the_channel_then_backs_off() {
        let mut s = TxScheduler::new(1);
        s.push(req(1, 869.525e6));
        let (mut out, mut ev) = (Vec::new(), Vec::new());
        s.poll(0.0, true, &mut out, &mut ev);
        assert!(out.is_empty());
        let mut t = 0.0;
        while out.is_empty() && t < 2.0 {
            s.poll(t, false, &mut out, &mut ev);
            t += 0.001;
        }
        assert_eq!(out.len(), 1);
        assert!(matches!(ev[0], SchedEvent::Sent { id: 1, .. }));
    }

    #[test]
    fn the_duty_cycle_holds_requests() {
        let mut s = TxScheduler::new(1);
        s.duty.bands = super::super::duty::parse_bands("100000000-100600000=1").unwrap();
        let (mut out, mut ev) = (Vec::new(), Vec::new());
        // 1 % at 100.1 MHz: 36 s an hour.
        let mut t = 0.0;
        for i in 0..200 {
            s.push(req(i, 100.1e6));
        }
        while t < 400.0 {
            s.poll(t, false, &mut out, &mut ev);
            t += 0.01;
        }
        let air: f64 = out
            .iter()
            .map(|r| r.lora.as_ref().unwrap().airtime(200))
            .sum();
        assert!(air <= 36.0, "{air}");
        assert!(ev
            .iter()
            .any(|e| matches!(e, SchedEvent::WaitingForDutyCycle { .. })));
    }

    #[test]
    fn a_waiting_request_fails_when_the_session_leaves_its_channel() {
        let mut s = TxScheduler::new(1);
        s.push(req(1, 869.5e6));
        s.push(req(2, 919.875e6));
        let mut ev = Vec::new();
        s.keep(|hz| hz == 919.875e6, &mut ev);
        assert_eq!(s.pending(), 1);
        assert_eq!(
            ev,
            [SchedEvent::Failed {
                message_key: Some(1),
                reason: Failure::NoChannel
            }]
        );
    }

    #[test]
    fn requests_fail_while_the_transmitter_is_off() {
        let mut s = TxScheduler::new(1);
        s.on = false;
        s.push(req(1, 869.5e6));
        let (mut out, mut ev) = (Vec::new(), Vec::new());
        s.poll(0.0, false, &mut out, &mut ev);
        assert!(out.is_empty());
        assert_eq!(
            ev[0],
            SchedEvent::Failed {
                message_key: Some(1),
                reason: Failure::TransmitOff
            }
        );
    }
}
