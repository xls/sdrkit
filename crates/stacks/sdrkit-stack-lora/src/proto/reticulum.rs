//! Reticulum packets on RNode LoRa radios: the RNode radio header with its split packets, the packet header, and the announce
//! with its two checks. Nothing is decrypted.
//! Document: "Reticulum Network Stack Manual", section "Wire Format".

use sdrkit_algo_crypto::curve::{ed25519_verify, Ed25519Key, X25519Secret};
use sdrkit_algo_crypto::hash::sha256;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// The LoRa sync word of RNode radios.
pub const SYNC: u8 = 0x12;
/// The shortest Reticulum packet.
pub const MIN_PACKET: usize = 19;
/// A stored first part of a split packet is forgotten after this time, seconds.
pub const SPLIT_HOLD_S: f64 = 5.0;

/// Bytes as lower-case hex digits.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(2 * b.len());
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

/// The first part of a split packet, kept until its second part comes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SplitStore {
    part: Option<(u8, f64, Vec<u8>)>,
}

impl SplitStore {
    /// Takes the payload of one LoRa frame heard at `t` seconds. Returns the Reticulum packet when the frame completes one.
    pub fn take(&mut self, t: f64, payload: &[u8]) -> Option<Vec<u8>> {
        if payload.len() < 2 {
            return None;
        }
        let seq = payload[0] >> 4;
        let body = &payload[1..];
        if payload[0] & 1 == 0 {
            self.part = None;
            return Some(body.to_vec());
        }
        match self.part.take() {
            Some((s, at, mut first)) if s == seq && t - at <= SPLIT_HOLD_S => {
                first.extend_from_slice(body);
                Some(first)
            }
            _ => {
                self.part = Some((seq, t, body.to_vec()));
                None
            }
        }
    }

    /// Forgets the stored part.
    pub fn clear(&mut self) {
        self.part = None;
    }
}

sdrkit_core::choice_enum! {
    /// The packet type.
    #[derive(Serialize, Deserialize)]
    pub enum PacketType {
        /// Data.
        Data = ("data", "DATA"),
        /// Announce.
        Announce = ("announce", "ANNOUNCE"),
        /// Link request.
        LinkRequest = ("linkrequest", "LINKREQUEST"),
        /// Proof.
        Proof = ("proof", "PROOF"),
    }
}

/// The destination types, by their two bits.
pub const DESTINATION_TYPES: [&str; 4] = ["single", "group", "plain", "link"];

/// Why bytes are not a Reticulum packet that the receiver shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReticulumError {
    /// Fewer than 19 bytes.
    #[error("shorter than a Reticulum packet")]
    Short,
    /// A hop count of 128 or more.
    #[error("the hop count is out of range")]
    Hops,
    /// The addresses, the context byte or the first data byte is missing.
    #[error("the packet is cut off")]
    CutOff,
    /// An announce shorter than its fixed parts.
    #[error("the announce is shorter than its fixed parts")]
    ShortAnnounce,
    /// The signature of an announce fails.
    #[error("the signature of the announce is wrong")]
    Signature,
    /// The destination hash of an announce does not follow from its public key and name hash.
    #[error("the destination hash does not belong to the announced identity")]
    Destination,
}

/// A good announce.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announce {
    /// The identity hash: the first 16 bytes of SHA-256 of the public key.
    pub identity: [u8; 16],
    /// The name hash.
    pub name_hash: [u8; 10],
    /// A ratchet key is present.
    pub ratchet: bool,
    /// The printable characters of the first 40 bytes of the application data, when there are 3 or more.
    pub text: Option<String>,
}

/// A Reticulum packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Packet {
    /// The flags byte.
    pub flags: u8,
    /// The hop count.
    pub hops: u8,
    /// The packet carries an interface access code: nothing after the hop count can be read without the key of the interface.
    pub access: bool,
    /// The transport id of a type 2 header.
    pub transport_id: Option<[u8; 16]>,
    /// The destination hash.
    pub destination: [u8; 16],
    /// The context byte.
    pub context: u8,
    /// The length of the data.
    pub data_len: usize,
    /// The checked announce, for an announce packet.
    pub announce: Option<Announce>,
}

impl Packet {
    /// The packet type.
    pub fn kind(&self) -> PacketType {
        match self.flags & 3 {
            0 => PacketType::Data,
            1 => PacketType::Announce,
            2 => PacketType::LinkRequest,
            _ => PacketType::Proof,
        }
    }
    /// The destination type: 0 single, 1 group, 2 plain, 3 link.
    pub fn destination_type(&self) -> u8 {
        (self.flags >> 2) & 3
    }
    /// The header has two addresses.
    pub fn header2(&self) -> bool {
        self.flags & 0x40 != 0
    }
    /// The propagation is transport, not broadcast.
    pub fn transport(&self) -> bool {
        self.flags & 0x10 != 0
    }
    /// The context flag.
    pub fn context_flag(&self) -> bool {
        self.flags & 0x20 != 0
    }
    /// A data packet whose destination is not plain: its data is encrypted.
    pub fn encrypted(&self) -> bool {
        self.kind() == PacketType::Data && self.destination_type() != 2
    }

    /// The content in one line.
    pub fn content(&self) -> String {
        if self.access {
            return format!("authenticated interface, {} bytes", self.data_len);
        }
        if let Some(a) = &self.announce {
            let mut s = format!(
                "identity {}, {} hops, signature good",
                hex(&a.identity[..8]),
                self.hops
            );
            if let Some(t) = &a.text {
                let _ = write!(s, ", \"{t}\"");
            }
            return s;
        }
        let mut s = format!(
            "{}, {} hops, {} bytes",
            DESTINATION_TYPES[usize::from(self.destination_type())],
            self.hops,
            self.data_len
        );
        if let Some(t) = self.transport_id.filter(|_| self.transport()) {
            let _ = write!(s, ", via {}", hex(&t[..4]));
        }
        if self.context != 0 {
            let _ = write!(s, ", context 0x{:02X}", self.context);
        }
        s
    }
}

/// The destination hash of an identity with public key `public` (X25519 then Ed25519) for the name hash `name_hash`.
pub fn destination_hash(public: &[u8; 64], name_hash: &[u8; 10]) -> ([u8; 16], [u8; 16]) {
    let mut identity = [0u8; 16];
    identity.copy_from_slice(&sha256(public)[..16]);
    let mut m = [0u8; 26];
    m[..10].copy_from_slice(name_hash);
    m[10..].copy_from_slice(&identity);
    let mut dest = [0u8; 16];
    dest.copy_from_slice(&sha256(&m)[..16]);
    (identity, dest)
}

/// The first 10 bytes of SHA-256 of the full name of a destination ("app.aspect").
pub fn name_hash(name: &str) -> [u8; 10] {
    let mut h = [0u8; 10];
    h.copy_from_slice(&sha256(name.as_bytes())[..10]);
    h
}

fn check_announce(
    destination: &[u8; 16],
    ratchet: bool,
    d: &[u8],
) -> Result<Announce, ReticulumError> {
    let r = if ratchet { 32 } else { 0 };
    if d.len() < 64 + 10 + 10 + r + 64 {
        return Err(ReticulumError::ShortAnnounce);
    }
    let (public, rest) = d.split_at(64);
    let (names, rest) = rest.split_at(20 + r);
    let (signature, app) = rest.split_at(64);
    let mut signed = Vec::with_capacity(16 + d.len() - 64);
    signed.extend_from_slice(destination);
    signed.extend_from_slice(public);
    signed.extend_from_slice(names);
    signed.extend_from_slice(app);
    let key: [u8; 32] = public[32..]
        .try_into()
        .map_err(|_| ReticulumError::ShortAnnounce)?;
    let sig: [u8; 64] = signature
        .try_into()
        .map_err(|_| ReticulumError::ShortAnnounce)?;
    ed25519_verify(&key, &signed, &sig).map_err(|_| ReticulumError::Signature)?;
    let public: [u8; 64] = public
        .try_into()
        .map_err(|_| ReticulumError::ShortAnnounce)?;
    let mut name_hash = [0u8; 10];
    name_hash.copy_from_slice(&names[..10]);
    let (identity, dest) = destination_hash(&public, &name_hash);
    if &dest != destination {
        return Err(ReticulumError::Destination);
    }
    let text: String = app
        .iter()
        .take(40)
        .filter(|c| (32..127).contains(*c))
        .map(|&c| c as char)
        .collect();
    Ok(Announce {
        identity,
        name_hash,
        ratchet,
        text: (text.len() >= 3).then_some(text),
    })
}

/// Parses a Reticulum packet (the LoRa payload without the RNode header) and checks an announce.
///
/// # Errors
///
/// [`ReticulumError`] for bytes that are not a packet, and for an announce that fails a check.
pub fn parse(b: &[u8]) -> Result<Packet, ReticulumError> {
    if b.len() < MIN_PACKET {
        return Err(ReticulumError::Short);
    }
    let (flags, hops) = (b[0], b[1]);
    if hops >= 128 {
        return Err(ReticulumError::Hops);
    }
    let mut p = Packet {
        flags,
        hops,
        access: flags & 0x80 != 0,
        transport_id: None,
        destination: [0; 16],
        context: 0,
        data_len: b.len() - 2,
        announce: None,
    };
    if p.access {
        return Ok(p);
    }
    let mut i = 2;
    if p.header2() {
        let t = b.get(i..i + 16).ok_or(ReticulumError::CutOff)?;
        p.transport_id = t.try_into().ok();
        i += 16;
    }
    let d = b.get(i..i + 16).ok_or(ReticulumError::CutOff)?;
    p.destination.copy_from_slice(d);
    i += 16;
    p.context = *b.get(i).ok_or(ReticulumError::CutOff)?;
    i += 1;
    let data = b.get(i..).filter(|d| !d.is_empty());
    let data = data.ok_or(ReticulumError::CutOff)?;
    p.data_len = data.len();
    if p.kind() == PacketType::Announce {
        p.announce = Some(check_announce(&p.destination, p.context_flag(), data)?);
    }
    Ok(p)
}

/// An identity that signs announces, for the test signal and for tests.
#[derive(Clone, Debug)]
pub struct Identity {
    public: [u8; 64],
    signer: Ed25519Key,
}

impl Identity {
    /// The identity of an X25519 private key and an Ed25519 seed.
    pub fn new(x25519: [u8; 32], ed25519_seed: [u8; 32]) -> Identity {
        let signer = Ed25519Key::from_seed(&sdrkit_algo_crypto::key::Key::new(ed25519_seed));
        let mut public = [0u8; 64];
        public[..32].copy_from_slice(
            &X25519Secret::new(sdrkit_algo_crypto::key::Key::new(x25519)).public_key(),
        );
        public[32..].copy_from_slice(&signer.public_key());
        Identity { public, signer }
    }

    /// The public key: X25519 then Ed25519.
    pub fn public(&self) -> &[u8; 64] {
        &self.public
    }

    /// The destination hash of the identity for the full name `name`.
    pub fn destination(&self, name: &str) -> [u8; 16] {
        destination_hash(&self.public, &name_hash(name)).1
    }

    /// An announce packet of the destination `name` with a type 1 header: `hops` hops, the random hash `random`, a ratchet key when
    /// given, and the application data `app`.
    pub fn announce(
        &self,
        name: &str,
        hops: u8,
        random: [u8; 10],
        ratchet: Option<[u8; 32]>,
        app: &[u8],
    ) -> Vec<u8> {
        let nh = name_hash(name);
        let dest = destination_hash(&self.public, &nh).1;
        let mut names = Vec::with_capacity(52);
        names.extend_from_slice(&nh);
        names.extend_from_slice(&random);
        if let Some(r) = ratchet {
            names.extend_from_slice(&r);
        }
        let mut signed = Vec::new();
        signed.extend_from_slice(&dest);
        signed.extend_from_slice(&self.public);
        signed.extend_from_slice(&names);
        signed.extend_from_slice(app);
        let signature = self.signer.sign(&signed);
        let mut p = vec![0x01 | u8::from(ratchet.is_some()) << 5, hops];
        p.extend_from_slice(&dest);
        p.push(0);
        p.extend_from_slice(&self.public);
        p.extend_from_slice(&names);
        p.extend_from_slice(&signature);
        p.extend_from_slice(app);
        p
    }
}

/// A packet with a type 1 header: the flags, the hop count, the destination, the context and the data.
pub fn build(flags: u8, hops: u8, destination: &[u8; 16], context: u8, data: &[u8]) -> Vec<u8> {
    let mut p = vec![flags & !0x40, hops];
    p.extend_from_slice(destination);
    p.push(context);
    p.extend_from_slice(data);
    p
}

/// The LoRa payloads that carry `packet` on an RNode radio with the sequence number `seq`: one frame, or two with the split
/// flag when `split` is set or the packet is longer than 254 bytes. The first of two frames takes `first` bytes of the packet.
pub fn radio_frames(packet: &[u8], seq: u8, split: bool, first: usize) -> Vec<Vec<u8>> {
    let header = seq << 4;
    if !split && packet.len() <= 254 {
        let mut f = vec![header];
        f.extend_from_slice(packet);
        return vec![f];
    }
    let cut = first
        .clamp(1, 254)
        .min(packet.len().saturating_sub(1))
        .max(1);
    let (a, b) = packet.split_at(cut.min(packet.len()));
    [a, b]
        .iter()
        .map(|part| {
            let mut f = vec![header | 1];
            f.extend_from_slice(part);
            f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// The worked announce: seed 01 to 20, X25519 key 21 to 40, "lxmf.delivery", random hash A0 to A9, "node one", 2 hops.
    pub fn r1() -> Vec<u8> {
        let seed: [u8; 32] = std::array::from_fn(|i| i as u8 + 1);
        let x: [u8; 32] = std::array::from_fn(|i| i as u8 + 0x21);
        let random: [u8; 10] = std::array::from_fn(|i| 0xA0 + i as u8);
        Identity::new(x, seed).announce("lxmf.delivery", 2, random, None, b"node one")
    }

    #[test]
    fn the_flags_byte_gives_its_fields() {
        let mut b = vec![0x50, 0x04];
        b.extend([0x11; 16]);
        b.extend([0x22; 16]);
        b.extend([0, 1, 2, 3]);
        let p = parse(&b).unwrap();
        assert!(p.header2() && p.transport());
        assert_eq!(
            (p.destination_type(), p.kind(), p.hops),
            (0, PacketType::Data, 4)
        );
        assert_eq!(p.destination, [0x22; 16]);
        assert_eq!(p.content(), "single, 4 hops, 3 bytes, via 11111111");
        let mut b = vec![0x00, 0x07];
        b.extend([0x33; 20]);
        let p = parse(&b).unwrap();
        assert!(!p.header2() && !p.transport() && !p.access);
        assert_eq!((p.kind(), p.hops), (PacketType::Data, 7));
        b[0] = 0x80;
        let p = parse(&b).unwrap();
        assert!(p.access);
        assert_eq!(p.content(), "authenticated interface, 20 bytes");
    }

    #[test]
    fn a_split_packet_is_joined_from_two_frames_of_one_sequence_number() {
        let mut s = SplitStore::default();
        assert_eq!(s.take(0.0, &[0x31, 1, 2, 3]), None);
        assert_eq!(s.take(0.1, &[0x31, 4, 5]), Some(vec![1, 2, 3, 4, 5]));
        // The store is empty again: the same frame alone is a first part.
        assert_eq!(s.take(0.2, &[0x31, 4, 5]), None);
        // Another sequence number replaces the stored part.
        assert_eq!(s.take(0.3, &[0x21, 9]), None);
        assert_eq!(s.take(0.4, &[0x21, 8]), Some(vec![9, 8]));
        // A part that waited too long is not joined.
        assert_eq!(s.take(1.0, &[0x41, 1]), None);
        assert_eq!(s.take(7.0, &[0x41, 2]), None);
        // A frame without the flag is a whole packet, and a payload of one byte is dropped.
        assert_eq!(s.take(7.1, &[0x40, 7, 7]), Some(vec![7, 7]));
        assert_eq!(s.take(7.2, &[0x40]), None);
    }

    #[test]
    fn a_data_packet_to_a_single_destination_is_marked_encrypted() {
        let mut b = vec![0x00, 0x01];
        b.extend([0x77; 16]);
        b.push(0);
        b.extend([0xEE; 30]);
        let p = parse(&b).unwrap();
        assert_eq!(p.kind(), PacketType::Data);
        assert!(p.encrypted());
        assert_eq!(hex(&p.destination), "77".repeat(16));
        assert_eq!(p.content(), "single, 1 hops, 30 bytes");
        // A plain destination is not encrypted, and a context shows.
        b[0] = 0x08;
        b[18] = 0x0E;
        let p = parse(&b).unwrap();
        assert!(!p.encrypted());
        assert_eq!(p.content(), "plain, 1 hops, 30 bytes, context 0x0E");
    }

    #[test]
    fn the_worked_announce_has_the_bytes_of_the_specification() {
        let p = r1();
        assert_eq!(p.len(), 175);
        assert_eq!(p[..2], [0x01, 0x02]);
        assert_eq!(
            p[2..18],
            unhex("A4 F1 FB 4A DE 20 AA CC 46 AF 88 5C C8 C7 C9 E8")[..]
        );
        assert_eq!(p[18], 0);
        assert_eq!(
            p[19..51],
            unhex("58 69 AF F4 50 54 97 32 CB AA ED 5E 5D F9 B3 0A 6D A3 1C B0 E5 74 2B AD 5A D4 A1 A7 68 F1 A6 7B")[..]
        );
        assert_eq!(
            p[51..83],
            unhex("79 B5 56 2E 8F E6 54 F9 40 78 B1 12 E8 A9 8B A7 90 1F 85 3A E6 95 BE D7 E0 E3 91 0B AD 04 96 64")[..]
        );
        assert_eq!(p[83..93], unhex("6E C6 0B C3 18 E2 C0 F0 D9 08")[..]);
        assert_eq!(
            p[103..167],
            unhex(
                "73 0A 02 76 7D A0 8F A9 33 F7 B2 CE EE 60 7F 69 48 5C C7 A2 B3 DE BD 25 D7 F8 F0 42 62 34 A6 20 \
                 3E 99 1C 9A 71 86 62 3B 6A 60 58 7F 91 BA E4 F0 9C 16 DB 5A B7 E4 F7 16 7B 29 93 38 6C 42 87 07"
            )[..]
        );
        assert_eq!(&p[167..], b"node one");
        let a = parse(&p).unwrap();
        assert_eq!(a.kind(), PacketType::Announce);
        assert_eq!(hex(&a.destination), "a4f1fb4ade20aacc46af885cc8c7c9e8");
        let an = a.announce.as_ref().unwrap();
        assert_eq!(hex(&an.identity), "d3659460b83a4336c91a295c1dbc4432");
        assert_eq!(
            a.content(),
            "identity d3659460b83a4336, 2 hops, signature good, \"node one\""
        );
    }

    #[test]
    fn an_announce_with_any_bit_changed_is_refused() {
        let p = r1();
        // The context byte (18) is in neither check.
        for byte in (2..p.len()).filter(|b| *b != 18) {
            let mut q = p.clone();
            q[byte] ^= 1 << (byte % 8);
            assert!(parse(&q).is_err(), "byte {byte}");
        }
        // An announce with a ratchet key, and one that is cut off.
        let id = Identity::new([7; 32], [9; 32]);
        let with = id.announce("app.aspect", 0, [1; 10], Some([5; 32]), b"");
        assert_eq!(with.len(), 2 + 16 + 1 + 64 + 20 + 32 + 64);
        let a = parse(&with).unwrap();
        assert!(a.announce.as_ref().unwrap().ratchet);
        assert_eq!(
            a.content(),
            format!(
                "identity {}, 0 hops, signature good",
                hex(&sha256(id.public())[..8])
            )
        );
        assert_eq!(
            parse(&with[..with.len() - 1]),
            Err(ReticulumError::ShortAnnounce)
        );
    }

    #[test]
    fn short_packets_and_large_hop_counts_are_refused() {
        assert_eq!(parse(&[0; 18]), Err(ReticulumError::Short));
        let mut b = vec![0x00, 0x80];
        b.extend([0; 20]);
        assert_eq!(parse(&b), Err(ReticulumError::Hops));
        // A type 2 header needs both addresses, the context and one data byte.
        let mut b = vec![0x40, 0x01];
        b.extend([0; 33]);
        assert_eq!(parse(&b), Err(ReticulumError::CutOff));
        b.push(0);
        assert!(parse(&b).is_ok());
    }

    #[test]
    fn a_packet_goes_into_one_radio_frame_or_two() {
        let p = r1();
        let one = radio_frames(&p, 1, false, 0);
        assert_eq!(one.len(), 1);
        assert_eq!((one[0][0], one[0].len()), (0x10, 176));
        let two = radio_frames(&p, 5, true, 100);
        assert_eq!((two[0][0], two[1][0]), (0x51, 0x51));
        let mut s = SplitStore::default();
        assert_eq!(s.take(0.0, &two[0]), None);
        assert_eq!(s.take(0.5, &two[1]), Some(p));
    }
}
