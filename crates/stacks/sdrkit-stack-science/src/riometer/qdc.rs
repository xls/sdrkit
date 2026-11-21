//! The quiet-day curve of the riometer: the power of the quiet sky in 288 bins of 5 sidereal minutes, and the absorption
//! against it.
//!
//! Each minute power `p` that is not interfered updates the bin of its local sidereal time: an empty bin takes `p`, unless
//! `p >= 1.15 n` for every bin `n` beside it that holds a value (the sky moves far less than that from bin to bin, so such a
//! `p` is interference and would stay as the bin's value); when `q < p < 1.15 q`, `q = 0.8 q + 0.2 p` (the upper envelope of quiet minutes: a jump of more than 0.6 dB is interference, not
//! sky); when `p <= q`, `q = 0.999 q + 0.001 p` (a slow pull-down for gain drift); `p >= 1.15 q` leaves `q`. The curve is ready
//! when 36 or more bins hold a value. Absorption = max(-2, 10 log10(q / p)) dB once ready.
//!
//! The text form is `qdc1` and the 288 linear values in `%.4g` form, separated by single spaces; 0 is an empty bin.
//!
//! Document: G. C. Little and H. Leinbach, "The Riometer - A Device for the Continuous Measurement of Ionospheric Absorption",
//! Proc. IRE 47 (2), 1959, pp. 315-320 (absorption as the quiet-day level over the measured level).

/// Bins of the curve.
pub const BINS: usize = 288;
/// Bins with a value for a ready curve.
pub const READY_BINS: usize = 36;
/// A power this many times the bin or more leaves the bin.
pub const JUMP: f64 = 1.15;
/// The weight of a power that raises the bin.
pub const RISE: f64 = 0.2;
/// The weight of a power that lowers the bin.
pub const FALL: f64 = 0.001;
/// The least absorption reported, dB.
pub const FLOOR_DB: f64 = -2.0;
/// The tag of the text form.
pub const TAG: &str = "qdc1";

/// Local mean sidereal time at Unix time `unix` and east longitude `lon_deg`, hours 0 to 24. GMST is the IAU 1982 form of
/// `sdrkit-util-geo`, which at J2000.0 is the 18.697374558 h of the USNO formula.
pub fn lst_hours(unix: f64, lon_deg: f64) -> f64 {
    crate::astro::sky::lmst_deg(crate::astro::sky::jd_unix(unix), lon_deg) / 15.0
}

/// The bin of a sidereal time (hours).
pub fn bin_of(lst_h: f64) -> usize {
    ((lst_h.rem_euclid(24.0) / 24.0 * BINS as f64).floor() as usize).min(BINS - 1)
}

/// `%.4g`: four significant digits, trailing zeros removed, the exponent form below 1e-4 or from 1e4.
pub fn g4(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return "0".into();
    }
    let exp = v.abs().log10().floor() as i32;
    let trim = |s: String| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    if !(-4..4).contains(&exp) {
        let s = format!("{v:.3e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let e: i32 = e.parse().unwrap_or(0);
        format!("{}e{}{:02}", trim(m.to_string()), if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        let decimals = (3 - exp).max(0) as usize;
        trim(format!("{v:.decimals$}"))
    }
}

/// The curve.
#[derive(Clone, Debug, PartialEq)]
pub struct Qdc {
    q: Vec<f64>,
}

impl Default for Qdc {
    fn default() -> Self {
        Qdc { q: vec![0.0; BINS] }
    }
}

impl Qdc {
    /// The value of each bin; 0 for an empty one.
    pub fn bins(&self) -> &[f64] {
        &self.q
    }

    /// Bins that hold a value.
    pub fn filled(&self) -> usize {
        self.q.iter().filter(|&&v| v > 0.0).count()
    }

    /// Whether the curve is ready.
    pub fn ready(&self) -> bool {
        self.filled() >= READY_BINS
    }

    /// The value at a sidereal time, `None` for an empty bin.
    pub fn at(&self, lst_h: f64) -> Option<f64> {
        Some(self.q[bin_of(lst_h)]).filter(|&v| v > 0.0)
    }

    /// Updates the bin of `lst_h` with a minute power `p` (module docs). Interfered minutes and powers that are not positive are
    /// left out.
    pub fn update(&mut self, lst_h: f64, p: f64, interfered: bool) {
        if interfered || !p.is_finite() || p <= 0.0 {
            return;
        }
        let k = bin_of(lst_h);
        if self.q[k] <= 0.0 {
            let mut beside = [(k + BINS - 1) % BINS, (k + 1) % BINS]
                .into_iter()
                .map(|n| self.q[n])
                .filter(|&n| n > 0.0)
                .peekable();
            if beside.peek().is_none() || beside.any(|n| p < JUMP * n) {
                self.q[k] = p;
            }
            return;
        }
        let q = &mut self.q[k];
        if p > *q && p < JUMP * *q {
            *q = (1.0 - RISE) * *q + RISE * p;
        } else if p <= *q {
            *q = (1.0 - FALL) * *q + FALL * p;
        }
    }

    /// The absorption of a power `p` at `lst_h`, dB: max(-2, 10 log10(q / p)) once the curve is ready and its bin holds a
    /// value, else 0.
    pub fn absorption(&self, lst_h: f64, p: f64) -> f64 {
        match (self.ready(), self.at(lst_h)) {
            (true, Some(q)) if p > 0.0 => (10.0 * (q / p).log10()).max(FLOOR_DB),
            _ => 0.0,
        }
    }

    /// The text form.
    pub fn export(&self) -> String {
        let mut s = String::from(TAG);
        for v in &self.q {
            s.push(' ');
            s.push_str(&g4(*v));
        }
        s
    }

    /// The curve of a text form: the tag and exactly 288 finite non-negative numbers.
    ///
    /// # Errors
    ///
    /// A text that says what is wrong.
    pub fn import(text: &str) -> Result<Qdc, String> {
        let mut w = text.split_whitespace();
        if w.next() != Some(TAG) {
            return Err(format!("the text does not start with {TAG}"));
        }
        let q: Vec<f64> = w
            .map(|t| t.parse::<f64>().map_err(|_| format!("not a number: {t}")))
            .collect::<Result<_, _>>()?;
        if q.len() != BINS {
            return Err(format!("{} values, {BINS} expected", q.len()));
        }
        if q.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err("a value is negative or not finite".into());
        }
        Ok(Qdc { q })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g4_is_the_c_form() {
        assert_eq!(g4(1.0), "1");
        assert_eq!(g4(0.001234567), "0.001235");
        assert_eq!(g4(1234.5), "1234");
        assert_eq!(g4(12345.0), "1.234e+04");
        assert_eq!(g4(0.00001234), "1.234e-05");
        assert_eq!(g4(2.5), "2.5");
        assert_eq!(g4(0.0), "0");
    }
}
