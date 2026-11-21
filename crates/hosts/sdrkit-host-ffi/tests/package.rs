//! The installed copy of the library and the examples against it. The test runs `tools/package.sh` for the host into
//! a temporary folder, builds `examples/` with CMake against it where `cmake` is on the path, builds one example with
//! a plain compiler line and one through pkg-config, and runs four examples on their test signals for two seconds each.
//! Skipped on Windows (`tools/package.ps1` serves it) and where there is no C compiler.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Runs `cmd` and returns its output, or `None` if the program does not exist.
fn try_run(cmd: &mut Command) -> Option<Output> {
    match cmd.output() {
        Ok(out) => Some(out),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("{cmd:?}: {e}"),
    }
}

/// Runs `cmd`, which must succeed, and returns its standard output.
fn run(what: &str, cmd: &mut Command) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{what}: {stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

/// Runs the example program `exe` with `args` in `dir` and checks that it succeeds and prints a line with `expect`.
fn check_example(exe: &Path, args: &[&str], dir: &Path, expect: &str) {
    let out = run(
        &exe.display().to_string(),
        Command::new(exe).args(args).current_dir(dir),
    );
    assert!(
        out.lines().any(|l| l.contains(expect)),
        "{}: no line with \"{expect}\" in:\n{out}",
        exe.display()
    );
}

#[test]
fn the_package_builds_the_examples_with_cmake_pkg_config_and_a_plain_line() {
    if cfg!(windows) {
        eprintln!("skipped: tools/package.ps1 makes the package on Windows");
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let tmp =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("package_{}", std::process::id()));
    let out_dir = tmp.join("dist");
    let built = run(
        "tools/package.sh",
        Command::new("sh")
            .arg(root.join("tools/package.sh"))
            .arg("")
            .arg(&out_dir),
    );
    let dist = PathBuf::from(
        built
            .lines()
            .last()
            .expect("the folder of the package")
            .trim(),
    );
    assert!(
        dist.join("include/sdrkit.h").is_file()
            && dist.join("lib/cmake/sdrkit/sdrkitConfig.cmake").is_file()
    );
    let lib = dist.join("lib");
    let examples = root.join("examples");
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());

    // The plain compiler line, with the dynamic library and its run path.
    let plain = tmp.join("fm_audio_plain");
    let Some(out) = try_run(
        Command::new(&cc)
            .arg("-std=c99")
            .arg(examples.join("c/fm_audio.c"))
            .arg(format!("-I{}", dist.join("include").display()))
            .arg(format!("-L{}", lib.display()))
            .arg("-lsdrkit")
            .arg(format!("-Wl,-rpath,{}", lib.display()))
            .arg("-o")
            .arg(&plain),
    ) else {
        eprintln!("skipped: no C compiler ({cc})");
        return;
    };
    assert!(
        out.status.success(),
        "the plain line: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // pkg-config with the static library and its system libraries.
    let pc_path = lib.join("pkgconfig");
    let flags = try_run(
        Command::new("pkg-config")
            .env("PKG_CONFIG_PATH", &pc_path)
            .args(["--cflags", "--libs", "sdrkit-static"]),
    );
    let static_exe = tmp.join("adsb_table_static");
    match flags {
        None => eprintln!("pkg-config is not on the path: the static build is not tested"),
        Some(f) => {
            assert!(
                f.status.success(),
                "pkg-config: {}",
                String::from_utf8_lossy(&f.stderr)
            );
            let flags = String::from_utf8_lossy(&f.stdout).into_owned();
            run(
                "the pkg-config build",
                Command::new(&cc)
                    .arg("-std=c99")
                    .arg(examples.join("c/adsb_table.c"))
                    .args(flags.split_whitespace())
                    .arg("-o")
                    .arg(&static_exe),
            );
        }
    }

    // CMake: every example except those that need SDL.
    let cmake_build = tmp.join("cmake");
    let configured = try_run(
        Command::new("cmake")
            .arg("-S")
            .arg(&examples)
            .arg("-B")
            .arg(&cmake_build)
            .arg(format!("-DCMAKE_PREFIX_PATH={}", dist.display()))
            .arg("-DCMAKE_BUILD_TYPE=Release"),
    );
    let cmake_ok = match configured {
        None => {
            eprintln!("skipped: cmake is not on the path");
            false
        }
        Some(out) => {
            assert!(
                out.status.success(),
                "cmake: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            run(
                "cmake --build",
                Command::new("cmake")
                    .arg("--build")
                    .arg(&cmake_build)
                    .args(["-j", "3"]),
            );
            true
        }
    };

    let program = |name: &str| {
        if cmake_ok {
            cmake_build.join(name)
        } else {
            tmp.join(name)
        }
    };
    if !cmake_ok {
        for name in ["list_stacks", "settings_save"] {
            run(
                name,
                Command::new(&cc)
                    .arg("-std=c99")
                    .arg(examples.join(format!("c/{name}.c")))
                    .arg(format!("-I{}", dist.join("include").display()))
                    .arg(format!("-L{}", lib.display()))
                    .arg("-lsdrkit")
                    .arg(format!("-Wl,-rpath,{}", lib.display()))
                    .arg("-o")
                    .arg(tmp.join(name)),
            );
        }
    }

    check_example(
        &plain,
        &["synth:fm", "2"],
        &tmp,
        "s of audio to fm_audio.wav",
    );
    check_example(&program("list_stacks"), &["fm"], &tmp, "1 of ");
    let adsb = if cmake_ok {
        program("adsb_table")
    } else {
        static_exe.clone()
    };
    if adsb.exists() {
        check_example(&adsb, &["synth:adsb", "2"], &tmp, "aircraft in the table");
    }
    if static_exe.exists() && static_exe != adsb {
        check_example(
            &static_exe,
            &["synth:adsb", "2"],
            &tmp,
            "aircraft in the table",
        );
    }
    check_example(
        &program("settings_save"),
        &["synth:fm"],
        &tmp,
        "imported the saved settings",
    );
    if cmake_ok {
        check_example(
            &program("fm_audio_cpp"),
            &["synth:fm", "2"],
            &tmp,
            "s of audio to",
        );
        // The static target sdrkit::sdrkit with its system libraries, and the typed retune of the session.
        let static_build = tmp.join("cmake_static");
        run(
            "cmake with the static library",
            Command::new("cmake")
                .arg("-S")
                .arg(&examples)
                .arg("-B")
                .arg(&static_build)
                .arg(format!("-DCMAKE_PREFIX_PATH={}", dist.display()))
                .arg("-DSDRKIT_EXAMPLES_STATIC=ON"),
        );
        run(
            "cmake --build with the static library",
            Command::new("cmake")
                .arg("--build")
                .arg(&static_build)
                .args(["--target", "fm_audio"]),
        );
        check_example(
            &static_build.join("fm_audio"),
            &["synth:fm", "2", "100"],
            &tmp,
            "s of audio to fm_audio.wav",
        );
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
