//! The bus of the operating system through the `nusb` crate: usbfs on Linux and Android, IOKit on macOS, WinUSB on
//! Windows. Calls wait on the library's own blocking forms; no async runtime is involved.

use crate::{
    Availability, BulkIn, BulkOut, Setup, Speed, Unavailable, UsbBus, UsbDevice, UsbDeviceInfo,
    UsbError, UsbEvent, UsbId, UsbWatch,
};
use nusb::transfer::{
    Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Out, Recipient, TransferError,
};
use nusb::{Endpoint, ErrorKind, MaybeFuture};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// The devices this bus has given ids to.
#[derive(Default)]
struct Known {
    next: u64,
    ids: HashMap<nusb::DeviceId, UsbId>,
    infos: HashMap<UsbId, nusb::DeviceInfo>,
}

impl Known {
    /// The id of `d`, a new one for a device not seen before, and its description.
    fn insert(&mut self, d: nusb::DeviceInfo) -> UsbDeviceInfo {
        let Known { next, ids, infos } = self;
        let id = *ids.entry(d.id()).or_insert_with(|| {
            *next += 1;
            UsbId(*next)
        });
        let info = describe(id, &d);
        infos.insert(id, d);
        info
    }

    #[cfg(not(target_os = "android"))]
    fn remove(&mut self, dev: nusb::DeviceId) -> Option<UsbId> {
        let id = self.ids.remove(&dev)?;
        self.infos.remove(&id);
        Some(id)
    }
}

/// The description of a listed device, from what the system keeps; no I/O on the device.
fn describe(id: UsbId, d: &nusb::DeviceInfo) -> UsbDeviceInfo {
    let (bus, ports) = position(d);
    UsbDeviceInfo {
        id,
        vendor_id: d.vendor_id(),
        product_id: d.product_id(),
        manufacturer: d.manufacturer_string().map(str::to_owned),
        product: d.product_string().map(str::to_owned),
        serial: d.serial_number().map(str::to_owned),
        bus,
        ports,
        blocked: blocked(d),
        release: release(d),
        speed: speed(d),
    }
}

#[cfg(not(target_os = "android"))]
fn release(d: &nusb::DeviceInfo) -> u16 {
    d.device_version()
}

#[cfg(target_os = "android")]
fn release(_d: &nusb::DeviceInfo) -> u16 {
    0
}

#[cfg(not(target_os = "android"))]
fn speed(d: &nusb::DeviceInfo) -> Speed {
    match d.speed() {
        Some(nusb::Speed::Low) => Speed::Low,
        Some(nusb::Speed::Full) => Speed::Full,
        Some(nusb::Speed::High) => Speed::High,
        Some(nusb::Speed::Super) => Speed::Super,
        Some(nusb::Speed::SuperPlus) => Speed::SuperPlus,
        _ => Speed::Unknown,
    }
}

#[cfg(target_os = "android")]
fn speed(_d: &nusb::DeviceInfo) -> Speed {
    Speed::Unknown
}

#[cfg(not(target_os = "android"))]
fn position(d: &nusb::DeviceInfo) -> (String, Vec<u8>) {
    (d.bus_id().to_owned(), d.port_chain().to_vec())
}

#[cfg(target_os = "android")]
fn position(_d: &nusb::DeviceInfo) -> (String, Vec<u8>) {
    (String::new(), Vec::new())
}

/// Windows opens a device only through WinUSB (whole device, or an interface of a composite device).
#[cfg(target_os = "windows")]
fn blocked(d: &nusb::DeviceInfo) -> Option<Unavailable> {
    match d.driver() {
        Some(s) if s.eq_ignore_ascii_case("winusb") || s.eq_ignore_ascii_case("usbccgp") => None,
        _ => Some(Unavailable::NeedsWinUsb),
    }
}

#[cfg(not(target_os = "windows"))]
fn blocked(_d: &nusb::DeviceInfo) -> Option<Unavailable> {
    None
}

/// The error of opening or claiming.
fn open_err(e: nusb::Error) -> UsbError {
    match e.kind() {
        ErrorKind::Disconnected | ErrorKind::NotFound => UsbError::NoDevice,
        ErrorKind::PermissionDenied => UsbError::NoPermission,
        ErrorKind::Busy => UsbError::Busy,
        ErrorKind::Unsupported if cfg!(target_os = "windows") => UsbError::NeedsWinUsb,
        _ => UsbError::Other(e.to_string()),
    }
}

/// The error of a call on an open device.
fn dev_err(e: nusb::Error) -> UsbError {
    match e.kind() {
        ErrorKind::Disconnected => UsbError::Unplugged,
        ErrorKind::PermissionDenied => UsbError::NoPermission,
        ErrorKind::Busy => UsbError::Busy,
        _ => UsbError::Other(e.to_string()),
    }
}

fn transfer_err(e: TransferError) -> UsbError {
    match e {
        TransferError::Cancelled => UsbError::Timeout,
        TransferError::Stall => UsbError::Stall,
        TransferError::Disconnected => UsbError::Unplugged,
        e => UsbError::Other(e.to_string()),
    }
}

/// The type and recipient fields of `bmRequestType`.
fn request_kind(request_type: u8) -> Result<(ControlType, Recipient), UsbError> {
    let t = match (request_type >> 5) & 3 {
        0 => ControlType::Standard,
        1 => ControlType::Class,
        2 => ControlType::Vendor,
        _ => return Err(UsbError::Other("request type 3 is reserved".into())),
    };
    let r = match request_type & 0x1f {
        0 => Recipient::Device,
        1 => Recipient::Interface,
        2 => Recipient::Endpoint,
        _ => Recipient::Other,
    };
    Ok((t, r))
}

/// The USB bus of the operating system. Ids count from 1 in the order this bus first sees the devices.
#[derive(Default)]
pub struct NusbBus {
    known: Arc<Mutex<Known>>,
}

impl std::fmt::Debug for NusbBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NusbBus")
    }
}

impl NusbBus {
    /// The bus. Nothing is opened until a call.
    pub fn new() -> NusbBus {
        NusbBus::default()
    }

    fn lock(&self) -> MutexGuard<'_, Known> {
        self.known
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl UsbBus for NusbBus {
    fn availability(&self) -> Availability {
        Availability::Available
    }

    fn devices(&self) -> Result<Vec<UsbDeviceInfo>, UsbError> {
        let list = nusb::list_devices().wait().map_err(open_err)?;
        let mut known = self.lock();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for d in list {
            let info = known.insert(d);
            seen.insert(info.id);
            out.push(info);
        }
        known.ids.retain(|_, id| seen.contains(id));
        known.infos.retain(|id, _| seen.contains(id));
        Ok(out)
    }

    #[cfg(not(target_os = "android"))]
    fn watch(&self) -> Result<Box<dyn UsbWatch>, UsbError> {
        let stream = nusb::watch_devices().map_err(dev_err)?;
        Ok(Box::new(NusbWatch {
            stream,
            known: self.known.clone(),
        }))
    }

    #[cfg(target_os = "android")]
    fn watch(&self) -> Result<Box<dyn UsbWatch>, UsbError> {
        Err(UsbError::Other(
            "this system reports USB arrivals to the app, not to the library".into(),
        ))
    }

    fn open(&self, id: UsbId) -> Result<Box<dyn UsbDevice>, UsbError> {
        let mut found = self.lock().infos.get(&id).cloned();
        if found.is_none() {
            self.devices()?;
            found = self.lock().infos.get(&id).cloned();
        }
        let d = found.ok_or(UsbError::NoDevice)?;
        let info = describe(id, &d);
        if info.blocked == Some(Unavailable::NeedsWinUsb) {
            return Err(UsbError::NeedsWinUsb);
        }
        let dev = d.open().wait().map_err(open_err)?;
        configure(&dev);
        Ok(Box::new(NusbDevice {
            dev,
            info,
            claimed: Vec::new(),
            outs: HashMap::new(),
            spare: None,
        }))
    }
}

/// Selects the first configuration of a device that has none active; vendor-class devices can come up that way.
#[cfg(not(target_os = "windows"))]
fn configure(dev: &nusb::Device) {
    if dev.active_configuration().is_err() {
        if let Some(c) = dev.configurations().next() {
            let _ = dev.set_configuration(c.configuration_value()).wait();
        }
    }
}

#[cfg(target_os = "windows")]
fn configure(_dev: &nusb::Device) {}

#[cfg(not(target_os = "android"))]
struct NusbWatch {
    stream: nusb::hotplug::HotplugWatch,
    known: Arc<Mutex<Known>>,
}

/// Wakes the thread that polls a watch.
#[cfg(not(target_os = "android"))]
struct Unpark(std::thread::Thread);

#[cfg(not(target_os = "android"))]
impl std::task::Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

#[cfg(not(target_os = "android"))]
impl UsbWatch for NusbWatch {
    fn next(&mut self, timeout: Duration) -> Result<Option<UsbEvent>, UsbError> {
        use futures_core::Stream;
        use std::task::{Context, Poll};
        let deadline = std::time::Instant::now() + timeout;
        let waker = std::task::Waker::from(Arc::new(Unpark(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        loop {
            match std::pin::Pin::new(&mut self.stream).poll_next(&mut cx) {
                Poll::Ready(Some(nusb::hotplug::HotplugEvent::Connected(d))) => {
                    let mut known = self
                        .known
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    return Ok(Some(UsbEvent::Arrived(known.insert(d))));
                }
                Poll::Ready(Some(nusb::hotplug::HotplugEvent::Disconnected(dev))) => {
                    let mut known = self
                        .known
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(id) = known.remove(dev) {
                        return Ok(Some(UsbEvent::Left(id)));
                    }
                }
                Poll::Ready(None) => return Err(UsbError::Other("the USB watch stopped".into())),
                Poll::Pending => {
                    let now = std::time::Instant::now();
                    if now >= deadline {
                        return Ok(None);
                    }
                    std::thread::park_timeout(deadline - now);
                }
            }
        }
    }
}

struct NusbDevice {
    dev: nusb::Device,
    info: UsbDeviceInfo,
    claimed: Vec<nusb::Interface>,
    outs: HashMap<u8, Endpoint<Bulk, Out>>,
    spare: Option<Buffer>,
}

impl NusbDevice {
    /// The interface that carries control transfers: the first claimed one. Windows sends control transfers only
    /// through an interface.
    fn control_interface(&self) -> Result<Option<&nusb::Interface>, UsbError> {
        match self.claimed.first() {
            Some(i) => Ok(Some(i)),
            None if cfg!(target_os = "windows") => Err(UsbError::Other(
                "claim an interface before a control transfer (WinUSB)".into(),
            )),
            None => Ok(None),
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn device_in(&self, req: ControlIn, timeout: Duration) -> Result<Vec<u8>, TransferError> {
        self.dev.control_in(req, timeout).wait()
    }

    #[cfg(target_os = "windows")]
    fn device_in(&self, _req: ControlIn, _timeout: Duration) -> Result<Vec<u8>, TransferError> {
        Err(TransferError::InvalidArgument)
    }

    #[cfg(not(target_os = "windows"))]
    fn device_out(&self, req: ControlOut<'_>, timeout: Duration) -> Result<(), TransferError> {
        self.dev.control_out(req, timeout).wait()
    }

    #[cfg(target_os = "windows")]
    fn device_out(&self, _req: ControlOut<'_>, _timeout: Duration) -> Result<(), TransferError> {
        Err(TransferError::InvalidArgument)
    }

    /// Opens endpoint `address` on the claimed interface that has it.
    fn endpoint<T: nusb::transfer::EndpointType, D: nusb::transfer::EndpointDirection>(
        &self,
        address: u8,
    ) -> Result<Endpoint<T, D>, UsbError> {
        for i in &self.claimed {
            match i.endpoint::<T, D>(address) {
                Ok(e) => return Ok(e),
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(dev_err(e)),
            }
        }
        Err(UsbError::Other(format!(
            "no claimed interface has bulk endpoint {address:02x}"
        )))
    }

    /// Claims `interface`. Where a kernel driver can be detached (Linux, Android), a busy interface is claimed again
    /// with the driver detached; a failure of the detach is [`UsbError::KernelDriver`].
    fn claim_interface(&self, interface: u8) -> Result<nusb::Interface, UsbError> {
        let detach = cfg!(any(target_os = "linux", target_os = "android"));
        match self.dev.claim_interface(interface).wait() {
            Ok(i) => Ok(i),
            Err(e) if detach && e.kind() == ErrorKind::Busy => self
                .dev
                .detach_and_claim_interface(interface)
                .wait()
                .map_err(|e| match e.kind() {
                    ErrorKind::Busy => UsbError::Busy,
                    ErrorKind::Disconnected => UsbError::Unplugged,
                    ErrorKind::NotFound => UsbError::Other(e.to_string()),
                    _ => UsbError::KernelDriver,
                }),
            Err(e) => Err(claim_err(e)),
        }
    }
}

fn claim_err(e: nusb::Error) -> UsbError {
    match e.kind() {
        ErrorKind::Disconnected => UsbError::Unplugged,
        ErrorKind::PermissionDenied => UsbError::NoPermission,
        ErrorKind::Busy => UsbError::Busy,
        ErrorKind::Other | ErrorKind::Unsupported if cfg!(target_os = "windows") => {
            UsbError::NeedsWinUsb
        }
        _ => UsbError::Other(e.to_string()),
    }
}

impl UsbDevice for NusbDevice {
    fn info(&self) -> &UsbDeviceInfo {
        &self.info
    }

    fn claim(&mut self, interface: u8) -> Result<(), UsbError> {
        if self
            .claimed
            .iter()
            .any(|i| i.interface_number() == interface)
        {
            return Ok(());
        }
        let i = self.claim_interface(interface)?;
        self.claimed.push(i);
        Ok(())
    }

    /// nusb 0.2 returns each control reply in a vector of its own; the bytes are copied into `buf`.
    fn control_in(
        &mut self,
        setup: Setup,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        let (control_type, recipient) = request_kind(setup.request_type)?;
        let req = ControlIn {
            control_type,
            recipient,
            request: setup.request,
            value: setup.value,
            index: setup.index,
            length: u16::try_from(buf.len())
                .map_err(|_| UsbError::Other("a control transfer is at most 65535 bytes".into()))?,
        };
        let data = match self.control_interface()? {
            Some(i) => i.control_in(req, timeout).wait(),
            None => self.device_in(req, timeout),
        }
        .map_err(transfer_err)?;
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    fn control_out(
        &mut self,
        setup: Setup,
        data: &[u8],
        timeout: Duration,
    ) -> Result<(), UsbError> {
        let (control_type, recipient) = request_kind(setup.request_type)?;
        if data.len() > usize::from(u16::MAX) {
            return Err(UsbError::Other(
                "a control transfer is at most 65535 bytes".into(),
            ));
        }
        let req = ControlOut {
            control_type,
            recipient,
            request: setup.request,
            value: setup.value,
            index: setup.index,
            data,
        };
        match self.control_interface()? {
            Some(i) => i.control_out(req, timeout).wait(),
            None => self.device_out(req, timeout),
        }
        .map_err(transfer_err)
    }

    fn bulk_in(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkIn>, UsbError> {
        if transfer_size == 0 || queued == 0 {
            return Err(UsbError::Other(
                "a bulk IN stream needs a transfer size and a queue".into(),
            ));
        }
        let ep = self.endpoint::<Bulk, In>(endpoint)?;
        Ok(Box::new(NusbBulkIn::new(ep, transfer_size, queued)))
    }

    fn bulk_out(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        if !self.outs.contains_key(&endpoint) {
            let ep = self.endpoint::<Bulk, Out>(endpoint)?;
            self.outs.insert(endpoint, ep);
        }
        let Some(ep) = self.outs.get_mut(&endpoint) else {
            return Err(UsbError::NoDevice);
        };
        let mut buf = match self.spare.take() {
            Some(b) if b.capacity() >= data.len() => b,
            _ => ep.allocate(data.len()),
        };
        buf.clear();
        buf.extend_from_slice(data);
        let c = ep.transfer_blocking(buf, timeout);
        self.spare = Some(c.buffer);
        c.status.map_err(transfer_err)?;
        Ok(c.actual_len)
    }

    fn bulk_out_queue(
        &mut self,
        endpoint: u8,
        transfer_size: usize,
        queued: usize,
    ) -> Result<Box<dyn BulkOut>, UsbError> {
        if transfer_size == 0 || queued == 0 {
            return Err(UsbError::Other(
                "a bulk OUT writer needs a transfer size and a queue".into(),
            ));
        }
        let ep = match self.outs.remove(&endpoint) {
            Some(ep) => ep,
            None => self.endpoint::<Bulk, Out>(endpoint)?,
        };
        Ok(Box::new(NusbBulkOut::new(ep, transfer_size, queued)))
    }

    fn clear_halt(&mut self, endpoint: u8) -> Result<(), UsbError> {
        if let Some(ep) = self.outs.get_mut(&endpoint) {
            return ep.clear_halt().wait().map_err(dev_err);
        }
        if endpoint & 0x80 != 0 {
            self.endpoint::<Bulk, In>(endpoint)?
                .clear_halt()
                .wait()
                .map_err(dev_err)
        } else {
            self.endpoint::<Bulk, Out>(endpoint)?
                .clear_halt()
                .wait()
                .map_err(dev_err)
        }
    }

    fn reset(&mut self) -> Result<(), UsbError> {
        self.dev.reset().wait().map_err(dev_err)
    }
}

/// A bulk OUT endpoint with a pool of `queued` buffers, and one more for a transfer shorter than a buffer.
struct NusbBulkOut {
    ep: Endpoint<Bulk, Out>,
    size: usize,
    queued: usize,
    free: Vec<Buffer>,
    /// The buffer the caller fills.
    current: Option<Buffer>,
    /// The buffer that a short transfer is copied into.
    spare: Option<Buffer>,
}

impl NusbBulkOut {
    fn new(ep: Endpoint<Bulk, Out>, transfer_size: usize, queued: usize) -> NusbBulkOut {
        let packet = ep.max_packet_size().max(1);
        let size = transfer_size.div_ceil(packet) * packet;
        let mut free = Vec::with_capacity(queued);
        for _ in 0..queued {
            free.push(ep.allocate(size));
        }
        let spare = Some(ep.allocate(size));
        NusbBulkOut {
            ep,
            size,
            queued,
            free,
            current: None,
            spare,
        }
    }

    /// Puts a buffer back: into the pool, or as the spare when the pool is whole (the buffers swap roles after a
    /// short transfer).
    fn give_back(&mut self, b: Buffer) {
        if self.free.len() < self.queued {
            self.free.push(b);
        } else {
            self.spare = Some(b);
        }
    }
}

impl BulkOut for NusbBulkOut {
    fn endpoint(&self) -> u8 {
        self.ep.endpoint_address()
    }

    fn transfer_size(&self) -> usize {
        self.size
    }

    fn in_flight(&self) -> usize {
        self.ep.pending()
    }

    fn buffer(&mut self) -> Option<&mut [u8]> {
        if self.current.is_none() {
            self.current = self.free.pop();
        }
        let b = self.current.as_mut()?;
        // A buffer keeps its length after a full transfer, so it is filled with zeros only on first use and after
        // a short transfer.
        if b.len() < self.size {
            let missing = self.size - b.len();
            b.extend_fill(missing, 0);
        }
        Some(&mut b[..])
    }

    fn submit(&mut self, len: usize) -> Result<(), UsbError> {
        if len > self.size {
            return Err(UsbError::Other(format!(
                "a transfer of {len} bytes is longer than the buffer of {}",
                self.size
            )));
        }
        let b = self
            .current
            .take()
            .ok_or_else(|| UsbError::Other("no buffer was taken before the submit".into()))?;
        if len == self.size {
            self.ep.submit(b);
            return Ok(());
        }
        let Some(mut s) = self.spare.take() else {
            self.current = Some(b);
            return Err(UsbError::Other(
                "no spare buffer for a short transfer".into(),
            ));
        };
        s.clear();
        s.extend_from_slice(&b[..len]);
        self.ep.submit(s);
        self.spare = Some(b);
        Ok(())
    }

    fn completed(&mut self, timeout: Duration) -> Option<Result<usize, UsbError>> {
        if self.ep.pending() == 0 {
            return None;
        }
        let Some(c) = self.ep.wait_next_complete(timeout) else {
            return Some(Err(UsbError::Timeout));
        };
        let n = c.actual_len;
        self.give_back(c.buffer);
        Some(c.status.map(|()| n).map_err(transfer_err))
    }

    fn cancel(&mut self) {
        self.ep.cancel_all();
        while self.ep.pending() > 0 {
            match self.ep.wait_next_complete(Duration::from_secs(1)) {
                Some(c) => self.give_back(c.buffer),
                None => break,
            }
        }
    }
}

impl Drop for NusbBulkOut {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// A bulk IN endpoint with `queued` transfers in flight while the caller holds one buffer.
struct NusbBulkIn {
    ep: Endpoint<Bulk, In>,
    size: usize,
    /// The buffer the caller reads, given back on the next call.
    current: Option<Buffer>,
    /// Buffers to submit on the next call.
    idle: Vec<Buffer>,
}

impl NusbBulkIn {
    fn new(mut ep: Endpoint<Bulk, In>, transfer_size: usize, queued: usize) -> NusbBulkIn {
        let packet = ep.max_packet_size().max(1);
        let size = transfer_size.div_ceil(packet) * packet;
        for _ in 0..queued {
            let b = ep.allocate(size);
            ep.submit(b);
        }
        let mut idle = Vec::with_capacity(queued + 1);
        idle.push(ep.allocate(size));
        NusbBulkIn {
            ep,
            size,
            current: None,
            idle,
        }
    }

    /// Cancels the transfers in flight and keeps their buffers, waiting up to `wait` for each.
    fn drain(&mut self, wait: Duration) -> Result<(), UsbError> {
        self.ep.cancel_all();
        while self.ep.pending() > 0 {
            match self.ep.wait_next_complete(wait) {
                Some(c) => self.idle.push(c.buffer),
                None => return Err(UsbError::Timeout),
            }
        }
        Ok(())
    }
}

impl BulkIn for NusbBulkIn {
    fn endpoint(&self) -> u8 {
        self.ep.endpoint_address()
    }

    fn transfer_size(&self) -> usize {
        self.size
    }

    fn next(&mut self, timeout: Duration) -> Result<&[u8], UsbError> {
        if let Some(b) = self.current.take() {
            self.idle.push(b);
        }
        while let Some(mut b) = self.idle.pop() {
            b.clear();
            b.set_requested_len(self.size);
            self.ep.submit(b);
        }
        let c = self
            .ep
            .wait_next_complete(timeout)
            .ok_or(UsbError::Timeout)?;
        match c.status {
            Ok(()) => Ok(&self.current.insert(c.buffer)[..]),
            Err(e) => {
                self.idle.push(c.buffer);
                Err(transfer_err(e))
            }
        }
    }

    fn clear_halt(&mut self) -> Result<(), UsbError> {
        if let Some(b) = self.current.take() {
            self.idle.push(b);
        }
        self.drain(Duration::from_secs(1))?;
        self.ep.clear_halt().wait().map_err(dev_err)
    }
}

impl Drop for NusbBulkIn {
    fn drop(&mut self) {
        let _ = self.drain(Duration::from_millis(200));
    }
}
