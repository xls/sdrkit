//! Every named binary BCH code used by a standard: its field, length, information bits and the errors it corrects.
//!
//! A [`BchCode`] is the data a standard prints; [`Bch::from_code`](super::Bch::from_code) builds the decoder. The generator is
//! not stored: it is the product of the minimal polynomials of `α^1 .. α^(2t)` over the code's field, and the tests compare it
//! with the generator (or the factors `g_1 .. g_t`) each standard prints. Keys are snake case and stable, so settings can name
//! a code (`by_key("dvb_s2_short_1_2")`).
//!
//! ```
//! use sdrkit_algo_fec::bch::{codes, Bch};
//! let code = codes::by_key("pocsag_31_21").unwrap();
//! assert_eq!((code.n, code.k, code.t), (31, 21, 2));
//! let mut bch = Bch::from_code(code).unwrap();
//! // ITU-R M.584-2: the synchronisation code word 0x7CD215D8 (31 bits and an even parity bit).
//! let mut word: Vec<u8> = (1..32).rev().map(|i| (0x7CD2_15D8u32 >> i) as u8 & 1).collect();
//! assert!(bch.is_codeword(&word));
//! word[3] ^= 1;
//! assert_eq!(bch.decode(&mut word).unwrap().corrected, 1);
//! ```

/// A named narrow-sense binary BCH code: roots `α^1 .. α^(2t)` in GF(2^`m`) built on the primitive polynomial `poly`, sent `n`
/// bits long (shortened when `n < 2^m − 1`), `k` information bits first and the `n − k` parity bits after them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BchCode {
    /// The stable snake-case name (`"dvb_s2_normal_1_2"`).
    pub key: &'static str,
    /// The field is GF(2^m).
    pub m: u8,
    /// The field's primitive polynomial, bit `i` the coefficient of `x^i` (`0x89` is `x^7 + x^3 + 1`).
    pub poly: u32,
    /// Bits per code word as sent.
    pub n: usize,
    /// Information bits per code word.
    pub k: usize,
    /// Errors corrected.
    pub t: usize,
}

macro_rules! codes {
    ($($(#[$doc:meta])* $name:ident = ($key:literal, $m:literal, $poly:literal, $n:literal, $k:literal, $t:literal);)*) => {
        $(
            $(#[$doc])*
            pub const $name: BchCode = BchCode { key: $key, m: $m, poly: $poly, n: $n, k: $k, t: $t };
        )*
        /// Every named code, in the order of this module.
        pub const ALL: &[BchCode] = &[$($name),*];
    };
}

codes! {
    /// Cospas-Sarsat first-generation beacon, BCH-1 over the 61 bits of PDF-1: BCH(82, 61), t = 3, the (127, 106) code
    /// shortened. C/S T.001 Annex B: g(x) = x^21 + x^18 + x^17 + x^15 + x^14 + x^12 + x^11 + x^8 + x^7 + x^6 +
    /// x^5 + x + 1 (`0x26D9E3`).
    SARSAT_82_61 = ("sarsat_82_61", 7, 0x89, 82, 61, 3);
    /// Cospas-Sarsat first-generation beacon, BCH-2 over the 26 bits of PDF-2 (long message): BCH(38, 26), t = 2, the (63, 51)
    /// code shortened. C/S T.001 Annex B: g(x) = x^12 + x^10 + x^8 + x^5 + x^4 + x^3 + 1 (`0x1539`).
    SARSAT_38_26 = ("sarsat_38_26", 6, 0x43, 38, 26, 2);
    /// Cospas-Sarsat second-generation beacon: BCH(250, 202), t = 6, the (255, 207) code shortened. C/S T.018 Appendix B:
    /// g(x) = `0x1C7EB85DF3C97`.
    SARSAT_250_202 = ("sarsat_250_202", 8, 0x11D, 250, 202, 6);
    /// POCSAG code words: BCH(31, 21), t = 2, followed by an even parity bit. ITU-R M.584-2 Annex 1:
    /// g(x) = x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1 (`0x769`).
    POCSAG_31_21 = ("pocsag_31_21", 5, 0x25, 31, 21, 2);
    /// FLEX code words: the POCSAG BCH(31, 21) code with an even parity bit (the FLEX specification is not
    /// public; the code is the one receivers of it use).
    FLEX_31_21 = ("flex_31_21", 5, 0x25, 31, 21, 2);
    /// GOES DCS: the BCH(31, 21) code of POCSAG.
    GOES_DCS_31_21 = ("goes_dcs_31_21", 5, 0x25, 31, 21, 2);
    /// P25 network identifier (NID): BCH(63, 16), t = 11, followed by a parity bit to make 64. Generator 6331 1413 6723 5453
    /// (octal) (TIA-102.BAAA is not freely available).
    P25_NID_63_16 = ("p25_nid_63_16", 6, 0x43, 63, 16, 11);
    /// DVB-T transmission parameter signalling: BCH(67, 53), t = 2, the (127, 113) code shortened. ETSI EN 300 744 clause 4.6.2:
    /// h(x) = x^14 + x^9 + x^8 + x^6 + x^5 + x^4 + x^2 + x + 1 (`0x4377`).
    DVB_T_TPS_67_53 = ("dvb_t_tps_67_53", 7, 0x89, 67, 53, 2);
    /// MAC/packet family: BCH(71, 57), t = 2, the (127, 113) code of the DVB-T TPS (generator `0x4377`) shortened to 71 bits.
    MAC_71_57 = ("mac_71_57", 7, 0x89, 71, 57, 2);
    /// MAC/packet family: BCH(94, 80), t = 2, generator `0x4377`.
    MAC_94_80 = ("mac_94_80", 7, 0x89, 94, 80, 2);
    /// BeiDou D1/D2 navigation message: BCH(15, 11), t = 1 (a Hamming code). BDS-SIS-ICD-B1I (navigation message
    /// error correction):
    /// g(x) = x^4 + x + 1 (`0x13`).
    BEIDOU_15_11 = ("beidou_15_11", 4, 0x13, 15, 11, 1);
    /// Datawell HXV buoy data: BCH(63, 51), t = 2, generator `0x1539`.
    DATAWELL_63_51 = ("datawell_63_51", 6, 0x43, 63, 51, 2);
    /// EDACS control channel: BCH(40, 28), t = 2, the (63, 51) code shortened, generator `0x1539` (no public
    /// standard).
    EDACS_40_28 = ("edacs_40_28", 6, 0x43, 40, 28, 2);
    /// DVB-S2 normal frame, rate 1/4: BCH(16200, 16008), t = 12, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_1_4 = ("dvb_s2_normal_1_4", 16, 0x1002D, 16200, 16008, 12);
    /// DVB-S2 normal frame, rate 1/3: BCH(21600, 21408), t = 12, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_1_3 = ("dvb_s2_normal_1_3", 16, 0x1002D, 21600, 21408, 12);
    /// DVB-S2 normal frame, rate 2/5: BCH(25920, 25728), t = 12, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_2_5 = ("dvb_s2_normal_2_5", 16, 0x1002D, 25920, 25728, 12);
    /// DVB-S2 normal frame, rate 1/2: BCH(32400, 32208), t = 12, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_1_2 = ("dvb_s2_normal_1_2", 16, 0x1002D, 32400, 32208, 12);
    /// DVB-S2 normal frame, rate 3/5: BCH(38880, 38688), t = 12, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_3_5 = ("dvb_s2_normal_3_5", 16, 0x1002D, 38880, 38688, 12);
    /// DVB-S2 normal frame, rate 2/3: BCH(43200, 43040), t = 10, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_2_3 = ("dvb_s2_normal_2_3", 16, 0x1002D, 43200, 43040, 10);
    /// DVB-S2 normal frame, rate 3/4: BCH(48600, 48408), t = 12, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_3_4 = ("dvb_s2_normal_3_4", 16, 0x1002D, 48600, 48408, 12);
    /// DVB-S2 normal frame, rate 4/5: BCH(51840, 51648), t = 12, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_4_5 = ("dvb_s2_normal_4_5", 16, 0x1002D, 51840, 51648, 12);
    /// DVB-S2 normal frame, rate 5/6: BCH(54000, 53840), t = 10, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_5_6 = ("dvb_s2_normal_5_6", 16, 0x1002D, 54000, 53840, 10);
    /// DVB-S2 normal frame, rate 8/9: BCH(57600, 57472), t = 8, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_8_9 = ("dvb_s2_normal_8_9", 16, 0x1002D, 57600, 57472, 8);
    /// DVB-S2 normal frame, rate 9/10: BCH(58320, 58192), t = 8, over GF(2^16). ETSI EN 302 307-1 table 5a.
    DVB_S2_NORMAL_9_10 = ("dvb_s2_normal_9_10", 16, 0x1002D, 58320, 58192, 8);
    /// DVB-S2 short frame, rate 1/4: BCH(3240, 3072), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_1_4 = ("dvb_s2_short_1_4", 14, 0x402B, 3240, 3072, 12);
    /// DVB-S2 short frame, rate 1/3: BCH(5400, 5232), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_1_3 = ("dvb_s2_short_1_3", 14, 0x402B, 5400, 5232, 12);
    /// DVB-S2 short frame, rate 2/5: BCH(6480, 6312), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_2_5 = ("dvb_s2_short_2_5", 14, 0x402B, 6480, 6312, 12);
    /// DVB-S2 short frame, rate 1/2: BCH(7200, 7032), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_1_2 = ("dvb_s2_short_1_2", 14, 0x402B, 7200, 7032, 12);
    /// DVB-S2 short frame, rate 3/5: BCH(9720, 9552), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_3_5 = ("dvb_s2_short_3_5", 14, 0x402B, 9720, 9552, 12);
    /// DVB-S2 short frame, rate 2/3: BCH(10800, 10632), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_2_3 = ("dvb_s2_short_2_3", 14, 0x402B, 10800, 10632, 12);
    /// DVB-S2 short frame, rate 3/4: BCH(11880, 11712), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_3_4 = ("dvb_s2_short_3_4", 14, 0x402B, 11880, 11712, 12);
    /// DVB-S2 short frame, rate 4/5: BCH(12600, 12432), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_4_5 = ("dvb_s2_short_4_5", 14, 0x402B, 12600, 12432, 12);
    /// DVB-S2 short frame, rate 5/6: BCH(13320, 13152), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_5_6 = ("dvb_s2_short_5_6", 14, 0x402B, 13320, 13152, 12);
    /// DVB-S2 short frame, rate 8/9: BCH(14400, 14232), t = 12, over GF(2^14). ETSI EN 302 307-1 table 5b.
    DVB_S2_SHORT_8_9 = ("dvb_s2_short_8_9", 14, 0x402B, 14400, 14232, 12);
    /// DVB-S2X normal frame, rate 2/9: BCH(14400, 14208), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_2_9 = ("dvb_s2x_normal_2_9", 16, 0x1002D, 14400, 14208, 12);
    /// DVB-S2X normal frame, rate 13/45: BCH(18720, 18528), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_13_45 = ("dvb_s2x_normal_13_45", 16, 0x1002D, 18720, 18528, 12);
    /// DVB-S2X normal frame, rate 9/20: BCH(29160, 28968), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_9_20 = ("dvb_s2x_normal_9_20", 16, 0x1002D, 29160, 28968, 12);
    /// DVB-S2X normal frame, rate 90/180: BCH(32400, 32208), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_90_180 = ("dvb_s2x_normal_90_180", 16, 0x1002D, 32400, 32208, 12);
    /// DVB-S2X normal frame, rate 96/180: BCH(34560, 34368), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_96_180 = ("dvb_s2x_normal_96_180", 16, 0x1002D, 34560, 34368, 12);
    /// DVB-S2X normal frame, rate 11/20: BCH(35640, 35448), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_11_20 = ("dvb_s2x_normal_11_20", 16, 0x1002D, 35640, 35448, 12);
    /// DVB-S2X normal frame, rate 100/180: BCH(36000, 35808), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_100_180 = ("dvb_s2x_normal_100_180", 16, 0x1002D, 36000, 35808, 12);
    /// DVB-S2X normal frame, rate 104/180: BCH(37440, 37248), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_104_180 = ("dvb_s2x_normal_104_180", 16, 0x1002D, 37440, 37248, 12);
    /// DVB-S2X normal frame, rate 26/45: BCH(37440, 37248), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_26_45 = ("dvb_s2x_normal_26_45", 16, 0x1002D, 37440, 37248, 12);
    /// DVB-S2X normal frame, rate 18/30: BCH(38880, 38688), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_18_30 = ("dvb_s2x_normal_18_30", 16, 0x1002D, 38880, 38688, 12);
    /// DVB-S2X normal frame, rate 28/45: BCH(40320, 40128), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_28_45 = ("dvb_s2x_normal_28_45", 16, 0x1002D, 40320, 40128, 12);
    /// DVB-S2X normal frame, rate 23/36: BCH(41400, 41208), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_23_36 = ("dvb_s2x_normal_23_36", 16, 0x1002D, 41400, 41208, 12);
    /// DVB-S2X normal frame, rate 116/180: BCH(41760, 41568), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_116_180 = ("dvb_s2x_normal_116_180", 16, 0x1002D, 41760, 41568, 12);
    /// DVB-S2X normal frame, rate 20/30: BCH(43200, 43008), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_20_30 = ("dvb_s2x_normal_20_30", 16, 0x1002D, 43200, 43008, 12);
    /// DVB-S2X normal frame, rate 124/180: BCH(44640, 44448), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_124_180 = ("dvb_s2x_normal_124_180", 16, 0x1002D, 44640, 44448, 12);
    /// DVB-S2X normal frame, rate 25/36: BCH(45000, 44808), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_25_36 = ("dvb_s2x_normal_25_36", 16, 0x1002D, 45000, 44808, 12);
    /// DVB-S2X normal frame, rate 128/180: BCH(46080, 45888), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_128_180 = ("dvb_s2x_normal_128_180", 16, 0x1002D, 46080, 45888, 12);
    /// DVB-S2X normal frame, rate 13/18: BCH(46800, 46608), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_13_18 = ("dvb_s2x_normal_13_18", 16, 0x1002D, 46800, 46608, 12);
    /// DVB-S2X normal frame, rate 132/180: BCH(47520, 47328), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_132_180 = ("dvb_s2x_normal_132_180", 16, 0x1002D, 47520, 47328, 12);
    /// DVB-S2X normal frame, rate 22/30: BCH(47520, 47328), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_22_30 = ("dvb_s2x_normal_22_30", 16, 0x1002D, 47520, 47328, 12);
    /// DVB-S2X normal frame, rate 135/180: BCH(48600, 48408), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_135_180 = ("dvb_s2x_normal_135_180", 16, 0x1002D, 48600, 48408, 12);
    /// DVB-S2X normal frame, rate 140/180: BCH(50400, 50208), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_140_180 = ("dvb_s2x_normal_140_180", 16, 0x1002D, 50400, 50208, 12);
    /// DVB-S2X normal frame, rate 7/9: BCH(50400, 50208), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_7_9 = ("dvb_s2x_normal_7_9", 16, 0x1002D, 50400, 50208, 12);
    /// DVB-S2X normal frame, rate 154/180: BCH(55440, 55248), t = 12, over GF(2^16). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_NORMAL_154_180 = ("dvb_s2x_normal_154_180", 16, 0x1002D, 55440, 55248, 12);
    /// DVB-S2X medium frame, rate 1/5: BCH(5840, 5660), t = 12, over GF(2^15). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_MEDIUM_1_5 = ("dvb_s2x_medium_1_5", 15, 0x802D, 5840, 5660, 12);
    /// DVB-S2X medium frame, rate 11/45: BCH(7920, 7740), t = 12, over GF(2^15). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_MEDIUM_11_45 = ("dvb_s2x_medium_11_45", 15, 0x802D, 7920, 7740, 12);
    /// DVB-S2X medium frame, rate 1/3: BCH(10800, 10620), t = 12, over GF(2^15). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_MEDIUM_1_3 = ("dvb_s2x_medium_1_3", 15, 0x802D, 10800, 10620, 12);
    /// DVB-S2X short frame, rate 11/45: BCH(3960, 3792), t = 12, over GF(2^14). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_SHORT_11_45 = ("dvb_s2x_short_11_45", 14, 0x402B, 3960, 3792, 12);
    /// DVB-S2X short frame, rate 4/15: BCH(4320, 4152), t = 12, over GF(2^14). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_SHORT_4_15 = ("dvb_s2x_short_4_15", 14, 0x402B, 4320, 4152, 12);
    /// DVB-S2X short frame, rate 14/45: BCH(5040, 4872), t = 12, over GF(2^14). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_SHORT_14_45 = ("dvb_s2x_short_14_45", 14, 0x402B, 5040, 4872, 12);
    /// DVB-S2X short frame, rate 7/15: BCH(7560, 7392), t = 12, over GF(2^14). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_SHORT_7_15 = ("dvb_s2x_short_7_15", 14, 0x402B, 7560, 7392, 12);
    /// DVB-S2X short frame, rate 8/15: BCH(8640, 8472), t = 12, over GF(2^14). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_SHORT_8_15 = ("dvb_s2x_short_8_15", 14, 0x402B, 8640, 8472, 12);
    /// DVB-S2X short frame, rate 26/45: BCH(9360, 9192), t = 12, over GF(2^14). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_SHORT_26_45 = ("dvb_s2x_short_26_45", 14, 0x402B, 9360, 9192, 12);
    /// DVB-S2X short frame, rate 32/45: BCH(11520, 11352), t = 12, over GF(2^14). ETSI EN 302 307-2 tables 4 to 6.
    DVB_S2X_SHORT_32_45 = ("dvb_s2x_short_32_45", 14, 0x402B, 11520, 11352, 12);
}

/// The code named `key`, or `None`.
pub fn by_key(key: &str) -> Option<&'static BchCode> {
    ALL.iter().find(|c| c.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bch::Bch;
    use crate::cyclic::poly_mul;
    use crate::ldpc::tables::dvb_bch;

    /// The bits of `v`, `len` of them, the most significant first.
    fn bits(v: u128, len: usize) -> Vec<u8> {
        (0..len).rev().map(|i| (v >> i) as u8 & 1).collect()
    }

    /// The bits of a hexadecimal string, the first digit first.
    fn hex_bits(s: &str) -> Vec<u8> {
        s.chars()
            .flat_map(|c| {
                let d = c.to_digit(16).unwrap() as u8;
                (0..4).rev().map(move |i| (d >> i) & 1)
            })
            .collect()
    }

    fn bin(s: &str) -> Vec<u8> {
        s.chars()
            .filter(|c| *c != ' ')
            .map(|c| (c == '1') as u8)
            .collect()
    }

    /// Encodes `info` with `code`; returns the parity bits.
    fn parity(code: &BchCode, info: &[u8]) -> Vec<u8> {
        let bch = Bch::from_code(code).unwrap();
        let mut w = vec![0u8; code.n];
        w[..code.k].copy_from_slice(info);
        bch.encode(&mut w).unwrap();
        w[code.k..].to_vec()
    }

    #[test]
    fn every_code_builds_with_its_printed_k_and_keys_are_unique() {
        for (i, c) in ALL.iter().enumerate() {
            let b = Bch::from_code(c).unwrap_or_else(|e| panic!("{}: {e}", c.key));
            assert_eq!((b.n(), b.k(), b.t()), (c.n, c.k, c.t), "{}", c.key);
            assert!(ALL[..i].iter().all(|o| o.key != c.key), "{}", c.key);
            assert_eq!(by_key(c.key), Some(c));
        }
        assert_eq!(ALL.len(), 13 + 55);
        assert!(by_key("dvb_s2_normal_1_7").is_none());
        // A code whose printed k does not match its field and t is refused.
        assert!(Bch::from_code(&BchCode {
            k: 60,
            ..SARSAT_82_61
        })
        .is_err());
    }

    #[test]
    fn sarsat_bch1_and_bch2_from_t001_annex_b() {
        // C/S T.001 Annex B: the generators as printed.
        assert_eq!(
            Bch::from_code(&SARSAT_82_61).unwrap().generator(),
            &[0x26_D9E3]
        );
        assert_eq!(
            Bch::from_code(&SARSAT_38_26).unwrap().generator(),
            &[0x1539]
        );
        // Annex B sample short message: bits 25 to 112 are 56E6804002202009655250 (hex); bits 25-85 are PDF-1, 86-106 BCH-1.
        let msg = hex_bits("56E6804002202009655250");
        assert_eq!(parity(&SARSAT_82_61, &msg[..61]), msg[61..82]);
        assert_eq!(msg[61..82], bin("001011001010101001001"));
        // Annex B BCH-2: PDF-2 data 10 0101 0111 0000 0000 0001 0111 has the check bits 0001 0101 0001.
        let pdf2 = bin("10 0101 0111 0000 0000 0001 0111");
        assert_eq!(parity(&SARSAT_38_26, &pdf2), bin("0001 0101 0001"));
    }

    #[test]
    fn sarsat_second_generation_from_t018_appendix_b() {
        let bch = Bch::from_code(&SARSAT_250_202).unwrap();
        // Appendix B: g(x) = 1110001111110101110000101110111110011110010010111.
        let g = bin("1110001111110101110000101110111110011110010010111");
        let g = g.iter().fold(0u64, |a, &b| (a << 1) | b as u64);
        assert_eq!(g, 0x1_C7EB_85DF_3C97);
        assert_eq!(bch.generator(), &[g]);
        // The sample message of Appendix B (202 bits, then 2 padding bits) and its BCH as printed in Figure B-1.
        let msg = hex_bits("00E608F4C986196188A047C000000000000FFFC0100C1A00960");
        assert_eq!(msg.len(), 204);
        assert_eq!(
            parity(&SARSAT_250_202, &msg[..202]),
            bits(0x492A_4FC5_7A49, 48)
        );
    }

    #[test]
    fn pocsag_sync_and_idle_words_from_m584() {
        let mut bch = Bch::from_code(&POCSAG_31_21).unwrap();
        assert_eq!(bch.generator(), &[0x769]);
        // ITU-R M.584-2: the synchronisation code word, and the idle code word (printed there as 0x7AC9C197, which is not a code
        // word; 0x7A89C197 is, and is what transmitters send).
        for word in [0x7CD2_15D8u32, 0x7A89_C197] {
            assert_eq!(word.count_ones() % 2, 0);
            let w = bits(word as u128 >> 1, 31);
            assert!(bch.is_codeword(&w), "{word:#x}");
        }
        assert!(!bch.is_codeword(&bits(0x7AC9_C197u128 >> 1, 31)));
        // Two errors in the sync word are corrected.
        let sent = bits(0x7CD2_15D8u128 >> 1, 31);
        let mut w = sent.clone();
        w[0] ^= 1;
        w[30] ^= 1;
        assert_eq!(bch.decode(&mut w).unwrap().corrected, 2);
        assert_eq!(w, sent);
    }

    #[test]
    fn printed_generators_of_the_short_codes() {
        // EN 300 744 clause 4.6.2: the TPS code's h(x) = x^14 + x^9 + x^8 + x^6 + x^5 + x^4 + x^2 + x + 1.
        for c in [DVB_T_TPS_67_53, MAC_71_57, MAC_94_80] {
            assert_eq!(
                Bch::from_code(&c).unwrap().generator(),
                &[0x4377],
                "{}",
                c.key
            );
        }
        // BeiDou: g(x) = x^4 + x + 1.
        assert_eq!(Bch::from_code(&BEIDOU_15_11).unwrap().generator(), &[0x13]);
        // The (63, 51) family: the SARSAT BCH-2 generator.
        for c in [DATAWELL_63_51, EDACS_40_28] {
            assert_eq!(
                Bch::from_code(&c).unwrap().generator(),
                &[0x1539],
                "{}",
                c.key
            );
        }
        for c in [FLEX_31_21, GOES_DCS_31_21] {
            assert_eq!(
                Bch::from_code(&c).unwrap().generator(),
                &[0x769],
                "{}",
                c.key
            );
        }
        // P25 NID: 6331 1413 6723 5453 (octal).
        assert_eq!(
            Bch::from_code(&P25_NID_63_16).unwrap().generator(),
            &[0o6331_1413_6723_5453]
        );
    }

    #[test]
    fn dvb_generators_are_the_products_of_the_printed_factors() {
        // EN 302 307-1 tables 6a and 6b, EN 302 307-2 table 7: g(x) = g_1(x) g_2(x) .. g_t(x).
        for c in ALL.iter().filter(|c| c.key.starts_with("dvb_s2")) {
            let factors: &[u32] = match c.m {
                16 => &dvb_bch::S2_NORMAL_MINPOLYS,
                15 => &dvb_bch::S2X_MEDIUM_MINPOLYS,
                _ => &dvb_bch::S2_SHORT_MINPOLYS,
            };
            assert_eq!(c.poly, factors[0], "{}: the field is g_1", c.key);
            let mut g: Vec<u64> = vec![1];
            for &f in &factors[..c.t] {
                g = poly_mul(&g, &[f as u64]);
            }
            let bch = Bch::from_code(c).unwrap();
            assert_eq!(bch.generator(), &g[..], "{}", c.key);
            assert_eq!(c.n - c.k, c.m as usize * c.t, "{}", c.key);
        }
        // The tables of the LDPC module name the same codes.
        for (prefix, table) in [
            ("dvb_s2_", dvb_bch::S2_BCH_PARAMS),
            ("dvb_s2x_", dvb_bch::S2X_BCH_PARAMS),
        ] {
            for &(name, k, n, t) in table {
                let key = format!("{prefix}{}", name.replace([' ', '/'], "_").to_lowercase());
                let c = by_key(&key).unwrap_or_else(|| panic!("{key}"));
                assert_eq!(
                    (c.n, c.k, c.t),
                    (n as usize, k as usize, t as usize),
                    "{key}"
                );
            }
        }
    }
}
