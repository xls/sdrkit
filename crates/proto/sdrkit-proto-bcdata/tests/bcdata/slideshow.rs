use sdrkit_proto_bcdata::mot::{param, Header, HeaderWriter, MotTime};
use sdrkit_proto_bcdata::slideshow::*;

fn header(t: u8, s: u16, f: impl Fn(HeaderWriter) -> HeaderWriter) -> Vec<u8> {
    let mut b = Vec::new();
    f(HeaderWriter::new(10, t, s)).write(&mut b).unwrap();
    b
}

#[test]
fn only_jfif_and_png_are_slides() {
    for (sub, f) in [
        (1, Some(PictureFormat::Jpeg)),
        (3, Some(PictureFormat::Png)),
        (0, None),
        (2, None),
    ] {
        let b = header(2, sub, |w| w);
        assert_eq!(
            Slide::from_header(&Header::parse(&b).unwrap()).map(|s| s.format),
            f
        );
    }
    let b = header(1, 0, |w| w);
    assert!(Slide::from_header(&Header::parse(&b).unwrap()).is_none());
    assert_eq!(
        (PictureFormat::Png.mime(), PictureFormat::Jpeg.codec()),
        ("image/png", sdrkit_core::media::Codec::Jpeg)
    );
}

#[test]
fn ts_101_499_parameters_and_trigger() {
    let b = header(2, 1, |w| {
        w.param(param::CATEGORY_SLIDE, &[4, 9])
            .param(param::CATEGORY_TITLE, b"Weather")
            .param(param::CLICK_THROUGH_URL, b"http://x.y/")
            .param(param::ALTERNATIVE_LOCATION_URL, b"http://x.y/a.jpg")
            .param(param::ALERT, &[1])
            .trigger_time(MotTime::At {
                unix_s: 1000,
                ms: 0,
            })
    });
    let h = Header::parse(&b).unwrap();
    let s = Slide::from_header(&h).unwrap();
    assert_eq!(s.category, Some((4, 9)));
    assert_eq!(s.category_title, Some("Weather"));
    assert_eq!(s.click_through, Some("http://x.y/"));
    assert_eq!(s.alternative_location, Some("http://x.y/a.jpg"));
    assert_eq!(s.alert, Some(1));
    assert!(!s.due(999) && s.due(1000));
    let mut c = Categories::default();
    c.add(&s, "w1.jpg");
    let b2 = header(2, 1, |w| w.param(param::CATEGORY_SLIDE, &[4, 2]));
    let h2 = Header::parse(&b2).unwrap();
    c.add(&Slide::from_header(&h2).unwrap(), "w0.jpg");
    c.add(&Slide::from_header(&h).unwrap(), "w1b.jpg");
    assert_eq!(c.list.len(), 1);
    assert_eq!(c.list[0].title, "Weather");
    assert_eq!(
        c.list[0].slides,
        vec![(2, "w0.jpg".to_string()), (9, "w1b.jpg".to_string())]
    );
}
