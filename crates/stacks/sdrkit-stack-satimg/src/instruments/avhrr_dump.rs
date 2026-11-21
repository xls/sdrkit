//! AVHRR of the MetOp X-band dump: the packets of APIDs 103 (channel 3A) and 104 (3B) through the AVHRR packet reader, lines
//! placed by the on-board time, and the on-board calibration views of each line (PRT counts, black body and space views)
//! with the brightness temperature of the method of the NOAA KLM User's Guide section 7 (not checked against the text). The
//! coefficients of each satellite are data, read from a file.

use super::avhrr::{decode_packet, pack10, word10, PACKET_LEN, WIDTH};
use super::direct::{PacketIn, PacketInstrument, TestPacket, TestSource};
use super::planes::{test_pixel, ChannelSpec, Planes};
use std::collections::BTreeMap;

/// The APID of the lines with channel 3A.
pub const APID_3A: u16 = 103;
/// The APID of the lines with channel 3B.
pub const APID_3B: u16 = 104;
/// The first constant of the Planck function, mW / (m² sr cm⁻⁴).
pub const C1: f64 = 1.191_042_7e-5;
/// The second constant of the Planck function, K cm.
pub const C2: f64 = 1.438_775_2;
/// The words of the three PRT counts.
pub const PRT_WORD: usize = 10_297;
/// The first word of the black body views.
pub const BB_WORD: usize = 10_305;
/// The output scale of the 10-bit counts.
pub const SCALE: u16 = 64;

/// The channel descriptions.
pub fn specs() -> Vec<ChannelSpec> {
    [("1", 0.63), ("2", 0.865), ("3", 1.6), ("4", 10.8), ("5", 12.0)]
        .iter()
        .map(|&(n, um)| (n, um, "AVHRR", WIDTH))
        .collect()
}

/// The coefficients of one thermal channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChannelCal {
    /// Central wave number, cm⁻¹.
    pub nu: f64,
    /// Band correction A.
    pub a: f64,
    /// Band correction B.
    pub b: f64,
    /// Radiance of space.
    pub ns: f64,
    /// Non-linearity b0.
    pub b0: f64,
    /// Non-linearity b1.
    pub b1: f64,
    /// Non-linearity b2.
    pub b2: f64,
}

/// The coefficients of one satellite: channels 3B, 4 and 5, and the PRT polynomials.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SatCal {
    /// Channels 3B, 4, 5.
    pub channels: [Option<ChannelCal>; 3],
    /// a0, a1, a2 of PRT 1 to 4.
    pub prt: Vec<[f64; 3]>,
}

/// The coefficients of every satellite of a calibration file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CalTable {
    /// By satellite name.
    pub sats: BTreeMap<String, SatCal>,
}

impl CalTable {
    /// Reads a calibration file: `#` lines, a header line, then rows `channel,<satellite>,<3b|4|5>,nu,a,b,ns,b0,b1,b2` and
    /// `prt,<satellite>,<1 to 4>,a0,a1,a2`.
    ///
    /// # Errors
    ///
    /// The line that does not read.
    pub fn parse(text: &str) -> Result<CalTable, String> {
        let mut t = CalTable::default();
        for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).skip(1) {
            let f: Vec<&str> = l.split(',').map(str::trim).collect();
            let num = |k: usize| f.get(k).and_then(|v| v.parse::<f64>().ok()).ok_or_else(|| l.to_string());
            let s = t.sats.entry(f.get(1).copied().unwrap_or("").to_string()).or_default();
            match f.first().copied() {
                Some("channel") => {
                    let k = match f.get(2).copied() {
                        Some("3b") => 0,
                        Some("4") => 1,
                        Some("5") => 2,
                        _ => return Err(l.to_string()),
                    };
                    s.channels[k] = Some(ChannelCal {
                        nu: num(3)?,
                        a: num(4)?,
                        b: num(5)?,
                        ns: num(6)?,
                        b0: num(7)?,
                        b1: num(8)?,
                        b2: num(9)?,
                    });
                }
                Some("prt") => {
                    let k = num(2)? as usize;
                    if !(1..=4).contains(&k) {
                        return Err(l.to_string());
                    }
                    if s.prt.len() < k {
                        s.prt.resize(k, [0.0; 3]);
                    }
                    s.prt[k - 1] = [num(3)?, num(4)?, num(5)?];
                }
                _ => return Err(l.to_string()),
            }
        }
        Ok(t)
    }
}

/// The calibration views of one line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineCal {
    /// The PRT counts taken (100 < count < 500), with their index 0 to 2.
    pub prt: Vec<(usize, f64)>,
    /// The mean black body count of channels 3B, 4, 5 (counts above 100).
    pub bb: [Option<f64>; 3],
    /// The mean space count of channels 3B, 4, 5 (counts above 880).
    pub space: [Option<f64>; 3],
    /// The line came on the 3B APID.
    pub is_3b: bool,
}

/// Reads the calibration views of the data field `d` of an AVHRR packet.
pub fn line_cal(d: &[u8], is_3b: bool) -> LineCal {
    let w = |k: usize| f64::from(word10(d, 14, k));
    let prt = (0..3)
        .map(|i| (i, w(PRT_WORD + i)))
        .filter(|&(_, c)| c > 100.0 && c < 500.0)
        .collect();
    let mean = |words: &mut dyn Iterator<Item = f64>, ok: &dyn Fn(f64) -> bool| {
        let v: Vec<f64> = words.filter(|&c| ok(c)).collect();
        (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
    };
    let mut bb = [None; 3];
    let mut space = [None; 3];
    for j in 0..3 {
        bb[j] = mean(&mut (0..10).map(|i| w(BB_WORD + 5 * i + j + 2)), &|c| c > 100.0);
        space[j] = mean(&mut (0..10).map(|i| w(5 * i + j + 2)), &|c| c > 880.0);
    }
    LineCal { prt, bb, space, is_3b }
}

/// The black body temperature of a line: the mean over its PRTs of the polynomial of the PRT index modulo 4.
pub fn black_body(cal: &SatCal, l: &LineCal) -> Option<f64> {
    if l.prt.is_empty() || cal.prt.is_empty() {
        return None;
    }
    let s: f64 = l
        .prt
        .iter()
        .map(|&(i, c)| {
            let p = cal.prt[i % 4 % cal.prt.len()];
            p[0] + p[1] * c + p[2] * c * c
        })
        .sum();
    Some(s / l.prt.len() as f64)
}

/// The brightness temperature, K, of `count` of thermal channel `k` (0: 3B, 1: 4, 2: 5) on line `l`. `None` without PRTs,
/// with a space count below 800, a black body count of 0 or less, or a space count not above the black body count.
pub fn temperature(cal: &SatCal, l: &LineCal, k: usize, count: f64) -> Option<f64> {
    let c = cal.channels.get(k).copied().flatten()?;
    let t_bb = black_body(cal, l)?;
    let (cb, cs) = (l.bb[k]?, l.space[k]?);
    if cs < 800.0 || cb <= 0.0 || cs <= cb {
        return None;
    }
    let t_star = c.a + c.b * t_bb;
    let n_bb = C1 * c.nu.powi(3) / ((C2 * c.nu / t_star).exp() - 1.0);
    let n_lin = c.ns + (n_bb - c.ns) * (cs - count) / (cs - cb);
    let n_e = n_lin + c.b0 + c.b1 * n_lin + c.b2 * n_lin * n_lin;
    if n_e <= 0.0 {
        return None;
    }
    Some((C2 * c.nu / (1.0 + C1 * c.nu.powi(3) / n_e).ln() - c.a) / c.b)
}

/// The AVHRR decoder of the dump. See the [module documentation](self).
pub struct AvhrrDump {
    planes: Planes,
    last: Option<(u32, f64)>,
    /// The calibration views of each line.
    pub lines: Vec<Option<LineCal>>,
    used: u64,
}

impl AvhrrDump {
    /// A decoder of at most `max_rows` lines.
    pub fn new(max_rows: usize) -> Self {
        AvhrrDump {
            planes: Planes::new(&specs(), max_rows),
            last: None,
            lines: Vec::new(),
            used: 0,
        }
    }

    /// The line of the on-board time `ms`: a step of t ms adds max(1, (t + 83) / 167) lines when 0 <= t < 3 600 000, else 1.
    fn place(&mut self, ms: f64) -> u32 {
        let line = match self.last {
            None => 0,
            Some((l, t0)) => {
                let t = ms - t0;
                let step = if (0.0..3_600_000.0).contains(&t) {
                    (((t + 83.0) / 167.0).floor() as u32).max(1)
                } else {
                    1
                };
                l + step
            }
        };
        self.last = Some((line, ms));
        line
    }
}

impl PacketInstrument for AvhrrDump {
    fn push(&mut self, p: &PacketIn<'_>) {
        if p.apid != APID_3A && p.apid != APID_3B {
            return;
        }
        let Some(l) = decode_packet(p.data) else { return };
        let row = self.place(f64::from(l.ms)) as usize;
        if row >= self.planes.max_rows {
            return;
        }
        for c in 0..5 {
            if let Some(r) = self.planes.pixels_mut(c, row) {
                for (o, &v) in r.iter_mut().zip(&l.pixels[c * WIDTH..(c + 1) * WIDTH]) {
                    *o = v * SCALE;
                }
            }
        }
        if self.lines.len() <= row {
            self.lines.resize(row + 1, None);
        }
        self.lines[row] = Some(line_cal(p.data, p.apid == APID_3B));
        self.used += 1;
    }
    fn planes(&self) -> &Planes {
        &self.planes
    }
    fn planes_mut(&mut self) -> &mut Planes {
        &mut self.planes
    }
    fn clear(&mut self) {
        self.planes.clear();
        self.lines.clear();
        self.last = None;
    }
    fn used(&self) -> u64 {
        self.used
    }
}

/// The data field of a test packet: line `y` of the test picture (each value / 4), the on-board time, the PRT counts 300,
/// 301, 302, black body views of 400 and space views of 985.
pub fn test_packet(y: u32) -> Vec<u8> {
    let mut words = vec![0u16; BB_WORD + 5 * 10 + 8];
    for x in 0..WIDTH {
        for c in 0..5 {
            words[55 + 5 * x + c] = test_pixel(c, x, y as usize) / 4;
        }
    }
    for i in 0..3 {
        words[PRT_WORD + i] = 300 + i as u16;
    }
    for i in 0..10 {
        for j in 0..3 {
            words[BB_WORD + 5 * i + j + 2] = 400;
            words[5 * i + j + 2] = 985;
        }
    }
    let mut d = vec![0u8; PACKET_LEN];
    d[0..2].copy_from_slice(&21_000u16.to_be_bytes());
    d[2..6].copy_from_slice(&(36_000_000 + 167 * y).to_be_bytes());
    let packed = pack10(&words);
    let n = packed.len().min(PACKET_LEN - 14);
    d[14..14 + n].copy_from_slice(&packed[..n]);
    d
}

/// The test packets: one line after the other, the odd lines on the 3B APID.
#[derive(Default)]
pub struct AvhrrSource {
    y: u32,
}

impl TestSource for AvhrrSource {
    fn next(&mut self, out: &mut Vec<TestPacket>) {
        out.push(TestPacket {
            apid: if self.y % 2 == 1 { APID_3B } else { APID_3A },
            flags: 3,
            data: test_packet(self.y),
        });
        self.y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "kind,satellite,key,v1,v2,v3,v4,v5,v6,v7\n\
        channel,MetOp-B,3b,2664.32,1.763611,0.997018,0,0,0,0\n\
        channel,MetOp-B,4,933.63,0.504183,0.998638,-4.75,4.85,-0.096771,0.00048091\n\
        channel,MetOp-B,5,839.62,0.381279,0.998861,-4.39,4.36,-0.076635,0.00033524\n\
        prt,MetOp-B,1,276.5853,0.050933,1.54333e-06\n\
        prt,MetOp-B,2,276.5335,0.051033,1.49751e-06\n\
        prt,MetOp-B,3,276.5721,0.051097,1.42928e-06\n\
        prt,MetOp-B,4,276.575,0.05102,1.50841e-06\n";

    #[test]
    fn the_worked_example_of_the_calibration() {
        let t = CalTable::parse(FILE).unwrap();
        let cal = &t.sats["MetOp-B"];
        let l = line_cal(&test_packet(0), false);
        assert_eq!(l.prt.len(), 3);
        let tbb = black_body(cal, &l).unwrap();
        assert!((tbb - 292.056).abs() < 0.001, "{tbb}");
        for (k, count, want) in [(1, 400.0, 292.046), (1, 700.0, 251.739), (2, 400.0, 292.044), (2, 700.0, 248.209)] {
            let got = temperature(cal, &l, k, count).unwrap();
            assert!((got - want).abs() < 0.01, "channel {k} count {count}: {got}");
        }
    }
}
