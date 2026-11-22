//! The DVB-T2 transmitter of the generator and of the sender: mode adaptation of transport streams, BICM, time interleaving,
//! L1 signalling, the frame builder, frequency interleaving, MISO, pilots, OFDM and P1, at the elementary period T. It makes
//! one T2-frame at a time ([`Transmitter::frame`]) or one symbol at a time ([`Transmitter::part`]). [`plan_frame`] gives the
//! frame of a sender with one data PLP.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 5 to 9.

use crate::bicm::{Coding, Encoder};
use crate::error::{Rule, T2Error};
use crate::frame::{dummy_cells, plp_in_frame, FrameCells};
use crate::interleave::{time_interleave, CellInterleaver, FreqInterleaver};
use crate::l1::{
    post_sizes, L1Codec, L1Post, L1Pre, PlpConfig, PlpDynamic, PostConfig, PostDynamic, PostSizes,
};
use crate::p1;
use crate::params::{allowed, k_bch, Bandwidth, Fft, Modulation, P1Info, Profile};
use crate::pilots::{FrameShape, PilotMaps, SymbolKind};
use sdrkit_algo_dsp::fft::Fft as DspFft;
use sdrkit_core::Complex32;
use sdrkit_proto_ipcast::adapt::TsFramer;
use sdrkit_proto_ipcast::bbheader::{bb_descramble, BbHeader, BBHEADER_LEN};
use sdrkit_proto_mpegts::source::PacketSource;
use sdrkit_util_bits::pack::unpack_into;
use sdrkit_util_bits::BitOrder;

/// The type of a PLP (table 30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlpKind {
    /// Common PLP.
    Common,
    /// Data PLP type 1 (one slice per frame).
    Type1,
    /// Data PLP type 2 (sub-slices).
    Type2,
}

impl PlpKind {
    fn code(self) -> u8 {
        match self {
            PlpKind::Common => 0,
            PlpKind::Type1 => 1,
            PlpKind::Type2 => 2,
        }
    }
}

/// The time interleaving of a PLP (clause 6.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeIl {
    /// No time interleaving (TIME_IL_LENGTH 0, one PLP only).
    Off,
    /// One Interleaving Frame per T2-frame in N_TI TI-blocks (TIME_IL_TYPE 0).
    Blocks(u8),
    /// One TI-block spread over P_I T2-frames (TIME_IL_TYPE 1).
    Frames(u8),
}

/// One PLP of the transmitter.
pub struct PlpSpec {
    /// PLP_ID.
    pub id: u8,
    /// The type.
    pub kind: PlpKind,
    /// PLP_GROUP_ID.
    pub group: u8,
    /// Coding and modulation.
    pub coding: Coding,
    /// FEC blocks per Interleaving Frame.
    pub blocks: u16,
    /// Time interleaving.
    pub time_il: TimeIl,
    /// The transport packets.
    pub source: Box<dyn PacketSource>,
}

/// The transmitter configuration.
pub struct TxConfig {
    /// Channel width.
    pub bandwidth: Bandwidth,
    /// The frame shape. `l_data` 0 selects the fewest data symbols that hold the PLPs and, in T2-Lite, keep each data PLP
    /// within [`LITE_MOST_BIT_RATE`].
    pub shape: FrameShape,
    /// Profile.
    pub profile: Profile,
    /// L1-post constellation (BPSK to 64-QAM).
    pub l1_mod: Modulation,
    /// NUM_T2_FRAMES (at least 2).
    pub num_t2_frames: u8,
    /// The PLPs: common PLPs first, then type 1, then type 2.
    pub plps: Vec<PlpSpec>,
    /// SUB_SLICES_PER_FRAME for the type 2 PLPs.
    pub sub_slices: u16,
    /// CELL_ID.
    pub cell_id: u16,
    /// NETWORK_ID.
    pub network_id: u16,
    /// T2_SYSTEM_ID.
    pub t2_system_id: u16,
    /// L1_REPETITION_FLAG.
    pub repetition: bool,
    /// L1_POST_SCRAMBLED.
    pub post_scrambled: bool,
    /// The signal of a MISO frame shape. Not read in SISO.
    pub miso: MisoOut,
    /// A FEF part of this many samples after every `fef_interval` T2-frames (a P1 of the non-T2 type, then silence).
    pub fef: Option<(u32, u8)>,
}

/// The path from one transmitter group to the receiving antenna: a complex gain and a delay in elementary periods T.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Path {
    /// The gain.
    pub gain: Complex32,
    /// The delay, samples at 1/T.
    pub delay: usize,
}

/// What a transmitter of a MISO frame shape sends (clause 9.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MisoOut {
    /// The signal of transmitter group 1.
    Group1,
    /// The signal of transmitter group 2: the modified Alamouti cells and the group 2 pilots.
    Group2,
    /// What one antenna receives: group 1 through the first path plus group 2 through the second.
    Sum([Path; 2]),
}

/// The packets that a caller gives in place of the source of the first PLP.
type Feed<'a, 'b> = Option<&'a mut (dyn PacketSource + 'b)>;

/// An Interleaving Frame in the making.
#[derive(Default)]
struct IfBuild {
    /// Its cells, of the TI-blocks made.
    out: Vec<Complex32>,
    /// The cells of the TI-block in the making, before time interleaving.
    cells: Vec<Complex32>,
    /// The TI-block in the making.
    s: usize,
    /// The FEC blocks of it made.
    r: usize,
    /// The FEC blocks of the Interleaving Frame made.
    made: usize,
}

struct PlpState {
    spec: PlpSpec,
    enc: Encoder,
    ci: CellInterleaver,
    framer: TsFramer,
    /// Cells of the current Interleaving Frame still to send (time interleaving over several frames).
    pending: Vec<Complex32>,
    pending_at: usize,
    next: IfBuild,
    bb: Vec<u8>,
    bits: Vec<u8>,
    block: Vec<Complex32>,
    multi: bool,
}

/// The transmitter. Each call of [`Transmitter::frame`] returns one T2-frame (and its FEF part, if any) at the elementary
/// period; [`Transmitter::part`] makes the same samples one symbol at a time.
pub struct Transmitter {
    cfg_bw: Bandwidth,
    shape: FrameShape,
    plps: Vec<PlpState>,
    maps: PilotMaps,
    cells: FrameCells,
    freq: FreqInterleaver,
    pre: L1Pre,
    post: L1Post,
    post_sizes: PostSizes,
    pre_codec: L1Codec,
    post_codec: L1Codec,
    num_t2_frames: u8,
    frame_count: u64,
    sub_slices: usize,
    profile: Profile,
    miso: MisoOut,
    fef: Option<(u32, u8)>,
    ifft: DspFft,
    /// The part that [`Transmitter::part`] makes next: 0 the P1 symbol, `l + 1` symbol `l`.
    at: usize,
    ahead: bool,
    /// The PLP cells, the L1-pre cells and the L1-post cells of the frame in the making.
    flat: Vec<Complex32>,
    pre_cells: Vec<Complex32>,
    post_cells: Vec<Complex32>,
    x: Vec<Complex32>,
    a: Vec<Complex32>,
    c1: Vec<Complex32>,
    c2: Vec<Complex32>,
    /// With [`MisoOut::Sum`]: the group 2 samples of the part in the making, the group 1 samples of it, and the last
    /// samples of each group for the delays of the paths.
    g2: Vec<Complex32>,
    g1: Vec<Complex32>,
    lines: [Vec<Complex32>; 2],
}

/// The most bits a second that a data PLP of T2-Lite with its common PLP carries (annex I.10).
pub const LITE_MOST_BIT_RATE: f64 = 4.0e6;
/// The longest FEF part, seconds: T2-base, T2-Lite (clause 8.4, annex I.9).
pub const LONGEST_FEF_S: [f64; 2] = [0.25, 1.0];

impl Transmitter {
    /// Builds the transmitter. Errors when the configuration breaks a rule of the standard that this transmitter tests.
    pub fn new(cfg: TxConfig) -> Result<Transmitter, T2Error> {
        let mut shape = cfg.shape;
        if cfg.plps.is_empty() {
            return Err(T2Error::Rule(Rule::NoPlp));
        }
        let lite = cfg.profile == Profile::Lite;
        let mut plps = Vec::with_capacity(cfg.plps.len());
        let multi = cfg.plps.len() > 1;
        if !allowed(shape.fft, shape.guard, shape.pp, shape.miso, lite) {
            return Err(T2Error::Rule(Rule::PilotsForGuard));
        }
        for spec in cfg.plps {
            let c = spec.coding;
            c.check(cfg.profile)?;
            if spec.time_il == TimeIl::Off && multi {
                return Err(T2Error::Rule(Rule::TimeIlOff));
            }
            plps.push(PlpState {
                enc: Encoder::new(c)?,
                ci: CellInterleaver::new(c.cells())?,
                framer: TsFramer::default(),
                pending: Vec::new(),
                pending_at: 0,
                next: IfBuild::default(),
                bb: vec![0; c.k_bch() / 8],
                bits: vec![0; c.k_bch()],
                block: vec![Complex32::default(); c.cells()],
                multi,
                spec,
            });
        }
        if let Some((len, iv)) = cfg.fef {
            if iv == 0 || !(cfg.num_t2_frames.max(2)).is_multiple_of(iv) {
                return Err(T2Error::Rule(Rule::FefInterval));
            }
            let bits = if lite { 24 } else { 22 };
            let longest = LONGEST_FEF_S[usize::from(lite)] * cfg.bandwidth.rate();
            if len >> bits != 0 || f64::from(len) > longest {
                return Err(T2Error::Rule(Rule::FefLength));
            }
        }
        let sub_slices = if plps.iter().any(|p| p.spec.kind == PlpKind::Type2) {
            cfg.sub_slices.max(1) as usize
        } else {
            1
        };
        for p in &plps {
            let p_i = match p.spec.time_il {
                TimeIl::Frames(n) => n.max(1) as usize,
                _ => 1,
            };
            let nc = p.spec.coding.cells();
            // Clause 8.3.6.1: D_i is whole. Clause 8.3.6.3.3: N_cells mod (5 N_subslices P_I) = 0 for type 2.
            if !(p.spec.blocks as usize * nc).is_multiple_of(p_i) {
                return Err(T2Error::Rule(Rule::InterleavingFrame));
            }
            if p.spec.kind == PlpKind::Type2 && nc % (5 * sub_slices * p_i) != 0 {
                return Err(T2Error::Rule(Rule::SubSlices));
            }
            if p_i > 1 && !(cfg.num_t2_frames as usize).is_multiple_of(p_i) {
                return Err(T2Error::Rule(Rule::FramesForPi));
            }
        }
        // A first frame shape, so the L1-post fields can be counted before the data symbols are chosen.
        let probe = FrameShape {
            l_data: if shape.fft == Fft::K32 { 65 } else { 64 },
            ..shape
        };
        let mut t = Transmitter {
            cfg_bw: cfg.bandwidth,
            shape,
            plps,
            maps: PilotMaps::new(probe)?,
            cells: FrameCells::default(),
            freq: FreqInterleaver::new(shape.fft, &[])?,
            pre: L1Pre::default(),
            post: L1Post::default(),
            post_sizes: post_sizes(1, Modulation::Bpsk, 1)?,
            pre_codec: L1Codec::pre()?,
            post_codec: L1Codec::pre()?,
            num_t2_frames: cfg.num_t2_frames.max(2),
            frame_count: 0,
            sub_slices,
            profile: cfg.profile,
            miso: cfg.miso,
            fef: cfg.fef,
            ifft: DspFft::inverse(shape.fft.size()),
            at: 0,
            ahead: false,
            flat: Vec::new(),
            pre_cells: Vec::new(),
            post_cells: Vec::new(),
            x: Vec::new(),
            a: Vec::new(),
            c1: vec![Complex32::default(); shape.fft.size()],
            c2: vec![Complex32::default(); shape.fft.size()],
            g2: Vec::new(),
            g1: Vec::new(),
            lines: [Vec::new(), Vec::new()],
        };
        t.pre.repetition = cfg.repetition;
        t.post = t.l1_post(0);
        let info = t.post.info_bits().len();
        let l1m = cfg.l1_mod;
        t.post_sizes = post_sizes(info, l1m, shape.n_p2())?;
        if lite && t.post_sizes.blocks > 1 {
            return Err(T2Error::Rule(Rule::LitePostBlocks));
        }
        t.post_codec = L1Codec::post(&t.post_sizes, l1m, cfg.post_scrambled)?;
        // The fewest data symbols that hold every PLP of the busiest frame.
        let need: usize = t.plps.iter().map(|p| p.cells_per_frame()).sum();
        if shape.l_data == 0 {
            let min = if shape.fft == Fft::K32 { 3 } else { 7 };
            let mut l = min;
            loop {
                let s = FrameShape { l_data: l, ..shape };
                if s.fft == Fft::K32 && s.l_f() % 2 == 1 {
                    l += 1;
                    continue;
                }
                let m = PilotMaps::new(s)?;
                let fc = FrameCells::new(&m, t.post_sizes.cells)?;
                if fc.d_plp >= need && !t.too_fast(s) || l > 4000 {
                    break;
                }
                l += 1;
            }
            shape.l_data = l;
        }
        t.shape = shape;
        t.maps = PilotMaps::new(shape)?;
        t.cells = FrameCells::new(&t.maps, t.post_sizes.cells)?;
        if t.cells.d_plp < need {
            return Err(T2Error::Rule(Rule::PlpsDoNotFit));
        }
        let counts: Vec<usize> = (0..shape.l_f())
            .map(|l| t.maps.symbol(l).data.len())
            .collect();
        t.freq = FreqInterleaver::new(shape.fft, &counts)?;
        let p1 = P1Info::new(
            shape.fft,
            shape.guard,
            cfg.profile,
            shape.miso,
            t.fef.is_some(),
        )
        .ok_or(Rule::NotSignalled)?;
        t.pre = L1Pre {
            stream_type: 0,
            bwt_ext: shape.extended,
            s1: p1.s1,
            s2: p1.s2,
            repetition: cfg.repetition,
            guard: shape.guard.code(),
            // Tone reservation (code 0010) leaves the reserved carriers at 0: no PAPR algorithm runs (clause 9.3).
            papr: if shape.tr { 2 } else { 0 },
            l1_mod: l1m.l1_code().unwrap_or(0),
            l1_cod: 0,
            l1_fec_type: 0,
            l1_post_size: t.post_sizes.cells as u32,
            l1_post_info_size: info as u32,
            pilot_pattern: shape.pp.code(),
            tx_id_availability: 0,
            cell_id: cfg.cell_id,
            network_id: cfg.network_id,
            t2_system_id: cfg.t2_system_id,
            num_t2_frames: t.num_t2_frames,
            num_data_symbols: shape.l_data as u16,
            regen_flag: 0,
            post_extension: false,
            num_rf: 1,
            current_rf_idx: 0,
            t2_version: 2,
            post_scrambled: cfg.post_scrambled,
            t2_base_lite: false,
            reserved: 0,
        };
        if t.too_fast(shape) {
            return Err(T2Error::Rule(Rule::LiteBitRate));
        }
        Ok(t)
    }

    /// T2-Lite: whether a data PLP with the common PLP of its group carries more than [`LITE_MOST_BIT_RATE`] in frames of
    /// `shape`, FEF parts included in the time.
    fn too_fast(&self, shape: FrameShape) -> bool {
        if self.profile != Profile::Lite {
            return false;
        }
        let fef = self.fef.map_or(0.0, |(len, iv)| f64::from(len) / f64::from(iv));
        let frame = p1::P1_LEN + shape.l_f() * shape.symbol_len().unwrap_or(0);
        let frame_s = (frame as f64 + fef) / self.rate();
        self.plps
            .iter()
            .filter(|p| p.spec.kind != PlpKind::Common)
            .any(|p| {
                let common: usize = self
                    .plps
                    .iter()
                    .filter(|c| c.spec.kind == PlpKind::Common && c.spec.group == p.spec.group)
                    .map(PlpState::bits_per_frame)
                    .sum();
                (p.bits_per_frame() + common) as f64 / frame_s > LITE_MOST_BIT_RATE
            })
    }

    /// The frame shape in use (with the data symbols chosen).
    pub fn shape(&self) -> FrameShape {
        self.shape
    }

    /// The sample rate (1/T).
    pub fn rate(&self) -> f64 {
        self.cfg_bw.rate()
    }

    /// The net bits of every PLP in one frame: FEC blocks times (K_bch - 80), spread over P_I frames.
    pub fn plp_bits_per_frame(&self) -> usize {
        self.plps.iter().map(PlpState::bits_per_frame).sum()
    }

    /// The mean power of a T2-frame: the P1 symbol at power 1 and each symbol after it with its pilots, their boost and
    /// its cells at power 1 (the mean of the L1 cells and of the data cells), over the frame with its guard intervals. A
    /// MISO frame counts one group: each group sends this power, and [`MisoOut::Sum`] adds the power gains of its paths.
    pub fn mean_power(&self) -> f32 {
        let shape = self.shape;
        let sym = shape.symbol_len().unwrap_or(0) as f32;
        let scale = 5.0 / (27.0 * shape.k_total() as f32).sqrt();
        let mut energy = p1::P1_LEN as f32;
        for l in 0..shape.l_f() {
            let map = self.maps.symbol(l);
            let pilots: f32 = map.pilots.iter().map(|p| p.amp * p.amp).sum();
            let cells = self.maps.active_cells(l) as f32;
            // A sample of the IFFT has the power of the sum of its carriers.
            energy += (pilots + cells) * scale * scale * sym;
        }
        energy / self.frame_len() as f32
    }

    /// The L1-pre of every frame.
    pub fn l1_pre(&self) -> &L1Pre {
        &self.pre
    }

    /// The samples of one T2-frame (P1 included) and of the FEF part after it, if one is due.
    pub fn frame_len(&self) -> usize {
        p1::P1_LEN + self.shape.l_f() * self.shape.symbol_len().unwrap_or(0)
    }

    fn plp_config(&self, p: &PlpState) -> PlpConfig {
        let c = p.spec.coding;
        let (len, ty) = match p.spec.time_il {
            TimeIl::Off => (0, false),
            TimeIl::Blocks(n) => (n.max(1), false),
            TimeIl::Frames(n) => (n.max(1), true),
        };
        PlpConfig {
            plp_id: p.spec.id,
            plp_type: p.spec.kind.code(),
            payload_type: 3,
            group_id: p.spec.group,
            cod: c.rate.code().unwrap_or(0),
            modulation: c.modulation.plp_code().unwrap_or(0),
            rotation: c.rotated,
            fec_type: c.frame.code(),
            num_blocks_max: p.spec.blocks,
            frame_interval: 1,
            time_il_length: len,
            time_il_type: ty,
            plp_mode: 1,
            static_flag: true,
            static_padding_flag: true,
            ..Default::default()
        }
    }

    /// The L1-post of frame index `idx` with the PLP starts of the schedule.
    fn l1_post(&self, idx: u8) -> L1Post {
        let config = PostConfig {
            sub_slices_per_frame: self.sub_slices as u16,
            rf: vec![(0, 0)],
            fef: self.fef.map(|(len, iv)| (0, len & 0x3F_FFFF, iv)),
            fef_length_msb: match (self.fef, self.profile) {
                (Some((len, _)), Profile::Lite) => (len >> 22) as u8 & 3,
                _ => 0,
            },
            plps: self.plps.iter().map(|p| self.plp_config(p)).collect(),
            ..Default::default()
        };
        let dynamic = self.dynamic(idx);
        let next = self
            .pre
            .repetition
            .then(|| self.dynamic(idx.wrapping_add(1) % self.num_t2_frames));
        L1Post {
            config,
            dynamic,
            next,
            extension: Vec::new(),
        }
    }

    /// The schedule: common and type 1 PLPs one after another from address 0, then the type 2 sub-slices.
    fn dynamic(&self, idx: u8) -> PostDynamic {
        let mut a = 0usize;
        let mut plps = Vec::new();
        let mut t2_total = 0usize;
        for p in &self.plps {
            if p.spec.kind != PlpKind::Type2 {
                plps.push(PlpDynamic {
                    plp_id: p.spec.id,
                    plp_start: a as u32,
                    num_blocks: p.spec.blocks,
                    reserved_2: 0,
                });
                a += p.cells_per_frame();
            } else {
                t2_total += p.cells_per_frame();
            }
        }
        let type_2_start = a;
        let ssi = t2_total / self.sub_slices;
        let mut off = 0;
        for p in &self.plps {
            if p.spec.kind == PlpKind::Type2 {
                plps.push(PlpDynamic {
                    plp_id: p.spec.id,
                    plp_start: (type_2_start + off) as u32,
                    num_blocks: p.spec.blocks,
                    reserved_2: 0,
                });
                off += p.cells_per_frame() / self.sub_slices;
            }
        }
        PostDynamic {
            frame_idx: idx,
            sub_slice_interval: if t2_total > 0 { ssi as u32 } else { 0 },
            type_2_start: if t2_total > 0 { type_2_start as u32 } else { 0 },
            plps,
            ..Default::default()
        }
    }

    /// Makes the next T2-frame (and FEF part) at the elementary period. Errors only on an internal inconsistency.
    pub fn frame(&mut self) -> Result<Vec<Complex32>, T2Error> {
        let mut out = Vec::with_capacity(self.frame_len());
        loop {
            self.part(None, &mut out)?;
            if self.at == 0 {
                return Ok(out);
            }
        }
    }

    /// Whether the next part starts a T2-frame.
    pub fn at_frame_start(&self) -> bool {
        self.at == 0
    }

    /// With `on`, [`Transmitter::part`] codes the FEC blocks of the next Interleaving Frame while it makes the symbols of
    /// a frame, a share with each symbol, so that the work of a frame is spread over the frame. The samples are the same.
    pub fn code_ahead(&mut self, on: bool) {
        self.ahead = on;
    }

    /// Adds the next part of the signal to `out`: the P1 symbol of a T2-frame, then each symbol of the frame, the FEF part
    /// with the last one. With `feed`, the first PLP takes its packets from it in place of its own source. Errors only on
    /// an internal inconsistency.
    pub fn part(&mut self, feed: Feed<'_, '_>, out: &mut Vec<Complex32>) -> Result<(), T2Error> {
        let from = out.len();
        self.g2.clear();
        self.next_part(feed, out)?;
        if let (true, MisoOut::Sum(paths)) = (self.shape.miso, self.miso) {
            let y = &mut out[from..];
            self.g1.clear();
            self.g1.extend_from_slice(y);
            delayed(&mut self.lines[0], paths[0], &self.g1, y, false);
            delayed(&mut self.lines[1], paths[1], &self.g2, y, true);
        }
        Ok(())
    }

    /// Whether the parts carry the group 2 samples in `g2` beside the group 1 samples.
    fn sums(&self) -> bool {
        self.shape.miso && matches!(self.miso, MisoOut::Sum(_))
    }

    /// [`Transmitter::part`] before the paths of [`MisoOut::Sum`]: the samples of the group that `out` takes, and with
    /// [`MisoOut::Sum`] the group 2 samples in `g2`. P1 and the FEF part are the same in both groups.
    fn next_part(&mut self, mut feed: Feed<'_, '_>, out: &mut Vec<Complex32>) -> Result<(), T2Error> {
        let idx = (self.frame_count % self.num_t2_frames as u64) as u8;
        if self.at == 0 {
            self.build_cells(idx, feed)?;
            let at = out.len();
            out.extend_from_slice(&p1::generate(P1Info {
                s1: self.pre.s1,
                s2: self.pre.s2,
            }));
            if self.sums() {
                self.g2.extend_from_slice(&out[at..]);
            }
            self.at = 1;
            return Ok(());
        }
        let l = self.at - 1;
        let l_f = self.shape.l_f();
        self.symbol(l, out)?;
        if self.ahead {
            for (i, p) in self.plps.iter_mut().enumerate() {
                let f = if i == 0 { feed.as_deref_mut() } else { None };
                p.code_ahead(f, l + 1, l_f)?;
            }
        }
        self.at += 1;
        if self.at <= l_f {
            return Ok(());
        }
        self.at = 0;
        self.frame_count += 1;
        if let Some((len, iv)) = self.fef {
            // A FEF part follows the T2-frames whose FRAME_IDX + 1 is a multiple of FEF_INTERVAL (clause 8.2).
            if iv > 0 && (idx as u64 + 1).is_multiple_of(iv as u64) {
                let at = out.len();
                let fef_p1 = p1::generate(P1Info { s1: 2, s2: 0 });
                out.extend_from_slice(&fef_p1);
                out.resize(
                    out.len() + (len as usize).saturating_sub(p1::P1_LEN),
                    Complex32::default(),
                );
                if self.sums() {
                    self.g2.extend_from_slice(&out[at..]);
                }
            }
        }
        Ok(())
    }

    /// The cells of frame index `idx`: the PLP cells at their addresses with the dummy cells, and the L1 cells.
    fn build_cells(&mut self, idx: u8, mut feed: Feed<'_, '_>) -> Result<(), T2Error> {
        // PLP cells of this frame.
        let mut plp_cells: Vec<Vec<Complex32>> = Vec::with_capacity(self.plps.len());
        let cfgs: Vec<PlpConfig> = self.plps.iter().map(|p| self.plp_config(p)).collect();
        for (i, (p, cfg)) in self.plps.iter_mut().zip(&cfgs).enumerate() {
            let f = if i == 0 { feed.as_deref_mut() } else { None };
            plp_cells.push(if plp_in_frame(cfg, idx) {
                p.frame_cells(f)?
            } else {
                Vec::new()
            });
        }
        // The one-dimensional addresses (clause 8.3.6.2).
        let mut flat = std::mem::take(&mut self.flat);
        flat.clear();
        flat.resize(self.cells.d_plp, Complex32::default());
        let mut used = 0usize;
        let dynamic = self.dynamic(idx);
        let mut t2_end = 0usize;
        for (p, cells) in self.plps.iter().zip(&plp_cells) {
            let d = dynamic
                .plps
                .iter()
                .find(|d| d.plp_id == p.spec.id)
                .copied()
                .unwrap_or_default();
            let start = d.plp_start as usize;
            if p.spec.kind == PlpKind::Type2 {
                let each = cells.len() / self.sub_slices;
                for j in 0..self.sub_slices {
                    let at = start + j * dynamic.sub_slice_interval as usize;
                    flat[at..at + each].copy_from_slice(&cells[j * each..(j + 1) * each]);
                    t2_end = t2_end.max(at + each);
                }
            } else {
                flat[start..start + cells.len()].copy_from_slice(cells);
                used = used.max(start + cells.len());
            }
        }
        used = used.max(t2_end);
        dummy_cells(&mut flat[used..]);
        self.flat = flat;
        // L1.
        self.post = self.l1_post(idx);
        let pre_bits = self.pre.to_bits();
        self.pre_cells.clear();
        self.pre_cells
            .resize(crate::l1::L1_PRE_CELLS, Complex32::default());
        self.pre_codec.encode(&pre_bits, &mut self.pre_cells)?;
        let mut post_bits = self.post.to_bits();
        post_bits.resize(self.post_sizes.k_sig * self.post_sizes.blocks, 0);
        let per = self.post_codec.cells();
        self.post_cells.clear();
        self.post_cells
            .resize(per * self.post_sizes.blocks, Complex32::default());
        for b in 0..self.post_sizes.blocks {
            let k = self.post_sizes.k_sig;
            self.post_codec.encode(
                &post_bits[b * k..(b + 1) * k],
                &mut self.post_cells[b * per..(b + 1) * per],
            )?;
        }
        Ok(())
    }

    /// Adds symbol `l` of the frame in the making to `out`: its cells, frequency interleaved, with the pilots, as samples
    /// with the guard interval. In MISO, `out` takes group 1, or group 2 with [`MisoOut::Group2`]; [`MisoOut::Sum`] puts
    /// group 2 in `g2`.
    fn symbol(&mut self, l: usize, out: &mut Vec<Complex32>) -> Result<(), T2Error> {
        let shape = self.shape;
        let n = shape.fft.size();
        let g = shape.guard.samples(shape.fft).ok_or(Rule::GuardForFft)?;
        let kt = shape.k_total();
        let kc = (kt - 1) / 2;
        let scale = 5.0 / (27.0 * kt as f32).sqrt();
        let sums = self.sums();
        let only2 = shape.miso && self.miso == MisoOut::Group2;
        let (x, a, c1, c2) = (&mut self.x, &mut self.a, &mut self.c1, &mut self.c2);
        let map = self.maps.symbol(l);
        let nd = map.data.len();
        x.clear();
        x.resize(nd, Complex32::default());
        let (first, off) = self.cells.symbol_start(l);
        if shape.kind(l) == SymbolKind::P2 {
            for (i, c) in self.pre_cells.iter().enumerate() {
                let (ll, p) = self.cells.l1_pre_cell(i);
                if ll == l {
                    x[p] = *c;
                }
            }
            for (i, c) in self.post_cells.iter().enumerate() {
                let (ll, p) = self.cells.l1_post_cell(i);
                if ll == l {
                    x[p] = *c;
                }
            }
        }
        let active = self.maps.active_cells(l);
        if active > off {
            x[off..active].copy_from_slice(&self.flat[first..first + active - off]);
        }
        // The unmodulated cells of the frame closing symbol stay 0 (clause 8.3.9).
        a.clear();
        a.resize(nd, Complex32::default());
        self.freq
            .for_symbol(l, nd)
            .ok_or(T2Error::NoInterleaver { cells: nd })?
            .apply(x, a)?;
        c1.fill(Complex32::default());
        c2.fill(Complex32::default());
        let bin = |k: u32| ((k as i64 - kc as i64).rem_euclid(n as i64)) as usize;
        for (p, &k) in map.data.iter().enumerate() {
            c1[bin(k)] = a[p];
            if shape.miso {
                // Modified Alamouti on pairs (clause 9.1).
                let v = if p % 2 == 0 {
                    -a[p + 1].conj()
                } else {
                    a[p - 1].conj()
                };
                c2[bin(k)] = v;
            }
        }
        for pc in &map.pilots {
            c1[bin(pc.k)] = Complex32::new(self.maps.pilot_value(l, pc, false), 0.0);
            if shape.miso {
                c2[bin(pc.k)] = Complex32::new(self.maps.pilot_value(l, pc, true), 0.0);
            }
        }
        let ifft = &mut self.ifft;
        let mut emit = |c: &mut Vec<Complex32>, out: &mut Vec<Complex32>| {
            ifft.process(c);
            for v in c.iter_mut() {
                *v *= scale;
            }
            out.extend_from_slice(&c[n - g..]);
            out.extend_from_slice(c);
        };
        if only2 {
            emit(c2, out);
        } else {
            emit(c1, out);
            if sums {
                emit(c2, &mut self.g2);
            }
        }
        Ok(())
    }
}

/// `y[i] = gain x[i - delay]` (or `+=` with `add`) over a stream that comes in blocks: `line` holds the last `delay`
/// samples of `x` before this block, zeros at first, and keeps those of this block for the next.
fn delayed(line: &mut Vec<Complex32>, p: Path, x: &[Complex32], y: &mut [Complex32], add: bool) {
    let d = p.delay;
    line.resize(d, Complex32::default());
    let m = x.len().min(y.len());
    for (i, out) in y.iter_mut().enumerate().take(m) {
        let v = if i < d { line[i] } else { x[i - d] } * p.gain;
        if add {
            *out += v;
        } else {
            *out = v;
        }
    }
    for j in 0..d {
        let s = m + j;
        line[j] = if s < d { line[s] } else { x[s - d] };
    }
}

impl PlpState {
    /// The cells this PLP sends in each frame it is in.
    fn cells_per_frame(&self) -> usize {
        let p_i = match self.spec.time_il {
            TimeIl::Frames(n) => n.max(1) as usize,
            _ => 1,
        };
        self.spec.blocks as usize * self.spec.coding.cells() / p_i
    }

    /// The net bits this PLP sends in each frame: FEC blocks times (K_bch - 80), spread over P_I frames.
    fn bits_per_frame(&self) -> usize {
        let p_i = match self.spec.time_il {
            TimeIl::Frames(n) => n.max(1) as usize,
            _ => 1,
        };
        self.spec.blocks as usize * (self.spec.coding.k_bch() - 80) / p_i
    }

    /// One BBFRAME of transport packets in normal mode (clauses 5.1.7, 5.2.4): header, data field, scrambled, as bits. The
    /// packets come from `feed`, else from the source of the PLP.
    fn bbframe(&mut self, feed: Feed<'_, '_>) {
        let k = self.spec.coding.k_bch();
        let dfl = k - 80;
        let own = &mut self.spec.source;
        let mut feed = feed;
        let mut next = || {
            let mut p = [0u8; 188];
            match feed.as_deref_mut() {
                Some(f) => f.next_packet(&mut p),
                None => own.next_packet(&mut p),
            }
            p
        };
        let syncd = self.framer.fill(
            &mut self.bb[BBHEADER_LEN..BBHEADER_LEN + dfl / 8],
            &mut next,
        );
        let mut h = BbHeader::transport(dfl as u16, syncd);
        h.single_stream = !self.multi;
        h.stream_id = self.spec.id;
        let mut hb = [0u8; BBHEADER_LEN];
        h.write(&mut hb);
        self.bb[..BBHEADER_LEN].copy_from_slice(&hb);
        bb_descramble(&mut self.bb);
        let _ = unpack_into(&self.bb, BitOrder::Msb, &mut self.bits);
    }

    /// The TI-blocks of an Interleaving Frame.
    fn n_ti(&self) -> usize {
        match self.spec.time_il {
            TimeIl::Blocks(n) => n.max(1) as usize,
            _ => 1,
        }
    }

    /// N_FEC_TI(n, s), the FEC blocks of TI-block `s`: the smaller TI-blocks first (clause 6.5.2).
    fn n_fec(&self, s: usize) -> usize {
        let (blocks, n_ti) = (self.spec.blocks as usize, self.n_ti());
        let base = blocks / n_ti;
        if s < n_ti - blocks % n_ti {
            base
        } else {
            base + 1
        }
    }

    /// Whether the next Interleaving Frame is whole.
    fn next_whole(&self) -> bool {
        self.next.s >= self.n_ti()
    }

    /// Makes the next FEC block of the next Interleaving Frame: encode, cell interleave (r counts within the TI-block).
    /// The FEC block that completes a TI-block is followed by the time interleaving of the TI-block.
    fn step(&mut self, feed: Feed<'_, '_>) -> Result<(), T2Error> {
        if self.next_whole() {
            return Ok(());
        }
        let nc = self.spec.coding.cells();
        let nfec = self.n_fec(self.next.s);
        let interleave = self.spec.time_il != TimeIl::Off;
        if self.next.r == 0 {
            self.next.cells.clear();
            self.next.cells.resize(nfec * nc, Complex32::default());
        }
        if nfec > 0 {
            self.bbframe(feed);
            let r = self.next.r;
            let rr = if interleave { r } else { 0 };
            self.enc.encode(&self.bits, &mut self.block)?;
            self.ci
                .interleave(rr, &self.block, &mut self.next.cells[r * nc..(r + 1) * nc]);
            self.next.r += 1;
            self.next.made += 1;
        }
        if self.next.r >= nfec {
            if interleave {
                let at = self.next.out.len();
                self.next
                    .out
                    .resize(at + nfec * nc, Complex32::default());
                time_interleave(nc, nfec, &self.next.cells, &mut self.next.out[at..]);
            } else {
                self.next.out.extend_from_slice(&self.next.cells);
            }
            self.next.s += 1;
            self.next.r = 0;
        }
        Ok(())
    }

    /// The cells of this PLP for one T2-frame.
    fn frame_cells(&mut self, mut feed: Feed<'_, '_>) -> Result<Vec<Complex32>, T2Error> {
        let per_frame = self.cells_per_frame();
        if self.pending_at >= self.pending.len() {
            while !self.next_whole() {
                self.step(feed.as_deref_mut())?;
            }
            std::mem::swap(&mut self.pending, &mut self.next.out);
            self.pending_at = 0;
            self.next.out.clear();
            self.next.s = 0;
            self.next.made = 0;
        }
        let out = self.pending[self.pending_at..self.pending_at + per_frame].to_vec();
        self.pending_at += per_frame;
        Ok(out)
    }

    /// Makes FEC blocks of the next Interleaving Frame until `num / den` of them are made, when the frame on air is the
    /// last one of its Interleaving Frame.
    fn code_ahead(&mut self, mut feed: Feed<'_, '_>, num: usize, den: usize) -> Result<(), T2Error> {
        if self.pending_at < self.pending.len() {
            return Ok(());
        }
        let due = self.spec.blocks as usize * num / den;
        while self.next.made < due && !self.next_whole() {
            self.step(feed.as_deref_mut())?;
        }
        Ok(())
    }
}

/// The cells of the time interleaver memory M_TI of T2-base (clause 6.5.2).
pub const TI_MEMORY_CELLS: usize = (1 << 19) + (1 << 15);
/// The cells of the time interleaver memory M_TI of T2-Lite (annex I.8).
pub const TI_MEMORY_CELLS_LITE: usize = 1 << 18;
/// The longest T2-frame, seconds (clause 8.3.1).
pub const LONGEST_FRAME_S: f64 = 0.25;

/// The T2-frame of a sender with one data PLP.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramePlan {
    /// L_data, the data symbols after the P2 symbols.
    pub l_data: usize,
    /// The FEC blocks of the PLP in each T2-frame.
    pub blocks: u16,
    /// The time interleaving.
    pub time_il: TimeIl,
    /// The samples of a T2-frame at the elementary period.
    pub frame_samples: usize,
    /// The seconds of a T2-frame.
    pub frame_s: f64,
    /// The bit rate of the transport stream that the PLP carries in normal mode.
    pub bit_rate: f64,
}

/// L1_POST_INFO_SIZE of a multiplex with one RF channel, one PLP, no FEF part and no auxiliary stream.
pub(crate) fn single_plp_post_bits() -> usize {
    L1Post {
        config: PostConfig {
            rf: vec![(0, 0)],
            plps: vec![PlpConfig::default()],
            ..Default::default()
        },
        dynamic: PostDynamic {
            plps: vec![PlpDynamic::default()],
            ..Default::default()
        },
        next: None,
        extension: Vec::new(),
    }
    .info_bits()
    .len()
}

/// The frame of a sender of `profile` with one data PLP of `coding` in a channel of `bandwidth`: the most FEC blocks that
/// fit a frame of at most `longest_s` seconds (at most [`LONGEST_FRAME_S`], at least the shortest frame of the FFT size),
/// that PLP_NUM_BLOCKS can state (1023), that `ti_blocks` TI-blocks hold in [`TI_MEMORY_CELLS`] ([`TI_MEMORY_CELLS_LITE`]
/// in T2-Lite) and, in T2-Lite, that carry at most [`LITE_MOST_BIT_RATE`]; then the fewest data symbols that hold them.
/// `ti_blocks` 0 is no time interleaving, with the FEC blocks of one TI-block at most. `shape.l_data` is not read.
///
/// # Errors
///
/// [`T2Error::Rule`] when the standard does not allow the combination (tables 58, 59, I.5 and I.6, the guard for the FFT
/// size, the extended carriers, the code for the profile, the L1-post constellation) or the frame holds no FEC block.
pub fn plan_frame(
    bandwidth: Bandwidth,
    shape: FrameShape,
    profile: Profile,
    coding: Coding,
    l1_mod: Modulation,
    ti_blocks: u8,
    longest_s: f64,
) -> Result<FramePlan, T2Error> {
    let lite = profile == Profile::Lite;
    let sym = shape.symbol_len().ok_or(Rule::GuardForFft)?;
    if !allowed(shape.fft, shape.guard, shape.pp, shape.miso, lite) {
        return Err(Rule::PilotsForGuard.into());
    }
    coding.check(profile)?;
    let k = k_bch(coding.frame, coding.rate).ok_or(Rule::NoCode)?;
    let n_p2 = shape.n_p2();
    let post = post_sizes(single_plp_post_bits(), l1_mod, n_p2)?.cells;
    let rate = bandwidth.rate();
    let k32 = shape.fft == Fft::K32;
    let min = if k32 { 3 } else { 7 };
    let longest = longest_s.min(LONGEST_FRAME_S) * rate;
    let mut most = ((longest as usize).saturating_sub(p1::P1_LEN) / sym).saturating_sub(n_p2);
    if k32 && (n_p2 + most) % 2 == 1 {
        most = most.saturating_sub(1);
    }
    let most = most.max(min);
    let maps = PilotMaps::new(FrameShape {
        l_data: most,
        ..shape
    })?;
    let closing = shape.has_fc().then(|| maps.active_cells(n_p2 + most - 1));
    let l1 = crate::l1::L1_PRE_CELLS + post;
    if !l1.is_multiple_of(n_p2) || l1 / n_p2 > maps.p2().data.len() {
        return Err(Rule::L1DoesNotFit.into());
    }
    // D_PLP of a frame of `l` data symbols (clause 8.3.2).
    let d_plp = |l: usize| {
        let normal = l - usize::from(closing.is_some());
        let data: usize = (n_p2..n_p2 + normal)
            .map(|s| maps.normal(s).data.len())
            .sum();
        n_p2 * maps.p2().data.len() - l1 + data + closing.unwrap_or(0)
    };
    let nc = coding.cells();
    let ti_memory = if lite { TI_MEMORY_CELLS_LITE } else { TI_MEMORY_CELLS };
    let memory = ti_blocks.max(1) as usize * (ti_memory / nc);
    let mut blocks = (d_plp(most) / nc).min(memory).min(1023);
    let seconds = |l: usize| (p1::P1_LEN + (n_p2 + l) * sym) as f64 / rate;
    // T2-Lite: `b` FEC blocks in a frame of `l` data symbols carry more than the profile allows.
    let fast = |b: usize, l: usize| lite && (b * (k - 80)) as f64 / seconds(l) > LITE_MOST_BIT_RATE;
    while blocks > 0 && fast(blocks, most) {
        blocks -= 1;
    }
    if blocks == 0 {
        return Err(Rule::PlpsDoNotFit.into());
    }
    let whole = |l: usize| !k32 || (n_p2 + l).is_multiple_of(2);
    let l_data = (min..=most)
        .find(|&l| whole(l) && d_plp(l) >= blocks * nc && !fast(blocks, l))
        .unwrap_or(most);
    let frame_samples = p1::P1_LEN + (n_p2 + l_data) * sym;
    let frame_s = seconds(l_data);
    Ok(FramePlan {
        l_data,
        blocks: blocks as u16,
        time_il: match ti_blocks {
            0 => TimeIl::Off,
            n => TimeIl::Blocks((n as usize).min(blocks) as u8),
        },
        frame_samples,
        frame_s,
        bit_rate: (blocks * (k - 80)) as f64 / frame_s,
    })
}
