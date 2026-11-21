//! MODIS of the EOS direct broadcast (APID 64): day packets with all 38 channels (2 at 250 m, 5 at 500 m, 31 at 1 km) and
//! night packets with the 17 thermal 1-km channels, 12-bit words with a checksum, ten 1-km rows a scan. No public document
//! confirms the packet layout used here.

use super::direct::{PacketIn, PacketInstrument, TestPacket, TestSource};
use super::planes::{bits_at, put_bits, test_pixel, ChannelSpec, Planes};

/// The APID of the instrument packets.
pub const APID: u16 = 64;
/// Frame positions of a scan.
pub const POSITIONS: usize = 1354;
/// Words of a day packet.
pub const DAY_WORDS: usize = 416;
/// Words of a night packet.
pub const NIGHT_WORDS: usize = 172;
/// The first data byte of the words.
pub const WORDS_AT: usize = 12;

/// The names of the 31 1-km channels in packet order.
pub const KM_NAMES: [&str; 31] = [
    "8", "9", "10", "11", "12", "13L", "13H", "14L", "14H", "15", "16", "17", "18", "19", "20", "21",
    "22", "23", "24", "25", "26", "27", "28", "29", "30", "31", "32", "33", "34", "35", "36",
];
const KM_UM: [f64; 31] = [
    0.412, 0.443, 0.488, 0.531, 0.551, 0.667, 0.667, 0.678, 0.678, 0.748, 0.869, 0.905, 0.936, 0.94,
    3.75, 3.959, 3.959, 4.05, 4.465, 4.515, 1.375, 6.715, 7.325, 8.55, 9.73, 11.03, 12.02, 13.335,
    13.635, 13.935, 14.235,
];
/// The plane of the first 1-km channel (planes 0 and 1 are the 250 m bands, 2 to 6 the 500 m bands).
pub const KM_FIRST: usize = 7;
/// The 1-km index of channel 20, the first channel of a night packet.
pub const NIGHT_FIRST: usize = 14;

/// The channel descriptions: bands 1 and 2 (5416 columns), 3 to 7 (2708), then the 31 1-km channels (1354).
pub fn specs() -> Vec<ChannelSpec> {
    let mut v: Vec<ChannelSpec> = vec![
        ("1", 0.645, "MODIS 250 m", 4 * POSITIONS),
        ("2", 0.858, "MODIS 250 m", 4 * POSITIONS),
    ];
    for (n, um) in [("3", 0.469), ("4", 0.555), ("5", 1.24), ("6", 1.64), ("7", 2.13)] {
        v.push((n, um, "MODIS 500 m", 2 * POSITIONS));
    }
    for (n, um) in KM_NAMES.iter().zip(KM_UM) {
        v.push((n, um, "MODIS 1 km", POSITIONS));
    }
    v
}

/// The plane of a channel name.
pub fn plane(name: &str) -> Option<usize> {
    specs().iter().position(|s| s.0 == name)
}

fn word(d: &[u8], k: usize) -> u16 {
    bits_at(d, 8 * WORDS_AT + 12 * k, 12) as u16
}

/// The checksum rule: the sum of the words before the last, modulo 65536, shifted right by 4, equals the last word.
pub fn checksum_ok(d: &[u8], words: usize) -> bool {
    let sum = (0..words - 1).fold(0u32, |s, k| (s + u32::from(word(d, k))) & 0xFFFF);
    (sum >> 4) as u16 == word(d, words - 1)
}

/// The MODIS decoder. See the [module documentation](self).
pub struct Modis {
    planes: Planes,
    base: Option<usize>,
    last_scan: Option<u8>,
    used: u64,
    /// Packets whose checksum failed.
    pub bad_checksums: u64,
}

impl Modis {
    /// A decoder whose 1-km pictures keep at most `max_rows` rows (the 250 m pictures 4 times as many).
    pub fn new(max_rows: usize) -> Self {
        Modis {
            planes: Planes::new(&specs(), 4 * max_rows),
            base: None,
            last_scan: None,
            used: 0,
            bad_checksums: 0,
        }
    }

    fn put(&mut self, plane: usize, col: usize, row: usize, v: u16) {
        self.planes.set(plane, col, row, v.saturating_mul(16));
    }
}

impl PacketInstrument for Modis {
    fn push(&mut self, p: &PacketIn<'_>) {
        if p.apid != APID || p.data.len() < 12 {
            return;
        }
        let d = p.data;
        let night = (d[8] >> 4) & 7 == 1;
        let scan = (d[8] >> 1) & 7;
        if d[9] & 0x80 != 0 {
            return;
        }
        let position = usize::from(d[9] & 0x7F) * 16 + usize::from(d[10] >> 4);
        if !(1..=POSITIONS).contains(&position) {
            return;
        }
        let (words, need) = if night { (NIGHT_WORDS, 270) } else { (DAY_WORDS, 636) };
        if d.len() < need {
            return;
        }
        if !checksum_ok(d, words) {
            self.bad_checksums += 1;
            return;
        }
        let starts = position == 1 && (night || p.flags == 1);
        if starts && self.last_scan != Some(scan) {
            self.base = Some(self.base.map_or(0, |b| b + 10));
            self.last_scan = Some(scan);
        }
        let Some(base) = self.base else { return };
        let pos = position - 1;
        self.used += 1;
        if night {
            for f in 0..10 {
                for i in 0..17 {
                    let v = word(d, (9 - f) * 17 + i);
                    self.put(KM_FIRST + NIGHT_FIRST + i, pos, base + f, v);
                }
            }
            return;
        }
        let first_row = if p.flags == 1 { 5 } else { 0 };
        for f in 0..5 {
            let o = (4 - f) * 83;
            let l = base + first_row + f;
            for band in 0..2 {
                for i in 0..4 {
                    for y in 0..4 {
                        let v = word(d, o + 16 * band + i * 4 + y);
                        self.put(band, 4 * pos + i, 4 * l + (3 - y), v);
                    }
                }
            }
            for c in 0..5 {
                for i in 0..2 {
                    for y in 0..2 {
                        let v = word(d, o + 32 + c * 4 + i * 2 + y);
                        self.put(2 + c, 2 * pos + i, 2 * l + (1 - y), v);
                    }
                }
            }
            for k in 0..31 {
                self.put(KM_FIRST + k, pos, l, word(d, o + 52 + k));
            }
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
}

/// The 12-bit test value of plane `plane` at (`x`, `y`) of that plane.
pub fn test_value(plane: usize, x: usize, y: usize) -> u16 {
    test_pixel(plane, x, y)
}

/// A day packet of the test picture: scan `scan`, frame position `position` (1 to 1354), the rows 5 to 9 (`upper`) or 0 to 4.
pub fn day_packet(scan: usize, position: usize, upper: bool) -> Vec<u8> {
    let mut words = vec![0u16; DAY_WORDS];
    let pos = position - 1;
    let first_row = if upper { 5 } else { 0 };
    for f in 0..5 {
        let o = (4 - f) * 83;
        let l = scan * 10 + first_row + f;
        for band in 0..2 {
            for i in 0..4 {
                for y in 0..4 {
                    words[o + 16 * band + i * 4 + y] = test_value(band, 4 * pos + i, 4 * l + (3 - y));
                }
            }
        }
        for c in 0..5 {
            for i in 0..2 {
                for y in 0..2 {
                    words[o + 32 + c * 4 + i * 2 + y] = test_value(2 + c, 2 * pos + i, 2 * l + (1 - y));
                }
            }
        }
        for k in 0..31 {
            words[o + 52 + k] = test_value(KM_FIRST + k, pos, l);
        }
    }
    let sum = words[..DAY_WORDS - 1].iter().fold(0u32, |s, &w| (s + u32::from(w)) & 0xFFFF);
    words[DAY_WORDS - 1] = (sum >> 4) as u16;
    let mut d = vec![0u8; WORDS_AT + DAY_WORDS * 3 / 2];
    d[8] = ((scan % 8) as u8) << 1;
    d[9] = (position / 16) as u8;
    d[10] = ((position % 16) as u8) << 4;
    for (k, &w) in words.iter().enumerate() {
        put_bits(&mut d, 8 * WORDS_AT + 12 * k, 12, u32::from(w));
    }
    d
}

/// The test packets of MODIS: day packets, scan after scan, two packets for each frame position.
#[derive(Default)]
pub struct ModisSource {
    scan: usize,
    position: usize,
}

impl TestSource for ModisSource {
    fn next(&mut self, out: &mut Vec<TestPacket>) {
        self.position += 1;
        if self.position > POSITIONS {
            self.position = 1;
            self.scan += 1;
        }
        for upper in [true, false] {
            out.push(TestPacket {
                apid: APID,
                flags: if upper { 1 } else { 2 },
                data: day_packet(self.scan, self.position, upper),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_scan_round_trips() {
        let mut s = ModisSource::default();
        let mut m = Modis::new(100);
        let mut pk = Vec::new();
        while pk.len() < 2 * POSITIONS {
            s.next(&mut pk);
        }
        for p in &pk {
            m.push(&PacketIn {
                apid: p.apid,
                flags: p.flags,
                seq: 0,
                data: &p.data,
            });
        }
        assert_eq!(m.bad_checksums, 0);
        for name in ["1", "2", "3", "7", "8", "29", "36"] {
            let pl = plane(name).unwrap();
            let p = &m.planes().planes[pl];
            for (x, y) in [(0, 0), (37, 3), (p.width - 1, p.rows - 1)] {
                assert_eq!(p.get(x, y), test_value(pl, x, y) * 16, "{name} at {x}, {y}");
            }
        }
    }
}
