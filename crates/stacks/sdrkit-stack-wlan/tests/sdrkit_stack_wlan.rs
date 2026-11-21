//! The test program of this crate: one module for each stack, and the helpers they share.

mod sdrkit_stack_wlan {
    pub mod alloc;
    pub mod ble;
    pub mod btclassic;
    pub mod espnow;
    pub mod halow;
    pub mod help;
    pub mod tracker;
    pub mod wfb;
    pub mod wifi;
}
