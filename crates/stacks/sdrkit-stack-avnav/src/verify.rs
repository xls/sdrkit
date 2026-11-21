//! What this crate registers beside its descriptors: the verify cases of each stack (test signals with checks of
//! the results).

use crate::ils::{IlsCommand, GEN_MARKER, GEN_MARKER_STOP};
use crate::marker::{GenMarker, MarkerKind, PassageKind};
use crate::ndb::{GenKeying, GEN_KEYING};
use crate::typed::{
    CarrierFields, DgpsCorrectionRow, DgpsFields, DmeFields, IdentFields, IlsFields, LoranFields,
    LoranStationRow, MarkerFields, MarkerPassageRow, VorFields, DGPS_CORRECTIONS, LORAN_STATIONS,
    MARKER_PASSAGES,
};
use crate::vor::{VorKind, GEN_KIND};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;
use sdrkit_util_geo::position::wrap180;

fn case(
    o: &VerifyOptions,
    name: &str,
    offset: f64,
    seconds: f64,
    audio: bool,
    check: fn(&Results) -> Vec<CheckLine>,
) -> VerifyCase {
    let mut c = VerifyCase::new(name, o, offset);
    c.seconds = o.seconds.max(seconds);
    c.expect_audio = Some(audio);
    c.check = Some(check);
    c
}

/// The text of field `c` (empty when it has no value).
fn text(r: &Results, c: &Column<field::Text>) -> String {
    r.fields().at(c).unwrap_or("").to_string()
}

pub(crate) static NDB_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        vec![
            // An identification of two letters is confirmed at its third hearing: 18 s hold three of 4.5 s each. The keyed
            // carrier is read one period after the 12 s of the other stacks, where the key is down again.
            case(o, "test signal, keyed tone", 40.0, 18.0, true, check_ndb),
            case(o, "test signal, keyed carrier", 40.0, 16.5, true, check_ndb)
                .with(&GEN_KEYING, GenKeying::A1a),
        ]
    },
    vector_settings: None,
    check_vector: None,
};

fn check_ndb(r: &Results) -> Vec<CheckLine> {
    let id = text(r, &IdentFields::IDENT);
    vec![
        CheckLine::new(
            "ident",
            id == "BK",
            format!("identification {id:?}, sent BK"),
        ),
        CheckLine::new(
            "carrier",
            r.fields().at(&CarrierFields::CARRIER) == Some(true),
            "carrier found",
        ),
    ]
}

pub(crate) static VOR_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        vec![
            case(o, "test signal, radial 045", 100.0, 12.0, true, check_vor),
            case(
                o,
                "test signal, Doppler VOR radial 045",
                100.0,
                12.0,
                true,
                check_vor,
            )
            .with(&GEN_KIND, VorKind::Doppler),
        ]
    },
    vector_settings: None,
    check_vector: None,
};

fn check_vor(r: &Results) -> Vec<CheckLine> {
    let radial = r.fields().at(&VorFields::RADIAL_DEG);
    let ok = radial.is_some_and(|x| wrap180(x - 45.0).abs() < 1.0);
    let id = text(r, &IdentFields::IDENT);
    vec![
        CheckLine::new("radial", ok, format!("radial {radial:?}, sent 45")),
        CheckLine::new(
            "ident",
            id == "SYD",
            format!("identification {id:?}, sent SYD"),
        ),
    ]
}

pub(crate) static ILS_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        let mut gs = case(o, "test signal, glide path", 100.0, 4.0, true, check_gs);
        gs.submode = Some("glideslope".into());
        let mut loc = case(o, "test signal, localizer", 100.0, 16.0, true, check_loc);
        loc.submode = Some("localizer".into());
        // Each marker keyed for 6 s: the passage ends 1.5 s after the last element and makes a row.
        let marker = |name: &str, m: GenMarker, check: fn(&Results) -> Vec<CheckLine>| {
            let mut c = case(o, name, 100.0, 8.0, true, check)
                .with(&GEN_MARKER, m)
                .with(&GEN_MARKER_STOP, 6.0);
            c.submode = Some("marker".into());
            c
        };
        vec![
            loc,
            gs,
            marker("test signal, outer marker", GenMarker::Outer, check_outer),
            marker("test signal, middle marker", GenMarker::Middle, check_middle),
            marker("test signal, inner marker", GenMarker::Inner, check_inner),
        ]
    },
    vector_settings: None,
    check_vector: None,
};

fn check_loc(r: &Results) -> Vec<CheckLine> {
    let ddm = r.fields().at(&IlsFields::DDM);
    vec![
        CheckLine::new(
            "ddm",
            ddm.is_some_and(|d| (d - 0.05).abs() < 0.005),
            format!("DDM {ddm:?}, sent 0.05"),
        ),
        CheckLine::new(
            "command",
            r.fields().at(&IlsFields::COMMAND) == Some(IlsCommand::FlyRight),
            format!(
                "command {:?}, want fly right",
                r.fields().at(&IlsFields::COMMAND)
            ),
        ),
        CheckLine::new(
            "ident",
            text(r, &IdentFields::IDENT) == "ISYD",
            text(r, &IdentFields::IDENT),
        ),
    ]
}

fn check_gs(r: &Results) -> Vec<CheckLine> {
    let ddm = r.fields().at(&IlsFields::DDM);
    vec![
        CheckLine::new(
            "ddm",
            ddm.is_some_and(|d| (d - 0.05).abs() < 0.005),
            format!("DDM {ddm:?}, sent 0.05"),
        ),
        CheckLine::new(
            "command",
            r.fields().at(&IlsFields::COMMAND) == Some(IlsCommand::FlyDown),
            format!(
                "command {:?}, want fly down",
                r.fields().at(&IlsFields::COMMAND)
            ),
        ),
    ]
}

fn check_outer(r: &Results) -> Vec<CheckLine> {
    check_marker(r, MarkerKind::Outer)
}

fn check_middle(r: &Results) -> Vec<CheckLine> {
    check_marker(r, MarkerKind::Middle)
}

fn check_inner(r: &Results) -> Vec<CheckLine> {
    check_marker(r, MarkerKind::Inner)
}

/// The marker `want` is named, its keying fits the standard at the rate sent (within 5 %), and its passage is one row of the
/// kind "Marker".
fn check_marker(r: &Results, want: MarkerKind) -> Vec<CheckLine> {
    let f = r.fields();
    let got = f.at(&MarkerFields::MARKER);
    let (rate, sent) = match want {
        MarkerKind::Middle => (f.at(&MarkerFields::PAIRS_PER_MIN), 90.0),
        MarkerKind::Inner => (f.at(&MarkerFields::KEYING_RATE), 6.0),
        _ => (f.at(&MarkerFields::KEYING_RATE), 2.0),
    };
    let rows: Vec<(PassageKind, MarkerKind)> = MARKER_PASSAGES
        .rows::<MarkerPassageRow>(r)
        .map(|(_, row)| (row.kind, row.marker))
        .collect();
    vec![
        CheckLine::new(
            "marker",
            got == Some(want),
            format!("marker {got:?}, sent {want:?}"),
        ),
        CheckLine::new(
            "keying",
            f.at(&MarkerFields::KEYING_FITS) == Some(true),
            text(r, &MarkerFields::KEYING),
        ),
        CheckLine::new(
            "rate",
            rate.is_some_and(|x| (x / sent - 1.0).abs() < 0.05),
            format!("rate {rate:?}, sent {sent}"),
        ),
        CheckLine::new(
            "passage",
            rows == [(PassageKind::Marker, want)],
            format!("passages {rows:?}"),
        ),
    ]
}

pub(crate) static DME_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        vec![case(
            o,
            "test signal, beacon 17X",
            0.0,
            8.0,
            false,
            check_dme,
        )]
    },
    vector_settings: None,
    check_vector: None,
};

fn check_dme(r: &Results) -> Vec<CheckLine> {
    let ch = text(r, &DmeFields::CHANNEL);
    let pps = r.fields().at(&DmeFields::PAIRS_PER_S).unwrap_or(0.0);
    vec![
        CheckLine::new("channel", ch == "17X", format!("channel {ch:?}, sent 17X")),
        CheckLine::new("pairs", pps > 1000.0, format!("{pps:.0} pairs a second")),
        CheckLine::new(
            "ident",
            text(r, &IdentFields::IDENT) == "SY",
            text(r, &IdentFields::IDENT),
        ),
    ]
}

pub(crate) static LORAN_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        let mut c = case(o, "test signal, GRI 9960", 0.0, 5.0, false, check_loran);
        c.settings = vec![CaseSetting::new(&crate::loran::GRI_SETTING, 9960i64)];
        vec![c]
    },
    vector_settings: None,
    check_vector: None,
};

fn check_loran(r: &Results) -> Vec<CheckLine> {
    let gri = r.fields().at(&LoranFields::GRI);
    let tds: Vec<f64> = LORAN_STATIONS
        .rows::<LoranStationRow>(r)
        .filter(|(_, s)| s.station != "M")
        .map(|(_, s)| s.td_us)
        .collect();
    let want = [13797.2, 26969.93, 42221.65];
    let ok = want.iter().all(|w| tds.iter().any(|t| (t - w).abs() < 0.5));
    vec![
        CheckLine::new("gri", gri == Some(9960), format!("GRI {gri:?}")),
        CheckLine::new("tds", ok, format!("TDs {tds:?}, sent {want:?}")),
    ]
}

pub(crate) static DGPS_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| {
        vec![case(
            o,
            "test signal, 200 bps",
            20.0,
            12.0,
            false,
            check_dgps,
        )]
    },
    vector_settings: None,
    check_vector: None,
};

fn check_dgps(r: &Results) -> Vec<CheckLine> {
    let st = r.fields().at(&DgpsFields::STATION_ID);
    let n = DGPS_CORRECTIONS.rows::<DgpsCorrectionRow>(r).count();
    vec![
        CheckLine::new(
            "station",
            st == Some(333),
            format!("station {st:?}, sent 333"),
        ),
        CheckLine::new("corrections", n == 6, format!("{n} corrections, sent 6")),
        CheckLine::new(
            "text",
            text(r, &DgpsFields::TEXT) == "SDRKIT TEST BEACON",
            text(r, &DgpsFields::TEXT),
        ),
    ]
}
