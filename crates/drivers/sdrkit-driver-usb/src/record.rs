//! A bus that passes every call to another bus and writes each exchange into a [`Script`].

use crate::script::{BulkItem, BulkKind, Entry, Payload, Script, Step};
use crate::{
    Availability, BulkIn, BulkOut, Setup, UsbBus, UsbDevice, UsbDeviceInfo, UsbError, UsbEvent,
    UsbId, UsbWatch,
};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// What a recording keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordOptions {
    /// Payloads of bulk transfers up to this many bytes are kept whole; a longer one is kept as its length, its CRC
    /// and its first `keep` bytes.
    pub keep: usize,
    /// Keep the time between bulk IN transfers as the `after` of each item.
    pub timing: bool,
}

impl Default for RecordOptions {
    fn default() -> Self {
        RecordOptions {
            keep: 4096,
            timing: false,
        }
    }
}

struct Recorder {
    script: Mutex<Script>,
    known: Mutex<HashSet<UsbId>>,
    options: RecordOptions,
}

impl Recorder {
    fn lock(&self) -> MutexGuard<'_, Script> {
        self.script
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn step(&self, step: Step) {
        self.lock().entries.push(Entry::Step(step));
    }

    /// Adds a step and returns its index among the entries.
    fn step_at(&self, step: Step) -> usize {
        let mut s = self.lock();
        s.entries.push(Entry::Step(step));
        s.entries.len() - 1
    }

    /// Sets the reply of the bulk OUT step at entry `at`.
    fn bulk_reply(&self, at: usize, r: Result<usize, UsbError>) {
        if let Some(Entry::Step(Step::BulkOut { reply, .. })) = self.lock().entries.get_mut(at) {
            *reply = r;
        }
    }

    /// Notes a device the bus showed: a `device` line before the first exchange, a plug step after it.
    fn saw(&self, info: &UsbDeviceInfo) {
        let mut known = self
            .known
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !known.insert(info.id) {
            return;
        }
        let mut s = self.lock();
        if s.entries.is_empty() {
            s.devices.push(info.clone());
        } else {
            s.entries
                .push(Entry::Step(Step::Plug { info: info.clone() }));
        }
    }

    fn left(&self, id: UsbId) {
        let mut known = self
            .known
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if known.remove(&id) {
            self.step(Step::Unplug { device: id });
        }
    }
}

/// A bus that passes every call to another bus and records it. The recording replays with `FakeUsb`.
#[derive(Clone)]
pub struct RecordingUsb {
    inner: Arc<dyn UsbBus>,
    rec: Arc<Recorder>,
}

impl std::fmt::Debug for RecordingUsb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecordingUsb")
    }
}

impl RecordingUsb {
    /// Records the calls to `inner`.
    pub fn new(inner: Arc<dyn UsbBus>, options: RecordOptions) -> RecordingUsb {
        RecordingUsb {
            inner,
            rec: Arc::new(Recorder {
                script: Mutex::new(Script::new()),
                known: Mutex::new(HashSet::new()),
                options,
            }),
        }
    }

    /// The exchanges recorded so far.
    pub fn script(&self) -> Script {
        self.rec.lock().clone()
    }

    /// Writes the exchanges recorded so far to the file at `path`.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        self.script().save(path)
    }
}

fn unit<T>(r: &Result<T, UsbError>) -> Result<(), UsbError> {
    r.as_ref().map(|_| ()).map_err(Clone::clone)
}

impl UsbBus for RecordingUsb {
    fn availability(&self) -> Availability {
        self.inner.availability()
    }

    fn devices(&self) -> Result<Vec<UsbDeviceInfo>, UsbError> {
        let list = self.inner.devices()?;
        for d in &list {
            self.rec.saw(d);
        }
        Ok(list)
    }

    fn watch(&self) -> Result<Box<dyn UsbWatch>, UsbError> {
        Ok(Box::new(RecWatch {
            inner: self.inner.watch()?,
            rec: self.rec.clone(),
        }))
    }

    fn open(&self, id: UsbId) -> Result<Box<dyn UsbDevice>, UsbError> {
        let r = self.inner.open(id);
        if let Ok(d) = &r {
            self.rec.saw(d.info());
        }
        self.rec.step(Step::Open {
            device: id,
            reply: unit(&r),
        });
        Ok(Box::new(RecDevice {
            inner: r?,
            rec: self.rec.clone(),
        }))
    }
}

struct RecWatch {
    inner: Box<dyn UsbWatch>,
    rec: Arc<Recorder>,
}

impl UsbWatch for RecWatch {
    fn next(&mut self, timeout: Duration) -> Result<Option<UsbEvent>, UsbError> {
        let e = self.inner.next(timeout)?;
        match &e {
            Some(UsbEvent::Arrived(info)) => self.rec.saw(info),
            Some(UsbEvent::Left(id)) => self.rec.left(*id),
            None => {}
        }
        Ok(e)
    }
}

struct RecDevice {
    inner: Box<dyn UsbDevice>,
    rec: Arc<Recorder>,
}

impl UsbDevice for RecDevice {
    fn info(&self) -> &UsbDeviceInfo {
        self.inner.info()
    }

    fn claim(&mut self, interface: u8) -> Result<(), UsbError> {
        let r = self.inner.claim(interface);
        self.rec.step(Step::Claim {
            device: self.info().id,
            interface,
            reply: r.clone(),
        });
        r
    }

    fn control_in(
        &mut self,
        setup: Setup,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        let r = self.inner.control_in(setup, buf, timeout);
        self.rec.step(Step::ControlIn {
            device: self.info().id,
            setup: setup.input(),
            length: u16::try_from(buf.len()).unwrap_or(u16::MAX),
            reply: r.as_ref().map(|&n| buf[..n].to_vec()).map_err(Clone::clone),
        });
        r
    }

    fn control_out(
        &mut self,
        setup: Setup,
        data: &[u8],
        timeout: Duration,
    ) -> Result<(), UsbError> {
        let r = self.inner.control_out(setup, data, timeout);
        self.rec.step(Step::ControlOut {
            device: self.info().id,
            setup: setup.output(),
            data: data.to_vec(),
            reply: r.clone(),
        });
        r
    }

    fn bulk_in(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkIn>, UsbError> {
        let inner = self.inner.bulk_in(endpoint, transfer_size, queued)?;
        Ok(Box::new(RecBulkIn {
            inner,
            rec: self.rec.clone(),
            device: self.info().id,
            endpoint,
            last: Instant::now(),
        }))
    }

    fn bulk_out(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        let r = self.inner.bulk_out(endpoint, data, timeout);
        self.rec.step(Step::BulkOut {
            device: self.info().id,
            endpoint,
            data: Payload::of(data, self.rec.options.keep),
            reply: r.clone(),
        });
        r
    }

    fn bulk_out_queue(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkOut>, UsbError> {
        let inner = self.inner.bulk_out_queue(endpoint, transfer_size, queued)?;
        Ok(Box::new(RecBulkOut {
            inner,
            rec: self.rec.clone(),
            device: self.info().id,
            endpoint,
            steps: VecDeque::with_capacity(queued),
            taken: false,
        }))
    }

    fn clear_halt(&mut self, endpoint: u8) -> Result<(), UsbError> {
        let r = self.inner.clear_halt(endpoint);
        self.rec.step(Step::ClearHalt {
            device: self.info().id,
            endpoint,
            reply: r.clone(),
        });
        r
    }

    fn reset(&mut self) -> Result<(), UsbError> {
        let r = self.inner.reset();
        self.rec.step(Step::Reset {
            device: self.info().id,
            reply: r.clone(),
        });
        r
    }
}

/// A queued bulk OUT writer that records each submit as a `bout` step; the step takes its reply when the
/// transfer completes. A cancelled transfer keeps the reply of a whole transfer.
struct RecBulkOut {
    inner: Box<dyn BulkOut>,
    rec: Arc<Recorder>,
    device: UsbId,
    endpoint: u8,
    /// The entries of the steps in flight, oldest first.
    steps: VecDeque<usize>,
    /// A buffer was taken and not yet submitted.
    taken: bool,
}

impl BulkOut for RecBulkOut {
    fn endpoint(&self) -> u8 {
        self.endpoint
    }

    fn transfer_size(&self) -> usize {
        self.inner.transfer_size()
    }

    fn in_flight(&self) -> usize {
        self.inner.in_flight()
    }

    fn buffer(&mut self) -> Option<&mut [u8]> {
        let b = self.inner.buffer();
        self.taken |= b.is_some();
        b
    }

    fn submit(&mut self, len: usize) -> Result<(), UsbError> {
        let data = match self.inner.buffer().filter(|_| self.taken) {
            Some(b) if len <= b.len() => Payload::of(&b[..len], self.rec.options.keep),
            _ => return self.inner.submit(len),
        };
        self.inner.submit(len)?;
        self.taken = false;
        let at = self.rec.step_at(Step::BulkOut {
            device: self.device,
            endpoint: self.endpoint,
            reply: Ok(data.len()),
            data,
        });
        self.steps.push_back(at);
        Ok(())
    }

    fn completed(&mut self, timeout: Duration) -> Option<Result<usize, UsbError>> {
        let r = self.inner.completed(timeout)?;
        if r != Err(UsbError::Timeout) {
            if let Some(at) = self.steps.pop_front() {
                self.rec.bulk_reply(at, r.clone());
            }
        }
        Some(r)
    }

    fn cancel(&mut self) {
        self.inner.cancel();
        self.steps.clear();
    }
}

struct RecBulkIn {
    inner: Box<dyn BulkIn>,
    rec: Arc<Recorder>,
    device: UsbId,
    endpoint: u8,
    last: Instant,
}

impl BulkIn for RecBulkIn {
    fn endpoint(&self) -> u8 {
        self.endpoint
    }

    fn transfer_size(&self) -> usize {
        self.inner.transfer_size()
    }

    /// Records each transfer as a bulk item; a timeout is not recorded, an unplug is recorded as `!unplug`.
    fn next(&mut self, timeout: Duration) -> Result<&[u8], UsbError> {
        let r = self.inner.next(timeout);
        let kind = match &r {
            Ok(data) => BulkKind::Data(Payload::of(data, self.rec.options.keep)),
            Err(UsbError::Timeout) => return r,
            Err(UsbError::Unplugged) => BulkKind::Unplug,
            Err(e) => BulkKind::Error(e.clone()),
        };
        let now = Instant::now();
        let after = if self.rec.options.timing {
            now - self.last
        } else {
            Duration::ZERO
        };
        self.last = now;
        self.rec.lock().entries.push(Entry::Bulk(BulkItem {
            device: self.device,
            endpoint: self.endpoint,
            after,
            repeat: 1,
            kind,
        }));
        r
    }

    fn clear_halt(&mut self) -> Result<(), UsbError> {
        let r = self.inner.clear_halt();
        self.rec.step(Step::ClearHalt {
            device: self.device,
            endpoint: self.endpoint,
            reply: r.clone(),
        });
        r
    }
}
