//! Each stack as the engine runs it: its test signal, moved by the offset of its verify case, through the receiver graph,
//! and the results. Also the regressions that the whole graph showed.

mod common;

use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, Results, StackConfig, StackDescriptor, Values};
use sdrkit_stack_custom::slotted_voice::{test_speech, v3, v4};

fn run(d: &'static StackDescriptor, offset: f64, seconds: f64) -> Results {
    let mut rx = receiver_after(d, offset, seconds);
    let mut out = Output::default();
    rx.finish(&mut out);
    rx.results().clone()
}

/// The receiver of `d` after `seconds` of its test signal moved by `offset` Hz.
fn receiver_after(d: &'static StackDescriptor, offset: f64, seconds: f64) -> Box<dyn Receiver> {
    let rate = d.tuning.rate_hz;
    let s = Values::defaults(&(d.settings)());
    let mut g = (d.make_generator.unwrap())(&GenConfig::new(rate, s.clone()).with_seed(1))
    .unwrap();
    let mut rx = (d.make_receiver)(&StackConfig::new(s)).unwrap();
    rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording()).unwrap();
    let mut out = Output::default();
    let mut x = vec![Complex32::new(0.0, 0.0); 65_536];
    let mut n = 0usize;
    for _ in 0..(rate * seconds) as usize / x.len() {
        g.generate(&mut x);
        for (i, v) in x.iter_mut().enumerate() {
            let a = std::f64::consts::TAU * (offset * (n + i) as f64 / rate).fract();
            *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
        }
        n += x.len();
        rx.feed(&x, &mut out);
        out.clear();
    }
    rx
}

/// A change of frequency by the host keeps the message log of `arq_packet` and the count of its messages, and forgets
/// the measurements of the last burst and the other counts.
#[test]
fn arq_packet_keeps_its_messages_over_a_change_of_frequency() {
    use sdrkit_stack_custom::typed::{ArqPacketFields as Ap, ArqPacketMessageRow, ARQ_PACKET_MESSAGES};
    let mut rx = receiver_after(&sdrkit_stack_custom::arq_packet::ARQ_PACKET, 1000.0, 10.0);
    let rows = |r: &Results| ARQ_PACKET_MESSAGES.rows::<ArqPacketMessageRow>(r).count();
    let f = rx.results().fields();
    let messages = f.at(&Ap::MESSAGES).unwrap_or(0);
    assert!(messages >= 1, "{messages}");
    assert_eq!(rows(rx.results()), messages as usize);
    assert!(f.at(&Ap::SNR_DB).is_some() && f.at(&Ap::PROFILE).is_some());
    assert!(f.at(&Ap::BLOCKS_GOOD).unwrap_or(0) > 0);
    rx.forget();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::new(0.0, 0.0); 200_000], &mut out);
    let f = rx.results().fields();
    assert_eq!(f.at(&Ap::MESSAGES), Some(messages));
    assert_eq!(rows(rx.results()), messages as usize);
    assert_eq!((f.at(&Ap::SNR_DB), f.at(&Ap::OFFSET_HZ), f.at(&Ap::PROFILE)), (None, None, None));
    assert_eq!((f.at(&Ap::BLOCKS_GOOD), f.at(&Ap::BURSTS), f.at(&Ap::LOCKED)), (Some(0), Some(0), Some(false)));
}

#[test]
fn slotted_voice_through_its_graph() {
    let r = run(&sdrkit_stack_custom::slotted_voice::SLOTTED_VOICE, 1200.0, 8.0);
    let f = r.fields();
    assert_eq!(f.get(&keys::LOCKED), Some(true));
    let ok = f.get(&keys::FRAMES_OK).unwrap_or(0);
    assert!(ok > 200 && f.get(&keys::FRAMES_BAD).unwrap_or(0) * 20 < ok, "{ok}");
    assert!((f.get(&keys::OFFSET_HZ).unwrap() - 1200.0).abs() < 150.0);
    assert_eq!(f.get(&keys::CALLSIGN), Some("TEST1"));
    // An 8000 symbols a second signal ends on the 8000 symbols a second receiver.
    assert_eq!(f.get(&keys::SYMBOL_RATE_HZ), Some(8000.0));
}

#[test]
fn qam_video_through_its_graph() {
    let r = run(&sdrkit_stack_custom::qam_video::QAM_VIDEO, 3000.0, 3.0);
    let f = r.fields();
    let ok = f.get(&keys::FRAMES_OK).unwrap_or(0);
    assert!(ok > 10 && f.get(&keys::FRAMES_BAD).unwrap_or(0) * 10 <= ok, "{ok}");
    assert!(f.at(&sdrkit_stack_custom::typed::QamVideoFields::PACKETS).unwrap_or(0) >= 20);
    assert!((f.get(&keys::OFFSET_HZ).unwrap() - 3000.0).abs() < 400.0);
    assert_eq!(f.get(&keys::CALLSIGN), Some("TEST1"));
}

#[test]
fn qam_video_coarse_search_far_from_the_centre() {
    // 84 kHz is 0.12 of the symbol rate: the last step of the coarse search.
    let r = run(&sdrkit_stack_custom::qam_video::QAM_VIDEO, 84_000.0, 3.0);
    let f = r.fields();
    assert!(f.get(&keys::FRAMES_OK).unwrap_or(0) > 10);
    assert!((f.get(&keys::OFFSET_HZ).unwrap() - 84_000.0).abs() < 1500.0);
}

#[test]
fn picture_burst_through_its_graph() {
    let r = run(&sdrkit_stack_custom::picture_burst::PICTURE_BURST, 500.0, 12.0);
    let f = r.fields();
    use sdrkit_stack_custom::typed::PictureBurstFields as Pb;
    assert_eq!(f.at(&Pb::LAYERS), f.at(&Pb::LAYERS_SENT));
    assert_eq!(f.at(&Pb::LAYERS), Some(8));
    assert!((f.get(&keys::OFFSET_HZ).unwrap() - 500.0).abs() < 150.0);
}

#[test]
fn version_4_decodes_nothing_from_version_3() {
    let mut tx = v3::Transmitter::new(sdrkit_codec_custom::voice3::V3Rate::R16, "TEST1", None);
    let mut x = Vec::new();
    tx.push(&test_speech(5 * 16_000, 1), &mut x);
    let mut rx = v4::Receiver::new();
    let mut ev = Vec::new();
    for c in x.chunks(2000) {
        rx.process(c, &mut ev);
    }
    assert_eq!(rx.report.slots_good, 0);
    assert!(!rx.report.locked);
}

#[test]
fn version_3_ignores_a_plain_up_sweep() {
    // The sweep alone at 10 dB SNR: no word decodes.
    let n = 3 * 48_000;
    let mut x: Vec<Complex32> = (0..n)
        .map(|i| sdrkit_stack_custom::slotted_voice::sweep::template(0, (i % 24_240) as i64))
        .collect();
    common::Noise::new(3).add(&mut x, 0.1);
    let mut rx = v3::Receiver::new();
    let mut ev = Vec::new();
    for c in x.chunks(1000) {
        rx.process(c, &mut ev);
    }
    assert_eq!(rx.report.words_good, 0);
}

#[test]
fn receivers_survive_odd_input() {
    for d in sdrkit_stack_custom::descriptors() {
        let rate = d.tuning.rate_hz;
        let mut rx = (d.make_receiver)(&StackConfig::new(Values::defaults(&(d.settings)()))).unwrap();
        rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording()).unwrap();
        let mut out = Output::default();
        let mut noise = common::Noise::new(4);
        for k in 0..20usize {
            let len = 1 + (k * 7919) % 30_000;
            let x: Vec<Complex32> = (0..len)
                .map(|i| match k % 4 {
                    0 => Complex32::new(0.0, 0.0),
                    1 => Complex32::new(1.0, -1.0),
                    2 => noise.sample(1.0),
                    _ => Complex32::new(if i % 2 == 0 { 1.0 } else { -1.0 }, 1.0),
                })
                .collect();
            rx.feed(&x, &mut out);
            out.clear();
        }
        let f = rx.results().fields();
        assert_ne!(f.get(&keys::LOCKED), Some(true), "{} {:?} {:?} {:?}", d.id, f.at(&sdrkit_stack_custom::typed::SlottedVoiceFields::FRAME_VERSION), f.get(&keys::FRAMES_OK), f.get(&keys::SYMBOL_RATE_HZ));
    }
}
