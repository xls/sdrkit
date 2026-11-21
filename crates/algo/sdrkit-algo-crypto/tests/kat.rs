//! Known-answer tests from the standards that define each primitive. Every vector names its document and clause.

mod common;

use common::{a, h};
use sdrkit_algo_crypto::curve::{
    ed25519_public_to_x25519, ed25519_verify, x25519, X25519_BASE_POINT,
};
use sdrkit_algo_crypto::mac::{cmac, hmac_sha256};
use sdrkit_algo_crypto::{
    sha1, sha256, Aes, AesKey, Ccm, Cmac, Counter, Des, Ed25519Key, Gcm, HmacSha256, Key, Sha1,
    Sha256, X25519Secret,
};

fn aes(key: &str) -> Aes {
    Aes::new(&AesKey::from_slice(&h(key)).unwrap())
}

// ----------------------------------------------------------------------------------------------------------------------------
// AES, FIPS 197 Appendix C (example vectors for the three key sizes).

#[test]
fn aes_fips197_appendix_c() {
    let pt = "00112233445566778899aabbccddeeff";
    for (key, ct) in [
        (
            "000102030405060708090a0b0c0d0e0f",
            "69c4e0d86a7b0430d8cdb78070b4c55a",
        ), // C.1
        (
            "000102030405060708090a0b0c0d0e0f1011121314151617",
            "dda97ca4864cdfe06eaf70a0ec0d7191",
        ), // C.2
        (
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
            "8ea2b7ca516745bfeafc49904b496089",
        ), // C.3
    ] {
        let c = aes(key);
        let mut b: [u8; 16] = a(pt);
        c.encrypt_block(&mut b);
        assert_eq!(b, a::<16>(ct), "key {key}");
        c.decrypt_block(&mut b);
        assert_eq!(b, a::<16>(pt));
    }
}

// ----------------------------------------------------------------------------------------------------------------------------
// SP 800-38A Appendix F: the four-block plaintext shared by every example.

const SP800_38A_PT: &str = "6bc1bee22e409f96e93d7e117393172a ae2d8a571e03ac9c9eb76fac45af8e51
                            30c81c46a35ce411e5fbc1191a0a52ef f69f2445df4f9b17ad2b417be66c3710";
const KEY128: &str = "2b7e151628aed2a6abf7158809cf4f3c";
const KEY192: &str = "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b";
const KEY256: &str = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";

#[test]
fn ecb_sp800_38a_f1() {
    for (key, ct) in [
        // F.1.1 / F.1.2 ECB-AES128
        (
            KEY128,
            "3ad77bb40d7a3660a89ecaf32466ef97 f5d3d58503b9699de785895a96fdbaaf
             43b1cd7f598ece23881b00e3ed030688 7b0c785e27e8ad3f8223207104725dd4",
        ),
        // F.1.3 / F.1.4 ECB-AES192
        (
            KEY192,
            "bd334f1d6e45f25ff712a214571fa5cc 974104846d0ad3ad7734ecb3ecee4eef
             ef7afd2270e2e60adce0ba2face6444e 9a4b41ba738d6c72fb16691603c18e0e",
        ),
        // F.1.5 / F.1.6 ECB-AES256
        (
            KEY256,
            "f3eed1bdb5d2a03c064b5a7e3db181f8 591ccb10d410ed26dc5ba74a31362870
             b6ed21b99ca6f4f9f153e7b1beafed1d 23304b7a39f9f3ff067d8d8f9e24ecc7",
        ),
    ] {
        let c = aes(key);
        let mut d = h(SP800_38A_PT);
        c.encrypt_ecb(&mut d).unwrap();
        assert_eq!(d, h(ct));
        c.decrypt_ecb(&mut d).unwrap();
        assert_eq!(d, h(SP800_38A_PT));
    }
}

#[test]
fn ctr_sp800_38a_f5() {
    let counter: [u8; 16] = a("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff");
    for (key, ct) in [
        // F.5.1 / F.5.2 CTR-AES128
        (
            KEY128,
            "874d6191b620e3261bef6864990db6ce 9806f66b7970fdff8617187bb9fffdff
             5ae4df3edbd5d35e5b4f09020db03eab 1e031dda2fbe03d1792170a0f3009cee",
        ),
        // F.5.3 / F.5.4 CTR-AES192
        (
            KEY192,
            "1abc932417521ca24f2b0459fe7e6e0b 090339ec0aa6faefd5ccc2c6f4ce8e94
             1e36b26bd1ebc670d1bd1d665620abf7 4f78a7f6d29809585a97daec58c6b050",
        ),
        // F.5.5 / F.5.6 CTR-AES256
        (
            KEY256,
            "601ec313775789a5b7a7f504bbf3d228 f443e3ca4d62b59aca84e990cacaf5c5
             2b0930daa23de94ce87017ba2d84988d dfc9c58db67aada613c2dd08457941a6",
        ),
    ] {
        let c = aes(key);
        // The examples' counter never carries past its low 32 bits, so every width produces the same keystream.
        for width in [Counter::Low32, Counter::Low64, Counter::Full128] {
            let mut d = h(SP800_38A_PT);
            c.apply_ctr(&counter, width, &mut d).unwrap();
            assert_eq!(d, h(ct), "{width:?}");
            c.apply_ctr(&counter, width, &mut d).unwrap();
            assert_eq!(d, h(SP800_38A_PT));
        }
        // A cut in the middle of a block: the first 23 bytes are the first 23 bytes of the full result.
        let mut d = h(SP800_38A_PT)[..23].to_vec();
        c.apply_ctr(&counter, Counter::Low32, &mut d).unwrap();
        assert_eq!(d, h(ct)[..23]);
    }
}

// ----------------------------------------------------------------------------------------------------------------------------
// CCM, SP 800-38C Appendix C (Examples 1 to 4) and RFC 3610 section 8 (packet vectors 1 and 2).

#[test]
fn ccm_sp800_38c_appendix_c() {
    let key = AesKey::from_slice(&h("404142434445464748494a4b4c4d4e4f")).unwrap();

    // Example 1: Tlen 32, Nlen 56, Alen 64, Plen 32.
    let ccm = Ccm::<4, 7>::new(&key);
    let mut p = h("20212223");
    let t = ccm
        .encrypt_in_place(&a("10111213141516"), &h("0001020304050607"), &mut p)
        .unwrap();
    assert_eq!([p, t.to_vec()].concat(), h("7162015b 4dac255d"));

    // Example 2: Tlen 48, Nlen 64, Alen 128, Plen 128.
    let ccm = Ccm::<6, 8>::new(&key);
    let mut p = h("202122232425262728292a2b2c2d2e2f");
    let t = ccm
        .encrypt_in_place(
            &a("1011121314151617"),
            &h("000102030405060708090a0b0c0d0e0f"),
            &mut p,
        )
        .unwrap();
    assert_eq!(
        [p, t.to_vec()].concat(),
        h("d2a1f0e051ea5f62081a7792073d593d 1fc64fbfaccd")
    );

    // Example 3: Tlen 64, Nlen 96, Alen 160, Plen 192.
    let ccm = Ccm::<8, 12>::new(&key);
    let nonce = a("101112131415161718191a1b");
    let aad = h("000102030405060708090a0b0c0d0e0f10111213");
    let pt = h("202122232425262728292a2b2c2d2e2f3031323334353637");
    let mut p = pt.clone();
    let t = ccm.encrypt_in_place(&nonce, &aad, &mut p).unwrap();
    assert_eq!(
        [p.clone(), t.to_vec()].concat(),
        h("e3b201a9f5b71a7a9b1ceaeccd97e70b6176aad9a4428aa5 484392fbc1b09951")
    );
    ccm.decrypt_in_place(&nonce, &aad, &mut p, &t).unwrap();
    assert_eq!(p, pt);

    // Example 4: Tlen 112, Nlen 104, Alen 524288 (the bytes 00..FF repeated 256 times), Plen 256.
    let ccm = Ccm::<14, 13>::new(&key);
    let aad: Vec<u8> = (0..65_536u32).map(|i| i as u8).collect();
    let mut p = h("202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f");
    let t = ccm
        .encrypt_in_place(&a("101112131415161718191a1b1c"), &aad, &mut p)
        .unwrap();
    assert_eq!(
        [p, t.to_vec()].concat(),
        h("69915dad1e84c6376a68c2967e4dab615ae0fd1faec44cc484828529463ccf72 b4ac6bec93e8598e7f0dadbcea5b")
    );
}

#[test]
fn ccm_rfc3610_packet_vectors() {
    // M = 8, L = 2 (a 13-byte nonce): the sizes Meshtastic direct messages use.
    let ccm =
        Ccm::<8, 13>::new(&AesKey::from_slice(&h("c0c1c2c3c4c5c6c7c8c9cacbcccdcecf")).unwrap());
    // Packet Vector #1.
    let mut p = h("08090a0b0c0d0e0f101112131415161718191a1b1c1d1e");
    let t = ccm
        .encrypt_in_place(
            &a("00000003020100a0a1a2a3a4a5"),
            &h("0001020304050607"),
            &mut p,
        )
        .unwrap();
    assert_eq!(p, h("588c979a61c663d2f066d0c2c0f989806d5f6b61dac384"));
    assert_eq!(t, a::<8>("17e8d12cfdf926e0"));
    // Packet Vector #2.
    let mut p = h("08090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
    let t = ccm
        .encrypt_in_place(
            &a("00000004030201a0a1a2a3a4a5"),
            &h("0001020304050607"),
            &mut p,
        )
        .unwrap();
    assert_eq!(p, h("72c91a36e135f8cf291ca894085c87e3cc15c439c9e43a3b"));
    assert_eq!(t, a::<8>("a091d56e10400916"));
}

// ----------------------------------------------------------------------------------------------------------------------------
// GCM: the test cases of "The Galois/Counter Mode of Operation (GCM)" (McGrew and Viega, Appendix B), the vectors SP 800-38D's
// validation suite starts from. Test cases with a 96-bit IV: 1-4 (AES-128), 7-10 (AES-192), 13-16 (AES-256).

const GCM_P: &str = "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72
                     1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255";
const GCM_A: &str = "feedfacedeadbeeffeedfacedeadbeefabaddad2";
const GCM_IV: &str = "cafebabefacedbaddecaf888";

fn gcm_case(key: &str, iv: &str, p: &[u8], aad: &[u8], c: &str, t: &str) {
    let gcm = Gcm::new(&AesKey::from_slice(&h(key)).unwrap());
    let mut d = p.to_vec();
    let tag = gcm.encrypt_in_place(&a(iv), aad, &mut d).unwrap();
    assert_eq!(d, h(c), "ciphertext, key {key}");
    assert_eq!(tag, a::<16>(t), "tag, key {key}");
    gcm.decrypt_in_place(&a(iv), aad, &mut d, &tag).unwrap();
    assert_eq!(d, p);
}

#[test]
fn gcm_test_cases() {
    let zero = "00000000000000000000000000000000";
    let p4 = &h(GCM_P)[..60];
    let a = h(GCM_A);
    // AES-128
    gcm_case(
        zero,
        "000000000000000000000000",
        &[],
        &[],
        "",
        "58e2fccefa7e3061367f1d57a4e7455a",
    ); // 1
    gcm_case(
        zero,
        "000000000000000000000000",
        &[0; 16],
        &[],
        "0388dace60b6a392f328c2b971b2fe78",
        "ab6e47d42cec13bdf53a67b21257bddf",
    ); // 2
    let k = "feffe9928665731c6d6a8f9467308308";
    let c = "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e
             21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091473f5985";
    gcm_case(
        k,
        GCM_IV,
        &h(GCM_P),
        &[],
        c,
        "4d5c2af327cd64a62cf35abd2ba6fab4",
    ); // 3
    gcm_case(
        k,
        GCM_IV,
        p4,
        &a,
        &c[..c.len() - 8],
        "5bc94fbc3221a5db94fae95ae7121a47",
    ); // 4
       // AES-192
    let zero24 = "000000000000000000000000000000000000000000000000";
    gcm_case(
        zero24,
        "000000000000000000000000",
        &[],
        &[],
        "",
        "cd33b28ac773f74ba00ed1f312572435",
    ); // 7
    gcm_case(
        zero24,
        "000000000000000000000000",
        &[0; 16],
        &[],
        "98e7247c07f0fe411c267e4384b0f600",
        "2ff58d80033927ab8ef4d4587514f0fb",
    ); // 8
    let k = "feffe9928665731c6d6a8f9467308308feffe9928665731c";
    let c = "3980ca0b3c00e841eb06fac4872a2757859e1ceaa6efd984628593b40ca1e19c
             7d773d00c144c525ac619d18c84a3f4718e2448b2fe324d9ccda2710acade256";
    gcm_case(
        k,
        GCM_IV,
        &h(GCM_P),
        &[],
        c,
        "9924a7c8587336bfb118024db8674a14",
    ); // 9
    gcm_case(
        k,
        GCM_IV,
        p4,
        &a,
        &c[..c.len() - 8],
        "2519498e80f1478f37ba55bd6d27618c",
    ); // 10
       // AES-256
    let zero32 = "0000000000000000000000000000000000000000000000000000000000000000";
    gcm_case(
        zero32,
        "000000000000000000000000",
        &[],
        &[],
        "",
        "530f8afbc74536b9a963b4f1c4cb738b",
    ); // 13
    gcm_case(
        zero32,
        "000000000000000000000000",
        &[0; 16],
        &[],
        "cea7403d4d606b6e074ec5d3baf39d18",
        "d0d1c8a799996bf0265b98b5d48ab919",
    ); // 14
    let k = "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308";
    let c = "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad";
    gcm_case(
        k,
        GCM_IV,
        &h(GCM_P),
        &[],
        c,
        "b094dac5d93471bdec1a502270e3cc6c",
    ); // 15
    gcm_case(
        k,
        GCM_IV,
        p4,
        &a,
        &c[..c.len() - 8],
        "76fc6ece0f4e1768cddf8853bb2d551b",
    ); // 16
}

// ----------------------------------------------------------------------------------------------------------------------------
// CMAC: RFC 4493 section 4 (AES-128) and SP 800-38B Appendix D.2 and D.3 (AES-192, AES-256).

#[test]
fn cmac_rfc4493_and_sp800_38b() {
    let m = h(SP800_38A_PT);
    for (key, macs) in [
        (
            KEY128,
            [
                "bb1d6929e95937287fa37d129b756746",
                "070a16b46b4d4144f79bdd9dd04a287c",
                "dfa66747de9ae63030ca32611497c827",
                "51f0bebf7e3b9d92fc49741779363cfe",
            ],
        ),
        (
            KEY192,
            [
                "d17ddf46adaacde531cac483de7a9367",
                "9e99a7bf31e710900662f65e617c5184",
                "8a1de5be2eb31aad089a82e6ee908b0e",
                "a1d5df0eed790f794d77589659f39a11",
            ],
        ),
        (
            KEY256,
            [
                "028962f61b7bf89efc6b551f4667d983",
                "28a7023f452e8f82bd4bf28d8c37c35c",
                "aaf3d8f1de5640c232f5b169b9c911e6",
                "e1992190549f6ed5696a2c056c315410",
            ],
        ),
    ] {
        let k = AesKey::from_slice(&h(key)).unwrap();
        for (len, mac) in [0, 16, 40, 64].into_iter().zip(macs) {
            assert_eq!(cmac(&k, &m[..len]), a::<16>(mac), "key {key}, {len} bytes");
            // The same, fed in uneven pieces through one reused object.
            let mut c = Cmac::new(&k);
            for piece in m[..len].chunks(7) {
                c.update(piece);
            }
            c.verify_truncated_reset(&h(mac)).unwrap();
            c.update(&m[..len]);
            c.verify_truncated_reset(&h(mac)[..4]).unwrap();
        }
    }
}

// ----------------------------------------------------------------------------------------------------------------------------
// SHA-256, FIPS 180-4 with the NIST example values (SHA256.pdf, SHA2_Additional.pdf).

#[test]
fn sha256_fips180_4() {
    assert_eq!(
        sha256(b"abc"),
        a("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
    assert_eq!(
        sha256(b""),
        a("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    );
    assert_eq!(
        sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        a("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1")
    );
    let mut s = Sha256::new();
    let chunk = [b'a'; 1000];
    for _ in 0..1000 {
        s.update(&chunk);
    }
    assert_eq!(
        s.finalize_reset(),
        a("cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0")
    );
    s.update(b"abc");
    assert_eq!(s.finalize_reset(), sha256(b"abc"));
}

// ----------------------------------------------------------------------------------------------------------------------------
// SHA-1, FIPS 180-4 with the NIST example values (SHA1.pdf, SHA2_Additional.pdf), and the WebSocket example of RFC 6455.

#[test]
fn sha1_fips180_4() {
    assert_eq!(sha1(b"abc"), a("a9993e364706816aba3e25717850c26c9cd0d89d"));
    assert_eq!(sha1(b""), a("da39a3ee5e6b4b0d3255bfef95601890afd80709"));
    assert_eq!(
        sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        a("84983e441c3bd26ebaae4aa1f95129e5e54670f1")
    );
    let mut s = Sha1::new();
    let chunk = [b'a'; 1000];
    for _ in 0..1000 {
        s.update(&chunk);
    }
    assert_eq!(
        s.finalize_reset(),
        a("34aa973cd4c4daa4f61eeb2bdbad27316534016f")
    );
    s.update(b"abc");
    assert_eq!(s.finalize_reset(), sha1(b"abc"));
}

/// RFC 6455 section 4.2.2: the key "dGhlIHNhbXBsZSBub25jZQ==" and the GUID give the accept value
/// "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", which is the base64 of this digest.
#[test]
fn sha1_rfc6455_handshake() {
    let input = b"dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    assert_eq!(sha1(input), a("b37a4f2cc0624f1690f64606cf385945b2bec4ea"));
}

// ----------------------------------------------------------------------------------------------------------------------------
// HMAC-SHA256, RFC 4231 section 4 (test cases 1 to 7).

#[test]
fn hmac_sha256_rfc4231() {
    let cases: [(Vec<u8>, Vec<u8>, &str); 7] = [
        (vec![0x0b; 20], b"Hi There".to_vec(), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"),
        (
            b"Jefe".to_vec(),
            b"what do ya want for nothing?".to_vec(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        ),
        (vec![0xaa; 20], vec![0xdd; 50], "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"),
        (
            (1..=25).collect(),
            vec![0xcd; 50],
            "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b",
        ),
        // Test case 5 is a 128-bit truncation.
        (vec![0x0c; 20], b"Test With Truncation".to_vec(), "a3b6167473100ee06e0c796c2955552b"),
        (
            vec![0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        ),
        (
            vec![0xaa; 131],
            b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed \
              before being used by the HMAC algorithm."
                .to_vec(),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2",
        ),
    ];
    for (n, (key, data, mac)) in cases.iter().enumerate() {
        let want = h(mac);
        assert_eq!(
            &hmac_sha256(key, data)[..want.len()],
            &want[..],
            "test case {}",
            n + 1
        );
        let mut m = HmacSha256::new(key);
        m.update(data);
        m.verify_truncated_reset(&want).unwrap();
    }
    // A 64-byte key is used as it is (no hashing, no padding): the boundary between RFC 2104's two key rules.
    let k64 = [0x5a; 64];
    let mut k65 = [0x5a; 65];
    assert_ne!(hmac_sha256(&k64, b"x"), hmac_sha256(&k65, b"x"));
    k65[64] = 0;
    assert_ne!(hmac_sha256(&k64, b"x"), hmac_sha256(&k65, b"x"));
}

// ----------------------------------------------------------------------------------------------------------------------------
// X25519, RFC 7748 sections 5.2 and 6.1.

#[test]
fn x25519_rfc7748_section_5_2() {
    assert_eq!(
        x25519(
            &a("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"),
            &a("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c"),
        ),
        a("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552")
    );
    assert_eq!(
        x25519(
            &a("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d"),
            &a("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493"),
        ),
        a("95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957")
    );
    // The iterated test: k = u = 9, then k, u = X25519(k, u), k.
    let (mut k, mut u) = (X25519_BASE_POINT, X25519_BASE_POINT);
    for i in 1..=1000 {
        let r = x25519(&k, &u);
        u = k;
        k = r;
        if i == 1 {
            assert_eq!(
                k,
                a::<32>("422c8e7a6227d7bca1350b3e2bb7279f7897b87bb6854b783c60e80311ae3079")
            );
        }
    }
    assert_eq!(
        k,
        a::<32>("684cf59ba83309552800ef566f2f4d3c1c3887c49360e3875f2eb94d99532c51")
    );
}

#[test]
fn x25519_rfc7748_section_6_1() {
    let alice = X25519Secret::new(Key::new(a(
        "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
    )));
    let bob = X25519Secret::new(Key::new(a(
        "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb",
    )));
    assert_eq!(
        alice.public_key(),
        a::<32>("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
    );
    assert_eq!(
        bob.public_key(),
        a::<32>("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")
    );
    let shared = a::<32>("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
    assert_eq!(
        alice.diffie_hellman(&bob.public_key()).unwrap().as_bytes(),
        &shared
    );
    assert_eq!(
        bob.diffie_hellman(&alice.public_key()).unwrap().as_bytes(),
        &shared
    );
}

// ----------------------------------------------------------------------------------------------------------------------------
// Ed25519, RFC 8032 section 7.1 (tests 1 to 3).

#[test]
fn ed25519_rfc8032_section_7_1() {
    for (seed, public, msg, sig) in [
        (
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "",
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155
             5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        ),
        (
            "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "72",
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da
             085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
        ),
        (
            "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            "af82",
            "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac
             18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
        ),
    ] {
        let key = Ed25519Key::from_seed(&Key::new(a(seed)));
        assert_eq!(key.public_key(), a::<32>(public));
        let s = key.sign(&h(msg));
        assert_eq!(s, a::<64>(sig));
        ed25519_verify(&a(public), &h(msg), &s).unwrap();
    }
}

#[test]
fn ed25519_to_x25519_conversions_agree() {
    // The X25519 public key of an identity's converted secret is the Montgomery form of its Ed25519 public key, and two
    // identities agree on one shared secret (the MeshCore direct-message key).
    let a_id = Ed25519Key::from_seed(&Key::new(a(
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
    )));
    let b_id = Ed25519Key::from_seed(&Key::new(a(
        "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
    )));
    for id in [&a_id, &b_id] {
        assert_eq!(
            id.x25519_secret().public_key(),
            ed25519_public_to_x25519(&id.public_key()).unwrap()
        );
    }
    let ab = a_id
        .x25519_secret()
        .diffie_hellman(&ed25519_public_to_x25519(&b_id.public_key()).unwrap())
        .unwrap();
    let ba = b_id
        .x25519_secret()
        .diffie_hellman(&ed25519_public_to_x25519(&a_id.public_key()).unwrap())
        .unwrap();
    assert_eq!(ab.as_bytes(), ba.as_bytes());
}

// ----------------------------------------------------------------------------------------------------------------------------
// DES: the FIPS 46 worked example key, and FIPS 81 Appendix B Table B1 (ECB, "Now is the time for all ").

#[test]
fn des_fips46_and_fips81() {
    let des = Des::new(&a("133457799bbcdff1"));
    let mut d = h("0123456789abcdef");
    des.encrypt_ecb(&mut d).unwrap();
    assert_eq!(d, h("85e813540f0ab405"));
    des.decrypt_ecb(&mut d).unwrap();
    assert_eq!(d, h("0123456789abcdef"));

    let des = Des::new(&a("0123456789abcdef"));
    let pt = b"Now is the time for all ".to_vec();
    let mut d = pt.clone();
    des.encrypt_ecb(&mut d).unwrap();
    assert_eq!(d, h("3fa40e8a984d4815 6a271787ab8883f9 893d51ec4b563b53"));
    des.decrypt_ecb(&mut d).unwrap();
    assert_eq!(d, pt);
}
