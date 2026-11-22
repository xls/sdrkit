//! The sender of the DMR stack: the stack's own receiver decodes what it sends in direct mode on both slots (the text,
//! its CRC, the source, the target, the group flag, the slot, the colour code and the direct-mode link), in ISO 8859-1
//! and in UTF-16LE; a voice call decodes to the speech frames sent, and push-to-talk sends the frames of the microphone
//! audio, which sound with its pitch; the bursts come every 60 ms with the carrier off between them and fit the 12.5 kHz
//! channel; halt ends after the burst on air; a text or a call is refused without a radio ID, without a target and with
//! the transmitter off. Nothing goes to a radio.

use crate::common::*;
use crate::speech::{pitch_hz, speech_codecs, vowel, vowel_frames, VOWEL_F0};
use crate::tx_host::*;
use sdrkit_algo_dsp::bandwidth::power_share_edges;
use sdrkit_algo_dsp::pulse::{Norm, Shape};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_codec_mbe::{Coder, Encoder};
use sdrkit_core::codec::{AudioCodec, CodecSet};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::media::Codec;
use sdrkit_core::{CommandError, Complex32, InputInfo, Owner, Receiver, StackConfig, Value};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_dvoice::dmr::{Lc, ShortData};
use sdrkit_stack_dvoice::dmr_tx::{text_octets, DmrSlot, DmrTxError, Sender, NOTE};
use sdrkit_stack_dvoice::gen::Prepared;
use sdrkit_stack_dvoice::sender::PEAK;
use sdrkit_stack_dvoice::typed::{DmrTxFields, DataTxKind};
use std::sync::{Arc, Mutex};

/// The dial of the transmitting stack.
const DIAL: f64 = 439.4125e6;

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

/// The message of `text` from radio 5051234 to `dst`.
fn sender(text: &str, dst: u32, group: bool, cc: u8, slot: DmrSlot) -> Sender {
    let (data, format) = text_octets(text);
    let m = ShortData {
        src: 5_051_234,
        dst,
        group,
        format,
        data: &data,
    };
    Sender::text(&m, cc, slot).expect("the text fits")
}

/// On slot 1 and slot 2, to a radio and to a talkgroup, in ISO 8859-1 and in UTF-16LE: the receiver reports the text
/// with a good CRC, the source, the target, the group flag, the slot, the colour code and the direct-mode link.
#[test]
fn the_receiver_decodes_texts_on_both_slots() {
    for (slot, n, text, dst, group, cc, format) in [
        (
            DmrSlot::S1,
            1,
            "Hello from VK2ABC, 73",
            5_051_235,
            false,
            1,
            3,
        ),
        (DmrSlot::S2, 2, "Grüße an alle", 505, true, 7, 3),
        (DmrSlot::S1, 1, "Привет, 73 ✓", 91, true, 3, 19),
    ] {
        let (r, _) = run(
            receiver("dmr", &[]).as_mut(),
            RATE,
            &on_air(&samples(sender(text, dst, group, cc, slot))),
            4800,
        );
        let m = &r["detail"]["messages"];
        assert_eq!(m.as_array().map(Vec::len), Some(1), "{text}: {m}");
        let m = &m[0];
        assert_eq!(m["text"], text);
        assert_eq!(
            (&m["crc_ok"], &m["data_format"], &m["slot"]),
            (&true.into(), &format.into(), &n.into()),
            "{m}"
        );
        assert_eq!(
            (&m["src"], &m["dst"], &m["group"]),
            (&5_051_234.into(), &dst.into(), &group.into()),
            "{m}"
        );
        let f = &r["fields"];
        assert_eq!(
            (&f["link"], &f["colour_code"]),
            (&"direct_mode".into(), &cc.into()),
            "{f}"
        );
    }
}

/// The bursts: 27.5 ms each, one every 60 ms, the carrier off for the rest; the magnitude 0.9 inside a burst; the
/// deviation ±1944 Hz and ±648 Hz at the symbol centres; the 99 %
/// power bandwidth of the bursts within the 12.5 kHz channel; the power in each adjacent channel (8.5 kHz around 12.5
/// kHz away) at least 55 dB below the power in the channel.
#[test]
fn the_bursts_keep_their_slot_and_fit_the_channel() {
    let s = sender(
        "A message of three blocks or so, 73",
        91,
        true,
        1,
        DmrSlot::S1,
    );
    let bursts = s.bursts_left();
    let s48 = samples(s);
    assert_eq!(s48.len(), bursts * 2880, "60 ms a burst");
    for k in 0..bursts {
        let b = &s48[k * 2880..(k + 1) * 2880];
        let on = &b[200..1300];
        assert!(
            on.iter().all(|z| (z.norm() - PEAK).abs() < 2e-2),
            "burst {k}: the level"
        );
        assert!(
            b[1500..].iter().all(|z| z.norm() == 0.0),
            "burst {k}: the carrier off after the burst and its fall"
        );
    }
    let peak = s48.iter().map(|z| z.norm()).fold(0f32, f32::max);
    assert!(peak <= PEAK + 1e-3, "peak {peak}");
    // The occupied bandwidth and the adjacent channel power of the whole signal.
    let n = 1024;
    let mut p = vec![0f32; n];
    welch(
        &s48,
        n,
        Window::BlackmanHarris4,
        0.5,
        Scaling::Power,
        &mut p,
    )
    .unwrap();
    let pd: Vec<f64> = p.iter().map(|&v| f64::from(v)).collect();
    let (lo, hi) = power_share_edges(&pd, 0, n - 1, 0.99).unwrap();
    let bw = (hi - lo + 1) as f64 * 48_000.0 / n as f64;
    let hz = |i: usize| (i as f64 - (n / 2) as f64) * 48_000.0 / n as f64;
    let band = |a: f64, b: f64| -> f64 {
        (0..n)
            .filter(|&i| hz(i) >= a && hz(i) < b)
            .map(|i| f64::from(p[i]))
            .sum()
    };
    let acp = 10.0
        * (band(8250.0, 16_750.0).max(band(-16_750.0, -8250.0)) / band(-6250.0, 6250.0)).log10();
    // The deviation: the instantaneous frequency through the matched root raised cosine of a receiver (so the pulse is a
    // raised cosine, free of interference between symbols), at the symbol centres of the bursts (10 samples a symbol; the
    // centre is the phase of the samples where the values gather best), the mean of the outer and the inner levels.
    let rrc = sdrkit_algo_dsp::pulse::taps(Shape::Rrc { beta: 0.2 }, 10.0, 16.0, Norm::Dc).unwrap();
    let f: Vec<f64> = {
        let raw = frequency(&s48, 48_000.0);
        let d = rrc.len() / 2;
        (0..raw.len())
            .map(|n| {
                (0..rrc.len())
                    .filter_map(|k| {
                        (n + d)
                            .checked_sub(k)
                            .and_then(|i| raw.get(i))
                            .map(|v| v * f64::from(rrc[k]))
                    })
                    .sum()
            })
            .collect()
    };
    let centres = |o: usize| -> Vec<f64> {
        (0..bursts)
            .flat_map(|k| (20..120).map(move |i| k * 2880 + o + 10 * i))
            .map(|n| f[n])
            .collect()
    };
    let spread = |v: &[f64]| -> f64 {
        v.iter()
            .map(|x| {
                [-1944.0, -648.0, 648.0, 1944.0]
                    .iter()
                    .map(|l| (x - l).abs())
                    .fold(f64::MAX, f64::min)
            })
            .sum::<f64>()
    };
    let at = (0..10)
        .min_by(|&a, &b| spread(&centres(a)).total_cmp(&spread(&centres(b))))
        .unwrap();
    let v = centres(at);
    let mean = |outer: bool| -> f64 {
        let s: Vec<f64> = v
            .iter()
            .map(|x| x.abs())
            .filter(|x| (*x > 1296.0) == outer)
            .collect();
        s.iter().sum::<f64>() / s.len() as f64
    };
    let (outer, inner) = (mean(true), mean(false));
    println!("DMR: occupied bandwidth {bw:.0} Hz, adjacent channel power {acp:.1} dB, deviation {outer:.0} Hz and {inner:.0} Hz");
    assert!(
        (outer - 1944.0).abs() < 30.0 && (inner - 648.0).abs() < 20.0,
        "deviation {outer} Hz and {inner} Hz"
    );
    assert!(bw <= 9_000.0 && bw > 5_000.0, "occupied bandwidth {bw} Hz");
    assert!(acp < -55.0, "adjacent channel power {acp:.1} dB");
}

/// The DMR stack with `settings`, the transmitter on, and its host.
fn station(settings: &[(&str, &str)]) -> (Box<dyn Receiver>, Arc<Host>) {
    let mut rx = (desc("dmr").make_receiver)(&StackConfig::new(values("dmr", settings)))
        .expect("the stack takes the settings");
    rx.configure(&InputInfo::new(RATE, DIAL, DIAL).recording())
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

/// The sender's fields: transmitting, what it sends, the frequency, the bursts and the note.
fn fields(rx: &dyn Receiver) -> (bool, Option<DataTxKind>, Option<f64>, i64, String) {
    let f = DmrTxFields::read(Owner::results(rx).fields()).expect("the sender's fields");
    (
        f.transmitting,
        f.tx_kind,
        f.tx_freq_hz.map(|h| h.0),
        f.tx_frames,
        f.tx_note.to_string(),
    )
}

fn refusal(e: &CommandError) -> Option<DmrTxError> {
    match e {
        CommandError::Failed { source } => source.downcast_ref::<DmrTxError>().cloned(),
        _ => None,
    }
}

fn send(rx: &mut dyn Receiver, text: &str) -> Result<(), CommandError> {
    call(rx, "send", &[("text", Value::Text(text.into()))])
}

/// `send` on the graph sends the text from the settings to the radio called, or to the talkgroup; the results say what
/// it sends; halt ends a transmission after the burst on air, with its end mark.
#[test]
fn a_text_on_the_graph_carries_the_settings_and_halt_ends_it() {
    let (mut rx, host) = station(&[
        ("radio_id", "5051234"),
        ("talkgroup", "505"),
        ("colour_code", "4"),
        ("slot", "2"),
    ]);
    idle(rx.as_mut(), &host, 0.2, false);
    send(rx.as_mut(), "73 DE VK2ABC").unwrap();
    assert_eq!(
        refusal(&send(rx.as_mut(), "again").unwrap_err()),
        Some(DmrTxError::Busy)
    );
    idle(rx.as_mut(), &host, 0.1, false);
    let (on, kind, freq, _, note) = fields(rx.as_ref());
    assert!(on);
    assert_eq!(
        (kind, freq, note.as_str()),
        (Some(DataTxKind::Text), Some(DIAL), NOTE)
    );
    idle(rx.as_mut(), &host, 0.6, false);
    let (on, _, _, bursts, _) = fields(rx.as_ref());
    assert!(!on);
    assert_eq!(bursts, 5, "two preambles, the header and two blocks");
    let b = host.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].2 && b[0].1 == DIAL);
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, _) = run(receiver("dmr", &[]).as_mut(), RATE, &x, 4800);
    let m = &r["detail"]["messages"][0];
    assert_eq!(
        (&m["text"], &m["src"], &m["dst"], &m["group"], &m["slot"]),
        (
            &"73 DE VK2ABC".into(),
            &5_051_234.into(),
            &505.into(),
            &true.into(),
            &2.into()
        ),
        "{m}"
    );
    assert_eq!(r["fields"]["colour_code"], 4);
    // Halt after the first burst: no more bursts, the end mark comes.
    send(
        rx.as_mut(),
        "A longer message that needs several data blocks to go out",
    )
    .unwrap();
    idle(rx.as_mut(), &host, 0.05, false);
    call(rx.as_mut(), "halt", &[]).unwrap();
    idle(rx.as_mut(), &host, 0.5, false);
    let b = host.bursts();
    assert_eq!(b.len(), 2);
    assert!(b[1].2, "the end mark came");
    let (on, _, _, bursts, _) = fields(rx.as_ref());
    assert!(!on && bursts <= 2, "{bursts}");
}

/// Without a radio ID or a target a text is refused with the error that names the setting; an empty text or one that
/// does not fit is refused; with the transmitter off it is refused as such. Nothing goes on air.
#[test]
fn texts_are_refused_and_nothing_is_sent() {
    let (mut rx, host) = station(&[("called_id", "5051235")]);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = send(rx.as_mut(), "hi").unwrap_err();
    let why = refusal(&e).expect("a refusal of the sender");
    assert_eq!(
        (why.clone(), why.setting()),
        (DmrTxError::NoSource, Some("radio_id"))
    );
    assert!(e.to_string().contains("radio_id"), "{e}");
    let (mut rx, host) = station(&[("radio_id", "5051234")]);
    idle(rx.as_mut(), &host, 0.2, false);
    assert_eq!(
        refusal(&send(rx.as_mut(), "hi").unwrap_err()),
        Some(DmrTxError::NoTarget)
    );
    let (mut rx, host) = station(&[("radio_id", "5051234"), ("called_id", "5051235")]);
    idle(rx.as_mut(), &host, 0.2, false);
    assert_eq!(
        refusal(&send(rx.as_mut(), "").unwrap_err()),
        Some(DmrTxError::TooLong(0))
    );
    assert_eq!(
        refusal(&send(rx.as_mut(), &"x".repeat(753)).unwrap_err()),
        Some(DmrTxError::TooLong(753))
    );
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(matches!(
        send(rx.as_mut(), "hi"),
        Err(CommandError::TransmitOff)
    ));
    idle(rx.as_mut(), &host, 0.3, false);
    assert!(host.bursts().is_empty());
}

/// The speech frames that the receiver of the DMR stack hands to its plug-in for `x` at [`RATE`], as 72 channel bits each,
/// and its view of the results.
fn received_frames(x: &[Complex32]) -> (serde_json::Value, Vec<[u8; 72]>) {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let mut set = CodecSet::default();
    let k = Arc::clone(&kept);
    set.register(
        Codec::Ambe2.fourcc(),
        Some("codewords"),
        Arc::new(move |_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Keep(Arc::clone(&k))) as Box<dyn AudioCodec>)
        }),
    );
    let (r, _) = run(
        receiver_with_codecs("dmr", &[], set).as_mut(),
        RATE,
        x,
        4800,
    );
    let frames = kept
        .lock()
        .unwrap()
        .iter()
        .map(|cw: &Vec<u8>| sdrkit_codec_mbe::codeword_channel_bits(cw.as_slice().try_into().expect("12 bytes")))
        .collect();
    (r, frames)
}

/// A voice call of three superframes on each slot, to a talkgroup and to a radio: the receiver reports the call with its
/// source, target, slot, colour code and direct-mode link, ended by the terminator, and hands on the 54 speech frames
/// sent, bit for bit.
#[test]
fn a_voice_call_decodes_to_the_frames_sent() {
    let frames = vowel_frames::<72>(Coder::HalfRate, 54);
    for (slot, n, dst, group, cc) in [
        (DmrSlot::S1, 1, 505, true, 1),
        (DmrSlot::S2, 2, 5_051_235, false, 9),
    ] {
        let lc = Lc {
            pf: false,
            flco: if group { 0 } else { 3 },
            fid: 0,
            opts: 0,
            dst,
            src: 5_051_234,
        };
        let mut s = Sender::voice(&lc, cc, slot, 18);
        let mut next = frames.iter();
        let mut s48 = Vec::new();
        while s.next_with(|f| *f = *next.next().expect("54 frames"), &mut s48) {}
        assert_eq!(
            s.bursts_sent(),
            20,
            "the header, 18 voice bursts and the terminator"
        );
        assert_eq!(s48.len(), 20 * 2880);
        let (r, got) = received_frames(&on_air(&s48));
        let c = calls(&r);
        assert_eq!(c.len(), 1, "{}", show(&c));
        assert_eq!(
            (&c[0]["src"], &c[0]["dst"], &c[0]["group"], &c[0]["slot"]),
            (&5_051_234.into(), &dst.into(), &group.into(), &n.into()),
            "{}",
            c[0]
        );
        assert_eq!(c[0]["end"], "terminator", "{}", c[0]);
        assert_eq!(c[0]["voice_frames"], 54, "{}", c[0]);
        assert_eq!(
            (&r["fields"]["link"], &r["fields"]["colour_code"]),
            (&"direct_mode".into(), &cc.into())
        );
        assert_eq!(got.len(), frames.len(), "slot {n}");
        assert!(got == frames, "slot {n}: the speech frames, bit for bit");
    }
}

/// The vowel at `t` seconds, as audio of full scale 1.
fn vowel_at(t: f64) -> f32 {
    // The vowel of the speech tests at 8 kHz, linearly interpolated between its samples.
    let n = t * 8000.0;
    let (a, b) = (vowel(n.floor() as usize), vowel(n.floor() as usize + 1));
    let w = (n - n.floor()) as f32;
    (f32::from(a) * (1.0 - w) + f32::from(b) * w) / 32768.0
}

/// Push-to-talk sends the speech of the host's microphone: the receiver reports the call of the settings and gets the
/// frames that an encoder makes of the same audio at 8 kHz, bit for bit; the speech decoder plays them with the pitch
/// of the vowel; key up ends the call with the terminator after the burst on air.
#[test]
fn push_to_talk_sends_the_microphone_bit_for_bit() {
    let (mut rx, host) = station(&[("radio_id", "5051234"), ("talkgroup", "505"), ("slot", "2")]);
    host.mic.lock().unwrap().0 = (0..96_000).map(|i| vowel_at(i as f64 / 48_000.0)).collect();
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    ptt(rx.as_mut(), true).expect("a call on air goes on");
    assert_eq!(
        refusal(&send(rx.as_mut(), "hi").unwrap_err()),
        Some(DmrTxError::Busy)
    );
    idle(rx.as_mut(), &host, 1.0, true);
    let (on, kind, freq, _, _) = fields(rx.as_ref());
    assert!(on);
    assert_eq!((kind, freq), (Some(DataTxKind::Voice), Some(DIAL)));
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 0.5, true);
    let (on, _, _, bursts, _) = fields(rx.as_ref());
    assert!(!on);
    let b = host.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    assert!(b[0].2 && b[0].1 == DIAL);
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(&b[0].0);
    let (r, got) = received_frames(&x);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(
        (&c[0]["src"], &c[0]["dst"], &c[0]["slot"], &c[0]["end"]),
        (
            &5_051_234.into(),
            &505.into(),
            &2.into(),
            &"terminator".into()
        )
    );
    let voice_bursts = bursts - 2;
    assert!(
        (16..=22).contains(&voice_bursts),
        "{voice_bursts} voice bursts for about 1.1 s"
    );
    assert_eq!(got.len() as i64, 3 * voice_bursts);
    // The same audio through a rate change to 8 kHz and a fresh encoder.
    let audio = host.mic.lock().unwrap().0.clone();
    let mut s8 = Vec::new();
    Resampler::<f32>::new(48_000.0, 8000.0)
        .unwrap()
        .process(&audio, &mut s8);
    let mut enc = Encoder::new(Coder::HalfRate);
    let want: Vec<[u8; 72]> = s8
        .as_chunks::<160>()
        .0
        .iter()
        .take(got.len())
        .map(|c| {
            let pcm: [i16; 160] =
                std::array::from_fn(|i| (c[i].clamp(-1.0, 1.0) * 32767.0).round() as i16);
            let mut f = [0u8; 72];
            enc.encode(&pcm, &mut f);
            f
        })
        .collect();
    assert!(
        got == want,
        "the speech frames of the microphone, bit for bit"
    );
    // The speech decoder of the stack plays the call with the pitch of the vowel.
    let (_, out) = run(
        receiver_with_codecs("dmr", &[], speech_codecs()).as_mut(),
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
    let f0 = pitch_hz(&sound[(0.3 * rate) as usize..(0.9 * rate) as usize], rate);
    assert!((f0 - VOWEL_F0).abs() < 0.04 * VOWEL_F0, "pitch {f0} Hz");
}

/// Without a radio ID or a target push-to-talk is refused with the error that names the setting, and with the
/// transmitter off it is refused as such; the time-out ends a call with the terminator.
#[test]
fn calls_are_refused_and_the_time_out_ends_one() {
    let (mut rx, host) = station(&[("talkgroup", "505")]);
    idle(rx.as_mut(), &host, 0.2, false);
    assert_eq!(
        refusal(&ptt(rx.as_mut(), true).unwrap_err()),
        Some(DmrTxError::NoSource)
    );
    let (mut rx, host) = station(&[("radio_id", "5051234")]);
    idle(rx.as_mut(), &host, 0.2, false);
    assert_eq!(
        refusal(&ptt(rx.as_mut(), true).unwrap_err()),
        Some(DmrTxError::NoTarget)
    );
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(matches!(
        ptt(rx.as_mut(), true),
        Err(CommandError::TransmitOff)
    ));
    assert!(host.bursts().is_empty());
    let (mut rx, host) = station(&[
        ("radio_id", "5051234"),
        ("talkgroup", "505"),
        ("tx_timeout", "1"),
    ]);
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 2.0, true);
    let (on, _, _, bursts, _) = fields(rx.as_ref());
    assert!(!on, "the time-out ended the call");
    assert_eq!(
        bursts,
        1 + 17 + 1,
        "the header, 1 s of voice bursts and the terminator"
    );
    assert_eq!(host.bursts().len(), 1);
}

/// A voice call in direct mode through noise at 15 dB in the channel: the receiver holds the timing and the levels across
/// the noise of the silent slot, so every speech frame arrives with at most a few bit errors.
#[test]
fn a_voice_call_through_noise_keeps_its_frames() {
    let frames = vowel_frames::<72>(Coder::HalfRate, 54);
    let lc = Lc {
        pf: false,
        flco: 0,
        fid: 0,
        opts: 0,
        dst: 505,
        src: 5_051_234,
    };
    let mut s = Sender::voice(&lc, 1, DmrSlot::S1, 18);
    let mut next = frames.iter();
    let mut s48 = Vec::new();
    while s.next_with(|f| *f = *next.next().expect("54 frames"), &mut s48) {}
    let mut x = on_air(&s48);
    // Noise of the power of the bursts less 15 dB in the 12.5 kHz channel.
    let power = f64::from(PEAK * PEAK) / 10f64.powf(1.5) * RATE / 12_500.0;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    for z in x.iter_mut() {
        *z += rng.complex_normal(power);
    }
    let (r, got) = received_frames(&x);
    assert_eq!(calls(&r).len(), 1);
    assert_eq!(got.len(), frames.len());
    let errors: usize = got
        .iter()
        .zip(&frames)
        .map(|(g, w)| (0..72).filter(|&i| g[i] != w[i]).count())
        .sum();
    assert!(errors <= 10, "{errors} bit errors in 3888 bits");
}
