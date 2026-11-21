//! The STL display (`stl.display`): the reception fields and RDS of the FM components at every tick, and at every report of the
//! link finder the kind of the link, its held changes (a row and an `stl.link` event each), the STL fields and the headline.
//!
//! | Kind | When |
//! |---|---|
//! | composite | the FM demodulator sees a carrier and the pilot is locked |
//! | mono | a carrier without a pilot |
//! | digital or unknown | no carrier; a region at least 6 dB over the floor, flatter than -3 dB and at least 60 kHz wide |
//! | unknown | no carrier; another region at least 6 dB over the floor |
//! | nothing | anything else |
//!
//! The first reading holds at once; a later change holds when the same new kind comes in two reports in a row.

use super::link::{LinkReport, Region};
use super::sca::ScaReport;
use super::sound::SoundReport;
use super::typed::{LinkEvent, LinkKind, LinkRow, StlFields, LINKS, LINK_CHANGED};
use crate::fmdemod::DemodReport;
use crate::graph::{
    clear_results, lock_state, publish_reception, ChannelReport, FmCommand, MpxReport, RdsReport,
    Tick,
};
use crate::results::Shown;
use crate::stereo::StereoReport;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, LockState, Record, Results, Values,
};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Kind, Port, RateFrom};
use std::fmt::Write;
use std::sync::Arc;

/// The lowest level over the floor of a signal that is not FM, dB.
pub const MIN_OVER_FLOOR_DB: f64 = 6.0;
/// A flatness above this is a flat (digital-like) spectrum, dB.
pub const FLAT_DB: f64 = -3.0;
/// The narrowest digital link, Hz.
pub const MIN_DIGITAL_HZ: f64 = 60e3;

/// The kind of a link from one report: the FM demodulator's carrier, the pilot lock and the occupied region.
pub fn classify(carrier: bool, pilot_locked: bool, region: Option<&Region>) -> LinkKind {
    match (carrier, pilot_locked, region) {
        (true, true, _) => LinkKind::Composite,
        (true, false, _) => LinkKind::Mono,
        (false, _, Some(r)) if r.over_floor_db >= MIN_OVER_FLOOR_DB => {
            if r.flatness_db > FLAT_DB && r.width_hz >= MIN_DIGITAL_HZ {
                LinkKind::DigitalUnknown
            } else {
                LinkKind::Unknown
            }
        }
        _ => LinkKind::Nothing,
    }
}

/// The held kind: the first reading at once, a change after the same new kind in two reports in a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KindHold {
    held: Option<LinkKind>,
    pending: Option<LinkKind>,
}

impl KindHold {
    /// The kind that holds.
    pub fn held(&self) -> LinkKind {
        self.held.unwrap_or_default()
    }
    /// One report's kind. `Some` with the new kind when a change holds (the first reading counts as one unless it is
    /// nothing).
    pub fn push(&mut self, k: LinkKind) -> Option<LinkKind> {
        let Some(held) = self.held else {
            self.held = Some(k);
            return (k != LinkKind::Nothing).then_some(k);
        };
        if k == held {
            self.pending = None;
            return None;
        }
        if self.pending == Some(k) {
            (self.held, self.pending) = (Some(k), None);
            return Some(k);
        }
        self.pending = Some(k);
        None
    }
}

/// The details of a link: pilot, peak deviation and offset of an FM link; level and width of a signal that is not FM.
fn details(out: &mut String, kind: LinkKind, pilot_hz: Option<f64>, l: &LinkReport, offset_hz: f64) {
    out.clear();
    let region = l.findings.region;
    match kind {
        LinkKind::Composite | LinkKind::Mono => {
            match pilot_hz {
                Some(p) => {
                    let _ = write!(out, "pilot {:.1} kHz", p / 1e3);
                }
                None => out.push_str("no pilot"),
            }
            if let Some(p) = l.peak_hz {
                let _ = write!(out, ", peak deviation {:.1} kHz", p / 1e3);
            }
            let _ = write!(out, ", offset {:+.1} kHz", offset_hz / 1e3);
        }
        LinkKind::DigitalUnknown => {
            let r = region.unwrap_or_default();
            let _ = write!(
                out,
                "not FM, {:.0} dB over the floor, about {:.0} kHz wide; not decoded",
                r.over_floor_db,
                r.width_hz / 1e3
            );
        }
        LinkKind::Unknown => {
            let r = region.unwrap_or_default();
            let _ = write!(out, "not FM, {:.0} dB over the floor", r.over_floor_db);
        }
        LinkKind::Nothing => out.push_str("no signal"),
    }
}

/// The STL display. It is the status component of the STL graph. Command: `clear`.
pub struct StlDisplay {
    store: Published,
    shown: Shown,
    hold: KindHold,
    link: Option<LinkReport>,
    sca: ScaReport,
    sound: SoundReport,
    changes: u32,
    summary: String,
    text: String,
    key: String,
}

impl Default for StlDisplay {
    fn default() -> Self {
        let mut d = StlDisplay {
            store: Published::from_decl(Arc::new(super::typed::stack_shape().results_decl())),
            shown: Shown::default(),
            hold: KindHold::default(),
            link: None,
            sca: ScaReport::default(),
            sound: SoundReport::default(),
            changes: 0,
            summary: String::new(),
            text: String::new(),
            key: String::new(),
        };
        d.restart();
        d
    }
}

impl StlDisplay {
    /// Forgets the link and clears the results.
    fn restart(&mut self) {
        self.hold = KindHold::default();
        (self.link, self.changes) = (None, 0);
        clear_results(&mut self.store, &mut self.shown);
    }

    /// The pilot level as deviation, while the pilot is locked: the pilot share of 75 kHz.
    fn pilot_hz(st: &StereoReport) -> Option<f64> {
        st.locked.then(|| f64::from(st.pilot_pct) / 100.0 * 75e3)
    }

    /// Takes a report of the link finder: the kind, a held change, the STL fields.
    fn link_report(&mut self, l: LinkReport, t: &Tick, io: &mut Io) {
        let region = l.findings.region;
        let kind = classify(l.carrier, t.st.locked, region.as_ref());
        self.link = Some(l);
        let pilot = Self::pilot_hz(t.st);
        if let Some(k) = self.hold.push(kind) {
            details(&mut self.text, k, pilot, &l, t.offset_hz());
            let over = region.map(|r| r.over_floor_db);
            self.changes += 1;
            self.key.clear();
            let _ = write!(self.key, "{:04}", self.changes);
            let row = LinkRow {
                time_s: Seconds(t.ch.time_s),
                link: k,
                text: &self.text,
                over_floor_db: over,
            };
            LINKS.put(&mut self.store, &self.key, &row);
            io.messages_out(0).push(LINK_CHANGED.event(
                t.ch.time_s,
                t.ch.tuned_hz,
                &LinkEvent {
                    link: k,
                    text: &self.text,
                    over_floor_db: over,
                },
            ));
        }
        let fm = l.carrier;
        let deviation = self.sound.deviation_hz;
        self.store.put(&StlFields {
            link: self.hold.held(),
            peak_deviation_hz: l.peak_hz.filter(|_| fm).map(Hertz),
            modulation_pct: l
                .peak_hz
                .filter(|_| fm && deviation > 0.0)
                .map(|p| p / deviation * 100.0),
            pilot_hz: pilot.map(Hertz),
            width_auto: l.auto,
            floor_measured: l.findings.floor_ok,
            occupied_width_hz: region.map(|r| Hertz(r.width_hz)),
            over_floor_db: region.map(|r| r.over_floor_db),
            flatness_db: region.map(|r| r.flatness_db),
            sca67_snr_db: self.sca.snr_db[0].filter(|_| fm),
            sca92_snr_db: self.sca.snr_db[1].filter(|_| fm),
            sound_peak_dbfs: self.sound.peak_dbfs,
            sound_rms_dbfs: self.sound.rms_dbfs,
        });
    }

    /// The headline and the lock state of the held kind.
    fn headline(&mut self, t: &Tick) -> LockState {
        let s = &mut self.summary;
        s.clear();
        let Some(l) = self.link else {
            s.push_str("Searching");
            return LockState::Searching;
        };
        let region = l.findings.region.unwrap_or_default();
        match self.hold.held() {
            LinkKind::Composite => {
                s.push_str("composite stereo");
                if let Some(p) = l.peak_hz {
                    let _ = write!(s, ", peak {:.1} kHz", p / 1e3);
                }
                if let Some(p) = Self::pilot_hz(t.st) {
                    let _ = write!(s, ", pilot {:.1} kHz", p / 1e3);
                }
                let v = &t.rds.view;
                if !v.station.trim().is_empty() {
                    let _ = write!(s, ", {}", v.station.trim());
                } else if v.sync {
                    s.push_str(", RDS synced");
                } else {
                    s.push_str(", no RDS");
                }
                lock_state(t.dm.carrier, t.mpx.snr_db)
            }
            LinkKind::Mono => {
                s.push_str("mono (no pilot)");
                if let Some(p) = l.peak_hz {
                    let _ = write!(s, ", peak {:.1} kHz", p / 1e3);
                }
                let _ = write!(s, ", audio SNR {:.0} dB", t.mpx.snr_db);
                lock_state(t.dm.carrier, t.mpx.snr_db)
            }
            LinkKind::DigitalUnknown => {
                let _ = write!(
                    s,
                    "digital or unknown link (not FM), {:.0} dB over the floor, about {:.0} kHz wide: not decoded",
                    region.over_floor_db,
                    region.width_hz / 1e3
                );
                LockState::Tracking
            }
            LinkKind::Unknown => {
                let _ = write!(
                    s,
                    "unknown signal (not FM), {:.0} dB over the floor: not decoded",
                    region.over_floor_db
                );
                LockState::Tracking
            }
            LinkKind::Nothing => {
                s.push_str("Searching");
                if !l.findings.floor_ok {
                    s.push_str(
                        "; digital check off: the sample rate does not reach past the channel to the floor",
                    );
                }
                LockState::Searching
            }
        }
    }
}

impl Component for StlDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "stl.display",
            vec![
                Port::typed::<ChannelReport>("channel"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<MpxReport>("spectrum"),
                Port::typed::<StereoReport>("stereo"),
                Port::typed::<RdsReport>("rds"),
                Port::typed::<LinkReport>("link"),
                Port::typed::<ScaReport>("sca"),
                Port::typed::<SoundReport>("sound"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(s) = io.items::<ScaReport>(6).last() {
            self.sca = s.value;
        }
        if let Some(s) = io.items::<SoundReport>(7).last() {
            self.sound = s.value;
        }
        if !io.tick() {
            return;
        }
        let (Some(ch), Some(dm), Some(st)) = (
            io.items::<ChannelReport>(0).last().map(|t| t.value),
            io.items::<DemodReport>(1).last().map(|t| t.value),
            io.items::<StereoReport>(3).last().map(|t| t.value),
        ) else {
            return;
        };
        let (Some(mpx), Some(rds)) = (
            io.items::<MpxReport>(2).last(),
            io.items::<RdsReport>(4).last(),
        ) else {
            return;
        };
        let link = io.items::<LinkReport>(5).last().map(|t| t.value);
        let rds = Arc::clone(&rds.value.0);
        let t = Tick {
            ch: &ch,
            dm: &dm,
            mpx: &mpx.value,
            st: &st,
            squelched: !dm.carrier,
            rds: &rds,
        };
        if let Some(l) = link {
            self.link_report(l, &t, io);
        }
        let lock = self.headline(&t);
        publish_reception(&mut self.store, &mut self.shown, &t, io);
        let r = self.store.results_mut();
        crate::results::write_status(r, lock, t.mpx.snr_db, ch.level_dbfs, t.offset_hz(), &self.summary);
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.store.results())
    }
    fn commands(&self) -> &Commands {
        FmCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match FmCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            FmCommand::Clear => self.restart(),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(width_hz: f64, over_floor_db: f64, flatness_db: f64) -> Region {
        Region {
            width_hz,
            centroid_hz: 0.0,
            over_floor_db,
            flatness_db,
        }
    }

    #[test]
    fn the_worked_kinds() {
        let digital = region(238e3, 20.0, -1.0);
        assert_eq!(classify(true, true, None), LinkKind::Composite);
        assert_eq!(classify(true, false, Some(&digital)), LinkKind::Mono);
        assert_eq!(classify(false, false, Some(&digital)), LinkKind::DigitalUnknown);
        assert_eq!(
            classify(false, false, Some(&region(30e3, 10.0, -1.0))),
            LinkKind::Unknown
        );
        assert_eq!(
            classify(false, false, Some(&region(238e3, 10.0, -8.0))),
            LinkKind::Unknown
        );
        assert_eq!(
            classify(false, false, Some(&region(238e3, 4.0, -1.0))),
            LinkKind::Nothing
        );
        assert_eq!(classify(false, true, None), LinkKind::Nothing);
    }

    #[test]
    fn a_change_holds_after_two_reports() {
        let mut h = KindHold::default();
        assert_eq!(h.push(LinkKind::Composite), Some(LinkKind::Composite));
        // One report of mono, then composite again: no change.
        assert_eq!(h.push(LinkKind::Mono), None);
        assert_eq!(h.push(LinkKind::Composite), None);
        assert_eq!(h.held(), LinkKind::Composite);
        assert_eq!(h.push(LinkKind::Mono), None);
        assert_eq!(h.push(LinkKind::Mono), Some(LinkKind::Mono));
        // Noise from the start is no change.
        let mut h = KindHold::default();
        assert_eq!(h.push(LinkKind::Nothing), None);
        assert_eq!(h.push(LinkKind::Nothing), None);
        assert_eq!(h.held(), LinkKind::Nothing);
    }
}
