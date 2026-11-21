//! Vaisala RS92: the Manchester UART frames with one Reed-Solomon code word, the configuration and the calibration. The stack
//! shows no position or PTU of an RS92.

use super::frame::{pattern_str, sync_word, FrameDecoder, SondeFrame, SondeType};
use sdrkit_algo_fec::rs::{codes::RS92_234_210, Rs};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::sync::SyncWord;

/// Frame length.
pub const FRAME: usize = 240;
/// Header bytes.
pub const HEADER: [u8; 6] = [0x2A, 0x2A, 0x2A, 0x2A, 0x2A, 0x10];
/// Calibration subframes.
pub const CAL_PARTS: usize = 32;
const MSG: usize = 210;

/// The RS code word of a frame (`sdrkit_algo_fec::rs::codes::RS92_234_210`): the 24 parity bytes after the message are the
/// coefficients of x^0 to x^23 and message byte j is that of x^(24 + j).
fn load_word(frame: &[u8], w: &mut Vec<u8>) {
    w.clear();
    w.extend_from_slice(&frame[6 + MSG..6 + MSG + 24]);
    w.extend_from_slice(&frame[6..6 + MSG]);
}

fn store_word(frame: &mut [u8], w: &[u8]) {
    frame[6 + MSG..6 + MSG + 24].copy_from_slice(&w[..24]);
    frame[6..6 + MSG].copy_from_slice(&w[24..]);
}

fn push_block(f: &mut Vec<u8>, id: u8, data: &[u8]) {
    debug_assert!(data.len().is_multiple_of(2));
    f.push(id);
    f.push((data.len() / 2) as u8);
    f.extend_from_slice(data);
    f.extend_from_slice(&CRC16_IBM_3740.checksum(data).to_le_bytes());
}

/// What a transmitted RS92 frame carries.
#[derive(Clone, Debug)]
pub struct Rs92Tx {
    /// Serial.
    pub serial: String,
    /// Frame number.
    pub frame: u16,
    /// Calibration (32 × 16 bytes); subframe `frame % 32` is sent.
    pub cal: Vec<u8>,
    /// GPS time of week, ms.
    pub tow_ms: u32,
    /// Tracked satellite channels (of 12).
    pub tracked: u8,
}

/// The calibration offset of the announced frequency (u16 LE, 10 kHz steps above 400 MHz).
pub const CAL_FREQ: usize = 0x02;

/// Builds a frame (header, blocks, parity).
pub fn build_frame(tx: &Rs92Tx) -> Vec<u8> {
    let mut f = Vec::with_capacity(FRAME);
    f.extend_from_slice(&HEADER);
    let mut cfg = [0u8; 32];
    cfg[0..2].copy_from_slice(&tx.frame.to_le_bytes());
    let mut ser = [b' '; 10];
    for (d, s) in ser.iter_mut().zip(tx.serial.bytes()) {
        *d = s;
    }
    cfg[2..12].copy_from_slice(&ser);
    let part = tx.frame as usize % CAL_PARTS;
    cfg[14] = part as u8;
    if tx.cal.len() >= (part + 1) * 16 {
        cfg[15..31].copy_from_slice(&tx.cal[part * 16..part * 16 + 16]);
    }
    push_block(&mut f, 0x65, &cfg);
    push_block(&mut f, 0x69, &[0u8; 24]);
    let mut gps = [0u8; 18];
    gps[0..4].copy_from_slice(&tx.tow_ms.to_le_bytes());
    for k in 0..12 {
        gps[6 + k] = if (k as u8) < tx.tracked { 0x0F } else { 0x00 };
    }
    push_block(&mut f, 0x67, &gps);
    push_block(&mut f, 0x68, &[0u8; 4]);
    while f.len() < 6 + MSG {
        f.push(0xFF);
    }
    f.resize(FRAME, 0);
    let rs = Rs::new(RS92_234_210).expect("RS92 code");
    let mut w = Vec::with_capacity(FRAME);
    load_word(&f, &mut w);
    if rs.encode_word(&mut w).is_ok() {
        store_word(&mut f, &w);
    }
    f
}

/// The symbols on air: a preamble of alternating symbols, then each byte as UART (start 0, 8 data LSB first, stop 1), each bit
/// Manchester (0 → 10, 1 → 01).
pub fn frame_symbols(frame: &[u8], out: &mut Vec<bool>) {
    for i in 0..160 {
        out.push(i % 2 == 0);
    }
    for &b in frame {
        let mut bits = [false; 10];
        bits[9] = true;
        for k in 0..8 {
            bits[1 + k] = (b >> k) & 1 == 1;
        }
        for bit in bits {
            if bit {
                out.push(false);
                out.push(true);
            } else {
                out.push(true);
                out.push(false);
            }
        }
    }
}

fn header_pattern() -> (u64, u32) {
    // The last three header bytes 2A 2A 10 as symbols (60 symbols).
    let mut s = String::new();
    for &b in &HEADER[3..] {
        let mut bits = [false; 10];
        bits[9] = true;
        for k in 0..8 {
            bits[1 + k] = (b >> k) & 1 == 1;
        }
        for bit in bits {
            s.push_str(if bit { "01" } else { "10" });
        }
    }
    pattern_str(&s)
}

/// The RS92 decoder.
pub struct Rs92Decoder {
    sync: SyncWord,
    inverted: bool,
    collecting: bool,
    syms: Vec<bool>,
    buf: Vec<u8>,
    rs: Rs,
    word: Vec<u8>,
    cal_serial: String,
    cal_have: [bool; CAL_PARTS],
    cal: [u8; CAL_PARTS * 16],
}

impl Default for Rs92Decoder {
    fn default() -> Self {
        Rs92Decoder {
            sync: sync_word(header_pattern(), 6),
            inverted: false,
            collecting: false,
            syms: Vec::with_capacity(20),
            buf: Vec::with_capacity(FRAME),
            rs: Rs::new(RS92_234_210).expect("RS92 code"),
            word: Vec::with_capacity(255),
            cal_serial: String::new(),
            cal_have: [false; CAL_PARTS],
            cal: [0; CAL_PARTS * 16],
        }
    }
}

impl Rs92Decoder {
    /// Decodes a whole frame (header included).
    pub fn decode_frame(&mut self, raw: &[u8], t: f64) -> SondeFrame {
        let mut out = SondeFrame {
            time: t,
            kind: Some(SondeType::Rs92),
            subtype: "RS92".into(),
            ..Default::default()
        };
        let mut f = raw.to_vec();
        f.resize(FRAME, 0);
        load_word(&f, &mut self.word);
        let Ok(n) = self.rs.decode(&mut self.word, &[]) else {
            return out;
        };
        store_word(&mut f, &self.word);
        out.corrections = n.corrected;
        let mut pos = 6;
        let (mut good, mut bad, mut cfg_ok) = (0, 0, false);
        while pos + 2 <= 6 + MSG {
            let id = f[pos];
            let len = f[pos + 1] as usize * 2;
            if !(0x65..=0x69).contains(&id) || pos + 2 + len + 2 > 6 + MSG {
                break;
            }
            let d = &f[pos + 2..pos + 2 + len];
            let crc = u16::from_le_bytes([f[pos + 2 + len], f[pos + 3 + len]]);
            if CRC16_IBM_3740.checksum(d) != crc {
                bad += 1;
                pos += len + 4;
                continue;
            }
            good += 1;
            match id {
                0x65 if len >= 31 => {
                    cfg_ok = true;
                    out.frame = Some(u16::from_le_bytes([d[0], d[1]]) as i64);
                    let serial: String = d[2..12]
                        .iter()
                        .filter(|&&c| (0x21..0x7F).contains(&c))
                        .map(|&c| c as char)
                        .collect();
                    if serial != self.cal_serial {
                        self.cal_serial = serial.clone();
                        self.cal_have = [false; CAL_PARTS];
                    }
                    let part = d[14] as usize;
                    if part < CAL_PARTS {
                        self.cal_have[part] = true;
                        self.cal[part * 16..part * 16 + 16].copy_from_slice(&d[15..31]);
                    }
                    out.serial = Some(serial);
                }
                0x67 if len >= 18 => {
                    let tow = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
                    out.gps_tow = Some(tow as f64 / 1000.0);
                    out.sats = Some(d[6..18].iter().filter(|&&s| s & 0x0F != 0).count() as u32);
                }
                _ => {}
            }
            pos += len + 4;
        }
        out.cal = Some((
            self.cal_have.iter().filter(|&&h| h).count() as u32,
            CAL_PARTS as u32,
        ));
        if self.cal_have[0] {
            let v = u16::from_le_bytes([self.cal[CAL_FREQ], self.cal[CAL_FREQ + 1]]);
            out.announced_hz = Some(400e6 + 10e3 * v as f64);
        }
        out.ok = cfg_ok && good >= 3 && bad == 0;
        out
    }
}

impl FrameDecoder for Rs92Decoder {
    fn kind(&self) -> SondeType {
        SondeType::Rs92
    }
    fn push(&mut self, sym: bool, t: f64, out: &mut Vec<SondeFrame>) {
        if !self.collecting {
            if let Some(inv) = self.sync.push(sym as u8).map(|h| h.inverted()) {
                self.inverted = inv;
                self.collecting = true;
                self.buf.clear();
                self.buf.extend_from_slice(&HEADER);
                self.syms.clear();
            }
            return;
        }
        self.syms.push(sym ^ self.inverted);
        if self.syms.len() < 20 {
            return;
        }
        let mut b = 0u8;
        for k in 0..8 {
            let (a, c) = (self.syms[2 + 2 * k], self.syms[3 + 2 * k]);
            // 01 → 1, 10 → 0; a violation (00, 11) takes the second half.
            let _ = a;
            b |= (c as u8) << k;
        }
        self.syms.clear();
        self.buf.push(b);
        if self.buf.len() >= FRAME {
            let f = std::mem::take(&mut self.buf);
            let fr = self.decode_frame(&f, t);
            out.push(fr);
            self.buf = f;
            self.buf.clear();
            self.collecting = false;
            self.sync.reset();
        }
    }
    fn reset(&mut self, _hard: bool) {
        self.collecting = false;
        self.sync.reset();
        self.buf.clear();
        self.syms.clear();
    }
}
