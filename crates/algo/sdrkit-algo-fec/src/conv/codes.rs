//! Every convolutional code and puncturing pattern the stacks use, named after the standard that defines it.
//!
//! Each code is written as its standard prints it (octal, newest bit first, outputs in sending order) and has a test that feeds a
//! single 1 through the encoder and compares every output with the generator's bits as printed (the impulse response).

use super::{ConvCode, Puncture};

// ---------------------------------------------------------------- K = 7, rate 1/2

/// CCSDS K = 7 rate 1/2 (CCSDS 131.0-B-5 clause 3.3): `G1 = 171`, `G2 = 133`, C1 (G1) sent first, **C2 inverted**. Used by the
/// CCSDS telemetry links (HRPT, AHRPT, xRIT, FY-3, CubeSats).
pub const CCSDS_K7: ConvCode = ConvCode::new(7, &[0o171, 0o133]).inverted(0b10);

/// The K = 7 rate 1/2 code `G1 = 171, G2 = 133` without inversion: the CCSDS punctured codes (CCSDS 131.0-B-5 clause 3.4.1:
/// "symbol inversion: none"), GPS L5 and L2C (IS-GPS-705J figure 3-7, IS-GPS-200N figure 3-14: G1 first), Meteor-M LRPT and
/// SBAS.
pub const K7_171_133: ConvCode = ConvCode::new(7, &[0o171, 0o133]);

/// The K = 7 rate 1/4 code `171, 133, 165, 117` without inversion, outputs in that order: the beacon and header codes of the
/// custom stacks.
pub const K7_171_133_165_117: ConvCode = ConvCode::new(7, &[0o171, 0o133, 0o165, 0o117]);

/// DVB-T / DVB-S / DVB-C inner code (EN 300 744 clause 4.3.3 figure 6; EN 300 421 clause 4.4.3): `X = 171`, `Y = 133`, X first.
pub const DVB_K7: ConvCode = ConvCode::new(7, &[0o171, 0o133]);

/// ISDB-T inner code (ARIB STD-B31 clause 3.6.2, figure 3-12): `G1 = 171` (X), `G2 = 133` (Y), X first: the DVB code. The
/// newest bit is the most significant tap; a register kept newest-at-bit-0 with these masks would encode the time-reversed code
/// (117, 155), which a real broadcast does not decode.
pub const ISDB_T_K7: ConvCode = DVB_K7;

/// ISDB-S inner code (ARIB STD-B20): the DVB code, also the coded part of TC8PSK.
pub const ISDB_S_K7: ConvCode = DVB_K7;

/// Galileo I/NAV and F/NAV (Galileo OS SIS ICD issue 2.1, clause 4.1.4.1, table 24 and figure 13): `G1 = 171`, `G2 = 133`, **G2 inverted**, G1 first.
pub const GALILEO_K7: ConvCode = ConvCode::new(7, &[0o171, 0o133]).inverted(0b10);

/// IEEE 802.11 OFDM (IEEE 802.11-2020 clause 17.3.5.6): `g0 = 133` (output A, sent first), `g1 = 171` (B). Also HaLow and the
/// Remote ID OFDM link (the standard is not freely available).
pub const WIFI_K7: ConvCode = ConvCode::new(7, &[0o133, 0o171]);

/// The K = 7 code `133, 171` (133 first) of Inmarsat-C, Aero (ARINC 741), HFDL (ARINC 635) and the MFSK/THOR short protocols.
/// See Inmarsat-C SDM and ARINC 741.
pub const INMARSAT_K7: ConvCode = ConvCode::new(7, &[0o133, 0o171]);

/// HFDL (ARINC 635-4): the same polynomials as [`INMARSAT_K7`].
pub const HFDL_K7: ConvCode = INMARSAT_K7;

/// The K = 7 code in the NASA-DSN order: `G2 = 133` sent first and **inverted**, then `G1 = 171`. The CCSDS code with its two
/// outputs swapped, as some CubeSat links send it.
pub const NASA_DSN_K7: ConvCode = ConvCode::new(7, &[0o133, 0o171]).inverted(0b01);

// ---------------------------------------------------------------- K = 7, lower rates

/// DAB mother code (EN 300 401 clause 11.1.1): rate 1/4, `g0 = 133, g1 = 171, g2 = 145, g3 = 133`, in that order. Also CDR.
pub const DAB_K7_R4: ConvCode = ConvCode::new(7, &[0o133, 0o171, 0o145, 0o133]);

/// DRM mother code (ES 201 980 clause 7.3.3): rate 1/6, `133, 171, 145, 133, 171, 145`.
pub const DRM_K7_R6: ConvCode = ConvCode::new(7, &[0o133, 0o171, 0o145, 0o133, 0o171, 0o145]);

/// LTE and NB-IoT tail-biting code (TS 36.212 clause 5.1.3.1): rate 1/3, `G0 = 133, G1 = 171, G2 = 165`.
pub const LTE_K7_R3: ConvCode = ConvCode::new(7, &[0o133, 0o171, 0o165]);

/// The mioty (TS-UNB) uplink code (ETSI TS 103 357 clause 6.4): rate 1/3, `155, 123, 137` in that order, the newest bit in the
/// most significant place as in [`WIFI_K7`], terminated by 6 zero bits.
pub const MIOTY_K7_R3: ConvCode = ConvCode::new(7, &[0o155, 0o123, 0o137]);

/// HD Radio FM rate 1/3 mother code (NRSC-5 SY_IDD_1011s rev. G, clause 9.3.4.1, table 9-2): `133, 171, 165`, tail-biting
/// (the last 6 bits of the transfer frame set the start state).
pub const HD_FM_K7_R3: ConvCode = LTE_K7_R3;

/// HD Radio FM rate 1/4 mother code (1011s clause 9.3.4.4, table 9-3): `133, 171, 165, 165`, tail-biting.
pub const HD_FM_K7_R4: ConvCode = ConvCode::new(7, &[0o133, 0o171, 0o165, 0o165]);

// ---------------------------------------------------------------- K = 9

/// HD Radio AM E1 mother code (NRSC-5 SY_IDD_1012s rev. G, clause 9.1.4.1, table 9-2): K = 9 rate 1/3 `561, 657, 711`,
/// tail-biting (the last 8 bits of the transfer frame set the start state).
pub const HD_AM_E1: ConvCode = ConvCode::new(9, &[0o561, 0o657, 0o711]);

/// HD Radio AM E2 and E3 mother code (1012s clauses 9.1.4.2 and 9.1.4.3, tables 9-3 and 9-4): K = 9 rate 1/3 `561, 753, 711`,
/// tail-biting.
pub const HD_AM_E2: ConvCode = ConvCode::new(9, &[0o561, 0o753, 0o711]);

/// UMTS BCH and the other rate 1/2 transport channels (3GPP TS 25.212 clause 4.2.3.1): K = 9, `G0 = 561`, `G1 = 753`, G0 first.
pub const UMTS_K9: ConvCode = ConvCode::new(9, &[0o561, 0o753]);

/// cdma2000 Sync Channel (3GPP2 C.S0002-C clause 3.1.3.1.4.1.4): K = 9 rate 1/2, `g0 = 753`, `g1 = 561`, the g0 symbol first.
pub const CDMA2000_K9: ConvCode = ConvCode::new(9, &[0o753, 0o561]);

// ---------------------------------------------------------------- K = 5 and K = 3

/// GSM (3GPP TS 45.003 clause 4.1.3): `G0 = 1 + D^3 + D^4` (23), `G1 = 1 + D + D^3 + D^4` (33), G0 first; terminated.
pub const GSM_K5: ConvCode = ConvCode::new(5, &[0o23, 0o33]);

/// NXDN, M17 and YSF: `G1 = 1 + D^3 + D^4` (23), `G2 = 1 + D + D^2 + D^4` (35), G1 first; terminated. Checked against the M17
/// specification (v2.0.10 appendix C); NXDN TS 1-A and the YSF air interface are not public ( for those).
pub const NXDN_K5: ConvCode = ConvCode::new(5, &[0o23, 0o35]);

/// M17 (M17 specification v2.0.10, appendix C): `G1 = 1 + D^3 + D^4`, `G2 = 1 + D + D^2 + D^4`, read alternately, 4 tail bits.
pub const M17_K5: ConvCode = NXDN_K5;

/// TETRA rate 1/4 mother code (EN 300 392-2 V3.8.1 clause 8.2.3.1.1, equations 8.3 to 8.6), outputs in the order G1 .. G4: `G1 = 1 + D + D^4` (31), `G2 = 1 + D^2 + D^3 + D^4` (27),
/// `G3 = 1 + D + D^2 + D^4` (35), `G4 = 1 + D + D^3 + D^4` (33).
pub const TETRA_K5_R4: ConvCode = ConvCode::new(5, &[0o31, 0o27, 0o35, 0o33]);

/// PSK31 QPSK (G3PLX's PSK31 description): K = 5 rate 1/2, `35, 23` (masks 0x17 and 0x19 of a newest-at-bit-0 register).
/// Streaming (no formal standard).
pub const QPSK31_K5: ConvCode = ConvCode::new(5, &[0o35, 0o23]);

/// D-STAR radio header (JARL D-STAR standard v6.0a, appendix Ap2): K = 3, `G1 = 1 + D + D^2` (7, the odd-numbered outputs, sent
/// first), `G2 = 1 + D^2` (5); terminated with two zero bits. Also Tetrapol.
pub const DSTAR_K3: ConvCode = ConvCode::new(3, &[0o7, 0o5]);

// ---------------------------------------------------------------- long codes

/// THOR 22/44/100 long code: K = 15 rate 1/2 `56711, 75063` (no formal standard).
pub const THOR_K15: ConvCode = ConvCode::new(15, &[0o56711, 0o75063]);

/// WSPR, JT4 and JT9 (A. Talbot G4JNT, "The WSPR Coding Process", 2009): K = 32 rate 1/2 with the register shifted left
/// (`reg = (reg << 1) | bit`) and `POLY1 = 0xF2D05351`, `POLY2 = 0xE4613C47`; newest first that is `0x8ACA0B4F, 0xE23C8627`
/// (octal 21262405517, 34217103047). 31 zero tail bits; decode with [`super::Fano`].
pub const WSPR_K32: ConvCode = ConvCode::from_lsb_masks(32, &[0xF2D0_5351, 0xE461_3C47]);

// ---------------------------------------------------------------- puncturing

/// No puncturing at rate 1/2.
pub const NONE_1_2: Puncture<'static> = Puncture::new(&[1, 1]);

/// DVB rate 2/3 (EN 300 744 table 2): `X: 10, Y: 11`, sent `X1 Y1 Y2`.
pub const DVB_2_3: Puncture<'static> = Puncture::new(&[1, 1, 0, 1]);
/// DVB rate 3/4: `X: 101, Y: 110`, sent `X1 Y1 Y2 X3`.
pub const DVB_3_4: Puncture<'static> = Puncture::new(&[1, 1, 0, 1, 1, 0]);
/// DVB rate 5/6: `X: 10101, Y: 11010`, sent `X1 Y1 Y2 X3 Y4 X5`.
pub const DVB_5_6: Puncture<'static> = Puncture::new(&[1, 1, 0, 1, 1, 0, 0, 1, 1, 0]);
/// DVB rate 7/8: `X: 1000101, Y: 1111010`, sent `X1 Y1 Y2 Y3 Y4 X5 Y6 X7`.
pub const DVB_7_8: Puncture<'static> = Puncture::new(&[1, 1, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0]);

/// IEEE 802.11 rate 2/3 (IEEE 802.11-2020 figure 17-9): `A1 B1 A2` (as [`WIFI_K7`]).
pub const WIFI_2_3: Puncture<'static> = Puncture::new(&[1, 1, 1, 0]);
/// IEEE 802.11 rate 3/4: `A1 B1 A2 B3`.
pub const WIFI_3_4: Puncture<'static> = Puncture::new(&[1, 1, 1, 0, 0, 1]);
/// IEEE 802.11 rate 5/6: `A1 B1 A2 B3 A4 B5`.
pub const WIFI_5_6: Puncture<'static> = Puncture::new(&[1, 1, 1, 0, 0, 1, 1, 0, 0, 1]);

/// CCSDS punctured rate 2/3 (CCSDS 131.0-B-5 table 3-1): the DVB pattern, on [`K7_171_133`].
pub const CCSDS_2_3: Puncture<'static> = DVB_2_3;
/// CCSDS punctured rate 3/4 (table 3-1).
pub const CCSDS_3_4: Puncture<'static> = DVB_3_4;
/// CCSDS punctured rate 5/6 (table 3-1).
pub const CCSDS_5_6: Puncture<'static> = DVB_5_6;
/// CCSDS punctured rate 7/8 (table 3-1).
pub const CCSDS_7_8: Puncture<'static> = DVB_7_8;

/// The 24 DAB puncturing vectors `PI_1 .. PI_24` (EN 300 401 clause 11.1.2, table 13): `DAB_PI[PI - 1]` keeps `8 + PI` of
/// each 32 mother-code bits (code rate `8 / (8 + PI)`).
pub const DAB_PI: [Puncture<'static>; 24] = [
    Puncture::new(&[
        1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 0,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1,
        0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1,
    ]),
];

/// DRM rate 1/6 (ES 201 980 clause 7.3.2, table 27): 1 input bits give 6 code bits.
pub const DRM_1_6: Puncture<'static> = Puncture::new(&[1, 1, 1, 1, 1, 1]);
/// DRM rate 1/4 (ES 201 980 clause 7.3.2, table 27): 1 input bits give 4 code bits.
pub const DRM_1_4: Puncture<'static> = Puncture::new(&[1, 1, 1, 1, 0, 0]);
/// DRM rate 3/10 (ES 201 980 clause 7.3.2, table 27): 3 input bits give 10 code bits.
pub const DRM_3_10: Puncture<'static> =
    Puncture::new(&[1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0]);
/// DRM rate 1/3 (ES 201 980 clause 7.3.2, table 27): 1 input bits give 3 code bits.
pub const DRM_1_3: Puncture<'static> = Puncture::new(&[1, 1, 1, 0, 0, 0]);
/// DRM rate 4/11 (ES 201 980 clause 7.3.2, table 27): 4 input bits give 11 code bits.
pub const DRM_4_11: Puncture<'static> = Puncture::new(&[
    1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0,
]);
/// DRM rate 2/5 (ES 201 980 clause 7.3.2, table 27): 2 input bits give 5 code bits.
pub const DRM_2_5: Puncture<'static> = Puncture::new(&[1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0]);
/// DRM rate 1/2 (ES 201 980 clause 7.3.2, table 27): 1 input bits give 2 code bits.
pub const DRM_1_2: Puncture<'static> = Puncture::new(&[1, 1, 0, 0, 0, 0]);
/// DRM rate 4/7 (ES 201 980 clause 7.3.2, table 27): 4 input bits give 7 code bits.
pub const DRM_4_7: Puncture<'static> = Puncture::new(&[
    1, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
]);
/// DRM rate 3/5 (ES 201 980 clause 7.3.2, table 27): 3 input bits give 5 code bits.
pub const DRM_3_5: Puncture<'static> =
    Puncture::new(&[1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0]);
/// DRM rate 2/3 (ES 201 980 clause 7.3.2, table 27): 2 input bits give 3 code bits.
pub const DRM_2_3: Puncture<'static> = Puncture::new(&[1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0]);
/// DRM rate 8/11 (ES 201 980 clause 7.3.2, table 27): 8 input bits give 11 code bits.
pub const DRM_8_11: Puncture<'static> = Puncture::new(&[
    1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0,
    0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
]);
/// DRM rate 3/4 (ES 201 980 clause 7.3.2, table 27): 3 input bits give 4 code bits.
pub const DRM_3_4: Puncture<'static> =
    Puncture::new(&[1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0]);
/// DRM rate 4/5 (ES 201 980 clause 7.3.2, table 27): 4 input bits give 5 code bits.
pub const DRM_4_5: Puncture<'static> = Puncture::new(&[
    1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
]);
/// DRM rate 7/8 (ES 201 980 clause 7.3.2, table 27): 7 input bits give 8 code bits.
pub const DRM_7_8: Puncture<'static> = Puncture::new(&[
    1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0,
    0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
]);
/// DRM rate 8/9 (ES 201 980 clause 7.3.2, table 27): 8 input bits give 9 code bits.
pub const DRM_8_9: Puncture<'static> = Puncture::new(&[
    1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0,
    0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
]);

/// DRM tail-bit puncturing (ES 201 980 clause 7.3.2, table 28): `DRM_TAIL[r_p]` for the 36 mother-code bits of the six tail
/// steps, `r_p = (2 N2 - 12) mod RY_p`.
pub const DRM_TAIL: [Puncture<'static>; 12] = [
    Puncture::new(&[
        1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0,
        1, 1, 0, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0,
        1, 1, 0, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0,
        1, 1, 0, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0,
        1, 1, 0, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 0, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 0, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 1, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 1, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 1, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 1, 0, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 1, 1, 0, 0,
    ]),
    Puncture::new(&[
        1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0,
        1, 1, 1, 1, 0, 0,
    ]),
];

/// The DAB tail puncturing vector `V_T` (EN 300 401 clause 11.1.2): `1100` six times over the last 24 mother-code bits.
pub const DAB_TAIL: Puncture<'static> = Puncture::new(&[
    1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0,
]);

/// M17 P1 (M17 specification v2.0.10 appendix E, equations E.1 and E.2): the 61-entry pattern for the link setup frame (488 to
/// 368 bits), keeping 46.
pub const M17_P1: Puncture<'static> = Puncture::new(&[
    1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1,
    1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1,
]);
/// M17 P2 (equation E.3): stream frames, 11 of 12.
pub const M17_P2: Puncture<'static> = Puncture::new(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0]);
/// M17 P3 (equation E.4): packet frames, 7 of 8.
pub const M17_P3: Puncture<'static> = Puncture::new(&[1, 1, 1, 1, 1, 1, 1, 0]);

/// TETRA rate 2/3 (EN 300 392-2 clause 8.2.3.1.3): `P = 1, 2, 5` of every 8 mother-code bits.
pub const TETRA_2_3: Puncture<'static> = Puncture::new(&[1, 1, 0, 0, 1, 0, 0, 0]);
/// TETRA rate 1/3 (clause 8.2.3.1.4): `P = 1, 2, 3, 5, 6, 7`.
pub const TETRA_1_3: Puncture<'static> = Puncture::new(&[1, 1, 1, 0, 1, 1, 1, 0]);
const TETRA_292_432_PATTERN: [u8; 176] = tetra_skip(&[1, 1, 0, 0, 1, 0, 0, 0], 172);
/// TETRA rate 292/432 (clause 8.2.3.1.5, TCH/4,8): rate 2/3 with `i = j + (j − 1) div 65`, so every 66th value of the rate 2/3
/// stream is also removed: a 176-bit period keeping 65.
pub const TETRA_292_432: Puncture<'static> = Puncture::new(&TETRA_292_432_PATTERN);
const TETRA_148_432_PATTERN: [u8; 48] = tetra_skip(&[1, 1, 1, 0, 1, 1, 1, 0], 46);
/// TETRA rate 148/432 (clause 8.2.3.1.6, TCH/2,4): rate 1/3 with `i = j + (j − 1) div 35`: a 48-bit period keeping 35.
pub const TETRA_148_432: Puncture<'static> = Puncture::new(&TETRA_148_432_PATTERN);

/// The base pattern repeated over `N` values with the value at `drop` removed.
const fn tetra_skip<const N: usize>(base: &[u8; 8], drop: usize) -> [u8; N] {
    let mut p = [0u8; N];
    let mut i = 0;
    while i < N {
        p[i] = base[i % 8];
        i += 1;
    }
    p[drop] = 0;
    p
}

/// HD Radio FM rate 2/5 from the rate 1/3 mother code (NRSC-5 1011s table 9-1): `g1 g2 g3, g1 g2`.
pub const HD_FM_2_5: Puncture<'static> = Puncture::new(&[1, 1, 1, 1, 1, 0]);
/// HD Radio FM rate 1/2 from the rate 1/3 mother code (table 9-1): `g1 g3`.
pub const HD_FM_1_2: Puncture<'static> = Puncture::new(&[1, 0, 1, 1, 0, 1]);
/// HD Radio FM rate 2/7 from the rate 1/4 mother code [`HD_FM_K7_R4`] (table 9-1): `g1 g2 g3 g4, g1 g2 g3`.
pub const HD_FM_2_7: Puncture<'static> = Puncture::new(&[1, 1, 1, 1, 1, 1, 1, 0]);
/// HD Radio AM E1 rate 5/12 (NRSC-5 1012s table 9-1): rows `11111 / 00011 / 11111`.
pub const HD_AM_E1_5_12: Puncture<'static> =
    Puncture::new(&[1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1]);
/// HD Radio AM E2 rate 2/3 (1012s table 9-1): rows `1111 / 0000 / 1010`.
pub const HD_AM_E2_2_3: Puncture<'static> = Puncture::new(&[1, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0, 0]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conv::Encoder;

    /// The response of each output to a single 1 followed by K - 1 zeros, as strings of 0 and 1.
    fn impulse(code: &ConvCode) -> Vec<String> {
        let k = code.k() as usize;
        let mut enc = Encoder::new(*code);
        let mut out = Vec::new();
        let mut bits = vec![0u8; k];
        bits[0] = 1;
        enc.encode(&bits, &mut out);
        (0..code.n())
            .map(|i| {
                out.iter()
                    .skip(i)
                    .step_by(code.n())
                    .map(|&b| char::from(b'0' + b))
                    .collect()
            })
            .collect()
    }

    fn check(code: &ConvCode, want: &[&str]) {
        assert_eq!(impulse(code), want, "{code:?}");
    }

    // The expected strings are the generators as printed in each standard, written out in binary by hand (octal digit by octal
    // digit, newest tap first), not computed from the constants.

    #[test]
    fn dvb_isdb_impulse_response() {
        // EN 300 744 figure 6: G1 = 171 = 1 111 001 (X), G2 = 133 = 1 011 011 (Y).
        check(&DVB_K7, &["1111001", "1011011"]);
        // ARIB STD-B31 figure 3-12: the same. The reversed-tap code would give X = 1001111, Y = 1101101.
        check(&ISDB_T_K7, &["1111001", "1011011"]);
        assert_ne!(impulse(&ISDB_T_K7), ["1001111", "1101101"]);
        check(&ISDB_S_K7, &["1111001", "1011011"]);
        check(&K7_171_133, &["1111001", "1011011"]);
    }

    #[test]
    fn ccsds_and_galileo_invert_the_second_output() {
        // CCSDS 131.0-B clause 3.3: C2 = NOT(G2 = 1 011 011).
        check(&CCSDS_K7, &["1111001", "0100100"]);
        check(&GALILEO_K7, &["1111001", "0100100"]);
    }

    #[test]
    fn wifi_and_133_first_codes() {
        // 802.11 figure 17-8: g0 = 133 (A) first, g1 = 171 (B).
        check(&WIFI_K7, &["1011011", "1111001"]);
        check(&INMARSAT_K7, &["1011011", "1111001"]);
        check(&HFDL_K7, &["1011011", "1111001"]);
        // The NASA-DSN order: NOT(G2 = 1 011 011) first, then G1 = 1 111 001.
        check(&NASA_DSN_K7, &["0100100", "1111001"]);
    }

    #[test]
    fn lower_rate_k7_codes() {
        // EN 300 401 clause 11.1.1: 133, 171, 145, 133.
        check(&DAB_K7_R4, &["1011011", "1111001", "1100101", "1011011"]);
        // ES 201 980 clause 7.3.3.
        check(
            &DRM_K7_R6,
            &[
                "1011011", "1111001", "1100101", "1011011", "1111001", "1100101",
            ],
        );
        // TS 36.212 clause 5.1.3.1: G0 = 133, G1 = 171, G2 = 165 = 1 110 101.
        check(&LTE_K7_R3, &["1011011", "1111001", "1110101"]);
        check(&HD_FM_K7_R3, &["1011011", "1111001", "1110101"]);
    }

    #[test]
    fn k9_codes() {
        // 561 = 101 110 001, 657 = 110 101 111, 711 = 111 001 001, 753 = 111 101 011.
        check(&HD_AM_E1, &["101110001", "110101111", "111001001"]);
        check(&HD_AM_E2, &["101110001", "111101011", "111001001"]);
        check(&UMTS_K9, &["101110001", "111101011"]);
        check(&CDMA2000_K9, &["111101011", "101110001"]);
    }

    #[test]
    fn k5_and_k3_codes() {
        // TS 45.003: G0 = 1 + D3 + D4 -> taps D0..D4 = 1 0 0 1 1; G1 = 1 + D + D3 + D4 -> 1 1 0 1 1.
        check(&GSM_K5, &["10011", "11011"]);
        // 1 + D3 + D4, 1 + D + D2 + D4.
        check(&NXDN_K5, &["10011", "11101"]);
        check(&M17_K5, &["10011", "11101"]);
        // EN 300 392-2 8.2.3.1.1: 1+D+D4, 1+D2+D3+D4, 1+D+D2+D4, 1+D+D3+D4.
        check(&TETRA_K5_R4, &["11001", "10111", "11101", "11011"]);
        check(&QPSK31_K5, &["11101", "10011"]);
        // 1 + D + D2, 1 + D2.
        check(&DSTAR_K3, &["111", "101"]);
    }

    #[test]
    fn long_codes() {
        // 56711 = 101 110 111 001 001, 75063 = 111 101 000 110 011.
        check(&THOR_K15, &["101110111001001", "111101000110011"]);
        // G4JNT: output j of the impulse is bit j (from bit 0) of POLY1 = 0xF2D05351 and POLY2 = 0xE4613C47.
        let p1: String = (0..32)
            .map(|j| char::from(b'0' + ((0xF2D0_5351u32 >> j) & 1) as u8))
            .collect();
        let p2: String = (0..32)
            .map(|j| char::from(b'0' + ((0xE461_3C47u32 >> j) & 1) as u8))
            .collect();
        assert_eq!(p1, "10001010110010100000101101001111");
        assert_eq!(p2, "11100010001111001000011000100111");
        check(&WSPR_K32, &[&p1, &p2]);
        assert_eq!(WSPR_K32.polys(), &[0o21262405517, 0o34217103047]);
    }

    #[test]
    fn named_punctures_have_their_rates() {
        for (p, k, n) in [
            (DVB_2_3, 3, 4),
            (DVB_3_4, 4, 6),
            (DVB_5_6, 6, 10),
            (DVB_7_8, 8, 14),
            (WIFI_2_3, 3, 4),
            (WIFI_3_4, 4, 6),
            (WIFI_5_6, 6, 10),
            (M17_P1, 46, 61),
            (M17_P2, 11, 12),
            (M17_P3, 7, 8),
            (TETRA_2_3, 3, 8),
            (TETRA_1_3, 6, 8),
            (HD_FM_2_5, 5, 6),
            (HD_FM_1_2, 4, 6),
            (HD_FM_2_7, 7, 8),
            (HD_AM_E1_5_12, 12, 15),
            (HD_AM_E2_2_3, 6, 12),
            (DAB_TAIL, 12, 24),
        ] {
            assert_eq!((p.kept(), p.period()), (k, n));
        }
        // DAB: PI keeps 8 + PI of 32 (table 13).
        for (i, p) in DAB_PI.iter().enumerate() {
            assert_eq!((p.kept(), p.period()), (9 + i, 32));
        }
        // DRM table 27: RY_p code bits per RX_p input bits.
        for (p, rx, ry) in [
            (DRM_1_6, 1, 6),
            (DRM_1_4, 1, 4),
            (DRM_3_10, 3, 10),
            (DRM_1_3, 1, 3),
            (DRM_4_11, 4, 11),
            (DRM_2_5, 2, 5),
            (DRM_1_2, 1, 2),
            (DRM_4_7, 4, 7),
            (DRM_3_5, 3, 5),
            (DRM_2_3, 2, 3),
            (DRM_8_11, 8, 11),
            (DRM_3_4, 3, 4),
            (DRM_4_5, 4, 5),
            (DRM_7_8, 7, 8),
            (DRM_8_9, 8, 9),
        ] {
            assert_eq!((p.kept(), p.period()), (ry, 6 * rx));
        }
        for (r, p) in DRM_TAIL.iter().enumerate() {
            assert_eq!((p.kept(), p.period()), (12 + r, 36));
        }
        // TETRA: 292 type-2 bits (4 of them tail) give 432 (TCH/4,8); 148 give 432 (TCH/2,4); M17 LSF 244 steps give 368.
        assert_eq!(TETRA_292_432.punctured_len(0, 292 * 4), 432);
        assert_eq!(TETRA_148_432.punctured_len(0, 148 * 4), 432);
        assert_eq!(M17_P1.punctured_len(0, 488), 368);
        assert_eq!(M17_P2.punctured_len(0, 296), 272);
        assert_eq!(M17_P3.punctured_len(0, 420), 368);
    }

    #[test]
    fn hd_fm_rate_quarter() {
        check(&HD_FM_K7_R4, &["1011011", "1111001", "1110101", "1110101"]);
    }
}
