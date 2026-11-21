//! What the crate registers beside its descriptors: one verification case per stack (its test signal, and a check of what the
//! stack publishes), and the benchmarks.

use crate::buoy::rx::LinkFormat;
use crate::typed::HfbeaconsFields;
use crate::typed::HfidFields;
use crate::typed::HfradarFields;
use crate::typed::Pactor1Fields;
use crate::typed::PactorhfFields;
use crate::typed::RprFields;
use crate::typed::TeleprinterFields;
use crate::typed::WavebuoyFields;
use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

const fn hook(cases: fn(&VerifyOptions) -> Vec<VerifyCase>) -> VerifyHooks {
    VerifyHooks {
        cases,
        vector_settings: None,
        check_vector: None,
    }
}

pub(crate) static ALE: VerifyHooks = hook(ale_cases);
pub(crate) static PACTOR1: VerifyHooks = hook(pactor1_cases);
pub(crate) static PACTOR2: VerifyHooks = hook(pactor2_cases);
pub(crate) static PACTOR3: VerifyHooks = hook(pactor3_cases);
pub(crate) static PACTOR4: VerifyHooks = hook(pactor4_cases);
pub(crate) static HFID: VerifyHooks = hook(hfid_cases);
pub(crate) static HFBEACONS: VerifyHooks = hook(hfbeacons_cases);
pub(crate) static HFRADAR: VerifyHooks = hook(hfradar_cases);
pub(crate) static WAVEBUOY: VerifyHooks = hook(wavebuoy_cases);
pub(crate) static MS110: VerifyHooks = hook(modem_cases);
pub(crate) static S4285: VerifyHooks = hook(s4285_cases);
pub(crate) static S4539: VerifyHooks = hook(modem_cases);
pub(crate) static ALE3G: VerifyHooks = hook(ale3g_cases);
pub(crate) static S4481: VerifyHooks = hook(s4481_cases);
pub(crate) static LINK11: VerifyHooks = hook(link11_cases);
pub(crate) static EFR: VerifyHooks = hook(efr_cases);
pub(crate) static GTOR: VerifyHooks = hook(text_cases);
pub(crate) static PICCOLO: VerifyHooks = hook(text_cases);
pub(crate) static COQUELET: VerifyHooks = hook(text_cases);
pub(crate) static CIS_HF: VerifyHooks = hook(cis_cases);
pub(crate) static ARQE: VerifyHooks = hook(arqe_cases);
pub(crate) static AUTOSPEC: VerifyHooks = hook(autospec_cases);
pub(crate) static CROWD36: VerifyHooks = hook(sender_cases);
pub(crate) static XPA: VerifyHooks = hook(xpa_cases);
pub(crate) static FSK200: VerifyHooks = hook(sender_cases);
pub(crate) static GWFSK: VerifyHooks = hook(sender_cases);
pub(crate) static FSK_MODEMS: VerifyHooks = hook(sender_cases);
pub(crate) static UVB76: VerifyHooks = hook(uvb76_cases);
pub(crate) static VLF_MSK: VerifyHooks = hook(sender_cases);
pub(crate) static RPR: VerifyHooks = hook(rpr_cases);

/// The number of rows of table `name`.
fn rows(r: &Results, name: &str) -> usize {
    r.decl()
        .table_id(name)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len())
}

/// The value of whole-number field `c`, 0 when it has none.
fn count(r: &Results, c: &Column<field::I64>) -> i64 {
    r.fields().at(c).unwrap_or(0)
}

/// A check that the typed table `name` holds at least one row.
fn has_rows(r: &Results, name: &str, what: &str) -> Vec<CheckLine> {
    let n = rows(r, name);
    vec![CheckLine::new(name, n > 0, format!("{n} {what}"))]
}

/// The ALE test calls: the sounding and the calls after it fill the events table.
fn ale_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(12.0);
    c.check = Some(|r| has_rows(r, "events", "calls heard"));
    vec![c]
}

/// A serial-tone modem's test signal: at least one message read to its end.
fn modem_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(6.0);
    c.check = Some(|r| {
        let n = count(r, &crate::typed::SerialFields::MESSAGES_TOTAL);
        vec![CheckLine::new(
            "messages_total",
            n > 0,
            format!("{n} messages read"),
        )]
    });
    vec![c]
}

/// The STANAG 4285 test signal: as [`modem_cases`], over two cycles of the test signal and 1 s more. The receiver reads a
/// message from its start-of-message pattern, so a start inside a transmission waits for the next one.
fn s4285_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut cases = modem_cases(o);
    let cycle = crate::s4285::test_cycle_s(&crate::s4285::GEN_DEFAULT);
    for c in &mut cases {
        c.seconds = c.seconds.max(2.0 * cycle + 1.0);
    }
    cases
}

/// The 3G-ALE exchange: PDUs with a good check.
fn ale3g_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(6.0);
    c.check = Some(|r| {
        let n = count(r, &crate::typed::Ale3gFields::PDUS_GOOD);
        vec![CheckLine::new("pdus_good", n > 0, format!("{n} PDUs read"))]
    });
    vec![c]
}

/// The STANAG 4481 test signal: the channels found.
fn s4481_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(4.0);
    c.check = Some(|r| has_rows(r, "channels", "channels"));
    vec![c]
}

/// The Link 11 test messages: at least one message read.
fn link11_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(3.0);
    c.check = Some(|r| has_rows(r, "messages", "messages"));
    vec![c]
}

/// A teleprinter test signal: characters read.
fn text_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(12.0);
    c.check = Some(|r| {
        let n = count(r, &TeleprinterFields::GOOD);
        vec![CheckLine::new("good", n > 0, format!("{n} good"))]
    });
    vec![c]
}

/// The ARQ-E and ARQ-N test signals: 200 idle words before the text, so the words lock after about 17 s.
fn arqe_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    ["arqe", "arqn"]
        .into_iter()
        .map(|sub| {
            let mut c = VerifyCase::new(format!("test signal, {sub}"), o, 0.0);
            c.submode = Some(sub.to_string());
            c.seconds = c.seconds.max(22.0);
            c.check = Some(|r| {
                let n = count(r, &TeleprinterFields::GOOD);
                vec![CheckLine::new("good", n > 0, format!("{n} good"))]
            });
            c
        })
        .collect()
}

/// The AUTOSPEC and SPREAD test signals: 150 idle characters before the text, so the words lock after about 27 s (AUTOSPEC) or
/// 34 s (SPREAD, whose lock holds 51 word places more).
fn autospec_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    ["autospec", "spread"]
        .into_iter()
        .map(|sub| {
            let mut c = VerifyCase::new(format!("test signal, {sub}"), o, 0.0);
            c.submode = Some(sub.to_string());
            c.seconds = c.seconds.max(if sub == "spread" { 38.0 } else { 30.0 });
            c.check = Some(|r| {
                let n = count(r, &TeleprinterFields::GOOD);
                vec![CheckLine::new("good", n > 0, format!("{n} good"))]
            });
            c
        })
        .collect()
}

/// The CIS test signals, one case per submode: BEE messages, CIS-14 frames, CIS-12 symbols, and the four identifications.
fn cis_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    fn messages(r: &Results) -> Vec<CheckLine> {
        has_rows(r, "messages", "messages")
    }
    fn good(r: &Results) -> Vec<CheckLine> {
        let n = count(r, &crate::typed::CisHfFields::GOOD);
        vec![CheckLine::new(
            "good",
            n > 0,
            format!("{n} frames or symbols"),
        )]
    }
    fn identified(r: &Results) -> Vec<CheckLine> {
        let id = r
            .fields()
            .at(&crate::typed::CisHfFields::IDENTIFIED)
            .unwrap_or_default();
        vec![CheckLine::new(
            "identified",
            !id.is_empty(),
            format!("{id:?}"),
        )]
    }
    type Check = fn(&Results) -> Vec<CheckLine>;
    let all: [(&str, f64, Check); 7] = [
        ("bee", 26.0, messages),
        ("cis14", 8.0, good),
        ("cis12", 5.0, good),
        ("cis36", 12.0, identified),
        ("ofdm_hdr", 6.0, identified),
        ("cis128", 6.0, identified),
        ("cis3000", 5.0, identified),
    ];
    all.into_iter()
        .map(|(sub, secs, check)| {
            let mut c = VerifyCase::new(format!("test signal, {sub}"), o, 0.0);
            c.submode = Some(sub.to_string());
            c.seconds = c.seconds.max(secs);
            c.check = Some(check);
            c
        })
        .collect()
}

/// The EFR test telegrams: at least one telegram in the table.
fn efr_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(6.0);
    c.check = Some(|r| has_rows(r, "telegrams", "telegrams"));
    vec![c]
}

/// The PACTOR-I test link: good packets counted and the frames table filled.
fn pactor1_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(10.0);
    c.check = Some(|r| {
        let good = count(r, &Pactor1Fields::PACKETS_OK);
        let mut v = has_rows(r, "frames", "frames heard");
        v.push(CheckLine::new(
            "packets_ok",
            good > 0,
            format!("{good} good packets"),
        ));
        v
    });
    vec![c]
}

fn pactorhf_case(o: &VerifyOptions) -> VerifyCase {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(8.0);
    c
}

/// A check that the whole-number field `f` is `want`.
fn value_is(r: &Results, c: &Column<field::I64>, want: i64) -> Vec<CheckLine> {
    let v = r.fields().at(c);
    vec![CheckLine::new(
        c.key,
        v == Some(want),
        format!("{v:?}, {want} expected"),
    )]
}

/// The PACTOR-II test signal: two carriers found.
fn pactor2_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = pactorhf_case(o);
    c.check = Some(|r| value_is(r, &PactorhfFields::CARRIER_COUNT, 2));
    vec![c]
}

/// The PACTOR-III test signal: its variant found.
fn pactor3_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = pactorhf_case(o);
    c.check = Some(|r| value_is(r, &PactorhfFields::VARIANT, 3));
    vec![c]
}

/// The PACTOR-IV test signal: its variant found.
fn pactor4_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = pactorhf_case(o);
    c.check = Some(|r| value_is(r, &PactorhfFields::VARIANT, 4));
    vec![c]
}

/// The HF waveform identifier: a best estimate named and the candidates table filled.
fn hfid_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(8.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let best = r.fields().at(&HfidFields::BEST);
        let mut v = has_rows(r, "candidates", "candidates");
        v.push(CheckLine::new(
            "best",
            best.is_some(),
            format!("best estimate {best:?}"),
        ));
        v
    });
    vec![c]
}

/// The beacon test signal: beacons heard, and the beacons table filled.
fn hfbeacons_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    // Tuned to the centre of the test signal, whose beacons lie at their own frequencies.
    c.freq_hz = Some(crate::beacons::gen::DEFAULT_CENTRE_HZ);
    c.seconds = c.seconds.max(25.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let heard = count(r, &HfbeaconsFields::HEARD_NOW);
        let mut v = has_rows(r, "beacons", "beacons listed");
        v.push(CheckLine::new(
            "heard_now",
            heard > 0,
            format!("{heard} beacons heard"),
        ));
        v
    });
    vec![c]
}

/// The ocean radar test signal: radars in the radars table, and sweeps counted.
fn hfradar_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let swept = count(r, &HfradarFields::SWEEPS_COUNTED);
        let mut v = has_rows(r, "radars", "radars listed");
        v.push(CheckLine::new(
            "sweeps_counted",
            swept > 0,
            format!("{swept} sweeps counted"),
        ));
        v
    });
    vec![c]
}

/// The wave-buoy test signal: the 2FSK format found and HXV vectors decoded.
fn wavebuoy_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(20.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let format = r.fields().at(&WavebuoyFields::FORMAT);
        let want = LinkFormat::Fsk2;
        let vectors = count(r, &WavebuoyFields::VECTOR_COUNT);
        vec![
            CheckLine::new(
                "format",
                format == Some(want),
                format!("{format:?}, {want:?} (2FSK) expected"),
            ),
            CheckLine::new(
                "vector_count",
                vectors > 0,
                format!("{vectors} vectors decoded"),
            ),
        ]
    });
    vec![c]
}

/// A sender stack's test signal: at least one row read.
fn sender_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(14.0);
    c.check = Some(|r| has_rows(r, "rows", "rows read"));
    vec![c]
}

/// The XPA test signal: groups read (the message lasts about 40 s at 10 Bd).
fn xpa_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(40.0);
    c.check = Some(|r| has_rows(r, "rows", "rows read"));
    vec![c]
}

/// The UVB-76 test signal: the buzz train locks (five buzzes of the 2.4 s train and the noise of the case).
fn uvb76_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(30.0);
    c.check = Some(|r| has_rows(r, "rows", "rows read"));
    vec![c]
}

/// The Robust Packet test signal (level 1, a burst every 3 s): bursts listed with their speed level.
fn rpr_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(9.0);
    c.check = Some(|r| {
        let level = count(r, &RprFields::SPEED_LEVEL);
        let mut v = has_rows(r, "bursts", "bursts listed");
        v.push(CheckLine::new(
            "speed_level",
            level == 1,
            format!("{level}, 1 expected"),
        ));
        v
    });
    vec![c]
}
