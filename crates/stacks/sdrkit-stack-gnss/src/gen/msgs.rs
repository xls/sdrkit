//! The message encoders of the test signal: the field layouts of each message, written with the shared parity, CRC,
//! convolutional code and BCH. Values are the integers that are broadcast. The orbits and clocks of the test signal are made from
//! the same integers.

use super::orbit::Elements;
use sdrkit_algo_fec::conv::codes::GALILEO_K7;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_util_bits::crc::catalog::CRC24_LTE_A;
use sdrkit_util_bits::crc::cyclic::{gps_lnav_parity, gps_lnav_word};
use sdrkit_util_bits::pack::{self, put_u64};

/// Writes `v` (low `len` bits) at 1-based bit `pos` of `b`.
fn set(b: &mut [u8], pos: usize, len: usize, v: i64) {
    // Every layout of this file fits its message, so the write cannot fail.
    let _ = put_u64(b, pos - 1, v as u64, len as u32);
}

/// Integers of a Kepler ephemeris and clock, with the scale of each per system.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeplerRaw {
    pub sqrt_a: i64,
    pub e: i64,
    pub i0: i64,
    pub omega0: i64,
    pub omega: i64,
    pub m0: i64,
    pub dn: i64,
    pub omega_dot: i64,
    pub idot: i64,
    pub cuc: i64,
    pub cus: i64,
    pub cic: i64,
    pub cis: i64,
    pub crc: i64,
    pub crs: i64,
    pub toe: i64,
    pub toc: i64,
    pub af0: i64,
    pub af1: i64,
    pub af2: i64,
    pub tgd: i64,
    pub iode: i64,
    pub iodc: i64,
    pub ura: i64,
}

/// The scale factors of a system's Kepler set.
#[derive(Clone, Copy, Debug)]
pub struct Scales {
    pub harm_angle: f64,
    pub harm_radius: f64,
    pub toe: f64,
    pub af0: f64,
    pub af1: f64,
    pub af2: f64,
    pub tgd: f64,
}

pub const GPS_S: Scales = Scales {
    harm_angle: 1.0 / 536_870_912.0, // 2^-29
    harm_radius: 1.0 / 32.0,
    toe: 16.0,
    af0: 1.0 / 2_147_483_648.0,
    af1: 1.0 / 8_796_093_022_208.0,
    af2: 1.0 / 36_028_797_018_963_968.0,
    tgd: 1.0 / 2_147_483_648.0,
};
pub const GAL_S: Scales = Scales {
    harm_angle: 1.0 / 536_870_912.0,
    harm_radius: 1.0 / 32.0,
    toe: 60.0,
    af0: 1.0 / 17_179_869_184.0,          // 2^-34
    af1: 1.0 / 70_368_744_177_664.0,      // 2^-46
    af2: 1.0 / 576_460_752_303_423_488.0, // 2^-59
    tgd: 1.0 / 4_294_967_296.0,           // 2^-32
};
pub const BDS_S: Scales = Scales {
    harm_angle: 1.0 / 2_147_483_648.0, // 2^-31
    harm_radius: 1.0 / 64.0,
    toe: 8.0,
    af0: 1.0 / 8_589_934_592.0,              // 2^-33
    af1: 1.0 / 1_125_899_906_842_624.0,      // 2^-50
    af2: 1.0 / 73_786_976_294_838_206_464.0, // 2^-66
    tgd: 1e-10,
};

const PI: f64 = crate::system::GPS_PI;
fn sc31() -> f64 {
    PI / 2_147_483_648.0
}
fn sc43() -> f64 {
    PI / 8_796_093_022_208.0
}

impl KeplerRaw {
    /// Quantises float values (angles in radians) at a system's scales.
    pub fn quantise(el: &Elements, clk: (f64, f64, f64, f64, f64), s: &Scales) -> KeplerRaw {
        let q = |x: f64, sc: f64| (x / sc).round() as i64;
        KeplerRaw {
            sqrt_a: q(el.sqrt_a, 1.0 / 524_288.0),
            e: q(el.e, 1.0 / 8_589_934_592.0),
            i0: q(el.i0, sc31()),
            omega0: q(el.omega0, sc31()),
            omega: q(el.omega, sc31()),
            m0: q(el.m0, sc31()),
            dn: q(el.delta_n, sc43()),
            omega_dot: q(el.omega_dot, sc43()),
            idot: q(el.idot, sc43()),
            cuc: q(el.cuc, s.harm_angle),
            cus: q(el.cus, s.harm_angle),
            cic: q(el.cic, s.harm_angle),
            cis: q(el.cis, s.harm_angle),
            crc: q(el.crc, s.harm_radius),
            crs: q(el.crs, s.harm_radius),
            toe: q(el.toe, s.toe),
            toc: q(clk.0, s.toe),
            af0: q(clk.1, s.af0),
            af1: q(clk.2, s.af1),
            af2: q(clk.3, s.af2),
            tgd: q(clk.4, s.tgd),
            iode: 0,
            iodc: 0,
            ura: 0,
        }
    }
    /// The elements the integers stand for.
    pub fn elements(&self, s: &Scales, geo: bool) -> Elements {
        Elements {
            sqrt_a: self.sqrt_a as f64 / 524_288.0,
            e: self.e as f64 / 8_589_934_592.0,
            i0: self.i0 as f64 * sc31(),
            omega0: self.omega0 as f64 * sc31(),
            omega: self.omega as f64 * sc31(),
            m0: self.m0 as f64 * sc31(),
            delta_n: self.dn as f64 * sc43(),
            omega_dot: self.omega_dot as f64 * sc43(),
            idot: self.idot as f64 * sc43(),
            cuc: self.cuc as f64 * s.harm_angle,
            cus: self.cus as f64 * s.harm_angle,
            cic: self.cic as f64 * s.harm_angle,
            cis: self.cis as f64 * s.harm_angle,
            crc: self.crc as f64 * s.harm_radius,
            crs: self.crs as f64 * s.harm_radius,
            toe: self.toe as f64 * s.toe,
            geo,
        }
    }
    /// (toc, af0, af1, af2, group delay) in seconds.
    pub fn clock(&self, s: &Scales) -> (f64, f64, f64, f64, f64) {
        (
            self.toc as f64 * s.toe,
            self.af0 as f64 * s.af0,
            self.af1 as f64 * s.af1,
            self.af2 as f64 * s.af2,
            self.tgd as f64 * s.tgd,
        )
    }
}

// ---------------------------------------------------------------- LNAV

/// A GPS almanac entry as integers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AlmRaw {
    pub prn: i64,
    pub e: i64,
    pub toa: i64,
    pub di: i64,
    pub omega_dot: i64,
    pub health: i64,
    pub sqrt_a: i64,
    pub omega0: i64,
    pub omega: i64,
    pub m0: i64,
    pub af0: i64,
    pub af1: i64,
}

/// Iono and UTC integers of page 18.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IonoUtcRaw {
    pub alpha: [i64; 4],
    pub beta: [i64; 4],
    pub a1: i64,
    pub a0: i64,
    pub tot: i64,
    pub wnt: i64,
    pub dtls: i64,
    pub wnlsf: i64,
    pub dn: i64,
    pub dtlsf: i64,
}

impl IonoUtcRaw {
    /// Typical values with `leap_s` leap seconds (GPS - UTC), and no leap second announced.
    pub fn typical(week: i64, leap_s: i64) -> IonoUtcRaw {
        IonoUtcRaw {
            alpha: [13, 1, -1, -1],
            beta: [44, 3, -2, -4],
            a1: 12,
            a0: -2,
            tot: 15,
            wnt: week % 256,
            dtls: leap_s,
            wnlsf: week % 256,
            dn: 7,
            dtlsf: leap_s,
        }
    }
}

/// What subframes 4 and 5 carry in a frame.
pub struct LnavPages<'a> {
    pub almanac: &'a [AlmRaw],
    pub iono_utc: IonoUtcRaw,
    pub wna: i64,
    pub toa: i64,
    pub qzss: bool,
}

/// Builds one LNAV subframe (300 transmitted bits) with parity. `sf_index`: the subframe's index in the week (its start is
/// sf_index × 6 s); `page`: the page of subframes 4 and 5 (1 to 25).
pub fn lnav_subframe(
    eph: &KeplerRaw,
    wn10: i64,
    health: i64,
    sf_index: i64,
    page: i64,
    pages: &LnavPages,
) -> Vec<u8> {
    let id = (sf_index % 5) + 1;
    let tow = (sf_index + 1) % 100_800;
    // Data bits by 1-based subframe position (parity positions filled later).
    let mut b = vec![0u8; 300];
    set(&mut b, 1, 8, 0x8B);
    set(&mut b, 31, 17, tow);
    set(&mut b, 50, 3, id);
    match id {
        1 => {
            set(&mut b, 61, 10, wn10);
            set(&mut b, 71, 2, 1);
            set(&mut b, 73, 4, eph.ura);
            set(&mut b, 77, 6, health);
            set(&mut b, 83, 2, eph.iodc >> 8);
            set(&mut b, 197, 8, eph.tgd);
            set(&mut b, 211, 8, eph.iodc & 0xFF);
            set(&mut b, 219, 16, eph.toc);
            set(&mut b, 241, 8, eph.af2);
            set(&mut b, 249, 16, eph.af1);
            set(&mut b, 271, 22, eph.af0);
        }
        2 => {
            set(&mut b, 61, 8, eph.iode);
            set(&mut b, 69, 16, eph.crs);
            set(&mut b, 91, 16, eph.dn);
            set(&mut b, 107, 8, eph.m0 >> 24);
            set(&mut b, 121, 24, eph.m0);
            set(&mut b, 151, 16, eph.cuc);
            set(&mut b, 167, 8, eph.e >> 24);
            set(&mut b, 181, 24, eph.e);
            set(&mut b, 211, 16, eph.cus);
            set(&mut b, 227, 8, eph.sqrt_a >> 24);
            set(&mut b, 241, 24, eph.sqrt_a);
            set(&mut b, 271, 16, eph.toe);
            set(&mut b, 287, 1, 0);
        }
        3 => {
            set(&mut b, 61, 16, eph.cic);
            set(&mut b, 77, 8, eph.omega0 >> 24);
            set(&mut b, 91, 24, eph.omega0);
            set(&mut b, 121, 16, eph.cis);
            set(&mut b, 137, 8, eph.i0 >> 24);
            set(&mut b, 151, 24, eph.i0);
            set(&mut b, 181, 16, eph.crc);
            set(&mut b, 197, 8, eph.omega >> 24);
            set(&mut b, 211, 24, eph.omega);
            set(&mut b, 241, 24, eph.omega_dot);
            set(&mut b, 271, 8, eph.iode);
            set(&mut b, 279, 14, eph.idot);
        }
        _ => page_45(&mut b, id, page, pages),
    }
    // Parity: words 2 and 10 choose bits 23 and 24 so the word ends in 00 (IS-GPS-200 clause 20.3.5.2).
    let mut out = vec![0u8; 300];
    let (mut p29, mut p30) = (0u8, 0u8);
    for w in 0..10 {
        let base = w * 30;
        let mut d = pack::to_u64(&b[base..base + 24]) as u32;
        if w == 1 || w == 9 {
            for t in 0..4u32 {
                d = (d & !3) | t;
                if gps_lnav_parity(d, p29, p30) & 3 == 0 {
                    break;
                }
            }
        }
        let word = gps_lnav_word(d, p29, p30);
        let _ = put_u64(&mut out, base, word as u64, 30);
        p29 = (word >> 1 & 1) as u8;
        p30 = (word & 1) as u8;
    }
    out
}

fn page_45(b: &mut [u8], id: i64, page: i64, pg: &LnavPages) {
    // Data ID 1 (2 bits); QZSS pages carry data ID 3 and are reserved.
    if pg.qzss {
        set(b, 61, 2, 3);
        set(b, 63, 6, 0);
        return;
    }
    set(b, 61, 2, 1);
    let alm = |prn: i64| pg.almanac.iter().find(|a| a.prn == prn).copied();
    let sv = if id == 5 {
        if page <= 24 {
            page
        } else {
            51
        }
    } else {
        match page {
            2 => 25,
            3 => 26,
            4 => 27,
            5 => 28,
            7 => 29,
            8 => 30,
            9 => 31,
            10 => 32,
            18 => 56,
            25 => 63,
            _ => 57,
        }
    };
    set(b, 63, 6, sv);
    if (1..=32).contains(&sv) {
        if let Some(a) = alm(sv) {
            set(b, 69, 16, a.e);
            set(b, 91, 8, a.toa);
            set(b, 99, 16, a.di);
            set(b, 121, 16, a.omega_dot);
            set(b, 137, 8, a.health);
            set(b, 151, 24, a.sqrt_a);
            set(b, 181, 24, a.omega0);
            set(b, 211, 24, a.omega);
            set(b, 241, 24, a.m0);
            set(b, 271, 8, a.af0 >> 3);
            set(b, 279, 11, a.af1);
            set(b, 290, 3, a.af0 & 7);
        } else {
            // A dummy page: no satellite in this slot.
            set(b, 63, 6, 0);
        }
    } else if sv == 56 {
        let u = &pg.iono_utc;
        set(b, 69, 8, u.alpha[0]);
        set(b, 77, 8, u.alpha[1]);
        set(b, 91, 8, u.alpha[2]);
        set(b, 99, 8, u.alpha[3]);
        set(b, 107, 8, u.beta[0]);
        set(b, 121, 8, u.beta[1]);
        set(b, 129, 8, u.beta[2]);
        set(b, 137, 8, u.beta[3]);
        set(b, 151, 24, u.a1);
        set(b, 181, 24, u.a0 >> 8);
        set(b, 211, 8, u.a0 & 0xFF);
        set(b, 219, 8, u.tot);
        set(b, 227, 8, u.wnt);
        set(b, 241, 8, u.dtls);
        set(b, 249, 8, u.wnlsf);
        set(b, 257, 8, u.dn);
        set(b, 271, 8, u.dtlsf);
    } else if sv == 51 {
        set(b, 69, 8, pg.toa);
        set(b, 77, 8, pg.wna);
    }
}

// ---------------------------------------------------------------- SBAS

/// Builds an SBAS message: preamble index, type, the 212 data bits; adds the CRC.
pub fn sbas_message(pre: usize, ty: u8, data: &[u8]) -> Vec<u8> {
    let mut m = vec![0u8; 250];
    set(&mut m, 1, 8, [0x53, 0x9A, 0xC6][pre % 3]);
    set(&mut m, 9, 6, ty as i64);
    m[14..226].copy_from_slice(&data[..212]);
    let c = CRC24_LTE_A.bits(&m[..226]);
    set(&mut m, 227, 24, c as i64);
    m
}

/// The structured SBAS content of the test signal: the 212 data bits of message `m` (of the week) of type `ty` from `prn`.
pub fn sbas_content(
    prn: u16,
    ty: u8,
    m: i64,
    period: i64,
    rnd: &mut impl FnMut() -> u64,
) -> Vec<u8> {
    let mut d = vec![0u8; 212];
    // 1-based positions in the message: data bit 15 is d[0].
    let mut put = |pos: usize, len: usize, v: i64| set(&mut d, pos - 14, len, v);
    match ty {
        1 => {
            for p in 1..=14 {
                put(14 + p, 1, 1);
            }
            put(14 + prn as usize, 1, 1);
            put(225, 2, 1);
        }
        2..=5 => {
            put(15, 2, 1);
            put(17, 2, 1);
            let blk = (ty - 2) as i64;
            for i in 0..13 {
                let s = blk * 13 + i + 1;
                let prc = ((s * 23 + period * 3).rem_euclid(120)) - 60;
                put(19 + 12 * i as usize, 12, prc);
                put(175 + 4 * i as usize, 4, s % 12);
            }
        }
        6 => {
            for k in 0..4 {
                put(15 + 2 * k, 2, 1);
            }
            for i in 0..51 {
                put(23 + 4 * i, 4, (i as i64) % 12);
            }
        }
        7 => {
            put(15, 4, 3);
            put(19, 2, 1);
            for i in 0..51 {
                put(23 + 4 * i, 4, (i as i64) % 16);
            }
        }
        9 => {
            let lon = (prn as f64 - 120.0) * 0.22;
            let r = 42_164_169.0;
            put(15, 8, 17);
            put(23, 13, m.rem_euclid(5400));
            put(36, 4, 2);
            put(40, 30, (r * lon.cos() / 0.08).round() as i64);
            put(70, 30, (r * lon.sin() / 0.08).round() as i64);
            put(100, 25, (48.0 / 0.4) as i64);
        }
        10 => {
            let vals: [(usize, i64); 16] = [
                (10, 80),
                (10, 5),
                (10, 6),
                (9, 7),
                (10, 8),
                (9, 9),
                (10, 11),
                (10, 12),
                (9, 13),
                (6, 10),
                (10, 14),
                (9, 15),
                (10, 50),
                (1, 0),
                (1, 1),
                (7, 3),
            ];
            let mut p = 15;
            for (l, v) in vals {
                put(p, l, v);
                p += l;
            }
        }
        12 => {
            put(15, 24, 7);
            put(39, 32, -1100);
            put(71, 8, 0);
            put(79, 8, 400 % 256);
            put(87, 8, 18);
            put(95, 8, 400 % 256);
            put(103, 8, 7);
            put(111, 8, 18);
            put(119, 3, 1);
            put(122, 20, m.rem_euclid(604_800));
            put(142, 10, 400);
            put(152, 1, 0);
        }
        17 => {
            for k in 0..3usize {
                let o = 15 + 67 * k;
                put(o, 2, 0);
                put(o + 2, 8, 120 + k as i64);
                put(o + 10, 8, 0);
                put(o + 18, 15, 16_000 - 5000 * k as i64);
                put(o + 33, 15, 0);
                put(o + 48, 9, 0);
            }
            put(216, 11, (m / 64).rem_euclid(2048));
        }
        18 => {
            put(15, 4, 2);
            put(19, 4, 0);
            put(23, 2, 0);
            for i in 0..75 {
                put(25 + i, 1, (i % 5 != 4) as i64);
            }
        }
        24 => {
            for i in 0..6i64 {
                put(15 + 12 * i as usize, 12, (i * 5) - 10);
                put(87 + 4 * i as usize, 4, 1);
            }
            put(111, 2, 1);
            put(113, 2, 0);
            put(115, 2, 1);
            lt_half(&mut d, 121 - 14, 3, 33, 40);
        }
        25 => {
            lt_half(&mut d, 15 - 14, 1, 33, 12);
            lt_half(&mut d, 121 - 14, 4, 33, 20);
        }
        26 => {
            let block = period.rem_euclid(4);
            put(15, 4, 0);
            put(19, 4, block);
            for i in 0..15 {
                put(23 + 13 * i as usize, 9, 24 + block * 3 + i);
                put(32 + 13 * i as usize, 4, i % 9);
            }
            put(218, 2, 0);
        }
        _ => {
            for v in d.iter_mut() {
                *v = (rnd() & 1) as u8;
            }
        }
    }
    d
}

/// A long-term half message with velocity code 0: slots `s` and `s + 1`, IODE, δx 1.5 m, δy −0.875 m, δz 0.25 m.
fn lt_half(d: &mut [u8], at0: usize, s: i64, iode: i64, daf0: i64) {
    let mut put = |pos: usize, len: usize, v: i64| set(d, at0 + pos, len, v);
    put(0, 1, 0);
    for k in 0..2i64 {
        let o = 1 + 51 * k as usize;
        put(o, 6, s + k);
        put(o + 6, 8, iode);
        put(o + 14, 9, 12);
        put(o + 23, 9, -7);
        put(o + 32, 9, 2);
        put(o + 41, 10, daf0);
    }
    put(103, 2, 1);
}

/// The rotation of message types of the generator.
pub const SBAS_ROTATION: [u8; 16] = [9, 2, 3, 4, 5, 18, 26, 25, 7, 10, 12, 17, 24, 27, 28, 1];

// ---------------------------------------------------------------- Galileo I/NAV

/// Galileo values the words carry beyond the Kepler set.
#[derive(Clone, Copy, Debug, Default)]
pub struct GalExtra {
    pub iodnav: i64,
    pub svid: i64,
    pub bgd_a: i64,
    pub bgd_b: i64,
    /// A0G, A1G, t0G, WN0G; `None` sends all ones.
    pub ggto: Option<(i64, i64, i64, i64)>,
}

/// The 128-bit word of type `ty`.
pub fn inav_word(ty: u8, r: &KeplerRaw, x: &GalExtra, wn: i64, tow: i64) -> Vec<u8> {
    let mut w = vec![0u8; 128];
    let mut put = |pos0: usize, len: usize, v: i64| set(&mut w, pos0 + 1, len, v);
    put(0, 6, ty as i64);
    match ty {
        1 => {
            put(6, 10, x.iodnav);
            put(16, 14, r.toe);
            put(30, 32, r.m0);
            put(62, 32, r.e);
            put(94, 32, r.sqrt_a);
        }
        2 => {
            put(6, 10, x.iodnav);
            put(16, 32, r.omega0);
            put(48, 32, r.i0);
            put(80, 32, r.omega);
            put(112, 14, r.idot);
        }
        3 => {
            put(6, 10, x.iodnav);
            put(16, 24, r.omega_dot);
            put(40, 16, r.dn);
            put(56, 16, r.cuc);
            put(72, 16, r.cus);
            put(88, 16, r.crc);
            put(104, 16, r.crs);
            put(120, 8, 107);
        }
        4 => {
            put(6, 10, x.iodnav);
            put(16, 6, x.svid);
            put(22, 16, r.cic);
            put(38, 16, r.cis);
            put(54, 14, r.toc);
            put(68, 31, r.af0);
            put(99, 21, r.af1);
            put(120, 6, r.af2);
        }
        5 => {
            put(6, 11, 120);
            put(17, 11, 20);
            put(28, 14, 0);
            put(47, 10, x.bgd_a);
            put(57, 10, x.bgd_b);
            put(73, 12, wn);
            put(85, 20, tow);
        }
        6 => {
            put(6, 32, -2);
            put(38, 24, 12);
            put(62, 8, 18);
            put(70, 8, 16);
            put(78, 8, wn % 256);
            put(86, 8, wn % 256);
            put(94, 3, 7);
            put(97, 8, 18);
            put(105, 20, tow);
        }
        10 => match x.ggto {
            Some((a0, a1, t0, wn0)) => {
                put(86, 16, a0);
                put(102, 12, a1);
                put(114, 8, t0);
                put(122, 6, wn0);
            }
            None => {
                put(86, 16, -1);
                put(102, 12, -1);
                put(114, 8, -1);
                put(122, 6, -1);
            }
        },
        0 => {
            put(6, 2, 2);
            put(96, 12, wn);
            put(108, 20, tow);
        }
        _ => {}
    }
    w
}

/// The word type sent in page slot `k` (0 to 14) of the 30 s subframe.
pub fn inav_schedule(k: i64) -> u8 {
    [2, 4, 6, 10, 0, 0, 0, 0, 0, 0, 1, 3, 5, 0, 0][k.rem_euclid(15) as usize]
}

/// The 250 symbols of a page part (sync pattern, then 240 interleaved coded symbols) for `part` (120 bits).
pub fn inav_part_symbols(part: &[u8]) -> Vec<u8> {
    let mut coded = Vec::with_capacity(240);
    Encoder::new(GALILEO_K7).encode(part, &mut coded);
    let mut out = vec![0u8; 250];
    out[..10].copy_from_slice(&crate::inav::SYNC);
    // 120 bits give 240 symbols, so the permutation cannot fail.
    let _ = crate::inav::interleaver().apply(&coded, &mut out[10..]);
    out
}

/// The even and odd parts (120 bits each) of a page carrying `word`.
pub fn inav_page(word: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut even = vec![0u8; 120];
    even[2..114].copy_from_slice(&word[..112]);
    let mut odd = vec![0u8; 120];
    odd[0] = 1;
    odd[2..18].copy_from_slice(&word[112..128]);
    let mut c_in = even[..114].to_vec();
    c_in.extend_from_slice(&odd[..82]);
    let c = CRC24_LTE_A.bits(&c_in);
    set(&mut odd, 83, 24, c as i64);
    (even, odd)
}

// ---------------------------------------------------------------- BeiDou

/// The 186-bit field streams of D1 subframes 1, 2 and 3.
pub fn bds_fields(r: &KeplerRaw, week: i64) -> [Vec<u8>; 3] {
    let mut f1 = vec![0u8; 186];
    let mut f2 = vec![0u8; 186];
    let mut f3 = vec![0u8; 186];
    let p = |b: &mut Vec<u8>, s: usize, l: usize, v: i64| set(b, s + 1, l, v);
    p(&mut f1, 0, 1, 0);
    p(&mut f1, 1, 5, r.iodc);
    p(&mut f1, 6, 4, r.ura);
    p(&mut f1, 10, 13, week);
    p(&mut f1, 23, 17, r.toc);
    p(&mut f1, 40, 10, r.tgd);
    p(&mut f1, 50, 10, r.tgd);
    p(&mut f1, 124, 11, r.af2);
    p(&mut f1, 135, 24, r.af0);
    p(&mut f1, 159, 22, r.af1);
    p(&mut f1, 181, 5, r.iode);
    p(&mut f2, 0, 16, r.dn);
    p(&mut f2, 16, 18, r.cuc);
    p(&mut f2, 34, 32, r.m0);
    p(&mut f2, 66, 32, r.e);
    p(&mut f2, 98, 18, r.cus);
    p(&mut f2, 116, 18, r.crc);
    p(&mut f2, 134, 18, r.crs);
    p(&mut f2, 152, 32, r.sqrt_a);
    p(&mut f2, 184, 2, r.toe >> 15);
    p(&mut f3, 0, 15, r.toe & 0x7FFF);
    p(&mut f3, 15, 32, r.i0);
    p(&mut f3, 47, 18, r.cic);
    p(&mut f3, 65, 24, r.omega_dot);
    p(&mut f3, 89, 18, r.cis);
    p(&mut f3, 107, 14, r.idot);
    p(&mut f3, 121, 32, r.omega0);
    p(&mut f3, 153, 32, r.omega);
    [f1, f2, f3]
}

/// The 224 information bits of a subframe: preamble, subframe ID, SOW, and `body` (the bits after the SOW).
pub fn bds_info(id: i64, sow: i64, body: &[u8]) -> Vec<u8> {
    let mut i = vec![0u8; 224];
    set(&mut i, 1, 11, 0b11100010010);
    set(&mut i, 16, 3, id);
    set(&mut i, 19, 8, sow >> 12);
    set(&mut i, 27, 12, sow & 0xFFF);
    let n = body.len().min(224 - 38);
    i[38..38 + n].copy_from_slice(&body[..n]);
    i
}

// ---------------------------------------------------------------- GLONASS

/// Sets a sign-magnitude field at bits `hi` down to `lo` of a string (bit k at `s[85 - k]`).
pub fn glo_set_sm(s: &mut [u8; 85], hi: usize, lo: usize, v: i64) {
    let len = (hi - lo + 1) as u32;
    s[85 - hi] = (v < 0) as u8;
    // The field lies inside the string, so the write cannot fail.
    let _ = put_u64(s, 86 - hi, v.unsigned_abs(), len - 1);
}

/// Sets an unsigned field.
pub fn glo_set_u(s: &mut [u8; 85], hi: usize, lo: usize, v: i64) {
    let _ = put_u64(s, 85 - hi, v as u64, (hi - lo + 1) as u32);
}

/// Fills the check bits of a string.
pub fn glo_finish(s: &mut [u8; 85]) {
    let _ = crate::fec::glonass().encode_bits(s);
}
