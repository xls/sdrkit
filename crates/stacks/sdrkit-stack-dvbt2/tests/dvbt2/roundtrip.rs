//! Round trips: the generator's transmitter into the receiver, clean and with noise, in several frame shapes.

use super::common::*;
use sdrkit_stack_dvbt2::params::*;
use sdrkit_stack_dvbt2::rx::{RxSettings, Stage};
use sdrkit_stack_dvbt2::tx::Transmitter;

#[test]
fn clean_2k_16qam_short_frame() {
    let cfg = single(
        Fft::K2,
        Guard::G1_8,
        PilotPattern::Pp3,
        coding(FecFrame::Short, CodeRate::R1_2, Modulation::Qam16, false),
        4,
    );
    let mut tx = Transmitter::new(cfg).expect("transmitter");
    let x = signal(&mut tx, 6, 3000);
    let (st, check, frames) = receive(&x, RxSettings::default(), 8192, 0);
    eprintln!(
        "{:?} frames {frames} check {check:?} log {:?}",
        st.stage, st.log
    );
    assert_eq!(st.stage, Stage::Locked);
    assert!(
        check.good > 50 && check.wrong == 0 && check.flagged == 0,
        "{check:?}"
    );
}

use sdrkit_core::Complex32;
use sdrkit_proto_mpegts::source::{Fill, Numbered, NumberedSource};
use sdrkit_stack_dvbt2::tx::{PlpKind, PlpSpec, TimeIl, TxConfig};

struct Case {
    fft: Fft,
    guard: Guard,
    pp: PilotPattern,
    ext: bool,
    c: (FecFrame, CodeRate, Modulation, bool),
    blocks: u16,
    l1: Modulation,
}

fn run_case(k: &Case) {
    let mut cfg = single(
        k.fft,
        k.guard,
        k.pp,
        coding(k.c.0, k.c.1, k.c.2, k.c.3),
        k.blocks,
    );
    cfg.shape.extended = k.ext;
    cfg.l1_mod = k.l1;
    let mut tx = Transmitter::new(cfg).unwrap_or_else(|e| panic!("{:?} {:?}", k.fft, e));
    let x = signal(&mut tx, 4, 5000);
    let (st, check, _) = receive(&x, RxSettings::default(), 50_000, 0);
    assert_eq!(
        st.stage,
        Stage::Locked,
        "{} {} {} ext {} {:?}: {:?}",
        k.fft.name(),
        k.guard.name(),
        k.pp.name(),
        k.ext,
        k.c,
        st.log
    );
    assert!(
        check.good > 0 && check.wrong == 0 && check.flagged == 0 && check.gaps == 0,
        "{} {} {}: {check:?}",
        k.fft.name(),
        k.guard.name(),
        k.pp.name()
    );
}

#[test]
fn every_fft_size_and_carrier_mode() {
    use CodeRate::*;
    use FecFrame::*;
    use Modulation::*;
    let cases = [
        Case {
            fft: Fft::K1,
            guard: Guard::G1_16,
            pp: PilotPattern::Pp4,
            ext: false,
            c: (Short, R2_3, Qpsk, false),
            blocks: 2,
            l1: Bpsk,
        },
        Case {
            fft: Fft::K2,
            guard: Guard::G1_32,
            pp: PilotPattern::Pp7,
            ext: false,
            c: (Short, R3_4, Qam64, true),
            blocks: 3,
            l1: Qam16,
        },
        Case {
            fft: Fft::K4,
            guard: Guard::G1_4,
            pp: PilotPattern::Pp1,
            ext: false,
            c: (Short, R3_5, Qam256, true),
            blocks: 2,
            l1: Qam64,
        },
        Case {
            fft: Fft::K8,
            guard: Guard::G19_256,
            pp: PilotPattern::Pp8,
            ext: false,
            c: (Normal, R2_3, Qam16, false),
            blocks: 1,
            l1: Qpsk,
        },
        Case {
            fft: Fft::K8,
            guard: Guard::G1_8,
            pp: PilotPattern::Pp2,
            ext: true,
            c: (Normal, R5_6, Qam64, true),
            blocks: 1,
            l1: Qam16,
        },
        Case {
            fft: Fft::K16,
            guard: Guard::G1_16,
            pp: PilotPattern::Pp5,
            ext: true,
            c: (Normal, R4_5, Qam256, false),
            blocks: 2,
            l1: Qpsk,
        },
        Case {
            fft: Fft::K32,
            guard: Guard::G1_128,
            pp: PilotPattern::Pp7,
            ext: true,
            c: (Normal, R3_5, Qam256, true),
            blocks: 3,
            l1: Qpsk,
        },
        Case {
            fft: Fft::K32,
            guard: Guard::G19_128,
            pp: PilotPattern::Pp2,
            ext: false,
            c: (Normal, R1_2, Qpsk, false),
            blocks: 2,
            l1: Bpsk,
        },
    ];
    for k in &cases {
        run_case(k);
    }
}

fn plp(
    id: u8,
    kind: PlpKind,
    group: u8,
    c: (FecFrame, CodeRate, Modulation, bool),
    blocks: u16,
    time_il: TimeIl,
    pid: u16,
) -> PlpSpec {
    PlpSpec {
        id,
        kind,
        group,
        coding: coding(c.0, c.1, c.2, c.3),
        blocks,
        time_il,
        source: Box::new(NumberedSource::new(Numbered::new(pid, Fill::I31N7))),
    }
}

fn multi_config(miso: bool) -> TxConfig {
    use CodeRate::*;
    use FecFrame::*;
    use Modulation::*;
    let mut cfg = single(
        Fft::K8,
        Guard::G1_8,
        if miso {
            PilotPattern::Pp1
        } else {
            PilotPattern::Pp3
        },
        coding(Short, R1_2, Qpsk, false),
        1,
    );
    cfg.shape.miso = miso;
    cfg.num_t2_frames = 4;
    cfg.sub_slices = 4;
    cfg.plps = vec![
        plp(
            0,
            PlpKind::Common,
            1,
            (Short, R1_2, Qpsk, false),
            1,
            TimeIl::Blocks(1),
            0x0100,
        ),
        plp(
            1,
            PlpKind::Type1,
            1,
            (Short, R2_3, Qam16, true),
            3,
            TimeIl::Blocks(2),
            0x0101,
        ),
        plp(
            2,
            PlpKind::Type2,
            2,
            (Short, R3_5, Qam64, false),
            4,
            TimeIl::Blocks(1),
            0x0102,
        ),
        plp(
            3,
            PlpKind::Type2,
            3,
            (Normal, R3_4, Qam64, true),
            2,
            TimeIl::Frames(2),
            0x0103,
        ),
    ];
    cfg
}

fn check_plp(x: &[Complex32], id: u8, pid: u16) {
    let set = RxSettings {
        plp: Some(id),
        ..Default::default()
    };
    let mut rx = sdrkit_stack_dvbt2::rx::T2Receiver::new(set);
    let pattern = Numbered::new(pid, Fill::I31N7);
    let mut check = sdrkit_proto_mpegts::source::NumberedCheck::default();
    let mut df = sdrkit_proto_ipcast::adapt::UpDeframer::new();
    for c in x.chunks(30_000) {
        rx.feed(c, &mut |b| {
            if b.plp == id {
                let _ = df.push_frame(b.bytes, &mut |u| check.push(&pattern, u.bytes, 0.0));
            }
        });
    }
    let st = rx.status();
    assert_eq!(st.stage, Stage::Locked, "{:?}", st.log);
    assert!(
        check.good > 20 && check.wrong == 0 && check.flagged == 0 && check.gaps == 0,
        "PLP {id}: {check:?} {:?}",
        st.problem
    );
}

#[test]
fn multiple_plps_common_type1_type2_and_time_interleaving() {
    let mut tx = Transmitter::new(multi_config(false)).unwrap();
    let x = signal(&mut tx, 8, 2000);
    for (id, pid) in [(1, 0x0101), (2, 0x0102), (3, 0x0103)] {
        check_plp(&x, id, pid);
    }
    // The common PLP of group 1 is decoded with PLP 1.
    let mut rx = sdrkit_stack_dvbt2::rx::T2Receiver::new(RxSettings {
        plp: Some(1),
        ..Default::default()
    });
    let mut common = 0;
    rx.feed(&x, &mut |b| common += (b.plp == 0 && b.ok) as usize);
    assert_eq!(rx.status().common, Some(0));
    assert!(common >= 6, "{common}");
}

#[test]
fn miso_with_a_second_transmitter_group() {
    let mut tx = Transmitter::new(multi_config(true)).unwrap();
    let x = signal(&mut tx, 6, 2000);
    check_plp(&x, 1, 0x0101);
    check_plp(&x, 2, 0x0102);
}

#[test]
fn t2_lite_with_its_own_code_rates() {
    use CodeRate::*;
    use FecFrame::*;
    use Modulation::*;
    for (rate, m) in [(R1_3, Qpsk), (R2_5, Qam16), (R1_3, Qam256)] {
        let mut cfg = single(
            Fft::K16,
            Guard::G1_16,
            PilotPattern::Pp4,
            coding(Short, rate, m, m != Qam256),
            4,
        );
        cfg.profile = Profile::Lite;
        let mut tx = Transmitter::new(cfg).unwrap();
        let x = signal(&mut tx, 4, 1000);
        let (st, check, _) = receive(&x, RxSettings::default(), 40_000, 0);
        assert_eq!(st.profile, Some(Profile::Lite));
        assert!(
            check.good > 10 && check.wrong == 0 && check.flagged == 0,
            "{rate:?} {m:?} {check:?}"
        );
    }
}

#[test]
fn l1_repetition_scrambled_l1_post_and_fef_parts() {
    let mut cfg = single(
        Fft::K4,
        Guard::G1_16,
        PilotPattern::Pp5,
        coding(FecFrame::Short, CodeRate::R3_5, Modulation::Qam64, true),
        3,
    );
    cfg.repetition = true;
    cfg.post_scrambled = true;
    cfg.num_t2_frames = 4;
    cfg.fef = Some((30_000, 2));
    let mut tx = Transmitter::new(cfg).unwrap();
    let x = signal(&mut tx, 8, 4000);
    let (st, check, _) = receive(&x, RxSettings::default(), 7_000, 0);
    assert!(st.frames >= 7, "{} {:?}", st.frames, st.log);
    assert!(
        check.good > 50 && check.wrong == 0 && check.flagged == 0 && check.gaps == 0,
        "{check:?}"
    );
}

fn impaired(x: &mut [Complex32], snr_db: f64, offset_hz: f64, rate: f64, seed: u64) {
    if offset_hz != 0.0 {
        sdrkit_algo_sim::offset::FrequencyOffset::new(rate, offset_hz, 0.0)
            .unwrap()
            .process(x);
    }
    let p = 10f64.powf(-snr_db / 10.0);
    sdrkit_algo_sim::noise::Awgn::with_power(p, seed)
        .unwrap()
        .process(x);
}

#[test]
fn noise_and_a_large_carrier_offset() {
    // 16-QAM 2/3 needs about 10 dB in a Gaussian channel (TS 102 831 table 49 order of magnitude): 15 dB with 120 kHz offset.
    let cfg = single(
        Fft::K8,
        Guard::G1_8,
        PilotPattern::Pp3,
        coding(FecFrame::Normal, CodeRate::R2_3, Modulation::Qam16, true),
        2,
    );
    let mut tx = Transmitter::new(cfg).unwrap();
    let mut x = signal(&mut tx, 5, 7000);
    impaired(&mut x, 15.0, 120_000.0, Bandwidth::B8.rate(), 7);
    let (st, check, _) = receive(&x, RxSettings::default(), 16_384, 0);
    assert!((st.offset_hz - 120_000.0).abs() < 100.0, "{}", st.offset_hz);
    assert!(st.snr_db.is_some_and(|s| s > 10.0), "{:?}", st.snr_db);
    assert!(
        check.good > 50 && check.wrong == 0 && check.flagged == 0,
        "{check:?}"
    );
}

#[test]
fn the_worker_pool_returns_the_frames_of_one_thread() {
    // Several PLPs with time interleaving, and a PLP of many blocks in one frame.
    let mut many = Transmitter::new(single(
        Fft::K8,
        Guard::G1_16,
        PilotPattern::Pp4,
        coding(FecFrame::Short, CodeRate::R2_3, Modulation::Qam64, true),
        9,
    ))
    .unwrap();
    let mut multi = Transmitter::new(multi_config(false)).unwrap();
    for (x, plp) in [(signal(&mut many, 5, 1234), None), (signal(&mut multi, 9, 500), Some(2))] {
        let run = |workers: Option<usize>| {
            let set = RxSettings {
                plp,
                ..RxSettings::default()
            };
            let mut rx = sdrkit_stack_dvbt2::rx::T2Receiver::new(set);
            rx.pool = workers.map(|n| {
                let pool = sdrkit_graph::Pool::new(4);
                pool.set_workers(n);
                pool
            });
            let mut frames: Vec<(u8, bool, Vec<u8>)> = Vec::new();
            for c in x.chunks(4096) {
                rx.feed(c, &mut |b| frames.push((b.plp, b.ok, b.bytes.to_vec())));
            }
            let st = rx.status();
            (frames, st.blocks_ok, st.blocks_bad, st.bch_corrected)
        };
        let one = run(None);
        assert!(one.0.len() > 8 && one.2 == 0, "{} frames, {} bad", one.0.len(), one.2);
        for workers in [0, 1, 3, 4] {
            assert!(run(Some(workers)) == one, "{workers} helpers");
        }
    }
}

#[test]
fn any_cut_of_the_input_returns_the_same_frames() {
    let cfg = single(
        Fft::K2,
        Guard::G1_16,
        PilotPattern::Pp4,
        coding(FecFrame::Short, CodeRate::R3_4, Modulation::Qam64, false),
        3,
    );
    let mut tx = Transmitter::new(cfg).unwrap();
    let mut x = signal(&mut tx, 5, 1234);
    impaired(&mut x, 25.0, -3_000.0, Bandwidth::B8.rate(), 3);
    let run = |chunk: usize| {
        let mut rx = sdrkit_stack_dvbt2::rx::T2Receiver::new(RxSettings::default());
        let mut frames: Vec<Vec<u8>> = Vec::new();
        for c in x.chunks(chunk) {
            rx.feed(c, &mut |b| frames.push(b.bytes.to_vec()));
        }
        frames
    };
    let a = run(4096);
    assert!(!a.is_empty());
    assert_eq!(a, run(997));
    assert_eq!(a, run(x.len()));
}

#[test]
fn tone_reservation_in_the_data_and_frame_closing_symbols() {
    for (fft, guard, pp) in [
        (Fft::K8, Guard::G1_8, PilotPattern::Pp2),
        (Fft::K1, Guard::G1_4, PilotPattern::Pp1),
    ] {
        let mut cfg = single(
            fft,
            guard,
            pp,
            coding(FecFrame::Short, CodeRate::R2_3, Modulation::Qam64, true),
            3,
        );
        cfg.shape.tr = true;
        let mut tx = Transmitter::new(cfg).unwrap();
        assert!(tx.shape().has_fc());
        let x = signal(&mut tx, 4, 3000);
        let (st, check, _) = receive(&x, RxSettings::default(), 20_000, 0);
        assert!(st.pre.is_some_and(|p| p.tone_reservation()));
        assert!(
            check.good > 20 && check.wrong == 0 && check.flagged == 0,
            "{} {check:?}",
            fft.name()
        );
    }
}

/// Annex D: one transport stream split into a data PLP (everything but PSI/SI) and a common PLP (PSI/SI), each with null packets
/// in the other's slots, comes back whole from the receiver and the merge of `sdrkit-proto-ipcast`.
#[test]
fn common_plp_is_merged_back_into_the_transport_stream() {
    use sdrkit_proto_mpegts::source::{null_packet, PacketSource, TestMux};
    use sdrkit_proto_ipcast::merge::CommonMerge;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    struct Split {
        mux: TestMux,
        q: [VecDeque<[u8; 188]>; 2],
        sent: Vec<[u8; 188]>,
    }
    struct Half(Arc<Mutex<Split>>, usize);
    impl PacketSource for Half {
        fn next_packet(&mut self, out: &mut [u8; 188]) {
            let mut s = self.0.lock().unwrap();
            if s.q[self.1].is_empty() {
                let mut p = [0u8; 188];
                s.mux.next_packet(&mut p);
                s.sent.push(p);
                let pid = ((p[1] as u16 & 0x1F) << 8) | p[2] as u16;
                let common = pid < 0x20;
                let (c, d) = if common {
                    (p, null_packet())
                } else {
                    (null_packet(), p)
                };
                s.q[0].push_back(c);
                s.q[1].push_back(d);
            }
            *out = s.q[self.1].pop_front().unwrap();
        }
    }
    let split = Arc::new(Mutex::new(Split {
        mux: TestMux::dvbt(7, "annex d").unwrap(),
        q: [VecDeque::new(), VecDeque::new()],
        sent: Vec::new(),
    }));
    let mut cfg = single(
        Fft::K4,
        Guard::G1_8,
        PilotPattern::Pp3,
        coding(FecFrame::Short, CodeRate::R1_2, Modulation::Qpsk, false),
        2,
    );
    let mut common = plp(
        0,
        PlpKind::Common,
        5,
        (FecFrame::Short, CodeRate::R1_2, Modulation::Qpsk, false),
        2,
        TimeIl::Blocks(1),
        0,
    );
    common.source = Box::new(Half(split.clone(), 0));
    let mut data = plp(
        1,
        PlpKind::Type1,
        5,
        (FecFrame::Short, CodeRate::R1_2, Modulation::Qam16, true),
        2,
        TimeIl::Blocks(1),
        0,
    );
    data.source = Box::new(Half(split.clone(), 1));
    cfg.plps = vec![common, data];
    let mut tx = Transmitter::new(cfg).unwrap();
    let x = signal(&mut tx, 5, 1000);
    let mut rx = sdrkit_stack_dvbt2::rx::T2Receiver::new(RxSettings::default());
    let (mut dd, mut dc) = (
        sdrkit_proto_ipcast::adapt::UpDeframer::new(),
        sdrkit_proto_ipcast::adapt::UpDeframer::new(),
    );
    let mut merge = CommonMerge::default();
    merge.set_common(true);
    let mut got: Vec<[u8; 188]> = Vec::new();
    rx.feed(&x, &mut |b| {
        let id = b.bytes[1];
        if id == 0 {
            let _ = dc.push_frame(b.bytes, &mut |u| merge.push_common(u.bytes, u.crc_ok));
        } else {
            let _ = dd.push_frame(b.bytes, &mut |u| merge.push_data(u.bytes, u.crc_ok));
        }
        merge.drain(|p, _| got.push(*p));
    });
    assert_eq!(rx.status().common, Some(0));
    let sent = &split.lock().unwrap().sent;
    assert!(got.len() > 30, "{}", got.len());
    // The receiver starts with the first frame, so the merged stream is the sent stream from its start.
    assert_eq!(&got[..], &sent[..got.len()]);
}

/// The transmitter of the stack's default test signal: 8K, guard 1/8, PP3, 64-QAM 2/3, normal FEC frames.
fn default_signal(rotated: bool) -> Transmitter {
    let cfg = single(
        Fft::K8,
        Guard::G1_8,
        PilotPattern::Pp3,
        coding(FecFrame::Normal, CodeRate::R2_3, Modulation::Qam64, rotated),
        4,
    );
    Transmitter::new(cfg).expect("transmitter")
}

#[test]
fn the_default_test_signal_decodes() {
    for rotated in [false, true] {
        let x = signal(&mut default_signal(rotated), 4, 5000);
        let (st, check, _) = receive(&x, RxSettings::default(), 50_000, 0);
        assert_eq!(st.stage, Stage::Locked, "{:?}", st.log);
        assert!(
            check.good > 0 && check.wrong == 0 && check.flagged == 0 && check.gaps == 0,
            "rotated {rotated}: {check:?}"
        );
        assert!(st.snr_db.is_some_and(|s| s > 40.0), "{:?}", st.snr_db);
    }
}

/// The default test signal at a mean power of `level_db` dBFS through an 8-bit converter: the fraction of I and Q values
/// clipped, the SNR from the pilots, and the FEC blocks decoded and failed.
fn through_8_bits(level_db: f32) -> (f64, f32, u64, u64) {
    let mut x = signal(&mut default_signal(true), 4, 5000);
    let gain = 10f32.powf(level_db / 20.0);
    x.iter_mut().for_each(|v| *v *= gain);
    let mut clipped = 0usize;
    let mut round = |v: f32| {
        let s = v * 128.0;
        clipped += usize::from(!(-128.0..=127.0).contains(&s));
        s.clamp(-128.0, 127.0).round() / 128.0
    };
    x.iter_mut()
        .for_each(|v| *v = Complex32::new(round(v.re), round(v.im)));
    let (st, _, _) = receive(&x, RxSettings::default(), 50_000, 0);
    (
        clipped as f64 / (2 * x.len()) as f64,
        st.snr_db.unwrap_or(0.0),
        st.blocks_ok,
        st.blocks_bad,
    )
}

#[test]
fn an_8_bit_converter_needs_headroom() {
    // 9 dB below full scale the converter clips next to nothing and its grain is far below the signal.
    let (clipped, snr, ok, bad) = through_8_bits(-9.0);
    assert!(clipped < 1e-3 && snr > 35.0, "clipped {clipped} snr {snr}");
    assert!(ok > 0 && bad == 0, "ok {ok} bad {bad}");
    // At a mean power of full scale each of I and Q has a standard deviation of 0.71 and clips at 1: the distortion is
    // about 14 dB below the signal, at the limit of 64-QAM 2/3.
    let (clipped, snr, _, _) = through_8_bits(0.0);
    assert!(clipped > 0.1 && snr < 15.0, "clipped {clipped} snr {snr}");
}
