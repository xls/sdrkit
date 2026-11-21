//! Transport stream recording, time-shift and playback on a generated multiplex.

mod support;

use sdrkit_core::{Storage, StorePath, Value};
use sdrkit_graph::{Buffer, Component, Format, Io};
use sdrkit_proto_mpegts::packet::{NULL_PID, TS_SIZE};
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_proto_mpegts::tables;
use sdrkit_svc_record::{read_recording, Container, Feed, TsFilePlayer};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use support::*;

#[test]
fn records_services_and_the_multiplex() {
    let mux = multiplex(4.0, 3, 0);
    let all = pids(&mux);
    let mut rig = Rig::new(&[("reserve_bytes", Value::I64(0)), ("prefix", text("tv"))]);
    let (_, svc) = rig.start(
        "ts.start",
        &[("session", text("s1")), ("services", text("1, 2"))],
    );
    let (_, whole) = rig.start("ts.start", &[("session", text("s1"))]);
    let (Feed::Ts(mut svc), Feed::Ts(mut whole)) = (svc, whole) else {
        panic!("not stream feeds")
    };
    for p in packets(&mux) {
        svc.push(&p);
        whole.push(&p);
    }
    rig.tick_until(|r| rows(&r.svc, "ts_files").len() == 3);
    drop((svc, whole));
    rig.settle();
    let files = rig.files("ts");
    assert_eq!(files.len(), 3, "{files:?}");
    assert!(rig.events_of("record.failed").is_empty());

    // The multiplex without null packets, in order.
    let mf = files.iter().find(|f| f.contains("_mux_")).unwrap();
    assert!(mf.contains("tv_mux_20250101-120000"), "{mf}");
    let got = rig.read(mf);
    let want: Vec<u16> = all.iter().copied().filter(|&p| p != NULL_PID).collect();
    assert_eq!(pids(&got), want);
    let r = read_recording(&*rig.storage, &StorePath::new(mf).unwrap()).unwrap();
    assert_eq!(
        (r.container, r.frames as usize),
        (Container::Ts, want.len())
    );

    // Service 1: its PMT, video and audio, and a PAT naming only it. Nothing passes before its PMT was seen.
    let f1 = files.iter().find(|f| f.contains("_sid1_")).unwrap();
    let b1 = rig.read(f1);
    let p1 = pids(&b1);
    assert!(p1.iter().all(|p| [0, PMT1, VID1, AUD1].contains(p)));
    let first_pmt = all.iter().position(|&p| p == PMT1).unwrap();
    let vids_after = all[first_pmt..].iter().filter(|&&p| p == VID1).count();
    assert_eq!(p1.iter().filter(|&&p| p == VID1).count(), vids_after);
    let pat_pkt = b1
        .as_chunks::<TS_SIZE>()
        .0
        .iter()
        .find(|p| u16::from_be_bytes([p[1], p[2]]) & 0x1FFF == 0)
        .unwrap();
    let sec = &pat_pkt[5..];
    let h = tables::check(sec).unwrap();
    assert_eq!(tables::pat(sec, &h).programs, vec![(1, PMT1)]);
    // Service 2.
    let f2 = files.iter().find(|f| f.contains("_sid2_")).unwrap();
    let p2 = pids(&rig.read(f2));
    assert!(p2.iter().all(|p| [0, PMT2, VID2].contains(p)));
    assert!(p2.contains(&VID2));

    // The sidecar.
    #[derive(serde::Deserialize)]
    struct Sidecar {
        service_id: u16,
        part: u32,
    }
    let side: Sidecar = serde_json::from_slice(&rig.read(&f1.replace(".ts", ".json"))).unwrap();
    assert_eq!((side.service_id, side.part), (1, 1));
}

#[test]
fn a_size_cap_starts_parts() {
    let mux = multiplex(4.0, 3, 0);
    let mut rig = Rig::new(&[
        ("reserve_bytes", Value::I64(0)),
        ("ts_drop_null", Value::Bool(false)),
        ("ts_max_bytes", Value::I64(100_000)),
    ]);
    let (_, feed) = rig.start("ts.start", &[("session", text("s1"))]);
    let Feed::Ts(mut f) = feed else {
        panic!("not a stream feed")
    };
    for p in packets(&mux) {
        f.push(&p);
    }
    drop(f);
    rig.settle();
    let files = rig.files("ts");
    assert!(files.len() >= 3, "{files:?}");
    let mut total = 0;
    for name in &files {
        let len = rig.read(name).len();
        assert!(len <= 100_000);
        total += len;
        assert!(!rig.read(&name.replace(".ts", ".json")).is_empty());
    }
    assert_eq!(total, mux.len());
    assert_eq!(rig.events_of("record.ended").len(), files.len());
}

#[test]
fn time_shift_pause_seek_and_live() {
    let mut rig = Rig::new(&[
        ("reserve_bytes", Value::I64(0)),
        ("timeshift_segment_bytes", Value::I64(64 * 188 * 5)),
        ("timeshift_bytes", Value::I64(64 * 188 * 5 * 8)),
    ]);
    let (row, feed) = rig.start("timeshift.start", &[("session", text("s1"))]);
    let Feed::TimeShift(mut f) = feed else {
        panic!("not a time-shift feed")
    };
    // About 56 400 bytes a second: 8 segments hold about 8.5 s.
    let all = packets(&multiplex(30.0, 4, 0));
    let per_s = all.len() / 30;
    for p in &all[..per_s * 6] {
        f.push(p);
    }
    f.flush();
    // Until the buffer holds what was pushed: the span stops growing.
    let mut last = Value::None;
    rig.tick_until(|r| {
        let now = r.field("timeshift_held_s");
        let steady = matches!(now, Value::Duration(s) if s > 5.5) && now == last;
        last = now;
        std::thread::sleep(std::time::Duration::from_millis(50));
        steady
    });
    let mut out = Vec::new();
    rig.svc.read_timeshift(0.0, &mut out, 10);
    // Playback is paced: at the same time, nothing more is due once the next PCR waits.
    while rig.svc.read_timeshift(0.0, &mut out, 10) {}
    let n = out.len();
    rig.svc.read_timeshift(0.0, &mut out, 10);
    assert_eq!(out.len(), n);
    rig.svc.tick(0.0, &mut rig.events);
    assert!(matches!(rig.field("timeshift_behind_s"), Value::Duration(b) if b < 0.3));
    // Back two seconds.
    rig.call("timeshift.seek", &[("behind_s", Value::Duration(2.0))])
        .unwrap();
    rig.svc.read_timeshift(1.0, &mut out, 1);
    rig.svc.tick(0.0, &mut rig.events);
    assert!(matches!(rig.field("timeshift_behind_s"), Value::Duration(b) if (b - 2.0).abs() < 0.3));
    // Paused while 20 more seconds arrive: the buffer drops the oldest segments.
    rig.call("timeshift.pause", &[("paused", Value::Bool(true))])
        .unwrap();
    for p in &all[per_s * 6..] {
        f.push(p);
    }
    f.flush();
    rig.tick_until(|r| matches!(r.field("timeshift_behind_s"), Value::Duration(b) if b > 20.0));
    std::thread::sleep(std::time::Duration::from_millis(200));
    rig.svc.tick(0.0, &mut rig.events);
    let segments = rig
        .storage
        .list(&StorePath::new("recordings/timeshift").unwrap())
        .unwrap();
    assert!(segments.len() <= 8, "{}", segments.len());
    rig.svc.tick(0.0, &mut rig.events);
    assert!(matches!(rig.field("timeshift_paused"), Value::Bool(true)));
    // Back to live.
    rig.call("timeshift.live", &[]).unwrap();
    while rig.svc.read_timeshift(2.0, &mut out, 10) {}
    rig.svc.tick(0.0, &mut rig.events);
    assert!(matches!(rig.field("timeshift_behind_s"), Value::Duration(b) if b < 0.3));
    rig.call("ts.stop", &[("row", text(&row))]).unwrap();
    drop(f);
    rig.settle();
    assert!(rig
        .storage
        .list(&StorePath::new("recordings/timeshift").unwrap())
        .is_err());
}

#[test]
fn the_player_paces_a_file_by_its_pcr_and_seeks() {
    let rig = Rig::new(&[]);
    // A PCR that wraps after 2 s.
    let wrap_base = (1u64 << 33) - 2 * 90_000;
    let bytes = multiplex(10.0, 2, wrap_base);
    let path = StorePath::new("tv/a.ts").unwrap();
    rig.storage.write(&path, &bytes).unwrap();
    let t = Arc::new(AtomicU64::new(100_000));
    let mut p = TsFilePlayer::new(rig.storage.clone(), clock(&t));
    let file = p.info().settings.id("file").unwrap();
    p.apply(file, &text("tv/a.ts")).unwrap();
    p.configure(&[]).unwrap();
    let field = |p: &TsFilePlayer, key: &str| {
        let r = p.results().unwrap();
        r.fields()
            .value(r.decl().field_id(key).unwrap())
            .cloned()
            .unwrap()
    };
    assert!(matches!(field(&p, "duration_s"), Value::Duration(d) if (d - 9.96).abs() < 1e-6));
    // One second of wall clock plays one second of stream.
    let run = |p: &mut TsFilePlayer, wall: f64| {
        let mut n = 0;
        let end = t.load(Ordering::Relaxed) + (wall * 1000.0) as u64;
        while t.load(Ordering::Relaxed) <= end {
            let formats = [Format::new(sdrkit_proto_mpegts::stream::kind(), 0.0)];
            let mut outs = [Buffer::new(&formats[0].kind)];
            let mut io = Io::standalone(&[], &formats, &[], &[0], &mut outs);
            p.process(&mut io);
            n += io.items_out::<TsPacket>(0).len();
            t.fetch_add(1, Ordering::Relaxed);
        }
        n
    };
    assert!(run(&mut p, 1.0) > 0);
    assert!(matches!(field(&p, "position_s"), Value::Duration(s) if (s - 1.0).abs() < 0.05));
    // Seek forward across the wrap.
    let seek = p.commands().id("seek").unwrap();
    let mut args = p.commands().def(seek).unwrap().new_args();
    args.set_value(args.decl().id("at_s").unwrap(), &Value::Duration(6.5))
        .unwrap();
    let checked = p.commands().check(seek, &args).unwrap();
    p.invoke(seek, &checked).unwrap();
    run(&mut p, 0.01);
    assert!(matches!(field(&p, "position_s"), Value::Duration(s) if (s - 6.48).abs() < 0.05));
}
