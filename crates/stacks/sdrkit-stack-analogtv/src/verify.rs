//! What this crate registers beside its descriptors: the verify hooks of each stack (test-signal cases, and the recorded vectors of
//! `tests/vectors/analogtv/` against their `expected.json`).

use crate::mac::frame::Variant;
use crate::results::SoundPlaying;
use crate::settings::{GenSystem, CHANNELS, GEN_SECOND, GEN_SYSTEM, GEN_TELETEXT};
use crate::sound_gen::SecondSound;
use crate::systems::Colour;
use crate::typed::{AtvFields, FpvFields, MacFields, TeletextFields};
use sdrkit_core::verify::{
    channel, check_against, json_at, tone_amplitude, CheckLine, Tolerance, VectorRun, VerifyCase,
    VerifyHooks, VerifyOptions,
};
use sdrkit_core::{Choice, FieldDecl, LockState, Results, Value, ValueKind};
use serde_json::Value as Json;

pub(crate) static ATV_HOOKS: VerifyHooks = VerifyHooks {
    cases: atv_cases,
    vector_settings: None,
    check_vector: Some(atv_check_vector),
};
pub(crate) static MAC_HOOKS: VerifyHooks = VerifyHooks {
    cases: mac_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static FPV_HOOKS: VerifyHooks = VerifyHooks {
    cases: fpv_cases,
    vector_settings: None,
    check_vector: None,
};

fn equal(key: &str, got: &str, want: &str) -> CheckLine {
    CheckLine::new(key, got == want, format!("{got:?}, expected {want:?}"))
}

fn equal_choice<E: Choice + PartialEq + std::fmt::Debug>(
    key: &str,
    got: Option<E>,
    want: E,
) -> CheckLine {
    CheckLine::new(
        key,
        got == Some(want),
        format!("{got:?}, expected {want:?}"),
    )
}

/// `atv`: PAL B/G with NICAM stereo and teletext, and SECAM L with its AM sound.
fn atv_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut pal = VerifyCase::new("test signal, PAL B/G, NICAM stereo, teletext", o, 0.0)
        .with(&GEN_SECOND, SecondSound::NicamStereo)
        .with(&GEN_TELETEXT, true);
    pal.seconds = o.seconds.max(2.0);
    pal.expect_audio = Some(true);
    pal.check = Some(check_pal);
    let mut secam = VerifyCase::new("test signal, SECAM L", o, 0.0).with(&GEN_SYSTEM, GenSystem::L);
    secam.seconds = o.seconds.max(2.0);
    secam.expect_audio = Some(true);
    secam.check = Some(check_secam);
    vec![pal, secam]
}

fn check_pal(r: &Results) -> Vec<CheckLine> {
    let f = r.fields();
    let pages = f.at(&TeletextFields::TELETEXT_PAGES).unwrap_or(0) as f64;
    vec![
        equal(
            "system",
            f.at(&AtvFields::SYSTEM).unwrap_or(""),
            "PAL B/G 625/50",
        ),
        equal_choice(
            "sound_system",
            f.at(&AtvFields::SOUND_SYSTEM),
            SoundPlaying::NicamStereo,
        ),
        CheckLine::new("teletext_pages", pages >= 3.0, format!("{pages}, need 3")),
    ]
}

fn check_secam(r: &Results) -> Vec<CheckLine> {
    vec![
        equal(
            "system",
            r.fields().at(&AtvFields::SYSTEM).unwrap_or(""),
            "SECAM L 625/50",
        ),
        equal_choice(
            "colour_system",
            r.fields().at(&AtvFields::COLOUR_SYSTEM),
            Colour::Secam,
        ),
    ]
}

/// `mac`: the D2-MAC test signal with its service names.
fn mac_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, D2-MAC/packet", o, 0.0);
    c.seconds = o.seconds.max(1.5);
    c.expect_audio = Some(true);
    c.check = Some(|r| {
        vec![
            equal_choice("system", r.fields().at(&MacFields::SYSTEM), Variant::D2),
            equal(
                "channel",
                r.fields().at(&MacFields::CHANNEL).unwrap_or(""),
                "TEST TV",
            ),
        ]
    });
    vec![c]
}

/// `fpv`: a PAL transmitter on R5, with R5 as the one channel.
fn fpv_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, PAL on R5", o, 0.0).with(&CHANNELS, "R5=5806000000");
    c.freq_hz = Some(5806e6);
    c.seconds = o.seconds.max(2.0);
    c.expect_audio = Some(true);
    c.check = Some(|r| {
        let f = r.fields();
        let channel = f.at(&FpvFields::CHANNEL).unwrap_or("").to_string();
        vec![
            equal(
                "system",
                f.at(&FpvFields::SYSTEM).unwrap_or(""),
                "PAL B/G 625/50",
            ),
            CheckLine::new("channel", channel.contains("R5"), channel.clone()),
        ]
    });
    vec![c]
}

/// The `atv` vectors: the lock state, the sound (tone amplitudes of the 48 kHz audio) and the times of the reports. The fields are
/// compared by the host under their own keys. The pictures are not in the results, so `picture` and `bars` are not checked here
/// (the crate's tests check them).
fn atv_check_vector(v: &VectorRun) -> Vec<CheckLine> {
    let mut out = Vec::new();
    let lock_state = match v.results.status().lock_state {
        LockState::Off => "off",
        LockState::Searching => "searching",
        LockState::Tracking => "tracking",
        LockState::Locked => "locked",
    };
    out.extend(v.check_value("lock_state", &Value::Text(lock_state.into())));
    for key in ["picture", "bars"] {
        if json_at(v.section, key).is_some() {
            out.push(CheckLine::not_checked(
                key,
                "pictures are a graph output, not a result",
            ));
        }
    }
    if let Some(a) = json_at(v.section, "audio_48k").and_then(Json::as_object) {
        let channels = usize::from(v.audio_channels.max(1));
        let frames = (0.2 * v.audio_rate_hz) as usize;
        let n = v.audio.len() / channels;
        let tail = &v.audio[channels * n.saturating_sub(frames)..];
        let tol = Tolerance::parse(&Json::from(0.03));
        for (k, want) in a {
            let c = if k.starts_with("left") { 0 } else { 1 };
            let hz = if k.contains("1500") { 1500.0 } else { 1000.0 };
            let path = format!("audio_48k.{k}");
            let got = tone_amplitude(&channel(tail, v.audio_channels, c), v.audio_rate_hz, hz);
            let decl = FieldDecl::new(path.clone(), path.clone(), "", ValueKind::F64);
            out.push(check_against(
                &path,
                want,
                &decl,
                Some(&Value::F64(got)),
                Some(&tol),
            ));
        }
    }
    if json_at(v.section, "timing_s").is_some() {
        out.push(CheckLine::not_checked(
            "timing_s",
            "the report times are checked by the crate's tests",
        ));
    }
    out
}
