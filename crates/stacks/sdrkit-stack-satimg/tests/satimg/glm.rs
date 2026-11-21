//! GOES-R Rebroadcast GLM data: units built by hand from the record tables of the guide, the test products through the
//! packet layer (a split payload, a missing packet), impossible values, a compressed payload, and the test signal through
//! the whole stack.

use super::common::{cell_i64, impaired, run, table_rows};
use sdrkit_codec_image::strip::Strip;
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::modes::glm::{
    self, decode_events, decode_flashes, decode_groups, BadLength, GlmQuality, GlmTestSource,
    RawFlash, APID_FLASHES, APID_GROUPS,
};
use sdrkit_stack_satimg::modes::grb::{GrbDecoder, GrbOut, DESCRIPTOR};
use sdrkit_stack_satimg::typed::GlmFlashRow;

/// Product time of the first test product, seconds since 1970: J2000 noon plus 815 000 000.25 s.
const PRODUCT: f64 = 946_728_000.0 + 815_000_000.25;

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

/// A unit of one flash, written byte by byte from Table 7.2.1.6.1.1 (little endian): id 1, time offsets 20000, 20524,
/// 19990, 20520, latitude 29.5, longitude -95.25, area 100, energy 6553, quality 0.
const FLASH_BYTES: [u8; 32] = [
    1, 0, 0, 0, 0, 0, 0, 0, // count
    0x01, 0x00, 0x20, 0x4E, 0x2C, 0x50, 0x16, 0x4E, 0x28, 0x50, // id, times
    0x00, 0x00, 0xEC, 0x41, 0x00, 0x80, 0xBE, 0xC2, // lat, lon (f32)
    0x64, 0x00, 0x99, 0x19, 0x00, 0x00, // area, energy, quality
];

#[test]
fn goes_glm_flash_unit_follows_the_guide() {
    assert_eq!(glm::flash_unit(&glm::TEST_FLASHES[..1]), FLASH_BYTES);
    let u = decode_flashes(&FLASH_BYTES, PRODUCT).unwrap();
    assert_eq!(u.rejected, 0);
    let f = &u.records[0];
    assert_eq!(f.id, 1);
    assert!(near(f.first, PRODUCT + 20_000.0 * 3.814756e-4 - 5.0, 1e-15));
    assert!(near(f.last, PRODUCT + 20_524.0 * 3.814756e-4 - 5.0, 1e-15));
    assert!(near(
        f.frame_first,
        PRODUCT + 19_990.0 * 3.814756e-4 - 5.0,
        1e-15
    ));
    assert!((f.last - f.first - 524.0 * 3.814756e-4).abs() < 1e-6);
    assert_eq!((f.lat, f.lon), (29.5, -95.25));
    assert!(near(f.area_km2.unwrap(), 15.2601862, 1e-12));
    assert!(near(f.energy_j.unwrap(), 6553.0 * 1.52597e-15, 1e-12));
    assert_eq!(f.quality, Some(GlmQuality::Good));
}

#[test]
fn goes_glm_group_and_event_units_follow_the_guide() {
    // Group 101 of flash 1 (Table 7.2.1.6.1.2, 28-byte stride): time offsets 20000 and 19990, latitude 29.5, longitude
    // -95.25, area fill, energy 3000, parent 1, quality 5, then 4 bytes the table does not describe.
    let g: [u8; 36] = [
        1, 0, 0, 0, 0, 0, 0, 0, //
        101, 0, 0, 0, 0x20, 0x4E, 0x16, 0x4E, //
        0x00, 0x00, 0xEC, 0x41, 0x00, 0x80, 0xBE, 0xC2, //
        0xFF, 0xFF, 0xB8, 0x0B, 0x01, 0x00, 0x05, 0x00, //
        0xAA, 0xAA, 0xAA, 0xAA,
    ];
    let u = decode_groups(&g, PRODUCT).unwrap();
    let r = &u.records[0];
    assert_eq!((r.id, r.parent, r.lat, r.lon), (101, 1, 29.5, -95.25));
    assert_eq!(r.area_km2, None);
    assert!(near(r.energy_j.unwrap(), 3000.0 * 1.52597e-15, 1e-12));
    assert_eq!(r.quality, Some(GlmQuality::Duration));
    assert!(near(r.time, PRODUCT + 2.629512, 1e-15));
    // Event 1001 of group 101 (Table 7.2.1.6.1.3): time 20000, latitude 47000, longitude 20000, energy 1500.
    let e: [u8; 24] = [
        1, 0, 0, 0, 0, 0, 0, 0, //
        0xE9, 0x03, 0x00, 0x00, 0x20, 0x4E, 0x98, 0xB7, //
        0x20, 0x4E, 0xDC, 0x05, 101, 0, 0, 0,
    ];
    let u = decode_events(&e, PRODUCT, None).unwrap();
    assert_eq!(
        (u.records[0].id, u.records[0].parent, u.records[0].lon),
        (1001, 101, None)
    );
    assert!(near(u.records[0].lat, 28.91016, 1e-12));
    let u = decode_events(&e, PRODUCT, Some(-141.56)).unwrap();
    assert!(near(u.records[0].lon.unwrap(), 40.6256 - 141.56, 1e-12));
    assert!(near(
        u.records[0].energy_j.unwrap(),
        1500.0 * 1.52597e-15,
        1e-12
    ));
    // A unit whose length does not match its count.
    assert_eq!(decode_groups(&g[..32], PRODUCT), Err(BadLength));
    assert_eq!(decode_events(&e[..20], PRODUCT, None), Err(BadLength));
}

fn decode(packets: &[Vec<u8>], d: &mut GrbDecoder) -> usize {
    let mut files = 0;
    for p in packets {
        d.push(5, p, 0.0, &mut |o| {
            if let GrbOut::File(_) = o {
                files += 1
            }
        });
    }
    files
}

#[test]
fn goes_glm_test_products_come_back_exactly() {
    let mut src = GlmTestSource::default();
    let mut d = GrbDecoder::default();
    let files = decode(&src.next_product(), &mut d) + decode(&src.next_product(), &mut d);
    assert_eq!(files, 2, "the two metadata payloads");
    assert_eq!(d.glm.event_lon_offset, Some(glm::TEST_EVENT_LON_OFFSET));
    let r = d.glm.report();
    let st = &r.stats;
    assert_eq!((st.flashes, st.groups, st.events), (6, 12, 24));
    assert_eq!((st.payloads, st.dropped, st.packets_lost), (8, 1, 1));
    assert_eq!(
        (
            st.rejected,
            st.bad_length,
            st.group_bad_length,
            st.not_decoded
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(d.stats.crc_bad, 0);
    assert_eq!(r.flashes.len(), 6);
    for (i, e) in r.flashes.iter().enumerate() {
        let raw: &RawFlash = &glm::TEST_FLASHES[i % 3];
        let pt = PRODUCT + 20.0 * (i / 3) as f64;
        let f = &e.flash;
        assert_eq!(e.product, 815_000_000_250_000 + 20_000_000 * (i / 3) as u64);
        assert_eq!(f.id, raw.id);
        assert!(near(
            f.first,
            pt + f64::from(raw.t_first) * 3.814756e-4 - 5.0,
            1e-15
        ));
        assert!(near(
            f.last,
            pt + f64::from(raw.t_last) * 3.814756e-4 - 5.0,
            1e-15
        ));
        assert_eq!((f.lat, f.lon), (f64::from(raw.lat), f64::from(raw.lon)));
        let links = [(2, 4), (1, 2), (3, 6)][i % 3];
        assert_eq!((e.groups, e.events), links, "flash {}", f.id);
    }
    assert_eq!(r.flashes[2].flash.area_km2, None);
    assert_eq!(r.flashes[2].flash.quality, Some(GlmQuality::EventCount));
    assert_eq!(r.flashes[1].flash.quality, Some(GlmQuality::TimeOrder));
    assert_eq!(r.groups.len(), 12);
    assert!(r.groups.iter().all(|g| g.events == 2));
    assert_eq!(r.groups[3].group.parent, 3);
    let newest = PRODUCT + 20.0 + 40_000.0 * 3.814756e-4 - 5.0;
    assert!(near(r.newest_flash.unwrap(), newest, 1e-15));
    assert_eq!(r.rate_per_minute, 6.0);
    // Four more products: the links of the first products are let go, and their rows keep their counts.
    for _ in 0..4 {
        decode(&src.next_product(), &mut d);
    }
    let r = d.glm.report();
    assert_eq!(r.flashes.len(), 18);
    for (i, e) in r.flashes.iter().enumerate() {
        assert_eq!(
            (e.groups, e.events),
            [(2, 4), (1, 2), (3, 6)][i % 3],
            "row {i}"
        );
    }
    assert!(r.groups.iter().all(|g| g.events == 2));
}

/// The position in `p` of the first packet of `apid`.
fn first_of(p: &[Vec<u8>], apid: u16) -> usize {
    p.iter()
        .position(|q| u16::from_be_bytes([q[0], q[1]]) & 0x7FF == apid)
        .unwrap()
}

#[test]
fn goes_glm_a_missing_packet_drops_its_payload() {
    // The middle packet of the three event packets is lost: the last one shows the gap and the events are dropped.
    let mut p = GlmTestSource::default().next_product();
    let at = first_of(&p, glm::APID_EVENTS);
    p.remove(at + 1);
    let mut d = GrbDecoder::default();
    decode(&p, &mut d);
    let st = &d.glm.stats;
    assert_eq!((st.flashes, st.groups, st.events), (3, 6, 0));
    assert_eq!((st.dropped, st.packets_lost), (1, 1));
    // The last of the two group packets is lost: the first group packet of the next product shows the gap and drops them.
    let mut src = GlmTestSource::default();
    let mut first = src.next_product();
    let at = first_of(&first, APID_GROUPS);
    first.remove(at + 1);
    let mut d = GrbDecoder::default();
    decode(&first, &mut d);
    assert_eq!(d.glm.stats.groups, 0);
    decode(&src.next_product(), &mut d);
    let st = &d.glm.stats;
    assert_eq!(st.groups, 6, "only the groups of the second product");
    assert_eq!((st.dropped, st.packets_lost), (2, 2));
    let r = d.glm.report();
    assert_eq!((r.flashes[0].groups, r.flashes[3].groups), (0, 2));
}

#[test]
fn goes_glm_impossible_values_are_rejected_and_counted() {
    let good = glm::TEST_FLASHES[0];
    let bad = [
        RawFlash { lat: 95.0, ..good },
        RawFlash { lat: 67.0, ..good },
        RawFlash {
            lon: f32::NAN,
            ..good
        },
        RawFlash {
            lon: -210.0,
            ..good
        },
        RawFlash { quality: 7, ..good },
        RawFlash {
            area: 65_533,
            ..good
        },
        RawFlash {
            energy: 65_534,
            ..good
        },
        RawFlash {
            t_first: 30_000,
            ..good
        },
    ];
    let edge = RawFlash {
        lon: -200.0,
        area: glm::FILL,
        energy: glm::FILL,
        quality: glm::FILL,
        ..good
    };
    let mut flashes = bad.to_vec();
    flashes.push(edge);
    let u = decode_flashes(&glm::flash_unit(&flashes), PRODUCT).unwrap();
    assert_eq!(u.rejected, 8);
    assert_eq!(u.records.len(), 1);
    let f = &u.records[0];
    assert_eq!(
        (f.lon, f.area_km2, f.energy_j, f.quality),
        (160.0, None, None, None)
    );
    // Through the decoder: the rejects are counted and a group unit of 24-byte records counts as a wrong length.
    let mut d = GrbDecoder::default();
    let mut seq = 0;
    let pl = glm::generic(0, 815_000_000, 0, 0, &glm::flash_unit(&bad));
    decode(&[packet(APID_FLASHES, &pl, &mut seq)], &mut d);
    let mut g = glm::group_unit(&glm::TEST_GROUPS[..2]);
    g.truncate(8 + 48);
    let mut seq = 0;
    decode(
        &[packet(
            APID_GROUPS,
            &glm::generic(0, 815_000_000, 0, 0, &g),
            &mut seq,
        )],
        &mut d,
    );
    let st = &d.glm.stats;
    assert_eq!(
        (st.rejected, st.flashes, st.group_bad_length, st.groups),
        (8, 0, 1, 0)
    );
}

fn packet(apid: u16, payload: &[u8], seq: &mut u16) -> Vec<u8> {
    let p = sdrkit_stack_satimg::modes::s2::crc_packet(apid, 3, *seq, &[0; 8], payload, true);
    *seq += 1;
    p
}

#[test]
fn goes_glm_a_compressed_payload_is_saved_not_decoded() {
    let mut d = GrbDecoder::default();
    let mut seq = 0;
    let pl = glm::generic(2, 815_000_000, 0, 0, &[0, 1, 0, 0, 9, 9, 9]);
    let mut names = Vec::new();
    d.push(5, &packet(APID_FLASHES, &pl, &mut seq), 0.0, &mut |o| {
        if let GrbOut::File(f) = o {
            names.push(f.name)
        }
    });
    assert_eq!(names, ["302_815000000.bin"]);
    assert_eq!((d.glm.stats.not_decoded, d.glm.stats.flashes), (1, 0));
}

#[test]
fn goes_glm_test_signal_gives_flash_rows() {
    let h = impaired(
        Harness::new(&DESCRIPTOR).rate(20e6).seconds(1.0),
        Some(14.0),
        10e3,
        0.0,
    );
    let x = h.generate().expect("a signal");
    let r = run::<Strip>(&h, &x, 20e6, 1686.6e6, 1686.6e6, 65_536, None);
    assert!(r.num("flash_count") >= 3.0, "{}", r.num("flash_count"));
    assert!(r.num("event_count") >= 12.0, "{}", r.num("event_count"));
    assert_eq!(r.num("glm_rejected"), 0.0);
    assert!(r.num("glm_dropped") >= 1.0, "{}", r.num("glm_dropped"));
    assert!(r.num("flash_rate") >= 3.0);
    let keys: Vec<String> = table_rows(&r.results)
        .into_iter()
        .filter(|(t, k, _)| t == "flashes" && k.ends_with("-1"))
        .map(|(_, k, _)| k)
        .collect();
    // Flash 1 has two groups in every product but the newest, whose groups may not have come when the signal ends.
    let newest = keys.iter().max().cloned().unwrap_or_default();
    for k in keys.iter().filter(|k| **k != newest) {
        assert_eq!(
            cell_i64(&r.results, "flashes", k, &GlmFlashRow::GROUPS),
            Some(2),
            "{k}"
        );
        assert_eq!(
            cell_i64(&r.results, "flashes", k, &GlmFlashRow::EVENTS),
            Some(4),
            "{k}"
        );
    }
    assert!(keys.len() >= 3, "{keys:?}");
}
