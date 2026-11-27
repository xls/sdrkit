//! FM video and NBTV in `atv`, and the `fpv` stack.

use super::common::*;
use sdrkit_core::{
    Choice, Complex32, Generator, InputInfo, Output, Owner, Picture, Receiver, SettingSet,
    StackConfig,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_analogtv::atv::nbtv::{NbtvFormat, NbtvGenerator};
use sdrkit_stack_analogtv::card::nbtv_cell;
use sdrkit_stack_analogtv::channels;
use sdrkit_stack_analogtv::fpv::{FpvGenParams, FpvGenerator};
use sdrkit_stack_analogtv::settings::FpvSetting;
use sdrkit_stack_analogtv::typed::{FpvChannelRow, FpvFields, FPV_CHANNELS};

/// FM video at 30 dB carrier to noise in 17 MHz.
fn fm_video(p: FpvGenParams, seconds: f64) -> Run {
    fm_video_imp(p, Imp::cnr(30.0), seconds)
}

fn fm_video_imp(p: FpvGenParams, imp: Imp, seconds: f64) -> Run {
    let rate = p.rate_hz;
    let mut g = impaired(
        Box::new(FpvGenerator::new(FpvGenParams { sound_hz: 0.0, ..p })),
        imp,
    );
    let mut rx = atv(rate, &[("modulation", "fm")]);
    run(&mut rx, rate, seconds, 100_000, |b| {
        g.generate(b);
    })
}

#[test]
fn v1_v2_v5_fm_video() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let r = fm_video(FpvGenParams::default(), 2.5);
            let d = r.last();
            assert_eq!(d.state, 2);
            assert!(d.data_valid);
            assert!(r.first(|d| d.state == 2).unwrap() <= 1.5);
            assert_eq!(d.lines, 625);
            assert_eq!(d.colour_system, "PAL");
            assert!(d.colour);
            assert_eq!(d.fm_polarity, "sync tip is the lowest frequency");
            assert_eq!(d.detector, "FM discriminator");
            assert_eq!(d.sound_system, "not decoded");
            assert!(d.fields_bad <= 4);
            let p = r.picture.as_ref().unwrap();
            assert_eq!(p.picture.layout.as_ref().unwrap().width, 768);
            assert!(bars_error(p) < 0.35, "bars {}", bars_error(p));
        });
        s.spawn(|| {
            let r = fm_video(
                FpvGenParams {
                    sync_high: true,
                    ..Default::default()
                },
                4.5,
            );
            let d = r.last();
            assert_eq!(d.state, 2);
            assert_eq!(d.fm_polarity, "sync tip is the highest frequency");
            assert!(r.first(|d| d.state == 2).unwrap() <= 3.5);
        });
        s.spawn(|| {
            let r = fm_video_imp(
                FpvGenParams {
                    deviation_hz: 6e6,
                    ..Default::default()
                },
                Imp {
                    cnr_db: 22.0,
                    offset_hz: -2.5e6,
                    ..Default::default()
                },
                2.5,
            );
            assert_eq!(r.last().state, 2);
            assert!(r.first(|d| d.state == 2).unwrap() <= 2.0);
            assert!(bars_error(r.picture.as_ref().unwrap()) < 0.45);
        });
        s.spawn(|| {
            let r = fm_video(
                FpvGenParams {
                    ac_s: 2e-3,
                    ..Default::default()
                },
                2.75,
            );
            assert_eq!(r.last().state, 2);
            assert!(r.first(|d| d.state == 2).unwrap() <= 2.5);
            assert!(bars_error(r.picture.as_ref().unwrap()) < 0.45);
        });
        s.spawn(|| {
            let r = fm_video(
                FpvGenParams {
                    none: true,
                    ..Default::default()
                },
                2.0,
            );
            assert!(r.reports.iter().all(|d| d.state < 2 && !d.data_valid));
        });
    });
}

/// The NBTV picture against its pattern, cell by cell (columns on bar edges skipped): the worst of the bars' and the ramp's mean errors.
fn nbtv_error(p: &AtvPicture, lines: usize, samples: usize) -> f64 {
    let l = p.picture.layout.as_ref().unwrap();
    let (w, h) = (l.width as usize, l.height as usize);
    let scale = w / lines;
    assert_eq!(h / scale, samples);
    let (mut eb, mut nb, mut er, mut nr) = (0.0, 0.0, 0.0, 0.0);
    let mut signed = 0.0;
    for li in 0..lines {
        let edge = (li * 8) % lines < 8 || ((li + 1) * 8) % lines < 8;
        for c in 0..samples {
            let want = nbtv_cell(0, lines, samples, li, c) as f64;
            let (x, y) = (li * scale + scale / 2, c * scale + scale / 2);
            let got = p.picture.data[(y * w + x) * 4 + 1] as f64 / 255.0;
            if c < samples / 2 {
                if !edge && c > 0 {
                    eb += (got - want).abs();
                    nb += 1.0;
                }
            } else if c + 1 < samples {
                signed += got - want;
                er += (got - want).abs();
                nr += 1.0;
            }
        }
    }
    eprintln!(
        "nbtv {lines} lines: bars {:.4} ramp {:.4} ramp signed {:.4}",
        eb / nb,
        er / nr,
        signed / nr
    );
    (eb / nb).max(er / nr)
}

fn nbtv_case(lines: u32, fm: bool, offset: f64, seconds: f64) {
    let rate = 10e6;
    let f = NbtvFormat {
        lines,
        rate: 12.5,
        vertical: true,
    };
    let imp = Imp {
        cnr_db: 35.0,
        offset_hz: offset,
        ..Default::default()
    };
    let mut g = impaired(Box::new(NbtvGenerator::new(f, rate, fm, 5e3, 0)), imp);
    let lines_s = lines.to_string();
    let modulation = if fm { "nbtv_fm" } else { "nbtv_am" };
    let mut rx = atv(
        rate,
        &[("modulation", modulation), ("nbtv_lines", &lines_s)],
    );
    let r = run(&mut rx, rate, seconds, 100_000, |b| {
        g.generate(b);
    });
    let d = r.last();
    assert_eq!(d.state, 2, "{lines} {fm}");
    assert!(d.nbtv_line_locked && d.nbtv_picture_locked);
    let want_hz = lines as f64 * 12.5;
    assert!(
        (d.line_hz / want_hz - 1.0).abs() < 0.002,
        "line rate {}",
        d.line_hz
    );
    assert!(
        r.pictures as f64 >= 0.6 * seconds * 12.5 - 1.0,
        "pictures {}",
        r.pictures
    );
    assert_eq!(d.sound_system, "none (the audio is the picture)");
    assert_eq!(d.colour_system, "mono");
    let e = nbtv_error(r.picture.as_ref().unwrap(), lines as usize, f.samples());
    assert!(e < 0.06, "{lines} {fm}: picture error {e}");
}

#[test]
fn y6_y7_y8_nbtv() {
    std::thread::scope(|s| {
        s.spawn(|| nbtv_case(32, false, 0.0, 3.0));
        s.spawn(|| nbtv_case(32, true, 40e3, 3.0));
        s.spawn(|| nbtv_case(60, false, -50e3, 3.0));
    });
}

/// One row of the FPV channel table.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    present: bool,
    over_db: f64,
    heard: Option<f64>,
}

/// What the `fpv` results say.
#[derive(Clone, Debug, Default)]
struct Fpv {
    state: u8,
    data_valid: bool,
    lines: u32,
    colour_system: String,
    tuned: String,
    over_db: f64,
    sound_present: bool,
    subcarrier_mhz: f64,
    s60_db: f64,
    s65_db: f64,
    table: Vec<Row>,
}

/// The `fpv` fields and channel table of `rx`.
fn fpv_read(rx: &StackGraph) -> Fpv {
    let res = Owner::results(rx);
    let f = res.fields();
    let state = f.at(&FpvFields::STATE).map_or(0, |s| s.index() as u8);
    let table = res
        .decl()
        .table_id(FPV_CHANNELS.key)
        .and_then(|id| res.table(id))
        .map(|t| {
            t.rows_newest_first()
                .map(|(k, row)| Row {
                    name: k.to_string(),
                    present: row.at(&FpvChannelRow::PRESENT).unwrap_or(false),
                    over_db: row.at(&FpvChannelRow::OVER_NOISE_DB).unwrap_or(0.0),
                    heard: row.at(&FpvChannelRow::HEARD),
                })
                .collect()
        })
        .unwrap_or_default();
    Fpv {
        state,
        data_valid: state == 2,
        lines: f.at(&FpvFields::LINES).unwrap_or(0) as u32,
        colour_system: f
            .at(&FpvFields::COLOUR_SYSTEM)
            .map(|e| e.label().to_string())
            .unwrap_or_default(),
        tuned: f
            .at(&FpvFields::CHANNEL)
            .and_then(|n| n.split(" / ").next())
            .unwrap_or("")
            .to_string(),
        over_db: f.at(&FpvFields::OVER_NOISE_DB).unwrap_or(0.0),
        sound_present: f.at(&FpvFields::SOUND_PRESENT).unwrap_or(false),
        subcarrier_mhz: f.at(&FpvFields::SOUND_SUBCARRIER).unwrap_or(0.0) / 1e6,
        s60_db: f.at(&FpvFields::SUBCARRIER_60_DB).unwrap_or(0.0),
        s65_db: f.at(&FpvFields::SUBCARRIER_65_DB).unwrap_or(0.0),
        table,
    }
}

/// An `fpv` receiver with the FPV channels of the data folder.
fn fpv_receiver() -> StackGraph {
    let text = fpv_channels_text();
    let v = values_of(&FpvSetting::schema(), &[("channels", text.as_str())]);
    sdrkit_stack_analogtv::fpv::receiver(&StackConfig::new(v)).unwrap()
}

fn fpv_channels() -> Vec<channels::FpvChannel> {
    channels::fpv_channels(
        &channels::parse(&fpv_channels_text(), channels::FPV_CHANNELS_CAP).unwrap(),
    )
}

struct FpvRun {
    d: Fpv,
    audio: Vec<f32>,
    pictures: u64,
    picture: Option<Picture>,
    rx: StackGraph,
}

/// The FPV receiver on a transmitter at 30 dB carrier to noise in 17 MHz.
fn fpv_run(p: FpvGenParams, centre_mhz: f64, seconds: f64) -> FpvRun {
    let rate = p.rate_hz;
    let mut g = impaired(Box::new(FpvGenerator::new(p)), Imp::cnr(30.0));
    let mut rx = fpv_receiver();
    rx.configure(&InputInfo::new(rate, centre_mhz * 1e6, centre_mhz * 1e6))
        .unwrap();
    let (audio, pictures, picture) = feed(g.as_mut(), &mut rx, seconds);
    FpvRun {
        d: fpv_read(&rx),
        audio,
        pictures,
        picture,
        rx,
    }
}

/// Feeds `seconds` of `g` to `rx`: the audio, the pictures seen and the last one.
fn feed(
    g: &mut dyn Generator,
    rx: &mut StackGraph,
    seconds: f64,
) -> (Vec<f32>, u64, Option<Picture>) {
    let rate = g.rate_hz();
    let mut buf = vec![Complex32::default(); 100_000];
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut count = 0;
    let mut last = None;
    for _ in 0..(seconds * rate / 1e5) as usize {
        g.generate(&mut buf);
        out.clear();
        rx.feed(&buf, &mut out);
        audio.extend_from_slice(out.audio());
        for p in pictures(&out) {
            count += 1;
            last = Some(p);
        }
    }
    (audio, count, last)
}

#[test]
fn p1_p2_p3_p4_fpv() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let r = fpv_run(FpvGenParams::default(), 5806.0, 2.0);
            let d = &r.d;
            assert_eq!(d.state, 2);
            assert!(d.data_valid);
            assert_eq!(d.lines, 625);
            assert_eq!(d.colour_system, "PAL");
            assert_eq!(d.tuned, "R5");
            assert!(d.over_db > 8.0, "over {}", d.over_db);
            let p = r.picture.as_ref().unwrap();
            let l = p.layout.as_ref().unwrap();
            assert_eq!((l.width, l.height), (768, 576));
            assert!(d.sound_present);
            assert!((d.subcarrier_mhz - 6.5).abs() < 0.01);
            assert!(
                d.s65_db > 15.0 && d.s60_db < 6.0,
                "{} {}",
                d.s65_db,
                d.s60_db
            );
            let left = &r.audio;
            assert!(left.len() > 48_000);
            let tail = &left[24_000..];
            let total = sdrkit_dev_test::measure::rms(tail).powi(2);
            let t = tone(tail, 1000.0).powi(2) / 2.0;
            assert!(t > 0.5 * total, "tone power {t} of {total}");
            assert!(r.pictures > 10);
        });
        s.spawn(|| {
            let r = fpv_run(
                FpvGenParams {
                    ntsc: true,
                    sound_hz: 6.0e6,
                    ..Default::default()
                },
                5806.0,
                2.0,
            );
            assert_eq!(r.d.state, 2);
            assert_eq!(r.d.lines, 525);
            assert!((r.d.subcarrier_mhz - 6.0).abs() < 0.01);
            assert!(r.d.sound_present);
        });
        s.spawn(|| {
            // A transmitter on R1 (5658 MHz) and the radio 3 MHz below it (no other channel is nearer).
            let r = fpv_run(
                FpvGenParams {
                    centre_hz: 3e6,
                    ..Default::default()
                },
                5655.0,
                2.0,
            );
            assert_eq!(r.d.state, 2);
            assert_eq!(r.d.lines, 625);
            assert_eq!(r.d.tuned, "R1");
        });
        s.spawn(|| {
            let r = fpv_run(
                FpvGenParams {
                    none: true,
                    ..Default::default()
                },
                5806.0,
                1.0,
            );
            assert_eq!(r.d.state, 0);
            assert!(!r.d.data_valid);
            assert!(!r.d.sound_present);
            let r5 = r.d.table.iter().find(|c| c.name == "R5").unwrap();
            assert!(!r5.present);
            // A transmitter afterwards stands out against the floor learnt.
            let mut rx = r.rx;
            // The same level as before (the chain puts the carrier at 0 dBFS), without noise.
            let mut g = impaired(
                Box::new(FpvGenerator::new(FpvGenParams::default())),
                Imp::default(),
            );
            feed(g.as_mut(), &mut rx, 0.5);
            let d = fpv_read(&rx);
            assert!(d.over_db > 15.0, "over {}", d.over_db);
        });
    });
}

/// The level of the tuned channel is the power of the input in dBFS: a carrier at -20 dBFS reads -20 dBFS.
#[test]
fn p10_level_is_the_input_level_in_dbfs() {
    let p = FpvGenParams {
        amplitude: 0.1,
        deviation_hz: 4e6,
        ..Default::default()
    };
    let rate = p.rate_hz;
    let mut g = FpvGenerator::new(p);
    let mut rx = fpv_receiver();
    rx.configure(&InputInfo::new(rate, 5806e6, 5806e6)).unwrap();
    feed(&mut g, &mut rx, 1.0);
    let res = Owner::results(&rx);
    let level = res.fields().at(&FpvFields::LEVEL_DBFS).unwrap();
    assert!((level + 20.0).abs() <= 1.0, "level {level} dBFS");
}

#[test]
fn p5_scan_builds_the_table() {
    // Transmitters on R5 (5806) and A8 (5725); the receiver visits the scan centres of the 5.8 GHz plan.
    let rate = 20e6;
    let mut rx = fpv_receiver();
    let centres = channels::fpv_scan_centres(&fpv_channels(), false);
    let mut gens = [
        FpvGenerator::new(FpvGenParams::default()),
        FpvGenerator::new(FpvGenParams::default()),
    ];
    let mut noise = noise(1e-3, 9);
    let mut buf = vec![Complex32::default(); 100_000];
    let mut tmp = vec![Complex32::default(); 100_000];
    let mut out = Output::default();
    let mut configured = false;
    for &c in &centres {
        let info = InputInfo::new(rate, c * 1e6, c * 1e6);
        if configured {
            rx.retuned(&info);
        } else {
            rx.configure(&info).unwrap();
            configured = true;
        }
        let mut shifts: Vec<_> = [5806.0, 5725.0]
            .iter()
            .map(|f| {
                sdrkit_algo_sim::offset::FrequencyOffset::new(rate, (f - c) * 1e6, 0.0).unwrap()
            })
            .collect();
        // Each centre is heard for 0.12 s (a report is judged every 0.1 s).
        for _ in 0..24 {
            buf.fill(Complex32::default());
            for (k, f) in [5806.0f64, 5725.0].iter().enumerate() {
                if (f - c).abs() < 9.0 {
                    gens[k].generate(&mut tmp);
                    shifts[k].process(&mut tmp);
                    sdrkit_algo_sim::front::add_into(&mut buf, &tmp);
                }
            }
            noise.process(&mut buf);
            out.clear();
            rx.feed(&buf, &mut out);
        }
    }
    let d = fpv_read(&rx);
    let names: Vec<&str> = d.table.iter().map(|r| r.name.as_str()).collect();
    let row = |n: &str| {
        d.table
            .iter()
            .find(|r| r.name == n)
            .unwrap_or_else(|| panic!("{n} not in {names:?}"))
            .clone()
    };
    assert!(
        row("R5").present && row("R5").over_db > 15.0,
        "R5 {:?}",
        row("R5")
    );
    assert!(
        row("A8").present && row("A8").over_db > 15.0,
        "A8 {:?}",
        row("A8")
    );
    assert!(row("R1").heard.is_some() && !row("R1").present);
    assert_eq!(row("G1").heard, None);
    let present = d.table.iter().filter(|r| r.present).count();
    assert!((2..=8).contains(&present), "present {present}");
    // A change of frequency by the host keeps the scan list: the meter still knows when it heard each channel.
    sdrkit_core::Receiver::forget(&mut rx);
    for _ in 0..24 {
        buf.fill(Complex32::default());
        noise.process(&mut buf);
        out.clear();
        rx.feed(&buf, &mut out);
    }
    let after = fpv_read(&rx);
    for name in ["R5", "A8", "R1"] {
        assert!(
            after
                .table
                .iter()
                .any(|r| r.name == name && r.heard.is_some()),
            "{name}"
        );
    }
}

#[test]
fn p6_p7_rates_and_plans() {
    let mut rx = fpv_receiver();
    assert!(rx.configure(&InputInfo::new(10e6, 5806e6, 5806e6)).is_err());
    let ch = fpv_channels();
    assert_eq!(ch.len(), 56);
    let mut names: Vec<String> = ch.iter().map(|c| c.name.clone()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 56);
    let f = |n: &str| ch.iter().find(|c| c.name == n).unwrap().mhz;
    for (n, mhz) in [
        ("R1", 5658.0),
        ("R5", 5806.0),
        ("R8", 5917.0),
        ("A1", 5865.0),
        ("A8", 5725.0),
        ("B1", 5733.0),
        ("B8", 5866.0),
        ("E1", 5705.0),
        ("E5", 5885.0),
        ("E8", 5945.0),
        ("F1", 5740.0),
        ("F8", 5880.0),
        ("L1", 5362.0),
        ("L8", 5621.0),
        ("G1", 1080.0),
        ("G8", 1360.0),
    ] {
        assert_eq!(f(n), mhz, "{n}");
    }
    for k in 1..8 {
        assert!((f(&format!("R{}", k + 1)) - f(&format!("R{k}")) - 37.0).abs() <= 1.0);
        assert_eq!(f(&format!("F{}", k + 1)) - f(&format!("F{k}")), 20.0);
    }
    assert_eq!(channels::fpv_nearest(&ch, 5806.4, 4.0).unwrap().name, "R5");
    assert!(channels::fpv_nearest(&ch, 3000.0, 4.0).is_none());
    assert_eq!(channels::fpv_nearest(&ch, 1199.0, 4.0).unwrap().name, "G4");
    let at = channels::fpv_names_at(&ch, 5806.0);
    assert!(at.contains("R5") && at.contains("A4"), "{at}");
    // Every 5.8 GHz channel is within 4.5 MHz of a scan centre.
    let centres = channels::fpv_scan_centres(&ch, false);
    for c in ch.iter().filter(|c| c.band != "G") {
        assert!(
            centres.iter().any(|&x| (x - c.mhz).abs() <= 4.5),
            "{}",
            c.name
        );
    }
}

#[test]
fn p8_channel_and_scan_commands_ask_the_host() {
    let mut rx = fpv_receiver();
    rx.configure(&InputInfo::new(20e6, 5806e6, 5806e6)).unwrap();
    invoke(&mut rx, "channel.tune", "R5").unwrap();
    assert!(matches!(
        invoke(&mut rx, "channel.tune", "Z9"),
        Err(sdrkit_core::CommandError::NoSuchRow { .. })
    ));
    let plan = sdrkit_stack_analogtv::fpv::ScanPlan::G58;
    sdrkit_stack_analogtv::typed::FPV_SCAN
        .call(&mut rx, &sdrkit_stack_analogtv::typed::FpvScanArgs { plan })
        .unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 400_000], &mut out);
    assert!(out.requests.iter().any(|q| matches!(
        q,
        sdrkit_core::HostRequest::Retune { tuned_hz, .. } if *tuned_hz == 5806e6
    )));
    let centres = channels::fpv_scan_centres(&fpv_channels(), false).len();
    assert!(
        out.requests.iter().any(|q| matches!(
            q,
            sdrkit_core::HostRequest::Hop { channels, .. } if channels.len() == centres
        )),
        "{:?}",
        out.requests
    );
}

/// The receiver runs with two bands of the FPV channels of the data folder, set apart: with the R band `channel.tune` R5
/// asks for 5806 MHz and the table lists 8 channels; with the G band G4 asks for 1200 MHz and the 1.2 GHz scan visits the
/// band. A label of the other band is refused.
#[test]
fn p9_the_receiver_follows_the_channels_set() {
    let all = fpv_channels_text();
    for (band, label, hz, other) in [("R", "R5", 5806e6, "G4"), ("G", "G4", 1200e6, "R5")] {
        let text: String = all
            .split(';')
            .filter(|e| e.starts_with(band) && e[1..2].bytes().all(|b| b.is_ascii_digit()))
            .collect::<Vec<_>>()
            .join(";");
        let v = values_of(&FpvSetting::schema(), &[("channels", text.as_str())]);
        let mut rx = sdrkit_stack_analogtv::fpv::receiver(&StackConfig::new(v)).unwrap();
        rx.configure(&InputInfo::new(20e6, hz, hz)).unwrap();
        invoke(&mut rx, "channel.tune", label).unwrap();
        assert!(matches!(
            invoke(&mut rx, "channel.tune", other),
            Err(sdrkit_core::CommandError::NoSuchRow { .. })
        ));
        let plan = if band == "G" {
            sdrkit_stack_analogtv::fpv::ScanPlan::G12
        } else {
            sdrkit_stack_analogtv::fpv::ScanPlan::G58
        };
        sdrkit_stack_analogtv::typed::FPV_SCAN
            .call(&mut rx, &sdrkit_stack_analogtv::typed::FpvScanArgs { plan })
            .unwrap();
        let mut out = Output::default();
        let mut requests = Vec::new();
        let silence = vec![Complex32::default(); 100_000];
        for _ in 0..30 {
            out.clear();
            rx.feed(&silence, &mut out);
            requests.append(&mut out.requests);
        }
        out.requests = requests;
        assert_eq!(fpv_read(&rx).table.len(), 8, "{band}");
        assert!(
            out.requests.iter().any(|q| matches!(
                q,
                sdrkit_core::HostRequest::Retune { tuned_hz, .. } if *tuned_hz == hz
            )),
            "{band}: {:?}",
            out.requests
        );
        assert!(
            out.requests.iter().any(|q| matches!(
                q,
                sdrkit_core::HostRequest::Hop { channels, .. } if !channels.is_empty()
            )),
            "{band}: {:?}",
            out.requests
        );
    }
}
