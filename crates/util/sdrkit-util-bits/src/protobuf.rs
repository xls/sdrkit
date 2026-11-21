//! The protobuf wire format (protobuf.dev, "Encoding"): varints, zigzag, fixed32 and fixed64, length-delimited fields.
//!
//! [`Fields`] iterates over the fields of a message without allocating; unknown fields are simply the ones the caller does not
//! match. [`Writer`] appends fields to a caller's `Vec<u8>`.
//!
//! ```
//! use sdrkit_util_bits::protobuf::{Fields, Wire, Writer};
//! let mut buf = Vec::new();
//! let mut w = Writer::new(&mut buf);
//! w.uint(1, 150);
//! w.string(2, "testing");
//! assert_eq!(buf, [0x08, 0x96, 0x01, 0x12, 0x07, b't', b'e', b's', b't', b'i', b'n', b'g']);
//! let f: Vec<_> = Fields::new(&buf).collect::<Result<_, _>>().unwrap();
//! assert_eq!(f[0], (1, Wire::Varint(150)));
//! assert_eq!(f[1].1.bytes(), b"testing");
//! ```

use crate::Malformation;
use crate::{Error, Result};

/// A field's value as read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wire<'a> {
    /// Wire type 0.
    Varint(u64),
    /// Wire type 1.
    Fixed64(u64),
    /// Wire type 2.
    Bytes(&'a [u8]),
    /// Wire type 5.
    Fixed32(u32),
}

impl<'a> Wire<'a> {
    /// As an unsigned number (varint or fixed); 0 for bytes.
    pub fn u64(&self) -> u64 {
        match *self {
            Wire::Varint(v) | Wire::Fixed64(v) => v,
            Wire::Fixed32(v) => v as u64,
            Wire::Bytes(_) => 0,
        }
    }
    /// As an `int32` or `int64` (a negative value is a 10-byte varint in two's complement).
    pub fn i64(&self) -> i64 {
        self.u64() as i64
    }
    /// As an `int32`.
    pub fn i32(&self) -> i32 {
        self.u64() as i64 as i32
    }
    /// As an `sint32` or `sint64` (zigzag).
    pub fn sint(&self) -> i64 {
        unzigzag(self.u64())
    }
    /// As an `sfixed32`.
    pub fn sfixed32(&self) -> i32 {
        self.u64() as u32 as i32
    }
    /// As an `sfixed64`.
    pub fn sfixed64(&self) -> i64 {
        self.u64() as i64
    }
    /// As a `float` (fixed32).
    pub fn f32(&self) -> f32 {
        f32::from_bits(self.u64() as u32)
    }
    /// As a `double` (fixed64).
    pub fn f64(&self) -> f64 {
        f64::from_bits(self.u64())
    }
    /// As a `bool`.
    pub fn bool(&self) -> bool {
        self.u64() != 0
    }
    /// The bytes of a length-delimited field (empty for other types).
    pub fn bytes(&self) -> &'a [u8] {
        match *self {
            Wire::Bytes(b) => b,
            _ => &[],
        }
    }
    /// The text of a length-delimited field, when it is valid UTF-8.
    pub fn str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.bytes()).ok()
    }
}

/// Zigzag encoding of a signed value (sint32, sint64): 0, -1, 1, -2 ... become 0, 1, 2, 3 ...
#[inline]
pub const fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

/// Zigzag decoding.
#[inline]
pub const fn unzigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

/// Reads one varint at `*at` and moves past it. An error when it runs past the end or is longer than 10 bytes or 64 bits.
pub fn read_varint(b: &[u8], at: &mut usize) -> Result<u64> {
    let mut v: u64 = 0;
    for shift in (0..70).step_by(7) {
        let c = *b.get(*at).ok_or(Error::InputTooShort)?;
        *at += 1;
        if shift == 63 && c > 1 {
            return Err(Error::Malformed(Malformation::VarintOverflow));
        }
        v |= ((c & 0x7F) as u64) << shift;
        if c & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(Error::Malformed(Malformation::VarintLength))
}

/// The fields of a message, in order, as an iterator of `(field number, value)`. A malformed message ends with one error item.
#[derive(Clone, Debug)]
pub struct Fields<'a> {
    b: &'a [u8],
    at: usize,
    broken: bool,
}

impl<'a> Fields<'a> {
    /// The fields of `b`.
    pub fn new(b: &'a [u8]) -> Self {
        Fields {
            b,
            at: 0,
            broken: false,
        }
    }

    fn next_field(&mut self) -> Result<(u32, Wire<'a>)> {
        let key = read_varint(self.b, &mut self.at)?;
        let num = key >> 3;
        if num == 0 || num > (1 << 29) - 1 {
            return Err(Error::Malformed(Malformation::FieldNumber));
        }
        let take = |at: &mut usize, n: usize| -> Result<&'a [u8]> {
            let end = at.checked_add(n).ok_or(Error::InputTooShort)?;
            let s = self.b.get(*at..end).ok_or(Error::InputTooShort)?;
            *at = end;
            Ok(s)
        };
        let w = match key & 7 {
            0 => Wire::Varint(read_varint(self.b, &mut self.at)?),
            1 => {
                let s = take(&mut self.at, 8)?;
                Wire::Fixed64(u64::from_le_bytes([
                    s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
                ]))
            }
            2 => {
                let n = read_varint(self.b, &mut self.at)?;
                let n = usize::try_from(n).map_err(|_| Error::InputTooShort)?;
                Wire::Bytes(take(&mut self.at, n)?)
            }
            5 => {
                let s = take(&mut self.at, 4)?;
                Wire::Fixed32(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
            }
            _ => return Err(Error::Malformed(Malformation::WireType)),
        };
        Ok((num as u32, w))
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u32, Wire<'a>)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.broken || self.at >= self.b.len() {
            return None;
        }
        let r = self.next_field();
        if r.is_err() {
            self.broken = true;
        }
        Some(r)
    }
}

/// The last value of field `num` in a message (protobuf's rule for a repeated non-repeated field), or `None`. An error for a
/// malformed message.
pub fn get(msg: &[u8], num: u32) -> Result<Option<Wire<'_>>> {
    let mut last = None;
    for f in Fields::new(msg) {
        let (n, w) = f?;
        if n == num {
            last = Some(w);
        }
    }
    Ok(last)
}

/// The varints of a packed repeated field's bytes.
pub fn packed_varints(b: &[u8]) -> impl Iterator<Item = Result<u64>> + '_ {
    let mut at = 0;
    let mut broken = false;
    std::iter::from_fn(move || {
        if broken || at >= b.len() {
            return None;
        }
        let r = read_varint(b, &mut at);
        broken = r.is_err();
        Some(r)
    })
}

/// The values of a packed repeated `fixed32` field's bytes (a trailing partial value is left out).
pub fn packed_fixed32(b: &[u8]) -> impl Iterator<Item = u32> + '_ {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}

/// The values of a packed repeated `fixed64` field's bytes.
pub fn packed_fixed64(b: &[u8]) -> impl Iterator<Item = u64> + '_ {
    b.as_chunks::<8>()
        .0
        .iter()
        .map(|c| u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
}

/// Appends fields to a caller's `Vec<u8>`.
#[derive(Debug)]
pub struct Writer<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    /// A writer appending to `out`.
    pub fn new(out: &'a mut Vec<u8>) -> Self {
        Writer { out }
    }

    /// Appends a varint.
    pub fn varint(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                self.out.push(b);
                return;
            }
            self.out.push(b | 0x80);
        }
    }

    fn key(&mut self, num: u32, wt: u8) {
        self.varint(((num as u64) << 3) | wt as u64);
    }

    /// A varint field (`uint32`, `uint64`, enums).
    pub fn uint(&mut self, num: u32, v: u64) -> &mut Self {
        self.key(num, 0);
        self.varint(v);
        self
    }

    /// An `int32` or `int64` field (negative values take 10 bytes).
    pub fn int(&mut self, num: u32, v: i64) -> &mut Self {
        self.uint(num, v as u64)
    }

    /// A `sint32` or `sint64` field (zigzag).
    pub fn sint(&mut self, num: u32, v: i64) -> &mut Self {
        self.uint(num, zigzag(v))
    }

    /// A `bool` field.
    pub fn bool(&mut self, num: u32, v: bool) -> &mut Self {
        self.uint(num, v as u64)
    }

    /// A `fixed32` or `sfixed32` field.
    pub fn fixed32(&mut self, num: u32, v: u32) -> &mut Self {
        self.key(num, 5);
        self.out.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// A `fixed64` or `sfixed64` field.
    pub fn fixed64(&mut self, num: u32, v: u64) -> &mut Self {
        self.key(num, 1);
        self.out.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// A `float` field.
    pub fn float(&mut self, num: u32, v: f32) -> &mut Self {
        self.fixed32(num, v.to_bits())
    }

    /// A `double` field.
    pub fn double(&mut self, num: u32, v: f64) -> &mut Self {
        self.fixed64(num, v.to_bits())
    }

    /// A length-delimited `bytes` field.
    pub fn bytes(&mut self, num: u32, b: &[u8]) -> &mut Self {
        self.key(num, 2);
        self.varint(b.len() as u64);
        self.out.extend_from_slice(b);
        self
    }

    /// A `string` field.
    pub fn string(&mut self, num: u32, s: &str) -> &mut Self {
        self.bytes(num, s.as_bytes())
    }

    /// A packed repeated varint field.
    pub fn packed_varints(&mut self, num: u32, vs: &[u64]) -> &mut Self {
        self.key(num, 2);
        let len: usize = vs.iter().map(|&v| varint_len(v)).sum();
        self.varint(len as u64);
        for &v in vs {
            self.varint(v);
        }
        self
    }

    /// An embedded message written by `f`; its length is put in front afterwards (the bytes are moved once).
    pub fn message(&mut self, num: u32, f: impl FnOnce(&mut Writer)) -> &mut Self {
        self.key(num, 2);
        let start = self.out.len();
        f(&mut Writer::new(self.out));
        let len = self.out.len() - start;
        let mut tmp = [0u8; 10];
        let mut n = 0;
        let mut v = len as u64;
        loop {
            let b = (v & 0x7F) as u8;
            v >>= 7;
            tmp[n] = if v == 0 { b } else { b | 0x80 };
            n += 1;
            if v == 0 {
                break;
            }
        }
        self.out.splice(start..start, tmp[..n].iter().copied());
        self
    }
}

/// The length in bytes of `v` as a varint.
pub const fn varint_len(v: u64) -> usize {
    if v == 0 {
        1
    } else {
        (64 - v.leading_zeros() as usize).div_ceil(7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(b: &[u8]) -> Vec<(u32, Wire<'_>)> {
        Fields::new(b).collect::<Result<_>>().unwrap()
    }

    #[test]
    fn encoding_guide_examples() {
        // protobuf.dev "Encoding": nested message 1a 03 08 96 01; packed [3, 270, 86942] is 22 06 03 8e 02 9e a7 05.
        let mut b = Vec::new();
        Writer::new(&mut b).message(3, |w| {
            w.uint(1, 150);
        });
        assert_eq!(b, [0x1A, 0x03, 0x08, 0x96, 0x01]);
        b.clear();
        Writer::new(&mut b).packed_varints(4, &[3, 270, 86942]);
        assert_eq!(b, [0x22, 0x06, 0x03, 0x8E, 0x02, 0x9E, 0xA7, 0x05]);
        let f = fields(&b);
        let v: Vec<u64> = packed_varints(f[0].1.bytes())
            .collect::<Result<_>>()
            .unwrap();
        assert_eq!(v, [3, 270, 86942]);
        // Zigzag table: 0 -> 0, -1 -> 1, 1 -> 2, -2 -> 3, 0x7fffffff -> 0xfffffffe, -0x80000000 -> 0xffffffff.
        for (s, u) in [
            (0i64, 0u64),
            (-1, 1),
            (1, 2),
            (-2, 3),
            (0x7FFF_FFFF, 0xFFFF_FFFE),
            (-0x8000_0000, 0xFFFF_FFFF),
        ] {
            assert_eq!(zigzag(s), u);
            assert_eq!(unzigzag(u), s);
        }
        // int32 -2 is a 10-byte varint.
        b.clear();
        Writer::new(&mut b).int(1, -2);
        assert_eq!(
            b,
            [0x08, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]
        );
        assert_eq!(fields(&b)[0].1.i32(), -2);
    }

    #[test]
    fn every_type_round_trips() {
        let mut b = Vec::new();
        Writer::new(&mut b)
            .uint(1, 300)
            .sint(9, -5)
            .fixed32(4, 7)
            .fixed64(5, u64::MAX)
            .string(2, "hi")
            .float(7, 1.5)
            .double(8, -2.25)
            .bool(10, true)
            .int(3, -2)
            .uint(1, 301);
        let f = fields(&b);
        assert_eq!(f.len(), 10);
        assert_eq!(get(&b, 1).unwrap().unwrap().u64(), 301);
        assert_eq!(get(&b, 9).unwrap().unwrap().sint(), -5);
        assert_eq!(get(&b, 4).unwrap().unwrap().sfixed32(), 7);
        assert_eq!(get(&b, 5).unwrap().unwrap().u64(), u64::MAX);
        assert_eq!(get(&b, 2).unwrap().unwrap().str(), Some("hi"));
        assert_eq!(get(&b, 7).unwrap().unwrap().f32(), 1.5);
        assert_eq!(get(&b, 8).unwrap().unwrap().f64(), -2.25);
        assert!(get(&b, 10).unwrap().unwrap().bool());
        assert_eq!(get(&b, 3).unwrap().unwrap().i64(), -2);
        assert_eq!(get(&b, 99).unwrap(), None);
        let words = [1u8, 0, 0, 0, 2, 0, 0, 0];
        assert_eq!(packed_fixed32(&words).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(packed_fixed64(&words).collect::<Vec<_>>(), [0x2_0000_0001]);
        assert_eq!(varint_len(0), 1);
        assert_eq!(varint_len(127), 1);
        assert_eq!(varint_len(128), 2);
        assert_eq!(varint_len(u64::MAX), 10);
    }

    #[test]
    fn malformed_input_is_an_error() {
        assert!(Fields::new(&[0x0A, 0x05, 1]).any(|f| f.is_err()));
        assert!(Fields::new(&[0x00]).any(|f| f.is_err())); // field number 0
        assert!(Fields::new(&[0x0B]).any(|f| f.is_err())); // wire type 3 (group)
        assert!(Fields::new(&[0x08, 0xFF]).any(|f| f.is_err()));
        let mut at = 0;
        assert!(read_varint(&[0xFF; 11], &mut at).is_err());
        let mut at = 0;
        assert!(read_varint(
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x02],
            &mut at
        )
        .is_err());
        assert!(get(&[0x0A, 0x05, 1], 1).is_err());
        // A long nested message moves its length in front correctly.
        let mut b = Vec::new();
        Writer::new(&mut b).message(1, |w| {
            w.bytes(2, &[7; 200]);
        });
        let f = fields(&b);
        assert_eq!(f[0].1.bytes().len(), 203);
    }
}
