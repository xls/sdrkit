//! DVB subtitles: segments, regions, CLUTs, the 2-, 4- and 8-bit pixel code strings, display sets as RGBA and palette-indexed
//! bitmaps, and what the host shows at a given video time.
//!
//! Standard: ETSI EN 300 743.

use crate::pes::circle_diff;
use sdrkit_util_bits::BitReader;
use std::collections::{BTreeMap, VecDeque};

/// One region of a display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubRegion {
    /// Left, in display pixels (the display window included).
    pub x: u32,
    /// Top.
    pub y: u32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Straight (not premultiplied) RGBA, 4 bytes per pixel.
    pub rgba: Vec<u8>,
    /// 8-bit palette indices.
    pub indices: Vec<u8>,
    /// 256 RGBA palette entries.
    pub palette: Vec<[u8; 4]>,
}

/// One display set (the typed item `dvb.subtitle`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleDisplay {
    /// PTS on the stream timeline (90 kHz, 33 bits as sent); `None` shows at once.
    pub pts: Option<i64>,
    /// page_time_out in seconds.
    pub timeout_s: u32,
    /// Display width.
    pub width: u32,
    /// Display height.
    pub height: u32,
    /// A sequence number.
    pub seq: u64,
    /// The regions (empty: a clear).
    pub regions: Vec<SubRegion>,
}

impl SubtitleDisplay {
    /// All regions composed onto a transparent RGBA canvas of the display size.
    pub fn compose(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut c = vec![0u8; w * h * 4];
        for r in &self.regions {
            for yy in 0..r.height as usize {
                let y = r.y as usize + yy;
                if y >= h {
                    break;
                }
                for xx in 0..r.width as usize {
                    let x = r.x as usize + xx;
                    if x >= w {
                        break;
                    }
                    let s = &r.rgba[(yy * r.width as usize + xx) * 4..][..4];
                    if s[3] > 0 {
                        c[(y * w + x) * 4..][..4].copy_from_slice(s);
                    }
                }
            }
        }
        c
    }
}

#[derive(Clone, Debug)]
struct Region {
    version: Option<u8>,
    width: u32,
    height: u32,
    depth: u8,
    clut: u8,
    fill: bool,
    bg: [u8; 3],
    objects: Vec<(u16, u8, u32, u32, u8, u8)>,
    pixels: Vec<u8>,
}

#[derive(Clone, Debug)]
enum Object {
    Pixels { top: Vec<u8>, bottom: Vec<u8> },
    Chars(String),
}

/// The 2-, 4- and 8-bit CLUTs as RGBA.
pub type Cluts = ([[u8; 4]; 4], [[u8; 4]; 16], [[u8; 4]; 256]);

/// The default CLUTs (EN 300 743 clause 10).
pub fn default_clut() -> Cluts {
    let c2 = [
        [0, 0, 0, 0],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
        [127, 127, 127, 255],
    ];
    let mut c4 = [[0u8; 4]; 16];
    for (i, e) in c4.iter_mut().enumerate() {
        let v = if i < 8 { 255 } else { 127 };
        *e = [
            if i & 1 != 0 { v } else { 0 },
            if i & 2 != 0 { v } else { 0 },
            if i & 4 != 0 { v } else { 0 },
            255,
        ];
    }
    c4[0] = [0, 0, 0, 0];
    let mut c8 = [[0u8; 4]; 256];
    for (i, e) in c8.iter_mut().enumerate().skip(1) {
        let b = |m: usize, x: u8| if i & m != 0 { x } else { 0 };
        *e = if i < 8 {
            [b(1, 255), b(2, 255), b(4, 255), 63]
        } else {
            match i & 0x88 {
                0x00 => [
                    b(1, 85) + b(0x10, 170),
                    b(2, 85) + b(0x20, 170),
                    b(4, 85) + b(0x40, 170),
                    255,
                ],
                0x08 => [
                    b(1, 85) + b(0x10, 170),
                    b(2, 85) + b(0x20, 170),
                    b(4, 85) + b(0x40, 170),
                    127,
                ],
                0x80 => [
                    127 + b(1, 43) + b(0x10, 85),
                    127 + b(2, 43) + b(0x20, 85),
                    127 + b(4, 43) + b(0x40, 85),
                    255,
                ],
                _ => [
                    b(1, 43) + b(0x10, 85),
                    b(2, 43) + b(0x20, 85),
                    b(4, 43) + b(0x40, 85),
                    255,
                ],
            }
        };
    }
    (c2, c4, c8)
}

#[derive(Clone, Debug)]
struct Clut {
    version: Option<u8>,
    c2: [[u8; 4]; 4],
    c4: [[u8; 4]; 16],
    c8: [[u8; 4]; 256],
}

impl Default for Clut {
    fn default() -> Self {
        let (c2, c4, c8) = default_clut();
        Clut {
            version: None,
            c2,
            c4,
            c8,
        }
    }
}

/// YCrCbT to straight RGBA (BT.601 studio range, or BT.709). Y = 0 is fully transparent.
pub fn ycrcbt_to_rgba(y: u8, cr: u8, cb: u8, t: u8, bt709: bool) -> [u8; 4] {
    if y == 0 {
        return [0, 0, 0, 0];
    }
    let (y, cr, cb) = (
        1.164 * (y as f32 - 16.0),
        cr as f32 - 128.0,
        cb as f32 - 128.0,
    );
    let (r, g, b) = if bt709 {
        (y + 1.793 * cr, y - 0.534 * cr - 0.213 * cb, y + 2.112 * cb)
    } else {
        (y + 1.596 * cr, y - 0.813 * cr - 0.392 * cb, y + 2.017 * cb)
    };
    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [q(r), q(g), q(b), 255 - t]
}

/// A subtitle decoder for one PID and page.
#[derive(Clone, Debug)]
pub struct SubtitleDecoder {
    /// composition_page_id.
    pub composition_page: u16,
    /// ancillary_page_id.
    pub ancillary_page: u16,
    regions: BTreeMap<u8, Region>,
    page_regions: Vec<(u8, u32, u32)>,
    cluts: BTreeMap<u8, Clut>,
    objects: BTreeMap<u16, Object>,
    timeout: u32,
    display: (u32, u32, u32, u32),
    pts: Option<i64>,
    seq: u64,
    /// Use BT.709 for HD displays.
    pub bt709_for_hd: bool,
    /// Display sets emitted.
    pub display_sets: u64,
}

impl SubtitleDecoder {
    /// A decoder for these pages.
    pub fn new(composition_page: u16, ancillary_page: u16) -> Self {
        SubtitleDecoder {
            composition_page,
            ancillary_page,
            regions: BTreeMap::new(),
            page_regions: Vec::new(),
            cluts: BTreeMap::new(),
            objects: BTreeMap::new(),
            timeout: 0,
            display: (720, 576, 0, 0),
            pts: None,
            seq: 0,
            bt709_for_hd: false,
            display_sets: 0,
        }
    }

    /// A subtitle PES payload (data_identifier 0x20, subtitle_stream_id 0) with its PTS. Returns the display sets it completes.
    pub fn push_pes(&mut self, d: &[u8], pts: Option<i64>) -> Vec<SubtitleDisplay> {
        let mut out = Vec::new();
        if d.len() < 2 || d[0] != 0x20 || d[1] != 0x00 {
            return out;
        }
        self.pts = pts;
        let mut i = 2;
        while i < d.len() {
            if d[i] == 0xFF {
                break;
            }
            if d[i] != 0x0F || i + 6 > d.len() {
                break;
            }
            let ty = d[i + 1];
            let page = u16::from_be_bytes([d[i + 2], d[i + 3]]);
            let len = u16::from_be_bytes([d[i + 4], d[i + 5]]) as usize;
            if i + 6 + len > d.len() {
                break; // a segment past the end stops the walk
            }
            let seg = &d[i + 6..i + 6 + len];
            if page == self.composition_page || page == self.ancillary_page {
                self.segment(ty, seg, &mut out);
            }
            i += 6 + len;
        }
        out
    }

    fn segment(&mut self, ty: u8, s: &[u8], out: &mut Vec<SubtitleDisplay>) {
        match ty {
            0x10 if s.len() >= 2 => {
                self.timeout = s[0] as u32;
                let state = (s[1] >> 2) & 3;
                if state == 1 || state == 2 {
                    self.regions.clear();
                    self.cluts.clear();
                    self.objects.clear();
                }
                self.page_regions.clear();
                for r in s[2..].as_chunks::<6>().0 {
                    self.page_regions.push((
                        r[0],
                        u16::from_be_bytes([r[2], r[3]]) as u32,
                        u16::from_be_bytes([r[4], r[5]]) as u32,
                    ));
                }
            }
            0x11 if s.len() >= 10 => {
                let id = s[0];
                let version = s[1] >> 4;
                let fill = s[1] & 0x08 != 0;
                let width = u16::from_be_bytes([s[2], s[3]]) as u32;
                let height = u16::from_be_bytes([s[4], s[5]]) as u32;
                let depth = (s[6] >> 2) & 7;
                let clut = s[7];
                let bg = [s[9] >> 2 & 3, s[9] >> 4, s[8]];
                if width == 0 || height == 0 || width > 4096 || height > 4096 {
                    self.regions.remove(&id);
                    return;
                }
                let mut objects = Vec::new();
                let mut k = 10;
                while k + 6 <= s.len() {
                    let oid = u16::from_be_bytes([s[k], s[k + 1]]);
                    let otype = s[k + 2] >> 6;
                    let x = (u16::from_be_bytes([s[k + 2], s[k + 3]]) & 0x0FFF) as u32;
                    let y = (u16::from_be_bytes([s[k + 4], s[k + 5]]) & 0x0FFF) as u32;
                    k += 6;
                    let (mut fg, mut bgc) = (1u8, 0u8);
                    if (otype == 1 || otype == 2) && k + 2 <= s.len() {
                        fg = s[k];
                        bgc = s[k + 1];
                        k += 2;
                    }
                    objects.push((oid, otype, x, y, fg, bgc));
                }
                let rebuild = self.regions.get(&id).is_none_or(|r| {
                    r.version != Some(version) || r.width != width || r.height != height
                });
                let depth_code = match depth {
                    1 => 2,
                    2 => 4,
                    _ => 8,
                };
                let r = self.regions.entry(id).or_insert(Region {
                    version: None,
                    width,
                    height,
                    depth: depth_code,
                    clut,
                    fill,
                    bg,
                    objects: Vec::new(),
                    pixels: Vec::new(),
                });
                r.objects = objects;
                r.clut = clut;
                r.depth = depth_code;
                r.fill = fill;
                r.bg = bg;
                if rebuild {
                    r.version = Some(version);
                    r.width = width;
                    r.height = height;
                    let code = if fill {
                        bg[match depth_code {
                            2 => 0,
                            4 => 1,
                            _ => 2,
                        }]
                    } else {
                        0
                    };
                    r.pixels = vec![code; (width * height) as usize];
                    // Objects already received are drawn at once.
                    let objs: Vec<_> = r.objects.clone();
                    for (oid, _, x, y, fg, bgc) in objs {
                        if let Some(o) = self.objects.get(&oid).cloned() {
                            let r = self.regions.get_mut(&id).expect("region");
                            draw(r, &o, x, y, fg, bgc);
                        }
                    }
                }
            }
            0x12 if s.len() >= 2 => {
                let id = s[0];
                let version = s[1] >> 4;
                let hd = self.display.0 > 720 && self.bt709_for_hd;
                let c = self.cluts.entry(id).or_default();
                if c.version == Some(version) {
                    return;
                }
                *c = Clut {
                    version: Some(version),
                    ..Clut::default()
                };
                let mut k = 2;
                while k + 2 <= s.len() {
                    let e = s[k] as usize;
                    let flags = s[k + 1];
                    let (y, cr, cb, t);
                    if flags & 1 != 0 {
                        if k + 6 > s.len() {
                            break;
                        }
                        (y, cr, cb, t) = (s[k + 2], s[k + 3], s[k + 4], s[k + 5]);
                        k += 6;
                    } else {
                        if k + 4 > s.len() {
                            break;
                        }
                        let v = u16::from_be_bytes([s[k + 2], s[k + 3]]);
                        y = ((v >> 10) as u8) << 2;
                        cr = (((v >> 6) & 0xF) as u8) << 4;
                        cb = (((v >> 2) & 0xF) as u8) << 4;
                        t = (v & 3) as u8 * 85;
                        k += 4;
                    }
                    let rgba = ycrcbt_to_rgba(y, cr, cb, t, hd);
                    if flags & 0x80 != 0 && e < 4 {
                        c.c2[e] = rgba;
                    }
                    if flags & 0x40 != 0 && e < 16 {
                        c.c4[e] = rgba;
                    }
                    if flags & 0x20 != 0 {
                        c.c8[e] = rgba;
                    }
                }
            }
            0x13 if s.len() >= 3 => {
                let id = u16::from_be_bytes([s[0], s[1]]);
                let method = (s[2] >> 2) & 3;
                let obj = if method == 0 && s.len() >= 7 {
                    let tl = u16::from_be_bytes([s[3], s[4]]) as usize;
                    let bl = u16::from_be_bytes([s[5], s[6]]) as usize;
                    let top = s[7..(7 + tl).min(s.len())].to_vec();
                    let bottom = s[(7 + tl).min(s.len())..(7 + tl + bl).min(s.len())].to_vec();
                    Object::Pixels { top, bottom }
                } else if method == 1 && s.len() >= 4 {
                    let n = s[3] as usize;
                    let chars: String = s[4..]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .take(n)
                        .map(|c| {
                            char::from_u32(u16::from_be_bytes([c[0], c[1]]) as u32).unwrap_or(' ')
                        })
                        .collect();
                    Object::Chars(chars)
                } else {
                    return;
                };
                self.objects.insert(id, obj.clone());
                let ids: Vec<u8> = self.regions.keys().copied().collect();
                for rid in ids {
                    let r = self.regions.get_mut(&rid).expect("region");
                    let uses: Vec<_> = r.objects.iter().filter(|o| o.0 == id).copied().collect();
                    for (_, _, x, y, fg, bg) in uses {
                        draw(r, &obj, x, y, fg, bg);
                    }
                }
            }
            0x14 if s.len() >= 5 => {
                let window = s[0] & 0x08 != 0;
                let mut w = u16::from_be_bytes([s[1], s[2]]) as u32 + 1;
                let mut h = u16::from_be_bytes([s[3], s[4]]) as u32 + 1;
                if !(1..=4096).contains(&w) || !(1..=4096).contains(&h) {
                    (w, h) = (720, 576);
                }
                let (mut ox, mut oy) = (0, 0);
                if window && s.len() >= 13 {
                    ox = u16::from_be_bytes([s[5], s[6]]) as u32;
                    oy = u16::from_be_bytes([s[9], s[10]]) as u32;
                }
                self.display = (w, h, ox, oy);
            }
            0x80 => {
                out.push(self.emit());
            }
            _ => {}
        }
    }

    fn emit(&mut self) -> SubtitleDisplay {
        self.seq += 1;
        self.display_sets += 1;
        let (w, h, ox, oy) = self.display;
        let mut regions = Vec::new();
        for &(id, x, y) in &self.page_regions {
            let Some(r) = self.regions.get(&id) else {
                continue;
            };
            let clut = self.cluts.get(&r.clut).cloned().unwrap_or_default();
            let mut palette = vec![[0u8; 4]; 256];
            match r.depth {
                2 => palette[..4].copy_from_slice(&clut.c2),
                4 => palette[..16].copy_from_slice(&clut.c4),
                _ => palette.copy_from_slice(&clut.c8),
            }
            let mut rgba = Vec::with_capacity(r.pixels.len() * 4);
            for &p in &r.pixels {
                rgba.extend_from_slice(&palette[p as usize]);
            }
            regions.push(SubRegion {
                x: x + ox,
                y: y + oy,
                width: r.width,
                height: r.height,
                rgba,
                indices: r.pixels.clone(),
                palette,
            });
        }
        SubtitleDisplay {
            pts: self.pts,
            timeout_s: self.timeout,
            width: w,
            height: h,
            seq: self.seq,
            regions,
        }
    }
}

/// Draws an object into a region (both fields, interleaved; the top field repeated when the bottom one is empty).
fn draw(r: &mut Region, o: &Object, x: u32, y: u32, fg: u8, bg: u8) {
    match o {
        Object::Pixels { top, bottom } => {
            draw_field(r, top, x, y);
            if bottom.is_empty() {
                draw_field(r, top, x, y + 1);
            } else {
                draw_field(r, bottom, x, y + 1);
            }
        }
        Object::Chars(s) => {
            let mut cx = x;
            for ch in s.chars() {
                let g = crate::font::glyph(ch);
                for row in 0..8u32 {
                    for col in 0..6u32 {
                        let on = col < 5 && row < 7 && (g[col as usize] >> row) & 1 == 1;
                        put(r, cx + col, y + row, if on { fg } else { bg });
                    }
                }
                cx += 6;
            }
        }
    }
}

fn put(r: &mut Region, x: u32, y: u32, v: u8) {
    if x < r.width && y < r.height {
        r.pixels[(y * r.width + x) as usize] = v;
    }
}

/// Decodes one field's pixel data sub-blocks (EN 300 743 7.2.5) into every second line from `y`.
#[allow(clippy::needless_range_loop)]
fn draw_field(r: &mut Region, data: &[u8], x0: u32, y0: u32) {
    let mut map24: [u8; 4] = [0, 7, 8, 15];
    let mut map28: [u8; 4] = [0x00, 0x77, 0x88, 0xFF];
    let mut map48: [u8; 16] = std::array::from_fn(|i| (i * 17) as u8);
    let (mut x, mut y) = (x0, y0);
    let mut i = 0;
    let depth = r.depth;
    while i < data.len() {
        let ty = data[i];
        i += 1;
        match ty {
            0x10..=0x12 => {
                let mut br = BitReader::new(&data[i..]);
                loop {
                    let (run, code) = match ty {
                        0x10 => two_bit(&mut br),
                        0x11 => four_bit(&mut br),
                        _ => eight_bit(&mut br),
                    };
                    if br.overrun() {
                        break;
                    }
                    if run == 0 {
                        break; // end of string
                    }
                    let v = match (ty, depth) {
                        (0x10, 4) => map24[code as usize & 3],
                        (0x10, 8) => map28[code as usize & 3],
                        (0x11, 8) => map48[code as usize & 15],
                        (0x11, 2) => code >> 2,
                        (0x12, 2) => code >> 6,
                        (0x12, 4) => code >> 4,
                        _ => code,
                    };
                    for _ in 0..run {
                        put(r, x, y, v);
                        x += 1;
                    }
                }
                i += br.pos().div_ceil(8);
            }
            0x20 if i + 2 <= data.len() => {
                let v = u16::from_be_bytes([data[i], data[i + 1]]);
                for k in 0..4 {
                    map24[k] = ((v >> (12 - 4 * k)) & 0xF) as u8;
                }
                i += 2;
            }
            0x21 if i + 4 <= data.len() => {
                map28.copy_from_slice(&data[i..i + 4]);
                i += 4;
            }
            0x22 if i + 8 <= data.len() => {
                for k in 0..8 {
                    map48[2 * k] = data[i + k] >> 4;
                    map48[2 * k + 1] = data[i + k] & 0xF;
                }
                i += 8;
            }
            0xF0 => {
                x = x0;
                y += 2;
            }
            _ => break, // an unknown type stops the field
        }
    }
}

/// One 2-bit code: (run, code); run 0 ends the string.
fn two_bit(b: &mut BitReader) -> (u32, u8) {
    let c = b.u(2) as u8;
    if c != 0 {
        return (1, c);
    }
    if b.bit() {
        let run = (b.u(3) as u32) + 3;
        return (run, b.u(2) as u8);
    }
    if b.bit() {
        return (1, 0);
    }
    match b.u(2) {
        0 => (0, 0),
        1 => (2, 0),
        2 => {
            let run = (b.u(4) as u32) + 12;
            (run, b.u(2) as u8)
        }
        _ => {
            let run = (b.u(8) as u32) + 29;
            (run, b.u(2) as u8)
        }
    }
}

/// One 4-bit code: (run, code); run 0 ends the string.
fn four_bit(b: &mut BitReader) -> (u32, u8) {
    let c = b.u(4) as u8;
    if c != 0 {
        return (1, c);
    }
    if !b.bit() {
        let n = b.u(3) as u32;
        return if n == 0 { (0, 0) } else { (n + 2, 0) };
    }
    if !b.bit() {
        let run = (b.u(2) as u32) + 4;
        return (run, b.u(4) as u8);
    }
    match b.u(2) {
        0 => (1, 0),
        1 => (2, 0),
        2 => {
            let run = (b.u(4) as u32) + 9;
            (run, b.u(4) as u8)
        }
        _ => {
            let run = (b.u(8) as u32) + 25;
            (run, b.u(4) as u8)
        }
    }
}

/// One 8-bit code: (run, code); run 0 ends the string.
fn eight_bit(b: &mut BitReader) -> (u32, u8) {
    let c = b.u(8) as u8;
    if c != 0 {
        return (1, c);
    }
    if !b.bit() {
        let n = b.u(7) as u32;
        (n, 0)
    } else {
        let n = b.u(7) as u32;
        (n, b.u(8) as u8)
    }
}

/// What the host shows at video time `t` (ETSI EN 300 743 5.1).
#[derive(Clone, Debug, Default)]
pub struct SubtitlePresenter {
    queue: VecDeque<SubtitleDisplay>,
    shown: Option<(u64, i64)>,
}

/// The display queue length.
pub const QUEUE: usize = 24;
/// The minimum time a display stays up, in 90 kHz units.
pub const MIN_HOLD: i64 = 135_000;

impl SubtitlePresenter {
    /// A new presenter.
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds a display (a display without PTS is stamped with `now`, so it shows at once).
    pub fn push(&mut self, mut d: SubtitleDisplay, now: i64) {
        if d.pts.is_none() {
            d.pts = Some(now);
        }
        self.queue.push_back(d);
        while self.queue.len() > QUEUE {
            self.queue.pop_front();
        }
    }
    /// Forgets everything (a new PID or page).
    pub fn reset(&mut self) {
        self.queue.clear();
        self.shown = None;
    }
    /// The display to show at video PTS `t` (33-bit circle).
    pub fn at(&mut self, t: i64) -> Option<&SubtitleDisplay> {
        let due = self
            .queue
            .iter()
            .enumerate()
            .filter(|(_, d)| circle_diff(t, d.pts.unwrap_or(t)) >= 0)
            .max_by_key(|(_, d)| d.pts.map(|p| circle_diff(p, t)).unwrap_or(0))
            .map(|(i, _)| i);
        // A display already shown stays up at least 1.5 s.
        if let Some((seq, first)) = self.shown {
            if let Some(k) = self.queue.iter().position(|d| d.seq == seq) {
                let held = circle_diff(t, first);
                let newer_with_regions =
                    due.is_some_and(|i| i > k && !self.queue[i].regions.is_empty());
                if (0..MIN_HOLD).contains(&held) && !newer_with_regions {
                    return self.queue.get(k);
                }
            }
        }
        let i = due?;
        let d = &self.queue[i];
        let pts = d.pts.unwrap_or(t);
        let expired = d.timeout_s > 0 && circle_diff(t, pts) > d.timeout_s as i64 * 90_000;
        if d.regions.is_empty() || expired {
            return None;
        }
        if self.shown.map(|s| s.0) != Some(d.seq) {
            self.shown = Some((d.seq, t));
        }
        Some(&self.queue[i])
    }
}

/// Builds a segment (for generators and tests).
pub fn build_segment(ty: u8, page: u16, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0x0F, ty];
    v.extend_from_slice(&page.to_be_bytes());
    v.extend_from_slice(&(body.len() as u16).to_be_bytes());
    v.extend_from_slice(body);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct W(Vec<bool>);
    impl W {
        fn u(&mut self, n: u32, v: u32) {
            for i in (0..n).rev() {
                self.0.push((v >> i) & 1 == 1);
            }
        }
        fn bytes(&self) -> Vec<u8> {
            self.0
                .chunks(8)
                .map(|c| {
                    c.iter()
                        .enumerate()
                        .fold(0u8, |a, (i, &b)| a | ((b as u8) << (7 - i)))
                })
                .collect()
        }
    }

    fn page(timeout: u8, state: u8, regions: &[(u8, u16, u16)]) -> Vec<u8> {
        let mut b = vec![timeout, (state << 2)];
        for &(id, x, y) in regions {
            b.push(id);
            b.push(0);
            b.extend_from_slice(&x.to_be_bytes());
            b.extend_from_slice(&y.to_be_bytes());
        }
        b
    }

    fn region(
        id: u8,
        version: u8,
        w: u16,
        h: u16,
        depth: u8,
        objects: &[(u16, u16, u16)],
    ) -> Vec<u8> {
        let mut b = vec![id, version << 4 | 0x08];
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.push((depth << 5) | (depth << 2));
        b.push(1);
        b.push(0);
        b.push(0);
        for &(oid, x, y) in objects {
            b.extend_from_slice(&oid.to_be_bytes());
            b.extend_from_slice(&x.to_be_bytes());
            b.extend_from_slice(&y.to_be_bytes());
        }
        b
    }

    fn pes(segs: &[Vec<u8>]) -> Vec<u8> {
        let mut v = vec![0x20, 0x00];
        for s in segs {
            v.extend_from_slice(s);
        }
        v.push(0xFF);
        v
    }

    #[test]
    fn four_bit_region_and_runs() {
        // 40 x 6, 4-bit, at (100, 200). Each line: runs covering every 4-bit code length.
        let mut w = W::default();
        w.u(4, 5); // 1 pixel of 5
        w.u(4, 0);
        w.u(1, 0);
        w.u(3, 3); // 5 pixels of 0
        w.u(4, 0);
        w.u(1, 1);
        w.u(1, 0);
        w.u(2, 2);
        w.u(4, 9); // 6 pixels of 9
        w.u(4, 0);
        w.u(1, 1);
        w.u(1, 1);
        w.u(2, 2);
        w.u(4, 3);
        w.u(4, 7); // 12 pixels of 7
        w.u(4, 0);
        w.u(1, 1);
        w.u(1, 1);
        w.u(2, 0); // 1 pixel of 0
        w.u(4, 0);
        w.u(1, 1);
        w.u(1, 1);
        w.u(2, 1); // 2 pixels of 0
        w.u(4, 13); // 1 pixel of 13: total 1+5+6+12+1+2+1 = 28
        w.u(4, 0);
        w.u(1, 0);
        w.u(3, 0); // end
        let line = w.bytes();
        let mut field = Vec::new();
        for _ in 0..3 {
            field.push(0x11);
            field.extend_from_slice(&line);
            field.push(0xF0);
        }
        let mut obj = vec![0, 1, 0x00];
        obj.extend_from_slice(&(field.len() as u16).to_be_bytes());
        obj.extend_from_slice(&0u16.to_be_bytes());
        obj.extend_from_slice(&field);
        let mut dds = vec![0x00];
        dds.extend_from_slice(&1279u16.to_be_bytes());
        dds.extend_from_slice(&719u16.to_be_bytes());
        // CLUT 1: entry 9 in the 4-bit table set to white, full range.
        let clut = vec![1, 0x00, 9, 0x40 | 1, 235, 128, 128, 0];
        let segs = vec![
            build_segment(0x14, 1, &dds),
            build_segment(0x10, 1, &page(5, 1, &[(0, 100, 200)])),
            build_segment(0x11, 1, &region(0, 0, 40, 6, 2, &[(1, 0, 0)])),
            build_segment(0x12, 2, &clut),
            build_segment(0x13, 2, &obj),
            build_segment(0x80, 1, &[]),
        ];
        let mut d = SubtitleDecoder::new(1, 2);
        let out = d.push_pes(&pes(&segs), Some(9000));
        assert_eq!(out.len(), 1);
        let s = &out[0];
        assert_eq!((s.width, s.height), (1280, 720));
        assert_eq!(s.regions.len(), 1);
        let r = &s.regions[0];
        assert_eq!((r.x, r.y, r.width, r.height), (100, 200, 40, 6));
        let expect: Vec<u8> = [
            vec![5],
            vec![0; 5],
            vec![9; 6],
            vec![7; 12],
            vec![0],
            vec![0; 2],
            vec![13],
        ]
        .concat();
        for line in 0..6 {
            assert_eq!(
                &r.indices[line * 40..line * 40 + 28],
                &expect[..],
                "line {line}"
            );
        }
        assert_eq!(&r.rgba[(6) * 4..(6) * 4 + 4], &[255, 255, 255, 255]);
        // The default 4-bit entry 5 (magenta, full intensity).
        assert_eq!(&r.rgba[..4], &[255, 0, 255, 255]);
    }

    #[test]
    fn two_and_eight_bit_strings_long_runs() {
        let mut w = W::default();
        w.u(2, 0);
        w.u(1, 0);
        w.u(1, 0);
        w.u(2, 3);
        w.u(8, 251);
        w.u(2, 1); // 280 pixels of 1
        w.u(2, 0);
        w.u(1, 1);
        w.u(3, 7);
        w.u(2, 2); // 10 pixels of 2
        w.u(2, 0);
        w.u(1, 0);
        w.u(1, 0);
        w.u(2, 2);
        w.u(4, 13);
        w.u(2, 3); // 25 pixels of 3
        w.u(2, 0);
        w.u(1, 0);
        w.u(1, 0);
        w.u(2, 0);
        let two = w.bytes();
        let mut r = Region {
            version: None,
            width: 400,
            height: 2,
            depth: 2,
            clut: 0,
            fill: false,
            bg: [0; 3],
            objects: vec![],
            pixels: vec![0; 800],
        };
        let mut field = vec![0x10];
        field.extend(two);
        draw_field(&mut r, &field, 0, 0);
        assert!(r.pixels[..280].iter().all(|&p| p == 1));
        assert!(r.pixels[280..290].iter().all(|&p| p == 2));
        assert!(r.pixels[290..315].iter().all(|&p| p == 3));
        let mut w = W::default();
        w.u(8, 0);
        w.u(1, 1);
        w.u(7, 127);
        w.u(8, 200); // 127 of 200
        w.u(8, 0);
        w.u(1, 0);
        w.u(7, 3); // 3 of 0
        w.u(8, 17); // 1 of 17
        w.u(8, 0);
        w.u(1, 0);
        w.u(7, 0);
        let mut r = Region {
            version: None,
            width: 200,
            height: 1,
            depth: 8,
            clut: 0,
            fill: false,
            bg: [0; 3],
            objects: vec![],
            pixels: vec![9; 200],
        };
        let mut field = vec![0x12];
        field.extend(w.bytes());
        draw_field(&mut r, &field, 0, 0);
        assert!(r.pixels[..127].iter().all(|&p| p == 200));
        assert_eq!(&r.pixels[127..131], &[0, 0, 0, 17]);
    }

    #[test]
    fn presentation_timing() {
        let disp = |seq: u64, pts: i64, regions: bool| SubtitleDisplay {
            pts: Some(pts),
            timeout_s: 3,
            width: 720,
            height: 576,
            seq,
            regions: if regions {
                vec![SubRegion {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    rgba: vec![255; 4],
                    indices: vec![1],
                    palette: vec![[0; 4]; 256],
                }]
            } else {
                vec![]
            },
        };
        let mut p = SubtitlePresenter::new();
        let base = crate::pes::WRAP_33 - 45_000; // half a second before the wrap
        p.push(disp(1, base, true), 0);
        assert!(p.at(base - 1).is_none());
        assert_eq!(p.at(base).unwrap().seq, 1);
        // A clear 0.5 s later: the 1.5 s hold keeps the display up.
        p.push(disp(2, (base + 45_000) % crate::pes::WRAP_33, false), 0);
        assert_eq!(p.at((base + 90_000) % crate::pes::WRAP_33).unwrap().seq, 1);
        assert!(p.at((base + 150_000) % crate::pes::WRAP_33).is_none());
        // Timeout: 3 s.
        let mut p = SubtitlePresenter::new();
        p.push(disp(3, 1000, true), 0);
        assert!(p.at(1000 + 2 * 90_000).is_some());
        assert!(p.at(1000 + 4 * 90_000).is_none());
    }

    #[test]
    fn state_and_other_pages() {
        let mut w = W::default();
        w.u(2, 1);
        w.u(2, 0);
        w.u(1, 0);
        w.u(1, 0);
        w.u(2, 0);
        let mut field = vec![0x10];
        field.extend(w.bytes());
        field.push(0xF0);
        let mut obj = vec![0, 7, 0x00];
        obj.extend_from_slice(&(field.len() as u16).to_be_bytes());
        obj.extend_from_slice(&0u16.to_be_bytes());
        obj.extend_from_slice(&field);
        let mut d = SubtitleDecoder::new(1, 1);
        let first = d.push_pes(
            &pes(&[
                build_segment(0x10, 1, &page(0, 1, &[(3, 0, 0)])),
                build_segment(0x11, 1, &region(3, 0, 4, 2, 1, &[(7, 0, 0)])),
                build_segment(0x13, 1, &obj),
                build_segment(0x80, 1, &[]),
            ]),
            Some(0),
        );
        // A 2-bit region with only a top field: the line is repeated.
        assert_eq!(first[0].regions[0].indices, vec![1, 0, 0, 0, 1, 0, 0, 0]);
        // State 0 keeps the region; a page with no regions clears.
        let again = d.push_pes(
            &pes(&[
                build_segment(0x10, 1, &page(0, 0, &[(3, 0, 0)])),
                build_segment(0x80, 1, &[]),
            ]),
            Some(1),
        );
        assert_eq!(again[0].regions.len(), 1);
        let clear = d.push_pes(
            &pes(&[
                build_segment(0x10, 1, &page(0, 0, &[])),
                build_segment(0x80, 1, &[]),
            ]),
            Some(2),
        );
        assert!(clear[0].regions.is_empty());
        // Other pages are ignored.
        let other = d.push_pes(
            &pes(&[
                build_segment(0x10, 9, &page(0, 1, &[(3, 0, 0)])),
                build_segment(0x80, 9, &[]),
            ]),
            Some(3),
        );
        assert!(other.is_empty());
    }
}
