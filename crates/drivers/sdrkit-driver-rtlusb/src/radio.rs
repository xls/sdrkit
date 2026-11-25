//! An open dongle as the shared runner drives it: the settings applied to the dongle in a fixed order, the bulk stream
//! into the sink, and the gap after a retune (specification 6.3).

use crate::device::{Direct, Gain, Rtl2832, TunerKind, ENDPOINT, PACKET};
use crate::{e4000, RtlError};
use sdrkit_core::publish::field;
use sdrkit_core::{Field, Hz, Results, SettingSet, Values};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::{Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::{
    Capabilities, DriverError, RadioSetting, RateLimits, AGC, BANDWIDTH, BIAS_TEE, CENTRE, GAIN,
    RATE, RATE_EXACT,
};
use sdrkit_driver_usb::{BulkIn, UsbError};
use sdrkit_graph::stream::SampleType;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The tuner of the dongle.
pub const TUNER: Field<field::Text> = Field::new("tuner", "Tuner", "");
/// The PLL of the tuner locked at the last tune.
pub const PLL_LOCKED: Field<field::Bool> = Field::new("pll_locked", "PLL locked", "");

/// The rate used when none is set.
pub const DEFAULT_RATE: u32 = 2_048_000;
/// The highest rate that most hosts take without loss.
pub const STEADY_RATE: Hz = 2.56e6;
/// The rates that people normally use.
pub const COMMON_RATES: [Hz; 11] = [
    250_000.0, 300_000.0, 900_001.0, 1.024e6, 1.4e6, 1.8e6, 2.048e6, 2.4e6, 2.56e6, 2.88e6, 3.2e6,
];
/// The widths of the filter of an R820T-family tuner.
pub const R82XX_BANDWIDTHS: [Hz; 12] = [
    350e3, 550e3, 1.2e6, 1.45e6, 1.55e6, 1.8e6, 1.95e6, 2.05e6, 2.43e6, 6e6, 7e6, 8e6,
];
/// Direct sampling is used below this frequency when it is set.
pub const DIRECT_BELOW_HZ: f64 = 24e6;

/// How long a bulk read waits before the stream looks at a stop.
const POLL: Duration = Duration::from_millis(50);
/// No data for this long ends the stream.
const STALL: Duration = Duration::from_secs(3);
/// The settle time after a retune.
const SETTLE: Duration = Duration::from_millis(5);

/// The size of one bulk transfer and the number of transfers in flight for the rate `rate_hz`: about 32 ms per
/// transfer, a multiple of 16 KiB from 16 KiB to 256 KiB, and at least 500 ms in flight.
pub fn transfer_plan(rate_hz: f64) -> (usize, usize) {
    const UNIT: f64 = 16_384.0;
    let bytes_per_s = (rate_hz * 2.0).max(1.0);
    let size = ((bytes_per_s * 0.032 / UNIT).ceil() * UNIT).clamp(UNIT, 262_144.0) as usize;
    let queued = ((bytes_per_s * 0.5) / size as f64).ceil().clamp(8.0, 32.0) as usize;
    (size / PACKET * PACKET, queued)
}

/// The rate to set for the values: the rate set, raised to the next rate the chip makes; 0 takes the default.
///
/// # Errors
///
/// [`DriverError::Refused`] for [`RATE`] above 3.2 MHz, or not one the chip makes with [`RATE_EXACT`] on.
pub fn rate_for(values: &Values) -> Result<u32, DriverError> {
    let refused = DriverError::Refused {
        id: RadioSetting::Rate.id(),
    };
    let asked = values.get(&RATE).round();
    if asked <= 0.0 {
        return Ok(DEFAULT_RATE);
    }
    if asked > 3_200_000.0 {
        return Err(refused);
    }
    let r = asked as u32;
    let snapped = match r {
        0..=225_000 => 225_001,
        300_001..=900_000 => 900_001,
        _ => r,
    };
    if snapped != r && values.get(&RATE_EXACT) {
        return Err(refused);
    }
    Ok(snapped)
}

/// The retunes, for the stream thread: a count that is odd while a configure runs, and the time of its last change.
#[derive(Debug)]
pub struct Mark {
    epoch: Instant,
    seq: AtomicU64,
    at_ns: AtomicU64,
}

impl Mark {
    /// No retune yet.
    pub fn new() -> Mark {
        Mark {
            epoch: Instant::now(),
            seq: AtomicU64::new(0),
            at_ns: AtomicU64::new(0),
        }
    }

    fn step(&self) {
        let ns = self.epoch.elapsed().as_nanos() as u64;
        self.at_ns.store(ns, Ordering::Release);
        self.seq.fetch_add(1, Ordering::AcqRel);
    }

    /// A retune begins now.
    pub fn begin(&self) {
        self.step();
    }

    /// The retune ends now.
    pub fn end(&self) {
        self.step();
    }

    fn seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    fn at(&self) -> Instant {
        self.epoch + Duration::from_nanos(self.at_ns.load(Ordering::Acquire))
    }
}

impl Default for Mark {
    fn default() -> Self {
        Mark::new()
    }
}

/// Decides which bytes after a retune were sampled before the tuner settled. It maps each byte position of the
/// stream to the time it was sampled: position / byte rate + the least delay seen between that time and the arrival of
/// a transfer (the delay may rise by 1 ms per second, for clock drift and lost samples). After a retune at T it drops
/// every byte sampled before T + 5 ms, also those of transfers that were complete before the retune but read after
/// it; while the retune runs it drops every byte. At most `cap` bytes are dropped for one retune.
#[derive(Clone, Debug)]
pub struct SettleGate {
    epoch: Instant,
    byte_rate: f64,
    pos: u64,
    delay: Option<f64>,
    last: f64,
    pending: Option<Instant>,
    until: Option<u64>,
    dropped: usize,
    cap: usize,
}

/// How fast the delay estimate of [`SettleGate`] may rise, in seconds per second.
const DELAY_RISE: f64 = 1e-3;

impl SettleGate {
    /// A gate for a stream that started at `epoch` with `byte_rate` bytes per second; it drops at most `cap` bytes
    /// per retune.
    pub fn new(epoch: Instant, byte_rate: f64, cap: usize) -> SettleGate {
        SettleGate {
            epoch,
            byte_rate: byte_rate.max(1.0),
            pos: 0,
            delay: None,
            last: 0.0,
            pending: None,
            until: None,
            dropped: 0,
            cap,
        }
    }

    /// A retune begins: every byte is dropped until it ends.
    pub fn begin(&mut self) {
        if self.until.is_none() {
            self.dropped = 0;
        }
        self.until = Some(u64::MAX);
        self.pending = None;
    }

    /// A retune ended at `at`.
    pub fn mark(&mut self, at: Instant) {
        self.pending = Some(at);
    }

    /// The bytes to drop from the start of the next transfer, of `len` bytes, read at `now`. An even count.
    pub fn front(&mut self, now: Instant, len: usize) -> usize {
        let t = now.saturating_duration_since(self.epoch).as_secs_f64();
        let end = self.pos + len as u64;
        let seen = t - end as f64 / self.byte_rate;
        let delay = match self.delay {
            None => seen,
            Some(d) => (d + DELAY_RISE * (t - self.last)).min(seen),
        };
        (self.delay, self.last) = (Some(delay), t);
        if let Some(at) = self.pending.take() {
            let settled = (at + SETTLE)
                .saturating_duration_since(self.epoch)
                .as_secs_f64();
            let p = ((settled - delay) * self.byte_rate).ceil().max(0.0) as u64;
            self.until = match self.until {
                None => {
                    self.dropped = 0;
                    Some(p)
                }
                Some(u64::MAX) => Some(p),
                Some(u) => Some(u.max(p)),
            };
        }
        let mut k = 0;
        if let Some(p) = self.until {
            let room = self.cap.saturating_sub(self.dropped);
            let want = p.saturating_sub(self.pos).min(len as u64) as usize;
            k = ((want + 1) & !1).min(len).min(room & !1);
            self.dropped += k;
            if end >= p || k < want {
                self.until = None;
            }
        }
        self.pos = end;
        k
    }
}

/// The settings of the values, in the form the dongle takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wanted {
    /// The frequency.
    pub centre: u64,
    /// The sample rate.
    pub rate: u32,
    /// The tuner filter, 0 from the rate.
    pub bandwidth: u32,
    /// The tuner gain.
    pub gain: Gain,
    /// The frequency correction.
    pub ppm: f64,
    /// Direct sampling at this frequency.
    pub direct: Direct,
    /// Offset tuning.
    pub offset: bool,
    /// The digital AGC.
    pub rtl_agc: bool,
    /// The bias tee.
    pub bias: bool,
}

/// What was applied to the dongle; `None` for a part not applied yet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Applied {
    ppm: Option<f64>,
    direct: Option<Direct>,
    rate: Option<u32>,
    bandwidth: Option<u32>,
    offset: Option<bool>,
    centre: Option<u64>,
    gain: Option<Gain>,
    mode: Option<(bool, bool)>,
    bias: Option<bool>,
}

/// The dongles this driver has open, by bus path, with their tuners.
pub type OpenSet = Mutex<HashMap<String, TunerKind>>;

/// An open dongle, run by the shared runner.
pub struct RtlRadio {
    dev: Option<Rtl2832>,
    path: String,
    open: Arc<OpenSet>,
    applied: Applied,
    mark: Arc<Mark>,
    /// The tuner marks the retune itself (a change of the frequency alone while streaming).
    edges: Arc<AtomicBool>,
    counter: bool,
    bytes: Arc<AtomicU64>,
    product: String,
    /// Direct sampling is set, for the frequencies below [`DIRECT_BELOW_HZ`].
    ds_on: bool,
}

impl RtlRadio {
    /// The radio of the open dongle `dev`; `open` lists it until it closes. With `counter` the dongle streams the
    /// counter of the test mode. `bytes` counts the bytes the stream reads.
    pub fn new(
        mut dev: Rtl2832,
        open: Arc<OpenSet>,
        counter: bool,
        bytes: Arc<AtomicU64>,
    ) -> RtlRadio {
        let mark = Arc::new(Mark::new());
        let edges = Arc::new(AtomicBool::new(false));
        let (m, e) = (mark.clone(), edges.clone());
        dev.set_retune_edge(Some(Arc::new(move |on| {
            if e.load(Ordering::Acquire) {
                if on {
                    m.begin();
                } else {
                    m.end();
                }
            }
        })));
        let path = dev.info().path();
        let product = dev.info().product.clone().unwrap_or_default();
        open.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(path.clone(), dev.tuner());
        RtlRadio {
            dev: Some(dev),
            path,
            open,
            applied: Applied::default(),
            mark,
            edges,
            counter,
            bytes,
            product,
            ds_on: false,
        }
    }

    /// The open dongle.
    pub fn device(&mut self) -> Option<&mut Rtl2832> {
        self.dev.as_mut()
    }

    /// The settings of `values` for this dongle.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] for a rate or a frequency the dongle does not take.
    pub fn wanted(&self, values: &Values) -> Result<Wanted, DriverError> {
        let dev = self.dev.as_ref().ok_or(DriverError::NotNow {
            state: sdrkit_driver_core::Doing::Closed,
        })?;
        let rate = rate_for(values)?;
        let centre = values.get(&CENTRE).round().max(0.0);
        let ds = values.get(&crate::DIRECT_SAMPLING);
        let direct = match ds {
            crate::DirectSampling::I if centre < DIRECT_BELOW_HZ => Direct::I,
            crate::DirectSampling::Q if centre < DIRECT_BELOW_HZ => Direct::Q,
            _ => Direct::Off,
        };
        let (min, max) = freq_range(dev, direct != Direct::Off);
        if centre < min || centre > max {
            return Err(DriverError::Refused {
                id: RadioSetting::Centre.id(),
            });
        }
        let gain = if values.get(&AGC) {
            Gain::Auto
        } else {
            Gain::Manual((values.get(&GAIN) * 10.0).round() as i32)
        };
        Ok(Wanted {
            centre: centre as u64,
            rate,
            bandwidth: values.get(&BANDWIDTH).round().clamp(0.0, 20e6) as u32,
            gain,
            ppm: values.get(&crate::PPM),
            direct: if dev.board() == crate::r82xx::Board::Generic {
                direct
            } else {
                Direct::Off
            },
            offset: values.get(&crate::OFFSET_TUNING) && dev.tuner().zero_if(),
            rtl_agc: values.get(&crate::RTL_AGC),
            bias: values.get(&BIAS_TEE),
        })
    }

    /// Applies `w` to the dongle: only the parts that differ from what was applied, in the order frequency correction,
    /// direct sampling, rate, filter, offset tuning, frequency, gain, demodulator mode, bias tee.
    fn apply(&mut self, w: &Wanted) -> Result<(), RtlError> {
        let a = &mut self.applied;
        let d = self
            .dev
            .as_mut()
            .ok_or(RtlError::NotHere("the dongle is closed"))?;
        if a.ppm != Some(w.ppm) {
            d.set_ppm(w.ppm)?;
            a.ppm = Some(w.ppm);
        }
        if a.direct != Some(w.direct) {
            if d.direct() != w.direct {
                a.centre = None;
                d.set_direct(w.direct, w.centre)?;
                a.centre = Some(w.centre);
            }
            a.direct = Some(w.direct);
        }
        if a.rate != Some(w.rate) {
            d.set_sample_rate(w.rate)?;
            a.rate = Some(w.rate);
        }
        if a.bandwidth != Some(w.bandwidth) {
            if w.bandwidth > 0 || a.bandwidth.is_some() {
                d.set_bandwidth(w.bandwidth)?;
            }
            a.bandwidth = Some(w.bandwidth);
        }
        if a.offset != Some(w.offset) {
            d.set_offset_tuning(w.offset)?;
            a.offset = Some(w.offset);
        }
        if a.centre != Some(w.centre) || d.frequency() != Some(w.centre) {
            a.centre = None;
            d.set_frequency(w.centre)?;
            a.centre = Some(w.centre);
        }
        if a.gain != Some(w.gain) {
            d.set_gain(w.gain)?;
            a.gain = Some(w.gain);
        }
        let mode = (w.rtl_agc, self.counter);
        if a.mode != Some(mode) {
            d.set_rtl_agc(w.rtl_agc)?;
            if self.counter {
                d.set_test_mode(true)?;
            }
            a.mode = Some(mode);
        }
        if a.bias != Some(w.bias) {
            if w.bias || a.bias.is_some() {
                d.set_bias_tee(w.bias)?;
            }
            a.bias = Some(w.bias);
        }
        Ok(())
    }
}

/// The frequency range of the dongle in Hz, with direct sampling or through the tuner.
fn freq_range(dev: &Rtl2832, direct: bool) -> (f64, f64) {
    let tuner = match dev.tuner() {
        TunerKind::E4000 => (e4000::MIN_HZ, e4000::MAX_HZ),
        _ => crate::r82xx::range_hz(dev.board()),
    };
    if direct {
        (0.0, tuner.1)
    } else {
        tuner
    }
}

impl Radio for RtlRadio {
    type Stream = RtlStream;

    fn configure(&mut self, values: &Values, live: bool) -> Result<String, DriverError> {
        let w = self.wanted(values)?;
        let a = self.applied;
        let others = Some(w.rate) != a.rate
            || Some(w.direct) != a.direct
            || Some(w.offset) != a.offset
            || Some(w.bandwidth) != a.bandwidth
            || Some(w.ppm) != a.ppm;
        // A change of the frequency alone is marked by the tuner around its LO change; any other change of the
        // samples is marked around the whole configure.
        let whole = live && others;
        self.edges.store(live && !others, Ordering::Release);
        if whole {
            self.mark.begin();
        }
        let r = self.apply(&w);
        if whole {
            self.mark.end();
        }
        self.edges.store(false, Ordering::Release);
        r?;
        self.ds_on = values.get(&crate::DIRECT_SAMPLING) != crate::DirectSampling::Off;
        let dev = self.dev.as_ref();
        let mut notes = Vec::new();
        if values.get(&crate::OFFSET_TUNING) && dev.is_some_and(|d| !d.tuner().zero_if()) {
            notes.push("offset tuning applies only to zero-IF tuners");
        }
        if values.get(&crate::DIRECT_SAMPLING) != crate::DirectSampling::Off
            && dev.is_some_and(|d| d.board() != crate::r82xx::Board::Generic)
        {
            notes.push("an RTL-SDR Blog V4 or V4L receives HF through its upconverter: direct sampling is not used");
        }
        Ok(notes.join("; "))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<RtlStream, DriverError> {
        let d = self.dev.as_mut().ok_or(DriverError::NotNow {
            state: sdrkit_driver_core::Doing::Closed,
        })?;
        let rate = d.rate_hz();
        let (size, queued) = transfer_plan(rate);
        d.reset_buffer()?;
        let bulk = d
            .device()
            .bulk_in(ENDPOINT, size, queued)
            .map_err(RtlError::from)?;
        let byte_rate = rate * 2.0;
        let cap = (queued + 1) * size + (byte_rate * SETTLE.as_secs_f64()) as usize;
        Ok(RtlStream {
            bulk: Some(bulk),
            abort: abort.clone(),
            mark: self.mark.clone(),
            seen: self.mark.seq(),
            gate: SettleGate::new(Instant::now(), byte_rate, cap),
            queued,
            bytes: self.bytes.clone(),
        })
    }

    fn close(&mut self) {
        if let Some(mut d) = self.dev.take() {
            d.close();
            self.open
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&self.path);
        }
    }

    fn rate_hz(&self) -> Hz {
        self.dev.as_ref().map_or(0.0, Rtl2832::rate_hz)
    }

    fn centre_hz(&self) -> Hz {
        self.dev
            .as_ref()
            .and_then(Rtl2832::frequency)
            .map_or(0.0, |f| f as f64)
    }

    fn capabilities(&self) -> Capabilities {
        let Some(d) = self.dev.as_ref() else {
            return Capabilities::default();
        };
        let (min, max) = freq_range(d, self.ds_on && d.board() == crate::r82xx::Board::Generic);
        let gains: Vec<f64> = d.gains().iter().map(|&g| f64::from(g) / 10.0).collect();
        let (gmin, gmax) = (gains[0], gains[gains.len() - 1]);
        let r82 = d.tuner() != TunerKind::E4000;
        Capabilities {
            name: format!(
                "RTL2832U {} {}",
                d.tuner().name(),
                if self.product.is_empty() {
                    &self.path
                } else {
                    &self.product
                }
            ),
            rates: RateLimits {
                min_hz: 225_001.0,
                max_hz: 3.2e6,
                ranges: vec![(225_001.0, 300_000.0), (900_001.0, 3.2e6)],
                steady_hz: Some(STEADY_RATE),
            },
            min_freq_hz: min,
            max_freq_hz: max,
            gain_db: Some((gmin, gmax, 0.1)),
            bandwidth_hz: Some(if r82 { (250e3, 8e6) } else { (1e6, 27e6) }),
            has_agc: true,
            transmit: None,
            has_bias_tee: true,
            realtime: true,
            common_rates_hz: COMMON_RATES.to_vec(),
            common_bandwidths_hz: if r82 {
                R82XX_BANDWIDTHS.to_vec()
            } else {
                Vec::new()
            },
            gain_steps_db: gains,
        }
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cu8
    }

    fn live_rate_change(&self) -> bool {
        false
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        self.wanted(values).map(|_| ())
    }

    fn status(&self) -> String {
        match &self.dev {
            Some(d) => format!(
                "{} Msps, tuner {}{}",
                sdrkit_driver_core::fmt_m(d.rate_hz()),
                d.tuner().name(),
                if d.locked() || d.direct() != Direct::Off {
                    ""
                } else {
                    ", PLL not locked"
                }
            ),
            None => String::new(),
        }
    }

    fn readings(&mut self, results: &mut Results) {
        if let Some(d) = &self.dev {
            let mut f = results.fields_mut();
            f.set(&TUNER, d.tuner().name());
            f.set(&PLL_LOCKED, d.locked());
        }
    }
}

impl Drop for RtlRadio {
    fn drop(&mut self) {
        self.close();
    }
}

/// The read loop of an open dongle: bulk transfers go to the sink unchanged, apart from the bytes of a retune gap.
pub struct RtlStream {
    bulk: Option<Box<dyn BulkIn>>,
    abort: Arc<Abort>,
    mark: Arc<Mark>,
    seen: u64,
    gate: SettleGate,
    queued: usize,
    bytes: Arc<AtomicU64>,
}

impl RtlStream {
    fn read(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let Some(bulk) = self.bulk.as_mut() else {
            return Ok(());
        };
        let mut last = Instant::now();
        let mut errors = 0usize;
        let mut gap = 0usize;
        loop {
            if ctx.stopping() || self.abort.fired() || sink.closed() {
                return Ok(());
            }
            match bulk.next(POLL) {
                Ok(data) => {
                    let now = Instant::now();
                    // A retune that came while the read waited applies to this transfer.
                    let m = self.mark.seq();
                    if m != self.seen {
                        self.seen = m;
                        if m % 2 == 1 {
                            self.gate.begin();
                        } else {
                            self.gate.mark(self.mark.at());
                        }
                    }
                    last = now;
                    errors = 0;
                    self.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                    let k = self.gate.front(now, data.len());
                    gap += k;
                    if k < data.len() {
                        if gap > 0 {
                            sink.deliberate_gap((gap / 2) as u64);
                            gap = 0;
                        }
                        sink.write(&data[k..]);
                    }
                }
                Err(UsbError::Timeout) => {
                    if last.elapsed() >= STALL {
                        return Err(RtlError::NotHere("the dongle sends no samples").into());
                    }
                }
                Err(UsbError::Unplugged) => return Err(RtlError::from(UsbError::Unplugged).into()),
                Err(e) => {
                    errors += 1;
                    if errors >= self.queued {
                        return Err(RtlError::from(e).into());
                    }
                }
            }
        }
    }
}

impl RadioStream for RtlStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let r = self.read(sink, ctx);
        // Free the endpoint before the runner opens the dongle again.
        self.bulk = None;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfers_hold_whole_packets_and_half_a_second() {
        for rate in [250_000.0, 1.024e6, 2.4e6, 3.2e6] {
            let (size, queued) = transfer_plan(rate);
            assert_eq!(size % PACKET, 0);
            assert!((16_384..=262_144).contains(&size));
            assert!(
                size as f64 * queued as f64 >= rate * 2.0 * 0.5,
                "rate {rate}"
            );
        }
        assert_eq!(transfer_plan(2.4e6), (163_840, 15));
        assert_eq!(transfer_plan(3.2e6), (212_992, 16));
    }

    /// Feeds transfers of 48 000 bytes (10 ms at 4.8 MB/s); transfer `k` is read at `read_ms(k)`. A retune at 52 ms.
    fn run(read_ms: impl Fn(u64) -> f64, cap: usize) -> Vec<usize> {
        let t0 = Instant::now();
        let ms = |m: f64| t0 + Duration::from_secs_f64(m / 1e3);
        let mut g = SettleGate::new(t0, 4.8e6, cap);
        (1..=10)
            .map(|k| {
                let at = read_ms(k);
                if at > 52.0 && at - 10.0 <= 52.0 || (k == 6 && at > 52.0) {
                    g.mark(ms(52.0));
                }
                g.front(ms(at), 48_000)
            })
            .collect()
    }

    #[test]
    fn the_gate_drops_up_to_five_ms_after_the_retune() {
        // Read on time: transfer 6 (50 to 60 ms) loses 50 to 57 ms.
        let on_time = run(|k| k as f64 * 10.0, usize::MAX);
        assert_eq!(on_time, [0, 0, 0, 0, 0, 33_600, 0, 0, 0, 0]);
    }

    #[test]
    fn every_byte_is_dropped_while_a_retune_runs() {
        // The retune runs from 50 to 70 ms: transfers 6 and 7 go whole, transfer 8 (70 to 80 ms) loses 70 to 75 ms.
        let t0 = Instant::now();
        let ms = |m: f64| t0 + Duration::from_secs_f64(m / 1e3);
        let mut g = SettleGate::new(t0, 4.8e6, usize::MAX);
        let mut out = Vec::new();
        for k in 1..=10u32 {
            let at = f64::from(k) * 10.0;
            if k == 6 {
                g.begin();
            }
            if k == 8 {
                g.mark(ms(70.0));
            }
            out.push(g.front(ms(at), 48_000));
        }
        assert_eq!(out, [0, 0, 0, 0, 0, 48_000, 48_000, 24_000, 0, 0]);
    }

    #[test]
    fn a_late_read_does_not_shorten_the_gap() {
        // Transfer 6 is read 12.8 ms late and transfer 7 at once after it: the same bytes are dropped, less what the
        // delay estimate may rise in those 22.8 ms (at most 110 bytes).
        let late = run(
            |k| match k {
                6 => 72.8,
                7 => 72.9,
                _ => k as f64 * 10.0,
            },
            usize::MAX,
        );
        assert!((33_490..=33_600).contains(&late[5]), "{late:?}");
        assert_eq!(late.iter().sum::<usize>(), late[5]);
    }

    #[test]
    fn a_transfer_read_after_the_retune_is_dropped_whole_up_to_the_cap() {
        // Transfers 5 and 6 are read after the retune; the cap stops the gap at 60 000 bytes.
        let behind = run(
            |k| match k {
                5 => 55.0,
                6 => 60.0,
                _ => k as f64 * 10.0,
            },
            60_000,
        );
        assert_eq!(behind, [0, 0, 0, 0, 48_000, 12_000, 0, 0, 0, 0]);
    }
}
