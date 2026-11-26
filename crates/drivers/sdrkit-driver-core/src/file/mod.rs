//! The file driver: plays SigMF, WAV and raw recordings from the host's storage. The address is the store path of the file.
//! Playback runs on its own thread in blocks of 32 768 samples, paced to real time times a pace or as fast as the reader takes
//! them. It waits for room and never drops. The tuning range is the centre of the recording, and the driver refuses another
//! centre. The rate is the rate of the file.

pub mod format;

pub use format::{probe, FileFault, FileInfo, FileKind, Format, Stored};

use crate::paced::{PacedWriter, Pacing, BENCHMARK, BLOCK, PACE};
use crate::{
    Capabilities, Doing, DriverDescriptor, DriverError, Link, RadioSetting, RateLimits, Retuned,
    RingWriter, Source, SourceDriver, Started, StreamFormat, CENTRE, RATE, RATE_EXACT,
};
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, SettingSet, Storage, StorePath, Values};
use sdrkit_graph::stream::{Kind, SampleType};
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Start again from the beginning at the end of the file.
pub const LOOP: Setting<field::Bool> = Setting::new("loop", "Loop", "", false)
    .section(crate::form::OTHER_SECTION)
    .help("Starts again from the beginning at the end of the file.");
/// The rate of a raw file whose name does not give it. 0 takes the rate from the file.
pub const FILE_RATE: Setting<field::Hz> = Setting::new("file_rate", "Rate of the file", "Hz", 0.0)
    .range(0.0, 100e6, 1.0)
    .section(crate::form::RATE_SECTION)
    .help("For a raw file whose name does not give its rate. 0 takes the rate from the file.");

sdrkit_core::settings! {
    /// The own settings of the file driver.
    pub enum FileSetting {
        /// [`FILE_RATE`].
        FileRate = FILE_RATE,
        /// [`PACE`].
        Pace = PACE,
        /// [`LOOP`].
        Loop = LOOP,
        /// [`BENCHMARK`].
        Benchmark = BENCHMARK,
    }
}

const fn produces(kind: Kind, sample: SampleType) -> StreamFormat {
    StreamFormat {
        kind,
        sample,
        min_rate_hz: 1.0,
        max_rate_hz: 100e6,
        channels: 1,
    }
}

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "file",
    name: "Recording",
    opens: crate::Opens::Many,
    addresses: &["file:<path> (a SigMF, WAV or raw IQ recording in the storage folder)"],
    address: &[
        crate::AddressPart::text("path", "File", "", crate::Syntax::StorePath)
            .required()
            .help("A SigMF, WAV or raw IQ recording, as a path in the storage folder."),
    ],
    settings: &[FileSetting::SETTINGS],
    commands: &[],
    readings: &[],
    produces: &[
        produces(Kind::Iq, SampleType::Cs8),
        produces(Kind::Iq, SampleType::Cu8),
        produces(Kind::Iq, SampleType::Cs16),
        produces(Kind::Iq, SampleType::Cf32),
        produces(Kind::Real, SampleType::I8),
        produces(Kind::Real, SampleType::U8),
        produces(Kind::Real, SampleType::I16),
        produces(Kind::Real, SampleType::F32),
    ],
    accepts: &[],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The file driver. It reads the files from the host's storage.
#[derive(Clone)]
pub struct FileDriver {
    storage: Arc<dyn Storage>,
}

impl FileDriver {
    /// A driver that reads files from `storage`.
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        FileDriver { storage }
    }
}

impl SourceDriver for FileDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }
    fn open(&self, address: &str, v: &Values) -> Result<Box<dyn Source>, DriverError> {
        let path = StorePath::new(address).map_err(|_| DriverError::Address)?;
        let mut info = probe(self.storage.as_ref(), &path)?;
        if v.get(&FILE_RATE) > 0.0 {
            info.rate_hz = v.get(&FILE_RATE);
        } else if info.rate_hz <= 0.0 && v.get(&RATE_EXACT) && v.get(&RATE) > 0.0 {
            info.rate_hz = v.get(&RATE);
        }
        let no = |fault| DriverError::File {
            path: path.clone(),
            fault,
        };
        if info.rate_hz <= 0.0 {
            return Err(no(FileFault::NoRate));
        }
        if info.samples() == 0 {
            return Err(no(FileFault::Empty));
        }
        let r = info.rate_hz;
        let centre_hz = info.centre_hz.unwrap_or(v.get(&CENTRE));
        let caps = Capabilities {
            name: path.file_name().to_owned(),
            rates: RateLimits {
                min_hz: r,
                max_hz: r,
                ranges: vec![(r, r)],
                steady_hz: None,
            },
            min_freq_hz: centre_hz,
            max_freq_hz: centre_hz,
            ..Default::default()
        };
        Ok(Box::new(FileSource {
            caps,
            info,
            storage: self.storage.clone(),
            centre_hz,
            asked_hz: v.get(&CENTRE),
            pacing: Pacing::of(v),
            looping: v.get(&LOOP),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }))
    }
}

/// A file that plays.
pub struct FileSource {
    caps: Capabilities,
    info: FileInfo,
    storage: Arc<dyn Storage>,
    centre_hz: f64,
    /// The last centre that the host requested. A second request for the same centre is not refused.
    asked_hz: f64,
    pacing: Pacing,
    looping: bool,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FileSource {
    /// What the file holds.
    pub fn info(&self) -> &FileInfo {
        &self.info
    }
    fn started(&self) -> Started {
        Started {
            rate_hz: self.info.rate_hz,
            centre_hz: self.centre_hz,
        }
    }
}

impl Source for FileSource {
    fn capabilities(&self) -> &Capabilities {
        &self.caps
    }

    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        if self.thread.is_some() {
            return Err(DriverError::NotNow {
                state: Doing::Running,
            });
        }
        if sink.sample_type() != self.info.format.ring_type() {
            return Err(DriverError::WrongSampleType {
                ring: sink.sample_type(),
                radio: self.info.format.ring_type(),
            });
        }
        let data = &self.info.data_path;
        let f = self
            .storage
            .open_reader(data)
            .map_err(|source| DriverError::Storage {
                path: data.clone(),
                source,
            })?;
        let (info, pacing, looping, stop) = (
            self.info.clone(),
            self.pacing,
            self.looping,
            self.stop.clone(),
        );
        let t = std::thread::Builder::new()
            .name("sdrkit-file".into())
            .spawn(move || {
                play(
                    f,
                    &info,
                    PacedWriter::new(sink, 0.0, pacing),
                    pacing,
                    looping,
                    &stop,
                );
            })
            .map_err(|_| DriverError::Thread)?;
        self.thread = Some(t);
        Ok(self.started())
    }

    /// Refuses a centre other than the centre of the recording and other than the one the host requested last.
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        let asked = values.get(&CENTRE);
        if asked != self.centre_hz && asked != self.asked_hz {
            return Err(DriverError::Refused {
                id: RadioSetting::Centre.id(),
            });
        }
        self.asked_hz = asked;
        Ok(Retuned::Live(self.started()))
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    fn sample_type(&self) -> SampleType {
        self.info.format.ring_type()
    }

    /// Returns `Link::Up` while the recording plays, `Link::Stopped` once it has played to its end, and `Link::Idle`
    /// before the start.
    fn link(&self) -> Link {
        match &self.thread {
            Some(t) if t.is_finished() => Link::Stopped,
            Some(_) => Link::Up,
            None => Link::Idle,
        }
    }
}

impl Drop for FileSource {
    fn drop(&mut self) {
        self.stop();
    }
}

fn play(
    mut f: Box<dyn sdrkit_core::storage::StoreReader>,
    info: &FileInfo,
    mut out: PacedWriter,
    pacing: Pacing,
    looping: bool,
    stop: &AtomicBool,
) {
    let bps = info.format.bytes_per_sample() as u64;
    let end = info.start + (info.end - info.start) / bps * bps;
    let mut bytes = vec![0u8; BLOCK * bps as usize];
    out.reanchor(info.rate_hz, pacing);
    loop {
        if f.seek(SeekFrom::Start(info.start)).is_err() {
            return;
        }
        let mut pos = info.start;
        while pos < end {
            let n = usize::try_from(end - pos)
                .unwrap_or(usize::MAX)
                .min(bytes.len());
            if f.read_exact(&mut bytes[..n]).is_err() {
                break;
            }
            pos += n as u64;
            info.format.to_ring(&mut bytes[..n]);
            if !out.push(&bytes[..n], stop) {
                return;
            }
        }
        if !looping || stop.load(Ordering::Acquire) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring;
    use crate::testing::values;
    use sdrkit_core::{Complex32, MemStorage, Value};
    use std::time::{Duration, Instant};

    fn driver(files: &[(&str, &[u8])]) -> FileDriver {
        let st = MemStorage::new();
        for (p, data) in files {
            st.write(&StorePath::new(p).unwrap(), data).unwrap();
        }
        FileDriver::new(Arc::new(st))
    }

    fn drain(r: &mut crate::RingReader, until_gone: Duration) -> Vec<Complex32> {
        let mut got = Vec::new();
        let mut buf = vec![Complex32::default(); 1 << 16];
        let t = Instant::now();
        while t.elapsed() < until_gone {
            r.wait(1, Duration::from_millis(20));
            let n = r.read_iq(&mut buf);
            got.extend_from_slice(&buf[..n]);
            if n == 0 && r.writer_gone() {
                break;
            }
        }
        got
    }

    #[test]
    fn a_raw_file_plays_whole_then_stops() {
        let bytes: Vec<u8> = (0..100_000u32)
            .flat_map(|i| [(i % 100) as u8, 0xC0])
            .collect();
        let d = driver(&[("rec/t_1Msps.cs8", &bytes)]);
        let v = values(&d, &[("benchmark", Value::Bool(true))]);
        let mut src = d.open("rec/t_1Msps.cs8", &v).unwrap();
        assert_eq!(src.capabilities().rates.max_hz, 1e6);
        // The ring is smaller than the file: the reader waits, nothing is dropped.
        let (w, mut r) = ring(1 << 14, SampleType::Cs8);
        let s = src.start(w).unwrap();
        assert_eq!((s.rate_hz, s.centre_hz), (1e6, 100e6));
        let got = drain(&mut r, Duration::from_secs(5));
        assert_eq!(got.len(), 100_000);
        // At the end, the radio reports that its stream stopped.
        let t = Instant::now();
        while src.link() != Link::Stopped && t.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(src.link(), Link::Stopped);
        assert!(got
            .iter()
            .enumerate()
            .all(|(i, c)| *c == Complex32::new((i % 100) as f32 / 128.0, -0.5)));
        assert_eq!(r.stats().dropped(), 0);
        src.stop();
    }

    #[test]
    fn playback_is_paced_and_loops() {
        let mut bytes = Vec::new();
        for i in 0..10_000 {
            bytes.extend((i as f32).to_le_bytes());
            bytes.extend(0f32.to_le_bytes());
        }
        let d = driver(&[("l.cf32", &bytes)]);
        // No rate in the name: refused unless given.
        let e = d.open("l.cf32", &values(&d, &[])).err();
        assert!(matches!(
            e,
            Some(DriverError::File {
                fault: FileFault::NoRate,
                ..
            })
        ));
        let v = values(
            &d,
            &[("file_rate", Value::Hz(0.5e6)), ("loop", Value::Bool(true))],
        );
        let mut src = d.open("l.cf32", &v).unwrap();
        let (w, mut r) = ring(1 << 20, SampleType::Cf32);
        src.start(w).unwrap();
        let got = drain(&mut r, Duration::from_millis(300));
        src.stop();
        // 300 ms at 0.5 Msps is 150 000 samples, in blocks of 10 000 (the file is shorter than a block).
        assert!((100_000..=200_000).contains(&got.len()), "{}", got.len());
        assert_eq!(got[10_000].re, 0.0);
        assert_eq!(got[10_001].re, 1.0);
    }

    #[test]
    fn the_tuning_range_is_the_centre_and_another_centre_is_refused() {
        let meta = br#"{"global":{"core:datatype":"cf32_le","core:sample_rate":500000},
            "captures":[{"core:sample_start":0,"core:frequency":100000000.0}]}"#;
        let d = driver(&[("fm.sigmf-meta", meta), ("fm.sigmf-data", &[0u8; 800])]);
        let mut src = d.open("fm.sigmf-meta", &values(&d, &[])).unwrap();
        let caps = src.capabilities();
        assert_eq!((caps.min_freq_hz, caps.max_freq_hz), (100e6, 100e6));
        let moved = values(&d, &[("centre_hz", Value::Hz(90e6))]);
        assert_eq!(
            src.apply(&moved),
            Err(DriverError::Refused {
                id: RadioSetting::Centre.id()
            })
        );
        let same = values(&d, &[("centre_hz", Value::Hz(100e6))]);
        assert_eq!(
            src.apply(&same).unwrap(),
            Retuned::Live(Started {
                rate_hz: 500e3,
                centre_hz: 100e6,
            })
        );
    }

    #[test]
    fn a_bad_address_or_a_missing_file_is_refused() {
        let d = driver(&[]);
        let v = values(&d, &[]);
        assert_eq!(d.open("/abs/x.cu8", &v).err(), Some(DriverError::Address));
        assert!(matches!(
            d.open("x_1Msps.cu8", &v).err(),
            Some(DriverError::Storage { .. })
        ));
    }
}
