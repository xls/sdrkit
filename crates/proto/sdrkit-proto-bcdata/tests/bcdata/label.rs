use sdrkit_proto_bcdata::label::*;
use sdrkit_proto_bcdata::tags::{Item, Tag};
use sdrkit_proto_bcdata::Error;
use sdrkit_proto_bcdata::Malformed;
use sdrkit_util_text::charset_field::Charset;

fn feed(d: &mut Decoder, segs: &[u8]) -> Vec<Update> {
    let mut v = Vec::new();
    let mut at = 0;
    while at < segs.len() {
        let n = segment_len(&segs[at..]).unwrap();
        v.push(d.push(&segs[at..at + n]).unwrap());
        at += n;
    }
    v
}

#[test]
fn segment_layout_of_clause_7_4_5_2() {
    let mut s = Vec::new();
    write_label("Hi", Charset::EbuLatin, false, &mut s).unwrap();
    // Toggle 0, first 1, last 1, C 0, length − 1 = 1. Charset 0. Then the CRC.
    assert_eq!(&s[..4], &[0x61, 0x00, b'H', b'i']);
    assert_eq!(u16::from_be_bytes([s[4], s[5]]), crate::genibus(&s[..4]));
    // A 20-character label: two segments, the second numbered 1.
    let mut s = Vec::new();
    write_label("ABCDEFGHIJKLMNOPQRST", Charset::Utf8, true, &mut s).unwrap();
    assert_eq!(&s[..2], &[0x80 | 0x40 | 15, 0xF0]);
    assert_eq!(&s[20..22], &[0x80 | 0x20 | 3, 0x10]);
    let mut c = Vec::new();
    write_clear(true, &mut c);
    assert_eq!(&c[..2], &[0x80 | 0x70 | 1, 0]);
    assert_eq!(segment_len(&c), Some(4));
}

#[test]
fn label_with_dl_plus_like_a_station() {
    let mut d = Decoder::new();
    let mut s = Vec::new();
    write_label("Beyoncé - Halo", Charset::Utf8, false, &mut s).unwrap();
    // The DL Plus command comes before its label: it waits for it.
    let mut p = Vec::new();
    let tags = [
        Tag {
            content_type: 4,
            start: 0,
            length_marker: 6,
        },
        Tag {
            content_type: 1,
            start: 10,
            length_marker: 3,
        },
    ];
    write_dl_plus(
        false,
        false,
        Item {
            toggle: false,
            running: true,
        },
        &tags,
        &mut p,
    )
    .unwrap();
    assert_eq!(feed(&mut d, &p), vec![Update::Nothing]);
    assert_eq!(*feed(&mut d, &s).last().unwrap(), Update::Label);
    assert_eq!(d.label(), "Beyoncé - Halo");
    assert_eq!(d.tags().text(4), Some("Beyoncé"));
    assert_eq!(d.tags().text(1), Some("Halo"));
    assert!(d.tags().item.running);
    // A repeat changes nothing.
    assert!(feed(&mut d, &s).iter().all(|&u| u == Update::Nothing));
    assert_eq!(feed(&mut d, &p), vec![Update::Nothing]);
    // A bad CRC leaves the label.
    let mut bad = s.clone();
    bad[3] ^= 1;
    let n = segment_len(&bad).unwrap();
    assert_eq!(d.push(&bad[..n]), Err(Error::Crc));
    assert_eq!(d.label(), "Beyoncé - Halo");
    // A new label (toggle 1) clears the tags. A tag past the end of the label is left out.
    let mut s2 = Vec::new();
    write_label("Next song", Charset::EbuLatin, true, &mut s2).unwrap();
    assert_eq!(*feed(&mut d, &s2).last().unwrap(), Update::Label);
    assert_eq!(d.tags().entries().count(), 0);
    let mut p2 = Vec::new();
    write_dl_plus(
        true,
        true,
        Item::default(),
        &[
            Tag {
                content_type: 1,
                start: 0,
                length_marker: 8,
            },
            Tag {
                content_type: 4,
                start: 5,
                length_marker: 40,
            },
        ],
        &mut p2,
    )
    .unwrap();
    assert_eq!(feed(&mut d, &p2), vec![Update::Tags]);
    assert_eq!(
        d.tags().entries().collect::<Vec<_>>(),
        vec![(1, "Next song")]
    );
    let (link, item, t) = parse_dl_plus_command(&p2).unwrap();
    assert_eq!((link, item.running, t.as_slice().len()), (true, false, 2));
    let mut c = Vec::new();
    write_clear(true, &mut c);
    assert_eq!(feed(&mut d, &c), vec![Update::Cleared]);
    assert_eq!(d.label(), "");
}

#[test]
fn segments_out_of_order_and_charsets() {
    let mut d = Decoder::new();
    let mut s = Vec::new();
    write_label(
        "Radio Łódź 97,4 MHz – Twoje radio",
        Charset::Utf8,
        false,
        &mut s,
    )
    .unwrap();
    // Send the segments last first.
    let mut segs = Vec::new();
    let mut at = 0;
    while at < s.len() {
        let n = segment_len(&s[at..]).unwrap();
        segs.push(s[at..at + n].to_vec());
        at += n;
    }
    segs.reverse();
    for g in &segs {
        d.push(g).unwrap();
    }
    assert_eq!(d.label(), "Radio Łódź 97,4 MHz – Twoje radio");
    let mut s = Vec::new();
    write_label("Größe", Charset::Ucs2, false, &mut s).unwrap();
    let mut d = Decoder::new();
    feed(&mut d, &s);
    assert_eq!((d.label(), d.charset()), ("Größe", Charset::Ucs2));
    assert!(write_label(&"x".repeat(129), Charset::EbuLatin, false, &mut Vec::new()).is_err());
    assert_eq!(
        d.push(&[0x7F, 0x00, 0, 0]),
        Err(Error::Invalid(Malformed::LabelCommand))
    );
}
