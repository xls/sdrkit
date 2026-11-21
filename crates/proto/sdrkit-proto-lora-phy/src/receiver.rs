//! One channel of a capture with its detectors: shift, filter and decimate, resample to twice the chip rate, then run every
//! detector on the chip stream.

use crate::channel::{ChannelFilter, ChipResampler};
use crate::chips::{self, ChipRings};
use crate::error::Result;
use crate::rx::{BusyChange, Detector, DetectorConfig, LoraFrame, LoraSymbols};
use sdrkit_core::Complex32;

/// One channel with its detectors.
///
/// ```
/// use sdrkit_proto_lora_phy::{ChannelRx, DetectorConfig, LoraParams, TxFrame};
/// let p = LoraParams::new(7, 125e3, 5, 8, 0x12);
/// let frame = TxFrame::new(&p, b"hello").unwrap().at(0.01);
/// let x = frame.render(250e3, 0.02);
/// let mut rx = ChannelRx::new(250e3, 0.0, 125e3, vec![DetectorConfig::new(7, 125e3, 0x12)]).unwrap();
/// let mut out = Vec::new();
/// rx.process(&x, &mut out);
/// assert_eq!(out[0].0.payload, b"hello");
/// assert!(out[0].0.crc_ok);
/// ```
pub struct ChannelRx {
    /// The offset of the channel in the input, Hz.
    pub offset_hz: f64,
    /// The bandwidth, Hz.
    pub bw: f64,
    input_rate: f64,
    filter: Option<ChannelFilter>,
    chips: ChipResampler,
    rings: ChipRings,
    detectors: Vec<Detector>,
    tmp: Vec<Complex32>,
    tmp2: Vec<Complex32>,
    found: Vec<(LoraFrame, LoraSymbols)>,
    changes: Vec<BusyChange>,
    delay_in: f64,
    /// The input samples seen.
    pub input_count: u64,
}

impl ChannelRx {
    /// The channel at `offset_hz` of `bw` in an input at `rate`, with these detectors. If a value is out of range, this
    /// returns an error.
    pub fn new(rate: f64, offset_hz: f64, bw: f64, detectors: Vec<DetectorConfig>) -> Result<Self> {
        let filter = ChannelFilter::new(rate, offset_hz, bw)?;
        let (crate_, d_delay) = (filter.output_rate(), filter.delay());
        let chips = ChipResampler::new(crate_, bw)?;
        let delay_in = d_delay + chips.delay() * rate / crate_;
        Self::build(
            rate,
            offset_hz,
            bw,
            Some(filter),
            chips,
            delay_in,
            detectors,
        )
    }

    /// A channel whose samples come from a shared channeliser at `channel_rate`, with this delay in input samples. Feed it with
    /// [`process_channel`](Self::process_channel).
    pub fn fed(
        rate: f64,
        offset_hz: f64,
        bw: f64,
        channel_rate: f64,
        delay_in: f64,
        detectors: Vec<DetectorConfig>,
    ) -> Result<Self> {
        let chips = ChipResampler::new(channel_rate, bw)?;
        let delay = delay_in + chips.delay() * rate / channel_rate;
        Self::build(rate, offset_hz, bw, None, chips, delay, detectors)
    }

    fn build(
        rate: f64,
        offset_hz: f64,
        bw: f64,
        filter: Option<ChannelFilter>,
        chips: ChipResampler,
        delay_in: f64,
        detectors: Vec<DetectorConfig>,
    ) -> Result<Self> {
        let mut rings = ChipRings::new()?;
        rings.reset(0);
        let start = rings.x1_first;
        let detectors = detectors
            .into_iter()
            .map(|c| Detector::new(c, start))
            .collect::<Result<Vec<_>>>()?;
        Ok(ChannelRx {
            offset_hz,
            bw,
            input_rate: rate,
            filter,
            chips,
            rings,
            detectors,
            tmp: Vec::new(),
            tmp2: Vec::new(),
            found: Vec::new(),
            changes: Vec::with_capacity(16),
            delay_in,
            input_count: 0,
        })
    }

    /// The detectors.
    pub fn detectors(&self) -> &[Detector] {
        &self.detectors
    }

    /// True if a detector is inside a frame.
    pub fn busy(&self) -> bool {
        self.detectors.iter().any(|d| d.busy())
    }

    /// The busy changes of every detector during the last call of [`ChannelRx::process`] or
    /// [`ChannelRx::process_channel`], in input order. `channel_hz` is 0: a [`crate::Bank`] fills it in.
    pub fn busy_changes(&self) -> &[BusyChange] {
        &self.changes
    }

    /// Feeds input samples. Decoded frames go to `out`, with their start and end as input sample indices.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<(LoraFrame, LoraSymbols)>) {
        self.tmp.clear();
        match &mut self.filter {
            Some(f) => f.process(input, &mut self.tmp),
            None => self.tmp.extend_from_slice(input),
        }
        self.input_count += input.len() as u64;
        self.after_channel(out);
    }

    /// Feeds channel samples from a shared channeliser. `input_len` is the number of input samples they cover.
    pub fn process_channel(
        &mut self,
        channel: &[Complex32],
        input_len: usize,
        out: &mut Vec<(LoraFrame, LoraSymbols)>,
    ) {
        self.tmp.clear();
        self.tmp.extend_from_slice(channel);
        self.input_count += input_len as u64;
        self.after_channel(out);
    }

    fn after_channel(&mut self, out: &mut Vec<(LoraFrame, LoraSymbols)>) {
        self.tmp2.clear();
        self.chips.process(&self.tmp, &mut self.tmp2);
        self.rings.push(&self.tmp2);
        self.found.clear();
        self.changes.clear();
        let per_chip = self.input_rate / self.bw;
        for (k, d) in self.detectors.iter_mut().enumerate() {
            d.run(&self.rings, &mut self.found);
            for &(chip, busy) in d.busy_changes() {
                self.changes.push(BusyChange {
                    at: (chip * per_chip - self.delay_in).max(0.0).round() as u64,
                    detector: k,
                    tag: d.config().tag,
                    channel_hz: 0.0,
                    busy,
                });
            }
        }
        if self.detectors.len() > 1 {
            self.changes.sort_unstable_by_key(|c| (c.at, c.detector));
        }
        for (mut f, s) in self.found.drain(..) {
            f.start = (f.start as f64 * per_chip - self.delay_in).max(0.0).round() as u64;
            f.end = (f.end as f64 * per_chip - self.delay_in).max(0.0).round() as u64;
            out.push((f, s));
        }
        let keep = self
            .detectors
            .iter()
            .map(|d| d.needs_from())
            .fold(f64::INFINITY, f64::min);
        if keep.is_finite() {
            let k2 = (2.0 * keep - chips::TAPS as f64 - 8.0).max(0.0) as u64;
            self.rings.trim(k2);
        }
    }

    /// Forgets the signal, after a retune or a reset. The filters, the rings and every detector start again. The detector
    /// counters stay.
    pub fn reset(&mut self) -> Result<()> {
        let cfgs: Vec<DetectorConfig> = self.detectors.iter().map(|d| d.config().clone()).collect();
        let stats: Vec<_> = self.detectors.iter().map(|d| *d.stats()).collect();
        let mut fresh = match self.filter {
            Some(_) => ChannelRx::new(self.input_rate, self.offset_hz, self.bw, cfgs)?,
            None => {
                let chips = self.chips.clone();
                Self::build(
                    self.input_rate,
                    self.offset_hz,
                    self.bw,
                    None,
                    chips,
                    self.delay_in,
                    cfgs,
                )?
            }
        };
        for (d, s) in fresh.detectors.iter_mut().zip(stats) {
            d.set_stats(s);
        }
        *self = fresh;
        Ok(())
    }

    /// The bytes the rings hold.
    pub fn memory_bytes(&self) -> usize {
        (self.rings.x1.capacity() + self.rings.x2.capacity()) * std::mem::size_of::<Complex32>()
    }
}
