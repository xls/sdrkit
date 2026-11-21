//! The tests of sdrkit-proto-bcdata, as one test program. Each module tests one module of the crate. `alloc` counts allocations in
//! steady state, per thread, so the other tests can run at the same time.

#[path = "bcdata/aac.rs"]
mod aac;
#[path = "bcdata/alloc.rs"]
mod alloc;
#[path = "bcdata/component.rs"]
mod component;
#[path = "bcdata/datagroup.rs"]
mod datagroup;
#[path = "bcdata/journaline.rs"]
mod journaline;
#[path = "bcdata/label.rs"]
mod label;
#[path = "bcdata/mot.rs"]
mod mot;
#[path = "bcdata/packet.rs"]
mod packet;
#[path = "bcdata/playout.rs"]
mod playout;
#[path = "bcdata/publish.rs"]
mod publish;
#[path = "bcdata/slideshow.rs"]
mod slideshow;
#[path = "bcdata/spi.rs"]
mod spi;
#[path = "bcdata/tags.rs"]
mod tags;

/// CRC-16/GENIBUS computed bit by bit from its definition (x^16 + x^12 + x^5 + 1, register all ones, result inverted), as a
/// second implementation for the CRCs of EN 300 401.
pub fn genibus(data: &[u8]) -> u16 {
    let mut r: u16 = 0xFFFF;
    for &b in data {
        for i in (0..8).rev() {
            let bit = ((b >> i) & 1) as u16 ^ (r >> 15);
            r <<= 1;
            if bit != 0 {
                r ^= 0x1021;
            }
        }
    }
    !r
}
