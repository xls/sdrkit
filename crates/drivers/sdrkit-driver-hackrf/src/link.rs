//! One open HackRF shared by its receive and transmit sides: the control handle, the direction the device streams in,
//! the receive settings, and the receive side as the runner drives it (radio and read loop).

use crate::ctl::{Ctl, Facts};
use crate::error::HackrfError;
use crate::proto::{self, Mode};
use crate::{HackrfSetting, Timing, AMP, CLOCK_OUT, LNA, VGA};
use sdrkit_core::{Hz, Results, SettingSet, Values};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::{Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::{
    own_id, Capabilities, DriverError, RadioSetting, RateLimits, TxLimits, BANDWIDTH, BIAS_TEE,
    CENTRE, GAIN, RATE,
};
use sdrkit_driver_usb::{BulkIn, UsbError};
use sdrkit_graph::stream::SampleType;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Locks `m`, also after a panic of another holder.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The receive settings, read from the values of a radio.
#[derive(Clone, Debug, PartialEq)]
pub struct RxConfig {
    /// The centre frequency in Hz.
    pub centre: u64,
    /// The sample rate.
    pub rate: f64,
    /// The baseband filter in Hz.
    pub filter: u32,
    /// The LNA gain in dB.
    pub lna: u32,
    /// The VGA gain in dB.
    pub vga: u32,
    /// The RF amplifier.
    pub amp: bool,
    /// Antenna port power.
    pub antenna_power: bool,
    /// The 10 MHz clock output.
    pub clock_out: bool,
}

impl RxConfig {
    /// The receive settings of `v` for the device `f`.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] for a frequency outside 0 to 7250 MHz, a rate outside the rates of the board, antenna
    /// power on a board without it, or the clock output on firmware without it.
    pub fn from_values(v: &Values, f: &Facts) -> Result<RxConfig, DriverError> {
        let centre = v.get(&CENTRE);
        if !(0.0..=proto::MAX_FREQ_HZ as f64).contains(&centre) {
            return Err(DriverError::Refused {
                id: RadioSetting::Centre.id(),
            });
        }
        let rate = match v.get(&RATE) {
            r if r > 0.0 => r,
            _ => proto::DEFAULT_RATE_HZ,
        };
        if rate < f.board.min_rate_hz() - 0.5 || rate > proto::MAX_RATE_HZ + 0.5 {
            return Err(DriverError::Refused {
                id: RadioSetting::Rate.id(),
            });
        }
        let filter = match v.get(&BANDWIDTH) {
            w if w > 0.0 => proto::filter_at_least(w),
            _ => proto::auto_filter(rate),
        };
        let (mut lna, mut vga) = proto::split_gain(v.get(&GAIN));
        if let Ok(db) = u32::try_from(v.get(&LNA)) {
            lna = proto::lna_step(db);
        }
        if let Ok(db) = u32::try_from(v.get(&VGA)) {
            vga = proto::vga_step(db);
        }
        let antenna_power = v.get(&BIAS_TEE);
        if antenna_power && !f.board.antenna_power() {
            return Err(DriverError::Refused {
                id: RadioSetting::BiasTee.id(),
            });
        }
        let clock_out = v.get(&CLOCK_OUT);
        if clock_out && f.api < 0x0103 {
            return Err(DriverError::Refused {
                id: own_id(HackrfSetting::ClockOut),
            });
        }
        Ok(RxConfig {
            centre: centre.round() as u64,
            rate,
            filter,
            lna,
            vga,
            amp: v.get(&AMP),
            antenna_power,
            clock_out,
        })
    }

    /// Warnings for the log: a rate below 8 MS/s, a frequency outside the specified range of the board.
    pub fn notes(&self, f: &Facts) -> String {
        let mut notes = Vec::new();
        if self.rate < proto::CLEAN_RATE_HZ - 0.5 {
            notes.push("below 8 MS/s: aliasing; prefer decimation".to_string());
        }
        let (lo, hi) = f.board.specified_hz();
        let c = self.centre as f64;
        if c < lo || c > hi {
            notes.push(format!(
                "{} MHz is outside the specified range of the board ({} to {} MHz): reduced performance",
                sdrkit_driver_core::fmt_m(c),
                sdrkit_driver_core::fmt_m(lo),
                sdrkit_driver_core::fmt_m(hi)
            ));
        }
        notes.join("; ")
    }
}

/// The direction the device streams in.
pub enum Dir {
    /// Idle.
    Off,
    /// Receiving into these queued reads.
    Rx(Box<dyn BulkIn>),
    /// Held by the transmitter since `since`. `resume`: receive starts again when transmit ends.
    Tx {
        /// When receive stopped (or transmit began).
        since: Instant,
        /// Receive starts again when transmit ends.
        resume: bool,
    },
}

/// Figures of the link that the readings show.
#[derive(Clone, Debug, Default)]
pub struct LinkStats {
    /// A clock is detected on the clock input (read after each start, USB API 0x0106 or later).
    pub clock_in: Option<bool>,
    /// The last switch from receive to transmit.
    pub rx_to_tx: Option<Duration>,
    /// The last switch from transmit to receive.
    pub tx_to_rx: Option<Duration>,
    /// Shortfalls the device reported since the open.
    pub shortfalls: u64,
    /// The rate the device samples at, from the streaming state.
    pub measured_rate: Option<f64>,
    /// The device buffer size: `None` not read yet, `Some(None)` read and refused.
    pub buffer: Option<Option<u32>>,
}

/// One open device, shared by the radio, its read loop and the transmitter. Lock order: `dir`, then `ctl`.
pub struct Link {
    /// What the device is.
    pub facts: Facts,
    /// The timings.
    pub timing: Timing,
    /// The control side.
    pub ctl: Mutex<Ctl>,
    /// The direction.
    pub dir: Mutex<Dir>,
    /// Woken when the direction changes or the link goes.
    pub wake: Condvar,
    /// Callers of [`Link::lock_dir`] that wait for the direction.
    pub dir_waiters: AtomicUsize,
    /// Callers of [`Link::lock_ctl`] that wait for the control side.
    pub ctl_waiters: AtomicUsize,
    /// The receive settings.
    pub rx: Mutex<RxConfig>,
    /// The figures of the readings.
    pub stats: Mutex<LinkStats>,
    /// Set when receive restarts after transmit: the start of the gap.
    pub gap: Mutex<Option<Instant>>,
    /// The runner wants receive to stream.
    pub rx_wanted: AtomicBool,
    lost: AtomicBool,
    closed: AtomicBool,
}

impl Link {
    /// A link to the device of `ctl`, described by `facts`, with the receive settings `rx`.
    pub fn new(ctl: Ctl, facts: Facts, timing: Timing, rx: RxConfig) -> Link {
        Link {
            facts,
            timing,
            ctl: Mutex::new(ctl),
            dir: Mutex::new(Dir::Off),
            wake: Condvar::new(),
            dir_waiters: AtomicUsize::new(0),
            ctl_waiters: AtomicUsize::new(0),
            rx: Mutex::new(rx),
            stats: Mutex::new(LinkStats::default()),
            gap: Mutex::new(None),
            rx_wanted: AtomicBool::new(false),
            lost: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        }
    }

    /// Locks the direction for a caller other than the read loop, which gives way while such a caller waits.
    pub fn lock_dir(&self) -> MutexGuard<'_, Dir> {
        self.dir_waiters.fetch_add(1, Ordering::AcqRel);
        let g = lock(&self.dir);
        self.dir_waiters.fetch_sub(1, Ordering::AcqRel);
        g
    }

    /// Locks the control side for a caller other than the transmit writer, which gives way while such a caller waits.
    pub fn lock_ctl(&self) -> MutexGuard<'_, Ctl> {
        self.ctl_waiters.fetch_add(1, Ordering::AcqRel);
        let g = lock(&self.ctl);
        self.ctl_waiters.fetch_sub(1, Ordering::AcqRel);
        g
    }

    /// The device left the bus or the link was closed.
    pub fn gone(&self) -> bool {
        self.lost.load(Ordering::Acquire) || self.closed.load(Ordering::Acquire)
    }

    /// Notes that the device left the bus and wakes the waiters.
    pub fn mark_lost(&self) {
        self.lost.store(true, Ordering::Release);
        self.wake.notify_all();
    }

    /// Notes an error of a transfer: an unplug marks the link lost.
    pub fn note(&self, e: &HackrfError) {
        if e.unplugged() {
            self.mark_lost();
        }
    }

    /// Starts receive: the clock input status (USB API 0x0106 or later), rate, filter, frequency, amplifier, LNA, VGA,
    /// clock output, antenna power (on boards that have it), mode receive, then the queued reads.
    ///
    /// The clock input status is read before the rate: on firmware 2026.01.3 the read sets the sample clock again from
    /// the rate before the last rate request, so a read after the rate request can leave the device at its previous
    /// rate.
    pub fn start_rx(&self, dir: &mut Dir, ctl: &mut Ctl) -> Result<(), HackrfError> {
        let c = lock(&self.rx).clone();
        if ctl.api() >= 0x0106 {
            match ctl.clock_in() {
                Ok(v) => lock(&self.stats).clock_in = Some(v),
                Err(e) if e.unplugged() => {
                    self.note(&e);
                    return Err(e);
                }
                Err(_) => {}
            }
        }
        let r = (|| {
            ctl.sample_rate(c.rate)?;
            ctl.filter(c.filter)?;
            ctl.frequency(c.centre)?;
            ctl.amp(c.amp)?;
            ctl.lna(c.lna)?;
            ctl.vga(c.vga)?;
            if ctl.api() >= 0x0103 {
                ctl.clock_out(c.clock_out)?;
            }
            if self.facts.board.antenna_power() {
                ctl.antenna_power(c.antenna_power)?;
            }
            ctl.set_mode(Mode::Receive)?;
            ctl.reads(self.timing.read_size, self.timing.reads_queued)
        })();
        let reads = r.inspect_err(|e| self.note(e))?;
        *dir = Dir::Rx(reads);
        self.wake.notify_all();
        Ok(())
    }

    /// Stops receive: the queued reads are cancelled and ended first, then mode off is sent.
    pub fn stop_rx(&self, dir: &mut Dir, ctl: &mut Ctl) {
        if matches!(dir, Dir::Rx(_)) {
            *dir = Dir::Off;
            if !self.gone() {
                if let Err(e) = ctl.set_mode(Mode::Off) {
                    self.note(&e);
                }
            }
        }
        self.wake.notify_all();
    }

    /// Sends what changed from `old` to `new` while receive streams.
    fn apply_live(&self, ctl: &mut Ctl, old: &RxConfig, new: &RxConfig) -> Result<(), HackrfError> {
        if new.filter != old.filter {
            ctl.filter(new.filter)?;
        }
        if new.centre != old.centre {
            ctl.frequency(new.centre)?;
        }
        if new.amp != old.amp {
            ctl.amp(new.amp)?;
        }
        if new.lna != old.lna {
            ctl.lna(new.lna)?;
        }
        if new.vga != old.vga {
            ctl.vga(new.vga)?;
        }
        if new.clock_out != old.clock_out {
            ctl.clock_out(new.clock_out)?;
        }
        if new.antenna_power != old.antenna_power && self.facts.board.antenna_power() {
            ctl.antenna_power(new.antenna_power)?;
        }
        Ok(())
    }

    /// Closes the link: receive stops (reads first, then mode off), a transmission ends with mode off, and the device
    /// is released, whoever still holds the link. Nothing is sent afterwards.
    pub fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let mut dir = self.lock_dir();
        *dir = Dir::Off;
        self.wake.notify_all();
        let mut ctl = self.lock_ctl();
        if !self.lost.load(Ordering::Acquire) && ctl.mode_now() != Some(Mode::Off) {
            let _ = ctl.set_mode(Mode::Off);
        }
        ctl.release();
    }

    /// What the device can do.
    pub fn capabilities(&self) -> Capabilities {
        let b = self.facts.board;
        let rates = RateLimits {
            min_hz: b.min_rate_hz(),
            max_hz: proto::MAX_RATE_HZ,
            ranges: vec![(b.min_rate_hz(), proto::MAX_RATE_HZ)],
            steady_hz: None,
        };
        let filters: Vec<Hz> = proto::FILTERS_HZ.iter().map(|&f| f64::from(f)).collect();
        let (lo, hi) = b.specified_hz();
        let transmit = TxLimits {
            min_freq_hz: lo,
            max_freq_hz: hi,
            rates: rates.clone(),
            bandwidth_hz: Some((filters[0], filters[15])),
            power_db: (-f64::from(proto::TX_VGA_MAX), 0.0, 1.0),
            start_power_db: None,
            common_rates_hz: COMMON_RATES.to_vec(),
            full_output_dbm: b.tx_output_dbm(),
        };
        let serial = self.facts.serial();
        let tail = &serial[serial.len().saturating_sub(8)..];
        Capabilities {
            name: if tail.is_empty() {
                self.facts.name()
            } else {
                format!("{} {tail}", self.facts.name())
            },
            rates,
            min_freq_hz: 0.0,
            max_freq_hz: proto::MAX_FREQ_HZ as f64,
            gain_db: Some((0.0, proto::GAIN_MAX, 2.0)),
            bandwidth_hz: Some((filters[0], filters[15])),
            has_agc: false,
            transmit: Some(transmit),
            has_bias_tee: b.antenna_power(),
            realtime: true,
            common_rates_hz: COMMON_RATES.to_vec(),
            common_bandwidths_hz: filters,
            gain_steps_db: Vec::new(),
        }
    }
}

/// The rates people normally use.
pub const COMMON_RATES: &[Hz] = &[2e6, 4e6, 8e6, 10e6, 12.5e6, 16e6, 20e6];

/// The receive side of an open device, as the runner drives it.
pub struct HackrfRadio {
    /// The link.
    pub link: Arc<Link>,
}

impl Radio for HackrfRadio {
    type Stream = HackrfStream;

    /// Not live: stores and checks the settings; the next start sends them. Live: sends what changed, or stores it while
    /// the transmitter holds the device.
    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError> {
        let link = &self.link;
        let cfg = RxConfig::from_values(values, &link.facts)?;
        let notes = cfg.notes(&link.facts);
        let dir = link.lock_dir();
        let old = std::mem::replace(&mut *lock(&link.rx), cfg.clone());
        if live && matches!(*dir, Dir::Rx(_)) {
            let mut ctl = link.lock_ctl();
            if let Err(e) = link.apply_live(&mut ctl, &old, &cfg) {
                link.note(&e);
                *lock(&link.rx) = old;
                return Err(e.into());
            }
        }
        Ok(notes)
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<HackrfStream, DriverError> {
        let link = &self.link;
        link.rx_wanted.store(true, Ordering::Release);
        let mut dir = link.lock_dir();
        match &mut *dir {
            Dir::Tx { resume, .. } => *resume = true,
            Dir::Rx(_) => {}
            Dir::Off => {
                let mut ctl = link.lock_ctl();
                link.start_rx(&mut dir, &mut ctl)?;
            }
        }
        drop(dir);
        Ok(HackrfStream::new(link.clone(), abort.clone()))
    }

    fn stream_ended(&mut self) {
        let link = &self.link;
        link.rx_wanted.store(false, Ordering::Release);
        let mut dir = link.lock_dir();
        match &mut *dir {
            Dir::Tx { resume, .. } => *resume = false,
            Dir::Rx(_) => {
                let mut ctl = link.lock_ctl();
                link.stop_rx(&mut dir, &mut ctl);
            }
            Dir::Off => {}
        }
    }

    fn close(&mut self) {
        self.link.close();
    }

    fn rate_hz(&self) -> Hz {
        lock(&self.link.rx).rate
    }

    fn centre_hz(&self) -> Hz {
        lock(&self.link.rx).centre as Hz
    }

    fn capabilities(&self) -> Capabilities {
        self.link.capabilities()
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cs8
    }

    /// A new rate stops the stream and starts it again.
    fn live_rate_change(&self) -> bool {
        false
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        RxConfig::from_values(values, &self.link.facts).map(|_| ())
    }

    fn status(&self) -> String {
        let f = &self.link.facts;
        let c = lock(&self.link.rx);
        format!(
            "{}, firmware {}, {} MS/s at {} MHz",
            f.name(),
            f.version,
            sdrkit_driver_core::fmt_m(c.rate),
            sdrkit_driver_core::fmt_m(c.centre as f64)
        )
    }

    fn readings(&mut self, results: &mut Results) {
        crate::write_readings(&self.link, results);
    }
}

/// The read loop of the receive side. It stays in its loop while the transmitter holds the device, and marks the gap
/// when receive starts again.
pub struct HackrfStream {
    link: Arc<Link>,
    abort: Arc<Abort>,
    rate: f64,
    last_block: Instant,
    next_poll: Instant,
    shortfalls: Option<u32>,
    sampled: Option<(u32, Instant)>,
    off_rate: u32,
}

impl HackrfStream {
    fn new(link: Arc<Link>, abort: Arc<Abort>) -> HackrfStream {
        let rate = lock(&link.rx).rate;
        HackrfStream {
            link,
            abort,
            rate,
            last_block: Instant::now(),
            next_poll: Instant::now(),
            shortfalls: None,
            sampled: None,
            off_rate: 0,
        }
    }

    /// Reads the streaming state when it is due and the control side is free, counts new overruns as a gap, and warns
    /// once when the rate the device samples at differs from the set rate by more than 5 % on two polls in a row.
    fn poll_state(&mut self, sink: &mut Sink, ctx: &StreamCtx) {
        let Some(every) = self.link.timing.state_poll else {
            return;
        };
        let now = Instant::now();
        if now < self.next_poll || self.link.facts.api < 0x0106 {
            return;
        }
        self.next_poll = now + every;
        let Ok(mut ctl) = self.link.ctl.try_lock() else {
            return;
        };
        let Ok(s) = ctl.stream_state() else { return };
        drop(ctl);
        if let Some(before) = self.shortfalls {
            let grown = s.shortfalls.wrapping_sub(before);
            if grown > 0 && grown < 1 << 30 {
                lock(&self.link.stats).shortfalls += u64::from(grown);
                let lost = u64::from(grown) * u64::from(s.longest / 2).max(1);
                sink.lost(lost);
            }
        }
        self.shortfalls = Some(s.shortfalls);
        if let Some((bytes, at)) = self.sampled {
            let secs = now.duration_since(at).as_secs_f64();
            let measured = f64::from(s.sampled.wrapping_sub(bytes)) / 2.0 / secs;
            lock(&self.link.stats).measured_rate = Some(measured);
            let off = (measured - self.rate).abs() > 0.05 * self.rate;
            self.off_rate = if off { self.off_rate + 1 } else { 0 };
            if self.off_rate == 2 {
                ctx.notices.warn(format!(
                    "the device samples at {} MS/s, not at the set {} MS/s",
                    sdrkit_driver_core::fmt_m(measured),
                    sdrkit_driver_core::fmt_m(self.rate)
                ));
            }
        }
        self.sampled = Some((s.sampled, now));
    }
}

impl RadioStream for HackrfStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let link = self.link.clone();
        let slice = link.timing.read_slice;
        loop {
            if ctx.stopping() || self.abort.fired() || sink.closed() {
                return Ok(());
            }
            if link.gone() {
                return Err(HackrfError::Usb(UsbError::Unplugged).into());
            }
            // Others that wait for the direction go first: the loop takes it back at once otherwise.
            while link.dir_waiters.load(Ordering::Acquire) > 0 && !ctx.stopping() {
                std::thread::sleep(Duration::from_micros(200));
            }
            let mut dir = lock(&link.dir);
            match &mut *dir {
                Dir::Rx(reads) => match reads.next(slice) {
                    Ok(block) => {
                        if let Some(since) = lock(&link.gap).take() {
                            let gap = (since.elapsed().as_secs_f64() * self.rate).round() as u64;
                            sink.deliberate_gap(gap.max(1));
                        }
                        sink.write(block);
                        self.last_block = Instant::now();
                    }
                    Err(UsbError::Timeout) => {
                        if self.last_block.elapsed() >= link.timing.watchdog {
                            return Err(HackrfError::StoppedStreaming.into());
                        }
                    }
                    Err(UsbError::Stall) => {
                        if let Err(e) = reads.clear_halt() {
                            let e = HackrfError::Usb(e);
                            link.note(&e);
                            return Err(e.into());
                        }
                    }
                    Err(e) => {
                        let e = HackrfError::Usb(e);
                        link.note(&e);
                        return Err(e.into());
                    }
                },
                Dir::Off | Dir::Tx { .. } => {
                    // No watchdog while the transmitter holds the device or receive is stopped.
                    self.last_block = Instant::now();
                    let _ = link
                        .wake
                        .wait_timeout(dir, slice)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    continue;
                }
            }
            drop(dir);
            self.poll_state(sink, ctx);
        }
    }
}
