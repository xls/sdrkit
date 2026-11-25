//! USB driver for RTL2832U dongles with an R820T, R820T2, R860, R828D or E4000 tuner. It talks to the dongle through
//! the USB transport of `sdrkit-driver-usb` and streams unsigned 8-bit I/Q.

#![warn(missing_docs)]

pub mod chip;
pub mod device;
pub mod diag;
mod driver;
pub mod e4000;
mod error;
pub mod ids;
pub mod r82xx;
pub mod radio;

pub use device::{Direct, Gain, Rtl2832, TunerKind};
pub use driver::{
    address_of, dongles, open_selected, DirectSampling, RtlKindCommand, RtlSetting, RtlUsbDriver,
    Selector, DEVICES, DIRECT_SAMPLING, DISCOVER, D_ADDRESS, D_NAME, D_PATH, D_SERIAL, D_STATE,
    D_TUNER, KEY, OFFSET_TUNING, PPM, RTL_AGC, TUNER_AGC,
};
pub use error::RtlError;

/// The entry of `table` nearest `target`; on a tie the earlier one.
pub fn nearest_gain(table: &[i32], target: i32) -> i32 {
    let mut best = table[0];
    for &g in table {
        if (g - target).abs() < (best - target).abs() {
            best = g;
        }
    }
    best
}
