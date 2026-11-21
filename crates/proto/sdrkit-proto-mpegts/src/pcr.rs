//! The system time clock (ISO/IEC 13818-1 2.4.2): PCR values paired with their arrival positions, a line fitted over the last few
//! seconds (following drift up to ±500 ppm), and a new segment at a jump.

use std::collections::VecDeque;

/// Seconds of PCR history the line is fitted over.
pub const FIT_SECONDS: f64 = 4.0;
/// A PCR this far (seconds) off the fitted line starts a new segment.
pub const JUMP_S: f64 = 0.1;

/// The PCRs that a new clock has room for: the fit window at one PCR every 7.8 ms. A stream with its PCRs at least that
/// far apart never makes the store grow.
pub const FIT_POINTS: usize = 512;

/// The clock of one PCR PID.
#[derive(Clone, Debug)]
pub struct Stc {
    points: VecDeque<(f64, f64)>,
    /// PCRs seen.
    pub count: u64,
    /// The first PCR (27 MHz units).
    pub first: Option<u64>,
    /// Segments started (jumps and discontinuities).
    pub segments: u64,
    /// Set when the last PCR started a new segment; media after it carry a discontinuity.
    pub jumped: bool,
    /// The PCR seconds unwrap.
    wrap_base: f64,
    last_raw: Option<f64>,
}

/// The PCR wrap in seconds (2^33 / 90 000).
pub const PCR_WRAP_S: f64 = 95_443.717_688_888_9;

impl Default for Stc {
    fn default() -> Self {
        Stc {
            points: VecDeque::with_capacity(FIT_POINTS),
            count: 0,
            first: None,
            segments: 0,
            jumped: false,
            wrap_base: 0.0,
            last_raw: None,
        }
    }
}

impl Stc {
    /// A new clock, with room for [`FIT_POINTS`] PCRs.
    pub fn new() -> Self {
        Self::default()
    }

    /// The PCRs that the store holds without growing.
    pub fn capacity(&self) -> usize {
        self.points.capacity()
    }

    /// A PCR (27 MHz units) arriving at `arrival` (signal seconds). `discontinuity` is the adaptation field's flag.
    pub fn push(&mut self, pcr: u64, arrival: f64, discontinuity: bool) {
        self.count += 1;
        self.first.get_or_insert(pcr);
        let raw = crate::packet::pcr_seconds(pcr);
        if let Some(l) = self.last_raw {
            if raw < l - PCR_WRAP_S / 2.0 {
                self.wrap_base += PCR_WRAP_S;
            }
        }
        self.last_raw = Some(raw);
        let v = raw + self.wrap_base;
        self.jumped = false;
        let off_line = self
            .predict(arrival)
            .is_some_and(|p| (p - v).abs() > JUMP_S);
        if discontinuity || off_line {
            self.points.clear();
            self.segments += 1;
            self.jumped = true;
        }
        self.points.push_back((arrival, v));
        while self.points.len() > 2 && arrival - self.points[0].0 > FIT_SECONDS {
            self.points.pop_front();
        }
    }

    /// The fitted clock at `arrival`: (offset, slope) of the least-squares line.
    fn fit(&self) -> Option<(f64, f64, f64)> {
        let n = self.points.len();
        if n == 0 {
            return None;
        }
        let x0 = self.points[0].0;
        let y0 = self.points[0].1;
        if n == 1 {
            return Some((x0, y0, 1.0));
        }
        let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
        for &(x, y) in &self.points {
            let (dx, dy) = (x - x0, y - y0);
            sx += dx;
            sy += dy;
            sxx += dx * dx;
            sxy += dx * dy;
        }
        let nf = n as f64;
        let den = nf * sxx - sx * sx;
        let slope = if den.abs() > 1e-18 {
            ((nf * sxy - sx * sy) / den).clamp(1.0 - 500e-6, 1.0 + 500e-6)
        } else {
            1.0
        };
        let icpt = (sy - slope * sx) / nf;
        Some((x0, y0 + icpt, slope))
    }

    /// The STC (seconds on the PCR timeline) at arrival time `arrival`.
    pub fn predict(&self, arrival: f64) -> Option<f64> {
        if self.points.len() < 2 {
            return self.points.back().map(|&(x, y)| y + (arrival - x));
        }
        self.fit().map(|(x0, y0, s)| y0 + s * (arrival - x0))
    }

    /// The fitted rate of the clock against arrival time (1.0 nominal).
    pub fn slope(&self) -> f64 {
        self.fit().map_or(1.0, |f| f.2)
    }
}

/// Pacing of a file or time-shift buffer by its PCR (ISO/IEC 13818-1 2.4.2): the first PID seen carrying a PCR is the clock; a packet more
/// than 2 s late, a backward PCR or a jump of more than elapsed + 1 h re-anchors.
#[derive(Clone, Debug, Default)]
pub struct Pacer {
    pid: Option<u16>,
    anchor: Option<(f64, f64)>,
    last: f64,
    /// Re-anchors so far.
    pub reanchors: u64,
}

impl Pacer {
    /// A new pacer.
    pub fn new() -> Self {
        Self::default()
    }
    /// A PCR of `pid` (seconds) seen at wall time `wall`. Returns how long to wait (seconds) before passing the packet on.
    pub fn pcr(&mut self, pid: u16, pcr_s: f64, wall: f64) -> f64 {
        if *self.pid.get_or_insert(pid) != pid {
            return 0.0;
        }
        let Some((w0, p0)) = self.anchor else {
            self.anchor = Some((wall, pcr_s));
            self.last = pcr_s;
            return 0.0;
        };
        let elapsed = wall - w0;
        let due = w0 + (pcr_s - p0);
        if pcr_s < self.last || pcr_s - p0 > elapsed + 3600.0 || wall - due > 2.0 {
            self.reanchors += 1;
            self.anchor = Some((wall, pcr_s));
            self.last = pcr_s;
            return 0.0;
        }
        self.last = pcr_s;
        (due - wall).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_store_does_not_grow_at_a_pcr_every_24_ms() {
        let mut s = Stc::new();
        let room = s.capacity();
        assert!(room >= FIT_POINTS);
        for i in 0..2000 {
            let t = i as f64 * 0.024;
            s.push(((10.0 + t) * 27e6) as u64, t, false);
        }
        assert_eq!(s.capacity(), room);
        assert!((s.slope() - 1.0).abs() < 1e-6);
    }
    #[test]
    fn follows_drift_and_jumps() {
        let mut s = Stc::new();
        for i in 0..100 {
            let t = i as f64 * 0.04;
            s.push(((10.0 + t * (1.0 + 300e-6)) * 27e6) as u64, t, false);
        }
        assert!((s.slope() - (1.0 + 300e-6)).abs() < 1e-6);
        assert!((s.predict(4.0).unwrap() - (10.0 + 4.0 * 1.0003)).abs() < 1e-4);
        s.push((50.0 * 27e6) as u64, 4.04, false);
        assert!(s.jumped);
        assert_eq!(s.segments, 1);
        let mut p = Pacer::new();
        assert_eq!(p.pcr(1, 100.0, 0.0), 0.0);
        assert!((p.pcr(1, 100.5, 0.1) - 0.4).abs() < 1e-9);
        assert_eq!(p.pcr(1, 99.0, 0.2), 0.0);
        assert_eq!(p.reanchors, 1);
    }
}
