//! The beacon side channel of frame version 3: the frame bits (callsign, position, power, link quality, grid, text), the rate
//! 1/4 convolutional code, the BPSK slice at +4500 Hz and 1000 symbols a second, and its receiver.

use crate::family::{crc16_bits, crc32, push_bits, push_bytes, read_bits};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_fec::conv::codes::K7_171_133_165_117;
use sdrkit_algo_fec::conv::{Encoder, Ends, Viterbi};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The beacon alphabet: codes 0 to 38 (code 39 has no character).
pub const ALPHABET: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/-";
/// The sync word of a frame without text.
pub const SYNC_HEADER: u64 = 0xDEC9_5176_68DB_6B8B;
/// The sync word of a frame with text.
pub const SYNC_FULL: u64 = 0x936D_AE59_FB6C_4644;
/// Header bits.
pub const HEADER_BITS: usize = 170;
/// The longest text, bytes.
pub const MAX_TEXT: usize = 100;
/// The slice frequency at 48 kHz, Hz.
pub const SLICE_HZ: f64 = 4500.0;
/// Beacon symbols a second at 48 kHz.
pub const SYMBOL_RATE: f64 = 1000.0;

/// The fields of a beacon frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Beacon {
    /// Up to 9 characters of the beacon alphabet.
    pub callsign: String,
    /// Degrees north.
    pub lat: f64,
    /// Degrees east.
    pub lon: f64,
    /// Transmit power, dBm (-40 to 87.5 in 0.5 dB steps).
    pub power_dbm: f64,
    /// Link quality, 0 to 255.
    pub link: u8,
    /// Six-character grid locator.
    pub grid: String,
    /// Up to 100 bytes of text.
    pub text: Vec<u8>,
}

/// Why a beacon cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeaconError {
    /// The callsign is longer than 9 characters or has a character outside the alphabet.
    Callsign,
    /// The grid locator is not six characters of the form AA00aa.
    Grid,
}

impl std::fmt::Display for BeaconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BeaconError::Callsign => "the callsign must be 1 to 9 characters of A-Z, 0-9, / and -",
            BeaconError::Grid => "the grid locator must be six characters such as KO26ab",
        })
    }
}

impl std::error::Error for BeaconError {}

/// The grid value of a six-character locator.
pub fn grid_value(grid: &str) -> Option<u64> {
    let g = grid.as_bytes();
    if g.len() != 6 {
        return None;
    }
    let up = |c: u8, base: u8, n: u8| {
        let c = c.to_ascii_uppercase();
        (c >= base && c < base + n).then(|| u64::from(c - base))
    };
    let f1 = up(g[0], b'A', 18)?;
    let f2 = up(g[1], b'A', 18)?;
    let d1 = up(g[2], b'0', 10)?;
    let d2 = up(g[3], b'0', 10)?;
    let s1 = up(g[4], b'A', 24)?;
    let s2 = up(g[5], b'A', 24)?;
    Some(((((f1 * 18 + f2) * 10 + d1) * 10 + d2) * 24 + s1) * 24 + s2)
}

/// The locator of a grid value: field letters upper case, subsquare letters lower case.
pub fn grid_text(mut v: u64) -> String {
    let s2 = v % 24;
    v /= 24;
    let s1 = v % 24;
    v /= 24;
    let d2 = v % 10;
    v /= 10;
    let d1 = v % 10;
    v /= 10;
    let f2 = v % 18;
    let f1 = (v / 18) % 18;
    [b'A' + f1 as u8, b'A' + f2 as u8, b'0' + d1 as u8, b'0' + d2 as u8, b'a' + s1 as u8, b'a' + s2 as u8]
        .iter()
        .map(|&c| c as char)
        .collect()
}

/// The frame bits of `b`: the 170 header bits and, when the text is not empty, the text and its CRC-32. `header_only` drops
/// the text.
pub fn frame_bits(b: &Beacon, header_only: bool) -> Result<Vec<u8>, BeaconError> {
    let call = b.callsign.trim().to_ascii_uppercase();
    if call.len() > 9 {
        return Err(BeaconError::Callsign);
    }
    let mut cv: u64 = 0;
    for i in 0..9 {
        let c = call.as_bytes().get(i).copied().unwrap_or(b' ');
        cv = cv * 40 + ALPHABET.iter().position(|&a| a == c).ok_or(BeaconError::Callsign)? as u64;
    }
    let grid = grid_value(&b.grid).ok_or(BeaconError::Grid)?;
    let text: &[u8] = if header_only { &[] } else { &b.text[..b.text.len().min(MAX_TEXT)] };
    let mut v = Vec::with_capacity(HEADER_BITS + 8 * text.len() + 32);
    push_bits(&mut v, 1, 4);
    push_bits(&mut v, u64::from(!text.is_empty()), 4);
    push_bits(&mut v, cv, 48);
    let lat = (b.lat * f64::from(1 << 23) / 90.0).round_ties_even() as i64;
    let lon = (b.lon * f64::from(1 << 24) / 180.0).round_ties_even() as i64;
    push_bits(&mut v, (lat & 0xFF_FFFF) as u64, 24);
    push_bits(&mut v, (lon & 0x1FF_FFFF) as u64, 25);
    push_bits(&mut v, ((b.power_dbm + 40.0) * 2.0).round_ties_even().clamp(0.0, 255.0) as u64, 8);
    push_bits(&mut v, u64::from(b.link), 8);
    push_bits(&mut v, grid, 25);
    push_bits(&mut v, text.len() as u64, 8);
    let crc = crc16_bits(&v);
    push_bits(&mut v, u64::from(crc), 16);
    if !text.is_empty() {
        push_bytes(&mut v, text);
        push_bits(&mut v, u64::from(crc32(text)), 32);
    }
    Ok(v)
}

fn signed(v: u64, width: u32) -> i64 {
    let s = 64 - width;
    ((v << s) as i64) >> s
}

/// The beacon of frame bits `bits` (the header, and the text when the length is not 0); `None` when a CRC fails, the text
/// length is above 100 or the bits are too few.
pub fn parse(bits: &[u8]) -> Option<Beacon> {
    if bits.len() < HEADER_BITS || crc16_bits(&bits[..154]) != read_bits(bits, 154, 16) as u16 {
        return None;
    }
    let len = read_bits(bits, 146, 8) as usize;
    if len > MAX_TEXT {
        return None;
    }
    let mut text = Vec::with_capacity(len);
    if len > 0 {
        if bits.len() < HEADER_BITS + 8 * len + 32 {
            return None;
        }
        for i in 0..len {
            text.push(read_bits(bits, HEADER_BITS + 8 * i, 8) as u8);
        }
        if crc32(&text) != read_bits(bits, HEADER_BITS + 8 * len, 32) as u32 {
            return None;
        }
    }
    let mut cv = read_bits(bits, 8, 48);
    let mut call = [b' '; 9];
    for i in (0..9).rev() {
        call[i] = ALPHABET.get((cv % 40) as usize).copied().unwrap_or(b'?');
        cv /= 40;
    }
    Some(Beacon {
        callsign: String::from_utf8_lossy(&call).trim_end().to_string(),
        lat: signed(read_bits(bits, 56, 24), 24) as f64 * 90.0 / f64::from(1 << 23),
        lon: signed(read_bits(bits, 80, 25), 25) as f64 * 180.0 / f64::from(1 << 24),
        power_dbm: -40.0 + 0.5 * read_bits(bits, 105, 8) as f64,
        link: read_bits(bits, 113, 8) as u8,
        grid: grid_text(read_bits(bits, 121, 25)),
        text,
    })
}

/// The coded bits of frame bits `bits`: rate 1/4, with the six tail bits.
pub fn code(bits: &[u8]) -> Vec<u8> {
    let mut e = Encoder::new(K7_171_133_165_117);
    let mut out = Vec::with_capacity(4 * (bits.len() + 6));
    e.encode(bits, &mut out);
    e.terminate(&mut out);
    out
}

/// The RRC of the slice: roll-off 0.35, 8 symbols, `sps` samples a symbol, unit energy.
fn pulse(sps: usize) -> Vec<f32> {
    taps(Shape::Rrc { beta: 0.35 }, sps as f64, 8.0, Norm::Energy).expect("beacon pulse")
}

/// The slice of frame bits `bits` at `fs`: the sync word (bit 1 is +1) and the coded bits (bit 0 is +1) shaped with the RRC,
/// scaled to unit RMS and moved to +4500 Hz.
pub fn slice(bits: &[u8], fs: f64) -> Vec<Complex32> {
    let sync = if read_bits(bits, 4, 4) == 1 { SYNC_FULL } else { SYNC_HEADER };
    let mut sym: Vec<f32> = (0..64).rev().map(|i| if (sync >> i) & 1 == 1 { 1.0 } else { -1.0 }).collect();
    sym.extend(code(bits).iter().map(|&b| if b == 0 { 1.0 } else { -1.0 }));
    let sps = (fs / SYMBOL_RATE).round() as usize;
    let h = pulse(sps);
    let mut y = vec![0f32; sym.len() * sps + h.len() - 1];
    for (k, &s) in sym.iter().enumerate() {
        for (i, &t) in h.iter().enumerate() {
            y[k * sps + i] += s * t;
        }
    }
    let rms = (y.iter().map(|v| f64::from(v * v)).sum::<f64>() / y.len() as f64).sqrt();
    y.iter()
        .enumerate()
        .map(|(i, &v)| {
            let a = TAU * (SLICE_HZ * i as f64 / fs).fract();
            let m = f64::from(v) / rms;
            Complex32::new((m * a.cos()) as f32, (m * a.sin()) as f32)
        })
        .collect()
}

/// The decimation of the receiver: 48 kHz to 16 samples a symbol.
const DECIM: usize = 3;
/// Samples a symbol after the decimation.
const SPS: usize = 16;

/// A beacon heard.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The fields.
    pub beacon: Beacon,
    /// True for a frame with text.
    pub full: bool,
    /// The input sample (48 kHz) of the first sync symbol.
    pub at: u64,
}

/// The receiver of the beacon slice in a 48 kHz stream whose centre is the voice carrier.
pub struct BeaconReceiver {
    h: Vec<f32>,
    /// Mixed input at 48 kHz, `raw[0]` is input `raw_base`.
    raw: Vec<Complex32>,
    raw_base: u64,
    /// Matched-filter output at 16 kHz; `y[0]` is decimated sample `y_base`.
    y: Vec<Complex32>,
    y_base: u64,
    /// The next decimated sample to try as a frame start.
    next: u64,
    n_in: u64,
    vit: Viterbi,
    syncs: [[f32; 64]; 2],
}

impl Default for BeaconReceiver {
    fn default() -> Self {
        Self::new()
    }
}

impl BeaconReceiver {
    /// A receiver for 48 kHz input.
    pub fn new() -> BeaconReceiver {
        let word = |w: u64| std::array::from_fn(|i| if (w >> (63 - i)) & 1 == 1 { 1.0 } else { -1.0 });
        BeaconReceiver {
            h: pulse(48),
            raw: Vec::new(),
            raw_base: 0,
            y: Vec::new(),
            y_base: 0,
            next: 0,
            n_in: 0,
            vit: Viterbi::new(K7_171_133_165_117, HEADER_BITS + 8 * MAX_TEXT + 32 + 6).expect("beacon Viterbi"),
            syncs: [word(SYNC_HEADER), word(SYNC_FULL)],
        }
    }

    /// Forgets the stream.
    pub fn reset(&mut self) {
        *self = BeaconReceiver {
            vit: std::mem::replace(&mut self.vit, Viterbi::new(K7_171_133_165_117, 8).expect("Viterbi")),
            ..BeaconReceiver::new()
        };
    }

    /// Takes 48 kHz samples and appends the beacons that they complete.
    pub fn process(&mut self, x: &[Complex32], heard: &mut Vec<Heard>) {
        for &v in x {
            let a = -TAU * (SLICE_HZ * self.n_in as f64 / 48_000.0).fract();
            self.raw.push(v * Complex32::new(a.cos() as f32, a.sin() as f32));
            self.n_in += 1;
        }
        // Matched filter, every third output.
        let l = self.h.len() as u64;
        loop {
            let j = self.y_base + self.y.len() as u64;
            let end = j * DECIM as u64 + l;
            if end > self.raw_base + self.raw.len() as u64 {
                break;
            }
            let s = (j * DECIM as u64 - self.raw_base) as usize;
            let mut acc = Complex32::new(0.0, 0.0);
            for (i, &t) in self.h.iter().enumerate() {
                acc += self.raw[s + i] * t;
            }
            self.y.push(acc);
        }
        let keep = ((self.y_base + self.y.len() as u64) * DECIM as u64).saturating_sub(self.raw_base);
        if keep > 4096 {
            self.raw.drain(..keep as usize - 1);
            self.raw_base += keep - 1;
        }
        self.search(heard);
        // Keep 5 s of the filtered stream.
        let cap = 5 * 16_000;
        if self.y.len() > cap {
            let drop = self.y.len() - cap;
            self.y.drain(..drop);
            self.y_base += drop as u64;
            self.next = self.next.max(self.y_base);
        }
    }

    fn sync_metric(&self, at: usize, w: usize) -> (f32, Complex32, Complex32) {
        let (mut c1, mut c2, mut e) = (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0), 0f32);
        for k in 0..64 {
            let v = self.y[at + SPS * k];
            e += v.norm_sqr();
            if k < 32 {
                c1 += v * self.syncs[w][k];
            } else {
                c2 += v * self.syncs[w][k];
            }
        }
        ((c1 + c2).norm_sqr() / (64.0 * e.max(1e-20)), c1, c2)
    }

    fn search(&mut self, heard: &mut Vec<Heard>) {
        let end = self.y_base + self.y.len() as u64;
        while self.next + (64 * SPS + 2 * SPS) as u64 <= end {
            let at = (self.next - self.y_base) as usize;
            let mut best = (0f32, 0usize, 0usize);
            for o in 0..SPS {
                for w in 0..2 {
                    let m = self.sync_metric(at + o, w).0;
                    if m > best.0 {
                        best = (m, o, w);
                    }
                }
            }
            if best.0 < 0.25 {
                self.next += SPS as u64;
                continue;
            }
            // Refine on the next symbol too, then decode when the frame is in.
            let pos = at + best.1;
            let full = best.2 == 1;
            let need_steps = if full { HEADER_BITS + 8 * MAX_TEXT + 32 + 6 } else { HEADER_BITS + 6 };
            let have = (self.y.len() - pos) / SPS;
            if have < 64 + 4 * (HEADER_BITS + 6) || (full && have < 64 + 4 * 210) {
                return;
            }
            match self.decode(pos, full, need_steps) {
                Decoded::Frame(b, used) => {
                    heard.push(Heard {
                        beacon: b,
                        full,
                        at: (self.y_base + pos as u64) * DECIM as u64,
                    });
                    self.next = self.y_base + (pos + used * SPS) as u64;
                }
                Decoded::Wait => return,
                Decoded::Fail => self.next += SPS as u64,
            }
        }
    }

    fn symbols(&self, pos: usize, n: usize, f: f64, phase: Complex32, out: &mut Vec<f32>) -> bool {
        out.clear();
        if pos + SPS * (n - 1) >= self.y.len() {
            return false;
        }
        for k in 0..n {
            let a = -TAU * f * k as f64 / SYMBOL_RATE;
            let r = Complex32::new(a.cos() as f32, a.sin() as f32);
            out.push((self.y[pos + SPS * k] * r * phase).re);
        }
        true
    }

    fn decode(&mut self, pos: usize, full: bool, _max_steps: usize) -> Decoded {
        let w = usize::from(full);
        let (_, c1, c2) = self.sync_metric(pos, w);
        let est = f64::from((c2 * c1.conj()).arg()) / (TAU * 0.032);
        let mut sym = Vec::new();
        for df in [0.0, 1.0, -1.0, 2.0, -2.0, 3.5, -3.5, 5.0, -5.0] {
            for base in [est, 0.0] {
                let f = base + df;
                if base == 0.0 && df != 0.0 {
                    continue;
                }
                // The phase of the sync with this frequency removed.
                let mut c = Complex32::new(0.0, 0.0);
                for k in 0..64 {
                    let a = -TAU * f * k as f64 / SYMBOL_RATE;
                    c += self.y[pos + SPS * k] * Complex32::new(a.cos() as f32, a.sin() as f32) * self.syncs[w][k];
                }
                let phase = c.conj() / c.norm().max(1e-20);
                let mut steps = HEADER_BITS + 6;
                if full {
                    if !self.symbols(pos, 64 + 4 * 210, f, phase, &mut sym) {
                        return Decoded::Wait;
                    }
                    let Some(bits) = self.viterbi(&sym[64..], 210, Ends::Start(0)) else {
                        continue;
                    };
                    let len = read_bits(&bits, 146, 8) as usize;
                    if len == 0 || len > MAX_TEXT {
                        continue;
                    }
                    steps = HEADER_BITS + 8 * len + 32 + 6;
                }
                if !self.symbols(pos, 64 + 4 * steps, f, phase, &mut sym) {
                    return Decoded::Wait;
                }
                let Some(bits) = self.viterbi(&sym[64..], steps, Ends::Zero) else {
                    continue;
                };
                if let Some(b) = parse(&bits) {
                    if b.text.is_empty() != !full {
                        continue;
                    }
                    return Decoded::Frame(b, 64 + 4 * steps);
                }
            }
        }
        Decoded::Fail
    }

    fn viterbi(&mut self, soft: &[f32], steps: usize, ends: Ends) -> Option<Vec<u8>> {
        let n = 4 * steps;
        let mean = soft[..n].iter().map(|v| v.abs()).sum::<f32>() / n as f32;
        let scale = 24.0 / mean.max(1e-12);
        let llr: Vec<i8> = soft[..n].iter().map(|v| (v * scale).round().clamp(-127.0, 127.0) as i8).collect();
        let keep = if matches!(ends, Ends::Zero) { steps - 6 } else { steps };
        let mut out = vec![0u8; keep];
        self.vit.decode(&llr, ends, &mut out).ok()?;
        Some(out)
    }
}

enum Decoded {
    Frame(Beacon, usize),
    Wait,
    Fail,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_bits(h: &str, n: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for c in h.chars() {
            push_bits(&mut v, u64::from(c.to_digit(16).unwrap()), 4);
        }
        v.truncate(n);
        v
    }

    const FULL: &str = "11885b9c50f2004dc6ee11f9e73c002aaf20528961521a4b081248185b481d195cdd1a5b99c812d050530b08184818dd5cdd1bdb48191a59da5d185b081d9bda58d9481b5bd9194b8815195b1b081b59481dda185d081e5bdd481a19585c8b938191408";

    fn fields() -> Beacon {
        Beacon {
            callsign: "LY2ABC".into(),
            lat: 54.687,
            lon: 25.279,
            power_dbm: 20.0,
            link: 0,
            grid: "KO26ab".into(),
            text: Vec::new(),
        }
    }

    #[test]
    fn frame_bits_of_the_vectors() {
        let h = frame_bits(&fields(), true).unwrap();
        assert_eq!(h, hex_bits("10885b9c50f2004dc6ee11f9e73c002aaf204021050", 170));
        assert_eq!(code(&h), hex_bits("000faed3a5edcf1d3a518794d3f4c160b60e7d8caab6a5b34235f0000fae2613bd3fb6fbcbd30e23fb955edc046500abc19a5bc1604c19a549af000000fa1722d2d2dd7355b34235ffaed35f00faed3a5ed350a17d8c5f00", 704));
        // The full frame: read it, check the fields, and make it again from them.
        let full = hex_bits(FULL, 794);
        let b = parse(&full).unwrap();
        assert_eq!(b.text.len(), 74);
        assert_eq!(Beacon { text: Vec::new(), ..b.clone() }, Beacon { lat: b.lat, lon: b.lon, ..fields() });
        assert!((b.lat - 54.687).abs() < 1e-5 && (b.lon - 25.279).abs() < 1e-5);
        assert_eq!(frame_bits(&b, false).unwrap(), full);
        for i in [0, 40, 160, 300, 793] {
            let mut bad = full.clone();
            bad[i] ^= 1;
            assert_eq!(parse(&bad), None, "{i}");
        }
        assert_eq!(parse(&full[..700]), None);
        assert_eq!(parse(&[0u8; 100]), None);
        assert_eq!(frame_bits(&Beacon { callsign: "ABCDEFGHIJ".into(), ..fields() }, true), Err(BeaconError::Callsign));
    }

    #[test]
    fn the_slice_of_the_full_frame() {
        let y = slice(&hex_bits(FULL, 794), 48_000.0);
        assert_eq!(y.len(), 157_056);
        let want = [
            (0, 0.0020454, 0.0),
            (1, 0.0027027, 0.0018059),
            (2, 0.0016991, 0.0041020),
            (7, -0.0054477, -0.0081531),
            (500, 1.033_362, -1.033_362),
            (3000, 0.0, -1.2361608),
            (20000, -1.2836765, 0.0),
            (100_000, -0.9102434, 0.0),
            (157_055, 0.0, 0.0),
        ];
        for (i, re, im) in want {
            assert!((y[i].re - re).abs() < 2e-4 && (y[i].im - im).abs() < 2e-4, "{i} {:?}", y[i]);
        }
    }

    #[test]
    fn a_clean_stream_gives_both_frames_in_order() {
        let mut b = fields();
        b.text = b"TEST1 beacon text".to_vec();
        let mut x = vec![Complex32::new(0.0, 0.0); 3000];
        x.extend(slice(&frame_bits(&b, true).unwrap(), 48_000.0));
        x.extend(vec![Complex32::new(0.0, 0.0); 2000]);
        x.extend(slice(&frame_bits(&b, false).unwrap(), 48_000.0));
        x.extend(vec![Complex32::new(0.0, 0.0); 4000]);
        let mut rx = BeaconReceiver::new();
        let mut heard = Vec::new();
        for c in x.chunks(1000) {
            rx.process(c, &mut heard);
        }
        assert_eq!(heard.len(), 2, "{heard:?}");
        assert!(!heard[0].full && heard[1].full);
        assert_eq!(heard[1].beacon.text, b.text);
        assert_eq!(heard[0].beacon.callsign, "LY2ABC");
    }
}
