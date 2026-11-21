//! The 16-bit channel pictures of the `eodb` instruments: one picture for each channel that grows with the pass, with the
//! name and centre wavelength of the channel. Rows that changed leave as 8-bit `picture.strip` items (the top 8 bits); the
//! 16-bit values stay here for the colour products and the checks.

use sdrkit_codec_image::strip::Strip;
use std::sync::Arc;

/// One channel of an instrument.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    /// The channel name ("M4", "1", "VIS" ...).
    pub name: &'static str,
    /// The centre wavelength, µm (0 when not known).
    pub um: f64,
    /// The picture of the strips: one name for each width.
    pub group: &'static str,
    /// The channel number inside its group, 1-based.
    pub number: u16,
    /// Columns.
    pub width: usize,
    /// Rows held.
    pub rows: usize,
    /// The values, row by row.
    pub data: Vec<u16>,
    dirty: Vec<u32>,
}

impl Plane {
    /// The value at (`col`, `row`), 0 outside the rows held.
    pub fn get(&self, col: usize, row: usize) -> u16 {
        if col < self.width && row < self.rows {
            self.data[row * self.width + col]
        } else {
            0
        }
    }
}

/// The description of a channel: name, centre wavelength (µm), strip picture name and width.
pub type ChannelSpec = (&'static str, f64, &'static str, usize);

/// The channel pictures of one instrument.
#[derive(Clone, Debug, PartialEq)]
pub struct Planes {
    /// The channels.
    pub planes: Vec<Plane>,
    /// The most rows a channel keeps.
    pub max_rows: usize,
    picture: u32,
}

impl Planes {
    /// Channels of `specs`, each at most `max_rows` rows.
    pub fn new(specs: &[ChannelSpec], max_rows: usize) -> Self {
        let mut planes = Vec::with_capacity(specs.len());
        for &(name, um, group, width) in specs {
            let number = planes.iter().filter(|p: &&Plane| p.group == group).count() as u16 + 1;
            planes.push(Plane {
                name,
                um,
                group,
                number,
                width,
                rows: 0,
                data: Vec::new(),
                dirty: Vec::new(),
            });
        }
        Planes {
            planes,
            max_rows,
            picture: 0,
        }
    }

    /// No channels.
    pub fn none() -> Self {
        Planes::new(&[], 0)
    }

    /// Starts new pictures.
    pub fn clear(&mut self) {
        for p in &mut self.planes {
            p.rows = 0;
            p.data.clear();
            p.dirty.clear();
        }
        self.picture += 1;
    }

    /// The index of the channel named `name`.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.planes.iter().position(|p| p.name == name)
    }

    /// The lines of the tallest channel.
    pub fn lines(&self) -> usize {
        self.planes.iter().map(|p| p.rows).max().unwrap_or(0)
    }

    /// Row `row` of channel `ch` to write, grown with zeros when needed and marked as changed. `None` beyond the row limit or
    /// for an unknown channel.
    pub fn pixels_mut(&mut self, ch: usize, row: usize) -> Option<&mut [u16]> {
        let max = self.max_rows;
        let p = self.planes.get_mut(ch)?;
        if row >= max {
            return None;
        }
        if row >= p.rows {
            p.rows = row + 1;
            p.data.resize(p.rows * p.width, 0);
        }
        if p.dirty.last() != Some(&(row as u32)) {
            p.dirty.push(row as u32);
        }
        let w = p.width;
        Some(&mut p.data[row * w..(row + 1) * w])
    }

    /// Sets one value; out of range is ignored.
    pub fn set(&mut self, ch: usize, col: usize, row: usize, v: u16) {
        if let Some(r) = self.pixels_mut(ch, row) {
            if let Some(x) = r.get_mut(col) {
                *x = v;
            }
        }
    }

    /// Appends a strip for each changed row of each channel (top 8 bits of each value) and forgets the changes.
    pub fn take_strips(&mut self, out: &mut Vec<Strip>) {
        let picture = self.picture;
        for p in &mut self.planes {
            p.dirty.sort_unstable();
            p.dirty.dedup();
            for &row in &p.dirty {
                let r = row as usize;
                if r >= p.rows {
                    continue;
                }
                let px: Vec<u8> = p.data[r * p.width..(r + 1) * p.width]
                    .iter()
                    .map(|&v| (v >> 8) as u8)
                    .collect();
                out.push(Strip {
                    picture,
                    instrument: p.group,
                    ..Strip::new(p.number, row, 0, 1, p.width as u32, Arc::from(px))
                });
            }
            p.dirty.clear();
        }
    }
}

/// Reads `n` bits (at most 32), most significant first, at bit `pos` of `data`; bits past the end read as 0.
#[inline]
pub fn bits_at(data: &[u8], pos: usize, n: u32) -> u32 {
    let mut v = 0u64;
    let first = pos / 8;
    let need = (pos % 8 + n as usize).div_ceil(8);
    for k in 0..need {
        v = (v << 8) | u64::from(data.get(first + k).copied().unwrap_or(0));
    }
    let total = 8 * need as u32;
    ((v >> (total - (pos % 8) as u32 - n)) & ((1u64 << n) - 1)) as u32
}

/// Writes the low `n` bits of `v`, most significant first, at bit `pos` of `data` (which must be long enough).
#[inline]
pub fn put_bits(data: &mut [u8], pos: usize, n: u32, v: u32) {
    for k in 0..n as usize {
        let b = (v >> (n as usize - 1 - k)) & 1;
        let p = pos + k;
        if b == 1 {
            data[p / 8] |= 0x80 >> (p % 8);
        } else {
            data[p / 8] &= !(0x80 >> (p % 8));
        }
    }
}

/// The 12-bit value of the test picture of the `eodb` instruments at channel `c`, column `x`, line `y`.
pub fn test_pixel(c: usize, x: usize, y: usize) -> u16 {
    let (c, x, y) = (c as u64, x as u64, y as u64);
    let check = if (x / 16 + y / 8) % 2 == 1 { 200 } else { 0 };
    let v = 3 * x + 5 * y * (c + 2) + (311 * c) % 997 + check + (((x * x * (c + 3) + 7 * y * y) / 16) & 63);
    (v & 4095) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_round_trip() {
        let mut d = vec![0u8; 16];
        put_bits(&mut d, 5, 17, 0x1_2345);
        put_bits(&mut d, 22, 12, 0xABC);
        assert_eq!(bits_at(&d, 5, 17), 0x1_2345);
        assert_eq!(bits_at(&d, 22, 12), 0xABC);
    }

    #[test]
    fn changed_rows_become_strips() {
        let mut p = Planes::new(&[("A", 1.0, "X", 4), ("B", 2.0, "X", 4)], 10);
        p.set(1, 2, 3, 0x1234);
        let mut s = Vec::new();
        p.take_strips(&mut s);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].channel, s[0].first_row, s[0].samples[2]), (2, 3, 0x12));
        p.take_strips(&mut s);
        assert_eq!(s.len(), 1);
    }
}
