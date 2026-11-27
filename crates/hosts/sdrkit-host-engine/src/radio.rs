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


/// A move of a radio that streams to another centre, until the thread of the radio has reached its first sample. The
/// thread passes no sample while a move waits: the samples that the radio captured before it settled are left out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Move {
    /// The radio is being told.
    Ordered,
    /// The radio took the centre. With `seq`, the driver finds the first sample of its retune of that number
    /// ([`RingStats::moves`]); else the first sample is the one after `floor` samples of the ring.
    Taken { seq: Option<u64>, floor: u64 },
}

/// The moves of one radio that wait, the oldest first.
pub(crate) type Moves = Arc<Mutex<VecDeque<Move>>>;

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
    /// The moves that the thread of the radio has not reached.
    moves: Moves,
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
            moves: Moves::default(),
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
        // A new ring holds samples of the new centre only: no move waits.
        crate::feed::lock(&self.moves).clear();
        self.feed.moves_done_to(self.feed.moves_asked());
        let started = self.radio.start(writer)?;
        self.tuning.set(started.rate_hz, started.centre_hz);
        let stop = Arc::new(AtomicBool::new(false));
        let (s, t, f) = (
            Arc::clone(&stop),
            Arc::clone(&self.tuning),
            Arc::clone(&self.feed),
        );
        let (hop, wake) = (Arc::clone(&self.hop), Arc::clone(&self.wake));
        let moves = Arc::clone(&self.moves);
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
                        moves: &moves,
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

    /// Marks the start of a step of a hop in which the radio stays at its centre, as a move that takes no time: the
    /// samples that the radio wrote so far are left out, and the next block counts the move, so a session takes the
    /// frequency of the step at that block.
    pub(crate) fn mark_step(&mut self) {
        let Some(ring) = self.ring.as_ref().filter(|_| self.pump.is_some()) else {
            return;
        };
        let mut moves = crate::feed::lock(&self.moves);
        moves.push_back(Move::Taken {
            seq: None,
            floor: ring.written(),
        });
        self.feed.move_asked();
    }

    /// Applies `value` to setting `id` of the radio. A new centre of a radio that streams is a move: the thread of the
    /// radio passes no sample from the order until the first sample that the radio captured at the new centre, which
    /// the driver finds ([`RingStats::moves`]) or, for a source with no samples in flight, is the next one written.
    fn apply_radio(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let centre = self.radio.schema().id(sdrkit_driver_core::CENTRE.field.key) == Ok(id);
        let ring = self.ring.clone().filter(|_| centre && self.pump.is_some());
        let Some(ring) = ring else {
            let r = self.radio.apply(id, value);
            self.sync();
            return r;
        };
        let before = (self.tuning.get().1, ring.moves().begun);
        crate::feed::lock(&self.moves).push_back(Move::Ordered);
        let r = self.radio.apply(id, value);
        self.sync();
        let mut moves = crate::feed::lock(&self.moves);
        if self.tuning.get().1.to_bits() == before.0.to_bits() {
            // The radio stays where it is.
            moves.pop_back();
        } else if let Some(m) = moves.back_mut() {
            let begun = ring.moves().begun;
            *m = Move::Taken {
                seq: (begun > before.1).then_some(begun),
                floor: ring.written(),
            };
            self.feed.move_asked();
        }
        r
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
        let _ = self.apply_radio(id, &v);
    }

    /// True while the host leaves the rate of the radio on Automatic. The engine then sets the rate that the stack of
    /// the session needs ([`RadioOwner::set_resolved_rate`]), and the rate setting reads that rate.
    pub(crate) fn rate_is_automatic(&self) -> bool {
        self.auto_rate.load(Ordering::Acquire)
    }

    /// True if the centre of the samples of the radio carries no carrier of its converter.
    pub(crate) fn centre_clean(&self) -> bool {
        self.radio.centre_clean()
    }

    /// The limits of the rates of the radio.
    pub(crate) fn rates(&self) -> &sdrkit_driver_core::RateLimits {
        &self.radio.capabilities().rates
    }

    /// Shows `text` in the radio's status and log.
    pub(crate) fn note(&mut self, text: &str) {
        sdrkit_core::Owner::problem(&mut self.radio, text);
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
        let r = checked.and_then(|()| self.apply_radio(id, &value));
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
        let r = self.apply_radio(id, value);
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
    moves: &'a Mutex<VecDeque<Move>>,
}

/// Leaves out the samples of `reader` before the first sample of the oldest move that waits. Returns false if no move
/// waits: the samples pass. A move whose first sample is reached is counted as done in `feed`.
fn settle_move(
    reader: &mut RingReader,
    feed: &Feed,
    moves: &Mutex<VecDeque<Move>>,
    left_out: &mut LeftOut,
) -> bool {
    let Some(front) = crate::feed::lock(moves).front().copied() else {
        return false;
    };
    let at = match front {
        Move::Ordered => None,
        Move::Taken { seq: None, floor } => Some(floor),
        Move::Taken { seq: Some(seq), .. } => {
            let m = reader.stats().moves();
            (m.found >= seq).then_some(m.at)
        }
    };
    let Some(at) = at else {
        if feed.live() {
            let n = reader.available();
            left_out.leave(reader, n);
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
        return true;
    };
    let ahead = usize::try_from(at.saturating_sub(reader.position())).unwrap_or(usize::MAX);
    let n = ahead.min(reader.available());
    left_out.leave(reader, n);
    if n == ahead {
        let mut moves = crate::feed::lock(moves);
        moves.pop_front();
        feed.moves_done_to(
            feed.moves_asked() - moves.iter().filter(|m| **m != Move::Ordered).count() as u64,
        );
    }
    true
}

/// Leaves out the samples of `reader` that the radio captured while it settled after a change other than a retune: a
/// gain, a rate, another setting, the start of the stream ([`RingStats::unsettled`]). Returns `None` if it left samples
/// out or waits for the radio to settle, else how many samples may pass before the next such stretch. Before the first
/// block (`first`) the samples left out are no gap: the stack has had no sample before them.
fn settle_change(reader: &mut RingReader, feed: &Feed, left_out: &mut LeftOut, first: bool) -> Option<usize> {
    let pos = reader.position();
    let Some(u) = reader.stats().unsettled(pos) else {
        return Some(usize::MAX);
    };
    if pos < u.from {
        return Some(usize::try_from(u.from - pos).unwrap_or(usize::MAX));
    }
    let n = match u.to {
        None if !feed.live() => {
            std::thread::sleep(Duration::from_millis(1));
            return None;
        }
        None => reader.available(),
        Some(to) => usize::try_from(to - pos)
            .unwrap_or(usize::MAX)
            .min(reader.available()),
    };
    if first {
        LeftOut::default().leave(reader, n);
    } else {
        left_out.leave(reader, n);
    }
    None
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
/// A feed that is not live (a recording, a generator) takes no sample before a session or a service reads it, and when its
/// source ends, its sessions take what waits in their queues and end ([`Feed::input_ended`]).
///
/// During a hop ([`crate::hop`]) a step passes the samples of its dwell and no more, then signals the control thread and
/// passes nothing until the radio has moved: a live radio's samples are dropped meanwhile, those of another source wait in
/// the ring.
///
/// At a move of the radio to another centre ([`Move`]), in a hop or not, the samples before the first one that the radio
/// captured at the new centre are left out. The first block after it has the new centre, counts the move
/// ([`crate::feed::Block::moves`]) and starts with a gap made on purpose of all that was left out. The samples that the
/// radio captured while it settled after another change (a gain, a rate, the start of the stream) are left out in the
/// same way, with no move counted.
fn pump(mut reader: RingReader, feed: &Feed, tuning: &Tuning, stop: &AtomicBool, hop: &Steps<'_>) {
    let mut rate_changes = tuning.rate_changes.load(Ordering::Acquire);
    let mut left_out = LeftOut::default();
    // A block has gone to the feed.
    let mut passed = false;
    let live = feed.live();
    while !stop.load(Ordering::Acquire) {
        drop_former_rate(&mut reader, tuning, &mut rate_changes);
        if reader.wait(1, Duration::from_millis(50)) == 0 {
            // The writer can put its last samples in before it goes: the ring is read once more after it is gone.
            if reader.writer_gone() && reader.available() == 0 {
                if !live && !stop.load(Ordering::Acquire) {
                    feed.input_ended();
                }
                return;
            }
            continue;
        }
        // A recording or a generator keeps its samples in the ring until a session or a service reads the feed.
        if !live && !feed.wait_for_reader(tuning.get(), Duration::from_millis(50)) {
            continue;
        }
        if settle_move(&mut reader, feed, hop.moves, &mut left_out) {
            continue;
        }
        let Some(steady) = settle_change(&mut reader, feed, &mut left_out, !passed) else {
            continue;
        };
        let phase = hop.gate.phase();
        let limit = match phase {
            Phase::Off => BLOCK.min(steady),
            Phase::Step { left } => usize::try_from(left).unwrap_or(BLOCK).min(BLOCK).min(steady),
            Phase::Held => {
                if feed.live() {
                    let n = reader.available();
                    left_out.leave(&mut reader, n);
                } else {
                    hop.gate.wait_moved(Duration::from_millis(50));
                }
                continue;
            }
            Phase::Moved { moves, dwell_s } => {
                // The thread is at the first sample of the move: the step starts here.
                if feed.moves_done() >= moves {
                    let rate = tuning.get().0;
                    hop.gate
                        .start_step(moves, dwell_s.map(|d| (d * rate).round() as u64));
                }
                continue;
            }
        };
        let (rate, centre) = tuning.get();
        let mut lost = 0;
        // A live feed counts a session that is behind; another feed waits for it. Neither returns an error.
        let sent = feed.deliver_gaps(rate, centre, false, |iq, gaps| {
            if iq.len() != BLOCK {
                iq.resize(BLOCK, Complex32::default());
            }
            left_out.put(gaps);
            // The gaps are counted from the next read: take those of the samples that this read returns.
            let n = reader.available().min(limit);
            let from = gaps.len();
            gaps.extend(reader.take_gaps(n));
            lost = gaps[from..]
                .iter()
                .filter(|g| !g.deliberate)
                .map(|g| g.lost)
                .sum();
            reader.read_iq(&mut iq[..n])
        });
        passed |= matches!(sent, Ok(n) if n > 0);
        if let (Phase::Step { .. }, Ok(n)) = (phase, sent) {
            // The samples that the radio or its link lost inside the step are signal time of the dwell too.
            if hop.gate.passed(n as u64 + lost) {
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
        let moves = Mutex::default();
        let steps = Steps {
            gate: &gate,
            wake: &wake,
            moves: &moves,
        };
        pump(r, &feed, &tuning, &AtomicBool::new(false), &steps);
        let block = rx.try_recv().unwrap();
        assert_eq!(block.samples().len(), 1500);
        assert_eq!(block.gaps(), [sdrkit_driver_core::Gap::lost_at(1000, 4096)]);
    }
    #[test]
    fn the_samples_before_the_first_one_of_a_move_are_left_out_and_the_block_counts_the_move() {
        let (mut w, r) = ring(1 << 16, sdrkit_graph::stream::SampleType::Cf32);
        let stats = r.stats();
        // 1000 samples of the centre before, then the radio is told to move: 1500 more of them were in flight.
        assert_eq!(w.write_iq(&[Complex32::new(1.0, 0.0); 2500]), 2500);
        let seq = stats.begin_move();
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
        tuning.set(48e3, 101e6);
        let moves = Mutex::new(VecDeque::from([Move::Taken {
            seq: Some(seq),
            floor: 1000,
        }]));
        feed.move_asked();
        let (gate, wake) = (HopGate::default(), Condvar::new());
        gate.hold();
        gate.moved(feed.moves_asked(), None);
        let steps = Steps {
            gate: &gate,
            wake: &wake,
            moves: &moves,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let done = {
            let (feed, stop) = (Arc::clone(&feed), Arc::clone(&stop));
            std::thread::spawn(move || {
                // The driver has not found the first sample: nothing passes.
                std::thread::sleep(Duration::from_millis(100));
                assert_eq!(feed.moves_done(), 0);
                // The driver marks the place, then writes the samples of the new centre.
                stats.end_move(seq, 2500);
                assert_eq!(w.write_iq(&[Complex32::new(2.0, 0.0); 700]), 700);
                drop(w);
                stop
            })
        };
        pump(r, &feed, &tuning, &stop, &steps);
        done.join().unwrap();
        let block = rx.try_recv().unwrap();
        assert_eq!(block.samples(), [Complex32::new(2.0, 0.0); 700]);
        assert_eq!((block.centre_hz, block.moves), (101e6, 1));
        let left_out = sdrkit_driver_core::Gap {
            deliberate: true,
            ..sdrkit_driver_core::Gap::lost_at(0, 2500)
        };
        assert_eq!(block.gaps(), [left_out]);
        assert!(rx.try_recv().is_err());
        assert_eq!(gate.phase(), Phase::Off);
    }

    #[test]
    fn the_samples_that_the_radio_captured_while_it_settled_do_not_reach_the_stack() {
        let (mut w, r) = ring(1 << 16, sdrkit_graph::stream::SampleType::Cf32);
        let stats = r.stats();
        let (low, good, spoiled) = (
            Complex32::new(0.158, 0.0),
            Complex32::new(1.0, 0.0),
            Complex32::new(3.0, 0.0),
        );
        // The stream starts while the held gain searches: its first three blocks are 16 dB low.
        stats.begin_settle();
        for _ in 0..3 {
            assert_eq!(w.write_iq(&[low; 1024]), 1024);
        }
        stats.settled(3072);
        assert_eq!(w.write_iq(&[good; 2000]), 2000);
        // A gain change: 500 samples of the gain before were in flight, then 300 while the radio settled.
        stats.begin_settle();
        stats.settle_from(5572);
        assert_eq!(w.write_iq(&[good; 500]), 500);
        assert_eq!(w.write_iq(&[spoiled; 300]), 300);
        stats.settled(5872);
        assert_eq!(w.write_iq(&[good; 1000]), 1000);
        drop(w);
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
        tuning.set(1e6, 100e6);
        let (gate, wake) = (HopGate::default(), Condvar::new());
        let moves = Mutex::default();
        let steps = Steps {
            gate: &gate,
            wake: &wake,
            moves: &moves,
        };
        pump(r, &feed, &tuning, &AtomicBool::new(false), &steps);
        let (mut samples, mut left_out) = (Vec::new(), Vec::new());
        while let Ok(block) = rx.try_recv() {
            left_out.extend(block.gaps().iter().map(|g| (samples.len(), g.lost, g.deliberate)));
            samples.extend_from_slice(block.samples());
        }
        // The stretch of the start comes before the first block: it is no gap.
        assert_eq!(samples, [good; 3500]);
        assert_eq!(left_out, [(2500, 300, true)]);
        assert_eq!(feed.moves_done(), 0);
    }
}
