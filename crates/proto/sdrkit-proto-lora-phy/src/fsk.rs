//! The FSK modulation of the Semtech LoRa radios as LoRaWAN uses it: 50 kbit/s, 25 kHz deviation, Gaussian BT 1.0 or no shaping,
//! sync word C1 94 C1, a length byte, the payload and a CRC-16/SEMTECH-FSK, whitened with PN9.
//!
//! Standard: LoRa Alliance RP002-1.0.4 clause 4.2

use crate::channel::ChannelFilter;
use crate::error::{LoraError, Param, Result};
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC16_SEMTECH_FSK;
use sdrkit_util_bits::seq::ByteSequence;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The bit rate, bit/s.
pub const BIT_RATE: f64 = 50_000.0;
/// The deviation, Hz.
pub const DEVIATION: f64 = 25_000.0;
/// The sync word.
pub const SYNC: [u8; 3] = [0xC1, 0x94, 0xC1];

/// The sync word as levels (+1 for a 1 bit), first bit first.
const SYNC_LEVELS: [f32; 24] = {
    let mut t = [0f32; 24];
    let mut i = 0;
    while i < 24 {
        t[i] = if (SYNC[i / 8] >> (7 - i % 8)) & 1 == 1 {
            1.0
        } else {
            -1.0
        };
        i += 1;
    }
    t
};

fn pn9() -> ByteSequence {
    // The key is in the sdrkit-util-bits list, so the lookup always succeeds.
    ByteSequence::named("pn9_whitening").unwrap_or_else(|| ByteSequence::table(&[]))
}

/// Whitens or de-whitens bytes with PN9 from its start. The operation is its own inverse.
pub fn whiten(b: &mut [u8]) {
    pn9().apply(b);
}

/// The CRC of the length byte and the payload.
pub fn crc(data: &[u8]) -> u16 {
    CRC16_SEMTECH_FSK.checksum(data)
}

/// Appends the bytes on the air to `out`: the preamble, the sync word, then the whitened length, payload and CRC (high byte
/// first). If the payload is longer than 255 bytes, this returns an error.
pub fn encode(
    payload: &[u8],
    preamble_byte: u8,
    preamble_len: usize,
    out: &mut Vec<u8>,
) -> Result<()> {
    if payload.len() > 255 {
        return Err(LoraError::PayloadTooLong { len: payload.len() });
    }
    out.extend(std::iter::repeat_n(preamble_byte, preamble_len));
    out.extend_from_slice(&SYNC);
    let body = out.len();
    out.push(payload.len() as u8);
    out.extend_from_slice(payload);
    let c = crc(&out[body..]);
    out.extend_from_slice(&c.to_be_bytes());
    whiten(&mut out[body..]);
    Ok(())
}

/// The time on air of a payload of `len` bytes with a 5-byte preamble, seconds.
pub fn airtime(len: usize) -> f64 {
    ((5 + 3 + 1 + len + 2) * 8) as f64 / BIT_RATE
}

/// The clean signal of the bytes `bytes` at `rate`, most significant bit first, at 0 Hz and unit amplitude: Gaussian BT 1.0
/// when `shaped`, else unshaped CPFSK. If the rate is below twice the bit rate, this returns an error.
pub fn modulate(bytes: &[u8], rate: f64, shaped: bool) -> Result<Vec<Complex32>> {
    if !(rate.is_finite() && rate >= 2.0 * BIT_RATE) {
        return Err(LoraError::BadParameter(Param::FskRate));
    }
    let sps = rate / BIT_RATE;
    let h = 2.0 * DEVIATION / BIT_RATE;
    let mut m = if shaped {
        Cpm::gfsk(1.0, h, sps)
    } else {
        Cpm::cpfsk(h, sps)
    }
    .map_err(LoraError::Sim)?;
    let mut levels = Vec::with_capacity(bytes.len() * 8);
    for b in bytes {
        for k in (0..8).rev() {
            levels.push(if (b >> k) & 1 == 1 { 1.0 } else { -1.0 });
        }
    }
    let mut out = Vec::with_capacity((levels.len() as f64 * sps) as usize + 64);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    Ok(out)
}

/// A frame that the FSK receiver found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FskFrame {
    /// The PHY payload.
    pub payload: Vec<u8>,
    /// The carrier offset from the channel, Hz.
    pub offset_hz: f64,
    /// The measured deviation, Hz.
    pub deviation_hz: f64,
    /// The tones were the other way round.
    pub inverted: bool,
    /// The level, dBFS.
    pub level_db: f64,
    /// The SNR in 100 kHz, dB.
    pub snr_db: f64,
    /// The correlation score of the sync word, 0 to 1.
    pub score: f64,
    /// The input index of the first bit of the sync word.
    pub start: u64,
}

/// The counters of the FSK receiver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FskStats {
    /// Strong sync words.
    pub syncs: u64,
    /// Frames with a right CRC.
    pub frames: u64,
    /// CRC failures after a strong sync word.
    pub crc_errors: u64,
}

/// The FSK receiver of one channel.
pub struct FskRx {
    filter: ChannelFilter,
    rate: f64,
    decim: f64,
    spb: f64,
    tmp: Vec<Complex32>,
    prev: Complex32,
    /// Discriminator values averaged over most of a bit, Hz.
    avg: VecDeque<f32>,
    pow: VecDeque<f32>,
    raw: VecDeque<f32>,
    body: Vec<u8>,
    first: u64,
    sum: f64,
    win: usize,
    skip_to: u64,
    /// The counters.
    pub stats: FskStats,
}

impl FskRx {
    /// A receiver of the channel at `offset_hz` in an input at `rate`. If the rate is too low for the 110 kHz channel, this
    /// returns an error.
    pub fn new(rate: f64, offset_hz: f64) -> Result<Self> {
        let filter = ChannelFilter::new(rate, offset_hz, 110e3)?;
        let out = filter.output_rate();
        let spb = out / BIT_RATE;
        let win = (spb * 0.6).round().max(1.0) as usize;
        Ok(FskRx {
            filter,
            rate,
            decim: rate / out,
            spb,
            tmp: Vec::new(),
            prev: Complex32::new(1.0, 0.0),
            avg: VecDeque::new(),
            pow: VecDeque::new(),
            raw: VecDeque::with_capacity(win + 1),
            body: Vec::with_capacity(258),
            first: 0,
            sum: 0.0,
            win,
            skip_to: 0,
            stats: FskStats::default(),
        })
    }

    /// The sample rate of the channel, Hz.
    pub fn channel_rate(&self) -> f64 {
        self.rate / self.decim
    }

    fn at(&self, pos: f64) -> f32 {
        let i = pos.floor();
        let fr = (pos - i) as f32;
        let i = i as i64 - self.first as i64;
        if i < 0 || (i as usize + 1) >= self.avg.len() {
            return 0.0;
        }
        let i = i as usize;
        self.avg[i] * (1.0 - fr) + self.avg[i + 1] * fr
    }

    /// Feeds input. Frames with a right CRC go to `out`.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<FskFrame>) {
        self.tmp.clear();
        self.filter.process(input, &mut self.tmp);
        let k = self.channel_rate() / std::f64::consts::TAU;
        for &x in &self.tmp {
            let z = x * self.prev.conj();
            let d = atan2(z.im, z.re) as f64 * k;
            self.prev = x;
            let d = d.clamp(-80e3, 80e3) as f32;
            self.raw.push_back(d);
            self.sum += d as f64;
            if self.raw.len() > self.win {
                if let Some(old) = self.raw.pop_front() {
                    self.sum -= old as f64;
                }
            }
            self.avg
                .push_back((self.sum / self.raw.len() as f64) as f32);
            self.pow.push_back(x.norm_sqr());
        }
        let spb = self.spb;
        // Search the newest positions for a sync word.
        let need_before = 40.0 * spb;
        let frame_max = (8.0 * (1 + 255 + 2) as f64 + 2.0) * spb;
        let end = self.first + self.avg.len() as u64;
        let mut pos = (self.first as f64 + need_before).max(self.skip_to as f64) as u64;
        // A position is searched when the longest frame after it is in the buffer.
        while (pos as f64 + frame_max) < end as f64 {
            // The centre of the last bit of the sync word is at pos.
            let p = pos as f64;
            let mut m = 0.0f32;
            for j in 0..16 {
                m += self.at(p - (24.0 + j as f64) * spb);
            }
            m /= 16.0;
            let mut score = 0.0f32;
            let mut dev = 0.0f32;
            for (j, s) in SYNC_LEVELS.iter().enumerate() {
                let v = self.at(p - (23 - j) as f64 * spb) - m;
                score += s * v;
                dev += v.abs();
            }
            let pre_dev: f32 = (0..16)
                .map(|j| (self.at(p - (24.0 + j as f64) * spb) - m).abs())
                .sum::<f32>()
                / 16.0;
            dev /= 24.0;
            let norm = if dev > 1000.0 {
                score / (24.0 * dev)
            } else {
                0.0
            };
            if !(norm.abs() > 0.85 && pre_dev > 0.5 * dev && dev > 8e3) {
                pos += 1;
                continue;
            }
            // Refine to the best position nearby.
            let mut best = (norm.abs(), p);
            for dpos in 1..(spb.ceil() as i64) {
                let q = p + dpos as f64;
                let mut sc = 0.0f32;
                for (j, s) in SYNC_LEVELS.iter().enumerate() {
                    sc += s * (self.at(q - (23 - j) as f64 * spb) - m);
                }
                let nn = (sc / (24.0 * dev)).abs();
                if nn > best.0 {
                    best = (nn, q);
                }
            }
            let inverted = norm < 0.0;
            self.stats.syncs += 1;
            let q = best.1;
            let byte = |rx: &Self, b: usize| -> u8 {
                (0..8).fold(0u8, |a, k| {
                    let v = rx.at(q + (b * 8 + k + 1) as f64 * spb) - m;
                    (a << 1) | ((v > 0.0) != inverted) as u8
                })
            };
            let mut len_b = [byte(self, 0)];
            whiten(&mut len_b);
            let len = len_b[0] as usize;
            let total = 1 + len + 2;
            let mut body = std::mem::take(&mut self.body);
            body.clear();
            for b in 0..total {
                body.push(byte(self, b));
            }
            whiten(&mut body);
            let c = u16::from_be_bytes([body[total - 2], body[total - 1]]);
            if len > 0 && crc(&body[..total - 2]) == c {
                self.stats.frames += 1;
                let start_ch = (q - 24.0 * spb).max(self.first as f64);
                let end_ch = q + (total * 8) as f64 * spb;
                let i0 = (start_ch as u64 - self.first) as usize;
                let i1 = ((end_ch as u64).saturating_sub(self.first) as usize).min(self.pow.len());
                let i1 = i1.max(i0 + 1).min(self.pow.len());
                let pw: f64 = self
                    .pow
                    .range(i0.min(i1)..i1)
                    .map(|&x| x as f64)
                    .sum::<f64>()
                    / (i1 - i0.min(i1)).max(1) as f64;
                let bits = (total * 8).min(400);
                let mut var = 0.0f64;
                for i in 0..bits {
                    let v = (self.at(q + (i + 1) as f64 * spb) - m).abs() as f64 - dev as f64;
                    var += v * v;
                }
                var /= bits as f64;
                let snr = 10.0 * ((dev as f64).powi(2) / var.max(1.0)).log10() - 3.0;
                out.push(FskFrame {
                    payload: body[1..total - 2].to_vec(),
                    offset_hz: m as f64,
                    deviation_hz: dev as f64,
                    inverted,
                    level_db: 10.0 * pw.max(1e-20).log10(),
                    snr_db: snr,
                    score: best.0 as f64,
                    start: (start_ch * self.decim) as u64,
                });
                self.body = body;
                pos = end_ch as u64;
                self.skip_to = pos;
            } else {
                self.body = body;
                self.stats.crc_errors += 1;
                pos = (q + 8.0 * spb) as u64;
            }
        }
        self.skip_to = self.skip_to.max(pos);
        // Keep what the search still needs.
        let keep_from = (pos as f64 - need_before - 2.0).max(0.0) as u64;
        while self.first < keep_from && !self.avg.is_empty() {
            self.avg.pop_front();
            self.pow.pop_front();
            self.first += 1;
        }
    }
}
