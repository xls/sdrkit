//! One Horus channel: the shared channelizer to about 10 ksps, the tone estimator on an averaged spectrum, the 4FSK
//! demodulator with its four readings of the bits and the frame search, and the RTTY demodulators at 50, 100 and 300 baud with
//! their character framers. Decisions are taken on tone energies of the IQ samples.

use super::layer2::{Layer2, V1_PAYLOAD, V2_PAYLOAD};
use super::packet::{parse_sentence, Binary};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::charframe::CharFramer;
use sdrkit_algo_modem::fsk::FskLlr;
use sdrkit_algo_modem::tonebank::{RetunableCorrelator, ToneBank};
use sdrkit_core::Complex32;
use std::collections::VecDeque;

/// The symbol rate of Horus Binary, Hz.
pub const BAUD: f64 = 100.0;
/// The RTTY rates, Hz.
pub const RTTY_BAUDS: [f64; 3] = [50.0, 100.0, 300.0];
/// The preamble byte and the unique word that start a frame.
const SYNC_WORD: u32 = 0x1B2424;
/// Differing bits that a sync still takes.
const SYNC_ERRORS: u32 = 3;
/// Candidates kept for each reading.
const CANDIDATES: usize = 64;
/// Bits kept for each reading: two frames of symbols.
const BITS_KEPT: usize = 2400;
/// The bits of the preamble byte and the unique word.
const SYNC_WORD_BITS: usize = 24;
/// Symbols of tone energies kept for a second reading of a frame: a v2 frame with its sync word and room for its timing.
const SYMBOLS_KEPT: usize = 300;
/// How far the clock of a frame is searched from the running clock, ppm.
const CLOCK_SEARCH_PPM: f64 = 1000.0;

/// The output rate of a channel for the input rate `rate`: the first of 10000, 9600, 12000, 8000 and 16000 Hz that divides
/// the rate, so 100 baud has a whole number of samples a symbol; else the rate over the nearest whole factor to 10 kHz.
pub fn channel_rate(rate: f64) -> f64 {
    for c in [10_000.0, 9_600.0, 12_000.0, 8_000.0, 16_000.0] {
        let d = rate / c;
        if rate >= c && (d - d.round()).abs() < 1e-9 {
            return c;
        }
    }
    rate / (rate / 10_000.0).round().max(1.0)
}

/// What a channel decoded.
#[derive(Clone, Debug, PartialEq)]
pub enum Decoded {
    /// A binary packet whose CRC is good: its payload bytes and the bits that Golay corrected.
    Binary { bytes: Vec<u8>, fixed: usize },
    /// An RTTY sentence whose checksum is good, from the framer of `bits` data bits at `baud`.
    Rtty {
        sentence: String,
        baud: f64,
        bits: u8,
    },
}

/// A packet of a channel, with its measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Signal time of the end of the packet in the channel, s.
    pub time_s: f64,
    /// The packet.
    pub decoded: Decoded,
    /// The SNR in 3 kHz, dB.
    pub snr_db: f64,
    /// The middle of the lowest and the highest tone, Hz from the channel centre.
    pub centre_hz: f64,
    /// The tone spacing, Hz.
    pub spacing_hz: f64,
}

/// The tone estimator: an averaged power spectrum, and the peaks that make 4FSK and RTTY tones.
#[derive(Clone, Debug)]
pub struct ToneEstimator {
    rate: f64,
    n: usize,
    hop: usize,
    fft: Fft,
    win: Vec<f32>,
    ring: Vec<Complex32>,
    pos: usize,
    filled: usize,
    since_hop: usize,
    since_eval: usize,
    work: Vec<Complex32>,
    avg: Vec<f32>,
    spectra: usize,
    weight: f32,
    /// The four 4FSK tones, Hz, lowest first.
    pub fsk: Option<[f64; 4]>,
    /// The two RTTY tones, Hz, lowest first.
    pub rtty: Option<[f64; 2]>,
}

impl ToneEstimator {
    /// An estimator at the channel rate `rate`: bins of about 10 Hz, half-length hops, about 3 s of memory.
    pub fn new(rate: f64) -> ToneEstimator {
        let n = ((rate / 10.0).ceil() as usize).next_power_of_two();
        let hop = n / 2;
        ToneEstimator {
            rate,
            n,
            hop,
            fft: Fft::forward(n),
            win: Window::Hann.coefficients(n),
            ring: vec![Complex32::default(); n],
            pos: 0,
            filled: 0,
            since_hop: 0,
            since_eval: 0,
            work: vec![Complex32::default(); n],
            avg: vec![0.0; n],
            spectra: 0,
            weight: (hop as f64 / rate / 3.0).min(1.0) as f32,
            fsk: None,
            rtty: None,
        }
    }

    /// The spectrum is usable: 4 spectra are in.
    pub fn usable(&self) -> bool {
        self.spectra >= 4
    }

    /// Takes samples. Returns true when the 4FSK or the RTTY tones changed.
    pub fn push(&mut self, x: &[Complex32]) -> bool {
        let mut changed = false;
        for &s in x {
            self.ring[self.pos] = s;
            self.pos = (self.pos + 1) % self.n;
            self.filled = (self.filled + 1).min(self.n);
            self.since_hop += 1;
            self.since_eval += 1;
            if self.since_hop >= self.hop && self.filled == self.n {
                self.since_hop = 0;
                self.spectrum();
            }
            if self.since_eval as f64 >= 0.5 * self.rate {
                self.since_eval = 0;
                if self.usable() {
                    changed |= self.estimate();
                }
            }
        }
        changed
    }

    fn spectrum(&mut self) {
        for i in 0..self.n {
            self.work[i] = self.ring[(self.pos + i) % self.n] * self.win[i];
        }
        self.fft.process(&mut self.work);
        self.spectra += 1;
        let a = if self.spectra <= 4 {
            1.0 / self.spectra as f32
        } else {
            self.weight
        };
        for (v, w) in self.avg.iter_mut().zip(&self.work) {
            *v += a * (w.norm_sqr() - *v);
        }
    }

    /// The SNR in 3 kHz of a signal whose tones lie from `lo_hz` to `hi_hz`, from the averaged spectrum: the power above the
    /// floor (the median of the spectrum) from 150 Hz below the lowest tone to 150 Hz above the highest, over the floor of one
    /// sample. `None` before the spectrum is usable.
    pub fn snr_3k_db(&self, lo_hz: f64, hi_hz: f64) -> Option<f64> {
        if !self.usable() {
            return None;
        }
        let mut sorted = self.avg.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let floor = f64::from(sorted[self.n / 2]).max(1e-30);
        let bin = self.bin_hz();
        let n = self.n as i64;
        let (k0, k1) = (
            ((lo_hz - 150.0) / bin).floor() as i64,
            ((hi_hz + 150.0) / bin).ceil() as i64,
        );
        let excess: f64 = (k0..=k1)
            .map(|k| f64::from(self.avg[k.rem_euclid(n) as usize]) - floor)
            .sum();
        let ratio = excess / (self.n as f64 * floor) * self.rate / 3000.0;
        Some(10.0 * ratio.max(1e-6).log10())
    }

    /// The bin width, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.n as f64
    }

    /// The averaged spectrum from -0.42 to +0.42 of the rate in about `points` points, each the strongest bin of its group,
    /// in dB over the median of the spectrum: the frequency of the first point, the step, and the points. `None` before the
    /// spectrum is usable.
    pub fn spectrum_db(&self, points: usize) -> Option<(f64, f64, Vec<f32>)> {
        if !self.usable() || points == 0 {
            return None;
        }
        let bin = self.bin_hz();
        let half = (0.42 * self.rate / bin).floor() as i64;
        let n = self.n as i64;
        let mut sorted = self.avg.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let floor = sorted[self.n / 2].max(1e-30);
        let group = ((2 * half + 1) as usize).div_ceil(points).max(1) as i64;
        let mut out = Vec::with_capacity(points);
        let mut k = -half;
        while k <= half {
            let top = (k..(k + group).min(half + 1))
                .map(|j| self.avg[j.rem_euclid(n) as usize])
                .fold(0f32, f32::max);
            out.push(10.0 * (top.max(1e-30) / floor).log10());
            k += group;
        }
        Some((-half as f64 * bin, group as f64 * bin, out))
    }

    /// The averaged power at the frequencies `-0.4 x rate` to `+0.4 x rate`, lowest first, with the frequency of the first bin.
    fn ordered(&self) -> (f64, Vec<f32>) {
        let bin = self.bin_hz();
        let half = (0.4 * self.rate / bin).floor() as i64;
        let n = self.n as i64;
        let v = (-half..=half)
            .map(|k| self.avg[k.rem_euclid(n) as usize])
            .collect();
        (-half as f64 * bin, v)
    }

    /// The floor (the median of the averaged spectrum in the range) and the peaks: (Hz, power), strongest first.
    pub fn peaks(&self) -> (f32, Vec<(f64, f32)>) {
        let bin = self.bin_hz();
        let (f0, raw) = self.ordered();
        let h = (45.0 / bin).round() as usize;
        let m = raw.len();
        let smooth: Vec<f32> = (0..m)
            .map(|i| {
                let (lo, hi) = (i.saturating_sub(h), (i + h + 1).min(m));
                raw[lo..hi].iter().sum::<f32>() / (hi - lo) as f32
            })
            .collect();
        let mut sorted = smooth.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let floor = sorted[m / 2].max(1e-30);
        let mut cands: Vec<(usize, f32)> = (1..m - 1)
            .filter(|&i| {
                smooth[i] > 1.5 * floor && smooth[i] >= smooth[i - 1] && smooth[i] > smooth[i + 1]
            })
            .map(|i| (i, smooth[i]))
            .collect();
        cands.sort_by(|a, b| b.1.total_cmp(&a.1));
        let apart = (80.0 / bin).round().max(2.0) as usize;
        let mut picked: Vec<(usize, f32)> = Vec::new();
        for c in cands {
            if picked.iter().all(|p| p.0.abs_diff(c.0) >= apart) {
                picked.push(c);
            }
            if picked.len() == 12 {
                break;
            }
        }
        let peaks = picked
            .iter()
            .map(|&(i, p)| {
                let (a, b, c) = (smooth[i - 1], smooth[i], smooth[i + 1]);
                let den = a - 2.0 * b + c;
                let off = if den < 0.0 {
                    (0.5 * (a - c) / den).clamp(-0.5, 0.5)
                } else {
                    0.0
                };
                (f0 + (i as f64 + f64::from(off)) * bin, p)
            })
            .collect();
        (floor, peaks)
    }

    /// Looks for the 4FSK and RTTY tones. Returns true when they changed.
    fn estimate(&mut self) -> bool {
        let (_, peaks) = self.peaks();
        let mut changed = false;
        let top: Vec<(f64, f32)> = peaks.iter().take(8).copied().collect();
        let mut best: Option<([f64; 4], f32)> = None;
        let k = top.len();
        for a in 0..k {
            for b in a + 1..k {
                for c in b + 1..k {
                    for d in c + 1..k {
                        let mut f = [top[a].0, top[b].0, top[c].0, top[d].0];
                        f.sort_by(|x, y| x.total_cmp(y));
                        let g = [f[1] - f[0], f[2] - f[1], f[3] - f[2]];
                        let mean = (g[0] + g[1] + g[2]) / 3.0;
                        if mean > 0.0 && g.iter().all(|x| (x - mean).abs() <= 0.25 * mean) {
                            let p = top[a].1 + top[b].1 + top[c].1 + top[d].1;
                            if best.is_none_or(|(_, q)| p > q) {
                                best = Some((f, p));
                            }
                        }
                    }
                }
            }
        }
        if let Some((p, _)) = best {
            // The four tones are equally spaced: the least-squares fit of a centre and a spacing to the four peaks.
            let c = p.iter().sum::<f64>() / 4.0;
            let s = (-1.5 * p[0] - 0.5 * p[1] + 0.5 * p[2] + 1.5 * p[3]) / 5.0;
            let f = [c - 1.5 * s, c - 0.5 * s, c + 0.5 * s, c + 1.5 * s];
            let lim = 0.25 * self.bin_hz() + 1.5;
            if self
                .fsk
                .is_none_or(|o| o.iter().zip(&f).any(|(a, b)| (a - b).abs() > lim))
            {
                self.fsk = Some(f);
                changed = true;
            }
        }
        'pairs: for i in 0..peaks.len() {
            for j in i + 1..peaks.len() {
                let (a, b) = (peaks[i].0, peaks[j].0);
                if (a - b).abs() < 1300.0 {
                    let t = [a.min(b), a.max(b)];
                    if self
                        .rtty
                        .is_none_or(|o| (o[0] - t[0]).abs() > 3.0 || (o[1] - t[1]).abs() > 3.0)
                    {
                        self.rtty = Some(t);
                        changed = true;
                    }
                    break 'pairs;
                }
            }
        }
        changed
    }
}

/// One reading of the 4FSK symbols: mirrored or not, the two bits of a symbol in order or swapped.
#[derive(Clone, Debug, Default)]
struct Reading {
    mirrored: bool,
    swapped: bool,
    bits: Vec<u8>,
    /// The reliability of each bit (its |LLR|).
    rel: Vec<f32>,
    /// The bit index of `bits[0]` since the start.
    base: u64,
    reg: u32,
    cands: Vec<u64>,
}

/// The 4FSK demodulator of a channel and its frame search.
pub struct FourFsk {
    rate: f64,
    demod: Option<ToneBank>,
    tones: [f64; 4],
    llr: FskLlr,
    energies: Vec<f32>,
    labels: Vec<u16>,
    frame_energies: Vec<f32>,
    frame_labels: Vec<u16>,
    llrs: [f32; 2],
    amp2: f32,
    noise: f32,
    readings: [Reading; 4],
    v1: Layer2,
    v2: Layer2,
    /// Unique words found.
    pub syncs: u64,
    /// Frames that failed their CRC.
    pub failed: u64,
}

impl FourFsk {
    /// A demodulator at the channel rate `rate`; it waits for tones.
    pub fn new(rate: f64) -> FourFsk {
        let readings =
            [(false, false), (false, true), (true, false), (true, true)].map(|(m, s)| Reading {
                mirrored: m,
                swapped: s,
                ..Reading::default()
            });
        FourFsk {
            rate,
            demod: None,
            tones: [0.0; 4],
            llr: FskLlr::natural(4).expect("four tones"),
            energies: Vec::new(),
            labels: Vec::new(),
            frame_energies: Vec::new(),
            frame_labels: Vec::new(),
            llrs: [0.0; 2],
            amp2: 0.0,
            noise: 0.0,
            readings,
            v1: Layer2::new(V1_PAYLOAD).expect("the v1 size"),
            v2: Layer2::new(V2_PAYLOAD).expect("the v2 size"),
            syncs: 0,
            failed: 0,
        }
    }

    /// The tones, Hz, lowest first.
    pub fn tones(&self) -> Option<[f64; 4]> {
        self.demod.as_ref().map(|_| self.tones)
    }

    /// A frame is in progress: a reading has a sync whose frame is not yet complete.
    pub fn busy(&self) -> bool {
        let full = 8 * self.v2.body_len() as u64;
        self.demod.is_some()
            && self.readings.iter().any(|r| {
                r.cands
                    .iter()
                    .any(|&c| r.base + r.bits.len() as u64 - c < full)
            })
    }

    /// Follows new tones: the window of the correlators fills again, the symbol clock stays.
    pub fn retune(&mut self, tones: [f64; 4]) {
        let f = tones.map(|t| t / self.rate);
        match self.demod.as_mut() {
            Some(b) => {
                if b.retune(&f).is_err() {
                    self.demod = None;
                }
            }
            None => {
                self.demod = ToneBank::new(&f, (self.rate / BAUD).round().max(4.0) as usize, 24.0)
                    .ok()
                    .map(|b| b.with_history(SYMBOLS_KEPT));
                for rd in &mut self.readings {
                    *rd = Reading {
                        mirrored: rd.mirrored,
                        swapped: rd.swapped,
                        ..Reading::default()
                    };
                }
            }
        }
        self.tones = tones;
    }

    /// Demodulates `x`; packets go to `out` with the signal time `time_s` of the end of `x`.
    pub fn process(&mut self, x: &[Complex32], time_s: f64, out: &mut Vec<Found>) {
        let Some(d) = self.demod.as_mut() else {
            return;
        };
        self.energies.clear();
        d.process(x, &mut self.energies);
        let n = self.energies.len() / 4;
        self.labels.resize(n, 0);
        if self.llr.decide(&self.energies, &mut self.labels).is_err() {
            return;
        }
        for k in 0..n {
            let e = [
                self.energies[4 * k],
                self.energies[4 * k + 1],
                self.energies[4 * k + 2],
                self.energies[4 * k + 3],
            ];
            // The tone amplitude and the noise for the LLRs, followed over about 64 symbols.
            if let Some((a2, n)) = FskLlr::estimate(&e, 4) {
                let w = if self.noise == 0.0 { 1.0 } else { 1.0 / 64.0 };
                self.amp2 += w * (a2 - self.amp2);
                self.noise += w * (n - self.noise);
            }
            let _ = self
                .llr
                .llrs(&e, self.amp2.sqrt(), self.noise.max(1e-30), &mut self.llrs);
            let rel = [self.llrs[0].abs(), self.llrs[1].abs()];
            let s = self.labels[k] as u8;
            for r in 0..4 {
                let rd = &self.readings[r];
                let sym = if rd.mirrored { 3 - s } else { s };
                let (a, b) = (sym >> 1, sym & 1);
                let ((a, ra), (b, rb)) = if rd.swapped {
                    ((b, rel[1]), (a, rel[0]))
                } else {
                    ((a, rel[0]), (b, rel[1]))
                };
                self.push_bit(r, a, ra, time_s, out);
                self.push_bit(r, b, rb, time_s, out);
            }
        }
    }

    fn push_bit(&mut self, r: usize, bit: u8, rel: f32, time_s: f64, out: &mut Vec<Found>) {
        let rd = &mut self.readings[r];
        rd.bits.push(bit);
        rd.rel.push(rel);
        rd.reg = ((rd.reg << 1) | u32::from(bit)) & 0xFF_FFFF;
        let end = rd.base + rd.bits.len() as u64;
        if (rd.reg ^ SYNC_WORD).count_ones() <= SYNC_ERRORS && !rd.cands.contains(&end) {
            if rd.cands.len() >= CANDIDATES {
                rd.cands.drain(..CANDIDATES / 2);
            }
            rd.cands.push(end);
            self.syncs += 1;
        }
        if rd.bits.len() > 2 * BITS_KEPT {
            let cut = rd.bits.len() - BITS_KEPT;
            rd.bits.drain(..cut);
            rd.rel.drain(..cut);
            rd.base += cut as u64;
        }
        let mut i = 0;
        while i < self.readings[r].cands.len() {
            let c = self.readings[r].cands[i];
            let have = end - c;
            let try_v1 = have == 8 * (self.v1.body_len() as u64);
            let try_v2 = have == 8 * (self.v2.body_len() as u64);
            if !(try_v1 || try_v2) {
                i += 1;
                continue;
            }
            let nb = if try_v1 {
                self.v1.body_len()
            } else {
                self.v2.body_len()
            };
            let Some((bits, rels, snr_db)) = self.frame(r, c, nb) else {
                self.readings[r].cands.remove(i);
                continue;
            };
            let l2 = if try_v1 { &self.v1 } else { &self.v2 };
            let found = [true, false].into_iter().find_map(|msb| {
                let place = |j: usize| if msb { (j / 8) * 8 + 7 - j % 8 } else { j };
                let mut body = vec![0u8; nb];
                let mut rel = vec![0f32; 8 * nb];
                for (j, (&x, &w)) in bits.iter().zip(&rels).enumerate() {
                    let i = place(j);
                    body[i / 8] |= x << (i % 8);
                    rel[i] = w;
                }
                let (payload, fixed) = l2.decode_soft(&body, &rel)?;
                Binary::parse(&payload).map(|_| (payload, fixed))
            });
            match found {
                Some((bytes, fixed)) => {
                    let spacing = (self.tones[3] - self.tones[0]) / 3.0;
                    out.push(Found {
                        time_s,
                        decoded: Decoded::Binary { bytes, fixed },
                        snr_db,
                        centre_hz: (self.tones[0] + self.tones[3]) / 2.0,
                        spacing_hz: spacing,
                    });
                    let lo = c.saturating_sub(24);
                    let hi = c + 504;
                    self.readings[r].cands.retain(|&s| s < lo || s > hi);
                    i = 0;
                }
                None if try_v2 => {
                    self.failed += 1;
                    self.readings[r].cands.remove(i);
                }
                None => i += 1,
            }
        }
    }

    /// The `8 nb` bits of reading `r` from bit `c` on, their reliabilities and the SNR in 3 kHz of the frame. The symbols are
    /// taken again at the timing that fits the whole frame best, with the unique word and the preamble byte before it, and
    /// their soft values use one amplitude and noise for the frame. A frame that the history no longer holds, or that starts
    /// between two symbols, keeps the symbols of the running clock. `None` when the reading no longer holds the bits.
    fn frame(&mut self, r: usize, c: u64, nb: usize) -> Option<(Vec<u8>, Vec<f32>, f64)> {
        let symbols = 4 * nb;
        let lead = (SYNC_WORD_BITS / 2) as u64;
        let bank = self.demod.as_ref()?;
        self.frame_energies.clear();
        let refit = c
            .is_multiple_of(2)
            .then_some(c / 2)
            .and_then(|first| first.checked_sub(lead))
            .and_then(|first| bank.refit(first, symbols + lead as usize, CLOCK_SEARCH_PPM))
            .is_some_and(|t| {
                bank.energies_at(&t, symbols + lead as usize, &mut self.frame_energies)
            });
        let rd = &self.readings[r];
        let at = usize::try_from(c.checked_sub(rd.base)?).ok()?;
        if !refit || rd.bits.len() < at + 8 * nb {
            let bits = rd.bits.get(at..at + 8 * nb)?.to_vec();
            let rels = rd.rel[at..at + 8 * nb].to_vec();
            return Some((bits, rels, f64::NAN));
        }
        let e = &self.frame_energies[4 * lead as usize..];
        let (a, n) = FskLlr::estimate(e, 4)?;
        self.frame_labels.resize(symbols, 0);
        self.llr.decide(e, &mut self.frame_labels).ok()?;
        let mut llrs = vec![0f32; 2 * symbols];
        self.llr.llrs(e, a, n.max(1e-30), &mut llrs).ok()?;
        let (mut bits, mut rels) = (Vec::with_capacity(8 * nb), Vec::with_capacity(8 * nb));
        for (k, &label) in self.frame_labels.iter().enumerate() {
            let s = if rd.mirrored {
                3 - label as u8
            } else {
                label as u8
            };
            let pair = [(s >> 1, llrs[2 * k].abs()), (s & 1, llrs[2 * k + 1].abs())];
            let [first, second] = if rd.swapped { [pair[1], pair[0]] } else { pair };
            for (b, w) in [first, second] {
                bits.push(b);
                rels.push(w);
            }
        }
        Some((bits, rels, symbol_snr(e)))
    }
}

/// The SNR in 3 kHz of the symbols of `energies` (4 a symbol): the strongest energies over the means of the other three.
fn symbol_snr(energies: &[f32]) -> f64 {
    let (mut s, mut n) = (0f64, 0f64);
    for e in energies.as_chunks::<4>().0 {
        let (k, &m) = e
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap_or((0, &0.0));
        s += f64::from(m);
        n += e
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != k)
            .map(|(_, &v)| f64::from(v))
            .sum::<f64>()
            / 3.0;
    }
    10.0 * ((s - n) / n.max(1e-30)).max(1e-6).log10() + 10.0 * (BAUD / 3000.0).log10()
}

/// The text of one framer, until a line ends with a checksum, and the turns of the two tones over the bits it read.
#[derive(Clone, Debug, Default)]
struct Line {
    text: String,
    turns: [Complex32; 2],
}

impl Line {
    fn clear(&mut self) {
        self.text.clear();
        self.turns = [Complex32::default(); 2];
    }
}

/// The RTTY demodulator at one rate: the energies at the two tones and the framers of 7 and 8 bits in both senses. The tones
/// of the lane are the estimated ones moved by what the bits of its good sentences measured.
struct RttyLane {
    baud: f64,
    window: usize,
    corr: RetunableCorrelator,
    framers: Vec<(CharFramer, Line)>,
    s: f64,
    n: f64,
    alpha: f64,
    e: [f32; 2],
    /// Hz added to the estimated tones.
    fix: [f64; 2],
}

/// A tone that the bits of a good sentence put this far from the estimate, or farther, is not followed, in bits a second.
const RTTY_FIX_LIMIT: f64 = 0.25;
/// Estimated tones that move this far are of another signal: the lanes forget what they measured, Hz.
const RTTY_NEW_SIGNAL_HZ: f64 = 50.0;

/// The RTTY demodulators of a channel.
pub struct Rtty {
    rate: f64,
    tones: Option<[f64; 2]>,
    lanes: Vec<RttyLane>,
}

impl Rtty {
    /// RTTY at the channel rate `rate`; it waits for tones.
    pub fn new(rate: f64) -> Rtty {
        Rtty {
            rate,
            tones: None,
            lanes: Vec::new(),
        }
    }

    /// The estimated tones, Hz, lowest first.
    pub fn tones(&self) -> Option<[f64; 2]> {
        self.tones
    }

    /// Follows new estimated tones: the correlators mix their window again; the framers and their lines go on. Returns true
    /// when the tones are the first ones or moved so far that they are of another signal.
    pub fn retune(&mut self, tones: [f64; 2]) -> bool {
        let new_signal = self.tones.is_none_or(|o| {
            (o[0] - tones[0]).abs() > RTTY_NEW_SIGNAL_HZ
                || (o[1] - tones[1]).abs() > RTTY_NEW_SIGNAL_HZ
        });
        self.tones = Some(tones);
        if self.lanes.is_empty() {
            let f = [tones[0] / self.rate, tones[1] / self.rate];
            self.lanes = RTTY_BAUDS
                .iter()
                .filter_map(|&baud| {
                    let spb = self.rate / baud;
                    let window = spb.round().max(2.0) as usize;
                    let corr = RetunableCorrelator::new(&f, window).ok()?;
                    let framers = [(7u8, false), (7, true), (8, false), (8, true)]
                        .iter()
                        .filter_map(|&(bits, inv)| {
                            CharFramer::new(spb, bits, inv)
                                .ok()
                                .map(|fr| (fr, Line::default()))
                        })
                        .collect();
                    Some(RttyLane {
                        baud,
                        window,
                        corr,
                        framers,
                        s: 0.0,
                        n: 0.0,
                        alpha: 1.0 / (2.0 * self.rate),
                        e: [0.0; 2],
                        fix: [0.0; 2],
                    })
                })
                .collect();
            return true;
        }
        for lane in &mut self.lanes {
            if new_signal {
                lane.fix = [0.0; 2];
            }
            let f = [
                (tones[0] + lane.fix[0]) / self.rate,
                (tones[1] + lane.fix[1]) / self.rate,
            ];
            let _ = lane.corr.retune(&f);
        }
        new_signal
    }

    /// A sentence is in progress: a line holds a `$$`.
    pub fn busy(&self) -> bool {
        self.lanes
            .iter()
            .any(|l| l.framers.iter().any(|(_, line)| line.text.contains("$$")))
    }

    /// Demodulates `x`; sentences go to `out` with the signal time `time_s` of the end of `x`.
    pub fn process(&mut self, x: &[Complex32], time_s: f64, out: &mut Vec<Found>) {
        let Some(tones) = self.tones else {
            return;
        };
        let rate = self.rate;
        for lane in &mut self.lanes {
            for &v in x {
                lane.corr.push(v);
                if !lane.corr.ready() {
                    continue;
                }
                let _ = lane.corr.energies(&mut lane.e);
                let (lo, hi) = (f64::from(lane.e[0]), f64::from(lane.e[1]));
                lane.s += lane.alpha * (lo.max(hi) - lane.s);
                lane.n += lane.alpha * (lo.min(hi) - lane.n);
                let d = ((hi - lo) / (hi + lo).max(1e-30)) as f32;
                let mut good: Option<[Complex32; 2]> = None;
                for (f, line) in &mut lane.framers {
                    let c = f.push(d);
                    // The turn of the tone of each bit read, over the bit.
                    if let Some(high) = f.decided() {
                        let k = usize::from(high);
                        if let Some(z) = lane.corr.half_turn(k) {
                            line.turns[k] += z;
                        }
                    }
                    let Some(c) = c else {
                        continue;
                    };
                    if !(32..=126).contains(&c) || line.text.len() >= 400 {
                        line.clear();
                        continue;
                    }
                    line.text.push(char::from(c));
                    if let Some(s) = take_sentence(&line.text) {
                        if parse_sentence(&s, &[]).is_some() {
                            let snr = 10.0
                                * ((lane.s - lane.n) / lane.n.max(1e-30)).max(1e-6).log10()
                                + 10.0 * (lane.baud / 3000.0).log10();
                            let t = [tones[0] + lane.fix[0], tones[1] + lane.fix[1]];
                            out.push(Found {
                                time_s,
                                decoded: Decoded::Rtty {
                                    sentence: s,
                                    baud: lane.baud,
                                    bits: f.data_bits(),
                                },
                                snr_db: snr,
                                centre_hz: (t[0] + t[1]) / 2.0,
                                spacing_hz: t[1] - t[0],
                            });
                            good = Some(line.turns);
                        }
                        line.clear();
                    }
                }
                // The bits of a good sentence say where its tones lie.
                if let Some(turns) = good {
                    let h = (lane.window / 2) as f64;
                    let limit = RTTY_FIX_LIMIT * lane.baud;
                    for (fix, z) in lane.fix.iter_mut().zip(turns) {
                        if z.norm_sqr() > 0.0 {
                            let off = f64::from(z.arg()) / (std::f64::consts::TAU * h) * rate;
                            *fix = (*fix + off).clamp(-limit, limit);
                        }
                    }
                    let f = [
                        (tones[0] + lane.fix[0]) / rate,
                        (tones[1] + lane.fix[1]) / rate,
                    ];
                    let _ = lane.corr.retune(&f);
                }
            }
        }
    }
}

/// The sentence of a line that ends with `*` and 4 hex digits, holds at least 10 characters and a `$$`: from the last `$$`.
fn take_sentence(text: &str) -> Option<String> {
    let b = text.as_bytes();
    if b.len() < 10 || b[b.len() - 5] != b'*' || !b[b.len() - 4..].iter().all(u8::is_ascii_hexdigit)
    {
        return None;
    }
    let at = text.rfind("$$")?;
    Some(text[at..].to_string())
}

/// One channel: the channelizer, the tone estimator, the 4FSK and the RTTY demodulators.
pub struct Channel {
    /// The place of the channel, Hz from the centre of the input.
    pub offset_hz: f64,
    chan: Channelizer,
    rate: f64,
    /// The tone estimator.
    pub est: ToneEstimator,
    /// The 4FSK demodulator.
    pub fsk: FourFsk,
    /// The RTTY demodulators.
    pub rtty: Rtty,
    buf: Vec<Complex32>,
    /// The samples of the last second, so the demodulators read a packet that began before the first tones were found.
    recent: VecDeque<Complex32>,
    samples: u64,
}

/// Seconds of samples that a demodulator reads again when it starts.
const REPLAY_S: f64 = 1.0;

impl Channel {
    /// A channel `offset_hz` from the centre of an input at `input_rate`. `None` when the channelizer cannot be made.
    pub fn new(input_rate: f64, offset_hz: f64) -> Option<Channel> {
        let rate = channel_rate(input_rate);
        let mut cfg = ChannelConfig::new(input_rate, offset_hz, 0.8 * rate, rate);
        cfg.exact_rate = true;
        cfg.atten_db = 70.0;
        let chan = Channelizer::new(cfg).ok()?;
        let rate = chan.output_rate();
        Some(Channel {
            offset_hz,
            chan,
            rate,
            est: ToneEstimator::new(rate),
            fsk: FourFsk::new(rate),
            rtty: Rtty::new(rate),
            buf: Vec::new(),
            recent: VecDeque::with_capacity((REPLAY_S * rate) as usize),
            samples: 0,
        })
    }

    /// The output rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Signal time of the channel, s.
    pub fn time_s(&self) -> f64 {
        self.samples as f64 / self.rate
    }

    /// Takes samples of the input; packets go to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Found>) {
        self.buf.clear();
        self.chan.process(x, &mut self.buf);
        let buf = std::mem::take(&mut self.buf);
        for piece in buf.chunks(256) {
            self.est.push(piece);
            let fsk_new = self.fsk.tones().is_none();
            let mut rtty_new = false;
            // A retune mixes the window of the correlators again and loses no symbol. Inside a frame the 4FSK demodulator
            // keeps its tones unless one moved by a tenth of the correlator width (10 Hz) or more.
            if let Some(t) = self.est.fsk {
                let far = self
                    .fsk
                    .tones()
                    .is_none_or(|o| o.iter().zip(&t).any(|(a, b)| (a - b).abs() >= 0.1 * BAUD));
                if self.fsk.tones() != Some(t) && (far || !self.fsk.busy()) {
                    self.fsk.retune(t);
                }
            }
            if let Some(t) = self.est.rtty {
                if self.rtty.tones() != Some(t) {
                    rtty_new = self.rtty.retune(t);
                }
            }
            self.samples += piece.len() as u64;
            let t = self.time_s();
            let first = out.len();
            // A demodulator that starts, or that follows another signal, reads the last second first.
            if fsk_new && self.fsk.tones().is_some() {
                let (a, b) = self.recent.as_slices();
                self.fsk.process(a, t, out);
                self.fsk.process(b, t, out);
            }
            if rtty_new {
                let (a, b) = self.recent.as_slices();
                self.rtty.process(a, t, out);
                self.rtty.process(b, t, out);
            }
            self.fsk.process(piece, t, out);
            self.rtty.process(piece, t, out);
            let cap = (REPLAY_S * self.rate) as usize;
            self.recent.extend(piece);
            if self.recent.len() > cap {
                let cut = self.recent.len() - cap;
                self.recent.drain(..cut);
            }
            // The spectrum gives the SNR when it is usable: the energies of the symbols hold the leakage between tones.
            for f in &mut out[first..] {
                let half = match f.decoded {
                    Decoded::Binary { .. } => 1.5 * f.spacing_hz,
                    Decoded::Rtty { .. } => 0.5 * f.spacing_hz,
                };
                if let Some(s) = self.est.snr_3k_db(f.centre_hz - half, f.centre_hz + half) {
                    f.snr_db = s;
                }
            }
        }
        self.buf = buf;
    }
}
