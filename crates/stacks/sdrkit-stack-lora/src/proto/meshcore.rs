//! MeshCore: the radio setting, the packet format, channels (group text and data), adverts with their
//! Ed25519 signature, and the end-to-end direct messages of the companion client.

use super::error::{ParseError, Part};
use super::meshtastic::{base64_decode, hex, hex_decode};
use sdrkit_algo_crypto::{curve, sha256, Aes, AesKey, Ed25519Key, HmacSha256, Key, Sha256};
use sdrkit_proto_lora_phy::{Ldro, LoraParams};
use serde::{Deserialize, Serialize};

/// The Public channel's secret.
pub const PUBLIC_SECRET: [u8; 16] = [
    0x8B, 0x33, 0x87, 0xE9, 0xC5, 0xCD, 0xEA, 0x6A, 0xC9, 0xE5, 0xED, 0xBA, 0xA1, 0x15, 0xCD, 0x72,
];

/// The longest payload.
pub const MAX_PAYLOAD: usize = 184;

// ---------------------------------------------------------------------------------------------------------------------------------
// The radio setting.

/// The LoRa setting of a MeshCore network: sync 0x12, preamble 32 up to SF8 and 16 above.
pub fn params(sf: u8, bw: f64, cr: u8) -> LoraParams {
    LoraParams {
        sf,
        bw,
        cr,
        preamble: if sf <= 8 { 32 } else { 16 },
        sync: 0x12,
        ldro: Ldro::Auto,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The packet.

/// Payload type names.
pub fn type_name(t: u8) -> String {
    let s = match t {
        0 => "REQ",
        1 => "RESPONSE",
        2 => "TXT_MSG",
        3 => "ACK",
        4 => "ADVERT",
        5 => "GRP_TXT",
        6 => "GRP_DATA",
        7 => "ANON_REQ",
        8 => "PATH",
        9 => "TRACE",
        10 => "MULTIPART",
        11 => "CONTROL",
        15 => "RAW_CUSTOM",
        _ => return format!("TYPE_{t}"),
    };
    s.into()
}

/// A parsed packet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct McPacket {
    /// Route type (0 transport flood, 1 flood, 2 direct, 3 transport direct).
    pub route: u8,
    /// Payload type.
    pub ptype: u8,
    /// Payload version.
    pub version: u8,
    /// Path hash size in bytes.
    pub hash_size: u8,
    /// The path hashes.
    pub path: Vec<Vec<u8>>,
    /// The payload.
    pub payload: Vec<u8>,
}

impl McPacket {
    /// Parses a frame. `Err` with the refusal note.
    pub fn parse(b: &[u8]) -> Result<McPacket, ParseError> {
        if b.len() < 3 {
            return Err(ParseError::Short(Part::MeshCorePacket));
        }
        let h = b[0];
        let route = h & 3;
        let ptype = (h >> 2) & 0xF;
        let version = h >> 6;
        let mut at = 1;
        if route == 0 || route == 3 {
            if b.len() < at + 4 {
                return Err(ParseError::Short(Part::TransportCodes));
            }
            at += 4;
        }
        let pl = *b.get(at).ok_or(ParseError::Short(Part::TransportCodes))?;
        at += 1;
        let count = (pl & 0x3F) as usize;
        let size = ((pl >> 6) + 1) as usize;
        if size == 4 || count * size > 64 || at + count * size > b.len() {
            return Err(ParseError::PathLength);
        }
        let path = (0..count)
            .map(|i| b[at + i * size..at + (i + 1) * size].to_vec())
            .collect();
        at += count * size;
        if at >= b.len() {
            return Err(ParseError::Short(Part::MeshCorePayload));
        }
        let payload = b[at..].to_vec();
        if payload.len() > MAX_PAYLOAD {
            return Err(ParseError::PayloadTooLong { max: MAX_PAYLOAD });
        }
        Ok(McPacket {
            route,
            ptype,
            version,
            hash_size: size as u8,
            path,
            payload,
        })
    }
    /// The frame bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out =
            vec![(self.route & 3) | ((self.ptype & 0xF) << 2) | ((self.version & 3) << 6)];
        if self.route == 0 || self.route == 3 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        }
        let size = self.hash_size.max(1);
        out.push((self.path.len() as u8 & 0x3F) | ((size - 1) << 6));
        for p in &self.path {
            out.extend_from_slice(p);
        }
        out.extend_from_slice(&self.payload);
        out
    }
    /// The path as "aa,bbcc".
    pub fn path_text(&self) -> String {
        self.path
            .iter()
            .map(|p| hex(p))
            .collect::<Vec<_>>()
            .join(",")
    }
    /// `, <n> hop(s)` or ", direct from sender".
    pub fn hop_text(&self) -> String {
        if !self.path.is_empty() {
            format!(", {} hop(s)", self.path.len())
        } else if self.route == 0 || self.route == 1 {
            ", direct from sender".into()
        } else {
            String::new()
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Crypto.

/// A channel's hash: the first byte of SHA-256 of its secret.
pub fn channel_hash(secret: &[u8]) -> u8 {
    sha256(secret)[0]
}

/// The secret of a hashtag channel: the first 16 bytes of SHA-256 of the name.
pub fn hashtag_secret(name: &str) -> [u8; 16] {
    let d = sha256(name.as_bytes());
    let mut s = [0u8; 16];
    s.copy_from_slice(&d[..16]);
    s
}

/// A user secret: hex of 32 or 64 digits or base64 of 16 or 32 bytes.
pub fn parse_secret(s: &str) -> Option<Vec<u8>> {
    let c: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .collect();
    if (c.len() == 32 || c.len() == 64) && c.chars().all(|x| x.is_ascii_hexdigit()) {
        return hex_decode(&c);
    }
    let b = base64_decode(s)?;
    (b.len() == 16 || b.len() == 32).then_some(b)
}

/// The 2-byte MAC: HMAC-SHA256 keyed with the secret padded with zeros to 32 bytes, cut to its first two bytes.
fn mac2(secret: &[u8], data: &[u8]) -> [u8; 2] {
    let mut key = [0u8; 32];
    let n = secret.len().min(32);
    key[..n].copy_from_slice(&secret[..n]);
    let mut m = HmacSha256::new(&key);
    m.update(data);
    let r = m.finalize_reset();
    [r[0], r[1]]
}

/// The AES-128 cipher of a channel or contact secret (its first 16 bytes). `None` if the secret is shorter.
fn cipher(secret: &[u8]) -> Option<Aes> {
    let k = AesKey::from_slice(secret.get(..16)?).ok()?;
    Some(Aes::new(&k))
}

/// MAC then decrypt (AES-128 ECB): `None` when the MAC does not match or the ciphertext is not a non-empty multiple of 16
/// bytes.
pub fn mac_then_decrypt(secret: &[u8], mac: &[u8], ct: &[u8]) -> Option<Vec<u8>> {
    if ct.is_empty() || !ct.len().is_multiple_of(16) || mac.len() < 2 {
        return None;
    }
    if mac2(secret, ct) != [mac[0], mac[1]] {
        return None;
    }
    let mut out = ct.to_vec();
    cipher(secret)?.decrypt_ecb(&mut out).ok()?;
    Some(out)
}

/// Encrypt then MAC (AES-128 ECB): the MAC (2 bytes) followed by the ciphertext (zero-padded to 16 bytes). `None` if the
/// secret is shorter than 16 bytes.
pub fn encrypt_then_mac(secret: &[u8], plain: &[u8]) -> Option<Vec<u8>> {
    let mut ct = plain.to_vec();
    let pad = (16 - ct.len() % 16) % 16;
    ct.extend(std::iter::repeat_n(0, if ct.is_empty() { 16 } else { pad }));
    cipher(secret)?.encrypt_ecb(&mut ct).ok()?;
    let m = mac2(secret, &ct);
    let mut out = m.to_vec();
    out.extend_from_slice(&ct);
    Some(out)
}

/// An own identity: an Ed25519 key pair.
#[derive(Clone)]
pub struct Identity {
    key: Ed25519Key,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Identity({})", hex(&self.public()[..6]))
    }
}

impl Identity {
    /// From a 32-byte seed.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Identity {
            key: Ed25519Key::from_seed(&Key::new(*seed)),
        }
    }
    /// A fresh identity from the random source of the operating system.
    pub fn generate() -> Self {
        let mut s = [0u8; 32];
        if getrandom::getrandom(&mut s).is_err() {
            s = sha256(&crate::mesh::os_seed().to_le_bytes());
        }
        Self::from_seed(&s)
    }
    /// The public key.
    pub fn public(&self) -> [u8; 32] {
        self.key.public_key()
    }
    /// The node hash (first public key byte).
    pub fn hash(&self) -> u8 {
        self.public()[0]
    }
    /// The 32-byte shared secret with a peer: X25519 of the two keys converted to their X25519 forms. `None` if the peer's key
    /// is not a curve point or is of small order.
    pub fn shared_secret(&self, peer: &[u8; 32]) -> Option<[u8; 32]> {
        let mont = curve::ed25519_public_to_x25519(peer).ok()?;
        let k = self.key.x25519_secret().diffie_hellman(&mont).ok()?;
        Some(*k.as_bytes())
    }
    /// Signs.
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.key.sign(msg)
    }
}

/// Verifies an Ed25519 signature with the strict rules of RFC 8032.
pub fn verify(public: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
    curve::ed25519_verify(public, msg, sig).is_ok()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Adverts.

/// A decoded advert.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Advert {
    /// Public key.
    pub public_key: [u8; 32],
    /// Timestamp.
    pub timestamp: u32,
    /// Signature good.
    pub signature_ok: bool,
    /// Role number.
    pub role: u8,
    /// Position.
    pub position: Option<(f64, f64)>,
    /// Name.
    pub name: String,
    /// App data cut short.
    pub incomplete: bool,
}

/// Role names.
pub fn role_name(r: u8) -> &'static str {
    match r {
        1 => "Chat",
        2 => "Repeater",
        3 => "Room server",
        4 => "Sensor",
        _ => "Unknown",
    }
}

/// Parses an advert payload (at least 101 bytes).
pub fn parse_advert(p: &[u8]) -> Option<Advert> {
    if p.len() < 101 {
        return None;
    }
    let pk: [u8; 32] = p[0..32].try_into().ok()?;
    let ts = u32::from_le_bytes(p[32..36].try_into().ok()?);
    let sig: [u8; 64] = p[36..100].try_into().ok()?;
    let app = &p[100..p.len().min(132)];
    let mut msg = Vec::with_capacity(32 + 4 + app.len());
    msg.extend_from_slice(&pk);
    msg.extend_from_slice(&p[32..36]);
    msg.extend_from_slice(app);
    let mut a = Advert {
        public_key: pk,
        timestamp: ts,
        signature_ok: verify(&pk, &msg, &sig),
        ..Default::default()
    };
    let flags = app[0];
    a.role = flags & 0x0F;
    let mut at = 1;
    if flags & 0x10 != 0 {
        if app.len() < at + 8 {
            a.incomplete = true;
            return Some(a);
        }
        let lat = i32::from_le_bytes(app[at..at + 4].try_into().ok()?) as f64 * 1e-6;
        let lon = i32::from_le_bytes(app[at + 4..at + 8].try_into().ok()?) as f64 * 1e-6;
        if !(lat == 0.0 && lon == 0.0) {
            a.position = Some((lat, lon));
        }
        at += 8;
    }
    for bit in [0x20u8, 0x40] {
        if flags & bit != 0 {
            if app.len() < at + 2 {
                a.incomplete = true;
                return Some(a);
            }
            at += 2;
        }
    }
    if flags & 0x80 != 0 {
        let rest = &app[at.min(app.len())..];
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        a.name = String::from_utf8_lossy(&rest[..end]).into_owned();
    }
    Some(a)
}

/// Builds an advert payload, signed.
pub fn build_advert(
    id: &Identity,
    timestamp: u32,
    role: u8,
    position: Option<(f64, f64)>,
    name: &str,
) -> Vec<u8> {
    let mut app = Vec::new();
    let mut flags = role & 0x0F;
    if position.is_some() {
        flags |= 0x10;
    }
    if !name.is_empty() {
        flags |= 0x80;
    }
    app.push(flags);
    if let Some((lat, lon)) = position {
        app.extend_from_slice(&((lat * 1e6).round() as i32).to_le_bytes());
        app.extend_from_slice(&((lon * 1e6).round() as i32).to_le_bytes());
    }
    let room = 32usize.saturating_sub(app.len());
    app.extend_from_slice(super::meshtastic::cut_bytes(name, room).as_bytes());
    let pk = id.public();
    let mut msg = pk.to_vec();
    msg.extend_from_slice(&timestamp.to_le_bytes());
    msg.extend_from_slice(&app);
    let sig = id.sign(&msg);
    let mut out = pk.to_vec();
    out.extend_from_slice(&timestamp.to_le_bytes());
    out.extend_from_slice(&sig);
    out.extend_from_slice(&app);
    out
}

/// Clean text: control characters become blanks.
pub fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect()
}

/// A decoded group text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupText {
    /// The sender's clock.
    pub timestamp: u32,
    /// Text type.
    pub text_type: u8,
    /// Attempt.
    pub attempt: u8,
    /// Sender name (empty when unknown).
    pub sender: String,
    /// The text.
    pub text: String,
}

/// Parses a group text plaintext.
pub fn parse_group_text(p: &[u8]) -> Result<GroupText, ParseError> {
    if p.len() < 5 {
        return Err(ParseError::Short(Part::GroupText));
    }
    let ts = u32::from_le_bytes(
        p[0..4]
            .try_into()
            .expect("the range has the array's length"),
    );
    let flags = p[4];
    let tt = flags >> 2;
    if tt != 0 {
        return Err(ParseError::TextType(tt));
    }
    let rest = &p[5..];
    let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
    let line = clean(&String::from_utf8_lossy(&rest[..end]));
    let (sender, text) = match line.split_once(": ") {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (String::new(), line),
    };
    Ok(GroupText {
        timestamp: ts,
        text_type: tt,
        attempt: flags & 3,
        sender,
        text,
    })
}

/// The plaintext of a text (group or direct): timestamp, flags, text.
pub fn text_plain(timestamp: u32, attempt: u8, text: &str) -> Vec<u8> {
    let mut p = timestamp.to_le_bytes().to_vec();
    p.push(attempt & 3);
    p.extend_from_slice(text.as_bytes());
    p
}

/// A group text packet (GRP_TXT, flood) on a channel. The text is cut so that the payload stays at most 184 bytes. `None` if the
/// secret is shorter than 16 bytes.
pub fn build_group_text(
    secret: &[u8],
    timestamp: u32,
    sender: &str,
    text: &str,
    path: Vec<Vec<u8>>,
) -> Option<Vec<u8>> {
    // payload = hash + MAC + ciphertext (multiple of 16). 184 - 3 = 181 -> 176 bytes of ciphertext -> 171 of text line.
    let max_line = 176 - 5;
    let line = format!("{sender}: {text}");
    let line = super::meshtastic::cut_bytes(&line, max_line).to_string();
    let plain = text_plain(timestamp, 0, &line);
    let mut payload = vec![channel_hash(secret)];
    payload.extend_from_slice(&encrypt_then_mac(secret, &plain)?);
    Some(
        McPacket {
            route: 1,
            ptype: 5,
            version: 0,
            hash_size: 1,
            path,
            payload,
        }
        .encode(),
    )
}

/// The expected acknowledgement code of a text: the first 4 bytes of SHA-256 over the plaintext and the sender's public key.
pub fn ack_code(plain: &[u8], sender_public: &[u8; 32]) -> u32 {
    let mut h = Sha256::new();
    h.update(plain);
    h.update(sender_public);
    let d = h.finalize_reset();
    u32::from_le_bytes([d[0], d[1], d[2], d[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_secret() {
        assert_eq!(
            base64_decode("izOH6cXN6mrJ5e26oRXNcg==").unwrap(),
            PUBLIC_SECRET.to_vec()
        );
        assert_eq!(channel_hash(&PUBLIC_SECRET), 0x11);
        assert_eq!(
            hex(&hashtag_secret("#test")),
            "9cd8fcf22a47333b591d96a2b848b73f"
        );
    }

    #[test]
    fn group_text() {
        let pkt = build_group_text(
            &PUBLIC_SECRET,
            1_760_000_000,
            "Dubai Repeater",
            "hello mesh",
            vec![vec![0xAB]],
        )
        .unwrap();
        assert_eq!(
            hex(&pkt),
            "1501ab11f920a61f33a7906d3a9dcd1751d8c3318196e47c7d5dbda404b869f4b2ed4e459842"
        );
        let p = McPacket::parse(&pkt).unwrap();
        assert_eq!(
            (p.route, p.ptype, p.version, p.path_text()),
            (1, 5, 0, "ab".to_string())
        );
        let plain = mac_then_decrypt(&PUBLIC_SECRET, &p.payload[1..3], &p.payload[3..]).unwrap();
        let g = parse_group_text(&plain).unwrap();
        assert_eq!(
            (g.sender.as_str(), g.text.as_str(), g.timestamp),
            ("Dubai Repeater", "hello mesh", 1_760_000_000)
        );
    }

    #[test]
    fn adverts_sign_and_verify() {
        let id = Identity::from_seed(&[7u8; 32]);
        let a = build_advert(&id, 1_760_000_000, 2, Some((25.2, 55.3)), "Test Repeater");
        let pkt = McPacket {
            route: 1,
            ptype: 4,
            version: 0,
            hash_size: 1,
            path: vec![],
            payload: a.clone(),
        }
        .encode();
        assert_eq!(pkt[0], 0x11);
        assert_eq!(&pkt[34..38], &[0x00, 0x78, 0xE7, 0x68]);
        assert_eq!(pkt[102], 0x92);
        let d = parse_advert(&a).unwrap();
        assert!(d.signature_ok);
        assert_eq!(d.name, "Test Repeater");
        assert_eq!(role_name(d.role), "Repeater");
        let mut bad = a.clone();
        bad[40] ^= 1;
        assert!(!parse_advert(&bad).unwrap().signature_ok);
    }

    #[test]
    fn shared_secrets_agree() {
        let a = Identity::from_seed(&[1u8; 32]);
        let b = Identity::from_seed(&[2u8; 32]);
        assert_eq!(a.shared_secret(&b.public()), b.shared_secret(&a.public()));
    }

    #[test]
    fn the_radio_setting() {
        let lp = params(8, 62.5e3, 8);
        assert_eq!((lp.sync, lp.preamble, lp.ldro_on()), (0x12, 32, false));
        assert_eq!(params(9, 125e3, 5).preamble, 16);
    }
}
