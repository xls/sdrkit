//! The declarations that a generic consumer reads from the four stacks are the ones in `tests/declarations.txt`
//! (`SDRKIT_WRITE_DECLARATIONS=1` writes the file), and a consumer reads rows and result fields as structs.

use std::path::PathBuf;

#[test]
fn the_declarations_of_the_four_stacks_are_the_recorded_ones() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/declarations.txt");
    if let Err(e) =
        sdrkit_dev_interface::declarations::check(&sdrkit_stack_adsb::descriptors(), &path)
    {
        panic!("{e}");
    }
}

/// A consumer reads the aircraft table and the tracker fields as structs.
#[test]
fn a_consumer_reads_rows_and_fields_as_structs() {
    use super::common::{at_station, receiver, run, CENTRE};
    use sdrkit_core::interface::ReadFields;
    use sdrkit_core::{Complex32, Generator};
    use sdrkit_stack_adsb::generator::{AdsbGenerator, AirspaceOptions};
    use sdrkit_stack_adsb::typed::{AircraftRow, TrackerFields, AIRCRAFT};
    let rate = 4e6;
    let mut g = AdsbGenerator::airspace(rate, AirspaceOptions::default(), 2);
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 3.0) as usize];
    g.generate(&mut x);
    let mut rx = receiver(&[]);
    at_station(&mut rx, CENTRE.0, CENTRE.1);
    run(&mut rx, rate, &x, &[65536]);
    let results = sdrkit_core::Owner::results(&rx);
    let rows: Vec<(&str, AircraftRow)> = AIRCRAFT.rows(results).collect();
    let fields = TrackerFields::read(results.fields()).expect("the tracker fields");
    assert_eq!(rows.len() as i64, fields.aircraft_count);
    assert!(rows.len() >= 8);
    for (key, a) in &rows {
        assert_eq!(*key, a.icao);
        assert_eq!(a.source, "0");
    }
    let placed = rows.iter().filter(|(_, a)| a.position.is_some()).count();
    assert_eq!(placed as i64, fields.with_position);
    assert!(rows
        .iter()
        .any(|(_, a)| a.callsign.is_some() && a.distance_m.is_some()));
}
