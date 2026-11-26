//! The UMTS system information blocks read beyond the MIB (3GPP TS 25.331 clause 11.2, unaligned PER): SIB 1 (the location
//! and routing area codes from the NAS system information, TS 24.008 clause 10.5.1.12) and SIB 3 (the cell identity, whether
//! the cell is barred or reserved). Each is described once over [`PerIo`]; the same description reads and writes. The parts
//! after the fields read are not described.

use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::per::{PerIo, PerReader, PerWriter};
use sdrkit_util_bits::{BitOrder, Pad};

/// System information block type 1.
pub const SIB_1: u8 = 1;

/// What SIB 1 names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sib1 {
    /// cn-CommonGSM-MAP-NAS-SysInfo: its first two octets are the LAC.
    pub common_nas: Vec<u8>,
    /// cn-DomainSysInfoList: the domain (true for the PS domain) and its NAS system information (None for ANSI-41).
    pub domains: Vec<(bool, Option<Vec<u8>>, u8)>,
}

impl Sib1 {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        // ue-ConnTimersAndConstants, ue-IdleTimersAndConstants and the extensions: none described.
        let mut present = [false; 3];
        p.optionals(&mut present);
        p.octets(&mut self.common_nas, 1, 8);
        let mut n = self.domains.len().max(1);
        p.count(&mut n, 1, 4);
        if p.failed() {
            return;
        }
        self.domains.resize(n, (false, Some(vec![0]), 6));
        for (ps, nas, drx) in self.domains.iter_mut() {
            p.flag(ps);
            let mut ansi = nas.is_none();
            p.flag(&mut ansi);
            if ansi {
                // NAS-SystemInformationANSI-41: BIT STRING (SIZE (1..2048)), skipped.
                let mut len = 1usize;
                p.count(&mut len, 1, 2048);
                let mut x = 0u64;
                for _ in 0..len {
                    p.bits(&mut x, 1);
                }
                *nas = None;
            } else {
                let mut v = nas.clone().unwrap_or_else(|| vec![0]);
                p.octets(&mut v, 1, 8);
                *nas = Some(v);
            }
            let mut d = i64::from(*drx);
            p.int(&mut d, 6, 9);
            *drx = d as u8;
        }
    }
    /// The location area code.
    pub fn lac(&self) -> Option<u16> {
        (self.common_nas.len() >= 2).then(|| u16::from_be_bytes([self.common_nas[0], self.common_nas[1]]))
    }
    /// The routing area code: the first octet of the NAS system information of the PS domain.
    pub fn rac(&self) -> Option<u8> {
        self.domains.iter().find(|d| d.0).and_then(|d| d.1.as_ref()?.first().copied())
    }
}

/// What SIB 3 names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sib3 {
    /// sib4indicator.
    pub sib4: bool,
    /// cellIdentity (28 bits).
    pub cell_id: u32,
    /// cellBarred is barred.
    pub barred: bool,
    /// cellReservedForOperatorUse is reserved.
    pub reserved_operator: bool,
    /// cellReservationExtension is reserved.
    pub reservation_extension: bool,
}

/// A signed or unsigned number of `lo..=hi` that this description passes over: written as `lo`.
fn skip_int<P: PerIo>(p: &mut P, lo: i64, hi: i64) {
    let mut x = lo;
    p.int(&mut x, lo, hi);
}

/// An OPTIONAL number of `lo..=hi`, present when `on`.
fn skip_opt_int<P: PerIo>(p: &mut P, on: bool, lo: i64, hi: i64) {
    if on {
        skip_int(p, lo, hi);
    }
}

impl Sib3 {
    fn codec<P: PerIo>(&mut self, p: &mut P) {
        // v4b0NonCriticalExtensions: not described.
        let mut ext = false;
        p.flag(&mut ext);
        p.flag(&mut self.sib4);
        let mut id = u64::from(self.cell_id);
        p.bits(&mut id, 28);
        self.cell_id = id as u32;
        // CellSelectReselectInfoSIB-3-4: passed over. A writer writes the least form: no mapping, CPICH Ec/N0 without
        // q-Hyst-2-S, FDD without the searches and RAT list.
        let (mut mapping, mut hcs) = (false, false);
        p.flag(&mut mapping);
        p.flag(&mut hcs);
        if mapping {
            let mut n = 1usize;
            p.count(&mut n, 1, 16);
            for _ in 0..n {
                skip_int(p, 0, 3); // rat
                let mut m = 1usize;
                p.count(&mut m, 1, 3);
                for _ in 0..m {
                    let (mut p1, mut up) = (false, false);
                    p.flag(&mut p1);
                    p.flag(&mut up);
                    skip_int(p, 0, 3); // functionType
                    skip_opt_int(p, p1, 0, 127);
                    skip_int(p, 0, 127);
                    skip_opt_int(p, up, 1, 91);
                }
            }
        }
        let mut quality = 0usize;
        p.choice(&mut quality, 2, false);
        if quality == 0 {
            let mut h2 = false;
            p.flag(&mut h2);
            skip_opt_int(p, h2, 0, 20);
        }
        let mut mode = 0usize;
        p.choice(&mut mode, 2, false);
        let fdd = mode == 0;
        let mut o = [false; 4];
        p.optionals(&mut o);
        let (search_lo, search_hi) = if fdd { (-16, 10) } else { (-53, 45) };
        skip_opt_int(p, o[0], search_lo, search_hi);
        skip_opt_int(p, o[1], search_lo, search_hi);
        skip_opt_int(p, o[2], -53, 45);
        if o[3] {
            let mut n = 1usize;
            p.count(&mut n, 1, 15);
            for _ in 0..n {
                let mut h = false;
                p.flag(&mut h);
                skip_int(p, 0, 1); // rat-Identifier
                skip_int(p, search_lo, search_hi);
                skip_opt_int(p, h, -53, 45);
                skip_int(p, search_lo, search_hi);
            }
        }
        if fdd {
            skip_int(p, -24, 0); // q-QualMin
        }
        skip_int(p, -58, -13); // q-RxlevMin
        skip_int(p, 0, 20); // q-Hyst-l-S
        skip_int(p, 0, 31); // t-Reselection-S
        if hcs {
            let mut o = [false; 3];
            p.optionals(&mut o);
            skip_opt_int(p, o[0], 0, 7);
            skip_opt_int(p, o[1], 0, 99);
            if o[2] {
                let mut t = 0usize;
                p.choice(&mut t, 6, false);
                if t > 0 {
                    let mut n = false;
                    p.flag(&mut n);
                    skip_opt_int(p, n, 1, 16);
                    skip_int(p, 0, 7);
                }
            }
        }
        skip_int(p, -50, 33); // maxAllowedUL-TX-Power
        // CellAccessRestriction.
        let mut classes = false;
        p.flag(&mut classes);
        let mut barred = usize::from(!self.barred);
        p.choice(&mut barred, 2, false);
        self.barred = barred == 0;
        if self.barred {
            skip_int(p, 0, 1); // intraFreqCellReselectionInd
            skip_int(p, 0, 7); // t-Barred
        }
        // ReservedIndicator: reserved is the first value.
        for v in [&mut self.reserved_operator, &mut self.reservation_extension] {
            let mut x = !*v;
            p.flag(&mut x);
            *v = !x;
        }
        if classes {
            let mut x = 0u64;
            p.bits(&mut x, 16);
        }
    }
}

/// The bits (one per byte) of a block, as octets.
fn octets(bits: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    pack(bits, BitOrder::Msb, Pad::Zero, &mut b);
    b
}

/// Writes a block with `f`; the bits, one per byte.
fn write(f: impl FnOnce(&mut PerWriter)) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut w = PerWriter::new(&mut bytes);
    f(&mut w);
    let n = w.finish();
    let mut b = Vec::new();
    unpack(&bytes, BitOrder::Msb, &mut b);
    b.truncate(n);
    b
}

/// SIB 1 from the data bits of the block, none when it breaks a constraint.
pub fn parse_sib1(d: &[u8]) -> Option<Sib1> {
    let o = octets(d);
    let mut r = PerReader::new(&o);
    let mut s = Sib1::default();
    s.codec(&mut r);
    (!r.failed()).then_some(s)
}

/// SIB 3 from the data bits of the block: the cell identity always, the access restriction when the parts before it read.
pub fn parse_sib3(d: &[u8]) -> Option<(u32, Option<Sib3>)> {
    let o = octets(d);
    let cell = sdrkit_util_bits::pack::field(d, 2, 28)? as u32;
    let mut r = PerReader::new(&o);
    let mut s = Sib3::default();
    s.codec(&mut r);
    Some((cell, (!r.failed()).then_some(s)))
}

/// The data bits of a SIB 1.
pub fn sib1_data(s: &Sib1) -> Vec<u8> {
    write(|w| s.clone().codec(w))
}

/// The data bits of a SIB 3.
pub fn sib3_data(s: &Sib3) -> Vec<u8> {
    write(|w| s.clone().codec(w))
}

/// The SIB 1 of a cell with `lac` and `rac`: a CS domain and a PS domain of GSM-MAP.
pub fn sib1_of(lac: u16, rac: u8) -> Sib1 {
    let [a, b] = lac.to_be_bytes();
    Sib1 {
        common_nas: vec![a, b],
        domains: vec![(false, Some(vec![0x0A, 0x01]), 7), (true, Some(vec![rac, 0x01]), 7)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sib1_and_sib3_read_back() {
        let s1 = sib1_of(0x2A1F, 0x33);
        let d = sib1_data(&s1);
        let got = parse_sib1(&d).unwrap();
        assert_eq!((got.lac(), got.rac()), (Some(0x2A1F), Some(0x33)));
        let s3 = Sib3 {
            sib4: false,
            cell_id: 0x00AB_CDEF,
            barred: true,
            reserved_operator: false,
            reservation_extension: true,
        };
        let d = sib3_data(&s3);
        assert_eq!(parse_sib3(&d), Some((0x00AB_CDEF, Some(s3))));
        assert_eq!(parse_sib3(&d[..40]).map(|x| x.1), Some(None));
    }
}
