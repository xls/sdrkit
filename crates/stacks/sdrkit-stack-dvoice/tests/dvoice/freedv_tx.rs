//! The sender of the FreeDV stack: the stack's own receiver decodes what it sends on 1600, 700D and 700E (the callsign text
//! and the Codec 2 frames bit for bit from the moment it trusts the waveform to the last frame), the signal stays in its
//! band at its level, push-to-talk off ends after the frame on air, and a transmission is refused without a callsign text,
//! with the transmitter off, in a submode that does not send, and without the Codec 2 tables. Nothing goes to a radio.

use crate::common::*;
use crate::tx_host::*;
use sdrkit_algo_dsp::bandwidth::power_share_edges;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_codec_codec2::{Codebooks, Encoder, Mode};
use sdrkit_core::codec::{AudioCodec, CodecSet};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::media::Codec;
use sdrkit_core::{CommandError, Complex32, InputInfo, Owner, Receiver, StackConfig};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_dvoice::freedv_ofdm::{W700D, W700E};
use sdrkit_stack_dvoice::freedv_tx::{FdmdvSender, FreedvTransmit, FreedvTxError, Sender};
use sdrkit_stack_dvoice::gen::Prepared;
use sdrkit_stack_dvoice::sender::PEAK;
use sdrkit_stack_dvoice::typed::{FreedvTxFields, FreedvTxKind};
use serde_json::Value as Json;
use std::sync::{Arc, Mutex};

/// The dial of the transmitting stack.
const DIAL: f64 = 14.236e6;

fn books() -> Arc<Codebooks> {
    made_up_books(Mode::M700C)
}

/// The Codec 2 frames that the stack's own receiver in `submode` hands to its plug-in, and its view of the results, for
/// `x` at [`RATE`].
fn receive(submode: &str, x: &[Complex32]) -> (Json, Vec<Vec<u8>>) {
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
    let mut cfg = StackConfig::new(values("freedv", &[]));
    cfg.submode = Some(submode.into());
    let mut g = sdrkit_stack_dvoice::receiver_graph("freedv", &cfg)
        .expect("a digital voice stack")
        .expect("the graph builds");
    g.set_codecs(set);
    let (r, _) = run(&mut g, RATE, x, 4096);
    let frames = kept.lock().unwrap().clone();
    (r, frames)
}

/// The 8 kHz samples `s8` at [`RATE`], with 0.3 s of silence before them.
fn on_air(s8: &[Complex32]) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); 2400];
    x.extend_from_slice(s8);
    let mut g = Prepared::with_source_rate(x, 8000.0, RATE, false);
    generate_all(&mut g)
}

/// `n` Codec 2 700C frames of random bits (28 bits, the last 4 bits of the fourth byte zero).
fn codec_frames(n: usize, seed: u64) -> Vec<[u8; 4]> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            let mut b = [0u8; 32];
            rng.fill_bits(&mut b);
            std::array::from_fn(|i| {
                (0..8).fold(0u8, |v, j| {
                    let n = 8 * i + j;
                    v | if n < 28 { b[n] << (7 - j) } else { 0 }
                })
            })
        })
        .collect()
}

/// Runs `s` to its end with the codec frames `frames` (whole modem frames of them), keying up after the last; the 8 kHz
/// samples.
fn transmission(mut s: Sender, frames: &[[u8; 4]]) -> Vec<Complex32> {
    let mut out = Vec::new();
    let mut k = 0;
    loop {
        if k + s.codec_frames() > frames.len() {
            s.stop();
        }
        let more = s.next(
            |f| {
                *f = frames[k];
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

/// Where `got` starts in `sent`, when `got` holds the frames of `sent` from there to the last one, in order. Frames
/// after those are the damaged frames that the receiver hands over after the signal ends, while it holds sync.
fn run_in(sent: &[Vec<u8>], got: &[Vec<u8>]) -> Option<usize> {
    (0..sent.len()).find(|&i| {
        let n = sent.len() - i;
        got.len() >= n && got[..n] == sent[i..]
    })
}

/// On 700D and 700E, with the clipper and the transmit filter off and on: the receiver reports the callsign text, and
/// from the moment it trusts the waveform (within 2.5 s) it hands over every codec frame bit for bit, to the last one.
#[test]
fn the_receiver_decodes_the_callsign_and_every_codec_frame_to_the_last() {
    for (sub, w) in [("700D", &W700D), ("700E", &W700E)] {
        for (clip, filter) in [(false, false), (true, true)] {
            let n = 240;
            let sent = codec_frames(n, 3);
            let s8 = transmission(Sender::new(w, "N0CALL", clip, filter, 10_000), &sent);
            let (r, got) = receive(sub, &on_air(&s8));
            let what = format!("{sub} clip {clip} filter {filter}");
            let c = calls(&r);
            assert_eq!(c.len(), 1, "{what}: {}", show(&c));
            assert_eq!(
                (&c[0]["src"], &c[0]["text"]),
                (&"N0CALL".into(), &"N0CALL".into()),
                "{what}"
            );
            let sent: Vec<Vec<u8>> = sent.iter().map(|f| f.to_vec()).collect();
            let at = run_in(&sent, &got)
                .unwrap_or_else(|| panic!("{what}: {} frames not in order", got.len()));
            let after = got.len() - (n - at);
            println!("{what}: frames {at} to {n} of {n}, then {after} damaged frames");
            assert!(
                at as f64 * 0.04 <= 2.5,
                "{what}: trusted after {} s",
                at as f64 * 0.04
            );
            assert!(after <= 50, "{what}: {after} frames after the end");
        }
    }
}

/// The 99 % power bandwidth of each waveform lies within 300 Hz to 2700 Hz of the sideband and exceeds the span of its
/// carriers by at most 400 Hz, 200 Hz with the transmit filter; no sample is above 0.9 of full scale, and the peak
/// reaches it.
#[test]
fn the_signal_stays_in_its_band_at_its_level() {
    for w in [&W700D, &W700E] {
        for (clip, filter) in [(false, false), (false, true), (true, false)] {
            let s8 = transmission(
                Sender::new(w, "N0CALL", clip, filter, 10_000),
                &codec_frames(200, 5),
            );
            let peak = s8.iter().map(|z| z.norm()).fold(0f32, f32::max);
            assert!(peak <= PEAK + 1e-6 && peak > 0.8, "{} peak {peak}", w.name);
            let limited =
                s8.iter().filter(|z| z.norm() >= PEAK - 1e-4).count() as f64 / s8.len() as f64;
            println!(
                "{} clip {clip} filter {filter}: {:.3} % of the samples at the limit",
                w.name,
                100.0 * limited
            );
            assert!(clip || limited < 0.005, "{}: {limited}", w.name);
            let n = 1024;
            let mut p = vec![0f32; n];
            welch(
                &s8[2000..s8.len() - 2000],
                n,
                Window::BlackmanHarris4,
                0.5,
                Scaling::Power,
                &mut p,
            )
            .unwrap();
            let pd: Vec<f64> = p.iter().map(|&v| f64::from(v)).collect();
            let (lo, hi) = power_share_edges(&pd, 0, n - 1, 0.99).unwrap();
            let hz = |i: usize| (i as f64 - (n / 2) as f64) * 8000.0 / n as f64;
            let bw = (hi - lo + 1) as f64 * 8000.0 / n as f64;
            println!(
                "{} clip {clip} filter {filter}: {:.0} Hz to {:.0} Hz, {bw:.0} Hz, peak {peak:.3}",
                w.name,
                hz(lo),
                hz(hi)
            );
            assert!(
                hz(lo) >= 300.0 && hz(hi) <= 2700.0,
                "{}: {} to {}",
                w.name,
                hz(lo),
                hz(hi)
            );
            let span = (w.nc + 2) as f64 * 8000.0 / w.m as f64;
            assert!(
                bw <= span + if filter || clip { 200.0 } else { 400.0 },
                "{}: {bw} Hz",
                w.name
            );
        }
    }
}

/// The waveform of each submode: 700E in 700E, else 700D; a transmission ends after the frame on air with one pilot row.
#[test]
fn a_transmission_ends_after_the_frame_on_air_with_a_pilot_row() {
    for w in [&W700D, &W700E] {
        let mut s = Sender::new(w, "N0CALL", false, false, 10_000);
        let mut out = Vec::new();
        for _ in 0..3 {
            assert!(s.next(|f| *f = [0x55; 4], &mut out));
        }
        assert_eq!(out.len(), 3 * w.frame());
        s.stop();
        assert!(s.next(|_| unreachable!("no codec frame after the stop"), &mut out));
        assert_eq!(out.len(), 3 * w.frame() + 160, "{}: one pilot row", w.name);
        assert!(out[out.len() - 1].norm() < 1e-3, "the fall");
        assert!(!s.next(|_| {}, &mut out));
        assert_eq!(s.frames_sent(), 3);
    }
    // The time-out: a limit of 5 modem frames.
    let mut s = Sender::new(&W700E, "N0CALL", false, false, 5);
    let mut out = Vec::new();
    while s.next(|f| *f = [0; 4], &mut out) {}
    assert_eq!(s.frames_sent(), 5);
}

/// The FreeDV stack in `submode` with `settings`, the transmitter on, the test tables when `tables`, and its host with
/// `seconds` of speech-like microphone audio.
fn station(
    submode: &str,
    settings: &[(&str, &str)],
    tables: bool,
    seconds: f64,
) -> (Box<dyn Receiver>, Arc<Host>) {
    station_with(submode, settings, tables.then(books), seconds)
}

/// The FreeDV stack in `submode` with `settings`, the transmitter on, the Codec 2 tables `tables`, and its host with
/// `seconds` of speech-like microphone audio.
fn station_with(
    submode: &str,
    settings: &[(&str, &str)],
    tables: Option<Arc<Codebooks>>,
    seconds: f64,
) -> (Box<dyn Receiver>, Arc<Host>) {
    let mut cfg = StackConfig::new(values("freedv", settings));
    cfg.submode = Some(submode.into());
    let mut rx = (desc("freedv").make_receiver)(&cfg).expect("the stack takes the settings");
    rx.configure(&InputInfo::new(RATE, DIAL - 20e3, DIAL).recording())
        .unwrap();
    let audio: Vec<f32> = (0..(seconds * 48_000.0) as usize)
        .map(|i| speechlike(i as f64 / 48_000.0))
        .collect();
    let host = Arc::new(Host {
        mic: Mutex::new((audio, 0, 0)),
        ..Host::default()
    });
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the graph of the stack");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(host.clone());
    g.set_context(ctx);
    if let Some(t) = tables {
        g.component_as_mut::<FreedvTransmit>("transmit")
            .expect("the sender")
            .set_codebooks(t);
    }
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, host)
}

fn fields(rx: &dyn Receiver) -> FreedvTxFields {
    FreedvTxFields::read(Owner::results(rx).fields()).expect("the sender's fields")
}

/// The refusal inside a command error.
fn refusal(e: &CommandError) -> Option<FreedvTxError> {
    match e {
        CommandError::Failed { source } => source.downcast_ref::<FreedvTxError>().cloned(),
        _ => None,
    }
}

/// Push-to-talk sends the speech of the host's microphone: the receiver gets the callsign text and, after a first modem
/// frame of coded silence, the Codec 2 frames that an encoder makes of the same audio at 8 kHz, bit for bit; key up ends
/// the transmission with its end mark.
#[test]
fn push_to_talk_sends_the_microphone_bit_for_bit() {
    for (sub, w, kind) in [
        ("700D", &W700D, FreedvTxKind::Voice700D),
        ("700E", &W700E, FreedvTxKind::Voice700E),
    ] {
        let (mut rx, host) = station(sub, &[("callsign", "N0CALL")], true, 12.0);
        idle(rx.as_mut(), &host, 0.2, false);
        assert!(!fields(rx.as_ref()).transmitting);
        ptt(rx.as_mut(), true).unwrap();
        ptt(rx.as_mut(), true).expect("a transmission on air goes on");
        idle(rx.as_mut(), &host, 9.0, true);
        let f = fields(rx.as_ref());
        assert!(f.transmitting);
        assert_eq!(f.tx_kind, Some(kind));
        assert_eq!(f.tx_freq_hz.map(|h| h.0), Some(DIAL));
        ptt(rx.as_mut(), false).unwrap();
        idle(rx.as_mut(), &host, 1.0, true);
        let f = fields(rx.as_ref());
        assert!(!f.transmitting);
        let b = host.bursts();
        assert_eq!(b.len(), 1, "{sub}: one transmission");
        let (samples, centre, ended, starts) = &b[0];
        assert!(*ended && starts.is_empty());
        assert_eq!(*centre, DIAL);
        let mut x = vec![Complex32::default(); 9600];
        x.extend_from_slice(samples);
        let (r, got) = receive(sub, &x);
        let c = calls(&r);
        assert_eq!(c.len(), 1, "{sub}: {}", show(&c));
        assert_eq!(c[0]["src"], "N0CALL");
        // The same audio through a rate change to 8 kHz and a fresh encoder, after the frame of coded silence.
        let per = w.k() / 28;
        let sent = f.tx_frames as usize * per;
        let audio = host.mic.lock().unwrap().0.clone();
        let mut s8 = vec![0f32; per * 320];
        Resampler::<f32>::new(48_000.0, 8000.0)
            .unwrap()
            .process(&audio, &mut s8);
        let mut enc = Encoder::new(Mode::M700C, &books()).unwrap();
        let want: Vec<Vec<u8>> = s8
            .as_chunks::<320>()
            .0
            .iter()
            .take(sent)
            .map(|c| {
                let pcm: Vec<i16> = c
                    .iter()
                    .map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i16)
                    .collect();
                let mut f = vec![0u8; 4];
                enc.encode(&pcm, &mut f).unwrap();
                f
            })
            .collect();
        assert_eq!(want.len(), sent);
        let at = run_in(&want, &got).unwrap_or_else(|| {
            panic!("{sub}: the codec frames of the microphone, bit for bit, to the last")
        });
        assert!(
            got.len() - (sent - at) <= 50,
            "{sub}: {} frames after the end",
            got.len() - (sent - at)
        );
        println!(
            "{sub}: {} modem frames; codec frames {at} to {sent} received",
            f.tx_frames
        );
        assert!(
            at as f64 * 0.04 <= 2.5,
            "{sub}: trusted after {} s",
            at as f64 * 0.04
        );
    }
}

/// Without a callsign text push-to-talk is refused with the error that names the setting; the submode RADE does not
/// send; without the Codec 2 tables it is refused with the missing table; with the transmitter off it is refused
/// as such. Nothing goes on air.
#[test]
fn transmissions_are_refused_and_nothing_is_sent() {
    let (mut rx, host) = station("700D", &[], true, 1.0);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    let why = refusal(&e).expect("a refusal of the sender");
    assert_eq!(
        (why.clone(), why.setting()),
        (FreedvTxError::NoSource, Some("callsign"))
    );
    assert!(e.to_string().contains("callsign"), "{e}");
    let (mut rx, host) = station("rade", &[("callsign", "N0CALL")], true, 1.0);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    assert_eq!(refusal(&e), Some(FreedvTxError::NoWaveform), "rade");
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(host.bursts().is_empty());
    let (mut rx, host) = station("auto", &[("callsign", "N0CALL")], false, 1.0);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    assert!(
        matches!(refusal(&e), Some(FreedvTxError::NoTable(_))),
        "{e}"
    );
    let (mut rx, host) = station("700E", &[("callsign", "N0CALL")], true, 1.0);
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(matches!(
        ptt(rx.as_mut(), true),
        Err(CommandError::TransmitOff)
    ));
    idle(rx.as_mut(), &host, 0.3, false);
    assert!(host.bursts().is_empty());
    // A callsign text that is not printable ASCII is not taken.
    let mut bad = values("freedv", &[]);
    let schema = (desc("freedv").settings)();
    bad.set(
        schema.id("callsign").unwrap(),
        &sdrkit_core::Value::Text("N0CALL\u{7}".into()),
    )
    .unwrap();
    assert!((desc("freedv").make_receiver)(&StackConfig::new(bad)).is_err());
}

/// The time-out ends a transmission after its time with the end mark; halt ends one after the frame on air.
#[test]
fn the_time_out_and_halt_end_a_transmission() {
    let (mut rx, host) = station(
        "auto",
        &[("callsign", "N0CALL"), ("tx_timeout", "1")],
        true,
        4.0,
    );
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 2.0, true);
    let f = fields(rx.as_ref());
    assert!(!f.transmitting);
    assert_eq!(
        (f.tx_kind, f.tx_frames),
        (Some(FreedvTxKind::Voice700D), 7),
        "1 s of 160 ms frames"
    );
    let b = host.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].2, "the end mark came");
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 0.5, true);
    call(rx.as_mut(), "halt", &[]).unwrap();
    idle(rx.as_mut(), &host, 0.5, true);
    let b = host.bursts();
    assert_eq!(b.len(), 2);
    assert!(b[1].2);
    assert!(!fields(rx.as_ref()).transmitting);
}

// ---------------------------------------------------------------------------------------------------------------------------
// 1600.

/// `n` Codec 2 1300 frames of random bits (52 bits, the low 4 bits of the seventh byte zero).
fn codec_frames_1300(n: usize, seed: u64) -> Vec<[u8; 7]> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            let mut b = [0u8; 56];
            rng.fill_bits(&mut b[..52]);
            std::array::from_fn(|i| (0..8).fold(0u8, |v, j| v | b[8 * i + j] << (7 - j)))
        })
        .collect()
}

/// Runs the 1600 sender `s` to its end with the codec frames `frames`, keying up after the last; the 8 kHz samples.
fn transmission_1600(mut s: FdmdvSender, frames: &[[u8; 7]]) -> Vec<Complex32> {
    let mut out = Vec::new();
    let mut k = 0;
    loop {
        if k >= frames.len() {
            s.stop();
        }
        let more = s.next(
            |f| {
                *f = frames[k];
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

/// `x` at 8 kHz moved by `offset_hz`, with white noise for `snr_db` in 3000 Hz against the mean power of `x` from its first
/// to its last sample above 1 % of its peak (`None`: no noise).
fn channel(x: &[Complex32], offset_hz: f64, snr_db: Option<f64>, seed: u64) -> Vec<Complex32> {
    let peak = x.iter().map(|z| z.norm()).fold(0f32, f32::max);
    let on: Vec<usize> = (0..x.len()).filter(|&i| x[i].norm() > 0.01 * peak).collect();
    let body = &x[on[0]..=on[on.len() - 1]];
    let power = body.iter().map(|z| z.norm_sqr()).sum::<f32>() as f64 / body.len() as f64;
    // Complex noise of power N over 8000 Hz has N × 3000 / 8000 in 3000 Hz.
    let sigma = snr_db.map_or(0.0, |s| (power * 8000.0 / 3000.0 / 10f64.powf(s / 10.0) / 2.0).sqrt() as f32);
    let mut rng = Rng::new(seed);
    (0..x.len() + 4000)
        .map(|n| {
            let turn = (offset_hz * n as f64 / 8000.0).fract() as f32 * std::f32::consts::TAU;
            let s = x.get(n).copied().unwrap_or_default();
            s * Complex32::from_polar(1.0, turn) + Complex32::new(rng.normal_f32(), rng.normal_f32()) * sigma
        })
        .collect()
}

/// The result fields of the stack in `submode` after `x` at [`RATE`], read while the signal is on.
fn fields_while_on(submode: &str, x: &[Complex32]) -> Json {
    let mut cfg = StackConfig::new(values("freedv", &[]));
    cfg.submode = Some(submode.into());
    let mut rx = (desc("freedv").make_receiver)(&cfg).expect("the receiver builds");
    rx.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .expect("configure");
    let mut out = sdrkit_core::Output::default();
    for c in x.chunks(4096) {
        rx.feed(c, &mut out);
        out.clear();
    }
    view(Owner::results(rx.as_ref()))["fields"].clone()
}

/// Where the frames of `sent` from `at` to the last one end `got`, in order and with nothing after them: the frames of
/// `got` before them (damaged frames while the receiver finds the symbol timing) and `at`.
fn run_to_end(sent: &[Vec<u8>], got: &[Vec<u8>]) -> Option<(usize, usize)> {
    (0..got.len()).find_map(|j| {
        let at = sent.iter().position(|s| *s == got[j])?;
        (got[j..] == sent[at..]).then_some((j, at))
    })
}

/// On 1600, 5 Hz off, with the clipper and the transmit filter off, the filter alone, and both, starting anywhere in a
/// symbol of the receiver: the receiver in the submodes 1600 and automatic reports the callsign text; within 0.6 s and
/// after at most 3 damaged frames it hands over every Codec 2 1300 frame bit for bit, to the last one and nothing after
/// it; while the signal is on, its submode is 1600.
#[test]
fn the_receiver_decodes_the_callsign_and_every_1600_codec_frame_to_the_last() {
    for (clip, filter) in [(false, false), (false, true), (true, true)] {
        let n = 240;
        let sent = codec_frames_1300(n, 3);
        let s8 = transmission_1600(FdmdvSender::new("N0CALL", clip, filter, 10_000), &sent);
        let sent: Vec<Vec<u8>> = sent.iter().map(|f| f.to_vec()).collect();
        for (sub, lead) in [("1600", 0), ("auto", 0), ("1600", 37), ("1600", 101)] {
            let mut x = vec![Complex32::default(); lead];
            x.extend_from_slice(&s8);
            let x = on_air(&channel(&x, 5.0, None, 1));
            let what = format!("{sub} clip {clip} filter {filter} start {lead}");
            let (r, got) = receive(sub, &x);
            let c = calls(&r);
            assert_eq!(c.len(), 1, "{what}: {}", show(&c));
            assert_eq!(
                (&c[0]["src"], &c[0]["text"]),
                (&"N0CALL".into(), &"N0CALL".into()),
                "{what}"
            );
            let (before, at) = run_to_end(&sent, &got)
                .unwrap_or_else(|| panic!("{what}: {} frames, not the last ones in order", got.len()));
            println!("{what}: {before} damaged frames, then frames {at} to {n} of {n}");
            assert!(at as f64 * 0.04 <= 0.6, "{what}: trusted after {} s", at as f64 * 0.04);
            assert!(before <= 3, "{what}: {before} damaged frames before");
        }
        let x = on_air(&channel(&s8, 5.0, None, 1));
        let f = fields_while_on("1600", &x[..x.len() / 2]);
        assert_eq!((&f["submode"], &f["sync_1600"]), (&"1600".into(), &"synced".into()), "{f}");
    }
}

/// Measured over the seeds 1 to 10 (250 frames each, white noise, SNR in 3000 Hz), from the first frame the receiver hands
/// over: without the clipper it hands over 97.0 % of the Codec 2 frames bit for bit at 9 dB and 99.7 % at 12 dB; with the
/// clipper and the filter, at the peak and the noise of 12 dB without (15.2 dB against its mean power), 99.6 %. The
/// specification gives no SNR for 1600.
#[test]
fn frame_success_of_1600_in_white_noise() {
    for (snr_db, least, clip) in [(9.0, 0.95, false), (12.0, 0.99, false), (15.2, 0.99, true)] {
        let (mut good, mut total) = (0, 0);
        for seed in 1..=10u64 {
            let sent = codec_frames_1300(250, seed);
            let s8 = transmission_1600(FdmdvSender::new("N0CALL", clip, clip, 10_000), &sent);
            let (_, got) = receive("1600", &on_air(&channel(&s8, 0.0, Some(snr_db), seed)));
            let here: Vec<bool> = sent.iter().map(|f| got.iter().any(|g| g[..] == f[..])).collect();
            let first = here.iter().position(|&h| h).unwrap_or(here.len());
            good += here[first..].iter().filter(|&&h| h).count();
            total += here.len() - first;
        }
        let rate = good as f64 / total as f64;
        println!("1600 clip {clip} at {snr_db} dB: {good} of {total} frames ({:.1} %)", 100.0 * rate);
        assert!(rate >= least, "1600 at {snr_db} dB: {good} of {total}");
    }
}

/// The 99 % power bandwidth of 1600 lies within 300 Hz to 2700 Hz of the sideband; no sample is above 0.9 of full scale,
/// the peak reaches it, and the peak-to-mean power ratio falls with the clipper.
#[test]
fn the_1600_signal_stays_in_its_band_at_its_level() {
    for (clip, filter, widest, papr_most) in [
        (false, false, 1350.0, 9.0),
        (false, true, 1350.0, 9.0),
        (true, false, 1350.0, 5.5),
    ] {
        let s8 = transmission_1600(
            FdmdvSender::new("N0CALL", clip, filter, 10_000),
            &codec_frames_1300(400, 5),
        );
        let peak = s8.iter().map(|z| z.norm()).fold(0f32, f32::max);
        assert!(peak <= PEAK + 1e-6 && peak > 0.8, "peak {peak}");
        let limited = s8.iter().filter(|z| z.norm() >= PEAK - 1e-4).count() as f64 / s8.len() as f64;
        let body = &s8[400..s8.len() - 400];
        let mean = body.iter().map(|z| z.norm_sqr()).sum::<f32>() / body.len() as f32;
        let papr = 10.0 * (peak * peak / mean).log10();
        let n = 1024;
        let mut p = vec![0f32; n];
        welch(body, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut p).unwrap();
        let pd: Vec<f64> = p.iter().map(|&v| f64::from(v)).collect();
        let (lo, hi) = power_share_edges(&pd, 0, n - 1, 0.99).unwrap();
        let hz = |i: usize| (i as f64 - (n / 2) as f64) * 8000.0 / n as f64;
        let bw = (hi - lo + 1) as f64 * 8000.0 / n as f64;
        println!(
            "1600 clip {clip} filter {filter}: {:.0} Hz to {:.0} Hz, {bw:.0} Hz, peak {peak:.3}, peak to mean {papr:.1} dB, {:.3} % of the samples at the limit",
            hz(lo),
            hz(hi),
            100.0 * limited
        );
        assert!(clip || limited < 0.005, "{limited}");
        assert!(hz(lo) >= 300.0 && hz(hi) <= 2700.0, "{} to {}", hz(lo), hz(hi));
        assert!(bw <= widest, "{bw} Hz");
        assert!(papr <= papr_most, "{papr} dB");
    }
}

/// A 1600 transmission is the first symbol and its frames of two symbols; it ends after the frame on air with the six
/// slots in which the shaped pulses end and the fall over their end, and at the time-out.
#[test]
fn a_1600_transmission_ends_after_the_frame_on_air() {
    let mut s = FdmdvSender::new("N0CALL", false, false, 10_000);
    let mut out = Vec::new();
    for _ in 0..3 {
        assert!(s.next(|f| *f = [0x55; 7], &mut out));
    }
    assert_eq!(out.len(), 160 + 3 * 320);
    s.stop();
    assert!(s.next(|_| unreachable!("no codec frame after the stop"), &mut out));
    assert_eq!(out.len(), 160 + 3 * 320 + 6 * 160, "the end of the pulses");
    assert!(out[out.len() - 1].norm() < 1e-3, "the fall");
    assert!(!s.next(|_| {}, &mut out));
    assert_eq!(s.frames_sent(), 3);
    let mut s = FdmdvSender::new("N0CALL", false, false, 5);
    let mut out = Vec::new();
    while s.next(|f| *f = [0; 7], &mut out) {}
    assert_eq!(s.frames_sent(), 5);
}

/// Push-to-talk in the submode 1600 sends the speech of the host's microphone: the receiver gets the callsign text and,
/// after two frames of coded silence, the Codec 2 1300 frames that an encoder makes of the same audio at 8 kHz, bit for
/// bit to the last one; key up ends the transmission after the frame on air with its end mark.
#[test]
fn push_to_talk_sends_the_microphone_bit_for_bit_on_1600() {
    let books = made_up_books(Mode::M1300);
    let (mut rx, host) = station_with("1600", &[("callsign", "N0CALL")], Some(books.clone()), 12.0);
    idle(rx.as_mut(), &host, 0.2, false);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 9.0, true);
    let f = fields(rx.as_ref());
    assert!(f.transmitting);
    assert_eq!(f.tx_kind, Some(FreedvTxKind::Voice1600));
    assert_eq!(f.tx_freq_hz.map(|h| h.0), Some(DIAL));
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 1.0, true);
    let f = fields(rx.as_ref());
    assert!(!f.transmitting);
    let b = host.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    let (samples, centre, ended, starts) = &b[0];
    assert!(*ended && starts.is_empty());
    assert_eq!(*centre, DIAL);
    let mut x = vec![Complex32::default(); 9600];
    x.extend_from_slice(samples);
    let (r, got) = receive("1600", &x);
    let c = calls(&r);
    assert_eq!(c.len(), 1, "{}", show(&c));
    assert_eq!(c[0]["src"], "N0CALL");
    // The same audio through a rate change to 8 kHz and a fresh encoder, after the two frames of coded silence.
    let sent = f.tx_frames as usize;
    let audio = host.mic.lock().unwrap().0.clone();
    let mut s8 = vec![0f32; 2 * 320];
    Resampler::<f32>::new(48_000.0, 8000.0)
        .unwrap()
        .process(&audio, &mut s8);
    let mut enc = Encoder::new(Mode::M1300, &books).unwrap();
    let want: Vec<Vec<u8>> = s8
        .as_chunks::<320>()
        .0
        .iter()
        .take(sent)
        .map(|c| {
            let pcm: Vec<i16> = c.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i16).collect();
            let mut f = vec![0u8; 7];
            enc.encode(&pcm, &mut f).unwrap();
            assert_eq!(f[6] & 0x0F, 0, "52 bits");
            f
        })
        .collect();
    assert_eq!(want.len(), sent);
    let at = run_in(&want, &got)
        .unwrap_or_else(|| panic!("the codec frames of the microphone, bit for bit, to the last"));
    println!("1600: {sent} frames; codec frames {at} to {sent} received");
    assert_eq!(got.len(), sent - at, "frames after the end");
    assert!(at as f64 * 0.04 <= 0.5, "trusted after {} s", at as f64 * 0.04);
}

/// In the submode 1600 push-to-talk is refused without a callsign text, without the Codec 2 1300 tables (the 700C ones
/// do not do) and with the transmitter off. Nothing goes on air.
#[test]
fn transmissions_on_1600_are_refused_and_nothing_is_sent() {
    let m1300 = || Some(made_up_books(Mode::M1300));
    let (mut rx, host) = station_with("1600", &[], m1300(), 1.0);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    assert_eq!(refusal(&e), Some(FreedvTxError::NoSource), "{e}");
    let (mut rx, host) = station("1600", &[("callsign", "N0CALL")], true, 1.0);
    idle(rx.as_mut(), &host, 0.2, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    assert!(matches!(refusal(&e), Some(FreedvTxError::NoTable(_))), "{e}");
    let (mut rx, host) = station_with("1600", &[("callsign", "N0CALL")], m1300(), 1.0);
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2, false);
    assert!(matches!(ptt(rx.as_mut(), true), Err(CommandError::TransmitOff)));
    idle(rx.as_mut(), &host, 0.3, false);
    assert!(host.bursts().is_empty());
}
