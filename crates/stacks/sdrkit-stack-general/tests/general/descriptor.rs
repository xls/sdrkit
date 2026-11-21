//! The stack as the engine sees it: the descriptor, its settings, receiver and generator, a run like that of the engine, the
//! declared results and events, and the preset command.

use crate::common::{values, Detail};
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Owner, StackConfig};
use sdrkit_stack_general::{descriptors, CARRIER_0_3_DBFS, DESCRIPTOR};

#[test]
fn the_descriptor() {
    let d = &DESCRIPTOR;
    assert_eq!(d.id, "general");
    assert_eq!(d.group, "Listening");
    assert_eq!(d.tuning.default_hz, 0.0);
    assert_eq!(d.tuning.rate_hz, 2e6);
    assert_eq!(d.tuning.min_rate_hz, 250e3);
    assert_eq!(d.tuning.tune_offset_hz, 250e3);
    assert_eq!(d.tuning.radio_filter_hz, Some(1.75e6));
    assert_eq!(d.tuning.min_hz, 0.1e6);
    assert_eq!(d.tuning.max_hz, 6000e6);
    assert!(
        d.submodes.iter().any(|s| s.id == "general") && d.submodes.iter().any(|s| s.id == "am")
    );
    assert_eq!(descriptors().len(), 1);
    let schema = (d.settings)();
    for key in [
        "demod",
        "width",
        "channel_offset",
        "step",
        "squelch",
        "squelch_level",
        "agc",
        "gain",
        "deemph_nfm",
        "deemph_wfm",
        "nfm_lowpass",
        "cw_pitch",
        "am_auto",
        "am_detector",
        "am_nr",
        "am_bandwidth",
        "am_enhance",
        "nb",
        "nb_threshold",
        "af_highpass",
        "af_lowpass",
        "af_notch",
        "voice_nr",
        "voice_nr_strength",
        "presets",
        "tone_gate",
        "listen_channel",
    ] {
        let id = schema.id(key).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            !schema.setting(id).unwrap().restart,
            "{key} needs no restart"
        );
    }
    assert!(
        schema.id("rec_record").is_err(),
        "recording is not a setting of the stack"
    );
    assert_eq!(d.lock_profile.capture, &["tone"]);
    assert!(serde_json::to_string(&schema)
        .unwrap()
        .contains("Auto quality"));
}

#[test]
fn a_run_like_that_of_the_engine() {
    let d = &DESCRIPTOR;
    let level = CARRIER_0_3_DBFS.to_string();
    let gen_settings = values(&[
        ("gen_noise", "snr"),
        ("gen_snr_db", "25"),
        ("gen_level_dbfs", &level),
        ("gen_seed", "3"),
    ]);
    let settings = gen_settings.clone();
    let rate = d.tuning.rate_hz;
    // The generator through the framework's impairments, as the engine runs it.
    let clean = (d.make_generator.unwrap())(&GenConfig::new(rate, gen_settings.clone()).with_seed(3))
    .expect("the generator builds");
    let mut g = sdrkit_components::sim::impaired(clean, &gen_settings).expect("valid settings");
    let mut rx = (d.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    let tuned = d.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, tuned + d.tuning.tune_offset_hz, tuned).recording())
        .expect("the rate is accepted");
    let mut out = Output::default();
    let mut buf = vec![Complex32::default(); 65_536];
    let mut reports = 0;
    let mut last_blocks = 0;
    let mut audio = 0;
    let total = (3.0 * rate) as usize;
    let mut done = 0;
    while done < total {
        let n = buf.len().min(total - done);
        assert_eq!(g.generate(&mut buf[..n]), n, "no samples dropped");
        out.clear();
        rx.feed(&buf[..n], &mut out);
        for p in out.audio_runs() {
            audio += p.samples.len() / usize::from(p.channels.max(1));
            assert!(p.rate_hz == 48_000.0);
        }
        // A new report moves the block counters on.
        let det = Detail::of(Owner::results(rx.as_ref()));
        let blocks = det.blocks_open + det.blocks_closed;
        if blocks != last_blocks {
            reports += 1;
            last_blocks = blocks;
        }
        done += n;
    }
    let det = Detail::of(Owner::results(rx.as_ref()));
    assert!(reports > 10, "{reports} reports");
    assert_eq!(
        det.blocks_open + det.blocks_closed,
        300,
        "3 s of 10 ms blocks"
    );
    assert!(
        det.squelch_open && det.noise_valid && det.snr_db > 15.0,
        "{}",
        det.summary
    );
    assert!((audio as f64 - 3.0 * 48_000.0).abs() < 2000.0, "{audio}");
    let json = serde_json::to_value(Owner::results(rx.as_ref())).unwrap();
    assert_eq!(json["status"]["lock_state"], "locked");
}

#[test]
fn the_benchmarks_run() {
    for b in sdrkit_stack_general::benchmarks() {
        let r = (b.run)(0.01);
        assert!(r.items_per_second > 0.0, "{}", b.name);
    }
}

/// Every event that the receiver emits is declared, and no event sends its frequency as 0: an NFM station with a CTCSS tone,
/// and one with a DCS code.
#[test]
fn events_are_declared_and_name_their_frequency() {
    use crate::common::*;
    use sdrkit_stack_general::results::declaration;
    let decl = declaration();
    type Pairs<'a> = &'a [(&'a str, &'a str)];
    let cases: [(Pairs, Pairs); 2] = [
        (
            &[("demod", "nfm")],
            &[("gen_ctcss", "88.5"), ("gen_snr_db", "30")],
        ),
        (
            &[("demod", "nfm")],
            &[("gen_dcs", "023N"), ("gen_snr_db", "30")],
        ),
    ];
    let mut kinds = std::collections::BTreeSet::new();
    for (rx_settings, gen_settings) in cases {
        let mut rx = receiver(2e6, -250e3, rx_settings);
        let mut g = generator(2e6, "nfm", gen_settings);
        let mut r = run_gen(&mut rx, &mut g, 3.0, 20_000);
        g.set_station(false);
        let tail = run_gen(&mut rx, &mut g, 1.0, 20_000);
        r.events.extend(tail.events);
        for e in &r.events {
            assert!(
                decl.events().iter().any(|d| d.kind == e.kind()),
                "{} is not declared",
                e.kind()
            );
            assert!(e.freq_hz > 0.0, "{} sends no frequency", e.kind());
            kinds.insert(e.kind().to_owned());
        }
    }
    assert!(
        kinds.contains("tones.subaudio"),
        "no tones.subaudio event in {kinds:?}"
    );
}

/// The presets are a table, and `preset.apply` sets the demodulator, the width and the squelch of one and requests the host to
/// tune to its frequency. A key that matches no preset is refused.
#[test]
fn a_preset_is_listed_and_applied() {
    use crate::common::receiver;
    use sdrkit_core::{owner, CommandError, HostRequest, Receiver, Value};
    use sdrkit_stack_general::typed::{PresetRow, PRESETS};
    use sdrkit_stack_general::Demod;
    let text = crate::common::presets_text();
    let mut rx = receiver(2e6, -250e3, &[("presets", &text)]);
    let r = Owner::results(&rx);
    let t = r.table(r.decl().table_id(PRESETS.key).unwrap()).unwrap();
    assert_eq!(t.len(), crate::common::presets().len());
    assert_eq!(
        t.row("ham_20m_usb").unwrap().at(&PresetRow::DEMOD),
        Some(Demod::Usb)
    );
    let id = rx.commands().id("preset.apply").unwrap();
    let def = rx.commands().def(id).unwrap().clone();
    let mut args = def.new_args();
    let row = def.args.record_decl().id("row").unwrap();
    args.set_value(row, &Value::Text("ham_20m_usb".into()))
        .unwrap();
    owner::invoke(&mut rx, id, &args).unwrap();
    assert_eq!(rx.settings().demod, Demod::Usb);
    assert_eq!(rx.settings().width(), 2700.0);
    let mut out = Output::default();
    rx.feed(&[Complex32::default(); 1000], &mut out);
    assert!(matches!(
        out.requests.as_slice(),
        [HostRequest::Retune { tuned_hz, .. }] if *tuned_hz == 14.2e6
    ));
    args.set_value(row, &Value::Text("nowhere".into())).unwrap();
    assert!(matches!(
        owner::invoke(&mut rx, id, &args),
        Err(CommandError::NoSuchRow { .. })
    ));
}
