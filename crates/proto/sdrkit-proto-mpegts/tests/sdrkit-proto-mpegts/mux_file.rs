//! Reading the whole of `tests/vectors/dvbt/mux_two_services.ts` produces every value of part "mux" of `expected.json`.

use sdrkit_core::Choice;
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_proto_mpegts::codec::{CodecRule, EsCodec, EsKind, EsPackaging};
use sdrkit_proto_mpegts::si::ServiceKind;
use sdrkit_proto_mpegts::{Demultiplexer, DemuxOutput};
use serde_json::Value;

/// The test multiplex and its expected values, or `None` (the test is skipped) when the recording is not in the recordings folder.
fn load() -> Option<(Vec<u8>, Value)> {
    let path = sdrkit_dev_test::recording("dvbt/mux_two_services.ts")?;
    let ts = std::fs::read(path).expect("test vector");
    Some((ts, read_expected("dvbt").expect("expected.json")))
}

fn hex(pid: u16) -> String {
    format!("0x{pid:04X}")
}

#[test]
fn the_test_multiplex_demultiplexes_as_expected() {
    let Some((ts, exp)) = load() else {
        return;
    };
    let m = &exp["mux"];
    let mut d = Demultiplexer::new();
    d.probe_all = true;
    d.si.country = "AUS".into();
    d.auto_select = true;
    let mut out = DemuxOutput::default();
    let mut media = 0usize;
    let mut aligner = sdrkit_proto_mpegts::input::TsAligner::new();
    let mut packets = Vec::new();
    aligner.push(&ts, &mut |p| packets.push((p.bytes.to_vec(), p.loss)));
    aligner.finish(&mut |p| packets.push((p.bytes.to_vec(), p.loss)));
    assert_eq!(
        aligner.stride(),
        m["packet_size"].as_u64().unwrap() as usize
    );
    for (p, loss) in &packets {
        d.push(p, None, *loss, &mut out);
        media += out.media.len();
        out.clear();
    }
    d.flush(&mut out);
    assert!(media > 0);
    eprintln!(
        "media packets {media}, services {:?}",
        d.si.listed()
            .iter()
            .map(|s| (
                &s.name,
                s.streams
                    .iter()
                    .map(|e| (&e.codec, e.props.width, e.props.sample_rate))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
    let st = &d.ts.stats;
    assert_eq!(st.packets, m["packets"].as_u64().unwrap());
    assert_eq!(st.cc_errors, m["cc_errors"].as_u64().unwrap());
    assert_eq!(st.tei_packets, m["tei_packets"].as_u64().unwrap());
    // Rate and duration from the PCR.
    let rate = st.packets as f64 * 1504.0 / d.ts.now.max(1e-9) * (st.packets - 1) as f64
        / st.packets as f64;
    let duration = st.packets as f64 * 1504.0 / m["constant_rate_bps"].as_f64().unwrap();
    assert!(
        (duration - m["duration_s"].as_f64().unwrap()).abs() <= 0.01,
        "duration {duration}"
    );
    assert!(
        (rate / m["constant_rate_bps"].as_f64().unwrap() - 1.0).abs() < 2e-3,
        "rate {rate}"
    );
    let si = &d.si;
    assert_eq!(si.tsid as u64, m["transport_stream_id"].as_u64().unwrap());
    assert_eq!(si.onid as u64, m["original_network_id"].as_u64().unwrap());
    assert_eq!(si.network_id as u64, m["network_id"].as_u64().unwrap());
    assert_eq!(si.network_name, m["network_name"].as_str().unwrap());
    for (pid, n) in m["pid_counts"].as_object().unwrap() {
        let p = u16::from_str_radix(&pid[2..], 16).unwrap();
        assert_eq!(
            d.ts.report(p).packets,
            n.as_u64().unwrap(),
            "packets of {pid}"
        );
    }
    for (pid, n) in m["pes_starts"].as_object().unwrap() {
        let p = u16::from_str_radix(&pid[2..], 16).unwrap();
        assert_eq!(
            d.ts.report(p).pes_starts,
            n.as_u64().unwrap(),
            "PES starts of {pid}"
        );
    }
    for (pid, n) in m["first_pts_90khz"].as_object().unwrap() {
        let p = u16::from_str_radix(&pid[2..], 16).unwrap();
        assert_eq!(
            d.ts.report(p).first_pts,
            Some(n.as_i64().unwrap()),
            "first PTS of {pid}"
        );
    }
    for (pid, v) in m["pcr"].as_object().unwrap() {
        let p = u16::from_str_radix(&pid[2..], 16).unwrap();
        let r = d.ts.report(p);
        assert_eq!(r.pcr_count, v["count"].as_u64().unwrap(), "PCRs of {pid}");
        assert_eq!(
            r.first_pcr,
            Some(v["first_27mhz"].as_u64().unwrap()),
            "first PCR of {pid}"
        );
    }
    assert_eq!(
        sdrkit_proto_mpegts::time::utc_text(si.first_utc.unwrap()),
        m["utc_first_tdt"].as_str().unwrap()
    );
    assert_eq!(
        sdrkit_proto_mpegts::time::utc_text(si.last_utc.unwrap()),
        m["utc_last_tdt"].as_str().unwrap()
    );
    let lto = &m["local_time_offset"];
    let o = &si.offsets[0];
    assert_eq!(o.country, lto["country"].as_str().unwrap());
    assert_eq!(o.region as u64, lto["region"].as_u64().unwrap());
    assert_eq!(o.offset_min as i64, lto["offset_min"].as_i64().unwrap());
    assert_eq!(
        sdrkit_proto_mpegts::time::utc_text(o.time_of_change),
        lto["time_of_change"].as_str().unwrap()
    );
    assert_eq!(
        o.next_offset_min as i64,
        lto["next_offset_min"].as_i64().unwrap()
    );
    // Terrestrial delivery.
    let td = &m["terrestrial_delivery"];
    match &si.own_delivery[0] {
        sdrkit_proto_mpegts::tables::Delivery::Terrestrial {
            frequency_hz,
            bandwidth_mhz,
            constellation,
            hierarchy,
            code_rate_hp,
            code_rate_lp,
            guard_interval,
            mode,
            other_frequency,
            ..
        } => {
            assert_eq!(*frequency_hz, td["frequency_hz"].as_u64().unwrap());
            assert_eq!(*bandwidth_mhz as u64, td["bandwidth_mhz"].as_u64().unwrap());
            assert_eq!(constellation, td["constellation"].as_str().unwrap());
            assert_eq!(hierarchy, td["hierarchy"].as_str().unwrap());
            assert_eq!(code_rate_hp, td["code_rate_hp"].as_str().unwrap());
            assert_eq!(code_rate_lp, td["code_rate_lp"].as_str().unwrap());
            assert_eq!(guard_interval, td["guard_interval"].as_str().unwrap());
            assert_eq!(mode, td["mode"].as_str().unwrap());
            assert_eq!(*other_frequency, td["other_frequency"].as_bool().unwrap());
        }
        other => panic!("delivery {other:?}"),
    }
    // Services.
    let order: Vec<u64> = si.listed().iter().map(|s| s.service_id as u64).collect();
    let want: Vec<u64> = m["service_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(order, want);
    for es in m["services"].as_array().unwrap() {
        let sid = es["service_id"].as_u64().unwrap() as u16;
        let s = si.service(sid).unwrap();
        assert_eq!(s.name, es["name"].as_str().unwrap());
        assert_eq!(s.provider, es["provider"].as_str().unwrap());
        assert_eq!(s.service_type as u64, es["service_type"].as_u64().unwrap());
        assert_eq!(
            ServiceKind::CHOICES[usize::from(s.kind.index())].label,
            es["type_name"].as_str().unwrap()
        );
        assert_eq!(s.lcn as u64, es["lcn"].as_u64().unwrap());
        assert_eq!(s.visible, es["visible"].as_bool().unwrap());
        assert_eq!(s.pmt_pid as u64, es["pmt_pid"].as_u64().unwrap());
        assert_eq!(s.pcr_pid as u64, es["pcr_pid"].as_u64().unwrap());
        assert_eq!(
            s.running_status as u64,
            es["running_status"].as_u64().unwrap()
        );
        assert_eq!(s.free_ca, es["free_ca"].as_bool().unwrap());
        assert_eq!(s.scrambled, es["scrambled"].as_bool().unwrap());
        assert_eq!(s.eit_schedule, es["eit_schedule"].as_bool().unwrap());
        assert_eq!(s.eit_pf, es["eit_present_following"].as_bool().unwrap());
        assert_eq!(
            s.now.as_ref().map(|e| e.title.as_str()),
            es["now"].as_str(),
            "now of {sid}"
        );
        assert_eq!(
            s.next.as_ref().map(|e| e.title.as_str()),
            es["next"].as_str(),
            "next of {sid}"
        );
        let streams = es["streams"].as_array().unwrap();
        assert_eq!(s.streams.len(), streams.len());
        for (e, x) in s.streams.iter().zip(streams) {
            let p = &e.props;
            assert_eq!(e.pid as u64, x["pid"].as_u64().unwrap());
            assert_eq!(e.stream_type as u64, x["stream_type"].as_u64().unwrap());
            let kind = match e.kind {
                Some(EsKind::Video) => "video",
                Some(EsKind::Audio) => "audio",
                _ => "other",
            };
            assert_eq!(kind, x["kind"].as_str().unwrap());
            assert_eq!(
                e.codec
                    .map(|c| &*EsCodec::CHOICES[usize::from(c.index())].key),
                x["codec"].as_str(),
                "codec of {}",
                hex(e.pid)
            );
            assert_eq!(
                e.fourcc().map(|f| f.as_str().to_owned()).as_deref(),
                x["fourcc"].as_str(),
                "fourcc of {}",
                hex(e.pid)
            );
            if let Some(pk) = x["packaging"].as_str() {
                let p = e.packaging.unwrap();
                assert_eq!(&*EsPackaging::CHOICES[usize::from(p.index())].key, pk);
            }
            if let Some(d) = x["descriptor"].as_str() {
                // The descriptor that decided, written as "AC-3 (0x6A)".
                let tag = d
                    .split_once("(0x")
                    .and_then(|(_, t)| u8::from_str_radix(t.trim_end_matches(')'), 16).ok());
                assert_eq!(
                    e.rule,
                    tag.map(CodecRule::Descriptor),
                    "rule of {}",
                    hex(e.pid)
                );
            }
            if let Some(l) = x["lang"].as_str() {
                assert_eq!(e.lang, l);
            }
            if let Some(v) = x["width"].as_u64() {
                assert_eq!(
                    (p.width as u64, p.height as u64),
                    (v, x["height"].as_u64().unwrap()),
                    "size of {}",
                    hex(e.pid)
                );
                let fr = p.frame_rate.unwrap();
                assert_eq!(
                    (fr.0 as f64 / fr.1 as f64).round() as u64,
                    x["frame_rate"].as_u64().unwrap()
                );
                assert_eq!(p.aspect, x["aspect"].as_str().unwrap());
                assert_eq!(p.profile.as_deref(), x["profile"].as_str());
            }
            if let Some(v) = x["sample_rate"].as_u64() {
                assert_eq!(p.sample_rate as u64, v, "rate of {}", hex(e.pid));
                assert_eq!(p.channels as u64, x["channels"].as_u64().unwrap());
            }
            if let Some(v) = x["bitrate_kbps"].as_u64() {
                assert_eq!(p.bitrate_kbps as u64, v);
            }
            if let Some(v) = x["object_type"].as_str() {
                assert_eq!(p.object_type, v);
            }
        }
    }
    // The guide.
    for (sid, events) in m["epg"].as_object().unwrap() {
        let sid: u16 = sid.parse().unwrap();
        let got = si.epg.events((si.onid, si.tsid, sid));
        let want = events.as_array().unwrap();
        assert_eq!(got.len(), want.len(), "events of {sid}");
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.event_id as u64, w["event_id"].as_u64().unwrap());
            assert_eq!(g.start, w["start_utc"].as_i64().unwrap());
            assert_eq!(g.duration, w["duration_s"].as_i64().unwrap());
            assert_eq!(
                g.running as u64,
                w["running"].as_u64().unwrap(),
                "running of {}",
                g.title
            );
            assert_eq!(g.genre as i64, w["genre_nibble"].as_i64().unwrap());
            assert_eq!(
                g.content.first().copied().unwrap_or(0) as u64,
                w["content_byte"].as_u64().unwrap()
            );
            assert_eq!(g.lang, w["lang"].as_str().unwrap());
            assert_eq!(g.title, w["title"].as_str().unwrap());
            assert_eq!(g.text, w["text"].as_str().unwrap());
            assert_eq!(g.extended, w["extended"].as_str().unwrap());
            if let Some(r) = w["parental_rating"].as_object() {
                for (c, v) in r {
                    assert_eq!(
                        g.ratings.iter().find(|x| &x.0 == c).map(|x| x.1 as u64),
                        v.as_u64()
                    );
                }
            }
            if let Some(a) = w["minimum_age"].as_u64() {
                assert_eq!(g.min_age as u64, a);
            }
        }
    }
}

#[test]
fn any_cut_of_the_file_returns_the_same_packets() {
    let Some((ts, _)) = load() else {
        return;
    };
    let mut whole = Vec::new();
    let mut a = sdrkit_proto_mpegts::input::TsAligner::new();
    a.push(&ts, &mut |p| whole.push(p.bytes.to_vec()));
    a.finish(&mut |p| whole.push(p.bytes.to_vec()));
    for cut in [1usize, 187, 4096] {
        let mut got = Vec::new();
        let mut a = sdrkit_proto_mpegts::input::TsAligner::new();
        for c in ts[..200_000].chunks(cut) {
            a.push(c, &mut |p| got.push(p.bytes.to_vec()));
        }
        a.finish(&mut |p| got.push(p.bytes.to_vec()));
        assert_eq!(got.len(), 200_000 / 188, "cut {cut}");
        assert!(got.iter().zip(&whole).all(|(a, b)| a == b));
    }
}
