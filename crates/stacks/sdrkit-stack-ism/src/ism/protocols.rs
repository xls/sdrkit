//! The 48 device protocols of the ISM sensors stack as `sdrkit_proto_ookdev` table entries: each with its line code, its timing, its
//! frame test and its fields.
//!
//! No public standard defines these devices.

use super::checks::{
    CRC16_ERT_IDM, CRC16_ERT_SCM, CRC16_HONEYWELL_8050, CRC4_KEDSUM, CRC8_31, CRC8_HYUNDAI,
    CRC8_PMV107J, CRC8_RUBICSON, CRC8_SCHRADER, CRC8_TOYOTA,
};
use sdrkit_proto_ookdev::reading::measure;
use sdrkit_proto_ookdev::{BitRows, Check, Coding, Ctx, Modulation, ProtocolDef, Reading, Timing};
use sdrkit_util_bits::checksum::{
    each_byte_even_parity, lfsr_digest16, lfsr_digest8, lfsr_digest8_reflect, nibble_sum, sum8,
    xor8,
};
use sdrkit_util_bits::crc::catalog::{
    CRC16_EN_13757, CRC16_SPI_FUJITSU, CRC16_UMTS, CRC8_NRSC_5, CRC8_SMBUS,
};
use sdrkit_util_bits::line::{three_of_six, ManchesterConv};
use sdrkit_util_bits::pack::{pack_into, BitOrder, Pad};
use std::cell::RefCell;

/// The kind of a device that a protocol decodes, in snake_case.
pub fn kind_of(protocol: usize) -> &'static str {
    KINDS.get(protocol).copied().unwrap_or("sensor")
}

const KINDS: [&str; 48] = [
    "weather", "weather", "weather", "weather", "weather", "weather", "weather", "weather",
    "weather", "weather", "weather", "weather", "weather", "weather", "weather", "weather",
    "weather", "weather", "meter", "meter", "weather", "sensor", "sensor", "weather", "tpms",
    "tpms", "remote", "weather", "weather", "weather", "weather", "weather", "weather", "tpms",
    "tpms", "tpms", "tpms", "tpms", "tpms", "tpms", "energy", "energy", "zwave", "zwave", "zwave",
    "meter", "remote", "remote",
];

const fn pd(
    name: &'static str,
    modulation: Modulation,
    timing: Timing,
    decode: sdrkit_proto_ookdev::DecodeFn,
) -> ProtocolDef {
    ProtocolDef {
        name,
        modulation,
        timing,
        despike_us: 0.0,
        decode,
    }
}

const OOK: Modulation = Modulation::Ook;
const FSK: Modulation = Modulation::Fsk;
use Coding::{ManchesterZero as MZ, Nrz, Ppm, Pwm};

/// A timing with a gap, sync and tolerance (0 for none).
const fn t(c: Coding, s: f32, l: f32, r: f32, g: f32, y: f32, tol: f32) -> Timing {
    Timing::new(c, s, l, r)
        .with_gap(g)
        .with_sync(y)
        .with_tolerance(tol)
}

macro_rules! dec {
    ($f:ident) => {{
        fn wrap(rows: &BitRows, ctx: &mut Ctx, out: &mut Vec<Reading>) {
            out.extend($f(rows, ctx));
        }
        wrap
    }};
}

/// The Manchester half-bit of the Itron ERT meters, µs: a data bit of 1/32768 s (32768 bit/s).
pub const ERT_HALF_US: f32 = (1e6 / 65536.0) as f32;

/// Every protocol, in the order of their numbers.
pub static PROTOCOLS: [ProtocolDef; 48] = [
    pd(
        "Nexus, FreeTec NC-7345, NX-3980, Solight TE82S, TFA 30.3209",
        OOK,
        t(Ppm, 1000., 2000., 5000., 3000., 0., 0.),
        dec!(nexus),
    ),
    pd(
        "Rubicson, TFA 30.3197, InFactory PT-310",
        OOK,
        t(Ppm, 1000., 2000., 4800., 3000., 0., 0.),
        dec!(rubicson),
    ),
    pd(
        "Generic temperature sensor 1",
        OOK,
        t(Ppm, 2000., 4000., 10000., 4800., 0., 0.),
        dec!(generic_temp),
    ),
    pd(
        "LaCrosse TX141-Bv2/Bv3, TX141TH-Bv2, TX141W, TFA 30.3221",
        OOK,
        t(Pwm, 208., 417., 1700., 625., 833., 0.),
        dec!(tx141),
    ),
    pd(
        "Acurite 592TXR tower, Acurite 5-in-1",
        OOK,
        t(Pwm, 220., 408., 4000., 500., 620., 0.),
        dec!(acurite_tower),
    ),
    pd(
        "Fine Offset WH1080 / WH3080",
        OOK,
        t(Pwm, 544., 1524., 2800., 0., 0., 0.),
        dec!(wh1080),
    ),
    pd(
        "Ambient Weather F007TH / F012TH, TFA 30.3208, SwitchDoc F016TH",
        OOK,
        t(MZ, 500., 0., 2400., 0., 0., 0.),
        dec!(f007th),
    ),
    pd(
        "Oregon Scientific v2.1 and v3",
        OOK,
        t(MZ, 440., 0., 2400., 0., 0., 0.),
        dec!(oregon),
    ),
    pd(
        "Acurite 606TX, Technoline TX960",
        OOK,
        t(Ppm, 2000., 4000., 10000., 7000., 0., 0.),
        dec!(acurite606),
    ),
    pd(
        "Acurite 609TXC",
        OOK,
        t(Ppm, 1000., 2000., 10000., 3000., 0., 0.),
        dec!(acurite609),
    ),
    pd(
        "Prologue, FreeTec NC-7104",
        OOK,
        t(Ppm, 2000., 4000., 10000., 7000., 0., 0.),
        dec!(prologue),
    ),
    pd(
        "Kedsum, Pearl NC-7415",
        OOK,
        t(Ppm, 2000., 4000., 9400., 4400., 0., 0.),
        dec!(kedsum),
    ),
    pd(
        "Globaltronics GT-WT-02",
        OOK,
        t(Ppm, 2500., 5000., 12000., 8000., 0., 0.),
        dec!(gtwt02),
    ),
    pd(
        "Bresser 3CH thermo-hygro",
        OOK,
        t(Pwm, 250., 500., 1250., 625., 750., 0.),
        dec!(bresser3ch),
    ),
    pd(
        "Fine Offset WH2 / WH2A / WH5, Telldus Proove, TFA 30.3225",
        OOK,
        t(Pwm, 500., 1500., 1200., 0., 0., 160.),
        dec!(wh2),
    ),
    pd(
        "ThermoPro TP11",
        OOK,
        t(Ppm, 500., 1500., 4000., 2000., 0., 0.),
        dec!(tp11),
    ),
    pd(
        "ThermoPro TP08 / TP12 / TP20",
        OOK,
        t(Ppm, 500., 1500., 4000., 2000., 0., 0.),
        dec!(tp12),
    ),
    pd(
        "FT-004-B",
        OOK,
        t(Ppm, 1956., 3900., 4000., 4000., 0., 0.),
        dec!(ft004b),
    ),
    pd(
        "Itron ERT SCM",
        OOK,
        t(MZ, ERT_HALF_US, 0., 64., 0., 0., 0.),
        dec!(ert_scm),
    ),
    pd(
        "Itron ERT IDM",
        OOK,
        t(MZ, ERT_HALF_US, 0., 20000., 0., 0., 0.),
        dec!(ert_idm),
    ),
    pd(
        "Solight TE44 / TE66, EMOS E0107T",
        OOK,
        t(Ppm, 972., 1932., 6000., 3000., 0., 0.),
        dec!(solight),
    ),
    pd(
        "Honeywell / Ademco 345 MHz door and window, 2Gig DW10/DW11",
        OOK,
        t(Nrz, 136., 136., 408., 0., 0., 0.),
        dec!(honeywell),
    ),
    pd(
        "Kerui PIR / contact",
        OOK,
        t(Pwm, 420., 960., 9900., 1100., 0., 160.),
        dec!(kerui),
    ),
    pd(
        "Springfield temperature and soil moisture",
        OOK,
        t(Ppm, 2000., 4000., 9200., 5000., 0., 0.),
        dec!(springfield),
    ),
    pd(
        "Schrader TPMS (FCC MRXGG4)",
        OOK,
        t(MZ, 120., 0., 480., 0., 0., 0.),
        dec!(schrader),
    ),
    pd(
        "Schrader EG53MA4 TPMS (Opel, Saab, Chevrolet)",
        OOK,
        t(MZ, 123., 0., 300., 0., 0., 0.),
        dec!(schrader_eg53),
    ),
    pd(
        "PT2260 / PT2262 / SC2260 / EV1527 remotes and sensors",
        OOK,
        t(Pwm, 464., 1404., 1800., 0., 0., 200.),
        dec!(ev1527),
    ),
    pd(
        "Bresser 5-in-1",
        FSK,
        t(Nrz, 124., 124., 25000., 0., 0., 0.),
        dec!(bresser5),
    ),
    pd(
        "Bresser 6-in-1 family",
        FSK,
        t(Nrz, 124., 124., 25000., 0., 0., 0.),
        dec!(bresser6),
    ),
    pd(
        "Bresser 7-in-1 family",
        FSK,
        t(Nrz, 124., 124., 25000., 0., 0., 0.),
        dec!(bresser7),
    ),
    pd(
        "LaCrosse TX29-IT, TFA 30.3159",
        FSK,
        Timing::new(Nrz, 55., 55., 4000.).with_width_check(),
        dec!(tx29),
    ),
    pd(
        "LaCrosse TX35DTH-IT, TFA 30.3155",
        FSK,
        Timing::new(Nrz, 105., 105., 4000.).with_width_check(),
        dec!(tx35),
    ),
    pd(
        "Fine Offset / Ecowitt WH24, WH65, WS69, HP1000, Misol WS2320",
        FSK,
        t(Nrz, 58., 58., 20000., 0., 0., 0.),
        dec!(wh24),
    ),
    pd(
        "Toyota TPMS (Pacific PMV-C210)",
        FSK,
        t(Nrz, 52., 52., 150., 0., 0., 0.),
        dec!(toyota),
    ),
    pd(
        "Pacific PMV-107J TPMS (Toyota, 315 MHz)",
        FSK,
        t(Nrz, 100., 100., 250., 0., 0., 0.),
        dec!(pmv107j),
    ),
    pd(
        "Citroen / Peugeot / Fiat / Mitsubishi TPMS (VDO)",
        FSK,
        t(Nrz, 52., 52., 150., 0., 0., 0.),
        dec!(citroen),
    ),
    pd(
        "Renault / Dacia TPMS",
        FSK,
        t(Nrz, 52., 52., 150., 0., 0., 0.),
        dec!(renault),
    ),
    pd(
        "Ford TPMS (VDO S180084730Z)",
        FSK,
        t(Nrz, 52., 52., 150., 0., 0., 0.),
        dec!(ford),
    ),
    pd(
        "Hyundai / Kia TPMS (VDO TG1C)",
        FSK,
        t(Nrz, 52., 52., 150., 0., 0., 0.),
        dec!(hyundai),
    ),
    pd(
        "Abarth 124 Spider / Fiat 124 / Mazda MX-5 ND TPMS (VDO TG1C)",
        FSK,
        t(Nrz, 52., 52., 150., 0., 0., 0.),
        dec!(abarth),
    ),
    pd(
        "Efergy e2 classic current clamp",
        FSK,
        t(Pwm, 64., 136., 400., 200., 500., 0.),
        dec!(efergy),
    ),
    pd(
        "CurrentCost TX and EnviR",
        FSK,
        t(Nrz, 250., 250., 8000., 0., 0., 0.),
        dec!(currentcost),
    ),
    ProtocolDef {
        name: "Z-Wave R1 (9.6 kbit/s)",
        modulation: FSK,
        timing: t(Nrz, 52., 52., 3000., 0., 0., 0.),
        despike_us: 25.0,
        decode: dec!(zwave_r1),
    },
    ProtocolDef {
        name: "Z-Wave R2 (40 kbit/s)",
        modulation: FSK,
        timing: t(Nrz, 25., 25., 1000., 0., 0., 0.),
        despike_us: 10.0,
        decode: dec!(zwave_r2),
    },
    pd(
        "Z-Wave R3 (100 kbit/s)",
        FSK,
        t(Nrz, 10., 10., 500., 0., 0., 0.),
        dec!(zwave_r3),
    ),
    pd(
        "Wireless M-Bus T and C (link header)",
        FSK,
        t(Nrz, 10., 10., 500., 0., 0., 0.),
        dec!(wmbus_header),
    ),
    pd(
        "Somfy RTS blind and awning remotes (rolling code)",
        OOK,
        t(Nrz, 604., 604., 20_000., 0., 0., 0.),
        dec!(somfy_rts),
    ),
    pd(
        "KeeLoq-style rolling-code remotes (HCS301 code word)",
        OOK,
        t(Pwm, 400., 800., 12_000., 3000., 0., 0.),
        dec!(keeloq),
    ),
];

// Field helpers: each quantity in the one unit of the measurement catalogue.

fn temp_c(r: Reading, v: f64, d: u8) -> Reading {
    r.num(&measure::TEMPERATURE_C, v, d)
}
/// A temperature the device sends in °F, as °C.
fn temp_f(r: Reading, v: f64, d: u8) -> Reading {
    r.num(&measure::TEMPERATURE_C, (v - 32.0) * 5.0 / 9.0, d.max(1))
}
fn hum(r: Reading, v: f64) -> Reading {
    r.num(&measure::HUMIDITY_PCT, v, 0)
}
/// A wind speed the device sends in km/h, as m/s.
fn wind_km_h(r: Reading, v: f64, d: u8) -> Reading {
    r.num(&measure::WIND_SPEED_M_S, v / 3.6, d.max(1))
}
/// A wind gust the device sends in km/h, as m/s.
fn gust_km_h(r: Reading, v: f64, d: u8) -> Reading {
    r.num(&measure::WIND_GUST_M_S, v / 3.6, d.max(1))
}
fn wind_m_s(r: Reading, v: f64) -> Reading {
    r.num(&measure::WIND_SPEED_M_S, v, 1)
}
fn gust_m_s(r: Reading, v: f64) -> Reading {
    r.num(&measure::WIND_GUST_M_S, v, 1)
}
fn wind_dir(r: Reading, v: f64, d: u8) -> Reading {
    r.num(&measure::WIND_DIR_DEG, v, d)
}
fn rain_mm(r: Reading, v: f64) -> Reading {
    r.num(&measure::RAIN_MM, v, 1)
}
fn kpa(r: Reading, v: f64, d: u8) -> Reading {
    r.num(&measure::PRESSURE_KPA, v, d)
}
/// A pressure the device sends in PSI, as kPa (1 PSI is 6.894757 kPa, NIST SP 811 appendix B).
fn psi(r: Reading, v: f64) -> Reading {
    r.num(&measure::PRESSURE_KPA, v * 6.894_757, 1)
}
fn uv(r: Reading, v: f64, d: u8) -> Reading {
    r.num(&measure::UV_INDEX, v, d)
}

// Row helpers.

/// The first 64 bytes of row `r`, zero padded.
fn bytes64(rows: &BitRows, r: usize) -> [u8; 64] {
    let mut b = [0u8; 64];
    rows.row(r).copy_padded(&mut b);
    b
}

/// The first 64 bytes of row `r` with every bit of the row inverted (bits past the row stay 0).
fn bytes64_inv(rows: &BitRows, r: usize) -> [u8; 64] {
    let mut b = bytes64(rows, r);
    let n = rows.len(r);
    for (i, x) in b.iter_mut().enumerate().take(n.div_ceil(8)) {
        *x = !*x;
        let left = n - i * 8;
        if left < 8 {
            *x &= !(0xFFu8 >> left);
        }
    }
    b
}

/// `n` bits of row `r` from `pos` (MSB first, zero padded) into `N` bytes.
fn ext<const N: usize>(rows: &BitRows, r: usize, pos: usize, n: usize) -> [u8; N] {
    let mut b = [0u8; N];
    let _ = rows.extract(r, pos, n.min(N * 8), &mut b);
    b
}

/// The bytes of the first row of at least `min` bits sent `k` times, when it is at most `max` bits long.
fn first_repeated(rows: &BitRows, k: usize, min: usize, max: usize) -> Option<[u8; 64]> {
    let r = rows.repeated_row(k, min)?;
    (rows.len(r) <= max).then(|| bytes64(rows, r))
}

thread_local! {
    static DECODED: RefCell<BitRows> = RefCell::new(BitRows::new(1, 4096));
}

/// Runs `f` with row `r` of `rows` Manchester-decoded from `pos` by `conv` (at most `max` bits) in a scratch row of this thread.
fn manchester<T>(
    rows: &BitRows,
    r: usize,
    pos: usize,
    max: usize,
    conv: ManchesterConv,
    f: impl FnOnce(&BitRows) -> T,
) -> T {
    DECODED.with(|d| {
        let mut d = d.borrow_mut();
        rows.decode_manchester(r, pos, max, conv, &mut d);
        f(&d)
    })
}

/// Runs `f` with row `r` of `rows` FM0-decoded from `pos` (at most `max` bits) in a scratch row of this thread.
fn fm0<T>(rows: &BitRows, r: usize, pos: usize, max: usize, f: impl FnOnce(&BitRows) -> T) -> T {
    DECODED.with(|d| {
        let mut d = d.borrow_mut();
        rows.decode_fm0(r, pos, max, &mut d);
        f(&d)
    })
}

/// Every bit of `p` inverted (a pattern searched in rows the device sends inverted).
fn inv<const N: usize>(p: [u8; N]) -> [u8; N] {
    p.map(|x| !x)
}

fn temp12(hi: u8, lo: u8) -> f64 {
    let v = ((hi as i32 & 0xF) << 8) | lo as i32;
    let v = if v & 0x800 != 0 { v - 4096 } else { v };
    v as f64 * 0.1
}

fn bcd(b: u8) -> u32 {
    (b >> 4) as u32 * 10 + (b & 0xF) as u32
}

fn bcd_ok(b: u8) -> bool {
    (b >> 4) <= 9 && (b & 0xF) <= 9
}

fn bcd6(a: u8, b: u8, c: u8) -> u32 {
    bcd(a) * 10000 + bcd(b) * 100 + bcd(c)
}

fn rubicson_crc(b: &[u8]) -> bool {
    let five = [
        b[0],
        b[1],
        b[2],
        b[3] & 0xF0,
        (b[3] & 0x0F) << 4 | b[4] >> 4,
    ];
    CRC8_RUBICSON.checksum(&five) == 0
}

/// Reverses the bits within each nibble (Oregon Scientific sends each nibble least significant bit first).
pub fn reflect_nibbles(b: u8) -> u8 {
    let r4 = |n: u8| ((n & 1) << 3) | ((n & 2) << 1) | ((n & 4) >> 1) | ((n & 8) >> 3);
    (r4(b >> 4) << 4) | r4(b & 0xF)
}

/// Whether the Oregon Scientific sum of the first 15 nibbles matches its two check nibbles.
pub fn oregon_checksum_ok(m: &[u8]) -> bool {
    nibble_sum(m, 15) == ((m[8] >> 4) << 4) | (m[7] & 0xF)
}

fn hex_id(b: &[u8]) -> String {
    let mut s = String::with_capacity(2 * b.len());
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

// 1
fn nexus(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let r = rows.repeated_row(3, 36)?;
    if rows.len(r) > 37 {
        return None;
    }
    let b = bytes64(rows, r);
    if b[3] >> 4 != 0xF
        || (b[0] == 0 && b[2] == 0 && b[3] == 0)
        || (b[0] == 0xFF && b[2] == 0xFF && b[3] == 0xFF)
        || (b[1] >> 4) & 3 == 3
        || rubicson_crc(&b)
    {
        return None;
    }
    let h = ((b[3] & 0xF) << 4) | (b[4] >> 4);
    if h > 100 {
        return None;
    }
    let model = if h != 0 { "Nexus-TH" } else { "Nexus-T" };
    let mut rd = Reading::new(model, "weather", b[0].to_string(), Check::Repeat)
        .with_channel(((b[1] >> 4) & 3) as i32 + 1)
        .with_battery(b[1] & 0x80 != 0);
    rd = temp_c(rd, temp12(b[1], b[2]), 1);
    if h != 0 {
        rd = hum(rd, h as f64);
    }
    Some(rd)
}

// 2
fn rubicson(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let r = rows.repeated_row(3, 36)?;
    if rows.len(r) > 38 {
        return None;
    }
    let b = bytes64(rows, r);
    if b[3] >> 4 != 0xF || !rubicson_crc(&b) {
        return None;
    }
    let rd = Reading::new(
        "Rubicson-Temperature",
        "weather",
        b[0].to_string(),
        Check::Crc,
    )
    .with_channel(((b[1] >> 4) & 3) as i32 + 1)
    .with_battery(b[1] & 0x80 != 0);
    Some(temp_c(rd, temp12(b[1], b[2]), 1))
}

// 21
fn solight(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let r = rows.repeated_row(3, 37)?;
    if rows.len(r) != 37 {
        return None;
    }
    let b = bytes64(rows, r);
    if b[3] >> 4 != 0xF || !rubicson_crc(&b) {
        return None;
    }
    let rd = Reading::new("Solight-TE44", "weather", b[0].to_string(), Check::Crc)
        .with_channel(((b[1] >> 4) & 3) as i32 + 1)
        .with_battery(b[1] & 0x80 != 0);
    Some(temp_c(rd, temp12(b[1], b[2]), 2))
}

// 3
fn generic_temp(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() < 10 || (1..10).any(|i| rows.len(i) != 24) {
        return None;
    }
    let b = bytes64(rows, 1);
    if (b[0] == 0 && b[1] == 0 && b[2] == 0) || (b[0] == 0xFF && b[1] == 0xFF && b[2] == 0xFF) {
        return None;
    }
    if rows.repeated_row(3, 24).is_none_or(|r| rows.len(r) != 24) {
        return None;
    }
    let raw = ((((b[1] as u16) & 0x3F) << 10) | ((b[2] as u16) << 2)) as i16 >> 4;
    let rd = Reading::new(
        "Generic-Temperature",
        "weather",
        b[0].to_string(),
        Check::Repeat,
    );
    Some(temp_c(rd, raw as f64 * 0.1, 2).with(&measure::BATTERY_BITS, (b[1] >> 6) as i64))
}

// 4
fn tx141(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    // The device sends inverted bits; inversion does not change which rows repeat.
    let n = rows.num_rows();
    let k = if n <= 5 { 3 } else { 5 };
    let r = rows
        .repeated_row(k, 32)
        .or_else(|| rows.repeated_row(2, 64))
        .or_else(|| {
            if n > 4 {
                return None;
            }
            (0..n).find(|&i| {
                (rows.len(i) == 40 || rows.len(i) == 41) && {
                    let b = bytes64_inv(rows, i);
                    lfsr_digest8_reflect(&b[0..4], 0x31, 0xF4) == b[4]
                }
            })
        })?;
    let len = rows.len(r);
    let b = bytes64_inv(rows, r);
    if len >= 64 {
        if b[0] >> 3 != 1 || CRC8_31.checksum(&b[0..8]) != 0 {
            return None;
        }
        let id = ((b[0] as u32 & 7) << 16) | (b[1] as u32) << 8 | b[2] as u32;
        let raw1 = ((b[4] as u32) << 4) | (b[5] >> 4) as u32;
        let raw2 = (((b[5] & 0xF) as u32) << 8) | b[6] as u32;
        let rd = Reading::new(
            "LaCrosse-TX141W",
            "weather",
            format!("{id:05x}"),
            Check::Crc,
        )
        .with_channel(((b[3] >> 4) & 3) as i32)
        .with_battery(b[3] & 0x80 == 0);
        return match b[3] & 0xF {
            1 => Some(hum(temp_c(rd, (raw1 as f64 - 500.0) * 0.1, 1), raw2 as f64)),
            2 => Some(wind_dir(
                wind_km_h(rd, raw1 as f64 * 0.1, 1),
                raw2 as f64,
                0,
            )),
            _ => None,
        };
    }
    let (model, th) = match len {
        41 if n > 12 => return None,
        40 | 41 => ("LaCrosse-TX141THBv2", true),
        37..=39 => ("LaCrosse-TX141Bv2", false),
        32 => ("LaCrosse-TX141B", false),
        l if l <= 36 => ("LaCrosse-TX141Bv3", false),
        _ => return None,
    };
    if b[0] == 0 {
        return None;
    }
    let temp = ((((b[1] & 0xF) as i32) << 8 | b[2] as i32) - 500) as f64 * 0.1;
    if !(-40.0..=140.0).contains(&temp) {
        return None;
    }
    if th {
        let h = b[3];
        if !(1..=100).contains(&h) || lfsr_digest8_reflect(&b[0..4], 0x31, 0xF4) != b[4] {
            return None;
        }
        let rd = Reading::new(model, "weather", format!("{:02x}", b[0]), Check::Crc)
            .with_channel(((b[1] >> 4) & 3) as i32)
            .with_battery(b[1] & 0x80 == 0);
        Some(hum(temp_c(rd, temp, 1), h as f64))
    } else {
        let mut rd = Reading::new(model, "weather", format!("{:02x}", b[0]), Check::Repeat)
            .with_battery(b[1] & 0x80 != 0);
        if len != 32 {
            rd = rd.with_channel(((b[1] >> 4) & 3) as i32);
        }
        Some(temp_c(rd, temp, 1))
    }
}

// 5
fn acurite_tower(rows: &BitRows, _: &mut Ctx) -> Vec<Reading> {
    let mut out = Vec::new();
    for r in 0..rows.num_rows() {
        let n = rows.len(r) / 8;
        if !(6..=10).contains(&n) {
            continue;
        }
        let b = bytes64_inv(rows, r);
        if b[0] == 0 && b[1] == 0 && b[2] == 0 && b[n - 1] == 0 {
            continue;
        }
        // Each of bytes 2 to n-2 carries its own even parity bit.
        if sum8(&b[..n - 1]) != b[n - 1] || !each_byte_even_parity(&b[2..n - 1]) || b[0] >> 6 == 1 {
            continue;
        }
        let ch = ["C", "E", "B", "A"][(b[0] >> 6) as usize];
        let batt = b[2] & 0x40 != 0;
        match b[2] & 0x3F {
            0x04 if n == 7 => {
                let h = b[3] & 0x7F;
                if h > 100 && h != 127 {
                    continue;
                }
                let t = ((((b[4] & 0x7F) as i32) << 7 | (b[5] & 0x7F) as i32) - 1000) as f64 * 0.1;
                if !(-40.0..=70.0).contains(&t) {
                    continue;
                }
                let id = ((b[0] as u32 & 0x3F) << 8) | b[1] as u32;
                let mut rd =
                    Reading::new("Acurite-Tower", "weather", id.to_string(), Check::Checksum)
                        .with_channel_name(ch)
                        .with_battery(batt);
                rd = temp_c(rd, t, 1);
                if h != 127 {
                    rd = hum(rd, h as f64);
                }
                out.push(rd);
            }
            m @ (0x31 | 0x38) if n == 8 => {
                let id = ((b[0] as u32 & 0x0F) << 8) | b[1] as u32;
                let raw = ((b[3] as u32 & 0x1F) << 3) | ((b[4] as u32 & 0x70) >> 4);
                let ws = if raw > 0 {
                    raw as f64 * 0.8278 + 1.0
                } else {
                    0.0
                };
                let mut rd =
                    Reading::new("Acurite-5n1", "weather", id.to_string(), Check::Checksum)
                        .with_channel_name(ch)
                        .with_battery(batt);
                rd = wind_km_h(rd, ws, 1);
                if m == 0x31 {
                    const T: [u8; 16] = [14, 11, 13, 12, 15, 10, 0, 9, 3, 6, 4, 5, 2, 7, 1, 8];
                    rd = wind_dir(rd, T[(b[4] & 0xF) as usize] as f64 * 22.5, 1).num(
                        &measure::RAIN_MM,
                        (((b[5] as u32 & 0x7F) << 7) | (b[6] as u32 & 0x7F)) as f64 * 0.254,
                        1,
                    );
                } else {
                    let f =
                        ((((b[4] & 0x0F) as i32) << 7 | (b[5] & 0x7F) as i32) - 400) as f64 * 0.1;
                    let h = b[6] & 0x7F;
                    if !(-40.0..=158.0).contains(&f) || h > 100 {
                        continue;
                    }
                    rd = hum(temp_f(rd, f, 1), h as f64);
                }
                out.push(rd);
            }
            _ => {}
        }
    }
    out
}

// 6
fn wh1080(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() != 1 {
        return None;
    }
    let len = rows.len(0);
    let b: [u8; 64] = if (88..=99).contains(&len) {
        bytes64(rows, 0)
    } else if len == 87 {
        let mut v = [0u8; 64];
        v[0] = (rows.row(0).byte(0) >> 1) | 0x80;
        let rest: [u8; 10] = ext(rows, 0, 7, 80);
        v[1..11].copy_from_slice(&rest);
        v
    } else {
        return None;
    };
    if b[0] != 0xFF || CRC8_NRSC_5.checksum(&b[0..11]) != 0 || b[1] >> 4 != 0xA {
        return None;
    }
    const D: [u16; 16] = [
        0, 23, 45, 68, 90, 113, 135, 158, 180, 203, 225, 248, 270, 293, 315, 338,
    ];
    let id = ((b[1] & 0xF) << 4) | (b[2] >> 4);
    let t = ((((b[2] & 3) as i32) << 8 | b[3] as i32) - 400) as f64 * 0.1;
    let rd = Reading::new("Fineoffset-WHx080", "weather", id.to_string(), Check::Crc)
        .with_battery(b[9] >> 4 != 1);
    let rd = wind_dir(
        hum(temp_c(rd, t, 1), b[4] as f64),
        D[(b[9] & 0xF) as usize] as f64,
        0,
    );
    let rd = gust_km_h(
        wind_km_h(rd, b[5] as f64 * 0.34 * 3.6, 2),
        b[6] as f64 * 0.34 * 3.6,
        2,
    );
    Some(rain_mm(
        rd,
        ((((b[7] & 0xF) as u32) << 8) | b[8] as u32) as f64 * 0.3,
    ))
}

// 7
fn f007th(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    for r in 0..rows.num_rows() {
        let p = rows
            .search(r, 0, &[0x01, 0x40], 12)
            .or_else(|| rows.search(r, 0, &[0xFD, 0x40], 12));
        let Some(p) = p else { continue };
        if p + 8 + 48 > rows.len(r) {
            continue;
        }
        let b: [u8; 6] = ext(rows, r, p + 8, 48);
        if lfsr_digest8(&b[0..5], 0x98, 0x3E) ^ 0x64 != b[5] {
            continue;
        }
        let h = b[4];
        let f = ((((b[2] & 0xF) as i32) << 8 | b[3] as i32) - 400) as f64 * 0.1;
        if h > 100 || !(-40.0..344.0).contains(&f) {
            continue;
        }
        let rd = Reading::new(
            "Ambientweather-F007TH",
            "weather",
            b[1].to_string(),
            Check::Crc,
        )
        .with_channel(((b[2] >> 4) & 7) as i32 + 1)
        .with_battery(b[2] & 0x80 == 0);
        return Some(hum(temp_f(rd, f, 1), h as f64));
    }
    None
}

fn oregon_fields(m: &[u8], model: &'static str) -> Option<Reading> {
    let digits = [m[5] >> 4, m[4] & 0xF, m[4] >> 4, m[6] & 0xF, m[6] >> 4];
    if digits.iter().any(|&d| d > 9) {
        return None;
    }
    let mut t = (m[5] >> 4) as f64 * 10.0
        + (m[4] & 0xF) as f64
        + (m[4] >> 4) as f64 * 0.1
        + (m[5] & 7) as f64 * 100.0;
    if m[5] & 8 != 0 {
        t = -t;
    }
    let h = (m[6] & 0xF) as f64 * 10.0 + (m[6] >> 4) as f64;
    let id = (m[2] & 0x0F) | (m[3] & 0xF0);
    let rd = Reading::new(model, "weather", id.to_string(), Check::Checksum)
        .with_channel((m[2] >> 4) as i32)
        .with_battery(m[3] & 0x4 == 0);
    Some(hum(temp_c(rd, t, 2), h))
}

// 8
fn oregon(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let b = bytes64(rows, 0);
    if (b[1] == 0x55 && b[2] == 0x55) || (b[1] == 0xAA && b[2] == 0xAA) {
        for k in 0..8 {
            let s: [u8; 2] = ext(rows, 0, 24 + k, 16);
            if s != [0x55, 0x99] && s != [0xAA, 0x99] {
                continue;
            }
            let found = manchester(rows, 0, 40 + k, 173, ManchesterConv::Ieee, |d| {
                let nbits = d.len(0);
                let mut m = bytes64(d, 0);
                m.iter_mut().for_each(|x| *x = reflect_nibbles(*x));
                let id = (m[0] as u16) << 8 | m[1] as u16;
                let model = match (id, nbits) {
                    (0x1D20, 76) => "Oregon-THGR122N",
                    (0x1D20, 68) => "Oregon-THGR228N",
                    (0x1D30, 68 | 76) => "Oregon-THGR968",
                    _ => return None,
                };
                if !oregon_checksum_ok(&m) {
                    return None;
                }
                oregon_fields(&m, model)
            });
            if found.is_some() {
                return found;
            }
        }
        return None;
    }
    let v3 = (b[0] & 0xF == 0xF && b[1] == 0xFF && b[2] >> 6 == 3)
        || (b[0] & 0xF == 0 && b[1] == 0 && b[2] >> 6 == 0);
    if !v3 {
        return None;
    }
    let n = rows.len(0);
    let start = rows.search(0, 0, &[0x00, 0x05], 16)? + 16;
    if !(56..=352).contains(&(n - start)) {
        return None;
    }
    let mut m: [u8; 64] = ext(rows, 0, start, n - start);
    m.iter_mut().for_each(|x| *x = reflect_nibbles(*x));
    let id = (m[0] as u16) << 8 | m[1] as u16;
    if (id & 0xF0FF != 0xF024 && id != 0xF8B4) || !oregon_checksum_ok(&m) {
        return None;
    }
    oregon_fields(&m, "Oregon-THGR810")
        .filter(|r| (-50.0..=70.0).contains(&r.get(&measure::TEMPERATURE_C).unwrap_or(999.0)))
}

// 9
fn acurite606(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let b = first_repeated(rows, 3, 32, 33)?;
    if b[..4].iter().all(|&x| x == 0) || lfsr_digest8(&b[0..3], 0x98, 0xF1) != b[3] {
        return None;
    }
    let rd = Reading::new(
        "Acurite-606TX",
        "weather",
        b[0].to_string(),
        Check::Checksum,
    )
    .with_channel(((b[1] >> 4) & 3) as i32 + 1)
    .with_battery(b[1] & 0x80 != 0);
    Some(temp_c(rd, temp12(b[1], b[2]), 1))
}

// 10
fn acurite609(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    (0..rows.num_rows()).find_map(|r| {
        if rows.len(r) != 40 {
            return None;
        }
        let b = bytes64(rows, r);
        let s = sum8(&b[0..4]);
        if s != b[4] || s == 0 || b[3] > 100 {
            return None;
        }
        let rd = Reading::new(
            "Acurite-609TXC",
            "weather",
            b[0].to_string(),
            Check::Checksum,
        )
        .with_battery(b[1] & 0x80 == 0);
        Some(hum(temp_c(rd, temp12(b[1], b[2]), 1), b[3] as f64))
    })
}

// 11
fn prologue(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.len(0) != 0 && rows.len(0) <= 8 {
        return None;
    }
    let b = first_repeated(rows, 4, 36, 37)?;
    if b[0] >> 4 != 9 && b[0] >> 4 != 5 {
        return None;
    }
    let id = ((b[0] & 0xF) << 4) | (b[1] >> 4);
    let t = ((((b[2] as u16) << 8) | (b[3] & 0xF0) as u16) as i16 >> 4) as f64 * 0.1;
    let h = ((b[3] & 0xF) << 4) | (b[4] >> 4);
    let mut rd = Reading::new("Prologue-TH", "weather", id.to_string(), Check::Repeat)
        .with_channel((b[1] & 3) as i32 + 1);
    rd = temp_c(rd, t, 1);
    if h != 0xCC {
        rd = hum(rd, h as f64);
    }
    Some(rd)
}

// 12
fn kedsum(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() < 6 || (0..5).any(|i| rows.len(i) != 0) {
        return None;
    }
    let r = rows.repeated_row(4, 42)?;
    if rows.len(r) != 42 {
        return None;
    }
    let b: [u8; 5] = ext(rows, r, 2, 40);
    if CRC4_KEDSUM.checksum(&b[0..4]) ^ (b[4] >> 4) != b[4] & 0xF {
        return None;
    }
    let raw = (((b[2] & 0xF) as i32) << 8) | (((b[2] >> 4) as i32) << 4) | (b[1] & 0xF) as i32;
    let f = (raw - 900) as f64 * 0.1;
    let rd = Reading::new("Kedsum-TH", "weather", b[0].to_string(), Check::Crc)
        .with_channel(((b[1] >> 4) & 3) as i32 + 1)
        .with_battery(b[1] >> 6 == 2);
    Some(hum(
        temp_c(rd, (f - 32.0) / 1.8, 1),
        (((b[3] & 0xF) << 4) | (b[3] >> 4)) as f64,
    ))
}

// 13
fn gtwt02(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() < 2 {
        return None;
    }
    (0..rows.num_rows()).find_map(|r| {
        let b: [u8; 5] = match rows.len(r) {
            37 => ext(rows, r, 0, 37),
            39 => ext(rows, r, 2, 37),
            _ => return None,
        };
        if b.iter().all(|&x| x == 0) {
            return None;
        }
        let nib = |x: u8| (x >> 4) as u32 + (x & 0xF) as u32;
        let sum =
            (nib(b[0]) + nib(b[1]) + nib(b[2]) + (b[3] >> 4) as u32 + (b[3] & 0xE) as u32) % 64;
        if sum != (((b[3] & 1) as u32) << 5) + (b[4] >> 3) as u32 {
            return None;
        }
        let ch = (b[1] >> 4) & 3;
        let t = temp12(b[1], b[2]);
        if ch > 2 || !(-20.0..=60.0).contains(&t) {
            return None;
        }
        let h = match b[3] >> 1 {
            10 => 0,
            110 => 100,
            x @ 20..=90 => x,
            _ => return None,
        };
        let rd = Reading::new("GT-WT02", "weather", b[0].to_string(), Check::Checksum)
            .with_channel(ch as i32 + 1)
            .with_battery(b[1] & 0x80 == 0);
        Some(hum(temp_c(rd, t, 1), h as f64))
    })
}

// 14
fn bresser3ch(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let b = first_repeated(rows, 3, 40, 42)?.map(|x| !x);
    if b[0]
        .wrapping_add(b[1])
        .wrapping_add(b[2])
        .wrapping_add(b[3])
        .wrapping_sub(b[4])
        != 0
    {
        return None;
    }
    let ch = (b[1] >> 4) & 3;
    let f = ((((b[1] & 0xF) as i32) << 8) + b[2] as i32 - 900) as f64 * 0.1;
    if ch == 0 || b[3] > 100 || !(-20.0..=160.0).contains(&f) {
        return None;
    }
    let rd = Reading::new("Bresser-3CH", "weather", b[0].to_string(), Check::Checksum)
        .with_channel(ch as i32)
        .with_battery(b[1] & 0x80 == 0);
    Some(hum(temp_c(rd, (f - 32.0) / 1.8, 1), b[3] as f64))
}

// 15
fn wh2(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let row = rows.row(0);
    let (model, b): (&'static str, [u8; 6]) = match (rows.len(0), row.byte(0)) {
        (48, 0xFF) => ("Fineoffset-WH2", ext(rows, 0, 8, 40)),
        (55, 0xFE) => {
            let b: [u8; 6] = ext(rows, 0, 7, 48);
            (
                if b[3] == 0xFF {
                    "TFA-303225"
                } else {
                    "Fineoffset-WH2A"
                },
                b,
            )
        }
        (47, 0xFE) => ("Fineoffset-WH5", ext(rows, 0, 7, 40)),
        (49, 0xFF) if row.byte(1) & 0x80 != 0 => ("Fineoffset-TelldusProove", ext(rows, 0, 9, 40)),
        _ => return None,
    };
    if CRC8_31.checksum(&b[0..4]) != b[4] || b[0] >> 4 != 4 {
        return None;
    }
    let tfa = model == "TFA-303225";
    if tfa && sum8(&b[0..5]) != b[5] {
        return None;
    }
    let id = ((b[0] & 0xF) << 4) | (b[1] >> 4);
    let raw = (((b[1] & 0xF) as i32) << 8) | b[2] as i32;
    let mut batt = None;
    let t = if tfa {
        batt = Some(raw & 0x800 == 0);
        ((raw & 0x7FF) - 400) as f64 * 0.1
    } else if model == "Fineoffset-WH5" {
        let t = (raw - 400) as f64 * 0.1;
        if !(-40.0..=60.0).contains(&t) {
            return None;
        }
        t
    } else {
        let m = (raw & 0x7FF) as f64 * 0.1;
        if raw & 0x800 != 0 {
            -m
        } else {
            m
        }
    };
    let mut rd = temp_c(
        Reading::new(model, "weather", id.to_string(), Check::Crc),
        t,
        1,
    );
    if let Some(ok) = batt {
        rd = rd.with_battery(ok);
    }
    if b[3] != 0xFF {
        rd = hum(rd, b[3] as f64);
    }
    Some(rd)
}

// 16
fn tp11(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let b = first_repeated(rows, 2, 32, 33)?;
    if b[..4].iter().all(|&x| x == 0) || b[..4].iter().all(|&x| x == 0xFF) {
        return None;
    }
    if lfsr_digest8_reflect(&b[0..3], 0x51, 0x04) != b[3] {
        return None;
    }
    let id = ((b[0] as u32) << 4) | (b[1] >> 4) as u32;
    let t = (((((b[1] & 0xF) as i32) << 8) | b[2] as i32) - 200) as f64 * 0.1;
    Some(temp_c(
        Reading::new("Thermopro-TP11", "weather", id.to_string(), Check::Crc),
        t,
        1,
    ))
}

// 17
fn tp12(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let n = rows.num_rows();
    let need = if n <= 5 { 2 } else { 5 };
    (0..n).find_map(|r| {
        if rows.len(r) != 41 {
            return None;
        }
        let b: [u8; 5] = ext(rows, r, 0, 40);
        let c = (0..n)
            .filter(|&j| rows.len(j) >= 40 && ext::<5>(rows, j, 0, 40) == b)
            .count();
        if c < need
            || b[..4].iter().all(|&x| x == 0)
            || lfsr_digest8_reflect(&b[0..4], 0x51, 0x04) != b[4]
        {
            return None;
        }
        let t1 = (((((b[2] & 0xF0) as i32) << 4) | b[1] as i32) - 200) as f64 * 0.1;
        let t2 = (((((b[2] & 0xF) as i32) << 8) | b[3] as i32) - 200) as f64 * 0.1;
        Some(
            Reading::new("Thermopro-TP12", "weather", b[0].to_string(), Check::Crc)
                .num(&measure::TEMPERATURE_1_C, t1, 1)
                .num(&measure::TEMPERATURE_2_C, t2, 1),
        )
    })
}

// 18
fn ft004b(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.len(0) != 137 && rows.len(0) != 138 {
        return None;
    }
    // Three copies of 46 bits: each bit by majority.
    let mut bits = [0u8; 48];
    for (i, bit) in bits.iter_mut().enumerate().take(46) {
        *bit = (rows.bit(0, i) + rows.bit(0, i + 46) + rows.bit(0, i + 92) >= 2) as u8;
    }
    let mut m = [0u8; 6];
    pack_into(&bits, BitOrder::Lsb, Pad::Zero, &mut m).ok()?;
    if m[0] != 0xF4 {
        return None;
    }
    let t = ((((m[4] & 7) as u32) << 8) | m[3] as u32) as f64 * 0.05 - 40.0;
    Some(temp_c(
        Reading::new("FT-004B", "weather", "0", Check::Repeat),
        t,
        1,
    ))
}

// 19
fn ert_scm(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.len(0) != 96 {
        return None;
    }
    let b = bytes64(rows, 0);
    if b[..4].iter().all(|&x| x == 0) || CRC16_ERT_SCM.checksum(&b[2..12]) != 0 {
        return None;
    }
    let id = (((b[2] as u32) & 6) << 23) | (b[7] as u32) << 16 | (b[8] as u32) << 8 | b[9] as u32;
    let rd = Reading::new("ERT-SCM", "meter", id.to_string(), Check::Crc);
    let rd = rd.with(&measure::PHYSICAL_TAMPER, (b[3] >> 6) != 0);
    let rd = rd.with(&measure::ERT_TYPE, ((b[3] >> 2) & 0xF) as i64);
    let rd = rd.with(&measure::ENCODER_TAMPER, (b[3] & 3) != 0);
    Some(rd.with(
        &measure::CONSUMPTION,
        (((b[4] as u32) << 16) | (b[5] as u32) << 8 | b[6] as u32) as i64,
    ))
}

// 20
fn ert_idm(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.len(0) < 720 {
        return None;
    }
    let p = rows.search(0, 0, &[0x16, 0xA3, 0x1C], 24)?;
    if p + 720 > rows.len(0) {
        return None;
    }
    let b: [u8; 90] = ext(rows, 0, p, 720);
    if CRC16_ERT_IDM.checksum(&b[2..88]) != (b[88] as u16) << 8 | b[89] as u16 {
        return None;
    }
    let be32 = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let rd = Reading::new("ERT-IDM", "meter", be32(7).to_string(), Check::Crc);
    let rd = rd.with(&measure::ERT_TYPE, (b[6]) as i64);
    let rd = rd.with(&measure::PACKET_TYPE, (b[2]) as i64);
    let rd = rd.with(&measure::APP_VERSION, (b[5]) as i64);
    let rd = rd.with(&measure::INTERVAL_COUNT, (b[11]) as i64);
    let rd = rd.with(&measure::LAST_CONSUMPTION, (be32(27)) as i64);
    Some(rd.with(
        &measure::TRANSMIT_OFFSET,
        ((b[84] as u32) << 8 | b[85] as u32) as i64,
    ))
}

// 22
fn honeywell(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.len(0) < 120 {
        return None;
    }
    let mut from = 0;
    while let Some(p) = rows.search(0, from, &[0x55, 0x55, 0x56], 24) {
        from = p + 1;
        // A frame that ends with a 0 bit ends with a space chip, which the slicer does not see: 47 bits and one mark chip
        // at the end of the row read as 48 bits (the bytes are zero padded).
        let whole = rows.len(0) == p + 24 + 95;
        let b = manchester(rows, 0, p + 24, 96, ManchesterConv::Ieee, |d| {
            (d.len(0) >= 48 || (whole && d.len(0) == 47)).then(|| bytes64(d, 0))
        });
        let Some(b) = b else { continue };
        let ch = b[0] >> 4;
        let id = (((b[0] & 0xF) as u32) << 16) | (b[1] as u32) << 8 | b[2] as u32;
        let crc = (b[4] as u16) << 8 | b[5] as u16;
        let calc = if matches!(ch, 2 | 4 | 9 | 10 | 12) {
            CRC16_HONEYWELL_8050.checksum(&b[0..4])
        } else {
            CRC16_UMTS.checksum(&b[0..4])
        };
        if calc != crc || (id == 0 && crc == 0) {
            continue;
        }
        let e = b[3];
        let rd = Reading::new(
            "Honeywell-Security",
            "sensor",
            format!("{id:05x}"),
            Check::Crc,
        )
        .with_channel(ch as i32)
        .with_battery(e & 0x08 == 0)
        .with(&measure::DOOR_OPEN, e & 0x80 != 0);
        let rd = rd.with(&measure::EVENT, (e) as i64);
        let rd = rd.with(&measure::TAMPER, ((e >> 6) & 1) != 0);
        let rd = rd.with(&measure::ALARM, ((e >> 4) & 1) as i64);
        return Some(rd.with(&measure::HEARTBEAT, ((e >> 2) & 1) as i64));
    }
    None
}

// 23
fn kerui(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let r = rows.repeated_row(9, 25)?;
    if rows.len(r) != 25 {
        return None;
    }
    let raw = bytes64(rows, r);
    if raw[..3].iter().all(|&x| x == 0) {
        return None;
    }
    let b = [!raw[0], !raw[1], !raw[2]];
    let id = ((b[0] as u32) << 12) | (b[1] as u32) << 4 | (b[2] >> 4) as u32;
    let (state, low) = match b[2] & 0xF {
        0xA => ("motion", false),
        0xE => ("open", false),
        0x7 => ("close", false),
        0xB => ("tamper", false),
        0x5 => ("water", false),
        0xF => ("battery low", true),
        _ => return None,
    };
    Some(
        Reading::new(
            "Kerui-Security",
            "sensor",
            format!("{id:05x}"),
            Check::Repeat,
        )
        .with_battery(!low)
        .with(&measure::STATE, state),
    )
}

// 24
fn springfield(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let r = rows.repeated_row(3, 36)?;
    if rows.len(r) > 37 {
        return None;
    }
    let b = bytes64(rows, r);
    if b[..4].iter().all(|&x| x == 0) || b[..4].iter().all(|&x| x == 0xFF) {
        return None;
    }
    let x = xor8(&b[0..4]);
    if (x >> 4) ^ (x & 0xF) != 0 {
        return None;
    }
    let t = temp12(b[1], b[2]);
    let m = (b[3] >> 4) as f64 * 10.0;
    if !(-30.0..=70.0).contains(&t) || m > 100.0 {
        return None;
    }
    let rd = Reading::new(
        "Springfield-Soil",
        "weather",
        b[0].to_string(),
        Check::Checksum,
    )
    .with_channel(((b[1] >> 4) & 3) as i32 + 1)
    .with_battery(b[1] & 0x80 == 0);
    Some(temp_c(rd, t, 1).num(&measure::MOISTURE_PCT, m, 0))
}

// 25
fn schrader(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.len(0) != 68 {
        return None;
    }
    let b: [u8; 8] = ext(rows, 0, 4, 64);
    if CRC8_SCHRADER.checksum(&b[0..7]) != b[7] {
        return None;
    }
    let id = (((b[1] & 0xF) as u32) << 24) | (b[2] as u32) << 16 | (b[3] as u32) << 8 | b[4] as u32;
    let rd = Reading::new("Schrader", "tpms", format!("{id:07X}"), Check::Crc);
    Some(
        temp_c(kpa(rd, b[5] as f64 * 2.5, 1), b[6] as f64 - 50.0, 0).with(
            &measure::FLAGS,
            i64::from(((b[0] & 0xF) << 4) | (b[1] >> 4)),
        ),
    )
}

// 26
fn schrader_eg53(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.len(0) != 120 {
        return None;
    }
    let b: [u8; 10] = ext(rows, 0, 40, 80);
    if [1, 2, 4, 5, 7, 8].iter().all(|&i| b[i] == 0) || sum8(&b[0..9]) != b[9] {
        return None;
    }
    let id = (b[4] as u32) << 16 | (b[5] as u32) << 8 | b[6] as u32;
    let rd = Reading::new(
        "Schrader-EG53MA4",
        "tpms",
        format!("{id:06X}"),
        Check::Checksum,
    );
    Some(temp_f(kpa(rd, b[7] as f64 * 2.5, 1), b[8] as f64, 0))
}

// 27
fn ev1527(rows: &BitRows, ctx: &mut Ctx) -> Option<Reading> {
    if rows.len(0) != 25 {
        return None;
    }
    let row = rows.row(0);
    let b = [!row.byte(0), !row.byte(1), !row.byte(2), row.byte(3)];
    if b[3] & 0x80 == 0 || (b[0] == 0 && b[1] == 0) || b[2] == 0 {
        return None;
    }
    let code = (b[0] as u64) << 16 | (b[1] as u64) << 8 | b[2] as u64;
    // A code counts from its second copy in one train, and each remote is reported once per train.
    if ctx.first_copy(code) {
        return None;
    }
    let id = (b[0] as u32) << 8 | b[1] as u32;
    if !ctx.first_copy((1 << 40) | id as u64) {
        return None;
    }
    let tri: String = (0..12)
        .map(|i| match (code >> (22 - 2 * i)) & 3 {
            0 => '0',
            1 => 'Z',
            2 => 'X',
            _ => '1',
        })
        .collect();
    let rd = Reading::new("Generic-Remote", "remote", id.to_string(), Check::Repeat)
        .with(&measure::CODE, &format!("{code:06x}"));
    Some(
        rd.with(&measure::COMMAND, &(b[2]).to_string())
            .with(&measure::TRI_STATE, &tri),
    )
}

// 28
fn bresser5(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() != 1 || !(248..=440).contains(&rows.len(0)) {
        return None;
    }
    let p = rows.search(0, 0, &[0xAA, 0xAA, 0xAA, 0x2D, 0xD4], 40)?;
    if p + 40 + 26 * 8 > rows.len(0) {
        return None;
    }
    let m: [u8; 26] = ext(rows, 0, p + 40, 26 * 8);
    if (0..13).any(|i| m[i] ^ m[i + 13] != 0xFF)
        || m[13] as u32 != m[14..26].iter().map(|b| b.count_ones()).sum::<u32>()
    {
        return None;
    }
    let ty = m[15] & 0x7F;
    let rain = (bcd(m[23]) + bcd(m[24]) * 100) as f64 * 0.1;
    let mut t = None;
    if m[20] & 0xF <= 9 {
        let v = (bcd(m[20]) + (m[21] & 0xF) as u32 * 100) as f64 * 0.1;
        t = Some(if m[25] & 0xF != 0 { -v } else { v });
    }
    let model = if (0x39..=0x3B).contains(&ty) {
        "Bresser-ProRainGauge"
    } else {
        "Bresser-5in1"
    };
    let mut rd = Reading::new(model, "weather", m[14].to_string(), Check::Checksum)
        .with_battery(m[25] & 0x80 == 0);
    if let Some(t) = t {
        rd = temp_c(rd, t, 1);
    }
    if model == "Bresser-ProRainGauge" {
        return Some(rain_mm(rd, rain * 25.0));
    }
    if m[22] & 0xF <= 9 {
        rd = hum(rd, bcd(m[22]) as f64);
    }
    let gust = ((((m[17] & 0xF) as u32) << 8) + m[16] as u32) as f64 * 0.1;
    let speed = (bcd(m[18]) + (m[19] & 0xF) as u32 * 100) as f64 * 0.1;
    rd = wind_dir(
        wind_m_s(gust_m_s(rd, gust), speed),
        (m[17] >> 4) as f64 * 22.5,
        1,
    );
    Some(rain_mm(rd, rain))
}

// 29
fn bresser6(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() != 1 || !(160..=440).contains(&rows.len(0)) {
        return None;
    }
    let p = rows.search(0, 0, &[0xAA, 0xAA, 0x2D, 0xD4], 32)?;
    if p + 32 + 18 * 8 > rows.len(0) {
        return None;
    }
    let m: [u8; 18] = ext(rows, 0, p + 32, 18 * 8);
    if lfsr_digest16(&m[2..17], 0x8810, 0x5412) != (m[0] as u16) << 8 | m[1] as u16
        || sum8(&m[2..18]) != 0xFF
    {
        return None;
    }
    let id = format!("{:02x}{:02x}{:02x}{:02x}", m[2], m[3], m[4], m[5]);
    let ty = m[6] >> 4;
    let mut rd =
        Reading::new("Bresser-6in1", "weather", id, Check::Crc).with_channel((m[6] & 7) as i32);
    let rain_present = m[16] & 1 != 0;
    if !rain_present {
        rd = rd.with_battery(m[13] & 0x02 != 0);
    }
    if m[12] <= 0x99 && m[13] >> 4 <= 9 && !rain_present {
        let raw = bcd(m[12]) as f64 * 10.0 + (m[13] >> 4) as f64;
        let mut v = if m[13] & 0x08 != 0 {
            (raw - 1000.0) * 0.1
        } else {
            raw * 0.1
        };
        if v < -50.0 {
            v = -raw * 0.1;
        }
        rd = temp_c(rd, v, 1);
    }
    if !rain_present && bcd_ok(m[14]) {
        let h = bcd(m[14]);
        if ty == 4 {
            const MT: [u8; 16] = [0, 7, 13, 20, 27, 33, 40, 47, 53, 60, 67, 73, 80, 87, 93, 99];
            if (1..=16).contains(&h) {
                rd = rd.num(&measure::MOISTURE_PCT, MT[(h - 1) as usize] as f64, 0);
            }
        } else {
            rd = hum(rd, h as f64);
        }
    }
    if ty != 2 && ty != 4 {
        let (w7, w8, w9) = (!m[7], !m[8], !m[9]);
        if w7 <= 0x99 && w8 <= 0x99 && w9 <= 0x99 && bcd_ok(w7) && bcd_ok(w9) {
            let gust = (bcd(w7) * 10 + (w8 >> 4) as u32) as f64 * 0.1;
            let avg = (bcd(w9) * 10 + (w8 & 0xF) as u32) as f64 * 0.1;
            rd = wind_m_s(gust_m_s(rd, gust), avg);
            if bcd_ok(m[10]) {
                rd = wind_dir(rd, (bcd(m[10]) * 10 + (m[11] >> 4) as u32) as f64, 0);
            }
        }
        let (u15, u16_) = (!m[15], !m[16]);
        if m[16] & 0xF == 0 && u15 <= 0x99 && bcd_ok(u15) && (u16_ >> 4) <= 9 && !rain_present {
            rd = uv(rd, (bcd(u15) * 10 + (u16_ >> 4) as u32) as f64 * 0.1, 1);
        }
    }
    if rain_present {
        let (a, b, c) = (!m[12], !m[13], !m[14]);
        if bcd_ok(a) && bcd_ok(b) && bcd_ok(c) {
            rd = rain_mm(rd, (bcd(a) * 10000 + bcd(b) * 100 + bcd(c)) as f64 * 0.1);
        }
    }
    Some(rd.with(&measure::SENSOR_TYPE, (ty) as i64))
}

// 30
fn bresser7(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() != 1 || rows.len(0) < 160 {
        return None;
    }
    let start = rows.search(0, 0, &[0xAA, 0xAA, 0xAA, 0x2D, 0xD4], 40)? + 40;
    if start + 21 * 8 > rows.len(0) {
        return None;
    }
    let raw: [u8; 25] = ext(rows, 0, start, 25 * 8);
    if raw[21] == 0 {
        return None;
    }
    let ty = raw[6] >> 4;
    let ch = (raw[6] & 7) as i32;
    let m = raw.map(|x| x ^ 0xAA);
    if ((m[0] as u16) << 8 | m[1] as u16) ^ lfsr_digest16(&m[2..25], 0x8810, 0xBA95) != 0x6DF1 {
        return None;
    }
    let id = ((m[2] as u32) << 8 | m[3] as u32).to_string();
    let batt = m[15] & 6 != 6;
    match ty {
        1 | 12 | 13 => {
            let mut rd = Reading::new("Bresser-7in1", "weather", id, Check::Crc)
                .with_channel(ch)
                .with_battery(batt);
            let traw = bcd(m[14]) as f64 * 10.0 + (m[15] >> 4) as f64;
            let t = if traw > 600.0 {
                (traw - 1000.0) * 0.1
            } else {
                traw * 0.1
            };
            rd = hum(temp_c(rd, t, 1), bcd(m[16]) as f64);
            if ty != 12 {
                rd = wind_dir(rd, (bcd(m[4]) * 10 + (m[5] >> 4) as u32) as f64, 0);
                rd = gust_m_s(rd, (bcd(m[7]) * 10 + (m[8] >> 4) as u32) as f64 * 0.1);
                rd = wind_m_s(rd, ((m[8] & 0xF) as u32 * 100 + bcd(m[9])) as f64 * 0.1);
                rd = rd.num(&measure::LIGHT_LUX, bcd6(m[17], m[18], m[19]) as f64, 0);
                rd = uv(rd, (bcd(m[20]) * 10 + (m[21] >> 4) as u32) as f64 * 0.1, 1);
            }
            rd = rain_mm(rd, bcd6(m[10], m[11], m[12]) as f64 * 0.1);
            if ty == 13 && m[23] >> 4 < 10 {
                rd = rd.num(
                    &measure::GLOBE_TEMPERATURE_C,
                    bcd(m[22]) as f64 + (m[23] >> 4) as f64 * 0.1,
                    1,
                );
            }
            Some(rd)
        }
        8 => {
            let mut rd = Reading::new("Bresser-7in1", "sensor", id, Check::Crc)
                .with_channel(ch)
                .with_battery(batt);
            if m[10] & 0xF != 0xF {
                let v = (m[10] & 0xF) as u32 * 1000 + bcd(m[11]) * 10 + (m[12] >> 4) as u32;
                rd = rd.num(&measure::PM2_5_UG_M3, v as f64, 0);
            }
            if m[12] & 0xF != 0xF {
                let v = (m[12] & 0xF) as u32 * 1000 + bcd(m[13]) * 10 + (m[14] >> 4) as u32;
                rd = rd.num(&measure::PM10_UG_M3, v as f64, 0);
            }
            Some(rd)
        }
        10 | 11 => {
            let model = if ty == 10 {
                "Bresser-CO2"
            } else {
                "Bresser-HCHOVOC"
            };
            let mut rd = Reading::new(model, "sensor", id, Check::Crc)
                .with_channel(ch)
                .with_battery(batt);
            if m[5] & 0xF != 0xF {
                let v = (bcd(m[4]) * 100 + bcd(m[5])) as f64;
                rd = if ty == 10 {
                    rd.num(&measure::CO2_PPM, v, 0)
                } else {
                    rd.num(&measure::HCHO_PPB, v, 0)
                };
            }
            if ty == 11 && m[22] & 0xF != 0xF {
                rd = rd.with(&measure::VOC_LEVEL, (m[22] & 0xF) as i64);
            }
            Some(rd)
        }
        _ => None,
    }
}

fn lacrosse_it(rows: &BitRows, model: &'static str) -> Option<Reading> {
    (0..rows.num_rows()).find_map(|r| {
        let p = rows.search(r, 0, &[0xA2, 0xDD, 0x49], 24)?;
        if p + 20 + 40 > rows.len(r) {
            return None;
        }
        let b: [u8; 5] = ext(rows, r, p + 20, 40);
        if CRC8_31.checksum(&b[0..4]) != b[4] {
            return None;
        }
        let mut id = (((b[0] & 0xF) as u32) << 2) | (b[1] >> 6) as u32;
        let t = 10.0 * (b[1] & 0xF) as f64 + (b[2] >> 4) as f64 + 0.1 * (b[2] & 0xF) as f64 - 40.0;
        let h = b[3] & 0x7F;
        if h == 0x7D {
            id += 64;
        }
        let mut rd = temp_c(
            Reading::new(model, "weather", id.to_string(), Check::Crc)
                .with_battery(b[3] & 0x80 == 0),
            t,
            1,
        );
        if h != 0x6A && h != 0x7D {
            rd = hum(rd, h as f64);
        }
        if b[1] & 0x20 != 0 {
            rd = rd.with(&measure::NEW_BATTERY, true);
        }
        Some(rd)
    })
}

// 31, 32
fn tx29(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    lacrosse_it(rows, "LaCrosse-TX29IT")
}
fn tx35(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    lacrosse_it(rows, "LaCrosse-TX35DTHIT")
}

// 33
fn wh24(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let len = rows.len(0);
    if !(190..=268).contains(&len) {
        return None;
    }
    let start = rows.search(0, 0, &[0xAA, 0x2D, 0xD4], 24)? + 24;
    if start + 17 * 8 > len {
        return None;
    }
    let model = if len > 215 {
        "Fineoffset-WS69"
    } else if len - (start + 136) < 8 && start < 61 {
        "Fineoffset-WH24"
    } else {
        "Fineoffset-WH65B"
    };
    let b: [u8; 17] = ext(rows, 0, start, 17 * 8);
    if b[0] != 0x24 || CRC8_31.checksum(&b[0..16]) != 0 || sum8(&b[0..16]) != b[16] {
        return None;
    }
    let wh24 = model == "Fineoffset-WH24";
    let factor = if wh24 { 1.12 } else { 0.51 };
    let mut rd =
        Reading::new(model, "weather", b[1].to_string(), Check::Crc).with_battery(b[3] & 0x08 == 0);
    let tr = ((b[3] as u32 & 7) << 8) | b[4] as u32;
    if tr != 0x7FF {
        rd = temp_c(rd, (tr as f64 - 400.0) * 0.1, 1);
    }
    if b[5] != 0xFF {
        rd = hum(rd, b[5] as f64);
    }
    let dir = b[2] as u32 | ((b[3] as u32 >> 7) << 8);
    if dir != 0x1FF {
        rd = wind_dir(rd, dir as f64, 0);
    }
    let ws = b[6] as u32 | (((b[3] as u32 >> 4) & 1) << 8);
    if ws != 0x1FF {
        rd = wind_m_s(rd, ws as f64 * 0.125 * factor);
    }
    if b[7] != 0xFF {
        rd = gust_m_s(rd, b[7] as f64 * factor);
    }
    rd = rain_mm(
        rd,
        ((b[8] as u32) << 8 | b[9] as u32) as f64 * if wh24 { 0.3 } else { 0.254 },
    );
    let u = (b[10] as u32) << 8 | b[11] as u32;
    if u != 0xFFFF {
        const TH: [u32; 13] = [
            432, 851, 1210, 1570, 2017, 2450, 2761, 3100, 3512, 3918, 4277, 4650, 5029,
        ];
        rd = uv(rd, TH.iter().filter(|&&t| t < u).count() as f64, 0);
    }
    let light = (b[12] as u32) << 16 | (b[13] as u32) << 8 | b[14] as u32;
    if light != 0xFFFFFF {
        rd = rd.num(&measure::LIGHT_LUX, light as f64 * 0.1, 1);
    }
    Some(rd)
}

// 34
fn toyota(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let n = rows.len(0);
    let mut from = 0;
    while from + 12 <= n {
        let Some(p) = rows.search(0, from, &[0xA9, 0xE0], 12) else {
            break;
        };
        from = p + 1;
        // The data is FM0 (biphase space), not differential Manchester.
        let b = fm0(rows, 0, p + 11, 80, |d| {
            (d.len(0) >= 72).then(|| bytes64(d, 0))
        });
        let Some(b) = b else { continue };
        if CRC8_TOYOTA.checksum(&b[0..8]) != b[8] {
            continue;
        }
        let p1 = ((b[4] as u32 & 0x7F) << 1) | (b[5] >> 7) as u32;
        let tt = ((b[5] as u32 & 0x7F) << 1) | (b[6] >> 7) as u32;
        if p1 != (!b[7]) as u32 {
            continue;
        }
        let rd = Reading::new("Toyota", "tpms", hex_id(&b[0..4]), Check::Crc);
        return Some(
            temp_c(psi(rd, p1 as f64 * 0.25 - 7.0), tt as f64 - 40.0, 0)
                .with(&measure::STATUS, ((b[4] & 0x80) | (b[6] & 0x7F)) as i64),
        );
    }
    None
}

// 35
fn pmv107j(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let n = rows.len(0);
    let mut from = 0;
    while from + 6 <= n {
        let Some(p) = rows.search(0, from, &[0xF8], 6) else {
            break;
        };
        from = p + 1;
        let b = fm0(rows, 0, p + 6, 70, |d| {
            if d.len(0) < 67 {
                return None;
            }
            let mut b = [0u8; 9];
            b[0] = (d.row(0).byte(0) >> 6) & 3;
            let rest: [u8; 8] = ext(d, 0, 2, 64);
            b[1..].copy_from_slice(&rest);
            Some(b)
        });
        let Some(b) = b else { continue };
        if CRC8_PMV107J.checksum(&b[0..8]) != b[8] || b[5] != !b[6] {
            continue;
        }
        let id = ((b[0] as u32) << 26)
            | (b[1] as u32) << 18
            | (b[2] as u32) << 10
            | (b[3] as u32) << 2
            | (b[4] >> 6) as u32;
        let rd = Reading::new("PMV-107J", "tpms", format!("{id:08x}"), Check::Crc)
            .with_battery(b[4] & 0x20 == 0);
        return Some(temp_c(
            kpa(rd, (b[5] as f64 - 40.0) * 2.48, 1),
            b[7] as f64 - 40.0,
            0,
        ));
    }
    None
}

/// The VDO TPMS frames: the device sends inverted bits, so the preamble is searched inverted and the Manchester pairs are read
/// by the Thomas convention (the IEEE pairs of the inverted bits). `step` moves the search on when a check fails.
#[allow(clippy::too_many_arguments)]
fn vdo<const N: usize>(
    rows: &BitRows,
    pre: [u8; N],
    pre_bits: usize,
    all_rows: bool,
    raw_needed: usize,
    step: usize,
    max: usize,
    min: usize,
    f: impl Fn(&[u8; 64]) -> Option<Reading>,
) -> Option<Reading> {
    let pre = inv(pre);
    let nrows = if all_rows { rows.num_rows() } else { 1 };
    for r in 0..nrows {
        let mut from = 0;
        while let Some(p) = rows.search(r, from, &pre, pre_bits) {
            if rows.len(r) - p < raw_needed {
                break;
            }
            let found = manchester(rows, r, p + pre_bits, max, ManchesterConv::Thomas, |d| {
                (d.len(0) >= min).then(|| f(&bytes64(d, 0))).flatten()
            });
            if found.is_some() {
                return found;
            }
            from = p + step;
        }
    }
    None
}

// 36
fn citroen(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    vdo(rows, [0xAA, 0xA9], 16, false, 178, 2, 88, 80, |b| {
        if b[6] == 0 || b[7] == 0 || xor8(&b[1..10]) != 0 {
            return None;
        }
        let rd = Reading::new("Citroen", "tpms", hex_id(&b[1..5]), Check::Checksum);
        let rd = temp_c(kpa(rd, b[6] as f64 * 1.364, 0), b[7] as f64 - 50.0, 0);
        Some(
            rd.with(&measure::REPEAT, (b[5] & 0xF) as i64)
                .with(&measure::FLAGS, (b[5] >> 4) as i64),
        )
    })
}

// 37
fn renault(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    vdo(rows, [0xAA, 0xA9], 16, true, 160, 15, 160, 72, |b| {
        if CRC8_SMBUS.checksum(&b[0..8]) != b[8] {
            return None;
        }
        let id = (b[5] as u32) << 16 | (b[4] as u32) << 8 | b[3] as u32;
        let rd = Reading::new("Renault", "tpms", format!("{id:06x}"), Check::Crc);
        let rd = kpa(
            rd,
            ((((b[0] & 3) as u32) << 8) | b[1] as u32) as f64 * 0.75,
            2,
        );
        Some(temp_c(rd, b[2] as f64 - 30.0, 0).with(&measure::FLAGS, i64::from(b[0] >> 2)))
    })
}

// 38
fn ford(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    vdo(rows, [0xAA, 0xA9], 16, true, 144, 15, 160, 64, |b| {
        if sum8(&b[0..7]) != b[7] {
            return None;
        }
        let fl = b[6] & 0x4C;
        if !matches!(fl, 0x08 | 0x04 | 0x44) || b[6] & 0x90 != 0 {
            return None;
        }
        let mut rd = psi(
            Reading::new("Ford", "tpms", hex_id(&b[0..4]), Check::Checksum),
            ((((b[6] & 0x20) as u32) << 3) | b[4] as u32) as f64 * 0.25,
        );
        if b[5] & 0x80 == 0 {
            rd = temp_c(rd, (b[5] & 0x7F) as f64 - 56.0, 0);
        }
        Some(
            rd.with(&measure::MOVING, fl == 0x44)
                .with(&measure::LEARNING, fl == 0x08),
        )
    })
}

// 39
fn hyundai(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    // The preamble (55 55 55 56 on air) starts with a space chip before the first mark, which the slicer does not see:
    // the search takes the 31 chips after it.
    vdo(
        rows,
        [0x55, 0x55, 0x55, 0x53],
        31,
        false,
        80,
        2,
        80,
        80,
        |b| {
            if CRC8_HYUNDAI.checksum(&b[0..9]) != b[9] {
                return None;
            }
            let rd = Reading::new("Hyundai-VDO", "tpms", hex_id(&b[1..5]), Check::Crc);
            let rd = temp_c(kpa(rd, b[6] as f64 * 1.375, 1), b[7] as f64 - 50.0, 0);
            Some(
                rd.with(&measure::REPEAT, (b[5] & 0xF) as i64)
                    .with(&measure::STATE, &(b[0]).to_string()),
            )
        },
    )
}

// 40
fn abarth(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if !(151..=209).contains(&rows.len(0)) {
        return None;
    }
    vdo(rows, [0xAA, 0xAA, 0xA9], 24, false, 80, 2, 72, 72, |b| {
        let t = b[6] as f64 - 50.0;
        if xor8(&b[0..9]) != 0 || !(-50.0..=125.0).contains(&t) {
            return None;
        }
        let rd = Reading::new(
            "Abarth-124Spider",
            "tpms",
            hex_id(&b[0..4]),
            Check::Checksum,
        );
        Some(temp_c(kpa(rd, b[5] as f64 * 1.38, 1), t, 0).with(&measure::STATUS, (b[7]) as i64))
    })
}

// 41
fn efergy(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    if rows.num_rows() != 1 || !(64..=65).contains(&rows.len(0)) {
        return None;
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes64(rows, 0)[..8]);
    match b[0] >> 4 {
        0 => {}
        0xF => b.iter_mut().for_each(|x| *x = !*x),
        _ => return None,
    }
    if b.iter().filter(|&&x| x == 0).count() > 5 {
        return None;
    }
    let s = sum8(&b[0..7]);
    if s == 0 || s != b[7] {
        return None;
    }
    let fact = 15 - b[6] as i8 as i32;
    if !(7..=23).contains(&fact) {
        return None;
    }
    let id = (b[2] as u32) << 8 | b[1] as u32;
    let cur = ((b[4] as u32) << 8 | b[5] as u32) as f64 / (1u64 << fact) as f64;
    Some(
        Reading::new("Efergy-e2CT", "energy", id.to_string(), Check::Checksum)
            .with_battery(b[3] & 0x40 != 0)
            .num(&measure::CURRENT_A, cur, 2)
            .with(&measure::INTERVAL, (((b[3] >> 4) & 3) as f64 + 1.0) * 6.0)
            .with(&measure::LEARNING, b[3] >> 7 != 0),
    )
}

// 42
fn currentcost(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    // The device sends inverted bits: search the inverted patterns and read Manchester by the Thomas convention.
    let envir = inv([0x55, 0x55, 0x55, 0x55, 0xA4, 0x57]);
    let tx = inv([0xCC, 0xCC, 0xCC, 0xCE, 0x91, 0x58]);
    for r in 0..rows.num_rows() {
        let n = rows.len(r);
        let (start, is_envir) = match rows.search(r, 0, &envir, 48) {
            Some(p) => (p + 47, true),
            None => match rows.search(r, 0, &tx, 45) {
                Some(p) => (p + 45, false),
                None => continue,
            },
        };
        if n < start + 112 {
            continue;
        }
        let b = manchester(rows, r, start, 4096, ManchesterConv::Thomas, |d| {
            (d.len(0) >= 64).then(|| bytes64(d, 0))
        });
        let Some(b) = b else { continue };
        let dev = ((b[0] as u32 & 0xF) << 8) | b[1] as u32;
        match b[0] >> 4 {
            0 => {
                let pw = |i: usize| {
                    if b[i] & 0x80 != 0 {
                        (((b[i] & 0x7F) as u32) << 8 | b[i + 1] as u32) as f64
                    } else {
                        0.0
                    }
                };
                let model = if is_envir {
                    "CurrentCost-EnviR"
                } else {
                    "CurrentCost-TX"
                };
                return Some(
                    Reading::new(model, "energy", dev.to_string(), Check::Manchester)
                        .num(&measure::POWER_0_W, pw(2), 0)
                        .num(&measure::POWER_1_W, pw(4), 0)
                        .num(&measure::POWER_2_W, pw(6), 0),
                );
            }
            4 => {
                let model = if is_envir {
                    "CurrentCost-EnviRCounter"
                } else {
                    "CurrentCost-Counter"
                };
                let rd = Reading::new(model, "energy", dev.to_string(), Check::Manchester);
                let rd = rd.with(&measure::SENSOR_TYPE, (b[3]) as i64);
                return Some(rd.with(
                    &measure::COUNTER,
                    (u32::from_be_bytes([b[4], b[5], b[6], b[7]])) as i64,
                ));
            }
            _ => {}
        }
    }
    None
}

/// Searches a Z-Wave start of frame in `c` (row 0) and returns the first MPDU that passes its check.
fn zwave_frame(c: &BitRows, rate: u8) -> Option<Reading> {
    let n = c.len(0);
    let mut from = 0;
    let mut bytes = Vec::new();
    while let Some(p) = c.search(0, from, &[0x55, 0xF0], 16) {
        from = p + 1;
        let avail = (n - p - 16) / 8;
        if avail < 10 {
            break;
        }
        bytes.clear();
        bytes.resize(avail, 0);
        c.extract(0, p + 16, avail * 8, &mut bytes).ok()?;
        let l = bytes[7] as usize;
        let (lo, hi) = if rate == 3 { (11, 170) } else { (10, 64) };
        if l < lo || l > hi || l > bytes.len() {
            continue;
        }
        let mpdu = &bytes[..l];
        let ok = if rate == 3 {
            CRC16_SPI_FUJITSU.checksum(&mpdu[..l - 2])
                == (mpdu[l - 2] as u16) << 8 | mpdu[l - 1] as u16
        } else {
            mpdu[l - 1] == 0xFF ^ xor8(&mpdu[..l - 1])
        };
        if ok {
            if let Some(rd) = crate::zwave::frame::ism_reading(mpdu, rate) {
                return Some(rd);
            }
        }
    }
    None
}

fn zwave_rows(rows: &BitRows, rate: u8) -> Option<Reading> {
    for r in 0..rows.num_rows() {
        for inverted in [false, true] {
            let row = rows.row(r);
            let mut one = BitRows::from_bytes(row.bytes, row.len).ok()?;
            if inverted {
                one.invert();
            }
            if rate == 1 {
                let mut dec = BitRows::new(1, 4096);
                for start in 0..2 {
                    one.decode_manchester(0, start, 4096, ManchesterConv::Ieee, &mut dec);
                    if let Some(rd) = zwave_frame(&dec, rate) {
                        return Some(rd);
                    }
                }
            } else if let Some(rd) = zwave_frame(&one, rate) {
                return Some(rd);
            }
        }
    }
    None
}

// 43, 44, 45
fn zwave_r1(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    zwave_rows(rows, 1)
}
fn zwave_r2(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    zwave_rows(rows, 2)
}
fn zwave_r3(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    zwave_rows(rows, 3)
}

// 46
fn wmbus_header(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let n = rows.len(0);
    if !(136..=3136).contains(&n) {
        return None;
    }
    let after = rows.search(0, 0, &[0x54, 0x3D], 16)? + 16;
    let mut bytes = Vec::new();
    let mode = if rows.bits_u64(0, after, 8) == 0x54 {
        let mode = match rows.bits_u64(0, after + 8, 8) {
            0xCD => "C",
            0x3D => "C (format B)",
            _ => return None,
        };
        let start = after + 16;
        let nb = (n - start) / 8;
        bytes.resize(nb, 0);
        rows.extract(0, start, nb * 8, &mut bytes).ok()?;
        mode
    } else {
        let mut i = after;
        while i + 12 <= n {
            let hi = three_of_six::decode(rows.bits_u64(0, i, 6) as u8);
            let lo = three_of_six::decode(rows.bits_u64(0, i + 6, 6) as u8);
            match (hi, lo) {
                (Some(h), Some(l)) => bytes.push(h << 4 | l),
                _ => break,
            }
            i += 12;
        }
        "T"
    };
    if bytes.len() < 12 {
        return None;
    }
    let ok = if mode == "C (format B)" {
        let l = bytes[0] as usize;
        if l < 12 || l + 1 > bytes.len() {
            return None;
        }
        let e = (l - 2).min(125);
        CRC16_EN_13757.checksum(&bytes[..=e]) == (bytes[e + 1] as u16) << 8 | bytes[e + 2] as u16
    } else {
        CRC16_EN_13757.checksum(&bytes[..10]) == (bytes[10] as u16) << 8 | bytes[11] as u16
    };
    if !ok || (bytes[2] == 0xFF && bytes[3] == 0x03) {
        return None;
    }
    let mfr = crate::wmbus::link::manufacturer((bytes[3] as u16) << 8 | bytes[2] as u16);
    let id = format!(
        "{:02X}{:02X}{:02X}{:02X}",
        bytes[7], bytes[6], bytes[5], bytes[4]
    );
    let ty = crate::wmbus::link::device_type_name(bytes[9])
        .map(String::from)
        .unwrap_or_else(|| format!("{:02X}", bytes[9]));
    let rd = Reading::new("Wireless-MBus", "meter", id, Check::Crc)
        .with(&measure::MODE, mode)
        .with(&measure::MANUFACTURER, &mfr)
        .with(&measure::TYPE, &ty);
    Some(
        rd.with(&measure::VERSION, (bytes[8]) as i64)
            .with(&measure::C_FIELD, &format!("{:02X}", bytes[1]))
            .with(&measure::LENGTH_BYTES, i64::from(bytes[0])),
    )
}

// 47. Somfy RTS, from a public prose description of the protocol: 433.42 MHz OOK; hardware sync 2416 µs on and off, a software
// sync of 4550 µs on and 604 µs off, then 56 bits Manchester with 604 µs half-bits (a rising edge is 1); each byte after the
// first is sent XOR the byte sent before it; the XOR of the 14 nibbles is 0. Byte 1 high nibble is the button, bytes 2 and 3
// the rolling code, bytes 4 to 6 the address (low byte first). Only the address and the button are shown: the rolling code is
// read for the check and dropped, so nothing here can make or predict a frame.
fn somfy_rts(rows: &BitRows, _: &mut Ctx) -> Option<Reading> {
    let n = rows.len(0);
    // The software sync: at least 7 one-chips after at least 3 zero-chips, then one zero-chip.
    let mut i = 0;
    let start = loop {
        while i < n && rows.bit(0, i) == 1 {
            i += 1;
        }
        let z = i;
        while i < n && rows.bit(0, i) == 0 {
            i += 1;
        }
        let o = i;
        while i < n && rows.bit(0, i) == 1 {
            i += 1;
        }
        if i >= n {
            return None;
        }
        if o - z >= 3 && i - o >= 7 {
            break i + 1;
        }
    };
    let mut f = [0u8; 7];
    let got = manchester(rows, 0, start, 56, ManchesterConv::Ieee, |d| {
        let k = d.len(0);
        if k >= 55 {
            d.row(0).copy_padded(&mut f);
        }
        k
    });
    // The last bit 0 ends on a high half-chip that the silence after the frame takes in.
    if got == 55 && n == start + 111 && rows.bit(0, n - 1) == 1 {
    } else if got != 56 {
        return None;
    }
    let mut d = f;
    for k in 1..7 {
        d[k] = f[k] ^ f[k - 1];
    }
    if d.iter().fold(0u8, |a, &b| a ^ (b >> 4) ^ (b & 15)) != 0 {
        return None;
    }
    let button = match d[1] >> 4 {
        0x1 => "My",
        0x2 => "Up",
        0x3 => "My + Up",
        0x4 => "Down",
        0x5 => "My + Down",
        0x6 => "Up + Down",
        0x8 => "Prog",
        0x9 => "Sun + Flag",
        0xA => "Flag",
        _ => return None,
    };
    let address = u32::from(d[6]) << 16 | u32::from(d[5]) << 8 | u32::from(d[4]);
    let rd = Reading::new("Somfy-RTS", "remote", format!("{address:06X}"), Check::Checksum)
        .with(&measure::BUTTON, button);
    Some(rd.with(&measure::TYPE, "rolling code"))
}

// 48. A KeeLoq-style code word, from the Microchip HCS301 data sheet (DS21143): a preamble of 12 pulses of TE (400 µs) with TE
// gaps, a header of 10 TE, then 66 bits in PWM (a 1 is TE on and 2 TE off, a 0 is 2 TE on and TE off), least significant bit
// first: 32 bits of encrypted hopping code, the 28-bit serial number, the buttons S3 S0 S1 S2, VLOW and RPT. Only the serial
// number, the buttons and the battery are shown; the hopping code is not read. A word counts from its second copy.
fn keeloq(rows: &BitRows, ctx: &mut Ctx) -> Option<Reading> {
    let r = (1..rows.num_rows()).find(|&r| {
        rows.len(r) == 66
            && (10..=12).contains(&rows.len(r - 1))
            && (0..rows.len(r - 1)).all(|i| rows.bit(r - 1, i) == 1)
    })?;
    let lsb = |pos: usize, n: usize| (0..n).fold(0u32, |v, i| v | u32::from(rows.bit(r, pos + i)) << i);
    let serial = lsb(32, 28);
    let (s3, s0, s1, s2) = (lsb(60, 1), lsb(61, 1), lsb(62, 1), lsb(63, 1));
    let (vlow, rpt) = (lsb(64, 1), lsb(65, 1));
    if serial == 0 || serial == 0x0FFF_FFFF || s0 + s1 + s2 + s3 == 0 {
        return None;
    }
    // No check covers the word: it counts from its second copy in one train (a held button repeats it), once per train.
    let fixed = u64::from(serial) << 4 | u64::from(s3 << 3 | s2 << 2 | s1 << 1 | s0);
    if ctx.first_copy(fixed) || !ctx.first_copy((1 << 40) | fixed) {
        return None;
    }
    let mut buttons = String::new();
    for (on, name) in [(s0, "S0"), (s1, "S1"), (s2, "S2"), (s3, "S3")] {
        if on == 1 {
            if !buttons.is_empty() {
                buttons.push_str(" + ");
            }
            buttons.push_str(name);
        }
    }
    let rd = Reading::new("KeeLoq-HCS", "remote", format!("{serial:07X}"), Check::Repeat)
        .with_battery(vlow == 0)
        .with(&measure::BUTTON, &buttons)
        .with(&measure::TYPE, "rolling code");
    Some(rd.with(&measure::REPEAT, i64::from(rpt)))
}
