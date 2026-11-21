//! Checks that every operation allocates nothing: each is run 1,000 times after its keyed object and buffers are made, under an
//! allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; the library itself forbids `unsafe`.)

use sdrkit_algo_crypto::curve::{ed25519_public_to_x25519, ed25519_verify};
use sdrkit_algo_crypto::mac::{cmac, hmac_sha256};
use sdrkit_algo_crypto::{
    sha1, sha256, Aes, AesKey, Ccm, Cmac, Counter, Des, Ed25519Key, Gcm, HmacSha256, Key, Sha1,
    Sha256, X25519Secret,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(name: &str, mut f: impl FnMut()) {
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..1000 {
        f();
    }
    assert_eq!(
        ALLOCS.load(Ordering::Relaxed) - before,
        0,
        "{name} allocated"
    );
}

// One test function, so no other test thread allocates while counting.
#[test]
fn no_operation_allocates() {
    let k16 = AesKey::from_slice(&[1; 16]).unwrap();
    let k32 = AesKey::from_slice(&[2; 32]).unwrap();
    let aes = Aes::new(&k16);
    let ccm = Ccm::<8, 13>::new(&k32);
    let gcm = Gcm::new(&k32);
    let des = Des::new(&[3; 8]);
    let mut c = Cmac::new(&k16);
    let mut hm = HmacSha256::new(&[4; 32]);
    let mut sh = Sha256::new();
    let mut sh1 = Sha1::new();
    let x = X25519Secret::new(Key::new([5; 32]));
    let ed = Ed25519Key::from_seed(&Key::new([6; 32]));
    let peer = X25519Secret::new(Key::new([7; 32])).public_key();
    let ed_pub = ed.public_key();
    let sig = ed.sign(b"advert");
    let mut buf = [0u8; 255];
    let mut out = [0u8; 255];
    let mut tag8 = [0u8; 8];
    let mut tag16 = [0u8; 16];

    count("aes ecb", || {
        aes.encrypt_ecb(&mut buf[..240]).unwrap();
        aes.decrypt_ecb(&mut buf[..240]).unwrap();
    });
    count("aes ctr", || {
        aes.apply_ctr(&[0; 16], Counter::Low32, &mut buf).unwrap()
    });
    count("aes new", || {
        black_box(Aes::new(&k32));
    });
    count("ccm", || {
        tag8 = ccm.encrypt_in_place(&[1; 13], b"hdr", &mut buf).unwrap();
        ccm.decrypt_to(&[1; 13], b"hdr", &buf, &tag8, &mut out)
            .unwrap();
        ccm.decrypt_in_place(&[1; 13], b"hdr", &mut buf, &tag8)
            .unwrap();
    });
    count("gcm", || {
        tag16 = gcm.encrypt_in_place(&[1; 12], b"hdr", &mut buf).unwrap();
        gcm.decrypt_in_place(&[1; 12], b"hdr", &mut buf, &tag16)
            .unwrap();
        assert!(gcm
            .decrypt_in_place(&[2; 12], b"hdr", &mut buf, &tag16)
            .is_err());
    });
    count("des", || des.decrypt_ecb(&mut buf[..248]).unwrap());
    count("cmac", || {
        c.update(&buf[..16]);
        c.update(&buf[16..29]);
        black_box(c.finalize_reset());
        black_box(cmac(&k16, &buf));
    });
    count("hmac", || {
        hm.update(&buf);
        let _ = black_box(hm.verify_truncated_reset(&[0, 0]));
        black_box(hmac_sha256(&[9; 100], &buf));
    });
    count("sha256", || {
        sh.update(&buf);
        black_box(sh.finalize_reset());
        black_box(sha256(&buf));
    });
    count("sha1", || {
        sh1.update(&buf);
        black_box(sh1.finalize_reset());
        black_box(sha1(&buf));
    });
    count("x25519", || {
        black_box(x.diffie_hellman(&peer).unwrap());
    });
    count("ed25519", || {
        black_box(ed.sign(b"advert"));
        ed25519_verify(&ed_pub, b"advert", &sig).unwrap();
        black_box(ed25519_public_to_x25519(&ed_pub).unwrap());
        black_box(ed.x25519_secret());
    });
}
