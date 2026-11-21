//! Channel names that a standard defines by a rule, not by a table.
//!
//! Standard: ICAO Annex 10 Volume V, 4.1.2.4 (8.33 kHz channel naming).

/// The ICAO name of the 8.33 kHz channel at `hz` ("118.005" for 118.000 MHz).
///
/// The three 8.33 kHz channels in each 25 kHz step are named by the 25 kHz frequency plus 5, 10 and 15 kHz. The function
/// rounds `hz` to the nearest 8.33 kHz channel first.
pub fn icao_833_name(hz: f64) -> String {
    let step = 25e3 / 3.0;
    let k_total = (hz / step).round() as i64;
    let block = k_total.div_euclid(3);
    let k = k_total.rem_euclid(3);
    let name_hz = block as f64 * 25e3 + 5e3 * (k + 1) as f64;
    format!("{:.3}", name_hz / 1e6)
}
