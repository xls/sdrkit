//! The DECT-2020 NR physical layer that the `dect_nrplus` stack reads, for µ = 1, β = 1 and one stream: numerology, the STF,
//! the DRS of the first symbol, the PCC cells, scrambling, the turbo code and rate matching of the physical layer control
//! field, and the fields of control field types 1 and 2.
//! Standard: ETSI TS 103 636-3 V1.5.1 clauses 5 to 7; ETSI TS 103 636-4 V1.5.1 clause 6.2.

use sdrkit_algo_fec::llr::Llr8;
use sdrkit_algo_fec::ratematch::{LteCode, LteRateMatch};
use sdrkit_algo_fec::turbo::Turbo;
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC16_XMODEM;
use sdrkit_util_bits::seq::gold_3gpp;

/// The sample rate, Hz.
pub const RATE_HZ: f64 = 1.728e6;
/// FFT points.
pub const NFFT: usize = 64;
/// Cyclic prefix, samples.
pub const CP: usize = 8;
/// One OFDM symbol with its prefix, samples.
pub const SYMBOL: usize = NFFT + CP;
/// The STF, samples.
pub const STF_LEN: usize = 112;
/// Occupied carriers.
pub const OCCUPIED: usize = 56;
/// PCC bits.
pub const PCC_BITS: usize = 196;
/// The cover of the 7 repetitions of the STF.
pub const COVER: [f32; 7] = [1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0];
/// The values of the 14 STF carriers (−28, −24 .. −4, 4 .. 28) before the factor e^(jπ/4).
const STF_SIGNS: [f32; 14] = [
    1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0,
];
/// The DRS sequence.
pub const DRS: [i8; 56] = [
    1, 1, 1, 1, -1, 1, 1, -1, -1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, 1, -1, 1, -1, 1, 1, 1, 1, 1, -1,
    1, -1, -1, 1, 1, -1, -1, -1, -1, 1, -1, -1, -1, -1, -1, 1, 1, 1, -1, 1, 1, -1, -1, 1, -1, -1,
    -1,
];
/// The PCC scrambling initial value (TS 103 636-3 clause 7.5.4): "DECT" in ASCII.
pub const PCC_CINIT: u32 = 0x4445_4354;
/// The CRC masks: none, closed-loop MIMO, beamforming (TS 103 636-3 clauses 7.5.2.2 and 7.5.2.3).
pub const MASKS: [u16; 3] = [0x0000, 0x5555, 0xAAAA];

/// The carrier of occupied index `k` (0 to 55): −28 to −1, then 1 to 28.
pub fn carrier(k: usize) -> i32 {
    if k < 28 {
        k as i32 - 28
    } else {
        k as i32 - 27
    }
}

/// The FFT bin of carrier `c`.
pub fn bin(c: i32) -> usize {
    c.rem_euclid(NFFT as i32) as usize
}

/// The 16 samples of the STF base sequence S at unit mean power.
pub fn stf_base() -> [Complex32; 16] {
    let rot = Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_4);
    let carriers: Vec<(i32, Complex32)> = (-7..=7)
        .filter(|&i| i != 0)
        .zip(STF_SIGNS)
        .map(|(i, s)| (4 * i, rot * s))
        .collect();
    let mut s = [Complex32::default(); 16];
    for (n, o) in s.iter_mut().enumerate() {
        for &(c, v) in &carriers {
            let p = std::f32::consts::TAU * c as f32 * n as f32 / NFFT as f32;
            *o += v * Complex32::from_polar(1.0, p);
        }
        *o /= (carriers.len() as f32).sqrt();
    }
    s
}

/// The occupied indices of the DRS of the first OFDM symbol.
pub fn drs_indices() -> impl Iterator<Item = usize> {
    (0..14).map(|i| 4 * i)
}

/// The 98 PCC cells: (symbol 1 or 2, occupied index), in order.
pub fn pcc_cells() -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = (0..OCCUPIED)
        .filter(|k| k % 4 != 0)
        .map(|k| (1, k))
        .collect();
    v.extend((0..OCCUPIED).map(|k| (2, k)));
    v
}

/// The PCC scrambling sequence for `c_init`.
pub fn scrambling(c_init: u32) -> [u8; PCC_BITS] {
    let mut c = [0u8; PCC_BITS];
    gold_3gpp(c_init, &mut c);
    c
}

/// The QPSK symbol of bits `b0`, `b1`.
pub fn qpsk(b0: u8, b1: u8) -> Complex32 {
    let k = std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(
        k * (1.0 - 2.0 * f32::from(b0)),
        k * (1.0 - 2.0 * f32::from(b1)),
    )
}

/// The 16 CRC bits of `field` (most significant first) XOR `mask`.
pub fn crc_bits(field: &[u8], mask: u16) -> [u8; 16] {
    let v = CRC16_XMODEM.bits(field) ^ mask;
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = (v >> (15 - i) & 1) as u8;
    }
    out
}

/// The 196 PCC bits of control field `field` (40 or 80 bits) with CRC mask `mask`, before scrambling.
pub fn encode(field: &[u8], mask: u16) -> [u8; PCC_BITS] {
    let k = field.len() + 16;
    let mut c = field.to_vec();
    c.extend(crc_bits(field, mask));
    let turbo = Turbo::new(k).expect("K = 56 or 96 is a size of the turbo code");
    let mut streams = vec![0u8; 3 * (k + 4)];
    turbo.encode(&c, &mut streams).expect("the lengths match");
    let rm = LteRateMatch::new(LteCode::Turbo, k + 4, 0).expect("the stream length is valid");
    let mut e = [0u8; PCC_BITS];
    rm.rate_match(&streams, 0, rm.buffer_len(), &mut e)
        .expect("the buffer is valid");
    e
}

/// A control field read: its bits and the CRC mask that checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The 40 or 80 field bits.
    pub field: Vec<u8>,
    /// The CRC mask.
    pub mask: u16,
}

/// The turbo decoder, the rate matcher and the buffers for one control field size.
struct Size {
    /// The field bits, A.
    a: usize,
    turbo: Turbo,
    rm: LteRateMatch,
    streams: Vec<i8>,
    bits: Vec<u8>,
}

impl Size {
    fn new(a: usize) -> Self {
        let k = a + 16;
        Size {
            a,
            turbo: Turbo::new(k).expect("K = 56 or 96 is a size of the turbo code"),
            rm: LteRateMatch::new(LteCode::Turbo, k + 4, 0).expect("the stream length is valid"),
            streams: vec![0; 3 * (k + 4)],
            bits: vec![0; k],
        }
    }
}

/// Decoder of the PCC. It holds a turbo decoder and a rate matcher for each of the two control field sizes (A = 40, K = 56;
/// A = 80, K = 96), made when the component is made, so `read` allocates nothing.
pub struct PccDecoder {
    sizes: [Size; 2],
}

impl Default for PccDecoder {
    fn default() -> Self {
        PccDecoder {
            sizes: [Size::new(40), Size::new(80)],
        }
    }
}

impl PccDecoder {
    /// Decodes PCC LLRs (descrambled, positive means 0): A = 40, then 80; turbo decoding with 8 iterations, then the CRC with
    /// each mask. Returns the field bits (valid until the next call) and the mask that checked.
    pub fn read(&mut self, llr: &[Llr8; PCC_BITS]) -> Option<(&[u8], u16)> {
        for z in &mut self.sizes {
            let a = z.a;
            z.streams.fill(0);
            z.rm.dematch(llr, 0, z.rm.buffer_len(), &mut z.streams)
                .ok()?;
            let mut mask = None;
            let ok = z
                .turbo
                .decode(
                    &z.streams,
                    8,
                    |b| {
                        let rx = b[a..].iter().fold(0u16, |v, &x| v << 1 | u16::from(x));
                        let m = CRC16_XMODEM.bits(&b[..a]) ^ rx;
                        if MASKS.contains(&m) {
                            mask = Some(m);
                            true
                        } else {
                            false
                        }
                    },
                    &mut z.bits,
                )
                .is_ok();
            if let (true, Some(mask)) = (ok, mask) {
                return Some((&z.bits[..a], mask));
            }
        }
        None
    }
}

/// One-shot form of [`PccDecoder::read`] for the tests: it makes a decoder for each call.
#[cfg(test)]
pub fn decode(llr: &[Llr8; PCC_BITS]) -> Option<Decoded> {
    let mut d = PccDecoder::default();
    let (field, mask) = d.read(llr)?;
    Some(Decoded {
        field: field.to_vec(),
        mask,
    })
}

/// The fields of a physical layer control field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlField {
    /// Type 1 (40 bits) or type 2 (80 bits).
    pub kind: u8,
    /// The header format.
    pub format: u8,
    /// The packet length is in slots (else subslots).
    pub slots: bool,
    /// The packet length (the field + 1).
    pub length: u8,
    /// The short network id.
    pub network: u8,
    /// The transmitter identity.
    pub transmitter: u16,
    /// The transmit power field.
    pub power: u8,
    /// The MCS of the data field.
    pub mcs: u8,
    /// Type 2: the receiver identity.
    pub receiver: Option<u16>,
    /// Type 2: spatial streams field, redundancy version, new data indication, HARQ process, feedback format and information.
    pub harq: Option<(u8, u8, bool, u8, u8, u16)>,
}

fn take(b: &[u8], at: &mut usize, n: usize) -> u32 {
    let v = b[*at..*at + n]
        .iter()
        .fold(0u32, |v, &x| v << 1 | u32::from(x));
    *at += n;
    v
}

fn put(out: &mut Vec<u8>, v: u32, n: usize) {
    out.extend((0..n).rev().map(|i| (v >> i & 1) as u8));
}

impl ControlField {
    /// The fields of 40 or 80 bits.
    pub fn read(b: &[u8]) -> Option<ControlField> {
        if b.len() != 40 && b.len() != 80 {
            return None;
        }
        let at = &mut 0;
        let format = take(b, at, 3) as u8;
        let slots = take(b, at, 1) == 1;
        let length = take(b, at, 4) as u8 + 1;
        let network = take(b, at, 8) as u8;
        let transmitter = take(b, at, 16) as u16;
        let power = take(b, at, 4) as u8;
        if b.len() == 40 {
            take(b, at, 1);
            let mcs = take(b, at, 3) as u8;
            return Some(ControlField {
                kind: 1,
                format,
                slots,
                length,
                network,
                transmitter,
                power,
                mcs,
                receiver: None,
                harq: None,
            });
        }
        let mcs = take(b, at, 4) as u8;
        let receiver = take(b, at, 16) as u16;
        let streams = take(b, at, 2) as u8;
        let rv = take(b, at, 2) as u8;
        let ndi = take(b, at, 1) == 1;
        let process = take(b, at, 3) as u8;
        let fb_format = take(b, at, 4) as u8;
        let fb_info = take(b, at, 12) as u16;
        Some(ControlField {
            kind: 2,
            format,
            slots,
            length,
            network,
            transmitter,
            power,
            mcs,
            receiver: Some(receiver),
            harq: Some((streams, rv, ndi, process, fb_format, fb_info)),
        })
    }

    /// The bits of the field (40 for type 1, 80 for type 2).
    pub fn bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(80);
        put(&mut b, u32::from(self.format), 3);
        put(&mut b, u32::from(self.slots), 1);
        put(&mut b, u32::from(self.length.saturating_sub(1)), 4);
        put(&mut b, u32::from(self.network), 8);
        put(&mut b, u32::from(self.transmitter), 16);
        put(&mut b, u32::from(self.power), 4);
        match (self.receiver, self.harq) {
            (Some(rx), Some((s, rv, ndi, p, ff, fi))) => {
                put(&mut b, u32::from(self.mcs), 4);
                put(&mut b, u32::from(rx), 16);
                put(&mut b, u32::from(s), 2);
                put(&mut b, u32::from(rv), 2);
                put(&mut b, u32::from(ndi), 1);
                put(&mut b, u32::from(p), 3);
                put(&mut b, u32::from(ff), 4);
                put(&mut b, u32::from(fi), 12);
            }
            _ => {
                put(&mut b, 0, 1);
                put(&mut b, u32::from(self.mcs), 3);
            }
        }
        b
    }

    /// The id: "network 5A, RD 1234".
    pub fn id(&self) -> String {
        format!("network {:02X}, RD {:04X}", self.network, self.transmitter)
    }

    /// The text: "Type 2 format 0, to 5678, length 3 subslots, MCS 2, HARQ 1 rv 0, feedback format 0", with the mask.
    pub fn text(&self, mask: u16) -> String {
        let unit = if self.slots { "slots" } else { "subslots" };
        let mut t = match (self.receiver, self.harq) {
            (Some(rx), Some((_, rv, _, p, ff, _))) => format!(
                "Type 2 format {}, to {rx:04X}, length {} {unit}, MCS {}, HARQ {p} rv {rv}, feedback format {ff}",
                self.format, self.length, self.mcs
            ),
            _ => format!(
                "Type 1 format {}, length {} {unit}, MCS {}, power {}",
                self.format, self.length, self.mcs, self.power
            ),
        };
        t.push_str(match mask {
            0x5555 => ", closed-loop MIMO",
            0xAAAA => ", beamformed",
            _ => "",
        });
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type1() -> ControlField {
        ControlField {
            kind: 1,
            format: 0,
            slots: false,
            length: 4,
            network: 0x5A,
            transmitter: 0x1234,
            power: 5,
            mcs: 2,
            receiver: None,
            harq: None,
        }
    }

    #[test]
    fn the_worked_type_1_field_and_its_crc() {
        let b = type1().bits();
        let v = b.iter().fold(0u64, |v, &x| v << 1 | u64::from(x));
        assert_eq!(v, 0x03_5A12_3452);
        assert_eq!(CRC16_XMODEM.bits(&b), 0x6DA7);
        assert_eq!(ControlField::read(&b), Some(type1()));
    }

    #[test]
    fn the_code_comes_back_for_both_sizes_and_every_mask() {
        let t2 = ControlField {
            kind: 2,
            receiver: Some(0x5678),
            harq: Some((0, 0, true, 1, 0, 0)),
            ..type1()
        };
        for f in [type1(), t2] {
            for mask in MASKS {
                let e = encode(&f.bits(), mask);
                let llr: [i8; PCC_BITS] = e.map(|b| if b == 1 { -40 } else { 40 });
                let d = decode(&llr).unwrap();
                assert_eq!(d.mask, mask);
                assert_eq!(ControlField::read(&d.field), Some(f));
            }
        }
        assert_eq!(
            t2.text(0x5555),
            "Type 2 format 0, to 5678, length 4 subslots, MCS 2, HARQ 1 rv 0, feedback format 0, closed-loop MIMO"
        );
    }

    #[test]
    fn the_cells_and_the_stf() {
        assert_eq!(pcc_cells().len(), 98);
        assert_eq!(
            (carrier(0), carrier(27), carrier(28), carrier(55)),
            (-28, -1, 1, 28)
        );
        let s = stf_base();
        let p: f32 = s.iter().map(|z| z.norm_sqr()).sum::<f32>() / 16.0;
        assert!((p - 1.0).abs() < 1e-3, "{p}");
    }
}

#[cfg(test)]
mod noise {
    use super::*;

    #[test]
    fn the_code_reads_the_field_at_0_db_es_n0() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
        let f: Vec<u8> = (0..40).map(|i| ((i * 7) % 3 % 2) as u8).collect();
        let e = encode(&f, 0);
        let mut ok = 0;
        for _ in 0..50 {
            // Each bit on one axis of a QPSK cell of unit energy: amplitude 1/sqrt(2), noise variance 1/2 per axis.
            let a = std::f32::consts::FRAC_1_SQRT_2;
            let llr: [i8; PCC_BITS] = std::array::from_fn(|i| {
                let y = a * (1.0 - 2.0 * f32::from(e[i])) + 0.5f32.sqrt() * rng.normal_f32();
                (16.0 * a * y).round().clamp(-127.0, 127.0) as i8
            });
            if decode(&llr).is_some_and(|d| d.field == f) {
                ok += 1;
            }
        }
        assert!(ok >= 45, "{ok}");
    }
}
