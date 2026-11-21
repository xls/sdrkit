//! The data file of the usual dials of the radio science monitors: every row of a stack of the crate lies in its tuning
//! range, and a row without a submode is the stack's default dial.

use sdrkit_stack_science as sci;

const DIALS: &str = include_str!("../../../../../data/science/science_dials.csv");

#[test]
fn the_dials_file_agrees_with_the_stacks() {
    let mut rows = 0;
    for line in DIALS.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let cells: Vec<&str> = line.splitn(5, ',').collect();
        assert_eq!(cells.len(), 5, "{line}");
        let (stack, submode) = cells[0].split_once('/').map_or((cells[0], None), |(a, b)| (a, Some(b)));
        let f: f64 = cells[1].parse().unwrap();
        let (lo, hi): (f64, f64) = (cells[2].parse().unwrap(), cells[3].parse().unwrap());
        assert!(lo <= f && f <= hi, "{line}");
        let Some(d) = sci::descriptors().into_iter().find(|d| d.id == stack) else {
            continue;
        };
        rows += 1;
        if submode.is_none() {
            assert_eq!(f, d.tuning.default_hz, "{line}");
            assert_eq!((lo, hi), (d.tuning.min_hz, d.tuning.max_hz), "{line}");
        }
    }
    assert!(rows >= 6, "{rows}");
}
