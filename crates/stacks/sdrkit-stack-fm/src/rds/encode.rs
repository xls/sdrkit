//! The RDS sender: the group schedule (0A station name, 2A radio text, 4A clock time, extra groups), the blocks with their
//! checkwords and offset words, the differential coding, and the shaped biphase wave that modulates the 57 kHz subcarrier.
//! Standard: IEC 62106 (group layouts 0A, 2A and 4A, Annex B coding, the biphase symbol and its spectrum shaping).

use super::block::{encode_block, OFFSET_A, OFFSET_B, OFFSET_C, OFFSET_CP, OFFSET_D};
use sdrkit_util_text::ebu::RDS;

/// The RDS bit rate: 57 kHz / 48, bit/s.
pub const BIT_RATE: f64 = 1187.5;
/// The samples of the shaped wave in one bit.
pub const WAVE_OVERSAMPLE: usize = 128;
/// The bits that one shaped symbol spans.
pub const WAVE_SPAN: usize = 8;
/// The AF code that fills a pair.
const AF_FILLER: u8 = 205;
/// The AF code that counts the frequencies: 224 + n.
const AF_COUNT_BASE: u8 = 224;
/// The days from the modified Julian date 0 to 1970-01-01.
const MJD_UNIX_EPOCH: i64 = 40_587;

/// The RDS bytes of a text (IEC 62106 Annex E). A character the table lacks is sent as `?`.
pub fn rds_bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| RDS.byte_of(c).unwrap_or(b'?'))
        .collect()
}

/// What the RDS groups carry.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RdsContent {
    /// Programme identification.
    pub pi: u16,
    /// Programme service name: padded with spaces or cut to 8 characters.
    pub ps: String,
    /// Radio text: cut to 64 characters; empty sends no 2A groups.
    pub rt: String,
    /// The text A/B flag.
    pub rt_ab: bool,
    /// Programme type, 0 to 31.
    pub pty: u8,
    /// Traffic programme.
    pub tp: bool,
    /// Traffic announcement.
    pub ta: bool,
    /// Music (true) or speech.
    pub music: bool,
    /// Stereo: the decoder identification bit d0.
    pub stereo: bool,
    /// Alternative frequencies as AF codes 1 to 204 (87.5 MHz + 0.1 MHz times the code), at most 25, sent by method A.
    pub af_codes: Vec<u8>,
    /// Extra groups as blocks B, C and D, sent as every sixth group.
    pub extra_groups: Vec<[u16; 3]>,
    /// The clock group 4A with this local offset in half hours (−24 to 24), or `None` for no clock group.
    pub clock_offset: Option<i8>,
}

/// The 26-bit words of group `g` (blocks A, B, C, D): each block with its checkword and offset word, C′ for a version B group.
pub fn group_words(g: [u16; 4]) -> [u32; 4] {
    let c = if g[1] & 0x0800 != 0 {
        OFFSET_CP
    } else {
        OFFSET_C
    };
    [
        encode_block(g[0], OFFSET_A),
        encode_block(g[1], OFFSET_B),
        encode_block(g[2], c),
        encode_block(g[3], OFFSET_D),
    ]
}

/// Group 4A for the minute that starts at `unix_s` (UTC) with the local offset `offset` in half hours.
pub fn clock_group(pi: u16, common: u16, unix_s: f64, offset: i8) -> [u16; 4] {
    let minutes = (unix_s / 60.0).floor() as i64;
    let mjd = minutes.div_euclid(1440) + MJD_UNIX_EPOCH;
    let hour = (minutes.rem_euclid(1440) / 60) as u16;
    let minute = (minutes.rem_euclid(60)) as u16;
    let mjd = mjd.clamp(0, 0x1_FFFF) as u32;
    let b = (4 << 12) | common | (mjd >> 15) as u16;
    let c = ((mjd & 0x7FFF) << 1) as u16 | (hour >> 4);
    let sign = u16::from(offset < 0);
    let d = ((hour & 0xF) << 12)
        | (minute << 6)
        | (sign << 5)
        | (offset.unsigned_abs().min(31) as u16);
    [pi, b, c, d]
}

/// The station name, the text segments and the AF pairs of a content.
#[derive(Clone, Debug)]
struct Prepared {
    ps: [u8; 8],
    rt_segs: Vec<[u8; 4]>,
    af_pairs: Vec<u16>,
}

impl Prepared {
    fn of(c: &RdsContent) -> Prepared {
        let mut ps = [b' '; 8];
        for (d, s) in ps.iter_mut().zip(rds_bytes(&c.ps)) {
            *d = s;
        }
        let mut rt = rds_bytes(&c.rt);
        rt.truncate(64);
        if !rt.is_empty() && rt.len() < 64 {
            rt.push(0x0D);
        }
        while !rt.len().is_multiple_of(4) {
            rt.push(b' ');
        }
        let rt_segs = rt.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        let codes: Vec<u8> = c
            .af_codes
            .iter()
            .copied()
            .filter(|c| (1..=204).contains(c))
            .take(25)
            .collect();
        let mut af = Vec::new();
        if !codes.is_empty() {
            af.push(AF_COUNT_BASE + codes.len() as u8);
            af.extend_from_slice(&codes);
        }
        if af.len() % 2 == 1 || af.is_empty() {
            af.resize(af.len() + 1 + usize::from(af.is_empty()), AF_FILLER);
        }
        let af_pairs = af
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        Prepared {
            ps,
            rt_segs,
            af_pairs,
        }
    }
}

/// The RDS groups and bits of a station.
///
/// The schedule repeats five slots: four 0A groups and one 2A group of radio text (a 0A group when there is no text). With
/// extra groups, every sixth group is the next of them. With the clock on, the first group that starts after the minute
/// changes in UTC is 4A. A new content takes effect for the name, the codes and the flags at the next 0A group of segment
/// 0, and for the text at the next text cycle.
#[derive(Clone, Debug)]
pub struct RdsEncoder {
    c: RdsContent,
    prep: Prepared,
    pending: Option<RdsContent>,
    pending_text: Option<(String, bool)>,
    k: u64,
    slot: u8,
    ps_seg: u8,
    rt_seg: usize,
    af_i: usize,
    extra_i: usize,
    ct_minute: Option<i64>,
    groups: u64,
    bits: [u32; 4],
    bit_i: u8,
    sent: bool,
}

impl RdsEncoder {
    /// An encoder for content `c`.
    pub fn new(c: RdsContent) -> Self {
        RdsEncoder {
            prep: Prepared::of(&c),
            c,
            pending: None,
            pending_text: None,
            k: 0,
            slot: 0,
            ps_seg: 0,
            rt_seg: 0,
            af_i: 0,
            extra_i: 0,
            ct_minute: None,
            groups: 0,
            bits: [0; 4],
            bit_i: 104,
            sent: false,
        }
    }

    /// The content in force.
    pub fn content(&self) -> &RdsContent {
        &self.c
    }

    /// The groups made so far.
    pub fn groups(&self) -> u64 {
        self.groups
    }

    /// Replaces the content at the next boundaries of its parts (see [`RdsEncoder`]).
    pub fn set_content(&mut self, c: RdsContent) {
        if c.rt != self.c.rt || c.rt_ab != self.c.rt_ab {
            self.pending_text = Some((c.rt.clone(), c.rt_ab));
        } else {
            self.pending_text = None;
        }
        self.pending = Some(c);
    }

    fn take_fields(&mut self) {
        if let Some(n) = self.pending.take() {
            let (rt, ab) = (std::mem::take(&mut self.c.rt), self.c.rt_ab);
            self.c = RdsContent { rt, rt_ab: ab, ..n };
            let segs = std::mem::take(&mut self.prep.rt_segs);
            self.prep = Prepared::of(&self.c);
            self.prep.rt_segs = segs;
            self.af_i = 0;
        }
    }

    fn take_text(&mut self) {
        if let Some((rt, ab)) = self.pending_text.take() {
            self.c.rt = rt;
            self.c.rt_ab = ab;
            self.prep.rt_segs = Prepared::of(&self.c).rt_segs;
            self.rt_seg = 0;
        }
    }

    /// The next group as blocks A, B, C, D. `unix_s` is the time the group starts on air, if known.
    pub fn next_group(&mut self, unix_s: Option<f64>) -> [u16; 4] {
        self.groups += 1;
        let k = self.k;
        self.k += 1;
        if !self.c.extra_groups.is_empty() && k % 6 == 5 {
            let g = self.c.extra_groups[self.extra_i % self.c.extra_groups.len()];
            self.extra_i += 1;
            return [self.c.pi, g[0], g[1], g[2]];
        }
        if let (Some(offset), Some(t)) = (self.c.clock_offset, unix_s) {
            let minute = (t / 60.0).floor() as i64;
            match self.ct_minute {
                Some(m) if m != minute => {
                    self.ct_minute = Some(minute);
                    return clock_group(self.c.pi, self.common(), t, offset);
                }
                None => self.ct_minute = Some(minute),
                _ => {}
            }
        }
        let slot = self.slot;
        self.slot = (self.slot + 1) % 5;
        if slot == 4 {
            if self.rt_seg == 0 {
                self.take_text();
            }
            if !self.prep.rt_segs.is_empty() {
                return self.text_group();
            }
        }
        if self.ps_seg == 0 {
            self.take_fields();
        }
        self.name_group()
    }

    /// TP and PTY as block B holds them.
    fn common(&self) -> u16 {
        (u16::from(self.c.tp) << 10) | (u16::from(self.c.pty & 0x1F) << 5)
    }

    /// Group 0A of the next segment of the station name.
    fn name_group(&mut self) -> [u16; 4] {
        let seg = self.ps_seg;
        self.ps_seg = (self.ps_seg + 1) % 4;
        // Decoder identification: d3 is sent in segment 0 and d0 (stereo) in segment 3; d1 and d2 stay 0.
        let di = u16::from(seg == 3 && self.c.stereo);
        let b = self.common()
            | (u16::from(self.c.ta) << 4)
            | (u16::from(self.c.music) << 3)
            | (di << 2)
            | u16::from(seg);
        let c = self.prep.af_pairs[self.af_i % self.prep.af_pairs.len()];
        self.af_i += 1;
        let s = 2 * usize::from(seg);
        let d = u16::from_be_bytes([self.prep.ps[s], self.prep.ps[s + 1]]);
        [self.c.pi, b, c, d]
    }

    /// Group 2A of the next segment of the radio text.
    fn text_group(&mut self) -> [u16; 4] {
        let seg = self.rt_seg;
        self.rt_seg = (self.rt_seg + 1) % self.prep.rt_segs.len();
        let s = self.prep.rt_segs[seg];
        let b = (2 << 12) | self.common() | (u16::from(self.c.rt_ab) << 4) | seg as u16;
        [
            self.c.pi,
            b,
            u16::from_be_bytes([s[0], s[1]]),
            u16::from_be_bytes([s[2], s[3]]),
        ]
    }

    /// The next bit on air: the data bit (each block most significant bit first) differentially coded, starting from 0.
    /// `unix_s` is the time of the bit on air, if known; it times the clock group.
    pub fn next_bit(&mut self, unix_s: Option<f64>) -> bool {
        if self.bit_i >= 104 {
            self.bits = group_words(self.next_group(unix_s));
            self.bit_i = 0;
        }
        let w = self.bits[usize::from(self.bit_i / 26)];
        let data = (w >> (25 - u32::from(self.bit_i % 26))) & 1 == 1;
        self.bit_i += 1;
        self.sent ^= data;
        self.sent
    }
}

/// The biphase pulse `h(t)` of the spectrum shaping, `t` in bits: the inverse transform of `cos(π f T / 4)` for
/// `|f| <= 2 / T`, scaled to 1 at `t = 0`.
fn shaping_pulse(t: f64) -> f64 {
    let u = 8.0 * t;
    if (u.abs() - 1.0).abs() < 1e-9 {
        std::f64::consts::FRAC_PI_4
    } else {
        (std::f64::consts::FRAC_PI_2 * u).cos() / (1.0 - u * u)
    }
}

/// The shaped biphase wave of the RDS bits, at [`BIT_RATE`] times [`WAVE_OVERSAMPLE`] samples a second.
///
/// Each bit sent is an impulse pair, `+δ(t − T/4)` and `−δ(t − 3T/4)` for a 1 (negated for a 0), through the filter
/// `cos(π f T / 4)` for `|f| <= 2 / T`: the data spectrum is zero beyond 2375 Hz from the subcarrier. The symbol is
/// cut to [`WAVE_SPAN`] bits and centred in them, so the wave lags the bit clock by 3.5 bits. Its magnitude never passes 1: the symbol is
/// scaled by its largest sum over any bit pattern.
#[derive(Clone, Debug)]
pub struct RdsWave {
    table: Vec<f32>,
    symbols: [f32; WAVE_SPAN],
    newest: usize,
    phase: usize,
}

impl Default for RdsWave {
    fn default() -> Self {
        Self::new()
    }
}

impl RdsWave {
    /// The wave of no bits yet.
    pub fn new() -> Self {
        let n = WAVE_SPAN * WAVE_OVERSAMPLE;
        let lag = WAVE_SPAN as f64 / 2.0 - 0.5;
        let mut table: Vec<f64> = (0..n)
            .map(|i| {
                let x = i as f64 / WAVE_OVERSAMPLE as f64 - lag;
                shaping_pulse(x - 0.25) - shaping_pulse(x - 0.75)
            })
            .collect();
        let worst = (0..WAVE_OVERSAMPLE)
            .map(|p| {
                (0..WAVE_SPAN)
                    .map(|k| table[p + k * WAVE_OVERSAMPLE].abs())
                    .sum::<f64>()
            })
            .fold(0.0, f64::max);
        table.iter_mut().for_each(|v| *v /= worst);
        RdsWave {
            table: table.into_iter().map(|v| v as f32).collect(),
            symbols: [0.0; WAVE_SPAN],
            newest: 0,
            phase: 0,
        }
    }

    /// The sample rate of the wave, Hz.
    pub fn rate() -> f64 {
        BIT_RATE * WAVE_OVERSAMPLE as f64
    }

    /// The next sample. At the start of each bit `bit` gives the next bit sent.
    #[inline]
    pub fn next(&mut self, bit: impl FnOnce() -> bool) -> f32 {
        if self.phase == 0 {
            self.newest = (self.newest + 1) % WAVE_SPAN;
            self.symbols[self.newest] = if bit() { 1.0 } else { -1.0 };
        }
        let mut s = 0.0;
        for k in 0..WAVE_SPAN {
            let sym = self.symbols[(self.newest + WAVE_SPAN - k) % WAVE_SPAN];
            s += sym * self.table[self.phase + k * WAVE_OVERSAMPLE];
        }
        self.phase = (self.phase + 1) % WAVE_OVERSAMPLE;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wave_stays_within_one_and_is_a_biphase_symbol() {
        let mut w = RdsWave::new();
        let mut x = Vec::new();
        let mut i = 0u32;
        for _ in 0..WAVE_OVERSAMPLE * 64 {
            x.push(w.next(|| {
                i = i.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                i >> 16 & 1 == 1
            }));
        }
        assert!(x.iter().all(|v| v.abs() <= 1.0 + 1e-6));
        assert!(x.iter().any(|v| v.abs() > 0.5));
        // The symbol of a 1: positive in the first half of its bit, negative in the second, 3.5 bits late.
        let lag = (WAVE_SPAN / 2 - 1) * WAVE_OVERSAMPLE + WAVE_OVERSAMPLE / 2;
        let q = WAVE_OVERSAMPLE / 4;
        assert!(w.table[lag + q] > 0.0 && w.table[lag + 3 * q] < 0.0);
    }
}
