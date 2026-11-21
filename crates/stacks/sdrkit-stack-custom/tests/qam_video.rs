//! `qam_video` frames and the container through the modulator and the frame receiver (Part D section 14).

mod common;

use common::{resample, Noise};
use sdrkit_core::Complex32;
use sdrkit_stack_custom::qam_video::container::{Packet, Reassembler, Unpacker, VIDEO};
use sdrkit_stack_custom::qam_video::phy::{coded_bits, frame_symbols, modulate, permutation, scrambler, Burst, Header, Profile, Waveform};
use sdrkit_stack_custom::qam_video::rx::{Frame, FrameReceiver};
use sdrkit_stack_custom::qam_video::{ident_callsign, session_frames, Session, Source};
use std::f64::consts::TAU;

fn payloads(waveform: Waveform, n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|f| (0..4 * waveform.cell()).map(|i| ((7 * i + 3 + 11 * f) % 256) as u8).collect()).collect()
}

fn burst(waveform: Waveform) -> Burst {
    Burst {
        waveform,
        fec: 1,
        link: 0,
        first_seq: 0,
        first_sweep: 0,
        lead: 200,
        tail: 200,
        chirp: true,
    }
}

/// Carrier offset in cycles a symbol, fractional delay, Es/N0.
fn channel(x: &[Complex32], cps: f64, delay: f64, esn0: f64, seed: u64) -> Vec<Complex32> {
    let mut y = resample(x, 0.0, delay);
    for (i, v) in y.iter_mut().enumerate() {
        let a = TAU * (cps / 4.0 * i as f64).fract() + 1.0;
        *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
    }
    Noise::new(seed).add(&mut y, 4.0 / 10f64.powf(esn0 / 10.0));
    y
}

fn receive(x: &[Complex32], rate: f64, chunk: usize) -> Vec<Frame> {
    let mut rx = FrameReceiver::new(rate);
    let mut out = Vec::new();
    for c in x.chunks(chunk.max(1)) {
        rx.process(c, &mut out);
    }
    rx.process(&vec![Complex32::new(0.0, 0.0); 40_000], &mut out);
    rx.finish(&mut out);
    out
}

fn good(f: &[Frame]) -> usize {
    f.iter().filter(|f| f.cells.iter().all(Option::is_some)).count()
}

#[test]
fn each_profile_and_form_at_35_db() {
    for p in Profile::ALL {
        for robust in [false, true] {
            let m = Waveform { profile: p, robust };
            let pay = payloads(m, 4);
            let y = channel(&modulate(&burst(m), &pay), 0.0, 0.0, 35.0, 1);
            let f = receive(&y, p.symbol_rate(), 7000);
            assert_eq!(f.len(), 4, "{p:?} {robust}");
            assert_eq!(good(&f), 4, "{p:?} {robust}");
            for (i, fr) in f.iter().enumerate() {
                assert_eq!(fr.header.seq, i as u16);
                assert_eq!(fr.header.waveform, m);
                let joined: Vec<u8> = fr.cells.iter().flatten().flatten().copied().collect();
                assert_eq!(joined, pay[i]);
            }
        }
    }
}

#[test]
fn carrier_offsets_and_delays_of_profile_a() {
    let m = Waveform { profile: Profile::A, robust: false };
    let x = modulate(&burst(m), &payloads(m, 3));
    for cps in [-0.02, -0.01, 0.0, 0.01, 0.02] {
        for delay in [0.0, 0.37, 0.8] {
            let f = receive(&channel(&x, cps, delay, 18.0, 2), 700_000.0, 10_000);
            assert_eq!(good(&f), 3, "{cps} {delay}");
            let hz = f[0].carrier * 700_000.0;
            assert!((hz - cps * 700_000.0).abs() < 300.0, "{cps} {delay}: {hz}");
        }
    }
}

#[test]
fn profile_d_with_offset_and_delay() {
    let m = Waveform { profile: Profile::D, robust: false };
    let f = receive(&channel(&modulate(&burst(m), &payloads(m, 3)), 0.02, 0.5, 30.0, 3), 1_400_000.0, 10_000);
    assert_eq!(good(&f), 3);
}

#[test]
fn chunks_of_any_size() {
    let m = Waveform { profile: Profile::B, robust: false };
    let y = channel(&modulate(&burst(m), &payloads(m, 3)), 0.005, 0.0, 20.0, 4);
    let mut rx = FrameReceiver::new(1_050_000.0);
    let mut out = Vec::new();
    let mut at = 0;
    let mut k = 1usize;
    while at < y.len() {
        let n = (k * 7919 % 5000 + 1).min(y.len() - at);
        rx.process(&y[at..at + n], &mut out);
        at += n;
        k += 1;
    }
    rx.process(&vec![Complex32::new(0.0, 0.0); 40_000], &mut out);
    rx.finish(&mut out);
    assert_eq!(good(&out), 3);
}

#[test]
fn nothing_in_noise_and_nothing_at_another_rate() {
    let mut n = vec![Complex32::new(0.0, 0.0); 300_000];
    Noise::new(5).add(&mut n, 1.0);
    assert!(receive(&n, 700_000.0, 10_000).is_empty());
    let m = Waveform { profile: Profile::A, robust: false };
    let y = channel(&modulate(&burst(m), &payloads(m, 3)), 0.0, 0.0, 30.0, 6);
    assert!(receive(&y, 1_400_000.0, 10_000).is_empty());
}

#[test]
fn bursts_of_noise_give_no_wrong_data() {
    let m = Waveform { profile: Profile::A, robust: false };
    let pay = payloads(m, 3);
    let x = modulate(&burst(m), &pay);
    for trial in 0..20u64 {
        let mut y = channel(&x, 0.0, 0.0, 25.0, 100 + trial);
        let mut nz = Noise::new(trial);
        for b in 0..(trial % 6 + 1) {
            let at = ((trial * 7919 + b * 104_729) as usize) % (y.len() - 3000);
            let len = ((trial * 31 + b * 977) % 3000 + 1) as usize;
            for v in &mut y[at..at + len] {
                *v += nz.sample(20.0);
            }
        }
        for f in receive(&y, 700_000.0, 10_000) {
            assert_eq!(f.cells.len(), 4);
            for (b, c) in f.cells.iter().enumerate() {
                if let Some(c) = c {
                    let i = usize::from(f.header.seq);
                    assert_eq!(c[..], pay[i][b * m.cell()..(b + 1) * m.cell()], "{trial}");
                }
            }
        }
    }
}

#[test]
fn the_synthetic_stream_through_the_container_at_13_5_db() {
    let s = Session {
        waveform: Waveform { profile: Profile::A, robust: false },
        fec: 1,
        callsign: "TEST1".into(),
        tag: b"TEST".to_vec(),
        source: Source::Synthetic {
            seconds: 0.5,
            video_bps: 1_200_000.0,
        },
    };
    let (frames, sent) = session_frames(&s).unwrap();
    let b = Burst { fec: 1, ..burst(s.waveform) };
    let y = channel(&modulate(&b, &frames), 0.007, 0.31, 13.5, 7);
    let got = receive(&y, 700_000.0, 20_000);
    assert_eq!(got.len(), frames.len());
    let mut un = Unpacker::new(178);
    let mut packets: Vec<Packet> = Vec::new();
    for f in &got {
        un.frame(f.header.seq, usize::from(f.header.fec), &f.cells, &mut packets);
    }
    un.finish(&mut packets);
    assert_eq!(packets, sent);
    let mut r = Reassembler::default();
    let units = packets.iter().filter(|p| p.kind == VIDEO).filter_map(|p| r.push(p)).count();
    assert!(units >= 14, "{units}");
    let id = packets.iter().find_map(|p| (p.kind == 2).then(|| ident_callsign(&p.payload)).flatten());
    assert_eq!(id.as_deref(), Some("TEST1"));
}

#[test]
fn a_file_with_one_frame_cut() {
    let file: Vec<u8> = (0..20_000u32).map(|i| (i * 31 % 251) as u8).collect();
    for (fec, whole) in [(3usize, true), (0, false)] {
        let s = Session {
            waveform: Waveform { profile: Profile::A, robust: false },
            fec,
            callsign: "TEST1".into(),
            tag: Vec::new(),
            source: Source::File(file.clone()),
        };
        let (frames, _) = session_frames(&s).unwrap();
        let b = Burst { fec: fec as u8, ..burst(s.waveform) };
        let mut y = channel(&modulate(&b, &frames), 0.0, 0.0, 24.0, 8);
        // Cut the middle half of the third frame.
        let fs = 4 * s.waveform.frame_symbols();
        let start = 4 * 200 + 2 * fs + fs / 4;
        for v in &mut y[start..start + fs / 2] {
            *v = Complex32::new(0.0, 0.0);
        }
        let got = receive(&y, 700_000.0, 20_000);
        let mut un = Unpacker::new(178);
        let mut packets: Vec<Packet> = Vec::new();
        for f in &got {
            un.frame(f.header.seq, usize::from(f.header.fec), &f.cells, &mut packets);
        }
        un.finish(&mut packets);
        let data: Vec<u8> = packets.iter().filter(|p| p.kind == 4).flat_map(|p| p.payload.clone()).collect();
        assert_eq!(data == file, whole, "fec {fec}");
        if whole {
            assert!(un.counts.restored >= 1 && un.counts.lost == 0);
        }
    }
}

/// Section 14.2 test 8: one echo of 0.35 one symbol late, offset 0.004 cycles a symbol, delay 0.3 sample, 15 frames of profile
/// A. At Es/N0 22 dB at least 14 frames are good with the equaliser, and no fewer without it: the one-tap path of this receiver
/// already decodes them there. At 16 dB, where it does not, the equaliser still gives at least 14 and more than without it.
#[test]
fn multipath_with_the_equaliser() {
    let m = Waveform { profile: Profile::A, robust: false };
    let x = modulate(&burst(m), &payloads(m, 15));
    let mut e = x.clone();
    for i in 4..x.len() {
        e[i] = x[i] + x[i - 4] * 0.35;
    }
    for (db, seed) in [(22.0, 8), (16.0, 9)] {
        let y = channel(&e, 0.004, 0.3, db, seed);
        let count = |eq: bool| {
            let mut rx = FrameReceiver::new(Profile::A.symbol_rate());
            rx.equaliser = eq;
            let mut out = Vec::new();
            for c in y.chunks(7000) {
                rx.process(c, &mut out);
            }
            rx.process(&vec![Complex32::new(0.0, 0.0); 40_000], &mut out);
            rx.finish(&mut out);
            good(&out)
        };
        let (with, without) = (count(true), count(false));
        assert!(with >= 14 && with >= without, "{db} dB: {with} good with the equaliser, {without} without");
        if db < 20.0 {
            assert!(with > without, "{db} dB: {with} good with the equaliser, {without} without");
        }
    }
}

/// The frame vectors of section 15.2 for every profile and form: the interleaver and scrambler tables, the coded bits of
/// frame 0 (exact), its symbols and the modulator output with and without the chirp (to 1e-4).
#[test]
fn the_frame_vectors() {
    let Some(root) = common::recordings() else { return };
    let dir = common::folder_with(&root, "frame_A_robust_syms0.cf32");
    let perm: Vec<u16> = std::fs::read(common::recording(&dir, "perm_int32le.bin")).unwrap().as_chunks::<4>().0.iter().map(|c| u16::try_from(i32::from_le_bytes(*c)).unwrap()).collect();
    assert_eq!(perm, permutation());
    assert_eq!(std::fs::read(common::recording(&dir, "scramble_7680.bin")).unwrap(), scrambler());
    let near = |a: &[Complex32], b: &[Complex32], what: &str| {
        assert_eq!(a.len(), b.len(), "{what}: length");
        let e = a.iter().zip(b).map(|(x, y)| (x - y).norm()).fold(0f32, f32::max);
        assert!(e < 1e-4, "{what}: error {e}");
    };
    for p in Profile::ALL {
        for robust in [false, true] {
            let w = Waveform { profile: p, robust };
            let tag = format!("frame_{}{}_", p.letter(), if robust { "_robust" } else { "" });
            let pay = std::fs::read(common::recording(&dir, &format!("{tag}payload2.bin"))).unwrap();
            assert_eq!(pay, payloads(w, 2).concat(), "{tag}");
            let pays: Vec<Vec<u8>> = pay.chunks(pay.len() / 2).map(<[u8]>::to_vec).collect();
            assert_eq!(coded_bits(w, &pays[0]), std::fs::read(common::recording(&dir, &format!("{tag}coded0.bin"))).unwrap(), "{tag}");
            let h = Header { waveform: w, fec: 1, sweep: 0, seq: 0, link: 0 };
            near(&frame_symbols(&h, &pays[0]), &common::read_cf32(&common::recording(&dir, &format!("{tag}syms0.cf32"))), &tag);
            for chirp in [true, false] {
                let b = Burst { waveform: w, fec: 1, link: 0, first_seq: 0, first_sweep: 0, lead: 32, tail: 32, chirp };
                let name = format!("{tag}iq_{}chirp.cf32", if chirp { "" } else { "no" });
                near(&modulate(&b, &pays), &common::read_cf32(&common::recording(&dir, &name)), &name);
            }
        }
    }
}

/// The recordings of section 15.2 at the radio rate (profile A at 2.8 Msps, profile D at 5.6 Msps, 4 samples a symbol, a
/// 1.5 s stream at Es/N0 25 dB), in both sample forms: 508 of 508 and 1340 of 1340 frames good.
#[test]
fn the_recorded_streams() {
    let Some(root) = common::recordings() else { return };
    let dir = common::folder_with(&root, "frame_A_robust_syms0.cf32");
    for (stem, p, frames) in [("_PA_700ksym", Profile::A, 508), ("_PD_1400ksym", Profile::D, 1340)] {
        for (name, rate, y) in common::both_forms(&dir, stem) {
            assert_eq!(rate, 4.0 * p.symbol_rate(), "{name}");
            let f = receive(&y, p.symbol_rate(), 65_536);
            assert_eq!((f.len(), good(&f)), (frames, frames), "{name}");
        }
    }
}

#[test]
fn proposed_thresholds() {
    for (p, robust, esn0) in [(Profile::A, false, 13.5), (Profile::D, false, 19.0), (Profile::A, true, 6.0)] {
        let m = Waveform { profile: p, robust };
        let y = channel(&modulate(&burst(m), &payloads(m, 10)), 0.006, 0.4, esn0, 9);
        let f = receive(&y, p.symbol_rate(), 20_000);
        assert_eq!(f.len(), 10, "{p:?} {robust}");
        let bad: usize = f.iter().map(|f| f.cells.iter().filter(|c| c.is_none()).count()).sum();
        assert!(bad <= 1, "{p:?} {robust}: {bad} of 40 blocks failed at {esn0} dB");
    }
}

/// A transmission of `file` (profile A, FEC 1, LY2ABC) at the radio rate `rate`: a clock error of `ppm`, the carrier at `hz`,
/// noise for Es/N0 `esn0` when given, and 8-bit samples written to a file and read back when `eight_bit`.
fn radio_signal(file: &[u8], rate: f64, ppm: f64, hz: f64, esn0: Option<f64>, eight_bit: bool, seed: u64) -> Vec<Complex32> {
    use sdrkit_stack_custom::qam_video::session_samples;
    use sdrkit_stack_custom::qam_video::stack::tx_session;
    let s = tx_session("LY2ABC", "54455354", Profile::A, 1, Source::File(file.to_vec())).unwrap();
    let (x, _) = session_samples(&s).unwrap();
    let fs = 4.0 * Profile::A.symbol_rate();
    let mut y = Vec::new();
    sdrkit_algo_dsp::resample::Resampler::new(fs, rate).unwrap().process(&x, &mut y);
    let mut y = resample(&y, ppm, 0.0);
    y.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), (0.3 * rate) as usize));
    common::shift(&mut y, hz, rate);
    if let Some(db) = esn0 {
        // The data has unit power a sample at any rate, so a symbol holds rate / Rs of it.
        Noise::new(seed).add(&mut y, rate / Profile::A.symbol_rate() / 10f64.powf(db / 10.0));
    }
    if !eight_bit {
        return y;
    }
    let peak = y.iter().map(|v| v.re.abs().max(v.im.abs())).fold(0f32, f32::max);
    let g = 0.8 * 127.0 / peak;
    let bytes: Vec<u8> = y.iter().flat_map(|v| [(v.re * g).round() as i8 as u8, (v.im * g).round() as i8 as u8]).collect();
    let path = std::env::temp_dir().join(format!("qam_video_radio_{seed}_{}.ci8", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let back = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    back.as_chunks::<2>().0.iter().map(|c| Complex32::new(f32::from(c[0] as i8) / 127.0, f32::from(c[1] as i8) / 127.0)).collect()
}

/// What the stack's decoder gave from a radio-rate signal: the file from the Data packets of stream 1, and the results.
fn radio_receive(y: &[Complex32], rate: f64) -> (Vec<u8>, sdrkit_core::Record) {
    use sdrkit_graph::{Decoder, Format, Kind, Outbox};
    use sdrkit_stack_custom::qam_video::container::{parse, Parsed, DATA};
    let mut d = sdrkit_stack_custom::qam_video::stack::QvDecoder::default();
    d.configure(&Format::iq(rate, 0.0, 0.0)).unwrap();
    let mut out = Outbox::new(&[Kind::Bytes]);
    let mut parts: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut take = |out: &mut Outbox| {
        for p in out.bytes(0).drain(..) {
            if let Parsed::Packet(p, _) = parse(&p.data) {
                if p.kind == DATA && p.stream == 1 {
                    parts.push((p.seq, p.payload));
                }
            }
        }
    };
    for (i, c) in y.chunks(65_536).enumerate() {
        d.process(c, (i * 65_536) as u64, &mut out);
        take(&mut out);
    }
    d.finish(&mut out);
    take(&mut out);
    let mut p = d.results();
    d.report(&mut p);
    parts.sort_by_key(|p| p.0);
    parts.dedup_by_key(|p| p.0);
    (parts.into_iter().flat_map(|p| p.1).collect(), p.results().fields().clone())
}

/// Section 14.5 tests 1 to 5: a file sent as Data packets (LY2ABC, profile A, FEC 1) through the stack's decoder at the radio
/// rate, with the host stage's coarse mixer and resampler. Clean at 2.4 and 3.2 Msps; +14 and -14 kHz with 8-bit samples, +20
/// and -20 ppm and 22 dB; +6 kHz at 20 dB; -3 kHz with 8-bit samples and +50 ppm without noise. The 8-bit signals pass
/// through a saved file. Each time the file is whole and no group is lost; the clean cases have no bad frame or block and the
/// callsign; the carrier is within 1500 Hz and the Es/N0 estimate within 5 dB.
#[test]
fn radio_rate_path_with_clock_errors() {
    use sdrkit_core::publish::keys;
    use sdrkit_stack_custom::typed::QamVideoFields as Qv;
    let file: Vec<u8> = (0..60_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
    for (rate, ppm, hz, esn0, eight, clean) in [
        (2.4e6, 0.0, 0.0, None, false, true),
        (3.2e6, 0.0, 0.0, None, false, true),
        (2.4e6, 20.0, 14_000.0, Some(22.0), true, false),
        (2.4e6, -20.0, -14_000.0, Some(22.0), true, false),
        (2.4e6, 0.0, 6000.0, Some(20.0), false, false),
        (2.4e6, 50.0, -3000.0, None, true, false),
    ] {
        let case = format!("{rate} sps, {ppm} ppm, {hz} Hz, {esn0:?} dB, 8-bit {eight}");
        let y = radio_signal(&file, rate, ppm, hz, esn0, eight, ppm.abs() as u64 + 3);
        let (got, f) = radio_receive(&y, rate);
        assert!(got == file, "{case}: {} of {} bytes", got.len(), file.len());
        assert_eq!(f.at(&Qv::GROUPS_LOST), Some(0), "{case}");
        let off = f.get(&keys::OFFSET_HZ).unwrap_or(f64::NAN);
        assert!((off - hz).abs() < 1500.0, "{case}: carrier {off}");
        if let Some(db) = esn0 {
            let e = f.at(&Qv::ESN0_DB).unwrap_or(f64::NAN);
            assert!((e - db).abs() < 5.0, "{case}: Es/N0 {e}");
        }
        if clean {
            assert_eq!((f.get(&keys::FRAMES_BAD), f.at(&Qv::BLOCKS_BAD)), (Some(0), Some(0)), "{case}");
            assert_eq!(f.get(&keys::CALLSIGN), Some("LY2ABC"), "{case}");
        }
    }
}


/// Section 14.5 test 9: 1 s of the synthetic stream, FEC 1, offset 0.01 Rs, through the stack's decoder: A and B at 2.4 Msps
/// and 24 dB, C at 3.2 Msps and 24 dB, D at 3.2 Msps and 30 dB. The profile and the callsign are right, video and audio
/// packets arrive, and no group is lost and no frame is bad.
#[test]
fn synthetic_stream_of_each_profile() {
    use sdrkit_core::publish::keys;
    use sdrkit_graph::{Decoder, Format, Kind, Outbox};
    use sdrkit_stack_custom::qam_video::container::{parse, Parsed, AUDIO};
    use sdrkit_stack_custom::qam_video::session_samples;
    use sdrkit_stack_custom::qam_video::stack::{tx_session, QvDecoder};
    use sdrkit_stack_custom::typed::QamVideoFields as Qv;
    for (p, rate, db, listen) in [(Profile::A, 2.4e6, 24.0, 0u16), (Profile::B, 2.4e6, 24.0, 1), (Profile::C, 3.2e6, 24.0, 2), (Profile::D, 3.2e6, 30.0, 2)] {
        let s = tx_session("LY2ABC", "54455354", p, 1, Source::Synthetic { seconds: 1.0, video_bps: 1_200_000.0 }).unwrap();
        let (x, _) = session_samples(&s).unwrap();
        let fs = 4.0 * p.symbol_rate();
        let mut y = Vec::new();
        sdrkit_algo_dsp::resample::Resampler::new(fs, rate).unwrap().process(&x, &mut y);
        common::shift(&mut y, 0.01 * p.symbol_rate(), rate);
        Noise::new(listen.into()).add(&mut y, rate / p.symbol_rate() / 10f64.powf(db / 10.0));
        let mut d = QvDecoder::default();
        let schema = d.settings();
        d.apply(schema.id("listen_profile").unwrap(), &sdrkit_core::Value::Choice(listen)).unwrap();
        d.configure(&Format::iq(rate, 0.0, 0.0)).unwrap();
        let mut out = Outbox::new(&[Kind::Bytes]);
        let (mut video, mut audio) = (0, 0);
        let mut count = |out: &mut Outbox| {
            for b in out.bytes(0).drain(..) {
                if let Parsed::Packet(q, _) = parse(&b.data) {
                    video += usize::from(q.kind == VIDEO);
                    audio += usize::from(q.kind == AUDIO);
                }
            }
        };
        for (i, c) in y.chunks(65_536).enumerate() {
            d.process(c, (i * 65_536) as u64, &mut out);
            count(&mut out);
        }
        d.finish(&mut out);
        count(&mut out);
        let mut r = d.results();
        d.report(&mut r);
        let f = r.results().fields();
        let case = format!("{p:?}");
        assert_eq!(f.at(&Qv::PROFILE), Some(format!("{}", p.letter()).as_str()), "{case}");
        assert_eq!(f.get(&keys::CALLSIGN), Some("LY2ABC"), "{case}");
        assert!(video > 0 && audio > 0, "{case}: {video} video, {audio} audio");
        assert_eq!((f.at(&Qv::GROUPS_LOST), f.get(&keys::FRAMES_BAD)), (Some(0), Some(0)), "{case}");
        assert!(f.get(&keys::FRAMES_OK).unwrap_or(0) > 10, "{case}");
    }
}
