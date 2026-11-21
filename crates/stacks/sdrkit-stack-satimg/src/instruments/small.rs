//! The smaller packet instruments of `eodb`: AMSR2 (GCOM-W1), METimage (MetOp-SG), TIRS (Landsat) and VEGETATION (Proba).
//! No public document confirms the packet layouts used here.

use super::direct::{PacketIn, PacketInstrument, TestPacket, TestSource};
use super::planes::{bits_at, put_bits, test_pixel, ChannelSpec, Planes};

/// Writes `v` × `scale` (at most 65535) at (`col`, `row`) of channel `ch`.
fn put(p: &mut Planes, ch: usize, col: usize, row: usize, v: u32, scale: u32) {
    p.set(ch, col, row, (v * scale).min(65_535) as u16);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// AMSR2.

/// The APID of AMSR2.
pub const AMSR2_APID: u16 = 1576;
/// Columns of an AMSR2 line.
pub const AMSR2_WIDTH: usize = 243;
/// Pixels of an AMSR2 packet.
pub const AMSR2_PIXELS: usize = 25;
/// Packets of an AMSR2 line at the most.
pub const AMSR2_PACKETS: usize = 20;

/// AMSR2: 20 channels of 243 columns.
pub struct Amsr2 {
    planes: Planes,
    line: usize,
    k: usize,
    used: u64,
}

impl Amsr2 {
    /// A decoder of at most `max_rows` lines.
    pub fn new(max_rows: usize) -> Self {
        const NAMES: [&str; 20] = [
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20",
        ];
        let specs: Vec<ChannelSpec> = NAMES.iter().map(|&n| (n, 0.0, "AMSR2", AMSR2_WIDTH)).collect();
        Amsr2 {
            planes: Planes::new(&specs, max_rows),
            line: 0,
            k: 0,
            used: 0,
        }
    }
}

impl PacketInstrument for Amsr2 {
    fn push(&mut self, p: &PacketIn<'_>) {
        if p.apid != AMSR2_APID || p.data.len() < 1018 {
            return;
        }
        if self.k < AMSR2_PACKETS {
            for i in 0..AMSR2_PIXELS {
                let col = self.k * AMSR2_PIXELS + i;
                if col >= AMSR2_WIDTH {
                    break;
                }
                for c in 0..20 {
                    let at = 10 + 2 * (20 * i + c);
                    let w = u32::from(u16::from_be_bytes([p.data[at], p.data[at + 1]]));
                    let low = w & 0x7FF;
                    let v = if w & 0x800 != 0 { low } else { 2048 + low };
                    put(&mut self.planes, c, col, self.line, v, 16);
                }
            }
            self.used += 1;
        }
        self.k += 1;
        if p.flags == 1 {
            self.line += 1;
            self.k = 0;
        }
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn used(&self) -> u64 {
        self.used
    }
    fn clear(&mut self) {
        self.planes.clear();
        self.line = 0;
        self.k = 0;
    }
}

/// The AMSR2 test packets: 10 packets a line, the last with flags 01.
#[derive(Default)]
pub struct Amsr2Source {
    line: usize,
    k: usize,
}

impl TestSource for Amsr2Source {
    fn next(&mut self, out: &mut Vec<TestPacket>) {
        let mut d = vec![0u8; 1018];
        for i in 0..AMSR2_PIXELS {
            let col = self.k * AMSR2_PIXELS + i;
            for c in 0..20 {
                let v = test_pixel(c, col, self.line);
                let w = if v < 2048 { 0x800 | v } else { v - 2048 };
                let at = 10 + 2 * (20 * i + c);
                d[at..at + 2].copy_from_slice(&w.to_be_bytes());
            }
        }
        let last = (self.k + 1) * AMSR2_PIXELS >= AMSR2_WIDTH;
        out.push(TestPacket {
            apid: AMSR2_APID,
            flags: if last { 1 } else { 0 },
            data: d,
        });
        if last {
            self.k = 0;
            self.line += 1;
        } else {
            self.k += 1;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// METimage.

/// The APIDs of METimage.
pub const METIMAGE_APIDS: [u16; 2] = [1443, 1444];
/// Columns of a finished METimage scan.
pub const METIMAGE_WIDTH: usize = 3192;
/// Columns of a scan before the turn.
pub const METIMAGE_SCAN: usize = 3248;
/// Detector lines of a scan.
pub const METIMAGE_LINES: usize = 24;
/// Columns of a packet.
pub const METIMAGE_COLUMNS: usize = 56;

/// The channel marker table: marker, channel (1 to 20), bits of a value, detectors in reverse order.
pub const METIMAGE_MARKERS: [(u8, usize, u32, bool); 20] = [
    (30, 1, 18, true),
    (28, 2, 18, true),
    (26, 3, 18, true),
    (25, 4, 17, true),
    (23, 5, 17, true),
    (24, 6, 17, true),
    (22, 7, 17, true),
    (4, 8, 17, false),
    (12, 9, 17, false),
    (8, 10, 17, false),
    (17, 11, 17, false),
    (15, 12, 18, false),
    (16, 13, 18, false),
    (20, 14, 18, false),
    (40, 15, 18, false),
    (39, 16, 18, false),
    (37, 17, 17, false),
    (34, 18, 17, false),
    (35, 19, 17, false),
    (33, 20, 17, false),
];

/// METimage: 20 channels of 3192 columns, 24 lines a scan.
pub struct Metimage {
    planes: Planes,
    scan: Vec<Vec<u16>>,
    scans: usize,
    open: bool,
    used: u64,
}

impl Metimage {
    /// A decoder of at most `max_rows` lines.
    pub fn new(max_rows: usize) -> Self {
        const NAMES: [&str; 20] = [
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20",
        ];
        let specs: Vec<ChannelSpec> = NAMES.iter().map(|&n| (n, 0.0, "METimage", METIMAGE_WIDTH)).collect();
        Metimage {
            planes: Planes::new(&specs, max_rows),
            scan: vec![vec![0; METIMAGE_SCAN * METIMAGE_LINES]; 20],
            scans: 0,
            open: false,
            used: 0,
        }
    }

    fn end_scan(&mut self) {
        if !self.open {
            return;
        }
        for (c, s) in self.scan.iter_mut().enumerate() {
            for l in 0..METIMAGE_LINES {
                let row = self.scans * METIMAGE_LINES + l;
                if let Some(r) = self.planes.pixels_mut(c, row) {
                    for (x, o) in r.iter_mut().enumerate() {
                        *o = s[l * METIMAGE_SCAN + METIMAGE_SCAN - 1 - METIMAGE_COLUMNS - x];
                    }
                }
            }
            s.fill(0);
        }
        self.scans += 1;
        self.open = false;
    }
}

impl PacketInstrument for Metimage {
    fn push(&mut self, p: &PacketIn<'_>) {
        if !METIMAGE_APIDS.contains(&p.apid) || p.data.len() < 21 {
            return;
        }
        let d = p.data;
        let Some(&(marker, ch, bits, rev)) = METIMAGE_MARKERS.iter().find(|m| m.0 == d[15]) else {
            return;
        };
        let need = if bits == 17 { 3021 } else { 3196 };
        let offset = usize::from(u16::from_be_bytes([d[19], d[20]]));
        if d.len() < need || offset + METIMAGE_COLUMNS > METIMAGE_SCAN {
            return;
        }
        if offset == 0 && marker == 30 {
            self.end_scan();
        }
        self.open = true;
        let s = &mut self.scan[ch - 1];
        for i in 0..METIMAGE_COLUMNS {
            for l in 0..METIMAGE_LINES {
                let n = 25 * i + 1 + if rev { 23 - l } else { l };
                let v = bits_at(d, 8 * 44 + n * bits as usize, bits) >> (bits - 16);
                s[l * METIMAGE_SCAN + offset + i] = v as u16;
            }
        }
        self.used += 1;
    }
    fn finish(&mut self) {
        self.end_scan();
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn used(&self) -> u64 {
        self.used
    }
    fn clear(&mut self) {
        self.planes.clear();
        self.scans = 0;
    }
}

/// The 16-bit test value of METimage channel `ch` (1 to 20) at the finished column `x` and line `y`.
pub fn metimage_test_value(ch: usize, x: usize, y: usize) -> u16 {
    test_pixel(ch - 1, x, y) * 16
}

/// The METimage test packets: scan after scan, for each column offset each channel's packet.
#[derive(Default)]
pub struct MetimageSource {
    scan: usize,
    offset: usize,
}

impl TestSource for MetimageSource {
    fn next(&mut self, out: &mut Vec<TestPacket>) {
        for &(marker, ch, bits, rev) in &METIMAGE_MARKERS {
            let len = if bits == 17 { 3021 } else { 3196 };
            let mut d = vec![0u8; len];
            d[15] = marker;
            d[19..21].copy_from_slice(&(self.offset as u16).to_be_bytes());
            for i in 0..METIMAGE_COLUMNS {
                // The finished picture is turned left to right with the first 56 columns dropped.
                let col = self.offset + i;
                let x = (METIMAGE_SCAN - 1 - METIMAGE_COLUMNS).checked_sub(col);
                for l in 0..METIMAGE_LINES {
                    let v = x.map_or(0, |x| metimage_test_value(ch, x, self.scan * METIMAGE_LINES + l));
                    let n = 25 * i + 1 + if rev { 23 - l } else { l };
                    put_bits(&mut d, 8 * 44 + n * bits as usize, bits, u32::from(v) << (bits - 16));
                }
            }
            out.push(TestPacket {
                apid: METIMAGE_APIDS[0],
                flags: 3,
                data: d,
            });
        }
        self.offset += METIMAGE_COLUMNS;
        if self.offset + METIMAGE_COLUMNS > METIMAGE_SCAN {
            self.offset = 0;
            self.scan += 1;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// TIRS.

/// The first APID of TIRS.
pub const TIRS_APID: u16 = 1792;
/// Columns of a TIRS line.
pub const TIRS_WIDTH: usize = 1280;

/// TIRS: 3 pictures of 1280 columns, one row a packet.
pub struct Tirs {
    planes: Planes,
    rows: [usize; 3],
    used: u64,
}

impl Tirs {
    /// A decoder of at most `max_rows` lines.
    pub fn new(max_rows: usize) -> Self {
        let specs: Vec<ChannelSpec> =
            vec![("1", 10.9, "TIRS", TIRS_WIDTH), ("2", 12.0, "TIRS", TIRS_WIDTH), ("3", 12.0, "TIRS", TIRS_WIDTH)];
        Tirs {
            planes: Planes::new(&specs, max_rows),
            rows: [0; 3],
            used: 0,
        }
    }
}

fn word12(d: &[u8], at: usize, k: usize) -> u32 {
    bits_at(d, 8 * at + 12 * k, 12)
}

impl PacketInstrument for Tirs {
    fn push(&mut self, p: &PacketIn<'_>) {
        let r = usize::from(p.apid.wrapping_sub(TIRS_APID));
        if r >= 3 || p.data.len() < 5848 {
            return;
        }
        let row = self.rows[r];
        for i in 0..640 {
            put(&mut self.planes, r, 2 * i, row, word12(p.data, 31, i), 16);
            put(&mut self.planes, r, 2 * i + 1, row, word12(p.data, 31, 1941 + i), 16);
        }
        self.rows[r] += 1;
        self.used += 1;
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn used(&self) -> u64 {
        self.used
    }
    fn clear(&mut self) {
        self.planes.clear();
        self.rows = [0; 3];
    }
}

/// The TIRS test packets: one row of each picture in turn.
#[derive(Default)]
pub struct TirsSource {
    row: usize,
}

impl TestSource for TirsSource {
    fn next(&mut self, out: &mut Vec<TestPacket>) {
        for r in 0..3 {
            let mut d = vec![0u8; 5848];
            for i in 0..640 {
                put_bits(&mut d, 8 * 31 + 12 * i, 12, u32::from(test_pixel(r, 2 * i, self.row)));
                put_bits(&mut d, 8 * 31 + 12 * (1941 + i), 12, u32::from(test_pixel(r, 2 * i + 1, self.row)));
            }
            out.push(TestPacket {
                apid: TIRS_APID + r as u16,
                flags: 3,
                data: d,
            });
        }
        self.row += 1;
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// VEGETATION.

/// The APIDs of channels 1 to 6 of the groups VEG-1, VEG-2 and VEG-3.
pub const VEG_APIDS: [[u16; 6]; 3] = [
    [290, 274, 322, 282, 298, 330],
    [289, 273, 321, 281, 297, 329],
    [292, 276, 324, 284, 300, 332],
];

/// VEGETATION: 3 groups of 6 channels; channels 1 to 3 of 5200 columns, 4 to 6 of 1024.
pub struct Vegetation {
    planes: Planes,
    rows: [usize; 18],
    used: u64,
}

/// The columns of channel `c` (0 to 5).
pub fn veg_width(c: usize) -> usize {
    if c < 3 {
        5200
    } else {
        1024
    }
}

impl Vegetation {
    /// A decoder of at most `max_rows` lines.
    pub fn new(max_rows: usize) -> Self {
        const NAMES: [[&str; 6]; 3] = [
            ["1-1", "1-2", "1-3", "1-4", "1-5", "1-6"],
            ["2-1", "2-2", "2-3", "2-4", "2-5", "2-6"],
            ["3-1", "3-2", "3-3", "3-4", "3-5", "3-6"],
        ];
        let mut specs: Vec<ChannelSpec> = Vec::new();
        for g in NAMES {
            for (c, n) in g.iter().enumerate() {
                specs.push((n, 0.0, if c < 3 { "VEGETATION 5200" } else { "VEGETATION 1024" }, veg_width(c)));
            }
        }
        Vegetation {
            planes: Planes::new(&specs, max_rows),
            rows: [0; 18],
            used: 0,
        }
    }
}

impl PacketInstrument for Vegetation {
    fn push(&mut self, p: &PacketIn<'_>) {
        let Some(k) = VEG_APIDS.iter().flatten().position(|&a| a == p.apid) else {
            return;
        };
        let c = k % 6;
        let w = veg_width(c);
        if p.data.len() < 18 + w * 3 / 2 {
            return;
        }
        let row = self.rows[k];
        for x in 0..w {
            put(&mut self.planes, k, x, row, word12(p.data, 18, x), 16);
        }
        self.rows[k] += 1;
        self.used += 1;
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn used(&self) -> u64 {
        self.used
    }
    fn clear(&mut self) {
        self.planes.clear();
        self.rows = [0; 18];
    }
}

/// The VEGETATION test packets: one row of the six channels of VEG-1 in turn.
#[derive(Default)]
pub struct VegetationSource {
    row: usize,
}

impl TestSource for VegetationSource {
    fn next(&mut self, out: &mut Vec<TestPacket>) {
        for (c, &apid) in VEG_APIDS[0].iter().enumerate() {
            let w = veg_width(c);
            let mut d = vec![0u8; 18 + w * 3 / 2];
            for x in 0..w {
                put_bits(&mut d, 8 * 18 + 12 * x, 12, u32::from(test_pixel(c, x, self.row)));
            }
            out.push(TestPacket { apid, flags: 3, data: d });
        }
        self.row += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(i: &mut dyn PacketInstrument, s: &mut dyn TestSource, rounds: usize) {
        let mut pk = Vec::new();
        for _ in 0..rounds {
            s.next(&mut pk);
        }
        for p in &pk {
            i.push(&PacketIn {
                apid: p.apid,
                flags: p.flags,
                seq: 0,
                data: &p.data,
            });
        }
        i.finish();
    }

    #[test]
    fn amsr2_round_trips() {
        let mut d = Amsr2::new(100);
        feed(&mut d, &mut Amsr2Source::default(), 30);
        for c in [0, 7, 19] {
            for (x, y) in [(0, 0), (242, 2), (100, 1)] {
                assert_eq!(d.planes().planes[c].get(x, y), test_pixel(c, x, y) * 16);
            }
        }
    }

    #[test]
    fn metimage_round_trips() {
        let mut d = Metimage::new(100);
        feed(&mut d, &mut MetimageSource::default(), 58);
        for ch in [1, 8, 14] {
            let p = &d.planes().planes[ch - 1];
            assert_eq!(p.rows, 24);
            for (x, y) in [(0, 0), (3191, 23), (1000, 7)] {
                assert_eq!(p.get(x, y), metimage_test_value(ch, x, y), "channel {ch} at {x}, {y}");
            }
        }
    }

    #[test]
    fn tirs_and_vegetation_round_trip() {
        let mut t = Tirs::new(10);
        feed(&mut t, &mut TirsSource::default(), 3);
        assert_eq!(t.planes().planes[0].get(1279, 2), test_pixel(0, 1279, 2) * 16);
        let mut v = Vegetation::new(10);
        feed(&mut v, &mut VegetationSource::default(), 3);
        assert_eq!(v.planes().planes[0].get(5199, 2), test_pixel(0, 5199, 2) * 16);
        assert_eq!(v.planes().planes[5].get(1023, 1), test_pixel(5, 1023, 1) * 16);
    }
}
