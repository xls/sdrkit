//! The saved settings blob: the settings of owners by text key, as bytes that the host stores and does not read.
//!
//! Layout, little-endian:
//!
//! ```text
//! magic "SDKS" | version u16 | owner count u32 | owners | CRC-32/ISO-HDLC u32 of every byte before it
//! owner: key | entry count u32 | entries          entry: key | tag u8 | value length u32 | value
//! key: length u32 | UTF-8
//! ```
//!
//! A setting is stored by its key and a choice by the key of its option, so a blob stays valid when a family adds,
//! removes or reorders settings and options. The tags are frozen: see [`Stored::tag`].

use sdrkit_core::Value;
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use std::sync::Arc;

/// The first bytes of every blob.
const MAGIC: [u8; 4] = *b"SDKS";
/// The format version that this library writes and the latest that it reads.
pub(crate) const VERSION: u16 = 1;

const TAG_BOOL: u8 = 1;
const TAG_I64: u8 = 2;
const TAG_F64: u8 = 3;
const TAG_TEXT: u8 = 4;
const TAG_HZ: u8 = 5;
const TAG_TIME: u8 = 6;
const TAG_DURATION: u8 = 7;
const TAG_POSITION: u8 = 8;
const TAG_CHOICE: u8 = 9;
const TAG_BYTES: u8 = 10;

/// The value of one setting in a blob.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Stored {
    /// Any value but a choice and [`Value::None`].
    Value(Value),
    /// A choice, by the key of its option.
    Choice(String),
    /// A value of a type that this library does not know, by its tag.
    Unknown(u8),
}

/// The settings of one owner in a blob.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Owner {
    /// The owner key.
    pub key: String,
    /// The settings, by setting key.
    pub entries: Vec<(String, Stored)>,
}

/// Why bytes are not a blob that this library reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlobError {
    /// The bytes do not begin with the magic number, or have version 0.
    NotSettings,
    /// A later version of the library wrote the bytes.
    Version(u16),
    /// The bytes are cut short, fail the checksum, or do not decode at byte offset `at`.
    Damaged(usize),
}

impl Stored {
    /// The frozen tag of the value, or `None` for a value that a blob does not hold ([`Value::None`], a choice by index).
    fn tag(&self) -> Option<u8> {
        Some(match self {
            Stored::Value(v) => match v {
                Value::Bool(_) => TAG_BOOL,
                Value::I64(_) => TAG_I64,
                Value::F64(_) => TAG_F64,
                Value::Text(_) => TAG_TEXT,
                Value::Hz(_) => TAG_HZ,
                Value::Time(_) => TAG_TIME,
                Value::Duration(_) => TAG_DURATION,
                Value::Position { .. } => TAG_POSITION,
                Value::Bytes(_) => TAG_BYTES,
                // No setting is a flag set.
                Value::None | Value::Choice(_) | Value::Flags(_) => return None,
            },
            Stored::Choice(_) => TAG_CHOICE,
            Stored::Unknown(t) => *t,
        })
    }

    fn payload(&self, out: &mut Vec<u8>) {
        match self {
            Stored::Value(v) => match v {
                Value::Bool(b) => out.push(u8::from(*b)),
                Value::I64(x) => out.extend_from_slice(&x.to_le_bytes()),
                Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => {
                    out.extend_from_slice(&x.to_le_bytes());
                }
                Value::Position { lat, lon } => {
                    out.extend_from_slice(&lat.to_le_bytes());
                    out.extend_from_slice(&lon.to_le_bytes());
                }
                Value::Text(t) => out.extend_from_slice(t.as_bytes()),
                Value::Bytes(b) => out.extend_from_slice(b),
                Value::None | Value::Choice(_) | Value::Flags(_) => {}
            },
            Stored::Choice(k) => out.extend_from_slice(k.as_bytes()),
            Stored::Unknown(_) => {}
        }
    }
}

/// The blob of `owners`. Entries that a blob does not hold ([`Stored::tag`] is `None`) are left out. Returns `None` if a
/// key or a value is longer than 2^32 - 1 bytes.
pub(crate) fn encode(owners: &[Owner]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    put_len(&mut out, owners.len())?;
    let mut value = Vec::new();
    for o in owners {
        put_text(&mut out, &o.key)?;
        let entries: Vec<_> = o
            .entries
            .iter()
            .filter_map(|(k, v)| Some((k, v, v.tag()?)))
            .collect();
        put_len(&mut out, entries.len())?;
        for (key, v, tag) in entries {
            put_text(&mut out, key)?;
            out.push(tag);
            value.clear();
            v.payload(&mut value);
            put_len(&mut out, value.len())?;
            out.extend_from_slice(&value);
        }
    }
    let crc = CRC32_ISO_HDLC.checksum(&out);
    out.extend_from_slice(&crc.to_le_bytes());
    Some(out)
}

fn put_len(out: &mut Vec<u8>, n: usize) -> Option<()> {
    out.extend_from_slice(&u32::try_from(n).ok()?.to_le_bytes());
    Some(())
}

fn put_text(out: &mut Vec<u8>, s: &str) -> Option<()> {
    put_len(out, s.len())?;
    out.extend_from_slice(s.as_bytes());
    Some(())
}

/// The owners of the blob `bytes`. Checks the magic number, then the version, then the checksum, then every length.
///
/// # Errors
///
/// [`BlobError`]. The call never panics, whatever the bytes.
pub(crate) fn decode(bytes: &[u8]) -> Result<Vec<Owner>, BlobError> {
    if bytes.len() < MAGIC.len() || bytes[..MAGIC.len()] != MAGIC {
        return Err(BlobError::NotSettings);
    }
    let head = MAGIC.len() + 2;
    let version = match bytes.get(MAGIC.len()..head) {
        Some(&[a, b]) => u16::from_le_bytes([a, b]),
        _ => return Err(BlobError::Damaged(bytes.len())),
    };
    if version == 0 {
        return Err(BlobError::NotSettings);
    }
    if version > VERSION {
        return Err(BlobError::Version(version));
    }
    let body_end = bytes
        .len()
        .checked_sub(4)
        .filter(|&n| n >= head)
        .ok_or(BlobError::Damaged(bytes.len()))?;
    let (body, crc) = bytes.split_at(body_end);
    let crc = u32::from_le_bytes([crc[0], crc[1], crc[2], crc[3]]);
    if CRC32_ISO_HDLC.checksum(body) != crc {
        return Err(BlobError::Damaged(body_end));
    }
    let mut r = Reader {
        bytes: body,
        at: head,
    };
    let mut owners = Vec::new();
    for _ in 0..r.u32()? {
        let key = r.text()?;
        let mut entries = Vec::new();
        for _ in 0..r.u32()? {
            let key = r.text()?;
            let tag = r.take(1)?[0];
            let start = r.at;
            let n = r.u32()? as usize;
            let v = r.take(n)?;
            entries.push((key, value_of(tag, v).ok_or(BlobError::Damaged(start))?));
        }
        owners.push(Owner { key, entries });
    }
    if r.at != body.len() {
        return Err(BlobError::Damaged(r.at));
    }
    Ok(owners)
}

/// The value of tag `tag` in the bytes `v`, or `None` if `v` does not fit the tag.
fn value_of(tag: u8, v: &[u8]) -> Option<Stored> {
    let f64_at = |i: usize| {
        v.get(i..i + 8)
            .and_then(|b| b.try_into().ok())
            .map(f64::from_le_bytes)
    };
    let f64_only = || if v.len() == 8 { f64_at(0) } else { None };
    let text = || std::str::from_utf8(v).ok().map(str::to_owned);
    Some(match tag {
        TAG_BOOL => Stored::Value(Value::Bool(match v {
            [0] => false,
            [1] => true,
            _ => return None,
        })),
        TAG_I64 => Stored::Value(Value::I64(i64::from_le_bytes(v.try_into().ok()?))),
        TAG_F64 => Stored::Value(Value::F64(f64_only()?)),
        TAG_HZ => Stored::Value(Value::Hz(f64_only()?)),
        TAG_TIME => Stored::Value(Value::Time(f64_only()?)),
        TAG_DURATION => Stored::Value(Value::Duration(f64_only()?)),
        TAG_POSITION if v.len() == 16 => Stored::Value(Value::Position {
            lat: f64_at(0)?,
            lon: f64_at(8)?,
        }),
        TAG_POSITION => return None,
        TAG_TEXT => Stored::Value(Value::Text(text()?)),
        TAG_CHOICE => Stored::Choice(text()?),
        TAG_BYTES => Stored::Value(Value::Bytes(Arc::from(v))),
        t => Stored::Unknown(t),
    })
}

/// A cursor over the body of a blob. Every read checks the length that remains.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], BlobError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or(BlobError::Damaged(self.at))?;
        let s = &self.bytes[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, BlobError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn text(&mut self) -> Result<String, BlobError> {
        let start = self.at;
        let n = self.u32()? as usize;
        let b = self.take(n)?;
        std::str::from_utf8(b)
            .map(str::to_owned)
            .map_err(|_| BlobError::Damaged(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_type() -> Vec<Owner> {
        let v = |x: Value| Stored::Value(x);
        vec![
            Owner {
                key: "engine".into(),
                entries: vec![
                    ("flag".into(), v(Value::Bool(true))),
                    ("off".into(), v(Value::Bool(false))),
                    ("count".into(), v(Value::I64(-42))),
                    ("gain".into(), v(Value::F64(-3.25))),
                    ("callsign".into(), v(Value::Text("N0CALL é".into()))),
                    ("empty".into(), v(Value::Text(String::new()))),
                    ("centre_hz".into(), v(Value::Hz(433.92e6))),
                    ("at".into(), v(Value::Time(1.7e9))),
                    ("hold".into(), v(Value::Duration(2.5))),
                    (
                        "station".into(),
                        v(Value::Position {
                            lat: -33.86,
                            lon: 151.21,
                        }),
                    ),
                    ("detector".into(), Stored::Choice("auto".into())),
                    ("key".into(), v(Value::Bytes(Arc::from(&[0u8, 255, 7][..])))),
                ],
            },
            Owner {
                key: "service.scan".into(),
                entries: vec![],
            },
        ]
    }

    #[test]
    fn every_type_round_trips() {
        let owners = every_type();
        let blob = encode(&owners).unwrap();
        assert_eq!(&blob[..4], b"SDKS");
        assert_eq!(decode(&blob).unwrap(), owners);
    }

    #[test]
    fn a_value_that_a_blob_does_not_hold_is_left_out() {
        let owners = vec![Owner {
            key: "engine".into(),
            entries: vec![
                ("none".into(), Stored::Value(Value::None)),
                ("index".into(), Stored::Value(Value::Choice(2))),
                ("kept".into(), Stored::Value(Value::I64(1))),
            ],
        }];
        let back = decode(&encode(&owners).unwrap()).unwrap();
        assert_eq!(
            back[0].entries,
            vec![("kept".into(), Stored::Value(Value::I64(1)))]
        );
    }

    #[test]
    fn a_later_version_is_refused_before_the_checksum() {
        let mut blob = encode(&every_type()).unwrap();
        blob[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
        assert_eq!(decode(&blob), Err(BlobError::Version(VERSION + 1)));
        blob[4..6].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(decode(&blob), Err(BlobError::NotSettings));
        assert_eq!(decode(b"{\"version\":1}"), Err(BlobError::NotSettings));
        assert_eq!(decode(&[]), Err(BlobError::NotSettings));
    }

    #[test]
    fn an_unknown_tag_decodes_as_unknown() {
        let mut blob = encode(&[Owner {
            key: "engine".into(),
            entries: vec![("x".into(), Stored::Value(Value::I64(5)))],
        }])
        .unwrap();
        // magic 4, version 2, owner count 4, owner key 4 + 6, entry count 4, entry key 4 + 1: the tag follows.
        let tag = 4 + 2 + 4 + 4 + 6 + 4 + 4 + 1;
        blob[tag] = 200;
        let n = blob.len() - 4;
        let crc = CRC32_ISO_HDLC.checksum(&blob[..n]);
        blob[n..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(decode(&blob).unwrap()[0].entries[0].1, Stored::Unknown(200));
    }

    #[test]
    fn every_truncation_is_refused() {
        let blob = encode(&every_type()).unwrap();
        for n in 0..blob.len() {
            assert!(decode(&blob[..n]).is_err(), "{n} bytes");
        }
        let mut longer = blob.clone();
        longer.push(0);
        assert!(decode(&longer).is_err());
    }

    #[test]
    fn random_damage_is_refused_without_a_panic() {
        let blob = encode(&every_type()).unwrap();
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..500 {
            let mut b = blob.clone();
            let flips = 1 + (next() % 4) as usize;
            for _ in 0..flips {
                let i = (next() % b.len() as u64) as usize;
                let x = (next() % 255) as u8 + 1;
                b[i] ^= x;
            }
            if b == blob {
                continue;
            }
            assert!(decode(&b).is_err(), "{b:?}");
        }
    }

    #[test]
    fn damage_with_a_valid_checksum_is_refused_without_a_panic() {
        let blob = encode(&every_type()).unwrap();
        let n = blob.len() - 4;
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..500 {
            let mut b = blob.clone();
            let i = 6 + (next() % (n - 6) as u64) as usize;
            b[i] ^= (next() % 255) as u8 + 1;
            let crc = CRC32_ISO_HDLC.checksum(&b[..n]);
            b[n..].copy_from_slice(&crc.to_le_bytes());
            // A change inside a value can still decode; a change of a length or a key must not panic.
            let _ = decode(&b);
        }
    }
}
