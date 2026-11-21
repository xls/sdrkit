//! The DVB-T2 receiver as a plain type: P1 search, guard and carrier-mode trials on the first P2 symbols, symbol timing and
//! carrier tracking, channel estimation (SISO and MISO), L1 decoding, cell addressing, time and cell de-interleaving and the
//! FEC of the chosen data PLP and its common PLP. BBFRAMEs leave descrambled through a callback.
//!
//! Standard: ETSI EN 302 755 V1.4.1, and ETSI TS 102 831 V1.2.1 clause 10 (receiver guidelines).

use crate::bicm::Coding;
use crate::error::{Rule, T2Error};
use crate::fec::{supported, IfCells, T2Fec};
use crate::frame::{plp_in_frame, plp_ranges, FrameCells};
use crate::interleave::FreqInterleaver;
use crate::l1::{post_sizes, L1Codec, L1Post, L1Pre, PlpConfig, L1_PRE_BITS, L1_PRE_CELLS};
use crate::p1::{P1Detector, P1Found, P1_LEN};
use crate::params::{
    Bandwidth, CodeRate, FecFrame, Guard, Modulation, P1Info, PilotPattern, Profile,
};
use crate::pilots::{FrameShape, PilotKind, PilotMaps, SymbolKind, SymbolMap};
use sdrkit_algo_modem::ofdm::{
    equalise, ChannelEstimator, FftFramer, OfdmLayout, PilotLs, PilotPattern as Pattern,
};
use sdrkit_core::Complex32;
use sdrkit_graph::{Pool, SlicePool};
use sdrkit_proto_ipcast::components::PlpPair;

/// The receiver settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RxSettings {
    /// The channel width (sets the elementary period).
    pub bandwidth: Bandwidth,
    /// The data PLP to decode. `None` takes the first data PLP.
    pub plp: Option<u8>,
    /// The most LDPC iterations per FEC block.
    pub max_iter: u32,
}

impl Default for RxSettings {
    fn default() -> Self {
        RxSettings {
            bandwidth: Bandwidth::B8,
            plp: None,
            max_iter: 50,
        }
    }
}

/// How far the receiver has come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Looking for P1.
    #[default]
    Searching,
    /// P1 found, L1 not decoded yet.
    P1,
    /// L1 decoded: frames are followed.
    Locked,
}

/// The counters and parameters the receiver publishes.
#[derive(Clone, Debug, Default)]
pub struct RxStatus {
    /// The stage.
    pub stage: Stage,
    /// The last P1 fields.
    pub p1: Option<P1Info>,
    /// The frame shape from L1-pre.
    pub shape: Option<FrameShape>,
    /// The profile.
    pub profile: Option<Profile>,
    /// The last L1-pre.
    pub pre: Option<L1Pre>,
    /// The carrier offset in Hz.
    pub offset_hz: f64,
    /// The length of one T2-frame (FEF parts not included), s.
    pub frame_length_s: Option<f64>,
    /// The SNR from the pilots, dB.
    pub snr_db: Option<f32>,
    /// T2-frames decoded.
    pub frames: u64,
    /// P1 symbols seen.
    pub p1_count: u64,
    /// L1-pre or L1-post failures.
    pub l1_errors: u64,
    /// The data PLP decoded.
    pub plp: Option<u8>,
    /// The common PLP decoded with it.
    pub common: Option<u8>,
    /// FEC blocks that the BCH decoder accepted.
    pub blocks_ok: u64,
    /// FEC blocks that failed.
    pub blocks_bad: u64,
    /// Bits the BCH decoder corrected.
    pub bch_corrected: u64,
    /// LDPC iterations of the last frame's blocks, mean.
    pub iterations: f32,
    /// A configuration that the receiver cannot follow.
    pub problem: Option<Problem>,
    /// Log lines since the last [`T2Receiver::take_log`].
    pub log: Vec<String>,
}

sdrkit_core::choice_enum! {
    /// A configuration of the multiplex that the receiver cannot follow.
    pub enum Problem {
        /// Time-frequency slicing (NUM_RF above 1).
        TimeFrequencySlicing = ("tfs", "Time-frequency slicing is not supported"),
        /// A frame shape that the demodulator cannot be set up for.
        FrameShape = ("frame_shape", "Frame shape not supported"),
        /// L1_POST_SIZE does not match the L1-post coding.
        PostSize = ("l1_post_size", "L1_POST_SIZE does not match the L1-post coding"),
        /// The requested PLP is not in the multiplex.
        NoSuchPlp = ("no_such_plp", "The PLP is not in this multiplex"),
        /// The multiplex has no data PLP.
        NoDataPlp = ("no_data_plp", "No data PLP in this multiplex"),
        /// A PLP uses a coding that the receiver does not know.
        UnknownCoding = ("unknown_coding", "A PLP uses an unknown coding"),
    }
}

/// One BBFRAME, descrambled.
#[derive(Clone, Copy, Debug)]
pub struct BbFrameOut<'a> {
    /// The PLP it belongs to.
    pub plp: u8,
    /// The K_bch / 8 bytes: BBHEADER, data field and padding.
    pub bytes: &'a [u8],
    /// The BCH decoder accepted the block.
    pub ok: bool,
    /// Signal time of the T2-frame that ended it, s.
    pub time: f64,
}

/// The geometry and the demodulators of one frame shape.
struct Phy {
    shape: FrameShape,
    maps: PilotMaps,
    framer: FftFramer,
    /// Estimators for P2, the normal phases and the frame closing symbol. MISO adds a second set for the inverted pilots.
    est: Vec<PilotLs>,
    est2: Vec<PilotLs>,
    freq: Option<FreqInterleaver>,
    cells: Vec<Complex32>,
    z: Vec<Complex32>,
    w: Vec<f32>,
    zd: Vec<Complex32>,
    wd: Vec<f32>,
    xd: Vec<Complex32>,
    xw: Vec<f32>,
    guard: usize,
    sym: usize,
}

impl Phy {
    fn new(shape: FrameShape) -> Result<Phy, T2Error> {
        let maps = PilotMaps::new(shape)?;
        let kt = shape.k_total();
        let first = -((kt as i32 - 1) / 2);
        let off = if shape.extended { 0 } else { shape.fft.k_ext() };
        let w = crate::pilots::pilot_prbs(shape.fft.k_total(true) + 1);
        let guard = shape.guard.samples(shape.fft).ok_or(Rule::GuardForFft)?;
        let mk = |maps: &[&SymbolMap],
                  phase_l: &[usize],
                  group: Option<bool>|
         -> Result<OfdmLayout, T2Error> {
            let cells = maps
                .iter()
                .zip(phase_l)
                .map(|(m, &l)| {
                    m.pilots
                        .iter()
                        .filter(|p| match group {
                            None => true,
                            Some(want_same) => {
                                let a = shape_pilot(&shape, p, l);
                                a == want_same
                            }
                        })
                        .map(|p| {
                            let v = p.amp * (1.0 - 2.0 * w[p.k as usize + off] as f32);
                            (first + p.k as i32, Complex32::new(v, 0.0))
                        })
                        .collect()
                })
                .collect();
            OfdmLayout::new(
                shape.fft.size(),
                &[guard],
                first,
                first + kt as i32 - 1,
                &[],
                &[],
                &Pattern { cells },
            )
            .map_err(T2Error::from)
        };
        let dy = shape.pp.dy();
        let normals: Vec<&SymbolMap> = (0..dy).map(|ph| maps.normal(ph)).collect();
        let phases: Vec<usize> = (0..dy).collect();
        let fc_l = shape.l_f() - 1;
        let groups: Vec<Option<bool>> = if shape.miso {
            vec![Some(true), Some(false)]
        } else {
            vec![None]
        };
        let mut sets = Vec::new();
        for g in &groups {
            let p2 = mk(&[maps.p2()], &[0], *g)?;
            let normal = mk(&normals, &phases, *g)?;
            let fc = mk(&[maps.symbol(fc_l)], &[fc_l], *g)?;
            sets.push(vec![
                PilotLs::new(&p2, false),
                PilotLs::new(&normal, false),
                PilotLs::new(&fc, false),
            ]);
        }
        let est = sets.remove(0);
        let est2 = sets.pop().unwrap_or_default();
        let layout0 = mk(&[maps.p2()], &[0], None)?;
        let backoff = (guard / 4).min(64);
        let framer = FftFramer::new(&layout0, backoff)?;
        let n = shape.fft.size();
        Ok(Phy {
            shape,
            maps,
            framer,
            est,
            est2,
            freq: None,
            cells: vec![Complex32::default(); kt],
            z: vec![Complex32::default(); kt],
            w: vec![0.0; kt],
            zd: vec![Complex32::default(); kt],
            wd: vec![0.0; kt],
            xd: vec![Complex32::default(); kt],
            xw: vec![0.0; kt],
            guard,
            sym: n + guard,
        })
    }
}

/// Whether MISO group 2 sends a pilot of symbol `l` with the same sign as group 1.
fn shape_pilot(shape: &FrameShape, p: &crate::pilots::PilotCell, l: usize) -> bool {
    let dx = shape.pp.dx() as i64;
    let rel = p.k as i64 - shape.kx() as i64;
    match p.kind {
        PilotKind::Scattered | PilotKind::FrameClosing | PilotKind::ContinualOnGrid => {
            !(rel.rem_euclid(dx) == 0 && rel.div_euclid(dx).rem_euclid(2) == 1)
        }
        PilotKind::Continual | PilotKind::P2Same => true,
        PilotKind::Edge => l.is_multiple_of(2),
        PilotKind::P2 => !(p.k.is_multiple_of(3) && (p.k / 3) % 2 == 1),
    }
}

/// One PLP being decoded.
struct PlpRx {
    id: u8,
    cfg: PlpConfig,
    coding: Coding,
    /// Cells of the Interleaving Frame so far.
    acc_z: Vec<Complex32>,
    acc_w: Vec<f32>,
    /// Frames of the current Interleaving Frame received.
    frames_in: usize,
    blocks: usize,
    /// (symbol, first data cell, count, offset in this frame's cells).
    segs: Vec<(usize, usize, usize, usize)>,
    frame_base: usize,
    frame_len: usize,
}

enum State {
    Search,
    /// A T2 P1 at `start`: try guards and carrier modes on P2 once the samples are in.
    Acquire {
        start: u64,
        info: P1Info,
        freq: f64,
    },
    /// Decoding the frame whose P1 starts at `start`. `l` is the next symbol.
    Frame {
        start: u64,
        l: usize,
        p2_done: bool,
    },
    /// Waiting for the P1 expected at `at`.
    AwaitP1 {
        at: u64,
    },
}

/// The DVB-T2 receiver. Feed it samples at the elementary period 1/T of the configured width.
pub struct T2Receiver {
    set: RxSettings,
    rate: f64,
    hist: Vec<Complex32>,
    base: u64,
    p1: P1Detector,
    found: Vec<P1Found>,
    state: State,
    phy: Option<Phy>,
    pre_codec: L1Codec,
    post_codec: Option<(usize, Modulation, bool, usize, L1Codec)>,
    p2_z: Vec<Complex32>,
    p2_w: Vec<f32>,
    l1_z: Vec<Complex32>,
    l1_w: Vec<f32>,
    l1_bits: Vec<u8>,
    pre: Option<L1Pre>,
    post: Option<L1Post>,
    /// The L1-post being read. It keeps the memory of the one before.
    post_buf: L1Post,
    /// The frame cells and the shape and L1-post size they were made for.
    fcells: Option<(FrameShape, usize, FrameCells)>,
    /// The cell address ranges of one PLP in the frame.
    ranges: Vec<(usize, usize)>,
    plps: Vec<PlpRx>,
    freq: f64,
    p1_info: Option<P1Info>,
    misses: u32,
    status: RxStatus,
    snr_acc: (f64, u32),
    frame_time: f64,
    /// The pool that decodes the FEC blocks of an Interleaving Frame (the output does not depend on it). `None`
    /// decodes in line.
    pub pool: Option<Pool>,
    fec: T2Fec,
    /// The Interleaving Frame that the FEC decodes in line.
    cells: IfCells,
    /// With a list, the Interleaving Frames go to it and the caller decodes them ([`T2Receiver::deferred`]).
    later: Option<Vec<IfCells>>,
    cell_pool: SlicePool<Complex32>,
    weight_pool: SlicePool<f32>,
}

impl T2Receiver {
    /// A receiver with these settings.
    pub fn new(set: RxSettings) -> T2Receiver {
        T2Receiver {
            set,
            rate: set.bandwidth.rate(),
            hist: Vec::new(),
            base: 0,
            p1: P1Detector::new(0.4, 60),
            found: Vec::new(),
            state: State::Search,
            phy: None,
            pre_codec: L1Codec::pre().unwrap_or_else(|_| unreachable!("the L1-pre code is fixed")),
            post_codec: None,
            p2_z: Vec::new(),
            p2_w: Vec::new(),
            l1_z: Vec::new(),
            l1_w: Vec::new(),
            l1_bits: Vec::new(),
            pre: None,
            post: None,
            post_buf: L1Post::default(),
            fcells: None,
            ranges: Vec::new(),
            plps: Vec::new(),
            freq: 0.0,
            p1_info: None,
            misses: 0,
            status: RxStatus::default(),
            snr_acc: (0.0, 0),
            frame_time: 0.0,
            pool: None,
            fec: T2Fec::new(set.max_iter),
            cells: IfCells::default(),
            later: None,
            cell_pool: SlicePool::default(),
            weight_pool: SlicePool::default(),
        }
    }

    /// A receiver that leaves the FEC of the PLPs to the caller: [`T2Receiver::take_cells`] has the Interleaving
    /// Frames, no BBFRAME reaches the callback of [`T2Receiver::feed`], and the status counts no FEC block.
    pub fn deferred(set: RxSettings) -> T2Receiver {
        T2Receiver {
            later: Some(Vec::new()),
            ..T2Receiver::new(set)
        }
    }

    /// Moves the Interleaving Frames that are complete since the last call to `to`, in the order of their end.
    pub fn take_cells(&mut self, to: &mut Vec<IfCells>) {
        if let Some(later) = self.later.as_mut() {
            to.append(later);
        }
    }

    /// The status.
    pub fn status(&self) -> &RxStatus {
        &self.status
    }

    /// The L1-post of the last T2-frame.
    pub fn post(&self) -> Option<&L1Post> {
        self.post.as_ref()
    }

    /// Takes the log lines written since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.status.log)
    }

    /// The settings.
    pub fn settings(&self) -> RxSettings {
        self.set
    }

    /// Chooses another data PLP (`None` for the first). The next frame uses it.
    pub fn set_plp(&mut self, plp: Option<u8>) {
        self.set.plp = plp;
        self.plps.clear();
    }

    /// Starts again from the P1 search, keeping the settings.
    pub fn reset(&mut self) {
        let set = self.set;
        *self = if self.later.is_some() {
            T2Receiver::deferred(set)
        } else {
            T2Receiver::new(set)
        };
    }

    fn log(&mut self, s: String) {
        self.status.log.push(s);
    }

    fn end(&self) -> u64 {
        self.base + self.hist.len() as u64
    }

    fn slice(&self, from: u64, len: usize) -> Option<&[Complex32]> {
        let a = from.checked_sub(self.base)? as usize;
        self.hist.get(a..a + len)
    }

    /// Feeds samples at the elementary period. BBFRAMEs of the decoded PLPs go to `out`.
    pub fn feed(&mut self, x: &[Complex32], out: &mut impl FnMut(BbFrameOut<'_>)) {
        let found = &mut self.found;
        self.p1.process(x, |f| found.push(f));
        self.status.p1_count += self.found.len() as u64;
        self.hist.extend(x.iter().map(|&s| {
            if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            }
        }));
        loop {
            if !self.step(out) {
                break;
            }
        }
        self.trim();
    }

    fn keep_from(&self) -> u64 {
        match self.state {
            State::Search => self.end().saturating_sub(4 * P1_LEN as u64),
            State::Acquire { start, .. } => start,
            State::Frame { start, l, .. } => {
                let s = self.phy.as_ref().map(|p| p.sym).unwrap_or(0) as u64;
                start + P1_LEN as u64 + l as u64 * s
            }
            State::AwaitP1 { at } => at.saturating_sub(8192),
        }
    }

    fn trim(&mut self) {
        let keep = self.keep_from().min(self.end());
        if keep > self.base + (1 << 16) {
            let n = (keep - self.base) as usize;
            self.hist.drain(..n);
            self.base = keep;
        }
    }

    /// One step of the state machine. False when it needs more samples.
    fn step(&mut self, out: &mut impl FnMut(BbFrameOut<'_>)) -> bool {
        match self.state {
            State::Search => {
                let hit = self
                    .found
                    .iter()
                    .position(|f| f.info.is_some_and(|i| i.is_t2() && i.fft().is_some()));
                let f = match hit {
                    Some(i) => {
                        let f = self.found[i];
                        self.found.drain(..=i);
                        Some(f)
                    }
                    None => {
                        self.found.clear();
                        None
                    }
                };
                match f {
                    Some(f) => {
                        let info = f.info.unwrap_or(P1Info { s1: 0, s2: 0 });
                        self.status.stage = Stage::P1;
                        self.status.p1 = Some(info);
                        self.freq = f.freq;
                        self.log(format!(
                            "P1 found: S1 {} S2 {} ({:.0} Hz offset)",
                            info.s1,
                            info.s2,
                            f.freq * self.rate
                        ));
                        self.state = State::Acquire {
                            start: f.start,
                            info,
                            freq: f.freq,
                        };
                        true
                    }
                    None => false,
                }
            }
            State::Acquire { start, info, freq } => {
                let Some((fft, guards)) = info.fft() else {
                    self.state = State::Search;
                    return true;
                };
                let n = fft.size();
                let gmax = guards
                    .iter()
                    .filter_map(|g| g.samples(fft))
                    .max()
                    .unwrap_or(0);
                let need = start + P1_LEN as u64 + (fft.n_p2() * (n + gmax)) as u64 + 64;
                if self.end() < need {
                    return false;
                }
                self.found.retain(|f| f.start > start + P1_LEN as u64);
                self.freq = freq;
                if self.acquire(start, info) {
                    self.state = State::Frame {
                        start,
                        l: 0,
                        p2_done: true,
                    };
                } else {
                    self.status.l1_errors += 1;
                    self.log("L1-pre not decoded after P1; searching again".into());
                    self.state = State::Search;
                }
                true
            }
            State::Frame { start, l, p2_done } => self.frame_step(start, l, p2_done, out),
            State::AwaitP1 { at } => {
                if self.end() < at + P1_LEN as u64 + 700 {
                    return false;
                }
                let tol = self.phy.as_ref().map(|p| p.guard as u64 + 64).unwrap_or(64);
                let near = self
                    .found
                    .iter()
                    .filter(|f| {
                        f.start + tol >= at
                            && f.start <= at + tol
                            && f.info.is_some_and(|i| i.is_t2())
                    })
                    .min_by_key(|f| f.start.abs_diff(at))
                    .copied();
                self.found.retain(|f| f.start > at + tol);
                let start = match near {
                    Some(f) => {
                        self.misses = 0;
                        f.start
                    }
                    None => {
                        self.misses += 1;
                        if self.misses > 3 {
                            self.log("P1 lost; searching again".into());
                            self.status.stage = Stage::Searching;
                            self.state = State::Search;
                            return true;
                        }
                        at
                    }
                };
                self.state = State::Frame {
                    start,
                    l: 0,
                    p2_done: false,
                };
                true
            }
        }
    }

    /// Tries each allowed guard (best cyclic prefix correlation first) and each carrier mode on the P2 symbols after P1 until
    /// L1-pre decodes.
    fn acquire(&mut self, start: u64, info: P1Info) -> bool {
        let Some((fft, guards)) = info.fft() else {
            return false;
        };
        let n = fft.size();
        let p2 = start + P1_LEN as u64;
        let mut scored: Vec<(f32, Guard)> = guards
            .iter()
            .filter_map(|&g| {
                let gl = g.samples(fft)?;
                let x = self.slice(p2, gl + n)?;
                let (mut c, mut e) = (Complex32::default(), 0f32);
                for i in 0..gl {
                    c += x[i] * x[i + n].conj();
                    e += 0.5 * (x[i].norm_sqr() + x[i + n].norm_sqr());
                }
                Some(((c.norm() / e.max(1e-12)) * (gl as f32).sqrt(), g))
            })
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let modes: &[bool] = if fft.has_extended() {
            &[false, true]
        } else {
            &[false]
        };
        for &(_, guard) in &scored {
            for &extended in modes {
                let probe = FrameShape {
                    fft,
                    extended,
                    pp: PilotPattern::Pp1,
                    guard,
                    miso: info.miso(),
                    tr: false,
                    l_data: if fft == crate::params::Fft::K32 { 9 } else { 8 },
                };
                if let Ok(mut phy) = Phy::new(probe) {
                    phy.framer.set_freq(self.freq);
                    if self.p2_symbols(&mut phy, start) {
                        if let Some(pre) = self.decode_pre(&phy) {
                            self.log(format!(
                                "L1-pre decoded: {} {}{} {} {}",
                                fft.name(),
                                guard.name(),
                                if extended { " extended" } else { "" },
                                PilotPattern::from_code(pre.pilot_pattern)
                                    .map(|p| p.name())
                                    .unwrap_or("?"),
                                if info.miso() { "MISO" } else { "SISO" }
                            ));
                            return self.adopt(pre, info, start);
                        }
                    }
                }
            }
        }
        false
    }

    /// Demodulates and equalises the P2 symbols of the frame at `start` into `p2_z`, `p2_w` (frequency de-interleaved).
    fn p2_symbols(&mut self, phy: &mut Phy, start: u64) -> bool {
        let n_p2 = phy.shape.n_p2();
        let c = phy.maps.p2().data.len();
        self.p2_z.resize(n_p2 * c, Complex32::default());
        self.p2_w.resize(n_p2 * c, 0.0);
        if phy.freq.is_none() {
            match FreqInterleaver::new(phy.shape.fft, &[c]) {
                Ok(f) => phy.freq = Some(f),
                Err(_) => return false,
            }
        }
        for l in 0..n_p2 {
            if !self.symbol(phy, start, l) {
                return false;
            }
            self.p2_z[l * c..(l + 1) * c].copy_from_slice(&phy.xd[..c]);
            self.p2_w[l * c..(l + 1) * c].copy_from_slice(&phy.xw[..c]);
        }
        true
    }

    /// Demodulates symbol `l` of the frame at `start`: FFT, pilot sign, channel estimate, equalisation (Alamouti for MISO),
    /// data cells in carrier order, then frequency de-interleaving into `phy.xd`, `phy.xw`.
    fn symbol(&mut self, phy: &mut Phy, start: u64, l: usize) -> bool {
        let sh = phy.shape;
        let n = sh.fft.size();
        let s0 = start + P1_LEN as u64 + (l * phy.sym) as u64;
        let backoff = phy.framer.backoff();
        let Some(x) = self.slice(s0, phy.sym) else {
            return false;
        };
        // Carrier tracking from the cyclic prefix (it repeats the end of the symbol).
        let mut cp = Complex32::default();
        for i in backoff..phy.guard {
            cp += x[i] * x[i + n].conj();
        }
        let x0 = s0;
        if phy
            .framer
            .demodulate(x, phy.guard, x0, &mut phy.cells)
            .is_err()
        {
            return false;
        }
        if cp.norm() > 0.0 {
            // The prefix phase holds the whole offset. Remove the part already corrected to keep the residual.
            let turn = 2.0 * std::f64::consts::PI * self.freq * n as f64;
            let cp = cp * Complex32::new(turn.cos() as f32, turn.sin() as f32);
            let eps = -(cp.arg() as f64) / (2.0 * std::f64::consts::PI * n as f64);
            self.freq += 0.05 * eps;
            phy.framer.set_freq(self.freq);
        }
        // The estimators hold the pilots without the frame-level PN sign. They see the symbol with that sign removed.
        let flip = crate::pilots::frame_pn(l) == 1;
        if flip {
            phy.cells.iter_mut().for_each(|c| *c = -*c);
        }
        let kind = sh.kind(l);
        let (ei, sym_no) = match kind {
            SymbolKind::P2 => (0, 0),
            SymbolKind::Normal => (1, (l % sh.pp.dy()) as u64),
            SymbolKind::FrameClosing => (2, 0),
        };
        let map = phy.maps.symbol(l);
        let nd = map.data.len();
        if phy.est[ei].push(sym_no, &phy.cells).is_err() {
            return false;
        }
        let est2_ok =
            !sh.miso || phy.est2.is_empty() || phy.est2[ei].push(sym_no, &phy.cells).is_ok();
        if flip {
            phy.cells.iter_mut().for_each(|c| *c = -*c);
        }
        if !est2_ok {
            return false;
        }
        let h1 = phy.est[ei].h();
        let mean_h = h1.iter().map(|v| v.norm_sqr()).sum::<f32>() / h1.len().max(1) as f32;
        let noise = phy.est[ei].noise().max(mean_h * 1e-5).max(1e-20);
        self.snr_acc.0 += (mean_h / noise) as f64;
        self.snr_acc.1 += 1;
        if sh.miso && !phy.est2.is_empty() {
            let s = phy.est[ei].h();
            let d = phy.est2[ei].h();
            for p in (0..nd).step_by(2) {
                let (k0, k1) = (map.data[p] as usize, map.data[(p + 1).min(nd - 1)] as usize);
                let hs = (s[k0] + s[k1]) * 0.5;
                let hd = (d[k0] + d[k1]) * 0.5;
                let g1 = (hs + hd) * 0.5;
                let g2 = (hs - hd) * 0.5;
                let (r0, r1) = (phy.cells[k0], phy.cells[k1]);
                let e = g1.norm_sqr() + g2.norm_sqr();
                if e <= 0.0 {
                    phy.zd[p] = Complex32::default();
                    phy.wd[p] = 0.0;
                    continue;
                }
                let a0 = (g1.conj() * r0 + g2 * r1.conj()) / e;
                let a1 = (g1.conj() * r1 - g2 * r0.conj()) / e;
                phy.zd[p] = a0;
                phy.wd[p] = e / noise;
                if p + 1 < nd {
                    phy.zd[p + 1] = a1;
                    phy.wd[p + 1] = e / noise;
                }
            }
        } else {
            if equalise(&phy.cells, h1, noise, &mut phy.z, &mut phy.w).is_err() {
                return false;
            }
            for (p, &k) in map.data.iter().enumerate() {
                phy.zd[p] = phy.z[k as usize];
                phy.wd[p] = phy.w[k as usize];
            }
        }
        let Some(fi) = phy.freq.as_ref().and_then(|f| f.for_symbol(l, nd)) else {
            return false;
        };
        if fi.invert(&phy.zd[..nd], &mut phy.xd[..nd]).is_err()
            || fi.invert(&phy.wd[..nd], &mut phy.xw[..nd]).is_err()
        {
            return false;
        }
        true
    }

    fn decode_pre(&mut self, phy: &Phy) -> Option<L1Pre> {
        let n_p2 = phy.shape.n_p2();
        let c = phy.maps.p2().data.len();
        self.l1_z.resize(L1_PRE_CELLS, Complex32::default());
        self.l1_w.resize(L1_PRE_CELLS, 0.0);
        for i in 0..L1_PRE_CELLS {
            let (l, p) = (i % n_p2, i / n_p2);
            self.l1_z[i] = self.p2_z[l * c + p];
            self.l1_w[i] = self.p2_w[l * c + p];
        }
        self.l1_bits.resize(L1_PRE_BITS, 0);
        let ok = self
            .pre_codec
            .decode(&self.l1_z, &self.l1_w, &mut self.l1_bits)
            .ok()?;
        if !ok {
            return None;
        }
        L1Pre::from_bits(&self.l1_bits)
    }

    /// Takes a decoded L1-pre: builds the demodulator of its frame shape and decodes L1-post of this frame.
    fn adopt(&mut self, pre: L1Pre, info: P1Info, start: u64) -> bool {
        let Some((fft, _)) = info.fft() else {
            return false;
        };
        let (Some(guard), Some(pp)) = (
            Guard::from_code(pre.guard),
            PilotPattern::from_code(pre.pilot_pattern),
        ) else {
            return false;
        };
        if pre.num_rf > 1 {
            self.status.problem = Some(Problem::TimeFrequencySlicing);
        }
        let shape = FrameShape {
            fft,
            extended: pre.bwt_ext,
            pp,
            guard,
            miso: info.miso(),
            tr: pre.tone_reservation(),
            l_data: pre.num_data_symbols as usize,
        };
        if self.phy.as_ref().map(|p| p.shape) != Some(shape) {
            let mut phy = match Phy::new(shape) {
                Ok(p) => p,
                Err(_) => {
                    self.status.problem = Some(Problem::FrameShape);
                    return false;
                }
            };
            let counts: Vec<usize> = (0..shape.l_f())
                .map(|l| phy.maps.symbol(l).data.len())
                .collect();
            phy.freq = FreqInterleaver::new(fft, &counts).ok();
            phy.framer.set_freq(self.freq);
            self.phy = Some(phy);
            self.plps.clear();
        }
        self.status.shape = Some(shape);
        self.status.frame_length_s = shape
            .symbol_len()
            .map(|s| (P1_LEN + shape.l_f() * s) as f64 / self.rate);
        self.status.profile = info.profile();
        self.p1_info = Some(info);
        // The P2 symbols again with the right shape (tone reservation and pattern do not change P2, the carrier mode does).
        let mut phy = self.phy.take();
        let ok = phy.as_mut().is_some_and(|p| self.p2_symbols(p, start));
        self.phy = phy;
        if !ok {
            return false;
        }
        self.pre = Some(pre);
        self.status.pre = Some(pre);
        self.decode_post(start)
    }

    /// Decodes L1-post from the stored P2 cells, then lays out the PLPs of this frame.
    fn decode_post(&mut self, _start: u64) -> bool {
        let (Some(pre), Some(phy)) = (self.pre, self.phy.as_ref()) else {
            return false;
        };
        let n_p2 = phy.shape.n_p2();
        let c = phy.maps.p2().data.len();
        let Some(m) = Modulation::from_l1_code(pre.l1_mod) else {
            return false;
        };
        let info = pre.l1_post_info_size as usize;
        let Ok(sizes) = post_sizes(info, m, n_p2) else {
            return false;
        };
        if sizes.cells != pre.l1_post_size as usize {
            self.status.problem = Some(Problem::PostSize);
            return false;
        }
        let key = (info, m, pre.post_scrambled, n_p2);
        let fresh = self.post_codec.as_ref().map(|p| (p.0, p.1, p.2, p.3)) != Some(key);
        if fresh {
            match L1Codec::post(&sizes, m, pre.post_scrambled) {
                Ok(codec) => self.post_codec = Some((info, m, pre.post_scrambled, n_p2, codec)),
                Err(_) => return false,
            }
        }
        let Some((.., codec)) = self.post_codec.as_mut() else {
            return false;
        };
        let per = codec.cells();
        let base = L1_PRE_CELLS / n_p2;
        self.l1_bits.resize(sizes.k_sig * sizes.blocks, 0);
        self.l1_z.resize(per, Complex32::default());
        self.l1_w.resize(per, 0.0);
        let mut all_ok = true;
        for b in 0..sizes.blocks {
            for j in 0..per {
                let i = b * per + j;
                let (l, p) = (i % n_p2, base + i / n_p2);
                self.l1_z[j] = self.p2_z[l * c + p];
                self.l1_w[j] = self.p2_w[l * c + p];
            }
            let k = sizes.k_sig;
            all_ok &= codec
                .decode(
                    &self.l1_z[..per],
                    &self.l1_w[..per],
                    &mut self.l1_bits[b * k..(b + 1) * k],
                )
                .unwrap_or(false);
        }
        if all_ok && self.post_buf.read(&self.l1_bits, &pre) {
            match self.post.as_mut() {
                Some(p) => std::mem::swap(p, &mut self.post_buf),
                None => self.post = Some(std::mem::take(&mut self.post_buf)),
            }
        } else {
            self.status.l1_errors += 1;
            // L1 repetition: the previous frame carried this frame's dynamic part.
            let Some(old) = self.post.as_mut() else {
                return false;
            };
            let Some(mut next) = old.next.take() else {
                return false;
            };
            std::mem::swap(&mut old.dynamic, &mut next);
        }
        let Some(phy) = self.phy.as_ref() else {
            return false;
        };
        if self
            .fcells
            .as_ref()
            .is_none_or(|(shape, cells, _)| (*shape, *cells) != (phy.shape, sizes.cells))
        {
            let Ok(fc) = FrameCells::new(&phy.maps, sizes.cells) else {
                self.fcells = None;
                return false;
            };
            self.fcells = Some((phy.shape, sizes.cells, fc));
        }
        self.status.stage = Stage::Locked;
        self.layout_plps();
        // PLP cells in the P2 symbols.
        for l in 0..n_p2 {
            route(
                &mut self.plps,
                l,
                &self.p2_z[l * c..(l + 1) * c],
                &self.p2_w[l * c..(l + 1) * c],
            );
        }
        true
    }

    /// Chooses the PLPs to decode and maps their slices of this frame onto symbols.
    fn layout_plps(&mut self) {
        let (Some(_), Some(post), Some(fc)) = (
            self.pre,
            self.post.as_ref(),
            self.fcells.as_ref().map(|f| &f.2),
        ) else {
            return;
        };
        let lite = self.p1_info.and_then(|i| i.profile()) == Some(Profile::Lite);
        let data = post
            .config
            .plps
            .iter()
            .find(|p| matches!(p.plp_type, 1 | 2) && self.set.plp.is_none_or(|id| id == p.plp_id))
            .copied();
        let Some(data) = data else {
            if self.status.problem.is_none() {
                self.status.problem = Some(match self.set.plp {
                    Some(_) => Problem::NoSuchPlp,
                    None => Problem::NoDataPlp,
                });
            }
            return;
        };
        let common = post
            .config
            .plps
            .iter()
            .find(|p| p.plp_type == 0 && p.group_id == data.group_id)
            .copied();
        self.status.plp = Some(data.plp_id);
        self.status.common = common.map(|c| c.plp_id);
        let want = [common, Some(data)];
        if !self
            .plps
            .iter()
            .map(|p| p.cfg)
            .eq(want.iter().flatten().copied())
        {
            self.plps.clear();
            for cfg in want.into_iter().flatten() {
                let (Some(rate), Some(m), Some(frame)) = (
                    CodeRate::from_code(cfg.cod, lite),
                    Modulation::from_plp_code(cfg.modulation),
                    FecFrame::from_code(cfg.fec_type),
                ) else {
                    self.status.problem = Some(Problem::UnknownCoding);
                    continue;
                };
                let coding = Coding {
                    frame,
                    rate,
                    modulation: m,
                    rotated: cfg.rotation,
                };
                if !supported(coding) {
                    continue;
                }
                self.plps.push(PlpRx {
                    id: cfg.plp_id,
                    cfg,
                    coding,
                    acc_z: Vec::new(),
                    acc_w: Vec::new(),
                    frames_in: 0,
                    blocks: 0,
                    segs: Vec::new(),
                    frame_base: 0,
                    frame_len: 0,
                });
            }
        }
        let frame_idx = post.dynamic.frame_idx;
        let ns = post.config.sub_slices_per_frame.max(1) as usize;
        let ssi = post.dynamic.sub_slice_interval as usize;
        for p in &mut self.plps {
            p.segs.clear();
            p.frame_len = 0;
            if !plp_in_frame(&p.cfg, frame_idx) {
                continue;
            }
            let Some(d) = post.dynamic.plps.iter().find(|d| d.plp_id == p.id).copied() else {
                continue;
            };
            // A new Interleaving Frame starts where (FRAME_IDX - FIRST_FRAME_IDX) / I_JUMP is a multiple of P_I.
            let pi = p.cfg.p_i();
            let k =
                (frame_idx - p.cfg.first_frame_idx) as usize / p.cfg.frame_interval.max(1) as usize;
            if k.is_multiple_of(pi) {
                p.acc_z.clear();
                p.acc_w.clear();
                p.frames_in = 0;
            } else if p.frames_in == 0 {
                // Joined in the middle of an Interleaving Frame: wait for the next one.
                continue;
            }
            p.blocks = d.num_blocks as usize;
            plp_ranges(&p.cfg, &d, p.coding.cells(), ns, ssi, &mut self.ranges);
            p.frame_base = p.acc_z.len();
            let mut off = 0;
            for &(a, len) in &self.ranges {
                let mut a = a;
                let mut left = len;
                while left > 0 {
                    let Some((l, cell)) = fc.locate(a) else { break };
                    let avail = fc.active[l] - cell;
                    let take = avail.min(left);
                    p.segs.push((l, cell, take, off));
                    off += take;
                    a += take;
                    left -= take;
                }
            }
            p.frame_len = off;
            let new_len = p.acc_z.len() + off;
            p.acc_z.resize(new_len, Complex32::default());
            p.acc_w.resize(new_len, 0.0);
        }
    }

    fn frame_step(
        &mut self,
        start: u64,
        l: usize,
        p2_done: bool,
        out: &mut impl FnMut(BbFrameOut<'_>),
    ) -> bool {
        let Some(mut phy) = self.phy.take() else {
            self.state = State::Search;
            return true;
        };
        let n_p2 = phy.shape.n_p2();
        let l_f = phy.shape.l_f();
        let mut l = l;
        let mut more = true;
        if l == 0 {
            // P2: needs all P2 symbols, then L1.
            let need = start + P1_LEN as u64 + (n_p2 * phy.sym) as u64 + 64;
            if self.end() < need {
                self.phy = Some(phy);
                return false;
            }
            self.frame_time = start as f64 / self.rate;
            if self.pre.is_some() && !p2_done {
                let ok = self.p2_symbols(&mut phy, start);
                self.phy = Some(phy);
                let pre = match (ok, self.phy.take()) {
                    (true, Some(p)) => {
                        let r = self.decode_pre(&p);
                        self.phy = Some(p);
                        r
                    }
                    (_, p) => {
                        self.phy = p;
                        None
                    }
                };
                match pre {
                    Some(pre)
                        if Some(pre) == self.pre
                            || self.pre.is_some_and(|o| same_shape(&o, &pre)) =>
                    {
                        self.pre = Some(pre);
                        self.status.pre = Some(pre);
                        if !self.decode_post(start) {
                            self.skip_frame(start);
                            return true;
                        }
                    }
                    Some(pre) => {
                        // The configuration changed: rebuild everything.
                        let info = self.p1_info.unwrap_or(P1Info { s1: 0, s2: 0 });
                        if !self.adopt(pre, info, start) {
                            self.skip_frame(start);
                            return true;
                        }
                    }
                    None => {
                        self.status.l1_errors += 1;
                        if !self.decode_post(start) {
                            self.skip_frame(start);
                            return true;
                        }
                    }
                }
            } else {
                self.phy = Some(phy);
            }
            let Some(p) = self.phy.take() else {
                return true;
            };
            phy = p;
            l = n_p2;
        }
        while l < l_f {
            let s0 = start + P1_LEN as u64 + (l * phy.sym) as u64;
            if self.end() < s0 + phy.sym as u64 {
                more = false;
                break;
            }
            if self.symbol(&mut phy, start, l) {
                let nd = phy.maps.symbol(l).data.len();
                route(&mut self.plps, l, &phy.xd[..nd], &phy.xw[..nd]);
            }
            l += 1;
        }
        let sym = phy.sym;
        self.phy = Some(phy);
        if l < l_f {
            self.state = State::Frame {
                start,
                l,
                p2_done: true,
            };
            return more;
        }
        // The frame is complete.
        self.status.frames += 1;
        if self.snr_acc.1 > 0 {
            self.status.snr_db =
                Some((10.0 * (self.snr_acc.0 / self.snr_acc.1 as f64).log10()) as f32);
            self.snr_acc = (0.0, 0);
        }
        self.status.offset_hz = self.freq * self.rate;
        self.finish_frame(out);
        let mut next = start + P1_LEN as u64 + (l_f * sym) as u64;
        if let (Some(post), Some(_)) = (self.post.as_ref(), self.pre) {
            if let Some((_, len, iv)) = post.config.fef {
                let idx = post.dynamic.frame_idx as usize;
                let len = len as u64 | ((post.config.fef_length_msb as u64) << 22);
                if iv > 0 && (idx + 1).is_multiple_of(iv as usize) {
                    next += len;
                }
            }
        }
        self.state = State::AwaitP1 { at: next };
        true
    }

    fn skip_frame(&mut self, start: u64) {
        let (sym, l_f) = self
            .phy
            .as_ref()
            .map(|p| (p.sym, p.shape.l_f()))
            .unwrap_or((0, 0));
        self.state = State::AwaitP1 {
            at: start + P1_LEN as u64 + (l_f * sym) as u64,
        };
    }

    /// Time and cell de-interleaving and FEC of every Interleaving Frame that is complete.
    fn finish_frame(&mut self, out: &mut impl FnMut(BbFrameOut<'_>)) {
        let time = self.frame_time;
        let pair = PlpPair {
            plp: self.status.plp,
            common: self.status.common,
        };
        self.fec.counts.start_mean();
        for p in &mut self.plps {
            if p.frame_len == 0 {
                continue;
            }
            p.frames_in += 1;
            if p.frames_in < p.cfg.p_i() {
                continue;
            }
            let total = p.blocks * p.coding.cells();
            if p.acc_z.len() >= total && total > 0 {
                let c = &mut self.cells;
                (c.plp, c.coding, c.n_ti) = (p.id, Some(p.coding), p.cfg.n_ti());
                (c.blocks, c.time, c.pair) = (p.blocks, time, pair);
                c.z = self
                    .cell_pool
                    .at_least(total, |s| s.copy_from_slice(&p.acc_z[..total]));
                c.w = self
                    .weight_pool
                    .at_least(total, |s| s.copy_from_slice(&p.acc_w[..total]));
                match self.later.as_mut() {
                    Some(later) => later.push(c.clone()),
                    None => self.fec.decode(c, self.pool.as_ref(), out),
                }
            }
            p.acc_z.clear();
            p.acc_w.clear();
            p.frames_in = 0;
        }
        let n = self.fec.counts;
        (self.status.blocks_ok, self.status.blocks_bad) = (n.ok, n.bad);
        self.status.bch_corrected = n.corrected;
        if let Some(mean) = n.mean_iterations() {
            self.status.iterations = mean;
        }
    }
}

/// Copies the cells of symbol `l` that belong to the decoded PLPs.
fn route(plps: &mut [PlpRx], l: usize, z: &[Complex32], w: &[f32]) {
    for p in plps.iter_mut() {
        for &(sl, cell, n, off) in &p.segs {
            if sl == l {
                let at = p.frame_base + off;
                p.acc_z[at..at + n].copy_from_slice(&z[cell..cell + n]);
                p.acc_w[at..at + n].copy_from_slice(&w[cell..cell + n]);
            }
        }
    }
}

fn same_shape(a: &L1Pre, b: &L1Pre) -> bool {
    (
        a.guard,
        a.pilot_pattern,
        a.bwt_ext,
        a.num_data_symbols,
        a.papr,
        a.l1_mod,
        a.l1_post_size,
        a.l1_post_info_size,
    ) == (
        b.guard,
        b.pilot_pattern,
        b.bwt_ext,
        b.num_data_symbols,
        b.papr,
        b.l1_mod,
        b.l1_post_size,
        b.l1_post_info_size,
    )
}
