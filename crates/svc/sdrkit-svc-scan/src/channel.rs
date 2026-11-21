//! The channel scan: it visits every channel of a plan or a raster once, lets the [`LockTest`] of the stack decide each one,
//! and fills a [`ScanTable`].

use crate::host::{EndReason, Progress, ScanMachine, ScanState, Spectrum, Step, Target};
use crate::lock::{Channel, EmptyReason, LockTest, Verdict};
use crate::observe::Observation;
use crate::table::{ScanRow, ScanTable};
use sdrkit_core::Results;

/// How a channel scan behaves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelScanConfig {
    /// Keep the rows of empty channels in the table, with the reason.
    pub keep_empty: bool,
    /// Added to the clock of the scan to give seconds since 1970.
    pub clock_offset_s: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Settle { until: f64 },
    Test { since: f64 },
    Done,
}

/// A channel scan.
pub struct ChannelScanner {
    channels: Vec<Channel>,
    test: Box<dyn LockTest>,
    cfg: ChannelScanConfig,
    index: usize,
    phase: Phase,
    table: ScanTable,
    found: usize,
    stopped: bool,
    /// The width in which the power of each channel is measured, Hz.
    widths: Vec<f64>,
    /// The channels that a spectrum showed without a signal. They are not tuned.
    quiet: Vec<bool>,
    /// The channels left out so far because a spectrum showed no signal.
    skipped: usize,
    /// The bins of a spectrum in order of power, kept between frames.
    sorted: Vec<f32>,
    /// The channels in the last spectrum with their power, dB, kept between frames.
    seen: Vec<(usize, f32)>,
}

/// The part of a spectrum, from its middle, in which channels are judged: the edges are in the slope of the radio's filter.
const USED_SPAN: f64 = 0.9;
/// The share of the bins of a spectrum that are taken as below the noise floor.
const FLOOR_SHARE: f64 = 0.1;
/// A channel has a signal if its power is this far above the noise floor of the spectrum, dB.
const SIGNAL_DB: f32 = 8.0;
/// A channel this far below the channel next to it holds the side of that signal and none of its own, dB.
const SIDE_DB: f32 = 4.0;
/// The bins this close to the middle of a spectrum are left out: a radio has a carrier of its own there, Hz.
const CENTRE_GUARD_HZ: f64 = 10e3;

/// The width in which the power of each channel is measured: its bandwidth, or the distance to the nearest other channel.
/// 0 for a channel that has neither: it is always tuned.
fn widths(channels: &[Channel]) -> Vec<f64> {
    channels
        .iter()
        .enumerate()
        .map(|(i, c)| {
            c.bandwidth_hz.filter(|w| *w > 0.0).unwrap_or_else(|| {
                let gap = |j: Option<usize>| {
                    j.and_then(|j| channels.get(j))
                        .map(|o| (o.freq_hz - c.freq_hz).abs())
                        .filter(|d| *d > 0.0)
                };
                match (gap(i.checked_sub(1)), gap(Some(i + 1))) {
                    (Some(a), Some(b)) => a.min(b),
                    (a, b) => a.or(b).unwrap_or(0.0),
                }
            })
        })
        .collect()
}

impl std::fmt::Debug for ChannelScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelScanner")
            .field("channels", &self.channels.len())
            .field("index", &self.index)
            .field("phase", &self.phase)
            .field("found", &self.found)
            .finish()
    }
}

impl ChannelScanner {
    /// A scan of `channels` judged by `test`.
    pub fn new(
        channels: Vec<Channel>,
        test: Box<dyn LockTest>,
        cfg: ChannelScanConfig,
    ) -> ChannelScanner {
        ChannelScanner {
            widths: widths(&channels),
            quiet: vec![false; channels.len()],
            skipped: 0,
            sorted: Vec::new(),
            seen: Vec::new(),
            channels,
            test,
            cfg,
            index: 0,
            phase: Phase::Done,
            table: ScanTable::default(),
            found: 0,
            stopped: false,
        }
    }

    /// What was found so far.
    pub fn table(&self) -> &ScanTable {
        &self.table
    }

    /// The table, when the scan is over.
    pub fn into_table(self) -> ScanTable {
        self.table
    }

    /// Channels with a station so far.
    pub fn found(&self) -> usize {
        self.found
    }

    /// Ends the scan.
    pub fn stop(&mut self) -> Step {
        self.stopped = true;
        self.phase = Phase::Done;
        Step::Done
    }

    /// Channels left out because a spectrum showed no signal on them.
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    fn tune(&mut self, now: f64) -> Step {
        while self.quiet.get(self.index) == Some(&true) {
            self.record(now, Verdict::Empty(EmptyReason::NoSignal));
            self.skipped += 1;
            self.index += 1;
        }
        let Some(c) = self.channels.get(self.index) else {
            self.phase = Phase::Done;
            return Step::Done;
        };
        self.test.begin(c);
        self.phase = Phase::Settle {
            until: now + self.test.settle_s(),
        };
        Step::Tune
    }

    fn record(&mut self, now: f64, verdict: Verdict) {
        let c = &self.channels[self.index];
        let at = now + self.cfg.clock_offset_s;
        let mut row = ScanRow {
            freq_hz: c.freq_hz,
            stack: c.stack.clone(),
            channel: c.name.clone(),
            first_seen_s: at,
            last_seen_s: at,
            seen: 1,
            settings: c.settings.clone(),
            ..ScanRow::default()
        };
        match verdict {
            Verdict::Locked(f) => {
                self.found += 1;
                row.locked = true;
                row.name = f.name;
                row.snr_db = f.snr_db;
                row.level_dbfs = f.level_dbfs;
                row.values = f
                    .values
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), v))
                    .collect();
            }
            Verdict::Empty(why) => {
                if !self.cfg.keep_empty {
                    return;
                }
                row.empty = Some(why);
            }
            Verdict::Pending => return,
        }
        self.table.upsert(row);
    }
}

impl ScanMachine for ChannelScanner {
    fn start(&mut self, now: f64) -> Step {
        self.index = 0;
        self.found = 0;
        self.skipped = 0;
        self.quiet.fill(false);
        self.stopped = false;
        self.tune(now)
    }

    /// Marks the channels from the tuned one on that lie whole in the spectrum and show no signal. The tuned channel, if
    /// it is one of them, is empty at once. A spectrum that comes while the radio settles is not used: it can hold samples
    /// of the channel before.
    fn spectrum(&mut self, now: f64, s: &Spectrum<'_>) {
        let Phase::Test { .. } = self.phase else {
            return;
        };
        let n = s.power_db.len();
        if n < 16 || s.span_hz <= 0.0 {
            return;
        }
        self.sorted.clear();
        self.sorted.extend_from_slice(s.power_db);
        self.sorted.sort_unstable_by(f32::total_cmp);
        let floor = self.sorted[(n as f64 * FLOOR_SHARE) as usize];
        let bin = s.span_hz / n as f64;
        let low = s.centre_hz - s.span_hz / 2.0;
        let half_used = s.span_hz * USED_SPAN / 2.0;
        self.seen.clear();
        for i in self.index..self.channels.len() {
            let (f, w) = (self.channels[i].freq_hz, self.widths[i]);
            if w <= 0.0 || (f - s.centre_hz).abs() + w / 2.0 > half_used {
                // The channels are in order of frequency in most plans, but not in all: look at every one.
                continue;
            }
            let (a, b) = (
                ((f - w / 2.0 - low) / bin).floor().max(0.0) as usize,
                (((f + w / 2.0 - low) / bin).ceil() as usize).min(n),
            );
            let (mut sum, mut count) = (0.0f64, 0u32);
            for (k, p) in s.power_db.iter().enumerate().take(b).skip(a) {
                let at = low + (k as f64 + 0.5) * bin;
                if (at - s.centre_hz).abs() >= CENTRE_GUARD_HZ {
                    sum += 10f64.powf(f64::from(*p) / 10.0);
                    count += 1;
                }
            }
            if count > 0 {
                self.seen
                    .push((i, (10.0 * (sum / f64::from(count)).log10()) as f32));
            }
        }
        // No signal: near the noise floor, or the side of a stronger signal on the channel next to it.
        for k in 0..self.seen.len() {
            let (i, p) = self.seen[k];
            let beside = |j: Option<usize>| {
                j.and_then(|j| self.seen.get(j))
                    .filter(|(o, _)| o.abs_diff(i) == 1)
                    .is_some_and(|(_, q)| p < q - SIDE_DB)
            };
            if p <= floor + SIGNAL_DB || beside(k.checked_sub(1)) || beside(Some(k + 1)) {
                self.quiet[i] = true;
            }
        }
        let _ = now;
    }

    fn tick(&mut self, now: f64, obs: &Observation, results: Option<&Results>) -> Option<Step> {
        match self.phase {
            Phase::Done => None,
            Phase::Settle { until } => {
                if now >= until {
                    self.phase = Phase::Test { since: now };
                }
                None
            }
            Phase::Test { .. } if self.quiet.get(self.index) == Some(&true) => Some(self.tune(now)),
            Phase::Test { since } => match self.test.judge(now - since, obs, results) {
                Verdict::Pending => None,
                v => {
                    self.record(now, v);
                    self.index += 1;
                    Some(self.tune(now))
                }
            },
        }
    }

    fn audio_wanted(&self) -> bool {
        false
    }

    fn finished(&self) -> bool {
        self.phase == Phase::Done
    }

    fn target(&self) -> Option<Target<'_>> {
        let c = self
            .channels
            .get(self.index)
            .filter(|_| self.phase != Phase::Done)?;
        Some(Target {
            freq_hz: c.freq_hz,
            stack: &c.stack,
            submode: "",
            name: &c.name,
            settings: &c.settings,
            bandwidth_hz: c.bandwidth_hz,
            rate_hz: None,
        })
    }

    fn progress(&self) -> Progress {
        let (state, end) = match self.phase {
            Phase::Settle { .. } => (ScanState::Settle, EndReason::None),
            Phase::Test { .. } => (ScanState::Test, EndReason::None),
            Phase::Done if self.stopped => (ScanState::Done, EndReason::Stopped),
            Phase::Done => (ScanState::Done, EndReason::Complete),
        };
        Progress {
            state,
            index: self.index,
            count: self.channels.len(),
            at_priority: false,
            pass: 0,
            steps: self.index as u64,
            found: self.found,
            end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::Found;

    const STATIONS: [f64; 3] = [92.9e6, 104.9e6, 106.5e6];

    /// A station locks after 2 s. An empty channel is left after 1.5 s.
    struct Fm(f64);

    impl LockTest for Fm {
        fn begin(&mut self, c: &Channel) {
            self.0 = c.freq_hz;
        }
        fn settle_s(&self) -> f64 {
            0.2
        }
        fn judge(&mut self, elapsed_s: f64, _: &Observation, _: Option<&Results>) -> Verdict {
            let station = STATIONS.iter().any(|s| (s - self.0).abs() < 1.0);
            match (station, elapsed_s) {
                (true, t) if t >= 2.0 => Verdict::Locked(Found::default()),
                (false, t) if t >= 1.5 => Verdict::Empty(EmptyReason::NoSignal),
                _ => Verdict::Pending,
            }
        }
    }

    /// A spectrum 2 MHz wide around `centre`: noise at -100 dB, each station 150 kHz wide at -60 dB.
    fn spectrum(centre: f64) -> Vec<f32> {
        (0..1024)
            .map(|k| {
                let f = centre - 1e6 + (f64::from(k) + 0.5) * 2e6 / 1024.0;
                if STATIONS.iter().any(|s| (s - f).abs() < 75e3) {
                    -60.0
                } else {
                    -100.0
                }
            })
            .collect()
    }

    /// Scans the FM band on its raster of 100 kHz. Returns the tunes, the seconds and the scanner.
    fn scan(with_spectrum: bool) -> (Vec<f64>, f64, ChannelScanner) {
        let channels = Channel::raster(87.5e6, 108e6, 100e3, "fm");
        let mut s = ChannelScanner::new(channels, Box::new(Fm(0.0)), ChannelScanConfig::default());
        let (mut t, mut tunes) = (0.0, Vec::new());
        let mut step = Some(s.start(t));
        while step != Some(Step::Done) && t < 1000.0 {
            if step == Some(Step::Tune) {
                tunes.push(s.target().unwrap().freq_hz);
            }
            t += 0.1;
            if let (true, Some(at)) = (with_spectrum, tunes.last()) {
                let bins = spectrum(*at);
                s.spectrum(
                    t,
                    &Spectrum {
                        centre_hz: *at,
                        span_hz: 2e6,
                        power_db: &bins,
                    },
                );
            }
            step = s.tick(t, &Observation::default(), None);
        }
        (tunes, t, s)
    }

    #[test]
    fn a_spectrum_leaves_out_the_channels_without_a_signal() {
        let (tunes, seconds, s) = scan(true);
        assert_eq!(s.found(), 3);
        let found: Vec<f64> = s.table().locked().map(|r| r.freq_hz).collect();
        assert_eq!(found, STATIONS);
        // Every station is tuned. The band between them costs one short tune for each 0.9 MHz: the half of the spectrum
        // above the tuned channel.
        assert!(STATIONS.iter().all(|f| tunes.contains(f)));
        assert!(tunes.len() <= 3 + 24, "{} tunes", tunes.len());
        assert!(seconds < 20.0, "{seconds} s");
    }

    #[test]
    fn without_a_spectrum_every_channel_is_tuned() {
        let (tunes, seconds, s) = scan(false);
        assert_eq!((s.found(), tunes.len(), s.skipped()), (3, 206, 0));
        assert!(seconds > 300.0);
    }
}
