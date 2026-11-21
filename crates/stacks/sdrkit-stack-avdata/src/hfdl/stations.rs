//! The HFDL ground stations and bands of the published system tables, and the band hopper that follows the busiest band.

/// A ground station.
#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    /// Id.
    pub id: u8,
    /// Name.
    pub name: &'static str,
    /// Position.
    pub lat: f64,
    /// Position.
    pub lon: f64,
    /// Frequencies in kHz, bit 0 first.
    pub khz: &'static [u32],
}

/// The built-in table (published HFDL system tables. Data the host can replace).
pub const TABLE: [Station; 16] = [
    Station {
        id: 1,
        name: "San Francisco",
        lat: 37.6,
        lon: -122.4,
        khz: &[21934, 17919, 13276, 11327, 10081, 8927, 6559, 5508],
    },
    Station {
        id: 2,
        name: "Molokai",
        lat: 21.2,
        lon: -157.2,
        khz: &[21937, 17934, 13324, 13312, 10027, 8936, 8912, 5514],
    },
    Station {
        id: 3,
        name: "Reykjavik",
        lat: 64.1,
        lon: -21.9,
        khz: &[17985, 15025, 11184, 8977, 6712, 5720, 3900],
    },
    Station {
        id: 4,
        name: "Riverhead",
        lat: 40.9,
        lon: -72.6,
        khz: &[21931, 17919, 13276, 11387, 8912, 6661, 5652],
    },
    Station {
        id: 5,
        name: "Auckland",
        lat: -37.0,
        lon: 174.8,
        khz: &[17916, 13351, 10084, 8921, 6535, 5583],
    },
    Station {
        id: 6,
        name: "Hat Yai",
        lat: 6.9,
        lon: 100.4,
        khz: &[21949, 17928, 13270, 10066, 8825, 6535, 5655],
    },
    Station {
        id: 7,
        name: "Shannon",
        lat: 52.7,
        lon: -8.9,
        khz: &[11384, 10081, 8942, 6532, 5547, 3455],
    },
    Station {
        id: 8,
        name: "Johannesburg",
        lat: -26.1,
        lon: 28.2,
        khz: &[21949, 17922, 13321, 11321, 8834, 5529, 4681],
    },
    Station {
        id: 9,
        name: "Barrow",
        lat: 71.3,
        lon: -156.8,
        khz: &[21937, 17916, 11354, 10093, 8936, 6646, 5544, 4687],
    },
    Station {
        id: 10,
        name: "Muan",
        lat: 34.9,
        lon: 126.4,
        khz: &[21931, 17958, 13342, 11330, 8939, 6589, 5502],
    },
    Station {
        id: 11,
        name: "Albrook",
        lat: 9.0,
        lon: -79.6,
        khz: &[17901, 13264, 10063, 8894, 6589, 5589],
    },
    Station {
        id: 13,
        name: "Santa Cruz",
        lat: -17.6,
        lon: -63.1,
        khz: &[21997, 17916, 13315, 11318, 8957, 6628, 4660],
    },
    Station {
        id: 14,
        name: "Krasnoyarsk",
        lat: 56.2,
        lon: 92.5,
        khz: &[21990, 17912, 13321, 10087, 8886, 6596, 5559],
    },
    Station {
        id: 15,
        name: "Al Muharraq",
        lat: 26.3,
        lon: 50.6,
        khz: &[21982, 17967, 13354, 11348, 8885, 5544],
    },
    Station {
        id: 16,
        name: "Agana",
        lat: 13.5,
        lon: 144.8,
        khz: &[21928, 17919, 13312, 11306, 8927, 6652],
    },
    Station {
        id: 17,
        name: "Canarias",
        lat: 27.9,
        lon: -15.4,
        khz: &[21955, 17928, 13303, 11348, 8948, 6529, 5589],
    },
];

/// The station with this id.
pub fn by_id(id: u8) -> Option<&'static Station> {
    TABLE.iter().find(|s| s.id == id)
}

/// The station on a frequency: the first in the table that lists it.
pub fn on_khz(khz: u32) -> Option<&'static Station> {
    TABLE.iter().find(|s| s.khz.contains(&khz))
}

/// The name of a station id ("San Francisco"), or "station N".
pub fn name(id: u8) -> String {
    by_id(id).map_or(format!("station {id}"), |s| s.name.to_string())
}

/// The frequencies (kHz) of a mask for a station ("bitN" beyond the table).
pub fn mask_khz(id: u8, mask: u32) -> Vec<String> {
    let st = by_id(id);
    (0..20)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| {
            st.and_then(|s| s.khz.get(i))
                .map_or(format!("bit{i}"), |k| k.to_string())
        })
        .collect()
}

/// Every frequency of the table, kHz, sorted, without duplicates.
pub fn all_khz() -> Vec<u32> {
    let mut v: Vec<u32> = TABLE.iter().flat_map(|s| s.khz.iter().copied()).collect();
    v.sort();
    v.dedup();
    v
}

/// A band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// Name.
    pub name: &'static str,
    /// Centre, MHz.
    pub centre_mhz: f64,
    /// Range, kHz.
    pub lo_khz: u32,
    /// Range, kHz.
    pub hi_khz: u32,
    /// In the tour.
    pub tour: bool,
}

/// The twelve bands.
pub const BANDS: [Band; 12] = [
    Band {
        name: "5.5 MHz",
        centre_mhz: 5.58,
        lo_khz: 5450,
        hi_khz: 5725,
        tour: true,
    },
    Band {
        name: "6.6 MHz",
        centre_mhz: 6.62,
        lo_khz: 6525,
        hi_khz: 6715,
        tour: true,
    },
    Band {
        name: "8.9 MHz",
        centre_mhz: 8.90,
        lo_khz: 8820,
        hi_khz: 8980,
        tour: true,
    },
    Band {
        name: "10.06 MHz",
        centre_mhz: 10.06,
        lo_khz: 10020,
        hi_khz: 10100,
        tour: true,
    },
    Band {
        name: "11.3 MHz",
        centre_mhz: 11.29,
        lo_khz: 11180,
        hi_khz: 11390,
        tour: true,
    },
    Band {
        name: "13.3 MHz",
        centre_mhz: 13.31,
        lo_khz: 13260,
        hi_khz: 13360,
        tour: true,
    },
    Band {
        name: "17.94 MHz",
        centre_mhz: 17.945,
        lo_khz: 17900,
        hi_khz: 17990,
        tour: true,
    },
    Band {
        name: "21.96 MHz",
        centre_mhz: 21.963,
        lo_khz: 21920,
        hi_khz: 22000,
        tour: true,
    },
    Band {
        name: "3.455 MHz",
        centre_mhz: 3.455,
        lo_khz: 3450,
        hi_khz: 3460,
        tour: false,
    },
    Band {
        name: "3.9 MHz",
        centre_mhz: 3.90,
        lo_khz: 3895,
        hi_khz: 3905,
        tour: false,
    },
    Band {
        name: "4.67 MHz",
        centre_mhz: 4.675,
        lo_khz: 4655,
        hi_khz: 4695,
        tour: false,
    },
    Band {
        name: "15.03 MHz",
        centre_mhz: 15.025,
        lo_khz: 15020,
        hi_khz: 15030,
        tour: false,
    },
];

/// The band of a frequency (Hz): the one whose range holds it, or whose centre is within 0.1 MHz.
pub fn band_of(hz: f64) -> Option<usize> {
    let khz = hz / 1e3;
    BANDS
        .iter()
        .position(|b| khz >= b.lo_khz as f64 && khz <= b.hi_khz as f64)
        .or_else(|| {
            BANDS
                .iter()
                .position(|b| (hz / 1e6 - b.centre_mhz).abs() <= 0.1)
        })
}

/// What the hopper decides.
#[derive(Clone, Debug, PartialEq)]
pub enum Hop {
    /// Stay.
    Stay,
    /// Tune to this band's centre.
    To(usize),
}

#[derive(Clone, Debug, Default)]
struct BandMem {
    rate: f64,
    last_visit: f64,
    visited: bool,
    gave_traffic: bool,
    announced: Vec<(u32, f64)>,
}

/// The band hopper ("follow the best band"): a pure rule given the time, the band the radio is on, the good PDUs so far and the
/// frequencies the squitters announce.
#[derive(Clone, Debug)]
pub struct Hopper {
    mem: Vec<BandMem>,
    cur: Option<usize>,
    arrived: f64,
    last_t: f64,
    last_count: u64,
    rate: f64,
    zero_phase: bool,
    pending: Option<(usize, f64)>,
    /// The status line.
    pub status: String,
}

impl Default for Hopper {
    fn default() -> Self {
        Hopper {
            mem: vec![BandMem::default(); BANDS.len()],
            cur: None,
            arrived: 0.0,
            last_t: 0.0,
            last_count: 0,
            rate: 0.0,
            zero_phase: true,
            pending: None,
            status: String::new(),
        }
    }
}

fn range_text(b: &Band) -> String {
    format!(
        "{} ({:.2} - {:.2})",
        b.name,
        b.lo_khz as f64 / 1e3,
        b.hi_khz as f64 / 1e3
    )
}

impl Hopper {
    /// One step, about once a second. `announced` are frequencies (kHz) the squitters heard just now announce.
    pub fn step(&mut self, t: f64, radio_hz: f64, pdus: u64, announced: &[u32]) -> Hop {
        for &k in announced {
            if let Some(b) = band_of(k as f64 * 1e3) {
                let a = &mut self.mem[b].announced;
                if !a.iter().any(|x| x.0 == k) {
                    a.push((k, t));
                } else if let Some(x) = a.iter_mut().find(|x| x.0 == k) {
                    x.1 = t;
                }
            }
        }
        for m in self.mem.iter_mut() {
            m.announced.retain(|x| t - x.1 <= 600.0);
        }
        let Some(band) = band_of(radio_hz) else {
            self.cur = None;
            self.pending = None;
            self.status = "not on an HFDL band".into();
            return Hop::Stay;
        };
        if let Some((want, at)) = self.pending {
            if band == want || t - at > 6.0 {
                self.pending = None;
            } else {
                return Hop::Stay;
            }
        }
        if self.cur != Some(band) {
            if let Some(c) = self.cur {
                self.mem[c].last_visit = t;
                self.mem[c].rate = self.rate;
            }
            self.cur = Some(band);
            self.arrived = t;
            self.rate = 0.0;
            self.zero_phase = true;
            self.last_count = pdus;
            self.last_t = t;
            self.mem[band].visited = true;
        }
        // The PDU rate, per minute, smoothed with a 120 s time constant. Right after arriving it follows quickly.
        let dt = (t - self.last_t).max(0.0);
        let new = pdus.saturating_sub(self.last_count.min(pdus));
        let inst = if dt > 0.0 {
            new as f64 / dt * 60.0
        } else {
            0.0
        };
        let mut a = 1.0 - (-dt / 120.0).exp();
        let since = t - self.arrived;
        if self.zero_phase {
            a = a.max(0.5);
            if since > 360.0 || self.rate > 0.0 {
                self.zero_phase = false;
            }
        }
        if dt > 0.0 {
            self.rate += a * (inst - self.rate);
        }
        self.last_t = t;
        self.last_count = pdus;
        if new > 0 {
            self.mem[band].gave_traffic = true;
        }
        let here = &BANDS[band];
        let look = if self.rate > 0.0 && self.rate < 4.0 {
            120.0
        } else {
            50.0
        };
        if self.rate >= 4.0 {
            self.status = format!("{}: {:.0} PDU/min, staying", range_text(here), self.rate);
            return Hop::Stay;
        }
        if since < look {
            self.status = if self.rate == 0.0 {
                format!(
                    "{}: silent, deciding in {:.0} s",
                    range_text(here),
                    look - since
                )
            } else {
                format!("{}: looking", range_text(here))
            };
            return Hop::Stay;
        }
        // The best other band.
        let score = |i: usize, m: &BandMem| {
            let fade = if m.visited {
                m.rate * (-(t - m.last_visit) / 900.0).exp()
                    + 3.0 * (1.0 - (-(t - m.last_visit) / 900.0).exp())
            } else {
                3.0
            };
            fade + 0.75 * m.announced.len().min(4) as f64 + if i == band { -1e9 } else { 0.0 }
        };
        let mut best: Option<(f64, f64, usize)> = None;
        for (i, m) in self.mem.iter().enumerate() {
            if i == band {
                continue;
            }
            let b = &BANDS[i];
            if !b.tour && m.announced.is_empty() && !m.gave_traffic {
                continue;
            }
            let s = score(i, m);
            let age = if m.visited {
                m.last_visit
            } else {
                f64::NEG_INFINITY
            };
            if best.is_none_or(|x| s > x.0 + 1e-9 || ((s - x.0).abs() <= 1e-9 && age < x.1)) {
                best = Some((s, age, i));
            }
        }
        let Some((s, _, to)) = best else {
            self.status = format!(
                "{}: {:.1} PDU/min, no better band known",
                range_text(here),
                self.rate
            );
            return Hop::Stay;
        };
        if self.rate == 0.0 || s >= self.rate + 1.0 {
            self.status = format!(
                "{}: {}, moving to {}",
                range_text(here),
                if self.rate == 0.0 {
                    "silent".to_string()
                } else {
                    "weak".to_string()
                },
                range_text(&BANDS[to])
            );
            self.pending = Some((to, t));
            return Hop::To(to);
        }
        self.arrived = t;
        self.status = format!(
            "{}: {:.1} PDU/min, no better band known",
            range_text(here),
            self.rate
        );
        Hop::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands() {
        assert_eq!(BANDS[band_of(8.927e6).unwrap()].name, "8.9 MHz");
        assert_eq!(BANDS[band_of(8.90e6).unwrap()].name, "8.9 MHz");
        assert_eq!(BANDS[band_of(21.963e6).unwrap()].name, "21.96 MHz");
        assert!(band_of(14.2e6).is_none());
        for k in all_khz() {
            assert!(band_of(k as f64 * 1e3).is_some(), "{k}");
        }
        assert_eq!(on_khz(8927).unwrap().name, "San Francisco");
        assert_eq!(mask_khz(1, 0x21), vec!["21934", "8927"]);
    }

    fn sim(h: &mut Hopper, good: &dyn Fn(usize, f64) -> f64, secs: f64) -> Vec<usize> {
        let mut radio = BANDS[2].centre_mhz * 1e6;
        let mut count = 0u64;
        let mut acc = 0.0;
        let mut visits = vec![];
        let mut t = 0.0;
        while t < secs {
            let b = band_of(radio).unwrap();
            if visits.last() != Some(&b) {
                visits.push(b);
            }
            acc += good(b, t) / 60.0;
            while acc >= 1.0 {
                acc -= 1.0;
                count += 1;
            }
            if let Hop::To(n) = h.step(t, radio, count, &[]) {
                radio = BANDS[n].centre_mhz * 1e6;
            }
            t += 1.0;
        }
        visits
    }

    #[test]
    fn h18_finds_the_working_band_and_stays() {
        let mut h = Hopper::default();
        let v = sim(&mut h, &|b, _| if b == 5 { 10.0 } else { 0.0 }, 1500.0);
        assert_eq!(*v.last().unwrap(), 5);
        assert!(h.status.contains("staying"));
    }

    #[test]
    fn h18_tours_the_common_bands_when_silent() {
        let mut h = Hopper::default();
        let v = sim(&mut h, &|_, _| 0.0, 8.0 * 60.0);
        let first8: Vec<usize> = v.iter().take(8).copied().collect();
        let mut s = first8.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), 8, "{v:?}");
        assert!(first8.iter().all(|&b| BANDS[b].tour));
    }

    #[test]
    fn h18_off_band_and_counter_back() {
        let mut h = Hopper::default();
        assert_eq!(h.step(0.0, 14.2e6, 0, &[]), Hop::Stay);
        assert_eq!(h.status, "not on an HFDL band");
        h.step(1.0, 8.9e6, 100, &[]);
        assert_eq!(h.step(2.0, 8.9e6, 50, &[]), Hop::Stay);
    }

    #[test]
    fn h18_follows_a_change_and_uses_announcements() {
        let mut h = Hopper::default();
        let v = sim(
            &mut h,
            &|b, t| {
                if (t < 1200.0 && b == 2) || (t >= 1200.0 && b == 4) {
                    10.0
                } else {
                    0.0
                }
            },
            4000.0,
        );
        assert_eq!(*v.last().unwrap(), 4, "{v:?}");
        let mut h = Hopper::default();
        let _ = h.step(0.0, 8.9e6, 0, &[13276, 13321, 13312]);
        let mut radio = 8.9e6;
        let mut t = 1.0;
        while t < 60.0 {
            if let Hop::To(n) = h.step(t, radio, 0, &[]) {
                radio = BANDS[n].centre_mhz * 1e6;
                break;
            }
            t += 1.0;
        }
        assert_eq!(BANDS[band_of(radio).unwrap()].name, "13.3 MHz");
    }
}
