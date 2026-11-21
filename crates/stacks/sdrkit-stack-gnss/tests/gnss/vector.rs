//! The recorded vector `tests/vectors/gnss/gnss_l1_multi_4m` against its `expected.json`: the front end, the acquisition of
//! every L1 code family, tracking and the whole stack.

use sdrkit_core::{Complex32, InputInfo, Output, Owner, Receiver, StackConfig, Value, Values};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_stack_gnss::acq::{AcqHit, Integration, Searcher};
use sdrkit_stack_gnss::codes::{self, SignalKind};
use sdrkit_stack_gnss::frontend::{BandBuffer, BandResampler, Conditioner, Excision, SinCos};
use sdrkit_stack_gnss::system::{SatId, System};
use sdrkit_stack_gnss::track::{ChanEvent, Channel};
use std::collections::BTreeMap;

struct Prepared {
    band: BandBuffer,
    level_dbfs: f64,
    clip: f64,
    dc: (f64, f64),
    x: Vec<Complex32>,
}

fn prepare() -> Option<Prepared> {
    let rec = sdrkit_dev_test::read_recording("gnss/gnss_l1_multi_4m")?;
    assert_eq!(rec.rate_hz, 4e6);
    let lut = SinCos::new();
    let mut cond = Conditioner::default();
    let mut rs = BandResampler::new(rec.rate_hz, 4.096e6, 0.0);
    let mut exc = Excision::default();
    let mut band = BandBuffer::default();
    let mut tmp = Vec::new();
    let mut piece = vec![Complex32::new(0.0, 0.0); 16_384];
    for c in rec.samples.as_chunks::<16_384>().0 {
        piece.copy_from_slice(c);
        cond.process(&mut piece, rec.rate_hz);
        tmp.clear();
        rs.process(&piece, &lut, &mut tmp);
        exc.process(&tmp, &mut band.data);
    }
    Some(Prepared {
        band,
        level_dbfs: cond.level_dbfs(),
        clip: cond.clip_percent(),
        dc: cond.dc,
        x: rec.samples,
    })
}

#[allow(clippy::too_many_arguments)]
fn search(
    s: &mut Searcher,
    band: &BandBuffer,
    prn: u16,
    chips: &[u8],
    centre: f64,
    half: f64,
    integ: Integration,
    subs: usize,
) -> AcqHit {
    s.add_code(prn, chips);
    let n = s.replicas.n;
    let start = 8192u64;
    let seg = band.slice(start, integ.periods * n).unwrap();
    let mut job = s.start(
        prn, seg, start, centre, half, subs, integ, 1575.42e6, 0.0, 1e-3, None,
    );
    loop {
        let mut b = 1e9;
        if let Some((hit, _)) = s.step(&mut job, &mut b) {
            return hit;
        }
    }
}

/// The first code epoch (input samples at 4 Msps) of a hit, modulo the code period near `want`.
fn epoch_sample(hit: &AcqHit, kind: SignalKind, want: f64) -> f64 {
    let fs_in = 4e6;
    let period_in = kind.period_s() * fs_in / (1.0 + hit.doppler_hz / 1575.42e6);
    let first = (hit.seg_start as f64 + hit.code_phase) * fs_in / 4.096e6;
    let k = ((first - want) / period_in).round();
    first - k * period_in
}

#[test]
fn front_end_acquisition_and_tracking() {
    let exp = read_expected("gnss").unwrap();
    let Some(p) = prepare() else {
        return;
    };
    let fe = &exp["front_end"];
    let lv = fe["level_dbfs"].as_array().unwrap();
    assert!(
        p.level_dbfs >= lv[0].as_f64().unwrap() && p.level_dbfs <= lv[1].as_f64().unwrap(),
        "{}",
        p.level_dbfs
    );
    assert!(p.clip <= fe["clip_percent_max"].as_f64().unwrap());
    assert!(
        p.dc.0.abs() <= fe["dc_abs_max"].as_f64().unwrap()
            && p.dc.1.abs() <= fe["dc_abs_max"].as_f64().unwrap()
    );

    let truth: BTreeMap<String, &serde_json::Value> = exp["satellites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["name"].as_str().unwrap().to_string(), s))
        .collect();
    // GPS 1-32, QZSS 193-202, SBAS 120-158 over -10..+10 kHz with 16 ms in 2 ms pairs.
    let mut s = Searcher::new(SignalKind::L1Ca);
    let integ = Integration {
        periods: 16,
        coherent: 2,
    };
    let mut found = Vec::new();
    let mut false_hits = 0;
    let prns: Vec<u16> = (1..=32).chain(193..=202).chain(120..=158).collect();
    let mut hits = BTreeMap::new();
    for prn in prns {
        let sys = System::of_l1_prn(prn).unwrap();
        let name = SatId::new(sys, prn).name();
        let hit = search(
            &mut s,
            &p.band,
            prn,
            &codes::l1_ca(prn).unwrap(),
            0.0,
            10e3,
            integ,
            4,
        );
        if hit.found {
            match truth.get(&name) {
                Some(t) => {
                    let td = t["doppler_hz_at_0_75s"].as_f64().unwrap();
                    assert!(
                        (hit.doppler_hz - td).abs() < 300.0,
                        "{name}: {} vs {td}",
                        hit.doppler_hz
                    );
                    let want = t["first_code_epoch_sample"].as_f64().unwrap();
                    let got = epoch_sample(&hit, SignalKind::L1Ca, want);
                    assert!((got - want).abs() < 1.5, "{name}: epoch {got} vs {want}");
                    found.push(name.clone());
                    hits.insert(name, hit);
                }
                None => false_hits += 1,
            }
        }
    }
    for sys in ["GPS", "QZSS", "SBAS"] {
        for prn in exp["present"][sys].as_array().unwrap() {
            let prn = prn.as_u64().unwrap() as u16;
            let name = SatId::new(System::of_l1_prn(prn).unwrap(), prn).name();
            assert!(found.contains(&name), "{name} not found ({found:?})");
        }
    }
    // at most one false detection over the searches.
    assert!(false_hits <= 1, "{false_hits} false detections");
    eprintln!("A3 (Galileo E1-B): needs the OS SIS ICD Annex C code table, which this crate does not carry; not run");

    // a channel on each detection, tracked to the end of the recording.
    let lut = SinCos::new();
    for (name, hit) in &hits {
        let t = truth[name];
        let sys = System::of_l1_prn(hit.prn).unwrap();
        // Start the channel from the hit at the segment, feeding the band in pieces as it arrived.
        let mut band = BandBuffer::default();
        let start_at = hit.seg_start as usize + 16 * 4096;
        band.data.extend_from_slice(&p.band.data[..start_at]);
        let mut ch = Channel::start(
            SatId::new(sys, hit.prn),
            SignalKind::L1Ca,
            1575.42e6,
            &codes::l1_ca(hit.prn).unwrap(),
            hit,
            band.end(),
            0.0,
            0.0,
            0.0,
        );
        let mut ev = Vec::new();
        let mut lock = None;
        let mut dop_075 = None;
        let mut pos = start_at;
        while pos < p.band.data.len() {
            let end = (pos + 16_384).min(p.band.data.len());
            band.data.extend_from_slice(&p.band.data[pos..end]);
            pos = end;
            let now = pos as f64 / 4.096e6;
            assert!(ch.run(&band, &lut, now, &mut ev), "{name}: {ev:?}");
            for e in ev.drain(..) {
                if e == ChanEvent::Locked {
                    lock = Some(now);
                }
            }
            if dop_075.is_none() && now >= 0.75 {
                dop_075 = Some(ch.carr_freq);
            }
        }
        let lock = lock.unwrap_or_else(|| panic!("{name} did not lock"));
        assert!(lock <= 1.2, "{name}: lock at {lock}");
        let cn0 = t["cn0_dbhz"].as_f64().unwrap();
        assert!(
            (ch.cn0.value - cn0).abs() <= 2.5,
            "{name}: C/N0 {} vs {cn0}",
            ch.cn0.value
        );
        let td = t["doppler_hz_at_0_75s"].as_f64().unwrap();
        assert!(
            (dop_075.unwrap() - td).abs() <= 10.0,
            "{name}: Doppler {} vs {td}",
            dop_075.unwrap()
        );
    }
    let _ = p.x.len();
}

#[test]
fn the_whole_stack_on_the_recording() {
    let exp = read_expected("gnss").unwrap();
    let Some(rec) = sdrkit_dev_test::read_recording("gnss/gnss_l1_multi_4m") else {
        return;
    };
    let desc = &sdrkit_stack_gnss::GNSS;
    let schema = (desc.settings)();
    let mut settings = Values::defaults(&schema);
    settings
        .set(schema.id("known_week").unwrap(), &Value::I64(2400))
        .unwrap();
    let mut rx = sdrkit_stack_gnss::gnss_receiver(&StackConfig::new(settings)).unwrap();
    rx.configure(&InputInfo::new(4e6, 1575.42e6, 1575.42e6).recording())
        .unwrap();
    let mut out = Output::default();
    let mut i = 0;
    let mut k = 0usize;
    while i < rec.samples.len() {
        let n = [16_384, 65_536, 40_000, 23_456][k % 4].min(rec.samples.len() - i);
        rx.feed(&rec.samples[i..i + n], &mut out);
        out.clear();
        i += n;
        k += 1;
    }
    rx.refresh_results();
    let r = rx.results();
    let core = &rx
        .graph()
        .component_as::<sdrkit_stack_gnss::components::GnssCoreComponent>("rx")
        .unwrap()
        .core;
    let d = core.last_report();
    for l in r.log_lines() {
        eprintln!("{l}");
    }
    eprintln!("{:?}", d.search);
    assert_eq!(d.state, 1, "{}", r.status().summary);
    let present: Vec<String> = ["GPS", "QZSS", "SBAS", "Galileo"]
        .iter()
        .flat_map(|s| {
            exp["present"][*s].as_array().unwrap().iter().map(move |p| {
                let prn = p.as_u64().unwrap() as u16;
                let sys = match *s {
                    "GPS" => System::Gps,
                    "QZSS" => System::Qzss,
                    "SBAS" => System::Sbas,
                    _ => System::Galileo,
                };
                SatId::new(sys, prn).name()
            })
        })
        .collect();
    let mut gps_locked = 0;
    let mut best = 0.0f64;
    for c in &d.channels {
        if c.state_level >= 1 {
            let name = c.name.as_str();
            assert!(
                present.contains(&name.to_string()),
                "{name} locked but not present"
            );
            if name.starts_with('G') {
                gps_locked += 1;
            }
            best = best.max(c.cn0);
        }
    }
    assert!(
        gps_locked >= 2,
        "{gps_locked} GPS locked: {}",
        r.status().summary
    );
    assert!((40.0..=46.0).contains(&best), "best {best}");
    assert!(d.fix.is_none());
}
