//! What the receivers of one radio share: the registry of streams, the receiver slots that carry packets from the stream
//! thread to the read loop of each receiver, the packet pool, the status board, the throughput figures, and what is known per
//! address. Packets are received into pooled buffers and shared with every receiver that takes samples from them. The pool and
//! every queue have a cap.

use crate::plan::{self, Gain};
use crate::profile::{FilterBoard, Model, Profile};
use crate::{
    ACHIEVED, BOARD, BOARD_NAME, BUSY, EXCITER, FORWARD, FREQUENCY_SENT, GAPS, HL2_PA, MAC, N2ADR,
    OVERLOAD, PACKETS, PLL, PPM, PROTOCOL, QUEUE_DROPPED, RADIO_RECEIVERS, RECEIVER, RECEIVERS,
    REVERSE, SEQ_LOST, SETTLE_DROPS, SETTLE_LOST, SUPPLY, SWAP_IQ, SWR, TEMPERATURE, THROUGHPUT,
    TX_TIMEOUT, UNDERRUNS, VERSION,
};
use sdrkit_core::net::{Datagram, DatagramOptions, Transport};
use sdrkit_core::{Complex32, Hz, Results, SettingSet, Values};
use sdrkit_driver_core::runner::Notices;
use sdrkit_driver_core::{
    DriverError, RadioSetting, TxState, TxStats, AGC, CENTRE, GAIN, RATE, RATE_EXACT,
};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

/// Locks `m`, also after a thread panicked while it held the lock.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The largest datagram kept (a protocol 2 packet is 1444 bytes).
pub const PKT_BYTES: usize = 1500;
/// Packets a receiver's queue holds before it drops (about 0.17 s at 384 ksps with one receiver).
pub const QUEUE_CAP: usize = 512;
/// The most packet buffers in the pool of a stream.
pub const POOL_CAP: usize = 2 * QUEUE_CAP;
/// The packet buffers a pool starts with: the packets a receiver can fall behind before the pool grows.
pub const POOL_START: usize = 64;

/// One received datagram.
pub struct Pkt {
    /// Bytes received.
    pub len: usize,
    /// The bytes.
    pub buf: [u8; PKT_BYTES],
}

impl Pkt {
    /// The bytes received.
    pub fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

/// A fixed pool of packet buffers: a buffer is reused once no receiver holds it any more.
pub struct Pool {
    v: Vec<Arc<Pkt>>,
    at: usize,
}

fn fresh() -> Arc<Pkt> {
    Arc::new(Pkt {
        len: 0,
        buf: [0; PKT_BYTES],
    })
}

impl Default for Pool {
    fn default() -> Self {
        let mut v = Vec::with_capacity(POOL_CAP);
        v.extend((0..POOL_START).map(|_| fresh()));
        Pool { v, at: 0 }
    }
}

impl Pool {
    /// The buffer to receive into next: the next one round the pool, or a new one when that is still held by a receiver.
    pub fn buffer(&mut self) -> &mut Pkt {
        if Arc::get_mut(&mut self.v[self.at]).is_none() {
            if self.v.len() < POOL_CAP {
                self.v.push(fresh());
                self.at = self.v.len() - 1;
            } else {
                // Every buffer is still held by a slow receiver: replace this one (the receiver keeps its copy).
                self.v[self.at] = fresh();
            }
        }
        Arc::get_mut(&mut self.v[self.at]).expect("an unshared buffer")
    }

    /// The buffer just received into, to share. The next [`Pool::buffer`] moves on.
    pub fn take(&mut self) -> Arc<Pkt> {
        let p = self.v[self.at].clone();
        self.at = (self.at + 1) % self.v.len();
        p
    }
}

/// One packet for a receiver.
pub struct Item {
    /// The packet.
    pub pkt: Arc<Pkt>,
    /// Protocol 1: receivers in the layout of the packet. Protocol 2: 0.
    pub layout: u8,
}

/// What a receiver's read loop got.
pub enum Popped {
    /// A packet.
    Item(Item),
    /// Nothing within the wait.
    Idle,
    /// The stream is lost, for this reason.
    Lost(DriverError),
    /// The receiver left the stream.
    Closed,
}

#[derive(Default)]
struct SlotQ {
    items: VecDeque<Item>,
    lost: Option<DriverError>,
    closed: bool,
    active: bool,
    had: bool,
}

/// A receiver's counters, shown in its readings.
#[derive(Debug, Default)]
pub struct RxCounters {
    /// Samples lost to sequence gaps.
    pub seq_lost: AtomicU64,
    /// Samples lost to packets dropped while the layout settled.
    pub settle_lost: AtomicU64,
    /// Samples dropped because the receiver's queue was full.
    pub queue_dropped: AtomicU64,
    /// Samples offered to the ring.
    pub offered: AtomicU64,
}

/// One receiver of a stream: the queue from the stream thread to its read loop, its sample order, and its counters.
pub struct RxSlot {
    /// The receiver number, from 1.
    pub n: usize,
    /// Where its log lines go.
    pub notices: Notices,
    /// Take the sample words the other way round.
    pub swap: AtomicBool,
    /// The factor on the sample words ([`plan::Gain::Words`]), [`plan::UNITY`] for none.
    pub factor: AtomicU32,
    /// Its counters.
    pub counters: RxCounters,
    q: Mutex<SlotQ>,
    cv: Condvar,
}

impl RxSlot {
    /// A slot for receiver `n`.
    pub fn new(n: usize, notices: Notices, p: &RxParams) -> Arc<RxSlot> {
        let s = Arc::new(RxSlot {
            n,
            notices,
            swap: AtomicBool::new(p.swap),
            factor: AtomicU32::new(plan::UNITY),
            counters: RxCounters::default(),
            q: Mutex::new(SlotQ {
                items: VecDeque::with_capacity(QUEUE_CAP),
                ..Default::default()
            }),
            cv: Condvar::new(),
        });
        s.set(p);
        s
    }

    /// Takes the sample order and the word factor of the receiver.
    pub fn set(&self, p: &RxParams) {
        self.swap.store(p.swap, Ordering::Relaxed);
        let factor = match p.gain {
            Gain::Words(f) => f,
            _ => plan::UNITY,
        };
        self.factor.store(factor, Ordering::Relaxed);
    }

    /// Queues a packet (stream thread). Returns false when the receiver is not reading (not started, or gone). A full queue drops the
    /// packet and counts its samples.
    pub fn push(&self, pkt: &Arc<Pkt>, layout: u8, samples: usize) -> bool {
        let mut q = lock(&self.q);
        if !q.active || q.closed {
            return false;
        }
        q.had = true;
        if q.items.len() >= QUEUE_CAP {
            self.counters
                .queue_dropped
                .fetch_add(samples as u64, Ordering::Relaxed);
            return true;
        }
        q.items.push_back(Item {
            pkt: pkt.clone(),
            layout,
        });
        drop(q);
        self.cv.notify_one();
        true
    }

    /// Counts samples lost before the next packet, only for a receiver that reads and has had samples already. Returns whether
    /// they were counted.
    pub fn add_loss(&self, samples: u64, settling: bool) -> bool {
        let q = lock(&self.q);
        let counted = q.had && q.active;
        drop(q);
        if counted {
            let c = if settling {
                &self.counters.settle_lost
            } else {
                &self.counters.seq_lost
            };
            c.fetch_add(samples, Ordering::Relaxed);
        }
        counted
    }

    /// The stream is lost for `why`: wake the read loop.
    pub fn lost(&self, why: &DriverError) {
        let mut q = lock(&self.q);
        if q.lost.is_none() {
            q.lost = Some(why.clone());
        }
        q.items.clear();
        drop(q);
        self.cv.notify_all();
    }

    /// The receiver left: wake the read loop.
    pub fn close(&self) {
        let mut q = lock(&self.q);
        q.closed = true;
        q.items.clear();
        drop(q);
        self.cv.notify_all();
    }

    /// The read loop starts (true) or ends (false) taking packets.
    pub fn activate(&self, on: bool) {
        let mut q = lock(&self.q);
        q.active = on;
        if !on {
            q.items.clear();
        }
    }

    /// Waits up to `wait` for a packet.
    pub fn pop(&self, wait: Duration) -> Popped {
        let mut q = lock(&self.q);
        let end = Instant::now() + wait;
        loop {
            if q.closed {
                return Popped::Closed;
            }
            if let Some(why) = &q.lost {
                return Popped::Lost(why.clone());
            }
            if let Some(it) = q.items.pop_front() {
                return Popped::Item(it);
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Popped::Idle;
            }
            q = self
                .cv
                .wait_timeout(q, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// What one receiver requires from the radio, from its settings.
#[derive(Clone, Debug, PartialEq)]
pub struct RxParams {
    /// The frequency sent, Hz.
    pub freq: u32,
    /// The requested rate, 0 for any.
    pub rate_asked: Hz,
    /// Accept only an offered rate equal to the requested rate.
    pub rate_exact: bool,
    /// The rate given for it.
    pub rate: u32,
    /// Swap I and Q.
    pub swap: bool,
    /// The N2ADR filter board.
    pub n2adr: bool,
    /// The gain.
    pub gain: Gain,
    /// The transmit time limit, s.
    pub tx_timeout: f64,
    /// The Hermes-Lite 2 power amplifier.
    pub hl2pa: bool,
    /// The clock correction, ppm.
    pub ppm: f64,
}

impl RxParams {
    /// What `values` ask of a radio of the profile `p` with the board `board`, if known.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] for a rate, frequency or gain the radio cannot take.
    pub fn from_values(
        p: &Profile,
        board: Option<u8>,
        values: &Values,
    ) -> Result<RxParams, DriverError> {
        let ppm = values.get(&PPM);
        let rate_asked = values.get(&RATE);
        let rate_exact = values.get(&RATE_EXACT);
        let hl2 = p.model == Model::HermesLite2;
        let manual = (!values.get(&AGC)).then(|| values.get(&GAIN));
        Ok(RxParams {
            freq: plan::freq_sent(values.get(&CENTRE), ppm).ok_or(DriverError::Refused {
                id: RadioSetting::Centre.id(),
            })?,
            rate_asked,
            rate_exact,
            rate: plan::choose_rate(rate_asked, p.rates(board), rate_exact)?,
            swap: values.get(&SWAP_IQ),
            n2adr: hl2 && p.filter == FilterBoard::N2adr && values.get(&N2ADR),
            gain: plan::gain(p, p.has_gain(board), manual)?,
            tx_timeout: values.get(&TX_TIMEOUT),
            hl2pa: hl2 && values.get(&HL2_PA),
            ppm,
        })
    }
}

/// What discovery told about a radio.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// Board id.
    pub board: Option<u8>,
    /// The MAC address.
    pub mac: Option<[u8; 6]>,
    /// Gateware or firmware version text.
    pub version: String,
    /// Protocol version supported (protocol 2), times ten.
    pub protocol: Option<u8>,
    /// Receivers it said it has.
    pub receivers: Option<u8>,
    /// It was busy at discovery.
    pub busy: bool,
}

/// The measurements of the radio.
#[derive(Clone, Debug, Default)]
pub struct Meter {
    /// The last ADC overload.
    pub overload_at: Option<Instant>,
    /// Hermes-Lite 2 temperature reading.
    pub temp: Option<u16>,
    /// Exciter power.
    pub exciter: Option<u16>,
    /// Forward power.
    pub fwd: Option<u16>,
    /// Reverse power.
    pub rev: Option<u16>,
    /// AIN3 (Hermes-Lite 2: PA current).
    pub ain3: Option<u16>,
    /// AIN4.
    pub ain4: Option<u16>,
    /// Supply voltage.
    pub supply: Option<u16>,
    /// PTT input.
    pub ptt: bool,
    /// Dot.
    pub dot: bool,
    /// Dash.
    pub dash: bool,
    /// PLL locked (protocol 2).
    pub pll: Option<bool>,
    /// Firmware versions (Mercury, Penelope, Metis/Hermes).
    pub versions: Option<[u8; 3]>,
}

impl Meter {
    /// ADC overload in the last 500 ms.
    pub fn overload(&self) -> bool {
        self.overload_at
            .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
    }
}

/// Throughput over windows of at least 1 s.
#[derive(Debug)]
pub struct Thru {
    start: Instant,
    bytes: u64,
    packets: u64,
    gaps: u64,
    settle: u64,
    per_rx: Vec<u64>,
    /// The last complete window.
    pub last: Option<ThruFig>,
}

/// One window's figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThruFig {
    /// Mbit/s of sample packets.
    pub mbps: f64,
    /// Packets per second.
    pub pps: f64,
    /// Per receiver (index n - 1): samples written plus counted lost, per second.
    pub achieved: Vec<f64>,
    /// Sequence gaps.
    pub gaps: u64,
    /// Packets dropped while settling.
    pub settle: u64,
}

impl Thru {
    /// Figures for up to `receivers` receivers.
    pub fn new(receivers: usize) -> Thru {
        Thru {
            start: Instant::now(),
            bytes: 0,
            packets: 0,
            gaps: 0,
            settle: 0,
            per_rx: vec![0; receivers],
            last: None,
        }
    }
    /// A sample packet arrived.
    pub fn packet(&mut self, bytes: usize) {
        self.bytes += bytes as u64;
        self.packets += 1;
        self.roll();
    }
    /// Receiver `n` got (or lost) samples.
    pub fn samples(&mut self, n: usize, k: u64) {
        if let Some(v) = self.per_rx.get_mut(n - 1) {
            *v += k;
        }
    }
    /// A sequence gap.
    pub fn gap(&mut self) {
        self.gaps += 1;
    }
    /// A packet dropped while settling.
    pub fn settled(&mut self) {
        self.settle += 1;
    }
    /// A rate or receiver-count change starts a new window.
    pub fn restart(&mut self) {
        self.start = Instant::now();
        self.bytes = 0;
        self.packets = 0;
        self.gaps = 0;
        self.settle = 0;
        self.per_rx.iter_mut().for_each(|v| *v = 0);
    }
    fn roll(&mut self) {
        let t = self.start.elapsed().as_secs_f64();
        if t < 1.0 {
            return;
        }
        let mut fig = self.last.take().unwrap_or_default();
        fig.mbps = self.bytes as f64 * 8.0 / t / 1e6;
        fig.pps = self.packets as f64 / t;
        fig.achieved.clear();
        fig.achieved
            .extend(self.per_rx.iter().map(|&v| v as f64 / t));
        fig.gaps = self.gaps;
        fig.settle = self.settle;
        self.last = Some(fig);
        self.restart();
    }
}

/// The status board of one stream: discovery facts, measurements, throughput.
#[derive(Debug)]
pub struct Board {
    /// From discovery.
    pub facts: Facts,
    /// Measurements.
    pub meter: Meter,
    /// Throughput.
    pub thru: Thru,
    /// Transmit underrun slots.
    pub underruns: u64,
}

/// Writes the readings of receiver `slot` into `r`: the board of the stream, the rate-independent counts, and `receivers`
/// (in the stream, or DDCs enabled) and `freq` (the frequency sent).
pub fn write_readings(
    p: &Profile,
    board: &Board,
    slot: &RxSlot,
    receivers: usize,
    freq: u32,
    r: &mut Results,
) {
    let mut f = r.fields_mut();
    let facts = &board.facts;
    let count = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
    f.opt(&BOARD, facts.board.map(i64::from));
    f.opt(
        &BOARD_NAME,
        facts
            .board
            .map(|b| crate::profile::board_name(p.protocol, b)),
    );
    f.opt(&MAC, facts.mac.as_ref().map(crate::p1::mac_text).as_deref());
    f.opt(
        &VERSION,
        Some(facts.version.as_str()).filter(|v| !v.is_empty()),
    );
    let pv = facts.protocol.map(|pv| format!("{}.{}", pv / 10, pv % 10));
    f.opt(&PROTOCOL, pv.as_deref());
    f.opt(&RADIO_RECEIVERS, facts.receivers.map(i64::from));
    f.set(&BUSY, facts.busy);
    f.set(&RECEIVERS, count(receivers as u64));
    f.set(&RECEIVER, count(slot.n as u64));
    f.set(&FREQUENCY_SENT, f64::from(freq));
    let m = &board.meter;
    f.set(&OVERLOAD, m.overload());
    f.opt(&PLL, m.pll);
    let temp = m
        .temp
        .filter(|_| p.model == Model::HermesLite2)
        .map(plan::hl2_temperature);
    f.opt(&TEMPERATURE, temp);
    f.opt(&EXCITER, m.exciter.map(i64::from));
    f.opt(&FORWARD, m.fwd.map(i64::from));
    f.opt(&REVERSE, m.rev.map(i64::from));
    f.opt(&SUPPLY, m.supply.map(i64::from));
    let swr = m.fwd.zip(m.rev).and_then(|(fw, rv)| plan::swr(fw, rv));
    f.opt(&SWR, swr);
    let c = &slot.counters;
    f.set(&SEQ_LOST, count(c.seq_lost.load(Ordering::Relaxed)));
    f.set(&SETTLE_LOST, count(c.settle_lost.load(Ordering::Relaxed)));
    f.set(
        &QUEUE_DROPPED,
        count(c.queue_dropped.load(Ordering::Relaxed)),
    );
    f.set(&UNDERRUNS, count(board.underruns));
    let t = board.thru.last.as_ref();
    f.opt(&THROUGHPUT, t.map(|t| t.mbps));
    f.opt(&PACKETS, t.map(|t| t.pps));
    f.opt(
        &ACHIEVED,
        t.and_then(|t| t.achieved.get(slot.n - 1).copied()),
    );
    f.opt(&GAPS, t.map(|t| count(t.gaps)));
    let settle = t.filter(|_| p.protocol == 1).map(|t| count(t.settle));
    f.opt(&SETTLE_DROPS, settle);
}

/// What a transmitter shares with the stream thread: its sample queue and state.
pub struct TxQueue {
    inner: Mutex<TxQ>,
}

struct TxQ {
    buf: VecDeque<Complex32>,
    cyclic: Vec<Complex32>,
    cyc_at: usize,
    state: TxState,
    flushed: bool,
    starved_since: Option<Instant>,
    sent: u64,
    buffers: u64,
    padded: u64,
    underruns: u64,
    underrun_samples: u64,
    transmitting_since: Option<Instant>,
    notices: Vec<String>,
}

/// The transmit queue's size: half a second at 192 ksps.
pub const TX_QUEUE: usize = 96_000;

impl Default for TxQueue {
    fn default() -> Self {
        TxQueue {
            inner: Mutex::new(TxQ {
                buf: VecDeque::with_capacity(TX_QUEUE),
                cyclic: Vec::new(),
                cyc_at: 0,
                state: TxState::Disarmed,
                flushed: false,
                starved_since: None,
                sent: 0,
                buffers: 0,
                padded: 0,
                underruns: 0,
                underrun_samples: 0,
                transmitting_since: None,
                notices: Vec::new(),
            }),
        }
    }
}

impl TxQueue {
    /// Where the transmitter is.
    pub fn state(&self) -> TxState {
        lock(&self.inner).state
    }
    /// Sets the state (on air: from now).
    pub fn set_state(&self, s: TxState) {
        let mut q = lock(&self.inner);
        if q.state == TxState::Closed {
            return;
        }
        if s.transmitting() && !q.state.transmitting() {
            q.transmitting_since = Some(Instant::now());
            q.starved_since = None;
        }
        if !s.transmitting() {
            q.transmitting_since = None;
            q.buf.clear();
            q.cyclic.clear();
            q.flushed = false;
        }
        q.state = s;
    }
    /// A note for the transmitter's log.
    pub fn note(&self, line: String) {
        let mut q = lock(&self.inner);
        if q.notices.len() < 64 {
            q.notices.push(line);
        }
    }
    /// Takes the notes.
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.inner).notices)
    }
    /// Queues samples and returns how many fit.
    pub fn push(&self, iq: &[Complex32]) -> usize {
        let mut q = lock(&self.inner);
        let n = iq.len().min(TX_QUEUE - q.buf.len());
        q.buf.extend(iq[..n].iter().map(|s| {
            let m = s.norm();
            if m > 1.0 {
                s / m
            } else {
                *s
            }
        }));
        q.flushed = false;
        n
    }
    /// Ends a burst.
    pub fn flush(&self) {
        lock(&self.inner).flushed = true;
    }
    /// Repeats `iq` until disarmed.
    pub fn cyclic(&self, iq: &[Complex32]) {
        let mut q = lock(&self.inner);
        q.buf.clear();
        q.cyclic.clear();
        q.cyclic.extend_from_slice(iq);
        q.cyc_at = 0;
        q.state = TxState::Cyclic;
    }
    /// Fills `out` with the next samples, zeros where there are none (counted as underrun unless the burst was flushed). Returns how
    /// long the queue has been empty.
    pub fn pull(&self, out: &mut [Complex32]) -> Duration {
        let mut q = lock(&self.inner);
        let mut got = 0;
        if q.state == TxState::Cyclic && !q.cyclic.is_empty() {
            for o in out.iter_mut() {
                *o = q.cyclic[q.cyc_at];
                q.cyc_at = (q.cyc_at + 1) % q.cyclic.len();
            }
            got = out.len();
        } else {
            for o in out.iter_mut() {
                match q.buf.pop_front() {
                    Some(s) => {
                        *o = s;
                        got += 1;
                    }
                    None => *o = Complex32::new(0.0, 0.0),
                }
            }
        }
        q.sent += out.len() as u64;
        q.buffers += 1;
        let missing = (out.len() - got) as u64;
        if missing > 0 {
            if q.flushed {
                q.padded += missing;
            } else {
                if got > 0 || q.starved_since.is_none() {
                    q.underruns += 1;
                }
                q.underrun_samples += missing;
            }
        }
        if got == 0 {
            let since = *q.starved_since.get_or_insert_with(Instant::now);
            since.elapsed()
        } else {
            q.starved_since = None;
            Duration::ZERO
        }
    }
    /// The counters.
    pub fn stats(&self) -> TxStats {
        let q = lock(&self.inner);
        TxStats {
            sent: q.sent,
            buffers: q.buffers,
            queued: q.buf.len(),
            padded: q.padded,
            underruns: q.underruns,
            underrun_samples: q.underrun_samples,
            transmitting_since: q.transmitting_since,
        }
    }
}

/// Why a join did not take.
pub enum JoinError {
    /// The stream is lost or stopping: make a new one.
    Unusable,
    /// The join failed.
    Failed(DriverError),
}

/// What a receiver needs from its radio's stream, either protocol.
pub trait Stream: Send + Sync {
    /// Joins receiver `n` to the running stream.
    fn join(
        &self,
        n: usize,
        p: &RxParams,
        notices: &Notices,
    ) -> Result<(Arc<RxSlot>, String), JoinError>;
    /// Applies the new settings of receiver `n`, and returns the notes.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] for a rate the board does not offer.
    fn update(&self, n: usize, p: &RxParams) -> Result<String, DriverError>;
    /// Receiver `n` leaves. The last one stops the stream.
    fn leave(&self, n: usize);
    /// The rate receiver `n` runs at.
    fn rate(&self, n: usize) -> u32;
    /// The stream was lost.
    fn lost(&self) -> bool;
    /// What discovery said.
    fn facts(&self) -> Facts;
    /// Writes the readings of receiver `n` into `r`.
    fn write_readings(&self, n: usize, r: &mut Results);
    /// A status line for receiver `n`.
    fn status(&self, n: usize) -> String;
    /// Claims the transmitter of the radio for `q`. A radio has one.
    ///
    /// # Errors
    ///
    /// [`HpsdrError::TransmitterTaken`](crate::HpsdrError::TransmitterTaken) if another transmitter is open.
    fn claim(&self, q: &Arc<TxQueue>) -> Result<(), DriverError>;
    /// Starts transmitting from `q` at `freq` (Hz sent) with `drive` (0 to 255).
    ///
    /// # Errors
    ///
    /// [`DriverError::NotNow`] if the stream stopped,
    /// [`HpsdrError::TransmitterTaken`](crate::HpsdrError::TransmitterTaken) if another transmitter is on air.
    fn tx_arm(&self, q: &Arc<TxQueue>, freq: u32, drive: u8) -> Result<(), DriverError>;
    /// Changes the carrier and drive while transmitting.
    fn tx_set(&self, freq: u32, drive: u8);
    /// Ends the transmission (MOX or PTT cleared at once).
    fn tx_end(&self, why: &str);
    /// The clock correction in effect, ppm.
    fn ppm(&self) -> f64;
}

/// What is known per `host:port` while the program runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Known {
    /// The receiver count the radio said.
    pub receivers: Option<u8>,
    /// The board id it answered with.
    pub board: Option<u8>,
}

/// Where a running stream of one profile and address is found.
pub type Cell = Arc<Mutex<Option<Weak<dyn Stream>>>>;

/// The streams of a driver list, and what is remembered per address. Shared by the four HPSDR drivers.
#[derive(Default)]
pub struct Hub {
    cells: Mutex<HashMap<String, Cell>>,
    known: Mutex<HashMap<String, Known>>,
}

impl Hub {
    /// What is remembered for `host:port`.
    pub fn known(&self, hostport: &str) -> Known {
        lock(&self.known).get(hostport).copied().unwrap_or_default()
    }

    /// Remembers a radio's answer.
    pub fn remember(&self, hostport: &str, board: u8, receivers: Option<u8>) {
        let mut k = lock(&self.known);
        let e = k.entry(hostport.to_string()).or_default();
        e.board = Some(board);
        if receivers.is_some() {
            e.receivers = receivers;
        }
    }

    /// The cell of a profile and address: holds the stream while it runs.
    pub fn cell(&self, p: &Profile, hostport: &str) -> Cell {
        lock(&self.cells)
            .entry(format!("{}|{hostport}", p.key))
            .or_default()
            .clone()
    }

    /// Streams still alive. Every socket is closed when this is 0.
    #[cfg(test)]
    pub fn live_streams(&self) -> usize {
        lock(&self.cells)
            .values()
            .filter(|c| lock(c).as_ref().is_some_and(|w| w.strong_count() > 0))
            .count()
    }
}

/// The receive timeout of a stream socket: a stream thread notices a stop within about this long.
pub const RECV_TIMEOUT: Duration = Duration::from_millis(100);
/// The receive buffer size requested for a stream socket.
pub const RECV_BUFFER: usize = 4 << 20;

/// A datagram socket on any local address, with a 4 MiB receive buffer and the receive timeout.
///
/// # Errors
///
/// [`DriverError::Net`] if the transport provides no socket.
pub fn socket(net: &dyn Transport) -> Result<Box<dyn Datagram>, DriverError> {
    let mut s = net.bind(&DatagramOptions {
        recv_buffer: Some(RECV_BUFFER),
        ..DatagramOptions::any_v4()
    })?;
    s.set_read_timeout(Some(RECV_TIMEOUT))?;
    Ok(s)
}

/// True if `owner` is a transmitter other than `q` that is not closed.
pub fn other_owner(owner: Option<&Arc<TxQueue>>, q: &Arc<TxQueue>) -> bool {
    owner.is_some_and(|o| !Arc::ptr_eq(o, q) && o.state() != TxState::Closed)
}
