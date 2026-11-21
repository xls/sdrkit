//! Fragmented MP4: ISO/IEC 14496-12 clauses 4.2, 8.8 and 8.16, and the esds descriptors of ISO/IEC 14496-1 clause 8.3.3.

use sdrkit_proto_ipcast::mp4::{
    audio_specific_config, boxes, samples, segment_kind, write_box, Segment, Track,
};

fn full(version: u8, flags: u32, rest: &[u8]) -> Vec<u8> {
    let mut v = vec![
        version,
        (flags >> 16) as u8,
        (flags >> 8) as u8,
        flags as u8,
    ];
    v.extend_from_slice(rest);
    v
}

/// An init segment of one track with a sample entry `entry` holding `config_box`, and a `trex` with default duration 3600.
fn init(entry: &[u8; 4], prefix: usize, config_box: (&[u8; 4], &[u8])) -> Vec<u8> {
    let mut out = Vec::new();
    write_box(&mut out, b"ftyp", b"iso6\0\0\0\0iso6dash");
    let mut tkhd = vec![0u8; 80];
    tkhd[12..16].copy_from_slice(&1u32.to_be_bytes());
    let mut mdhd = vec![0u8; 24];
    mdhd[12..16].copy_from_slice(&90_000u32.to_be_bytes());
    let mut se = vec![0u8; prefix];
    write_box(&mut se, config_box.0, config_box.1);
    let mut stsd = full(0, 0, &1u32.to_be_bytes());
    write_box(&mut stsd, entry, &se);
    let mut stbl = Vec::new();
    write_box(&mut stbl, b"stsd", &stsd);
    let mut minf = Vec::new();
    write_box(&mut minf, b"stbl", &stbl);
    let mut mdia = Vec::new();
    write_box(&mut mdia, b"mdhd", &mdhd);
    write_box(&mut mdia, b"minf", &minf);
    let mut trak = Vec::new();
    write_box(&mut trak, b"tkhd", &tkhd);
    write_box(&mut trak, b"mdia", &mdia);
    let mut trex = full(0, 0, &1u32.to_be_bytes());
    trex.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0x0E, 0x10, 0, 0, 0, 0, 0, 1, 0, 0]);
    let mut mvex = Vec::new();
    write_box(&mut mvex, b"trex", &trex);
    let mut moov = Vec::new();
    write_box(&mut moov, b"trak", &trak);
    write_box(&mut moov, b"mvex", &mvex);
    write_box(&mut out, b"moov", &moov);
    out
}

#[test]
fn init_segment_track_and_codec() {
    let i = init(b"hvc1", 78, (b"hvcC", &[1, 2, 3]));
    assert_eq!(segment_kind(&i), Segment::Init);
    let t = Track::parse_init(&i).unwrap();
    assert_eq!(
        (t.track_id, t.timescale, t.config.clone(), &t.codec_fourcc()),
        (1, 90_000, vec![1, 2, 3], b"hvc1")
    );
    assert_eq!((t.default_duration, t.default_flags), (3600, 0x0001_0000));
    // A short or broken init segment returns an error, never a panic.
    for n in [0, 10, 40, i.len() - 3] {
        assert!(Track::parse_init(&i[..n]).is_err(), "{n}");
    }
}

#[test]
fn esds_descriptors_are_walked_not_scanned() {
    // ES_Descriptor (tag 3) with ES_ID 0x0005, a DecoderConfigDescriptor (tag 4) and DecoderSpecificInfo (tag 5) 0x12 0x10
    // (AAC LC, 44.1 kHz, stereo). A scan for the first byte equal to 5 would read ES_ID as the config.
    let mut dsi = vec![0x05, 0x80, 0x80, 0x02, 0x12, 0x10]; // a 4-byte expandable size
    let mut dc = vec![
        0x04,
        (13 + dsi.len()) as u8,
        0x40,
        0x15,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    dc.append(&mut dsi);
    let mut es = vec![0x03, (3 + dc.len()) as u8, 0x00, 0x05, 0x00];
    es.append(&mut dc);
    let esds = full(0, 0, &es);
    assert_eq!(audio_specific_config(&esds), Some(&[0x12, 0x10][..]));
    let i = init(b"mp4a", 28, (b"esds", &esds));
    let t = Track::parse_init(&i).unwrap();
    assert_eq!(
        (t.config.clone(), &t.codec_fourcc()),
        (vec![0x12, 0x10], b"aac ")
    );
}

fn media(base_offset: Option<u64>, default_base_is_moof: bool, big_moof: bool) -> Vec<u8> {
    let mut seg = Vec::new();
    write_box(&mut seg, b"styp", b"msdh\0\0\0\0msdh");
    let mut flags = 0u32;
    let mut rest = 1u32.to_be_bytes().to_vec();
    if let Some(b) = base_offset {
        flags |= 1;
        rest.extend_from_slice(&b.to_be_bytes());
    }
    if default_base_is_moof {
        flags |= 0x020000;
    }
    let tfhd = full(0, flags, &rest);
    let tfdt = full(1, 0, &180_000u64.to_be_bytes());
    // trun: data offset, sizes and flags of the first sample. Durations come from trex.
    let mut trun = full(0, 0x01 | 0x04 | 0x200, &2u32.to_be_bytes());
    let off_at = trun.len();
    trun.extend_from_slice(&0u32.to_be_bytes());
    trun.extend_from_slice(&0u32.to_be_bytes()); // first sample flags: sync
    trun.extend_from_slice(&3u32.to_be_bytes());
    trun.extend_from_slice(&2u32.to_be_bytes());
    let build = |trun: &[u8]| {
        let mut traf = Vec::new();
        write_box(&mut traf, b"tfhd", &tfhd);
        write_box(&mut traf, b"tfdt", &tfdt);
        write_box(&mut traf, b"trun", trun);
        let mut moof_body = Vec::new();
        write_box(&mut moof_body, b"traf", &traf);
        let mut moof = Vec::new();
        if big_moof {
            // A 64-bit size box (size 1, then the size).
            moof.extend_from_slice(&1u32.to_be_bytes());
            moof.extend_from_slice(b"moof");
            moof.extend_from_slice(&((moof_body.len() + 16) as u64).to_be_bytes());
            moof.extend_from_slice(&moof_body);
        } else {
            write_box(&mut moof, b"moof", &moof_body);
        }
        moof
    };
    let moof_len = build(&trun).len();
    let moof_at = seg.len();
    let data_at = moof_at + moof_len + 8;
    let base = base_offset.unwrap_or(moof_at as u64);
    trun[off_at..off_at + 4].copy_from_slice(&((data_at as u64 - base) as u32).to_be_bytes());
    seg.extend(build(&trun));
    write_box(&mut seg, b"mdat", &[0xA, 0xB, 0xC, 0xD, 0xE]);
    seg
}

#[test]
fn samples_with_every_base_offset_rule() {
    let t = Track::parse_init(&init(b"hvc1", 78, (b"hvcC", &[1]))).unwrap();
    for (base, dbim, big) in [
        (None, true, false),
        (None, false, false),
        (Some(3), false, false),
        (None, true, true),
    ] {
        let seg = media(base, dbim, big);
        assert_eq!(segment_kind(&seg), Segment::Media);
        let mut got = Vec::new();
        let n = samples(&seg, &t, |s| got.push((s.dts, s.keyframe, s.data.to_vec()))).unwrap();
        assert_eq!(n, 2, "{base:?} {dbim} {big}");
        assert_eq!(got[0], (180_000, true, vec![0xA, 0xB, 0xC]));
        assert_eq!(
            got[1],
            (183_600, false, vec![0xD, 0xE]),
            "duration and flags from trex"
        );
    }
}

#[test]
fn box_walk_stops_at_broken_boxes() {
    let mut b = Vec::new();
    write_box(&mut b, b"free", &[0; 4]);
    b.extend_from_slice(&[0, 0, 0, 50, b'm', b'd', b'a', b't', 1, 2]);
    let mut it = boxes(&b);
    assert_eq!(it.next().unwrap().kind, *b"free");
    assert!(it.next().is_none());
    assert!(it.truncated());
    assert!(samples(&b, &Track::default(), |_| {}).is_err());
}
