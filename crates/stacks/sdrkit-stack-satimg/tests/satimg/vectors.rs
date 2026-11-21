//! The recordings decode as `tests/vectors/satimg/expected.json` says.

use super::common::{cell_i64, pgm, run, vector_file};
use sdrkit_codec_image::strip::{Strip, StripPicture};
use sdrkit_core::publish::keys;
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::typed::{MeteorLrptChannelRow, VirtualChannelRow, TEXT_EVENT};

fn expected() -> serde_json::Value {
    sdrkit_dev_test::sigmf::read_expected("satimg").unwrap()
}

fn vector(name: &str) -> serde_json::Value {
    expected()["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["file"].as_str().unwrap().starts_with(name))
        .unwrap()
        .clone()
}

fn n(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn within(v: f64, e: &serde_json::Value) -> bool {
    (v - n(&e["value"])).abs() <= n(&e["tol"])
}

/// Compares a channel of a picture with an expected PGM where both have data: (pixels compared, worst difference, mean
/// difference).
fn compare(pic: &StripPicture, ch: u16, file: &str) -> (u64, i32, f64) {
    let got = pic.channel(ch).expect("the channel");
    let path = vector_file(file).expect("the expected picture");
    let (pw, ph, want) = pgm(&path);
    let w = pic.width as usize;
    assert_eq!(w, pw);
    let rows = (pic.height as usize).min(ph);
    let (mut sum, mut cnt, mut worst) = (0u64, 0u64, 0i32);
    for y in 0..rows {
        for x in 0..w {
            let a = got[y * w + x] as i32;
            let b = want[y * w + x] as i32;
            if a == 0 || b == 0 {
                continue;
            }
            let d = (a - b).abs();
            worst = worst.max(d);
            sum += d as u64;
            cnt += 1;
        }
    }
    (cnt, worst, sum as f64 / cnt.max(1) as f64)
}

#[test]
fn meteor_recording() {
    use sdrkit_stack_satimg::modes::meteor::DESCRIPTOR;
    let e = vector("meteor");
    let x = e["expect"].clone();
    let Some(rec) = sdrkit_dev_test::read_recording("satimg/meteor_lrpt_oqpsk72k_250k") else {
        return;
    };
    let h = Harness::new(&DESCRIPTOR);
    let r = run::<Strip>(
        &h,
        &rec.samples,
        250e3,
        137.9e6,
        137.9e6,
        65_536,
        Some("msumr.out"),
    );
    measured("meteor_lrpt_oqpsk72k_250k", &r);
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert_eq!(r.text("modulation"), x["modulation_running"]);
    assert_eq!(r.num("symbol_rate_hz"), n(&x["symbol_rate"]));
    assert!(
        within(r.num("offset_hz"), &x["carrier_offset_hz"]),
        "{}",
        r.num("offset_hz")
    );
    assert!(
        within(r.num("snr_db"), &x["symbol_snr_db"]),
        "{}",
        r.num("snr_db")
    );
    assert!(
        within(r.num("clock_ppm"), &x["symbol_clock_ppm"]),
        "{}",
        r.num("clock_ppm")
    );
    let good = r.num("frames_ok");
    assert!(
        good >= n(&x["frames_good"]["min"]) && good <= n(&x["frames_good"]["max"]),
        "{good}"
    );
    assert!(r.num("frames_bad") <= n(&x["frames_failed"]["max"]));
    assert_eq!(r.num("spacecraft"), 156.0);
    assert_eq!(r.num("vcid"), 5.0);
    assert!(r.num("packets") >= 960.0);
    assert_eq!(r.num("packets_lost"), 0.0);
    let packets = |ch: usize| -> f64 {
        cell_i64(&r.results, "channels", &ch.to_string(), &MeteorLrptChannelRow::PACKETS)
            .map_or(f64::NAN, |v| v as f64)
    };
    for ch in 1..=3 {
        assert!(packets(ch) >= 300.0, "{ch}");
    }
    for ch in 4..=6 {
        assert_eq!(packets(ch), 0.0, "{ch}");
    }
    assert!(packets(7) >= 20.0);
    assert_eq!(r.num("quality"), 80.0);
    let rows = r.num("picture_rows");
    assert!((23.0..=25.0).contains(&rows), "{rows}");
    assert_eq!(r.text("colour_channels"), "1,2,3");
    assert!(r.num("first_decoding") <= 2.0);
    // The picture from the strips, compared with the expected pictures.
    let mut pic = StripPicture::new(1, 1568, 12_000);
    for s in r.of::<Strip>() {
        pic.add(&s).unwrap();
    }
    for ch in 1..=3u16 {
        assert!(pic.complete_strips(ch, (1 << 14) - 1) >= 22, "{ch}");
        let (cnt, worst, mean) = compare(&pic, ch, &format!("meteor_ch{ch}.pgm"));
        assert!(cnt > 1568 * 8 * 20, "too few pixels compared: {cnt}");
        assert!(worst <= 2, "channel {ch}: worst {worst}");
        assert!(mean <= 0.5, "channel {ch}: mean {mean}");
    }
}

#[test]
fn goes_recording() {
    use sdrkit_graph::Picture;
    let e = vector("goes");
    let x = e["expect"].clone();
    let Some(rec) = sdrkit_dev_test::read_recording("satimg/goes_hrit_2m") else {
        return;
    };
    let h = Harness::new(&sdrkit_stack_satimg::modes::xrit::GOES);
    let r = run::<Picture>(
        &h,
        &rec.samples,
        2e6,
        1694.1e6,
        1694.1e6,
        65_536,
        Some("products.pictures"),
    );
    measured("goes_hrit_2m", &r);
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert!(
        within(r.num("offset_hz"), &x["carrier_offset_hz"]),
        "{}",
        r.num("offset_hz")
    );
    assert!(
        within(r.num("snr_db"), &x["symbol_snr_db"]),
        "{}",
        r.num("snr_db")
    );
    assert!(
        within(r.num("symbol_rate_measured_hz"), &x["symbol_rate_measured"]),
        "{}",
        r.num("symbol_rate_measured_hz")
    );
    assert_eq!(r.num("g2_inverted"), 1.0);
    assert_eq!(r.text("rs_basis"), "dual");
    let good = r.num("frames_ok");
    assert!(
        good >= n(&x["frames_good"]["min"]) && good <= n(&x["frames_good"]["max"]),
        "{good}"
    );
    assert_eq!(r.num("frames_bad"), 0.0);
    assert_eq!(r.num("spacecraft"), 126.0);
    assert!(r.num("fill_frames") >= 30.0);
    assert_eq!(r.num("frame_gaps"), 0.0);
    assert_eq!(r.num("crc_errors"), 0.0);
    assert_eq!(r.num("file_errors"), 0.0);
    assert_eq!(r.num("length_errors"), 0.0);
    for (vc, min) in [(0, 3.0), (2, 9.0), (13, 16.0), (30, 6.0)] {
        let got = cell_i64(&r.results, "virtual_channels", &vc.to_string(), &VirtualChannelRow::FRAMES)
            .map_or(0.0, |v| v as f64);
        assert!(got >= min, "VC {vc}: {got}");
    }
    assert_eq!(r.num("products"), 12.0);
    assert!(r.num("first_decoding") <= 0.6);
    let want_text = x["products"]["kinds_each_round"][3]["text"]
        .as_str()
        .unwrap();
    assert!(r
        .events
        .iter()
        .any(|e| e.is(&TEXT_EVENT) && e.get(&keys::TEXT) == Some(want_text)));
    // The pictures equal the PGMs, the relayed JPEG equals the file.
    let pics = r.of::<Picture>();
    let has = |name: &str, want: &[u8]| {
        pics.iter().any(|p| {
            p.name.as_deref() == Some(name)
                && p.data.len() >= want.len()
                && p.data[..want.len()] == *want
        })
    };
    for (name, pg) in [
        (
            "OR_ABI-L2-CMIPF-M6C13_G16_s20262851200204_e20262851209512_c20262851209583.lrit",
            "goes_13.pgm",
        ),
        (
            "OR_ABI-L2-CMIPM1-M6C02_G16_s20262851201251_e20262851201308_c20262851201365.lrit",
            "goes_2.pgm",
        ),
    ] {
        let (w, h, want) = pgm(&vector_file(pg).expect("the expected picture"));
        assert!(has(name, &want[..w * h]), "{pg}");
    }
    let relay = std::fs::read(vector_file("goes_relay.jpg").expect("the relayed picture")).unwrap();
    assert!(has("himawari9_relay_test.jpg", &relay));
}

#[test]
fn ahrpt_recording() {
    use sdrkit_stack_satimg::modes::hrpt::DESCRIPTOR;
    let e = vector("ahrpt");
    let x = e["expect"].clone();
    let Some(rec) = sdrkit_dev_test::read_recording("satimg/ahrpt_metop_4m") else {
        return;
    };
    let h = Harness::new(&DESCRIPTOR);
    let r = run::<Strip>(
        &h,
        &rec.samples,
        4e6,
        1701.3e6,
        1701.3e6,
        65_536,
        Some("avhrr.out"),
    );
    measured("ahrpt_metop_4m", &r);
    assert_eq!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert_eq!(r.text("variant"), "metop");
    assert!(
        within(r.num("offset_hz"), &x["carrier_offset_hz"]),
        "{}",
        r.num("offset_hz")
    );
    assert!(
        within(r.num("snr_db"), &x["symbol_snr_db"]),
        "{}",
        r.num("snr_db")
    );
    assert!(r.num("frames_ok") >= n(&x["frames_good"]["min"]));
    assert!(r.num("frames_bad") <= 1.0);
    assert_eq!(r.num("spacecraft"), 11.0);
    assert_eq!(r.num("vcid"), 9.0);
    assert_eq!(r.num("phase_ambiguity_mirrored"), 1.0);
    assert!(r.num("avhrr_lines") >= 3.0);
    assert!(r.num("packets_apid_103") >= 3.0);
    assert_eq!(r.num("packets_apid_104"), 0.0);
    // The first three lines of every channel equal the expected pictures.
    let mut pic = StripPicture::new(1, 2048, 4000);
    for s in r.of::<Strip>() {
        pic.add(&s).unwrap();
    }
    for ch in 1..=5u16 {
        let got = pic.channel(ch).expect("the channel");
        let (pw, ph, want) = pgm(&vector_file(&format!("ahrpt_ch{ch}.pgm")).unwrap());
        assert_eq!(pic.width as usize, pw);
        assert_eq!(&got[..pw * ph], &want[..], "channel {ch}");
    }
}

/// Prints the results of a recording: frames, picture rows and the bit error rate estimate.
pub fn measured(name: &str, r: &super::common::Ran) {
    println!(
        "measured {name}: frames_ok {} frames_bad {} picture_rows {} pictures.rows {} ber {:.3e} snr_db {:.2}",
        r.num("frames_ok"),
        r.num("frames_bad"),
        r.num("picture_rows"),
        r.num("pictures.rows"),
        r.num("ber"),
        r.num("snr_db")
    );
}
