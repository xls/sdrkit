//! Known-answer tests of the cellular channel numbers: the values come from the formulas and tables of 3GPP TS 45.005
//! clause 2, TS 36.101 table 5.7.3-1 and TS 38.104 tables 5.4.2.1-1, 5.4.2.3-1, 5.4.3.1-1 and 5.4.3.3-1.

use sdrkit_data_plans::cellular::{self, CellSystem};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn earfcn(n: u32) -> Option<(f64, u16)> {
    cellular::channel_hz(CellSystem::Lte, n).map(|(hz, b)| (hz, b.number()))
}

#[test]
fn earfcn_table_5_7_3_1() {
    // F_DL = F_DL_low + 0.1 (N_DL - N_Offs-DL).
    for (n, mhz, band) in [
        (6300, 806.0, 20),
        (0, 2110.0, 1),
        (599, 2169.9, 1),
        (1200, 1805.0, 3),
        (3450, 925.0, 8),
        (3650, 945.0, 8),
        (9920, 1452.0, 32),
        (9919, 467.4, 31),
        (43590, 3600.0, 43),
        (68586, 617.0, 71),
        (70596, 422.0, 88),
    ] {
        let (hz, b) = earfcn(n).unwrap_or_else(|| panic!("EARFCN {n}"));
        assert!(close(hz, mhz * 1e6), "EARFCN {n}: {hz}");
        assert_eq!(b, band, "EARFCN {n}");
    }
    assert!(earfcn(4960).is_none());
    let (n, b) = cellular::channel_of(CellSystem::Lte, 806e6, &[], Some(20e3)).unwrap();
    assert_eq!((n, b.number()), (6300, 20));
    let (n, b) = cellular::channel_of(CellSystem::Lte, 1844.9e6, &[9], Some(20e3)).unwrap();
    assert_eq!((n, b.number()), (3800, 9));
    assert!(cellular::channel_of(CellSystem::Lte, 1000e6, &[], None).is_none());
    let at: Vec<u16> = cellular::bands_at(CellSystem::Lte, 2125e6, &[])
        .iter()
        .map(|b| b.number())
        .collect();
    assert_eq!(at, vec![1, 4, 10, 65, 66]);
}

#[test]
fn nr_arfcn_table_5_4_2_1_1() {
    for (n, mhz) in [
        (0, 0.0),
        (599_999, 2999.995),
        (600_000, 3000.0),
        (620_000, 3300.0),
        (2_016_666, 24249.99),
        (2_016_667, 24250.08),
        (3_279_165, 99999.96),
    ] {
        assert!(close(cellular::nr_arfcn_hz(n).unwrap(), mhz * 1e6), "{n}");
        assert_eq!(cellular::nr_arfcn_of(mhz * 1e6), Some(n), "{n}");
    }
    assert_eq!(cellular::nr_arfcn_hz(3_279_166), None);
    assert_eq!(cellular::nr_arfcn_of(806e6), Some(161_200));
    assert_eq!(cellular::nr_arfcn_of(-1.0), None);
}

#[test]
fn nr_band_ranges_follow_the_global_raster() {
    // Every NR-ARFCN range of table 5.4.2.3-1 starts on the global raster.
    for b in cellular::bands(CellSystem::Nr, &[]) {
        for r in b.info.numbers {
            let f = cellular::nr_arfcn_hz(r.first).unwrap();
            assert!(close(r.first_hz, f), "{}", b.name);
            let l = cellular::nr_arfcn_hz(r.last).unwrap();
            assert!(close(b.channel_hz(r.last).unwrap(), l), "{}", b.name);
        }
        for s in b.info.sync {
            for g in [s.first, s.last] {
                let hz = cellular::gscn_hz(g).unwrap();
                assert!(hz > b.dl_low_hz && hz < b.dl_high_hz, "{} GSCN {g}", b.name);
            }
        }
    }
    let n78 = cellular::band(CellSystem::Nr, 78).unwrap();
    let r: Vec<(u32, u32, u32)> = n78
        .info
        .numbers
        .iter()
        .map(|r| (r.first, r.step, r.last))
        .collect();
    assert_eq!(r, vec![(620_000, 1, 653_333), (620_000, 2, 653_332)]);
    let n41 = cellular::band(CellSystem::Nr, 41).unwrap();
    assert_eq!(n41.info.numbers[0].last, 537_999);
    assert_eq!(n41.nearest_channel(2593.05e6).unwrap().0, 518_610);
}

#[test]
fn gscn_table_5_4_3_1_1() {
    for (g, mhz) in [
        (2, 1.25),
        (3, 1.35),
        (4, 1.45),
        (7498, 2999.05),
        (7499, 3000.0),
        (7711, 3305.28),
        (7875, 3541.44),
        (22_255, 24248.64),
        (22_256, 24250.08),
        (26_639, 99988.32),
    ] {
        assert!(close(cellular::gscn_hz(g).unwrap(), mhz * 1e6), "{g}");
        assert_eq!(cellular::gscn_of(mhz * 1e6), Some(g), "{g}");
    }
    assert_eq!(cellular::gscn_hz(1), None);
    assert_eq!(cellular::gscn_hz(26_640), None);
    for g in (2..26_640).step_by(7) {
        assert_eq!(cellular::gscn_of(cellular::gscn_hz(g).unwrap()), Some(g));
    }
    let (b, s) = cellular::sync_band(7875, Some(30), &[]).unwrap();
    assert_eq!((b.number(), s.first, s.last), (77, 7711, 8329));
    assert_eq!(
        cellular::sync_band(7875, Some(30), &[78])
            .unwrap()
            .0
            .number(),
        78
    );
    assert_eq!(
        cellular::sync_band(1990, Some(15), &[]).unwrap().0.number(),
        20
    );
    assert_eq!(
        cellular::sync_band(1990, Some(15), &[28])
            .unwrap()
            .0
            .number(),
        28
    );
    assert!(cellular::sync_band(1990, Some(30), &[]).is_none());
}

#[test]
fn gsm_arfcn_45_005_clause_2() {
    // P-GSM: F_DL = 935 + 0.2 n. E-GSM and R-GSM: F_DL = 935 + 0.2 (n - 1024) for n of 955 to 1023.
    for (n, mhz, band) in [
        (1, 935.2, 900),
        (124, 959.8, 900),
        (0, 935.0, 900),
        (975, 925.2, 900),
        (1023, 934.8, 900),
        (955, 921.2, 900),
        (128, 869.2, 850),
        (251, 893.8, 850),
        (512, 1805.2, 1800),
        (885, 1879.8, 1800),
    ] {
        let (hz, b) = cellular::channel_hz(CellSystem::Gsm, n).unwrap();
        assert!(close(hz, mhz * 1e6), "ARFCN {n}");
        assert_eq!(b.number(), band, "ARFCN {n}");
    }
    let pcs = cellular::band(CellSystem::Gsm, 1900).unwrap();
    assert!(close(pcs.channel_hz(810).unwrap(), 1989.8e6));
    assert!(cellular::channel_hz(CellSystem::Gsm, 125).is_none());
    let (n, _) = cellular::channel_of(CellSystem::Gsm, 922.2e6, &[], Some(1.0)).unwrap();
    assert_eq!(n, 960);
}
