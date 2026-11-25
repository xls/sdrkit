//! The Zadoff-Chu pair trigger: a continuous search for the two DroneID symbols (roots 600 and 147, 2192 samples at
//! 15.36 Msps apart) on the DroneID frequencies inside the capture. It does not use the burst detector: a burst near the
//! noise floor or cut by the edge of the capture is found too. Each frequency is mixed to 0 Hz and decimated to about
//! 4 Msps, where two FFT correlators look for the two symbols. A pair starts the burst receiver on the samples of the
//! burst.

use super::droneid::{self, BurstDecode, BurstReceiver, FS, NFFT};
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::{resample_span, SpanResampler};
use sdrkit_algo_modem::correlate::{SampleSyncSet, SyncPeak};
use sdrkit_core::Complex32;
use std::ops::Range;

/// The rate of the search, Hz: the capture rate is divided by a whole number to come near it.
const SEARCH_RATE_HZ: f64 = 4e6;
/// The FFT size of the correlators.
const SEARCH_FFT: usize = 2048;
/// The lowest metric of each symbol in the search. For noise alone the metric of a template of `L` samples is above `g`
/// with the probability `(1 - g)^(L - 1)`: about 1 in 100 000 positions at 4 Msps.
pub const PAIR_THRESHOLD: f32 = 0.04;
/// The part of the capture that a radio's filter leaves flat, as a share of the rate on each side of the centre.
pub const EDGE: f64 = 0.47;
/// The search of a DroneID frequency needs this much of the burst on each side of the search centre, inside the flat part of
/// the capture. The search centre is the centre of the burst, or as near to it as the edge of the capture allows.
const SEARCH_HALF_WIDTH_HZ: f64 = 2e6;
/// A DroneID frequency is searched when at least this share of its carriers is inside the flat part of the capture. The code
/// has a rate of about 1/5, so a burst with a third of its carriers still decodes at a good level.
const SEARCH_VISIBLE: f64 = 0.35;
/// The width of a DroneID burst, Hz.
const WIDTH_HZ: f64 = 9e6;
/// Samples at 15.36 Msps from the prefix of symbol 4 to the prefix of symbol 6.
const PAIR_SPACING: f64 = 2.0 * (NFFT as f64 + 72.0);
/// Samples at 15.36 Msps around the burst that the receiver gets.
const MARGIN: f64 = 300.0;
/// Bursts that wait for their last samples, at most.
const PENDING: usize = 8;
/// Peaks of one symbol that wait for the other symbol, at most.
const WAITING: usize = 32;

/// One burst that the trigger found.
#[derive(Clone, Debug)]
pub struct Hit {
    /// The start of symbol 2, seconds of signal since the trigger was made.
    pub start_s: f64,
    /// The DroneID frequency that was searched, as an offset from the capture centre, Hz.
    pub offset_hz: f64,
    /// The metrics of the two symbols in the search.
    pub pair: [f32; 2],
    /// The share of the carriers inside the capture.
    pub visible: f64,
    /// The level of the burst, dBFS.
    pub level_dbfs: f64,
    /// What the burst receiver found.
    pub decode: BurstDecode,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    /// The capture sample where the prefix of symbol 4 starts.
    at: u64,
    channel: usize,
    pair: [f32; 2],
}

/// The search on one DroneID frequency.
struct Channel {
    offset_hz: f64,
    nco: Option<Nco>,
    dec: Decimator<Complex32>,
    /// The correlators of the two symbols, on one forward FFT.
    sync: SampleSyncSet,
    waiting: [Vec<SyncPeak>; 2],
    on: Range<usize>,
    low: Vec<Complex32>,
    /// The peaks of each symbol in the samples searched last.
    peaks: [Vec<SyncPeak>; 2],
    /// Decimated samples searched so far.
    fed: u64,
}

/// The pair trigger of a capture.
pub struct PairTrigger {
    rate: f64,
    factor: usize,
    /// The capture sample of the template start of a peak at decimated index 0.
    phase: i64,
    spacing: f64,
    window: f64,
    channels: Vec<Channel>,
    ring: Vec<Complex32>,
    n: u64,
    pending: Vec<Pending>,
    pairs: u64,
    rx: BurstReceiver,
    /// From the capture rate to 15.36 Msps, with room for one burst with its margins.
    resampler: Option<SpanResampler>,
    mixed: Vec<Complex32>,
    span: Vec<Complex32>,
    burst: Vec<Complex32>,
}

/// The template of the symbol of root index `r` after the mixer and the decimator of a channel whose search centre is
/// `shift_hz` above the centre of the burst, and the capture sample of the symbol start when the template is at decimated
/// index 0.
fn template(rate: f64, factor: usize, r: usize, shift_hz: f64) -> (Vec<Complex32>, i64) {
    const PAD: usize = 64;
    let symbol = droneid::zc_symbol(r);
    let mut padded = vec![Complex32::new(0.0, 0.0); PAD];
    padded.extend_from_slice(&symbol);
    padded.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), PAD));
    let mut at_rate = Vec::new();
    resample_span(&padded, FS, rate, &mut at_rate);
    if shift_hz != 0.0 {
        Nco::new(rate, -shift_hz).mix_in_place(&mut at_rate);
    }
    let mut dec = decimator(rate, factor);
    let delay = dec.delay().ceil() as usize;
    let lead = 8 * factor;
    let mut input = vec![Complex32::new(0.0, 0.0); lead];
    input.extend_from_slice(&at_rate);
    input.extend(std::iter::repeat_n(
        Complex32::new(0.0, 0.0),
        2 * delay + 4 * factor,
    ));
    let mut out = Vec::new();
    dec.process(&input, &mut out);
    let start = lead as f64 + PAD as f64 * rate / FS;
    let first = (start / factor as f64).floor() as usize;
    let len =
        ((symbol.len() as f64 * rate / FS + 2.0 * delay as f64) / factor as f64).ceil() as usize;
    let end = (first + len).min(out.len());
    let phase = start.round() as i64 - (first * factor) as i64;
    (out[first..end].to_vec(), phase)
}

fn decimator(rate: f64, factor: usize) -> Decimator<Complex32> {
    let out = rate / factor as f64;
    Decimator::new(rate, factor, 0.35 * out, 40.0)
}

/// The offsets from the capture centre `centre_hz` of the DroneID frequencies that a capture at `rate` can search. An
/// unknown centre (0) gives none.
pub fn searched(rate: f64, centre_hz: f64) -> impl Iterator<Item = f64> {
    let reach = EDGE * rate + (0.5 - SEARCH_VISIBLE) * WIDTH_HZ;
    super::DRONEID_FREQS
        .iter()
        .map(move |mhz| mhz * 1e6 - centre_hz)
        .filter(move |o| centre_hz != 0.0 && o.abs() <= reach)
}

impl PairTrigger {
    /// A trigger for a capture at `rate` that searches the frequencies `offsets` (Hz from the capture centre).
    pub fn new(rate: f64, offsets: impl Iterator<Item = f64>) -> PairTrigger {
        let factor = ((rate / SEARCH_RATE_HZ).round() as usize).max(1);
        let low_rate = rate / factor as f64;
        let mut phase = 0;
        let channels = offsets
            .filter_map(|offset_hz| {
                let room = (EDGE * rate - SEARCH_HALF_WIDTH_HZ).max(0.0);
                let search_hz = offset_hz.clamp(-room, room);
                let (t600, at) = template(rate, factor, 0, search_hz - offset_hz);
                let (t147, _) = template(rate, factor, 1, search_hz - offset_hz);
                phase = at;
                let longest = t600.len().max(t147.len());
                let fft = SEARCH_FFT.max((2 * longest).next_power_of_two());
                let sync = SampleSyncSet::new(&[&t600, &t147], fft, &[0], PAIR_THRESHOLD).ok()?;
                Some(Channel {
                    offset_hz,
                    nco: (search_hz != 0.0).then(|| Nco::new(rate, -search_hz)),
                    dec: decimator(rate, factor),
                    sync,
                    waiting: [Vec::with_capacity(WAITING), Vec::with_capacity(WAITING)],
                    on: droneid::visible_carriers(offset_hz, EDGE * rate),
                    low: Vec::with_capacity(1 << 14),
                    peaks: [Vec::with_capacity(64), Vec::with_capacity(64)],
                    fed: 0,
                })
            })
            .collect();
        PairTrigger {
            rate,
            factor,
            phase,
            spacing: PAIR_SPACING * low_rate / FS,
            // A carrier offset moves the peak of root 147 within the symbol: any place of it counts.
            window: 0.5 * NFFT as f64 * low_rate / FS + 2.0,
            channels,
            ring: vec![Complex32::new(0.0, 0.0); (rate * 4e-3) as usize],
            n: 0,
            pending: Vec::with_capacity(PENDING),
            pairs: 0,
            rx: BurstReceiver::default(),
            // Room for one burst with its margins at the capture rate.
            resampler: SpanResampler::new(rate, FS)
                .ok()
                .map(|r| r.with_room((rate * 1.5e-3) as usize + 4096)),
            mixed: Vec::with_capacity((rate * 4e-3) as usize / 8 + 1),
            span: Vec::with_capacity((rate * 1.2e-3) as usize),
            burst: Vec::with_capacity(16_384),
        }
    }

    /// The frequencies searched.
    pub fn channels(&self) -> usize {
        self.channels.len()
    }

    /// The pairs found so far.
    pub fn pairs(&self) -> u64 {
        self.pairs
    }

    /// Feeds samples of the capture. The bursts whose last sample arrived are decoded and appended to `out`.
    pub fn feed(&mut self, x: &[Complex32], out: &mut Vec<Hit>) {
        if self.channels.is_empty() {
            return;
        }
        for block in x.chunks(self.ring.len() / 8) {
            self.search(block);
            self.keep(block);
            self.decode_ready(out);
        }
    }

    fn keep(&mut self, x: &[Complex32]) {
        let len = self.ring.len();
        let at = (self.n % len as u64) as usize;
        let first = x.len().min(len - at);
        self.ring[at..at + first].copy_from_slice(&x[..first]);
        self.ring[..x.len() - first].copy_from_slice(&x[first..]);
        self.n += x.len() as u64;
    }

    fn search(&mut self, x: &[Complex32]) {
        let (factor, phase) = (self.factor as i64, self.phase);
        let (spacing, window) = (self.spacing, self.window);
        for (ci, ch) in self.channels.iter_mut().enumerate() {
            ch.low.clear();
            match ch.nco.as_mut() {
                Some(nco) => {
                    self.mixed.resize(x.len(), Complex32::new(0.0, 0.0));
                    nco.mix(x, &mut self.mixed);
                    ch.dec.process(&self.mixed, &mut ch.low);
                }
                None => ch.dec.process(x, &mut ch.low),
            }
            ch.peaks.iter_mut().for_each(Vec::clear);
            ch.sync.process(&ch.low, &mut ch.peaks);
            for r in 0..2 {
                for i in 0..ch.peaks[r].len() {
                    let p = ch.peaks[r][i];

                    // The other symbol of this peak, if it waits.
                    let other = &ch.waiting[1 - r];
                    let found = other.iter().position(|q| {
                        let (a, b) = if r == 0 { (&p, q) } else { (q, &p) };
                        ((b.index as f64 - a.index as f64) - spacing).abs() <= window
                    });
                    let Some(j) = found else {
                        if ch.waiting[r].len() == WAITING {
                            ch.waiting[r].remove(0);
                        }
                        ch.waiting[r].push(p);
                        continue;
                    };
                    let q = ch.waiting[1 - r].remove(j);
                    let (a, b) = if r == 0 { (p, q) } else { (q, p) };
                    let at = a.index as i64 * factor + phase;
                    self.pairs += 1;
                    if at >= 0 && self.pending.len() < PENDING {
                        self.pending.push(Pending {
                            at: at as u64,
                            channel: ci,
                            pair: [a.metric, b.metric],
                        });
                    }
                }
            }
            // A correlator reports a peak up to one FFT frame and one template after its samples. A peak that waited
            // longer than that and the pair spacing can get no partner.
            ch.fed += ch.low.len() as u64;
            let horizon = (spacing + window) as u64 + 2 * ch.sync.fft_size() as u64;
            let fed = ch.fed;
            for w in &mut ch.waiting {
                w.retain(|p| p.index + horizon >= fed);
            }
        }
    }

    fn decode_ready(&mut self, out: &mut Vec<Hit>) {
        let ratio = self.rate / FS;
        let sym = NFFT as f64 + 72.0;
        // Symbol 2 starts two symbols before the pair, and symbol 9 ends five symbols and one long symbol after its start.
        let before = ((2.0 * sym + MARGIN) * ratio).ceil() as u64;
        let after = ((6.0 * sym + 8.0 + MARGIN) * ratio).ceil() as u64;
        let mut i = 0;
        while i < self.pending.len() {
            let p = self.pending[i];
            if self.n < p.at + after {
                i += 1;
                continue;
            }
            self.pending.remove(i);
            let from = p.at.saturating_sub(before);
            if self.n - from >= self.ring.len() as u64 {
                continue;
            }
            let len = self.ring.len() as u64;
            self.span.clear();
            self.span
                .extend((from..p.at + after).map(|k| self.ring[(k % len) as usize]));
            let ch = &self.channels[p.channel];
            if ch.offset_hz != 0.0 {
                Nco::new(self.rate, -ch.offset_hz).mix_in_place(&mut self.span);
            }
            let Some(resampler) = self.resampler.as_mut() else {
                continue;
            };
            resampler.run(&self.span, &mut self.burst);
            let zc_at = ((p.at - from) as f64 / ratio).round() as usize;
            let decode = self.rx.decode_at(&self.burst, zc_at, ch.on.clone());
            let first = decode.t2.min(self.burst.len());
            let last = (first + (7.0 * sym) as usize + 8).min(self.burst.len());
            let power = self.burst[first..last]
                .iter()
                .map(|v| v.norm_sqr() as f64)
                .sum::<f64>()
                / (last - first).max(1) as f64;
            out.push(Hit {
                start_s: (from as f64 + decode.t2 as f64 * ratio) / self.rate,
                offset_hz: ch.offset_hz,
                pair: p.pair,
                visible: ch.on.len() as f64 / droneid::NC as f64,
                level_dbfs: power_to_db(power),
                decode,
            });
        }
    }
}
