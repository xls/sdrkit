//! Span-block scans and surveys with a fake engine: a fake stack whose result `signals` counts the stations in the span it
//! watches, and fake spectrum frames with keyed carriers. Plans, finds, locks, the quiet resume, the end of the total
//! time, rejected candidates skipped in later passes, and the rows of the hits, candidates and occupied tables.

use sdrkit_core::command::CommandError;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{Choice, Clock, MemStorage, Record, Value};
use sdrkit_svc_memory::MemoryFilter;
use sdrkit_svc_scan::*;
use sdrkit_util_text::morse::Keyer;
use std::sync::Arc;

const SIGNALS: Field<field::I64> = Field::new("signals", "Signals", "");
/// A result of another stack, and one that a scan cannot watch.
const LEVEL: Field<field::F64> = Field::new("level_dbfs", "Level", "dBFS");
const OPEN: Field<field::Bool> = Field::new("open", "Open", "");
const STEP_S: f64 = 0.05;
/// After a retune the results of the fake stack are empty for this long.
const CLEAR_S: f64 = 1.0;
/// The span that the fake stack watches.
const SPAN_HZ: f64 = 48e3;
/// The width of a voice channel of the fake spectrum.
const VOICE_HZ: f64 = 10e3;

/// The service, told of session `1`, which runs the fake stack `cw` and receives 250 kHz.
fn service() -> ScanService {
    let mut s = ScanService::open(
        Arc::new(MemStorage::new()),
        Clock::fixed(1_000_000.0),
        Arc::new(|_: &str| None),
        Arc::new(|_: &MemoryFilter| Vec::new()),
    )
    .unwrap();
    s.set_view("1", Some(view(250e3)));
    s
}

/// What session `1` receives at `span_hz` and shows: the results of the fake stack.
fn view(span_hz: f64) -> SessionView {
    SessionView {
        listened_hz: 7.0e6,
        span_hz,
        stack: "cw".into(),
        results: Arc::clone(Published::new(&[&SIGNALS]).results().decl()),
    }
}

fn call(
    s: &mut ScanService,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = s.commands().id(key)?;
    let mut given = s.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let fid = given.decl().id(k).unwrap();
        given.set_value(fid, v).unwrap();
    }
    owner::invoke(s, id, &given)
}

fn set(s: &mut ScanService, key: &str, v: Value) {
    let id = s.schema().id(key).unwrap();
    owner::apply(s, id, &v).unwrap();
}

fn text(v: &str) -> Value {
    Value::Text(v.into())
}

fn num(v: &Value) -> f64 {
    match v {
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => *x,
        Value::I64(x) => *x as f64,
        v => panic!("not a number: {v:?}"),
    }
}

fn cell<'a>(r: &'a Record, key: &str) -> &'a Value {
    r.value(r.decl().id(key).unwrap()).unwrap()
}

fn field<'a>(s: &'a ScanService, key: &str) -> Option<&'a Value> {
    s.results()
        .fields()
        .value(s.results_decl().field_id(key).unwrap())
}

/// The rows of table `key`, oldest first.
fn rows(s: &ScanService, key: &str) -> Vec<Record> {
    let id = s.results_decl().table_id(key).unwrap();
    let mut v: Vec<Record> = s
        .results()
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(_, r)| r.clone())
        .collect();
    v.reverse();
    v
}

/// A station: where it is, when it sends, whether the stack's own test passes on it, and whether it is keyed in the
/// spectrum.
#[derive(Clone, Copy, Debug)]
struct Station {
    hz: f64,
    from: f64,
    to: f64,
    stack_sees: bool,
}

/// The engine as the scan sees it: it steps the scan every 50 ms, tunes where it asks, gives the results of the fake
/// stack and, while the scan needs them, spectrum frames of the tuned block.
struct Fake {
    scan: ScanService,
    stations: Vec<Station>,
    /// The centres of voice channels: noise 10 dB over the noise of the block across [`VOICE_HZ`].
    voices: Vec<f64>,
    results: Published,
    empty: Published,
    t: f64,
    tuned_hz: f64,
    tuned_rate: Option<f64>,
    tuned_at: f64,
    /// The engine clears the results from this time on, as when the host moves the frequency.
    clear_from: f64,
    tunes: Vec<(f64, f64, Option<f64>)>,
    frame_t: f64,
    keys: Vec<(bool, f64)>,
    rng: u64,
    frame: Vec<f32>,
    frames: u64,
}

impl Fake {
    fn new(scan: ScanService, stations: Vec<Station>) -> Fake {
        let keyer = Keyer::new(20.0).unwrap();
        let mut keys: Vec<(bool, f64)> = keyer
            .key("CQ CQ DE K1ABC K1ABC TEST")
            .map(|k| (k.on, k.seconds))
            .collect();
        keys.push((false, 0.5));
        Fake {
            scan,
            stations,
            voices: Vec::new(),
            results: Published::new(&[&SIGNALS]),
            empty: Published::new(&[&SIGNALS]),
            t: 0.0,
            tuned_hz: 0.0,
            tuned_rate: None,
            tuned_at: 0.0,
            clear_from: f64::MAX,
            tunes: Vec::new(),
            frame_t: 0.0,
            keys,
            rng: 0x2545_F491_4F6C_DD1D,
            frame: Vec::new(),
            frames: 0,
        }
    }

    /// True if the Morse of the stations is key down at time `t` (the keying repeats).
    fn key_down(&self, t: f64) -> bool {
        let period: f64 = self.keys.iter().map(|k| k.1).sum();
        let mut x = t.rem_euclid(period);
        for &(on, s) in &self.keys {
            if x < s {
                return on;
            }
            x -= s;
        }
        false
    }

    fn uniform(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        ((self.rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// The frames of the tuned block up to now, while the scan needs them.
    fn feed_frames(&mut self) {
        let Some(need) = self.scan.spectrum_need() else {
            self.frame_t = self.t;
            return;
        };
        let frame_s = need.bins as f64 / need.rate_hz;
        if self.tuned_rate != Some(need.rate_hz) {
            self.frame_t = self.t;
            return;
        }
        let bin_hz = need.rate_hz / need.bins as f64;
        while self.frame_t + frame_s <= self.t {
            self.frame_t += frame_s;
            let mut frame = std::mem::take(&mut self.frame);
            frame.clear();
            for _ in 0..need.bins {
                let e = -self.uniform().ln();
                frame.push(-100.0 + 10.0 * e.log10() as f32);
            }
            let down = self.key_down(self.frame_t);
            for s in &self.stations {
                let b = need.bins as f64 / 2.0 + (s.hz - self.tuned_hz) / bin_hz;
                if down
                    && (s.from..s.to).contains(&self.frame_t)
                    && b >= 0.0
                    && b < need.bins as f64
                {
                    frame[b.round() as usize] = -80.0;
                }
            }
            for &hz in &self.voices {
                let b = need.bins as f64 / 2.0 + (hz - self.tuned_hz) / bin_hz;
                let half = VOICE_HZ / bin_hz / 2.0;
                let lo = (b - half).round().max(0.0) as usize;
                let hi = ((b + half).round().max(0.0) as usize).min(need.bins);
                for x in frame.iter_mut().take(hi).skip(lo) {
                    *x += 10.4;
                }
            }
            self.scan.frame(&SpectrumFrame {
                time_s: self.t,
                centre_hz: self.tuned_hz,
                span_hz: need.rate_hz,
                bins_db: &frame,
            });
            self.frames += 1;
            self.frame = frame;
        }
    }

    /// One step of the engine. Returns false when the scan is over.
    fn step(&mut self) -> bool {
        self.feed_frames();
        let seen = self
            .stations
            .iter()
            .filter(|s| {
                s.stack_sees
                    && (s.from..s.to).contains(&self.t)
                    && (s.hz - self.tuned_hz).abs() <= SPAN_HZ / 2.0
            })
            .count();
        self.results.set(&SIGNALS, seen as i64);
        let clear = self.t - self.tuned_at < CLEAR_S || self.t >= self.clear_from;
        let r = if clear { &self.empty } else { &self.results };
        let step = self.scan.step(self.t, Some(r.results()));
        let mut going = true;
        match step {
            Some(Step::Tune) => {
                let target = self.scan.target().unwrap();
                self.tuned_hz = target.freq_hz;
                self.tuned_rate = target.rate_hz;
                self.tuned_at = self.t;
                self.tunes.push((self.t, self.tuned_hz, self.tuned_rate));
            }
            Some(Step::Done) => going = false,
            None => {}
        }
        self.t += STEP_S;
        going
    }

    fn run_until(&mut self, t: f64) -> bool {
        while self.t < t {
            if !self.step() {
                return false;
            }
        }
        true
    }
}

fn span_args(dwell_s: f64, total_s: f64) -> Vec<(&'static str, Value)> {
    vec![
        ("session", text("1")),
        ("start_hz", Value::Hz(7.0e6)),
        ("stop_hz", Value::Hz(7.2e6)),
        ("span_hz", Value::Hz(SPAN_HZ)),
        ("watch", text("signals")),
        ("stack", text("cw")),
        ("dwell_s", Value::Duration(dwell_s)),
        ("total_s", Value::Duration(total_s)),
    ]
}

fn with(
    mut a: Vec<(&'static str, Value)>,
    more: &[(&'static str, Value)],
) -> Vec<(&'static str, Value)> {
    a.extend(more.iter().cloned());
    a
}

#[test]
fn a_watch_that_the_stack_does_not_have_is_refused_with_the_keys_it_has() {
    let mut s = service();
    for cmd in ["span", "survey"] {
        let args = with(
            span_args(5.0, 600.0),
            &[("watch", text("nothing")), ("dry", Value::Bool(true))],
        );
        let e = call(&mut s, cmd, &args).unwrap_err();
        assert!(
            e.to_string()
                .contains("the stack cw has no result nothing to watch; it has: signals"),
            "{cmd}: {e}"
        );
        let args = with(span_args(5.0, 600.0), &[("dry", Value::Bool(true))]);
        call(&mut s, cmd, &args).unwrap();
    }
    // A stack other than the one of the session is looked up; one the service cannot find is refused.
    let args = with(
        span_args(5.0, 600.0),
        &[("stack", text("fm")), ("dry", Value::Bool(true))],
    );
    let e = call(&mut s, "span", &args).unwrap_err();
    assert!(e.to_string().contains("there is no stack fm"), "{e}");
    s.set_stack_results(Arc::new(|id: &str| {
        (id == "fm").then(|| Arc::clone(Published::new(&[&LEVEL, &OPEN]).results().decl()))
    }));
    let e = call(&mut s, "span", &args).unwrap_err();
    assert!(e.to_string().contains("it has: level_dbfs"), "{e}");
    let args = with(
        span_args(5.0, 600.0),
        &[
            ("stack", text("fm")),
            ("watch", text("level_dbfs")),
            ("dry", Value::Bool(true)),
        ],
    );
    call(&mut s, "span", &args).unwrap();
}

#[test]
fn plans_say_what_a_scan_will_do_and_start_nothing() {
    let mut s = service();
    // Five blocks of 48 kHz over 200 kHz, 5 s each after a settle of 0.15 s: 25.75 s a pass.
    let r = call(
        &mut s,
        "span",
        &with(span_args(5.0, 600.0), &[("dry", Value::Bool(true))]),
    )
    .unwrap()
    .unwrap();
    assert_eq!(num(cell(&r, "channels")), 5.0);
    assert_eq!(cell(&r, "kind"), &Value::Choice(ScanKind::Span.index()));
    assert_eq!(num(cell(&r, "dwell_s")), 5.0);
    assert!((num(cell(&r, "pass_s")) - 25.75).abs() < 1e-9);
    assert!((num(cell(&r, "passes")) - 600.0 / 25.75).abs() < 1e-9);
    assert!(s.session().is_none(), "a plan starts nothing");
    // The same range as a survey: one pass of span blocks fits into the total time, so it runs span blocks.
    let narrow = with(
        span_args(5.0, 600.0),
        &[("rate_hz", Value::Hz(2.4e6)), ("dry", Value::Bool(true))],
    );
    let r = call(&mut s, "survey", &narrow).unwrap().unwrap();
    assert_eq!(cell(&r, "kind"), &Value::Choice(ScanKind::Span.index()));
    // 70 to 450 MHz in an hour: 7917 span blocks of 10.15 s take 22 hours, so stage one looks at 43 blocks of 9 MHz.
    let wide = vec![
        ("session", text("1")),
        ("start_hz", Value::Hz(70e6)),
        ("stop_hz", Value::Hz(450e6)),
        ("span_hz", Value::Hz(SPAN_HZ)),
        ("watch", text("signals")),
        ("total_s", Value::Duration(3600.0)),
        ("rate_hz", Value::Hz(10e6)),
        ("dry", Value::Bool(true)),
    ];
    let r = call(&mut s, "survey", &wide).unwrap().unwrap();
    assert_eq!(cell(&r, "kind"), &Value::Choice(ScanKind::Survey.index()));
    assert_eq!(num(cell(&r, "channels")), 43.0);
    assert!((num(cell(&r, "pass_s")) - 43.0 * 2.15).abs() < 1e-9);
    assert!((num(cell(&r, "passes")) - 3600.0 / (43.0 * 2.15)).abs() < 1e-9);
    assert!((num(cell(&r, "visit_s")) - 10.15).abs() < 1e-9);
    assert!((num(cell(&r, "span_pass_s")) - 7917.0 * 10.15).abs() < 1e-6);
    // A survey without a rate takes the width that the session receives: stage one looks at blocks of 250 kHz.
    let no_rate = |session: &str| {
        vec![
            ("session", text(session)),
            ("start_hz", Value::Hz(70e6)),
            ("stop_hz", Value::Hz(450e6)),
            ("span_hz", Value::Hz(SPAN_HZ)),
            ("watch", text("signals")),
            ("dry", Value::Bool(true)),
        ]
    };
    let r = call(&mut s, "survey", &no_rate("1")).unwrap().unwrap();
    assert_eq!(cell(&r, "kind"), &Value::Choice(ScanKind::Survey.index()));
    assert_eq!(
        num(cell(&r, "channels")),
        blocks(70e6, 450e6, 250e3 * USED_SHARE).len() as f64
    );
    // A session whose input has no known rate, and a session that the service was not told of, are refused.
    s.set_view("1", Some(view(0.0)));
    let e = call(&mut s, "survey", &no_rate("1")).unwrap_err();
    assert!(e.to_string().contains("no sample rate"), "{e}");
    let e = call(&mut s, "survey", &no_rate("2")).unwrap_err();
    assert!(e.to_string().contains("no session 2"), "{e}");
    s.set_view("1", Some(view(250e3)));
    // The plan of an activity scan of a band: 3 channels with the settle and dwell settings.
    let r = call(
        &mut s,
        "band",
        &[
            ("session", text("1")),
            ("start_hz", Value::Hz(144e6)),
            ("stop_hz", Value::Hz(144.05e6)),
            ("step_hz", Value::Hz(25e3)),
            ("dry", Value::Bool(true)),
        ],
    )
    .unwrap()
    .unwrap();
    assert_eq!(cell(&r, "kind"), &Value::Choice(ScanKind::Activity.index()));
    assert!((num(cell(&r, "pass_s")) - 3.0 * 0.65).abs() < 1e-9);
    assert!(s.session().is_none());
}

#[test]
fn the_look_sets_the_plan_of_a_survey_and_a_shorter_dwell_is_refused() {
    let mut s = service();
    // 7.0 to 7.2 MHz at 250 kHz until stopped: one block of stage one, and span blocks take longer, so it is a survey.
    let args = vec![
        ("session", text("1")),
        ("start_hz", Value::Hz(7.0e6)),
        ("stop_hz", Value::Hz(7.2e6)),
        ("span_hz", Value::Hz(SPAN_HZ)),
        ("watch", text("signals")),
        ("dwell_s", Value::Duration(10.0)),
        ("total_s", Value::Duration(0.0)),
        ("rate_hz", Value::Hz(250e3)),
        ("dry", Value::Bool(true)),
    ];
    let plan = |s: &mut ScanService| {
        let r = call(s, "survey", &args).unwrap().unwrap();
        assert_eq!(cell(&r, "kind"), &Value::Choice(ScanKind::Survey.index()));
        let get = |k: &str| num(cell(&r, k));
        (get("channels"), get("dwell_s"), get("pass_s"))
    };
    let (blocks, look, pass) = plan(&mut s);
    assert_eq!(blocks, 1.0);
    assert_eq!(look, 2.0);
    assert!((pass - 2.15).abs() < 1e-9, "{pass}");
    set(&mut s, "look", Value::Duration(4.0));
    let (blocks, look, pass) = plan(&mut s);
    assert_eq!(blocks, 1.0);
    assert_eq!(look, 4.0);
    assert!((pass - 4.15).abs() < 1e-9, "{pass}");
    // A dwell on a candidate under the look is refused, by the plan too, and nothing starts.
    let short = with(args.clone(), &[("dwell_s", Value::Duration(3.0))]);
    let e = call(&mut s, "survey", &short).unwrap_err();
    assert!(e.to_string().contains("shorter than the look"), "{e}");
    let short = with(short, &[("dry", Value::Bool(false))]);
    assert!(call(&mut s, "survey", &short).is_err());
    assert!(s.session().is_none());
    // The setting has its common values.
    let id = s.schema().id("look").unwrap();
    assert_eq!(s.schema().common(id, None), &[2.0, 3.0, 4.0, 6.0]);
}

#[test]
fn a_span_scan_locks_on_a_find_and_resumes_after_quiet() {
    let mut s = service();
    set(&mut s, "quiet", Value::Duration(30.0));
    call(&mut s, "span", &span_args(5.0, 0.0)).unwrap();
    // A station in the third block (7.076 to 7.124 MHz) that sends for 40 s.
    let mut f = Fake::new(
        s,
        vec![Station {
            hz: 7.11e6,
            from: 0.0,
            to: 40.0,
            stack_sees: true,
        }],
    );
    assert!(f.run_until(14.0));
    let blocks: Vec<f64> = f.tunes.iter().map(|t| t.1).collect();
    assert_eq!(blocks, [7.024e6, 7.062e6, 7.1e6]);
    assert_eq!(
        field(&f.scan, "state"),
        Some(&Value::Choice(ScanState::Locked.index()))
    );
    assert!(f.scan.audio_wanted());
    let hits = rows(&f.scan, "hits");
    assert_eq!(hits.len(), 1);
    assert_eq!(num(cell(&hits[0], "freq_hz")), 7.1e6);
    assert_eq!(num(cell(&hits[0], "pass")), 0.0);
    // The host centres on the signal: the results are empty for 2 s. The scan stays locked.
    f.clear_from = 20.0;
    assert!(f.run_until(22.0));
    f.clear_from = f64::MAX;
    assert!(f.run_until(40.0));
    assert_eq!(f.tunes.len(), 3, "no early resume");
    // The station ends at 40 s; 30 s of quiet later the scan goes on.
    assert!(f.run_until(69.8));
    assert_eq!(f.tunes.len(), 3);
    assert!(f.run_until(70.5));
    assert_eq!(f.tunes.len(), 4);
    assert_eq!(f.tunes[3].1, 7.138e6);
    assert!(f.tunes[3].0 >= 70.0 - 1e-9);
    // The find ends where the lock ended, and the host attaches the decoded text.
    let hits = rows(&f.scan, "hits");
    assert!((num(cell(&hits[0], "end_time")) - (1_000_000.0 + 70.0)).abs() < 0.1);
    call(&mut f.scan, "hit.text", &[("text", text("CQ DE K1ABC"))]).unwrap();
    let hits = rows(&f.scan, "hits");
    assert_eq!(cell(&hits[0], "text"), &text("CQ DE K1ABC"));
    let e = call(
        &mut f.scan,
        "hit.text",
        &[("text", text("x")), ("hit", text("7"))],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::NoSuchRow { .. }), "{e:?}");
}

#[test]
fn a_find_that_stays_holds_until_resumed() {
    let mut s = service();
    set(&mut s, "after_find", Value::Choice(AfterFind::Stay.index()));
    call(&mut s, "span", &span_args(5.0, 0.0)).unwrap();
    let mut f = Fake::new(
        s,
        vec![Station {
            hz: 7.03e6,
            from: 0.0,
            to: 5.0,
            stack_sees: true,
        }],
    );
    assert!(f.run_until(300.0));
    assert_eq!(f.tunes.len(), 1);
    assert_eq!(
        field(&f.scan, "state"),
        Some(&Value::Choice(ScanState::Locked.index()))
    );
    call(
        &mut f.scan,
        "control",
        &[("action", Value::Choice(Action::Resume.index()))],
    )
    .unwrap();
    assert!(f.run_until(300.2));
    assert_eq!(f.tunes.len(), 2);
    assert_eq!(f.tunes[1].1, 7.062e6);
}

#[test]
fn a_span_scan_ends_when_its_total_time_is_over() {
    let mut s = service();
    call(&mut s, "span", &span_args(5.0, 60.0)).unwrap();
    let mut f = Fake::new(s, Vec::new());
    assert!(f.run_until(59.9));
    assert!(f.scan.session().is_some());
    let left = num(field(&f.scan, "time_left_s").unwrap());
    assert!(left > 0.0 && left < 0.2, "{left}");
    assert!(!f.run_until(70.0), "the scan ends at 60 s");
    assert_eq!(
        field(&f.scan, "end"),
        Some(&Value::Choice(EndReason::TimeOver.index()))
    );
    // 60 s of blocks of 5.15 s: 11 retunes after the first, the twelfth block not finished; the second pass began.
    assert_eq!(f.tunes.len(), 12);
    assert_eq!(num(field(&f.scan, "pass").unwrap()), 2.0);
    assert!((num(field(&f.scan, "passes").unwrap()) - 60.0 / 25.75).abs() < 1e-9);
}

fn survey_args() -> Vec<(&'static str, Value)> {
    vec![
        ("session", text("1")),
        ("start_hz", Value::Hz(144.0e6)),
        ("stop_hz", Value::Hz(145.0e6)),
        ("span_hz", Value::Hz(SPAN_HZ)),
        ("watch", text("signals")),
        ("stack", text("cw")),
        ("dwell_s", Value::Duration(8.0)),
        ("total_s", Value::Duration(70.0)),
        ("rate_hz", Value::Hz(250e3)),
    ]
}

#[test]
fn a_survey_visits_candidates_finds_one_and_skips_the_rejected_one_later() {
    let mut s = service();
    set(&mut s, "quiet", Value::Duration(3.0));
    let r = call(&mut s, "survey", &survey_args()).unwrap().unwrap();
    assert_eq!(cell(&r, "kind"), &Value::Choice(ScanKind::Survey.index()));
    // Stage one: 5 blocks of 225 kHz. A is keyed and the stack finds it while it sends (25 s); B is keyed all the time,
    // and the stack finds nothing on it.
    let a = Station {
        hz: 144.30e6,
        from: 0.0,
        to: 25.0,
        stack_sees: true,
    };
    let b = Station {
        hz: 144.62e6,
        from: 0.0,
        to: 1e9,
        stack_sees: false,
    };
    let mut f = Fake::new(s, vec![a, b]);
    let need = f.scan.spectrum_need().unwrap();
    assert_eq!(need.bins, 4096);
    assert!(!f.run_until(200.0), "the survey ends in its total time");
    assert_eq!(
        field(&f.scan, "end"),
        Some(&Value::Choice(EndReason::TimeOver.index()))
    );
    assert!(f.frames > 0);
    // Stage-one blocks are tuned with the wide rate, visits with the rate of the stack.
    let wide: Vec<f64> = f
        .tunes
        .iter()
        .filter(|t| t.2 == Some(250e3))
        .map(|t| t.1)
        .collect();
    let visits: Vec<f64> = f
        .tunes
        .iter()
        .filter(|t| t.2.is_none())
        .map(|t| t.1)
        .collect();
    assert_eq!(&wide[..5], &crate_blocks());
    let near = |v: &f64, hz: f64| (v - hz).abs() < 200.0;
    assert_eq!(
        visits.iter().filter(|v| near(v, a.hz)).count(),
        1,
        "{visits:?}"
    );
    assert_eq!(
        visits.iter().filter(|v| near(v, b.hz)).count(),
        1,
        "B is skipped in the second pass: {visits:?}"
    );
    assert!(
        num(field(&f.scan, "pass").unwrap()) >= 1.0,
        "a second pass ran"
    );
    let hits = rows(&f.scan, "hits");
    assert_eq!(hits.len(), 1);
    assert!(near(&num(cell(&hits[0], "freq_hz")), a.hz));
    let cands = rows(&f.scan, "candidates");
    let state = |hz: f64| -> Vec<Value> {
        cands
            .iter()
            .filter(|r| near(&num(cell(r, "freq_hz")), hz))
            .map(|r| cell(r, "state").clone())
            .collect()
    };
    assert_eq!(state(a.hz), [Value::Choice(CandidateState::Found.index())]);
    assert_eq!(
        state(b.hz),
        [Value::Choice(CandidateState::Rejected.index())]
    );
    let b_row = cands
        .iter()
        .find(|r| near(&num(cell(r, "freq_hz")), b.hz))
        .unwrap();
    assert!(num(cell(b_row, "skip_until")) > 1_000_000.0 + 1800.0);
    let wpm = num(cell(b_row, "wpm"));
    assert!((wpm - 20.0).abs() < 3.0, "{wpm}");
}

#[test]
fn a_rejected_candidate_is_visited_again_after_its_hold() {
    let mut s = service();
    set(&mut s, "reject_hold", Value::Duration(5.0));
    call(&mut s, "survey", &survey_args()).unwrap();
    let b = Station {
        hz: 144.62e6,
        from: 0.0,
        to: 1e9,
        stack_sees: false,
    };
    let mut f = Fake::new(s, vec![b]);
    assert!(!f.run_until(200.0));
    let visits = f
        .tunes
        .iter()
        .filter(|t| t.2.is_none() && (t.1 - b.hz).abs() < 200.0)
        .count();
    assert!(
        visits >= 2,
        "a hold of 5 s ends within a pass: {visits} visits"
    );
    let rejected = rows(&f.scan, "candidates")
        .iter()
        .filter(|r| cell(r, "state") == &Value::Choice(CandidateState::Rejected.index()))
        .count();
    assert_eq!(rejected, visits);
}

#[test]
fn a_survey_lists_a_voice_channel_as_occupied_and_a_new_scan_clears_the_table() {
    let mut s = service();
    call(&mut s, "survey", &survey_args()).unwrap();
    let mut f = Fake::new(s, Vec::new());
    let voice = 144.40e6;
    f.voices.push(voice);
    assert!(f.run_until(15.0));
    let first = rows(&f.scan, "occupied");
    assert_eq!(first.len(), 1, "{first:?}");
    assert!((num(cell(&first[0], "freq_hz")) - voice).abs() < 500.0);
    assert!((num(cell(&first[0], "width_hz")) / VOICE_HZ - 1.0).abs() < 0.15);
    assert!((num(cell(&first[0], "snr_db")) - 10.0).abs() < 1.5);
    assert_eq!(num(cell(&first[0], "seen")), 1.0);
    assert!(!f.run_until(200.0), "the survey ends in its total time");
    // The channel keeps its one row over the passes, and is not visited.
    let last = rows(&f.scan, "occupied");
    assert_eq!(last.len(), 1, "{last:?}");
    let passes = num(field(&f.scan, "pass").unwrap());
    assert!(passes >= 2.0, "{passes}");
    let seen = num(cell(&last[0], "seen"));
    assert!(seen >= passes && seen <= passes + 1.0, "{seen} of {passes}");
    assert_eq!(num(cell(&last[0], "pass")), seen - 1.0);
    assert!(f.tunes.iter().all(|t| t.2.is_some()), "{:?}", f.tunes);
    assert!(rows(&f.scan, "candidates").is_empty());
    call(&mut f.scan, "survey", &survey_args()).unwrap();
    assert!(rows(&f.scan, "occupied").is_empty());
}

/// The stage-one blocks of the survey of 144 to 145 MHz at 250 kHz: 225 kHz each, spread edge to edge.
fn crate_blocks() -> Vec<f64> {
    blocks(144.0e6, 145.0e6, 250e3 * USED_SHARE)
}

#[test]
fn the_new_tables_commands_and_settings_are_declared() {
    let s = service();
    for t in ["candidates", "occupied"] {
        assert!(s.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in ["span", "survey", "hit.text"] {
        assert!(s.commands().id(c).is_ok(), "{c}");
    }
    for k in ["after_find", "quiet", "reject_hold"] {
        assert!(s.schema().id(k).is_ok(), "{k}");
    }
    for k in ["time_left_s", "pass_s", "passes", "candidates", "watched"] {
        assert!(s.results_decl().field_id(k).is_some(), "{k}");
    }
}
