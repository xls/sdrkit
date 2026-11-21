//! The decoders of the family: one slot of audio in, decode records out.

pub mod ft;
pub mod ftmem;
pub mod js8;
pub mod msk144;
pub mod q65;
pub mod slot;
pub mod slow;
pub mod slowmodes;

pub use slot::{Decode, Slot};

#[cfg(test)]
pub(crate) mod testutil {
    use super::slot::Slot;
    use crate::encode::{audio, Transmission};
    use crate::mode::Sub;
    use std::sync::Arc;

    /// Gaussian noise of sigma `s`, seeded.
    pub fn noise(n: usize, s: f32, seed: u64) -> Vec<f32> {
        let mut a = vec![0f32; n];
        if let Ok(mut w) = sdrkit_algo_sim::noise::Awgn::with_power((s * s) as f64, seed) {
            w.process_real(&mut a);
        }
        a
    }

    /// A slot of `sub` with these stations (text, Hz, SNR dB, DT s, mirrored), noise of sigma 0.05.
    pub fn slot_of(sub: Sub, stations: &[(&str, f64, f64, f64, bool)], seed: u64) -> Slot {
        let (w0, w1) = sub.window();
        let n = ((w1 - w0) * 8000.0) as usize;
        let mut a = noise(n, 0.05, seed);
        for &(text, hz, snr, dt, mir) in stations {
            let amp = (2.0 * 10f64.powf(snr / 10.0) * 0.05 * 0.05 * 2500.0 / 4000.0).sqrt() as f32;
            let t = Transmission {
                sub,
                hz,
                amp,
                mirrored: mir,
            };
            let s = audio(&t, text, 8000.0, &Default::default()).unwrap();
            let start = ((sub.t0() + dt - w0) * 8000.0).round() as i64;
            for (i, v) in s.iter().enumerate() {
                let j = start + i as i64;
                if j >= 0 && (j as usize) < n {
                    a[j as usize] += v;
                }
            }
        }
        Slot {
            sub,
            utc: 0.0,
            audio: Arc::new(a),
            rate: 8000.0,
            t_first: w0,
            search: false,
            dial_hz: 0.0,
            index: 0,
            early: false,
            seq: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::slowmodes::*;
    use super::testutil::slot_of;
    use crate::mode::{Mode, Sub};
    use crate::proto::msg77::CallHashes;
    use crate::proto::wsprmsg::WsprHashes;

    fn run(sub: Sub, st: &[(&str, f64, f64, f64, bool)], seed: u64) -> Vec<super::Decode> {
        let slot = slot_of(sub, st, seed);
        let mut h = CallHashes::default();
        let mut w = WsprHashes::default();
        let codes = Default::default();
        let mut ctx = SlowCtx {
            hashes: &mut h,
            wspr: &mut w,
            mirrored: true,
            codes: &codes,
        };
        let t = std::time::Instant::now();
        let d = match sub.mode {
            Mode::Wspr => decode_wspr(&slot, &mut ctx),
            Mode::Jt4 => decode_jt4(&slot, &mut ctx),
            Mode::Jt9 => decode_jt9(&slot, &mut ctx),
            _ => decode_jt65(&slot, &mut ctx),
        };
        eprintln!("{} {} ms", sub.label(), t.elapsed().as_millis());
        for x in &d {
            eprintln!(
                "{:>4} {:6.2} {:8.2} {} {}",
                x.snr, x.dt, x.hz, x.text, x.dbm
            );
        }
        for s in st {
            let x = d
                .iter()
                .find(|x| x.text == s.0)
                .unwrap_or_else(|| panic!("{}: {} missing", sub.label(), s.0));
            assert!((x.hz - s.1).abs() < 2.0, "{} hz {}", s.0, x.hz);
            assert!((x.dt - s.3).abs() < 0.2, "{} dt {}", s.0, x.dt);
        }
        d
    }

    #[test]
    fn wspr_stations() {
        let st = [
            ("K1ABC FN42 37", 1450.0, -12.0, 0.0, false),
            ("G4XYZ IO91 23", 1480.0, -18.0, 0.5, false),
            ("PJ4/K1ABC 30", 1520.0, -20.0, -0.5, false),
            ("DL1XYZ/P 10", 1550.0, -14.0, 1.0, false),
        ];
        run(Sub::new(Mode::Wspr), &st, 2);
    }

    #[test]
    fn jt_family_stations() {
        let st = [
            ("CQ K1ABC FN42", 600.0, -12.0, 0.0, false),
            ("K1ABC W9XYZ -12", 1000.0, -14.0, 0.3, false),
            ("CQ DX G4ABC IO91", 1800.0, -13.0, 0.1, false),
        ];
        run(Sub::new(Mode::Jt9), &st, 3);
        run(Sub::new(Mode::Jt4), &st, 4);
        run(Sub::new(Mode::Jt65), &st, 5);
    }
}
