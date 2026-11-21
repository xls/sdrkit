//! AX.25 UI frames from APRS packets in TNC2 text (`FROM>TO,DIGI1*,DIGI2:info`), and KISS framing for programs that expect a KISS
//! TNC.

/// KISS frame end.
pub const FEND: u8 = 0xC0;
/// KISS frame escape.
pub const FESC: u8 = 0xDB;
/// Escaped frame end.
pub const TFEND: u8 = 0xDC;
/// Escaped frame escape.
pub const TFESC: u8 = 0xDD;

/// One address in AX.25 form (7 bytes): six characters shifted left by one, then the SSID byte. `None` when the call sign is longer
/// than six characters, not capital letters and digits, or the SSID is not 0 to 15.
fn address(call: &str, last: bool, repeated: bool) -> Option<[u8; 7]> {
    let (base, ssid) = match call.split_once('-') {
        Some((b, s)) => (b, s.parse::<u8>().ok().filter(|s| *s <= 15)?),
        None => (call, 0),
    };
    if base.is_empty()
        || base.len() > 6
        || !base
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return None;
    }
    let mut a = [b' ' << 1; 7];
    for (i, b) in base.bytes().enumerate() {
        a[i] = b << 1;
    }
    a[6] = 0x60 | (ssid << 1) | u8::from(last) | if repeated { 0x80 } else { 0 };
    Some(a)
}

/// The AX.25 UI frame (without flags or FCS, as KISS carries it) of a TNC2 packet. A digipeater marked with `*` has its "has been
/// repeated" bit set, as have those before it. `None` when an address cannot be encoded (an APRS-IS construct such as `qAR` or
/// `TCPIP*`).
pub fn from_tnc2(tnc2: &str) -> Option<Vec<u8>> {
    let (header, info) = tnc2.split_once(':')?;
    let (from, rest) = header.split_once('>')?;
    let mut parts = rest.split(',');
    let to = parts.next()?;
    let digis: Vec<&str> = parts.filter(|d| !d.is_empty()).collect();
    if digis.len() > 8 {
        return None;
    }
    let last_used = digis.iter().rposition(|d| d.ends_with('*'));
    let mut f = Vec::with_capacity(16 + 7 * digis.len() + info.len());
    f.extend_from_slice(&address(to, false, false)?);
    f.extend_from_slice(&address(from, digis.is_empty(), false)?);
    for (i, d) in digis.iter().enumerate() {
        let call = d.trim_end_matches('*');
        f.extend_from_slice(&address(
            call,
            i + 1 == digis.len(),
            last_used.is_some_and(|l| i <= l),
        )?);
    }
    f.push(0x03); // UI frame
    f.push(0xF0); // no layer 3
    f.extend_from_slice(info.as_bytes());
    Some(f)
}

/// A KISS data frame on a port (0 to 15).
pub fn kiss_frame(frame: &[u8], port: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(frame.len() + 4);
    out.push(FEND);
    out.push((port & 0x0F) << 4);
    for &b in frame {
        match b {
            FEND => out.extend_from_slice(&[FESC, TFEND]),
            FESC => out.extend_from_slice(&[FESC, TFESC]),
            _ => out.push(b),
        }
    }
    out.push(FEND);
    out
}

/// The frames in a KISS byte stream (data frames only, unescaped, without the command byte).
pub fn kiss_decode(stream: &[u8]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    for chunk in stream.split(|b| *b == FEND) {
        if chunk.len() < 2 || chunk[0] & 0x0F != 0 {
            continue;
        }
        let mut f = Vec::with_capacity(chunk.len());
        let mut esc = false;
        for &b in &chunk[1..] {
            match (esc, b) {
                (true, TFEND) => f.push(FEND),
                (true, TFESC) => f.push(FESC),
                (true, other) => f.push(other),
                (false, FESC) => {
                    esc = true;
                    continue;
                }
                (false, other) => f.push(other),
            }
            esc = false;
        }
        frames.push(f);
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tnc2_packet_becomes_a_ui_frame() {
        let f = from_tnc2("VK2ABC-9>APRS,WIDE1-1*,WIDE2-1:!3351.00S/15112.00E>test").unwrap();
        assert_eq!(&f[..6], b"APRS  ".map(|b| b << 1));
        assert_eq!(f[6], 0x60);
        assert_eq!(&f[7..13], b"VK2ABC".map(|b| b << 1));
        assert_eq!(f[13], 0x60 | (9 << 1));
        assert_eq!(f[20], 0x80 | 0x60 | (1 << 1)); // WIDE1-1, repeated
        assert_eq!(f[27], 0x60 | (1 << 1) | 1); // WIDE2-1, last address
        assert_eq!(&f[28..30], &[0x03, 0xF0]);
        assert!(f.ends_with(b">test"));
        assert!(from_tnc2("VK2ABC>APRS,qAR,VK2XYZ:x").is_none());
        assert!(from_tnc2("TOOLONGCALL>APRS:x").is_none());
        assert!(from_tnc2("A>B-16:x").is_none());
        assert!(from_tnc2("no header").is_none());
    }

    #[test]
    fn kiss_escapes_round_trip() {
        let frame = vec![1, FEND, 2, FESC, 3];
        let k = kiss_frame(&frame, 0);
        assert_eq!(k, [FEND, 0, 1, FESC, TFEND, 2, FESC, TFESC, 3, FEND]);
        let mut two = k.clone();
        two.extend(kiss_frame(b"abc", 0));
        assert_eq!(kiss_decode(&two), [frame, b"abc".to_vec()]);
    }
}
