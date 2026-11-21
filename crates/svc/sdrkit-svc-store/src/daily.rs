//! Decoded messages saved to files: every observation is appended, with no rate limit, to
//! `<folder>/<YYYY-MM-DD>/<stack>.<ext>` as JSON lines, a JSON list or CSV. The date is the host's local date.

use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::sync::Arc;

use sdrkit_core::storage::{Storage, StoreWriter, WriteMode};
use sdrkit_core::StorePath;
use sdrkit_svc_events::plain::PlainObservation;
use sdrkit_svc_events::Observation;

use crate::time::{date, iso};

use crate::error::StoreError;
use crate::export::csv_cell;

sdrkit_core::choice_enum! {
    /// The file format of the daily files.
    pub enum DailyFormat {
        /// One JSON object per line.
        JsonLines = ("jsonl", "JSON lines"),
        /// One JSON list per file, kept valid after every message.
        Json = ("json", "JSON"),
        /// CSV with fixed columns and the event's fields as one JSON column.
        Csv = ("csv", "CSV"),
    }
}

impl DailyFormat {
    fn extension(self) -> &'static str {
        match self {
            DailyFormat::JsonLines => "jsonl",
            DailyFormat::Json => "json",
            DailyFormat::Csv => "csv",
        }
    }
}

const CSV_HEADER: &str = "time,kind,identity,freq_hz,lat,lon,distance_km,text,fields\r\n";

/// The daily files writer.
pub struct DailyFiles {
    storage: Arc<dyn Storage>,
    folder: StorePath,
    format: DailyFormat,
    offset_min: i32,
    open: HashMap<String, (StorePath, Box<dyn StoreWriter>)>,
    written: u64,
}

impl DailyFiles {
    /// Files under `folder` of `storage`, dated with the host's offset from UTC, minutes.
    pub fn new(
        storage: Arc<dyn Storage>,
        folder: StorePath,
        format: DailyFormat,
        offset_min: i32,
    ) -> Self {
        DailyFiles {
            storage,
            folder,
            format,
            offset_min,
            open: HashMap::new(),
            written: 0,
        }
    }

    /// The file that a message of event kind `kind` at `unix_time` goes to: the part of the kind key before the first dot
    /// names the file.
    ///
    /// # Errors
    ///
    /// [`StoreError::Storage`] if the folder and the name do not make a path.
    pub fn path_for(&self, kind: &str, unix_time: f64) -> Result<StorePath, StoreError> {
        let stack = kind.split('.').next().unwrap_or(kind);
        let safe: String = stack
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let day = date(unix_time, self.offset_min).ok_or(StoreError::Time { unix_time })?;
        let rest = format!("{day}/{safe}.{}", self.format.extension());
        self.folder.join(&rest).map_err(|e| StoreError::Storage {
            path: self.folder.clone(),
            source: sdrkit_core::StorageError::InvalidPath(e),
        })
    }

    /// Appends one observation and returns the file it went to.
    ///
    /// # Errors
    ///
    /// [`StoreError::Storage`] if the file cannot be opened or written. [`StoreError::Json`] if the line cannot be made.
    pub fn append(&mut self, o: &Observation<'_>) -> Result<StorePath, StoreError> {
        let path = self.path_for(o.event.kind(), o.unix_time)?;
        let fail = |source| StoreError::Storage {
            path: path.clone(),
            source,
        };
        let reopen = self
            .open
            .get(o.event.kind())
            .is_none_or(|(p, _)| *p != path);
        if reopen {
            let w = self
                .storage
                .open_writer(&path, WriteMode::Update)
                .map_err(fail)?;
            self.open
                .insert(o.event.kind().to_owned(), (path.clone(), w));
        }
        let Some((_, f)) = self.open.get_mut(o.event.kind()) else {
            return Ok(path);
        };
        let io = |e: std::io::Error| StoreError::Storage {
            path: path.clone(),
            source: e.into(),
        };
        let len = f.seek(SeekFrom::End(0)).map_err(io)?;
        match self.format {
            DailyFormat::JsonLines => {
                let mut l = serde_json::to_vec(&PlainObservation::from(o))?;
                l.push(b'\n');
                f.write_all(&l).map_err(io)?;
            }
            DailyFormat::Csv => {
                let mut s = String::new();
                if len == 0 {
                    s.push_str(CSV_HEADER);
                }
                let opt = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_default();
                let row = [
                    iso(o.unix_time),
                    csv_cell(o.event.kind()),
                    csv_cell(o.identity.unwrap_or("")),
                    o.event.freq_hz.to_string(),
                    opt(o.position.map(|p| p.lat_deg)),
                    opt(o.position.map(|p| p.lon_deg)),
                    opt(o.distance_km),
                    csv_cell(o.text.unwrap_or("")),
                    csv_cell(&serde_json::to_string(&PlainObservation::from(o).fields)?),
                ];
                s.push_str(&row.join(","));
                s.push_str("\r\n");
                f.write_all(s.as_bytes()).map_err(io)?;
            }
            DailyFormat::Json => {
                let obj = serde_json::to_string(&PlainObservation::from(o))?;
                if len < 4 {
                    f.set_len(0).map_err(fail)?;
                    f.seek(SeekFrom::Start(0)).map_err(io)?;
                    f.write_all(format!("[\n{obj}\n]\n").as_bytes())
                        .map_err(io)?;
                } else {
                    // The file ends "\n]\n": write over the closing bracket.
                    f.seek(SeekFrom::Start(len - 2)).map_err(io)?;
                    f.write_all(format!(",\n{obj}\n]\n").as_bytes())
                        .map_err(io)?;
                }
            }
        }
        f.flush().map_err(io)?;
        self.written += 1;
        Ok(path)
    }

    /// The number of messages written since the writer was made.
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Closes every open file. They open again on the next message.
    pub fn close(&mut self) {
        self.open.clear();
    }
}
