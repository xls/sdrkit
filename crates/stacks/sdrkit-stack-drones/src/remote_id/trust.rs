//! The trust check of Remote ID. The broadcasts of each drone go to the trust engine of the `remoteid` crate, which keeps the
//! evidence of one transmitter and grades it (authenticated, plausible, suspicious, spoofed) with the reasons behind the grade.
//! The verdict is advice: nothing is dropped.

use super::messages::{Link, Payload};
use remoteid::trust::ReasonSet;
use remoteid::wifi::BeaconInfo;
use remoteid::{Observation, Reason, TrustConfig, TrustState, Verdict};
use sdrkit_core::publish::keys::Urgency;

sdrkit_core::choice_enum! {
    /// The trust verdict of a drone.
    pub enum TrustVerdict {
        /// Signatures verify under a trusted key and nothing contradicts them.
        Authenticated = ("authenticated", "Authenticated"),
        /// Nothing contradicts the broadcasts.
        Plausible = ("plausible", "Plausible"),
        /// Physical or protocol faults that a real transmitter rarely shows.
        Suspicious = ("suspicious", "Suspicious"),
        /// Strong evidence that the broadcasts are fake.
        Spoofed = ("spoofed", "Spoofed"),
    }
}

impl TrustVerdict {
    fn of(v: Verdict) -> TrustVerdict {
        match v {
            Verdict::Authenticated => TrustVerdict::Authenticated,
            Verdict::Plausible => TrustVerdict::Plausible,
            Verdict::Suspicious => TrustVerdict::Suspicious,
            Verdict::Spoofed => TrustVerdict::Spoofed,
        }
    }
    /// True for suspicious and spoofed.
    pub fn flagged(self) -> bool {
        matches!(self, TrustVerdict::Suspicious | TrustVerdict::Spoofed)
    }
    /// The urgency of an event with this verdict: notice when flagged, else none.
    pub fn urgency(self) -> Urgency {
        if self.flagged() {
            Urgency::Notice
        } else {
            Urgency::None
        }
    }
    /// The lower-case name ("spoofed").
    pub fn name(self) -> &'static str {
        match self {
            TrustVerdict::Authenticated => "authenticated",
            TrustVerdict::Plausible => "plausible",
            TrustVerdict::Suspicious => "suspicious",
            TrustVerdict::Spoofed => "spoofed",
        }
    }
}

/// The trust engine's name of an air interface.
pub fn transport(l: Link) -> remoteid::Transport {
    match l {
        Link::Ble4 => remoteid::Transport::Bluetooth4,
        Link::Ble5 => remoteid::Transport::Bluetooth5,
        Link::WifiBeacon => remoteid::Transport::WifiBeacon,
    }
}

/// What the receiver knows of one received payload besides its bytes.
#[derive(Clone, Copy, Debug)]
pub struct Heard<'a> {
    /// The air interface.
    pub link: Link,
    /// The message counter.
    pub counter: u8,
    /// The message or pack.
    pub payload: &'a Payload,
    /// A monotonic clock, milliseconds.
    pub now_ms: u64,
    /// The UTC clock, Unix milliseconds, for live samples only.
    pub unix_ms: Option<u64>,
    /// The header fields of a Wi-Fi beacon.
    pub beacon: Option<BeaconInfo>,
}

/// The trust state of one drone: the evidence, the verdict and its reasons.
#[derive(Clone, Debug)]
pub struct Trust {
    state: Box<TrustState>,
    verdict: TrustVerdict,
    reasons: ReasonSet,
    text: String,
}

impl PartialEq for Trust {
    fn eq(&self, o: &Trust) -> bool {
        self.verdict == o.verdict && self.reasons == o.reasons
    }
}

impl Default for Trust {
    fn default() -> Self {
        Trust::new()
    }
}

impl Trust {
    /// No evidence yet: plausible, no reason.
    pub fn new() -> Trust {
        Trust {
            state: Box::new(TrustState::new()),
            verdict: TrustVerdict::Plausible,
            reasons: ReasonSet::default(),
            text: String::new(),
        }
    }

    /// Judges one payload with `cfg`. Returns true when the verdict changed. Allocates nothing unless the set of reasons
    /// changed and its text needs more room than before.
    pub fn observe(&mut self, cfg: &TrustConfig, h: &Heard<'_>) -> bool {
        let mut obs = Observation::new(
            transport(h.link),
            Some(h.counter),
            h.payload.as_slice(),
            h.now_ms,
        );
        if let Some(u) = h.unix_ms {
            obs = obs.with_unix_ms(u);
        }
        if let Some(b) = h.beacon {
            obs = obs.with_beacon(b);
        }
        let a = self.state.observe(cfg, &obs);
        self.set(a.verdict, a.reasons)
    }

    /// Adds evidence from outside this transmitter at `now_ms` and grades again. Returns true when the verdict changed.
    pub fn note(&mut self, cfg: &TrustConfig, r: Reason, now_ms: u64) -> bool {
        self.state.note(r, now_ms);
        let a = self.state.assessment(cfg, now_ms);
        self.set(a.verdict, a.reasons)
    }

    fn set(&mut self, v: Verdict, reasons: ReasonSet) -> bool {
        if reasons != self.reasons {
            self.reasons = reasons;
            self.text.clear();
            for (i, r) in reasons.iter().enumerate() {
                if i > 0 {
                    self.text.push_str(", ");
                }
                self.text.push_str(r.name());
            }
        }
        let v = TrustVerdict::of(v);
        let changed = v != self.verdict;
        self.verdict = v;
        changed
    }

    /// The verdict.
    pub fn verdict(&self) -> TrustVerdict {
        self.verdict
    }

    /// The active reasons: their stable names joined by ", ", empty when there is none.
    pub fn reasons(&self) -> &str {
        &self.text
    }

    /// Frames judged.
    pub fn frames(&self) -> u64 {
        self.state.frames()
    }
}
