//! Meteomodem M10 and M20: the Manchester frames with the linear checksum, their GPS data and the sensors.

use super::frame::{
    heading, pattern_str, steinhart_c, sync_word, FrameDecoder, SondeFrame, SondeType,
};
use crate::calendar;
use sdrkit_util_bits::checksum::m10_checksum;
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_geo::time::gnss::{self, GPS_EPOCH_UNIX};
use sdrkit_util_geo::time::LeapSeconds;

/// The sync symbols ("1" = the higher frequency).
pub const SYNC: &str = "11001100110011001010011001001100";
/// Thermistor fit (Shibaura PB5-41E).
pub const THERMISTOR: [f64; 4] = [1.07303516e-3, 2.41296733e-4, 2.26744154e-6, 6.52855181e-8];
/// Series resistors of the three ranges.
pub const RS: [f64; 3] = [12.1e3, 36.5e3, 475e3];
/// Parallel resistors of the three ranges (none for the first).
pub const RP: [f64; 3] = [f64::INFINITY, 330e3, 2e6];

/// The thermistor resistance of an ADC reading in a range.
pub fn thermistor_r(adc: f64, range: usize) -> Option<f64> {
    if adc <= 0.0 || adc >= 4095.0 || range > 2 {
        return None;
    }
    let d = (4095.0 - adc) / adc - RS[range] / RP[range];
    (d > 0.0).then(|| RS[range] / d)
}

/// The ADC reading and range that represent a temperature best.
pub fn thermistor_adc(t_c: f64) -> (u16, usize) {
    let r = super::frame::steinhart_inverse(t_c, THERMISTOR);
    let mut best = (0u16, 0usize, f64::MAX);
    for range in 0..3 {
        let q = RS[range] / r + RS[range] / RP[range];
        let adc = 4095.0 / (1.0 + q);
        let score = (adc - 2048.0).abs();
        if score < best.2 && (50.0..4045.0).contains(&adc) {
            best = (adc.round() as u16, range, score);
        }
    }
    (best.0, best.1)
}

/// Temperature from an ADC reading and range, °C (valid −120…+60).
pub fn temperature(adc: f64, range: usize) -> Option<f64> {
    let r = thermistor_r(adc, range)?;
    let t = steinhart_c(r, THERMISTOR);
    (-120.0..=60.0).contains(&t).then_some(t)
}

/// The cold correction of the humidity.
pub fn humidity_correct(raw: f64, t: f64) -> f64 {
    let mut rh = raw;
    if t < 0.0 {
        rh -= t / 5.5;
    }
    if t < -30.0 {
        rh *= 1.0 + (-30.0 - t) / 75.0;
    }
    rh.clamp(0.0, 100.0)
}

/// Inverse of [`humidity_correct`].
pub fn humidity_raw(rh: f64, t: f64) -> f64 {
    let mut v = rh;
    if t < -30.0 {
        v /= 1.0 + (-30.0 - t) / 75.0;
    }
    if t < 0.0 {
        v += t / 5.5;
    }
    v
}

/// Differential Manchester symbols of the sync and `bytes` (MSB first): a change at every bit boundary, a second one mid-bit for 1.
pub fn frame_symbols(bytes: &[u8], out: &mut Vec<bool>) {
    for c in SYNC.bytes() {
        out.push(c == b'1');
    }
    let mut level = *out.last().unwrap_or(&false);
    for &b in bytes {
        for k in (0..8).rev() {
            let bit = (b >> k) & 1 == 1;
            level = !level;
            out.push(level);
            if bit {
                level = !level;
            }
            out.push(level);
        }
    }
}

/// Fills the checksum (big endian, in the last two bytes) of a frame.
pub fn seal(frame: &mut [u8]) {
    let n = frame.len();
    let c = m10_checksum(&frame[..n - 2]);
    frame[n - 2..].copy_from_slice(&c.to_be_bytes());
}

fn be16(f: &[u8], o: usize) -> i16 {
    i16::from_be_bytes([f[o], f[o + 1]])
}
fn be32(f: &[u8], o: usize) -> i32 {
    i32::from_be_bytes([f[o], f[o + 1], f[o + 2], f[o + 3]])
}
fn be24s(f: &[u8], o: usize) -> i32 {
    let v = (f[o] as i32) << 16 | (f[o + 1] as i32) << 8 | f[o + 2] as i32;
    if v & 0x80_0000 != 0 {
        v - 0x100_0000
    } else {
        v
    }
}

/// The M10 serial text of five raw bytes ("%1X%02u-%1X-%1u%04u").
pub fn m10_serial(b: &[u8]) -> String {
    let v = u16::from_le_bytes([b[3], b[4]]);
    format!(
        "{:X}{:02}-{:X}-{}{:04}",
        b[2] >> 4,
        b[2] & 0xF,
        b[0] & 0xF,
        (v >> 13) & 7,
        v & 0x1FFF
    )
}

/// The five raw bytes of an M10 serial text.
pub fn m10_serial_bytes(s: &str) -> [u8; 5] {
    let p: Vec<&str> = s.split('-').collect();
    let (a, b, c) = (
        p.first().copied().unwrap_or("000"),
        p.get(1).copied().unwrap_or("0"),
        p.get(2).copied().unwrap_or("00000"),
    );
    let hi = u8::from_str_radix(&a[..1], 16).unwrap_or(0);
    let lo: u8 = a[1..].parse().unwrap_or(0);
    let mid = u8::from_str_radix(b, 16).unwrap_or(0);
    let top: u16 = c[..1].parse().unwrap_or(0);
    let low: u16 = c[1..].parse().unwrap_or(0);
    let v = (top << 13) | (low & 0x1FFF);
    [
        mid & 0xF,
        0x08,
        (hi << 4) | (lo & 0xF),
        v as u8,
        (v >> 8) as u8,
    ]
}

/// The M20 serial text of three raw bytes ("%u%02u-%u-%u%04u").
pub fn m20_serial(b: &[u8]) -> String {
    let y = ((b[1] as u16 & 0x0F) << 8) | b[2] as u16;
    format!(
        "{}{:02}-{}-{}{:04}",
        b[0] >> 4,
        b[0] & 0xF,
        b[1] >> 5,
        (b[1] >> 4) & 1,
        y
    )
}

/// The three raw bytes of an M20 serial text.
pub fn m20_serial_bytes(s: &str) -> [u8; 3] {
    let p: Vec<&str> = s.split('-').collect();
    let a = p.first().copied().unwrap_or("000");
    let hi: u8 = a[..1].parse().unwrap_or(0);
    let lo: u8 = a[1..].parse().unwrap_or(0);
    let mid: u8 = p.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let c = p.get(2).copied().unwrap_or("00000");
    let x: u8 = c[..1].parse().unwrap_or(0);
    let y: u16 = c[1..].parse().unwrap_or(0);
    [
        (hi << 4) | (lo & 0xF),
        (mid << 5) | ((x & 1) << 4) | ((y >> 8) as u8 & 0x0F),
        y as u8,
    ]
}

/// What an M10 frame carries (Trimble GPS, type 0x9F).
#[derive(Clone, Debug, Default)]
pub struct M10Tx {
    /// Serial text.
    pub serial: String,
    /// Latitude, longitude (°), height above the ellipsoid (m).
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Height.
    pub alt: f64,
    /// East, north, up speeds (m/s).
    pub ve: f64,
    /// North.
    pub vn: f64,
    /// Up.
    pub vu: f64,
    /// UTC, Unix seconds.
    pub utc: f64,
    /// Satellites.
    pub sats: u8,
    /// Temperature °C.
    pub temp_c: f64,
    /// Humidity %.
    pub humidity: f64,
    /// Battery V.
    pub battery_v: f64,
}

/// Builds a 101-byte M10 frame.
pub fn m10_frame(tx: &M10Tx) -> Vec<u8> {
    let mut f = vec![0u8; 0x65];
    f[0] = 0x64;
    f[1] = 0x9F;
    f[2] = 0x20;
    let sp = |v: f64| ((v / 0.005).round().clamp(-32768.0, 32767.0) as i16).to_be_bytes();
    f[4..6].copy_from_slice(&sp(tx.ve));
    f[6..8].copy_from_slice(&sp(tx.vn));
    f[8..10].copy_from_slice(&sp(tx.vu));
    let gps = gnss::gps_from_utc(tx.utc, &LeapSeconds::BUILTIN).unwrap_or(gnss::WeekTime {
        week: 0,
        tow_s: 0.0,
    });
    let week = gps.week as f64;
    let tow_ms = (gps.tow_s * 1000.0).round() as u32;
    f[0x0A..0x0E].copy_from_slice(&tow_ms.to_be_bytes());
    let deg = |d: f64| ((d * 4_294_967_296.0 / 360.0).round() as i64 as i32).to_be_bytes();
    f[0x0E..0x12].copy_from_slice(&deg(tx.lat));
    f[0x12..0x16].copy_from_slice(&deg(tx.lon));
    f[0x16..0x1A].copy_from_slice(&((tx.alt * 1000.0).round() as i32).to_be_bytes());
    f[0x1A] = 0x03;
    f[0x1E] = tx.sats;
    f[0x1F] = gnss::gps_minus_utc(tx.utc, &LeapSeconds::BUILTIN).unwrap_or(0) as u8;
    f[0x20..0x22].copy_from_slice(&(week as u16).to_be_bytes());
    let h = (humidity_raw(tx.humidity, tx.temp_c) * 1000.0)
        .round()
        .max(0.0) as u32;
    f[0x32..0x35].copy_from_slice(&h.to_le_bytes()[..3]);
    let (adc, range) = thermistor_adc(tx.temp_c);
    f[0x3E] = range as u8;
    f[0x3F..0x41].copy_from_slice(&(0xA000u16 + adc).to_le_bytes());
    f[0x45..0x47].copy_from_slice(&((tx.battery_v * 1000.0).round() as u16).to_be_bytes());
    f[0x5D..0x62].copy_from_slice(&m10_serial_bytes(&tx.serial));
    seal(&mut f);
    f
}

/// Builds a 70-byte M20 frame (length 0x45), or 68 bytes with `short` (length 0x43).
pub fn m20_frame(tx: &M10Tx, counter: u8, short: bool) -> Vec<u8> {
    let len = if short { 0x43 } else { 0x45 };
    let mut f = vec![0u8; len + 1];
    f[0] = len as u8;
    f[1] = 0x20;
    let (adc, range) = thermistor_adc(tx.temp_c);
    let raw = match range {
        2 => 8192 + adc,
        1 => 4096 + adc,
        _ => adc,
    };
    f[0x02..0x04].copy_from_slice(&2000u16.to_le_bytes());
    f[0x04..0x06].copy_from_slice(&raw.to_le_bytes());
    f[0x06..0x08].copy_from_slice(&2000u16.to_le_bytes());
    let h = (tx.alt * 100.0).round() as i32;
    f[0x08..0x0B].copy_from_slice(&h.to_be_bytes()[1..]);
    let sp = |v: f64| ((v * 100.0).round().clamp(-32768.0, 32767.0) as i16).to_be_bytes();
    f[0x0B..0x0D].copy_from_slice(&sp(tx.ve));
    f[0x0D..0x0F].copy_from_slice(&sp(tx.vn));
    let gps = gnss::gps_from_utc(tx.utc, &LeapSeconds::BUILTIN).unwrap_or(gnss::WeekTime {
        week: 0,
        tow_s: 0.0,
    });
    let week = gps.week as f64;
    let tow = gps.tow_s.round() as u32;
    f[0x0F..0x12].copy_from_slice(&tow.to_be_bytes()[1..]);
    f[0x12..0x15].copy_from_slice(&m20_serial_bytes(&tx.serial));
    f[0x15] = counter;
    f[0x18..0x1A].copy_from_slice(&sp(tx.vu));
    f[0x1A..0x1C].copy_from_slice(&(week as u16).to_be_bytes());
    let deg = |d: f64| ((d * 1e6).round() as i32).to_be_bytes();
    f[0x1C..0x20].copy_from_slice(&deg(tx.lat));
    f[0x20..0x24].copy_from_slice(&deg(tx.lon));
    f[0x26] = (tx.battery_v / 3.3 * 255.0).round().clamp(0.0, 255.0) as u8;
    if !short {
        f[0x43] = 9;
    }
    seal(&mut f);
    f
}

fn gps_week_fix(mut w: i64) -> Option<i64> {
    if w > 4000 {
        return None;
    }
    if w < 1304 {
        while calendar::civil(GPS_EPOCH_UNIX + w * 604_800).year < 2018 {
            w += 1024;
        }
    }
    Some(w)
}

/// Decodes a checked M10 frame.
pub fn decode_m10(f: &[u8], t: f64) -> Option<SondeFrame> {
    let len = f[0] as usize;
    if !matches!(len, 0x64 | 0x66 | 0x76) || f.len() != len + 1 {
        return None;
    }
    let ty = f[1];
    if ty != 0x9F && ty != 0xAF {
        return None;
    }
    let ok = m10_checksum(&f[..len - 1]) == u16::from_be_bytes([f[len - 1], f[len]]);
    let mut out = SondeFrame {
        time: t,
        kind: Some(SondeType::M10),
        subtype: if ty == 0xAF { "M10+" } else { "M10" }.into(),
        ok,
        ..Default::default()
    };
    if !ok {
        return Some(out);
    }
    out.serial = Some(m10_serial(&f[0x5D..0x62]));
    if ty == 0x9F {
        let (ve, vn, vu) = (
            be16(f, 4) as f64 * 0.005,
            be16(f, 6) as f64 * 0.005,
            be16(f, 8) as f64 * 0.005,
        );
        let tow = u32::from_be_bytes([f[0x0A], f[0x0B], f[0x0C], f[0x0D]]) as f64 / 1000.0;
        let lat = be32(f, 0x0E) as f64 * 360.0 / 4_294_967_296.0;
        let lon = be32(f, 0x12) as f64 * 360.0 / 4_294_967_296.0;
        let alt = be32(f, 0x16) as f64 / 1000.0;
        out.lat = Some(lat);
        out.lon = Some(lon);
        out.alt = Some(alt);
        out.vel_v = Some(vu);
        out.vel_h = Some((ve * ve + vn * vn).sqrt());
        out.heading = Some(heading(ve, vn));
        out.sats = Some(f[0x1E] as u32);
        if let Some(w) = gps_week_fix(u16::from_be_bytes([f[0x20], f[0x21]]) as i64) {
            out.utc = Some(GPS_EPOCH_UNIX as f64 + w as f64 * 604_800.0 + tow - f[0x1F] as f64);
        }
    } else {
        let lat = i32::from_be_bytes([f[4], f[5], f[6], f[7]]) as f64 * 1e-6;
        let lon = i32::from_be_bytes([f[8], f[9], f[10], f[11]]) as f64 * 1e-6;
        let alt = be24s(f, 0x0C) as f64 / 100.0;
        let (ve, vn, vu) = (
            be16(f, 0x0F) as f64 / 100.0,
            be16(f, 0x11) as f64 / 100.0,
            be16(f, 0x13) as f64 / 100.0,
        );
        out.lat = Some(lat);
        out.lon = Some(lon);
        out.alt = Some(alt);
        out.vel_v = Some(vu);
        out.vel_h = Some((ve * ve + vn * vn).sqrt());
        out.heading = Some(heading(ve, vn));
        let (hh, mm, ss, dd, mo, yy) = (f[0x15], f[0x16], f[0x17], f[0x18], f[0x19], f[0x1A]);
        if hh < 24 && mm < 60 && ss < 61 && (1..=31).contains(&dd) && (1..=12).contains(&mo) {
            out.utc = calendar::unix(
                2000 + yy as i64,
                mo as u32,
                dd as u32,
                hh as u32,
                mm as u32,
                ss.min(59) as u32,
            )
            .map(|u| u as f64);
        }
    }
    let range = f[0x3E] as usize;
    let adc = u16::from_le_bytes([f[0x3F], f[0x40]]).wrapping_sub(0xA000) as f64;
    out.temp_c = temperature(adc, range);
    let raw = (f[0x32] as u32 | (f[0x33] as u32) << 8 | (f[0x34] as u32) << 16) as f64 / 1000.0;
    if let Some(t) = out.temp_c {
        out.humidity = Some(humidity_correct(raw, t));
    }
    out.battery_v = Some(u16::from_be_bytes([f[0x45], f[0x46]]) as f64 / 1000.0);
    out.frame = None;
    Some(out)
}

/// Decodes a checked M20 frame.
/// Decodes an M20 frame. `leap` turns its GPS time into UTC.
pub fn decode_m20(f: &[u8], t: f64, leap: &LeapSeconds) -> Option<SondeFrame> {
    let len = f[0] as usize;
    if !(len == 0x45 || len == 0x43) || f.len() != len + 1 || f[1] != 0x20 {
        return None;
    }
    let ok = m10_checksum(&f[..len - 1]) == u16::from_be_bytes([f[len - 1], f[len]]);
    let mut out = SondeFrame {
        time: t,
        kind: Some(SondeType::M20),
        subtype: "M20".into(),
        ok,
        ..Default::default()
    };
    if !ok {
        return Some(out);
    }
    out.serial = Some(m20_serial(&f[0x12..0x15]));
    let raw = u16::from_le_bytes([f[0x04], f[0x05]]);
    let range = if raw > 8191 {
        2
    } else if raw > 4095 {
        1
    } else {
        0
    };
    out.temp_c = temperature((raw & 0xFFF) as f64, range);
    out.alt = Some(be24s(f, 0x08) as f64 / 100.0);
    let (ve, vn, vu) = (
        be16(f, 0x0B) as f64 / 100.0,
        be16(f, 0x0D) as f64 / 100.0,
        be16(f, 0x18) as f64 / 100.0,
    );
    out.vel_v = Some(vu);
    out.vel_h = Some((ve * ve + vn * vn).sqrt());
    out.heading = Some(heading(ve, vn));
    let tow = ((f[0x0F] as u32) << 16 | (f[0x10] as u32) << 8 | f[0x11] as u32) as f64;
    if let Some(w) = gps_week_fix(u16::from_be_bytes([f[0x1A], f[0x1B]]) as i64) {
        out.utc = gnss::utc_from_gps(
            &gnss::WeekTime {
                week: w as u32,
                tow_s: tow,
            },
            leap,
        )
        .ok();
    }
    out.lat = Some(be32(f, 0x1C) as f64 * 1e-6);
    out.lon = Some(be32(f, 0x20) as f64 * 1e-6);
    out.battery_v = Some(f[0x26] as f64 * 3.3 / 255.0);
    out.frame = Some(f[0x15] as i64);
    Some(out)
}

/// The M10 or M20 decoder.
pub struct MeteomodemDecoder {
    kind: SondeType,
    sync: SyncWord,
    collecting: bool,
    syms: Vec<bool>,
    buf: Vec<u8>,
    want: usize,
    leap: LeapSeconds,
}

impl MeteomodemDecoder {
    fn new(kind: SondeType) -> Self {
        MeteomodemDecoder {
            kind,
            sync: sync_word(pattern_str(SYNC), 3),
            collecting: false,
            syms: Vec::with_capacity(16),
            buf: Vec::with_capacity(0x80),
            want: 0,
            leap: LeapSeconds::BUILTIN,
        }
    }
    /// The M10 decoder.
    pub fn m10() -> Self {
        Self::new(SondeType::M10)
    }
    /// The M20 decoder.
    pub fn m20() -> Self {
        Self::new(SondeType::M20)
    }
}

impl FrameDecoder for MeteomodemDecoder {
    fn kind(&self) -> SondeType {
        self.kind
    }
    fn push(&mut self, sym: bool, t: f64, out: &mut Vec<SondeFrame>) {
        if !self.collecting {
            if self.sync.push(sym as u8).is_some() {
                self.collecting = true;
                self.buf.clear();
                self.syms.clear();
                self.want = 0;
            }
            return;
        }
        self.syms.push(sym);
        if self.syms.len() < 16 {
            return;
        }
        let mut b = 0u8;
        for k in 0..8 {
            b = (b << 1) | (self.syms[2 * k] != self.syms[2 * k + 1]) as u8;
        }
        self.syms.clear();
        self.buf.push(b);
        if self.buf.len() == 2 {
            let len = self.buf[0] as usize;
            let ok_len = match self.kind {
                SondeType::M20 => (len == 0x45 || len == 0x43) && self.buf[1] == 0x20,
                _ => {
                    matches!(len, 0x64 | 0x66 | 0x76)
                        && (self.buf[1] == 0x9F || self.buf[1] == 0xAF)
                }
            };
            if !ok_len {
                self.collecting = false;
                self.sync.reset();
                return;
            }
            self.want = len + 1;
        }
        if self.want > 0 && self.buf.len() >= self.want {
            let fr = match self.kind {
                SondeType::M20 => decode_m20(&self.buf, t, &self.leap),
                _ => decode_m10(&self.buf, t),
            };
            if let Some(fr) = fr {
                out.push(fr);
            }
            self.collecting = false;
            self.sync.reset();
        }
    }
    fn set_leap_seconds(&mut self, leap: &LeapSeconds) {
        self.leap = *leap;
    }
    fn reset(&mut self, _hard: bool) {
        self.collecting = false;
        self.sync.reset();
        self.buf.clear();
        self.syms.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serials() {
        assert_eq!(m10_serial(&[0x02, 0x08, 0x3A, 0x31, 0x25]), "310-2-11329");
        assert_eq!(m10_serial(&[0x02, 0x1A, 0x74, 0xCF, 0x4B]), "704-2-23023");
        assert_eq!(m10_serial(&m10_serial_bytes("310-2-11329")), "310-2-11329");
        assert_eq!(m20_serial(&m20_serial_bytes("211-4-01234")), "211-4-01234");
    }

    #[test]
    fn frames_start_right() {
        let tx = M10Tx {
            serial: "310-2-11329".into(),
            lat: 25.1,
            lon: 55.2,
            alt: 8000.0,
            utc: 1_780_272_000.0,
            temp_c: -30.0,
            humidity: 40.0,
            battery_v: 3.1,
            sats: 9,
            ve: 10.0,
            ..Default::default()
        };
        let f = m10_frame(&tx);
        assert_eq!((f.len(), f[0], f[1], f[2]), (101, 0x64, 0x9F, 0x20));
        let d = decode_m10(&f, 0.0).unwrap();
        assert!(d.ok);
        assert!((d.temp_c.unwrap() + 30.0).abs() < 0.3);
        assert!((d.humidity.unwrap() - 40.0).abs() < 0.6);
        assert!((d.utc.unwrap() - tx.utc).abs() < 1.0);
        let g = m20_frame(
            &M10Tx {
                alt: -151.37,
                ..tx.clone()
            },
            3,
            false,
        );
        assert_eq!((g.len(), g[0], g[1]), (70, 0x45, 0x20));
        let d = decode_m20(&g, 0.0, &LeapSeconds::BUILTIN).unwrap();
        assert!(d.ok && (d.alt.unwrap() + 151.37).abs() < 0.006);
    }
}
