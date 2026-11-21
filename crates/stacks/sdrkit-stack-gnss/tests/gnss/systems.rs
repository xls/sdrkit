//! One channel per system on the generator: acquisition, lock, the message layer, and the navigation data it decodes, compared
//! with what the generator sent. Galileo uses the generator's synthetic E1-B code table (the ICD's table is not carried here).

use sdrkit_core::Complex32;
use sdrkit_stack_gnss::acq::{Integration, Searcher};
use sdrkit_stack_gnss::codes::{self, E1bTable, SignalKind};
use sdrkit_stack_gnss::frontend::{BandBuffer, BandResampler, Conditioner, SinCos};
use sdrkit_stack_gnss::gen::{GenOptions, GnssGenerator, GALILEO_TABLE_SEED};
use sdrkit_stack_gnss::navstore::NavStore;
use sdrkit_stack_gnss::orbit::Kepler;
use sdrkit_stack_gnss::system::{glo_carrier, SatId, System};
use sdrkit_stack_gnss::track::{ChanEvent, Channel};

struct Run {
    nav: NavStore,
    units: usize,
    bad: u64,
    lock: Option<f64>,
    ch: Channel,
}

/// Renders only `pick` of the generator and runs one channel on it for `secs`.
fn run(
    opt: GenOptions,
    pick: impl Fn(&sdrkit_stack_gnss::gen::SimSat) -> bool,
    secs: f64,
) -> (Run, GnssGenerator, SatId) {
    let fs = opt.rate_hz;
    let centre = opt.centre_hz;
    let mut g = GnssGenerator::new(opt);
    let mut sky = crate::Sky::new(&g);
    let idx = g
        .sats
        .iter()
        .position(|s| s.rendered && pick(s))
        .expect("a satellite to track");
    for (i, s) in g.sats.iter_mut().enumerate() {
        s.rendered = i == idx;
    }
    let sat = g.sats[idx].clone();
    let kind = sat.kind;
    let prn_code = match sat.id.system {
        System::Galileo => E1bTable::synthetic(GALILEO_TABLE_SEED, 36)
            .code(sat.id.prn)
            .unwrap()
            .to_vec(),
        System::Beidou => codes::bds_b1i(sat.id.prn).unwrap(),
        System::Glonass => codes::glonass_code(),
        _ => codes::l1_ca(sat.id.prn).unwrap(),
    };
    let lut = SinCos::new();
    let mut cond = Conditioner::default();
    let mut rs = BandResampler::new(fs, kind.band_rate(), sat.carrier - centre);
    let mut band = BandBuffer::default();
    let mut x = vec![Complex32::new(0.0, 0.0); 16_384];
    let n = kind.period_samples();
    let k = match kind {
        SignalKind::E1b => 4,
        _ => 16,
    };
    while band.data.len() < (k + 4) * n {
        sky.render(&mut g, &mut x);
        cond.process(&mut x, fs);
        rs.process(&x, &lut, &mut band.data);
    }
    let mut s = Searcher::new(kind);
    let key = if sat.id.system == System::Glonass {
        0
    } else {
        sat.id.prn
    };
    s.add_code(key, &prn_code);
    let integ = Integration {
        periods: k,
        coherent: if kind == SignalKind::L1Ca { 2 } else { 1 },
    };
    let subs = if kind == SignalKind::E1b { 2 } else { 4 };
    let seg = band.slice(0, k * n).unwrap().to_vec();
    let mut job = s.start(
        key,
        &seg,
        0,
        0.0,
        6e3,
        subs,
        integ,
        sat.carrier,
        0.0,
        1e-3,
        None,
    );
    let hit = loop {
        let mut b = 1e9;
        if let Some((h, _)) = s.step(&mut job, &mut b) {
            break h;
        }
    };
    assert!(hit.found, "{:?} {hit:?}", sat.id);
    let id = if sat.id.system == System::Glonass {
        SatId::glo_channel(sat.glo_k)
    } else {
        sat.id
    };
    let mut ch = Channel::start(
        id,
        kind,
        sat.carrier,
        &prn_code,
        &hit,
        band.end(),
        0.0,
        0.0,
        0.0,
    );
    ch.set_glo_k(sat.glo_k);
    let mut nav = NavStore::new(2400);
    let mut ev = Vec::new();
    let mut units = 0;
    let mut lock = None;
    for i in 0..(secs * fs / 16_384.0) as usize {
        sky.render(&mut g, &mut x);
        cond.process(&mut x, fs);
        rs.process(&x, &lut, &mut band.data);
        let now = (i + 1) as f64 * 16_384.0 / fs;
        assert!(ch.run(&band, &lut, now, &mut ev), "{:?}: {ev:?}", sat.id);
        for e in ev.drain(..) {
            match e {
                ChanEvent::Locked => lock = Some(now),
                ChanEvent::Unit { unit, time, .. } => {
                    units += 1;
                    nav.add(ch.sat, sat.glo_k, &unit, time, now);
                }
                _ => {}
            }
        }
        band.trim(ch.needs_from(), 1 << 17);
    }
    let bad = ch.bad;
    (
        Run {
            nav,
            units,
            bad,
            lock,
            ch,
        },
        g,
        sat.id,
    )
}

fn same(a: &Kepler, b: &Kepler) {
    let pairs = [
        ("toe", a.toe, b.toe),
        ("toc", a.toc, b.toc),
        ("sqrt_a", a.sqrt_a, b.sqrt_a),
        ("e", a.e, b.e),
        ("m0", a.m0, b.m0),
        ("delta_n", a.delta_n, b.delta_n),
        ("omega0", a.omega0, b.omega0),
        ("i0", a.i0, b.i0),
        ("omega", a.omega, b.omega),
        ("omega_dot", a.omega_dot, b.omega_dot),
        ("idot", a.idot, b.idot),
        ("cuc", a.cuc, b.cuc),
        ("cus", a.cus, b.cus),
        ("crc", a.crc, b.crc),
        ("crs", a.crs, b.crs),
        ("cic", a.cic, b.cic),
        ("cis", a.cis, b.cis),
        ("af0", a.af0, b.af0),
        ("af1", a.af1, b.af1),
        ("tgd", a.tgd, b.tgd),
    ];
    for (n, x, y) in pairs {
        assert!(
            (x - y).abs() <= 1e-12 * y.abs().max(1e-20),
            "{n}: {x} != {y}"
        );
    }
    assert_eq!(a.week, b.week);
}

#[test]
fn galileo_inav_channel() {
    let (r, g, id) = run(
        GenOptions {
            systems: System::Galileo.bit(),
            ..Default::default()
        },
        |s| s.cn0 > 36.0,
        40.0,
    );
    assert!(r.lock.is_some());
    assert!(r.units > 10, "{} pages", r.units);
    assert!(r.bad <= 1);
    let e = r.nav.eph.get(&id).expect("Galileo ephemeris");
    same(e, &g.broadcast_kepler(id).unwrap());
    let ggto = r.nav.ggto.expect("GST-GPS");
    assert!((ggto.a0g - 25e-9).abs() < 0.1e-9);
    assert!(r.ch.time.is_some());
}

#[test]
fn beidou_d1_meo_channel() {
    let (r, g, id) = run(
        GenOptions {
            systems: System::Beidou.bit(),
            centre_hz: 1561.098e6,
            ..Default::default()
        },
        |s| !sdrkit_stack_gnss::system::bds_is_geo(s.id.prn) && s.cn0 > 36.0,
        50.0,
    );
    assert!(r.lock.is_some());
    assert!(r.units >= 5, "{} subframes", r.units);
    assert_eq!(r.bad, 0);
    let e = r.nav.eph.get(&id).expect("BeiDou D1 ephemeris");
    same(e, &g.broadcast_kepler(id).unwrap());
    assert!(r.nav.bdt_gps.is_none() || (r.nav.bdt_gps.unwrap().0 - 30e-9).abs() < 0.1e-9);
}

#[test]
fn beidou_d2_geo_channel() {
    let (r, g, id) = run(
        GenOptions {
            systems: System::Beidou.bit(),
            centre_hz: 1561.098e6,
            mask_deg: 0.0,
            ..Default::default()
        },
        |s| sdrkit_stack_gnss::system::bds_is_geo(s.id.prn),
        60.0,
    );
    assert!(r.lock.is_some());
    assert!(r.units >= 40, "{} subframes", r.units);
    assert_eq!(r.bad, 0);
    let e = r.nav.eph.get(&id).expect("BeiDou D2 ephemeris");
    same(e, &g.broadcast_kepler(id).unwrap());
    // The GEO orbit model of the receiver against the generator's truth at the start.
    let t = g.start_time() - 14.0;
    let p = e.position(t);
    let truth = match &g.sats.iter().find(|s| s.id == id).unwrap().orbit {
        sdrkit_stack_gnss::gen::Orbit::Kepler(el, k) => el.ecef(t - el.toe, k).0,
        _ => unreachable!(),
    };
    assert!(sdrkit_stack_gnss::geo::dist(p, truth) < 1e-3);
}

#[test]
fn glonass_channel_and_orbit() {
    let (r, g, _) = run(
        GenOptions {
            systems: System::Glonass.bit(),
            centre_hz: 1602e6,
            rate_hz: 10e6,
            ..Default::default()
        },
        |s| s.cn0 > 36.0,
        40.0,
    );
    assert!(r.lock.is_some());
    assert!(r.units >= 10, "{} strings", r.units);
    assert!(r.bad <= 1, "{}", r.bad);
    // The ephemeris is filed under the slot number from string 4.
    let (slot_id, e) = r.nav.glo_eph.iter().next().expect("GLONASS ephemeris");
    assert!(slot_id.glo_k().is_none(), "{slot_id:?}");
    let sim = g.sats.iter().find(|s| s.id == *slot_id).unwrap();
    assert_eq!(e.k, sim.glo_k);
    assert_eq!(e.tb, sim.glo_tb);
    // the orbit from the broadcast state within 0.05 m of the truth in range.
    let truth = match &sim.orbit {
        sdrkit_stack_gnss::gen::Orbit::Glo(t) => t.clone(),
        _ => unreachable!(),
    };
    let tod = (g.start_time() + 10_800.0 - 18.0).rem_euclid(86_400.0);
    let (p, _) = e.integrate(tod, 30.0);
    let q = truth.pos(tod);
    let rx = g.receiver();
    let dr = sdrkit_stack_gnss::geo::dist(p, rx) - sdrkit_stack_gnss::geo::dist(q, rx);
    assert!(dr.abs() < 0.05, "{dr}");
    let t = r.nav.glo_time.expect("string 5");
    assert!((t.tau_gps - 20e-9).abs() < 1e-9);
    let _ = glo_carrier(0);
}
