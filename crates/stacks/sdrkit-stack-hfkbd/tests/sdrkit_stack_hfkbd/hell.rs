//! Hellschreiber: every variant draws, noise draws nothing, the columns match the font, a user font, and the recording.

use crate::common::*;
use sdrkit_codec_image::raster::Raster;
use sdrkit_stack_hfkbd as hf;

#[test]
fn hell_draws_text_and_noise_draws_nothing() {
    for v in ["Feld Hell", "FSK Hell 245", "HellX5", "Hell 80"] {
        let h = snr_2500(harness(&hf::HELL), 20.0)
            .with_settings(&[pick(&hf::hell::VARIANT_SETTING, v)])
            .set(&hf::mfsk::CENTRE, 1000.0)
            .set(&hf::mfsk::FIND_SIGNAL, false)
            .seconds(12.0);
        let r = run(&h);
        let cols = num(&r, "columns");
        let want = 11.0 * hf::hell::variant(v).unwrap().columns;
        assert!(cols > 0.8 * want, "{v}: {cols} of {want}");
    }
    let r = feed(
        &harness(&hf::HELL).set(&hf::mfsk::FIND_SIGNAL, false),
        &noise_only(60.0, 2),
    );
    assert_eq!(num(&r, "columns"), 0.0);
}

/// The last 60 columns of a picture against the ideal columns, after the best alignment along the columns and by a pixel up or
/// down (Hell has no clock: the column boundary comes from the blank rows).
fn agreement_with_font(p: &Raster) -> f64 {
    let ideal = hf::hell::text_columns(hf::hell::TEST_TEXT, None);
    let n = 60usize.min(p.width() as usize);
    let mut best = 0.0f64;
    for v in -1i32..=1 {
        for shift in 0..ideal.len() {
            let mut eq = 0;
            for k in 0..n {
                let col = ideal[(shift + k) % ideal.len()];
                let x = p.width() - n as u32 + k as u32;
                for px in 0..14i32 {
                    let src = px + v;
                    let want = (0..14).contains(&src) && (col >> src) & 1 == 1;
                    let got = grey(p, x, (13 - px) as u32) > 128;
                    eq += (got == want) as usize;
                }
            }
            best = best.max(eq as f64 / (n * 14) as f64);
        }
    }
    best
}

#[test]
fn hell_columns_match_the_font() {
    let h = snr_2500(harness(&hf::HELL), 20.0)
        .set(&hf::mfsk::CENTRE, 1500.0)
        .set(&hf::mfsk::FIND_SIGNAL, false)
        .seconds(10.0);
    let x = h.generate().unwrap();
    let (mut rx, mut tap) = receiver_with_pictures(&h, RATE);
    feed_rx(rx.as_mut(), &x, 800);
    let p = pictures(&mut tap).pop().expect("a picture");
    let best = agreement_with_font(&p);
    assert!(best > 0.92, "{best}");
}

#[test]
fn hell_user_font() {
    // The Hell font has no Ł, Ş or →. A TrueType font the host supplies draws them inside the 14 pixels.
    for c in ['Ł', 'Ş', '→'] {
        assert!(sdrkit_codec_image::font::hell::glyph(c).is_none());
    }
    let path = "/System/Library/Fonts/Supplemental/Arial.ttf";
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let font = sdrkit_codec_image::font::ttf::TrueTypeFont::parse(bytes).unwrap();
    for c in ['Ł', 'Ş'] {
        let cols = hf::hell::glyph_pixels(c, Some(&font)).unwrap();
        let px: u32 = cols.iter().map(|c| c.count_ones()).sum();
        assert!(px >= 4, "{c}");
        assert!(cols.iter().all(|c| c >> 14 == 0), "{c}");
    }
    // ASCII keeps the Hell glyphs.
    assert_eq!(
        hf::hell::glyph_pixels('A', Some(&font)),
        hf::hell::glyph_pixels('A', None)
    );
}

/// The best share of equal thresholded pixels of the last 60 columns of `got` slid over the expected picture.
fn hell_agreement(got: &[[u8; 14]], refw: u32, refpx: &[u8]) -> f64 {
    let n = got.len().min(60);
    let last = &got[got.len() - n..];
    let mut best = 0.0f64;
    for off in 0..=(refw as usize).saturating_sub(n) {
        let mut eq = 0;
        for (x, col) in last.iter().enumerate() {
            for (p, &v) in col.iter().enumerate() {
                let y = 13 - p;
                let r = refpx[(y * refw as usize + off + x) * 3];
                eq += ((v > 128) == (r > 128)) as usize;
            }
        }
        best = best.max(eq as f64 / (n * 14) as f64);
    }
    best
}

#[test]
fn feldhell_recording() {
    let name = "hfkbd_feldhell_12k";
    let (rw, _rh, refpx) = read_png("feldhell_received.png");
    let rec = sdrkit_dev_test::read_recording(&format!("hfkbd/{name}")).expect("the recording");
    for piece in [800, 4093] {
        let h = harness(&hf::HELL)
            .with_settings(&[pick(&hf::hell::VARIANT_SETTING, "Feld Hell")])
            .set(&hf::mfsk::CENTRE, 1000.0)
            .set(&hf::mfsk::FIND_SIGNAL, false);
        let (mut rx, mut tap) = receiver_with_pictures(&h, rec.rate_hz);
        feed_rx(rx.as_mut(), &rec.samples, piece);
        let cols = field_num(rx.results(), "columns");
        let pics = pictures(&mut tap);
        let p = pics.last().expect("a picture");
        let got: Vec<[u8; 14]> = (0..p.width())
            .map(|x| {
                let mut c = [0u8; 14];
                for (k, v) in c.iter_mut().enumerate() {
                    *v = grey(p, x, 13 - k as u32);
                }
                c
            })
            .collect();
        let a = hell_agreement(&got, rw, &refpx);
        assert!((240.0..=280.0).contains(&cols), "{cols}");
        assert!(a >= 0.9, "{a}");
    }
}
