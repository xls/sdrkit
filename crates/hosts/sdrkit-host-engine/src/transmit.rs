//! Transmit through a radio: the session thread sends the samples that a session's `tx.sink` writes to the transmitter of
//! the session's radio, one burst at a time.

use crate::feed::lock;
use crate::radio::Radio;
use crate::streams::Streams;
use sdrkit_core::publish::{field, keys};
use sdrkit_core::{
    Clock, Complex32, Event, EventCatalog, EventDef, Field, SettingSet, Value, Values,
};
use sdrkit_driver_core::{DriverError, Transmitter, TxSetting};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Bursts that the session's radio sent.
pub const TX_SENT: Field<field::I64> = Field::new("session.tx_sent", "Bursts sent", "");
/// Bursts that were refused or did not reach the radio in full.
pub const TX_FAILED: Field<field::I64> = Field::new("session.tx_failed", "Bursts failed", "");
/// What stops the session from transmitting now: the transmit switch of the engine is off, or the transmitter of the
/// radio is at its lowest power. No value while nothing stops it.
pub const TX_BLOCKED: Field<field::Text> = Field::new("session.tx_blocked", "Transmit blocked", "");
/// When the last burst ended.
pub const TX_LAST: Field<field::Time> = Field::new("session.tx_last", "Last burst", "");

/// The burst went on air.
pub const BURST_SENT: Field<field::Bool> = Field::new("sent", "Sent", "");
/// The samples of the burst.
pub const BURST_SAMPLES: Field<field::I64> = Field::new("samples", "Samples", "");
/// The attenuation of the transmitter during the burst: the negative of [`BURST_TX_POWER`].
pub const BURST_ATTENUATION: Field<field::F64> = Field::new("attenuation", "Attenuation", "dB");
/// How many times the engine sent the burst: 1, and 1 more for each time the radio lost a part of it.
pub const BURST_ATTEMPTS: Field<field::I64> = Field::new("attempts", "Attempts", "");
/// Why the burst did not go on air in full.
pub const BURST_REFUSED: Field<field::Text> = Field::new("refused", "Refused because", "");
/// The radio setting `tx_power` during the burst: the output power relative to the full output of the radio.
pub const BURST_TX_POWER: Field<field::F64> = Field::new("tx_power_db", "Transmit power", "dB");

/// A burst of a session ended: on air, or refused with the reason. One event for each burst that the session wrote,
/// also when the engine sent it more than once. The event's frequency is the burst's centre.
pub static TRANSMIT: EventDef = EventDef::new(
    "session.transmit",
    "Burst",
    &[
        &BURST_SENT,
        &BURST_SAMPLES,
        &BURST_ATTENUATION,
        &BURST_ATTEMPTS,
        &BURST_REFUSED,
        &keys::FAMILY,
        &BURST_TX_POWER,
    ],
);

/// The samples the session thread takes from the transmit stream at one time.
const CHUNK: usize = 4096;
/// The longest time that the transmit thread waits for room in the transmitter. A radio takes the first buffers of a burst
/// more slowly than it plays them, and a driver puts silence in front of a burst: the room comes later than one buffer's
/// time.
const WAIT: Duration = Duration::from_secs(2);
/// The longest time that the end of a burst waits until the radio has played it.
const DRAIN_WAIT: Duration = Duration::from_secs(4);

/// The lead of a burst whose end is not queued: the burst goes on the air when its first samples have waited this
/// long. The radio then plays this far behind the stack, so the stack can be this late with its next samples. The
/// first sample of a burst is on the air within this time, one look of the transmit thread ([`TX_POLL`]) and the time
/// that the radio takes to go on the air.
pub(crate) const LEAD: Duration = Duration::from_millis(40);

/// How long before the handover of a burst with a stated start the transmitter is armed. The calls that arm a radio
/// take a time that the engine does not know; they end inside this time, and zeros fill what is left of it, so the
/// time of the first sample does not depend on them. Past this time the burst is late by the excess.
const ARM_AHEAD: Duration = Duration::from_millis(100);

/// Zeros for the front of a burst with a stated start.
static ZEROS: [Complex32; CHUNK] = [Complex32::new(0.0, 0.0); CHUNK];

/// The clock of the start and the starvation of a burst.
#[derive(Clone, Debug)]
pub(crate) enum Mono {
    /// The time since the transmit side was made.
    Real(Instant),
    /// Microseconds that a test sets.
    #[cfg(test)]
    Manual(Arc<std::sync::atomic::AtomicU64>),
}

impl Mono {
    fn now(&self) -> Duration {
        match self {
            Mono::Real(start) => start.elapsed(),
            #[cfg(test)]
            Mono::Manual(us) => Duration::from_micros(us.load(Ordering::Acquire)),
        }
    }
}

/// How long before the handover of a burst with a stated start `tx` is armed: [`ARM_AHEAD`] and the time of one
/// transfer at `rate_hz`, so the zeros in front complete the first transfer.
fn arm_ahead(tx: &dyn Transmitter, rate_hz: f64) -> Duration {
    ARM_AHEAD + Duration::from_secs_f64(tx.capabilities().buffer_samples as f64 / rate_hz)
}

/// How many more times the engine sends a burst that the radio lost a part of.
const RESEND: u32 = 2;

/// Why a burst did not go on air in full.
#[derive(Debug, thiserror::Error)]
enum Refusal {
    #[error("transmit is off: the engine setting \"Transmit\" (transmit_allowed) is off")]
    NotAllowed,
    #[error("the transmitter did not open: {0}")]
    Open(DriverError),
    /// The sentence that names the transmit setting by its label.
    #[error("the burst does not fit the transmitter: {0}")]
    Fit(String),
    #[error("the transmitter refused its settings: {0}")]
    Configure(DriverError),
    #[error("the transmitter did not go on air: {0}")]
    Arm(DriverError),
    #[error("the transmitter accepted {accepted} of {of} samples")]
    Short { accepted: usize, of: usize },
    #[error("the burst did not fit the session's transmit stream: {lost} samples were lost")]
    Overflow { lost: u64 },
    #[error("the transmitter failed: {0}")]
    Push(DriverError),
    #[error("the burst left the radio with {gaps} gaps of {samples} samples in all: the radio ran out of samples")]
    Gaps { gaps: u64, samples: u64 },
    #[error("the radio did not play the whole burst in time: its end was cut")]
    Cut,
    #[error("the transmitter did not end the burst: {0}")]
    Flush(DriverError),
    #[error("the stack wrote the burst too slowly: the radio ran out of samples {after_s:.3} s into the burst, which was ended")]
    Starved { after_s: f64 },
}

impl Refusal {
    /// A burst that the transmit setting refuses, named by its label in `values`, the transmit settings, if known.
    fn fit(values: Option<&Values>, e: &sdrkit_core::SettingError) -> Refusal {
        Refusal::Fit(values.map_or_else(
            || e.to_string(),
            |v| sdrkit_core::owner::setting_error_text(v.schema(), "setting", e),
        ))
    }
    /// True if the radio took the burst and lost a part of it on the way: the same samples can go out whole in a new
    /// burst. False if the burst never started, if its samples are gone, or if the transmitter failed.
    fn lost_on_the_way(&self) -> bool {
        match self {
            // The transmitter stopped taking samples, the radio ran dry inside the burst, or the radio did not get
            // the end of the burst in time.
            Refusal::Short { .. } | Refusal::Gaps { .. } | Refusal::Cut => true,
            Refusal::NotAllowed
            | Refusal::Open(_)
            | Refusal::Fit(_)
            | Refusal::Configure(_)
            | Refusal::Arm(_)
            | Refusal::Overflow { .. }
            | Refusal::Push(_)
            | Refusal::Flush(_)
            | Refusal::Starved { .. } => false,
        }
    }
}

/// What the session's results show about its bursts.
#[derive(Clone, Debug, Default)]
pub(crate) struct TxReport {
    pub(crate) sent: u64,
    pub(crate) failed: u64,
    /// When the last burst ended, seconds since 1970.
    pub(crate) last: Option<f64>,
    /// Why the last burst failed. `None` if it went on air.
    pub(crate) problem: Option<String>,
    /// Moves on when `problem` changes.
    pub(crate) problem_seq: u64,
    /// What stops a burst from this session now, as a sentence. `None` if nothing does.
    pub(crate) blocked: Option<String>,
}

/// Where the burst is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Burst {
    /// No burst.
    Idle,
    /// On air, with the samples sent and the centre of the burst.
    Sending { samples: u64, centre_hz: f64 },
    /// Refused or broken: its samples are dropped until it ends.
    Dropped { samples: u64, centre_hz: f64 },
    /// Ended and reported before its end mark came: its samples are dropped until the end mark.
    Ended,
}

/// The transmit side of a session on a radio that can transmit.
///
/// Rule: the transmitter opens, off air, at the first burst. A burst whose end mark is queued starts at once. A burst
/// whose end is not queued starts when its first samples have waited [`LEAD`] and that much of it is queued (at most
/// half of the stream): the stack says when a burst starts, and the lead is all that the engine adds. The engine then
/// sets the transmit frequency and rate of the radio's transmit settings to those of the burst, arms, and writes the
/// samples as the stack writes them. A burst of any length is sent this way. The burst ends at its end mark
/// (`HostData::Iq` with `end` set): the engine flushes and disarms. A burst is refused if the engine setting
/// `transmit_allowed` is off, if the transmitter refuses, or if a part of it was written over in the transmit stream
/// before it was read.
///
/// Rule: a burst with a stated start ([`Streams::transmit_start`]) waits in the transmit stream. The engine opens
/// and configures the transmitter, off the air, and reads its latency ([`Transmitter::start_latency`]). It arms the
/// transmitter [`ARM_AHEAD`] and one transfer before the handover, and pushes zeros for the time that is left less
/// the latency, then the burst: the first sample of the burst is on the air at the stated time. A burst written
/// later than that goes out at once and is late by what is missing. A burst that is sent again goes out at once.
///
/// Rule: a burst on the air whose stack wrote less than the radio has played is starved. The engine disarms at once,
/// reports the burst as failed ([`Refusal::Starved`]) and drops the rest of it up to its end mark.
///
/// Rule: a burst whose end is queued is copied whole out of the transmit stream before it starts, so a burst with a
/// part written over never goes on air. The engine keeps the samples of the burst while it fits the room of the
/// transmit stream. If the radio lost a part of the burst ([`Refusal::lost_on_the_way`]), the engine sends the kept
/// samples again as a new burst, at most [`RESEND`] times. Later bursts wait in the transmit stream in their order.
/// A burst longer than the transmit stream is not kept and is sent once.
pub(crate) struct TxSide {
    radio: Arc<Radio>,
    allowed: Arc<AtomicBool>,
    clock: Clock,
    tx: Option<Box<dyn Transmitter>>,
    /// The transmit settings, rate and centre that the transmitter has.
    configured: Option<(u64, f64, f64)>,
    /// The position of the engine in the transmit stream.
    at: u64,
    /// The index of the next burst end in the transmit stream.
    next_end: u64,
    /// The samples of the transmit stream lost so far.
    lost: u64,
    /// The state that `report.blocked` was made from: the switch and the count of setting changes of the radio.
    blocked_for: Option<(bool, u64)>,
    /// The underruns of the transmitter (count, samples) when the burst started.
    underruns: (u64, u64),
    /// The radio setting `tx_power` that the transmitter was configured with, dB.
    tx_power: Option<f64>,
    buf: Vec<Complex32>,
    burst: Burst,
    /// The samples of the burst in flight. Grows to the longest burst, at most the room of the transmit stream.
    keep: Vec<Complex32>,
    /// The samples of the burst in `keep`. `None` if the burst does not fit.
    kept: Option<usize>,
    /// The rate of the burst in flight.
    rate_hz: f64,
    /// Why the burst in flight broke before its end.
    refusal: Option<Refusal>,
    /// The burst in flight is counted as failed.
    counted: bool,
    /// The samples that the transmitter took in the last attempt.
    pushed: u64,
    /// Set when the session stops: no burst is sent again.
    halt: Arc<AtomicBool>,
    mono: Mono,
    /// A test sends at a rate that its radio does not have: the burst is not fitted to the radio.
    #[cfg(test)]
    any_rate: bool,
    /// When the first samples of the burst that waits for its lead were seen.
    seen: Option<Duration>,
    /// When the burst in flight went on the air.
    on_air: Duration,
    /// The place of the engine in the stated starts of the transmit stream.
    next_start: u64,
    /// The stated start of the burst that starts next, UTC.
    due: Option<f64>,
    /// The time that the zeros in front of the burst in flight play.
    zeros: Duration,
    /// What the lead of the transmit stream was made from: the switch, and the bits of the rate and of the centre.
    lead_for: Option<(bool, u64, u64)>,
    pub(crate) report: TxReport,
}

impl TxSide {
    /// The transmit side of a session on `radio`, if the radio can transmit. `allowed` follows the engine setting
    /// `transmit_allowed`.
    pub(crate) fn new(radio: Arc<Radio>, allowed: Arc<AtomicBool>, clock: Clock) -> Option<TxSide> {
        radio.owner.lock().tx_values()?;
        Some(TxSide {
            radio,
            allowed,
            clock,
            tx: None,
            configured: None,
            at: 0,
            next_end: 0,
            lost: 0,
            blocked_for: None,
            underruns: (0, 0),
            tx_power: None,
            buf: vec![Complex32::default(); CHUNK],
            burst: Burst::Idle,
            keep: Vec::new(),
            kept: None,
            rate_hz: 0.0,
            refusal: None,
            counted: false,
            pushed: 0,
            halt: Arc::new(AtomicBool::new(false)),
            mono: Mono::Real(Instant::now()),
            #[cfg(test)]
            any_rate: false,
            seen: None,
            on_air: Duration::ZERO,
            next_start: 0,
            due: None,
            zeros: Duration::ZERO,
            lead_for: None,
            report: TxReport::default(),
        })
    }

    /// Sends what waits in `streams` to the transmitter, and ends the burst at its end mark. The end of a burst is an
    /// event in `events`, stamped by `catalog`.
    pub(crate) fn step(
        &mut self,
        streams: &Streams,
        catalog: &EventCatalog,
        events: &mut Vec<Event>,
    ) {
        self.note_blocked();
        self.note_lead(streams);
        let room = streams.transmit_room();
        loop {
            if self.burst == Burst::Idle {
                // A stated start counts for the burst that starts in this look only.
                self.due = None;
                let stated = streams.burst_start(self.at, &mut self.next_start);
                if let Some(utc) = stated {
                    if !self.handover(streams, utc) {
                        return;
                    }
                }
                match streams.burst_len(self.at, self.next_end) {
                    Some(len) if len <= room => {
                        self.seen = None;
                        self.send_whole(streams, len, catalog, events);
                        continue;
                    }
                    // The end is queued and a part of the burst is written over: it is refused below.
                    Some(_) => {}
                    None => {
                        if stated.is_none() && !self.lead_over(streams, room) {
                            return;
                        }
                    }
                }
                self.seen = None;
            }
            // The end of the burst is not queued, or the burst is longer than the stream: it goes out as it comes.
            let Some(r) = streams.read_burst(&mut self.at, &mut self.next_end, &mut self.buf)
            else {
                self.check_starved(catalog, events);
                return;
            };
            if self.burst == Burst::Ended {
                // What is lost of this burst does not count for the next one.
                self.lost = streams.transmit_lost();
                if r.end {
                    self.burst = Burst::Idle;
                }
                continue;
            }
            let n = r.iq.samples;
            let lost = streams.transmit_lost();
            let overflow = lost - self.lost;
            self.lost = lost;
            if self.burst == Burst::Idle && n > 0 {
                self.kept = Some(0);
                self.rate_hz = r.iq.rate_hz;
                let started = if overflow > 0 {
                    Err(Refusal::Overflow { lost: overflow })
                } else {
                    self.start(r.iq.rate_hz, r.iq.centre_hz)
                };
                self.burst = match started {
                    Ok(()) => {
                        // The radio plays the zeros in front before the samples of the stack.
                        self.on_air = self.mono.now() + self.zeros;
                        Burst::Sending {
                            samples: 0,
                            centre_hz: r.iq.centre_hz,
                        }
                    }
                    Err(why) => {
                        self.refuse(why);
                        Burst::Dropped {
                            samples: 0,
                            centre_hz: r.iq.centre_hz,
                        }
                    }
                };
            } else if let (Burst::Sending { samples, centre_hz }, true) = (self.burst, overflow > 0)
            {
                self.refuse(Refusal::Overflow { lost: overflow });
                self.burst = Burst::Dropped { samples, centre_hz };
            }
            // The kept samples are the burst only while none was written over and they fit.
            self.kept = match self.kept {
                Some(k) if overflow == 0 && k + n <= room => {
                    if self.keep.len() < k + n {
                        self.keep.resize(k + n, Complex32::default());
                    }
                    self.keep[k..k + n].copy_from_slice(&self.buf[..n]);
                    Some(k + n)
                }
                _ => None,
            };
            if let Burst::Sending { samples, centre_hz } = self.burst {
                let pushed = self.tx.as_mut().map(|t| t.push(&self.buf[..n], WAIT));
                self.burst = match pushed {
                    Some(Ok(k)) if k == n => Burst::Sending {
                        samples: samples + n as u64,
                        centre_hz,
                    },
                    Some(Ok(k)) => {
                        self.refuse(Refusal::Short { accepted: k, of: n });
                        Burst::Dropped {
                            samples: samples + k as u64,
                            centre_hz,
                        }
                    }
                    Some(Err(e)) => {
                        self.broken();
                        self.refuse(Refusal::Push(e));
                        Burst::Dropped { samples, centre_hz }
                    }
                    None => {
                        self.refuse(Refusal::Open(DriverError::CannotTransmit));
                        Burst::Dropped { samples, centre_hz }
                    }
                };
            }
            if r.end {
                self.close(true, catalog, events);
            }
        }
    }

    /// True if the burst that waits in `streams` without its end can start: its first samples have waited [`LEAD`],
    /// and the samples of that time, at most half of the `room` of the stream, are queued.
    fn lead_over(&mut self, streams: &Streams, room: usize) -> bool {
        let queued = streams.transmit_queued(self.at);
        if queued == 0 {
            self.seen = None;
            return false;
        }
        let now = self.mono.now();
        let seen = *self.seen.get_or_insert(now);
        let lead = (LEAD.as_secs_f64() * streams.transmit_rate()).ceil() as u64;
        now.saturating_sub(seen) >= LEAD && queued >= lead.min(room as u64 / 2)
    }

    /// Tells the graph through `streams` how long before its stated start a burst must be written: the latency of
    /// the transmitter, the time that the transmitter is armed ahead, and one look of the transmit thread. For this
    /// the transmitter is opened and configured, off the air, at the format that the graph gave. Done again when the
    /// transmit switch or that format changes; zero while the transmitter cannot take the format.
    fn note_lead(&mut self, streams: &Streams) {
        let (rate_hz, centre_hz) = (streams.transmit_rate(), streams.transmit_centre());
        let state = (
            self.allowed.load(Ordering::Acquire),
            rate_hz.to_bits(),
            centre_hz.to_bits(),
        );
        if self.burst != Burst::Idle || rate_hz <= 0.0 || self.lead_for == Some(state) {
            return;
        }
        self.lead_for = Some(state);
        let lead = match (self.prepare(rate_hz, centre_hz), self.tx.as_ref()) {
            (Ok(()), Some(tx)) => tx.start_latency() + arm_ahead(&**tx, rate_hz) + TX_POLL,
            _ => Duration::ZERO,
        };
        streams.set_transmit_lead(lead);
    }

    /// True if the burst that waits in `streams` with the stated start `utc` goes to the transmitter now: the time to
    /// its start is no more than the latency of the transmitter, [`ARM_AHEAD`] and one transfer. Before that the
    /// transmitter is opened and configured, off the air. A burst that the transmitter cannot take goes on at once,
    /// and its start refuses it.
    fn handover(&mut self, streams: &Streams, utc: f64) -> bool {
        if streams.transmit_queued(self.at) == 0 {
            return false;
        }
        let rate_hz = streams.transmit_rate();
        if rate_hz <= 0.0 || self.prepare(rate_hz, streams.transmit_centre()).is_err() {
            return true;
        }
        let Some(tx) = self.tx.as_ref() else {
            return true;
        };
        let ahead = tx.start_latency() + arm_ahead(&**tx, rate_hz);
        if utc - self.clock.now() > ahead.as_secs_f64() {
            return false;
        }
        self.due = Some(utc);
        true
    }

    /// Pushes the zeros that put the first sample of the burst on the air at `utc`: the time to `utc` less the
    /// latency of the transmitter, at `rate_hz`. Pushes nothing if that time has passed. On an error the transmitter
    /// is off the air.
    fn fill_to(&mut self, utc: f64, rate_hz: f64) -> Result<(), Refusal> {
        let Some(tx) = self.tx.as_mut() else {
            return Ok(());
        };
        let most = arm_ahead(&**tx, rate_hz).as_secs_f64();
        let left = (utc - self.clock.now() - tx.start_latency().as_secs_f64()).min(most);
        if left.is_nan() || left <= 0.0 {
            return Ok(());
        }
        let mut zeros = (left * rate_hz).round() as usize;
        self.zeros = Duration::from_secs_f64(zeros as f64 / rate_hz);
        while zeros > 0 {
            let of = zeros.min(CHUNK);
            let failed = match tx.push(&ZEROS[..of], WAIT) {
                Ok(accepted) if accepted == of => None,
                Ok(accepted) => Some(Refusal::Short { accepted, of }),
                Err(e) => Some(Refusal::Push(e)),
            };
            if let Some(why) = failed {
                tx.disarm();
                return Err(why);
            }
            zeros -= of;
        }
        Ok(())
    }

    /// Ends the burst on the air if the stack wrote less of it than the radio has played since it went on the air:
    /// the radio has no sample left. The transmitter goes off the air at once, the burst is reported as failed, and
    /// what the stack still writes of it is dropped up to its end mark.
    fn check_starved(&mut self, catalog: &EventCatalog, events: &mut Vec<Event>) {
        let Burst::Sending { samples, centre_hz } = self.burst else {
            return;
        };
        if self.rate_hz <= 0.0 {
            return;
        }
        let written_s = samples as f64 / self.rate_hz;
        let on_air_s = self.mono.now().saturating_sub(self.on_air).as_secs_f64();
        if on_air_s <= written_s {
            return;
        }
        self.pushed = samples;
        // The transmitter is disarmed without an end of the burst: nothing more goes out.
        let _ = self.off_air(false);
        self.kept = None;
        self.refusal = None;
        self.burst = Burst::Ended;
        let why = Refusal::Starved { after_s: written_s };
        self.report(1, Err(why), centre_hz, catalog, events);
    }

    /// Copies the burst of `len` samples whose end is queued out of `streams`, sends it, and reports it. A burst with a
    /// part written over in the stream is refused before it starts.
    fn send_whole(
        &mut self,
        streams: &Streams,
        len: usize,
        catalog: &EventCatalog,
        events: &mut Vec<Event>,
    ) {
        if self.keep.len() < len {
            self.keep.resize(len, Complex32::default());
        }
        let mut got = 0;
        let (mut rate_hz, mut centre_hz) = (0.0, 0.0);
        while let Some(r) =
            streams.read_burst(&mut self.at, &mut self.next_end, &mut self.keep[got..len])
        {
            got += r.iq.samples;
            (rate_hz, centre_hz) = (r.iq.rate_hz, r.iq.centre_hz);
            if r.end {
                break;
            }
        }
        if len == 0 {
            return;
        }
        self.pushed = 0;
        let (attempts, out) = if got < len {
            // The samples lost after the end of this burst count for the burst that follows.
            let lost = (len - got) as u64;
            self.lost = (self.lost + lost).min(streams.transmit_lost());
            (1, Err(Refusal::Overflow { lost }))
        } else {
            let first = self.attempt(len, rate_hz, centre_hz);
            self.again(1, first, len, rate_hz, centre_hz)
        };
        self.report(attempts, out, centre_hz, catalog, events);
    }

    /// Keeps `why` as the reason that the burst in flight broke before its end. A burst that is not sent again counts
    /// as failed at once: a burst can run for as long as the session does.
    fn refuse(&mut self, why: Refusal) {
        if !why.lost_on_the_way() {
            self.count_failed(&why);
            self.counted = true;
            self.kept = None;
        }
        self.refusal = Some(why);
    }

    /// Counts a failed burst and keeps `why` as the problem.
    fn count_failed(&mut self, why: &Refusal) {
        self.report.failed += 1;
        self.report.problem = Some(why.to_string());
        self.report.problem_seq += 1;
    }

    /// Sends the `len` samples in `keep` as one burst at `rate_hz` around `centre_hz`: arms, writes, ends the burst, waits
    /// until the radio has played it, and goes off air.
    fn attempt(&mut self, len: usize, rate_hz: f64, centre_hz: f64) -> Result<(), Refusal> {
        self.pushed = 0;
        self.start(rate_hz, centre_hz)?;
        let mut broke = None;
        for piece in self.keep[..len].chunks(CHUNK) {
            let Some(t) = self.tx.as_mut() else {
                broke = Some(Refusal::Open(DriverError::CannotTransmit));
                break;
            };
            match t.push(piece, WAIT) {
                Ok(k) => {
                    self.pushed += k as u64;
                    if k < piece.len() {
                        let of = piece.len();
                        broke = Some(Refusal::Short { accepted: k, of });
                    }
                }
                Err(e) => broke = Some(Refusal::Push(e)),
            }
            if broke.is_some() {
                break;
            }
        }
        if matches!(broke, Some(Refusal::Push(_))) {
            self.broken();
        }
        let ended = self.off_air(broke.is_none());
        broke.map_or(ended, Err)
    }

    /// Sends the `len` samples in `keep` again while `out`, the outcome of attempt number `attempts`, says that the
    /// radio lost a part of the burst. Returns the count of attempts and the outcome of the last.
    fn again(
        &mut self,
        mut attempts: u32,
        mut out: Result<(), Refusal>,
        len: usize,
        rate_hz: f64,
        centre_hz: f64,
    ) -> (u32, Result<(), Refusal>) {
        while out.as_ref().is_err_and(Refusal::lost_on_the_way)
            && attempts <= RESEND
            && !self.halt.load(Ordering::Acquire)
        {
            attempts += 1;
            out = self.attempt(len, rate_hz, centre_hz);
        }
        (attempts, out)
    }

    /// Opens, configures and arms the transmitter for a burst at `rate_hz` around `centre_hz`, and pushes the zeros
    /// in front of a burst with a stated start ([`TxSide::fill_to`]).
    fn start(&mut self, rate_hz: f64, centre_hz: f64) -> Result<(), Refusal> {
        let due = self.due.take();
        self.zeros = Duration::ZERO;
        self.prepare(rate_hz, centre_hz)?;
        let tx = self
            .tx
            .as_mut()
            .ok_or(Refusal::Open(DriverError::CannotTransmit))?;
        let s = tx.stats();
        self.underruns = (s.underruns, s.underrun_samples);
        tx.arm().map_err(Refusal::Arm)?;
        match due {
            Some(utc) => self.fill_to(utc, rate_hz),
            None => Ok(()),
        }
    }

    /// Opens the transmitter, off the air, and configures it for a burst at `rate_hz` around `centre_hz`. The
    /// transmit frequency and the transmit rate of the radio's settings become those of the burst. Refused while the
    /// engine setting `transmit_allowed` is off.
    fn prepare(&mut self, rate_hz: f64, centre_hz: f64) -> Result<(), Refusal> {
        if !self.allowed.load(Ordering::Acquire) {
            return Err(Refusal::NotAllowed);
        }
        #[cfg(test)]
        if self.any_rate {
            return Ok(());
        }
        let (values, seq) = {
            let mut o = self.radio.owner.lock();
            if self.tx.is_none() {
                self.tx = Some(o.transmitter().map_err(Refusal::Open)?);
                self.configured = None;
            }
            o.tx_tune(centre_hz, Some(rate_hz))
                .map_err(|e| Refusal::fit(o.tx_values().as_ref(), &e))?;
            (o.tx_values(), o.tx_seq())
        };
        let cannot = || Refusal::Open(DriverError::CannotTransmit);
        let mut values: Values = values.ok_or_else(cannot)?;
        let tx = self.tx.as_mut().ok_or_else(cannot)?;
        self.tx_power = match values.value(TxSetting::Power.id()) {
            Some(Value::F64(db)) => Some(*db),
            _ => None,
        };
        if self.configured != Some((seq, rate_hz, centre_hz)) {
            values
                .set(TxSetting::Frequency.id(), &Value::Hz(centre_hz))
                .and_then(|()| values.set(TxSetting::Rate.id(), &Value::Hz(rate_hz)))
                .map_err(|e| Refusal::fit(Some(&values), &e))?;
            tx.configure(&values).map_err(Refusal::Configure)?;
            self.configured = Some((seq, rate_hz, centre_hz));
        }
        Ok(())
    }

    /// Ends the burst that runs, if one does, and reports it. The session stops: the burst is not sent again.
    pub(crate) fn end(&mut self, catalog: &EventCatalog, events: &mut Vec<Event>) {
        self.close(false, catalog, events);
    }

    /// Ends the burst that went out as it came, if one runs, and reports it. `whole` says that its end mark was read:
    /// the kept samples are then the burst, and go out again if the radio lost a part of it.
    fn close(&mut self, whole: bool, catalog: &EventCatalog, events: &mut Vec<Event>) {
        let (sent, samples, centre_hz) = match self.burst {
            Burst::Idle => return,
            Burst::Ended => {
                self.burst = Burst::Idle;
                return;
            }
            Burst::Sending { samples, centre_hz } => (true, samples, centre_hz),
            Burst::Dropped { samples, centre_hz } => (false, samples, centre_hz),
        };
        self.burst = Burst::Idle;
        self.pushed = samples;
        let ended = self.off_air(sent);
        let first = self.refusal.take().map_or(ended, Err);
        let (attempts, out) = match self.kept.take().filter(|_| whole) {
            Some(len) => self.again(1, first, len, self.rate_hz, centre_hz),
            None => (1, first),
        };
        self.report(attempts, out, centre_hz, catalog, events);
    }

    /// Takes the transmitter off air at the end of a burst. If the transmitter took the `whole` burst, ends the burst
    /// first and waits until the radio has played it.
    ///
    /// # Errors
    ///
    /// The reason if the burst did not leave the radio whole.
    fn off_air(&mut self, whole: bool) -> Result<(), Refusal> {
        let Some(t) = self.tx.as_mut() else {
            return Ok(());
        };
        if !whole {
            t.disarm();
            return Ok(());
        }
        let flushed = t.flush();
        // Disarm drops what is queued: the burst is on air only after the radio has played it.
        let played = flushed.is_err() || t.drain(DRAIN_WAIT);
        t.disarm();
        flushed.map_err(Refusal::Flush)?;
        if !played {
            return Err(Refusal::Cut);
        }
        // A burst with gaps is not a frame that a receiver reads.
        let s = t.stats();
        let gaps = s.underruns.saturating_sub(self.underruns.0);
        if gaps > 0 {
            let samples = s.underrun_samples.saturating_sub(self.underruns.1);
            return Err(Refusal::Gaps { gaps, samples });
        }
        Ok(())
    }

    /// Counts the burst, keeps the reason as the problem if it failed, and makes its event. `attempts` is how many
    /// times the engine sent it, `out` the outcome of the last attempt.
    fn report(
        &mut self,
        attempts: u32,
        out: Result<(), Refusal>,
        centre_hz: f64,
        catalog: &EventCatalog,
        events: &mut Vec<Event>,
    ) {
        let now = self.clock.now();
        self.report.last = Some(now);
        match &out {
            Ok(()) => {
                self.report.sent += 1;
                if self.report.problem.take().is_some() {
                    self.report.problem_seq += 1;
                }
            }
            Err(why) => {
                if !std::mem::take(&mut self.counted) {
                    self.count_failed(why);
                }
            }
        }
        let mut e = TRANSMIT.event(now, centre_hz);
        e.fields.set(&BURST_SENT, out.is_ok());
        e.fields.set(
            &BURST_SAMPLES,
            i64::try_from(self.pushed).unwrap_or(i64::MAX),
        );
        e.fields
            .opt(&BURST_ATTENUATION, self.tx_power.map(|db| 0.0 - db));
        e.fields.set(&BURST_ATTEMPTS, i64::from(attempts));
        if out.is_err() {
            e.fields.opt(&BURST_REFUSED, self.report.problem.as_deref());
        }
        e.fields.set(&keys::FAMILY, keys::Family::Receiver);
        e.fields.opt(&BURST_TX_POWER, self.tx_power);
        if catalog.stamp(&mut e).is_some() {
            events.push(e);
        }
    }

    /// Writes what stops a burst now into the report. The sentence is made again only when the switch or a setting of
    /// the radio changed.
    fn note_blocked(&mut self) {
        let allowed = self.allowed.load(Ordering::Acquire);
        let o = self.radio.owner.lock();
        let now = (allowed, o.tx_seq());
        if self.blocked_for == Some(now) {
            return;
        }
        self.blocked_for = Some(now);
        let muted = o.tx_values().and_then(|v| {
            let id = TxSetting::Power.id();
            let min = v.schema().setting(id)?.range?.min;
            match v.value(id)? {
                Value::F64(db) if *db <= min => Some(*db),
                _ => None,
            }
        });
        self.report.blocked = match (allowed, muted) {
            (false, _) => Some(
                "transmit is off: the engine setting \"Transmit\" (transmit_allowed) is off".into(),
            ),
            (true, Some(db)) => Some(format!(
                "the transmitter is muted: the radio setting \"Transmit power\" (tx_power) is {db} dB, its lowest value"
            )),
            (true, None) => None,
        };
    }

    /// Closes the transmitter that failed during a burst. The next burst opens it again.
    fn broken(&mut self) {
        if let Some(mut t) = self.tx.take() {
            t.close();
        }
        self.configured = None;
    }
}

impl Drop for TxSide {
    fn drop(&mut self) {
        if let Some(mut t) = self.tx.take() {
            t.disarm();
            t.close();
        }
    }
}

/// The transmit side of a session on a thread of its own. A burst goes to the radio at the pace of the radio, and the
/// end of a burst waits until the radio has played it: on the session thread that would hold up the receive side for
/// the length of every burst.
pub(crate) struct TxWorker {
    stop: Arc<AtomicBool>,
    /// The report of the transmit side, and the burst events that the session has not taken.
    shared: Arc<std::sync::Mutex<(TxReport, Vec<Event>)>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// How often the worker looks for transmit samples.
const TX_POLL: Duration = Duration::from_millis(5);

impl TxWorker {
    /// Starts the thread that sends the transmit stream of `streams` through `side`.
    ///
    /// # Errors
    ///
    /// The error of the system if the thread does not start.
    pub(crate) fn start(
        mut side: TxSide,
        streams: Arc<Streams>,
        catalog: Arc<EventCatalog>,
    ) -> std::io::Result<TxWorker> {
        let stop = Arc::new(AtomicBool::new(false));
        side.halt = Arc::clone(&stop);
        let shared = Arc::new(std::sync::Mutex::new((side.report.clone(), Vec::new())));
        let (halt, out) = (Arc::clone(&stop), Arc::clone(&shared));
        let thread = std::thread::Builder::new()
            .name("sdrkit-transmit".into())
            .spawn(move || {
                let mut events = Vec::new();
                let copy = |side: &TxSide, events: &mut Vec<Event>| {
                    let mut s = lock(&out);
                    s.0.clone_from(&side.report);
                    s.1.append(events);
                };
                while !halt.load(Ordering::Acquire) {
                    side.step(&streams, &catalog, &mut events);
                    copy(&side, &mut events);
                    std::thread::sleep(TX_POLL);
                }
                // The burst that runs ends, and the transmitter goes off air.
                side.end(&catalog, &mut events);
                copy(&side, &mut events);
            })?;
        Ok(TxWorker {
            stop,
            shared,
            thread: Some(thread),
        })
    }

    /// Appends the burst events since the last call to `events` and returns the report of the transmit side.
    pub(crate) fn take(&self, events: &mut Vec<Event>) -> TxReport {
        let mut s = lock(&self.shared);
        events.append(&mut s.1);
        s.0.clone()
    }

    /// Stops the thread after it ended the burst that runs, and returns what [`TxWorker::take`] returns.
    pub(crate) fn finish(mut self, events: &mut Vec<Event>) -> TxReport {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.take(events)
    }
}

impl Drop for TxWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, Setup};
    use sdrkit_core::net::FakeTransport;
    use sdrkit_core::{Facilities, MemStorage, Random};
    use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
    use sdrkit_driver_core::{TxCapabilities, TxState, TxStats};
    use sdrkit_graph::components::{HostData, HostPort, HostStreams};
    use std::sync::Mutex;

    /// What the test transmitter saw, and what it does.
    #[derive(Default)]
    struct Air {
        /// The samples of each burst, in the order of the calls of arm.
        bursts: Vec<Vec<Complex32>>,
        /// The bursts from now on that lose samples in the radio.
        lose: usize,
        /// The samples that the transmitter takes of each burst. `None`: all.
        takes: Option<usize>,
        /// The radio never plays the end of a burst.
        cut: bool,
        underruns: u64,
        /// Runs once, at the end of the first burst.
        at_first_end: Option<Box<dyn FnOnce() + Send>>,
        /// The clock of the bench in microseconds and the rate of the radio: with them the radio plays a burst from
        /// the time it was armed, and a push after the radio ran out of samples is an underrun.
        radio: Option<(Arc<std::sync::atomic::AtomicU64>, f64)>,
        /// The time from which the radio plays the burst without a gap, microseconds.
        plays_from: f64,
        /// On the air.
        armed: bool,
        /// When each burst was armed, microseconds.
        armed_at: Vec<u64>,
        /// The time from a push to the antenna.
        latency: Duration,
    }

    /// A transmitter that keeps what it is given and loses samples as [`Air`] says.
    struct Double {
        caps: TxCapabilities,
        air: Arc<Mutex<Air>>,
    }

    impl Transmitter for Double {
        fn capabilities(&self) -> &TxCapabilities {
            &self.caps
        }
        fn configure(&mut self, _: &Values) -> Result<(), DriverError> {
            Ok(())
        }
        fn arm(&mut self) -> Result<(), DriverError> {
            let mut a = lock(&self.air);
            a.bursts.push(Vec::new());
            a.armed = true;
            if let Some(now) = a.radio.as_ref().map(|(c, _)| c.load(Ordering::Acquire)) {
                a.armed_at.push(now);
                a.plays_from = now as f64;
            }
            Ok(())
        }
        fn disarm(&mut self) {
            lock(&self.air).armed = false;
        }
        fn push(&mut self, iq: &[Complex32], _: Duration) -> Result<usize, DriverError> {
            let mut a = lock(&self.air);
            if let Some((now, rate)) = a
                .radio
                .as_ref()
                .map(|(c, rate)| (c.load(Ordering::Acquire) as f64, *rate))
            {
                let got = a.bursts.last().map_or(0, Vec::len);
                let dry_at = a.plays_from + got as f64 / rate * 1e6;
                if now > dry_at + 1.0 {
                    a.underruns += 1;
                    a.plays_from += now - dry_at;
                }
            }
            let takes = a.takes;
            let burst = a.bursts.last_mut().ok_or(DriverError::CannotTransmit)?;
            let n = takes.map_or(iq.len(), |t| t.saturating_sub(burst.len()).min(iq.len()));
            burst.extend_from_slice(&iq[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> Result<(), DriverError> {
            let hook = {
                let mut a = lock(&self.air);
                if a.lose > 0 {
                    a.lose -= 1;
                    a.underruns += 1;
                }
                a.at_first_end.take()
            };
            if let Some(f) = hook {
                f();
            }
            Ok(())
        }
        fn start_latency(&self) -> Duration {
            lock(&self.air).latency
        }
        fn drain(&mut self, _: Duration) -> bool {
            !lock(&self.air).cut
        }
        fn play_cyclic(&mut self, _: &[Complex32]) -> Result<(), DriverError> {
            Err(DriverError::CannotTransmit)
        }
        fn state(&self) -> TxState {
            TxState::Disarmed
        }
        fn stats(&self) -> TxStats {
            let n = lock(&self.air).underruns;
            TxStats {
                underruns: n,
                underrun_samples: 100 * n,
                ..TxStats::default()
            }
        }
        fn close(&mut self) {}
    }

    /// A transmit side on a fake radio, with the test transmitter in the place of the transmitter of the radio.
    struct Bench {
        // The radio lives as long as the engine and the server do.
        _srv: FakeIiod,
        engine: Engine,
        side: TxSide,
        streams: Arc<Streams>,
        air: Arc<Mutex<Air>>,
        events: Vec<Event>,
        /// The clock of the transmit side, microseconds.
        clock: Arc<std::sync::atomic::AtomicU64>,
    }

    /// A bench whose transmit stream holds `room` samples. `allowed` is the transmit switch of the engine.
    fn bench(room: usize, allowed: bool) -> Bench {
        let net = FakeTransport::new();
        let srv = FakeIiod::start_on(net.clone(), FakeOpts::default());
        let engine = Engine::new(Setup {
            facilities: Facilities {
                storage: Arc::new(MemStorage::new()),
                clock: Clock::fixed(1.7e9),
                random: Random::fixed(7),
                codecs: Arc::default(),
            },
            transport: Arc::new(net),
            stacks: Vec::new(),
            families: Vec::new(),
            components: sdrkit_graph::Registry::standard(),
        })
        .unwrap();
        let kind = engine.find("driver.iiod").unwrap();
        let values = engine
            .owner(kind, |o| Values::defaults(o.schema()))
            .unwrap();
        let r = engine.open_radio("iiod", &srv.addr(), &values).unwrap();
        let radio = engine.shared.radio(r).unwrap();
        let caps = {
            let mut real = radio.owner.lock().transmitter().unwrap();
            let caps = real.capabilities().clone();
            real.close();
            caps
        };
        let air = Arc::new(Mutex::new(Air::default()));
        let mut side = TxSide::new(
            radio,
            Arc::new(AtomicBool::new(allowed)),
            Clock::fixed(1.7e9),
        )
        .unwrap();
        side.tx = Some(Box::new(Double {
            caps,
            air: Arc::clone(&air),
        }));
        let clock = Arc::new(std::sync::atomic::AtomicU64::new(0));
        side.mono = Mono::Manual(Arc::clone(&clock));
        Bench {
            _srv: srv,
            engine,
            side,
            streams: Arc::new(Streams::new(1, room)),
            air,
            events: Vec::new(),
            clock,
        }
    }

    impl Bench {
        fn step(&mut self) {
            let catalog = Arc::clone(self.engine.event_catalog());
            self.side.step(&self.streams, &catalog, &mut self.events);
        }

        /// The sent flag, the attempts and the refusal of each burst event.
        fn told(&self) -> Vec<(bool, i64, String)> {
            let field = |e: &Event| {
                (
                    e.fields.get(&BURST_SENT).unwrap(),
                    e.fields.get(&BURST_ATTEMPTS).unwrap(),
                    e.fields.get(&BURST_REFUSED).unwrap_or_default().to_owned(),
                )
            };
            self.events.iter().map(field).collect()
        }

        fn on_air(&self) -> Vec<Vec<Complex32>> {
            lock(&self.air).bursts.clone()
        }

        /// Moves the clock of the transmit side on by `us` microseconds.
        fn advance(&self, us: u64) {
            self.clock.fetch_add(us, Ordering::AcqRel);
        }

        /// Makes the test transmitter a radio that plays at `rate` on the clock of the bench, whatever rates the
        /// radio of the bench has.
        fn paced(&mut self, rate: f64) {
            lock(&self.air).radio = Some((Arc::clone(&self.clock), rate));
            self.side.any_rate = true;
        }

        /// The stack writes a burst of `total` samples at `rate` in pieces of `piece`, each when its time has come
        /// on the clock of the bench, every second piece `late_us` late; `stall` (samples, microseconds) holds the
        /// stack up once after that many samples. The transmit side looks every 5 ms. The burst gets its end mark.
        /// Returns the time from the first write to the arming of the transmitter, in seconds.
        fn stream(
            &mut self,
            rate: f64,
            piece: usize,
            total: usize,
            late_us: u64,
            stall: Option<(usize, u64)>,
        ) -> Option<f64> {
            let start = self.clock.load(Ordering::Acquire);
            let before = lock(&self.air).armed_at.len();
            let (mut written, mut k, mut held) = (0_usize, 0_u64, 0_u64);
            let mut ended = false;
            while !ended {
                let now = self.clock.load(Ordering::Acquire) - start;
                loop {
                    if let Some((_, us)) = stall.filter(|(at, _)| written >= *at && held == 0) {
                        held = us;
                    }
                    let due = (written as f64 / rate * 1e6) as u64 + held + (k % 2) * late_us;
                    if due > now {
                        break;
                    }
                    if written == total {
                        write_at(&self.streams, &[], rate, true);
                        ended = true;
                        break;
                    }
                    let n = piece.min(total - written);
                    write_at(&self.streams, &iq(written, n), rate, false);
                    written += n;
                    k += 1;
                }
                self.step();
                self.advance(5000);
            }
            self.step();
            let a = lock(&self.air);
            a.armed_at.get(before).map(|at| (*at - start) as f64 / 1e6)
        }
    }

    /// `n` samples that start at the value `from`.
    fn iq(from: usize, n: usize) -> Vec<Complex32> {
        (from..from + n)
            .map(|i| Complex32::new(i as f32, 0.0))
            .collect()
    }

    fn write(s: &Streams, samples: &[Complex32], end: bool) {
        write_at(s, samples, 2.4e6, end);
    }

    fn write_at(s: &Streams, samples: &[Complex32], rate_hz: f64, end: bool) {
        s.write(
            HostPort::Transmit,
            HostData::Iq {
                samples,
                rate_hz,
                centre_hz: 915e6,
                end,
                start: None,
            },
        );
    }

    #[test]
    fn a_burst_that_lost_samples_in_the_radio_is_sent_again() {
        let mut b = bench(1 << 16, true);
        lock(&b.air).lose = 1;
        let burst = iq(0, 10_000);
        write(&b.streams, &burst, true);
        b.step();
        assert_eq!(b.told(), [(true, 2, String::new())]);
        assert_eq!(b.on_air(), [burst.clone(), burst]);
        assert_eq!(b.events[0].fields.get(&BURST_SAMPLES), Some(10_000));
        assert_eq!((b.side.report.sent, b.side.report.failed), (1, 0));
        assert_eq!(b.side.report.problem, None);
        assert_eq!(b.side.report.problem_seq, 0);
    }

    #[test]
    fn a_burst_that_always_loses_samples_fails_after_three_attempts() {
        let mut b = bench(1 << 16, true);
        lock(&b.air).lose = 3;
        let burst = iq(0, 5000);
        write(&b.streams, &burst, true);
        b.step();
        let told = b.told();
        assert_eq!(told.len(), 1);
        assert_eq!((told[0].0, told[0].1), (false, 3));
        assert!(told[0].2.contains("1 gaps of 100 samples"), "{}", told[0].2);
        assert_eq!(b.on_air().len(), 3);
        assert_eq!((b.side.report.sent, b.side.report.failed), (0, 1));
        assert_eq!(b.side.report.problem.as_deref(), Some(told[0].2.as_str()));
        // The next burst goes out whole: the problem goes.
        write(&b.streams, &burst, true);
        b.step();
        assert_eq!(b.told()[1], (true, 1, String::new()));
        assert_eq!((b.side.report.sent, b.side.report.failed), (1, 1));
        assert_eq!(b.side.report.problem, None);
    }

    #[test]
    fn a_burst_refused_before_it_starts_is_not_sent_again() {
        let mut b = bench(1 << 16, false);
        write(&b.streams, &iq(0, 5000), true);
        b.step();
        let told = b.told();
        assert_eq!((told[0].0, told[0].1), (false, 1));
        assert!(told[0].2.contains("transmit_allowed"), "{}", told[0].2);
        assert_eq!(told.len(), 1);
        assert!(b.on_air().is_empty());
        assert_eq!((b.side.report.sent, b.side.report.failed), (0, 1));
    }

    #[test]
    fn bursts_written_while_a_burst_is_sent_again_follow_it_in_order() {
        let mut b = bench(1 << 16, true);
        let (first, second, third) = (iq(0, 3000), iq(10_000, 2000), iq(20_000, 1000));
        {
            let mut a = lock(&b.air);
            a.lose = 1;
            let (s, second, third) = (Arc::clone(&b.streams), second.clone(), third.clone());
            a.at_first_end = Some(Box::new(move || {
                write(&s, &second, true);
                write(&s, &third, true);
            }));
        }
        write(&b.streams, &first, true);
        b.step();
        assert_eq!(b.on_air(), [first.clone(), first, second, third]);
        let ok = |attempts| (true, attempts, String::new());
        assert_eq!(b.told(), [ok(2), ok(1), ok(1)]);
        assert_eq!((b.side.report.sent, b.side.report.failed), (3, 0));
    }

    #[test]
    fn a_burst_written_over_while_a_burst_is_sent_again_is_refused_whole() {
        let mut b = bench(1024, true);
        let (first, second, third) = (iq(0, 300), iq(10_000, 600), iq(20_000, 600));
        {
            let mut a = lock(&b.air);
            a.lose = 1;
            let (s, second, third) = (Arc::clone(&b.streams), second.clone(), third.clone());
            a.at_first_end = Some(Box::new(move || {
                write(&s, &second, true);
                // The stream holds 1024 samples: 176 samples of the burst before are written over.
                write(&s, &third, true);
            }));
        }
        write(&b.streams, &first, true);
        b.step();
        // No sample of the burst that lost a part goes on air.
        assert_eq!(b.on_air(), [first.clone(), first, third]);
        let told = b.told();
        assert_eq!(told.len(), 3);
        assert_eq!((told[0].0, told[0].1), (true, 2));
        assert_eq!((told[1].0, told[1].1), (false, 1));
        assert!(told[1].2.contains("176 samples were lost"), "{}", told[1].2);
        assert_eq!((told[2].0, told[2].1), (true, 1));
        assert_eq!((b.side.report.sent, b.side.report.failed), (2, 1));
    }

    #[test]
    fn a_burst_that_lost_its_start_behind_a_burst_written_over_is_refused() {
        let mut b = bench(1024, true);
        let first = iq(0, 100);
        {
            let mut a = lock(&b.air);
            a.lose = 1;
            let s = Arc::clone(&b.streams);
            a.at_first_end = Some(Box::new(move || {
                write(&s, &iq(10_000, 200), true);
                // The end of this burst is not written: all of the burst before and 76 of its samples are written over.
                write(&s, &iq(20_000, 1100), false);
            }));
        }
        write(&b.streams, &first, true);
        b.step();
        write(&b.streams, &[], true);
        b.step();
        assert_eq!(b.on_air(), [first.clone(), first]);
        let told = b.told();
        assert_eq!(told.len(), 3);
        assert_eq!((told[0].0, told[0].1), (true, 2));
        assert!(told[1].2.contains("200 samples were lost"), "{}", told[1].2);
        assert!(told[2].2.contains("76 samples were lost"), "{}", told[2].2);
        assert_eq!((b.side.report.sent, b.side.report.failed), (1, 2));
    }

    #[test]
    fn a_burst_written_over_whole_is_refused() {
        let mut b = bench(1024, true);
        write(&b.streams, &iq(0, 500), true);
        write(&b.streams, &iq(10_000, 1024), true);
        b.step();
        let told = b.told();
        assert_eq!(told.len(), 2);
        assert_eq!((told[0].0, told[0].1), (false, 1));
        assert!(told[0].2.contains("500 samples were lost"), "{}", told[0].2);
        assert_eq!((told[1].0, told[1].1), (true, 1));
        assert_eq!(b.on_air(), [iq(10_000, 1024)]);
    }

    #[test]
    fn a_burst_that_started_before_its_end_was_written_is_sent_again() {
        let mut b = bench(1024, true);
        lock(&b.air).lose = 1;
        let burst = iq(0, 800);
        // The burst waits for its lead, then it starts: half of this small stream is queued.
        write(&b.streams, &burst[..600], false);
        b.step();
        assert!(b.on_air().is_empty());
        b.advance(LEAD.as_micros() as u64);
        b.step();
        assert_eq!(b.on_air(), [burst[..600].to_vec()]);
        assert!(b.events.is_empty());
        write(&b.streams, &burst[600..], true);
        b.step();
        assert_eq!(b.told(), [(true, 2, String::new())]);
        assert_eq!(b.on_air(), [burst.clone(), burst]);
        assert_eq!(b.events[0].fields.get(&BURST_SAMPLES), Some(800));
    }

    #[test]
    fn a_burst_that_the_transmitter_took_in_part_or_that_was_cut_is_sent_again() {
        let mut b = bench(1 << 16, true);
        lock(&b.air).takes = Some(4500);
        write(&b.streams, &iq(0, 5000), true);
        b.step();
        let told = b.told();
        assert_eq!((told[0].0, told[0].1), (false, 3));
        assert!(told[0].2.contains("accepted 404 of 904"), "{}", told[0].2);
        assert_eq!(b.events[0].fields.get(&BURST_SAMPLES), Some(4500));
        {
            let mut a = lock(&b.air);
            a.takes = None;
            a.cut = true;
        }
        write(&b.streams, &iq(0, 5000), true);
        b.step();
        let told = b.told();
        assert_eq!((told[1].0, told[1].1), (false, 3));
        assert!(told[1].2.contains("cut"), "{}", told[1].2);
        assert_eq!(b.on_air().len(), 6);
    }

    #[test]
    fn a_stopped_session_does_not_send_a_burst_again() {
        let mut b = bench(1 << 16, true);
        lock(&b.air).lose = 3;
        b.side.halt.store(true, Ordering::Release);
        write(&b.streams, &iq(0, 5000), true);
        b.step();
        let told = b.told();
        assert_eq!((told[0].0, told[0].1), (false, 1));
        assert_eq!(b.on_air().len(), 1, "{}", told[0].2);
    }

    /// The sum of the samples that went on the air in burst `i`, and their count.
    fn sum_on_air(b: &Bench, i: usize) -> (f64, usize) {
        let a = lock(&b.air);
        let burst = &a.bursts[i];
        (burst.iter().map(|c| f64::from(c.re)).sum(), burst.len())
    }

    /// The sum of the values 0 to `n` - 1 as `f32` samples hold them.
    fn sum_of(n: usize) -> f64 {
        (0..n).map(|i| f64::from(i as f32)).sum()
    }

    /// A burst whose end is queued starts in the look that finds it, at both session rates.
    #[test]
    fn a_short_burst_starts_at_once() {
        for rate in [250e3, 2e6] {
            let mut b = bench(crate::transmit_room(rate), true);
            b.paced(rate);
            b.advance(1_000_000);
            write_at(&b.streams, &iq(0, (0.2 * rate) as usize), rate, true);
            b.step();
            assert_eq!(lock(&b.air).armed_at, [1_000_000], "{rate}");
            assert_eq!(b.told(), [(true, 1, String::new())]);
        }
    }

    /// A burst longer than the transmit stream, written at the pace of the receive side in small and in large
    /// pieces (the weak-signal stacks send 12.6 s to 110 s), is on the air within the lead and one look after its
    /// first samples, and the radio never runs out of samples, also when every second piece is 30 ms late.
    #[test]
    fn a_long_burst_written_in_real_time_starts_within_the_lead_and_has_no_gap() {
        // The pieces: 5 ms, and the block of the test-signal radio (131 ms at 250 kHz, 16 ms at 2 MHz).
        for (rate, piece) in [(250e3, 1250), (250e3, 32_768), (2e6, 10_000), (2e6, 32_768)] {
            for late_us in [0, 30_000] {
                let mut b = bench(crate::transmit_room(rate), true);
                b.paced(rate);
                let total = (3.0 * rate) as usize;
                assert!(total > b.streams.transmit_room());
                let late_s = b.stream(rate, piece, total, late_us, None);
                let what = format!("{rate} Hz, pieces of {piece}, {late_us} us late");
                if late_us == 0 {
                    assert!(
                        late_s.is_some_and(|s| s <= 0.045),
                        "{what}: on air {late_s:?} s after the first samples"
                    );
                }
                assert_eq!(b.told(), [(true, 1, String::new())], "{what}");
                assert_eq!(lock(&b.air).underruns, 0, "{what}");
                assert_eq!(sum_on_air(&b, 0), (sum_of(total), total), "{what}");
                assert!(!lock(&b.air).armed);
            }
        }
    }

    /// Push-to-talk: a transmission of no known length, written block by block of the receive side until the key is
    /// let go. Speech starts within the lead and has no gap.
    #[test]
    fn push_to_talk_starts_within_the_lead_and_has_no_gap() {
        for (rate, piece) in [(250e3, 32_768), (2e6, 32_768), (48e3, 4800)] {
            let mut b = bench(crate::transmit_room(rate), true);
            b.paced(rate);
            let total = (4.0 * rate) as usize;
            let late_s = b.stream(rate, piece, total, 20_000, None);
            assert!(late_s.is_some_and(|s| s <= 0.07), "{rate}: {late_s:?}");
            assert_eq!(b.told(), [(true, 1, String::new())], "{rate}");
            assert_eq!(lock(&b.air).underruns, 0, "{rate}");
            assert_eq!(sum_on_air(&b, 0).1, total);
        }
    }

    /// The stack stops writing inside a burst: the burst ends at once with the transmitter off the air and one event,
    /// the rest of the burst is dropped, and the next burst goes out.
    #[test]
    fn a_starved_burst_ends_off_the_air_and_the_next_burst_goes_out() {
        for rate in [250e3, 2e6] {
            let mut b = bench(crate::transmit_room(rate), true);
            b.paced(rate);
            let total = (3.0 * rate) as usize;
            let before = (1.0 * rate) as usize;
            // The stack is held up for 0.3 s after 1 s of the burst.
            b.stream(rate, 10_000, total, 0, Some((before, 300_000)));
            let told = b.told();
            assert_eq!(told.len(), 1, "{told:?}");
            assert_eq!((told[0].0, told[0].1), (false, 1));
            assert!(told[0].2.contains("too slowly"), "{}", told[0].2);
            assert_eq!(sum_on_air(&b, 0).1, before);
            assert_eq!(b.events[0].fields.get(&BURST_SAMPLES), Some(before as i64));
            assert!(!lock(&b.air).armed);
            // The radio played what it had: no push came after it ran out.
            assert_eq!(lock(&b.air).underruns, 0);
            assert_eq!((b.side.report.sent, b.side.report.failed), (0, 1));
            let next = iq(7, 5000);
            write_at(&b.streams, &next, rate, true);
            b.step();
            assert_eq!(b.told()[1], (true, 1, String::new()));
            assert_eq!(b.on_air().len(), 2);
            assert_eq!(b.on_air()[1], next);
        }
    }

    /// The UTC at which the clock of the bench is 0.
    const BENCH_UTC: f64 = 1.7e9;

    /// Makes the wall clock of the transmit side follow the clock of the bench, and the test transmitter a radio
    /// that plays at `rate` with `latency_ms` from a push to the antenna.
    fn timed(b: &mut Bench, rate: f64, latency_ms: u64) {
        b.paced(rate);
        lock(&b.air).latency = Duration::from_millis(latency_ms);
        let c = Arc::clone(&b.clock);
        b.side.clock = Clock::new(move || BENCH_UTC + c.load(Ordering::Acquire) as f64 / 1e6);
    }

    /// When the first sample of burst `i` that is not zero is on the air, in seconds on the clock of the bench: the
    /// time of arming, the latency, and the samples in front of it. Also the count of those samples.
    fn air_time(b: &Bench, i: usize, rate: f64) -> (f64, usize) {
        let a = lock(&b.air);
        let zeros = a.bursts[i]
            .iter()
            .position(|s| *s != Complex32::default())
            .unwrap();
        let s = a.armed_at[i] as f64 / 1e6 + a.latency.as_secs_f64() + zeros as f64 / rate;
        (s, zeros)
    }

    /// A burst written whole before its stated start waits, and its first sample is on the air at the stated time
    /// to the sample, whatever the latency of the transmitter. The transmitter is armed no earlier than the
    /// latency, the arming time and one look before the start.
    #[test]
    fn a_whole_burst_with_a_stated_start_is_on_the_air_at_that_time() {
        let rate = 250e3;
        for latency_ms in [0, 37, 262] {
            let mut b = bench(1 << 16, true);
            timed(&mut b, rate, latency_ms);
            let burst = iq(1, 10_000);
            b.streams.transmit_start(BENCH_UTC + 0.8);
            write_at(&b.streams, &burst, rate, true);
            for _ in 0..400 {
                b.step();
                if !b.events.is_empty() {
                    break;
                }
                b.advance(5000);
            }
            assert_eq!(b.told(), [(true, 1, String::new())], "{latency_ms} ms");
            let (s, zeros) = air_time(&b, 0, rate);
            assert!((s - 0.8).abs() <= 1.0 / rate, "{latency_ms} ms: on air at {s}");
            let armed = lock(&b.air).armed_at[0] as f64 / 1e6;
            let earliest = 0.8 - latency_ms as f64 / 1e3 - ARM_AHEAD.as_secs_f64() - 0.005;
            assert!(armed >= earliest - 1e-6, "{latency_ms} ms: armed at {armed}");
            assert_eq!(b.on_air()[0][zeros..], burst[..], "{latency_ms} ms");
            assert_eq!(b.events[0].fields.get(&BURST_SAMPLES), Some(10_000));
        }
    }

    /// A long burst with a stated start that the stack writes in real time, from half a second before the start: its
    /// first sample is on the air at the stated time, and the radio never runs out of samples.
    #[test]
    fn a_streamed_burst_with_a_stated_start_is_on_the_air_at_that_time() {
        let rate = 250e3;
        let mut b = bench(crate::transmit_room(rate), true);
        timed(&mut b, rate, 262);
        let total = (3.0 * rate) as usize;
        b.streams.transmit_start(BENCH_UTC + 0.5);
        b.stream(rate, 1250, total, 0, None);
        assert_eq!(b.told(), [(true, 1, String::new())]);
        assert_eq!(lock(&b.air).underruns, 0);
        // The first sample that the stack writes here is zero: the second one is found.
        let (second, found) = air_time(&b, 0, rate);
        let (s, zeros) = (second - 1.0 / rate, found - 1);
        assert!((s - 0.5).abs() <= 1.0 / rate, "on air at {s}");
        assert_eq!(sum_on_air(&b, 0), (sum_of(total), zeros + total));
    }

    /// A burst whose stated start has passed goes out at once, with no zeros in front.
    #[test]
    fn a_burst_written_after_its_stated_start_goes_out_at_once() {
        let rate = 250e3;
        let mut b = bench(1 << 16, true);
        timed(&mut b, rate, 262);
        b.advance(1_000_000);
        let burst = iq(1, 10_000);
        b.streams.transmit_start(BENCH_UTC + 0.9);
        write_at(&b.streams, &burst, rate, true);
        b.step();
        assert_eq!(b.told(), [(true, 1, String::new())]);
        assert_eq!(b.on_air(), [burst]);
    }

    /// The UTC of instant `t`, from one reading of both clocks.
    fn utc_of(t: Instant) -> f64 {
        let (now, utc) = (Instant::now(), crate::system_clock().now());
        if t >= now {
            utc + (t - now).as_secs_f64()
        } else {
            utc - (now - t).as_secs_f64()
        }
    }

    /// Watches the stand-in radio `board` play at `rate` until `done`. Returns when its first transmit sample that is
    /// not zero left it: the end of the played transfers less the samples after that one. A look between the arrival
    /// of a transfer and its time of play is early by that transfer, so the latest look counts.
    fn first_on_air(
        board: &sdrkit_driver_core::iiod::fake::Board,
        rate: f64,
        done: &AtomicBool,
    ) -> Option<Instant> {
        let (mut scanned, mut first, mut on_air) = (0, None, None);
        while !done.load(Ordering::Acquire) {
            let end = board.tx_play.lock().unwrap().0;
            let n = {
                let bytes = board.tx_bytes.lock().unwrap();
                let n = bytes.len() / 4;
                if first.is_none() {
                    first = (scanned..n).find(|&i| bytes[4 * i..4 * i + 4] != [0; 4]);
                    scanned = n;
                }
                n
            };
            if let (Some(end), Some(k)) = (end, first) {
                let t = end.checked_sub(Duration::from_secs_f64((n - k) as f64 / rate));
                on_air = on_air.max(t);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        on_air
    }

    /// Sends a burst of `seconds` with a stated start through the iiod driver to a stand-in radio that plays its
    /// transmit buffers at the sample rate, in real time. `ahead` is how long before its start the stack begins to
    /// write the burst at the sample rate; `None` writes it whole. Returns the time of the first sample on the air
    /// less the stated start, in seconds.
    fn start_error_on_the_stand_in_radio(seconds: f64, ahead: Option<f64>) -> f64 {
        let rate = 2.5e6;
        let net = FakeTransport::new();
        let opts = FakeOpts {
            tx_plays: true,
            ..FakeOpts::default()
        };
        let srv = FakeIiod::start_on(net.clone(), opts);
        let engine = Engine::new(Setup {
            facilities: Facilities {
                storage: Arc::new(MemStorage::new()),
                clock: crate::system_clock(),
                random: Random::fixed(7),
                codecs: Arc::default(),
            },
            transport: Arc::new(net),
            stacks: Vec::new(),
            families: Vec::new(),
            components: sdrkit_graph::Registry::standard(),
        })
        .unwrap();
        let kind = engine.find("driver.iiod").unwrap();
        let values = engine
            .owner(kind, |o| Values::defaults(o.schema()))
            .unwrap();
        let r = engine.open_radio("iiod", &srv.addr(), &values).unwrap();
        let radio = engine.shared.radio(r).unwrap();
        let allowed = Arc::new(AtomicBool::new(true));
        let mut side = TxSide::new(radio, allowed, crate::system_clock()).unwrap();
        let streams = Streams::new(1, crate::transmit_room(rate));
        let catalog = Arc::clone(engine.event_catalog());
        let mut events = Vec::new();
        let total = (seconds * rate) as usize;
        let piece = vec![Complex32::new(0.5, 0.0); (0.005 * rate) as usize];
        let begin = Instant::now();
        let start = crate::system_clock().now() + 1.0;
        let done = AtomicBool::new(false);
        let on_air = std::thread::scope(|scope| {
            let watch = scope.spawn(|| first_on_air(&srv.board, rate, &done));
            streams.transmit_start(start);
            let mut written = 0;
            let mut ended = false;
            while events.is_empty() && begin.elapsed() < Duration::from_secs(20) {
                let due = match ahead {
                    None => total,
                    Some(a) => {
                        let t = begin.elapsed().as_secs_f64() - (1.0 - a);
                        ((t.max(0.0) * rate) as usize).min(total)
                    }
                };
                while written < due {
                    let n = piece.len().min(due - written);
                    write_at(&streams, &piece[..n], rate, false);
                    written += n;
                }
                if written == total && !ended {
                    write_at(&streams, &[], rate, true);
                    ended = true;
                }
                side.step(&streams, &catalog, &mut events);
                std::thread::sleep(TX_POLL);
            }
            done.store(true, Ordering::Release);
            watch.join().unwrap()
        });
        assert_eq!(events.len(), 1, "one burst is reported");
        assert_eq!(events[0].fields.get(&BURST_SENT), Some(true));
        assert_eq!(srv.board.tx_gap_in_signal.load(Ordering::Relaxed), 0);
        utc_of(on_air.expect("a time of play")) - start
    }

    /// The largest error of the start of a burst with a stated start on the stand-in radio, in seconds.
    const START_TOLERANCE_S: f64 = 0.020;

    /// A burst written whole with a stated start leaves the stand-in radio at that time: the engine hands it over
    /// early by the latency that the driver reports.
    #[test]
    fn a_whole_burst_leaves_the_stand_in_radio_at_its_stated_start() {
        let error = start_error_on_the_stand_in_radio(0.3, None);
        eprintln!("whole burst on air {error:+.4} s from its stated start");
        assert!(error.abs() <= START_TOLERANCE_S, "start error {error:+.4} s");
    }

    /// A burst that the stack writes at the sample rate from 0.6 s before its stated start leaves the stand-in radio
    /// at that time.
    #[test]
    fn a_streamed_burst_leaves_the_stand_in_radio_at_its_stated_start() {
        let error = start_error_on_the_stand_in_radio(1.0, Some(0.6));
        eprintln!("streamed burst on air {error:+.4} s from its stated start");
        assert!(error.abs() <= START_TOLERANCE_S, "start error {error:+.4} s");
    }
}
