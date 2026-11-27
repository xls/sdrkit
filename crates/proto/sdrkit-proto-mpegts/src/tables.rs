//! PSI and SI sections: the section header and CRC rules, the descriptors, and parsers for the PAT, CAT,
//! PMT, NIT, SDT, BAT, EIT, TDT, TOT and the ATSC virtual channel table into plain owned values.

use crate::error::{Error, SectionFault};
use crate::time::{bcd_duration, mjd_bcd_to_unix};
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;
use sdrkit_util_text::dvb::{Options as TextOptions, Usage as TextUse};

/// The fixed part of a section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionHeader {
    /// table_id.
    pub table_id: u8,
    /// section_syntax_indicator.
    pub syntax: bool,
    /// Bytes in the whole section (3 + section_length).
    pub total_len: usize,
    /// table_id_extension (0 for short sections).
    pub ext: u16,
    /// version_number.
    pub version: u8,
    /// current_next_indicator.
    pub current: bool,
    /// section_number.
    pub number: u8,
    /// last_section_number.
    pub last: u8,
}

/// The largest section of PSI tables (PAT, CAT, PMT, TSDT): 1024 bytes; everything else 4096 (ISO/IEC 13818-1 2.4.4).
pub fn max_len(table_id: u8) -> usize {
    if table_id <= 0x03 {
        1024
    } else {
        4096
    }
}

/// Whether a table carries a CRC (decided by table_id, never by the syntax bit; the TOT has one though its syntax bit is 0).
pub fn has_crc(table_id: u8) -> bool {
    !matches!(table_id, 0x70 | 0x71 | 0x72 | 0x7E)
}

/// Reads the header of `s` (whole or partial: only the first 8 bytes are needed for long sections).
pub fn header(s: &[u8]) -> Option<SectionHeader> {
    if s.len() < 3 {
        return None;
    }
    let total_len = 3 + (((s[1] & 0x0F) as usize) << 8 | s[2] as usize);
    let syntax = s[1] & 0x80 != 0;
    let mut h = SectionHeader {
        table_id: s[0],
        syntax,
        total_len,
        ext: 0,
        version: 0,
        current: true,
        number: 0,
        last: 0,
    };
    if is_long(s[0]) && s.len() >= 8 {
        h.ext = u16::from_be_bytes([s[3], s[4]]);
        h.version = (s[5] >> 1) & 0x1F;
        h.current = s[5] & 1 != 0;
        h.number = s[6];
        h.last = s[7];
    }
    Some(h)
}

/// Tables that have the long header (table_id_extension, version, section numbers). Decided by table_id.
pub fn is_long(table_id: u8) -> bool {
    !matches!(table_id, 0x70..=0x73 | 0x7E)
}

/// Checks a complete section: length, minimum size and CRC. Returns its header.
pub fn check(s: &[u8]) -> Result<SectionHeader, Error> {
    let h = header(s).ok_or(Error::Section(SectionFault::Short))?;
    if h.total_len > s.len() || h.total_len > max_len(h.table_id) {
        return Err(SectionFault::Length.into());
    }
    let min = match h.table_id {
        0x70 => 8,
        0x73 => 14,
        0x71 | 0x72 | 0x7E => 3,
        _ => 12,
    };
    if h.total_len < min {
        return Err(SectionFault::Short.into());
    }
    if has_crc(h.table_id) && CRC32_MPEG2.checksum(&s[..h.total_len]) != 0 {
        return Err(SectionFault::Crc.into());
    }
    Ok(h)
}

/// The body of a long section: after the 8-byte header, before the CRC.
fn body<'a>(s: &'a [u8], h: &SectionHeader) -> &'a [u8] {
    if h.total_len >= 12 && h.total_len <= s.len() {
        &s[8..h.total_len - 4]
    } else {
        &[]
    }
}

/// Iterates over a descriptor loop: (tag, payload). Stops at a descriptor that runs past the loop; `damaged()` says so.
pub struct Descriptors<'a> {
    rest: &'a [u8],
    damaged: bool,
}

impl<'a> Descriptors<'a> {
    /// The descriptors in `b`.
    pub fn new(b: &'a [u8]) -> Self {
        Descriptors {
            rest: b,
            damaged: false,
        }
    }
    /// Whether the loop ended in a truncated descriptor.
    pub fn damaged(&self) -> bool {
        self.damaged
    }
}

impl<'a> Iterator for Descriptors<'a> {
    type Item = (u8, &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.len() < 2 {
            if !self.rest.is_empty() {
                self.damaged = true;
                self.rest = &[];
            }
            return None;
        }
        let tag = self.rest[0];
        let len = self.rest[1] as usize;
        if 2 + len > self.rest.len() {
            self.damaged = true;
            self.rest = &[];
            return None;
        }
        let d = &self.rest[2..2 + len];
        self.rest = &self.rest[2 + len..];
        Some((tag, d))
    }
}

/// A 12-bit length-prefixed loop at `b[at..]`: (loop, rest after it). `None` when it runs past the data.
fn loop12(b: &[u8], at: usize) -> Option<(&[u8], usize)> {
    if at + 2 > b.len() {
        return None;
    }
    let len = ((b[at] as usize & 0x0F) << 8) | b[at + 1] as usize;
    let end = at + 2 + len;
    (end <= b.len()).then(|| (&b[at + 2..end], end))
}

fn lang(b: &[u8]) -> String {
    b.iter()
        .take(3)
        .map(|&c| {
            if c.is_ascii_graphic() {
                (c as char).to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn text(b: &[u8], u: TextUse, o: &TextOptions) -> String {
    crate::si::dvb_text(b, u, o)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Elementary stream descriptors.

/// A teletext descriptor entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeletextEntry {
    /// ISO 639-2 language.
    pub lang: String,
    /// teletext_type: 1 initial page, 2 subtitle, 3 additional information, 4 programme schedule, 5 subtitle for the hearing impaired.
    pub kind: u8,
    /// Magazine 1 to 8.
    pub magazine: u8,
    /// Page number 100 to 899.
    pub page: u16,
}

/// A subtitling descriptor entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleEntry {
    /// ISO 639-2 language.
    pub lang: String,
    /// subtitling_type (0x10 to 0x14 normal, 0x20 to 0x24 hard of hearing, 0x01 to 0x03 EBU teletext).
    pub kind: u8,
    /// composition_page_id.
    pub composition_page: u16,
    /// ancillary_page_id.
    pub ancillary_page: u16,
}

/// AC-3 or E-AC-3 descriptor fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ac3Descriptor {
    /// component_type, when present.
    pub component_type: Option<u8>,
    /// bsid, when present.
    pub bsid: Option<u8>,
    /// mainid, when present.
    pub mainid: Option<u8>,
    /// asvc, when present.
    pub asvc: Option<u8>,
}

/// Supplementary audio (extension descriptor 0x06).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SupplementaryAudio {
    /// mix_type: 1 complete main, 0 supplementary.
    pub mix_type: u8,
    /// editorial_classification (1 audio description for the visually impaired ...).
    pub editorial: u8,
    /// Language, when present.
    pub lang: Option<String>,
}

/// The descriptors of an elementary stream (or a programme) the demultiplexer interprets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EsDescriptors {
    /// Registration format_identifier.
    pub registration: Option<[u8; 4]>,
    /// CA descriptors: (CA_system_id, ECM or EMM PID).
    pub ca: Vec<(u16, u16)>,
    /// ISO 639 entries: (language, audio_type).
    pub languages: Vec<(String, u8)>,
    /// Maximum bitrate in bit/s.
    pub max_bitrate: Option<u32>,
    /// A smoothing buffer descriptor was present.
    pub smoothing_buffer: bool,
    /// AVC: (profile_idc, constraint flags, level_idc).
    pub avc: Option<(u8, u8, u8)>,
    /// HEVC: (profile_idc, tier, level_idc).
    pub hevc: Option<(u8, u8, u8)>,
    /// MPEG-2 AAC descriptor present.
    pub mpeg2_aac: bool,
    /// DVB AAC: (profile_and_level, AAC_type).
    pub dvb_aac: Option<(u8, Option<u8>)>,
    /// AC-3.
    pub ac3: Option<Ac3Descriptor>,
    /// Enhanced AC-3.
    pub eac3: Option<Ac3Descriptor>,
    /// DTS.
    pub dts: bool,
    /// AC-4 (extension 0x15).
    pub ac4: bool,
    /// Supplementary audio (extension 0x06).
    pub supplementary: Option<SupplementaryAudio>,
    /// Teletext entries (0x56; VBI teletext 0x46 likewise).
    pub teletext: Vec<TeletextEntry>,
    /// A teletext descriptor (or VBI teletext) was present, even with no usable entry.
    pub has_teletext: bool,
    /// Subtitling entries.
    pub subtitles: Vec<SubtitleEntry>,
    /// A subtitling descriptor was present.
    pub has_subtitles: bool,
    /// component_tag of the stream identifier descriptor.
    pub component_tag: Option<u8>,
    /// data_broadcast_id.
    pub data_broadcast_id: Option<u16>,
    /// The last private_data_specifier seen.
    pub private_data_specifier: Option<u32>,
    /// The payload of an IOD_descriptor (MPEG-4 initial object descriptor, ISO/IEC 13818-1 2.6.40). Read it with
    /// [`crate::mpeg4::IodDescriptor::parse`].
    pub iod: Option<Vec<u8>>,
    /// The ES_ID of an SL_descriptor (ISO/IEC 13818-1 2.6.42).
    pub sl_es_id: Option<u16>,
    /// The (ES_ID, FlexMuxChannel) entries of an FMC_descriptor (ISO/IEC 13818-1 2.6.44).
    pub fmc: Vec<(u16, u8)>,
}

fn ac3_desc(d: &[u8], eac3: bool) -> Ac3Descriptor {
    let mut a = Ac3Descriptor::default();
    if d.is_empty() {
        return a;
    }
    let f = d[0];
    let mut i = 1;
    let mut take = |present: bool| -> Option<u8> {
        if present && i < d.len() {
            i += 1;
            Some(d[i - 1])
        } else {
            None
        }
    };
    a.component_type = take(f & 0x80 != 0);
    a.bsid = take(f & 0x40 != 0);
    a.mainid = take(f & 0x20 != 0);
    a.asvc = take(f & 0x10 != 0);
    let _ = eac3;
    a
}

/// Reads an elementary stream's (or a programme's) descriptor loop.
pub fn es_descriptors(b: &[u8]) -> EsDescriptors {
    let mut e = EsDescriptors::default();
    for (tag, d) in Descriptors::new(b) {
        match tag {
            0x05 if d.len() >= 4 => e.registration = Some([d[0], d[1], d[2], d[3]]),
            0x09 if d.len() >= 4 => e.ca.push((
                u16::from_be_bytes([d[0], d[1]]),
                u16::from_be_bytes([d[2], d[3]]) & 0x1FFF,
            )),
            0x0A => {
                for c in d.as_chunks::<4>().0 {
                    e.languages.push((lang(&c[..3]), c[3]));
                }
            }
            0x1D => e.iod = Some(d.to_vec()),
            0x1E => e.sl_es_id = crate::mpeg4::sl_descriptor(d),
            0x1F => e.fmc.extend(crate::mpeg4::fmc_descriptor(d)),
            0x0E if d.len() >= 3 => {
                e.max_bitrate =
                    Some(((d[0] as u32 & 0x3F) << 16 | (d[1] as u32) << 8 | d[2] as u32) * 400)
            }
            0x10 => e.smoothing_buffer = true,
            0x28 if d.len() >= 3 => e.avc = Some((d[0], d[1], d[2])),
            0x2B => e.mpeg2_aac = true,
            0x38 if d.len() >= 13 => e.hevc = Some((d[0] & 0x1F, (d[0] >> 5) & 1, d[11])),
            0x46 | 0x56 => {
                e.has_teletext = true;
                for c in d.as_chunks::<5>().0 {
                    let mag = c[3] & 7;
                    let (tens, units) = (c[4] >> 4, c[4] & 0x0F);
                    if tens > 9 || units > 9 {
                        continue;
                    }
                    let magazine = if mag == 0 { 8 } else { mag };
                    e.teletext.push(TeletextEntry {
                        lang: lang(&c[..3]),
                        kind: c[3] >> 3,
                        magazine,
                        page: magazine as u16 * 100 + tens as u16 * 10 + units as u16,
                    });
                }
            }
            0x52 if !d.is_empty() => e.component_tag = Some(d[0]),
            0x59 => {
                e.has_subtitles = true;
                for c in d.as_chunks::<8>().0 {
                    e.subtitles.push(SubtitleEntry {
                        lang: lang(&c[..3]),
                        kind: c[3],
                        composition_page: u16::from_be_bytes([c[4], c[5]]),
                        ancillary_page: u16::from_be_bytes([c[6], c[7]]),
                    });
                }
            }
            0x5F if d.len() >= 4 => {
                e.private_data_specifier = Some(u32::from_be_bytes([d[0], d[1], d[2], d[3]]))
            }
            0x66 if d.len() >= 2 => e.data_broadcast_id = Some(u16::from_be_bytes([d[0], d[1]])),
            0x6A => e.ac3 = Some(ac3_desc(d, false)),
            0x7A => e.eac3 = Some(ac3_desc(d, true)),
            0x7B => e.dts = true,
            0x7C if !d.is_empty() => {
                let t = (d.len() >= 3 && d[1] & 0x80 != 0).then(|| d[2]);
                e.dvb_aac = Some((d[0], t));
            }
            0x7F if !d.is_empty() => match d[0] {
                0x15 => e.ac4 = true,
                0x06 if d.len() >= 2 => {
                    let f = d[1];
                    let lang_present = f & 1 != 0;
                    e.supplementary = Some(SupplementaryAudio {
                        mix_type: f >> 7,
                        editorial: (f >> 2) & 0x1F,
                        lang: (lang_present && d.len() >= 5).then(|| lang(&d[2..5])),
                    });
                }
                _ => {}
            },
            _ => {}
        }
    }
    e
}

// ---------------------------------------------------------------------------------------------------------------------------------
// PAT, CAT, PMT.

/// A programme association section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pat {
    /// transport_stream_id.
    pub tsid: u16,
    /// (program_number, PID); program 0 is the NIT PID.
    pub programs: Vec<(u16, u16)>,
}

/// Parses a checked PAT section.
pub fn pat(s: &[u8], h: &SectionHeader) -> Pat {
    let programs = body(s, h)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| {
            (
                u16::from_be_bytes([c[0], c[1]]),
                u16::from_be_bytes([c[2], c[3]]) & 0x1FFF,
            )
        })
        .collect();
    Pat {
        tsid: h.ext,
        programs,
    }
}

/// Parses a checked CAT section: (CA_system_id, EMM PID), at most 256.
pub fn cat(s: &[u8], h: &SectionHeader) -> Vec<(u16, u16)> {
    let mut e = es_descriptors(body(s, h)).ca;
    e.truncate(256);
    e
}

/// One elementary stream of a PMT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PmtStream {
    /// stream_type.
    pub stream_type: u8,
    /// elementary_PID.
    pub pid: u16,
    /// Its descriptors.
    pub desc: EsDescriptors,
}

/// A programme map section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pmt {
    /// program_number.
    pub program: u16,
    /// version_number.
    pub version: u8,
    /// PCR_PID.
    pub pcr_pid: u16,
    /// Programme-level descriptors.
    pub program_desc: EsDescriptors,
    /// The streams in loop order.
    pub streams: Vec<PmtStream>,
}

/// Parses a checked PMT section. `None` when its loops run past the section.
pub fn pmt(s: &[u8], h: &SectionHeader) -> Option<Pmt> {
    let b = body(s, h);
    if b.len() < 4 {
        return None;
    }
    let pcr_pid = u16::from_be_bytes([b[0], b[1]]) & 0x1FFF;
    let (pi, mut at) = loop12(b, 2)?;
    let program_desc = es_descriptors(pi);
    let mut streams = Vec::new();
    while at + 5 <= b.len() {
        let stream_type = b[at];
        let pid = u16::from_be_bytes([b[at + 1], b[at + 2]]) & 0x1FFF;
        let (d, next) = loop12(b, at + 3)?;
        streams.push(PmtStream {
            stream_type,
            pid,
            desc: es_descriptors(d),
        });
        at = next;
    }
    Some(Pmt {
        program: h.ext,
        version: h.version,
        pcr_pid,
        program_desc,
        streams,
    })
}

// ---------------------------------------------------------------------------------------------------------------------------------
// NIT and BAT.

/// A delivery system descriptor.
#[derive(Clone, Debug, PartialEq)]
pub enum Delivery {
    /// Terrestrial (0x5A).
    Terrestrial {
        /// Centre frequency in Hz.
        frequency_hz: u64,
        /// Bandwidth in MHz (8, 7, 6, 5).
        bandwidth_mhz: u8,
        /// HP stream (true) or LP.
        priority: bool,
        /// Time slicing in use.
        time_slicing: bool,
        /// MPE-FEC in use.
        mpe_fec: bool,
        /// "QPSK", "16-QAM", "64-QAM".
        constellation: String,
        /// "none", "1", "2", "4" (α).
        hierarchy: String,
        /// HP code rate ("2/3").
        code_rate_hp: String,
        /// LP code rate.
        code_rate_lp: String,
        /// Guard interval ("1/8").
        guard_interval: String,
        /// "2K", "8K", "4K".
        mode: String,
        /// other_frequency_flag.
        other_frequency: bool,
    },
    /// Cable (0x44).
    Cable {
        /// Frequency in Hz.
        frequency_hz: u64,
        /// Symbol rate in symbols per second.
        symbol_rate: u32,
        /// Modulation code (1 16-QAM ... 5 256-QAM).
        modulation: u8,
        /// FEC inner code.
        fec_inner: u8,
    },
    /// Satellite (0x43).
    Satellite {
        /// Frequency in Hz.
        frequency_hz: u64,
        /// Orbital position in 0.1 degrees.
        orbital_position: u16,
        /// East (true) or west.
        east: bool,
        /// Polarisation (0 H, 1 V, 2 L, 3 R).
        polarization: u8,
        /// DVB-S2.
        s2: bool,
        /// Symbol rate in symbols per second.
        symbol_rate: u32,
        /// FEC inner code.
        fec_inner: u8,
    },
    /// DVB-T2 (extension 0x04).
    T2 {
        /// plp_id.
        plp_id: u8,
        /// T2_system_id.
        system_id: u16,
    },
    /// DVB-S2X (extension 0x17).
    S2x,
}

fn bcd_digits(b: &[u8]) -> u64 {
    let mut v = 0u64;
    for &x in b {
        v = v * 100 + ((x >> 4) as u64 % 10) * 10 + (x & 0x0F) as u64 % 10;
    }
    v
}

fn terrestrial(d: &[u8]) -> Option<Delivery> {
    if d.len() < 7 {
        return None;
    }
    let f = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) as u64 * 10;
    let bw = match d[4] >> 5 {
        0 => 8,
        1 => 7,
        2 => 6,
        3 => 5,
        _ => 0,
    };
    let rate = |v: u8| {
        ["1/2", "2/3", "3/4", "5/6", "7/8"]
            .get(v as usize)
            .copied()
            .unwrap_or("reserved")
            .to_string()
    };
    Some(Delivery::Terrestrial {
        frequency_hz: f,
        bandwidth_mhz: bw,
        priority: d[4] & 0x10 != 0,
        time_slicing: d[4] & 0x08 == 0,
        mpe_fec: d[4] & 0x04 == 0,
        constellation: ["QPSK", "16-QAM", "64-QAM", "reserved"][(d[5] >> 6) as usize].into(),
        hierarchy: ["none", "1", "2", "4", "none", "1", "2", "4"][((d[5] >> 3) & 7) as usize]
            .into(),
        code_rate_hp: rate(d[5] & 7),
        code_rate_lp: rate(d[6] >> 5),
        guard_interval: ["1/32", "1/16", "1/8", "1/4"][((d[6] >> 3) & 3) as usize].into(),
        mode: ["2K", "8K", "4K", "reserved"][((d[6] >> 1) & 3) as usize].into(),
        other_frequency: d[6] & 1 != 0,
    })
}

/// A logical channel number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lcn {
    /// service_id.
    pub service_id: u16,
    /// visible_service_flag.
    pub visible: bool,
    /// The number.
    pub lcn: u16,
    /// From an HD simulcast descriptor (0x88): replaces the plain one.
    pub hd: bool,
    /// For NorDig v2 lists: the list's country code (upper case); empty otherwise.
    pub country: String,
}

/// One transport stream of a NIT or BAT.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TsEntry {
    /// transport_stream_id.
    pub tsid: u16,
    /// original_network_id.
    pub onid: u16,
    /// Service list: (service_id, service_type).
    pub services: Vec<(u16, u8)>,
    /// Logical channel numbers.
    pub lcns: Vec<Lcn>,
    /// Delivery systems.
    pub delivery: Vec<Delivery>,
    /// Frequency list (Hz).
    pub frequencies: Vec<u64>,
}

/// A network information (or bouquet association) section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nit {
    /// network_id (bouquet_id for a BAT).
    pub id: u16,
    /// NIT actual (0x40) or BAT; false for NIT other.
    pub actual: bool,
    /// Network (or bouquet) name.
    pub name: Option<String>,
    /// Multilingual names: (language, name).
    pub names: Vec<(String, String)>,
    /// Transport streams.
    pub streams: Vec<TsEntry>,
}

/// Reads LCN descriptors under the private data specifier in force.
fn lcn_descriptor(tag: u8, d: &[u8], pds: Option<u32>, out: &mut Vec<Lcn>) {
    let eacem_like = |pds: Option<u32>| {
        matches!(
            pds,
            None | Some(0x28) | Some(0x29) | Some(0x233A) | Some(0x3200)
        )
    };
    match tag {
        0x83 if eacem_like(pds) => {
            for c in d.as_chunks::<4>().0 {
                out.push(Lcn {
                    service_id: u16::from_be_bytes([c[0], c[1]]),
                    visible: c[2] & 0x80 != 0,
                    lcn: u16::from_be_bytes([c[2], c[3]]) & 0x03FF,
                    hd: false,
                    country: String::new(),
                });
            }
        }
        0x88 if matches!(pds, Some(0x28) | None) => {
            for c in d.as_chunks::<4>().0 {
                out.push(Lcn {
                    service_id: u16::from_be_bytes([c[0], c[1]]),
                    visible: c[2] & 0x80 != 0,
                    lcn: u16::from_be_bytes([c[2], c[3]]) & 0x03FF,
                    hd: true,
                    country: String::new(),
                });
            }
        }
        0x87 if pds == Some(0x29) => {
            let mut i = 0;
            while i + 2 <= d.len() {
                let name_len = d[i + 1] as usize;
                let at = i + 2 + name_len;
                if at + 4 > d.len() {
                    break;
                }
                let country: String = d[at..at + 3]
                    .iter()
                    .map(|&c| (c as char).to_ascii_uppercase())
                    .collect();
                let n = d[at + 3] as usize;
                let mut j = at + 4;
                for _ in 0..n {
                    if j + 4 > d.len() {
                        break;
                    }
                    out.push(Lcn {
                        service_id: u16::from_be_bytes([d[j], d[j + 1]]),
                        visible: d[j + 2] & 0x80 != 0,
                        lcn: u16::from_be_bytes([d[j + 2], d[j + 3]]) & 0x03FF,
                        hd: false,
                        country: country.clone(),
                    });
                    j += 4;
                }
                i = j;
            }
        }
        _ => {}
    }
}

/// Parses a checked NIT (0x40, 0x41) or BAT (0x4A) section.
pub fn nit(s: &[u8], h: &SectionHeader, o: &TextOptions) -> Option<Nit> {
    let b = body(s, h);
    let mut n = Nit {
        id: h.ext,
        actual: h.table_id != 0x41,
        ..Default::default()
    };
    let (net, at) = loop12(b, 0)?;
    for (tag, d) in Descriptors::new(net) {
        match tag {
            0x40 | 0x47 => n.name = Some(text(d, TextUse::Name, o)),
            0x5B | 0x5C => {
                let mut i = 0;
                while i + 4 <= d.len() {
                    let l = d[i + 3] as usize;
                    if i + 4 + l > d.len() {
                        break;
                    }
                    n.names.push((
                        lang(&d[i..i + 3]),
                        text(&d[i + 4..i + 4 + l], TextUse::Name, o),
                    ));
                    i += 4 + l;
                }
            }
            _ => {}
        }
    }
    let (tsl, _) = loop12(b, at)?;
    let mut i = 0;
    while i + 6 <= tsl.len() {
        let tsid = u16::from_be_bytes([tsl[i], tsl[i + 1]]);
        let onid = u16::from_be_bytes([tsl[i + 2], tsl[i + 3]]);
        let (d, next) = loop12(tsl, i + 4)?;
        let mut e = TsEntry {
            tsid,
            onid,
            ..Default::default()
        };
        let mut pds = None;
        for (tag, x) in Descriptors::new(d) {
            match tag {
                0x41 => e.services.extend(
                    x.as_chunks::<3>()
                        .0
                        .iter()
                        .map(|c| (u16::from_be_bytes([c[0], c[1]]), c[2])),
                ),
                0x5A => e.delivery.extend(terrestrial(x)),
                0x44 if x.len() >= 11 => e.delivery.push(Delivery::Cable {
                    frequency_hz: bcd_digits(&x[0..4]) * 100,
                    modulation: x[6],
                    symbol_rate: (bcd_digits(&x[7..11]) / 10 * 100) as u32,
                    fec_inner: x[10] & 0x0F,
                }),
                0x43 if x.len() >= 11 => e.delivery.push(Delivery::Satellite {
                    frequency_hz: bcd_digits(&x[0..4]) * 10_000,
                    orbital_position: bcd_digits(&x[4..6]) as u16,
                    east: x[6] & 0x80 != 0,
                    polarization: (x[6] >> 5) & 3,
                    s2: x[6] & 0x04 != 0,
                    symbol_rate: (bcd_digits(&x[7..11]) / 10 * 100) as u32,
                    fec_inner: x[10] & 0x0F,
                }),
                0x7F if !x.is_empty() => match x[0] {
                    0x04 if x.len() >= 4 => e.delivery.push(Delivery::T2 {
                        plp_id: x[1],
                        system_id: u16::from_be_bytes([x[2], x[3]]),
                    }),
                    0x17 => e.delivery.push(Delivery::S2x),
                    _ => {}
                },
                0x62 if !x.is_empty() => {
                    let kind = x[0] & 3;
                    for c in x[1..].as_chunks::<4>().0 {
                        e.frequencies.push(match kind {
                            3 => u32::from_be_bytes([c[0], c[1], c[2], c[3]]) as u64 * 10,
                            1 => bcd_digits(c) * 10_000,
                            _ => bcd_digits(c) * 100,
                        });
                    }
                }
                0x5F if x.len() >= 4 => pds = Some(u32::from_be_bytes([x[0], x[1], x[2], x[3]])),
                0x83 | 0x87 | 0x88 => lcn_descriptor(tag, x, pds, &mut e.lcns),
                _ => {}
            }
        }
        if n.streams.len() < 4096 {
            n.streams.push(e);
        }
        i = next;
    }
    Some(n)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// SDT.

/// One service of an SDT.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdtService {
    /// service_id.
    pub service_id: u16,
    /// EIT_schedule_flag.
    pub eit_schedule: bool,
    /// EIT_present_following_flag.
    pub eit_pf: bool,
    /// running_status.
    pub running: u8,
    /// free_CA_mode.
    pub free_ca: bool,
    /// service_type (0 unknown).
    pub service_type: u8,
    /// Provider name.
    pub provider: String,
    /// Service name.
    pub name: String,
    /// Multilingual names: (language, provider, name).
    pub names: Vec<(String, String, String)>,
    /// Service replacement (linkage 0x05): (onid, tsid, sid).
    pub replacement: Option<(u16, u16, u16)>,
    /// CA identifiers.
    pub ca_ids: Vec<u16>,
}

/// A service description section.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sdt {
    /// transport_stream_id.
    pub tsid: u16,
    /// original_network_id.
    pub onid: u16,
    /// SDT actual (0x42) or other (0x46).
    pub actual: bool,
    /// The services.
    pub services: Vec<SdtService>,
}

/// Parses a checked SDT section.
pub fn sdt(s: &[u8], h: &SectionHeader, o: &TextOptions) -> Option<Sdt> {
    let b = body(s, h);
    if b.len() < 3 {
        return None;
    }
    let mut out = Sdt {
        tsid: h.ext,
        onid: u16::from_be_bytes([b[0], b[1]]),
        actual: h.table_id == 0x42,
        services: Vec::new(),
    };
    let mut i = 3;
    while i + 5 <= b.len() {
        let mut sv = SdtService {
            service_id: u16::from_be_bytes([b[i], b[i + 1]]),
            eit_schedule: b[i + 2] & 0x02 != 0,
            eit_pf: b[i + 2] & 0x01 != 0,
            running: b[i + 3] >> 5,
            free_ca: b[i + 3] & 0x10 != 0,
            ..Default::default()
        };
        let (d, next) = loop12(b, i + 3)?;
        for (tag, x) in Descriptors::new(d) {
            match tag {
                0x48 if x.len() >= 2 => {
                    sv.service_type = x[0];
                    let pl = x[1] as usize;
                    if 2 + pl < x.len() {
                        sv.provider = text(&x[2..2 + pl], TextUse::Name, o);
                        let nl = x[2 + pl] as usize;
                        let end = (3 + pl + nl).min(x.len());
                        sv.name = text(&x[3 + pl..end], TextUse::Name, o);
                    } else {
                        sv.provider = text(&x[2..], TextUse::Name, o);
                    }
                }
                0x5D => {
                    let mut k = 0;
                    while k + 4 <= x.len() {
                        let l = lang(&x[k..k + 3]);
                        let pl = x[k + 3] as usize;
                        if k + 4 + pl >= x.len() {
                            break;
                        }
                        let p = text(&x[k + 4..k + 4 + pl], TextUse::Name, o);
                        let nl = x[k + 4 + pl] as usize;
                        let st = k + 5 + pl;
                        if st + nl > x.len() {
                            break;
                        }
                        sv.names
                            .push((l, p, text(&x[st..st + nl], TextUse::Name, o)));
                        k = st + nl;
                    }
                }
                0x4A if x.len() >= 7 && x[6] == 0x05 => {
                    sv.replacement = Some((
                        u16::from_be_bytes([x[2], x[3]]),
                        u16::from_be_bytes([x[0], x[1]]),
                        u16::from_be_bytes([x[4], x[5]]),
                    ))
                }
                0x53 => sv.ca_ids.extend(
                    x.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|c| u16::from_be_bytes([c[0], c[1]])),
                ),
                _ => {}
            }
        }
        out.services.push(sv);
        i = next;
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// EIT.

/// A piece of an extended event descriptor, kept as bytes until all pieces are joined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtendedPiece {
    /// Language.
    pub lang: String,
    /// descriptor_number.
    pub number: u8,
    /// last_descriptor_number.
    pub last: u8,
    /// Items: (description bytes, item bytes).
    pub items: Vec<(Vec<u8>, Vec<u8>)>,
    /// Text bytes.
    pub text: Vec<u8>,
}

/// An event as carried in an EIT section, before it is merged into the guide.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawEvent {
    /// event_id.
    pub event_id: u16,
    /// Start (Unix seconds); `None` when undefined.
    pub start: Option<i64>,
    /// Duration in seconds.
    pub duration: i64,
    /// running_status.
    pub running: u8,
    /// free_CA_mode.
    pub free_ca: bool,
    /// Short events: (language, name bytes, text bytes).
    pub short: Vec<(String, Vec<u8>, Vec<u8>)>,
    /// Extended event pieces.
    pub extended: Vec<ExtendedPiece>,
    /// Content bytes (level 1 and 2 nibbles), up to 8.
    pub content: Vec<u8>,
    /// Parental ratings: (country, rating).
    pub ratings: Vec<(String, u8)>,
    /// Component descriptors: (stream_content, component_type, language).
    pub components: Vec<(u8, u8, String)>,
    /// CRIDs: (type, text).
    pub crids: Vec<(u8, String)>,
    /// Time-shifted event: the (service_id, event_id) that it repeats.
    pub time_shifted: Option<(u16, u16)>,
    /// UK guidance text (descriptor 0x89 under DTG).
    pub guidance: Option<String>,
}

/// An event information section.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Eit {
    /// table_id.
    pub table_id: u8,
    /// service_id.
    pub service_id: u16,
    /// transport_stream_id.
    pub tsid: u16,
    /// original_network_id.
    pub onid: u16,
    /// section_number.
    pub section: u8,
    /// segment_last_section_number.
    pub segment_last: u8,
    /// last_table_id.
    pub last_table_id: u8,
    /// Events before any damage.
    pub events: Vec<RawEvent>,
    /// The event loop ran past the section.
    pub damaged: bool,
}

/// Parses a checked EIT section (keeping the events before a damaged one).
pub fn eit(s: &[u8], h: &SectionHeader, o: &TextOptions) -> Option<Eit> {
    let b = body(s, h);
    if b.len() < 6 {
        return None;
    }
    let mut e = Eit {
        table_id: h.table_id,
        service_id: h.ext,
        tsid: u16::from_be_bytes([b[0], b[1]]),
        onid: u16::from_be_bytes([b[2], b[3]]),
        section: h.number,
        segment_last: b[4],
        last_table_id: b[5],
        ..Default::default()
    };
    let mut i = 6;
    while i < b.len() {
        if i + 12 > b.len() {
            e.damaged = true;
            break;
        }
        let x = &b[i..];
        let mut ev = RawEvent {
            event_id: u16::from_be_bytes([x[0], x[1]]),
            start: mjd_bcd_to_unix(&x[2..7]),
            duration: bcd_duration(&x[7..10]).unwrap_or(0),
            running: x[10] >> 5,
            free_ca: x[10] & 0x10 != 0,
            ..Default::default()
        };
        let Some((d, next)) = loop12(b, i + 10) else {
            e.damaged = true;
            break;
        };
        let mut pds: Option<u32> = None;
        let mut descs = Descriptors::new(d);
        for (tag, y) in descs.by_ref() {
            match tag {
                0x4D if y.len() >= 4 => {
                    let nl = y[3] as usize;
                    if 4 + nl < y.len() {
                        let tl = y[4 + nl] as usize;
                        let te = (5 + nl + tl).min(y.len());
                        ev.short.push((
                            lang(&y[..3]),
                            y[4..4 + nl].to_vec(),
                            y[5 + nl..te].to_vec(),
                        ));
                    } else {
                        ev.short.push((lang(&y[..3]), y[4..].to_vec(), Vec::new()));
                    }
                }
                0x4E if y.len() >= 5 => {
                    let mut p = ExtendedPiece {
                        number: y[0] >> 4,
                        last: y[0] & 0x0F,
                        lang: lang(&y[1..4]),
                        ..Default::default()
                    };
                    let il = y[4] as usize;
                    let items = &y[5..(5 + il).min(y.len())];
                    let mut k = 0;
                    while k < items.len() {
                        let dl = items[k] as usize;
                        if k + 1 + dl >= items.len() {
                            break;
                        }
                        let desc = items[k + 1..k + 1 + dl].to_vec();
                        let vl = items[k + 1 + dl] as usize;
                        let vs = k + 2 + dl;
                        if vs + vl > items.len() {
                            break;
                        }
                        p.items.push((desc, items[vs..vs + vl].to_vec()));
                        k = vs + vl;
                    }
                    let ts = 5 + il;
                    if ts < y.len() {
                        let tl = y[ts] as usize;
                        p.text = y[ts + 1..(ts + 1 + tl).min(y.len())].to_vec();
                    }
                    ev.extended.push(p);
                }
                0x54 => {
                    for c in y.as_chunks::<2>().0 {
                        if ev.content.len() < 8 {
                            ev.content.push(c[0]);
                        }
                    }
                }
                0x55 => {
                    for c in y.as_chunks::<4>().0 {
                        let country: String = c[..3]
                            .iter()
                            .map(|&k| (k as char).to_ascii_uppercase())
                            .collect();
                        ev.ratings.push((country, c[3]));
                    }
                }
                0x50 if y.len() >= 6 => ev.components.push((y[0] & 0x0F, y[1], lang(&y[3..6]))),
                0x76 => {
                    let mut k = 0;
                    while k < y.len() {
                        let t = y[k] >> 2;
                        let loc = y[k] & 3;
                        if loc == 0 {
                            if k + 1 >= y.len() {
                                break;
                            }
                            let l = y[k + 1] as usize;
                            if k + 2 + l > y.len() {
                                break;
                            }
                            ev.crids.push((
                                t,
                                String::from_utf8_lossy(&y[k + 2..k + 2 + l]).into_owned(),
                            ));
                            k += 2 + l;
                        } else {
                            k += 3;
                        }
                    }
                }
                0x4F if y.len() >= 4 => {
                    ev.time_shifted = Some((
                        u16::from_be_bytes([y[0], y[1]]),
                        u16::from_be_bytes([y[2], y[3]]),
                    ))
                }
                0x5F if y.len() >= 4 => pds = Some(u32::from_be_bytes([y[0], y[1], y[2], y[3]])),
                0x89 if pds == Some(0x233A) && y.len() >= 4 => {
                    let st = if y[0] & 3 == 1 { 2 } else { 1 };
                    if y.len() > st + 3 {
                        ev.guidance = Some(text(&y[st + 3..], TextUse::Description, o));
                    }
                }
                _ => {}
            }
        }
        if descs.damaged() {
            e.damaged = true;
            break;
        }
        e.events.push(ev);
        i = next;
    }
    Some(e)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// TDT, TOT.

/// A local time offset entry of the TOT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalTimeOffset {
    /// ISO 3166 country code.
    pub country: String,
    /// country_region_id.
    pub region: u8,
    /// Offset in minutes (signed).
    pub offset_min: i32,
    /// time_of_change (Unix seconds).
    pub time_of_change: i64,
    /// next_time_offset in minutes (signed).
    pub next_offset_min: i32,
}

impl LocalTimeOffset {
    /// The offset in force at `utc`.
    pub fn at(&self, utc: i64) -> i32 {
        if utc >= self.time_of_change {
            self.next_offset_min
        } else {
            self.offset_min
        }
    }
}

/// UTC of a TDT or TOT section (`None` when undefined).
pub fn tdt_tot_utc(s: &[u8]) -> Option<i64> {
    mjd_bcd_to_unix(s.get(3..8)?)
}

/// The local time offsets of a checked TOT section.
pub fn tot_offsets(s: &[u8], h: &SectionHeader) -> Vec<LocalTimeOffset> {
    let mut out = Vec::new();
    let Some((d, _)) = loop12(&s[..h.total_len.saturating_sub(4)], 8) else {
        return out;
    };
    for (tag, x) in Descriptors::new(d) {
        if tag != 0x58 {
            continue;
        }
        for c in x.as_chunks::<13>().0 {
            let sign = if c[3] & 1 != 0 { -1 } else { 1 };
            let hm = |a: u8, b: u8| -> i32 {
                let h = crate::time::bcd(a).unwrap_or(0) as i32;
                let m = crate::time::bcd(b).unwrap_or(0) as i32;
                h * 60 + m
            };
            out.push(LocalTimeOffset {
                country: c[..3]
                    .iter()
                    .map(|&k| (k as char).to_ascii_uppercase())
                    .collect(),
                region: c[3] >> 2,
                offset_min: sign * hm(c[4], c[5]),
                time_of_change: mjd_bcd_to_unix(&c[6..11]).unwrap_or(i64::MAX),
                next_offset_min: sign * hm(c[11], c[12]),
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------------------
// ATSC virtual channel table.

/// One ATSC virtual channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualChannel {
    /// short_name (non-printable characters as "?").
    pub short_name: String,
    /// major_channel_number.
    pub major: u16,
    /// minor_channel_number.
    pub minor: u16,
    /// service_type (2 TV, 3 radio).
    pub service_type: u8,
    /// program_number.
    pub program_number: u16,
    /// channel_TSID.
    pub tsid: u16,
}

/// Parses a checked TVCT (0xC8) or CVCT (0xC9) section.
pub fn vct(s: &[u8], h: &SectionHeader) -> Vec<VirtualChannel> {
    let b = body(s, h);
    let mut out = Vec::new();
    if b.len() < 2 {
        return out;
    }
    let n = b[1] as usize;
    let mut i = 2;
    for _ in 0..n.min(256) {
        if i + 32 > b.len() {
            break;
        }
        let x = &b[i..];
        let name: String = (0..7)
            .map(|k| u16::from_be_bytes([x[2 * k], x[2 * k + 1]]))
            .take_while(|&u| u != 0)
            .map(|u| {
                char::from_u32(u as u32)
                    .filter(|c| !c.is_control())
                    .unwrap_or('?')
            })
            .collect();
        let major = ((x[14] as u16 & 0x0F) << 6) | (x[15] as u16 >> 2);
        let minor = ((x[15] as u16 & 0x03) << 8) | x[16] as u16;
        let tsid = u16::from_be_bytes([x[22], x[23]]);
        let program_number = u16::from_be_bytes([x[24], x[25]]);
        let service_type = x[27] & 0x3F;
        let dl = ((x[30] as usize & 0x03) << 8) | x[31] as usize;
        out.push(VirtualChannel {
            short_name: name.trim().to_string(),
            major,
            minor,
            service_type,
            program_number,
            tsid,
        });
        i += 32 + dl;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Writing sections (generators and tests).

/// Builds a long section: header, `body`, CRC.
pub fn build_long(
    table_id: u8,
    ext: u16,
    version: u8,
    number: u8,
    last: u8,
    body: &[u8],
) -> Vec<u8> {
    let len = 5 + body.len() + 4;
    let mut s = vec![table_id, 0xB0 | ((len >> 8) as u8 & 0x0F), len as u8];
    s.extend_from_slice(&ext.to_be_bytes());
    s.push(0xC0 | (version & 0x1F) << 1 | 1);
    s.push(number);
    s.push(last);
    s.extend_from_slice(body);
    let crc = CRC32_MPEG2.checksum(&s);
    s.extend_from_slice(&crc.to_be_bytes());
    s
}

/// A descriptor: tag, length, payload.
pub fn descriptor(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![tag, payload.len() as u8];
    v.extend_from_slice(payload);
    v
}

/// A 12-bit length-prefixed loop (with the top 4 bits set to 0xF).
pub fn loop_bytes(content: &[u8]) -> Vec<u8> {
    let mut v = vec![
        0xF0 | ((content.len() >> 8) as u8 & 0x0F),
        content.len() as u8,
    ];
    v.extend_from_slice(content);
    v
}

/// A PAT section.
pub fn build_pat(tsid: u16, version: u8, programs: &[(u16, u16)]) -> Vec<u8> {
    let mut b = Vec::new();
    for &(p, pid) in programs {
        b.extend_from_slice(&p.to_be_bytes());
        b.extend_from_slice(&(0xE000 | pid).to_be_bytes());
    }
    build_long(0x00, tsid, version, 0, 0, &b)
}

/// A PMT section: (stream_type, pid, ES descriptors).
pub fn build_pmt(
    program: u16,
    version: u8,
    pcr_pid: u16,
    program_desc: &[u8],
    streams: &[(u8, u16, Vec<u8>)],
) -> Vec<u8> {
    let mut b = (0xE000 | pcr_pid).to_be_bytes().to_vec();
    b.extend_from_slice(&loop_bytes(program_desc));
    for (st, pid, d) in streams {
        b.push(*st);
        b.extend_from_slice(&(0xE000 | pid).to_be_bytes());
        b.extend_from_slice(&loop_bytes(d));
    }
    build_long(0x02, program, version, 0, 0, &b)
}

/// An SDT actual section with service descriptors: (sid, service_type, provider, name, free_CA_mode). Every service is
/// running and has both EIT flags.
pub fn build_sdt(
    tsid: u16,
    onid: u16,
    version: u8,
    number: u8,
    last: u8,
    services: &[(u16, u8, &str, &str, bool)],
) -> Vec<u8> {
    let mut b = onid.to_be_bytes().to_vec();
    b.push(0xFF);
    for &(sid, st, prov, name, free_ca) in services {
        b.extend_from_slice(&sid.to_be_bytes());
        b.push(0xFC | 0x03);
        let mut sd = vec![st, prov.len() as u8];
        sd.extend_from_slice(prov.as_bytes());
        sd.push(name.len() as u8);
        sd.extend_from_slice(name.as_bytes());
        let d = descriptor(0x48, &sd);
        let l = d.len();
        b.push(0x80 | u8::from(free_ca) << 4 | ((l >> 8) as u8 & 0x0F)); // running 4
        b.push(l as u8);
        b.extend_from_slice(&d);
    }
    build_long(0x42, tsid, version, number, last, &b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pat_pmt_round_trip_and_crc_rules() {
        let s = build_pat(7, 3, &[(0, 0x10), (100, 0x100)]);
        let h = check(&s).unwrap();
        assert_eq!((h.table_id, h.ext, h.version), (0, 7, 3));
        assert_eq!(pat(&s, &h).programs, vec![(0, 0x10), (100, 0x100)]);
        let mut bad = s.clone();
        bad[9] ^= 1;
        assert_eq!(check(&bad), Err(Error::Section(SectionFault::Crc)));
        // A damaged PAT with its syntax bit cleared is still CRC-checked (by table_id).
        bad[1] &= 0x7F;
        assert_eq!(check(&bad), Err(Error::Section(SectionFault::Crc)));
        let mut lang = b"eng".to_vec();
        lang.push(0);
        let p = build_pmt(
            100,
            0,
            0x101,
            &[],
            &[
                (0x1B, 0x101, vec![]),
                (0x0F, 0x102, descriptor(0x0A, &lang)),
            ],
        );
        let h = check(&p).unwrap();
        let m = pmt(&p, &h).unwrap();
        assert_eq!(m.pcr_pid, 0x101);
        assert_eq!(m.streams.len(), 2);
        assert_eq!(m.streams[1].desc.languages, vec![("eng".to_string(), 0)]);
    }

    #[test]
    fn short_and_truncated_sections_are_rejected() {
        assert_eq!(
            check(&[0x00, 0xB0, 0x02, 0, 0]),
            Err(Error::Section(SectionFault::Short))
        );
        assert_eq!(
            check(&[0x00, 0xB0, 0x40, 0, 0, 0, 0, 0]),
            Err(Error::Section(SectionFault::Length))
        );
        let big = vec![0u8; 1100];
        let mut s = vec![0x02, 0xB4, 0x40];
        s.extend_from_slice(&big);
        assert_eq!(check(&s), Err(Error::Section(SectionFault::Length)));
    }
}
