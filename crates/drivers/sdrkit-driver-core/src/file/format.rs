//! What a recording file holds: SigMF, WAV and raw files with the rate in the name, read from the host's storage. The
//! samples go to the ring in a native type, so the reader converts them once.
//!
//! Standard: SigMF specification v1.0.0, global object (core:datatype, core:sample_rate, core:header_bytes,
//! core:trailing_bytes, core:dataset) and captures (core:sample_start, core:frequency).

use crate::DriverError;
use sdrkit_core::{Hz, Storage, StorePath};
use sdrkit_graph::stream::SampleType;
use serde_json::Value as Json;
use std::io::{Read, Seek, SeekFrom};

/// Why a file is not a recording that the driver can play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFault {
    /// The SigMF meta file is not a JSON object.
    NotSigmf,
    /// The SigMF meta file has no `global` object.
    NoGlobal,
    /// The SigMF meta file has no `core:datatype` text.
    NoDatatype,
    /// The data type is not one the driver plays (complex or real `f32`, `i16`, `u16`, `i8`, `u8`).
    Datatype,
    /// No data file is beside the SigMF meta file.
    NoData,
    /// No SigMF meta file is beside the data file.
    NoMeta,
    /// The first sample is past the end of the data file.
    StartPastEnd,
    /// The file is not RIFF/WAVE.
    NotWav,
    /// A WAV file that does not have two channels (I and Q).
    WavChannels {
        /// The channels it has.
        channels: u16,
    },
    /// A WAV format that is not 8-bit or 16-bit PCM or 32-bit float.
    WavFormat {
        /// The format tag.
        tag: u16,
        /// The bits per value.
        bits: u16,
    },
    /// The WAV data comes before its format.
    DataBeforeFormat,
    /// The WAV file has no data.
    WavNoData,
    /// The sample rate is not known. The `file_rate` setting supplies it.
    NoRate,
    /// The file holds no samples.
    Empty,
}

/// How one part (I or Q) is stored in a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stored {
    /// Signed 8-bit.
    I8,
    /// Unsigned 8-bit, centred on 127.5.
    U8,
    /// Signed 16-bit.
    I16,
    /// Unsigned 16-bit, centred on 32767.5.
    U16,
    /// 32-bit float.
    F32,
}

impl Stored {
    /// Bytes per part.
    pub fn bytes(self) -> usize {
        match self {
            Stored::I8 | Stored::U8 => 1,
            Stored::I16 | Stored::U16 => 2,
            Stored::F32 => 4,
        }
    }
}

/// How the samples of a file are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// The type of one part.
    pub ty: Stored,
    /// I and Q pairs. False for real samples.
    pub complex: bool,
    /// Multi-byte parts are big endian.
    pub big_endian: bool,
}

impl Format {
    /// Complex, little endian.
    pub const fn complex(ty: Stored) -> Format {
        Format {
            ty,
            complex: true,
            big_endian: false,
        }
    }

    /// Bytes per sample (a pair for complex).
    pub fn bytes_per_sample(&self) -> usize {
        self.ty.bytes() * if self.complex { 2 } else { 1 }
    }

    /// The type of the samples in the ring. Unsigned 16-bit parts become signed ones.
    pub fn ring_type(&self) -> SampleType {
        match (self.ty, self.complex) {
            (Stored::I8, true) => SampleType::Cs8,
            (Stored::U8, true) => SampleType::Cu8,
            (Stored::I16 | Stored::U16, true) => SampleType::Cs16,
            (Stored::F32, true) => SampleType::Cf32,
            (Stored::I8, false) => SampleType::I8,
            (Stored::U8, false) => SampleType::U8,
            (Stored::I16 | Stored::U16, false) => SampleType::I16,
            (Stored::F32, false) => SampleType::F32,
        }
    }

    /// Makes `bytes`, as stored in the file, into the bytes of [`Format::ring_type`] in place: big-endian parts are swapped,
    /// and unsigned 16-bit parts lose their offset.
    pub fn to_ring(&self, bytes: &mut [u8]) {
        let w = self.ty.bytes();
        if w == 1 || (!self.big_endian && self.ty != Stored::U16) {
            return;
        }
        for part in bytes.chunks_exact_mut(w) {
            if self.big_endian {
                part.reverse();
            }
            if self.ty == Stored::U16 {
                part[1] ^= 0x80;
            }
        }
    }

    /// Reads a SigMF `core:datatype` (any case, with an optional `_le` or `_be`, which 8-bit types ignore).
    ///
    /// # Errors
    ///
    /// [`FileFault::Datatype`] for a type the driver does not play.
    pub fn from_sigmf(datatype: &str) -> Result<Format, FileFault> {
        let d = datatype.trim().to_ascii_lowercase();
        let (body, big_endian) = if let Some(b) = d.strip_suffix("_be") {
            (b, true)
        } else if let Some(b) = d.strip_suffix("_le") {
            (b, false)
        } else {
            (d.as_str(), false)
        };
        let (complex, ty) = match body.split_at_checked(1) {
            Some(("c", t)) => (true, t),
            Some(("r", t)) => (false, t),
            _ => return Err(FileFault::Datatype),
        };
        let ty = match ty {
            "f32" => Stored::F32,
            "i16" => Stored::I16,
            "u16" => Stored::U16,
            "i8" => Stored::I8,
            "u8" => Stored::U8,
            _ => return Err(FileFault::Datatype),
        };
        Ok(Format {
            ty,
            complex,
            big_endian: big_endian && ty.bytes() > 1,
        })
    }
}

/// What kind of file it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// A SigMF recording (meta plus data).
    Sigmf,
    /// RIFF/WAVE with two channels.
    Wav,
    /// Raw samples, the format from the extension.
    Raw,
}

/// What a file holds, read from its header or name only.
#[derive(Clone, Debug, PartialEq)]
pub struct FileInfo {
    /// The file the user picked (the meta file for SigMF).
    pub path: StorePath,
    /// The file with the samples.
    pub data_path: StorePath,
    /// What kind of file.
    pub kind: FileKind,
    /// How the samples are stored.
    pub format: Format,
    /// The sample rate. 0 if not known.
    pub rate_hz: Hz,
    /// The centre frequency, if known.
    pub centre_hz: Option<Hz>,
    /// The first byte of samples in the data file.
    pub start: u64,
    /// One past the last byte of samples.
    pub end: u64,
    /// SigMF `core:description`, or empty.
    pub description: String,
    /// How many SigMF annotations.
    pub annotations: usize,
}

impl FileInfo {
    /// Samples in the file.
    pub fn samples(&self) -> u64 {
        (self.end - self.start) / self.format.bytes_per_sample() as u64
    }
    /// The length in seconds, if the rate is known.
    pub fn seconds(&self) -> Option<f64> {
        (self.rate_hz > 0.0).then(|| self.samples() as f64 / self.rate_hz)
    }
}

/// The number (digits and dots) just before `unit` in a file name (case-insensitive).
fn number_before(name: &str, unit: &str) -> Option<f64> {
    let lower = name.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut from = 0;
    while let Some(i) = lower[from..].find(unit) {
        let at = from + i;
        let mut s = at;
        while s > 0 && (bytes[s - 1].is_ascii_digit() || bytes[s - 1] == b'.') {
            s -= 1;
        }
        if let Ok(v) = lower[s..at].trim_start_matches('.').parse::<f64>() {
            return Some(v);
        }
        from = at + unit.len();
    }
    None
}

/// The rate written in a file name before `msps`, used when it is 0.5 to 100 Msps.
pub fn rate_from_name(name: &str) -> Option<Hz> {
    number_before(name, "msps")
        .filter(|v| (0.5..=100.0).contains(v))
        .map(|v| v * 1e6)
}

/// The centre frequency written in a file name before `MHz`.
pub fn centre_from_name(name: &str) -> Option<Hz> {
    number_before(name, "mhz")
        .filter(|v| *v > 0.0)
        .map(|v| v * 1e6)
}

fn fault(path: &StorePath, fault: FileFault) -> DriverError {
    DriverError::File {
        path: path.clone(),
        fault,
    }
}

fn storage(path: &StorePath) -> impl Fn(sdrkit_core::StorageError) -> DriverError + '_ {
    move |source| DriverError::Storage {
        path: path.clone(),
        source,
    }
}

fn file_len(storage_: &dyn Storage, p: &StorePath) -> Result<u64, DriverError> {
    storage_.stat(p).map(|e| e.size).map_err(storage(p))
}

fn exists(storage_: &dyn Storage, p: &StorePath) -> bool {
    storage_.stat(p).is_ok_and(|e| !e.is_dir)
}

fn sibling(p: &StorePath, ext: &str) -> Option<StorePath> {
    p.with_extension(ext).ok()
}

/// Reads the header or the name of the file at `path` in `storage` (never its samples) and says how to play it.
///
/// # Errors
///
/// [`DriverError::Storage`] if the file cannot be read, [`DriverError::File`] if it is not a recording that the driver can play.
pub fn probe(storage_: &dyn Storage, path: &StorePath) -> Result<FileInfo, DriverError> {
    let ext = path.extension().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "sigmf-meta" => probe_sigmf(storage_, path),
        "sigmf-data" => {
            let meta = sibling(path, "sigmf-meta")
                .filter(|m| exists(storage_, m))
                .ok_or_else(|| fault(path, FileFault::NoMeta))?;
            probe_sigmf(storage_, &meta)
        }
        "wav" => probe_wav(storage_, path),
        _ => probe_raw(storage_, path, &ext),
    }
}

fn probe_raw(storage_: &dyn Storage, path: &StorePath, ext: &str) -> Result<FileInfo, DriverError> {
    let ty = match ext {
        "cu8" | "u8" => Stored::U8,
        "cs16" | "ci16" | "s16" => Stored::I16,
        "cf32" | "fc32" | "cfile" => Stored::F32,
        _ => Stored::I8,
    };
    let end = file_len(storage_, path)?;
    let name = path.file_name();
    Ok(FileInfo {
        path: path.clone(),
        data_path: path.clone(),
        kind: FileKind::Raw,
        format: Format::complex(ty),
        rate_hz: rate_from_name(name).unwrap_or(0.0),
        centre_hz: centre_from_name(name),
        start: 0,
        end,
        description: String::new(),
        annotations: 0,
    })
}

fn probe_wav(storage_: &dyn Storage, path: &StorePath) -> Result<FileInfo, DriverError> {
    let mut f = storage_.open_reader(path).map_err(storage(path))?;
    let size = f.size().map_err(storage(path))?;
    let not_wav = || fault(path, FileFault::NotWav);
    let mut head = [0u8; 12];
    f.read_exact(&mut head).map_err(|_| not_wav())?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Err(not_wav());
    }
    let mut pos = 12u64;
    let mut fmt: Option<(Format, Hz)> = None;
    loop {
        let mut ch = [0u8; 8];
        f.seek(SeekFrom::Start(pos)).map_err(|_| not_wav())?;
        if f.read_exact(&mut ch).is_err() {
            return Err(fault(path, FileFault::WavNoData));
        }
        let len = u64::from(u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]));
        let body = pos + 8;
        match &ch[0..4] {
            b"fmt " => {
                let mut b = [0u8; 64];
                let n = usize::try_from(len.min(64)).unwrap_or(64);
                f.read_exact(&mut b[..n]).map_err(|_| not_wav())?;
                if n < 16 {
                    return Err(not_wav());
                }
                let mut tag = u16::from_le_bytes([b[0], b[1]]);
                let channels = u16::from_le_bytes([b[2], b[3]]);
                let rate = f64::from(u32::from_le_bytes([b[4], b[5], b[6], b[7]]));
                let bits = u16::from_le_bytes([b[14], b[15]]);
                if tag == 0xFFFE && n >= 26 {
                    // The sub-format GUID of WAVE_FORMAT_EXTENSIBLE starts with the format tag.
                    tag = u16::from_le_bytes([b[24], b[25]]);
                }
                if channels != 2 {
                    return Err(fault(path, FileFault::WavChannels { channels }));
                }
                let ty = match (tag, bits) {
                    (1, 16) => Stored::I16,
                    (1, 8) => Stored::U8,
                    (3, 32) => Stored::F32,
                    _ => return Err(fault(path, FileFault::WavFormat { tag, bits })),
                };
                fmt = Some((Format::complex(ty), rate));
            }
            b"data" => {
                let (format, rate) = fmt.ok_or_else(|| fault(path, FileFault::DataBeforeFormat))?;
                let end = if len == 0 || len == 0xFFFF_FFFF || body + len > size {
                    size
                } else {
                    body + len
                };
                return Ok(FileInfo {
                    path: path.clone(),
                    data_path: path.clone(),
                    kind: FileKind::Wav,
                    format,
                    rate_hz: rate,
                    centre_hz: centre_from_name(path.file_name()),
                    start: body,
                    end,
                    description: String::new(),
                    annotations: 0,
                });
            }
            _ => {}
        }
        pos = body + len + (len & 1);
    }
}

fn num(v: Option<&Json>) -> Option<f64> {
    v.and_then(Json::as_f64)
}

/// Reads the SigMF meta file `meta`. This is the one place that reads SigMF JSON text in this crate.
fn probe_sigmf(storage_: &dyn Storage, meta: &StorePath) -> Result<FileInfo, DriverError> {
    let text = storage_.read(meta).map_err(storage(meta))?;
    let root: Json = serde_json::from_slice(&text).map_err(|_| fault(meta, FileFault::NotSigmf))?;
    let root = root
        .as_object()
        .ok_or_else(|| fault(meta, FileFault::NotSigmf))?;
    let global = root
        .get("global")
        .and_then(Json::as_object)
        .ok_or_else(|| fault(meta, FileFault::NoGlobal))?;
    let datatype = global
        .get("core:datatype")
        .and_then(Json::as_str)
        .ok_or_else(|| fault(meta, FileFault::NoDatatype))?;
    let format = Format::from_sigmf(datatype).map_err(|f| fault(meta, f))?;
    let first_capture = root
        .get("captures")
        .and_then(Json::as_array)
        .and_then(|a| a.first());
    let rate_hz = num(global.get("core:sample_rate"))
        .or_else(|| num(first_capture.and_then(|c| c.get("core:sample_rate"))))
        .filter(|r| *r > 0.0)
        .unwrap_or(0.0);
    let annotations = root.get("annotations").and_then(Json::as_array);
    let centre_hz = num(first_capture.and_then(|c| c.get("core:frequency"))).or_else(|| {
        annotations?.iter().find_map(|a| {
            let lo = num(a.get("core:freq_lower_edge"))?;
            let hi = num(a.get("core:freq_upper_edge"))?;
            (hi > lo && lo > 0.0).then_some((lo + hi) / 2.0)
        })
    });
    let dataset = global
        .get("core:dataset")
        .and_then(Json::as_str)
        .and_then(|d| match meta.parent() {
            Some(dir) if !dir.is_root() => dir.join(d).ok(),
            _ => StorePath::new(d).ok(),
        })
        .filter(|p| exists(storage_, p));
    let data_path = dataset
        .or_else(|| sibling(meta, "sigmf-data").filter(|p| exists(storage_, p)))
        .or_else(|| sibling(meta, "SIGMF-DATA").filter(|p| exists(storage_, p)))
        .ok_or_else(|| fault(meta, FileFault::NoData))?;
    let size = file_len(storage_, &data_path)?;
    let count = |v: Option<&Json>| num(v).filter(|x| *x > 0.0).map_or(0, |x| x as u64);
    let header = count(global.get("core:header_bytes"));
    let start = header
        + count(first_capture.and_then(|c| c.get("core:sample_start")))
            * format.bytes_per_sample() as u64;
    if start > size {
        return Err(fault(meta, FileFault::StartPastEnd));
    }
    let trailing = count(global.get("core:trailing_bytes"));
    let end = if trailing < size {
        size - trailing
    } else {
        size
    }
    .max(start);
    Ok(FileInfo {
        path: meta.clone(),
        data_path,
        kind: FileKind::Sigmf,
        format,
        rate_hz,
        centre_hz,
        start,
        end,
        description: global
            .get("core:description")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string(),
        annotations: annotations.map_or(0, |a| a.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::MemStorage;

    fn path(p: &str) -> StorePath {
        StorePath::new(p).unwrap()
    }

    fn put(st: &MemStorage, p: &str, data: &[u8]) {
        st.write(&path(p), data).unwrap();
    }

    fn fault_of(st: &MemStorage, p: &str) -> FileFault {
        match probe(st, &path(p)) {
            Err(DriverError::File { fault, .. }) => fault,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn datatypes() {
        assert_eq!(
            Format::from_sigmf("cf32_le"),
            Ok(Format::complex(Stored::F32))
        );
        assert_eq!(
            Format::from_sigmf("CI16_BE"),
            Ok(Format {
                ty: Stored::I16,
                complex: true,
                big_endian: true
            })
        );
        assert_eq!(
            Format::from_sigmf("ci8_le"),
            Ok(Format::complex(Stored::I8))
        );
        assert_eq!(
            Format::from_sigmf("cu8_be"),
            Ok(Format::complex(Stored::U8))
        );
        assert_eq!(
            Format::from_sigmf("ri16_le"),
            Ok(Format {
                ty: Stored::I16,
                complex: false,
                big_endian: false
            })
        );
        assert_eq!(Format::from_sigmf("ci32_le"), Err(FileFault::Datatype));
    }

    #[test]
    fn stored_bytes_become_ring_bytes() {
        let be16 = Format {
            ty: Stored::I16,
            complex: true,
            big_endian: true,
        };
        assert_eq!(be16.ring_type(), SampleType::Cs16);
        let mut b = [0x40, 0, 0xC0, 0];
        be16.to_ring(&mut b);
        assert_eq!(b, [0, 0x40, 0, 0xC0]);
        let u16le = Format::complex(Stored::U16);
        let mut b = [0xFF, 0xFF, 0, 0];
        u16le.to_ring(&mut b);
        assert_eq!(b, [0xFF, 0x7F, 0, 0x80]);
        let mut f = 0.25f32.to_be_bytes().to_vec();
        f.extend((-1.5f32).to_be_bytes());
        Format {
            ty: Stored::F32,
            complex: true,
            big_endian: true,
        }
        .to_ring(&mut f);
        assert_eq!(&f[..4], &0.25f32.to_le_bytes());
        assert_eq!(&f[4..], &(-1.5f32).to_le_bytes());
        let real8 = Format {
            ty: Stored::I8,
            complex: false,
            big_endian: false,
        };
        assert_eq!(real8.ring_type(), SampleType::I8);
        assert_eq!(Format::complex(Stored::U8).ring_type(), SampleType::Cu8);
    }

    #[test]
    fn names_carry_rate_and_centre() {
        assert_eq!(
            rate_from_name("x_fm_100.000MHz_2.4Msps_20250101-120000.cs16"),
            Some(2.4e6)
        );
        assert_eq!(
            centre_from_name("x_fm_100.000MHz_2.4Msps_20250101-120000.cs16"),
            Some(100e6)
        );
        assert_eq!(rate_from_name("capture_0.25msps.cu8"), None);
        assert_eq!(rate_from_name("capture_250Msps.cu8"), None);
        assert_eq!(rate_from_name("capture.cu8"), None);
        assert_eq!(rate_from_name("a_10MSPS.cs8"), Some(10e6));
    }

    #[test]
    fn a_sigmf_recording_is_read() {
        let st = MemStorage::new();
        put(
            &st,
            "fm/fm.sigmf-meta",
            br#"{"global":{"core:datatype":"cf32_le","core:sample_rate":500000,
            "core:description":"Synthetic FM broadcast station (stereo, RDS), 4.0 s."},
            "captures":[{"core:sample_start":0,"core:frequency":100000000.0}],"annotations":[]}"#,
        );
        put(&st, "fm/fm.sigmf-data", &vec![0u8; 16_000_000]);
        let i = probe(&st, &path("fm/fm.sigmf-meta")).unwrap();
        assert_eq!(i.format, Format::complex(Stored::F32));
        assert_eq!((i.rate_hz, i.centre_hz, i.start), (500e3, Some(100e6), 0));
        assert_eq!(i.samples(), 2_000_000);
        assert!(i.description.starts_with("Synthetic FM"));
        let via_data = probe(&st, &path("fm/fm.sigmf-data")).unwrap();
        assert_eq!(via_data.data_path, i.data_path);
    }

    #[test]
    fn sigmf_details() {
        let st = MemStorage::new();
        put(&st, "d/a.sigmf-data", &[0u8; 1000]);
        put(
            &st,
            "d/a.sigmf-meta",
            br#"{"global":{"core:datatype":"ci16_le","core:header_bytes":8,"core:trailing_bytes":12},
            "captures":[{"core:sample_start":10,"core:sample_rate":1e6}],
            "annotations":[{"core:freq_lower_edge":99e6,"core:freq_upper_edge":101e6}]}"#,
        );
        let i = probe(&st, &path("d/a.sigmf-meta")).unwrap();
        assert_eq!(
            (i.rate_hz, i.centre_hz, i.start, i.end, i.annotations),
            (1e6, Some(100e6), 48, 988, 1)
        );
        put(
            &st,
            "d/b.sigmf-meta",
            br#"{"global":{"core:datatype":"cu8","core:dataset":"a.sigmf-data"},"captures":[{"core:sample_start":2000}]}"#,
        );
        assert_eq!(fault_of(&st, "d/b.sigmf-meta"), FileFault::StartPastEnd);
        put(&st, "d/c.sigmf-meta", b"[1,2]");
        assert_eq!(fault_of(&st, "d/c.sigmf-meta"), FileFault::NotSigmf);
        put(&st, "d/c.sigmf-meta", br#"{"captures":[]}"#);
        assert_eq!(fault_of(&st, "d/c.sigmf-meta"), FileFault::NoGlobal);
        put(&st, "d/c.sigmf-meta", br#"{"global":{"core:datatype":5}}"#);
        assert_eq!(fault_of(&st, "d/c.sigmf-meta"), FileFault::NoDatatype);
        put(
            &st,
            "d/c.sigmf-meta",
            br#"{"global":{"core:datatype":"cf32"}}"#,
        );
        assert_eq!(fault_of(&st, "d/c.sigmf-meta"), FileFault::NoData);
        put(&st, "d/lone.sigmf-data", &[0u8; 8]);
        assert_eq!(fault_of(&st, "d/lone.sigmf-data"), FileFault::NoMeta);
    }

    fn wav(
        channels: u16,
        tag: u16,
        bits: u16,
        rate: u32,
        data: &[u8],
        data_len: u32,
        extensible: bool,
    ) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend((if extensible { 0xFFFEu16 } else { tag }).to_le_bytes());
        fmt.extend(channels.to_le_bytes());
        fmt.extend(rate.to_le_bytes());
        fmt.extend((rate * channels as u32 * bits as u32 / 8).to_le_bytes());
        fmt.extend((channels * bits / 8).to_le_bytes());
        fmt.extend(bits.to_le_bytes());
        if extensible {
            fmt.extend(22u16.to_le_bytes());
            fmt.extend(bits.to_le_bytes());
            fmt.extend(3u32.to_le_bytes());
            fmt.extend(tag.to_le_bytes());
            fmt.extend([0u8; 14]);
        }
        let mut v = b"RIFF\0\0\0\0WAVE".to_vec();
        v.extend(b"LIST");
        v.extend(3u32.to_le_bytes());
        v.extend([1, 2, 3, 0]);
        v.extend(b"fmt ");
        v.extend((fmt.len() as u32).to_le_bytes());
        v.extend(fmt);
        v.extend(b"data");
        v.extend(data_len.to_le_bytes());
        v.extend(data);
        v
    }

    #[test]
    fn wav_files() {
        let st = MemStorage::new();
        let data: Vec<u8> = (0..40).collect();
        put(
            &st,
            "x_433.920MHz.wav",
            &wav(2, 1, 16, 250_000, &data, 40, false),
        );
        let i = probe(&st, &path("x_433.920MHz.wav")).unwrap();
        assert_eq!(
            (i.format.ty, i.rate_hz, i.centre_hz, i.samples()),
            (Stored::I16, 250e3, Some(433.92e6), 10)
        );
        put(
            &st,
            "s.wav",
            &wav(2, 3, 32, 48_000, &data, 0xFFFF_FFFF, true),
        );
        let i = probe(&st, &path("s.wav")).unwrap();
        assert_eq!((i.format.ty, i.samples()), (Stored::F32, 5));
        put(&st, "m.wav", &wav(1, 1, 16, 48_000, &data, 40, false));
        assert_eq!(
            fault_of(&st, "m.wav"),
            FileFault::WavChannels { channels: 1 }
        );
        let mut early = b"RIFF\0\0\0\0WAVEdata\x04\0\0\0abcd".to_vec();
        early.extend(b"fmt ");
        put(&st, "e.wav", &early);
        assert_eq!(fault_of(&st, "e.wav"), FileFault::DataBeforeFormat);
    }
}
