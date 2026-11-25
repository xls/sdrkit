//! Builds every `tests/c_*.c` program against `include/sdrkit.h` and the static library, and runs each with a new folder
//! for its engine. `tests/c_presets.c` is built and run only with the feature `presets`. A program prints `<name> ok` on its last line when every check passed. Skipped where there is no C
//! compiler or on Windows. The examples of `examples/` are compiled against the headers, and the Swift files with their
//! example are type-checked where there is a Swift compiler.

use std::path::{Path, PathBuf};
use std::process::Command;

fn native_libs() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &[
            "-framework",
            "Security",
            "-framework",
            "CoreFoundation",
            "-framework",
            "Accelerate",
            "-framework",
            "AudioToolbox",
            "-framework",
            "IOKit",
            "-liconv",
            "-lc",
            "-lm",
        ]
    } else if cfg!(target_os = "linux") {
        &[
            "-lgcc_s",
            "-lutil",
            "-lrt",
            "-lpthread",
            "-lm",
            "-ldl",
            "-lc",
        ]
    } else {
        &[]
    }
}

/// The C programs, by name (`c_engine` for `tests/c_engine.c`).
fn programs(crate_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(crate_dir.join("tests"))
        .expect("the tests folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_owned(), p)))
        .filter(|(n, _)| n.starts_with("c_"))
        .filter(|(n, _)| n != "c_presets" || cfg!(feature = "presets"))
        .collect();
    v.sort();
    v
}

/// The static library next to this test, built if it is missing or old.
fn static_lib() -> PathBuf {
    // target/<profile>/deps/c_api-<hash> -> target/<profile>/libsdrkit.a
    let exe = std::env::current_exe().expect("the test's path");
    let profile_dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("the profile folder");
    let mut b = Command::new(env!("CARGO"));
    b.args(["build", "-q", "-p", "sdrkit-host-ffi", "--lib"]);
    // The library holds the families that this test was built with.
    if !cfg!(feature = "all-families") {
        let on: Vec<&str> = sdrkit::families()
            .iter()
            .filter(|f| !f.object)
            .map(|f| f.key)
            .collect();
        b.args(["--no-default-features", "--features", &on.join(",")]);
    }
    if cfg!(feature = "presets") {
        b.args(["--features", "presets"]);
    }
    if profile_dir.file_name().is_some_and(|n| n == "release") {
        b.arg("--release");
    }
    assert!(
        b.status().is_ok_and(|s| s.success()),
        "the static library did not build"
    );
    profile_dir.join("libsdrkit.a")
}

#[test]
fn every_c_program_builds_against_the_header_and_runs() {
    if cfg!(windows) {
        eprintln!("skipped: the C programs run on Unix only");
        return;
    }
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib = static_lib();
    let list = programs(&crate_dir);
    assert!(!list.is_empty(), "no tests/c_*.c programs");
    for (name, src) in list {
        let tmp = std::env::temp_dir().join(format!("sdrkit_{name}_{}", std::process::id()));
        let Some(exe) = build_c(&crate_dir, &lib, &name, &src) else {
            return;
        };
        let run = Command::new(&exe)
            .arg(&tmp)
            .output()
            .expect("run the C program");
        let _ = std::fs::remove_file(&exe);
        let _ = std::fs::remove_dir_all(&tmp);
        let stdout = String::from_utf8_lossy(&run.stdout);
        assert!(
            run.status.success(),
            "{name}: {stdout}{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            stdout.lines().last().unwrap_or(""),
            format!("{name} ok"),
            "{name} did not finish"
        );
    }
}

/// The C and C++ examples of `examples/` compile against the headers of this crate as strict C99 and C++17 without a
/// warning: no variable-length array, no extension of a compiler. The programs that need SDL are left out. Skipped where
/// there is no C compiler. `tests/package.rs` builds and runs them against an installed copy of the library.
#[test]
fn every_example_compiles_against_the_headers() {
    if cfg!(windows) {
        return;
    }
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let examples = crate_dir.join("../../../examples");
    let include = crate_dir.join("include");
    let mut problems = Vec::new();
    for (folder, compiler, flags) in [
        (
            "c",
            std::env::var("CC").unwrap_or_else(|_| "cc".into()),
            &["-std=c99", "-pedantic", "-Wvla"][..],
        ),
        (
            "cpp",
            std::env::var("CXX").unwrap_or_else(|_| "c++".into()),
            &["-std=c++17", "-pedantic", "-Wno-missing-field-initializers"][..],
        ),
    ] {
        let mut sources: Vec<PathBuf> = std::fs::read_dir(examples.join(folder))
            .expect("the folder of the examples")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| !p.to_string_lossy().ends_with("_sdl.c"))
            .collect();
        sources.sort();
        assert!(!sources.is_empty(), "no examples in {folder}");
        for src in sources {
            let checked = Command::new(&compiler)
                .args(["-Wall", "-Wextra", "-Werror"])
                .args(flags)
                .args(["-fsyntax-only", "-I"])
                .arg(&include)
                .arg(&src)
                .output();
            match checked {
                Err(e) => {
                    eprintln!("skipped: no compiler ({compiler}: {e})");
                    return;
                }
                Ok(out) if !out.status.success() => problems.push(format!(
                    "{}:\n{}",
                    src.display(),
                    String::from_utf8_lossy(&out.stderr)
                )),
                Ok(_) => {}
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Every header of the library is valid C++17: the base header, the presets header and each written header of a
/// family, each compiled alone. A member or a parameter with a reserved word of C++ as its name fails here. Skipped where there is no C++
/// compiler.
#[test]
fn every_header_compiles_as_cpp() {
    if cfg!(windows) {
        return;
    }
    let include = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include");
    let mut headers = vec![include.join("sdrkit.h"), include.join("sdrkit_presets.h")];
    let mut families: Vec<PathBuf> = std::fs::read_dir(include.join("sdrkit"))
        .expect("the folder of the family headers")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "h"))
        .collect();
    families.sort();
    headers.extend(families);
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
    let mut problems = Vec::new();
    for header in &headers {
        let checked = Command::new(&cxx)
            .args(["-std=c++17", "-fsyntax-only", "-x", "c++", "-I"])
            .arg(&include)
            .arg(header)
            .output();
        match checked {
            Err(e) => {
                eprintln!("skipped: no C++ compiler ({cxx}: {e})");
                return;
            }
            Ok(out) if !out.status.success() => problems.push(format!(
                "{}:\n{}",
                header.display(),
                String::from_utf8_lossy(&out.stderr)
            )),
            Ok(_) => {}
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Builds the C program `src` against the header and the static library `lib`, and returns the program, or `None`
/// where there is no C compiler.
fn build_c(crate_dir: &Path, lib: &Path, name: &str, src: &Path) -> Option<PathBuf> {
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let exe = std::env::temp_dir().join(format!("sdrkit_{name}_{}.bin", std::process::id()));
    let built = Command::new(&cc)
        .args(["-std=c99", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(crate_dir.join("include"))
        .arg(format!(
            "-DSDRKIT_DATA_FOLDER=\"{}\"",
            crate_dir.join("../../../data").display()
        ))
        .arg(src)
        .arg(lib)
        .args(native_libs())
        .arg("-o")
        .arg(&exe)
        .status();
    match built {
        Err(e) => {
            eprintln!("skipped: no C compiler ({cc}: {e})");
            None
        }
        Ok(s) => {
            assert!(s.success(), "the C program {name} did not build");
            Some(exe)
        }
    }
}

/// The written Swift files and the Swift example agree with the C headers: the Swift compiler checks their types
/// against a module of `sdrkit.h` and the family headers. Skipped where there is no Swift compiler.
#[test]
fn the_swift_files_and_the_swift_example_type_check() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let module = std::env::temp_dir().join(format!("sdrkit_swift_{}", std::process::id()));
    std::fs::create_dir_all(module.join("SDRKitC")).expect("the module folder");
    let mut map = format!(
        "module SDRKitC {{\n    header \"{}\"\n",
        crate_dir.join("include/sdrkit.h").display()
    );
    let mut sources = vec![
        crate_dir.join("swift/SDRKit.swift"),
        crate_dir.join("swift/Examples/FmReceiver.swift"),
    ];
    for family in sdrkit::families() {
        let mut name = family.key.to_owned();
        name[..1].make_ascii_uppercase();
        sources.push(crate_dir.join(format!("swift/{name}.swift")));
        // The base header includes the headers of the session, the radio and the engine.
        if !family.object {
            let header = crate_dir.join(format!("include/sdrkit/{}.h", family.key));
            map.push_str(&format!("    header \"{}\"\n", header.display()));
        }
    }
    map.push_str("    export *\n}\n");
    std::fs::write(module.join("SDRKitC/module.modulemap"), map).expect("the module map");
    let checked = Command::new("swiftc")
        .args(["-typecheck", "-I"])
        .arg(&module)
        .args(&sources)
        .output();
    let _ = std::fs::remove_dir_all(&module);
    match checked {
        Err(e) => eprintln!("skipped: no Swift compiler ({e})"),
        Ok(out) => assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        ),
    }
}
