//! `arq_packet` through its transmitter, a test channel and its receiver, and its selective-repeat layer over a lossy link
//! (Part F section 11).

// Test channels take many plain parameters; profiles are walked by index.
#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

mod common;

use common::Noise;
use sdrkit_core::Complex32;
use sdrkit_stack_custom::arq_packet::arq::{Assembler, Listener, OutBurst, Sender, Stepper};
use sdrkit_stack_custom::arq_packet::rx::{Receiver, RxBurst};
use sdrkit_stack_custom::arq_packet::{
    ack_header, ack_payload, burst, burst_at, max_blocks, one_way, one_way_seconds, parse_ack, shape, Header, FS_TX,
    PROFILES,
};
use std::f64::consts::TAU;

/// A uniform value in `[0, 1)` from a seeded xorshift.
struct Uni(u64);

impl Uni {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A burst at 640 ksps and unit power with the given payload bytes per block.
fn phy_burst(h: &Header, payloads: &[Vec<u8>]) -> Vec<Complex32> {
    shape(&burst(h, payloads))
}

fn random_payloads(profile: usize, blocks: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut u = Uni(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    (0..blocks).map(|_| (0..PROFILES[profile].payload()).map(|_| (u.next() * 256.0) as u8).collect()).collect()
}

/// `lead` zeros, the burst turned by `phase` and moved by `hz`, `tail` zeros, and noise of Es/N0 `esn0_db` (8 samples a
/// symbol at 640 ksps, unit power); no noise when `esn0_db` is `None`.
fn channel(x: &[Complex32], lead: usize, tail: usize, hz: f64, phase: f64, esn0_db: Option<f64>, rate: f64, seed: u64) -> Vec<Complex32> {
    let mut y = vec![Complex32::new(0.0, 0.0); lead];
    y.extend_from_slice(x);
    y.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), tail));
    for (i, v) in y.iter_mut().enumerate() {
        let a = TAU * (hz * i as f64 / rate).fract() + phase;
        *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
    }
    if let Some(db) = esn0_db {
        let sps = rate / 80_000.0;
        Noise::new(seed).add(&mut y, sps / 10f64.powf(db / 10.0));
    }
    y
}

fn receive(y: &[Complex32], rate: f64, chunk: usize) -> (Vec<RxBurst>, u64) {
    let mut rx = Receiver::new(rate);
    let mut out = Vec::new();
    for c in y.chunks(chunk) {
        rx.process(c, &mut out);
    }
    rx.finish(&mut out);
    (out, rx.headers)
}

#[test]
fn clean_loopback_every_profile() {
    for p in 0..9 {
        let h = Header { callsign: "DL1XYZ".into(), profile: p as u8, blocks: 3, seq: 9, flags: 1 };
        let pl = random_payloads(p, 3, p as u64);
        let y = channel(&phy_burst(&h, &pl), 1234, 3000, 37.0, 0.4, None, FS_TX, 1);
        let (b, _) = receive(&y, FS_TX, 4096);
        assert_eq!(b.len(), 1, "profile {p}");
        assert_eq!(b[0].header, h);
        for (i, blk) in b[0].blocks.iter().enumerate() {
            assert_eq!(blk.as_ref(), Some(&pl[i]), "profile {p} block {i}");
        }
        assert!((b[0].offset_hz - 37.0).abs() < 3.0, "profile {p}: {}", b[0].offset_hz);
    }
}

#[test]
fn awgn_at_the_required_es_n0_plus_3_db() {
    let mut u = Uni(77);
    for p in 0..9 {
        let es = PROFILES[p].required_db + 3.0;
        let (mut bad, mut total, mut est) = (0, 0, 0.0);
        let mut heard = 0;
        for t in 0..10 {
            let h = Header { callsign: "N0CALL".into(), profile: p as u8, blocks: 3, seq: t, flags: 0 };
            let pl = random_payloads(p, 3, 100 * p as u64 + u64::from(t));
            let off = u.next() * 100.0 * if t % 2 == 1 { 1.0 } else { -1.0 };
            let lead = 300 + (u.next() * 1500.0) as usize;
            let y = channel(&phy_burst(&h, &pl), lead, 3000, off, u.next() * TAU, Some(es), FS_TX, 1000 + 10 * p as u64 + u64::from(t));
            let (b, _) = receive(&y, FS_TX, 3001);
            total += 3;
            match b.first() {
                Some(r) => {
                    heard += 1;
                    est += r.esn0_db;
                    bad += r.blocks.iter().zip(&pl).filter(|(a, b)| a.as_ref() != Some(*b)).count();
                }
                None => bad += 3,
            }
        }
        let ber = bad as f64 / total as f64;
        let mean = est / f64::from(heard.max(1));
        assert!(ber < 0.05, "profile {p}: block error rate {ber}");
        assert!((mean - es).abs() < 1.5, "profile {p}: Es/N0 {mean} against {es}");
    }
}

#[test]
fn header_sensitivity_at_minus_6_db() {
    let mut u = Uni(5);
    let mut ok = 0;
    for t in 0..60u64 {
        let h = Header { callsign: "DL1XYZ".into(), profile: 0, blocks: 1, seq: (t % 64) as u8, flags: 0 };
        let pl = random_payloads(0, 1, t);
        let off = (u.next() * 2.0 - 1.0) * 100.0;
        let y = channel(&phy_burst(&h, &pl), 1000, 3000, off, u.next() * TAU, Some(-6.0), FS_TX, 500 + t);
        let (_, headers) = receive(&y, FS_TX, 4096);
        ok += usize::from(headers >= 1);
    }
    assert!(ok >= 54, "{ok} of 60 headers");
}

#[test]
fn acknowledgement_over_the_air() {
    let bm: Vec<bool> = (0..300).map(|i| (i * 13 + 5) % 7 < 4).collect();
    let pl = ack_payload(12.0, &bm);
    let h = ack_header("LY2XYZ", 33);
    let y = channel(&phy_burst(&h, &[pl]), 2000, 3000, -60.0, 1.0, Some(0.0), FS_TX, 3);
    let (b, _) = receive(&y, FS_TX, 4096);
    assert_eq!(b.len(), 1);
    assert!(b[0].header.is_ack() && b[0].header.profile == 0);
    assert_eq!((b[0].header.callsign.as_str(), b[0].header.seq), ("LY2XYZ", 33));
    let (snr, back) = parse_ack(b[0].blocks[0].as_ref().expect("the block"));
    assert_eq!(snr, 12);
    assert_eq!(&back[..300], &bm[..]);
    assert!(back[300..].iter().all(|v| !v));
}

#[test]
fn no_false_alarm_on_noise() {
    let mut rx = Receiver::new(FS_TX);
    let mut out = Vec::new();
    let mut x = vec![Complex32::new(0.0, 0.0); 12_000];
    for r in 0..1000u64 {
        x.fill(Complex32::new(0.0, 0.0));
        Noise::new(9000 + r).add(&mut x, 1.0);
        rx.reset();
        rx.process(&x, &mut out);
        rx.finish(&mut out);
    }
    assert_eq!((out.len(), rx.headers), (0, 0));
}

#[test]
fn resampled_input_at_1_and_2_msps() {
    let h = Header { callsign: "DL1XYZ".into(), profile: 2, blocks: 4, seq: 3, flags: 0 };
    let pl = random_payloads(2, 4, 8);
    for rate in [1e6, 2e6] {
        let x = sdrkit_algo_dsp::sinc_table::SincConverter::convert(&phy_burst(&h, &pl), FS_TX, rate).unwrap();
        let y = channel(&x, 5000, 9000, 80.0, 0.2, Some(15.0), rate, 4);
        let (b, _) = receive(&y, rate, 10_000);
        assert_eq!(b.len(), 1, "{rate}");
        assert!(b[0].blocks.iter().zip(&pl).all(|(a, b)| a.as_ref() == Some(b)), "{rate}");
    }
}

#[test]
fn two_bursts_in_order() {
    let h1 = Header { callsign: "DL1XYZ".into(), profile: 3, blocks: 5, seq: 1, flags: 0 };
    let h2 = Header { callsign: "DL1XYZ".into(), profile: 5, blocks: 6, seq: 2, flags: 4 };
    let p1 = random_payloads(3, 5, 1);
    let p2 = random_payloads(5, 6, 2);
    let mut y = channel(&phy_burst(&h1, &p1), 2000, 2000, -40.0, 0.1, None, FS_TX, 0);
    y.extend(channel(&phy_burst(&h2, &p2), 1000, 3000, 55.0, 2.1, None, FS_TX, 0));
    Noise::new(6).add(&mut y, 8.0 / 10f64.powf(2.5));
    let (b, _) = receive(&y, FS_TX, 1000);
    assert_eq!(b.len(), 2);
    assert_eq!((b[0].header.seq, b[1].header.seq), (1, 2));
    assert!(b[0].blocks.iter().zip(&p1).all(|(a, b)| a.as_ref() == Some(b)));
    assert!(b[1].blocks.iter().zip(&p2).all(|(a, b)| a.as_ref() == Some(b)));
}

/// One message over a lossy link without a radio: each block is lost with probability `p_block` and each acknowledgement with
/// probability `p_ack`. Returns the message delivered, the payload bytes sent and the bursts sent.
fn lossy(msg: &[u8], profile: impl Fn(usize) -> usize, per: usize, p_block: f64, p_ack: f64, lose_first: bool, seed: u64) -> (Vec<u8>, usize, usize) {
    let mut u = Uni(seed | 1);
    let mut s = Sender::new(msg, 0);
    let mut asm = Assembler::default();
    let mut i = 0;
    let mut sent = std::collections::HashSet::new();
    while let Some(b) = s.next_burst(profile(i), per) {
        i += 1;
        assert!(i < 100_000, "no progress");
        // With `lose_first`, block 0 of a burst is lost when it carries bytes sent for the first time.
        let first_new = sent.insert(b.blocks[0].offset);
        for x in &b.blocks {
            sent.insert(x.offset);
        }
        let pl = b.payloads();
        let got: Vec<Option<Vec<u8>>> = pl
            .into_iter()
            .enumerate()
            .map(|(k, p)| (!(lose_first && k == 0 && first_new) && u.next() >= p_block).then_some(p))
            .collect();
        let bitmap = asm.take_burst(&got);
        if u.next() < p_ack {
            s.on_timeout();
        } else {
            assert!(s.on_ack(b.seq, &bitmap));
        }
    }
    (asm.complete().expect("a complete message").to_vec(), s.payload_bytes, s.bursts)
}

fn text(n: usize, seed: u64) -> Vec<u8> {
    let mut u = Uni(seed | 1);
    (0..n).map(|_| (u.next() * 256.0) as u8).collect()
}

#[test]
fn selective_repeat_efficiency() {
    let m = text(100_000, 1);
    for loss in [0.0, 0.05, 0.2, 0.5] {
        let (got, sent, _) = lossy(&m, |_| 2, 64, loss, 0.0, false, 7);
        assert_eq!(got, m);
        let eff = m.len() as f64 / sent as f64;
        if loss == 0.0 {
            assert!(eff > 0.95, "{eff}");
        } else if loss < 0.5 {
            assert!(eff >= 0.9 * (1.0 - loss), "loss {loss}: {eff}");
        }
    }
}

#[test]
fn every_profile_with_block_and_ack_loss() {
    for p in 0..9 {
        let m = text(5000 + 77 * p, p as u64 + 3);
        let (got, _, _) = lossy(&m, |_| p, 16, 0.3, 0.2, false, 11 + p as u64);
        assert_eq!(got, m, "profile {p}");
    }
}

#[test]
fn one_byte_and_changing_profiles() {
    let (got, _, bursts) = lossy(b"x", |_| 0, 16, 0.0, 0.0, false, 1);
    assert_eq!((got.as_slice(), bursts), (&b"x"[..], 1));
    let mut s = Sender::new(b"x", 0);
    assert_eq!(s.next_burst(0, 16).map(|b| b.blocks.len()), Some(1));
    let m = text(3000, 9);
    let seq = [8, 2, 5];
    let (got, _, _) = lossy(&m, |i| seq[i % 3], 6, 0.0, 0.0, true, 2);
    assert_eq!(got, m);
}

#[test]
fn wrong_acknowledgement_is_ignored() {
    let mut s = Sender::new(&text(500, 2), 0);
    let b: OutBurst = s.next_burst(2, 4).unwrap();
    assert!(!s.on_ack((b.seq + 1) % 64, &[true; 4]));
    assert_eq!(s.in_flight(), Some(b.seq));
    assert!(s.on_ack(b.seq, &[true; 4]));
    assert_eq!(s.in_flight(), None);
}

#[test]
fn profile_stepping() {
    // Up after 2 good acknowledgements with room.
    let mut st = Stepper::new(2);
    st.on_ack(30.0, 0, 10);
    assert_eq!(st.profile, 2);
    st.on_ack(30.0, 0, 10);
    assert_eq!(st.profile, 3);
    // No step without room.
    let mut st = Stepper::new(2);
    for _ in 0..5 {
        st.on_ack(PROFILES[2].required_db + 2.0, 0, 10);
    }
    assert_eq!(st.profile, 2);
    // Down 1 on failures, then down 2 on the second in a row.
    let mut st = Stepper::new(6);
    st.on_ack(30.0, 5, 10);
    assert_eq!(st.profile, 5);
    st.on_ack(30.0, 5, 10);
    assert_eq!(st.profile, 3);
    // No up for 4 acknowledgements after a step down.
    for _ in 0..4 {
        st.on_ack(40.0, 0, 10);
        assert_eq!(st.profile, 3);
    }
    st.on_ack(40.0, 0, 10);
    st.on_ack(40.0, 0, 10);
    assert_eq!(st.profile, 4);
    // Down 2 on a time-out; down 1 at a margin below 1 dB; clamps at 0 and 8.
    st.on_timeout();
    assert_eq!(st.profile, 2);
    let mut st = Stepper::new(4);
    st.on_ack(PROFILES[4].required_db + 0.5, 0, 10);
    assert_eq!(st.profile, 3);
    let mut st = Stepper::new(1);
    st.on_timeout();
    assert_eq!(st.profile, 0);
    let mut st = Stepper::new(8);
    for _ in 0..6 {
        st.on_ack(80.0, 0, 10);
    }
    assert_eq!(st.profile, 8);
}

/// Noise of Es/N0 `db` on a transmitter output at level 0.35 and rate `rate`.
fn engine_noise(x: &mut [Complex32], db: f64, rate: f64, seed: u64) {
    Noise::new(seed).add(x, 0.1225 * rate / (80_000.0 * 10f64.powf(db / 10.0)));
}

/// A delivered message: the station, the bytes, and whether it was a repeat.
type Got = (String, Vec<u8>, bool);

fn listen(y: &[Complex32], rate: f64, chunk: usize) -> (Vec<Got>, usize, Vec<RxBurst>) {
    let (bursts, _) = receive(y, rate, chunk);
    let mut l = Listener::default();
    let mut got = Vec::new();
    let mut bad = 0;
    for b in &bursts {
        bad += b.blocks.iter().filter(|v| v.is_none()).count();
        if let (_, Some(d)) = l.take(&b.header.callsign, &b.blocks, b.time) {
            got.push((d.callsign, d.bytes, d.repeat));
        }
    }
    (got, bad, bursts)
}

#[test]
fn two_way_session_through_a_noisy_channel() {
    let m = text(1500, 4);
    let mut s = Sender::new(&m, 0);
    let mut rx_data = Receiver::new(FS_TX);
    let mut rx_ack = Receiver::new(FS_TX);
    let mut l = Listener::default();
    let mut delivered = None;
    let mut bursts = 0;
    while let Some(b) = s.next_burst(2, 4) {
        bursts += 1;
        assert!(bursts <= 40, "not done in 40 bursts");
        let h = Header { callsign: "TEST1".into(), profile: 2, blocks: b.blocks.len() as u16, seq: b.seq, flags: b.flags };
        // Every third data burst is lost on the way.
        let y = if bursts % 3 == 0 {
            vec![Complex32::new(0.0, 0.0); 30_000]
        } else {
            channel(&phy_burst(&h, &b.payloads()), 2000, 3000, 60.0, 0.3, Some(9.0), FS_TX, bursts as u64)
        };
        let mut got = Vec::new();
        rx_data.process(&y, &mut got);
        rx_data.finish(&mut got);
        let Some(r) = got.first() else {
            s.on_timeout();
            continue;
        };
        let (bitmap, d) = l.take(&r.header.callsign, &r.blocks, bursts as f64);
        if d.is_some() {
            delivered = d;
        }
        // The answer, and every fourth answer lost.
        if bursts % 4 == 0 {
            s.on_timeout();
            continue;
        }
        let a = channel(&phy_burst(&ack_header("TEST2", r.header.seq), &[ack_payload(r.esn0_db, &bitmap)]), 1500, 3000, -50.0, 1.1, Some(9.0), FS_TX, 1000 + bursts as u64);
        let mut acks = Vec::new();
        rx_ack.process(&a, &mut acks);
        rx_ack.finish(&mut acks);
        match acks.first().filter(|a| a.header.is_ack()).and_then(|a| a.blocks[0].as_ref().map(|p| (a.header.seq, parse_ack(p)))) {
            Some((seq, (_, bm))) => {
                s.on_ack(seq, &bm);
            }
            None => s.on_timeout(),
        }
    }
    assert!(s.done());
    assert_eq!(delivered.expect("the message").bytes, m);
}

#[test]
fn one_way_at_250_500_and_1000_ksps() {
    for rate in [250e3, 500e3, 1e6] {
        let (y, _) = one_way(b"One way test text for the receiver.", "TEST1", 2, 2, rate, 0);
        let (got, bad, bursts) = listen(&y, rate, 8192);
        assert_eq!(got.len(), 2, "{rate}");
        assert!(!got[0].2 && got[1].2, "{rate}: one message, one repeat");
        assert_eq!(bad, 0);
        assert!(bursts.iter().all(|b| b.header.profile == 2));
    }
}

#[test]
fn one_way_with_impairments_at_250_ksps() {
    let rate = 250e3;
    let cases = [
        (2, 0.0, 9.0),
        (2, 200.0, 9.0),
        (2, -250.0, 9.0),
        (0, 150.0, 3.0),
        (5, -200.0, 16.0),
        (8, 100.0, 33.0),
        (2, 2000.0, 9.0),
        (2, -2000.0, 9.0),
        (2, 2500.0, 9.0),
        (0, -2000.0, 1.0),
        (0, 1500.0, 1.0),
        (5, -2400.0, 16.0),
        (8, 2000.0, 33.0),
    ];
    let m = text(600, 5);
    for (i, &(p, hz, db)) in cases.iter().enumerate() {
        let (mut y, _) = one_way(&m, "TEST1", p, 1, rate, 0);
        common::shift(&mut y, hz, rate);
        engine_noise(&mut y, db, rate, 40 + i as u64);
        let (got, _, _) = listen(&y, rate, 5000);
        assert_eq!(got.first().map(|g| &g.1), Some(&m), "profile {p}, {hz} Hz, {db} dB");
    }
}

#[test]
fn forty_kilobytes_in_one_pass() {
    let rate = 250e3;
    let m = text(40_000, 6);
    let (mut y, _) = one_way(&m, "TEST1", 2, 1, rate, 0);
    let t = one_way_seconds(m.len(), 2, 1);
    let len = y.len() as f64 / rate;
    assert!((t - len).abs() < 0.1 * len, "{t} s by the formula, {len} s made");
    common::shift(&mut y, 200.0, rate);
    engine_noise(&mut y, 12.0, rate, 8);
    let (got, _, bursts) = listen(&y, rate, 16_384);
    assert!(bursts.len() >= 3);
    assert_eq!(got.first().map(|g| &g.1), Some(&m));
}

#[test]
fn iq_file_round_trip() {
    let rate = 250e3;
    let m = b"A message written to a file of complex samples and read back.".to_vec();
    let (y, _) = one_way(&m, "TEST1", 2, 1, rate, 0);
    let path = std::env::temp_dir().join(format!("arq_packet_{}.cf32", std::process::id()));
    let bytes: Vec<u8> = y.iter().flat_map(|v| [v.re.to_le_bytes(), v.im.to_le_bytes()].concat()).collect();
    std::fs::write(&path, bytes).unwrap();
    let back = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let z: Vec<Complex32> = back
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| Complex32::new(f32::from_le_bytes([c[0], c[1], c[2], c[3]]), f32::from_le_bytes([c[4], c[5], c[6], c[7]])))
        .collect();
    let (got, _, _) = listen(&z, rate, 4096);
    assert_eq!(got.first().map(|g| &g.1), Some(&m));
}

#[test]
fn odd_input_never_gives_a_burst() {
    let mut u = Uni(3);
    // Every kind of input meets every block size at 48 and 250 ksps; at 1 Msps each kind meets one size.
    let sizes = [1usize, 7, 96, 4801, 65_536];
    let mut runs = Vec::new();
    for rate in [48e3, 250e3] {
        for kind in 0..5 {
            for size in sizes {
                runs.push((rate, kind, size));
            }
        }
    }
    for kind in 0..5 {
        runs.push((1e6, kind, sizes[(kind + 2) % 5]));
    }
    for (rate, kind, size) in runs {
        {
            let n = (4.0 * rate) as usize;
            let x: Vec<Complex32> = (0..n)
                .map(|i| match kind {
                    0 => Complex32::new((u.next() - 0.5) as f32, (u.next() - 0.5) as f32),
                    1 => Complex32::new(0.0, 0.0),
                    2 => Complex32::new(0.7, -0.2),
                    3 => Complex32::new(if u.next() < 0.5 { 1.0 } else { -1.0 }, if u.next() < 0.5 { 1.0 } else { -1.0 }),
                    _ => Complex32::new(if i % 9973 == 0 { 1.0 } else { 0.0 }, 0.0),
                })
                .collect();
            let mut rx = Receiver::new(rate);
            let mut out = Vec::new();
            for c in x.chunks(size) {
                rx.process(c, &mut out);
            }
            rx.finish(&mut out);
            assert_eq!((out.len(), rx.headers), (0, 0), "{rate} kind {kind} blocks of {size}");
        }
    }
}

/// Files made as the description makes its sample files, at 320 ksps: 0.1 s of silence, then 8 bursts of
/// `min(6, max blocks in 100 ms)` blocks of a repeated neutral sentence, each followed by `16 000 + 1000 b` zeros, and noise
/// at an SNR over the non-zero samples.
#[test]
fn sample_files_at_320_ksps() {
    let sentence = b"The quick brown fox jumps over the lazy dog while the packet link sends its test sentence. ";
    for (p, snr) in [(1usize, 12.0), (4, 22.0), (7, 34.0)] {
        let blocks = max_blocks(&PROFILES[p], 100.0).min(6);
        let pl_len = PROFILES[p].payload();
        let mut y = vec![Complex32::new(0.0, 0.0); 32_000];
        let mut sent = Vec::new();
        for b in 0..8u8 {
            let payload: Vec<u8> = sentence.iter().copied().cycle().skip(usize::from(b) * 7).take(blocks * pl_len).collect();
            let pl: Vec<Vec<u8>> = payload.chunks(pl_len).map(<[u8]>::to_vec).collect();
            let h = Header { callsign: "TEST1".into(), profile: p as u8, blocks: blocks as u16, seq: b, flags: 0 };
            y.extend(sdrkit_algo_dsp::sinc_table::SincConverter::convert(&phy_burst(&h, &pl), FS_TX, 320e3).unwrap());
            y.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 16_000 + 1000 * usize::from(b)));
            sent.push(pl);
        }
        let on: Vec<f64> = y.iter().filter(|v| v.norm_sqr() > 0.0).map(|v| f64::from(v.norm_sqr())).collect();
        let ps = on.iter().sum::<f64>() / on.len() as f64;
        Noise::new(p as u64).add(&mut y, ps / 10f64.powf(snr / 10.0));
        let (b, _) = receive(&y, 320e3, 32_768);
        assert_eq!(b.len(), 8, "profile {p}: {:?}", b.iter().map(|r| (r.header.seq, r.time)).collect::<Vec<_>>());
        for (r, s) in b.iter().zip(&sent) {
            assert!(r.blocks.iter().zip(s).all(|(a, b)| a.as_ref() == Some(b)), "profile {p} burst {}", r.header.seq);
        }
    }
}

/// The three recorded files of section 11.6 at 320 ksps, in both sample forms: every block of all 8 bursts decodes, with
/// callsign LY1TST and sequence numbers 0 to 7, and each payload repeats one 99-byte sentence.
#[test]
fn the_recorded_vectors() {
    let Some(root) = common::recordings() else { return };
    let dir = common::folder_with(&root, "_P7_high_cf32.sigmf-data");
    for (stem, p) in [("_P1_low", 1usize), ("_P4_mid", 4), ("_P7_high", 7)] {
        for (name, rate, y) in common::both_forms(&dir, stem) {
            assert_eq!(rate, 320e3, "{name}");
            let blocks = max_blocks(&PROFILES[p], 100.0).min(6);
            let (b, _) = receive(&y, rate, 32_768);
            assert_eq!(b.iter().map(|r| r.header.seq).collect::<Vec<_>>(), (0..8).collect::<Vec<u8>>(), "{name}");
            for r in &b {
                assert_eq!((r.header.callsign.as_str(), usize::from(r.header.profile), usize::from(r.header.blocks)), ("LY1TST", p, blocks), "{name}");
                let bytes: Vec<u8> = r.blocks.iter().map(|x| x.clone().unwrap_or_else(|| panic!("{name} burst {}: a block failed", r.header.seq))).collect::<Vec<_>>().concat();
                assert!(bytes.iter().all(|c| c.is_ascii_graphic() || *c == b' '), "{name}: not text");
                assert!(bytes.iter().zip(bytes.iter().skip(99)).all(|(a, b)| a == b), "{name}: not a repeated 99-byte sentence");
            }
        }
    }
}

#[test]
fn burst_at_the_radio_rate_has_the_level() {
    let h = Header { callsign: "TEST1".into(), profile: 2, blocks: 1, seq: 1, flags: 0 };
    let x = burst_at(&h, &random_payloads(2, 1, 1), 250e3);
    let p = common::power(&x);
    assert!((p - 0.1225).abs() < 0.015, "{p}");
}

/// The test signal through the stack's graph at `offset` Hz with noise of Es/N0 `db`, for `seconds`, with `profile` set.
fn engine(profile: u16, offset: f64, db: f64, seconds: f64) -> sdrkit_core::Results {
    use sdrkit_core::{GenConfig, InputInfo, Output, StackConfig, Value, Values};
    let d = &sdrkit_stack_custom::arq_packet::ARQ_PACKET;
    let rate = d.tuning.rate_hz;
    let mut s = Values::defaults(&(d.settings)());
    let id = s.schema().id("profile").unwrap();
    s.set(id, &Value::Choice(profile)).unwrap();
    let mut g = (d.make_generator.unwrap())(&GenConfig::new(rate, s.clone()).with_seed(1)).unwrap();
    let mut rx = (d.make_receiver)(&StackConfig::new(s)).unwrap();
    rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording()).unwrap();
    let mut out = Output::default();
    let mut x = vec![Complex32::new(0.0, 0.0); 50_000];
    let mut n = 0usize;
    let mut noise = Noise::new(12);
    let var = rate / (80_000.0 * 10f64.powf(db / 10.0));
    for _ in 0..(rate * seconds) as usize / x.len() {
        g.generate(&mut x);
        for (i, v) in x.iter_mut().enumerate() {
            let a = TAU * (offset * (n + i) as f64 / rate).fract();
            *v = *v * Complex32::new(a.cos() as f32, a.sin() as f32) + noise.sample(var);
        }
        n += x.len();
        rx.feed(&x, &mut out);
        out.clear();
    }
    rx.finish(&mut out);
    rx.results().clone()
}

#[test]
fn test_signal_through_the_graph() {
    use sdrkit_core::publish::keys;
    use sdrkit_stack_custom::typed::ArqPacketFields as Ap;
    for (profile, seconds) in [(2u16, 12.0), (0, 16.0)] {
        let r = engine(profile, 1000.0, 15.0, seconds);
        let f = r.fields();
        let good = f.at(&Ap::BLOCKS_GOOD).unwrap_or(0);
        assert!(f.at(&Ap::MESSAGES).unwrap_or(0) >= 3, "profile {profile}");
        assert!(f.at(&Ap::BLOCKS_BAD).unwrap_or(0) * 10 <= good && good > 0);
        assert!((f.get(&keys::OFFSET_HZ).unwrap() - 1000.0).abs() < 200.0);
        assert_eq!(f.at(&Ap::PROFILE), Some(i64::from(profile)));
    }
}

/// The longest burst of every profile (1 s of air time) with a sample-clock error of +20 and -20 ppm, at an Es/N0 of the need
/// of the profile plus 6 dB (at plus 3 dB, the level of the noise test, a few of the up to 331 blocks fail with no clock error
/// either): every block good.
#[test]
fn longest_bursts_with_a_clock_error() {
    for p in 0..9 {
        let nb = max_blocks(&PROFILES[p], 1000.0);
        let es = PROFILES[p].required_db + 6.0;
        for (i, ppm) in [20.0, -20.0].into_iter().enumerate() {
            let h = Header { callsign: "DL1XYZ".into(), profile: p as u8, blocks: nb as u16, seq: 5, flags: 0 };
            let pl = random_payloads(p, nb, 300 + p as u64);
            let x = common::resample(&phy_burst(&h, &pl), ppm, 0.0);
            let y = channel(&x, 1500, 3000, 40.0, 0.7, Some(es), FS_TX, 77 + 2 * p as u64 + i as u64);
            let (b, _) = receive(&y, FS_TX, 8192);
            assert_eq!(b.len(), 1, "profile {p} at {ppm} ppm");
            let bad = b[0].blocks.iter().zip(&pl).filter(|(a, c)| a.as_ref() != Some(*c)).count();
            assert_eq!(bad, 0, "profile {p} at {ppm} ppm: {bad} of {nb} blocks bad");
        }
    }
}

/// The samples a graph sends to its transmitter.
#[derive(Default)]
struct Air(std::sync::Mutex<Vec<Complex32>>);

impl sdrkit_graph::components::HostStreams for Air {
    fn write(&self, port: sdrkit_graph::components::HostPort, data: sdrkit_graph::components::HostData<'_>) {
        if let (sdrkit_graph::components::HostPort::Transmit, sdrkit_graph::components::HostData::Iq { samples, .. }) = (port, data) {
            self.0.lock().unwrap().extend_from_slice(samples);
        }
    }
}

/// The sender component's two-way path in the stack's graph at 250 ksps: `send` starts a session, the far end loses the
/// first data burst and acknowledges every other one through the graph's input, the sender repeats what was not
/// acknowledged, and the far end assembles the whole message.
#[test]
fn two_way_sender_in_the_graph() {
    use sdrkit_core::{InputInfo, Output, Owner, Receiver as _, StackConfig, Value, Values};
    use std::sync::Arc;
    let rate = 250e3;
    let d = &sdrkit_stack_custom::arq_packet::ARQ_PACKET;
    let mut s = Values::defaults(&(d.settings)());
    let schema = s.schema().clone();
    let set = |s: &mut Values, key: &str, v: Value| s.set(schema.id(key).unwrap(), &v).unwrap();
    let message: Vec<u8> = (0..4000u32).map(|i| b'a' + (i * 7 % 26) as u8).collect();
    set(&mut s, "callsign", Value::Text("TEST1".into()));
    set(&mut s, "link", Value::Choice(1));
    set(&mut s, "message", Value::Text(String::from_utf8(message.clone()).unwrap()));
    let mut g = sdrkit_stack_custom::arq_packet::stack::graph(&StackConfig::new(s)).unwrap();
    let air = Arc::new(Air::default());
    g.set_context(sdrkit_graph::RunContext { transmit: true, streams: Some(air.clone()), ..Default::default() });
    g.configure(&InputInfo::new(rate, 0.0, 0.0)).unwrap();
    let send = g.commands().id("send").unwrap();
    let args = g.commands().check(send, &g.commands().def(send).unwrap().new_args()).unwrap();
    g.invoke(send, &args).unwrap();
    let mut far = Receiver::new(rate);
    let mut l = Listener::default();
    let mut answers: Vec<Complex32> = Vec::new();
    let (mut data_bursts, mut lost, mut acks) = (0, 0, 0);
    let mut delivered = None;
    let mut out = Output::default();
    for _ in 0..600 {
        // 0.05 s of input: the queued answers, then quiet.
        let n = 12_500;
        let mut x: Vec<Complex32> = answers.drain(..answers.len().min(n)).collect();
        x.resize(n, Complex32::new(0.0, 0.0));
        g.feed(&x, &mut out);
        out.clear();
        let sent = std::mem::take(&mut *air.0.lock().unwrap());
        let mut got = Vec::new();
        far.process(&sent, &mut got);
        for b in got.into_iter().filter(|b| !b.header.is_ack()) {
            data_bursts += 1;
            if data_bursts == 1 {
                lost += 1;
                continue;
            }
            let (bitmap, m) = l.take(&b.header.callsign, &b.blocks, data_bursts as f64);
            if m.is_some() {
                delivered = m;
            }
            answers.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 5000));
            answers.extend(burst_at(&ack_header("TEST2", b.header.seq), &[ack_payload(b.esn0_db, &bitmap)], rate));
            acks += 1;
        }
        if delivered.is_some() && answers.is_empty() {
            break;
        }
    }
    let m = delivered.unwrap_or_else(|| panic!("no message after {data_bursts} bursts, {acks} acknowledgements"));
    assert_eq!(m.bytes, message);
    assert_eq!(lost, 1);
    assert!(acks >= 2 && data_bursts >= 3, "{data_bursts} bursts, {acks} acknowledgements");
}
