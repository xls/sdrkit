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

/// The feed of an IQ recording. The engine passes it the samples of the stream. [`IqFeed::push`] never waits on storage and
/// allocates nothing once its file is open.
pub struct IqFeed {
    spec: IqSpec,
    clock: Clock,
    spool: Spool<Vec<u8>>,
    live: Arc<Live>,
    session: String,
    format: Option<(Hz, Hz)>,
    total: u64,
    ended: bool,
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
            self.spool.op(Op::Close {
                slot: 0,
                keep: true,
                log: None,
            });
        }
        self.format = Some((rate_hz, centre_hz));
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
            let Some(block) = self.spool.block(0) else {
                self.live.add_dropped(rest.len() as u64);
                break;
            };
            let n = ((block.capacity() - block.len()) / width).min(rest.len());
            let (now, tail) = rest.split_at(n);
            self.spec.sample.encode(now, block);
            rest = tail;
            self.total += n as u64;
            self.live.add_frames(n as u64);
            if block.capacity() - block.len() < width {
                self.spool.send(0);
            }
        }
        if self.spec.max_s > 0.0 && self.total as f64 >= self.spec.max_s * rate {
            self.end();
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
        let (name, meta) = match self.spec.container {
            IqContainer::Sigmf => (
                format!("{base}.sigmf-data"),
                Some(sigmf::write(&sigmf::Meta {
                    datatype: self.spec.sample.sigmf_datatype().into(),
                    rate_hz: rate,
                    centre_hz: centre,
                    datetime: iso_utc(now),
                    description: self.spec.stack_id.clone(),
                    annotation: None,
                })),
            ),
            IqContainer::Raw => (format!("{base}.{}", self.spec.sample.extension()), None),
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
