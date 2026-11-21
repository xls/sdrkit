//! The science packets of the microwave radiometer of the Arctic Weather Satellite: the packet data field, the scan record
//! and the picture rows.
//!
//! Standard: Arctic Weather Satellite Direct Data Broadcast Interface Control Document AWS-OSE-ICD-0083, sections 5.2
//! (source packet), 6.1 and Table 6-4 (science record), Figure 6-2 (spot).

use sdrkit_codec_image::strip::Strip;
use std::sync::Arc;

/// The spacecraft id of the transfer frames, 10 bits.
pub const SPACECRAFT: u16 = 104;
/// The virtual channel of the direct broadcast.
pub const SCIENCE_VC: u8 = 3;
/// The virtual channel of idle frames.
pub const IDLE_VC: u8 = 7;
/// The APID of the science packets.
pub const SCIENCE_APID: u16 = 100;
/// Bytes of the data field header of a packet: version and time status, service type, subtype, message counter,
/// destination, time preamble, 4 bytes of seconds and 3 of the fraction.
pub const FIELD_HEADER: usize = 15;
/// The service type of a science packet.
pub const SERVICE_SCIENCE: u8 = 128;
/// Bytes of a scan record.
pub const RECORD_BYTES: usize = 7599;
/// The place of the first scene spot in the record.
pub const SCENE_OFFSET: usize = 1799;
/// Bytes of a spot: 19 values of 16 bits, low byte first, and an encoder value.
pub const SPOT_BYTES: usize = 40;
/// Scene spots of a scan.
pub const SPOTS: usize = 145;
/// Channels: 8 of the 54 GHz receiver, 1 of the 89 GHz receiver, 6 of the 183 GHz receiver, 4 of the 325 GHz receiver.
pub const CHANNELS: usize = 19;
/// The most rows of a picture.
pub const MAX_ROWS: u32 = 8000;

/// Sample of spot `p` and channel `c` of a record of at least 7599 bytes.
pub fn sample(record: &[u8], p: usize, c: usize) -> u16 {
    let at = SCENE_OFFSET + SPOT_BYTES * p + 2 * c;
    u16::from_le_bytes([record[at], record[at + 1]])
}

/// The data field header of a science packet at `seconds` of the on-board clock.
pub fn field_header(seconds: u32) -> [u8; FIELD_HEADER] {
    let mut h = [0u8; FIELD_HEADER];
    h[0] = 0x20;
    h[1] = SERVICE_SCIENCE;
    h[2] = 25;
    h[7] = 0x2F;
    h[8..12].copy_from_slice(&seconds.to_be_bytes());
    h
}

/// Counters of [`MwrDecoder`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MwrStats {
    /// Science packets taken.
    pub packets: u64,
    /// Science packets whose error control field was wrong.
    pub crc_errors: u64,
    /// Scan records closed.
    pub scans: u64,
    /// Scan records that were not 7599 bytes long and were cut or filled with zeros.
    pub odd_records: u64,
    /// Rows of the picture.
    pub rows: u32,
}

/// Gathers the science packets of a scan into its record and gives the scan out as 19 strips of 145 pixels. A pixel is the
/// 16-bit sample scaled between a floor and a ceiling that follow each channel slowly.
#[derive(Clone, Debug)]
pub struct MwrDecoder {
    record: Vec<u8>,
    open: bool,
    range: [Option<(f32, f32)>; CHANNELS],
    /// Counters.
    pub stats: MwrStats,
}

impl Default for MwrDecoder {
    fn default() -> Self {
        MwrDecoder {
            record: Vec::with_capacity(RECORD_BYTES + 1024),
            open: false,
            range: [None; CHANNELS],
            stats: MwrStats::default(),
        }
    }
}

impl MwrDecoder {
    /// Starts a new picture.
    pub fn new_picture(&mut self) {
        self.stats.rows = 0;
        self.range = [None; CHANNELS];
    }

    /// Forgets the record in progress.
    pub fn restart(&mut self) {
        self.open = false;
        self.record.clear();
    }

    /// Takes the data field of a science packet without its error control field, with the sequence flags of the packet (1
    /// first, 0 continuation, 2 last, 3 not segmented).
    pub fn push(&mut self, flags: u8, data: &[u8], crc_ok: bool, out: &mut Vec<Strip>) {
        if data.len() < FIELD_HEADER {
            return;
        }
        self.stats.packets += 1;
        self.stats.crc_errors += u64::from(!crc_ok);
        let source = &data[FIELD_HEADER..];
        match flags & 3 {
            1 => {
                // A first packet while a record is open closes that record as it is.
                self.close(out);
                self.record.extend_from_slice(source);
                self.open = true;
            }
            0 if self.open => self.record.extend_from_slice(source),
            2 if self.open => {
                self.record.extend_from_slice(source);
                self.close(out);
            }
            3 => {
                self.close(out);
                self.record.extend_from_slice(source);
                self.open = true;
                self.close(out);
            }
            _ => {}
        }
    }

    fn close(&mut self, out: &mut Vec<Strip>) {
        if !std::mem::take(&mut self.open) {
            self.record.clear();
            return;
        }
        self.stats.scans += 1;
        self.stats.odd_records += u64::from(self.record.len() != RECORD_BYTES);
        self.record.resize(RECORD_BYTES, 0);
        let row = self.stats.rows;
        if row < MAX_ROWS {
            for c in 0..CHANNELS {
                let values: Vec<f32> = (0..SPOTS)
                    .map(|p| f32::from(sample(&self.record, p, c)))
                    .collect();
                let lo = values.iter().copied().fold(f32::INFINITY, f32::min);
                let hi = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let (floor, ceiling) = match self.range[c] {
                    None => (lo, hi),
                    Some((f, c)) => (
                        (0.98 * f + 0.02 * lo).min(lo),
                        (0.98 * c + 0.02 * hi).max(hi),
                    ),
                };
                self.range[c] = Some((floor, ceiling));
                let span = (ceiling - floor).max(1.0);
                let px: Vec<u8> = values
                    .iter()
                    .map(|v| ((v - floor) / span * 255.0).round().clamp(0.0, 255.0) as u8)
                    .collect();
                let mut s = Strip::new(c as u16 + 1, row, 0, 1, SPOTS as u32, Arc::from(px));
                s.instrument = "MWR";
                out.push(s);
            }
            self.stats.rows += 1;
        }
        self.record.clear();
    }
}

/// The scan record of line `line` of the test signal: the sample of spot `p` and channel `c` is `64 value(c, 8 p, line)`,
/// every other byte 0.
pub fn test_record(line: u32, value: impl Fn(u16, u32, u32) -> u16) -> Vec<u8> {
    let mut r = vec![0u8; RECORD_BYTES];
    for p in 0..SPOTS {
        for c in 0..CHANNELS {
            let v = value(c as u16, 8 * p as u32, line) * 64;
            let at = SCENE_OFFSET + SPOT_BYTES * p + 2 * c;
            r[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    r
}
