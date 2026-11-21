//! One channel on a generated signal : acquisition, lock, C/N0, Doppler and the navigation frames.

use sdrkit_core::Complex32;
use sdrkit_stack_gnss::acq::{Integration, Searcher};
use sdrkit_stack_gnss::codes::{self, SignalKind};
use sdrkit_stack_gnss::frontend::{BandBuffer, BandResampler, Conditioner, SinCos};
use sdrkit_stack_gnss::gen::{GenOptions, GnssGenerator};
use sdrkit_stack_gnss::system::System;
use sdrkit_stack_gnss::track::{ChanEvent, Channel};

#[test]
fn a_gps_channel_locks_and_frames() {
    let fs = 4e6;
    let mut g = GnssGenerator::new(GenOptions {
        max_sats: Some(1),
        ..Default::default()
    });
    let mut sky = crate::Sky::new(&g);
    let sat = g.rendered().next().unwrap().clone();
    let lut = SinCos::new();
    let mut cond = Conditioner::default();
    let mut rs = BandResampler::new(fs, 4.096e6, 0.0);
    let mut band = BandBuffer::default();
    let mut x = vec![Complex32::new(0.0, 0.0); 16384];
    // 40 ms for the search.
    while band.data.len() < 40 * 4096 + 8192 {
        sky.render(&mut g, &mut x);
        cond.process(&mut x, fs);
        rs.process(&x, &lut, &mut band.data);
    }
    let kind = SignalKind::L1Ca;
    let mut s = Searcher::new(kind);
    let chips = codes::l1_ca(sat.id.prn).unwrap();
    s.add_code(sat.id.prn, &chips);
    let integ = Integration {
        periods: 16,
        coherent: 2,
    };
    let seg = band.slice(0, 16 * 4096).unwrap().to_vec();
    let mut job = s.start(
        sat.id.prn, &seg, 0, 0.0, 10e3, 4, integ, 1575.42e6, 0.0, 1e-3, None,
    );
    let mut res = None;
    while res.is_none() {
        let mut b = 1e9;
        res = s.step(&mut job, &mut b);
    }
    let (hit, _) = res.unwrap();
    let truth = g.truth_doppler(sat.id, g.start_time()).unwrap();
    assert!(hit.found, "{hit:?}");
    assert!(
        (hit.doppler_hz - truth).abs() < 150.0,
        "{} {}",
        hit.doppler_hz,
        truth
    );
    let mut ch = Channel::start(
        sat.id,
        kind,
        1575.42e6,
        &chips,
        &hit,
        band.end(),
        0.0,
        0.0,
        0.0,
    );
    let mut ev = Vec::new();
    let mut frames = 0;
    let mut locked_at = None;
    for k in 0..(40.0 * fs / 16384.0) as usize {
        sky.render(&mut g, &mut x);
        cond.process(&mut x, fs);
        rs.process(&x, &lut, &mut band.data);
        let t = (k + 1) as f64 * 16384.0 / fs;
        assert!(ch.run(&band, &lut, t, &mut ev), "{ev:?}");
        for e in ev.drain(..) {
            match e {
                ChanEvent::Locked => locked_at = Some(t),
                ChanEvent::Unit { .. } => frames += 1,
                ChanEvent::Lost { reason, .. } => panic!("{reason}"),
                ChanEvent::Bad => panic!("bad subframe"),
                _ => {}
            }
        }
        let keep = ch.needs_from();
        band.trim(keep, 1 << 19);
    }
    let lock = locked_at.expect("locked");
    assert!(lock < 2.0, "{lock}");
    let t_end = g.start_time() + 40.0;
    let td = g.truth_doppler(sat.id, t_end).unwrap();
    assert!((ch.carr_freq - td).abs() < 5.0, "{} {}", ch.carr_freq, td);
    assert!(
        (ch.cn0.value - sat.cn0).abs() < 2.0,
        "{} {}",
        ch.cn0.value,
        sat.cn0
    );
    assert!(frames >= 4, "{frames}");
    assert_eq!(sat.id.system, System::Gps);
}
