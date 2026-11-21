//! The channel list: the tracks of the hits as channels that a user acts on, confirmed after a number of sweeps, with flags
//! for images, spurs and the DC spike, a guess of the channel type, and an optional channel raster.

use crate::hits::Track;
use crate::plan::{HopPlan, Range};

sdrkit_core::choice_enum! {
    /// What a channel probably is.
    pub enum ChannelType {
        /// An unmodulated carrier.
        Carrier = ("carrier", "Carrier"),
        /// AM voice.
        Am = ("am", "AM"),
        /// Narrow FM, present most of the time.
        Nfm = ("nfm", "NFM"),
        /// Push-to-talk voice: narrow FM that comes and goes.
        KeyedVoice = ("keyed_voice", "Keyed voice"),
        /// Narrow data: under 4 kHz, or narrow and bursty.
        NarrowDigital = ("narrow_digital", "Narrow digital"),
        /// Digital voice or data, 4 to 100 kHz, flat-topped.
        Digital = ("digital", "Digital"),
        /// FM broadcast.
        WfmBroadcast = ("wfm_broadcast", "WFM broadcast"),
        /// Wide digital: DAB, DVB, LTE, Wi-Fi.
        WideDigital = ("wide_digital", "Wide digital"),
    }
}

impl ChannelType {
    /// The demodulator to listen with (`"am"`, `"nfm"`, `"wfm"`, `"cw"`). Empty for data.
    pub fn demod(self) -> &'static str {
        match self {
            ChannelType::Carrier => "cw",
            ChannelType::Am => "am",
            ChannelType::Nfm | ChannelType::KeyedVoice => "nfm",
            ChannelType::WfmBroadcast => "wfm",
            ChannelType::NarrowDigital | ChannelType::Digital | ChannelType::WideDigital => "",
        }
    }
}

/// Guesses a channel's type. `share_present` is the share of sweeps it was present in since first seen, `count` how many times it
/// opened, `peak_to_mean_db` how far its peak stands over its mean (small for flat-topped digital signals), `res_hz` the resolution
/// (the larger of the grid's bin and the window's -6 dB width): a signal no wider than that is narrower than the sweep can tell, and
/// is taken for a carrier.
pub fn guess_type(
    width_hz: f64,
    centre_hz: f64,
    share_present: f64,
    count: u32,
    peak_to_mean_db: f32,
    res_hz: f64,
) -> ChannelType {
    let keyed = share_present < 0.6 || count >= 3;
    let flat = peak_to_mean_db < 2.0 && width_hz > 2.0 * res_hz;
    let airband = (108e6..=137e6).contains(&centre_hz);
    let hf = centre_hz < 30e6;
    let fm_band = (76e6..=108e6).contains(&centre_hz);
    let w = width_hz;
    if w <= 1e3 || w <= 1.5 * res_hz {
        if keyed {
            ChannelType::NarrowDigital
        } else {
            ChannelType::Carrier
        }
    } else if w < 4e3 {
        ChannelType::NarrowDigital
    } else if w <= 20e3 {
        if airband || hf {
            ChannelType::Am
        } else if flat {
            ChannelType::Digital
        } else if keyed {
            ChannelType::KeyedVoice
        } else {
            ChannelType::Nfm
        }
    } else if w <= 100e3 {
        ChannelType::Digital
    } else if w <= 300e3 {
        if flat && !fm_band {
            ChannelType::WideDigital
        } else {
            ChannelType::WfmBroadcast
        }
    } else {
        ChannelType::WideDigital
    }
}

/// A channel raster: centres snap to `origin_hz + k × step_hz`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Raster {
    /// A channel centre, Hz.
    pub origin_hz: f64,
    /// The spacing, Hz.
    pub step_hz: f64,
}

impl Raster {
    /// The nearest channel centre.
    pub fn snap(&self, hz: f64) -> f64 {
        if self.step_hz <= 0.0 {
            return hz;
        }
        self.origin_hz + ((hz - self.origin_hz) / self.step_hz).round() * self.step_hz
    }
}

/// Why a channel is hidden.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// The mirror of a stronger signal about a hop centre.
    pub image: bool,
    /// Made by the radio (the same offset from the hop centre in several hops).
    pub spur: bool,
    /// The DC spike at a hop centre.
    pub dc: bool,
}

impl Flags {
    /// Any flag.
    pub fn any(&self) -> bool {
        self.image || self.spur || self.dc
    }
}

/// A channel in the list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// The track's id.
    pub id: u64,
    /// The centre, Hz (snapped to the raster when there is one).
    pub centre_hz: f64,
    /// The width, Hz.
    pub width_hz: f64,
    /// The best SNR, dB.
    pub snr_db: f32,
    /// The last peak, dBFS.
    pub peak_db: f32,
    /// The guessed type.
    pub kind: ChannelType,
    /// Sweeps seen in.
    pub sweeps_seen: u64,
    /// The share of sweeps present.
    pub share_present: f64,
    /// Times it opened.
    pub count: u32,
    /// First and last seen, s.
    pub first_seen_s: f64,
    /// Last seen, s.
    pub last_seen_s: f64,
    /// Open now.
    pub active: bool,
    /// Seen in enough sweeps to show.
    pub confirmed: bool,
    /// Why it is hidden.
    pub flags: Flags,
    /// Its hop is visited more often.
    pub priority: bool,
}

/// How the list is built.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelConfig {
    /// Sweeps a channel must be seen in to show.
    pub min_sweeps: u64,
    /// An image is at least this much weaker than its mirror, dB.
    pub image_margin_db: f32,
    /// A spur is at the same hop offset in at least this many hops.
    pub spur_hops: usize,
    /// The channel raster, when there is one.
    pub raster: Option<Raster>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        ChannelConfig {
            min_sweeps: 2,
            image_margin_db: 20.0,
            spur_hops: 3,
            raster: None,
        }
    }
}

sdrkit_core::choice_enum! {
    /// What to do with a channel.
    pub enum HitAction {
        /// Tunes a session to it.
        Tune = ("tune", "Tune"),
        /// Saves it to the memories.
        Save = ("save", "Save"),
        /// Locks it out: its bins are masked from now on.
        Lockout = ("lockout", "Lock out"),
        /// Visits its hop more often.
        Priority = ("priority", "Priority"),
        /// Removes its row.
        Remove = ("remove", "Remove"),
    }
}

/// What an action did, for the scan and the reply.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ActionOutcome {
    /// Tune to the channel.
    Tune,
    /// Save the channel.
    Save,
    /// This range is locked out.
    Lockout(Range),
    /// This frequency's hop is a priority hop.
    Priority(f64),
    /// The row was removed.
    Removed,
}

/// The channel list.
#[derive(Debug, Default)]
pub struct ChannelList {
    cfg: ChannelConfig,
    channels: Vec<Channel>,
    lockouts: Vec<Range>,
    priority: Vec<u64>,
    removed: Vec<u64>,
    hops: Vec<usize>,
}

impl ChannelList {
    /// An empty list.
    pub fn new(cfg: ChannelConfig) -> Self {
        ChannelList {
            cfg,
            ..Default::default()
        }
    }

    /// All channels, flagged and unconfirmed ones included.
    pub fn all(&self) -> &[Channel] {
        &self.channels
    }

    #[cfg(test)]
    /// The channels to show: confirmed, and without flags unless `show_flagged`.
    pub fn visible(&self, show_flagged: bool) -> impl Iterator<Item = &Channel> {
        self.channels
            .iter()
            .filter(move |c| c.confirmed && (show_flagged || !c.flags.any()))
    }

    /// Adds a locked-out range.
    pub fn add_lockout(&mut self, r: Range) {
        self.lockouts.push(r);
        self.channels
            .retain(|c| !(c.centre_hz >= r.low_hz && c.centre_hz <= r.high_hz));
    }

    /// Rebuilds the list from the tracks. `res_hz` is the resolution (see [`guess_type`]).
    pub fn update(&mut self, tracks: &[Track], plan: &HopPlan, res_hz: f64) {
        self.channels.clear();
        for t in tracks {
            if self.removed.contains(&t.id) {
                continue;
            }
            if self
                .lockouts
                .iter()
                .any(|r| t.centre_hz >= r.low_hz && t.centre_hz <= r.high_hz)
            {
                continue;
            }
            let centre = match self.cfg.raster {
                Some(r) => r.snap(t.centre_hz),
                None => t.centre_hz,
            };
            self.channels.push(Channel {
                id: t.id,
                centre_hz: centre,
                width_hz: t.width_hz,
                snr_db: t.best_snr_db,
                peak_db: t.peak_db,
                kind: guess_type(
                    t.width_hz,
                    t.centre_hz,
                    t.share_present(),
                    t.count,
                    t.peak_to_mean_db,
                    res_hz,
                ),
                sweeps_seen: t.sweeps_seen,
                share_present: t.share_present(),
                count: t.count,
                first_seen_s: t.first_seen_s,
                last_seen_s: t.last_seen_s,
                active: t.is_open(),
                confirmed: t.sweeps_seen >= self.cfg.min_sweeps,
                flags: Flags::default(),
                priority: self.priority.contains(&t.id),
            });
        }
        self.flag(tracks, plan, res_hz);
        self.channels
            .sort_unstable_by(|a, b| a.centre_hz.total_cmp(&b.centre_hz).then(a.id.cmp(&b.id)));
    }

    fn flag(&mut self, tracks: &[Track], plan: &HopPlan, res_hz: f64) {
        let tol = 2.0 * res_hz;
        let n = self.channels.len();
        // Offsets from the hop centre that saw each channel.
        let hop_of = |hz: f64| plan.hop_for(hz).map(|h| (h.index, h.centre_hz));
        for i in 0..n {
            let (hz, width, peak) = {
                let c = &self.channels[i];
                let t = tracks.iter().find(|t| t.id == c.id);
                (
                    t.map_or(c.centre_hz, |t| t.centre_hz),
                    c.width_hz,
                    c.peak_db,
                )
            };
            let Some((hop, hc)) = hop_of(hz) else {
                continue;
            };
            let off = hz - hc;
            // DC: right at the hop centre and narrow.
            if off.abs() <= 1.5 * res_hz && width <= 1.5 * res_hz {
                self.channels[i].flags.dc = true;
            }
            // Image: a much stronger signal at the mirror about the hop centre.
            let mirror = hc - off;
            if off.abs() > 2.0 * res_hz {
                let image = self.channels.iter().enumerate().any(|(j, o)| {
                    j != i
                        && (o.centre_hz - mirror).abs() <= tol.max(o.width_hz / 2.0)
                        && o.peak_db >= peak + self.cfg.image_margin_db
                });
                if image {
                    self.channels[i].flags.image = true;
                }
            }
            // Spur: narrow signals at the same offset in several hops.
            if width <= 1.5 * res_hz {
                let hops = &mut self.hops;
                hops.clear();
                hops.push(hop);
                for (j, o) in self.channels.iter().enumerate() {
                    if j == i || o.width_hz > 1.5 * res_hz {
                        continue;
                    }
                    if let Some((h2, c2)) = hop_of(o.centre_hz) {
                        if !hops.contains(&h2) && ((o.centre_hz - c2) - off).abs() <= res_hz {
                            hops.push(h2);
                        }
                    }
                }
                if hops.len() >= self.cfg.spur_hops {
                    self.channels[i].flags.spur = true;
                }
            }
        }
    }

    /// The channel `id`.
    pub fn get(&self, id: u64) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == id)
    }

    /// Applies `action` to channel `id`. `None` if there is no such channel.
    pub fn apply(&mut self, id: u64, action: HitAction) -> Option<ActionOutcome> {
        let c = *self.get(id)?;
        Some(match action {
            HitAction::Tune => ActionOutcome::Tune,
            HitAction::Save => ActionOutcome::Save,
            HitAction::Lockout => {
                let half = c.width_hz / 2.0 + 1.0;
                let r = Range::new(c.centre_hz - half, c.centre_hz + half);
                self.add_lockout(r);
                ActionOutcome::Lockout(r)
            }
            HitAction::Priority => {
                if !self.priority.contains(&id) {
                    self.priority.push(id);
                }
                if let Some(ch) = self.channels.iter_mut().find(|ch| ch.id == id) {
                    ch.priority = true;
                }
                ActionOutcome::Priority(c.centre_hz)
            }
            HitAction::Remove => {
                self.removed.push(id);
                self.channels.retain(|ch| ch.id != id);
                ActionOutcome::Removed
            }
        })
    }

    /// Fills `mask` (one entry per grid bin) with the locked-out and excluded bins.
    pub fn mask_bins(&self, grid: &crate::grid::Grid, exclusions: &[Range], mask: &mut Vec<bool>) {
        mask.clear();
        mask.resize(grid.bins, false);
        for r in self.lockouts.iter().chain(exclusions) {
            for m in &mut mask[grid.bins_between(r.low_hz, r.high_hz)] {
                *m = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hits::{Detection, HitConfig, HitTracker};
    use crate::plan::PlanConfig;

    #[test]
    fn type_guesses() {
        let bin = 1e3;
        assert_eq!(
            guess_type(1e3, 145e6, 1.0, 1, 0.0, bin),
            ChannelType::Carrier
        );
        assert_eq!(
            guess_type(1e3, 433e6, 0.2, 5, 0.0, bin),
            ChannelType::NarrowDigital
        );
        assert_eq!(guess_type(8e3, 120e6, 0.4, 4, 6.0, bin), ChannelType::Am);
        assert_eq!(guess_type(12e3, 7.1e6, 1.0, 1, 6.0, bin), ChannelType::Am);
        assert_eq!(guess_type(12e3, 145e6, 1.0, 1, 6.0, bin), ChannelType::Nfm);
        assert_eq!(
            guess_type(12e3, 145e6, 0.3, 6, 6.0, bin),
            ChannelType::KeyedVoice
        );
        assert_eq!(
            guess_type(12e3, 446e6, 1.0, 1, 1.0, bin),
            ChannelType::Digital
        );
        assert_eq!(
            guess_type(60e3, 446e6, 1.0, 1, 1.0, bin),
            ChannelType::Digital
        );
        assert_eq!(
            guess_type(200e3, 98e6, 1.0, 1, 5.0, bin),
            ChannelType::WfmBroadcast
        );
        assert_eq!(
            guess_type(200e3, 868e6, 1.0, 1, 1.0, bin),
            ChannelType::WideDigital
        );
        assert_eq!(
            guess_type(1.5e6, 220e6, 1.0, 1, 1.0, bin),
            ChannelType::WideDigital
        );
        assert_eq!(ChannelType::Nfm.demod(), "nfm");
        let r = Raster {
            origin_hz: 0.0,
            step_hz: 12.5e3,
        };
        assert_eq!(r.snap(145.0063e6), 145.0125e6);
    }

    fn det(hz: f64, peak: f32, width: f64) -> Detection {
        Detection {
            start: 0,
            end: 1,
            centre_hz: hz,
            width_hz: width,
            peak_hz: hz,
            peak_db: peak,
            floor_db: -90.0,
            snr_db: peak + 90.0,
            peak_to_mean_db: 3.0,
        }
    }

    #[test]
    fn confirmed_flags_and_actions() {
        let mut cfg = PlanConfig::new(100e6, 110e6, 2e6);
        cfg.overlap = 0.0;
        let plan = HopPlan::new(&cfg).unwrap();
        let bin = 10e3;
        let h = plan.hops[2];
        let mut tracker = HitTracker::new(HitConfig::default());
        let dets = vec![
            det(h.centre_hz + 300e3, -30.0, 20e3),            // strong
            det(h.centre_hz - 300e3, -70.0, 10e3),            // its image
            det(plan.hops[0].centre_hz + 200e3, -75.0, 10e3), // spur in three hops
            det(plan.hops[1].centre_hz + 200e3, -75.0, 10e3),
            det(plan.hops[4].centre_hz + 200e3, -75.0, 10e3),
            det(h.centre_hz + 5e3, -60.0, 10e3), // DC
        ];
        let mut list = ChannelList::new(ChannelConfig::default());
        tracker.update(0.0, &dets, bin);
        list.update(tracker.tracks(), &plan, bin);
        assert_eq!(
            list.visible(true).count(),
            0,
            "not confirmed after one sweep"
        );
        tracker.update(1.0, &dets, bin);
        list.update(tracker.tracks(), &plan, bin);
        assert_eq!(list.visible(true).count(), 6);
        let shown: Vec<f64> = list.visible(false).map(|c| c.centre_hz).collect();
        assert_eq!(shown.len(), 1, "{:?}", list.all());
        assert!((shown[0] - (h.centre_hz + 300e3)).abs() < 1.0);
        let all = list.all();
        assert!(all.iter().any(|c| c.flags.image));
        assert_eq!(all.iter().filter(|c| c.flags.spur).count(), 3);
        assert!(all.iter().any(|c| c.flags.dc));

        let strong = list.visible(false).next().unwrap().id;
        assert_eq!(
            list.apply(strong, HitAction::Tune),
            Some(ActionOutcome::Tune)
        );
        assert_eq!(list.get(strong).map(|c| c.kind.demod()), Some("nfm"));
        assert_eq!(
            list.apply(strong, HitAction::Save),
            Some(ActionOutcome::Save)
        );
        assert!(matches!(
            list.apply(strong, HitAction::Priority),
            Some(ActionOutcome::Priority(_))
        ));
        let Some(ActionOutcome::Lockout(range)) = list.apply(strong, HitAction::Lockout) else {
            panic!()
        };
        let g = crate::grid::Grid::new(100e6, 110e6, bin).unwrap();
        let mut mask = Vec::new();
        list.mask_bins(&g, &[], &mut mask);
        assert!(mask[g.index_of(range.low_hz + 1.0).unwrap()]);
        tracker.update(2.0, &dets, bin);
        list.update(tracker.tracks(), &plan, bin);
        assert!(list
            .all()
            .iter()
            .all(|c| (c.centre_hz - (h.centre_hz + 300e3)).abs() > 1.0));
        let any = list.all()[0].id;
        assert_eq!(
            list.apply(any, HitAction::Remove),
            Some(ActionOutcome::Removed)
        );
        tracker.update(3.0, &dets, bin);
        list.update(tracker.tracks(), &plan, bin);
        assert!(list.all().iter().all(|c| c.id != any));
    }
}
