//! The worked example of the specification (section 7), byte for byte: an R820T2 dongle opened, set to 2.4 MS/s,
//! tuned to 100.0 MHz, switched to manual gain and set to 29.7 dB, then the endpoint reset.

use sdrkit_driver_rtlusb::{Gain, Rtl2832, TunerKind};
use sdrkit_driver_usb::{FakeUsb, Script, UsbBus, UsbId};
use std::path::Path;

fn script() -> Script {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/usb/rtl/r820t2_100mhz.usbscript");
    Script::load(&path).unwrap()
}

#[test]
fn the_worked_example_is_sent_byte_for_byte() {
    let bus = FakeUsb::new(script());
    let dev = bus.open(UsbId(1)).unwrap();
    let mut d = Rtl2832::open(dev).unwrap();
    assert_eq!(d.tuner(), TunerKind::R820T);
    assert_eq!(d.set_sample_rate(2_400_000).unwrap(), 2_400_000.0);
    assert_eq!(d.if_hz(), 1_815_000);
    d.set_frequency(100_000_000).unwrap();
    assert!(d.locked());
    assert_eq!(d.set_gain(Gain::Manual(0)).unwrap(), Some(0));
    assert_eq!(d.set_gain(Gain::Manual(297)).unwrap(), Some(297));
    d.reset_buffer().unwrap();
    if let Err(m) = bus.check() {
        panic!("{m}");
    }
    assert_eq!(
        d.r82xx_image().unwrap(),
        [
            0x18, 0x12, 0x68, 0xc0, 0x40, 0xc8, 0x8f, 0x68, 0x53, 0x75, 0x68, 0x84, 0xbb, 0x80,
            0x31, 0xca, 0x5b, 0x90, 0x30, 0x48, 0xec, 0x2a, 0x34, 0x24, 0xdd, 0x6e, 0x40
        ]
    );
}
