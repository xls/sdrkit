//! Measurements of a bit stream whose format is not public: the repetition period, the constant (sync) part, and attempts with
//! the CRCs and checksums of `sdrkit_util_bits`.

use sdrkit_util_bits::checksum::{sum8, xor8};
use sdrkit_util_bits::crc::catalog::{self, Engine};
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::pack::pack;
use sdrkit_util_bits::{BitOrder, Pad};
use serde::{Deserialize, Serialize};

/// A check the frame analyser tries: a catalogue CRC, or a weak 8-bit checksum.
#[derive(Clone, Copy, Debug)]
pub enum Check {
    /// A CRC of the `sdrkit_util_bits` catalogue.
    Crc(Engine),
    /// The sum of the bytes, modulo 256.
    Sum8,
    /// The XOR of the bytes.
    Xor8,
}

impl Check {
    /// The name that the attempt reports: the catalogue name of a CRC.
    pub fn name(self) -> &'static str {
        match self {
            Check::Crc(e) => e.spec().name,
            Check::Sum8 => "sum of bytes",
            Check::Xor8 => "xor of bytes",
        }
    }
    /// The width of the check value in bytes.
    pub fn bytes(self) -> usize {
        match self {
            Check::Crc(e) => e.spec().width as usize / 8,
            _ => 1,
        }
    }
    /// A 16 or 32-bit check (strong) rather than an 8-bit one.
    pub fn strong(self) -> bool {
        self.bytes() > 1
    }
    /// The check value of `data`.
    pub fn compute(self, data: &[u8]) -> u64 {
        match self {
            Check::Crc(e) => e.checksum(data),
            Check::Sum8 => sum8(data) as u64,
            Check::Xor8 => xor8(data) as u64,
        }
    }
}

/// The checks tried, strong ones first.
pub static CATALOGUE: &[Check] = &[
    Check::Crc(Engine::U16(&catalog::CRC16_IBM_3740)),
    Check::Crc(Engine::U16(&catalog::CRC16_XMODEM)),
    Check::Crc(Engine::U16(&catalog::CRC16_KERMIT)),
    Check::Crc(Engine::U16(&catalog::CRC16_IBM_SDLC)),
    Check::Crc(Engine::U16(&catalog::CRC16_SPI_FUJITSU)),
    Check::Crc(Engine::U16(&catalog::CRC16_ARC)),
    Check::Crc(Engine::U16(&catalog::CRC16_MODBUS)),
    Check::Crc(Engine::U16(&catalog::CRC16_UMTS)),
    Check::Crc(Engine::U16(&catalog::CRC16_DNP)),
    Check::Crc(Engine::U16(&catalog::CRC16_EN_13757)),
    Check::Crc(Engine::U32(&catalog::CRC32_ISO_HDLC)),
    Check::Crc(Engine::U8(&catalog::CRC8_SMBUS)),
    Check::Crc(Engine::U8(&catalog::CRC8_MAXIM_DOW)),
    Check::Sum8,
    Check::Xor8,
];

/// The check named `name` in [`CATALOGUE`].
pub fn check_named(name: &str) -> Option<Check> {
    CATALOGUE.iter().copied().find(|c| c.name() == name)
}

/// Bits (as 0/1) to hex, most significant first, the last byte padded with zeros.
pub fn bits_hex(bits: &[u8]) -> String {
    let mut bytes = Vec::with_capacity(bits.len().div_ceil(8));
    pack(bits, BitOrder::Msb, Pad::Zero, &mut bytes);
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// The repetition period of a bit stream: (lag, z), or `None`.
pub fn period(bits: &[u8]) -> Option<(usize, f64)> {
    let n = bits.len();
    let ones = bits.iter().filter(|&&b| b == 1).count();
    if n == 0 || ones * 10 < n || ones * 10 > 9 * n {
        return None;
    }
    let mut best: Option<(usize, f64)> = None;
    let max = 2048.min(n / 5);
    let mut cols = vec![0u32; max + 1];
    for lag in 32..=max {
        let k = n / lag;
        if k < 2 {
            break;
        }
        cols[..lag].iter_mut().for_each(|c| *c = 0);
        for (i, &b) in bits[..k * lag].iter().enumerate() {
            cols[i % lag] += b as u32;
        }
        let kf = k as f64;
        let s: f64 = cols[..lag]
            .iter()
            .map(|&c| (2.0 * c as f64 - kf).powi(2) / kf)
            .sum();
        let z = (s - lag as f64) / (2.0 * lag as f64).sqrt();
        if z >= 9.0 && best.is_none_or(|b| z > b.1) {
            best = Some((lag, z));
        }
    }
    best
}

/// The best z over all lags (for reports on noise).
pub fn best_z(bits: &[u8]) -> f64 {
    let n = bits.len();
    let max = 2048.min(n / 5);
    let mut best = f64::MIN;
    for lag in 32..=max {
        let k = n / lag;
        if k < 2 {
            break;
        }
        let mut cols = vec![0u32; lag];
        for (i, &b) in bits[..k * lag].iter().enumerate() {
            cols[i % lag] += b as u32;
        }
        let kf = k as f64;
        let s: f64 = cols
            .iter()
            .map(|&c| (2.0 * c as f64 - kf).powi(2) / kf)
            .sum();
        best = best.max((s - lag as f64) / (2.0 * lag as f64).sqrt());
    }
    best
}

/// The constant part of a period: (start column, length, majority bits).
pub fn sync(bits: &[u8], lag: usize) -> Option<(usize, usize, Vec<u8>)> {
    let k = bits.len() / lag;
    if k < 2 {
        return None;
    }
    let mut good = vec![false; lag];
    let mut maj = vec![0u8; lag];
    for j in 0..lag {
        let ones = (0..k).filter(|&p| bits[p * lag + j] == 1).count();
        let dissent = ones.min(k - ones);
        good[j] = dissent * 12 <= k;
        maj[j] = (ones * 2 > k) as u8;
    }
    if good.iter().all(|&g| g) {
        return None; // constant everywhere: not a sync
    }
    // The longest cyclic run of good columns.
    let mut best = (0usize, 0usize);
    let start0 = good.iter().position(|&g| !g).unwrap_or(0);
    let mut run_start = 0;
    let mut run = 0;
    for i in 1..=lag {
        let j = (start0 + i) % lag;
        if good[j] {
            if run == 0 {
                run_start = j;
            }
            run += 1;
            if run > best.1 {
                best = (run_start, run);
            }
        } else {
            run = 0;
        }
    }
    let len = best.1.min(lag / 2);
    if len < 8 {
        return None;
    }
    let b: Vec<u8> = (0..len).map(|i| maj[(best.0 + i) % lag]).collect();
    Some((best.0, len, b))
}

/// A CRC attempt that fits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    pub name: String,
    pub inverted: bool,
    pub lsb_first: bool,
    pub with_sync: bool,
    pub little_endian: bool,
    pub passes: usize,
    pub frames: usize,
}

/// The whole bytes of a frame of bits: `inv` complements, `lsb` puts the first bit in each byte's least significant place.
pub fn frame_bytes(bits: &[u8], inv: bool, lsb: bool) -> Vec<u8> {
    let order = if lsb { BitOrder::Lsb } else { BitOrder::Msb };
    let mut out = Vec::with_capacity(bits.len() / 8);
    pack(bits, order, Pad::Drop, &mut out);
    if inv {
        out.iter_mut().for_each(|b| *b = !*b);
    }
    out
}

/// Whether frame bytes pass a check: the bytes after the first `skip` hold at least one data byte, then the check value, big or
/// little endian.
pub fn passes(c: Check, bytes: &[u8], skip: usize, little: bool) -> bool {
    let w = c.bytes();
    if bytes.len() < skip + w + 1 {
        return false;
    }
    let frame = &bytes[skip..];
    match c {
        Check::Crc(e) => e.verify(
            frame,
            if little {
                Trailer::Little
            } else {
                Trailer::Big
            },
        ),
        _ => c.compute(&frame[..frame.len() - 1]) == frame[frame.len() - 1] as u64,
    }
}

/// Tries every algorithm and option on the frames (bits each); the best attempt that fits.
pub fn attempt(frames: &[Vec<u8>], sync_bits: usize) -> Option<Attempt> {
    let mut best: Option<(Attempt, bool)> = None;
    let n = frames.len();
    // The plain options first, so of two equivalent fits (KERMIT, or XMODEM on reflected bytes) the plain one is reported.
    for inv in [false, true] {
        for lsb in [false, true] {
            for with_sync in [true, false] {
                for &c in CATALOGUE {
                    let skip = if with_sync { 0 } else { sync_bits / 8 };
                    if !with_sync && skip == 0 {
                        continue;
                    }
                    for little in [false, true] {
                        if !c.strong() && little {
                            continue;
                        }
                        let p = frames
                            .iter()
                            .filter(|f| passes(c, &frame_bytes(f, inv, lsb), skip, little))
                            .count();
                        let ok = if c.strong() {
                            n >= 3 && p * 2 >= n && p >= 3
                        } else {
                            n >= 8 && p == n
                        };
                        if !ok {
                            continue;
                        }
                        let better = match &best {
                            None => true,
                            Some((b, strong)) => {
                                p > b.passes || (p == b.passes && c.strong() && !strong)
                            }
                        };
                        if better {
                            best = Some((
                                Attempt {
                                    name: c.name().into(),
                                    inverted: inv,
                                    lsb_first: lsb,
                                    with_sync,
                                    little_endian: little,
                                    passes: p,
                                    frames: n,
                                },
                                c.strong(),
                            ));
                        }
                    }
                }
            }
        }
    }
    best.map(|b| b.0)
}

/// Whether frame bits pass a given attempt.
pub fn attempt_passes(a: &Attempt, bits: &[u8], sync_bits: usize) -> bool {
    let Some(c) = check_named(&a.name) else {
        return false;
    };
    let skip = if a.with_sync { 0 } else { sync_bits / 8 };
    passes(
        c,
        &frame_bytes(bits, a.inverted, a.lsb_first),
        skip,
        a.little_endian,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    fn to_bits(bytes: &[u8], lsb: bool, inv: bool) -> Vec<u8> {
        let mut v = Vec::new();
        for &b in bytes {
            for i in 0..8 {
                let bit = if lsb {
                    (b >> i) & 1
                } else {
                    (b >> (7 - i)) & 1
                };
                v.push(bit ^ inv as u8);
            }
        }
        v
    }

    #[test]
    fn w10_check_values() {
        for &c in CATALOGUE {
            let want = match c {
                Check::Crc(e) => e.spec().check,
                Check::Sum8 => 0xDD,
                Check::Xor8 => 0x31,
            };
            assert_eq!(c.compute(b"123456789"), want, "{}", c.name());
        }
    }

    #[test]
    fn w10_period_sync_and_attempts() {
        let mut rng = Rng::new(5);
        // A period of 128 bits with a constant 32 bits from bit 37 (1ACF...).
        let pattern = [0x1Au8, 0xCF, 0xFC, 0x1D];
        let pb = to_bits(&pattern, false, false);
        let mut bits = Vec::new();
        for _ in 0..60 {
            for j in 0..128 {
                if (37..37 + 32).contains(&j) {
                    bits.push(pb[j - 37]);
                } else {
                    bits.push(rng.below(2) as u8);
                }
            }
        }
        let (lag, z) = period(&bits).unwrap();
        assert_eq!(lag, 128);
        assert!(z > 9.0);
        let (start, len, maj) = sync(&bits, 128).unwrap();
        assert_eq!(start, 37);
        assert!(len >= 32);
        assert!(bits_hex(&maj).starts_with("1ACF"));

        // KERMIT over 10 frames, plain.
        let kermit = check_named("CRC-16/KERMIT").unwrap();
        let mut frames = Vec::new();
        for _ in 0..10 {
            let mut d: Vec<u8> = (0..10).map(|_| rng.below(256) as u8).collect();
            let c = kermit.compute(&d);
            d.push((c >> 8) as u8);
            d.push(c as u8);
            frames.push(to_bits(&d, false, false));
        }
        let a = attempt(&frames, 0).unwrap();
        assert_eq!(
            (a.name.as_str(), a.passes, a.inverted, a.lsb_first),
            ("CRC-16/KERMIT", 10, false, false)
        );
        let mut f = frames[0].clone();
        f[20] ^= 1;
        assert!(!attempt_passes(&a, &f, 0));

        // CRC-32 over 9 frames, inverted and least significant bit first.
        let c32 = check_named("CRC-32/ISO-HDLC").unwrap();
        let mut frames = Vec::new();
        for _ in 0..9 {
            let mut d: Vec<u8> = (0..12).map(|_| rng.below(256) as u8).collect();
            let c = c32.compute(&d);
            d.extend_from_slice(&(c as u32).to_be_bytes());
            frames.push(to_bits(&d, true, true));
        }
        let a = attempt(&frames, 0).unwrap();
        assert_eq!(
            (a.name.as_str(), a.passes, a.inverted, a.lsb_first),
            ("CRC-32/ISO-HDLC", 9, true, true)
        );

        // Random frames: no CRC.
        let frames: Vec<Vec<u8>> = (0..10)
            .map(|_| (0..128).map(|_| rng.below(2) as u8).collect())
            .collect();
        assert!(attempt(&frames, 0).is_none());
        // Random bits: no period, z under 7, no sync; constant bits: no period.
        let r: Vec<u8> = (0..8192).map(|_| rng.below(2) as u8).collect();
        assert!(period(&r).is_none());
        assert!(best_z(&r) < 7.0);
        assert!(period(&vec![1u8; 4096]).is_none());
        assert_eq!(bits_hex(&[1, 0, 1, 1, 0, 0, 0, 0, 1]), "B080");
    }
}
