//! Frame search (null symbol, PRS, integer carrier offset) and per-frame tracking (timing with SFN window placement,
//! sample clock, fine carrier offset, fades and re-acquisition).

use super::{
    cp_corr, load_window, wrap_half, FrameSoft, NullSpectrum, Ofdm, OfdmOut, Quick, SyncState,
};
use crate::tables::{self, CARRIERS, FRAME, SYMBOLS, TG, TNULL, TS, TU};
use sdrkit_core::Complex32 as C;

/// Samples a search needs in its buffer.
const NEED: usize = FRAME + TNULL + 4096 + TS;
/// From the centre of the quietest window to the useful start of the PRS.
const NULL_TO_PRS: i64 = 1832;

impl Ofdm {
    /// One search step; returns whether it changed anything (so the caller runs again).
    pub(super) fn search(&mut self, out: &mut OfdmOut) -> bool {
        // The search runs on the oldest NEED samples held, at positions fixed by the signal (not by how it was cut into
        // pieces): after a failure it skips ahead by half a frame times the failures.
        if self.wait_until > self.buf_start {
            let to = self.wait_until.min(self.end);
            self.trim_to(to);
            if self.buf_start < self.wait_until {
                return false;
            }
        }
        if self.buf.len() < NEED {
            return false;
        }
        // Null symbol: the quietest 2048-sample window over one frame, in steps of 128, from 64-sample block powers.
        let blocks = (FRAME + TU) / 64;
        let mut bp = std::mem::take(&mut self.mags);
        bp.clear();
        for b in 0..blocks {
            bp.push(
                self.buf[b * 64..(b + 1) * 64]
                    .iter()
                    .map(|c| c.norm_sqr())
                    .sum(),
            );
        }
        let frame_power: f32 = bp[..FRAME / 64].iter().sum::<f32>() / FRAME as f32;
        let mut best = (f32::MAX, 0usize);
        let mut w: f32 = bp[..32].iter().sum();
        for p in (0..FRAME).step_by(128) {
            let b = p / 64;
            if p > 0 {
                w += bp[b + 30] + bp[b + 31] - bp[b - 2] - bp[b - 1];
            }
            if w < best.0 {
                best = (w, p);
            }
        }
        self.mags = bp;
        let quiet_mean = best.0 / TU as f32;
        if quiet_mean.is_nan() || quiet_mean > 0.6 * frame_power || frame_power <= 0.0 {
            // No DAB frame here: keep one frame, count the rest as searched without a null.
            let drop = NEED - FRAME;
            self.quiet += drop as f64;
            let from = self.buf_start + drop as i64;
            self.trim_to(from);
            self.state = SyncState::Searching;
            return true;
        }
        self.quiet = 0.0;
        self.state = SyncState::Syncing;
        let est = (best.1 + 1024) as i64 + NULL_TO_PRS;
        let (lo, hi) = match self.quick {
            Some(q) => (q.shift - 2, q.shift + 2),
            None => (-80, 80),
        };
        // Rank (position, shift) pairs. The differential correlation picks the shifts quickly (it does not depend on the window's
        // timing); the impulse response measures them.
        let mut cands: Vec<(f32, i64, i32)> = Vec::new();
        let mut pos = est - 1400;
        while pos <= est + 1000 {
            let abs = self.buf_start + pos;
            self.load_fft(abs, 0.0);
            let mut top = [(0f32, 0i32); 3];
            for s in lo..=hi {
                let d = self.diff_corr(s);
                if d > top[2].0 {
                    top[2] = (d, s);
                    top.sort_by(|a, b| b.0.total_cmp(&a.0));
                }
            }
            for &(d, s) in &top {
                if d <= 0.0 {
                    continue;
                }
                self.prs_correlate(s);
                let mut peak = 0f32;
                let mut sum = 0f32;
                for v in &self.scratch {
                    let p = v.norm_sqr();
                    sum += p;
                    peak = peak.max(p);
                }
                let mean = sum / TU as f32;
                cands.push((peak / mean.max(1e-30), abs, s));
            }
            pos += 128;
        }
        cands.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut found: Option<(f32, i64, i32, usize)> = None;
        for &(_, abs, s) in cands.iter().take(3) {
            self.load_fft(abs, 0.0);
            self.prs_correlate(s);
            let (peak, ratio, _) = self.peak_median();
            if found.is_none_or(|f| ratio > f.0) {
                found = Some((ratio, abs, s, peak));
            }
        }
        match found {
            Some((ratio, abs, s, peak)) if ratio >= 25.0 => {
                let d = if peak >= TU / 2 {
                    peak as i64 - TU as i64
                } else {
                    peak as i64
                };
                let u = abs + d;
                self.pred = u as f64;
                self.cfo = s as f64;
                self.stats.prs_peak = ratio;
                self.state = SyncState::Locked;
                self.first_frame = true;
                self.fades = 0;
                self.last_found = None;
                self.failed = 0;
                if self.quick.take().is_some() {
                    self.stats.relocks += 1;
                    self.stats.reacquiring = false;
                } else {
                    self.eps = 0.0;
                }
                if self.stats.lock_time_s == 0.0 {
                    self.stats.lock_time_s = (u - 3160).max(0) as f64 / tables::RATE;
                }
                // Keep from a little before the frame's null.
                self.trim_to(u - 3160 - 1100);
                true
            }
            _ => {
                self.failed = (self.failed + 1).min(6);
                self.wait_until = self.buf_start + (FRAME as i64 / 2) * self.failed as i64;
                if let Some(q) = self.quick.as_mut() {
                    q.tries += 1;
                    if q.tries >= 6 {
                        self.quick = None;
                        self.stats.reacquiring = false;
                        self.stats.reset_decoding = true;
                        self.cfo = 0.0;
                        self.eps = 0.0;
                        self.have_slow = false;
                    }
                } else {
                    // A null without a PRS: try the mirrored spectrum next time.
                    self.mirror = !self.mirror;
                    for c in self.buf.iter_mut() {
                        *c = c.conj();
                    }
                }
                let _ = out;
                true
            }
        }
    }

    /// The differential correlation of the spectrum in `self.win` with the PRS at integer shift `s`, normalised.
    fn diff_corr(&self, s: i32) -> f32 {
        let mut acc = C::default();
        let mut pw = 0f32;
        let mut prev: Option<(C, C)> = None;
        for k in (-tables::HALF..=tables::HALF).filter(|&k| k != 0) {
            let x = self.win[tables::bin(k + s)];
            let p = self.prs[tables::bin(k)];
            pw += x.norm_sqr();
            if let Some((xp, pp)) = prev {
                if k != 1 {
                    acc += x * xp.conj() * (p * pp.conj()).conj();
                }
            }
            prev = Some((x, p));
        }
        if pw > 0.0 {
            acc.norm() / pw
        } else {
            0.0
        }
    }

    /// One tracking step: demodulates the next frame when the buffer holds it. Returns whether it did anything.
    pub(super) fn track(&mut self, out: &mut OfdmOut) -> bool {
        let k = 1.0 + self.eps;
        let u_pred = self.pred;
        let need_end = (u_pred + (75 * TS) as f64 * k) as i64 + TU as i64 + 600;
        if self.end < need_end {
            return false;
        }
        let w = u_pred.round() as i64 - 64;
        if self.first_frame {
            // Fine offset from the cyclic prefixes, then the neighbours one spacing either side (the n + ½ ambiguity).
            let frac = self.cp_offset(u_pred, k);
            let base = self.cfo + wrap_half(frac - self.cfo);
            let mut best = (0f32, base);
            for c in [base, base - 1.0, base + 1.0] {
                self.load_fft(w, c);
                self.prs_correlate(0);
                let (_, ratio, _) = self.peak_median();
                if ratio > best.0 {
                    best = (ratio, c);
                }
            }
            self.cfo = best.1;
        }
        self.load_fft(w, self.cfo);
        self.prs_correlate(0);
        let impulse_peak = {
            let (peak, ratio, _) = self.peak_median();
            self.stats.prs_peak = ratio;
            (peak, ratio)
        };
        let impulse = self.pool_f.exact(TU, |b| {
            for (o, v) in b.iter_mut().zip(self.scratch.iter()) {
                *o = v.norm() / TU as f32;
            }
        });
        let found = impulse_peak.1 >= 12.0;
        // Power of the PRS's useful part.
        let prs_power = {
            let a = (w + 64 - self.buf_start).max(0) as usize;
            let b = (a + TU).min(self.buf.len());
            self.buf[a..b]
                .iter()
                .map(|c| c.norm_sqr() as f64)
                .sum::<f64>()
                / TU as f64
        };
        let u_used: f64;
        let faded;
        if found {
            let u_found = (w + self.place()) as f64;
            if self.first_frame {
                u_used = u_pred;
            } else {
                let diff = u_found - u_pred;
                if self.last_found.is_some() && diff.abs() <= 48.0 {
                    self.eps = (self.eps + 0.25 * diff / FRAME as f64).clamp(-400e-6, 400e-6);
                }
                u_used = u_found;
            }
            self.fades = 0;
            faded = false;
            self.slow_power = if self.have_slow {
                0.8 * self.slow_power + 0.2 * prs_power
            } else {
                prs_power
            };
        } else if self.have_slow && prs_power < 0.3 * self.slow_power && self.fades < 12 {
            self.fades += 1;
            u_used = u_pred;
            faded = true;
        } else {
            // Lost samples or a jump: quick re-acquisition from half a frame before the expected frame start.
            self.stats.sync = false;
            self.stats.sync_losses += 1;
            self.stats.reacquiring = true;
            self.state = SyncState::Searching;
            let expected_null = u_pred.round() as i64 - (TNULL + TG) as i64;
            self.trim_to(expected_null - FRAME as i64 / 2);
            self.quick = Some(Quick {
                tries: 0,
                shift: self.cfo.round() as i32,
            });
            self.failed = 0;
            self.wait_until = self.buf_start;
            self.since_report = 0;
            self.publish(out);
            return true;
        }
        if found && !self.first_frame {
            let frac = self.cp_offset(u_used, k);
            self.cfo += wrap_half(frac - self.cfo);
        }
        // Transmitter identification: the null symbol of this frame.
        let null_start = u_used.round() as i64 - (TNULL + TG) as i64;
        let tii_spectrum = if self.settings.tii && null_start >= self.buf_start {
            load_window(
                &self.buf,
                self.buf_start,
                null_start + (TNULL - TU) as i64 / 2,
                self.cfo,
                &mut self.win,
            );
            self.fft.process(&mut self.win);
            let win = &self.win;
            Some(self.pool_f.exact(CARRIERS + 1, |b| {
                for (i, o) in b.iter_mut().enumerate() {
                    let kk = i as i32 - 768;
                    *o = if kk == 0 {
                        0.0
                    } else {
                        win[tables::bin(kk)].norm_sqr()
                    };
                }
            }))
        } else {
            None
        };
        // Demodulate.
        let coherent = self.settings.coherent;
        self.demod
            .transform(&self.buf, self.buf_start, u_used, self.cfo, self.eps);
        let (slow_cell, slow_llr) = if self.have_slow {
            (self.slow_cell, self.slow_llr)
        } else {
            (0.0, 0.0)
        };
        let mut erased = [false; SYMBOLS];
        super::cut_symbols(&self.buf, self.buf_start, u_used, k, &mut erased);
        let demod = &mut self.demod;
        let mut measure = Default::default();
        let soft = self.pool.exact((SYMBOLS - 1) * 2 * CARRIERS, |b| {
            measure = demod.demodulate(coherent, slow_cell, slow_llr, &erased, b);
        });
        let m: super::demod::FrameMeasure = measure;
        if found {
            if self.have_slow {
                self.slow_cell = 0.8 * self.slow_cell + 0.2 * m.mean_cell;
                self.slow_llr = 0.8 * self.slow_llr + 0.2 * m.mean_llr;
            } else {
                self.slow_cell = m.mean_cell;
                self.slow_llr = m.mean_llr;
                self.have_slow = true;
            }
        }
        if let Some(power) = tii_spectrum {
            let signal_power = self.slow_power as f32 * TU as f32 / CARRIERS as f32 * TU as f32;
            out.nulls.push(NullSpectrum {
                frame: self.frames,
                power,
                signal_power,
            });
        }
        let constellation = {
            let demod = &self.demod;
            self.pool_c.exact(CARRIERS, |b| demod.constellation(b))
        };
        out.frames.push(FrameSoft {
            frame: self.frames,
            start: null_start,
            faded,
            soft,
        });
        self.frames += 1;
        self.stats.sync = found;
        self.stats.snr_db = m.snr_db;
        self.stats.detection = m.detection;
        self.stats.constellation = Some(constellation);
        self.stats.impulse = Some(impulse);
        self.last_found = if found { Some(null_start) } else { None };
        self.first_frame = false;
        self.pred = u_used + FRAME as f64 * k;
        self.since_report = 0;
        self.publish(out);
        let keep = self.pred.round() as i64 - (TNULL + TG) as i64 - 1100;
        self.trim_to(keep);
        true
    }

    /// The carrier offset modulo one spacing from the cyclic prefixes of the frame whose PRS starts at `u0`.
    fn cp_offset(&self, u0: f64, k: f64) -> f64 {
        let mut s = C::default();
        for l in 0..SYMBOLS {
            let u = (u0 + (l * TS) as f64 * k).round() as i64;
            s += cp_corr(&self.buf, self.buf_start, u);
        }
        -(s.im as f64).atan2(s.re as f64) / std::f64::consts::TAU
    }

    /// Window placement in single-frequency networks (section 4.4), from the impulse response in `self.scratch` (taken in a window
    /// starting 64 samples before the predicted start): the offset of the useful start from that window's start.
    fn place(&mut self) -> i64 {
        let n = TU as i64;
        let signed = |i: usize| {
            if i as i64 >= n / 2 {
                i as i64 - n
            } else {
                i as i64
            }
        };
        self.mags.clear();
        self.mags.extend(self.scratch.iter().map(|v| v.norm_sqr()));
        let (peak_i, peak_p) =
            self.mags.iter().enumerate().fold(
                (0usize, 0f32),
                |a, (i, &p)| if p > a.1 { (i, p) } else { a },
            );
        self.powers.clear();
        self.powers.extend_from_slice(&self.mags);
        let mid = self.mags.len() / 2;
        let med = *self
            .mags
            .select_nth_unstable_by(mid, |a, b| a.total_cmp(b))
            .1;
        let thr = (0.1 * peak_p).max(12.0 * med);
        let pk = signed(peak_i);
        const SPAN: i64 = 488;
        let paths = &mut self.paths;
        paths.clear();
        paths.extend(
            self.powers
                .iter()
                .enumerate()
                .filter(|(_, &p)| p >= thr)
                .map(|(i, &p)| (signed(i), p)),
        );
        paths.sort_unstable_by_key(|p| p.0);
        let mut near = paths.iter().filter(|p| (p.0 - pk).abs() <= SPAN);
        let e = near.next().map_or(pk, |p| p.0);
        let l = near.next_back().map_or(e, |p| p.0);
        if l - e <= SPAN - 8 {
            return e;
        }
        let mut best = (0f32, pk);
        for &(t, _) in paths.iter() {
            let en: f32 = paths
                .iter()
                .filter(|p| p.0 >= t - 16 && p.0 <= t + SPAN)
                .map(|p| p.1)
                .sum();
            if en > best.0 {
                best = (en, t);
            }
        }
        best.1
    }
}
