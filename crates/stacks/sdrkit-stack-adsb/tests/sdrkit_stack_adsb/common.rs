//! Helpers of the test program: a receiver with settings, running it over samples while collecting every accepted frame and
//! every report, a JSON view of the typed results, noise from `sdrkit-algo-sim`, and checking decoded frames off against the
//! transmissions.
#![allow(dead_code)]

pub use sdrkit_core::Owner;
use sdrkit_core::{
    Complex32, FieldDecl, InputInfo, LockState, Output, Receiver, Results, StackConfig, Station,
    Value, Values,
};
use sdrkit_graph::{Buffer, Data, Kind, StackGraph, Timed};
use sdrkit_stack_adsb::checks::{Accepted, Msg};
use sdrkit_stack_adsb::generator::Transmission;
use sdrkit_stack_adsb::message::Frame;
use sdrkit_util_geo::Position;
use serde_json::{json, Map, Value as J};

/// The settings of a stack at their defaults with these changes, each in the settings JSON form (effort fixed at all passes
/// for ADS-B, so the load of the computer cannot change results).
pub fn config(desc: &sdrkit_core::StackDescriptor, settings: &[(&str, J)]) -> StackConfig {
    let mut values = Values::defaults(&(desc.settings)());
    if desc.id == "adsb" {
        values.update_from_json(&json!({"effort": "2"})).unwrap();
    }
    for (k, v) in settings {
        let mut one = Map::new();
        one.insert((*k).to_owned(), v.clone());
        values
            .update_from_json(&J::Object(one))
            .unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    StackConfig::new(values)
}

/// An ADS-B receiver with these settings (the rest at their defaults).
pub fn receiver(settings: &[(&str, J)]) -> StackGraph {
    sdrkit_stack_adsb::receiver::receiver(&config(&sdrkit_stack_adsb::ADSB, settings))
        .expect("the ADS-B receiver builds")
}

/// Runs the command `key` of an owner with its default arguments.
pub fn command(o: &mut dyn Owner, key: &str) {
    let id = o.commands().id(key).expect("a declared command");
    let args = o.commands().defs()[id.index()].new_args();
    sdrkit_core::owner::invoke(o, id, &args).expect("the command runs");
}

/// Sets the setting `key` of an owner to `v`.
pub fn set(o: &mut dyn Owner, key: &str, v: Value) -> Result<(), sdrkit_core::SettingError> {
    let id = o.schema().id(key)?;
    sdrkit_core::owner::apply(o, id, &v)
}

/// Returns a receiver the station position.
pub fn at_station(rx: &mut dyn Receiver, lat: f64, lon: f64) {
    rx.set_station(&Station::at(Position::new(lat, lon)));
}

/// The test vector's station, the centre of the default airspace.
pub const CENTRE: (f64, f64) = (25.25, 55.36);

/// The results of a run.
pub struct Run {
    /// Every frame this receiver accepted.
    pub frames: Vec<Accepted>,
    /// Every event.
    pub output: Output,
    /// The view of each report ([`view`]) with `time` set to the signal time fed when it came.
    pub reports: Vec<J>,
}

/// Feeds `samples` in pieces given by `pattern` (cycled), collecting every accepted frame of this receiver.
pub fn run(rx: &mut StackGraph, rate: f64, samples: &[Complex32], pattern: &[usize]) -> Run {
    rx.configure(&InputInfo::new(rate, 1090e6, 1090e6).recording())
        .expect("configure");
    feed(rx, rate, samples, pattern)
}

/// Like [`run`] without configuring.
pub fn feed(rx: &mut StackGraph, rate: f64, samples: &[Complex32], pattern: &[usize]) -> Run {
    let mut tap = rx.graph_mut().tap("checks.out", 1 << 16).expect("tap");
    let mut buf = Buffer::new(&Kind::typed::<Msg>());
    let mut frames = Vec::new();
    let mut output = Output::default();
    let mut reports = Vec::new();
    let mut last_report = 0i64;
    let mut i = 0;
    let mut k = 0;
    while i < samples.len() {
        let n = pattern[k % pattern.len()].min(samples.len() - i).max(1);
        k += 1;
        let mut out = Output::default();
        rx.feed(&samples[i..i + n], &mut out);
        output.events.extend(out.events);
        i += n;
        while tap.pop(&mut buf) {
            if let Data::Items(Some(v)) = &buf.data {
                if let Some(v) = v.as_any().downcast_ref::<Vec<Timed<Msg>>>() {
                    for m in v {
                        if let Msg::Frame(a) = &m.value {
                            if a.source == 0 {
                                frames.push(a.clone());
                            }
                        }
                    }
                }
            }
        }
        let mut d = view(Owner::results(rx));
        let r = d["report"].as_i64().unwrap_or(0);
        if r != last_report {
            last_report = r;
            d["time"] = d.get("time_s").cloned().unwrap_or(json!(i as f64 / rate));
            reports.push(d);
        }
    }
    Run {
        frames,
        output,
        reports,
    }
}

/// Feeds a whole signal to any receiver in pieces of `block`, collecting the events.
pub fn feed_all(rx: &mut dyn Receiver, rate: f64, x: &[Complex32], block: usize) -> Output {
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    let mut all = Output::default();
    for c in x.chunks(block) {
        let mut out = Output::default();
        rx.feed(c, &mut out);
        all.events.extend(out.events);
    }
    all
}

/// A value as JSON: a choice becomes its key, a position its latitude and longitude, bytes a list of numbers. Returns `None`
/// for no value.
fn cell(d: &FieldDecl, v: &Value) -> Option<J> {
    Some(match v {
        Value::Text(t) => json!(t),
        Value::Choice(i) => json!(d.choices.get(usize::from(*i))?.key),
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => json!(x),
        Value::I64(x) => json!(x),
        Value::Bool(b) => json!(b),
        Value::Position { lat, lon } => json!([lat, lon]),
        Value::Bytes(b) => json!(b.as_ref()),
        _ => return None,
    })
}

/// The points in a trail cell of the JSON view.
pub fn trail_points(cell: &J) -> usize {
    let bytes: Vec<u8> = serde_json::from_value(cell.clone()).unwrap();
    let mut points = Vec::new();
    sdrkit_proto_aero::trail::decode(&bytes, &mut points).unwrap();
    points.len()
}

/// The rows of a table as objects, oldest written first. A position becomes `lat` and `lon`.
fn rows(r: &Results, key: &str) -> Vec<Map<String, J>> {
    let Some(t) = r.decl().table_id(key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    let mut out: Vec<Map<String, J>> = t
        .rows_newest_first()
        .map(|(id, rec)| {
            let mut m = Map::new();
            for (d, v) in rec.iter() {
                let c = d.key.as_ref();
                match (c, cell(d, v)) {
                    (_, None) => {}
                    ("position" | "corner", Some(J::Array(p))) => {
                        m.insert("lat".into(), p[0].clone());
                        m.insert("lon".into(), p[1].clone());
                    }
                    (_, Some(v)) => {
                        m.insert(c.to_owned(), v);
                    }
                }
            }
            m.insert("_id".into(), json!(id));
            m
        })
        .collect();
    out.reverse();
    out
}

/// The aircraft table with the names that the tests compare: `baro_alt_ft`, `messages`, `position_kind` or `cpr`, `distance_nm`,
/// the winds as (from, speed), newest first.
fn aircraft(r: &Results) -> Vec<J> {
    let mut v: Vec<J> = rows(r, "aircraft")
        .into_iter()
        .rev()
        .map(|mut m| {
            let mut rename = |from: &str, to: &str| {
                if let Some(x) = m.remove(from) {
                    m.insert(to.into(), x);
                }
            };
            rename("alt_ft", "baro_alt_ft");
            rename("heard", "messages");
            if let Some(f) = m.remove("fix") {
                match f.as_str() {
                    Some(c) if c.starts_with("cpr_") => {
                        m.insert("cpr".into(), json!(&c[4..]));
                    }
                    _ => {
                        m.insert("position_kind".into(), f);
                    }
                }
            }
            if let Some(d) = m.remove("distance_m").and_then(|d| d.as_f64()) {
                m.insert("distance_nm".into(), json!(d / 1852.0));
            }
            for w in ["derived", "reported"] {
                let from = m.remove(&format!("wind_{w}_from_deg"));
                let kt = m.remove(&format!("wind_{w}_kt"));
                if let (Some(a), Some(b)) = (from, kt) {
                    m.insert(format!("wind_{w}"), json!([a, b]));
                }
            }
            J::Object(m)
        })
        .collect();
    v.retain(|a| a.get("icao").is_some());
    v
}

/// A JSON view of a stack's typed results: every field under its key, the aircraft newest first with the names that the tests
/// compare, the other tables as lists, and the counts by short names (`good`, `failed`, `frames`, `replies`).
pub fn view(r: &Results) -> J {
    let mut d = Map::new();
    for (f, v) in r.fields().iter() {
        if let Some(v) = cell(f, v) {
            d.insert(f.key.to_string(), v);
        }
    }
    let alias = |d: &mut Map<String, J>, from: &str, to: &str| {
        if let Some(v) = d.get(from).cloned() {
            d.insert(to.into(), v);
        }
    };
    alias(&mut d, "frames_ok", "good");
    alias(&mut d, "frames_bad", "failed");
    alias(&mut d, "frames_ok", "frames");
    alias(&mut d, "message_rate", "messages_per_second");
    alias(&mut d, "modeac_replies", "replies");
    if let Some(m) = d.get("max_range_m").and_then(|x| x.as_f64()) {
        d.insert("max_range_nm".into(), json!(m / 1852.0));
    }
    d.insert(
        "state".into(),
        json!(match r.status().lock_state {
            LockState::Locked => 2,
            LockState::Tracking => 1,
            _ => 0,
        }),
    );
    d.insert("aircraft".into(), J::Array(aircraft(r)));
    for t in r.decl().tables() {
        let t = t.key.as_ref();
        if t != "aircraft" {
            d.insert(
                t.replace('.', "_"),
                J::Array(rows(r, t).into_iter().map(J::Object).collect()),
            );
        }
    }
    alias(&mut d, "fisb_products", "fisb");
    alias(&mut d, "modeac", "codes");
    let mut by_kind = Map::new();
    for k in [
        "mode_a",
        "mode_c",
        "mode_s",
        "all_call_a",
        "all_call_c",
        "ac_only_all_call",
    ] {
        if let Some(v) = d.get(k) {
            by_kind.insert(k.into(), v.clone());
        }
    }
    d.insert("by_kind".into(), J::Object(by_kind));
    J::Object(d)
}

/// Adds white noise so that a pulse of amplitude 0.1 is `snr_db` above the noise in 2 MHz (the generator's `signal_info`), from
/// `sdrkit-algo-sim`. Infinite `snr_db` adds none.
pub fn add_noise(x: &mut [Complex32], rate: f64, snr_db: f64, seed: u64) {
    if !snr_db.is_finite() {
        return;
    }
    let n0 = 0.01 / 10f64.powf(snr_db / 10.0) / 2e6;
    let mut awgn =
        sdrkit_algo_sim::noise::Awgn::with_power(n0 * rate, seed).expect("a noise power");
    awgn.process(x);
}

/// Like [`add_noise`], with the noise passed through the radio's filter `filter_hz` wide (a low-pass of `sdrkit-algo-dsp` at half the
/// width) when that filter is narrower than the sample rate allows, so the SNR is measured in the radio's band at any rate.
pub fn add_band_noise(x: &mut [Complex32], rate: f64, snr_db: f64, filter_hz: f64, seed: u64) {
    let cutoff = filter_hz / 2.0;
    if !snr_db.is_finite() || cutoff >= 0.42 * rate {
        add_noise(x, rate, snr_db, seed);
        return;
    }
    let n0 = 0.01 / 10f64.powf(snr_db / 10.0) / 2e6;
    let mut n = vec![Complex32::new(0.0, 0.0); x.len()];
    sdrkit_algo_sim::noise::Awgn::with_power(n0 * rate, seed)
        .expect("a noise power")
        .process(&mut n);
    let taps = sdrkit_algo_dsp::fir::lowpass(rate, 0.9 * cutoff, 1.1 * cutoff, 60.0).unwrap();
    sdrkit_algo_dsp::fir::Fir::<Complex32>::new(&taps).process_in_place(&mut n);
    for (a, b) in x.iter_mut().zip(&n) {
        *a += b;
    }
}

/// A frame matches a transmission: the same bytes (a DF11: the same first 32 bits) and a time within 3 µs.
pub fn same(f: &Frame, t: f64, tx: &Transmission) -> bool {
    if (t - tx.time).abs() > 3e-6 || f.bits != tx.frame.bits {
        return false;
    }
    if f.df() == 11 {
        f.bytes[..4] == tx.frame.bytes[..4]
    } else {
        f.data() == tx.frame.data()
    }
}

/// (transmissions decoded, phantoms).
pub fn check_off(frames: &[Accepted], txs: &[Transmission]) -> (Vec<bool>, usize) {
    let mut got = vec![false; txs.len()];
    let mut phantoms = 0;
    for a in frames {
        // Transmissions are in time order: look near the frame's time.
        let lo = txs.partition_point(|t| t.time < a.time - 4e-6);
        let mut hit = false;
        for (j, t) in txs.iter().enumerate().skip(lo) {
            if t.time > a.time + 4e-6 {
                break;
            }
            if same(&a.frame, a.time, t) {
                got[j] = true;
                hit = true;
            }
        }
        if !hit {
            eprintln!(
                "phantom: {} at {:.6} repaired {} level {:?}",
                a.frame.hex(),
                a.time,
                a.repaired,
                a.level_dbfs
            );
            phantoms += 1;
        }
    }
    (got, phantoms)
}

/// Reads transmissions.csv of the recorded vector.
pub fn vector_transmissions() -> Vec<(Transmission, bool)> {
    let p = sdrkit_dev_test::sigmf::vectors_dir()
        .join("adsb")
        .join("transmissions.csv");
    let text = std::fs::read_to_string(p).expect("transmissions.csv");
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let t = Transmission {
                time: f[0].parse().unwrap(),
                frame: Frame::from_hex(f[1]).unwrap(),
                amp: f[4].parse().unwrap(),
                freq: f[5].parse().unwrap(),
                overlapped: f[6] == "1",
            };
            (t, f[7] == "1")
        })
        .collect()
}
