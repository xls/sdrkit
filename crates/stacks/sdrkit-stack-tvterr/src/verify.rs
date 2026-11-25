//! What the crate registers beside its descriptors: one verification case per stack (its test signal off the tuned frequency,
//! which must lock), the comparison of the recorded vectors, and the benchmarks of the crate's hot paths.

use sdrkit_core::publish::Declare;
use sdrkit_core::verify::{CheckLine, VectorRun, VerifyCase, VerifyHooks, VerifyOptions};

pub(crate) static ISDBT: VerifyHooks = VerifyHooks {
    cases: isdbt_cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static ISDBTSB: VerifyHooks = VerifyHooks {
    cases: isdbtsb_cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static ATSC: VerifyHooks = VerifyHooks {
    cases: atsc_cases,
    vector_settings: None,
    check_vector: Some(check_vector),
};

pub(crate) static ATSC3: VerifyHooks = VerifyHooks {
    cases: atsc3_cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static DTMB: VerifyHooks = VerifyHooks {
    cases: dtmb_cases,
    vector_settings: None,
    check_vector: Some(check_vector),
};

/// TV (auto) has no test signal of its own: the host's generic run on the signal of another TV stack is its case.
pub(crate) static AUTO: VerifyHooks = VerifyHooks {
    cases: |_| Vec::new(),
    vector_settings: None,
    check_vector: None,
};

/// The demo signal 3 kHz off, long enough for the interleavers to fill: TMCC decoded and a layer synchronised.
fn isdbt_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 3e3);
    c.seconds = c.seconds.max(4.0);
    c.check = Some(|r| {
        let f = &crate::typed::IsdbtFields::DETECTION;
        count_at_least(f.key(), r.fields().at(f), 2)
    });
    vec![c]
}

/// The test signal of three segments 3 kHz off: TMCC decoded and a layer synchronised.
fn isdbtsb_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 3e3);
    c.seconds = c.seconds.max(5.0);
    c.check = Some(|r| {
        let f = &crate::typed::IsdbtFields::DETECTION;
        count_at_least(f.key(), r.fields().at(f), 2)
    });
    vec![c]
}

/// The test signal 10 kHz off: the transport stream flows.
fn atsc_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 10e3);
    c.seconds = c.seconds.max(1.0);
    c.check = Some(|r| {
        let f = &crate::typed::AtscFields::DETECTION;
        count_at_least(f.key(), r.fields().at(f), 4)
    });
    vec![c]
}

/// The test signal 3 kHz off: frames decoded, baseband packets given out and the media packets of its service.
fn atsc3_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 3e3);
    c.seconds = c.seconds.max(1.0);
    c.check = Some(|r| {
        let frames = &crate::typed::Atsc3Fields::FRAMES_OK;
        let packets = &crate::typed::Atsc3FrameFields::BASEBAND_PACKETS;
        let mut out = count_at_least(frames.key(), r.fields().at(frames), 3);
        out.extend(count_at_least(packets.key(), r.fields().at(packets), 10));
        // The service of the test signal: its video and its audio as media packets.
        let video = &crate::typed::Atsc3Fields::VIDEO_PACKETS;
        let audio = &crate::typed::Atsc3Fields::AUDIO_PACKETS;
        out.extend(count_at_least(video.key(), r.fields().at(video), 4));
        out.extend(count_at_least(audio.key(), r.fields().at(audio), 4));
        out
    });
    vec![c]
}

/// The test signal 3 kHz off: frame headers tracked.
fn dtmb_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 3e3);
    c.seconds = c.seconds.max(1.0);
    c.check = Some(|r| {
        let f = &crate::typed::DtmbFields::FRAMES;
        count_at_least(f.key(), r.fields().at(f), 10)
    });
    vec![c]
}

/// The count `key` of the results, `value`, at least `min`.
fn count_at_least(key: &str, value: Option<i64>, min: i64) -> Vec<CheckLine> {
    match value {
        Some(v) => vec![CheckLine::new(
            key,
            v >= min,
            format!("{key} {v}, at least {min}"),
        )],
        None => vec![CheckLine::new(key, false, format!("{key} has no value"))],
    }
}

/// The packet keys of a vector are checked by the crate's tests, which see every transport packet. The host compares the
/// others by name.
fn check_vector(v: &VectorRun) -> Vec<CheckLine> {
    let mut out = Vec::new();
    if let Some(e) = v.section["expect"].as_object() {
        for k in [
            "first_packet_within_s",
            "good_packets_min",
            "wrong_packets_max",
            "packet_rate_per_s",
            "flagged_share_max",
        ] {
            if e.contains_key(k) {
                out.push(CheckLine::not_checked(
                    format!("expect.{k}"),
                    "the transport packets are checked by the tests of sdrkit-stack-tvterr",
                ));
            }
        }
    }
    out
}
