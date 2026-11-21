//! The test recording decodes as `expected.json` says (part "dvbt"), with the bandwidth set to 7 MHz and with Auto. The packets
//! it delivers demultiplex as part "mux_from_recording" says. And any cut of the input decodes to the same packets.

use crate::common::{choice, config, drain_packets, rows};
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, Results, Value};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_proto_mpegts::source::FileSource;
use sdrkit_proto_mpegts::{Demultiplexer, DemuxOutput};
use sdrkit_stack_dvbt::settings::BandwidthChoice;

/// The results at the end and the transport packets delivered.
struct Run {
    results: Results,
    packets: Vec<[u8; 188]>,
}

fn run(
    bandwidth: BandwidthChoice,
    cut: usize,
    samples: &[Complex32],
    rate: f64,
    centre: f64,
) -> Run {
    let mut rx =
        sdrkit_stack_dvbt::receiver::receiver(&config(&[("bandwidth", choice(bandwidth))]))
            .unwrap();
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    let mut tap = rx.graph_mut().tap("fec.out", 1 << 14).unwrap();
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let mut packets = Vec::new();
    let mut out = Output::default();
    for c in samples.chunks(cut) {
        rx.feed(c, &mut out);
        out.clear();
        drain_packets(&mut tap, &mut buf, &mut packets);
    }
    rx.finish(&mut out);
    drain_packets(&mut tap, &mut buf, &mut packets);
    Run {
        results: Owner::results(&rx).clone(),
        packets,
    }
}

/// Field `k` as text: a choice by its key, other values as people read them.
fn field(rx: &Run, k: &str) -> String {
    let r = &rx.results;
    let Some(id) = r.decl().field_id(k) else {
        return String::new();
    };
    match r.fields().value(id) {
        Some(Value::Choice(i)) => r.decl().fields().field(id).unwrap().choices[usize::from(*i)]
            .key
            .to_string(),
        _ => r
            .fields()
            .text(id)
            .map(|t| t.to_string())
            .unwrap_or_default(),
    }
}

#[test]
fn the_recording_decodes_as_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("dvbt/dvbt_7mhz_2k_16qam_8msps") else {
        return;
    };
    let Some(ts) = sdrkit_dev_test::recording("dvbt/mux_two_services.ts") else {
        return;
    };
    let exp = read_expected("dvbt").unwrap();
    let e = &exp["dvbt"];
    let ts = std::fs::read(ts).unwrap();
    let source = FileSource::new(&ts);
    let src = source.packets();
    for bw in [BandwidthChoice::B7, BandwidthChoice::Auto] {
        let rx = run(bw, 16384, &rec.samples, rec.rate_hz, rec.centre_hz);
        assert_eq!(field(&rx, "channel_width"), "7000000", "{bw:?}");
        assert_eq!(field(&rx, "fft"), e["fft"].as_str().unwrap());
        assert_eq!(field(&rx, "fft_size"), e["fft_size"].to_string());
        assert_eq!(field(&rx, "guard"), e["guard_interval"].as_str().unwrap());
        assert_eq!(field(&rx, "guard_samples"), e["guard_samples"].to_string());
        assert_eq!(
            field(&rx, "constellation"),
            e["constellation"].as_str().unwrap()
        );
        assert_eq!(field(&rx, "hierarchy"), e["hierarchy"].as_str().unwrap());
        assert_eq!(
            field(&rx, "code_rate_hp"),
            e["code_rate_hp"].as_str().unwrap()
        );
        assert_eq!(
            field(&rx, "code_rate_lp"),
            e["code_rate_lp"].as_str().unwrap()
        );
        assert_eq!(field(&rx, "cell_id"), e["cell_id"].to_string());
        assert_eq!(
            field(&rx, "tps_length"),
            e["tps_length_indicator"].to_string()
        );
        // `state_at_end` "streaming": locked, with packet alignment.
        assert_eq!(e["state_at_end"].as_str(), Some("streaming"));
        assert_eq!(
            rx.results.status().lock_state,
            sdrkit_core::LockState::Locked
        );
        assert_eq!(field(&rx, "fec_sync"), "yes");
        let net: f64 = field(&rx, "net_bitrate_bps").parse().unwrap();
        assert!((net - e["net_bitrate_bps"].as_f64().unwrap()).abs() < 1.0);
        let cfo: f64 = field(&rx, "offset_hz").parse().unwrap();
        let tol = exp["tolerance"]["cfo_hz"].as_array().unwrap();
        assert!(
            cfo >= tol[0].as_f64().unwrap() && cfo <= tol[1].as_f64().unwrap(),
            "offset {cfo}"
        );
        let snr: f64 = field(&rx, "snr_db").parse().unwrap();
        let tol = exp["tolerance"]["snr_db"].as_array().unwrap();
        assert!(
            snr >= tol[0].as_f64().unwrap() && snr <= tol[1].as_f64().unwrap(),
            "snr {snr}"
        );
        let first = rx
            .results
            .fields()
            .at(&sdrkit_stack_dvbt::typed::DvbtFields::FIRST_PACKET)
            .unwrap_or(f64::MAX);
        assert!(first <= 0.25, "first packet at {first} s");
        let failed: u64 = field(&rx, "rs_failed").parse().unwrap();
        let corrected: u64 = field(&rx, "rs_corrected").parse().unwrap();
        let blocks: u64 = field(&rx, "rs_blocks").parse().unwrap();
        assert_eq!(failed, 0);
        assert!(corrected <= 5);
        let got = &rx.packets;
        assert!(got.len() >= 5500, "{} packets", got.len());
        assert!(blocks >= got.len() as u64 && blocks <= got.len() as u64 + 8);
        assert!(got.iter().all(|p| p[1] & 0x80 == 0), "a packet with TEI");
        // Every delivered packet is the source packet at its position, from a group boundary on.
        // The position in the source: the first offset where a run of 200 packets matches (null packets repeat).
        let k = (0..2000)
            .find(|&k| src[k..k + 200] == got[..200])
            .expect("the delivered packets are in the source");
        assert_eq!(k % 8, 0, "first source packet {k}");
        assert!(k <= 1600);
        let bad = got
            .iter()
            .enumerate()
            .filter(|(i, p)| src.get(k + i) != Some(*p))
            .count();
        if bad > 0 {
            let firstbad = got
                .iter()
                .enumerate()
                .position(|(i, p)| src.get(k + i) != Some(p))
                .unwrap();
            let p = &got[firstbad];
            let wherein = src.iter().position(|s| s == p);
            eprintln!(
                "k {k}, first bad {firstbad} of {}, found at {:?}; bytes {:02x?} vs {:02x?}",
                got.len(),
                wherein,
                &p[..8],
                &src[k + firstbad][..8]
            );
        }
        assert_eq!(bad, 0, "{bad} packets differ from the source");
        eprintln!("bandwidth {bw:?}: {} packets from source packet {k}, first at {first:.4} s, offset {cfo:.1} Hz, SNR {snr:.1} dB", got.len());
        // The demultiplexer finds both services, the network and now/next in them.
        let m = &exp["mux_from_recording"];
        let mut d = Demultiplexer::new();
        let mut out = DemuxOutput::default();
        for p in got {
            d.push(p, None, false, &mut out);
            out.clear();
        }
        assert_eq!(d.ts.stats.cc_errors, m["cc_errors"].as_u64().unwrap());
        let names: Vec<String> = d.si.listed().iter().map(|s| s.name.clone()).collect();
        let want: Vec<String> = m["services_named"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, want);
        assert_eq!(d.si.network_name, m["network_name"].as_str().unwrap());
        for sid in m["now_next_known_for"].as_array().unwrap() {
            let s = d.si.service(sid.as_u64().unwrap() as u16).unwrap();
            assert!(
                s.now.is_some() && s.next.is_some(),
                "now/next of {}",
                s.service_id
            );
        }
        if k == 352 && got.len() == 5991 {
            for (pid, n) in m["pid_counts"].as_object().unwrap() {
                let p = u16::from_str_radix(&pid[2..], 16).unwrap();
                assert_eq!(d.ts.report(p).packets, n.as_u64().unwrap(), "{pid}");
            }
        } else {
            // Recompute from the source slice.
            let mut d2 = Demultiplexer::new();
            for p in &src[k..k + got.len()] {
                d2.push(p, None, false, &mut out);
                out.clear();
            }
            for pid in m["pid_counts"].as_object().unwrap().keys() {
                let p = u16::from_str_radix(&pid[2..], 16).unwrap();
                assert_eq!(d.ts.report(p).packets, d2.ts.report(p).packets, "{pid}");
            }
        }
        // The services the receiver itself reports.
        assert_eq!(rows(&rx.results, "services"), 2);
        assert_eq!(field(&rx, "network_name"), "Test Network");
    }
}

#[test]
fn any_cut_returns_the_same_packets_and_a_reset_repeats_them() {
    let Some(rec) = sdrkit_dev_test::read_recording("dvbt/dvbt_7mhz_2k_16qam_8msps") else {
        return;
    };
    // A part of the recording (the decoder is the same at any cut, the whole file at every cut takes long in debug builds).
    let part = &rec.samples[..1_600_000];
    let base = run(BandwidthChoice::B7, 16384, part, rec.rate_hz, rec.centre_hz);
    let want = base.packets;
    assert!(want.len() > 500);
    for cut in [1usize << 20, 4096, 1013] {
        let rx = run(BandwidthChoice::B7, cut, part, rec.rate_hz, rec.centre_hz);
        assert!(rx.packets == want, "cut {cut}");
    }
    // One-sample pieces over a shorter part.
    let short = &rec.samples[..600_000];
    let a = run(
        BandwidthChoice::B7,
        16384,
        short,
        rec.rate_hz,
        rec.centre_hz,
    );
    let b = run(BandwidthChoice::B7, 1, short, rec.rate_hz, rec.centre_hz);
    assert!(a.packets == b.packets);
    // A reset (a reconfigure) followed by the same input delivers the same packets again.
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&config(&[(
        "bandwidth",
        choice(BandwidthChoice::B7),
    )]))
    .unwrap();
    let info = InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording();
    let mut out = Output::default();
    rx.configure(&info).unwrap();
    for c in part.chunks(16384) {
        rx.feed(c, &mut out);
    }
    rx.configure(&info).unwrap();
    let mut tap = rx.graph_mut().tap("fec.out", 1 << 14).unwrap();
    let mut buf = sdrkit_graph::Buffer::new(&sdrkit_proto_mpegts::stream::kind());
    let mut again = Vec::new();
    for c in part.chunks(16384) {
        rx.feed(c, &mut out);
        out.clear();
        drain_packets(&mut tap, &mut buf, &mut again);
    }
    rx.finish(&mut out);
    drain_packets(&mut tap, &mut buf, &mut again);
    assert!(again == want);
}
