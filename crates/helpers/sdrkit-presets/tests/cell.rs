//! The cell band file of the repository's data folder: every row loads, and the rows give the settings and the scan lists
//! of the cell stacks.

use sdrkit_core::Value;
use sdrkit_presets::cell::{cell_list, cell_settings, read_cell_lists, CellList, CELL_BANDS_FILE};
use std::path::PathBuf;

fn rows() -> Vec<CellList> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data").join(CELL_BANDS_FILE);
    read_cell_lists(&path).expect("the cell band file")
}

#[test]
fn every_row_loads() {
    let rows = rows();
    // 12 rows named by area, and 3 rows (cell, nr, nbiot) for each of the 34 countries named by code.
    assert_eq!(rows.len(), 12 + 3 * 34);
    let au = cell_settings(&rows, "cell", "AU").expect("an AU row");
    assert_eq!(au, vec![("band_order".to_owned(), Value::Text("28 5 26 8 3 1 7 40".into()))]);
    for r in &rows {
        assert!(["cell", "nr", "nbiot", "gsmr"].contains(&r.stack.as_str()), "{r:?}");
        match r.list.as_str() {
            "band_order" | "scan_bands" | "scan_offer" => {
                assert_eq!(r.bands().len(), r.values.len(), "{r:?}");
                let mut b = r.bands();
                b.sort();
                b.dedup();
                assert_eq!(b.len(), r.values.len(), "a band twice: {r:?}");
            }
            "scan_mhz" => {
                let (lo, hi) = r.range_hz().expect("two edges");
                assert!(lo < hi, "{r:?}");
            }
            other => panic!("unknown list {other}"),
        }
    }
}

#[test]
fn band_order_settings_of_two_rows() {
    let rows = rows();
    for (stack, first) in [("cell", "1 3 7 8 20"), ("nbiot", "1 3 7 8 20"), ("nr", "78 77 28")] {
        let s = cell_settings(&rows, stack, "europe").expect("a europe row");
        match &s[..] {
            [(k, Value::Text(t))] => {
                assert_eq!(k, "band_order");
                assert!(t.starts_with(first), "{stack}: {t}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            cell_settings(&rows, stack, "band_number"),
            Some(vec![("band_order".to_owned(), Value::Text(String::new()))])
        );
    }
    assert!(cell_settings(&rows, "gsmr", "europe").is_none());
}

#[test]
fn scan_lists() {
    let rows = rows();
    let presets = cell_list(&rows, "cell", "scan_bands", "europe").expect("the LTE presets");
    assert_eq!(presets.bands(), vec![20, 8, 3, 1, 7]);
    let custom = cell_list(&rows, "cell", "scan_mhz", "europe").expect("the LTE custom range");
    assert_eq!(custom.range_hz(), Some((790e6, 830e6)));
    let nbiot = cell_list(&rows, "nbiot", "scan_offer", "europe").expect("the NB-IoT bands");
    assert_eq!(nbiot.bands().len(), 18);
    let railway = cell_list(&rows, "gsmr", "scan_mhz", "europe_railway").expect("the railway range");
    assert_eq!(railway.range_hz(), Some((921e6, 925e6)));
    let all = cell_list(&rows, "gsmr", "scan_mhz", "gsm900").expect("the GSM 900 range");
    assert_eq!(all.range_hz(), Some((921e6, 960e6)));
}

/// In Australia the presets of `cell` and `nr` give the band search the known carriers of the country first, and the
/// ranges where the country uses each band: n78 from 3400 MHz, not 3300 MHz.
#[test]
fn australian_band_search_settings() {
    let d = sdrkit_presets::Data::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")).expect("the data");
    let au = sdrkit_presets::Region::new("AU").expect("a country code");
    let text = |stack: &str, key: &str| -> String {
        let p = d.preset(stack, &au, &sdrkit_presets::Options::default()).expect("a preset");
        match p.settings.iter().find(|(k, _)| k == key) {
            Some((_, Value::Text(t))) => t.clone(),
            other => panic!("{stack} {key}: {other:?}"),
        }
    };
    assert_eq!(text("nr", "search_first"), "884.45");
    assert_eq!(text("cell", "search_first"), "778 763 795.5");
    let nr = text("nr", "search_ranges");
    assert!(nr.split(' ').any(|r| r == "3400-3800"), "{nr}");
    assert!(!nr.contains("3300"), "{nr}");
    assert!(text("cell", "search_ranges").split(' ').any(|r| r == "758-803"));
}
