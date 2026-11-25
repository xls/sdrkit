//! An E4000 dongle against an emulated register file of the tuner: the probe, the initialisation, the synthesiser,
//! the band, the filters, the gain, offset tuning and standby.

use sdrkit_driver_rtlusb::{Gain, Rtl2832, TunerKind};
use sdrkit_driver_usb::{BulkIn, Setup, UsbDevice, UsbDeviceInfo, UsbError, UsbId};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The tuner registers, the register pointer, and the demodulator writes.
#[derive(Default)]
struct State {
    regs: Vec<u8>,
    ptr: usize,
    demod: Vec<(u16, u16, Vec<u8>)>,
}

/// A dongle whose I2C bus has an E4000 at 0xC8 and nothing else.
struct Emulated {
    info: UsbDeviceInfo,
    s: Arc<Mutex<State>>,
}

impl UsbDevice for Emulated {
    fn info(&self) -> &UsbDeviceInfo {
        &self.info
    }
    fn claim(&mut self, _: u8) -> Result<(), UsbError> {
        Ok(())
    }
    fn control_in(&mut self, s: Setup, buf: &mut [u8], _: Duration) -> Result<usize, UsbError> {
        let st = self.s.lock().unwrap();
        match (s.index, s.value) {
            (0x0600, 0xC8) => {
                for (i, b) in buf.iter_mut().enumerate() {
                    *b = st.regs[st.ptr + i];
                }
                Ok(buf.len())
            }
            (0x0600, _) => Err(UsbError::Stall),
            _ => {
                buf.fill(0);
                Ok(buf.len())
            }
        }
    }
    fn control_out(&mut self, s: Setup, data: &[u8], _: Duration) -> Result<(), UsbError> {
        let mut st = self.s.lock().unwrap();
        match (s.index, s.value) {
            (0x0610, 0xC8) => {
                st.ptr = usize::from(data[0]);
                for (i, &b) in data[1..].iter().enumerate() {
                    let at = st.ptr + i;
                    // Register 0x07 bit 0 reads as PLL locked.
                    st.regs[at] = if at == 0x07 { b | 0x01 } else { b };
                }
                Ok(())
            }
            (0x0610, _) => Err(UsbError::Stall),
            (i, v) if i & 0xFF00 == 0 && i & 0x10 != 0 => {
                st.demod.push((i & 0x0F, v >> 8, data.to_vec()));
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn bulk_in(&mut self, _: u8, _: usize, _: usize) -> Result<Box<dyn BulkIn>, UsbError> {
        Err(UsbError::Stall)
    }
    fn bulk_out(&mut self, _: u8, _: &[u8], _: Duration) -> Result<usize, UsbError> {
        Err(UsbError::Stall)
    }
    fn clear_halt(&mut self, _: u8) -> Result<(), UsbError> {
        Ok(())
    }
    fn reset(&mut self) -> Result<(), UsbError> {
        Ok(())
    }
}

fn open() -> (Rtl2832, Arc<Mutex<State>>) {
    let mut regs = vec![0u8; 256];
    regs[0x02] = 0x40;
    regs[0x07] = 0x01;
    let s = Arc::new(Mutex::new(State {
        regs,
        ..State::default()
    }));
    let mut info = UsbDeviceInfo::new(UsbId(1), 0x0bda, 0x2838);
    info.manufacturer = Some("Realtek".into());
    info.product = Some("RTL2838UHIDIR".into());
    let d = Rtl2832::open(Box::new(Emulated { info, s: s.clone() })).unwrap();
    (d, s)
}

/// The last value written to demodulator page `page` register `reg`.
fn demod(s: &Mutex<State>, page: u16, reg: u16) -> Option<u8> {
    s.lock()
        .unwrap()
        .demod
        .iter()
        .rev()
        .find(|w| w.0 == page && w.1 == reg)
        .map(|w| w.2[0])
}

#[test]
fn an_e4000_is_initialised_tuned_and_set() {
    let (mut d, s) = open();
    assert_eq!(d.tuner(), TunerKind::E4000);
    {
        let st = s.lock().unwrap();
        let r = &st.regs;
        assert_eq!(r[0x00], 0x07);
        assert_eq!(r[0x7A], 0x96);
        assert_eq!((r[0x1D], r[0x1E], r[0x1F]), (0x10, 0x04, 0x1A));
        // Automatic gain: AGC mode 9, mixer automatic.
        assert_eq!((r[0x1A] & 0x0F, r[0x20] & 0x01), (9, 1));
        // IF gains 6, 0, 0, 0, 9, 9 dB; the narrowest filters, the channel filter on.
        assert_eq!((r[0x16], r[0x17]), (0x01, 0x12));
        assert_eq!((r[0x11], r[0x12]), (0xFF, 0x1F));
    }
    // The zero-IF row of the demodulator.
    assert_eq!(demod(&s, 1, 0xB1), Some(0x1B));
    assert_eq!(demod(&s, 0, 0x08), Some(0xCD));
    assert_eq!(demod(&s, 1, 0x15), Some(0x00));

    d.set_sample_rate(2_400_000).unwrap();
    d.set_frequency(100_000_000).unwrap();
    assert!(d.locked());
    {
        let st = s.lock().unwrap();
        let r = &st.regs;
        // R = 32 with three-phase mixing, Z = 111, X = 7281, VHF II band.
        assert_eq!((r[0x0D], r[0x09]), (0x0D, 111));
        assert_eq!(u16::from_le_bytes([r[0x0A], r[0x0B]]), 7281);
        assert_eq!((r[0x07] & 0x06, r[0x78], r[0x10] & 0x0F), (0x00, 0x03, 0));
        // Filters nearest 2.4 MHz: mixer 2.3 MHz, RC 2.6 MHz, channel 2.4 MHz.
        assert_eq!((r[0x11] >> 4, r[0x11] & 0x0F, r[0x12] & 0x1F), (14, 12, 26));
    }
    // UHF band, RF filter nearest 433 MHz (425 MHz, index 3).
    d.set_frequency(433_920_000).unwrap();
    {
        let r = &s.lock().unwrap().regs;
        assert_eq!((r[0x07] & 0x06, r[0x10] & 0x0F), (0x04, 3));
    }
    // A manual gain of 29 dB: mixer 4 dB, LNA 25 dB (code 13).
    assert_eq!(d.set_gain(Gain::Manual(290)).unwrap(), Some(290));
    {
        let r = &s.lock().unwrap().regs;
        assert_eq!((r[0x1A] & 0x0F, r[0x20] & 0x01), (0, 0));
        assert_eq!((r[0x14] & 0x0F, r[0x15] & 0x01), (13, 0));
    }
    // 42 dB: mixer 12 dB, LNA 30 dB.
    assert_eq!(d.set_gain(Gain::Manual(430)).unwrap(), Some(420));
    assert_eq!(s.lock().unwrap().regs[0x15] & 0x01, 1);

    // Offset tuning at 2.4 MS/s: the tuner 2.04 MHz below, the demodulator shifts it back.
    d.set_frequency(100_000_000).unwrap();
    d.set_offset_tuning(true).unwrap();
    assert_eq!(d.if_hz(), 2_040_000);
    assert_eq!(
        (demod(&s, 1, 0x19), demod(&s, 1, 0x1A), demod(&s, 1, 0x1B)),
        (Some(0x3B), Some(0x77), Some(0x78))
    );
    {
        let r = &s.lock().unwrap().regs;
        // 97.96 MHz: R = 32, Z = floor(3134.72 / 28.8) = 108.
        assert_eq!(r[0x09], 108);
        // Filters nearest 4.08 MHz: mixer 4.2, RC 4.4, channel 4.1 MHz.
        assert_eq!((r[0x11] >> 4, r[0x11] & 0x0F, r[0x12] & 0x1F), (9, 10, 7));
    }
    d.close();
    assert_eq!(s.lock().unwrap().regs[0x00] & 0x02, 0, "standby");
}
