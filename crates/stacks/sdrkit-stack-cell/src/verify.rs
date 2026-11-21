//! What the family registers beside its descriptors: one verification case per stack. Each case runs the stack's test signal
//! through its receiver and makes sure that the stack's main table holds at least one row.

use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

macro_rules! hooks {
    ($name:ident, $cases:ident, $table:expr, $seconds:expr, $what:expr) => {
        hooks!($name, $cases, $table, $seconds, $what, None);
    };
    ($name:ident, $cases:ident, $table:expr, $seconds:expr, $what:expr, $freq:expr) => {
        pub(crate) static $name: VerifyHooks = VerifyHooks {
            cases: $cases,
            vector_settings: None,
            check_vector: None,
        };
        fn $cases(o: &VerifyOptions) -> Vec<VerifyCase> {
            let mut c = VerifyCase::new("test signal", o, 0.0);
            c.freq_hz = $freq;
            c.seconds = c.seconds.max($seconds);
            c.expect_audio = Some(false);
            c.check = Some(|r: &Results| check_rows(r, $table, $what));
            vec![c]
        }
    };
}

hooks!(LTE, lte_cases, "cells", 1.0, "LTE cells");
// The cells of the NR test scene lie on the sync raster around the centre that the scene assumes; the search finds them only
// when the session listens there.
hooks!(NR, nr_cases, "cells", 1.0, "NR cells", Some(crate::nr::TEST_SIGNAL_HZ));
hooks!(NBIOT, nbiot_cases, "cells", 3.0, "NB-IoT cells");
hooks!(GSM, gsm_cases, "cells", 3.0, "GSM cells");
hooks!(LBJ, lbj_cases, "messages", 6.0, "LBJ messages");
hooks!(BALISE, balise_cases, "telegrams", 1.0, "balise telegrams");
hooks!(EOT, eot_cases, "frames", 2.0, "end-of-train frames");
hooks!(UMTS, umts_cases, "cells", 1.0, "UMTS cells");
hooks!(CDMA2000, cdma2000_cases, "cells", 2.0, "CDMA2000 base stations");
hooks!(TDSCDMA, tdscdma_cases, "cells", 1.0, "TD-SCDMA cells");

/// C-V2X: the detector finds the sidelink structure of its test signal.
pub(crate) static CV2X: VerifyHooks = VerifyHooks {
    cases: cv2x_cases,
    vector_settings: None,
    check_vector: None,
};

fn cv2x_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(2.0);
    c.expect_audio = Some(false);
    c.check = Some(|r: &Results| {
        let d = r.fields().at(&crate::typed::Cv2xFields::DETECTED).unwrap_or(false);
        vec![CheckLine::new(
            crate::typed::Cv2xFields::DETECTED.key,
            d,
            format!("detected: {d}"),
        )]
    });
    vec![c]
}

/// The table `table` holds at least one row.
fn check_rows(r: &Results, table: &str, what: &str) -> Vec<CheckLine> {
    let n = r
        .decl()
        .table_id(table)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len());
    vec![CheckLine::new(table, n > 0, format!("{n} {what} listed"))]
}
