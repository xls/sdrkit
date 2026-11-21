//! Carousel file broadcasts: the computed packets, reassembly out of order and with repeats, tails and silence, hostile
//! packets, the limits, and the cleaning of names.

use sdrkit_proto_ipcast::filecast::{
    clean_name, head_packet, part_packet, tail_packet, FileAssembler, FilecastConfig, FinishedFile,
};

fn asm(p: usize) -> FileAssembler {
    FileAssembler::new(FilecastConfig {
        part_bytes: p,
        ..FilecastConfig::default()
    })
}

fn push(a: &mut FileAssembler, pkt: &[u8], now: f64) -> Vec<FinishedFile> {
    let mut out = Vec::new();
    a.push(pkt, now, &mut |f| out.push(f));
    out
}

#[test]
fn the_computed_packets_are_reproduced() {
    let h = head_packet(0x1000, "A.bz2", 3000, 3, 160);
    assert_eq!((h[1], &h[2..4], &h[4..8]), (0x03, &[0xA0, 0x00][..], &[0x00, 0x10, 0x00, 0x00][..]));
    assert_eq!(h[72..74], [3, 0]);
    assert_eq!(h[84..90], *b"A.bz2\0");
    assert_eq!(h[145..149], [0xB8, 0x0B, 0x00, 0x00]);
    assert_eq!(
        part_packet(0x1000, 2, &[0xAA, 0xBB, 0xCC, 0xDD]),
        [0x00, 0x01, 0x14, 0x00, 0x00, 0x10, 0x00, 0x00, 0x02, 0x00, 0, 0, 0, 0, 0, 0, 0xAA, 0xBB, 0xCC, 0xDD]
    );
    assert_eq!(
        tail_packet(0x1000),
        [0x00, 0xFF, 0x10, 0x00, 0x00, 0x10, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    // With P = 1411, part 2 lands at 2822.
    let mut a = asm(1411);
    push(&mut a, &h, 0.0);
    push(&mut a, &part_packet(0x1000, 2, &[0xAA, 0xBB, 0xCC, 0xDD]), 0.0);
    let f = push(&mut a, &tail_packet(0x1000), 0.0);
    assert_eq!(f.len(), 1);
    assert!(!f[0].complete);
    assert_eq!(f[0].data[2822..2826], [0xAA, 0xBB, 0xCC, 0xDD]);
    assert_eq!(f[0].data.len(), 3000);
}

fn part_data(k: usize) -> Vec<u8> {
    (0..100).map(|i| (k * 100 + i) as u8).collect()
}

#[test]
fn out_of_order_parts_and_repeats_make_one_file() {
    let mut a = asm(100);
    let h = head_packet(7, "FILE.DAT", 250, 3, 242);
    let mut files = Vec::new();
    files.extend(push(&mut a, &h, 0.0));
    files.extend(push(&mut a, &h, 0.1));
    for k in [2usize, 0, 0] {
        files.extend(push(&mut a, &part_packet(7, k as u16, &part_data(k)), 0.2));
    }
    files.extend(push(&mut a, &part_packet(7, 1, &part_data(1)), 0.3));
    assert_eq!(files.len(), 1);
    let f = &files[0];
    assert!(f.complete);
    assert_eq!(f.data.len(), 250);
    assert_eq!((f.data[99], f.data[100], f.data[200]), (99, 100, 200));
    // The same file again after it finished: nothing new, nothing open.
    let mut again = push(&mut a, &h, 10.0);
    for k in 0..3 {
        again.extend(push(&mut a, &part_packet(7, k, &part_data(k as usize)), 10.0));
    }
    again.extend(push(&mut a, &tail_packet(7), 10.0));
    assert!(again.is_empty());
    assert_eq!(a.open_files(), 0);
    assert!(a.stats.repeats >= 4);
}

#[test]
fn a_tail_or_silence_finishes_an_incomplete_file() {
    let mut a = asm(100);
    push(&mut a, &head_packet(1, "ONE.DAT", 250, 3, 242), 0.0);
    push(&mut a, &part_packet(1, 0, &part_data(0)), 0.0);
    let f = push(&mut a, &tail_packet(1), 1.0);
    assert_eq!(f.len(), 1);
    assert!(!f[0].complete);
    assert_eq!(f[0].received, 1);
    push(&mut a, &head_packet(2, "TWO.DAT", 250, 3, 242), 0.0);
    let mut out = Vec::new();
    a.expire(100.0, &mut |f| out.push(f));
    assert!(out.is_empty());
    a.expire(121.0, &mut |f| out.push(f));
    assert_eq!(out.len(), 1);
    assert!(!out[0].complete);
}

#[test]
fn hostile_packets_are_counted_and_open_nothing() {
    let mut a = asm(100);
    let mut no_parts = head_packet(1, "NAME.X", 250, 0, 242);
    no_parts[72] = 0;
    let beyond = head_packet(2, "NAME.X", 1000, 3, 242);
    let no_end: Vec<u8> = {
        let mut h = head_packet(3, "NAME.X", 250, 3, 160);
        for b in &mut h[84..] {
            *b = b'A';
        }
        h
    };
    let mut long = head_packet(4, "NAME.X", 250, 3, 242);
    long[2..4].copy_from_slice(&500u16.to_le_bytes());
    for p in [
        &no_parts[..],
        &beyond,
        &no_end,
        &[0, 1, 2, 3, 4][..],
        &long,
        &part_packet(9, 0, &[1, 2, 3]),
    ] {
        assert!(push(&mut a, p, 0.0).is_empty());
    }
    push(&mut a, &head_packet(5, "GOOD.X", 250, 3, 242), 0.0);
    push(&mut a, &part_packet(5, 3, &[0; 10]), 0.0);
    assert!(a.stats.bad > 3, "{:?}", a.stats);
    assert_eq!(a.stats.orphan, 1);
    assert_eq!(a.open_files(), 1);
}

#[test]
fn the_file_and_memory_limits_hold() {
    let mut a = FileAssembler::new(FilecastConfig {
        part_bytes: 100,
        max_file_bytes: 1000,
        max_open_bytes: 2000,
        ..FilecastConfig::default()
    });
    push(&mut a, &head_packet(1, "BIG.DAT", 1100, 11, 242), 0.0);
    assert_eq!(a.open_files(), 0);
    let mut out = Vec::new();
    for (id, t) in [(10u32, 0.0), (11, 1.0), (12, 2.0)] {
        out.extend(push(&mut a, &head_packet(id, "MID.DAT", 1000, 10, 242), t));
    }
    assert_eq!(a.open_files(), 2);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].id, 10);
    assert_eq!(a.stats.evicted, 1);
}

#[test]
fn names_are_cleaned() {
    assert_eq!(clean_name("../../etc/passwd"), "_.._etc_passwd");
    assert_eq!(clean_name(".."), "");
    assert_eq!(clean_name("C:\\x\ty"), "C__x_y");
    assert_eq!(clean_name(&"a".repeat(300)).len(), 200);
}
