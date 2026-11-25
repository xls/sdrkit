//! Register access of the RTL2832U through vendor control transfers: the demodulator, USB and system register blocks,
//! the I2C bridge, the I2C repeater to the tuner and the GPIO pins (specification sections 2.2 to 2.5 and 5.1).

use crate::RtlError;
use sdrkit_driver_usb::{Setup, UsbDevice, UsbDeviceInfo, UsbError};
use std::time::Duration;

/// The timeout of one control transfer.
pub const CONTROL_TIMEOUT: Duration = Duration::from_millis(300);

/// `wIndex` of a write to the USB block.
const USB_WRITE: u16 = 0x0110;
/// `wIndex` of a read and of a write of the system block.
const SYS_READ: u16 = 0x0200;
const SYS_WRITE: u16 = 0x0210;
/// `wIndex` of an I2C read and of an I2C write through the bridge.
const I2C_READ: u16 = 0x0600;
const I2C_WRITE: u16 = 0x0610;

/// USB block registers.
pub const USB_SYSCTL: u16 = 0x2000;
/// Max packet size of endpoint A.
pub const USB_EPA_MAXPKT: u16 = 0x2158;
/// Control of endpoint A: FIFO reset and stall.
pub const USB_EPA_CTL: u16 = 0x2148;

/// System block registers.
pub const DEMOD_CTL: u16 = 0x3000;
/// GPIO output values.
pub const GPO: u16 = 0x3001;
/// GPIO output enables.
pub const GPOE: u16 = 0x3003;
/// GPIO directions (1 = input).
pub const GPD: u16 = 0x3004;
/// Demodulator power control.
pub const DEMOD_CTL_1: u16 = 0x300B;

/// The chip of an open dongle. Every call is one or two control transfers.
pub struct Chip {
    dev: Box<dyn UsbDevice>,
}

impl Chip {
    /// The chip behind the open device `dev`.
    pub fn new(dev: Box<dyn UsbDevice>) -> Chip {
        Chip { dev }
    }

    /// The device as the bus listed it.
    pub fn info(&self) -> &UsbDeviceInfo {
        self.dev.info()
    }

    /// The open device.
    pub fn device(&mut self) -> &mut dyn UsbDevice {
        self.dev.as_mut()
    }

    fn out(&mut self, value: u16, index: u16, data: &[u8]) -> Result<(), RtlError> {
        self.dev
            .control_out(Setup::vendor(0, value, index), data, CONTROL_TIMEOUT)
            .map_err(RtlError::from)
    }

    fn inp(&mut self, value: u16, index: u16, buf: &mut [u8]) -> Result<usize, RtlError> {
        self.dev
            .control_in(Setup::vendor(0, value, index).input(), buf, CONTROL_TIMEOUT)
            .map_err(RtlError::from)
    }

    /// Writes `data` to consecutive USB block registers from `addr`.
    ///
    /// # Errors
    ///
    /// The error of the transfer.
    pub fn usb_write(&mut self, addr: u16, data: &[u8]) -> Result<(), RtlError> {
        self.out(addr, USB_WRITE, data)
    }

    /// Writes one system block register.
    ///
    /// # Errors
    ///
    /// The error of the transfer.
    pub fn sys_write(&mut self, addr: u16, value: u8) -> Result<(), RtlError> {
        self.out(addr, SYS_WRITE, &[value])
    }

    /// Reads one system block register.
    ///
    /// # Errors
    ///
    /// The error of the transfer, or [`RtlError::ShortRead`].
    pub fn sys_read(&mut self, addr: u16) -> Result<u8, RtlError> {
        let mut b = [0u8; 1];
        match self.inp(addr, SYS_READ, &mut b)? {
            1 => Ok(b[0]),
            got => Err(RtlError::ShortRead {
                addr: 0,
                want: 1,
                got,
            }),
        }
    }

    /// Writes `data` to demodulator page `page` from register `reg` (the first byte at `reg`), then makes the dummy
    /// read of page 0x0A register 0x01 that every demodulator write is followed by. The result of that read is not
    /// used; only an unplug is reported from it.
    ///
    /// # Errors
    ///
    /// The error of the write, or [`UsbError::Unplugged`] from the dummy read.
    pub fn demod_write(&mut self, page: u8, reg: u8, data: &[u8]) -> Result<(), RtlError> {
        self.out((u16::from(reg) << 8) | 0x20, 0x0010 | u16::from(page), data)?;
        let mut b = [0u8; 1];
        match self.inp(0x0120, 0x000A, &mut b) {
            Err(RtlError::Usb(UsbError::Unplugged)) => Err(UsbError::Unplugged.into()),
            _ => Ok(()),
        }
    }

    /// Writes one demodulator register.
    ///
    /// # Errors
    ///
    /// As [`Chip::demod_write`].
    pub fn demod_write1(&mut self, page: u8, reg: u8, value: u8) -> Result<(), RtlError> {
        self.demod_write(page, reg, &[value])
    }

    /// Writes `data` as one I2C write to the device at 8-bit address `addr`.
    ///
    /// # Errors
    ///
    /// The error of the transfer (an absent device fails).
    pub fn i2c_write(&mut self, addr: u8, data: &[u8]) -> Result<(), RtlError> {
        self.out(u16::from(addr), I2C_WRITE, data)
    }

    /// Fills `buf` with one I2C read from the device at 8-bit address `addr`.
    ///
    /// # Errors
    ///
    /// The error of the transfer, or [`RtlError::ShortRead`].
    pub fn i2c_read(&mut self, addr: u8, buf: &mut [u8]) -> Result<(), RtlError> {
        let got = self.inp(u16::from(addr), I2C_READ, buf)?;
        if got == buf.len() {
            Ok(())
        } else {
            Err(RtlError::ShortRead {
                addr,
                want: buf.len(),
                got,
            })
        }
    }

    /// Reads register `reg` of the I2C device at `addr`: a write of the register number, then a read of one byte.
    ///
    /// # Errors
    ///
    /// As [`Chip::i2c_read`].
    pub fn i2c_read_reg(&mut self, addr: u8, reg: u8) -> Result<u8, RtlError> {
        self.i2c_write(addr, &[reg])?;
        let mut b = [0u8; 1];
        self.i2c_read(addr, &mut b)?;
        Ok(b[0])
    }

    /// Switches the I2C repeater to the tuner on or off (page 1 register 0x01).
    ///
    /// # Errors
    ///
    /// As [`Chip::demod_write`].
    pub fn repeater(&mut self, on: bool) -> Result<(), RtlError> {
        self.demod_write1(1, 0x01, if on { 0x18 } else { 0x10 })
    }

    /// Makes GPIO `n` a driven output.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn gpio_output(&mut self, n: u8) -> Result<(), RtlError> {
        let d = self.sys_read(GPD)?;
        self.sys_write(GPD, d & !(1 << n))?;
        let e = self.sys_read(GPOE)?;
        self.sys_write(GPOE, e | (1 << n))
    }

    /// Drives GPIO `n` to `on`.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn gpio_set(&mut self, n: u8, on: bool) -> Result<(), RtlError> {
        let v = self.sys_read(GPO)?;
        let v = if on { v | (1 << n) } else { v & !(1 << n) };
        self.sys_write(GPO, v)
    }

    /// Runs `f` with the repeater on and switches it off after `f`, also when `f` fails (but not after an unplug).
    ///
    /// # Errors
    ///
    /// The error of `f`, else the error of a repeater write.
    pub fn with_repeater<T>(
        &mut self,
        f: impl FnOnce(&mut Chip) -> Result<T, RtlError>,
    ) -> Result<T, RtlError> {
        self.repeater(true)?;
        let r = f(self);
        match &r {
            Err(e) if e.unplugged() => r,
            Err(_) => {
                let _ = self.repeater(false);
                r
            }
            Ok(_) => self.repeater(false).and(r),
        }
    }
}
