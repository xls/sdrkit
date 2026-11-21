//! The recordings made off air in Sydney: ABC on Australian channel 12 (226.5 MHz, a 7 MHz channel, 8K 1/16 64-QAM 3/4). The
//! held-gain recording decodes as the live signal did: 16 services, 33 dB, the guide, teletext page 801 and H.264 video that
//! starts at a keyframe. The first recording is clipped by too much radio gain, so its low SNR is not the receiver's.

use crate::common::{config, has_row, rows, select, value};
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Receiver, Results, StreamKind, Value};
use sdrkit_proto_mpegts::publish::{NAME, SERVICES};
use sdrkit_stack_dvbt::bandwidth::WidthSource;
use sdrkit_stack_dvbt::params::{CodeRate, Constellation, Guard, Mode};
use sdrkit_stack_dvbt::typed::DvbtFields as F;

/// The sample rate of both recordings, 64/7 MHz.
const RATE: f64 = 9_142_856.0;
/// The centre of both recordings, Australian channel 12.
const CENTRE: f64 = 226.5e6;

/// The samples of a recording of 16-bit pairs, or `None` when the recording is missing.
fn samples(name: &str) -> Option<Vec<Complex32>> {
    let raw = std::fs::read(sdrkit_dev_test::recording(name)?).expect("the recording reads");
    Some(
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                let i = i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0;
                let q = i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0;
                Complex32::new(i, q)
            })
            .collect(),
    )
}

/// The results of the held-gain recording.
struct Decoded {
    results: Results,
    /// The results after the host changed the frequency.
    forgotten: Results,
    services: Vec<String>,
    first_video_keyframe: Option<bool>,
}

fn decode_held_gain() -> Option<Decoded> {
    let x = samples("real/dvbt/dvbt_ch12_abc_sydney_held_gain")?;
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&config(&[])).unwrap();
    rx.configure(&InputInfo::new(RATE, CENTRE, CENTRE).recording())
        .unwrap();
    let mut out = Output::default();
    let mut first_video_keyframe = None;
    let mut selected = false;
    for c in x.chunks(16384) {
        rx.feed(c, &mut out);
        // The service plays once the service list names it.
        if !selected && has_row(Owner::results(&rx), "services", "549") {
            select(&mut rx, "549");
            selected = true;
        }
        if first_video_keyframe.is_none() {
            first_video_keyframe = out
                .media
                .iter()
                .find(|m| m.kind == StreamKind::H264)
                .map(|m| m.packet.keyframe);
        }
        out.clear();
    }
    rx.finish(&mut out);
    let r = Owner::results(&rx);
    let services = r
        .decl()
        .table_id(SERVICES.key)
        .and_then(|id| r.table(id))
        .map(|t| {
            t.rows_newest_first()
                .filter_map(|(_, row)| row.get(&NAME).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let results = r.clone();
    rx.forget();
    Some(Decoded {
        results,
        forgotten: Owner::results(&rx).clone(),
        services,
        first_video_keyframe,
    })
}

/// One decode serves every check of the held-gain recording.
fn held_gain() -> Option<&'static Decoded> {
    static D: std::sync::OnceLock<Option<Decoded>> = std::sync::OnceLock::new();
    D.get_or_init(decode_held_gain).as_ref()
}

#[test]
fn held_gain_recording_decodes_as_live() {
    let Some(d) = held_gain() else { return };
    let r = &d.results;
    let f = r.fields();
    let snr = r.status().snr_db.unwrap_or(0.0);
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert!((33.6..35.1).contains(&snr), "SNR {snr}");
    assert_eq!(f.at(&F::FFT), Some(Mode::K8));
    assert_eq!(f.at(&F::GUARD), Some(Guard::G16));
    assert_eq!(f.at(&F::CONSTELLATION), Some(Constellation::Qam64));
    assert_eq!(f.at(&F::CODE_RATE_HP), Some(CodeRate::R34));
    assert_eq!(f.at(&F::RS_FAILED), Some(0));
    let packets = f.at(&F::PACKETS).unwrap_or(0);
    assert!(packets >= 36_000, "{packets} packets");
    assert_eq!(d.services.len(), 16, "{:?}", d.services);
    for n in [
        "ABC NEWS",
        "ABC TV",
        "ABCTV HD",
        "triple j",
        "ABC Classic",
        "ABC NewsRadio",
    ] {
        assert!(d.services.iter().any(|s| s == n), "{n} in {:?}", d.services);
    }
    let guide = rows(r, "guide");
    assert!(guide >= 700, "{guide} guide events");
    let page = r
        .decl()
        .table_id("teletext_pages")
        .and_then(|id| r.table(id))
        .and_then(|t| t.row("801"));
    assert!(page.is_some(), "teletext page 801");
}

/// A change of frequency keeps the guide, and forgets the services, the PIDs and every
/// measurement of the multiplex before.
#[test]
fn a_change_of_frequency_keeps_the_guide_and_forgets_the_multiplex() {
    assert_eq!(
        sdrkit_stack_dvbt::typed::shape("dvbt").kept_keys(),
        Ok(["guide", "guide_ratings"]
            .map(|t| format!("table {t}"))
            .to_vec())
    );
    let Some(d) = held_gain() else { return };
    let (before, after) = (&d.results, &d.forgotten);
    assert!(rows(before, "guide") >= 700);
    for t in ["guide", "guide_ratings"] {
        assert_eq!(rows(after, t), rows(before, t), "{t}");
    }
    for t in ["services", "ts_demux.pids"] {
        assert!(rows(before, t) > 0, "{t}");
        assert_eq!(rows(after, t), 0, "{t}");
    }
    assert_eq!(before.fields().at(&F::FFT), Some(Mode::K8));
    assert_eq!(after.fields().at(&F::FFT), None);
    assert!(before.fields().at(&F::LEVEL_DBFS).is_some());
    assert_eq!(after.fields().at(&F::LEVEL_DBFS), None);
    assert_eq!(after.fields().at(&F::CONSTELLATION), None);
    assert!(matches!(value(before, "sync.timing"), Some(Value::F64(_))));
    assert_eq!(value(after, "sync.timing"), Some(&Value::None));
}

/// The width measured on the 7 MHz channel, with its neighbour channel 11 on air, is a 7 MHz channel.
#[test]
fn held_gain_recording_measures_a_7_mhz_channel() {
    let Some(d) = held_gain() else { return };
    let f = d.results.fields();
    assert_eq!(f.at(&F::BANDWIDTH_FROM), Some(WidthSource::Measured));
    assert_eq!(f.at(&F::CHANNEL_WIDTH), Some(7e6));
}

/// Media for the host start at a keyframe, not in the middle of a group of pictures.
#[test]
fn held_gain_recording_video_starts_at_a_keyframe() {
    let Some(d) = held_gain() else { return };
    assert_eq!(d.first_video_keyframe, Some(true));
}

/// The first recording replays at about 4 dB because the radio gain was too high: its mean power is above full scale
/// and its samples clip. The held-gain recording of the same multiplex (about -16.5 dBFS) replays at 33 dB, as live.
#[test]
fn the_low_snr_recording_is_clipped_by_the_radio() {
    let Some(x) = samples("real/dvbt/dvbt_ch12_abc_sydney") else {
        return;
    };
    let n = x.len().min(2_000_000);
    let power: f64 = x[..n].iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / n as f64;
    let clipped = x[..n]
        .iter()
        .filter(|v| v.re.abs() >= 0.9999 || v.im.abs() >= 0.9999)
        .count();
    assert!(10.0 * power.log10() > 0.0, "mean power {power}");
    assert!(clipped * 20 > n, "{clipped} of {n} samples at full scale");
    let Some(d) = held_gain() else { return };
    assert!(d.results.status().snr_db.unwrap_or(0.0) > 33.6);
}
