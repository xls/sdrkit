//! The oceanographic radar bands, the scan plan and the radar hint.

/// A radar band: name, working range (MHz), range class.
pub const BANDS: &[(&str, f64, f64, &str)] = &[
    ("4.4 - 4.65 MHz", 4.40, 4.70, "long range"),
    ("5.25 MHz", 5.20, 5.30, "long range"),
    ("12 - 14 MHz", 11.90, 14.10, "medium range"),
    ("24.45 - 24.65 MHz", 24.40, 24.70, "short range"),
    ("41.5 - 42.5 MHz", 41.40, 42.60, "very short range"),
];

/// The name of the band a frequency (Hz) lies in, or `None` outside the radar bands.
pub fn band_of(hz: f64) -> Option<&'static str> {
    let m = hz / 1e6;
    BANDS.iter().find(|b| m >= b.1 && m <= b.2).map(|b| b.0)
}

/// The range class by centre frequency.
pub fn range_class(hz: f64) -> &'static str {
    let m = hz / 1e6;
    if m < 8.0 {
        "long range"
    } else if m < 20.0 {
        "medium range"
    } else if m < 30.0 {
        "short range"
    } else {
        "very short range"
    }
}

/// The scan plan: each band split into the fewest equal parts of at most 0.8 x rate, the parts' middles (rounded to kHz).
pub fn scan_plan(rate: f64) -> Vec<f64> {
    if rate.is_nan() || rate <= 0.0 {
        return Vec::new();
    }
    let span = 0.8 * rate;
    let mut v = Vec::new();
    for b in BANDS {
        let (lo, hi) = (b.1 * 1e6, b.2 * 1e6);
        let parts = ((hi - lo) / span).ceil().max(1.0) as usize;
        let w = (hi - lo) / parts as f64;
        for k in 0..parts {
            v.push(((lo + w * (k as f64 + 0.5)) / 1e3).round() * 1e3);
        }
    }
    v
}

/// The hint for a radar: never an identification.
pub fn hint(centre_hz: f64, width_hz: f64, repeat_s: Option<f64>) -> String {
    let kw = width_hz / 1e3;
    let km = 150.0 / kw.max(1e-9);
    let class = range_class(centre_hz);
    let rep = repeat_s.map_or("-".to_string(), |r| format!("{r:.2} s"));
    let seasonde =
        repeat_s.is_some_and(|r| (0.35..=1.2).contains(&r)) && (15.0..=150.0).contains(&kw);
    if seasonde {
        format!(
            "possible SeaSonde-like radar: {kw:.0} kHz sweep every {rep} (about {km:.1} km range resolution), {class} band"
        )
    } else {
        format!("possible HF ocean radar: {kw:.0} kHz sweep every {rep} (about {km:.1} km range resolution), {class} band")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c6_bands_plan_hint() {
        assert_eq!(band_of(13.0e6), Some("12 - 14 MHz"));
        assert_eq!(band_of(4.5e6), Some("4.4 - 4.65 MHz"));
        assert_eq!(band_of(7.0e6), None);
        assert_eq!(scan_plan(20e6).len(), 5);
        assert!(scan_plan(2e6).len() > 5);
        assert!(scan_plan(0.0).is_empty());
        for &rate in &[2e6, 10e6, 20e6, 250e3] {
            let plan = scan_plan(rate);
            for b in BANDS {
                let mut f = b.1 * 1e6;
                while f <= b.2 * 1e6 {
                    assert!(
                        plan.iter().any(|&p| (p - f).abs() <= 0.4 * rate + 1e3),
                        "{rate} {f}"
                    );
                    f += 10e3;
                }
            }
        }
        let h = hint(13.5e6, 50e3, Some(0.5));
        assert!(
            h.starts_with("possible") && h.contains("SeaSonde") && h.contains("3.0 km"),
            "{h}"
        );
        assert!(hint(13.5e6, 200e3, None).starts_with("possible HF ocean radar"));
    }
}
