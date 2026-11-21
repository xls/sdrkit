use sdrkit_proto_bcdata::datagroup::{self, Header as Dg};
use sdrkit_proto_bcdata::mot::*;
use sdrkit_util_text::charset_field::Charset;

#[test]
fn header_core_layout() {
    // BodySize 1000, HeaderSize 7, ContentType 2, ContentSubType 1: 28 + 13 + 6 + 9 bits.
    let mut b = Vec::new();
    HeaderWriter::new(1000, 2, 1).write(&mut b).unwrap();
    assert_eq!(b, [0x00, 0x00, 0x3E, 0x80, 0x03, 0x84, 0x01]);
    let h = Header::parse(&b).unwrap();
    assert_eq!(
        (
            h.body_size,
            h.header_size,
            h.content_type,
            h.content_subtype
        ),
        (1000, 7, 2, 1)
    );
    assert_eq!(h.mime(), Some("image/jpeg"));
}

#[test]
fn parameters_with_every_pli() {
    let long = vec![b'n'; 200];
    let mut b = Vec::new();
    HeaderWriter::new(5, 2, 3)
        .param(0x0A, &[])
        .param(param::ALERT, &[1])
        .param(param::TRIGGER_TIME, &[0, 0, 0, 0])
        .param(param::CLICK_THROUGH_URL, b"http://a.b/x")
        .param(param::CATEGORY_TITLE, &long)
        .write(&mut b)
        .unwrap();
    // PLI 00, 01, 10, 11 with a 7-bit length, 11 with a 15-bit length.
    assert_eq!(&b[7..9], &[0x0A, 0x40 | 0x29]);
    assert_eq!(b[10], 0x80 | 0x05);
    assert_eq!(&b[15..17], &[0xC0 | 0x27, 12]);
    assert_eq!(&b[29..32], &[0xC0 | 0x26, 0x80, 200]);
    let h = Header::parse(&b).unwrap();
    assert_eq!(h.params().count(), 5);
    assert_eq!(h.param(param::CATEGORY_TITLE).unwrap().len(), 200);
    assert_eq!(h.trigger_time(), Some(MotTime::Now));
    // A cut header is refused.
    assert!(Header::parse(&b[..b.len() - 1]).is_err());
    // A header whose last parameter runs past HeaderSize is refused.
    let mut bad = b.clone();
    let core: u64 = (5 << 28) | (32 << 15) | (2 << 9) | 3; // HeaderSize 32 cuts the 200-byte parameter
    bad[..7].copy_from_slice(&core.to_be_bytes()[1..]);
    assert!(Header::parse(&bad).is_err());
}

#[test]
fn content_name_charsets() {
    let mut b = Vec::new();
    HeaderWriter::new(1, 2, 1)
        .param(param::CONTENT_NAME, &[0x00, b'S', 0x24, b'a'])
        .write(&mut b)
        .unwrap();
    let h = Header::parse(&b).unwrap();
    assert_eq!(h.content_name(), Some((Charset::EbuLatin, &b"S\x24a"[..])));
    assert_eq!(h.content_name_text(), "Sła");
    let mut b = Vec::new();
    HeaderWriter::new(1, 2, 1)
        .content_name("Größe.png")
        .write(&mut b)
        .unwrap();
    assert_eq!(Header::parse(&b).unwrap().content_name_text(), "Größe.png");
}

#[test]
fn mot_time_of_clause_6_2_4_1() {
    // MJD 60000 is 2023-02-25. 13:45 UTC in the short form.
    let t = MotTime::parse(&[0xBA, 0x98, 0x03, 0x6D]).unwrap();
    assert_eq!(
        t,
        MotTime::At {
            unix_s: 1_677_283_200 + 13 * 3600 + 45 * 60,
            ms: 0
        }
    );
    let mut b = Vec::new();
    t.write(&mut b);
    assert_eq!(b, [0xBA, 0x98, 0x03, 0x6D]);
    // The long form: UTC flag, 30 s and 500 ms.
    let t = MotTime::At {
        unix_s: 1_677_283_200 + 30,
        ms: 500,
    };
    let mut b = Vec::new();
    t.write(&mut b);
    assert_eq!(b.len(), 6);
    assert_eq!(b[2] & 0x08, 0x08);
    assert_eq!(u16::from_be_bytes([b[4], b[5]]), (30 << 10) | 500);
    assert_eq!(MotTime::parse(&b).unwrap(), t);
    assert_eq!(MotTime::parse(&[0, 0, 0, 0]).unwrap(), MotTime::Now);
    assert!(MotTime::parse(&[0x80, 0, 0x0F, 0xC0]).is_err()); // hour 63
    assert!(MotTime::parse(&[0x80, 0, 0x08, 0]).is_err()); // long form, 4 bytes
}

#[test]
fn segmentation_header() {
    let mut s = Vec::new();
    write_segment(2, b"abc", &mut s).unwrap();
    assert_eq!(s, [0x40, 0x03, b'a', b'b', b'c']);
    assert_eq!(parse_segment(&s).unwrap(), (2, &b"abc"[..]));
    assert!(parse_segment(&s[..4]).is_err());
}

fn groups(tid: u16, t: u8, part: &[u8], seg_size: usize, out: &mut Vec<Vec<u8>>) {
    let n = part.len().div_ceil(seg_size).max(1);
    for (i, c) in part.chunks(seg_size).enumerate() {
        let mut seg = Vec::new();
        write_segment(0, c, &mut seg).unwrap();
        let mut g = Vec::new();
        Dg {
            group_type: t,
            continuity: i as u8 & 15,
            segment: Some((i + 1 == n, i as u16)),
            transport_id: Some(tid),
            ..Dg::default()
        }
        .write(&seg, &mut g)
        .unwrap();
        out.push(g);
    }
}

fn run(a: &mut Assembler, gs: &[Vec<u8>]) -> Vec<(u16, String, Vec<u8>)> {
    let mut v = Vec::new();
    for g in gs {
        a.push(&datagroup::parse(g).unwrap(), &mut |o| {
            v.push((
                o.transport_id,
                o.header.content_name_text(),
                o.body.to_vec(),
            ))
        });
    }
    v
}

#[test]
fn header_mode_once_repeats_counted_changes_given_again() {
    let body: Vec<u8> = (0..3000u32).map(|i| (i * 3) as u8).collect();
    let mut h = Vec::new();
    HeaderWriter::new(body.len() as u32, 2, 3)
        .content_name("a.png")
        .write(&mut h)
        .unwrap();
    let mut gs = Vec::new();
    groups(9, datagroup::MOT_HEADER, &h, 10, &mut gs);
    groups(9, datagroup::MOT_BODY, &body, 700, &mut gs);
    // Out of order: bodies first.
    gs.rotate_left(2);
    let mut a = Assembler::new();
    assert_eq!(
        run(&mut a, &gs),
        vec![(9, "a.png".to_string(), body.clone())]
    );
    assert!(run(&mut a, &gs).is_empty());
    assert!(a.stats.repeats > 0);
    // A new body with the same transport id is a new version.
    let mut body2 = body.clone();
    body2[5] ^= 1;
    let mut gs2 = Vec::new();
    groups(9, datagroup::MOT_BODY, &body2, 700, &mut gs2);
    assert_eq!(run(&mut a, &gs2), vec![(9, "a.png".to_string(), body2)]);
    // A header announcing one byte more returns nothing.
    let mut h3 = Vec::new();
    HeaderWriter::new(body.len() as u32 + 1, 2, 3)
        .write(&mut h3)
        .unwrap();
    let mut gs3 = Vec::new();
    groups(10, datagroup::MOT_HEADER, &h3, 100, &mut gs3);
    groups(10, datagroup::MOT_BODY, &body, 700, &mut gs3);
    assert!(run(&mut a, &gs3).is_empty());
    assert_eq!(a.stats.dropped, 1);
}

#[test]
fn directory_mode() {
    let b1 = vec![1u8; 500];
    let b2 = vec![2u8; 20];
    let mut h1 = Vec::new();
    HeaderWriter::new(500, 2, 1)
        .content_name("one.jpg")
        .write(&mut h1)
        .unwrap();
    let mut h2 = Vec::new();
    HeaderWriter::new(20, 1, 0)
        .content_name("two.txt")
        .write(&mut h2)
        .unwrap();
    let mut dir = Vec::new();
    write_directory(&[(1, &h1), (2, &h2)], 100, 256, &mut dir).unwrap();
    let d = Directory::parse(&dir).unwrap();
    assert_eq!(
        (d.objects, d.carousel_period, d.segment_size),
        (2, 100, 256)
    );
    assert_eq!(d.header(2).unwrap().content_name_text(), "two.txt");
    assert_eq!(
        u32::from_be_bytes([dir[0], dir[1], dir[2], dir[3]]) as usize,
        dir.len()
    );
    let mut gs = Vec::new();
    groups(1, datagroup::MOT_BODY, &b1, 200, &mut gs);
    groups(2, datagroup::MOT_BODY, &b2, 200, &mut gs);
    groups(100, datagroup::MOT_DIRECTORY, &dir, 30, &mut gs);
    let mut a = Assembler::new();
    let mut got = run(&mut a, &gs);
    got.sort();
    assert_eq!(
        got,
        vec![(1, "one.jpg".into(), b1), (2, "two.txt".into(), b2)]
    );
    assert!(a.directory().is_some());
    // Scrambled bodies and compressed directories are counted.
    let mut gx = Vec::new();
    groups(3, datagroup::MOT_BODY_SCRAMBLED, b"x", 10, &mut gx);
    groups(3, datagroup::MOT_DIRECTORY_COMPRESSED, b"x", 10, &mut gx);
    run(&mut a, &gx);
    assert_eq!((a.stats.scrambled, a.stats.compressed_directories), (1, 1));
    // A directory whose count differs from its entries is refused.
    let mut bad = dir.clone();
    bad[5] = 3;
    assert!(Directory::parse(&bad).is_err());
}

#[test]
fn bodies_above_the_limit_are_dropped() {
    let mut a = Assembler::with_limits(Limits {
        objects: 2,
        body: 1000,
        directory: 1000,
    });
    let mut h = Vec::new();
    HeaderWriter::new(2000, 2, 1).write(&mut h).unwrap();
    let mut gs = Vec::new();
    groups(1, datagroup::MOT_HEADER, &h, 100, &mut gs);
    groups(1, datagroup::MOT_BODY, &[0; 2000], 400, &mut gs);
    assert!(run(&mut a, &gs).is_empty());
    assert!(a.stats.dropped >= 1);
}
