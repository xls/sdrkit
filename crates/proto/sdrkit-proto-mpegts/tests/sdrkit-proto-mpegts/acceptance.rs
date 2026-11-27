//! Sections and streams built by hand and fed as packets.

use sdrkit_proto_mpegts::packet::{packetize_section, write_header, Header, TS_SIZE};
use sdrkit_proto_mpegts::pes::{build_pes, WRAP_33};
use sdrkit_proto_mpegts::tables::{
    self, build_long, build_pat, build_pmt, build_sdt, descriptor, loop_bytes,
};
use sdrkit_proto_mpegts::time::{to_bcd, unix_to_mjd_bcd};
use sdrkit_proto_mpegts::{Demultiplexer, DemuxOutput, SiDb};
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;

fn short_event(lang: &[u8; 3], name: &str, text: &str) -> Vec<u8> {
    let mut d = lang.to_vec();
    d.push(name.len() as u8);
    d.extend_from_slice(name.as_bytes());
    d.push(text.len() as u8);
    d.extend_from_slice(text.as_bytes());
    descriptor(0x4D, &d)
}

fn extended(lang: &[u8; 3], num: u8, last: u8, text: &str) -> Vec<u8> {
    let mut d = vec![num << 4 | last];
    d.extend_from_slice(lang);
    d.push(0);
    d.push(text.len() as u8);
    d.extend_from_slice(text.as_bytes());
    descriptor(0x4E, &d)
}

fn event(id: u16, start: i64, dur_s: i64, running: u8, free_ca: bool, desc: &[u8]) -> Vec<u8> {
    let mut e = id.to_be_bytes().to_vec();
    e.extend_from_slice(&unix_to_mjd_bcd(start));
    e.extend_from_slice(&[
        to_bcd((dur_s / 3600) as u32),
        to_bcd((dur_s / 60 % 60) as u32),
        to_bcd((dur_s % 60) as u32),
    ]);
    let l = desc.len();
    e.push(running << 5 | (free_ca as u8) << 4 | ((l >> 8) as u8 & 0x0F));
    e.push(l as u8);
    e.extend_from_slice(desc);
    e
}

fn eit(table: u8, sid: u16, tsid: u16, onid: u16, section: u8, events: &[Vec<u8>]) -> Vec<u8> {
    let mut b = tsid.to_be_bytes().to_vec();
    b.extend_from_slice(&onid.to_be_bytes());
    b.push(section);
    b.push(table);
    for e in events {
        b.extend_from_slice(e);
    }
    build_long(table, sid, 0, section, 1, &b)
}

const T0: i64 = 1_768_467_600; // 2026-01-15 09:00 UTC

#[test]
fn eit_with_two_events() {
    let mut db = SiDb::new();
    db.push_section(0, &build_pat(7, 0, &[(100, 0x100)]), 0.0);
    db.push_section(
        0x11,
        &build_sdt(7, 0x2310, 0, 0, 0, &[(100, 1, "P", "News One", false)]),
        0.0,
    );
    let mut d1 = short_event(b"eng", "Morning", "Text one");
    d1.extend(extended(b"eng", 0, 0, "Long description"));
    d1.extend(descriptor(0x54, &[0x20, 0x00]));
    let d2 = short_event(b"eng", "Later", "Text two");
    db.push_section(
        0x12,
        &eit(
            0x4E,
            100,
            7,
            0x2310,
            0,
            &[event(1, T0, 3600, 4, false, &d1)],
        ),
        0.0,
    );
    db.push_section(
        0x12,
        &eit(
            0x4E,
            100,
            7,
            0x2310,
            1,
            &[event(2, T0 + 3600, 1800, 1, false, &d2)],
        ),
        0.0,
    );
    let ev = db.epg.events((0x2310, 7, 100));
    assert_eq!(ev.len(), 2);
    assert_eq!(
        (
            ev[0].title.as_str(),
            ev[0].text.as_str(),
            ev[0].extended.as_str(),
            ev[0].genre,
            ev[0].running,
            ev[0].lang.as_str()
        ),
        ("Morning", "Text one", "Long description", 2, 4, "eng")
    );
    assert!(ev[0].start < ev[1].start);
    let s = db.service(100).unwrap();
    assert_eq!(s.now.as_ref().unwrap().title, "Morning");
    assert_eq!(s.next.as_ref().unwrap().title, "Later");
}

#[test]
fn a_full_event() {
    let mut db = SiDb::new();
    let mut d = short_event(b"eng", "Film", "");
    // Ratings: 13 in one country, 10 in another -> minimum age 16.
    d.extend(descriptor(
        0x55,
        &[b'G', b'B', b'R', 13, b'F', b'R', b'A', 10],
    ));
    let mut crid = vec![(1 << 2), 6];
    crid.extend_from_slice(b"/prog1");
    crid.push(2 << 2);
    crid.push(4);
    crid.extend_from_slice(b"/ser");
    d.extend(descriptor(0x76, &crid));
    d.extend(descriptor(0x54, &[0x20, 0x00, 0x43, 0x00]));
    db.push_section(
        0x12,
        &eit(0x4F, 5, 9, 0x22, 0, &[event(1, T0, 3600, 1, true, &d)]),
        0.0,
    );
    let ev = db.epg.events((0x22, 9, 5));
    let e = ev[0];
    assert_eq!(e.min_age, 16);
    assert_eq!(
        (e.crid_programme.as_str(), e.crid_series.as_str()),
        ("/prog1", "/ser")
    );
    assert_eq!(e.content, vec![0x20, 0x43]);
    assert!(e.other_ts && e.free_ca);
}

#[test]
fn bad_sections_are_counted_and_a_damaged_loop_keeps_what_came_before() {
    let mut db = SiDb::new();
    let good = eit(
        0x4E,
        1,
        1,
        1,
        0,
        &[event(1, T0, 600, 4, false, &short_event(b"eng", "A", ""))],
    );
    let mut bad = good.clone();
    bad[12] ^= 1;
    db.push_section(0x12, &bad, 0.0);
    db.push_section(0x12, &[0x4E, 0xF0, 0x03, 0, 0, 0], 0.0);
    let mut trunc = good.clone();
    trunc.truncate(good.len() - 6);
    db.push_section(0x12, &trunc, 0.0);
    // A PAT on the EIT PID: wrong table.
    db.push_section(0x12, &build_pat(1, 0, &[]), 0.0);
    let c = db.counts["EIT"];
    assert_eq!((c.crc, c.length + c.other), (1, 2));
    assert_eq!(db.counts["PAT"].other, 1);
    // A damaged event loop: the second event claims more descriptors than the section holds.
    let e1 = event(1, T0, 600, 4, false, &short_event(b"eng", "First", ""));
    let mut e2 = event(
        2,
        T0 + 600,
        600,
        1,
        false,
        &short_event(b"eng", "Second", ""),
    );
    e2[11] = 0xFF;
    let mut b = 1u16.to_be_bytes().to_vec();
    b.extend_from_slice(&1u16.to_be_bytes());
    b.push(0);
    b.push(0x4E);
    b.extend(e1);
    b.extend(e2);
    let s = build_long(0x50, 1, 0, 0, 0, &b);
    db.push_section(0x12, &s, 0.0);
    let ev = db.epg.events((1, 1, 1));
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].title, "First");
}

fn tot(utc: i64, entries: &[(&[u8; 3], i32, i64, i32)]) -> Vec<u8> {
    let mut d = Vec::new();
    for &(c, off, change, next) in entries {
        d.extend_from_slice(c);
        let pol = (off < 0) as u8;
        d.push(pol);
        let a = off.unsigned_abs();
        d.push(to_bcd(a / 60));
        d.push(to_bcd(a % 60));
        d.extend_from_slice(&unix_to_mjd_bcd(change));
        let n = next.unsigned_abs();
        d.push(to_bcd(n / 60));
        d.push(to_bcd(n % 60));
    }
    let desc = descriptor(0x58, &d);
    let len = 5 + 2 + desc.len() + 4;
    let mut s = vec![0x73, 0x70 | ((len >> 8) as u8 & 0x0F), len as u8];
    s.extend_from_slice(&unix_to_mjd_bcd(utc));
    s.extend(loop_bytes(&desc));
    append_crc(&mut s);
    s
}

#[test]
fn local_time_offsets() {
    // Lithuania: +3 h until the change, then +2 h (end of summer time, 2026-10-25 01:00 UTC).
    let change = 1_792_890_000;
    let mut db = SiDb::new();
    db.country = "LTU".into();
    db.push_section(
        0x14,
        &tot(
            change - 3600,
            &[
                (b"LTU", 180, change, 120),
                (b"ARG", -180, i64::MAX / 4, -180),
            ],
        ),
        0.0,
    );
    assert_eq!(db.local_offset_min(0.0), Some(180));
    db.push_section(0x14, &tot(change + 60, &[(b"LTU", 180, change, 120)]), 1.0);
    assert_eq!(db.local_offset_min(1.0), Some(120));
    db.country = "ARG".into();
    db.push_section(
        0x14,
        &tot(change, &[(b"ARG", -180, change + 999_999, -180)]),
        2.0,
    );
    assert_eq!(db.local_offset_min(2.0), Some(-180));
    // TDT.
    let mut tdt = vec![0x70, 0x70, 0x05];
    tdt.extend_from_slice(&unix_to_mjd_bcd(T0));
    db.push_section(0x14, &tdt, 10.0);
    assert_eq!(db.utc_now(12.5), Some(T0 + 2));
}

fn ts_packets(section: &[u8], pid: u16, cc: &mut u8) -> Vec<[u8; TS_SIZE]> {
    packetize_section(section, pid, cc)
}

#[test]
fn collector_over_packets_and_the_stream_clock() {
    let mut d = Demultiplexer::new();
    let mut out = DemuxOutput::default();
    let evs: Vec<Vec<u8>> = (0..12)
        .map(|i| {
            event(
                i,
                T0 + i as i64 * 600,
                600,
                1,
                false,
                &short_event(
                    b"eng",
                    &format!("Event number {i} with a long title"),
                    "and some text",
                ),
            )
        })
        .collect();
    let s = eit(0x50, 1, 1, 1, 0, &evs);
    let mut cc = 0;
    let pk = ts_packets(&s, 0x12, &mut cc);
    assert!(pk.len() >= 3);
    for p in &pk {
        d.push(p, Some(0.0), false, &mut out);
    }
    assert_eq!(d.si.epg.events((1, 1, 1)).len(), 12);
    // Packet loss in the middle: the section is dropped.
    let s2 = eit(0x50, 2, 1, 1, 0, &evs);
    let pk = ts_packets(&s2, 0x12, &mut cc);
    d.push(&pk[0], Some(0.0), false, &mut out);
    d.push(&pk[2], Some(0.0), false, &mut out);
    assert!(d.si.epg.events((1, 1, 2)).is_empty());
    // A bad CRC is counted.
    let mut s3 = eit(0x50, 3, 1, 1, 0, &evs[..1]);
    let n = s3.len();
    s3[n - 1] ^= 1;
    for p in ts_packets(&s3, 0x12, &mut cc) {
        d.push(&p, Some(0.0), false, &mut out);
    }
    assert_eq!(d.si.counts["EIT"].crc, 1);
    // The stream clock advances UTC.
    let mut tdt = vec![0x70, 0x70, 0x05];
    tdt.extend_from_slice(&unix_to_mjd_bcd(T0));
    let mut c4 = 0;
    for p in ts_packets(&tdt, 0x14, &mut c4) {
        d.push(&p, Some(5.0), false, &mut out);
    }
    let null = {
        let mut p = [0xFFu8; TS_SIZE];
        write_header(
            &mut p,
            &Header {
                pid: 0x1FFF,
                afc: 1,
                ..Default::default()
            },
        );
        p
    };
    d.push(&null, Some(8.0), false, &mut out);
    assert_eq!(d.si.utc_now(8.0), Some(T0 + 3));
}

#[test]
fn fuzzed_sections_never_crash_and_caps_hold() {
    let mut rng = sdrkit_test_rng(1);
    let mut db = SiDb::new();
    let seeds: Vec<Vec<u8>> = vec![
        build_pat(1, 0, &[(1, 0x100), (2, 0x200)]),
        build_pmt(
            1,
            0,
            0x101,
            &descriptor(0x09, &[0, 1, 0xE0, 0x10]),
            &[
                (0x1B, 0x101, descriptor(0x0A, b"eng\0")),
                (0x06, 0x102, descriptor(0x59, b"eng\x10\0\x01\0\x01")),
            ],
        ),
        build_sdt(1, 1, 0, 0, 0, &[(1, 1, "P", "N", false)]),
        eit(
            0x4E,
            1,
            1,
            1,
            0,
            &[event(1, T0, 600, 4, false, &short_event(b"eng", "A", "B"))],
        ),
        tot(T0, &[(b"AUS", 600, T0, 600)]),
    ];
    let pids = [0u16, 0x100, 0x11, 0x12, 0x14];
    for i in 0..200_000u32 {
        let k = (i as usize) % seeds.len();
        let mut s = if i % 3 == 0 {
            (0..(rng() % 300) as usize)
                .map(|_| rng() as u8)
                .collect::<Vec<u8>>()
        } else {
            let mut s = seeds[k].clone();
            for _ in 0..1 + rng() % 4 {
                let at = (rng() as usize) % s.len();
                s[at] = rng() as u8;
            }
            s
        };
        if i % 5 == 0 {
            // A valid CRC on the mutated section, so the parsers see the damage.
            let n = s.len();
            if n > 8 {
                s.truncate(n - 4);
                append_crc(&mut s);
                let total = (s.len() - 3) as u16;
                s[1] = (s[1] & 0xF0) | ((total >> 8) as u8 & 0x0F);
                s[2] = total as u8;
                s.truncate(3 + total as usize - 4);
                append_crc(&mut s);
            }
        }
        db.push_section(pids[k], &s, i as f64 * 0.001);
    }
    assert!(db.services().len() <= 65536);
    assert!(db.cat.len() <= 256);
    assert!(db.other_multiplexes.len() <= 4096);
    assert!(db.epg.len() <= 400 * 3000);
    let _ = tables::check(&[]);
}

fn sdrkit_test_rng(seed: u64) -> impl FnMut() -> u32 {
    let mut s = seed.wrapping_mul(0x9E3779B97F4A7C15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 32) as u32
    }
}

fn pes_packets(pid: u16, pes: &[u8], cc: &mut u8) -> Vec<[u8; TS_SIZE]> {
    let mut out = Vec::new();
    for (k, ch) in pes.chunks(184).enumerate() {
        let mut p = [0xFFu8; TS_SIZE];
        if ch.len() == 184 {
            write_header(
                &mut p,
                &Header {
                    pusi: k == 0,
                    pid,
                    afc: 1,
                    cc: *cc,
                    ..Default::default()
                },
            );
            p[4..].copy_from_slice(ch);
        } else {
            // Stuffing in the adaptation field to fill the packet.
            write_header(
                &mut p,
                &Header {
                    pusi: k == 0,
                    pid,
                    afc: 3,
                    cc: *cc,
                    ..Default::default()
                },
            );
            let af = 184 - ch.len() - 1;
            p[4] = af as u8;
            if af > 0 {
                p[5] = 0;
            }
            p[5 + af..].copy_from_slice(ch);
        }
        *cc = (*cc + 1) & 15;
        out.push(p);
    }
    out
}

fn service_with(streams: &[(u8, u16, Vec<u8>)]) -> (Demultiplexer, DemuxOutput) {
    let mut d = Demultiplexer::new();
    d.auto_select = true;
    d.probe_all = true;
    let mut out = DemuxOutput::default();
    let (mut c0, mut c1) = (0u8, 0u8);
    for p in packetize_section(&build_pat(1, 0, &[(1, 0x100)]), 0, &mut c0) {
        d.push(&p, Some(0.0), false, &mut out);
    }
    for p in packetize_section(&build_pmt(1, 0, streams[0].1, &[], streams), 0x100, &mut c1) {
        d.push(&p, Some(0.0), false, &mut out);
    }
    for p in packetize_section(
        &build_sdt(1, 1, 0, 0, 0, &[(1, 1, "P", "TV", false)]),
        0x11,
        &mut c0,
    ) {
        d.push(&p, Some(0.0), false, &mut out);
    }
    (d, out)
}

#[test]
fn pes_across_packets_and_a_pts_wrap() {
    let (mut d, mut out) = service_with(&[(0x02, 0x101, vec![])]);
    let mut cc = 0;
    let mut pts_out = Vec::new();
    for (i, pts) in [WRAP_33 - 9000, 9000, 27000].into_iter().enumerate() {
        let mut es = vec![
            0, 0, 1, 0xB3, 0x2D, 0x02, 0x40, 0x33, 0xFF, 0xFF, 0xE0, 0x18,
        ];
        es.extend_from_slice(&[0, 0, 1, 0x00, 0x00, 0x0F, 0xFF, 0xF8]);
        es.extend(vec![i as u8; 900]);
        let pes = build_pes(0xE0, Some(pts), Some(pts - 3600), &es, false);
        for p in pes_packets(0x101, &pes, &mut cc) {
            d.push(&p, Some(0.0), false, &mut out);
        }
        pts_out.extend(
            out.media
                .drain(..)
                .map(|m| (m.packet.pts.unwrap(), m.packet.dts.unwrap())),
        );
    }
    d.flush(&mut out);
    pts_out.extend(
        out.media
            .drain(..)
            .map(|m| (m.packet.pts.unwrap(), m.packet.dts.unwrap())),
    );
    assert_eq!(pts_out.len(), 3);
    assert_eq!(pts_out[1].0 - pts_out[0].0, 18_000);
    assert_eq!(pts_out[2].0 - pts_out[1].0, 18_000);
    assert_eq!(pts_out[1].0 - pts_out[1].1, 3600);
}

#[test]
fn ac3_and_eac3_signalling() {
    let ac3_frame = [0x0B, 0x77, 0x00, 0x00, 0x14, 0x40, 0x40, 0x00];
    let cases: Vec<(u8, Vec<u8>, &str)> = vec![
        (0x06, descriptor(0x6A, &[0x00]), "ac-3"),
        (0x81, vec![], "ac-3"),
        (0x06, descriptor(0x05, b"AC-3"), "ac-3"),
        (0x06, descriptor(0x7A, &[0x00]), "ec-3"),
        (0x87, vec![], "ec-3"),
    ];
    for (st, desc, fourcc) in cases {
        let (d, _) = service_with(&[(0x02, 0x101, vec![]), (st, 0x102, desc)]);
        let s = d.si.service(1).unwrap();
        assert_eq!(
            s.streams[1]
                .fourcc()
                .map(|f| f.as_str().to_owned())
                .as_deref(),
            Some(fourcc),
            "stream_type 0x{st:02X}"
        );
        let _ = ac3_frame;
    }
}

/// Appends the MPEG-2 CRC-32 of a section to it (ISO/IEC 13818-1 Annex A).
fn append_crc(s: &mut Vec<u8>) {
    let crc = CRC32_MPEG2.checksum(s);
    s.extend_from_slice(&crc.to_be_bytes());
}

/// A multiplex of two TV services, 1 first in list order and 2 after it: the PAT and both PMTs, then, if `sdt_free_ca`, an
/// SDT that sets free_CA_mode on service 1 only. The demultiplexer chooses as the transport stream layer of a stack does.
fn two_services(sdt_free_ca: bool) -> (Demultiplexer, DemuxOutput) {
    let mut d = Demultiplexer::new();
    d.auto_select = true;
    d.probe_all = true;
    let mut out = DemuxOutput::default();
    let mut cc = [0u8; 4];
    let mut sections = vec![
        (build_pat(1, 0, &[(1, 0x100), (2, 0x200)]), 0),
        (
            build_pmt(1, 0, 0x101, &[], &[(0x02, 0x101, vec![]), (0x03, 0x102, vec![])]),
            0x100,
        ),
        (
            build_pmt(2, 0, 0x201, &[], &[(0x02, 0x201, vec![]), (0x03, 0x202, vec![])]),
            0x200,
        ),
    ];
    if sdt_free_ca {
        let sdt = build_sdt(
            1,
            1,
            0,
            0,
            0,
            &[(1, 1, "P", "Pay", true), (2, 1, "P", "Free", false)],
        );
        sections.push((sdt, 0x11));
    }
    for (k, (s, pid)) in sections.iter().enumerate() {
        for p in packetize_section(s, *pid, &mut cc[k.min(3)]) {
            d.push(&p, Some(0.0), false, &mut out);
            out.clear();
        }
    }
    (d, out)
}

/// Three seconds of payload packets: those of service 2 clear, and, if `scrambled_one`, those of service 1 with the
/// scrambling bits set.
fn play(d: &mut Demultiplexer, out: &mut DemuxOutput, scrambled_one: bool) {
    let mut cc = [0u8; 4];
    let mut t = 0.0;
    for _ in 0..300 {
        for (k, pid) in [0x101u16, 0x102, 0x201, 0x202].into_iter().enumerate() {
            let scrambled = pid < 0x200;
            if scrambled && !scrambled_one {
                continue;
            }
            let mut p = [0x5Au8; TS_SIZE];
            write_header(
                &mut p,
                &Header {
                    pid,
                    afc: 1,
                    cc: cc[k],
                    scrambling: if scrambled { 2 } else { 0 },
                    ..Default::default()
                },
            );
            cc[k] = (cc[k] + 1) & 15;
            t += 0.0025;
            d.push(&p, Some(t), false, out);
            out.clear();
        }
    }
}

/// The player plays the first service that is not scrambled: a service whose packets carry scrambling bits, or whose SDT
/// entry sets free_CA_mode while none of its streams has been seen clear, is passed over and listed as scrambled. A service
/// the user chose stays chosen.
#[test]
fn the_player_passes_over_a_scrambled_service() {
    use sdrkit_proto_mpegts::si::CaState;
    // By the scrambling bits of its packets.
    let (mut d, mut out) = two_services(false);
    assert_eq!(d.selection.service, Some(1), "before its packets, service 1 looks clear");
    play(&mut d, &mut out, true);
    assert_eq!(d.selection.service, Some(2));
    assert_eq!(d.si.service(1).unwrap().ca_state, CaState::Scrambled);
    assert_eq!(d.si.service(2).unwrap().ca_state, CaState::Clear);
    // By free_CA_mode of the SDT, before any packet of service 1.
    let (mut d, mut out) = two_services(true);
    assert_eq!(d.selection.service, Some(2));
    assert_eq!(d.si.service(1).unwrap().ca_state, CaState::Scrambled);
    play(&mut d, &mut out, false);
    assert_eq!(d.selection.service, Some(2));
    // The choice of the user is kept.
    let (mut d, mut out) = two_services(false);
    d.auto_select = false;
    d.select(Some(1));
    play(&mut d, &mut out, true);
    assert_eq!(d.selection.service, Some(1));
}
