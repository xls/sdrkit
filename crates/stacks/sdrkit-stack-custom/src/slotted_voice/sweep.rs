//! The sweep signature of frame version 3: four chirp variants of one period (24240 samples at 48 kHz, -5 to +5 kHz), the
//! station identity that a cycle of 24 periods spells, the canceller that finds and removes the sweep in a received stream,
//! and the scanner that reads the identity from a long stretch of input without frame lock.

use super::phy::crc8;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;
use std::sync::OnceLock;

/// Samples a period.
pub const PERIOD: usize = 24_240;
/// Half a period: the span of a fast variant.
pub const HALF: usize = PERIOD / 2;
/// The sample rate of the sweep.
pub const FS: f64 = 48_000.0;
/// The sweep rate of the slow variants, Hz/s.
pub const RATE: f64 = 10_000.0 / 0.505;
/// Periods in an identity cycle.
pub const CYCLE: usize = 24;
/// The up variant.
pub const UP: u8 = 0;
/// The down variant.
pub const DOWN: u8 = 1;
/// The first fast variant.
pub const FAST1: u8 = 2;
/// The second fast variant.
pub const FAST2: u8 = 3;
/// Samples of a template kept on each side of its period, for shifted copies.
const PAD: usize = 512;

/// Sample `m` of variant `v`, counted from the start of its period; any `m` (the slow variants continue their formula, the
/// fast variants are zero outside their half).
pub fn value(v: u8, m: i64) -> Complex32 {
    let t = m as f64 / FS;
    let (amp, ph) = match v {
        UP => (1.0, -5000.0 * t + RATE * t * t / 2.0),
        DOWN => (1.0, 5000.0 * t - RATE * t * t / 2.0),
        FAST1 if (0..HALF as i64).contains(&m) => (2f64.sqrt(), -5000.0 * t + RATE * t * t),
        FAST2 if (HALF as i64..PERIOD as i64).contains(&m) => {
            let u = t - 0.2525;
            (2f64.sqrt(), -5000.0 * u + RATE * u * u)
        }
        _ => return Complex32::new(0.0, 0.0),
    };
    let a = TAU * ph.rem_euclid(1.0);
    Complex32::new((amp * a.cos()) as f32, (amp * a.sin()) as f32)
}

/// The four templates over `-PAD .. PERIOD + PAD`.
fn templates() -> &'static [Vec<Complex32>; 4] {
    static T: OnceLock<[Vec<Complex32>; 4]> = OnceLock::new();
    T.get_or_init(|| {
        std::array::from_fn(|v| {
            (-(PAD as i64)..(PERIOD + PAD) as i64)
                .map(|m| value(v as u8, m))
                .collect()
        })
    })
}

/// Sample `m` (in `-PAD .. PERIOD + PAD`) of variant `v`, from the table.
#[inline]
pub fn template(v: u8, m: i64) -> Complex32 {
    templates()[v as usize][(m + PAD as i64) as usize]
}

/// The alphabet of the sweep identity: codes 0 to 39.
pub const ALPHABET: &[u8; 40] = b" ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-/.";
/// The suffix classes 0 to 15.
pub const SUFFIXES: [&str; 16] = [
    "", "/P", "/M", "/MM", "/A", "/QRP", "/B", "/AM", "/R", "/S", "/T", "/X", "/1", "/2", "/3", "/4",
];

/// The 44-bit identity word of `callsign` (payload of class and base, then the CRC-8); `None` when it cannot be sent.
pub fn id_word(callsign: &str) -> Option<u64> {
    let call = callsign.trim().to_ascii_uppercase();
    let mut class = 0;
    let mut best = 0;
    for (i, s) in SUFFIXES.iter().enumerate().skip(1) {
        if s.len() > best && call.len() > s.len() && call.ends_with(s) {
            class = i;
            best = s.len();
        }
    }
    let base = &call[..call.len() - best];
    if base.is_empty() || base.len() > 6 {
        return None;
    }
    let mut value: u64 = 0;
    for i in 0..6 {
        let c = base.as_bytes().get(i).copied().unwrap_or(b' ');
        value = value * 40 + ALPHABET.iter().position(|&a| a == c)? as u64;
    }
    let payload = ((class as u64) << 32) | value;
    let bytes = &payload.to_be_bytes()[3..];
    Some((payload << 8) | u64::from(crc8(bytes)))
}

/// The variants of the 24 periods of the cycle of `callsign`: fast1, fast2, then 22 dibits of the word; all up when the
/// callsign cannot be sent.
pub fn id_cycle(callsign: &str) -> [u8; CYCLE] {
    let mut c = [UP; CYCLE];
    if let Some(w) = id_word(callsign) {
        c[0] = FAST1;
        c[1] = FAST2;
        for i in 0..22 {
            c[2 + i] = ((w >> (42 - 2 * i)) & 3) as u8;
        }
    }
    c
}

/// The callsign of a 44-bit word, when the CRC holds, the value is in range and the base is not all spaces.
pub fn decode_word(word: u64) -> Option<String> {
    let payload = word >> 8;
    if crc8(&payload.to_be_bytes()[3..]) != (word & 0xFF) as u8 {
        return None;
    }
    let class = (payload >> 32) as usize;
    let mut value = payload & 0xFFFF_FFFF;
    if value >= 40u64.pow(6) || class >= 16 {
        return None;
    }
    let mut base = [b' '; 6];
    for i in (0..6).rev() {
        base[i] = ALPHABET[(value % 40) as usize];
        value /= 40;
    }
    let base = String::from_utf8_lossy(&base).trim_end().to_string();
    if base.trim().is_empty() {
        return None;
    }
    Some(format!("{base}{}", SUFFIXES[class]))
}

/// The callsign that 24 variants spell when they start at the fast1, fast2 pair.
pub fn decode_cycle(v: &[u8]) -> Option<String> {
    if v.len() < CYCLE || v[0] != FAST1 || v[1] != FAST2 {
        return None;
    }
    let w = v[2..CYCLE].iter().fold(0u64, |a, &d| (a << 2) | u64::from(d & 3));
    decode_word(w)
}

/// What the canceller found in one period.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeriodFit {
    /// The variant.
    pub variant: u8,
    /// The shift of the template that fit best, samples.
    pub lag: i64,
    /// The complex amplitude removed.
    pub gain: Complex32,
    /// The power of the winner over the mean of the other three, dB.
    pub margin_db: f64,
}

/// Finds the variant and shift (within `lag ± width`) that fit the period of `x` starting at `start`, and subtracts it.
/// Only the samples of `x` inside the period are used and changed; a period cut by the ends of `x` is fitted on its part.
pub fn cancel_period(x: &mut [Complex32], start: i64, centre_lag: i64, width: i64) -> Option<PeriodFit> {
    let lo = start.max(0);
    let hi = (start + PERIOD as i64).min(x.len() as i64);
    if hi - lo < (PERIOD / 4) as i64 {
        return None;
    }
    let mut best: Option<(u8, i64, Complex32, f64)> = None;
    let mut powers = [0f64; 4];
    for v in 0..4u8 {
        let mut vbest = (0i64, Complex32::new(0.0, 0.0), 0f64, 0f64);
        for lag in centre_lag - width..=centre_lag + width {
            let (mut acc, mut e) = (Complex32::new(0.0, 0.0), 0f32);
            for n in lo..hi {
                let t = template(v, n - start - lag);
                acc += x[n as usize] * t.conj();
                e += t.norm_sqr();
            }
            let p = acc.norm_sqr() as f64 / f64::from(e.max(1e-9));
            if p > vbest.2 {
                vbest = (lag, acc / e.max(1e-9), p, f64::from(e));
            }
        }
        powers[v as usize] = vbest.2;
        if best.is_none_or(|b| vbest.2 > b.3) {
            best = Some((v, vbest.0, vbest.1, vbest.2));
        }
    }
    let (v, lag, gain, p) = best?;
    let others = (powers.iter().sum::<f64>() - p) / 3.0;
    for n in lo..hi {
        x[n as usize] -= gain * template(v, n - start - lag);
    }
    Some(PeriodFit {
        variant: v,
        lag,
        gain,
        margin_db: 10.0 * (p / others.max(1e-12)).log10(),
    })
}

/// Collects the variants of successive periods and reads the identity: a callsign counts once it decoded twice at least
/// 12 periods apart, with a mean margin of 5 dB over the last cycle.
#[derive(Clone, Debug, Default)]
pub struct IdReader {
    seen: Vec<(u8, f64)>,
    first: Option<(String, usize)>,
    /// The callsign read.
    pub callsign: Option<String>,
}

impl IdReader {
    /// Takes the fit of the next period.
    pub fn push(&mut self, fit: &PeriodFit) {
        self.seen.push((fit.variant, fit.margin_db));
        let n = self.seen.len();
        if n < CYCLE {
            return;
        }
        let w = &self.seen[n - CYCLE..];
        let v: Vec<u8> = w.iter().map(|s| s.0).collect();
        let Some(call) = decode_cycle(&v) else {
            return;
        };
        let margin = w.iter().map(|s| s.1).sum::<f64>() / CYCLE as f64;
        if margin < 5.0 {
            return;
        }
        match &self.first {
            Some((c, at)) if *c == call && n - at >= 12 => self.callsign = Some(call),
            Some((c, _)) if *c == call => {}
            _ => self.first = Some((call, n)),
        }
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        *self = IdReader::default();
    }
}

/// Input the scanner keeps, samples (36 s).
pub const SCAN_KEEP: usize = 36 * 48_000;
/// Input before the first scan, samples (26 s).
pub const SCAN_FIRST: u64 = 26 * 48_000;
/// Input between scans, samples (10 s).
pub const SCAN_EVERY: u64 = 10 * 48_000;
/// The carrier shifts the scanner tries, samples of correlation lag (a carrier of `f` Hz moves the slow variants by
/// `-f FS / RATE` samples): -700 to +700 in steps of 4, about +-290 Hz.
const SCAN_SHIFT: i64 = 700;

/// The identity cycles a scan found: the callsign and the sample where its 24 periods start.
pub type Cycles = Vec<(String, i64)>;

/// Reads the station identity from the sweep alone, for signals too weak to lock to: it keeps the last 36 s of input and,
/// from 26 s on, every 10 s, on a helper thread, correlates the stored input with the four variants, finds the period start and the carrier
/// shift together on the correlation folded over the periods, takes the strongest variant of each period, and reads the
/// identity cycle from them.
/// A scan that falls due while the last one still runs waits, with the input of its due time, and starts when that one
/// ends; a later due scan takes its place. [`Scanner::finish`] runs the scan that waits, so the last scan due always runs.
#[derive(Debug, Default)]
pub struct Scanner {
    buf: Vec<Complex32>,
    total: u64,
    next: u64,
    /// The scan in progress on a helper thread: its result and the input sample where its stretch starts.
    running: Option<(std::sync::mpsc::Receiver<Cycles>, i64)>,
    /// The scan due while another ran: its stretch and the input sample where the stretch starts.
    waiting: Option<(Vec<Complex32>, i64)>,
    /// The cycles decoded so far: the callsign and the input sample where its 24 periods start.
    heard: Vec<(String, i64)>,
    /// The callsign read: decoded from two cycles at least 12 periods apart.
    pub callsign: Option<String>,
}

impl Scanner {
    /// Takes input at 48 kHz; scans when one is due.
    pub fn push(&mut self, x: &[Complex32]) {
        if self.next == 0 {
            self.next = SCAN_FIRST;
        }
        self.buf.extend_from_slice(x);
        if self.buf.len() > SCAN_KEEP + SCAN_EVERY as usize {
            let drop = self.buf.len() - SCAN_KEEP;
            self.buf.drain(..drop);
        }
        self.total += x.len() as u64;
        if let Some((rx, at)) = &self.running {
            match rx.try_recv() {
                Ok(found) => {
                    let at = *at;
                    self.running = None;
                    self.take(found, at);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.running = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.total >= self.next {
            self.next = self.total + SCAN_EVERY;
            let from = self.buf.len().saturating_sub(SCAN_KEEP);
            let at = self.total as i64 - (self.buf.len() - from) as i64;
            self.waiting = Some((self.buf[from..].to_vec(), at));
        }
        if self.running.is_none() {
            if let Some((stretch, at)) = self.waiting.take() {
                // The scan runs on a helper thread, so the receiving thread does not stall on its transforms.
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(scan(&stretch));
                });
                self.running = Some((rx, at));
            }
        }
    }

    /// Waits for the scan in progress, if any, runs the one waiting, and takes what they found.
    pub fn finish(&mut self) {
        if let Some((rx, at)) = self.running.take() {
            if let Ok(found) = rx.recv() {
                self.take(found, at);
            }
        }
        if let Some((stretch, at)) = self.waiting.take() {
            let found = scan(&stretch);
            self.take(found, at);
        }
    }

    /// Takes the cycles that a scan of the stretch from input sample `at` found.
    fn take(&mut self, found: Cycles, at: i64) {
        for (c, start) in found {
            let start = at + start;
            if self.heard.iter().any(|(h, s)| *h == c && (start - s).abs() >= 12 * PERIOD as i64) {
                self.callsign = Some(c.clone());
            }
            if !self.heard.iter().any(|(h, s)| *h == c && (start - s).abs() < PERIOD as i64 / 2) {
                self.heard.push((c, start));
            }
        }
        let oldest = self.total as i64 - 2 * SCAN_KEEP as i64;
        self.heard.retain(|h| h.1 >= oldest);
    }

    /// Forgets the input and the callsign.
    pub fn reset(&mut self) {
        *self = Scanner::default();
    }
}

/// The identity cycles in the sweep of `x` (48 kHz), found as [`Scanner`] describes: for every 24 periods in a row whose
/// variants, turned to start at the fast pair, decode with a mean margin of 5 dB, the callsign and the sample of `x` where
/// the first of the 24 periods starts.
pub fn scan(x: &[Complex32]) -> Cycles {
    let n = x.len();
    if n < (CYCLE + 2) * PERIOD {
        return Vec::new();
    }
    let size = (n + PERIOD).next_power_of_two();
    let mean = x.iter().sum::<Complex32>() / n as f32;
    let mut fx = vec![Complex32::new(0.0, 0.0); size];
    for (o, v) in fx.iter_mut().zip(x) {
        *o = v - mean;
    }
    Fft::forward(size).process(&mut fx);
    let lags = n - PERIOD + 1;
    // The correlation power of each variant at each lag, and its fold over the periods.
    let mut power: Vec<Vec<f32>> = Vec::with_capacity(4);
    let mut fold = vec![vec![0f64; PERIOD]; 4];
    for v in 0..4u8 {
        let mut ft = vec![Complex32::new(0.0, 0.0); size];
        for (m, o) in ft.iter_mut().enumerate().take(PERIOD) {
            *o = value(v, m as i64);
        }
        Fft::forward(size).process(&mut ft);
        for (a, b) in ft.iter_mut().zip(&fx) {
            *a = b * a.conj();
        }
        Fft::inverse(size).process(&mut ft);
        let p: Vec<f32> = ft[..lags].iter().map(|c| c.norm_sqr()).collect();
        for (l, &q) in p.iter().enumerate() {
            fold[v as usize][l % PERIOD] += f64::from(q);
        }
        power.push(p);
    }
    // The fast variants move by half as much as the slow ones, the down variant the other way.
    let shift = |v: usize, d: i64| -> i64 {
        match v {
            0 => d,
            1 => -d,
            _ => d / 2,
        }
    };
    let mut best = (0f64, 0usize, 0i64);
    let mut d = -SCAN_SHIFT;
    while d <= SCAN_SHIFT {
        for g in 0..PERIOD {
            let mut sum = 0f64;
            for (v, f) in fold.iter().enumerate() {
                sum += f[(g as i64 + shift(v, d)).rem_euclid(PERIOD as i64) as usize];
            }
            if sum > best.0 {
                best = (sum, g, d);
            }
        }
        d += 4;
    }
    let (_, g, d) = best;
    // The winner of each period, at the peak near its expected lag.
    let mut fits: Vec<(i64, u8, f64)> = Vec::new();
    let mut start = g as i64 - PERIOD as i64;
    while start + PERIOD as i64 + SCAN_SHIFT < lags as i64 {
        if start - SCAN_SHIFT >= 0 {
            let mut p = [0f64; 4];
            for (v, pv) in p.iter_mut().enumerate() {
                let at = start + shift(v, d);
                *pv = (at - 3..=at + 3).filter(|&l| (0..lags as i64).contains(&l)).map(|l| f64::from(power[v][l as usize])).fold(0.0, f64::max);
            }
            let (w, &pw) = p.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap_or((0, &0.0));
            let others = (p.iter().sum::<f64>() - pw) / 3.0;
            fits.push((start, w as u8, 10.0 * (pw / others.max(1e-30)).log10()));
        }
        start += PERIOD as i64;
    }
    let mut out = Vec::new();
    // Every 24 periods in a row hold one whole cycle, turned: it starts at a fast pair (the dibits can make one too).
    for w in fits.windows(CYCLE) {
        let margin = w.iter().map(|f| f.2).sum::<f64>() / CYCLE as f64;
        if margin < 5.0 {
            continue;
        }
        for i in (0..CYCLE).filter(|&i| w[i].1 == FAST1 && w[(i + 1) % CYCLE].1 == FAST2) {
            let v: Vec<u8> = (0..CYCLE).map(|k| w[(i + k) % CYCLE].1).collect();
            if let Some(c) = decode_cycle(&v) {
                out.push((c, w[0].0));
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identity_of_a_callsign() {
        assert_eq!(id_word("LY2ABC/P"), Some(0x14D_2AE8_93E1));
        assert_eq!(id_cycle("ly2abc/p"), [2, 3, 0, 1, 1, 0, 3, 1, 0, 2, 2, 2, 3, 2, 2, 0, 2, 1, 0, 3, 3, 2, 0, 1]);
        assert_eq!(decode_word(0x14D_2AE8_93E1).as_deref(), Some("LY2ABC/P"));
        assert_eq!(decode_word(0x14D_2AE8_93E0), None);
        assert_eq!(decode_cycle(&id_cycle("OH2XYZ/MM")).as_deref(), Some("OH2XYZ/MM"));
        assert_eq!(id_cycle(""), [UP; CYCLE]);
        assert_eq!(id_word("ABCDEFG"), None);
        assert_eq!(id_word("AB_C"), None);
    }

    #[test]
    fn variants_are_apart_and_of_equal_energy() {
        let t: Vec<Vec<Complex32>> = (0..4u8).map(|v| (0..PERIOD as i64).map(|m| value(v, m)).collect()).collect();
        let e: Vec<f64> = t.iter().map(|x| x.iter().map(|c| c.norm_sqr() as f64).sum()).collect();
        for v in 0..4 {
            assert!((e[v] / e[0] - 1.0).abs() < 0.01, "{v}");
            for w in v + 1..4 {
                let c: Complex32 = t[v].iter().zip(&t[w]).map(|(a, b)| a * b.conj()).sum();
                assert!((c.norm() as f64) < 0.02 * e[0], "{v} {w}");
            }
        }
    }

    #[test]
    fn the_canceller_removes_a_shifted_variant() {
        let mut x: Vec<Complex32> = (0..PERIOD as i64 + 2000)
            .map(|n| {
                if (1000..1000 + PERIOD as i64).contains(&n) {
                    value(DOWN, n - 1000 - 3) * Complex32::new(0.2, 0.1)
                } else {
                    Complex32::new(0.0, 0.0)
                }
            })
            .collect();
        let before: f32 = x.iter().map(|c| c.norm_sqr()).sum();
        let fit = cancel_period(&mut x, 1000, 0, 5).unwrap();
        assert_eq!((fit.variant, fit.lag), (DOWN, 3));
        let after: f32 = x.iter().map(|c| c.norm_sqr()).sum();
        assert!(after < before * 1e-6, "{after}");
        assert!(fit.margin_db > 20.0);
    }
}

