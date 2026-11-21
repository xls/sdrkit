//! CIS-12: 12 DPSK carriers at 700 + 200 k Hz, 120 Bd, with an unmodulated pilot at 3300 Hz. The receiver finds the pilot (the
//! frequency error) and the carriers in the averaged spectrum, the symbol clock from the energy of the carrier integrations, and
//! reads the phase step of each carrier as a digit; the PSK order comes from the moments of the steps.

use super::{Out, Sub};
use crate::typed::{CisHfCarrierRow, CIS_HF_CARRIERS};
use num_complex::Complex64;
use sdrkit_algo_dsp::bandscan::BandScan;
use sdrkit_algo_modem::pskorder::PskOrder;
use sdrkit_core::publish::Published;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

const RATE: f64 = super::RATE;
/// Samples a symbol.
const SYMBOL: usize = 100;
/// The integration of a carrier, samples (5 ms: 200 Hz apart carriers are orthogonal over it).
const INTEGRATE: usize = 60;
/// Where the integration starts in a symbol.
const LEAD: usize = 20;
/// Samples kept for the first symbols after a lock (1.25 s).
const KEEP: usize = 15_000;
/// The pilot, Hz.
pub const PILOT_HZ: f64 = 3300.0;

/// The carrier `k` (0 to 11), Hz.
pub fn carrier(k: usize) -> f64 {
    700.0 + 200.0 * k as f64
}

/// The `cis12` receiver.
pub struct Cis12 {
    scan: BandScan,
    offset: Option<f64>,
    offset_since: u64,
    pilot_fails: u32,
    on: [bool; 12],
    locked: bool,
    lock_fails: u32,
    noise: f32,
    snr: Option<f64>,
    n: u64,
    rot: [Complex64; 12],
    step: [Complex64; 12],
    ring: Vec<[Complex64; 12]>,
    at: usize,
    sums: [Complex64; 12],
    /// Per kept sample: the 12 integrations that end there.
    hist: Vec<[Complex32; 12]>,
    hist_base: u64,
    energy: Vec<f32>,
    clock: Option<f64>,
    next: f64,
    since_clock: u32,
    prev: Option<[Complex32; 12]>,
    order: PskOrder,
    groups: usize,
    off_run: u32,
    symbols: i64,
}

impl Default for Cis12 {
    fn default() -> Self {
        Self::new()
    }
}

impl Cis12 {
    /// The receiver.
    pub fn new() -> Self {
        Cis12 {
            scan: BandScan::new(4096, RATE, 0.2).expect("a valid spectrum"),
            offset: None,
            offset_since: 0,
            pilot_fails: 0,
            on: [false; 12],
            locked: false,
            lock_fails: 0,
            noise: 0.0,
            snr: None,
            n: 0,
            rot: [Complex64::new(1.0, 0.0); 12],
            step: [Complex64::new(1.0, 0.0); 12],
            ring: vec![[Complex64::default(); 12]; INTEGRATE],
            at: 0,
            sums: [Complex64::default(); 12],
            hist: Vec::new(),
            hist_base: 0,
            energy: Vec::new(),
            clock: None,
            next: 0.0,
            since_clock: 0,
            prev: None,
            order: PskOrder::new(0.995),
            groups: 0,
            off_run: 0,
            symbols: 0,
        }
    }

    fn set_offset(&mut self, off: f64) {
        if self.offset.is_none() {
            self.offset_since = self.n;
        }
        self.offset = Some(off);
        for k in 0..12 {
            self.step[k] = Complex64::from_polar(1.0, -TAU * (carrier(k) + off) / RATE);
        }
    }

    /// The looks at the spectrum: the pilot, then the carriers.
    fn look(&mut self) {
        let noise = self.scan.noise(-5900.0, 5900.0);
        self.noise = noise;
        let pilot = self.scan.peak(3100.0, 3500.0).filter(|&(f, p)| {
            p > 30.0 * noise && self.scan.at(f - 25.0) < 0.1 * p && self.scan.at(f + 25.0) < 0.1 * p
        });
        match pilot {
            Some((f, _)) => {
                self.pilot_fails = 0;
                let off = f - PILOT_HZ;
                let new = match self.offset {
                    Some(o) if (off - o).abs() <= 3.0 => o + 0.2 * (off - o),
                    _ => off,
                };
                self.set_offset(new);
            }
            None => {
                self.pilot_fails += 1;
                if self.pilot_fails >= 9 {
                    self.offset = None;
                    self.locked = false;
                    self.clock = None;
                }
            }
        }
        let Some(off) = self.offset else { return };
        let bins = self.scan.band_bins(-50.0, 50.0) as f32;
        let mut ons = 0;
        let (mut mid, mut car) = (0.0f64, 0.0f64);
        for k in 0..12 {
            let f = carrier(k) + off;
            self.on[k] = self.scan.band_power(f - 50.0, f + 50.0) as f32 > 40.0 * noise * bins;
            ons += usize::from(self.on[k]);
            car += self.scan.band_power(f - 20.0, f + 20.0);
            if k < 11 {
                mid += self.scan.band_power(f + 80.0, f + 120.0);
            }
        }
        let comb = mid * 12.0 < 0.3 * car * 11.0;
        if ons >= 6 && comb {
            self.lock_fails = 0;
            self.locked = true;
        } else {
            self.lock_fails += 1;
            if self.lock_fails >= 6 {
                self.locked = false;
                self.clock = None;
            }
        }
        self.snr = Some(self.scan.snr_db(
            carrier(0) + off - 100.0,
            carrier(11) + off + 100.0,
            noise,
        ));
    }

    /// The energy of the integrations that end at kept sample `n` (absolute).
    fn energy_at(&self, n: u64) -> f64 {
        let i = (n - self.hist_base) as usize;
        f64::from(self.energy[i])
    }

    /// The best symbol start (absolute, fractional) of the `nsym` symbols that end before the newest sample.
    fn find_clock(&self, nsym: usize) -> Option<f64> {
        let end = self.n;
        let first = end.checked_sub((nsym * SYMBOL + SYMBOL) as u64)?;
        if first < self.hist_base {
            return None;
        }
        // Energy at each even start offset 0 to 98.
        let e: Vec<f64> = (0..SYMBOL / 2)
            .map(|j| {
                let o = (2 * j) as u64;
                (0..nsym)
                    .map(|s| self.energy_at(first + o + (s * SYMBOL + LEAD + INTEGRATE - 1) as u64))
                    .sum()
            })
            .collect();
        // A circular moving sum over +-10 samples (5 entries either side).
        let m = e.len();
        let sums: Vec<f64> = (0..m)
            .map(|i| (0..11).map(|d| e[(i + m + d - 5) % m]).sum())
            .collect();
        let (i, _) = sums.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
        Some(first as f64 + (2 * i) as f64)
    }

    /// Reads the symbol that starts at `s` (absolute).
    fn symbol(&mut self, s: f64, out: &mut Out) {
        let at = (s.round() as u64) + (LEAD + INTEGRATE - 1) as u64;
        if at < self.hist_base || at >= self.n {
            return;
        }
        let cur = self.hist[(at - self.hist_base) as usize];
        let sigma2 = f64::from(self.noise) / 1536.0;
        let e: f64 = cur.iter().map(|v| f64::from(v.norm_sqr())).sum();
        if e < 6.0 * 12.0 * INTEGRATE as f64 * sigma2 {
            self.prev = None;
            self.off_run += 1;
            if self.off_run == 60 {
                out.chars.push_str("\nsymbols stopped\n");
                self.groups = 0;
            }
            return;
        }
        self.off_run = 0;
        let Some(prev) = self.prev.replace(cur) else {
            return;
        };
        let steps: Vec<Complex32> = (0..12).map(|k| cur[k] * prev[k].conj()).collect();
        // A carrier is read in this symbol when both integrations stand 6 times above the noise of one integration.
        let floor = 6.0 * INTEGRATE as f64 * sigma2;
        let live: Vec<bool> = (0..12)
            .map(|k| f64::from(cur[k].norm_sqr()) > floor && f64::from(prev[k].norm_sqr()) > floor)
            .collect();
        for (k, u) in steps.iter().enumerate() {
            if live[k] {
                self.order.push(*u);
            }
        }
        self.order.decay();
        let order = self.order.order().unwrap_or(4);
        let mut group = String::with_capacity(13);
        let mut unsure = 0;
        for (k, u) in steps.iter().enumerate() {
            if !live[k] {
                group.push('-');
                continue;
            }
            let v = f64::from(u.arg()).rem_euclid(TAU) / TAU * f64::from(order);
            let d = v.round();
            if (v - d).abs() > 0.25 {
                unsure += 1;
            }
            let d = (d as u32) % order;
            group.push(char::from_digit(d, 10).unwrap_or('?'));
        }
        if unsure > 2 {
            group.push('?');
        }
        out.chars.push_str(&group);
        out.good += 1;
        self.symbols += 1;
        self.groups += 1;
        out.chars.push(if self.groups.is_multiple_of(6) {
            '\n'
        } else {
            ' '
        });
    }

    fn sample(&mut self, x: Complex32) {
        self.n += 1;
        let x = Complex64::new(f64::from(x.re), f64::from(x.im));
        let mut row = [Complex32::default(); 12];
        let mut e = 0.0f32;
        let old = self.ring[self.at];
        let mut new = [Complex64::default(); 12];
        for k in 0..12 {
            let v = x * self.rot[k];
            self.rot[k] *= self.step[k];
            new[k] = v;
            self.sums[k] += v - old[k];
            let s = Complex32::new(self.sums[k].re as f32, self.sums[k].im as f32);
            row[k] = s;
            e += s.norm_sqr();
        }
        if self.n.is_multiple_of(1024) {
            for r in &mut self.rot {
                *r /= r.norm();
            }
        }
        self.ring[self.at] = new;
        self.at = (self.at + 1) % INTEGRATE;
        if self.at == 0 {
            for k in 0..12 {
                self.sums[k] = self.ring.iter().map(|r| r[k]).sum();
            }
        }
        self.hist.push(row);
        self.energy.push(e);
        if self.hist.len() >= 2 * KEEP {
            let cut = self.hist.len() - KEEP;
            self.hist.drain(..cut);
            self.energy.drain(..cut);
            self.hist_base += cut as u64;
        }
    }
}

impl Sub for Cis12 {
    fn push(&mut self, x: &[Complex32], out: &mut Out) {
        for &v in x {
            if self.scan.push(std::slice::from_ref(&v)) > 0 && self.scan.frames() >= 3 {
                self.look();
            }
            self.sample(v);
            if !self.locked {
                continue;
            }
            match self.clock {
                None => {
                    // The first lock: the newest 25 symbols (made once the pilot gave the offset), then back to the start of the
                    // kept samples.
                    if self.n < self.offset_since + (26 * SYMBOL) as u64 {
                        continue;
                    }
                    if let Some(c) = self.find_clock(25) {
                        let k = ((c - self.hist_base as f64) / SYMBOL as f64)
                            .floor()
                            .max(0.0);
                        self.clock = Some(c);
                        self.next = c - k * SYMBOL as f64;
                        self.since_clock = 0;
                        self.prev = None;
                    }
                }
                Some(_) => {
                    while self.next + (LEAD + INTEGRATE) as f64 <= self.n as f64 {
                        let s = self.next;
                        self.symbol(s, out);
                        self.next += SYMBOL as f64;
                        self.since_clock += 1;
                    }
                    if self.since_clock >= 24 {
                        self.since_clock = 0;
                        if let Some(c) = self.find_clock(24) {
                            // The difference to the running clock, folded into one symbol; half of it, at most 5.
                            let d = (c - self.next).rem_euclid(SYMBOL as f64);
                            let d = if d > SYMBOL as f64 / 2.0 {
                                d - SYMBOL as f64
                            } else {
                                d
                            };
                            self.next += (0.5 * d).clamp(-5.0, 5.0);
                        }
                    }
                }
            }
        }
    }
    fn state(&self) -> u8 {
        if self.locked && self.clock.is_some() && self.symbols > 0 && self.off_run < 60 {
            2
        } else if self.offset.is_some() {
            1
        } else {
            0
        }
    }
    fn summary(&self) -> String {
        match (self.offset, self.locked) {
            (Some(o), true) => format!(
                "CIS-12: pilot {:.1} Hz, {} carriers, {}",
                PILOT_HZ + o,
                self.on.iter().filter(|&&v| v).count(),
                modulation(self.order.order())
            ),
            (Some(o), false) => format!("pilot {:.1} Hz, carriers not found", PILOT_HZ + o),
            _ => "searching".to_string(),
        }
    }
    fn measure<'a>(&'a self, m: &mut super::Measures<'a>) {
        m.pilot_hz = self.offset.map(|o| ((PILOT_HZ + o) * 10.0).round() / 10.0);
        let seen = self.on.iter().filter(|&&v| v).count() as i64;
        m.carriers = Some(seen);
        m.symbol_rate = self.locked.then_some(120.0);
        m.modulation = Some(modulation(self.order.order()));
        m.m2 = Some((self.order.m2() * 100.0).round() / 100.0);
        m.m4 = Some((self.order.m4() * 100.0).round() / 100.0);
    }
    fn rows(&self, res: &mut Published) {
        for k in 0..12 {
            CIS_HF_CARRIERS.put(
                res,
                &format!("{}", k + 1),
                &CisHfCarrierRow {
                    number: (k + 1) as i64,
                    freq_hz: carrier(k) + self.offset.unwrap_or(0.0),
                    on: self.on[k],
                },
            );
        }
    }
    fn snr_db(&self) -> Option<f64> {
        self.snr.filter(|_| self.offset.is_some())
    }
    fn offset_hz(&self) -> Option<f64> {
        self.offset.map(|o| (o * 100.0).round() / 100.0)
    }
}

/// The name of a differential PSK of `order`.
fn modulation(order: Option<u32>) -> &'static str {
    match order {
        Some(2) => "DBPSK",
        Some(4) => "DQPSK",
        Some(8) => "D8PSK",
        _ => "unknown",
    }
}
