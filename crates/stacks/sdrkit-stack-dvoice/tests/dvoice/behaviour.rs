//! The shared behaviour of the stacks: descriptors and rates, the verdict, lists across a retune, saving and loading, the
//! sequence, no invented entries, the encryption rule of the media output, and the voice monitor with a codec plug-in.
use crate::common::*;
use sdrkit_core::codec::{AudioCodec, CodecError, CodecSet, Pcm};
use sdrkit_core::{owner, Complex32, FourCc, InputInfo, Output, Receiver, StateBlob};

/// Runs command `key` of `rx` with its default arguments.
pub fn command(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).expect("a command of the stack");
    let args = rx.commands().defs()[usize::from(id.0)].new_args();
    owner::invoke(rx, id, &args).expect("the command runs");
}
use std::sync::Arc;

/// Complex white noise of standard deviation `sigma` per part.
pub fn noise(sigma: f32, seed: u64, n: usize) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    (0..n)
        .map(|_| Complex32::new(rng.normal_f32() * sigma, rng.normal_f32() * sigma))
        .collect()
}

/// Adds noise at `snr_db` below the mean power of the samples where the carrier is on.
fn add_noise(x: &mut [Complex32], snr_db: f64, seed: u64) {
    let on: Vec<f32> = x
        .iter()
        .map(|s| s.norm_sqr())
        .filter(|&p| p > 1e-4)
        .collect();
    let p = on.iter().sum::<f32>() / on.len().max(1) as f32;
    let sigma = (p / 2.0 / 10f32.powf(snr_db as f32 / 10.0)).sqrt();
    let n = noise(sigma, seed, x.len());
    for (s, n) in x.iter_mut().zip(n) {
        *s += n;
    }
}

/// Moves the samples by `hz`.
fn shift(x: &mut [Complex32], rate: f64, hz: f64) {
    sdrkit_algo_dsp::nco::Nco::new(rate, hz).mix_in_place(x);
}

#[test]
fn every_mode_has_a_descriptor_a_generator_and_settings() {
    let ids: Vec<&str> = sdrkit_stack_dvoice::descriptors()
        .iter()
        .map(|d| d.id)
        .collect();
    for id in [
        "dmr",
        "nxdn",
        "dpmr",
        "tetra",
        "p25",
        "p25p2",
        "tetrapol",
        "dstar",
        "ysf",
        "m17",
        "freedv",
        "dvoice-auto",
    ] {
        assert!(ids.contains(&id), "{id}");
    }
    for d in sdrkit_stack_dvoice::descriptors() {
        assert_eq!(d.group, "Digital voice");
        assert!(d.make_generator.is_some(), "{}", d.id);
        let s = (d.settings)();
        for key in [
            "squelch_pct",
            "allow_only",
            "allow",
            "mute_list",
            "priority",
            "hold",
            "labels",
            "hold_timeout_s",
            "hang_s",
            "listen",
        ] {
            assert!(s.id(key).is_ok(), "{} lacks {key}", d.id);
        }
        // Every setting reaches the receiver.
        let mut rx = receiver(d.id, &[]);
        for (i, setting) in s.settings().iter().enumerate() {
            if !setting.field.key.starts_with("gen_") {
                let id = sdrkit_core::SettingId(i as u16);
                owner::apply(rx.as_mut(), id, &setting.default)
                    .unwrap_or_else(|e| panic!("{} {}: {e}", d.id, setting.field.key));
            }
        }
    }
}

#[test]
fn a_rate_below_the_minimum_is_refused_with_its_text() {
    let mut rx = receiver("dmr", &[]);
    assert!(rx
        .configure(&InputInfo::new(48_000.0, 446e6, 446e6))
        .is_err());
    assert_eq!(
        rx.results().status().summary,
        "DMR needs a sample rate of at least 96 ksps"
    );
    let mut rx = receiver("tetrapol", &[]);
    assert!(rx
        .configure(&InputInfo::new(64_000.0, 392e6, 392e6))
        .is_ok());
}

#[test]
fn the_receivers_work_at_any_tested_rate() {
    for rate in [96_000.0, 250_000.0, 1_000_000.0, 2_048_000.0] {
        let mut g = generator("dmr", rate, 3, &[]);
        let x = generate_all(g.as_mut());
        let mut rx = receiver("dmr", &[]);
        let (r, _) = run(rx.as_mut(), rate, &x, 8192);
        let c = calls(&r);
        assert!(
            c.iter().any(|c| c["src"] == 3101234 && c["dst"] == 91),
            "at {rate}: {}",
            show(&c)
        );
    }
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32]) -> Output {
    let mut all = Output::default();
    let mut out = Output::default();
    for c in x.chunks(4800) {
        rx.feed(c, &mut out);
        for p in out.audio_runs() {
            all.push_audio(p.samples, p.rate_hz, p.channels);
        }
        all.events.append(&mut out.events);
        out.clear();
    }
    all
}

#[test]
fn the_verdict_says_why_nothing_decodes() {
    let rate = 96_000.0;
    // Silence: no input.
    let mut rx = receiver("p25", &[]);
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 96_000]);
    assert_eq!(field(rx.results(), "verdict"), "no_input");
    assert_eq!(rx.results().status().summary, "No signal at the channel");
    // Noise: power, no sync. And nothing logged.
    let mut rx = receiver("p25", &[]);
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    feed(rx.as_mut(), &noise(0.05, 9, 5 * 96_000));
    assert!(
        rx.results()
            .status()
            .summary
            .starts_with("Power, but no frame sync"),
        "{}",
        rx.results().status().summary
    );
    let r = view(rx.results());
    assert!(calls(&r).is_empty());
    // A decoding signal.
    let mut g = generator("p25", rate, 1, &[]);
    let x = generate_all(g.as_mut());
    let mut rx = receiver("p25", &[]);
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    feed(rx.as_mut(), &x[..(3.0 * rate) as usize]);
    assert_eq!(field(rx.results(), "verdict"), "decoding");
}

#[test]
fn noise_and_a_neighbouring_channel_invent_no_entries() {
    for id in [
        "dmr",
        "nxdn",
        "dpmr",
        "p25",
        "ysf",
        "m17",
        "dstar",
        "dvoice-auto",
    ] {
        let rate = 96_000.0;
        let mut rx = receiver(id, &[]);
        rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
            .unwrap();
        feed(rx.as_mut(), &noise(0.1, 5, 8 * 96_000));
        let r = view(rx.results());
        assert!(
            calls(&r).is_empty(),
            "{id} logged from noise: {}",
            show(&calls(&r))
        );
    }
    // A DMR base station 12.5 kHz away: nothing from it.
    let rate = 96_000.0;
    let mut g = generator("dmr", rate, 3, &[]);
    let mut x = generate_all(g.as_mut());
    shift(&mut x, rate, 12_500.0);
    add_noise(&mut x, 25.0, 4);
    let mut rx = receiver("dmr", &[]);
    let (r, _) = run(rx.as_mut(), rate, &x, 4800);
    assert!(calls(&r).is_empty(), "{}", show(&calls(&r)));
}

#[test]
fn a_retune_keeps_the_lists_and_closes_the_call_in_progress() {
    let rate = 96_000.0;
    let mut g = generator("dmr", rate, 3, &[]);
    let x = generate_all(g.as_mut());
    let mut rx = receiver("dmr", &[]);
    rx.configure(&InputInfo::new(rate, 446e6, 446e6).recording())
        .unwrap();
    // Up to the middle of the first call.
    feed(rx.as_mut(), &x[..(1.0 * rate) as usize]);
    let seq_before = view(rx.results())["detail"]["sequence"].as_u64().unwrap();
    rx.retuned(&InputInfo::new(rate, 446.1e6, 446.1e6).recording());
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 48_000]);
    let r = view(rx.results());
    let c = calls(&r);
    assert!(!c.is_empty());
    let first = &c[0];
    assert_eq!(first["active"], false);
    assert_eq!(first["end"], "closed");
    assert!(r["detail"]["sequence"].as_u64().unwrap() > seq_before);
}

#[test]
fn lists_are_saved_and_restored_and_another_version_is_refused() {
    let rate = 96_000.0;
    let mut g = generator("m17", rate, 3, &[]);
    let mut x = generate_all(g.as_mut());
    add_noise(&mut x, 25.0, 8);
    let mut rx = receiver("m17", &[]);
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    feed(rx.as_mut(), &x);
    let n = calls(&view(rx.results())).len();
    assert!(n >= 3, "{n}: {}", show(&calls(&view(rx.results()))));
    let blob = rx.save_state().expect("a saved state");
    // The clear command empties the lists.
    command(rx.as_mut(), "clear");
    feed(rx.as_mut(), &[Complex32::new(0.0, 0.0); 4800]);
    assert_eq!(calls(&view(rx.results())).len(), 0);
    rx.restore_state(&blob).unwrap();
    feed(rx.as_mut(), &[Complex32::new(0.0, 0.0); 4800]);
    assert_eq!(calls(&view(rx.results())).len(), n);
    // A state of another version or another stack changes nothing.
    command(rx.as_mut(), "clear");
    feed(rx.as_mut(), &[Complex32::new(0.0, 0.0); 4800]);
    let other_version = StateBlob::new(
        blob.owner().to_owned(),
        blob.version() + 8,
        blob.bytes().to_vec(),
    );
    assert!(rx.restore_state(&other_version).is_err());
    let other_stack = StateBlob::new("dmr", blob.version(), blob.bytes().to_vec());
    assert!(rx.restore_state(&other_stack).is_err());
    assert_eq!(calls(&view(rx.results())).len(), 0);
}

/// A test plug-in: each frame becomes 20 ms of a 1 kHz tone at 8 kHz.
struct Tone(u32);
impl AudioCodec for Tone {
    fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
        pcm.rate_hz = 8000;
        pcm.channels = 1;
        for _ in 0..160 {
            let v = if erasure || frame.is_empty() {
                0.0
            } else {
                (self.0 as f32 * std::f32::consts::TAU / 8.0).sin() * 8000.0
            };
            self.0 += 1;
            pcm.samples.push(v as i16);
        }
        Ok(())
    }
}

fn tone_codecs(code: &str) -> CodecSet {
    let mut set = CodecSet::default();
    set.register(
        FourCc::new(code),
        None,
        Arc::new(|_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Tone(0)) as Box<dyn AudioCodec>)
        }),
    );
    set
}

#[test]
fn clear_speech_reaches_the_codec_and_the_speaker_and_encrypted_speech_never_does() {
    let rate = 96_000.0;
    let mut g = generator("p25", rate, 3, &[]);
    let x = generate_all(g.as_mut());
    let mut rx = receiver_with_codecs("p25", &[], tone_codecs("imbe"));
    let (r, out) = run(rx.as_mut(), rate, &x, 4800);
    assert!(out.audio().iter().any(|&a| a.abs() > 0.05));
    let c = calls(&r);
    let clear = c
        .iter()
        .find(|c| c["dst"] == 1201 && c["kind"] == "voice")
        .unwrap();
    let enc = c.iter().find(|c| c["dst"] == 1300).unwrap();
    assert_eq!(clear["media_frames"], clear["voice_frames"]);
    assert_eq!(enc["media_frames"], 0);
    assert!(enc["voice_frames"].as_u64().unwrap() > 0);
    // With `listen` off nothing is emitted.
    let mut rx = receiver_with_codecs("p25", &[("listen", "0")], tone_codecs("imbe"));
    let (r, out) = run(rx.as_mut(), rate, &x, 4800);
    assert!(out.audio().iter().all(|&a| a == 0.0));
    assert!(calls(&r).iter().all(|c| c["media_frames"] == 0));
}

#[test]
fn without_a_codec_the_mode_says_so() {
    let rate = 96_000.0;
    let mut g = generator("dmr", rate, 3, &[]);
    let x = generate_all(g.as_mut());
    let mut rx = receiver("dmr", &[]);
    let (r, out) = run(rx.as_mut(), rate, &x, 4800);
    assert!(out.audio().iter().all(|&a| a == 0.0));
    assert_eq!(
        r["fields"]["codec"],
        "AMBE+2 decoder not installed: identities and messages are shown, no sound"
    );
}

#[test]
fn the_monitor_mutes_and_filters_by_list() {
    use sdrkit_stack_dvoice::typed::HISTORY;
    let rate = 96_000.0;
    let mut g = generator("p25", rate, 3, &[]);
    let x = generate_all(g.as_mut());
    // TG 1201 on the mute list: no sound from it, the u2u call still plays.
    let mut rx = receiver_with_codecs("p25", &[("mute_list", "TG 1201")], tone_codecs("imbe"));
    run(rx.as_mut(), rate, &x, 4800);
    let hist = table(rx.results(), &HISTORY);
    let v = |dst: u64| {
        hist.iter()
            .find(|h| h["dst"].as_str().and_then(|s| s.parse::<u64>().ok()) == Some(dst))
            .map(|h| h["verdict"].as_str().unwrap().to_string())
    };
    assert_eq!(v(1201).as_deref(), Some("muted"));
    assert_eq!(v(1300).as_deref(), Some("encrypted"));
    assert_eq!(v(2003).as_deref(), Some("play"));
}
