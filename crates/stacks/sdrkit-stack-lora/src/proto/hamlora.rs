//! LoRa APRS and MeshCom 4: the frames, their checks, their radio settings and their builders.

use super::error::{ParseError, Part};
use crate::aprs::info::{parse_info, AprsInfo};
use sdrkit_proto_lora_phy::{Ldro, LoraParams};
use serde::{Deserialize, Serialize};

/// The LoRa APRS prefix.
pub const PREFIX: [u8; 3] = [0x3C, 0xFF, 0x01];

/// A call of a LoRa APRS or MeshCom header: 1 to 20 letters, digits or "-".
pub fn call_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// A LoRa APRS packet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LoraAprs {
    /// Source.
    pub source: String,
    /// Destination.
    pub dest: String,
    /// Path (comma-separated, with "*").
    pub path: String,
    /// The information field.
    pub info: AprsInfo,
    /// The TNC2 text.
    pub tnc2: String,
}

/// Parses a LoRa APRS frame. `Err` with the note.
pub fn parse_lora_aprs(b: &[u8]) -> Result<LoraAprs, ParseError> {
    if b.len() < 3 || b[..3] != PREFIX {
        return Err(ParseError::NoPrefix);
    }
    let mut t = &b[3..];
    while let Some((&last, rest)) = t.split_last() {
        if matches!(last, 0 | b'\r' | b'\n') {
            t = rest;
        } else {
            break;
        }
    }
    let gt = t.iter().position(|&c| c == b'>');
    let colon = gt.and_then(|g| t[g..].iter().position(|&c| c == b':').map(|c| c + g));
    let (Some(g), Some(c)) = (gt, colon) else {
        return Err(ParseError::NotTnc2);
    };
    if g < 1 || c < g + 2 {
        return Err(ParseError::NotTnc2);
    }
    let head = std::str::from_utf8(&t[..c]).map_err(|_| ParseError::BadCall)?;
    let (src, rest) = head.split_once('>').ok_or(ParseError::NotTnc2)?;
    let (dest, path) = match rest.split_once(',') {
        Some((d, p)) => (d, p),
        None => (rest, ""),
    };
    if !call_ok(src) || !call_ok(dest) {
        return Err(ParseError::BadCall);
    }
    if !path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ',' || c == '*')
    {
        return Err(ParseError::BadPath);
    }
    let info = parse_info(&t[c + 1..], dest)?;
    Ok(LoraAprs {
        source: src.into(),
        dest: dest.into(),
        path: path.into(),
        info,
        tnc2: String::from_utf8_lossy(t).into_owned(),
    })
}

/// The TNC2 text `SOURCE>DEST[,PATH]:INFO`. An empty path is left out.
pub fn tnc2(source: &str, dest: &str, path: &str, info: &str) -> String {
    if path.is_empty() {
        format!("{source}>{dest}:{info}")
    } else {
        format!("{source}>{dest},{path}:{info}")
    }
}

/// A LoRa APRS frame from TNC2 text.
pub fn build_lora_aprs(tnc2: &str) -> Vec<u8> {
    let mut b = PREFIX.to_vec();
    b.extend_from_slice(tnc2.as_bytes());
    b
}

/// The LoRa setting of LoRa APRS that a stack takes without a channel list: SF12, 125 kHz, CR 4/5, preamble 8, sync 0x12.
pub fn lora_aprs_params() -> LoraParams {
    params(12, 125e3, 5, 8, 0x12)
}

/// The LoRa setting of MeshCom that a stack takes without a channel list: SF11, 250 kHz, CR 4/6, preamble 32, sync 0x2B.
pub fn meshcom_params() -> LoraParams {
    params(11, 250e3, 6, 32, 0x2B)
}

/// A LoRa setting of the two amateur protocols, LDRO automatic.
pub fn params(sf: u8, bw: f64, cr: u8, preamble: u16, sync: u8) -> LoraParams {
    LoraParams {
        sf,
        bw,
        cr,
        preamble,
        sync,
        ldro: Ldro::Auto,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// MeshCom.

/// A MeshCom packet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeshCom {
    /// "TEXT", "POSITION", "HEY", "ACK".
    pub kind: String,
    /// Message id.
    pub msg_id: u32,
    /// Hop limit / maximum hops (low nibble of the flags).
    pub max_hop: u8,
    /// Flags byte.
    pub flags: u8,
    /// Source call.
    pub source: String,
    /// Relays after the source.
    pub relays: String,
    /// Destination ("*", a call, or a group number).
    pub dest: String,
    /// The call part of the destination.
    pub dest_call: String,
    /// The APRS information (type character + payload).
    pub info: Option<AprsInfo>,
    /// The raw text after the type character.
    pub text: String,
    /// Hardware id.
    pub hw: u8,
    /// Modulation.
    pub modulation: u8,
    /// Country.
    pub country: u8,
    /// Firmware.
    pub fw: u8,
    /// Battery percent (/B=).
    pub battery: Option<u8>,
    /// The acknowledged id (ACK).
    pub acked: Option<u32>,
}

impl MeshCom {
    /// `HW <n>, MOD <m>, country <c>, FW <f>[, server][, track][, app offline][, mesh]`.
    pub fn trailer_summary(&self) -> String {
        let mut s = format!(
            "HW {}, MOD {}, country {}, FW {}",
            self.hw, self.modulation, self.country, self.fw
        );
        for (bit, name) in [
            (0x80, "server"),
            (0x40, "track"),
            (0x20, "app offline"),
            (0x10, "mesh"),
        ] {
            if self.flags & bit != 0 {
                s += &format!(", {name}");
            }
        }
        s
    }
    /// The channel of a text: "MeshCom all" for "*", else `MeshCom <dest>`.
    pub fn channel(&self) -> String {
        if self.dest == "*" {
            "MeshCom all".into()
        } else {
            format!("MeshCom {}", self.dest)
        }
    }
}

/// Parses a MeshCom frame. `Err` with the note (prefixed "MeshCom: ").
pub fn parse_meshcom(b: &[u8]) -> Result<MeshCom, ParseError> {
    if b.is_empty() || !matches!(b[0], b':' | b'!' | b'@' | b'A') {
        return Err(ParseError::MeshComType);
    }
    if b[0] == b'A' {
        if b.len() < 12 {
            return Err(ParseError::MeshComAck);
        }
        let id = u32::from_le_bytes(
            b[1..5]
                .try_into()
                .expect("the range has the array's length"),
        );
        if b[5] & 0x80 == 0 || b[5] & 0x7F > 7 || b[10..12] != [1, 0] {
            return Err(ParseError::MeshComAck);
        }
        let acked = u32::from_le_bytes(
            b[6..10]
                .try_into()
                .expect("the range has the array's length"),
        );
        return Ok(MeshCom {
            kind: "ACK".into(),
            msg_id: id,
            max_hop: b[5] & 0x7F,
            acked: Some(acked),
            ..Default::default()
        });
    }
    if b.len() < 16 {
        return Err(ParseError::Short(Part::MeshComPacket));
    }
    let id = u32::from_le_bytes(
        b[1..5]
            .try_into()
            .expect("the range has the array's length"),
    );
    let flags = b[5];
    let rest = &b[6..];
    let Some(gt) = rest.iter().take(121).position(|&c| c == b'>') else {
        return Err(ParseError::MeshComSource);
    };
    let src_path = &rest[..gt];
    let after = &rest[gt + 1..];
    let Some(tc) = after
        .iter()
        .take(121)
        .position(|&c| matches!(c, b':' | b'!' | b'@'))
    else {
        return Err(ParseError::MeshComDestination);
    };
    let printable = |s: &[u8]| s.iter().all(|&c| (0x20..0x7F).contains(&c));
    if !printable(src_path) || !printable(&after[..tc]) || tc == 0 {
        return Err(ParseError::MeshComDestination);
    }
    let Some(nul) = after[tc..].iter().position(|&c| c == 0) else {
        return Err(ParseError::Short(Part::MeshComPayloadEnd));
    };
    let nul = 6 + gt + 1 + tc + nul;
    if b.len() < nul + 5 {
        return Err(ParseError::Short(Part::MeshComTrailer));
    }
    let sum: u16 = b[..nul + 3]
        .iter()
        .fold(0u16, |a, &x| a.wrapping_add(x as u16));
    let sent = u16::from_be_bytes([b[nul + 3], b[nul + 4]]);
    if sum != sent {
        return Err(ParseError::MeshComChecksum);
    }
    let sp = String::from_utf8_lossy(src_path).into_owned();
    let (source, relays) = match sp.split_once(',') {
        Some((a, r)) => (a.to_string(), r.to_string()),
        None => (sp.clone(), String::new()),
    };
    if !call_ok(&source) {
        return Err(ParseError::MeshComCall);
    }
    let dest = String::from_utf8_lossy(&after[..tc]).into_owned();
    let dest_call = dest.rsplit(',').next().unwrap_or("").to_string();
    let tch = after[tc];
    let body = &after[tc..nul - (6 + gt + 1)];
    let text = String::from_utf8_lossy(&body[1..]).into_owned();
    let mut m = MeshCom {
        kind: match tch {
            b':' => "TEXT",
            b'!' => "POSITION",
            _ => "HEY",
        }
        .into(),
        msg_id: id,
        max_hop: flags & 0x0F,
        flags,
        source,
        relays,
        dest,
        dest_call,
        text: text.clone(),
        hw: b[nul + 1],
        modulation: b[nul + 2] & 0x0F,
        country: b[nul + 2] >> 4,
        fw: b.get(nul + 5).copied().unwrap_or(0),
        ..Default::default()
    };
    if tch == b'!' {
        let mut info = parse_info(body, "APRS").ok();
        if let Some(i) = &mut info {
            if let Some(p) = i.comment.find("/B=") {
                let d: String = i.comment[p + 3..]
                    .chars()
                    .take(3)
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                m.battery = d.parse().ok();
                let end = p + 3 + d.len();
                i.comment = format!("{}{}", &i.comment[..p], &i.comment[end..])
                    .trim()
                    .to_string();
            }
            i.comment = i.comment.trim().to_string();
        }
        m.info = info;
    }
    Ok(m)
}

/// Builds a MeshCom frame: `kind` ':' text, '!' position, '@' HEY. The source path, destination and payload as text.
pub fn build_meshcom(
    kind: u8,
    msg_id: u32,
    max_hop: u8,
    flags: u8,
    src_path: &str,
    dest: &str,
    payload: &str,
) -> Vec<u8> {
    let mut b = vec![kind];
    b.extend_from_slice(&msg_id.to_le_bytes());
    b.push((flags & 0xF0) | (max_hop & 0x0F));
    b.extend_from_slice(src_path.as_bytes());
    b.push(b'>');
    b.extend_from_slice(dest.as_bytes());
    b.push(kind);
    b.extend_from_slice(payload.as_bytes());
    b.push(0);
    b.push(4); // hardware id
    b.push(3); // modulation 3, country 0
    let sum: u16 = b.iter().fold(0u16, |a, &x| a.wrapping_add(x as u16));
    b.extend_from_slice(&sum.to_be_bytes());
    b.push(35); // firmware
    b.push(0x84); // last hardware
    b.push(b'a');
    b.push(0x7E);
    b
}

/// A MeshCom acknowledgement.
pub fn build_meshcom_ack(own_id: u32, hops: u8, acked: u32) -> Vec<u8> {
    let mut b = vec![b'A'];
    b.extend_from_slice(&own_id.to_le_bytes());
    b.push(0x80 | (hops & 7));
    b.extend_from_slice(&acked.to_le_bytes());
    b.extend_from_slice(&[1, 0]);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::meshtastic::{hex, hex_decode};

    #[test]
    fn lora_aprs() {
        let f = build_lora_aprs("A61AB-7>APLRT1,WIDE1-1:!2504.83N/05508.42E>Marina tracker 12.6V");
        let p = parse_lora_aprs(&f).unwrap();
        assert_eq!(
            (p.source.as_str(), p.dest.as_str(), p.path.as_str()),
            ("A61AB-7", "APLRT1", "WIDE1-1")
        );
        assert_eq!(p.info.comment, "Marina tracker 12.6V");
        assert_eq!(
            (p.info.symbol_table, p.info.symbol_code),
            (Some('/'), Some('>'))
        );
        assert!(parse_lora_aprs(b"A61AB>APRS:x").is_err());
        assert!(parse_lora_aprs(&build_lora_aprs("A61AB:x")).is_err());
        assert!(parse_lora_aprs(&build_lora_aprs("A6!AB>APRS:x")).is_err());
        assert!(parse_lora_aprs(&build_lora_aprs("A61AB>APRS,WI$E:x")).is_err());
        assert_eq!(
            parse_lora_aprs(&build_lora_aprs("A61AB>APRS:%%unknown"))
                .unwrap()
                .info
                .kind,
            "Other"
        );
    }

    #[test]
    fn meshcom_recording_frames() {
        let a = hex_decode("3a44332211154e3043414c4c2d323e2a3a68656c6c6f2066726f6d207364726b69740004030a342384617e").unwrap();
        let m = parse_meshcom(&a).unwrap();
        assert_eq!(
            (
                m.kind.as_str(),
                m.msg_id,
                m.max_hop,
                m.source.as_str(),
                m.dest.as_str()
            ),
            ("TEXT", 0x11223344, 5, "N0CALL-2", "*")
        );
        assert_eq!(m.text, "hello from sdrkit");
        assert_eq!((m.hw, m.modulation, m.country, m.fw), (4, 3, 0, 35));
        let built = build_meshcom(
            b':',
            0x11223344,
            5,
            0x10,
            "N0CALL-2",
            "*",
            "hello from sdrkit",
        );
        assert_eq!(hex(&built), hex(&a));
        let b = hex_decode("2145332211144e3043414c4c2d322c4e3043414c4c2d313e2a21353232322e32314e2f30303435332e373145234d657368436f6d206e6f6465202f423d303634202f413d303030313233000403112d2384617e").unwrap();
        let p = parse_meshcom(&b).unwrap();
        assert_eq!((p.relays.as_str(), p.battery), ("N0CALL-1", Some(64)));
        let i = p.info.unwrap();
        assert!((i.alt_m.unwrap() - 37.49).abs() < 0.01);
        let mut bad = a.clone();
        bad[10] ^= 1;
        assert_eq!(
            parse_meshcom(&bad).unwrap_err(),
            ParseError::MeshComChecksum
        );
        let ack = build_meshcom_ack(1, 3, 0x11223344);
        assert_eq!(parse_meshcom(&ack).unwrap().acked, Some(0x11223344));
        assert!(parse_lora_aprs(&a).is_err());
    }
}
