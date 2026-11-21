//! ZIP products: the first file of the archive, stored or deflated, at most 16 MiB unpacked.
//! Standard: PKWARE APPNOTE 4.3.7, RFC 1951

use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;

/// The most bytes unpacked.
pub const MAX_UNPACKED: usize = 16 << 20;

/// The first file of a ZIP archive: (its name, its bytes). `None` when it is not a ZIP or cannot be unpacked.
pub fn first_file(zip: &[u8]) -> Option<(String, Vec<u8>)> {
    if zip.len() < 30 || &zip[..4] != b"PK\x03\x04" {
        return None;
    }
    let le16 = |o: usize| u16::from_le_bytes([zip[o], zip[o + 1]]) as usize;
    let le32 = |o: usize| u32::from_le_bytes([zip[o], zip[o + 1], zip[o + 2], zip[o + 3]]) as usize;
    let method = le16(8);
    let comp = le32(18);
    let size = le32(22);
    let name_len = le16(26);
    let extra = le16(28);
    let start = 30 + name_len + extra;
    if start > zip.len() {
        return None;
    }
    let name = String::from_utf8_lossy(&zip[30..30 + name_len]).to_string();
    let body = if comp > 0 && start + comp <= zip.len() {
        &zip[start..start + comp]
    } else {
        &zip[start..]
    };
    let data = match method {
        0 => {
            let n = if size > 0 {
                size.min(body.len())
            } else {
                body.len()
            };
            body[..n.min(MAX_UNPACKED)].to_vec()
        }
        8 => miniz_oxide::inflate::decompress_to_vec_with_limit(body, MAX_UNPACKED).ok()?,
        _ => return None,
    };
    Some((name, data))
}

/// A ZIP archive with one stored file (for the generators).
pub fn stored(name: &str, data: &[u8]) -> Vec<u8> {
    let crc = CRC32_ISO_HDLC.checksum(data);
    let mut z = Vec::new();
    z.extend_from_slice(b"PK\x03\x04");
    z.extend_from_slice(&20u16.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes()); // stored
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(&0x5B61u16.to_le_bytes());
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&(data.len() as u32).to_le_bytes());
    z.extend_from_slice(&(data.len() as u32).to_le_bytes());
    z.extend_from_slice(&(name.len() as u16).to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(name.as_bytes());
    z.extend_from_slice(data);
    // Central directory and its end, so other tools open it.
    let cd = z.len();
    z.extend_from_slice(b"PK\x01\x02");
    z.extend_from_slice(&20u16.to_le_bytes());
    z.extend_from_slice(&20u16.to_le_bytes());
    z.extend_from_slice(&[0; 4]);
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(&0x5B61u16.to_le_bytes());
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&(data.len() as u32).to_le_bytes());
    z.extend_from_slice(&(data.len() as u32).to_le_bytes());
    z.extend_from_slice(&(name.len() as u16).to_le_bytes());
    z.extend_from_slice(&[0; 12]);
    z.extend_from_slice(&0u32.to_le_bytes());
    z.extend_from_slice(name.as_bytes());
    let cd_len = z.len() - cd;
    z.extend_from_slice(b"PK\x05\x06");
    z.extend_from_slice(&[0; 4]);
    z.extend_from_slice(&1u16.to_le_bytes());
    z.extend_from_slice(&1u16.to_le_bytes());
    z.extend_from_slice(&(cd_len as u32).to_le_bytes());
    z.extend_from_slice(&(cd as u32).to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_and_deflated_files_unpack() {
        let z = stored("A.TXT", b"hello emwin");
        assert_eq!(
            first_file(&z),
            Some(("A.TXT".into(), b"hello emwin".to_vec()))
        );
        let body = miniz_oxide::deflate::compress_to_vec(b"deflated text deflated text", 6);
        let mut z2 = z[..30].to_vec();
        z2[8] = 8;
        z2[18..22].copy_from_slice(&(body.len() as u32).to_le_bytes());
        z2[22..26].copy_from_slice(&27u32.to_le_bytes());
        z2[26..28].copy_from_slice(&1u16.to_le_bytes());
        z2.truncate(30);
        z2.push(b'B');
        z2.extend_from_slice(&body);
        assert_eq!(first_file(&z2).unwrap().1, b"deflated text deflated text");
        assert!(first_file(b"not a zip").is_none());
    }
}
