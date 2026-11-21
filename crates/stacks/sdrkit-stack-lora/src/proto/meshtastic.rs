//! Meshtastic: modem presets and channel slots, the 16-byte radio header, channel keys and their hash,
//! AES-CTR channel encryption, the public-key (X25519 + AES-256-CCM) direct messages, the `Data` message and every port.

use sdrkit_algo_crypto::{sha256, Aes, AesKey, Ccm, Counter, Key, X25519Secret};
use sdrkit_core::base64::{self, Alphabet};
use sdrkit_proto_lora_phy::{Ldro, LoraParams};
use sdrkit_util_bits::protobuf::{self, Fields, Wire, Writer};
use serde::{Deserialize, Serialize};

/// The fields of a protobuf message, in order. `None` if the message is not well formed.
fn fields(b: &[u8]) -> Option<Vec<(u32, Wire<'_>)>> {
    Fields::new(b).collect::<Result<Vec<_>, _>>().ok()
}

/// The first field `num` of a message read with [`fields`].
fn get<'a>(fs: &[(u32, Wire<'a>)], num: u32) -> Option<Wire<'a>> {
    fs.iter().find(|f| f.0 == num).map(|f| f.1)
}

/// Every value of the repeated field `num`, packed or not: fixed32 values when `fixed32`, else varints.
fn repeated(fs: &[(u32, Wire<'_>)], num: u32, fixed32: bool) -> Vec<u64> {
    let mut v = Vec::new();
    for (n, w) in fs {
        if *n != num {
            continue;
        }
        match w {
            Wire::Bytes(b) if fixed32 => v.extend(protobuf::packed_fixed32(b).map(u64::from)),
            Wire::Bytes(b) => v.extend(protobuf::packed_varints(b).map_while(Result::ok)),
            w => v.push(w.u64()),
        }
    }
    v
}

/// A `string` field as text. Bytes that are not UTF-8 show as the replacement character.
fn text(w: &Wire<'_>) -> String {
    String::from_utf8_lossy(w.bytes()).into_owned()
}

/// The broadcast address.
pub const BROADCAST: u32 = 0xFFFF_FFFF;

/// The default channel key ("AQ==", index 1).
pub const DEFAULT_KEY: [u8; 16] = [
    0xD4, 0xF1, 0xBB, 0x3A, 0x20, 0x29, 0x07, 0x59, 0xF0, 0xBC, 0xFF, 0xAB, 0xCF, 0x4E, 0x69, 0x01,
];

// ---------------------------------------------------------------------------------------------------------------------------------
// Presets and channel slots.

pub use sdrkit_proto_meshtastic::{
    check_channel_num, djb2, preset, preset_bw, slot_centre_hz, slot_count, slot_hz, slot_index,
    Preset, SlotBand, PRESETS, WIDE_LORA_FROM_HZ,
};

/// The LoRa setting of a preset in a band (preamble 16, sync 0x2B, explicit header, CRC on, LDRO automatic).
pub fn lora_params(p: &Preset, band: &SlotBand) -> LoraParams {
    LoraParams {
        sf: p.sf,
        bw: preset_bw(p, band),
        cr: p.cr,
        preamble: 16,
        sync: 0x2B,
        ldro: Ldro::Auto,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The radio header.

/// The 16-byte radio header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// Destination node number.
    pub to: u32,
    /// Sender node number.
    pub from: u32,
    /// Packet id.
    pub id: u32,
    /// Hop limit (0..7).
    pub hop_limit: u8,
    /// Want-ack.
    pub want_ack: bool,
    /// Via MQTT.
    pub via_mqtt: bool,
    /// Hop start (0..7).
    pub hop_start: u8,
    /// Channel hash (0 for a public-key direct message).
    pub channel: u8,
    /// Next hop: the last byte of the next relay (0 = no preference).
    pub next_hop: u8,
    /// Relay node: the last byte of the node that sent this copy.
    pub relay: u8,
}

impl Header {
    /// Parses the first 16 bytes.
    pub fn parse(b: &[u8]) -> Option<Header> {
        if b.len() < 16 {
            return None;
        }
        let u = |i: usize| {
            u32::from_le_bytes(
                b[i..i + 4]
                    .try_into()
                    .expect("the range has the array's length"),
            )
        };
        let fl = b[12];
        Some(Header {
            to: u(0),
            from: u(4),
            id: u(8),
            hop_limit: fl & 7,
            want_ack: fl & 8 != 0,
            via_mqtt: fl & 16 != 0,
            hop_start: fl >> 5,
            channel: b[13],
            next_hop: b[14],
            relay: b[15],
        })
    }
    /// The 16 bytes.
    pub fn bytes(&self) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[0..4].copy_from_slice(&self.to.to_le_bytes());
        b[4..8].copy_from_slice(&self.from.to_le_bytes());
        b[8..12].copy_from_slice(&self.id.to_le_bytes());
        b[12] = (self.hop_limit & 7)
            | if self.want_ack { 8 } else { 0 }
            | if self.via_mqtt { 16 } else { 0 }
            | ((self.hop_start & 7) << 5);
        b[13] = self.channel;
        b[14] = self.next_hop;
        b[15] = self.relay;
        b
    }
    /// Hops travelled: hop start - hop limit when hop start is known and at least the limit. -1 otherwise.
    pub fn hops(&self) -> i32 {
        if self.hop_start > 0 && self.hop_start >= self.hop_limit {
            (self.hop_start - self.hop_limit) as i32
        } else {
            -1
        }
    }
}

/// "!xxxxxxxx", or "^all" for broadcast.
pub fn node_name(n: u32) -> String {
    if n == BROADCAST {
        "^all".into()
    } else {
        format!("!{n:08x}")
    }
}

/// A node number from "!xxxxxxxx" or 8 hex digits.
pub fn parse_node(s: &str) -> Option<u32> {
    let t = s.trim().trim_start_matches('!');
    if t.len() != 8 {
        return None;
    }
    u32::from_str_radix(t, 16).ok()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Keys.

/// Expands a pre-shared key: `None` refused, `Some(empty)` no encryption.
pub fn expand_key(psk: &[u8]) -> Option<Vec<u8>> {
    match psk.len() {
        0 => Some(Vec::new()),
        1 if psk[0] == 0 => Some(Vec::new()),
        1 => {
            let mut k = DEFAULT_KEY.to_vec();
            k[15] = k[15].wrapping_add(psk[0] - 1);
            Some(k)
        }
        2..=16 => {
            let mut k = psk.to_vec();
            k.resize(16, 0);
            Some(k)
        }
        17..=32 => {
            let mut k = psk.to_vec();
            k.resize(32, 0);
            Some(k)
        }
        _ => None,
    }
}

/// The bytes of base64 text (RFC 4648 section 4 or 5, padding optional, whitespace ignored). `None` if it is not base64.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let alphabet = if t.contains(['-', '_']) {
        Alphabet::UrlSafe
    } else {
        Alphabet::Standard
    };
    let mut out = Vec::new();
    base64::decode_into(&t, alphabet, &mut out).ok()?;
    Some(out)
}

/// Base64 text with padding (RFC 4648 section 4).
pub fn base64_encode(b: &[u8]) -> String {
    let mut s = String::new();
    base64::encode_into(b, Alphabet::Standard, true, &mut s);
    s
}

/// The bytes of hex text. Spaces and colons between the digits are ignored. `None` if it is empty or not hex.
pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let t: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .collect();
    if t.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    base64::unhex_into(&t, &mut out).ok()?;
    Some(out)
}

/// Lower-case hex text.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::new();
    base64::hex_into(b, &mut s);
    s.make_ascii_lowercase();
    s
}

/// A key text: base64, hex of 32 or 64 digits, "default", "none" / "off". Returns the expanded key (empty: no encryption).
pub fn parse_key_text(s: &str) -> Option<Vec<u8>> {
    let t = s.trim();
    match t.to_ascii_lowercase().as_str() {
        "default" => return Some(DEFAULT_KEY.to_vec()),
        "none" | "off" => return Some(Vec::new()),
        _ => {}
    }
    let compact: String = t
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .collect();
    if (compact.len() == 32 || compact.len() == 64)
        && compact.chars().all(|c| c.is_ascii_hexdigit())
    {
        return expand_key(&hex_decode(&compact)?);
    }
    expand_key(&base64_decode(t)?)
}

/// The channel hash: XOR of the name's bytes and the expanded key's bytes.
pub fn channel_hash(name: &str, key: &[u8]) -> u8 {
    name.bytes()
        .chain(key.iter().copied())
        .fold(0, |a, b| a ^ b)
}

/// A known channel: name and expanded key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    /// The name.
    pub name: String,
    /// The expanded key (empty: no encryption).
    pub key: Vec<u8>,
    /// Added by the user (not a default).
    pub user: bool,
}

impl Channel {
    /// The hash.
    pub fn hash(&self) -> u8 {
        channel_hash(&self.name, &self.key)
    }
}

/// The channels at start: the default channel of every preset with the default key.
pub fn default_channels() -> Vec<Channel> {
    [
        "LongFast",
        "LongSlow",
        "LongTurbo",
        "LongMod",
        "LongModerate",
        "MediumFast",
        "MediumSlow",
        "ShortFast",
        "ShortSlow",
        "ShortTurbo",
        "EdgeFastLow",
    ]
    .iter()
    .map(|n| Channel {
        name: n.to_string(),
        key: DEFAULT_KEY.to_vec(),
        user: false,
    })
    .collect()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Crypto.

/// The AES-CTR counter block of a packet: id as 64-bit LE, sender as 32-bit LE, four zero bytes.
pub fn ctr_nonce(id: u32, from: u32) -> [u8; 16] {
    let mut n = [0u8; 16];
    n[0..4].copy_from_slice(&id.to_le_bytes());
    n[8..12].copy_from_slice(&from.to_le_bytes());
    n
}

/// AES-CTR with a 16- or 32-byte key (encryption and decryption are the same). With no key or a key of another length, the data
/// stays as it is. The counter block is [`ctr_nonce`], and the whole block counts up (big-endian).
pub fn aes_ctr(key: &[u8], id: u32, from: u32, data: &mut [u8]) {
    if key.len() != 16 && key.len() != 32 {
        return;
    }
    let Ok(k) = AesKey::from_slice(key) else {
        return;
    };
    // A packet is far shorter than 2^128 blocks, so the counter cannot run out.
    let _ = Aes::new(&k).apply_ctr(&ctr_nonce(id, from), Counter::Full128, data);
}

/// The PKI shared key: SHA-256 of the X25519 shared secret. `None` if the peer's key is a point of small order.
pub fn pki_shared_key(own_private: &[u8; 32], peer_public: &[u8; 32]) -> Option<[u8; 32]> {
    let s = X25519Secret::new(Key::new(*own_private));
    let shared = s.diffie_hellman(peer_public).ok()?;
    Some(sha256(shared.as_bytes()))
}

/// The X25519 public key of a private key.
pub fn public_key(private: &[u8; 32]) -> [u8; 32] {
    X25519Secret::new(Key::new(*private)).public_key()
}

/// A fresh clamped X25519 private key from the random source of the operating system.
pub fn new_private_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    if getrandom::getrandom(&mut k).is_err() {
        k = sha256(&crate::mesh::os_seed().to_le_bytes());
    }
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;
    k
}

fn pki_nonce(id: u32, extra: u32, from: u32) -> [u8; 13] {
    let mut n = [0u8; 13];
    n[0..4].copy_from_slice(&id.to_le_bytes());
    n[4..8].copy_from_slice(&extra.to_le_bytes());
    n[8..12].copy_from_slice(&from.to_le_bytes());
    n
}

/// Encrypts a Data message for a PKI direct message (AES-256-CCM, 8-byte tag, 13-byte nonce): ciphertext, tag, extra nonce
/// (little endian). `None` if the message is too long for CCM.
pub fn pki_encrypt(
    shared: &[u8; 32],
    id: u32,
    from: u32,
    extra: u32,
    plain: &[u8],
) -> Option<Vec<u8>> {
    let c = Ccm::<8, 13>::new(&AesKey::Aes256(Key::new(*shared)));
    let mut out = plain.to_vec();
    let tag = c
        .encrypt_in_place(&pki_nonce(id, extra, from), &[], &mut out)
        .ok()?;
    out.extend_from_slice(&tag);
    out.extend_from_slice(&extra.to_le_bytes());
    Some(out)
}

/// Decrypts a PKI body. `None` if it is too short or the tag check fails.
pub fn pki_decrypt(shared: &[u8; 32], id: u32, from: u32, body: &[u8]) -> Option<Vec<u8>> {
    if body.len() < 12 {
        return None;
    }
    let (rest, extra) = body.split_at(body.len() - 4);
    let (ct, tag) = rest.split_at(rest.len() - 8);
    let extra = u32::from_le_bytes(extra.try_into().ok()?);
    let tag: [u8; 8] = tag.try_into().ok()?;
    let c = Ccm::<8, 13>::new(&AesKey::Aes256(Key::new(*shared)));
    let mut out = ct.to_vec();
    c.decrypt_in_place(&pki_nonce(id, extra, from), &[], &mut out, &tag)
        .ok()?;
    Some(out)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The Data message.

/// A parsed `Data` message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Data {
    /// Port number.
    pub port: u32,
    /// Payload.
    pub payload: Vec<u8>,
    /// Want response.
    pub want_response: bool,
    /// dest.
    pub dest: u32,
    /// source.
    pub source: u32,
    /// The packet this answers.
    pub request_id: u32,
    /// The message this replies to.
    pub reply_id: u32,
    /// A reaction.
    pub emoji: u32,
    /// Bitfield (bit 0: OK to upload).
    pub bitfield: Option<u32>,
}

impl Data {
    /// Parses with the acceptance rule of 5.5: first field 1 (varint, 1..511), every field number at most 10, no malformed field.
    pub fn parse_strict(b: &[u8]) -> Option<Data> {
        let fs = fields(b)?;
        let first = fs.first()?;
        match first {
            (1, Wire::Varint(v)) if (1..=511).contains(v) => {}
            _ => return None,
        }
        if fs.iter().any(|f| f.0 > 10) {
            return None;
        }
        Some(Self::from_fields(&fs))
    }
    /// Parses leniently (unknown fields skipped).
    pub fn parse(b: &[u8]) -> Option<Data> {
        Some(Self::from_fields(&fields(b)?))
    }
    fn from_fields(fs: &[(u32, Wire<'_>)]) -> Data {
        let mut d = Data::default();
        for (n, w) in fs {
            match n {
                1 => d.port = w.u64() as u32,
                2 => d.payload = w.bytes().to_vec(),
                3 => d.want_response = w.bool(),
                4 => d.dest = w.u64() as u32,
                5 => d.source = w.u64() as u32,
                6 => d.request_id = w.u64() as u32,
                7 => d.reply_id = w.u64() as u32,
                8 => d.emoji = w.u64() as u32,
                9 => d.bitfield = Some(w.u64() as u32),
                _ => {}
            }
        }
        d
    }
    /// The bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut w_buf = Vec::new();
        let mut w = Writer::new(&mut w_buf);
        w.uint(1, self.port as u64);
        if !self.payload.is_empty() {
            w.bytes(2, &self.payload);
        }
        if self.want_response {
            w.uint(3, 1);
        }
        if self.dest != 0 {
            w.fixed32(4, self.dest);
        }
        if self.source != 0 {
            w.fixed32(5, self.source);
        }
        if self.request_id != 0 {
            w.fixed32(6, self.request_id);
        }
        if self.reply_id != 0 {
            w.fixed32(7, self.reply_id);
        }
        if self.emoji != 0 {
            w.fixed32(8, self.emoji);
        }
        if let Some(b) = self.bitfield {
            w.uint(9, b as u64);
        }
        w_buf
    }
}

/// The port names of portnums.proto.
pub fn port_name(p: u32) -> String {
    let s = match p {
        0 => "UNKNOWN_APP",
        1 => "TEXT_MESSAGE_APP",
        2 => "REMOTE_HARDWARE_APP",
        3 => "POSITION_APP",
        4 => "NODEINFO_APP",
        5 => "ROUTING_APP",
        6 => "ADMIN_APP",
        7 => "TEXT_MESSAGE_COMPRESSED_APP",
        8 => "WAYPOINT_APP",
        9 => "AUDIO_APP",
        10 => "DETECTION_SENSOR_APP",
        11 => "ALERT_APP",
        12 => "KEY_VERIFICATION_APP",
        13 => "REMOTE_SHELL_APP",
        32 => "REPLY_APP",
        33 => "IP_TUNNEL_APP",
        34 => "PAXCOUNTER_APP",
        35 => "STORE_FORWARD_PLUSPLUS_APP",
        36 => "NODE_STATUS_APP",
        37 => "MESH_BEACON_APP",
        38 => "PAGING_APP",
        64 => "SERIAL_APP",
        65 => "STORE_FORWARD_APP",
        66 => "RANGE_TEST_APP",
        67 => "TELEMETRY_APP",
        68 => "ZPS_APP",
        69 => "SIMULATOR_APP",
        70 => "TRACEROUTE_APP",
        71 => "NEIGHBORINFO_APP",
        72 => "ATAK_PLUGIN",
        73 => "MAP_REPORT_APP",
        74 => "POWERSTRESS_APP",
        75 => "LORAWAN_BRIDGE",
        76 => "RETICULUM_TUNNEL_APP",
        77 => "CAYENNE_APP",
        78 => "ATAK_PLUGIN_V2",
        79 => "LORA_OTA_APP",
        112 => "GROUPALARM_APP",
        256 => "PRIVATE_APP",
        257 => "ATAK_FORWARDER",
        _ => return format!("PORT_{p}"),
    };
    s.to_string()
}

/// Every port number with a name (41 of them).
pub const PORTS: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 32, 33, 34, 35, 36, 37, 38, 64, 65, 66, 67, 68,
    69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 112, 256, 257,
];

/// The HardwareModel names of mesh.proto.
pub fn hw_name(n: u32) -> String {
    const T: &[&str] = &[
        "UNSET",
        "TLORA_V2",
        "TLORA_V1",
        "TLORA_V2_1_1P6",
        "TBEAM",
        "HELTEC_V2_0",
        "TBEAM_V0P7",
        "T_ECHO",
        "TLORA_V1_1P3",
        "RAK4631",
        "HELTEC_V2_1",
        "HELTEC_V1",
        "LILYGO_TBEAM_S3_CORE",
        "RAK11200",
        "NANO_G1",
        "TLORA_V2_1_1P8",
        "TLORA_T3_S3",
        "NANO_G1_EXPLORER",
        "NANO_G2_ULTRA",
        "LORA_TYPE",
        "WIPHONE",
        "WIO_WM1110",
        "RAK2560",
        "HELTEC_HRU_3601",
        "HELTEC_WIRELESS_BRIDGE",
        "STATION_G1",
        "RAK11310",
        "SENSELORA_RP2040",
        "SENSELORA_S3",
        "CANARYONE",
        "RP2040_LORA",
        "STATION_G2",
        "LORA_RELAY_V1",
        "NRF52840DK",
        "PPR",
        "GENIEBLOCKS",
        "NRF52_UNKNOWN",
        "PORTDUINO",
        "ANDROID_SIM",
        "DIY_V1",
        "NRF52840_PCA10059",
        "DR_DEV",
        "M5STACK",
        "HELTEC_V3",
        "HELTEC_WSL_V3",
        "BETAFPV_2400_TX",
        "BETAFPV_900_NANO_TX",
        "RPI_PICO",
        "HELTEC_WIRELESS_TRACKER",
        "HELTEC_WIRELESS_PAPER",
        "T_DECK",
        "T_WATCH_S3",
        "PICOMPUTER_S3",
        "HELTEC_HT62",
        "EBYTE_ESP32_S3",
        "ESP32_S3_PICO",
        "CHATTER_2",
        "HELTEC_WIRELESS_PAPER_V1_0",
        "HELTEC_WIRELESS_TRACKER_V1_0",
        "UNPHONE",
        "TD_LORAC",
        "CDEBYTE_EORA_S3",
        "TWC_MESH_V4",
        "NRF52_PROMICRO_DIY",
        "RADIOMASTER_900_BANDIT_NANO",
        "HELTEC_CAPSULE_SENSOR_V3",
        "HELTEC_VISION_MASTER_T190",
        "HELTEC_VISION_MASTER_E213",
        "HELTEC_VISION_MASTER_E290",
        "HELTEC_MESH_NODE_T114",
        "SENSECAP_INDICATOR",
        "TRACKER_T1000_E",
        "RAK3172",
        "WIO_E5",
        "RADIOMASTER_900_BANDIT",
        "ME25LS01_4Y10TD",
        "RP2040_FEATHER_RFM95",
        "M5STACK_COREBASIC",
        "M5STACK_CORE2",
        "RPI_PICO2",
        "M5STACK_CORES3",
        "SEEED_XIAO_S3",
        "MS24SF1",
        "TLORA_C6",
        "WISMESH_TAP",
        "ROUTASTIC",
        "MESH_TAB",
        "MESHLINK",
        "XIAO_NRF52_KIT",
        "THINKNODE_M1",
        "THINKNODE_M2",
        "T_ETH_ELITE",
        "HELTEC_SENSOR_HUB",
    ];
    if n == 255 {
        return "PRIVATE_HW".into();
    }
    T.get(n as usize)
        .map_or_else(|| format!("HW_{n}"), |s| s.to_string())
}

/// The role names of config.proto.
pub fn role_name(n: u32) -> String {
    const T: &[&str] = &[
        "CLIENT",
        "CLIENT_MUTE",
        "ROUTER",
        "ROUTER_CLIENT",
        "REPEATER",
        "TRACKER",
        "SENSOR",
        "TAK",
        "CLIENT_HIDDEN",
        "LOST_AND_FOUND",
        "TAK_TRACKER",
        "ROUTER_LATE",
        "CLIENT_BASE",
    ];
    T.get(n as usize)
        .map_or_else(|| format!("ROLE_{n}"), |s| s.to_string())
}

/// The role number of a name.
pub fn role_number(name: &str) -> Option<u32> {
    (0..13).find(|&i| role_name(i).eq_ignore_ascii_case(name))
}

/// The `Routing.Error` number `MAX_RETRANSMIT`.
pub const ROUTING_MAX_RETRANSMIT: u32 = 5;

/// The name of a `Routing.Error` number.
pub fn routing_error_name(n: u32) -> String {
    let s = match n {
        0 => "NONE",
        1 => "NO_ROUTE",
        2 => "GOT_NAK",
        3 => "TIMEOUT",
        4 => "NO_INTERFACE",
        5 => "MAX_RETRANSMIT",
        6 => "NO_CHANNEL",
        7 => "TOO_LARGE",
        8 => "NO_RESPONSE",
        9 => "DUTY_CYCLE_LIMIT",
        32 => "BAD_REQUEST",
        33 => "NOT_AUTHORIZED",
        34 => "PKI_FAILED",
        35 => "PKI_UNKNOWN_PUBKEY",
        36 => "ADMIN_BAD_SESSION_KEY",
        37 => "ADMIN_PUBLIC_KEY_UNAUTHORIZED",
        38 => "RATE_LIMIT_EXCEEDED",
        39 => "PKI_SEND_FAIL_PUBLIC_KEY",
        _ => return format!("ERROR_{n}"),
    };
    s.into()
}

/// The number of a Routing error name.
pub fn routing_error_number(name: &str) -> Option<u32> {
    (0..64).find(|&i| routing_error_name(i) == name)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Port payloads.

/// A User (NodeInfo).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// "!xxxxxxxx".
    pub id: String,
    /// Long name.
    pub long_name: String,
    /// Short name.
    pub short_name: String,
    /// Hardware model number.
    pub hw_model: u32,
    /// Licensed (ham).
    pub is_licensed: bool,
    /// Role.
    pub role: u32,
    /// X25519 public key (32 bytes, or empty).
    pub public_key: Vec<u8>,
    /// Is unmessagable.
    pub is_unmessagable: bool,
}

impl User {
    /// Parses.
    pub fn parse(b: &[u8]) -> Option<User> {
        let fs = fields(b)?;
        let mut u = User::default();
        for (n, w) in &fs {
            match n {
                1 => u.id = text(w),
                2 => u.long_name = text(w),
                3 => u.short_name = text(w),
                5 => u.hw_model = w.u64() as u32,
                6 => u.is_licensed = w.bool(),
                7 => u.role = w.u64() as u32,
                8 => u.public_key = w.bytes().to_vec(),
                9 => u.is_unmessagable = w.bool(),
                _ => {}
            }
        }
        Some(u)
    }
    /// The bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut w_buf = Vec::new();
        let mut w = Writer::new(&mut w_buf);
        w.string(1, &self.id)
            .string(2, &self.long_name)
            .string(3, &self.short_name);
        if self.hw_model != 0 {
            w.uint(5, self.hw_model as u64);
        }
        if self.is_licensed {
            w.uint(6, 1);
        }
        if self.role != 0 {
            w.uint(7, self.role as u64);
        }
        if self.public_key.len() == 32 {
            w.bytes(8, &self.public_key);
        }
        w_buf
    }
}

/// A Position.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// Latitude, degrees.
    pub lat: Option<f64>,
    /// Longitude, degrees.
    pub lon: Option<f64>,
    /// Altitude, metres.
    pub alt: Option<i32>,
    /// Time (Unix).
    pub time: u32,
    /// Satellites in view.
    pub sats: u32,
    /// Precision bits.
    pub precision_bits: u32,
}

impl Position {
    /// Parses.
    pub fn parse(b: &[u8]) -> Option<Position> {
        let fs = fields(b)?;
        let mut p = Position::default();
        let mut hae = None;
        let mut ts = 0;
        for (n, w) in &fs {
            match n {
                1 => p.lat = Some(w.sfixed32() as f64 * 1e-7),
                2 => p.lon = Some(w.sfixed32() as f64 * 1e-7),
                3 => p.alt = Some(w.i32()),
                4 => p.time = w.u64() as u32,
                7 => ts = w.u64() as u32,
                9 => hae = Some(w.sint() as i32),
                19 => p.sats = w.u64() as u32,
                23 => p.precision_bits = w.u64() as u32,
                _ => {}
            }
        }
        if p.alt.is_none() {
            p.alt = hae;
        }
        if p.time == 0 {
            p.time = ts;
        }
        Some(p)
    }
    /// A fix: both coordinates present and not both 0.
    pub fn has_fix(&self) -> bool {
        matches!((self.lat, self.lon), (Some(a), Some(b)) if !(a == 0.0 && b == 0.0))
    }
    /// Encodes with the coordinates reduced to `precision` bits (0 or 32: exact), centred in their cell.
    pub fn encode(
        lat: f64,
        lon: f64,
        alt: i32,
        time: u32,
        precision: u32,
        source_manual: bool,
    ) -> Vec<u8> {
        let red = |v: f64| -> i32 {
            let i = (v * 1e7).round() as i32;
            if precision == 0 || precision >= 32 {
                return i;
            }
            let mask: u32 = u32::MAX << (32 - precision);
            let t = (i as u32) & mask;
            (t.wrapping_add(1u32 << (31 - precision))) as i32
        };
        let mut w_buf = Vec::new();
        let mut w = Writer::new(&mut w_buf);
        w.fixed32(1, red(lat) as u32)
            .fixed32(2, red(lon) as u32)
            .int(3, alt as i64)
            .fixed32(4, time);
        w.uint(5, if source_manual { 1 } else { 2 });
        if precision > 0 && precision < 32 {
            w.uint(23, precision as u64);
        } else {
            w.uint(23, 32);
        }
        w_buf
    }
}

/// Device and environment telemetry values (the ones shown).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    /// Time.
    pub time: u32,
    /// Battery percent (above 100: powered).
    pub battery: Option<u32>,
    /// Volts.
    pub voltage: Option<f32>,
    /// Channel use, percent.
    pub ch_util: Option<f32>,
    /// Air time, percent.
    pub air_tx: Option<f32>,
    /// Uptime, s.
    pub uptime: Option<u32>,
    /// Temperature, C.
    pub temperature: Option<f32>,
    /// Humidity, percent.
    pub humidity: Option<f32>,
    /// Pressure, hPa.
    pub pressure: Option<f32>,
    /// The other named values.
    pub other: Vec<(String, f32)>,
    /// The detail line.
    pub summary: String,
    /// Device metrics were present.
    pub device: bool,
    /// Environment metrics were present.
    pub environment: bool,
}

fn telemetry(b: &[u8]) -> Option<Telemetry> {
    let fs = fields(b)?;
    let mut t = Telemetry::default();
    let mut summary = String::new();
    for (n, w) in &fs {
        match n {
            1 => t.time = w.u64() as u32,
            2 => {
                t.device = true;
                let f = fields(w.bytes())?;
                for (k, v) in &f {
                    match k {
                        1 => t.battery = Some(v.u64() as u32),
                        2 => t.voltage = Some(v.f32()),
                        3 => t.ch_util = Some(v.f32()),
                        4 => t.air_tx = Some(v.f32()),
                        5 => t.uptime = Some(v.u64() as u32),
                        _ => {}
                    }
                }
                let v = t.voltage.unwrap_or(0.0);
                summary = match t.battery {
                    Some(b) if b > 100 => format!("powered, {v:.2} V"),
                    Some(b) => format!("battery {b} %, {v:.2} V"),
                    None => format!("{v:.2} V"),
                };
                if t.ch_util.is_some() || t.air_tx.is_some() {
                    summary += &format!(
                        ", ch util {:.1} %, air tx {:.1} %",
                        t.ch_util.unwrap_or(0.0),
                        t.air_tx.unwrap_or(0.0)
                    );
                }
            }
            3 => {
                t.environment = true;
                const NAMES: &[(u32, &str)] = &[
                    (4, "gas resistance"),
                    (5, "voltage"),
                    (6, "current"),
                    (7, "IAQ"),
                    (8, "distance"),
                    (9, "lux"),
                    (10, "white lux"),
                    (11, "IR lux"),
                    (12, "UV lux"),
                    (13, "wind direction"),
                    (14, "wind speed"),
                    (15, "weight"),
                    (16, "wind gust"),
                    (17, "wind lull"),
                    (18, "radiation"),
                    (19, "rain 1 h"),
                    (20, "rain 24 h"),
                    (21, "soil moisture"),
                    (22, "soil temperature"),
                ];
                let f = fields(w.bytes())?;
                for (k, v) in &f {
                    match k {
                        1 => t.temperature = Some(v.f32()),
                        2 => t.humidity = Some(v.f32()),
                        3 => t.pressure = Some(v.f32()),
                        k => {
                            if let Some((_, name)) = NAMES.iter().find(|x| x.0 == *k) {
                                let val = match v {
                                    Wire::Fixed32(_) => v.f32(),
                                    _ => v.u64() as f32,
                                };
                                t.other.push((name.to_string(), val));
                            }
                        }
                    }
                }
                let mut parts = Vec::new();
                if let Some(x) = t.temperature {
                    parts.push(format!("{x:.1} C"));
                }
                if let Some(x) = t.humidity {
                    parts.push(format!("{x:.0} %"));
                }
                if let Some(x) = t.pressure {
                    parts.push(format!("{x:.1} hPa"));
                }
                for (n, v) in &t.other {
                    parts.push(format!("{n} {v}"));
                }
                summary = format!("environment {}", parts.join(", "));
            }
            4 => {
                let f = fields(w.bytes())?;
                let g = |k| get(&f, k).map(|v| v.u64()).unwrap_or(0);
                summary = format!("air quality PM1.0 {}, PM2.5 {}, PM10 {}", g(1), g(2), g(3));
            }
            5 => {
                let f = fields(w.bytes())?;
                let g = |k| get(&f, k).map(|v| v.f32()).unwrap_or(0.0);
                summary = format!(
                    "power ch1 {:.2} V {:.1} mA, ch2 {:.2} V {:.1} mA",
                    g(1),
                    g(2),
                    g(3),
                    g(4)
                );
            }
            6 => {
                let f = fields(w.bytes())?;
                let g = |k| get(&f, k).map(|v| v.u64()).unwrap_or(0);
                t.uptime = Some(g(1) as u32);
                let gf = |k| get(&f, k).map(|v| v.f32()).unwrap_or(0.0);
                t.ch_util = Some(gf(2));
                t.air_tx = Some(gf(3));
                summary = format!(
                    "local stats, uptime {} s, sent {}, received {}, bad {}, online {} of {}",
                    g(1),
                    g(4),
                    g(5),
                    g(6),
                    g(7),
                    g(8)
                );
            }
            7 => {
                let f = fields(w.bytes())?;
                let g = |k| get(&f, k).map(|v| v.u64()).unwrap_or(0);
                let gf = |k| get(&f, k).map(|v| v.f32()).unwrap_or(0.0);
                summary = format!("health heart {} bpm, SpO2 {} %, {:.1} C", g(1), g(2), gf(3));
            }
            8 => {
                let f = fields(w.bytes())?;
                let g = |k| get(&f, k).map(|v| v.u64()).unwrap_or(0);
                summary = format!("host uptime {} s, free memory {} bytes", g(1), g(2));
            }
            _ => {}
        }
    }
    t.summary = summary;
    Some(t)
}

/// A traceroute / route discovery.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// Nodes towards.
    pub route: Vec<u32>,
    /// SNR towards, quarter dB.
    pub snr_towards: Vec<i32>,
    /// Nodes back.
    pub route_back: Vec<u32>,
    /// SNR back.
    pub snr_back: Vec<i32>,
}

impl Route {
    /// Parses.
    pub fn parse(b: &[u8]) -> Option<Route> {
        let fs = fields(b)?;
        Some(Route {
            route: repeated(&fs, 1, true)
                .into_iter()
                .map(|v| v as u32)
                .collect(),
            snr_towards: repeated(&fs, 2, false)
                .into_iter()
                .map(|v| v as i64 as i32)
                .collect(),
            route_back: repeated(&fs, 3, true)
                .into_iter()
                .map(|v| v as u32)
                .collect(),
            snr_back: repeated(&fs, 4, false)
                .into_iter()
                .map(|v| v as i64 as i32)
                .collect(),
        })
    }
    /// The bytes (packed).
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf);
        let pack32 = |v: &[u32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
        let varints = |v: &[i32]| v.iter().map(|&s| s as i64 as u64).collect::<Vec<u64>>();
        if !self.route.is_empty() {
            w.bytes(1, &pack32(&self.route));
        }
        if !self.snr_towards.is_empty() {
            w.packed_varints(2, &varints(&self.snr_towards));
        }
        if !self.route_back.is_empty() {
            w.bytes(3, &pack32(&self.route_back));
        }
        if !self.snr_back.is_empty() {
            w.packed_varints(4, &varints(&self.snr_back));
        }
        buf
    }
}

/// A Routing message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Routing {
    /// The error (0 = ack).
    pub error: u32,
    /// A route request or reply.
    pub discovery: bool,
}

impl Routing {
    /// Parses.
    pub fn parse(b: &[u8]) -> Option<Routing> {
        let fs = fields(b)?;
        let mut r = Routing::default();
        for (n, w) in &fs {
            match n {
                1 | 2 => r.discovery = true,
                3 => r.error = w.u64() as u32,
                _ => {}
            }
        }
        Some(r)
    }
    /// The bytes of an ack or error.
    pub fn encode_error(error: u32) -> Vec<u8> {
        let mut w_buf = Vec::new();
        let mut w = Writer::new(&mut w_buf);
        w.uint(3, error as u64);
        w_buf
    }
}

/// The data decoded from a port payload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortInfo {
    /// The detail line.
    pub summary: String,
    /// A text message.
    pub text: Option<String>,
    /// NodeInfo.
    pub user: Option<User>,
    /// A position.
    pub position: Option<Position>,
    /// Telemetry.
    pub telemetry: Option<Telemetry>,
    /// Routing.
    pub routing: Option<Routing>,
    /// A route discovery.
    pub route: Option<Route>,
    /// A waypoint (name, lat, lon).
    pub waypoint: Option<(String, f64, f64)>,
    /// An alert.
    pub alert: bool,
}

/// Text cut after `max` bytes on a character boundary, with "..." when cut.
pub fn cut_text(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut e = max;
    while !s.is_char_boundary(e) {
        e -= 1;
    }
    format!("{}...", &s[..e])
}

/// Text cut to at most `max` bytes on a character boundary (no marker).
pub fn cut_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut e = max;
    while !s.is_char_boundary(e) {
        e -= 1;
    }
    &s[..e]
}

fn printable(b: &[u8]) -> String {
    b.iter()
        .map(|&c| {
            if (0x20..0x7F).contains(&c) {
                c as char
            } else {
                '.'
            }
        })
        .collect()
}

/// Decodes a port payload (the table of 5.7).
pub fn decode_port(d: &Data) -> PortInfo {
    let p = &d.payload;
    let mut info = PortInfo::default();
    let n = p.len();
    info.summary = match d.port {
        1 => {
            let t = String::from_utf8_lossy(p).into_owned();
            let mut s = cut_text(&t, 80);
            if d.emoji != 0 {
                s = format!("reaction {t}");
            } else if d.reply_id != 0 {
                s += " (reply)";
            }
            info.text = Some(t);
            s
        }
        2 => {
            let f = fields(p).unwrap_or_default();
            let g = |k| get(&f, k).map(|v| v.u64()).unwrap_or(0);
            format!(
                "hardware type {}, GPIO mask 0x{:x}, value 0x{:x}",
                g(1),
                g(2),
                g(3)
            )
        }
        3 => match Position::parse(p) {
            Some(pos) if pos.has_fix() => {
                let mut s = format!(
                    "{:.5}, {:.5}, {} m",
                    pos.lat.unwrap_or(0.0),
                    pos.lon.unwrap_or(0.0),
                    pos.alt.unwrap_or(0)
                );
                if pos.precision_bits > 0 && pos.precision_bits < 32 {
                    s += &format!(", {} bit precision", pos.precision_bits);
                }
                info.position = Some(pos);
                s
            }
            Some(_) => "no position fix".into(),
            None => format!("{n} bytes (bad position)"),
        },
        4 => match User::parse(p) {
            Some(u) => {
                let mut s = format!(
                    "{} ({}), {}, {}",
                    u.long_name,
                    u.short_name,
                    hw_name(u.hw_model),
                    role_name(u.role)
                );
                if u.public_key.len() == 32 {
                    s += ", has public key";
                }
                if u.is_licensed {
                    s += ", licensed";
                }
                info.user = Some(u);
                s
            }
            None => format!("{n} bytes (bad user)"),
        },
        5 => match Routing::parse(p) {
            Some(r) => {
                let s = if r.discovery {
                    "route discovery".to_string()
                } else if r.error == 0 {
                    format!("ack of {:08x}", d.request_id)
                } else {
                    format!(
                        "error {} for {:08x}",
                        routing_error_name(r.error),
                        d.request_id
                    )
                };
                info.routing = Some(r);
                s
            }
            None => format!("{n} bytes (bad routing)"),
        },
        6 => format!("admin message, {n} bytes"),
        7 => format!("compressed text, {n} bytes"),
        8 => {
            let f = fields(p).unwrap_or_default();
            let name = get(&f, 6).map(|w| text(&w)).unwrap_or_default();
            let desc = get(&f, 7).map(|w| text(&w)).unwrap_or_default();
            let lat = get(&f, 2)
                .map(|w| w.sfixed32() as f64 * 1e-7)
                .unwrap_or(0.0);
            let lon = get(&f, 3)
                .map(|w| w.sfixed32() as f64 * 1e-7)
                .unwrap_or(0.0);
            let exp = get(&f, 4).map(|w| w.u64()).unwrap_or(0);
            let icon = get(&f, 8).map(|w| w.u64()).unwrap_or(0);
            info.waypoint = Some((name.clone(), lat, lon));
            format!("waypoint {name}: {desc}, {lat:.5}, {lon:.5}, expires {exp}, icon {icon}")
        }
        9 => format!("{n} bytes of audio"),
        10 | 32 | 66 => {
            let t = String::from_utf8_lossy(p).into_owned();
            t
        }
        11 => {
            info.alert = true;
            format!("alert: {}", String::from_utf8_lossy(p))
        }
        12 => {
            let f = fields(p).unwrap_or_default();
            format!(
                "key verification, nonce {}",
                get(&f, 1).map(|w| w.u64()).unwrap_or(0)
            )
        }
        33 => {
            if n >= 20 && p[0] >> 4 == 4 {
                format!(
                    "IP packet, {n} bytes, {}.{}.{}.{} > {}.{}.{}.{}",
                    p[12], p[13], p[14], p[15], p[16], p[17], p[18], p[19]
                )
            } else {
                format!("IP packet, {n} bytes")
            }
        }
        34 => {
            let f = fields(p).unwrap_or_default();
            let g = |k| get(&f, k).map(|v| v.u64()).unwrap_or(0);
            format!("Wi-Fi {}, BLE {}, uptime {} s", g(1), g(2), g(3))
        }
        64 => printable(p),
        65 => store_forward(p, &mut info),
        67 => match telemetry(p) {
            Some(t) => {
                let s = t.summary.clone();
                info.telemetry = Some(t);
                s
            }
            None => format!("{n} bytes (bad telemetry)"),
        },
        70 => match Route::parse(p) {
            Some(r) => {
                let request = d.want_response || d.request_id == 0;
                let names: Vec<String> = r.route.iter().map(|&x| node_name(x)).collect();
                let snrs = |v: &[i32]| {
                    v.iter()
                        .map(|&s| format!("{:.1}", s as f64 / 4.0))
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                let mut s = if request {
                    let mut parts = vec![node_name(d.source)];
                    parts.extend(names);
                    format!("traceroute request: {}", parts.join(" > "))
                } else {
                    let mut parts = vec![];
                    parts.extend(names);
                    format!("traceroute reply: {}", parts.join(" > "))
                };
                if !r.snr_towards.is_empty() {
                    s += &format!(" (SNR {} dB)", snrs(&r.snr_towards));
                }
                if !r.route_back.is_empty() {
                    s += &format!(", back {} hops", r.route_back.len());
                }
                info.route = Some(r);
                s
            }
            None => format!("{n} bytes (bad route)"),
        },
        71 => {
            let f = fields(p).unwrap_or_default();
            let ns: Vec<String> = f
                .iter()
                .filter(|x| x.0 == 4)
                .map(|x| {
                    let g = fields(x.1.bytes()).unwrap_or_default();
                    let id = get(&g, 1).map(|w| w.u64() as u32).unwrap_or(0);
                    let snr = get(&g, 2).map(|w| w.f32()).unwrap_or(0.0);
                    format!("{} {:.1} dB", node_name(id), snr)
                })
                .collect();
            let shown: Vec<String> = ns.iter().take(4).cloned().collect();
            format!("{} neighbors: {}", ns.len(), shown.join(", "))
        }
        72 => {
            let f = fields(p).unwrap_or_default();
            let mut s = "TAK".to_string();
            if let Some(c) = get(&f, 2) {
                let g = fields(c.bytes()).unwrap_or_default();
                if let Some(cs) = get(&g, 2) {
                    s += &format!(" {}", text(&cs));
                }
            }
            if let Some(pl) = get(&f, 5) {
                let g = fields(pl.bytes()).unwrap_or_default();
                let lat = get(&g, 1).map(|w| w.sfixed32() as f64 * 1e-7);
                let lon = get(&g, 2).map(|w| w.sfixed32() as f64 * 1e-7);
                if let (Some(a), Some(b)) = (lat, lon) {
                    s += &format!(", {a:.5}, {b:.5}");
                }
            }
            s
        }
        73 => {
            let f = fields(p).unwrap_or_default();
            let t = |k| get(&f, k).map(|w| text(&w)).unwrap_or_default();
            let g = |k| get(&f, k).map(|w| w.u64()).unwrap_or(0);
            let lat = get(&f, 9).map(|w| w.sfixed32() as f64 * 1e-7);
            let lon = get(&f, 10).map(|w| w.sfixed32() as f64 * 1e-7);
            let mut u = User {
                long_name: t(1),
                short_name: t(2),
                role: g(3) as u32,
                hw_model: g(4) as u32,
                ..Default::default()
            };
            u.id = String::new();
            let mut s = format!(
                "map report {} ({}), {}, {}, firmware {}, region {}, preset {}, {} nodes online",
                u.long_name,
                u.short_name,
                role_name(u.role),
                hw_name(u.hw_model),
                t(5),
                g(6),
                g(7),
                g(13)
            );
            if let (Some(a), Some(b)) = (lat, lon) {
                s += &format!(", {a:.5}, {b:.5}");
                info.position = Some(Position {
                    lat: Some(a),
                    lon: Some(b),
                    alt: get(&f, 11).map(|w| w.i32()),
                    ..Default::default()
                });
            }
            info.user = Some(u);
            s
        }
        _ => format!("{n} bytes"),
    };
    info
}

fn store_forward(p: &[u8], info: &mut PortInfo) -> String {
    let f = fields(p).unwrap_or_default();
    let rr = get(&f, 1).map(|w| w.u64()).unwrap_or(0);
    let name = match rr {
        0 => "UNSET",
        1 => "ROUTER_ERROR",
        2 => "ROUTER_HEARTBEAT",
        3 => "ROUTER_PING",
        4 => "ROUTER_PONG",
        5 => "ROUTER_BUSY",
        6 => "ROUTER_HISTORY",
        7 => "ROUTER_STATS",
        8 => "ROUTER_TEXT_DIRECT",
        9 => "ROUTER_TEXT_BROADCAST",
        64 => "CLIENT_ERROR",
        65 => "CLIENT_HISTORY",
        66 => "CLIENT_STATS",
        67 => "CLIENT_PING",
        68 => "CLIENT_PONG",
        106 => "CLIENT_ABORT",
        _ => "",
    };
    let mut s = if name.is_empty() {
        format!("store and forward {rr}")
    } else {
        format!("store and forward {name}")
    };
    if let Some(st) = get(&f, 2) {
        let g = fields(st.bytes()).unwrap_or_default();
        let v = |k| get(&g, k).map(|w| w.u64()).unwrap_or(0);
        s += &format!(", {} messages, {} saved, max {}", v(1), v(2), v(3));
    }
    if let Some(h) = get(&f, 3) {
        let g = fields(h.bytes()).unwrap_or_default();
        let v = |k| get(&g, k).map(|w| w.u64()).unwrap_or(0);
        s += &format!(", history {} messages over {} min", v(1), v(2));
    }
    if let Some(h) = get(&f, 4) {
        let g = fields(h.bytes()).unwrap_or_default();
        let v = |k| get(&g, k).map(|w| w.u64()).unwrap_or(0);
        s += &format!(", heartbeat every {} s", v(1));
    }
    if let Some(t) = get(&f, 5) {
        let txt = text(&t);
        s += &format!(": {}", cut_text(&txt, 80));
        info.text = Some(txt);
    }
    s
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Building packets.

/// Encrypts `data` for a channel packet and puts the header in front.
pub fn build_channel_packet(h: &Header, key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut body = data.to_vec();
    aes_ctr(key, h.id, h.from, &mut body);
    let mut out = h.bytes().to_vec();
    out.extend_from_slice(&body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn djb2_and_names() {
        assert_eq!(djb2("LongFast"), 0x07C6_3403);
        assert_eq!(preset("long mod").unwrap().name, "LongModerate");
    }

    #[test]
    fn channel_hash_of_longfast() {
        assert_eq!(channel_hash("LongFast", &DEFAULT_KEY), 0x08);
    }

    #[test]
    fn default_key_packet() {
        let h = Header {
            to: 0x7A6D648C,
            from: 0x929,
            id: 0x13B2D662,
            hop_limit: 3,
            hop_start: 3,
            want_ack: true,
            channel: 0x08,
            next_hop: 0,
            relay: 0x29,
            via_mqtt: false,
        };
        let data = [0x08, 0x01, 0x12, 0x04, 0x74, 0x65, 0x73, 0x74, 0x48, 0x00];
        let pkt = build_channel_packet(&h, &DEFAULT_KEY, &data);
        assert_eq!(
            hex(&pkt),
            "8c646d7a2909000062d6b2136b08002946f2ecd0b02ac4cf2be5"
        );
    }

    #[test]
    fn aes256() {
        let key: Vec<u8> = (0..32).collect();
        let h = Header {
            to: BROADCAST,
            from: 0x77,
            id: 90,
            hop_limit: 3,
            hop_start: 3,
            channel: channel_hash("Friends", &key),
            relay: 0x77,
            ..Default::default()
        };
        let d = Data {
            port: 1,
            payload: b"secret".to_vec(),
            bitfield: Some(0),
            ..Default::default()
        };
        let pkt = build_channel_packet(&h, &key, &d.encode());
        assert_eq!(
            hex(&pkt),
            "ffffffff770000005a000000634100779a4823bfc6429c7c74ac25a2"
        );
    }

    #[test]
    fn keys_expand() {
        assert_eq!(parse_key_text("AQ==").unwrap(), DEFAULT_KEY.to_vec());
        let k2 = parse_key_text("Ag==").unwrap();
        assert_eq!(k2[15], DEFAULT_KEY[15] + 1);
        assert!(parse_key_text("AA==").unwrap().is_empty());
        assert!(parse_key_text("not base64 !!").is_none());
        assert!(expand_key(&[1u8; 40]).is_none());
        assert_eq!(
            base64_decode(&base64_encode(&[1, 2, 3, 4, 5])).unwrap(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn pki_round_trip() {
        let a = new_private_key();
        let b = new_private_key();
        let ka = pki_shared_key(&a, &public_key(&b)).unwrap();
        let kb = pki_shared_key(&b, &public_key(&a)).unwrap();
        assert_eq!(ka, kb);
        let body = pki_encrypt(&ka, 77, 0x1234, 0xdeadbeef, b"\x08\x01\x12\x02hi").unwrap();
        assert_eq!(
            pki_decrypt(&kb, 77, 0x1234, &body).unwrap(),
            b"\x08\x01\x12\x02hi"
        );
        let mut bad = body.clone();
        bad[2] ^= 1;
        assert!(pki_decrypt(&kb, 77, 0x1234, &bad).is_none());
    }
}
