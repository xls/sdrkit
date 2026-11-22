//! The sender of the D-STAR stack: the headers, the slow-data block and the CRC equal the worked examples of the
//! specification; the stack's own receiver decodes what it sends (the header fields, the text, the voice frames of the
//! silence pattern byte for byte, the end pattern), and with the header lost it takes the header from the slow data; a
//! voice transmission decodes to the speech frames sent, and push-to-talk sends the frames of the microphone audio,
//! which sound with its pitch, with the text message and the header copy in the slow data; frames garbled in the middle
//! of a transmission are lost frames, so the sound fades and keeps its timing; the signal fits its channel at its level; halt ends with the end pattern; a text or a
//! voice transmission is refused without an own call sign and with the transmitter off; the time-out ends a voice
//! transmission. Nothing goes to a radio.

use crate::common::*;
use crate::speech::{pitch_hz, speech_codecs, vowel, vowel_frames, VOWEL_F0};
use crate::tx_host::*;
use sdrkit_algo_dsp::bandwidth::power_share_edges;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_codec_mbe::{Coder, Encoder};
use sdrkit_core::codec::{AudioCodec, CodecSet};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::media::Codec;
use sdrkit_core::{CommandError, Complex32, InputInfo, Owner, Receiver, StackConfig, Value};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_dvoice::dstar::{
    end_bits, slow_blocks, start_bits, Header, VoiceFrames, SLOW_XOR,
};
use sdrkit_stack_dvoice::dstar_tx::{silence_frame, DstarTxError, Sender, NOTE};
use sdrkit_stack_dvoice::gen::{FskModulator, Prepared, TxPulse};
use sdrkit_stack_dvoice::sender::{FskBurst, PEAK, RISE_S};
use sdrkit_stack_dvoice::typed::{DstarTxFields, DstarTxKind};
use sdrkit_stack_dvoice::voice::channel_bits;
use serde_json::Value as Json;
use std::sync::{Arc, Mutex};

/// The dial of the transmitting stack.
const DIAL: f64 = 145.375e6;

fn direct(my: &str) -> Header {
    Header {
        flags: [0, 0, 0],
        rpt2: "DIRECT".into(),
        rpt1: "DIRECT".into(),
        ur: "CQCQCQ".into(),
        my: my.into(),
        suffix: String::new(),
    }
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// The two headers, the slow-data block of the message piece "HELLO" with its scrambled halves, and the CRC check value
/// equal the worked examples of the specification (7.8), computed there from the JARL D-STAR specification.
#[test]
fn the_headers_and_the_slow_data_match_the_worked_examples() {
    let h = Header {
        my: "VK2ABC".into(),
        ..direct("")
    };
    assert_eq!(
        h.to_bytes().to_vec(),
        hex("00 00 00 44 49 52 45 43 54 20 20 44 49 52 45 43 54 20 20 43 51 43 51 43 51 20 20 56 4B 32 41 42 43 20 20 20 20 20 20 33 90")
    );
    let r = Header {
        flags: [0x40, 0, 0],
        rpt2: "VK2RAA G".into(),
        rpt1: "VK2RAA C".into(),
        ..h.clone()
    };
    assert_eq!(
        r.to_bytes().to_vec(),
        hex("40 00 00 56 4B 32 52 41 41 20 47 56 4B 32 52 41 41 20 43 43 51 43 51 43 51 20 20 56 4B 32 41 42 43 20 20 20 20 20 20 15 19")
    );
    let blocks = slow_blocks(&h, "HELLO", &[], None);
    assert_eq!(blocks[0], [0x40, 0x48, 0x45, 0x4C, 0x4C, 0x4F]);
    let air =
        |three: &[u8]| -> Vec<u8> { three.iter().zip(SLOW_XOR).map(|(a, k)| a ^ k).collect() };
    assert_eq!(air(&blocks[0][..3]), [0x30, 0x07, 0xD6]);
    assert_eq!(air(&blocks[0][3..]), [0x3C, 0x03, 0xDC]);
    assert_eq!(
        sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC.checksum(b"123456789"),
        0x906E
    );
}

/// The AMBE frames that the stack's own receiver hands to its plug-in, and its view of the results, for `x` at [`RATE`].
fn receive(x: &[Complex32]) -> (Json, Vec<Vec<u8>>) {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let mut set = CodecSet::default();
    let k = Arc::clone(&kept);
    set.register(
        Codec::Ambe.fourcc(),
        None,
        Arc::new(move |_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Keep(Arc::clone(&k))) as Box<dyn AudioCodec>)
        }),
    );
    let mut rx = receiver_with_codecs("dstar", &[], set);
    let (r, _) = run(rx.as_mut(), RATE, x, 4800);
    let frames = kept.lock().unwrap().clone();
    (r, frames)
}

/// The 48 kHz samples `s48` at [`RATE`], with 0.2 s of silence before them.
fn on_air(s48: &[Complex32]) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(s48);
    let mut g = Prepared::new(x, RATE, false);
    generate_all(&mut g)
}

/// Runs `s` to its end; the 48 kHz samples.
fn samples(mut s: Sender) -> Vec<Complex32> {
    let mut out = Vec::new();
    while s.next(&mut out) {}
    out
}

/// The receiver decodes the header fields of a repeater transmission, the text, and every voice frame as the silence
/// frame byte for byte; the end pattern ends the call.
#[test]
fn the_receiver_decodes_the_header_the_text_and_the_silence_frames() {
    let h = Header {
        flags: [0x40, 0, 0],
        rpt2: "VK2RAA G".into(),
        rpt1: "VK2RAA C".into(),
        ur: "CQCQCQ".into(),
        my: "VK2ABC".into(),
        suffix: "ID51".into(),
    };
    let s = Sender::text(&h, "HELLO FROM VK2ABC");
    let n = s.frames_planned();
    assert_eq!(n, 42, "13 slow-data blocks in two groups of 21 frames");
    let (r, got) = receive(&on_air(&samples(s)));
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    let d = &c[0]["details"];
    assert_eq!(
        (&c[0]["src"], &c[0]["dst"]),
        (&"VK2ABC".into(), &"CQCQCQ".into())
    );
    assert_eq!(
        (&d["my"], &d["suffix"], &d["ur"]),
        (&"VK2ABC".into(), &"ID51".into(), &"CQCQCQ".into())
    );
    assert_eq!(
        (&d["rpt1"], &d["rpt2"]),
        (&"VK2RAA C".into(), &"VK2RAA G".into())
    );
    assert_eq!(
        (&d["flag1"], &d["header_ok"], &d["header_from_slow_data"]),
        (&64.into(), &true.into(), &false.into())
    );
    assert_eq!(c[0]["text"], "HELLO FROM VK2ABC");
    assert_eq!(c[0]["end"], "terminator");
    assert_eq!(c[0]["voice_frames"], n);
    assert_eq!(got.len(), n);
    // The payload holds the bits as sent, the first most significant: each byte of the pattern with its bits in the
    // other order.
    let silence = channel_bits::<9>(&silence_frame());
    assert_eq!(
        silence.map(u8::reverse_bits),
        [0x9E, 0x8D, 0x32, 0x88, 0x26, 0x1A, 0x3F, 0x61, 0xE8]
    );
    assert!(
        got.iter().all(|f| f[..] == silence),
        "every voice frame is the silence pattern"
    );
    assert_eq!(r["detail"]["messages"][0]["text"], "HELLO FROM VK2ABC");
}

/// With the start of the transmission lost (the receiver hears it from the first voice frame on), the header comes
/// from the slow-data copy.
#[test]
fn a_late_entry_takes_the_header_from_the_slow_data() {
    let h = direct("N0CALL");
    let s48 = samples(Sender::text(&h, "LATE ENTRY"));
    // The bit sync, the frame sync and the header: 739 bits at 10 samples a bit; cut a little into the first frame.
    let cut = (64 + 15 + 660 + 20) * 10;
    let (r, _) = receive(&on_air(&s48[cut..]));
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(c[0]["src"], "N0CALL");
    assert_eq!(c[0]["details"]["header_from_slow_data"], true);
    assert_eq!(c[0]["text"], "LATE ENTRY");
}

/// The 99 % power bandwidth is within the 6.25 kHz channel, the deviation is ±1.2 kHz, and the envelope is constant at 0.9
/// of full scale.
#[test]
fn the_signal_fits_its_channel_at_its_level() {
    let s48 = samples(Sender::text(&direct("N0CALL"), "BANDWIDTH"));
    let peak = s48.iter().map(|z| z.norm()).fold(0f32, f32::max);
    assert!((peak - PEAK).abs() < 1e-3, "peak {peak}");
    let mid = &s48[4000..s48.len() - 4000];
    assert!(
        mid.iter().all(|z| (z.norm() - PEAK).abs() < 1e-3),
        "constant envelope"
    );
    let n = 4096;
    let mut p = vec![0f32; n];
    welch(mid, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut p).unwrap();
    let pd: Vec<f64> = p.iter().map(|&v| f64::from(v)).collect();
    let (lo, hi) = power_share_edges(&pd, 0, n - 1, 0.99).unwrap();
    let bw = (hi - lo + 1) as f64 * 48_000.0 / n as f64;
    // GMSK has no overshoot: runs of equal bits reach the full deviation of 1.2 kHz and no more.
    let f = frequency(mid, 48_000.0);
    let dev = f.iter().fold(0f64, |m, v| m.max(v.abs()));
    println!("D-STAR occupied bandwidth {bw:.0} Hz, deviation {dev:.0} Hz");
    assert!((1150.0..=1205.0).contains(&dev), "deviation {dev} Hz");
    assert!(bw <= 6_250.0 && bw > 3_000.0, "occupied bandwidth {bw} Hz");
}

/// The D-STAR stack with `settings`, the transmitter on, and its host.
fn station(settings: &[(&str, &str)]) -> (Box<dyn Receiver>, Arc<Host>) {
    let mut rx = (desc("dstar").make_receiver)(&StackConfig::new(values("dstar", settings)))
        .expect("the stack takes the settings");
    rx.configure(&InputInfo::new(RATE, DIAL - 50e3, DIAL).recording())
        .unwrap();
    let host = Arc::new(Host::default());
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the graph of the stack");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(host.clone());
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, host)
}

/// The sender's fields: transmitting, what it sends, the frequency, the frames and the note.
fn fields(rx: &dyn Receiver) -> (bool, Option<DstarTxKind>, Option<f64>, i64, String) {
    let f = DstarTxFields::read(Owner::results(rx).fields()).expect("the sender's fields");
    (
        f.transmitting,
        f.tx_kind,
        f.tx_freq_hz.map(|h| h.0),
        f.tx_frames,
        f.tx_note.to_string(),
    )
}

fn refusal(e: &CommandError) -> Option<DstarTxError> {
    match e {
        CommandError::Failed { source } => source.downcast_ref::<DstarTxError>().cloned(),
        _ => None,
    }
}

fn send(rx: &mut dyn Receiver, text: &str) -> Result<(), CommandError> {
    call(rx, "send", &[("text", Value::Text(text.into()))])
}

/// `send` on the graph sends the header of the settings and the text; the results say what it sends and that the voice
/// frames are silence; halt ends a second transmission after the frame on air with the end pattern and the end mark.
#[test]
fn a_text_on_the_graph_carries_the_settings_and_halt_ends_it() {
    let (mut rx, host) = station(&[
        ("callsign", "n0call"),
        ("your_call", "VK2ABC"),
        ("suffix", "TX"),
    ]);
    idle(rx.as_mut(), &host, 0.2, false);
    send(rx.as_mut(), "73 DE N0CALL").unwrap();
    assert_eq!(
        refusal(&send(rx.as_mut(), "again").unwrap_err()),
        Some(DstarTxError::Busy)
    );
    idle(rx.as_mut(), &host, 0.3, false);
    let (on, kind, freq, _, note) = fields(rx.as_ref());
    assert!(on);
    assert_eq!(
        (kind, freq, note.as_str()),
        (Some(DstarTxKind::Text), Some(DIAL), NOTE)
    );
    idle(rx.as_mut(), &host, 1.5, false);
    let (on, _, _, frames, _) = fields(rx.as_ref());
    assert!(!on);
    assert_eq!(frames, 42);
    let b = host.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].2 && b[0].1 == DIAL);
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, got) = receive(&x);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (&c[0]["src"], &c[0]["dst"], &c[0]["text"]),
        (&"N0CALL".into(), &"VK2ABC".into(), &"73 DE N0CALL".into())
    );
    assert_eq!(
        (&c[0]["details"]["suffix"], &c[0]["details"]["rpt1"]),
        (&"TX".into(), &"DIRECT".into())
    );
    assert_eq!(got.len(), 42);
    // Halt after the start: the end pattern follows the frame on air.
    send(rx.as_mut(), "SHORT").unwrap();
    idle(rx.as_mut(), &host, 0.3, false);
    call(rx.as_mut(), "halt", &[]).unwrap();
    idle(rx.as_mut(), &host, 0.5, false);
    let b = host.bursts();
    assert_eq!(b.len(), 2);
    assert!(b[1].2, "the end mark came");
    let (on, _, _, frames, _) = fields(rx.as_ref());
    assert!(!on && frames < 42, "{frames}");
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[1].0);
    let (r, _) = receive(&x);
    let c = calls(&r);
    assert_eq!(
        (c.len(), &c[0]["end"]),
        (1, &"terminator".into()),
        "{}",
        show(&c)
    );
}

/// A voice transmission of 40 frames: the receiver reports the call of the header, ended by the end pattern, and hands
/// on the speech frames sent, bit for bit.
#[test]
fn a_voice_transmission_decodes_to_the_frames_sent() {
    let frames = vowel_frames::<72>(Coder::Dstar, 40);
    let mut s = Sender::voice(&direct("VK2ABC"), "", 40);
    assert!(s.speaking());
    let mut next = frames.iter();
    let mut s48 = Vec::new();
    while s.next_with(|f| *f = *next.next().expect("40 frames"), &mut s48) {}
    assert_eq!(s.frames_sent(), 40);
    let (r, got) = receive(&on_air(&s48));
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (
            &c[0]["src"],
            &c[0]["dst"],
            &c[0]["end"],
            &c[0]["voice_frames"]
        ),
        (
            &"VK2ABC".into(),
            &"CQCQCQ".into(),
            &"terminator".into(),
            &40.into()
        )
    );
    let want: Vec<Vec<u8>> = frames
        .iter()
        .map(|f| channel_bits::<9>(f).to_vec())
        .collect();
    assert!(got == want, "the speech frames, bit for bit");
}

/// The vowel at `t` seconds, as audio of full scale 1.
fn vowel_at(t: f64) -> f32 {
    let n = t * 8000.0;
    let (a, b) = (vowel(n.floor() as usize), vowel(n.floor() as usize + 1));
    let w = (n - n.floor()) as f32;
    (f32::from(a) * (1.0 - w) + f32::from(b) * w) / 32768.0
}

/// Push-to-talk sends the speech of the host's microphone: the receiver reports the call of the settings and gets the
/// frames that an encoder makes of the same audio at 8 kHz, bit for bit; the speech decoder plays them with the pitch
/// of the vowel; key up ends the transmission with the end pattern after the frame on air.
#[test]
fn push_to_talk_sends_the_microphone_bit_for_bit() {
    let (mut rx, host) = station(&[("callsign", "VK2ABC"), ("your_call", "N0CALL")]);
    host.mic.lock().unwrap().0 = (0..96_000).map(|i| vowel_at(i as f64 / 48_000.0)).collect();
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    ptt(rx.as_mut(), true).expect("a transmission on air goes on");
    assert_eq!(
        refusal(&send(rx.as_mut(), "hi").unwrap_err()),
        Some(DstarTxError::Busy)
    );
    idle(rx.as_mut(), &host, 1.0, true);
    let (on, kind, freq, _, note) = fields(rx.as_ref());
    assert!(on);
    assert_eq!(
        (kind, freq, note.as_str()),
        (Some(DstarTxKind::Voice), Some(DIAL), NOTE)
    );
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 0.5, true);
    let (on, _, _, sent, _) = fields(rx.as_ref());
    assert!(!on);
    assert!(
        (40..=60).contains(&sent),
        "{sent} voice frames for about 1 s"
    );
    let b = host.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    assert!(b[0].2 && b[0].1 == DIAL);
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, got) = receive(&x);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (&c[0]["src"], &c[0]["dst"], &c[0]["end"]),
        (&"VK2ABC".into(), &"N0CALL".into(), &"terminator".into())
    );
    assert_eq!(got.len() as i64, sent);
    // The same audio through a rate change to 8 kHz and a fresh encoder.
    let audio = host.mic.lock().unwrap().0.clone();
    let mut s8 = Vec::new();
    Resampler::<f32>::new(48_000.0, 8000.0)
        .unwrap()
        .process(&audio, &mut s8);
    let mut enc = Encoder::new(Coder::Dstar);
    let want: Vec<Vec<u8>> = s8
        .as_chunks::<160>()
        .0
        .iter()
        .take(got.len())
        .map(|c| {
            let pcm: [i16; 160] =
                std::array::from_fn(|i| (c[i].clamp(-1.0, 1.0) * 32767.0).round() as i16);
            let mut f = [0u8; 72];
            enc.encode(&pcm, &mut f);
            channel_bits::<9>(&f).to_vec()
        })
        .collect();
    assert!(
        got == want,
        "the speech frames of the microphone, bit for bit"
    );
    // The speech decoder of the stack plays the transmission with the pitch of the vowel.
    let (_, out) = run(
        receiver_with_codecs("dstar", &[], speech_codecs()).as_mut(),
        RATE,
        &x,
        4800,
    );
    let mut sound = Vec::new();
    let mut rate = 0.0;
    for p in out.audio_runs() {
        sound.extend_from_slice(p.samples);
        rate = p.rate_hz;
    }
    let f0 = pitch_hz(&sound[(0.3 * rate) as usize..(0.8 * rate) as usize], rate);
    println!(
        "D-STAR push-to-talk: {} s of sound, pitch {f0:.1} Hz",
        sound.len() as f64 / rate
    );
    assert!((f0 - VOWEL_F0).abs() < 0.04 * VOWEL_F0, "pitch {f0} Hz");
}

/// Push-to-talk with the setting `message`: the slow data of the voice frames carries the text and the header copy, so
/// the receiver shows the text with the call and as a message; with the start of the transmission lost it takes the
/// header from the slow data and shows the text again. The speech frames are those of the microphone, one per frame.
#[test]
fn push_to_talk_carries_the_text_message_and_the_header_copy() {
    let msg = "QRV 145.375 VK2ABC";
    let (mut rx, host) = station(&[("callsign", "VK2ABC"), ("message", msg)]);
    host.mic.lock().unwrap().0 = (0..96_000).map(|i| vowel_at(i as f64 / 48_000.0)).collect();
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 1.5, true);
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 0.5, true);
    let (on, _, _, sent, _) = fields(rx.as_ref());
    assert!(!on && sent >= 56, "{sent} voice frames");
    let b = host.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, got) = receive(&x);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (
            &c[0]["src"],
            &c[0]["text"],
            &c[0]["details"]["header_from_slow_data"]
        ),
        (&"VK2ABC".into(), &msg.into(), &false.into())
    );
    assert_eq!(r["detail"]["messages"][0]["text"], msg);
    assert_eq!(got.len() as i64, sent);
    // The bit sync, the frame sync and the header at 20 samples a bit; cut a little into the first frame.
    let cut = (64 + 15 + 660 + 20) * 20;
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0[cut..]);
    let (r, _) = receive(&x);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (
            &c[0]["src"],
            &c[0]["text"],
            &c[0]["details"]["header_from_slow_data"]
        ),
        (&"VK2ABC".into(), &msg.into(), &true.into())
    );
    // A text message that does not fit is not taken.
    let mut bad = values("dstar", &[]);
    let schema = (desc("dstar").settings)();
    bad.set(
        schema.id("message").unwrap(),
        &Value::Text("A TEXT OF MORE THAN TWENTY CHARACTERS".into()),
    )
    .unwrap();
    assert!((desc("dstar").make_receiver)(&StackConfig::new(bad)).is_err());
}

/// A codec plug-in that keeps every frame it gets, `None` for a lost frame.
struct Frames(Arc<Mutex<Vec<Option<Vec<u8>>>>>);

impl AudioCodec for Frames {
    fn decode(
        &mut self,
        frame: &[u8],
        erasure: bool,
        pcm: &mut sdrkit_core::codec::Pcm,
    ) -> Result<(), sdrkit_core::codec::CodecError> {
        self.0
            .lock()
            .unwrap()
            .push((!erasure).then(|| frame.to_vec()));
        pcm.rate_hz = 8000;
        pcm.channels = 1;
        pcm.samples.extend(std::iter::repeat_n(0, 160));
        Ok(())
    }
}

/// A voice transmission of 84 frames whose frames 25 to 50 (with the data sync of frame 42) are random bits on the same
/// carrier: the receiver hands those frames on as lost frames from the second one, so the speech decoder repeats the
/// last frame three times and then is silent; every other frame arrives bit for bit, and the sound keeps its timing.
#[test]
fn frames_garbled_mid_transmission_are_lost_and_the_sound_fades() {
    let speech = vowel_frames::<72>(Coder::Dstar, 84);
    let mut bits = start_bits(&direct("VK2ABC"));
    let mut frames = VoiceFrames::new(slow_blocks(&direct("VK2ABC"), "", &[], None).split_off(4));
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    for (k, f) in speech.iter().enumerate() {
        let at = bits.len();
        frames.push(f, &mut bits);
        if (25..=50).contains(&k) {
            for b in &mut bits[at..] {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *b = (seed >> 32) as u8 & 1;
            }
        }
    }
    bits.extend(end_bits());
    let mut burst = FskBurst::new(
        FskModulator::new(4800.0, TxPulse::Gaussian(0.5), 1200.0),
        48_000.0,
        RISE_S,
    );
    let mut s48 = Vec::new();
    burst.bits(&bits, &mut s48);
    burst.end(&mut s48);
    let x = on_air(&s48);
    // The frames the plug-in gets.
    let kept = Arc::new(Mutex::new(Vec::new()));
    let mut set = CodecSet::default();
    let k = Arc::clone(&kept);
    set.register(
        Codec::Ambe.fourcc(),
        None,
        Arc::new(move |_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Frames(Arc::clone(&k))) as Box<dyn AudioCodec>)
        }),
    );
    let (r, _) = run(
        receiver_with_codecs("dstar", &[], set).as_mut(),
        RATE,
        &x,
        4800,
    );
    let c = calls(&r);
    assert_eq!(
        (c.len(), &c[0]["end"]),
        (1, &"terminator".into()),
        "{}",
        show(&c)
    );
    let got = kept.lock().unwrap().clone();
    assert_eq!(
        got.len(),
        84,
        "one frame or lost frame for each frame position"
    );
    for (k, (g, f)) in got.iter().zip(&speech).enumerate() {
        if (26..=50).contains(&k) {
            assert!(g.is_none(), "frame {k} of random bits is a lost frame");
        } else if k != 25 {
            assert_eq!(
                g.as_deref(),
                Some(&channel_bits::<9>(f)[..]),
                "frame {k} bit for bit"
            );
        }
    }
    // The speech decoder: sound before, silence within the garbled frames after three repeats, sound after.
    let (_, out) = run(
        receiver_with_codecs("dstar", &[], speech_codecs()).as_mut(),
        RATE,
        &x,
        4800,
    );
    let mut sound = Vec::new();
    let mut rate = 0.0;
    for p in out.audio_runs() {
        sound.extend_from_slice(p.samples);
        rate = p.rate_hz;
    }
    let n = (rate / 50.0) as usize;
    assert_eq!(sound.len() / n, 84, "the sound keeps its timing");
    let rms =
        |k: usize| (sound[k * n..(k + 1) * n].iter().map(|v| v * v).sum::<f32>() / n as f32).sqrt();
    let level: Vec<f32> = (0..84).map(rms).collect();
    assert!(level[5..25].iter().all(|&v| v > 0.01), "speech before");
    assert!(
        level[30..=50].iter().all(|&v| v < 1e-4),
        "silence after the repeats"
    );
    assert!(level[55..80].iter().all(|&v| v > 0.01), "speech after");
}

/// Without an own call sign push-to-talk is refused with the error that names the setting, and with the transmitter
/// off it is refused as such; the time-out ends a voice transmission with the end pattern.
#[test]
fn voice_is_refused_and_the_time_out_ends_it() {
    let (mut rx, host) = station(&[]);
    idle(rx.as_mut(), &host, 0.2, false);
    assert_eq!(
        refusal(&ptt(rx.as_mut(), true).unwrap_err()),
        Some(DstarTxError::NoSource)
    );
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(matches!(
        ptt(rx.as_mut(), true),
        Err(CommandError::TransmitOff)
    ));
    assert!(host.bursts().is_empty());
    let (mut rx, host) = station(&[("callsign", "VK2ABC"), ("tx_timeout", "1")]);
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 2.0, true);
    let (on, _, _, sent, _) = fields(rx.as_ref());
    assert!(!on, "the time-out ended the transmission");
    assert_eq!(sent, 50, "1 s of voice frames");
    let b = host.bursts();
    assert_eq!(b.len(), 1);
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, got) = receive(&x);
    let c = calls(&r);
    assert_eq!(
        (c.len(), &c[0]["end"]),
        (1, &"terminator".into()),
        "{}",
        show(&c)
    );
    assert_eq!(got.len(), 50);
}

/// Without an own call sign a text is refused with the error that names the setting; a text that does not fit is
/// refused; with the transmitter off it is refused as such. Nothing goes on air.
#[test]
fn texts_are_refused_and_nothing_is_sent() {
    let (mut rx, host) = station(&[]);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = send(rx.as_mut(), "hi").unwrap_err();
    let why = refusal(&e).expect("a refusal of the sender");
    assert_eq!(
        (why.clone(), why.setting()),
        (DstarTxError::NoSource, Some("callsign"))
    );
    assert!(e.to_string().contains("callsign"), "{e}");
    let (mut rx, host) = station(&[("callsign", "N0CALL")]);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = send(rx.as_mut(), "A TEXT OF MORE THAN TWENTY CHARACTERS").unwrap_err();
    assert_eq!(refusal(&e), Some(DstarTxError::BadText));
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(matches!(
        send(rx.as_mut(), "hi"),
        Err(CommandError::TransmitOff)
    ));
    idle(rx.as_mut(), &host, 0.3, false);
    assert!(host.bursts().is_empty());
    // A call sign outside the alphabet is not taken.
    let mut bad = values("dstar", &[]);
    let schema = (desc("dstar").settings)();
    bad.set(
        schema.id("callsign").unwrap(),
        &Value::Text("N0-CALL!".into()),
    )
    .unwrap();
    assert!((desc("dstar").make_receiver)(&StackConfig::new(bad)).is_err());
}
