//! Bad input returns an error value, never a panic; failed tag checks release no plaintext.

mod common;

use common::{a, h};
use sdrkit_algo_crypto::curve::{ed25519_public_to_x25519, ed25519_verify};
use sdrkit_algo_crypto::{
    Aes, AesKey, Ccm, Cmac, Counter, Des, Ed25519Key, Error, Gcm, HmacSha256, Key, X25519Secret,
};

#[test]
fn key_lengths_are_checked() {
    for n in [0, 1, 15, 17, 23, 25, 31, 33, 64] {
        assert_eq!(
            AesKey::from_slice(&vec![0; n]).unwrap_err(),
            Error::KeyLength { got: n }
        );
    }
    assert_eq!(
        Key::<32>::from_slice(&[0; 16]).unwrap_err(),
        Error::KeyLength { got: 16 }
    );
    assert_eq!(AesKey::from_slice(&[0; 24]).unwrap().len(), 24);
}

#[test]
fn keys_are_not_printed() {
    let k = AesKey::from_slice(&[0xAB; 16]).unwrap();
    let s = format!(
        "{k:?} {:?} {:?}",
        Aes::new(&k),
        Ed25519Key::from_seed(&Key::new([0xAB; 32]))
    );
    assert!(
        !s.contains("171") && !s.to_lowercase().contains("ab, "),
        "{s}"
    );
}

#[test]
fn ecb_needs_whole_blocks() {
    let aes = Aes::new(&AesKey::from_slice(&[1; 16]).unwrap());
    let mut d = [0u8; 17];
    assert_eq!(
        aes.encrypt_ecb(&mut d).unwrap_err(),
        Error::NotWholeBlocks { block: 16, got: 17 }
    );
    assert_eq!(
        aes.decrypt_ecb(&mut d[..5]).unwrap_err(),
        Error::NotWholeBlocks { block: 16, got: 5 }
    );
    assert!(aes.encrypt_ecb(&mut []).is_ok());
    let des = Des::new(&[1; 8]);
    assert_eq!(
        des.decrypt_ecb(&mut d[..13]).unwrap_err(),
        Error::NotWholeBlocks { block: 8, got: 13 }
    );
    assert!(d.iter().all(|&b| b == 0), "a refused call changes nothing");
}

#[test]
fn ctr_counter_widths_differ_only_at_the_wrap() {
    let aes = Aes::new(&AesKey::from_slice(&[9; 16]).unwrap());
    let iv = a::<16>("000102030405060708090a0bffffffff");
    let ks = |w| {
        let mut d = [0u8; 32];
        aes.apply_ctr(&iv, w, &mut d).unwrap();
        d
    };
    let block = |c: &str| {
        let mut b = a::<16>(c);
        aes.encrypt_block(&mut b);
        b
    };
    // Low32 wraps the last four bytes; Low64 and Full128 carry into byte 11.
    assert_eq!(
        ks(Counter::Low32)[16..],
        block("000102030405060708090a0b00000000")
    );
    assert_eq!(
        ks(Counter::Low64)[16..],
        block("000102030405060708090a0c00000000")
    );
    assert_eq!(
        ks(Counter::Full128)[16..],
        block("000102030405060708090a0c00000000")
    );
    assert_eq!(
        ks(Counter::Low32)[..16],
        block("000102030405060708090a0bffffffff")
    );
}

#[test]
fn ccm_failure_zeroes_and_decrypt_to_keeps_the_input() {
    let ccm = Ccm::<8, 13>::new(&AesKey::from_slice(&[3; 32]).unwrap());
    let nonce = [5u8; 13];
    let mut msg = *b"a direct message";
    let tag = ccm.encrypt_in_place(&nonce, b"hdr", &mut msg).unwrap();
    let ct = msg;

    let mut bad = tag;
    bad[7] ^= 1;
    let mut buf = ct;
    assert_eq!(
        ccm.decrypt_in_place(&nonce, b"hdr", &mut buf, &bad)
            .unwrap_err(),
        Error::Authentication
    );
    assert_eq!(buf, [0; 16]);
    let mut buf = ct;
    assert_eq!(
        ccm.decrypt_in_place(&nonce, b"hdX", &mut buf, &tag)
            .unwrap_err(),
        Error::Authentication
    );

    // A wrong key, then the right one, on the same untouched ciphertext.
    let wrong = Ccm::<8, 13>::new(&AesKey::from_slice(&[4; 32]).unwrap());
    let mut out = [0xEE; 16];
    assert_eq!(
        wrong
            .decrypt_to(&nonce, b"hdr", &ct, &tag, &mut out)
            .unwrap_err(),
        Error::Authentication
    );
    assert_eq!(out, [0; 16]);
    ccm.decrypt_to(&nonce, b"hdr", &ct, &tag, &mut out).unwrap();
    assert_eq!(&out, b"a direct message");
    assert_eq!(
        ccm.decrypt_to(&nonce, b"hdr", &ct, &tag, &mut out[..3])
            .unwrap_err(),
        Error::OutputLength
    );
}

#[test]
fn ccm_length_limit() {
    // A 13-byte nonce leaves a 2-byte length field: at most 65,535 bytes.
    assert_eq!(Ccm::<4, 13>::MAX_LEN, 65_535);
    assert_eq!(Ccm::<4, 7>::MAX_LEN, usize::MAX);
    let ccm = Ccm::<4, 13>::new(&AesKey::from_slice(&[3; 16]).unwrap());
    let mut big = vec![0u8; 65_536];
    assert_eq!(
        ccm.encrypt_in_place(&[0; 13], &[], &mut big).unwrap_err(),
        Error::TooLong
    );
    assert!(ccm
        .encrypt_in_place(&[0; 13], &[], &mut big[..65_535])
        .is_ok());
    assert_eq!(
        ccm.decrypt_in_place(&[0; 13], &[], &mut big, &[0; 4])
            .unwrap_err(),
        Error::TooLong
    );
}

#[test]
fn gcm_failure_zeroes_and_decrypt_to_keeps_the_input() {
    let gcm = Gcm::new(&AesKey::from_slice(&[3; 32]).unwrap());
    let mut msg = *b"hello from the chat";
    let tag = gcm.encrypt_in_place(&[1; 12], b"hdr", &mut msg).unwrap();
    let ct = msg;
    let mut buf = ct;
    assert_eq!(
        gcm.decrypt_in_place(&[2; 12], b"hdr", &mut buf, &tag)
            .unwrap_err(),
        Error::Authentication
    );
    assert_eq!(buf, [0; 19]);
    let mut out = [0u8; 19];
    gcm.decrypt_to(&[1; 12], b"hdr", &ct, &tag, &mut out)
        .unwrap();
    assert_eq!(&out, b"hello from the chat");
}

#[test]
fn truncated_tags() {
    let key = AesKey::from_slice(&[7; 16]).unwrap();
    let mut c = Cmac::new(&key);
    c.update(b"x");
    let full = c.finalize_reset();
    for bad in [&[][..], &[0u8; 17][..]] {
        c.update(b"x");
        assert_eq!(c.verify_truncated_reset(bad).unwrap_err(), Error::TagLength);
    }
    c.update(b"x");
    let mut t = full[..4].to_vec();
    t[3] ^= 0x80;
    assert_eq!(
        c.verify_truncated_reset(&t).unwrap_err(),
        Error::Authentication
    );
    // After a failure the object is ready again.
    c.update(b"x");
    c.verify_truncated_reset(&full[..4]).unwrap();

    let mut m = HmacSha256::new(b"k");
    assert_eq!(
        m.verify_truncated_reset(&[0; 33]).unwrap_err(),
        Error::TagLength
    );
}

#[test]
fn bad_curve_input() {
    // RFC 7748 section 6.1: a low-order peer key (here u = 0 and u = 1) produces the all-zero secret, which is refused.
    let me = X25519Secret::new(Key::new([0x42; 32]));
    let mut one = [0u8; 32];
    one[0] = 1;
    assert_eq!(
        me.diffie_hellman(&[0; 32]).unwrap_err(),
        Error::NonContributory
    );
    assert_eq!(me.diffie_hellman(&one).unwrap_err(), Error::NonContributory);

    // An Ed25519 y-coordinate with no point on the curve (y = 2), and a changed signature.
    let not_a_point = a::<32>("0200000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(
        ed25519_public_to_x25519(&not_a_point).unwrap_err(),
        Error::PublicKey
    );
    assert_eq!(
        ed25519_verify(&not_a_point, b"", &[0; 64]).unwrap_err(),
        Error::PublicKey
    );
    let k = Ed25519Key::from_seed(&Key::new([1; 32]));
    let mut sig = k.sign(b"m");
    sig[10] ^= 1;
    assert_eq!(
        ed25519_verify(&k.public_key(), b"m", &sig).unwrap_err(),
        Error::Signature
    );
    // A non-canonical S (S + l) is refused by the strict check (RFC 8032 section 5.1.7 step 1).
    let mut sig = k.sign(b"m");
    let l = h("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
    let mut carry = 0u16;
    for i in 0..32 {
        let s = sig[32 + i] as u16 + l[i] as u16 + carry;
        sig[32 + i] = s as u8;
        carry = s >> 8;
    }
    assert!(ed25519_verify(&k.public_key(), b"m", &sig).is_err());
}

#[test]
fn errors_have_messages() {
    for e in [
        Error::KeyLength { got: 3 },
        Error::NotWholeBlocks { block: 16, got: 3 },
        Error::TooLong,
        Error::TagLength,
        Error::OutputLength,
        Error::Authentication,
        Error::PublicKey,
        Error::NonContributory,
        Error::Signature,
    ] {
        assert!(!e.to_string().is_empty());
    }
}
