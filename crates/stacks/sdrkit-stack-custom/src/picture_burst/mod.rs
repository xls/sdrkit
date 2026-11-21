//! `picture_burst`: one picture in one burst of about 10 s. A single carrier at 158 ksymbols a second (632 ksps, 4 samples a
//! symbol) carries a header sent 16 times and up to 8 layers of LDPC blocks (n = 7680), robust first, of an embedded wavelet
//! picture stream; a pilot follows every 31 data symbols, and a pair of chirps marks the start and the end of the burst.

pub mod rx;
pub mod stack;

pub use stack::PICTURE_BURST;

use crate::family::{self, ldpc, push_bits, qam, read_bits};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_fec::conv::codes::K7_171_133_165_117;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_core::Complex32;
use sdrkit_util_bits::xorshift::Xorshift64;
use std::sync::OnceLock;

/// The sample rate.
pub const FS: f64 = 632_000.0;
/// Samples a symbol.
pub const SPS: usize = 4;
/// Symbols a second.
pub const SYMBOL_RATE: f64 = 158_000.0;
/// Samples of one sweep.
pub const SWEEP: usize = 25_280;
/// Samples of a guard.
pub const GUARD: usize = 48;
/// The pulse centre of slot 0.
pub const SLOT0: usize = 2 * SWEEP + GUARD;
/// Data symbols of the 16 header copies.
pub const HEADER_SYMBOLS: usize = 18_944;
/// Header bits.
pub const HEADER_BITS: usize = 290;
/// Coded bits of one header copy.
pub const HEADER_CODED: usize = 1184;
/// The code length of the layer blocks.
pub const N: usize = 7680;
/// The sweep bandwidth, Hz.
pub const SWEEP_HZ: f64 = 170_000.0;
/// The sweep rate, Hz/s.
pub const SWEEP_RATE: f64 = SWEEP_HZ / 0.04;
/// Seconds of a burst.
pub const SECONDS: f64 = 10.0;
/// The scrambler constant.
pub const SCRAMBLE_KEY: u64 = 0x4B_4149_5453_4352;

/// The modulations by header code.
pub const BITS: [usize; 6] = [1, 2, 4, 6, 8, 10];

/// One layer of the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layer {
    /// Modulation code: 0 BPSK, 1 QPSK, 2 16-QAM, 3 64-QAM, 4 256-QAM, 5 1024-QAM.
    pub modulation: u8,
    /// LDPC rate index.
    pub rate: u8,
    /// Repetitions, 1 to 8.
    pub reps: u8,
    /// Blocks, 0 to 511.
    pub blocks: u16,
}

impl Layer {
    /// Bits a symbol.
    pub fn bits(self) -> usize {
        BITS[usize::from(self.modulation)]
    }

    /// Symbols of a block.
    pub fn block_symbols(self) -> usize {
        N / self.bits() * usize::from(self.reps)
    }

    /// The information bits of a block.
    pub fn k(self) -> usize {
        let (a, d) = ldpc::RATES[usize::from(self.rate)];
        N * a / d
    }

    /// Payload bytes of a block.
    pub fn payload(self) -> usize {
        (self.k() - 16) / 8
    }
}

/// The profiles: (modulation, rate index, repetitions, share %) of each layer.
pub const PROFILES: [&[(u8, u8, u8, u32)]; 4] = [
    &[(0, 0, 8, 20), (0, 0, 4, 8), (0, 0, 2, 8), (0, 0, 1, 6), (1, 0, 1, 7), (1, 3, 1, 8), (1, 6, 1, 22), (2, 4, 1, 21)],
    &[(0, 0, 8, 30), (0, 0, 4, 15), (0, 0, 2, 15), (0, 0, 1, 12), (1, 0, 1, 12), (1, 3, 1, 16)],
    &[(0, 0, 4, 6), (0, 0, 1, 5), (1, 0, 1, 5), (1, 3, 1, 8), (1, 6, 1, 20), (2, 4, 1, 22), (2, 5, 1, 14), (3, 5, 1, 20)],
    &[(0, 0, 8, 5), (0, 0, 2, 3), (1, 0, 1, 3), (1, 6, 1, 89)],
];

/// The layer plan of profile `p` for a burst of `seconds`.
pub fn plan(p: usize, seconds: f64) -> Vec<Layer> {
    let slots = ((seconds * FS - (4 * SWEEP) as f64 - (2 * GUARD) as f64) / 4.0).floor() as usize;
    let data = slots * 31 / 32;
    let budget = data - HEADER_SYMBOLS;
    let mut layers: Vec<Layer> = Vec::new();
    for &(m, r, reps, share) in PROFILES[p] {
        if share == 0 {
            continue;
        }
        let mut l = Layer {
            modulation: m,
            rate: r,
            reps,
            blocks: 0,
        };
        let b = (budget * share as usize / 100) / l.block_symbols();
        l.blocks = b.clamp(1, 511) as u16;
        layers.push(l);
    }
    let used = |ls: &[Layer]| ls.iter().map(|l| usize::from(l.blocks) * l.block_symbols()).sum::<usize>();
    if let Some(last) = layers.len().checked_sub(1) {
        while layers[last].blocks < 511 && used(&layers) + layers[last].block_symbols() <= budget {
            layers[last].blocks += 1;
        }
    }
    layers
}

/// The fields of a header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// Up to 9 characters.
    pub callsign: String,
    /// Picture width.
    pub width: u32,
    /// Picture height.
    pub height: u32,
    /// Wavelet levels.
    pub levels: u32,
    /// Top plane (-1: no coded plane).
    pub top_plane: i32,
    /// Profile number.
    pub profile: u8,
    /// The layers.
    pub layers: Vec<Layer>,
    /// The codec decision count.
    pub decisions: u32,
}

fn call_code(c: u8) -> u64 {
    match c {
        b'A'..=b'Z' => u64::from(c - b'A' + 1),
        b'0'..=b'9' => u64::from(c - b'0' + 27),
        b'-' => 38,
        b'.' => 39,
        _ => 37,
    }
}

/// The 290 header bits.
pub fn header_bits(h: &Header) -> Vec<u8> {
    let mut v = Vec::with_capacity(HEADER_BITS);
    push_bits(&mut v, 1, 4);
    let call = h.callsign.to_ascii_uppercase();
    for i in 0..9 {
        push_bits(&mut v, call.as_bytes().get(i).map_or(0, |&c| call_code(c)), 6);
    }
    push_bits(&mut v, u64::from(h.width), 13);
    push_bits(&mut v, u64::from(h.height), 13);
    push_bits(&mut v, u64::from(h.levels), 3);
    push_bits(&mut v, (h.top_plane + 1).max(0) as u64, 4);
    push_bits(&mut v, u64::from(h.profile), 3);
    push_bits(&mut v, h.layers.len() as u64, 4);
    for i in 0..8 {
        let l = h.layers.get(i).copied().unwrap_or(Layer {
            modulation: 0,
            rate: 0,
            reps: 1,
            blocks: 0,
        });
        push_bits(&mut v, u64::from(l.modulation), 3);
        push_bits(&mut v, u64::from(l.rate), 3);
        push_bits(&mut v, u64::from(l.reps.max(1) - 1), 3);
        push_bits(&mut v, u64::from(l.blocks), 9);
    }
    push_bits(&mut v, u64::from(h.decisions), 32);
    let crc = family::crc16(&family::pack(&v));
    push_bits(&mut v, u64::from(crc), 16);
    v
}

/// The header of 290 bits, when it passes every check.
pub fn parse_header(b: &[u8]) -> Option<Header> {
    if b.len() < HEADER_BITS || family::crc16(&family::pack(&b[..274])) != read_bits(b, 274, 16) as u16 {
        return None;
    }
    if read_bits(b, 0, 4) != 1 {
        return None;
    }
    let mut call = String::new();
    for i in 0..9 {
        let c = read_bits(b, 4 + 6 * i, 6) as u8;
        match c {
            0 => {}
            1..=26 => call.push((b'A' + c - 1) as char),
            27..=36 => call.push((b'0' + c - 27) as char),
            37 => call.push('/'),
            38 => call.push('-'),
            39 => call.push('.'),
            _ => return None,
        }
    }
    let n = read_bits(b, 94, 4) as usize;
    let (w, ht, lv) = (read_bits(b, 58, 13) as u32, read_bits(b, 71, 13) as u32, read_bits(b, 84, 3) as u32);
    if n > 8 || w == 0 || ht == 0 || lv == 0 {
        return None;
    }
    let mut layers = Vec::with_capacity(n);
    for i in 0..8 {
        let at = 98 + 18 * i;
        let l = Layer {
            modulation: read_bits(b, at, 3) as u8,
            rate: read_bits(b, at + 3, 3) as u8,
            reps: read_bits(b, at + 6, 3) as u8 + 1,
            blocks: read_bits(b, at + 9, 9) as u16,
        };
        if l.modulation > 5 || l.rate > 6 {
            return None;
        }
        if i < n {
            if l.k() <= 16 || (l.reps > 1 && l.modulation != 0) {
                return None;
            }
            layers.push(l);
        }
    }
    Some(Header {
        callsign: call,
        width: w,
        height: ht,
        levels: lv,
        top_plane: read_bits(b, 87, 4) as i32 - 1,
        profile: read_bits(b, 91, 3) as u8,
        layers,
        decisions: read_bits(b, 242, 32) as u32,
    })
}

/// `count` bits of the scrambler of seed `seed`.
pub fn scrambler(seed: u32, count: usize) -> Vec<u8> {
    let state = Xorshift64::GOLDEN.wrapping_mul(u64::from(seed) + 1) ^ SCRAMBLE_KEY;
    let mut g = Xorshift64::new(state);
    let mut w = 0u64;
    (0..count)
        .map(|i| {
            if i % 64 == 0 {
                w = g.step();
            }
            ((w >> (i % 64)) & 1) as u8
        })
        .collect()
}

/// The sign of pilot `i`: +1 or -1.
pub fn pilot(i: u64) -> f32 {
    let mut z = i.wrapping_mul(Xorshift64::GOLDEN).wrapping_add(0x123_4567);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    if z & 1 == 1 {
        -1.0
    } else {
        1.0
    }
}

/// Slots for `data` data symbols.
pub fn slots_for(data: usize) -> usize {
    data + data / 31
}

/// The coded header copy (before the scrambler).
pub fn header_coded(h: &Header) -> Vec<u8> {
    let mut e = Encoder::new(K7_171_133_165_117);
    let mut out = Vec::with_capacity(HEADER_CODED);
    e.encode(&header_bits(h), &mut out);
    e.terminate(&mut out);
    out
}

/// The scrambler seed of block `g`, repetition `r`.
pub fn block_seed(g: usize, r: usize) -> u32 {
    (0x10000 + 8 * g + r) as u32
}

/// The data symbols of the header and of every layer block of `stream` (padded to the capacity).
pub fn data_symbols(h: &Header, stream: &[u8]) -> Vec<Complex32> {
    let mut out = Vec::new();
    let coded = header_coded(h);
    for c in 0..16 {
        let s = scrambler(0x4000 + c, HEADER_CODED);
        out.extend(coded.iter().zip(&s).map(|(&b, &k)| Complex32::new(if b ^ k == 0 { 1.0 } else { -1.0 }, 0.0)));
    }
    let mut at = 0;
    let mut g = 0;
    for l in &h.layers {
        let code = ldpc::code(N, usize::from(l.rate)).expect("family code of n = 7680");
        let set = qam::point_set(l.bits()).expect("family map");
        for _ in 0..l.blocks {
            let p = l.payload();
            let bytes: Vec<u8> = (0..p).map(|i| stream.get(at + i).copied().unwrap_or(0)).collect();
            at += p;
            let mut info = Vec::with_capacity(l.k());
            family::push_bytes(&mut info, &bytes);
            push_bits(&mut info, u64::from(family::crc16(&bytes)), 16);
            let mut cw = vec![0u8; N];
            code.encode(&info, &mut cw);
            for r in 0..usize::from(l.reps) {
                let s = scrambler(block_seed(g, r), N);
                let bits: Vec<u8> = cw.iter().zip(&s).map(|(a, b)| a ^ b).collect();
                qam::map(&set, l.bits(), &bits, &mut out);
            }
            g += 1;
        }
    }
    out
}

/// The slots: the data symbols with a pilot after every 31.
pub fn slots(data: &[Complex32]) -> Vec<Complex32> {
    let mut out = Vec::with_capacity(slots_for(data.len()));
    for (d, &v) in data.iter().enumerate() {
        out.push(v);
        if d % 31 == 30 {
            out.push(Complex32::new(pilot((d / 31) as u64), 0.0));
        }
    }
    out
}

/// The up sweep (binary32).
pub fn up_sweep() -> &'static [Complex32] {
    static S: OnceLock<Vec<Complex32>> = OnceLock::new();
    S.get_or_init(|| {
        let k = SWEEP_HZ / 0.04;
        (0..SWEEP)
            .map(|n| {
                let t = n as f64 / FS;
                let phi = std::f64::consts::PI * k * t * t - std::f64::consts::PI * SWEEP_HZ * t;
                let a = if n < 1000 {
                    0.5 - 0.5 * (std::f64::consts::PI * n as f64 / 1000.0).cos()
                } else if n >= SWEEP - 1000 {
                    0.5 - 0.5 * (std::f64::consts::PI * (SWEEP - 1 - n) as f64 / 1000.0).cos()
                } else {
                    1.0
                };
                Complex32::new((a * phi.cos()) as f32, (a * phi.sin()) as f32)
            })
            .collect()
    })
}

/// The RRC: roll-off 0.2, 12 symbols, 4 samples a symbol, unit energy (49 taps).
pub fn rrc() -> &'static [f32] {
    static H: OnceLock<Vec<f32>> = OnceLock::new();
    H.get_or_init(|| taps(Shape::Rrc { beta: 0.2 }, SPS as f64, 12.0, Norm::Energy).expect("RRC taps"))
}

/// The burst of slot symbols `s` at 632 ksps, not scaled.
pub fn burst(s: &[Complex32]) -> Vec<Complex32> {
    let e = SLOT0 + GUARD + 4 * s.len();
    let mut x = vec![Complex32::new(0.0, 0.0); e + 2 * SWEEP];
    let up = up_sweep();
    for (i, &v) in up.iter().enumerate() {
        x[i] = v;
        x[SWEEP + i] = v.conj();
        x[e + i] = v;
        x[e + SWEEP + i] = v.conj();
    }
    let h = rrc();
    for (k, &v) in s.iter().enumerate() {
        let c = SLOT0 + 4 * k - 24;
        let v = v * 2.0;
        for (i, &t) in h.iter().enumerate() {
            x[c + i] += v * t;
        }
    }
    x
}

/// The test picture: smooth gradients, rings and bars, `w` by `h`, RGB.
pub fn test_picture(w: usize, h: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f64 / w as f64, y as f64 / h as f64);
            let r = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt();
            let ring = (0.5 + 0.5 * (r * 40.0).cos()) * 80.0;
            let bar = if (x / (w / 16).max(1)).is_multiple_of(2) { 40.0 } else { 0.0 };
            v.push((fx * 175.0 + ring).min(255.0) as u8);
            v.push((fy * 200.0 + bar).min(255.0) as u8);
            v.push(((1.0 - fx) * 120.0 + ring * 0.5 + bar).min(255.0) as u8);
        }
    }
    v
}

/// A burst ready to send: its samples at 632 ksps (not scaled), the header and the padded stream.
#[derive(Clone, Debug)]
pub struct Made {
    /// The samples.
    pub samples: Vec<Complex32>,
    /// The header.
    pub header: Header,
    /// The codec stream, padded to the capacity.
    pub stream: Vec<u8>,
}

/// Why a burst cannot be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MakeError {
    /// The callsign is longer than 9 characters or has another character than A-Z, 0-9, / - and .
    Callsign,
    /// The profile is not 0 to 3.
    Profile,
    /// The picture is not 1 to 8191 pixels a side or the codec refused it.
    Picture,
}

impl std::fmt::Display for MakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MakeError::Callsign => "the callsign must be up to 9 characters of A-Z, 0-9, /, - and .",
            MakeError::Profile => "the profile must be 0 to 3",
            MakeError::Picture => "the picture cannot be coded",
        })
    }
}

impl std::error::Error for MakeError {}

/// The callsign as sent: upper case, no trailing spaces; an error for more than 9 characters or another character.
pub fn callsign(c: &str) -> Result<String, MakeError> {
    let c = c.trim_end().to_ascii_uppercase();
    if c.len() > 9 || !c.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b"/-.".contains(&b)) {
        return Err(MakeError::Callsign);
    }
    Ok(c)
}

/// Makes the burst of an RGB picture `w` by `h` with profile `p` and `call`.
pub fn make(rgb: &[u8], w: u32, h: u32, p: usize, call: &str) -> Result<Made, MakeError> {
    let call = callsign(call)?;
    if p > 3 {
        return Err(MakeError::Profile);
    }
    let layers = plan(p, SECONDS);
    let cap: usize = layers.iter().map(|l| usize::from(l.blocks) * l.payload()).sum();
    let mut stream = Vec::new();
    let ph = sdrkit_codec_custom::picture::Encoder::new()
        .encode(rgb, w, h, cap, &mut stream)
        .map_err(|_| MakeError::Picture)?;
    stream.resize(cap, 0);
    let header = Header {
        callsign: call,
        width: ph.width,
        height: ph.height,
        levels: ph.levels,
        top_plane: ph.top_plane,
        profile: p as u8,
        layers,
        decisions: ph.decisions,
    };
    let s = slots(&data_symbols(&header, &stream));
    Ok(Made {
        samples: burst(&s),
        header,
        stream,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_of(bits: &[u8]) -> String {
        family::pack(bits).iter().map(|b| format!("{b:02X}")).collect()
    }

    fn p0_header() -> Header {
        Header {
            callsign: "LY1ABC/P".into(),
            width: 1920,
            height: 1080,
            levels: 5,
            top_plane: 11,
            profile: 0,
            layers: plan(0, SECONDS),
            decisions: 0xDEAD_BEEF,
        }
    }

    #[test]
    fn plans_of_the_four_profiles() {
        let want: [&[(u16, usize)]; 4] = [
            &[(4, 952), (3, 714), (7, 1666), (11, 2618), (27, 6426), (30, 19_140), (85, 73_270), (214, 153_652)],
            &[(7, 1666), (7, 1666), (14, 3332), (23, 5474), (46, 10_948), (71, 45_298)],
            &[(2, 476), (9, 2142), (19, 4522), (30, 19_140), (77, 66_374), (170, 122_060), (108, 86_184), (264, 210_672)],
            &[(1, 238), (2, 476), (11, 2618), (352, 303_424)],
        ];
        for p in 0..4 {
            let l = plan(p, SECONDS);
            let got: Vec<(u16, usize)> = l.iter().map(|l| (l.blocks, usize::from(l.blocks) * l.payload())).collect();
            assert_eq!(got, want[p].to_vec(), "{p}");
            let data: usize = HEADER_SYMBOLS + l.iter().map(|l| usize::from(l.blocks) * l.block_symbols()).sum::<usize>();
            assert_eq!(data, 1_505_024);
            assert_eq!(slots_for(data), 1_553_573);
        }
    }

    #[test]
    fn header_scrambler_and_pilot_vectors() {
        let h = p0_header();
        let mut b = header_bits(&h);
        assert_eq!(hex_of(&b), "131970108395000F00438B8200E040180C02070002C801B2C078E0555035B7AB6FBBF5DD40");
        assert_eq!(hex_of(&header_coded(&h)[..64]), "000FAE261CE9FB31");
        assert_eq!(parse_header(&b), Some(h));
        b[40] ^= 1;
        assert_eq!(parse_header(&b), None);
        for (seed, want) in [(0x4000, "9EEFB56C5AB646A8"), (0x400F, "607EB5D2D81E2E1C"), (0x10000, "9F68E2CA58AEC90E")] {
            assert_eq!(hex_of(&scrambler(seed, 64)), want);
        }
        assert_eq!(hex_of(&scrambler(0x10009, 128)), "ED64EAA835D925085CA3686E8FC2FD2F");
        let signs: Vec<u8> = (0..64).map(|i| u8::from(pilot(i) < 0.0)).collect();
        assert_eq!(hex_of(&signs), "5B0B6E027527BF4F");
    }

    #[test]
    fn sweep_values_and_the_header_of_the_slot_vectors() {
        let s = up_sweep();
        assert!((s[1000].re - 0.462_312_85).abs() < 1e-5 && (s[1000].im + 0.886_716_9).abs() < 1e-5);
        assert!((s[12_640].re - 1.0).abs() < 1e-5 && s[12_640].im.abs() < 1e-4);
        assert!(s[0].norm() < 1e-9 && s[SWEEP - 1].norm() < 1e-9);
        // The profile 3 header of the slot vectors parses.
        let mut bits = Vec::new();
        let hx = "13197144D4000002800B4B6D00E0100808800B38580000000000000000000000EA70D62280";
        for c in hx.chars() {
            push_bits(&mut bits, u64::from(c.to_digit(16).unwrap()), 4);
        }
        let h = parse_header(&bits).unwrap();
        assert_eq!((h.callsign.as_str(), h.width, h.height, h.levels, h.top_plane, h.decisions), ("LY1TST", 320, 180, 5, 10, 240_067));
        assert_eq!(h.layers, plan(3, SECONDS));
        assert_eq!(header_bits(&h)[..HEADER_BITS], bits[..HEADER_BITS]);
    }
}
