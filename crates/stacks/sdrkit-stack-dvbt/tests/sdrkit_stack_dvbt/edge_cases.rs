//! Edge cases that the other modules do not cover: a scan of a channel list, the shared pilot sequence and the shared outer
//! de-interleaver.

use sdrkit_stack_dvbt::coding;
use sdrkit_stack_dvbt::outer::OuterDecoder;
use sdrkit_stack_dvbt::params::Mode;
use sdrkit_stack_dvbt::scan::ScanCommand;
use sdrkit_stack_dvbt::Bandwidth;
use sdrkit_util_bits::interleave::DelayLanes;

/// A scan of the channels of a country moves on from step 0.
#[test]
fn a_channel_list_scans_past_step_0() {
    let mut s = crate::common::channel_scan(&crate::common::country_channels("AU"), 600e6);
    assert_eq!(s.progress(), (0, 32));
    let ScanCommand::Tune { hz, bandwidth } = s.start(0.0) else {
        panic!("the scan tunes the first channel")
    };
    assert_eq!((hz, bandwidth), (177.5e6, Some(Bandwidth::B7)));
    let mut p = sdrkit_core::publish::Published::from_decl(std::sync::Arc::new(
        sdrkit_stack_dvbt::typed::shape("dvbt").results_decl(),
    ));
    p.put(&crate::common::report_fields(0, Some(0.0)));
    s.on_report(0.3, p.results());
    assert_eq!(s.progress(), (1, 32));
}

/// The pilot sequence w_k is `sdrkit_util_bits::seq::DVBT_PILOTS`, the sequence EN 300 744 clause 4.5.2 writes as
/// x^11 + x^2 + 1 started all ones: w_0 to w_10 are 1, w_11 to w_19 are 0, w_20 is 1. The continual pilots of 2K and 8K come
/// from the shared 2K table (EN 300 744 table 7: 45 and 177 carriers).
#[test]
fn the_pilot_sequence_is_the_shared_one() {
    let w = coding::pilot_sequence(32);
    assert_eq!(&w[..11], &[1; 11]);
    assert_eq!(&w[11..20], &[0; 9]);
    assert_eq!(w[20], 1);
    let mut shared = sdrkit_util_bits::seq::DVBT_PILOTS;
    let mut v = [0u8; 32];
    shared.fill_bits(&mut v);
    assert_eq!(&w[..], &v[..]);
    assert_eq!(coding::continual(Mode::K2).len(), 45);
    assert_eq!(coding::continual(Mode::K8).len(), 177);
    assert_eq!(coding::tps_carriers(Mode::K8).len(), 68);
}

/// The outer de-interleaver is the shared convolutional interleaver, which keeps its 12 branches in one bank. Making it takes
/// fewer allocations than there are branches.
#[test]
fn the_outer_deinterleaver_is_one_memory() {
    let before = crate::allocations();
    let mut d: DelayLanes<u8> = sdrkit_proto_mpegts::outer::interleaver(true);
    let made = crate::allocations() - before;
    assert!(made < 12, "{made} allocations");
    assert_eq!(d.lanes(), 12);
    let delay = 11 * 17 * 12;
    let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8 + 1).collect();
    let mut line = data.clone();
    sdrkit_proto_mpegts::outer::interleaver(false).process(&mut line);
    d.process(&mut line);
    assert_eq!(&line[delay..], &data[..data.len() - delay]);
    drop(OuterDecoder::new());
}
