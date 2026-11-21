//! Behaviour that the shared crates give the stacks of this crate: bounds, allocation, scale and ranges.

use crate::allocations;
use crate::common;
use sdrkit_core::{Complex32, StackDescriptor, Value};
use sdrkit_stack_avdata::{acars, airband, hfdl, selcal, vdl2, vdl4};
use sdrkit_util_geo::Position;

/// The VDL2 Reed-Solomon decoder refuses an erasure index out of range or too many erasures, and leaves the block as it was.
#[test]
fn vdl2_reed_solomon_refuses_erasures_out_of_range() {
    let data: Vec<u8> = (0..40).map(|i| (i * 7 + 3) as u8).collect();
    let mut c = vdl2::phy::Codec::default();
    let mut par = [0u8; 6];
    assert!(c.parity(&data, &mut par));
    let mut x = data.clone();
    assert_eq!(c.correct(&mut x, &par[..4], &[40]), None);
    assert_eq!(c.correct(&mut x, &par[..4], &[usize::MAX]), None);
    assert_eq!(c.correct(&mut x, &par[..2], &[0, 1, 2]), None);
    assert_eq!(c.correct(&mut x, &[0; 7], &[]), None);
    assert_eq!(x, data);
    x[5] ^= 0x10;
    assert_eq!(c.correct(&mut x, &par[..4], &[5]), Some(1));
    assert_eq!(x, data);
}

/// The VDL2 body decoder keeps its codes and buffers: a burst allocates nothing once each length was seen.
#[test]
fn vdl2_body_decoding_allocates_nothing_once_warm() {
    let mut codec = vdl2::phy::Codec::default();
    let hdlc: Vec<u8> = (0..300 * 8).map(|i| ((i * 7 % 11) < 5) as u8).collect();
    let mut bits = Vec::new();
    vdl2::phy::burst_bits(&mut codec, &hdlc, &mut bits);
    let mut s = vdl2::phy::scrambler();
    let mut d: Vec<u8> = bits.iter().map(|&b| b ^ s.bit()).collect();
    let (len, _) = vdl2::phy::read_header(&d).unwrap();
    d[25 + 8 * 7] ^= 1;
    let mut dec = vdl2::phy::BodyDecoder::default();
    let mut body = vdl2::phy::Body::default();
    dec.decode(&d[25..], len, &mut body);
    assert_eq!(body.hdlc, hdlc);
    let n = allocations(|| {
        for _ in 0..100 {
            dec.decode(&d[25..], len, &mut body);
        }
    });
    assert_eq!(n, 0, "allocations in 100 bursts");
    assert_eq!((body.corrected, body.failed), (1, false));
}

/// The HFDL data decoder takes quantised LLRs whatever their scale, and allocates nothing once each rate was seen.
#[test]
fn hfdl_viterbi_takes_any_scale_and_allocates_nothing_once_warm() {
    let mut dec = hfdl::phy::DataDecoder::default();
    let mut out = Vec::new();
    for idx in [3usize, 7] {
        let r = hfdl::phy::rate(idx);
        let pdu: Vec<u8> = (0..r.octets()).map(|i| (i * 13 + 5) as u8).collect();
        let s = hfdl::phy::burst_symbols(&pdu, r);
        let pts: Vec<Complex32> = (0..r.blocks)
            .flat_map(|b| {
                let a = hfdl::phy::PREAMBLE + b * 45;
                s[a..a + 30].to_vec()
            })
            .collect();
        // Weights so large that summed f32 metrics overflow.
        let w = vec![1e36f32; pts.len()];
        dec.decode(&pts, &w, r, &mut out);
        assert_eq!(out, pdu, "rate {idx}");
        let n = allocations(|| {
            for _ in 0..10 {
                dec.decode(&pts, &w, r, &mut out);
            }
        });
        assert_eq!(n, 0, "rate {idx}");
        assert_eq!(out, pdu);
    }
}

/// Above about 42 MS/s, the CIC of the airband channel picks an input resolution that fits its register, refuses one that
/// wraps, and the channel then decimates with a lower order.
#[test]
fn airband_cic_holds_full_scale_above_42_msps() {
    for rate in [50e6f64, 61.44e6] {
        // To about 48 kS/s.
        let r = (rate / 48e3).floor() as usize;
        let mut cic = airband::channel::channel_cic(r).unwrap();
        let x = vec![Complex32::new(1.0, -1.0); 20 * r];
        let mut y = Vec::new();
        cic.process(&x, &mut y);
        let last = *y.last().unwrap();
        assert!(
            (last - Complex32::new(1.0, -1.0)).norm() < 1e-3,
            "{rate}: {last}"
        );
    }
    assert!(sdrkit_algo_dsp::cic::Cic::<Complex32>::new(5, (61.44e6f64 / 48e3) as usize).is_err());
}

/// Dead reckoning goes along the WGS-84 ellipsoid, so positions at long range are right.
#[test]
fn dead_reckoning_follows_the_ellipsoid() {
    let fix = Position::new(60.0, 10.0);
    // 45 minutes at 480 kt towards the north-east: 360 nautical miles.
    let p = vdl2::tables::dead_reckoning(fix, 2700.0, 45.0, 480.0);
    let d = fix.distance_m(&p);
    assert!((d - 360.0 * 1852.0).abs() < 1.0, "{d}");
    assert!((fix.initial_bearing_deg(&p) - 45.0).abs() < 1e-6);
    let q = fix.destination(45.0, 360.0 * 1852.0);
    assert!(p.distance_m(&q) < 1e-3);
    // A flat-earth step of 360 nm north-east lands tens of kilometres away from the true point.
    let nm = 360.0;
    let dlat = nm * 45f64.to_radians().cos() / 60.0;
    let dlon = nm * 45f64.to_radians().sin() / 60.0 / 60f64.to_radians().cos();
    let flat = Position::new(60.0 + dlat, 10.0 + dlon);
    assert!(flat.distance_m(&p) > 10_000.0);
    // Outside the window, the fix itself.
    assert_eq!(vdl2::tables::dead_reckoning(fix, 3000.0, 45.0, 480.0), fix);
}

/// Only one stack has the id `selcal`. It belongs to this crate, on the decoder of sdrkit-proto-tones.
#[test]
fn the_selcal_stack_id_is_the_shared_one() {
    assert_eq!(selcal::DESCRIPTOR.id, sdrkit_proto_tones::selcal::STACK_ID);
    let ids: Vec<&str> = sdrkit_stack_avdata::descriptors()
        .iter()
        .map(|d| d.id)
        .collect();
    assert_eq!(ids.iter().filter(|i| **i == "selcal").count(), 1);
}

/// A stack, its rate and centre, and settings it must refuse.
type Case<'a> = (&'a StackDescriptor, f64, f64, &'a [(&'a str, Value)]);

/// Every stack refuses a bad setting value, and the setting keeps its value.
#[test]
fn every_stack_refuses_bad_settings() {
    let bad: &[Case] = &[
        (
            &acars::DESCRIPTOR,
            2e6,
            131.5e6,
            &[
                ("threshold_db", Value::F64(99.0)),
                ("channels", Value::Text("131.525 abc".into())),
                ("channels", Value::Text("150.0".into())),
                ("join_blocks", Value::Text("maybe".into())),
            ],
        ),
        (
            &vdl2::DESCRIPTOR,
            1e6,
            136.85e6,
            &[
                ("tune_error_hz", Value::Hz(9999.0)),
                ("channels", Value::Text("1369".into())),
            ],
        ),
        (
            &vdl4::DESCRIPTOR,
            1e6,
            136.85e6,
            &[("channels", Value::Text("200".into()))],
        ),
        (
            &hfdl::DESCRIPTOR,
            48e3,
            8.92e6,
            &[
                ("channel_filter_hz", Value::Hz(100.0)),
                ("channels", Value::Text("50".into())),
            ],
        ),
        (
            &airband::DESCRIPTOR,
            1e6,
            118.75e6,
            &[
                ("squelch_db", Value::F64(99.0)),
                ("hang_s", Value::Duration(-1.0)),
                ("channels", Value::Text("v3:garbage".into())),
            ],
        ),
        (
            &selcal::DESCRIPTOR,
            500e3,
            8.941e6,
            &[("demod", Value::Choice(99))],
        ),
    ];
    for (d, rate, centre, cases) in bad {
        let mut rx = common::receiver(d, *rate, *centre, *centre, &[]);
        for (k, v) in cases.iter() {
            assert!(
                common::set(rx.as_mut(), k, v).is_err(),
                "{} accepted {k} = {v:?}",
                d.id
            );
        }
    }
}
