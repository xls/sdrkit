//! Protocol-level vectors: each composes the primitives the way a protocol does (its counter block, nonce, MAC input and key
//! derivation) and checks a published packet. The layouts written out here belong to the protocol crates; they are repeated in
//! these tests only to show that the primitives give what each protocol needs.

mod common;

use common::{a, h};
use sdrkit_algo_crypto::mac::hmac_sha256;
use sdrkit_algo_crypto::{sha256, Aes, AesKey, Ccm, Cmac, Counter, HmacSha256, Key, X25519Secret};

/// The Meshtastic default channel key ("AQ==", key index 1).
const MESHTASTIC_DEFAULT_KEY: [u8; 16] = [
    0xD4, 0xF1, 0xBB, 0x3A, 0x20, 0x29, 0x07, 0x59, 0xF0, 0xBC, 0xFF, 0xAB, 0xCF, 0x4E, 0x69, 0x01,
];

/// Meshtastic channel counter block: packet id as 64-bit LE, sender as 32-bit LE, four zero bytes.
fn meshtastic_counter(id: u32, from: u32) -> [u8; 16] {
    let mut n = [0u8; 16];
    n[0..4].copy_from_slice(&id.to_le_bytes());
    n[8..12].copy_from_slice(&from.to_le_bytes());
    n
}

#[test]
fn meshtastic_default_key_packet() {
    // A Meshtastic packet: 8c646d7a 29090000 62d6b213 6b 08 00 29 | 46f2ecd0b02ac4cf2be5, from 0x929, id 0x13B2D662,
    // on "LongFast" with the default key; the Data message is port 1, "test".
    let aes = Aes::new(&AesKey::Aes128(Key::new(MESHTASTIC_DEFAULT_KEY)));
    let mut body = h("46f2ecd0b02ac4cf2be5");
    aes.apply_ctr(
        &meshtastic_counter(0x13B2_D662, 0x929),
        Counter::Low32,
        &mut body,
    )
    .unwrap();
    assert_eq!(body, h("08011204746573744800"));
}

#[test]
fn meshtastic_aes256_channel_packet() {
    // A Meshtastic packet with key 00..1F, from 0x77, id 90, Data port 1 "secret" with bitfield 0.
    let key: [u8; 32] = core::array::from_fn(|i| i as u8);
    let aes = Aes::new(&AesKey::Aes256(Key::new(key)));
    let mut body = h("9a4823bfc6429c7c74ac25a2");
    aes.apply_ctr(&meshtastic_counter(90, 0x77), Counter::Low32, &mut body)
        .unwrap();
    assert_eq!(
        body,
        [&h("08011206")[..], b"secret", &h("4800")[..]].concat()
    );
}

#[test]
fn meshtastic_pki_direct_message() {
    // The Meshtastic firmware's public-key crypto test (test/test_crypto): the receiver's private key, the sender's public key,
    // and the packet 8c646d7a 29090000 62d6b213 6b000000 | 40df24abfcc30a17a3d9 | 046726099e796a1c | 036a792b
    // (ciphertext, 8-byte tag, extra nonce). Key = SHA-256 of the X25519 secret; AES-256-CCM, M = 8, 13-byte nonce
    // id (LE) | extra nonce | sender (LE) | 0.
    let own = X25519Secret::new(Key::new(a(
        "a00330633e63522f8a4d81ec6d9d1e6617f6c8ffd3a4c698229537d44e522277",
    )));
    let peer = a("db18fc50eea47f00251cb784819a3cf5fc361882597f589f0d7ff820e8064457");
    let shared = own.diffie_hellman(&peer).unwrap();
    let key = AesKey::Aes256(Key::new(sha256(shared.as_bytes())));
    let ccm = Ccm::<8, 13>::new(&key);

    let (id, from) = (0x13B2_D662u32, 0x0929u32);
    let extra = h("036a792b");
    let mut nonce = [0u8; 13];
    nonce[0..4].copy_from_slice(&id.to_le_bytes());
    nonce[4..8].copy_from_slice(&extra);
    nonce[8..12].copy_from_slice(&from.to_le_bytes());
    assert_eq!(nonce, a::<13>("62d6b213036a792b2909000000"));

    let ct = h("40df24abfcc30a17a3d9");
    let tag = a::<8>("046726099e796a1c");
    let mut out = [0u8; 10];
    ccm.decrypt_to(&nonce, &[], &ct, &tag, &mut out).unwrap();
    assert_eq!(out.to_vec(), h("08011204746573744800"));

    // Encrypting the plaintext again produces the same packet (the transmit path).
    let mut p = out;
    assert_eq!(ccm.encrypt_in_place(&nonce, &[], &mut p).unwrap(), tag);
    assert_eq!(p.to_vec(), ct);
}

#[test]
fn lorawan_1_0_data_frame_with_session_keys() {
    // LoRaWAN 1.0.x unconfirmed data up (the example of the MIT-licensed `lora-packet` library): PHYPayload
    // 40 F17DBE49 00 0200 01 95437876 2B11FF0D, NwkSKey 44024241ed4ce9a68c6a8bc055233fd3, AppSKey ec925802ae430ca77fd3dd73cb2cc588,
    // payload "test".
    let frame = h("40f17dbe4900020001954378762b11ff0d");
    let (msg, mic) = frame.split_at(frame.len() - 4);
    let dev_addr = &msg[1..5]; // as sent (little-endian)
    let fcnt: u32 = 2;

    // MIC (LoRaWAN 1.0.4 section 4.4): CMAC over B0 | msg, B0 = 49 | 00000000 | dir | DevAddr | FCnt (32-bit LE) | 00 | len(msg).
    let mut b0 = [0u8; 16];
    b0[0] = 0x49;
    b0[5] = 0; // uplink
    b0[6..10].copy_from_slice(dev_addr);
    b0[10..14].copy_from_slice(&fcnt.to_le_bytes());
    b0[15] = msg.len() as u8;
    let mut cmac = Cmac::new(&AesKey::from_slice(&h("44024241ed4ce9a68c6a8bc055233fd3")).unwrap());
    cmac.update(&b0);
    cmac.update(msg);
    cmac.verify_truncated_reset(mic).unwrap();

    // FRMPayload (section 4.3.3): XOR with AES(AppSKey, A_i), A_i = 01 | 00000000 | dir | DevAddr | FCnt | 00 | i, i from 1.
    let mut a1 = b0;
    a1[0] = 0x01;
    a1[15] = 1;
    let mut payload = msg[9..].to_vec();
    Aes::new(&AesKey::from_slice(&h("ec925802ae430ca77fd3dd73cb2cc588")).unwrap())
        .apply_ctr(&a1, Counter::Low32, &mut payload)
        .unwrap();
    assert_eq!(payload, b"test");
}

/// The MeshCore Public channel secret.
const MESHCORE_PUBLIC: [u8; 16] = [
    0x8B, 0x33, 0x87, 0xE9, 0xC5, 0xCD, 0xEA, 0x6A, 0xC9, 0xE5, 0xED, 0xBA, 0xA1, 0x15, 0xCD, 0x72,
];

#[test]
fn meshcore_channel_hashes_and_hashtag_secret() {
    // MeshCore public channel vectors.
    assert_eq!(sha256(&MESHCORE_PUBLIC)[0], 0x11);
    assert_eq!(
        sha256(b"#test")[..16],
        h("9cd8fcf22a47333b591d96a2b848b73f")
    );
}

#[test]
fn meshcore_group_text_on_public() {
    // A MeshCore packet: 15 01 ab 11 | MAC f920 | 32 bytes of ciphertext. MAC = first 2 bytes of HMAC-SHA256 keyed with the
    // secret zero-padded to 32 bytes, over the ciphertext; then AES-128-ECB with the secret.
    let pkt = h("1501ab11f920a61f33a7906d3a9dcd1751d8c3318196e47c7d5dbda404b869f4b2ed4e459842");
    let (mac, ct) = (&pkt[4..6], &pkt[6..]);
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(&MESHCORE_PUBLIC);
    let mut hmac = HmacSha256::new(&key);
    hmac.update(ct);
    hmac.verify_truncated_reset(mac).unwrap();
    assert_eq!(&hmac_sha256(&key, ct)[..2], mac);

    let mut pt = ct.to_vec();
    Aes::new(&AesKey::Aes128(Key::new(MESHCORE_PUBLIC)))
        .decrypt_ecb(&mut pt)
        .unwrap();
    assert_eq!(
        u32::from_le_bytes(pt[0..4].try_into().unwrap()),
        1_760_000_000
    );
    assert_eq!(pt[4], 0); // plain text, attempt 0
    let text = b"Dubai Repeater: hello mesh";
    assert_eq!(&pt[5..5 + text.len()], text);
    assert!(pt[5 + text.len()..].iter().all(|&b| b == 0));
}

#[test]
fn flipper_chat_key_from_password() {
    // The Flipper Sub-GHz chat key is SHA-256 of the password's bytes, used as an AES-256-GCM key.
    let k = sha256(b"testkey");
    assert!(matches!(AesKey::from_slice(&k).unwrap(), AesKey::Aes256(_)));
}
