//! The AAC-LC encoder of test signals: 960-sample frames, long windows only, sine window, one channel pair element with one
//! scale factor per channel, no TNS, prediction or M/S, Huffman codebook 11 with escapes, and a fill element that makes each
//! raw data block exactly its budget.
//!
//! Standard: ISO/IEC 14496-3 clauses 4.4.2 (syntax), 4.6.1 to 4.6.3 (quantization, scale factors, Huffman) and 4.6.11.

use crate::ics::Bands;
use crate::mdct::Mdct;
use crate::tables::{CB11_CODE, CB11_LEN, SCF_CODE, SCF_LEN};
use crate::{Error, Result};
use sdrkit_util_bits::BitWriter;

/// Samples per channel in one frame.
pub const FRAME: usize = 960;

/// Escape bits of a codebook 11 value (clause 4.6.3.3): 0 below 16, else the prefix and the word.
fn esc_bits(v: u32) -> usize {
    if v < 16 {
        0
    } else {
        let n = 31 - v.leading_zeros() as usize;
        (n - 4 + 1) + n
    }
}

/// Quantizes one spectral value: `sign(x) floor(|x / step|^(3/4) + 0.4054)` (clause 4.6.1.3).
fn quantize(x: f32, step: f32) -> i32 {
    let q = ((x.abs() / step).powf(0.75) + 0.4054).floor() as i32;
    if x < 0.0 {
        -q
    } else {
        q
    }
}

/// The AAC-LC encoder of a stereo channel pair. It allocates nothing after [`Encoder::new`] once the output has the capacity
/// of one block.
#[derive(Clone, Debug)]
pub struct Encoder {
    rate: u32,
    prev: [[f32; FRAME]; 2],
    window: Box<[f32; 2 * FRAME]>,
    mdct: Mdct,
    bands: Bands,
    n_bands: usize,
    z: Box<[f32; 2 * FRAME]>,
    spec: [[f32; FRAME]; 2],
    q: [[i32; FRAME]; 2],
}

impl Encoder {
    /// An encoder at 48 000 or 32 000 Hz.
    ///
    /// # Errors
    /// [`Error::SampleRate`] for another rate.
    pub fn new(rate: u32) -> Result<Self> {
        let bands = match rate {
            48_000 => Bands::new(3, FRAME)?,
            32_000 => Bands::new(5, FRAME)?,
            _ => return Err(Error::SampleRate),
        };
        let n = bands.offsets(false).len() - 1;
        let mut window = Box::new([0f32; 2 * FRAME]);
        for (i, w) in window.iter_mut().enumerate() {
            // The sine window of clause 4.6.11.3.2.
            *w = ((std::f64::consts::PI * (i as f64 + 0.5)) / (2 * FRAME) as f64).sin() as f32;
        }
        Ok(Encoder {
            rate,
            prev: [[0.0; FRAME]; 2],
            window,
            mdct: Mdct::new(FRAME),
            bands,
            n_bands: n,
            z: Box::new([0.0; 2 * FRAME]),
            spec: [[0.0; FRAME]; 2],
            q: [[0; FRAME]; 2],
        })
    }

    /// The sampling rate.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    fn band(&self, b: usize) -> std::ops::Range<usize> {
        let o = self.bands.offsets(false);
        usize::from(o[b])..usize::from(o[b + 1])
    }

    /// Bits for one channel at scale factor `sf`, with its quantized values written to `self.q[ch]`. Returns the bits and
    /// max_sfb.
    fn channel_bits(&mut self, ch: usize, sf: i32) -> (usize, usize) {
        let step = 2f32.powf((sf - 100) as f32 / 4.0);
        let mut max_sfb = 0;
        let mut bits = 0usize;
        for b in 0..self.n_bands {
            let r = self.band(b);
            let mut nz = false;
            for k in r.clone() {
                let v = quantize(self.spec[ch][k], step).clamp(-8191, 8191);
                self.q[ch][k] = v;
                nz |= v != 0;
            }
            if nz {
                max_sfb = b + 1;
                bits += 1; // scale factor difference 0
                for k in r.step_by(2) {
                    let (y, z) = (
                        self.q[ch][k].unsigned_abs(),
                        self.q[ch][k + 1].unsigned_abs(),
                    );
                    bits += CB11_LEN[(y.min(16) * 17 + z.min(16)) as usize] as usize;
                    bits += (y != 0) as usize + (z != 0) as usize + esc_bits(y) + esc_bits(z);
                }
            }
        }
        // Side information: global gain, ics_info, sections (at most one per band), pulse, TNS and gain control flags.
        let mut sections = 0;
        let mut last = None;
        for b in 0..max_sfb {
            let z = self.zero(ch, b);
            if Some(z) != last {
                sections += 1;
                last = Some(z);
            }
        }
        (bits + 8 + 11 + sections * 14 + 3, max_sfb)
    }

    fn zero(&self, ch: usize, b: usize) -> bool {
        self.q[ch][self.band(b)].iter().all(|&v| v == 0)
    }

    /// Writes one individual_channel_stream (clause 4.4.2.7).
    fn write_channel(&self, w: &mut BitWriter<'_>, ch: usize, sf: i32, max_sfb: usize) {
        let q = &self.q[ch];
        w.put(sf as u64, 8); // global_gain
                             // ics_info: reserved bit, ONLY_LONG_SEQUENCE, sine window, max_sfb, no prediction.
        w.put(0, 1);
        w.put(0, 2);
        w.put(0, 1);
        w.put(max_sfb as u64, 6);
        w.put(0, 1);
        // section_data: codebook 0 for zero bands, 11 for the others.
        let mut b = 0;
        while b < max_sfb {
            let z = self.zero(ch, b);
            let mut e = b + 1;
            while e < max_sfb && self.zero(ch, e) == z {
                e += 1;
            }
            w.put(if z { 0 } else { 11 }, 4);
            let mut len = e - b;
            while len >= 31 {
                w.put(31, 5);
                len -= 31;
            }
            w.put(len as u64, 5);
            b = e;
        }
        // scale_factor_data: the same scale factor for every band.
        for b in 0..max_sfb {
            if !self.zero(ch, b) {
                w.put(SCF_CODE[60] as u64, SCF_LEN[60] as u32);
            }
        }
        w.put(0, 1); // pulse_data_present
        w.put(0, 1); // tns_data_present
        w.put(0, 1); // gain_control_data_present
        for b in 0..max_sfb {
            if self.zero(ch, b) {
                continue;
            }
            for k in self.band(b).step_by(2) {
                let (y, z) = (q[k], q[k + 1]);
                let (ya, za) = (y.unsigned_abs(), z.unsigned_abs());
                let i = (ya.min(16) * 17 + za.min(16)) as usize;
                w.put(CB11_CODE[i] as u64, CB11_LEN[i] as u32);
                if ya != 0 {
                    w.put((y < 0) as u64, 1);
                }
                if za != 0 {
                    w.put((z < 0) as u64, 1);
                }
                for v in [ya, za] {
                    if v >= 16 {
                        // The escape sequence: n - 4 ones, a zero, then the n low bits.
                        let n = 31 - v.leading_zeros() as usize;
                        for _ in 0..n - 4 {
                            w.put(1, 1);
                        }
                        w.put(0, 1);
                        w.put((v - (1 << n)) as u64, n as u32);
                    }
                }
            }
        }
    }

    /// Encodes 960 samples per channel (-1 to 1) into a raw data block of exactly `budget` bytes in `out` (cleared first).
    /// `prefix` goes first as it is (for example a data stream element with PAD). A frame that does not fit is sent as silence.
    ///
    /// # Errors
    /// [`Error::InputLength`] when a channel has fewer than 960 samples, and [`Error::Budget`] when `prefix` and a block of
    /// silence do not fit `budget`.
    pub fn encode(
        &mut self,
        left: &[f32],
        right: &[f32],
        prefix: &[u8],
        budget: usize,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        if left.len() < FRAME || right.len() < FRAME {
            return Err(Error::InputLength);
        }
        for (ch, x) in [left, right].into_iter().enumerate() {
            self.z[..FRAME].copy_from_slice(&self.prev[ch]);
            for (z, &v) in self.z[FRAME..].iter_mut().zip(x) {
                // Twice full scale: the inverse transform of clause 4.6.11.3.2 (2 / window length) halves the result.
                *z = v * 65536.0;
            }
            self.prev[ch].copy_from_slice(&self.z[FRAME..]);
            for (z, w) in self.z.iter_mut().zip(self.window.iter()) {
                *z *= w;
            }
            self.mdct.forward(&self.z[..], &mut self.spec[ch])?;
        }
        let avail = budget.checked_sub(prefix.len()).ok_or(Error::Budget)? * 8;
        // Header bits: CPE id, tag, common window (3 + 4 + 1), END (3), FIL header (3 + 4 + 8 at most), alignment.
        let overhead = 8 + 3 + 15 + 8;
        let mut sf = [0i32; 2];
        for (ch, s) in sf.iter_mut().enumerate() {
            // The smallest scale factor that keeps every value within 8191.
            let p = self.spec[ch]
                .iter()
                .fold(0f32, |m, v| m.max(v.abs()))
                .max(1e-9);
            let need = 4.0 * (p / 8191f32.powf(4.0 / 3.0)).log2() + 100.0;
            *s = (need.ceil() as i32).clamp(0, 255);
        }
        let mut chosen = None;
        for extra in 0..120 {
            let s = [(sf[0] + extra).min(255), (sf[1] + extra).min(255)];
            let (b0, m0) = self.channel_bits(0, s[0]);
            let (b1, m1) = self.channel_bits(1, s[1]);
            if b0 + b1 + overhead <= avail {
                chosen = Some((s, [m0, m1]));
                break;
            }
        }
        let (s, m) = chosen.unwrap_or_else(|| {
            self.q = [[0; FRAME]; 2];
            ([100, 100], [0, 0])
        });
        out.clear();
        let mut w = BitWriter::new(out);
        w.bytes(prefix);
        w.put(1, 3); // ID_CPE
        w.put(0, 4); // element_instance_tag
        w.put(0, 1); // common_window
        self.write_channel(&mut w, 0, s[0], m[0]);
        self.write_channel(&mut w, 1, s[1], m[1]);
        // A fill element up to the budget, then ID_END.
        let used = w.bits_written() + 3 + 3;
        let left_bits = (budget * 8).checked_sub(used).ok_or(Error::Budget)?;
        let fill_bytes = left_bits.saturating_sub(4 + 8) / 8;
        if left_bits >= 7 + 8 {
            w.put(6, 3); // ID_FIL
            let cnt = fill_bytes.min(15 + 254);
            if cnt >= 15 {
                w.put(15, 4);
                w.put((cnt - 14) as u64, 8);
            } else {
                w.put(cnt as u64, 4);
            }
            if cnt > 0 {
                w.put(0, 8); // EXT_FILL, fill nibble 0
                for _ in 1..cnt {
                    w.put(0xA5, 8);
                }
            }
        }
        w.put(7, 3); // ID_END
        w.finish();
        if out.len() > budget {
            return Err(Error::Budget);
        }
        out.resize(budget, 0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(f: usize, hz: f32) -> Vec<f32> {
        (0..FRAME)
            .map(|i| 0.5 * (std::f32::consts::TAU * hz * (f * FRAME + i) as f32 / 48_000.0).sin())
            .collect()
    }

    #[test]
    fn blocks_fit_their_budget() {
        let mut e = Encoder::new(48_000).unwrap();
        let mut b = Vec::new();
        for f in 0..20 {
            e.encode(&tone(f, 1000.0), &tone(f, 3000.0), &[], 98, &mut b)
                .unwrap();
            assert_eq!(b.len(), 98);
            // The block starts with ID_CPE (001) and tag 0.
            assert_eq!(b[0] >> 1, 0b001_0000);
        }
    }

    #[test]
    fn refuses_bad_input() {
        assert_eq!(Encoder::new(44_100).err(), Some(Error::SampleRate));
        let mut e = Encoder::new(32_000).unwrap();
        let mut b = Vec::new();
        let x = tone(0, 1000.0);
        assert_eq!(
            e.encode(&x[..10], &x, &[], 98, &mut b),
            Err(Error::InputLength)
        );
        assert_eq!(e.encode(&x, &x, &[0; 10], 5, &mut b), Err(Error::Budget));
        assert_eq!(e.encode(&x, &x, &[], 2, &mut b), Err(Error::Budget));
    }

    #[test]
    fn escape_lengths_of_clause_4_6_3_3() {
        // 16 is a zero separator and 4 bits: 5 bits. 8191 is 8 ones, a zero and 12 bits: 21 bits.
        assert_eq!(esc_bits(15), 0);
        assert_eq!(esc_bits(16), 5);
        assert_eq!(esc_bits(8191), 9 + 12);
    }
}
