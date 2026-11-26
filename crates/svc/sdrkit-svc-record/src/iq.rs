//! IQ recording: the samples of a stream as SigMF or raw files.

use crate::error::RecordError;
use crate::format::{iq_name, iso_utc, stamp, IqContainer, IqSample};
use crate::sigmf;
use crate::spool::{self, Encoding, Live, Op, Spool, Writer};
use sdrkit_core::{Clock, Complex32, Hz, Storage, StorePath};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Bytes in one block of the writer.
const BLOCK_BYTES: usize = 256 * 1024;

/// How the samples are recorded.
#[derive(Clone, Debug)]
pub(crate) struct IqSpec {
    pub folder: StorePath,
    pub prefix: String,
    pub container: IqContainer,
    pub sample: IqSample,
    pub stack_id: String,
    pub utc_offset_min: i32,
    pub max_s: f64,
    pub queue_bytes: u64,
}

/// The control side of an IQ recording.
pub(crate) struct IqControl {
    pub writer: Writer<Vec<u8>>,
    pub live: Arc<Live>,
}

/// The feed of an IQ recording. The engine passes it the samples of the stream and the places where samples are missing.
/// [`IqFeed::push`] keeps every sample: when the write queue is full it waits for the writer. It allocates nothing once
/// its file is open.
pub struct IqFeed {
    spec: IqSpec,
    clock: Clock,
    spool: Spool<Vec<u8>>,
    live: Arc<Live>,
    session: String,
    format: Option<(Hz, Hz)>,
    total: u64,
    ended: bool,
    /// The metadata of the open SigMF file, the samples in that file, and the gaps in it.
    meta: Option<sigmf::Meta>,
    in_file: u64,
    gaps: Vec<(u64, u64)>,
}

impl std::fmt::Debug for IqFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IqFeed")
            .field("session", &self.session)
            .field("format", &self.format)
            .finish()
    }
}

/// Starts an IQ recording of `session` on `storage`. The file opens at the first [`IqFeed::set_format`].
pub(crate) fn start(
    spec: IqSpec,
    session: &str,
    storage: Arc<dyn Storage>,
    clock: Clock,
) -> Result<(IqFeed, IqControl), RecordError> {
    let buffers = (spec.queue_bytes / BLOCK_BYTES as u64).max(2) as usize;
    let (spool, writer) = spool::start(storage, 1, buffers, || Vec::with_capacity(BLOCK_BYTES))?;
    let live = Arc::new(Live::default());
    Ok((
        IqFeed {
            spec,
            clock,
            spool,
            live: live.clone(),
            session: session.into(),
            format: None,
            total: 0,
            ended: false,
            meta: None,
            in_file: 0,
            gaps: Vec::new(),
        },
        IqControl { writer, live },
    ))
}

impl IqFeed {
    /// The session whose samples this records.
    pub fn session(&self) -> &str {
        &self.session
    }

    /// True once the recording has ended. The engine drops an ended feed.
    pub fn is_ended(&self) -> bool {
        self.ended
    }

    /// Sets the stack id for the names and the metadata of the next files.
    pub fn set_stack(&mut self, stack_id: &str) {
        stack_id.clone_into(&mut self.spec.stack_id);
    }

    /// Sets the rate and the centre of the samples that follow. A change closes the open file and opens a new one.
    pub fn set_format(&mut self, rate_hz: Hz, centre_hz: Hz) {
        if self.ended
            || self.format == Some((rate_hz, centre_hz))
            || !rate_hz.is_finite()
            || rate_hz < 1.0
        {
            return;
        }
        if self.format.is_some() {
            self.spool.send(0);
            self.write_gaps();
            self.spool.op(Op::Close {
                slot: 0,
                keep: true,
                log: None,
            });
        }
        self.format = Some((rate_hz, centre_hz));
        self.live.rate_bits.store(rate_hz.to_bits(), Ordering::Relaxed);
        self.open(rate_hz, centre_hz);
    }

    /// Adds samples in the format of the last [`IqFeed::set_format`]. Samples before the first one are left out.
    pub fn push(&mut self, iq: &[Complex32]) {
        if self.ended {
            return;
        }
        if self.live.stopping() {
            self.end();
            return;
        }
        let Some((rate, _)) = self.format else {
            return;
        };
        let width = self.spec.sample.bytes();
        let mut rest = iq;
        while !rest.is_empty() {
            let Some(block) = self.spool.block_waiting(0) else {
                self.live.add_dropped(rest.len() as u64);
                break;
            };
            let n = ((block.capacity() - block.len()) / width).min(rest.len());
            let (now, tail) = rest.split_at(n);
            self.spec.sample.encode(now, block);
            rest = tail;
            self.total += n as u64;
            self.in_file += n as u64;
            self.live.add_frames(n as u64);
            if block.capacity() - block.len() < width {
                self.spool.send(0);
            }
        }
        if self.spec.max_s > 0.0 && self.total as f64 >= self.spec.max_s * rate {
            self.end();
        }
    }

    /// `lost` samples are missing before the next sample: the radio or the session lost them. A SigMF file marks the
    /// place with an annotation.
    pub fn gap(&mut self, lost: u64) {
        if self.ended || lost == 0 || self.format.is_none() {
            return;
        }
        self.gaps.push((self.in_file, lost));
        self.live.gaps.fetch_add(1, Ordering::Relaxed);
        self.live.gap_samples.fetch_add(lost, Ordering::Relaxed);
    }

    /// Writes the metadata of the open file again with its gaps, if it has any.
    fn write_gaps(&mut self) {
        let gaps = std::mem::take(&mut self.gaps);
        if let (Some(meta), false) = (self.meta.as_mut(), gaps.is_empty()) {
            meta.gaps = gaps;
            self.spool.op(Op::Meta {
                slot: 0,
                ext: "sigmf-meta",
                meta: sigmf::write(meta),
            });
        }
    }

    fn open(&mut self, rate: Hz, centre: Hz) {
        let now = self.clock.now();
        let base = iq_name(
            &self.spec.prefix,
            &self.spec.stack_id,
            centre,
            rate,
            &stamp(now, self.spec.utc_offset_min),
        );
        self.in_file = 0;
        self.gaps.clear();
        self.meta = match self.spec.container {
            IqContainer::Sigmf => Some(sigmf::Meta {
                datatype: self.spec.sample.sigmf_datatype().into(),
                rate_hz: rate,
                centre_hz: centre,
                datetime: iso_utc(now),
                description: self.spec.stack_id.clone(),
                annotation: None,
                gaps: Vec::new(),
            }),
            IqContainer::Raw => None,
        };
        let (name, meta) = match &self.meta {
            Some(m) => (format!("{base}.sigmf-data"), Some(sigmf::write(m))),
            None => (format!("{base}.{}", self.spec.sample.extension()), None),
        };
        match self.spec.folder.join(&name) {
            Ok(path) => self.spool.op(Op::Open {
                slot: 0,
                path,
                encoding: Encoding::Plain,
                meta: meta.map(|m| ("sigmf-meta", m)),
            }),
            Err(_) => self.end(),
        }
    }

    fn end(&mut self) {
        if self.ended {
            return;
        }
        self.spool.send(0);
        self.write_gaps();
        self.spool.op(Op::Finish);
        self.ended = true;
        self.live.ended.store(true, Ordering::Relaxed);
    }
}

impl Drop for IqFeed {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spool::Note;
    use sdrkit_core::storage::{StoreEntry, StoreReader, StoreWriter, WriteMode};
    use sdrkit_core::{MemStorage, StorageError};
    use std::time::Duration;

    /// Storage whose writes take a while, slower than the samples come.
    struct Slow(MemStorage);

    struct SlowWriter(Box<dyn StoreWriter>);

    impl std::io::Write for SlowWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            std::thread::sleep(Duration::from_millis(3));
            self.0.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }

    impl std::io::Seek for SlowWriter {
        fn seek(&mut self, to: std::io::SeekFrom) -> std::io::Result<u64> {
            self.0.seek(to)
        }
    }

    impl StoreWriter for SlowWriter {
        fn set_len(&mut self, len: u64) -> Result<(), StorageError> {
            self.0.set_len(len)
        }
        fn sync(&mut self) -> Result<(), StorageError> {
            self.0.sync()
        }
    }

    impl Storage for Slow {
        fn read(&self, p: &StorePath) -> Result<Vec<u8>, StorageError> {
            self.0.read(p)
        }
        fn write(&self, p: &StorePath, data: &[u8]) -> Result<(), StorageError> {
            self.0.write(p, data)
        }
        fn open_writer(&self, p: &StorePath, mode: WriteMode) -> Result<Box<dyn StoreWriter>, StorageError> {
            Ok(Box::new(SlowWriter(self.0.open_writer(p, mode)?)))
        }
        fn open_reader(&self, p: &StorePath) -> Result<Box<dyn StoreReader>, StorageError> {
            self.0.open_reader(p)
        }
        fn remove(&self, p: &StorePath) -> Result<(), StorageError> {
            self.0.remove(p)
        }
        fn remove_tree(&self, p: &StorePath) -> Result<(), StorageError> {
            self.0.remove_tree(p)
        }
        fn rename(&self, from: &StorePath, to: &StorePath) -> Result<(), StorageError> {
            self.0.rename(from, to)
        }
        fn list(&self, folder: &StorePath) -> Result<Vec<StoreEntry>, StorageError> {
            self.0.list(folder)
        }
        fn stat(&self, p: &StorePath) -> Result<StoreEntry, StorageError> {
            self.0.stat(p)
        }
        fn free_space(&self) -> Result<Option<u64>, StorageError> {
            self.0.free_space()
        }
    }

    #[test]
    fn a_slow_writer_keeps_every_sample_and_the_gaps_are_marked() {
        let storage = Arc::new(Slow(MemStorage::new()));
        let spec = IqSpec {
            folder: StorePath::new("rec").unwrap(),
            prefix: "t".into(),
            container: IqContainer::Sigmf,
            sample: IqSample::Cs16,
            stack_id: "test".into(),
            utc_offset_min: 0,
            max_s: 0.0,
            // The smallest queue: two blocks.
            queue_bytes: 1,
        };
        let (mut feed, mut control) =
            start(spec, "session.s1", storage.clone(), Clock::fixed(1.7e9)).unwrap();
        feed.set_format(1e6, 100e6);
        let block = vec![Complex32::new(0.25, -0.25); 16_384];
        for i in 0..100 {
            if i == 40 {
                feed.gap(500_000);
            }
            feed.push(&block);
        }
        drop(feed);
        let mut done = false;
        for _ in 0..60_000 {
            match control.writer.note() {
                Some(Note::Done) => {
                    done = true;
                    break;
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        assert!(done, "the writer ends");
        let files = storage.list(&StorePath::new("rec").unwrap()).unwrap();
        let data = files.iter().find(|f| f.name.ends_with(".sigmf-data")).unwrap();
        let meta = files.iter().find(|f| f.name.ends_with(".sigmf-meta")).unwrap();
        let path = |name: &str| StorePath::new(&format!("rec/{name}")).unwrap();
        assert_eq!(storage.read(&path(&data.name)).unwrap().len(), 100 * 16_384 * 4);
        let m = sigmf::read(&storage.read(&path(&meta.name)).unwrap()).unwrap();
        assert_eq!(m.gaps, [(40 * 16_384, 500_000)]);
        assert_eq!(control.live.dropped.load(Ordering::Relaxed), 0);
        assert_eq!(
            crate::service::iq_summary(&control.live),
            "recorded 1.64 s, 1 gap, 0.50 s lost at the radio"
        );
    }
}
