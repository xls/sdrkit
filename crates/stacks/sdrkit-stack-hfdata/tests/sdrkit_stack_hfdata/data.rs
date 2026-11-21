//! The data files of the crate: the starting dials and the band presets of the HF serial-tone and naval HF stacks. The stacks
//! hold neither.

use crate::common::data_rows;
use sdrkit_stack_hfdata::descriptors;

#[test]
fn the_data_files_have_their_rows() {
    let dials = data_rows("hf_serial_dials.csv");
    assert_eq!(dials.len(), 6);
    for row in &dials {
        let d = descriptors()
            .into_iter()
            .find(|d| d.id == row[0])
            .unwrap_or_else(|| panic!("no stack {}", row[0]));
        let hz: f64 = row[1].parse().unwrap();
        assert!(hz >= d.tuning.min_hz && hz <= d.tuning.max_hz, "{}", d.id);
        assert_ne!(
            hz, d.tuning.default_hz,
            "{}: the descriptor holds the test-signal dial",
            d.id
        );
        // The usual range of the file lies in the tuning range of the stack.
        assert!(row[2].parse::<f64>().unwrap() >= d.tuning.min_hz);
        assert!(row[3].parse::<f64>().unwrap() <= d.tuning.max_hz);
    }
    let bands = data_rows("hf_band_presets.csv");
    assert_eq!(bands.len(), 6);
    for b in &bands {
        let hz: f64 = b[1].parse().unwrap();
        assert!((3e6..30e6).contains(&hz), "{b:?}");
    }
}
