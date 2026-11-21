//! A chip receiver for 2-level FSK bursts with a sync word: mix, low-pass and resample to a whole number of samples per chip,
//! a soft chip value at every sample (FM discriminator or tone energy), the sync search at every sample phase and both
//! polarities, the choice among the phases that report one sync, and the noise floor of the channel. A [`Reader`] reads the
//! frame after the sync from the soft chips. FLARM and the ADS-L channels are configurations of it.

use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_core::Complex32;
use std::f64::consts::PI;

/// 20 ms blocks kept for the noise floor: 5 s.
const BLOCKS: usize = 250;
/// Frames that can wait for their chips at one time.
const PENDING: usize = 16;
/// The chips of the running mean of plain chips.
const MEAN_CHIPS: u64 = 32;

/// How a chip value is made from the samples of one chip time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detector {
    /// The phase step between samples, scaled so that the nominal deviation gives ±1, summed over the chip time.
    Discriminator,
    /// The energy at the upper tone minus the energy at the lower tone over the chip time: the non-coherent detector for a
    /// modulation index of 1.
    ToneEnergy,
}

/// The signal of one channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChipConfig {
    /// The chip rate, chips per second.
    pub chip_rate: f64,
    /// The samples per chip at the work rate.
    pub samples_per_chip: usize,
    /// The peak deviation, Hz. A chip 1 is the higher frequency.
    pub deviation_hz: f64,
    /// The pass edge of the channel filter, Hz.
    pub pass_hz: f64,
    /// The stop edge of the channel filter, Hz.
    pub stop_hz: f64,
    /// The chip detector.
    pub detector: Detector,
    /// The bits are Manchester coded: bit 1 is the chips 0 then 1.
    pub manchester: bool,
    /// The sync pattern, its last bit at bit 0.
    pub sync: u32,
    /// The bits of the sync pattern, at most 32.
    pub sync_bits: u32,
    /// The samples after a start that are not searched.
    pub skip_samples: u64,
    /// The chips kept for each sample phase. A power of two, and more than the longest frame with its sync.
    pub ring_chips: usize,
}

impl ChipConfig {
    /// The rate that the chips are read at, Hz.
    pub fn work_rate(&self) -> f64 {
        self.chip_rate * self.samples_per_chip as f64
    }
    /// The chips of one bit.
    pub fn chips_per_bit(&self) -> u64 {
        if self.manchester {
            2
        } else {
            1
        }
    }
}

/// The counters of a channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChipStats {
    /// Syncs taken.
    pub syncs: u64,
    /// Good frames.
    pub good: u64,
    /// Syncs with at most one wrong bit that gave no frame.
    pub bad: u64,
}

/// What a [`Reader`] says about a sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Call again when this many chips after the sync are there (more than at this call).
    Need(u64),
    /// A good frame.
    Good,
    /// No frame.
    Bad,
}

/// Reads the frame after a sync.
pub trait Reader {
    /// The chips after the sync that the first call of [`Reader::read`] needs.
    fn first_need(&self) -> u64;
    /// Reads the frame from `view`. It keeps what it reads.
    fn read(&mut self, view: &ChipView<'_>) -> Outcome;
}

/// The soft chips around one sync, for a [`Reader`].
#[derive(Clone, Copy, Debug)]
pub struct ChipView<'a> {
    chips: &'a [f32],
    power: &'a [f32],
    base: usize,
    mask: usize,
    k: u64,
    n: u64,
    cfg: &'a ChipConfig,
    floor: Option<f64>,
    after: u64,
    /// The sync was the complement of the pattern: every chip is inverted.
    pub inverted: bool,
    /// The wrong bits of the sync.
    pub sync_errors: u32,
}

impl ChipView<'_> {
    #[inline]
    fn chip(&self, c: u64) -> f32 {
        self.chips[self.base + (c as usize & self.mask)]
    }

    /// The chips after the sync that are there now.
    pub fn chips_after(&self) -> u64 {
        self.after
    }

    /// The chips of one bit.
    pub fn chips_per_bit(&self) -> u64 {
        self.cfg.chips_per_bit()
    }

    /// The level that a plain chip is compared with: the mean of the chips of the sync pattern, which has as many chips of
    /// each kind, so the mean lies between the two tones whatever the carrier offset.
    pub fn centre(&self) -> f32 {
        let n = u64::from(self.cfg.sync_bits);
        let mut s = 0.0;
        for j in 0..n {
            s += self.chip(self.k.wrapping_sub(j));
        }
        s / n as f32
    }

    /// Bit `j` after the sync and its margin. `centre` is the level of [`ChipView::centre`] (not used for Manchester bits).
    #[inline]
    pub fn bit(&self, j: usize, centre: f32) -> (bool, f32) {
        if self.cfg.manchester {
            let c = self.k + 1 + 2 * j as u64;
            let (a, b) = (self.chip(c), self.chip(c + 1));
            ((b > a) != self.inverted, (b - a).abs())
        } else {
            let v = self.chip(self.k + 1 + j as u64);
            ((v > centre) != self.inverted, (v - centre).abs())
        }
    }

    /// Byte `j` after the sync, most significant bit first, by hard decisions.
    pub fn byte(&self, j: usize, centre: f32) -> u8 {
        (0..8).fold(0, |v, b| v << 1 | u8::from(self.bit(j * 8 + b, centre).0))
    }

    /// The level of the burst over the noise floor of the channel, dB, from the mean power of the chips `from` to `to`
    /// counted from the last chip of the sync.
    pub fn level_db(&self, from: i64, to: i64) -> f64 {
        let mut s = 0.0f64;
        for c in from..=to {
            let at = self.k.wrapping_add_signed(c);
            s += f64::from(self.power[self.base + (at as usize & self.mask)]);
        }
        let n = (to - from + 1).max(1) as f64 * self.cfg.samples_per_chip as f64;
        let pm = s / n;
        let nf = self.floor.unwrap_or(pm / 100.0).max(1e-20);
        db((pm - nf).max(0.01 * nf) / nf)
    }

    /// The mean power of the samples of the chips `from` to `to` counted from the last chip of the sync, dBFS.
    pub fn power_dbfs(&self, from: i64, to: i64) -> f64 {
        let mut s = 0.0f64;
        for c in from..=to {
            let at = self.k.wrapping_add_signed(c);
            s += f64::from(self.power[self.base + (at as usize & self.mask)]);
        }
        let n = (to - from + 1).max(1) as f64 * self.cfg.samples_per_chip as f64;
        db((s / n).max(1e-30))
    }

    /// The time of the first chip of the sync, seconds since the configure.
    pub fn sync_time_s(&self) -> f64 {
        let chips = u64::from(self.cfg.sync_bits) * self.cfg.chips_per_bit();
        let first = (self.n + 1).saturating_sub(chips * self.cfg.samples_per_chip as u64);
        first as f64 / self.cfg.work_rate()
    }
}

#[derive(Clone, Copy, Debug)]
struct Cand {
    n: u64,
    phase: usize,
    k: u64,
    inverted: bool,
    errors: u32,
    score: f32,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    c: Cand,
    need: u64,
}

/// The receiver of one channel.
#[derive(Clone, Debug)]
pub struct ChipChannel<R> {
    cfg: ChipConfig,
    /// The reader of the frames.
    pub reader: R,
    front: Option<Channelizer>,
    /// The channel filter at the work rate: the channeliser keeps only its passband free of aliases.
    sharp: Option<Fir<Complex32>>,
    buf: Vec<Complex32>,
    prev: Complex32,
    disc_scale: f32,
    turn: Complex32,
    turn_step: Complex32,
    lo: Vec<Complex32>,
    hi: Vec<Complex32>,
    lo_sum: Complex32,
    hi_sum: Complex32,
    disc: Vec<f32>,
    pw: Vec<f32>,
    di: usize,
    dsum: f32,
    psum: f32,
    n: u64,
    chips: Vec<f32>,
    cpow: Vec<f32>,
    mean: Vec<f32>,
    regs: Vec<[u32; 2]>,
    group: Option<(u64, Cand)>,
    last_taken: Option<u64>,
    pending: Vec<Pending>,
    block: usize,
    block_sum: f64,
    block_n: usize,
    blocks: Vec<f32>,
    block_i: usize,
    sorted: Vec<f32>,
    floor: Option<f64>,
    energy_at: Option<f64>,
    stats: ChipStats,
}

impl<R: Reader> ChipChannel<R> {
    /// The receiver of a channel with the signal `cfg`, whose frames `reader` reads. It runs after [`ChipChannel::configure`].
    pub fn new(cfg: ChipConfig, reader: R) -> Self {
        let spc = cfg.samples_per_chip.max(1);
        let ring = cfg.ring_chips.next_power_of_two();
        let rate = cfg.work_rate();
        let w = 2.0 * PI * cfg.deviation_hz / rate;
        ChipChannel {
            reader,
            front: None,
            sharp: None,
            buf: Vec::new(),
            prev: Complex32::new(0.0, 0.0),
            disc_scale: (rate / (2.0 * PI * cfg.deviation_hz)) as f32,
            turn: Complex32::new(1.0, 0.0),
            turn_step: Complex32::new(w.cos() as f32, -w.sin() as f32),
            lo: vec![Complex32::default(); spc],
            hi: vec![Complex32::default(); spc],
            lo_sum: Complex32::default(),
            hi_sum: Complex32::default(),
            disc: vec![0.0; spc],
            pw: vec![0.0; spc],
            di: 0,
            dsum: 0.0,
            psum: 0.0,
            n: 0,
            chips: vec![0.0; spc * ring],
            cpow: vec![0.0; spc * ring],
            mean: vec![0.0; spc],
            regs: vec![[0; 2]; spc],
            group: None,
            last_taken: None,
            pending: Vec::with_capacity(PENDING),
            block: (0.02 * rate).round().max(1.0) as usize,
            block_sum: 0.0,
            block_n: 0,
            blocks: Vec::with_capacity(BLOCKS),
            block_i: 0,
            sorted: Vec::with_capacity(BLOCKS),
            floor: None,
            energy_at: None,
            stats: ChipStats::default(),
            cfg,
        }
    }

    /// The signal of the channel.
    pub fn config(&self) -> &ChipConfig {
        &self.cfg
    }

    /// Starts again for an input at `rate` with the channel `offset_hz` from its centre. The filters, the detector, the noise
    /// floor and the sync search start again. The counters and the reader stay.
    ///
    /// # Errors
    ///
    /// The errors of the channeliser and of the filter design for a rate that they refuse.
    pub fn configure(
        &mut self,
        rate: f64,
        offset_hz: f64,
    ) -> Result<(), sdrkit_algo_dsp::DspError> {
        let work = self.cfg.work_rate();
        let mut c = ChannelConfig::new(rate, offset_hz, 2.0 * self.cfg.pass_hz, work);
        c.exact_rate = true;
        c.atten_db = 60.0;
        let front = Channelizer::with_stop(c, self.cfg.stop_hz)?;
        let sharp = Fir::new(&lowpass(work, self.cfg.pass_hz, self.cfg.stop_hz, 50.0)?);
        let stats = self.stats;
        let fresh = ChipChannel::new(self.cfg.clone(), ());
        self.front = Some(front);
        self.sharp = Some(sharp);
        self.prev = fresh.prev;
        self.turn = fresh.turn;
        self.lo = fresh.lo;
        self.hi = fresh.hi;
        self.lo_sum = fresh.lo_sum;
        self.hi_sum = fresh.hi_sum;
        self.disc = fresh.disc;
        self.pw = fresh.pw;
        self.di = 0;
        self.dsum = 0.0;
        self.psum = 0.0;
        self.n = 0;
        self.chips = fresh.chips;
        self.cpow = fresh.cpow;
        self.mean = fresh.mean;
        self.regs = fresh.regs;
        self.group = None;
        self.last_taken = None;
        self.pending.clear();
        self.block_sum = 0.0;
        self.block_n = 0;
        self.blocks.clear();
        self.block_i = 0;
        self.floor = None;
        self.energy_at = None;
        self.stats = stats;
        Ok(())
    }

    /// Stops the channel: it takes no samples until the next [`ChipChannel::configure`].
    pub fn stop(&mut self) {
        self.front = None;
        self.pending.clear();
        self.group = None;
        self.energy_at = None;
    }

    /// The channel runs.
    pub fn running(&self) -> bool {
        self.front.is_some()
    }

    /// The counters so far.
    pub fn stats(&self) -> ChipStats {
        self.stats
    }

    /// Sets the counters to zero.
    pub fn reset_stats(&mut self) {
        self.stats = ChipStats::default();
    }

    /// The noise floor, dBFS for each sample. `None` until it is measured.
    pub fn noise_dbfs(&self) -> Option<f64> {
        self.floor.map(db)
    }

    /// The time of the last 20 ms block with more than 1.6 times the noise floor, seconds since the configure.
    pub fn energy_at(&self) -> Option<f64> {
        self.energy_at
    }

    /// Feeds input samples. The reader takes the frames.
    pub fn feed(&mut self, x: &[Complex32]) {
        let Some(front) = self.front.as_mut() else {
            return;
        };
        let mut buf = std::mem::take(&mut self.buf);
        buf.clear();
        front.process(x, &mut buf);
        if let Some(f) = self.sharp.as_mut() {
            f.process_in_place(&mut buf);
        }
        for &y in &buf {
            self.sample(y);
        }
        self.buf = buf;
    }

    /// The chip value of the chip time that ends with `y`, and its power.
    #[inline]
    fn chip_value(&mut self, y: Complex32) -> (f32, f32) {
        let spc = self.cfg.samples_per_chip;
        let p = y.norm_sqr();
        self.psum += p - self.pw[self.di];
        self.pw[self.di] = p;
        let v = match self.cfg.detector {
            Detector::Discriminator => {
                let d = y * self.prev.conj();
                self.prev = y;
                let f = d.im.atan2(d.re) * self.disc_scale;
                self.dsum += f - self.disc[self.di];
                self.disc[self.di] = f;
                self.dsum
            }
            Detector::ToneEnergy => {
                // `hi` holds the samples turned down by the deviation: a tone at the upper frequency adds up in it.
                let (h, l) = (y * self.turn, y * self.turn.conj());
                self.turn *= self.turn_step;
                self.hi_sum += h - self.hi[self.di];
                self.lo_sum += l - self.lo[self.di];
                self.hi[self.di] = h;
                self.lo[self.di] = l;
                self.hi_sum.norm_sqr() - self.lo_sum.norm_sqr()
            }
        };
        self.di = (self.di + 1) % spc;
        if self.di == 0 {
            // The running sums are made anew each chip time, against rounding drift.
            self.psum = self.pw.iter().sum();
            match self.cfg.detector {
                Detector::Discriminator => self.dsum = self.disc.iter().sum(),
                Detector::ToneEnergy => {
                    self.hi_sum = self.hi.iter().sum();
                    self.lo_sum = self.lo.iter().sum();
                    let m = self.turn.norm();
                    if m > 0.0 {
                        self.turn /= m;
                    }
                }
            }
        }
        (v, p)
    }

    #[inline]
    fn sample(&mut self, y: Complex32) {
        let (v, p) = self.chip_value(y);
        let spc = self.cfg.samples_per_chip;
        let mask = self.chips.len() / spc - 1;
        let n = self.n;
        self.n += 1;
        let phase = (n % spc as u64) as usize;
        let k = n / spc as u64;
        let base = phase * (mask + 1);
        let slot = base + (k as usize & mask);
        self.chips[slot] = v;
        self.cpow[slot] = self.psum;
        let bits = self.cfg.sync_bits;
        let low = u32::MAX >> (32 - bits);
        if k >= 1 {
            let (bit, pairing) = if self.cfg.manchester {
                let prev = self.chips[base + ((k - 1) as usize & mask)];
                (v > prev, (k & 1) as usize)
            } else {
                // The mean of the last 32 chips of this phase, made anew every 256 chips.
                let old = self.chips[base + (k.wrapping_sub(MEAN_CHIPS) as usize & mask)];
                self.mean[phase] += (v - old) / MEAN_CHIPS as f32;
                if k.is_multiple_of(256) {
                    let mut s = 0.0;
                    for j in 0..MEAN_CHIPS {
                        s += self.chips[base + (k.wrapping_sub(j) as usize & mask)];
                    }
                    self.mean[phase] = s / MEAN_CHIPS as f32;
                }
                (v > self.mean[phase], 0)
            };
            let r = {
                let r = &mut self.regs[phase][pairing];
                *r = (*r << 1) | u32::from(bit);
                *r
            };
            if n >= self.cfg.skip_samples {
                let e = ((r ^ self.cfg.sync) & low).count_ones();
                // Plain chips are first compared with the running mean, which holds what came before the burst while the
                // burst is young: a loose match there is counted again against the mean of the pattern itself.
                let loose = if self.cfg.manchester { 2 } else { 6 };
                let found = (e <= loose || e >= bits - loose)
                    .then(|| {
                        if self.cfg.manchester {
                            return Some((e, 0.0));
                        }
                        let at = |c: u64| self.chips[base + (c as usize & mask)];
                        let m = (0..u64::from(bits))
                            .map(|j| at(k.wrapping_sub(j)))
                            .sum::<f32>()
                            / bits as f32;
                        let e = (0..bits)
                            .filter(|&j| {
                                (at(k.wrapping_sub(u64::from(j))) > m)
                                    != (self.cfg.sync >> j & 1 != 0)
                            })
                            .count() as u32;
                        (e <= 2 || e >= bits - 2).then_some((e, m))
                    })
                    .flatten();
                if let Some((e, m)) = found {
                    let inverted = e >= bits - 2;
                    let c = Cand {
                        n,
                        phase,
                        k,
                        inverted,
                        errors: if inverted { bits - e } else { e },
                        score: self.score(base, mask, k, m),
                    };
                    self.group = Some(match self.group {
                        Some((n0, b)) if n <= n0 + 3 * spc as u64 => {
                            (n0, if c.score > b.score { c } else { b })
                        }
                        Some((_, b)) => {
                            self.take(b);
                            (n, c)
                        }
                        None => (n, c),
                    });
                }
            }
        }
        if let Some((n0, b)) = self.group {
            if n > n0 + 3 * spc as u64 {
                self.group = None;
                self.take(b);
            }
        }
        if !self.pending.is_empty() {
            self.finish_frames(phase, k);
        }
        // The noise floor from 20 ms blocks.
        self.block_sum += f64::from(p);
        self.block_n += 1;
        if self.block_n == self.block {
            self.end_block(n);
        }
    }

    /// How clearly the chips of a sync that ends at chip `k` show its bits.
    fn score(&self, base: usize, mask: usize, k: u64, mean: f32) -> f32 {
        let at = |c: u64| self.chips[base + (c as usize & mask)];
        let mut s = 0.0;
        for j in 0..u64::from(self.cfg.sync_bits) {
            s += if self.cfg.manchester {
                (at(k.wrapping_sub(2 * j)) - at(k.wrapping_sub(2 * j + 1))).abs()
            } else {
                (at(k.wrapping_sub(j)) - mean).abs()
            };
        }
        s
    }

    /// Takes the best candidate of a group, unless it lies within 3 bits of the last sync taken.
    fn take(&mut self, c: Cand) {
        let near = 3 * self.cfg.chips_per_bit() * self.cfg.samples_per_chip as u64;
        if self.last_taken.is_some_and(|l| c.n <= l + near) {
            return;
        }
        self.last_taken = Some(c.n);
        self.stats.syncs += 1;
        if self.pending.len() < PENDING {
            self.pending.push(Pending {
                c,
                need: self.reader.first_need(),
            });
        }
    }

    fn finish_frames(&mut self, phase: usize, k: u64) {
        let mut i = 0;
        while i < self.pending.len() {
            let p = self.pending[i];
            if p.c.phase != phase || k < p.c.k + p.need {
                i += 1;
                continue;
            }
            let ring = self.chips.len() / self.cfg.samples_per_chip;
            let view = ChipView {
                chips: &self.chips,
                power: &self.cpow,
                base: phase * ring,
                mask: ring - 1,
                k: p.c.k,
                n: p.c.n,
                cfg: &self.cfg,
                floor: self.floor,
                after: k - p.c.k,
                inverted: p.c.inverted,
                sync_errors: p.c.errors,
            };
            match self.reader.read(&view) {
                // A frame longer than the ring cannot be read.
                Outcome::Need(more) if more > p.need && (more as usize) < ring - 64 => {
                    self.pending[i].need = more;
                    i += 1;
                    continue;
                }
                Outcome::Good => self.stats.good += 1,
                _ => {
                    if p.c.errors <= 1 {
                        self.stats.bad += 1;
                    }
                }
            }
            self.pending.swap_remove(i);
        }
    }

    fn end_block(&mut self, n: u64) {
        let m = (self.block_sum / self.block_n as f64) as f32;
        self.block_sum = 0.0;
        self.block_n = 0;
        if self.blocks.len() < BLOCKS {
            self.blocks.push(m);
        } else {
            self.blocks[self.block_i] = m;
            self.block_i = (self.block_i + 1) % BLOCKS;
        }
        if self.blocks.len() >= 3 {
            self.sorted.clear();
            self.sorted.extend_from_slice(&self.blocks);
            self.sorted
                .sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let i = ((self.sorted.len() - 1) as f64 * 0.1).floor() as usize;
            let fl = 1.05 * f64::from(self.sorted[i]);
            self.floor = Some(fl);
            if f64::from(m) > 1.6 * fl {
                self.energy_at = Some(n as f64 / self.cfg.work_rate());
            }
        }
    }
}

impl Reader for () {
    fn first_need(&self) -> u64 {
        0
    }
    fn read(&mut self, _view: &ChipView<'_>) -> Outcome {
        Outcome::Bad
    }
}
