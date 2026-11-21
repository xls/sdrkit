//! The e-CALLISTO spectrogram file of `callisto`: a FITS file with the spectrum image in the primary HDU and one binary table
//! extension with the columns `TIME` and `FREQUENCY`.
//!
//! Documents: ETH CALLISTO Operating Manual, chapter 11 "Description of FITS-file header" (keywords and their order; `BITPIX =
//! 16` with values 0 to 255 digits, `BZERO = 0.`, `BSCALE = 1.`, `BUNIT = 'digits'`; `DATE-OBS` as `yyyy/mm/dd`); the file name
//! `<station>_<yyyymmdd>_<hhmmss>_<ff>.fit` of the manual and the e-CALLISTO archive; the Python reader of e-CALLISTO files in
//! the suncasa package (one binary table, `TIME` in seconds of the day and `FREQUENCY` in MHz, from its first row). The manual
//! describes two binary tables; the readers in use take one with both columns, which is what is written. Axis 1 is time
//! (`NAXIS1` samples), axis 2 the channels, from the highest frequency down (`CDELT2` negative, as in the manual's example).

use crate::calendar;
use sdrkit_codec_fits::{self as fits, Card, Column, ColumnType, Hdu};

sdrkit_core::choice_enum! {
    /// The pixel type of the spectrum image.
    pub enum Bitpix {
        /// 16-bit integers holding 0 to 255, as the manual's example header.
        Sixteen = ("16", "16 bits (the manual's example)"),
        /// 8-bit bytes.
        Eight = ("8", "8 bits"),
    }
}

impl Bitpix {
    /// The `BITPIX` value.
    pub fn value(self) -> i32 {
        match self {
            Bitpix::Sixteen => 16,
            Bitpix::Eight => 8,
        }
    }
}

/// How power maps to digits and what the header names.
#[derive(Clone, Debug, PartialEq)]
pub struct FileSpec {
    /// The instrument code: letters and digits.
    pub station: String,
    /// The two-digit focus code, 0 to 63.
    pub focus: u8,
    /// The pixel type.
    pub bitpix: Bitpix,
    /// dB per digit.
    pub db_per_digit: f64,
    /// The power of digit 0, dB of full scale.
    pub zero_dbfs: f64,
    /// Latitude (degrees, north positive), longitude (degrees, east positive) and height (m) of the antenna.
    pub position: Option<(f64, f64, f64)>,
}

impl FileSpec {
    /// The digit of a power (linear, full scale 1): (dB - zero) / step, rounded and clamped to 0 to 255.
    pub fn digit(&self, power: f32) -> f64 {
        let db = 10.0 * f64::from(power).max(1e-30).log10();
        ((db - self.zero_dbfs) / self.db_per_digit.max(1e-6))
            .round()
            .clamp(0.0, 255.0)
    }

    /// The file name of a file that starts at `start_utc` (Unix s): `<station>_<yyyymmdd>_<hhmmss>_<ff>.fit`.
    pub fn name(&self, start_utc: f64) -> String {
        let c = calendar::civil(start_utc.floor() as i64);
        format!(
            "{}_{:04}{:02}{:02}_{:02}{:02}{:02}_{:02}.fit",
            self.station, c.year, c.month, c.day, c.hour, c.minute, c.second, self.focus
        )
    }
}

/// `yyyy/mm/dd` and `hh:mm:ss.sss` of a Unix time.
fn date_time(utc: f64) -> (String, String, String) {
    let whole = utc.floor();
    let c = calendar::civil(whole as i64);
    let ms = ((utc - whole) * 1000.0).round().min(999.0) as u32;
    (
        format!("{:04}/{:02}/{:02}", c.year, c.month, c.day),
        format!("{:04}-{:02}-{:02}", c.year, c.month, c.day),
        format!("{:02}:{:02}:{:02}.{ms:03}", c.hour, c.minute, c.second),
    )
}

/// The bytes of one file: `rows` (power per channel, increasing frequency) of `row_s` seconds from `start_utc`, the channel
/// centres `freqs_hz` (increasing).
pub fn file_bytes(
    spec: &FileSpec,
    start_utc: f64,
    row_s: f64,
    freqs_hz: &[f64],
    rows: &[Vec<f32>],
) -> Vec<u8> {
    let nt = rows.len();
    let nf = freqs_hz.len();
    let mut pixels = vec![0.0; nt * nf];
    for (i, row) in rows.iter().enumerate() {
        for j in 0..nf {
            // Image row 0 is the highest frequency.
            pixels[j * nt + i] = spec.digit(row.get(nf - 1 - j).copied().unwrap_or(0.0));
        }
    }
    let (dmin, dmax) = pixels
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    let (dmin, dmax) = if nt * nf == 0 { (0.0, 0.0) } else { (dmin, dmax) };
    let end_utc = start_utc + nt as f64 * row_s;
    let (date_obs, date_iso, time_obs) = date_time(start_utc);
    let (date_end, _, time_end) = date_time(end_utc);
    let day0 = (start_utc / 86_400.0).floor() * 86_400.0;
    let sod = start_utc - day0;
    let top_mhz = freqs_hz.last().copied().unwrap_or(0.0) / 1e6;
    let step_mhz = if nf > 1 {
        (freqs_hz[nf - 1] - freqs_hz[0]) / (nf - 1) as f64 / 1e6
    } else {
        0.0
    };
    let (lat, lon, alt) = spec.position.unwrap_or((0.0, 0.0, 0.0));
    let mut p = Hdu::image(spec.bitpix.value(), vec![nt, nf], pixels);
    p.cards = vec![
        Card::comment("Warning: the value of CDELT1 may be rounded!"),
        Card::comment("Warning: the frequency axis may not be regular!"),
        Card::comment("Warning: the value of CDELT2 may be rounded!"),
        Card::comment(""),
        Card::value("DATE", date_iso.as_str()),
        Card::value(
            "CONTENT",
            format!("{date_obs} Radio flux density ({})", spec.station),
        ),
        Card::value("ORIGIN", spec.station.as_str()),
        Card::value("TELESCOP", "Radio Spectrometer"),
        Card::value("INSTRUME", spec.station.as_str()),
        Card::value("OBJECT", "Sun"),
        Card::value("DATE-OBS", date_obs.as_str()),
        Card::value("TIME-OBS", time_obs.as_str()),
        Card::value("DATE-END", date_end.as_str()),
        Card::value("TIME-END", time_end.as_str()),
        Card::value("BZERO", 0.0),
        Card::value("BSCALE", 1.0),
        Card::value("BUNIT", "digits"),
        Card::value("DATAMIN", dmin as i64),
        Card::value("DATAMAX", dmax as i64),
        Card::value("CRVAL1", sod),
        Card::value("CRPIX1", 0i64),
        Card::value("CTYPE1", "Time [UT]"),
        Card::value("CDELT1", row_s),
        Card::value("CRVAL2", top_mhz),
        Card::value("CRPIX2", 0i64),
        Card::value("CTYPE2", "Frequency [MHz]"),
        Card::value("CDELT2", -step_mhz),
        Card::history("Spectra of one capture without a frequency sweep"),
        Card::value("OBS_LAT", lat.abs()),
        Card::value("OBS_LAC", if lat < 0.0 { "S" } else { "N" }),
        Card::value("OBS_LON", lon.abs()),
        Card::value("OBS_LOC", if lon < 0.0 { "W" } else { "E" }),
        Card::value("OBS_ALT", alt),
        Card::value("FRQFILE", "NONE"),
        Card::value("PWM_VAL", 0i64),
    ];
    let table = Hdu::table(
        1,
        vec![
            Column {
                name: "TIME".into(),
                unit: String::new(),
                kind: ColumnType::F64,
                repeat: nt,
                values: (0..nt).map(|i| sod + i as f64 * row_s).collect(),
            },
            Column {
                name: "FREQUENCY".into(),
                unit: String::new(),
                kind: ColumnType::F64,
                repeat: nf,
                values: freqs_hz.iter().rev().map(|f| f / 1e6).collect(),
            },
        ],
    );
    fits::write(&[p, table])
}

/// What a reader takes from a file: the time of each column (seconds of the day), the frequency of each channel (MHz, as
/// stored), the digits (channel-major, `NAXIS1` time samples a channel) and `DATE-OBS` with `-` for `/`. One table with both
/// columns, or two tables with one each, are accepted; `BITPIX` 8 or 16.
pub type Read = (Vec<f64>, Vec<f64>, Vec<f64>, String);

/// Reads a file written by [`file_bytes`] or by a CALLISTO station.
///
/// # Errors
///
/// A text that says what is missing.
pub fn read_file(bytes: &[u8]) -> Result<Read, String> {
    let hdus = fits::read(bytes).map_err(|e| e.to_string())?;
    let p = hdus.first().ok_or("no HDU")?;
    let fits::Data::Image {
        bitpix, pixels, ..
    } = &p.data
    else {
        return Err("no image".into());
    };
    if ![8, 16].contains(bitpix) {
        return Err(format!("BITPIX {bitpix}"));
    }
    let column = |name: &str| {
        hdus[1..]
            .iter()
            .find_map(|h| h.column(name))
            .map(|c| c.values.clone())
            .ok_or(format!("no column {name}"))
    };
    let date = p.str("DATE-OBS").ok_or("no DATE-OBS")?.replace('/', "-");
    Ok((column("TIME")?, column("FREQUENCY")?, pixels.clone(), date))
}
