//! The scheduler on a simulated clock.

use super::{Conflict, Job, Missed, Outcome, Repeat, SchedAction, Scheduler, Target};

/// Monday 1 January 2024, 00:00 UTC.
const T0: i64 = 1_704_067_200;
const H: i64 = 3600;
const D: i64 = 86_400;

fn job(id: &str, start: i64, dur: i64) -> Job {
    Job {
        id: id.into(),
        name: id.into(),
        start,
        duration_s: dur,
        ..Default::default()
    }
}

/// Ticks every `step` seconds over [from, to), returning (time, action).
fn run(s: &mut Scheduler, from: i64, to: i64, step: i64) -> Vec<(i64, SchedAction)> {
    let mut v = Vec::new();
    let mut t = from;
    while t < to {
        v.extend(s.tick(t, None).into_iter().map(|a| (t, a)));
        t += step;
    }
    v
}

fn starts(v: &[(i64, SchedAction)]) -> Vec<(i64, String)> {
    v.iter()
        .filter_map(|(t, a)| match a {
            SchedAction::Start(r) => Some((*t, r.job_id.clone())),
            _ => None,
        })
        .collect()
}

fn stops(v: &[(i64, SchedAction)]) -> Vec<(i64, Outcome)> {
    v.iter()
        .filter_map(|(t, a)| match a {
            SchedAction::Stop { reason, .. } => Some((*t, *reason)),
            _ => None,
        })
        .collect()
}

#[test]
fn once_with_padding_and_a_name() {
    let mut s = Scheduler::new();
    let mut j = job("news", T0 + H, 1800);
    j.pad_before_s = 60;
    j.pad_after_s = 120;
    j.name_template = "{name}_{freq}_{date}-{time}".into();
    j.target = Target {
        freq_hz: Some(506e6),
        ..Target::default()
    };
    s.upsert(j).unwrap();
    let v = run(&mut s, T0, T0 + 2 * H, 1);
    assert_eq!(starts(&v), vec![(T0 + H - 60, "news".to_string())]);
    assert_eq!(stops(&v), vec![(T0 + H + 1800 + 120, Outcome::Done)]);
    let SchedAction::Start(r) = &v[0].1 else {
        panic!()
    };
    assert_eq!(r.file_name, "news_506000000_20240101-005900");
    assert_eq!(r.target.freq_hz, Some(506e6));
    s.report(r.id, T0 + 2 * H, "news.ts", 1234, false);
    let h = s.history.back().unwrap();
    assert_eq!(
        (h.outcome, h.bytes, h.ran),
        (Outcome::Done, 1234, Some((T0 + H - 60, T0 + H + 1920)))
    );
}

#[test]
fn repeats_daily_weekly_every_and_cron() {
    let mut s = Scheduler::new();
    s.set_utc_offset(3600); // local = UTC + 1 h
                            // Each on its own radio, so they never meet.
    let on = |r: &str, j: Job| Job {
        resource: r.into(),
        ..j
    };
    s.upsert(on(
        "a",
        Job {
            repeat: Repeat::Daily,
            ..job("daily", T0 + 6 * H, 60)
        },
    ))
    .unwrap();
    s.upsert(on(
        "b",
        Job {
            repeat: Repeat::Weekly { days: vec![5, 6] },
            ..job("weekend", T0 + 8 * H, 60)
        },
    ))
    .unwrap();
    s.upsert(on(
        "c",
        Job {
            repeat: Repeat::Every { seconds: 6 * H },
            max_runs: Some(5),
            ..job("every", T0, 60)
        },
    ))
    .unwrap();
    // 07:30 local on weekdays = 06:30 UTC.
    s.upsert(on(
        "d",
        Job {
            repeat: Repeat::Cron {
                rule: "30 7 * * mon-fri".into(),
            },
            ..job("cron", T0, 60)
        },
    ))
    .unwrap();
    let v = run(&mut s, T0, T0 + 7 * D, 30);
    let count = |id: &str| starts(&v).iter().filter(|x| x.1 == id).count();
    assert_eq!(count("daily"), 7);
    assert_eq!(count("weekend"), 2); // Saturday and Sunday
    assert_eq!(count("every"), 5); // the run limit
    assert_eq!(count("cron"), 5);
    let cron: Vec<i64> = starts(&v)
        .iter()
        .filter(|x| x.1 == "cron")
        .map(|x| x.0)
        .collect();
    assert_eq!(cron[0], T0 + 6 * H + 1800);
    // The weekend job: Saturday 6 January at 08:00 UTC.
    let w: Vec<i64> = starts(&v)
        .iter()
        .filter(|x| x.1 == "weekend")
        .map(|x| x.0)
        .collect();
    assert_eq!(w[0], T0 + 5 * D + 8 * H);
}

#[test]
fn overlaps_by_priority() {
    let mut s = Scheduler::new();
    s.upsert(Job {
        priority: 1,
        ..job("low", T0 + 100, 1000)
    })
    .unwrap();
    s.upsert(Job {
        priority: 5,
        ..job("high", T0 + 500, 200)
    })
    .unwrap();
    s.upsert(Job {
        priority: 0,
        on_conflict: Conflict::StartWhenFree,
        min_s: 60,
        ..job("patient", T0 + 200, 1000)
    })
    .unwrap();
    s.upsert(Job {
        priority: 0,
        ..job("loser", T0 + 300, 100)
    })
    .unwrap();
    s.upsert(Job {
        resource: "radio2".into(),
        ..job("other", T0 + 500, 100)
    })
    .unwrap();
    let v = run(&mut s, T0, T0 + 2000, 1);
    assert_eq!(
        starts(&v),
        vec![
            (T0 + 100, "low".to_string()),
            (T0 + 500, "high".to_string()),
            (T0 + 500, "other".to_string()),
            (T0 + 700, "patient".to_string()),
        ]
    );
    assert!(stops(&v).contains(&(T0 + 500, Outcome::Preempted)));
    let loser = s.history.iter().find(|h| h.job_id == "loser").unwrap();
    assert_eq!(loser.outcome, Outcome::Busy);
    let low = s.history.iter().find(|h| h.job_id == "low").unwrap();
    assert_eq!(low.outcome, Outcome::Preempted);
}

#[test]
fn missed_while_asleep_and_the_disk_guard() {
    let mut s = Scheduler::new();
    s.upsert(job("gone", T0 + 100, 100)).unwrap(); // passes entirely while asleep
    s.upsert(Job {
        grace_s: 600,
        ..job("late", T0 + 200, 3600)
    })
    .unwrap(); // started while asleep, within grace
    s.upsert(Job {
        on_missed: Missed::Skip,
        ..job("strict", T0 + 200, 3600)
    })
    .unwrap();
    s.upsert(Job {
        resource: "r2".into(),
        min_free_bytes: 1 << 30,
        ..job("disk", T0 + 300, 600)
    })
    .unwrap();
    assert!(s.tick(T0, Some(10 << 30)).is_empty());
    // Asleep until T0 + 400.
    let a = s.tick(T0 + 400, Some(100 << 20));
    let started: Vec<&str> = a
        .iter()
        .filter_map(|x| match x {
            SchedAction::Start(r) => Some(r.job_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(started, vec!["late"]);
    let outcome =
        |s: &Scheduler, id: &str| s.history.iter().find(|h| h.job_id == id).unwrap().outcome;
    assert_eq!(outcome(&s, "gone"), Outcome::Missed);
    assert_eq!(outcome(&s, "strict"), Outcome::Missed);
    assert_eq!(outcome(&s, "disk"), Outcome::NoSpace);
    // A failure reported by the host ends the run.
    let id = s.running[0].id;
    s.report(id, T0 + 500, "late.wav", 10, true);
    assert!(s.running.is_empty());
    assert_eq!(outcome(&s, "late"), Outcome::Failed);
}

#[test]
fn saved_and_restored() {
    let mut s = Scheduler::new();
    s.upsert(Job {
        repeat: Repeat::Every { seconds: 600 },
        ..job("e", T0, 60)
    })
    .unwrap();
    run(&mut s, T0, T0 + 1000, 10);
    let back = Scheduler::from_json(&s.to_json()).unwrap();
    assert_eq!(back, s);
    assert!(s
        .upsert(Job {
            repeat: Repeat::Cron { rule: "x".into() },
            ..job("bad", 0, 1)
        })
        .is_err());
    let stop = s.remove("e", T0 + 1210);
    assert!(stop.is_empty() || matches!(stop[0], SchedAction::Stop { .. }));
    assert!(s.jobs.is_empty());
    // A padded window touches the range before its nominal start.
    let g = Job {
        pad_before_s: 120,
        ..job("g", T0, 5400)
    };
    assert_eq!(g.windows(T0 - 100, T0, 0), vec![(T0, T0 + 5400)]);
}
