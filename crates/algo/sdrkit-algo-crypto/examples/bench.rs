//! Times each operation: `cargo run --release -p sdrkit-algo-crypto --example bench`. Each line is the best of several runs. Byte
//! operations are in MB/s on a 255-byte packet (the largest LoRa payload) and on 64 KiB; key operations are in operations per
//! second.
//!
//! On aarch64, build with `RUSTFLAGS="--cfg aes_armv8 --cfg polyval_armv8"` to use the ARMv8 AES and PMULL instructions.

use sdrkit_algo_crypto::curve::ed25519_verify;
use sdrkit_algo_crypto::{
    sha1, sha256, Aes, AesKey, Ccm, Cmac, Counter, Des, Ed25519Key, Gcm, HmacSha256, Key,
    X25519Secret,
};
use std::hint::black_box;
use std::time::Instant;

fn best(mut step: impl FnMut()) -> f64 {
    step();
    let start = Instant::now();
    let mut best = f64::INFINITY;
    let mut runs = 0;
    while runs < 5 || start.elapsed().as_secs_f64() < 0.3 {
        let t = Instant::now();
        for _ in 0..16 {
            step();
        }
        best = best.min(t.elapsed().as_secs_f64() / 16.0);
        runs += 1;
    }
    best
}

fn bytes(name: &str, n: usize, step: impl FnMut()) {
    println!("{name:40} {:10.1} MB/s", n as f64 / best(step) / 1e6);
}

fn ops(name: &str, step: impl FnMut()) {
    println!("{name:40} {:10.0} op/s", 1.0 / best(step));
}

fn main() {
    let k16 = AesKey::from_slice(&[1; 16]).unwrap();
    let k32 = AesKey::from_slice(&[2; 32]).unwrap();
    let aes128 = Aes::new(&k16);
    let aes256 = Aes::new(&k32);
    let ccm = Ccm::<8, 13>::new(&k32);
    let gcm = Gcm::new(&k32);
    let des = Des::new(&[3; 8]);
    let mut cmac = Cmac::new(&k16);
    let mut hmac = HmacSha256::new(&[4; 32]);
    let mut big = vec![0u8; 65_536];
    let mut pkt = [0u8; 255];

    for (label, n) in [("255 B", 255usize), ("64 KiB", 65_536)] {
        let buf: &mut [u8] = if n == 255 { &mut pkt } else { &mut big };
        let whole = n / 16 * 16;
        bytes(&format!("aes-128 ecb decrypt, {label}"), whole, || {
            aes128.decrypt_ecb(&mut buf[..whole]).unwrap()
        });
        bytes(&format!("aes-128 ctr, {label}"), n, || {
            aes128.apply_ctr(&[0; 16], Counter::Low32, buf).unwrap()
        });
        bytes(&format!("aes-256 ctr, {label}"), n, || {
            aes256.apply_ctr(&[0; 16], Counter::Low32, buf).unwrap()
        });
        if n <= Ccm::<8, 13>::MAX_LEN {
            bytes(&format!("aes-256 ccm<8,13> encrypt, {label}"), n, || {
                black_box(ccm.encrypt_in_place(&[1; 13], &[], buf).unwrap());
            });
        }
        bytes(&format!("aes-256 gcm encrypt, {label}"), n, || {
            black_box(gcm.encrypt_in_place(&[1; 12], &[], buf).unwrap());
        });
        bytes(&format!("aes-128 cmac, {label}"), n, || {
            cmac.update(buf);
            black_box(cmac.finalize_reset());
        });
        bytes(&format!("hmac-sha256, {label}"), n, || {
            hmac.update(buf);
            black_box(hmac.finalize_reset());
        });
        bytes(&format!("sha256, {label}"), n, || {
            black_box(sha256(buf));
        });
        bytes(&format!("sha1, {label}"), n, || {
            black_box(sha1(buf));
        });
        let w = n / 8 * 8;
        bytes(&format!("des ecb decrypt, {label}"), w, || {
            des.decrypt_ecb(&mut buf[..w]).unwrap()
        });
    }

    ops("aes-256 key expansion", || {
        black_box(Aes::new(black_box(&k32)));
    });
    let me = X25519Secret::new(Key::new([5; 32]));
    let peer = X25519Secret::new(Key::new([6; 32])).public_key();
    ops("x25519 shared secret", || {
        black_box(me.diffie_hellman(black_box(&peer)).unwrap());
    });
    let ed = Ed25519Key::from_seed(&Key::new([7; 32]));
    let (public, sig) = (ed.public_key(), ed.sign(b"advert"));
    ops("ed25519 sign (64 B)", || {
        black_box(ed.sign(black_box(&[0u8; 64])));
    });
    ops("ed25519 verify (6 B)", || {
        ed25519_verify(&public, b"advert", black_box(&sig)).unwrap()
    });
}
