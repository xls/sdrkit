//! Checks of the whole crate: the CRCs by catalogue name, no file access in the components, the declared events of every stack,
//! and the settings that the receivers refuse.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{owner, StackConfig, Value, Values};
use sdrkit_stack_science as sci;
use sdrkit_util_bits::crc::catalog::{entry, CRC16_IBM_3740, CRC32_ISO_HDLC, CRC8_SMBUS};
use std::path::Path;

/// The text of every source file of the crate, with its path.
fn sources() -> Vec<(String, String)> {
    fn walk(d: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push((
                    p.display().to_string(),
                    std::fs::read_to_string(&p).unwrap(),
                ));
            }
        }
    }
    let mut v = Vec::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut v);
    v
}

/// The crate names its CRCs by the catalogue (CRC-16/IBM-3740, CRC-8/SMBUS) and has no CRC function of its own.
#[test]
fn crc16_and_crc8_are_catalogue_crcs_by_name() {
    for (name, check) in [("CRC-16/IBM-3740", 0x29B1u64), ("CRC-8/SMBUS", 0xF4)] {
        assert_eq!(
            entry(name).unwrap().engine.checksum(b"123456789"),
            check,
            "{name}"
        );
    }
    // An RS41 subframe block ends with the CRC-16/IBM-3740 of its data, little-endian.
    let mut f = Vec::new();
    sci::sonde::rs41::push_block(&mut f, 0x79, b"123456789");
    assert_eq!(
        u16::from_le_bytes([f[11], f[12]]),
        CRC16_IBM_3740.checksum(b"123456789")
    );
    // An e-Czas frame ends with the CRC-8/SMBUS of its five scrambled data bytes.
    let e = sci::eczas::frame(0x60, [1, 2, 3, 4, 5]);
    assert_eq!(e[11], CRC8_SMBUS.checksum(&e[3..8]));
    for (path, text) in sources() {
        assert!(
            !text.contains("fn crc16_ccitt"),
            "{path} defines a CRC of its own"
        );
        assert!(
            !text.contains("fn crc8_07"),
            "{path} defines a CRC of its own"
        );
    }
}

/// The SSDV packets carry the catalogue's CRC-32/ISO-HDLC, and the crate has no CRC function of its own.
#[test]
fn crc32_is_the_catalogue_crc32_iso_hdlc() {
    assert_eq!(
        entry("CRC-32/ISO-HDLC")
            .unwrap()
            .engine
            .checksum(b"123456789"),
        0xCBF4_3926
    );
    let p = sci::ssdv::test_packets("TEST", 1, 32, 16, 6, false)[0];
    let end = 15 + sci::ssdv::PAYLOAD_NOFEC;
    assert_eq!(
        u32::from_be_bytes([p[end], p[end + 1], p[end + 2], p[end + 3]]),
        CRC32_ISO_HDLC.checksum(&p[1..end])
    );
    for (path, text) in sources() {
        assert!(
            !text.contains("fn crc32"),
            "{path} defines a CRC of its own"
        );
    }
}

/// No source file of the crate touches the file system: pictures leave as items, everything else as results.
#[test]
fn components_never_touch_files() {
    for (path, text) in sources() {
        assert!(!text.contains("std::fs"), "{path} uses std::fs");
        assert!(!text.contains("File::"), "{path} opens a file");
    }
}

/// Every event of every stack's test signal is declared by the stack, and its frequency is known.
#[test]
fn every_stack_emits_what_it_declares() {
    let runs: [(&sdrkit_core::StackDescriptor, f64, f64); 8] = [
        (&sci::SONDE, 2.5e6, 6.0),
        (&sci::TIME, 64e3, 5.0),
        (&sci::ECZAS, 48e3, 15.0),
        (&sci::SAME, 64e3, 12.0),
        (&sci::MSCATTER, 48e3, 8.0),
        (&sci::SSDV, 250e3, 3.0),
        (&sci::ASTRO, 2.5e6, 1.0),
        (&sci::IONO, 1e6, 2.0),
    ];
    for (desc, rate, seconds) in runs {
        let r = sdrkit_dev_test::Harness::new(desc)
            .rate(rate)
            .seconds(seconds)
            .run()
            .unwrap();
        let decl = declaration(desc.id);
        for e in &r.events {
            assert!(
                decl.events().iter().any(|d| d.kind == e.kind()),
                "{}: {} is not declared",
                desc.id,
                e.kind()
            );
            assert!(e.freq_hz != 0.0 || desc.id == "eczas", "{}: {e:?}", desc.id);
        }
    }
}

/// Settings are strict: a value out of range or not among the choices is refused, never replaced.
#[test]
fn bad_settings_are_refused() {
    fn refused(desc: &'static sdrkit_core::StackDescriptor, key: &str, v: Value) -> bool {
        let values = sdrkit_dev_test::Harness::new(desc).values().unwrap();
        let mut rx = (desc.make_receiver)(&StackConfig::new(values)).unwrap();
        let id = rx.schema().id(key).unwrap();
        owner::apply(rx.as_mut(), id, &v).is_err()
    }
    assert!(refused(&sci::SONDE, "channels", Value::I64(12)));
    assert!(refused(&sci::TIME, "station", Value::Choice(99)));
    assert!(refused(&sci::SAME, "channel", Value::Choice(9)));
    assert!(refused(&sci::ASTRO, "fft", Value::Choice(7)));
}

/// Every generator makes its station at 0 Hz.
#[test]
fn every_generator_puts_its_station_at_0_hz() {
    for d in sci::descriptors() {
        let schema = sdrkit_components::sim::with_generator_settings(&(d.settings)()).unwrap();
        let cfg = sdrkit_core::GenConfig::new(250e3, Values::defaults(&schema)).with_seed(1);
        let g = (d.make_generator.unwrap())(&cfg).unwrap();
        assert_eq!(
            g.station_offset_hz(d.tuning.tune_offset_hz),
            0.0,
            "{}",
            d.id
        );
        assert!(g.signal_info().is_some(), "{}", d.id);
    }
}

/// The registry makes every component of the family under the name the component reports.
#[test]
fn the_registry_names_match_the_components() {
    let mut r = sdrkit_graph::Registry::new();
    sci::register(&mut r);
    let names: Vec<String> = r.names().map(str::to_string).collect();
    assert_eq!(names.len(), 37);
    for name in names {
        let c = r.make(&name).unwrap();
        assert_eq!(c.info().name, name);
    }
}

/// Every stack's verify cases pass when run as the engine runs them: at the stack's own rate and frequency, with the case's
/// settings and its noise as the standard test-signal keys.
#[test]
fn every_verify_case_passes() {
    use sdrkit_core::verify::VerifyOptions;
    let o = VerifyOptions {
        seconds: 5.0,
        clean: false,
        snr_db: 20.0,
    };
    for desc in sci::descriptors() {
        for case in (desc.verify.unwrap().cases)(&o) {
            let mut stored = case.settings.clone();
            let has = |stored: &[CaseSetting], key: &str| stored.iter().any(|c| c.key == key);
            if let Some(snr) = case.snr_db {
                if !has(&stored, GEN_NOISE.key()) {
                    stored.push(CaseSetting::new(&GEN_NOISE, NoiseKind::Snr));
                }
                if !has(&stored, GEN_SNR_DB.key()) {
                    stored.push(CaseSetting::new(&GEN_SNR_DB, snr));
                }
            }
            if case.offset_hz != 0.0 && !has(&stored, GEN_OFFSET_HZ.key()) {
                stored.push(CaseSetting::new(&GEN_OFFSET_HZ, case.offset_hz));
            }
            let mut h = sdrkit_dev_test::Harness::new(desc)
                .seconds(case.seconds)
                .piece(16_384)
                .with_settings(&stored);
            if let Some(s) = &case.submode {
                h = h.submode(s);
            }
            let r = h.run().unwrap_or_else(|e| panic!("{}: {e}", desc.id));
            for line in (case.check.unwrap())(&r.results) {
                assert_eq!(
                    line.outcome,
                    sdrkit_core::verify::Outcome::Pass,
                    "{} {}: {:?} with {stored:?}: {}",
                    desc.id,
                    case.name,
                    line,
                    r.results.status().summary
                );
            }
            if case.expect_lock {
                assert_ne!(
                    r.results.status().lock_state,
                    sdrkit_core::LockState::Searching,
                    "{} {}",
                    desc.id,
                    case.name
                );
            }
        }
    }
}

fn declaration(id: &str) -> sdrkit_core::ResultsDecl {
    match id {
        "sonde" => sci::sonde::table::declaration(),
        "time" => sci::time::declaration(),
        "eczas" => sci::eczas::declaration(),
        "same" => sci::same::declaration(),
        "mscatter" => sci::mscatter::declaration(),
        "ssdv" => sci::ssdv::declaration(),
        "astro" => sci::astro::declaration(),
        _ => sci::iono::declaration(),
    }
}

/// The generators that make a noise floor of their own leave it out when the framework adds noise: the quietest stretch of 64
/// samples of each test signal follows `gen_snr_db` alone, 20 dB more noise from 100 dB (its top) to 80 dB, where a noise of the
/// generator's own would hold it still.
#[test]
fn generators_with_their_own_noise_leave_it_to_the_framework() {
    use sdrkit_dev_test::Harness;
    let cases: [(&'static sdrkit_core::StackDescriptor, f64); 5] = [
        (&sci::beacon::DESCRIPTOR, 6.0),
        (&sci::eme::DESCRIPTOR, 6.0),
        (&sci::bursts::JOVE, 30.0),
        (&sci::bursts::CALLISTO, 30.0),
        (&sci::radar::DESCRIPTOR, 0.5),
    ];
    let power = |c: &[sdrkit_core::Complex32]| c.iter().map(|z| z.norm_sqr() as f64).sum::<f64>() / c.len() as f64;
    for (desc, seconds) in cases {
        let quiet = |snr_db: f64| {
            let x = Harness::new(desc)
                .seconds(seconds)
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, snr_db)
                .generate()
                .unwrap();
            x.as_chunks::<64>().0.iter().map(|c| power(c)).fold(f64::INFINITY, f64::min)
        };
        let (q100, q80) = (quiet(100.0), quiet(80.0));
        assert!(q80 > 30.0 * q100, "{}: quietest {q100:e} at 100 dB, {q80:e} at 80 dB", desc.id);
    }
}
