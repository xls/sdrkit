//! Round trips through the stack descriptors: the generator, independent impairments (sdrkit-dev-test), the receiver graph.

use sdrkit_algo_sim::front::Quantiser;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_core::publish::keys;
use sdrkit_core::{
    owner, Complex32, ConfigError, InputInfo, LockState, Output, Owner, Receiver, StackConfig,
    Value, Values,
};
use sdrkit_stack_gnss::components::{GnssCoreComponent, SbasContentComponent};
use sdrkit_stack_gnss::gen::{GenOptions, GnssGenerator};
use sdrkit_stack_gnss::geo;
use sdrkit_stack_gnss::system::System;
use sdrkit_stack_gnss::typed::{
    GnssFields, GnssSatelliteRow, SbasFields, SbasMessageRow, FIX, NMEA, SBAS_MESSAGES,
};

/// The stack's settings at their defaults, with the known week 2400 and `extra`.
fn settings(desc: &sdrkit_core::StackDescriptor, extra: &[(&str, Value)]) -> Values {
    let schema = (desc.settings)();
    let mut v = Values::defaults(&schema);
    if let Ok(id) = schema.id("known_week") {
        v.set(id, &Value::I64(2400)).unwrap();
    }
    for (k, x) in extra {
        v.set(schema.id(k).unwrap(), x).unwrap();
    }
    v
}

/// GPS, 8-bit quantised, a radio error of +1.5 kHz added by the impairment, 40 s.
#[test]
fn gps_through_the_stack() {
    let fs = 4e6;
    let desc = &sdrkit_stack_gnss::GNSS;
    let gen_cfg = GenOptions {
        systems: System::Gps.bit(),
        ..Default::default()
    };
    let mut g = GnssGenerator::new(gen_cfg);
    let mut sky = crate::Sky::new(&g);
    let n_tx = g.rendered().count();
    let truth_cn0: Vec<(String, f64)> = g.rendered().map(|s| (s.id.name(), s.cn0)).collect();
    let mut rx = sdrkit_stack_gnss::gnss_receiver(&StackConfig::new(settings(
        desc,
        &[
            ("sys_galileo", Value::Bool(false)),
            ("sys_beidou", Value::Bool(false)),
            ("sys_glonass", Value::Bool(false)),
        ],
    )))
    .unwrap();
    rx.configure(&InputInfo::new(fs, 1575.42e6, 1575.42e6).recording())
        .unwrap();
    let mut off = FrequencyOffset::new(fs, 1500.0, 0.0).unwrap();
    let q = Quantiser::new(8).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); 65_536];
    let mut out = Output::default();
    let mut nmea = 0;
    let mut fix_events = 0;
    let secs = 40.0;
    for _ in 0..(secs * fs) as usize / x.len() {
        sky.render(&mut g, &mut x);
        off.process(&mut x);
        // About 0.25 rms into the 8-bit quantiser.
        let gain = 0.25 / sdrkit_dev_test::measure::power(&x).sqrt() as f32;
        sdrkit_algo_sim::front::scale(&mut x, gain);
        q.process(&mut x);
        rx.feed(&x, &mut out);
        for e in out.events.drain(..) {
            if e.is(&NMEA) {
                nmea += 1;
                let s = e.get(&keys::TEXT).unwrap();
                assert!(s.starts_with("$GPRMC") || s.starts_with("$GPGGA"));
            }
            if e.is(&FIX) {
                fix_events += 1;
                assert!(e.get(&keys::POSITION).is_some());
            }
        }
        out.clear();
    }
    rx.refresh_results();
    let r = rx.results();
    let core = &rx
        .graph()
        .component_as::<GnssCoreComponent>("rx")
        .unwrap()
        .core;
    let d = core.last_report();
    for l in r.log_lines().iter().rev().take(15) {
        eprintln!("{l}");
    }
    assert_eq!(d.state, 2, "{}", r.status().summary);
    let fix = d.fix.as_ref().unwrap();
    let ecef = fix.ecef;
    let truth = g.receiver();
    let tg = geo::ecef_to_geodetic(truth);
    let enu = geo::ecef_to_enu(
        tg,
        [ecef[0] - truth[0], ecef[1] - truth[1], ecef[2] - truth[2]],
    );
    let herr = (enu[0].powi(2) + enu[1].powi(2)).sqrt();
    eprintln!("{} error {enu:?}", fix.type_text);
    assert!(herr < 8.0 && herr > 0.05, "{enu:?}");
    assert!(enu[2].abs() < 15.0, "{enu:?}");
    // The estimated error between 1/6 and 6 times the real one + 5 m.
    let est = fix.herr;
    assert!(
        est > herr / 6.0 && est < 6.0 * herr + 5.0,
        "{est} vs {herr}"
    );
    assert!(
        (22.0..=32.0).contains(&fix.first_fix_s),
        "first fix {}",
        fix.first_fix_s
    );
    let used = fix.satellites;
    assert!(used + 1 >= n_tx, "{used} of {n_tx}");
    assert_eq!(fix.type_text, format!("GPS {used} satellites"));
    assert_eq!(fix.week, 2400);
    assert_eq!(fix.leap, Some(18));
    // GPS time of week within 5 µs of the truth at the fix's receive instant.
    let tow_truth = g.time_of_sample(fix.signal_s * fs);
    assert!(
        (fix.tow - tow_truth).abs() < 5e-6,
        "{} {tow_truth}",
        fix.tow
    );
    let utc = geo::utc_unix(2400, tow_truth, 18);
    assert!((fix.utc.unwrap() - utc).abs() < 1e-5);
    // The frequency error: the +1.5 kHz the impairment added.
    let fe = d.frequency_error_hz.unwrap();
    assert!((fe - 1500.0).abs() < 3.0, "{fe}");
    // Channels: C/N0 near the truth, all framed, no bad subframes, azimuth and elevation near the truth.
    let sky = sdrkit_stack_gnss::gen::sky_table(&g);
    for c in &d.channels {
        let name = c.name.as_str();
        let t = truth_cn0
            .iter()
            .find(|t| t.0 == name)
            .unwrap_or_else(|| panic!("{name} not transmitted"));
        assert!((c.cn0 - t.1).abs() < 2.5, "{name} {} vs {}", c.cn0, t.1);
        assert!(c.state_level >= 3, "{name} {:?}", c.state);
        assert_eq!(c.bad, 0, "{name}");
        let (az, el, _) = sky[name];
        // The azimuth error as an angle on the sky (near the zenith a large azimuth step is a small angle).
        let daz = (c.az.unwrap() - az) * el.to_radians().cos();
        assert!(daz.abs() < 1.5, "{name} az {:?} {az} el {el}", c.az);
        assert!((c.el.unwrap() - el).abs() < 1.5, "{name} el");
    }
    let tracked = d.channels.iter().filter(|c| c.state_level >= 1).count();
    assert!(tracked + 1 >= n_tx);
    let f = r.fields();
    let frames_ok = f.get(&keys::FRAMES_OK).unwrap();
    assert!(frames_ok > 40, "{frames_ok}");
    assert_eq!(f.get(&keys::FRAMES_BAD), Some(0));
    assert_eq!(f.at(&GnssFields::LEAP_SECONDS), Some(18));
    assert!(f.at(&GnssFields::UTC).is_some());
    assert!(d.iono_valid && d.utc_valid);
    assert_eq!(d.utc.unwrap().dtls, 18);
    // In 40 s one frame of subframes 4 and 5 comes after frame sync: page 18 of subframe 5 holds one almanac.
    assert!(d.almanac_count >= 1);
    assert!(d.scatter.len() <= 400);
    assert_eq!(d.acq_plot.as_ref().unwrap().points.len(), 256);
    let best = r.status().snr_db.unwrap();
    assert!((40.0..=46.0).contains(&best), "{best}");
    let lvl = r.status().level_dbfs.unwrap();
    assert!((-20.0..=-14.0).contains(&lvl), "{lvl}");
    assert!(d.sky.len() >= n_tx);
    let sats = r.table(r.decl().table_id("satellites").unwrap()).unwrap();
    assert!(sats.len() >= n_tx);
    let used_rows = sats
        .rows_newest_first()
        .filter(|(_, row)| row.at(&GnssSatelliteRow::USED) == Some(true))
        .count();
    assert_eq!(used_rows, used);
    assert!(nmea >= 2, "{nmea}");
    assert!(fix_events >= 1);
    assert!(serde_json::to_string(r).unwrap().len() < 100_000);
}

#[test]
fn a_rate_too_low_is_refused() {
    let desc = &sdrkit_stack_gnss::GNSS;
    let mut rx = (desc.make_receiver)(&StackConfig::new(settings(desc, &[]))).unwrap();
    let e = rx.configure(&InputInfo::new(1.5e6, 1575.42e6, 1575.42e6).recording());
    assert!(
        matches!(e, Err(ConfigError::RateTooLow { need, .. }) if need == 2.046e6),
        "{e:?}"
    );
    let mut out = Output::default();
    rx.feed(&vec![Complex32::new(0.0, 0.0); 100_000], &mut out);
    let r = rx.results();
    assert!(r
        .decl()
        .table_id("satellites")
        .and_then(|t| r.table(t))
        .is_none_or(|t| t.is_empty()));
}

/// The SBAS generator (PRN 123, 127, 128, structured content) at 4 Msps, 45 s, through the `sbas` stack, without noise.
#[test]
fn sbas_stack() {
    let fs = 4e6;
    let desc = &sdrkit_stack_gnss::SBAS;
    let mut g = GnssGenerator::new(sdrkit_stack_gnss::settings::gen_options(
        &settings(desc, &[]),
        fs,
        1,
        true,
        None,
    ));
    let mut sky = crate::Sky::new(&g);
    let mut rx = sdrkit_stack_gnss::sbas_receiver(&StackConfig::new(settings(desc, &[]))).unwrap();
    rx.configure(&InputInfo::new(fs, 1575.42e6, 1575.42e6).recording())
        .unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); 65_536];
    let mut out = Output::default();
    let secs = 45.0;
    for _ in 0..(secs * fs) as usize / x.len() {
        sky.render(&mut g, &mut x);
        rx.feed(&x, &mut out);
        out.clear();
    }
    rx.refresh_results();
    let r = rx.results();
    for l in r.log_lines().iter().rev().take(10) {
        eprintln!("{l}");
    }
    assert_eq!(
        r.status().lock_state,
        LockState::Locked,
        "{}",
        r.status().summary
    );
    let content = &rx
        .graph()
        .component_as::<SbasContentComponent>("sbas")
        .unwrap()
        .content;
    let prns: Vec<u16> = content.sats.keys().copied().collect();
    assert_eq!(prns, vec![123, 127, 128]);
    let types_sent = [9u64, 2, 3, 4, 5, 18, 26, 25, 7, 10, 12, 17, 24, 27, 28, 1];
    // The time a satellite was heard; messages at least 0.45 a second of that.
    for (prn, s) in &content.sats {
        let msgs = s.messages as f64;
        let span = s.last_s - s.first_s + 1.0;
        assert!(msgs >= 0.45 * span, "{prn}: {msgs} in {span} s");
        assert!(msgs > 10.0, "{prn}: {msgs}");
        assert_eq!(s.bad, 0, "{prn}");
        let mask = s.mask.as_ref().unwrap();
        assert_eq!(mask.iodp, 1);
        assert_eq!(mask.slots.len(), 15);
        assert_eq!(mask.slots[14], *prn);
        let fast = s.fast.iter().flatten().filter(|f| f.prc.is_some()).count();
        assert!(fast >= 38, "{prn}: {fast} fast slots");
        let p = s.geo.unwrap().pos;
        let rr = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        assert!((rr - 42_164_169.0).abs() < 100.0, "{rr}");
        assert_eq!(s.time.unwrap().dtls, 18);
        assert_eq!(s.almanac.len(), 3);
        assert_eq!(s.iono[&0].points.len(), 60);
        let seen = s.types_seen;
        assert!(seen.count_ones() >= 12, "{prn}: types {seen:b}");
        for t in 0..64u64 {
            if seen >> t & 1 == 1 {
                assert!(types_sent.contains(&t), "{prn}: type {t} not sent");
            }
        }
    }
    assert!(r
        .log_lines()
        .iter()
        .any(|l| l.contains("type 1") || l.contains("PRN mask")));
    assert!(
        r.status().summary.starts_with("SBAS: 3 satellites tracked"),
        "{}",
        r.status().summary
    );
    let rows = |key: &str| r.table(r.decl().table_id(key).unwrap()).unwrap().len();
    assert!(rows("fast_corrections") > 0 && rows("ionosphere") > 0);
    assert_eq!(rows("sbas_satellites"), 3);
    assert_eq!(
        r.fields().at(&SbasFields::SBAS_MESSAGES),
        Some(content.total as i64)
    );
    let total = content.total as i64;
    // Select another satellite through the stack's command, then refuse one that is not heard.
    let select = rx.commands().id("select").unwrap();
    let mut args = rx.commands().def(select).unwrap().new_args();
    let row = args.decl().id("row").unwrap();
    args.set_value(row, &Value::Text("128".into())).unwrap();
    owner::invoke(&mut rx, select, &args).unwrap();
    rx.refresh_results();
    assert_eq!(rx.results().fields().at(&SbasFields::SELECTED), Some(128));
    args.set_value(row, &Value::Text("140".into())).unwrap();
    assert!(owner::invoke(&mut rx, select, &args).is_err());
    // A change of frequency by the host: the message log, the counts by type and the count of messages stay. The
    // corrections of the satellites and the other fields of the same component go.
    let logged = SBAS_MESSAGES.rows::<SbasMessageRow>(rx.results()).count();
    assert!(logged > 0);
    rx.forget();
    let r = rx.results();
    let rows = |key: &str| r.table(r.decl().table_id(key).unwrap()).unwrap().len();
    assert_eq!(SBAS_MESSAGES.rows::<SbasMessageRow>(r).count(), logged);
    assert!(rows("types") > 0);
    assert_eq!(r.fields().at(&SbasFields::SBAS_MESSAGES), Some(total));
    assert_eq!(
        (rows("fast_corrections"), rows("ionosphere"), rows("sbas_satellite_types")),
        (0, 0, 0)
    );
    assert_eq!(r.fields().at(&SbasFields::SELECTED), None);
    assert_eq!(r.fields().at(&SbasFields::DATA_VALID), Some(false));
    // The count of the guard of the receiver is a field of the stack.
    assert!(r.fields().get(&keys::BAD_SAMPLES).is_some());
}
