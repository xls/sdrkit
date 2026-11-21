//! DAB+ audio superframes: superframe synchronisation, the Reed-Solomon code with the bytes of lost logical frames as erasures,
//! the Fire code and header, access units with their CRC, and PAD in the data stream element. The AudioSpecificConfig and LATM
//! come from the shared AAC packaging.
//!
//! Standard: TS 102 563 clauses 5 and 6.

use crate::fec::crc_ok;
use sdrkit_algo_fec::rs::{codes::DAB_PLUS_120_110, Rs};
use sdrkit_core::media::Packet;
use sdrkit_graph::PacketPool;
use sdrkit_proto_bcdata::aac::{self, AudioSpecificConfig};
use sdrkit_util_bits::crc::catalog::CRC16_DAB_FIRE;
use std::sync::Arc;

/// The superframe header's audio parameters (TS 102 563 clause 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioParams {
    /// The header byte (low 7 bits used).
    pub header: u8,
    /// DAC rate: 48 kHz (true) or 32 kHz.
    pub dac_48k: bool,
    /// SBR present.
    pub sbr: bool,
    /// AAC channel mode stereo.
    pub stereo: bool,
    /// Parametric stereo.
    pub ps: bool,
    /// MPEG Surround configuration (reported, not decoded).
    pub surround: u8,
}

impl AudioParams {
    /// From the header byte; `None` for PS without SBR (invalid).
    pub fn from_header(h: u8) -> Option<Self> {
        let p = AudioParams {
            header: h & 0x7F,
            dac_48k: h & 0x40 != 0,
            sbr: h & 0x20 != 0,
            stereo: h & 0x10 != 0,
            ps: h & 0x08 != 0,
            surround: h & 7,
        };
        if p.ps && !p.sbr {
            None
        } else {
            Some(p)
        }
    }
    /// The output rate.
    pub fn output_rate(&self) -> u32 {
        if self.dac_48k {
            48_000
        } else {
            32_000
        }
    }
    /// The AAC core rate.
    pub fn core_rate(&self) -> u32 {
        if self.sbr {
            self.output_rate() / 2
        } else {
            self.output_rate()
        }
    }
    /// Access units per superframe.
    pub fn units(&self) -> usize {
        match (self.dac_48k, self.sbr) {
            (true, false) => 6,
            (true, true) => 3,
            (false, false) => 4,
            (false, true) => 2,
        }
    }
    /// Channels the core codes.
    pub fn core_channels(&self) -> u8 {
        if self.stereo && !self.ps {
            2
        } else {
            1
        }
    }
    /// Output channels.
    pub fn output_channels(&self) -> u8 {
        if self.stereo || self.ps {
            2
        } else {
            1
        }
    }
    /// "AAC-LC", "HE-AAC" or "HE-AAC v2".
    pub fn profile(&self) -> &'static str {
        match (self.sbr, self.ps) {
            (false, _) => "AAC-LC",
            (true, false) => "HE-AAC",
            (true, true) => "HE-AAC v2",
        }
    }
    /// The codec id in sdrkit's table.
    pub fn codec_id(&self) -> &'static str {
        match (self.sbr, self.ps) {
            (false, _) => "aac",
            (true, false) => "he_aac",
            (true, true) => "he_aac_v2",
        }
    }
    /// Output samples per access unit (960, or 1920 with SBR).
    pub fn samples_per_unit(&self) -> u32 {
        if self.sbr {
            1920
        } else {
            960
        }
    }
    /// The shared AudioSpecificConfig of these parameters (TS 102 563 clause 5.2).
    pub fn config(&self) -> AudioSpecificConfig {
        // `from_header` refuses PS without SBR, the one combination `dab_plus` refuses.
        AudioSpecificConfig::dab_plus(self.dac_48k, self.sbr, self.stereo, self.ps)
            .unwrap_or_else(|_| AudioSpecificConfig::lc(self.output_rate(), self.output_channels()))
    }

    /// The AudioSpecificConfig bytes (explicit hierarchical signalling for SBR and PS; frame length 960).
    pub fn audio_specific_config(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(5);
        // The configuration is valid by construction (rates 16 to 48 kHz, 1 or 2 channels).
        let _ = self.config().write(&mut v);
        v
    }
}

/// Wraps an access unit in an AudioMuxElement with an in-band StreamMuxConfig (LATM, ISO/IEC 14496-3 clause 1.7.3), appended to
/// `out`.
pub fn latm(params: &AudioParams, au: &[u8], out: &mut Vec<u8>) {
    // A configuration of `AudioParams` always writes.
    let _ = aac::write_latm(&params.config(), true, au, out);
}

/// The PAD of a DAB+ access unit (TS 102 563 clause 5.2: a data stream element first): the element's bytes, `None` without PAD.
pub fn pad_of_au(au: &[u8]) -> Option<&[u8]> {
    if au.len() < 3 || au[0] >> 5 != 4 {
        return None;
    }
    let mut count = au[1] as usize;
    let mut at = 2;
    if count == 255 {
        count += *au.get(2)? as usize;
        at = 3;
    }
    if count < 2 || at + count > au.len() {
        return None;
    }
    Some(&au[at..at + count])
}

/// One access unit out, or a loss.
#[derive(Clone, Debug)]
pub enum AuEvent {
    /// A good access unit (without its CRC).
    Unit {
        /// The bytes, at the signal time of the unit.
        data: Packet,
        /// Presentation time in output samples.
        pts: i64,
        /// The audio parameters of its superframe.
        params: AudioParams,
        /// Signal time.
        time: f64,
    },
    /// Access units lost (bad CRC or a bad superframe): their total duration in output samples.
    Lost {
        /// How many.
        units: u32,
        /// Output samples to conceal.
        samples: u32,
        /// Presentation time of the first.
        pts: i64,
        /// Signal time.
        time: f64,
    },
}

/// Counters of the DAB+ decoder.
#[derive(Clone, Copy, Debug, Default)]
pub struct DabPlusStats {
    /// Logical frames in.
    pub logical_frames: u64,
    /// Good superframes.
    pub superframes_ok: u64,
    /// Bad superframes (while synchronised).
    pub superframes_bad: u64,
    /// RS bytes corrected.
    pub rs_bytes_corrected: u64,
    /// RS code words beyond repair.
    pub rs_uncorrectable: u64,
    /// Good access units.
    pub access_units_ok: u64,
    /// Bad or lost access units.
    pub access_units_bad: u64,
    /// Units concealed.
    pub concealed: u64,
    /// Synchronised.
    pub synced: bool,
}

/// The superframe decoder.
pub struct SuperframeDecoder {
    frames: Vec<(Arc<[u8]>, bool)>,
    since_try: usize,
    synced: bool,
    bad_in_row: u32,
    rs: Rs,
    sf: Vec<u8>,
    word: Vec<u8>,
    erasures: Vec<u16>,
    /// The current parameters.
    pub params: Option<AudioParams>,
    pts: i64,
    units: PacketPool,
    /// Counters.
    pub stats: DabPlusStats,
}

impl Default for SuperframeDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SuperframeDecoder {
    /// A decoder.
    pub fn new() -> Self {
        SuperframeDecoder {
            frames: Vec::with_capacity(5),
            since_try: 0,
            synced: false,
            bad_in_row: 0,
            rs: Rs::new(DAB_PLUS_120_110).unwrap_or_else(|_| unreachable!("a table code")),
            sf: Vec::new(),
            word: vec![0; 120],
            erasures: Vec::with_capacity(120),
            params: None,
            pts: 0,
            units: PacketPool::default(),
            stats: DabPlusStats::default(),
        }
    }

    /// Forgets the synchronisation (a new service, a reconfiguration).
    pub fn resync(&mut self) {
        self.frames.clear();
        self.synced = false;
        self.since_try = 0;
        self.bad_in_row = 0;
        self.stats.synced = false;
    }

    /// Puts in one logical frame (an erased one enters as zero bytes, keeping the phase).
    pub fn push(&mut self, data: &Arc<[u8]>, erased: bool, time: f64, out: &mut Vec<AuEvent>) {
        self.stats.logical_frames += 1;
        if self.frames.len() == 5 {
            self.frames.remove(0);
        }
        self.frames.push((data.clone(), erased));
        self.since_try += 1;
        if self.frames.len() < 5 {
            return;
        }
        if self.synced && self.since_try < 5 {
            return;
        }
        let len = self.frames[0].0.len();
        if self.frames.iter().any(|f| f.0.len() != len) || !len.is_multiple_of(24) || len == 0 {
            self.resync();
            return;
        }
        let n = len / 24;
        self.since_try = 0;
        match self.try_superframe(n, time, out) {
            true => {
                self.synced = true;
                self.bad_in_row = 0;
                self.stats.superframes_ok += 1;
            }
            false => {
                if self.synced {
                    self.stats.superframes_bad += 1;
                    let p = self.params;
                    if let Some(p) = p {
                        let units = p.units() as u32;
                        self.stats.access_units_bad += units as u64;
                        self.stats.concealed += units as u64;
                        out.push(AuEvent::Lost {
                            units,
                            samples: units * p.samples_per_unit(),
                            pts: self.pts,
                            time,
                        });
                        self.pts += (units * p.samples_per_unit()) as i64;
                    }
                    self.bad_in_row += 1;
                    if self.bad_in_row >= 8 {
                        self.synced = false;
                        self.bad_in_row = 0;
                    }
                }
            }
        }
        self.stats.synced = self.synced;
    }

    fn try_superframe(&mut self, n: usize, time: f64, out: &mut Vec<AuEvent>) -> bool {
        // The bytes as received. Logical frame f holds rows 24 f to 24 f + 23 of every code word: when the frame is marked
        // erased, those rows are erasures.
        self.sf.clear();
        self.erasures.clear();
        for (k, (f, erased)) in self.frames.iter().enumerate() {
            self.sf.extend_from_slice(f);
            if *erased {
                self.erasures.extend((24 * k as u16)..(24 * k as u16 + 24));
            }
        }
        // With more erasures than parity bytes no word can be repaired: a word is good only when it is already a code word, and
        // no decode is tried, so the lost frame is never "corrected" into a wrong word.
        let hopeless = self.erasures.len() > 10;
        let mut corrected = 0u64;
        let mut failed = 0u64;
        for i in 0..n {
            for (j, w) in self.word.iter_mut().enumerate() {
                *w = self.sf[i + j * n];
            }
            if hopeless {
                if !self.rs.check(&self.word).unwrap_or(false) {
                    failed += 1;
                }
                continue;
            }
            match self.rs.decode(&mut self.word, &self.erasures) {
                Ok(d) => {
                    corrected += d.corrected as u64;
                    if d.corrected > 0 {
                        for (j, w) in self.word.iter().enumerate() {
                            self.sf[i + j * n] = *w;
                        }
                    }
                }
                Err(_) => failed += 1,
            }
        }
        let payload = &self.sf[..110 * n];
        // Fire code.
        if CRC16_DAB_FIRE.checksum(&payload[2..11]) != u16::from_be_bytes([payload[0], payload[1]])
        {
            return false;
        }
        let Some(params) = AudioParams::from_header(payload[2]) else {
            return false;
        };
        let units = params.units();
        let mut starts = [0usize; 7];
        starts[0] = match units {
            2 => 5,
            3 => 6,
            4 => 8,
            _ => 11,
        };
        for (k, s) in starts.iter_mut().enumerate().take(units).skip(1) {
            let bit = 24 + 12 * (k - 1);
            let b = bit / 8;
            let v = ((payload[b] as usize) << 8) | payload[b + 1] as usize;
            *s = if bit % 8 == 0 { v >> 4 } else { v & 0xFFF };
        }
        starts[units] = 110 * n;
        for k in 0..units {
            if starts[k + 1] <= starts[k] || starts[k + 1] > 110 * n {
                return false;
            }
        }
        // A good superframe.
        self.stats.rs_bytes_corrected += corrected;
        self.stats.rs_uncorrectable += failed;
        if self.params.map(|p| p.header) != Some(params.header) {
            self.params = Some(params);
        }
        let spu = params.samples_per_unit() as i64;
        for k in 0..units {
            let au = &payload[starts[k]..starts[k + 1]];
            if au.len() >= 3 && crc_ok(au) {
                self.stats.access_units_ok += 1;
                let n = au.len() - 2;
                let data = self.units.make(n, time, |b| b.copy_from_slice(&au[..n]));
                out.push(AuEvent::Unit {
                    data,
                    pts: self.pts,
                    params,
                    time,
                });
            } else {
                self.stats.access_units_bad += 1;
                self.stats.concealed += 1;
                out.push(AuEvent::Lost {
                    units: 1,
                    samples: spu as u32,
                    pts: self.pts,
                    time,
                });
            }
            self.pts += spu;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(v: &[u8]) -> String {
        v.iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn audio_specific_configs() {
        let p = |h| AudioParams::from_header(h).unwrap();
        assert_eq!(hex(&p(0x50).audio_specific_config()), "11 94");
        assert_eq!(hex(&p(0x10).audio_specific_config()), "12 94");
        assert_eq!(hex(&p(0x40).audio_specific_config()), "11 8C");
        assert_eq!(hex(&p(0x70).audio_specific_config()), "2B 11 8A 00");
        assert_eq!(hex(&p(0x30).audio_specific_config()), "2C 12 8A 00");
        assert_eq!(hex(&p(0x68).audio_specific_config()), "EB 09 8A 00");
        assert_eq!(hex(&p(0x28).audio_specific_config()), "EC 0A 8A 00");
    }

    #[test]
    fn header_table() {
        let p = AudioParams::from_header(0x50).unwrap();
        assert_eq!(
            (
                p.output_rate(),
                p.core_rate(),
                p.units(),
                p.core_channels(),
                p.profile()
            ),
            (48000, 48000, 6, 2, "AAC-LC")
        );
        let p = AudioParams::from_header(0x10).unwrap();
        assert_eq!((p.output_rate(), p.units()), (32000, 4));
        let p = AudioParams::from_header(0x70).unwrap();
        assert_eq!(
            (p.core_rate(), p.units(), p.core_channels(), p.profile()),
            (24000, 3, 2, "HE-AAC")
        );
        let p = AudioParams::from_header(0x38).unwrap();
        assert_eq!(
            (
                p.core_rate(),
                p.units(),
                p.core_channels(),
                p.output_channels(),
                p.profile()
            ),
            (16000, 2, 1, 2, "HE-AAC v2")
        );
        assert!(AudioParams::from_header(0x4D).is_none());
        assert_eq!(AudioParams::from_header(0x45).unwrap().surround, 5);
    }
}
