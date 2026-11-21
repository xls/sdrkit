use sdrkit_core::publish::keys;
use sdrkit_core::{Published, Value};
use sdrkit_proto_bcdata::mot::{self, Header, HeaderWriter};
use sdrkit_proto_bcdata::publish::{self, Publisher};
use sdrkit_proto_bcdata::slideshow::{Categories, PictureFormat, Slide};
use sdrkit_proto_bcdata::spi::{decode, write, Guide};
use sdrkit_proto_bcdata::tags::{parse_rtplus, TagSet};
use std::sync::Arc;

/// A store of the result sets and tables of the shared interface.
fn published() -> Published {
    let shape = publish::interface().stack_shape("bcdata").unwrap();
    Published::from_decl(Arc::new(shape.results_decl()))
}

fn ascii(b: &[u8], s: &mut String) {
    s.extend(b.iter().map(|&c| c as char));
}

/// Artist "ABBA" at 0 and title "SOS" at 7 of "ABBA - SOS".
fn abba() -> TagSet {
    let (item, t) = parse_rtplus(0x0008, (4 << 13) | (3 << 1), (1 << 11) | (7 << 5) | 2);
    let mut set = TagSet::default();
    set.merge_received(item, &t, b"ABBA - SOS", |_| true, ascii);
    set
}

#[test]
fn a_label_and_its_tags_are_written_once() {
    let mut p = published();
    let mut w = Publisher::new();
    let tags = abba();
    w.label(&mut p, "ABBA - SOS", &tags);
    let r = p.results();
    assert_eq!(r.fields().get(&keys::TEXT), Some("ABBA - SOS"));
    assert_eq!(r.fields().get(&publish::ITEM_RUNNING), Some(true));
    let id = r.decl().table_id("label_tags").unwrap();
    let t = r.table(id).unwrap();
    assert_eq!(t.len(), 2);
    let title = t.row("ITEM.TITLE").unwrap();
    assert_eq!(title.get(&keys::TEXT), Some("SOS"));
    assert_eq!(title.get(&publish::CONTENT_CODE), Some(1));
    assert_eq!(t.row("ITEM.ARTIST").unwrap().get(&keys::TEXT), Some("ABBA"));
    // The same data again: no change.
    let before = p.results().change_counter();
    w.label(&mut p, "ABBA - SOS", &tags);
    assert_eq!(p.results().change_counter(), before);
    // Tags that went: their rows go. A cleared display clears the text.
    w.label(&mut p, "News", &TagSet::default());
    let r = p.results();
    assert_eq!(r.table(id).unwrap().len(), 0);
    assert_eq!(r.fields().get(&keys::TEXT), Some("News"));
    w.clear_label(&mut p);
    assert_eq!(
        p.results()
            .fields()
            .value(p.results().decl().field_id("text").unwrap()),
        Some(&Value::None)
    );
}

#[test]
fn the_guide_writes_services_programmes_and_groups() {
    let svc = write::element(
        0x28,
        &[],
        &[
            write::text(0x12, "Test Melody"),
            write::element(0x29, &[(0x80, write::bearer(0xE1, 0x1234, 0xC221, 0))], &[]),
        ],
    );
    let si = write::element(0x03, &[], &[svc]);
    let mut g = Guide::default();
    g.add(&decode(&si).unwrap());
    let mut p = published();
    let mut w = Publisher::new();
    w.guide(&mut p, &g);
    let r = p.results();
    let t = r
        .table(r.decl().table_id("guide_services").unwrap())
        .unwrap();
    let row = t.row("C221").unwrap();
    assert_eq!(row.get(&publish::NAME), Some("Test Melody"));
    assert_eq!(
        (row.get(&publish::SID), row.get(&publish::EID)),
        (Some(0xC221), Some(0x1234))
    );
    // The same guide again: nothing written.
    let before = p.results().change_counter();
    w.guide(&mut p, &g);
    assert_eq!(p.results().change_counter(), before);
    // A new service list without the service: its row goes.
    g.add(&decode(&write::element(0x03, &[], &[])).unwrap());
    w.guide(&mut p, &g);
    let r = p.results();
    assert!(r
        .table(r.decl().table_id("guide_services").unwrap())
        .unwrap()
        .is_empty());
}

#[test]
fn a_slide_shares_its_bytes_and_logos_and_categories_are_rows() {
    let mut b = Vec::new();
    HeaderWriter::new(10, 2, 3)
        .content_name("logo.png")
        .param(mot::param::CATEGORY_SLIDE, &[3, 7])
        .param(mot::param::CATEGORY_TITLE, b"Weather")
        .write(&mut b)
        .unwrap();
    let h = Header::parse(&b).unwrap();
    let slide = Slide::from_header(&h).unwrap();
    let body: Arc<[u8]> = Arc::from(&b"\x89PNG"[..]);
    let mut p = published();
    let mut w = Publisher::new();
    let pic = w.slide(&mut p, &slide, "logo.png", Arc::clone(&body));
    assert!(Arc::ptr_eq(&pic.data, &body));
    assert_eq!(pic.codec, sdrkit_core::media::Codec::Png);
    let r = p.results();
    assert_eq!(r.fields().get(&publish::SLIDE_NAME), Some("logo.png"));
    assert_eq!(r.fields().get(&publish::SLIDE_CATEGORY), Some("Weather"));
    let mut c = Categories::default();
    c.add(&slide, "logo.png");
    w.categories(&mut p, &c);
    w.logo(&mut p, "logo.png", PictureFormat::Png, Arc::clone(&body));
    let r = p.results();
    let cats = r
        .table(r.decl().table_id("slide_categories").unwrap())
        .unwrap();
    assert_eq!(
        cats.row("3/7").unwrap().get(&publish::NAME),
        Some("logo.png")
    );
    let logos = r.table(r.decl().table_id("logos").unwrap()).unwrap();
    assert_eq!(
        logos.row("logo.png").unwrap().get(&publish::PNG),
        Some(&b"\x89PNG"[..])
    );
    c.clear();
    w.categories(&mut p, &c);
    let r = p.results();
    assert!(r
        .table(r.decl().table_id("slide_categories").unwrap())
        .unwrap()
        .is_empty());
}
