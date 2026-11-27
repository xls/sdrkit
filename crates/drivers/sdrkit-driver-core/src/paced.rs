//! Paced writing for sources that are not radios (a file, a generator): real time times a pace, and no sample dropped.

use crate::RingWriter;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Hz};
use sdrkit_core::{Setting, Values};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The speed of the playback: 1 is real time, lower values are slow motion.
pub const PACE: Setting<field::F64> = Setting::new("pace", "Speed", "x real time", 1.0)
    .range(0.05, 1.0, 0.05)
    .section(crate::form::OTHER_SECTION)
    .help("1 plays in real time. Lower values play in slow motion, for slow test machines.");
/// Play as fast as the reader takes the samples.
pub const BENCHMARK: Setting<field::Bool> =
    Setting::new("benchmark", "As fast as possible", "", false)
        .section(crate::form::OTHER_SECTION)
        .help("Plays as fast as the receiver takes the samples, to measure its speed.");

/// Samples per block for files and generators.
pub const BLOCK: usize = 32_768;

/// The most a benchmark run keeps queued in the ring.
pub const BENCHMARK_QUEUE: usize = 1 << 21;

/// How a file or a generator is paced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pacing {
    /// Real time x pace (1 real time, below 1 slow motion).
    RealTime(f64),
    /// As fast as the consumer takes, with at most [`BENCHMARK_QUEUE`] samples queued.
    Benchmark,
}

impl Pacing {
    /// The pacing that [`PACE`] and [`BENCHMARK`] of `values` give.
    pub fn of(values: &Values) -> Pacing {
        if values.get(&BENCHMARK) {
            Pacing::Benchmark
        } else {
            Pacing::RealTime(values.get(&PACE))
        }
    }
}

/// The time of a paced writer: the clock it reads and how it waits.
pub(crate) trait Time: Send + Sync {
    fn now(&self) -> Instant;
    fn sleep(&self, d: Duration);
}

/// The system clock.
pub(crate) struct SystemClock;

impl Time for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// The longest single wait of a writer, so that a stop or a cut ends a wait in time.
pub(crate) const SLICE: Duration = Duration::from_millis(20);

/// Writes blocks into the ring at a pace, waiting for room (1 ms steps) so nothing is ever dropped.
pub struct PacedWriter {
    writer: RingWriter,
    rate: Hz,
    pacing: Pacing,
    anchor: Instant,
    sent: u64,
    time: Arc<dyn Time>,
}

impl PacedWriter {
    /// A writer at `rate_hz`; the clock is anchored now.
    pub fn new(writer: RingWriter, rate_hz: Hz, pacing: Pacing) -> Self {
        Self::with_time(writer, rate_hz, pacing, Arc::new(SystemClock))
    }

    /// A writer at `rate_hz` on the clock `time`, anchored now.
    pub(crate) fn with_time(
        writer: RingWriter,
        rate_hz: Hz,
        pacing: Pacing,
        time: Arc<dyn Time>,
    ) -> Self {
        PacedWriter {
            writer,
            rate: rate_hz,
            pacing,
            anchor: time.now(),
            sent: 0,
            time,
        }
    }

    /// Starts the clock again (a new rate or pace).
    pub fn reanchor(&mut self, rate_hz: Hz, pacing: Pacing) {
        self.rate = rate_hz;
        self.pacing = pacing;
        self.anchor = self.time.now();
        self.sent = 0;
    }

    /// Writes the whole native samples of `bytes`, waiting for room and then for their time. It returns false if the source must
    /// stop (`stop` is set, or the reader was dropped).
    pub fn push(&mut self, bytes: &[u8], stop: &AtomicBool) -> bool {
        let size = crate::sample::size(self.writer.sample_type());
        self.push_with(bytes.len() / size, stop, |w, from, n| {
            w.write(&bytes[from * size..(from + n) * size])
        })
    }

    /// Writes `iq` into a [`SampleType::Cf32`](sdrkit_graph::stream::SampleType::Cf32) ring, as [`PacedWriter::push`] does.
    pub fn push_iq(&mut self, iq: &[Complex32], stop: &AtomicBool) -> bool {
        self.push_with(iq.len(), stop, |w, from, n| w.write_iq(&iq[from..from + n]))
    }

    fn push_with(
        &mut self,
        len: usize,
        stop: &AtomicBool,
        mut write: impl FnMut(&mut RingWriter, usize, usize) -> usize,
    ) -> bool {
        let limit = match self.pacing {
            Pacing::Benchmark => BENCHMARK_QUEUE.min(self.writer.capacity()),
            Pacing::RealTime(_) => self.writer.capacity(),
        };
        let mut done = 0;
        while done < len {
            if stop.load(Ordering::Acquire) || self.writer.closed() {
                return false;
            }
            let queued = self.writer.capacity() - self.writer.room();
            let room = limit.saturating_sub(queued).min(self.writer.room());
            if room == 0 {
                self.time.sleep(Duration::from_millis(1));
                continue;
            }
            done += write(&mut self.writer, done, (len - done).min(room));
        }
        self.sent += len as u64;
        if let Pacing::RealTime(pace) = self.pacing {
            let pace = if pace > 0.0 { pace } else { 1.0 };
            if self.rate > 0.0 {
                let due =
                    self.anchor + Duration::from_secs_f64(self.sent as f64 / (self.rate * pace));
                loop {
                    if stop.load(Ordering::Acquire) {
                        return false;
                    }
                    let now = self.time.now();
                    if now >= due {
                        break;
                    }
                    self.time.sleep((due - now).min(SLICE));
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring;
    use sdrkit_graph::stream::SampleType;

    #[test]
    fn real_time_pacing_takes_about_the_signal_time() {
        let (w, mut r) = ring(1 << 20, SampleType::Cf32);
        let mut p = PacedWriter::new(w, 1e6, Pacing::RealTime(1.0));
        let stop = AtomicBool::new(false);
        let t = Instant::now();
        for _ in 0..8 {
            assert!(p.push_iq(&[Complex32::default(); 25_000], &stop));
        }
        let e = t.elapsed().as_secs_f64();
        assert!((0.19..0.35).contains(&e), "{e}");
        assert_eq!(r.skip_all(), 200_000);
    }

    #[test]
    fn a_full_ring_makes_the_writer_wait_not_drop() {
        let (w, mut r) = ring(1000, SampleType::Cf32);
        let mut p = PacedWriter::new(w, 0.0, Pacing::Benchmark);
        let stop = AtomicBool::new(false);
        let h = std::thread::spawn(move || {
            let block: Vec<_> = (0..5000).map(|i| Complex32::new(i as f32, 0.0)).collect();
            p.push_iq(&block, &stop)
        });
        let mut got = Vec::new();
        let mut buf = [Complex32::default(); 300];
        while got.len() < 5000 {
            r.wait(1, Duration::from_millis(100));
            let n = r.read_iq(&mut buf);
            got.extend(buf[..n].iter().map(|c| c.re));
        }
        assert!(h.join().unwrap());
        assert!(got.iter().enumerate().all(|(i, &v)| v == i as f32));
        assert_eq!(r.stats().dropped(), 0);
    }
}
