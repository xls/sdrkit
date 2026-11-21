//! Audio super frames (ES 201 980 clause 5.4.1) and text messages (clause 6.5).

use sdrkit_util_bits::crc::catalog::{CRC16_GENIBUS, CRC8_SAE_J1850};
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::{BitOrder, BitReader, BitWriter};

/// The CRC-8 of an AAC frame (clause 5.4.1, annex D): over the first `crc_bits` bits of `frame`, most significant bit first.
/// The CRC covers the side information and TNS bits only: (mono1, mono2) of a mono frame, (stereo1 to stereo5) of a stereo
/// frame. `None` when the frame has fewer than `crc_bits` bits.
pub fn aac_crc(frame: &[u8], crc_bits: usize) -> Option<u8> {
    if crc_bits > 8 * frame.len() {
        return None;
    }
    let mut bits = Vec::with_capacity(crc_bits.next_multiple_of(8));
    unpack(&frame[..crc_bits.div_ceil(8)], BitOrder::Msb, &mut bits);
    bits.truncate(crc_bits);
    Some(CRC8_SAE_J1850.bits(&bits))
}

/// One AAC frame of a super frame with the CRC the super frame sent for it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AacFrame {
    /// The frame's bytes.
    pub bytes: Vec<u8>,
    /// The `aac_crc_bits` the super frame sent for the frame.
    pub crc: u8,
}

impl AacFrame {
    /// Whether the sent CRC is the CRC of the first `crc_bits` bits of the frame ([`aac_crc`]).
    pub fn crc_ok(&self, crc_bits: usize) -> bool {
        aac_crc(&self.bytes, crc_bits) == Some(self.crc)
    }
}

/// An AAC super frame cut into its frames (table 10).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SuperFrame {
    /// The frames.
    pub frames: Vec<AacFrame>,
}

/// Builds an AAC audio super frame of exactly `len` bytes (tables 10 and 11): the frame borders, the CRC of each frame over its
/// first `crc_bits[i]` bits, then the frames, padded with zeros. `None` when the frames do not fit, when `crc_bits` has not one
/// entry per frame, or when a frame is shorter than its `crc_bits`.
pub fn build_superframe(frames: &[Vec<u8>], crc_bits: &[usize], len: usize) -> Option<Vec<u8>> {
    let n = frames.len();
    if n == 0 || crc_bits.len() != n {
        return None;
    }
    let mut out = Vec::with_capacity(len);
    let mut w = BitWriter::new(&mut out);
    let mut pos = 0usize;
    for f in &frames[..n - 1] {
        pos += f.len();
        w.put(pos as u64 & 0xFFF, 12);
    }
    w.align();
    w.finish();
    for (f, &c) in frames.iter().zip(crc_bits) {
        out.push(aac_crc(f, c)?);
    }
    for f in frames {
        out.extend_from_slice(f);
    }
    if out.len() > len {
        return None;
    }
    out.resize(len, 0);
    Some(out)
}

/// Bytes of the super frame header and CRCs for `n` frames.
pub fn overhead(n: usize) -> usize {
    (12 * (n - 1)).div_ceil(8) + n
}

/// Parses a super frame of `n` frames. `None` when the borders are not consistent (the whole super frame is then lost). The
/// CRCs are not checked here, because the bits they cover depend on the AAC syntax of each frame ([`AacFrame::crc_ok`]).
pub fn parse_superframe(b: &[u8], n: usize) -> Option<SuperFrame> {
    let head = overhead(n);
    if b.len() < head {
        return None;
    }
    let payload = b.len() - head;
    let mut rd = BitReader::new(b);
    let mut borders = Vec::with_capacity(n + 1);
    let mut last = 0usize;
    borders.push(last);
    for _ in 0..n - 1 {
        let v = rd.u(12) as usize;
        if v < last || v > payload {
            return None;
        }
        borders.push(v);
        last = v;
    }
    borders.push(payload);
    let crcs = &b[head - n..head];
    let data = &b[head..];
    let frames = (0..n)
        .map(|i| AacFrame {
            bytes: data[borders[i]..borders[i + 1]].to_vec(),
            crc: crcs[i],
        })
        .collect();
    Some(SuperFrame { frames })
}

/// The text message transmitter: segments of up to 16 bytes, sent as 4-byte units (one per logical frame).
#[derive(Clone, Debug)]
pub struct TextSender {
    units: Vec<[u8; 4]>,
    pos: usize,
}

impl TextSender {
    /// A sender of `text` with `toggle`.
    pub fn new(text: &str, toggle: bool) -> TextSender {
        let b = text.as_bytes();
        let b = &b[..b.len().min(128)];
        let segs: Vec<&[u8]> = b.chunks(16).collect();
        let nseg = segs.len().max(1);
        let mut units = Vec::new();
        for (i, s) in segs.iter().enumerate() {
            let first = i == 0;
            let last = i == nseg - 1;
            let mut seg = vec![0xFF, 0xFF, 0xFF, 0xFF];
            let h0 = (toggle as u8) << 7
                | (first as u8) << 6
                | (last as u8) << 5
                | ((s.len() - 1) as u8 & 15);
            let h1 = if first { 0 } else { ((i as u8) & 7) << 4 };
            let mut body = vec![h0, h1];
            body.extend_from_slice(s);
            let crc = CRC16_GENIBUS.checksum(&body);
            seg.extend_from_slice(&body);
            seg.extend_from_slice(&crc.to_be_bytes());
            while seg.len() % 4 != 0 {
                seg.push(0);
            }
            for c in seg.chunks(4) {
                units.push([c[0], c[1], c[2], c[3]]);
            }
        }
        TextSender { units, pos: 0 }
    }
    /// The next 4-byte unit.
    pub fn next_unit(&mut self) -> [u8; 4] {
        if self.units.is_empty() {
            return [0; 4];
        }
        let u = self.units[self.pos % self.units.len()];
        self.pos += 1;
        u
    }
}

/// The text message receiver.
#[derive(Clone, Debug, Default)]
pub struct TextReceiver {
    buf: Vec<u8>,
    in_seg: bool,
    segs: [Option<Vec<u8>>; 8],
    toggle: Option<bool>,
    last_seg: Option<usize>,
    /// The message, cleaned for display.
    pub message: Option<String>,
    /// Segments with a good CRC.
    pub good: u64,
    /// Segments with a bad CRC.
    pub bad: u64,
}

impl TextReceiver {
    /// Takes one 4-byte unit. Returns true when the message changed.
    pub fn push(&mut self, u: &[u8]) -> bool {
        if u == [0xFF, 0xFF, 0xFF, 0xFF] {
            self.buf.clear();
            self.in_seg = true;
            return false;
        }
        if !self.in_seg {
            return false;
        }
        self.buf.extend_from_slice(u);
        if self.buf.len() < 2 {
            return false;
        }
        let len = (self.buf[0] & 15) as usize + 1;
        let need = 2 + len + 2;
        if self.buf.len() < need {
            return false;
        }
        self.in_seg = false;
        let body = &self.buf[..2 + len];
        let crc = u16::from_be_bytes([self.buf[2 + len], self.buf[3 + len]]);
        if CRC16_GENIBUS.checksum(body) != crc {
            self.bad += 1;
            return false;
        }
        self.good += 1;
        let toggle = body[0] & 0x80 != 0;
        let first = body[0] & 0x40 != 0;
        let last = body[0] & 0x20 != 0;
        let num = if first {
            0
        } else {
            ((body[1] >> 4) & 7) as usize
        };
        if self.toggle != Some(toggle) {
            self.segs = Default::default();
            self.toggle = Some(toggle);
            self.last_seg = None;
        }
        self.segs[num] = Some(body[2..].to_vec());
        if last {
            self.last_seg = Some(num);
        }
        if let Some(l) = self.last_seg {
            if self.segs[..=l].iter().all(|s| s.is_some()) {
                let mut all = Vec::new();
                for s in self.segs[..=l].iter().flatten() {
                    all.extend_from_slice(s);
                }
                let text = clean_text(&all);
                if self.message.as_deref() != Some(&text) {
                    self.message = Some(text);
                    return true;
                }
            }
        }
        false
    }
}

/// 0x0A and 0x0B become a line break, other control codes are dropped, invalid UTF-8 becomes U+FFFD.
pub fn clean_text(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    s.chars()
        .filter_map(|c| match c {
            '\n' | '\u{0B}' => Some('\n'),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superframe_round_trip_and_corrupt_borders() {
        let frames: Vec<Vec<u8>> = (0..10).map(|i| vec![i as u8 + 1; 30 + i]).collect();
        let total = overhead(10) + frames.iter().map(|f| f.len()).sum::<usize>();
        let sf = build_superframe(&frames, &[45; 10], total).unwrap();
        let p = parse_superframe(&sf, 10).unwrap();
        for (a, b) in p.frames.iter().zip(&frames) {
            assert_eq!(&a.bytes, b);
            assert!(a.crc_ok(45));
        }
        let mut bad = sf.clone();
        bad[0] = 0xFF; // first border beyond the payload
        bad[1] = 0xF0;
        assert!(parse_superframe(&bad, 10).is_none());
        // A flip inside the covered bits fails the CRC. A flip after them does not.
        let head = overhead(10);
        let mut crc_bad = sf.clone();
        crc_bad[head + 3] ^= 1;
        crc_bad[head + 30 + 10] ^= 1;
        let p = parse_superframe(&crc_bad, 10).unwrap();
        assert!(!p.frames[0].crc_ok(45) && p.frames[1].crc_ok(45));
    }

    #[test]
    fn text_round_trip_with_toggle() {
        let mut tx = TextSender::new("DRM test signal\x0bline two", false);
        let mut rx = TextReceiver::default();
        let mut changed = false;
        for _ in 0..40 {
            changed |= rx.push(&tx.next_unit());
        }
        assert!(changed);
        assert_eq!(rx.message.as_deref(), Some("DRM test signal\nline two"));
        assert!(rx.good >= 2 && rx.bad == 0);
        let mut tx2 = TextSender::new("New", true);
        for _ in 0..12 {
            rx.push(&tx2.next_unit());
        }
        assert_eq!(rx.message.as_deref(), Some("New"));
    }
}
