//! Test helpers: the recordings folder, the WAV and `.c2` vectors, codebooks, and field unpacking.

#![allow(dead_code)]

use sdrkit_codec_codec2::{Codebooks, Mode, Table};
use sdrkit_util_bits::gray::ungray;
use sdrkit_util_bits::BitReader;
use std::path::PathBuf;

/// The folder of the Codec 2 vectors: `$SDRKIT_RECORDINGS/codec2`, else `../sdrkit-recordings/codec2` beside the workspace.
pub fn vectors_dir() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let rec = match std::env::var_os("SDRKIT_RECORDINGS").filter(|v| !v.is_empty()) {
        Some(v) => root.join(PathBuf::from(v)),
        None => root.join("..").join("sdrkit-recordings"),
    };
    rec.join("codec2")
}

/// A vector file, or `None` with the reason printed.
pub fn vector(name: &str) -> Option<Vec<u8>> {
    let p = vectors_dir().join(name);
    match std::fs::read(&p) {
        Ok(b) => Some(b),
        Err(_) => {
            eprintln!("skipped: the vector {} is missing", p.display());
            None
        }
    }
}

/// The samples of a 16-bit mono WAV vector with a 44-byte header.
pub fn wav(name: &str) -> Option<Vec<i16>> {
    let b = vector(name)?;
    Some(
        b[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect(),
    )
}

/// The file name stem of a mode in the vector names.
pub fn stem(mode: Mode) -> &'static str {
    match mode {
        Mode::M3200 => "codec2_3200",
        Mode::M1300 => "codec2_1300",
        Mode::M1200 => "codec2_1200",
        Mode::M700C => "codec2_700c",
    }
}

/// The folder of the table files.
pub fn tables_dir() -> PathBuf {
    vectors_dir().join("codebooks")
}

/// The real tables of `mode` from the vectors folder, or `None` with the reason printed.
pub fn real_books(mode: Mode) -> Option<Codebooks> {
    let books = Codebooks::load_dir(&tables_dir()).expect("table files that are there must load");
    match books.check(mode) {
        Ok(()) => Some(books),
        Err(e) => {
            eprintln!(
                "skipped: {e}. Put the table files in {}",
                tables_dir().display()
            );
            None
        }
    }
}

/// Tables of the right shape with made-up values, for tests that do not compare with the vectors.
pub fn dummy_books() -> Codebooks {
    let mut b = Codebooks::new();
    for t in Table::ALL {
        let n = t.len();
        let v: Vec<f32> = match t {
            Table::Lsp(i) => (0..n)
                .map(|j| 200.0 + 300.0 * i as f32 + 300.0 * j as f32 / n as f32)
                .collect(),
            Table::LspDelta(_) => (0..n).map(|j| 25.0 + 10.0 * j as f32).collect(),
            Table::LspVq(0) => (0..n)
                .map(|j| 0.25 * (1 + j % 10) as f32 + 0.001 * (j / 10) as f32)
                .collect(),
            Table::LspVq(_) => (0..n)
                .map(|j| ((j * 7919 % 1000) as f32 / 1000.0 - 0.5) * 0.1)
                .collect(),
            Table::WoE => (0..n)
                .map(|j| {
                    if j % 2 == 0 {
                        (j / 2) as f32 / 64.0
                    } else {
                        (j / 2) as f32 / 4.0 - 10.0
                    }
                })
                .collect(),
            Table::Vq(_) => (0..n)
                .map(|j| (j * 7919 % 1000) as f32 / 100.0 - 5.0)
                .collect(),
            Table::Energy => (0..n).map(|j| 5.0 * j as f32 - 10.0).collect(),
            Table::NlpFir => {
                // A 48-tap Hamming windowed sinc with a 600 Hz cut-off at 8 kHz.
                let h: Vec<f64> = (0..48)
                    .map(|i| {
                        let m = i as f64 - 23.5;
                        let fc = 600.0 / 8000.0;
                        let s = (std::f64::consts::TAU * fc * m).sin() / (std::f64::consts::PI * m);
                        s * (0.54 - 0.46 * (std::f64::consts::TAU * i as f64 / 47.0).cos())
                    })
                    .collect();
                let sum: f64 = h.iter().sum();
                h.iter().map(|v| (v / sum) as f32).collect()
            }
        };
        b.set(t, &v).unwrap();
    }
    b
}

/// The fields of one frame of `mode`, decoded from Gray code where the mode uses it.
pub fn fields(mode: Mode, frame: &[u8]) -> Vec<u32> {
    let widths: &[u32] = match mode {
        Mode::M3200 => &[1, 1, 7, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
        Mode::M1300 => &[1, 1, 1, 1, 7, 5, 4, 4, 4, 4, 4, 4, 4, 3, 3, 2],
        Mode::M1200 => &[1, 1, 8, 1, 1, 8, 9, 9, 9, 1],
        Mode::M700C => &[9, 9, 4, 6],
    };
    let gray = mode != Mode::M700C;
    let mut r = BitReader::new(frame);
    widths
        .iter()
        .map(|&w| {
            let v = r.u(w) as u32;
            if gray {
                ungray(v)
            } else {
                v
            }
        })
        .collect()
}

/// The band distance of spec section 16.1 between `got` and `want`: frames of 320 samples with a hop of 160 and a Hann window,
/// a 512-point FFT, 16 bands of 16 bins (bins 1 to 256), `10 log10(power + 1000)` per band, and the RMS over the bands of the
/// dB difference. Only frames within 50 dB of the loudest frame of `want` count. Returns the mean, the 95th percentile and the
/// maximum, in dB.
pub fn band_distance(got: &[i16], want: &[i16]) -> (f64, f64, f64) {
    use sdrkit_algo_dsp::fft::Fft;
    use sdrkit_algo_dsp::window::Window;
    use sdrkit_algo_dsp::Complex32;
    let win = Window::Hann.coefficients(320);
    let mut fft = Fft::forward(512);
    let mut bands = |x: &[i16]| -> Vec<([f64; 16], f64)> {
        let mut out = Vec::new();
        let mut start = 0;
        while start + 320 <= x.len() {
            let mut buf = vec![Complex32::new(0.0, 0.0); 512];
            for i in 0..320 {
                buf[i].re = x[start + i] as f32 * win[i];
            }
            fft.process(&mut buf);
            let mut b = [0.0f64; 16];
            let mut total = 0.0;
            for (k, c) in buf.iter().enumerate().take(257).skip(1) {
                let p = c.norm_sqr() as f64;
                b[(k - 1) / 16] += p;
                total += p;
            }
            out.push((b, total));
            start += 160;
        }
        out
    };
    let g = bands(got);
    let w = bands(want);
    let loudest = w.iter().map(|f| f.1).fold(0.0, f64::max);
    let mut d: Vec<f64> = g
        .iter()
        .zip(&w)
        .filter(|(_, wf)| wf.1 > 0.0 && 10.0 * (loudest / wf.1).log10() <= 50.0)
        .map(|(gf, wf)| {
            let s: f64 = (0..16)
                .map(|k| {
                    let e = 10.0 * (gf.0[k] + 1000.0).log10() - 10.0 * (wf.0[k] + 1000.0).log10();
                    e * e
                })
                .sum();
            (s / 16.0).sqrt()
        })
        .collect();
    d.sort_by(f64::total_cmp);
    let mean = d.iter().sum::<f64>() / d.len().max(1) as f64;
    let p95 = d[((d.len() as f64 * 0.95) as usize).min(d.len() - 1)];
    (mean, p95, *d.last().unwrap_or(&0.0))
}
