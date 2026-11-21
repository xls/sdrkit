//! Transport packets (ISO/IEC 13818-1 2.4.3.2): the header, the adaptation field and the payload.

/// Bytes in a transport packet.
pub const TS_SIZE: usize = 188;
/// The sync byte.
pub const SYNC: u8 = 0x47;
/// The PID of null packets.
pub const NULL_PID: u16 = 0x1FFF;

/// A packet header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// transport_error_indicator.
    pub tei: bool,
    /// payload_unit_start_indicator.
    pub pusi: bool,
    /// transport_priority.
    pub priority: bool,
    /// The PID.
    pub pid: u16,
    /// transport_scrambling_control (0 clear).
    pub scrambling: u8,
    /// adaptation_field_control: 1 payload only, 2 adaptation only, 3 both.
    pub afc: u8,
    /// continuity_counter.
    pub cc: u8,
}

impl Header {
    /// Whether the packet carries a payload.
    pub fn has_payload(&self) -> bool {
        self.afc & 1 != 0
    }
    /// Whether the packet carries an adaptation field.
    pub fn has_adaptation(&self) -> bool {
        self.afc & 2 != 0
    }
}

/// The header of a packet (`None` when it is shorter than 4 bytes or does not start with the sync byte).
pub fn header(p: &[u8]) -> Option<Header> {
    if p.len() < 4 || p[0] != SYNC {
        return None;
    }
    Some(Header {
        tei: p[1] & 0x80 != 0,
        pusi: p[1] & 0x40 != 0,
        priority: p[1] & 0x20 != 0,
        pid: ((p[1] as u16 & 0x1F) << 8) | p[2] as u16,
        scrambling: p[3] >> 6,
        afc: (p[3] >> 4) & 3,
        cc: p[3] & 0x0F,
    })
}

/// The adaptation field fields sdrkit uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Adaptation {
    /// discontinuity_indicator.
    pub discontinuity: bool,
    /// random_access_indicator.
    pub random_access: bool,
    /// The PCR in 27 MHz units (base × 300 + extension).
    pub pcr: Option<u64>,
}

/// A packet taken apart: header, adaptation field and payload.
#[derive(Clone, Copy, Debug)]
pub struct Parsed<'a> {
    /// The header.
    pub header: Header,
    /// The adaptation field, when present.
    pub adaptation: Option<Adaptation>,
    /// The payload (empty when there is none).
    pub payload: &'a [u8],
}

/// Takes a 188-byte packet apart. `None` when the sync byte is wrong.
pub fn parse(p: &[u8]) -> Option<Parsed<'_>> {
    let h = header(p)?;
    let mut start = 4usize;
    let mut adaptation = None;
    if h.has_adaptation() {
        let len = *p.get(4)? as usize;
        start = 5 + len;
        if len > 0 && p.len() > 5 {
            let flags = p[5];
            let mut a = Adaptation {
                discontinuity: flags & 0x80 != 0,
                random_access: flags & 0x40 != 0,
                pcr: None,
            };
            if flags & 0x10 != 0 && len >= 7 && p.len() >= 12 {
                let base = ((p[6] as u64) << 25)
                    | ((p[7] as u64) << 17)
                    | ((p[8] as u64) << 9)
                    | ((p[9] as u64) << 1)
                    | (p[10] as u64 >> 7);
                let ext = (((p[10] & 1) as u64) << 8) | p[11] as u64;
                a.pcr = Some(base * 300 + ext);
            }
            adaptation = Some(a);
        } else {
            adaptation = Some(Adaptation::default());
        }
    }
    let payload = if h.has_payload() && start < p.len().min(TS_SIZE) {
        &p[start..p.len().min(TS_SIZE)]
    } else {
        &p[0..0]
    };
    Some(Parsed {
        header: h,
        adaptation,
        payload,
    })
}

/// Seconds of a 27 MHz PCR value.
pub fn pcr_seconds(pcr: u64) -> f64 {
    (pcr / 300) as f64 / 90_000.0 + (pcr % 300) as f64 / 27_000_000.0
}

/// Writes a packet header into `p[0..4]`.
pub fn write_header(p: &mut [u8], h: &Header) {
    p[0] = SYNC;
    p[1] = (h.tei as u8) << 7
        | (h.pusi as u8) << 6
        | (h.priority as u8) << 5
        | ((h.pid >> 8) as u8 & 0x1F);
    p[2] = h.pid as u8;
    p[3] = (h.scrambling & 3) << 6 | (h.afc & 3) << 4 | (h.cc & 0x0F);
}

/// Splits a section (or several, back to back) into packets of `pid`, the first with PUSI and a pointer field of 0, padded with
/// 0xFF. `cc` is the counter of the first packet and is advanced.
pub fn packetize_section(section: &[u8], pid: u16, cc: &mut u8) -> Vec<[u8; TS_SIZE]> {
    let mut out = Vec::new();
    let mut rest = section;
    let mut first = true;
    while first || !rest.is_empty() {
        let mut p = [0xFFu8; TS_SIZE];
        write_header(
            &mut p,
            &Header {
                pusi: first,
                pid,
                afc: 1,
                cc: *cc,
                ..Default::default()
            },
        );
        *cc = (*cc + 1) & 15;
        let mut at = 4;
        if first {
            p[4] = 0;
            at = 5;
        }
        let n = rest.len().min(TS_SIZE - at);
        p[at..at + n].copy_from_slice(&rest[..n]);
        rest = &rest[n..];
        out.push(p);
        first = false;
    }
    out
}

/// Splits a PES packet into packets of `pid`, the first with PUSI. The last packet is filled up with adaptation field stuffing
/// (ISO/IEC 13818-1 2.4.3.5). `cc` is the counter of the first packet and is advanced.
pub fn packetize_pes(pes: &[u8], pid: u16, cc: &mut u8) -> Vec<[u8; TS_SIZE]> {
    let mut out = Vec::with_capacity(pes.len().div_ceil(TS_SIZE - 4).max(1));
    for (k, chunk) in pes.chunks(TS_SIZE - 4).enumerate() {
        let mut p = [0xFFu8; TS_SIZE];
        let stuff = TS_SIZE - 4 - chunk.len();
        write_header(
            &mut p,
            &Header {
                pusi: k == 0,
                pid,
                afc: if stuff > 0 { 3 } else { 1 },
                cc: *cc,
                ..Default::default()
            },
        );
        *cc = (*cc + 1) & 15;
        if stuff > 0 {
            // adaptation_field_length, then the flags byte (all 0) and 0xFF stuffing.
            p[4] = (stuff - 1) as u8;
            if stuff > 1 {
                p[5] = 0;
            }
        }
        p[4 + stuff..].copy_from_slice(chunk);
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_and_pcr() {
        let mut p = [0u8; TS_SIZE];
        write_header(
            &mut p,
            &Header {
                pusi: true,
                pid: 0x1234,
                afc: 3,
                cc: 7,
                ..Default::default()
            },
        );
        p[4] = 7;
        p[5] = 0x10;
        // PCR base 1 second = 90000, extension 5.
        let base: u64 = 90_000;
        p[6] = (base >> 25) as u8;
        p[7] = (base >> 17) as u8;
        p[8] = (base >> 9) as u8;
        p[9] = (base >> 1) as u8;
        p[10] = ((base & 1) << 7) as u8 | 0x7E;
        p[11] = 5;
        let q = parse(&p).unwrap();
        assert_eq!(q.header.pid, 0x1234);
        assert!(q.header.pusi);
        assert_eq!(q.header.cc, 7);
        assert_eq!(q.adaptation.unwrap().pcr, Some(90_000 * 300 + 5));
        assert_eq!(q.payload.len(), TS_SIZE - 12);
        assert!((pcr_seconds(90_000 * 300) - 1.0).abs() < 1e-12);
    }
}
