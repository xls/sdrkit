//! Test helpers for stacks: generate a stack's test signal, impair it through `sdrkit-algo-sim`, feed it to the stack's
//! receiver, collect what comes out, and compare it with the expectations of a recorded vector.
//!
//! [`Harness`] runs one stack and returns a [`Run`]. [`plans`] loads the band plans of the repository's `data/plans/`. [`sigmf`] reads SigMF recordings. [`measure`] has small measurements.
//! [`fer`] measures the frame error rate of decoders against the signal to noise ratio.
//! [`Rng`], [`Awgn`], [`FrequencyOffset`], [`ClockError`], [`Echo`], [`Quantizer`], [`add_into`] and [`scale`] impair a
//! signal by hand.

#![warn(missing_docs)]

mod error;
pub mod fer;
mod harness;
mod impair;
mod plans;
pub mod recordings;

pub use error::Error;
pub use harness::{Harness, Run};
pub use impair::{add_into, scale, Awgn, ClockError, Echo, FrequencyOffset, Quantizer, Rng};
pub use plans::{plan_files, plans};
pub use recordings::{read_recording, recording, recordings_dir};

use sdrkit_core::Complex32;
#[cfg(test)]
use std::f64::consts::TAU;

/// Small measurements for tests.
pub mod measure {
    use super::Complex32;
    use std::f64::consts::TAU;

    /// The peak amplitude of the tone at `freq_hz` in a real signal: `2 |(1/N) sum x[n] e^{-j 2 pi f n / rate}|` (a single-bin DFT).
    /// A sine of amplitude `a` at exactly `freq_hz` reads `a`; an integer number of cycles in the block keeps leakage from other tones
    /// out.
    pub fn tone_amplitude(x: &[f32], rate_hz: f64, freq_hz: f64) -> f64 {
        if x.is_empty() {
            return 0.0;
        }
        let w = TAU * freq_hz / rate_hz;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let (mut c, mut s) = (1.0f64, 0.0f64);
        let (rs, rc) = w.sin_cos();
        for (n, &v) in x.iter().enumerate() {
            if n % 1024 == 0 {
                let (ss, cc) = (w * n as f64).sin_cos();
                c = cc;
                s = ss;
            }
            re += v as f64 * c;
            im -= v as f64 * s;
            let nc = c * rc - s * rs;
            s = s * rc + c * rs;
            c = nc;
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    /// The root mean square of a real signal.
    pub fn rms(x: &[f32]) -> f64 {
        if x.is_empty() {
            return 0.0;
        }
        (x.iter().map(|&v| v as f64 * v as f64).sum::<f64>() / x.len() as f64).sqrt()
    }

    /// The mean power `|x|^2` of a complex signal.
    pub fn power(x: &[Complex32]) -> f64 {
        if x.is_empty() {
            return 0.0;
        }
        x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64
    }

    /// Splits interleaved stereo `[l, r, l, r, ...]` into its two channels.
    pub fn deinterleave(x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        (
            x.iter().step_by(2).copied().collect(),
            x.iter().skip(1).step_by(2).copied().collect(),
        )
    }

    /// `20 log10(a / b)`.
    pub fn ratio_db(a: f64, b: f64) -> f64 {
        20.0 * (a / b).log10()
    }
}

/// Reading SigMF recordings (the `.sigmf-meta` JSON and the `.sigmf-data` samples). The datatypes `cf32_le` and `ci16_le` are read.
pub mod sigmf {
    use super::Complex32;
    use crate::Error;
    use serde_json::Value as Json;
    use std::path::{Path, PathBuf};

    /// A recording: its samples and what its metadata says.
    #[derive(Clone, Debug)]
    pub struct Recording {
        /// `core:sample_rate`.
        pub rate_hz: f64,
        /// `core:frequency` of the first capture (0 when absent).
        pub centre_hz: f64,
        /// `core:description` (empty when absent).
        pub description: String,
        /// The samples.
        pub samples: Vec<Complex32>,
    }

    /// The workspace's `tests/vectors` folder, found by walking up from this crate (or from `CARGO_MANIFEST_DIR` at run time).
    pub fn vectors_dir() -> PathBuf {
        let start = std::env::var_os("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
        let mut dir: Option<&Path> = Some(&start);
        while let Some(d) = dir {
            let v = d.join("tests").join("vectors");
            if v.is_dir() {
                return v;
            }
            dir = d.parent();
        }
        panic!("no tests/vectors folder above {}", start.display());
    }

    /// Decodes little-endian complex float32 bytes (`cf32_le`). A trailing partial sample is ignored.
    pub fn decode_cf32_le(bytes: &[u8]) -> Vec<Complex32> {
        bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| {
                Complex32::new(
                    f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                    f32::from_le_bytes([c[4], c[5], c[6], c[7]]),
                )
            })
            .collect()
    }

    /// Decodes little-endian complex 16-bit integer bytes (`ci16_le`: I then Q, two's complement). Each value is divided by
    /// 32768, so full scale is -1 to 32767/32768. A trailing partial sample is ignored.
    ///
    /// SigMF 1.0 `core:datatype` returns the layout. SigMF does not fix a scale; 32768 is the scale the sdrkit recorder writes.
    ///
    /// ```
    /// use sdrkit_dev_test::sigmf::decode_ci16_le;
    /// let x = decode_ci16_le(&[0x00, 0x80, 0xFF, 0x7F]);
    /// assert_eq!((x[0].re, x[0].im), (-1.0, 32767.0 / 32768.0));
    /// ```
    pub fn decode_ci16_le(bytes: &[u8]) -> Vec<Complex32> {
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                Complex32::new(
                    i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0,
                    i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0,
                )
            })
            .collect()
    }

    /// Encodes samples as `cf32_le` bytes.
    pub fn encode_cf32_le(samples: &[Complex32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(samples.len() * 8);
        for s in samples {
            out.extend_from_slice(&s.re.to_le_bytes());
            out.extend_from_slice(&s.im.to_le_bytes());
        }
        out
    }

    /// Reads a recording from its `.sigmf-meta` path (or its base path without an extension). The data file is in the same
    /// folder.
    ///
    /// # Errors
    ///
    /// What [`read_pair`] returns.
    pub fn read(meta_path: &Path) -> Result<Recording, Error> {
        let base = meta_path.with_extension("");
        read_pair(
            &base.with_extension("sigmf-meta"),
            &base.with_extension("sigmf-data"),
        )
    }

    /// Reads a recording whose `.sigmf-meta` and `.sigmf-data` are in different places.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] or [`Error::Json`] if a file does not read, [`Error::Datatype`] for a datatype other than `cf32_le` or
    /// `ci16_le`, and [`Error::NoRate`] if the description has no sample rate.
    pub fn read_pair(meta: &Path, data: &Path) -> Result<Recording, Error> {
        let json = read_json(meta)?;
        let global = &json["global"];
        let datatype = global["core:datatype"].as_str().unwrap_or("");
        let decode: fn(&[u8]) -> Vec<Complex32> = match datatype {
            "cf32_le" => decode_cf32_le,
            "ci16_le" => decode_ci16_le,
            _ => {
                return Err(Error::Datatype {
                    path: meta.to_path_buf(),
                    found: datatype.to_string(),
                })
            }
        };
        let rate_hz = global["core:sample_rate"]
            .as_f64()
            .ok_or_else(|| Error::NoRate {
                path: meta.to_path_buf(),
            })?;
        let centre_hz = json["captures"][0]["core:frequency"]
            .as_f64()
            .unwrap_or(0.0);
        let description = global["core:description"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let bytes = std::fs::read(data).map_err(|source| Error::Io {
            path: data.to_path_buf(),
            source,
        })?;
        Ok(Recording {
            rate_hz,
            centre_hz,
            description,
            samples: decode(&bytes),
        })
    }

    /// Reads `tests/vectors/<stack>/<name>.sigmf-meta` and its samples from the recordings folder ([`crate::recordings`]).
    /// A test uses [`crate::read_recording`], which skips when the samples are missing.
    ///
    /// # Errors
    ///
    /// What [`read_pair`] returns.
    pub fn read_vector(stack: &str, name: &str) -> Result<Recording, Error> {
        let data = crate::recordings::recordings_dir()
            .join(stack)
            .join(format!("{name}.sigmf-data"));
        read_pair(
            &vectors_dir().join(stack).join(format!("{name}.sigmf-meta")),
            &data,
        )
    }

    /// Reads `tests/vectors/<stack>/expected.json`.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] or [`Error::Json`] if the file does not read.
    pub fn read_expected(stack: &str) -> Result<Json, Error> {
        read_json(&vectors_dir().join(stack).join("expected.json"))
    }

    fn read_json(path: &Path) -> Result<Json, Error> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&text).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, rate: f64, f: f64, amp: f32) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = TAU * f * i as f64 / rate;
                Complex32::new(amp * p.cos() as f32, amp * p.sin() as f32)
            })
            .collect()
    }

    /// The frequency of a complex tone, from the mean phase step.
    fn freq_of(x: &[Complex32], rate: f64) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for w in x.windows(2) {
            let p = w[1] * w[0].conj();
            re += p.re as f64;
            im += p.im as f64;
        }
        im.atan2(re) * rate / TAU
    }

    #[test]
    fn the_rng_is_repeatable_and_its_normals_are_standard() {
        let a: Vec<u64> = {
            let mut r = Rng::new(7);
            (0..5).map(|_| r.next_u64()).collect()
        };
        let b: Vec<u64> = {
            let mut r = Rng::new(7);
            (0..5).map(|_| r.next_u64()).collect()
        };
        assert_eq!(a, b);
        assert_ne!(Rng::new(8).next_u64(), a[0]);
        let mut r = Rng::new(1);
        let n = 200_000;
        let v: Vec<f64> = (0..n).map(|_| r.gauss()).collect();
        let mean = v.iter().sum::<f64>() / n as f64;
        let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.01, "mean {mean}");
        assert!((var - 1.0).abs() < 0.01, "variance {var}");
        assert!((0..1000).all(|_| r.below(10) < 10));
    }

    #[test]
    fn noise_has_the_asked_power_in_the_asked_bandwidth() {
        // A carrier of power 0.09 at 40 dB C/N in 200 kHz, sampled at 2 MHz: the total noise is 0.09e-4 * 10.
        let mut n = Awgn::new(0.09, 40.0, 200e3, 2e6, 3);
        assert!((n.power() - 0.09e-4 * 10.0).abs() < 1e-12);
        let x = n.noise(400_000);
        let p = measure::power(&x);
        assert!(
            (p / n.power() - 1.0).abs() < 0.01,
            "measured {p}, asked {}",
            n.power()
        );
        // Same seed, same noise; the piece size changes nothing.
        let mut a = Awgn::with_sigma(0.1, 5);
        let mut b = Awgn::with_sigma(0.1, 5);
        let mut x1 = vec![Complex32::default(); 1000];
        a.process(&mut x1);
        let mut x2 = vec![Complex32::default(); 1000];
        for c in x2.chunks_mut(37) {
            b.process(c);
        }
        assert_eq!(x1, x2);
    }

    #[test]
    fn an_offset_moves_a_tone() {
        let mut x = tone(10_000, 1e6, 10e3, 1.0);
        let mut off = FrequencyOffset::new(1e6, 2_500.0);
        for c in x.chunks_mut(333) {
            off.process(c);
        }
        assert!((freq_of(&x, 1e6) - 12_500.0).abs() < 0.5);
    }

    #[test]
    fn a_fast_clock_returns_more_samples_and_lower_frequencies() {
        let rate = 1e6;
        let x = tone(1_000_000, rate, 100e3, 0.5);
        let mut clk = ClockError::new(rate, 100.0);
        let mut y = Vec::new();
        for c in x.chunks(4096) {
            clk.process(c, &mut y);
        }
        let extra = y.len() as f64 - x.len() as f64;
        assert!((extra - 100.0).abs() <= 2.0, "{extra} extra samples");
        let f = freq_of(&y[1000..], rate);
        let want = 100e3 / (1.0 + 100e-6);
        assert!((f - want).abs() < 0.2, "{f} Hz, want {want}");
        // The amplitude is kept.
        let p = measure::power(&y[1000..]);
        assert!((p / 0.25 - 1.0).abs() < 0.01);
    }

    #[test]
    fn an_echo_adds_a_delayed_weaker_copy() {
        let mut x = [Complex32::default(); 20];
        x[0] = Complex32::new(1.0, 0.0);
        let mut e = Echo::new(5, -6.0);
        let (a, b) = x.split_at_mut(3);
        e.process(a);
        e.process(b);
        assert_eq!(x[0], Complex32::new(1.0, 0.0));
        assert!((x[5].re - 0.501).abs() < 0.001);
        assert!(x
            .iter()
            .enumerate()
            .all(|(i, v)| i == 0 || i == 5 || v.norm() == 0.0));
        let mut y = vec![Complex32::new(1.0, 0.0); 4];
        Echo::with_phase(1, 0.0, std::f64::consts::PI).process(&mut y);
        assert!(y[1].norm() < 1e-6);
    }

    #[test]
    fn the_quantizer_clips_and_rounds() {
        let q = Quantizer::new(8, 1.0);
        let mut x = vec![Complex32::new(2.0, -3.0), Complex32::new(0.001, 0.5)];
        q.process(&mut x);
        assert!((x[0].re - 1.0).abs() < 0.005 && (x[0].im + 1.0).abs() < 0.005);
        assert!((x[1].im - 0.5).abs() <= 1.0 / 255.0);
        let levels: std::collections::BTreeSet<i64> = (0..10_000)
            .map(|i| {
                let mut v = [Complex32::new(i as f32 / 5000.0 - 1.0, 0.0)];
                q.process(&mut v);
                (v[0].re * 1e6) as i64
            })
            .collect();
        assert_eq!(levels.len(), 256);
    }

    #[test]
    fn tone_amplitude_reads_a_sine() {
        let x: Vec<f32> = (0..48_000)
            .map(|i| {
                (0.4 * (TAU * 1000.0 * i as f64 / 48e3).sin()
                    + 0.1 * (TAU * 3000.0 * i as f64 / 48e3).cos()) as f32
            })
            .collect();
        assert!((measure::tone_amplitude(&x, 48e3, 1000.0) - 0.4).abs() < 1e-6);
        assert!((measure::tone_amplitude(&x, 48e3, 3000.0) - 0.1).abs() < 1e-6);
        assert!(measure::tone_amplitude(&x, 48e3, 2000.0) < 1e-6);
        assert!(
            (measure::rms(&x) - (0.4f64.powi(2) / 2.0 + 0.1f64.powi(2) / 2.0).sqrt()).abs() < 1e-6
        );
        let (l, r) = measure::deinterleave(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!((l, r), (vec![1.0, 3.0], vec![2.0, 4.0]));
    }

    #[test]
    fn ci16_le_reads_i_then_q_at_full_scale() {
        // SigMF 1.0 ci16_le: little-endian int16 pairs, I first. 0x8000 is -32768 and 0x7FFF is 32767.
        let x = sigmf::decode_ci16_le(&[0x00, 0x80, 0xFF, 0x7F, 0x00, 0x40, 0x00, 0xC0, 0x01]);
        assert_eq!(x.len(), 2);
        assert_eq!((x[0].re, x[0].im), (-1.0, 32767.0 / 32768.0));
        assert_eq!((x[1].re, x[1].im), (0.5, -0.5));
    }

    #[test]
    fn a_ci16_le_recording_reads_and_another_datatype_is_refused() {
        let dir = std::env::temp_dir().join(format!("sdrkit-test-ci16-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let meta = dir.join("r.sigmf-meta");
        let data = dir.join("r.sigmf-data");
        let json = |datatype: &str| {
            format!(
                r#"{{"global":{{"core:datatype":"{datatype}","core:sample_rate":2100000.0}},"captures":[{{"core:frequency":202928000.0}}]}}"#
            )
        };
        std::fs::write(&meta, json("ci16_le")).unwrap();
        std::fs::write(&data, [0x00, 0x40, 0x00, 0xC0]).unwrap();
        let rec = sigmf::read(&meta).unwrap();
        assert_eq!((rec.rate_hz, rec.centre_hz), (2.1e6, 202.928e6));
        assert_eq!(rec.samples, vec![Complex32::new(0.5, -0.5)]);
        std::fs::write(&meta, json("ci8")).unwrap();
        let e = sigmf::read(&meta).unwrap_err();
        assert!(
            matches!(e, Error::Datatype { ref found, .. } if found == "ci8"),
            "{e}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sigmf_round_trips_and_the_fm_vector_reads() {
        let x = vec![Complex32::new(0.25, -1.5), Complex32::new(3.0, 1e-3)];
        assert_eq!(sigmf::decode_cf32_le(&sigmf::encode_cf32_le(&x)), x);
        let rec = sigmf::read_vector("fm", "fm_stereo_rds_500k").expect("the FM vector");
        assert_eq!(rec.rate_hz, 500_000.0);
        assert_eq!(rec.centre_hz, 100e6);
        assert_eq!(rec.samples.len(), 2_000_000);
        assert!(sigmf::read_expected("fm").unwrap()["rds"]["pi"] == "4A21");
    }
}
