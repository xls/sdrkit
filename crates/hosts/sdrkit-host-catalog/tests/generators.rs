//! The test signal of every stack is made at the sample rate that it is asked for. A generator that makes it at another
//! rate has every frequency and every symbol rate of the signal wrong by the ratio of the two, and nothing else shows it.

use sdrkit_core::{GenConfig, Values};

#[test]
fn every_test_signal_is_at_the_rate_asked_for() {
    let mut faults = Vec::new();
    for d in sdrkit_host_catalog::all() {
        let Some(make) = d.make_generator else {
            continue;
        };
        let schema = sdrkit_components::sim::with_generator_settings(&(d.settings)()).unwrap();
        let rate = d.tuning.rate_hz;
        match make(&GenConfig::new(rate, Values::defaults(&schema))) {
            Ok(g) if (g.rate_hz() - rate).abs() > 1e-3 * rate => {
                faults.push(format!("{}: asked {rate}, made at {}", d.id, g.rate_hz()));
            }
            Ok(_) => {}
            // HamDRM has no test signal until the host gives the file with its format.
            Err(_) if d.id == "hamdrm" => {}
            Err(e) => faults.push(format!("{}: {e}", d.id)),
        }
    }
    assert!(
        faults.is_empty(),
        "{} faults:\n{}",
        faults.len(),
        faults.join("\n")
    );
}
