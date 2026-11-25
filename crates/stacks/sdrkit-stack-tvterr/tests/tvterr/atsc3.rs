//! ATSC 3.0 tests: bootstraps in the test signal at several rates, offsets and cuts, noise only, the rate rule,
//! and the delivery chain of the receiver graph fed with baseband packets.

use super::common::{self, chain, impair};
use sdrkit_core::{Complex32, ConfigError, InputInfo, Output, Owner, Results, StreamKind, Values};
use sdrkit_graph::media::{AudioProps, Codec, MediaPacket, Packaging};
use sdrkit_graph::{
    no_values, Buffer, CommandTarget, Component, ComponentInfo, Data, Format, Graph, GraphBuilder,
    Io, Kind, PacketPool, Port, TapReader, Timed,
};
use sdrkit_stack_tvterr::atsc3::components::ComponentState;
use sdrkit_stack_tvterr::atsc3::delivery::ComponentKind;
use sdrkit_stack_tvterr::atsc3::gen::Payload;
use sdrkit_stack_tvterr::atsc3::testsvc;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use sdrkit_proto_ipcast::alp::{write_bbp_header, write_single, BbpExtension, PacketType};
use sdrkit_proto_ipcast::ip::write_ipv4_udp;
use sdrkit_proto_ipcast::lct::LctBuilder;
use sdrkit_proto_ipcast::mp4::write_box;
use sdrkit_stack_tvterr::atsc3::svc::{build_lls, build_signed_lls, LLS_ADDR, LLS_PORT};
use sdrkit_stack_tvterr::atsc3::{delivery, results, Atsc3GenParams, Atsc3Generator, DESCRIPTOR};
use sdrkit_stack_tvterr::typed::{
    Atsc3ComponentRow as C, Atsc3Fields as F, Atsc3ServiceRow as S, ATSC3_COMPONENTS,
    ATSC3_SERVICES,
};
use std::net::Ipv4Addr;

#[test]
fn bootstraps_at_rates_offsets_and_cuts() {
    for (rate, cfo, snr, chunk) in [
        (6.144e6, 0.0, 25.0, 65536),
        (8e6, 3_000.0, 15.0, 4096),
        (10e6, -3_000.0, 25.0, 7),
        (20e6, 0.0, 25.0, 65536),
    ] {
        let x = common::generate(
            Atsc3Generator::new(Atsc3GenParams {
                rate_hz: rate,
                ..Default::default()
            })
            .unwrap(),
            0.55,
        );
        let x = impair(&x, rate, &chain(cfo, 0.0, Some(snr), 6.144e6));
        let mut rx = common::receiver(&DESCRIPTOR, &[]);
        let r = common::run(rx.as_mut(), rate, &x, chunk, "");
        let n = r.f("bootstraps");
        assert!(n >= 4.0, "{rate} {cfo}: {n} bootstraps");
        assert!(
            (r.f("offset_hz") - cfo).abs() < 300.0,
            "{rate}: cfo {}",
            r.f("offset_hz")
        );
        assert_eq!(r.f("preamble_structure"), 3.0);
        assert!(
            (r.f("min_time_next") - 0.15).abs() < 1e-9,
            "{}",
            r.f("min_time_next")
        );
    }
}

#[test]
fn noise_finds_no_bootstrap_and_rates_are_checked() {
    let x = common::noise(4_000_000, 0.09, 3);
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    let r = common::run(rx.as_mut(), 8e6, &x, 65536, "");
    assert_eq!(r.f("bootstraps"), 0.0);
    assert!(rx.configure(&InputInfo::new(6e6, 539e6, 539e6)).is_err());
    assert!(rx.configure(&InputInfo::new(6.144e6, 539e6, 539e6)).is_ok());
}

/// Half a second of the test signal at the declared rate, default settings: the stack is Locked, every frame whose samples
/// are in is decoded, and the baseband packets carry the datagrams of the generator.
#[test]
fn the_test_signal_locks_the_stack_within_half_a_second() {
    let rate = DESCRIPTOR.tuning.rate_hz;
    let x = common::generate(
        Atsc3Generator::new(Atsc3GenParams {
            rate_hz: rate,
            ..Default::default()
        })
        .unwrap(),
        0.5,
    );
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    let r = common::run(rx.as_mut(), rate, &x, 65536, "");
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert!(r.f("frames_ok") >= 3.0, "{} frames", r.f("frames_ok"));
    assert_eq!(r.f("frames_failed"), 0.0);
    assert_eq!(r.f("fft_size"), 8192.0);
    assert_eq!(r.f("guard_samples"), 192.0);
    assert_eq!((r.f("l1_basic_mode"), r.f("l1_detail_mode")), (4.0, 2.0));
    assert_eq!((r.f("preamble_symbols"), r.f("subframes")), (1.0, 1.0));
    assert_eq!(r.f("subframe_symbols"), 72.0);
    assert_eq!((r.f("pilot_dx"), r.f("pilot_dy")), (16.0, 2.0));
    assert_eq!((r.f("plps"), r.f("plp_id")), (1.0, 0.0));
    assert_eq!(
        (
            r.f("plp_points"),
            r.f("plp_code_rate"),
            r.f("plp_ldpc_bits")
        ),
        (4.0, 8.0, 64800.0)
    );
    assert_eq!(r.rows("plps"), 1);
    assert!(
        r.f("baseband_packets") >= 44.0,
        "{}",
        r.f("baseband_packets")
    );
    assert_eq!(r.f("baseband_packets_bad"), 0.0);
    assert!(r.f("mer_db") > 25.0, "{}", r.f("mer_db"));
    // A baseband packet of 4296 bytes holds three ALP packets and a half; each is one datagram of the generator.
    assert!(
        r.f("alp.alp_packets") >= 3.0 * 44.0,
        "{}",
        r.f("alp.alp_packets")
    );
    assert_eq!(r.f("alp.alp_dropped"), 0.0);
    assert!(
        r.f("ip.ip_packets") >= 3.0 * 44.0,
        "{}",
        r.f("ip.ip_packets")
    );
    assert_eq!(r.f("ip.ip_bad"), 0.0);
}

/// The test signal fed in pieces of 16384 samples at the declared rate: the state is Locked before 0.3 s of signal are in.
/// A frame is 0.1016 s long and the results are published every 0.1 s.
#[test]
fn the_stack_locks_within_three_tenths_of_a_second() {
    let rate = DESCRIPTOR.tuning.rate_hz;
    let x = common::generate(
        Atsc3Generator::new(Atsc3GenParams {
            rate_hz: rate,
            ..Default::default()
        })
        .unwrap(),
        0.5,
    );
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    rx.configure(&InputInfo::new(rate, 539e6, 539e6).recording())
        .unwrap();
    let mut out = Output::default();
    let mut locked_at = None;
    for (k, c) in x.chunks(16384).enumerate() {
        rx.feed(c, &mut out);
        out.clear();
        let state = Owner::results(rx.as_ref()).status().lock_state;
        if state == sdrkit_core::LockState::Locked {
            locked_at = Some((k + 1) as f64 * 16384.0 / rate);
            break;
        }
    }
    let t = locked_at.expect("the stack locks");
    assert!(t <= 0.3, "locked after {t} s");
}

/// The test signal 3 kHz off in noise of 10 dB (in 6.144 MHz), at 8 and at 20 Msps: at least 8 of the 9 frames of one
/// second decode and no baseband packet is bad.
#[test]
fn frames_decode_with_a_carrier_offset_in_noise_at_two_rates() {
    for (rate, cfo, chunk) in [(8e6, 3_000.0, 4096), (20e6, -3_000.0, 65536)] {
        let x = common::generate(
            Atsc3Generator::new(Atsc3GenParams {
                rate_hz: rate,
                ..Default::default()
            })
            .unwrap(),
            1.0,
        );
        let x = impair(&x, rate, &chain(cfo, 0.0, Some(10.0), 6.144e6));
        let mut rx = common::receiver(&DESCRIPTOR, &[]);
        let r = common::run(rx.as_mut(), rate, &x, chunk, "");
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Locked
        );
        assert!(
            r.f("frames_ok") >= 8.0,
            "{rate}: {} frames",
            r.f("frames_ok")
        );
        assert_eq!(r.f("frames_failed"), 0.0, "{rate}");
        assert_eq!(r.f("baseband_packets_bad"), 0.0, "{rate}");
        assert!(
            (r.f("offset_hz") - cfo).abs() < 300.0,
            "{rate}: {}",
            r.f("offset_hz")
        );
        assert!(
            (r.f("mer_db") - 10.7).abs() < 2.5,
            "{rate}: MER {}",
            r.f("mer_db")
        );
        assert_eq!(r.f("ip.ip_bad"), 0.0);
    }
}

/// Time-aligned frames with an L1-Detail of two coded blocks in four Preamble symbols, a PLP that begins after 1000 dummy
/// cells, and samples after the last symbol of each frame: the FEC blocks that cross from frame to frame are joined.
#[test]
fn time_aligned_frames_follow_each_other() {
    let rate = 6.144e6;
    let p = Atsc3GenParams {
        rate_hz: rate,
        fields: sdrkit_stack_tvterr::atsc3::bootstrap::Fields {
            preamble: 1,
            ..Atsc3GenParams::default().fields
        },
        symbols: 20,
        l1_detail_mode: 1,
        l1_detail_reserved: 400,
        time_aligned: true,
        plp_start: 1000,
        ..Default::default()
    };
    let x = common::generate(Atsc3Generator::new(p).unwrap(), 0.5);
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    let r = common::run(rx.as_mut(), rate, &x, 65536, "");
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked
    );
    // A frame is 35 ms; 13 are whole in 0.5 s with the delay of the search.
    assert!(r.f("frames_ok") >= 12.0, "{} frames", r.f("frames_ok"));
    assert_eq!(r.f("frames_failed"), 0.0);
    assert_eq!((r.f("preamble_symbols"), r.f("l1_detail_mode")), (4.0, 1.0));
    assert!(
        (r.f("frame_length") - 0.035).abs() < 1e-5,
        "{}",
        r.f("frame_length")
    );
    // 131 731 cells of the PLP a frame are 4.07 blocks, and most frames begin with the end of a block: without the join of
    // the crossing block about 3.1 blocks a frame would decode.
    let per_frame = r.f("baseband_packets") / r.f("frames_ok");
    assert!(per_frame >= 3.9, "{per_frame} packets a frame");
    assert_eq!(r.f("baseband_packets_bad"), 0.0);
}

/// When the signal stops, the state returns to Searching: 0.45 s of the test signal, then 1 s of noise.
#[test]
fn the_state_returns_to_searching_when_the_signal_stops() {
    let rate = DESCRIPTOR.tuning.rate_hz;
    let mut x = common::generate(
        Atsc3Generator::new(Atsc3GenParams {
            rate_hz: rate,
            ..Default::default()
        })
        .unwrap(),
        0.45,
    );
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    rx.configure(&InputInfo::new(rate, 539e6, 539e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65536) {
        rx.feed(c, &mut out);
        out.clear();
    }
    assert_eq!(
        Owner::results(rx.as_ref()).status().lock_state,
        sdrkit_core::LockState::Locked
    );
    x = common::noise(rate as usize, 0.01, 5);
    for c in x.chunks(65536) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let r = Owner::results(rx.as_ref());
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Searching);
    let f = sdrkit_stack_tvterr::typed::Atsc3Fields::BOOTSTRAP_SEEN;
    assert_eq!(r.fields().at(&f), Some(false));
}

/// One second of the test signal through the receiver on one thread (`cut` false) or with the stage of the FEC blocks on
/// its own thread: the stage names, the frames decoded and the baseband packets.
fn staged_run(cut: bool) -> (Vec<String>, Option<i64>, Option<i64>) {
    let rate = DESCRIPTOR.tuning.rate_hz;
    let x = common::generate(
        Atsc3Generator::new(Atsc3GenParams {
            rate_hz: rate,
            ..Default::default()
        })
        .unwrap(),
        1.0,
    );
    let settings = Values::defaults(&(DESCRIPTOR.settings)());
    let mut rx = sdrkit_stack_tvterr::atsc3::receiver(&sdrkit_core::StackConfig::new(settings))
        .expect("the receiver builds");
    sdrkit_core::Receiver::configure(&mut rx, &InputInfo::new(rate, 539e6, 539e6).recording())
        .expect("the rate");
    let threads: Vec<_> = sdrkit_core::Receiver::staged(&mut rx)
        .expect("stages")
        .cut(u32::from(cut))
        .into_iter()
        .map(|w| std::thread::spawn(move || w.run()))
        .collect();
    assert_eq!(threads.len(), usize::from(cut));
    let mut out = Output::default();
    for c in x.chunks(16_384) {
        sdrkit_core::Receiver::feed(&mut rx, c, &mut out);
        out.clear();
    }
    let st = sdrkit_core::Receiver::staged(&mut rx).expect("stages");
    st.drain(&mut out);
    let mut loads = Vec::new();
    st.stage_loads(&mut loads);
    drop(st.cut(0));
    for t in threads {
        t.join().expect("a stage thread");
    }
    let r = Owner::results(&rx);
    (
        loads.iter().map(|l| l.name.clone()).collect(),
        r.fields()
            .at(&sdrkit_stack_tvterr::typed::Atsc3Fields::FRAMES_OK),
        r.fields()
            .at(&sdrkit_stack_tvterr::typed::Atsc3FrameFields::BASEBAND_PACKETS),
    )
}

/// The stages are `front` (bootstraps, OFDM and signalling) and `fec` (the FEC blocks and all after them). With `fec` on its
/// own thread the frames and the baseband packets are those of one thread.
#[test]
fn the_fec_stage_on_its_own_thread_gives_the_same_frames() {
    let whole = staged_run(false);
    assert_eq!(whole.0, ["front", "fec"]);
    assert!(whole.1 >= Some(8), "{:?} frames", whole.1);
    assert!(whole.2 >= Some(8 * 14), "{:?} packets", whole.2);
    assert_eq!(staged_run(true), whole);
}

/// A FEC block of 16200 bits: QPSK at 8/15 with BCH, or 16 points at 6/15 with a CRC.
fn short_coding(sixteen: bool) -> sdrkit_stack_tvterr::atsc3::bicm::PlpCoding {
    use sdrkit_stack_tvterr::atsc3::bicm::{Outer, PlpCoding};
    PlpCoding {
        ldpc_bits: 16200,
        rate: if sixteen { 6 } else { 8 },
        bits: if sixteen { 4 } else { 2 },
        outer: if sixteen { Outer::Crc } else { Outer::Bch },
    }
}

/// The receiver with setting `plp` at `plp`.
fn receiver_of_plp(plp: i64) -> Box<dyn sdrkit_core::Receiver> {
    let schema = (DESCRIPTOR.settings)();
    let mut v = Values::defaults(&schema);
    let id = schema.id("plp").expect("the setting");
    v.set(id, &sdrkit_core::Value::I64(plp))
        .expect("a PLP id");
    (DESCRIPTOR.make_receiver)(&sdrkit_core::StackConfig::new(v)).expect("the receiver builds")
}

/// A PLP through each time interleaver, through the whole stack: 0.6 s of the test signal with the convolutional time
/// interleaver of 724 rows, whose lines fill in two frames, with the hybrid time interleaver of 3 TI blocks with the
/// cell interleaver, and with the hybrid time interleaver over 3 subframes. The stack locks, and no baseband packet is bad.
#[test]
fn the_stack_decodes_time_interleaved_plps() {
    use sdrkit_stack_tvterr::atsc3::gen::GenTi;
    let rate = 6.144e6;
    for (time_interleaver, least) in [
        (
            GenTi::Convolutional {
                depth: 1,
                extended: false,
            },
            2.0 * 59.0,
        ),
        (
            GenTi::Hybrid {
                inter_subframe: true,
                ti_blocks: 3,
                cell_interleaver: true,
                spare: 0,
            },
            2.0 * 59.0,
        ),
        (
            GenTi::Hybrid {
                inter_subframe: false,
                ti_blocks: 3,
                cell_interleaver: true,
                spare: 4,
            },
            4.0 * 59.0,
        ),
    ] {
        let p = Atsc3GenParams {
            rate_hz: rate,
            coding: short_coding(false),
            time_interleaver,
            ..Default::default()
        };
        let x = common::generate(Atsc3Generator::new(p).unwrap(), 0.6);
        let mut rx = common::receiver(&DESCRIPTOR, &[]);
        let r = common::run(rx.as_mut(), rate, &x, 65536, "");
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Locked,
            "{time_interleaver:?}"
        );
        assert_eq!(r.f("frames_failed"), 0.0, "{time_interleaver:?}");
        assert!(
            r.f("baseband_packets") >= least,
            "{time_interleaver:?}: {} packets",
            r.f("baseband_packets")
        );
        assert_eq!(r.f("baseband_packets_bad"), 0.0, "{time_interleaver:?}");
        assert_eq!(r.f("fec_blocks"), r.f("fec_blocks_good"));
        assert_eq!(r.f("ip.ip_bad"), 0.0);
    }
}

/// Two PLPs in one subframe, the second dispersed in 4 subslices: without the setting the stack decodes the first, with
/// `plp` at 1 the second, and with `plp` at 5, an id that the subframe does not hold, no frame decodes.
#[test]
fn the_setting_chooses_the_plp() {
    use sdrkit_stack_tvterr::atsc3::gen::{GenPlp, GenTi};
    let rate = 6.144e6;
    let p = Atsc3GenParams {
        rate_hz: rate,
        coding: short_coding(false),
        plp_cells: Some(200_000),
        more_plps: vec![GenPlp {
            coding: short_coding(true),
            seed: 9,
            start: 200_000,
            cells: Some(240_000),
            subslices: 4,
            interval: 70_000,
            time_interleaver: GenTi::None,
            lls: false,
            ldm: None,
        }],
        ..Default::default()
    };
    let x = common::generate(Atsc3Generator::new(p).unwrap(), 0.5);
    for (plp, id, points, per_frame) in [(-1, 0.0, 4.0, 24.0), (0, 0.0, 4.0, 24.0), (1, 1.0, 16.0, 59.0)] {
        let mut rx = receiver_of_plp(plp);
        let r = common::run(rx.as_mut(), rate, &x, 65536, "");
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Locked,
            "plp {plp}"
        );
        assert_eq!((r.f("plps"), r.f("plp_id")), (2.0, id), "plp {plp}");
        assert_eq!(r.f("plp_points"), points, "plp {plp}");
        assert_eq!(r.rows("plps"), 2);
        assert_eq!(r.f("frames_failed"), 0.0, "plp {plp}");
        assert!(r.f("frames_ok") >= 3.0, "plp {plp}");
        let packets = r.f("baseband_packets") / r.f("frames_ok");
        assert!(packets >= per_frame, "plp {plp}: {packets} packets a frame");
        assert_eq!(r.f("baseband_packets_bad"), 0.0, "plp {plp}");
    }
    let mut rx = receiver_of_plp(5);
    let r = common::run(rx.as_mut(), rate, &x, 65536, "");
    assert_eq!(r.f("frames_ok"), 0.0);
    assert_eq!(r.t("not_decoded"), "plp");
    assert!(receiver_of_plp(63).configure(&InputInfo::new(rate, 539e6, 539e6)).is_ok());
}

/// A core PLP with an enhanced PLP on all its cells 4 dB below it, as in A/322 clause 6.4.4 (QPSK 4/15 and 64 points
/// 10/15): without the setting and with `plp` at 0 the stack decodes the core PLP, with `plp` at 1 the enhanced PLP after
/// the core PLP is taken off. No baseband packet is bad.
#[test]
fn the_setting_chooses_a_layer() {
    use sdrkit_stack_tvterr::atsc3::bicm::{Outer, PlpCoding};
    use sdrkit_stack_tvterr::atsc3::gen::{GenLdm, GenPlp, GenTi};
    let rate = 6.144e6;
    let coding = |rate, bits| PlpCoding {
        ldpc_bits: 64800,
        rate,
        bits,
        outer: Outer::Bch,
    };
    let p = Atsc3GenParams {
        rate_hz: rate,
        coding: coding(4, 2),
        more_plps: vec![GenPlp {
            coding: coding(10, 6),
            seed: 9,
            start: 0,
            cells: None,
            subslices: 1,
            interval: 0,
            time_interleaver: GenTi::None,
            lls: false,
            ldm: Some(GenLdm { core: 0, cores: 1, level: 8 }),
        }],
        ..Default::default()
    };
    let x = common::generate(Atsc3Generator::new(p).unwrap(), 0.5);
    // 482 596 cells a frame: 14.9 core blocks of 32 400 cells, 44.7 enhanced blocks of 10 800.
    for (plp, id, points, per_frame) in [(-1, 0.0, 4.0, 14.0), (0, 0.0, 4.0, 14.0), (1, 1.0, 64.0, 43.0)] {
        let mut rx = receiver_of_plp(plp);
        let r = common::run(rx.as_mut(), rate, &x, 65536, "");
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Locked,
            "plp {plp}"
        );
        assert_eq!((r.f("plps"), r.f("plp_id")), (2.0, id), "plp {plp}");
        assert_eq!(r.f("plp_points"), points, "plp {plp}");
        assert_eq!(r.rows("plps"), 2);
        assert_eq!(r.f("frames_failed"), 0.0, "plp {plp}");
        assert!(r.f("frames_ok") >= 3.0, "plp {plp}");
        let packets = r.f("baseband_packets") / r.f("frames_ok");
        assert!(packets >= per_frame, "plp {plp}: {packets} packets a frame");
        assert_eq!(r.f("baseband_packets_bad"), 0.0, "plp {plp}");
    }
}

/// Two non-dispersed PLPs in one subframe, QPSK 8/15 with BCH and no time interleaver, and 16 points 6/15 with a CRC
/// through the hybrid time interleaver: each decodes when the setting chooses it, and the table lists both with the
/// coding, cells and time interleaver that L1-Detail signals, and marks the one decoded.
#[test]
fn the_plp_table_lists_every_plp_of_the_subframe() {
    use sdrkit_stack_tvterr::atsc3::gen::{FrameBuilder, GenPlp, GenTi};
    use sdrkit_stack_tvterr::typed::{Atsc3PlpRow, ATSC3_PLPS};
    let rate = 6.144e6;
    let p = Atsc3GenParams {
        rate_hz: rate,
        coding: short_coding(false),
        plp_cells: Some(200_000),
        more_plps: vec![GenPlp {
            coding: short_coding(true),
            seed: 9,
            start: 200_000,
            cells: None,
            subslices: 1,
            interval: 0,
            time_interleaver: GenTi::Hybrid {
                inter_subframe: false,
                ti_blocks: 2,
                cell_interleaver: true,
                spare: 0,
            },
            lls: false,
            ldm: None,
        }],
        ..Default::default()
    };
    let sent = FrameBuilder::new(p.clone()).unwrap().detail().subframes[0].plps.clone();
    assert_eq!(sent.len(), 2);
    let x = common::generate(Atsc3Generator::new(p).unwrap(), 0.5);
    for (plp, id) in [(-1, 0), (1, 1)] {
        let mut rx = receiver_of_plp(plp);
        let r = common::run(rx.as_mut(), rate, &x, 65536, "");
        assert_eq!(r.f("plp_id"), f64::from(id), "plp {plp}");
        assert_eq!(r.f("frames_failed"), 0.0, "plp {plp}");
        assert!(r.f("frames_ok") >= 3.0, "plp {plp}");
        assert_eq!(r.f("baseband_packets_bad"), 0.0, "plp {plp}");
        let mut rows: Vec<(String, Atsc3PlpRow)> = ATSC3_PLPS
            .rows::<Atsc3PlpRow>(&r.results)
            .map(|(k, row)| (k.to_owned(), row))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let want = |n: usize, points, rate, ti| {
            (
                format!("0.{n}"),
                Atsc3PlpRow {
                    subframe: 0,
                    id: n as i64,
                    layer: 0,
                    points: Some(points),
                    code_rate: Some(rate),
                    ldpc_bits: Some(16200),
                    cells: i64::from(sent[n].size),
                    time_interleaver: ti,
                    decoded: n == id as usize,
                    bonding: None,
                    bonded_bsid: None,
                },
            )
        };
        assert_eq!(rows, vec![want(0, 4, 8, 0), want(1, 16, 6, 2)], "plp {plp}");
    }
}

/// An enhanced PLP 10 dB below two core PLPs that it lies on (A/322 clause 7.2.7.4), without time interleaving: 16
/// points 8/15 and 6/15 with the 16200-bit code, 5 and 4 blocks of 4050 cells, and 64 points 10/15 with the 64800-bit
/// code, whose blocks cross from one core PLP to the other and from frame to frame. With `plp` at its id the stack
/// decodes it, no baseband packet is bad, and its modulation error ratio, measured after the core PLPs are taken off, is
/// far above the 10 dB that the core constellation would give.
#[test]
fn an_enhanced_plp_across_two_core_plps_decodes_through_the_stack() {
    use sdrkit_stack_tvterr::atsc3::bicm::{Outer, PlpCoding};
    use sdrkit_stack_tvterr::atsc3::gen::{GenLdm, GenPlp, GenTi};
    let rate = 6.144e6;
    let coding = |ldpc_bits, rate, bits| PlpCoding {
        ldpc_bits,
        rate,
        bits,
        outer: Outer::Bch,
    };
    let plp = |coding, start, cells, ldm| GenPlp {
        coding,
        seed: 9,
        start,
        cells,
        subslices: 1,
        interval: 0,
        time_interleaver: GenTi::None,
        lls: false,
        ldm,
    };
    let p = Atsc3GenParams {
        rate_hz: rate,
        coding: coding(16200, 8, 4),
        plp_cells: Some(5 * 4050),
        more_plps: vec![
            plp(coding(16200, 6, 4), 5 * 4050, Some(4 * 4050), None),
            plp(
                coding(64800, 10, 6),
                0,
                None,
                Some(GenLdm {
                    core: 0,
                    cores: 2,
                    level: 15,
                }),
            ),
        ],
        ..Default::default()
    };
    let x = common::generate(Atsc3Generator::new(p).unwrap(), 0.5);
    let mut rx = receiver_of_plp(2);
    let r = common::run(rx.as_mut(), rate, &x, 65536, "");
    assert_eq!((r.f("plp_id"), r.f("plp_points")), (2.0, 64.0));
    assert_eq!(r.f("frames_failed"), 0.0);
    assert!(r.f("frames_ok") >= 3.0);
    assert!(r.f("baseband_packets") >= 3.0 * 3.0);
    assert_eq!(r.f("baseband_packets_bad"), 0.0);
    assert!(r.f("mer_db") > 20.0, "{} dB", r.f("mer_db"));
}

/// A PLP bonded with another RF channel without SNR averaging (A/322 Annex K.2) decodes as a PLP of its own, and the
/// table shows the bonding and the BSID of the other channel. With SNR averaging (K.3) the stack does not decode it.
#[test]
fn the_plp_table_shows_a_bonded_plp() {
    use sdrkit_stack_tvterr::typed::{Atsc3PlpRow, ATSC3_PLPS};
    let rate = 6.144e6;
    for (format, decoded) in [(0u8, true), (1, false)] {
        let p = Atsc3GenParams {
            rate_hz: rate,
            coding: short_coding(false),
            bonding: Some((format, 0x2B1)),
            ..Default::default()
        };
        let x = common::generate(Atsc3Generator::new(p).unwrap(), 0.5);
        let mut rx = receiver_of_plp(-1);
        let r = common::run(rx.as_mut(), rate, &x, 65536, "");
        let rows: Vec<Atsc3PlpRow> = ATSC3_PLPS
            .rows::<Atsc3PlpRow>(&r.results)
            .map(|(_, row)| row)
            .collect();
        assert_eq!(rows.len(), 1, "format {format}");
        let row = &rows[0];
        assert_eq!(
            (row.bonding, row.bonded_bsid, row.decoded),
            (Some(i64::from(format)), Some(0x2B1), decoded),
            "format {format}"
        );
        if decoded {
            assert!(r.f("frames_ok") >= 3.0);
            assert!(r.f("baseband_packets") > 0.0);
            assert_eq!(r.f("baseband_packets_bad"), 0.0);
        } else {
            assert_eq!(r.t("not_decoded"), "plp");
        }
    }
}

/// Frames of three subframes (8K, 16K and 32K FFT). A PLP through the convolutional time interleaver continues in every
/// subframe: the stack locks and no baseband packet is bad. A second PLP that only the second subframe carries is decoded
/// when the setting names it.
#[test]
fn the_stack_takes_every_subframe_of_a_frame() {
    use sdrkit_stack_tvterr::atsc3::gen::{GenPlace, GenPlp, GenSubframe, GenTi};
    use sdrkit_stack_tvterr::atsc3::layout::FftSize;
    let rate = 6.144e6;
    let place = |plp, start, cells| GenPlace {
        plp,
        start,
        cells,
        subslices: 1,
        interval: 0,
    };
    let p = Atsc3GenParams {
        rate_hz: rate,
        symbols: 24,
        coding: short_coding(false),
        time_interleaver: GenTi::Convolutional {
            depth: 0,
            extended: false,
        },
        more_plps: vec![GenPlp {
            coding: short_coding(true),
            seed: 3,
            start: 0,
            cells: Some(0),
            subslices: 1,
            interval: 0,
            time_interleaver: GenTi::None,
            lls: false,
            ldm: None,
        }],
        more_subframes: vec![
            GenSubframe {
                fft: FftSize::K16,
                guard: 768,
                symbols: 16,
                reduced_carriers: 0,
                pilots: (16, 4),
                pilot_boost: 0,
                frequency_interleaver: true,
                boundary: (true, true),
                plps: vec![place(0, 0, Some(100_000)), place(1, 100_000, None)],
            },
            GenSubframe {
                fft: FftSize::K32,
                guard: 2048,
                symbols: 9,
                reduced_carriers: 4,
                pilots: (12, 2),
                pilot_boost: 4,
                frequency_interleaver: false,
                boundary: (true, true),
                plps: vec![place(0, 0, None)],
            },
        ],
        ..Default::default()
    };
    // A frame is 0.1318 s: 7 whole frames in 1 s.
    let x = common::generate(Atsc3Generator::new(p).unwrap(), 1.0);
    for (plp, id, per_frame) in [(-1, 0.0, 50.0), (1, 1.0, 20.0)] {
        let mut rx = receiver_of_plp(plp);
        let r = common::run(rx.as_mut(), rate, &x, 65536, "");
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Locked,
            "plp {plp}"
        );
        assert_eq!(r.f("subframes"), 3.0);
        assert_eq!(r.f("plp_id"), id, "plp {plp}");
        assert!(r.f("frames_ok") >= 6.0, "plp {plp}: {}", r.f("frames_ok"));
        assert_eq!(r.f("frames_failed"), 0.0, "plp {plp}");
        assert_eq!(r.f("baseband_packets_bad"), 0.0, "plp {plp}");
        // The lines of 512 rows fill in the first frame.
        let packets = r.f("baseband_packets") / (r.f("frames_ok") - 1.0);
        assert!(packets >= per_frame, "plp {plp}: {packets} packets a frame");
        assert_eq!(r.f("fec_blocks"), r.f("fec_blocks_good"));
    }
}

/// The share of one core that each stage takes on 3 s of the test signal at 8 Msps with a time interleaver.
#[test]
#[ignore = "a measurement: run it alone in an optimised build"]
fn the_share_of_each_stage_with_a_time_interleaver() {
    use sdrkit_stack_tvterr::atsc3::gen::GenTi;
    let rate = DESCRIPTOR.tuning.rate_hz;
    let seconds = 3.0;
    for time_interleaver in [
        GenTi::None,
        GenTi::Convolutional {
            depth: 3,
            extended: false,
        },
        GenTi::Convolutional {
            depth: 3,
            extended: true,
        },
        GenTi::Hybrid {
            inter_subframe: true,
            ti_blocks: 4,
            cell_interleaver: true,
            spare: 0,
        },
        GenTi::Hybrid {
            inter_subframe: false,
            ti_blocks: 4,
            cell_interleaver: true,
            spare: 2,
        },
        GenTi::None,
    ] {
        let p = Atsc3GenParams {
            rate_hz: rate,
            time_interleaver,
            ..Default::default()
        };
        let x = common::generate(Atsc3Generator::new(p).unwrap(), seconds);
        let settings = Values::defaults(&(DESCRIPTOR.settings)());
        let mut rx = sdrkit_stack_tvterr::atsc3::receiver(&sdrkit_core::StackConfig::new(settings))
            .expect("the receiver builds");
        sdrkit_core::Receiver::configure(&mut rx, &InputInfo::new(rate, 539e6, 539e6).recording())
            .expect("the rate");
        let mut out = Output::default();
        for c in x.chunks(16_384) {
            sdrkit_core::Receiver::feed(&mut rx, c, &mut out);
            out.clear();
        }
        let st = sdrkit_core::Receiver::staged(&mut rx).expect("stages");
        st.drain(&mut out);
        let mut loads = Vec::new();
        st.stage_loads(&mut loads);
        let shares: Vec<(String, f64)> = loads
            .iter()
            .map(|l| (l.name.clone(), l.busy_ns as f64 / 1e9 / seconds))
            .collect();
        let r = Owner::results(&rx);
        eprintln!(
            "{time_interleaver:?}: share per stage {shares:.3?}, packets {:?}, bad {:?}",
            r.fields()
                .at(&sdrkit_stack_tvterr::typed::Atsc3FrameFields::BASEBAND_PACKETS),
            r.fields()
                .at(&sdrkit_stack_tvterr::typed::Atsc3FrameFields::BASEBAND_PACKETS_BAD),
        );
    }
}

/// A test source: passes the baseband packets of its queue to the graph, four in a pass.
struct Bbps {
    packets: Arc<Mutex<VecDeque<Vec<u8>>>>,
    at: usize,
    pool: PacketPool,
}

impl Component for Bbps {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.bbp", vec![Port::iq("in")], vec![Port::bytes("out")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Bytes, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.iq(0).len();
        for _ in 0..n.min(4) {
            let Some(p) = self.packets.lock().expect("the queue").pop_front() else {
                return;
            };
            self.at += 1;
            let t = self.at as f64 * 1e-3;
            io.bytes_out(0)
                .push(self.pool.make(p.len(), t, |b| b.copy_from_slice(&p)));
        }
    }
}

fn full(version: u8, flags: u32, rest: &[u8]) -> Vec<u8> {
    let mut v = vec![
        version,
        (flags >> 16) as u8,
        (flags >> 8) as u8,
        flags as u8,
    ];
    v.extend_from_slice(rest);
    v
}

/// An HEVC init segment of track 1 at 90 kHz, `trex` default duration 3600.
fn init() -> Vec<u8> {
    let mut out = Vec::new();
    write_box(&mut out, b"ftyp", b"iso6\0\0\0\0iso6dash");
    let mut tkhd = vec![0u8; 80];
    tkhd[12..16].copy_from_slice(&1u32.to_be_bytes());
    let mut mdhd = vec![0u8; 24];
    mdhd[12..16].copy_from_slice(&90_000u32.to_be_bytes());
    let mut se = vec![0u8; 78];
    write_box(&mut se, b"hvcC", &[1, 2, 3]);
    let mut stsd = full(0, 0, &1u32.to_be_bytes());
    write_box(&mut stsd, b"hvc1", &se);
    let mut stbl = Vec::new();
    write_box(&mut stbl, b"stsd", &stsd);
    let mut minf = Vec::new();
    write_box(&mut minf, b"stbl", &stbl);
    let mut mdia = Vec::new();
    write_box(&mut mdia, b"mdhd", &mdhd);
    write_box(&mut mdia, b"minf", &minf);
    let mut trak = Vec::new();
    write_box(&mut trak, b"tkhd", &tkhd);
    write_box(&mut trak, b"mdia", &mdia);
    let mut trex = full(0, 0, &1u32.to_be_bytes());
    trex.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0x0E, 0x10, 0, 0, 0, 0, 0, 1, 0, 0]);
    let mut mvex = Vec::new();
    write_box(&mut mvex, b"trex", &trex);
    let mut moov = Vec::new();
    write_box(&mut moov, b"trak", &trak);
    write_box(&mut moov, b"mvex", &mvex);
    write_box(&mut out, b"moov", &moov);
    out
}

/// A media segment of two samples (3 and 2 bytes) at decoding time `dts`.
fn media(dts: u64) -> Vec<u8> {
    let mut seg = Vec::new();
    write_box(&mut seg, b"styp", b"msdh\0\0\0\0msdh");
    let tfhd = full(0, 0x020000, &1u32.to_be_bytes());
    let tfdt = full(1, 0, &dts.to_be_bytes());
    let mut trun = full(0, 0x01 | 0x04 | 0x200, &2u32.to_be_bytes());
    let off_at = trun.len();
    trun.extend_from_slice(&0u32.to_be_bytes());
    trun.extend_from_slice(&0u32.to_be_bytes());
    trun.extend_from_slice(&3u32.to_be_bytes());
    trun.extend_from_slice(&2u32.to_be_bytes());
    let build = |trun: &[u8]| {
        let mut traf = Vec::new();
        write_box(&mut traf, b"tfhd", &tfhd);
        write_box(&mut traf, b"tfdt", &tfdt);
        write_box(&mut traf, b"trun", trun);
        let mut moof_body = Vec::new();
        write_box(&mut moof_body, b"traf", &traf);
        let mut moof = Vec::new();
        write_box(&mut moof, b"moof", &moof_body);
        moof
    };
    let moof_len = build(&trun).len();
    let moof_at = seg.len();
    let data_at = moof_at + moof_len + 8;
    trun[off_at..off_at + 4].copy_from_slice(&((data_at - moof_at) as u32).to_be_bytes());
    seg.extend(build(&trun));
    write_box(&mut seg, b"mdat", &[0xA, 0xB, 0xC, 0xD, 0xE]);
    seg
}

/// The baseband packets of a service list and one ROUTE service (init and two media segments), in IPv4 and UDP and ALP. With
/// `bad_checksum`, the IPv4 header checksum of the LLS packet is wrong.
fn delivery_packets(bad_checksum: bool) -> Vec<Vec<u8>> {
    delivery_packets_with(bad_checksum, false, 0, 0, false)
}

/// As [`delivery_packets`]; with `signed` the service list and a system time come in a signed multi-table, `sls` signalling
/// objects go to TSI 0 of the service's session (and as many to another session), and `compressed` ALP packets of compressed IP
/// follow. With `signed_package` each signalling object is a signed package of two documents.
fn delivery_packets_with(
    bad_checksum: bool,
    signed: bool,
    sls: u32,
    compressed: usize,
    signed_package: bool,
) -> Vec<Vec<u8>> {
    let xml = r#"<SLT bsid="1"><Service serviceId="1001" majorChannelNo="7" minorChannelNo="1" serviceCategory="1" shortServiceName="TEST"><BroadcastSvcSignaling slsProtocol="1" slsDestinationIpAddress="239.255.1.1" slsDestinationUdpPort="5000" slsSourceIpAddress="10.1.1.1"/></Service></SLT>"#;
    let src = Ipv4Addr::new(10, 1, 1, 1);
    let mut alp = Vec::new();
    let mut ip = Vec::new();
    let mut id = 0u16;
    let mut send = |dst: Ipv4Addr, port: u16, payload: &[u8], corrupt: bool, alp: &mut Vec<u8>| {
        ip.clear();
        id += 1;
        let mut ranges = Vec::new();
        write_ipv4_udp(&mut ip, src, dst, id, 1500, 1, port, payload, |r| {
            ranges.push(r)
        })
        .unwrap_or_else(|e| panic!("{e}"));
        for r in ranges {
            let mut pkt = ip[r].to_vec();
            if corrupt {
                pkt[10] ^= 0x55;
            }
            write_single(PacketType::Ipv4, None, &pkt, alp).unwrap_or_else(|e| panic!("{e}"));
        }
    };
    let time = r#"<SystemTime currentUtcOffset="37" utcLocalOffset="-PT5H"/>"#;
    let lls = if signed {
        build_signed_lls(0, 1, &[(1, 1, xml), (3, 1, time)], &[0x5A; 32])
    } else {
        build_lls(1, 0, 1, xml)
    };
    send(
        Ipv4Addr::from(LLS_ADDR),
        LLS_PORT,
        &lls,
        bad_checksum,
        &mut alp,
    );
    let service = Ipv4Addr::new(239, 255, 1, 1);
    let other = Ipv4Addr::new(239, 255, 1, 9);
    // The signature part holds filler bytes: the receiver does not check it.
    let sls_object = if signed_package {
        b"Content-Type: multipart/signed; boundary=s\r\n\r\n--s\r\nContent-Type: multipart/related; boundary=b\r\n\r\n\
--b\r\nContent-Type: application/route-usd+xml\r\n\r\n<BundleDescription/>\r\n\
--b\r\nContent-Type: application/route-s-tsid+xml\r\n\r\n<S-TSID/>\r\n--b--\r\n\
--s\r\nContent-Type: application/pkcs7-signature\r\n\r\n\x30\x00\xA5\r\n--s--\r\n"
            .to_vec()
    } else {
        b"--b\r\nContent-Type: application/route-usd+xml\r\n\r\n<BundleDescription/>\r\n--b--\r\n"
            .to_vec()
    };
    // The session description of the service names the channel of its video. It goes to the session of the service only.
    let stsid = b"--b\r\nContent-Type: application/route-s-tsid+xml\r\n\r\n\
<S-TSID><RS><LS tsi=\"2\"><SrcFlow rt=\"true\"/></LS></RS></S-TSID>\r\n--b--\r\n"
        .to_vec();
    let objects = [
        (0u32, 1u32 << 17, stsid),
        (2, 1, init()),
        (2, 2, media(0)),
        (2, 3, media(7200)),
    ]
    .into_iter()
    .chain((0..sls).map(|k| (0, 100 + k, sls_object.clone())));
    for (tsi, toi, obj) in objects {
        // The signalling objects that the test counts go to another session too.
        for (dst, copy) in [(service, true), (other, tsi == 0 && toi != 1 << 17)] {
            if !copy {
                continue;
            }
            for (k, c) in obj.chunks(500).enumerate() {
                let mut p = Vec::new();
                LctBuilder {
                    tsi,
                    toi,
                    codepoint: 0,
                    transfer_len: Some(obj.len() as u64),
                    close_object: false,
                }
                .write(&mut p)
                .unwrap_or_else(|e| panic!("{e}"));
                p.extend_from_slice(&((k * 500) as u32).to_be_bytes());
                p.extend_from_slice(c);
                send(dst, 5000, &p, false, &mut alp);
            }
        }
    }
    for _ in 0..compressed {
        write_single(PacketType::CompressedIp, None, &[0xFD; 24], &mut alp)
            .unwrap_or_else(|e| panic!("{e}"));
    }
    alp.chunks(1000)
        .enumerate()
        .map(|(k, c)| {
            let mut bb = Vec::new();
            write_bbp_header(if k == 0 { 0 } else { 8191 }, BbpExtension::None, &mut bb)
                .unwrap_or_else(|e| panic!("{e}"));
            bb.extend_from_slice(c);
            bb
        })
        .collect()
}

/// The delivery chain of the receiver graph, fed by a test source instead of the frame decoder, with a tap on the media
/// packets.
struct Chain {
    g: Graph,
    queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    tap: TapReader,
    /// The media packets that the chain gave.
    packets: Vec<MediaPacket>,
    /// Those that went to the host, by their stream kind.
    host: Vec<StreamKind>,
}

impl Chain {
    fn new() -> Chain {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let b = GraphBuilder::new()
            .add(
                "source",
                Bbps {
                    packets: Arc::clone(&queue),
                    at: 0,
                    pool: PacketPool::default(),
                },
            )
            .add("report", results::Report::default())
            .input("source.in")
            .status_from("report")
            .command_alias("service.select", CommandTarget::every("service.select"));
        let mut g = delivery(b, "source.out")
            .build()
            .unwrap_or_else(|e| panic!("{e}"));
        g.declare(&results::declaration());
        g.configure_input(Format::iq(8e6, 539e6, 539e6))
            .unwrap_or_else(|e| panic!("{e}"));
        let tap = g.tap("media.out", 1 << 12).unwrap_or_else(|e| panic!("{e}"));
        Chain {
            g,
            queue,
            tap,
            packets: Vec::new(),
            host: Vec::new(),
        }
    }

    /// Passes `packets` through the chain.
    fn feed(&mut self, packets: Vec<Vec<u8>>) {
        let passes = packets.len() / 4 + 8;
        self.queue.lock().expect("the queue").extend(packets);
        let mut out = Output::default();
        let mut buf = Buffer::new(&Kind::typed::<MediaPacket>());
        for _ in 0..passes {
            self.g.process(&[Complex32::new(0.0, 0.0); 64], &mut out);
            self.host.extend(out.media.iter().map(|m| m.kind));
            out.clear();
            while self.tap.pop(&mut buf) {
                if let Data::Items(Some(v)) = &buf.data {
                    let items = v
                        .as_any()
                        .downcast_ref::<Vec<Timed<MediaPacket>>>()
                        .expect("media packets");
                    self.packets.extend(items.iter().map(|p| p.value.clone()));
                }
            }
        }
    }

    /// Runs `service.select` with the row `row`.
    fn select(&mut self, row: &str) -> bool {
        let id = self.g.commands().id("service.select").expect("the command");
        let def = self.g.commands().def(id).expect("the command").clone();
        let mut record = def.new_args();
        let arg = def.args.id("row").expect("the argument");
        record
            .set_value(sdrkit_core::FieldId(arg.0), &sdrkit_core::Value::Text(row.into()))
            .expect("a text");
        sdrkit_core::owner::invoke(&mut self.g, id, &record).is_ok()
    }

    fn results(&mut self) -> Results {
        self.g.refresh_results();
        Owner::results(&self.g).clone()
    }

    /// The packets of `codec`.
    fn of(&self, codec: Codec) -> Vec<&MediaPacket> {
        self.packets
            .iter()
            .filter(|p| p.format.codec == codec)
            .collect()
    }
}

fn run_delivery(packets: Vec<Vec<u8>>) -> Results {
    let mut c = Chain::new();
    c.feed(packets);
    let mut out = Output::default();
    c.g.finish(&mut out);
    c.results()
}

/// The row `key` of table `table` of `r`.
fn row_of(r: &Results, table: &str, key: &str) -> sdrkit_core::Record {
    r.decl()
        .table_id(table)
        .and_then(|id| r.table(id))
        .and_then(|t| t.row(key))
        .unwrap_or_else(|| panic!("no row {key} in {table}"))
        .clone()
}

/// Checks the video packets from access unit `first` of the test signal on: the bytes, the times in ticks of 90 kHz
/// (decoding every 3600, presentation 3600 later), a sync sample and clean start at the head of each segment, and the
/// format.
fn check_video(video: &[&MediaPacket], first: i64) {
    for (i, p) in video.iter().enumerate() {
        let n = first + i as i64;
        let (segment, sample) = ((n / 2) as u64, (n % 2) as u32);
        assert_eq!(p.packet.dts, Some(n * 3600), "video packet {i}");
        assert_eq!(p.packet.pts, Some(n * 3600 + 3600), "video packet {i}");
        assert_eq!((p.packet.keyframe, p.packet.clean_start), (sample == 0, sample == 0));
        assert_eq!(p.packet.bytes(), &testsvc::video_sample(segment, sample)[..]);
        let f = &p.format;
        assert_eq!((f.codec, f.packaging), (Codec::Hevc, Packaging::LengthPrefixed(4)));
        assert_eq!(f.timebase, (1, 90_000));
        assert_eq!(&f.config[..], &testsvc::video_config()[..]);
        let v = f.video.as_ref().expect("video properties");
        assert_eq!((v.width, v.height), (1280, 720));
        assert_eq!(p.stream_kind(), Some(StreamKind::Hevc));
    }
}

/// Checks the audio packets from access unit `first` of the test signal on: the bytes, the times in ticks of 48 kHz
/// (every 1920), and the format.
fn check_audio(audio: &[&MediaPacket], first: i64) {
    for (i, p) in audio.iter().enumerate() {
        let n = first + i as i64;
        let (segment, sample) = ((n / 2) as u64, (n % 2) as u32);
        assert_eq!(p.packet.dts, Some(n * 1920), "audio packet {i}");
        assert_eq!(p.packet.pts, Some(n * 1920), "audio packet {i}");
        assert!(p.packet.keyframe && p.packet.clean_start);
        assert_eq!(p.packet.bytes(), &testsvc::audio_sample(segment, sample)[..]);
        let f = &p.format;
        assert_eq!((f.codec, f.packaging), (Codec::Ac4, Packaging::Raw));
        assert_eq!(f.timebase, (1, 48_000));
        assert_eq!(&f.config[..], &testsvc::AUDIO_CONFIG[..]);
        assert_eq!(f.audio, Some(AudioProps { sample_rate: 48_000, channels: 2 }));
    }
}

/// The payload of the test signal through the delivery chain: the service list, and the video and the audio of the
/// service as media packets with the bytes, times and formats of the access units that the generator made.
#[test]
fn the_service_of_the_test_signal_gives_media_packets() {
    let mut payload = Payload::new(5);
    let mut c = Chain::new();
    c.feed((0..150).map(|_| payload.packet(1000)).collect());
    let (video, audio) = (c.of(Codec::Hevc), c.of(Codec::Ac4));
    // 150 baseband packets hold five rounds of the service, each with two access units of video and of audio.
    assert_eq!((video.len(), audio.len()), (10, 10));
    assert_eq!(c.packets.len(), 20);
    check_video(&video, 0);
    check_audio(&audio, 0);
    // The host takes the video. The shared media form names AC-4, and no host decoder takes it.
    assert_eq!(c.host, vec![StreamKind::Hevc; 10]);
    let r = c.results();
    let f = r.fields();
    assert_eq!(f.at(&F::SERVICE_RECEIVED), Some(i64::from(testsvc::SERVICE_ID)));
    assert_eq!((f.at(&F::VIDEO_PACKETS), f.at(&F::AUDIO_PACKETS)), (Some(10), Some(10)));
    assert_eq!(f.at(&F::MEDIA_SAMPLES), Some(20));
    let row = row_of(&r, ATSC3_SERVICES.key, "1001");
    assert_eq!(row.at(&S::NAME), Some(testsvc::SERVICE_NAME));
    assert_eq!((row.at(&S::MAJOR), row.at(&S::MINOR)), (Some(39), Some(1)));
    assert_eq!(row.at(&S::CATEGORY), Some(results::ServiceCategory::Tv));
    assert_eq!(row.at(&S::DELIVERY), Some(results::Delivery::Route));
    assert_eq!(row.at(&S::ACTION), Some(results::ServiceAction::Receiving));
    assert_eq!(row.at(&S::PROTECTED), Some(false));
    // Each round has the file description and the package of four documents. The file description of a sixth round is in.
    assert_eq!(row.at(&S::SLS_OBJECTS), Some(11));
    assert_eq!(row.at(&S::SLS_DOCUMENTS), Some(26));
    let v = row_of(&r, ATSC3_COMPONENTS.key, "239.255.10.2:5002/1");
    assert_eq!((v.at(&C::FOURCC), v.at(&C::TSI)), (Some("hev1"), Some(1)));
    assert_eq!(v.at(&C::DESTINATION), Some("239.255.10.2:5002"));
    assert_eq!(v.at(&C::KIND), Some(ComponentKind::Video));
    assert_eq!(v.at(&C::STATE), Some(ComponentState::Given));
    assert_eq!(v.at(&C::SERVICE), Some(1001));
    let a = row_of(&r, ATSC3_COMPONENTS.key, "239.255.10.2:5002/2");
    assert_eq!(a.at(&C::FOURCC), Some("ac-4"));
    assert_eq!(a.at(&C::KIND), Some(ComponentKind::Audio));
    assert_eq!(a.at(&C::STATE), Some(ComponentState::Given));
}

/// The baseband packets of `datagrams`, in IPv4 and UDP and ALP.
fn baseband(datagrams: impl IntoIterator<Item = testsvc::Datagram>) -> Vec<Vec<u8>> {
    let mut alp = Vec::new();
    let mut ip = Vec::new();
    for (id, d) in datagrams.into_iter().enumerate() {
        ip.clear();
        let mut ranges = Vec::new();
        write_ipv4_udp(&mut ip, testsvc::SOURCE, d.dst, id as u16, 1500, 1, d.port, &d.payload, |r| {
            ranges.push(r)
        })
        .unwrap_or_else(|e| panic!("{e}"));
        for r in ranges {
            write_single(PacketType::Ipv4, None, &ip[r], &mut alp).unwrap_or_else(|e| panic!("{e}"));
        }
    }
    alp.chunks(1000)
        .enumerate()
        .map(|(k, c)| {
            let mut bb = Vec::new();
            write_bbp_header(if k == 0 { 0 } else { 8191 }, BbpExtension::None, &mut bb)
                .unwrap_or_else(|e| panic!("{e}"));
            bb.extend_from_slice(c);
            bb
        })
        .collect()
}

/// The datagrams of round `segment` of the service of the test signal without its service list.
fn round_without_list(segment: u64) -> VecDeque<testsvc::Datagram> {
    let mut d = VecDeque::new();
    testsvc::round(segment, &mut d);
    d.pop_front();
    d
}

/// A second service, 1002, in its own ROUTE session: its session description names three channels, an encrypted video
/// track, an MPEG-H audio track and a channel without an init segment. `service.select` moves the media from one service
/// to the other, a service with nothing to give says why, and a row that is not in the list changes nothing.
#[test]
fn the_chosen_service_is_given_and_an_encrypted_one_is_not() {
    let other = Ipv4Addr::new(239, 255, 10, 3);
    let slt = testsvc::slt().replace(
        "</SLT>",
        "<Service serviceId=\"1002\" sltSvcSeqNum=\"0\" protected=\"true\" majorChannelNo=\"39\" minorChannelNo=\"2\" \
serviceCategory=\"1\" shortServiceName=\"TEST 2\"><BroadcastSvcSignaling slsProtocol=\"1\" \
slsDestinationIpAddress=\"239.255.10.3\" slsDestinationUdpPort=\"5003\" slsSourceIpAddress=\"10.0.0.1\"/></Service>\
<Service serviceId=\"1003\" sltSvcSeqNum=\"0\" majorChannelNo=\"39\" minorChannelNo=\"3\" serviceCategory=\"1\" \
shortServiceName=\"TEST 3\"><BroadcastSvcSignaling slsProtocol=\"2\" slsDestinationIpAddress=\"239.255.10.4\" \
slsDestinationUdpPort=\"5004\"/></Service></SLT>",
    );
    let list = testsvc::Datagram {
        dst: Ipv4Addr::from(LLS_ADDR),
        port: LLS_PORT,
        payload: build_lls(1, 0, 2, &slt),
    };
    // The objects of service 1002, sent to its own session.
    let rename = |mut file: Vec<u8>, from: &[u8; 4], to: &[u8; 4]| {
        let at = file.windows(4).position(|w| w == from).expect("the sample entry");
        file[at..at + 4].copy_from_slice(to);
        file
    };
    let stsid = b"--b\r\nContent-Type: application/route-s-tsid+xml\r\n\r\n<S-TSID><RS>\
<LS tsi=\"1\"><SrcFlow rt=\"true\"/></LS><LS tsi=\"2\"><SrcFlow rt=\"true\"/></LS><LS tsi=\"3\"><SrcFlow rt=\"true\"/></LS>\
</RS></S-TSID>\r\n--b--\r\n";
    let second = |segment: u64| {
        let mut d = VecDeque::new();
        testsvc::route_packets(0, 1 << 17 | 1, 3, stsid, &mut d);
        testsvc::route_packets(1, 1, 5, &rename(testsvc::video_init(), b"hev1", b"encv"), &mut d);
        testsvc::route_packets(2, 1, 5, &rename(testsvc::audio_init(), b"ac-4", b"mhm1"), &mut d);
        testsvc::route_packets(1, 2 + segment as u32, 8, &testsvc::video_segment(segment), &mut d);
        testsvc::route_packets(2, 2 + segment as u32, 8, &testsvc::audio_segment(segment), &mut d);
        for x in &mut d {
            (x.dst, x.port) = (other, 5003);
        }
        d
    };
    let mut c = Chain::new();
    // Before a service list is read no row can be chosen.
    assert!(!c.select("1001"));
    let first = std::iter::once(list)
        .chain(round_without_list(0))
        .chain(second(0));
    c.feed(baseband(first));
    // Without a choice the first visible TV service by ROUTE is given.
    assert_eq!((c.of(Codec::Hevc).len(), c.of(Codec::Ac4).len()), (2, 2));
    let r = c.results();
    assert_eq!(r.fields().at(&F::SERVICE), None);
    assert_eq!(r.fields().at(&F::SERVICE_RECEIVED), Some(1001));
    let action = |r: &Results, key: &str| row_of(r, ATSC3_SERVICES.key, key).at(&S::ACTION);
    assert_eq!(action(&r, "1001"), Some(results::ServiceAction::Receiving));
    assert_eq!(action(&r, "1002"), Some(results::ServiceAction::Receive));
    assert_eq!(action(&r, "1003"), Some(results::ServiceAction::Unsupported));
    assert_eq!(row_of(&r, ATSC3_SERVICES.key, "1002").at(&S::PROTECTED), Some(true));
    let state = |r: &Results, key: &str| {
        let row = row_of(r, ATSC3_COMPONENTS.key, key);
        (row.at(&C::STATE), row.at(&C::SERVICE), row.at(&C::FOURCC).map(String::from))
    };
    assert_eq!(
        state(&r, "239.255.10.3:5003/1"),
        (Some(ComponentState::Encrypted), Some(1002), Some("encv".into()))
    );
    assert_eq!(
        state(&r, "239.255.10.3:5003/2"),
        (Some(ComponentState::NoFormat), Some(1002), Some("mhm1".into()))
    );
    // A row that is not in the list is refused and the service stays.
    assert!(!c.select("7"));
    // Service 1002 is chosen: nothing of it can be given, and nothing of 1001 comes any more.
    assert!(c.select("1002"));
    c.feed(baseband(round_without_list(1).into_iter().chain(second(1))));
    assert_eq!(c.packets.len(), 4);
    let r = c.results();
    assert_eq!(r.fields().at(&F::SERVICE), Some(1002));
    assert_eq!(r.fields().at(&F::SERVICE_RECEIVED), Some(1002));
    assert_eq!(action(&r, "1001"), Some(results::ServiceAction::Receive));
    assert_eq!(action(&r, "1002"), Some(results::ServiceAction::Encrypted));
    assert_eq!(
        state(&r, "239.255.10.2:5002/1").0,
        Some(ComponentState::OtherService)
    );
    // Back to service 1001: its next segment is given.
    assert!(c.select("1001"));
    c.feed(baseband(round_without_list(2)));
    let video = c.of(Codec::Hevc);
    assert_eq!(video.len(), 4);
    check_video(&video[2..], 4);
    check_audio(&c.of(Codec::Ac4)[2..], 4);
    let r = c.results();
    assert_eq!(action(&r, "1001"), Some(results::ServiceAction::Receiving));
    assert_eq!(action(&r, "1002"), Some(results::ServiceAction::Receive));
    // The automatic choice again.
    assert!(c.select(""));
    assert_eq!(c.results().fields().at(&F::SERVICE), None);
}

/// A service whose session description gives code point 128 the entity mode: the init segment and the media segment
/// come behind header lines and are read after them.
#[test]
fn objects_in_entity_mode_are_read_after_their_header_lines() {
    let stsid = b"--b\r\nContent-Type: application/route-s-tsid+xml\r\n\r\n<S-TSID><RS><LS tsi=\"1\"><SrcFlow rt=\"true\">\
<Payload codePoint=\"128\" formatId=\"2\" frag=\"0\" order=\"true\"/></SrcFlow></LS></RS></S-TSID>\r\n--b--\r\n";
    let entity = |file: Vec<u8>| {
        let mut object =
            format!("Content-Type: video/mp4\r\nContent-Length: {}\r\n\r\n", file.len()).into_bytes();
        object.extend(file);
        object
    };
    let mut d = VecDeque::new();
    d.push_back(testsvc::Datagram {
        dst: Ipv4Addr::from(LLS_ADDR),
        port: LLS_PORT,
        payload: testsvc::lls().to_vec(),
    });
    testsvc::route_packets(0, 1 << 17 | 1, 3, stsid, &mut d);
    testsvc::route_packets(1, 1, 128, &entity(testsvc::video_init()), &mut d);
    testsvc::route_packets(1, 2, 128, &entity(testsvc::video_segment(0)), &mut d);
    // The same segment on a code point without the entity mode is no segment: its first bytes are header lines.
    testsvc::route_packets(1, 3, 8, &entity(testsvc::video_segment(1)), &mut d);
    let mut c = Chain::new();
    c.feed(baseband(d));
    let video = c.of(Codec::Hevc);
    assert_eq!(video.len(), 2);
    check_video(&video, 0);
}

/// One second of the test signal through the stack: the service list, and the video of the service at the host with
/// the times of the access units in order, the audio at the output of the graph.
#[test]
fn the_test_signal_gives_the_service_list_and_media_packets() {
    let rate = DESCRIPTOR.tuning.rate_hz;
    let x = common::generate(
        Atsc3Generator::new(Atsc3GenParams {
            rate_hz: rate,
            ..Default::default()
        })
        .unwrap(),
        1.0,
    );
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    rx.configure(&InputInfo::new(rate, 539e6, 539e6).recording())
        .expect("rate accepted");
    let mut tap = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph")
        .tap("media.out", 1 << 12)
        .expect("the media output");
    let mut out = Output::default();
    let mut buf = Buffer::new(&Kind::typed::<MediaPacket>());
    let mut packets = Vec::new();
    let mut host = Vec::new();
    for c in x.chunks(65536) {
        rx.feed(c, &mut out);
        host.append(&mut out.media);
        out.clear();
        while tap.pop(&mut buf) {
            if let Data::Items(Some(v)) = &buf.data {
                let items = v
                    .as_any()
                    .downcast_ref::<Vec<Timed<MediaPacket>>>()
                    .expect("media packets");
                packets.extend(items.iter().map(|p| p.value.clone()));
            }
        }
    }
    let of = |codec: Codec| -> Vec<&MediaPacket> {
        packets.iter().filter(|p| p.format.codec == codec).collect()
    };
    let (video, audio) = (of(Codec::Hevc), of(Codec::Ac4));
    assert!(video.len() >= 16 && audio.len() >= 16, "{} video, {} audio", video.len(), audio.len());
    // The first packet is the head of a segment: the access units from there on come without a gap.
    let first = video[0].packet.dts.expect("a decoding time") / 3600;
    assert_eq!(first % 2, 0);
    check_video(&video, first);
    let first = audio[0].packet.dts.expect("a decoding time") / 1920;
    assert_eq!(first % 2, 0);
    check_audio(&audio, first);
    // The host has the video packets, each with the format and the times.
    assert_eq!(host.len(), video.len());
    for (h, v) in host.iter().zip(&video) {
        assert_eq!(h.kind, StreamKind::Hevc);
        assert_eq!((h.packet.pts, h.packet.dts), (v.packet.pts, v.packet.dts));
        assert_eq!(h.format.timebase, (1, 90_000));
    }
    let r = Owner::results(rx.as_ref()).clone();
    let f = r.fields();
    assert_eq!(f.at(&F::SERVICE_RECEIVED), Some(1001));
    assert_eq!(f.at(&F::VIDEO_PACKETS), Some(video.len() as i64));
    assert_eq!(f.at(&F::AUDIO_PACKETS), Some(audio.len() as i64));
    let row = row_of(&r, ATSC3_SERVICES.key, "1001");
    assert_eq!(row.at(&S::NAME), Some("TEST 1"));
    assert_eq!(row.at(&S::ACTION), Some(results::ServiceAction::Receiving));
    // The command of the stack reaches the report and the media component: a row of the list is taken, another refused.
    let select = |rx: &mut dyn sdrkit_core::Receiver, row: &str| {
        let id = rx.commands().id("service.select").expect("the command");
        let def = rx.commands().def(id).expect("the command").clone();
        let mut record = def.new_args();
        let arg = def.args.id("row").expect("the argument");
        record
            .set_value(sdrkit_core::FieldId(arg.0), &sdrkit_core::Value::Text(row.into()))
            .expect("a text");
        sdrkit_core::owner::invoke(rx, id, &record).is_ok()
    };
    assert!(select(rx.as_mut(), "1001"));
    assert!(!select(rx.as_mut(), "7"));
    let r = Owner::results(rx.as_ref()).clone();
    assert_eq!(r.fields().at(&F::SERVICE), Some(1001));
}

#[test]
fn the_delivery_chain_runs_in_the_receiver_graph() {
    let r = run_delivery(delivery_packets(false));
    let f = r.fields();
    assert_eq!(f.at(&F::LLS_TABLES), Some(1), "{f:?}");
    assert_eq!(f.at(&F::SERVICE_RECEIVED), Some(1001));
    assert_eq!(f.at(&F::ROUTE_OBJECTS), Some(4));
    assert_eq!(f.at(&F::MEDIA_SAMPLES), Some(4));
    let table = |key: &str| {
        r.decl()
            .table_id(key)
            .and_then(|id| r.table(id))
            .expect("a declared table")
    };
    let comp = table(ATSC3_COMPONENTS.key);
    assert_eq!(comp.len(), 1);
    let c = comp.row("239.255.1.1:5000/2").expect("a component");
    assert_eq!(c.at(&C::FOURCC), Some("hvc1"));
    assert_eq!(c.at(&C::STATE), Some(ComponentState::Given));
    let row = table(ATSC3_SERVICES.key).row("1001").expect("service 1001");
    assert_eq!(row.at(&S::MAJOR), Some(7));
    assert_eq!(row.at(&S::MINOR), Some(1));
    assert_eq!(row.at(&S::NAME), Some("TEST"));
    assert_eq!(row.at(&S::ACTION), Some(results::ServiceAction::Receiving));
}

#[test]
fn a_bad_ip_header_checksum_is_refused() {
    // The service list sent with a wrong IPv4 header checksum: a receiver must not read it.
    let r = run_delivery(delivery_packets(true));
    let f = r.fields();
    assert_eq!(f.at(&F::LLS_TABLES), Some(0), "{f:?}");
    assert_eq!(
        f.at(&F::ROUTE_OBJECTS),
        Some(4),
        "the other packets still pass"
    );
}

#[test]
fn signed_signalling_sls_objects_and_compressed_ip_are_reported() {
    // Plain signalling, no signalling objects, no compressed IP.
    let r = run_delivery(delivery_packets(false));
    let f = r.fields();
    assert_eq!(f.at(&F::SIGNED_SIGNALLING), Some(false));
    assert_eq!(f.at(&F::LEAP_SECONDS), None);
    // The service list and the system time in a signed multi-table, 3 signalling objects for the service (and 3 to another
    // session), 5 ALP packets of compressed IP.
    let r = run_delivery(delivery_packets_with(false, true, 3, 5, false));
    let f = r.fields();
    assert_eq!(f.at(&F::LLS_TABLES), Some(2), "{f:?}");
    assert_eq!(f.at(&F::SIGNED_SIGNALLING), Some(true));
    assert_eq!(f.at(&F::LEAP_SECONDS), Some(37));
    assert_eq!(f.at(&F::SERVICE_RECEIVED), Some(1001));
    assert_eq!(
        f.at(&F::MEDIA_SAMPLES),
        Some(4),
        "signalling objects are not media"
    );
    let id = f.decl().id("alp.compressed_ip").expect("the ALP count");
    assert_eq!(f.value(id), Some(&sdrkit_core::Value::I64(5)));
    assert!(r
        .log_lines()
        .iter()
        .any(|l| l.ends_with(sdrkit_proto_ipcast::components::COMPRESSED_IP_NOTE)));
    let row = r
        .decl()
        .table_id(ATSC3_SERVICES.key)
        .and_then(|id| r.table(id))
        .and_then(|t| t.row("1001"))
        .expect("service 1001")
        .clone();
    // The three objects and the session description.
    assert_eq!(row.at(&S::SLS_OBJECTS), Some(4));
    assert_eq!(row.at(&S::SLS_DOCUMENTS), Some(4));
    assert_eq!(row.at(&S::SLS_SIGNED_PACKAGES), Some(0));
    // Plain low-level signalling, 2 signalling objects that are signed packages of two documents each: the station signs.
    let r = run_delivery(delivery_packets_with(false, false, 2, 0, true));
    assert_eq!(r.fields().at(&F::SIGNED_SIGNALLING), Some(true));
    let row = r
        .decl()
        .table_id(ATSC3_SERVICES.key)
        .and_then(|id| r.table(id))
        .and_then(|t| t.row("1001"))
        .expect("service 1001")
        .clone();
    assert_eq!(row.at(&S::SLS_OBJECTS), Some(3));
    assert_eq!(row.at(&S::SLS_DOCUMENTS), Some(5));
    assert_eq!(row.at(&S::SLS_SIGNED_PACKAGES), Some(2));
}

/// The chosen service, the received service and the service of a component name a row of the stack's `services` table.
#[test]
fn the_service_fields_refer_to_the_services_table() {
    let decl = sdrkit_stack_tvterr::typed::shape("atsc3").results_decl();
    let table = |key: &str| {
        decl.tables()
            .iter()
            .find(|t| t.key == key)
            .unwrap_or_else(|| panic!("no table {key}"))
            .columns
            .clone()
    };
    let components = table("components");
    let fields = [
        (decl.fields().clone(), "service"),
        (decl.fields().clone(), "service_received"),
        (components, "service"),
    ];
    for (record, key) in &fields {
        let d = record.id(key).and_then(|id| record.field(id)).expect("declared");
        let r = d.refers_to.as_ref().expect("a reference");
        assert_eq!(
            (r.owner.is_none(), &*r.table, &*r.column),
            (true, "services", ""),
            "{key}"
        );
    }
    table("services");
}
