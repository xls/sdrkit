//! Meteor-M MSU-MR image packets: APID 64 to 69 carry channels 1 to 6. Each packet holds one 8-line row part of 14
//! JPEG-coded 8 x 8 blocks, placed in the picture by its on-board time.
//! Standard: ITU-T T.81 annex K (the Huffman and quantisation tables)

use crate::items::SpacePacket;
use sdrkit_codec_image::jpeg::tables::{scaled, LUMA_Q};
use sdrkit_codec_image::jpeg::{
    decode_block, dequantise, encode_block, fdct_8x8, idct_8x8, quantise, EntropyReader,
    EntropyWriter, StandardTables,
};
use sdrkit_codec_image::strip::Strip;
use std::sync::Arc;

/// Pixels in a picture line.
pub const LINE_WIDTH: u32 = 1568;
/// Blocks per packet.
pub const BLOCKS: usize = 14;
/// Lines per row (strip).
pub const ROW_LINES: u32 = 8;
/// Packets in a period (14 for each of three channels and one of telemetry).
pub const PERIOD: u16 = 43;
/// The most rows kept.
pub const MAX_ROWS: u32 = 1500;

/// A decoded image packet.
#[derive(Clone, Debug, PartialEq)]
pub struct MsuPacket {
    /// Channel 1 to 6.
    pub channel: u16,
    /// Day.
    pub day: u16,
    /// Milliseconds of the day (on-board, Moscow time).
    pub ms: u32,
    /// First MCU (0, 14 ... 182).
    pub mcu: u8,
    /// Quality factor.
    pub quality: u8,
    /// 8 x 112 pixels.
    pub pixels: Vec<u8>,
}

/// Why an image packet was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Shorter than 15 bytes.
    Short,
    /// Quality 0 or above 100.
    Quality,
    /// MCU number not a multiple of 14 or above 195.
    Mcu,
    /// The Huffman data.
    Data,
}

/// The quantisation table of quality factor `q` (1 to 100): the luminance table of T.81 annex K scaled as the Independent JPEG
/// Group scales it, with no upper limit on an entry.
pub fn quant_table(q: u8) -> [u16; 64] {
    scaled(&LUMA_Q, q as u32, u16::MAX)
}

/// Decodes `blocks` 8 x 8 blocks of `data` with quality `q` into 8 rows of 8 x `blocks` pixels. The DC predictor starts at 0.
/// If the data is cut short or holds a code that is not in the tables, it returns [`Refused::Data`].
pub fn decode_blocks(data: &[u8], blocks: usize, q: u8, out: &mut [u8]) -> Result<(), Refused> {
    let width = blocks * 8;
    if out.len() < 8 * width {
        return Err(Refused::Data);
    }
    let t = StandardTables::get();
    let qt = quant_table(q);
    let mut r = EntropyReader::new(data, false);
    let mut pred = 0i32;
    let (mut zz, mut coefs) = ([0i16; 64], [0f32; 64]);
    for b in 0..blocks {
        decode_block(&mut r, &t.dc_luma, &t.ac_luma, &mut pred, &mut zz)
            .map_err(|_| Refused::Data)?;
        dequantise(&zz, &qt, &mut coefs);
        idct_8x8(&coefs, &mut out[b * 8..], width);
    }
    if r.overrun() {
        return Err(Refused::Data);
    }
    Ok(())
}

/// Codes 8 rows of 8 x `blocks` pixels at quality `q` (the generator's side), padded with 1 bits to a whole byte.
pub fn encode_blocks(pixels: &[u8], blocks: usize, q: u8) -> Vec<u8> {
    let width = blocks * 8;
    let t = StandardTables::get();
    let qt = quant_table(q);
    let mut out = Vec::with_capacity(blocks * 32);
    if pixels.len() < 8 * width {
        return out;
    }
    let mut w = EntropyWriter::new(&mut out, false);
    let mut pred = 0i32;
    let (mut zz, mut coefs) = ([0i16; 64], [0f32; 64]);
    for b in 0..blocks {
        fdct_8x8(&pixels[b * 8..], width, &mut coefs);
        quantise(&coefs, &qt, &mut zz);
        if encode_block(&mut w, &t.dc_luma, &t.ac_luma, &mut pred, &zz).is_err() {
            break;
        }
    }
    w.align();
    out
}

/// Decodes the user data of an image packet of `channel`.
pub fn decode_packet(channel: u16, data: &[u8]) -> Result<MsuPacket, Refused> {
    if data.len() < 15 {
        return Err(Refused::Short);
    }
    let day = u16::from_be_bytes([data[0], data[1]]);
    let ms = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
    let mcu = data[8];
    let quality = data[13];
    if quality == 0 || quality > 100 {
        return Err(Refused::Quality);
    }
    if !mcu.is_multiple_of(14) || mcu > 195 {
        return Err(Refused::Mcu);
    }
    let mut pixels = vec![0u8; 8 * 8 * BLOCKS];
    decode_blocks(&data[14..], BLOCKS, quality, &mut pixels)?;
    Ok(MsuPacket {
        channel,
        day,
        ms,
        mcu,
        quality,
        pixels,
    })
}

/// Builds the user data of an image packet (the generator's side).
pub fn build_packet(day: u16, ms: u32, mcu: u8, quality: u8, pixels: &[u8]) -> Vec<u8> {
    let mut d = Vec::with_capacity(400);
    d.extend_from_slice(&day.to_be_bytes());
    d.extend_from_slice(&ms.to_be_bytes());
    d.extend_from_slice(&[0, 0]); // microseconds
    d.push(mcu);
    d.extend_from_slice(&[0, 0, 0, 0]); // scan and segment headers
    d.push(quality);
    d.extend_from_slice(&encode_blocks(pixels, BLOCKS, quality));
    d
}

/// Places image packets in rows by their on-board time.
#[derive(Clone, Debug, Default)]
pub struct RowPlacer {
    started: bool,
    row: u32,
    row_ms: Option<f64>,
    row_seq: u16,
    row_time_ms: Option<f64>,
    last_mcu: [Option<u8>; 7],
    last_seq: Option<u16>,
    prev_row_start: Option<(f64, u16)>,
}

impl RowPlacer {
    /// The learnt row time, ms.
    pub fn row_time_ms(&self) -> Option<f64> {
        self.row_time_ms
    }

    /// Starts a new picture (row 0 again, nothing learnt kept but the row time).
    pub fn restart(&mut self) {
        let rt = self.row_time_ms;
        *self = RowPlacer::default();
        self.row_time_ms = rt;
    }

    /// Notes a packet of the shared counter that is not an image packet (telemetry), so gaps are seen.
    pub fn note_seq(&mut self, seq: u16) {
        self.last_seq = Some(seq);
    }

    /// The row of an image packet.
    pub fn place(&mut self, p: &MsuPacket, seq: u16) -> u32 {
        let t = p.ms as f64;
        let c = (p.channel as usize).min(6);
        if !self.started {
            self.started = true;
            self.row = 0;
            self.row_ms = Some(t);
            self.row_seq = seq;
            self.last_mcu = [None; 7];
            self.last_mcu[c] = Some(p.mcu);
            self.last_seq = Some(seq);
            return 0;
        }
        let mut placed = false;
        if let (Some(rt), Some(t0)) = (self.row_time_ms, self.row_ms) {
            let mut dt = t - t0;
            if dt < -43_200_000.0 {
                dt += 86_400_000.0; // forward across midnight
            }
            if dt > -rt / 2.0 && dt < 1_800_000.0 {
                let k = (dt / rt).round();
                if k >= 1.0 {
                    self.new_row(k as u32, t, seq, k == 1.0);
                }
                placed = true;
            }
        }
        if !placed {
            if let Some(last) = self.last_seq {
                let gap = seq.wrapping_sub(last) & 0x3FFF;
                if gap > PERIOD && gap < 8192 {
                    self.new_row((gap / PERIOD) as u32, t, seq, false);
                    self.last_mcu = [None; 7];
                }
            }
            // A packet whose block position does not move on for its channel starts a new row; so does one whose on-board time
            // is far (half a second) from the row of the row start (the first row of a pass can come in part).
            let far = self
                .row_ms
                .is_some_and(|t0| (t - t0).abs() > 500.0 && (t - t0).abs() < 43_200_000.0);
            if self.last_mcu[c].is_some_and(|m| p.mcu <= m) || far {
                self.new_row(1, t, seq, true);
            }
        }
        self.last_mcu[c] = Some(p.mcu);
        self.last_seq = Some(seq);
        self.row
    }

    fn new_row(&mut self, k: u32, t: f64, seq: u16, consecutive: bool) {
        // Learn the row time from consecutive rows within one period of the shared counter.
        if consecutive && k == 1 {
            if let Some(t0) = self.row_ms {
                let dseq = seq.wrapping_sub(self.row_seq) & 0x3FFF;
                let mut dt = t - t0;
                if dt < -43_200_000.0 {
                    dt += 86_400_000.0;
                }
                if dseq <= PERIOD && dt > 0.0 && dt < 20_000.0 {
                    self.row_time_ms = Some(match self.row_time_ms {
                        Some(r) => 0.8 * r + 0.2 * dt,
                        None => dt,
                    });
                }
            }
        }
        self.prev_row_start = self.row_ms.map(|t| (t, self.row_seq));
        self.row += k;
        self.row_ms = Some(t);
        self.row_seq = seq;
        self.last_mcu = [None; 7];
    }
}

/// Counters per channel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MsuStats {
    /// Image packets per channel (index 1 to 6).
    pub per_channel: [u64; 7],
    /// Telemetry packets.
    pub telemetry: u64,
    /// Other packets.
    pub other: u64,
    /// Packets refused.
    pub refused: u64,
    /// The last quality factor.
    pub quality: u8,
    /// The last on-board time, ms of day.
    pub onboard_ms: Option<u32>,
    /// Rows of the picture.
    pub rows: u32,
}

/// The MSU-MR decoder: packets in, strips out.
#[derive(Clone, Debug, Default)]
pub struct MsuMr {
    placer: RowPlacer,
    picture: u32,
    /// Counters.
    pub stats: MsuStats,
}

impl MsuMr {
    /// Starts a new picture.
    pub fn new_picture(&mut self) {
        self.picture += 1;
        self.placer.restart();
        self.stats.rows = 0;
    }
    /// Returns the row placer.
    pub fn placer(&self) -> &RowPlacer {
        &self.placer
    }
    /// Takes a packet, appending the strip it makes.
    pub fn push(&mut self, p: &SpacePacket, out: &mut Vec<Strip>) {
        match p.apid {
            64..=69 => {
                let ch = p.apid - 63;
                match decode_packet(ch, &p.data) {
                    Ok(m) => {
                        self.stats.per_channel[ch as usize] += 1;
                        self.stats.quality = m.quality;
                        self.stats.onboard_ms = Some(m.ms);
                        let row = self.placer.place(&m, p.seq);
                        if row >= MAX_ROWS {
                            return;
                        }
                        self.stats.rows = self.stats.rows.max(row + 1);
                        out.push(Strip {
                            picture: self.picture,
                            channel: ch,
                            first_row: row * ROW_LINES,
                            x: m.mcu as u32 * 8,
                            rows: ROW_LINES as u16,
                            width: (BLOCKS * 8) as u32,
                            samples: Arc::from(m.pixels),
                            mask: 1u64 << (m.mcu / 14),
                            onboard_s: Some(m.ms as f64 / 1000.0),
                            instrument: "MSU-MR",
                        });
                    }
                    Err(_) => {
                        self.stats.refused += 1;
                        self.stats.per_channel[ch as usize] += 1;
                        self.placer.note_seq(p.seq);
                    }
                }
            }
            70 => {
                self.stats.telemetry += 1;
                self.placer.note_seq(p.seq);
            }
            _ => self.stats.other += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(ch: u16, ms: u32, mcu: u8) -> MsuPacket {
        MsuPacket {
            channel: ch,
            day: 20_000,
            ms,
            mcu,
            quality: 80,
            pixels: vec![],
        }
    }

    #[test]
    fn a_strip_decodes_and_a_cut_one_is_refused() {
        let px: Vec<u8> = (0..8 * 112)
            .map(|i| ((i % 112) * 2 + i / 112 * 3) as u8)
            .collect();
        let d = build_packet(20_000, 1_234_567, 28, 80, &px);
        let m = decode_packet(1, &d).unwrap();
        assert_eq!((m.mcu, m.quality, m.ms), (28, 80, 1_234_567));
        let mse: f64 = px
            .iter()
            .zip(&m.pixels)
            .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
            .sum::<f64>()
            / px.len() as f64;
        assert!(10.0 * (65025.0 / mse.max(1e-9)).log10() > 30.0);
        assert!(decode_packet(1, &d[..d.len() / 2]).is_err());
        assert_eq!(decode_packet(1, &d[..10]), Err(Refused::Short));
    }

    #[test]
    fn rows_follow_time_through_gaps_and_wraps() {
        let mut pl = RowPlacer::default();
        let mut seq = 16_000u16;
        let mut rows = Vec::new();
        for r in 0..30u32 {
            let t = 50_000_000 + r * 1230;
            for ch in 1..=3u16 {
                for k in 0..14u8 {
                    seq = seq.wrapping_add(1);
                    if (10..15).contains(&r) {
                        continue; // a fade: these rows are lost
                    }
                    rows.push((r, pl.place(&packet(ch, t, k * 14), seq & 0x3FFF)));
                }
            }
            seq = seq.wrapping_add(1); // telemetry
        }
        for (want, got) in rows {
            assert_eq!(want, got);
        }
        let rt = pl.row_time_ms().unwrap();
        assert!((rt - 1230.0).abs() <= 1.0, "{rt}");
    }
}
