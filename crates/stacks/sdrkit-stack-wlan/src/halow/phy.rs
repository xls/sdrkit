//! The S1G PHY: widths and their tone plans, the training fields, the SIG fields of the S1G_1M, S1G_SHORT and S1G_LONG
//! preambles, rates and valid combinations, and the PPDU builder.
//! Standard: IEEE Std 802.11-2020 clause 23.

use super::coding::{data_segments, encode, map, points, punctured, scrambler, Interleavers};
pub use super::coding::{mcs_params, n_col, REP};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC4_INTERLAKEN;
use serde::{Deserialize, Serialize};

/// The widths, MHz.
pub const WIDTHS: [u32; 5] = [1, 2, 4, 8, 16];

/// The index of a width in [`WIDTHS`].
pub fn width_index(w: u32) -> usize {
    WIDTHS.iter().position(|&x| x == w).unwrap_or(0)
}

/// The grid a width is sampled on: (rate, FFT size).
pub fn grid(w: u32) -> (f64, usize) {
    match w {
        8 => (8e6, 256),
        16 => (16e6, 512),
        _ => (4e6, 128),
    }
}

/// The lowest radio rate for a width, and the recommended one.
pub fn width_rates(w: u32) -> (f64, f64) {
    match w {
        1 => (2e6, 2e6),
        2 => (2e6, 4e6),
        4 => (4e6, 5e6),
        8 => (8e6, 10e6),
        _ => (16e6, 20e6),
    }
}

/// The widest width a rate holds (0 for none).
pub fn max_width_mhz(rate: f64) -> u32 {
    let mut best = 0;
    for w in WIDTHS {
        if rate >= width_rates(w).0 * 0.999_999 {
            best = w;
        }
    }
    best
}

sdrkit_core::choice_enum! {
    /// The preamble of a PPDU.
    #[derive(Serialize, Deserialize)]
    pub enum Preamble {
        /// S1G_SHORT (2 MHz and wider).
        Short = ("s1g_short", "S1G_SHORT"),
        /// S1G_LONG (2 MHz and wider).
        Long = ("s1g_long", "S1G_LONG"),
        /// S1G_1M (1 MHz).
        OneM = ("s1g_1m", "S1G_1M"),
    }
}

impl Preamble {
    /// "S1G_SHORT", "S1G_LONG", "S1G_1M".
    pub fn name(self) -> &'static str {
        match self {
            Preamble::Short => "S1G_SHORT",
            Preamble::Long => "S1G_LONG",
            Preamble::OneM => "S1G_1M",
        }
    }
    /// The place in `per_pre`: 0, 1 or 2.
    pub fn slot(self) -> usize {
        match self {
            Preamble::Short => 0,
            Preamble::Long => 1,
            Preamble::OneM => 2,
        }
    }
}

/// A tone plan: used tones, pilots and data tones (ascending), with the pilots' base values.
#[derive(Clone, Debug, PartialEq)]
pub struct TonePlan {
    /// Width, MHz.
    pub width: u32,
    /// Every used tone.
    pub used: Vec<i32>,
    /// Pilot tones.
    pub pilots: Vec<i32>,
    /// Pilot base values.
    pub pilot_vals: Vec<f32>,
    /// Data tones.
    pub data: Vec<i32>,
}

fn sym_range(lo: i32, hi: i32) -> Vec<i32> {
    let mut v: Vec<i32> = (lo..=hi).map(|k| -k).collect();
    v.extend(lo..=hi);
    v.sort();
    v
}

/// The tone plan of a width.
pub fn plan(w: u32) -> TonePlan {
    let (used, pilots): (Vec<i32>, Vec<i32>) = match w {
        1 => (sym_range(1, 13), vec![-7, 7]),
        2 => (sym_range(1, 28), vec![-21, -7, 7, 21]),
        4 => (sym_range(2, 58), vec![-53, -25, -11, 11, 25, 53]),
        8 => (
            sym_range(2, 122),
            vec![-103, -75, -39, -11, 11, 39, 75, 103],
        ),
        _ => {
            let half = sym_range(2, 122);
            let p8 = [-103, -75, -39, -11, 11, 39, 75, 103];
            let mut u: Vec<i32> = half.iter().map(|k| k - 128).collect();
            u.extend(half.iter().map(|k| k + 128));
            let mut p: Vec<i32> = p8.iter().map(|k| k - 128).collect();
            p.extend(p8.iter().map(|k| k + 128));
            (u, p)
        }
    };
    let data = used
        .iter()
        .copied()
        .filter(|k| !pilots.contains(k))
        .collect();
    let n = pilots.len();
    let pilot_vals = (0..n)
        .map(|i| if i + 1 == n { -1.0 } else { 1.0 })
        .collect();
    TonePlan {
        width: w,
        used,
        pilots,
        pilot_vals,
        data,
    }
}

/// The 2 MHz segments of a width: (centre tone, rotation γ).
pub fn segments(w: u32) -> Vec<(i32, Complex32)> {
    let one = Complex32::new(1.0, 0.0);
    let m1 = Complex32::new(-1.0, 0.0);
    let j = Complex32::new(0.0, 1.0);
    match w {
        2 => vec![(0, one)],
        4 => vec![(-32, one), (32, j)],
        8 => vec![(-96, one), (-32, m1), (32, m1), (96, m1)],
        16 => vec![
            (-224, one),
            (-160, m1),
            (-96, m1),
            (-32, m1),
            (32, one),
            (96, m1),
            (160, m1),
            (224, m1),
        ],
        _ => vec![(0, one)],
    }
}

const STF2: [(i32, f32); 12] = [
    (-24, 1.0),
    (-20, -1.0),
    (-16, 1.0),
    (-12, -1.0),
    (-8, -1.0),
    (-4, 1.0),
    (4, -1.0),
    (8, -1.0),
    (12, 1.0),
    (16, 1.0),
    (20, 1.0),
    (24, 1.0),
];

/// The STF of a width in the frequency domain.
pub fn stf_freq(w: u32) -> Vec<(i32, Complex32)> {
    if w == 1 {
        let s = [
            (-12, 1.0),
            (-8, -1.0),
            (-4, 1.0),
            (4, -1.0),
            (8, -1.0),
            (12, 1.0),
        ];
        return s.iter().map(|&(k, v)| (k, Complex32::new(v, v))).collect();
    }
    let mut v = Vec::new();
    for (c, g) in segments(w) {
        for &(k, s) in &STF2 {
            v.push((c + k, Complex32::new(s, s) * g));
        }
    }
    v
}

const LLTF: [i8; 53] = [
    1, 1, -1, -1, 1, 1, -1, 1, -1, 1, 1, 1, 1, 1, 1, -1, -1, 1, 1, -1, 1, -1, 1, 1, 1, 1, 0, 1, -1,
    -1, 1, 1, -1, 1, -1, 1, -1, -1, -1, -1, -1, 1, 1, -1, -1, 1, -1, 1, -1, 1, 1, 1, 1,
];

/// The LTF of a width: ±1 on exactly the used tones (2 MHz: the HT-LTF; the others from a fixed pseudo-random sequence).
pub fn ltf_freq(w: u32) -> Vec<(i32, f32)> {
    let p = plan(w);
    if w == 2 {
        return p
            .used
            .iter()
            .map(|&k| {
                let v = match k {
                    -28 | -27 => 1.0,
                    27 | 28 => -1.0,
                    _ => LLTF[(k + 26) as usize] as f32,
                };
                (k, v)
            })
            .collect();
    }
    let mut s = scrambler(match w {
        1 => 0x2B,
        4 => 0x45,
        8 => 0x19,
        _ => 0x63,
    });
    p.used
        .iter()
        .map(|&k| (k, if s.bit() == 1 { -1.0 } else { 1.0 }))
        .collect()
}

/// Data tones of a width.
pub fn n_sd(w: u32) -> usize {
    match w {
        1 => 24,
        2 => 52,
        4 => 108,
        8 => 234,
        _ => 468,
    }
}

/// Data bits per symbol (`None` when not a whole number).
pub fn n_dbps(w: u32, mcs: u8, nss: u8) -> Option<usize> {
    let (b, _, (num, den)) = mcs_params(mcs)?;
    if mcs == 10 {
        return Some(6 * nss as usize);
    }
    let x = n_sd(w) * b * nss as usize * num;
    x.is_multiple_of(den).then_some(x / den)
}

/// BCC encoders: one per 2400 data bits per symbol.
pub fn n_es(n_dbps: usize) -> usize {
    n_dbps.div_ceil(2400).max(1)
}

/// Whether the width can send this MCS with this many streams.
pub fn valid(w: u32, mcs: u8, nss: u8) -> bool {
    if !(1..=4).contains(&nss) || mcs > 10 {
        return false;
    }
    if mcs == 10 {
        return w == 1 && nss == 1;
    }
    match n_dbps(w, mcs, nss) {
        Some(d) => d % n_es(d) == 0,
        None => false,
    }
}

/// The data rate in Mbit/s.
pub fn rate_mbps(w: u32, mcs: u8, nss: u8, sgi: bool) -> f64 {
    n_dbps(w, mcs, nss).unwrap_or(0) as f64 / if sgi { 36.0 } else { 40.0 }
}

/// The number of LTFs for a number of space-time streams.
pub fn n_ltf(nsts: u8) -> usize {
    match nsts {
        1 => 1,
        2 => 2,
        _ => 4,
    }
}

/// The SIG field of an S1G PPDU (the bit positions are defined in [`encode_sig`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sig {
    /// Space-time streams (1 to 4).
    pub nsts: u8,
    /// Short guard interval.
    pub sgi: bool,
    /// LDPC coding.
    pub ldpc: bool,
    /// LDPC extra symbol.
    pub ldpc_extra: bool,
    /// STBC.
    pub stbc: bool,
    /// MCS.
    pub mcs: u8,
    /// Aggregation (A-MPDU; the length is in symbols).
    pub aggregated: bool,
    /// Length: bytes, or symbols when aggregated.
    pub length: u16,
    /// Response indication.
    pub response: u8,
    /// Smoothing.
    pub smoothing: bool,
    /// Traveling pilots.
    pub traveling: bool,
    /// Null data packet.
    pub ndp: bool,
    /// BW field width, MHz (SHORT and LONG).
    pub bw: u32,
    /// Partial AID.
    pub id: u16,
    /// Uplink.
    pub uplink: bool,
    /// Multi-user (LONG).
    pub mu: bool,
    /// Beamformed (LONG).
    pub beamformed: bool,
}

impl Sig {
    /// Spatial streams (half the space-time streams with STBC).
    pub fn nss(&self) -> u8 {
        if self.stbc {
            (self.nsts / 2).max(1)
        } else {
            self.nsts
        }
    }
}

/// The CRC-4 of the SIG information bits, most significant bit first: CRC-4/INTERLAKEN, x^4 + x + 1, preset to ones and
/// inverted (clause 23.3.8.2.2.3, the same CRC as the VHT-SIG-B and HT-SIG fields use).
pub fn sig_crc_bits(bits: &[u8]) -> [u8; 4] {
    let c = CRC4_INTERLAKEN.bits(bits);
    [(c >> 3) & 1, (c >> 2) & 1, (c >> 1) & 1, c & 1]
}

fn put(b: &mut [u8], at: usize, n: usize, v: u32) {
    for i in 0..n {
        b[at + i] = ((v >> i) & 1) as u8;
    }
}

fn get(b: &[u8], at: usize, n: usize) -> u32 {
    (0..n).fold(0, |a, i| a | (b[at + i] as u32) << i)
}

fn bw_code(w: u32) -> u32 {
    match w {
        4 => 1,
        8 => 2,
        16 => 3,
        _ => 0,
    }
}

/// The SIG bits (information, CRC-4 and six tail bits): 36 for S1G_1M, 52 for S1G_SHORT and S1G_LONG.
pub fn encode_sig(s: &Sig, pre: Preamble) -> Vec<u8> {
    let mut b;
    match pre {
        Preamble::OneM => {
            b = vec![0u8; 36];
            put(&mut b, 0, 2, s.nsts.saturating_sub(1) as u32);
            put(&mut b, 2, 1, s.sgi as u32);
            put(&mut b, 3, 1, s.ldpc as u32);
            put(&mut b, 4, 1, s.ldpc_extra as u32);
            put(&mut b, 5, 1, s.stbc as u32);
            put(&mut b, 6, 1, 1);
            put(&mut b, 7, 4, s.mcs as u32);
            put(&mut b, 11, 1, s.aggregated as u32);
            put(&mut b, 12, 9, s.length as u32);
            put(&mut b, 21, 2, s.response as u32);
            put(&mut b, 23, 1, s.smoothing as u32);
            put(&mut b, 24, 1, s.traveling as u32);
            put(&mut b, 25, 1, s.ndp as u32);
            let c = sig_crc_bits(&b[..26]);
            b[26..30].copy_from_slice(&c);
        }
        _ => {
            b = vec![0u8; 52];
            let long = pre == Preamble::Long;
            put(&mut b, 0, 1, if long { s.mu as u32 } else { 1 });
            put(&mut b, 1, 1, s.stbc as u32);
            put(&mut b, 2, 1, s.uplink as u32);
            put(&mut b, 3, 2, bw_code(s.bw));
            put(&mut b, 5, 2, s.nsts.saturating_sub(1) as u32);
            put(&mut b, 7, 9, s.id as u32);
            put(&mut b, 16, 1, s.sgi as u32);
            put(&mut b, 17, 1, s.ldpc as u32);
            put(&mut b, 18, 1, s.ldpc_extra as u32);
            put(&mut b, 19, 4, s.mcs as u32);
            put(
                &mut b,
                23,
                1,
                if long {
                    s.beamformed as u32
                } else {
                    s.smoothing as u32
                },
            );
            put(&mut b, 24, 1, s.aggregated as u32);
            put(&mut b, 25, 9, s.length as u32);
            put(&mut b, 34, 2, s.response as u32);
            if long {
                put(&mut b, 36, 1, s.smoothing as u32);
                put(&mut b, 37, 1, s.traveling as u32);
            } else {
                put(&mut b, 36, 1, s.traveling as u32);
                put(&mut b, 37, 1, s.ndp as u32);
            }
            put(&mut b, 38, 4, 0x0F);
            let c = sig_crc_bits(&b[..42]);
            b[42..46].copy_from_slice(&c);
        }
    }
    b
}

/// Reads and checks SIG bits heard at width `w` (4.6): `None` when refused.
pub fn decode_sig(b: &[u8], pre: Preamble, w: u32) -> Option<Sig> {
    let (info, total) = if pre == Preamble::OneM {
        (26, 36)
    } else {
        (42, 52)
    };
    if b.len() < total
        || sig_crc_bits(&b[..info]) != b[info..info + 4]
        || b[info + 4..total].iter().any(|&x| x != 0)
    {
        return None;
    }
    let mut s = Sig::default();
    match pre {
        Preamble::OneM => {
            if w != 1 || b[6] != 1 {
                return None;
            }
            s.nsts = get(b, 0, 2) as u8 + 1;
            s.sgi = b[2] == 1;
            s.ldpc = b[3] == 1;
            s.ldpc_extra = b[4] == 1;
            s.stbc = b[5] == 1;
            s.mcs = get(b, 7, 4) as u8;
            s.aggregated = b[11] == 1;
            s.length = get(b, 12, 9) as u16;
            s.response = get(b, 21, 2) as u8;
            s.smoothing = b[23] == 1;
            s.traveling = b[24] == 1;
            s.ndp = b[25] == 1;
            s.bw = 1;
        }
        _ => {
            let long = pre == Preamble::Long;
            if get(b, 38, 4) != 0x0F {
                return None;
            }
            if long {
                if b[0] == 1 {
                    return None;
                }
            } else if b[0] != 1 {
                return None;
            }
            s.bw = [2, 4, 8, 16][get(b, 3, 2) as usize];
            if s.bw != w {
                return None;
            }
            s.stbc = b[1] == 1;
            s.uplink = b[2] == 1;
            s.nsts = get(b, 5, 2) as u8 + 1;
            s.id = get(b, 7, 9) as u16;
            s.sgi = b[16] == 1;
            s.ldpc = b[17] == 1;
            s.ldpc_extra = b[18] == 1;
            s.mcs = get(b, 19, 4) as u8;
            s.aggregated = b[24] == 1;
            s.length = get(b, 25, 9) as u16;
            s.response = get(b, 34, 2) as u8;
            if long {
                s.beamformed = b[23] == 1;
                s.smoothing = b[36] == 1;
                s.traveling = b[37] == 1;
            } else {
                s.smoothing = b[23] == 1;
                s.traveling = b[36] == 1;
                s.ndp = b[37] == 1;
            }
        }
    }
    if s.stbc && (s.nsts - 1) % 2 == 0 {
        return None;
    }
    if s.mcs > 10 {
        return None;
    }
    if !s.ndp && !valid(w, s.mcs, s.nss()) {
        return None;
    }
    Some(s)
}

/// Returns the number of symbols of the data field: 0 for an NDP or for a width, MCS and stream count that have no rate.
pub fn data_symbols(s: &Sig, w: u32) -> usize {
    if s.ndp {
        return 0;
    }
    if s.aggregated {
        return s.length as usize;
    }
    let Some(d) = n_dbps(w, s.mcs, s.nss()) else {
        return 0;
    };
    let e = n_es(d);
    let mut n = (8 + 8 * s.length as usize + 6 * e).div_ceil(d);
    if s.stbc && n % 2 == 1 {
        n += 1;
    }
    n
}

/// Sample counts of a PPDU on its grid (N = FFT size).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// STF length.
    pub stf: usize,
    /// LTF1 length.
    pub ltf1: usize,
    /// SIG symbols.
    pub sig_syms: usize,
    /// Samples before the data field (from the STF start).
    pub pre_data: usize,
    /// Data symbol length.
    pub sym: usize,
    /// Data symbols.
    pub n_sym: usize,
}

impl Layout {
    /// The layout of a PPDU on its own grid.
    pub fn of(pre: Preamble, s: &Sig, w: u32) -> Layout {
        Layout::of_n(pre, s, w, grid(w).1)
    }
    /// The layout of a PPDU on a grid of `n` tones of 31.25 kHz.
    pub fn of_n(pre: Preamble, s: &Sig, w: u32, n: usize) -> Layout {
        let full = n + n / 4;
        let nltf = n_ltf(s.nsts);
        let (stf, ltf1, sig_syms) = match pre {
            Preamble::OneM => (5 * n, 5 * n, 6),
            _ => (5 * n / 2, 5 * n / 2, 2),
        };
        let extra = match pre {
            Preamble::Long => full * (1 + nltf + 1),
            _ => full * (nltf - 1),
        };
        Layout {
            stf,
            ltf1,
            sig_syms,
            pre_data: stf + ltf1 + sig_syms * full + extra,
            sym: if s.sgi { n + n / 8 } else { full },
            n_sym: data_symbols(s, w),
        }
    }
    /// The whole PPDU.
    pub fn total(&self) -> usize {
        self.pre_data + self.n_sym * self.sym
    }
}

/// Scrambled, coded, interleaved and mapped data symbols: for each symbol the points on the data tones (ascending). Panics for a
/// combination that [`valid`] refuses.
pub fn encode_data(psdu: &[u8], w: u32, mcs: u8, seed: u8) -> Vec<Vec<Complex32>> {
    let d = n_dbps(w, mcs, 1).expect("a valid combination");
    let e = n_es(d);
    let n_sym = (8 + 8 * psdu.len() + 6 * e).div_ceil(d);
    let mut bits = vec![0u8; 8];
    sdrkit_util_bits::pack::unpack(psdu, sdrkit_util_bits::BitOrder::Lsb, &mut bits);
    bits.resize(n_sym * d, 0);
    let mut s = scrambler(seed);
    for b in bits.iter_mut() {
        *b ^= s.bit();
    }
    let tail = 8 + 8 * psdu.len();
    for b in bits[tail..tail + 6 * e].iter_mut() {
        *b = 0;
    }
    let (bpsc, pattern, _) = mcs_params(mcs).expect("a valid MCS");
    // The encoders, punctured: encoder k takes the bits k, k + e, k + 2e ...
    let mut streams: Vec<Vec<u8>> = Vec::with_capacity(e);
    for k in 0..e {
        let input: Vec<u8> = bits.iter().skip(k).step_by(e).copied().collect();
        let mut c = Vec::new();
        encode(&input, &mut c);
        let mut p = Vec::new();
        punctured(&pattern, &c, &mut p);
        streams.push(p);
    }
    let sd = n_sd(w);
    let ncbps = if mcs == 10 { 12 } else { sd * bpsc };
    let s_blk = (bpsc / 2).max(1);
    let set = points(bpsc);
    let mut perms = Interleavers::default();
    let mut out = Vec::with_capacity(n_sym);
    let mut pos = vec![0usize; e];
    for _ in 0..n_sym {
        // The stream parser: blocks of s bits from each encoder in turn.
        let mut c = Vec::with_capacity(ncbps);
        while c.len() < ncbps {
            for (k, st) in streams.iter().enumerate() {
                for _ in 0..s_blk {
                    c.push(st.get(pos[k]).copied().unwrap_or(0));
                    pos[k] += 1;
                }
            }
        }
        c.truncate(ncbps);
        if mcs == 10 {
            let r: Vec<u8> = c.iter().zip(REP.iter()).map(|(&a, &b)| a ^ b).collect();
            c.extend(r);
        }
        out.push(map_symbol(&c, w, bpsc, &set, &mut perms));
    }
    out
}

fn map_symbol(
    coded: &[u8],
    w: u32,
    bpsc: usize,
    set: &sdrkit_algo_dsp::constellation::PointSet,
    perms: &mut Interleavers,
) -> Vec<Complex32> {
    let sd = n_sd(w);
    let mut pts = vec![Complex32::new(0.0, 0.0); sd];
    let segs = data_segments(w, sd);
    let s_blk = (bpsc / 2).max(1);
    // The 16 MHz segment parser: blocks of s bits to the two segments in turn.
    let mut parts: [Vec<u8>; 2] = [Vec::new(), Vec::new()];
    if segs[1].1 > 0 {
        let mut i = 0;
        while i < coded.len() {
            for p in parts.iter_mut() {
                p.extend_from_slice(&coded[i..(i + s_blk).min(coded.len())]);
                i += s_blk;
            }
        }
    } else {
        parts[0] = coded.to_vec();
    }
    for (si, &(off, n)) in segs.iter().enumerate() {
        if n == 0 {
            continue;
        }
        let ncbps = n * bpsc;
        let mut inter = vec![0u8; ncbps];
        let _ = perms
            .get(ncbps, bpsc, n_col(w))
            .apply(&parts[si][..ncbps], &mut inter);
        for t in 0..n {
            pts[off + t] = map(set, &inter[t * bpsc..(t + 1) * bpsc]);
        }
    }
    pts
}

/// Encodes the SIG bits into coded symbols: S1G_1M 6 symbols of 24 tone bits (12 coded bits and their repetition), the others 2
/// symbols of 52.
pub fn sig_symbols(bits: &[u8], pre: Preamble) -> Vec<Vec<u8>> {
    let mut c = Vec::new();
    encode(bits, &mut c);
    let mut perms = Interleavers::default();
    if pre == Preamble::OneM {
        let perm = perms.get(24, 1, 8);
        c.chunks(12)
            .map(|blk| {
                let mut r = blk.to_vec();
                r.extend(blk.iter().zip(REP.iter()).map(|(&a, &b)| a ^ b));
                let mut o = vec![0u8; 24];
                let _ = perm.apply(&r, &mut o);
                o
            })
            .collect()
    } else {
        let perm = perms.get(52, 1, 13);
        c.chunks(52)
            .map(|blk| {
                let mut o = vec![0u8; 52];
                let _ = perm.apply(blk, &mut o);
                o
            })
            .collect()
    }
}

/// Everything a PPDU is built from.
#[derive(Clone, Debug, PartialEq)]
pub struct PpduSpec {
    /// Width, MHz.
    pub width: u32,
    /// Preamble.
    pub preamble: Preamble,
    /// The SIG field (BW is set from the width).
    pub sig: Sig,
    /// The PSDU with its FCS (used when the data is coded).
    pub psdu: Vec<u8>,
    /// Scrambler seed.
    pub seed: u8,
    /// Beamforming phase tilt per tone (S1G_LONG), rad.
    pub tilt: f32,
}

fn ifft_into(
    fft: &mut Fft,
    n: usize,
    tones: &[(i32, Complex32)],
    scale: f32,
    out: &mut Vec<Complex32>,
) {
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    for &(k, v) in tones {
        x[((k + n as i32) % n as i32) as usize] += v;
    }
    fft.process(&mut x);
    out.clear();
    out.extend(x.iter().map(|v| v * scale));
}

/// The time-domain long training symbol of a width on its grid (N samples, the PPDU's scale).
pub fn ltf_time(w: u32) -> Vec<Complex32> {
    ltf_time_n(w, grid(w).1)
}

/// The time-domain long training symbol of a width on a grid of `n` tones of 31.25 kHz.
pub fn ltf_time_n(w: u32, n: usize) -> Vec<Complex32> {
    let mut f = Fft::inverse(n);
    let tones: Vec<(i32, Complex32)> = ltf_freq(w)
        .into_iter()
        .map(|(k, v)| (k, Complex32::new(v, 0.0)))
        .collect();
    let mut out = Vec::new();
    ifft_into(
        &mut f,
        n,
        &tones,
        1.0 / (plan(w).used.len() as f32).sqrt(),
        &mut out,
    );
    out
}

/// Builds a PPDU on its grid. A combination the width cannot send produces the preamble and SIG only; PPDUs whose data is not
/// coded here (several streams, STBC, LDPC, A-MPDU) get QPSK filler.
pub fn build_ppdu(p: &PpduSpec, rng: &mut sdrkit_algo_sim::rng::Rng) -> Vec<Complex32> {
    let w = p.width;
    let (_, n) = grid(w);
    let mut fft = Fft::inverse(n);
    let pl = plan(w);
    let scale = 1.0 / (pl.used.len() as f32).sqrt();
    let pol = pilot_polarity();
    let mut sig = p.sig;
    sig.bw = w;
    let lay = Layout::of(p.preamble, &sig, w);
    let mut out = Vec::with_capacity(lay.total());
    let mut sym = Vec::new();
    let tilt = |k: i32| Complex32::from_polar(1.0, p.tilt * k as f32);
    let push_sym = |out: &mut Vec<Complex32>, s: &[Complex32], gi: usize| {
        out.extend_from_slice(&s[n - gi..]);
        out.extend_from_slice(s);
    };
    // STF
    let stf = stf_freq(w);
    let e: f32 = stf.iter().map(|t| t.1.norm_sqr()).sum();
    ifft_into(&mut fft, n, &stf, 1.0 / e.sqrt(), &mut sym);
    for i in 0..lay.stf {
        out.push(sym[i % n]);
    }
    let stf_sym = sym.clone();
    // LTF1
    let ltf: Vec<(i32, Complex32)> = ltf_freq(w)
        .into_iter()
        .map(|(k, v)| (k, Complex32::new(v, 0.0)))
        .collect();
    ifft_into(&mut fft, n, &ltf, scale, &mut sym);
    let lts = sym.clone();
    if p.preamble == Preamble::OneM {
        for _ in 0..4 {
            push_sym(&mut out, &lts, n / 4);
        }
    } else {
        out.extend_from_slice(&lts[n / 2..]);
        out.extend_from_slice(&lts);
        out.extend_from_slice(&lts);
    }
    // SIG
    let sbits = encode_sig(&sig, p.preamble);
    let ssyms = sig_symbols(&sbits, p.preamble);
    let one = Complex32::new(1.0, 0.0);
    for (i, coded) in ssyms.iter().enumerate() {
        let mut tones = Vec::new();
        let rot = if p.preamble == Preamble::Long && i == 1 {
            Complex32::new(0.0, 1.0)
        } else {
            one
        };
        if p.preamble == Preamble::OneM {
            let p1 = plan(1);
            for (j, &k) in p1.data.iter().enumerate() {
                tones.push((
                    k,
                    Complex32::new(if coded[j] == 1 { 1.0 } else { -1.0 }, 0.0),
                ));
            }
            for (j, &k) in p1.pilots.iter().enumerate() {
                tones.push((k, Complex32::new(p1.pilot_vals[j] * pol[i % 127], 0.0)));
            }
        } else {
            let p2 = plan(2);
            for (c, g) in segments(w) {
                for (j, &k) in p2.data.iter().enumerate() {
                    if pl.used.contains(&(c + k)) {
                        let v = Complex32::new(if coded[j] == 1 { 1.0 } else { -1.0 }, 0.0) * rot;
                        tones.push((c + k, v * g));
                    }
                }
                for (j, &k) in p2.pilots.iter().enumerate() {
                    if pl.used.contains(&(c + k)) {
                        tones.push((
                            c + k,
                            Complex32::new(p2.pilot_vals[j] * pol[i % 127], 0.0) * g,
                        ));
                    }
                }
            }
        }
        ifft_into(&mut fft, n, &tones, scale, &mut sym);
        push_sym(&mut out, &sym, n / 4);
    }
    let nltf = n_ltf(sig.nsts);
    let mut z = lay.sig_syms;
    match p.preamble {
        Preamble::Long => {
            // D-STF, D-LTFs and SIG-B, beamformed.
            let t: Vec<(i32, Complex32)> = stf.iter().map(|&(k, v)| (k, v * tilt(k))).collect();
            ifft_into(&mut fft, n, &t, 1.0 / e.sqrt(), &mut sym);
            push_sym(&mut out, &sym, n / 4);
            let t: Vec<(i32, Complex32)> = ltf.iter().map(|&(k, v)| (k, v * tilt(k))).collect();
            ifft_into(&mut fft, n, &t, scale, &mut sym);
            for _ in 0..nltf {
                push_sym(&mut out, &sym, n / 4);
            }
            let t: Vec<(i32, Complex32)> = pl
                .data
                .iter()
                .enumerate()
                .map(|(j, &k)| {
                    (
                        k,
                        Complex32::new(if j % 3 == 0 { -1.0 } else { 1.0 }, 0.0) * tilt(k),
                    )
                })
                .chain(pl.pilots.iter().enumerate().map(|(j, &k)| {
                    (
                        k,
                        Complex32::new(pl.pilot_vals[j] * pol[z % 127], 0.0) * tilt(k),
                    )
                }))
                .collect();
            ifft_into(&mut fft, n, &t, scale, &mut sym);
            push_sym(&mut out, &sym, n / 4);
            z += 1;
        }
        _ => {
            for _ in 1..nltf {
                push_sym(&mut out, &lts, n / 4);
            }
        }
    }
    let _ = stf_sym;
    let gi = if sig.sgi { n / 8 } else { n / 4 };
    let coded_ok = !sig.ndp
        && sig.nss() == 1
        && !sig.stbc
        && !sig.ldpc
        && !sig.aggregated
        && sig.length > 0
        && valid(w, sig.mcs, 1);
    let data_syms: Vec<Vec<Complex32>> = if coded_ok {
        encode_data(&p.psdu, w, sig.mcs, p.seed)
    } else if lay.n_sym > 0 && valid(w, sig.mcs, sig.nss()) {
        let q = points(2);
        (0..lay.n_sym)
            .map(|_| {
                (0..pl.data.len())
                    .map(|_| map(&q, &[rng.bit(), rng.bit()]))
                    .collect()
            })
            .collect()
    } else {
        Vec::new()
    };
    for d in &data_syms {
        let mut tones: Vec<(i32, Complex32)> = pl
            .data
            .iter()
            .zip(d.iter())
            .map(|(&k, &v)| (k, v))
            .collect();
        for (j, &k) in pl.pilots.iter().enumerate() {
            tones.push((k, Complex32::new(pl.pilot_vals[j] * pol[z % 127], 0.0)));
        }
        if p.preamble == Preamble::Long {
            for t in tones.iter_mut() {
                t.1 *= tilt(t.0);
            }
        }
        ifft_into(&mut fft, n, &tones, scale, &mut sym);
        push_sym(&mut out, &sym, gi);
        z += 1;
    }
    out
}

/// The pilot polarity sequence p_0 .. p_126 of the 802.11 OFDM PHYs (clause 17.3.5.10), as +1 and -1.
pub fn pilot_polarity() -> [f32; 127] {
    sdrkit_algo_modem::ofdm::ieee80211_pilot_polarity().map(|v| v as f32)
}
