//! Test helpers: a seeded Gaussian noise source, a carrier offset, a fractional delay, and the recorded IQ files.

#![allow(dead_code)]

use sdrkit_core::Complex32;
use std::f64::consts::TAU;
use std::path::{Path, PathBuf};

/// The environment variable that names the folder of recorded IQ files.
pub const RECORDINGS_VAR: &str = "SDRKIT_CUSTOM_RECORDINGS";

/// The recordings folder that `SDRKIT_CUSTOM_RECORDINGS` names, or `None` when the variable is not set. A variable that names
/// no folder fails the test.
pub fn recordings() -> Option<PathBuf> {
    let Some(p) = std::env::var_os(RECORDINGS_VAR).map(PathBuf::from) else {
        eprintln!("{RECORDINGS_VAR} is not set: recorded IQ files skipped");
        return None;
    };
    assert!(p.is_dir(), "{RECORDINGS_VAR}={}: not a folder", p.display());
    Some(p)
}

/// The sub-folder of `root` that holds a file whose name ends with `marker`. Fails the test when there is none or more than
/// one.
pub fn folder_with(root: &Path, marker: &str) -> PathBuf {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|d| d.is_dir() && std::fs::read_dir(d).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().ends_with(marker)))
        .collect();
    assert_eq!(dirs.len(), 1, "sub-folders of {} with a file *{marker}: {dirs:?}", root.display());
    dirs.pop().unwrap()
}

/// The file in `dir` whose name ends with `suffix`. Fails the test when there is none or more than one.
pub fn recording(dir: &Path, suffix: &str) -> PathBuf {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(suffix))
        .map(|e| e.path())
        .collect();
    assert_eq!(found.len(), 1, "files *{suffix} in {}: {found:?}", dir.display());
    found.pop().unwrap()
}

/// Reads a SigMF recording of complex float32 or complex int16 samples (little-endian) from its data file. Returns the
/// sample rate from the meta file and the samples, int16 scaled by 1/32768.
pub fn read_sigmf(data: &Path) -> (f64, Vec<Complex32>) {
    let name = data.to_string_lossy();
    let meta_path = PathBuf::from(name.replace(".sigmf-data", ".sigmf-meta"));
    let meta = std::fs::read_to_string(&meta_path).unwrap_or_else(|e| panic!("{}: {e}", meta_path.display()));
    let field = |key: &str| -> String {
        let i = meta.find(key).unwrap_or_else(|| panic!("{key} not in {}", meta_path.display())) + key.len();
        meta[i..].trim_start_matches(|c: char| c == '"' || c == ':' || c.is_whitespace()).chars().take_while(|c| !matches!(c, ',' | '"' | '\n')).collect()
    };
    let rate: f64 = field("core:sample_rate").trim().parse().expect("sample rate");
    let kind = field("core:datatype");
    let b = std::fs::read(data).unwrap_or_else(|e| panic!("{}: {e}", data.display()));
    let x = match kind.as_str() {
        "cf32_le" => b.as_chunks::<8>().0.iter().map(|c| Complex32::new(f32::from_le_bytes([c[0], c[1], c[2], c[3]]), f32::from_le_bytes([c[4], c[5], c[6], c[7]]))).collect(),
        "ci16_le" => b
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| Complex32::new(f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0, f32::from(i16::from_le_bytes([c[2], c[3]])) / 32768.0))
            .collect(),
        k => panic!("{}: data type {k}", meta_path.display()),
    };
    (rate, x)
}

/// Reads a file of complex float32 samples, I then Q, little-endian.
pub fn read_cf32(path: &Path) -> Vec<Complex32> {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    b.as_chunks::<8>().0.iter().map(|c| Complex32::new(f32::from_le_bytes([c[0], c[1], c[2], c[3]]), f32::from_le_bytes([c[4], c[5], c[6], c[7]]))).collect()
}

/// Both SigMF forms (complex float32 and complex int16) of the recording in `dir` whose data file names end with `stem`
/// followed by `_cf32.sigmf-data` and `_ci16.sigmf-data`.
pub fn both_forms(dir: &Path, stem: &str) -> [(String, f64, Vec<Complex32>); 2] {
    ["cf32", "ci16"].map(|k| {
        let (rate, x) = read_sigmf(&recording(dir, &format!("{stem}_{k}.sigmf-data")));
        (format!("{stem} {k}"), rate, x)
    })
}

/// Seeded Gaussian noise (xorshift64 and Box-Muller).
pub struct Noise(u64);

impl Noise {
    /// A source with this seed.
    pub fn new(seed: u64) -> Noise {
        Noise(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(0x2545_F491_4F6C_DD1D) | 1)
    }

    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// A complex sample of total variance `var`.
    pub fn sample(&mut self, var: f64) -> Complex32 {
        let r = (-var * self.uniform().ln()).sqrt();
        let a = TAU * self.uniform();
        Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
    }

    /// Adds noise of variance `var` to every sample.
    pub fn add(&mut self, x: &mut [Complex32], var: f64) {
        for v in x {
            *v += self.sample(var);
        }
    }
}

/// Moves `x` by `hz` at sample rate `fs`.
pub fn shift(x: &mut [Complex32], hz: f64, fs: f64) {
    for (i, v) in x.iter_mut().enumerate() {
        let a = TAU * (hz * i as f64 / fs).fract();
        *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
    }
}

/// The mean power of `x`.
pub fn power(x: &[Complex32]) -> f64 {
    x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len().max(1) as f64
}

/// `x` read at `pos = i (1 + ppm 1e-6) + delay` by cubic interpolation: a clock error and a fractional delay.
pub fn resample(x: &[Complex32], ppm: f64, delay: f64) -> Vec<Complex32> {
    let r = 1.0 + ppm * 1e-6;
    let n = ((x.len() as f64 - 4.0 - delay) / r) as usize;
    let rd = sdrkit_algo_dsp::interp::Reader::new(sdrkit_algo_dsp::interp::Cubic);
    (0..n).map(|i| rd.at(x, i as f64 * r - delay)).collect()
}

/// Reads a binary PPM (P6, maxval 255). Returns width, height and the RGB bytes.
pub fn read_ppm(path: &Path) -> (usize, usize, Vec<u8>) {
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
    assert_eq!(fields[0], "P6", "{}", path.display());
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    (w, h, b[i + 1..i + 1 + w * h * 3].to_vec())
}
