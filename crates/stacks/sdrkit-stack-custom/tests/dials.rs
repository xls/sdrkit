//! The data file of the usual dials: every row of a stack of the crate is its default dial, and its usual range lies in
//! the tuning range of the stack.

const DIALS: &str = include_str!("../../../../data/custom/custom_dials.csv");

#[test]
fn the_dials_file_agrees_with_the_stacks() {
    let mut rows = 0;
    for line in DIALS.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let cells: Vec<&str> = line.splitn(5, ',').collect();
        assert_eq!(cells.len(), 5, "{line}");
        let f: f64 = cells[1].parse().unwrap();
        let (lo, hi): (f64, f64) = (cells[2].parse().unwrap(), cells[3].parse().unwrap());
        assert!(lo <= f && f <= hi, "{line}");
        let Some(d) = sdrkit_stack_custom::descriptors()
            .into_iter()
            .find(|d| d.id == cells[0])
        else {
            continue;
        };
        rows += 1;
        assert_eq!(f, d.tuning.default_hz, "{line}");
        assert!(d.tuning.min_hz <= lo && hi <= d.tuning.max_hz, "{line}");
    }
    assert_eq!(rows, sdrkit_stack_custom::descriptors().len());
}
