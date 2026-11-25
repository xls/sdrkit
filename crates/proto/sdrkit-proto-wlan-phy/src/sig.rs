//! The signal fields of the PPDU formats after HT: VHT-SIG-A and VHT-SIG-B (802.11ac), HE-SIG-A (802.11ax) and U-SIG
//! (802.11be). Each type holds the bits as sent and reads the fields from them; the builders make the bits for test signals.
//!
//! A field of several bits is sent least significant bit first. The CRC of VHT-SIG-A is the 8-bit CRC of HT-SIG (polynomial 07,
//! register preset to ones, complemented, highest-order bit first). HE-SIG-A and U-SIG carry the four highest-order bits of
//! that CRC, the highest first.
//!
//! Standard: IEEE Std 802.11-2020 clause 21.3.8.3; IEEE Std 802.11ax-2021 clause 27.3.11.7; IEEE Std 802.11be-2024 clause
//! 36.3.12.7.

use sdrkit_util_bits::crc::catalog::CRC8_CDR;

/// Why a signal field cannot be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigError {
    /// The CRC is wrong.
    Crc,
    /// The tail bits are not zero.
    Tail,
    /// A reserved bit that is always 1 is 0.
    Reserved,
}

/// The bits `from..from + n` of `w` as a number, least significant first.
fn get(w: u32, from: u32, n: u32) -> u32 {
    (w >> from) & ((1 << n) - 1)
}

/// The two words of a signal field as `n` bits each in air order.
fn air<const N: usize>(a: u32, b: u32) -> [u8; N] {
    let half = N / 2;
    std::array::from_fn(|i| {
        if i < half {
            ((a >> i) & 1) as u8
        } else {
            ((b >> (i - half)) & 1) as u8
        }
    })
}

/// The two words of `N` bits in air order.
fn words<const N: usize>(b: &[u8; N]) -> (u32, u32) {
    let half = N / 2;
    let w = |r: std::ops::Range<usize>| r.rev().fold(0u32, |v, i| v << 1 | u32::from(b[i] & 1));
    (w(0..half), w(half..N))
}

/// The CRC-8 of the first `covered` bits of the two words (`half` bits each), as the register holds it.
fn crc8(a: u32, b: u32, half: usize, covered: usize) -> u8 {
    let mut bits = [0u8; 52];
    for (i, v) in bits.iter_mut().enumerate().take(covered) {
        *v = if i < half { ((a >> i) & 1) as u8 } else { ((b >> (i - half)) & 1) as u8 };
    }
    CRC8_CDR.bits(&bits[..covered])
}

/// The bandwidth of a VHT or HE bandwidth code 0 to 3, MHz.
fn mhz(code: u32) -> u16 {
    20 << code.min(3)
}

/// VHT-SIG-A: two words of 24 bits (VHT-SIG-A1, VHT-SIG-A2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VhtSigA {
    /// VHT-SIG-A1, bit 0 first on the air.
    pub a1: u32,
    /// VHT-SIG-A2 with its CRC and tail.
    pub a2: u32,
}

impl VhtSigA {
    /// The field of a single-user PPDU at `bw_mhz` (20, 40, 80, 160) with `nsts` space-time streams (1 to 8).
    pub fn su(bw_mhz: u16, nsts: u8, mcs: u8, short_gi: bool, ldpc: bool, n_sym: usize) -> VhtSigA {
        let bw = match bw_mhz {
            40 => 1,
            80 => 2,
            160 => 3,
            _ => 0,
        };
        let a1 = bw | 1 << 2 | u32::from(nsts.clamp(1, 8) - 1) << 10 | 1 << 23;
        let disamb = short_gi && n_sym % 10 == 9;
        let a2 = u32::from(short_gi)
            | u32::from(disamb) << 1
            | u32::from(ldpc) << 2
            | u32::from(mcs & 15) << 4
            | 1 << 9;
        let mut s = VhtSigA { a1, a2 };
        s.a2 |= u32::from(s.crc().reverse_bits()) << 10;
        s
    }

    /// The CRC of the first 34 bits.
    pub fn crc(&self) -> u8 {
        crc8(self.a1, self.a2, 24, 34)
    }

    /// The 48 bits in air order.
    pub fn bits(&self) -> [u8; 48] {
        air(self.a1, self.a2)
    }

    /// Reads the 48 bits. Errors for a wrong CRC, a tail that is not zero, or a reserved bit that is 0.
    pub fn from_bits(b: &[u8; 48]) -> Result<VhtSigA, SigError> {
        let (a1, a2) = words(b);
        let s = VhtSigA { a1, a2 };
        if get(a2, 18, 6) != 0 {
            return Err(SigError::Tail);
        }
        if get(a2, 10, 8) as u8 != s.crc().reverse_bits() {
            return Err(SigError::Crc);
        }
        if get(a1, 2, 1) == 0 || get(a1, 23, 1) == 0 || get(a2, 9, 1) == 0 {
            return Err(SigError::Reserved);
        }
        Ok(s)
    }

    /// The bandwidth, MHz (160 also means 80+80).
    pub fn bandwidth_mhz(&self) -> u16 {
        mhz(get(self.a1, 0, 2))
    }
    /// Space-time block coding.
    pub fn stbc(&self) -> bool {
        get(self.a1, 3, 1) == 1
    }
    /// The group ID: 0 and 63 are single-user.
    pub fn group_id(&self) -> u8 {
        get(self.a1, 4, 6) as u8
    }
    /// A single-user PPDU.
    pub fn is_su(&self) -> bool {
        matches!(self.group_id(), 0 | 63)
    }
    /// Space-time streams: of the one user (single-user), or of all four user positions together (multi-user).
    pub fn nsts(&self) -> u8 {
        if self.is_su() {
            get(self.a1, 10, 3) as u8 + 1
        } else {
            (0..4).map(|u| get(self.a1, 10 + 3 * u, 3) as u8).sum()
        }
    }
    /// The partial AID (single-user).
    pub fn partial_aid(&self) -> u16 {
        get(self.a1, 13, 9) as u16
    }
    /// The short guard interval.
    pub fn short_gi(&self) -> bool {
        get(self.a2, 0, 1) == 1
    }
    /// With the short guard interval: the symbol count from L-SIG is one too high.
    pub fn short_gi_disambiguation(&self) -> bool {
        get(self.a2, 1, 1) == 1
    }
    /// LDPC coding (of user 0 in a multi-user PPDU).
    pub fn ldpc(&self) -> bool {
        get(self.a2, 2, 1) == 1
    }
    /// The LDPC encoder added a symbol.
    pub fn ldpc_extra_symbol(&self) -> bool {
        get(self.a2, 3, 1) == 1
    }
    /// The MCS, 0 to 9 (single-user).
    pub fn mcs(&self) -> u8 {
        get(self.a2, 4, 4) as u8
    }
    /// A beamforming matrix is applied (single-user).
    pub fn beamformed(&self) -> bool {
        get(self.a2, 8, 1) == 1
    }
}

/// The 26 bits of VHT-SIG-B of a single-user 20 MHz PPDU for an A-MPDU of `length4` units of four octets (17 bits), three
/// reserved ones and the tail.
pub fn vht_sig_b_20(length4: u32) -> [u8; 26] {
    let w = (length4 & 0x1_FFFF) | 7 << 17;
    std::array::from_fn(|i| ((w >> i) & 1) as u8)
}

/// The A-MPDU length of a single-user 20 MHz VHT-SIG-B, octets.
pub fn vht_sig_b_20_length(b: &[u8; 26]) -> usize {
    4 * (0..17).rev().fold(0usize, |v, i| v << 1 | usize::from(b[i] & 1))
}

/// The CRC of the first 20 bits of VHT-SIG-B as the SERVICE field carries it in its bits 8 to 15 (bit 8 least significant).
pub fn vht_sig_b_crc(b: &[u8; 26]) -> u8 {
    CRC8_CDR.bits(&b[..20]).reverse_bits()
}

/// The format a HE-SIG-A field is read as. L-SIG and the modulation of the symbols give it, not the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeFormat {
    /// HE SU PPDU.
    Su,
    /// HE ER SU PPDU.
    ErSu,
    /// HE MU PPDU.
    Mu,
    /// HE TB PPDU.
    Tb,
}

/// HE-SIG-A: two words of 26 bits (HE-SIG-A1, HE-SIG-A2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeSigA {
    /// HE-SIG-A1.
    pub a1: u32,
    /// HE-SIG-A2 with its CRC and tail.
    pub a2: u32,
}

/// The fields of a HE-SIG-A of an HE SU or HE ER SU PPDU, for [`HeSigA::su`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeSu {
    /// The MCS, 0 to 11.
    pub mcs: u8,
    /// Dual carrier modulation.
    pub dcm: bool,
    /// The BSS colour, 0 to 63.
    pub bss_color: u8,
    /// The bandwidth code: 0 to 3 are 20, 40, 80, 160 MHz (HE SU); 0 and 1 are 242 and upper 106 tones (HE ER SU).
    pub bandwidth: u8,
    /// The guard interval and HE-LTF size code, 0 to 3.
    pub gi_ltf: u8,
    /// Space-time streams, 1 to 8.
    pub nsts: u8,
    /// LDPC coding.
    pub ldpc: bool,
    /// An uplink PPDU.
    pub uplink: bool,
}

impl HeSigA {
    /// The field of an HE SU or HE ER SU PPDU.
    pub fn su(f: &HeSu) -> HeSigA {
        let a1 = 1
            | u32::from(f.uplink) << 2
            | u32::from(f.mcs & 15) << 3
            | u32::from(f.dcm) << 7
            | u32::from(f.bss_color & 63) << 8
            | 1 << 14
            | u32::from(f.bandwidth & 3) << 19
            | u32::from(f.gi_ltf & 3) << 21
            | u32::from(f.nsts.clamp(1, 8) - 1) << 23;
        let a2 = 127 | u32::from(f.ldpc) << 7 | 1 << 14;
        HeSigA { a1, a2 }.sealed()
    }

    /// The field with its CRC set.
    pub fn sealed(mut self) -> HeSigA {
        self.a2 = (self.a2 & 0xFFFF) | u32::from(self.crc()) << 16;
        self
    }

    /// The four CRC bits as bits 16 to 19 of the second word carry them (bit 16 least significant).
    pub fn crc(&self) -> u8 {
        crc8(self.a1, self.a2, 26, 42).reverse_bits() & 15
    }

    /// The 52 bits in air order.
    pub fn bits(&self) -> [u8; 52] {
        air(self.a1, self.a2)
    }

    /// Reads the 52 bits. Errors for a wrong CRC or a tail that is not zero.
    pub fn from_bits(b: &[u8; 52]) -> Result<HeSigA, SigError> {
        let (a1, a2) = words(b);
        let s = HeSigA { a1, a2 };
        if get(a2, 20, 6) != 0 {
            return Err(SigError::Tail);
        }
        if get(a2, 16, 4) as u8 != s.crc() {
            return Err(SigError::Crc);
        }
        Ok(s)
    }

    /// HE SU and HE TB: the format bit (1: HE SU, 0: HE TB).
    pub fn format_su(&self) -> bool {
        get(self.a1, 0, 1) == 1
    }
    /// An uplink PPDU. HE TB PPDUs are always uplink.
    pub fn uplink(&self, f: HeFormat) -> bool {
        match f {
            HeFormat::Su | HeFormat::ErSu => get(self.a1, 2, 1) == 1,
            HeFormat::Mu => get(self.a1, 0, 1) == 1,
            HeFormat::Tb => true,
        }
    }
    /// The BSS colour.
    pub fn bss_color(&self, f: HeFormat) -> u8 {
        (match f {
            HeFormat::Su | HeFormat::ErSu => get(self.a1, 8, 6),
            HeFormat::Mu => get(self.a1, 5, 6),
            HeFormat::Tb => get(self.a1, 1, 6),
        }) as u8
    }
    /// The bandwidth, MHz. An HE ER SU PPDU is 20 MHz wide. Codes 4 to 7 of an HE MU PPDU are 80 and 160 MHz with punctured
    /// channels.
    pub fn bandwidth_mhz(&self, f: HeFormat) -> u16 {
        match f {
            HeFormat::Su => mhz(get(self.a1, 19, 2)),
            HeFormat::ErSu => 20,
            HeFormat::Mu => match get(self.a1, 15, 3) {
                c @ 0..=3 => mhz(c),
                4 | 5 => 80,
                _ => 160,
            },
            HeFormat::Tb => mhz(get(self.a1, 24, 2)),
        }
    }
    /// The MCS of the data field (HE SU, HE ER SU). HE MU and HE TB do not carry it here.
    pub fn mcs(&self, f: HeFormat) -> Option<u8> {
        matches!(f, HeFormat::Su | HeFormat::ErSu).then(|| get(self.a1, 3, 4) as u8)
    }
    /// Dual carrier modulation of the data field (HE SU, HE ER SU).
    pub fn dcm(&self, f: HeFormat) -> bool {
        matches!(f, HeFormat::Su | HeFormat::ErSu) && get(self.a1, 7, 1) == 1
    }
    /// The guard interval and HE-LTF size code (HE SU, HE ER SU, HE MU): 0 is 1x HE-LTF with 0.8 µs (HE SU) or 4x with 0.8 µs
    /// (HE MU), 1 is 2x with 0.8 µs, 2 is 2x with 1.6 µs, 3 is 4x with 3.2 µs.
    pub fn gi_ltf(&self, f: HeFormat) -> Option<u8> {
        match f {
            HeFormat::Su | HeFormat::ErSu => Some(get(self.a1, 21, 2) as u8),
            HeFormat::Mu => Some(get(self.a1, 23, 2) as u8),
            HeFormat::Tb => None,
        }
    }
    /// Space-time streams (HE SU, HE ER SU, without Doppler).
    pub fn nsts(&self, f: HeFormat) -> Option<u8> {
        matches!(f, HeFormat::Su | HeFormat::ErSu).then(|| get(self.a1, 23, 3) as u8 + 1)
    }
    /// LDPC coding of the data field (HE SU, HE ER SU).
    pub fn ldpc(&self, f: HeFormat) -> Option<bool> {
        matches!(f, HeFormat::Su | HeFormat::ErSu).then(|| get(self.a2, 7, 1) == 1)
    }
    /// The MCS of HE-SIG-B, 0 to 5 (HE MU).
    pub fn sig_b_mcs(&self) -> u8 {
        get(self.a1, 1, 3) as u8
    }
    /// HE-SIG-B symbols minus one, or MU-MIMO users minus one when HE-SIG-B is compressed (HE MU).
    pub fn sig_b_symbols(&self) -> u8 {
        get(self.a1, 18, 4) as u8
    }
    /// HE-SIG-B has no common field (HE MU).
    pub fn sig_b_compressed(&self) -> bool {
        get(self.a1, 22, 1) == 1
    }
}

/// U-SIG: two words of 26 bits (U-SIG-1, U-SIG-2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct USig {
    /// U-SIG-1.
    pub u1: u32,
    /// U-SIG-2 with its CRC and tail.
    pub u2: u32,
}

impl USig {
    /// The field of an EHT MU PPDU: bandwidth code 0 to 5, the BSS colour, the PPDU type and compression mode, the EHT-SIG MCS
    /// code and the EHT-SIG symbol count minus one. The disregard and validate bits are 1.
    pub fn eht(bandwidth: u8, uplink: bool, bss_color: u8, ppdu_type: u8, sig_mcs: u8, sig_symbols: u8) -> USig {
        let u1 = u32::from(bandwidth & 7) << 3
            | u32::from(uplink) << 6
            | u32::from(bss_color & 63) << 7
            | 127 << 13
            | 31 << 20
            | 1 << 25;
        let u2 = u32::from(ppdu_type & 3)
            | 1 << 2
            | 31 << 3
            | 1 << 8
            | u32::from(sig_mcs & 3) << 9
            | u32::from(sig_symbols & 31) << 11;
        let mut s = USig { u1, u2 };
        s.u2 |= u32::from(s.crc()) << 16;
        s
    }

    /// The four CRC bits as bits 16 to 19 of the second word carry them.
    pub fn crc(&self) -> u8 {
        crc8(self.u1, self.u2, 26, 42).reverse_bits() & 15
    }

    /// The 52 bits in air order.
    pub fn bits(&self) -> [u8; 52] {
        air(self.u1, self.u2)
    }

    /// Reads the 52 bits. Errors for a wrong CRC or a tail that is not zero.
    pub fn from_bits(b: &[u8; 52]) -> Result<USig, SigError> {
        let (u1, u2) = words(b);
        let s = USig { u1, u2 };
        if get(u2, 20, 6) != 0 {
            return Err(SigError::Tail);
        }
        if get(u2, 16, 4) as u8 != s.crc() {
            return Err(SigError::Crc);
        }
        Ok(s)
    }

    /// The PHY version identifier: 0 is EHT.
    pub fn version(&self) -> u8 {
        get(self.u1, 0, 3) as u8
    }
    /// The bandwidth, MHz; 0 for a code this standard does not define. Codes 4 and 5 are the two 320 MHz channelisations.
    pub fn bandwidth_mhz(&self) -> u16 {
        match get(self.u1, 3, 3) {
            c @ 0..=3 => mhz(c),
            4 | 5 => 320,
            _ => 0,
        }
    }
    /// An uplink PPDU.
    pub fn uplink(&self) -> bool {
        get(self.u1, 6, 1) == 1
    }
    /// The BSS colour.
    pub fn bss_color(&self) -> u8 {
        get(self.u1, 7, 6) as u8
    }
    /// The TXOP field.
    pub fn txop(&self) -> u8 {
        get(self.u1, 13, 7) as u8
    }
    /// The PPDU type and compression mode, 0 to 3. With `uplink`, 0 is a trigger-based PPDU; else 0 is OFDMA, 1 is a
    /// single-user or sounding transmission, 2 is non-OFDMA MU-MIMO.
    pub fn ppdu_type(&self) -> u8 {
        get(self.u2, 0, 2) as u8
    }
    /// A trigger-based PPDU (no EHT-SIG follows).
    pub fn is_tb(&self) -> bool {
        self.uplink() && self.ppdu_type() == 0
    }
    /// The punctured channel information (EHT MU).
    pub fn punctured(&self) -> u8 {
        get(self.u2, 3, 5) as u8
    }
    /// The EHT-SIG MCS code, 0 to 3 (EHT MU).
    pub fn sig_mcs(&self) -> u8 {
        get(self.u2, 9, 2) as u8
    }
    /// EHT-SIG symbols (EHT MU).
    pub fn sig_symbols(&self) -> u8 {
        get(self.u2, 11, 5) as u8 + 1
    }
}
