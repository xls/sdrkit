//! What the crate registers beside its descriptors: one verification case for each stack (the stack's own test signal, which
//! must lock and fill the stack's counters) and the benchmarks of the receivers.

use sdrkit_core::publish::keys;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

/// The value of the result field `key` as a number. A field with no value or of another kind reads as 0.
fn number(r: &Results, key: &str) -> f64 {
    match r.decl().field_id(key).and_then(|id| r.fields().value(id)) {
        Some(sdrkit_core::Value::I64(v)) => *v as f64,
        _ => 0.0,
    }
}

/// A check that the field `key` is at least `min`.
fn at_least(r: &Results, key: &str, min: f64) -> CheckLine {
    let v = number(r, key);
    CheckLine::new(key, v >= min, format!("{key} {v}, at least {min}"))
}

macro_rules! hooks {
    ($name:ident, $offset:expr, $seconds:expr, $field:expr, $min:expr) => {
        pub(crate) static $name: VerifyHooks = VerifyHooks {
            cases: |o: &VerifyOptions| {
                let mut c = VerifyCase::new("test signal", o, $offset);
                c.seconds = c.seconds.max($seconds);
                c.check = Some(|r: &Results| vec![at_least(r, $field, $min)]);
                vec![c]
            },
            vector_settings: None,
            check_vector: None,
        };
    };
}

hooks!(
    MPT1327,
    300.0,
    3.0,
    crate::typed::Mpt1327Fields::MESSAGES.key,
    1.0
);
hooks!(
    LTR,
    200.0,
    3.0,
    crate::typed::LtrFields::WORDS_GOOD.key,
    1.0
);
hooks!(EDACS, 300.0, 5.0, keys::FRAMES_OK.key, 1.0);
hooks!(
    TONES,
    300.0,
    25.0,
    crate::typed::TonesFields::DETECTIONS.key,
    1.0
);
hooks!(PAGER, 300.0, 4.0, crate::typed::PagerFields::PAGES.key, 1.0);
hooks!(
    MULTITONE,
    300.0,
    6.0,
    crate::typed::MultitoneFields::TRANSMISSIONS.key,
    1.0
);
hooks!(
    ERMES,
    300.0,
    3.0,
    crate::typed::ErmesFields::BATCHES.key,
    1.0
);
hooks!(
    MOTOROLA_TYPE2,
    300.0,
    3.0,
    crate::typed::MotorolaType2Fields::MESSAGES.key,
    1.0
);
hooks!(
    OPENSKY,
    300.0,
    4.0,
    crate::typed::OpenskyFields::BLOCKS.key,
    1.0
);
hooks!(GSC, 300.0, 6.0, crate::typed::GscFields::PAGES.key, 1.0);
hooks!(
    SCADA,
    300.0,
    3.0,
    crate::typed::ScadaFields::MESSAGES.key,
    1.0
);
