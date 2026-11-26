//! What an LE Audio broadcast source advertises about its stream: the BIGInfo in the ACAD of its periodic advertising
//! (Bluetooth Core Specification v5.4 Vol 6 Part B clause 4.4.6.4 and Vol 3 Part C clause 11, AD type 0x2C), the Basic
//! Audio Announcement with its BASE in the periodic AdvData (Basic Audio Profile 1.0.1 clause 3.7.2.2, service UUID
//! 0x1851), the Broadcast Audio Announcement with the Broadcast_ID in the extended AdvData (Broadcast Audio Scan Service
//! and BAP clause 3.7.2.1, service UUID 0x1852), the Public Broadcast Announcement (Public Broadcast Profile 1.0 clause
//! 4.1, service UUID 0x1856) and the Broadcast_Name (AD type 0x30). The codec configuration and the metadata are the LTV
//! structures of the Assigned Numbers, clauses 6.12.5 and 6.12.6.
//!
//! The audio itself travels on the BIS of the BIG; nothing here decodes it.

use super::ad;

/// The AD type of BIGInfo in the ACAD.
pub const BIGINFO: u8 = 0x2C;
/// The AD type of Broadcast_Name.
pub const BROADCAST_NAME: u8 = 0x30;
/// The service UUID of the Basic Audio Announcement (its service data is the BASE).
pub const BASIC_AUDIO: u16 = 0x1851;
/// The service UUID of the Broadcast Audio Announcement (its service data starts with the Broadcast_ID).
pub const BROADCAST_AUDIO: u16 = 0x1852;
/// The service UUID of the Public Broadcast Announcement.
pub const PUBLIC_BROADCAST: u16 = 0x1856;
/// The coding format of LC3 (Assigned Numbers clause 2.11).
pub const LC3: u8 = 0x06;

/// The fields of BIGInfo that tell what the stream is (Vol 6 Part B clause 4.4.6.4, figure 4.43).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BigInfo {
    /// ISO_Interval, units of 1.25 ms.
    pub iso_interval: u16,
    /// Num_BIS: the BIS of the BIG.
    pub num_bis: u8,
    /// SDU_Interval, µs.
    pub sdu_interval_us: u32,
    /// Max_SDU, bytes.
    pub max_sdu: u16,
    /// Max_PDU, bytes.
    pub max_pdu: u8,
    /// The PHY of the BIS: 0 LE 1M, 1 LE 2M, 2 LE Coded.
    pub phy: u8,
    /// Framed SDUs.
    pub framed: bool,
    /// The BIG is encrypted: the BIGInfo holds GIV and GSKD (57 bytes in place of 33).
    pub encrypted: bool,
}

fn bits(b: &[u8], at: usize, n: usize) -> u64 {
    let mut v = 0u64;
    for (i, &x) in b[at..at + n].iter().enumerate() {
        v |= u64::from(x) << (8 * i);
    }
    v
}

impl BigInfo {
    /// The BIGInfo of an AD structure's data (33 bytes, or 57 for an encrypted BIG). `None` for another length.
    pub fn parse(d: &[u8]) -> Option<BigInfo> {
        let encrypted = match d.len() {
            33 => false,
            57 => true,
            _ => return None,
        };
        let w0 = bits(d, 0, 4);
        let w2 = bits(d, 8, 4);
        let w4 = bits(d, 17, 4);
        let chm_phy = bits(d, 23, 5);
        let count = bits(d, 28, 5);
        Some(BigInfo {
            iso_interval: ((w0 >> 15) & 0xFFF) as u16,
            num_bis: ((w0 >> 27) & 0x1F) as u8,
            sdu_interval_us: (w4 & 0xF_FFFF) as u32,
            max_sdu: ((w4 >> 20) & 0xFFF) as u16,
            max_pdu: ((w2 >> 24) & 0xFF) as u8,
            phy: ((chm_phy >> 37) & 7) as u8,
            framed: (count >> 39) & 1 != 0,
            encrypted,
        })
    }

    /// The 33 bytes (57 when `encrypted`) of a BIGInfo with these fields, for test signals: the BIG offset, the timing of the
    /// subevents, the seed access address, the CRC preset, the channel map (channels 0 to 36), the payload count, GIV and GSKD
    /// are fixed values.
    pub fn build(&self) -> Vec<u8> {
        let mut d = vec![0u8; if self.encrypted { 57 } else { 33 }];
        let put = |d: &mut [u8], at: usize, n: usize, v: u64| {
            for i in 0..n {
                d[at + i] = (v >> (8 * i)) as u8;
            }
        };
        let w0 = 100 | u64::from(self.iso_interval & 0xFFF) << 15 | u64::from(self.num_bis & 0x1F) << 27;
        put(&mut d, 0, 4, w0);
        // NSE 2, BN 1, Sub_Interval 5000 µs, PTO 0.
        put(&mut d, 4, 4, 2 | 1 << 5 | 5000 << 8);
        // BIS_Spacing 5000 µs, IRC 1, Max_PDU.
        put(&mut d, 8, 4, 5000 | 1 << 20 | u64::from(self.max_pdu) << 24);
        put(&mut d, 13, 4, 0x8E89_BED6 ^ 0x0101_0101);
        put(&mut d, 17, 4, u64::from(self.sdu_interval_us & 0xF_FFFF) | u64::from(self.max_sdu & 0xFFF) << 20);
        put(&mut d, 21, 2, 0x1234);
        put(&mut d, 23, 5, 0x1F_FFFF_FFFF | u64::from(self.phy & 7) << 37);
        put(&mut d, 28, 5, u64::from(self.framed) << 39);
        for (i, b) in d.iter_mut().enumerate().skip(33) {
            *b = i as u8;
        }
        d
    }

    /// The BIGInfo in an ACAD (AD structures), if one is there.
    pub fn in_acad(acad: &[u8]) -> Option<BigInfo> {
        ad::structures(acad)
            .filter(|s| s.kind == BIGINFO)
            .find_map(|s| BigInfo::parse(s.data))
    }
}

/// The codec configuration of a subgroup or a BIS: the LTV structures of Assigned Numbers clause 6.12.5 that it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodecConfig {
    /// Sampling_Frequency, Hz.
    pub sampling_hz: Option<u32>,
    /// Frame_Duration, µs (7500 or 10000).
    pub frame_us: Option<u32>,
    /// Audio_Channel_Allocation (bit 0 front left, bit 1 front right, ...); 0 is mono without a location.
    pub allocation: Option<u32>,
    /// Octets_Per_Codec_Frame.
    pub octets: Option<u16>,
    /// Codec_Frame_Blocks_Per_SDU.
    pub blocks: Option<u8>,
}

impl CodecConfig {
    /// The configuration in the LTV structures `ltv`. Unknown types and short values are left out.
    pub fn parse(ltv: &[u8]) -> CodecConfig {
        let mut c = CodecConfig::default();
        for (t, v) in ltvs(ltv) {
            match (t, v.len()) {
                (0x01, 1) => {
                    const HZ: [u32; 13] = [
                        8000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 88200, 96000, 176_400, 192_000,
                        384_000,
                    ];
                    c.sampling_hz = HZ.get(usize::from(v[0]).wrapping_sub(1)).copied();
                }
                (0x02, 1) => {
                    c.frame_us = match v[0] {
                        0 => Some(7500),
                        1 => Some(10_000),
                        _ => None,
                    }
                }
                (0x03, 4) => c.allocation = Some(u32::from_le_bytes([v[0], v[1], v[2], v[3]])),
                (0x04, 2) => c.octets = Some(u16::from_le_bytes([v[0], v[1]])),
                (0x05, 1) => c.blocks = Some(v[0]),
                _ => {}
            }
        }
        c
    }

    /// `self` with the values of `over` where it has them (a BIS's configuration over its subgroup's).
    pub fn with(self, over: CodecConfig) -> CodecConfig {
        CodecConfig {
            sampling_hz: over.sampling_hz.or(self.sampling_hz),
            frame_us: over.frame_us.or(self.frame_us),
            allocation: over.allocation.or(self.allocation),
            octets: over.octets.or(self.octets),
            blocks: over.blocks.or(self.blocks),
        }
    }
}

/// The metadata that a subgroup or a public broadcast announcement carries (Assigned Numbers clause 6.12.6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    /// Language: the three letters of ISO 639-3.
    pub language: Option<String>,
    /// Program_Info: the program, as text.
    pub program: Option<String>,
    /// Streaming_Audio_Contexts.
    pub contexts: Option<u16>,
}

impl Metadata {
    /// The metadata in the LTV structures `ltv`.
    pub fn parse(ltv: &[u8]) -> Metadata {
        let mut m = Metadata::default();
        for (t, v) in ltvs(ltv) {
            match t {
                0x02 if v.len() == 2 => m.contexts = Some(u16::from_le_bytes([v[0], v[1]])),
                0x03 => m.program = std::str::from_utf8(v).ok().map(str::to_owned),
                0x04 if v.len() == 3 => m.language = std::str::from_utf8(v).ok().map(str::to_owned),
                _ => {}
            }
        }
        m
    }
}

/// One subgroup of a BASE.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subgroup {
    /// Coding_Format (0x06 LC3, 0xFF vendor specific).
    pub coding_format: u8,
    /// Company_ID of a vendor specific codec.
    pub company: u16,
    /// The codec configuration of the subgroup.
    pub config: CodecConfig,
    /// Its metadata.
    pub metadata: Metadata,
    /// Its BIS: (BIS_index, the configuration of the BIS over the subgroup's).
    pub bis: Vec<(u8, CodecConfig)>,
}

/// The Broadcast Audio Source Endpoint structure (BAP clause 3.7.2.2, table 3.16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    /// Presentation_Delay, µs.
    pub presentation_delay_us: u32,
    /// The subgroups.
    pub subgroups: Vec<Subgroup>,
}

/// The type and value of each LTV structure of `b` (length, type, length - 1 value bytes); a structure that runs past the
/// end ends the walk.
fn ltvs(b: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let len = usize::from(*b.get(at)?);
        if len == 0 || at + 1 + len > b.len() {
            return None;
        }
        let s = (b[at + 1], &b[at + 2..at + 1 + len]);
        at += 1 + len;
        Some(s)
    })
}

impl Base {
    /// The BASE of the service data of a Basic Audio Announcement (after its UUID). `None` when it is truncated.
    pub fn parse(d: &[u8]) -> Option<Base> {
        let mut r = Reader { b: d, at: 0 };
        let delay = r.take(3)?;
        let presentation_delay_us = u32::from(delay[0]) | u32::from(delay[1]) << 8 | u32::from(delay[2]) << 16;
        let n = r.byte()?;
        let mut subgroups = Vec::new();
        for _ in 0..n {
            let num_bis = r.byte()?;
            let codec = r.take(5)?;
            let cl = r.byte()?;
            let config = CodecConfig::parse(r.take(usize::from(cl))?);
            let ml = r.byte()?;
            let metadata = Metadata::parse(r.take(usize::from(ml))?);
            let mut bis = Vec::new();
            for _ in 0..num_bis {
                let index = r.byte()?;
                let l = r.byte()?;
                bis.push((index, config.with(CodecConfig::parse(r.take(usize::from(l))?))));
            }
            subgroups.push(Subgroup {
                coding_format: codec[0],
                company: u16::from_le_bytes([codec[1], codec[2]]),
                config,
                metadata,
                bis,
            });
        }
        Some(Base {
            presentation_delay_us,
            subgroups,
        })
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }

    fn byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
}

/// What the AdvData of a broadcast source says: the Broadcast_ID, the Broadcast_Name, the public announcement's features and
/// metadata, and the BASE.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Announcement {
    /// Broadcast_ID (24 bits).
    pub broadcast_id: Option<u32>,
    /// Broadcast_Name.
    pub name: Option<String>,
    /// Public Broadcast Announcement features: bit 0 encrypted, bit 1 standard quality, bit 2 high quality.
    pub public_features: Option<u8>,
    /// The metadata of the public announcement.
    pub public_metadata: Option<Metadata>,
    /// The BASE.
    pub base: Option<Base>,
}

impl Announcement {
    /// What the AD structures `data` say. Every part is `None` when its structure is not there.
    pub fn parse(data: &[u8]) -> Announcement {
        let mut a = Announcement::default();
        for s in ad::structures(data) {
            if s.kind == BROADCAST_NAME {
                a.name = std::str::from_utf8(s.data).ok().map(str::to_owned);
            }
            if s.kind != ad::SERVICE_DATA16 || s.data.len() < 2 {
                continue;
            }
            let uuid = u16::from_le_bytes([s.data[0], s.data[1]]);
            let d = &s.data[2..];
            match uuid {
                BROADCAST_AUDIO if d.len() >= 3 => {
                    a.broadcast_id = Some(u32::from(d[0]) | u32::from(d[1]) << 8 | u32::from(d[2]) << 16);
                }
                PUBLIC_BROADCAST if !d.is_empty() => {
                    a.public_features = Some(d[0]);
                    if let Some(&l) = d.get(1) {
                        a.public_metadata = d.get(2..2 + usize::from(l)).map(Metadata::parse);
                    }
                }
                BASIC_AUDIO => a.base = Base::parse(d),
                _ => {}
            }
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biginfo_and_base_read_back() {
        for encrypted in [false, true] {
            let b = BigInfo {
                iso_interval: 8,
                num_bis: 2,
                sdu_interval_us: 10_000,
                max_sdu: 100,
                max_pdu: 100,
                phy: 1,
                framed: false,
                encrypted,
            };
            let mut acad = vec![b.build().len() as u8 + 1, BIGINFO];
            acad.extend(b.build());
            assert_eq!(BigInfo::in_acad(&acad), Some(b));
        }
        // 40 ms delay, one LC3 subgroup at 48 kHz, 10 ms, 100 octets, English, two BIS (left, right).
        let base = [
            0x40, 0x9C, 0x00, 1, 2, 0x06, 0, 0, 0, 0, 10, 2, 0x01, 0x08, 2, 0x02, 0x01, 3, 0x04, 100, 0, 5, 4, 0x04, b'e',
            b'n', b'g', 1, 6, 5, 0x03, 1, 0, 0, 0, 2, 6, 5, 0x03, 2, 0, 0, 0,
        ];
        let b = Base::parse(&base).unwrap();
        assert_eq!(b.presentation_delay_us, 40_000);
        let g = &b.subgroups[0];
        assert_eq!((g.coding_format, g.config.sampling_hz, g.config.frame_us), (LC3, Some(48_000), Some(10_000)));
        assert_eq!(g.config.octets, Some(100));
        assert_eq!(g.metadata.language.as_deref(), Some("eng"));
        assert_eq!(g.bis.iter().map(|b| (b.0, b.1.allocation)).collect::<Vec<_>>(), [(1, Some(1)), (2, Some(2))]);
        assert_eq!(g.bis[0].1.octets, Some(100));
    }
}
