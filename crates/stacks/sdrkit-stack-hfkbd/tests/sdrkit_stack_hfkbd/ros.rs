//! ROS: the grid at two rates is named, other signals are not, and the recording.

use crate::common::*;
use sdrkit_core::LockState;
use sdrkit_stack_hfkbd as hf;

#[test]
fn ros_identifies_the_grid_and_not_the_rest() {
    for (baud, want) in [(16.0, "ros16"), (8.0, "ros8")] {
        let r = run(&snr_2500(harness(&hf::ROS), 15.0)
            .set(&hf::ros::GEN_BAUD, baud)
            .seconds(20.0));
        assert_eq!(
            text_field(&r, "sub_mode"),
            want,
            "{:?}",
            text_field(&r, "why")
        );
    }
    for sig in ["noise", "fsk", "carrier", "sweep"] {
        let r = run(&snr_2500(harness(&hf::ROS), 15.0)
            .set(
                &hf::ros::GEN_SIGNAL,
                hf::ros::GenSignal::from_key(sig).unwrap(),
            )
            .seconds(20.0));
        assert_ne!(
            r.results.status().lock_state,
            LockState::Locked,
            "{sig}: {:?}",
            text_field(&r, "why")
        );
    }
}

#[test]
fn ros16_recording() {
    let name = "hfkbd_ros16_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        let r = recording(&harness(&hf::ROS).piece(piece), name);
        assert_eq!(r.results.status().lock_state, LockState::Locked);
        assert_eq!(text_field(&r, "sub_mode"), "ros16");
        assert!(in_range(
            num(&r, "spacing_hz"),
            &e["expect"]["grid_spacing_hz"]
        ));
        let (lo, hi) = (num(&r, "lowest_hz"), num(&r, "highest_hz"));
        assert!(in_range(lo, &e["expect"]["tones_lo_hz"]));
        assert!(in_range(hi, &e["expect"]["tones_hi_hz"]));
        assert!(in_range(hi - lo, &e["expect"]["span_hz"]));
        // ROS text is not public, so nothing is read.
        assert!(r.field("text").is_none());
    }
}
