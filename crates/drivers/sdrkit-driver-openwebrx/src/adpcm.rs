//! IMA ADPCM as OpenWebRX servers send it: 4 bits per sample, the low nibble first. Audio comes with or without `SYNC`
//! headers. A waterfall line starts from a fresh state and has 10 padding samples in front.
//!
//! Standard: IMA Digital Audio Focus and Technical Working Groups, "Recommended Practices for Enhancing Digital Audio
//! Compatibility in Multimedia Systems" (1992), ADPCM.

/// The IMA step table.
pub const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

/// The IMA index table, for `n & 7`.
pub const INDEX: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// The 4 bytes that start a sync header.
pub const SYNC: [u8; 4] = *b"SYNC";
/// Data bytes between two sync headers.
pub const SYNC_SPACING: usize = 1000;
/// Padding samples in front of a waterfall line.
pub const FFT_PAD: usize = 10;

/// The codec state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ima {
    /// Step index, 0 to 88.
    pub index: i32,
    /// The last output sample.
    pub pred: i32,
}

impl Ima {
    /// Decodes one nibble.
    #[inline]
    pub fn decode(&mut self, n: u8) -> i16 {
        let step = STEPS[self.index as usize];
        let mut diff = step >> 3;
        if n & 1 != 0 {
            diff += step >> 2;
        }
        if n & 2 != 0 {
            diff += step >> 1;
        }
        if n & 4 != 0 {
            diff += step;
        }
        if n & 8 != 0 {
            diff = -diff;
        }
        self.pred = (self.pred + diff).clamp(-32768, 32767);
        self.index = (self.index + INDEX[(n & 7) as usize]).clamp(0, 88);
        self.pred as i16
    }

    /// Encodes one sample with the matching IMA encoder.
    #[cfg(test)]
    pub fn encode(&mut self, sample: i16) -> u8 {
        let mut diff = sample as i32 - self.pred;
        let mut n = 0u8;
        if diff < 0 {
            n = 8;
            diff = -diff;
        }
        let mut step = STEPS[self.index as usize];
        let mut d = step >> 3;
        if diff >= step {
            n |= 4;
            diff -= step;
            d += step;
        }
        step >>= 1;
        if diff >= step {
            n |= 2;
            diff -= step;
            d += step;
        }
        step >>= 1;
        if diff >= step {
            n |= 1;
            d += step;
        }
        self.pred = (self.pred + if n & 8 != 0 { -d } else { d }).clamp(-32768, 32767);
        self.index = (self.index + INDEX[(n & 7) as usize]).clamp(0, 88);
        n
    }

    /// Encodes samples two per byte, the low nibble first. An odd last sample fills the low nibble only.
    #[cfg(test)]
    pub fn encode_all(&mut self, samples: &[i16]) -> Vec<u8> {
        samples
            .chunks(2)
            .map(|c| {
                let lo = self.encode(c[0]);
                let hi = c.get(1).map(|s| self.encode(*s)).unwrap_or(0);
                lo | hi << 4
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// The first bytes of a connection decide.
    Unknown,
    Sync,
    Plain,
}

/// The decoder of one audio stream. Narrow and HD audio each have one.
#[derive(Clone, Debug)]
pub struct AudioDecoder {
    mode: Mode,
    ima: Ima,
    head: [u8; 8],
    have: usize,
    in_header: bool,
    left: usize,
}

impl Default for AudioDecoder {
    fn default() -> Self {
        AudioDecoder {
            mode: Mode::Unknown,
            ima: Ima::default(),
            head: [0; 8],
            have: 0,
            in_header: false,
            left: 0,
        }
    }
}

impl AudioDecoder {
    /// Whether the stream was found to carry sync headers (`None`: not known yet).
    pub fn synced(&self) -> Option<bool> {
        match self.mode {
            Mode::Unknown => None,
            Mode::Sync => Some(true),
            Mode::Plain => Some(false),
        }
    }

    #[inline]
    fn byte(&mut self, b: u8, out: &mut Vec<i16>) {
        out.push(self.ima.decode(b & 0x0F));
        out.push(self.ima.decode(b >> 4));
    }

    /// Decodes ADPCM bytes and appends the samples to `out`. Returns the samples lost to a resync (two per dropped byte).
    pub fn decode_adpcm(&mut self, data: &[u8], out: &mut Vec<i16>) -> u64 {
        let mut lost = 0u64;
        for &b in data {
            match self.mode {
                Mode::Plain => self.byte(b, out),
                Mode::Unknown => {
                    self.head[self.have] = b;
                    self.have += 1;
                    if b != SYNC[self.have - 1] {
                        self.mode = Mode::Plain;
                        let (head, n) = (self.head, self.have);
                        self.have = 0;
                        for &h in &head[..n] {
                            self.byte(h, out);
                        }
                    } else if self.have == 4 {
                        self.mode = Mode::Sync;
                        self.in_header = true;
                    }
                }
                Mode::Sync if self.in_header => {
                    self.head[self.have] = b;
                    self.have += 1;
                    if self.have <= 4 {
                        // Slide until the bytes held are a start of `SYNC`.
                        while self.have > 0 && self.head[..self.have] != SYNC[..self.have] {
                            self.head.copy_within(1..self.have, 0);
                            self.have -= 1;
                            lost += 2;
                        }
                    } else if self.have == 8 {
                        let index = i16::from_le_bytes([self.head[4], self.head[5]]) as i32;
                        let pred = i16::from_le_bytes([self.head[6], self.head[7]]) as i32;
                        self.ima = Ima {
                            index: index.clamp(0, 88),
                            pred,
                        };
                        self.in_header = false;
                        self.have = 0;
                        self.left = SYNC_SPACING;
                    }
                }
                Mode::Sync => {
                    self.byte(b, out);
                    self.left -= 1;
                    if self.left == 0 {
                        self.in_header = true;
                    }
                }
            }
        }
        lost
    }
}

/// Uncompressed audio: 16-bit little-endian samples. A sample can be split between two messages.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pcm16 {
    odd: Option<u8>,
}

impl Pcm16 {
    /// Splits `data` into a sample joined with the odd byte of the last call, and whole samples. A last odd byte waits for
    /// the next call.
    pub fn split<'a>(&mut self, mut data: &'a [u8]) -> (Option<[u8; 2]>, &'a [u8]) {
        let mut joined = None;
        if let Some(lo) = self.odd.take() {
            match data.split_first() {
                Some((&hi, rest)) => {
                    joined = Some([lo, hi]);
                    data = rest;
                }
                None => {
                    self.odd = Some(lo);
                    return (None, &[]);
                }
            }
        }
        let whole = data.len() & !1;
        if whole < data.len() {
            self.odd = Some(data[whole]);
        }
        (joined, &data[..whole])
    }
}

/// Decodes an ADPCM waterfall line into dB values in place of what `out` holds.
pub fn decode_fft_adpcm(data: &[u8], out: &mut Vec<f32>) {
    out.clear();
    let mut ima = Ima::default();
    let mut k = 0usize;
    for &b in data {
        for n in [b & 0x0F, b >> 4] {
            let v = ima.decode(n);
            if k >= FFT_PAD {
                out.push(v as f32 / 100.0);
            }
            k += 1;
        }
    }
}

/// Decodes an uncompressed waterfall line (32-bit little-endian floats) in place of what `out` holds.
pub fn decode_fft_f32(data: &[u8], out: &mut Vec<f32>) {
    out.clear();
    out.extend(
        data.as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_plain(bytes: &[u8], from: Ima) -> Vec<i16> {
        let mut s = from;
        bytes
            .iter()
            .flat_map(|b| [b & 15, b >> 4])
            .map(|n| s.decode(n))
            .collect()
    }

    #[test]
    fn a_known_vector_round_trips() {
        let mut e = Ima::default();
        let bytes = e.encode_all(&[0, 1000, 2000, 1000, 0, -1000, -2000, -1000]);
        assert_eq!(bytes, [0x70, 0x77, 0xfd, 0xbf]);
        assert_eq!(
            e,
            Ima {
                index: 43,
                pred: -1056
            }
        );
        assert_eq!(
            decode_plain(&bytes, Ima::default()),
            [0, 11, 41, 104, 4, -195, -625, -1056]
        );
        let mut d = AudioDecoder::default();
        let mut out = Vec::new();
        assert_eq!(d.decode_adpcm(&bytes, &mut out), 0);
        assert_eq!(d.synced(), Some(false));
        assert_eq!(out, [0, 11, 41, 104, 4, -195, -625, -1056]);
    }

    #[test]
    fn a_sync_header_decodes_in_every_split() {
        let mut e = Ima {
            index: 20,
            pred: 1234,
        };
        assert_eq!(e.encode_all(&[1300, 1400, 1500, 1400]), [0x55, 0xb3]);
        assert_eq!(
            e,
            Ima {
                index: 26,
                pred: 1410
            }
        );
        let frame = [0x53, 0x59, 0x4e, 0x43, 0x14, 0x00, 0xd2, 0x04, 0x55, 0xb3];
        let want = [1302i16, 1402, 1494, 1410];
        let mut d = AudioDecoder::default();
        let mut out = Vec::new();
        d.decode_adpcm(&frame, &mut out);
        assert_eq!(out, want);
        assert_eq!(d.synced(), Some(true));
        assert_eq!(
            d.ima,
            Ima {
                index: 26,
                pred: 1410
            }
        );
        // Every split of the header produces the same.
        for cut in 1..frame.len() {
            let mut d = AudioDecoder::default();
            let mut out = Vec::new();
            d.decode_adpcm(&frame[..cut], &mut out);
            d.decode_adpcm(&frame[cut..], &mut out);
            assert_eq!(out, want, "cut {cut}");
        }
    }

    #[test]
    fn resync_after_1000_bytes_drops_and_counts() {
        let mut d = AudioDecoder::default();
        let mut out = Vec::new();
        let mut first = vec![0x53, 0x59, 0x4e, 0x43, 0, 0, 0, 0];
        first.extend(vec![0x11u8; 1000]);
        assert_eq!(d.decode_adpcm(&first, &mut out), 0);
        assert_eq!(out.len(), 2000);
        out.clear();
        let next = [
            0x00, 0x00, 0x53, 0x59, 0x4e, 0x43, 0x14, 0x00, 0xd2, 0x04, 0x55, 0xb3,
        ];
        assert_eq!(d.decode_adpcm(&next, &mut out), 4);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], 1302);
        // A wrong byte inside a header start is slid past as well.
        let mut d2 = AudioDecoder::default();
        let mut o = Vec::new();
        d2.decode_adpcm(&first, &mut o);
        assert_eq!(
            d2.decode_adpcm(b"SYSYNC\x14\x00\xd2\x04\x55\xb3", &mut o),
            4
        );
    }

    #[test]
    fn pcm16_keeps_an_odd_byte() {
        let mut d = Pcm16::default();
        assert_eq!(
            d.split(&[0x00, 0x80, 0xff, 0x7f]),
            (None, &[0x00, 0x80, 0xff, 0x7f][..])
        );
        assert_eq!(d.split(&[0x01, 0x00, 0x02]), (None, &[0x01, 0x00][..]));
        assert_eq!(d.split(&[]), (None, &[][..]));
        assert_eq!(d.split(&[0x00]), (Some([0x02, 0x00]), &[][..]));
    }

    #[test]
    fn waterfall_lines_in_both_codings() {
        let values = [-100.0f64, -95.5, -60.25, -60.25, -98.7, -101.0];
        let mut samples = vec![-10000i16; FFT_PAD];
        samples.extend(values.iter().map(|v| (v * 100.0).trunc() as i16));
        let bytes = Ima::default().encode_all(&samples);
        assert_eq!(bytes, [0xff, 0xff, 0xff, 0xff, 0x0d, 0x08, 0x03, 0x8d]);
        let mut out = Vec::new();
        decode_fft_adpcm(&bytes, &mut out);
        let want = [-99.49f32, -94.46, -62.44, -58.29, -99.87, -105.4];
        assert_eq!(out.len(), 6);
        for (a, b) in out.iter().zip(want) {
            assert!((a - b).abs() < 1e-4, "{out:?}");
        }
        let mut again = Vec::new();
        decode_fft_adpcm(&bytes, &mut again);
        assert_eq!(out, again);
        let raw = [
            0x00, 0x00, 0xc8, 0xc2, 0x00, 0x00, 0xbf, 0xc2, 0x00, 0x00, 0x71, 0xc2, 0x00, 0x00,
            0x71, 0xc2, 0x66, 0x66, 0xc5, 0xc2, 0x00, 0x00, 0xca, 0xc2,
        ];
        decode_fft_f32(&raw, &mut out);
        assert_eq!(out, [-100.0, -95.5, -60.25, -60.25, -98.7, -101.0]);
    }
}
