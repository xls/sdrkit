//! The BCH outer codes of DVB-S2, DVB-S2X and DVB-T2: the polynomials `g_1 .. g_12` of each frame size and the parameters
//! of each code.
//!
//! `g_i` is the minimal polynomial of `α^(2i−1)`, with `α` a root of `g_1`. The generator of a code that corrects `t`
//! errors is the product of the first `t` polynomials of its frame size. DVB-T2 uses the polynomials of DVB-S2.
//!
//! Standards: ETSI EN 302 307-1 V1.4.1 tables 5a, 5b, 6a and 6b; ETSI EN 302 307-2 V1.4.1 tables 4 to 7 and 19b to 19d;
//! ETSI EN 302 755 V1.4.1 tables 6(a), 6(b), 7(a), 7(b) and I.1.

/// g1(x) .. g12(x) for normal frames (EN 302 307-1 table 6a), over GF(2^16);
/// each as a u32 bit mask, bit i = coefficient of x^i.
pub const S2_NORMAL_MINPOLYS: [u32; 12] = [
    0x1002d, // g1(x) = 1 + x^2 + x^3 + x^5 + x^16
    0x10173, // g2(x) = 1 + x + x^4 + x^5 + x^6 + x^8 + x^16
    0x10fbd, // g3(x) = 1 + x^2 + x^3 + x^4 + x^5 + x^7 + x^8 + x^9 + x^10 + x^11 + x^16
    0x15a55, // g4(x) = 1 + x^2 + x^4 + x^6 + x^9 + x^11 + x^12 + x^14 + x^16
    0x11f2f, // g5(x) = 1 + x + x^2 + x^3 + x^5 + x^8 + x^9 + x^10 + x^11 + x^12 + x^16
    0x1f7b5, // g6(x) = 1 + x^2 + x^4 + x^5 + x^7 + x^8 + x^9 + x^10 + x^12 + x^13 + x^14 + x^15 + x^16
    0x1af65, // g7(x) = 1 + x^2 + x^5 + x^6 + x^8 + x^9 + x^10 + x^11 + x^13 + x^15 + x^16
    0x17367, // g8(x) = 1 + x + x^2 + x^5 + x^6 + x^8 + x^9 + x^12 + x^13 + x^14 + x^16
    0x10ea1, // g9(x) = 1 + x^5 + x^7 + x^9 + x^10 + x^11 + x^16
    0x175a7, // g10(x) = 1 + x + x^2 + x^5 + x^7 + x^8 + x^10 + x^12 + x^13 + x^14 + x^16
    0x13a2d, // g11(x) = 1 + x^2 + x^3 + x^5 + x^9 + x^11 + x^12 + x^13 + x^16
    0x11ae3, // g12(x) = 1 + x + x^5 + x^6 + x^7 + x^9 + x^11 + x^12 + x^16
];

/// g1(x) .. g12(x) for short frames (EN 302 307-1 table 6b), over GF(2^14).
pub const S2_SHORT_MINPOLYS: [u32; 12] = [
    0x0402b, // g1(x) = 1 + x + x^3 + x^5 + x^14
    0x04941, // g2(x) = 1 + x^6 + x^8 + x^11 + x^14
    0x04647, // g3(x) = 1 + x + x^2 + x^6 + x^9 + x^10 + x^14
    0x05591, // g4(x) = 1 + x^4 + x^7 + x^8 + x^10 + x^12 + x^14
    0x06b55, // g5(x) = 1 + x^2 + x^4 + x^6 + x^8 + x^9 + x^11 + x^13 + x^14
    0x06389, // g6(x) = 1 + x^3 + x^7 + x^8 + x^9 + x^13 + x^14
    0x06ce5, // g7(x) = 1 + x^2 + x^5 + x^6 + x^7 + x^10 + x^11 + x^13 + x^14
    0x04f21, // g8(x) = 1 + x^5 + x^8 + x^9 + x^10 + x^11 + x^14
    0x0460f, // g9(x) = 1 + x + x^2 + x^3 + x^9 + x^10 + x^14
    0x05a49, // g10(x) = 1 + x^3 + x^6 + x^9 + x^11 + x^12 + x^14
    0x05811, // g11(x) = 1 + x^4 + x^11 + x^12 + x^14
    0x065ef, // g12(x) = 1 + x + x^2 + x^3 + x^5 + x^6 + x^7 + x^8 + x^10 + x^13 + x^14
];

/// g1(x) .. g12(x) for DVB-S2X medium frames, n_ldpc = 32 400 (EN 302 307-2 table 7), over GF(2^15).
/// DVB-S2X normal and short frames use `S2_NORMAL_MINPOLYS` and `S2_SHORT_MINPOLYS`.
pub const S2X_MEDIUM_MINPOLYS: [u32; 12] = [
    0x0802d, // g1(x) = 1 + x^2 + x^3 + x^5 + x^15
    0x08c93, // g2(x) = 1 + x + x^4 + x^7 + x^10 + x^11 + x^15
    0x0b555, // g3(x) = 1 + x^2 + x^4 + x^6 + x^8 + x^10 + x^12 + x^13 + x^15
    0x08d6d, // g4(x) = 1 + x^2 + x^3 + x^5 + x^6 + x^8 + x^10 + x^11 + x^15
    0x094d7, // g5(x) = 1 + x + x^2 + x^4 + x^6 + x^7 + x^10 + x^12 + x^15
    0x0b0d1, // g6(x) = 1 + x^4 + x^6 + x^7 + x^12 + x^13 + x^15
    0x0d8b5, // g7(x) = 1 + x^2 + x^4 + x^5 + x^7 + x^11 + x^12 + x^14 + x^15
    0x0cb55, // g8(x) = 1 + x^2 + x^4 + x^6 + x^8 + x^9 + x^11 + x^14 + x^15
    0x0bab7, // g9(x) = 1 + x + x^2 + x^4 + x^5 + x^7 + x^9 + x^11 + x^12 + x^13 + x^15
    0x0bc9f, // g10(x) = 1 + x + x^2 + x^3 + x^4 + x^7 + x^10 + x^11 + x^12 + x^13 + x^15
    0x08a17, // g11(x) = 1 + x + x^2 + x^4 + x^9 + x^11 + x^15
    0x0ed15, // g12(x) = 1 + x^2 + x^4 + x^8 + x^10 + x^11 + x^13 + x^14 + x^15
];

/// DVB-S2 BCH parameters (EN 302 307-1 tables 5a and 5b): (frame and code identifier, K_bch, N_bch, t).
/// N_bch equals K_ldpc of the LDPC code.
pub const S2_BCH_PARAMS: &[(&str, u32 /*k_bch*/, u32 /*n_bch*/, u8 /*t*/)] = &[
    ("normal 1/4", 16008, 16200, 12),
    ("normal 1/3", 21408, 21600, 12),
    ("normal 2/5", 25728, 25920, 12),
    ("normal 1/2", 32208, 32400, 12),
    ("normal 3/5", 38688, 38880, 12),
    ("normal 2/3", 43040, 43200, 10),
    ("normal 3/4", 48408, 48600, 12),
    ("normal 4/5", 51648, 51840, 12),
    ("normal 5/6", 53840, 54000, 10),
    ("normal 8/9", 57472, 57600, 8),
    ("normal 9/10", 58192, 58320, 8),
    ("short 1/4", 3072, 3240, 12),
    ("short 1/3", 5232, 5400, 12),
    ("short 2/5", 6312, 6480, 12),
    ("short 1/2", 7032, 7200, 12),
    ("short 3/5", 9552, 9720, 12),
    ("short 2/3", 10632, 10800, 12),
    ("short 3/4", 11712, 11880, 12),
    ("short 4/5", 12432, 12600, 12),
    ("short 5/6", 13152, 13320, 12),
    ("short 8/9", 14232, 14400, 12),
];

/// DVB-S2X BCH parameters for the codes it adds (EN 302 307-2 tables 4, 5 and 6).
/// N_bch equals K_ldpc, except medium 1/5, where it is the K_ldpc of 5 840 after VL-SNR shortening (X_s = 640).
/// 104/180 and 26/45, 132/180 and 22/30, and 140/180 and 7/9 share their BCH parameters.
pub const S2X_BCH_PARAMS: &[(&str, u32 /*k_bch*/, u32 /*n_bch*/, u8 /*t*/)] = &[
    ("normal 2/9", 14208, 14400, 12),
    ("normal 13/45", 18528, 18720, 12),
    ("normal 9/20", 28968, 29160, 12),
    ("normal 90/180", 32208, 32400, 12),
    ("normal 96/180", 34368, 34560, 12),
    ("normal 11/20", 35448, 35640, 12),
    ("normal 100/180", 35808, 36000, 12),
    ("normal 104/180", 37248, 37440, 12),
    ("normal 26/45", 37248, 37440, 12),
    ("normal 18/30", 38688, 38880, 12),
    ("normal 28/45", 40128, 40320, 12),
    ("normal 23/36", 41208, 41400, 12),
    ("normal 116/180", 41568, 41760, 12),
    ("normal 20/30", 43008, 43200, 12),
    ("normal 124/180", 44448, 44640, 12),
    ("normal 25/36", 44808, 45000, 12),
    ("normal 128/180", 45888, 46080, 12),
    ("normal 13/18", 46608, 46800, 12),
    ("normal 132/180", 47328, 47520, 12),
    ("normal 22/30", 47328, 47520, 12),
    ("normal 135/180", 48408, 48600, 12),
    ("normal 140/180", 50208, 50400, 12),
    ("normal 7/9", 50208, 50400, 12),
    ("normal 154/180", 55248, 55440, 12),
    ("medium 1/5", 5660, 5840, 12),
    ("medium 11/45", 7740, 7920, 12),
    ("medium 1/3", 10620, 10800, 12),
    ("short 11/45", 3792, 3960, 12),
    ("short 4/15", 4152, 4320, 12),
    ("short 14/45", 4872, 5040, 12),
    ("short 7/15", 7392, 7560, 12),
    ("short 8/15", 8472, 8640, 12),
    ("short 26/45", 9192, 9360, 12),
    ("short 32/45", 11352, 11520, 12),
];

/// DVB-S2X BCH parameters of the VL-SNR frames (EN 302 307-2 tables 19b, 19c and 19d).
/// Short 1/5 is the DVB-S2 short 1/4 code and short 1/3 the DVB-S2 short 1/3 code; short 1/5 SF2 is that 1/5 code shortened.
pub const S2X_VLSNR_BCH_PARAMS: &[(&str, u32 /*k_bch*/, u32 /*n_bch*/, u8 /*t*/)] = &[
    ("normal 2/9", 14208, 14400, 12),
    ("medium 1/5", 5660, 5840, 12),
    ("medium 11/45", 7740, 7920, 12),
    ("medium 1/3", 10620, 10800, 12),
    ("short 11/45", 3792, 3960, 12),
    ("short 4/15", 4152, 4320, 12),
    ("short 1/3", 5232, 5400, 12),
    ("short 1/5", 3072, 3240, 12),
    ("short 1/5 SF2", 2512, 2680, 12),
];

/// DVB-T2 BCH parameters (EN 302 755 tables 6(a) and 6(b)); "lite" rows are the T2-Lite set of table I.1.
/// All values equal the DVB-S2 values for the same frame size and code identifier.
pub const T2_BCH_PARAMS: &[(&str, u32 /*k_bch*/, u32 /*n_bch*/, u8 /*t*/)] = &[
    ("normal 1/2", 32208, 32400, 12),
    ("normal 3/5", 38688, 38880, 12),
    ("normal 2/3", 43040, 43200, 10),
    ("normal 3/4", 48408, 48600, 12),
    ("normal 4/5", 51648, 51840, 12),
    ("normal 5/6", 53840, 54000, 10),
    ("short 1/4", 3072, 3240, 12),
    ("short 1/2", 7032, 7200, 12),
    ("short 3/5", 9552, 9720, 12),
    ("short 2/3", 10632, 10800, 12),
    ("short 3/4", 11712, 11880, 12),
    ("short 4/5", 12432, 12600, 12),
    ("short 5/6", 13152, 13320, 12),
    ("lite 1/4", 3072, 3240, 12),
    ("lite 1/3", 5232, 5400, 12),
    ("lite 2/5", 6312, 6480, 12),
    ("lite 1/2", 7032, 7200, 12),
    ("lite 3/5", 9552, 9720, 12),
    ("lite 2/3", 10632, 10800, 12),
    ("lite 3/4", 11712, 11880, 12),
];
