//! GOES GVAR: the block format and the receiver on its test signal.

use super::common::{impaired, run};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::gen::test_value;
use sdrkit_stack_satimg::instruments::gvar::{
    build_block, crc, sequence, sync_code, sync_tail, test_scan, Block, Deframer, Header, Imager,
    LineDoc, HEADER_BYTES, INFRARED, INFRARED_WIDTH, SYNC_BITS, VISIBLE, VISIBLE_WIDTH,
};
use sdrkit_stack_satimg::modes::gvar::DESCRIPTOR;

fn bits_of(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|b| (0..8).rev().map(move |k| (b >> k) & 1))
        .collect()
}

#[test]
fn the_sequence_gives_the_sync_code_and_the_block_coding() {
    assert_eq!(sequence().period(40_000), Some(32_767));
    assert_eq!(sync_tail(), 0x1BE7_D01F_BF80_FFFE);
    assert_eq!(sync_code().len(), SYNC_BITS);
    // The sequence after the code, as the coding of a block of zeros shows it: every second byte is complemented.
    let h = Header {
        id: 1,
        word_size: 10,
        word_count: 0x2842,
        data_valid: 1,
        ..Header::default()
    };
    let copy = h.bytes();
    assert_eq!(copy[..9], [0x01, 0x0A, 0x28, 0x42, 0, 0, 0, 0, 0x01]);
    assert_eq!(copy[28..], [0xDA, 0xDA]);
    assert_eq!(crc(b"123456789"), !0x29B1);
    let coded = build_block(&h, &[]);
    assert_eq!(coded[..8], [0x00, 0x08, 0xD6, 0xB8, 0xF9, 0x0D, 0x01, 0x1A]);
    let t: Vec<u8> = (0..8)
        .map(|i| coded[i] ^ copy[i] ^ if i % 2 == 1 { 0xFF } else { 0 })
        .collect();
    assert_eq!(t, [0x01, 0xFD, 0xFE, 0x05, 0xF9, 0xF2, 0x01, 0xE5]);
}

#[test]
fn the_header_is_read_from_a_good_copy_or_by_vote() {
    let h = Header {
        id: 240,
        word_size: 8,
        word_count: 8042,
        product: 3,
        repeat: 1,
        version: 2,
        data_valid: 1,
        sps: 1,
        range: 13 << 4,
        block_count: 0xBEEF,
        crc_ok: true,
    };
    let mut copies: Vec<u8> = (0..3).flat_map(|_| h.bytes()).collect();
    assert_eq!(Header::parse(&copies), h);
    assert_eq!(
        (h.block(), h.spacecraft(), h.data_bits()),
        (Some(0), 13, Some(64_320))
    );
    // The CRC covers the id byte as sent: block 0 is 240.
    copies[HEADER_BYTES + 3] ^= 0x10;
    copies[2 * HEADER_BYTES + 12] ^= 0x01;
    assert_eq!(Header::parse(&copies), h);
    copies[5] ^= 0x80;
    let voted = Header::parse(&copies);
    assert!(!voted.crc_ok);
    assert_eq!(
        Header {
            crc_ok: true,
            ..voted
        },
        h
    );
    let idle = Header {
        id: 15,
        word_size: 8,
        word_count: 2682,
        ..h
    };
    assert_eq!((idle.block(), idle.data_bits()), (Some(15), Some(21_440)));
    assert_eq!(Header { id: 12, ..h }.block(), None);
    assert_eq!(Header { word_size: 7, ..h }.data_bits(), None);
}

/// The blocks of one scan through the deframer.
fn deframed(scan: u32) -> (Vec<Block>, Deframer) {
    let mut bits = vec![0u8; 333];
    for (h, data) in test_scan(scan, 100, test_value) {
        bits.extend_from_slice(&sync_code());
        bits.extend(bits_of(&build_block(&h, &data)));
    }
    bits.extend_from_slice(&[0u8; 64]);
    let mut d = Deframer::default();
    let mut blocks = Vec::new();
    for piece in bits.chunks(77_777) {
        d.push(piece, &mut blocks);
    }
    (blocks, d)
}

#[test]
fn blocks_of_different_lengths_are_found_and_drawn() {
    let (blocks, d) = deframed(700);
    assert_eq!(blocks.len(), 12);
    assert_eq!(
        (d.stats.good, d.stats.bad, d.stats.data_crc_errors),
        (12, 0, 0)
    );
    let bits: Vec<usize> = blocks.iter().map(|b| b.data.len() * 8).collect();
    assert_eq!(bits[0] + 10_032 + 720 + 16, 75_088);
    assert_eq!(bits[11], 64_320);
    assert_eq!(bits[1], 4 * 5252 * 10);
    assert_eq!(bits[2], 3 * 5256 * 10);
    assert_eq!(bits[3], 209_600);
    let ids: Vec<u8> = blocks.iter().map(|b| b.header.id).collect();
    assert_eq!(ids, [240, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    let doc = LineDoc::read(&blocks[2].data, 2 * 5256);
    assert_eq!(
        (doc.scan, doc.channel, doc.detector, doc.pixels, doc.words),
        (700, 6, 1, 5236, 5256)
    );
    let mut imager = Imager::default();
    let mut strips = Vec::new();
    for b in &blocks {
        imager.push(b, &mut strips);
    }
    let ir: Vec<&Strip> = strips.iter().filter(|s| s.instrument == INFRARED).collect();
    assert_eq!(
        ir.iter().map(|s| s.channel).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    for s in &ir {
        assert_eq!((s.first_row, s.width as usize), (699, INFRARED_WIDTH));
        for x in 0..INFRARED_WIDTH {
            assert_eq!(
                u16::from(s.samples[x]),
                test_value(s.channel - 1, 2 * x as u32, 700) >> 2
            );
        }
    }
    let vis: Vec<&Strip> = strips.iter().filter(|s| s.instrument == VISIBLE).collect();
    assert_eq!(
        vis.iter().map(|s| s.first_row).collect::<Vec<_>>(),
        [1398, 1399]
    );
    for (s, block) in vis.iter().zip([3u32, 7]) {
        assert_eq!(s.width as usize, VISIBLE_WIDTH);
        for x in 0..VISIBLE_WIDTH {
            let want = test_value(4, 4 * x as u32, 8 * 700 + block - 3) >> 2;
            assert_eq!(u16::from(s.samples[x]), want);
        }
    }
    assert_eq!(imager.stats.frame_starts, 0);
    let (blocks, _) = deframed(1);
    let mut imager = Imager::default();
    imager.push(&blocks[0], &mut strips);
    assert_eq!(imager.stats.frame_starts, 1);
}

#[test]
fn a_block_without_a_good_header_is_not_read() {
    let (h, data) = test_scan(5, 0, test_value).swap_remove(3);
    let mut coded = build_block(&h, &data);
    for copy in 0..3 {
        coded[copy * HEADER_BYTES + 2 + copy] ^= 0x40;
    }
    let mut bits = sync_code();
    bits.extend(bits_of(&coded));
    let mut d = Deframer::default();
    let mut blocks = Vec::new();
    d.push(&bits, &mut blocks);
    assert_eq!((d.stats.good, d.stats.bad), (0, 1));
    assert!(blocks[0].data.is_empty() && !blocks[0].header.crc_ok);
}

#[test]
fn the_receiver_lists_blocks_and_draws_both_pictures() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(6e6).seconds(2.5),
        Some(25.0),
        3e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = run::<Strip>(&h, &x, 6e6, 1685.7e6, 1685.7e6, 65_536, Some("gvar.out"));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_ok") >= 12.0, "{}", r.num("frames_ok"));
    assert_eq!(r.num("frames_bad"), 0.0);
    assert_eq!(r.num("spacecraft"), 13.0);
    assert!(r.num("visible_rows") >= 2.0 && r.num("infrared_rows") >= 1.0);
    assert_eq!(r.num("data_crc_errors"), 0.0);
    let strips = r.of::<Strip>();
    let s = strips
        .iter()
        .find(|s| s.instrument == INFRARED && s.channel == 3)
        .unwrap();
    let scan = s.first_row + 1;
    assert!((0..INFRARED_WIDTH)
        .all(|x| u16::from(s.samples[x]) == test_value(2, 2 * x as u32, scan) >> 2));
}

#[test]
fn noise_gives_no_block() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
    };
    let x: Vec<Complex32> = (0..(3.0 * 6e6) as usize)
        .map(|_| Complex32::new(next() + next(), next() + next()) * 0.2)
        .collect();
    let h = Harness::new(&DESCRIPTOR).rate(6e6);
    let r = run::<Strip>(&h, &x, 6e6, 1685.7e6, 1685.7e6, 65_536, Some("gvar.out"));
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.num("frames_ok"), 0.0);
    assert!(r.of::<Strip>().is_empty());
}
