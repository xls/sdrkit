//! Every Reed-Solomon code the stacks use, named after its standard.

use super::{Basis, RsCode, SymbolOrder};

const fn gf256(name: &'static str, poly: u32, n: u16, k: u16, fcr: u16, prim: u16) -> RsCode {
    RsCode {
        name,
        m: 8,
        poly,
        n,
        k,
        fcr,
        prim,
        order: SymbolOrder::HighFirst,
        basis: Basis::Conventional,
    }
}

const fn gf64(name: &'static str, n: u16, k: u16, fcr: u16) -> RsCode {
    RsCode {
        name,
        m: 6,
        poly: 0x43,
        n,
        k,
        fcr,
        prim: 1,
        order: SymbolOrder::HighFirst,
        basis: Basis::Conventional,
    }
}

/// DVB-S, DVB-T, DVB-C outer code (EN 300 421 clause 4.4.2, EN 300 744 clause 4.3.2): RS(204,188) shortened from RS(255,239),
/// `p(x) = x^8 + x^4 + x^3 + x^2 + 1`, `g(x) = (x + λ^0) .. (x + λ^15)`, `λ = 02h`. Also ISDB-T (ARIB STD-B31 clause 3.3), ISDB-S,
/// DMB and DAB packet mode (EN 300 401 clause 5.3.5.1).
pub const DVB_204_188: RsCode = gf256("dvb rs(204,188)", 0x11D, 204, 188, 0, 1);

/// ISDB-T multiplex position information (ARIB STD-B31): RS(204,196) shortened from RS(255,247), 8 roots from `λ^0`.
pub const ISDB_T_204_196: RsCode = gf256("isdb-t rs(204,196)", 0x11D, 204, 196, 0, 1);

/// ATSC 8-VSB (A/53 part 2 clause 5.2.2, figure 5.4): RS(207,187), t = 10, roots `α^0 .. α^19`.
pub const ATSC_207_187: RsCode = gf256("atsc rs(207,187)", 0x11D, 207, 187, 0, 1);

/// DAB+ superframe (ETSI TS 102 563 clause 6.1): RS(120,110) shortened from RS(255,245), `G(x) = Π_{i=0}^{9} (x + α^i)`.
pub const DAB_PLUS_120_110: RsCode = gf256("dab+ rs(120,110)", 0x11D, 120, 110, 0, 1);

/// ISDB-S TMCC: RS(64,48) (ARIB STD-B20 is not freely available).
pub const ISDB_S_TMCC_64_48: RsCode = gf256("isdb-s tmcc rs(64,48)", 0x11D, 64, 48, 0, 1);

/// DMR (ETSI TS 102 361-1 annex B.3.6): RS(12,9) shortened from RS(255,252), `G(x) = (x + α)(x + α^2)(x + α^3)`.
pub const DMR_12_9: RsCode = gf256("dmr rs(12,9)", 0x11D, 12, 9, 1, 1);

/// HD Radio header: RS(96,88), roots from `α^1`, data first. Deprecated: NRSC-5 sends the parity first, so use
/// [`HD_RADIO_HEADER_96_88`]. This entry stays for code that sets the order itself.
pub const HD_RADIO_96_88: RsCode = gf256("hd radio rs(96,88)", 0x11D, 96, 88, 1, 1);

/// HD Radio audio PDU header (NRSC-5 document 1017s clause 5.2.3.1): RS(96,88) over GF(256) with
/// `p(x) = x^8 + x^4 + x^3 + x^2 + 1` and the eight roots `α^1 .. α^8`. The parity comes first in the PDU, the last parity byte
/// first ([`SymbolOrder::LowFirst`]: PDU byte `i` is the coefficient of `x^i`).
pub const HD_RADIO_HEADER_96_88: RsCode = RsCode {
    order: SymbolOrder::LowFirst,
    ..gf256("hd radio header rs(96,88)", 0x11D, 96, 88, 1, 1)
};

/// Vaisala RS41 radiosonde: RS(255,231) (24 parity symbols), roots from `α^0`, shortened to 156 symbols in use, two code words
/// interleaved, the parity sent first (no published standard).
pub const RS41_255_231: RsCode = RsCode {
    order: SymbolOrder::LowFirst,
    ..gf256("rs41 rs(255,231)", 0x11D, 255, 231, 0, 1)
};

/// Vaisala RS92 radiosonde: RS(255,231) shortened to RS(234,210), parity first.
pub const RS92_234_210: RsCode = RsCode {
    order: SymbolOrder::LowFirst,
    ..gf256("rs92 rs(234,210)", 0x11D, 234, 210, 0, 1)
};

/// CCSDS telemetry RS(255,223), E = 16 (CCSDS 131.0-B-5 clause 4.3): `F(x) = x^8 + x^7 + x^2 + x + 1`,
/// `g(x) = Π_{j=112}^{143} (x − α^(11 j))`, dual basis on air.
pub const CCSDS_255_223: RsCode = RsCode {
    basis: Basis::Dual,
    ..gf256("ccsds rs(255,223)", 0x187, 255, 223, 112, 11)
};

/// CCSDS RS(255,239), E = 8 (clause 4.3): roots `α^(11 j)`, `j = 120 .. 135`, dual basis.
pub const CCSDS_255_239: RsCode = RsCode {
    basis: Basis::Dual,
    ..gf256("ccsds rs(255,239)", 0x187, 255, 239, 120, 11)
};

/// The CCSDS RS(255,223) code with symbols in the conventional basis (as used by SSDV and some CubeSat links).
pub const CCSDS_255_223_CONVENTIONAL: RsCode =
    gf256("ccsds rs(255,223) conventional", 0x187, 255, 223, 112, 11);

/// RS(255,239) over the CCSDS field with 16 check symbols, roots `α^(11 j)` for `j = 112 .. 127`, conventional basis: the
/// first 16 roots of the RS(255,223) code. Not [`CCSDS_255_239`], whose roots start at `j = 120`. Some CubeSat links (NGHam)
/// send it shortened.
pub const CCSDS_255_239_ROOT112_CONVENTIONAL: RsCode = gf256(
    "rs(255,239) roots from 112, conventional",
    0x187,
    255,
    239,
    112,
    11,
);

/// UAT basic uplink/ADS-B (RTCA DO-282): RS(30,18), field `0x187`, roots from `α^120` (DO-282 is not free).
pub const UAT_30_18: RsCode = gf256("uat rs(30,18)", 0x187, 30, 18, 120, 1);
/// UAT long ADS-B: RS(48,34).
pub const UAT_48_34: RsCode = gf256("uat rs(48,34)", 0x187, 48, 34, 120, 1);
/// UAT ground uplink (six interleaved): RS(92,72).
pub const UAT_92_72: RsCode = gf256("uat rs(92,72)", 0x187, 92, 72, 120, 1);

/// VDL Mode 3 data burst (ICAO Annex 10 Volume III 6.4.4): RS(72,62) shortened from RS(255,245), field `0x187`, roots
/// `α^120 .. α^129`.
pub const VDL3_72_62: RsCode = gf256("vdl3 rs(72,62)", 0x187, 72, 62, 120, 1);

/// VDL mode 2 (ICAO 9776): 6 parity symbols over the `0x187` field with roots `α^130 .. α^135` (the code of roots from
/// `α^120` read in reverse order), for a block of `n` symbols. Unsent parity is decoded as erasures.
pub const fn vdl2(n: u16) -> RsCode {
    gf256("vdl2 rs(n,n-6)", 0x187, n, n - 6, 130, 1)
}

/// ARDOP: RS over `0x11D` with `nroots` parity symbols and roots from `α^(255 − nroots)`.
pub const fn ardop(n: u16, nroots: u16) -> RsCode {
    gf256("ardop rs", 0x11D, n, n - nroots, 255 - nroots, 1)
}

/// Galileo I/NAV FEC2 outer code (Galileo OS SIS ICD issue 2.1, annex F): RS(118,58) shortened from RS(255,195),
/// `g(x) = Π_{i=1}^{60} (x − α^i)`. On air the data come first, lowest power first (`c_j` is the coefficient of `x^(60+j)`), then
/// the parity lowest power first: reverse each part before and after this codec, which uses [`SymbolOrder::HighFirst`].
pub const GALILEO_FEC2_118_58: RsCode = gf256("galileo fec2 rs(118,58)", 0x11D, 118, 58, 1, 1);

/// P25 phase 1 RS(24,12) over GF(64) (`x^6 + x + 1`), roots from `α^1` (TIA-102.BAAA is not free).
pub const P25_24_12: RsCode = gf64("p25 rs(24,12)", 24, 12, 1);
/// P25 phase 1 RS(24,16).
pub const P25_24_16: RsCode = gf64("p25 rs(24,16)", 24, 16, 1);
/// P25 phase 1 RS(36,20).
pub const P25_36_20: RsCode = gf64("p25 rs(36,20)", 36, 20, 1);
/// P25 phase 2 RS(45,26).
pub const P25_45_26: RsCode = gf64("p25 rs(45,26)", 45, 26, 1);
/// P25 phase 2 RS(52,30).
pub const P25_52_30: RsCode = gf64("p25 rs(52,30)", 52, 30, 1);

/// JT65 (J. Taylor, "The JT65 communications protocol", QEX 2005): RS(63,12) over GF(64) `x^6 + x + 1`, roots from `α^3`, decoded
/// with errors and erasures.
pub const JT65_63_12: RsCode = gf64("jt65 rs(63,12)", 63, 12, 3);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rs::Rs;

    /// A 31-byte NGHam data block (header 17, "HELLO", CRC BCEC, 23 zero bytes) gets the 16 check bytes of the smallsat
    /// specification, and 8 byte errors are corrected.
    #[test]
    fn rs_239_with_roots_from_112_codes_a_short_word() {
        let code = CCSDS_255_239_ROOT112_CONVENTIONAL.shortened(47);
        let mut rs = Rs::new(code).unwrap();
        let mut word = vec![0u8; 47];
        word[..8].copy_from_slice(&[0x17, 0x48, 0x45, 0x4C, 0x4C, 0x4F, 0xBC, 0xEC]);
        rs.encode_word(&mut word).unwrap();
        assert_eq!(
            word[31..],
            [
                0x88, 0x12, 0xC5, 0x96, 0xCD, 0x33, 0x86, 0x05, 0xD5, 0x05, 0xAE, 0xFE, 0x0A, 0xCC,
                0x9A, 0x76
            ]
        );
        let good = word.clone();
        for i in 0..8 {
            word[i * 5] ^= 0x5A;
        }
        assert_eq!(rs.decode(&mut word, &[]).unwrap().corrected, 8);
        assert_eq!(word, good);
    }
}
