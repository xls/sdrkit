//! The sender of the M17 stack: the stack's own receiver decodes what it sends (identities, frame numbers, the Codec 2
//! frames bit for bit, packets), the link setup frame matches the worked example of the specification, the signal fits
//! its channel, push-to-talk off ends with the end-of-stream frame, and a transmission without an own call sign is
//! refused. Nothing goes to a radio.

use crate::common::*;
use crate::tx_host::*;
use sdrkit_algo_dsp::bandwidth::power_share_edges;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_codec_codec2::{Codebooks, Encoder, Mode, Table};
use sdrkit_core::codec::{AudioCodec, CodecError, CodecSet, Pcm};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::media::Codec;
use sdrkit_core::{
    Clock, CommandError, Complex32, InputInfo, Output, Owner, Receiver, StackConfig, Value,
};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_dvoice::gen::Prepared;
use sdrkit_stack_dvoice::m17::{
    self, encode_callsign, lsf_bytes, sms_payload, stream_frame, text_metas, unscramble_soft,
    Codecs, TypeField,
};
use sdrkit_stack_dvoice::m17_tx::{Header, M17Transmit, M17TxError, Sender};
use sdrkit_stack_dvoice::sender::PEAK;
use sdrkit_stack_dvoice::typed::{M17TxFields, M17TxKind};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The dial of the transmitting stack.
const DIAL: f64 = 433.475e6;
/// The clock of a host that times bursts.
const NOW: f64 = 1_800_000_000.0;

/// Codec 2 tables of the 3200 bit/s rate with made-up values of the right shape: the frames are exact, not good speech.
fn books() -> Arc<Codebooks> {
    made_up_books(Mode::M3200)
}

/// The Codec 2 frames that the stack's own receiver hands to its plug-in, and its view of the results, for `x` at `rate`.
fn receive(x: &[Complex32], rate: f64) -> (serde_json::Value, Vec<Vec<u8>>) {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let mut set = CodecSet::default();
    let k = Arc::clone(&kept);
    set.register(
        Codec::Codec2.fourcc(),
        None,
        Arc::new(move |_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Keep(Arc::clone(&k))) as Box<dyn AudioCodec>)
        }),
    );
    let mut rx = receiver_with_codecs("m17", &[], set);
    let (r, _) = run(rx.as_mut(), rate, x, 4800);
    let frames = kept.lock().unwrap().clone();
    (r, frames)
}

/// The 48 kHz samples of `s`, with silence around them, at [`RATE`].
fn on_air(s48: &[Complex32]) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(s48);
    x.extend(std::iter::repeat_n(Complex32::default(), 24_000));
    let mut g = Prepared::new(x, RATE, false);
    generate_all(&mut g)
}

/// The test header: from N0CALL to T3ST on channel access number `can`, with the META text `text`.
fn header(can: u8, text: &str) -> Header {
    Header {
        src: encode_callsign("N0CALL"),
        dst: encode_callsign("T3ST"),
        can,
        metas: if text.is_empty() {
            Vec::new()
        } else {
            text_metas(text)
        },
    }
}

/// Runs the voice stream `s` to its end with the payloads `frames`, keying up before the last of them; the 48 kHz samples.
fn voice_samples(mut s: Sender, frames: &[[u8; 16]]) -> Vec<Complex32> {
    let mut out = Vec::new();
    let mut k = 0;
    loop {
        if k + 1 >= frames.len() {
            s.stop();
        }
        let more = s.next(
            |p| {
                *p = frames[k.min(frames.len() - 1)];
                k += 1;
            },
            &mut out,
        );
        if !more {
            break;
        }
    }
    out
}

/// Runs the packet `s` to its end; the 48 kHz samples.
fn packet_samples(mut s: Sender) -> Vec<Complex32> {
    let mut out = Vec::new();
    while s.next(|_| unreachable!("a packet asks for no speech"), &mut out) {}
    out
}

fn payloads(n: usize, seed: u8) -> Vec<[u8; 16]> {
    (0..n)
        .map(|k| std::array::from_fn(|i| ((k * 16 + i) as u8).wrapping_mul(37).wrapping_add(seed)))
        .collect()
}

/// The link setup frame, the META text block, the SMS packet, the call sign coding and the CRC equal the worked examples
/// of the specification (8.8), computed there from the public M17 specification, not from this library.
#[test]
fn the_link_setup_frame_matches_the_worked_example() {
    // Base 40, first character least significant: N=14, 0=27, C=3, A=1, L=12, L=12.
    let n0call =
        14 + 27 * 40 + 3 * 40u64.pow(2) + 40u64.pow(3) + 12 * 40u64.pow(4) + 12 * 40u64.pow(5);
    assert_eq!(encode_callsign("N0CALL"), n0call);
    assert_eq!(encode_callsign("@ALL"), 0xFFFF_FFFF_FFFF);
    // Voice 3200, CAN 0: TYPE 0x0005; the worked example's source address 0x0000129F510E to broadcast, empty META.
    let typ = TypeField {
        stream: true,
        data_type: 2,
        ..Default::default()
    }
    .encode();
    assert_eq!(typ, 0x0005);
    let lsf = lsf_bytes(0xFFFF_FFFF_FFFF, 0x0000_129F_510E, typ, &[0; 14]);
    let mut want = vec![0xFF; 6];
    want.extend([0x00, 0x00, 0x12, 0x9F, 0x51, 0x0E, 0x00, 0x05]);
    want.extend([0; 14]);
    want.extend([0x2C, 0xC2]);
    assert_eq!(lsf.to_vec(), want);
    // The same with the META text HELLO in one block.
    let meta = text_metas("HELLO");
    assert_eq!(meta.len(), 1);
    assert_eq!(
        meta[0],
        [0x11, 0x48, 0x45, 0x4C, 0x4C, 0x4F, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]
    );
    let lsf = lsf_bytes(0xFFFF_FFFF_FFFF, 0x0000_129F_510E, typ, &meta[0]);
    assert_eq!(&lsf[28..], &[0x7B, 0x5E]);
    // The sender's header gives the same frame from the settings' values.
    let h = Header {
        src: 0x0000_129F_510E,
        dst: encode_callsign("@ALL"),
        can: 0,
        metas: Vec::new(),
    };
    assert_eq!(
        h.voice_lsfs(),
        vec![lsf_bytes(0xFFFF_FFFF_FFFF, 0x0000_129F_510E, 5, &[0; 14])]
    );
    // The CAN goes to bits 7 to 10, a packet clears bit 0 and sets data type 1.
    assert_eq!(header(9, "").voice_lsfs()[0][12..14], [0x04, 0x85]);
    assert_eq!(header(9, "").packet_lsf()[12..14], [0x04, 0x82]);
    // SMS "HI" and the CRC check values.
    assert_eq!(sms_payload("HI"), vec![0x05, 0x48, 0x49, 0x00, 0x13, 0xBA]);
    // The CRC of "123456789" is 0x772B: the protocol byte is the "1".
    assert_eq!(
        m17::packet_payload(b'1', b"23456789"),
        b"123456789\x77\x2B".to_vec()
    );
    // Sync words, the preamble (+3, -3 in turn) and the end of transmission.
    let lv = sdrkit_stack_dvoice::gen::bits_to_levels(&m17::preamble(true));
    assert_eq!(lv.len(), 192);
    assert!(lv.chunks(2).all(|p| p == [3.0, -3.0]));
    let word = |b: &[u8]| b[..16].iter().fold(0u16, |a, &x| (a << 1) | u16::from(x));
    assert_eq!(word(&m17::lsf_frame(&lsf)), 0x55F7);
    assert_eq!(word(&m17::eot()), 0x555D);
    let s = stream_frame(&Codecs::default(), &lsf, 0, false, &[0; 16]);
    assert_eq!((s.len(), word(&s)), (384, 0xFF5D));
}

/// The frames that the sender emits: the preamble, the link setup frame, the stream frames numbered from 0 with the
/// end-of-stream bit on the last one only and the LICH counter running 0 to 5, then the end of transmission. Read with the
/// receiver's decoders.
#[test]
fn stream_frames_count_and_the_last_one_ends_the_stream() {
    let frames = payloads(14, 5);
    let h = header(2, "");
    let mut s = Sender::voice(&h, 10_000);
    let mut units = Vec::new();
    let mut k = 0;
    loop {
        if k + 1 >= frames.len() {
            s.stop();
        }
        let Some(bits) = s.next_bits(|p| {
            *p = frames[k];
            k += 1;
        }) else {
            break;
        };
        units.push(bits);
    }
    assert_eq!(units.len(), 2 + frames.len() + 1);
    assert_eq!(units[0], m17::preamble(true));
    assert_eq!(units[1], m17::lsf_frame(&h.voice_lsfs()[0]));
    assert_eq!(units[units.len() - 1], m17::eot());
    assert_eq!(s.frames_sent(), frames.len() as u64);
    let mut codecs = Codecs::default();
    for (i, bits) in units[2..2 + frames.len()].iter().enumerate() {
        let last = i == frames.len() - 1;
        let soft: Vec<f32> = bits[16..]
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        let d = unscramble_soft(&soft);
        let (payload, errs) = codecs.stream.decode(&d[96..], 148, true).unwrap();
        assert_eq!(errs, 0);
        let bytes = sdrkit_stack_dvoice::bits::bits_to_bytes(&payload[..144]);
        let fnum = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!(fnum, i as u16 | if last { 0x8000 } else { 0 }, "frame {i}");
        assert_eq!(bytes[2..], frames[i]);
        let lich: Vec<u16> = (0..4)
            .map(|k| m17::decode_lich_word(&d[k * 24..k * 24 + 24]).unwrap().0)
            .collect();
        assert_eq!(lich[3] & 0xFF, ((i % 6) as u16) << 5);
    }
    assert!(s.next_bits(|_| unreachable!()).is_none());
}

/// The receiver decodes the sender's voice stream: source, destination, CAN, META text, every Codec 2 frame bit for bit,
/// and with the end of transmission cut off, the end-of-stream bit of the last frame ends the call.
#[test]
fn a_voice_stream_decodes_with_its_identities_and_every_codec_frame() {
    let frames = payloads(40, 3);
    let s48 = voice_samples(Sender::voice(&header(5, "HELLO T3ST"), 10_000), &frames);
    // Preamble, link setup frame, 40 stream frames, end of transmission, each 1920 samples, and the fall.
    assert!(
        s48.len() >= 43 * 1920 && s48.len() < 44 * 1920,
        "{}",
        s48.len()
    );
    let (r, got) = receive(&on_air(&s48), RATE);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(c[0]["src"], "N0CALL");
    assert_eq!(c[0]["dst"], "T3ST");
    assert_eq!(c[0]["access_code"], 5);
    assert_eq!(c[0]["text"], "HELLO T3ST");
    assert_eq!(c[0]["voice_frames"], 40);
    assert_eq!(c[0]["end"], "terminator");
    let want: Vec<Vec<u8>> = frames
        .iter()
        .flat_map(|f| f.chunks(8).map(<[u8]>::to_vec))
        .collect();
    assert_eq!(got, want, "every Codec 2 frame arrives as sent");
    // Without the end of transmission, the end-of-stream bit alone ends the call.
    // Four symbols of the end of transmission are left, too few for its sync word.
    let cut = 42 * 1920 + 80;
    let (r, _) = receive(&on_air(&s48[..cut]), RATE);
    let c = calls(&r);
    assert_eq!(
        (c.len(), &c[0]["end"]),
        (1, &serde_json::json!("terminator"))
    );
    assert_eq!(c[0]["voice_frames"], 40);
}

/// A short and a long SMS text and a data packet arrive whole, with their addresses.
#[test]
fn text_and_data_packets_arrive_whole() {
    let long: String = (0..300)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    for text in ["Hello from T3ST, 73", long.as_str()] {
        let s = Sender::packet(&header(1, ""), &sms_payload(text)).unwrap();
        assert_eq!(s.kind(), M17TxKind::Packet);
        let s48 = packet_samples(s);
        let (r, _) = receive(&on_air(&s48), RATE);
        let m = &r["detail"]["messages"];
        assert_eq!(m.as_array().map(Vec::len), Some(1), "{m}");
        assert_eq!(m[0]["text"], text);
        assert_eq!(m[0]["src"], "N0CALL");
        assert_eq!(m[0]["dst"], "T3ST");
    }
    let s = Sender::packet(&header(1, ""), &m17::packet_payload(0x07, &[1, 2, 3, 4, 5])).unwrap();
    let (r, _) = receive(&on_air(&packet_samples(s)), RATE);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (&c[0]["kind"], &c[0]["src"]),
        (&serde_json::json!("packet"), &serde_json::json!("N0CALL"))
    );
    assert_eq!(c[0]["details"]["protocol"], 7);
    // More than 33 frames do not fit in one packet.
    let too_long = "x".repeat(m17_tx_text_max() + 1);
    assert!(matches!(
        Sender::packet(&header(1, ""), &sms_payload(&too_long)),
        Err(M17TxError::TooLong { .. })
    ));
}

fn m17_tx_text_max() -> usize {
    sdrkit_stack_dvoice::m17_tx::TEXT_MAX
}

/// The 99 % power bandwidth of a voice stream is within the 9 kHz of the public M17 specification, and the carrier has
/// the level of 0.9 of full scale.
#[test]
fn the_signal_fits_its_channel_at_its_level() {
    let frames = payloads(60, 11);
    let s48 = voice_samples(Sender::voice(&header(0, ""), 10_000), &frames);
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
    assert!(bw <= 9_000.0, "occupied bandwidth {bw} Hz");
    assert!(bw > 6_000.0, "occupied bandwidth {bw} Hz");
}

// ---------------------------------------------------------------------------------------------------------------------------
// The stack on the graph.

/// The M17 stack with `settings`, the transmitter on, the test tables, and its host; with `lead`, on a live radio whose
/// host times bursts with that lead.
fn station(
    settings: &[(&str, &str)],
    tables: bool,
    lead: Option<Duration>,
) -> (Box<dyn Receiver>, Arc<Host>) {
    let mut rx = (desc("m17").make_receiver)(&StackConfig::new(values("m17", settings)))
        .expect("the stack takes the settings");
    let input = InputInfo::new(RATE, DIAL - 50e3, DIAL);
    rx.configure(&if lead.is_some() {
        input
    } else {
        input.recording()
    })
    .unwrap();
    let audio: Vec<f32> = (0..96_000)
        .map(|i| speechlike(i as f64 / 48_000.0))
        .collect();
    let host = Arc::new(Host {
        lead: lead.unwrap_or_default(),
        mic: Mutex::new((audio, 0, 0)),
        ..Host::default()
    });
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the graph of the stack");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(host.clone());
    ctx.clock = Clock::fixed(NOW);
    g.set_context(ctx);
    if tables {
        g.component_as_mut::<M17Transmit>("transmit")
            .expect("the sender")
            .set_codebooks(books());
    }
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, host)
}

fn fields(rx: &dyn Receiver) -> M17TxFields {
    M17TxFields::read(Owner::results(rx).fields()).expect("the sender's fields")
}

/// The refusal inside a command error.
fn refusal(e: &CommandError) -> Option<M17TxError> {
    match e {
        CommandError::Failed { source } => source.downcast_ref::<M17TxError>().cloned(),
        _ => None,
    }
}

/// Push-to-talk sends the speech of the host's microphone: the receiver gets the identities of the settings and the
/// Codec 2 frames that an encoder makes of the same audio at 8 kHz, bit for bit; key up ends with the end-of-stream frame.
#[test]
fn push_to_talk_sends_the_microphone_bit_for_bit() {
    let settings = [
        ("callsign", "n0call"),
        ("destination", "T3ST"),
        ("can", "3"),
        ("meta_text", "73 FROM T3ST"),
    ];
    let (mut rx, host) = station(&settings, true, None);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(!fields(rx.as_ref()).transmitting);
    ptt(rx.as_mut(), true).unwrap();
    ptt(rx.as_mut(), true).expect("a stream on air goes on");
    idle(rx.as_mut(), &host, 1.0, true);
    let f = fields(rx.as_ref());
    assert!(f.transmitting);
    assert_eq!(f.tx_kind, Some(M17TxKind::Voice));
    assert_eq!(f.tx_freq_hz.map(|h| h.0), Some(DIAL));
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 0.5, true);
    let f = fields(rx.as_ref());
    assert!(!f.transmitting);
    let b = host.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    let (samples, centre, ended, starts) = &b[0];
    assert!(*ended && starts.is_empty());
    assert_eq!(*centre, DIAL);
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(samples);
    let (r, got) = receive(&x, RATE);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (&c[0]["src"], &c[0]["dst"]),
        (&serde_json::json!("N0CALL"), &serde_json::json!("T3ST"))
    );
    assert_eq!(c[0]["access_code"], 3);
    assert_eq!(c[0]["text"], "73 FROM T3ST");
    assert_eq!(c[0]["end"], "terminator");
    let n = c[0]["voice_frames"].as_i64().unwrap();
    assert_eq!(n, f.tx_frames);
    assert!((24..=32).contains(&n), "{n} frames for about 1.1 s");
    // The same audio through a rate change to 8 kHz and a fresh encoder.
    let audio = host.mic.lock().unwrap().0.clone();
    let mut s8 = Vec::new();
    Resampler::<f32>::new(48_000.0, 8000.0)
        .unwrap()
        .process(&audio, &mut s8);
    let mut enc = Encoder::new(Mode::M3200, &books()).unwrap();
    let want: Vec<Vec<u8>> = s8
        .as_chunks::<160>()
        .0
        .iter()
        .take(2 * n as usize)
        .map(|c| {
            let pcm: Vec<i16> = c
                .iter()
                .map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i16)
                .collect();
            let mut f = vec![0u8; 8];
            enc.encode(&pcm, &mut f).unwrap();
            f
        })
        .collect();
    assert_eq!(got.len(), want.len());
    assert_eq!(
        got, want,
        "the Codec 2 frames of the microphone, bit for bit"
    );
}

/// Without an own call sign every transmit command is refused with the error that names the setting, and nothing goes on
/// air; with the transmitter off the command is refused as such.
#[test]
fn an_empty_source_is_refused_and_nothing_is_sent() {
    let (mut rx, host) = station(&[], true, None);
    idle(rx.as_mut(), &host, 0.2, false);
    for e in [
        ptt(rx.as_mut(), true).unwrap_err(),
        call(rx.as_mut(), "send", &[("text", Value::Text("hi".into()))]).unwrap_err(),
    ] {
        let why = refusal(&e).expect("a refusal of the sender");
        assert_eq!(why, M17TxError::NoSource);
        assert_eq!(why.setting(), Some("callsign"));
        assert!(e.to_string().contains("callsign"), "{e}");
    }
    idle(rx.as_mut(), &host, 0.3, false);
    assert!(host.bursts().is_empty());
    // A call sign outside the alphabet is not taken.
    let mut bad = values("m17", &[]);
    let schema = (desc("m17").settings)();
    bad.set(
        schema.id("callsign").unwrap(),
        &Value::Text("N0 CALL!".into()),
    )
    .unwrap();
    assert!((desc("m17").make_receiver)(&StackConfig::new(bad)).is_err());
    // The transmitter off.
    let (mut rx, host) = station(&[("callsign", "N0CALL")], true, None);
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(matches!(
        ptt(rx.as_mut(), true),
        Err(CommandError::TransmitOff)
    ));
    assert!(host.bursts().is_empty());
}

/// Without the Codec 2 tables push-to-talk is refused with the table that is missing; a packet needs none and is sent,
/// and halt ends it with the end mark.
#[test]
fn missing_tables_refuse_voice_but_not_a_packet() {
    let (mut rx, host) = station(&[("callsign", "N0CALL")], false, None);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    assert!(matches!(refusal(&e), Some(M17TxError::NoTable(_))), "{e}");
    call(
        rx.as_mut(),
        "send",
        &[("text", Value::Text("Hello from N0CALL".into()))],
    )
    .unwrap();
    assert!(matches!(
        call(
            rx.as_mut(),
            "send",
            &[("text", Value::Text("again".into()))]
        )
        .map_err(|e| refusal(&e)),
        Err(Some(M17TxError::Busy))
    ));
    idle(rx.as_mut(), &host, 0.6, false);
    let b = host.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].2, "the end mark came");
    let f = fields(rx.as_ref());
    assert_eq!(
        (f.transmitting, f.tx_kind, f.tx_frames),
        (false, Some(M17TxKind::Packet), 1)
    );
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, _) = receive(&x, RATE);
    assert_eq!(r["detail"]["messages"][0]["text"], "Hello from N0CALL");
    // A long data packet, ended by halt after its first frames: the end mark still comes.
    call(
        rx.as_mut(),
        "send_data",
        &[
            ("protocol", Value::I64(7)),
            ("data", Value::Bytes(vec![0x55; 700].into())),
        ],
    )
    .unwrap();
    idle(rx.as_mut(), &host, 0.1, false);
    call(rx.as_mut(), "halt", &[]).unwrap();
    idle(rx.as_mut(), &host, 0.5, false);
    let b = host.bursts();
    assert_eq!(b.len(), 2);
    assert!(b[1].2);
    let f = fields(rx.as_ref());
    assert!(
        f.tx_frames < 28,
        "halt dropped the frames not yet sent: {}",
        f.tx_frames
    );
}

/// On a live radio whose host times bursts, push-to-talk states its start once, on the first sample, the lead and at most
/// two passes after the clock; the samples are those of a transmission without a stated start.
#[test]
fn a_live_radio_gets_the_start_on_the_first_sample() {
    let lead = Duration::from_millis(300);
    let run = |lead: Option<Duration>| {
        let (mut rx, host) = station(&[("callsign", "N0CALL")], true, lead);
        idle(rx.as_mut(), &host, 0.2, false);
        ptt(rx.as_mut(), true).unwrap();
        idle(rx.as_mut(), &host, 0.5, true);
        ptt(rx.as_mut(), false).unwrap();
        idle(rx.as_mut(), &host, 0.3, true);
        let mut b = host.bursts();
        assert_eq!(b.len(), 1, "one transmission");
        b.remove(0)
    };
    let timed = run(Some(lead));
    let plain = run(None);
    assert!(plain.3.is_empty(), "no start without a lead");
    assert_eq!(timed.3.len(), 1, "one start");
    let (at, utc) = timed.3[0];
    assert_eq!(at, 0, "the start is on the first sample");
    let earliest = NOW + lead.as_secs_f64();
    assert!(
        utc >= earliest && utc <= earliest + 0.2 + 1e-6,
        "{}",
        utc - NOW
    );
    assert!(timed.2);
    assert_eq!(timed.0, plain.0, "the same transmission");
}

/// A table file in the storage folder that is not the right file refuses push-to-talk with its name.
#[test]
fn a_wrong_table_file_in_the_storage_is_refused() {
    let (mut rx, host) = station(&[("callsign", "N0CALL")], false, None);
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the graph of the stack");
    let storage = sdrkit_core::MemStorage::new();
    let name = Table::LspDelta(0).file_name();
    let path = sdrkit_core::storage::StorePath::new(&format!("codec2/{name}")).unwrap();
    sdrkit_core::storage::Storage::write(&storage, &path, &vec![0u8; 4 * Table::LspDelta(0).len()])
        .unwrap();
    let mut ctx: RunContext = g.context().clone();
    ctx.storage = Arc::new(storage);
    g.set_context(ctx);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    assert_eq!(refusal(&e), Some(M17TxError::BadTable(name)), "{e}");
}

/// The time-out ends a stream with the end-of-stream frame after its time; the transmitter going off cuts a stream, and
/// its end mark still comes.
#[test]
fn the_time_out_and_the_transmitter_off_end_a_stream() {
    let (mut rx, host) = station(&[("callsign", "N0CALL"), ("tx_timeout", "1")], true, None);
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 2.0, true);
    let f = fields(rx.as_ref());
    assert!(!f.transmitting);
    assert_eq!(f.tx_frames, 25, "1 s of 40 ms frames");
    let b = host.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].2, "the end mark came");
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, _) = receive(&x, RATE);
    let c = calls(&r);
    assert_eq!(
        (c.len(), &c[0]["end"]),
        (1, &serde_json::json!("terminator"))
    );
    assert_eq!(c[0]["voice_frames"], 25);
    // The transmitter goes off in the middle of a stream.
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 0.3, true);
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.3, true);
    let b = host.bursts();
    assert_eq!(b.len(), 2);
    assert!(b[1].2, "the cut stream ends with its end mark");
    assert!(!fields(rx.as_ref()).transmitting);
}

/// A codec plug-in that answers every frame with 20 ms of a steady level.
struct Loud;
impl AudioCodec for Loud {
    fn decode(&mut self, _frame: &[u8], _erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
        pcm.rate_hz = 8000;
        pcm.channels = 1;
        pcm.samples.extend(std::iter::repeat_n(8000, 160));
        Ok(())
    }
}

/// The receive audio of the stack is silent from key down to the end of the transmission, and comes back after key up
/// while the received stream goes on.
#[test]
fn receive_audio_is_muted_while_keyed() {
    let mut set = CodecSet::default();
    set.register(
        Codec::Codec2.fourcc(),
        None,
        Arc::new(|_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Loud) as Box<dyn AudioCodec>)
        }),
    );
    let mut rx = receiver_with_codecs("m17", &[("callsign", "N0CALL")], set);
    rx.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    let host = Arc::new(Host::default());
    {
        let g = rx
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<Graph>())
            .expect("the graph of the stack");
        let mut ctx: RunContext = g.context().clone();
        ctx.streams = Some(host.clone());
        g.set_context(ctx);
        g.component_as_mut::<M17Transmit>("transmit")
            .expect("the sender")
            .set_codebooks(books());
    }
    rx.set_transmit(true);
    // A received stream of 100 frames (4 s), fed in pieces of 0.1 s.
    let x = on_air(&voice_samples(
        Sender::voice(&header(0, ""), 1000),
        &payloads(100, 7),
    ));
    let mut out = Output::default();
    let mut peaks = Vec::new();
    for (i, piece) in x.chunks((RATE / 10.0) as usize).enumerate() {
        match i {
            15 => ptt(rx.as_mut(), true).unwrap(),
            25 => ptt(rx.as_mut(), false).unwrap(),
            _ => {}
        }
        out.clear();
        rx.feed(piece, &mut out);
        peaks.push(out.audio().iter().fold(0.0f32, |m, s| m.max(s.abs())));
    }
    assert!(peaks.len() > 40, "{} pieces", peaks.len());
    let heard = |r: std::ops::Range<usize>| peaks[r].iter().any(|p| *p > 0.1);
    assert!(heard(8..15), "audio before key down: {peaks:?}");
    assert!(
        peaks[17..=25].iter().all(|p| *p == 0.0),
        "no audio while keyed: {peaks:?}"
    );
    assert_eq!(host.bursts().len(), 1);
    assert!(heard(29..40), "audio after key up: {peaks:?}");
}
