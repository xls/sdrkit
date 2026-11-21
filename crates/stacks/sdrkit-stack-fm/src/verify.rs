//! What FM registers beside its descriptor (`HOOKS`): the verify cases (test signals with checks of the decoded RDS and
//! of the choice between RDS and RBDS), the comparison of the recorded test vector with its `expected.json`, and the
//! benchmarks.

use crate::pty::pi_from_call_letters;
use crate::results::Reading;
use crate::settings::{GEN_EXTRA_GROUPS, GEN_PI, GEN_PS};
use sdrkit_core::verify::{
    channel, check_against, json_at, CheckLine, Tolerance, VectorRun, VerifyCase, VerifyHooks,
    VerifyOptions,
};
use sdrkit_core::{FieldDecl, LockState, Results, Value, ValueKind};

pub(crate) static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: Some(check_vector),
};

/// The test station (stereo, RDS "SDRKIT", PI 4A21, PTY 10) on 100.0 MHz; a US station on 101.1 MHz (RBDS: call letters "WXYZ",
/// PTY 10 is "Country"); and a station outside North America on 104.1 MHz, an odd tenth, whose PI 2049 falls in the K range and
/// which sends the Australian country code 0xF0 in group 1A: it must not show call letters ("KGEJ").
fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let offset = 1000.0;
    let mut stereo = VerifyCase::new("test signal, stereo with RDS", o, offset);
    stereo.expect_audio = Some(true);
    stereo.check = Some(check_station);
    let mut us = VerifyCase::new("test signal, RBDS on 101.1 MHz", o, offset)
        .with(&GEN_PI, &format!("{:04X}", pi_from_call_letters("WXYZ")))
        .with(&GEN_PS, "WXYZ FM ");
    us.freq_hz = Some(101.1e6);
    us.expect_audio = Some(true);
    us.check = Some(check_rbds);
    let mut au = VerifyCase::new("test signal, Australian station on 104.1 MHz", o, offset)
        .with(&GEN_PI, "2049")
        .with(&GEN_PS, "SYDNEY  ")
        // Group 1A, variant 0: the extended country code 0xF0 (Australia); block B with TP and PTY 10.
        .with(&GEN_EXTRA_GROUPS, "1540 00F0 0000");
    au.freq_hz = Some(104.1e6);
    au.seconds = o.seconds.max(3.0);
    au.expect_audio = Some(true);
    au.check = Some(check_not_rbds);
    vec![stereo, us, au]
}

fn check_station(r: &Results) -> Vec<CheckLine> {
    let d = Reading::from_results(r);
    let st = d.station.trim_end();
    vec![
        CheckLine::new(
            "station",
            st == "SDRKIT",
            format!("{st:?}, sent \"SDRKIT\""),
        ),
        CheckLine::new("pi", d.pi == 0x4A21, format!("{:04X}, sent 4A21", d.pi)),
        CheckLine::new(
            "programme",
            d.programme == "Pop Music",
            format!("{:?}, sent PTY 10 (RDS)", d.programme),
        ),
        CheckLine::new("stereo", d.stereo, format!("{:.0} %", d.stereo_pct)),
    ]
}

fn check_rbds(r: &Results) -> Vec<CheckLine> {
    let d = Reading::from_results(r);
    vec![
        CheckLine::new(
            "call letters",
            d.call_letters == "WXYZ",
            format!("{:?}, sent the PI of WXYZ", d.call_letters),
        ),
        CheckLine::new(
            "programme",
            d.programme == "Country",
            format!("{:?}, PTY 10 in RBDS", d.programme),
        ),
    ]
}

fn check_not_rbds(r: &Results) -> Vec<CheckLine> {
    let d = Reading::from_results(r);
    vec![
        CheckLine::new(
            "ecc",
            d.ecc == Some(0xF0),
            format!("{:?}, sent 0xF0", d.ecc),
        ),
        CheckLine::new(
            "no call letters",
            d.call_letters.is_empty() && !d.rbds,
            format!("call letters {:?}, rbds {}", d.call_letters, d.rbds),
        ),
        CheckLine::new(
            "programme",
            d.programme == "Pop Music",
            format!("{:?}, PTY 10 in RDS", d.programme),
        ),
    ]
}

fn lock_state_name(l: LockState) -> &'static str {
    match l {
        LockState::Off => "off",
        LockState::Searching => "searching",
        LockState::Tracking => "tracking",
        LockState::Locked => "locked",
    }
}

/// The first report time (seconds of signal fed) at which `f` holds.
fn first(v: &VectorRun, f: impl Fn(&Reading, &Results) -> bool) -> Option<f64> {
    v.reports
        .iter()
        .find_map(|(t, r)| f(&Reading::from_results(r), r).then_some(*t))
}

/// Compares the recording with `expected.json`: the RDS, the signal, the sound and the timing.
fn check_vector(v: &VectorRun) -> Vec<CheckLine> {
    let mut out = Vec::new();
    let d = Reading::from_results(v.results);
    let tol = |path: &str| Tolerance::for_key(v.section, v.expected, path);
    let mut key = |path: &str, kind: ValueKind, actual: Option<Value>, t: Option<Tolerance>| {
        if let Some(exp) = json_at(v.section, path) {
            let t = t.or_else(|| tol(path));
            let decl = FieldDecl::new(path.to_owned(), path.to_owned(), "", kind);
            out.push(check_against(path, exp, &decl, actual.as_ref(), t.as_ref()));
        }
    };
    let text = |x: &str| Some(Value::Text(x.to_owned()));
    let exact = Some(Tolerance::Exact);
    // RDS: names, flags and counters (the PI, PS, RT, PTY and flags exactly).
    key(
        "rds.pi",
        ValueKind::Text,
        text(&format!("{:04X}", d.pi)),
        None,
    );
    key("rds.ps", ValueKind::Text, text(&d.station), None);
    key("rds.rt", ValueKind::Text, text(&d.text), None);
    key(
        "rds.rt_complete",
        ValueKind::Bool,
        Some(Value::Bool(d.text_complete)),
        None,
    );
    key(
        "rds.pty",
        ValueKind::I64,
        d.pty.map(|x| Value::I64(i64::from(x))),
        exact.clone(),
    );
    key("rds.pty_name", ValueKind::Text, text(&d.programme), None);
    key(
        "rds.tp",
        ValueKind::Bool,
        Some(Value::Bool(d.traffic)),
        None,
    );
    key("rds.ta", ValueKind::Bool, Some(Value::Bool(d.ta)), None);
    key(
        "rds.music",
        ValueKind::Bool,
        Some(Value::Bool(d.music)),
        None,
    );
    key("rds.rbds", ValueKind::Bool, Some(Value::Bool(d.rbds)), None);
    key(
        "rds.call_sign",
        ValueKind::Text,
        text(&d.call_letters),
        None,
    );
    key("rds.sync", ValueKind::Bool, Some(Value::Bool(d.rds)), None);
    key(
        "rds.block_ok_pct",
        ValueKind::F64,
        Some(Value::F64(d.rds_ok)),
        Some(Tolerance::AtLeast(99.0)),
    );
    key(
        "rds.groups_decoded",
        ValueKind::I64,
        Some(Value::I64(d.groups as i64)),
        None,
    );
    // The signal.
    key(
        "signal.state",
        ValueKind::Text,
        text(lock_state_name(v.results.status().lock_state)),
        None,
    );
    key(
        "signal.carrier",
        ValueKind::Bool,
        Some(Value::Bool(d.carrier)),
        None,
    );
    key(
        "signal.stereo",
        ValueKind::Bool,
        Some(Value::Bool(d.stereo)),
        None,
    );
    key(
        "signal.stereo_pct",
        ValueKind::F64,
        Some(Value::F64(d.stereo_pct)),
        Some(Tolerance::AtLeast(99.0)),
    );
    key(
        "signal.pilot_pct",
        ValueKind::F64,
        Some(Value::F64(d.pilot_pct)),
        None,
    );
    key(
        "signal.cfo_hz",
        ValueKind::F64,
        Some(Value::F64(d.offset_hz)),
        None,
    );
    key(
        "signal.dev_khz_peak",
        ValueKind::F64,
        Some(Value::F64(d.deviation_hz / 1e3)),
        Some(Tolerance::PlusMinus(3.0)),
    );
    key(
        "signal.snr_db",
        ValueKind::F64,
        Some(Value::F64(d.snr_db)),
        None,
    );
    key(
        "signal.multipath_pct",
        ValueKind::F64,
        Some(Value::F64(d.multipath_pct)),
        Some(Tolerance::AtMost(3.0)),
    );
    key(
        "signal.equaliser",
        ValueKind::Bool,
        Some(Value::Bool(d.equaliser)),
        None,
    );
    // The sound: frames, and the tones of the last second (a full-scale sine is 1.0).
    let channels = v.audio_channels.max(1);
    let frames = v.audio.len() / usize::from(channels);
    key(
        "audio_48k.frames",
        ValueKind::I64,
        Some(Value::I64(frames as i64)),
        Some(Tolerance::PlusMinus(200.0)),
    );
    let rate = if v.audio_rate_hz > 0.0 {
        v.audio_rate_hz
    } else {
        48e3
    };
    let last_s = &v.audio[v
        .audio
        .len()
        .saturating_sub(usize::from(channels) * rate as usize)..];
    let (l, r) = (channel(last_s, channels, 0), channel(last_s, channels, 1));
    let amp = |x: &[f32], hz: f64| sdrkit_core::verify::tone_amplitude(x, rate, hz);
    let (l1, l3, r3, r1) = (
        amp(&l, 1000.0),
        amp(&l, 3000.0),
        amp(&r, 3000.0),
        amp(&r, 1000.0),
    );
    key(
        "audio_48k.left_1khz_amp",
        ValueKind::F64,
        Some(Value::F64(l1)),
        None,
    );
    key(
        "audio_48k.right_3khz_amp",
        ValueKind::F64,
        Some(Value::F64(r3)),
        None,
    );
    // The other channel's tone: what leaks across, as the separation (at least 30 dB unless the file says otherwise).
    let sep = match tol("separation_db") {
        Some(Tolerance::AtLeast(x)) => x,
        _ => 30.0,
    };
    let leak = |wanted: f64| wanted * 10f64.powf(-sep / 20.0);
    key(
        "audio_48k.left_3khz_amp",
        ValueKind::F64,
        Some(Value::F64(l3)),
        Some(Tolerance::AtMost(leak(l1))),
    );
    key(
        "audio_48k.right_1khz_amp",
        ValueKind::F64,
        Some(Value::F64(r1)),
        Some(Tolerance::AtMost(leak(r3))),
    );
    // Timing: the first report at which each item appeared; without a tolerance, at most one report (0.2 s) later than expected.
    let ps = v.section["rds"]["ps"].as_str().unwrap_or("").to_string();
    let times: [(&str, Option<f64>); 5] = [
        (
            "t_lock",
            first(v, |_, r| r.status().lock_state == LockState::Locked),
        ),
        ("t_stereo", first(v, |d, _| d.stereo)),
        ("t_rds_sync", first(v, |d, _| d.rds)),
        ("t_ps", first(v, |d, _| !ps.is_empty() && d.station == ps)),
        ("t_rt_complete", first(v, |d, _| d.text_complete)),
    ];
    for (k, t) in times {
        let path = format!("timing_s.{k}");
        let Some(exp) = json_at(v.section, &path).and_then(serde_json::Value::as_f64) else {
            continue;
        };
        let t_tol = tol(&path).unwrap_or(Tolerance::AtMost(exp + 0.2));
        key(&path, ValueKind::F64, t.map(Value::F64), Some(t_tol));
    }
    out
}
