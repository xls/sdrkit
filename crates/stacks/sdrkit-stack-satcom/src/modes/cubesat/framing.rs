//! What the family adds to the CubeSat frame layers of `sdrkit-proto-link` and `sdrkit-proto-ccsds`: the CSP header in words and the text
//! beacon reader.

use sdrkit_proto_link::csp::{Header, FLAG_CRC32, FLAG_HMAC, FLAG_RDP, FLAG_XTEA};

/// The CSP header in words, for example "CSP priority 2 from 1:7 to 10:31 (CRC32)".
pub fn csp_words(h: &Header) -> String {
    let mut f = Vec::new();
    for (bit, n) in [
        (FLAG_HMAC, "HMAC"),
        (FLAG_XTEA, "XTEA"),
        (FLAG_RDP, "RDP"),
        (FLAG_CRC32, "CRC32"),
    ] {
        if h.flags & bit != 0 {
            f.push(n);
        }
    }
    let flags = if f.is_empty() {
        String::new()
    } else {
        format!(" ({})", f.join(", "))
    };
    format!(
        "CSP priority {} from {}:{} to {}:{}{flags}",
        h.priority, h.source, h.source_port, h.destination, h.dest_port
    )
}

/// A text beacon: the longest run of printable ASCII in `b`, when it is long enough to be text (at least 8 characters, at least
/// 4 of them letters). A run of symbols only is not text.
pub fn text_beacon(b: &[u8]) -> Option<String> {
    let mut best: &[u8] = &[];
    let mut start = 0;
    for (i, &c) in b.iter().chain(std::iter::once(&0u8)).enumerate() {
        if !(0x20..0x7F).contains(&c) {
            if i - start > best.len() {
                best = &b[start..i];
            }
            start = i + 1;
        }
    }
    let letters = best.iter().filter(|c| c.is_ascii_alphabetic()).count();
    let text = String::from_utf8_lossy(best);
    (text.trim().len() >= 8 && letters >= 4).then(|| text.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_beacons_and_csp_words() {
        assert_eq!(text_beacon(b"\x01\x02hi hi\x00"), None);
        assert_eq!(
            text_beacon(b"\x01CQ CQ DE TESTSAT\x00\x02"),
            Some("CQ CQ DE TESTSAT".into())
        );
        assert_eq!(text_beacon(b"!!!!!!!!!!!!"), None);
        let h = Header {
            priority: 2,
            source: 1,
            destination: 10,
            dest_port: 31,
            source_port: 7,
            flags: FLAG_CRC32,
        };
        assert_eq!(csp_words(&h), "CSP priority 2 from 1:7 to 10:31 (CRC32)");
    }
}
