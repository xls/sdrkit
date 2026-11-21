//! The test program of the ADS-B family: the message layer, the receivers against their generators, the four stacks, the
//! recorded vector, the recordings made off air, and single behaviours.

mod sdrkit_stack_adsb {
    mod behaviour;
    pub mod common;
    mod declarations;
    mod messages;
    mod modes;
    mod real;
    mod roundtrip;
    mod vector;
}
