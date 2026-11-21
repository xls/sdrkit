//! Shared test helpers: the vector folder, WAV and PPM files, and quality metrics.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The environment variable that names the codec vector folder.
pub const VECTORS_VAR: &str = "SDRKIT_CODEC_VECTORS";

/// The codec vector folder that `SDRKIT_CODEC_VECTORS` names, or `None` when the variable is not set. A variable that names no
/// folder fails the test.
pub fn vectors() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os(VECTORS_VAR)?);
    assert!(p.is_dir(), "{VECTORS_VAR}={}: not a folder", p.display());
    Some(p)
}

/// The folder in `root` that holds a file whose name ends with `suffix`. Fails the test when there is none.
pub fn folder_with(root: &Path, suffix: &str) -> PathBuf {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .find(|d| {
            std::fs::read_dir(d).is_ok_and(|mut it| {
                it.any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().ends_with(suffix)))
            })
        })
        .unwrap_or_else(|| panic!("no folder in {} holds a file *{suffix}", root.display()))
}

/// The picture vectors: their folder, and the extension of the stream files, which is the folder's name.
pub struct Pictures {
    /// The folder.
    pub dir: PathBuf,
    ext: String,
}

impl Pictures {
    /// Finds the picture folder in `root`: the one that holds the 1920 x 1080 input picture.
    pub fn find(root: &Path) -> Pictures {
        let dir = folder_with(root, "_1920x1080_input.ppm");
        let ext = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Pictures { dir, ext }
    }

    /// The stream file of `stem`.
    pub fn stream(&self, stem: &str) -> PathBuf {
        self.dir.join(format!("{stem}.{}", self.ext))
    }
}

/// Reads a 16-bit PCM mono WAV file. Returns the rate and the samples.
pub fn read_wav(path: &std::path::Path) -> (u32, Vec<i16>) {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&b[0..4], b"RIFF");
    assert_eq!(&b[8..12], b"WAVE");
    let mut i = 12;
    let mut rate = 0;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if id == b"fmt " {
            assert_eq!(u16::from_le_bytes([body[2], body[3]]), 1, "mono");
            rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            assert_eq!(u16::from_le_bytes([body[14], body[15]]), 16);
        } else if id == b"data" {
            let s = body
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c))
                .collect();
            return (rate, s);
        }
        i += 8 + len + (len & 1);
    }
    panic!("no data chunk in {}", path.display());
}

/// Reads a binary PPM (P6, maxval 255). Returns width, height and the RGB bytes.
pub fn read_ppm(path: &std::path::Path) -> (usize, usize, Vec<u8>) {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).to_string());
    }
    i += 1;
    assert_eq!(fields[0], "P6");
    let w: usize = fields[1].parse().unwrap();
    let h: usize = fields[2].parse().unwrap();
    (w, h, b[i..i + w * h * 3].to_vec())
}

/// PSNR over all bytes: 10 log10(255^2 / MSE), 99 when equal.
pub fn psnr(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mse: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (x as f64 - y as f64).powi(2))
        .sum::<f64>()
        / a.len() as f64;
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

/// The SHA-256 of `data`, as lower-case hex.
pub fn sha256(data: &[u8]) -> String {
    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut m = data.to_vec();
    let bits = (data.len() as u64) * 8;
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    for c in m.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([c[4 * i], c[4 * i + 1], c[4 * i + 2], c[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

/// Segmental SNR in dB over 20 ms frames of `clean` at `rate` (the source evaluator's rule).
pub fn seg_snr(clean: &[f64], test: &[f64], rate: usize) -> f64 {
    let fl = rate / 50;
    let n = clean.len().min(test.len()) / fl;
    let mut energies = Vec::new();
    for f in 0..n {
        energies.push(
            clean[f * fl..(f + 1) * fl]
                .iter()
                .map(|v| v * v)
                .sum::<f64>(),
        );
    }
    let emax = energies.iter().cloned().fold(0.0, f64::max);
    let mut sum = 0.0;
    let mut cnt = 0;
    for f in 0..n {
        let e = energies[f];
        if e < 1e-4 * emax || e < 1000.0 * fl as f64 {
            continue;
        }
        let err: f64 = (0..fl)
            .map(|i| (clean[f * fl + i] - test[f * fl + i]).powi(2))
            .sum();
        let snr = (10.0 * (e / err.max(1e-9)).log10()).clamp(-10.0, 35.0);
        sum += snr;
        cnt += 1;
    }
    if cnt == 0 {
        0.0
    } else {
        sum / cnt as f64
    }
}

/// Decimates by 2 with the 63-tap Blackman low-pass at 3.8 kHz (zero phase, normalised to sum 1).
pub fn decimate_lo(x: &[f64]) -> Vec<f64> {
    let n = 63;
    let fc = 3800.0 / 16000.0;
    let mut h = vec![0f64; n];
    for (i, hv) in h.iter_mut().enumerate() {
        let t = i as f64 - 31.0;
        let s = if t == 0.0 {
            2.0 * fc
        } else {
            (2.0 * std::f64::consts::PI * fc * t).sin() / (std::f64::consts::PI * t)
        };
        let a = 2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64;
        *hv = s * (0.42 - 0.5 * a.cos() + 0.08 * (2.0 * a).cos());
    }
    let sum: f64 = h.iter().sum();
    for v in h.iter_mut() {
        *v /= sum;
    }
    let mut out = Vec::with_capacity(x.len() / 2);
    let mut m = 0;
    while m < x.len() {
        let mut acc = 0.0;
        for (i, hv) in h.iter().enumerate() {
            let k = m as isize + i as isize - 31;
            if k >= 0 && (k as usize) < x.len() {
                acc += hv * x[k as usize];
            }
        }
        out.push(acc);
        m += 2;
    }
    out
}

/// segSNRlo: at 16 kHz both signals are decimated first. `delay` aligns `test` to `clean`.
pub fn seg_snr_lo(clean: &[i16], test: &[i16], rate: u32, delay: usize) -> f64 {
    let r: Vec<f64> = clean.iter().map(|&v| v as f64).collect();
    let t: Vec<f64> = test.iter().skip(delay).map(|&v| v as f64).collect();
    let n = r.len().min(t.len());
    if rate == 16000 {
        seg_snr(&decimate_lo(&r[..n]), &decimate_lo(&t[..n]), 8000)
    } else {
        seg_snr(&r[..n], &t[..n], 8000)
    }
}
