//! One S1G receiving chain: the signal moved and resampled to its grid rate, the preamble found and timed, the offset and channel
//! measured, the SIG field read and the data field decoded.
//! Standard: IEEE Std 802.11-2020 clause 23.3.8 and 23.3.9.

use super::coding::{Bcc, DataDecoder, Demapper};
use super::phy::*;
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::{FractionalResampler, RationalResampler};
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The long training symbol of a width: the width, its time samples on the chain's grid and its tones.
type Lts = (u32, Vec<Complex32>, Vec<(i32, f32)>);

sdrkit_core::choice_enum! {
    /// Why the data field of a PPDU was not decoded. The label is the text of the frame log.
    #[derive(Serialize, Deserialize)]
    pub enum Undecoded {
        /// A null data packet: it has no data field.
        Ndp = ("ndp", "NDP"),
        /// Space-time block coding.
        Stbc = ("stbc", "STBC"),
        /// Several spatial streams.
        SpatialStreams = ("spatial_streams", "spatial streams"),
        /// LDPC coding.
        Ldpc = ("ldpc", "LDPC"),
        /// An A-MPDU.
        Ampdu = ("ampdu", "A-MPDU"),
        /// A length of 0.
        Empty = ("empty", "empty"),
        /// A length that the frame cannot hold.
        Length = ("length", "length"),
    }
}

impl Undecoded {
    /// The text of the frame log.
    pub fn name(self) -> &'static str {
        match self {
            Undecoded::Ndp => "NDP",
            Undecoded::Stbc => "STBC",
            Undecoded::SpatialStreams => "spatial streams",
            Undecoded::Ldpc => "LDPC",
            Undecoded::Ampdu => "A-MPDU",
            Undecoded::Empty => "empty",
            Undecoded::Length => "length",
        }
    }
}

/// One PPDU (the `s1g.ppdu` stream).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct S1gPpdu {
    /// Width, MHz.
    pub width: u32,
    /// Preamble.
    pub preamble: Option<Preamble>,
    /// The SIG field.
    pub sig: Sig,
    /// The data was decoded.
    pub decoded: bool,
    /// PSDU bytes with the FCS (not yet checked).
    pub psdu: Vec<u8>,
    /// Why the data was not decoded, `None` when it was or when it failed to decode.
    pub reason: Option<Undecoded>,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Tone SNR, dB.
    pub snr_db: f64,
    /// The chain's offset from the centre, Hz.
    pub sub_offset_hz: f64,
    /// Signal time of the STF start, s.
    pub time_s: f64,
}

impl S1gPpdu {
    /// Spatial streams.
    pub fn nss(&self) -> u8 {
        self.sig.nss()
    }
}

/// A chain's counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChainStats {
    /// Preambles detected.
    pub preambles: u64,
    /// SIG fields read.
    pub sig_ok: u64,
    /// SIG fields not readable.
    pub sig_bad: u64,
    /// Data fields cut short.
    pub data_bad: u64,
}

impl std::ops::AddAssign for ChainStats {
    fn add_assign(&mut self, o: Self) {
        self.preambles += o.preambles;
        self.sig_ok += o.sig_ok;
        self.sig_bad += o.sig_bad;
        self.data_bad += o.data_bad;
    }
}

#[derive(Clone, Copy, Debug)]
struct Cand {
    det: u64,
    coarse: f64,
}

/// The delay-and-correlate gate for a 1 MHz PPDU at the chain's centre: the samples pass a moving sum of 1 µs (noise bandwidth
/// 1 MHz, the six tones of the short training field within 2.1 dB), and the correlation runs over 96 µs of the field's 160 µs.
/// It measures the same normalised correlation at a lag of 8 µs as the chain's gate, in the 1 MHz of the signal: a 1 MHz
/// signal at 3 dB in 1 MHz reads near 0.6 here, against 0.29 in the 4 MHz chain. On noise the 96 values of the window that are
/// independent give a mean square of 1/96, so 0.30 is passed with a chance of about 0.02 % (0.3 % with a window of 64 µs,
/// which opened 28 times in 30 s of noise against 2).
struct NarrowGate {
    box_len: usize,
    lag: usize,
    win: usize,
    raw: Vec<Complex32>,
    sum: Complex32,
    hist: Vec<Complex32>,
    prods: Vec<Complex32>,
    p1: Vec<f32>,
    p2: Vec<f32>,
    c: (f64, f64),
    a: f64,
    b: f64,
    run: u32,
    armed: bool,
    n: u64,
}

impl NarrowGate {
    fn new(rate: f64, lag: usize, win: usize) -> NarrowGate {
        let zero = Complex32::new(0.0, 0.0);
        let box_len = ((rate / 1e6).round() as usize).max(1);
        NarrowGate {
            box_len,
            lag,
            win,
            raw: vec![zero; box_len],
            sum: zero,
            hist: vec![zero; lag],
            prods: vec![zero; win],
            p1: vec![0.0; win],
            p2: vec![0.0; win],
            c: (0.0, 0.0),
            a: 0.0,
            b: 0.0,
            run: 0,
            armed: true,
            n: 0,
        }
    }

    /// Takes one sample. Returns the sum of the window's lag products when the gate fires: above 0.30 for `need` samples,
    /// re-armed under 0.20.
    fn push(&mut self, s: Complex32, need: u32) -> Option<(f64, f64)> {
        let n = self.n as usize;
        self.n += 1;
        let i = n % self.box_len;
        self.sum += s - self.raw[i];
        self.raw[i] = s;
        if i == 0 && (n & 8191) < self.box_len {
            self.sum = self.raw.iter().sum();
        }
        let s = self.sum;
        let h = n % self.lag;
        let d = self.hist[h];
        self.hist[h] = s;
        let k = n % self.win;
        let p = s * d.conj();
        let o = self.prods[k];
        self.prods[k] = p;
        self.c.0 += p.re as f64 - o.re as f64;
        self.c.1 += p.im as f64 - o.im as f64;
        let a = s.norm_sqr();
        self.a += a as f64 - self.p1[k] as f64;
        self.p1[k] = a;
        let b = d.norm_sqr();
        self.b += b as f64 - self.p2[k] as f64;
        self.p2[k] = b;
        if k == 0 && (n & 8191) < self.win {
            self.c = self
                .prods
                .iter()
                .fold((0.0, 0.0), |a, v| (a.0 + v.re as f64, a.1 + v.im as f64));
            self.a = self.p1.iter().map(|&v| v as f64).sum();
            self.b = self.p2.iter().map(|&v| v as f64).sum();
        }
        let c2 = self.c.0 * self.c.0 + self.c.1 * self.c.1;
        let pp = self.a * self.b;
        if self.a <= 1e-10 || self.b <= 1e-10 || c2 < 0.04 * pp {
            self.armed = true;
            self.run = 0;
        } else if c2 > 0.09 * pp {
            self.run += 1;
            if self.armed && self.run >= need {
                self.armed = false;
                return Some(self.c);
            }
        } else {
            self.run = 0;
        }
        None
    }
}

struct Frame {
    ppdu: S1gPpdu,
    w: u32,
    pre: Preamble,
    f: f64,
    data_start: u64,
    end: u64,
    dltf: u64,
    h: Vec<Complex32>,
    sigma2: f32,
    sig_syms: usize,
}

/// One chain.
pub struct S1gChain {
    rate_in: f64,
    /// The chain's grid rate.
    pub rate: f64,
    #[allow(dead_code)]
    s: usize,
    n_fft: usize,
    /// Widths tried, in order.
    pub widths: Vec<u32>,
    /// Offset of the chain from the radio centre (sub-channel scan), Hz.
    pub sub_offset: f64,
    scan: bool,
    nco: Option<Nco>,
    rs: ChainResampler,
    shift: f64,
    /// Counts every detection (the first main chain); the others count a detection only when its SIG is read.
    pub primary: bool,
    k_ring: usize,
    mixed: Vec<Complex32>,
    rsout: Vec<Complex32>,
    y: Vec<Complex32>,
    base: u64,
    n: u64,
    lag: usize,
    win: usize,
    need: u32,
    prods: Vec<Complex32>,
    p1: Vec<f32>,
    p2: Vec<f32>,
    c_acc: (f64, f64),
    p1_acc: f64,
    p2_acc: f64,
    armed: bool,
    run: u32,
    narrow: Option<NarrowGate>,
    last_det: Option<u64>,
    busy_until: u64,
    cands: VecDeque<Cand>,
    frame: Option<Frame>,
    /// Counters.
    pub stats: ChainStats,
    /// Signal time at sample 0.
    pub time_base: f64,
    fft: Fft,
    lts: Vec<Lts>,
    pol: [f32; 127],
    bcc: Bcc,
    dd: DataDecoder,
    perms: super::coding::Interleavers,
    seg: Vec<Complex32>,
    pref: Vec<f64>,
    xplan: Option<(usize, Fft, Fft)>,
    xa: Vec<Complex32>,
    xb: Vec<Complex32>,
    xc: Vec<Complex32>,
}

/// Brings the radio stream to a chain's grid rate: a pass-through, an integer decimator, a rational polyphase resampler or a
/// fractional one, each a windowed sinc keeping the passband free of aliases by 60 dB.
pub enum ChainResampler {
    /// The rates are equal.
    Pass,
    /// An integer reduction.
    Decim(Decimator<Complex32>),
    /// L / M.
    Rational(RationalResampler<Complex32>),
    /// Any other ratio.
    Fractional(FractionalResampler<Complex32>),
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

impl ChainResampler {
    /// From `rate_in` to `rate`, flat to `pass` Hz.
    pub fn new(rate_in: f64, rate: f64, pass: f64) -> ChainResampler {
        if (rate_in - rate).abs() < 1.0 {
            return ChainResampler::Pass;
        }
        let low = rate_in.min(rate);
        let pass = pass.min(0.46 * low);
        let r = rate_in / rate;
        if r > 1.0 && (r - r.round()).abs() < 1e-9 {
            let d = r.round() as usize;
            return ChainResampler::Decim(Decimator::with_alias_free(rate_in, d, pass, pass, 60.0));
        }
        let (a, b) = ((rate_in / 1e3).round() as u64, (rate / 1e3).round() as u64);
        if (a as f64 * 1e3 - rate_in).abs() < 1e-3 && (b as f64 * 1e3 - rate).abs() < 1e-3 {
            let g = gcd(a, b);
            let (l, m) = ((b / g) as usize, (a / g) as usize);
            if l <= 32 && m <= 64 {
                let pf = pass / low;
                return ChainResampler::Rational(
                    RationalResampler::with_spec(l, m, pf, (1.0 - pf).max(pf + 0.02), 60.0)
                        .expect("the filter settings are valid"),
                );
            }
        }
        let stop = (low - pass).max(pass * 1.05);
        ChainResampler::Fractional(
            FractionalResampler::new(
                rate_in,
                rate,
                pass,
                stop.min(rate_in / 2.0 + (rate_in / 2.0 - pass)),
                60.0,
            )
            .expect("valid rates"),
        )
    }
    /// Clears the state.
    pub fn reset(&mut self) {
        match self {
            ChainResampler::Pass => {}
            ChainResampler::Decim(d) => d.reset(),
            ChainResampler::Rational(r) => r.reset(),
            ChainResampler::Fractional(f) => f.reset(),
        }
    }
    /// Appends the output.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        match self {
            ChainResampler::Pass => out.extend_from_slice(x),
            ChainResampler::Decim(d) => d.process(x, out),
            ChainResampler::Rational(r) => r.process(x, out),
            ChainResampler::Fractional(f) => f.process(x, out),
        }
    }
}

fn bin(k: i32, n: usize) -> usize {
    ((k + n as i32) % n as i32) as usize
}

impl S1gChain {
    /// A chain for radio input at `rate_in`, the wanted signal at `offset` Hz from the radio centre (and the chain `sub_offset` from
    /// it). `rate` is the chain's nominal grid (4, 8 or 16 Msps). When the radio rate divided by an integer is a grid of 31.25
    /// kHz tones (N a multiple of 8) that holds the widest width tried, the chain runs there instead (no fractional resampling;
    /// the tone indices do not depend on the grid).
    pub fn new(
        rate_in: f64,
        rate: f64,
        widths: Vec<u32>,
        offset: f64,
        sub_offset: f64,
        scan: bool,
    ) -> S1gChain {
        let widest = widths.iter().copied().max().unwrap_or(1);
        let min_grid = match widest {
            1 | 2 => 2e6,
            4 => 4e6,
            8 => 8e6,
            _ => 16e6,
        };
        let is_grid = |r: f64| {
            let n = r / 31_250.0;
            (n - n.round()).abs() < 1e-6 && (n.round() as u64).is_multiple_of(8)
        };
        let mut chosen: Option<(f64, usize)> = None;
        for d in 1..=64usize {
            let r = rate_in / d as f64;
            if r < min_grid * 0.999_999 {
                break;
            }
            if is_grid(r) {
                chosen = Some((r, d));
            }
        }
        let half = widths
            .iter()
            .map(|&w| {
                plan(w)
                    .used
                    .iter()
                    .map(|k| k.unsigned_abs())
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(13);
        let pass = (half as f64 + 1.5) * 31.25e3;
        let (rate, rs) = match chosen {
            Some((r, 1)) => (r, ChainResampler::Pass),
            Some((r, d)) => (
                r,
                ChainResampler::Decim(Decimator::with_alias_free(
                    rate_in,
                    d,
                    pass.min(0.46 * r),
                    pass.min(0.46 * r),
                    60.0,
                )),
            ),
            None => (rate, ChainResampler::new(rate_in, rate, pass)),
        };
        let n_fft = (rate / 31_250.0).round() as usize;
        let s = n_fft;
        let lag = n_fft / 4;
        let win = 3 * n_fft / 4;
        let narrow = widths.contains(&1).then(|| NarrowGate::new(rate, lag, 3 * n_fft));
        let shift = offset + sub_offset;
        let lts = widths
            .iter()
            .map(|&w| (w, ltf_time_n(w, n_fft), ltf_freq(w)))
            .collect();
        S1gChain {
            rate_in,
            rate,
            s,
            n_fft,
            widths,
            sub_offset,
            scan,
            nco: (shift != 0.0).then(|| Nco::new(rate_in, -shift)),
            rs,
            shift,
            primary: !scan,
            k_ring: 0,
            mixed: Vec::new(),
            rsout: Vec::new(),
            y: Vec::new(),
            base: 0,
            n: 0,
            lag,
            win,
            need: (n_fft / 2) as u32,
            prods: vec![Complex32::new(0.0, 0.0); win],
            p1: vec![0.0; win],
            p2: vec![0.0; win],
            c_acc: (0.0, 0.0),
            p1_acc: 0.0,
            p2_acc: 0.0,
            armed: true,
            run: 0,
            narrow,
            last_det: None,
            busy_until: 0,
            cands: VecDeque::new(),
            frame: None,
            stats: ChainStats::default(),
            time_base: 0.0,
            fft: Fft::forward(n_fft),
            lts,
            pol: pilot_polarity(),
            bcc: Bcc::new(64),
            dd: DataDecoder::new(),
            perms: super::coding::Interleavers::default(),
            seg: Vec::new(),
            pref: Vec::new(),
            xplan: None,
            xa: Vec::new(),
            xb: Vec::new(),
            xc: Vec::new(),
        }
    }

    /// Signal time now.
    pub fn time_s(&self) -> f64 {
        self.time_base + self.n as f64 / self.rate
    }

    /// The input rate.
    pub fn input_rate(&self) -> f64 {
        self.rate_in
    }

    fn us(&self, us: f64) -> u64 {
        (us * self.rate / 1e6).round() as u64
    }

    /// Runs radio samples; PPDUs are appended to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<S1gPpdu>) {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        if let Some(n) = self.nco.as_mut() {
            n.mix_in_place(&mut self.mixed);
        }
        let mut buf = std::mem::take(&mut self.rsout);
        buf.clear();
        self.rs.process(&self.mixed, &mut buf);
        for &s in &buf {
            self.push(s, out);
        }
        self.rsout = buf;
        self.trim();
    }

    fn push(&mut self, s: Complex32, out: &mut Vec<S1gPpdu>) {
        let n = self.n;
        self.y.push(s);
        self.n += 1;
        let k = self.k_ring;
        self.k_ring = if k + 1 == self.win { 0 } else { k + 1 };
        let li = self.y.len().wrapping_sub(1 + self.lag);
        let d = if li < self.y.len() {
            self.y[li]
        } else {
            Complex32::new(0.0, 0.0)
        };
        let p = s * d.conj();
        let o = self.prods[k];
        self.prods[k] = p;
        self.c_acc.0 += p.re as f64 - o.re as f64;
        self.c_acc.1 += p.im as f64 - o.im as f64;
        let a = s.norm_sqr();
        self.p1_acc += a as f64 - self.p1[k] as f64;
        self.p1[k] = a;
        let b = d.norm_sqr();
        self.p2_acc += b as f64 - self.p2[k] as f64;
        self.p2[k] = b;
        if k == 0 && (self.n & 8191) < self.win as u64 {
            self.c_acc = self
                .prods
                .iter()
                .fold((0.0, 0.0), |a, v| (a.0 + v.re as f64, a.1 + v.im as f64));
            self.p1_acc = self.p1.iter().map(|&v| v as f64).sum();
            self.p2_acc = self.p2.iter().map(|&v| v as f64).sum();
        }
        let ready = n >= self.busy_until && self.frame.is_none();
        if let Some(c) = self.narrow.as_mut().and_then(|g| g.push(s, self.need)) {
            // The chain's own gate fires first on a strong signal; this one adds what it missed.
            let fresh = self.last_det.is_none_or(|d| n > d + self.us(160.0));
            if ready && fresh {
                self.candidate(n, c.1.atan2(c.0) * self.rate / (TAU * self.lag as f64));
            }
        }
        // |C| / sqrt(P1 P2): fires above 0.30 for the whole run, re-arms under 0.20 (compared squared, without the roots).
        let c2 = self.c_acc.0 * self.c_acc.0 + self.c_acc.1 * self.c_acc.1;
        let pp = self.p1_acc * self.p2_acc;
        let live = self.p1_acc > 1e-10 && self.p2_acc > 1e-10;
        if !live || c2 < 0.04 * pp {
            self.armed = true;
            self.run = 0;
        } else if c2 > 0.09 * pp {
            self.run += 1;
            if self.armed && self.run >= self.need && ready {
                self.armed = false;
                // Coarse offset from the last 16 µs of lag products.
                let m = self.n_fft / 2;
                let mut c = Complex32::new(0.0, 0.0);
                for i in 0..m {
                    c += self.prods[(k + self.win - i) % self.win];
                }
                self.candidate(n, (c.im as f64).atan2(c.re as f64) * self.rate / (TAU * self.lag as f64));
            }
        } else {
            self.run = 0;
        }
        if (n & 63) == 0 && (!self.cands.is_empty() || self.frame.is_some()) {
            self.service(out);
        }
    }

    /// Queues a detection at sample `det` with the coarse offset `coarse` (Hz).
    fn candidate(&mut self, det: u64, coarse: f64) {
        if self.primary {
            self.stats.preambles += 1;
        }
        if self.cands.len() >= 8 {
            self.cands.pop_front();
        }
        self.cands.push_back(Cand { det, coarse });
        self.last_det = Some(det);
    }

    /// Starts again at signal time `t` (a lazily run scan chain): every buffer and the detector are cleared, the counters stay.
    pub fn reset(&mut self, t: f64) {
        self.y.clear();
        self.base = 0;
        self.n = 0;
        self.k_ring = 0;
        self.prods
            .iter_mut()
            .for_each(|v| *v = Complex32::new(0.0, 0.0));
        self.p1.iter_mut().for_each(|v| *v = 0.0);
        self.p2.iter_mut().for_each(|v| *v = 0.0);
        self.c_acc = (0.0, 0.0);
        self.p1_acc = 0.0;
        self.p2_acc = 0.0;
        self.armed = true;
        self.run = 0;
        if let Some(g) = self.narrow.as_mut() {
            *g = NarrowGate::new(self.rate, g.lag, g.win);
        }
        self.last_det = None;
        self.busy_until = 0;
        self.cands.clear();
        self.frame = None;
        self.time_base = t;
        self.nco = (self.shift != 0.0).then(|| Nco::new(self.rate_in, -self.shift));
        self.rs.reset();
    }

    /// Nothing waits: no candidate and no frame.
    pub fn idle(&self) -> bool {
        self.cands.is_empty() && self.frame.is_none()
    }

    fn gate(&self) -> u64 {
        if self.widths.contains(&1) {
            self.us(575.0)
        } else {
            self.us(365.0)
        }
    }

    fn service(&mut self, out: &mut Vec<S1gPpdu>) {
        if self.frame.is_some() {
            let n = self.n;
            let Some(f) = self.frame.take_if(|f| n > f.end + 2) else {
                return;
            };
            self.decode_data(f, out);
        }
        while let Some(&c) = self.cands.front() {
            if self.n < c.det + self.gate() {
                return;
            }
            self.cands.pop_front();
            if c.det < self.busy_until {
                continue;
            }
            self.examine(c, out);
            if self.frame.is_some() {
                return;
            }
        }
    }

    /// Processes what can be processed now (the end of a stream).
    pub fn flush(&mut self, out: &mut Vec<S1gPpdu>) {
        self.service(out);
    }

    fn trim(&mut self) {
        let mut keep = self.n.saturating_sub((32 * self.n_fft) as u64);
        if let Some(c) = self.cands.front() {
            keep = keep.min(c.det.saturating_sub((8 * self.n_fft) as u64));
        }
        if let Some(f) = &self.frame {
            keep = keep.min(f.ppdu_start());
        }
        let step = (2048 * self.n_fft) as u64;
        if keep > self.base + step {
            let d = (keep - self.base) as usize;
            self.y.drain(..d);
            self.base = keep;
        }
    }

    /// Derotated copy of `[from, from + len)` into `self.seg`, with phase 0 at `from`.
    fn derotate(&mut self, from: u64, len: usize, f: f64) -> bool {
        if from < self.base || from + len as u64 > self.n {
            return false;
        }
        self.seg.clear();
        let start = (from - self.base) as usize;
        self.seg.extend_from_slice(&self.y[start..start + len]);
        // A new oscillator starts at phase 0 on the first sample.
        Nco::new(self.rate, -f).mix_in_place(&mut self.seg);
        true
    }

    fn fft_seg(&mut self, at: usize) -> Vec<Complex32> {
        let mut x = self.seg[at..at + self.n_fft].to_vec();
        self.fft.process(&mut x);
        x
    }

    /// The correlation of `self.seg` with a training symbol at every position, by FFT (into `self.xc`).
    fn xcorr(&mut self, li: usize, count: usize) {
        let n = self.n_fft;
        let m = (count + n).next_power_of_two();
        if self.xplan.as_ref().is_some_and(|p| p.0 != m) {
            self.xplan = None;
        }
        let (_, fwd, inv) = self
            .xplan
            .get_or_insert_with(|| (m, Fft::forward(m), Fft::inverse(m)));
        self.xa.clear();
        self.xa
            .extend_from_slice(&self.seg[..(count + n - 1).min(self.seg.len())]);
        self.xa.resize(m, Complex32::new(0.0, 0.0));
        self.xb.clear();
        self.xb.extend_from_slice(&self.lts[li].1);
        self.xb.resize(m, Complex32::new(0.0, 0.0));
        fwd.process(&mut self.xa);
        fwd.process(&mut self.xb);
        for (a, b) in self.xa.iter_mut().zip(self.xb.iter()) {
            *a *= b.conj();
        }
        inv.process(&mut self.xa);
        let s = 1.0 / m as f32;
        self.xc.clear();
        self.xc.extend(self.xa[..count].iter().map(|v| v * s));
    }

    fn examine(&mut self, c: Cand, out: &mut Vec<S1gPpdu>) {
        let n = self.n_fft;
        // The detection fires between about 12 µs into the STF and its end (160 µs for S1G_1M, 80 µs otherwise): the first long
        // training symbol lies between det + 8 µs and det + 156 µs (1 MHz), or det + 16 µs and det + 84 µs (wider).
        let one = self.widths.contains(&1);
        let lo = c.det + self.us(4.0);
        let hi = c.det + self.us(if one { 160.0 } else { 90.0 });
        let span = (hi - lo) as usize;
        let need = span + 3 * (n + n / 4) + n;
        if lo < self.base || lo + need as u64 > self.n || !self.derotate(lo, need, c.coarse) {
            return;
        }
        // Prefix sums of the power, for the window energies.
        self.pref.clear();
        self.pref.push(0.0);
        let mut acc = 0f64;
        for v in &self.seg {
            acc += v.norm_sqr() as f64;
            self.pref.push(acc);
        }
        let mut hyps: Vec<(f32, u32, u64)> = Vec::new();
        for li in 0..self.lts.len() {
            let w = self.lts[li].0;
            let (k_reps, sp) = if w == 1 {
                (4usize, n + n / 4)
            } else {
                (2usize, n)
            };
            let (a_us, b_us) = if w == 1 { (4.0, 160.0) } else { (12.0, 90.0) };
            let p0 = (c.det + self.us(a_us) - lo) as usize;
            let p1 = ((c.det + self.us(b_us) - lo) as usize).min(span);
            let count = p1 + (k_reps - 1) * sp + 1;
            self.xcorr(li, count);
            let et: f32 = self.lts[li].1.iter().map(|v| v.norm_sqr()).sum();
            let mut best = (0f32, 0u64);
            for p in p0..=p1 {
                let mut sum = 0f32;
                let mut ew = 0f64;
                for j in 0..k_reps {
                    let q = p + j * sp;
                    sum += self.xc[q].norm();
                    ew += self.pref[q + n] - self.pref[q];
                }
                let q = sum / (et * ew as f32 / k_reps as f32).sqrt().max(1e-20) / k_reps as f32;
                if q > best.0 {
                    best = (q, lo + p as u64);
                }
            }
            if best.0 > 0.4 {
                hyps.push((best.0, w, best.1));
            }
        }
        hyps.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let tried = !hyps.is_empty();
        for (_, w, p) in hyps {
            if self.try_hypothesis(c, w, p, out) {
                return;
            }
        }
        // A SIG that fails for every hypothesis; a scan chain does not count its failures (it sees parts of wider signals).
        if !self.scan && tried {
            self.stats.sig_bad += 1;
        }
    }

    fn try_hypothesis(&mut self, c: Cand, w: u32, p: u64, out: &mut Vec<S1gPpdu>) -> bool {
        let n = self.n_fft;
        let bo = (self.n_fft / 16) as u64;
        let full = n + n / 4;
        let (k_reps, sp) = if w == 1 { (4usize, full) } else { (2usize, n) };
        let ltf1_end = if w == 1 {
            p - (n / 4) as u64 + 5 * n as u64
        } else {
            p + 2 * n as u64
        };
        let n_sig = if w == 1 { 6 } else { 2 };
        let seg_from = p - bo;
        let seg_len = (ltf1_end + (n_sig * full) as u64 - seg_from) as usize;
        if !self.derotate(seg_from, seg_len, c.coarse) {
            return false;
        }
        let Some(li) = self.lts.iter().position(|l| l.0 == w) else {
            return false;
        };
        let ltf = self.lts[li].2.clone();
        // Residual offset from the repetitions.
        let reps: Vec<Vec<Complex32>> = (0..k_reps).map(|j| self.fft_seg(j * sp)).collect();
        let mut acc = Complex32::new(0.0, 0.0);
        for j in 0..k_reps - 1 {
            for &(k, _) in &ltf {
                acc += reps[j + 1][bin(k, n)] * reps[j][bin(k, n)].conj();
            }
        }
        let res = (acc.im as f64).atan2(acc.re as f64) * self.rate / (TAU * sp as f64);
        let f = c.coarse + res;
        if !self.derotate(seg_from, seg_len, f) {
            return false;
        }
        let reps: Vec<Vec<Complex32>> = (0..k_reps).map(|j| self.fft_seg(j * sp)).collect();
        let mut h = vec![Complex32::new(0.0, 0.0); n];
        let mut pw = 0f32;
        let mut noise = 0f32;
        for &(k, l) in &ltf {
            let b = bin(k, n);
            let mut m = Complex32::new(0.0, 0.0);
            for r in &reps {
                m += r[b] * l;
            }
            m /= k_reps as f32;
            for r in &reps {
                noise += (r[b] * l - m).norm_sqr();
            }
            h[b] = m;
            pw += m.norm_sqr();
        }
        let used = ltf.len() as f32;
        let mean_h = pw / used;
        if mean_h < 1e-14 {
            return false;
        }
        let sigma2 = (noise / (used * (k_reps as f32 - 1.0)) * k_reps as f32
            / (k_reps as f32 - 1.0))
            .max(mean_h * 1e-4);
        // SIG.
        let sig_at = (ltf1_end - seg_from) as usize;
        let mut soft: Vec<f32> = Vec::with_capacity(104);
        let mut mse = 0f64;
        let mut mse_n = 0usize;
        let mut long = false;
        let pl = if w == 1 { plan(1) } else { plan(2) };
        let segs = if w == 1 {
            vec![(0, Complex32::new(1.0, 0.0))]
        } else {
            segments(w)
        };
        let used_w = plan(w).used;
        for i in 0..n_sig {
            let y = self.fft_seg(sig_at + i * full + n / 4 - bo as usize);
            let z = |k: i32| -> Option<Complex32> {
                let hk = h[bin(k, n)];
                (hk.norm_sqr() > mean_h * 1e-3 && used_w.contains(&k)).then(|| y[bin(k, n)] / hk)
            };
            // Common phase and slope from the pilots.
            let mut cp = Complex32::new(0.0, 0.0);
            let mut pts: Vec<(f32, f32)> = Vec::new();
            for &(cc, g) in &segs {
                for (j, &k) in pl.pilots.iter().enumerate() {
                    if let Some(v) = z(cc + k) {
                        let e = v * g.conj() * (pl.pilot_vals[j] * self.pol[i % 127]);
                        cp += e;
                        pts.push(((cc + k) as f32, e.im.atan2(e.re)));
                    }
                }
            }
            let mut phi0 = cp.im.atan2(cp.re);
            let _ = &pts;
            // Refine the common phase with decisions on the data tones (BPSK on either axis); the timing slope is in the channel.
            let mut dd = Complex32::new(0.0, 0.0);
            for &(cc, g) in &segs {
                for &k in &pl.data {
                    if let Some(zz) = z(cc + k) {
                        let e = zz * g.conj() * Complex32::from_polar(1.0, -phi0);
                        let wgt = h[bin(cc + k, n)].norm_sqr() / mean_h;
                        let d = if e.re.abs() >= e.im.abs() {
                            Complex32::new(e.re.signum(), 0.0)
                        } else {
                            Complex32::new(0.0, e.im.signum())
                        };
                        dd += e * d.conj() * wgt;
                    }
                }
            }
            phi0 += dd.im.atan2(dd.re);
            let slope = 0.0f32;
            let corr = |k: i32| Complex32::from_polar(1.0, -(phi0 + slope * k as f32));
            let mut vals: Vec<Complex32> = Vec::with_capacity(pl.data.len());
            for &k in &pl.data {
                let mut v = Complex32::new(0.0, 0.0);
                for &(cc, g) in &segs {
                    if let Some(zz) = z(cc + k) {
                        let wgt = h[bin(cc + k, n)].norm_sqr() / mean_h;
                        let e = zz * corr(cc + k) * g.conj();
                        v += e * wgt;
                    }
                }
                vals.push(v);
            }
            if w != 1 && i == 1 {
                let er: f32 = vals.iter().map(|v| v.re * v.re).sum();
                let ei: f32 = vals.iter().map(|v| v.im * v.im).sum();
                long = ei > er;
            }
            for v in vals.iter_mut() {
                if long && i == 1 {
                    *v *= Complex32::new(0.0, -1.0);
                }
            }
            // The SNR from the individual tones.
            for &(cc, g) in &segs {
                for &k in &pl.data {
                    if let Some(zz) = z(cc + k) {
                        let mut e = zz * corr(cc + k) * g.conj();
                        if long && i == 1 {
                            e *= Complex32::new(0.0, -1.0);
                        }
                        mse += ((e.re.abs() - 1.0).powi(2) + e.im * e.im) as f64;
                        mse_n += 1;
                    }
                }
            }
            // BPSK: bit 1 is +1, so the LLR (positive means 0) is minus the real part.
            let llr: Vec<f32> = vals.iter().map(|v| -v.re).collect();
            let mut coded = vec![0f32; llr.len()];
            if w == 1 {
                let _ = self.perms.get(24, 1, 8).invert(&llr, &mut coded);
                for t in 0..12 {
                    soft.push(
                        coded[t]
                            + if REP[t] == 1 {
                                -coded[12 + t]
                            } else {
                                coded[12 + t]
                            },
                    );
                }
            } else {
                let _ = self.perms.get(52, 1, 13).invert(&llr, &mut coded);
                soft.extend_from_slice(&coded);
            }
        }
        let pre = if w == 1 {
            Preamble::OneM
        } else if long {
            Preamble::Long
        } else {
            Preamble::Short
        };
        let nbits = if w == 1 { 36 } else { 52 };
        let mut bits = Vec::new();
        if !self.bcc.decode(&soft, nbits, &mut bits) {
            return false;
        }
        // The tail bits the decoder ended on.
        bits.extend([0u8; 6]);
        let Some(sig) = decode_sig(&bits, pre, w) else {
            return false;
        };
        self.stats.sig_ok += 1;
        if !self.primary && !self.scan {
            self.stats.preambles += 1;
        }
        let snr = (-10.0 * (mse / mse_n.max(1) as f64).max(1e-4).log10()).clamp(0.0, 40.0);
        let lv_from = p;
        let lv_n = self.us(64.0) as usize;
        let lv = if lv_from >= self.base && lv_from + lv_n as u64 <= self.n {
            let a = (lv_from - self.base) as usize;
            self.y[a..a + lv_n]
                .iter()
                .map(|v| v.norm_sqr() as f64)
                .sum::<f64>()
                / lv_n as f64
        } else {
            0.0
        };
        let lay = Layout::of_n(pre, &sig, w, self.n_fft);
        let ltf1_start = if w == 1 {
            p - (n / 4) as u64
        } else {
            p - (n / 2) as u64
        };
        let stf_start = ltf1_start - lay.stf as u64;
        let data_start = stf_start + lay.pre_data as u64;
        let end = data_start + (lay.n_sym * lay.sym) as u64;
        self.busy_until = end.min(c.det + self.us(100_000.0));
        let mut ppdu = S1gPpdu {
            width: w,
            preamble: Some(pre),
            sig,
            decoded: false,
            psdu: Vec::new(),
            reason: None,
            cfo_hz: f,
            level_dbfs: sdrkit_algo_dsp::meter::power_to_db(lv).max(-200.0),
            snr_db: snr,
            sub_offset_hz: self.sub_offset,
            time_s: self.time_base + stf_start as f64 / self.rate,
        };
        let reason = if sig.ndp {
            Some(Undecoded::Ndp)
        } else if sig.stbc {
            Some(Undecoded::Stbc)
        } else if sig.nss() > 1 {
            Some(Undecoded::SpatialStreams)
        } else if sig.ldpc {
            Some(Undecoded::Ldpc)
        } else if sig.aggregated {
            Some(Undecoded::Ampdu)
        } else if sig.length == 0 {
            Some(Undecoded::Empty)
        } else if lay.n_sym == 0 || lay.n_sym > 1024 {
            Some(Undecoded::Length)
        } else {
            None
        };
        if reason.is_some() {
            ppdu.reason = reason;
            out.push(ppdu);
            return true;
        }
        let dltf = ltf1_end + (2 * full + full) as u64;
        self.frame = Some(Frame {
            ppdu,
            w,
            pre,
            f,
            data_start,
            end,
            dltf,
            h,
            sigma2,
            sig_syms: n_sig,
        });
        true
    }

    fn decode_data(&mut self, fr: Frame, out: &mut Vec<S1gPpdu>) {
        let n = self.n_fft;
        let bo = (self.n_fft / 16) as u64;
        let w = fr.w;
        let sig = fr.ppdu.sig;
        let lay = Layout::of_n(fr.pre, &sig, w, self.n_fft);
        let from = if fr.pre == Preamble::Long {
            fr.dltf
        } else {
            fr.data_start
        };
        let from = from - bo;
        let len = (fr.end - from) as usize + n;
        let mut ppdu = fr.ppdu;
        if !self.derotate(from, len.min((self.n - from) as usize), fr.f)
            || self.seg.len() < (fr.end - from) as usize
        {
            self.stats.data_bad += 1;
            ppdu.reason = Some(Undecoded::Length);
            out.push(ppdu);
            return;
        }
        let pl = plan(w);
        let mut h = fr.h;
        if fr.pre == Preamble::Long {
            // The data is beamformed: the channel from the first D-LTF.
            let y = self.fft_seg(n / 4);
            for (k, l) in ltf_freq(w) {
                h[bin(k, n)] = y[bin(k, n)] * l;
            }
        }
        let mean_h = pl
            .used
            .iter()
            .map(|&k| h[bin(k, n)].norm_sqr())
            .sum::<f32>()
            / pl.used.len() as f32;
        let Some((bpsc, _, _)) = mcs_params(sig.mcs) else {
            return;
        };
        let mut qam = Demapper::new(bpsc);
        let Some(d) = n_dbps(w, sig.mcs, 1) else {
            return;
        };
        self.dd.start(n_es(d));
        let z0 = fr.sig_syms + if fr.pre == Preamble::Long { 1 } else { 0 };
        let gi = lay.sym - n;
        let base = (fr.data_start - from) as usize;
        let mut soft: Vec<f32> = Vec::with_capacity(pl.data.len() * bpsc);
        for i in 0..lay.n_sym {
            let at = base + i * lay.sym + gi - bo as usize;
            if at + n > self.seg.len() {
                self.stats.data_bad += 1;
                ppdu.reason = Some(Undecoded::Length);
                out.push(ppdu);
                return;
            }
            let y = self.fft_seg(at);
            let pol = self.pol[(z0 + i) % 127];
            let mut cp = Complex32::new(0.0, 0.0);
            let mut pts: Vec<(f32, f32)> = Vec::with_capacity(pl.pilots.len());
            for (j, &k) in pl.pilots.iter().enumerate() {
                let hk = h[bin(k, n)];
                if hk.norm_sqr() < mean_h * 1e-3 {
                    continue;
                }
                let e = y[bin(k, n)] / hk * (pl.pilot_vals[j] * pol);
                cp += e;
                pts.push((k as f32, e.im.atan2(e.re)));
            }
            let phi0 = cp.im.atan2(cp.re);
            // A weighted least-squares fit of phase and slope over the pilots (known) and the data tones (decided).
            let (mut sw, mut sk, mut skk, mut se, mut ske) = (0f32, 0f32, 0f32, 0f32, 0f32);
            let mut fit = |k: f32, e: f32, wgt: f32| {
                sw += wgt;
                sk += wgt * k;
                skk += wgt * k * k;
                se += wgt * e;
                ske += wgt * k * e;
            };
            for &(k, a) in &pts {
                fit(k, wrap(a - phi0), 1.0);
            }
            for &k in &pl.data {
                let hk = h[bin(k, n)];
                let g = hk.norm_sqr();
                if g < mean_h * 1e-3 {
                    continue;
                }
                let zz = y[bin(k, n)] / hk * Complex32::from_polar(1.0, -phi0);
                let d = qam.nearest(zz);
                if d.norm_sqr() > 0.0 {
                    let e = zz * d.conj();
                    fit(k as f32, e.im.atan2(e.re), g / mean_h * d.norm_sqr());
                }
            }
            let det = sw * skk - sk * sk;
            let (dphi, slope) = if det.abs() > 1e-9 {
                ((se * skk - sk * ske) / det, (sw * ske - sk * se) / det)
            } else {
                (if sw > 0.0 { se / sw } else { 0.0 }, 0.0)
            };
            let phi0 = phi0 + dphi;
            soft.clear();
            for &k in &pl.data {
                let hk = h[bin(k, n)];
                let g = hk.norm_sqr();
                if g < mean_h * 1e-3 {
                    soft.extend(std::iter::repeat_n(0.0, bpsc));
                    continue;
                }
                let zz = y[bin(k, n)] / hk * Complex32::from_polar(1.0, -(phi0 + slope * k as f32));
                qam.demap(zz, g / fr.sigma2, &mut soft);
            }
            self.dd.symbol(&soft, w, pl.data.len(), sig.mcs);
        }
        let len = sig.length as usize;
        match self.dd.finish(len, sig.mcs) {
            Some(p) => {
                ppdu.psdu = p.to_vec();
                ppdu.decoded = true;
            }
            None => {
                self.stats.data_bad += 1;
                ppdu.reason = Some(Undecoded::Length);
            }
        }
        out.push(ppdu);
    }
}

impl Frame {
    fn ppdu_start(&self) -> u64 {
        self.dltf.min(self.data_start).saturating_sub(4096)
    }
}

fn wrap(a: f32) -> f32 {
    let mut a = a;
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}
