//! The plan view that receivers and hosts read: a plan is a list of named channels and a list of frequency segments with an
//! optional tuning raster. The loader builds one plan from each band group of the data.

pub use crate::data::{AnalogSystem, PlanKind};
use serde::{Deserialize, Serialize};

/// One channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    /// Its name as the plan prints it ("21", "E5", "9A", "12B", "16", "PMR 3").
    pub name: String,
    /// Centre frequency, Hz.
    pub centre_hz: f64,
    /// Occupied width, Hz.
    pub width_hz: f64,
    /// The channel is withdrawn from this service (reallocated to mobile broadband, or not assigned).
    #[serde(default, skip_serializing_if = "is_false")]
    pub reallocated: bool,
    /// The other frequency of a duplex pair (a marine coast station, a repeater output), Hz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair_hz: Option<f64>,
    /// A short note ("distress and calling", "AIS 1").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Channel {
    /// A channel with no flags.
    pub fn new(name: impl Into<String>, centre_hz: f64, width_hz: f64) -> Channel {
        Channel {
            name: name.into(),
            centre_hz,
            width_hz,
            reallocated: false,
            pair_hz: None,
            note: String::new(),
        }
    }

    /// The lower edge, Hz.
    pub fn lower_hz(&self) -> f64 {
        self.centre_hz - self.width_hz / 2.0
    }

    /// The upper edge, Hz.
    pub fn upper_hz(&self) -> f64 {
        self.centre_hz + self.width_hz / 2.0
    }

    /// With the reallocated flag set.
    pub fn reallocated(mut self, yes: bool) -> Channel {
        self.reallocated = yes;
        self
    }

    /// With a note.
    pub fn note(mut self, note: impl Into<String>) -> Channel {
        self.note = note.into();
        self
    }

    /// With the other frequency of a pair.
    pub fn pair(mut self, hz: f64) -> Channel {
        self.pair_hz = Some(hz);
        self
    }
}

/// A frequency range of a plan (a band, a sub-band), with an optional raster of channels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    /// Its name ("40 m", "MW", "UHF airband").
    pub name: String,
    /// Lower edge, Hz.
    pub lo_hz: f64,
    /// Upper edge, Hz.
    pub hi_hz: f64,
    /// The raster step, Hz (channels are `first_hz + k * step_hz` inside the segment).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_hz: Option<f64>,
    /// The first raster point, Hz (defaults to `lo_hz`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_hz: Option<f64>,
    /// The usual modulation there, as a modulation id ("lsb", "usb", "am", "nfm", "cw").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modulation: String,
    /// A short note.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Segment {
    /// A segment without a raster.
    pub fn new(name: impl Into<String>, lo_hz: f64, hi_hz: f64) -> Segment {
        Segment {
            name: name.into(),
            lo_hz,
            hi_hz,
            step_hz: None,
            first_hz: None,
            modulation: String::new(),
            note: String::new(),
        }
    }

    /// With a raster.
    pub fn raster(mut self, first_hz: f64, step_hz: f64) -> Segment {
        self.first_hz = Some(first_hz);
        self.step_hz = Some(step_hz);
        self
    }

    /// With the usual modulation.
    pub fn modulation(mut self, m: impl Into<String>) -> Segment {
        self.modulation = m.into();
        self
    }

    /// With a note.
    pub fn note(mut self, note: impl Into<String>) -> Segment {
        self.note = note.into();
        self
    }

    /// Whether `hz` is inside the segment (edges included, 1 Hz slack).
    pub fn contains(&self, hz: f64) -> bool {
        hz >= self.lo_hz - 1.0 && hz <= self.hi_hz + 1.0
    }

    /// The raster points inside the segment, lowest first (empty without a raster).
    pub fn raster_points(&self) -> Vec<f64> {
        let Some(step) = self.step_hz.filter(|s| *s > 0.0) else {
            return Vec::new();
        };
        let first = self.first_hz.unwrap_or(self.lo_hz);
        let n = ((self.hi_hz - first) / step + 1e-6).floor();
        if n < 0.0 {
            return Vec::new();
        }
        (0..=n as usize)
            .map(|k| round_hz(first + k as f64 * step))
            .collect()
    }

    /// The raster point nearest `hz`, held inside the segment; `hz` itself without a raster.
    pub fn snap(&self, hz: f64) -> f64 {
        let Some(step) = self.step_hz.filter(|s| *s > 0.0) else {
            return hz.clamp(self.lo_hz, self.hi_hz);
        };
        let first = self.first_hz.unwrap_or(self.lo_hz);
        let last = ((self.hi_hz - first) / step + 1e-6).floor().max(0.0);
        let k = ((hz - first) / step).round().clamp(0.0, last);
        round_hz(first + k * step)
    }
}

/// Rounds to 1 Hz (rasters such as 8.33 kHz keep their fraction below 1 Hz out of the result).
pub(crate) fn round_hz(hz: f64) -> f64 {
    hz.round()
}

/// How far a plan has been checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    /// Checked against the cited regulation or standard.
    Checked,
    /// Built from the cited source but not fully checked: a host must say so.
    Unchecked,
    /// Made by the user.
    User,
}

/// A channel plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelPlan {
    /// Stable id ("tv_eu", "fm_europe", "ham_r1").
    pub id: String,
    /// Name shown.
    pub name: String,
    /// What it is for.
    pub kind: PlanKind,
    /// ISO 3166 alpha-2 codes it is the default for.
    #[serde(default)]
    pub countries: Vec<String>,
    /// ITU regions (1 to 3) it is the default for when a country is not listed by any plan of its kind.
    #[serde(default)]
    pub itu_regions: Vec<u8>,
    /// The stack ids suggested for it, in the order to try ("dvbt2", "dvbt").
    #[serde(default)]
    pub modes: Vec<String>,
    /// The analogue system, for analogue TV plans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analog: Option<AnalogSystem>,
    /// The FM de-emphasis time constant, s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deemphasis_s: Option<f64>,
    /// Channels, in frequency order.
    #[serde(default)]
    pub channels: Vec<Channel>,
    /// Segments, in frequency order.
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// How far it has been checked.
    pub review: Review,
    /// Where the numbers come from (a regulation or standard).
    #[serde(default)]
    pub source: String,
    /// A note for the picker.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// Which way to step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// To a higher frequency.
    Up,
    /// To a lower frequency.
    Down,
}

/// A place in a plan: a channel, a raster point of a segment, or inside a segment off its raster.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Place {
    /// The plan id.
    pub plan: String,
    /// The name to show ("21", "E5", "40 m", "MW 1008 kHz").
    pub name: String,
    /// The channel centre or raster point, Hz.
    pub centre_hz: f64,
    /// The channel width, when it is a channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_hz: Option<f64>,
    /// The segment name, when inside one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<String>,
    /// The usual modulation there.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub modulation: String,
    /// The channel is reallocated.
    pub reallocated: bool,
    /// The analogue vision carrier, Hz (analogue TV plans).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vision_hz: Option<f64>,
    /// The analogue sound carrier, Hz (analogue TV plans).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_hz: Option<f64>,
    /// The other frequency of a duplex pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair_hz: Option<f64>,
    /// The offset of the requested frequency from `centre_hz`, Hz.
    pub offset_hz: f64,
}

impl ChannelPlan {
    /// Where the plan is the default, as tags: its country codes, then `itu1` to `itu3` for its ITU regions. Empty for a
    /// plan that names neither.
    pub fn scope(&self) -> Vec<String> {
        self.countries
            .iter()
            .cloned()
            .chain(self.itu_regions.iter().map(|r| format!("itu{r}")))
            .collect()
    }
    /// A plan with no channels or segments.
    pub fn new(id: &str, name: &str, kind: PlanKind, review: Review, source: &str) -> ChannelPlan {
        ChannelPlan {
            id: id.into(),
            name: name.into(),
            kind,
            countries: Vec::new(),
            itu_regions: Vec::new(),
            modes: Vec::new(),
            analog: None,
            deemphasis_s: None,
            channels: Vec::new(),
            segments: Vec::new(),
            review,
            source: source.into(),
            note: String::new(),
        }
    }

    /// The lowest and highest frequency it covers, Hz.
    pub fn span(&self) -> Option<(f64, f64)> {
        let lo = self
            .channels
            .iter()
            .map(Channel::lower_hz)
            .chain(self.segments.iter().map(|s| s.lo_hz))
            .fold(f64::INFINITY, f64::min);
        let hi = self
            .channels
            .iter()
            .map(Channel::upper_hz)
            .chain(self.segments.iter().map(|s| s.hi_hz))
            .fold(f64::NEG_INFINITY, f64::max);
        (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
    }

    /// The channel `hz` falls in, nearest centre first; with `tolerance_hz`, only a channel whose centre is that close.
    pub fn channel_at(&self, hz: f64, tolerance_hz: Option<f64>) -> Option<&Channel> {
        self.channels
            .iter()
            .filter(|c| match tolerance_hz {
                Some(t) => (c.centre_hz - hz).abs() <= t,
                None => hz >= c.lower_hz() - 1.0 && hz <= c.upper_hz() + 1.0,
            })
            .min_by(|a, b| {
                (a.centre_hz - hz)
                    .abs()
                    .total_cmp(&(b.centre_hz - hz).abs())
                    .then(a.reallocated.cmp(&b.reallocated))
            })
    }

    /// The segment `hz` falls in (the narrowest when segments nest).
    pub fn segment_at(&self, hz: f64) -> Option<&Segment> {
        self.segments
            .iter()
            .filter(|s| s.contains(hz))
            .min_by(|a, b| (a.hi_hz - a.lo_hz).total_cmp(&(b.hi_hz - b.lo_hz)))
    }

    /// Where `hz` is in this plan: a channel first, then a segment (with its nearest raster point).
    pub fn place(&self, hz: f64) -> Option<Place> {
        if let Some(c) = self.channel_at(hz, None) {
            let (vision, sound) = match &self.analog {
                Some(a) => {
                    let v = c.lower_hz() + a.vision_above_lower_hz;
                    (Some(v), Some(v + a.sound_spacing_hz))
                }
                None => (None, None),
            };
            return Some(Place {
                plan: self.id.clone(),
                name: c.name.clone(),
                centre_hz: c.centre_hz,
                width_hz: Some(c.width_hz),
                segment: self.segment_at(hz).map(|s| s.name.clone()),
                modulation: self
                    .segment_at(hz)
                    .map(|s| s.modulation.clone())
                    .unwrap_or_default(),
                reallocated: c.reallocated,
                vision_hz: vision,
                sound_hz: sound,
                pair_hz: c.pair_hz,
                offset_hz: hz - c.centre_hz,
            });
        }
        let s = self.segment_at(hz)?;
        let centre = s.snap(hz);
        let name = if s.step_hz.is_some() {
            format!("{} {}", s.name, format_frequency(centre))
        } else {
            s.name.clone()
        };
        Some(Place {
            plan: self.id.clone(),
            name,
            centre_hz: if s.step_hz.is_some() { centre } else { hz },
            width_hz: None,
            segment: Some(s.name.clone()),
            modulation: s.modulation.clone(),
            reallocated: false,
            vision_hz: None,
            sound_hz: None,
            pair_hz: None,
            offset_hz: if s.step_hz.is_some() {
                hz - centre
            } else {
                0.0
            },
        })
    }

    /// The channel with this name, as typed ("e5", "Ch 21", "S 11", "9a", "K21" all work where the plan has that channel).
    pub fn find(&self, typed: &str) -> Option<&Channel> {
        let want = normalise_name(typed);
        if want.is_empty() {
            return None;
        }
        if let Some(c) = self
            .channels
            .iter()
            .find(|c| normalise_name(&c.name) == want)
        {
            return Some(c);
        }
        // "21" finds "E21" or "U21", and "E21" finds "21", when only the letter prefix differs.
        let digits = |s: &str| -> String {
            s.trim_start_matches(|ch: char| ch.is_ascii_alphabetic())
                .to_string()
        };
        let want_tail = digits(&want);
        if want_tail.is_empty() || !want_tail.starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
        let want_prefix = &want[..want.len() - want_tail.len()];
        if !want_prefix.is_empty() && !["E", "U", "K", "CH", "C"].contains(&want_prefix) {
            return None;
        }
        let matches: Vec<&Channel> = self
            .channels
            .iter()
            .filter(|c| digits(&normalise_name(&c.name)) == want_tail)
            .collect();
        // A bare number is ambiguous when two prefixes share it (cable "S21" and UHF "21"): prefer a channel without a prefix.
        matches
            .iter()
            .find(|c| normalise_name(&c.name) == want_tail)
            .or_else(|| matches.first())
            .copied()
    }

    /// The next channel or raster point from `hz` in `dir`, skipping reallocated channels unless `include_reallocated`.
    /// Returns the frequency and its name. Wraps around at the ends.
    pub fn step(
        &self,
        hz: f64,
        dir: Direction,
        include_reallocated: bool,
    ) -> Option<(f64, String)> {
        let mut points: Vec<(f64, String)> = self
            .channels
            .iter()
            .filter(|c| include_reallocated || !c.reallocated)
            .map(|c| (c.centre_hz, c.name.clone()))
            .collect();
        for s in &self.segments {
            for p in s.raster_points() {
                points.push((p, format!("{} {}", s.name, format_frequency(p))));
            }
        }
        if points.is_empty() {
            return None;
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        points.dedup_by(|a, b| (a.0 - b.0).abs() < 0.5);
        let found = match dir {
            Direction::Up => points.iter().find(|p| p.0 > hz + 0.5).or(points.first()),
            Direction::Down => points
                .iter()
                .rev()
                .find(|p| p.0 < hz - 0.5)
                .or(points.last()),
        };
        found.cloned()
    }

    /// Returns every channel and raster point, lowest first. Reallocated channels are included only if `include_reallocated` is true.
    pub fn tune_points(&self, include_reallocated: bool) -> Vec<f64> {
        let mut v: Vec<f64> = self
            .channels
            .iter()
            .filter(|c| include_reallocated || !c.reallocated)
            .map(|c| c.centre_hz)
            .collect();
        for s in &self.segments {
            v.extend(s.raster_points());
        }
        v.sort_by(f64::total_cmp);
        v.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        v
    }
}

/// A name as typed, in one spelling: upper case, no spaces, dashes or underscores, without a leading "CHANNEL" or "CH".
pub fn normalise_name(s: &str) -> String {
    let t: String = s
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_' | '.'))
        .collect::<String>()
        .to_ascii_uppercase();
    for p in ["CHANNEL", "CHAN", "CH"] {
        if let Some(r) = t.strip_prefix(p) {
            if !r.is_empty() && r.starts_with(|c: char| c.is_ascii_digit()) {
                return r.to_string();
            }
        }
    }
    t
}

/// A frequency in the unit a person reads: kHz below 30 MHz ("1008 kHz", "9650 kHz"), MHz above ("145.5 MHz"), GHz from 1 GHz.
pub fn format_frequency(hz: f64) -> String {
    let trim = |v: f64, decimals: usize| -> String {
        let s = format!("{v:.decimals$}");
        let s = if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        };
        s
    };
    if hz >= 1e9 {
        format!("{} GHz", trim(hz / 1e9, 6))
    } else if hz >= 30e6 {
        format!("{} MHz", trim(hz / 1e6, 4))
    } else {
        format!("{} kHz", trim(hz / 1e3, 2))
    }
}
