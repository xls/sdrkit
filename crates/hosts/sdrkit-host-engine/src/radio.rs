//! An open radio: the radio as an owner, its ring, and the thread that reads the ring into the radio's feed.

use crate::feed::{Feed, BLOCK};
use crate::hop::{HopGate, Phase};
use crate::published::Kept;
use crate::EngineError;
use sdrkit_core::{
    CommandError, CommandId, Commands, Complex32, Event, EventCatalog, Owner, Record, Results,
    ResultsDecl, Schema, SettingError, SettingId, StateBlob, StateError, Value, Values,
};
use sdrkit_driver_core::{
    ring, Gap, OpenRadio, RingReader, RingStats, CENTRE_GIVEN, RATE_GIVEN, RESTART,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// The fewest and the most samples that the ring of a radio holds.
const RING_MIN: usize = 1 << 21;
const RING_MAX: usize = 1 << 23;
/// The time of samples at the highest rate of the radio that its ring holds, seconds: a network hands samples over in
/// bursts, and the sessions take them from the ring at their own pace.
const RING_S: f64 = 0.25;

/// The samples that the ring of a radio with the highest rate `max_rate_hz` holds: [`RING_S`] of them as a power of
/// two, from [`RING_MIN`] to [`RING_MAX`].
pub(crate) fn ring_samples(max_rate_hz: f64) -> usize {
    // The cast saturates, and the limits hold the result.
    ((max_rate_hz.max(0.0) * RING_S) as usize)
        .clamp(RING_MIN, RING_MAX)
        .next_power_of_two()
        .min(RING_MAX)
}
/// The events a radio keeps for the host. The oldest go first.
const EVENT_CAP: usize = 256;

/// The rate and centre of the samples, as the radio last reported them.
#[derive(Debug, Default)]
pub(crate) struct Tuning {
    rate: AtomicU64,
    centre: AtomicU64,
    /// Counts the changes of the rate. The samples that wait in the ring at a change have the former rate.
    rate_changes: AtomicU64,
}

impl Tuning {
    fn set(&self, rate: f64, centre: f64) {
        if self.rate.swap(rate.to_bits(), Ordering::Relaxed) != rate.to_bits() {
            self.rate_changes.fetch_add(1, Ordering::Release);
        }
        self.centre.store(centre.to_bits(), Ordering::Relaxed);
    }
    fn get(&self) -> (f64, f64) {
        (
            f64::from_bits(self.rate.load(Ordering::Relaxed)),
            f64::from_bits(self.centre.load(Ordering::Relaxed)),
        )
    }
}

/// An open radio as an owner, with its own queue of events.
pub(crate) struct RadioOwner {
    radio: OpenRadio,
    /// Shared with [`Radio::events`], so that the host takes the events without the lock of the owner.
    events: Arc<Mutex<VecDeque<Event>>>,
    tuning: Arc<Tuning>,
    pump: Option<Pump>,
    feed: Arc<Feed>,
    scratch: Vec<Event>,
    /// Shared with [`Radio::auto_rate`].
    auto_rate: Arc<AtomicBool>,
    /// Shared with [`Radio::rate_given_back`].
    rate_given_back: Arc<AtomicBool>,
    /// Shared with [`Radio::hop`].
    hop: Arc<HopGate>,
    /// Wakes the control thread at the end of a step of a hop.
    wake: Arc<Condvar>,
    /// The counters of the ring of the stream that runs.
    ring: Option<Arc<RingStats>>,
}

struct Pump {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

/// An open radio: its owner and its feed.
pub(crate) struct Radio {
    pub(crate) owner: Kept<RadioOwner>,
    events: Arc<Mutex<VecDeque<Event>>>,
    pub(crate) feed: Arc<Feed>,
    /// The key of its driver kind.
    pub(crate) driver: &'static str,
    pub(crate) address: String,
    /// The host leaves the rate to the engine: the rate was 0 at the open, or the host set it to 0 since. Read
    /// without the lock of the owner.
    auto_rate: Arc<AtomicBool>,
    /// The host set the rate to 0 and the engine did not resolve the rate since.
    rate_given_back: Arc<AtomicBool>,
    /// The phase of the hop of the radio ([`crate::hop`]).
    pub(crate) hop: Arc<HopGate>,
}

impl Radio {
    /// Starts `radio` streaming into a new ring, read into `feed`. The thread of the radio signals `wake` at the end of a
    /// step of a hop.
    pub(crate) fn start(
        radio: OpenRadio,
        address: &str,
        feed: Arc<Feed>,
        auto_rate: bool,
        wake: Arc<Condvar>,
    ) -> Result<Radio, EngineError> {
        let driver = radio.kind().key;
        let events = Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_CAP)));
        let auto_rate = Arc::new(AtomicBool::new(auto_rate));
        let rate_given_back = Arc::new(AtomicBool::new(false));
        let hop = Arc::new(HopGate::default());
        let mut owner = RadioOwner {
            radio,
            events: Arc::clone(&events),
            tuning: Arc::new(Tuning::default()),
            pump: None,
            feed: Arc::clone(&feed),
            scratch: Vec::new(),
            auto_rate: Arc::clone(&auto_rate),
            rate_given_back: Arc::clone(&rate_given_back),
            hop: Arc::clone(&hop),
            wake,
            ring: None,
        };
        owner.stream()?;
        Ok(Radio {
            owner: Kept::new(owner),
            events,
            feed,
            driver,
            address: address.to_owned(),
            auto_rate,
            rate_given_back,
            hop,
        })
    }
}

impl Radio {
    /// Moves the radio's events into `out`. Does not wait for the lock of the owner.
    pub(crate) fn take_events(&self, out: &mut Vec<Event>) {
        out.extend(crate::feed::lock(&self.events).drain(..));
    }

    /// [`RadioOwner::rate_is_automatic`], without the lock of the owner.
    pub(crate) fn rate_is_automatic(&self) -> bool {
        self.auto_rate.load(Ordering::Acquire)
    }

    /// True once after the host set the rate to Automatic: the engine then resolves the rate again.
    pub(crate) fn take_rate_given_back(&self) -> bool {
        self.rate_given_back.swap(false, Ordering::AcqRel)
    }
}

impl RadioOwner {
    /// Starts the stream into a new ring and the thread that reads it.
    fn stream(&mut self) -> Result<(), EngineError> {
        let samples = ring_samples(self.radio.capabilities().rates.max_hz);
        // A session's queue holds what the ring holds: a burst that fits the ring reaches every session whole.
        self.feed.set_queue(samples / BLOCK);
        let (writer, reader) = ring(samples, self.radio.sample_type());
        self.ring = Some(reader.stats());
        self.hop.restarted();
        let started = self.radio.start(writer)?;
        self.tuning.set(started.rate_hz, started.centre_hz);
        let stop = Arc::new(AtomicBool::new(false));
        let (s, t, f) = (
            Arc::clone(&stop),
            Arc::clone(&self.tuning),
            Arc::clone(&self.feed),
        );
        let (hop, wake) = (Arc::clone(&self.hop), Arc::clone(&self.wake));
        let thread = std::thread::Builder::new()
            .name("sdrkit-radio".into())
            .spawn(move || {
                pump(
                    reader,
                    &f,
                    &t,
                    &s,
                    &Steps {
                        gate: &hop,
                        wake: &wake,
                    },
                )
            })
            .map_err(|e| EngineError::thread(&e))?;
        self.pump = Some(Pump { stop, thread });
        Ok(())
    }

    /// Stops the stream and its thread.
    pub(crate) fn stop(&mut self) {
        if let Some(p) = self.pump.take() {
            p.stop.store(true, Ordering::Release);
            // The thread may wait for room in the queue of a session.
            self.feed.cancel_waits(true);
            let _ = p.thread.join();
            self.feed.cancel_waits(false);
        }
        self.radio.stop();
    }

    /// Brings the results up to date, keeps the radio's events with their kind ids, and starts the stream again when the
    /// radio requests it. `now` is the wall time.
    pub(crate) fn poll(&mut self, now: f64, catalog: &EventCatalog) {
        let mut events = std::mem::take(&mut self.scratch);
        self.radio.poll(now, &mut events);
        let mut restart = false;
        {
            let mut kept = crate::feed::lock(&self.events);
            for mut e in events.drain(..) {
                restart |= e.is(&RESTART);
                if catalog.stamp(&mut e).is_some() {
                    if kept.len() == EVENT_CAP {
                        kept.pop_front();
                    }
                    kept.push_back(e);
                }
            }
        }
        self.scratch = events;
        if restart {
            self.stop();
            // A refusal shows in the radio's own results.
            let _ = self.stream();
        }
        self.sync();
    }

    /// The saved state of the radio, if its driver keeps one.
    pub(crate) fn save_state(&self) -> Option<StateBlob> {
        self.radio.save_state()
    }

    /// Restores the radio from a state that [`RadioOwner::save_state`] returned.
    pub(crate) fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.radio.restore_state(state)
    }

    /// The lowest and highest frequency that the radio tunes to now, Hz.
    pub(crate) fn freq_range(&self) -> (f64, f64) {
        let c = self.radio.capabilities();
        (c.min_freq_hz, c.max_freq_hz)
    }

    /// The rate and centre of the radio's samples, as it last reported them.
    pub(crate) fn tuning(&self) -> (f64, f64) {
        self.tuning.get()
    }

    /// The samples that the radio wrote into the ring of its stream so far: the place in the ring of the next sample
    /// that it writes.
    pub(crate) fn ring_written(&self) -> u64 {
        self.ring.as_ref().map_or(0, |r| r.written())
    }

    /// The transmit settings of the radio, if it can transmit.
    pub(crate) fn tx_values(&self) -> Option<Values> {
        self.radio.tx_values()
    }

    /// A number that moves on at each change of a transmit setting.
    pub(crate) fn tx_seq(&self) -> u64 {
        self.radio.tx_seq()
    }

    /// Sets the transmit frequency of the radio and, if `rate_hz` is given, its transmit rate
    /// ([`sdrkit_driver_core::OpenRadio::tx_tune`]).
    pub(crate) fn tx_tune(
        &mut self,
        centre_hz: f64,
        rate_hz: Option<f64>,
    ) -> Result<(), sdrkit_core::SettingError> {
        self.radio.tx_tune(centre_hz, rate_hz)
    }

    /// Opens the transmitter, off air, with the radio's transmit settings.
    pub(crate) fn transmitter(
        &mut self,
    ) -> Result<Box<dyn sdrkit_driver_core::Transmitter>, sdrkit_driver_core::DriverError> {
        self.radio.transmitter()
    }

    /// Applies the centre frequency again: a driver that makes its signal from the settings makes it again.
    pub(crate) fn renew(&mut self) {
        use sdrkit_core::SettingSet;
        let id = sdrkit_driver_core::RadioSetting::Centre.id();
        let Some(v) = self.radio.values().and_then(|v| v.value(id)).cloned() else {
            return;
        };
        let _ = self.radio.apply(id, &v);
        self.sync();
    }

    /// True while the host leaves the rate of the radio on Automatic. The engine then sets the rate that the stack of
    /// the session needs ([`RadioOwner::set_resolved_rate`]), and the rate setting reads that rate.
    pub(crate) fn rate_is_automatic(&self) -> bool {
        self.auto_rate.load(Ordering::Acquire)
    }

    /// The limits of the rates of the radio.
    pub(crate) fn rates(&self) -> &sdrkit_driver_core::RateLimits {
        &self.radio.capabilities().rates
    }

    /// Sets the rate that the engine resolved for a stack. The rate stays on Automatic for the host.
    pub(crate) fn set_resolved_rate(&mut self, rate_hz: f64) -> Result<(), EngineError> {
        let id = EngineError::id_of(self.radio.schema(), sdrkit_driver_core::RATE.field.key)?;
        let r = self.radio.apply(id, &Value::Hz(rate_hz));
        self.sync();
        r.map_err(|e| EngineError::setting(self.radio.schema(), e))
    }

    /// What the radio can do.
    pub(crate) fn capabilities(&self) -> &sdrkit_driver_core::Capabilities {
        self.radio.capabilities()
    }

    /// The sample type that the driver of the radio delivers.
    pub(crate) fn sample_type(&self) -> sdrkit_graph::SampleType {
        self.radio.sample_type()
    }

    /// The gain, the automatic gain and the bandwidth that the radio is set to.
    pub(crate) fn gain_settings(&self) -> (f64, bool, f64) {
        use sdrkit_driver_core as dc;
        self.radio.values().map_or((0.0, false, 0.0), |v| {
            (v.get(&dc::GAIN), v.get(&dc::AGC), v.get(&dc::BANDWIDTH))
        })
    }

    /// Carries out the request `request` of the client of a share that steers the radio. A rate goes in as a rate that
    /// the engine resolved, so the rate stays on Automatic for the host and for the next stack.
    ///
    /// # Errors
    ///
    /// [`EngineError::Setting`] if the radio refuses the value. A request that names no setting of every radio does
    /// nothing.
    pub(crate) fn remote(
        &mut self,
        request: &sdrkit_serve_core::Request,
    ) -> Result<(), EngineError> {
        use sdrkit_driver_core as dc;
        use sdrkit_serve_core::Request;
        let (key, value) = match *request {
            Request::Frequency { hz } => (dc::CENTRE.field.key, Value::Hz(hz)),
            Request::Rate { hz } => return self.set_resolved_rate(hz),
            Request::Bandwidth { hz } => (dc::BANDWIDTH.field.key, Value::Hz(hz)),
            Request::Gain { db } => (dc::GAIN.field.key, Value::F64(db)),
            Request::Agc { on } => (dc::AGC.field.key, Value::Bool(on)),
            Request::BiasTee { .. } | Request::Correction { .. } | Request::Other { .. } => {
                return Ok(())
            }
        };
        let id = EngineError::id_of(self.radio.schema(), key)?;
        let checked = self.radio.schema().check(id, &value);
        let r = checked.and_then(|()| self.radio.apply(id, &value));
        self.sync();
        r.map_err(|e| EngineError::setting(self.radio.schema(), e))
    }

    fn sync(&self) {
        let f = self.radio.results().fields();
        if let (Some(rate), Some(centre)) = (f.get(&RATE_GIVEN), f.get(&CENTRE_GIVEN)) {
            self.tuning.set(rate, centre);
        }
    }
}

impl Owner for RadioOwner {
    fn schema(&self) -> &Arc<Schema> {
        self.radio.schema()
    }
    fn values(&self) -> Option<&Values> {
        self.radio.values()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        // A rate from the host stays. 0 gives the rate back to the engine and does not reach the driver: the radio
        // keeps its rate until the engine sets the rate of a stack.
        let rate = self.radio.schema().id(sdrkit_driver_core::RATE.field.key) == Ok(id);
        if rate && matches!(value, Value::Hz(hz) | Value::F64(hz) if *hz <= 0.0) {
            self.auto_rate.store(true, Ordering::Release);
            self.rate_given_back.store(true, Ordering::Release);
            return Ok(());
        }
        let r = self.radio.apply(id, value);
        self.sync();
        if rate && r.is_ok() {
            self.auto_rate.store(false, Ordering::Release);
            self.rate_given_back.store(false, Ordering::Release);
        }
        r
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.radio.results_decl()
    }
    fn results(&self) -> &Results {
        self.radio.results()
    }
    fn commands(&self) -> &Commands {
        self.radio.commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        self.radio.invoke(id, args)
    }
}

/// Drops what waits in the ring if the rate changed since `seen` changes of it.
fn drop_former_rate(reader: &mut RingReader, tuning: &Tuning, seen: &mut u64) {
    let changes = tuning.rate_changes.load(Ordering::Acquire);
    if changes != *seen {
        *seen = changes;
        reader.skip_all();
    }
}

/// The hop of a radio as its thread sees it: the phase, and the signal to the control thread at the end of a step.
struct Steps<'a> {
    gate: &'a HopGate,
    wake: &'a Condvar,
}

/// The samples that the thread of a radio left out since the last block it passed: those that it dropped on purpose and
/// those that the driver marked, by kind.
#[derive(Default)]
struct LeftOut {
    deliberate: u64,
    lost: u64,
}

impl LeftOut {
    /// Drops `n` waiting samples of `reader` on purpose, with the gaps that the driver marked among them.
    fn leave(&mut self, reader: &mut RingReader, n: usize) {
        for g in reader.take_gaps(n) {
            if g.deliberate {
                self.deliberate += g.lost;
            } else {
                self.lost += g.lost;
            }
        }
        self.deliberate += reader.skip(n) as u64;
    }

    /// Writes what was left out as gaps at the start of a block, and forgets it.
    fn put(&mut self, gaps: &mut Vec<Gap>) {
        if self.lost > 0 {
            gaps.push(Gap::lost_at(0, self.lost));
        }
        if self.deliberate > 0 {
            gaps.push(Gap {
                deliberate: true,
                ..Gap::lost_at(0, self.deliberate)
            });
        }
        *self = LeftOut::default();
    }
}

/// The loop of a radio's thread: whatever the ring holds goes to the feed, one block at a time. At a change of the rate the
/// samples that wait in the ring are dropped: they have the former rate and would go out with the new one. The loop ends
/// when `stop` is set, or when the ring is empty and its writer is gone (the radio stopped, or a recording ended).
///
/// During a hop ([`crate::hop`]) a step passes the samples of its dwell and no more, then signals the control thread and
/// passes nothing until the radio has moved: a live radio's samples are dropped meanwhile, those of another source wait in
/// the ring. The samples before the first one at the new frequency are left out, and the first block of the step starts
/// with a gap made on purpose of all that was left out.
fn pump(mut reader: RingReader, feed: &Feed, tuning: &Tuning, stop: &AtomicBool, hop: &Steps<'_>) {
    let mut rate_changes = tuning.rate_changes.load(Ordering::Acquire);
    let mut left_out = LeftOut::default();
    while !stop.load(Ordering::Acquire) {
        drop_former_rate(&mut reader, tuning, &mut rate_changes);
        if reader.wait(1, Duration::from_millis(50)) == 0 {
            if reader.writer_gone() {
                return;
            }
            continue;
        }
        let phase = hop.gate.phase();
        let limit = match phase {
            Phase::Off => BLOCK,
            Phase::Step { left } => usize::try_from(left).unwrap_or(BLOCK).min(BLOCK),
            Phase::Held => {
                if feed.live() {
                    let n = reader.available();
                    left_out.leave(&mut reader, n);
                } else {
                    hop.gate.wait_moved(Duration::from_millis(50));
                }
                continue;
            }
            Phase::Moved { at, dwell_s } => {
                let ahead =
                    usize::try_from(at.saturating_sub(reader.position())).unwrap_or(usize::MAX);
                let n = ahead.min(reader.available());
                left_out.leave(&mut reader, n);
                if n == ahead {
                    let rate = tuning.get().0;
                    hop.gate
                        .start_step(at, dwell_s.map(|d| (d * rate).round() as u64));
                }
                continue;
            }
        };
        let (rate, centre) = tuning.get();
        // A live feed counts a session that is behind; another feed waits for it. Neither returns an error.
        let sent = feed.deliver_gaps(rate, centre, false, |iq, gaps| {
            if iq.len() != BLOCK {
                iq.resize(BLOCK, Complex32::default());
            }
            left_out.put(gaps);
            // The gaps are counted from the next read: take those of the samples that this read returns.
            let n = reader.available().min(limit);
            gaps.extend(reader.take_gaps(n));
            reader.read_iq(&mut iq[..n])
        });
        if let (Phase::Step { .. }, Ok(n)) = (phase, sent) {
            if hop.gate.passed(n as u64) {
                hop.wake.notify_all();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_holds_a_quarter_second_at_the_highest_rate_within_its_limits() {
        // A radio that does not tell its rates, and a slow one: the smallest ring.
        assert_eq!(ring_samples(0.0), 1 << 21);
        assert_eq!(ring_samples(3.2e6), 1 << 21);
        // 10 Msps: 2.5 million samples, as the next power of two.
        assert_eq!(ring_samples(10e6), 1 << 22);
        // 61.44 Msps: the largest ring.
        assert_eq!(ring_samples(61.44e6), 1 << 23);
        // The queue of a session holds the ring.
        assert_eq!(ring_samples(0.0) / BLOCK, crate::session::QUEUE);
    }

    #[test]
    fn the_samples_that_wait_at_a_change_of_the_rate_are_dropped() {
        let (mut w, mut r) = ring(1 << 16, sdrkit_graph::stream::SampleType::Cf32);
        let tuning = Tuning::default();
        tuning.set(8e6, 100e6);
        let mut seen = tuning.rate_changes.load(Ordering::Acquire);
        assert_eq!(w.write_iq(&[Complex32::new(1.0, 0.0); 1000]), 1000);
        // Another frequency at the same rate drops nothing.
        tuning.set(8e6, 101e6);
        drop_former_rate(&mut r, &tuning, &mut seen);
        assert_eq!(r.available(), 1000);
        tuning.set(64e6 / 7.0, 101e6);
        drop_former_rate(&mut r, &tuning, &mut seen);
        assert_eq!(r.available(), 0);
        assert_eq!(w.write_iq(&[Complex32::new(1.0, 0.0); 10]), 10);
        drop_former_rate(&mut r, &tuning, &mut seen);
        assert_eq!(r.available(), 10);
    }

    #[test]
    fn a_gap_that_the_driver_marked_is_in_the_block_at_its_place() {
        let (mut w, r) = ring(1 << 16, sdrkit_graph::stream::SampleType::Cf32);
        let one = [Complex32::new(1.0, 0.0); 1000];
        assert_eq!(w.write_iq(&one), 1000);
        w.mark_gap(4096);
        assert_eq!(w.write_iq(&one[..500]), 500);
        let catalog = Arc::new(sdrkit_graph::Catalog::default());
        let feed = Arc::new(Feed::new(
            true,
            catalog,
            sdrkit_graph::RunContext::default(),
        ));
        let (tx, rx) = std::sync::mpsc::sync_channel(8);
        assert!(feed.subscribe(crate::feed::Sub {
            session: 1,
            tx,
            dropped: Arc::default(),
            demand: Arc::default(),
            queue: Arc::default(),
            steers: true,
        }));
        let tuning = Tuning::default();
        tuning.set(48e3, 100e6);
        drop(w);
        // The writer is gone: the loop ends when the ring is empty.
        let (gate, wake) = (HopGate::default(), Condvar::new());
        let steps = Steps {
            gate: &gate,
            wake: &wake,
        };
        pump(r, &feed, &tuning, &AtomicBool::new(false), &steps);
        let block = rx.try_recv().unwrap();
        assert_eq!(block.samples().len(), 1500);
        assert_eq!(block.gaps(), [sdrkit_driver_core::Gap::lost_at(1000, 4096)]);
    }
}
