//! The scanners against a fake engine: stations that are active at set times, a lock state that locks some time after a retune,
//! and results with names. Time is simulated, so the tests are exact and fast.

use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{
    HostRequest, LockCondition, LockProfile, LockState, NameSource, OwnerKey, OwnerSetting,
    ResultsDecl, Status, Value,
};
use sdrkit_svc_memory::{Memory, MemoryFilter, MemoryStore};
use sdrkit_svc_scan::*;
use std::sync::Arc;

const TONE: Field<field::Text> = Field::new("tone", "Tone", "");
const ENSEMBLE: Field<field::Text> = Field::new("ensemble_label", "Ensemble", "");
const SNR_DB: Field<field::Text> = Field::new("snr_db", "SNR", "");
const STATION: Field<field::Text> = Field::new("station", "Station", "");
const PI: Field<field::Text> = Field::new("pi", "PI", "");

/// The profiles of DAB, the TV stacks and FM, as the stacks declare them.
const DAB: LockProfile = LockProfile {
    settle_s: 0.2,
    quiet_after_s: Some(1.5),
    signal_snr_db: 4.0,
    timeout_s: 5.0,
    lock: LockCondition::LockState {
        at_least: LockState::Locked,
    },
    collect_s: 3.0,
    name_from: &[NameSource::Field("ensemble_label")],
    capture: &["snr_db"],
};
const TV: LockProfile = LockProfile {
    settle_s: 0.3,
    quiet_after_s: Some(2.0),
    signal_snr_db: 3.0,
    timeout_s: 8.0,
    lock: LockCondition::LockState {
        at_least: LockState::Locked,
    },
    collect_s: 4.0,
    name_from: &[NameSource::Summary],
    capture: &[],
};
const FM: LockProfile = LockProfile {
    settle_s: 0.1,
    quiet_after_s: Some(0.4),
    signal_snr_db: 10.0,
    timeout_s: 1.0,
    lock: LockCondition::All(&[
        LockCondition::LockState {
            at_least: LockState::Tracking,
        },
        LockCondition::Snr { min_db: 12.0 },
    ]),
    collect_s: 4.0,
    name_from: &[NameSource::Field("station")],
    capture: &["pi", "programme", "text"],
};

#[derive(Clone, Debug, Default)]
struct Station {
    freq_hz: f64,
    /// The times it transmits.
    active: Vec<(f64, f64)>,
    /// The stack locks this long after a retune. `None` never.
    lock_after_s: Option<f64>,
    /// The SNR while on air or locked.
    snr_db: f32,
    /// The fields published once locked.
    fields: Vec<(&'static Field<field::Text>, &'static str)>,
    /// The name appears this long after the lock.
    name_after_s: f64,
    tone: &'static str,
}

/// A channel as the fake engine tuned it.
#[derive(Clone, Debug, PartialEq)]
struct Tuned {
    freq_hz: f64,
    stack: String,
    submode: String,
    settings: Vec<OwnerSetting>,
}

impl Tuned {
    fn of(t: Target<'_>) -> Tuned {
        Tuned {
            freq_hz: t.freq_hz,
            stack: t.stack.into(),
            submode: t.submode.into(),
            settings: t.settings.to_vec(),
        }
    }
}

struct FakeEngine {
    now: f64,
    stations: Vec<Station>,
    home: Option<Tuned>,
    tuned: Option<Tuned>,
    tuned_at: f64,
    retunes: Vec<(f64, Tuned)>,
    audio: bool,
    audio_on_time: f64,
    decl: Arc<ResultsDecl>,
    results: Published,
    observer: Observer,
}

impl FakeEngine {
    fn new(stations: Vec<Station>) -> FakeEngine {
        let decl =
            Arc::new(ResultsDecl::new().with_fields(&[&TONE, &ENSEMBLE, &SNR_DB, &STATION, &PI]));
        FakeEngine {
            now: 0.0,
            stations,
            home: None,
            tuned: None,
            tuned_at: 0.0,
            retunes: Vec::new(),
            audio: false,
            audio_on_time: 0.0,
            results: Published::from_decl(Arc::clone(&decl)),
            decl,
            observer: Observer::default(),
        }
    }

    fn station(&self) -> Option<&Station> {
        let f = self.tuned.as_ref()?.freq_hz;
        self.stations.iter().find(|s| (s.freq_hz - f).abs() < 1.0)
    }

    fn busy(&self) -> bool {
        self.station()
            .is_some_and(|s| s.active.iter().any(|&(a, b)| self.now >= a && self.now < b))
    }

    fn tuned_freqs(&self) -> Vec<f64> {
        self.retunes.iter().map(|(_, t)| t.freq_hz).collect()
    }

    fn retune(&mut self, t: Tuned) {
        self.tuned = Some(t.clone());
        self.tuned_at = self.now;
        self.retunes.push((self.now, t));
        self.results = Published::from_decl(Arc::clone(&self.decl));
    }

    fn advance(&mut self, seconds: f64) -> f64 {
        if self.audio {
            self.audio_on_time += seconds;
        }
        self.now += seconds;
        self.now
    }

    fn lock_state(&self) -> LockState {
        match self.station() {
            Some(s)
                if s.lock_after_s
                    .is_some_and(|l| self.now - self.tuned_at >= l) =>
            {
                LockState::Locked
            }
            Some(s) if s.lock_after_s.is_some() => LockState::Tracking,
            _ => LockState::Searching,
        }
    }

    /// Publishes what the tuned stack shows now and reads the observation from it.
    fn measure(&mut self, obs: &mut Observation) {
        let on = self.busy();
        let since = self.now - self.tuned_at;
        let st = self.station().cloned();
        let locked = st
            .as_ref()
            .and_then(|s| s.lock_after_s)
            .is_some_and(|l| since >= l);
        let snr = st
            .as_ref()
            .filter(|_| on || locked)
            .map(|s| s.snr_db)
            .unwrap_or(0.0);
        let lock_state = self.lock_state();
        let p = &mut self.results;
        p.results_mut().set_status(
            &Status::new(lock_state)
                .with_snr_db(snr)
                .with_level_dbfs(-80.0 + snr),
        );
        if let Some(s) = &st {
            if on {
                p.set(&TONE, s.tone);
            }
            if locked && since >= s.lock_after_s.unwrap_or(0.0) + s.name_after_s {
                for (f, v) in &s.fields {
                    p.set(f, v);
                }
            }
        }
        self.observer.read(p.results(), obs);
    }
}

const TICK: f64 = 0.05;

/// Runs a scanner until it ends or until `until_s`, calling `act` before every tick so that a test can press buttons. On
/// the end the engine tunes back home. Returns true if the scan ended.
fn drive<M: ScanMachine>(
    s: &mut M,
    e: &mut FakeEngine,
    until_s: f64,
    mut act: impl FnMut(f64, &mut M) -> Option<Step>,
) -> bool {
    let mut step = Some(s.start(e.now));
    let mut obs = Observation::default();
    loop {
        match step.take() {
            Some(Step::Tune) => e.retune(Tuned::of(s.target().unwrap())),
            Some(Step::Done) => {
                if let Some(h) = e.home.clone() {
                    e.retune(h);
                }
                return true;
            }
            None => {}
        }
        e.audio = s.audio_wanted();
        let now = e.advance(TICK);
        if now > until_s {
            return false;
        }
        if let Some(c) = act(now, s) {
            step = Some(c);
            continue;
        }
        e.measure(&mut obs);
        step = s.tick(now, &obs, Some(e.results.results()));
    }
}

fn home(mhz: f64, stack: &str) -> Tuned {
    Tuned {
        freq_hz: mhz * 1e6,
        stack: stack.into(),
        submode: String::new(),
        settings: Vec::new(),
    }
}

fn setting(stack: &'static str, key: &str, v: f64) -> OwnerSetting {
    OwnerSetting {
        owner: OwnerKey::of(stack),
        key: key.into(),
        value: Value::Hz(v),
    }
}

fn entry(mhz: f64, name: &str) -> ScanEntry {
    ScanEntry::new(mhz * 1e6, "general", name)
}

fn list(entries: Vec<ScanEntry>) -> Channels {
    Channels::List(entries)
}

fn station(mhz: f64, active: &[(f64, f64)]) -> Station {
    Station {
        freq_hz: mhz * 1e6,
        active: active.to_vec(),
        snr_db: 20.0,
        ..Station::default()
    }
}

#[test]
fn stops_on_activity_holds_waits_the_hang_and_moves_on() {
    let mut e = FakeEngine::new(vec![station(145.6, &[(3.0, 6.0)])]);
    let mut s = ActivityScanner::new(
        list(vec![
            entry(145.5, "A"),
            entry(145.6, "B"),
            entry(145.7, "C"),
        ]),
        ActivityConfig {
            hang_s: 2.0,
            ..ActivityConfig::default()
        },
    );
    assert!(!drive(&mut s, &mut e, 12.0, |_, _| None));
    let hits: Vec<&Hit> = s.hits().hits().collect();
    assert_eq!(hits.len(), 1, "{hits:?}");
    let h = hits[0];
    assert_eq!(h.name, "B");
    assert!((h.start_s - 3.0).abs() < 0.7, "{h:?}");
    assert!((h.end_s - 6.0).abs() < 0.11, "{h:?}");
    // It left B about the hang time after the activity ended.
    let left_b = e
        .retunes
        .iter()
        .find(|(t, tune)| *t > 6.0 && tune.freq_hz != 145.6e6)
        .unwrap()
        .0;
    assert!((left_b - 8.0).abs() < 0.11, "left at {left_b}");
    // Sound played only while held (about 3 s on air plus 2 s hang).
    assert!((e.audio_on_time - 5.0).abs() < 0.8, "{}", e.audio_on_time);
    // The entry counts its hit.
    assert_eq!(s.channels().entry(1).unwrap().hits, 1);
}

#[test]
fn locked_out_entries_and_band_steps_are_passed_by() {
    let mut e = FakeEngine::new(vec![]);
    let mut b = entry(145.6, "B");
    b.lockout = true;
    let mut s = ActivityScanner::new(
        list(vec![entry(145.5, "A"), b, entry(145.7, "C")]),
        ActivityConfig::default(),
    );
    drive(&mut s, &mut e, 5.0, |_, _| None);
    assert!(!e.tuned_freqs().contains(&145.6e6));

    // A band step locked out by the user is not visited on later passes.
    let mut e = FakeEngine::new(vec![station(144.025, &[(0.0, 100.0)])]);
    let band = BandSearch::new(144.0e6, 144.05e6, 25e3, "general").unwrap();
    let mut s = ActivityScanner::new(
        Channels::Band(band),
        ActivityConfig {
            hang_s: 0.5,
            ..ActivityConfig::default()
        },
    );
    let mut locked = false;
    drive(&mut s, &mut e, 10.0, |now, s| {
        if !locked && s.holding() && now > 1.0 {
            locked = true;
            return s.lockout_current(now);
        }
        None
    });
    assert!(locked);
    let after: Vec<f64> = e
        .retunes
        .iter()
        .skip_while(|(t, _)| *t < 1.5)
        .map(|(_, t)| t.freq_hz)
        .collect();
    assert!(
        !after.is_empty() && !after.contains(&144.025e6),
        "{after:?}"
    );
    assert_eq!(s.lockouts().collect::<Vec<_>>(), [144.025e6]);
    assert_eq!(s.hits().len(), 1);
}

#[test]
fn every_channel_locked_out_ends_the_scan() {
    let mut e = FakeEngine::new(vec![]);
    let mut a = entry(1.0, "A");
    a.lockout = true;
    let mut s = ActivityScanner::new(list(vec![a]), ActivityConfig::default());
    assert_eq!(s.start(0.0), Step::Done);
    assert_eq!(s.progress().end, EndReason::NothingToScan);
    assert!(s.finished());
    let _ = e.advance(0.0);
}

#[test]
fn priority_is_checked_while_another_channel_is_held_and_the_scan_returns() {
    // B is busy for a long time; P (priority) is quiet. The scan must visit P every 2 s and return to B, one long hit for B.
    let mut e = FakeEngine::new(vec![station(145.6, &[(0.5, 9.0)])]);
    let mut p = entry(145.0, "P");
    p.priority = true;
    let mut s = ActivityScanner::new(
        list(vec![p, entry(145.6, "B")]),
        ActivityConfig {
            priority_interval_s: 2.0,
            ..ActivityConfig::default()
        },
    );
    drive(&mut s, &mut e, 12.0, |_, _| None);
    let visits_p_while_b: usize = e
        .retunes
        .iter()
        .filter(|(t, tune)| *t > 1.5 && *t < 9.0 && tune.freq_hz == 145.0e6)
        .count();
    assert!(visits_p_while_b >= 2, "{:?}", e.retunes);
    // After each priority check the next retune goes back to B.
    for w in e.retunes.windows(2) {
        if w[0].0 > 1.5 && w[0].0 < 9.0 && w[0].1.freq_hz == 145.0e6 {
            assert_eq!(w[1].1.freq_hz, 145.6e6, "{:?}", e.retunes);
        }
    }
    let b_hits: Vec<&Hit> = s.hits().hits().filter(|h| h.name == "B").collect();
    assert_eq!(
        b_hits.len(),
        1,
        "one stop, kept across priority checks: {b_hits:?}"
    );
    assert!(b_hits[0].end_s > 8.5);
}

#[test]
fn an_active_priority_channel_is_held_then_the_scan_returns() {
    let mut e = FakeEngine::new(vec![
        station(145.6, &[(0.5, 20.0)]),
        station(145.0, &[(2.0, 4.0)]),
    ]);
    let mut p = entry(145.0, "P");
    p.priority = true;
    let mut s = ActivityScanner::new(
        list(vec![p, entry(145.6, "B")]),
        ActivityConfig {
            priority_interval_s: 2.0,
            hang_s: 1.0,
            ..ActivityConfig::default()
        },
    );
    drive(&mut s, &mut e, 10.0, |_, _| None);
    let ph: Vec<&Hit> = s.hits().hits().filter(|h| h.name == "P").collect();
    assert_eq!(ph.len(), 1, "{:?}", s.hits().hits().collect::<Vec<_>>());
    assert!(ph[0].priority);
    assert!(ph[0].duration_s() > 1.0);
    // Back on B after the priority stop's hang.
    let after = e
        .retunes
        .iter()
        .find(|(t, _)| *t > ph[0].end_s)
        .expect("a retune after the priority stop");
    assert_eq!(after.1.freq_hz, 145.6e6);
    assert!((after.0 - (ph[0].end_s + 1.0)).abs() < 0.11, "{after:?}");
}

#[test]
fn maximum_hold_forces_the_scan_on() {
    let mut e = FakeEngine::new(vec![station(145.6, &[(0.0, 100.0)])]);
    let mut s = ActivityScanner::new(
        list(vec![entry(145.5, "A"), entry(145.6, "B")]),
        ActivityConfig {
            max_hold_s: Some(3.0),
            ..ActivityConfig::default()
        },
    );
    drive(&mut s, &mut e, 20.0, |_, _| None);
    let n = s.hits().len();
    assert!(n >= 3, "B is stopped on repeatedly, at most 3 s each: {n}");
    for h in s.hits().hits() {
        assert!(h.duration_s() <= 3.0 + TICK + 1e-6, "{h:?}");
    }
}

#[test]
fn skip_pause_and_resume() {
    let mut e = FakeEngine::new(vec![station(145.6, &[(0.0, 100.0)])]);
    let mut s = ActivityScanner::new(
        list(vec![entry(145.5, "A"), entry(145.6, "B")]),
        ActivityConfig::default(),
    );
    let mut skipped_at = None;
    let mut paused = false;
    drive(&mut s, &mut e, 8.0, |now, s| {
        if skipped_at.is_none() && s.holding() {
            skipped_at = Some(now);
            return s.skip(now);
        }
        if !paused && now > 3.0 {
            paused = true;
            s.pause();
        }
        if now > 6.0 && paused {
            paused = false;
            s.resume(now);
        }
        None
    });
    assert!(skipped_at.is_some());
    // While paused no retune happens.
    assert!(!e.retunes.iter().any(|(t, _)| *t > 3.05 && *t < 6.0));
    // The skipped stop is logged; after the resume B is held again.
    assert_eq!(s.hits().len(), 1);
    assert!(s.holding());
}

#[test]
fn band_search_runs_one_pass_and_returns() {
    let mut e = FakeEngine::new(vec![station(446.03125, &[(0.0, 100.0)])]);
    let band = BandSearch::new(446.00625e6, 446.09375e6, 12.5e3, "general").unwrap();
    e.home = Some(home(100.0, "fm"));
    let mut s = ActivityScanner::new(
        Channels::Band(band),
        ActivityConfig {
            passes: Some(1),
            max_hold_s: Some(1.0),
            ..ActivityConfig::default()
        },
    );
    assert!(drive(&mut s, &mut e, 60.0, |_, _| None));
    assert_eq!(s.progress().end, EndReason::PassesDone);
    assert_eq!(e.tuned.as_ref().unwrap().freq_hz, 100e6);
    // Eight steps visited once each, then home.
    assert_eq!(e.retunes.len(), 9);
    assert_eq!(s.hits().len(), 1);
    assert!((s.hits().hits().next().unwrap().freq_hz - 446.03125e6).abs() < 1.0);
}

#[test]
fn memory_scan_writes_hits_and_tones_back() {
    let mut store = MemoryStore::new();
    let mut a = Memory::new("Repeater", 145.6e6, "general");
    a.submode = "nfm".into();
    let a = store.insert(a, 0.0).unwrap();
    let mut b = Memory::new("Not scanned", 145.7e6, "general");
    b.scan = false;
    store.insert(b, 0.0).unwrap();
    store
        .insert(Memory::new("Quiet", 145.8e6, "general"), 0.0)
        .unwrap();
    let user_list = ScanList {
        entries: vec![entry(118.1, "Tower")],
    };
    let mem = ScanList::from_memories(
        store
            .filter(&MemoryFilter::InScan)
            .into_iter()
            .cloned()
            .collect(),
    );
    let mut st = station(145.6, &[(1.0, 3.0)]);
    st.tone = "CTCSS 88.5 Hz";
    let mut e = FakeEngine::new(vec![st]);
    let mut s = ActivityScanner::new(
        list(mem.entries),
        ActivityConfig {
            clock_offset_s: 1_000_000.0,
            ..ActivityConfig::default()
        },
    );
    drive(&mut s, &mut e, 8.0, |_, _| None);
    assert!(!e.tuned_freqs().contains(&145.7e6), "not in scan");
    assert_eq!(
        e.retunes[0].1.submode, "nfm",
        "the demodulator of the memory goes with it"
    );
    for r in s.hits().memory_hits() {
        let HostRequest::MemoryHit { memory, at_s, tone } = r else {
            panic!("{r:?}")
        };
        store
            .record_hit(memory.parse().unwrap(), at_s, &tone)
            .unwrap();
    }
    let m = store.get(a).unwrap();
    assert_eq!(m.hits, 1);
    assert_eq!(m.tone, "CTCSS 88.5 Hz");
    assert!(m.last_hit_s.unwrap() > 1_000_000.0);
    // The user's own list is untouched by a memory scan.
    assert_eq!(user_list.entries.len(), 1);
    assert_eq!(user_list.entries[0].hits, 0);
}

fn dab_channels() -> Vec<Channel> {
    vec![
        Channel::new("5A", 174.928e6, "dab"),
        Channel::new("11D", 222.064e6, "dab"),
        Channel::new("12A", 223.936e6, "dab"),
        Channel::new("12C", 227.36e6, "dab"),
    ]
}

#[test]
fn dab_channel_scan_exits_quiet_channels_early_and_names_ensembles() {
    let mut e = FakeEngine::new(vec![
        Station {
            freq_hz: 222.064e6,
            lock_after_s: Some(1.0),
            snr_db: 14.0,
            fields: vec![(&ENSEMBLE, "Mux One"), (&SNR_DB, "14")],
            name_after_s: 0.5,
            ..Station::default()
        },
        // A signal that never locks: tried until the timeout.
        Station {
            freq_hz: 223.936e6,
            active: vec![(0.0, 1000.0)],
            snr_db: 8.0,
            ..Station::default()
        },
        Station {
            freq_hz: 227.36e6,
            lock_after_s: Some(2.0),
            snr_db: 20.0,
            fields: vec![(&ENSEMBLE, "Mux Two")],
            ..Station::default()
        },
    ]);
    e.home = Some(home(98.8, "fm"));
    let mut s = ChannelScanner::new(
        dab_channels(),
        Box::new(ProfileLockTest::new(DAB)),
        ChannelScanConfig {
            keep_empty: true,
            ..ChannelScanConfig::default()
        },
    );
    assert!(drive(&mut s, &mut e, 100.0, |_, _| None));
    assert_eq!(e.tuned.as_ref().unwrap().freq_hz, 98.8e6);
    let t = s.table();
    assert_eq!(t.rows.len(), 4);
    let names: Vec<(&str, bool, Option<EmptyReason>)> = t
        .rows
        .iter()
        .map(|r| (r.name.as_str(), r.locked, r.empty))
        .collect();
    assert_eq!(
        names,
        [
            ("", false, Some(EmptyReason::NoSignal)),
            ("Mux One", true, None),
            ("", false, Some(EmptyReason::NoLock)),
            ("Mux Two", true, None)
        ]
    );
    assert_eq!(t.rows[1].value("snr_db"), Some(&Value::Text("14".into())));
    // Quiet 5A lasted about the quiet time, not the timeout.
    let t5a = e.retunes[1].0 - e.retunes[0].0;
    assert!(t5a < 2.0, "{t5a}");
    assert_eq!(s.found(), 2);
    let pr = s.progress();
    assert_eq!(
        (pr.state, pr.found, pr.end),
        (ScanState::Done, 2, EndReason::Complete)
    );
}

#[test]
fn tv_channel_scan_carries_the_channel_width() {
    let mut ch = Channel::new("21", 474e6, "dvbt");
    ch.settings.push(setting("stack.dvbt", "bandwidth", 8e6));
    let mut ch2 = Channel::new("22", 482e6, "dvbt");
    ch2.settings.push(setting("stack.dvbt", "bandwidth", 7e6));
    let mut e = FakeEngine::new(vec![Station {
        freq_hz: 482e6,
        lock_after_s: Some(1.5),
        snr_db: 22.0,
        ..Station::default()
    }]);
    let mut s = ChannelScanner::new(
        vec![ch, ch2],
        Box::new(ProfileLockTest::new(TV)),
        ChannelScanConfig::default(),
    );
    assert!(drive(&mut s, &mut e, 100.0, |_, _| None));
    assert_eq!(e.retunes[0].1.settings[0].value, Value::Hz(8e6));
    assert_eq!(e.retunes[1].1.settings[0].value, Value::Hz(7e6));
    let t = s.into_table();
    assert_eq!(t.rows.len(), 1, "empty channels are left out by default");
    assert_eq!(t.rows[0].channel, "22");
    assert_eq!(t.rows[0].settings[0].value, Value::Hz(7e6));
    let m = t.to_memories("TV", false);
    assert_eq!(m[0].settings[0].value, Value::Hz(7e6));
}

#[test]
fn fm_scan_reads_station_names_and_the_table_survives_a_restart() {
    let mut e = FakeEngine::new(vec![Station {
        freq_hz: 98.8e6,
        lock_after_s: Some(0.2),
        snr_db: 30.0,
        fields: vec![(&STATION, "RADIO 1"), (&PI, "C201")],
        name_after_s: 1.0,
        ..Station::default()
    }]);
    let mut s = ChannelScanner::new(
        Channel::raster(98.7e6, 98.9e6, 100e3, "fm"),
        Box::new(ProfileLockTest::new(FM)),
        ChannelScanConfig::default(),
    );
    assert!(drive(&mut s, &mut e, 100.0, |_, _| None));
    let t = s.into_table();
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0].name, "RADIO 1");
    assert_eq!(t.rows[0].value("pi"), Some(&Value::Text("C201".into())));

    // The saved form reads back, and a second scan merges into what was kept.
    let mut back: ScanTable = serde_json::from_slice(&serde_json::to_vec(&t).unwrap()).unwrap();
    back.merge(t.clone());
    assert_eq!(back.rows.len(), 1);
    assert_eq!(back.rows[0].seen, 2);
}

#[test]
fn a_scan_until_stopped_does_not_end() {
    let mut e = FakeEngine::new(vec![]);
    let mut s = ActivityScanner::new(list(vec![entry(1.0, "A")]), ActivityConfig::default());
    assert!(!drive(&mut s, &mut e, 2.0, |_, _| None));
}
