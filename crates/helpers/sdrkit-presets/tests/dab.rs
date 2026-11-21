//! The DAB groups of the repository's data folder: every group loads, and the settings list the centres of its blocks.

use sdrkit_core::Value;
use sdrkit_presets::dab::{dab_settings, read_dab_groups, DabGroup, DAB_PLANS_FILE};
use std::path::PathBuf;

fn groups() -> Vec<DabGroup> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data")
        .join(DAB_PLANS_FILE);
    read_dab_groups(&path).expect("the DAB plan file")
}

fn channels(s: &[(String, Value)]) -> Vec<f64> {
    match s.iter().find(|(k, _)| k == "channels") {
        Some((_, Value::Text(t))) => t.split(';').map(|f| f.parse().unwrap()).collect(),
        _ => panic!("no channels setting"),
    }
}

#[test]
fn every_dab_group_loads_with_its_blocks() {
    let g = groups();
    let count = |id: &str| g.iter().find(|x| x.id == id).unwrap().channels.len();
    assert_eq!(
        (
            count("dab_band3"),
            count("dab_band3_lband"),
            count("dab_tdmb_kr")
        ),
        (38, 23, 21)
    );
    let both = channels(&dab_settings(&g, &["dab_band3", "dab_band3_lband"]).unwrap());
    assert_eq!(both.len(), 61);
    assert_eq!((both[0], both[60]), (174.928e6, 1490.624e6));
    let kr = channels(&dab_settings(&g, &["dab_tdmb_kr"]).unwrap());
    assert_eq!(kr[0], 175.28e6);
    assert!(dab_settings(&g, &["dab_atlantis"]).is_none());
}
