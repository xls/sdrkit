//! PES assembly, and the PTS and DTS of the header unwrapped onto a 64-bit timeline.
//!
//! Standard: ISO/IEC 13818-1 2.4.3.6, 2.4.3.7.

use sdrkit_graph::{Packet, PacketPool};

/// The largest PES assembled; a larger one is discarded.
pub const MAX_PES: usize = 4 << 20;

/// 2^33: the wrap of a 90 kHz timestamp.
pub const WRAP_33: i64 = 1 << 33;

/// A PES packet as output by the demultiplexer.
#[derive(Clone, Debug)]
pub struct PesPacket {
    /// The PID it came on.
    pub pid: u16,
    /// stream_id.
    pub stream_id: u8,
    /// PTS (33 bits, as sent).
    pub pts: Option<i64>,
    /// DTS (33 bits, as sent).
    pub dts: Option<i64>,
    /// data_alignment_indicator.
    pub aligned: bool,
    /// The payload after the PES header.
    pub payload: Packet,
    /// Some of its packets were lost or had errors.
    pub damaged: bool,
    /// A discontinuity (the stream clock or the counters) came before it.
    pub discontinuity: bool,
    /// random_access_indicator was set on its first packet.
    pub random_access: bool,
    /// Arrival time of its first packet (signal seconds).
    pub time: f64,
    /// What the PMT says the stream is (set by the demultiplexer when it routes the PES on).
    pub info: Option<crate::codec::CodecInfo>,
    /// The DVB AAC descriptor's profile_and_level, for HE-AAC signalling.
    pub aac_profile_level: Option<u8>,
    /// For subtitles: the composition and ancillary pages chosen.
    pub pages: Option<(u16, u16)>,
    /// The system time clock at its arrival (seconds), when the service has a PCR.
    pub stc: Option<f64>,
}

impl sdrkit_graph::Item for PesPacket {
    const KIND: &'static str = "mpeg.pes";
}

/// A 33-bit timestamp from the 5 bytes of a PES header field.
pub fn timestamp(b: &[u8]) -> Option<i64> {
    if b.len() < 5 {
        return None;
    }
    let v = ((b[0] as i64 >> 1) & 7) << 30
        | (b[1] as i64) << 22
        | ((b[2] as i64) >> 1) << 15
        | (b[3] as i64) << 7
        | (b[4] as i64) >> 1;
    Some(v)
}

/// Writes a 33-bit timestamp field with the given 4-bit prefix (0x2 PTS only, 0x3 PTS of PTS+DTS, 0x1 DTS).
pub fn write_timestamp(prefix: u8, t: i64, out: &mut Vec<u8>) {
    let t = t.rem_euclid(WRAP_33);
    out.push(prefix << 4 | ((t >> 29) as u8 & 0x0E) | 1);
    out.push((t >> 22) as u8);
    out.push(((t >> 14) as u8 & 0xFE) | 1);
    out.push((t >> 7) as u8);
    out.push(((t << 1) as u8) | 1);
}

/// A parsed PES header: (stream_id, PTS, DTS, data_alignment, header length).
pub type PesHeader = (u8, Option<i64>, Option<i64>, bool, usize);

/// The parsed header of a PES packet.
pub fn parse_header(b: &[u8]) -> Option<PesHeader> {
    if b.len() < 6 || b[0] != 0 || b[1] != 0 || b[2] != 1 {
        return None;
    }
    let sid = b[3];
    // Streams without the optional header.
    if matches!(sid, 0xBC | 0xBE | 0xBF | 0xF0 | 0xF1 | 0xF2 | 0xF8 | 0xFF) {
        return Some((sid, None, None, false, 6));
    }
    if b.len() < 9 || b[6] & 0xC0 != 0x80 {
        return Some((sid, None, None, false, 6.min(b.len())));
    }
    let aligned = b[6] & 0x04 != 0;
    let flags = b[7] >> 6;
    let hl = b[8] as usize;
    let mut pts = None;
    let mut dts = None;
    if flags & 2 != 0 && hl >= 5 {
        pts = timestamp(&b[9..]);
        if flags == 3 && hl >= 10 {
            dts = timestamp(&b[14..]);
        }
    }
    Some((sid, pts, dts, aligned, 9 + hl))
}

/// Unwraps 33-bit timestamps onto a 64-bit timeline: a value smaller than the last by more than 2^32 is a wrap.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unwrapper {
    base: i64,
    last: Option<i64>,
}

impl Unwrapper {
    /// The unwrapped value of `v` (33 bits).
    pub fn unwrap(&mut self, v: i64) -> i64 {
        let mut u = self.base + v;
        if let Some(l) = self.last {
            if u < l - (1 << 32) {
                self.base += WRAP_33;
                u += WRAP_33;
            } else if u > l + (1 << 32) && self.base >= WRAP_33 {
                self.base -= WRAP_33;
                u -= WRAP_33;
            }
        }
        self.last = Some(u);
        u
    }
    /// Forgets the timeline.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The difference `a - b` on the 33-bit circle (in -2^32 .. 2^32).
pub fn circle_diff(a: i64, b: i64) -> i64 {
    let d = (a - b).rem_euclid(WRAP_33);
    if d >= 1 << 32 {
        d - WRAP_33
    } else {
        d
    }
}

/// Assembles PES packets from the payloads of one PID. The payloads come from a pool, so nothing is allocated once the pool
/// has grown and the earlier payloads are dropped.
#[derive(Debug, Default)]
pub struct PesAssembler {
    buf: Vec<u8>,
    pool: PacketPool,
    active: bool,
    damaged: bool,
    discontinuity: bool,
    random_access: bool,
    time: f64,
    /// PES packets discarded as too large.
    pub too_large: u64,
}

impl PesAssembler {
    /// Marks the PES in progress damaged (a lost or bad packet).
    pub fn damage(&mut self) {
        if self.active {
            self.damaged = true;
        }
    }
    /// Drops the PES in progress.
    pub fn drop_partial(&mut self) {
        self.active = false;
        self.buf.clear();
    }
    /// Marks a discontinuity on the next PES.
    pub fn discontinuity(&mut self) {
        self.discontinuity = true;
    }
    /// One packet's payload. `out` gets every completed PES.
    pub fn push(
        &mut self,
        pid: u16,
        payload: &[u8],
        pusi: bool,
        random_access: bool,
        time: f64,
        out: &mut impl FnMut(PesPacket),
    ) {
        if pusi {
            if self.active {
                self.finish(pid, out);
            }
            self.active = true;
            self.buf.clear();
            self.random_access = random_access;
            self.time = time;
        } else if !self.active {
            return;
        }
        if self.buf.len() + payload.len() > MAX_PES {
            self.too_large += 1;
            self.active = false;
            self.buf.clear();
            self.damaged = false;
            return;
        }
        self.buf.extend_from_slice(payload);
        // A bounded PES ends when its length is reached.
        if self.buf.len() >= 6 {
            let len = u16::from_be_bytes([self.buf[4], self.buf[5]]) as usize;
            if len > 0 && self.buf.len() >= len + 6 {
                self.buf.truncate(len + 6);
                self.finish(pid, out);
            }
        }
    }
    /// Ends the PES in progress (end of stream).
    pub fn flush(&mut self, pid: u16, out: &mut impl FnMut(PesPacket)) {
        if self.active {
            self.finish(pid, out);
        }
    }
    fn finish(&mut self, pid: u16, out: &mut impl FnMut(PesPacket)) {
        self.active = false;
        let damaged = std::mem::take(&mut self.damaged);
        let disc = std::mem::take(&mut self.discontinuity);
        let Some((sid, pts, dts, aligned, hl)) = parse_header(&self.buf) else {
            self.buf.clear();
            return;
        };
        let body = &self.buf[hl.min(self.buf.len())..];
        let payload = self
            .pool
            .make(body.len(), self.time, |b| b.copy_from_slice(body));
        self.buf.clear();
        out(PesPacket {
            pid,
            stream_id: sid,
            pts,
            dts,
            aligned,
            payload,
            damaged,
            discontinuity: disc,
            random_access: self.random_access,
            time: self.time,
            info: None,
            aac_profile_level: None,
            pages: None,
            stc: None,
        });
    }
}

/// Builds a PES packet (for generators and tests).
pub fn build_pes(
    stream_id: u8,
    pts: Option<i64>,
    dts: Option<i64>,
    payload: &[u8],
    bounded: bool,
) -> Vec<u8> {
    let mut h = Vec::new();
    match (pts, dts) {
        (Some(p), Some(d)) => {
            write_timestamp(3, p, &mut h);
            write_timestamp(1, d, &mut h);
        }
        (Some(p), None) => write_timestamp(2, p, &mut h),
        _ => {}
    }
    let flags = match (pts, dts) {
        (Some(_), Some(_)) => 0xC0,
        (Some(_), None) => 0x80,
        _ => 0,
    };
    let mut v = vec![0, 0, 1, stream_id, 0, 0, 0x84, flags, h.len() as u8];
    v.extend_from_slice(&h);
    v.extend_from_slice(payload);
    let len = v.len() - 6;
    if bounded && len <= 0xFFFF {
        v[4] = (len >> 8) as u8;
        v[5] = len as u8;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_round_trip_and_unwrap() {
        let mut v = Vec::new();
        write_timestamp(2, WRAP_33 - 9000, &mut v);
        assert_eq!(timestamp(&v), Some(WRAP_33 - 9000));
        let mut u = Unwrapper::default();
        let a = u.unwrap(WRAP_33 - 9000);
        let b = u.unwrap(9000);
        assert_eq!(b - a, 18_000);
        assert_eq!(circle_diff(9000, WRAP_33 - 9000), 18_000);
        assert_eq!(circle_diff(WRAP_33 - 9000, 9000), -18_000);
    }

    #[test]
    fn pes_across_packets() {
        let pes = build_pes(0xE0, Some(1000), Some(900), &[7u8; 400], false);
        let mut a = PesAssembler::default();
        let mut got = Vec::new();
        for (i, c) in pes.chunks(184).enumerate() {
            a.push(0x100, c, i == 0, i == 0, 0.0, &mut |p| got.push(p));
        }
        assert!(got.is_empty());
        a.push(
            0x100,
            &build_pes(0xE0, Some(4600), None, &[1], false),
            true,
            false,
            0.1,
            &mut |p| got.push(p),
        );
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].pts, got[0].dts), (Some(1000), Some(900)));
        assert_eq!(got[0].payload.len, 400);
        assert!(got[0].random_access);
    }
}
