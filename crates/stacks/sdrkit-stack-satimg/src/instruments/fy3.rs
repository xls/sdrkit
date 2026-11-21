//! The FY-3 instruments: VIRR, MERSI (FY-3A/B/C), MERSI-II, MERSI-3, MERSI-LL, MERSI-RM, MWRI and MWRI-2, each a
//! bit stream on its virtual channel, to picture strips.

use crate::items::Vcdu;
use sdrkit_codec_image::strip::Strip;
use std::collections::BTreeMap;
use std::sync::Arc;

/// An FY-3 instrument with a picture (or counted only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Instrument {
    /// VIRR (FY-3A, B, C AHRPT).
    Virr,
    /// MERSI (FY-3A, B, C).
    Mersi,
    /// MERSI-II (FY-3D).
    Mersi2,
    /// MERSI-3 (FY-3F, H).
    Mersi3,
    /// MERSI-LL (FY-3E).
    MersiLl,
    /// MERSI-RM (FY-3G).
    MersiRm,
    /// MWRI (FY-3D).
    Mwri,
    /// MWRI-2 (FY-3F, H) and MWRI-RM (FY-3G).
    Mwri2,
    /// Counted, not decoded.
    Other(&'static str),
}

impl Instrument {
    /// The name.
    pub fn name(self) -> &'static str {
        match self {
            Instrument::Virr => "VIRR",
            Instrument::Mersi => "MERSI",
            Instrument::Mersi2 => "MERSI-II",
            Instrument::Mersi3 => "MERSI-3",
            Instrument::MersiLl => "MERSI-LL",
            Instrument::MersiRm => "MERSI-RM",
            Instrument::Mwri => "MWRI",
            Instrument::Mwri2 => "MWRI-2",
            Instrument::Other(n) => n,
        }
    }
    /// It has a picture.
    pub fn imaging(self) -> bool {
        !matches!(self, Instrument::Other(_))
    }
}

/// The satellite of a spacecraft id.
pub fn satellite(sc: u8) -> Option<&'static str> {
    Some(match sc {
        49 => "FY-3A",
        50 => "FY-3B",
        51 => "FY-3C",
        52 => "FY-3D",
        56 => "FY-3E",
        60 => "FY-3F",
        64 => "FY-3G",
        68 => "FY-3H",
        _ => return None,
    })
}

/// The instrument on a virtual channel; `mpt` for the MPT product.
pub fn instrument(sc: u8, vc: u8, mpt: bool) -> Option<Instrument> {
    use Instrument::*;
    Some(match (satellite(sc), vc) {
        (Some("FY-3A" | "FY-3B" | "FY-3C"), 5) if !mpt => Virr,
        (Some("FY-3A" | "FY-3B" | "FY-3C"), 3) => Mersi,
        (Some("FY-3D"), 3) => Mersi2,
        (Some("FY-3D"), 5) => Other("WAAI"),
        (Some("FY-3D"), 9) => Other("GAS"),
        (Some("FY-3D"), 10) => Mwri,
        (Some("FY-3E"), 3) => MersiLl,
        (Some("FY-3E"), 5) => Other("X-EUVI"),
        (Some("FY-3E"), 10 | 18) => Other("WindRAD"),
        (Some("FY-3F" | "FY-3H"), 3) => Mersi3,
        (Some("FY-3F" | "FY-3H"), 42) => Mwri2,
        (Some("FY-3G"), 35) => MersiRm,
        (Some("FY-3G"), 38 | 47) => Other("PMR"),
        (Some("FY-3G"), 42) => Mwri2,
        (None, 5) if !mpt => Virr,
        (None, 3) => {
            if mpt {
                Mersi
            } else {
                Mersi2
            }
        }
        _ => return None,
    })
}

/// The VIRR sync word (60 bits).
pub const VIRR_SYNC: u64 = 0x0A11_6FD7_19D8_3C95;
/// Bytes per VIRR frame.
pub const VIRR_LEN: usize = 26_050;
/// The bit where the VIRR pixels start (from the start of the sync word).
pub const VIRR_PIXELS_BIT: usize = 436 * 8 + 2;
/// The MERSI head-frame sync (48 bits).
pub const MERSI_HEAD_SYNC: u64 = 0x55AA_55AA_55AA;
/// The MERSI scan-frame sync (28 bits; this implementation's value, see the module documentation).
pub const MERSI_SCAN_SYNC: u64 = 0x5A5_C3A9;
/// The MWRI sync (48 bits).
pub const MWRI_SYNC: u64 = 0xEB90_1ACF_FC1D;
/// The MWRI-2 sync (64 bits).
pub const MWRI2_SYNC: u64 = 0xA7F8_AAAA_EB90_352E;

/// A MERSI layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MersiLayout {
    /// 250 m channels.
    pub ch250: usize,
    /// 1 km channels.
    pub ch1k: usize,
    /// 250 m width.
    pub width250: usize,
    /// Head frame bits.
    pub head_bits: usize,
    /// 250 m scan frame bits.
    pub scan250_bits: usize,
    /// 1 km scan frame bits.
    pub scan1k_bits: usize,
    /// Pixels start (byte, bit).
    pub pixels: (usize, usize),
}

impl MersiLayout {
    /// The layout of an instrument.
    pub fn of(i: Instrument) -> Option<MersiLayout> {
        let l = |a, b, w, h, s2, s1, p| MersiLayout {
            ch250: a,
            ch1k: b,
            width250: w,
            head_bits: h,
            scan250_bits: s2,
            scan1k_bits: s1,
            pixels: p,
        };
        Some(match i {
            Instrument::Mersi => l(5, 15, 8192, 206_048, 99_120, 25_392, (92, 6)),
            Instrument::Mersi2 => l(6, 19, 8192, 1_329_256, 98_856, 25_128, (59, 6)),
            Instrument::Mersi3 => l(6, 19, 8192, 1_329_256, 98_904, 25_176, (65, 6)),
            Instrument::MersiLl => l(2, 16, 6144, 419_176, 74_976, 19_680, (146, 6)),
            Instrument::MersiRm => l(0, 8, 6240, 42_864, 18_816, 18_816, (2, 6)),
            _ => return None,
        })
    }
    /// The 1 km grid's width.
    pub fn width1k(&self) -> usize {
        self.width250 / 4
    }
}

/// The picture width of an instrument.
pub fn width(i: Instrument) -> u32 {
    match i {
        Instrument::Virr => 2048,
        Instrument::Mwri => 266,
        Instrument::Mwri2 => 492,
        _ => MersiLayout::of(i).map_or(2048, |l| l.width1k() as u32),
    }
}

/// Channels of an instrument (number, name) as the screens list them.
pub fn channels(i: Instrument) -> Vec<(u16, String)> {
    match i {
        Instrument::Virr => [
            "1 (0.63 um red)",
            "2 (0.865 um)",
            "3 (3.74 um)",
            "4 (10.8 um)",
            "5 (12 um)",
            "6 (1.6 um)",
            "7 (0.455 um blue)",
            "8 (0.505 um)",
            "9 (0.555 um green)",
            "10 (1.36 um)",
        ]
        .iter()
        .enumerate()
        .map(|(k, n)| (k as u16 + 1, n.to_string()))
        .collect(),
        Instrument::Mwri => (1..=10).map(|c| (c, format!("{c}"))).collect(),
        Instrument::Mwri2 => (1..=26).map(|c| (c, format!("{c}"))).collect(),
        _ => {
            let Some(l) = MersiLayout::of(i) else {
                return vec![];
            };
            let mut v = Vec::new();
            for c in 1..=(l.ch250 + l.ch1k) as u16 {
                let name = match (i, c) {
                    (Instrument::Mersi, 1) => "1 (0.47 um blue)".to_string(),
                    (Instrument::Mersi, 2) => "2 (0.55 um green)".into(),
                    (Instrument::Mersi, 3) => "3 (0.65 um red)".into(),
                    (Instrument::Mersi, 4) => "4 (0.865 um)".into(),
                    (Instrument::Mersi, 5) => "5 (10.8 um)".into(),
                    (Instrument::Mersi2 | Instrument::Mersi3, 1) => "1 (0.47 um blue)".into(),
                    (Instrument::Mersi2 | Instrument::Mersi3, 2) => "2 (0.55 um green)".into(),
                    (Instrument::Mersi2 | Instrument::Mersi3, 3) => "3 (0.65 um red)".into(),
                    (Instrument::Mersi2 | Instrument::Mersi3, 4) => "4 (0.865 um)".into(),
                    (Instrument::Mersi2 | Instrument::Mersi3, 5) => "24 (10.8 um)".into(),
                    (Instrument::Mersi2 | Instrument::Mersi3, 6) => "25 (12 um)".into(),
                    (_, c) if (c as usize) <= l.ch250 => format!("{c} (250 m)"),
                    (_, c) => format!("{c} (1 km)"),
                };
                v.push((c, name));
            }
            v
        }
    }
}

/// The thermal channels (shown inverted).
pub fn thermal(i: Instrument, ch: u16) -> bool {
    match i {
        Instrument::Virr => matches!(ch, 3..=5),
        Instrument::Mersi => ch == 5,
        Instrument::Mersi2 | Instrument::Mersi3 => matches!(ch, 5 | 6),
        _ => false,
    }
}

fn get_bits(data: &[u8], bit: usize, n: usize) -> u32 {
    let mut v = 0u32;
    for i in 0..n {
        let b = bit + i;
        let byte = data.get(b / 8).copied().unwrap_or(0);
        v = (v << 1) | ((byte >> (7 - b % 8)) & 1) as u32;
    }
    v
}

fn put_bits(data: &mut [u8], bit: usize, n: usize, v: u32) {
    for i in 0..n {
        let b = bit + i;
        if (v >> (n - 1 - i)) & 1 == 1 {
            data[b / 8] |= 0x80 >> (b % 8);
        } else {
            data[b / 8] &= !(0x80 >> (b % 8));
        }
    }
}

fn put_sync(data: &mut [u8], sync: u64, bits: usize) {
    for i in 0..bits {
        let v = ((sync >> (bits - 1 - i)) & 1) as u32;
        put_bits(data, i, 1, v);
    }
}

/// Builds a VIRR frame (the generator's side): 10 x 2048 pixels of 10 bits (channel-major), day and ms of day.
pub fn build_virr(day: u16, ms: u32, px: &[u16]) -> Vec<u8> {
    let mut f = vec![0u8; VIRR_LEN];
    put_sync(&mut f, VIRR_SYNC, 60);
    f[8..10].copy_from_slice(&day.to_be_bytes());
    f[10..14].copy_from_slice(&ms.to_be_bytes());
    for x in 0..2048 {
        for c in 0..10 {
            put_bits(
                &mut f,
                VIRR_PIXELS_BIT + (10 * x + c) * 10,
                10,
                px[c * 2048 + x] as u32,
            );
        }
    }
    f
}

/// Builds an MWRI frame: 10 channels x 266 pixels, 16 bits little-endian, scanned right to left.
pub fn build_mwri(ms: u32, px: &[u16]) -> Vec<u8> {
    let mut f = vec![0u8; 7546];
    put_sync(&mut f, MWRI_SYNC, 48);
    f[8..12].copy_from_slice(&ms.to_be_bytes());
    for c in 0..10 {
        for x in 0..266 {
            let o = 200 + c * 727 + (265 - x) * 2;
            f[o..o + 2].copy_from_slice(&px[c * 266 + x].to_le_bytes());
        }
    }
    f
}

/// Builds an MWRI-2 frame: 26 channels x 492 pixels, 16 bits big-endian.
pub fn build_mwri2(ms: u32, px: &[u16]) -> Vec<u8> {
    let mut f = vec![0u8; 43_000];
    put_sync(&mut f, MWRI2_SYNC, 64);
    f[8..12].copy_from_slice(&ms.to_be_bytes());
    for c in 0..26 {
        for x in 0..492 {
            let o = 1008 + c * 1604 + (491 - x) * 2;
            f[o..o + 2].copy_from_slice(&px[c * 492 + x].to_be_bytes());
        }
    }
    f
}

/// Builds the frames of one MERSI scan: the head frame, then each channel's detector lines, 12 bits a pixel.
pub fn build_mersi_scan(
    l: &MersiLayout,
    ms: u32,
    pixel: impl Fn(usize, usize, usize) -> u16,
) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut head = vec![0u8; l.head_bits.div_ceil(8)];
    put_sync(&mut head, MERSI_HEAD_SYNC, 48);
    head[8..12].copy_from_slice(&ms.to_be_bytes());
    out.push(head);
    let start = l.pixels.0 * 8 + l.pixels.1;
    for line in 0..40 {
        for c in 0..l.ch250 {
            let mut f = vec![0u8; l.scan250_bits.div_ceil(8)];
            put_sync(&mut f, MERSI_SCAN_SYNC, 28);
            f[4] = c as u8 + 1;
            f[5] = line as u8;
            for x in 0..l.width250 {
                put_bits(&mut f, start + x * 12, 12, pixel(c + 1, line, x) as u32);
            }
            out.push(f);
        }
    }
    for line in 0..10 {
        for c in 0..l.ch1k {
            let mut f = vec![0u8; l.scan1k_bits.div_ceil(8)];
            put_sync(&mut f, MERSI_SCAN_SYNC, 28);
            f[4] = (l.ch250 + c) as u8 + 1;
            f[5] = line as u8;
            for x in 0..l.width1k() {
                put_bits(
                    &mut f,
                    start + x * 12,
                    12,
                    pixel(l.ch250 + c + 1, line, x) as u32,
                );
            }
            out.push(f);
        }
    }
    out
}

/// One instrument's decoder of its virtual channel's bit stream.
struct StreamDecoder {
    instrument: Instrument,
    buf: Vec<u8>,
    last_counter: Option<u32>,
    /// Frames found.
    frames: u64,
    /// Rows (lines or scans) emitted.
    rows: u64,
    placer: super::avhrr::LinePlacer,
    picture: u32,
    mersi_scan: BTreeMap<(u16, u8), Vec<u16>>,
    mersi_row: u32,
    mwri_row: u32,
}

/// What a decoder found, per instrument.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct InstrumentStats {
    /// Frames found.
    pub frames: u64,
    /// Lines or scans (images) emitted.
    pub rows: u64,
    /// VCDUs taken (counted instruments).
    pub vcdus: u64,
}

/// The FY-3 instrument decoders of all virtual channels.
#[derive(Default)]
pub struct Fy3Instruments {
    mpt: bool,
    decoders: BTreeMap<u8, StreamDecoder>,
    /// Counters by instrument name.
    pub stats: BTreeMap<String, InstrumentStats>,
    /// The spacecraft heard.
    pub spacecraft: Option<u8>,
    picture: u32,
}

impl Fy3Instruments {
    /// Decoders for the AHRPT (`mpt` false) or MPT product.
    pub fn new(mpt: bool) -> Self {
        Fy3Instruments {
            mpt,
            ..Default::default()
        }
    }

    /// Starts a new picture.
    pub fn new_picture(&mut self) {
        self.picture += 1;
        for d in self.decoders.values_mut() {
            d.placer.restart();
            d.mersi_row = 0;
            d.mwri_row = 0;
            d.picture = self.picture;
        }
    }

    /// Takes a VCDU, appending strips.
    pub fn push(&mut self, v: &Vcdu, out: &mut Vec<Strip>) {
        self.spacecraft = Some(v.spacecraft);
        let Some(ins) = instrument(v.spacecraft, v.vcid, self.mpt) else {
            return;
        };
        let st = self.stats.entry(ins.name().to_string()).or_default();
        st.vcdus += 1;
        if !ins.imaging() {
            return;
        }
        let picture = self.picture;
        let d = self
            .decoders
            .entry(v.vcid)
            .or_insert_with(|| StreamDecoder {
                instrument: ins,
                buf: Vec::new(),
                last_counter: None,
                frames: 0,
                rows: 0,
                placer: Default::default(),
                picture,
                mersi_scan: BTreeMap::new(),
                mersi_row: 0,
                mwri_row: 0,
            });
        if let Some(c) = d.last_counter {
            if (c + 1) & 0xFF_FFFF != v.counter {
                // Lost data: zeros keep a frame in progress at its length.
                let lost = ((v.counter.wrapping_sub(c + 1)) & 0xFF_FFFF).min(64) as usize;
                if !d.buf.is_empty() {
                    d.buf.extend(std::iter::repeat_n(0, lost * v.data().len()));
                }
            }
        }
        d.last_counter = Some(v.counter);
        d.buf.extend_from_slice(v.data());
        d.run(out);
        let st = self.stats.entry(ins.name().to_string()).or_default();
        st.frames = d.frames;
        st.rows = d.rows;
    }
}

impl StreamDecoder {
    fn sync(&self) -> (u64, usize, usize) {
        match self.instrument {
            Instrument::Virr => (VIRR_SYNC, 60, VIRR_LEN),
            Instrument::Mwri => (MWRI_SYNC, 48, 7546),
            Instrument::Mwri2 => (MWRI2_SYNC, 64, 43_000),
            _ => (MERSI_SCAN_SYNC, 28, 0),
        }
    }

    /// Finds the next sync at a byte or bit position; returns the bit offset.
    fn find(&self, sync: u64, bits: usize, from_bit: usize) -> Option<usize> {
        let total = self.buf.len() * 8;
        if total < from_bit + bits {
            return None;
        }
        let mask = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let mut reg = 0u64;
        for b in from_bit..total {
            let bit = (self.buf[b / 8] >> (7 - b % 8)) & 1;
            reg = (reg << 1) | bit as u64;
            if b + 1 - from_bit >= bits && (reg & mask) ^ sync == 0 {
                return Some(b + 1 - bits);
            }
        }
        None
    }

    fn find_mersi(&self) -> Option<(usize, bool)> {
        let total = self.buf.len() * 8;
        let (mut reg, m48, m28) = (0u64, (1u64 << 48) - 1, (1u64 << 28) - 1);
        for b in 0..total {
            let bit = (self.buf[b / 8] >> (7 - b % 8)) & 1;
            reg = (reg << 1) | bit as u64;
            if b + 1 >= 28 && reg & m28 == MERSI_SCAN_SYNC {
                return Some((b + 1 - 28, false));
            }
            if b + 1 >= 48 && reg & m48 == MERSI_HEAD_SYNC {
                return Some((b + 1 - 48, true));
            }
        }
        None
    }

    fn run(&mut self, out: &mut Vec<Strip>) {
        match self.instrument {
            Instrument::Virr | Instrument::Mwri | Instrument::Mwri2 => self.run_fixed(out),
            _ => self.run_mersi(out),
        }
    }

    fn run_fixed(&mut self, out: &mut Vec<Strip>) {
        let (sync, bits, len) = self.sync();
        loop {
            let Some(at) = self.find(sync, bits, 0) else {
                // Keep the tail: it can hold the start of a sync word.
                let keep = bits.div_ceil(8) + 1;
                if self.buf.len() > keep {
                    let n = self.buf.len() - keep;
                    self.buf.drain(..n);
                }
                return;
            };
            let byte = at / 8;
            let shift = at % 8;
            if self.buf.len() < byte + len + 1 {
                if byte > 0 {
                    self.buf.drain(..byte);
                }
                return;
            }
            let frame: Vec<u8> = (0..len)
                .map(|i| {
                    let a = self.buf[byte + i];
                    let b = self.buf.get(byte + i + 1).copied().unwrap_or(0);
                    if shift == 0 {
                        a
                    } else {
                        (a << shift) | (b >> (8 - shift))
                    }
                })
                .collect();
            self.buf.drain(..byte + len);
            self.frames += 1;
            self.decode_fixed(&frame, out);
        }
    }

    fn decode_fixed(&mut self, f: &[u8], out: &mut Vec<Strip>) {
        match self.instrument {
            Instrument::Virr => {
                let ms = u32::from_be_bytes([f[10], f[11], f[12], f[13]]);
                let Some(row) = self.placer.place(ms as f64) else {
                    return;
                };
                if row >= 4000 {
                    return;
                }
                self.rows += 1;
                for c in 0..10usize {
                    let px: Vec<u8> = (0..2048)
                        .map(|x| (get_bits(f, VIRR_PIXELS_BIT + (10 * x + c) * 10, 10) >> 2) as u8)
                        .collect();
                    out.push(strip(self.picture, c as u16 + 1, row, 2048, px, "VIRR"));
                }
            }
            Instrument::Mwri | Instrument::Mwri2 => {
                let row = self.mwri_row;
                self.mwri_row += 1;
                if row >= 4000 {
                    return;
                }
                self.rows += 1;
                let (nch, w, base, stride, le, name) = if self.instrument == Instrument::Mwri {
                    (10, 266, 200, 727, true, "MWRI")
                } else {
                    (26, 492, 1008, 1604, false, "MWRI-2")
                };
                for c in 0..nch {
                    let px: Vec<u8> = (0..w)
                        .map(|x| {
                            let o = base + c * stride + (w - 1 - x) * 2;
                            let v = if le {
                                u16::from_le_bytes([f[o], f[o + 1]])
                            } else {
                                u16::from_be_bytes([f[o], f[o + 1]])
                            };
                            (v >> 8) as u8
                        })
                        .collect();
                    out.push(strip(self.picture, c as u16 + 1, row, w as u32, px, name));
                }
            }
            _ => {}
        }
    }

    fn run_mersi(&mut self, out: &mut Vec<Strip>) {
        let Some(l) = MersiLayout::of(self.instrument) else {
            return;
        };
        loop {
            // The next head frame or scan frame, whichever comes first (one pass over the bits).
            let (at, is_head) = match self.find_mersi() {
                Some(x) => x,
                None => {
                    let keep = 8;
                    if self.buf.len() > keep {
                        let n = self.buf.len() - keep;
                        self.buf.drain(..n);
                    }
                    return;
                }
            };
            let byte = at / 8;
            let shift = at % 8;
            if is_head {
                let len = l.head_bits.div_ceil(8);
                if self.buf.len() < byte + len + 1 {
                    if byte > 0 {
                        self.buf.drain(..byte);
                    }
                    return;
                }
                self.buf.drain(..byte + len);
                self.flush_scan(&l, out);
                continue;
            }
            // A scan frame: its channel decides its length.
            if self.buf.len() < byte + 8 {
                if byte > 0 {
                    self.buf.drain(..byte);
                }
                return;
            }
            let read = |buf: &[u8], i: usize| -> u8 {
                let a = buf[byte + i];
                let b = buf.get(byte + i + 1).copied().unwrap_or(0);
                if shift == 0 {
                    a
                } else {
                    (a << shift) | (b >> (8 - shift))
                }
            };
            let ch = read(&self.buf, 4) as usize;
            let line = read(&self.buf, 5);
            let is250 = ch >= 1 && ch <= l.ch250;
            let bits = if is250 { l.scan250_bits } else { l.scan1k_bits };
            let len = bits.div_ceil(8);
            if self.buf.len() < byte + len + 1 {
                if byte > 0 {
                    self.buf.drain(..byte);
                }
                return;
            }
            let frame: Vec<u8> = (0..len).map(|i| read(&self.buf, i)).collect();
            self.buf.drain(..byte + len);
            self.frames += 1;
            if ch == 0 || ch > l.ch250 + l.ch1k {
                continue;
            }
            let w = if is250 { l.width250 } else { l.width1k() };
            let start = l.pixels.0 * 8 + l.pixels.1;
            let px: Vec<u16> = (0..w)
                .map(|x| get_bits(&frame, start + x * 12, 12) as u16)
                .collect();
            self.mersi_scan.insert((ch as u16, line), px);
        }
    }

    /// Emits the scan collected so far: 10 rows on the 1 km grid per channel (250 m channels averaged 4 x 4).
    fn flush_scan(&mut self, l: &MersiLayout, out: &mut Vec<Strip>) {
        if self.mersi_scan.is_empty() {
            return;
        }
        let w1 = l.width1k();
        let row0 = self.mersi_row * 10;
        self.mersi_row += 1;
        if row0 >= 4000 {
            self.mersi_scan.clear();
            return;
        }
        self.rows += 1;
        let name = self.instrument.name();
        for ch in 1..=(l.ch250 + l.ch1k) as u16 {
            let is250 = (ch as usize) <= l.ch250;
            let mut px = vec![0u8; w1 * 10];
            let mut any = false;
            for r in 0..10usize {
                for x in 0..w1 {
                    let v = if is250 {
                        let mut s = 0u32;
                        let mut n = 0u32;
                        for dy in 0..4 {
                            if let Some(line) = self.mersi_scan.get(&(ch, (r * 4 + dy) as u8)) {
                                for dx in 0..4 {
                                    s += line[x * 4 + dx] as u32;
                                    n += 1;
                                }
                            }
                        }
                        if n == 0 {
                            continue;
                        }
                        (s + n / 2) / n
                    } else {
                        match self.mersi_scan.get(&(ch, r as u8)) {
                            Some(line) => line[x] as u32,
                            None => continue,
                        }
                    };
                    any = true;
                    px[r * w1 + x] = (v >> 4).min(255) as u8;
                }
            }
            if any {
                out.push(Strip {
                    picture: self.picture,
                    channel: ch,
                    first_row: row0,
                    x: 0,
                    rows: 10,
                    width: w1 as u32,
                    samples: Arc::from(px),
                    mask: 1,
                    onboard_s: None,
                    instrument: name,
                });
            }
        }
        self.mersi_scan.clear();
    }
}

fn strip(picture: u32, ch: u16, row: u32, w: u32, px: Vec<u8>, name: &'static str) -> Strip {
    Strip {
        picture,
        channel: ch,
        first_row: row,
        x: 0,
        rows: 1,
        width: w,
        samples: Arc::from(px),
        mask: 1,
        onboard_s: None,
        instrument: name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruments_by_spacecraft_and_vc() {
        assert_eq!(instrument(51, 5, false), Some(Instrument::Virr));
        assert_eq!(instrument(51, 3, true), Some(Instrument::Mersi));
        assert_eq!(instrument(52, 10, false), Some(Instrument::Mwri));
        assert_eq!(instrument(60, 42, false), Some(Instrument::Mwri2));
        assert_eq!(instrument(99, 3, false), Some(Instrument::Mersi2));
        assert_eq!(satellite(56), Some("FY-3E"));
        let l = MersiLayout::of(Instrument::Mersi).unwrap();
        assert!(l.pixels.0 * 8 + l.pixels.1 + 8192 * 12 <= l.scan250_bits);
    }

    // The VIRR pixels fit in the frame.
    const _: () = assert!(VIRR_PIXELS_BIT + 2048 * 10 * 10 <= VIRR_LEN * 8);
}
