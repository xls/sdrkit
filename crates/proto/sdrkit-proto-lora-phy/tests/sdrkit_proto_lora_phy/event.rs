//! The frame event and the shared interface of the crate.

use sdrkit_proto_lora_phy::components::{interface, FRAME_EVENT};

/// The shared interface lists the frame event with the keys of its declaration, in order.
#[test]
fn the_shared_interface_lists_the_frame_event() {
    let shared = interface();
    assert!(shared.shared);
    let events = &shared.stacks[0].events;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].def.kind, "lora.frame");
    let keys: Vec<&str> = FRAME_EVENT.fields.iter().map(|f| f.key()).collect();
    assert_eq!(
        keys,
        [
            "payload",
            "sf",
            "bw_hz",
            "cr",
            "crc_present",
            "crc_ok",
            "implicit",
            "sync",
            "inverted",
            "offset_hz",
            "clock_ppm",
            "snr_db",
            "level_dbfs",
            "corrected",
            "tag",
            "channel_hz"
        ]
    );
}
