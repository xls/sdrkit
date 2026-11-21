//! Lockheed Martin LMS6-1680: the UART frames with their GPS and configuration subframes.

use super::frame::{pattern_str, sync_word, FrameDecoder, SondeFrame, SondeType};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::sync::SyncWord;

const GPS_LEN: usize = 28;
const CFG_LEN: usize = 16;

/// What one LMS6 second carries.
#[derive(Clone, Debug, Default)]
pub struct Lms6Tx {
    /// Serial number.
    pub serial: u32,
    /// Frame counter.
    pub frame: u32,
    /// GPS time of week, ms.
    pub tow_ms: u32,
    /// Latitude, longitude °, altitude m.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Altitude.
    pub alt: f64,
    /// East, north, up m/s.
    pub ve: f64,
    /// North.
    pub vn: f64,
    /// Up.
    pub vu: f64,
}

fn subframe(ty: u8, payload: &[u8], out: &mut Vec<u8>) {
    let start = out.len();
    out.extend_from_slice(&[0xCA, 0xCA, 0xCA, 0xCA, 0x24, 0x52, ty]);
    out.extend_from_slice(payload);
    let c = CRC16_IBM_3740.checksum(&out[start + 4..]);
    out.extend_from_slice(&c.to_be_bytes());
}

/// The bytes of one second: a GPS subframe and a configuration subframe.
pub fn second_bytes(tx: &Lms6Tx) -> Vec<u8> {
    let mut g = Vec::with_capacity(GPS_LEN);
    g.extend_from_slice(&tx.frame.to_be_bytes());
    g.extend_from_slice(&tx.tow_ms.to_be_bytes());
    let deg = |d: f64| ((d * 4_294_967_296.0 / 360.0).round() as i64 as i32).to_be_bytes();
    g.extend_from_slice(&deg(tx.lat));
    g.extend_from_slice(&deg(tx.lon));
    g.extend_from_slice(&((tx.alt * 1000.0).round() as i32).to_be_bytes());
    for v in [tx.ve, tx.vn, tx.vu] {
        g.extend_from_slice(&((v * 100.0).round().clamp(-32768.0, 32767.0) as i16).to_be_bytes());
    }
    let hd = super::frame::heading(tx.ve, tx.vn);
    g.extend_from_slice(&((hd * 100.0).round() as u16).to_be_bytes());
    let mut out = Vec::new();
    subframe(0x54, &g, &mut out);
    let mut c = [0u8; CFG_LEN];
    c[0..4].copy_from_slice(&tx.serial.to_be_bytes());
    subframe(0x4D, &c, &mut out);
    out
}

/// UART 8N1 bits of bytes (start 0, 8 data LSB first, stop 1).
pub fn uart_bits(bytes: &[u8], out: &mut Vec<bool>) {
    for &b in bytes {
        out.push(false);
        for k in 0..8 {
            out.push((b >> k) & 1 == 1);
        }
        out.push(true);
    }
}

/// The LMS6 decoder.
pub struct Lms6Decoder {
    sync: SyncWord,
    inverted: bool,
    collecting: bool,
    bits: Vec<bool>,
    buf: Vec<u8>,
    ty: u8,
    serial: Option<u32>,
}

impl Default for Lms6Decoder {
    fn default() -> Self {
        let mut b = Vec::new();
        uart_bits(&[0xCA, 0xCA, 0x24, 0x52], &mut b);
        let s: String = b.iter().map(|&x| if x { '1' } else { '0' }).collect();
        Lms6Decoder {
            sync: sync_word(pattern_str(&s), 3),
            inverted: false,
            collecting: false,
            bits: Vec::with_capacity(10),
            buf: Vec::with_capacity(64),
            ty: 0,
            serial: None,
        }
    }
}

impl Lms6Decoder {
    fn finish(&mut self, t: f64, out: &mut Vec<SondeFrame>) {
        let n = self.buf.len();
        let crc = u16::from_be_bytes([self.buf[n - 2], self.buf[n - 1]]);
        let mut whole = vec![0x24, 0x52, self.ty];
        whole.extend_from_slice(&self.buf[..n - 2]);
        let ok = CRC16_IBM_3740.checksum(&whole) == crc;
        let p = &self.buf[..n - 2];
        if self.ty == 0x4D {
            if ok {
                self.serial = Some(u32::from_be_bytes([p[0], p[1], p[2], p[3]]));
            }
            return;
        }
        let mut f = SondeFrame {
            time: t,
            kind: Some(SondeType::Lms6),
            subtype: "LMS6-1680".into(),
            serial: self.serial.map(|s| format!("LMS6-{s}")),
            ok,
            ..Default::default()
        };
        if ok {
            let be32 = |o: usize| i32::from_be_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
            let be16 = |o: usize| i16::from_be_bytes([p[o], p[o + 1]]) as f64 / 100.0;
            f.frame = Some(be32(0) as u32 as i64);
            f.gps_tow = Some(be32(4) as u32 as f64 / 1000.0);
            f.lat = Some(be32(8) as f64 * 360.0 / 4_294_967_296.0);
            f.lon = Some(be32(12) as f64 * 360.0 / 4_294_967_296.0);
            f.alt = Some(be32(16) as f64 / 1000.0);
            let (ve, vn, vu) = (be16(20), be16(22), be16(24));
            f.vel_v = Some(vu);
            f.vel_h = Some((ve * ve + vn * vn).sqrt());
            f.heading = Some(u16::from_be_bytes([p[26], p[27]]) as f64 / 100.0);
        }
        out.push(f);
    }
}

impl FrameDecoder for Lms6Decoder {
    fn kind(&self) -> SondeType {
        SondeType::Lms6
    }
    fn push(&mut self, sym: bool, t: f64, out: &mut Vec<SondeFrame>) {
        if !self.collecting {
            if let Some(inv) = self.sync.push(sym as u8).map(|h| h.inverted()) {
                self.inverted = inv;
                self.collecting = true;
                self.buf.clear();
                self.bits.clear();
                self.ty = 0;
            }
            return;
        }
        self.bits.push(sym ^ self.inverted);
        if self.bits.len() < 10 {
            return;
        }
        let mut b = 0u8;
        for k in 0..8 {
            b |= (self.bits[1 + k] as u8) << k;
        }
        let framing = !self.bits[0] && self.bits[9];
        self.bits.clear();
        if self.ty == 0 {
            if !framing || (b != 0x54 && b != 0x4D) {
                self.collecting = false;
                self.sync.reset();
                return;
            }
            self.ty = b;
            return;
        }
        self.buf.push(b);
        let want = if self.ty == 0x54 { GPS_LEN } else { CFG_LEN } + 2;
        if self.buf.len() >= want {
            self.finish(t, out);
            self.collecting = false;
            self.sync.reset();
        }
    }
    fn reset(&mut self, _hard: bool) {
        self.collecting = false;
        self.sync.reset();
        self.buf.clear();
        self.bits.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_round_trip() {
        let tx = Lms6Tx {
            serial: 77668800,
            frame: 5,
            tow_ms: 1000,
            lat: 46.77,
            lon: -100.77,
            alt: 12000.0,
            ve: 5.0,
            vn: 0.0,
            vu: 5.0,
        };
        let mut bits = Vec::new();
        for _ in 0..2 {
            uart_bits(&second_bytes(&tx), &mut bits);
        }
        let mut d = Lms6Decoder::default();
        let mut out = Vec::new();
        for b in bits {
            d.push(b, 0.0, &mut out);
        }
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[1].ok);
        assert_eq!(out[1].serial.as_deref(), Some("LMS6-77668800"));
        assert!((out[1].lat.unwrap() - 46.77).abs() < 2e-5);
        assert!((out[1].alt.unwrap() - 12000.0).abs() < 0.01);
        assert_eq!(out[1].frame, Some(5));
    }
}
