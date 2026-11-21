//! What this crate registers beside its descriptor (`HOOKS`): the verification hooks (test signal cases for NFM with a CTCSS
//! tone, AM and USB, with checks of the results, and the comparison of the recorded vectors with their expected values) and the
//! benchmarks.

use crate::listen::{ChainSettings, ChainSquelch, ListeningChain};
use crate::settings::{Deemphasis, AM_AUTO, CW_PITCH, DEEMPH_NFM, DEMOD, GEN_CTCSS, WIDTH};
use crate::typed::GeneralFields as F;
use crate::types::Demod;
use sdrkit_core::publish::Declare;
use sdrkit_core::verify::{
    check_against, json_at, json_number, tone_amplitude, CaseSetting, CheckLine, Tolerance,
    VectorRun, VerifyCase, VerifyHooks, VerifyOptions,
};
use sdrkit_core::{Choice, FieldDecl, LockState, Results, Value, ValueKind};
use serde_json::Value as Json;

pub(crate) static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: Some(vector_settings),
    check_vector: Some(check_vector),
};

/// NFM with a CTCSS tone of 88.5 Hz, AM, and USB: each must open the squelch and make sound. NFM must find its tone.
///
/// The generator already places its station where the radio sees it when tuned with the tune offset of the stack (`-tune_offset_hz` in the
/// samples), while the host places a generator's 0 Hz on the tuned frequency. Each case's offset makes up the difference, so the
/// station lands on the tuned frequency plus the impairment's offset.
fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let base = |name: &str, demod: Demod, offset: f64| {
        let mut c = VerifyCase::new(name, o, offset).with(&DEMOD, demod);
        c.expect_audio = Some(true);
        c
    };
    let mut nfm =
        base("test signal, NFM with CTCSS 88.5 Hz", Demod::Nfm, 1000.0).with(&GEN_CTCSS, 88.5);
    nfm.check = Some(check_nfm);
    let mut am = base("test signal, AM", Demod::Am, 300.0);
    am.check = Some(check_open);
    let mut usb = base("test signal, USB", Demod::Usb, 30.0);
    usb.check = Some(check_open);
    vec![nfm, am, usb]
}

/// The squelch is open at the end, with a usable SNR.
fn check_open(r: &Results) -> Vec<CheckLine> {
    let open = r.fields().at(&F::SQUELCH_OPEN).unwrap_or(false);
    let snr = r.fields().at(&F::SNR_DB).unwrap_or(0.0);
    vec![
        CheckLine::new("squelch open", open, format!("open {open}")),
        CheckLine::new("snr", snr >= 10.0, format!("{snr:.1} dB, need 10")),
    ]
}

fn check_nfm(r: &Results) -> Vec<CheckLine> {
    let mut v = check_open(r);
    let t = r.fields().at(&F::CTCSS_HZ);
    v.push(CheckLine::new(
        "ctcss",
        t == Some(88.5),
        format!("found {t:?}, sent 88.5 Hz"),
    ));
    v
}

/// The receiver as the notes of the expected values say: the demodulator and filter of the recording, Auto quality off (the
/// envelope detector for AM), de-emphasis off, the CW pitch given.
fn vector_settings(_name: &str, section: &Json, _expected: &Json) -> Vec<CaseSetting> {
    let mut s = vec![
        CaseSetting::new(&AM_AUTO, false),
        CaseSetting::new(&DEEMPH_NFM, Deemphasis::Off),
    ];
    if let Some(d) = section["demod"].as_str().and_then(Demod::from_key) {
        s.push(CaseSetting::new(&DEMOD, d));
    }
    let set = &section["settings"];
    if let Some(w) = set["width_hz"].as_f64() {
        s.push(CaseSetting::new(&WIDTH, w));
    }
    if let Some(p) = set["pitch_hz"].as_f64() {
        s.push(CaseSetting::new(&CW_PITCH, p));
    }
    if set["am_auto_quality"].as_bool() == Some(true) {
        s.push(CaseSetting::new(&AM_AUTO, true));
    }
    if let Some(us) = set["deemphasis_us"].as_f64() {
        if us > 0.0 {
            if let Some(d) = Deemphasis::from_key(&format!("{}", us.round())) {
                s.push(CaseSetting::new(&DEEMPH_NFM, d));
            }
        }
    }
    s
}

const RATE: f64 = 48_000.0;

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}

/// The value of result field `f`, or `None` when it has none.
fn get<'a>(r: &'a Results, f: &dyn Declare) -> Option<&'a Value> {
    let id = r.decl().fields().id(f.key())?;
    r.fields().value(id).filter(|v| !v.is_none())
}

/// A number of the notes of the expected values, which no result field holds.
fn number(path: &str) -> FieldDecl {
    FieldDecl::new(path.to_owned(), path.to_owned(), "", ValueKind::F64)
}

/// Compares one recording with its section of the expected values.
fn check_vector(v: &VectorRun) -> Vec<CheckLine> {
    let mut out = Vec::new();
    let r = v.results;
    // The demodulator the recording requested is the one in use.
    out.extend(v.check_field("demod", &F::DEMOD));
    // The receiver writes mono audio.
    let mono = v.audio;
    let tail = &mono[mono.len().saturating_sub((0.5 * RATE) as usize)..];
    // The squelch, report by report: closed in the first N reports, open from N + 2 on (one report can go either way).
    let timeline: String = v
        .reports
        .iter()
        .map(|(_, r)| {
            // The lock state shows the state: locked is "squelch open".
            if r.status().lock_state == LockState::Locked {
                'O'
            } else {
                '.'
            }
        })
        .collect();
    let mut closed = 0;
    if let Some(want) = v.section["squelch_timeline"].as_str() {
        closed = want.chars().take_while(|&c| c == '.').count();
        let ok = timeline.len() >= want.len()
            && timeline[..closed].chars().all(|c| c == '.')
            && timeline[(closed + 1).min(timeline.len())..]
                .chars()
                .all(|c| c == 'O');
        out.push(CheckLine::new(
            "squelch_timeline",
            ok,
            format!(
                "{timeline}, expected {want} (closed for {closed} reports, open from the {}th on)",
                closed + 2
            ),
        ));
    }
    // The final values.
    let fin = |k: &str| format!("final.{k}");
    let tol = |k: &str| Tolerance::for_key(v.section, v.expected, k);
    let mut key = |path: &str, decl: &FieldDecl, actual: Option<&Value>, t: Option<Tolerance>| {
        if let Some(exp) = json_at(v.section, path) {
            out.push(check_against(path, exp, decl, actual, t.as_ref()));
        }
    };
    let state = match r.status().lock_state {
        LockState::Locked => 2,
        LockState::Tracking => 1,
        _ => 0,
    };
    let state_decl = FieldDecl::new("state", "State", "", ValueKind::I64);
    key(
        &fin("state"),
        &state_decl,
        Some(&Value::I64(state)),
        Some(Tolerance::Exact),
    );
    key(
        &fin("squelch_open"),
        &F::SQUELCH_OPEN.decl(),
        get(r, &F::SQUELCH_OPEN),
        Some(Tolerance::Exact),
    );
    key(
        &fin("level_dbfs"),
        &F::LEVEL_DBFS.decl(),
        get(r, &F::LEVEL_DBFS),
        tol("level_dbfs").or(Some(Tolerance::PlusMinus(1.0))),
    );
    let noise_tol = tol("noise_dbfs").or(Some(Tolerance::PlusMinus(3.0)));
    key(
        &fin("noise_dbfs"),
        &F::NOISE_DBFS.decl(),
        get(r, &F::NOISE_DBFS),
        noise_tol.clone(),
    );
    key(
        &fin("snr_db"),
        &F::SNR_DB.decl(),
        get(r, &F::SNR_DB),
        tol("snr_db"),
    );
    // The threshold follows the noise (6 dB above it), so it has the noise's tolerance.
    key(
        &fin("squelch_threshold_dbfs"),
        &F::SQUELCH_THRESHOLD_DBFS.decl(),
        get(r, &F::SQUELCH_THRESHOLD_DBFS),
        noise_tol,
    );
    key(
        &fin("offset_hz"),
        &F::OFFSET_HZ.decl(),
        get(r, &F::OFFSET_HZ),
        tol("offset_hz").or(Some(Tolerance::PlusMinus(10.0))),
    );
    // The filter.
    if let Some(f) = v.section["filter_hz"].as_array() {
        let have = [
            r.fields().at(&F::FILTER_LO_HZ),
            r.fields().at(&F::FILTER_HI_HZ),
        ];
        let want: Vec<f64> = f.iter().filter_map(Json::as_f64).collect();
        out.push(CheckLine::new(
            "filter_hz",
            have[0] == want.first().copied() && have[1] == want.get(1).copied(),
            format!("{have:?}, expected {}", v.section["filter_hz"]),
        ));
    }
    // The sound.
    let a = &v.section["audio"];
    if let Some(hz) = a["tone_hz"].as_f64() {
        let at = tone_amplitude(tail, RATE, hz);
        let beside = [hz - 150.0, hz + 150.0].map(|f| tone_amplitude(tail, RATE, f));
        out.push(CheckLine::new(
            "audio.tone_hz",
            at > 4.0 * beside[0].max(beside[1]),
            format!(
                "{at:.3} at {hz} Hz, {:.4} and {:.4} 150 Hz either side",
                beside[0], beside[1]
            ),
        ));
        out.push(check_against(
            "audio.tone_amp",
            &a["tone_amp"],
            &number("audio.tone_amp"),
            Some(&Value::F64(at)),
            tol("tone_amp").as_ref(),
        ));
        if let Some(exp) = a.get("ctcss_amp_in_audio") {
            let tone = json_at(v.section, "ctcss.detected_hz")
                .and_then(json_number)
                .unwrap_or(88.5);
            out.push(check_against(
                "audio.ctcss_amp_in_audio",
                exp,
                &number("audio.ctcss_amp_in_audio"),
                Some(&Value::F64(tone_amplitude(tail, RATE, tone))),
                tol("ctcss_amp_in_audio").as_ref(),
            ));
        }
    }
    if let Some(exp) = a.get("rms_while_closed") {
        let n = ((closed as f64 * 0.1 * RATE) as usize).min(mono.len());
        out.push(check_against(
            "audio.rms_while_closed",
            exp,
            &number("audio.rms_while_closed"),
            Some(&Value::F64(rms(&mono[..n]))),
            tol("rms_while_closed")
                .or(Some(Tolerance::AtMost(1e-4)))
                .as_ref(),
        ));
    }
    if let Some(exp) = a.get("audio_dbfs_rms") {
        let db = 20.0 * rms(tail).max(1e-12).log10();
        out.push(check_against(
            "audio.audio_dbfs_rms",
            exp,
            &number("audio.audio_dbfs_rms"),
            Some(&Value::F64(db)),
            tol("audio_dbfs_rms")
                .or(Some(Tolerance::PlusMinus(1.5)))
                .as_ref(),
        ));
    }
    // The CTCSS tone, and the tone squelch on the recording's sound.
    let c = &v.section["ctcss"];
    if let Some(exp) = c.get("detected_hz") {
        out.push(check_against(
            "ctcss.detected_hz",
            exp,
            &F::CTCSS_HZ.decl(),
            get(r, &F::CTCSS_HZ),
            Some(&Tolerance::Exact),
        ));
    }
    if let Some(o) = c.as_object() {
        let input_ctcss = a["ctcss_amp_in_audio"].as_f64().unwrap_or(0.0588);
        for (k, want) in o.iter().filter(|(k, _)| k.starts_with("tone_squelch_")) {
            let hz: f64 = k["tone_squelch_".len()..]
                .replace('_', ".")
                .parse()
                .unwrap_or(0.0);
            out.push(tone_squelch_line(
                &format!("ctcss.{k}"),
                mono,
                hz,
                want,
                input_ctcss,
            ));
        }
    }
    out
}

/// Runs the recording's sound through the listening chain with a CTCSS squelch on `hz`, and compares with `want`
/// (`timeline_per_0_1s_of_audio`, `out_1khz_amp`, `out_<tone>hz_amp`): a tone that is sent opens it within 0.9 s and keeps it open,
/// passes the 1 kHz tone and takes its own tone at least 15 dB down. Another tone never opens it.
fn tone_squelch_line(key: &str, mono: &[f32], hz: f64, want: &Json, input_ctcss: f64) -> CheckLine {
    let mut s = ChainSettings::default();
    s.control.on = true;
    s.control.squelch = ChainSquelch::Ctcss;
    s.control.ctcss_hz = hz as f32;
    s.control.squelch_hang_ms = 250.0;
    let mut chain = ListeningChain::new(RATE, s);
    let mut out = Vec::with_capacity(mono.len());
    let mut timeline = String::new();
    for block in mono.chunks(4800) {
        let mut st: Vec<[f32; 2]> = block.iter().map(|&x| [x, x]).collect();
        chain.process(&mut st);
        timeline.push(if chain.control().squelch_open() {
            'O'
        } else {
            '.'
        });
        out.extend(st.iter().map(|f| f[0]));
    }
    let tail = &out[out.len().saturating_sub((0.5 * RATE) as usize)..];
    let should_open = want["timeline_per_0_1s_of_audio"]
        .as_str()
        .is_some_and(|t| t.contains('O'));
    if should_open {
        let first = timeline.find('O');
        let k = tone_amplitude(tail, RATE, 1000.0);
        let k_want = want["out_1khz_amp"].as_f64().unwrap_or(0.25);
        let own = tone_amplitude(tail, RATE, hz);
        let down = 20.0 * (input_ctcss / own.max(1e-12)).log10();
        let ok = first.is_some_and(|f| f < 9 && timeline[f..].chars().all(|c| c == 'O'))
            && (k - k_want).abs() < 0.01
            && down >= 15.0;
        CheckLine::new(
            key,
            ok,
            format!(
                "{timeline}; 1 kHz {k:.4} (expected {k_want}); {hz} Hz {down:.1} dB down (need 15)"
            ),
        )
    } else {
        let r = rms(tail);
        CheckLine::new(
            key,
            !timeline.contains('O') && r < 1e-4,
            format!("{timeline}; rms {r:.2e} (never open, silent)"),
        )
    }
}
