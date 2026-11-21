//! A bank of LoRa channels in one capture, with their detectors.

use crate::channel::{self, offset_room};
use crate::error::{LoraError, Result};
use crate::receiver::ChannelRx;
use crate::rx::{BusyChange, DetectorConfig, DetectorStats, LoraFrame, LoraSymbols};
use sdrkit_algo_dsp::bank::OverlapSaveBank;
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};

/// The stop-band attenuation of the channel filters, dB.
const ATTEN_DB: f64 = 55.0;

/// One channel of a plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BankChannel {
    /// The centre frequency, Hz (absolute: the bank subtracts the centre of the capture).
    pub freq_hz: f64,
    /// The bandwidth, Hz.
    pub bw: f64,
    /// Its detectors.
    pub detectors: Vec<DetectorConfig>,
}

/// What the bank decodes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BankPlan {
    /// The channels.
    pub channels: Vec<BankChannel>,
    /// The shared channeliser is used from this many channels in band on (0: never).
    pub fft_from: usize,
}

/// The state of one detector, for reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DetectorState {
    /// Its tag.
    pub tag: u32,
    /// Its counters.
    pub stats: DetectorStats,
    /// It is inside a frame.
    pub busy: bool,
    /// Its channel is in band.
    pub in_band: bool,
}

/// The pass and stop edges of a channel filter: the band, the offset room, and 0.3 BW of transition.
fn edges(rate: f64, bw: f64) -> (f64, f64) {
    let pass = bw / 2.0 + offset_room(bw);
    let stop = (pass + 0.3 * bw).min(rate / 2.0);
    (pass, stop)
}

/// The bank.
pub struct Bank {
    rate: f64,
    centre: f64,
    plan: BankPlan,
    chans: Vec<Option<ChannelRx>>,
    shared: Option<OverlapSaveBank>,
    shared_out: Vec<Vec<Complex32>>,
    base: u64,
    changes: Vec<BusyChange>,
}

impl Bank {
    /// The bank for an input at `rate` centred on `centre_hz`, whose first sample has the index `base`. Channels out of band
    /// stay idle. If a detector configuration is not valid, this returns an error.
    pub fn new(rate: f64, centre_hz: f64, plan: BankPlan, base: u64) -> Result<Self> {
        let inband: Vec<bool> = plan
            .channels
            .iter()
            .map(|c| channel::in_band(rate, c.freq_hz - centre_hz, c.bw))
            .collect();
        let count = inband.iter().filter(|&&b| b).count();
        let mut shared = None;
        if plan.fft_from > 0 && count >= plan.fft_from {
            // The narrowest transition band sets the FFT size.
            let transition = plan
                .channels
                .iter()
                .zip(&inband)
                .filter(|(_, &ok)| ok)
                .map(|(c, _)| {
                    let (p, s) = edges(rate, c.bw);
                    s - p
                })
                .fold(f64::INFINITY, f64::min);
            let size = OverlapSaveBank::size_for(rate, transition, ATTEN_DB).max(1024);
            shared = Some(OverlapSaveBank::new(rate, size).map_err(LoraError::Dsp)?);
        }
        let mut chans = Vec::with_capacity(plan.channels.len());
        for (c, &ok) in plan.channels.iter().zip(&inband) {
            if !ok {
                chans.push(None);
                continue;
            }
            let off = c.freq_hz - centre_hz;
            let rx = match &mut shared {
                Some(b) => {
                    let (pass, stop) = edges(rate, c.bw);
                    let i = b
                        .add_channel(off, pass, stop, ATTEN_DB)
                        .map_err(LoraError::Dsp)?;
                    ChannelRx::fed(
                        rate,
                        off,
                        c.bw,
                        b.output_rate(i),
                        b.delay(i),
                        c.detectors.clone(),
                    )?
                }
                None => ChannelRx::new(rate, off, c.bw, c.detectors.clone())?,
            };
            chans.push(Some(rx));
        }
        let outs = shared.as_ref().map_or(0, |b| b.len());
        Ok(Bank {
            rate,
            centre: centre_hz,
            plan,
            chans,
            shared,
            shared_out: vec![Vec::new(); outs],
            base,
            changes: Vec::with_capacity(16),
        })
    }

    /// The plan.
    pub fn plan(&self) -> &BankPlan {
        &self.plan
    }

    /// The number of channels in band.
    pub fn in_band(&self) -> usize {
        self.chans.iter().filter(|c| c.is_some()).count()
    }

    /// The number of detectors that run.
    pub fn demodulators(&self) -> usize {
        self.chans
            .iter()
            .flatten()
            .map(|c| c.detectors().len())
            .sum()
    }

    /// Feeds input. Decoded frames go to `out`, with absolute input indices and the frequency of their channel.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<(LoraFrame, LoraSymbols)>) {
        let start = out.len();
        match &mut self.shared {
            Some(b) => {
                for o in &mut self.shared_out {
                    o.clear();
                }
                if b.process(input, &mut self.shared_out).is_ok() {
                    for (c, o) in self.chans.iter_mut().flatten().zip(&self.shared_out) {
                        c.process_channel(o, input.len(), out);
                    }
                }
            }
            None => {
                for c in self.chans.iter_mut().flatten() {
                    c.process(input, out);
                }
            }
        }
        self.changes.clear();
        let mut first = 0;
        for (c, rx) in self.plan.channels.iter().zip(&self.chans) {
            if let Some(rx) = rx {
                for ch in rx.busy_changes() {
                    self.changes.push(BusyChange {
                        at: ch.at + self.base,
                        detector: first + ch.detector,
                        channel_hz: c.freq_hz,
                        ..*ch
                    });
                }
            }
            first += c.detectors.len();
        }
        self.changes.sort_unstable_by_key(|c| (c.at, c.detector));
        for (f, _) in &mut out[start..] {
            f.start += self.base;
            f.end += self.base;
            // Each frame carries the frequency of its channel.
            if let Some(c) = self
                .plan
                .channels
                .iter()
                .find(|c| c.bw == f.bw && c.detectors.iter().any(|d| d.tag == f.tag))
            {
                f.channel_hz = c.freq_hz;
            }
        }
    }

    /// The state of every detector of the plan. Detectors of channels out of band have zero counts.
    pub fn states(&self) -> Vec<DetectorState> {
        let mut v = Vec::new();
        for (c, rx) in self.plan.channels.iter().zip(&self.chans) {
            match rx {
                Some(rx) => {
                    for d in rx.detectors() {
                        v.push(DetectorState {
                            tag: d.config().tag,
                            stats: *d.stats(),
                            busy: d.busy(),
                            in_band: true,
                        });
                    }
                }
                None => {
                    for d in &c.detectors {
                        v.push(DetectorState {
                            tag: d.tag,
                            in_band: false,
                            ..Default::default()
                        });
                    }
                }
            }
        }
        v
    }

    /// True if a detector is inside a frame.
    pub fn busy(&self) -> bool {
        self.chans.iter().flatten().any(|c| c.busy())
    }

    /// The busy changes of every detector during the last call of [`Bank::process`], in input order. Each names the
    /// detector by its place in [`Bank::states`], with the absolute input sample index and the channel's frequency. The state
    /// after the last change of a detector is its state in [`Bank::states`].
    pub fn busy_changes(&self) -> &[BusyChange] {
        &self.changes
    }

    /// The input rate and centre, Hz.
    pub fn input(&self) -> (f64, f64) {
        (self.rate, self.centre)
    }

    /// The bytes the rings hold.
    pub fn memory_bytes(&self) -> usize {
        self.chans.iter().flatten().map(|c| c.memory_bytes()).sum()
    }
}
