//! `mac`: the generator through impairments into the receiver.
use super::common::{impaired, pictures, values_of, AtvPicture, Imp};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    Choice, Complex32, Generator, InputInfo, Output, Owner, Receiver, Results, SettingSet,
    StackConfig,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_analogtv::card::EBU_BARS;
use sdrkit_stack_analogtv::mac::frame::Variant;
use sdrkit_stack_analogtv::mac::gen::{MacGenParams, MacGenerator};
use sdrkit_stack_analogtv::results::SoundCoding;
use sdrkit_stack_analogtv::settings::MacSetting;
use sdrkit_stack_analogtv::typed::MacFields;

/// The demultiplexer's part of a MAC report.
#[derive(Clone, Debug, Default)]
struct Mux {
    channel: String,
    network: String,
    country: String,
    sound_errors: u64,
    corrected: u64,
    ca_system: String,
    sound_scrambled: bool,
    ecm_packets: u64,
}

/// The line decoder's part of a MAC report.
#[derive(Clone, Debug, Default)]
struct Lines {
    line_ppm: f64,
}

/// What the `mac` results say.
#[derive(Clone, Debug, Default)]
struct MacDetail {
    state: u8,
    data_valid: bool,
    variant: String,
    scrambled: bool,
    aspect: String,
    sound_present: bool,
    carrier_offset_hz: f64,
    inverted: bool,
    summary: String,
    mux: Mux,
    lines: Lines,
}

fn read(res: &Results) -> MacDetail {
    // Before the first report the fields have no values.
    let Some(f) = MacFields::read(res.fields()) else {
        return MacDetail {
            summary: res.status().summary.clone(),
            ..MacDetail::default()
        };
    };
    let text = |x: Option<&str>| x.unwrap_or("").to_string();
    let state = f.state.index() as u8;
    MacDetail {
        state,
        data_valid: state == 2,
        variant: f.system.map(|e| e.label().to_string()).unwrap_or_default(),
        scrambled: f.scrambled,
        aspect: f.aspect.map(|e| e.label().to_string()).unwrap_or_default(),
        sound_present: f.sound_present,
        carrier_offset_hz: f.offset_hz.0,
        inverted: f.spectrum_inverted,
        summary: res.status().summary.clone(),
        mux: Mux {
            channel: text(f.channel),
            network: text(f.network),
            country: text(f.country),
            sound_errors: f.sound_errors as u64,
            corrected: f.packets_corrected as u64,
            ca_system: f
                .ca_system
                .map(|e| e.label().to_string())
                .unwrap_or_default(),
            sound_scrambled: f.sound_mode == SoundCoding::Scrambled,
            ecm_packets: f.ecm_packets as u64,
        },
        lines: Lines {
            line_ppm: f.line_error_ppm,
        },
    }
}

/// A `mac` receiver.
fn mac_receiver() -> StackGraph {
    let v = values_of(&MacSetting::schema(), &[]);
    sdrkit_stack_analogtv::mac::receiver(&StackConfig::new(v)).unwrap()
}

struct MacRun {
    d: MacDetail,
    reports: Vec<MacDetail>,
    audio: Vec<f32>,
    rate_hz: f64,
    picture: Option<AtvPicture>,
    pictures: u64,
}

/// The MAC generator through `imp` (its carrier to noise is in 27 MHz) into a fresh receiver.
fn mac_run(p: MacGenParams, imp: Imp, seconds: f64) -> MacRun {
    let rate = p.rate_hz;
    let mut g = impaired(Box::new(MacGenerator::new(p)), imp);
    let mut rx = mac_receiver();
    rx.configure(&InputInfo::new(rate, 1200e6, 1200e6).recording())
        .unwrap();
    let mut buf = vec![Complex32::default(); 100_000];
    let mut out = Output::default();
    let total = (seconds * rate) as usize;
    let mut done = 0;
    let mut r = MacRun {
        d: MacDetail::default(),
        reports: Vec::new(),
        audio: Vec::new(),
        rate_hz: 32_000.0,
        picture: None,
        pictures: 0,
    };
    let mut counter = Owner::results(&rx).change_counter();
    while done < total {
        let n = buf.len().min(total - done);
        g.generate(&mut buf[..n]);
        out.clear();
        rx.feed(&buf[..n], &mut out);
        r.audio.extend_from_slice(out.audio());
        if !out.audio().is_empty() {
            r.rate_hz = out.audio_format().map_or(r.rate_hz, |f| f.0);
        }
        let colour = true;
        for p in pictures(&out) {
            r.pictures += 1;
            r.picture = Some(AtvPicture { picture: p, colour });
        }
        let res = Owner::results(&rx);
        if res.change_counter() != counter {
            counter = res.change_counter();
            let d = read(res);
            r.reports.push(d.clone());
            r.d = d;
        }
        done += n;
    }
    r
}

/// The frequency of the sound (mono at 32 kHz) from zero crossings of the second half.
fn tone_hz(audio: &[f32]) -> f64 {
    let x = &audio[audio.len() / 2..];
    let mut ups = Vec::new();
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            ups.push(i as f64 - 1.0 + (-x[i - 1] / (x[i] - x[i - 1])) as f64);
        }
    }
    assert!(ups.len() > 10, "no tone");
    (ups.len() - 1) as f64 * 32_000.0 / (ups[ups.len() - 1] - ups[0])
}

/// The mean absolute difference (0..255) of the bars against the card's EBU bars (columns 46 to 721, rows 40 to 172 at 768 wide).
fn bars_diff(p: &AtvPicture) -> f64 {
    let l = p.picture.layout.as_ref().unwrap();
    let w = l.width as usize;
    let (x0, x1) = (0.06 * w as f64, 0.94 * w as f64);
    let mut s = 0.0;
    let mut n = 0.0;
    for (k, bar) in EBU_BARS.iter().enumerate() {
        let a = x0 + (x1 - x0) * k as f64 / 8.0;
        let b = x0 + (x1 - x0) * (k + 1) as f64 / 8.0;
        let (ca, cb) = ((a + 0.2 * (b - a)) as usize, (b - 0.2 * (b - a)) as usize);
        for r in 60..150 {
            for c in ca..cb {
                let i = (r * w + c) * 4;
                let got = [
                    p.picture.data[i + 2],
                    p.picture.data[i + 1],
                    p.picture.data[i],
                ];
                for ch in 0..3 {
                    s += (got[ch] as f64 - bar[ch] * 255.0).abs();
                    n += 1.0;
                }
            }
        }
    }
    s / n
}

#[test]
fn m1_d2_mac_decodes_the_service_and_the_sound() {
    let r = mac_run(
        MacGenParams::default(),
        Imp {
            cnr_db: 40.0,
            ..Default::default()
        },
        1.0,
    );
    let d = &r.d;
    assert_eq!(d.state, 2);
    assert!(d.data_valid);
    assert_eq!(d.variant, Variant::D2.label());
    assert_eq!(d.mux.channel, "TEST TV");
    assert_eq!(d.mux.network, "TEST SATELLITE");
    assert_eq!(d.mux.country, "TEST COUNTRY");
    assert!(!d.scrambled);
    assert_eq!(d.aspect, "4:3");
    assert!(d.sound_present);
    assert_eq!(d.mux.sound_errors, 0);
    assert_eq!(d.mux.corrected, 0);
    let f = tone_hz(&r.audio);
    assert!((f - 1000.0).abs() < 5.0, "tone {f}");
    let p = r.picture.as_ref().unwrap();
    assert_eq!(p.picture.layout.as_ref().unwrap().width, 768);
    assert!(bars_diff(p) < 14.0, "bars {}", bars_diff(p));
    assert!(r.reports.iter().any(|d| d.summary.contains("TEST TV")));
}

#[test]
fn m2_offset_and_another_tone() {
    let r = mac_run(
        MacGenParams {
            tone_hz: 1500.0,
            ..MacGenParams::default()
        },
        Imp {
            cnr_db: 30.0,
            offset_hz: 200e3,
            ..Default::default()
        },
        1.0,
    );
    assert_eq!(r.d.state, 2);
    assert!(
        (r.d.carrier_offset_hz - 200e3).abs() < 20e3,
        "offset {}",
        r.d.carrier_offset_hz
    );
    assert!((tone_hz(&r.audio) - 1500.0).abs() < 8.0);
    assert!(bars_diff(r.picture.as_ref().unwrap()) < 16.0);
}

#[test]
fn m3_m4_m5_m6_noise_wide_scrambled_inverted() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let r = mac_run(
                MacGenParams::default(),
                Imp {
                    cnr_db: 18.0,
                    ..Default::default()
                },
                1.5,
            );
            assert_eq!(r.d.state, 2);
            assert_eq!(r.d.mux.channel, "TEST TV");
            assert!((tone_hz(&r.audio) - 1000.0).abs() < 8.0);
            assert!(bars_diff(r.picture.as_ref().unwrap()) < 22.0);
        });
        s.spawn(|| {
            let r = mac_run(
                MacGenParams {
                    wide: true,
                    ..MacGenParams::default()
                },
                Imp {
                    cnr_db: 30.0,
                    ..Default::default()
                },
                1.0,
            );
            assert_eq!(r.d.aspect, "16:9");
            let p = r.picture.as_ref().unwrap();
            assert_eq!(p.picture.layout.as_ref().unwrap().width, 1024);
            assert!(bars_diff(p) < 16.0);
        });
        s.spawn(|| {
            let r = mac_run(
                MacGenParams {
                    scrambled: true,
                    ..MacGenParams::default()
                },
                Imp {
                    cnr_db: 30.0,
                    ..Default::default()
                },
                1.0,
            );
            let d = &r.d;
            assert_eq!(d.state, 2);
            assert!(d.scrambled);
            assert_eq!(d.mux.ca_system, "EuroCrypt");
            assert!(d.mux.sound_scrambled);
            assert!(d.mux.ecm_packets > 0);
            assert!(!d.sound_present);
            assert!(
                r.audio.iter().all(|x| x.abs() < 1e-3),
                "scrambled sound is not played"
            );
            assert_eq!(d.mux.channel, "TEST TV");
        });
        s.spawn(|| {
            let r = mac_run(
                MacGenParams::default(),
                Imp {
                    cnr_db: 30.0,
                    inverted: true,
                    ..Default::default()
                },
                1.0,
            );
            assert_eq!(r.d.state, 2);
            assert!(r.d.inverted);
            assert!(bars_diff(r.picture.as_ref().unwrap()) < 16.0);
        });
    });
}

#[test]
fn m7_m8_d_mac() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let r = mac_run(
                MacGenParams {
                    variant: Variant::D,
                    tone_hz: 800.0,
                    ..MacGenParams::default()
                },
                Imp {
                    cnr_db: 35.0,
                    ..Default::default()
                },
                1.0,
            );
            assert_eq!(r.d.state, 2);
            assert_eq!(r.d.variant, Variant::D.label());
            assert_eq!(r.d.mux.channel, "TEST TV");
            assert!((tone_hz(&r.audio) - 800.0).abs() < 5.0);
            assert!(bars_diff(r.picture.as_ref().unwrap()) < 16.0);
        });
        s.spawn(|| {
            let r = mac_run(
                MacGenParams {
                    variant: Variant::D,
                    ..MacGenParams::default()
                },
                Imp {
                    cnr_db: 20.0,
                    offset_hz: 200e3,
                    ..Default::default()
                },
                1.2,
            );
            assert_eq!(r.d.state, 2);
            assert_eq!(r.d.mux.channel, "TEST TV");
            assert!((tone_hz(&r.audio) - 1000.0).abs() < 8.0);
        });
    });
}

#[test]
fn m9_clock_error() {
    std::thread::scope(|s| {
        for ppm in [-60.0, 60.0] {
            s.spawn(move || {
                let r = mac_run(
                    MacGenParams::default(),
                    Imp {
                        cnr_db: 30.0,
                        clock_ppm: ppm,
                        ..Default::default()
                    },
                    1.0,
                );
                assert_eq!(r.d.state, 2);
                assert!(
                    (r.d.lines.line_ppm + ppm).abs() < 10.0,
                    "{ppm}: {}",
                    r.d.lines.line_ppm
                );
                assert!(bars_diff(r.picture.as_ref().unwrap()) < 16.0);
                let f = tone_hz(&r.audio);
                assert!((f - 1000.0 * (1.0 + ppm * 1e-6)).abs() < 8.0);
            });
        }
    });
}

#[test]
fn m10_other_rates() {
    std::thread::scope(|s| {
        for (rate, v) in [(16e6, Variant::D2), (25e6, Variant::D2), (25e6, Variant::D)] {
            s.spawn(move || {
                let r = mac_run(
                    MacGenParams {
                        rate_hz: rate,
                        variant: v,
                        ..MacGenParams::default()
                    },
                    Imp {
                        cnr_db: 30.0,
                        ..Default::default()
                    },
                    1.0,
                );
                assert_eq!(r.d.state, 2, "{rate} {v:?}");
                assert_eq!(r.d.mux.channel, "TEST TV");
                assert!(bars_diff(r.picture.as_ref().unwrap()) < 18.0);
            });
        }
    });
}

#[test]
fn m11_noise_only_and_low_rates() {
    let r = mac_run(
        MacGenParams::default(),
        Imp {
            cnr_db: -20.0,
            ..Default::default()
        },
        0.5,
    );
    assert_eq!(r.d.state, 0);
    assert_eq!(r.pictures, 0);
    let mut rx = mac_receiver();
    assert!(rx.configure(&InputInfo::new(10e6, 1e9, 1e9)).is_err());
}

#[test]
fn m13_in_the_stack_descriptor() {
    let d = &sdrkit_stack_analogtv::MAC;
    let values = sdrkit_core::Values::defaults(&(d.settings)());
    let mut g = (d.make_generator.unwrap())(
        &sdrkit_core::GenConfig::new(20e6, values.clone()).with_seed(1),
    )
    .unwrap();
    let mut rx = (d.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(20e6, 1200e6, 1200e6).recording())
        .unwrap();
    let mut buf = vec![Complex32::default(); 200_000];
    let mut out = Output::default();
    let mut seqs = Vec::new();
    for _ in 0..100 {
        g.generate(&mut buf);
        out.clear();
        rx.feed(&buf, &mut out);
        if let Some(n) = Owner::results(rx.as_ref()).fields().at(&MacFields::FRAMES) {
            if seqs.last() != Some(&n) {
                seqs.push(n);
            }
        }
    }
    let res = Owner::results(rx.as_ref());
    assert!(seqs.len() > 3);
    assert!(seqs.windows(2).all(|w| w[1] > w[0]));
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    assert_eq!(res.fields().at(&MacFields::CHANNEL), Some("TEST TV"));
}
