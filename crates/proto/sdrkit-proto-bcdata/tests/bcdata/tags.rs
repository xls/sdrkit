use sdrkit_proto_bcdata::tags::*;

#[test]
fn rtplus_group_layout() {
    // Two tags in blocks B, C, D: toggle 1, running 1, type 4 at 0 length 4, type 1 at 7 length 3.
    let b = 0x0018;
    let c = (4 << 13) | (3 << 1);
    let d = (1 << 11) | (7 << 5) | 2;
    let (item, t) = parse_rtplus(b, c, d);
    assert_eq!(
        item,
        Item {
            toggle: true,
            running: true
        }
    );
    assert_eq!(
        t.as_slice(),
        &[
            Tag {
                content_type: 4,
                start: 0,
                length_marker: 3
            },
            Tag {
                content_type: 1,
                start: 7,
                length_marker: 2
            }
        ]
    );
    // Type 63 needs the high bit of type 2 in block C.
    let (_, t) = parse_rtplus(7, 0x0001, 0xF800);
    assert_eq!(
        (t.as_slice()[0].content_type, t.as_slice()[1].content_type),
        (56, 63)
    );
}

#[test]
fn dl_plus_body_layout() {
    let mut b = Vec::new();
    write_dl_plus(
        Item {
            toggle: true,
            running: false,
        },
        &[Tag {
            content_type: 1,
            start: 2,
            length_marker: 3,
        }],
        &mut b,
    )
    .unwrap();
    assert_eq!(b, [0x08, 1, 2, 3]);
    let (item, t) = parse_dl_plus(&b).unwrap();
    assert!(item.toggle && !item.running);
    assert_eq!(t.as_slice().len(), 1);
    assert!(parse_dl_plus(&[0x10]).is_err());
    assert!(parse_dl_plus(&[0x03, 1, 2]).is_err());
    assert!(write_dl_plus(Item::default(), &[], &mut b).is_err());
}

#[test]
fn tag_text_counts_characters_and_names_come_from_ids() {
    let t = Tag {
        content_type: 4,
        start: 2,
        length_marker: 2,
    };
    assert_eq!(t.text("Ñoñó x"), Some("ñó"));
    assert_eq!(
        Tag {
            content_type: 4,
            start: 5,
            length_marker: 5
        }
        .text("short"),
        None
    );
    assert_eq!(
        Tag {
            content_type: 0,
            start: 0,
            length_marker: 1
        }
        .text("ab"),
        None
    );
    assert_eq!(t.info().unwrap().class, "ITEM.ARTIST");
    assert!(Tag {
        content_type: 54,
        ..t
    }
    .info()
    .is_none());
}

/// Bytes as the RDS table reads them in this ASCII range.
fn ascii(b: &[u8], s: &mut String) {
    s.extend(b.iter().map(|&c| c as char));
}

/// An RT+ group: running, item toggle `toggle`, tag 1 (type, start, length marker) and tag 2.
fn group(toggle: bool, t1: (u16, u16, u16), t2: (u16, u16, u16)) -> (Item, Tags) {
    let b = 0x0008 | ((toggle as u16) << 4) | (t1.0 >> 3);
    let c = ((t1.0 & 7) << 13) | (t1.1 << 7) | (t1.2 << 1) | (t2.0 >> 5);
    let d = ((t2.0 & 0x1F) << 11) | (t2.1 << 5) | t2.2;
    parse_rtplus(b, c, d)
}

#[test]
fn rt_plus_waits_for_every_segment_a_tag_covers() {
    // 2A RadioText "Artist - Title of the song", Artist at 0 length marker 5, Title at 9 length marker 16.
    let mut rt = [b' '; 64];
    rt[..27].copy_from_slice(b"Artist - Title of the song\r");
    let mut seen = [false; 16];
    let (item, tags) = group(false, (4, 0, 5), (1, 9, 16));
    let mut s = TagSet::default();
    // Nothing has arrived: no text.
    assert!(!s.merge_received(
        Item {
            running: false,
            ..item
        },
        &tags,
        &rt,
        |i| seen[i / 4],
        ascii
    ));
    assert_eq!((s.text(4), s.text(1)), (None, None));
    assert_eq!(s.tags().len(), 2);
    // Segments 0 and 1 (characters 0 to 7) arrive: the Artist is covered, the Title is not.
    seen[0] = true;
    seen[1] = true;
    assert!(s.refresh(&rt, |i| seen[i / 4], ascii));
    assert_eq!((s.text(4), s.text(1)), (Some("Artist"), None));
    // Segments 2 to 5 arrive, but segment 6 (characters 24 to 27) is still missing.
    seen[2..6].iter_mut().for_each(|x| *x = true);
    assert!(!s.refresh(&rt, |i| seen[i / 4], ascii));
    assert_eq!(s.text(1), None);
    seen[6] = true;
    assert!(s.refresh(&rt, |i| seen[i / 4], ascii));
    assert_eq!(s.text(1), Some("Title of the song"));
    assert!(!s.refresh(&rt, |i| seen[i / 4], ascii));
}

#[test]
fn rt_plus_refuses_a_carriage_return_a_blank_and_the_end() {
    let rt = b"Hello world\r    ";
    let all = |_: usize| true;
    let mut s = TagSet::default();
    // Over the carriage return (11), past the end of the buffer, only spaces: no text. The dummy type 0 is not kept.
    let (item, tags) = group(false, (4, 8, 4), (1, 15, 3));
    s.merge_received(item, &tags, rt, all, ascii);
    let (item, tags) = group(false, (2, 12, 3), (0, 0, 4));
    s.merge_received(item, &tags, rt, all, ascii);
    assert_eq!(s.entries().count(), 0);
    assert_eq!(
        s.tags().iter().map(|t| t.content_type).collect::<Vec<_>>(),
        [1, 2, 4]
    );
    // Artist "Hello" (0, 4) and Title "world" (6, 4). A new tag replaces the kept one of its type.
    let (item, tags) = group(false, (4, 0, 4), (1, 6, 4));
    assert!(s.merge_received(item, &tags, rt, all, ascii));
    assert_eq!(
        s.entries().collect::<Vec<_>>(),
        [(1, "world"), (4, "Hello")]
    );
}

#[test]
fn rt_plus_counts_2b_segments_of_two_characters() {
    // 2B: 32 characters in 16 segments of 2. Title at 2 length marker 2 covers segments 1 and 2.
    let rt = b"A-BCD                           ";
    let mut seen = [false; 16];
    seen[1] = true;
    let (item, tags) = group(false, (1, 2, 2), (0, 0, 0));
    let mut s = TagSet::default();
    s.merge_received(item, &tags, rt, |i| seen[i / 2], ascii);
    assert_eq!(s.text(1), None);
    seen[2] = true;
    s.refresh(rt, |i| seen[i / 2], ascii);
    assert_eq!(s.text(1), Some("BCD"));
}

#[test]
fn rt_plus_toggle_and_clear_forget_texts_and_kept_tags() {
    let rt = b"Hello world\r";
    let all = |_: usize| true;
    let mut s = TagSet::default();
    let (item, tags) = group(false, (4, 0, 4), (1, 6, 4));
    s.merge_received(item, &tags, rt, all, ascii);
    assert_eq!(s.entries().count(), 2);
    // A new item: the old tags go, and only the new group's tag is kept.
    let (item, tags) = group(true, (31, 0, 4), (0, 0, 0));
    assert!(s.merge_received(item, &tags, rt, all, ascii));
    assert_eq!(s.entries().collect::<Vec<_>>(), [(31, "Hello")]);
    assert_eq!(s.tags().len(), 1);
    // The RadioText A/B flag changed: the host clears.
    s.clear();
    assert_eq!((s.entries().count(), s.tags().len()), (0, 0));
    assert!(!s.refresh(rt, all, ascii));
}
