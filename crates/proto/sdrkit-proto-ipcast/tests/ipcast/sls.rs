//! The SLS delivery object in each of its forms (ATSC A/331 clause 7.1.6.1, annex A.3.3.4 and A.3.3.5).

use sdrkit_proto_ipcast::gzip::compress;
use sdrkit_proto_ipcast::route::{mime_package, mime_parts};
use sdrkit_proto_ipcast::sls::{read_sls, xml_root, SlsKind, SlsRead, SLS_MAX_DEPTH};

const USBD: &str = r#"<?xml version="1.0" encoding="UTF-8"?><BundleDescription><UserServiceDescription serviceId="5"/></BundleDescription>"#;
const STSID: &str = r#"<!-- sessions --><S-TSID xmlns="tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/S-TSID/1.0/"><RS/></S-TSID>"#;
const MPD: &str = r#"<mpd:MPD xmlns:mpd="urn:mpeg:dash:schema:mpd:2011"/>"#;

fn gz(b: &[u8]) -> Vec<u8> {
    let mut z = Vec::new();
    compress(b, 6, &mut z);
    z
}

/// The parts between delimiter lines of `boundary`, each with its headers, closed.
fn body(boundary: &str, parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut v = Vec::new();
    for (headers, data) in parts {
        v.extend_from_slice(format!("--{boundary}\r\n{headers}\r\n").as_bytes());
        v.extend_from_slice(data);
        v.extend_from_slice(b"\r\n");
    }
    v.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    v
}

/// A `multipart/related` package of the three documents with its header block.
fn related(boundary: &str) -> Vec<u8> {
    let mut v =
        format!("Content-Type: multipart/related; boundary=\"{boundary}\"\r\n\r\n").into_bytes();
    v.extend(body(
        boundary,
        &[
            (
                "Content-Type: application/route-usd+xml\r\nContent-Location: usbd.xml\r\n",
                USBD.as_bytes(),
            ),
            (
                "Content-Type: application/route-s-tsid+xml\r\nContent-Location: stsid.xml\r\n",
                STSID.as_bytes(),
            ),
            (
                "Content-Type: application/dash+xml\r\nContent-Location: a.mpd\r\n",
                MPD.as_bytes(),
            ),
        ],
    ));
    v
}

fn read(object: &[u8]) -> (SlsRead, Vec<(SlsKind, String)>) {
    let mut docs = Vec::new();
    let r = read_sls(object, &mut |d| {
        docs.push((d.kind, d.content_location.to_string()))
    });
    (r, docs)
}

fn three() -> Vec<(SlsKind, String)> {
    vec![
        (SlsKind::Usbd, "usbd.xml".into()),
        (SlsKind::Stsid, "stsid.xml".into()),
        (SlsKind::Mpd, "a.mpd".into()),
    ]
}

#[test]
fn root_elements() {
    assert_eq!(xml_root(USBD.as_bytes()), Some("BundleDescription"));
    assert_eq!(xml_root(STSID.as_bytes()), Some("S-TSID"));
    assert_eq!(xml_root(MPD.as_bytes()), Some("MPD"));
    assert_eq!(
        xml_root(b"\xEF\xBB\xBF\n<!DOCTYPE x><UserServiceDescription/>"),
        Some("UserServiceDescription")
    );
    assert_eq!(xml_root(b"plain text"), None);
    assert_eq!(xml_root(b"<?xml version=\"1.0\""), None);
    assert_eq!(xml_root(b""), None);
}

#[test]
fn a_package_with_and_without_its_header_block() {
    let with = related("B1");
    let (r, docs) = read(&with);
    assert_eq!(
        (r.documents, r.signed, r.broken_gzip, r.refused),
        (3, false, 0, 0)
    );
    assert_eq!(docs, three());
    // Without the header block the object starts with its delimiter line.
    let at = with.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let without = &with[at..];
    assert!(without.starts_with(b"--B1"));
    let (r, docs) = read(without);
    assert_eq!((r.documents, r.refused), (3, 0));
    assert_eq!(docs, three());
    assert_eq!(mime_parts(without, |_| {}).unwrap(), 3);
    // Line feeds alone end the lines.
    let lf: Vec<u8> = without.iter().copied().filter(|&c| c != b'\r').collect();
    assert_eq!(read(&lf).1, three());
}

#[test]
fn a_compressed_package_and_compressed_parts() {
    let (r, docs) = read(&gz(&related("B2")));
    assert_eq!((r.documents, r.broken_gzip), (3, 0));
    assert_eq!(docs, three());
    // A broken member is counted and not read.
    let mut bad = gz(&related("B2"));
    let k = bad.len() / 2;
    bad[k] ^= 0x55;
    let (r, docs) = read(&bad);
    assert_eq!((r.documents, r.broken_gzip), (0, 1));
    assert!(docs.is_empty());
    // A part with Content-Encoding: gzip, and one that only starts with the gzip magic.
    let (zu, zs) = (gz(USBD.as_bytes()), gz(STSID.as_bytes()));
    let p = body(
        "B3",
        &[
            (
                "Content-Location: usbd.xml\r\nContent-Encoding: gzip\r\n",
                &zu,
            ),
            ("Content-Location: stsid.xml\r\n", &zs),
            ("Content-Location: a.mpd\r\n", MPD.as_bytes()),
        ],
    );
    let (r, docs) = read(&p);
    assert_eq!((r.documents, r.broken_gzip, r.refused), (3, 0, 0));
    assert_eq!(docs, three());
}

#[test]
fn a_signed_package() {
    // Not a real signature: the reader skips the part.
    let signature = [0x30u8, 0x82, 0x01, 0x00, 0xA5, 0x5A];
    let inner = related("IN");
    let at = inner.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let (inner_headers, inner_body) = (
        std::str::from_utf8(&inner[..at + 2]).unwrap(),
        &inner[at + 4..],
    );
    let mut signed =
        b"Content-Type: multipart/signed; protocol=\"application/pkcs7-signature\"; boundary=OUT\r\n\r\n".to_vec();
    signed.extend(body(
        "OUT",
        &[
            (inner_headers, inner_body),
            (
                "Content-Type: application/pkcs7-signature; name=smime.p7s\r\n",
                &signature,
            ),
        ],
    ));
    let (r, docs) = read(&signed);
    assert_eq!((r.documents, r.signed, r.refused), (3, true, 0));
    assert_eq!(docs, three());
    // The signed package compressed.
    let (r, docs) = read(&gz(&signed));
    assert_eq!((r.documents, r.signed), (3, true));
    assert_eq!(docs, three());
    // The inner package read as a part of the outer one.
    let mut n = 0;
    mime_parts(&signed, |p| {
        if p.content_type.starts_with("multipart/related") {
            n = mime_package(p.content_type, p.body, |_| {}).unwrap();
        }
    })
    .unwrap();
    assert_eq!(n, 3);
}

#[test]
fn a_bare_document_and_things_that_are_no_document() {
    let (r, docs) = read(STSID.as_bytes());
    assert_eq!((r.documents, r.signed, r.refused), (1, false, 0));
    assert_eq!(docs, vec![(SlsKind::Stsid, String::new())]);
    assert_eq!(read(&gz(USBD.as_bytes())).0.documents, 1);
    assert_eq!(read(b"").0, SlsRead::default());
    let (r, _) = read(b"neither a package nor XML");
    assert_eq!((r.documents, r.refused), (0, 1));
    let (r, _) = read(&body("B", &[("Content-Location: x\r\n", b"not XML")]));
    assert_eq!((r.documents, r.refused), (0, 1));
}

#[test]
fn packages_nest_to_the_depth_limit() {
    // `levels` packages around one document.
    let nest = |levels: usize| {
        let mut v = STSID.as_bytes().to_vec();
        for l in 0..levels {
            let b = format!("L{l}");
            let h = if l == 0 {
                "Content-Location: stsid.xml\r\n".to_string()
            } else {
                format!("Content-Type: multipart/related; boundary=L{}\r\n", l - 1)
            };
            v = body(&b, &[(h.as_str(), &v)]);
        }
        v
    };
    let (r, docs) = read(&nest(SLS_MAX_DEPTH));
    assert_eq!((r.documents, r.refused), (1, 0));
    assert_eq!(docs[0].0, SlsKind::Stsid);
    let (r, docs) = read(&nest(SLS_MAX_DEPTH + 1));
    assert_eq!((r.documents, r.refused), (0, 1));
    assert!(docs.is_empty());
}

#[test]
fn a_cut_object_gives_no_panic_and_nothing_past_the_cut() {
    let whole = related("CUT");
    let ends: Vec<usize> = [USBD, STSID, MPD]
        .iter()
        .map(|d| {
            let at = whole
                .windows(d.len())
                .position(|w| w == d.as_bytes())
                .unwrap();
            // A part is whole once the delimiter after it is.
            at + d.len() + 2 + 5
        })
        .collect();
    for cut in (0..whole.len()).step_by(7) {
        let (r, docs) = read(&whole[..cut]);
        let want = ends.iter().filter(|&&e| cut >= e).count();
        assert_eq!(docs.len(), want, "cut at {cut}");
        assert_eq!(r.documents, want);
    }
    let z = gz(&whole);
    for cut in (0..z.len()).step_by(5) {
        let (r, docs) = read(&z[..cut]);
        assert!(docs.is_empty() && r.documents == 0, "cut at {cut}");
    }
}
