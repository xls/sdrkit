//! The transmitter of a HackRF: half duplex on the one antenna port. Arming stops receive and goes through mode off;
//! the samples go to the device as signed 8-bit I/Q in whole blocks from a writer thread that keeps a few bulk
//! transfers in flight; ending a burst follows the flush rule of the firmware; disarming cancels the transfers, goes
//! through mode off and starts receive again where it ran.

use crate::ctl::Ctl;
use crate::error::HackrfError;
use crate::link::{lock, Dir, Link};
use crate::proto::{self, Mode, BLOCK};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Results, Setting, SettingSet, Values};
use sdrkit_driver_core::{
    tx_schema, Doing, DriverError, Transmitter, TxCapabilities, TxLimits, TxSetting, TxState,
    TxStats, TX_BANDWIDTH, TX_FREQUENCY, TX_POWER, TX_RATE,
};
use sdrkit_driver_usb::{BulkOut, UsbError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The RF amplifier while transmitting.
pub const TX_AMP: Setting<field::Bool> = Setting::new("tx_amp", "Transmit amplifier", "", false)
    .help(
        "The front-end amplifier while transmitting, about 11 dB. Off, the output is about 11 dB below the typical figures \
         of the board.",
    );

sdrkit_core::settings! {
    /// The own transmit settings of the HackRF driver.
    pub enum HackrfTxSetting {
        /// [`TX_AMP`].
        Amp = TX_AMP,
    }
}

/// The transmit settings, read from the values of a transmitter.
#[derive(Clone, Debug, PartialEq)]
struct TxConfig {
    freq: u64,
    rate: f64,
    filter: u32,
    tx_vga: u32,
    amp: bool,
}

impl TxConfig {
    fn from_values(v: &Values, limits: &TxLimits, rx_rate: f64) -> Result<TxConfig, DriverError> {
        let refused = |s: TxSetting| DriverError::Refused { id: s.id() };
        let freq = v.get(&TX_FREQUENCY);
        if freq < limits.min_freq_hz || freq > limits.max_freq_hz {
            return Err(refused(TxSetting::Frequency));
        }
        let rate = match v.get(&TX_RATE) {
            r if r > 0.0 => r,
            _ => rx_rate,
        };
        if rate < limits.rates.min_hz - 0.5 || rate > limits.rates.max_hz + 0.5 {
            return Err(refused(TxSetting::Rate));
        }
        let filter = match v.get(&TX_BANDWIDTH) {
            w if w > 0.0 => proto::filter_at_least(w),
            _ => proto::auto_filter(rate),
        };
        let power = v.get(&TX_POWER);
        let tx_vga = (f64::from(proto::TX_VGA_MAX) + power)
            .round()
            .clamp(0.0, f64::from(proto::TX_VGA_MAX)) as u32;
        Ok(TxConfig {
            freq: freq.round() as u64,
            rate,
            filter,
            tx_vga,
            amp: v.get(&TX_AMP),
        })
    }
}

/// What the transmitter and its writer thread share.
#[derive(Default)]
pub(crate) struct TxShared {
    stop: AtomicBool,
    flush: AtomicBool,
    busy: AtomicBool,
    started: AtomicBool,
    /// Set by the drain to ask for the flush zeros; the writer clears it once they are sent.
    zeros: AtomicBool,
    closed: AtomicBool,
    error: Mutex<Option<HackrfError>>,
    stats: Mutex<TxStats>,
}

impl TxShared {
    /// Stops the writer for good: the source stops.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.stop.store(true, Ordering::Release);
    }

    /// The transmitter was closed.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn fail(&self, e: HackrfError) {
        lock(&self.error).get_or_insert(e);
    }
}

/// The play-out of the device as the host sees it: bytes written since the first write of a burst against the time at
/// the rate. Data that arrives after the device ran dry is an underrun.
struct Playout {
    bytes_per_s: f64,
    start: Option<Instant>,
    written: f64,
}

impl Playout {
    /// The samples the device had to fill with zeros before a write at `now`, if it ran dry.
    fn before_write(&mut self, now: Instant) -> Option<u64> {
        let s = self.start?;
        let ahead = self.written - now.duration_since(s).as_secs_f64() * self.bytes_per_s;
        if ahead >= 0.0 {
            return None;
        }
        self.start = Some(now);
        self.written = 0.0;
        Some((-ahead / 2.0) as u64)
    }

    fn after_write(&mut self, bytes: usize, now: Instant) {
        self.start.get_or_insert(now);
        self.written += bytes as f64;
    }

    fn end_burst(&mut self) {
        self.start = None;
        self.written = 0.0;
    }
}

/// The longest single wait for a transfer result, so that a stop is seen quickly.
const RESULT_WAIT: Duration = Duration::from_millis(5);

/// What the writer thread needs. The writer owns the bulk OUT queue: only it submits, takes results and cancels.
struct Writer {
    link: Arc<Link>,
    sh: Arc<TxShared>,
    cons: rtrb::Consumer<u8>,
    out: Box<dyn BulkOut>,
    size: usize,
    rate: f64,
    armed_at: Instant,
    /// The bytes of the flush zeros that end a burst.
    zero_bytes: usize,
    /// The last result taken, or the submit that found the queue empty: the start of the write timeout.
    last_done: Instant,
}

impl Writer {
    /// Streams until stopped, then cancels the transfers in flight. After an error it also sends mode off, so no
    /// carrier goes out until the transmitter is disarmed.
    fn run(mut self) {
        let r = self.stream();
        self.out.cancel();
        self.sh.busy.store(false, Ordering::Release);
        if let Err(e) = r {
            let link = &self.link;
            link.note(&e);
            if !link.gone() {
                let _ = link.lock_ctl().set_mode(Mode::Off);
            }
            self.sh.fail(e);
        }
    }

    /// Submits whole transfers while the ring holds one, and the rest padded to whole blocks when a flush asks for it;
    /// sends the flush zeros when the drain asks for them. Mode transmit goes before the first submit. Returns when
    /// stopped.
    fn stream(&mut self) -> Result<(), HackrfError> {
        let link = self.link.clone();
        let device_counts = link.facts.api >= 0x0106 && link.timing.state_poll.is_some();
        let mut play = Playout {
            bytes_per_s: 2.0 * self.rate,
            start: None,
            written: 0.0,
        };
        let mut next_poll = Instant::now();
        let mut shortfalls: Option<u32> = None;
        loop {
            if self.stopped() {
                return Ok(());
            }
            if link.gone() {
                return Err(HackrfError::Usb(UsbError::Unplugged));
            }
            if device_counts && Instant::now() >= next_poll {
                if let Some(every) = link.timing.state_poll {
                    next_poll = Instant::now() + every;
                    self.poll_state(&mut shortfalls);
                }
            }
            let avail = self.cons.slots();
            let flushing = self.sh.flush.load(Ordering::Acquire);
            let n = if avail >= self.size {
                self.size
            } else if flushing {
                avail
            } else {
                0
            };
            if n == 0 {
                if flushing {
                    self.sh.flush.store(false, Ordering::Release);
                    play.end_burst();
                }
                self.reap(false)?;
                if self.sh.zeros.load(Ordering::Acquire) {
                    self.send_zeros()?;
                    continue;
                }
                let idle = self.out.in_flight() == 0;
                self.sh.busy.store(!idle, Ordering::Release);
                std::thread::park_timeout(if idle {
                    Duration::from_millis(2)
                } else {
                    Duration::from_micros(500)
                });
                continue;
            }
            self.sh.busy.store(true, Ordering::Release);
            if !self.wait_free()? || !self.submit(n, &mut play, device_counts)? {
                return Ok(());
            }
        }
    }

    /// True once the transmitter asked the writer to stop.
    fn stopped(&self) -> bool {
        self.sh.stop.load(Ordering::Acquire)
    }

    /// Waits until the queue has a free buffer. Returns false when stopped meanwhile.
    fn wait_free(&mut self) -> Result<bool, HackrfError> {
        while self.out.buffer().is_none() {
            if self.stopped() {
                return Ok(false);
            }
            self.reap(true)?;
        }
        Ok(true)
    }

    /// Takes the results of the transfers that are done; with `wait` true it waits for the oldest first. A transfer
    /// that gives no result within the write timeout is a [`UsbError::Timeout`].
    fn reap(&mut self, wait: bool) -> Result<(), HackrfError> {
        let mut need = wait;
        while self.out.in_flight() > 0 && !self.stopped() {
            match self
                .out
                .completed(if need { RESULT_WAIT } else { Duration::ZERO })
            {
                None => break,
                Some(Ok(_)) => {
                    self.last_done = Instant::now();
                    need = false;
                }
                Some(Err(UsbError::Timeout)) => {
                    if self.last_done.elapsed() > self.link.timing.write_timeout {
                        return Err(HackrfError::Usb(UsbError::Timeout));
                    }
                    if !need {
                        break;
                    }
                }
                Some(Err(e)) => return Err(e.into()),
            }
        }
        Ok(())
    }

    /// Submits the free buffer with its first `len` bytes; the queue was empty if none are in flight.
    fn submit_buffer(&mut self, len: usize) -> Result<(), HackrfError> {
        if self.out.in_flight() == 0 {
            self.last_done = Instant::now();
        }
        Ok(self.out.submit(len)?)
    }

    /// Fills the free buffer with `n` bytes of the ring, pads it to whole blocks and submits it; mode transmit before
    /// the first. Returns false, and sends nothing, when stopped while it waited for the control side.
    fn submit(
        &mut self,
        n: usize,
        play: &mut Playout,
        device_counts: bool,
    ) -> Result<bool, HackrfError> {
        let padded = n.div_ceil(BLOCK) * BLOCK;
        let Some(buf) = self.out.buffer() else {
            return Ok(true);
        };
        if let Ok(chunk) = self.cons.read_chunk(n) {
            let (a, b) = chunk.as_slices();
            buf[..a.len()].copy_from_slice(a);
            buf[a.len()..a.len() + b.len()].copy_from_slice(b);
            chunk.commit_all();
        }
        buf[n..padded].fill(0);
        let first = !self.sh.started.load(Ordering::Acquire);
        if first && !self.start()? {
            return Ok(false);
        }
        if !device_counts {
            if let Some(lost) = play.before_write(Instant::now()) {
                let mut st = lock(&self.sh.stats);
                st.underruns += 1;
                st.underrun_samples += lost;
            }
        }
        self.submit_buffer(padded)?;
        let now = Instant::now();
        play.after_write(padded, now);
        if first {
            lock(&self.link.stats).rx_to_tx = Some(now.duration_since(self.armed_at));
        }
        let mut st = lock(&self.sh.stats);
        st.sent += (padded / 2) as u64;
        st.buffers += 1;
        st.padded += ((padded - n) / 2) as u64;
        Ok(true)
    }

    /// Sends mode transmit, after the other callers of the control side. Returns false when the transmitter or the
    /// link was stopped meanwhile.
    fn start(&mut self) -> Result<bool, HackrfError> {
        let link = &self.link;
        while link.ctl_waiters.load(Ordering::Acquire) > 0 && !self.stopped() {
            std::thread::sleep(Duration::from_micros(200));
        }
        let mut ctl = lock(&link.ctl);
        if self.stopped() || link.gone() {
            return Ok(false);
        }
        match ctl.set_mode(Mode::Transmit) {
            Err(HackrfError::Usb(UsbError::Stall)) => return Err(HackrfError::TransmitDisabled),
            r => r?,
        }
        self.sh.started.store(true, Ordering::Release);
        Ok(true)
    }

    /// Sends the flush zeros behind the last burst, over as many buffers as they need, and waits until every transfer
    /// is done. Clears the request of the drain when they all went out.
    fn send_zeros(&mut self) -> Result<(), HackrfError> {
        let mut left = self.zero_bytes;
        while left > 0 {
            if !self.wait_free()? {
                return Ok(());
            }
            let Some(buf) = self.out.buffer() else {
                continue;
            };
            let n = left.min(buf.len());
            buf[..n].fill(0);
            self.submit_buffer(n)?;
            left -= n;
        }
        while self.out.in_flight() > 0 {
            if self.stopped() {
                return Ok(());
            }
            self.reap(true)?;
        }
        lock(&self.sh.stats).padded += (self.zero_bytes / 2) as u64;
        self.sh.zeros.store(false, Ordering::Release);
        Ok(())
    }

    /// Reads the streaming state and counts new underruns.
    fn poll_state(&mut self, before: &mut Option<u32>) {
        let Ok(mut ctl) = self.link.ctl.try_lock() else {
            return;
        };
        let Ok(s) = ctl.stream_state() else { return };
        drop(ctl);
        if let Some(b) = *before {
            let grown = s.shortfalls.wrapping_sub(b);
            if grown > 0 && grown < 1 << 30 {
                let mut st = lock(&self.sh.stats);
                st.underruns += u64::from(grown);
                st.underrun_samples += u64::from(grown) * u64::from(s.longest / 2);
                lock(&self.link.stats).shortfalls += u64::from(grown);
            }
        }
        *before = Some(s.shortfalls);
    }
}

/// The transmitter of a HackRF.
pub(crate) struct HackrfTransmitter {
    link: Arc<Link>,
    caps: TxCapabilities,
    cfg: TxConfig,
    state: TxState,
    sh: Arc<TxShared>,
    prod: Option<rtrb::Producer<u8>>,
    capacity: usize,
    writer: Option<JoinHandle<()>>,
    since: Option<Instant>,
    zero_bytes: usize,
    lines: Vec<String>,
}

/// The longest flush write: a buffer size reply above it is taken as this.
const MAX_FLUSH: usize = 1 << 20;

/// How many transfers the sample ring holds.
const RING_TRANSFERS: usize = 2;

/// How many bulk OUT transfers can be in flight together.
const IN_FLIGHT: usize = 3;

impl HackrfTransmitter {
    /// The transmitter of `link`, disarmed, with `values`.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] for a value outside the limits of the board.
    pub fn open(
        link: Arc<Link>,
        values: &Values,
    ) -> Result<(HackrfTransmitter, Arc<TxShared>), DriverError> {
        let limits = link
            .capabilities()
            .transmit
            .ok_or(DriverError::CannotTransmit)?;
        let cfg = TxConfig::from_values(values, &limits, lock(&link.rx).rate)?;
        let size = proto::tx_write_size(cfg.rate);
        let caps = TxCapabilities {
            name: format!("{} TX", link.facts.name()),
            settings: tx_schema(HackrfTxSetting::SETTINGS, &limits),
            limits,
            outputs: vec!["tx".into()],
            cyclic: false,
            full_duplex: false,
            buffer_samples: size / 2,
        };
        let sh = Arc::new(TxShared::default());
        Ok((
            HackrfTransmitter {
                link,
                caps,
                cfg,
                state: TxState::Disarmed,
                sh: sh.clone(),
                prod: None,
                capacity: 0,
                writer: None,
                since: None,
                zero_bytes: 0,
                lines: Vec::new(),
            },
            sh,
        ))
    }

    fn closed(&self) -> bool {
        self.state == TxState::Closed || self.sh.is_closed() || self.link.gone()
    }

    /// The error of the writer, if it stopped.
    fn writer_error(&self) -> Option<DriverError> {
        lock(&self.sh.error).clone().map(DriverError::from)
    }

    /// The flush write that ends a burst for this firmware: the buffer size the device reported, 32 768 bytes below USB
    /// API 0x0112, and none when the device drains by itself (the size request failed).
    fn flush_bytes(&self) -> usize {
        match lock(&self.link.stats).buffer {
            Some(Some(n)) => (n as usize).min(MAX_FLUSH),
            Some(None) => 0,
            None => proto::FIXED_BUFFER,
        }
    }

    /// Sends the transmit settings: mode off, rate, filter, frequency, amplifier, the device buffer size once per open
    /// (USB API 0x0112 or later), TX VGA, antenna power.
    fn prepare(&self, ctl: &mut Ctl) -> Result<(), HackrfError> {
        let c = &self.cfg;
        ctl.set_mode(Mode::Off)?;
        ctl.sample_rate(c.rate)?;
        ctl.filter(c.filter)?;
        ctl.frequency(c.freq)?;
        ctl.amp(c.amp)?;
        let unread = lock(&self.link.stats).buffer.is_none();
        if unread && ctl.api() >= 0x0112 {
            let size = match ctl.buffer_size() {
                Ok(n) if n > 0 => Some(n),
                Err(e) if e.unplugged() => return Err(e),
                _ => None,
            };
            lock(&self.link.stats).buffer = Some(size);
        }
        ctl.tx_vga(c.tx_vga)?;
        if self.link.facts.board.antenna_power() {
            ctl.antenna_power(lock(&self.link.rx).antenna_power)?;
        }
        Ok(())
    }

    /// Stops the writer and joins it; the writer cancels its transfers before it ends.
    fn stop_writer(&mut self) {
        self.sh.stop.store(true, Ordering::Release);
        if let Some(w) = self.writer.take() {
            w.thread().unpark();
            let _ = w.join();
        }
        self.prod = None;
    }

    /// Ends transmit: mode off, then receive again if it ran before and the runner still wants it.
    fn end(&mut self) {
        let t0 = Instant::now();
        let link = self.link.clone();
        let mut dir = link.lock_dir();
        let mut ctl = link.lock_ctl();
        if !link.gone() {
            if let Err(e) = ctl.set_mode(Mode::Off) {
                link.note(&e);
            }
        }
        let (since, resume) = match &*dir {
            Dir::Tx { since, resume } => (Some(*since), *resume),
            _ => (None, false),
        };
        *dir = Dir::Off;
        if resume && link.rx_wanted.load(Ordering::Acquire) && !link.gone() && !self.sh.is_closed()
        {
            match link.start_rx(&mut dir, &mut ctl) {
                Ok(()) => {
                    *lock(&link.gap) = since;
                    lock(&link.stats).tx_to_rx = Some(t0.elapsed());
                }
                Err(e) => {
                    self.lines
                        .push(format!("receive did not start again after transmit: {e}"));
                    link.mark_lost();
                }
            }
        }
        link.wake.notify_all();
    }
}

impl Transmitter for HackrfTransmitter {
    fn capabilities(&self) -> &TxCapabilities {
        &self.caps
    }

    /// Stores and checks the settings. While on air, the frequency, the power, the filter and the amplifier change at
    /// once; a new rate is refused until disarmed.
    fn configure(&mut self, values: &Values) -> Result<(), DriverError> {
        if self.closed() {
            return Err(DriverError::NotNow {
                state: Doing::Closed,
            });
        }
        let cfg = TxConfig::from_values(values, &self.caps.limits, lock(&self.link.rx).rate)?;
        if self.state.transmitting() {
            if (cfg.rate - self.cfg.rate).abs() > 0.5 {
                return Err(DriverError::NotNow {
                    state: Doing::Transmitting,
                });
            }
            let mut ctl = self.link.lock_ctl();
            let r = (|| {
                if cfg.freq != self.cfg.freq {
                    ctl.frequency(cfg.freq)?;
                }
                if cfg.filter != self.cfg.filter {
                    ctl.filter(cfg.filter)?;
                }
                if cfg.tx_vga != self.cfg.tx_vga {
                    ctl.tx_vga(cfg.tx_vga)?;
                }
                if cfg.amp != self.cfg.amp {
                    ctl.amp(cfg.amp)?;
                }
                Ok::<(), HackrfError>(())
            })();
            drop(ctl);
            if let Err(e) = r {
                self.link.note(&e);
                return Err(e.into());
            }
        } else {
            self.caps.buffer_samples = proto::tx_write_size(cfg.rate) / 2;
        }
        self.cfg = cfg;
        Ok(())
    }

    /// Stops receive (reads first, then mode off), sends the transmit settings, opens the bulk OUT queue and starts the
    /// writer. Mode transmit follows when the first transfer is filled.
    fn arm(&mut self) -> Result<(), DriverError> {
        if self.closed() {
            return Err(DriverError::NotNow {
                state: Doing::Closed,
            });
        }
        if self.state.transmitting() {
            return Ok(());
        }
        let t0 = Instant::now();
        let link = self.link.clone();
        let mut dir = link.lock_dir();
        let resume = match &*dir {
            Dir::Rx(_) => true,
            Dir::Off => false,
            Dir::Tx { .. } => {
                return Err(DriverError::NotNow {
                    state: Doing::Transmitting,
                })
            }
        };
        *dir = Dir::Off;
        let size = proto::tx_write_size(self.cfg.rate);
        let mut ctl = link.lock_ctl();
        let out = match self
            .prepare(&mut ctl)
            .and_then(|()| ctl.writes(size, IN_FLIGHT))
        {
            Ok(out) => out,
            Err(e) => {
                link.note(&e);
                if resume && !link.gone() {
                    let _ = ctl.set_mode(Mode::Off);
                    if link.start_rx(&mut dir, &mut ctl).is_ok() {
                        *lock(&link.gap) = Some(t0);
                    }
                }
                return Err(e.into());
            }
        };
        *dir = Dir::Tx { since: t0, resume };
        drop(ctl);
        drop(dir);
        link.wake.notify_all();

        self.capacity = size * RING_TRANSFERS;
        let (prod, cons) = rtrb::RingBuffer::<u8>::new(self.capacity);
        self.zero_bytes = self.flush_bytes();
        for flag in [
            &self.sh.stop,
            &self.sh.flush,
            &self.sh.busy,
            &self.sh.started,
            &self.sh.zeros,
        ] {
            flag.store(false, Ordering::Release);
        }
        *lock(&self.sh.error) = None;
        let w = Writer {
            link: link.clone(),
            sh: self.sh.clone(),
            cons,
            out,
            size,
            rate: self.cfg.rate,
            armed_at: t0,
            zero_bytes: self.zero_bytes,
            last_done: Instant::now(),
        };
        let handle = std::thread::Builder::new()
            .name("sdrkit-hackrf-tx".into())
            .spawn(move || w.run())
            .map_err(|_| DriverError::Thread);
        let handle = match handle {
            Ok(h) => h,
            Err(e) => {
                self.end();
                return Err(e);
            }
        };
        self.writer = Some(handle);
        self.prod = Some(prod);
        self.state = TxState::Armed;
        self.since = Some(Instant::now());
        Ok(())
    }

    /// Stops the writer (queued samples are dropped), sends mode off and starts receive again where it ran.
    fn disarm(&mut self) {
        if !self.state.transmitting() {
            return;
        }
        self.stop_writer();
        self.end();
        self.state = if self.closed() {
            TxState::Closed
        } else {
            TxState::Disarmed
        };
        self.since = None;
    }

    /// Queues samples as signed 8-bit I/Q, waiting up to `wait` for room.
    fn push(&mut self, iq: &[Complex32], wait: Duration) -> Result<usize, DriverError> {
        if let Some(e) = self.writer_error() {
            return Err(e);
        }
        if self.state != TxState::Armed {
            return Err(DriverError::NotNow {
                state: Doing::Disarmed,
            });
        }
        let end = Instant::now() + wait;
        let writer = self.writer.as_ref().map(|w| w.thread().clone());
        let Some(prod) = self.prod.as_mut() else {
            return Err(DriverError::NotNow {
                state: Doing::Disarmed,
            });
        };
        let mut taken = 0;
        loop {
            let n = (prod.slots() / 2).min(iq.len() - taken);
            if n > 0 {
                if let Ok(mut chunk) = prod.write_chunk(2 * n) {
                    let (a, b) = chunk.as_mut_slices();
                    let bytes = iq[taken..taken + n]
                        .iter()
                        .flat_map(|c| [proto::tx_byte(c.re), proto::tx_byte(c.im)]);
                    for (slot, v) in a.iter_mut().chain(b.iter_mut()).zip(bytes) {
                        *slot = v;
                    }
                    chunk.commit_all();
                    taken += n;
                }
                if let Some(t) = &writer {
                    t.unpark();
                }
            }
            if taken == iq.len() || Instant::now() >= end {
                return Ok(taken);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Ends a burst: the writer sends what is queued, the last transfer padded with zeros to whole blocks.
    fn flush(&mut self) -> Result<(), DriverError> {
        if let Some(e) = self.writer_error() {
            return Err(e);
        }
        if self.state != TxState::Armed {
            return Err(DriverError::NotNow {
                state: Doing::Disarmed,
            });
        }
        self.sh.flush.store(true, Ordering::Release);
        if let Some(w) = &self.writer {
            w.thread().unpark();
        }
        Ok(())
    }

    /// The device buffer: the first sample of a burst reaches the antenna once the device holds this much.
    fn start_latency(&self) -> Duration {
        Duration::from_secs_f64(proto::FIXED_BUFFER as f64 / (2.0 * self.cfg.rate))
    }

    /// Waits until the writer has sent the burst that [`Transmitter::flush`] ended and no transfer is in flight, then
    /// has the writer send the flush zeros of the firmware and waits for them. Where the device drains by itself, it
    /// waits for the time of its buffer instead.
    fn drain(&mut self, wait: Duration) -> bool {
        let end = Instant::now() + wait;
        loop {
            let empty = self
                .prod
                .as_ref()
                .is_none_or(|p| p.slots() == self.capacity);
            if empty
                && !self.sh.busy.load(Ordering::Acquire)
                && !self.sh.flush.load(Ordering::Acquire)
            {
                break;
            }
            if self.writer_error().is_some() || Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if !self.sh.started.load(Ordering::Acquire) {
            return true;
        }
        if self.zero_bytes == 0 {
            let t = Duration::from_secs_f64(proto::DRAIN_BUFFER as f64 / (2.0 * self.cfg.rate));
            std::thread::sleep(t.min(end.saturating_duration_since(Instant::now())));
            return true;
        }
        self.sh.zeros.store(true, Ordering::Release);
        if let Some(w) = &self.writer {
            w.thread().unpark();
        }
        while self.sh.zeros.load(Ordering::Acquire) {
            if self.writer_error().is_some()
                || self.writer.is_none()
                || self.closed()
                || Instant::now() >= end
            {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    /// A HackRF has no buffer that it repeats by itself.
    fn play_cyclic(&mut self, _iq: &[Complex32]) -> Result<(), DriverError> {
        Err(DriverError::NotNow {
            state: if self.state.transmitting() {
                Doing::Transmitting
            } else {
                Doing::Disarmed
            },
        })
    }

    fn state(&self) -> TxState {
        if self.closed() {
            TxState::Closed
        } else {
            self.state
        }
    }

    fn stats(&self) -> TxStats {
        let mut s = lock(&self.sh.stats).clone();
        s.queued = self
            .prod
            .as_ref()
            .map_or(0, |p| (self.capacity - p.slots()) / 2);
        s.transmitting_since = self.since.filter(|_| self.state.transmitting());
        s
    }

    fn poll(&mut self, results: &mut Results) {
        for l in self.lines.drain(..) {
            results.log(l);
        }
        if let Some(e) = lock(&self.sh.error).as_ref() {
            results.problem(&e.to_string());
        }
    }

    fn close(&mut self) {
        self.disarm();
        self.sh.close();
        self.state = TxState::Closed;
    }
}

impl Drop for HackrfTransmitter {
    fn drop(&mut self) {
        self.close();
    }
}
