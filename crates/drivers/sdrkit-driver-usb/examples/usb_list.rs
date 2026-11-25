//! Lists the USB devices of this system without opening any of them.
//!
//! `cargo run -p sdrkit-driver-usb --features nusb --example usb_list`. Without the feature, or on a platform
//! without USB access, it prints why no device can be listed.

use sdrkit_driver_usb::{platform_bus, Availability};

fn main() {
    let bus = platform_bus();
    let availability = bus.availability();
    if let Availability::Unavailable(_) = availability {
        println!("{}", availability.text());
        return;
    }
    let devices = match bus.devices() {
        Ok(d) => d,
        Err(e) => {
            println!("the devices cannot be listed: {e}");
            return;
        }
    };
    if devices.is_empty() {
        println!("no USB devices");
    }
    for d in devices {
        let text = |s: &Option<String>| s.as_deref().unwrap_or("-").to_owned();
        print!(
            "{:04x}:{:04x}  release {:04x}  speed {}  path {}  manufacturer {}  product {}  serial {}",
            d.vendor_id,
            d.product_id,
            d.release,
            d.speed.key(),
            d.path(),
            text(&d.manufacturer),
            text(&d.product),
            text(&d.serial)
        );
        match d.blocked {
            Some(why) => println!("  ({})", why.text()),
            None => println!(),
        }
    }
}
