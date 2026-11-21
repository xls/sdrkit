//! The physical layer of `qam_video`: profiles, the frame (Zadoff-Chu preamble, convolutionally coded header, data region with
//! pilots), the LDPC blocks with their CRC, the interleaver and scrambler of 7680 bits, the V-shaped sweep signature, and the
//! modulator at 4 samples a symbol.

use crate::family::{self, ldpc, push_bits, qam, read_bits};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_fec::conv::codes::K7_171_133;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_modem::zadoff_chu::zadoff_chu;
use sdrkit_core::Complex32;
use sdrkit_util_bits::xorshift::Xorshift64;
use std::f64::consts::TAU;
use std::sync::OnceLock;

/// Samples a symbol.
pub const SPS: usize = 4;
/// Preamble symbols.
pub const PREAMBLE: usize = 32;
/// Header symbols.
pub const HEADER: usize = 140;
/// Coded bits a frame.
pub const FRAME_BITS: usize = 7680;
/// LDPC blocks a frame.
pub const BLOCKS: usize = 4;
/// The block length.
pub const N: usize = 1920;
/// The pilot seed.
pub const PILOT_SEED: u64 = 0xA11_CE0F_FEE5;
/// The interleaver seed.
pub const PERM_SEED: u64 = 0x1920_C0DE;
/// The scrambler seed.
pub const SCRAMBLE_SEED: u64 = 0x5C4A_3B1E_2D0F;
/// The amplitude of the sweep signature (10 dB below the data).
pub const SWEEP_AMP: f64 = 0.316_227_766_016_837_94;

/// A profile: symbol rate and normal modulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    /// 700 ksym/s, 16-QAM.
    A,
    /// 1050 ksym/s, 16-QAM.
    B,
    /// 1400 ksym/s, 16-QAM.
    C,
    /// 1400 ksym/s, 64-QAM.
    D,
}

impl Profile {
    /// Every profile by header code.
    pub const ALL: [Profile; 4] = [Profile::A, Profile::B, Profile::C, Profile::D];

    /// The header code.
    pub fn code(self) -> u8 {
        Profile::ALL.iter().position(|&p| p == self).unwrap_or(0) as u8
    }

    /// Symbols a second.
    pub fn symbol_rate(self) -> f64 {
        match self {
            Profile::A => 700_000.0,
            Profile::B => 1_050_000.0,
            Profile::C | Profile::D => 1_400_000.0,
        }
    }

    /// The letter.
    pub fn letter(self) -> char {
        (b'A' + self.code()) as char
    }
}

/// A profile and a waveform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Waveform {
    /// The profile.
    pub profile: Profile,
    /// Robust form: QPSK and LDPC rate 1/2.
    pub robust: bool,
}

impl Waveform {
    /// Bits a data symbol.
    pub fn bits(self) -> usize {
        match (self.robust, self.profile) {
            (true, _) => 2,
            (false, Profile::D) => 6,
            _ => 4,
        }
    }

    /// The LDPC rate index (rate 1/2 or 3/4).
    pub fn rate(self) -> usize {
        if self.robust {
            2
        } else {
            4
        }
    }

    /// The information length of a block.
    pub fn k(self) -> usize {
        if self.robust {
            960
        } else {
            1440
        }
    }

    /// The bytes of a cell (a block payload).
    pub fn cell(self) -> usize {
        (self.k() - 16) / 8
    }

    /// Data symbols a frame.
    pub fn data_symbols(self) -> usize {
        FRAME_BITS / self.bits()
    }

    /// Pilots a frame.
    pub fn pilots(self) -> usize {
        self.data_symbols().div_ceil(15) + 1
    }

    /// Symbols a frame.
    pub fn frame_symbols(self) -> usize {
        PREAMBLE + HEADER + self.data_symbols() + self.pilots()
    }

    /// Frames in a sweep period.
    pub fn sweep_frames(self) -> usize {
        ((0.5 * self.profile.symbol_rate() / self.frame_symbols() as f64).round() as usize).max(1)
    }
}

/// The fields of a frame header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The profile and waveform.
    pub waveform: Waveform,
    /// The FEC profile index, 0 to 7.
    pub fec: u8,
    /// The frame index in the sweep period.
    pub sweep: u16,
    /// The frame counter.
    pub seq: u16,
    /// Reported Es/N0 in 0.25 dB steps; 0 unknown.
    pub link: u8,
}

/// The 64 header bits.
pub fn header_bits(h: &Header) -> Vec<u8> {
    let mut v = Vec::with_capacity(64);
    push_bits(&mut v, 0, 2);
    push_bits(&mut v, u64::from(h.waveform.profile.code()), 2);
    push_bits(&mut v, u64::from(h.waveform.robust), 1);
    push_bits(&mut v, u64::from(h.fec), 3);
    push_bits(&mut v, u64::from(h.sweep), 10);
    push_bits(&mut v, u64::from(h.seq), 16);
    push_bits(&mut v, u64::from(h.link), 8);
    push_bits(&mut v, 0, 6);
    let crc = family::crc16(&family::pack(&v));
    push_bits(&mut v, u64::from(crc), 16);
    v
}

/// The header of 64 bits, when its CRC holds and its version is 0.
pub fn parse_header(bits: &[u8]) -> Option<Header> {
    if bits.len() < 64 || family::crc16(&family::pack(&bits[..48])) != read_bits(bits, 48, 16) as u16 {
        return None;
    }
    if read_bits(bits, 0, 2) != 0 {
        return None;
    }
    Some(Header {
        waveform: Waveform {
            profile: Profile::ALL[read_bits(bits, 2, 2) as usize],
            robust: bits[4] == 1,
        },
        fec: read_bits(bits, 5, 3) as u8,
        sweep: read_bits(bits, 8, 10) as u16,
        seq: read_bits(bits, 18, 16) as u16,
        link: read_bits(bits, 34, 8) as u8,
    })
}

/// The 140 coded header bits: rate 1/2, K = 7, with the tail.
pub fn header_coded(h: &Header) -> Vec<u8> {
    let mut e = Encoder::new(K7_171_133);
    let mut out = Vec::with_capacity(HEADER);
    e.encode(&header_bits(h), &mut out);
    e.terminate(&mut out);
    out
}

/// The preamble: Zadoff-Chu root 7, length 32.
pub fn preamble() -> &'static [Complex32] {
    static P: OnceLock<Vec<Complex32>> = OnceLock::new();
    P.get_or_init(|| zadoff_chu(7, PREAMBLE))
}

/// The pilot signs, 2048 of them (+1 or -1).
pub fn pilots() -> &'static [f32] {
    static P: OnceLock<Vec<f32>> = OnceLock::new();
    P.get_or_init(|| {
        let mut g = Xorshift64::new(PILOT_SEED);
        (0..2048).map(|_| if (g.step() >> 40) & 1 == 1 { -1.0 } else { 1.0 }).collect()
    })
}

/// The interleaver: coded bit `i` goes to position `perm[i]`.
pub fn permutation() -> &'static [u16] {
    static P: OnceLock<Vec<u16>> = OnceLock::new();
    P.get_or_init(|| {
        let mut p: Vec<u16> = (0..FRAME_BITS as u16).collect();
        let mut g = Xorshift64::new(PERM_SEED);
        for i in (1..FRAME_BITS).rev() {
            let j = (g.step() % (i as u64 + 1)) as usize;
            p.swap(i, j);
        }
        p
    })
}

/// The scrambler bits.
pub fn scrambler() -> &'static [u8] {
    static S: OnceLock<Vec<u8>> = OnceLock::new();
    S.get_or_init(|| {
        let mut g = Xorshift64::new(SCRAMBLE_SEED);
        (0..FRAME_BITS).map(|_| ((g.step() >> 41) & 1) as u8).collect()
    })
}

/// The 7680 transmitted bits of a frame payload (4 cells; a short payload is padded with zero bytes).
pub fn coded_bits(waveform: Waveform, payload: &[u8]) -> Vec<u8> {
    let code = ldpc::code(N, waveform.rate()).expect("the family code of n = 1920");
    let c = waveform.cell();
    let mut word = vec![0u8; FRAME_BITS];
    for b in 0..BLOCKS {
        let cell: Vec<u8> = (0..c).map(|i| payload.get(b * c + i).copied().unwrap_or(0)).collect();
        let mut info = Vec::with_capacity(waveform.k());
        family::push_bytes(&mut info, &cell);
        push_bits(&mut info, u64::from(family::crc16(&cell)), 16);
        code.encode(&info, &mut word[b * N..(b + 1) * N]);
    }
    transmit_bits(&word)
}

/// The transmitted bits of the four code words `word` (7680 bits): interleaved, then scrambled.
pub fn transmit_bits(word: &[u8]) -> Vec<u8> {
    let p = permutation();
    let s = scrambler();
    let mut tx = vec![0u8; FRAME_BITS];
    for i in 0..FRAME_BITS {
        tx[p[i] as usize] = word[i];
    }
    for i in 0..FRAME_BITS {
        tx[i] ^= s[i];
    }
    tx
}

/// True for the region positions that hold a pilot, and the pilot number.
pub fn region_layout(waveform: Waveform) -> Vec<Option<usize>> {
    let mut v = Vec::with_capacity(waveform.data_symbols() + waveform.pilots());
    let mut placed = 0;
    let mut pilot = 0;
    while placed < waveform.data_symbols() {
        v.push(Some(pilot));
        pilot += 1;
        let n = 15.min(waveform.data_symbols() - placed);
        v.extend(std::iter::repeat_n(None, n));
        placed += n;
    }
    v.push(Some(pilot));
    v
}

/// The symbols of a frame.
pub fn frame_symbols(h: &Header, payload: &[u8]) -> Vec<Complex32> {
    let waveform = h.waveform;
    let mut out = Vec::with_capacity(waveform.frame_symbols());
    out.extend_from_slice(preamble());
    out.extend(header_coded(h).iter().map(|&b| Complex32::new(if b == 0 { 1.0 } else { -1.0 }, 0.0)));
    let set = qam::point_set(waveform.bits()).expect("the family map");
    let mut data = Vec::with_capacity(waveform.data_symbols());
    qam::map(&set, waveform.bits(), &coded_bits(waveform, payload), &mut data);
    let mut d = data.into_iter();
    let pil = pilots();
    for slot in region_layout(waveform) {
        out.push(match slot {
            Some(i) => Complex32::new(pil[i], 0.0),
            None => d.next().unwrap_or_default(),
        });
    }
    out
}

/// The sweep signature at position `u` (symbols) of a period of `p` symbols.
pub fn chirp(u: f64, p: f64) -> Complex32 {
    let u = u.rem_euclid(p);
    let ph = if u < p / 2.0 {
        0.45 * (u - 2.0 * u * u / p)
    } else {
        let v = u - p / 2.0;
        0.45 * (-v + 2.0 * v * v / p)
    };
    let a = TAU * ph.rem_euclid(1.0);
    Complex32::new(a.cos() as f32, a.sin() as f32)
}

/// Fills `out[i]` with `chirp(u0 + i du, p)`. Within runs of 64 values that stay in one half of the period, the values come
/// from a second-order phasor recursion started from the exact value; other runs are computed one by one.
pub fn chirp_fill(u0: f64, du: f64, p: f64, out: &mut [Complex32]) {
    const RUN: usize = 64;
    let half = p / 2.0;
    let phase = |u: f64| {
        let u = u.rem_euclid(p);
        if u < half {
            0.45 * (u - 2.0 * u * u / p)
        } else {
            let v = u - half;
            0.45 * (-v + 2.0 * v * v / p)
        }
    };
    let cis = |c: f64| {
        let a = TAU * c.rem_euclid(1.0);
        Complex32::new(a.cos() as f32, a.sin() as f32)
    };
    for (r, run) in out.chunks_mut(RUN).enumerate() {
        let ua = u0 + (r * RUN) as f64 * du;
        let ub = ua + (run.len() - 1) as f64 * du;
        let (ha, hb) = ((ua / half).floor(), (ub / half).floor());
        if ha != hb || run.len() < 3 {
            for (i, o) in run.iter_mut().enumerate() {
                *o = chirp(ua + i as f64 * du, p);
            }
            continue;
        }
        // phase(u + du) - phase(u) changes by the constant second difference inside one half.
        let (f0, f1, f2) = (phase(ua), phase(ua + du), phase(ua + 2.0 * du));
        let mut v = cis(f0);
        let mut step = cis(f1 - f0);
        let turn = cis(f2 - 2.0 * f1 + f0);
        for o in run.iter_mut() {
            *o = v;
            v *= step;
            step *= turn;
        }
    }
}

/// The RRC: roll-off 0.25, 12 symbols, 4 samples a symbol, unit energy (49 taps).
pub fn rrc() -> &'static [f32] {
    static H: OnceLock<Vec<f32>> = OnceLock::new();
    H.get_or_init(|| taps(Shape::Rrc { beta: 0.25 }, SPS as f64, 12.0, Norm::Energy).expect("RRC taps"))
}

/// What the modulator sends.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    /// The profile and waveform.
    pub waveform: Waveform,
    /// The FEC profile index of the headers.
    pub fec: u8,
    /// The link quality value.
    pub link: u8,
    /// The sequence number of the first frame.
    pub first_seq: u16,
    /// The sweep index of the first frame.
    pub first_sweep: u16,
    /// Zero symbols before the frames.
    pub lead: usize,
    /// Zero symbols after the frames.
    pub tail: usize,
    /// The sweep signature on.
    pub chirp: bool,
}

/// The samples at 4 samples a symbol of `payloads`, one frame each.
pub fn modulate(b: &Burst, payloads: &[Vec<u8>]) -> Vec<Complex32> {
    let k = b.waveform.sweep_frames();
    let fs = b.waveform.frame_symbols();
    let mut sym = vec![Complex32::new(0.0, 0.0); b.lead];
    for (f, p) in payloads.iter().enumerate() {
        let h = Header {
            waveform: b.waveform,
            fec: b.fec,
            sweep: ((usize::from(b.first_sweep) + f) % k) as u16,
            seq: b.first_seq.wrapping_add(f as u16),
            link: b.link,
        };
        sym.extend(frame_symbols(&h, p));
    }
    sym.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), b.tail));
    let h = rrc();
    let len = SPS * sym.len();
    let mut out = vec![Complex32::new(0.0, 0.0); len];
    for (kk, &s) in sym.iter().enumerate() {
        if s == Complex32::new(0.0, 0.0) {
            continue;
        }
        let s = s * 2.0;
        for (i, &t) in h.iter().enumerate() {
            let n = (SPS * kk + i) as i64 - 24;
            if (0..len as i64).contains(&n) {
                out[n as usize] += s * t;
            }
        }
    }
    if b.chirp {
        let p = (k * fs) as f64;
        for f in 0..payloads.len() {
            let sw = (usize::from(b.first_sweep) + f) % k;
            let start = SPS * (b.lead + f * fs);
            for i in 0..SPS * fs {
                let u = (sw * fs) as f64 + i as f64 / SPS as f64;
                out[start + i] += chirp(u, p) * SWEEP_AMP as f32;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chirp_fill_follows_the_chirp() {
        for (u0, p) in [(0.0, 1000.0), (123.37, 4508.0), (-7.1, 333.0)] {
            let mut out = vec![Complex32::new(0.0, 0.0); 20_000];
            chirp_fill(u0, 0.25, p, &mut out);
            for (i, v) in out.iter().enumerate() {
                assert!((v - chirp(u0 + 0.25 * i as f64, p)).norm() < 2e-4, "{u0} {p} {i}");
            }
        }
    }

    fn bits(s: &str) -> Vec<u8> {
        s.bytes().map(|c| c - b'0').collect()
    }

    #[test]
    fn header_vector() {
        let h = Header {
            waveform: Waveform {
                profile: Profile::C,
                robust: true,
            },
            fec: 5,
            sweep: 777,
            seq: 54321,
            link: 99,
        };
        assert_eq!(header_bits(&h), bits("0010110111000010011101010000110001011000110000001101011101101011"));
        assert_eq!(
            header_coded(&h),
            bits("00001110001001100111001110100101110001110111001011001011101001001110001110010100101110001101101111010111010100011100111101100100100100011011")
        );
        assert_eq!(parse_header(&header_bits(&h)), Some(h));
        for i in 0..64 {
            let mut b = header_bits(&h);
            b[i] ^= 1;
            assert_eq!(parse_header(&b), None, "{i}");
        }
        let z = Header {
            waveform: Waveform {
                profile: Profile::A,
                robust: false,
            },
            fec: 0,
            sweep: 0,
            seq: 0,
            link: 0,
        };
        assert_eq!(read_bits(&header_bits(&z), 48, 16), 0x0E10);
    }

    #[test]
    fn derived_numbers_of_each_profile() {
        let table = [
            (Profile::A, false, 1920, 129, 2221, 158),
            (Profile::B, false, 1920, 129, 2221, 236),
            (Profile::C, false, 1920, 129, 2221, 315),
            (Profile::D, false, 1280, 87, 1539, 455),
            (Profile::A, true, 3840, 257, 4269, 82),
            (Profile::B, true, 3840, 257, 4269, 123),
            (Profile::C, true, 3840, 257, 4269, 164),
            (Profile::D, true, 3840, 257, 4269, 164),
        ];
        for (p, r, d, pil, fs, k) in table {
            let m = Waveform { profile: p, robust: r };
            assert_eq!((m.data_symbols(), m.pilots(), m.frame_symbols(), m.sweep_frames()), (d, pil, fs, k), "{p:?} {r}");
            assert_eq!(m.cell(), if r { 118 } else { 178 });
            assert_eq!(region_layout(m).len(), d + pil);
            assert_eq!(frame_symbols(&Header { waveform: m, fec: 1, sweep: 0, seq: 0, link: 0 }, &[]).len(), fs);
        }
    }

    #[test]
    fn tables_of_pilots_interleaver_and_scrambler() {
        let s: String = pilots()[..64].iter().map(|&v| if v > 0.0 { '+' } else { '-' }).collect();
        assert_eq!(s, "+-++---+++++-++++++-----------+-++-++++-+----+--+-+-+----+-++-+-");
        let pb: Vec<u8> = pilots().iter().map(|&v| u8::from(v < 0.0)).collect();
        assert_eq!(family::crc32(&pb), 0xB0A4_63D4);
        assert_eq!(&permutation()[..16], &[1475, 3969, 2072, 713, 2696, 7171, 6704, 4070, 3993, 7250, 1139, 7590, 6266, 4673, 2286, 4843]);
        let pl: Vec<u8> = permutation().iter().flat_map(|&v| u32::from(v).to_le_bytes()).collect();
        assert_eq!(family::crc32(&pl), 0x30B6_6493);
        let sc = scrambler();
        assert_eq!(sc[..64].to_vec(), bits("1010111011111110100101001101111010001001111011101000110000001111"));
        assert_eq!(family::crc32(sc), 0x0CDF_E683);
        let ones = sc.iter().filter(|&&b| b == 1).count() as f64 / FRAME_BITS as f64;
        assert!((0.45..0.55).contains(&ones));
    }

    #[test]
    fn coded_bits_of_the_vector_payloads() {
        for (robust, crc) in [(false, 0xB732_D9FDu32), (true, 0xC4C2_70B5)] {
            let m = Waveform {
                profile: Profile::A,
                robust,
            };
            let payload: Vec<u8> = (0..4 * m.cell()).map(|i| ((7 * i + 3) % 256) as u8).collect();
            assert_eq!(family::crc32(&coded_bits(m, &payload)), crc, "{robust}");
        }
    }

    #[test]
    fn the_sweep_signature() {
        let p = 1000.0;
        for (u, re, im) in [
            (0.0, 1.0, 0.0),
            (1.0, -0.949294, 0.314390),
            (10.0, -0.844328, 0.535827),
            (250.0, 0.0, 1.0),
            (500.0, 1.0, 0.0),
            (750.0, 0.0, -1.0),
            (999.5, 0.157831, -0.987466),
        ] {
            let c = chirp(u, p);
            assert!((c.re as f64 - re).abs() < 1e-5 && (c.im as f64 - im).abs() < 1e-5, "{u}");
        }
        assert!((chirp(1234.0, p) - chirp(234.0, p)).norm() < 1e-5);
        // The power with the sweep is about 1.1 times the power without.
        let m = Waveform {
            profile: Profile::A,
            robust: false,
        };
        let b = Burst {
            waveform: m,
            fec: 1,
            link: 0,
            first_seq: 0,
            first_sweep: 0,
            lead: 0,
            tail: 0,
            chirp: false,
        };
        let pay: Vec<Vec<u8>> = (0..3).map(|f| (0..712).map(|i| ((7 * i + 3 + 11 * f) % 256) as u8).collect()).collect();
        let pw = |x: &[Complex32]| x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len() as f64;
        let a = pw(&modulate(&b, &pay));
        let c = pw(&modulate(&Burst { chirp: true, ..b.clone() }, &pay));
        assert!((1.07..1.13).contains(&(c / a)), "{}", c / a);
        assert!((a - 1.0).abs() < 0.05, "{a}");
    }
}
