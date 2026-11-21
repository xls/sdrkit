//! Runs each example program on its test signal for a few seconds and checks its exit code and one line of its output.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The program of example `name`, built if it is missing.
fn example(name: &str) -> PathBuf {
    // target/<profile>/deps/run-<hash> -> target/<profile>/examples/<name>
    let exe = std::env::current_exe().expect("the path of the test");
    let profile = exe
        .parent()
        .and_then(Path::parent)
        .expect("the profile folder");
    let program = profile
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    if !program.exists() {
        let mut build = Command::new(env!("CARGO"));
        build.args(["build", "-q", "-p", "sdrkit-examples", "--example", name]);
        if profile.file_name().is_some_and(|n| n == "release") {
            build.arg("--release");
        }
        assert!(
            build.status().is_ok_and(|s| s.success()),
            "the example {name} did not build"
        );
    }
    program
}

/// Runs example `name` with `args` and returns its standard output. The run must succeed.
fn run(name: &str, args: &[&str]) -> String {
    let out = Command::new(example(name))
        .args(args)
        .current_dir(env!("CARGO_TARGET_TMPDIR"))
        .output()
        .expect("run the example");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{name}: {stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

#[test]
fn fm_audio_writes_audio_from_the_test_signal() {
    // The WAV file goes to the working folder of the run, the temporary folder of the tests.
    let out = run("fm_audio", &["synth:fm", "2"]);
    assert!(out.contains("s of audio to fm_audio.wav"), "{out}");
    let _ = std::fs::remove_file(Path::new(env!("CARGO_TARGET_TMPDIR")).join("fm_audio.wav"));
}

#[test]
fn meshtastic_hears_nodes_of_the_test_signal() {
    let out = run("meshtastic", &["synth:meshtastic", "15"]);
    let last = out.lines().last().unwrap_or("");
    assert!(
        last.ends_with("nodes heard") && !last.starts_with("0 "),
        "{out}"
    );
}

#[test]
fn meshtastic_starts_with_the_preset_of_a_country() {
    let out = run("meshtastic", &["synth:meshtastic", "1", "AU"]);
    assert!(
        out.lines()
            .last()
            .is_some_and(|l| l.ends_with("nodes heard")),
        "{out}"
    );
}

#[test]
fn any_stack_prints_the_tables_of_a_stack() {
    let out = run("any_stack", &["adsb", "synth:adsb", "2", "2"]);
    assert!(out.contains("table aircraft:"), "{out}");
}
