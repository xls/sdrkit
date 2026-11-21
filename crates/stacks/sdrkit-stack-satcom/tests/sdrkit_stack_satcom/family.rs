//! Rules every stack of the family follows: CRCs by catalogue name, strict settings, verification cases that pass.

use crate::common::{set, values};
use sdrkit_core::ValueKind;
use sdrkit_util_bits::crc::catalog;
use serde_json::json;
use std::path::Path;

/// The family's CRCs are catalogue entries, each with the check value of "123456789" from the CRC RevEng catalogue. No source
/// file of the crate defines a CRC, a Fletcher sum or a date conversion of its own.
#[test]
fn crcs_come_from_the_catalogue() {
    let check = b"123456789";
    // CRC-16/IBM-SDLC (X.25 FCS of HDLC and AX.25, ISO/IEC 13239).
    assert_eq!(catalog::CRC16_IBM_SDLC.checksum(check), 0x906E);
    // CRC-16/KERMIT (the ACARS block check sequence, ARINC 618).
    assert_eq!(catalog::CRC16_KERMIT.checksum(check), 0x2189);
    // CRC-16/IBM-3740 (the CRC-16 named CCITT-FALSE).
    assert_eq!(catalog::CRC16_IBM_3740.checksum(check), 0x29B1);
    // CRC-32/ISO-HDLC and CRC-32/MPEG-2 are different CRCs; neither is "crc32".
    assert_eq!(catalog::CRC32_ISO_HDLC.checksum(check), 0xCBF4_3926);
    assert_eq!(catalog::CRC32_MPEG2.checksum(check), 0x0376_E6E7);
    // CRC-32/ISCSI (CRC-32C of the CubeSat Space Protocol).
    assert_eq!(catalog::CRC32_ISCSI.checksum(check), 0xE306_9283);

    let mut found = Vec::new();
    scan(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    assert!(found.is_empty(), "private checks or dates: {found:?}");
}

fn scan(dir: &Path, found: &mut Vec<String>) {
    let private = |l: &str| {
        let l = l.trim_start();
        if l.starts_with("//") {
            return false;
        }
        let Some(rest) = l.split("fn ").nth(1) else {
            return false;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        name.starts_with("crc")
            || name.starts_with("fletcher")
            || name == "days_from_civil"
            || name == "civil_from_days"
    };
    for e in std::fs::read_dir(dir).expect("the crate's sources") {
        let p = e.expect("a directory entry").path();
        if p.is_dir() {
            scan(&p, found);
        } else if p.extension().is_some_and(|x| x == "rs") {
            let text = std::fs::read_to_string(&p).expect("a source file");
            for (i, l) in text.lines().enumerate() {
                if private(l) {
                    found.push(format!("{}:{}", p.display(), i + 1));
                }
            }
        }
    }
}

/// A setting value of the wrong kind, out of range or not one of the choices is refused by every stack. It never falls back to a
/// default.
#[test]
fn every_stack_refuses_bad_settings() {
    for d in sdrkit_stack_satcom::descriptors() {
        let mut rx = (d.make_receiver)(&sdrkit_core::StackConfig::new(values(d, &[]))).unwrap();
        let schema = rx.schema().clone();
        for s in schema.settings() {
            let key = s.field.key.to_string();
            if key.starts_with("gen_") {
                continue;
            }
            let bad = match s.field.kind {
                ValueKind::F64 | ValueKind::Hz | ValueKind::Duration | ValueKind::I64 => {
                    match &s.range {
                        Some(r) => json!(r.max + 1e9),
                        None => json!("many"),
                    }
                }
                ValueKind::Choice => json!("no such choice"),
                ValueKind::Bool => json!("perhaps"),
                _ => continue,
            };
            assert!(
                set(rx.as_mut(), &key, bad.clone()).is_err(),
                "{}: {key} accepted {bad:?}",
                d.id
            );
        }
    }
}

/// Every verification case of every stack passes when run as `sdrkit verify` runs it (3 s, 30 dB in the stack's bandwidth, the
/// station moved by the case's offset), with the samples fed in signal time instead of wall time.
#[test]
fn every_verify_case_passes_off_line() {
    use sdrkit_core::verify::{Outcome, VerifyOptions};
    use sdrkit_core::{Complex32, GenConfig, InputInfo, LockState, Output};
    let o = VerifyOptions {
        seconds: 3.0,
        clean: false,
        snr_db: 30.0,
    };
    let mut failed = Vec::new();
    for d in sdrkit_stack_satcom::descriptors() {
        let hooks = d.verify.expect("every stack has hooks");
        for case in (hooks.cases)(&o) {
            let values = values(d, &case.settings);
            let rate = d.tuning.rate_hz;
            let tuned = case.freq_hz.unwrap_or(d.tuning.default_hz);
            let centre = tuned + d.tuning.tune_offset_hz;
            let make = d.make_generator.expect("a generator");
            let mut gc = GenConfig::new(rate, values.clone()).with_seed(1);
            gc.submode = case.submode.clone();
            let raw = make(&gc).unwrap();
            // The generator puts the station where its `station_offset_hz` says; it is moved onto the tuned frequency below.
            let station = raw.station_offset_hz(d.tuning.tune_offset_hz);
            let mut g =
                sdrkit_components::sim::impaired(raw, &values).expect("the generator's settings");
            let mut x = vec![Complex32::default(); (case.seconds * rate) as usize];
            let n = g.generate(&mut x);
            x.truncate(n);
            let shift = tuned - centre - station + case.offset_hz;
            if shift.abs() > 0.5 {
                sdrkit_algo_sim::offset::FrequencyOffset::new(rate, shift, 0.0)
                    .expect("an offset")
                    .process(&mut x);
            }
            if let Some(snr) = case.snr_db {
                let pre = &x[..x.len().min((rate * 0.25) as usize + 1)];
                let p = pre.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / pre.len() as f64;
                let noise = p.max(1e-12) / 10f64.powf(snr / 10.0) * rate / d.tuning.bandwidth_hz;
                sdrkit_algo_sim::noise::Awgn::with_power(noise, 7)
                    .expect("a noise power")
                    .process(&mut x);
            }
            let mut cfg = sdrkit_core::StackConfig::new(values.clone());
            cfg.submode = case.submode.clone();
            let mut rx = (d.make_receiver)(&cfg).unwrap();
            rx.configure(&InputInfo::new(rate, centre, tuned))
                .expect("the stack's own rate");
            let mut out = Output::default();
            let mut best = LockState::Off;
            let rank = |l: LockState| l as u8;
            for piece in x.chunks((rate * 0.01) as usize) {
                out.clear();
                rx.feed(piece, &mut out);
                let l = rx.results().status().lock_state;
                if matches!(l, LockState::Tracking | LockState::Locked) && rank(l) > rank(best) {
                    best = l;
                }
            }
            if case.expect_lock && !matches!(best, LockState::Tracking | LockState::Locked) {
                failed.push(format!("{} / {}: no lock", d.id, case.name));
            }
            if let Some(check) = case.check {
                for c in check(rx.results()) {
                    if c.outcome == Outcome::Fail {
                        failed.push(format!("{} / {}: {} {}", d.id, case.name, c.key, c.detail));
                    }
                }
            }
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}
