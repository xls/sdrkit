//! The frame encoders of the device families of `ism`: each builds the bytes of one frame from the raw values of its fields,
//! with the checks that its protocol in [`super::protocols`] tests, and the pulse train that the device sends (lengths at
//! [`TRAIN_RATE_HZ`]). The test signal ([`super::gen`]) and `send` ([`super::send`]) build their trains here.
//!
//! Each encoder states the timing that its devices send. A family with raw bytes takes the bytes of the frame as the protocol
//! reads them and adds the checks.

use super::checks::{
    CRC16_ERT_IDM, CRC16_ERT_SCM, CRC16_HONEYWELL_8050, CRC4_KEDSUM, CRC8_31, CRC8_HYUNDAI,
    CRC8_PMV107J, CRC8_RUBICSON, CRC8_SCHRADER, CRC8_TOYOTA,
};
use super::gen::TRAIN_RATE_HZ;
use super::protocols::{reflect_nibbles, ERT_HALF_US};
use sdrkit_proto_ookdev::{encode, Modulation, Pulse, Train};
use sdrkit_util_bits::checksum::{
    lfsr_digest16, lfsr_digest8, lfsr_digest8_reflect, nibble_sum, sum8, xor8,
};
use sdrkit_util_bits::crc::catalog::{CRC16_UMTS, CRC8_NRSC_5, CRC8_SMBUS};
use sdrkit_util_bits::line::{Manchester, ManchesterConv};
use sdrkit_util_bits::pack::{unpack, BitOrder};

/// The bits of `b`, most significant bit first.
pub(super) fn bits_of(b: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 * b.len());
    unpack(b, BitOrder::Msb, &mut v);
    v
}

/// An empty train of modulation `m`.
pub(super) fn train(m: Modulation) -> Train {
    Train::new(m, TRAIN_RATE_HZ)
}

/// Appends one pulse of `mark_us` and `space_us`.
pub(super) fn push(t: &mut Train, mark_us: f32, space_us: f32) {
    t.pulses
        .push(Pulse::new(t.samples(mark_us), t.samples(space_us)));
}

/// Sets the space after the last pulse to `space_us`.
pub(super) fn set_last_space(t: &mut Train, space_us: f32) {
    let s = t.samples(space_us);
    if let Some(p) = t.pulses.last_mut() {
        p.space = s;
    }
}

/// Manchester "1 = high then low" (the Thomas convention) with half-bit `half_us`, as OOK.
pub(super) fn manchester_ook(bits: &[u8], half_us: f32) -> Train {
    let mut t = train(Modulation::Ook);
    let mut chips = Vec::new();
    encode::manchester(bits, ManchesterConv::Thomas, half_us, &mut chips, &mut t);
    t
}

/// The Manchester chips of `bits` by `conv`.
fn chips(bits: &[u8], conv: ManchesterConv) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 * bits.len());
    Manchester::encode(bits, conv, &mut v);
    v
}

/// The IEEE 802.3 Manchester chips of `bits` (1 is 01), which some devices send as plain bits.
pub(super) fn ieee_chips(bits: &[u8]) -> Vec<u8> {
    chips(bits, ManchesterConv::Ieee)
}

/// The levels `levels` as FSK NRZ of `bit_us` per level and deviation `dev` Hz.
pub(super) fn fsk(levels: &[u8], bit_us: f32, dev: f64) -> Train {
    let mut t = train(Modulation::Fsk);
    t.deviation_hz = dev;
    encode::nrz(levels, bit_us, &mut t);
    t
}

/// The two BCD digits of `v` (0 to 99).
fn bcd(v: u32) -> u8 {
    ((((v / 10) % 10) << 4) | (v % 10)) as u8
}

/// Nexus: 12 copies of 36 bits in PPM (a 500 µs pulse, then 1000 µs for a 0 and 2000 µs for a 1), each followed by a 500 µs
/// pulse and 4000 µs of silence. `channel` is the field (0 to 3), `t` the temperature in tenths of °C (12 bits), `humidity`
/// 0 for none.
pub fn nexus(id: u8, channel: u8, battery_ok: bool, t: i16, humidity: u8) -> Train {
    let b = [
        id,
        u8::from(battery_ok) << 7 | (channel & 3) << 4 | ((t >> 8) as u8 & 0xF),
        t as u8,
        0xF0 | humidity >> 4,
        (humidity & 0xF) << 4,
    ];
    let bits = &bits_of(&b)[..36];
    let mut tr = train(Modulation::Ook);
    for _ in 0..12 {
        encode::ppm(bits, 500.0, 1000.0, 2000.0, &mut tr);
        push(&mut tr, 500.0, 4000.0);
    }
    tr
}

/// LaCrosse TX141TH-Bv2: 12 copies, each 4 sync pulses of 833 µs and the 40 bits in PWM (a 1 is 417 µs on, 208 µs off): the
/// id, the battery-low bit, the channel (0 to 3) and the 12-bit temperature `raw` (tenths of °C plus 500), the humidity and
/// the LFSR digest (generator 0x31, key 0xF4) of the 4 octets before it.
pub fn tx141th(id: u8, channel: u8, battery_ok: bool, raw: u16, humidity: u8) -> Train {
    let low = if battery_ok { 0 } else { 0x80 };
    let mut b = [
        id,
        low | (channel & 3) << 4 | ((raw >> 8) as u8 & 0xF),
        raw as u8,
        humidity,
        0,
    ];
    b[4] = lfsr_digest8_reflect(&b[0..4], 0x31, 0xF4);
    let bits = bits_of(&b);
    let mut tr = train(Modulation::Ook);
    for _ in 0..12 {
        for _ in 0..4 {
            push(&mut tr, 833.0, 833.0);
        }
        encode::pwm(&bits, (417.0, 208.0), (208.0, 417.0), &mut tr);
    }
    tr
}

/// The Acurite tower family (592TXR tower, 5-in-1 and the other message types): `msg` is the message without its checksum,
/// 5 to 9 octets. Bytes 2 to the last carry their even parity in their top bit, then the add checksum follows. 3 copies,
/// each 4 sync pulses (620 µs on, 596 µs off) and the bits in PWM (a 1 is 408 µs on, 204 µs off; a 0 is 220 µs on, 392 µs
/// off), 2192 µs of silence between copies.
pub fn acurite_txr(msg: &[u8]) -> Train {
    let par = |x: u8| {
        let x = x & 0x7F;
        if x.count_ones() % 2 == 1 {
            x | 0x80
        } else {
            x
        }
    };
    let mut b: Vec<u8> = msg
        .iter()
        .enumerate()
        .map(|(i, &x)| if i >= 2 { par(x) } else { x })
        .collect();
    b.push(sum8(&b));
    let bits = bits_of(&b);
    let mut tr = train(Modulation::Ook);
    for r in 0..3 {
        for _ in 0..4 {
            push(&mut tr, 620.0, 596.0);
        }
        encode::pwm(&bits, (408.0, 204.0), (220.0, 392.0), &mut tr);
        if r < 2 {
            set_last_space(&mut tr, 2192.0);
        }
    }
    tr
}

/// The Acurite 592TXR tower message (type 0x04): the 14-bit id, the channel code (b0 bits 7-6: 3 A, 2 B, 0 C), the
/// humidity (127 for none) and the temperature `t` (tenths of °C plus 1000, 14 bits).
pub fn acurite_tower(id: u16, channel_code: u8, battery_ok: bool, humidity: u8, t: u16) -> Train {
    acurite_txr(&[
        (channel_code & 3) << 6 | ((id >> 8) as u8 & 0x3F),
        id as u8,
        u8::from(battery_ok) << 6 | 0x04,
        humidity,
        (t >> 7) as u8 & 0x7F,
        t as u8 & 0x7F,
    ])
}

/// The raw fields of a Fine Offset WH1080 weather message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wh1080 {
    /// The id.
    pub id: u8,
    /// The battery is good.
    pub battery_ok: bool,
    /// The temperature: tenths of °C plus 400, 10 bits.
    pub t: u16,
    /// The humidity, %.
    pub humidity: u8,
    /// The wind speed: units of 0.34 m/s.
    pub speed: u8,
    /// The wind gust: units of 0.34 m/s.
    pub gust: u8,
    /// The rain: units of 0.3 mm, 12 bits.
    pub rain: u16,
    /// The wind direction: the index of the 16 directions from north.
    pub dir: u8,
}

/// Fine Offset WH1080: 88 bits in PWM (a 1 is a 544 µs pulse, a 0 a 1524 µs pulse, each followed by 1036 µs), the weather
/// message (type 0xA) with its CRC-8 (poly 0x31, register 0xFF).
pub fn wh1080(w: &Wh1080) -> Train {
    let mut b = [
        0xFFu8,
        0xA0 | w.id >> 4,
        (w.id & 0xF) << 4 | ((w.t >> 8) as u8 & 3),
        w.t as u8,
        w.humidity,
        w.speed,
        w.gust,
        (w.rain >> 8) as u8 & 0xF,
        w.rain as u8,
        u8::from(!w.battery_ok) << 4 | (w.dir & 0xF),
        0,
    ];
    b[10] = CRC8_NRSC_5.checksum(&b[0..10]);
    let mut tr = train(Modulation::Ook);
    encode::pwm(&bits_of(&b), (544.0, 1036.0), (1524.0, 1036.0), &mut tr);
    tr
}

/// Ambient Weather F007TH: after 15 zero bits and a 1, the 48 message bits, Manchester at 500 µs half-bits. `channel` is the
/// field (0 to 7), `t` the temperature in tenths of °F plus 400 (12 bits).
pub fn f007th(id: u8, channel: u8, battery_ok: bool, t: u16, humidity: u8) -> Train {
    let mut b = [
        0x45u8,
        id,
        u8::from(!battery_ok) << 7 | (channel & 7) << 4 | ((t >> 8) as u8 & 0xF),
        t as u8,
        humidity,
        0,
    ];
    b[5] = lfsr_digest8(&b[0..5], 0x98, 0x3E) ^ 0x64;
    let mut bits = vec![0u8; 15];
    bits.push(1);
    bits.extend(bits_of(&b));
    manchester_ook(&bits, 500.0)
}

/// The Oregon Scientific nibble checksum over the first 15 nibbles, into nibbles 15 and 16.
pub(super) fn oregon_checksum(m: &mut [u8]) {
    let c = nibble_sum(m, 15);
    m[7] = (m[7] & 0xF0) | (c & 0xF);
    m[8] = (c >> 4) << 4 | (m[8] & 0xF);
}

/// The fields of an Oregon Scientific temperature and humidity message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Oregon {
    /// The channel (0 to 15).
    pub channel: u8,
    /// The rolling id.
    pub id: u8,
    /// The battery is good.
    pub battery_ok: bool,
    /// The temperature, tenths of °C (at most 7999 in size).
    pub t: i16,
    /// The humidity, % (0 to 99).
    pub humidity: u8,
}

/// The 9 message octets of an Oregon Scientific sensor `sensor` (in reading order) with its nibble checksum.
fn oregon_msg(sensor: u16, o: &Oregon) -> [u8; 9] {
    let a = o.t.unsigned_abs();
    let (hundreds, tens, units, tenths) = (a / 1000, a / 100 % 10, a / 10 % 10, a % 10);
    let h = u16::from(o.humidity);
    let mut m = [
        (sensor >> 8) as u8,
        sensor as u8,
        (o.channel & 0xF) << 4 | (o.id & 0xF),
        (o.id & 0xF0) | u8::from(!o.battery_ok) << 2,
        (tenths << 4 | units) as u8,
        (tens << 4 | u16::from(o.t < 0) << 3 | (hundreds & 7)) as u8,
        ((h % 10) << 4 | (h / 10 % 10)) as u8,
        0,
        0,
    ];
    oregon_checksum(&mut m);
    m
}

/// Oregon Scientific THGR810 (v3, sensor F824): after 24 zero bits and the nibble 5 (sent 0101), the nibble-reflected
/// message, Manchester at 488 µs half-bits.
pub fn oregon_v3(o: &Oregon) -> Train {
    let mut bits = vec![0u8; 24];
    bits.extend_from_slice(&[0, 1, 0, 1]);
    for x in oregon_msg(0xF824, o) {
        bits.extend(bits_of(&[reflect_nibbles(x)]));
    }
    manchester_ook(&bits, 488.0)
}

/// Oregon Scientific THGR228N (v2.1, sensor 1D20): the sync 55 55 55 55 99, then the first 68 bits of the nibble-reflected
/// message, each sent as an inverted and a normal bit, Manchester at 488 µs half-bits.
pub fn oregon_v21(o: &Oregon) -> Train {
    let mut data = Vec::new();
    for x in oregon_msg(0x1D20, o) {
        data.extend(bits_of(&[reflect_nibbles(x)]));
    }
    data.truncate(68);
    let mut bits = bits_of(&[0x55, 0x55, 0x55, 0x55, 0x99]);
    bits.extend(ieee_chips(&data));
    manchester_ook(&bits, 488.0)
}

/// Schrader TPMS (FCC MRXGG4): the bits 0111, then the 8 octets (the flags, the 28-bit id, the pressure in units of 2.5 kPa,
/// the temperature in °C plus 50, the CRC-8 poly 0x07 register 0xF0), Manchester at 120 µs half-bits.
pub fn schrader(flags: u8, id: u32, pressure: u8, temp: u8) -> Train {
    let mut b = [
        flags >> 4,
        (flags & 0xF) << 4 | ((id >> 24) as u8 & 0xF),
        (id >> 16) as u8,
        (id >> 8) as u8,
        id as u8,
        pressure,
        temp,
        0,
    ];
    b[7] = CRC8_SCHRADER.checksum(&b[0..7]);
    let mut bits = vec![0u8, 1, 1, 1];
    bits.extend(bits_of(&b));
    manchester_ook(&bits, 120.0)
}

/// A PT2262 / EV1527 remote: 6 copies of the 24 bits of `code` in PWM (a 1 is 1404 µs on, 464 µs off), each ended by a
/// 464 µs sync pulse and 14 ms of silence.
pub fn ev1527(code: [u8; 3]) -> Train {
    let bits = bits_of(&code);
    let mut tr = train(Modulation::Ook);
    for _ in 0..6 {
        encode::pwm(&bits, (1404.0, 464.0), (464.0, 1404.0), &mut tr);
        push(&mut tr, 464.0, 14000.0);
    }
    tr
}

/// Toyota TPMS (PMV-C210): the 32-bit id, the pressure `p1` (units of 0.25 PSI plus 7 PSI), the temperature `temp` (°C plus
/// 40), the status bits (bit 7 and bits 6-0), the inverted pressure and the CRC-8 (poly 0x07, register 0x80). FSK NRZ at
/// 52 µs, ±30 kHz: 01010101001111, then the 72 bits and 4 zero bits in FM0.
pub fn toyota(id: u32, p1: u8, temp: u8, status: u8) -> Train {
    let [i0, i1, i2, i3] = id.to_be_bytes();
    let mut b = [
        i0,
        i1,
        i2,
        i3,
        (status & 0x80) | p1 >> 1,
        (p1 & 1) << 7 | temp >> 1,
        (temp & 1) << 7 | (status & 0x7F),
        !p1,
        0,
    ];
    b[8] = CRC8_TOYOTA.checksum(&b[0..8]);
    let mut data = bits_of(&b);
    data.extend_from_slice(&[0, 0, 0, 0]);
    let mut bits: Vec<u8> = "01010101001111".bytes().map(|c| c - b'0').collect();
    fm0(&data, 1, &mut bits);
    fsk(&bits, 52.0, 30e3)
}

/// Appends the FM0 (biphase space) chips of `data` to `out`, starting from level `lvl`: a level change at the start of each
/// cell, and one more in its middle for a 0.
fn fm0(data: &[u8], mut lvl: u8, out: &mut Vec<u8>) {
    for &d in data {
        let a = 1 - lvl;
        let c = if d != 0 { a } else { 1 - a };
        out.push(a);
        out.push(c);
        lvl = c;
    }
}

/// The VDO TPMS frame of the Renault and Ford sensors: as read after inversion, AA AA AA A9, the IEEE Manchester chips of
/// `b` and AA; sent inverted as FSK NRZ at 52 µs, ±30 kHz.
fn vdo_inverted(b: &[u8]) -> Train {
    let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xA9]);
    bits.extend(ieee_chips(&bits_of(b)));
    bits.extend(bits_of(&[0xAA]));
    let inv: Vec<u8> = bits.iter().map(|x| 1 - x).collect();
    fsk(&inv, 52.0, 30e3)
}

/// Renault TPMS: the flags (6 bits), the pressure (units of 0.75 kPa, 10 bits), the temperature (°C plus 30), the 24-bit id
/// low octet first, the octets FF FF and the CRC-8 (poly 0x07, register 0).
pub fn renault(flags: u8, pressure: u16, temp: u8, id: u32) -> Train {
    let mut b = vec![
        flags << 2 | ((pressure >> 8) as u8 & 3),
        pressure as u8,
        temp,
        id as u8,
        (id >> 8) as u8,
        (id >> 16) as u8,
        0xFF,
        0xFF,
        0,
    ];
    b[8] = CRC8_SMBUS.checksum(&b[0..8]);
    vdo_inverted(&b)
}

/// Ford TPMS (VDO): the 32-bit id, the pressure (units of 0.25 PSI, 9 bits), the temperature (°C plus 56, 7 bits), the flag
/// octet 0x05 (normal) with the top pressure bit in bit 5, and the add checksum.
pub fn ford(id: u32, pressure: u16, temp: u8) -> Train {
    let [i0, i1, i2, i3] = id.to_be_bytes();
    let mut b = vec![
        i0,
        i1,
        i2,
        i3,
        pressure as u8,
        temp & 0x7F,
        0x05 | ((pressure >> 8) as u8 & 1) << 5,
        0,
    ];
    b[7] = sum8(&b[0..7]);
    vdo_inverted(&b)
}

/// Efergy e2 classic: the id (low octet first), the battery bit, the interval code (0 to 3: 6 to 24 s), the current `raw`
/// and the exponent octet (the current is raw / 2^(15 − exponent) A), and the add checksum. FSK PWM, ±20 kHz: a 500 µs sync
/// mark and 136 µs of space, then a 1 is 64 µs mark and 136 µs space and a 0 the other way round.
pub fn efergy(id: u16, battery_ok: bool, interval: u8, raw: u16, exponent: i8) -> Train {
    let mut b = [
        0x00u8,
        id as u8,
        (id >> 8) as u8,
        u8::from(battery_ok) << 6 | (interval & 3) << 4,
        (raw >> 8) as u8,
        raw as u8,
        exponent as u8,
        0,
    ];
    b[7] = sum8(&b[0..7]);
    let mut tr = train(Modulation::Fsk);
    tr.deviation_hz = 20e3;
    push(&mut tr, 500.0, 136.0);
    encode::pwm(&bits_of(&b), (64.0, 136.0), (136.0, 64.0), &mut tr);
    set_last_space(&mut tr, 200.0);
    tr
}

/// CurrentCost TX: the power message of device `dev` (12 bits) with power 0 of `power` W (15 bits) and no power 1 or 2.
/// FSK NRZ at 250 µs, ±20 kHz: as read after inversion, 0xCC, the first 45 bits of CC CC CC CE 91 5D and the IEEE Manchester
/// chips of the 8 octets; sent inverted.
pub fn currentcost(dev: u16, power: u16) -> Train {
    let b = [
        (dev >> 8) as u8 & 0xF,
        dev as u8,
        0x80 | ((power >> 8) as u8 & 0x7F),
        power as u8,
        0,
        0,
        0,
        0,
    ];
    let mut bits = bits_of(&[0xCC]);
    bits.extend(bits_of(&[0xCC, 0xCC, 0xCC, 0xCE, 0x91, 0x5D])[..45].iter());
    bits.extend(ieee_chips(&bits_of(&b)));
    let inv: Vec<u8> = bits.iter().map(|x| 1 - x).collect();
    fsk(&inv, 250.0, 20e3)
}

/// The fields of a Bresser 6-in-1 weather message (sensor type 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bresser6 {
    /// The 32-bit id.
    pub id: u32,
    /// The channel (0 to 7).
    pub channel: u8,
    /// The battery is good.
    pub battery_ok: bool,
    /// The temperature, tenths of °C (−500 to 999).
    pub t: i16,
    /// The humidity, % (0 to 99).
    pub humidity: u8,
    /// The wind gust, tenths of m/s (0 to 999).
    pub gust: u16,
    /// The mean wind speed, tenths of m/s (0 to 999).
    pub speed: u16,
    /// The wind direction, degrees (0 to 999).
    pub dir: u16,
    /// The UV index, tenths (0 to 999).
    pub uv: u16,
}

/// Bresser 6-in-1: FSK NRZ at 124 µs, ±40 kHz, after AA AA AA AA 2D D4: the LFSR digest 16 (generator 0x8810, key 0x5412),
/// the id, the type and channel, the wind values inverted in BCD, the direction, the temperature, the humidity, the UV index
/// inverted, and the octet that makes the add sum of octets 2 to 17 0xFF.
pub fn bresser6(w: &Bresser6) -> Train {
    let traw = if w.t < 0 {
        1000 + i32::from(w.t)
    } else {
        i32::from(w.t)
    } as u32;
    let (g, s, d, u) = (
        u32::from(w.gust),
        u32::from(w.speed),
        u32::from(w.dir),
        u32::from(w.uv),
    );
    let mut m = [0u8; 18];
    m[2..6].copy_from_slice(&w.id.to_be_bytes());
    m[6] = 0x10 | (w.channel & 7);
    m[7] = !bcd(g / 10);
    m[8] = !((((g % 10) << 4) | (s % 10)) as u8);
    m[9] = !bcd(s / 10);
    m[10] = bcd(d / 10);
    m[11] = ((d % 10) << 4) as u8;
    m[12] = bcd(traw / 10);
    m[13] = ((traw % 10) as u8) << 4 | u8::from(w.t < 0) << 3 | u8::from(w.battery_ok) << 1;
    m[14] = bcd(u32::from(w.humidity));
    m[15] = !bcd(u / 10);
    m[16] = (!((u % 10) as u8) & 0xF) << 4;
    m[17] = 0xFFu8.wrapping_sub(sum8(&m[2..17]));
    let dg = lfsr_digest16(&m[2..17], 0x8810, 0x5412);
    m[0] = (dg >> 8) as u8;
    m[1] = dg as u8;
    let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
    bits.extend(bits_of(&m));
    fsk(&bits, 124.0, 40e3)
}

/// The fields of a Bresser 5-in-1 weather message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bresser5 {
    /// The id.
    pub id: u8,
    /// The battery is good.
    pub battery_ok: bool,
    /// The temperature, tenths of °C (−999 to 999).
    pub t: i16,
    /// The humidity, % (0 to 99).
    pub humidity: u8,
    /// The wind gust, tenths of m/s (12 bits).
    pub gust: u16,
    /// The mean wind speed, tenths of m/s (0 to 999).
    pub speed: u16,
    /// The wind direction: the index of 16 directions from north.
    pub dir: u8,
    /// The rain, tenths of mm (0 to 9999).
    pub rain: u16,
}

/// Bresser 5-in-1: FSK NRZ at 124 µs, ±40 kHz, after AA AA AA AA AA 2D D4: 13 octets inverted, the count of set bits of the
/// 12 message octets, and the message (the id, the type 0, the wind, the temperature and humidity in BCD, the rain in BCD,
/// the battery and the sign of the temperature).
pub fn bresser5(w: &Bresser5) -> Train {
    let a = u32::from(w.t.unsigned_abs());
    let (s, r) = (u32::from(w.speed), u32::from(w.rain));
    let mut m = [0u8; 26];
    m[14..26].copy_from_slice(&[
        w.id,
        0x00,
        w.gust as u8,
        (w.dir & 0xF) << 4 | ((w.gust >> 8) as u8 & 0xF),
        bcd(s % 100),
        (s / 100 % 10) as u8,
        bcd(a % 100),
        (a / 100 % 10) as u8,
        bcd(u32::from(w.humidity)),
        bcd(r % 100),
        bcd(r / 100),
        u8::from(!w.battery_ok) << 7 | u8::from(w.t < 0),
    ]);
    m[13] = m[14..26].iter().map(|b| b.count_ones()).sum::<u32>() as u8;
    for i in 0..13 {
        m[i] = !m[i + 13];
    }
    let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
    bits.extend(bits_of(&m));
    fsk(&bits, 124.0, 40e3)
}

/// LaCrosse TX29-IT (`bit_us` 55) and TX35DTH-IT (`bit_us` 105): FSK NRZ, ±30 kHz, after AA AA AA 2D D4: the model nibble 9,
/// the 6-bit id, the new-battery bit, the temperature `t` (tenths of °C plus 400, 0 to 999, in BCD), the battery-low bit, the
/// humidity (0x6A for none) and the CRC-8 (poly 0x31, register 0).
pub fn lacrosse_it(bit_us: f32, id: u8, battery_ok: bool, t: u16, humidity: u8) -> Train {
    let mut b = [
        0x90 | (id >> 2 & 0xF),
        (id & 3) << 6 | (t / 100 % 10) as u8,
        ((t / 10 % 10) as u8) << 4 | (t % 10) as u8,
        u8::from(!battery_ok) << 7 | (humidity & 0x7F),
        0,
    ];
    b[4] = CRC8_31.checksum(&b[0..4]);
    let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
    bits.extend(bits_of(&b));
    fsk(&bits, bit_us, 30e3)
}

/// The raw fields of a Fine Offset WH24 message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wh24 {
    /// The id.
    pub id: u8,
    /// The battery is good.
    pub battery_ok: bool,
    /// The temperature: tenths of °C plus 400, 11 bits (0x7FF none).
    pub t: u16,
    /// The humidity, % (0xFF none).
    pub humidity: u8,
    /// The wind direction, degrees, 9 bits (0x1FF none).
    pub dir: u16,
    /// The wind speed: units of 0.14 m/s, 9 bits (0x1FF none).
    pub speed: u16,
    /// The wind gust: units of 1.12 m/s (0xFF none).
    pub gust: u8,
    /// The rain: units of 0.3 mm.
    pub rain: u16,
    /// The UV value (0xFFFF none).
    pub uv: u16,
    /// The light: tenths of lux, 24 bits (0xFFFFFF none).
    pub light: u32,
}

/// Fine Offset WH24: FSK NRZ at 58 µs, ±30 kHz, after AA AA AA AA AA 2D D4: the 17 octets with the CRC-8 (poly 0x31,
/// register 0) and the add checksum.
pub fn wh24(w: &Wh24) -> Train {
    let mut b = [
        0x24u8,
        w.id,
        w.dir as u8,
        ((w.dir >> 8) as u8 & 1) << 7
            | ((w.speed >> 8) as u8 & 1) << 4
            | u8::from(!w.battery_ok) << 3
            | ((w.t >> 8) as u8 & 7),
        w.t as u8,
        w.humidity,
        w.speed as u8,
        w.gust,
        (w.rain >> 8) as u8,
        w.rain as u8,
        (w.uv >> 8) as u8,
        w.uv as u8,
        (w.light >> 16) as u8,
        (w.light >> 8) as u8,
        w.light as u8,
        0,
        0,
    ];
    b[15] = CRC8_31.checksum(&b[0..15]);
    b[16] = sum8(&b[0..16]);
    let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
    bits.extend(bits_of(&b));
    fsk(&bits, 58.0, 30e3)
}

/// Prologue: 7 copies of 36 bits in PPM (a 500 µs pulse, then 2000 µs for a 0 and 4000 µs for a 1), each followed by a
/// 500 µs pulse and a 9000 µs sync gap. The bits: the type 9, the id, the battery bit, the button bit 0, the channel field
/// (0 to 3), the temperature `t` (tenths of °C, 12 bits) and the humidity (0xCC for none).
pub fn prologue(id: u8, channel: u8, battery_ok: bool, t: i16, humidity: u8) -> Train {
    let t = t as u16 & 0xFFF;
    let b = [
        0x90 | id >> 4,
        (id & 0xF) << 4 | u8::from(battery_ok) << 3 | (channel & 3),
        (t >> 4) as u8,
        ((t & 0xF) as u8) << 4 | humidity >> 4,
        (humidity & 0xF) << 4,
    ];
    let bits = &bits_of(&b)[..36];
    let mut tr = train(Modulation::Ook);
    for _ in 0..7 {
        encode::ppm(bits, 500.0, 2000.0, 4000.0, &mut tr);
        push(&mut tr, 500.0, 9000.0);
    }
    tr
}

/// Bresser 3CH: `b` is the 4 octets as the protocol reads them after inversion (the id, the battery, channel and temperature
/// high bits, the temperature low octet, the humidity); the add checksum follows and the 40 bits are sent inverted. 15 copies,
/// each 4 sync pulses of 750 µs on and 750 µs off and the bits in PWM (a sent 1 is 250 µs on and 500 µs off, a 0 the other
/// way round).
pub fn bresser3ch(b: [u8; 4]) -> Train {
    let row: Vec<u8> = [b[0], b[1], b[2], b[3], sum8(&b)]
        .iter()
        .map(|x| !x)
        .collect();
    let bits = bits_of(&row);
    let mut tr = train(Modulation::Ook);
    for _ in 0..15 {
        for _ in 0..4 {
            push(&mut tr, 750.0, 750.0);
        }
        encode::pwm(&bits, (250.0, 500.0), (500.0, 250.0), &mut tr);
    }
    tr
}

/// Citroen TPMS (VDO): `b` is octets 0 to 8 of the message (the state, the 32-bit id, the flags and repeat, the pressure,
/// the temperature, the battery octet); the check octet that makes the XOR of octets 1 to 9 zero follows. FSK NRZ at 52 µs,
/// ±30 kHz: 55 55 55 56, the message in Manchester (1 is 10), the trailer 01111110.
pub fn citroen(b: [u8; 9]) -> Train {
    let mut m = b.to_vec();
    m.push(xor8(&b[1..9]));
    vdo_plain(&[0x55, 0x55, 0x55, 0x56], &m, 0x7E)
}

/// A VDO TPMS frame sent as it is read: the preamble `pre`, the message `m` in Manchester (1 is 10) and the trailer octet
/// `tail`, as FSK NRZ at 52 µs, ±30 kHz.
fn vdo_plain(pre: &[u8], m: &[u8], tail: u8) -> Train {
    let mut bits = bits_of(pre);
    bits.extend(chips(&bits_of(m), ManchesterConv::Thomas));
    bits.extend(bits_of(&[tail]));
    fsk(&bits, 52.0, 30e3)
}

/// Schrader EG53MA4 TPMS: `b` is the 14 octets of the row as the protocol reads it (5 octets before the message, then octets
/// 0 to 8 of the message: the flags, the 24-bit id, the pressure in units of 2.5 kPa, the temperature in °F); the add
/// checksum of the message follows. Manchester at 123 µs half-bits; the first bit is a 0.
pub fn schrader_eg53(b: [u8; 14]) -> Train {
    let mut m = b.to_vec();
    m.push(sum8(&b[5..14]));
    manchester_ook(&bits_of(&m), 123.0)
}

/// Bresser 7-in-1: `m` is octets 2 to 24 of the message before whitening; octets 0 and 1 are the LFSR digest 16 (generator
/// 0x8810, key 0xBA95) XOR 0x6DF1, and every octet is sent XOR 0xAA. FSK NRZ at 124 µs, ±40 kHz, after AA AA AA 2D D4.
pub fn bresser7(m: [u8; 23]) -> Train {
    let d = lfsr_digest16(&m, 0x8810, 0xBA95) ^ 0x6DF1;
    let mut full = vec![(d >> 8) as u8, d as u8];
    full.extend_from_slice(&m);
    let raw: Vec<u8> = full.iter().map(|x| x ^ 0xAA).collect();
    let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
    bits.extend(bits_of(&raw));
    fsk(&bits, 124.0, 40e3)
}

/// The 36 bits of a Rubicson frame: the id, the battery-good bit, the channel field (0 to 3), the 12-bit temperature `t`
/// (tenths of °C), the nibble F and the CRC-8 (poly 0x31, register 0x6C) of the 4 octets that end with that nibble.
fn rubicson_bits(id: u8, channel: u8, battery_ok: bool, t: i16) -> Vec<u8> {
    let b = [
        id,
        u8::from(battery_ok) << 7 | (channel & 3) << 4 | ((t >> 8) as u8 & 0xF),
        t as u8,
        0xF0,
    ];
    let c = CRC8_RUBICSON.checksum(&b);
    let mut bits = bits_of(&[b[0], b[1], b[2], 0xF0 | c >> 4, (c & 0xF) << 4]);
    bits.truncate(36);
    bits
}

/// Rubicson, TFA 30.3197, InFactory PT-310: 12 copies of the 36 bits of [`rubicson_bits`] in PPM (500 µs marks, the first
/// of each copy 388 µs; a 976 µs space for a 0 and 1940 µs for a 1), each closed by a 500 µs mark and 4000 µs of silence.
/// The wake-up pulses before the first copy are left out.
pub fn rubicson(id: u8, channel: u8, battery_ok: bool, t: i16) -> Train {
    let bits = rubicson_bits(id, channel, battery_ok, t);
    let mut tr = train(Modulation::Ook);
    for _ in 0..12 {
        let first = tr.pulses.len();
        encode::ppm(&bits, 500.0, 976.0, 1940.0, &mut tr);
        tr.pulses[first].mark = tr.samples(388.0);
        push(&mut tr, 500.0, 4000.0);
    }
    tr
}

/// Solight TE44, TE66, EMOS E0107T: 12 copies of the 36 bits of [`rubicson_bits`] and a 0 bit in PPM (490 µs marks, a
/// 976 µs space for a 0 and 1946 µs for a 1, the space of the 37th bit 980 µs), each closed by a 490 µs mark and 3880 µs.
/// Not confirmed from a device document: the 980 µs space of the 37th bit.
pub fn solight(id: u8, channel: u8, battery_ok: bool, t: i16) -> Train {
    let bits = rubicson_bits(id, channel, battery_ok, t);
    let mut tr = train(Modulation::Ook);
    for _ in 0..12 {
        encode::ppm(&bits, 490.0, 976.0, 1946.0, &mut tr);
        push(&mut tr, 490.0, 980.0);
        push(&mut tr, 490.0, 3880.0);
    }
    tr
}

/// Generic temperature sensor 1: a 500 µs mark and 7000 µs, then 10 frames of 24 bits in PPM (500 µs marks, a 2000 µs space
/// for a 0 and 4000 µs for a 1), each closed by a 500 µs mark and 7000 µs. The bits: the id, the 2-bit battery field and
/// the temperature `t` (tenths of °C) times 4 as a 14-bit two's complement field.
/// Not confirmed from a device document: the 500 µs mark and the 7000 µs space between frames.
pub fn generic_temp(id: u8, battery: u8, t: i16) -> Train {
    let f = (t as u16).wrapping_mul(4) & 0x3FFF;
    let bits = bits_of(&[id, (battery & 3) << 6 | (f >> 8) as u8, f as u8]);
    let mut tr = train(Modulation::Ook);
    push(&mut tr, 500.0, 7000.0);
    for _ in 0..10 {
        encode::ppm(&bits, 500.0, 2000.0, 4000.0, &mut tr);
        push(&mut tr, 500.0, 7000.0);
    }
    tr
}

/// LaCrosse TX141-B (`len` 32), TX141-Bv2 (37) and TX141-Bv3 (33): `copies` copies, each 4 sync pulses of 833 µs and the
/// bits in PWM as the TX141TH-Bv2 sends them, then two 833 µs sync pulses. The bits: the id, the battery-good bit, the
/// channel field (0 to 3), the 12-bit temperature `raw` (tenths of °C plus 500), then the TH layout cut to `len`: a
/// humidity octet 0 and the digest.
/// Not confirmed from a device document: the bits after the temperature, and 12 copies of the B and Bv2.
pub fn tx141b(len: usize, copies: usize, id: u8, channel: u8, battery_ok: bool, raw: u16) -> Train {
    let mut b = [
        id,
        u8::from(battery_ok) << 7 | (channel & 3) << 4 | ((raw >> 8) as u8 & 0xF),
        raw as u8,
        0,
        0,
    ];
    b[4] = lfsr_digest8_reflect(&b[0..4], 0x31, 0xF4);
    let mut bits = bits_of(&b);
    bits.truncate(len);
    let mut tr = train(Modulation::Ook);
    for _ in 0..copies {
        for _ in 0..4 {
            push(&mut tr, 833.0, 833.0);
        }
        encode::pwm(&bits, (417.0, 208.0), (208.0, 417.0), &mut tr);
    }
    push(&mut tr, 833.0, 833.0);
    push(&mut tr, 833.0, 833.0);
    tr
}

/// Acurite 606TX, Technoline TX960: a 500 µs mark and an 8500 µs sync space, then 8 copies of 33 bits in PPM (500 µs marks,
/// a 2000 µs space for a 0 and 4000 µs for a 1), each closed by a 500 µs mark and 8500 µs. The bits: the id, the
/// battery-good bit, the button bit, the channel field (0 to 3), the 12-bit temperature `t` (tenths of °C), the LFSR digest
/// (generator 0x98, key 0xF1) of the 3 octets before it and a 0 bit.
/// Not confirmed from a device document: the 2000 µs space of the 33rd bit, and 8 copies (6 to 8).
pub fn acurite606(id: u8, channel: u8, battery_ok: bool, button: bool, t: i16) -> Train {
    let mut b = [
        id,
        u8::from(battery_ok) << 7
            | u8::from(button) << 6
            | (channel & 3) << 4
            | ((t >> 8) as u8 & 0xF),
        t as u8,
        0,
    ];
    b[3] = lfsr_digest8(&b[0..3], 0x98, 0xF1);
    let mut bits = bits_of(&b);
    bits.push(0);
    let mut tr = train(Modulation::Ook);
    push(&mut tr, 500.0, 8500.0);
    for _ in 0..8 {
        encode::ppm(&bits, 500.0, 2000.0, 4000.0, &mut tr);
        push(&mut tr, 500.0, 8500.0);
    }
    tr
}

/// Acurite 609TXC: a 500 µs mark and an 8500 µs sync space, then 3 copies of 40 bits in PPM (500 µs marks, a 1000 µs space
/// for a 0 and 2000 µs for a 1), each closed by a 500 µs mark and 8500 µs. The bits: the id, the status nibble 2 with its
/// battery-low bit (bit 3), the 12-bit temperature `t` (tenths of °C), the humidity and the add checksum.
/// Not confirmed from a device document: 3 copies.
pub fn acurite609(id: u8, battery_ok: bool, t: i16, humidity: u8) -> Train {
    let mut b = [
        id,
        (0x2 | u8::from(!battery_ok) << 3) << 4 | ((t >> 8) as u8 & 0xF),
        t as u8,
        humidity,
        0,
    ];
    b[4] = sum8(&b[0..4]);
    let bits = bits_of(&b);
    let mut tr = train(Modulation::Ook);
    push(&mut tr, 500.0, 8500.0);
    for _ in 0..3 {
        encode::ppm(&bits, 500.0, 1000.0, 2000.0, &mut tr);
        push(&mut tr, 500.0, 8500.0);
    }
    tr
}

/// Kedsum, Pearl NC-7415: 15 sync pulses (a 500 µs mark and 8000 µs), then 6 copies of 42 bits in PPM (500 µs marks, a
/// 2000 µs space for a 0 and 4000 µs for a 1), each closed by a sync pulse. The bits: two 0 bits, the id, the battery
/// field (10 good, 01 weak), the channel field (0 to 3), the 12-bit temperature `raw` (tenths of °F plus 900) as low,
/// middle and high nibble, the humidity with its nibbles swapped, the flags nibble and the CRC-4 (poly 0x3) of the 4
/// octets before it XOR the flags.
pub fn kedsum(id: u8, channel: u8, battery_ok: bool, raw: u16, humidity: u8, flags: u8) -> Train {
    let flags = flags & 0xF;
    let mut b = [
        id,
        (if battery_ok { 2 } else { 1 }) << 6 | (channel & 3) << 4 | (raw & 0xF) as u8,
        (((raw >> 4) & 0xF) as u8) << 4 | ((raw >> 8) & 0xF) as u8,
        humidity.rotate_right(4),
        flags << 4,
    ];
    b[4] |= (CRC4_KEDSUM.checksum(&b[0..4]) ^ flags) & 0xF;
    let mut bits = vec![0u8, 0];
    bits.extend(bits_of(&b));
    let mut tr = train(Modulation::Ook);
    for _ in 0..15 {
        push(&mut tr, 500.0, 8000.0);
    }
    for _ in 0..6 {
        encode::ppm(&bits, 500.0, 2000.0, 4000.0, &mut tr);
        push(&mut tr, 500.0, 8000.0);
    }
    tr
}

/// Globaltronics GT-WT-02: a 500 µs mark and a 9000 µs sync space, then 3 copies of 37 bits in PPM (500 µs marks, a 2000 µs
/// space for a 0 and 4000 µs for a 1), each closed by a 500 µs mark and 9000 µs. The bits: the id, the battery-low bit, the
/// manual-send bit 0, the channel field (0 to 2), the 12-bit temperature `t` (tenths of °C), the 7-bit humidity code
/// (20 to 90, 10 low, 110 high) and the sum of the nibbles of the 31 bits before it modulo 64 (the last nibble 3 bits).
/// Not confirmed from a device document: 3 copies, and that the 2000 and 4000 µs widths are the spaces.
pub fn gtwt02(id: u8, channel: u8, battery_ok: bool, t: i16, humidity: u8) -> Train {
    let b = [
        id,
        u8::from(!battery_ok) << 7 | (channel & 3) << 4 | ((t >> 8) as u8 & 0xF),
        t as u8,
    ];
    let h = humidity & 0x7F;
    let nib = |x: u8| u32::from(x >> 4) + u32::from(x & 0xF);
    let sum =
        (nib(b[0]) + nib(b[1]) + nib(b[2]) + u32::from(h >> 3) + u32::from((h << 1) & 0xE)) % 64;
    let mut bits = bits_of(&b);
    bits.extend((0..7).rev().map(|i| (h >> i) & 1));
    bits.extend((0..6).rev().map(|i| (sum >> i) as u8 & 1));
    let mut tr = train(Modulation::Ook);
    push(&mut tr, 500.0, 9000.0);
    for _ in 0..3 {
        encode::ppm(&bits, 500.0, 2000.0, 4000.0, &mut tr);
        push(&mut tr, 500.0, 9000.0);
    }
    tr
}

/// A model of the Fine Offset WH2 family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wh2Model {
    /// Fine Offset WH2: 8 leading 1 bits.
    Wh2,
    /// Fine Offset WH2A: 7 leading 1 bits and the add checksum.
    Wh2a,
    /// Fine Offset WH5: 7 leading 1 bits.
    Wh5,
    /// Telldus Proove: 9 leading 1 bits.
    Telldus,
    /// TFA 30.3225: 7 leading 1 bits and the add checksum.
    Tfa,
}

/// The Fine Offset WH2 family: 2 copies of the bits in PWM (a 1 is a 544 µs mark, a 0 a 1524 µs mark, each followed by
/// 1036 µs), 5000 µs between them. The bits: the leading 1 bits of the model, the type nibble 4, the id, the 12-bit
/// temperature field `raw` as the model codes it, the humidity (0xFF for none), the CRC-8 (poly 0x31, register 0) of the 4
/// octets before it and, for the WH2A and the TFA, the add checksum of the 5 octets before it.
/// Not confirmed from a device document: the 5000 µs between the copies.
pub fn wh2(model: Wh2Model, id: u8, raw: u16, humidity: u8) -> Train {
    let mut b = vec![
        0x40 | id >> 4,
        (id & 0xF) << 4 | ((raw >> 8) as u8 & 0xF),
        raw as u8,
        humidity,
        0,
    ];
    b[4] = CRC8_31.checksum(&b[0..4]);
    if matches!(model, Wh2Model::Wh2a | Wh2Model::Tfa) {
        b.push(sum8(&b));
    }
    let lead = match model {
        Wh2Model::Wh2 => 8,
        Wh2Model::Telldus => 9,
        _ => 7,
    };
    let mut bits = vec![1u8; lead];
    bits.extend(bits_of(&b));
    let mut tr = train(Modulation::Ook);
    for k in 0..2 {
        encode::pwm(&bits, (544.0, 1036.0), (1524.0, 1036.0), &mut tr);
        if k == 0 {
            set_last_space(&mut tr, 5000.0);
        }
    }
    tr
}

/// ThermoPro TP11: 4 copies of 32 bits in PPM (500 µs marks, a 500 µs space for a 0 and 1500 µs for a 1), each closed by a
/// 500 µs mark and 3000 µs. The bits: the 12-bit id, the 12-bit temperature `raw` (tenths of °C plus 200) and the
/// reflected LFSR digest (generator 0x51, key 0x04) of the 3 octets before it.
/// Not confirmed from a device document: the 500 µs marks, the space widths, the 3000 µs between copies and 4 copies.
pub fn tp11(id: u16, raw: u16) -> Train {
    let mut b = [
        (id >> 4) as u8,
        ((id & 0xF) as u8) << 4 | ((raw >> 8) as u8 & 0xF),
        raw as u8,
        0,
    ];
    b[3] = lfsr_digest8_reflect(&b[0..3], 0x51, 0x04);
    let bits = bits_of(&b);
    let mut tr = train(Modulation::Ook);
    for _ in 0..4 {
        encode::ppm(&bits, 500.0, 500.0, 1500.0, &mut tr);
        push(&mut tr, 500.0, 3000.0);
    }
    tr
}

/// ThermoPro TP08, TP12, TP20: 17 copies of 40 bits in PPM (500 µs marks, a 500 µs space for a 0 and 1500 µs for a 1),
/// every copy but the last with a trailing 1 bit, each closed by a 500 µs mark and 3000 µs. The bits: the id, the two
/// 12-bit temperatures `raw1` (the probe) and `raw2` (tenths of °C plus 200) and the reflected LFSR digest (generator 0x51,
/// key 0x04) of the 4 octets before it.
/// Not confirmed from a device document: the 500 µs marks, the space widths and the 3000 µs between copies.
pub fn tp12(id: u8, raw1: u16, raw2: u16) -> Train {
    let mut b = [
        id,
        raw1 as u8,
        ((raw1 >> 8) as u8 & 0xF) << 4 | ((raw2 >> 8) as u8 & 0xF),
        raw2 as u8,
        0,
    ];
    b[4] = lfsr_digest8_reflect(&b[0..4], 0x51, 0x04);
    let bits = bits_of(&b);
    let mut tr = train(Modulation::Ook);
    for k in 0..17 {
        encode::ppm(&bits, 500.0, 500.0, 1500.0, &mut tr);
        if k < 16 {
            encode::ppm(&[1], 500.0, 500.0, 1500.0, &mut tr);
        }
        push(&mut tr, 500.0, 3000.0);
    }
    tr
}

/// FT-004-B: 3 copies of 46 bits back to back in PPM (500 µs marks, a 1956 µs space for a 0 and 3900 µs for a 1), closed by
/// a 500 µs mark and 4000 µs. The bits are the octets F4 F3 24, the temperature `raw` (units of 0.05 °C from −40 °C, 11 bits)
/// and 13, each least significant bit first; the octets other than the type F4 and the temperature bits are those of the
/// published row.
/// Not confirmed from a device document: the 500 µs marks and the content of the octets other than the type and temperature.
pub fn ft004b(raw: u16) -> Train {
    let m = [
        0xF4,
        0xF3,
        0x24,
        raw as u8,
        0x80 | ((raw >> 8) as u8 & 7),
        0x13,
    ];
    let mut copy = Vec::with_capacity(48);
    unpack(&m, BitOrder::Lsb, &mut copy);
    copy.truncate(46);
    let mut tr = train(Modulation::Ook);
    for _ in 0..3 {
        encode::ppm(&copy, 500.0, 1956.0, 3900.0, &mut tr);
    }
    push(&mut tr, 500.0, 4000.0);
    tr
}

/// Itron ERT SCM: the 96 bits in Manchester (1 is high then low) at a data bit of 1/32768 s: F9 53 (the sync bit and the
/// preamble), the high 2 bits of the 26-bit id, the physical tamper (2 bits), the ERT type (4 bits), the encoder tamper (2
/// bits), the 24-bit consumption, the low 24 bits of the id and the CRC-16 (poly 0x6F63, register 0) of octets 2 to 9.
/// Not confirmed from a device document: the data bit (sources give 1/32768 s and 1/16384 s), the polarity of the sync bit
/// (the stack reads it as a 0), one copy, the hop channel.
pub fn ert_scm(id: u32, ert_type: u8, physical: u8, encoder: u8, consumption: u32) -> Train {
    let [_, c2, c1, c0] = consumption.to_be_bytes();
    let [_, i2, i1, i0] = id.to_be_bytes();
    let mut b = [
        0xF9,
        0x53,
        ((id >> 24) as u8 & 3) << 1,
        (physical & 3) << 6 | (ert_type & 0xF) << 2 | (encoder & 3),
        c2,
        c1,
        c0,
        i2,
        i1,
        i0,
        0,
        0,
    ];
    let crc = CRC16_ERT_SCM.checksum(&b[2..10]).to_be_bytes();
    b[10..12].copy_from_slice(&crc);
    manchester_ook(&bits_of(&b), ERT_HALF_US)
}

/// Itron ERT IDM: `m` is octets 2 to 87 of the frame (from the packet type 0x1C to the meter-id CRC); the training sync 55 55
/// and the frame sync 16 A3 go before it and the CRC-16 (poly 0x1021, register 0xD895) of `m` after it. Manchester (1 is
/// high then low) at a data bit of 1/32768 s.
/// Not confirmed from a device document: the data bit (sources give 1/32768 s and 1/16384 s), one copy, the hop channel.
pub fn ert_idm(m: [u8; 86]) -> Train {
    let mut f = vec![0x55, 0x55, 0x16, 0xA3];
    f.extend_from_slice(&m);
    f.extend_from_slice(&CRC16_ERT_IDM.checksum(&m).to_be_bytes());
    manchester_ook(&bits_of(&f), ERT_HALF_US)
}

/// Honeywell / Ademco 345 MHz, 2Gig DW10/DW11: 6 copies, 2000 µs apart, of the 64 bits in Manchester (1 is low then high)
/// with 136 µs half-bits: FF FE, the channel nibble, the 20-bit id, the event octet and the CRC-16 (register 0, poly 0x8050
/// on channels 2, 4, 9, 10 and 12, else 0x8005) of the 4 octets before it.
/// Not confirmed from a device document: the 2000 µs between copies; the second burst of 6 copies is not sent.
pub fn honeywell(channel: u8, id: u32, event: u8) -> Train {
    let mut b = [
        0xFF,
        0xFE,
        (channel & 0xF) << 4 | ((id >> 16) as u8 & 0xF),
        (id >> 8) as u8,
        id as u8,
        event,
        0,
        0,
    ];
    let crc = if matches!(channel, 2 | 4 | 9 | 10 | 12) {
        CRC16_HONEYWELL_8050.checksum(&b[2..6])
    } else {
        CRC16_UMTS.checksum(&b[2..6])
    };
    b[6..8].copy_from_slice(&crc.to_be_bytes());
    let levels = ieee_chips(&bits_of(&b));
    let mut tr = train(Modulation::Ook);
    for k in 0..6 {
        if k > 0 {
            set_last_space(&mut tr, 2000.0);
        }
        encode::nrz(&levels, 136.0, &mut tr);
    }
    tr
}

/// Kerui PIR and contact sensors: 25 copies of the 24 bits in PWM (a 1 is a 960 µs mark and 320 µs, a 0 a 320 µs mark and
/// 960 µs), each closed by a 320 µs mark and 9000 µs. The bits: the 20-bit id and the 4-bit command.
/// Not confirmed from a device document: the 320 µs and 960 µs widths, the 9000 µs space and the encoder chip.
pub fn kerui(id: u32, command: u8) -> Train {
    let code = [
        (id >> 12) as u8,
        (id >> 4) as u8,
        ((id & 0xF) as u8) << 4 | (command & 0xF),
    ];
    let bits = bits_of(&code);
    let mut tr = train(Modulation::Ook);
    for _ in 0..25 {
        encode::pwm(&bits, (960.0, 320.0), (320.0, 960.0), &mut tr);
        push(&mut tr, 320.0, 9000.0);
    }
    tr
}

/// Springfield temperature and soil moisture: 4 copies of 36 bits in PPM (500 µs marks, a 2000 µs space for a 0 and
/// 4000 µs for a 1), every copy but the last with a trailing 0 bit, each closed by a 500 µs mark and 7000 µs. The nibbles:
/// the id (2), the flags (battery low, button, channel field 0 to 3), the 12-bit temperature `t` (tenths of °C), the
/// moisture (0 to 10), the XOR of the 7 nibbles before it and 0.
/// Not confirmed from a device document: the 500 µs marks, the space widths, 4 copies, the 7000 µs between copies, the
/// last nibble and the trailing bit.
pub fn springfield(
    id: u8,
    channel: u8,
    battery_ok: bool,
    button: bool,
    t: i16,
    moisture: u8,
) -> Train {
    let flags = u8::from(!battery_ok) << 3 | u8::from(button) << 2 | (channel & 3);
    let mut b = [
        id,
        flags << 4 | ((t >> 8) as u8 & 0xF),
        t as u8,
        (moisture & 0xF) << 4,
        0,
    ];
    let x = xor8(&b[0..4]);
    b[3] |= (x >> 4) ^ (x & 0xF);
    let mut bits = bits_of(&b);
    bits.truncate(36);
    let mut tr = train(Modulation::Ook);
    for k in 0..4 {
        encode::ppm(&bits, 500.0, 2000.0, 4000.0, &mut tr);
        if k < 3 {
            encode::ppm(&[0], 500.0, 2000.0, 4000.0, &mut tr);
        }
        push(&mut tr, 500.0, 7000.0);
    }
    tr
}

/// Pacific PMV-107J TPMS: FSK NRZ at 100 µs, ±30 kHz: the chips 1111110, then in FM0 the 66 bits (the 28-bit id, the 6
/// flag bits, the pressure `p` (kPa / 2.48 plus 40), its inverse, the temperature (°C plus 40) and the CRC-8 (poly 0x13,
/// register 0) of the 58 bits before it with 6 leading 0 bits) and two 1 cells.
/// Not confirmed from a device document: the deviation, one copy, the two trailing cells.
pub fn pmv107j(id: u32, flags: u8, p: u8, temp: u8) -> Train {
    let v = (u64::from(id) & 0x0FFF_FFFF) << 6 | u64::from(flags & 0x3F);
    let mut b = [0u8; 9];
    b[0..5].copy_from_slice(&v.to_be_bytes()[3..8]);
    b[5] = p;
    b[6] = !p;
    b[7] = temp;
    b[8] = CRC8_PMV107J.checksum(&b[0..8]);
    let mut data = bits_of(&b).split_off(6);
    data.extend_from_slice(&[1, 1]);
    let mut chips = vec![1u8, 1, 1, 1, 1, 1, 0];
    fm0(&data, 0, &mut chips);
    fsk(&chips, 100.0, 30e3)
}

/// Hyundai / Kia TPMS (VDO TG1C): `b` is octets 0 to 8 of the message (the state, the 32-bit id, the flags and repeat
/// counter, the pressure in units of 1.375 kPa, the temperature in °C plus 50, the battery octet); the CRC-8 (poly 0x07,
/// register 0xAA) follows. FSK NRZ at 52 µs, ±30 kHz: 55 55 55 56, the message in Manchester (1 is 10) and 55.
/// Not confirmed from a device document: the deviation, one copy, the trailing 55.
pub fn hyundai(b: [u8; 9]) -> Train {
    let mut m = b.to_vec();
    m.push(CRC8_HYUNDAI.checksum(&b));
    vdo_plain(&[0x55, 0x55, 0x55, 0x56], &m, 0x55)
}

/// Abarth 124 Spider, Fiat 124, Mazda MX-5 ND TPMS (VDO TG1C): `b` is octets 0 to 7 of the message (the 32-bit id, the flags,
/// the pressure in units of 1.38 kPa, the temperature in °C plus 50, the status); the octet that makes the XOR of the 9
/// octets zero follows. FSK NRZ at 52 µs, ±30 kHz: 55 55 55 55 55 56, the message in Manchester (1 is 10) and 55.
/// Not confirmed from a device document: the deviation, the 48 preamble chips, one copy, the trailing 55.
pub fn abarth(b: [u8; 8]) -> Train {
    let mut m = b.to_vec();
    m.push(xor8(&b));
    vdo_plain(&[0x55, 0x55, 0x55, 0x55, 0x55, 0x56], &m, 0x55)
}
