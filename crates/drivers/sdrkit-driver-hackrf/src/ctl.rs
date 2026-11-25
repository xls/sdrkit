//! One open HackRF: what it is, read once at open, and its requests, each checked against the version rules before it
//! is sent.

use crate::error::HackrfError;
use crate::proto::{self, request, Board, Mode, PartSerial, StreamState};
use sdrkit_driver_usb::{BulkIn, Setup, UsbDevice, UsbDeviceInfo, UsbError};
use std::time::Duration;

/// What a device is, read when it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    /// The device as the bus listed it.
    pub info: UsbDeviceInfo,
    /// The board id.
    pub board_id: u8,
    /// The board.
    pub board: Board,
    /// The board revision (USB API 0x0106 or later).
    pub revision: Option<u8>,
    /// The supported-platforms mask of the firmware (USB API 0x0106 or later).
    pub platforms: Option<u32>,
    /// The USB API version (bcdDevice).
    pub api: u16,
    /// The firmware version string.
    pub version: String,
    /// The part id and serial number.
    pub part_serial: Option<PartSerial>,
}

impl Facts {
    /// The serial number: from the request, else from the string of the bus.
    pub fn serial(&self) -> String {
        self.part_serial
            .map(|p| p.serial_text())
            .or_else(|| self.info.serial.clone())
            .unwrap_or_default()
    }

    /// A name for the device: the board and its revision.
    pub fn name(&self) -> String {
        match self.revision {
            Some(r) if r < 0xFE => format!("{} ({})", self.board.name(), proto::revision_text(r)),
            _ => self.board.name().to_string(),
        }
    }
}

/// The setup of the standard request for the device descriptor.
fn descriptor_setup() -> Setup {
    Setup {
        request_type: 0x00,
        request: proto::GET_DESCRIPTOR,
        value: 0x0100,
        index: 0,
    }
    .input()
}

/// A vendor IN request to `usb` that is checked only against the table of forbidden requests (the version is not known
/// yet when it is used).
fn early_in(
    usb: &mut dyn UsbDevice,
    req: u8,
    buf: &mut [u8],
    timeout: Duration,
) -> Result<usize, HackrfError> {
    proto::min_api(req, 0)?;
    Ok(usb.control_in(Setup::vendor(req, 0, 0).input(), buf, timeout)?)
}

/// Reads what the device `usb` is: the device descriptor, the part id and serial number, the board revision and the
/// supported platforms (USB API 0x0106 or later), the board id and the version string, in this order.
///
/// # Errors
///
/// A USB error, a short descriptor or board id, or [`HackrfError::Prototype`].
pub fn identify(
    usb: &mut dyn UsbDevice,
    info: &UsbDeviceInfo,
    timeout: Duration,
) -> Result<Facts, HackrfError> {
    let mut d = [0u8; 18];
    let n = usb.control_in(descriptor_setup(), &mut d, timeout)?;
    let api = proto::api_from_descriptor(&d[..n]).ok_or(HackrfError::Short {
        request: proto::GET_DESCRIPTOR,
        want: 14,
        got: n,
    })?;
    let mut ps = [0u8; 24];
    let part_serial = match early_in(usb, request::PART_SERIAL, &mut ps, timeout) {
        Ok(n) => PartSerial::parse(&ps[..n]),
        Err(e) if e.unplugged() => return Err(e),
        Err(_) => None,
    };
    let (mut revision, mut platforms) = (None, None);
    if api >= 0x0106 {
        let mut b = [0u8; 1];
        match early_in(usb, request::BOARD_REVISION, &mut b, timeout) {
            Ok(1) => revision = Some(b[0]),
            Err(e) if e.unplugged() => return Err(e),
            _ => {}
        }
        let mut b = [0u8; 4];
        match early_in(usb, request::PLATFORMS, &mut b, timeout) {
            Ok(4) => platforms = Some(u32::from_be_bytes(b)),
            Err(e) if e.unplugged() => return Err(e),
            _ => {}
        }
    }
    let mut b = [0u8; 1];
    let n = early_in(usb, request::BOARD_ID, &mut b, timeout)?;
    if n < 1 {
        return Err(HackrfError::Short {
            request: request::BOARD_ID,
            want: 1,
            got: 0,
        });
    }
    let board = Board::from_id(b[0])?;
    let mut v = [0u8; 255];
    let n = early_in(usb, request::VERSION, &mut v, timeout)?;
    let version = String::from_utf8_lossy(&v[..n])
        .trim_end_matches('\0')
        .trim()
        .to_string();
    Ok(Facts {
        info: info.clone(),
        board_id: b[0],
        board,
        revision,
        platforms,
        api,
        version,
        part_serial,
    })
}

/// The control side of an open device. Every request passes [`proto::check`] first; a request that fails it is not
/// sent.
pub struct Ctl {
    usb: Option<Box<dyn UsbDevice>>,
    api: u16,
    board: Board,
    control: Duration,
    mode_off: Duration,
    mode: Option<Mode>,
    clock_out: bool,
}

impl Ctl {
    /// The control side of `usb`, a device with USB API `api` and board `board`. `control` is the timeout of a request,
    /// `mode_off` that of the request that ends a transmission.
    pub fn new(
        usb: Box<dyn UsbDevice>,
        api: u16,
        board: Board,
        control: Duration,
        mode_off: Duration,
    ) -> Ctl {
        Ctl {
            usb: Some(usb),
            api,
            board,
            control,
            mode_off,
            mode: None,
            clock_out: false,
        }
    }

    /// The USB API version.
    pub fn api(&self) -> u16 {
        self.api
    }

    /// The last mode sent.
    pub fn mode_now(&self) -> Option<Mode> {
        self.mode
    }

    /// The device, or [`UsbError::NoDevice`] once it was released.
    fn dev(&mut self) -> Result<&mut Box<dyn UsbDevice>, HackrfError> {
        self.usb
            .as_mut()
            .ok_or(HackrfError::Usb(UsbError::NoDevice))
    }

    /// Gives the device back to the system: its interface is released, and a later open of it can claim it. Every
    /// request afterwards fails.
    pub fn release(&mut self) {
        self.usb = None;
    }

    fn out(&mut self, req: u8, value: u16, index: u16, data: &[u8]) -> Result<(), HackrfError> {
        proto::check(req, value, self.api, self.board)?;
        let t = if req == request::MODE && value == Mode::Off as u16 {
            self.mode_off
        } else {
            self.control
        };
        self.dev()?
            .control_out(Setup::vendor(req, value, index), data, t)?;
        Ok(())
    }

    fn input(
        &mut self,
        req: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
    ) -> Result<usize, HackrfError> {
        proto::check(req, value, self.api, self.board)?;
        let t = self.control;
        Ok(self
            .dev()?
            .control_in(Setup::vendor(req, value, index).input(), buf, t)?)
    }

    /// Sends transceiver mode `m`. The request for [`Mode::Off`] has the long timeout, since it can wait for the device
    /// to drain a transmission.
    pub fn set_mode(&mut self, m: Mode) -> Result<(), HackrfError> {
        self.out(request::MODE, m as u16, 0, &[])?;
        self.mode = Some(m);
        Ok(())
    }

    /// Sends the sample rate `rate` as a frequency and a divider.
    pub fn sample_rate(&mut self, rate: f64) -> Result<(), HackrfError> {
        let (f, d) = proto::rate_ratio(rate);
        self.out(request::SAMPLE_RATE, 0, 0, &proto::rate_data(f, d))
    }

    /// Sends the baseband filter `hz`, one of [`proto::FILTERS_HZ`].
    pub fn filter(&mut self, hz: u32) -> Result<(), HackrfError> {
        let (value, index) = proto::split_u32(hz);
        self.out(request::FILTER, value, index, &[])
    }

    /// Sends the frequency `hz` in the simple form.
    ///
    /// # Errors
    ///
    /// [`HackrfError::Frequency`] above 7250 MHz, before anything is sent.
    pub fn frequency(&mut self, hz: u64) -> Result<(), HackrfError> {
        if hz > proto::MAX_FREQ_HZ {
            return Err(HackrfError::Frequency(hz));
        }
        self.out(request::FREQUENCY, 0, 0, &proto::frequency_data(hz))
    }

    /// Turns the RF amplifier on or off, for receive and transmit.
    pub fn amp(&mut self, on: bool) -> Result<(), HackrfError> {
        self.out(request::AMP, u16::from(on), 0, &[])
    }

    fn gain(&mut self, req: u8, db: u32) -> Result<(), HackrfError> {
        let mut b = [0u8; 1];
        let n = self.input(req, 0, db as u16, &mut b)?;
        if n < 1 || b[0] == 0 {
            return Err(HackrfError::Rejected { request: req });
        }
        Ok(())
    }

    /// Sends the LNA gain `db`, already on its step.
    pub fn lna(&mut self, db: u32) -> Result<(), HackrfError> {
        self.gain(request::LNA, db)
    }

    /// Sends the VGA gain `db`, already on its step.
    pub fn vga(&mut self, db: u32) -> Result<(), HackrfError> {
        self.gain(request::VGA, db)
    }

    /// Sends the TX VGA gain `db`.
    pub fn tx_vga(&mut self, db: u32) -> Result<(), HackrfError> {
        self.gain(request::TX_VGA, db)
    }

    /// Turns antenna port power on or off.
    pub fn antenna_power(&mut self, on: bool) -> Result<(), HackrfError> {
        self.out(request::ANTENNA_POWER, u16::from(on), 0, &[])
    }

    /// Turns the 10 MHz clock output on or off. Off is sent only after an on.
    pub fn clock_out(&mut self, on: bool) -> Result<(), HackrfError> {
        if on || self.clock_out {
            self.out(request::CLOCK_OUT, u16::from(on), 0, &[])?;
            self.clock_out = on;
        }
        Ok(())
    }

    /// True if a clock is detected on the clock input.
    pub fn clock_in(&mut self) -> Result<bool, HackrfError> {
        let mut b = [0u8; 1];
        let n = self.input(request::CLOCK_IN, 0, 0, &mut b)?;
        Ok(n >= 1 && b[0] != 0)
    }

    /// The device buffer size in bytes. Reading it turns the drain of the firmware off until the device restarts.
    pub fn buffer_size(&mut self) -> Result<u32, HackrfError> {
        let mut b = [0u8; 4];
        let n = self.input(request::BUFFER_SIZE, 0, 0, &mut b)?;
        if n < 4 {
            return Err(HackrfError::Short {
                request: request::BUFFER_SIZE,
                want: 4,
                got: n,
            });
        }
        Ok(u32::from_le_bytes(b))
    }

    /// The streaming state.
    pub fn stream_state(&mut self) -> Result<StreamState, HackrfError> {
        let mut b = [0u8; 40];
        let n = self.input(request::STREAM_STATE, 0, 0, &mut b)?;
        StreamState::parse(&b[..n]).ok_or(HackrfError::Short {
            request: request::STREAM_STATE,
            want: 40,
            got: n,
        })
    }

    /// Queues `queued` bulk reads of `size` bytes on the receive endpoint.
    pub fn reads(&mut self, size: usize, queued: usize) -> Result<Box<dyn BulkIn>, HackrfError> {
        Ok(self.dev()?.bulk_in(proto::EP_RX, size, queued)?)
    }

    /// Writes `data` to the transmit endpoint.
    pub fn write(&mut self, data: &[u8], timeout: Duration) -> Result<usize, HackrfError> {
        Ok(self.dev()?.bulk_out(proto::EP_TX, data, timeout)?)
    }
}
