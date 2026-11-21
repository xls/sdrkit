//! gzip over DEFLATE from the `miniz_oxide` crate. The reader makes sure that the CRC-32 and the length of the trailer
//! are right and stops at a size limit.
//!
//! Standard: RFC 1952, RFC 1951.

use crate::error::{Check, Limit, Malformed};
use crate::error::{Error, Result};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;

const FHCRC: u8 = 2;
const FEXTRA: u8 = 4;
const FNAME: u8 = 8;
const FCOMMENT: u8 = 16;

/// Appends the gzip member of `data` to `out`: a 10-byte header (no name, no time, OS 255 "unknown"), the DEFLATE data at
/// `level` (0 to 10) and the trailer.
pub fn compress(data: &[u8], level: u8, out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 255]);
    out.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(data, level.min(10)));
    out.extend_from_slice(&CRC32_ISO_HDLC.checksum(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
}

/// The data of the first gzip member of `b`. If the header is wrong or short, the data is longer than `max_len`, or the CRC-32
/// or ISIZE is wrong, it returns an error.
///
/// ```
/// use sdrkit_proto_ipcast::gzip;
/// let mut z = Vec::new();
/// gzip::compress(b"hello", 6, &mut z);
/// assert_eq!(gzip::decompress(&z, 1 << 20).unwrap(), b"hello");
/// ```
pub fn decompress(b: &[u8], max_len: usize) -> Result<Vec<u8>> {
    if b.len() < 18 {
        return Err(Error::Short);
    }
    if b[0] != 0x1F || b[1] != 0x8B || b[2] != 8 {
        return Err(Error::Invalid(Malformed::GzipHeader));
    }
    let flg = b[3];
    if flg & 0xE0 != 0 {
        return Err(Error::Invalid(Malformed::GzipFlags));
    }
    let mut at = 10;
    if flg & FEXTRA != 0 {
        let x = b.get(at..at + 2).ok_or(Error::Short)?;
        at += 2 + u16::from_le_bytes([x[0], x[1]]) as usize;
    }
    for f in [FNAME, FCOMMENT] {
        if flg & f != 0 {
            let n = b
                .get(at..)
                .ok_or(Error::Short)?
                .iter()
                .position(|&c| c == 0)
                .ok_or(Error::Short)?;
            at += n + 1;
        }
    }
    if flg & FHCRC != 0 {
        let x = b.get(at..at + 2).ok_or(Error::Short)?;
        let want = u16::from_le_bytes([x[0], x[1]]);
        if CRC32_ISO_HDLC.checksum(&b[..at]) as u16 != want {
            return Err(Error::Check(Check::GzipHeaderCrc16));
        }
        at += 2;
    }
    if at + 8 > b.len() {
        return Err(Error::Short);
    }
    let body = &b[at..];
    let (data, used) = inflate(body, max_len)?;
    let t = body.get(used..used + 8).ok_or(Error::Short)?;
    let crc = u32::from_le_bytes([t[0], t[1], t[2], t[3]]);
    let isize = u32::from_le_bytes([t[4], t[5], t[6], t[7]]);
    if CRC32_ISO_HDLC.checksum(&data) != crc {
        return Err(Error::Check(Check::GzipCrc32));
    }
    if data.len() as u32 != isize {
        return Err(Error::Check(Check::GzipIsize));
    }
    Ok(data)
}

/// Inflates raw DEFLATE data. It returns the data and the bytes of input used.
fn inflate(body: &[u8], max_len: usize) -> Result<(Vec<u8>, usize)> {
    use miniz_oxide::inflate::core::{decompress, inflate_flags, DecompressorOxide};
    use miniz_oxide::inflate::TINFLStatus;
    let mut st = DecompressorOxide::new();
    let mut out = vec![0u8; max_len.clamp(1, 1 << 16)];
    let mut in_at = 0;
    let mut out_at = 0;
    loop {
        let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
        let (status, i, o) = decompress(&mut st, &body[in_at..], &mut out, out_at, flags);
        in_at += i;
        out_at += o;
        match status {
            TINFLStatus::Done => {
                out.truncate(out_at);
                return Ok((out, in_at));
            }
            TINFLStatus::HasMoreOutput => {
                if out.len() >= max_len {
                    return Err(Error::TooLarge(Limit::GzipData));
                }
                let n = (out.len() * 2).min(max_len);
                out.resize(n, 0);
            }
            TINFLStatus::NeedsMoreInput | TINFLStatus::FailedCannotMakeProgress => {
                return Err(Error::Short)
            }
            _ => return Err(Error::Invalid(Malformed::Deflate)),
        }
    }
}
