//! The recording made off air in Sydney (ensemble 9A, 202.928 MHz, 2.1 Msps, 16-bit samples): the ensemble, its 34 services
//! and their labels, the label of 2SM, and the HE-AAC v2 access units of Triple M, byte for byte.

use super::common::{rows, select, set};
use sdrkit_core::SettingSet;
use sdrkit_core::{
    Complex32, InputInfo, Output, Owner, Receiver, StackConfig, StreamKind, Value, Values,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_dab::results::*;
use sdrkit_stack_dab::typed::*;
use sdrkit_stack_dab::DabSetting;

/// SId, label, short label, sub-channel, kbit/s, protection.
const EXPECTED: [(&str, &str, &str, u8, u16, &str); 34] = [
    ("1194", "2SM Talk & Sport", "2SM", 1, 64, "EEP 3-A"),
    ("1195", "GorillaDanceHits", "Gorilla", 2, 48, "EEP 4-A"),
    ("1196", "ZOO MusicVariety", "ZOO", 3, 48, "EEP 4-A"),
    ("1197", "Dance Super Digi", "Dance", 4, 48, "EEP 4-A"),
    ("1198", "FUN Classic Hits", "FUN", 5, 48, "EEP 4-A"),
    ("119E", "2DayFM", "2DayFM", 8, 32, "EEP 3-A"),
    ("119F", "Triple M", "Triple M", 9, 32, "EEP 3-A"),
    ("11A0", "MMM Classic Rock", "MMMClass", 16, 32, "EEP 3-A"),
    ("11A1", "MMM 90s", "MMM 90s", 11, 32, "EEP 3-A"),
    ("11A2", "OLDSKOOL 80s", "OLDSKL80", 13, 32, "EEP 3-A"),
    ("11A3", "Heart", "Heart", 12, 32, "EEP 3-A"),
    ("11A4", "MMM 80s", "MMM 80s", 14, 32, "EEP 3-A"),
    ("11A5", "OLDSKOOL 90 HITS", "OLDSKL90", 10, 32, "EEP 3-A"),
    ("11A6", "RnB FRIDAYS", "RnB FRI", 15, 32, "EEP 3-A"),
    ("11A7", "BLENDER BEATS", "BLENDER", 19, 32, "EEP 3-A"),
    ("11A8", "MMM 2000s", "MMM 00s", 17, 32, "EEP 3-A"),
    ("11A9", "DANCE HITS", "DANCE", 18, 32, "EEP 3-A"),
    ("11AB", "INDIE + ALT", "INDIE", 20, 32, "EEP 3-A"),
    ("11AC", "80s NEW WAVE", "80s WAVE", 21, 32, "EEP 3-A"),
    ("11B2", "SEN MELBOURNE", "SEN MELB", 26, 16, "EEP 4-A"),
    ("11B3", "Vision Christian", "Vision C", 23, 64, "EEP 3-B"),
    ("11B4", "Bunnings Tradio", "Bunnings", 22, 32, "EEP 3-B"),
    ("11B5", "SEN 1170", "SEN 1170", 25, 32, "EEP 3-A"),
    ("11B6", "Rete Italia", "Rete Ita", 24, 16, "EEP 4-A"),
    ("11E4", "SKY SPORTS RADIO", "SSR", 29, 56, "EEP 3-A"),
    ("11E5", "SkyThroughBredC", "STC", 30, 40, "EEP 3-A"),
    ("11E6", "Niche 01", "Niche 01", 31, 16, "EEP 3-A"),
    ("11E7", "Niche 02", "Niche 02", 32, 16, "EEP 3-A"),
    ("120C", "2MBS Fine Music", "2MBS", 51, 64, "EEP 3-A"),
    ("120D", "Koori Radio", "Koori", 50, 32, "EEP 3-A"),
    ("120E", "2RPH Digital", "2RPH", 53, 32, "EEP 4-A"),
    ("120F", "Christmas Hope", "Christms", 54, 32, "EEP 4-A"),
    ("1210", "Eastside Radio", "Eastside", 55, 48, "EEP 3-A"),
    ("121F", "2Triple0 985", "2Triple0", 52, 64, "EEP 3-B"),
];

/// The samples of the recording. The test stops here when the recording is missing (it never is in the family gate).
fn samples() -> Option<(f64, f64, Vec<Complex32>)> {
    let path = sdrkit_dev_test::recording("real/dab/dab_9a_sydney")?;
    let raw = std::fs::read(path).expect("the recording reads");
    let x = raw
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| {
            let i = i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0;
            let q = i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0;
            Complex32::new(i, q)
        })
        .collect();
    Some((2.1e6, 202.928e6, x))
}

/// FNV-1a over the bytes of every access unit, in order.
fn fnv(units: &[&[u8]]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for u in units {
        for &b in *u {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

struct Decoded {
    rx: StackGraph,
    units: Vec<Vec<u8>>,
    codec: Option<sdrkit_core::media::Codec>,
    config: Vec<u8>,
}

/// Decodes the recording playing the service whose row key is `sid` (`None`: the first service), with the AAC packaging
/// `packaging` (0 raw, 1 LATM).
fn decode(sid: Option<&str>, packaging: u16) -> Option<Decoded> {
    let (rate, centre, x) = samples()?;
    let values = Values::defaults(&DabSetting::schema());
    let mut rx = sdrkit_stack_dab::receiver::receiver(&StackConfig::new(values)).expect("builds");
    set(&mut rx, "aac_packaging", Value::Choice(packaging));
    if let Some(sid) = sid {
        select(&mut rx, sid);
    }
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .expect("rate");
    let mut out = Output::default();
    let mut units = Vec::new();
    let mut codec = None;
    let mut config = Vec::new();
    for piece in x.chunks(100_000) {
        out.clear();
        rx.feed(piece, &mut out);
        for m in out
            .media
            .iter()
            .filter(|m| m.kind == StreamKind::HeAac || m.kind == StreamKind::Aac)
        {
            codec = Some(m.format.codec);
            config = m.format.config.to_vec();
            units.push(m.packet.bytes().to_vec());
        }
    }
    Some(Decoded {
        rx,
        units,
        codec,
        config,
    })
}

#[test]
fn the_sydney_9a_ensemble_lists_its_34_services() {
    let Some(d) = decode(None, 0) else { return };
    let r = d.rx.results();
    let f = r.fields();
    assert_eq!(
        (
            f.at(&EnsembleFields::EID),
            f.at(&EnsembleFields::ENSEMBLE_LABEL),
            f.at(&EnsembleFields::ENSEMBLE_VALID)
        ),
        (Some(0x1004), Some("DAB+ Sydney 1"), Some(true))
    );
    let services = rows(r, &SERVICES);
    assert_eq!(services.len(), 34);
    for (sid, label, short, sub, kbps, prot) in EXPECTED {
        let got = services.iter().find(|(k, _)| *k == sid).unwrap().1;
        assert_eq!(
            (
                got.at(&ServiceRow::LABEL),
                got.at(&ServiceRow::SHORT_LABEL),
                got.at(&ServiceRow::SUBCHANNEL),
                got.at(&ServiceRow::BITRATE_KBPS)
            ),
            (
                Some(label),
                Some(short),
                Some(i64::from(sub)),
                Some(i64::from(kbps))
            ),
            "{sid}"
        );
        assert_eq!(got.at(&ServiceRow::PROTECTION), Some(prot), "{sid}");
        assert_eq!(
            got.at(&ServiceRow::KIND),
            Some(ServiceKind::DabPlus),
            "{sid}"
        );
    }
    assert_eq!(f.at(&SyncFields::FIB_BAD), Some(0));
    assert!(f.at(&SyncFields::FIB_OK).unwrap_or(0) >= 500);
    // With no service selected, the first service plays.
    assert_eq!(f.at(&PlayingFields::SELECTED_SERVICE), Some("1194"));
    assert!(!d.units.is_empty());
}

#[test]
fn triple_m_decodes_to_the_same_he_aac_v2_units() {
    let Some(d) = decode(Some("119F"), 0) else {
        return;
    };
    assert_eq!(d.codec, Some(sdrkit_core::media::Codec::HeAacV2));
    assert_eq!(d.units.len(), 91);
    assert_eq!(d.units.iter().map(Vec::len).sum::<usize>(), 12_997);
    let refs: Vec<&[u8]> = d.units.iter().map(Vec::as_slice).collect();
    assert_eq!(fnv(&refs), 0x0e40_f441_1243_b2fc);
    let f = d.rx.results().fields();
    assert_eq!(f.at(&PlayingFields::SELECTED_LABEL), Some("Triple M"));
    assert_eq!(f.at(&AudioFields::AUDIO_CODEC), Some(AudioCodec::HeAacV2));
    assert_eq!(d.config, [0xEB, 0x09, 0x8A, 0x00]);
}

#[test]
fn two_sm_shows_its_dynamic_label() {
    let Some(d) = decode(Some("1194"), 0) else {
        return;
    };
    let f = d.rx.results().fields();
    assert_eq!(
        f.get(&sdrkit_core::publish::keys::TEXT),
        Some("2SM 1269 AM and DAB+")
    );
    assert_eq!(d.codec, Some(sdrkit_core::media::Codec::HeAac));
    assert_eq!(d.units.len(), 91);
}

#[test]
fn triple_m_in_latm_carries_its_25_bit_config_unpadded() {
    let Some(d) = decode(Some("119F"), 1) else {
        return;
    };
    assert_eq!(d.units.len(), 91);
    assert_eq!(d.units.iter().map(Vec::len).sum::<usize>(), 13_725);
    let refs: Vec<&[u8]> = d.units.iter().map(Vec::as_slice).collect();
    assert_eq!(fnv(&refs), 0xe9ef_2638_ba07_aa27);
}
