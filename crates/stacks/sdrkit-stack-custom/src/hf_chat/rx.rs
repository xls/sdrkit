//! The receiver of `hf_chat` at 2400 samples a second (section 11). It needs no clock: it searches the stream for lattice
//! units, each start and carrier offset from the pair of rising and falling sweep sums ([`SweepSearch`]); a profile with
//! several units (0 and 1) is found by the sum of its units at their offsets. A unit found starts a try of every profile
//! listened to that has that many units. A try waits for its samples, finds the closing weave (which also gives the number
//! of blocks of a burst that can carry more than one), removes a carrier that follows a straight line between the offsets of
//! the units and of the closing weave, and decodes each block. Chirp symbols: every path is read at its own start, found to
//! a fraction of a chip from the data symbols, the bin energies of the paths are added, and the demapping is iterative (the
//! decoder's extrinsic LLRs fed back as a priori LLRs). OFDM symbols: the burst is moved onto whole samples, the channel
//! comes from the pilots, then from every carrier weighted by its decision.

use super::phy::{self, closing_sweeps, lattice_sweeps, layout, Layout, CARRIERS, OFDM_SYMBOL};
use super::{decoder, frame, permutation, Modulation, Profile, CLOSING, PROFILES, RATE, UNIT};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::interp::{Polyphase, Reader};
use sdrkit_algo_dsp::linalg::cholesky_solve;
use sdrkit_algo_fec::ldpc::LdpcDecoder;
use sdrkit_algo_modem::chirp::{Dechirp, Direction};
use sdrkit_algo_modem::fsk::{ln_i0, FskLlr};
use sdrkit_algo_modem::sweeps::{pair_max, peak_near, SweepSearch};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The most samples a rising and a falling peak move apart for a carrier offset: 160 samples, about 107 Hz.
pub const MAX_SHIFT: usize = 160;
/// The median of the pair statistic `T` of one unit in noise alone.
pub const T_MEDIAN: f64 = 18.43;
/// The spread of `T` in noise alone.
pub const T_SPREAD: f64 = 1.82;
/// The burst statistic a detection needs, for 1, 2 and 3 or more units.
pub fn threshold(units: usize) -> f64 {
    match units {
        1 => 16.0,
        2 => 10.5,
        _ => 9.5,
    }
}
/// `-ln 0.85`: the 15 % point of the power of complex Gaussian noise of unit variance.
const NOISE_QUANTILE: f64 = 0.162_518_9;
/// Samples of the stream the noise floor is taken over.
const NOISE_WINDOW: usize = 6 * 2400;
/// New samples between two updates of the sums.
const HOP: usize = 7000;
/// Samples after a detection in which a larger statistic replaces it: more than the 3600 samples over which the sweeps of a
/// unit line up in part with themselves.
const PEAK_GUARD: usize = 6000;
/// The scale of the LLRs given to the decoder, units per natural LLR (LLRs are limited to +-30 first).
const LLR_SCALE: f32 = 4.0;
/// The a priori LLR of a known zero fill bit of a chirp symbol.
const FILL_PRIOR: f32 = 25.0;
/// Rounds and iterations of the chirp decoder, and of the OFDM decoder.
const CHIRP_ROUNDS: usize = 6;
const CHIRP_ITERS: u32 = 12;
const OFDM_ROUNDS: usize = 8;
const OFDM_ITERS: u32 = 30;
/// Samples either side of the main path that the path search looks at.
const PATH_SPAN: i64 = 20;
/// The most blocks of a push-to-talk burst.
const MAX_VOICE_BLOCKS: usize = 200;
/// The pair statistic the closing weave needs.
const CLOSING_STAT: f64 = 30.0;

/// A burst the receiver found and decoded.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    /// The profile.
    pub profile: u8,
    /// The first sample of the first unit, in samples of the stream from its start.
    pub start: f64,
    /// The carrier offset at the first unit, Hz.
    pub offset_hz: f64,
    /// The SNR, dB in 2.4 kHz.
    pub snr_db: f64,
    /// The detection statistic.
    pub stat: f64,
    /// The `K` information bits of each block whose code word decoded.
    pub blocks: Vec<Option<Vec<u8>>>,
}

/// The profiles that share a number of units: profile 0, profile 1, and the profiles of one unit.
fn unit_groups() -> [(Vec<usize>, Vec<u8>); 3] {
    let offs = |p: &Profile| layout(p, 1).units;
    [(offs(&PROFILES[0]), vec![0]), (offs(&PROFILES[1]), vec![1]), (vec![0], vec![2, 3, 4, 5, 6, 7])]
}

/// The search state of one group.
#[derive(Clone, Debug)]
struct GroupSearch {
    offsets: Vec<usize>,
    profiles: Vec<u8>,
    /// The next start (absolute) whose statistic is to be read.
    next: u64,
    /// The best start of the run above the threshold, and its statistic.
    best: Option<(u64, f64)>,
    /// The later differences of the unit offsets: a burst at `t` also lines some of its units up at `t - d`.
    aliases: Vec<usize>,
    /// Peaks waiting for the statistic at their aliases.
    waiting: Vec<(u64, f64)>,
}

/// A burst waiting for its samples.
#[derive(Clone, Debug)]
struct Job {
    profile: u8,
    /// The detected start of the first unit (absolute sample).
    start: u64,
    stat: f64,
    /// Block counts already tested for a closing weave.
    tested: usize,
}

/// The `hf_chat` receiver (module documentation).
pub struct Receiver {
    listen: [bool; 8],
    x: Vec<Complex32>,
    /// The absolute index of `x[0]`.
    base: u64,
    search: SweepSearch,
    closing: SweepSearch,
    /// Normalised pair statistic `T` for the starts `base ..`: `NaN` where it is not known yet.
    t: Vec<f32>,
    /// The rising and falling sums for the starts `base ..`.
    a: Vec<f32>,
    b: Vec<f32>,
    /// The best shift of each `T`.
    shift: Vec<i32>,
    noise: f32,
    groups: Vec<GroupSearch>,
    jobs: Vec<Job>,
    decoders: Vec<LdpcDecoder>,
    reader: Reader<Polyphase<16, 64>>,
    /// The spans of the bursts decoded: a try that starts inside one is a part of it (a sidelobe, or the units of profile 0
    /// lined up with themselves at another offset) and is dropped.
    done: Vec<(u64, u64)>,
}

impl Receiver {
    /// A receiver of the profiles `listen[p]` is true for.
    pub fn new(listen: [bool; 8]) -> Receiver {
        let mut groups = Vec::new();
        for (offsets, profiles) in unit_groups() {
            let profiles: Vec<u8> = profiles.into_iter().filter(|&p| listen[usize::from(p)]).collect();
            if !profiles.is_empty() {
                let mut aliases: Vec<usize> = offsets.iter().flat_map(|&a| offsets.iter().filter(move |&&b| b > a).map(move |&b| b - a)).collect();
                aliases.sort_unstable();
                aliases.dedup();
                groups.push(GroupSearch { offsets, profiles, next: MAX_SHIFT as u64, best: None, aliases, waiting: Vec::new() });
            }
        }
        Receiver {
            listen,
            x: Vec::new(),
            base: 0,
            search: SweepSearch::new(RATE, &lattice_sweeps(), phy::RAMP).expect("the lattice sweeps"),
            closing: SweepSearch::new(RATE, &closing_sweeps(), phy::RAMP).expect("the closing sweeps"),
            t: Vec::new(),
            a: Vec::new(),
            b: Vec::new(),
            shift: Vec::new(),
            noise: 0.0,
            groups,
            jobs: Vec::new(),
            decoders: PROFILES.iter().map(decoder).collect(),
            reader: Reader::new(Polyphase::new(0.95, 5.0).expect("an interpolator")),
            done: Vec::new(),
        }
    }

    /// The profiles listened to.
    pub fn listen(&self) -> [bool; 8] {
        self.listen
    }

    /// Forgets the input and every try in progress.
    pub fn reset(&mut self) {
        *self = Receiver::new(self.listen);
    }

    /// The samples taken so far.
    pub fn position(&self) -> u64 {
        self.base + self.x.len() as u64
    }

    /// Takes baseband samples and appends the bursts decoded.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Burst>) {
        self.x.extend_from_slice(input);
        let span = self.search.span();
        while self.x.len() >= self.a.len() + span + HOP {
            self.extend(self.x.len() - span + 1);
        }
        self.run_jobs(out, false);
        self.trim();
    }

    /// Ends the input: reads the statistic to its end, and runs every try whose samples are there.
    pub fn finish(&mut self, out: &mut Vec<Burst>) {
        let span = self.search.span();
        if self.x.len() >= span {
            self.extend(self.x.len() - span + 1);
        }
        for g in 0..self.groups.len() {
            if let Some((at, stat)) = self.groups[g].best.take() {
                self.found(g, at, stat);
            }
        }
        self.confirm(true);
        self.run_jobs(out, true);
    }

    /// Computes the sums and `T` up to `starts` starts (relative), and reads the group statistics.
    fn extend(&mut self, starts: usize) {
        let from = self.a.len();
        if starts <= from {
            return;
        }
        let (a, b) = self.search.sums(&self.x[from..from + starts - from + self.search.span() - 1]);
        self.a.extend_from_slice(&a);
        self.b.extend_from_slice(&b);
        // The noise floor of the last seconds.
        let lo = self.x.len().saturating_sub(NOISE_WINDOW);
        // A stream without noise has no floor: a millionth of its power stands in.
        let w = &self.x[lo..];
        let mean = w.iter().map(|v| v.norm_sqr()).sum::<f32>() / w.len().max(1) as f32;
        self.noise = floor_of(w).max(1e-6 * mean).max(1e-20);
        let n = self.a.len();
        let first = self.t.len();
        let have = n.saturating_sub(MAX_SHIFT);
        if have > first {
            self.t.resize(have, f32::NAN);
            self.shift.resize(have, 0);
            let lo = first.max(MAX_SHIFT) - MAX_SHIFT;
            let pm = pair_max(&self.a[lo..have + MAX_SHIFT], &self.b[lo..have + MAX_SHIFT], MAX_SHIFT);
            for (i, (v, s)) in pm.into_iter().enumerate() {
                let at = lo + MAX_SHIFT + i;
                if at >= first {
                    self.t[at] = v / self.noise;
                    self.shift[at] = s;
                }
            }
        }
        self.scan_groups();
        self.confirm(false);
    }

    /// Reads the statistic of each group at every start whose units all have `T`, and keeps the peaks above the threshold.
    fn scan_groups(&mut self) {
        let known = self.base + self.t.len() as u64;
        for g in 0..self.groups.len() {
            let last = *self.groups[g].offsets.last().unwrap_or(&0) as u64;
            let units = self.groups[g].offsets.len();
            let thr = threshold(units);
            while self.groups[g].next + last < known {
                let at = self.groups[g].next;
                self.groups[g].next += 1;
                let Some(rel) = at.checked_sub(self.base) else {
                    continue;
                };
                let sum: f64 = self.groups[g].offsets.iter().map(|&o| f64::from(self.t[rel as usize + o])).sum();
                if !sum.is_finite() {
                    continue;
                }
                let stat = (sum - T_MEDIAN * units as f64) / (T_SPREAD * (units as f64).sqrt());
                if let Some((b_at, b_stat)) = self.groups[g].best {
                    if at <= b_at + PEAK_GUARD as u64 {
                        if stat > b_stat {
                            self.groups[g].best = Some((at, stat));
                        }
                        continue;
                    }
                    self.groups[g].best = None;
                    self.found(g, b_at, b_stat);
                }
                if stat >= thr {
                    self.groups[g].best = Some((at, stat));
                }
            }
        }
    }

    /// A burst of group `g` starts near `at`: a try for each of its profiles.
    fn found(&mut self, g: usize, at: u64, stat: f64) {
        if self.groups[g].aliases.is_empty() {
            for &p in &self.groups[g].profiles {
                self.jobs.push(Job { profile: p, start: at, stat, tested: 0 });
            }
        } else {
            self.groups[g].waiting.push((at, stat));
        }
    }

    /// The statistic of group `g` at the absolute start `at`, when its units are known.
    fn stat_at(&self, g: usize, at: u64) -> Option<f64> {
        let rel = at.checked_sub(self.base)? as usize;
        let units = self.groups[g].offsets.len() as f64;
        let mut sum = 0f64;
        for &o in &self.groups[g].offsets {
            sum += f64::from(*self.t.get(rel + o)?);
        }
        sum.is_finite().then(|| (sum - T_MEDIAN * units) / (T_SPREAD * units.sqrt()))
    }

    /// Starts the tries of the peaks of groups of several units once the statistic at their aliases is known: a peak whose
    /// statistic a later alias beats is the shadow of the burst there, which is found on its own.
    fn confirm(&mut self, end: bool) {
        let known = self.base + self.t.len() as u64;
        for g in 0..self.groups.len() {
            let last = *self.groups[g].offsets.last().unwrap_or(&0) as u64;
            let span = self.groups[g].aliases.last().copied().unwrap_or(0) as u64 + last + 20;
            let mut i = 0;
            while i < self.groups[g].waiting.len() {
                let (at, stat) = self.groups[g].waiting[i];
                if at + span >= known && !end {
                    i += 1;
                    continue;
                }
                self.groups[g].waiting.remove(i);
                let shadow = self.groups[g].aliases.clone().into_iter().any(|d| {
                    (at + d as u64 - 16..=at + d as u64 + 16).any(|t| self.stat_at(g, t).is_some_and(|s| s > stat))
                });
                if !shadow {
                    for &p in &self.groups[g].profiles {
                        self.jobs.push(Job { profile: p, start: at, stat, tested: 0 });
                    }
                }
            }
        }
    }

    /// Drops the samples no try and no search needs.
    fn trim(&mut self) {
        let mut keep = self.base + self.a.len() as u64;
        for g in &self.groups {
            keep = keep.min(g.next.saturating_sub(PEAK_GUARD as u64)).min(g.best.map_or(u64::MAX, |b| b.0));
            keep = g.waiting.iter().fold(keep, |k, w| k.min(w.0));
        }
        for j in &self.jobs {
            keep = keep.min(j.start);
        }
        let keep = keep.saturating_sub((NOISE_WINDOW + 2 * MAX_SHIFT + 4000) as u64);
        let drop = keep.saturating_sub(self.base) as usize;
        if drop > 4 * HOP && drop <= self.x.len() {
            self.x.drain(..drop);
            let d = drop.min(self.a.len());
            self.a.drain(..d);
            self.b.drain(..d);
            let d = drop.min(self.t.len());
            self.t.drain(..d);
            self.shift.drain(..d);
            self.base += drop as u64;
        }
        self.done.retain(|d| d.1 > self.base);
    }

    /// Runs the tries whose samples are there (all of them at the end of the input).
    fn run_jobs(&mut self, out: &mut Vec<Burst>, end: bool) {
        let mut i = 0;
        while i < self.jobs.len() {
            let job = self.jobs[i].clone();
            if self.done.iter().any(|d| job.start + 1200 > d.0 && job.start < d.1) {
                self.jobs.remove(i);
                continue;
            }
            match self.try_job(&job, end) {
                Step::Wait(tested) => {
                    self.jobs[i].tested = tested;
                    i += 1;
                }
                Step::Fail => {
                    self.jobs.remove(i);
                }
                Step::Done(b) => {
                    let p = &PROFILES[usize::from(job.profile)];
                    self.done.push((job.start, b.start.max(0.0) as u64 + p.burst_samples(b.blocks.len()) as u64));
                    self.jobs.remove(i);
                    out.push(b);
                }
            }
        }
    }

    fn try_job(&mut self, job: &Job, end: bool) -> Step {
        let p = &PROFILES[usize::from(job.profile)];
        let have = self.position();
        let bps = if p.slot_s.is_some() { p.blocks_per_slot() } else { MAX_VOICE_BLOCKS };
        let margin = (MAX_SHIFT + 64) as u64;
        // The block count from the closing weave, for a burst that can hold more than one block.
        let mut count = None;
        let mut tested = job.tested;
        if bps > 1 {
            while tested < bps {
                let c = tested + 1;
                let need = job.start + p.burst_samples(c) as u64 + margin;
                if need > have {
                    if end {
                        break;
                    }
                    return Step::Wait(tested);
                }
                tested = c;
                let at = job.start + layout(p, c).closing as u64;
                if self.closing_at(at).is_some() {
                    count = Some(c);
                    break;
                }
            }
        }
        let blocks = count.unwrap_or(1);
        let need = job.start + p.burst_samples(blocks) as u64 + margin;
        if need > have && !end {
            return Step::Wait(tested);
        }
        let Some(mut burst) = self.decode(p, job.start, blocks, job.stat) else {
            return Step::Fail;
        };
        // One block of a burst that may hold more: the first block's header gives the length of the message.
        if count.is_none() && bps > 1 {
            if let Some(Some(bits)) = burst.blocks.first() {
                if let Some(frame::Block::Part { index: 0, payload, .. }) = frame::parse_block(bits) {
                    let m = frame::message_len(&payload).map_or(1, |len| len.div_ceil(p.usable()));
                    if m > 1 && m <= bps {
                        if job.start + p.burst_samples(m) as u64 + margin > have && !end {
                            return Step::Wait(tested);
                        }
                        if let Some(b) = self.decode(p, job.start, m, job.stat) {
                            burst = b;
                        }
                    }
                }
            }
        }
        if burst.blocks.iter().any(Option::is_some) { Step::Done(burst) } else { Step::Fail }
    }

    /// The closing weave near absolute sample `at`: its start and carrier offset, when its statistic reaches
    /// [`CLOSING_STAT`].
    fn closing_at(&mut self, at: u64) -> Option<(f64, f64)> {
        let shift = 100usize;
        let lo = at.checked_sub(self.base + (shift + 40) as u64)? as usize;
        let hi = (lo + 2 * (shift + 40) + self.closing.span()).min(self.x.len());
        if hi < lo + self.closing.span() + 2 * shift + 80 {
            return None;
        }
        let (a, b) = self.closing.sums(&self.x[lo..hi]);
        let t = pair_max(&a, &b, shift);
        // Starts within 40 samples of the expected one.
        let centre = (shift + 40) - shift;
        let (i, &(v, s)) = t.iter().enumerate().skip(centre.saturating_sub(40)).take(81).max_by(|x, y| x.1 .0.total_cmp(&y.1 .0))?;
        if f64::from(v / self.noise.max(1e-20)) < CLOSING_STAT {
            return None;
        }
        let x = (i + shift) as i64;
        let (pk, qk) = same_path(&a, &b, x - i64::from(s), x + i64::from(s), 2.0 * self.noise)?;
        Some((lo as f64 + self.base as f64 + (pk + qk) / 2.0, self.closing.offset_hz(pk, qk)))
    }

    /// The start and carrier offset of the units of a burst whose first unit is near `start`, from the sums of all units at
    /// their offsets: the strongest path, its rising and falling peak paired by [`same_path`]. Also the offsets of the units
    /// whose own statistic stands out, and the power of the sums at delays of -[`PATH_SPAN`] to +[`PATH_SPAN`] samples from
    /// that path, the mean of the noise taken off.
    fn units_of(&self, offsets: &[usize], start: u64) -> Option<Units> {
        let rel = start.checked_sub(self.base)? as i64;
        let len = self.a.len() as i64;
        let search = 12i64;
        // The sums of the units, for starts rel - search - MAX_SHIFT ..
        let w = (2 * (search + MAX_SHIFT as i64) + 9) as usize;
        let lo = rel - search - MAX_SHIFT as i64 - 4;
        let (mut sa, mut sb) = (vec![0f32; w], vec![0f32; w]);
        for &o in offsets {
            for k in 0..w {
                let i = lo + k as i64 + o as i64;
                if i >= 0 && i < len {
                    sa[k] += self.a[i as usize];
                    sb[k] += self.b[i as usize];
                }
            }
        }
        let t = pair_max(&sa, &sb, MAX_SHIFT);
        let (i, &(_, s)) = t.iter().enumerate().max_by(|x, y| x.1 .0.total_cmp(&y.1 .0))?;
        let x = (i + MAX_SHIFT) as i64;
        let mean = 4.0 * self.noise * offsets.len() as f32;
        let (pk, qk) = same_path(&sa, &sb, x - i64::from(s), x + i64::from(s), mean)?;
        let at = |v: &[f32], c: f64, d: i64| usize::try_from(c.round() as i64 + d).ok().and_then(|i| v.get(i)).map_or(0.0, |&e| f64::from(e - mean));
        let lattice: Vec<f64> = (-PATH_SPAN..=PATH_SPAN).map(|d| at(&sa, pk, d) + at(&sb, qk, d)).collect();
        let t0 = lo as f64 + (pk + qk) / 2.0;
        let f0 = self.search.offset_hz(pk, qk);
        // Each unit alone, when its statistic is clear: its offset at its middle.
        let mut points = Vec::new();
        for &o in offsets {
            let c = (t0.round() as i64 + o as i64) as usize;
            let Some(&tv) = self.t.get(c) else {
                continue;
            };
            if !tv.is_finite() || (f64::from(tv) - T_MEDIAN) / T_SPREAD < 8.0 {
                continue;
            }
            let s = i64::from(self.shift[c]);
            let Some((p, q)) = same_path(&self.a, &self.b, c as i64 - s, c as i64 + s, 4.0 * self.noise) else {
                continue;
            };
            points.push(((p + q) / 2.0 + UNIT as f64 / 2.0, self.search.offset_hz(p, q)));
        }
        Some((t0 + self.base as f64, f0, points, lattice))
    }

    /// Decodes a burst of profile `p` with `blocks` blocks whose first unit is near `start`.
    fn decode(&mut self, p: &Profile, start: u64, blocks: usize, stat: f64) -> Option<Burst> {
        let l = layout(p, blocks);
        let (tau, f0, mut points, lattice) = self.units_of(&l.units, start)?;
        if let Some((c, fc)) = self.closing_at((tau + l.closing as f64).round() as u64) {
            if (fc - f0).abs() < 100.0 {
                points.push((c - self.base as f64 + CLOSING as f64 / 2.0, fc));
            }
        }
        // The carrier: a straight line between the points, constant outside; the mean offset with one point or none.
        points.sort_by(|x, y| x.0.total_cmp(&y.0));
        if points.len() < 2 {
            points = vec![(0.0, f0)];
        }
        let mut rel = tau - self.base as f64;
        let mut out = Vec::with_capacity(blocks);
        let mut snrs = Vec::new();
        match p.modulation {
            Modulation::Chirp { .. } => {
                // The start of the burst: that of the lattice, moved by what the first block decoded found.
                let mut moved = None;
                for b in 0..blocks {
                    let got = self.chirp_block(p, &l, b, rel, &points, &lattice).filter(|g| valid(p, &g.0));
                    out.push(got.map(|(bits, snr, shift)| {
                        snrs.push(snr);
                        moved.get_or_insert(shift);
                        bits
                    }));
                }
                rel += moved.unwrap_or(0.0);
            }
            Modulation::Ofdm { .. } => {
                rel += centroid(&lattice, 3.0 * f64::from(self.noise) * (16.0 * l.units.len() as f64).sqrt());
                let y = self.burst_samples(rel, l.total, &points);
                for b in 0..blocks {
                    out.push(self.ofdm_block(p, &l, &y, b).filter(|g| valid(p, &g.0)).map(|(bits, snr)| {
                        snrs.push(snr);
                        bits
                    }));
                }
            }
        }
        // The SNR of the decoded blocks, which the known symbols measure well; the power of the burst over the noise beside
        // it when none decoded.
        let snr_db = if snrs.is_empty() {
            self.snr(rel, l.total)
        } else {
            10.0 * (snrs.iter().sum::<f64>() / snrs.len() as f64).max(1e-4).log10()
        };
        let offset_hz = carrier_at(&points, rel);
        Some(Burst { profile: p.id, start: rel + self.base as f64, offset_hz, snr_db, stat, blocks: out })
    }

    /// `len` samples of the stream from relative position `rel`, read at whole steps through the interpolator, with the
    /// carrier of `points` (relative sample, Hz) removed.
    fn burst_samples(&self, rel: f64, len: usize, points: &[(f64, f64)]) -> Vec<Complex32> {
        let mut phase = 0f64;
        (0..len)
            .map(|n| {
                let pos = rel + n as f64;
                let v = self.reader.at(&self.x, pos) * Complex32::new(phase.cos() as f32, -phase.sin() as f32);
                phase = (phase + TAU * carrier_at(points, pos) / RATE) % TAU;
                v
            })
            .collect()
    }

    /// The SNR of the burst at `rel` of `len` samples, dB in 2.4 kHz: its mean power over the noise of the samples more than
    /// 60 samples outside it (the floor of the stream when fewer than 1500 are there), less 1.
    fn snr(&self, rel: f64, len: usize) -> f64 {
        let (s0, s1) = ((rel.max(0.0)) as usize, ((rel + len as f64) as usize).min(self.x.len()));
        let mean = |r: std::ops::Range<usize>| r.clone().map(|i| f64::from(self.x[i].norm_sqr())).sum::<f64>() / r.len().max(1) as f64;
        let before = s0.saturating_sub(60 + 3600)..s0.saturating_sub(60);
        let after = (s1 + 60).min(self.x.len())..(s1 + 60 + 3600).min(self.x.len());
        let quiet = before.len() + after.len();
        let noise = if quiet >= 1500 {
            (mean(before.clone()) * before.len() as f64 + mean(after.clone()) * after.len() as f64) / quiet as f64
        } else {
            f64::from(self.noise)
        };
        let p = mean(s0..s1);
        10.0 * (p / noise.max(1e-30) - 1.0).max(1e-4).log10()
    }

    /// The bin energies of every symbol sent of block `b` of a chirp burst (repeats included), the burst read from the
    /// relative position `rel` through the interpolator and the carrier of `points` removed.
    fn chirp_energies(&self, dech: &mut Dechirp, p: &Profile, l: &Layout, b: usize, rel: f64, points: &[(f64, f64)]) -> Option<Vec<f32>> {
        let Modulation::Chirp { guard, .. } = p.modulation else {
            return None;
        };
        let m = dech.chips();
        let weave = phy::weave(p.block_symbols());
        let mut energy = vec![0f32; p.block_symbols() * m];
        let mut buf = vec![Complex32::new(0.0, 0.0); m];
        for seg in l.segments.iter().filter(|s| s.block == b) {
            for k in 0..seg.count {
                let pos = rel + (seg.start + k * p.symbol_len() + guard) as f64;
                for (n, v) in buf.iter_mut().enumerate() {
                    *v = self.reader.at(&self.x, pos + n as f64);
                }
                let dir = if weave[seg.first + k] { Direction::Down } else { Direction::Up };
                let bins = carrier_at(points, pos + m as f64 / 2.0) * m as f64 / RATE;
                dech.dechirp(&buf, dir, bins).ok()?;
                energy[(seg.first + k) * m..(seg.first + k + 1) * m].copy_from_slice(dech.bins());
            }
        }
        Some(energy)
    }

    /// Moves the start of path `k` of `paths` by -0.5 to +0.5 chips (steps of 1/8, then a parabola) to where the strongest
    /// bins of all paths together hold the most energy, and reads its energies there. A path read off a whole chip loses
    /// energy to other bins: a part at the wrap of the chirp turns in phase.
    fn align(&self, dech: &mut Dechirp, p: &Profile, l: &Layout, b: usize, rel: f64, points: &[(f64, f64)], paths: &mut [ChirpPath], k: usize) -> Option<()> {
        let m = dech.chips();
        let mut score = [0f64; 9];
        for (q, sc) in score.iter_mut().enumerate() {
            let e = self.chirp_energies(dech, p, l, b, rel + paths[k].delay + (q as f64 - 4.0) / 8.0, points)?;
            let parts: Vec<&[f32]> = paths.iter().enumerate().map(|(j, o)| if j == k { &e[..] } else { &o.energy[..] }).collect();
            *sc = strongest(&parts, m, p.symbols()).1;
        }
        let i = (0..9).max_by(|&x, &y| score[x].total_cmp(&score[y])).unwrap_or(4);
        let mut d = (i as f64 - 4.0) / 8.0;
        if i > 0 && i < 8 {
            let (a, c) = (score[i - 1], score[i + 1]);
            let curve = a - 2.0 * score[i] + c;
            if curve < 0.0 {
                d += (0.5 * (a - c) / curve).clamp(-0.5, 0.5) / 8.0;
            }
        }
        paths[k].delay += d;
        paths[k].energy = self.chirp_energies(dech, p, l, b, rel + paths[k].delay, points)?;
        Some(())
    }

    /// Decodes block `b` of a chirp burst whose strongest path in the lattice starts at the relative position `rel`: its
    /// information bits, its SNR, and the samples by which the start of that path was moved. Each path is read at its own
    /// start, so that its energy lies in the bin of the value sent, and the bins of the paths are added. The data symbols
    /// give the distance of a second path but not its side; the side with more power in `lattice` is tried first, then the
    /// other.
    fn chirp_block(&mut self, p: &Profile, l: &Layout, b: usize, rel: f64, points: &[(f64, f64)], lattice: &[f64]) -> Option<(Vec<u8>, f64, f64)> {
        let Modulation::Chirp { sf, .. } = p.modulation else {
            return None;
        };
        let m = 1usize << sf;
        let mut dech = Dechirp::new(sf, 1).ok()?;
        let mut main = [ChirpPath { delay: 0.0, energy: self.chirp_energies(&mut dech, p, l, b, rel, points)? }];
        self.align(&mut dech, p, l, b, rel, points, &mut main, 0)?;
        let [main] = main;
        let noise = noise_per_bin(&main.energy, m);
        // Delays up to max(2, M / 8) are combined.
        let reach = (m / 8).clamp(2, PATH_SPAN as usize - 1);
        let sides: Vec<Option<f64>> = match echo(&main.energy, m, p.symbols(), &phy::weave(p.block_symbols()), noise, reach) {
            None => vec![None],
            Some(gap) => {
                let power = |d: i64| (d - 1..=d + 1).map(|i| lattice[(i + PATH_SPAN) as usize]).fold(f64::MIN, f64::max);
                let g = gap as i64;
                let side = if power(g) >= power(-g) { gap as f64 } else { -(gap as f64) };
                vec![Some(side), Some(-side)]
            }
        };
        for side in sides {
            let mut paths = vec![main.clone()];
            if let Some(d) = side {
                paths.push(ChirpPath { delay: main.delay + d, energy: self.chirp_energies(&mut dech, p, l, b, rel + main.delay + d, points)? });
                for k in [1, 0, 1] {
                    self.align(&mut dech, p, l, b, rel, points, &mut paths, k)?;
                }
            }
            if let Some((bits, snr)) = self.chirp_decode(p, &paths, noise) {
                return Some((bits, snr, paths[0].delay));
            }
        }
        None
    }

    /// Decodes a chirp block from the bin energies of its paths (section 11.4), `noise` the noise of a bin: its information
    /// bits and the SNR that the bins of the symbols sent give, the sum over the paths of `(E_sent - N) / (M N)`.
    fn chirp_decode(&mut self, p: &Profile, paths: &[ChirpPath], noise: f32) -> Option<(Vec<u8>, f64)> {
        let Modulation::Chirp { sf, .. } = p.modulation else {
            return None;
        };
        let m = 1usize << sf;
        let sym = p.symbols();
        let parts: Vec<&[f32]> = paths.iter().map(|q| &q.energy[..]).collect();
        // The mean SNR of each path in the bins that hold the most energy.
        let (best, _) = strongest(&parts, m, sym);
        let rho: Vec<f32> = parts.iter().map(|e| ((sent_energy(e, m, &best) / f64::from(noise)) as f32 - 1.0).max(0.0)).collect();
        let gamma = m as f32 * 10f32.powf(p.design_snr_db as f32 / 10.0);
        let wsum: f32 = rho.iter().map(|r| r / (1.0 + r)).sum::<f32>().max(1e-6);
        // Metrics per symbol value, summed over the repeats: the bins of the paths added with the weights rho / (1 + rho).
        let mut metrics = vec![0f32; sym * m];
        let mut comb = vec![0f32; m];
        for i in 0..p.block_symbols() {
            comb.fill(0.0);
            for (e, &r) in parts.iter().zip(&rho) {
                let w = r / (1.0 + r) / noise;
                for (o, &v) in comb.iter_mut().zip(&e[i * m..(i + 1) * m]) {
                    *o += w * v;
                }
            }
            let row = &mut metrics[(i % sym) * m..(i % sym + 1) * m];
            for (o, &v) in row.iter_mut().zip(&comb) {
                *o += ln_i0(2.0 * gamma.sqrt() * (v / wsum).max(0.0).sqrt());
            }
        }
        let labels: Vec<u16> = (0..m as u16).map(gray_inverse).collect();
        let fsk = FskLlr::new(&labels).ok()?;
        let bits = sym * usize::from(sf);
        let mut prior = vec![0f32; bits];
        prior[p.n..].fill(FILL_PRIOR);
        let mut llr = vec![0f32; bits];
        let perm = permutation(p.n);
        let dec = &mut self.decoders[usize::from(p.id)];
        let mut cw_llr = vec![0i8; p.n];
        let mut out = vec![0u8; p.n];
        let mut post = vec![0i16; p.n];
        for _ in 0..CHIRP_ROUNDS {
            fsk.soft_bits(&metrics, &prior, &mut llr).ok()?;
            for (i, &pi) in perm.iter().enumerate() {
                cw_llr[usize::from(pi)] = quantise(llr[i]);
            }
            if dec.decode(&cw_llr, CHIRP_ITERS, &mut out).is_ok() {
                let info = out[..p.k].to_vec();
                let v: Vec<usize> = phy::values(p, &super::interleave(&super::encode(p, &info))).into_iter().map(usize::from).collect();
                let nb = f64::from(noise);
                let snr: f64 = parts.iter().map(|e| ((sent_energy(e, m, &v) - nb) / (m as f64 * nb)).max(0.0)).sum();
                return Some((info, snr.max(1e-4)));
            }
            dec.posterior(&mut post).ok()?;
            for (i, &pi) in perm.iter().enumerate() {
                let j = usize::from(pi);
                prior[i] = ((f32::from(post[j]) - f32::from(cw_llr[j])) / LLR_SCALE).clamp(-25.0, 25.0);
            }
        }
        None
    }

    /// Decodes block `b` of an OFDM burst `y` (section 11.3): its information bits and the SNR, the mean channel power over
    /// the noise of a cell (the transform keeps the noise power).
    fn ofdm_block(&mut self, p: &Profile, l: &Layout, y: &[Complex32], b: usize) -> Option<(Vec<u8>, f64)> {
        let Modulation::Ofdm { bits: per, symbols } = p.modulation else {
            return None;
        };
        let seg = l.segments.iter().find(|s| s.block == b)?;
        let grid = phy::ofdm_layout(symbols);
        let mut fft = Fft::forward(CARRIERS);
        let g = 1.0 / (CARRIERS as f32).sqrt();
        // The cells of every symbol, carrier i at index i.
        let mut cells = vec![Complex32::new(0.0, 0.0); symbols * CARRIERS];
        let mut buf = vec![Complex32::new(0.0, 0.0); CARRIERS];
        for s in 0..symbols {
            let at = seg.start + s * OFDM_SYMBOL + 9;
            buf.copy_from_slice(y.get(at..at + CARRIERS)?);
            fft.process(&mut buf);
            for i in 0..CARRIERS {
                cells[s * CARRIERS + i] = buf[(i + CARRIERS - 24) % CARRIERS] * g;
            }
        }
        // Known cells and their weights: the pilots, weight 1.
        let mut known = vec![(Complex32::new(0.0, 0.0), 0f32); symbols * CARRIERS];
        for s in 0..symbols {
            for pl in grid.pilots(s as u64) {
                known[s * CARRIERS + pl.index as usize] = (pl.value, 1.0);
            }
        }
        let mut h = channel(&cells, &known, symbols, 3);
        let noise = residual_noise(&cells, &known, &h).max(1e-6);
        let perm = permutation(p.n);
        let dec = &mut self.decoders[usize::from(p.id)];
        let mut llr = vec![0f32; p.n];
        let mut cw_llr = vec![0i8; p.n];
        let mut out = vec![0u8; p.n];
        let mut post = vec![0i16; p.n];
        for round in 0..OFDM_ROUNDS {
            let mut at = 0;
            for s in 0..symbols {
                for &d in grid.data(s as u64) {
                    let i = s * CARRIERS + d as usize;
                    let z = h[i].conj() * cells[i];
                    if per == 1 {
                        llr[at] = 4.0 * z.re / noise;
                    } else {
                        let c = 2.0 * std::f32::consts::SQRT_2 / noise;
                        llr[at] = c * z.re;
                        llr[at + 1] = c * z.im;
                    }
                    at += per;
                }
            }
            for (i, &pi) in perm.iter().enumerate() {
                cw_llr[usize::from(pi)] = quantise(llr[i]);
            }
            if dec.decode(&cw_llr, OFDM_ITERS, &mut out).is_ok() {
                let gain = h.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / h.len() as f64;
                return Some((out[..p.k].to_vec(), gain / f64::from(noise)));
            }
            if round + 1 == OFDM_ROUNDS {
                break;
            }
            // Every carrier as a soft pilot, weighted by the reliability of its decision.
            dec.posterior(&mut post).ok()?;
            let mut at = 0;
            for s in 0..symbols {
                for &d in grid.data(s as u64) {
                    let soft: Vec<f32> = (0..per).map(|k| (f32::from(post[usize::from(perm[at + k])]) / LLR_SCALE / 2.0).tanh()).collect();
                    let w = soft.iter().map(|t| t * t).sum::<f32>() / per as f32;
                    let x = if per == 1 { Complex32::new(soft[0].signum(), 0.0) } else { Complex32::new(soft[0].signum(), soft[1].signum()) * std::f32::consts::FRAC_1_SQRT_2 };
                    known[s * CARRIERS + d as usize] = (x, w);
                    at += per;
                }
            }
            h = channel(&cells, &known, symbols, 4);
        }
        None
    }
}

/// True when the `K` bits of a block of profile `p` check: the block CRC or the acknowledgement CRC, or for a voice block
/// the CRC of its side channel. A code word decoded from silence is all zeros and fails each.
pub fn valid(p: &Profile, bits: &[u8]) -> bool {
    if frame::parse_block(bits).is_some() {
        return true;
    }
    p.slot_s.is_none() && frame::VoiceSide::parse(&frame::split_voice(p, bits).1).is_some()
}

/// The start (absolute sample) and the carrier offset (Hz) of the units of a burst, the offsets at the middle of the units
/// that stand out (relative sample, Hz), and the power of the unit sums at delays of -[`PATH_SPAN`] to +[`PATH_SPAN`] samples.
type Units = (f64, f64, Vec<(f64, f64)>, Vec<f64>);

/// One path of a chirp block: its delay from the start of the lattice in samples, and the bin energies of every symbol
/// read at that delay.
#[derive(Clone, Debug)]
struct ChirpPath {
    delay: f64,
    energy: Vec<f32>,
}

/// The delay of the power centroid of `lattice` (delays of -[`PATH_SPAN`] to +[`PATH_SPAN`] samples from the strongest path)
/// when a second path shows: a local peak above `least`, more than one sample from the strongest. Else 0. The window of the
/// OFDM symbols then lies between the paths.
fn centroid(lattice: &[f64], least: f64) -> f64 {
    let delay = |k: usize| k as f64 - PATH_SPAN as f64;
    let second = (0..lattice.len()).any(|k| {
        let v = lattice[k];
        delay(k).abs() > 1.0 && v > least && (k == 0 || v >= lattice[k - 1]) && (k + 1 == lattice.len() || v >= lattice[k + 1])
    });
    let w: f64 = lattice.iter().map(|v| v.max(0.0)).sum();
    if second && w > 0.0 {
        lattice.iter().enumerate().map(|(k, v)| v.max(0.0) * delay(k)).sum::<f64>() / w
    } else {
        0.0
    }
}

/// The rising peak near `p` of `a` and the falling peak near `q` of `b` that one path gives. With several paths the largest
/// rising and the largest falling value can come from different paths, which reads as a carrier offset. The lag at which
/// the two sums agree best (their product over +-[`PATH_SPAN`] samples, the noise mean `floor` taken off) lines them up, and
/// the delay with the largest sum of both is the path.
fn same_path(a: &[f32], b: &[f32], p: i64, q: i64, floor: f32) -> Option<(f64, f64)> {
    let get = |v: &[f32], i: i64| usize::try_from(i).ok().and_then(|i| v.get(i)).map_or(0.0, |&e| f64::from(e - floor));
    let agree = |lag: i64| (-PATH_SPAN..=PATH_SPAN).map(|d| get(a, p + d) * get(b, q + lag + d)).sum::<f64>();
    let lag = (-PATH_SPAN..=PATH_SPAN).map(|j| (j, agree(j))).max_by(|x, y| x.1.total_cmp(&y.1))?.0;
    let d = (-PATH_SPAN..=PATH_SPAN).map(|d| (d, get(a, p + d) + get(b, q + lag + d))).max_by(|x, y| x.1.total_cmp(&y.1))?.0;
    Some((peak_near(a, p + d, 1)?, peak_near(b, q + lag + d, 1)?))
}

/// The value of each data symbol whose bin holds the most energy, and the sum of those energies: the bin energies `parts`
/// (`m` a symbol sent) added over the parts and over the repeats of the `sym` data symbols.
fn strongest(parts: &[&[f32]], m: usize, sym: usize) -> (Vec<usize>, f64) {
    let mut acc = vec![0f32; sym * m];
    for e in parts {
        for (i, s) in e.chunks_exact(m).enumerate() {
            for (o, &v) in acc[(i % sym) * m..(i % sym + 1) * m].iter_mut().zip(s) {
                *o += v;
            }
        }
    }
    let mut total = 0f64;
    let best = acc
        .chunks_exact(m)
        .map(|s| {
            let (v, e) = s.iter().enumerate().fold((0, f32::MIN), |b, (i, &e)| if e > b.1 { (i, e) } else { b });
            total += f64::from(e);
            v
        })
        .collect();
    (best, total)
}

/// The mean energy of the bins `values` (one a data symbol) over every symbol sent of `energy`.
fn sent_energy(energy: &[f32], m: usize, values: &[usize]) -> f64 {
    let n = (energy.len() / m).max(1);
    energy.chunks_exact(m).enumerate().map(|(i, s)| f64::from(s[values[i % values.len()] % m])).sum::<f64>() / n as f64
}

/// The noise of a bin: the mean over the symbols of the median bin energy, over ln 2.
fn noise_per_bin(energy: &[f32], m: usize) -> f32 {
    let mut tmp = vec![0f32; m];
    let mut med = 0f64;
    for s in energy.chunks_exact(m) {
        tmp.copy_from_slice(s);
        let (_, v, _) = tmp.select_nth_unstable_by(m / 2, f32::total_cmp);
        med += f64::from(*v);
    }
    (med / (energy.len() / m).max(1) as f64 / std::f64::consts::LN_2).max(1e-30) as f32
}

/// The distance in chips, 2 to `reach`, of a second path from the path that `energy` was read at. A path `d` chips later
/// puts its energy `d` bins lower in a rising symbol and higher in a falling one (`weave` true); where it is the stronger
/// one, the first path lies as far on the other side of the strongest bin, so both sides are added. `None` when no distance
/// holds a tenth of the energy of the strongest bin and stands out of the noise.
fn echo(energy: &[f32], m: usize, sym: usize, weave: &[bool], noise: f32, reach: usize) -> Option<usize> {
    let (best, _) = strongest(&[energy], m, sym);
    let n = (energy.len() / m).max(1);
    let mean_at = |d: i64| {
        energy.chunks_exact(m).enumerate().map(|(i, s)| {
            let step = if weave[i] { d } else { -d };
            f64::from(s[(best[i % sym] as i64 + step).rem_euclid(m as i64) as usize])
        }).sum::<f64>() / n as f64
    };
    let nb = f64::from(noise);
    let floor = (6.0 * nb / (n as f64).sqrt()).max(0.1 * (mean_at(0) - nb));
    (2..=reach as i64).map(|g| (g as usize, mean_at(g) + mean_at(-g) - 2.0 * nb)).filter(|x| x.1 > floor).max_by(|x, y| x.1.total_cmp(&y.1)).map(|x| x.0)
}

/// What a try did.
enum Step {
    Wait(usize),
    Fail,
    Done(Burst),
}

/// The inverse of the Gray code: `B` with `B ^ (B >> 1) = v`.
pub fn gray_inverse(v: u16) -> u16 {
    let mut b = v;
    let mut s = v >> 1;
    while s != 0 {
        b ^= s;
        s >>= 1;
    }
    b
}

/// A natural LLR (positive means 0) in the decoder's units, limited to +-30 first.
fn quantise(l: f32) -> i8 {
    if l.is_nan() {
        return 0;
    }
    (l.clamp(-30.0, 30.0) * LLR_SCALE).round() as i8
}

/// The carrier offset at relative sample `pos` of a line through `points` (sample, Hz), constant outside them.
fn carrier_at(points: &[(f64, f64)], pos: f64) -> f64 {
    match points {
        [] => 0.0,
        [p] => p.1,
        _ => {
            let (first, last) = (points[0], points[points.len() - 1]);
            if pos <= first.0 {
                return first.1;
            }
            if pos >= last.0 {
                return last.1;
            }
            let i = points.windows(2).position(|w| pos <= w[1].0).unwrap_or(0);
            let (a, b) = (points[i], points[i + 1]);
            a.1 + (b.1 - a.1) * (pos - a.0) / (b.0 - a.0).max(1e-9)
        }
    }
}

/// The noise floor of `x`: the 15 % point of the power of every 7th sample, over `-ln 0.85`.
fn floor_of(x: &[Complex32]) -> f32 {
    let mut p: Vec<f32> = x.iter().step_by(7).map(|v| v.norm_sqr()).collect();
    if p.is_empty() {
        return 0.0;
    }
    let k = p.len() * 15 / 100;
    let (_, v, _) = p.select_nth_unstable_by(k, f32::total_cmp);
    (f64::from(*v) / NOISE_QUANTILE) as f32
}

/// Delay taps of the channel fit across the carriers: -1 to 10 samples.
const TAPS: usize = 12;

/// The channel of every cell of a block from the known cells `known` (value and weight, weight 0 for none): per symbol a
/// weighted least-squares fit of 12 delay taps (-1 to 10 samples) to `cells / value`, the taps smoothed over time with the
/// triangle weights 1, 2, ... `half + 1` ... 2, 1, and the channel of every carrier from them. With soft pilots (`half`
/// 4) a tap weaker than 3 times the median tap power is removed unless a neighbour is kept.
fn channel(cells: &[Complex32], known: &[(Complex32, f32)], symbols: usize, half: usize) -> Vec<Complex32> {
    // The phase of tap d on carrier i: exp(-j 2 pi (i - 24) d / 48).
    let e = |i: usize, d: usize| {
        let a = -TAU * (i as f64 - 24.0) * (d as f64 - 1.0) / CARRIERS as f64;
        Complex32::new(a.cos() as f32, a.sin() as f32)
    };
    let mut taps = vec![[Complex32::new(0.0, 0.0); TAPS]; symbols];
    for (s, t) in taps.iter_mut().enumerate() {
        // Normal equations as a real system of 2 TAPS unknowns: [Re A, -Im A; Im A, Re A].
        let mut ac = vec![Complex32::new(0.0, 0.0); TAPS * TAPS];
        let mut bc = [Complex32::new(0.0, 0.0); TAPS];
        let mut any = false;
        for i in 0..CARRIERS {
            let (x, w) = known[s * CARRIERS + i];
            if w <= 0.0 || x.norm_sqr() == 0.0 {
                continue;
            }
            any = true;
            let hcell = cells[s * CARRIERS + i] / x;
            let w = w * x.norm_sqr();
            for r in 0..TAPS {
                let er = e(i, r).conj();
                bc[r] += er * hcell * w;
                for c in 0..TAPS {
                    ac[r * TAPS + c] += er * e(i, c) * w;
                }
            }
        }
        if !any {
            continue;
        }
        let n = 2 * TAPS;
        let mut a = vec![0f64; n * n];
        let mut bv = vec![0f64; n];
        let ridge = 1e-6 * (0..TAPS).map(|r| f64::from(ac[r * TAPS + r].re)).sum::<f64>().max(1e-12);
        for r in 0..TAPS {
            for c in 0..TAPS {
                let v = ac[r * TAPS + c];
                a[r * n + c] = f64::from(v.re);
                a[r * n + TAPS + c] = -f64::from(v.im);
                a[(TAPS + r) * n + c] = f64::from(v.im);
                a[(TAPS + r) * n + TAPS + c] = f64::from(v.re);
            }
            a[r * n + r] += ridge;
            a[(TAPS + r) * n + TAPS + r] += ridge;
            bv[r] = f64::from(bc[r].re);
            bv[TAPS + r] = f64::from(bc[r].im);
        }
        if cholesky_solve(&mut a, &mut bv, n).is_ok() {
            for d in 0..TAPS {
                t[d] = Complex32::new(bv[d] as f32, bv[TAPS + d] as f32);
            }
        }
    }
    // Smoothing over time.
    let smooth: Vec<[Complex32; TAPS]> = (0..symbols)
        .map(|s| {
            let mut acc = [Complex32::new(0.0, 0.0); TAPS];
            let mut wsum = 0f32;
            for d in -(half as i64)..=half as i64 {
                let k = s as i64 + d;
                if k < 0 || k >= symbols as i64 {
                    continue;
                }
                let w = (half as i64 + 1 - d.abs()) as f32;
                wsum += w;
                for (o, v) in acc.iter_mut().zip(&taps[k as usize]) {
                    *o += v * w;
                }
            }
            acc.map(|v| v / wsum.max(1e-12))
        })
        .collect();
    let mut h = vec![Complex32::new(0.0, 0.0); symbols * CARRIERS];
    for (s, t) in smooth.iter().enumerate() {
        let mut t = *t;
        if half >= 4 {
            let mut pw: Vec<f32> = t.iter().map(|v| v.norm_sqr()).collect();
            let mut sorted = pw.clone();
            sorted.sort_unstable_by(f32::total_cmp);
            let floor = 3.0 * sorted[TAPS / 2];
            let strong: Vec<bool> = pw.iter().map(|&v| v >= floor).collect();
            for d in 0..TAPS {
                let near = strong[d] || (d > 0 && strong[d - 1]) || (d + 1 < TAPS && strong[d + 1]);
                if !near {
                    t[d] = Complex32::new(0.0, 0.0);
                    pw[d] = 0.0;
                }
            }
        }
        for i in 0..CARRIERS {
            h[s * CARRIERS + i] = (0..TAPS).map(|d| t[d] * e(i, d)).sum();
        }
    }
    h
}

/// The noise variance of a cell: the mean of `|cell - h x|^2` over the known cells, over the share that the fit leaves.
fn residual_noise(cells: &[Complex32], known: &[(Complex32, f32)], h: &[Complex32]) -> f32 {
    let (mut s, mut n) = (0f64, 0usize);
    for ((c, k), hh) in cells.iter().zip(known).zip(h) {
        if k.1 > 0.0 {
            s += f64::from((c - hh * k.0).norm_sqr());
            n += 1;
        }
    }
    // The 12-tap fit of 12 pilots leaves only the part the time smoothing (weights 1 to 4 to 1) takes out: 1 - 2 (4 / 16) +
    // 44 / 256 of the noise.
    (s / n.max(1) as f64 / 0.672) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_and_carrier_line() {
        for b in 0..1024u16 {
            assert_eq!(gray_inverse(b ^ (b >> 1)), b);
        }
        let pts = [(0.0, 10.0), (100.0, 20.0)];
        assert_eq!(carrier_at(&pts, -5.0), 10.0);
        assert_eq!(carrier_at(&pts, 50.0), 15.0);
        assert_eq!(carrier_at(&pts, 500.0), 20.0);
        assert_eq!(carrier_at(&[(3.0, 7.0)], 0.0), 7.0);
        assert_eq!(threshold(1), 16.0);
    }
}
