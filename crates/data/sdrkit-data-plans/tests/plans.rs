//! Tests of sdrkit-data-plans: the data against its schema, known values from the published plans, the region rules, user files and
//! the JSON calls.

use sdrkit_data_plans::api::{self, PlanFormat};
use sdrkit_data_plans::data::{self, Known};
use sdrkit_data_plans::naming::icao_833_name;
use sdrkit_data_plans::schema::{json_schema, json_schema_text};
use sdrkit_data_plans::*;
use serde_json::{json, Value};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The plans of the repository's `data/plans/`, as a store of their own.
fn store() -> Plans {
    (*sdrkit_dev_test::plans()).clone()
}

fn data_files() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = std::fs::read_dir(root().join("../../../data/plans"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

// ---- The schema ----

/// A small JSON Schema checker for the keywords the plan schema uses.
fn conforms(schema: &Value, root: &Value, v: &Value, path: &str, out: &mut Vec<String>) {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        let name = r.trim_start_matches("#/$defs/");
        return conforms(&root["$defs"][name], root, v, path, out);
    }
    if let Some(c) = schema.get("const") {
        if c != v {
            out.push(format!("{path}: not {c}"));
        }
    }
    if let Some(e) = schema.get("enum").and_then(Value::as_array) {
        if !e.contains(v) {
            out.push(format!("{path}: {v} is not one of {e:?}"));
        }
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let Some(o) = v.as_object() else {
                return out.push(format!("{path}: not an object"));
            };
            let props = schema["properties"].as_object().unwrap();
            for r in schema["required"].as_array().unwrap() {
                if !o.contains_key(r.as_str().unwrap()) {
                    out.push(format!("{path}: {r} is missing"));
                }
            }
            for (k, x) in o {
                match props.get(k) {
                    Some(s) => conforms(s, root, x, &format!("{path}.{k}"), out),
                    None => out.push(format!("{path}: {k} is not in the schema")),
                }
            }
        }
        Some("array") => match v.as_array() {
            Some(a) => {
                for (i, x) in a.iter().enumerate() {
                    conforms(&schema["items"], root, x, &format!("{path}[{i}]"), out);
                }
            }
            None => out.push(format!("{path}: not an array")),
        },
        Some("string") => {
            let Some(s) = v.as_str() else {
                return out.push(format!("{path}: not a string"));
            };
            if let Some(p) = schema.get("pattern").and_then(Value::as_str) {
                let ok = match p {
                    "^[a-z0-9_-]{1,64}$" => data::is_id(s),
                    "^[A-Z]{2}$" => s.len() == 2 && s.chars().all(|c| c.is_ascii_uppercase()),
                    "^[0-9]{4}(-[0-9]{2}(-[0-9]{2})?)?$" => data::is_date(s),
                    _ => panic!("pattern {p} is not known to the test"),
                };
                if !ok {
                    out.push(format!("{path}: {s:?} does not match {p}"));
                }
            }
        }
        Some("number") | Some("integer") => {
            let Some(n) = v.as_f64() else {
                return out.push(format!("{path}: not a number"));
            };
            if schema["type"] == "integer" && n.fract() != 0.0 {
                out.push(format!("{path}: not an integer"));
            }
            if let Some(m) = schema.get("minimum").and_then(Value::as_f64) {
                if n < m {
                    out.push(format!("{path}: below {m}"));
                }
            }
            if let Some(m) = schema.get("exclusiveMinimum").and_then(Value::as_f64) {
                if n <= m {
                    out.push(format!("{path}: not above {m}"));
                }
            }
            if let Some(m) = schema.get("maximum").and_then(Value::as_f64) {
                if n > m {
                    out.push(format!("{path}: above {m}"));
                }
            }
        }
        Some("boolean") if !v.is_boolean() => out.push(format!("{path}: not a boolean")),
        _ => {}
    }
}

/// A document with every property the schema allows, for the round trip through the model.
fn fullest(schema: &Value, root: &Value) -> Value {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        let name = r.trim_start_matches("#/$defs/");
        return fullest(&root["$defs"][name], root);
    }
    if let Some(c) = schema.get("const") {
        return c.clone();
    }
    if let Some(e) = schema.get("enum").and_then(Value::as_array) {
        return e[0].clone();
    }
    match schema["type"].as_str().unwrap_or("") {
        "object" => Value::Object(
            schema["properties"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, s)| (k.clone(), fullest(s, root)))
                .collect(),
        ),
        "array" => json!([fullest(&schema["items"], root)]),
        "string" => match schema.get("pattern").and_then(Value::as_str) {
            Some("^[A-Z]{2}$") => json!("AU"),
            Some("^[0-9]{4}(-[0-9]{2}(-[0-9]{2})?)?$") => json!("2013-12-10"),
            _ => json!("x"),
        },
        "integer" => json!(1),
        "number" => json!(1.0),
        "boolean" => json!(true),
        other => panic!("type {other} is not known to the test"),
    }
}

#[test]
fn the_checked_in_schema_is_the_generated_one() {
    let path = root().join("../../../docs/schema/plans.json");
    if std::env::var_os("SDRKIT_WRITE_SCHEMA").is_some() {
        std::fs::write(&path, json_schema_text()).unwrap();
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        on_disk == json_schema_text(),
        "docs/schema/plans.json is stale: run SDRKIT_WRITE_SCHEMA=1 cargo test -p sdrkit-data-plans --test plans"
    );
}

#[test]
fn every_data_file_follows_the_schema() {
    let schema = json_schema();
    for (name, text) in data_files() {
        let v: Value = serde_json::from_str(&text).unwrap();
        let mut out = Vec::new();
        conforms(&schema, &schema, &v, "$", &mut out);
        assert!(out.is_empty(), "{name}: {out:#?}");
    }
}

#[test]
fn the_schema_and_the_model_have_the_same_fields() {
    let schema = json_schema();
    // Every property of the schema is a field of the model: the fullest document reads without an unknown field.
    let doc = fullest(&schema, &schema);
    let file: PlanFile =
        serde_json::from_value(doc.clone()).expect("the model reads every schema property");
    // Every field of the model is in the schema: written back, the document still follows the schema with every key.
    let back = serde_json::to_value(&file).unwrap();
    let mut out = Vec::new();
    conforms(&schema, &schema, &back, "$", &mut out);
    assert!(out.is_empty(), "{out:#?}");
    assert_eq!(
        back["groups"][0]["bands"][0].as_object().unwrap().len(),
        doc["groups"][0]["bands"][0].as_object().unwrap().len() - 1,
        "every band field but the default in_use comes back"
    );
}

// ---- The data ----

#[test]
fn every_group_names_its_source_and_the_data_checks_clean() {
    let files: Vec<(String, PlanFile)> = data_files()
        .into_iter()
        .map(|(n, t)| (n.clone(), data::parse(&n, &t).unwrap()))
        .collect();
    let known = Known {
        countries: files
            .iter()
            .flat_map(|(_, f)| f.countries.iter())
            .filter(|c| !c.name.is_empty() && c.itu.is_some())
            .map(|c| c.code.clone())
            .collect(),
        ..Known::default()
    };
    assert_eq!(data::check(&files, &known), vec![]);
    let groups: Vec<&BandGroup> = files.iter().flat_map(|(_, f)| f.groups.iter()).collect();
    assert!(groups.len() > 60);
    for g in groups {
        assert!(
            !g.source.publisher.is_empty()
                && !g.source.title.is_empty()
                && !g.source.date.is_empty(),
            "{}",
            g.id
        );
        assert!(g.verified || !g.unverified_reason.is_empty(), "{}", g.id);
    }
    // The seven countries: every one names its sources. A country with one preset that no
    // free official source states keeps verified false with the reason.
    for id in ["AU", "NZ", "GB", "DE", "US", "CA", "JP"] {
        let plans = store();
        let c = plans
            .country_rows()
            .find(|c| c.code == id && !c.name.is_empty())
            .unwrap();
        assert!(!c.sources.is_empty(), "{id}");
        assert_eq!(c.verified, ["AU", "CA", "JP"].contains(&id), "{id}");
        assert!(c.verified || !c.unverified_reason.is_empty(), "{id}");
    }
}

#[test]
fn a_data_error_names_the_file_the_path_and_the_reason() {
    let e = data::parse(
        "mine.json",
        r#"{"format":1,"groups":[{"id":"a","name":"A","kind":"tv","verified":true,"stepp":1}]}"#,
    )
    .unwrap_err();
    assert_eq!(
        (e.file.as_str(), e.path.as_str()),
        ("mine.json", "$.groups[0].stepp")
    );
    assert!(e.reason.contains("unknown field"), "{}", e.reason);
    let e = data::parse("mine.json", r#"{"format":2}"#).unwrap_err();
    assert_eq!(e.path, "$.format");
    let bad = r#"{"format":1,"groups":[{"id":"a","name":"A","kind":"tv","verified":true,
        "source":{"publisher":"p","title":"t","date":"d"},
        "bands":[{"name":"B","service":"broadcasting","start_hz":2e6,"end_hz":1e6}]}]}"#;
    let f = data::parse("mine.json", bad).unwrap();
    let errs = data::check(&[("mine.json".into(), f)], &Known::default());
    assert_eq!(errs[0].path, "$.groups[0].bands[0].end_hz");
    assert_eq!(errs[0].to_string().split(": ").next(), Some("mine.json"));
}

fn plan(id: &str) -> &'static ChannelPlan {
    static PLANS: std::sync::OnceLock<std::sync::Arc<Plans>> = std::sync::OnceLock::new();
    PLANS
        .get_or_init(sdrkit_dev_test::plans)
        .get(id)
        .unwrap_or_else(|| panic!("{id}"))
}

fn mhz(p: &ChannelPlan, name: &str) -> f64 {
    p.find(name)
        .unwrap_or_else(|| panic!("{} {name}", p.id))
        .centre_hz
        / 1e6
}

#[test]
fn tv_channels_match_the_published_tables() {
    // ACMA, Television Channel Numbers and Frequency Limits.
    let au = plan("tv_au");
    let ch12 = au.find("12").unwrap();
    assert_eq!((ch12.centre_hz, ch12.width_hz), (226.5e6, 7e6));
    assert_eq!(mhz(au, "9A"), 205.5);
    assert_eq!(mhz(au, "6"), 177.5);
    assert_eq!(mhz(au, "28"), 529.5);
    assert_eq!(mhz(au, "51"), 690.5);
    assert!(au.find("52").unwrap().reallocated && !au.find("51").unwrap().reallocated);
    assert!(au.channels.iter().all(|c| c.width_hz == 7e6));
    assert_eq!(au.modes, ["dvbt", "dvb_terrestrial"]);
    // GE06 raster.
    let eu = plan("tv_eu");
    assert_eq!(mhz(eu, "21"), 474.0);
    assert_eq!(mhz(eu, "48"), 690.0);
    assert_eq!(mhz(eu, "69"), 858.0);
    assert!(eu.find("49").unwrap().reallocated && !eu.find("48").unwrap().reallocated);
    assert_eq!(eu.channel_at(690.4e6, None).unwrap().name, "48");
    assert_eq!(mhz(eu, "E5"), 177.5);
    assert_eq!(mhz(eu, "E12"), 226.5);
    assert_eq!(eu.step(690e6, Direction::Up, false).unwrap().1, "E5");
    assert_eq!(eu.step(690e6, Direction::Up, true).unwrap().1, "49");
    assert!(plan("tv_gb").find("38").unwrap().reallocated);
    // 47 CFR 73.603(a).
    let us = plan("tv_us");
    assert_eq!(mhz(us, "2"), 57.0);
    assert_eq!(mhz(us, "6"), 85.0);
    assert_eq!(mhz(us, "7"), 177.0);
    assert_eq!(mhz(us, "36"), 605.0);
    assert!(us.find("37").unwrap().reallocated);
    assert!(plan("tv_ca").find("36").unwrap().reallocated);
    // MIC plan: 473.142857 + 6i MHz is channel 13 + i.
    let jp = plan("tv_jp");
    assert!((mhz(jp, "13") - 473.142857).abs() < 1e-5);
    assert!((mhz(jp, "27") - 557.142857).abs() < 1e-5);
    assert!((mhz(jp, "52") - 707.142857).abs() < 1e-5);
    let br = plan("tv_isdbtb");
    assert!((mhz(br, "14") - 473.142857).abs() < 1e-5);
    assert!((mhz(br, "7") - 177.142857).abs() < 1e-5);
    let nz = plan("tv_nz");
    assert_eq!(mhz(nz, "25"), 506.0);
    assert!(nz.find("21").is_none());
    let cn = plan("tv_cn");
    assert_eq!(mhz(cn, "DS1"), 52.5);
    assert_eq!(mhz(cn, "DS6"), 171.0);
    assert_eq!(mhz(cn, "DS24"), 562.0);
    assert_eq!(mhz(cn, "DS25"), 610.0);
    assert_eq!(mhz(cn, "ds-13"), 474.0);
    assert!(cn.channel_at(585e6, None).is_none());
    let ce = plan("cable_eu");
    assert_eq!(mhz(ce, "S11"), 233.5);
    assert_eq!(mhz(ce, "s 41"), 466.0);
    assert_eq!(mhz(ce, "21"), 474.0);
    let cu = plan("cable_us");
    assert_eq!(mhz(cu, "95"), 93.0);
    assert_eq!(mhz(cu, "158"), 999.0);
}

#[test]
fn analogue_carriers_follow_bt1701() {
    let i = plan("an_i").place(474e6).unwrap();
    assert_eq!(i.vision_hz, Some(471.25e6));
    assert!((i.sound_hz.unwrap() - 477.2496e6).abs() < 1.0);
    let m = plan("an_m").place(57e6).unwrap();
    assert_eq!((m.vision_hz, m.sound_hz), (Some(55.25e6), Some(59.75e6)));
    assert!(plan("an_l").analog.as_ref().unwrap().am_sound);
}

#[test]
fn sound_broadcasting_rasters() {
    let na = plan("fm_north_america");
    let pts = na.tune_points(false);
    assert_eq!(
        (pts[0], *pts.last().unwrap(), pts.len()),
        (88.1e6, 107.9e6, 100)
    );
    assert_eq!(plan("fm_europe").tune_points(false).len(), 206);
    let au = plan("fm_australia_nz").tune_points(false);
    assert_eq!((au[0], *au.last().unwrap()), (87.7e6, 107.9e6));
    let jp = plan("fm_japan").tune_points(false);
    assert_eq!((jp[0], *jp.last().unwrap()), (76.1e6, 98.9e6));
    assert_eq!(plan("fm_oirt").segments[0].snap(69.0e6), 68.99e6);
    // GE06 Table A.3.1-15 and WI95revCO07 Table 2.
    let b3 = plan("dab_band3");
    assert_eq!(mhz(b3, "5A"), 174.928);
    assert_eq!(mhz(b3, "12B"), 225.648);
    assert_eq!(mhz(b3, "13F"), 239.2);
    assert_eq!(b3.channel_at(225.69e6, Some(50e3)).unwrap().name, "12B");
    assert!((mhz(plan("dab_band3_lband"), "LW") - 1490.624).abs() < 1e-6);
    let kr = plan("dab_tdmb_kr");
    assert_eq!(kr.channels.len(), 21);
    assert!((mhz(kr, "13C") - 214.736).abs() < 1e-6);
    // GE75 and Rio 1981.
    let r1 = plan("bc_r1").place(1010e3).unwrap();
    assert_eq!((r1.centre_hz, r1.name.as_str()), (1008e3, "MW 1008 kHz"));
    assert_eq!(
        plan("bc_r1").place(200e3).unwrap().segment.as_deref(),
        Some("LW")
    );
    let r2 = plan("bc_r2");
    assert_eq!(r2.place(1012e3).unwrap().centre_hz, 1010e3);
    assert_eq!(
        r2.segments
            .iter()
            .find(|s| s.name == "MW")
            .unwrap()
            .first_hz,
        Some(540e3)
    );
    assert!(r2
        .segments
        .iter()
        .all(|s| s.name != "LW" && s.name != "75 m"));
}

#[test]
fn amateur_bands_follow_article_5() {
    let edge = |id: &str, band: &str| {
        let s = plan(id).segments.iter().find(|s| s.name == band).unwrap();
        (s.lo_hz / 1e3, s.hi_hz / 1e3)
    };
    assert_eq!(edge("ham_r1", "160 m"), (1810.0, 1850.0));
    assert_eq!(edge("ham_r2", "40 m"), (7000.0, 7300.0));
    assert_eq!(edge("ham_r2", "70 cm"), (430000.0, 440000.0));
    assert_eq!(edge("ham_r1", "6 m"), (50000.0, 52000.0));
    assert!(plan("ham_r1")
        .segments
        .iter()
        .all(|s| s.name != "1.25 m" && s.name != "4 m"));
    assert_eq!(plan("ham_r3").place(7.1e6).unwrap().modulation, "lsb");
    assert!(plan("ham_r1").place(7.25e6).is_none());
}

#[test]
fn australian_amateur_bands_name_the_beacons_for_morse() {
    let text = std::fs::read_to_string(root().join("../../../data/plans/amateur.json")).unwrap();
    let file = data::parse("amateur.json", &text).unwrap();
    let au = file.groups.iter().find(|g| g.id == "ham_au").unwrap();
    let starts: Vec<f64> = au.bands.iter().map(|b| b.start_hz).collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]), "bands in order of frequency");
    let beacons = au
        .bands
        .iter()
        .find(|b| b.name.contains("beacon") && b.start_hz <= 432.4e6 && 432.4e6 <= b.end_hz)
        .expect("a beacon band that holds 432.4 MHz");
    assert!(beacons.modes.iter().any(|m| m == "cw"));
    let cm70 = au.bands.iter().find(|b| b.name == "70 cm").unwrap();
    assert_eq!((cm70.start_hz, cm70.end_hz), (430e6, 450e6));
}

#[test]
fn channelised_services() {
    // RR Appendix 18.
    let m = plan("marine_intl");
    assert_eq!(mhz(m, "16"), 156.8);
    assert_eq!(mhz(m, "70"), 156.525);
    assert_eq!(mhz(m, "88"), 157.425);
    let c1 = m.find("1").unwrap();
    assert_eq!((c1.centre_hz, c1.pair_hz), (156.05e6, Some(160.65e6)));
    assert!(
        m.find("27").unwrap().pair_hz.is_none(),
        "27 is simplex since WRC-19"
    );
    assert_eq!(mhz(m, "AIS2"), 162.025);
    let us = plan("marine_us");
    assert_eq!(mhz(us, "22A"), 157.1);
    assert_eq!(mhz(us, "WX1"), 162.55);
    // ICAO Doc 9718 Table 2-4.
    assert_eq!(icao_833_name(118.0e6), "118.005");
    assert_eq!(icao_833_name(118.008333e6), "118.010");
    assert_eq!(icao_833_name(118.016667e6), "118.015");
    assert_eq!(icao_833_name(132.025e6), "132.030");
    let a = plan("air_833");
    assert_eq!(a.channels.len(), 2280 + 4);
    assert!((a.find("132.035").unwrap().centre_hz - 132.0333333e6).abs() < 1.0);
    let a25 = plan("air_25");
    assert_eq!(a25.place(121.51e6).unwrap().name, "121.500");
    assert_eq!(a25.place(127.81e6).unwrap().centre_hz, 127.8e6);
    assert_eq!(a25.place(8.9e6).unwrap().modulation, "usb");
    // ECC/DEC/(15)05, 47 CFR 95, the class licences.
    assert_eq!(mhz(plan("pmr446"), "PMR 1"), 446.00625);
    assert_eq!(mhz(plan("pmr446"), "pmr16"), 446.19375);
    assert_eq!(
        plan("dpmr446").channels.last().unwrap().centre_hz,
        446.196875e6
    );
    assert_eq!(mhz(plan("frs_gmrs"), "FRS 8"), 467.5625);
    assert_eq!(mhz(plan("frs_gmrs"), "FRS 22"), 462.725);
    assert_eq!(mhz(plan("cb40"), "CB 19"), 27.185);
    assert_eq!(mhz(plan("cb40"), "CB 23"), 27.255);
    assert_eq!(mhz(plan("cb80_de"), "CB 80"), 26.955);
    assert_eq!(mhz(plan("cb_uk"), "UK 40"), 27.99125);
    let uhf = plan("uhf_cb");
    assert_eq!(mhz(uhf, "UHF 40"), 477.4);
    assert_eq!(mhz(uhf, "UHF 41"), 476.4375);
    assert_eq!(uhf.find("UHF 1").unwrap().pair_hz, Some(477.175e6));
    assert!(uhf.find("UHF 62").is_none());
    assert_eq!(mhz(plan("lpd433"), "LPD 69"), 434.775);
    assert_eq!(mhz(plan("cb_jp"), "8"), 27.144);
}

// ---- Regions ----

#[test]
fn names_and_typed_channels() {
    let plans = store();
    let de = ["pmr446", "tv_eu_uhf", "ham_r1", "dab_band3", "cable_eu", "marine_intl"];
    for id in de {
        assert!(plans.get(id).is_some(), "{id}");
    }
    assert_eq!(plans.name_of(446.08125e6, &de).unwrap().name, "PMR 7");
    assert_eq!(plans.name_of(474.1e6, &["tv_eu_uhf"]).unwrap().name, "21");
    assert_eq!(plans.name_of(7.1e6, &de).unwrap().name, "40 m");
    assert_eq!(plans.name_of(225.648e6, &["dab_band3"]).unwrap().name, "12B");
    assert!(plans.name_of(474.1e6, &["no_such_plan"]).is_none());
    let p = plans.parse_channel("9A", &["tv_au"]).unwrap();
    assert_eq!((p.plan.as_str(), p.centre_hz), ("tv_au", 205.5e6));
    assert_eq!(
        plans.parse_channel("9A", &["dab_band3"]).unwrap().centre_hz,
        202.928e6
    );
    assert_eq!(
        plans.parse_channel("uhf 11", &["uhf_cb"]).unwrap().centre_hz,
        476.675e6
    );
    // The first plan of the list that has the name decides.
    assert_eq!(
        plans.parse_channel("9A", &["dab_band3", "tv_au"]).unwrap().centre_hz,
        202.928e6
    );
    assert_eq!(
        plans.parse_channel("ch 16", &["marine_intl"]).unwrap().centre_hz,
        156.8e6
    );
    assert!(plans.parse_channel("ZZ9", &de).is_none());
}

// ---- User data ----

#[test]
fn a_user_file_replaces_a_builtin_group_with_the_same_id() {
    let mut plans = store();
    let text = r#"{"format":1,"groups":[{"id":"tv_au","name":"My Australian TV","kind":"tv","verified":false,
        "unverified_reason":"mine","source":{"publisher":"me","title":"notes","date":"2026"},
        "bands":[{"name":"VHF","service":"broadcasting","start_hz":222e6,"end_hz":231e6,"modes":["dvbt"],
                  "channel_width_hz":7e6,"channels":[{"name":"12","centre_hz":226.625e6}]}]}]}"#;
    let ids = api::import_as(&mut plans, text, PlanFormat::Auto, "", "").unwrap();
    assert_eq!(ids, ["tv_au"]);
    let tv = plans.get("tv_au").unwrap();
    assert_eq!(
        (tv.name.as_str(), tv.review),
        ("My Australian TV", Review::User)
    );
    assert_eq!(tv.find("12").unwrap().centre_hz, 226.625e6);
    assert_eq!(plans.all().filter(|p| p.id == "tv_au").count(), 1);
    assert_eq!(tv.countries, ["AU"]);
    // The saved user part reads back the same.
    let saved = api::user_plans_json(&plans);
    let mut again = store();
    api::import_as(&mut again, &saved, PlanFormat::Json, "", "").unwrap();
    assert_eq!(again.get("tv_au").unwrap().name, "My Australian TV");
    // Removing it brings the built-in group back.
    assert!(plans.remove_user("tv_au"));
    assert_eq!(
        plans
            .get("tv_au")
            .unwrap()
            .find("12")
            .unwrap()
            .centre_hz,
        226.5e6
    );
    assert!(!plans.remove_user("tv_eu"));
}

#[test]
fn a_user_file_adds_countries_presets_and_includes() {
    let mut plans = store();
    let text = r#"{"format":1,
        "groups":[{"id":"my_rep","name":"Repeaters","kind":"other","verified":false,"unverified_reason":"mine",
                   "source":{"publisher":"me","title":"club list","date":"2026"},"include":["pmr446"],
                   "bands":[{"name":"2 m","service":"amateur","start_hz":145.5e6,"end_hz":145.8e6,"modes":["general"],
                             "channel_width_hz":12.5e3,"channels":[{"name":"R1","centre_hz":145.6e6}]}]}],
        "countries":[{"code":"DE","groups":["my_rep"],"presets":{"tv_standard":"dvbt2"},"unverified_reason":"mine"}]}"#;
    let f = data::parse("mine.json", text).unwrap();
    plans.add_file("mine.json", f).unwrap();
    // The user's row of the country comes before the loaded ones.
    let de = plans.country_rows().find(|c| c.code == "DE").unwrap();
    assert_eq!(de.presets.tv_standard.as_deref(), Some("dvbt2"));
    assert_eq!(de.groups, ["my_rep"]);
    let mine = plans.get("my_rep").unwrap();
    assert_eq!(
        mine.channels.len(),
        17,
        "16 PMR446 channels and one repeater"
    );
    assert_eq!(mine.kind, PlanKind::Other);
    // A bad file adds nothing and names the path.
    let bad =
        r#"{"format":1,"countries":[{"code":"DE","groups":["nope"],"unverified_reason":"x"}]}"#;
    let err = plans
        .add_file("bad.json", data::parse("bad.json", bad).unwrap())
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("bad.json: $.countries[0].groups[0]"),
        "{err}"
    );
}

#[test]
fn user_plans_in_the_plan_view() {
    let mut plans = store();
    let mut p = ChannelPlan::new("mine", "My repeaters", PlanKind::Other, Review::User, "");
    p.countries = vec!["de".into()];
    p.channels.push(Channel::new("R2", 145.65e6, 12.5e3));
    p.channels.push(Channel::new("R1", 145.6e6, 12.5e3));
    plans.add_user(p.clone()).unwrap();
    assert_eq!(plans.get("mine").unwrap().channels[0].name, "R1");
    assert_eq!(plans.get("mine").unwrap().countries, vec!["DE"]);
    let mut bad = p.clone();
    bad.id = "bad id".into();
    bad.channels.push(Channel::new("", -1.0, 0.0));
    bad.segments.push(Segment::new("x", 2e6, 1e6));
    let Err(PlanError::Invalid(errs)) = plans.add_user(bad) else {
        panic!()
    };
    assert_eq!(errs.len(), 5, "{errs:?}");
    // CSV round trip of a built-in plan into a user plan.
    let csv = api::export(&plans, "lpd433", "csv")
        .unwrap()
        .replace("# id: lpd433", "# id: my_lpd");
    assert_eq!(
        api::import(&mut plans, &csv, "", "", "").unwrap(),
        ["my_lpd"]
    );
    assert_eq!(plans.get("my_lpd").unwrap().channels.len(), 69);
    let json = api::export_as(&plans, "tv_au", PlanFormat::Json).unwrap();
    assert!(api::is_plan_file(&json));
}

/// The lower and upper edge of a channel, MHz.
fn edges(p: &ChannelPlan, name: &str) -> (f64, f64) {
    let c = p.find(name).unwrap_or_else(|| panic!("{} {name}", p.id));
    (c.lower_hz() / 1e6, c.upper_hz() / 1e6)
}

#[test]
fn us_channels_above_36_are_listed_and_out_of_use() {
    // 47 CFR 73.603(a) and (c): channel 37 is 608-614 MHz, kept for radio astronomy.
    let us = plan("tv_us");
    assert_eq!(edges(us, "36"), (602.0, 608.0));
    assert_eq!(edges(us, "37"), (608.0, 614.0));
    // FCC 14-50 and 47 CFR 27.5(l): channels 38 to 51 (614-698 MHz) are the 600 MHz mobile band.
    assert_eq!(edges(us, "38"), (614.0, 620.0));
    assert_eq!(edges(us, "51"), (692.0, 698.0));
    // 47 CFR 27.5(b), (c): channels 52 to 69 (698-806 MHz) are the 700 MHz mobile band.
    assert_eq!(edges(us, "52"), (698.0, 704.0));
    assert_eq!(edges(us, "69"), (800.0, 806.0));
    assert!(!us.find("36").unwrap().reallocated);
    for n in 37..=69 {
        assert!(us.find(&n.to_string()).unwrap().reallocated, "{n}");
    }
    assert_eq!(*us.tune_points(false).last().unwrap(), 605e6);
    assert_eq!(*us.tune_points(true).last().unwrap(), 803e6);
}

#[test]
fn japan_vhf_channels_follow_the_mic_plan() {
    // ITE channel table and the MIC plan: 1-3 from 90 MHz, 4-7 from 170 MHz, 8-12 from 192 MHz, all 6 MHz.
    let jp = plan("tv_jp");
    let want = [
        ("1", 90.0),
        ("2", 96.0),
        ("3", 102.0),
        ("4", 170.0),
        ("5", 176.0),
        ("6", 182.0),
        ("7", 188.0),
        ("8", 192.0),
        ("9", 198.0),
        ("10", 204.0),
        ("11", 210.0),
        ("12", 216.0),
    ];
    for (name, lower) in want {
        assert_eq!(edges(jp, name), (lower, lower + 6.0), "{name}");
        assert!(jp.find(name).unwrap().reallocated, "{name}");
    }
    // Channels 7 and 8 overlap from 192 to 194 MHz; 193 MHz is nearer the centre of 8.
    assert_eq!(jp.channel_at(193.5e6, None).unwrap().name, "8");
    // No channel between 3 and 4.
    assert!(jp.channel_at(140e6, None).is_none());
    // UHF channel 13 keeps the 1/7 MHz ISDB-T offset and stays in use.
    assert!(!jp.find("13").unwrap().reallocated);
    // The analogue NTSC-J plan has the same VHF centres.
    let an = plan("an_j");
    for (name, _) in want {
        assert_eq!(
            jp.find(name).unwrap().centre_hz,
            an.find(name).unwrap().centre_hz,
            "{name}"
        );
    }
}

/// The segment named `name` of a plan.
fn segment<'a>(p: &'a ChannelPlan, name: &str) -> &'a Segment {
    p.segments
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{} {name}", p.id))
}

/// The stacks of the band of `group` named `band`.
fn band_modes(group: &str, band: &str) -> Vec<String> {
    let plans = store();
    let g = plans.group(group).unwrap_or_else(|| panic!("{group}"));
    g.group
        .bands
        .iter()
        .find(|b| b.name == band)
        .unwrap_or_else(|| panic!("{group} {band}"))
        .modes
        .clone()
}

#[test]
fn drm_plus_bands_follow_es_201_980() {
    // ES 201 980 introduction: Band I 47-68 MHz, Band II 87.5-107.9 MHz, Band III 174-240 MHz; clause 4.4.1: 100 kHz raster.
    let p = plan("drm_vhf");
    assert_eq!(p.modes, ["drm"]);
    for (name, lo, hi, first) in [
        ("VHF I", 47e6, 68e6, 47.1e6),
        ("VHF II", 87.5e6, 107.9e6, 87.6e6),
        ("VHF III", 174e6, 240e6, 174.1e6),
    ] {
        let s = segment(p, name);
        assert_eq!(
            (s.lo_hz, s.hi_hz, s.step_hz),
            (lo, hi, Some(100e3)),
            "{name}"
        );
        assert_eq!(s.raster_points().first(), Some(&first), "{name}");
    }
    assert_eq!(segment(p, "VHF III").snap(200.04e6), 200e6);
    assert!(p.segment_at(150e6).is_none());
}

#[test]
fn broadcast_modes_are_named_on_their_bands() {
    // DRM30 in every LF, MF and HF broadcasting band (ES 201 980 introduction).
    for g in ["bc_r1", "bc_r2", "bc_r3"] {
        for s in &plan(g).segments {
            assert!(
                band_modes(g, &s.name).iter().any(|m| m == "drm"),
                "{g} {}",
                s.name
            );
        }
        assert!(band_modes(g, "MW").iter().any(|m| m == "cquam"), "{g}");
        assert_eq!(band_modes(g, "MW")[0], "general", "{g}");
    }
    // HD Radio on AM and FM in the United States (47 CFR 73.404), not on the 9 kHz medium wave.
    assert!(band_modes("bc_r2", "MW").iter().any(|m| m == "hdradio"));
    assert!(!band_modes("bc_r1", "MW").iter().any(|m| m == "hdradio"));
    assert_eq!(band_modes("fm_north_america", "FM"), ["fm", "hdradio"]);
    // CDR in the Chinese FM band (ITU-R BS.1114-13 Annex 6, GY/T 268.1-2013).
    assert_eq!(band_modes("fm_china", "FM"), ["fm", "cdr"]);
    // DRM+ in Band II, the OIRT band and the Japanese FM band (ES 201 980 introduction).
    for g in ["fm_europe", "fm_oirt", "fm_japan"] {
        assert_eq!(band_modes(g, "FM"), ["fm", "drm"], "{g}");
    }
    // The plan view lists the stacks, the general receiver first.
    assert_eq!(plan("bc_r2").modes[0], "general");
    assert!(plan("fm_china").modes.iter().any(|m| m == "cdr"));
}

#[test]
fn analogue_tv_groups_of_countries_that_ended_analogue_tv() {
    for (cc, id) in [
        ("GB", "an_i"),
        ("JP", "an_j"),
        ("AU", "an_au"),
        ("NZ", "an_nz"),
        ("DE", "an_bg"),
        ("US", "an_m"),
        ("CA", "an_m"),
    ] {
        assert!(
            store()
                .country_rows()
                .any(|c| c.code == cc && c.groups.iter().any(|g| g == id)),
            "{cc}"
        );
    }
    let nz = store();
    let nz = nz.get("an_nz").unwrap();
    // Vision carrier 45.25 MHz (channel 1), 224.25 MHz (channel 11) and 519.25 MHz (channel 27).
    for (name, vision_hz) in [
        ("1", 45.25e6),
        ("4", 175.25e6),
        ("11", 224.25e6),
        ("27", 519.25e6),
    ] {
        let c = nz.find(name).unwrap();
        assert_eq!(c.lower_hz() + 1.25e6, vision_hz, "{name}");
    }
}

#[test]
fn fpv_channels_from_the_makers_charts() {
    let plans = store();
    let fpv = plans.get("fpv_video").unwrap();
    assert_eq!(fpv.channels.len(), 56);
    for (name, mhz) in [
        ("R1", 5658.0),
        ("R8", 5917.0),
        ("A1", 5865.0),
        ("B1", 5733.0),
        ("E5", 5885.0),
        ("F8", 5880.0),
        ("L1", 5362.0),
        ("G4", 1200.0),
    ] {
        assert_eq!(fpv.find(name).unwrap().centre_hz, mhz * 1e6, "{name}");
    }
}

#[test]
fn csv_round_trip_of_every_loaded_plan() {
    for p in store().all() {
        let text = io::to_csv(p);
        let back = io::from_csv(&text, "ignored", "ignored").unwrap();
        assert_eq!(back.id, p.id);
        assert_eq!(back.kind, p.kind);
        assert_eq!(back.countries, p.countries);
        assert_eq!(back.channels.len(), p.channels.len(), "{}", p.id);
        assert_eq!(back.segments.len(), p.segments.len(), "{}", p.id);
        for (a, b) in back.channels.iter().zip(&p.channels) {
            assert!(
                (a.centre_hz - b.centre_hz).abs() < 1.0,
                "{} {}",
                p.id,
                a.name
            );
            assert_eq!(
                (&a.name, a.reallocated, a.pair_hz.is_some()),
                (&b.name, b.reallocated, b.pair_hz.is_some())
            );
        }
        assert_eq!(back.deemphasis_s.is_some(), p.deemphasis_s.is_some());
        for (a, b) in back.segments.iter().zip(&p.segments) {
            assert_eq!(
                a.raster_points().len(),
                b.raster_points().len(),
                "{} {}",
                p.id,
                a.name
            );
            assert_eq!(a.modulation, b.modulation);
        }
    }
}

#[test]
fn json_import_into_the_registry() {
    let mut plans = store();
    let text = io::to_json(&[plans.get("pmr446").unwrap()]);
    let mut v = io::from_json(&text).unwrap();
    v[0].id = "my_pmr".into();
    plans.add_user(v.remove(0)).unwrap();
    assert_eq!(plans.get("my_pmr").unwrap().channels.len(), 16);
    let one = r#"{"id":"a","name":"A","kind":"other","review":"user","channels":[{"name":"1","centre_hz":1e6,"width_hz":1e4}]}"#;
    assert_eq!(io::from_json(one).unwrap().len(), 1);
    assert!(io::from_json("{").is_err());
}

#[test]
fn an_empty_store_is_valid_and_holds_no_row() {
    let plans = Plans::new();
    assert!(plans.is_empty());
    assert_eq!(plans.country_rows().count() + plans.region_rows().count(), 0);
    assert!(plans.parse_channel("21", &["tv_au"]).is_none());
    assert!(plans.places(474e6, &["tv_au"]).is_empty());
}

#[test]
fn a_store_reads_the_plan_files_of_a_storage_folder() {
    let storage = sdrkit_core::MemStorage::new();
    let folder = sdrkit_core::StorePath::new("plans").unwrap();
    let none = Plans::from_storage(&storage, &folder, &[]).unwrap();
    assert!(none.is_empty());
    for (name, text) in sdrkit_dev_test::plan_files() {
        let path = sdrkit_core::StorePath::new(&format!("plans/{name}")).unwrap();
        sdrkit_core::Storage::write(&storage, &path, text.as_bytes()).unwrap();
    }
    let path = sdrkit_core::StorePath::new("plans/user.json").unwrap();
    sdrkit_core::Storage::write(&storage, &path, b"not a plan file").unwrap();
    let plans = Plans::from_storage(&storage, &folder, &["user.json"]).unwrap();
    let repo = sdrkit_dev_test::plans();
    assert_eq!(
        plans.all().map(|p| p.id.clone()).collect::<Vec<_>>(),
        repo.all().map(|p| p.id.clone()).collect::<Vec<_>>()
    );
    assert_eq!(
        plans.country_rows().collect::<Vec<_>>(),
        repo.country_rows().collect::<Vec<_>>()
    );
    assert!(matches!(
        Plans::from_storage(&storage, &folder, &[]),
        Err(PlanError::Parse { .. })
    ));
}

#[test]
fn the_dab_blocks_of_the_standard_match_the_dab_plan_file() {
    let plans = store();
    let band3: Vec<(String, f64)> = plans
        .get("dab_band3")
        .unwrap()
        .channels
        .iter()
        .map(|c| (c.name.clone(), c.centre_hz))
        .collect();
    let blocks: Vec<(String, f64)> = dab::BAND_III
        .iter()
        .map(|b| (b.name.to_owned(), b.centre_hz))
        .collect();
    assert_eq!(band3, blocks);
    let l: Vec<(String, f64)> = plans
        .get("dab_band3_lband")
        .unwrap()
        .channels
        .iter()
        .filter(|c| c.centre_hz >= 1e9)
        .map(|c| (c.name.clone(), c.centre_hz))
        .collect();
    let blocks: Vec<(String, f64)> = dab::L_BAND
        .iter()
        .map(|b| (b.name.to_owned(), b.centre_hz))
        .collect();
    assert_eq!(l, blocks);
}

#[test]
fn the_cellular_bands_of_the_standards_match_the_cellular_plan_file() {
    let plans = store();
    let mut from_file = Vec::new();
    for id in ["cell_gsm", "cell_lte", "cell_nr"] {
        for b in &plans.group(id).unwrap().group.bands {
            let c = b.cellular.as_ref().unwrap();
            from_file.push((
                b.name.clone(),
                b.start_hz,
                b.end_hz,
                c.band,
                c.numbers.clone(),
                c.sync.clone(),
            ));
        }
    }
    let table: Vec<_> = cellular::cell_bands()
        .iter()
        .map(|b| {
            (
                b.name.to_owned(),
                b.dl_low_hz,
                b.dl_high_hz,
                b.info.band,
                b.info.numbers.to_vec(),
                b.info.sync.to_vec(),
            )
        })
        .collect();
    assert_eq!(from_file, table);
}
