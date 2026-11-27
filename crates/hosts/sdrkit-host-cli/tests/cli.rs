//! Each command of the tool, run in the process on an engine with storage in memory.

use sdrkit_cli::{run, Io};
use sdrkit_core::MemStorage;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

/// Runs `args` and returns the exit code, the output and the error text.
fn sdrkit_with(args: &[&str], lines: Option<Receiver<String>>) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let argv = std::iter::once("sdrkit")
        .chain(args.iter().copied())
        .map(Into::into);
    let code = run(
        argv,
        Some(Arc::new(MemStorage::new())),
        Io {
            out: &mut out,
            err: &mut err,
            redraw: false,
            lines,
        },
    );
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

fn sdrkit(args: &[&str]) -> (u8, String, String) {
    sdrkit_with(args, None)
}

/// Each line of `out` as JSON.
fn json_lines(out: &str) -> Vec<serde_json::Value> {
    out.lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON: {l}: {e}")))
        .collect()
}

#[test]
fn stacks_lists_the_catalog_by_group() {
    let (code, out, err) = sdrkit(&["stacks"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.lines().next().is_some_and(|l| l.starts_with("Key")));
    assert!(out.lines().any(|l| l.starts_with("fm ")), "{out}");
}

#[test]
fn an_unknown_stack_lists_the_stacks() {
    let (code, _, err) = sdrkit(&["run", "nosuchstack", "--seconds", "0.1"]);
    assert_eq!(code, 2);
    assert!(err.contains("no stack has the id `nosuchstack`"), "{err}");
    assert!(err.lines().any(|l| l.starts_with("fm ")), "{err}");
}

#[test]
fn show_reads_a_catalog_table_as_json() {
    let (code, out, err) = sdrkit(&["show", "engine", "stacks", "--json"]);
    assert_eq!(code, 0, "{err}");
    let v = json_lines(&out);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0]["table"], "stacks");
    let rows = v[0]["rows"].as_array().expect("rows");
    assert!(rows.iter().any(|r| r[0] == "fm"));
}

#[test]
fn settings_of_a_stack_kind_show_key_kind_and_help() {
    let (code, out, err) = sdrkit(&["settings", "kind:stack:fm"]);
    assert_eq!(code, 0, "{err}");
    let head = out.lines().next().unwrap_or_default();
    for h in [
        "Key", "Label", "Kind", "Range", "Common", "Choices", "Value", "Help",
    ] {
        assert!(head.contains(h), "{head}");
    }
    assert!(out.lines().count() > 2, "{out}");
}

#[test]
fn settings_show_the_common_values_for_the_present_option() {
    let (code, out, err) = sdrkit(&["settings", "kind:stack:general"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.lines().next().unwrap_or_default().contains("Common"));
    // The kind has no values: the widths are those of its default demodulator, NFM.
    let width = out
        .lines()
        .find(|l| l.starts_with("width "))
        .unwrap_or_default();
    assert!(
        width.contains("12.5 kHz") && width.contains("25 kHz"),
        "{width}"
    );
}

#[test]
fn a_bad_value_names_the_forms() {
    let (code, _, err) = sdrkit(&["settings", "engine", "position=north"]);
    assert_eq!(code, 2);
    assert!(err.contains("LAT,LON"), "{err}");
}

#[test]
fn a_bad_choice_names_the_setting_by_its_label() {
    let choices = vec![
        sdrkit_core::ChoiceDecl::new("2.4", "2.4 GHz"),
        sdrkit_core::ChoiceDecl::new("5", "5 GHz"),
        sdrkit_core::ChoiceDecl::new("6", "6 GHz"),
    ];
    let decl = sdrkit_core::FieldDecl::with_choices("band", "Band", choices);
    let e = sdrkit_cli::text::parse_value(&decl, "7").unwrap_err();
    assert_eq!(e.to_string(), "7 is not one of 2.4, 5, 6 for setting \"Band\" (band)");
}

#[test]
fn call_runs_a_command_of_the_engine_and_prints_its_reply() {
    let (code, out, err) = sdrkit(&["call", "engine", "state.save", "--json"]);
    assert_eq!(code, 0, "{err}");
    let v = json_lines(&out);
    assert_eq!(v[0]["command"], "state.save");
    assert!(v[0]["reply"].is_object(), "{out}");
}

#[test]
fn call_reads_a_table_and_runs_a_command_of_a_service() {
    let (code, out, err) = sdrkit(&["show", "service:memory", "--json"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        json_lines(&out)[0]["results"]["tables"].is_object(),
        "{out}"
    );
    let (code, out, err) = sdrkit(&["call", "service:memory"]);
    assert_eq!(code, 0, "{err}");
    assert!(!out.is_empty());
}

#[test]
fn an_unknown_owner_lists_the_owners() {
    let (code, _, err) = sdrkit(&["show", "service:nothing"]);
    assert_eq!(code, 2);
    assert!(err.contains("service:memory"), "{err}");
}

#[test]
fn run_on_the_test_signal_prints_json_lines_and_takes_console_lines() {
    let (tx, rx) = std::sync::mpsc::channel();
    for line in [
        "show session",
        "call stack clear",
        "call service:memory add name=Test freq_hz=100.3M stack=fm",
        "show service:memory memories",
    ] {
        tx.send(line.to_owned()).expect("send");
    }
    let (code, out, err) = sdrkit_with(
        &["run", "fm", "--source", "synth", "--seconds", "1", "--json"],
        Some(rx),
    );
    assert_eq!(code, 0, "{err}");
    let v = json_lines(&out);
    assert!(v.iter().any(|j| j.get("status").is_some()), "{out}");
    assert!(
        v.iter().any(|j| j.get("owner") == Some(&"session".into())),
        "{out}"
    );
    assert!(v.iter().any(|j| j["command"] == "clear"), "{out}");
    let memories = v
        .iter()
        .find(|j| j["table"] == "memories")
        .expect("the memories table");
    assert!(memories["rows"].to_string().contains("Test"), "{memories}");
}

#[test]
fn transmit_on_the_test_signal_is_refused_and_the_run_goes_on() {
    let dir = std::env::temp_dir().join(format!("sdrkit-cli-tx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("folder");
    let wav = dir.join("tone.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&wav, spec).expect("wav");
    for i in 0..4800 {
        w.write_sample(0.25 * (i as f32 * 0.13).sin())
            .expect("sample");
    }
    w.finalize().expect("wav");
    let (tx, rx) = std::sync::mpsc::channel();
    for line in [
        "call session ptt on=true",
        "call session send text=CQ",
        "call session tune",
        "show session",
    ] {
        tx.send(line.to_owned()).expect("send");
    }
    let wav_arg = wav.display().to_string();
    let (code, out, err) = sdrkit_with(
        &[
            "run",
            "general",
            "--source",
            "synth",
            "--seconds",
            "1",
            "--json",
            "--tx-audio",
            &wav_arg,
        ],
        Some(rx),
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(code, 0, "{err}");
    // The test signal cannot transmit and the switch is off: each command that transmits is refused.
    assert_eq!(err.matches("transmit is off").count(), 3, "{err}");
    let v = json_lines(&out);
    let fields = v
        .iter()
        .find_map(|j| j["results"]["fields"].as_object().cloned())
        .expect("the fields of the session");
    let none = |key: &str| fields.get(key).is_none_or(|v| v["kind"] == "none");
    assert!(none("session.tx_sent"), "{out}");
    assert!(none("session.audio_in_rate_hz"), "{out}");
    assert_eq!(fields["session.audio_in_wanted"]["value"], false, "{out}");
}

#[test]
fn declarations_write_a_header_that_a_c_compiler_accepts() {
    let dir = std::env::temp_dir().join(format!("sdrkit-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("folder");
    let h = dir.join("sdrkit_keys.h");
    let path = h.to_string_lossy().into_owned();
    let (code, _, err) = sdrkit(&["declarations", "--c-header", "--out", &path]);
    assert_eq!(code, 0, "{err}");
    let text = std::fs::read_to_string(&h).expect("header");
    assert!(text.contains("#define SDRKIT_ENGINE_SETTING_CALLSIGN \"callsign\""));
    let cc = std::process::Command::new("cc")
        .args(["-fsyntax-only", "-x", "c"])
        .arg(&h)
        .status();
    if let Ok(status) = cc {
        assert!(status.success(), "cc refused {path}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn verify_passes_the_test_signal_of_one_stack() {
    let (code, out, err) = sdrkit(&["verify", "fm", "--seconds", "2", "--json"]);
    assert_eq!(code, 0, "{out}{err}");
    let v = json_lines(&out);
    assert!(
        v.iter().any(
            |j| j["check"].as_str().is_some_and(|c| c.ends_with("locks")) && j["outcome"] == "pass"
        ),
        "{out}"
    );
}

#[test]
fn numbers_take_a_unit() {
    use sdrkit_cli::text::{parse_hz, Bare};
    let ok = |t, b| parse_hz(t, b).expect(t);
    assert_eq!(ok("919.875M", Bare::Frequency), 919.875e6);
    assert_eq!(ok("919.875MHz", Bare::Frequency), 919.875e6);
    assert_eq!(ok("919875000", Bare::Frequency), 919.875e6);
    assert_eq!(ok("100.3", Bare::Frequency), 100.3e6);
    assert_eq!(ok("48k", Bare::Hz), 48e3);
    assert_eq!(ok("2.4M", Bare::Hz), 2.4e6);
    assert_eq!(ok("250000", Bare::Hz), 250e3);
    assert!(parse_hz("fast", Bare::Hz).is_err());
}

#[test]
fn cells_follow_their_unit() {
    use sdrkit_cli::text::{age, duration, hz};
    assert_eq!(hz(919.875e6), "919.875 MHz");
    assert_eq!(hz(250e3), "250 kHz");
    assert_eq!(duration(54.0), "54 s");
    assert_eq!(age(1000.0, 1120.0), "2 min ago");
}

/// The data folder of the tests of `sdrkit-presets`.
fn region_data() -> String {
    format!(
        "{}/../../helpers/sdrkit-presets/tests/fixture",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[test]
fn run_with_region_data_starts_on_the_station_of_the_city() {
    let data = region_data();
    let mut args = vec!["run", "dvbt", "--seconds", "1", "--json", "--region", "AU"];
    args.extend(["--data", &data, "--city", "Sydney"]);
    let (code, out, err) = sdrkit(&args);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        err.contains("region data AU/Sydney: 226.5 MHz, 7 MHz wide, station ABC Sydney"),
        "{err}"
    );
    // A frequency and a setting of the command line replace those of the region data.
    args.extend(["--freq", "529.5", "--set", "bandwidth=8"]);
    let (code, out, err) = sdrkit(&args);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("region data AU/Sydney: 529.5 MHz"), "{err}");
    assert!(!err.contains("station"), "{err}");
}

#[test]
fn run_without_region_data_says_so_and_starts() {
    let nowhere = format!("{}/nowhere", region_data());
    let args = ["run", "dvbt", "--seconds", "1", "--json", "--region", "AU"];
    let (code, out, err) = sdrkit(&[&args[..], &["--data", &nowhere]].concat());
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        err.contains(&format!("no region data loaded: {nowhere}")),
        "{err}"
    );
    // Without `--data`, a storage that is not a folder has no region data and no note.
    let (code, out, err) = sdrkit(&args);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("region data"), "{err}");
}

#[test]
fn a_region_that_is_not_a_country_code_is_refused() {
    let (code, _, err) = sdrkit(&["stacks", "--region", "itu2"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--region") && err.contains("itu2"), "{err}");
}

#[test]
fn a_region_that_the_data_has_no_plans_for_is_refused_with_the_list() {
    let data = format!("{}/../../../data", env!("CARGO_MANIFEST_DIR"));
    let args = ["run", "dvbt", "--seconds", "1", "--json", "--region", "XX"];
    let (code, _, err) = sdrkit(&[&args[..], &["--data", &data]].concat());
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no plans for `XX`"), "{err}");
    assert!(err.lines().any(|l| l == "AU  Australia"), "{err}");
    assert!(err.lines().any(|l| l.starts_with("DE  ")), "{err}");
}

#[test]
fn a_rate_below_the_least_rate_of_a_stack_is_refused_with_both_numbers() {
    let (code, _, err) = sdrkit(&[
        "run",
        "aeromacs",
        "--source",
        "synth:aeromacs",
        "--rate",
        "2M",
        "--seconds",
        "1",
    ]);
    assert_ne!(code, 0, "{err}");
    assert!(err.contains("at least") && err.contains("2000000"), "{err}");
}

#[test]
fn a_valid_rate_is_the_rate_of_the_session() {
    let (code, out, err) = sdrkit(&[
        "run", "fm", "--source", "synth", "--rate", "5M", "--seconds", "1", "--json",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("5000000"), "{out}");
}

#[test]
fn a_wide_stack_starts_without_a_rate() {
    let (code, _, err) = sdrkit(&[
        "run",
        "aeromacs",
        "--source",
        "synth:aeromacs",
        "--seconds",
        "1",
    ]);
    assert_eq!(code, 0, "{err}");
}
