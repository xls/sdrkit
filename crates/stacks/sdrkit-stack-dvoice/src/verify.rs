//! What the stacks register beside their descriptors: one verification case for each stack (its test signal with an offset,
//! which must lock and fill the call log) and the benchmarks.

use crate::results::Verdict;
use crate::typed::SharedFields;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{Benchmark, Choice, Results};

pub(crate) static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static FREEDV_HOOKS: VerifyHooks = VerifyHooks {
    cases: freedv_cases,
    vector_settings: None,
    check_vector: None,
};

/// The test signal 400 Hz off the tuned frequency, long enough for every call of the schedule to start. No sound is expected
/// without a codec plug-in.
fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, 400.0)]
}

/// The offset of the FreeDV 1600 case, Hz: inside the pull-in range of the pilot loop, which has no coarse search.
pub const FREEDV_OFFSET_HZ: f64 = 5.0;
/// The offset of the FreeDV 700D and 700E cases, Hz: inside the −60 Hz to +59 Hz of their coarse search.
pub const FREEDV_OFDM_OFFSET_HZ: f64 = 40.0;

/// The FreeDV 1600 test signal [`FREEDV_OFFSET_HZ`] off the tuned frequency in the automatic submode, and the 700D and 700E
/// test signals [`FREEDV_OFDM_OFFSET_HZ`] off in their submodes.
fn freedv_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut cases = vec![case(o, FREEDV_OFFSET_HZ)];
    for submode in ["700D", "700E"] {
        let mut c = case(o, FREEDV_OFDM_OFFSET_HZ);
        c.name = format!("test signal of the submode {submode}");
        c.submode = Some(submode.into());
        cases.push(c);
    }
    cases
}

/// The test signal `offset_hz` off the tuned frequency for at least 12 s, with the checks of the call log.
fn case(o: &VerifyOptions, offset_hz: f64) -> VerifyCase {
    let mut c = VerifyCase::new("test signal", o, offset_hz);
    c.seconds = c.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.check = Some(check_calls);
    c
}

/// The call log holds at least one transmission and the verdict says the signal decodes.
fn check_calls(r: &Results) -> Vec<CheckLine> {
    let rows = r
        .decl()
        .table_id("calls")
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len());
    let verdict = r.fields().at(&SharedFields::VERDICT);
    vec![
        CheckLine::new("calls", rows > 0, format!("{rows} calls listed")),
        CheckLine::new(
            "verdict",
            matches!(
                verdict,
                Some(Verdict::Decoding | Verdict::Marginal | Verdict::NoInput)
            ),
            format!("verdict {}", verdict.map_or("none", |v| v.key())),
        ),
    ]
}

/// The family's hot paths: the symbol layer, the rate 3/4 trellis decoder of DMR and the Viterbi decoder of a NXDN channel.
pub fn benchmarks() -> Vec<Benchmark> {
    crate::bench::benchmarks()
}

/// The benchmark of DCR: a whole receiver at 250 ksps.
pub fn dcr_benchmarks() -> Vec<Benchmark> {
    crate::bench::dcr_benchmarks()
}

/// The benchmarks of FreeDV: a whole receiver on the 700D and on the 700E test signal at 96 ksps.
pub fn freedv_benchmarks() -> Vec<Benchmark> {
    crate::bench::freedv_benchmarks()
}
