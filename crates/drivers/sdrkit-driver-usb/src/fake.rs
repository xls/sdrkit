//! A USB bus in memory that replays a [`Script`] and reports the first call that does not follow it.

use crate::script::{BulkItem, BulkKind, Entry, Mode, Script, Step};
use crate::text::call_text;
use crate::{
    Availability, BulkIn, BulkOut, Payload, Setup, UsbBus, UsbDevice, UsbDeviceInfo, UsbError,
    UsbEvent, UsbId, UsbWatch,
};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// A call that does not follow the script, or a step of a strict script that no call took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// The step that was expected, counted from 1 among the steps of the script; `None` when no step was left.
    pub step: Option<usize>,
    /// The expected step as a script line without its reply.
    pub expected: Option<String>,
    /// The call as a script line, or `None` for a step that no call took.
    pub got: Option<String>,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.step.unwrap_or(0);
        match (&self.expected, &self.got) {
            (Some(e), Some(g)) => write!(f, "step {n}: expected `{e}`, got `{g}`"),
            (Some(e), None) => write!(f, "step {n}: `{e}` was not called"),
            (None, Some(g)) => write!(f, "no step is left for `{g}`"),
            (None, None) => write!(f, "no mismatch"),
        }
    }
}

/// The read position in the bulk items of one endpoint.
#[derive(Default)]
struct Queue {
    items: Vec<BulkItem>,
    item: usize,
    rep: u32,
    offset: usize,
}

struct State {
    mode: Mode,
    steps: Vec<Step>,
    used: Vec<bool>,
    cursor: usize,
    attached: Vec<UsbDeviceInfo>,
    streams: HashMap<(UsbId, u8), Queue>,
    events: Vec<UsbEvent>,
    mismatch: Option<Mismatch>,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A USB bus that replays a [`Script`]. Clones share the same bus.
///
/// Calls are checked against the steps in the order of the [`Mode`] of the script. In strict mode, the first call
/// that is not the next step fails with [`UsbError::Script`], and so does every call after it. Bulk IN data comes
/// from the bulk items of each endpoint, apart from the steps. [`FakeUsb::check`] reports the first mismatch or a
/// step that no call took.
#[derive(Clone)]
pub struct FakeUsb {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for FakeUsb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FakeUsb")
    }
}

impl FakeUsb {
    /// A bus that replays `script`. Unplug and plug steps at the start of a strict script take effect at once.
    pub fn new(script: Script) -> FakeUsb {
        let mut steps = Vec::new();
        let mut streams: HashMap<(UsbId, u8), Queue> = HashMap::new();
        for e in script.entries {
            match e {
                Entry::Step(s) => steps.push(s),
                Entry::Bulk(b) => streams
                    .entry((b.device, b.endpoint))
                    .or_default()
                    .items
                    .push(b),
            }
        }
        let mut state = State {
            mode: script.mode,
            used: vec![false; steps.len()],
            steps,
            cursor: 0,
            attached: script.devices,
            streams,
            events: Vec::new(),
            mismatch: None,
        };
        if state.mode == Mode::Strict {
            run_events(&mut state);
        }
        FakeUsb {
            shared: Arc::new(Shared {
                state: Mutex::new(state),
                wake: Condvar::new(),
            }),
        }
    }

    /// A bus that replays the script in `text`.
    ///
    /// # Errors
    ///
    /// When the text is not a script.
    pub fn from_text(text: &str) -> Result<FakeUsb, crate::ParseError> {
        Script::parse(text).map(FakeUsb::new)
    }

    /// Unplugs device `id` now: its open handles and streams fail with [`UsbError::Unplugged`], and watches see it
    /// leave.
    pub fn unplug(&self, id: UsbId) {
        let mut st = self.shared.lock();
        apply(&mut st, &Step::Unplug { device: id });
        self.shared.wake.notify_all();
    }

    /// Attaches `info` now; watches see it arrive.
    pub fn plug(&self, info: UsbDeviceInfo) {
        let mut st = self.shared.lock();
        apply(&mut st, &Step::Plug { info });
        self.shared.wake.notify_all();
    }

    /// The first call that did not follow the script.
    pub fn mismatch(&self) -> Option<Mismatch> {
        self.shared.lock().mismatch.clone()
    }

    /// Checks that every call followed the script and, in strict mode, that every step was taken.
    ///
    /// # Errors
    ///
    /// The first mismatch, else the first step that no call took.
    pub fn check(&self) -> Result<(), Mismatch> {
        let st = self.shared.lock();
        if let Some(m) = &st.mismatch {
            return Err(m.clone());
        }
        if st.mode == Mode::Strict {
            if let Some(i) = (0..st.steps.len()).find(|&i| !st.used[i]) {
                return Err(Mismatch {
                    step: Some(i + 1),
                    expected: Some(call_text(&st.steps[i])),
                    got: None,
                });
            }
        }
        Ok(())
    }

    /// Takes the step for `call` and returns its index. `Ok(None)` is a call of a lenient script that matches no
    /// step. With `report` false a call that does not match is not recorded as a mismatch (a call on an unplugged
    /// device). A call that matches allocates nothing.
    fn take(
        &self,
        st: &mut State,
        call: &Call<'_>,
        report: bool,
    ) -> Result<Option<usize>, UsbError> {
        if let Some(m) = &st.mismatch {
            if st.mode == Mode::Strict {
                return Err(UsbError::Script(m.to_string()));
            }
        }
        let i = match st.mode {
            Mode::Strict => {
                run_events(st);
                match st.steps.get(st.cursor) {
                    Some(s) if call.matches(s) => st.cursor,
                    other => {
                        if !report {
                            return Ok(None);
                        }
                        let m = Mismatch {
                            step: other.map(|_| st.cursor + 1),
                            expected: other.map(call_text),
                            got: Some(call_text(&call.step())),
                        };
                        let e = UsbError::Script(m.to_string());
                        st.mismatch = Some(m);
                        return Err(e);
                    }
                }
            }
            Mode::Lenient => {
                let ahead = (st.cursor..st.steps.len()).find(|&i| {
                    !st.used[i] && !st.steps[i].is_event() && call.matches(&st.steps[i])
                });
                let Some(i) = ahead else {
                    return Ok((0..st.steps.len())
                        .rev()
                        .find(|&i| st.used[i] && call.matches(&st.steps[i])));
                };
                for j in st.cursor..i {
                    if st.steps[j].is_event() && !st.used[j] {
                        st.used[j] = true;
                        let s = st.steps[j].clone();
                        apply(st, &s);
                    }
                }
                i
            }
        };
        st.used[i] = true;
        st.cursor = i + 1;
        run_events(st);
        self.shared.wake.notify_all();
        Ok(Some(i))
    }

    /// Records a call of a lenient script that needs a reply and matches no step.
    fn no_step(st: &mut State, call: &Call<'_>) -> UsbError {
        let m = Mismatch {
            step: None,
            expected: None,
            got: Some(call_text(&call.step())),
        };
        let e = UsbError::Script(m.to_string());
        st.mismatch.get_or_insert(m);
        e
    }
}

/// A call as the fake receives it, with borrowed data, so that matching a step allocates nothing.
#[derive(Clone, Copy)]
enum Call<'a> {
    Open(UsbId),
    Claim(UsbId, u8),
    ControlIn(UsbId, Setup, u16),
    ControlOut(UsbId, Setup, &'a [u8]),
    BulkOut(UsbId, u8, &'a [u8]),
    ClearHalt(UsbId, u8),
    Reset(UsbId),
}

impl Call<'_> {
    /// True if `step` is this call.
    fn matches(&self, step: &Step) -> bool {
        match (*self, step) {
            (Call::Open(a), Step::Open { device, .. }) => a == *device,
            (
                Call::Claim(a, i),
                Step::Claim {
                    device, interface, ..
                },
            ) => a == *device && i == *interface,
            (
                Call::ControlIn(a, s, l),
                Step::ControlIn {
                    device,
                    setup,
                    length,
                    ..
                },
            ) => a == *device && s == *setup && l == *length,
            (
                Call::ControlOut(a, s, d),
                Step::ControlOut {
                    device,
                    setup,
                    data,
                    ..
                },
            ) => a == *device && s == *setup && d == data.as_slice(),
            (
                Call::BulkOut(a, e, d),
                Step::BulkOut {
                    device,
                    endpoint,
                    data,
                    ..
                },
            ) => a == *device && e == *endpoint && data.matches(d),
            (
                Call::ClearHalt(a, e),
                Step::ClearHalt {
                    device, endpoint, ..
                },
            ) => a == *device && e == *endpoint,
            (Call::Reset(a), Step::Reset { device, .. }) => a == *device,
            _ => false,
        }
    }

    /// The call as a step, for a report.
    fn step(&self) -> Step {
        match *self {
            Call::Open(device) => Step::Open {
                device,
                reply: Ok(()),
            },
            Call::Claim(device, interface) => Step::Claim {
                device,
                interface,
                reply: Ok(()),
            },
            Call::ControlIn(device, setup, length) => Step::ControlIn {
                device,
                setup,
                length,
                reply: Ok(Vec::new()),
            },
            Call::ControlOut(device, setup, data) => Step::ControlOut {
                device,
                setup,
                data: data.to_vec(),
                reply: Ok(()),
            },
            Call::BulkOut(device, endpoint, data) => Step::BulkOut {
                device,
                endpoint,
                data: Payload::Full(data.to_vec()),
                reply: Ok(data.len()),
            },
            Call::ClearHalt(device, endpoint) => Step::ClearHalt {
                device,
                endpoint,
                reply: Ok(()),
            },
            Call::Reset(device) => Step::Reset {
                device,
                reply: Ok(()),
            },
        }
    }
}

/// Runs the unplug and plug steps at the cursor.
fn run_events(st: &mut State) {
    while let Some(s) = st.steps.get(st.cursor) {
        if !s.is_event() {
            break;
        }
        let s = s.clone();
        st.used[st.cursor] = true;
        st.cursor += 1;
        apply(st, &s);
    }
}

/// Applies an unplug or plug.
fn apply(st: &mut State, step: &Step) {
    match step {
        Step::Unplug { device } => {
            let before = st.attached.len();
            st.attached.retain(|d| d.id != *device);
            if st.attached.len() != before {
                st.events.push(UsbEvent::Left(*device));
            }
        }
        Step::Plug { info } => {
            st.attached.retain(|d| d.id != info.id);
            st.attached.push(info.clone());
            st.events.push(UsbEvent::Arrived(info.clone()));
        }
        _ => {}
    }
}

fn attached(st: &State, id: UsbId) -> bool {
    st.attached.iter().any(|d| d.id == id)
}

impl UsbBus for FakeUsb {
    fn availability(&self) -> Availability {
        Availability::Available
    }

    fn devices(&self) -> Result<Vec<UsbDeviceInfo>, UsbError> {
        Ok(self.shared.lock().attached.clone())
    }

    fn watch(&self) -> Result<Box<dyn UsbWatch>, UsbError> {
        let next = self.shared.lock().events.len();
        Ok(Box::new(FakeWatch {
            shared: self.shared.clone(),
            next,
        }))
    }

    fn open(&self, id: UsbId) -> Result<Box<dyn UsbDevice>, UsbError> {
        let mut st = self.shared.lock();
        let call = Call::Open(id);
        let Some(info) = st.attached.iter().find(|d| d.id == id).cloned() else {
            self.take(&mut st, &call, false)?;
            return Err(UsbError::NoDevice);
        };
        if let Some(i) = self.take(&mut st, &call, true)? {
            if let Step::Open { reply: Err(e), .. } = &st.steps[i] {
                return Err(e.clone());
            }
        }
        Ok(Box::new(FakeDevice {
            bus: self.clone(),
            info,
        }))
    }
}

struct FakeWatch {
    shared: Arc<Shared>,
    next: usize,
}

impl UsbWatch for FakeWatch {
    fn next(&mut self, timeout: Duration) -> Result<Option<UsbEvent>, UsbError> {
        let deadline = Instant::now() + timeout;
        let mut st = self.shared.lock();
        loop {
            if let Some(e) = st.events.get(self.next) {
                self.next += 1;
                return Ok(Some(e.clone()));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            st = self
                .shared
                .wake
                .wait_timeout(st, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
}

#[derive(Clone)]
struct FakeDevice {
    bus: FakeUsb,
    info: UsbDeviceInfo,
}

/// The reply of a step that has no data.
fn unit_reply(step: &Step) -> Result<(), UsbError> {
    match step {
        Step::Open { reply, .. }
        | Step::Claim { reply, .. }
        | Step::ControlOut { reply, .. }
        | Step::ClearHalt { reply, .. }
        | Step::Reset { reply, .. } => reply.clone(),
        _ => Ok(()),
    }
}

impl FakeDevice {
    /// Takes the step for `call` and passes it, or `None` for a lenient script without one, to `reply` while the
    /// bus is locked. `Err(Unplugged)` when the device left.
    fn call<R>(
        &self,
        call: &Call<'_>,
        reply: impl FnOnce(Option<&Step>) -> R,
    ) -> Result<R, UsbError> {
        let mut st = self.bus.shared.lock();
        if !attached(&st, self.info.id) {
            self.bus.take(&mut st, call, false)?;
            return Err(UsbError::Unplugged);
        }
        let step = self.bus.take(&mut st, call, true)?;
        if step.is_none() && matches!(call, Call::ControlIn(..)) {
            return Err(FakeUsb::no_step(&mut st, call));
        }
        Ok(reply(step.map(|i| &st.steps[i])))
    }

    fn unit(&self, call: &Call<'_>) -> Result<(), UsbError> {
        self.call(call, |s| s.map_or(Ok(()), unit_reply))?
    }

    /// The reply of a bulk OUT transfer of `data`.
    fn bulk_reply(&self, endpoint: u8, data: &[u8]) -> Result<Result<usize, UsbError>, UsbError> {
        self.call(&Call::BulkOut(self.info.id, endpoint, data), |s| match s {
            Some(Step::BulkOut { reply, .. }) => reply.clone(),
            _ => Ok(data.len()),
        })
    }
}

impl UsbDevice for FakeDevice {
    fn info(&self) -> &UsbDeviceInfo {
        &self.info
    }

    fn claim(&mut self, interface: u8) -> Result<(), UsbError> {
        self.unit(&Call::Claim(self.info.id, interface))
    }

    /// Copies the reply of the step into `buf`; allocates nothing when the step matches.
    fn control_in(
        &mut self,
        setup: Setup,
        buf: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, UsbError> {
        let length = u16::try_from(buf.len())
            .map_err(|_| UsbError::Other("a control transfer is at most 65535 bytes".into()))?;
        self.call(
            &Call::ControlIn(self.info.id, setup.input(), length),
            |s| match s {
                Some(Step::ControlIn {
                    reply: Ok(data), ..
                }) => {
                    let n = data.len().min(buf.len());
                    buf[..n].copy_from_slice(&data[..n]);
                    Ok(n)
                }
                Some(Step::ControlIn { reply: Err(e), .. }) => Err(e.clone()),
                _ => Ok(0),
            },
        )?
    }

    fn control_out(
        &mut self,
        setup: Setup,
        data: &[u8],
        _timeout: Duration,
    ) -> Result<(), UsbError> {
        self.unit(&Call::ControlOut(self.info.id, setup.output(), data))
    }

    fn bulk_in(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkIn>, UsbError> {
        if transfer_size == 0 || queued == 0 || endpoint & 0x80 == 0 {
            return Err(UsbError::Other(
                "a bulk IN stream needs an IN endpoint, a transfer size and a queue".into(),
            ));
        }
        let mut st = self.bus.shared.lock();
        if !attached(&st, self.info.id) {
            return Err(UsbError::Unplugged);
        }
        st.streams.entry((self.info.id, endpoint)).or_default();
        Ok(Box::new(FakeBulkIn {
            shared: self.bus.shared.clone(),
            bus: self.bus.clone(),
            device: self.info.id,
            endpoint,
            buf: vec![0; transfer_size],
            last: Instant::now(),
        }))
    }

    fn bulk_out(
        &mut self,
        endpoint: u8,
        data: &[u8],
        _timeout: Duration,
    ) -> Result<usize, UsbError> {
        self.bulk_reply(endpoint, data)?
    }

    fn bulk_out_queue(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkOut>, UsbError> {
        if transfer_size == 0 || queued == 0 || endpoint & 0x80 != 0 {
            return Err(UsbError::Other(
                "a bulk OUT writer needs an OUT endpoint, a transfer size and a queue".into(),
            ));
        }
        if !attached(&self.bus.shared.lock(), self.info.id) {
            return Err(UsbError::Unplugged);
        }
        Ok(Box::new(FakeBulkOut {
            device: self.clone(),
            endpoint,
            size: transfer_size,
            bufs: vec![vec![0; transfer_size]; queued],
            free: (0..queued).rev().collect(),
            current: None,
            flight: VecDeque::with_capacity(queued),
        }))
    }

    fn clear_halt(&mut self, endpoint: u8) -> Result<(), UsbError> {
        self.unit(&Call::ClearHalt(self.info.id, endpoint))
    }

    fn reset(&mut self) -> Result<(), UsbError> {
        self.unit(&Call::Reset(self.info.id))
    }
}

/// A queued bulk OUT writer: each submit takes its `bout` step at once; [`BulkOut::completed`] gives the replies in
/// order.
struct FakeBulkOut {
    device: FakeDevice,
    endpoint: u8,
    size: usize,
    bufs: Vec<Vec<u8>>,
    /// Indices of the free buffers.
    free: Vec<usize>,
    current: Option<usize>,
    /// The buffers in flight with the replies of their steps.
    flight: VecDeque<(usize, Result<usize, UsbError>)>,
}

impl BulkOut for FakeBulkOut {
    fn endpoint(&self) -> u8 {
        self.endpoint
    }

    fn transfer_size(&self) -> usize {
        self.size
    }

    fn in_flight(&self) -> usize {
        self.flight.len()
    }

    fn buffer(&mut self) -> Option<&mut [u8]> {
        if self.current.is_none() {
            self.current = self.free.pop();
        }
        let i = self.current?;
        Some(&mut self.bufs[i][..])
    }

    fn submit(&mut self, len: usize) -> Result<(), UsbError> {
        if len > self.size {
            return Err(UsbError::Other(format!(
                "a transfer of {len} bytes is longer than the buffer of {}",
                self.size
            )));
        }
        let i = self
            .current
            .ok_or_else(|| UsbError::Other("no buffer was taken before the submit".into()))?;
        let reply = self
            .device
            .bulk_reply(self.endpoint, &self.bufs[i][..len])?;
        self.current = None;
        self.flight.push_back((i, reply));
        Ok(())
    }

    fn completed(&mut self, _timeout: Duration) -> Option<Result<usize, UsbError>> {
        let (i, reply) = self.flight.pop_front()?;
        self.free.push(i);
        Some(reply)
    }

    fn cancel(&mut self) {
        while let Some((i, _)) = self.flight.pop_front() {
            self.free.push(i);
        }
    }
}

struct FakeBulkIn {
    shared: Arc<Shared>,
    bus: FakeUsb,
    device: UsbId,
    endpoint: u8,
    buf: Vec<u8>,
    /// When the item last delivered became ready, or the start of the stream.
    last: Instant,
}

impl BulkIn for FakeBulkIn {
    fn endpoint(&self) -> u8 {
        self.endpoint
    }

    fn transfer_size(&self) -> usize {
        self.buf.len()
    }

    fn next(&mut self, timeout: Duration) -> Result<&[u8], UsbError> {
        let deadline = Instant::now() + timeout;
        let key = (self.device, self.endpoint);
        let mut st = self.shared.lock();
        let n = loop {
            if !attached(&st, self.device) {
                return Err(UsbError::Unplugged);
            }
            let now = Instant::now();
            let q = st.streams.get_mut(&key).ok_or(UsbError::Unplugged)?;
            let wait_until = match q.items.get(q.item) {
                None => deadline,
                Some(item) if q.offset == 0 && self.last + item.after > now => {
                    (self.last + item.after).min(deadline)
                }
                Some(item) => {
                    if q.offset == 0 {
                        self.last += item.after;
                    }
                    let repeat = item.repeat.max(1);
                    let outcome = match &item.kind {
                        BulkKind::Data(p) => {
                            let n = p.copy_to(q.offset, &mut self.buf);
                            Ok((n, q.offset + n >= p.len()))
                        }
                        BulkKind::Error(e) => Err(Some(e.clone())),
                        BulkKind::Unplug => Err(None),
                    };
                    let done = !matches!(outcome, Ok((_, false)));
                    if done {
                        q.offset = 0;
                        q.rep += 1;
                        if q.rep >= repeat {
                            q.rep = 0;
                            q.item += 1;
                        }
                    }
                    match outcome {
                        Ok((n, _)) => {
                            if !done {
                                q.offset += n;
                            }
                            break n;
                        }
                        Err(Some(e)) => return Err(e),
                        Err(None) => {
                            apply(
                                &mut st,
                                &Step::Unplug {
                                    device: self.device,
                                },
                            );
                            self.shared.wake.notify_all();
                            return Err(UsbError::Unplugged);
                        }
                    }
                }
            };
            if now >= deadline {
                return Err(UsbError::Timeout);
            }
            st = self
                .shared
                .wake
                .wait_timeout(st, wait_until.saturating_duration_since(now))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        };
        drop(st);
        Ok(&self.buf[..n])
    }

    fn clear_halt(&mut self) -> Result<(), UsbError> {
        FakeDevice {
            bus: self.bus.clone(),
            info: UsbDeviceInfo::new(self.device, 0, 0),
        }
        .clear_halt(self.endpoint)
    }
}
