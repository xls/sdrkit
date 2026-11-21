//! The sender of the D-STAR stack: the headers, the slow-data block and the CRC equal the worked examples of the
//! specification; the stack's own receiver decodes what it sends (the header fields, the text, the voice frames of AMBE
//! silence byte for byte, the end pattern), and with the header lost it takes the header from the slow data; the signal
//! fits its channel at its level; halt ends with the end pattern; a text is refused without an own call sign and with
//! the transmitter off. Nothing goes to a radio.

use crate::common::*;
use crate::tx_host::*;
use sdrkit_algo_dsp::bandwidth::power_share_edges;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::codec::{AudioCodec, CodecSet};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::media::Codec;
use sdrkit_core::{CommandError, Complex32, InputInfo, Owner, Receiver, StackConfig, Value};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_dvoice::dstar::{slow_blocks, Header, SLOW_XOR};
use sdrkit_stack_dvoice::dstar_tx::{DstarTxError, Sender, NOTE, SILENCE};
use sdrkit_stack_dvoice::gen::Prepared;
use sdrkit_stack_dvoice::sender::PEAK;
use sdrkit_stack_dvoice::typed::{DstarTxFields, DstarTxKind};
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

/// The receiver decodes the header fields of a repeater transmission, the text, and every voice frame as the AMBE silence
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
    assert!(
        got.iter().all(|f| f[..] == SILENCE),
        "every voice frame is AMBE silence"
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

/// The sender's fields, settings and commands are of family version 1.3.
#[test]
fn the_sender_of_dstar_is_of_family_version_1_3() {
    let f = sdrkit_stack_dvoice::typed::family();
    for path in [
        "shape.DstarTxFields.tx_note",
        "dstar.setting.your_call",
        "dstar.command.send",
    ] {
        assert_eq!(f.since(path).minor, 3, "{path}");
    }
}
