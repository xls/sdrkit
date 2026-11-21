//! SSDV packets, pictures and the receiver.

use sci::ssdv::{self, header, Assembly, Framing, PacketCheck, SsdvGenerator, PACKET};
use sci::typed::{SsdvFields, SsdvPictureRow, SSDV_PICTURES};
use sdrkit_core::{Complex32, Generator};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;

fn check(p: &mut [u8; PACKET]) -> Result<usize, ssdv::CheckError> {
    PacketCheck::new().check(p)
}

fn decode_jpeg(j: &[u8]) -> (usize, usize, Vec<u8>) {
    let r = sdrkit_codec_image::jpeg::decode(j).expect("the JPEG decodes");
    let rgb = r.rgb().flatten().collect();
    (r.width() as usize, r.height() as usize, rgb)
}

fn mad(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (*x as f64 - *y as f64).abs())
        .sum::<f64>()
        / a.len() as f64
}

#[test]
fn packets_and_pictures() {
    let pk = ssdv::test_packets("TEST", 7, 96, 64, 6, true);
    assert!((5..=59).contains(&pk.len()), "{}", pk.len());
    let mut a: Option<Assembly> = None;
    for (i, p) in pk.iter().enumerate() {
        let mut q = *p;
        assert_eq!(check(&mut q), Ok(0));
        let h = header(&q);
        assert_eq!(
            (
                h.packet as usize,
                h.image,
                h.width,
                h.height,
                h.fec,
                h.call.as_str()
            ),
            (i, 7, 96, 64, true, "TEST")
        );
        assert_eq!(h.eoi, i + 1 == pk.len());
        match a.as_mut() {
            None => a = Some(Assembly::new(&q, 0.0)),
            Some(a) => {
                a.add(&q, 0.0);
            }
        }
    }
    let a = a.unwrap();
    assert!(a.complete() && a.missing().is_empty());
    let (w, h, px) = decode_jpeg(&a.jpeg);
    assert_eq!((w, h), (96, 64));
    let src = ssdv::test_picture(96, 64);
    let d = mad(&px, &src);
    assert!(d < 20.0, "{d}");
    // Corrupted bytes: 1–16 corrected exactly, 40 refused.
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    for n in 1..=16 {
        let mut q = pk[1];
        let mut used = std::collections::BTreeSet::new();
        while used.len() < n {
            used.insert(1 + rng.below(255) as usize);
        }
        for &i in &used {
            q[i] ^= 1 + rng.below(255) as u8;
        }
        assert_eq!(check(&mut q), Ok(n));
        assert_eq!(q, pk[1]);
    }
    for _ in 0..20 {
        let mut q = pk[1];
        let mut used = std::collections::BTreeSet::new();
        while used.len() < 40 {
            used.insert(1 + rng.below(255) as usize);
        }
        for &i in &used {
            q[i] ^= 1 + rng.below(255) as u8;
        }
        assert!(check(&mut q).is_err());
    }
    // Shuffled with duplicates: the same JPEG; one missing: incomplete but decodes; the late packet completes it.
    let mut b = Assembly::new(&pk[pk.len() - 1], 0.0);
    for p in pk.iter().rev().chain(pk.iter()) {
        b.add(p, 0.0);
    }
    assert_eq!(b.jpeg, a.jpeg);
    let mut c = Assembly::new(&pk[0], 0.0);
    for p in &pk[2..] {
        c.add(p, 0.0);
    }
    assert!(!c.complete());
    assert_eq!(c.missing(), vec![1]);
    let (_, _, px2) = decode_jpeg(&c.jpeg);
    assert!(mad(&px2, &src) < 60.0);
    c.add(&pk[1], 0.0);
    assert!(c.complete());
    assert_eq!(c.jpeg, a.jpeg);
    // No-FEC packets.
    let nf = ssdv::test_packets("TEST", 2, 96, 64, 4, false);
    let mut q = nf[0];
    assert_eq!(check(&mut q), Ok(0));
    q[40] ^= 1;
    assert!(check(&mut q).is_err());
    let mut q: [u8; PACKET] = nf[0];
    q[0] = 0x12;
    assert!(check(&mut q).is_ok());
}

/// The pictures the receiver assembled from the test picture sent with `framing`, `offset_hz` from the station frequency: one
/// completeness flag a picture, and the status line.
fn receive(
    framing: Framing,
    offset_hz: f64,
    ebno_db: Option<f64>,
    inverted: bool,
    baud_setting: sdrkit_stack_science::ssdv::Speed,
) -> (Vec<bool>, String) {
    let rate = 250e3;
    let pk = ssdv::test_packets("TEST", 3, 96, 64, 6, true);
    let mut g = SsdvGenerator::new(rate, &pk, framing, inverted);
    let n = (g.duration() * rate) as usize + 20000;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let mut nco = sdrkit_algo_dsp::nco::Nco::new(rate, offset_hz);
    nco.mix_in_place(&mut x);
    if let Some(e) = ebno_db {
        // Unit power, so Eb = 1 / baud.
        let baud = match framing {
            Framing::Plain(b) | Framing::N82(b) => b as f64,
        };
        let n0 = 1.0 / baud / 10f64.powf(e / 10.0);
        sdrkit_algo_sim::noise::Awgn::with_power(n0 * rate, 4)
            .unwrap()
            .process(&mut x);
    }
    let r = Harness::new(&sci::SSDV)
        .rate(rate)
        .freq(434.65e6)
        .piece(4096)
        .set(&sdrkit_stack_science::ssdv::BAUD_SETTING, baud_setting)
        .feed(&x)
        .unwrap();
    let id = r.results.decl().table_id(SSDV_PICTURES.key).unwrap();
    let complete = r
        .results
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(_, row)| row.at(&SsdvPictureRow::COMPLETE) == Some(true))
        .collect();
    (complete, r.results.status().summary.clone())
}

#[test]
fn receiver_plain_and_8n2() {
    let (p, s) = receive(
        Framing::Plain(9600),
        0.0,
        None,
        false,
        sdrkit_stack_science::ssdv::Speed::Auto,
    );
    assert!(p.first().is_some_and(|c| *c), "{s}");
    assert!(s.contains("9600"), "{s}");
    let (p, s) = receive(
        Framing::Plain(4800),
        1500.0,
        Some(26.0),
        true,
        sdrkit_stack_science::ssdv::Speed::Auto,
    );
    assert!(p.first().is_some_and(|c| *c), "{s}");
    let (p, s) = receive(
        Framing::N82(1200),
        0.0,
        None,
        false,
        sdrkit_stack_science::ssdv::Speed::Auto,
    );
    assert!(p.first().is_some_and(|c| *c), "{s}");
    assert!(s.contains("8N2"), "{s}");
    let (p, _) = receive(
        Framing::Plain(9600),
        0.0,
        None,
        false,
        sdrkit_stack_science::ssdv::Speed::B1200N82,
    );
    assert!(p.is_empty());
}

#[test]
fn the_test_signal_returns_a_picture_through_the_harness() {
    let r = Harness::new(&sci::SSDV)
        .rate(250e3)
        .seconds(6.0)
        .run()
        .unwrap();
    let n = r.results.fields().at(&SsdvFields::PICTURES_DONE).unwrap();
    assert!(n >= 1, "{}", r.results.status().summary);
    assert!(r.media_packets >= 1);
}
