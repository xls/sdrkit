//! `hf_chat` through its transmitter and receiver (Part H section 14): every profile through 12 kHz audio with a carrier and
//! clock offset, noise alone, fading channels, the recordings, and the session.

mod common;

use common::{power, shift, Noise};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Results, StackConfig, Values};
use sdrkit_stack_custom::hf_chat::frame::{self, blocks, parse_block, Block, Chat, Message, Recipient, ShortBeacon, VoiceSide};
use sdrkit_stack_custom::hf_chat::phy::{burst, from_audio, layout, to_audio};
use sdrkit_stack_custom::hf_chat::rx::{Burst, Receiver};
use sdrkit_stack_custom::hf_chat::stack as hc;
use sdrkit_stack_custom::typed::{HfChatBeaconRow, HfChatFields as Hc, HfChatMessageRow};
use sdrkit_stack_custom::hf_chat::{Profile, PROFILES, RATE};

/// The chat of section 9.10.
fn chat() -> Message {
    Message::Chat(Chat {
        from: "LY1ABC".into(),
        to: Recipient::Call("CQ".into()),
        raw: false,
        more: false,
        seq: 0,
        ack: 0,
        text: "CQ DE LY1ABC KO24".into(),
    })
}

/// The short beacon of section 9.10.
fn beacon() -> Message {
    Message::ShortBeacon(ShortBeacon {
        callsign: "LY1ABC".into(),
        locator: "KO24RM".into(),
        power_step: 63,
        fine_lat: 17,
        fine_lon: 60,
        link: 0,
    })
}

/// A voice block of profile `p`: a test pattern of codec bits and the side channel of LY1ABC, call `call`.
fn voice(p: &Profile, call: u8) -> Vec<u8> {
    let per = p.k / 15 - 12;
    let codec: Vec<Vec<u8>> = (0..15).map(|f| (0..per).map(|i| ((i * 7 + f * 3 + usize::from(call)) % 5 == 0) as u8).collect()).collect();
    let side = VoiceSide { callsign: "LY1ABC".into(), call, slow: vec![0; 108] };
    frame::voice_block(p, &codec, &side).unwrap()
}

/// The `K`-bit blocks a profile sends in the tests: a chat in profiles 1 to 5 (in profile 2 and 5 a longer chat that fills 2
/// and 3 blocks), the short beacon in profile 0, voice in 6 and 7.
fn payload(p: &Profile) -> Vec<Vec<u8>> {
    match p.id {
        0 => blocks(&beacon().bits().unwrap(), 0, p).unwrap(),
        2 | 5 => {
            // 60 characters fill 2 blocks of profile 2, 95 fill 3 of profile 5.
            let text: String = "CQ CQ DE LY1ABC LY1ABC KO24 TEST OF A LONGER CHAT IN MORE THAN ONE BLOCK OF THE PROFILE, 73 DE LY1ABC"
                .chars()
                .take(if p.id == 2 { 60 } else { 95 })
                .collect();
            let long = Message::Chat(Chat {
                text,
                ..match chat() {
                    Message::Chat(c) => c,
                    _ => unreachable!(),
                }
            });
            let b = blocks(&long.bits().unwrap(), 1, p).unwrap();
            assert_eq!(b.len(), if p.id == 2 { 2 } else { 3 });
            b
        }
        6 | 7 => vec![voice(p, 5)],
        _ => blocks(&chat().bits().unwrap(), 1, p).unwrap(),
    }
}

/// Noise of `snr_db` in 2.4 kHz for a burst of mean power `p`.
fn noise_var(p: f64, snr_db: f64) -> f64 {
    p / 10f64.powf(snr_db / 10.0)
}

/// A stream that starts 1.5 s before a slot: the burst of `blocks` starts `lead` samples in (fractional), carrier `hz`, noise
/// for `snr_db`, then through 12 kHz audio as 16-bit samples and back to baseband.
fn through_audio(p: &Profile, blocks: &[Vec<u8>], lead: f64, hz: f64, snr_db: f64, seed: u64) -> (Vec<Complex32>, f64) {
    let x = burst(p, blocks);
    let pw = power(&x);
    let whole = lead.floor() as usize;
    let mut y = vec![Complex32::new(0.0, 0.0); whole];
    y.extend_from_slice(&x);
    y.resize(y.len() + 2 * 2400, Complex32::new(0.0, 0.0));
    shift(&mut y, hz, RATE);
    Noise::new(seed).add(&mut y, noise_var(pw, snr_db));
    let a = to_audio(&y, 0.9);
    // The fraction of a sample as a delay of the 12 kHz audio.
    let d = (lead - whole as f64) * 5.0;
    let r = sdrkit_algo_dsp::interp::Reader::new(sdrkit_algo_dsp::interp::Cubic);
    let a: Vec<f32> = (0..a.len()).map(|i| r.at(&a, i as f64 - d)).collect();
    // 16-bit samples, rounded half away from zero.
    let q: Vec<f32> = a.iter().map(|&v| (v.clamp(-1.0, 1.0) * 32767.0).round() / 32768.0).collect();
    (from_audio(&q), pw)
}

fn receive(x: &[Complex32]) -> Vec<Burst> {
    let mut rx = Receiver::new([true; 8]);
    let mut out = Vec::new();
    for c in x.chunks(4000) {
        rx.process(c, &mut out);
    }
    rx.finish(&mut out);
    out
}

/// Section 14.2: per profile, one burst at the design SNR + 9 dB (profile 2 with 2 blocks, 5 with 3), through audio and a
/// 16-bit file. The burst starts 1.5 s - 1.2 s (even profiles) or 1.5 s + 1.7 s (odd) after the slot start, plus 0.37
/// samples, the stream 1.5 s before the slot start; carrier -77 Hz (even) or +83 Hz (odd). Detection, clock error within 2 ms,
/// carrier within 1 Hz, SNR within 3 dB, every block bit-exact, burst power 0.85 to 1.15, burst length the layout total.
#[test]
fn every_profile_with_carrier_and_clock_offsets() {
    for p in &PROFILES {
        let odd = p.id % 2 == 1;
        let lead = (if odd { 3.0 + 1.7 } else { 3.0 - 1.2 }) * RATE + 0.37;
        let hz = if odd { 83.0 } else { -77.0 };
        let snr = p.design_snr_db + 9.0;
        let sent = payload(p);
        let x = burst(p, &sent);
        assert_eq!(x.len(), layout(p, sent.len()).total);
        let pw = power(&x);
        assert!((0.85..=1.15).contains(&pw), "profile {}: power {pw}", p.id);
        let (y, _) = through_audio(p, &sent, lead, hz, snr, 100 + u64::from(p.id));
        let got = receive(&y);
        let b = got.iter().find(|b| b.profile == p.id).unwrap_or_else(|| panic!("profile {}: no burst, {got:?}", p.id));
        let clock = (b.start - lead) / RATE;
        assert!(clock.abs() < 0.002, "profile {}: clock error {clock} s", p.id);
        assert!((b.offset_hz - hz).abs() < 1.0, "profile {}: carrier {}", p.id, b.offset_hz);
        assert!((b.snr_db - snr).abs() < 3.0, "profile {}: SNR {} for {snr}", p.id, b.snr_db);
        assert_eq!(b.blocks.len(), sent.len(), "profile {}", p.id);
        for (i, (g, s)) in b.blocks.iter().zip(&sent).enumerate() {
            assert_eq!(g.as_ref(), Some(s), "profile {} block {i}", p.id);
        }
        eprintln!("profile {}: clock {:.4} s, carrier {:.2} Hz, SNR {:.1} dB for {snr}", p.id, clock, b.offset_hz, b.snr_db);
    }
}

/// Section 14.2, noise only: 30000 samples of uniform noise give no burst; a stream of 100 samples gives none and does not
/// fail.
#[test]
fn noise_alone_and_a_short_stream() {
    let mut g = 0x2545_F491_4F6C_DD1Du64;
    let mut uni = || {
        g ^= g << 13;
        g ^= g >> 7;
        g ^= g << 17;
        (g >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
    };
    let x: Vec<Complex32> = (0..30_000).map(|_| Complex32::new(uni(), uni())).collect();
    assert!(receive(&x).is_empty());
    assert!(receive(&x[..100]).is_empty());
}

#[test]
fn blocks_of_the_bursts_parse() {
    let p = &PROFILES[3];
    let b = payload(p);
    assert!(matches!(parse_block(&b[0]), Some(Block::Part { id: 1, index: 0, .. })));
}

/// Complex 12 kHz samples whose 0 Hz is the dial (upper sideband) to baseband.
fn from_dial(x: &[Complex32]) -> Vec<Complex32> {
    let mut m = x.to_vec();
    shift(&mut m, -1500.0, 12_000.0);
    sdrkit_stack_custom::hf_chat::phy::from_channel(&m)
}

/// The messages that the blocks of `bursts` complete.
fn messages(bursts: &[Burst]) -> Vec<Message> {
    let mut a = frame::Assembler::default();
    let mut out = Vec::new();
    for b in bursts {
        let p = &PROFILES[usize::from(b.profile)];
        for bits in b.blocks.iter().flatten() {
            if let Some(Block::Part { id, index, payload }) = parse_block(bits) {
                if let frame::Taken::Complete(_, Some(m)) = a.take(id, index, &payload, p.k, b.start / RATE) {
                    out.push(m);
                }
            }
        }
    }
    out
}

/// Section 15.1: the clean baseband files of profiles 0 to 5 hold the transmitter's burst after 1.5 s of zeros, and each
/// decodes bit-exact, the first unit at sample 3600; the files with noise of profile 0 (a chat over several bursts, -12 dB)
/// and profile 3 (a short beacon, +3 dB) at 12 kHz decode, in both sample forms. Skipped without the recordings.
#[test]
fn recordings() {
    let Some(root) = common::recordings() else {
        return;
    };
    let dir = common::folder_with(&root, "_p3_chat.cf32");
    for p in &PROFILES[..6] {
        let name = if p.id == 0 { "_p0_beacon.cf32".to_string() } else { format!("_p{}_chat.cf32", p.id) };
        let x = common::read_cf32(&common::recording(&dir, &name));
        let sent = if p.id == 0 { blocks(&beacon().bits().unwrap(), 0, p).unwrap() } else { blocks(&chat().bits().unwrap(), 1, p).unwrap() };
        let mine = burst(p, &sent);
        assert_eq!(x.len(), 3600 + mine.len() + 2400, "{name}");
        let err = x[3600..3600 + mine.len()].iter().zip(&mine).map(|(a, b)| (a - b).norm()).fold(0f32, f32::max);
        assert!(err < 1e-3, "{name}: largest difference {err}");
        let got = receive(&x);
        let b = got.iter().find(|b| b.profile == p.id).unwrap_or_else(|| panic!("{name}: {got:?}"));
        assert!((b.start - 3600.0).abs() < 0.5, "{name}: start {}", b.start);
        assert_eq!(b.blocks.first().cloned().flatten().as_ref(), Some(&sent[0]), "{name}");
    }
    // Profile 3: a short beacon of LY1TST in one block, +3 dB. Profile 0: a chat of three blocks, -12 dB, its first two in
    // two caller slots 240 s apart (the file ends before the third).
    for (stem, profile, snr, bursts) in [("_P3_beacon_mid", 3u8, 3.0, 1usize), ("_P0_chat_weak", 0, -12.0, 2)] {
        for (name, rate, x) in common::both_forms(&dir, stem) {
            assert_eq!(rate, 12_000.0);
            let got = receive(&from_dial(&x));
            assert_eq!(got.len(), bursts, "{name}: {got:?}");
            for (i, b) in got.iter().enumerate() {
                eprintln!("{name}: start {:.1}, carrier {:.2} Hz, SNR {:.1} dB", b.start, b.offset_hz, b.snr_db);
                assert_eq!(b.profile, profile, "{name}");
                assert!((b.start - (3600.0 + 576_000.0 * i as f64)).abs() < 0.5, "{name}: start {}", b.start);
                assert!((b.snr_db - snr).abs() < 4.0, "{name}: SNR {}", b.snr_db);
                let Some(Some(Block::Part { id, index, payload })) = b.blocks.first().map(|x| x.as_ref().and_then(|x| parse_block(x))) else {
                    panic!("{name}: {b:?}");
                };
                assert_eq!(usize::from(index), i, "{name}");
                if profile == 0 {
                    // The file's chat is not described: block 0 starts a chat frame, both blocks are of message 1.
                    assert_eq!(id, 1, "{name}");
                    if i == 0 {
                        assert_eq!(&payload[..5], &[0, 1, 0, 0, 1], "{name}");
                    }
                }
            }
            if profile == 3 {
                let m = messages(&got);
                assert!(matches!(&m[..], [Message::ShortBeacon(s)] if s.locator == "KO24RM"), "{name}: {m:?}");
            }
        }
    }
}


/// Runs the stack's graph on `x` at `rate`, the radio centred `centre` Hz above the dial (0 for none).
fn graph_run(chunks: impl Iterator<Item = Vec<Complex32>>, rate: f64, centre: f64) -> Results {
    let d = &hc::HF_CHAT;
    let mut rx = (d.make_receiver)(&StackConfig::new(Values::defaults(&(d.settings)()))).unwrap();
    rx.configure(&InputInfo::new(rate, 14.1e6 + centre, 14.1e6).recording()).unwrap();
    let mut out = Output::default();
    for c in chunks {
        rx.feed(&c, &mut out);
        out.clear();
    }
    rx.finish(&mut out);
    rx.results().clone()
}

/// The test signal of the stack through its graph with a carrier of +30 Hz (section 14.4, the synthetic source), at 250 ksps
/// and 2 Msps, the radio tuned 20 kHz above the dial as the test signal asks: the chat decodes, the offset within 20 Hz of
/// 30 Hz, the clock error under 3 s.
#[test]
fn test_signal_through_the_graph() {
    let d = &hc::HF_CHAT;
    for rate in [250_000.0, 2_000_000.0] {
        let mut g = (d.make_generator.unwrap())(&GenConfig::new(rate, Values::defaults(&(d.settings)())).with_seed(1)).unwrap();
        let centre = -g.station_offset_hz(d.tuning.tune_offset_hz);
        assert_eq!(centre, 20_000.0);
        let n = (rate * 25.0) as usize;
        let mut done = 0usize;
        let chunks = std::iter::from_fn(move || {
            if done >= n {
                return None;
            }
            let mut x = vec![Complex32::new(0.0, 0.0); 65_536];
            g.generate(&mut x);
            for (i, v) in x.iter_mut().enumerate() {
                let a = std::f64::consts::TAU * (30.0 * (done + i) as f64 / rate).fract();
                *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
            }
            done += x.len();
            Some(x)
        });
        let r = graph_run(chunks, rate, centre);
        let f = r.fields();
        let case = format!("{rate} sps");
        assert_eq!(f.at(&Hc::MESSAGES), Some(1), "{case}");
        assert_eq!(f.at(&Hc::BLOCKS_BAD), Some(0), "{case}");
        assert!((f.get(&keys::OFFSET_HZ).unwrap() - 30.0).abs() < 20.0, "{case}");
        assert!(f.at(&Hc::CLOCK_ERROR_S).unwrap().abs() < 3.0, "{case}");
        assert_eq!(f.get(&keys::CALLSIGN), Some("TEST1"), "{case}");
    }
}

/// `x` read `d` samples late through a windowed-sinc interpolator (the baseband fills its band).
fn delayed(x: &[Complex32], d: f64) -> Vec<Complex32> {
    let r = sdrkit_algo_dsp::interp::Reader::new(sdrkit_algo_dsp::interp::Polyphase::<16, 64>::new(0.95, 5.0).unwrap());
    (0..x.len() + d.ceil() as usize).map(|i| r.at(x, i as f64 - d)).collect()
}

/// A fading channel (section 14.1): two paths of equal mean power, the second `delay` samples late, each gain a sum of 32
/// sinusoids of Gaussian frequencies (standard deviation half the Doppler spread) and uniform phases over sqrt 32, computed
/// every 24 samples and joined by straight lines.
fn fading(x: &[Complex32], delay: f64, spread_hz: f64, seed: u64) -> Vec<Complex32> {
    let mut n = Noise::new(seed);
    let late = delayed(x, delay);
    let tones: Vec<Vec<(f64, f64)>> = (0..2)
        .map(|_| {
            (0..32)
                .map(|_| {
                    let f = f64::from(n.sample(2.0 * (spread_hz / 2.0).powi(2)).re);
                    let ph = f64::from(n.sample(1.0).arg());
                    (f, ph)
                })
                .collect()
        })
        .collect();
    let at = |p: usize, i: usize| {
        let t = i as f64 / RATE;
        let s: Complex32 = tones[p].iter().map(|&(f, ph)| { let a = std::f64::consts::TAU * f * t + ph; Complex32::new(a.cos() as f32, a.sin() as f32) }).sum();
        s / (32f32).sqrt()
    };
    let g = |p: usize, i: usize| {
        let (k, r) = (i / 24, (i % 24) as f32 / 24.0);
        at(p, k * 24) * (1.0 - r) + at(p, (k + 1) * 24) * r
    };
    (0..late.len())
        .map(|i| (x.get(i).copied().unwrap_or_default() * g(0, i) + late[i] * g(1, i)) * std::f32::consts::FRAC_1_SQRT_2)
        .collect()
}

/// `burst` after `lead` samples (fractional) and 1 s more at the end, through `channel`, moved by `hz`, with noise for
/// `snr_db` against the burst's own power.
fn stream(x: &[Complex32], lead: f64, hz: f64, snr_db: f64, channel: impl Fn(&[Complex32]) -> Vec<Complex32>, seed: u64) -> Vec<Complex32> {
    // The power of the burst as it goes into the channel: the interpolator of a fractional start takes a little of the band
    // edges.
    let x = if lead.fract() == 0.0 { x.to_vec() } else { delayed(x, lead.fract()) };
    let pw = power(&x);
    let mut y = vec![Complex32::new(0.0, 0.0); lead.floor() as usize];
    y.extend(x);
    y.resize(y.len() + 2400, Complex32::new(0.0, 0.0));
    let mut y = channel(&y);
    shift(&mut y, hz, RATE);
    Noise::new(seed).add(&mut y, noise_var(pw, snr_db));
    y
}

/// Section 14.3: a chat in profile 3 over the moderate channel (2 ms, 1 Hz) at +3 dB and +31 Hz, the burst 3 s + 11.3 samples
/// in; an acknowledgement in profile 3 over the same channel at +3 dB and -20 Hz; a short beacon in profile 0 over the poor
/// channel (5 ms, 2 Hz) at -9 dB and +12 Hz; two voice blocks in profiles 6 and 7 at the design SNR + 8 dB and +40 Hz. Each
/// decodes with its fields intact.
#[test]
fn fading_channels_and_voice() {
    let moderate = |x: &[Complex32]| fading(x, 4.8, 1.0, 31);
    let poor = |x: &[Complex32]| fading(x, 12.0, 2.0, 32);
    let p3 = &PROFILES[3];
    let sent = blocks(&chat().bits().unwrap(), 1, p3).unwrap();
    let y = stream(&burst(p3, &sent), 3.0 * RATE + 11.3, 31.0, 3.0, moderate, 41);
    let got = receive(&y);
    assert_eq!(messages(&got), vec![chat()], "chat over the moderate channel: {:?}", got.iter().map(|b| (b.profile, b.start, b.offset_hz, b.snr_db, b.blocks.iter().map(Option::is_some).collect::<Vec<_>>())).collect::<Vec<_>>());
    let ack = frame::Ack { callsign: "DL2XYZ".into(), id: 9, bitmap: 0x0001, snr_db: -3, profile: 3 };
    let y = stream(&burst(p3, &[ack.bits(p3.k).unwrap()]), 2.0 * RATE, -20.0, 3.0, moderate, 42);
    let got = receive(&y);
    let heard: Vec<Block> = got.iter().flat_map(|b| b.blocks.iter().flatten().filter_map(|x| parse_block(x))).collect();
    assert_eq!(heard, vec![Block::Ack(ack)], "acknowledgement over the moderate channel");
    let p0 = &PROFILES[0];
    let y = stream(&burst(p0, &blocks(&beacon().bits().unwrap(), 0, p0).unwrap()), 2.0 * RATE, 12.0, -9.0, poor, 43);
    assert_eq!(messages(&receive(&y)), vec![beacon()], "beacon over the poor channel");
    for p in &PROFILES[6..] {
        let sent = vec![voice(p, 5), voice(p, 6)];
        let y = stream(&burst(p, &sent), 2.0 * RATE, 40.0, p.design_snr_db + 8.0, |x| x.to_vec(), 44 + u64::from(p.id));
        let got = receive(&y);
        let b = got.iter().find(|b| b.profile == p.id).unwrap_or_else(|| panic!("profile {}: {got:?}", p.id));
        assert_eq!(b.blocks.iter().map(|x| x.as_ref()).collect::<Vec<_>>(), sent.iter().map(Some).collect::<Vec<_>>(), "profile {}", p.id);
        for (blk, call) in b.blocks.iter().flatten().zip([5, 6]) {
            let side = VoiceSide::parse(&frame::split_voice(p, blk).1).unwrap();
            assert_eq!((side.callsign.as_str(), side.call), ("LY1ABC", call), "profile {}", p.id);
        }
    }
}

/// The baseband `bb` (whose sample 0 is a slot start) at `rate` around a radio `centre` Hz above the dial, the carrier `hz`
/// off, with noise for `snr_db` in 2.4 kHz against a burst of unit power (`None` for no noise), in chunks of 0.25 s.
fn radio_chunks(bb: Vec<Complex32>, rate: f64, centre: f64, hz: f64, snr_db: Option<f64>, seed: u64) -> impl Iterator<Item = Vec<Complex32>> {
    let mut up = sdrkit_stack_custom::hf_chat::phy::ChannelUp::default();
    let mut rs = sdrkit_algo_dsp::resample::Resampler::<Complex32>::new(12_000.0, rate).unwrap();
    let mut noise = Noise::new(seed);
    let var = snr_db.map(|s| 10f64.powf(-s / 10.0) * rate / RATE);
    let step = (hz - centre) / rate;
    let (mut at, mut n) = (0usize, 0u64);
    std::iter::from_fn(move || {
        if at >= bb.len() {
            return None;
        }
        let mut ch = Vec::new();
        for &v in &bb[at..(at + 600).min(bb.len())] {
            up.push(v, &mut ch);
        }
        at += 600;
        let mut y = Vec::new();
        rs.process(&ch, &mut y);
        for v in y.iter_mut() {
            let a = std::f64::consts::TAU * (step * n as f64).fract();
            n += 1;
            *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
            if let Some(var) = var {
                *v += noise.sample(var);
            }
        }
        Some(y)
    })
}

/// The bursts `bursts` (slot index, blocks) of profile `p` on a baseband stream that starts at slot 0 and ends `tail` s
/// after the last burst.
fn slotted(p: &Profile, bursts: &[(i64, Vec<Vec<u8>>)], tail: f64) -> Vec<Complex32> {
    let slot = p.slot_s.unwrap();
    let mut x = Vec::new();
    for (s, b) in bursts {
        let at = ((*s as f64 * slot + 1.5) * RATE) as usize;
        let y = burst(p, b);
        x.resize(at, Complex32::new(0.0, 0.0));
        x.extend(y);
    }
    x.resize(x.len() + (tail * RATE) as usize, Complex32::new(0.0, 0.0));
    x
}

/// The rows of table `key` of `r`, newest first, as the values of `fields`.
fn rows(r: &Results, key: &str, fields: &[&sdrkit_core::interface::Column<sdrkit_core::publish::field::Text>]) -> Vec<Vec<String>> {
    let Some(t) = r.decl().table_id(key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first().map(|(_, row)| fields.iter().map(|f| row.at(f).unwrap_or_default().to_string()).collect()).collect()
}

/// Section 14.4: the full receiver at 250 ksps, with the dial at the centre and with the radio 20 kHz above it: a chat in
/// profile 3 at +3 dB and +-50 Hz, a short beacon in profile 0 at -8 dB and +-50 Hz, the first of the two slots of a chat in
/// profile 0 at -8 dB and +30 Hz (1 good block, no message), both slots without noise (the message after the second), a full
/// beacon in profile 3 at +3 dB. Each: offset within 8 Hz, clock error under 0.5 s, SNR within 6 dB, no bad block. Noise alone
/// on the grid of profile 3 gives no block.
#[test]
fn full_receiver_at_the_radio_rate() {
    let rate = 250_000.0;
    let (p0, p3) = (&PROFILES[0], &PROFILES[3]);
    let short_chat = Message::Chat(Chat { text: "CQ DE LY1ABC".into(), ..match chat() { Message::Chat(c) => c, _ => unreachable!() } });
    let two = blocks(&short_chat.bits().unwrap(), 2, p0).unwrap();
    assert_eq!(two.len(), 2);
    let full = Message::FullBeacon(frame::FullBeacon {
        callsign: "LY1ABC".into(),
        lat: 54.6872,
        lon: 25.2797,
        power_step: 140,
        link: 7,
        locator: "KO24RM".into(),
        text: "HELLO 73".into(),
    });
    struct Case {
        name: &'static str,
        p: &'static Profile,
        bursts: Vec<(i64, Vec<Vec<u8>>)>,
        snr: Option<f64>,
        hz: f64,
        messages: i64,
        beacons: i64,
        good: i64,
    }
    let cases = vec![
        Case { name: "chat, profile 3, +50 Hz", p: p3, bursts: vec![(0, blocks(&chat().bits().unwrap(), 1, p3).unwrap())], snr: Some(3.0), hz: 50.0, messages: 1, beacons: 0, good: 1 },
        Case { name: "chat, profile 3, -50 Hz", p: p3, bursts: vec![(0, blocks(&chat().bits().unwrap(), 1, p3).unwrap())], snr: Some(3.0), hz: -50.0, messages: 1, beacons: 0, good: 1 },
        Case { name: "short beacon, profile 0, +50 Hz", p: p0, bursts: vec![(0, blocks(&beacon().bits().unwrap(), 0, p0).unwrap())], snr: Some(-8.0), hz: 50.0, messages: 0, beacons: 1, good: 1 },
        Case { name: "short beacon, profile 0, -50 Hz", p: p0, bursts: vec![(0, blocks(&beacon().bits().unwrap(), 0, p0).unwrap())], snr: Some(-8.0), hz: -50.0, messages: 0, beacons: 1, good: 1 },
        Case { name: "chat, profile 0, first slot", p: p0, bursts: vec![(0, vec![two[0].clone()])], snr: Some(-8.0), hz: 30.0, messages: 0, beacons: 0, good: 1 },
        Case { name: "chat, profile 0, both slots", p: p0, bursts: vec![(0, vec![two[0].clone()]), (2, vec![two[1].clone()])], snr: None, hz: 30.0, messages: 1, beacons: 0, good: 2 },
        Case { name: "full beacon, profile 3", p: p3, bursts: vec![(0, blocks(&full.bits().unwrap(), 5, p3).unwrap())], snr: Some(3.0), hz: 0.0, messages: 0, beacons: 1, good: 1 },
    ];
    for c in &cases {
        for centre in [0.0, 20_000.0] {
            let case = format!("{}, radio {centre} Hz above the dial", c.name);
            let r = graph_run(radio_chunks(slotted(c.p, &c.bursts, 3.0), rate, centre, c.hz, c.snr, 7), rate, centre);
            let f = r.fields();
            assert_eq!(f.at(&Hc::MESSAGES), Some(c.messages), "{case}");
            assert_eq!(f.at(&Hc::BEACONS), Some(c.beacons), "{case}");
            assert_eq!((f.at(&Hc::BLOCKS_GOOD), f.at(&Hc::BLOCKS_BAD)), (Some(c.good), Some(0)), "{case}");
            let off = f.get(&keys::OFFSET_HZ).unwrap();
            assert!((off - c.hz).abs() < 8.0, "{case}: offset {off}");
            let clock = f.at(&Hc::CLOCK_ERROR_S).unwrap();
            assert!(clock.abs() < 0.5, "{case}: clock error {clock}");
            let snr = f.get(&keys::SNR_DB).unwrap();
            if let Some(s) = c.snr {
                assert!((snr - s).abs() < 6.0, "{case}: SNR {snr}");
            }
            if c.messages > 0 {
                let m = rows(&r, "messages", &[&HfChatMessageRow::CALLSIGN, &HfChatMessageRow::TO, &HfChatMessageRow::TEXT]);
                let m0 = if c.p.id == 0 { short_chat.clone() } else { chat() };
                let want = match m0 {
                    Message::Chat(ch) => vec![ch.from.clone(), ch.to.to_string(), ch.text.clone()],
                    _ => unreachable!(),
                };
                assert_eq!(m, vec![want], "{case}");
            }
            if c.beacons > 0 {
                let b = rows(&r, "beacons", &[&HfChatBeaconRow::CALLSIGN, &HfChatBeaconRow::LOCATOR, &HfChatBeaconRow::TEXT]);
                let text = if c.p.id == 3 { "HELLO 73" } else { "" };
                assert_eq!(b, vec![vec!["LY1ABC".to_string(), "KO24RM".into(), text.into()]], "{case}");
                if c.p.id == 3 {
                    let t = r.decl().table_id("beacons").and_then(|id| r.table(id)).unwrap();
                    let (_, row) = t.rows_newest_first().next().unwrap();
                    let (lat, _) = row.get(&keys::POSITION).unwrap();
                    assert!((lat - 54.6872).abs() < 1e-3, "{case}: latitude {lat}");
                }
            }
        }
    }
    // Noise alone, 60 s.
    let r = graph_run(radio_chunks(vec![Complex32::new(0.0, 0.0); 60 * 2400], rate, 0.0, 0.0, Some(0.0), 8), rate, 0.0);
    assert_eq!((r.fields().at(&Hc::BLOCKS_GOOD), r.fields().at(&Hc::MESSAGES)), (Some(0), Some(0)));
}

/// The samples a graph sends to its transmitter.
#[derive(Default)]
struct Air(std::sync::Mutex<Vec<Complex32>>);

impl sdrkit_graph::components::HostStreams for Air {
    fn write(&self, port: sdrkit_graph::components::HostPort, data: sdrkit_graph::components::HostData<'_>) {
        if let (sdrkit_graph::components::HostPort::Transmit, sdrkit_graph::components::HostData::Iq { samples, .. }) = (port, data) {
            self.0.lock().unwrap().extend_from_slice(samples);
        }
    }
}

/// Section 12.4, both ends in the stack's graphs at 250 ksps, each hearing what the other sends: the caller sends a chat of
/// two blocks of profile 3 in an acknowledged session (one block a slot); the answering station acknowledges in the next odd
/// slot and, at the SNR it hears, suggests profile 5; the caller cuts the chat again into blocks of profile 5 and sends them
/// all in one slot; the answering station has the chat, and the caller's session ends at the second acknowledgement.
#[test]
fn acknowledged_session_between_two_graphs() {
    use sdrkit_core::{Owner, Receiver as _, Value};
    use std::sync::Arc;
    let rate = 250e3;
    let d = &hc::HF_CHAT;
    let text: String = "CQ CQ DE TEST1, A CHAT OF ONE HUNDRED CHARACTERS IN AN ACKNOWLEDGED SESSION OF THE HF CHAT STACK 73".chars().take(100).collect();
    let make = |call: &str, text: &str| {
        let mut s = Values::defaults(&(d.settings)());
        let schema = s.schema().clone();
        s.set(schema.id("callsign").unwrap(), &Value::Text(call.into())).unwrap();
        s.set(schema.id("link").unwrap(), &Value::Choice(1)).unwrap();
        s.set(schema.id("text").unwrap(), &Value::Text(text.into())).unwrap();
        let mut g = hc::graph(&StackConfig::new(s)).unwrap();
        let air = Arc::new(Air::default());
        g.set_context(sdrkit_graph::RunContext { transmit: true, streams: Some(air.clone()), ..Default::default() });
        g.configure(&InputInfo::new(rate, 0.0, 0.0).recording()).unwrap();
        (g, air)
    };
    let (mut a, air_a) = make("TEST1", &text);
    let (mut b, air_b) = make("DL2XYZ", "");
    let send = a.commands().id("send").unwrap();
    let args = a.commands().check(send, &a.commands().def(send).unwrap().new_args()).unwrap();
    a.invoke(send, &args).unwrap();
    let n = (rate * 0.25) as usize;
    let (mut to_a, mut to_b): (Vec<Complex32>, Vec<Complex32>) = (Vec::new(), Vec::new());
    let mut out = Output::default();
    for _ in 0..(100.0 / 0.25) as usize {
        let mut xa: Vec<Complex32> = to_a.drain(..to_a.len().min(n)).collect();
        xa.resize(n, Complex32::new(0.0, 0.0));
        let mut xb: Vec<Complex32> = to_b.drain(..to_b.len().min(n)).collect();
        xb.resize(n, Complex32::new(0.0, 0.0));
        a.feed(&xa, &mut out);
        b.feed(&xb, &mut out);
        out.clear();
        to_b.extend(std::mem::take(&mut *air_a.0.lock().unwrap()));
        to_a.extend(std::mem::take(&mut *air_b.0.lock().unwrap()));
    }
    let (fa, fb) = (a.results().fields().clone(), b.results().fields().clone());
    assert_eq!(fb.at(&Hc::MESSAGES), Some(1), "the answering station's messages");
    let r = b.results().clone();
    assert_eq!(rows(&r, "messages", &[&HfChatMessageRow::CALLSIGN, &HfChatMessageRow::TEXT]), vec![vec!["TEST1".to_string(), text.clone()]]);
    assert_eq!(fb.at(&Hc::PROFILE), Some(5), "the second burst in the suggested profile");
    assert_eq!(fa.at(&Hc::ACKS_HEARD), Some(2), "acknowledgements the caller heard");
    assert_eq!(fa.get(&keys::CALLSIGN), Some("DL2XYZ"));
    assert_eq!(fb.at(&Hc::BLOCKS_BAD), Some(0));
}

/// Blocks of profile `p` that fail of `n` sent on the moderate fading channel at `snr_db`, each in its own stream with its
/// own fading, carrier (+10 to -10 Hz) and start; a block lost to sync is a failure.
fn moderate_failures(p: &Profile, snr_db: f64, n: usize) -> usize {
    (0..n)
        .filter(|&t| {
            let seed = 1000 * u64::from(p.id) + t as u64;
            let info: Vec<u8> = (0..p.k).map(|i| ((i as u64 * 2_654_435_761 + seed * 40_503) >> 7 & 1) as u8).collect();
            let sent = frame::block_bits((t % 16) as u8, 0, &info[8..p.k - 16], p.k);
            let y = stream(&burst(p, std::slice::from_ref(&sent)), 2.0 * RATE + (t % 7) as f64 * 0.3, 10.0 - (t % 5) as f64 * 5.0, snr_db, |x| fading(x, 4.8, 1.0, seed), seed + 7);
            !receive(&y).iter().any(|b| b.profile == p.id && b.blocks.first() == Some(&Some(sent.clone())))
        })
        .count()
}

/// Section 14.6 (proposed), OFDM: profile 5 on the moderate fading channel at its measured 10 % block error SNR (section
/// 12.5, 0.0 dB) + 1 dB loses at most 10 % of 40 blocks.
#[test]
fn block_error_of_profile_5_on_the_moderate_channel() {
    let bad = moderate_failures(&PROFILES[5], 1.0, 40);
    assert!(bad <= 4, "{bad} of 40 blocks failed");
}

/// Section 14.6 (proposed), chirps: profiles 0 to 3 on the moderate fading channel at their measured 10 % block error SNRs
/// (section 12.5) + 1 dB lose at most 10 % of the blocks. Profile 4 never reached 10 % there, so it has no value to meet.
#[test]
fn block_error_of_the_chirp_profiles_on_the_moderate_channel() {
    let mut failed = Vec::new();
    for (p, n) in PROFILES[..4].iter().zip([16usize, 24, 40, 40]) {
        let snr = sdrkit_stack_custom::hf_chat::arq::THRESHOLDS_DB[usize::from(p.id)] + 1.0;
        let bad = moderate_failures(p, snr, n);
        if bad * 10 > n {
            failed.push(format!("profile {} at {snr:.1} dB: {bad} of {n} blocks failed", p.id));
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
