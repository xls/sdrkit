//! Airband: the scene through the receiver (timing of every keyed burst), the recording, noise, heterodyne, short bursts, bad
//! samples, out of band, per-channel squelch and scan.

use crate::common::{self, num, rows, text};
use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_core::{Complex32, Generator, LockState, Output, Receiver};
use sdrkit_stack_avdata::airband::generator::{keyed, layout, Audio, Keying, TxChannel, AMPLITUDE};
use sdrkit_stack_avdata::airband::names::{list_to_text, Channel, STEP_833};
use sdrkit_stack_avdata::airband::{self, AirbandGenerator, TUNE_OFFSET_HZ};
use sdrkit_stack_avdata::typed::{AIRBAND_ACTIVITY, AIRBAND_CHANNELS, AIRBAND_SEEN};

const DIAL: f64 = 118.7e6;

/// The scene of a test: the generator's channels and the impairments the test adds.
#[derive(Clone, Debug)]
struct GenParams {
    rate_hz: f64,
    channels: Vec<TxChannel>,
    /// C/N of a 0 dB channel in 6.8 kHz, or `None` for no noise.
    snr_db: Option<f64>,
    /// The radio's tuning error, Hz (moves every channel).
    tune_error_hz: f64,
    seed: u64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 1e6,
            channels: layout(0),
            snr_db: Some(25.0),
            tune_error_hz: 0.0,
            seed: 1,
        }
    }
}

impl GenParams {
    fn generator(&self) -> AirbandGenerator {
        AirbandGenerator::new(airband::GenParams {
            rate_hz: self.rate_hz,
            channels: self.channels.clone(),
            seed: self.seed,
            ..Default::default()
        })
    }
}

fn rx(rate: f64, centre: f64, list: &[Channel], extra: &[(&str, &str)]) -> Box<dyn Receiver> {
    let lt = list_to_text(list);
    let mut st: Vec<(&str, &str)> = Vec::new();
    if !list.is_empty() {
        st.push(("channels", &lt));
    }
    st.extend_from_slice(extra);
    common::receiver(&airband::DESCRIPTOR, rate, centre, DIAL, &st)
}

/// A transmission in the log.
#[derive(Debug)]
struct Act {
    name: String,
    freq_hz: f64,
    start_s: f64,
    dur_s: f64,
    heterodyne: bool,
}

/// A channel of the bank.
#[derive(Debug)]
struct Ch {
    name: String,
    freq_hz: f64,
    in_band: bool,
    open: bool,
}

#[derive(Debug, Default)]
struct Bank {
    channels: Vec<Ch>,
    tune_error_hz: f64,
    known: bool,
}

#[derive(Debug, Default)]
struct Scan {
    channel: i64,
    playing: Vec<i64>,
}

#[derive(Debug)]
struct SeenRow {
    name: String,
}

#[derive(Debug, Default)]
struct SeenList {
    entries: Vec<SeenRow>,
}

/// What the tests read of the results.
#[derive(Debug, Default)]
struct AirbandDetail {
    state: u8,
    log: Vec<Act>,
    bank: Bank,
    scan: Scan,
    seen: SeenList,
}

fn detail(r: &dyn Receiver) -> AirbandDetail {
    let res = r.results();
    let tune = res
        .decl()
        .field_id("tuning_error")
        .and_then(|id| res.fields().value(id))
        .and_then(|v| match v {
            sdrkit_core::Value::Hz(x) => Some(*x),
            _ => None,
        });
    let chans = rows(res, &AIRBAND_CHANNELS);
    AirbandDetail {
        state: match res.status().lock_state {
            LockState::Locked => 2,
            LockState::Tracking => 1,
            _ => 0,
        },
        log: rows(res, &AIRBAND_ACTIVITY)
            .iter()
            .map(|a| Act {
                name: text(a, "name").to_owned(),
                freq_hz: num(a, "frequency"),
                start_s: num(a, "start_s"),
                dur_s: num(a, "length"),
                heterodyne: common::flag(a, "heterodyne"),
            })
            .collect(),
        bank: Bank {
            channels: chans
                .iter()
                .map(|c| Ch {
                    name: text(c, "name").to_owned(),
                    freq_hz: num(c, "frequency"),
                    in_band: common::flag(c, "in_band"),
                    open: common::flag(c, "open"),
                })
                .collect(),
            tune_error_hz: tune.unwrap_or(res.status().offset_hz.unwrap_or(0.0)),
            known: tune.is_some(),
        },
        scan: Scan {
            channel: res
                .decl()
                .field_id("scan_channel")
                .and_then(|id| res.fields().value(id))
                .map_or(-1, |v| match v {
                    sdrkit_core::Value::Hz(f) => *f as i64,
                    _ => -1,
                }),
            playing: chans
                .iter()
                .filter(|c| common::flag(c, "playing"))
                .map(|c| num(c, "frequency") as i64)
                .collect(),
        },
        seen: SeenList {
            entries: rows(res, &AIRBAND_SEEN)
                .iter()
                .map(|s| SeenRow {
                    name: text(s, "name").to_owned(),
                })
                .collect(),
        },
    }
}

fn chans_for(tx: &[TxChannel]) -> Vec<Channel> {
    tx.iter()
        .map(|c| {
            let f = DIAL + c.offset_hz;
            let is833 = ((f / 25e3) - (f / 25e3).round()).abs() > 1e-6;
            let f = if is833 {
                118e6 + ((f - 118e6) / STEP_833).round() * STEP_833
            } else {
                f
            };
            Channel {
                freq_hz: f,
                is833,
                label: format!("{:.0}", c.offset_hz),
                ..Default::default()
            }
        })
        .collect()
}

fn run_scene(
    p: &GenParams,
    secs: f64,
    list: &[Channel],
    extra: &[(&str, &str)],
    hook: &mut dyn FnMut(&mut [Complex32], f64),
) -> (AirbandDetail, Vec<f32>) {
    let mut g = p.generator();
    let centre = DIAL + TUNE_OFFSET_HZ;
    let mut r = rx(p.rate_hz, centre, list, extra);
    let mut out = Output::default();
    let mut buf = vec![Complex32::default(); 8192];
    let total = (secs * p.rate_hz) as usize;
    let mut done = 0;
    let mut audio = Vec::new();
    let mut offset = FrequencyOffset::new(p.rate_hz, p.tune_error_hz, 0.0).unwrap();
    let mut noise = p.snr_db.map(|s| {
        let level = NoiseLevel::SnrDb {
            snr_db: s,
            bandwidth_hz: 6.8e3,
        };
        Awgn::complex(level, AMPLITUDE * AMPLITUDE, p.rate_hz, p.seed ^ 0xA1B).unwrap()
    });
    while done < total {
        let n = buf.len().min(total - done);
        g.generate(&mut buf[..n]);
        offset.process(&mut buf[..n]);
        if let Some(a) = noise.as_mut() {
            a.process(&mut buf[..n]);
        }
        hook(&mut buf[..n], done as f64 / p.rate_hz);
        out.clear();
        r.feed(&buf[..n], &mut out);
        audio.extend(
            out.audio()
                .iter()
                .step_by(out.audio_format().map_or(1, |f| usize::from(f.1.max(1)))),
        );
        done += n;
    }
    (detail(r.as_ref()), audio)
}

/// Every keyed burst of channel `ci` inside the window is in the log once with its start within `slack` and its length within twice
/// that, and nothing else is.
fn timing(
    d: &AirbandDetail,
    tx: &TxChannel,
    freq: f64,
    from: f64,
    until: f64,
    slack: f64,
) -> Result<usize, String> {
    let mut want: Vec<(f64, f64)> = Vec::new();
    let mut t = 0.0;
    let step = 0.001;
    let mut on: Option<f64> = None;
    while t < until + 5.0 {
        let k = keyed(&tx.keying, t) > 0.0;
        match (k, on) {
            (true, None) => on = Some(t),
            (false, Some(s)) => {
                want.push((s, t - s));
                on = None;
            }
            _ => {}
        }
        t += step;
    }
    let want: Vec<(f64, f64)> = want
        .into_iter()
        .filter(|(s, l)| *s >= from && s + l <= until - 0.6)
        .collect();
    let got: Vec<(f64, f64)> = d
        .log
        .iter()
        .filter(|a| (a.freq_hz - freq).abs() < 10.0)
        .map(|a| (a.start_s, a.dur_s))
        .collect();
    for w in &want {
        let n = got
            .iter()
            .filter(|g| (g.0 - w.0).abs() <= slack && (g.1 - w.1).abs() <= 2.0 * slack)
            .count();
        if n != 1 {
            return Err(format!("{freq}: burst {w:?} matched {n} times in {got:?}"));
        }
    }
    let extra = got
        .iter()
        .filter(|g| g.0 >= from && g.0 + g.1 <= until - 0.6)
        .filter(|g| {
            !want
                .iter()
                .any(|w| (g.0 - w.0).abs() <= slack && (g.1 - w.1).abs() <= 2.0 * slack)
        })
        .count();
    if extra > 0 {
        return Err(format!(
            "{freq}: {extra} extra entries in {got:?}, wanted {want:?}"
        ));
    }
    Ok(want.len())
}

#[test]
fn b2_test_layout() {
    let p = GenParams {
        snr_db: Some(25.0),
        ..Default::default()
    };
    let list = chans_for(&p.channels);
    let (d, _) = run_scene(&p, 20.0, &list, &[], &mut |_, _| {});
    eprintln!(
        "tuning {:.0} known {} log {}",
        d.bank.tune_error_hz,
        d.bank.known,
        d.log.len()
    );
    for (tx, ch) in p.channels.iter().zip(&list) {
        if tx.keying == Keying::Continuous {
            let c = d
                .bank
                .channels
                .iter()
                .find(|c| c.freq_hz == ch.freq_hz)
                .unwrap();
            assert!(c.open, "ATIS open");
            continue;
        }
        timing(&d, tx, ch.freq_hz, 0.3, 20.0, 0.05).unwrap();
    }
    assert_eq!(d.bank.channels.len(), 5);
    assert!(
        d.bank.tune_error_hz.abs() < 150.0,
        "{}",
        d.bank.tune_error_hz
    );
}

#[test]
fn the_recording_decodes_as_expected() {
    let rec =
        sdrkit_dev_test::read_recording("avdata/airband_250k").expect("the airband recording");
    let list = vec![
        Channel {
            freq_hz: 118.7e6,
            label: "A".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: 118.708_333_333e6,
            is833: true,
            label: "B".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: 118.65e6,
            label: "C".into(),
            ..Default::default()
        },
    ];
    let mut r = rx(rec.rate_hz, rec.centre_hz, &list, &[]);
    common::run(r.as_mut(), &rec.samples, 65_536);
    let d = detail(r.as_ref());
    let mut got: Vec<(String, f64, f64)> = d
        .log
        .iter()
        .rev()
        .map(|a| (a.name.clone(), a.start_s, a.dur_s))
        .collect();
    got.sort_by(|a, b| a.1.total_cmp(&b.1));
    eprintln!("{got:?} tuning {}", d.bank.tune_error_hz);
    let want = [
        ("118.700", 0.5, 1.5),
        ("118.710", 1.2, 1.0),
        ("118.700", 3.5, 1.5),
        ("118.710", 4.0, 0.15),
    ];
    assert_eq!(got.len(), 4);
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.0, w.0);
        assert!((g.1 - w.1).abs() <= 0.05, "{g:?} {w:?}");
        assert!((g.2 - w.2).abs() <= 0.10, "{g:?} {w:?}");
    }
    let c = d
        .bank
        .channels
        .iter()
        .find(|c| c.name == "118.650")
        .unwrap();
    assert!(c.open);
    assert!(d.bank.tune_error_hz.abs() <= 300.0);
    assert!(!d.log.iter().any(|a| a.heterodyne));
}

#[test]
fn b4_noise_only() {
    let p = GenParams {
        channels: vec![TxChannel {
            offset_hz: 0.0,
            level_db: -200.0,
            tx_offset_hz: 0.0,
            depth: 0.5,
            audio: Audio::Carrier,
            keying: Keying::Continuous,
        }],
        ..Default::default()
    };
    let list = chans_for(&p.channels);
    let (d, _) = run_scene(&p, 30.0, &list, &[], &mut |_, _| {});
    assert!(d.log.is_empty());
    assert_eq!(d.state, 1);
}

#[test]
fn b10_two_stations_flag_a_heterodyne() {
    let ch = |off: f64, lvl: f64| TxChannel {
        offset_hz: 0.0,
        level_db: lvl,
        tx_offset_hz: off,
        depth: 0.0,
        audio: Audio::Carrier,
        keying: Keying::Periodic(3.0, 1.5, 0.5),
    };
    let p = GenParams {
        channels: vec![ch(0.0, 0.0), ch(400.0, -6.0)],
        snr_db: Some(30.0),
        ..Default::default()
    };
    let list = vec![Channel {
        freq_hz: DIAL,
        ..Default::default()
    }];
    let (d, _) = run_scene(&p, 10.0, &list, &[], &mut |_, _| {});
    assert!(d.log.len() >= 2);
    assert!(d.log.iter().any(|a| a.heterodyne));
}

#[test]
fn b11_short_bursts() {
    let tx = TxChannel {
        offset_hz: 0.0,
        level_db: 0.0,
        tx_offset_hz: 0.0,
        depth: 0.8,
        audio: Audio::Tone,
        keying: Keying::Bursts(vec![(-0.5, 1.0), (2.0, 0.15), (3.0, 0.08), (4.0, 0.15)]),
    };
    let p = GenParams {
        channels: vec![tx.clone()],
        snr_db: Some(25.0),
        ..Default::default()
    };
    let list = vec![Channel {
        freq_hz: DIAL,
        ..Default::default()
    }];
    let (d, _) = run_scene(&p, 6.0, &list, &[], &mut |_, _| {});
    assert_eq!(d.log.len(), 4, "{:?}", d.log);
    timing(&d, &tx, DIAL, 1.0, 6.0, 0.05).unwrap();
}

#[test]
fn b12_bad_samples() {
    let p = GenParams {
        snr_db: Some(25.0),
        ..Default::default()
    };
    let list = chans_for(&p.channels);
    let (d, audio) = run_scene(&p, 12.0, &list, &[], &mut |b, t0| {
        if (3.0..3.01).contains(&t0) || (t0 <= 3.0 && t0 + b.len() as f64 / 1e6 > 3.0) {
            for (i, v) in b.iter_mut().take(500).enumerate() {
                *v = if i % 2 == 0 {
                    Complex32::new(f32::NAN, 0.0)
                } else {
                    Complex32::new(f32::INFINITY, 1.0)
                };
            }
        }
    });
    assert!(d.log.len() >= 6);
    assert!(d.bank.tune_error_hz.is_finite());
    assert!(audio.iter().all(|v| v.is_finite()));
}

#[test]
fn b13_a_channel_out_of_band() {
    let list = vec![
        Channel {
            freq_hz: DIAL,
            ..Default::default()
        },
        Channel {
            freq_hz: 125e6,
            ..Default::default()
        },
    ];
    let p = GenParams::default();
    let (d, _) = run_scene(&p, 1.0, &list, &[], &mut |_, _| {});
    assert!(d
        .bank
        .channels
        .iter()
        .find(|c| c.freq_hz == 125e6)
        .is_some_and(|c| !c.in_band));
    assert!(d
        .bank
        .channels
        .iter()
        .find(|c| c.freq_hz == DIAL)
        .is_some_and(|c| c.in_band));
}

#[test]
fn b15_per_channel_squelch() {
    let mut p = GenParams {
        snr_db: Some(20.0),
        ..Default::default()
    };
    p.channels = layout(0);
    let mut list = chans_for(&p.channels);
    list[0].squelch_db = Some(45.0);
    list[2].squelch_db = Some(-5.0);
    let (d, _) = run_scene(&p, 24.0, &list, &[], &mut |_, _| {});
    let n = |f: f64| {
        d.log
            .iter()
            .filter(|a| (a.freq_hz - f).abs() < 10.0)
            .count()
    };
    assert_eq!(n(list[0].freq_hz), 0);
    assert!(n(list[2].freq_hz) >= 3);
}

#[test]
fn b17_scan() {
    let a = TxChannel {
        offset_hz: 0.0,
        level_db: -200.0,
        tx_offset_hz: 0.0,
        depth: 0.8,
        audio: Audio::Tone,
        keying: Keying::Continuous,
    };
    let b = TxChannel {
        offset_hz: 50e3,
        level_db: 0.0,
        tx_offset_hz: 0.0,
        depth: 0.8,
        audio: Audio::Tone,
        keying: Keying::Continuous,
    };
    let p = GenParams {
        channels: vec![a, b],
        snr_db: Some(25.0),
        ..Default::default()
    };
    let list = vec![
        Channel {
            freq_hz: DIAL,
            ..Default::default()
        },
        Channel {
            freq_hz: DIAL + 50e3,
            ..Default::default()
        },
    ];
    let (d, _) = run_scene(&p, 3.0, &list, &[("scan", "1")], &mut |_, _| {});
    let cb = d
        .bank
        .channels
        .iter()
        .find(|c| c.freq_hz == DIAL + 50e3)
        .unwrap();
    assert!(cb.open);
    assert_eq!(d.scan.channel, (DIAL + 50e3) as i64);
    assert!(d.scan.playing.contains(&((DIAL + 50e3) as i64)));
}

use sdrkit_dev_test::measure;

/// The audio SNR of a 1 kHz tone (48 kHz audio) over the keyed parts of `tx` (each trimmed by 0.25 s at both ends): tone power over the
/// rest.
fn tone_snr(audio: &[f32], tx: &TxChannel, from: f64) -> f64 {
    let rate = 48e3;
    let (mut sig, mut res) = (0.0, 0.0);
    let mut t = from;
    let end = audio.len() as f64 / rate - 0.5;
    while t < end {
        // 0.25 s pieces fully inside a keyed stretch (the audio runs 60 ms late).
        if (0..=10).all(|k| keyed(&tx.keying, t - 0.05 + 0.04 * k as f64) >= 1.0) {
            let a = ((t + 0.06) * rate) as usize;
            let piece = &audio[a..a + (0.25 * rate) as usize];
            let amp = measure::tone_amplitude(piece, rate, 1000.0);
            let p = measure::rms(piece).powi(2);
            sig += amp * amp / 2.0;
            res += (p - amp * amp / 2.0).max(1e-12);
            t += 0.25;
        }
        t += 0.05;
    }
    assert!(sig > 0.0, "no keyed audio");
    10.0 * (sig / res).log10()
}

fn tone_channel(level: f64, depth: f64) -> TxChannel {
    TxChannel {
        offset_hz: 0.0,
        level_db: level,
        tx_offset_hz: 300.0,
        depth,
        audio: Audio::Tone,
        keying: Keying::Periodic(4.0, 1.5, 1.0),
    }
}

#[test]
fn b2_tone_channel_audio() {
    let p = GenParams {
        snr_db: Some(25.0),
        ..Default::default()
    };
    let list = chans_for(&p.channels);
    // Only the tone channel playing (solo), so its audio can be measured.
    let mut list2 = list.clone();
    list2[4].solo = true;
    let (_, audio) = run_scene(&p, 12.0, &list2, &[], &mut |_, _| {});
    let snr = tone_snr(&audio, &p.channels[4], 1.0);
    assert!(snr > 20.0, "{snr}");
}

#[test]
fn b3_a_weak_tone_channel() {
    for cn in [15.0, 8.0] {
        let tx = tone_channel(0.0, 0.8);
        let p = GenParams {
            channels: vec![tx.clone()],
            snr_db: Some(cn),
            ..Default::default()
        };
        let list = vec![Channel {
            freq_hz: DIAL,
            ..Default::default()
        }];
        let (d, audio) = run_scene(&p, 12.0, &list, &[], &mut |_, _| {});
        timing(&d, &tx, DIAL, 0.3, 12.0, 0.06).unwrap_or_else(|e| panic!("{cn} dB: {e}"));
        let snr = tone_snr(&audio, &tx, 1.0);
        assert!(snr > cn - 6.0, "{cn} dB: audio SNR {snr}");
    }
}

#[test]
fn b5_a_strong_unlisted_neighbour() {
    for err in [0.0, -1500.0, 2000.0] {
        let wanted = TxChannel {
            offset_hz: 0.0,
            level_db: 0.0,
            tx_offset_hz: 0.0,
            depth: 0.8,
            audio: Audio::Voice,
            keying: Keying::Periodic(5.0, 1.8, 1.0),
        };
        let strong = TxChannel {
            offset_hz: STEP_833,
            level_db: 30.0,
            tx_offset_hz: 0.0,
            depth: 0.8,
            audio: Audio::Voice,
            keying: Keying::Periodic(3.0, 1.2, 0.2),
        };
        let p = GenParams {
            channels: vec![wanted.clone(), strong],
            snr_db: Some(15.0),
            tune_error_hz: err,
            ..Default::default()
        };
        let list = vec![Channel {
            freq_hz: DIAL,
            is833: true,
            ..Default::default()
        }];
        let (d, _) = run_scene(&p, 16.0, &list, &[], &mut |_, _| {});
        // An 8.33 kHz channel 1.5 kHz off is outside its own carrier window until the tuning error is measured (about 2 s).
        timing(&d, &wanted, DIAL, 3.0, 16.0, 0.06)
            .unwrap_or_else(|e| panic!("radio {err} Hz off: {e}"));
        assert!(
            d.log.iter().all(|a| (a.freq_hz - DIAL).abs() < 10.0),
            "a false opening"
        );
    }
}

#[test]
fn b6_a_tuning_error() {
    for err in [7000.0, -7000.0, 3000.0] {
        let p = GenParams {
            snr_db: Some(25.0),
            tune_error_hz: err,
            ..Default::default()
        };
        let list = chans_for(&p.channels);
        let (d, _) = run_scene(&p, 24.0, &list, &[], &mut |_, _| {});
        assert!(
            d.bank.known && (d.bank.tune_error_hz - err).abs() < 300.0,
            "{err}: measured {} known {}",
            d.bank.tune_error_hz,
            d.bank.known
        );
        for (tx, ch) in p.channels.iter().zip(&list) {
            if tx.keying != Keying::Continuous {
                timing(&d, tx, ch.freq_hz, 10.0, 24.0, 0.05)
                    .unwrap_or_else(|e| panic!("{err}: {e}"));
            }
        }
    }
}

#[test]
fn b7_b8_clock_and_clipping() {
    let p = GenParams {
        snr_db: Some(25.0),
        ..Default::default()
    };
    let list = chans_for(&p.channels);
    let mut clock = sdrkit_algo_sim::clock::ClockError::new(40.0).unwrap();
    let mut buf = Vec::new();
    let (d, _) = run_scene(&p, 16.0, &list, &[], &mut |b, _| {
        clock.process(b, &mut buf);
        // Keep the piece's length: the clock error moves samples by far less than a piece.
        for (i, v) in b.iter_mut().enumerate() {
            *v = buf.get(i).copied().unwrap_or_default();
        }
        buf.drain(..buf.len().min(b.len()));
    });
    for (tx, ch) in p.channels.iter().zip(&list) {
        if tx.keying != Keying::Continuous {
            timing(&d, tx, ch.freq_hz, 0.3, 16.0, 0.06).unwrap_or_else(|e| panic!("clock: {e}"));
        }
    }
    let (d, _) = run_scene(&p, 16.0, &list, &[], &mut |b, _| {
        let rms = (b.iter().map(|v| v.norm_sqr()).sum::<f32>() / b.len() as f32).sqrt();
        common::quantise(b, 8, 5.0 * rms.max(1e-6));
    });
    for (tx, ch) in p.channels.iter().zip(&list) {
        if tx.keying != Keying::Continuous {
            timing(&d, tx, ch.freq_hz, 0.3, 16.0, 0.06).unwrap_or_else(|e| panic!("clipping: {e}"));
        }
    }
}

#[test]
fn b9_overmodulation() {
    let tx = tone_channel(0.0, 1.4);
    let p = GenParams {
        channels: vec![tx.clone()],
        snr_db: Some(25.0),
        ..Default::default()
    };
    let list = vec![Channel {
        freq_hz: DIAL,
        ..Default::default()
    }];
    let (d, audio) = run_scene(&p, 12.0, &list, &[], &mut |_, _| {});
    timing(&d, &tx, DIAL, 0.3, 12.0, 0.05).unwrap();
    let snr = tone_snr(&audio, &tx, 1.0);
    assert!(snr > 6.0, "{snr}");
}

#[test]
fn b14_a_wide_capture() {
    for err in [8000.0, -10_000.0] {
        let v = |off: f64, lvl: f64, per: f64, on: f64, ph: f64| TxChannel {
            offset_hz: off,
            level_db: lvl,
            tx_offset_hz: 0.0,
            depth: 0.8,
            audio: Audio::Voice,
            keying: Keying::Periodic(per, on, ph),
        };
        let chans = vec![
            v(-700e3, 0.0, 5.0, 1.5, 0.5),
            v(-700e3 + 25e3, -3.0, 6.0, 2.0, 2.0),
            v(-300e3 + STEP_833, 2.0, 7.0, 1.0, 1.0),
            v(-300e3 + 2.0 * STEP_833, 0.0, 4.0, 1.5, 2.5),
            v(0.0, 0.0, 6.0, 2.5, 0.5),
            v(250e3, -2.0, 5.5, 1.2, 3.0),
            v(500e3 + STEP_833, 1.0, 4.5, 1.0, 1.5),
            v(725e3, 0.0, 8.0, 3.0, 1.0),
        ];
        let p = GenParams {
            rate_hz: 2e6,
            channels: chans,
            snr_db: Some(25.0),
            tune_error_hz: err,
            ..Default::default()
        };
        let mut list = chans_for(&p.channels);
        // A quiet channel between two busy ones: nothing must leak into it.
        list.push(Channel {
            freq_hz: DIAL + 100e3,
            label: "quiet".into(),
            ..Default::default()
        });
        let (d, _) = run_scene(&p, 24.0, &list, &[], &mut |_, _| {});
        assert!(
            d.bank.known && (d.bank.tune_error_hz - err).abs() < 500.0,
            "{err}: measured {}",
            d.bank.tune_error_hz
        );
        for (tx, ch) in p.channels.iter().zip(&list) {
            timing(&d, tx, ch.freq_hz, 8.0, 24.0, 0.06).unwrap_or_else(|e| panic!("{err}: {e}"));
        }
        assert!(
            !d.log
                .iter()
                .any(|a| (a.freq_hz - (DIAL + 100e3)).abs() < 10.0),
            "the quiet channel opened"
        );
    }
}

#[test]
fn b16_band_watch() {
    for err in [0.0, 2500.0, -6000.0] {
        let p = GenParams {
            snr_db: Some(25.0),
            tune_error_hz: err,
            ..Default::default()
        };
        let want = chans_for(&p.channels);
        // Listed: one channel nobody uses, so every carrier of the scene is unlisted.
        let list = vec![Channel {
            freq_hz: DIAL - 100e3,
            ..Default::default()
        }];
        let (d, _) = run_scene(&p, 16.0, &list, &[], &mut |_, _| {});
        for w in &want {
            assert!(
                d.seen.entries.iter().any(|s| s.name == w.name()),
                "{err}: {} not seen in {:?}",
                w.name(),
                d.seen.entries.iter().map(|s| &s.name).collect::<Vec<_>>()
            );
        }
        for s in &d.seen.entries {
            assert!(
                want.iter().any(|w| w.name() == s.name),
                "{err}: unexpected {}",
                s.name
            );
        }
    }
    let p = GenParams {
        snr_db: Some(25.0),
        channels: vec![],
        ..Default::default()
    };
    let list = vec![Channel {
        freq_hz: DIAL,
        ..Default::default()
    }];
    let (d, _) = run_scene(&p, 10.0, &list, &[], &mut |_, _| {});
    assert!(d.seen.entries.is_empty() && d.log.is_empty());
}

#[test]
fn any_cut_returns_the_same_log() {
    let rec =
        sdrkit_dev_test::read_recording("avdata/airband_250k").expect("the airband recording");
    let list = vec![
        Channel {
            freq_hz: 118.7e6,
            label: "A".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: 118.708_333_333e6,
            is833: true,
            label: "B".into(),
            ..Default::default()
        },
        Channel {
            freq_hz: 118.65e6,
            label: "C".into(),
            ..Default::default()
        },
    ];
    let mut got = Vec::new();
    let mut audio_len = Vec::new();
    for chunk in [65_536usize, 999, 7] {
        let mut r = rx(rec.rate_hz, rec.centre_hz, &list, &[]);
        let mut out = Output::default();
        let mut n = 0;
        for c in rec.samples.chunks(chunk) {
            out.clear();
            r.feed(c, &mut out);
            n += out.audio().len();
        }
        let d = detail(r.as_ref());
        got.push(
            d.log
                .iter()
                .map(|a| {
                    (
                        a.name.clone(),
                        (a.start_s * 1e4).round() as i64,
                        (a.dur_s * 1e4).round() as i64,
                    )
                })
                .collect::<Vec<_>>(),
        );
        audio_len.push(n);
    }
    assert_eq!(got[0].len(), 4);
    assert_eq!(got[0], got[1]);
    assert_eq!(got[0], got[2]);
    assert!(
        audio_len
            .iter()
            .all(|&n| n.abs_diff(audio_len[0]) <= 2 * 48),
        "{audio_len:?}"
    );
}
