//! NAL units of an H.264 or HEVC byte stream: the start code search and the removal of emulation prevention bytes.
//!
//! Standard: ITU-T H.264 Annex B.2 and clause 7.4.1, ITU-T H.265 Annex B.2 and clause 7.4.2.

/// The NAL units of an Annex B byte stream, as `(start, end)` byte ranges without their start codes. See [`NalUnits::new`].
#[derive(Clone, Debug)]
pub struct NalUnits<'a> {
    data: &'a [u8],
    next: Option<usize>,
}

/// The index just after the next start code (0x00 0x00 0x01) at or after `from`.
fn after_start_code(data: &[u8], from: usize) -> Option<usize> {
    let rest = data.get(from..)?;
    rest.windows(3)
        .position(|w| w == [0, 0, 1])
        .map(|i| from + i + 3)
}

impl<'a> NalUnits<'a> {
    /// The NAL units of `data`. Bytes before the first start code belong to no unit. The zero bytes before a start code
    /// (the leading zero of a four-byte start code, and trailing_zero_8bits) belong to no unit either (H.264 B.2).
    /// It allocates nothing.
    pub fn new(data: &'a [u8]) -> Self {
        NalUnits {
            data,
            next: after_start_code(data, 0),
        }
    }
}

impl Iterator for NalUnits<'_> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        let s = self.next?;
        let following = after_start_code(self.data, s);
        let mut e = following.map_or(self.data.len(), |n| n - 3);
        while e > s && self.data[e - 1] == 0 {
            e -= 1;
        }
        self.next = following;
        Some((s, e))
    }
}

/// The NAL units of an Annex B byte stream as `(start, end)` ranges, in a new `Vec`. [`NalUnits`] iterates over the same ranges
/// without allocating.
pub fn nal_units(data: &[u8]) -> Vec<(usize, usize)> {
    NalUnits::new(data).collect()
}

/// Appends the payload of NAL unit `nal` to `out`, with each emulation_prevention_three_byte removed: a 0x03 after two zero
/// bytes (H.264 clause 7.4.1). It allocates only when `out` must grow.
pub fn unescape_rbsp_into(nal: &[u8], out: &mut Vec<u8>) {
    out.reserve(nal.len());
    let mut zeros = 0;
    for &b in nal {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
}

/// The payload of NAL unit `nal` with the emulation prevention bytes removed, in a new `Vec` (see [`unescape_rbsp_into`]).
pub fn unescape_rbsp(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    unescape_rbsp_into(nal, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H.264 clause 7.4.1: within a NAL unit, 0x000003 stands for 0x0000 when the byte after it is 0x00, 0x01, 0x02 or 0x03.
    #[test]
    fn emulation_prevention_of_clause_7_4_1() {
        for b in 0..=3u8 {
            assert_eq!(
                unescape_rbsp(&[0x65, 0, 0, 3, b, 0x80]),
                [0x65, 0, 0, b, 0x80]
            );
        }
        assert_eq!(unescape_rbsp(&[0, 0, 3, 1, 0, 0, 3]), [0, 0, 1, 0, 0]);
        // Two escapes in a row: 00 00 03 00 00 03 00.
        assert_eq!(unescape_rbsp(&[0, 0, 3, 0, 0, 3, 0]), [0, 0, 0, 0, 0]);
        let mut v = vec![9];
        unescape_rbsp_into(&[0, 0, 3, 2], &mut v);
        assert_eq!(v, [9, 0, 0, 2]);
    }

    /// H.264 Annex B.2: three- and four-byte start codes, trailing zero bytes and bytes before the first start code.
    #[test]
    fn annex_b_start_codes() {
        let s = [0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3];
        assert_eq!(nal_units(&s), vec![(4, 7), (10, 12)]);
        let s = [0xFF, 0, 0, 1, 0x09, 0xF0, 0, 0, 0, 0, 1, 0x65, 0x88, 0, 0];
        assert_eq!(
            NalUnits::new(&s).collect::<Vec<_>>(),
            vec![(4, 6), (11, 13)]
        );
        assert_eq!(nal_units(&[1, 2, 3]), vec![]);
        assert_eq!(nal_units(&[0, 0, 1]), vec![(3, 3)]);
    }
}
