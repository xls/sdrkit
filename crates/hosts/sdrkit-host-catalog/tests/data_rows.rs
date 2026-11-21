//! A stack whose regional values are rows of a data file runs with the values of two rows. The helper `sdrkit-presets`
//! reads the rows and returns plain settings; the engine and the stack get those and nothing else.

use sdrkit_core::net::FakeTransport;
use sdrkit_core::Value;
use sdrkit_host_engine::OwnerRef;
use sdrkit_presets::{Data, Preset};
use std::sync::Arc;
use std::time::Duration;

type Settings = Vec<(String, Value)>;

fn data() -> Data {
    Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
}

/// The frequency to start `stack` on when the row gives none: the default of the stack, else the middle of its range.
fn start_hz(stack: &str) -> f64 {
    let d = sdrkit_host_catalog::all()
        .into_iter()
        .find(|d| d.id == stack)
        .unwrap_or_else(|| panic!("no stack {stack}"));
    if d.tuning.default_hz > 0.0 {
        d.tuning.default_hz
    } else {
        (d.tuning.min_hz + d.tuning.max_hz) / 2.0
    }
}

/// Starts `stack` on its test signal with `settings` on `hz`, runs it for a moment, and checks that the stack took
/// every value and reports no problem.
fn runs(stack: &str, row: &str, hz: Option<f64>, settings: Settings) {
    assert!(
        !settings.is_empty(),
        "{stack} {row}: the row gives no setting"
    );
    let sdr = sdrkit_host_catalog::engine_on(Arc::new(FakeTransport::new())).unwrap();
    let preset = Preset {
        stack: stack.to_owned(),
        submode: None,
        plan: String::new(),
        frequency_hz: hz.unwrap_or_else(|| start_hz(stack)),
        width_hz: None,
        station: None,
        channels: Vec::new(),
        settings,
    };
    let want = sdrkit_presets::preset_values(&sdr, &preset)
        .unwrap_or_else(|e| panic!("{stack} {row}: {e}"));
    let radio = sdr.open(&format!("synth:{stack}")).unwrap();
    let session = sdrkit_presets::start_preset(&sdr, radio, &preset)
        .unwrap_or_else(|e| panic!("{stack} {row}: {e}"));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(sdr.problem(session).unwrap(), None, "{stack} {row}");
    for (key, _) in &preset.settings {
        let (got, wanted) = sdr
            .owner(OwnerRef::Stack(session), |o| {
                let id = o.schema().id(key).unwrap();
                (
                    o.values().and_then(|v| v.value(id).cloned()),
                    want.value(id).cloned(),
                )
            })
            .unwrap();
        assert_eq!(got, wanted, "{stack} {row}: {key}");
    }
    sdr.stop_session(session).unwrap();
}

/// Runs `stack` with each of the two rows, which must differ.
fn two_rows(stack: &str, rows: [(&str, Option<f64>, Settings); 2]) {
    assert_ne!(
        rows[0].2, rows[1].2,
        "{stack}: the two rows give the same values"
    );
    for (row, hz, settings) in rows {
        runs(stack, row, hz, settings);
    }
}

#[test]
fn fm_runs_with_two_bands() {
    let d = data();
    let band = |id: &str| {
        d.fm_band(id)
            .unwrap_or_else(|| panic!("no band {id}"))
            .settings()
    };
    two_rows(
        "fm",
        [
            ("fm_europe", None, band("fm_europe")),
            ("fm_japan", None, band("fm_japan")),
        ],
    );
}

#[test]
fn dvbt_runs_with_the_channels_of_two_countries() {
    let d = data();
    let of = |cc: &str| d.dvbt_settings(cc).unwrap();
    two_rows(
        "dvbt",
        [
            ("AU", Some(226.5e6), of("AU")),
            ("DE", Some(474e6), of("DE")),
        ],
    );
}

#[test]
fn dab_runs_with_two_groups() {
    let groups = data().dab_groups().unwrap();
    assert!(groups.len() >= 2);
    let of =
        |i: usize| sdrkit_presets::dab::dab_settings(&groups, &[groups[i].id.as_str()]).unwrap();
    two_rows(
        "dab",
        [(&groups[0].id, None, of(0)), (&groups[1].id, None, of(1))],
    );
}

#[test]
fn analogue_tv_runs_with_the_plans_of_two_countries() {
    let d = data();
    let of = |cc: &str| d.atv_plan_of_country(cc).unwrap().settings();
    two_rows("atv", [("AU", None, of("AU")), ("US", None, of("US"))]);
}

#[test]
fn marine_vhf_and_atis_run_with_two_plans() {
    let plans = data().marine_plans().unwrap();
    assert!(plans.len() >= 2);
    for stack in ["marinevhf", "atis"] {
        two_rows(
            stack,
            [
                (&plans[0].id, None, plans[0].settings()),
                (&plans[1].id, None, plans[1].settings()),
            ],
        );
    }
}

#[test]
fn zwave_runs_with_two_regions() {
    let rows = data().zwave_regions().unwrap();
    assert!(rows.len() >= 2);
    let of = |i: usize| sdrkit_presets::ism::zwave_settings(&rows, &rows[i].id).unwrap();
    let (a, b) = (of(0), of(1));
    two_rows(
        "zwave",
        [(&rows[0].id, Some(a.0), a.1), (&rows[1].id, Some(b.0), b.1)],
    );
}

#[test]
fn wisun_runs_with_two_plans() {
    let plans = data().wisun_bands().unwrap();
    assert!(plans.len() >= 2);
    let last = plans.len() - 1;
    // The stack has no default frequency: each plan runs in the middle of its first range.
    let mid = |p: &sdrkit_presets::ism::WisunPlan| p.ranges_hz.first().map(|r| (r.0 + r.1) / 2.0);
    two_rows(
        "wisun",
        [
            (&plans[0].id, mid(&plans[0]), plans[0].settings()),
            (&plans[last].id, mid(&plans[last]), plans[last].settings()),
        ],
    );
}

#[test]
fn halow_runs_with_two_regions() {
    let rows = data().halow_regions().unwrap();
    assert!(rows.len() >= 2);
    let last = rows.len() - 1;
    two_rows(
        "halow",
        [
            (&rows[0].code, None, rows[0].settings()),
            (&rows[last].code, None, rows[last].settings()),
        ],
    );
}

#[test]
fn wifi_runs_with_two_domains() {
    let rows = data().wifi_domains().unwrap();
    let (first, last) = (&rows[0].domain, &rows[rows.len() - 1].domain);
    let of = |domain: &str| sdrkit_presets::wlan::wifi_settings(&rows, domain, false).unwrap();
    two_rows("wifi", [(first, None, of(first)), (last, None, of(last))]);
}

#[test]
fn acars_and_vdl2_run_with_two_channel_sets() {
    let rows = data().datalink_channels().unwrap();
    for stack in ["acars", "vdl2"] {
        let mut sets: Vec<&str> = rows
            .iter()
            .filter(|c| c.stack == stack)
            .map(|c| c.set.as_str())
            .collect();
        sets.dedup();
        assert!(sets.len() >= 2, "{stack}: {sets:?}");
        let of = |set: &str| sdrkit_presets::avdata::settings(&rows, stack, set).unwrap();
        two_rows(
            stack,
            [(sets[0], None, of(sets[0])), (sets[1], None, of(sets[1]))],
        );
    }
}

#[test]
fn lte_and_nbiot_run_with_two_band_orders() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../data/cell/band_order.csv"
    );
    let rows = sdrkit_presets::cell::read_cell_lists(std::path::Path::new(path)).unwrap();
    for stack in ["cell", "nbiot"] {
        let of = |name: &str| sdrkit_presets::cell::cell_settings(&rows, stack, name).unwrap();
        two_rows(
            stack,
            [
                ("europe", None, of("europe")),
                ("band_number", None, of("band_number")),
            ],
        );
    }
}

#[test]
fn meshtastic_runs_with_two_regions() {
    let d = data();
    let regions = d.meshtastic_regions().unwrap();
    assert!(regions.len() >= 2);
    let of = |i: usize| {
        let r = &regions[i];
        let s = d.meshtastic_settings(&r.name, "LongFast").unwrap();
        (
            r.name.as_str(),
            Some((r.start_hz + r.end_hz) / 2.0),
            s.settings,
        )
    };
    two_rows("meshtastic", [of(0), of(regions.len() - 1)]);
}

#[test]
fn wide_lorawan_runs_with_two_plans() {
    let d = data();
    let plans = d.lorawan_plans().unwrap();
    assert!(plans.len() >= 2);
    let of = |i: usize| {
        let p = &plans[i];
        let s = d.lorawan_settings(&p.plan, 0).unwrap();
        (
            p.plan.as_str(),
            s.frequency_hz.or(Some(p.centre_hz)),
            s.settings,
        )
    };
    two_rows("lorawanwide", [of(0), of(plans.len() - 1)]);
}

#[test]
fn the_mesh_stacks_run_with_two_band_plans() {
    let d = data();
    for stack in ["mesh", "meshtastic", "meshcore"] {
        let of = |plan: &'static str| {
            let s = d.mesh_band_plan_settings(plan, stack).unwrap();
            (plan, s.frequency_hz, s.settings)
        };
        two_rows(stack, [of("EU 868"), of("US 915")]);
    }
}

#[test]
fn ft8_runs_with_two_dials() {
    let d = data();
    let (dials, bands) = (d.wsjt_dials().unwrap(), d.wsjt_bands().unwrap());
    assert!(dials.len() >= 2);
    let of = |i: usize| sdrkit_presets::wsjt::settings(&dials[i..=i], &bands);
    two_rows(
        "ft8",
        [("first", None, of(0)), ("last", None, of(dials.len() - 1))],
    );
}

#[test]
fn the_general_receiver_runs_with_two_presets() {
    let presets = data().general_presets().unwrap();
    assert!(presets.len() >= 2);
    let of = |i: usize| vec![sdrkit_presets::general::presets_setting(&presets[i..=i])];
    two_rows(
        "general",
        [
            ("first", None, of(0)),
            ("last", None, of(presets.len() - 1)),
        ],
    );
}
