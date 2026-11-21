//! The code tables loaded at run time: the alist format, its refusals, the folder loader, and the receivers and generators
//! with a loaded table. The tables here are test codes made from a fixed seed (see [`super::common::test_code_rows`]), because
//! sdrkit does not ship the protocol tables. The FT8 table of `sdrkit-algo-fec` written as an alist file is the known answer of
//! the format.

use super::common::*;
use sdrkit_algo_fec::ldpc::alist::Alist;
use sdrkit_algo_fec::ldpc::{tables::wsjt, InfoAt, LdpcCode};
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Complex32, GenConfig, LockState, Receiver, SettingError, StackConfig, Value};
use sdrkit_stack_wsjt::encode;
use sdrkit_stack_wsjt::proto::codes::{self, CodeError, CodeId, CodeStore, CodeTable};
use sdrkit_stack_wsjt::proto::msg77::pack77;
use sdrkit_stack_wsjt::{Mode, Sub};
use std::path::PathBuf;

/// The alist text of a matrix of `n` columns with these rows.
fn alist_text(n: usize, rows: &[Vec<u16>]) -> String {
    let rows = rows
        .iter()
        .map(|r| r.iter().map(|&c| u32::from(c)).collect())
        .collect();
    Alist::from_rows(n, rows).unwrap().to_text()
}

/// The test code of `id` as the text of its file: an alist text, or the text form of the code over GF(64).
pub fn test_code_text(id: CodeId) -> String {
    if id.over_gf64() {
        return test_q65_code();
    }
    alist_text(id.n(), &test_code_rows(id.n(), id.k(), id as u64 + 7))
}

/// A store with the test codes of FST4, FST4W, MSK144 and Q65, loaded through the parsers of their files.
pub fn test_codes() -> CodeStore {
    let mut s = CodeStore::default();
    for id in CodeId::ALL {
        s.install(CodeTable::parse(id, &test_code_text(id)).unwrap());
    }
    s
}

/// A fresh folder for this test, holding the test codes' files when `with_files`.
fn folder(name: &str, with_files: bool) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sdrkit-wsjt-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    if with_files {
        for id in CodeId::ALL {
            std::fs::write(d.join(id.file_name()), test_code_text(id)).unwrap();
        }
    }
    d
}

#[test]
fn the_ft8_table_survives_the_alist_format() {
    let rows: Vec<Vec<u16>> = wsjt::FT8_CHECKS
        .iter()
        .map(|r| r.iter().map(|&b| b as u16).collect())
        .collect();
    let text = alist_text(174, &rows);
    let back = Alist::parse(&text).unwrap();
    assert_eq!(back.n(), 174);
    let mut a: Vec<Vec<u16>> = back
        .rows()
        .iter()
        .map(|r| r.iter().map(|&c| c as u16).collect())
        .collect();
    let mut b = rows.clone();
    for r in a.iter_mut().chain(b.iter_mut()) {
        r.sort_unstable();
    }
    assert_eq!(a, b);
    // The loaded matrix encodes exactly as the shipped one: the FT8 vector "CQ K1ABC FN42" again.
    let checks: Vec<&[u16]> = a.iter().map(Vec::as_slice).collect();
    let loaded = LdpcCode::from_checks("ft8 loaded", 174, 91, &checks, InfoAt::Start).unwrap();
    let mut info = pack77("CQ K1ABC FN42").unwrap();
    let crc = sdrkit_util_bits::crc::cyclic::FT8_CRC14.compute(&info);
    sdrkit_util_bits::pack::push_u64(&mut info, crc, 14);
    let (mut x, mut y) = (vec![0u8; 174], vec![0u8; 174]);
    loaded.encode(&info, &mut x).unwrap();
    codes::ft8().encode(&info, &mut y).unwrap();
    assert_eq!(x, y);
    assert_eq!(
        encode::ft8_tones(&x)[7..14],
        [0, 0, 0, 0, 0, 0, 0],
        "the first data symbols of the vector"
    );
}

#[test]
fn bad_tables_are_refused() {
    // Not numbers, a short file, an index beyond the matrix, column and row lists that disagree.
    for text in [
        "a b",
        "4 2\n2 4\n1 1",
        "2 1\n1 2\n1 1\n2\n1\n1\n1 3",
        "2 1\n1 2\n1 1\n2\n1\n1\n1 1",
    ] {
        assert!(
            matches!(
                CodeTable::parse_alist(CodeId::Msk144, text),
                Err(CodeError::Alist(_))
            ),
            "{text}"
        );
    }
    // The right format for the wrong code.
    let fst4 = test_code_text(CodeId::Fst4);
    assert!(matches!(
        CodeTable::parse_alist(CodeId::Msk144, &fst4),
        Err(CodeError::Size { .. })
    ));
    // Checks without an encoder for the information bits first: every check on the information bits only.
    let k = CodeId::Msk144.k();
    let rows: Vec<Vec<u16>> = (0..128 - k)
        .map(|j| vec![j as u16, (j + 1) as u16])
        .collect();
    assert!(matches!(
        CodeTable::from_checks(CodeId::Msk144, 128, &rows),
        Err(CodeError::NoEncoder(CodeId::Msk144))
    ));
    // A comment line is allowed.
    let mut text = String::from("# a test code\n");
    text += &test_code_text(CodeId::Msk144);
    assert!(CodeTable::parse_alist(CodeId::Msk144, &text).is_ok());
}

#[test]
fn a_folder_loads_the_files_it_holds() {
    let mut s = CodeStore::default();
    assert_eq!(s.load_dir(&folder("empty", false)).unwrap(), vec![]);
    let d = folder("full", true);
    assert_eq!(s.load_dir(&d).unwrap(), CodeId::ALL.to_vec());
    for id in CodeId::ALL {
        let n = if id.over_gf64() {
            s.symbols(id).unwrap().n()
        } else {
            s.get(id).unwrap().n()
        };
        assert_eq!(n, id.n());
    }
    // A broken file refuses the folder, and nothing is loaded.
    std::fs::write(d.join(CodeId::Fst4w.file_name()), "not a table").unwrap();
    let mut t = CodeStore::default();
    assert!(t.load_dir(&d).is_err());
    assert!(t.get(CodeId::Fst4).is_none());
}

#[test]
fn without_a_table_the_status_says_which_file_to_load() {
    for (m, file) in [
        (Mode::Fst4, "fst4_240_101.alist"),
        (Mode::Fst4w, "fst4w_240_74.alist"),
        (Mode::Msk144, "msk144_128_90.alist"),
        (Mode::Q65, "q65_65_15.nbcode"),
    ] {
        let a = noise(16 * 8000, SIGMA, 1);
        let mut rx = receiver(Some(m), &[], 8000.0, 0.0);
        assert!(texts(&feed(&mut rx, &iq_of(&a), 8000)).is_empty());
        let st = rx.results().status();
        assert_eq!(st.lock_state, LockState::Off, "{m:?}");
        assert!(st.summary.contains(file), "{m:?}: {}", st.summary);
    }
    // A folder with a broken table is a refused setting.
    let d = folder("broken", false);
    std::fs::write(d.join(CodeId::Msk144.file_name()), "1 2 3").unwrap();
    let mut rx = receiver(Some(Mode::Msk144), &[], 8000.0, 0.0);
    let id = rx.schema().id("code_dir").unwrap();
    assert!(matches!(
        owner::apply(&mut rx, id, &Value::Text(d.display().to_string())),
        Err(SettingError::Malformed { .. })
    ));
}

#[test]
fn msk144_decodes_through_the_receiver_with_a_loaded_table() {
    let d = folder("msk144", true);
    let dir = Value::Text(d.display().to_string());
    // The test signal with the folder's table, then noise, through the receiver with the same folder.
    let settings = settings(
        Some(Mode::Msk144),
        &[
            ("code_dir", dir.clone()),
            ("gen_single", Value::Text("CQ K1ABC FN42".into())),
        ],
    );
    let mut g = sdrkit_stack_wsjt::generator::WsjtGenerator::from_config(
        Mode::Msk144,
        &GenConfig::new(12000.0, settings.clone()).with_seed(1),
    );
    let mut iq = vec![Complex32::default(); 12000 * 16];
    assert_eq!(g.generate(&mut iq), iq.len());
    assert!(iq[12000 * 5].norm() > 0.5, "the generator sends");
    add_noise(&mut iq, 0.0, 12000.0, 5);
    let mut rx =
        sdrkit_stack_wsjt::receiver::receiver(Some(Mode::Msk144), &StackConfig::new(settings))
            .unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(12000.0, 50.26e6, 50.26e6).recording())
        .unwrap();
    let got = texts(&feed(&mut rx, &iq, 1200));
    assert!(got.contains(&"CQ K1ABC FN42".to_string()), "{got:?}");
    assert_ne!(rx.results().status().lock_state, LockState::Off);
}

#[test]
fn fst4_and_fst4w_decode_with_loaded_tables() {
    let codes = test_codes();
    for (sub, text) in [
        (Sub::new(Mode::Fst4).with_period(15.0), "CQ K1ABC FN42"),
        (Sub::new(Mode::Fst4w).with_period(120.0), "K1ABC FN42 37"),
    ] {
        // FST4W is searched around 1500 Hz, as WSPR.
        let a = audio_with(
            sub,
            &[(text, 1500.0, -8.0, 0.0, false)],
            sub.period,
            true,
            3,
            &codes,
        );
        let mut rx = receiver_with(
            Some(sub.mode),
            &[(
                if sub.mode == Mode::Fst4 {
                    "fst4_period"
                } else {
                    "fst4w_period"
                },
                Value::Duration(sub.period),
            )],
            8000.0,
            0.0,
            codes.clone(),
        );
        let got = texts(&feed(&mut rx, &iq_of(&a), 8000));
        assert!(
            got.contains(&text.to_string()),
            "{:?}: {got:?} {:?}",
            sub.mode,
            rx.results().log_lines()
        );
    }
}
