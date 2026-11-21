//! A recording made off air in Sydney: Triple M on 104.9 MHz (2.4 Msps, 16-bit samples). The receiver, set to Australia, must
//! report the station name, PI, programme type and RDS rate that the station sent on air.

use super::common::{detail, values};
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig};
use sdrkit_stack_fm::graph;

/// The samples of the recording. The test stops here when the recording is missing .
fn samples() -> Option<Vec<Complex32>> {
    let path = sdrkit_dev_test::recording("real/fm/fm_triplem_1049_sydney")?;
    let raw = std::fs::read(path).expect("the recording reads");
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

#[test]
fn triple_m_in_sydney() {
    let Some(x) = samples() else { return };
    let mut rx = graph::receiver(&StackConfig::new(values(&[("rds_variant", "rds")]))).expect("builds");
    rx.configure(&InputInfo::new(2_399_999.0, 104.9e6, 104.9e6))
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65_536) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let d = detail(&rx);
    assert_eq!(d.station, "TRIPLE M");
    assert_eq!(d.pi, 0x2049);
    assert_eq!(d.pty, Some(11));
    assert_eq!(d.programme, "Rock Music");
    assert!(!d.rbds && d.call_letters.is_empty());
    assert!(d.stereo && d.state == 2);
    // On air: 96 % usable blocks and 35 groups in these 6.1 s.
    assert!(d.rds && d.rds_ok >= 95.0, "usable {}", d.rds_ok);
    assert!(d.groups >= 33, "groups {}", d.groups);
}
