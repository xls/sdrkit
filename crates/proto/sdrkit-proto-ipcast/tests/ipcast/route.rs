//! ROUTE objects (RFC 9223 clauses 9.2, 9.3) and MIME multipart packages (RFC 2046 clause 5.1).

use sdrkit_proto_ipcast::lct::LctBuilder;
use sdrkit_proto_ipcast::route::{mime_parts, Destination, RouteAssembler};
use std::net::{IpAddr, Ipv4Addr};

/// The destination of the test session.
const DST: Destination = (IpAddr::V4(Ipv4Addr::new(239, 255, 1, 1)), 5000);

fn packets(obj: &[u8], tol: bool, chunk: usize) -> Vec<Vec<u8>> {
    let n = obj.len().div_ceil(chunk);
    (0..n)
        .map(|k| {
            let mut p = Vec::new();
            LctBuilder {
                tsi: 5,
                toi: 9,
                codepoint: 8,
                transfer_len: tol.then_some(obj.len() as u64),
                close_object: k == n - 1,
            }
            .write(&mut p)
            .unwrap();
            p.extend_from_slice(&((k * chunk) as u32).to_be_bytes());
            p.extend_from_slice(&obj[k * chunk..((k + 1) * chunk).min(obj.len())]);
            p
        })
        .collect()
}

#[test]
fn objects_from_shuffled_and_repeated_packets() {
    let obj: Vec<u8> = (0..5000u32).map(|i| (i * 7) as u8).collect();
    let pk = packets(&obj, true, 1400);
    let mut r = RouteAssembler::default();
    let mut got = Vec::new();
    for &i in &[3usize, 0, 0, 2] {
        r.push(&pk[i], DST, 0.0, &mut |o| got.push(o.data.to_vec()))
            .unwrap();
    }
    assert!(got.is_empty());
    assert_eq!(r.open(), 1);
    r.push(&pk[1], DST, 0.0, &mut |o| got.push(o.data.to_vec()))
        .unwrap();
    assert_eq!(got, [obj]);
}

#[test]
fn sessions_of_two_destinations_stay_apart() {
    // Two services send TSI 5, TOI 9 to different addresses: their packets interleave and each object comes out whole.
    let a = vec![0x11u8; 3000];
    let b = vec![0x22u8; 3000];
    let (pa, pb) = (packets(&a, true, 1000), packets(&b, true, 1000));
    let other: Destination = (IpAddr::V4(Ipv4Addr::new(239, 255, 1, 2)), 5000);
    let mut r = RouteAssembler::default();
    let mut got = Vec::new();
    for k in 0..3 {
        r.push(&pa[k], DST, 0.0, &mut |o| {
            got.push((o.dst, o.data.to_vec()))
        })
        .unwrap();
        r.push(&pb[k], other, 0.0, &mut |o| {
            got.push((o.dst, o.data.to_vec()))
        })
        .unwrap();
    }
    assert_eq!(got, [(DST, a), (other, b)]);
}

#[test]
fn the_close_object_flag_returns_the_length_without_ext_tol() {
    let obj = vec![0x5Au8; 3000];
    let pk = packets(&obj, false, 1000);
    let mut r = RouteAssembler::default();
    let mut got = Vec::new();
    for &i in &[2usize, 1, 0] {
        r.push(&pk[i], DST, 0.0, &mut |o| got.push(o.data.len()))
            .unwrap();
    }
    assert_eq!(got, [3000]);
}

#[test]
fn repair_packets_large_objects_and_timeouts() {
    let mut r = RouteAssembler::new(2, 10_000, 5.0);
    let mut p = packets(&[1; 100], true, 100).remove(0);
    p[0] = (p[0] & !3) | 1; // PSI 01: a repair packet
    r.push(&p, DST, 0.0, &mut |_| panic!()).unwrap();
    assert_eq!(r.stats.repair, 1);
    // An EXT_TOL of 5 GB is refused before any buffer is made.
    let mut big = Vec::new();
    LctBuilder {
        tsi: 1,
        toi: 1,
        codepoint: 0,
        transfer_len: Some(5 << 30),
        close_object: false,
    }
    .write(&mut big)
    .unwrap();
    big.extend_from_slice(&[0, 0, 0, 0, 1, 2, 3]);
    assert!(r.push(&big, DST, 0.0, &mut |_| panic!()).is_err());
    // An incomplete object is dropped after the timeout.
    let pk = packets(&[2; 3000], true, 1000);
    r.push(&pk[0], DST, 0.0, &mut |_| panic!()).unwrap();
    r.push(&pk[1], DST, 6.0, &mut |_| panic!()).unwrap();
    assert!(r.stats.dropped >= 2);
}

#[test]
fn mime_package_parts() {
    let m = b"Content-Type: multipart/related; type=\"application/mbms-envelope+xml\"; boundary=\"XX\"\r\n\r\n\
preamble\r\n--XX\r\nContent-Type: application/mbms-user-service-description+xml\r\nContent-Location: usbd.xml\r\n\r\n\
<BundleDescription/>\r\n--XX\r\nContent-Location: stsid.xml\r\n\r\n<S-TSID/>\r\n--XX--\r\n";
    let mut parts = Vec::new();
    let n = mime_parts(m, |p| {
        parts.push((p.content_location.to_string(), p.body.to_vec()))
    })
    .unwrap();
    assert_eq!(n, 2);
    assert_eq!(
        parts[0],
        ("usbd.xml".into(), b"<BundleDescription/>".to_vec())
    );
    assert_eq!(parts[1], ("stsid.xml".into(), b"<S-TSID/>".to_vec()));
    assert!(mime_parts(b"no headers", |_| {}).is_err());
}

#[test]
fn the_assembler_does_not_allocate() {
    let obj = vec![9u8; 4000];
    let pk = packets(&obj, true, 1000);
    let mut r = RouteAssembler::default();
    let mut k = 0;
    let mut n = 0;
    let a = crate::alloc::count(|| {
        r.push(&pk[k % pk.len()], DST, 0.0, &mut |o| n += o.data.len())
            .unwrap();
        k += 1;
    });
    assert_eq!(a, 0);
    assert!(n > 0);
}
