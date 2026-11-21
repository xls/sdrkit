//! Graw DFM-06, DFM-09 and DFM-17: the interleaved Hamming (8,4) frames, their GPS data and the measurement channels.

use super::frame::{pattern_str, steinhart_c, sync_word, FrameDecoder, SondeFrame, SondeType};
use crate::calendar;
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::sync::SyncWord;
use std::sync::OnceLock;

/// The check bits of each data bit of the Hamming (8,4) code, the first check bit most significant (the rows of `G = [I | P]`).
pub const CHECK_ROWS: [u32; 4] = [0x7, 0xB, 0xD, 0xE];
/// The header bits.
pub const HEADER: u16 = 0x45CF;
/// Symbols per frame.
pub const FRAME_SYMBOLS: usize = 560;
/// Thermistor fit (Epcos B57540G0502).
pub const THERMISTOR: [f64; 4] = [1.09698417e-3, 2.39564629e-4, 2.48821437e-6, 5.84354921e-8];

/// The Hamming (8,4) code of the frames: it corrects one error and detects two.
pub fn hamming() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(|| SmallCode::from_parity_rows(8, 4, &CHECK_ROWS).expect("DFM Hamming (8,4)"))
}

/// Encodes a nibble (MSB first) into a code word of 8 bits (bit 0 sent first).
pub fn hamming_encode(nibble: u8) -> [bool; 8] {
    let w = hamming().encode(nibble as u128);
    std::array::from_fn(|k| (w >> (7 - k)) & 1 == 1)
}

/// Decodes a code word (bit 0 sent first). Returns the nibble and whether a bit was corrected, or `None` for a detected double
/// error.
pub fn hamming_decode(bits: [bool; 8]) -> Option<(u8, bool)> {
    let w = bits.iter().fold(0u128, |w, &b| (w << 1) | b as u128);
    let (cw, fixed) = hamming().decode(w)?;
    Some((hamming().info(cw) as u8, fixed > 0))
}

/// Encodes nibbles into an interleaved block (bit j of codeword i at position L·j + i).
pub fn encode_block(nibbles: &[u8], out: &mut Vec<bool>) {
    let l = nibbles.len();
    let start = out.len();
    out.resize(start + 8 * l, false);
    for (i, &n) in nibbles.iter().enumerate() {
        let cw = hamming_encode(n);
        for (j, &b) in cw.iter().enumerate() {
            out[start + l * j + i] = b;
        }
    }
}

/// Decodes an interleaved block of `l` codewords: the nibbles and the number of corrections, or `None` on a detected error.
pub fn decode_block(bits: &[bool], l: usize, nib: &mut [u8]) -> Option<usize> {
    let mut fixes = 0;
    for (i, n) in nib.iter_mut().enumerate().take(l) {
        let mut cw = [false; 8];
        for (j, c) in cw.iter_mut().enumerate() {
            *c = bits[l * j + i];
        }
        let (v, f) = hamming_decode(cw)?;
        *n = v;
        fixes += f as usize;
    }
    Some(fixes)
}

/// A measurement value as the 24-bit float (4-bit exponent, 20-bit mantissa, value = m / 2^e), the most precise exponent.
pub fn encode_float(v: f64, dfm06: bool) -> u32 {
    let mut e = 15u32;
    while e > 0 && v * (1u64 << e) as f64 >= (1u64 << 20) as f64 {
        e -= 1;
    }
    let mut m = (v * (1u64 << e) as f64)
        .round()
        .clamp(0.0, ((1 << 20) - 1) as f64) as u32;
    if dfm06 {
        m &= !0xF;
    }
    (e << 20) | m
}

/// The value of a 24-bit measurement float.
pub fn decode_float(x: u32) -> f64 {
    let e = (x >> 20) & 0xF;
    let m = x & 0xF_FFFF;
    m as f64 / (1u64 << e) as f64
}

/// The DFM model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DfmModel {
    /// DFM-06 (opposite Manchester pairing, serial in channel 6).
    Dfm06,
    /// DFM-09 (serial channel 0xA).
    Dfm09,
    /// DFM-17 (serial channel 0xB).
    Dfm17,
}

impl DfmModel {
    /// The fixed calibration resistor of the temperature channel.
    pub fn rf(self) -> f64 {
        match self {
            DfmModel::Dfm17 => 332e3,
            _ => 220e3,
        }
    }
    /// The serial channel.
    pub fn serial_channel(self) -> u8 {
        match self {
            DfmModel::Dfm06 => 6,
            DfmModel::Dfm09 => 0xA,
            DfmModel::Dfm17 => 0xB,
        }
    }
    /// The name.
    pub fn name(self) -> &'static str {
        match self {
            DfmModel::Dfm06 => "DFM-06",
            DfmModel::Dfm09 => "DFM-09",
            DfmModel::Dfm17 => "DFM-17",
        }
    }
}

/// Bits (MSB first) of `v` into nibbles.
fn nibbles_of(v: u64, n: usize, out: &mut Vec<u8>) {
    for k in (0..n).rev() {
        out.push(((v >> (4 * k)) & 0xF) as u8);
    }
}

/// Makes the raw bits (280) of one frame from a configuration word and two data blocks.
pub fn frame_bits(cfg_chan: u8, cfg_val: u32, data: [(u8, u64); 2], out: &mut Vec<bool>) {
    for k in (0..16).rev() {
        out.push((HEADER >> k) & 1 == 1);
    }
    let mut nib = Vec::with_capacity(13);
    nib.push(cfg_chan & 0xF);
    nibbles_of(cfg_val as u64 & 0xFF_FFFF, 6, &mut nib);
    encode_block(&nib, out);
    for (id, payload) in data {
        nib.clear();
        nibbles_of(payload & 0xFFFF_FFFF_FFFF, 12, &mut nib);
        nib.push(id & 0xF);
        encode_block(&nib, out);
    }
}

/// Manchester symbols of bits: DFM-09/17 0 → 10, 1 → 01; DFM-06 the opposite.
pub fn manchester(bits: &[bool], dfm06: bool, out: &mut Vec<bool>) {
    for &b in bits {
        let b = b ^ dfm06;
        out.push(!b);
        out.push(b);
    }
}

#[derive(Clone, Debug, Default)]
struct Gps {
    mode: u8,
    ms: Option<u32>,
    lat: Option<f64>,
    lon: Option<f64>,
    alt_ell: Option<f64>,
    alt_msl: Option<f64>,
    sep: Option<f64>,
    vh: Option<f64>,
    vv: Option<f64>,
    dir: Option<f64>,
    date: Option<(i64, u32, u32, u32, u32)>,
    sats: Option<u32>,
    frame_no: Option<u8>,
    seen: [u64; 9],
}

/// The DFM decoder.
pub struct DfmDecoder {
    sync: SyncWord,
    inverted: bool,
    collecting: bool,
    syms: Vec<bool>,
    bits: Vec<bool>,
    frames: u64,
    gps: Gps,
    chans: [Option<u32>; 16],
    serial_lo: Option<u16>,
    serial_hi: Option<u16>,
    serial06: Option<(u32, u32)>,
    model: Option<DfmModel>,
    serial_chan: Option<u8>,
    first_time: Option<f64>,
}

impl Default for DfmDecoder {
    fn default() -> Self {
        let mut s = String::new();
        for k in (0..16).rev() {
            s.push_str(if (HEADER >> k) & 1 == 1 { "01" } else { "10" });
        }
        DfmDecoder {
            sync: sync_word(pattern_str(&s), 2),
            inverted: false,
            collecting: false,
            syms: Vec::with_capacity(FRAME_SYMBOLS),
            bits: Vec::with_capacity(280),
            frames: 0,
            gps: Gps::default(),
            chans: [None; 16],
            serial_lo: None,
            serial_hi: None,
            serial06: None,
            model: None,
            serial_chan: None,
            first_time: None,
        }
    }
}

fn bits_u64(nib: &[u8]) -> u64 {
    nib.iter().fold(0u64, |a, &n| (a << 4) | n as u64)
}

fn field(v: u64, start: u32, len: u32) -> u64 {
    (v >> (48 - start - len)) & ((1u64 << len) - 1)
}

fn sfield(v: u64, start: u32, len: u32) -> i64 {
    let x = field(v, start, len) as i64;
    if x & (1 << (len - 1)) != 0 {
        x - (1 << len)
    } else {
        x
    }
}

impl DfmDecoder {
    fn data_block(&mut self, id: u8, p: u64) {
        let g = &mut self.gps;
        if id <= 8 {
            g.seen[id as usize] = self.frames;
        }
        match id {
            0 => {
                g.mode = field(p, 16, 8) as u8;
                g.frame_no = Some(field(p, 24, 8) as u8);
                if g.mode >= 3 {
                    g.ms = Some(field(p, 0, 16) as u32);
                    g.vh = Some(field(p, 32, 16) as f64 / 100.0);
                }
            }
            1 if g.mode >= 3 => {
                g.lat = Some(sfield(p, 0, 32) as f64 * 1e-7);
                g.dir = Some(field(p, 32, 16) as f64 / 100.0);
            }
            1 => g.ms = Some(field(p, 32, 16) as u32),
            2 if g.mode >= 3 => {
                g.lon = Some(sfield(p, 0, 32) as f64 * 1e-7);
                g.vv = Some(sfield(p, 32, 16) as f64 / 100.0);
            }
            2 => {
                g.lat = Some(sfield(p, 0, 32) as f64 * 1e-7);
                g.vh = Some(field(p, 32, 16) as f64 / 100.0);
            }
            3 if g.mode >= 3 => g.alt_msl = Some(sfield(p, 0, 32) as f64 / 100.0),
            3 => {
                g.lon = Some(sfield(p, 0, 32) as f64 * 1e-7);
                g.dir = Some(field(p, 32, 16) as f64 / 100.0);
            }
            4 if g.mode < 3 => {
                g.alt_ell = Some(sfield(p, 0, 32) as f64 / 100.0);
                g.vv = Some(sfield(p, 32, 16) as f64 / 100.0);
            }
            5 if g.mode < 3 => g.sep = Some(sfield(p, 0, 16) as f64 / 100.0),
            8 => {
                g.date = Some((
                    field(p, 0, 12) as i64,
                    field(p, 12, 4) as u32,
                    field(p, 16, 5) as u32,
                    field(p, 21, 5) as u32,
                    field(p, 26, 6) as u32,
                ));
                g.sats = Some(field(p, 32, 8) as u32);
            }
            _ => {}
        }
    }

    fn config(&mut self, chan: u8, v: u32) {
        self.chans[chan as usize & 0xF] = Some(v);
        if chan == 5 && (v == 0xA0_0000 || v == 0xB0_0000) {
            self.model = Some(DfmModel::Dfm06);
        }
        if (0xA..=0xD).contains(&chan) {
            self.serial_chan = Some(chan);
            if self.model != Some(DfmModel::Dfm06) {
                self.model = Some(if chan == 0xA {
                    DfmModel::Dfm09
                } else {
                    DfmModel::Dfm17
                });
            }
            let half = v & 0xF;
            let num = ((v >> 4) & 0xFFFF) as u16;
            if half == 0 {
                self.serial_lo = Some(num);
            } else if half == 1 {
                self.serial_hi = Some(num);
            }
        }
        if chan == 6 && self.model == Some(DfmModel::Dfm06) {
            self.serial06 = match self.serial06 {
                Some((s, n)) if s == v => Some((s, n + 1)),
                _ => Some((v, 1)),
            };
        }
    }

    fn serial(&self) -> Option<String> {
        if self.model == Some(DfmModel::Dfm06) {
            return self
                .serial06
                .filter(|s| s.1 >= 2)
                .map(|s| format!("{:06X}", s.0));
        }
        match (self.serial_hi, self.serial_lo) {
            (Some(h), Some(l)) => Some(format!("{}", (h as u32) << 16 | l as u32)),
            _ => None,
        }
    }

    fn report(&self, t: f64) -> Option<SondeFrame> {
        let g = &self.gps;
        let recent = |id: usize| self.frames - g.seen[id] < 6 && g.seen[id] > 0;
        if ![0, 1, 2, 3, 4, 8].iter().all(|&i| recent(i)) {
            return None;
        }
        let serial = self.serial();
        if serial.is_none() && t - self.first_time.unwrap_or(t) < 15.0 {
            return None;
        }
        let model = self.model.unwrap_or(DfmModel::Dfm09);
        let mut f = SondeFrame {
            time: t,
            kind: Some(SondeType::Dfm),
            subtype: model.name().into(),
            serial,
            ok: true,
            ..Default::default()
        };
        f.lat = g.lat;
        f.lon = g.lon;
        f.alt = if g.mode >= 3 {
            g.alt_msl
        } else {
            g.alt_ell.map(|h| h + g.sep.unwrap_or(0.0))
        };
        f.vel_h = g.vh;
        f.vel_v = g.vv;
        f.heading = g.dir;
        f.sats = g.sats;
        f.frame = g.frame_no.map(|n| n as i64);
        if let (Some((y, mo, d, h, mi)), Some(ms)) = (g.date, g.ms) {
            if (1..=12).contains(&mo) && (1..=31).contains(&d) && h < 24 && mi < 60 {
                f.utc = calendar::unix(y, mo, d, h, mi, 0).map(|u| u as f64 + ms as f64 / 1000.0);
            }
        }
        // Temperature from channels 0, 3, 4.
        if let (Some(a), Some(b), Some(c)) = (self.chans[0], self.chans[3], self.chans[4]) {
            let (m0, m3, m4) = (decode_float(a), decode_float(b), decode_float(c));
            let g = m4 / model.rf();
            if g > 0.0 {
                let r = (m0 - m3) / g;
                if r > 0.0 {
                    let t = steinhart_c(r, THERMISTOR);
                    if (-120.0..=60.0).contains(&t) {
                        f.temp_c = Some(t);
                    }
                }
            }
        }
        if model != DfmModel::Dfm06 {
            if let Some(v) = self.chans[5] {
                f.battery_v = Some((v & 0xFFFF) as f64 / 1000.0);
            }
        }
        Some(f)
    }

    fn frame(&mut self, t: f64, out: &mut Vec<SondeFrame>) {
        self.frames += 1;
        self.first_time.get_or_insert(t);
        let mut nib = [0u8; 13];
        let bits = std::mem::take(&mut self.bits);
        let cfg = decode_block(&bits[16..72], 7, &mut nib);
        let mut bad = false;
        match cfg {
            Some(fx) if fx <= 4 => {
                let chan = nib[0];
                let v = bits_u64(&nib[1..7]) as u32;
                self.config(chan, v);
            }
            _ => bad = true,
        }
        let mut has8 = false;
        for k in 0..2 {
            let s = 72 + 104 * k;
            match decode_block(&bits[s..s + 104], 13, &mut nib) {
                Some(fx) if fx <= 4 => {
                    let p = bits_u64(&nib[0..12]);
                    let id = nib[12];
                    has8 |= id == 8;
                    self.data_block(id, p);
                }
                _ => bad = true,
            }
        }
        self.bits = bits;
        if bad {
            out.push(SondeFrame {
                time: t,
                kind: Some(SondeType::Dfm),
                subtype: self.model.unwrap_or(DfmModel::Dfm09).name().into(),
                serial: self.serial(),
                ok: false,
                ..Default::default()
            });
            return;
        }
        if has8 {
            if let Some(r) = self.report(t) {
                out.push(r);
            }
        }
    }
}

impl FrameDecoder for DfmDecoder {
    fn kind(&self) -> SondeType {
        SondeType::Dfm
    }
    fn push(&mut self, sym: bool, t: f64, out: &mut Vec<SondeFrame>) {
        if !self.collecting {
            if let Some(inv) = self.sync.push(sym as u8).map(|h| h.inverted()) {
                self.inverted = inv;
                self.collecting = true;
                self.bits.clear();
                for k in (0..16).rev() {
                    self.bits.push((HEADER >> k) & 1 == 1);
                }
                self.syms.clear();
            }
            return;
        }
        self.syms.push(sym ^ self.inverted);
        if self.syms.len() == 2 {
            self.bits.push(self.syms[1]);
            self.syms.clear();
        }
        if self.bits.len() >= 280 {
            if self.inverted && self.model.is_none() {
                // Opposite pairing: a DFM-06 (confirmed by its channel 5 pattern).
            }
            self.frame(t, out);
            self.collecting = false;
            self.sync.reset();
        }
    }
    fn reset(&mut self, hard: bool) {
        self.collecting = false;
        self.sync.reset();
        self.bits.clear();
        self.syms.clear();
        if hard {
            self.gps = Gps::default();
            self.chans = [None; 16];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_symbols() {
        let mut bits = Vec::new();
        for k in (0..16).rev() {
            bits.push((HEADER >> k) & 1 == 1);
        }
        let mut s = Vec::new();
        manchester(&bits, false, &mut s);
        let txt: String = s.iter().map(|&b| if b { '1' } else { '0' }).collect();
        assert_eq!(txt, "10011010100110010101101001010101");
        let mut fb = Vec::new();
        frame_bits(0, 0, [(0, 0), (1, 0)], &mut fb);
        assert_eq!(fb.len(), 280);
    }

    #[test]
    fn floats() {
        for v in [0.05, 1.234, 5.0, 2.2] {
            assert!((decode_float(encode_float(v, false)) - v).abs() < 1e-4);
        }
    }
}
