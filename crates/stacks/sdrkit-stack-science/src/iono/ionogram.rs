//! The ionogram of a sounder: each sweep dechirped (multiplied by the conjugate of the model chirp, so an echo delayed by τ becomes a
//! tone at −r·τ), averaged down by a boxcar, and transformed over 20 kHz of sweep into a delay profile that is added to the 50 kHz
//! column it belongs to.

use super::scale::{col_hz, IonogramDb, COLS, COL_HZ, ROWS, ROW_KM, ROW_S};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Complex32;
use std::fmt::Write;

/// Samples of the dechirp between two restarts of the oscillator from the exact chirp phase.
const CHUNK: usize = 64;
/// Points of the delay-profile FFT.
pub const PROFILE_LEN: usize = 512;

/// The sums of a sounder's ionogram: power per cell (200 delay rows by 600 columns of 50 kHz) and profiles added per column.
#[derive(Clone, Debug, PartialEq)]
pub struct Ionogram {
    /// Power sums, row by row.
    pub sum: Vec<f32>,
    /// Profiles added per column.
    pub count: Vec<u32>,
}

impl Default for Ionogram {
    fn default() -> Self {
        Ionogram {
            sum: vec![0.0; ROWS * COLS],
            count: vec![0; COLS],
        }
    }
}

impl Ionogram {
    /// Columns with data.
    pub fn swept(&self) -> usize {
        self.count.iter().filter(|&&c| c > 0).count()
    }

    /// The ionogram in dB over each column's median ("not swept" columns are marked).
    pub fn db(&self) -> IonogramDb {
        let mut out = IonogramDb::new();
        let mut scratch = Vec::with_capacity(ROWS);
        let mut col_v = Vec::with_capacity(ROWS);
        for col in 0..COLS {
            if self.count[col] == 0 {
                continue;
            }
            out.swept[col] = true;
            col_v.clear();
            col_v.extend((0..ROWS).map(|r| self.sum[r * COLS + col] as f64));
            let med = sdrkit_algo_dsp::stats::quantile_f64(&col_v, 0.5, &mut scratch)
                .unwrap_or(1.0)
                .max(1e-30);
            for (row, &v) in col_v.iter().enumerate() {
                out.db[row * COLS + col] = (10.0 * (v / med).max(1e-30).log10()) as f32;
            }
        }
        out
    }

    /// The CSV of the cells with data: a header, then `frequency_mhz,delay_ms,group_range_km,db` per cell, column by column.
    pub fn csv(&self) -> String {
        let db = self.db();
        let mut s = String::from("frequency_mhz,delay_ms,group_range_km,db\n");
        for col in 0..COLS {
            if !db.swept[col] {
                continue;
            }
            for row in 0..ROWS {
                let _ = writeln!(
                    s,
                    "{:.3},{:.3},{:.1},{:.2}",
                    col_hz(col) / 1e6,
                    row as f64 * ROW_S * 1e3,
                    row as f64 * ROW_KM,
                    db.at(row, col)
                );
            }
        }
        s
    }

    /// A grey picture of 600 × 200 pixels, lowest delay at the bottom: 0 dB black, `db_top` and above white, columns never swept
    /// black. Its PNG comes from `sdrkit_codec_image::png::encode`.
    pub fn picture(&self, db_top: f64) -> sdrkit_codec_image::Raster {
        let db = self.db();
        let mut px = vec![0u8; ROWS * COLS];
        for row in 0..ROWS {
            for col in 0..COLS {
                if !db.swept[col] {
                    continue;
                }
                let v = (db.at(row, col) as f64 / db_top).clamp(0.0, 1.0);
                px[(ROWS - 1 - row) * COLS + col] = (v * 255.0).round() as u8;
            }
        }
        sdrkit_codec_image::Raster::from_vec(
            sdrkit_codec_image::PixelFormat::Grey8,
            COLS as u32,
            ROWS as u32,
            px,
        )
        .expect("600 by 200 grey pixels")
    }
}

/// The dechirp of one sweep of rate `r` that passed 0 Hz at `start_s`.
pub struct Dechirper {
    rate: f64,
    centre: f64,
    r: f64,
    start_s: f64,
    osc: Nco,
    decim: usize,
    acc: Complex32,
    acc_n: usize,
    seg: Vec<Complex32>,
    seg_first_s: f64,
    fft: Fft,
    rows: [usize; ROWS],
}

impl Dechirper {
    /// A dechirper for samples at `rate_hz` around `centre_hz`, for a sweep of `r_hz_s` that passed 0 Hz at `start_s`. The boxcar
    /// brings the rate down to about `PROFILE_LEN` samples per 20 kHz of sweep, so one profile point is close to 0.05 ms of delay.
    pub fn new(rate_hz: f64, centre_hz: f64, r_hz_s: f64, start_s: f64) -> Dechirper {
        let want = PROFILE_LEN as f64 * r_hz_s * ROW_S;
        let decim = ((rate_hz / want).round() as usize).max(1);
        let fd = rate_hz / decim as f64;
        let mut rows = [0usize; ROWS];
        for (row, b) in rows.iter_mut().enumerate() {
            // An echo delayed by τ is a tone at −r·τ: a negative bin.
            let bin = (r_hz_s * row as f64 * ROW_S / (fd / PROFILE_LEN as f64)).round() as usize;
            *b = (PROFILE_LEN - bin % PROFILE_LEN) % PROFILE_LEN;
        }
        Dechirper {
            rate: rate_hz,
            centre: centre_hz,
            r: r_hz_s,
            start_s,
            osc: Nco::new(rate_hz, 0.0),
            decim,
            acc: Complex32::new(0.0, 0.0),
            acc_n: 0,
            seg: Vec::with_capacity(PROFILE_LEN),
            seg_first_s: 0.0,
            fft: Fft::forward(PROFILE_LEN),
            rows,
        }
    }

    /// The sweep's start, s.
    pub fn start_s(&self) -> f64 {
        self.start_s
    }

    /// A new centre: a profile under way is dropped.
    pub fn retune(&mut self, centre_hz: f64) {
        self.centre = centre_hz;
        self.seg.clear();
        self.acc_n = 0;
        self.acc = Complex32::new(0.0, 0.0);
    }

    /// Takes samples whose first is at signal time `t_s`, adding finished profiles to `ion` when their whole 20 kHz lies inside
    /// `span_hz`.
    pub fn push(&mut self, x: &[Complex32], t_s: f64, span_hz: (f64, f64), ion: &mut Ionogram) {
        let dt = 1.0 / self.rate;
        for (ci, chunk) in x.chunks(CHUNK).enumerate() {
            let t = t_s + (ci * CHUNK) as f64 * dt;
            // The exact phase of the conjugate model chirp at the chunk start, and its frequency in the middle of the chunk.
            let u = t - self.start_s;
            let cycles = -(0.5 * self.r * u * u - self.centre * u);
            let um = u + 0.5 * chunk.len() as f64 * dt;
            self.osc.set_freq(-(self.r * um - self.centre));
            self.osc.set_phase_cycles(cycles.rem_euclid(1.0));
            let mut buf = [Complex32::new(0.0, 0.0); CHUNK];
            let tmp = &mut buf[..chunk.len()];
            self.osc.mix(chunk, tmp);
            for (i, &v) in tmp.iter().enumerate() {
                if self.seg.is_empty() && self.acc_n == 0 {
                    self.seg_first_s = t + i as f64 * dt;
                }
                self.acc += v;
                self.acc_n += 1;
                if self.acc_n == self.decim {
                    self.seg.push(self.acc / self.decim as f32);
                    self.acc = Complex32::new(0.0, 0.0);
                    self.acc_n = 0;
                    if self.seg.len() == PROFILE_LEN {
                        self.profile(span_hz, ion);
                    }
                }
            }
        }
    }

    fn profile(&mut self, span_hz: (f64, f64), ion: &mut Ionogram) {
        let dur = PROFILE_LEN as f64 * self.decim as f64 / self.rate;
        let f0 = self.r * (self.seg_first_s - self.start_s);
        let f1 = f0 + self.r * dur;
        let mid = 0.5 * (f0 + f1);
        let inside = f0 >= span_hz.0 && f1 <= span_hz.1 && mid >= 0.0;
        if inside {
            let col = (mid / COL_HZ) as usize;
            if col < COLS {
                self.fft.process(&mut self.seg);
                for (row, &b) in self.rows.iter().enumerate() {
                    ion.sum[row * COLS + col] += self.seg[b].norm_sqr();
                }
                ion.count[col] += 1;
            }
        }
        self.seg.clear();
    }
}
