//! The navigation data store: it turns checked units into ephemerides, the GPS almanac, the ionosphere and UTC parameters, the
//! Galileo GST-GPS offset, BDT − GPS and GLONASS time. It keeps what it holds for each satellite.

use crate::bds::{self, BdsCollector, BdsSubframe};
use crate::geo::{resolve_week, Klobuchar};
use crate::glo::{self, GloCollector, GloTime};
use crate::inav::{self, Ggto, InavCollector, InavWord};
use crate::lnav::{self, LnavCollector, LnavContent, Subframe, UtcParams};
use crate::msg::NavUnit;
use crate::orbit::{Almanac, GloEphemeris, Kepler};
use crate::system::{SatId, System};
use std::collections::BTreeMap;

/// An ephemeris of either kind (the `gnss.ephemeris` stream).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Ephemeris {
    Kepler(Kepler),
    Glonass(GloEphemeris),
}

/// Per-satellite collectors and arrival times.
#[derive(Clone, Debug, Default)]
pub struct SatNav {
    pub lnav: LnavCollector,
    pub inav: InavCollector,
    pub bds: BdsCollector,
    pub glo: GloCollector,
    /// Satellite time of the last unit, and the signal time it arrived.
    pub last_tow: Option<f64>,
    pub last_unit_s: f64,
    /// LNAV: when subframes 1 to 3 last arrived (signal time).
    pub sf_seen: [Option<f64>; 3],
    pub last_sf: Option<(u8, f64)>,
    /// A GLONASS string whose time is not known yet (string 1 waits for the next string to confirm its time).
    pub glo_pending: Option<[u8; 85]>,
}

/// What a unit changed, for the receiver's log and outputs.
#[derive(Clone, Debug, PartialEq)]
pub enum NavNews {
    Ephemeris(SatId, Ephemeris),
    Log(String),
    /// A GLONASS channel learnt its slot.
    GloSlot(i8, u16),
}

/// The store.
#[derive(Clone, Debug, Default)]
pub struct NavStore {
    pub week_ref: i64,
    pub eph: BTreeMap<SatId, Kepler>,
    pub glo_eph: BTreeMap<SatId, GloEphemeris>,
    pub sats: BTreeMap<SatId, SatNav>,
    pub almanac: BTreeMap<u16, Almanac>,
    pub alm_time: Option<(f64, u32)>,
    pub iono: Option<Klobuchar>,
    pub utc: Option<UtcParams>,
    pub ggto: Option<Ggto>,
    pub bdt_gps: Option<(f64, f64)>,
    pub glo_time: Option<GloTime>,
}

impl NavStore {
    pub fn new(week_ref: i64) -> NavStore {
        NavStore {
            week_ref,
            ..Default::default()
        }
    }

    /// Forgets everything (a reset or retune).
    pub fn clear(&mut self) {
        *self = NavStore::new(self.week_ref);
    }

    /// The full GPS week of the newest GPS-like ephemeris, else from the known week.
    pub fn gps_week(&self) -> Option<i64> {
        let pick = |s: System| {
            self.eph
                .values()
                .filter(|e| e.system == Some(s))
                .map(|e| e.week)
                .max()
        };
        pick(System::Gps)
            .or_else(|| pick(System::Qzss))
            .or_else(|| pick(System::Beidou).map(|w| w + crate::system::BDT_WEEK0_IN_GPS))
            .or_else(|| pick(System::Galileo))
    }

    /// Takes a checked unit from satellite `sat` (`glo_k`: the GLONASS frequency channel) received at signal time `now`.
    pub fn add(
        &mut self,
        sat: SatId,
        glo_k: i8,
        unit: &NavUnit,
        time: Option<f64>,
        now: f64,
    ) -> Vec<NavNews> {
        let mut news = Vec::new();
        let week_ref = self.week_ref;
        let entry = self.sats.entry(sat).or_default();
        entry.last_unit_s = now;
        if time.is_some() {
            entry.last_tow = time;
        }
        match unit {
            NavUnit::Lnav {
                data,
                id,
                tow_count,
            } => {
                let sf = Subframe {
                    data: data.clone(),
                    id: *id,
                    tow_count: *tow_count,
                    inverted: false,
                };
                entry.last_tow = Some(sf.start_tow());
                entry.last_sf = Some((*id, now));
                if (1..=3).contains(id) {
                    entry.sf_seen[*id as usize - 1] = Some(now);
                }
                let c = lnav::decode(&sf, sat.system);
                match &c {
                    LnavContent::Almanac(a) if sat.system == System::Gps => {
                        let mut a = a.clone();
                        if let Some((_, wna)) = self.alm_time {
                            a.week = resolve_week(wna as i64, 256, week_ref);
                        }
                        self.almanac.insert(a.prn, a);
                    }
                    LnavContent::AlmanacTime { toa, wna } if sat.system == System::Gps => {
                        self.alm_time = Some((*toa, *wna));
                    }
                    LnavContent::IonoUtc(k, u) if sat.system == System::Gps => {
                        if self.iono.is_none() {
                            news.push(NavNews::Log(format!(
                                "ionosphere and UTC parameters from {}: leap seconds {}",
                                sat.name(),
                                u.dtls
                            )));
                        }
                        self.iono = Some(*k);
                        self.utc = Some(*u);
                    }
                    _ => {}
                }
                let entry = self.sats.get_mut(&sat).expect("the entry was made above");
                if let Some(e) = entry.lnav.add(c, sat.system, sat.prn, |w| {
                    resolve_week(w as i64, 1024, week_ref)
                }) {
                    self.store_kepler(sat, e, &mut news);
                }
            }
            NavUnit::Inav { word, .. } => {
                let w = inav::decode_word(word);
                if let InavWord::W10 { ggto: Some(g) } = &w {
                    if self.ggto.is_none() {
                        news.push(NavNews::Log(format!(
                            "Galileo GST-GPS offset from {}: {:.1} ns",
                            sat.name(),
                            g.a0g * 1e9
                        )));
                    }
                    self.ggto = Some(*g);
                }
                let full = |wn: u32| {
                    let mut w = wn as i64 + 1024;
                    while w + 2048 < week_ref {
                        w += 4096;
                    }
                    w
                };
                if let Some(e) = entry.inav.add(w, sat.prn, full) {
                    self.store_kepler(sat, e, &mut news);
                }
            }
            NavUnit::Bds { info, id, sow, d2 } => {
                let sf = BdsSubframe {
                    info: info.clone(),
                    id: *id,
                    sow: *sow,
                    corrected: 0,
                };
                if let Some((a0, a1)) = bds::bdt_gps(&sf) {
                    if self.bdt_gps.is_none() {
                        news.push(NavNews::Log(format!(
                            "BDT - GPS from {}: {:.1} ns",
                            sat.name(),
                            a0 * 1e9
                        )));
                    }
                    self.bdt_gps = Some((a0, a1));
                }
                let bref = week_ref - crate::system::BDT_WEEK0_IN_GPS;
                if let Some(e) = entry
                    .bds
                    .add(&sf, *d2, sat.prn, |w| resolve_week(w as i64, 8192, bref))
                {
                    self.store_kepler(sat, e, &mut news);
                }
            }
            NavUnit::Glo { bits, number } => {
                let mut s = [0u8; 85];
                s.copy_from_slice(&bits[..85]);
                if *number == 5 {
                    let t = glo::string5(&s);
                    if self.glo_time.is_none() {
                        news.push(NavNews::Log(format!(
                            "GLONASS time from {}: tauGPS {:.1} ns, tauC {:.1} ns",
                            sat.name(),
                            t.tau_gps * 1e9,
                            t.tau_c * 1e9
                        )));
                    }
                    self.glo_time = Some(t);
                }
                let tod = time.or_else(|| entry.last_tow.map(|t| t + 2.0));
                let Some(tod) = tod else {
                    entry.glo_pending = Some(s);
                    return news;
                };
                entry.last_tow = Some(tod);
                let mut strings = Vec::with_capacity(2);
                if let Some(p) = entry.glo_pending.take() {
                    strings.push((p, tod - 2.0));
                }
                strings.push((s, tod));
                for (st, t) in strings {
                    let entry = self.sats.get_mut(&sat).expect("the entry was made above");
                    if let Some(e) = entry.glo.add(&st, t, glo_k) {
                        let id = if e.slot > 0 {
                            SatId::new(System::Glonass, e.slot)
                        } else {
                            sat
                        };
                        let changed = self
                            .glo_eph
                            .get(&id)
                            .is_none_or(|o| o.tb != e.tb || o.slot != e.slot || o.bn != e.bn);
                        if changed {
                            let h = (e.tb / 3600.0).floor();
                            let m = ((e.tb - h * 3600.0) / 60.0).round();
                            news.push(NavNews::Log(format!(
                                "{}: ephemeris tb {:02}:{:02}, day {}, {}",
                                id.name(),
                                h,
                                m,
                                e.nt,
                                if e.healthy() { "healthy" } else { "unhealthy" }
                            )));
                            news.push(NavNews::Ephemeris(id, Ephemeris::Glonass(e.clone())));
                            if e.slot > 0 && sat.glo_k().is_some() {
                                news.push(NavNews::GloSlot(glo_k, e.slot));
                            }
                            self.glo_eph.insert(id, e);
                        }
                    }
                }
            }
            NavUnit::Sbas { .. } | NavUnit::Navic(_) => {}
        }
        news
    }

    fn store_kepler(&mut self, sat: SatId, e: Kepler, news: &mut Vec<NavNews>) {
        let changed = self
            .eph
            .get(&sat)
            .is_none_or(|o| o.iodc != e.iodc || o.toe != e.toe || o.iode != e.iode);
        if changed {
            news.push(NavNews::Log(format!(
                "{}: ephemeris IODE {}, week {}, {}",
                sat.name(),
                e.iode,
                e.week,
                if e.health == 0 {
                    "healthy"
                } else {
                    "unhealthy"
                }
            )));
            news.push(NavNews::Ephemeris(sat, Ephemeris::Kepler(e.clone())));
            self.eph.insert(sat, e);
        }
    }

    /// Parts held toward the next ephemeris of `sat`, of how many.
    pub fn parts(&self, sat: SatId) -> (usize, usize) {
        let Some(n) = self.sats.get(&sat) else {
            return (0, 3);
        };
        match sat.system {
            System::Gps | System::Qzss => (n.lnav.parts(), 3),
            System::Galileo => (n.inav.parts(), 4),
            System::Beidou => (n.bds.parts(crate::system::bds_is_geo(sat.prn)), 3),
            System::Glonass => (n.glo.parts(), 4),
            System::Sbas => (0, 0),
        }
    }

    /// Seconds until the last missing of subframes 1 to 3 arrives (LNAV), from the last subframe received.
    pub fn seconds_left(&self, sat: SatId, now: f64) -> Option<f64> {
        let n = self.sats.get(&sat)?;
        let (last_id, at) = n.last_sf?;
        let held = n.lnav.held();
        let mut worst: Option<f64> = None;
        for (k, h) in held.iter().enumerate() {
            if *h {
                continue;
            }
            let want = k as i64 + 1;
            let steps = (want - last_id as i64).rem_euclid(5);
            let steps = if steps == 0 { 5 } else { steps };
            let t = at + steps as f64 * 6.0 - now;
            worst = Some(worst.map_or(t, |w: f64| w.max(t)));
        }
        worst.map(|w| w.max(0.0))
    }

    /// The healthy flag of a satellite: from its ephemeris, else its almanac (GPS).
    pub fn healthy(&self, sat: SatId) -> Option<bool> {
        if let Some(e) = self.eph.get(&sat) {
            return Some(e.health == 0);
        }
        if let Some(e) = self.glo_eph.get(&sat) {
            return Some(e.healthy());
        }
        if sat.system == System::Gps {
            return self.almanac.get(&sat.prn).map(|a| a.health == 0);
        }
        None
    }
}
