//! Vaisala RS41: the whitened frames with their two Reed-Solomon code words, the subframe blocks, the calibration and the PTU
//! sensors.

use super::frame::{heading, pattern_lsb_first, sync_word, FrameDecoder, SondeFrame, SondeType};
use sdrkit_algo_fec::rs::{codes::RS41_255_231, Rs};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_geo::ellipsoid::{Ecef, Observer};
use sdrkit_util_geo::time::{gnss, LeapSeconds};

/// The frame header before whitening.
pub const HEADER: [u8; 8] = [0x86, 0x35, 0xF4, 0x40, 0x93, 0xDF, 0x1A, 0x60];

/// The 64-byte whitening sequence XORed onto every frame byte (repeated).
pub const WHITENING: [u8; 64] = [
    0x96, 0x83, 0x3E, 0x51, 0xB1, 0x49, 0x08, 0x98, 0x32, 0x05, 0x59, 0x0E, 0xF9, 0x44, 0xC6, 0x26,
    0x21, 0x60, 0xC2, 0xEA, 0x79, 0x5D, 0x6D, 0xA1, 0x54, 0x69, 0x47, 0x0C, 0xDC, 0xE8, 0x5C, 0xF1,
    0xF7, 0x76, 0x82, 0x7F, 0x07, 0x99, 0xA2, 0x2C, 0x93, 0x7C, 0x30, 0x63, 0xF5, 0x10, 0x2E, 0x61,
    0xD0, 0xBC, 0xB4, 0xB6, 0x06, 0xAA, 0xF4, 0x23, 0x78, 0x6E, 0x3B, 0xAE, 0xBF, 0x7B, 0x4C, 0xC1,
];

/// Standard frame length.
pub const FRAME_STD: usize = 320;
/// Extended frame length (byte 0x38 = 0xF0).
pub const FRAME_EXT: usize = 518;
/// Calibration subframes.
pub const CAL_PARTS: usize = 51;

const PARITY_POS: usize = 0x08;
const DATA_POS: usize = 0x38;

/// XORs the whitening sequence onto `frame` (it is its own inverse).
pub fn whiten(frame: &mut [u8]) {
    for (i, b) in frame.iter_mut().enumerate() {
        *b ^= WHITENING[i % 64];
    }
}

/// The Reed-Solomon code words of a de-whitened frame, for both frame lengths. Code word `c` holds parity bytes 0x08 + 24c + i
/// as the coefficients of x^i and data bytes 0x38 + 2j + c as those of x^(24 + j) (`sdrkit_algo_fec::rs::codes::RS41_255_231`,
/// shortened for the standard frame).
pub struct FrameRs {
    std: Rs,
    ext: Rs,
    word: Vec<u8>,
}

impl FrameRs {
    /// The decoders of the standard and the extended frame.
    pub fn new() -> FrameRs {
        let k_std = (FRAME_STD - DATA_POS) / 2;
        FrameRs {
            std: Rs::new(RS41_255_231.shortened((24 + k_std) as u16)).expect("RS41 code"),
            ext: Rs::new(RS41_255_231).expect("RS41 code"),
            word: Vec::with_capacity(255),
        }
    }

    fn code_for(&mut self, len: usize) -> Option<&mut Rs> {
        match len {
            FRAME_STD => Some(&mut self.std),
            FRAME_EXT => Some(&mut self.ext),
            _ => None,
        }
    }

    fn load(word: &mut Vec<u8>, frame: &[u8], c: usize) {
        let k = (frame.len() - DATA_POS) / 2;
        word.clear();
        word.extend_from_slice(&frame[PARITY_POS + 24 * c..PARITY_POS + 24 * c + 24]);
        word.extend((0..k).map(|j| frame[DATA_POS + 2 * j + c]));
    }

    fn store(word: &[u8], frame: &mut [u8], c: usize) {
        let k = (frame.len() - DATA_POS) / 2;
        frame[PARITY_POS + 24 * c..PARITY_POS + 24 * c + 24].copy_from_slice(&word[..24]);
        for j in 0..k {
            frame[DATA_POS + 2 * j + c] = word[24 + j];
        }
    }

    /// Fills in the parity of both code words of a de-whitened frame of 320 or 518 bytes. Other lengths are left as they are.
    pub fn fill_parity(&mut self, frame: &mut [u8]) {
        let mut word = std::mem::take(&mut self.word);
        if let Some(rs) = self.code_for(frame.len()) {
            for c in 0..2 {
                Self::load(&mut word, frame, c);
                if rs.encode_word(&mut word).is_ok() {
                    Self::store(&word, frame, c);
                }
            }
        }
        self.word = word;
    }

    /// Corrects both code words of a de-whitened frame in place. Returns the symbols corrected, or `None` when a code word is
    /// beyond repair or the frame length is not 320 or 518 bytes.
    pub fn correct(&mut self, frame: &mut [u8]) -> Option<u32> {
        let mut word = std::mem::take(&mut self.word);
        let mut total = Some(0);
        match self.code_for(frame.len()) {
            Some(rs) => {
                for c in 0..2 {
                    Self::load(&mut word, frame, c);
                    match rs.decode(&mut word, &[]) {
                        Ok(d) => {
                            total = total.map(|t| t + d.corrected);
                            Self::store(&word, frame, c);
                        }
                        Err(_) => {
                            total = None;
                            break;
                        }
                    }
                }
            }
            None => total = None,
        }
        self.word = word;
        total
    }
}

impl Default for FrameRs {
    fn default() -> Self {
        FrameRs::new()
    }
}

/// Appends a subframe block: id, length, data, CRC-16 (LE).
pub fn push_block(frame: &mut Vec<u8>, id: u8, data: &[u8]) {
    frame.push(id);
    frame.push(data.len() as u8);
    frame.extend_from_slice(data);
    let c = CRC16_IBM_3740.checksum(data);
    frame.extend_from_slice(&c.to_le_bytes());
}

/// What a transmitted frame carries.
#[derive(Clone, Debug)]
pub struct Rs41Tx {
    /// Serial (8 characters).
    pub serial: String,
    /// Frame counter.
    pub frame: u16,
    /// Battery, V.
    pub battery_v: f64,
    /// The whole calibration block (51 × 16 bytes); subframe `frame % 51` is sent.
    pub cal: Vec<u8>,
    /// GPS week and time of week (ms).
    pub week: u16,
    /// Time of week, ms.
    pub tow_ms: u32,
    /// ECEF position, m.
    pub pos: [f64; 3],
    /// ECEF velocity, m/s.
    pub vel: [f64; 3],
    /// Satellites.
    pub sats: u8,
    /// The 12 PTU measurement counts.
    pub ptu: [u32; 12],
    /// Make the extended 518-byte frame.
    pub extended: bool,
}

/// Builds a whole frame (de-whitened, with parity): the header, the blocks, padding.
pub fn build_frame(tx: &Rs41Tx) -> Vec<u8> {
    let len = if tx.extended { FRAME_EXT } else { FRAME_STD };
    let mut f = Vec::with_capacity(len);
    f.extend_from_slice(&HEADER);
    f.resize(DATA_POS, 0);
    f.push(if tx.extended { 0xF0 } else { 0x0F });
    // Status 0x79.
    let mut st = [0u8; 40];
    st[0..2].copy_from_slice(&tx.frame.to_le_bytes());
    let mut ser = [b' '; 8];
    for (d, s) in ser.iter_mut().zip(tx.serial.bytes()) {
        *d = s;
    }
    st[2..10].copy_from_slice(&ser);
    st[10] = (tx.battery_v * 10.0).round().clamp(0.0, 255.0) as u8;
    let part = tx.frame as usize % CAL_PARTS;
    st[23] = part as u8;
    if tx.cal.len() >= (part + 1) * 16 {
        st[24..40].copy_from_slice(&tx.cal[part * 16..part * 16 + 16]);
    }
    push_block(&mut f, 0x79, &st);
    // PTU 0x7A.
    let mut ptu = [0u8; 42];
    for (i, &v) in tx.ptu.iter().enumerate() {
        ptu[3 * i..3 * i + 3].copy_from_slice(&v.to_le_bytes()[..3]);
    }
    push_block(&mut f, 0x7A, &ptu);
    // GPS info 0x7C.
    let mut gi = [0u8; 30];
    gi[0..2].copy_from_slice(&tx.week.to_le_bytes());
    gi[2..6].copy_from_slice(&tx.tow_ms.to_le_bytes());
    for s in 0..tx.sats.min(12) as usize {
        gi[6 + 2 * s] = 3 + s as u8;
        gi[7 + 2 * s] = 40;
    }
    push_block(&mut f, 0x7C, &gi);
    // Raw GPS 0x7D (not decoded).
    push_block(&mut f, 0x7D, &[0u8; 89]);
    // GPS position 0x7B.
    let mut gp = [0u8; 21];
    for k in 0..3 {
        let cm = (tx.pos[k] * 100.0).round() as i32;
        gp[4 * k..4 * k + 4].copy_from_slice(&cm.to_le_bytes());
        let v = (tx.vel[k] * 100.0).round().clamp(-32768.0, 32767.0) as i16;
        gp[12 + 2 * k..14 + 2 * k].copy_from_slice(&v.to_le_bytes());
    }
    gp[18] = tx.sats;
    gp[19] = 1;
    gp[20] = 15;
    push_block(&mut f, 0x7B, &gp);
    if tx.extended {
        let n = FRAME_EXT - f.len() - 4;
        push_block(&mut f, 0x7E, &vec![0x11u8; n.min(255)]);
    }
    // Padding 0x76 to the end.
    let left = len - f.len();
    if left >= 4 {
        push_block(&mut f, 0x76, &vec![0u8; left - 4]);
    }
    f.resize(len, 0);
    FrameRs::new().fill_parity(&mut f);
    f
}

/// RS41 calibration constants written into a calibration block by the generator, and read by the decoder.
pub mod cal {
    /// Offset of the first calibration resistance (f32 LE).
    pub const RF1: usize = 61;
    /// Offset of the second calibration resistance.
    pub const RF2: usize = 65;
    /// Offset of the three polynomial coefficients.
    pub const CO: usize = 77;
    /// Offset of the three corrections.
    pub const CA: usize = 89;
    /// Offset of the humidity constant H0.
    pub const H0: usize = 117;
    /// Offset of the burst-kill timer (u16 LE).
    pub const KILL: usize = 39;
    /// The byte that marks the pressure subtype.
    pub const SGP: usize = 0x21F;
}

/// Calibration collected for one serial.
#[derive(Clone, Debug)]
struct CalStore {
    serial: String,
    have: [bool; CAL_PARTS],
    data: [u8; CAL_PARTS * 16],
}

impl CalStore {
    fn new(serial: &str) -> CalStore {
        CalStore {
            serial: serial.into(),
            have: [false; CAL_PARTS],
            data: [0; CAL_PARTS * 16],
        }
    }
    fn count(&self) -> u32 {
        self.have.iter().filter(|&&h| h).count() as u32
    }
    fn f32_at(&self, off: usize) -> f64 {
        f32::from_le_bytes([
            self.data[off],
            self.data[off + 1],
            self.data[off + 2],
            self.data[off + 3],
        ]) as f64
    }
    fn parts(&self, r: std::ops::RangeInclusive<usize>) -> bool {
        r.into_iter().all(|i| self.have[i])
    }
}

/// Temperature from the counts and a calibration. `None` outside −150…+100 °C.
pub fn temperature(
    f: f64,
    f1: f64,
    f2: f64,
    rf1: f64,
    rf2: f64,
    co: [f64; 3],
    ca: [f64; 3],
) -> Option<f64> {
    if f2 == f1 || rf2 == rf1 {
        return None;
    }
    let g = (f2 - f1) / (rf2 - rf1);
    let rb = (f1 * rf2 - f2 * rf1) / (f2 - f1);
    let r = (f / g - rb) * ca[0];
    let t = (co[0] + co[1] * r + co[2] * r * r + ca[1]) * (1.0 + ca[2]);
    (-150.0..=100.0).contains(&t).then_some(t)
}

/// Humidity from the counts, H0 and the temperature, clamped to 0–100 %.
pub fn humidity(f: f64, f1: f64, f2: f64, h0: f64, t: f64) -> Option<f64> {
    if f2 == f1 || h0 == 0.0 {
        return None;
    }
    let a1 = 350.0 / h0;
    let fh = (f - f1) / (f2 - f1);
    let mut rh = 100.0 * (a1 * fh - 7.5) - t / 5.5;
    if t < -20.0 {
        rh *= 1.0 + (-20.0 - t) / 100.0;
    }
    if t < -40.0 {
        rh *= 1.0 + (-40.0 - t) / 120.0;
    }
    Some(rh.clamp(0.0, 100.0))
}

/// The RS41 frame decoder.
pub struct Rs41Decoder {
    sync: SyncWord,
    inverted: bool,
    collecting: bool,
    bits: u32,
    byte: u8,
    buf: Vec<u8>,
    want: usize,
    rs: FrameRs,
    leap: LeapSeconds,
    cal: Option<CalStore>,
}

impl Default for Rs41Decoder {
    fn default() -> Self {
        let mut h = HEADER;
        whiten(&mut h);
        Rs41Decoder {
            sync: sync_word(pattern_lsb_first(&h), 5),
            inverted: false,
            collecting: false,
            bits: 0,
            byte: 0,
            buf: Vec::with_capacity(FRAME_EXT),
            want: FRAME_STD,
            rs: FrameRs::new(),
            leap: LeapSeconds::BUILTIN,
            cal: None,
        }
    }
}

impl Rs41Decoder {
    /// Decodes a whole whitened frame (as received, header included).
    pub fn decode_frame(&mut self, raw: &[u8], t: f64) -> SondeFrame {
        let mut f = raw.to_vec();
        whiten(&mut f);
        self.decode_plain(&mut f, t)
    }

    /// Decodes a de-whitened frame in place.
    pub fn decode_plain(&mut self, f: &mut [u8], t: f64) -> SondeFrame {
        let mut out = SondeFrame {
            time: t,
            kind: Some(SondeType::Rs41),
            subtype: "RS41-SG".into(),
            ..Default::default()
        };
        let Some(fixed) = self.rs.correct(f) else {
            return out;
        };
        out.corrections = fixed;
        let mut pos = DATA_POS + 1;
        let (mut good, mut bad) = (0, 0);
        let mut status_ok = false;
        let mut ptu: Option<[f64; 12]> = None;
        let mut gps_time = None;
        while pos + 2 <= f.len() {
            let id = f[pos];
            let len = f[pos + 1] as usize;
            if !(0x76..=0x8F).contains(&id) || pos + 2 + len + 2 > f.len() {
                break;
            }
            let data = &f[pos + 2..pos + 2 + len];
            let crc = u16::from_le_bytes([f[pos + 2 + len], f[pos + 3 + len]]);
            if CRC16_IBM_3740.checksum(data) != crc {
                bad += 1;
                pos += len + 4;
                continue;
            }
            good += 1;
            match id {
                0x79 if len >= 40 => {
                    status_ok = true;
                    out.frame = Some(u16::from_le_bytes([data[0], data[1]]) as i64);
                    let serial: String = data[2..10]
                        .iter()
                        .map(|&c| {
                            if (0x20..0x7F).contains(&c) {
                                c as char
                            } else {
                                '?'
                            }
                        })
                        .collect::<String>()
                        .trim_end()
                        .to_string();
                    out.battery_v = Some(data[10] as f64 / 10.0);
                    let part = data[23] as usize;
                    if self
                        .cal
                        .as_ref()
                        .map(|c| c.serial != serial)
                        .unwrap_or(true)
                    {
                        self.cal = Some(CalStore::new(&serial));
                    }
                    if part < CAL_PARTS {
                        let c = self.cal.get_or_insert_with(|| CalStore::new(&serial));
                        c.have[part] = true;
                        c.data[part * 16..part * 16 + 16].copy_from_slice(&data[24..40]);
                    }
                    out.serial = Some(serial);
                }
                0x7A if len >= 36 => {
                    let mut v = [0f64; 12];
                    for (i, x) in v.iter_mut().enumerate() {
                        *x = (data[3 * i] as u32
                            | (data[3 * i + 1] as u32) << 8
                            | (data[3 * i + 2] as u32) << 16) as f64;
                    }
                    ptu = Some(v);
                }
                0x7C if len >= 6 => {
                    let week = u16::from_le_bytes([data[0], data[1]]) as i64;
                    let tow =
                        u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as f64 / 1000.0;
                    gps_time = gnss::utc_from_gps(
                        &gnss::WeekTime {
                            week: week as u32,
                            tow_s: tow,
                        },
                        &self.leap,
                    )
                    .ok();
                }
                0x7B if len >= 19 => {
                    let i32at = |o: usize| {
                        i32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as f64
                    };
                    let i16at = |o: usize| i16::from_le_bytes([data[o], data[o + 1]]) as f64;
                    let x = [i32at(0) / 100.0, i32at(4) / 100.0, i32at(8) / 100.0];
                    let r = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
                    let at = Ecef::new(x[0], x[1], x[2]).to_position();
                    if let (true, Ok(p)) = (r > 6_300_000.0 && r < 6_500_000.0, at) {
                        let v = [i16at(12) / 100.0, i16at(14) / 100.0, i16at(16) / 100.0];
                        let enu = Observer::new(p).rotate_to_enu(v);
                        let (e, n, u) = (enu.east_m, enu.north_m, enu.up_m);
                        out.lat = Some(p.lat_deg);
                        out.lon = Some(p.lon_deg);
                        out.alt = Some(p.alt_m);
                        out.vel_v = Some(u);
                        out.vel_h = Some((e * e + n * n).sqrt());
                        out.heading = Some(heading(e, n));
                        out.sats = Some(data[18] as u32);
                    }
                }
                _ => {}
            }
            pos += len + 4;
        }
        out.utc = gps_time;
        if let Some(c) = &self.cal {
            out.cal = Some((c.count(), CAL_PARTS as u32));
            if c.have[0] {
                out.announced_hz =
                    Some(400e6 + 40e3 * c.data[3] as f64 + 10e3 * (c.data[2] >> 6) as f64);
            }
            if c.have[2] {
                let k = u16::from_le_bytes([c.data[cal::KILL], c.data[cal::KILL + 1]]);
                if k != 0xFFFF {
                    out.kill_timer_s = Some(k as f64);
                }
            }
            if c.have[33] && c.data[cal::SGP] == b'P' {
                out.subtype = "RS41-SGP".into();
            }
            if let Some(m) = ptu {
                if c.parts(3..=6) {
                    let co = [
                        c.f32_at(cal::CO),
                        c.f32_at(cal::CO + 4),
                        c.f32_at(cal::CO + 8),
                    ];
                    let ca = [
                        c.f32_at(cal::CA),
                        c.f32_at(cal::CA + 4),
                        c.f32_at(cal::CA + 8),
                    ];
                    let (rf1, rf2) = (c.f32_at(cal::RF1), c.f32_at(cal::RF2));
                    out.temp_c = temperature(m[0], m[1], m[2], rf1, rf2, co, ca);
                    if let (Some(t), true) = (out.temp_c, c.have[7]) {
                        out.humidity = humidity(m[3], m[4], m[5], c.f32_at(cal::H0), t);
                    }
                }
            }
        }
        out.ok = status_ok && good >= 3 && bad == 0;
        out
    }
}

impl FrameDecoder for Rs41Decoder {
    fn kind(&self) -> SondeType {
        SondeType::Rs41
    }
    fn push(&mut self, sym: bool, t: f64, out: &mut Vec<SondeFrame>) {
        if !self.collecting {
            if let Some(inv) = self.sync.push(sym as u8).map(|h| h.inverted()) {
                self.inverted = inv;
                self.collecting = true;
                self.buf.clear();
                let mut h = HEADER;
                whiten(&mut h);
                self.buf.extend_from_slice(&h);
                self.bits = 0;
                self.byte = 0;
                self.want = FRAME_STD;
            }
            return;
        }
        let b = sym ^ self.inverted;
        self.byte |= (b as u8) << self.bits;
        self.bits += 1;
        if self.bits < 8 {
            return;
        }
        self.buf.push(self.byte);
        self.bits = 0;
        self.byte = 0;
        if self.buf.len() == DATA_POS + 1 {
            let ft = self.buf[DATA_POS] ^ WHITENING[DATA_POS % 64];
            if ft == 0xF0 {
                self.want = FRAME_EXT;
            }
        }
        if self.buf.len() >= self.want {
            let mut f = std::mem::take(&mut self.buf);
            whiten(&mut f);
            let fr = self.decode_plain(&mut f, t);
            out.push(fr);
            self.buf = f;
            self.buf.clear();
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
    }
}

/// The bits on air for a frame: a 320-bit preamble of alternating bits then the whitened frame, LSB first.
pub fn frame_bits(frame: &[u8], out: &mut Vec<bool>) {
    for i in 0..320 {
        out.push(i % 2 == 1);
    }
    let mut f = frame.to_vec();
    whiten(&mut f);
    for b in f {
        for k in 0..8 {
            out.push((b >> k) & 1 == 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitened_header_is_the_known_sync() {
        let mut h = HEADER;
        whiten(&mut h);
        assert_eq!(h, [0x10, 0xB6, 0xCA, 0x11, 0x22, 0x96, 0x12, 0xF8]);
        assert_eq!(WHITENING[15], 0x26);
        assert_eq!(WHITENING[63], 0xC1);
    }
}
