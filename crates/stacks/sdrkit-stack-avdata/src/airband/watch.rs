//! The band watch: the carriers in the whole capture as lines of a peak-held spectrum, each moved to the nearest 8.33 kHz
//! raster channel, with the listed channels left out.

use super::names::{name, STEP_833};
use super::{BankState, Seen, SeenList};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};

/// The band watch.
pub struct Watch {
    rate: f64,
    centre: f64,
    n: usize,
    fft: Option<Fft>,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    work: Vec<Complex32>,
    hold: Vec<f32>,
    decay: f32,
    t: f64,
    next_frame: f64,
    next_report: f64,
    tune_err: f64,
    known: bool,
    listed: Vec<f64>,
    seen: Vec<Seen>,
}

impl Default for Watch {
    fn default() -> Self {
        Watch {
            rate: 0.0,
            centre: 0.0,
            n: 0,
            fft: None,
            win: Vec::new(),
            buf: Vec::new(),
            work: Vec::new(),
            hold: Vec::new(),
            decay: 1.0,
            t: 0.0,
            next_frame: 0.0,
            next_report: 0.5,
            tune_err: 0.0,
            known: false,
            listed: Vec::new(),
            seen: Vec::new(),
        }
    }
}

impl Watch {
    fn frame(&mut self) {
        let n = self.n;
        if self.buf.len() < n {
            return;
        }
        let start = self.buf.len() - n;
        self.work.clear();
        self.work
            .extend(self.buf[start..].iter().zip(&self.win).map(|(x, w)| x * *w));
        if let Some(f) = self.fft.as_mut() {
            f.process(&mut self.work);
        }
        for (h, v) in self.hold.iter_mut().zip(&self.work) {
            *h = (*h * self.decay).max(v.norm_sqr());
        }
    }

    fn report(&mut self) {
        let n = self.n as i64;
        let binw = self.rate / self.n as f64;
        let p = |k: i64| self.hold[k.rem_euclid(n) as usize] as f64;
        let r9 = (9e3 / binw).ceil() as i64;
        let r45 = (4.5e3 / binw).floor() as i64;
        let lim = ((0.45 * self.rate) / binw) as i64;
        let mut lines: Vec<(f64, f64)> = Vec::new();
        let mut around: Vec<f64> = Vec::with_capacity(2 * r9 as usize + 1);
        for k in -lim..=lim {
            let x = p(k);
            if x <= p(k - 1) || x < p(k + 1) {
                continue;
            }
            // The noise around: the lower quartile of +-9 kHz (voice sidebands, peak-held, fill much of that span. The noise
            // between them does not move).
            around.clear();
            around.extend((k - r9..=k + r9).filter(|&j| (j - k).abs() > 1).map(p));
            let q = around.len() / 4;
            around.select_nth_unstable_by(q, |a, b| a.total_cmp(b));
            let noise = around[q].max(1e-30);
            if 10.0 * (x / noise).log10() < 11.0 {
                continue;
            }
            let side = |s: i64| (r45..=r9).map(|d| p(k + s * d)).fold(0.0f64, f64::max);
            if 10.0 * (x / side(1).max(1e-30)).log10() < 9.0
                && 10.0 * (x / side(-1).max(1e-30)).log10() < 9.0
            {
                continue;
            }
            // The line's frequency between bins: a parabola through the log powers of the peak and its neighbours.
            let (a, b, c) = (p(k - 1).max(1e-30).ln(), x.ln(), p(k + 1).max(1e-30).ln());
            let den = a - 2.0 * b + c;
            let frac = if den < 0.0 {
                (0.5 * (a - c) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let f = self.centre + (k as f64 + frac) * binw;
            lines.push((f, 10.0 * (x / noise).log10()));
        }
        // The tuning error: the receiver's when known, else the shift that puts most lines on the 25 kHz raster.
        let (err, approx) = if self.known {
            (self.tune_err, false)
        } else {
            // The residual of a line from the 25 kHz raster at a trial error.
            let res = |f: f64, e: f64| {
                let x = (f - e) / 25e3;
                (x - x.round()) * 25e3
            };
            // Most lines within 450 Hz. Of equal counts, the smallest spread.
            let mut best = (0usize, f64::INFINITY, 0.0f64);
            let mut e = -12e3;
            while e <= 12e3 {
                let r: Vec<f64> = lines
                    .iter()
                    .map(|(f, _)| res(*f, e))
                    .filter(|r| r.abs() < 450.0)
                    .collect();
                let spread: f64 = r.iter().map(|v| v.abs()).sum();
                if r.len() > best.0 || (r.len() == best.0 && spread < best.1) {
                    best = (r.len(), spread, e);
                }
                e += 50.0;
            }
            // Then the median residual of those lines.
            let mut r: Vec<f64> = lines
                .iter()
                .map(|(f, _)| res(*f, best.2))
                .filter(|r| r.abs() < 450.0)
                .collect();
            r.sort_by(|a, b| a.total_cmp(b));
            let fine = if r.is_empty() { 0.0 } else { r[r.len() / 2] };
            (best.2 + fine, true)
        };
        let now = self.t;
        for e in self.seen.iter_mut() {
            e.active = false;
        }
        for (f, ex) in lines {
            let fc = f - err;
            let k = ((fc - 118e6) / STEP_833).round();
            let ch = 118e6 + k * STEP_833;
            if (ch - fc).abs() > 450.0 {
                continue;
            }
            if self.listed.iter().any(|l| (l - ch).abs() < 1.2e3) {
                continue;
            }
            let on25 = ((ch / 25e3) - (ch / 25e3).round()).abs() < 1e-6;
            let chr = if on25 { (ch / 1e3).round() * 1e3 } else { ch };
            match self
                .seen
                .iter_mut()
                .find(|s| (s.freq_hz - chr).abs() < 10.0)
            {
                Some(s) => {
                    s.reports += 1;
                    s.last_s = now;
                    s.active = true;
                    s.excess_db = s.excess_db.max(ex as f32);
                    s.approximate = approx;
                }
                None => self.seen.push(Seen {
                    freq_hz: chr,
                    is833: !on25,
                    name: name(chr, !on25),
                    excess_db: ex as f32,
                    reports: 1,
                    last_s: now,
                    active: true,
                    approximate: approx,
                }),
            }
        }
        self.seen.sort_by_key(|s| std::cmp::Reverse(s.reports));
        self.seen.truncate(200);
    }
}

impl Component for Watch {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.airband_watch",
            vec![Port::iq("in"), Port::typed::<BankState>("state")],
            vec![Port::typed::<SeenList>("seen").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.t = 0.0;
        self.next_frame = 0.0;
        self.next_report = 0.5;
        self.seen.clear();
        self.fft = None;
        if self.rate.is_finite() && self.rate >= super::MIN_RATE_HZ {
            let mut n = 256;
            while self.rate / n as f64 > 600.0 {
                n *= 2;
            }
            self.n = n;
            self.fft = Some(Fft::forward(n));
            self.win = (0..n)
                .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos()) as f32)
                .collect();
            self.hold = vec![0.0; n];
            // Peak hold with a 6 s half life, one frame every 50 ms.
            self.decay = 0.5f32.powf(0.05 / 6.0);
        }
        Ok(vec![Format::new(Kind::typed::<SeenList>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.items::<BankState>(1) {
            self.tune_err = s.value.tune_error_hz;
            self.known = s.value.known;
            self.listed = s.value.channels.iter().map(|c| c.freq_hz).collect();
        }
        if self.fft.is_none() {
            return;
        }
        let x = io.iq(0);
        let mut out = None;
        for &v in x {
            self.buf.push(v);
            self.t += 1.0 / self.rate;
            if self.t >= self.next_frame {
                self.next_frame += 0.05;
                self.frame();
                let keep = self.n;
                if self.buf.len() > 4 * keep {
                    self.buf.drain(..self.buf.len() - keep);
                }
            }
            if self.t >= self.next_report {
                self.next_report += 0.5;
                self.report();
                out = Some(SeenList {
                    entries: self.seen.clone(),
                });
            }
        }
        if self.buf.len() > 2 * self.n {
            let d = self.buf.len() - self.n;
            self.buf.drain(..d);
        }
        if let Some(s) = out {
            let t = self.t;
            io.items_out::<SeenList>(0)
                .push(Timed { time: t, value: s });
        }
    }
}
