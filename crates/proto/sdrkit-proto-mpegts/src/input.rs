//! Byte-stream alignment: finds 188-, 192- (M2TS) and 204-byte (with Reed-Solomon parity) packets in a byte
//! stream of any cut, strips the extra bytes, counts resynchronisations and marks losses. RTP datagrams (payload type 33) have their
//! header removed and their sequence numbers checked.

use crate::packet::{SYNC, TS_SIZE};

/// One aligned packet as output by the aligner.
#[derive(Clone, Copy, Debug)]
pub struct Aligned<'a> {
    /// The 188 bytes.
    pub bytes: &'a [u8],
    /// Data was lost just before this packet (a resync, an RTP sequence gap).
    pub loss: bool,
}

/// The aligner's state. Feed it bytes with [`TsAligner::push`] (a stream) or [`TsAligner::push_datagram`] (UDP or RTP).
#[derive(Clone, Debug, Default)]
pub struct TsAligner {
    buf: Vec<u8>,
    /// The packet length in the stream (188, 192 or 204), 0 until recognised.
    stride: usize,
    loss: bool,
    /// Resynchronisations so far.
    pub resyncs: u64,
    /// Packets delivered.
    pub packets: u64,
    /// Bytes dropped while searching.
    pub dropped_bytes: u64,
    rtp_seq: Option<u16>,
    searching: bool,
    skip: usize,
    /// RTP sequence gaps seen.
    pub rtp_gaps: u64,
}

const STRIDES: [usize; 3] = [188, 192, 204];

impl TsAligner {
    /// A new aligner.
    pub fn new() -> Self {
        Self::default()
    }
    /// Forgets the alignment (a new stream).
    pub fn reset(&mut self) {
        let (r, p, d, g) = (
            self.resyncs,
            self.packets,
            self.dropped_bytes,
            self.rtp_gaps,
        );
        *self = TsAligner {
            resyncs: r,
            packets: p,
            dropped_bytes: d,
            rtp_gaps: g,
            ..Default::default()
        };
    }
    /// The packet length recognised (0 until then).
    pub fn stride(&self) -> usize {
        self.stride
    }
    /// Marks a loss on the next packet (the transport below lost data).
    pub fn mark_loss(&mut self) {
        self.loss = true;
    }

    /// One UDP datagram: plain transport packets, or RTP carrying them (payload type 33), whose header is removed.
    pub fn push_datagram(&mut self, d: &[u8], out: &mut impl FnMut(Aligned)) {
        if d.len() >= 12 && d[0] >> 6 == 2 && d[1] & 0x7F == 33 {
            let cc = (d[0] & 0x0F) as usize;
            let mut start = 12 + 4 * cc;
            if d[0] & 0x10 != 0 && d.len() >= start + 4 {
                let ext = u16::from_be_bytes([d[start + 2], d[start + 3]]) as usize;
                start += 4 + 4 * ext;
            }
            let seq = u16::from_be_bytes([d[2], d[3]]);
            if let Some(prev) = self.rtp_seq {
                if seq != prev.wrapping_add(1) {
                    self.rtp_gaps += 1;
                    self.loss = true;
                }
            }
            self.rtp_seq = Some(seq);
            let mut end = d.len();
            if d[0] & 0x20 != 0 {
                end = end.saturating_sub(*d.last().unwrap_or(&0) as usize);
            }
            if start < end {
                self.push(&d[start..end], out);
            }
        } else {
            self.push(d, out);
        }
    }

    /// Bytes of a stream, cut anywhere.
    pub fn push(&mut self, bytes: &[u8], out: &mut impl FnMut(Aligned)) {
        let mut bytes = bytes;
        if self.skip > 0 {
            let k = self.skip.min(bytes.len());
            bytes = &bytes[k..];
            self.skip -= k;
        }
        self.buf.extend_from_slice(bytes);
        let mut pos = 0usize;
        loop {
            let len = self.buf.len();
            if self.stride == 0 {
                match self.recognise(pos) {
                    Some((at, stride)) => {
                        self.dropped_bytes += (at - pos) as u64;
                        pos = at;
                        self.stride = stride;
                    }
                    None => {
                        // Nothing confirmed in the first 400 bytes: keep the tail (a stream may start in it).
                        let keep = (len - pos).min(2 * 204 + 4);
                        if len - pos >= 409 {
                            self.dropped_bytes += (len - pos - keep) as u64;
                            pos = len - keep;
                        }
                        break;
                    }
                }
            }
            let s = self.stride;
            if pos + TS_SIZE > len {
                break;
            }
            if self.buf[pos] == SYNC && pos + TS_SIZE == len {
                // The packet ends exactly at the end of the data so far: accepted; its extra bytes are skipped when they come.
                self.emit(pos, out);
                self.skip = s - TS_SIZE;
                pos = len;
                break;
            }
            if pos + s > len {
                break;
            }
            if self.buf[pos] == SYNC && (pos + s == len || self.buf[pos + s] == SYNC) {
                self.emit(pos, out);
                pos += s;
                continue;
            }
            // Lost sync: search for a sync byte confirmed by another one a packet later.
            if !self.searching {
                self.resyncs += 1;
                self.searching = true;
            }
            self.loss = true;
            match self.confirmed_sync(pos + 1, s) {
                Some(at) => {
                    self.dropped_bytes += (at - pos) as u64;
                    pos = at;
                }
                None => {
                    let keep = s.min(len - pos);
                    self.dropped_bytes += (len - pos - keep) as u64;
                    pos = len - keep;
                    break;
                }
            }
        }
        self.buf.drain(..pos.min(self.buf.len()));
    }

    /// The end of the stream: a short stream that was never confirmed is read as 188-byte packets from a sync byte at its start.
    pub fn finish(&mut self, out: &mut impl FnMut(Aligned)) {
        if self.stride == 0 && self.buf.first() == Some(&SYNC) {
            let buf = std::mem::take(&mut self.buf);
            for p in buf.as_chunks::<TS_SIZE>().0 {
                if p[0] == SYNC {
                    self.packets += 1;
                    out(Aligned {
                        bytes: p,
                        loss: std::mem::take(&mut self.loss),
                    });
                }
            }
        }
        self.buf.clear();
    }

    fn emit(&mut self, pos: usize, out: &mut impl FnMut(Aligned)) {
        self.packets += 1;
        self.searching = false;
        let loss = std::mem::take(&mut self.loss);
        out(Aligned {
            bytes: &self.buf[pos..pos + TS_SIZE],
            loss,
        });
    }

    fn confirmed_sync(&self, from: usize, s: usize) -> Option<usize> {
        let b = &self.buf;
        (from..b.len()).find(|&i| b[i] == SYNC && (i + s < b.len() && b[i + s] == SYNC))
    }

    fn recognise(&self, from: usize) -> Option<(usize, usize)> {
        let b = &self.buf[from..];
        // Wait for enough bytes to see two packets of the longest kind.
        if b.len() < 2 * 204 + 1 {
            return None;
        }
        for i in 0..b.len().min(400) {
            if b[i] != SYNC {
                continue;
            }
            for s in STRIDES {
                if i + s < b.len()
                    && b[i + s] == SYNC
                    && (i + 2 * s >= b.len() || b[i + 2 * s] == SYNC)
                {
                    return Some((from + i, s));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packets(n: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for i in 0..n {
            let mut p = [0u8; 188];
            p[0] = 0x47;
            p[1] = 0x01;
            p[4] = i as u8;
            v.extend_from_slice(&p);
        }
        v
    }

    fn run(a: &mut TsAligner, data: &[u8], cut: usize) -> Vec<(u8, bool)> {
        let mut got = Vec::new();
        for c in data.chunks(cut) {
            a.push(c, &mut |p| got.push((p.bytes[4], p.loss)));
        }
        got
    }

    #[test]
    fn plain_m2ts_and_204_streams_align_at_any_cut() {
        for stride in [188usize, 192, 204] {
            let mut data = Vec::new();
            for (i, p) in packets(40).chunks(188).enumerate() {
                if stride == 192 {
                    data.extend_from_slice(&(i as u32).to_be_bytes());
                }
                data.extend_from_slice(p);
                if stride == 204 {
                    data.extend_from_slice(&[0xAA; 16]);
                }
            }
            for cut in [1usize, 7, 188, 1000, 100_000] {
                let mut a = TsAligner::new();
                let got = run(&mut a, &data, cut);
                assert_eq!(got.len(), 40, "stride {stride} cut {cut}");
                assert!(got.iter().enumerate().all(|(i, g)| g.0 == i as u8 && !g.1));
                assert_eq!(a.stride(), stride);
            }
        }
    }

    #[test]
    fn a_stray_byte_and_a_gap_resync_once_each() {
        let p = packets(30);
        let mut data = p[..188 * 10].to_vec();
        data.push(0x00);
        data.extend_from_slice(&p[188 * 10..188 * 20]);
        data.extend_from_slice(&[0x11; 1000]);
        data.extend_from_slice(&p[188 * 20..]);
        let mut a = TsAligner::new();
        let got = run(&mut a, &data, 333);
        // The packet before each fault is not confirmed by a sync byte after it, so it is dropped.
        assert_eq!(got.len(), 28);
        assert_eq!(a.resyncs, 2);
        let losses: Vec<u8> = got.iter().filter(|g| g.1).map(|g| g.0).collect();
        assert_eq!(losses, vec![10, 20]);
    }

    #[test]
    fn rtp_header_is_removed_and_gaps_tagged() {
        let p = packets(21);
        let mut a = TsAligner::new();
        let mut got = Vec::new();
        for (k, chunk) in p.chunks(188 * 7).enumerate() {
            let seq: u16 = if k == 2 { 5 } else { k as u16 };
            let mut d = vec![
                0x80,
                33,
                (seq >> 8) as u8,
                seq as u8,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                1,
            ];
            d.extend_from_slice(chunk);
            a.push_datagram(&d, &mut |x| got.push((x.bytes[4], x.loss)));
        }
        assert_eq!(got.len(), 21);
        assert_eq!(a.rtp_gaps, 1);
        assert!(got[14].1);
        assert_eq!(got.iter().filter(|g| g.1).count(), 1);
    }
}
