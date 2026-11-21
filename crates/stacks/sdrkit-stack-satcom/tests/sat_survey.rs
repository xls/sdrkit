//! The satellite carrier survey: carriers at 1 Msps, channels at 10 Msps, frame gaps and a repeated sequence at 20 Msps, noise
//! alone for each method, pieces of any size, the clear command, the channel and dial data files, and the Doppler-curve match
//! on simulated passes among 1500 satellites. Captures at 10 and 20 Msps are streamed from the generator in chunks.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{
    owner, Complex32, Event, Generator, InputInfo, Output, Owner, Receiver, Results, Value,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satcom::modes::sat_survey::gen::{GenParams, SatSurveyGenerator};
use sdrkit_stack_satcom::modes::sat_survey::{
    match_params, params_of, CHANNELS, DESCRIPTOR, SUBMODES,
};
use sdrkit_stack_satcom::typed::{SatSurveyFields, SAT_SURVEY_CARRIERS, SAT_SURVEY_CHANNELS};
use sdrkit_util_geo::orbit::dopplermatch::match_track;
use sdrkit_util_geo::orbit::sgp4::{Elements, Gravity, OpsMode, Sgp4};
use sdrkit_util_geo::orbit::Satellite;
use sdrkit_util_geo::{Observer, Position};

const CHANNEL_FILE: &str = include_str!("../../../../data/science/sat_survey_channels.csv");
const DIAL_FILE: &str = include_str!("../../../../data/science/science_dials.csv");

/// The rows of `submode` in the channel file, as the `channels` setting takes them.
fn channel_rows(submode: &str) -> String {
    CHANNEL_FILE
        .lines()
        .filter(|l| l.starts_with(&format!("{submode},")))
        .map(|l| l.split_once(',').map_or("", |(_, rest)| rest))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a run left.
struct Out {
    rx: Box<dyn Receiver>,
    events: Vec<Event>,
}

impl Out {
    fn results(&self) -> &Results {
        Owner::results(self.rx.as_ref())
    }
    fn int(&self, key: &str) -> i64 {
        match self.field(key) {
            Some(Value::I64(n)) => n,
            _ => -1,
        }
    }
    fn field(&self, key: &str) -> Option<Value> {
        let r = self.results();
        r.decl()
            .field_id(key)
            .and_then(|id| r.fields().value(id))
            .cloned()
    }
    fn detected(&self) -> bool {
        self.field(SatSurveyFields::DETECTED.key) == Some(Value::Bool(true))
    }
    fn level(&self) -> f64 {
        match self.field(SatSurveyFields::FRAME_LEVEL_DB.key) {
            Some(Value::F64(x)) => x,
            _ => f64::NAN,
        }
    }
    fn rows(&self, key: &str) -> Vec<Vec<Value>> {
        let r = self.results();
        let Some(t) = r.decl().table_id(key).and_then(|id| r.table(id)) else {
            return Vec::new();
        };
        t.rows_newest_first()
            .map(|(_, rec)| {
                (0..rec.decl().len())
                    .map(|i| {
                        let id = sdrkit_core::FieldId(u16::try_from(i).unwrap_or(u16::MAX));
                        rec.value(id).cloned().unwrap_or(Value::None)
                    })
                    .collect()
            })
            .collect()
    }
    fn cell(&self, key: &str, row: usize, col: &str) -> Value {
        let r = self.results();
        let t = r
            .decl()
            .table_id(key)
            .and_then(|id| r.table(id))
            .expect("table");
        let (_, rec) = t.rows_newest_first().nth(row).expect("row");
        rec.decl()
            .id(col)
            .and_then(|id| rec.value(id).cloned())
            .unwrap_or(Value::None)
    }
}

/// Streams `seconds` of the test signal of `submode` at `rate`, centred on `centre`, through the receiver in pieces of
/// `piece` samples. `settings` are text settings by key.
fn run(
    submode: &str,
    rate: f64,
    centre: f64,
    seconds: f64,
    g: GenParams,
    piece: usize,
    settings: &[(
        &sdrkit_core::Setting<sdrkit_core::publish::field::Text>,
        &str,
    )],
) -> Out {
    let mut h = Harness::new(&DESCRIPTOR)
        .submode(submode)
        .rate(rate)
        .freq(centre);
    for (s, v) in settings {
        h = h.set(*s, v);
    }
    let mut rx = h.receiver().expect("receiver");
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .expect("configure");
    let mut gen = SatSurveyGenerator::new(rate, params_of(Some(submode)), g, 7);
    let total = (rate * seconds).round() as usize;
    let mut chunk = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut events = Vec::new();
    let mut fed = 0;
    while fed < total {
        let n = piece.min(total - fed);
        gen.generate(&mut chunk[..n]);
        rx.feed(&chunk[..n], &mut out);
        events.append(&mut out.events);
        out.clear();
        fed += n;
    }
    rx.finish(&mut out);
    events.append(&mut out.events);
    Out { rx, events }
}

fn params(submode: &str, level_db: f64) -> GenParams {
    GenParams {
        level_db,
        ..GenParams::of(params_of(Some(submode)).method)
    }
}

fn noise(submode: &str) -> GenParams {
    GenParams {
        signal: false,
        ..GenParams::of(params_of(Some(submode)).method)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Carrier method.

#[test]
fn a_tsikada_carrier_at_18_db_in_1_khz_gives_a_line() {
    let o = run(
        "tsikada",
        1e6,
        150e6,
        10.0,
        params("tsikada", 18.0),
        50_000,
        &[],
    );
    assert!(
        o.int(SatSurveyFields::LINES.key) >= 1,
        "{}",
        o.int(SatSurveyFields::LINES.key)
    );
    let off = match o.cell(SAT_SURVEY_CARRIERS.key, 0, "offset_hz") {
        Value::Hz(f) => f,
        v => panic!("{v:?}"),
    };
    // The carrier sits at 20 kHz + 3000 tanh((300 - t) / 120) Hz: about 22.96 kHz.
    assert!((off - 22_960.0).abs() < 20.0, "{off}");
}

#[test]
fn the_first_line_takes_the_name_of_the_150_mhz_channel() {
    let g = GenParams {
        carrier_hz: 1500.0,
        ..params("tsikada", 30.0)
    };
    let rows = channel_rows("tsikada");
    let o = run(
        "tsikada",
        1e6,
        150e6,
        10.0,
        g,
        50_000,
        &[(&CHANNELS, &rows)],
    );
    assert!(o.int(SatSurveyFields::LINES.key) >= 1);
    let lines = o.rows(SAT_SURVEY_CARRIERS.key);
    let first = lines.last().expect("a line");
    assert_eq!(first[0], Value::I64(1), "{lines:?}");
    assert_eq!(
        o.cell(SAT_SURVEY_CARRIERS.key, lines.len() - 1, "channel"),
        Value::Text("Tsikada 150 MHz".into())
    );
}

#[test]
fn noise_alone_gives_no_carrier_line() {
    let o = run("tsikada", 1e6, 150e6, 10.0, noise("tsikada"), 50_000, &[]);
    assert_eq!(o.int(SatSurveyFields::LINES.key), 0);
    assert!(o.rows(SAT_SURVEY_CARRIERS.key).is_empty());
    assert!(o.events.is_empty());
}

// ---------------------------------------------------------------------------------------------------------------------------
// Channel method.

#[test]
fn uhf_satcom_at_10_msps_shows_3_to_11_channels() {
    let o = run(
        "uhf_satcom",
        10e6,
        255e6,
        3.0,
        params("uhf_satcom", 30.0),
        1 << 20,
        &[],
    );
    let n = o.int(SatSurveyFields::CHANNELS_SEEN.key);
    assert!(
        (3..=11).contains(&n),
        "{n}: {:?}",
        o.rows(SAT_SURVEY_CHANNELS.key)
    );
}

#[test]
fn noise_alone_shows_fewer_than_5_channels() {
    let o = run(
        "uhf_satcom",
        10e6,
        255e6,
        2.0,
        noise("uhf_satcom"),
        1 << 20,
        &[],
    );
    assert!(
        o.int(SatSurveyFields::CHANNELS_SEEN.key) < 5,
        "{}",
        o.int(SatSurveyFields::CHANNELS_SEEN.key)
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Frame methods.

#[test]
fn starlink_frames_at_minus_5_and_0_db_are_detected() {
    for db in [-5.0, 0.0] {
        let o = run(
            "starlink",
            20e6,
            11_325e6,
            2.0,
            params("starlink", db),
            1 << 20,
            &[],
        );
        assert!(o.detected(), "{db} dB: level {:.2} dB", o.level());
        let Some(Value::F64(rate)) = o.field(SatSurveyFields::MEASURED_RATE_HZ.key) else {
            panic!("no rate");
        };
        assert!((rate - 750.0).abs() < 0.1, "{rate}");
    }
}

#[test]
fn oneweb_sequence_at_minus_5_0_and_5_db_is_detected() {
    for db in [-5.0, 0.0, 5.0] {
        let o = run(
            "oneweb",
            20e6,
            11_075e6,
            1.0,
            params("oneweb", db),
            1 << 20,
            &[],
        );
        assert!(o.detected(), "{db} dB: level {:.2} dB", o.level());
    }
}

#[test]
fn noise_alone_is_not_detected_by_the_frame_methods() {
    let o = run(
        "starlink",
        20e6,
        11_325e6,
        2.0,
        noise("starlink"),
        1 << 20,
        &[],
    );
    assert!(!o.detected(), "starlink level {:.2} dB", o.level());
    let o = run("oneweb", 20e6, 11_075e6, 1.0, noise("oneweb"), 1 << 20, &[]);
    assert!(!o.detected(), "oneweb level {:.2} dB", o.level());
}

// ---------------------------------------------------------------------------------------------------------------------------
// Pieces, clear and the data files.

#[test]
fn pieces_of_any_size_give_the_same_output() {
    let cases = [
        ("tsikada", 6.0),
        ("uhf_satcom", 2.0),
        ("starlink", 2.0),
        ("oneweb", 1.0),
    ];
    for (submode, s) in cases {
        let g = GenParams::of(params_of(Some(submode)).method);
        let a = run(submode, 1e6, 150e6, s, g, 65_536, &[]);
        let b = run(submode, 1e6, 150e6, s, g, 777, &[]);
        assert_eq!(a.results().fields(), b.results().fields(), "{submode}");
        for t in [SAT_SURVEY_CARRIERS.key, SAT_SURVEY_CHANNELS.key] {
            assert_eq!(a.rows(t), b.rows(t), "{submode} {t}");
        }
    }
}

#[test]
fn clear_empties_the_tables() {
    for submode in ["tsikada", "uhf_satcom"] {
        let g = GenParams::of(params_of(Some(submode)).method);
        let mut o = run(submode, 1e6, 150e6, 6.0, g, 65_536, &[]);
        let key = if submode == "tsikada" {
            SAT_SURVEY_CARRIERS.key
        } else {
            SAT_SURVEY_CHANNELS.key
        };
        assert!(!o.rows(key).is_empty(), "{submode}");
        let commands = o.rx.commands();
        let id = commands.id("clear").expect("clear");
        let args = commands.defs()[id.index()].new_args();
        owner::invoke(o.rx.as_mut(), id, &args).expect("clear runs");
        assert!(o.rows(key).is_empty(), "{submode}");
    }
}

#[test]
fn the_submode_dials_are_those_of_the_dial_file() {
    for s in &SUBMODES {
        let row = DIAL_FILE
            .lines()
            .find(|l| l.starts_with(&format!("sat_survey/{},", s.id)))
            .unwrap_or_else(|| panic!("no dial for {}", s.id));
        let hz: f64 = row.split(',').nth(1).unwrap().parse().unwrap();
        assert_eq!(s.default_hz, hz, "{}", s.id);
    }
}

#[test]
fn every_row_of_the_channel_file_names_a_submode() {
    let ids: Vec<&str> = SUBMODES.iter().map(|s| s.id).collect();
    let mut n = 0;
    for line in CHANNEL_FILE.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let sub = line.split(',').next().unwrap();
        assert!(ids.contains(&sub), "{line}");
        n += 1;
    }
    assert!(n >= 20);
    let t = sdrkit_stack_satcom::modes::sat_survey::names::ChannelTable::parse(
        &channel_rows("iss"),
        "iss",
    )
    .unwrap();
    assert_eq!(t.rows().len(), 4);
}

// ---------------------------------------------------------------------------------------------------------------------------
// Doppler-curve match.

/// 2026-01-01 00:00 UTC.
const EPOCH_UNIX: f64 = 1_767_225_600.0;
const CARRIER_HZ: f64 = 437e6;

fn station() -> Observer {
    Observer::new(Position::new(-33.87, 151.21))
}

fn elements(rng: &mut Rng) -> Elements {
    Elements {
        epoch_days_1950: 7306.0 + EPOCH_UNIX / 86_400.0,
        bstar: 0.0,
        eccentricity: rng.range(0.0005, 0.01),
        arg_perigee_rad: rng.range(0.0, 360.0).to_radians(),
        inclination_rad: rng.range(5.0, 100.0).to_radians(),
        mean_anomaly_rad: rng.range(0.0, 360.0).to_radians(),
        mean_motion_rad_min: rng.range(13.0, 15.8) * std::f64::consts::TAU / 1440.0,
        raan_rad: rng.range(0.0, 360.0).to_radians(),
    }
}

fn satellite(el: &Elements) -> Satellite {
    Satellite::new(Sgp4::new(el, Gravity::Wgs72, OpsMode::Improved).unwrap())
}

/// 1500 random satellites, and 12 of them with a pass over the station: (index, time of the highest point).
fn catalogue() -> (Vec<Elements>, Vec<Satellite>, Vec<(usize, f64)>) {
    let mut rng = Rng::new(42);
    let els: Vec<Elements> = (0..1500).map(|_| elements(&mut rng)).collect();
    let sats: Vec<Satellite> = els.iter().map(satellite).collect();
    let obs = station();
    let mut passes = Vec::new();
    for (i, s) in sats.iter().enumerate() {
        if passes.len() == 12 {
            break;
        }
        if let Ok(Some(p)) = s.next_pass(&obs, EPOCH_UNIX, EPOCH_UNIX + 86_400.0, 25.0) {
            passes.push((i, p.max_unix));
        }
    }
    assert_eq!(passes.len(), 12);
    (els, sats, passes)
}

/// The measured track of satellite `s` for 180 s around `t_max`, a point a second, with `offset` and Gaussian noise of
/// `sigma` Hz.
fn track(s: &Satellite, t_max: f64, offset: f64, sigma: f64, rng: &mut Rng) -> Vec<(f64, f64)> {
    let obs = station();
    (0..180)
        .map(|k| {
            let t = t_max - 90.0 + k as f64;
            (
                t,
                offset + s.doppler_hz(&obs, t, CARRIER_HZ).unwrap() + sigma * rng.normal(),
            )
        })
        .collect()
}

#[test]
fn twelve_passes_among_1500_satellites_are_named() {
    let (_, sats, passes) = catalogue();
    let obs = station();
    let p = match_params(180);
    let mut rng = Rng::new(9);
    for sigma in [5.0, 15.0, 30.0] {
        let mut confident = 0;
        for &(i, t) in &passes {
            let offset = rng.range(-9000.0, 9000.0);
            let pts = track(&sats[i], t, offset, sigma, &mut rng);
            let m = match_track(&pts, CARRIER_HZ, &obs, &sats, &p);
            let best = m
                .named()
                .unwrap_or_else(|| panic!("σ {sigma}: {i} not named: {m:?}"));
            assert_eq!(best.index, i, "σ {sigma}: {m:?}");
            assert!((best.offset_hz - offset).abs() < 40.0, "σ {sigma}: {m:?}");
            if m.confident() {
                confident += 1;
            }
        }
        if sigma == 15.0 {
            assert!(confident >= 11, "{confident} confident of 12");
        }
    }
}

#[test]
fn no_confident_wrong_answer_and_no_answer_for_a_fixed_line() {
    let (_, sats, passes) = catalogue();
    let obs = station();
    let p = match_params(180);
    let mut rng = Rng::new(11);
    for sigma in [5.0, 15.0, 30.0, 60.0, 120.0] {
        for &(i, t) in &passes {
            let pts = track(&sats[i], t, rng.range(-9000.0, 9000.0), sigma, &mut rng);
            let m = match_track(&pts, CARRIER_HZ, &obs, &sats, &p);
            if m.confident() {
                assert_eq!(m.named().unwrap().index, i, "σ {sigma}: {m:?}");
            }
        }
    }
    for &(_, t) in &passes {
        let fixed: Vec<(f64, f64)> = (0..180)
            .map(|k| (t - 90.0 + k as f64, 1234.0 + 15.0 * rng.normal()))
            .collect();
        let m = match_track(&fixed, CARRIER_HZ, &obs, &sats, &p);
        assert!(m.named().is_none(), "{m:?}");
    }
}

#[test]
fn no_answer_at_60_hz_noise() {
    let (_, sats, passes) = catalogue();
    let obs = station();
    let p = match_params(180);
    let mut rng = Rng::new(13);
    let mut named = Vec::new();
    for &(i, t) in &passes {
        let pts = track(&sats[i], t, rng.range(-9000.0, 9000.0), 60.0, &mut rng);
        let m = match_track(&pts, CARRIER_HZ, &obs, &sats, &p);
        if let Some(c) = m.named() {
            named.push((i, c.rms_hz));
        }
    }
    assert!(named.is_empty(), "named at 60 Hz: {named:?}");
}

#[test]
fn two_satellites_of_a_launch_train_are_ambiguous() {
    let (mut els, _, passes) = catalogue();
    let (i, t) = passes[0];
    let mut twin = els[i];
    twin.mean_anomaly_rad += 0.01f64.to_radians();
    els.push(twin);
    let sats: Vec<Satellite> = els.iter().map(satellite).collect();
    let mut rng = Rng::new(17);
    let pts = track(&sats[i], t, 2500.0, 15.0, &mut rng);
    let m = match_track(&pts, CARRIER_HZ, &station(), &sats, &match_params(180));
    assert!(m.ambiguous(), "{m:?}");
    assert!(!m.confident(), "{m:?}");
}

#[test]
fn an_empty_catalogue_or_no_track_gives_no_answer() {
    let (_, sats, passes) = catalogue();
    let (i, t) = passes[0];
    let mut rng = Rng::new(19);
    let pts = track(&sats[i], t, 0.0, 15.0, &mut rng);
    let p = match_params(180);
    assert!(match_track(&pts, CARRIER_HZ, &station(), &[], &p)
        .named()
        .is_none());
    assert!(match_track(&[], CARRIER_HZ, &station(), &sats, &p)
        .named()
        .is_none());
}
