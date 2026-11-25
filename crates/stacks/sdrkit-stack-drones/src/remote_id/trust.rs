//! The trust check of Remote ID. The broadcasts of each drone go to the trust engine of the `remoteid` crate, which keeps the
//! evidence of one transmitter and grades it (authenticated, plausible, suspicious, spoofed) with the reasons behind the grade.
//! The verdict is advice: nothing is dropped.

use super::messages::{Link, Payload};
use remoteid::trust::ReasonSet;
use remoteid::wifi::BeaconInfo;
use remoteid::{Observation, Reason, TrustConfig, TrustState, Verdict};
use sdrkit_core::publish::keys::Urgency;
use sdrkit_core::FlagSet;

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

sdrkit_core::flag_set! {
    /// The reasons behind a trust verdict. The bit of a reason is its code in the trust engine, and its key is the
    /// engine's name of it.
    pub struct TrustReasons {
        KNOWN_SPOOFER = (
            "known_spoofer",
            "Known spoofer",
            "The broadcasts have the fingerprint of a public spoofing tool."
        ),
        AUTH_FAILED = (
            "auth_failed",
            "Authentication failed",
            "A signature or a key binding of the drone does not verify."
        ),
        TELEPORT = (
            "teleport",
            "Teleport",
            "The position jumped further than the speed limit allows."
        ),
        COUNTER_CONFLICT = (
            "counter_conflict",
            "Counter conflict",
            "One counter value carried two contents: two radios share an address."
        ),
        IDENTITY_CLONE = (
            "identity_clone",
            "Identity clone",
            "Another transmitter sends the same ID from a position that does not fit."
        ),
        TRANSPORT_DISAGREEMENT = (
            "transport_disagreement",
            "Transport disagreement",
            "The same ID says different things on Bluetooth and Wi-Fi."
        ),
        SPEED_IMPLAUSIBLE = (
            "speed_implausible",
            "Speed implausible",
            "The track is faster than any drone, over several windows."
        ),
        REPORTED_SPEED_MISMATCH = (
            "reported_speed_mismatch",
            "Reported speed mismatch",
            "The reported horizontal speed does not fit the track."
        ),
        REPORTED_COURSE_MISMATCH = (
            "reported_course_mismatch",
            "Reported course mismatch",
            "The reported direction does not fit the track."
        ),
        VERTICAL_RATE_IMPLAUSIBLE = (
            "vertical_rate_implausible",
            "Vertical rate implausible",
            "The climb or descent rate is impossible or does not fit the reported vertical speed."
        ),
        TIMESTAMP_FROZEN = (
            "timestamp_frozen",
            "Timestamp frozen",
            "The location timestamps stopped while the clock of the receiver ran."
        ),
        LOCATION_CLOCK_DRIFT = (
            "location_clock_drift",
            "Location clock drift",
            "The location timestamps run at a wrong rate or far from UTC."
        ),
        SYSTEM_CLOCK_DRIFT = (
            "system_clock_drift",
            "System clock drift",
            "The system or authentication timestamps are zero or far from UTC."
        ),
        TSF_WALL_CLOCK = (
            "tsf_wall_clock",
            "TSF wall clock",
            "The Wi-Fi timer holds a wall clock time, as software that injects frames writes it."
        ),
        TSF_DISCONTINUITY = (
            "tsf_discontinuity",
            "TSF discontinuity",
            "The Wi-Fi timer jumps against the clock of the receiver."
        ),
        COUNTER_INTERLEAVE = (
            "counter_interleave",
            "Counter interleave",
            "The message counter starts again and again."
        ),
        SIGNAL_JUMP = (
            "signal_jump",
            "Signal jump",
            "The signal strength jumps from frame to frame."
        ),
        SHARED_RADIO = (
            "shared_radio",
            "Shared radio",
            "Several transmitter addresses share one radio clock."
        ),
        INVALID_FIELDS = (
            "invalid_fields",
            "Invalid fields",
            "Location fields are outside their ranges or hold reserved codes."
        ),
        SERIAL_FORMAT = (
            "serial_format",
            "Serial format",
            "The serial number does not follow ANSI/CTA-2063-A."
        ),
        NO_BASIC_ID = (
            "no_basic_id",
            "No Basic ID",
            "The drone sends locations for a while and never a Basic ID."
        ),
        AUTH_EXPIRED = (
            "auth_expired",
            "Authentication expired",
            "A valid signature is outside its time window: a possible replay."
        ),
    }
}

impl TrustReasons {
    /// The set of the engine's reasons `set`.
    fn of(set: ReasonSet) -> TrustReasons {
        let mut out = TrustReasons::empty();
        for r in set.iter() {
            out.insert(TrustReasons::from_bits(1 << (r as u32)));
        }
        out
    }
}

/// The keys of the reasons with ", " between them, for a status line.
impl std::fmt::Display for TrustReasons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, e) in self.entries().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(&e.key)?;
        }
        Ok(())
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
    reasons: TrustReasons,
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
            reasons: TrustReasons::empty(),
        }
    }

    /// Judges one payload with `cfg`. Returns true when the verdict changed. Allocates nothing.
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
        self.reasons = TrustReasons::of(reasons);
        let v = TrustVerdict::of(v);
        let changed = v != self.verdict;
        self.verdict = v;
        changed
    }

    /// The verdict.
    pub fn verdict(&self) -> TrustVerdict {
        self.verdict
    }

    /// The active reasons, empty when there is none.
    pub fn reasons(&self) -> TrustReasons {
        self.reasons
    }

    /// Frames judged.
    pub fn frames(&self) -> u64 {
        self.state.frames()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::FlagSet;

    #[test]
    fn the_flags_are_the_reasons_of_the_trust_engine_in_code_order() {
        assert_eq!(TrustReasons::FLAGS.len(), Reason::ALL.len());
        for (i, r) in Reason::ALL.iter().enumerate() {
            assert_eq!(*r as usize, i);
            assert_eq!(TrustReasons::FLAGS[i].key, r.name());
        }
        let mut set = ReasonSet::default();
        set.insert(Reason::Teleport);
        set.insert(Reason::AuthExpired);
        let flags = TrustReasons::of(set);
        assert_eq!(flags, TrustReasons::TELEPORT | TrustReasons::AUTH_EXPIRED);
        assert_eq!(flags.to_string(), "teleport, auth_expired");
    }
}
