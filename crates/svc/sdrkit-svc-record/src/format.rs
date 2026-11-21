//! The containers and sample types of recordings, the file names, and the reader that returns the format of a recording file.
//!
//! Standard: SigMF specification v1.0.0, global object (core:datatype, core:sample_rate) and captures (core:frequency).

use crate::error::{FormatError, RecordError, SigmfField};
use crate::{sigmf, wav};
use sdrkit_core::{Complex32, Hz, Storage, StorePath};
use sdrkit_graph::stream::{Format, SampleType};
use sdrkit_util_geo::time::{iso8601, Civil};
use std::io::Read;

sdrkit_core::choice_enum! {
    /// The container of a recording file.
    pub enum Container {
        /// WAV: 16-bit integer or 32-bit float samples, interleaved.
        Wav = ("wav", "WAV"),
        /// FLAC: one channel of 16-bit samples.
        Flac = ("flac", "FLAC"),
        /// SigMF: a `.sigmf-data` file of complex samples and its `.sigmf-meta` metadata.
        Sigmf = ("sigmf", "SigMF"),
        /// Complex samples with no header. The name holds the rate and the centre.
        Raw = ("raw", "Raw"),
        /// An MPEG transport stream of 188-byte packets.
        Ts = ("ts", "Transport stream"),
    }
}

sdrkit_core::choice_enum! {
    /// The container of an audio recording.
    pub enum AudioContainer {
        /// WAV, mono or stereo, 16-bit.
        Wav = ("wav", "WAV"),
        /// FLAC, mono, 16-bit. Stereo audio is mixed to one channel.
        Flac = ("flac", "FLAC"),
    }
}

sdrkit_core::choice_enum! {
    /// The container of an IQ recording.
    pub enum IqContainer {
        /// SigMF data and metadata.
        Sigmf = ("sigmf", "SigMF"),
        /// Samples only, with the rate and the centre in the name.
        Raw = ("raw", "Raw"),
    }
}

sdrkit_core::choice_enum! {
    /// The sample type of an IQ recording.
    pub enum IqSample {
        /// Signed 8-bit pairs: round(x * 128), limited to -128..127.
        Cs8 = ("cs8", "8-bit"),
        /// Signed 16-bit pairs, little endian: round(x * 32768), limited to -32768..32767.
        Cs16 = ("cs16", "16-bit"),
        /// 32-bit float pairs, little endian.
        Cf32 = ("cf32", "Float"),
    }
}

sdrkit_core::choice_enum! {
    /// What a recording holds.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum MediaKind {
        /// Audio frames.
        Audio = ("audio", "Audio"),
        /// IQ samples.
        Iq = ("iq", "IQ"),
        /// A transport stream.
        Ts = ("ts", "Transport stream"),
    }
}

impl AudioContainer {
    /// The container.
    pub fn container(self) -> Container {
        match self {
            AudioContainer::Wav => Container::Wav,
            AudioContainer::Flac => Container::Flac,
        }
    }
}

impl IqContainer {
    /// The container.
    pub fn container(self) -> Container {
        match self {
            IqContainer::Sigmf => Container::Sigmf,
            IqContainer::Raw => Container::Raw,
        }
    }
}

impl IqSample {
    /// The stream sample type.
    pub fn sample_type(self) -> SampleType {
        match self {
            IqSample::Cs8 => SampleType::Cs8,
            IqSample::Cs16 => SampleType::Cs16,
            IqSample::Cf32 => SampleType::Cf32,
        }
    }
    /// The extension of a raw file.
    pub fn extension(self) -> &'static str {
        match self {
            IqSample::Cs8 => "cs8",
            IqSample::Cs16 => "cs16",
            IqSample::Cf32 => "cf32",
        }
    }
    /// The SigMF datatype.
    pub fn sigmf_datatype(self) -> &'static str {
        match self {
            IqSample::Cs8 => "ci8",
            IqSample::Cs16 => "ci16_le",
            IqSample::Cf32 => "cf32_le",
        }
    }
    /// Bytes per complex sample.
    pub fn bytes(self) -> usize {
        match self {
            IqSample::Cs8 => 2,
            IqSample::Cs16 => 4,
            IqSample::Cf32 => 8,
        }
    }
    /// Appends `iq` to `out` in this sample type. It allocates nothing when `out` has room.
    pub fn encode(self, iq: &[Complex32], out: &mut Vec<u8>) {
        match self {
            IqSample::Cs8 => {
                let q = |v: f32| (v * 128.0).round().clamp(-128.0, 127.0) as i8 as u8;
                out.extend(iq.iter().flat_map(|c| [q(c.re), q(c.im)]));
            }
            IqSample::Cs16 => {
                for c in iq {
                    out.extend_from_slice(&i16_full(c.re).to_le_bytes());
                    out.extend_from_slice(&i16_full(c.im).to_le_bytes());
                }
            }
            IqSample::Cf32 => {
                for c in iq {
                    out.extend_from_slice(&c.re.to_le_bytes());
                    out.extend_from_slice(&c.im.to_le_bytes());
                }
            }
        }
    }
    /// The sample type of a SigMF datatype or a raw extension.
    fn of_name(name: &str) -> Option<IqSample> {
        match name {
            "ci8" | "cs8" => Some(IqSample::Cs8),
            "ci16_le" | "ci16" | "cs16" => Some(IqSample::Cs16),
            "cf32_le" | "cf32" => Some(IqSample::Cf32),
            _ => None,
        }
    }
}

/// `x` as a 16-bit sample for IQ: round(x * 32768), limited.
fn i16_full(x: f32) -> i16 {
    (x * 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

/// `x` as a 16-bit audio sample: round(x * 32767), limited.
pub(crate) fn i16_audio(x: f32) -> i16 {
    (x * 32767.0).round().clamp(-32768.0, 32767.0) as i16
}

/// The rate in a file name: megasamples a second with up to 9 significant digits and no trailing zeros.
pub fn rate_for_name(rate_hz: Hz) -> String {
    let m = rate_hz / 1e6;
    if m <= 0.0 || !m.is_finite() {
        return "0".into();
    }
    let mag = m.log10().floor() as i32 + 1;
    let decimals = (9 - mag).max(0) as usize;
    let s = format!("{m:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// `text` with every character that is not a letter or a digit replaced by `-`.
pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}

/// The base name of an IQ recording (no folder, no extension): `<prefix>_<stack>_<MHz>MHz_<Msps>Msps_<YYYYMMDD-HHMMSS>`.
pub fn iq_name(prefix: &str, stack_id: &str, centre_hz: Hz, rate_hz: Hz, stamp: &str) -> String {
    format!(
        "{prefix}_{}_{:.3}MHz_{}Msps_{stamp}",
        clean(stack_id),
        centre_hz / 1e6,
        rate_for_name(rate_hz)
    )
}

/// The local time `unix` (UTC seconds) at `offset_min` minutes from UTC as `YYYYMMDD-HHMMSS`.
pub fn stamp(unix: f64, offset_min: i32) -> String {
    let secs = if unix.is_finite() {
        unix.floor() as i64
    } else {
        0
    };
    match Civil::from_unix_local(secs, 0, offset_min) {
        Ok(c) => format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            c.year, c.month, c.day, c.hour, c.minute, c.second
        ),
        Err(_) => "00000000-000000".into(),
    }
}

/// `unix` as ISO 8601 UTC text with whole seconds.
pub fn iso_utc(unix: f64) -> String {
    iso8601(unix, 0).map_or_else(|_| String::new(), |t| t.as_str().to_string())
}

/// A recording file as read from storage.
#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    /// The container.
    pub container: Container,
    /// What the file holds: the kind, the rate, the centre (IQ), the channels (audio) and the stored sample type.
    pub format: Format,
    /// The file that holds the samples (the `.sigmf-data` file of a SigMF recording).
    pub data: StorePath,
    /// The frames of audio, complex samples of IQ, or packets of a transport stream.
    pub frames: u64,
}

impl Recording {
    /// What the recording holds.
    pub fn kind(&self) -> MediaKind {
        match self.container {
            Container::Wav | Container::Flac => MediaKind::Audio,
            Container::Sigmf | Container::Raw => MediaKind::Iq,
            Container::Ts => MediaKind::Ts,
        }
    }
    /// The length in seconds, if the rate is known.
    pub fn seconds(&self) -> Option<f64> {
        (self.format.rate > 0.0).then(|| self.frames as f64 / self.format.rate)
    }
}

/// The container that the extension of `path` names. A `.sigmf-data` or `.sigmf-meta` file names a SigMF recording.
pub fn container_of(path: &StorePath) -> Option<Container> {
    match path.extension()? {
        "wav" => Some(Container::Wav),
        "flac" => Some(Container::Flac),
        "sigmf-meta" | "sigmf-data" => Some(Container::Sigmf),
        "cs8" | "cs16" | "cf32" => Some(Container::Raw),
        "ts" => Some(Container::Ts),
        _ => None,
    }
}

/// Reads the format of the recording at `path`: a WAV, FLAC, SigMF (the data or the meta file), raw IQ or transport stream
/// file.
///
/// # Errors
///
/// [`RecordError::Storage`] if the storage fails, [`RecordError::Format`] if the file does not read as its container.
pub fn read_recording(storage: &dyn Storage, path: &StorePath) -> Result<Recording, RecordError> {
    let bad = |error| RecordError::Format {
        path: path.clone(),
        error,
    };
    let container = container_of(path).ok_or(bad(FormatError::UnknownName))?;
    match container {
        Container::Wav => {
            let head = read_head(storage, path, wav::HEAD_MAX)?;
            let size = storage
                .stat(path)
                .map_err(|e| RecordError::storage(path, e))?
                .size;
            let w = wav::parse(&head, size).map_err(bad)?;
            Ok(Recording {
                container,
                format: Format {
                    sample_type: Some(w.sample_type),
                    ..Format::audio(f64::from(w.rate), w.channels)
                },
                data: path.clone(),
                frames: w.frames,
            })
        }
        Container::Flac => {
            let head = read_head(storage, path, 42)?;
            let (rate, channels, samples) =
                crate::flac::streaminfo(&head).ok_or(bad(FormatError::NotFlac))?;
            Ok(Recording {
                container,
                format: Format {
                    sample_type: Some(SampleType::I16),
                    ..Format::audio(f64::from(rate), channels)
                },
                data: path.clone(),
                frames: samples,
            })
        }
        Container::Sigmf => {
            let meta = path
                .with_extension("sigmf-meta")
                .map_err(RecordError::Name)?;
            let data = path
                .with_extension("sigmf-data")
                .map_err(RecordError::Name)?;
            let text = storage
                .read(&meta)
                .map_err(|e| RecordError::storage(&meta, e))?;
            let m = sigmf::read(&text).map_err(|error| RecordError::Format {
                path: meta.clone(),
                error,
            })?;
            let sample = IqSample::of_name(&m.datatype)
                .ok_or(bad(FormatError::SigmfField(SigmfField::Datatype)))?;
            let size = storage
                .stat(&data)
                .map_err(|e| RecordError::storage(&data, e))?
                .size;
            Ok(iq_recording(
                container,
                data,
                sample,
                m.rate_hz,
                m.centre_hz,
                size,
            ))
        }
        Container::Raw => {
            let sample = path
                .extension()
                .and_then(IqSample::of_name)
                .ok_or(bad(FormatError::UnknownName))?;
            let (centre, rate) =
                name_numbers(path.file_name()).ok_or(bad(FormatError::UnknownName))?;
            let size = storage
                .stat(path)
                .map_err(|e| RecordError::storage(path, e))?
                .size;
            Ok(iq_recording(
                container,
                path.clone(),
                sample,
                rate,
                centre,
                size,
            ))
        }
        Container::Ts => {
            let head = read_head(storage, path, 1)?;
            if head.first() != Some(&sdrkit_proto_mpegts::packet::SYNC) {
                return Err(bad(FormatError::NotTs));
            }
            let size = storage
                .stat(path)
                .map_err(|e| RecordError::storage(path, e))?
                .size;
            Ok(Recording {
                container,
                format: Format::new(sdrkit_proto_mpegts::stream::kind(), 0.0),
                data: path.clone(),
                frames: size / sdrkit_proto_mpegts::packet::TS_SIZE as u64,
            })
        }
    }
}

fn iq_recording(
    container: Container,
    data: StorePath,
    sample: IqSample,
    rate: Hz,
    centre: Hz,
    size: u64,
) -> Recording {
    Recording {
        container,
        format: Format {
            sample_type: Some(sample.sample_type()),
            ..Format::iq(rate, centre, centre)
        },
        data,
        frames: size / sample.bytes() as u64,
    }
}

/// The first `max` bytes of `path` (fewer if the file is shorter).
fn read_head(storage: &dyn Storage, path: &StorePath, max: usize) -> Result<Vec<u8>, RecordError> {
    let r = storage
        .open_reader(path)
        .map_err(|e| RecordError::storage(path, e))?;
    let mut head = Vec::with_capacity(max);
    r.take(max as u64)
        .read_to_end(&mut head)
        .map_err(|e| RecordError::storage(path, e.into()))?;
    Ok(head)
}

/// The centre and the rate in an IQ name: the numbers before `MHz` and before `Msps`.
fn name_numbers(name: &str) -> Option<(Hz, Hz)> {
    let number_before = |unit: &str| -> Option<f64> {
        let end = name.find(unit)?;
        let start = name[..end].rfind('_').map_or(0, |i| i + 1);
        name[start..end].parse::<f64>().ok()
    };
    Some((number_before("MHz")? * 1e6, number_before("Msps")? * 1e6))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(rate_for_name(2.4e6), "2.4");
        assert_eq!(rate_for_name(900_001.0), "0.900001");
        assert_eq!(rate_for_name(64e6 / 7.0), "9.14285714");
        assert_eq!(rate_for_name(20e6), "20");
        let n = iq_name("sdrkit", "fm/rds", 100e6, 2.4e6, "20250101-120000");
        assert_eq!(n, "sdrkit_fm-rds_100.000MHz_2.4Msps_20250101-120000");
        assert_eq!(name_numbers(&n), Some((100e6, 2.4e6)));
        assert_eq!(stamp(1_735_732_800.0, 60), "20250101-130000");
    }
}
