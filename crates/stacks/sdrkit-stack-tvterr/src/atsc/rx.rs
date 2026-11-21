//! The ATSC 1.0 receiver's signal processing, as plain types the components wrap: where the pilot is,
//! the front end (matched filter, resampling to one complex sample per symbol), the pilot loop (pilot to 0 Hz), the symbol
//! clock with segment sync (at two samples per symbol, on the strongest path at its own carrier phase), the field sync, the
//! per-field equaliser (feed-forward and decision-feedback, with long echo cancellers, retries and in-field tracking), and the
//! queue of fields for an equaliser that runs beside the receiver's thread.

use super::fec::{
    field_sync_known, FecState, FieldDecoder, TrellisSlicer, DATA_SEGS, SEG, SEG_DATA, SEG_SYNC,
};
use crate::band::resampler;
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::{Interpolator, Polyphase};
use sdrkit_algo_dsp::linalg::cholesky_solve;
use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::DspError;
use sdrkit_algo_modem::equalizer::{
    earliest_path, latest_path, DfeTracking, EchoCanceller, EchoFitRule, EchoSide, RealDfe,
};
use sdrkit_core::Complex32;
use sdrkit_graph::SlicePool;
use std::sync::Arc;

/// The symbol rate, 4.5 MHz / 286 × 684.
pub const FSYM: f64 = 4.5e6 / 286.0 * 684.0;
/// The pilot's place relative to the channel centre.
pub const PILOT_HZ: f64 = -FSYM / 4.0;
/// Excess bandwidth of the VSB edges.
pub const ROLLOFF: f64 = 0.1152;

/// The VSB pulse at one sample per symbol: a root-raised cosine of half the symbol rate (period 2 samples), 16 periods long,
/// unit energy.
pub fn vsb_pulse() -> Vec<f32> {
    vsb_edge_taps(2.0, 16.0)
}

/// Root-raised-cosine taps of the VSB edges ([`ROLLOFF`]) with a period of `sps` samples over `span` periods, unit energy.
fn vsb_edge_taps(sps: f64, span: f64) -> Vec<f32> {
    pulse::taps(Shape::Rrc { beta: ROLLOFF }, sps, span, Norm::Energy).unwrap_or_else(|_| vec![1.0])
}

/// Where the pilot is (stage 0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotFound {
    /// The shift that puts the pilot at its nominal place (Hz, applied after the mirror).
    pub shift_hz: f64,
    /// I and Q are swapped (the pilot was at the upper edge).
    pub mirrored: bool,
    /// The pilot line over the data just above it, dB.
    pub line_db: f64,
}

/// Looks for the pilot in an averaged spectrum of the whole band, as it is and mirrored. `p` is in natural
/// order (index n/2 is 0 Hz) with bins `rate / n` wide.
///
/// The pilot is a line at the lower edge of the channel. The symbols are real with the pilot as their mean, so the lines
/// of the segment sync lie in pairs at equal distances on both sides of the pilot. Two paths that oppose each other at the
/// pilot leave it weaker than those lines: a line midway between two others of about one height counts as higher than they.
pub fn find_pilot(p: &[f64], rate: f64) -> Option<PilotFound> {
    let n = p.len();
    let bw = rate / n as f64;
    let mut best: Option<(f64, PilotFound)> = None;
    for mirrored in [false, true] {
        // Mirrored: the spectrum is read backwards (a conjugate swaps the sides).
        let at = |i: usize| if mirrored { p[n - 1 - i] } else { p[i] };
        let span_lo = (0.1e6 / bw).ceil() as usize;
        let span_hi = (5.45e6 / bw).ceil() as usize;
        // The lines found: frequency and height over the bins next to them, dB.
        let mut lines = [(0f64, 0f64); 24];
        let mut count = 0;
        for i in 10 + span_lo..n.saturating_sub(span_hi + 1) {
            let f = (i as f64 - (n / 2) as f64) * bw;
            if (i as i64 - (n / 2) as i64).abs() <= 3 {
                continue;
            }
            let v = at(i);
            if v < at(i - 1) || v < at(i + 1) {
                continue;
            }
            // A line stands over the bins next to it on both sides; a ripple of the spectrum from two paths is as wide
            // as many bins.
            let above: f64 = (i + 3..i + 10).map(at).sum::<f64>() / 7.0;
            let under: f64 = (i - 9..i - 2).map(at).sum::<f64>() / 7.0;
            if v < 3.0 * above.max(under) {
                continue;
            }
            // The data just above against what lies just below the channel (VSB shape). Outside the band counts as empty.
            let (b0, b1) = ((0.35e6 / bw) as usize, (0.6e6 / bw) as usize);
            let data: f64 = (i + b0..i + b1).map(at).sum::<f64>() / (b1 - b0) as f64;
            let below = if i > b1 {
                (i - b1..i - b0).map(at).sum::<f64>() / (b1 - b0) as f64
            } else {
                0.0
            };
            if data < 2.0 * below {
                continue;
            }
            // Parabolic refinement of the line.
            let (a, b, c) = (at(i - 1).sqrt(), v.sqrt(), at(i + 1).sqrt());
            let d = 0.5 * (a - c) / (a - 2.0 * b + c).min(-1e-30);
            if count < lines.len() {
                lines[count] = (f + d * bw, 10.0 * (v / above).log10());
                count += 1;
            }
        }
        let lines = &lines[..count];
        for &(f, own) in lines {
            // Two lines of about one height (3 dB) at one distance under and over this one, within the vestige of the
            // channel.
            let pair = lines
                .iter()
                .filter(|l| l.0 < f - 4.0 * bw && l.0 > f - 0.31e6)
                .flat_map(|lo| {
                    lines
                        .iter()
                        .filter(move |hi| {
                            (hi.0 - f - (f - lo.0)).abs() < 1.5 * bw && (hi.1 - lo.1).abs() < 3.0
                        })
                        .map(move |hi| lo.1.max(hi.1))
                })
                .fold(f64::NEG_INFINITY, f64::max);
            let line_db = own.max(pair + 0.1);
            if best.as_ref().is_none_or(|(s, _)| line_db > *s) {
                best = Some((
                    line_db,
                    PilotFound {
                        shift_hz: PILOT_HZ - f,
                        mirrored,
                        line_db: own,
                    },
                ));
            }
        }
    }
    best.map(|b| b.1)
}

/// The front end: the matched VSB filter at the input rate, then resampling to one sample per symbol ([`FSYM`]). The channel
/// stays centred on 0 Hz.
pub struct Front {
    mf: Fir<Complex32>,
    rs: FractionalResampler<Complex32>,
    tmp: Vec<Complex32>,
}

impl Front {
    /// For input at `rate`.
    ///
    /// # Errors
    ///
    /// [`DspError`] if no resampler serves `rate`.
    pub fn new(rate: f64) -> Result<Front, DspError> {
        // The root-raised cosine of half the symbol rate, sampled at the input rate.
        let taps = vsb_edge_taps(rate / (FSYM / 2.0), 32.0);
        // The matched filter leaves ±3.0 MHz: flat to there, cut where the first image or alias of that band begins.
        let rs = resampler(rate, FSYM, 3.0e6, rate.min(FSYM) - 3.0e6)?;
        Ok(Front {
            mf: Fir::new(&taps),
            rs,
            tmp: Vec::new(),
        })
    }
    /// Appends the processed samples.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.tmp.resize(x.len(), Complex32::new(0.0, 0.0));
        self.mf.process(x, &mut self.tmp);
        self.rs.process(&self.tmp, out);
    }
}

/// What the pilot loop reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotLoopState {
    /// Locked on the pilot.
    pub locked: bool,
    /// The loop's frequency, Hz.
    pub freq_hz: f64,
    /// Pilot level over the data level, dB.
    pub pilot_db: f64,
}

/// Samples per update of the pilot loop.
const PILOT_UPDATE: usize = 16;
/// The noise bandwidth of the pilot loop while it acquires, per update.
const PILOT_BN_WIDE: f64 = 1.95e-4 * PILOT_UPDATE as f64;
/// The noise bandwidth once the pilot has been steady for about 0.1 s.
const PILOT_BN_NARROW: f64 = 8.35e-5 * PILOT_UPDATE as f64;
/// The damping of the pilot loop.
const PILOT_ZETA: f64 = 1.5;
/// The step of the pilot's filter per update: a one-pole average over about 128 samples.
const PILOT_AVG: f32 = PILOT_UPDATE as f32 / 128.0;
/// The step of the power average per update: about 4096 samples.
const POWER_AVG: f32 = PILOT_UPDATE as f32 / 4096.0;

/// The step of the mean size of the pilot per update: about 4096 samples.
const LEVEL_AVG: f32 = PILOT_UPDATE as f32 / 4096.0;
/// Half the pilot's share of the power of a signal with one path (1.25 squared over 21): a pilot under it narrows the loop.
const PILOT_SHARE: f32 = 0.5 * 1.5625 / 21.0;

/// The carrier loop on the pilot, with DC removal and gain normalisation. The input is the centred channel at one sample per
/// symbol, and the loop moves its pilot from [`PILOT_HZ`] to 0 Hz. The output is that signal without the pilot. Its real part
/// at the symbol instants is the data, rms about √21 (levels ±1 .. ±7). The loop filter and the oscillator are those of
/// `sdrkit_algo_dsp::loopfilter`.
pub struct PilotLoop {
    lf: LoopFilter,
    nco: PhaseAcc,
    /// Input that waits for the rest of its update.
    held: Vec<Complex32>,
    wide: bool,
    pl: Complex32,
    /// The pilot's average one update back. It steers the oscillator, so an update does not wait for the one before it.
    seen: Complex32,
    pow: f32,
    steady: usize,
    bad: usize,
    narrow: f64,
    /// The mean size of the pilot's average, and the part of the bandwidth in use for a weak pilot.
    level: f32,
    weak: f64,
    /// State.
    pub state: PilotLoopState,
}

impl PilotLoop {
    /// A loop starting at 0 Hz. `narrow` (0.25 .. 1) narrows it for a weak pilot.
    pub fn new(narrow: f64) -> PilotLoop {
        let narrow = narrow.clamp(0.25, 1.0);
        let mut lf = Self::filter(PILOT_BN_WIDE * narrow);
        // The pilot is found within a few kHz: the loop pulls at most about ±60 kHz.
        lf.set_clamp(0.035 / std::f64::consts::TAU);
        PilotLoop {
            lf,
            nco: PhaseAcc::new(0.0),
            held: Vec::with_capacity(PILOT_UPDATE),
            wide: true,
            pl: Complex32::new(0.0, 0.0),
            seen: Complex32::new(0.0, 0.0),
            pow: 21.0,
            steady: 0,
            bad: 0,
            narrow,
            level: 0.0,
            weak: 1.0,
            state: PilotLoopState::default(),
        }
    }

    fn filter(bn_t: f64) -> LoopFilter {
        // The detector outputs the phase error in cycles. The oscillator advances by the loop's output on every one of the
        // update's samples.
        LoopFilter::with_gain(bn_t, PILOT_ZETA, PILOT_UPDATE as f64)
            .expect("the pilot loop's bandwidth and damping are valid")
    }

    /// Runs the loop on whole updates of 16 samples and appends their output. The rest of `x` waits for the next call. The loop's
    /// error and the gain change once per update (the pilot's filter is far slower than that). The oscillator turns every sample.
    pub fn process(&mut self, mut x: &[Complex32], out: &mut Vec<Complex32>) {
        out.reserve(x.len() + PILOT_UPDATE);
        let mut o = [Complex32::new(0.0, 0.0); PILOT_UPDATE];
        if !self.held.is_empty() {
            let n = (PILOT_UPDATE - self.held.len()).min(x.len());
            self.held.extend_from_slice(&x[..n]);
            x = &x[n..];
            if self.held.len() < PILOT_UPDATE {
                return;
            }
            let mut upd = [Complex32::new(0.0, 0.0); PILOT_UPDATE];
            upd.copy_from_slice(&self.held);
            self.held.clear();
            self.update(&upd, &mut o);
            out.extend_from_slice(&o);
        }
        let (whole, rest) = x.as_chunks::<PILOT_UPDATE>();
        for upd in whole {
            self.update(upd, &mut o);
            out.extend_from_slice(&o);
        }
        self.held.extend_from_slice(rest);
        self.state.locked = self.steady >= (0.011 * FSYM) as usize;
        self.state.freq_hz = self.lf.integrator() * FSYM;
        self.state.pilot_db =
            10.0 * (self.pl.norm_sqr() as f64 / self.pow.max(1e-20) as f64).log10();
    }

    /// One update: steers the oscillator, turns `x` so that the pilot is at 0 Hz, and writes it without the pilot.
    #[inline]
    fn update(&mut self, x: &[Complex32; PILOT_UPDATE], out: &mut [Complex32; PILOT_UPDATE]) {
        // The error against the mean size of the pilot, so that noise larger than a weak pilot does not bend the detector.
        self.level += (self.seen.norm_sqr().sqrt() - self.level) * LEVEL_AVG;
        let err = f64::from((self.seen.im / self.level.max(1e-12)).clamp(-1.0, 1.0))
            / std::f64::consts::TAU;
        let wide = self.steady < (0.1 * FSYM) as usize;
        // A pilot under its usual share of the power (paths that oppose each other at the pilot) narrows the loop by as much,
        // which keeps the phase noise of the loop.
        let share = self.level * self.level / (PILOT_SHARE * self.pow.max(1e-20));
        let weak = f64::from(share.clamp(1.0 / 32.0, 1.0));
        if wide != self.wide || (weak / self.weak - 1.0).abs() > 0.25 {
            self.wide = wide;
            self.weak = weak;
            let bn = if wide { PILOT_BN_WIDE } else { PILOT_BN_NARROW };
            self.lf
                .set_bandwidth(bn * self.narrow * weak, PILOT_ZETA, PILOT_UPDATE as f64)
                .expect("the pilot loop's bandwidth and damping are valid");
        }
        let freq = self.lf.update(err);
        self.nco.set_freq(freq);
        // The oscillator's conjugate at the first sample of the update, and its turn from one sample to the next.
        let q = self.nco.phasor().conj();
        let (ws, wc) = approx::sin_cos(-std::f64::consts::TAU * freq);
        let w = Complex32::new(wc, ws);
        self.nco.steer(freq * PILOT_UPDATE as f64);
        // What turns sample i: the oscillator there, and the quarter of the symbol rate up (j^i). The first four, then
        // those times the turn of 4 and of 8 samples.
        let w2 = w * w;
        let w4 = w2 * w2;
        let w8 = w4 * w4;
        let first = [q, q * w, q * w2, q * w2 * w];
        let mut tr = [0f32; PILOT_UPDATE];
        let mut ti = [0f32; PILOT_UPDATE];
        (tr[0], ti[0]) = (first[0].re, first[0].im);
        (tr[1], ti[1]) = (-first[1].im, first[1].re);
        (tr[2], ti[2]) = (-first[2].re, -first[2].im);
        (tr[3], ti[3]) = (first[3].im, -first[3].re);
        for i in 0..4 {
            tr[4 + i] = tr[i] * w4.re - ti[i] * w4.im;
            ti[4 + i] = tr[i] * w4.im + ti[i] * w4.re;
        }
        for i in 0..8 {
            tr[8 + i] = tr[i] * w8.re - ti[i] * w8.im;
            ti[8 + i] = tr[i] * w8.im + ti[i] * w8.re;
        }
        let g = (21.0 / self.pow.max(1e-20)).sqrt();
        let dc = self.pl.re;
        let xr: [f32; PILOT_UPDATE] = std::array::from_fn(|i| x[i].re);
        let xi: [f32; PILOT_UPDATE] = std::array::from_fn(|i| x[i].im);
        let mut zr = [0f32; PILOT_UPDATE];
        let mut zi = [0f32; PILOT_UPDATE];
        for i in 0..PILOT_UPDATE {
            zr[i] = xr[i] * tr[i] - xi[i] * ti[i];
            zi[i] = xr[i] * ti[i] + xi[i] * tr[i];
        }
        // The pilot's sums and the power, in four lanes.
        let mut acc_re = [0f32; 4];
        let mut acc_im = [0f32; 4];
        let mut p2 = [0f32; 4];
        for (zr, zi) in zr.as_chunks::<4>().0.iter().zip(zi.as_chunks::<4>().0) {
            for l in 0..4 {
                acc_re[l] += zr[l];
                acc_im[l] += zi[l];
                p2[l] += (zr[l] - dc) * (zr[l] - dc);
            }
        }
        for i in 0..PILOT_UPDATE {
            zr[i] = (zr[i] - dc) * g;
            zi[i] *= g;
        }
        for i in 0..PILOT_UPDATE {
            out[i] = Complex32::new(zr[i], zi[i]);
        }
        let n = PILOT_UPDATE as f32;
        let acc = Complex32::new(
            (acc_re[0] + acc_re[1]) + (acc_re[2] + acc_re[3]),
            (acc_im[0] + acc_im[1]) + (acc_im[2] + acc_im[3]),
        );
        let p2 = (p2[0] + p2[1]) + (p2[2] + p2[3]);
        self.seen = self.pl;
        self.pl += (acc / n - self.pl) * PILOT_AVG;
        self.pow += (p2 / n - self.pow) * POWER_AVG;
        let good = err.abs() < 0.25 / std::f64::consts::TAU
            && self.pl.re > 0.0
            && self.pl.norm_sqr() > 0.002 * self.pow;
        if good {
            self.steady += PILOT_UPDATE;
            self.bad = 0;
        } else {
            self.bad += PILOT_UPDATE;
            if self.bad > (0.022 * FSYM) as usize {
                self.steady = 0;
            }
        }
    }
}

/// A segment of 832 symbols, the first four the segment sync.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// The symbols (data levels ±1 .. ±7 nominal): the part in phase with the path the clock holds.
    pub syms: Arc<[f32]>,
    /// The part in quadrature at the same instants. A path with another carrier phase is in both parts.
    pub quad: Arc<[f32]>,
    /// The clock moved to another position before this segment: it does not follow the one before it.
    pub jump: bool,
    /// Signal time of the first symbol, seconds.
    pub time: f64,
}

/// What the symbol clock reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClockState {
    /// Segment sync found.
    pub locked: bool,
    /// The sample clock offset, ppm (positive: the radio's clock is fast, symbols take more samples than nominal).
    pub ppm: f64,
    /// Segment-sync correlation quality, 0 .. 1.
    pub quality: f64,
}

/// The symbol interpolator: a windowed sinc of half length 4. The real signal fills 0.53 of its band.
type SymbolInterp = Polyphase<8, 256>;
const HALF: usize = 4;

/// The filter that reads the centred channel half-way between two samples: the middle row of a windowed sinc of half length 8.
type MidInterp = Polyphase<16, 256>;
/// Samples from the newest input of that filter to the one just before its output.
const MID_DELAY: usize = 8;

/// The clock's watch for a stronger path: segments in its mean, segments after the lock that all go into the mean (then
/// every fourth), segments between two looks, and how much stronger than the path held another must be (0.25 dB) on two
/// looks in a row.
const WATCH_SEGS: u32 = 4096;
const WATCH_ALL: u64 = 1024;
const WATCH_EVERY: u64 = 256;
const WATCH_RATIO: f32 = 1.03;

/// Looks for the field sync in a segment of complex symbols `(in phase, quadrature)`: the correlation with its PN511 at -2 to
/// +2 symbols. A path with carrier phase `p` whose sync starts `d` symbols after the segment gives a correlation of largest
/// size at `d` and of angle `p + (l - d) x 90 degrees` at lag `l`, because the spectrum of the signal is centred a quarter of
/// the symbol rate above the pilot. Returns `d` and `p` (radians), or `None` when no lag reaches 0.3 of a full correlation.
fn field_sync_place(seg: &[(f32, f32)]) -> Option<(f64, f64)> {
    let known = field_sync_known(false);
    let mut c = [(0f64, 0f64); 5];
    for (l, c) in c.iter_mut().enumerate() {
        let (mut re, mut im) = (0f32, 0f32);
        for (z, k) in seg[2 + l..].iter().zip(&known[4..515]) {
            re += z.0 * k;
            im += z.1 * k;
        }
        *c = (f64::from(re), f64::from(im));
    }
    let energy: f64 = seg[4..515]
        .iter()
        .map(|z| f64::from(z.0 * z.0 + z.1 * z.1))
        .sum();
    let mag: [f64; 5] = std::array::from_fn(|l| c[l].0.hypot(c[l].1));
    let best = (0..5).max_by(|&a, &b| mag[a].total_cmp(&mag[b]))?;
    if mag[best] * mag[best] < 0.09 * energy * (511.0 * 25.0) {
        return None;
    }
    let frac = if best > 0 && best < 4 {
        let (a, b, c) = (mag[best - 1], mag[best], mag[best + 1]);
        (0.5 * (a - c) / (a - 2.0 * b + c).min(-1e-9)).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let phase = c[best].1.atan2(c[best].0) + std::f64::consts::FRAC_PI_2 * frac;
    Some((best as f64 - 2.0 + frac, phase))
}

/// Segment sync and the symbol clock. The input is complex at one sample per symbol with the pilot's place at 0 Hz. It is
/// made at two samples per symbol: the samples themselves, and between them the centred channel through a half-sample filter,
/// turned back by a quarter of the symbol rate. The clock correlates that signal with the segment sync: the largest sum over
/// one segment of positions is the strongest path, and its angle is about the carrier phase of that path against the pilot
/// of all paths together. The clock then tracks the position with an early-late loop on the signal turned by that phase,
/// interpolating the symbols with a windowed sinc of half length 4. The loop's gain falls in steps.
///
/// The four sync symbols are a tone at a quarter of the symbol rate, for which a move of one symbol and a quarter turn of
/// the carrier are the same: they do not give the phase. Each field sync does ([`field_sync_place`]), and moves the position
/// and the turn to the path. While it tracks, the clock keeps the mean correlation at every position of a segment and moves
/// to a path that stays stronger than the one it holds, so the strongest path is the one the equaliser gets as its main path.
pub struct SymbolClock {
    /// Input samples so far, modulo 4.
    n: usize,
    mid: Fir<Complex32>,
    centred: Vec<Complex32>,
    /// The last `MID_DELAY` inputs, which wait for the values between them.
    late: Vec<Complex32>,
    /// The signal at two samples per symbol: its real and its imaginary part.
    buf: Vec<f32>,
    bufq: Vec<f32>,
    base: u64,
    interp: SymbolInterp,
    acc: Vec<f64>,
    accq: Vec<f64>,
    mags: Vec<f64>,
    acc_segs: usize,
    /// The turn that makes the path held real (cosine and sine of its phase), and whether a field sync has set it.
    turn: (f32, f32),
    phased: bool,
    /// The mean correlation with the segment sync at every sample of a segment after the position held, the segments in the
    /// mean, and the stronger path seen at the last look.
    watch_r: Vec<f32>,
    watch_i: Vec<f32>,
    watched: u32,
    candidate: Option<usize>,
    /// The position has moved to another path since the last segment.
    jumped: bool,
    // Position (absolute sample index, fractional) of the next segment's first symbol, and samples per symbol.
    pos: f64,
    step: f64,
    tracking: bool,
    weak: usize,
    segs_locked: u64,
    seg: Vec<(f32, f32)>,
    pool: SlicePool<f32>,
    /// State.
    pub state: ClockState,
}

impl Default for SymbolClock {
    fn default() -> Self {
        SymbolClock::new()
    }
}

impl SymbolClock {
    /// A clock searching for segment sync.
    pub fn new() -> SymbolClock {
        let mid = MidInterp::design(1.0, 80.0).unwrap_or_else(|_| unreachable!("a valid design"));
        SymbolClock {
            n: 0,
            // The row is in time order, and the filter takes its taps newest first.
            mid: Fir::new(&mid.row(128).iter().rev().copied().collect::<Vec<f32>>()),
            centred: Vec::new(),
            late: vec![Complex32::new(0.0, 0.0); MID_DELAY],
            buf: Vec::new(),
            bufq: Vec::new(),
            base: 0,
            interp: SymbolInterp::design(1.0, 55.0)
                .unwrap_or_else(|_| unreachable!("a valid design")),
            acc: vec![0.0; 2 * SEG],
            accq: vec![0.0; 2 * SEG],
            mags: Vec::with_capacity(2 * SEG),
            acc_segs: 0,
            turn: (1.0, 0.0),
            phased: false,
            watch_r: vec![0.0; 2 * SEG],
            watch_i: vec![0.0; 2 * SEG],
            watched: 0,
            candidate: None,
            jumped: false,
            pos: 0.0,
            step: 2.0,
            tracking: false,
            weak: 0,
            segs_locked: 0,
            seg: Vec::with_capacity(SEG),
            pool: SlicePool::default(),
            state: ClockState::default(),
        }
    }

    /// Back to searching.
    pub fn reset(&mut self) {
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.accq.iter_mut().for_each(|a| *a = 0.0);
        self.acc_segs = 0;
        self.tracking = false;
        self.step = 2.0;
        self.state = ClockState::default();
    }

    /// The signal at sample time `t`: its real and its imaginary part.
    #[inline]
    fn interp(&self, t: f64) -> (f32, f32) {
        let rel = t - self.base as f64;
        let i = rel.floor();
        let mu = (rel - i) as f32;
        let start = i as isize - (HALF as isize - 1);
        if start < 0 || start as usize + 2 * HALF > self.buf.len() {
            return (0.0, 0.0);
        }
        let w = start as usize..start as usize + 2 * HALF;
        (
            self.interp.at(&self.buf[w.clone()], mu),
            self.interp.at(&self.bufq[w], mu),
        )
    }

    /// The correlation of the signal with the segment sync at sample time `t`.
    #[inline]
    fn sync_corr(&self, t: f64) -> (f64, f64) {
        let (mut re, mut im) = (0f64, 0f64);
        for (i, s) in SEG_SYNC.iter().enumerate() {
            let (a, b) = self.interp(t + i as f64 * self.step);
            re += f64::from(s * a);
            im += f64::from(s * b);
        }
        (re, im)
    }

    /// Adds the segment at the position held to the mean correlation with the segment sync at every sample of a segment.
    /// Every [`WATCH_EVERY`] segments it looks for a path stronger than the one held by [`WATCH_RATIO`]: the second look in a
    /// row that finds the same one returns its distance in samples and its turn.
    fn watch_paths(&mut self) -> Option<(usize, (f32, f32))> {
        let need = 2 * SEG;
        let rel = self.pos - self.base as f64;
        let q = rel.round() as usize;
        // Every segment at first, then every fourth.
        let rest = self.segs_locked >= WATCH_ALL && !(self.segs_locked + 1).is_multiple_of(4);
        if rest || rel < 0.0 || q + need + 8 > self.buf.len() {
            return None;
        }
        self.watched = (self.watched + 1).min(WATCH_SEGS);
        let a = 1.0 / self.watched as f32;
        for (buf, mean) in [
            (&self.buf, &mut self.watch_r),
            (&self.bufq, &mut self.watch_i),
        ] {
            for (m, s) in mean.iter_mut().zip(buf[q..q + need + 6].windows(7)) {
                let c = SEG_SYNC[0] * s[0]
                    + SEG_SYNC[1] * s[2]
                    + SEG_SYNC[2] * s[4]
                    + SEG_SYNC[3] * s[6];
                *m += a * (c - *m);
            }
        }
        if !(self.segs_locked + 1).is_multiple_of(WATCH_EVERY) {
            return None;
        }
        let mag = |k: usize| self.watch_r[k].hypot(self.watch_i[k]);
        // The path held is within 4 symbols of the position.
        let held = (0..8).chain(need - 8..need).map(mag).fold(0f32, f32::max);
        let (k, other) = (16..need - 16)
            .map(|k| (k, mag(k)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        if other <= WATCH_RATIO * held {
            self.candidate = None;
            return None;
        }
        if self.candidate.is_none_or(|c| c.abs_diff(k) > 2) {
            self.candidate = Some(k);
            return None;
        }
        let turn = (self.watch_r[k] / other, self.watch_i[k] / other);
        self.candidate = None;
        self.watch_r.fill(0.0);
        self.watch_i.fill(0.0);
        self.watched = 0;
        Some((k, turn))
    }

    /// Appends `x` at two samples per symbol to the buffers, `MID_DELAY` input samples late.
    fn expand(&mut self, x: &[Complex32]) {
        // The centred channel: input sample n turned by (-j)^n.
        const DOWN: [Complex32; 4] = [
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, -1.0),
            Complex32::new(-1.0, 0.0),
            Complex32::new(0.0, 1.0),
        ];
        let n0 = self.n;
        let down: [Complex32; 4] = std::array::from_fn(|l| DOWN[(n0 + l) & 3]);
        self.centred.clear();
        let (fours, rest) = x.as_chunks::<4>();
        for z in fours {
            self.centred.extend_from_slice(&[
                z[0] * down[0],
                z[1] * down[1],
                z[2] * down[2],
                z[3] * down[3],
            ]);
        }
        self.centred
            .extend(rest.iter().zip(&down).map(|(z, d)| z * d));
        self.mid.process_in_place(&mut self.centred);
        self.late.extend_from_slice(x);
        // Half a sample on, the channel goes back up by the quarter of the symbol rate there: j^n and an eighth of a turn.
        let r = std::f32::consts::FRAC_1_SQRT_2;
        let up: [Complex32; 4] = std::array::from_fn(|l| {
            [
                Complex32::new(r, r),
                Complex32::new(-r, r),
                Complex32::new(-r, -r),
                Complex32::new(r, -r),
            ][(n0 + l) & 3]
        });
        let start = self.buf.len();
        self.buf.resize(start + 2 * x.len(), 0.0);
        self.bufq.resize(start + 2 * x.len(), 0.0);
        for (((pairs, quads), on), m) in self.buf[start..]
            .chunks_mut(8)
            .zip(self.bufq[start..].chunks_mut(8))
            .zip(self.late.chunks(4))
            .zip(self.centred.chunks(4))
        {
            let pairs = pairs.as_chunks_mut::<2>().0;
            let quads = quads.as_chunks_mut::<2>().0;
            for ((((pair, quad), on), m), u) in pairs.iter_mut().zip(quads).zip(on).zip(m).zip(&up)
            {
                pair[0] = on.re;
                pair[1] = m.re * u.re - m.im * u.im;
                quad[0] = on.im;
                quad[1] = m.re * u.im + m.im * u.re;
            }
        }
        self.late.drain(..x.len());
        self.n = (n0 + x.len()) & 3;
    }

    /// Feeds complex samples at one per symbol and `rate_hz`. Appends finished segments.
    pub fn process(&mut self, x: &[Complex32], rate_hz: f64, out: &mut Vec<Segment>) {
        self.expand(x);
        let need = 2 * SEG;
        loop {
            if !self.tracking {
                // Accumulate the sync correlation over whole segments, each window then dropped (whole segments, so the
                // accumulator keeps its alignment with the buffer).
                if need + 8 > self.buf.len() {
                    break;
                }
                for (buf, acc) in [(&self.buf, &mut self.acc), (&self.bufq, &mut self.accq)] {
                    for (p, a) in acc.iter_mut().enumerate() {
                        let s = &buf[p..];
                        let c = SEG_SYNC[0] * s[0]
                            + SEG_SYNC[1] * s[2]
                            + SEG_SYNC[2] * s[4]
                            + SEG_SYNC[3] * s[6];
                        *a += c as f64;
                    }
                }
                self.acc_segs += 1;
                self.buf.drain(..need);
                self.bufq.drain(..need);
                self.base += need as u64;
                if self.acc_segs >= 24 {
                    self.mags.clear();
                    self.mags
                        .extend(self.acc.iter().zip(&self.accq).map(|(a, b)| a.hypot(*b)));
                    let (pk, &v) = self
                        .mags
                        .iter()
                        .enumerate()
                        .max_by(|a, b| a.1.total_cmp(b.1))
                        .expect("the accumulator has two segments of entries");
                    let (a, c) = (
                        self.mags[(pk + need - 1) % need],
                        self.mags[(pk + 1) % need],
                    );
                    let found = (self.acc[pk], self.accq[pk]);
                    self.mags.sort_by(|a, b| a.total_cmp(b));
                    let med = self.mags[need / 2];
                    // Expected peak: 4 symbols x 5 x 5 per segment.
                    let expect = 100.0 * self.acc_segs as f64;
                    if v > 6.0 * med && v > 0.4 * expect {
                        let d = 0.5 * (a - c) / (a - 2.0 * v + c).min(-1e-9);
                        let mut p0 = pk as f64 + d.clamp(-0.5, 0.5);
                        if p0 < HALF as f64 + 1.0 {
                            p0 += need as f64;
                        }
                        self.pos = self.base as f64 + p0;
                        self.step = 2.0;
                        self.tracking = true;
                        self.weak = 0;
                        self.segs_locked = 0;
                        self.turn = ((found.0 / v) as f32, (found.1 / v) as f32);
                        self.phased = false;
                        self.jumped = true;
                        self.watch_r.fill(0.0);
                        self.watch_i.fill(0.0);
                        self.watched = 0;
                        self.candidate = None;
                        self.state.quality = (v / expect).min(1.0);
                    } else {
                        self.acc.iter_mut().for_each(|a| *a *= 0.5);
                        self.accq.iter_mut().for_each(|a| *a *= 0.5);
                        self.acc_segs /= 2;
                    }
                }
                continue;
            }
            // Tracking: one segment.
            let end = self.pos + SEG as f64 * self.step + 2.0 * HALF as f64 + 2.0;
            if end > (self.base + self.buf.len() as u64) as f64 {
                break;
            }
            // The segment, turned by the phase of the path held.
            let (tc, ts) = self.turn;
            let mut seg = std::mem::take(&mut self.seg);
            seg.clear();
            seg.extend((0..SEG).map(|i| {
                let (a, b) = self.interp(self.pos + i as f64 * self.step);
                (a * tc + b * ts, b * tc - a * ts)
            }));
            let syms = self.pool.exact(SEG, |v| {
                for (o, s) in v.iter_mut().zip(&seg) {
                    *o = s.0;
                }
            });
            let quad = self.pool.exact(SEG, |v| {
                for (o, s) in v.iter_mut().zip(&seg) {
                    *o = s.1;
                }
            });
            let found = field_sync_place(&seg);
            self.seg = seg;
            // Early-late on the four sync symbols: the vertex of a parabola through the correlation at -0.5, 0, +0.5 samples.
            let turned = |c: (f64, f64)| c.0 * f64::from(tc) + c.1 * f64::from(ts);
            let (cm, c0, cp) = (
                turned(self.sync_corr(self.pos - 0.5)),
                turned(self.sync_corr(self.pos)),
                turned(self.sync_corr(self.pos + 0.5)),
            );
            let den = cm - 2.0 * c0 + cp;
            let vertex = if den < -1e-9 {
                (0.25 * (cm - cp) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let q = (c0 / 100.0).clamp(-1.0, 1.0);
            self.state.quality += 0.05 * (q - self.state.quality);
            // The data beside the sync symbols spread the detector by about 0.3 samples a segment: the gain falls in steps to
            // one that leaves the clock within about 0.01 symbols.
            let (kp, ki) = if self.segs_locked < 300 {
                (0.35, 2.5e-5)
            } else if self.segs_locked < 1000 {
                (0.12, 3e-6)
            } else if self.segs_locked < 2000 {
                (0.05, 6e-7)
            } else {
                (0.006, 8e-9)
            };
            self.step = (self.step + ki * vertex).clamp(2.0 * (1.0 - 400e-6), 2.0 * (1.0 + 400e-6));
            out.push(Segment {
                syms,
                quad,
                time: (0.5 * self.pos - MID_DELAY as f64) / rate_hz,
                jump: std::mem::take(&mut self.jumped),
            });
            let stronger = self.watch_paths();
            self.pos += SEG as f64 * self.step + kp * vertex;
            self.segs_locked += 1;
            if let Some((offset, phase)) = found {
                // The field sync gives the place and the phase of the path: all of a large error at once, a tenth of a
                // small one.
                let part = if self.phased && offset.abs() < 0.25 && phase.abs() < 0.35 {
                    0.1
                } else {
                    1.0
                };
                self.phased = true;
                self.pos += part * offset * self.step;
                let now = f64::from(ts).atan2(f64::from(tc)) + part * phase;
                self.turn = (now.cos() as f32, now.sin() as f32);
            }
            if let Some((samples, turn)) = stronger {
                self.pos += samples as f64;
                self.turn = turn;
                self.phased = false;
                self.jumped = true;
            }
            self.weak = if q < 0.3 { self.weak + 1 } else { 0 };
            // Drop the samples behind the read position.
            let keep = (self.pos.floor() as u64).saturating_sub(self.base + 2 * HALF as u64 + 4);
            if self.weak > 50 || (self.segs_locked > 60 && self.state.quality < 0.3) {
                self.reset();
                let keep = (keep as usize).min(self.buf.len());
                self.buf.drain(..keep);
                self.bufq.drain(..keep);
                self.base += keep as u64;
                continue;
            }
            self.state.locked = true;
            self.state.ppm = (self.step / 2.0 - 1.0) * 1e6;
            if keep as usize > 65536 {
                self.buf.drain(..keep as usize);
                self.bufq.drain(..keep as usize);
                self.base += keep;
            }
        }
    }
}

/// What the field sync reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FieldSyncState {
    /// Fields are being framed.
    pub locked: bool,
    /// The parity of the last field (1 or 2).
    pub parity: u8,
    /// Field syncs expected but not found.
    pub misses: u64,
}

/// Symbols before a field's sync that come with the field, so that a long echo canceller sees the earlier field sync and its
/// recursion settles before the field.
pub const LEAD: usize = 1600;

/// A field: [`LEAD`] symbols before it (zeros where none were received), its field sync segment, the 312 data segments and
/// the next field's sync segment (as look-ahead for the equaliser).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// `LEAD + 314 × 832` symbols; the field sync starts at [`LEAD`].
    pub syms: Arc<[f32]>,
    /// The part in quadrature of the same symbols ([`Segment::quad`]).
    pub quad: Arc<[f32]>,
    /// Field 2 (the middle PN63 inverted).
    pub second: bool,
    /// Signal time of the field sync.
    pub time: f64,
}

/// Counts sign errors of a segment against the field sync of either parity over the PN511 and PN63s. Returns the errors and
/// whether it was the second field.
pub fn field_sync_errors(seg: &[f32]) -> (usize, bool) {
    let k1 = field_sync_known(false);
    let k2 = field_sync_known(true);
    let mut e1 = 0;
    let mut e2 = 0;
    for i in 4..704 {
        let s = seg[i] > 0.0;
        e1 += (s != (k1[i] > 0.0)) as usize;
        e2 += (s != (k2[i] > 0.0)) as usize;
    }
    if e1 <= e2 {
        (e1, false)
    } else {
        (e2, true)
    }
}

/// Frames fields from segments.
#[derive(Default)]
pub struct FieldFramer {
    /// The [`LEAD`] symbols before the field in assembly, then the field: the symbols and their part in quadrature.
    cur: Vec<f32>,
    cur_q: Vec<f32>,
    pool: SlicePool<f32>,
    segs: usize,
    second: bool,
    time: f64,
    in_field: bool,
    missed_in_row: usize,
    /// The newest symbols before the segment being taken (up to [`LEAD`]).
    lead: Vec<f32>,
    lead_q: Vec<f32>,
    /// State.
    pub state: FieldSyncState,
}

impl FieldFramer {
    /// Back to searching.
    pub fn reset(&mut self) {
        self.cur.clear();
        self.cur_q.clear();
        self.lead.clear();
        self.lead_q.clear();
        self.segs = 0;
        self.in_field = false;
        self.state.locked = false;
    }
    /// Takes one segment. Returns a field when one is complete.
    pub fn push(&mut self, seg: &Segment) -> Option<Field> {
        if seg.jump {
            self.reset();
        }
        let (err, second) = field_sync_errors(&seg.syms);
        let is_sync = err < 175;
        if !self.in_field {
            if is_sync {
                self.in_field = true;
                self.start(seg, second);
            } else {
                self.keep(seg);
            }
            return None;
        }
        self.cur.extend_from_slice(&seg.syms);
        self.cur_q.extend_from_slice(&seg.quad);
        self.segs += 1;
        if self.segs == DATA_SEGS + 2 {
            // This segment must be the next field sync.
            let ok = is_sync && second != self.second;
            let (cur, cur_q) = (&self.cur, &self.cur_q);
            let field = Field {
                syms: self.pool.exact(cur.len(), |v| v.copy_from_slice(cur)),
                quad: self.pool.exact(cur_q.len(), |v| v.copy_from_slice(cur_q)),
                second: self.second,
                time: self.time,
            };
            if ok {
                self.missed_in_row = 0;
            } else {
                self.state.misses += 1;
                self.missed_in_row += 1;
            }
            self.state.locked = self.missed_in_row < 3;
            self.state.parity = if self.second { 2 } else { 1 };
            // Start the next field with this segment, after the end of this field.
            for (lead, cur) in [(&mut self.lead, &self.cur), (&mut self.lead_q, &self.cur_q)] {
                lead.clear();
                lead.extend_from_slice(&cur[cur.len() - SEG - LEAD..cur.len() - SEG]);
            }
            let second = !self.second;
            self.start(seg, second);
            if self.missed_in_row >= 3 {
                self.reset();
                return None;
            }
            return Some(field);
        }
        None
    }

    /// Starts a field at its sync segment `seg`, after the lead held (zeros where none is held).
    fn start(&mut self, seg: &Segment, second: bool) {
        for (cur, lead, new) in [
            (&mut self.cur, &self.lead, &seg.syms),
            (&mut self.cur_q, &self.lead_q, &seg.quad),
        ] {
            cur.clear();
            cur.resize(LEAD - lead.len(), 0.0);
            cur.extend_from_slice(lead);
            cur.extend_from_slice(new);
        }
        self.segs = 1;
        self.second = second;
        self.time = seg.time;
    }

    /// Keeps the newest [`LEAD`] symbols while no field is in assembly.
    fn keep(&mut self, seg: &Segment) {
        for (lead, new) in [(&mut self.lead, &seg.syms), (&mut self.lead_q, &seg.quad)] {
            lead.extend_from_slice(new);
            if lead.len() > LEAD {
                lead.drain(..lead.len() - LEAD);
            }
        }
    }
}

/// The equalised data of a field.
#[derive(Clone, Debug, PartialEq)]
pub struct SoftField {
    /// 312 × 828 levels.
    pub levels: Arc<[f32]>,
    /// SNR on the known symbols, dB.
    pub snr_db: f64,
    /// The estimate is usable (SNR at least 9 dB).
    pub usable: bool,
    /// Field 2.
    pub second: bool,
    /// Signal time of the field sync.
    pub time: f64,
    /// The field does not follow the one before it (a lock lost and found): the decoder starts again with it.
    pub gap: bool,
}

/// Which equaliser made the levels of a field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EqKind {
    /// The feed-forward equaliser.
    #[default]
    Plain,
    /// The decision-feedback equaliser.
    Feedback,
}

/// What the equaliser did with a badly decoded field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetryCounters {
    /// Fields that got further tries.
    pub fields: u64,
    /// Tries with in-field tracking that lost fewer segments.
    pub tracking: u64,
    /// Tries with feedback taps from the field's own decoded symbols that lost fewer segments.
    pub decided: u64,
    /// Tries with the feed-forward equaliser that lost fewer segments.
    pub plain: u64,
}

/// Feed-forward taps of the decision-feedback equaliser, those before its cursor, and its feedback taps.
pub const DFE_FF: usize = 65;
const DFE_BEFORE: usize = 16;
/// Feedback taps.
pub const DFE_FB: usize = 128;
/// The nominal time between two fields, s.
const FIELD_S: f64 = (DATA_SEGS + 1) as f64 * SEG as f64 / FSYM;
/// The ridge of a fit on the sync symbols and of one on decided symbols.
const RIDGE_SYNC: f64 = 1e-3;
const RIDGE_DECIDED: f64 = 1e-6;
/// A field whose SNR on the known symbols and on the data against their nearest levels is at least this is not decoded by
/// the equaliser (dB; the code needs 15).
const SURE_DB: f64 = 19.0;
/// Fields that decode clean before in-field tracking stops.
const TRACKING_FIELDS: u32 = 200;
/// The symbols a field is equalised over by the sequential equalisers: its sync segment and its data.
const RUN_END: usize = (DATA_SEGS + 1) * SEG;

/// Taps before (post-echoes) and after (pre-echoes) the cursor.
pub const PRE: usize = 103;
/// Taps after the cursor.
pub const POST: usize = 24;

/// SNR on the known symbols under which no equaliser fits (dB).
const FIT_DB: f64 = 9.0;
/// Under this SNR a second look for a weaker earlier path runs (dB).
const SECOND_LOOK_DB: f64 = 16.0;
/// Under this SNR a look for a later path beyond the equaliser runs (dB).
const POST_LOOK_DB: f64 = 18.0;
/// Fields between two looks for an echo while they do not help.
const LOOK_EVERY: u64 = 8;
/// The earliest path searched before the strongest (symbols, 30.7 us).
const PRE_MAX: usize = 330;
/// The nearest and farthest later path for the post-echo canceller (symbols, 11 to 44.6 us).
const POST_MIN: usize = 118;
const POST_MAX: usize = 480;

/// The per-field equaliser.
///
/// - **Feed-forward**: a filter of 128 taps, least squares on the field sync and the segment syncs, ridge regularised. Re-solved
///   every field until decoding is clean, then every fourth field.
/// - **Decision feedback** ([`RealDfe`], 65 feed-forward and 128 feedback taps) when the feed-forward equaliser does not fit
///   (SNR on the known symbols under 9 dB), or fits poorly (under 16 dB: tried at most every eighth field, and taken when it
///   fits 2 dB better), trained on the field syncs. Once a field has decoded with it, its taps come from the decoded symbols
///   of a field (every fourth field, and at once after a field with damaged packets). Its feedback reads the survivors of
///   the trellis code ([`TrellisSlicer`]), so a wrong decision does not come back through a strong tap.
/// - **Retries**: a decoder of its own ([`FieldDecoder`]) decodes every field that is not sure to decode (the feed-forward
///   equaliser alone at 19 dB or more on the known symbols and on every eighth of the data). When 10 % or more of the segments fail
///   Reed-Solomon, the field is equalised again, each try replacing the result when it loses fewer segments: the same taps with
///   in-field tracking; feedback taps solved from the field's own decoded symbols; and, when 20 % still fail, the feed-forward
///   equaliser. A winning try is the first try of the next fields; tracking stops after 200 fields that decode clean.
/// - **In-field tracking** ([`DfeTracking`]): the field is equalised a segment at a time and the taps move after each.
/// - **A gap** (a field that does not follow the one before): the decoder starts again, the taps stay and are tried first.
///
/// Before it, a long echo canceller ([`EchoCanceller`]) can turn the field into its strongest path alone, fitted again on every
/// field's sync. It works on the symbols with their part in quadrature, so the echo can have any carrier phase:
///
/// - a pre-echo canceller when no equaliser fits (SNR on the known symbols under 9 dB) and the field sync shows an earlier path
///   of 0.4 of the strongest or more, up to 330 symbols before it; or when the fit is poor (under 16 dB) and a second look, at
///   most every eighth field while it does not help, finds an earlier path of 0.2 or more beyond the taps after the cursor. It
///   is dropped once a field fits at 9 dB without it (tried every eighth field);
/// - a post-echo canceller when the fit is under 18 dB (or nothing fits): at most every eighth field, the latest path of 0.2 or
///   more 118 to 480 symbols after the strongest; kept when the equaliser after it beats the fit without it by 0.5 dB (or nothing
///   fitted).
///
/// A canceller that fails to fit a field is dropped, and so is one with which two fields in a row lose half of their segments.
pub struct Equaliser {
    taps: Vec<f32>,
    bias: f32,
    span: (usize, usize),
    fields: u64,
    clean: u32,
    gain: f32,
    /// The last impulse response (taps), for the scope.
    pub last_snr: f64,
    snr_at_solve: f64,
    ata: Vec<f64>,
    atb: Vec<f64>,
    /// Of a run of known symbols: the samples under its rows, the sums of products per column distance, the symbols.
    win: Vec<f64>,
    prod: Vec<f64>,
    want: Vec<f64>,
    fir: Fir<f32>,
    rev: Vec<f32>,
    /// The field through `fir`: symbol `t` is at `t + POST`.
    filtered: Vec<f32>,
    levels: Vec<f32>,
    pool: SlicePool<f32>,
    canceller: Option<EchoCanceller>,
    rule: EchoFitRule,
    /// The field with the canceller applied, and its part in quadrature.
    cancelled: Vec<f32>,
    cancelled_q: Vec<f32>,
    /// Fields since the last look for an echo.
    since_look: u64,
    /// Fields in a row that lost half of their segments with a canceller in use.
    lost_cancelled: u8,
    scratch: Vec<f64>,
    dfe: RealDfe,
    /// The feed-forward taps in the form that runs with tracking.
    plain_run: RealDfe,
    /// The equaliser of the field in work.
    kind: EqKind,
    /// Feedback is in use: it is tried also when the feed-forward equaliser fits.
    prefer_feedback: bool,
    /// This field may try feedback on a poor feed-forward fit, and the fields since such a try.
    feedback_look: bool,
    since_feedback: u64,
    /// The feedback taps come from decoded symbols.
    dfe_decided: bool,
    tracking: bool,
    tracking_clean: u32,
    track: DfeTracking,
    judge: FieldDecoder,
    /// Decides the symbols of a sequential run.
    slicer: TrellisSlicer,
    /// The decoder's state before the field in work, and after the best try of it.
    before: FecState,
    after: FecState,
    /// Of the field in work: the known symbols (NaN elsewhere), the output and the decisions of a sequential run, the decoded
    /// symbols, and the levels of a try.
    known: Vec<f32>,
    y: Vec<f32>,
    d: Vec<f32>,
    decoded: Vec<f32>,
    trial: Vec<f32>,
    kept: (Vec<f32>, Vec<f32>, f32),
    last_time: Option<f64>,
    /// Retries so far.
    pub retries: RetryCounters,
}

impl Default for Equaliser {
    fn default() -> Self {
        Equaliser::new()
    }
}

impl Equaliser {
    /// An untrained equaliser.
    pub fn new() -> Equaliser {
        let n = PRE + POST + 1;
        Equaliser {
            taps: vec![0.0; n],
            bias: 0.0,
            span: (0, n),
            fields: 0,
            clean: 0,
            gain: 1.0,
            last_snr: 0.0,
            snr_at_solve: 0.0,
            ata: vec![0.0; (n + 1) * (n + 1)],
            atb: vec![0.0; n + 1],
            win: Vec::with_capacity(728 + n),
            prod: vec![0.0; n],
            want: Vec::with_capacity(728),
            fir: Fir::new(&vec![0.0; n]),
            rev: vec![0.0; n],
            filtered: Vec::new(),
            levels: Vec::with_capacity(DATA_SEGS * SEG_DATA),
            pool: SlicePool::default(),
            canceller: None,
            rule: EchoFitRule::default(),
            cancelled: Vec::new(),
            cancelled_q: Vec::new(),
            since_look: LOOK_EVERY,
            lost_cancelled: 0,
            scratch: Vec::new(),
            dfe: RealDfe::new(DFE_FF, DFE_BEFORE, DFE_FB)
                .unwrap_or_else(|_| unreachable!("valid tap counts")),
            plain_run: RealDfe::new(n, PRE, 0).unwrap_or_else(|_| unreachable!("valid tap counts")),
            kind: EqKind::Plain,
            prefer_feedback: false,
            feedback_look: true,
            since_feedback: LOOK_EVERY,
            dfe_decided: false,
            tracking: false,
            tracking_clean: 0,
            track: DfeTracking::default(),
            judge: FieldDecoder::new(),
            slicer: TrellisSlicer::default(),
            before: FecState::default(),
            after: FecState::default(),
            known: Vec::new(),
            y: Vec::new(),
            d: Vec::new(),
            decoded: Vec::new(),
            trial: Vec::with_capacity(DATA_SEGS * SEG_DATA),
            kept: (vec![0.0; DFE_FF], vec![0.0; DFE_FB], 0.0),
            last_time: None,
            retries: RetryCounters::default(),
        }
    }
    /// The equaliser that made the last field, and whether in-field tracking ran.
    pub fn kind(&self) -> (EqKind, bool) {
        (self.kind, self.tracking)
    }
    /// The long echo canceller in use: its side and delay (symbols).
    pub fn canceller(&self) -> Option<(EchoSide, usize)> {
        self.canceller.as_ref().map(|c| (c.side(), c.delay()))
    }
    /// The feed-forward taps (cursor at index `PRE`).
    pub fn taps(&self) -> &[f32] {
        &self.taps
    }
    /// Decoding of the last field was clean (or not): clean fields let the equaliser be re-solved less often.
    pub fn report(&mut self, clean: bool) {
        if clean {
            self.clean += 1;
        } else {
            self.clean = 0;
        }
    }

    fn known(field: &Field) -> impl Iterator<Item = (usize, f32)> + '_ {
        let k0 = field_sync_known(field.second);
        let k1 = field_sync_known(!field.second);
        let fs0 = (0..728).map(move |i| (i, k0[i]));
        let segs = (1..=DATA_SEGS).flat_map(|k| (0..4).map(move |i| (k * SEG + i, SEG_SYNC[i])));
        let fs1 = (0..728).map(move |i| ((DATA_SEGS + 1) * SEG + i, k1[i]));
        fs0.chain(segs).chain(fs1)
    }

    /// Adds the rows of a run of known symbols `want`, the first at `first`, to the normal equations. A row is the window under
    /// the taps and a 1 for the bias. The window of a row is that of the row before moved by one sample, so the sums of row
    /// `i + 1` of the matrix are those of row `i` with one product in and one out.
    fn add_run(&mut self, x: &[f32], first: usize, want: &[f64]) {
        let n = PRE + POST + 1;
        let m = n + 1;
        let len = want.len();
        // w[q] is sample first - PRE + q: column i of the run's rows is w[i .. i + len].
        self.win.clear();
        self.win
            .extend(x[first - PRE..first + len + POST].iter().map(|&v| v as f64));
        let w = &self.win;
        // prod[j]: column i times column i + j summed over the rows, for i = 0 first.
        for (j, p) in self.prod.iter_mut().enumerate() {
            *p = w[..len].iter().zip(&w[j..]).map(|(a, b)| a * b).sum();
        }
        let mut ones: f64 = w[..len].iter().sum();
        for i in 0..n {
            let k = n - i;
            for (a, p) in self.ata[i * m + i..i * m + n].iter_mut().zip(&self.prod) {
                *a += *p;
            }
            self.ata[i * m + n] += ones;
            self.atb[i] += w[i..].iter().zip(want).map(|(a, d)| a * d).sum::<f64>();
            if k > 1 {
                // Column i leaves the sums at the first row, and the sample after the last row comes in.
                let (out, into) = (w[i], w[i + len]);
                for ((p, o), e) in self.prod[..k - 1]
                    .iter_mut()
                    .zip(&w[i..])
                    .zip(&w[i + len..])
                {
                    *p += into * e - out * o;
                }
                ones += into - out;
            }
        }
        self.ata[n * m + n] += len as f64;
        self.atb[n] += want.iter().sum::<f64>();
    }

    fn solve(&mut self, x: &[f32], field: &Field) {
        let n = PRE + POST + 1;
        let m = n + 1;
        self.ata.iter_mut().for_each(|v| *v = 0.0);
        self.atb.iter_mut().for_each(|v| *v = 0.0);
        let mut rows = 0;
        let mut want = std::mem::take(&mut self.want);
        let mut known = Self::known(field)
            .filter(|&(t, _)| t >= PRE && t + POST < x.len())
            .peekable();
        // The known symbols come in runs next to each other (a field sync, a segment sync).
        while let Some((first, d)) = known.next() {
            want.clear();
            want.push(d as f64);
            while let Some((_, d)) = known.next_if(|&(t, _)| t == first + want.len()) {
                want.push(d as f64);
            }
            rows += want.len();
            self.add_run(x, first, &want);
        }
        drop(known);
        self.want = want;
        if rows < m {
            return;
        }
        let tr: f64 = (0..m).map(|i| self.ata[i * m + i]).sum::<f64>() / m as f64;
        for i in 0..m {
            for j in 0..i {
                self.ata[i * m + j] = self.ata[j * m + i];
            }
            self.ata[i * m + i] += 1e-3 * tr;
        }
        // The solution replaces the right-hand side, which the next solve sets again.
        if cholesky_solve(&mut self.ata, &mut self.atb, m).is_ok() {
            let w = &self.atb;
            for k in 0..n {
                self.taps[k] = w[k] as f32;
            }
            self.bias = w[n] as f32;
            // Trim taps that do nothing (a clean channel needs few).
            let mx = self.taps.iter().fold(0f32, |a, b| a.max(b.abs()));
            let thr = 2e-3 * mx;
            let lo = self.taps.iter().position(|v| v.abs() > thr).unwrap_or(PRE);
            let hi = self.taps.iter().rposition(|v| v.abs() > thr).unwrap_or(PRE) + 1;
            self.span = (lo.min(PRE), hi.max(PRE + 1));
        }
    }

    /// Runs the field `x` through the taps in use (symbols before the field count as zero). [`Equaliser::at`] reads the result.
    fn filter(&mut self, x: &[f32]) {
        let (lo, hi) = self.span;
        // The filter's newest sample is the last one under the taps, POST symbols after the cursor. Taps outside the span are 0.
        for (k, (r, t)) in self.rev.iter_mut().zip(self.taps.iter().rev()).enumerate() {
            let k = PRE + POST - k;
            *r = if k >= lo && k < hi { *t } else { 0.0 };
        }
        self.fir.set_taps(&self.rev);
        self.fir.reset();
        self.filtered.resize(x.len(), 0.0);
        self.fir.process(x, &mut self.filtered);
    }

    /// The equalised symbol `t` of the filtered field (`t + POST` inside the field).
    #[inline]
    fn at(&self, t: usize) -> f32 {
        self.filtered[t + POST] + self.bias
    }

    /// The SNR on the known symbols of the filtered field `x` (from its field sync on), dB.
    fn known_snr(&self, x: &[f32], field: &Field) -> f64 {
        let (mut e2, mut cnt) = (0.0f64, 0usize);
        for (t, d) in Self::known(field) {
            if t < PRE || t + POST >= x.len() {
                continue;
            }
            let y = self.at(t);
            e2 += ((y - d) as f64).powi(2);
            cnt += 1;
        }
        10.0 * (25.0 / (e2 / cnt.max(1) as f64).max(1e-12)).log10()
    }

    /// Solves the taps on `x` (a field from its sync on), filters it and returns the SNR on the known symbols.
    fn fit(&mut self, x: &[f32], field: &Field) -> f64 {
        self.solve(x, field);
        self.filter(x);
        self.known_snr(x, field)
    }

    /// Fits a canceller of the path `delay` symbols before or after the strongest on `all` (the field with its lead, and its
    /// part in quadrature) and applies it into `self.cancelled`. `None` when it is refused or its output is not usable.
    fn cancel(
        &mut self,
        side: EchoSide,
        delay: usize,
        all: (&[f32], &[f32]),
        known: &[f32],
    ) -> Option<EchoCanceller> {
        let (all, quad) = all;
        let mut c = EchoCanceller::fit(side, all, quad, LEAD, known, delay, &self.rule).ok()?;
        self.cancelled.resize(all.len(), 0.0);
        self.cancelled_q.resize(all.len(), 0.0);
        c.apply(all, quad, &mut self.cancelled, &mut self.cancelled_q)
            .then_some(c)
    }

    /// Tries a canceller of `delay` on the field: the SNR after it, and the canceller, which stays out of use.
    fn try_cancel(
        &mut self,
        side: EchoSide,
        delay: usize,
        all: (&[f32], &[f32]),
        known: &[f32],
        field: &Field,
    ) -> Option<(f64, EchoCanceller)> {
        let c = self.cancel(side, delay, all, known)?;
        let u = std::mem::take(&mut self.cancelled);
        let snr = self.fit_best(&u[LEAD..], field);
        self.cancelled = u;
        Some((snr, c))
    }

    /// The known symbols of `field` over `len` symbols from its sync on, NaN where a symbol is not known.
    fn set_known(&mut self, field: &Field, len: usize) {
        self.known.clear();
        self.known.resize(len, f32::NAN);
        for (t, d) in Self::known(field) {
            if t < len {
                self.known[t] = d;
            }
        }
    }

    /// The SNR of the feedback equaliser on the two field syncs of `x`, with the known symbols as its feedback, dB.
    fn feedback_snr(&self, x: &[f32]) -> f64 {
        let (mut e2, mut n) = (0f64, 0usize);
        for s in [0, RUN_END] {
            let (e, k) = self.dfe.error(x, &self.known, s + DFE_FB..s + 728);
            e2 += e;
            n += k;
        }
        10.0 * (25.0 / (e2 / n.max(1) as f64).max(1e-12)).log10()
    }

    /// Decides between the feed-forward equaliser, which fits `x` with `plain` dB, and the feedback equaliser. That one is
    /// tried when the other does not fit, when its fit is poor (under 16 dB; at most every eighth field while feedback is no
    /// better), and while feedback is in use. Its taps are those from decoded symbols while they fit, else they are solved on the
    /// two field syncs (not for a field that keeps its taps). Feedback is chosen when it fits 2 dB better, and kept until the
    /// feed-forward fit is 1 dB better.
    /// Returns the SNR of the one chosen.
    fn choose(&mut self, x: &[f32], field: &Field, plain: f64) -> f64 {
        self.kind = EqKind::Plain;
        self.set_known(field, x.len());
        let look = plain < SECOND_LOOK_DB && self.feedback_look;
        if plain >= FIT_DB && !self.prefer_feedback && !look {
            return plain;
        }
        let mut snr = if self.dfe_decided {
            self.feedback_snr(x)
        } else {
            f64::NEG_INFINITY
        };
        if snr < FIT_DB {
            self.dfe_decided = false;
            self.dfe.begin();
            for s in [0, RUN_END] {
                self.dfe.add_rows(x, &self.known, s + DFE_FB..s + 728);
            }
            if self.dfe.solve(RIDGE_SYNC) {
                snr = self.feedback_snr(x);
            }
        }
        let better = if self.prefer_feedback {
            snr + 1.0 >= plain
        } else {
            snr > plain + 2.0 || plain < FIT_DB
        };
        if snr >= FIT_DB && better {
            self.kind = EqKind::Feedback;
            return snr;
        }
        plain
    }

    /// Fits both equalisers on `x` and returns the SNR of the better (see [`Equaliser::choose`]).
    fn fit_best(&mut self, x: &[f32], field: &Field) -> f64 {
        let plain = self.fit(x, field);
        self.choose(x, field, plain)
    }

    /// The levels of the data of the field `x` from equaliser `kind`, with in-field tracking when `track`, into `out`. The
    /// feed-forward equaliser without tracking reads the filtered field.
    fn levels_of(&mut self, x: &[f32], kind: EqKind, track: bool, out: &mut Vec<f32>) {
        out.clear();
        let gain = self.gain;
        if kind == EqKind::Plain && !track {
            let bias = self.bias;
            for k in 1..=DATA_SEGS {
                let seg = &self.filtered[k * SEG + 4 + POST..(k + 1) * SEG + POST];
                out.extend(seg.iter().map(|v| (v + bias) * gain));
            }
            return;
        }
        self.y.clear();
        self.y.resize(x.len(), 0.0);
        self.d.clear();
        self.d.resize(x.len(), 0.0);
        let tracking = track.then_some(self.track);
        let eq = if kind == EqKind::Plain {
            // The taps always fit the lengths of `plain_run`.
            let _ = self.plain_run.set_taps(&self.taps, &[], self.bias);
            &mut self.plain_run
        } else {
            &mut self.dfe
        };
        eq.set_tracking(tracking);
        eq.run(
            x,
            &self.known,
            &mut self.slicer,
            0..RUN_END,
            &mut self.y,
            &mut self.d,
        );
        for k in 1..=DATA_SEGS {
            out.extend(self.y[k * SEG + 4..(k + 1) * SEG].iter().map(|v| v * gain));
        }
    }

    /// Decodes `levels` as the field after the state `before` and returns the segments that failed Reed-Solomon and the
    /// segments that came out.
    fn judge(&mut self, levels: &[f32]) -> (u64, u64) {
        self.judge.trial(&self.before, levels).unwrap_or((0, 0))
    }

    /// The symbols of the field as the decoder gave them, with the known ones, into `self.decoded` (NaN where neither).
    fn set_decoded(&mut self) {
        self.decoded.clear();
        self.decoded.extend_from_slice(&self.known);
        let dec = self.judge.decided();
        for k in 1..=DATA_SEGS {
            if (k + 1) * SEG <= self.decoded.len() {
                self.decoded[k * SEG + 4..(k + 1) * SEG]
                    .copy_from_slice(&dec[(k - 1) * SEG_DATA..k * SEG_DATA]);
            }
        }
    }

    /// Solves the feedback taps from the decoded symbols of the field `x`. `false`, with the taps unchanged, without a
    /// solution.
    fn solve_decided(&mut self, x: &[f32]) -> bool {
        self.dfe.begin();
        self.dfe.add_run(x, &self.decoded, SEG..RUN_END);
        self.dfe.solve(RIDGE_DECIDED)
    }

    /// Decodes the levels of the field `x` (in `self.levels`, from `self.kind`), and tries again when it decodes badly. Leaves
    /// the best levels in `self.levels`.
    fn decode_and_retry(&mut self, x: &[f32]) {
        let first = std::mem::take(&mut self.levels);
        let (mut failed, words) = self.judge(&first);
        self.levels = first;
        self.judge.keep(&mut self.after);
        let first_kind = self.kind;
        let first_tracking = self.tracking;
        let mut retried = false;
        if words > 0 && failed * 10 >= words {
            retried = true;
            self.retries.fields += 1;
            self.set_decoded();
            let mut trial = std::mem::take(&mut self.trial);
            // (a) The same taps with in-field tracking.
            if !first_tracking {
                if first_kind == EqKind::Feedback {
                    self.keep_feedback();
                }
                self.levels_of(x, first_kind, true, &mut trial);
                let (f, _) = self.judge(&trial);
                if f < failed {
                    failed = f;
                    std::mem::swap(&mut self.levels, &mut trial);
                    self.judge.keep(&mut self.after);
                    self.tracking = true;
                    self.tracking_clean = 0;
                    self.retries.tracking += 1;
                } else if first_kind == EqKind::Feedback {
                    self.restore_feedback();
                }
            }
            // (b) Feedback taps from the field's own decoded symbols.
            self.keep_feedback();
            if self.solve_decided(x) {
                self.levels_of(x, EqKind::Feedback, self.tracking, &mut trial);
                let (f, _) = self.judge(&trial);
                if f < failed {
                    failed = f;
                    std::mem::swap(&mut self.levels, &mut trial);
                    self.judge.keep(&mut self.after);
                    self.kind = EqKind::Feedback;
                    self.prefer_feedback = true;
                    self.dfe_decided = true;
                    self.retries.decided += 1;
                } else {
                    self.restore_feedback();
                }
            }
            // (c) The feed-forward equaliser, when 20 % still fail and it has not made the best try.
            if failed * 5 >= words && (self.kind != EqKind::Plain || self.tracking) {
                self.levels_of(x, EqKind::Plain, false, &mut trial);
                let (f, _) = self.judge(&trial);
                if f < failed {
                    failed = f;
                    std::mem::swap(&mut self.levels, &mut trial);
                    self.judge.keep(&mut self.after);
                    self.kind = EqKind::Plain;
                    self.prefer_feedback = false;
                    self.tracking = false;
                    self.retries.plain += 1;
                }
            }
            self.trial = trial;
        }
        if self.tracking {
            if failed == 0 {
                self.tracking_clean += 1;
                if self.tracking_clean >= TRACKING_FIELDS {
                    self.tracking = false;
                }
            } else {
                self.tracking_clean = 0;
            }
        }
        // The feedback taps for the next fields, from this field's decoded symbols: every fourth field, and at once after
        // damaged packets, while most of the field decodes.
        if !retried
            && self.kind == EqKind::Feedback
            && failed * 5 < words.max(1)
            && (failed > 0 || !self.dfe_decided || self.fields.is_multiple_of(4))
        {
            self.set_decoded();
            self.keep_feedback();
            if self.solve_decided(x) {
                self.dfe_decided = true;
            } else {
                self.restore_feedback();
            }
        }
        // A canceller that fits the known symbols and leaves half of the field lost on two fields in a row cancels a path
        // that is not there: it goes, and the next look waits its turn.
        if self.canceller.is_some() && words > 0 && failed * 2 >= words {
            self.lost_cancelled += 1;
            if self.lost_cancelled >= 2 {
                self.canceller = None;
                self.lost_cancelled = 0;
                self.since_look = 0;
            }
        } else {
            self.lost_cancelled = 0;
        }
        std::mem::swap(&mut self.before, &mut self.after);
    }

    /// Keeps the feedback equaliser's taps, and puts them back.
    fn keep_feedback(&mut self) {
        self.kept.0.copy_from_slice(self.dfe.feed_forward());
        self.kept.1.copy_from_slice(self.dfe.feedback());
        self.kept.2 = self.dfe.bias();
    }
    fn restore_feedback(&mut self) {
        // The kept taps have the lengths of the equaliser.
        let _ = self.dfe.set_taps(&self.kept.0, &self.kept.1, self.kept.2);
    }

    /// Equalises a field with every search and retry.
    pub fn equalise(&mut self, field: &Field) -> SoftField {
        let all = field.syms.clone();
        let quad = field.quad.clone();
        let known = field_sync_known(field.second);
        let gap = self
            .last_time
            .is_some_and(|t| !(0.5 * FIELD_S..1.5 * FIELD_S).contains(&(field.time - t)));
        self.last_time = Some(field.time);
        if gap {
            self.before.clear();
        }
        self.since_feedback += 1;
        self.feedback_look = self.since_feedback >= LOOK_EVERY;
        let mut input = std::mem::take(&mut self.cancelled);
        // The canceller in use, fitted again on this field.
        if let Some((side, delay)) = self.canceller() {
            self.canceller = self.cancel(side, delay, (&all, &quad), known);
            input = std::mem::take(&mut self.cancelled);
        }
        let using = self.canceller.is_some();
        let x: &[f32] = if using { &input[LEAD..] } else { &all[LEAD..] };
        // After a gap the last good taps are tried first.
        let resolve = !gap
            && (self.fields == 0 || self.clean < 2 || self.fields.is_multiple_of(4));
        if resolve {
            self.solve(x, field);
        }
        self.fields += 1;
        self.since_look += 1;
        self.filter(x);
        let mut snr = self.known_snr(x, field);
        if resolve {
            self.snr_at_solve = snr;
        } else if snr < self.snr_at_solve - 1.0 {
            // Trouble: solve again at once.
            snr = self.fit(x, field);
            self.snr_at_solve = snr;
        }
        snr = self.choose(x, field, snr);
        self.cancelled = input;
        let look = self.since_look >= LOOK_EVERY;
        // A try leaves the taps and the filtered field of another input: `dirty` asks for the fit of the input in use again.
        let mut dirty = false;
        let mut best: Option<(f64, EchoCanceller)> = None;
        if !using {
            // An earlier path stronger than the equaliser can take: when nothing fits, or on a second look when the fit is poor.
            let pre = if snr < FIT_DB {
                Some((6, 0.4))
            } else if snr < SECOND_LOOK_DB && look {
                self.since_look = 0;
                Some((POST + 2, 0.2))
            } else {
                None
            };
            if let Some((min_lag, rel)) = pre {
                let path = earliest_path(
                    &all,
                    &quad,
                    LEAD,
                    known,
                    min_lag,
                    PRE_MAX,
                    rel,
                    &mut self.scratch,
                );
                if let Some(p) = path {
                    dirty = true;
                    best = self
                        .try_cancel(
                            EchoSide::Pre,
                            (-p.lag) as usize,
                            (&all, &quad),
                            known,
                            field,
                        )
                        .filter(|(s, _)| *s >= FIT_DB && *s > snr);
                }
            }
            if best.is_none() && snr < POST_LOOK_DB && look {
                self.since_look = 0;
                let path = latest_path(
                    &all,
                    &quad,
                    LEAD,
                    known,
                    POST_MIN,
                    POST_MAX,
                    0.2,
                    &mut self.scratch,
                );
                if let Some(p) = path {
                    dirty = true;
                    best = self
                        .try_cancel(EchoSide::Post, p.lag as usize, (&all, &quad), known, field)
                        .filter(|(s, _)| *s > snr + 0.5 || (snr < FIT_DB && *s > snr));
                }
            }
        } else if self.canceller().is_some_and(|c| c.0 == EchoSide::Pre) && look {
            // A pre-echo canceller goes once a field fits without it.
            self.since_look = 0;
            let plain = self.fit_best(&all[LEAD..], field);
            dirty = true;
            if plain >= FIT_DB {
                self.canceller = None;
                snr = plain;
                self.snr_at_solve = plain;
                dirty = false;
            }
        }
        if let Some((s, c)) = best {
            // The taps and the filtered field are those of the winning try.
            self.canceller = Some(c);
            snr = s;
            self.snr_at_solve = s;
            dirty = false;
        }
        // The input in use, with the taps and the filtered field of its fit.
        let u = std::mem::take(&mut self.cancelled);
        let x = if self.canceller.is_some() {
            &u[LEAD..]
        } else {
            &all[LEAD..]
        };
        if dirty {
            snr = self.fit_best(x, field);
            self.snr_at_solve = snr;
        }
        self.last_snr = snr;
        // Feedback stays in use from the field that chose it until the feed-forward equaliser is chosen again.
        if self.feedback_look && snr < SECOND_LOOK_DB {
            self.since_feedback = 0;
        }
        self.prefer_feedback = self.kind == EqKind::Feedback;
        let usable = snr >= FIT_DB;
        let mut levels = std::mem::take(&mut self.levels);
        self.levels_of(x, self.kind, self.tracking, &mut levels);
        self.levels = levels;
        // A field that is sure to decode is not decoded here: the feed-forward equaliser alone, with the known symbols and
        // every part of the data well above the threshold of the code.
        let sure = self.kind == EqKind::Plain
            && !self.tracking
            && snr >= SURE_DB
            && worst_slicer_snr(&self.levels) >= SURE_DB;
        if usable && !sure {
            self.decode_and_retry(x);
        } else {
            self.before.clear();
        }
        self.cancelled = u;
        let levels = std::mem::take(&mut self.levels);
        let (mut num, mut den) = (0.0f64, 0.0f64);
        for seg in levels.as_chunks::<SEG_DATA>().0 {
            // The sums of one segment in four lanes.
            let mut sums = [[0f32; 4]; 2];
            for ys in seg.as_chunks::<4>().0 {
                for l in 0..4 {
                    let q = nearest_level(ys[l]);
                    sums[0][l] += ys[l] * q;
                    sums[1][l] += q * q;
                }
            }
            num += sums[0].iter().sum::<f32>() as f64;
            den += sums[1].iter().sum::<f32>() as f64;
        }
        // Decision-directed gain correction, for the next field.
        if den > 0.0 && snr > 12.0 {
            let g = (num / den) as f32;
            if g > 0.5 && g < 2.0 {
                self.gain /= g.powf(0.5);
            }
        }
        let shared = self
            .pool
            .exact(levels.len(), |v| v.copy_from_slice(&levels));
        self.levels = levels;
        SoftField {
            levels: shared,
            snr_db: snr,
            usable,
            second: field.second,
            time: field.time,
            gap,
        }
    }
}

/// The SNR of `levels` against their nearest levels, of the worst of 8 parts of the field, dB.
fn worst_slicer_snr(levels: &[f32]) -> f64 {
    let part = (levels.len() / 8).max(1);
    let mut worst = 0f64;
    for p in levels.chunks(part) {
        let mut lanes = [0f32; 4];
        for ys in p.as_chunks::<4>().0 {
            for l in 0..4 {
                let e = ys[l] - nearest_level(ys[l]);
                lanes[l] += e * e;
            }
        }
        worst = worst.max(f64::from(lanes.iter().sum::<f32>()) / p.len() as f64);
    }
    10.0 * (21.0 / worst.max(1e-12)).log10()
}

/// The nearest of the 8 levels.
#[inline]
pub fn nearest_level(y: f32) -> f32 {
    (((y + 7.0) * 0.5).round().clamp(0.0, 7.0)) * 2.0 - 7.0
}

#[cfg(test)]
mod tests {
    use super::super::fec::{FieldDecoder, FieldEncoder};
    use super::*;
    use sdrkit_algo_sim::rng::Rng;
    use sdrkit_proto_mpegts::outer::TS;
    use sdrkit_proto_mpegts::source::Numbered;

    /// `n` fields of symbols through a path `lag` symbols late (negative: early) whose gain at field position `p` (fields,
    /// fractional) is `gain(p)`, with noise `snr_db` under the direct path.
    fn fields(n: usize, lag: i64, gain: impl Fn(f64) -> f32, snr_db: f64, seed: u64) -> Vec<Field> {
        let mut enc = FieldEncoder::new();
        let mut rng = Rng::new(seed);
        let per = (DATA_SEGS + 1) * SEG;
        let mut s = Vec::with_capacity((n + 1) * per);
        let mut k = 0u32;
        for _ in 0..=n {
            let pk: Vec<[u8; TS]> = (0..DATA_SEGS)
                .map(|_| {
                    k += 1;
                    Numbered::TERRESTRIAL.packet(k - 1)
                })
                .collect();
            enc.encode_field(&pk, &mut s);
        }
        let sigma = (21.0f64 / 10f64.powf(snr_db / 10.0)).sqrt();
        let r: Vec<f32> = (0..s.len())
            .map(|t| {
                let from = t as i64 - lag;
                let e = if from >= 0 && (from as usize) < s.len() {
                    s[from as usize]
                } else {
                    0.0
                };
                s[t] + gain(t as f64 / per as f64) * e + (sigma * rng.normal()) as f32
            })
            .collect();
        (0..n)
            .map(|f| {
                let start = f * per;
                let mut syms = vec![0f32; LEAD.saturating_sub(start)];
                syms.extend_from_slice(&r[start.saturating_sub(LEAD)..start + per + SEG]);
                Field {
                    quad: vec![0f32; syms.len()].into(),
                    syms: syms.into(),
                    second: f % 2 == 1,
                    time: f as f64 * FIELD_S,
                }
            })
            .collect()
    }

    /// Equalises and decodes `fields`; returns the Reed-Solomon failures of the fields from `from` on and the segments
    /// they gave.
    fn decode(eq: &mut Equaliser, fields: &[Field], from: usize) -> (u64, u64) {
        let mut dec = FieldDecoder::new();
        let mut pk = Vec::new();
        let (mut failed, mut words) = (0, 0);
        for (f, field) in fields.iter().enumerate() {
            let sf = eq.equalise(field);
            assert!(sf.usable, "field {f}: {} dB", sf.snr_db);
            let c0 = dec.counters;
            pk.clear();
            dec.push_field(&sf.levels, &mut pk);
            if f > from {
                failed += dec.counters.rs_failed - c0.rs_failed;
                words += dec.counters.packets - c0.packets;
            }
        }
        (failed, words)
    }

    #[test]
    fn a_strong_post_echo_goes_to_the_feedback_equaliser() {
        // -3 dB, 54 symbols late (5 us), 28 dB: the feed-forward taps cannot undo it.
        let f = fields(6, 54, |_| 0.708, 28.0, 3);
        let mut eq = Equaliser::new();
        let (failed, words) = decode(&mut eq, &f, 1);
        assert_eq!(eq.kind().0, EqKind::Feedback);
        assert!(words >= 4 * 312 && failed == 0, "{failed} of {words}");
        assert!(eq.last_snr > 18.0, "{}", eq.last_snr);
    }

    #[test]
    fn a_field_that_decodes_badly_is_tried_again_with_tracking() {
        // An echo 30 symbols late whose gain swings between 0.15 and 0.65 once in two fields, 24 dB: taps solved on the field
        // sync are wrong in the middle of the field.
        let swing = |p: f64| 0.4 + 0.25 * (std::f64::consts::PI * p).sin() as f32;
        let f = fields(8, 30, swing, 24.0, 4);
        let mut eq = Equaliser::new();
        let (failed, words) = decode(&mut eq, &f, 2);
        assert!(
            eq.retries.fields >= 1 && eq.retries.tracking >= 1,
            "{:?}",
            eq.retries
        );
        assert!(eq.kind().1, "tracking stays on");
        assert!(
            words >= 5 * 312 && failed * 10 < words,
            "{failed} of {words}"
        );
    }

    #[test]
    fn fields_that_decode_get_no_further_try() {
        // No echo, 22 dB: the feed-forward equaliser, no retry, no tracking.
        let f = fields(5, 30, |_| 0.0, 22.0, 7);
        let mut eq = Equaliser::new();
        let (failed, words) = decode(&mut eq, &f, 1);
        assert_eq!((failed, words), (0, 3 * 312));
        assert_eq!(eq.retries, RetryCounters::default());
        assert_eq!(eq.kind(), (EqKind::Plain, false));
        assert_eq!(eq.canceller(), None);
    }

    #[test]
    fn echoes_at_the_ends_of_each_span_are_covered() {
        // Post-echoes of -6 dB at the last feed-forward tap before the cursor (103), around the first lag of the post-echo
        // canceller (118) and at the last feedback tap (128); pre-echoes of -10 dB around the first lag of the pre-echo
        // canceller (13) and around the last feed-forward tap after the cursor (24). 26 dB.
        for (lag, gain) in [
            (103, 0.5f32),
            (117, 0.5),
            (118, 0.5),
            (128, 0.5),
            (-12, 0.316),
            (-13, 0.316),
            (-24, 0.316),
            (-25, 0.316),
            (-26, 0.316),
        ] {
            let f = fields(5, lag, |_| gain, 26.0, 6);
            let mut eq = Equaliser::new();
            let (failed, words) = decode(&mut eq, &f, 1);
            assert!(
                words >= 3 * 312 && failed == 0,
                "lag {lag}: {failed} of {words}, {:?} {:?}",
                eq.kind(),
                eq.canceller()
            );
        }
    }

    #[test]
    fn a_field_after_a_missing_one_is_a_gap() {
        let f = fields(3, 30, |_| 0.4, 24.0, 4);
        let mut eq = Equaliser::new();
        assert!(!eq.equalise(&f[0]).gap);
        assert!(eq.equalise(&f[2]).gap);
    }

    #[test]
    fn the_field_sync_gives_the_place_and_the_phase_of_the_path() {
        let known = field_sync_known(false);
        let mut rng = Rng::new(21);
        for (shift, phase) in [(0usize, 0.0f64), (1, 1.2), (3, -2.5), (4, 3.0)] {
            // The sync `shift - 2` symbols from its place, turned by `phase`, in data and noise.
            let (sin, cos) = phase.sin_cos();
            let mut seg: Vec<(f32, f32)> = (0..SEG)
                .map(|_| (4.6 * rng.normal_f32(), 4.6 * rng.normal_f32()))
                .collect();
            for (i, k) in known.iter().enumerate().skip(4) {
                let at = i + shift - 2;
                seg[at] = (
                    k * cos as f32 + 0.5 * rng.normal_f32(),
                    k * sin as f32 + 0.5 * rng.normal_f32(),
                );
            }
            let (offset, found) = field_sync_place(&seg).expect("a field sync");
            assert!(
                (offset - (shift as f64 - 2.0)).abs() < 0.1,
                "{shift}: {offset}"
            );
            let d = (found - phase).rem_euclid(std::f64::consts::TAU);
            assert!(
                d.min(std::f64::consts::TAU - d) < 0.1,
                "{shift}: {found} for {phase}"
            );
        }
        // Data alone has no field sync.
        let seg: Vec<(f32, f32)> = (0..SEG)
            .map(|_| (4.6 * rng.normal_f32(), 4.6 * rng.normal_f32()))
            .collect();
        assert_eq!(field_sync_place(&seg), None);
    }

    #[test]
    fn the_pilot_is_found_between_stronger_lines_beside_it() {
        // 4096 bins of 8 Msps: the channel from the pilot up, the pilot 20 dB over the data beside it.
        let (n, rate) = (4096usize, 8e6);
        let bw = rate / n as f64;
        let bin = |hz: f64| (n as f64 / 2.0 + hz / bw).round() as usize;
        let pilot = bin(PILOT_HZ);
        let band = |p: &mut [f64]| {
            for (i, v) in p.iter_mut().enumerate() {
                *v = if i + 150 >= pilot && i < pilot + bin(5.38e6) - n / 2 {
                    1.0
                } else {
                    1e-3
                };
            }
        };
        let mut p = vec![0f64; n];
        band(&mut p);
        p[pilot] = 100.0;
        let found = find_pilot(&p, rate).expect("the pilot");
        assert!(found.shift_hz.abs() < bw && !found.mirrored, "{found:?}");
        // Two paths that oppose each other at the pilot: the pilot 6 dB over the data, lines 26 bins under and over it 10 dB
        // over the data.
        band(&mut p);
        p[pilot] = 4.0;
        p[pilot - 26] = 10.0;
        p[pilot + 26] = 12.0;
        let found = find_pilot(&p, rate).expect("the pilot");
        assert!(found.shift_hz.abs() < bw && !found.mirrored, "{found:?}");
        // A crest of a ripple as wide as many bins is no line.
        band(&mut p);
        for k in 0..40 {
            p[pilot + 60 + k] = 1.0 + 4.0 * (std::f64::consts::PI * k as f64 / 40.0).sin();
        }
        assert_eq!(find_pilot(&p, rate), None);
    }

    #[test]
    fn a_gap_restarts_the_decoder_and_keeps_the_taps() {
        let f = fields(7, 54, |_| 0.708, 28.0, 5);
        let mut eq = Equaliser::new();
        for field in &f[..3] {
            assert!(!eq.equalise(field).gap);
        }
        assert_eq!(eq.kind().0, EqKind::Feedback);
        // Fields 3 and 4 are lost.
        let sf = eq.equalise(&f[5]);
        assert!(sf.gap && sf.usable);
        assert_eq!(eq.kind().0, EqKind::Feedback);
        assert!(!eq.equalise(&f[6]).gap);
    }
}
