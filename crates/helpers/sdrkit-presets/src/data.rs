//! The data folder: its plan files, read when the folder opens, and its CSV lists, read when they are asked for.

use crate::error::DataError;
use crate::plans::PlanSet;
use crate::region::Region;
use sdrkit_util_text::csv;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The services that the folder lists known stations of. The key is the folder name under `stations/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StationKind {
    /// FM broadcast (`fm`).
    Fm,
    /// DAB and DAB+ (`dab`).
    Dab,
    /// DVB-T and DVB-T2 (`dvb-t`).
    DvbT,
    /// ATSC 1.0 (`atsc`).
    Atsc,
    /// ATSC 3.0 (`atsc3`).
    Atsc3,
    /// ISDB-T (`isdb-t`).
    IsdbT,
    /// DTMB (`dtmb`).
    Dtmb,
}

impl StationKind {
    /// Every kind.
    pub const ALL: [StationKind; 7] = [
        StationKind::Fm,
        StationKind::Dab,
        StationKind::DvbT,
        StationKind::Atsc,
        StationKind::Atsc3,
        StationKind::IsdbT,
        StationKind::Dtmb,
    ];

    /// The folder name of the kind.
    pub fn key(self) -> &'static str {
        match self {
            StationKind::Fm => "fm",
            StationKind::Dab => "dab",
            StationKind::DvbT => "dvb-t",
            StationKind::Atsc => "atsc",
            StationKind::Atsc3 => "atsc3",
            StationKind::IsdbT => "isdb-t",
            StationKind::Dtmb => "dtmb",
        }
    }

    /// The kind with the folder name `key`.
    pub fn from_key(key: &str) -> Option<StationKind> {
        StationKind::ALL.into_iter().find(|k| k.key() == key)
    }

    /// The kind whose stations the stack `stack` receives. `None` for a stack that the folder lists no stations of.
    pub fn of_stack(stack: &str) -> Option<StationKind> {
        crate::preset::recipe(stack).and_then(|r| r.stations)
    }
}

/// One row of `stations/<kind>/<ISO2>.csv`: a transmission that was received in a city.
#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    /// The city where it was received.
    pub city: String,
    /// The centre frequency, Hz.
    pub frequency_hz: f64,
    /// The channel width, Hz, if the row has one.
    pub width_hz: Option<f64>,
    /// The standard as the row writes it (`DVB-T2`), or empty.
    pub standard: String,
    /// The network or multiplex name.
    pub network: String,
    /// The names of the services.
    pub services: Vec<String>,
    /// The signal to noise ratio of the best report, dB.
    pub snr_db: Option<f64>,
    /// The count of reports.
    pub reports: u32,
    /// The date of the first report.
    pub first_seen: String,
    /// The date of the last report.
    pub last_seen: String,
}

/// One row of `cities/<ISO2>.csv`.
#[derive(Clone, Debug, PartialEq)]
pub struct City {
    /// The name.
    pub name: String,
    /// The first-level administrative division (state, province).
    pub admin1: String,
    /// Latitude, degrees north.
    pub lat: f64,
    /// Longitude, degrees east.
    pub lon: f64,
    /// The population.
    pub population: u64,
}

/// One row of `dab-tii/<ISO2>.csv`: a DAB transmitter with its identification codes.
#[derive(Clone, Debug, PartialEq)]
pub struct Transmitter {
    /// The ensemble id, as the row writes it (hexadecimal).
    pub eid: String,
    /// The main identifier.
    pub main: u32,
    /// The sub identifier.
    pub sub: u32,
    /// Latitude, degrees north.
    pub lat: f64,
    /// Longitude, degrees east.
    pub lon: f64,
    /// The name of the site.
    pub site: String,
    /// The radiated power, kW.
    pub power_kw: Option<f64>,
    /// The centre frequency of the channel, Hz.
    pub frequency_hz: f64,
    /// The count of reports.
    pub reports: u32,
    /// The date of the first report.
    pub first_seen: String,
    /// The date of the last report.
    pub last_seen: String,
}

/// One row of `countries.csv`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Country {
    /// The ISO 3166 alpha-2 code.
    pub iso2: String,
    /// The name.
    pub name: String,
}

/// An open data folder.
#[derive(Debug)]
pub struct Data {
    folder: PathBuf,
    pub(crate) plans: PlanSet,
    /// Every plan of the folder and of the user's file, by id.
    pub(crate) store: sdrkit_data_plans::Plans,
    /// The file of the user's plans, if the host set one.
    pub(crate) personal: Option<PathBuf>,
    newest: Option<SystemTime>,
}

/// The text of the file `path`.
pub(crate) fn read_text(path: &Path) -> Result<String, DataError> {
    std::fs::read_to_string(path).map_err(|e| io_error(path, e))
}

/// `e` as the error of `path`: a path that does not exist is missing data.
pub(crate) fn io_error(path: &Path, e: std::io::Error) -> DataError {
    if e.kind() == std::io::ErrorKind::NotFound {
        DataError::Missing {
            path: path.to_owned(),
        }
    } else {
        DataError::Read {
            path: path.to_owned(),
            source: e,
        }
    }
}

/// The newest modification time of the files under `dir`, down to `depth` folders.
fn newest_in(dir: &Path, depth: u32) -> Option<SystemTime> {
    let mut newest = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let t = if meta.is_dir() {
            if depth == 0 {
                continue;
            }
            newest_in(&entry.path(), depth - 1)
        } else {
            meta.modified().ok()
        };
        newest = newest.max(t);
    }
    newest
}

/// A CSV file with a header line.
pub(crate) struct Table {
    pub(crate) path: PathBuf,
    header: Vec<String>,
    pub(crate) rows: Vec<(usize, Vec<String>)>,
}

impl Table {
    /// Reads `path`. The first record names the columns, and every column of `columns` must be there.
    pub(crate) fn read(path: PathBuf, columns: &[&str]) -> Result<Table, DataError> {
        let text = read_text(&path)?;
        let mut rows = csv::read(&text, ',');
        if rows.is_empty() {
            return Err(DataError::Format {
                path,
                reason: "no header line".into(),
            });
        }
        let header: Vec<String> = rows.remove(0).1;
        for c in columns {
            if csv::column(&header, &[c]).is_none() {
                return Err(DataError::Format {
                    path,
                    reason: format!("no column `{c}`"),
                });
            }
        }
        Ok(Table { path, header, rows })
    }

    /// The field of column `name` in `row`, without outer spaces. Empty if the row is short.
    pub(crate) fn text<'a>(&self, row: &'a [String], name: &str) -> &'a str {
        csv::column(&self.header, &[name])
            .and_then(|c| row.get(c))
            .map_or("", |s| s.trim())
    }

    /// The number of column `name` in `row`. `None` for an empty field.
    pub(crate) fn number(&self, line: usize, row: &[String], name: &str) -> Result<Option<f64>, DataError> {
        let t = self.text(row, name);
        if t.is_empty() {
            return Ok(None);
        }
        match t.parse::<f64>() {
            Ok(v) if v.is_finite() => Ok(Some(v)),
            _ => Err(DataError::Format {
                path: self.path.clone(),
                reason: format!("line {line}: `{t}` in the column `{name}` is not a number"),
            }),
        }
    }

    /// The number of column `name` in `row`, which must not be empty.
    pub(crate) fn required(&self, line: usize, row: &[String], name: &str) -> Result<f64, DataError> {
        self.number(line, row, name)?
            .ok_or_else(|| DataError::Format {
                path: self.path.clone(),
                reason: format!("line {line}: the column `{name}` is empty"),
            })
    }

    /// The count of column `name` in `row`. 0 for an empty field.
    pub(crate) fn count(&self, line: usize, row: &[String], name: &str) -> Result<u64, DataError> {
        let v = self.number(line, row, name)?.unwrap_or(0.0);
        if v < 0.0 || v.fract() != 0.0 {
            return Err(DataError::Format {
                path: self.path.clone(),
                reason: format!("line {line}: the column `{name}` is not a count"),
            });
        }
        Ok(v as u64)
    }
}

/// `n` as a `u32`, the largest one if it does not fit.
fn clamp_u32(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Data {
    /// Opens the data folder `folder` and reads its plan files.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`] if the folder or its `plans` folder does not exist, [`DataError::Read`],
    /// [`DataError::Format`] for a plan file that does not have its format.
    pub fn open(folder: impl AsRef<Path>) -> Result<Data, DataError> {
        let folder = folder.as_ref().to_owned();
        std::fs::read_dir(&folder).map_err(|e| io_error(&folder, e))?;
        let plans = PlanSet::load(&folder.join("plans"))?;
        let store = crate::lookup::load_store(&folder.join("plans"))?;
        let newest = newest_in(&folder, 3);
        Ok(Data {
            folder,
            plans,
            store,
            personal: None,
            newest,
        })
    }

    /// The folder.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The modification time of the newest file of the folder, as it was when the folder opened.
    pub fn newest(&self) -> Option<SystemTime> {
        self.newest
    }

    /// The age of the data: the time since [`Data::newest`]. `None` for a folder without files or a time in the future.
    pub fn age(&self) -> Option<Duration> {
        SystemTime::now().duration_since(self.newest?).ok()
    }

    /// The countries of `countries.csv`.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`] if the file does not exist, [`DataError::Read`], [`DataError::Format`].
    pub fn countries(&self) -> Result<Vec<Country>, DataError> {
        let t = Table::read(self.folder.join("countries.csv"), &["iso2", "name"])?;
        Ok(t.rows
            .iter()
            .map(|(_, r)| Country {
                iso2: t.text(r, "iso2").to_ascii_uppercase(),
                name: t.text(r, "name").to_owned(),
            })
            .collect())
    }

    /// The cities of `cities/<ISO2>.csv` for the country of `region`.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`] if the file does not exist, [`DataError::Read`], [`DataError::Format`].
    pub fn cities(&self, region: &Region) -> Result<Vec<City>, DataError> {
        let path = self.country_file("cities", region);
        let t = Table::read(path, &["name", "admin1", "lat", "lon", "population"])?;
        t.rows
            .iter()
            .map(|(line, r)| {
                Ok(City {
                    name: t.text(r, "name").to_owned(),
                    admin1: t.text(r, "admin1").to_owned(),
                    lat: t.required(*line, r, "lat")?,
                    lon: t.required(*line, r, "lon")?,
                    population: t.count(*line, r, "population")?,
                })
            })
            .collect()
    }

    /// The DAB transmitters of `dab-tii/<ISO2>.csv` for the country of `region`.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`] if the file does not exist, [`DataError::Read`], [`DataError::Format`].
    pub fn dab_transmitters(&self, region: &Region) -> Result<Vec<Transmitter>, DataError> {
        let path = self.country_file("dab-tii", region);
        let columns = [
            "eid",
            "main",
            "sub",
            "lat",
            "lon",
            "site",
            "power_kw",
            "channel_mhz",
            "reports",
            "first_seen",
            "last_seen",
        ];
        let t = Table::read(path, &columns)?;
        t.rows
            .iter()
            .map(|(line, r)| {
                Ok(Transmitter {
                    eid: t.text(r, "eid").to_owned(),
                    main: clamp_u32(t.count(*line, r, "main")?),
                    sub: clamp_u32(t.count(*line, r, "sub")?),
                    lat: t.required(*line, r, "lat")?,
                    lon: t.required(*line, r, "lon")?,
                    site: t.text(r, "site").to_owned(),
                    power_kw: t.number(*line, r, "power_kw")?,
                    frequency_hz: t.required(*line, r, "channel_mhz")? * 1e6,
                    reports: clamp_u32(t.count(*line, r, "reports")?),
                    first_seen: t.text(r, "first_seen").to_owned(),
                    last_seen: t.text(r, "last_seen").to_owned(),
                })
            })
            .collect()
    }

    /// The known stations of `kind` in the country of `region`, from `stations/<kind>/<ISO2>.csv`: the stations of the
    /// region's city first, each part in the order of the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`] if the file does not exist, [`DataError::Read`], [`DataError::Format`].
    pub fn known_stations(
        &self,
        kind: StationKind,
        region: &Region,
    ) -> Result<Vec<Station>, DataError> {
        let path = self
            .folder
            .join("stations")
            .join(kind.key())
            .join(format!("{}.csv", region.country()));
        let columns = [
            "city",
            "freq_mhz",
            "bw_mhz",
            "standard",
            "network",
            "services",
            "snr_db",
            "reports",
            "first_seen",
            "last_seen",
        ];
        let t = Table::read(path, &columns)?;
        let mut rows = t
            .rows
            .iter()
            .map(|(line, r)| {
                Ok(Station {
                    city: t.text(r, "city").to_owned(),
                    frequency_hz: t.required(*line, r, "freq_mhz")? * 1e6,
                    width_hz: t
                        .number(*line, r, "bw_mhz")?
                        .filter(|w| *w > 0.0)
                        .map(|w| w * 1e6),
                    standard: t.text(r, "standard").to_owned(),
                    network: t.text(r, "network").to_owned(),
                    services: t
                        .text(r, "services")
                        .split('|')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect(),
                    snr_db: t.number(*line, r, "snr_db")?,
                    reports: clamp_u32(t.count(*line, r, "reports")?),
                    first_seen: t.text(r, "first_seen").to_owned(),
                    last_seen: t.text(r, "last_seen").to_owned(),
                })
            })
            .collect::<Result<Vec<Station>, DataError>>()?;
        // A stable sort: the city's rows first, the order of the file otherwise.
        rows.sort_by_key(|s| !region.is_city(&s.city));
        Ok(rows)
    }

    /// `<folder>/<part>/<ISO2>.csv` for the country of `region`.
    fn country_file(&self, part: &str, region: &Region) -> PathBuf {
        self.folder
            .join(part)
            .join(format!("{}.csv", region.country()))
    }
}
