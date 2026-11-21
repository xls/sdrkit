//! FIC, labels and the ensemble, PAD and slideshow, announcements, data services (packet mode with FEC, SPI, Journaline,
//! DMB), transmitter identification, settings and retunes.
#![allow(clippy::needless_range_loop)]

use super::common::*;
use sdrkit_core::publish::keys;
use sdrkit_core::{InputInfo, Output, Owner, Receiver, StreamKind};
use sdrkit_stack_dab::data::{mot, pad};
use sdrkit_stack_dab::ensemble::{Changes, Ensemble};
use sdrkit_stack_dab::fig::{self, FigLists, FigRecord};
use sdrkit_stack_dab::generator::content::Programme;
use sdrkit_stack_dab::generator::data::{mot_group, mot_header};
use sdrkit_stack_dab::generator::services::PadContent;
use sdrkit_stack_dab::generator::{
    data as gd, fic as gf, AnnouncementSpec, GenParams, ServiceAudio, ServiceSpec,
};
use sdrkit_stack_dab::results::*;
use sdrkit_stack_dab::tables::Protection;
use sdrkit_stack_dab::typed::*;

const RATE: f64 = 2.048e6;

fn apply(db: &mut Ensemble, f: &[u8]) {
    let rec = FigRecord::new(f[0] >> 5, &f[1..]);
    db.apply(&rec, &mut FigLists::default(), 0, &mut Changes::default());
}

#[test]
fn single_figs() {
    let mut db = Ensemble::default();
    // 0/0: EId 0xCE15, country 12, CIF count 5/0.
    let f00 = [0x05, 0x00, 0xCE, 0x15, 0x05, 0x00];
    let rec = FigRecord::new(0, &f00[1..]);
    let mut lists = FigLists::default();
    match fig::decode(&rec, &mut lists).unwrap() {
        fig::Fig::Ensemble { eid, cif_count, .. } => {
            assert_eq!(eid, 0xCE15);
            assert_eq!(eid >> 12, 12);
            assert_eq!(cif_count, 5 * 250);
        }
        f => panic!("{f:?}"),
    }
    apply(&mut db, &f00);
    apply(
        &mut db,
        &gf::fig0_1(&[(
            3,
            100,
            Protection::Eep {
                option: 0,
                level: 3,
            },
            36,
        )]),
    );
    let s = db.subchannels[&3];
    assert_eq!(
        (
            s.start_cu,
            s.size_cu,
            s.protection_text().as_str(),
            s.bitrate
        ),
        (100, 36, "EEP 3-A", 48)
    );
    apply(
        &mut db,
        &gf::fig0_2(
            &[gf::Fig02Service {
                sid: 0xCE01,
                caid: 0,
                components: vec![(0, 63, 3, true, false)],
            }],
            false,
        ),
    );
    let svc = &db.services[&0xCE01];
    assert_eq!(
        (
            svc.components[0].ty,
            svc.components[0].subch,
            svc.components[0].primary
        ),
        (63, Some(3), true)
    );
    apply(&mut db, &gd::fig0_9(2, 0xE1, 1));
    assert_eq!((db.lto, db.ecc), (2, 0xE1));
    // 0/10: Saturday 25 February 2023, 12:34:56.0 UTC = 1 677 328 496 s.
    apply(&mut db, &gf::fig0_10(1_677_328_496.0));
    assert_eq!(db.utc_seconds, 1_677_328_496);
    apply(&mut db, &gf::fig0_17(&[(0xCE01, false, Some(9), 11)]));
    let svc = &db.services[&0xCE01];
    assert_eq!((svc.pty, svc.language), (Some(11), Some(9)));
    assert_eq!(
        sdrkit_stack_dab::registry::programme_type(11, 1),
        "Rock Music"
    );
    apply(&mut db, &gd::fig0_21(0xCE15, 225_648, 0xD201, 88_500));
    let fl: Vec<_> = db.frequencies.values().collect();
    assert!(fl
        .iter()
        .any(|l| l.id == 0xCE15 && l.freqs_khz[0].0 == 225_648));
    assert!(fl.iter().any(|l| l.rm == 8 && l.freqs_khz[0].0 == 88_500));
    apply(
        &mut db,
        &gf::fig1(1, &0xCE01u16.to_be_bytes(), "Test Radio", 0xF000),
    );
    assert_eq!(db.services[&0xCE01].label, "Test Radio");
    assert_eq!(db.services[&0xCE01].short_label, "Test");
    // 1/0 of another EId does not name this ensemble.
    apply(
        &mut db,
        &gf::fig1(0, &0xCE01u16.to_be_bytes(), "Other", 0xFF00),
    );
    assert_eq!(db.label, "");
    for seg in gf::fig2_segments(1, &0xCE01u16.to_be_bytes(), "Hello World", false, 0xF800) {
        apply(&mut db, &seg);
    }
    assert_eq!(db.services[&0xCE01].label, "Hello World");
    assert_eq!(db.services[&0xCE01].short_label, "Hello");
    // A truncated 0/2 is an error.
    let cut = FigRecord::new(0, &[0x02, 0xCE, 0x01, 0x01, 0x00]);
    assert!(fig::decode(&cut, &mut FigLists::default()).is_err());
    // 0/22: main id 5 at 54.7, 25.3; sub id 3 with 40 µs placed relative to it; a sub id of an unheard main id is not placed.
    apply(
        &mut db,
        &gd::fig0_22(5, 54.7, 25.3, &[(3, 40, 0.01, -0.02)]),
    );
    let m = db.tii.iter().find(|t| t.main == 5 && t.sub == 0).unwrap();
    assert!((m.lat - 54.7).abs() < 0.01 && (m.lon - 25.3).abs() < 0.01);
    let s3 = db.tii.iter().find(|t| t.main == 5 && t.sub == 3).unwrap();
    assert_eq!(s3.delay_us, 40);
    assert!((s3.lat - 54.71).abs() < 0.01 && (s3.lon - 25.28).abs() < 0.01);
    let body = sdrkit_stack_dab::bits::written(|w| {
        w.put(0x16, 8);
        w.put(1, 1);
        w.put(9, 7);
        w.put(0, 5);
        w.put(1, 3);
        w.put(2, 5);
        w.put(10, 11);
        w.put(0, 32);
    });
    apply(&mut db, &gf::fig(0, &body));
    assert!(!db.tii.iter().any(|t| t.main == 9));
}

#[test]
fn generator_fic_and_records() {
    let x = default_signal(GenParams::default(), Air::default(), 4.0);
    let r = run(&x, RATE, -1, 65_536);
    assert!(r.count(&SyncFields::FRAMES) > 40);
    assert!(r.count(&SyncFields::FIB_OK) > 100);
    assert_eq!(r.count(&SyncFields::FIB_BAD), 0);
    let per: Vec<(String, i64)> = r
        .rows(&FIG_TYPES)
        .iter()
        .map(|(k, row)| (k.to_string(), row.at(&FigTypeRow::COUNT).unwrap_or(0)))
        .collect();
    let count = |n: &str| per.iter().find(|p| p.0 == n).map_or(0, |p| p.1);
    let frames = r.count(&SyncFields::FRAMES);
    assert!(
        count("0/0") + 2 >= frames,
        "0/0 {} of {frames}",
        count("0/0")
    );
    assert!(count("0/10") + 2 >= frames);
    assert!(count("1/1") > 0 && count("0/1") > 0 && count("0/2") > 0);
    // The analyser keeps one record each of 0/0 and 0/10.
    let recs = r.count(&DataFields::FIC_RECORDS);
    assert!((8..40).contains(&recs), "{recs} records");
    assert_eq!(r.rows(&SERVICE_COMPONENTS).len(), 4);
    assert_eq!(r.rows(&SUBCHANNELS).len(), 4);
    let mut r = r;
    r.rx.retuned(&InputInfo::new(RATE, 227.36e6, 227.36e6).recording());
    assert_eq!(r.count(&DataFields::FIC_RECORDS), 0);
    assert_eq!(r.rows(&SERVICES).len(), 0);
}

#[test]
fn programme_type() {
    let x = generate(
        GenParams {
            pty: Some((10, false)),
            ..Default::default()
        },
        3.0,
    );
    let r = run(&x, RATE, -1, 65_536);
    let rows = r.rows(&SERVICES);
    assert_eq!(rows.len(), 4);
    for (_, s) in rows {
        assert_eq!(s.at(&ServiceRow::PTY), Some(10));
        assert_eq!(s.at(&ServiceRow::LANGUAGE), Some(9));
        assert_eq!(s.at(&ServiceRow::PTY_STATIC), Some(true));
        assert_eq!(s.at(&ServiceRow::PTY_NAME), Some("Pop Music"));
    }
}

#[test]
fn announcements() {
    let ann = AnnouncementSpec {
        cluster: 3,
        support: 0x0003,
        active: 0x0002,
        subch: 2,
    };
    let p = GenParams {
        announcement: Some(ann.clone()),
        ..Default::default()
    };
    let mut x = generate(p.clone(), 3.0);
    let r = run_with(
        &x,
        RATE,
        &[("service", 1.0), ("announcement_types", 0x3F as f64)],
        65_536,
    );
    let supporting: Vec<_> = r
        .rows(&SERVICES)
        .into_iter()
        .filter(|(_, s)| s.at(&ServiceRow::ANNOUNCEMENT_SUPPORT).unwrap_or(0) != 0)
        .collect();
    assert_eq!(
        supporting[0].1.at(&ServiceRow::ANNOUNCEMENT_SUPPORT),
        Some(3)
    );
    let clusters = r.rows(&SERVICE_CLUSTERS);
    assert_eq!(clusters[0].1.at(&ServiceClusterRow::CLUSTER), Some(3));
    assert_eq!(
        clusters[0].1.at(&ServiceClusterRow::SERVICE),
        Some(supporting[0].0)
    );
    let act = r.rows(&ANNOUNCEMENTS);
    let a = act[0].1;
    assert_eq!(
        (
            a.at(&AnnouncementRow::CLUSTER),
            a.at(&AnnouncementRow::ASW),
            a.at(&AnnouncementRow::SUBCHANNEL)
        ),
        (Some(3), Some(2), Some(2))
    );
    assert_eq!(r.at(&AnnouncementFields::ANNOUNCEMENT_SUBCHANNEL), Some(2));
    assert_eq!(r.at(&AnnouncementFields::ANNOUNCEMENT_TYPE), Some(1));
    let r = run_with(
        &x,
        RATE,
        &[("service", 1.0), ("announcement_types", 4.0)],
        65_536,
    );
    assert_eq!(r.at(&AnnouncementFields::ANNOUNCEMENT_CLUSTER), None);
    let started: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.is(&ANNOUNCEMENT_STARTED))
        .collect();
    assert!(!started.is_empty());
    // Type 1 (road traffic) is not an alarm.
    assert_eq!(started[0].get(&keys::URGENCY), Some(keys::Urgency::Notice));
    // The FIG stops: the announcement ends after 40 frames.
    let q = GenParams {
        announcement: Some(AnnouncementSpec { active: 0, ..ann }),
        ..Default::default()
    };
    x.extend(generate(q, 6.0));
    let r = run_with(&x, RATE, &[("service", 1.0)], 65_536);
    assert!(r.rows(&ANNOUNCEMENTS).is_empty());
    assert!(r.events.iter().any(|e| e.is(&ANNOUNCEMENT_ENDED)));
}

#[test]
fn extended_labels() {
    let mut p = GenParams {
        fig2_labels: true,
        label: "Ensemble Ünïcødé".into(),
        ..Default::default()
    };
    p.services.truncate(2);
    p.services[0].label = "Café Ørsted".into();
    p.services[1].label = "Ελληνικά".into();
    let x = generate(p, 3.0);
    let r = run(&x, RATE, -1, 65_536);
    assert_eq!(
        r.at(&EnsembleFields::ENSEMBLE_LABEL),
        Some("Ensemble Ünïcødé")
    );
    let label = |k: &str| {
        r.rows(&SERVICES)
            .into_iter()
            .find(|(key, _)| *key == k)
            .and_then(|(_, s)| s.at(&ServiceRow::LABEL).map(str::to_string))
    };
    assert_eq!(label("C101").as_deref(), Some("Café Ørsted"));
    assert_eq!(label("C102").as_deref(), Some("Ελληνικά"));
}

#[test]
fn ensemble_change() {
    let a = GenParams {
        eid: 0xCE15,
        ..Default::default()
    };
    let mut b = GenParams {
        eid: 0xC222,
        label: "Other".into(),
        ..Default::default()
    };
    b.services.truncate(2);
    b.services[0].sid = 0xC301;
    b.services[1].sid = 0xC302;
    let mut x = generate(a, 2.0);
    x.extend(generate(b, 3.0));
    let r = run(&x, RATE, -1, 65_536);
    assert_eq!(r.at(&EnsembleFields::EID), Some(0xC222));
    let mut sids: Vec<&str> = r.rows(&SERVICES).into_iter().map(|(k, _)| k).collect();
    sids.sort_unstable();
    assert_eq!(sids, ["C301", "C302"]);
}

#[test]
fn slide_and_label_from_the_generator() {
    let x = default_signal(GenParams::default(), Air::cn(30.0), 5.0);
    let r = run(&x, RATE, 3, 65_536);
    let pics = r.units(StreamKind::Picture);
    assert_eq!(pics.len(), 1, "one picture, repetitions not counted");
    assert_eq!(
        pics[0].packet.bytes(),
        &sdrkit_stack_dab::generator::png::test_slide()[..]
    );
    assert_eq!(
        r.at(&sdrkit_proto_bcdata::publish::SlideFields::SLIDE_NAME),
        Some("Test slide")
    );
    assert_eq!(r.at(&PadFields::SLIDE_WIDTH), Some(80));
    assert_eq!(r.at(&PadFields::SLIDE_HEIGHT), Some(60));
    assert_eq!(
        r.at(&sdrkit_proto_bcdata::publish::LabelFields::TEXT),
        Some("Test Melody - test tune")
    );
    assert!(r.events.iter().any(|e| e.is(&SLIDE_SHOWN)));
    let r = run(&x, RATE, 1, 65_536);
    assert!(r.units(StreamKind::Picture).is_empty());
    assert_eq!(r.at(&PadFields::SLIDE_SEQUENCE), None);
}

#[test]
fn the_results_are_typed_and_a_service_is_selected_by_its_row() {
    let x = default_signal(GenParams::default(), Air::cn(30.0), 5.0);
    let mut r = run(&x, RATE, 3, 65_536);
    assert_eq!(
        r.at(&sdrkit_proto_bcdata::publish::LabelFields::TEXT),
        Some("Test Melody - test tune")
    );
    assert_eq!(r.at(&PadFields::LABEL_CHARSET), Some(0));
    assert_eq!(r.lock_state(), sdrkit_core::LockState::Locked);
    assert_eq!(r.at(&PadFields::SLIDE_WIDTH), Some(80));
    let services = r.rows(&SERVICES);
    assert_eq!(services.len(), 4);
    let (key, row) = services
        .iter()
        .find(|(_, s)| s.at(&ServiceRow::PLAYING) == Some(true))
        .unwrap();
    assert_eq!(*key, "C103");
    assert_eq!(row.at(&ServiceRow::SID), Some(0xC103));
    assert_eq!(row.at(&ServiceRow::SUBCHANNEL), Some(3));
    assert_eq!(
        row.at(&ServiceRow::NOW_PLAYING),
        Some("Test Melody - test tune")
    );
    assert_eq!(r.at(&PlayingFields::SELECTED_SERVICE), Some("C103"));
    assert_eq!(r.at(&PlayingFields::SELECTED_SUBCHANNEL), Some(3));
    assert_eq!(r.at(&EnsembleFields::ENSEMBLE_LABEL), Some("Test Ensemble"));
    // A row that the ensemble does not hold is refused with its key.
    let id = r.rx.commands().id("service.select").unwrap();
    let mut args = r.rx.commands().def(id).unwrap().new_args();
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, "C1FF");
    let e = sdrkit_core::owner::invoke(&mut r.rx, id, &args).unwrap_err();
    assert!(
        matches!(&e, sdrkit_core::CommandError::NoSuchRow { key, .. } if key == "C1FF"),
        "{e:?}"
    );
    select(&mut r.rx, "C101");
}

fn slide_groups(png: &[u8], bad_body: bool, extra: u32) -> Vec<Vec<u8>> {
    let header = mot_header(png.len() as u32 + extra, 2, 3, "slide.png");
    let gh = mot_group(3, 0x2222, &header);
    let mut gb = mot_group(4, 0x2222, png);
    if bad_body {
        gb[20] ^= 0x55;
    }
    vec![gh, gb]
}

/// Feeds data groups through the PAD decoder in the given X-PAD form and returns the MOT objects completed.
fn through_pad(groups: &[Vec<u8>], form: u8) -> Vec<mot::MotObject> {
    let mut d = pad::PadDecoder::new();
    let mut m = sdrkit_proto_bcdata::mot::Assembler::new();
    let mut objs = Vec::new();
    let mut frames: Vec<pad::PadItem> = Vec::new();
    for g in groups {
        match form {
            // Every piece with its own content indicator.
            0 => {
                frames.push(pad::PadItem::new(
                    pad::write::fpad(2, true),
                    &pad::write::variable_xpad(&[(1, pad::write::length_indicator(g.len()))]),
                    true,
                ));
                for (i, c) in g.chunks(16).enumerate() {
                    let t = if i == 0 { 12 } else { 13 };
                    frames.push(pad::PadItem::new(
                        pad::write::fpad(2, true),
                        &pad::write::variable_xpad(&[(t, c.to_vec())]),
                        true,
                    ));
                }
            }
            // An indicator on the first piece only.
            1 => {
                frames.push(pad::PadItem::new(
                    pad::write::fpad(2, true),
                    &pad::write::variable_xpad(&[(1, pad::write::length_indicator(g.len()))]),
                    true,
                ));
                for (i, c) in g.chunks(16).enumerate() {
                    if i == 0 {
                        frames.push(pad::PadItem::new(
                            pad::write::fpad(2, true),
                            &pad::write::variable_xpad(&[(12, c.to_vec())]),
                            true,
                        ));
                    } else {
                        let mut v = c.to_vec();
                        v.resize(16, 0);
                        frames.push(pad::PadItem::new(pad::write::fpad(2, false), &v, true));
                    }
                }
            }
            // Short X-PAD: 3 bytes after an indicator, then 4 without.
            _ => {
                let li = pad::write::length_indicator(g.len());
                frames.push(pad::PadItem::new(
                    pad::write::fpad(1, true),
                    &[vec![1], li[..3].to_vec()].concat(),
                    true,
                ));
                frames.push(pad::PadItem::new(
                    pad::write::fpad(1, false),
                    &[li[3..].to_vec(), vec![0, 0, 0]].concat(),
                    true,
                ));
                let mut first = true;
                let mut rest: &[u8] = g;
                while !rest.is_empty() {
                    if first {
                        let n = rest.len().min(3);
                        let mut v = vec![12];
                        v.extend_from_slice(&rest[..n]);
                        v.resize(4, 0);
                        frames.push(pad::PadItem::new(pad::write::fpad(1, true), &v, true));
                        rest = &rest[n..];
                        first = false;
                    } else {
                        let n = rest.len().min(4);
                        let mut v = rest[..n].to_vec();
                        v.resize(4, 0);
                        frames.push(pad::PadItem::new(pad::write::fpad(1, false), &v, true));
                        rest = &rest[n..];
                    }
                }
            }
        }
    }
    for f in frames {
        d.push(&f);
        for g in d.groups.drain(..) {
            if let Ok(p) = sdrkit_proto_bcdata::datagroup::parse(&g.data) {
                m.push(&p, &mut |o| objs.push(mot::MotObject::of(&o)));
            }
        }
    }
    objs
}

#[test]
fn pad_forms() {
    let png = sdrkit_stack_dab::generator::png::test_slide();
    // The short X-PAD form of the length indicator (1 byte of the 4 in the indicator's own 3-byte subfield) is not valid in
    // practice; for the short form send the indicator as two short X-PADs, which the decoder accepts as one subfield of type 1
    // continued only when it holds its 4 bytes: use the variable forms for the indicator and the short form for the data group.
    for form in [0u8, 1] {
        let o = through_pad(&slide_groups(&png, false, 0), form);
        assert_eq!(o.len(), 1, "form {form}");
        assert_eq!(&o[0].body[..], &png[..]);
    }
    // A body data group with a bad CRC produces no picture; the good one after it produces the picture.
    let mut g = slide_groups(&png, true, 0);
    g.extend(slide_groups(&png, false, 0).into_iter().skip(1));
    let o = through_pad(&g, 0);
    assert_eq!(o.len(), 1);
    // A header announcing one byte more than the body produces none.
    assert!(through_pad(&slide_groups(&png, false, 1), 0).is_empty());
    // Junk PAD is ignored without harm.
    let mut d = pad::PadDecoder::new();
    d.push(&pad::PadItem::new([0x20, 0x02], &[], true));
    d.push(&pad::PadItem::new([0x20, 0x02], &[0xFF; 3], true));
    d.push(&pad::PadItem::new([0x20, 0x00], &[], true));
    assert!(d.groups.is_empty());
    assert!(sdrkit_stack_dab::audio::dabplus::pad_of_au(&[0x80, 0xFF]).is_none());
}

fn data_ensemble() -> GenParams {
    let mut p = GenParams::default();
    p.services.truncate(2);
    // SPI and Journaline in one packet-mode sub-channel with the packet FEC.
    let mut groups: Vec<(u16, Vec<u8>)> = Vec::new();
    for g in gd::mot_directory(&gd::spi_objects(
        0xC0DE,
        &[(0xC101, "Test Tones 1"), (0xC102, "Test Tones 2")],
        1_700_000_000,
    )) {
        groups.push((100, g));
    }
    for g in gd::journaline_groups() {
        groups.push((101, g));
    }
    let frames = gd::packet_frames(32, &groups, true);
    p.services.push(ServiceSpec {
        sid: 0xE0C0_DE01,
        label: "Test Data".into(),
        subch: 7,
        bitrate: 32,
        protection: Protection::Eep {
            option: 0,
            level: 3,
        },
        audio: ServiceAudio::Data {
            tmid: 3,
            dscty: 60,
            scid: 0x101,
            frames,
        },
        programme: Programme::Silence,
        pad: PadContent::default(),
        ca: false,
        extra_components: Vec::new(),
    });
    p.extra_figs = vec![
        gd::fig0_3(0x101, true, 60, 7, 100),
        gd::fig0_14(7, 1),
        gd::fig0_8(0xE0C0_DE01, 0, None, Some(0x101)),
        gd::fig0_13(0xE0C0_DE01, 0, 0x007, &[]),
    ];
    p
}

/// A change of frequency by the host keeps the guide and its count, and forgets the ensemble and every measurement.
#[test]
fn a_change_of_frequency_keeps_the_guide_only() {
    use sdrkit_proto_bcdata::publish as bc;
    let x = generate_air(data_ensemble(), Air::cn(30.0), 6.0);
    let mut r = run_with(
        &x,
        RATE,
        &[("service", 1.0), ("data_subchannel", 7.0)],
        65_536,
    );
    let services = r.rows(&bc::GUIDE_SERVICE_TABLE).len();
    let programmes = r.rows(&bc::GUIDE_PROGRAMME_TABLE).len();
    let objects = r.at(&DataFields::SPI_OBJECTS);
    assert_eq!(services, 2);
    assert!(programmes > 0);
    assert!(objects.is_some_and(|n| n > 0));
    assert!(!r.rows(&SERVICES).is_empty());
    assert!(r.at(&EnsembleFields::ENSEMBLE_LABEL).is_some());
    assert!(r.at(&SignalFields::SNR_DB).is_some());
    assert!(r.at(&AudioFields::AUDIO_CODEC).is_some());
    // The session configures the receiver for the new frequency, then tells it to forget.
    r.rx.retuned(&InputInfo::new(RATE, 227.36e6, 227.36e6).recording());
    r.rx.forget();
    assert_eq!(r.rows(&bc::GUIDE_SERVICE_TABLE).len(), services);
    assert_eq!(r.rows(&bc::GUIDE_PROGRAMME_TABLE).len(), programmes);
    assert_eq!(r.at(&DataFields::SPI_OBJECTS), objects);
    assert!(r.rows(&SERVICES).is_empty());
    assert!(r.rows(&SUBCHANNELS).is_empty());
    assert!(r.rows(&JOURNALINE).is_empty());
    assert_eq!(r.at(&EnsembleFields::ENSEMBLE_LABEL), None);
    assert_eq!(r.at(&SignalFields::SNR_DB), None);
    assert_eq!(r.at(&SyncFields::FRAMES), None);
    assert_eq!(r.at(&AudioFields::AUDIO_CODEC), None);
    // The guide stays while the new ensemble fills its own tables.
    let y = generate_air(data_ensemble(), Air::cn(30.0), 3.0);
    let r = feed(r.rx, &y, 65_536);
    assert_eq!(r.rows(&bc::GUIDE_SERVICE_TABLE).len(), services);
    assert!(!r.rows(&SERVICES).is_empty());
    assert!(r.at(&EnsembleFields::ENSEMBLE_LABEL).is_some());
}

#[test]
fn spi_and_journaline_in_packet_mode_with_fec() {
    // SPI on packet address 100 of sub-channel 7.
    let x = generate_air(data_ensemble(), Air::cn(30.0), 6.0);
    let r = run_with(
        &x,
        RATE,
        &[("service", 1.0), ("data_subchannel", 7.0)],
        65_536,
    );
    use sdrkit_proto_bcdata::publish as bc;
    let services = r.rows(&bc::GUIDE_SERVICE_TABLE);
    assert_eq!(services.len(), 2);
    assert!(services
        .iter()
        .any(|(_, s)| s.get(&bc::NAME) == Some("Test Tones 1")));
    let programmes = r.rows(&bc::GUIDE_PROGRAMME_TABLE);
    assert!(
        programmes
            .iter()
            .any(|(_, p)| p.get(&bc::NAME) == Some("Morning Tones")),
        "{programmes:?}"
    );
    assert!(programmes
        .iter()
        .any(|(_, p)| p.get(&bc::START) == Some(1_700_001_800.0)));
    assert!(r.count(&DataFields::SPI_OBJECTS) > 0);
    // Journaline on address 101: signalled by FIG 0/13 for SCIdS 1 with a second packet component.
    let mut p = data_ensemble();
    p.extra_figs.push(gd::fig0_3(0x102, true, 60, 7, 101));
    p.extra_figs
        .push(gd::fig0_8(0xE0C0_DE01, 1, None, Some(0x102)));
    p.extra_figs
        .push(gd::fig0_13(0xE0C0_DE01, 1, 0x44A, &[0, 0]));
    p.services[2].extra_components.push((3, 0, 0x102));
    let x = generate(p, 6.0);
    let r = run_with(
        &x,
        RATE,
        &[("service", 1.0), ("data_subchannel", 7.0)],
        65_536,
    );
    let row = |t: &sdrkit_core::publish::TableDef, k: &str| {
        r.rows(t)
            .into_iter()
            .find(|(key, _)| *key == k)
            .map(|(_, row)| row.clone())
    };
    assert_eq!(
        row(&JOURNALINE, "0000").unwrap().at(&JournalineRow::TITLE),
        Some("Test News")
    );
    assert_eq!(
        row(&JOURNALINE_LINKS, "0000/0").unwrap().get(&keys::TEXT),
        Some("Weather")
    );
    assert_eq!(
        row(&JOURNALINE, "0010").unwrap().at(&JournalineRow::BODY),
        Some("Sunny, 21 degrees")
    );
    assert_eq!(
        row(&JOURNALINE_ITEMS, "0011/1").unwrap().get(&keys::TEXT),
        Some("Away\t0:0")
    );
}

#[test]
fn dmb_video_service() {
    let mut p = GenParams::default();
    p.services.truncate(1);
    // AAC-LC units for the DMB audio from the generator's encoder.
    let mut enc = sdrkit_codec_aac::Encoder::new(48_000).unwrap();
    let tone: Vec<f32> = (0..960 * 50)
        .map(|i| (0.3 * (std::f64::consts::TAU * 440.0 * i as f64 / 48_000.0).sin()) as f32)
        .collect();
    let units: Vec<Vec<u8>> = tone
        .chunks(960)
        .map(|c| {
            let mut u = Vec::new();
            enc.encode(c, c, &[], 120, &mut u).unwrap();
            u
        })
        .collect();
    let asc = sdrkit_stack_dab::audio::dabplus::AudioParams::from_header(0x50)
        .unwrap()
        .audio_specific_config();
    let frames = gd::dmb_frames(320, 5, &units, &asc);
    p.services.push(ServiceSpec {
        sid: 0xC401,
        label: "Test DMB".into(),
        subch: 9,
        bitrate: 320,
        protection: Protection::Eep {
            option: 0,
            level: 3,
        },
        audio: ServiceAudio::Data {
            tmid: 1,
            dscty: 24,
            scid: 0,
            frames,
        },
        programme: Programme::Silence,
        pad: PadContent::default(),
        ca: false,
        extra_components: Vec::new(),
    });
    let x = generate(p, 5.0);
    let mut rx = receiver(&[], RATE);
    select(&mut rx, "C401");
    let r = feed(rx, &x, 65_536);
    let dmb = r
        .rows(&SERVICES)
        .into_iter()
        .find(|(k, _)| *k == "C401")
        .unwrap()
        .1
        .clone();
    assert_eq!(dmb.at(&ServiceRow::KIND), Some(ServiceKind::Dmb));
    assert!(r.count(&DmbFields::DMB_TS_PACKETS) > 500);
    assert_eq!(r.count(&DmbFields::DMB_RS_FAILED), 0);
    assert_eq!(r.at(&DmbFields::DMB_VIDEO_WIDTH), Some(64));
    assert_eq!(r.at(&DmbFields::DMB_VIDEO_HEIGHT), Some(48));
    let video = r.units(StreamKind::H264);
    assert!(video.len() >= 10, "{} pictures", video.len());
    assert_eq!(video[0].format.fourcc().as_str(), "avc1");
    assert!(video[0].packet.bytes().starts_with(&[0, 0, 0, 1, 0x67]));
    let audio = r.units(StreamKind::Aac);
    assert!(audio.len() > 50);
    assert_eq!(&audio[0].format.config[..], &asc[..]);
}

#[test]
fn transmitter_identification() {
    let x = default_signal(
        GenParams {
            tii: vec![(1, 1, 0.0), (1, 3, -5.0), (1, 7, -11.0)],
            ..Default::default()
        },
        Air::cn(30.0),
        5.0,
    );
    let r = run(&x, RATE, 1, 65_536);
    let t = r.rows(&TRANSMITTERS);
    let ids: Vec<(i64, i64)> = t
        .iter()
        .map(|(_, v)| {
            (
                v.at(&TransmitterRow::MAIN).unwrap(),
                v.at(&TransmitterRow::SUB).unwrap(),
            )
        })
        .collect();
    assert!(
        ids.contains(&(1, 1)) && ids.contains(&(1, 3)) && ids.contains(&(1, 7)),
        "{ids:?}"
    );
    let level = t
        .iter()
        .find(|(k, _)| *k == "1.3")
        .unwrap()
        .1
        .at(&TransmitterRow::LEVEL_DB)
        .unwrap();
    assert!((level + 5.0).abs() < 1.5, "{level}");
    // The TII does not disturb decoding.
    assert_eq!(r.count(&SyncFields::FIB_BAD), 0);
    let r = run_with(&x, RATE, &[("service", 1.0), ("tii", 0.0)], 65_536);
    assert!(r.rows(&TRANSMITTERS).is_empty());
}

#[test]
fn settings_take_effect() {
    let x = default_signal(GenParams::default(), Air::cn(25.0), 4.0);
    let r = run_with(&x, RATE, &[("service", 1.0)], 65_536);
    assert_eq!(r.at(&SyncFields::DETECTION), Some(Detection::Coherent));
    let mut rx = receiver(&[("service", 1.0)], RATE);
    set(&mut rx, "detection", sdrkit_core::Value::Choice(1));
    set(&mut rx, "aac_packaging", sdrkit_core::Value::Choice(1));
    let r = feed(rx, &x, 65_536);
    assert_eq!(r.at(&SyncFields::DETECTION), Some(Detection::Differential));
    assert!(r
        .media
        .iter()
        .any(|m| m.format.packaging == sdrkit_core::media::Packaging::Latm));
    // Early start off: the first access unit comes later.
    let a = run_with(&x, RATE, &[("service", 1.0)], 65_536)
        .at(&TimingFields::TIMING_FIRST_ACCESS_UNIT)
        .unwrap();
    let b = run_with(&x, RATE, &[("service", 1.0), ("early_start", 0.0)], 65_536)
        .at(&TimingFields::TIMING_FIRST_ACCESS_UNIT)
        .unwrap();
    assert!(b > a + 0.05, "{a} {b}");
    // Impulses: blanked with the blanker, not without.
    let mut y = x.clone();
    for i in (100_000..y.len()).step_by(20_000) {
        for v in &mut y[i..i + 50] {
            *v *= 30.0;
        }
    }
    let on = run_with(&y, RATE, &[("service", 1.0)], 65_536);
    assert!(on.count(&SyncFields::IMPULSES_BLANKED) > 1000);
    let off = run_with(
        &y,
        RATE,
        &[("service", 1.0), ("impulse_blanker", 0.0)],
        65_536,
    );
    assert_eq!(off.count(&SyncFields::IMPULSES_BLANKED), 0);
    assert!(sdrkit_stack_dab::DESCRIPTOR.make_generator.is_some());
}

#[test]
fn service_switches_while_locked() {
    let x = default_signal(GenParams::default(), Air::cn(25.0), 6.0);
    let mut rx = receiver(&[("service", 1.0)], RATE);
    let mut out = Output::default();
    let mut first_after = Vec::new();
    let mut switched_at = None;
    let mut t = 0.0;
    for (k, c) in x.chunks(20_480).enumerate() {
        if k == 150 || k == 250 {
            select(&mut rx, &sid_of(if k == 150 { 3 } else { 4 }));
            switched_at = Some(t);
        }
        out.clear();
        rx.feed(c, &mut out);
        t += c.len() as f64 / RATE;
        if let Some(s) = switched_at {
            if !out.media.is_empty() || !out.audio().is_empty() {
                first_after.push(t - s);
                switched_at = None;
            }
        }
    }
    assert_eq!(first_after.len(), 2);
    assert!(first_after.iter().all(|&d| d < 1.0), "{first_after:?}");
    let f = rx.results().fields();
    assert_eq!(f.at(&SyncFields::SYNC_LOSSES), Some(0));
    assert_eq!(f.at(&PlayingFields::SELECTED_SUBCHANNEL), Some(4));
}

#[test]
fn a_rate_too_low_is_refused() {
    let mut rx = receiver(&[], RATE);
    assert!(rx
        .configure(&InputInfo::new(1.5e6, 225.648e6, 225.648e6))
        .is_err());
    assert!(rx
        .configure(&InputInfo::new(1.6e6, 225.648e6, 225.648e6))
        .is_ok());
}

#[test]
fn every_event_is_declared_with_its_frequency() {
    let p = GenParams {
        announcement: Some(sdrkit_stack_dab::generator::AnnouncementSpec {
            cluster: 1,
            support: 0x0003,
            active: 0x0002,
            subch: 2,
        }),
        ..Default::default()
    };
    let x = default_signal(GenParams::default(), Air::default(), 4.0);
    let mut y = generate(p, 3.0);
    y.extend_from_slice(&x);
    let r = run(&y, RATE, 3, 65_536);
    let decl = sdrkit_stack_dab::typed::shape().results_decl();
    let mut kinds = std::collections::BTreeSet::new();
    for e in &r.events {
        assert!(
            decl.events().iter().any(|d| d.kind == e.kind()),
            "{} is not declared",
            e.kind()
        );
        assert!(e.freq_hz > 0.0, "{} sent without its frequency", e.kind());
        assert!(e.get(&keys::FAMILY).is_some(), "{} has no family", e.kind());
        kinds.insert(e.kind().to_string());
    }
    for k in [
        "dab.ensemble",
        "dab.services",
        "dab.label",
        "dab.slide",
        "dab.announcement_started",
    ] {
        assert!(kinds.contains(k), "{k} not seen: {kinds:?}");
    }
}

/// The `channels` value of the DAB groups `ids` of the data folder.
fn group_channels(ids: &[&str]) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data")
        .join(sdrkit_presets::dab::DAB_PLANS_FILE);
    let groups = sdrkit_presets::dab::read_dab_groups(&path).unwrap();
    let s = sdrkit_presets::dab::dab_settings(&groups, ids).unwrap();
    match &s[0].1 {
        sdrkit_core::Value::Text(t) => t.clone(),
        v => panic!("{v:?}"),
    }
}

/// Service following on the channels of two groups of the data folder: each list keeps only the alternatives on its own
/// channels, and an alternative on a block of EN 300 401 takes the name of the block.
#[test]
fn following_keeps_the_channels_of_the_list() {
    let list = |ids: &[&str]| sdrkit_stack_dab::channels::parse_list(&group_channels(ids)).unwrap();
    let mut db = Ensemble::default();
    db.eid = Some(0xCE15);
    db.services.insert(0xCE01, Default::default());
    apply(&mut db, &gd::fig0_21(0xCE15, 225_648, 0xD201, 88_500));
    for l in db.frequencies.values_mut().filter(|l| l.id == 0xCE15) {
        let mut other = l.freqs_khz[0];
        other.0 = 175_280;
        l.freqs_khz.push(other);
    }
    let europe = sdrkit_stack_dab::follow::alternatives(&db, 0xCE01, &list(&["dab_band3"]));
    let korea = sdrkit_stack_dab::follow::alternatives(&db, 0xCE01, &list(&["dab_tdmb_kr"]));
    let hz = |v: &[sdrkit_stack_dab::follow::Alternative]| {
        v.iter().map(|a| (a.hz, a.channel)).collect::<Vec<_>>()
    };
    assert_eq!(hz(&europe), vec![(225.648e6, "12B")]);
    assert_eq!(hz(&korea), vec![(175.28e6, "")]);
}

/// The receiver takes the channel lists of two groups of the data folder, keeps the list it has when a malformed one is
/// set, and has no band setting.
#[test]
fn the_receiver_takes_the_channels_of_two_groups() {
    let mut rx = receiver(&[], RATE);
    let id = rx.schema().id("channels").unwrap();
    assert!(rx.schema().id("band").is_err());
    let now = |rx: &sdrkit_graph::StackGraph| match Owner::values(rx).and_then(|v| v.value(id)) {
        Some(sdrkit_core::Value::Text(t)) => t.clone(),
        v => panic!("{v:?}"),
    };
    for ids in [&["dab_band3", "dab_band3_lband"][..], &["dab_tdmb_kr"]] {
        let text = group_channels(ids);
        set(&mut rx, "channels", sdrkit_core::Value::Text(text.clone()));
        assert_eq!(now(&rx), text);
        for bad in ["12B", "225648000;-1"] {
            let v = sdrkit_core::Value::Text(bad.into());
            assert!(sdrkit_core::owner::apply(&mut rx, id, &v).is_err(), "{bad}");
        }
        assert_eq!(now(&rx), text);
    }
}
