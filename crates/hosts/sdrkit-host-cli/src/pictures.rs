//! Saving the pictures of a session as PNG files: each finished picture, and of the frames of a video one every few
//! seconds of signal.

use crate::error::CliError;
use crate::out::Out;
use sdrkit_codec_image::{png, Codec, ImageError, Picture, Raster};
use sdrkit_core::StreamKind;
use sdrkit_host_engine::{Engine, MediaRead, SessionId};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Writes the pictures of one session into a folder.
pub struct SavePictures {
    folder: PathBuf,
    every_s: f64,
    stack: String,
    /// The counters of the last picture and the last media packet read.
    picture_after: u64,
    media_after: u64,
    /// The signal time of the last picture saved, by picture name, and of the last video frame saved.
    last: Vec<(Option<Arc<str>>, f64)>,
    last_frame: Option<f64>,
    saved: u64,
    buf: Vec<u8>,
    /// A picture that could not be saved is told once.
    told: bool,
}

impl SavePictures {
    /// Saves into `folder`, which is made if it does not exist. Of pictures with the same name, and of video frames, one
    /// in each `every_s` seconds of signal is saved. `stack` starts each file name.
    ///
    /// # Errors
    ///
    /// [`CliError::Io`] if the folder cannot be made.
    pub fn new(folder: PathBuf, every_s: f64, stack: &str) -> Result<SavePictures, CliError> {
        std::fs::create_dir_all(&folder)?;
        Ok(SavePictures {
            folder,
            every_s: every_s.max(0.0),
            stack: stack.to_owned(),
            picture_after: 0,
            media_after: 0,
            last: Vec::new(),
            last_frame: None,
            saved: 0,
            buf: Vec::new(),
            told: false,
        })
    }

    /// Saves what came from `session` since the last call, and prints the path of each file.
    ///
    /// # Errors
    ///
    /// What the engine returns, and [`CliError::Io`] if a file cannot be written.
    pub fn pump(&mut self, engine: &Engine, session: SessionId, out: &mut Out) -> Result<(), CliError> {
        while let Some(r) = engine.picture(session, self.picture_after, &mut self.buf)? {
            self.picture_after = r.counter;
            let Some(tag) = &r.picture else { continue };
            if tag.state != sdrkit_core::PictureState::Finished || !self.due_picture(&tag.name, r.time) {
                continue;
            }
            let raster = picture_raster(&r, &self.buf);
            self.write(raster, out)?;
        }
        while let Some(r) = engine.media(session, self.media_after, &mut self.buf)? {
            self.media_after = r.counter;
            if r.kind != StreamKind::RawVideo || !due(self.last_frame, r.time, self.every_s) {
                continue;
            }
            self.last_frame = Some(r.time);
            let raster = picture_raster(&r, &self.buf);
            self.write(raster, out)?;
        }
        Ok(())
    }

    /// True if a finished picture named `name` at `time` is to be saved, and notes it.
    fn due_picture(&mut self, name: &Option<Arc<str>>, time: f64) -> bool {
        let at = self.last.iter().position(|(n, _)| n == name);
        if !due(at.map(|i| self.last[i].1), time, self.every_s) {
            return false;
        }
        match at {
            Some(i) => self.last[i].1 = time,
            None => self.last.push((name.clone(), time)),
        }
        true
    }

    fn write(&mut self, raster: Result<Raster, ImageError>, out: &mut Out) -> Result<(), CliError> {
        let bytes = match raster.and_then(|r| png::encode(&r)) {
            Ok(b) => b,
            Err(e) => {
                if !std::mem::replace(&mut self.told, true) {
                    out.note(&format!("a picture is not saved: {e}"))?;
                }
                return Ok(());
            }
        };
        self.saved += 1;
        let name = format!("{}_{}_{:04}.png", self.stack, utc_stamp(), self.saved);
        let path = self.folder.join(name);
        std::fs::write(&path, bytes)?;
        if out.json {
            out.json(&json!({ "saved": path }))
        } else {
            out.line(&format!("saved {}", path.display()))
        }
    }
}

/// True if a picture at `time` comes `every_s` seconds of signal or more after the one at `last`. A time that goes back
/// starts again.
fn due(last: Option<f64>, time: f64, every_s: f64) -> bool {
    last.is_none_or(|t| time < t || time - t >= every_s)
}

/// The pixels of the picture or frame `r` with the bytes `bytes`: raw pixels as they are, a compressed picture decoded.
fn picture_raster(r: &MediaRead, bytes: &[u8]) -> Result<Raster, ImageError> {
    match r.format.codec {
        Codec::Pixels(_) => Raster::from_picture(&Picture {
            codec: r.format.codec,
            layout: r.format.pixels.clone(),
            name: None,
            id: Picture::WHOLE,
            state: sdrkit_core::PictureState::Finished,
            data: Arc::from(bytes),
        }),
        codec => sdrkit_codec_image::decode(codec, bytes),
    }
}

/// The time of the clock in UTC, as `20261127-143005Z`.
fn utc_stamp() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, secs) = (s / 86_400, s % 86_400);
    let (y, m, d) = civil(i64::try_from(days).unwrap_or(0));
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// The year, month and day of the day `days` after 1970-01-01 in the Gregorian calendar.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_give_their_dates() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_419), (2025, 11, 27));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }

    #[test]
    fn of_pictures_with_one_name_one_in_each_interval_is_saved() {
        assert!(due(None, 3.0, 5.0));
        assert!(!due(Some(3.0), 7.9, 5.0));
        assert!(due(Some(3.0), 8.0, 5.0));
        // A new playback starts the signal time again.
        assert!(due(Some(30.0), 1.0, 5.0));
    }
}
