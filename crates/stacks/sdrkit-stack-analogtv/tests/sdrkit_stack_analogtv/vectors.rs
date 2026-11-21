//! The recorded test vectors decode as `tests/vectors/analogtv/expected.json` says.

use super::common::*;

fn check(name: &str, idx: usize) {
    let Some(rec) = sdrkit_dev_test::read_recording(&format!("analogtv/{name}")) else {
        return;
    };
    let exp = sdrkit_dev_test::sigmf::read_expected("analogtv").unwrap();
    let e = &exp["recordings"][idx]["expected"];
    let mut rx = atv(rec.rate_hz, &[]);
    let mut pos = 0;
    let r = run(
        &mut rx,
        rec.rate_hz,
        rec.samples.len() as f64 / rec.rate_hz,
        50_000,
        |b| {
            let n = b.len().min(rec.samples.len() - pos);
            b[..n].copy_from_slice(&rec.samples[pos..pos + n]);
            pos += n;
        },
    );
    let d = r.last();
    eprintln!("{name}: {d:#?}");
    if let Some(p) = &r.picture {
        eprintln!(
            "bars {:?}",
            bars(p)
                .iter()
                .map(|b| [b[0].round(), b[1].round(), b[2].round()])
                .collect::<Vec<_>>()
        );
    }
    for l in sdrkit_core::Owner::results(&rx).log_lines() {
        eprintln!("log: {l}");
    }
    assert_eq!(d.state, 2, "{name} state");
    assert_eq!(d.system, e["system"].as_str().unwrap());
    assert_eq!(d.colour_system, e["colour_system"].as_str().unwrap());
    assert!(d.colour, "colour");
    assert_eq!(d.lines, 625);
    assert!((d.field_hz - 50.0).abs() < 0.005, "field {}", d.field_hz);
    assert!(
        (d.vision_hz - e["vision_carrier"].as_f64().unwrap()).abs() < 600.0,
        "vision {}",
        d.vision_hz
    );
    assert!(
        (d.carrier_offset_hz - e["offset_hz"].as_f64().unwrap()).abs() < 600.0,
        "offset {}",
        d.carrier_offset_hz
    );
    assert!(
        (d.sound_hz - e["sound_carrier"].as_f64().unwrap()).abs() < 3000.0,
        "sound {}",
        d.sound_hz
    );
    assert!(
        (d.carrier_to_noise_db - e["carrier_to_noise_db"].as_f64().unwrap()).abs() < 1.5,
        "C/N {}",
        d.carrier_to_noise_db
    );
    assert_eq!(d.sound_system, e["sound_system"].as_str().unwrap());
    assert!(d.synchronous_detector);
    assert!(d.data_valid);
    assert!(
        (d.sync_depth_pct - e["sync_depth_pct"].as_f64().unwrap()).abs() < 1.5,
        "sync depth {}",
        d.sync_depth_pct
    );
    assert!(
        (d.line_error_ppm - e["line_error_ppm"].as_f64().unwrap()).abs() < 6.0,
        "ppm {}",
        d.line_error_ppm
    );
    if idx == 0 {
        assert!(d.video_snr_db >= 30.0, "video SNR {}", d.video_snr_db);
        assert!(
            (d.burst_level - 1.02).abs() < 0.12,
            "burst {}",
            d.burst_level
        );
        assert!(
            r.first(|d| d.nicam.as_ref().is_some_and(|n| n.valid))
                .unwrap()
                <= 0.5
        );
    } else {
        assert!(d.video_snr_db >= 25.0, "video SNR {}", d.video_snr_db);
        assert!(
            (d.chroma_level - 1.0).abs() < 0.25,
            "chroma {}",
            d.chroma_level
        );
    }
    assert!(d.sound_present);
    assert!(d.fields_bad <= 3, "bad fields {}", d.fields_bad);
    let p = r.picture.clone().expect("a picture");
    let l = p.picture.layout.as_ref().unwrap();
    assert_eq!((l.width, l.height), (768, 576));
    let b = bars(&p);
    let want = e["bars"].as_array().unwrap();
    for (k, m) in b.iter().enumerate() {
        for c in 0..3 {
            let w = want[k][c].as_f64().unwrap();
            assert!(
                (m[c] - w).abs() <= 23.0,
                "{name} bar {k} channel {c}: {:.1} against {w}",
                m[c]
            );
        }
    }
    let (left, right) = r.tail(0.2);
    if idx == 0 {
        let (l1, r15) = (tone(&left, 1000.0), tone(&right, 1500.0));
        assert!(
            (l1 - 0.25).abs() < 0.03 && (r15 - 0.25).abs() < 0.03,
            "NICAM tones {l1} {r15}"
        );
        assert!(tone(&left, 1500.0) < l1 / 31.6 && tone(&right, 1000.0) < r15 / 31.6);
    } else {
        let (a, b) = (tone(&left, 1000.0), tone(&right, 1000.0));
        assert!(
            (a - 0.5).abs() < 0.04 && (b - 0.5).abs() < 0.04,
            "FM tone {a} {b}"
        );
    }
    assert!(r.first(|d| d.state == 2).unwrap() <= 0.5);
}

#[test]
fn the_pal_i_nicam_vector_decodes() {
    check("atv_pal_i_nicam_10m", 0);
}

#[test]
fn the_secam_dk_vector_decodes() {
    check("atv_secam_dk_10m", 1);
}
