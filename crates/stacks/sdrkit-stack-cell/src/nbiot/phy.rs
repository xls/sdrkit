//! The NB-IoT downlink physical layer (3GPP TS 36.211 clause 10, TS 36.212 clause 6): the OFDM symbol layout at 1.92 Msps,
//! NPSS, NSSS, NRS, the resource element maps of NPBCH and NPDSCH, transmit diversity, and the channel coding of the MIB-NB and
//! SIB1-NB.

use crate::lte::phy::{append_crc16, append_crc24a, scrambling, tbcc_encode, Tbcc};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::{CRC16_XMODEM, CRC24_LTE_A};
use sdrkit_util_bits::seq::ZadoffChu;
use std::f64::consts::PI;

/// Samples per subframe at 1.92 Msps.
pub const SF: usize = 1920;
/// Samples per frame.
pub const FRAME: usize = 19200;
/// The rate.
pub const RATE: f64 = 1.92e6;

/// The start (within its subframe) of OFDM symbol `l` (0..13) and its cyclic prefix length.
pub fn symbol_start(l: usize) -> (usize, usize) {
    let slot = l / 7;
    let ls = l % 7;
    let start = slot * 960 + if ls == 0 { 0 } else { 138 + (ls - 1) * 137 };
    (start, if ls == 0 { 10 } else { 9 })
}

/// The useful part's start of symbol `l` within the subframe.
pub fn useful_start(l: usize) -> usize {
    let (s, cp) = symbol_start(l);
    s + cp
}

/// The NPSS cover code S(l) for l = 3..13.
pub const NPSS_COVER: [f32; 11] = [1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, 1.0];

/// The NPSS Zadoff-Chu sequence d(n), n = 0..10 (root 5), without the cover code.
pub fn npss_zc() -> [Complex32; 11] {
    let z = ZadoffChu::new(5, 11).expect("root 5 is prime to 11");
    let mut d = [Complex32::new(0.0, 0.0); 11];
    for (n, v) in d.iter_mut().enumerate() {
        let [re, im] = z.value(n as u64);
        *v = Complex32::new(re as f32, im as f32);
    }
    d
}

fn hadamard_row(row: usize, m: usize) -> f32 {
    // Sylvester Hadamard matrix of order 128: H[r][c] = (-1)^popcount(r & c).
    if (row & m).count_ones().is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// The NSSS of cell `pci` in a frame with `n_f` (132 values).
pub fn nsss(pci: u16, n_f: u32) -> [Complex32; 132] {
    let u = (pci as usize % 126) + 3;
    let q = pci as usize / 126;
    let row = [0usize, 31, 63, 127][q.min(3)];
    let theta = 33.0 / 132.0 * ((n_f / 2) % 4) as f64;
    let z = ZadoffChu::new(u as u32, 131).expect("131 is prime");
    let mut d = [Complex32::new(0.0, 0.0); 132];
    for (n, v) in d.iter_mut().enumerate() {
        let m = n % 128;
        let a = -2.0 * PI * theta * n as f64 + z.phase(n as u64);
        *v = Complex32::new(a.cos() as f32, a.sin() as f32) * hadamard_row(row, m);
    }
    d
}

/// The QPSK value of two bits.
#[inline]
pub fn qpsk(b0: u8, b1: u8) -> Complex32 {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(s * (1.0 - 2.0 * b0 as f32), s * (1.0 - 2.0 * b1 as f32))
}

/// The NRS value of port 2000 + `port` at symbol `l` of slot `ns` for cell `pci`, NRS index m (0 or 1).
pub fn nrs_value(pci: u16, ns: usize, l: usize, m: usize) -> Complex32 {
    let n = pci as u32;
    let lsym = (l % 7) as u32;
    let c_init = (1u32 << 10) * (7 * (ns as u32 + 1) + lsym + 1) * (2 * n + 1) + 2 * n + 1;
    let mp = m + 109;
    let c = scrambling(c_init, 2 * mp + 2);
    qpsk(c[2 * mp], c[2 * mp + 1])
}

/// The NRS subcarriers (0..11) of port `port` (0 or 1) in subframe symbol `l` (5, 6, 12 or 13).
pub fn nrs_subcarriers(pci: u16, port: usize, l: usize) -> [usize; 2] {
    let first = l % 7 == 5;
    let v = match (port, first) {
        (0, true) | (1, false) => 0,
        _ => 3,
    };
    let s = (v + pci as usize % 6) % 6;
    [s, s + 6]
}

/// Whether symbol `l` carries NRS.
pub fn nrs_symbol(l: usize) -> bool {
    matches!(l, 5 | 6 | 12 | 13)
}

/// Whether RE (k, l) is an NRS of either port.
pub fn is_nrs(pci: u16, k: usize, l: usize) -> bool {
    nrs_symbol(l) && (k % 3) == (pci as usize % 6) % 3
}

/// Whether RE (k, l) is an LTE CRS of ports 0 to `ports - 1` with frequency shift `shift` (in-band carriers).
pub fn is_crs(shift: usize, ports: usize, k: usize, l: usize) -> bool {
    let ls = l % 7;
    let on = ls == 0 || ls == 4 || (ports > 2 && ls == 1);
    on && (k % 3) == shift % 3
}

/// The NPBCH resource elements of subframe 0 in mapping order: (k, l).
pub fn npbch_res(pci: u16) -> Vec<(usize, usize)> {
    let mut v = Vec::with_capacity(100);
    for l in 3..14 {
        for k in 0..12 {
            if is_nrs(pci, k, l) || is_crs(pci as usize % 6, 4, k, l) {
                continue;
            }
            v.push((k, l));
        }
    }
    v
}

/// The NPDSCH resource elements of a subframe in mapping order: from `l_start`, leaving out the NRS of the `nrs_ports` ports in
/// use and (when `crs` is set) the LTE CRS of (shift, ports).
pub fn npdsch_res(
    pci: u16,
    nrs_ports: usize,
    l_start: usize,
    crs: Option<(usize, usize)>,
) -> Vec<(usize, usize)> {
    let mut v = Vec::with_capacity(160);
    for l in l_start..14 {
        for k in 0..12 {
            if nrs_symbol(l) && (0..nrs_ports).any(|p| nrs_subcarriers(pci, p, l).contains(&k)) {
                continue;
            }
            if let Some((s, p)) = crs {
                if is_crs(s, p, k, l) {
                    continue;
                }
            }
            v.push((k, l));
        }
    }
    v
}

/// The OFDM demodulator and modulator of the NB-IoT carrier (12 subcarriers at (k - 6 + 1/2) x 15 kHz).
pub struct Ofdm {
    fwd: Fft,
    inv: Fft,
    rot: Vec<Complex32>,
    buf: Vec<Complex32>,
}

impl Default for Ofdm {
    fn default() -> Self {
        Ofdm {
            fwd: Fft::forward(128),
            inv: Fft::inverse(128),
            rot: (0..128)
                .map(|n| {
                    let a = PI * n as f64 / 128.0;
                    Complex32::new(a.cos() as f32, a.sin() as f32)
                })
                .collect(),
            buf: vec![Complex32::new(0.0, 0.0); 128],
        }
    }
}

impl Ofdm {
    /// The 12 subcarrier values of the 128 useful samples at `x[at..]` (zeros past the end).
    pub fn demod(&mut self, x: &[Complex32], at: i64) -> [Complex32; 12] {
        for n in 0..128 {
            let i = at + n as i64;
            let s = if i >= 0 && (i as usize) < x.len() {
                x[i as usize]
            } else {
                Complex32::new(0.0, 0.0)
            };
            self.buf[n] = s * self.rot[n].conj();
        }
        self.fwd.process(&mut self.buf);
        let mut out = [Complex32::new(0.0, 0.0); 12];
        for (k, o) in out.iter_mut().enumerate() {
            let bin = (k as i64 - 6).rem_euclid(128) as usize;
            *o = self.buf[bin] / 128.0;
        }
        out
    }
    /// Appends symbol `l`'s samples (cyclic prefix and useful part) for subcarrier values `a`.
    pub fn modulate(&mut self, a: &[Complex32; 12], l: usize, out: &mut Vec<Complex32>) {
        self.buf
            .iter_mut()
            .for_each(|b| *b = Complex32::new(0.0, 0.0));
        for (k, &v) in a.iter().enumerate() {
            let bin = (k as i64 - 6).rem_euclid(128) as usize;
            self.buf[bin] = v;
        }
        self.inv.process(&mut self.buf);
        let (_, cp) = symbol_start(l);
        for n in -(cp as i64)..128 {
            let u = self.buf[n.rem_euclid(128) as usize];
            let a = PI * n as f64 / 128.0;
            out.push(u * Complex32::new(a.cos() as f32, a.sin() as f32));
        }
    }
}

/// Transmit diversity (36.211 6.3.4.3, two ports): the pair (x0, x1) on two consecutive REs, per port.
pub fn sfbc(x0: Complex32, x1: Complex32) -> [[Complex32; 2]; 2] {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    [[x0 * s, x1 * s], [-x1.conj() * s, x0.conj() * s]]
}

/// Soft bits (positive for 0) of `res` received values with channels `h[port][re]` and noise variance `noise`, for 1 or 2 ports.
pub fn soft_bits(
    y: &[Complex32],
    h: &[Vec<Complex32>],
    ports: usize,
    noise: f32,
    out: &mut Vec<f32>,
) {
    out.clear();
    let k = 2.0 * std::f32::consts::SQRT_2 / noise.max(1e-9);
    if ports == 1 {
        for (i, &r) in y.iter().enumerate() {
            let hh = h[0][i];
            let z = r * hh.conj();
            out.push(k * z.re);
            out.push(k * z.im);
        }
        return;
    }
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let mut i = 0;
    while i + 1 < y.len() {
        let (h0, h1) = ((h[0][i] + h[0][i + 1]) * 0.5, (h[1][i] + h[1][i + 1]) * 0.5);
        let (r0, r1) = (y[i], y[i + 1]);
        let x0 = h0.conj() * r0 + h1 * r1.conj();
        let x1 = h0.conj() * r1 - h1 * r0.conj();
        for x in [x0, x1] {
            out.push(k * s * x.re);
            out.push(k * s * x.im);
        }
        i += 2;
    }
    if i < y.len() {
        out.push(0.0);
        out.push(0.0);
    }
}

/// The 1600 coded bits of a MIB-NB (34 bits) for `ports` NRS ports.
pub fn encode_mib_nb(mib: &[u8; 34], ports: usize) -> Vec<u8> {
    let mut b = mib.to_vec();
    append_crc16(&mut b, if ports == 2 { 0xFFFF } else { 0 });
    tbcc_encode(&b, 1600)
}

/// Decodes 1600 soft values (zeros where not received): the MIB-NB and the port count, when the CRC checks with a mask.
pub fn decode_mib_nb(tbcc: &mut Tbcc, soft: &[f32], ports: usize) -> Option<[u8; 34]> {
    let mut bits = [0u8; 50];
    if !tbcc.decode(soft, 50, &mut bits) {
        return None;
    }
    let mask = if ports == 2 { 0xFFFF } else { 0 };
    if !CRC16_XMODEM.verify_bits_masked(&bits, mask) {
        return None;
    }
    let mut m = [0u8; 34];
    m.copy_from_slice(&bits[..34]);
    Some(m)
}

/// The NPBCH scrambling sequence (1600 values, c_init = PCI).
pub fn npbch_scrambling(pci: u16) -> Vec<u8> {
    scrambling(pci as u32, 1600)
}

/// The per-frame NPBCH phase rotation of later releases of 36.211: for each of the 100 symbols of frame `n_f`, from the
/// Gold sequence with c_init = (PCI + 1)((n_f mod 61) + 1) 2^9 + PCI, the pair (c(2i), c(2i + 1)) as a number 0 to 3: 0 means a
/// rotation by 1, 1 by -1, 2 by j, 3 by -j.
pub fn npbch_rotation(pci: u16, n_f: u32) -> Vec<u8> {
    let c_init = (pci as u32 + 1) * ((n_f % 61) + 1) * (1 << 9) + pci as u32;
    let c = scrambling(c_init, 200);
    c.chunks(2).map(|p| 2 * p[0] + p[1]).collect()
}

/// The rotation factor of a value of [`npbch_rotation`].
pub fn rotation_factor(r: u8) -> Complex32 {
    match r & 3 {
        0 => Complex32::new(1.0, 0.0),
        1 => Complex32::new(-1.0, 0.0),
        2 => Complex32::new(0.0, 1.0),
        _ => Complex32::new(0.0, -1.0),
    }
}

/// Takes the rotation out of a frame's soft values (two per symbol: the real part's bit, then the imaginary part's): each symbol
/// is multiplied by the conjugate rotation.
pub fn derotate_soft(soft: &mut [f32], rot: &[u8]) {
    for (s, &r) in soft.chunks_mut(2).zip(rot) {
        if s.len() < 2 {
            break;
        }
        let (a, b) = (s[0], s[1]);
        let (x, y) = match r & 3 {
            0 => (a, b),
            1 => (-a, -b),
            2 => (b, -a),
            _ => (-b, a),
        };
        s[0] = x;
        s[1] = y;
    }
}

/// SIB1-NB repetitions and transport block size of schedulingInfoSIB1 (36.213 Table 16.4.1.3-3, 16.4.1.5.2-1).
pub fn sib1_schedule(value: u8) -> Option<(u32, usize)> {
    if value > 11 {
        return None;
    }
    let reps = [4u32, 8, 16][value as usize % 3];
    let tbs = [208usize, 328, 440, 680][value as usize / 3];
    Some((reps, tbs))
}

/// The first SIB1-NB frame (n_f mod 256) for `reps` repetitions (36.213 Table 16.4.1.3-4).
pub fn sib1_start(reps: u32, pci: u16) -> u32 {
    match reps {
        4 => 16 * (pci as u32 % 4),
        8 => 16 * (pci as u32 % 2),
        _ => pci as u32 % 2,
    }
}

/// The NPDSCH scrambling of SIB1-NB for a repetition starting at frame `n_f`.
pub fn sib1_scrambling(pci: u16, n_f: u32, len: usize) -> Vec<u8> {
    let c_init = 0xFFFFu32 * (1 << 14) + (n_f % 2) * (1 << 13) + 4 * (1 << 9) + pci as u32;
    scrambling(c_init, len)
}

/// Encodes a SIB1-NB transport block (padded to `tbs` bits) into `e` bits.
pub fn encode_sib1(payload: &[u8], tbs: usize, e: usize) -> Vec<u8> {
    let mut b = payload.to_vec();
    b.resize(tbs, 0);
    append_crc24a(&mut b);
    tbcc_encode(&b, e)
}

/// Decodes a SIB1-NB transport block of `tbs` bits from soft values.
pub fn decode_sib1(tbcc: &mut Tbcc, soft: &[f32], tbs: usize) -> Option<Vec<u8>> {
    let mut bits = vec![0u8; tbs + 24];
    if !tbcc.decode(soft, tbs + 24, &mut bits) {
        return None;
    }
    CRC24_LTE_A.verify_bits(&bits).then(|| bits[..tbs].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        assert_eq!(symbol_start(7), (960, 10));
        assert_eq!(useful_start(13), 960 + 138 + 5 * 137 + 9);
        assert_eq!(npbch_res(100).len(), 100);
        assert_eq!(npbch_res(7).len(), 100);
        assert_eq!(npdsch_res(100, 2, 0, None).len(), 152);
        assert_eq!(npdsch_res(100, 1, 0, None).len(), 160);
    }

    #[test]
    fn sequences() {
        // Every identity's NSSS is distinct from the others.
        let a = nsss(0, 0);
        let b = nsss(1, 0);
        let c: f32 = a
            .iter()
            .zip(&b)
            .map(|(x, y)| (x * y.conj()).re)
            .sum::<f32>()
            .abs();
        assert!(c < 40.0, "{c}");
        let auto: f32 = a.iter().map(|x| x.norm_sqr()).sum();
        assert!((auto - 132.0).abs() < 1e-3);
        // The NPSS ZC has unit modulus.
        assert!(npss_zc().iter().all(|v| (v.norm() - 1.0).abs() < 1e-6));
    }

    #[test]
    fn ofdm_round_trip() {
        let mut o = Ofdm::default();
        let mut a = [Complex32::new(0.0, 0.0); 12];
        for (k, v) in a.iter_mut().enumerate() {
            *v = qpsk((k % 2) as u8, (k / 3 % 2) as u8);
        }
        let mut x = Vec::new();
        o.modulate(&a, 1, &mut x);
        let y = o.demod(&x, 9);
        for k in 0..12 {
            assert!((y[k] - a[k]).norm() < 1e-4);
        }
    }

    #[test]
    fn npbch_rotation_round_trips() {
        // Rotating a QPSK symbol and removing the rotation from its soft values returns the original soft values.
        let r = npbch_rotation(100, 37);
        assert_eq!(r.len(), 100);
        assert!(r.iter().all(|&v| v < 4));
        assert_ne!(r, npbch_rotation(100, 38));
        assert_eq!(npbch_rotation(100, 37), npbch_rotation(100, 37 + 61));
        for (i, &v) in r.iter().enumerate() {
            let x = Complex32::new(
                if i % 2 == 0 { 0.7 } else { -0.7 },
                if i % 3 == 0 { 0.7 } else { -0.7 },
            );
            let y = x * rotation_factor(v);
            let mut s = [y.re, y.im];
            derotate_soft(&mut s, &[v]);
            assert!(
                (s[0] - x.re).abs() < 1e-6 && (s[1] - x.im).abs() < 1e-6,
                "{v}"
            );
        }
    }

    #[test]
    fn mib_nb_coding() {
        let mut m = [0u8; 34];
        m[3] = 1;
        m[10] = 1;
        for ports in [1, 2] {
            let c = encode_mib_nb(&m, ports);
            let soft: Vec<f32> = c.iter().map(|&b| if b == 0 { 1.0 } else { -1.0 }).collect();
            assert_eq!(decode_mib_nb(&mut Tbcc::new(), &soft, ports), Some(m));
            assert_eq!(decode_mib_nb(&mut Tbcc::new(), &soft, 3 - ports), None);
        }
        assert_eq!(sib1_schedule(1), Some((8, 208)));
        assert_eq!(sib1_schedule(11), Some((16, 680)));
        assert_eq!(sib1_schedule(12), None);
        assert_eq!(sib1_start(8, 100), 0);
        assert_eq!(sib1_start(4, 17), 16);
    }
}
