//! Weather reports (APRS 1.0.1 chapter 12): the values after a weather station's position, and positionless weather.

/// Weather values. Each value is in the unit of the standard, named in the field. A field the station did not send, or sent as
/// dots or blanks, is `None`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Weather {
    /// Wind direction, degrees.
    pub wind_dir_deg: Option<u16>,
    /// Sustained wind speed (one minute), mph.
    pub wind_mph: Option<f64>,
    /// Gust (peak in the last five minutes), mph.
    pub gust_mph: Option<f64>,
    /// Temperature, degrees Fahrenheit.
    pub temp_f: Option<f64>,
    /// Rain in the last hour, inches.
    pub rain_1h_in: Option<f64>,
    /// Rain in the last 24 hours, inches.
    pub rain_24h_in: Option<f64>,
    /// Rain since midnight, inches.
    pub rain_midnight_in: Option<f64>,
    /// Relative humidity, percent.
    pub humidity_pct: Option<u8>,
    /// Barometric pressure, hPa.
    pub pressure_hpa: Option<f64>,
    /// Luminosity, W/m².
    pub luminosity_w_m2: Option<u16>,
    /// Snowfall in the last 24 hours, inches.
    pub snow_in: Option<f64>,
    /// The raw rain counter.
    pub rain_raw: Option<u16>,
    /// The software type and weather unit code after the values (for example `wRSW`).
    pub software: String,
}

impl Weather {
    /// Reads weather fields from the start of `b` and returns what follows them. If `positionless` is true, the first `s` is the
    /// wind speed (positionless weather sends `c` and `s` as fields). Otherwise `s` is snowfall.
    pub(crate) fn read_fields<'a>(&mut self, b: &'a [u8], positionless: bool) -> &'a [u8] {
        let mut i = 0;
        let mut wind_speed_due = positionless;
        while i < b.len() {
            let c = b[i];
            let n = match c {
                b'c' | b's' | b'g' | b't' | b'r' | b'p' | b'P' | b'L' | b'l' | b'#' => 3,
                b'h' => 2,
                b'b' => 5,
                _ => break,
            };
            let Some(field) = b.get(i + 1..i + 1 + n) else {
                break;
            };
            let ok = field
                .iter()
                .all(|&d| d.is_ascii_digit() || d == b'.' || d == b' ' || d == b'-');
            if !ok {
                break;
            }
            let v: Option<f64> = std::str::from_utf8(field)
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .map(f64::from);
            match c {
                b'c' => self.wind_dir_deg = v.map(|x| x as u16),
                b's' if wind_speed_due => {
                    self.wind_mph = v;
                    wind_speed_due = false;
                }
                b's' => self.snow_in = v,
                b'g' => self.gust_mph = v,
                b't' => self.temp_f = v,
                b'r' => self.rain_1h_in = v.map(|x| x / 100.0),
                b'p' => self.rain_24h_in = v.map(|x| x / 100.0),
                b'P' => self.rain_midnight_in = v.map(|x| x / 100.0),
                b'h' => self.humidity_pct = v.map(|x| if x == 0.0 { 100 } else { x as u8 }),
                b'b' => self.pressure_hpa = v.map(|x| x / 10.0),
                b'L' => self.luminosity_w_m2 = v.map(|x| x as u16),
                b'l' => self.luminosity_w_m2 = v.map(|x| x as u16 + 1000),
                _ => self.rain_raw = v.map(|x| x as u16),
            }
            i += 1 + n;
        }
        let rest = &b[i..];
        // The software type (one character) and the unit code (two to four) follow the values with no space.
        let end = rest
            .iter()
            .position(|&c| c == b' ')
            .unwrap_or(rest.len())
            .min(5);
        if end >= 1 && i > 0 {
            self.software = String::from_utf8_lossy(&rest[..end]).into_owned();
            return &rest[end..];
        }
        rest
    }
}
