//! The interference hunter: it learns the highest level of each bin, then reports bins that rise over it, with a likely
//! source, and a meter that follows one peak so a user can walk towards the source.

use crate::grid::{Grid, UNCOVERED_DB};
use crate::lin;

sdrkit_core::choice_enum! {
    /// A likely source of interference.
    pub enum SourceClass {
        /// A raised floor across much of the band: a switching power supply, a plasma screen, or arcing on a power line.
        BroadbandNoise = ("broadband_noise", "Broadband noise"),
        /// Evenly spaced narrow peaks: the harmonics of a switching power supply, a clock, or an LED driver.
        HarmonicComb = ("harmonic_comb", "Harmonic comb"),
        /// One steady narrow carrier: an oscillator or a clock harmonic in nearby electronics.
        NarrowCarrier = ("narrow_carrier", "Narrow carrier"),
        /// A wide flat signal: a digital transmitter such as Wi-Fi, a mobile network, or a data link.
        WidebandDigital = ("wideband_digital", "Wideband digital"),
        /// Comes and goes: a short-range transmitter such as a key fob, a weather sensor, or telemetry.
        Intermittent = ("intermittent", "Intermittent"),
        /// A modulated signal that was not there before.
        Unknown = ("unknown", "Unknown"),
    }
}

impl SourceClass {
    /// Plain words about what it probably is.
    pub fn describe(self) -> &'static str {
        match self {
            SourceClass::BroadbandNoise => {
                "Broadband noise: likely a switching power supply, a plasma screen, or arcing on a power line"
            }
            SourceClass::HarmonicComb => {
                "Evenly spaced peaks: likely the harmonics of a switching power supply, a clock, or an LED driver"
            }
            SourceClass::NarrowCarrier => {
                "A steady narrow carrier: likely an oscillator or a clock harmonic in nearby electronics"
            }
            SourceClass::WidebandDigital => {
                "A wide flat signal: likely a digital transmitter such as Wi-Fi, a mobile network, or a data link"
            }
            SourceClass::Intermittent => {
                "Comes and goes: likely a short-range transmitter such as a key fob, a weather sensor, or telemetry"
            }
            SourceClass::Unknown => "A modulated signal that was not there before",
        }
    }
}

/// New interference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interference {
    /// An id.
    pub id: u64,
    /// Low edge, Hz.
    pub low_hz: f64,
    /// High edge, Hz.
    pub high_hz: f64,
    /// The strongest bin, Hz.
    pub peak_hz: f64,
    /// How far the strongest bin stands over what was learnt, dB.
    pub rise_db: f32,
    /// The likely source.
    pub class: SourceClass,
    /// First seen, s.
    pub first_seen_s: f64,
    /// Last seen, s.
    pub last_seen_s: f64,
    /// Sweeps seen in.
    pub sweeps_seen: u64,
    /// Sweeps since first seen.
    pub sweeps_total: u64,
}

sdrkit_core::choice_enum! {
    /// Which way the level goes.
    pub enum Trend {
        /// About the same.
        Steady = ("steady", "Steady"),
        /// Getting louder (closer).
        Louder = ("louder", "Louder"),
        /// Getting quieter (further).
        Quieter = ("quieter", "Quieter"),
    }
}

/// The loud/quiet meter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meter {
    /// The peak followed, Hz.
    pub hz: f64,
    /// Its level now, dBFS.
    pub level_db: f32,
    /// Its rise over what was learnt, dB.
    pub rise_db: f32,
    /// The smoothed level, dBFS.
    pub smoothed_db: f32,
    /// The trend of the smoothed level.
    pub trend: Trend,
    /// The highest level so far, dBFS.
    pub max_db: f32,
    /// 0 (nothing over the band) to 1 (40 dB over).
    pub needle: f32,
    /// A tone for audio feedback, Hz (300 to 2000, higher when louder).
    pub tone_hz: f32,
}

/// The hunter's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct HunterConfig {
    /// Sweeps learnt before hunting.
    pub learn_sweeps: u32,
    /// A bin this far over what was learnt is new, dB.
    pub margin_db: f32,
    /// A rise over this share of the band is broadband noise.
    pub broadband_share: f64,
    /// The meter follows its peak within this distance per frame, Hz.
    pub follow_hz: f64,
}

impl Default for HunterConfig {
    fn default() -> Self {
        HunterConfig {
            learn_sweeps: 5,
            margin_db: 6.0,
            broadband_share: 0.2,
            follow_hz: 20e3,
        }
    }
}

/// The interference hunter.
#[derive(Clone, Debug)]
pub struct Hunter {
    grid: Grid,
    cfg: HunterConfig,
    base: Vec<f32>,
    learnt: u32,
    found: Vec<Interference>,
    next_id: u64,
    target: Option<f64>,
    follow: Option<f64>,
    meter: Option<Meter>,
    runs: Vec<(usize, usize, usize)>,
    seen: Vec<(f64, f64, f64, f32, SourceClass)>,
    narrow: Vec<usize>,
    comb: Vec<bool>,
    matched: Vec<bool>,
}

impl Hunter {
    /// A hunter over `grid`.
    pub fn new(grid: Grid, cfg: HunterConfig) -> Self {
        Hunter {
            base: vec![UNCOVERED_DB; grid.bins],
            grid,
            cfg,
            learnt: 0,
            found: Vec::new(),
            next_id: 1,
            target: None,
            follow: None,
            meter: None,
            runs: Vec::new(),
            seen: Vec::new(),
            narrow: Vec::new(),
            comb: Vec::new(),
            matched: Vec::new(),
        }
    }

    /// True once the band is learnt.
    pub fn learnt(&self) -> bool {
        self.learnt >= self.cfg.learn_sweeps
    }

    /// Starts learning again.
    pub fn relearn(&mut self) {
        self.base.iter_mut().for_each(|b| *b = UNCOVERED_DB);
        self.learnt = 0;
        self.found.clear();
        self.follow = None;
        self.meter = None;
    }

    /// The interference found, newest last.
    pub fn found(&self) -> &[Interference] {
        &self.found
    }

    /// Follows this frequency with the meter (`None`: the strongest new interference).
    pub fn set_target(&mut self, hz: Option<f64>) {
        self.target = hz;
        self.follow = hz;
        self.meter = None;
    }

    /// The meter, once it follows something.
    pub fn meter(&self) -> Option<&Meter> {
        self.meter.as_ref()
    }

    fn baseline_at(&self, hz: f64) -> f32 {
        self.grid
            .index_of(hz)
            .map(|k| self.base[k])
            .filter(|&b| b > UNCOVERED_DB)
            .unwrap_or(f32::NAN)
    }

    /// A stitched sweep at `now_s`.
    pub fn update(&mut self, now_s: f64, sweep_db: &[f32]) {
        let n = self.grid.bins.min(sweep_db.len());
        if !self.learnt() {
            for (b, &x) in self.base.iter_mut().zip(&sweep_db[..n]) {
                if x > UNCOVERED_DB {
                    *b = b.max(x);
                }
            }
            self.learnt += 1;
            return;
        }
        let grid = self.grid;
        let edge = |k: usize| grid.low_hz + k as f64 * grid.bin_hz;
        let base = &self.base;
        let margin = self.cfg.margin_db;
        let over = |k: usize| {
            sweep_db[k] > UNCOVERED_DB && base[k] > UNCOVERED_DB && sweep_db[k] > base[k] + margin
        };
        let mut runs = std::mem::take(&mut self.runs);
        runs.clear();
        let mut k = 0;
        while k < n {
            if over(k) {
                let a = k;
                let mut p = k;
                while k < n && over(k) {
                    if sweep_db[k] > sweep_db[p] {
                        p = k;
                    }
                    k += 1;
                }
                runs.push((a, k, p));
            } else {
                k += 1;
            }
        }
        let covered = (0..n)
            .filter(|&k| sweep_db[k] > UNCOVERED_DB && base[k] > UNCOVERED_DB)
            .count();
        let raised: usize = runs.iter().map(|r| r.1 - r.0).sum();
        let mut seen = std::mem::take(&mut self.seen);
        seen.clear();
        if covered > 0 && raised as f64 >= self.cfg.broadband_share * covered as f64 {
            let a = runs.first().map_or(0, |r| r.0);
            let b = runs.last().map_or(0, |r| r.1);
            let p = runs
                .iter()
                .map(|r| r.2)
                .max_by(|&x, &y| sweep_db[x].total_cmp(&sweep_db[y]))
                .unwrap_or(a);
            seen.push((
                edge(a),
                edge(b),
                grid.centre_hz(p),
                sweep_db[p] - base[p],
                SourceClass::BroadbandNoise,
            ));
        } else {
            // Narrow runs evenly spaced form a comb.
            let narrow = &mut self.narrow;
            narrow.clear();
            narrow.extend((0..runs.len()).filter(|&i| runs[i].1 - runs[i].0 <= 3));
            let comb = &mut self.comb;
            comb.clear();
            comb.resize(runs.len(), false);
            if narrow.len() >= 3 {
                for w in narrow.windows(3) {
                    let (p0, p1, p2) = (
                        runs[w[0]].2 as i64,
                        runs[w[1]].2 as i64,
                        runs[w[2]].2 as i64,
                    );
                    let (s1, s2) = (p1 - p0, p2 - p1);
                    if s1 >= 3 && (s1 - s2).abs() <= 2 {
                        comb[w[0]] = true;
                        comb[w[1]] = true;
                        comb[w[2]] = true;
                    }
                }
            }
            for (i, &(a, b, p)) in runs.iter().enumerate() {
                let width = b - a;
                let class = if comb[i] {
                    SourceClass::HarmonicComb
                } else if width <= 3 {
                    SourceClass::NarrowCarrier
                } else {
                    let mean = (a..b).map(|j| lin(sweep_db[j])).sum::<f32>() / width as f32;
                    if sweep_db[p] - crate::db(mean) < 3.0 {
                        SourceClass::WidebandDigital
                    } else {
                        SourceClass::Unknown
                    }
                };
                seen.push((
                    edge(a),
                    edge(b),
                    grid.centre_hz(p),
                    sweep_db[p] - base[p],
                    class,
                ));
            }
        }
        let mut matched = std::mem::take(&mut self.matched);
        matched.clear();
        matched.resize(self.found.len(), false);
        for &(lo, hi, peak, rise, class) in &seen {
            let hit = self.found.iter().position(|f| {
                peak >= f.low_hz - self.grid.bin_hz && peak <= f.high_hz + self.grid.bin_hz
                    || f.peak_hz >= lo && f.peak_hz <= hi
            });
            match hit {
                Some(i) if !matched[i] => {
                    matched[i] = true;
                    let f = &mut self.found[i];
                    f.low_hz = lo;
                    f.high_hz = hi;
                    f.peak_hz = peak;
                    f.rise_db = rise;
                    f.last_seen_s = now_s;
                    f.sweeps_seen += 1;
                    f.sweeps_total += 1;
                    if f.class != SourceClass::Intermittent {
                        f.class = class;
                    }
                }
                Some(_) => {}
                None => {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.found.push(Interference {
                        id,
                        low_hz: lo,
                        high_hz: hi,
                        peak_hz: peak,
                        rise_db: rise,
                        class,
                        first_seen_s: now_s,
                        last_seen_s: now_s,
                        sweeps_seen: 1,
                        sweeps_total: 1,
                    });
                    matched.push(true);
                }
            }
        }
        for (i, f) in self.found.iter_mut().enumerate() {
            if !matched[i] {
                f.sweeps_total += 1;
            }
            if f.sweeps_total >= 4 && (f.sweeps_seen as f64) < 0.5 * f.sweeps_total as f64 {
                f.class = SourceClass::Intermittent;
            }
        }
        self.runs = runs;
        self.seen = seen;
        self.matched = matched;
        if self.found.len() > 256 {
            let drop = self.found.len() - 256;
            self.found.drain(..drop);
        }
        if self.follow.is_none() {
            self.follow = self
                .found
                .iter()
                .filter(|f| f.last_seen_s == now_s)
                .max_by(|a, b| a.rise_db.total_cmp(&b.rise_db))
                .map(|f| f.peak_hz);
        }
        if let Some(hz) = self.follow {
            let lo = self.grid.low_hz;
            let bin = self.grid.bin_hz;
            self.meter_frame(now_s, lo, bin, &sweep_db[..n], Some(hz));
        }
    }

    /// A spectrum frame for the meter (`bins_db` from `low_hz` in steps of `bin_hz`): the meter finds
    /// the peak within `follow_hz` of the one it follows and updates. `around` overrides the frequency followed.
    pub fn meter_frame(
        &mut self,
        _now_s: f64,
        low_hz: f64,
        bin_hz: f64,
        bins_db: &[f32],
        around: Option<f64>,
    ) -> Option<Meter> {
        let hz = around.or(self.follow).or(self.target)?;
        let a = ((hz - self.cfg.follow_hz - low_hz) / bin_hz)
            .floor()
            .max(0.0) as usize;
        let b = (((hz + self.cfg.follow_hz - low_hz) / bin_hz)
            .ceil()
            .max(0.0) as usize)
            .min(bins_db.len());
        if a >= b {
            return None;
        }
        let p = (a..b).max_by(|&x, &y| bins_db[x].total_cmp(&bins_db[y]))?;
        let level = bins_db[p];
        if level <= UNCOVERED_DB {
            return None;
        }
        let peak_hz = low_hz + (p as f64 + 0.5) * bin_hz;
        if self.target.is_none() {
            self.follow = Some(peak_hz);
        }
        let base = self.baseline_at(peak_hz);
        let rise = if base.is_finite() { level - base } else { 0.0 };
        let (smoothed, max, prev) = match &self.meter {
            Some(m) => (
                m.smoothed_db + 0.3 * (level - m.smoothed_db),
                m.max_db.max(level),
                Some(m.smoothed_db),
            ),
            None => (level, level, None),
        };
        let trend = match prev {
            Some(p) if smoothed > p + 0.5 => Trend::Louder,
            Some(p) if smoothed < p - 0.5 => Trend::Quieter,
            _ => Trend::Steady,
        };
        let needle = (rise.max(0.0) / 40.0).clamp(0.0, 1.0);
        let m = Meter {
            hz: peak_hz,
            level_db: level,
            rise_db: rise,
            smoothed_db: smoothed,
            trend,
            max_db: max,
            needle,
            tone_hz: 300.0 + 1700.0 * needle,
        };
        self.meter = Some(m);
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet(seed: u64) -> Vec<f32> {
        let mut r = crate::fake::Rng::new(seed);
        (0..1000).map(|_| -90.0 + r.gauss() as f32).collect()
    }

    fn learnt() -> Hunter {
        let g = Grid::new(0.0, 1e6, 1e3).unwrap();
        let mut h = Hunter::new(g, HunterConfig::default());
        for s in 0..5 {
            let mut v = quiet(s);
            v[100] = -40.0; // a station that was always there
            h.update(s as f64, &v);
        }
        assert!(h.learnt());
        h
    }

    #[test]
    fn classes() {
        // A carrier.
        let mut h = learnt();
        let mut v = quiet(10);
        v[100] = -40.0;
        v[500] = -60.0;
        h.update(10.0, &v);
        assert_eq!(h.found().len(), 1);
        assert_eq!(h.found()[0].class, SourceClass::NarrowCarrier);
        assert_eq!(h.found()[0].peak_hz, 500.5e3);
        assert!(!h.found()[0].class.describe().is_empty());
        // A comb.
        let mut h = learnt();
        let mut v = quiet(11);
        for k in [200, 250, 300, 350] {
            v[k] = -60.0;
        }
        h.update(10.0, &v);
        assert_eq!(h.found().len(), 4);
        assert!(h
            .found()
            .iter()
            .all(|f| f.class == SourceClass::HarmonicComb));
        // Broadband.
        let mut h = learnt();
        let v: Vec<f32> = quiet(12).iter().map(|x| x + 12.0).collect();
        h.update(10.0, &v);
        assert_eq!(h.found().len(), 1);
        assert_eq!(h.found()[0].class, SourceClass::BroadbandNoise);
        // Wide and flat.
        let mut h = learnt();
        let mut v = quiet(13);
        for x in &mut v[600..640] {
            *x = -60.0;
        }
        h.update(10.0, &v);
        assert_eq!(h.found()[0].class, SourceClass::WidebandDigital);
        // Intermittent.
        let mut h = learnt();
        for s in 0..8u64 {
            let mut v = quiet(20 + s);
            if s % 4 == 0 {
                v[700] = -60.0;
            }
            h.update(10.0 + s as f64, &v);
        }
        assert_eq!(h.found()[0].class, SourceClass::Intermittent);
    }

    #[test]
    fn meter_follows_and_trends() {
        let mut h = learnt();
        let mut v = quiet(30);
        v[400] = -70.0;
        h.update(10.0, &v);
        let m0 = *h.meter().unwrap();
        assert_eq!(m0.hz, 400.5e3);
        assert!((m0.rise_db - 20.0).abs() < 4.0);
        // Walking closer: louder, and the peak moved a little.
        for (i, lvl) in [-65.0, -60.0, -55.0].iter().enumerate() {
            let mut f = quiet(40 + i as u64);
            f[402] = *lvl;
            let m = h.meter_frame(11.0 + i as f64, 0.0, 1e3, &f, None).unwrap();
            assert_eq!(m.trend, Trend::Louder);
            assert_eq!(m.hz, 402.5e3);
        }
        let m = *h.meter().unwrap();
        assert_eq!(m.max_db, -55.0);
        assert!(m.needle > 0.5 && m.tone_hz > 1000.0);
        let mut f = quiet(50);
        f[402] = -80.0;
        let m = h.meter_frame(15.0, 0.0, 1e3, &f, None).unwrap();
        assert_eq!(m.trend, Trend::Quieter);
        h.relearn();
        assert!(!h.learnt());
    }
}
