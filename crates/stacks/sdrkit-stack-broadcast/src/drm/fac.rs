//! The Fast Access Channel and the Service Description Channel (ES 201 980 clauses 6.3 and 6.4): fields, coding to bits and back.

use super::params::Robustness;
use sdrkit_util_bits::crc::catalog::{CRC16_GENIBUS, CRC8_SAE_J1850};
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::pack::{pack, push_u64, unpack, Pad};
use sdrkit_util_bits::{BitOrder, BitReader, BitWriter, HardBits};
use sdrkit_util_geo::time::date_from_mjd;
use sdrkit_util_ids::pty::PtyTable;

/// One service as the FAC describes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FacService {
    /// 24-bit service identifier.
    pub id: u32,
    /// Short id 0..3.
    pub short_id: u8,
    /// Audio conditional access.
    pub audio_ca: bool,
    /// Language code (table 18).
    pub language: u8,
    /// Data service (else audio).
    pub data: bool,
    /// Programme type (audio) or application id (data).
    pub descriptor: u8,
    /// Data conditional access.
    pub data_ca: bool,
}

/// One FAC block.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fac {
    /// Enhancement layer flag.
    pub enhancement: bool,
    /// 0 first frame (valid AFS index), 1 intermediate, 2 last, 3 first without a valid AFS index.
    pub identity: u8,
    /// RM flag (mode E).
    pub rm: bool,
    /// Spectrum occupancy 0..5.
    pub occupancy: u8,
    /// Short interleaver (400 ms; else long).
    pub short_interleaver: bool,
    /// MSC mode (0 64-QAM, 1 and 2 hierarchical, 3 16-QAM; mode E 0 16-QAM, 3 4-QAM).
    pub msc_mode: u8,
    /// SDC mode (0 16-QAM, 1 4-QAM).
    pub sdc_mode: u8,
    /// Number of services code (clause 6.3.3).
    pub services_code: u8,
    /// Reconfiguration index.
    pub reconfiguration: u8,
    /// Toggle.
    pub toggle: bool,
    /// The service(s) this block describes (one; two in mode E).
    pub services: Vec<FacService>,
}

/// Audio and data service counts of a services code; `None` when reserved.
pub fn service_counts(code: u8) -> Option<(u8, u8)> {
    Some(match code {
        0b0000 => (4, 0),
        0b0001 => (0, 1),
        0b0010 => (0, 2),
        0b0011 => (0, 3),
        0b0100 => (1, 0),
        0b0101 => (1, 1),
        0b0110 => (1, 2),
        0b0111 => (1, 3),
        0b1000 => (2, 0),
        0b1001 => (2, 1),
        0b1010 => (2, 2),
        0b1100 => (3, 0),
        0b1101 => (3, 1),
        0b1111 => (0, 4),
        _ => return None,
    })
}

/// The services code of audio and data counts.
pub fn services_code(audio: u8, data: u8) -> Option<u8> {
    (0u8..16).find(|&c| service_counts(c) == Some((audio, data)))
}

impl Fac {
    /// The FAC bits including the CRC (72; mode E 116), one bit per byte.
    pub fn to_bits(&self, r: Robustness) -> Vec<u8> {
        let mut b = Vec::with_capacity(r.fac_bits());
        for (v, n) in [
            (self.enhancement as u64, 1),
            (self.identity as u64, 2),
            (self.rm as u64, 1),
            (self.occupancy as u64, 3),
            (self.short_interleaver as u64, 1),
            (self.msc_mode as u64, 2),
            (self.sdc_mode as u64, 1),
            (self.services_code as u64, 4),
            (self.reconfiguration as u64, 3),
            (self.toggle as u64, 1),
            (0, 1),
        ] {
            push_u64(&mut b, v, n);
        }
        let n = if r == Robustness::E { 2 } else { 1 };
        for i in 0..n {
            let s = self.services.get(i).cloned().unwrap_or_default();
            for (v, n) in [
                (s.id as u64, 24),
                (s.short_id as u64, 2),
                (s.audio_ca as u64, 1),
                (s.language as u64, 4),
                (s.data as u64, 1),
                (s.descriptor as u64, 5),
                (s.data_ca as u64, 1),
                (0, 6),
            ] {
                push_u64(&mut b, v, n);
            }
        }
        CRC8_SAE_J1850.append_bits(&mut b);
        b
    }
    /// Reads FAC bits (one bit per byte). `None` when the input is short, the CRC fails or a value is reserved.
    pub fn from_bits(bits: &[u8], r: Robustness) -> Option<Fac> {
        let n = r.fac_bits();
        if bits.len() < n || !CRC8_SAE_J1850.verify_bits(&bits[..n]) {
            return None;
        }
        let mut rd = HardBits::new(&bits[..n]);
        let mut f = Fac {
            enhancement: rd.u(1) != 0,
            identity: rd.u(2) as u8,
            rm: rd.u(1) != 0,
            occupancy: rd.u(3) as u8,
            short_interleaver: rd.u(1) != 0,
            msc_mode: rd.u(2) as u8,
            sdc_mode: rd.u(1) as u8,
            services_code: rd.u(4) as u8,
            reconfiguration: rd.u(3) as u8,
            toggle: rd.u(1) != 0,
            services: Vec::new(),
        };
        rd.u(1);
        service_counts(f.services_code)?;
        r.range(f.occupancy)?;
        let ns = if r == Robustness::E { 2 } else { 1 };
        for _ in 0..ns {
            let s = FacService {
                id: rd.u(24) as u32,
                short_id: rd.u(2) as u8,
                audio_ca: rd.u(1) != 0,
                language: rd.u(4) as u8,
                data: rd.u(1) != 0,
                descriptor: rd.u(5) as u8,
                data_ca: rd.u(1) != 0,
            };
            rd.u(6);
            f.services.push(s);
        }
        Some(f)
    }
    /// The configuration part (everything but the identity, toggle and service), for consistency checks.
    pub fn config_key(&self) -> (u8, bool, u8, u8, u8, u8) {
        (
            self.occupancy,
            self.short_interleaver,
            self.msc_mode,
            self.sdc_mode,
            self.services_code,
            self.reconfiguration,
        )
    }
}

/// The name of a FAC language code (ES 201 980 clause 6.3.4, table 18).
pub fn language_name(code: u8) -> &'static str {
    sdrkit_util_ids::language::drm(code & 15).unwrap_or("Other language")
}

/// The name of a FAC programme type (ES 201 980 clause 6.3.4, table 19). Codes 30 and 31 are not programme types.
pub fn programme_type_name(code: u8) -> &'static str {
    sdrkit_util_ids::pty::name(code & 31, PtyTable::Drm).unwrap_or("Not used")
}

/// The text `YYYY-MM-DD HH:MM` of an MJD and a time of day, or `None` when the MJD is out of range.
pub fn date_text(mjd: u32, hour: u8, minute: u8) -> Option<String> {
    let (y, m, d) = date_from_mjd(mjd as i64).ok()?;
    Some(format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}"))
}

/// The short weekday name of an MJD (MJD 0 was a Wednesday).
pub fn weekday(mjd: u32) -> &'static str {
    ["Wed", "Thu", "Fri", "Sat", "Sun", "Mon", "Tue"][(mjd % 7) as usize]
}

/// SDC type 9: audio information.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioInfo {
    /// Short id.
    pub short_id: u8,
    /// Stream id.
    pub stream: u8,
    /// 0 AAC, 3 xHE-AAC.
    pub coding: u8,
    /// SBR.
    pub sbr: bool,
    /// Audio mode (AAC 0 mono, 1 PS, 2 stereo; xHE-AAC 0 mono, 2 stereo).
    pub mode: u8,
    /// Sampling rate code.
    pub rate_code: u8,
    /// Text messages carried.
    pub text: bool,
    /// Enhancement flag.
    pub enhancement: bool,
    /// MPEG Surround mode.
    pub surround: u8,
    /// xHE-AAC static configuration bytes.
    pub config: Vec<u8>,
}

impl AudioInfo {
    /// The core sampling rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        if self.coding == 3 {
            [0, 0, 16_000, 19_200, 24_000, 32_000, 38_400, 48_000][self.rate_code as usize & 7]
        } else {
            [8_000, 12_000, 16_000, 24_000, 0, 48_000, 0, 0][self.rate_code as usize & 7]
        }
    }
}

/// SDC type 5: application information.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppInfo {
    /// Short id.
    pub short_id: u8,
    /// Stream id.
    pub stream: u8,
    /// Packet mode (else stream mode).
    pub packet_mode: bool,
    /// Data units (packet mode).
    pub data_units: bool,
    /// Packet id.
    pub packet_id: u8,
    /// Application domain (0 DRM, 1 DAB).
    pub domain: u8,
    /// Packet length.
    pub packet_len: u8,
    /// Application data (DAB domain: the user application type in the first 11 bits).
    pub data: Vec<u8>,
}

impl AppInfo {
    /// The DAB user application type.
    pub fn user_app_type(&self) -> Option<u16> {
        (self.domain == 1 && self.data.len() >= 2)
            .then(|| (((self.data[0] as u16) << 8 | self.data[1] as u16) >> 5) & 0x7FF)
    }
}

/// One SDC data entity.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Entity {
    /// Type 0: multiplex description.
    Multiplex {
        /// Version flag (1 = announced for the next configuration).
        next: bool,
        /// Protection levels A and B.
        prot_a: u8,
        /// Level B.
        prot_b: u8,
        /// Stream lengths A and B in bytes.
        streams: Vec<(u16, u16)>,
    },
    /// Type 1: label.
    Label {
        /// Short id.
        short_id: u8,
        /// The label.
        text: String,
    },
    /// Type 5: application information.
    Application(AppInfo),
    /// Type 8: time and date.
    Time {
        /// MJD.
        mjd: u32,
        /// Hours UTC.
        hour: u8,
        /// Minutes.
        minute: u8,
        /// Local offset in half hours, when sent.
        offset_half_hours: Option<i8>,
    },
    /// Type 9: audio information.
    Audio(AudioInfo),
    /// Type 12: language and country.
    Language {
        /// Short id.
        short_id: u8,
        /// ISO 639-2.
        language: String,
        /// ISO 3166.
        country: String,
    },
    /// Any other type, not interpreted.
    Other {
        /// The type.
        kind: u8,
        /// The body.
        body: Vec<u8>,
    },
}

fn header(w: &mut BitWriter, len: usize, version: bool, kind: u8) {
    w.put(len as u64, 7);
    w.put(version as u64, 1);
    w.put(kind as u64, 4);
}

impl Entity {
    /// Writes the entity (header, 4 bits, then the body bytes).
    pub fn write(&self, w: &mut BitWriter) {
        match self {
            Entity::Multiplex {
                next,
                prot_a,
                prot_b,
                streams,
            } => {
                header(w, 3 * streams.len(), *next, 0);
                w.put(*prot_a as u64, 2);
                w.put(*prot_b as u64, 2);
                for &(a, b) in streams {
                    w.put(a as u64, 12);
                    w.put(b as u64, 12);
                }
            }
            Entity::Label { short_id, text } => {
                let b = text.as_bytes();
                let n = b.len().min(64);
                header(w, n, false, 1);
                w.put(*short_id as u64, 2);
                w.put(0, 2);
                w.bytes(&b[..n]);
            }
            Entity::Application(a) => {
                let n = 2 + a.data.len();
                header(w, n, false, 5);
                w.put(a.short_id as u64, 2);
                w.put(a.stream as u64, 2);
                w.put(a.packet_mode as u64, 1);
                w.put(a.data_units as u64, 1);
                w.put(a.packet_id as u64, 2);
                w.put(0, 1);
                w.put(a.domain as u64, 3);
                w.put(a.packet_len as u64, 8);
                w.bytes(&a.data);
            }
            Entity::Time {
                mjd,
                hour,
                minute,
                offset_half_hours,
            } => {
                let n = if offset_half_hours.is_some() { 4 } else { 3 };
                header(w, n, false, 8);
                w.put(*mjd as u64, 17);
                w.put(*hour as u64, 5);
                w.put(*minute as u64, 6);
                if let Some(o) = offset_half_hours {
                    w.put(0, 2);
                    w.put((*o < 0) as u64, 1);
                    w.put(o.unsigned_abs() as u64, 5);
                }
            }
            Entity::Audio(a) => {
                let n = 2 + a.config.len();
                header(w, n, false, 9);
                w.put(a.short_id as u64, 2);
                w.put(a.stream as u64, 2);
                w.put(a.coding as u64, 2);
                w.put(a.sbr as u64, 1);
                w.put(a.mode as u64, 2);
                w.put(a.rate_code as u64, 3);
                w.put(a.text as u64, 1);
                w.put(a.enhancement as u64, 1);
                w.put(a.surround as u64, 3);
                w.put(0, 3);
                w.bytes(&a.config);
            }
            Entity::Language {
                short_id,
                language,
                country,
            } => {
                header(w, 5, false, 12);
                let l = language.as_bytes();
                let c = country.as_bytes();
                w.put(*short_id as u64, 2);
                w.put(0, 2);
                for i in 0..3 {
                    w.put(*l.get(i).unwrap_or(&b' ') as u64, 8);
                }
                for i in 0..2 {
                    w.put(*c.get(i).unwrap_or(&b' ') as u64, 8);
                }
            }
            Entity::Other { kind, body } => {
                header(w, body.len(), false, *kind);
                w.put(0, 4);
                w.bytes(body);
            }
        }
    }
    /// The type number.
    pub fn kind(&self) -> u8 {
        match self {
            Entity::Multiplex { .. } => 0,
            Entity::Label { .. } => 1,
            Entity::Application(_) => 5,
            Entity::Time { .. } => 8,
            Entity::Audio(_) => 9,
            Entity::Language { .. } => 12,
            Entity::Other { kind, .. } => *kind,
        }
    }
    /// The time of a type 8 entity as `YYYY-MM-DD HH:MM`.
    pub fn time_text(&self) -> Option<String> {
        match self {
            Entity::Time {
                mjd, hour, minute, ..
            } => date_text(*mjd, *hour, *minute),
            _ => None,
        }
    }
}

/// The next `n` bytes at any bit position, or an empty vector when fewer are left.
fn take_bytes(rd: &mut BitReader, n: usize) -> Vec<u8> {
    if rd.left() < 8 * n {
        rd.skip(rd.left());
        return Vec::new();
    }
    let mut out = vec![0u8; n];
    rd.read_bytes(&mut out);
    out
}

/// Reads one entity. `None` when the entity is longer than what is left.
fn read_entity(rd: &mut BitReader) -> Option<Entity> {
    let len = rd.u(7) as usize;
    let version = rd.u(1) != 0;
    let kind = rd.u(4) as u8;
    let start = rd.pos();
    let total = 4 + 8 * len;
    if rd.left() < total {
        return None;
    }
    let e = match kind {
        0 => {
            let prot_a = rd.u(2) as u8;
            let prot_b = rd.u(2) as u8;
            let mut streams = Vec::new();
            for _ in 0..len / 3 {
                streams.push((rd.u(12) as u16, rd.u(12) as u16));
            }
            Entity::Multiplex {
                next: version,
                prot_a,
                prot_b,
                streams,
            }
        }
        1 => {
            let short_id = rd.u(2) as u8;
            rd.skip(2);
            let b = take_bytes(rd, len);
            Entity::Label {
                short_id,
                text: String::from_utf8_lossy(&b)
                    .trim_end_matches('\0')
                    .to_string(),
            }
        }
        5 if len >= 2 => {
            let short_id = rd.u(2) as u8;
            let stream = rd.u(2) as u8;
            let packet_mode = rd.u(1) != 0;
            let data_units = rd.u(1) != 0;
            let packet_id = rd.u(2) as u8;
            rd.skip(1);
            let domain = rd.u(3) as u8;
            let packet_len = rd.u(8) as u8;
            let data = take_bytes(rd, len - 2);
            Entity::Application(AppInfo {
                short_id,
                stream,
                packet_mode,
                data_units,
                packet_id,
                domain,
                packet_len,
                data,
            })
        }
        8 if len >= 3 => {
            let mjd = rd.u(17) as u32;
            let hour = rd.u(5) as u8;
            let minute = rd.u(6) as u8;
            let off = if len >= 4 {
                rd.skip(2);
                let neg = rd.u(1) != 0;
                let v = rd.u(5) as i8;
                Some(if neg { -v } else { v })
            } else {
                None
            };
            Entity::Time {
                mjd,
                hour,
                minute,
                offset_half_hours: off,
            }
        }
        9 if len >= 2 => {
            let a = AudioInfo {
                short_id: rd.u(2) as u8,
                stream: rd.u(2) as u8,
                coding: rd.u(2) as u8,
                sbr: rd.u(1) != 0,
                mode: rd.u(2) as u8,
                rate_code: rd.u(3) as u8,
                text: rd.u(1) != 0,
                enhancement: rd.u(1) != 0,
                surround: rd.u(3) as u8,
                config: {
                    rd.skip(3);
                    take_bytes(rd, len - 2)
                },
            };
            Entity::Audio(a)
        }
        12 if len >= 5 => {
            let short_id = rd.u(2) as u8;
            rd.skip(2);
            let l = take_bytes(rd, 3);
            let c = take_bytes(rd, 2);
            Entity::Language {
                short_id,
                language: String::from_utf8_lossy(&l).to_string(),
                country: String::from_utf8_lossy(&c).to_string(),
            }
        }
        _ => {
            rd.skip(4);
            Entity::Other {
                kind,
                body: take_bytes(rd, len),
            }
        }
    };
    // Skip whatever of the body was not read.
    let used = rd.pos() - start;
    if used < total {
        rd.skip(total - used);
    }
    (!rd.overrun()).then_some(e)
}

/// One SDC block.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sdc {
    /// AFS index.
    pub afs: u8,
    /// The entities.
    pub entities: Vec<Entity>,
}

impl Sdc {
    /// The SDC block with an 8-bit AFS field (its 4 high bits zero), the data field of `data_len` bytes padded with zeros, and
    /// the CRC-16 over both: the bytes the CRC covers (clause 6.4.2). `None` when the entities do not fit.
    pub fn to_bytes(&self, data_len: usize) -> Option<Vec<u8>> {
        let mut body = Vec::new();
        let mut w = BitWriter::new(&mut body);
        for e in &self.entities {
            e.write(&mut w);
        }
        w.finish();
        if body.len() > data_len {
            return None;
        }
        let mut field = vec![self.afs & 15];
        field.extend_from_slice(&body);
        field.resize(1 + data_len, 0);
        CRC16_GENIBUS.append(&mut field, Trailer::Big).ok()?;
        Some(field)
    }
    /// The SDC block as sent (clause 6.4.2): the AFS index (4 bits), the data field and the CRC, one bit per byte, without the
    /// padding.
    pub fn to_bits(&self, data_len: usize) -> Option<Vec<u8>> {
        let b = self.to_bytes(data_len)?;
        let mut bits = Vec::with_capacity(8 * b.len());
        unpack(&b, BitOrder::Msb, &mut bits);
        bits.drain(..4);
        Some(bits)
    }
    /// Reads an SDC block (as produced by [`to_bytes`](Self::to_bytes)); `None` when the CRC fails.
    pub fn from_bytes(b: &[u8]) -> Option<Sdc> {
        if b.len() < 4 || !CRC16_GENIBUS.verify(b, Trailer::Big) {
            return None;
        }
        let afs = b[0] & 15;
        let data = &b[1..b.len() - 2];
        let mut rd = BitReader::new(data);
        let mut entities = Vec::new();
        while rd.left() >= 12 {
            // Padding: a zero length and type 0 with nothing after it ends the list.
            let head = rd.peek(12);
            if head >> 5 == 0 && head & 15 == 0 {
                break;
            }
            match read_entity(&mut rd) {
                Some(e) => entities.push(e),
                None => break,
            }
        }
        Some(Sdc { afs, entities })
    }
    /// Reads an SDC block as sent (the bits of [`to_bits`](Self::to_bits), padding after them allowed), with a data field of
    /// `data_len` bytes. `None` when the input is short or the CRC fails.
    pub fn from_bits(bits: &[u8], data_len: usize) -> Option<Sdc> {
        let n = 4 + 8 * data_len + 16;
        if bits.len() < n {
            return None;
        }
        let mut full = vec![0u8; 4];
        full.extend_from_slice(&bits[..n]);
        let mut b = Vec::with_capacity(full.len() / 8);
        pack(&full, BitOrder::Msb, Pad::Zero, &mut b);
        Sdc::from_bytes(&b)
    }
}

/// The length of the SDC data field in bytes for `l_sdc` information bits (clause 6.4.2, table 21): the bits less the AFS index
/// and the CRC, in whole bytes.
pub fn sdc_data_len(l_sdc: usize) -> usize {
    l_sdc.saturating_sub(20) / 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fac_round_trip_and_crc() {
        let f = Fac {
            identity: 2,
            occupancy: 3,
            msc_mode: 0,
            services_code: services_code(1, 0).unwrap(),
            toggle: true,
            services: vec![FacService {
                id: 0xE12345,
                short_id: 0,
                language: 5,
                descriptor: 10,
                ..Default::default()
            }],
            ..Default::default()
        };
        let b = f.to_bits(Robustness::B);
        assert_eq!(b.len(), 72);
        assert_eq!(Fac::from_bits(&b, Robustness::B), Some(f.clone()));
        let mut bad = b.clone();
        bad[30] ^= 1;
        assert_eq!(Fac::from_bits(&bad, Robustness::B), None);
        let mut fe = f.clone();
        fe.occupancy = 0;
        fe.services.push(FacService::default());
        let be = fe.to_bits(Robustness::E);
        assert_eq!(be.len(), 116);
        assert_eq!(Fac::from_bits(&be, Robustness::E), Some(fe));
        assert_eq!(service_counts(0b1011), None);
    }

    #[test]
    fn sdc_every_entity_round_trips() {
        let s = Sdc {
            afs: 5,
            entities: vec![
                Entity::Multiplex {
                    next: false,
                    prot_a: 0,
                    prot_b: 1,
                    streams: vec![(0, 300), (0, 576)],
                },
                Entity::Label {
                    short_id: 0,
                    text: "Test DRM".into(),
                },
                Entity::Application(AppInfo {
                    short_id: 1,
                    stream: 1,
                    packet_mode: true,
                    data_units: true,
                    packet_id: 1,
                    domain: 1,
                    packet_len: 45,
                    data: vec![0x00, 0x40],
                }),
                Entity::Time {
                    mjd: 61319,
                    hour: 12,
                    minute: 0,
                    offset_half_hours: Some(-3),
                },
                Entity::Audio(AudioInfo {
                    coding: 0,
                    rate_code: 3,
                    text: true,
                    ..Default::default()
                }),
                Entity::Language {
                    short_id: 0,
                    language: "eng".into(),
                    country: "ae".into(),
                },
                Entity::Other {
                    kind: 7,
                    body: vec![1, 2, 3],
                },
            ],
        };
        let b = s.to_bytes(80).unwrap();
        let back = Sdc::from_bytes(&b).unwrap();
        assert_eq!(Sdc::from_bits(&s.to_bits(80).unwrap(), 80), Some(s.clone()));
        assert_eq!(back, s);
        match &back.entities[2] {
            Entity::Application(a) => assert_eq!(a.user_app_type(), Some(0x002)),
            _ => panic!(),
        }
        assert_eq!(back.entities[3].time_text().unwrap(), "2026-10-06 12:00");
        let mut bad = b.clone();
        bad[10] ^= 4;
        assert!(Sdc::from_bytes(&bad).is_none());
        assert!(s.to_bytes(10).is_none());
    }
}
