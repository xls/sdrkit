//! VIIRS of the JPSS direct broadcast (APIDs 800 to 821): 16 M bands of 3200 columns, 5 I bands of 6400 and the day-night
//! band of 4064. A scan group starts with a first packet whose time keys the group; each body packet holds one detector line
//! of one band in six zones of Rice-coded samples (CCSDS 121.0, 15 bits). Some bands are sent as a difference from a base
//! band. No public document confirms the packet layout used here.

use super::direct::{PacketIn, PacketInstrument, TestPacket, TestSource};
use super::planes::{test_pixel, ChannelSpec, Planes};
use sdrkit_proto_ccsds::rice::{self, Params};
use std::collections::BTreeMap;

/// The kind of a band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A moderate-resolution band.
    M,
    /// An imagery band.
    I,
    /// The day-night band.
    Dnb,
}

impl Kind {
    /// The widths of the six zones of a scan.
    pub fn zones(self) -> [usize; 6] {
        match self {
            Kind::M => [640, 368, 592, 592, 368, 640],
            Kind::I => [1280, 736, 1184, 1184, 736, 1280],
            Kind::Dnb => [784, 488, 760, 760, 488, 784],
        }
    }
    /// Columns of a scan line.
    pub fn width(self) -> usize {
        self.zones().iter().sum()
    }
    /// Detector lines of a scan.
    pub fn lines(self) -> usize {
        match self {
            Kind::I => 32,
            _ => 16,
        }
    }
    fn group(self) -> &'static str {
        match self {
            Kind::M => "VIIRS M",
            Kind::I => "VIIRS I",
            Kind::Dnb => "VIIRS DNB",
        }
    }
}

/// One band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// Its name.
    pub name: &'static str,
    /// Its APID.
    pub apid: u16,
    /// Centre wavelength, µm.
    pub um: f64,
    /// Its kind.
    pub kind: Kind,
    /// The zones are oversampled 1, 2, 3, 3, 2, 1.
    pub oversampled: bool,
    /// The output scale.
    pub scale: u32,
    /// The band it is a difference from, if any.
    pub base: Option<&'static str>,
}

const fn band(
    name: &'static str,
    apid: u16,
    um: f64,
    kind: Kind,
    oversampled: bool,
    base: Option<&'static str>,
) -> Band {
    let scale = match kind {
        Kind::Dnb => 2,
        _ if oversampled => 8,
        _ => 16,
    };
    Band {
        name,
        apid,
        um,
        kind,
        oversampled,
        scale,
        base,
    }
}

/// The 22 bands in picture order.
pub const BANDS: [Band; 22] = [
    band("M1", 804, 0.412, Kind::M, true, Some("M2")),
    band("M2", 803, 0.445, Kind::M, true, Some("M3")),
    band("M3", 802, 0.488, Kind::M, true, Some("M4")),
    band("M4", 800, 0.555, Kind::M, true, None),
    band("M5", 801, 0.672, Kind::M, true, Some("M4")),
    band("M6", 805, 0.746, Kind::M, false, None),
    band("M7", 806, 0.865, Kind::M, true, None),
    band("M8", 809, 1.24, Kind::M, false, Some("M10")),
    band("M9", 807, 1.378, Kind::M, false, None),
    band("M10", 808, 1.61, Kind::M, false, None),
    band("M11", 810, 2.25, Kind::M, false, Some("M10")),
    band("M12", 812, 3.7, Kind::M, false, None),
    band("M13", 811, 4.05, Kind::M, true, None),
    band("M14", 816, 8.55, Kind::M, false, Some("M15")),
    band("M15", 815, 10.763, Kind::M, false, None),
    band("M16", 814, 12.013, Kind::M, false, None),
    band("I1", 818, 0.64, Kind::I, false, None),
    band("I2", 819, 0.865, Kind::I, false, Some("I1")),
    band("I3", 820, 1.61, Kind::I, false, Some("I2")),
    band("I4", 813, 3.74, Kind::I, false, Some("M12")),
    band("I5", 817, 11.45, Kind::I, false, Some("M15")),
    band("DNB", 821, 0.7, Kind::Dnb, false, None),
];

/// The order in which difference bands are restored.
const RESTORE: [&str; 11] = ["M5", "M3", "M2", "M1", "M8", "M11", "M14", "I2", "I3", "I4", "I5"];

/// The oversampling of zone `z` of `b`.
pub fn oversampling(b: &Band, z: usize) -> usize {
    if b.oversampled {
        [1, 2, 3, 3, 2, 1][z]
    } else {
        1
    }
}

/// The index of the band of `name`.
pub fn index(name: &str) -> Option<usize> {
    BANDS.iter().position(|b| b.name == name)
}

/// The channel descriptions of the pictures.
pub fn specs() -> Vec<ChannelSpec> {
    BANDS
        .iter()
        .map(|b| (b.name, b.um, b.kind.group(), b.kind.width()))
        .collect()
}

/// The sync pattern a zone may end with in place of the packet's own.
pub const ZONE_END: [u8; 4] = [0xC0, 0x00, 0xFF, 0xEE];
/// The first byte of the zones in a body packet.
pub const ZONES_AT: usize = 88;
/// Scan groups kept open at once.
pub const OPEN_GROUPS: usize = 4;

#[derive(Clone, Debug)]
struct Group {
    index: usize,
    lines: Vec<Vec<Option<Vec<u16>>>>,
}

/// The VIIRS decoder. See the [module documentation](self).
pub struct Viirs {
    planes: Planes,
    groups: BTreeMap<u64, Group>,
    next_index: usize,
    segments: [Option<(u64, u16, u16)>; 22],
    samples: Vec<u32>,
    used: u64,
    /// Zones whose end pattern or Rice data did not check.
    pub bad_zones: u64,
}

impl Viirs {
    /// A decoder whose pictures keep at most `max_rows` rows.
    pub fn new(max_rows: usize) -> Self {
        Viirs {
            planes: Planes::new(&specs(), max_rows),
            groups: BTreeMap::new(),
            next_index: 0,
            segments: [None; 22],
            samples: vec![0; 1280 * 3],
            used: 0,
            bad_zones: 0,
        }
    }

    fn open(&mut self, key: u64) {
        if self.groups.contains_key(&key) {
            return;
        }
        while self.groups.len() >= OPEN_GROUPS {
            let Some((&k, _)) = self.groups.iter().min_by_key(|(_, g)| g.index) else {
                break;
            };
            if let Some(g) = self.groups.remove(&k) {
                self.write(g);
            }
        }
        let lines = BANDS.iter().map(|b| vec![None; b.kind.lines()]).collect();
        self.groups.insert(
            key,
            Group {
                index: self.next_index,
                lines,
            },
        );
        self.next_index += 1;
    }

    /// Decodes the zones of a body packet of band `bi` into a line.
    fn body(&mut self, bi: usize, d: &[u8]) -> Option<Vec<u16>> {
        let b = &BANDS[bi];
        let sync = &d[20..24];
        let mut line = vec![0u16; b.kind.width()];
        let mut at = ZONES_AT;
        let mut col = 0;
        for (z, &w) in b.kind.zones().iter().enumerate() {
            let os = oversampling(b, z);
            if at + 4 > d.len() {
                self.bad_zones += 1;
                return None;
            }
            let fill = usize::from(d[at]);
            let c = usize::from(u16::from_be_bytes([d[at + 2], d[at + 3]]));
            let len = c.checked_sub(4 + fill / 8)?;
            let end = d.get(at + 4 + c..at + 8 + c);
            if at + 4 + len > d.len() || end.is_none_or(|e| e != sync && e != ZONE_END) {
                self.bad_zones += 1;
                return None;
            }
            let n = w * os;
            let out = &mut self.samples[..n];
            match rice::decode(&d[at + 4..at + 4 + len], &Params::VIIRS, out) {
                Ok(r) if r.samples == n => {}
                _ => {
                    self.bad_zones += 1;
                    return None;
                }
            }
            for (x, chunk) in out.chunks(os).enumerate() {
                let s: u32 = chunk.iter().sum();
                line[col + x] = (s / os as u32) as u16;
            }
            col += w;
            at += c + 8;
        }
        Some(line)
    }

    /// Restores the difference bands of a group and writes its rows.
    fn write(&mut self, mut g: Group) {
        for name in RESTORE {
            let Some(bi) = index(name) else { continue };
            let Some(base) = BANDS[bi].base.and_then(index) else {
                continue;
            };
            let half = BANDS[bi].kind == Kind::I && BANDS[base].kind == Kind::M;
            for det in 0..g.lines[bi].len() {
                let bdet = if half { det / 2 } else { det };
                let Some(bl) = g.lines[base].get(bdet).cloned().flatten() else {
                    g.lines[bi][det] = None;
                    continue;
                };
                if let Some(l) = g.lines[bi][det].as_mut() {
                    for (x, v) in l.iter_mut().enumerate() {
                        let bx = if half { x / 2 } else { x };
                        let bv = i32::from(bl.get(bx).copied().unwrap_or(0));
                        *v = (i32::from(*v) + bv - 16_383).clamp(0, 65_535) as u16;
                    }
                }
            }
        }
        for (bi, lines) in g.lines.into_iter().enumerate() {
            let b = &BANDS[bi];
            let n = b.kind.lines();
            for (det, l) in lines.into_iter().enumerate() {
                let Some(l) = l else { continue };
                let row = g.index * n + (n - 1 - det);
                if let Some(r) = self.planes.pixels_mut(bi, row) {
                    for (o, v) in r.iter_mut().zip(&l) {
                        *o = (u32::from(*v) * b.scale).min(65_535) as u16;
                    }
                }
            }
        }
    }
}

impl PacketInstrument for Viirs {
    fn push(&mut self, p: &PacketIn<'_>) {
        let Some(bi) = BANDS.iter().position(|b| b.apid == p.apid) else {
            return;
        };
        let d = p.data;
        if p.flags == 1 {
            if d.len() >= 12 {
                let key = u64::from_be_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]);
                self.open(key);
                self.segments[bi] = Some((key, p.seq, u16::from(d[8])));
            }
            return;
        }
        let Some((key, first, n)) = self.segments[bi] else {
            return;
        };
        let after = p.seq.wrapping_sub(first) & 0x3FFF;
        if after == 0 || after > n + 2 || d.len() < 96 {
            return;
        }
        let det = usize::from(d[19]);
        if det >= BANDS[bi].kind.lines() || !self.groups.contains_key(&key) {
            return;
        }
        if let Some(line) = self.body(bi, d) {
            if let Some(g) = self.groups.get_mut(&key) {
                g.lines[bi][det] = Some(line);
                self.used += 1;
            }
        }
    }
    fn finish(&mut self) {
        let mut open: Vec<(u64, usize)> = self.groups.iter().map(|(k, g)| (*k, g.index)).collect();
        open.sort_by_key(|g| g.1);
        for (k, _) in open {
            if let Some(g) = self.groups.remove(&k) {
                self.write(g);
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

/// The restored 14-bit value of the test signal of band `bi` at column `x`, picture row `row`: twice the test picture,
/// cut to 14 bits.
pub fn test_value(bi: usize, x: usize, row: usize) -> u16 {
    (test_pixel(bi, x, row) * 2) & 0x3FFF
}

/// The test packets of VIIRS: one scan group after the other, each band's first packet and then one body packet for each
/// detector line, the difference bands sent as differences.
#[derive(Default)]
pub struct ViirsSource {
    group: u64,
    seq: [u16; 22],
}

/// The sync pattern of the test packets.
pub const TEST_SYNC: [u8; 4] = [0x51, 0x7A, 0x0C, 0x3E];

impl ViirsSource {
    fn value(bi: usize, det: usize, x: usize, g: usize) -> i32 {
        let n = BANDS[bi].kind.lines();
        let v = i32::from(test_value(bi, x, g * n + (n - 1 - det)));
        let Some(base) = BANDS[bi].base.and_then(index) else {
            return v;
        };
        let half = BANDS[bi].kind == Kind::I && BANDS[base].kind == Kind::M;
        let (bd, bx) = if half { (det / 2, x / 2) } else { (det, x) };
        let nb = BANDS[base].kind.lines();
        let bv = i32::from(test_value(base, bx, g * nb + (nb - 1 - bd)));
        v - bv + 16_383
    }

    fn body(bi: usize, det: usize, g: usize) -> Vec<u8> {
        let b = &BANDS[bi];
        let mut d = vec![0u8; ZONES_AT];
        d[19] = det as u8;
        d[20..24].copy_from_slice(&TEST_SYNC);
        let mut col = 0;
        let mut samples = Vec::new();
        let mut coded = Vec::new();
        for (z, &w) in b.kind.zones().iter().enumerate() {
            let os = oversampling(b, z);
            samples.clear();
            for x in col..col + w {
                let v = Self::value(bi, det, x, g).clamp(0, 32_767) as u32;
                samples.extend(std::iter::repeat_n(v, os));
            }
            coded.clear();
            if rice::encode(&samples, &Params::VIIRS, &mut coded).is_err() {
                coded.clear();
            }
            let c = 4 + coded.len();
            d.extend_from_slice(&[0, 0]);
            d.extend_from_slice(&(c as u16).to_be_bytes());
            d.extend_from_slice(&coded);
            d.extend_from_slice(&[0; 4]);
            d.extend_from_slice(&TEST_SYNC);
            col += w;
        }
        d
    }
}

impl TestSource for ViirsSource {
    fn next(&mut self, out: &mut Vec<TestPacket>) {
        let g = self.group as usize;
        for (bi, b) in BANDS.iter().enumerate() {
            let n = b.kind.lines();
            let mut first = vec![0u8; 12];
            first[..8].copy_from_slice(&(1000 + self.group).to_be_bytes());
            first[8] = n as u8;
            out.push(TestPacket {
                apid: b.apid,
                flags: 1,
                data: first,
            });
            for det in 0..n {
                out.push(TestPacket {
                    apid: b.apid,
                    flags: if det + 1 == n { 2 } else { 0 },
                    data: Self::body(bi, det, g),
                });
            }
            self.seq[bi] = self.seq[bi].wrapping_add(n as u16 + 1);
        }
        self.group += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_group_round_trips_with_differences() {
        let mut src = ViirsSource::default();
        let mut pk = Vec::new();
        src.next(&mut pk);
        let mut v = Viirs::new(100);
        let mut seq = [0u16; 22];
        for p in &pk {
            let bi = BANDS.iter().position(|b| b.apid == p.apid).unwrap();
            v.push(&PacketIn {
                apid: p.apid,
                flags: p.flags,
                seq: seq[bi],
                data: &p.data,
            });
            seq[bi] += 1;
        }
        v.finish();
        assert_eq!(v.bad_zones, 0);
        for name in ["M4", "M1", "M5", "M10", "M8", "M14", "I1", "I2", "I5", "DNB"] {
            let bi = index(name).unwrap();
            let b = &BANDS[bi];
            let p = &v.planes().planes[bi];
            assert_eq!(p.rows, b.kind.lines(), "{name}");
            for (x, row) in [(0, 0), (700, 3), (b.kind.width() - 1, b.kind.lines() - 1)] {
                let want = (u32::from(test_value(bi, x, row)) * b.scale).min(65_535) as u16;
                assert_eq!(p.get(x, row), want, "{name} at {x}, {row}");
            }
        }
    }
}
