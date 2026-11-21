//! The SuperSID text data file, written through the host's storage: one file per UTC day, in `sid_format` (one file per
//! transmitter, a timestamp on each line) or `supersid_format` (one file for all transmitters, row n at 0:00:00 UTC +
//! n × LogInterval, no timestamp).
//!
//! The header lines are `# Key = Value` in the order Site, Contact, Longitude, Latitude, two bare `#`, UTC_Offset, TimeZone,
//! UTC_StartTime, LogInterval, LogType, MonitorID, then StationID and Frequency (sid_format) or Stations and Frequencies
//! (supersid_format). Contact states the unit of the values, which no document defines: the power in the transmitter's
//! signal band relative to full scale, linear (a full-scale tone is 1). A value of 0 is no reading.
//!
//! UTC_StartTime is 00:00:00 of the file's day in both forms. A `supersid_format` file that starts during the day holds rows
//! of 0 from 0:00:00 up to the first interval measured. A file of the same name that is already there is continued: a
//! `sid_format` file gets the new lines at its end; a `supersid_format` file with the same stations, frequencies and log interval
//! is padded with rows of 0 up to the present row, and one with others is left alone and a new file with the suffix `_2`,
//! `_3`, ... is written.

use crate::calendar;
use sdrkit_core::{Storage, StorageError, StorePath, WriteMode};
use sdrkit_util_text::csv;
use std::io::Write;

sdrkit_core::choice_enum! {
    /// Which file form is written.
    pub enum LogFormat {
        /// No file.
        Off = ("off", "Off"),
        /// One file per transmitter, a timestamp on each line.
        Sid = ("sid_format", "sid_format (one file per transmitter)"),
        /// One file for all transmitters, one column per transmitter.
        Supersid = ("supersid_format", "supersid_format (one file, all transmitters)"),
    }
}

/// The text of the Contact header line: the unit of the values.
pub const CONTACT: &str = "sdrkit; value: power in the signal band relative to full scale, linear (a full-scale tone \
                           is 1); 0: no reading";
/// The type of the values.
pub const LOG_TYPE: &str = "raw";

/// What the header says about the site.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Site {
    /// The site name.
    pub site: String,
    /// The monitor id.
    pub monitor_id: String,
    /// The receiver's longitude, degrees east.
    pub longitude: Option<f64>,
    /// The receiver's latitude, degrees north.
    pub latitude: Option<f64>,
    /// The offset of local time from UTC, s.
    pub utc_offset_s: i32,
    /// The IANA time zone name.
    pub time_zone: String,
    /// The log interval, s.
    pub interval_s: f64,
}

/// One logged transmitter.
#[derive(Clone, Debug, PartialEq)]
pub struct Logged {
    /// Its label.
    pub label: String,
    /// Its frequency, Hz.
    pub frequency_hz: f64,
}

/// The writer of the data files of one run.
#[derive(Debug)]
pub struct SidLog {
    format: LogFormat,
    folder: StorePath,
    site: Site,
    stations: Vec<Logged>,
    day: Option<i64>,
    paths: Vec<StorePath>,
    next_row: u64,
}

/// `YYYY-MM-DD HH:MM:SS` of whole Unix seconds.
pub fn timestamp(unix: i64) -> String {
    let c = calendar::civil(unix);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// The value as the file writes it: `%.15f`; `None` as 0.
pub fn value_text(v: Option<f64>) -> String {
    format!("{:.15}", v.filter(|x| x.is_finite()).unwrap_or(0.0))
}

/// `name` with every character other than an ASCII letter, a digit, `-`, `_` and `.` replaced by `_`.
fn file_part(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `+HH:MM` or `-HH:MM` of an offset in seconds.
fn offset_text(s: i32) -> String {
    let sign = if s < 0 { '-' } else { '+' };
    let a = s.unsigned_abs();
    format!("{sign}{:02}:{:02}", a / 3600, a / 60 % 60)
}

impl SidLog {
    /// A writer of `format` files into `folder` for `stations` (in column order).
    pub fn new(format: LogFormat, folder: StorePath, site: Site, stations: Vec<Logged>) -> Self {
        SidLog {
            format,
            folder,
            site,
            stations,
            day: None,
            paths: Vec::new(),
            next_row: 0,
        }
    }

    /// The form written.
    pub fn format(&self) -> LogFormat {
        self.format
    }

    /// The files of the present day (empty before the first row).
    pub fn paths(&self) -> &[StorePath] {
        &self.paths
    }

    /// The header of the file of the day that starts at `day_start`: of station `station` (sid_format) or of all
    /// (supersid_format, `None`).
    pub fn header(&self, station: Option<usize>, day_start: i64) -> String {
        let s = &self.site;
        let num = |v: Option<f64>| v.map(csv::number).unwrap_or_default();
        let mut lines = vec![
            format!("# Site = {}", s.site),
            format!("# Contact = {CONTACT}"),
            format!("# Longitude = {}", num(s.longitude)),
            format!("# Latitude = {}", num(s.latitude)),
            "#".into(),
            "#".into(),
            format!("# UTC_Offset = {}", offset_text(s.utc_offset_s)),
            format!("# TimeZone = {}", s.time_zone),
            format!("# UTC_StartTime = {}", timestamp(day_start)),
            format!("# LogInterval = {}", csv::number(s.interval_s)),
            format!("# LogType = {LOG_TYPE}"),
            format!("# MonitorID = {}", s.monitor_id),
        ];
        match station.and_then(|i| self.stations.get(i)) {
            Some(st) => {
                lines.push(format!("# StationID = {}", st.label));
                lines.push(format!("# Frequency = {}", csv::number(st.frequency_hz)));
            }
            None => {
                let labels: Vec<&str> = self.stations.iter().map(|s| s.label.as_str()).collect();
                let freqs: Vec<String> = self
                    .stations
                    .iter()
                    .map(|s| csv::number(s.frequency_hz))
                    .collect();
                lines.push(format!("# Stations = {}", labels.join(", ")));
                lines.push(format!("# Frequencies = {}", freqs.join(", ")));
            }
        }
        let mut out = String::new();
        for l in lines {
            out.push_str(l.trim_end());
            out.push('\n');
        }
        out
    }

    /// The file names of the day that starts at `day_start`: one per station (sid_format) or one (supersid_format). A label
    /// that appears twice in the list gets its frequency in the name.
    pub fn file_names(&self, day_start: i64) -> Vec<String> {
        let date = &timestamp(day_start)[..10];
        let prefix = if self.site.site.is_empty() {
            String::new()
        } else {
            format!("{}_", file_part(&self.site.site))
        };
        match self.format {
            LogFormat::Off => Vec::new(),
            LogFormat::Sid => self
                .stations
                .iter()
                .map(|s| {
                    let twice = self.stations.iter().filter(|o| o.label == s.label).count() > 1;
                    let freq = if twice {
                        format!("_{}", csv::number(s.frequency_hz))
                    } else {
                        String::new()
                    };
                    format!("{prefix}{}{freq}_{date}.csv", file_part(&s.label))
                })
                .collect(),
            LogFormat::Supersid => {
                let p = if prefix.is_empty() {
                    "supersid_".to_string()
                } else {
                    prefix
                };
                vec![format!("{p}{date}.csv")]
            }
        }
    }

    /// Writes the values of the interval that starts at `utc` (Unix s), one per station in list order (`None`: no
    /// reading). Opens the files of a new UTC day first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] from the storage, or for a file name that is not a valid path.
    pub fn write(
        &mut self,
        storage: &dyn Storage,
        utc: f64,
        values: &[Option<f64>],
    ) -> Result<(), StorageError> {
        if self.format == LogFormat::Off || self.stations.is_empty() {
            return Ok(());
        }
        let whole = utc.floor() as i64;
        let day = whole.div_euclid(86_400) * 86_400;
        if self.day != Some(day) {
            self.open_day(storage, day)?;
        }
        match self.format {
            LogFormat::Off => {}
            LogFormat::Sid => {
                let ts = timestamp(whole);
                for (i, p) in self.paths.iter().enumerate() {
                    let line = format!("{ts}, {}\n", value_text(values.get(i).copied().flatten()));
                    append(storage, p, &line)?;
                }
            }
            LogFormat::Supersid => {
                let row = ((utc - day as f64) / self.site.interval_s.max(1e-3) + 1e-6).floor() as u64;
                if row < self.next_row {
                    return Ok(());
                }
                let mut text = String::new();
                for _ in self.next_row..row {
                    text.push_str(&self.row_text(&[]));
                }
                text.push_str(&self.row_text(values));
                if let Some(p) = self.paths.first() {
                    append(storage, p, &text)?;
                }
                self.next_row = row + 1;
            }
        }
        Ok(())
    }

    fn row_text(&self, values: &[Option<f64>]) -> String {
        let cells: Vec<String> = (0..self.stations.len())
            .map(|i| value_text(values.get(i).copied().flatten()))
            .collect();
        format!("{}\n", cells.join(", "))
    }

    fn open_day(&mut self, storage: &dyn Storage, day: i64) -> Result<(), StorageError> {
        self.day = Some(day);
        self.paths.clear();
        self.next_row = 0;
        let names = self.file_names(day);
        match self.format {
            LogFormat::Off => {}
            LogFormat::Sid => {
                for (i, name) in names.iter().enumerate() {
                    let p = self.folder.join(name)?;
                    if storage.stat(&p).is_err() {
                        storage.write(&p, self.header(Some(i), day).as_bytes())?;
                    }
                    self.paths.push(p);
                }
            }
            LogFormat::Supersid => {
                let header = self.header(None, day);
                let kept = |l: &&str| {
                    l.starts_with("# Stations")
                        || l.starts_with("# Frequencies")
                        || l.starts_with("# LogInterval")
                };
                let lists: Vec<&str> = header.lines().filter(kept).collect();
                let stem = names[0].trim_end_matches(".csv").to_string();
                for k in 1u32.. {
                    let name = if k == 1 {
                        format!("{stem}.csv")
                    } else {
                        format!("{stem}_{k}.csv")
                    };
                    let p = self.folder.join(&name)?;
                    match storage.read(&p) {
                        Err(StorageError::NotFound) => {
                            storage.write(&p, header.as_bytes())?;
                            self.paths.push(p);
                            break;
                        }
                        Err(e) => return Err(e),
                        Ok(old) => {
                            let old = String::from_utf8_lossy(&old);
                            let same = old.lines().filter(kept).eq(lists.iter().copied());
                            if same {
                                self.next_row = old
                                    .lines()
                                    .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                                    .count() as u64;
                                self.paths.push(p);
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Appends `text` to the file `p`.
fn append(storage: &dyn Storage, p: &StorePath, text: &str) -> Result<(), StorageError> {
    let mut w = storage.open_writer(p, WriteMode::Append)?;
    w.write_all(text.as_bytes())?;
    w.flush()?;
    Ok(())
}
