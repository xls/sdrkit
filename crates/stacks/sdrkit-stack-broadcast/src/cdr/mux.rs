//! The CDR multiplex: the control multiplex frame (SMCT and NIT; the ESG is listed as not decoded) and the service multiplex frame
//! (header, sub-frames with audio units, data units and the stream extension), with CRC-8 headers and CRC-32 tables and sections.
//! The byte layouts are sdrkit's own reading of the standard.
//!
//! Standard: GY/T 268.2-2013.

use sdrkit_util_bits::crc::catalog::{CRC32_BZIP2, CRC8_CDR};

fn cdr_crc8(b: &[u8]) -> u8 {
    CRC8_CDR.checksum(b)
}

fn cdr_crc32(b: &[u8]) -> u32 {
    CRC32_BZIP2.checksum(b)
}

/// One service multiplex frame's entry in the SMCT.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SmctEntry {
    /// Service multiplex frame id.
    pub id: u8,
    /// Hierarchical.
    pub hierarchical: bool,
    /// High protection.
    pub high: bool,
    /// Transmission mask of the four logical frames.
    pub mask: u8,
    /// The service id of each multiplex sub-frame.
    pub services: Vec<u16>,
}

/// The network information table.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Nit {
    /// Update counter.
    pub update: u8,
    /// Country (three letters).
    pub country: String,
    /// 36-bit network id.
    pub network_id: u64,
    /// Centre frequencies in 10 Hz.
    pub frequencies: Vec<u32>,
    /// Network name.
    pub name: String,
}

/// A control multiplex frame.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ControlFrame {
    /// SMCT update counter.
    pub smct_update: u8,
    /// SMCT.
    pub smct: Vec<SmctEntry>,
    /// NIT.
    pub nit: Option<Nit>,
    /// Tables present but not decoded (ESG).
    pub not_decoded: Vec<u8>,
}

fn table(id: u8, body: &[u8]) -> Vec<u8> {
    let mut t = vec![id];
    t.extend_from_slice(&(body.len() as u16).to_be_bytes());
    t.extend_from_slice(body);
    let c = cdr_crc32(&t);
    t.extend_from_slice(&c.to_be_bytes());
    t
}

impl ControlFrame {
    /// The frame's bytes, `len` long (padded with ones).
    pub fn to_bytes(&self, len: usize) -> Option<Vec<u8>> {
        let mut smct = vec![0, 1, self.smct_update, self.smct.len() as u8];
        for e in &self.smct {
            smct.push(e.id);
            smct.push((e.hierarchical as u8) << 1 | e.high as u8);
            smct.push(e.mask);
            smct.push(e.services.len() as u8);
            for s in &e.services {
                smct.extend_from_slice(&s.to_be_bytes());
            }
        }
        let mut tables = table(0x01, &smct);
        if let Some(n) = &self.nit {
            let mut b = vec![0, n.update];
            let c = n.country.as_bytes();
            for i in 0..3 {
                b.push(*c.get(i).unwrap_or(&b' '));
            }
            b.extend_from_slice(&n.network_id.to_be_bytes()[3..]);
            b.push(n.frequencies.len() as u8);
            for f in &n.frequencies {
                b.extend_from_slice(&f.to_be_bytes());
            }
            b.push(n.name.len() as u8);
            b.extend_from_slice(n.name.as_bytes());
            tables.extend(table(0x02, &b));
        }
        for &t in &self.not_decoded {
            tables.extend(table(t, &[0, 0]));
        }
        let mut f = vec![0xC0];
        f.extend_from_slice(&(tables.len() as u16).to_be_bytes());
        let c = cdr_crc8(&f);
        f.push(c);
        f.extend(tables);
        if f.len() > len {
            return None;
        }
        f.resize(len, 0xFF);
        Some(f)
    }
    /// Reads a control frame; returns it with the counts of tables good and bad.
    pub fn parse(b: &[u8]) -> Option<(ControlFrame, u32, u32)> {
        if b.len() < 4 || cdr_crc8(&b[..3]) != b[3] || b[0] != 0xC0 {
            return None;
        }
        let tl = u16::from_be_bytes([b[1], b[2]]) as usize;
        let tables = b.get(4..4 + tl)?;
        let mut p = 0;
        let mut cf = ControlFrame::default();
        let (mut good, mut bad) = (0, 0);
        while p + 3 <= tables.len() {
            let id = tables[p];
            let n = u16::from_be_bytes([tables[p + 1], tables[p + 2]]) as usize;
            let Some(t) = tables.get(p..p + 3 + n + 4) else {
                bad += 1;
                break;
            };
            let crc = u32::from_be_bytes([t[3 + n], t[4 + n], t[5 + n], t[6 + n]]);
            p += 3 + n + 4;
            if cdr_crc32(&t[..3 + n]) != crc {
                bad += 1;
                continue;
            }
            good += 1;
            let body = &t[3..3 + n];
            match id {
                0x01 if body.len() >= 4 => {
                    cf.smct_update = body[2];
                    let mut q = 4;
                    for _ in 0..body[3] {
                        let Some(h) = body.get(q..q + 4) else { break };
                        let ns = h[3] as usize;
                        let mut services = Vec::new();
                        for j in 0..ns {
                            if let Some(s) = body.get(q + 4 + 2 * j..q + 6 + 2 * j) {
                                services.push(u16::from_be_bytes([s[0], s[1]]));
                            }
                        }
                        cf.smct.push(SmctEntry {
                            id: h[0],
                            hierarchical: h[1] & 2 != 0,
                            high: h[1] & 1 != 0,
                            mask: h[2],
                            services,
                        });
                        q += 4 + 2 * ns;
                    }
                }
                0x02 if body.len() >= 11 => {
                    let mut id = [0u8; 8];
                    id[3..].copy_from_slice(&body[5..10]);
                    let nf = body[10] as usize;
                    let mut freqs = Vec::new();
                    for j in 0..nf {
                        if let Some(f) = body.get(11 + 4 * j..15 + 4 * j) {
                            freqs.push(u32::from_be_bytes([f[0], f[1], f[2], f[3]]));
                        }
                    }
                    let q = 11 + 4 * nf;
                    let nl = *body.get(q).unwrap_or(&0) as usize;
                    cf.nit = Some(Nit {
                        update: body[1],
                        country: String::from_utf8_lossy(&body[2..5]).to_string(),
                        network_id: u64::from_be_bytes(id) & 0xF_FFFF_FFFF,
                        frequencies: freqs,
                        name: String::from_utf8_lossy(body.get(q + 1..q + 1 + nl).unwrap_or(&[]))
                            .to_string(),
                    });
                }
                x => cf.not_decoded.push(x),
            }
        }
        Some((cf, good, bad))
    }
}

/// An audio stream's description (the sub-frame extension).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct AudioStream {
    /// Algorithm code.
    pub algorithm: u8,
    /// Bit rate, in 100 bit/s.
    pub bitrate: u16,
    /// Sampling rate code (table 9).
    pub rate_code: u8,
    /// Channels code (table 8: 1 mono, 2 two channels, 3 5.1).
    pub channels: u8,
    /// Language.
    pub language: String,
}

/// The sampling rate of a code (table 9).
pub fn sample_rate(code: u8) -> u32 {
    [
        8000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000, 96_000,
    ]
    .get(code as usize)
    .copied()
    .unwrap_or(0)
}

/// One multiplex sub-frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubFrame {
    /// Start time in 1/22 500 s.
    pub start: u32,
    /// Audio units: (stream, bytes).
    pub audio: Vec<(u8, Vec<u8>)>,
    /// Data units: (type, bytes).
    pub data: Vec<(u8, Vec<u8>)>,
    /// Audio streams.
    pub streams: Vec<AudioStream>,
}

/// One received sub-frame with its section CRC results.
#[derive(Clone, Debug, Default)]
pub struct RxSubFrame {
    /// The content (sections that failed are empty).
    pub sub: SubFrame,
    /// Audio section CRC good.
    pub audio_ok: bool,
    /// Data section CRC good.
    pub data_ok: bool,
}

fn section(units: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut s = vec![units.len() as u8];
    for (t, b) in units {
        s.push(*t);
        s.extend_from_slice(&(b.len() as u16).to_be_bytes());
        s.extend_from_slice(b);
    }
    let c = cdr_crc32(&s);
    s.extend_from_slice(&c.to_be_bytes());
    s
}

/// Units of a section: (type or stream, bytes).
type Units = Vec<(u8, Vec<u8>)>;

fn parse_section(b: &[u8]) -> Option<(Units, usize, bool)> {
    let n = *b.first()? as usize;
    let mut p = 1;
    let mut units = Vec::with_capacity(n);
    for _ in 0..n {
        let t = *b.get(p)?;
        let l = u16::from_be_bytes([*b.get(p + 1)?, *b.get(p + 2)?]) as usize;
        units.push((t, b.get(p + 3..p + 3 + l)?.to_vec()));
        p += 3 + l;
    }
    let crc = u32::from_be_bytes([*b.get(p)?, *b.get(p + 1)?, *b.get(p + 2)?, *b.get(p + 3)?]);
    let ok = cdr_crc32(&b[..p]) == crc;
    Some((units, p + 4, ok))
}

impl SubFrame {
    /// Bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = self.start.to_be_bytes().to_vec();
        b.push(0); // encapsulation: segment mode
        b.extend(section(&self.audio));
        b.extend(section(&self.data));
        b.push(self.streams.len() as u8);
        for s in &self.streams {
            b.push(s.algorithm);
            b.extend_from_slice(&s.bitrate.to_be_bytes());
            b.push(s.rate_code);
            b.push(s.channels);
            let l = s.language.as_bytes();
            for i in 0..3 {
                b.push(*l.get(i).unwrap_or(&b' '));
            }
        }
        b
    }
    /// Reads a sub-frame.
    pub fn parse(b: &[u8]) -> Option<RxSubFrame> {
        let start = u32::from_be_bytes([*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?]);
        let mut p = 5;
        let (audio, n, audio_ok) = parse_section(b.get(p..)?)?;
        p += n;
        let (data, n, data_ok) = parse_section(b.get(p..)?)?;
        p += n;
        let ns = *b.get(p)? as usize;
        p += 1;
        let mut streams = Vec::new();
        for _ in 0..ns {
            let s = b.get(p..p + 8)?;
            streams.push(AudioStream {
                algorithm: s[0],
                bitrate: u16::from_be_bytes([s[1], s[2]]),
                rate_code: s[3],
                channels: s[4],
                language: String::from_utf8_lossy(&s[5..8]).to_string(),
            });
            p += 8;
        }
        Some(RxSubFrame {
            sub: SubFrame {
                start,
                audio: if audio_ok { audio } else { Vec::new() },
                data: if data_ok { data } else { Vec::new() },
                streams,
            },
            audio_ok,
            data_ok,
        })
    }
}

/// A service multiplex frame: header and sub-frames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceFrame {
    /// Frame id.
    pub id: u8,
    /// Emergency.
    pub emergency: bool,
    /// NIT, SMCT, ESG update counters.
    pub updates: (u8, u8, u8),
    /// Sub-frames.
    pub subs: Vec<SubFrame>,
}

impl ServiceFrame {
    /// Bytes, `len` long (padded with ones).
    pub fn to_bytes(&self, len: usize) -> Option<Vec<u8>> {
        let subs: Vec<Vec<u8>> = self.subs.iter().map(|s| s.to_bytes()).collect();
        let mut h = vec![
            1,
            self.emergency as u8,
            self.id,
            self.updates.0,
            self.updates.1,
            self.updates.2,
            subs.len() as u8,
        ];
        for s in &subs {
            h.extend_from_slice(&(s.len() as u32).to_be_bytes()[1..]);
        }
        let c = cdr_crc8(&h);
        h.push(c);
        for s in subs {
            h.extend(s);
        }
        if h.len() > len {
            return None;
        }
        h.resize(len, 0xFF);
        Some(h)
    }
    /// The header bytes of `n` sub-frames.
    pub fn header_len(n: usize) -> usize {
        8 + 3 * n
    }
    /// Reads a frame: the header (or `None`), and each sub-frame.
    pub fn parse(b: &[u8]) -> Option<(ServiceFrame, Vec<Option<RxSubFrame>>)> {
        let n = *b.get(6)? as usize;
        let hl = 7 + 3 * n;
        if cdr_crc8(b.get(..hl)?) != *b.get(hl)? {
            return None;
        }
        let f = ServiceFrame {
            id: b[2],
            emergency: b[1] != 0,
            updates: (b[3], b[4], b[5]),
            subs: Vec::new(),
        };
        let mut p = hl + 1;
        let mut subs = Vec::with_capacity(n);
        for i in 0..n {
            let l = u32::from_be_bytes([0, b[7 + 3 * i], b[8 + 3 * i], b[9 + 3 * i]]) as usize;
            subs.push(b.get(p..p + l).and_then(SubFrame::parse));
            p += l;
        }
        Some((f, subs))
    }
}
