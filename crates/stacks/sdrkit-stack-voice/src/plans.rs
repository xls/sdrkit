//! The channel plan of the voice channel stack. The host supplies it as a plan file of `sdrkit-data-plans`
//! (`docs/schema/plans.json`); the stack holds no plan of its own. A plan is a receive aid. National rules decide what may be
//! used.
//!
//! What the file says about a channel:
//!
//! | In the file | Meaning here |
//! |---|---|
//! | band `modulation` | `nfm` (or `fm`), `am`, `usb`, `lsb`: the demodulator. Empty: from the signal or the submode. Any other word: no speech, the channel is shown and never played |
//! | channel `pair_hz` | the repeater input |
//! | channel `in_use: false` | shown, skipped by the scan |
//! | channel `note` | the use of the channel. The format has no field for a schedule: a note that ends in `; minutes 20-24, 50-54` gives the minutes of the hour the channel is on the air |

use crate::rx::Demod;
use sdrkit_core::Choice;
use sdrkit_data_plans::api::{import_as, PlanFormat};
use sdrkit_data_plans::{band_channels, BandGroup, PlanError, Plans};

sdrkit_core::choice_enum! {
    /// The services of the stack: its submodes.
    pub enum Service {
        /// Citizens band.
        Cb = ("cb", "CB radio"),
        /// FRS and GMRS.
        Frs = ("frs", "FRS / GMRS"),
        /// PMR446.
        Pmr446 = ("pmr446", "PMR446 analog"),
        /// Amateur FM repeaters.
        HamRepeaters = ("ham_repeaters", "Ham FM repeaters"),
        /// HF voice.
        HfVoice = ("hfvoice", "HFGCS / HF voice"),
    }
}

impl Service {
    /// The name of the service, for messages.
    pub fn title(self) -> std::borrow::Cow<'static, str> {
        Service::CHOICES[usize::from(self.index())].label.clone()
    }
}

/// The tuner is this far above the channel that a stepping scan of a sideband service listens to, Hz.
pub const SIDEBAND_TUNE_OFFSET_HZ: f64 = 12e3;

/// What a submode fixes: how its receivers work without a plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// The demodulator. `None` chooses AM, FM or SSB from the signal.
    pub demod: Option<Demod>,
    /// The raster step when no setting and no plan gives one, Hz.
    pub step_hz: f64,
    /// The channel width when no setting and no plan gives one, Hz.
    pub width_hz: f64,
    /// A sideband service: a transmission has no carrier, its speech lies above the channel frequency, the squelch follows
    /// the carrier finder alone, and the scan always steps the radio.
    pub sideband: bool,
    /// The raster cell names the channel and the receiver stays on it, whatever sideband the signal is on.
    pub on_raster: bool,
    /// How the carrier finder searches the raster.
    pub search: Search,
}

/// How the carrier finder searches a raster.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Search {
    /// The time above the threshold before a carrier is reported, s.
    pub report_after_s: f64,
    /// The signal one spectrum estimate covers, s.
    pub estimate_s: f64,
    /// A quiet carrier stays tracked this long, s.
    pub keep_s: f64,
    /// A cell this far below a neighbour is its skirt, dB.
    pub skirt_db: f64,
    /// The least signal-to-noise ratio of a new carrier, dB.
    pub min_snr_db: Option<f64>,
    /// Half the width a cell's power is summed over, as a part of the step.
    pub cell_half: f64,
    /// Half the width the occupied width is measured over, as a part of the step.
    pub width_half: f64,
}

impl Search {
    /// Narrow voice channels: a carrier fills less than its cell.
    pub const NARROW: Search = Search {
        report_after_s: 0.12,
        estimate_s: 0.05,
        keep_s: 5.0,
        skirt_db: 30.0,
        min_snr_db: None,
        cell_half: 0.4,
        width_half: 0.5,
    };
    /// Wide carriers on a coarse raster: the cells touch, and a carrier between two cells is measured over both.
    pub const WIDE: Search = Search {
        report_after_s: 0.3,
        estimate_s: 0.1,
        keep_s: 8.0,
        skirt_db: 20.0,
        min_snr_db: Some(6.0),
        cell_half: 0.5,
        width_half: 1.0,
    };
}

/// Narrow FM carriers on a 12.5 kHz raster with no service: the profile of the MDC-1200 stack.
pub const FM_CARRIERS: Profile = Profile {
    demod: Some(Demod::Fm),
    step_hz: 12.5e3,
    width_hz: 12.5e3,
    sideband: false,
    on_raster: false,
    search: Search::NARROW,
};

/// Wide FM carriers on a 100 kHz raster: the profile of the wireless microphone finder.
pub const WIDE_CARRIERS: Profile = Profile {
    demod: Some(Demod::Fm),
    step_hz: 100e3,
    width_hz: 160e3,
    sideband: false,
    on_raster: false,
    search: Search::WIDE,
};

impl Profile {
    /// The cut-off of the channel filter for a channel `width_hz` wide, Hz.
    pub fn cutoff_hz(&self, width_hz: f64) -> f64 {
        if self.sideband {
            4e3
        } else {
            (0.45 * width_hz).clamp(3e3, 7e3)
        }
    }
}

/// The profile of `service`.
pub fn profile(service: Service) -> Profile {
    match service {
        Service::Cb => Profile {
            demod: None,
            step_hz: 10e3,
            width_hz: 10e3,
            sideband: false,
            on_raster: true,
            search: Search::NARROW,
        },
        Service::Frs | Service::Pmr446 | Service::HamRepeaters => Profile {
            demod: Some(Demod::Fm),
            step_hz: 6.25e3,
            width_hz: 12.5e3,
            sideband: false,
            on_raster: false,
            search: Search::NARROW,
        },
        Service::HfVoice => Profile {
            demod: Some(Demod::Usb),
            step_hz: 1e3,
            width_hz: 2.7e3,
            sideband: true,
            on_raster: false,
            search: Search::NARROW,
        },
    }
}

/// One channel of a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// The number: the label when it is a number, else the place in the plan from 1.
    pub number: u32,
    /// The label ("19", "U5", "RV49", "HFGCS 11175").
    pub label: String,
    /// Where to listen, Hz.
    pub hz: f64,
    /// The repeater input, Hz. `None` for simplex.
    pub input_hz: Option<f64>,
    /// The demodulator. `None` leaves it to the submode.
    pub demod: Option<Demod>,
    /// The channel width, Hz.
    pub width_hz: f64,
    /// The use of the channel.
    pub name: String,
    /// No speech (telemetry, data). Shown, never played, not scanned.
    pub no_speech: bool,
    /// Not in use (reserved). Shown, skipped by the scan.
    pub skipped: bool,
    /// The minutes of the hour it is on the air, first and last of each broadcast. Empty for a channel without a schedule.
    pub minutes: Vec<(u8, u8)>,
}

impl Channel {
    /// The channel broadcasts in UTC minute `minute` of the hour. True for a channel without a schedule.
    pub fn on_air(&self, minute: u8) -> bool {
        self.minutes.is_empty()
            || self.minutes.iter().any(|&(a, b)| {
                (i32::from(minute) - i32::from(a)).rem_euclid(60)
                    <= (i32::from(b) - i32::from(a)).rem_euclid(60)
            })
    }

    /// The start minute of the next broadcast after minute `minute`. `None` for a channel without a schedule.
    pub fn next_start(&self, minute: u8) -> Option<u8> {
        self.minutes
            .iter()
            .map(|&(a, _)| a)
            .min_by_key(|&s| (i32::from(s) - i32::from(minute)).rem_euclid(60))
    }
}

/// A channel plan as loaded from a file.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The id of the plan in the file.
    pub id: String,
    /// The name of the plan.
    pub name: String,
    /// The channels in file order. The index of a channel is its key.
    pub channels: Vec<Channel>,
}

/// The form of a label that lookups compare: lower case, no spaces.
fn squeezed(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The demodulator of the band modulation `text`, and whether the band carries no speech.
fn demod_of(text: &str) -> (Option<Demod>, bool) {
    match text.trim().to_ascii_lowercase().as_str() {
        "" => (None, false),
        "nfm" | "fm" => (Some(Demod::Fm), false),
        "am" => (Some(Demod::Am), false),
        "usb" => (Some(Demod::Usb), false),
        "lsb" => (Some(Demod::Lsb), false),
        _ => (None, true),
    }
}

/// A channel note as its text and the schedule that it ends in (`; minutes 20-24, 50-54`). A note whose ending is not a
/// list of minutes below 60 is all text.
fn schedule_of(note: &str) -> (String, Vec<(u8, u8)>) {
    const MARK: &str = "minutes ";
    let (text, list) = match note.rfind("; minutes ") {
        Some(at) => (&note[..at], &note[at + 2 + MARK.len()..]),
        None => match note.strip_prefix(MARK) {
            Some(list) => ("", list),
            None => return (note.to_owned(), Vec::new()),
        },
    };
    let minute = |s: &str| s.trim().parse::<u8>().ok().filter(|m| *m < 60);
    let ranges: Option<Vec<(u8, u8)>> = list
        .split(',')
        .map(|r| match r.split_once('-') {
            Some((a, b)) => Some((minute(a)?, minute(b)?)),
            None => minute(r).map(|m| (m, m)),
        })
        .collect();
    match ranges {
        Some(r) if !r.is_empty() => (text.trim().to_owned(), r),
        _ => (note.to_owned(), Vec::new()),
    }
}

impl Plan {
    /// The plan of the band group `group`: the listed and numbered channels of its bands, in file order.
    pub fn of_group(group: &BandGroup) -> Plan {
        let mut channels: Vec<Channel> = Vec::new();
        for band in &group.bands {
            let (demod, no_speech) = demod_of(&band.modulation);
            for c in band_channels(band) {
                let (name, minutes) = schedule_of(&c.note);
                let place = channels.len() as u32 + 1;
                channels.push(Channel {
                    number: c.name.trim().parse().unwrap_or(place),
                    label: c.name,
                    hz: c.centre_hz,
                    input_hz: c.pair_hz,
                    demod,
                    width_hz: c.width_hz,
                    name,
                    no_speech,
                    skipped: c.reallocated,
                    minutes,
                });
            }
        }
        Plan {
            id: group.id.clone(),
            name: group.name.clone(),
            channels,
        }
    }

    /// The index of the channel with label `label` ("19", "ch 19", "Ch.19", "channel 19", "u5", "rv48"; spaces and letter
    /// case do not count).
    pub fn by_label(&self, label: &str) -> Option<usize> {
        let want = squeezed(label);
        let find = |w: &str| self.channels.iter().position(|c| squeezed(&c.label) == w);
        find(&want).or_else(|| {
            ["channel", "ch.", "ch"]
                .iter()
                .find_map(|p| want.strip_prefix(p))
                .and_then(find)
        })
    }

    /// The index of the channel nearest to `hz` within `tolerance_hz`. With `inputs` a repeater input matches its channel.
    pub fn by_frequency(&self, hz: f64, tolerance_hz: f64, inputs: bool) -> Option<usize> {
        self.nearest(hz, inputs, |_| tolerance_hz)
    }

    fn nearest(&self, hz: f64, inputs: bool, tolerance: impl Fn(&Channel) -> f64) -> Option<usize> {
        self.channels
            .iter()
            .enumerate()
            .flat_map(|(i, c)| {
                let input = c.input_hz.filter(|_| inputs);
                let limit = tolerance(c);
                [Some(c.hz), input]
                    .into_iter()
                    .flatten()
                    .map(move |f| (i, (f - hz).abs(), limit))
            })
            .filter(|(_, d, limit)| d <= limit)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _, _)| i)
    }

    /// The channel a carrier at `hz` belongs to: the nearest one within 0.6 of its width. In a sideband service the carrier
    /// must lie from 600 Hz below to 3600 Hz above the channel frequency, and the best match is nearest to 1500 Hz above
    /// it.
    pub fn match_carrier(&self, hz: f64, sideband: bool) -> Option<usize> {
        if !sideband {
            return self.nearest(hz, false, |c| 0.6 * c.width_hz);
        }
        self.channels
            .iter()
            .enumerate()
            .filter(|(_, c)| (-600.0..=3600.0).contains(&(hz - c.hz)))
            .min_by(|a, b| {
                (hz - a.1.hz - 1500.0)
                    .abs()
                    .total_cmp(&(hz - b.1.hz - 1500.0).abs())
            })
            .map(|(i, _)| i)
    }

    /// The raster step that fits the plan: its narrowest channel, Hz. `None` for a plan without channels.
    pub fn step_hz(&self) -> Option<f64> {
        self.channels
            .iter()
            .map(|c| c.width_hz)
            .filter(|w| *w > 0.0)
            .reduce(f64::min)
    }

    /// An absolute frequency the raster of the plan passes through: its first channel.
    pub fn origin_hz(&self) -> Option<f64> {
        self.channels.first().map(|c| c.hz)
    }
}

/// The first plan of the plan file `text`, read and checked as the host's plan import reads it.
///
/// # Errors
///
/// [`PlanError`] if the text is not a valid plan file or holds no plan.
pub fn load(text: &str) -> Result<Plan, PlanError> {
    let mut plans = Plans::new();
    let ids = import_as(&mut plans, text, PlanFormat::Auto, "", "")?;
    ids.first()
        .and_then(|id| plans.group(id))
        .map(|entry| Plan::of_group(&entry.group))
        .ok_or_else(|| PlanError::Invalid(vec!["the file holds no plan".into()]))
}

/// One band of a band list: its name and its edges, Hz.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    /// The name.
    pub name: String,
    /// The lower edge, Hz.
    pub start_hz: f64,
    /// The upper edge, Hz.
    pub end_hz: f64,
}

/// The bands of a plan file, for a stack that searches bands and has no channels.
#[derive(Clone, Debug, PartialEq)]
pub struct Bands {
    /// The name of the plan.
    pub name: String,
    /// The bands, in file order.
    pub bands: Vec<Band>,
}

impl Bands {
    /// The name of the first band that holds `hz`.
    pub fn name_of(&self, hz: f64) -> Option<&str> {
        self.bands
            .iter()
            .find(|b| (b.start_hz..=b.end_hz).contains(&hz))
            .map(|b| b.name.as_str())
    }
}

/// The bands of the first plan of the plan file `text`.
///
/// # Errors
///
/// [`PlanError`] if the text is not a valid plan file or holds no plan.
pub fn load_bands(text: &str) -> Result<Bands, PlanError> {
    let mut plans = Plans::new();
    let ids = import_as(&mut plans, text, PlanFormat::Auto, "", "")?;
    ids.first()
        .and_then(|id| plans.group(id))
        .map(|entry| Bands {
            name: entry.group.name.clone(),
            bands: entry
                .group
                .bands
                .iter()
                .map(|b| Band {
                    name: b.name.clone(),
                    start_hz: b.start_hz,
                    end_hz: b.end_hz,
                })
                .collect(),
        })
        .ok_or_else(|| PlanError::Invalid(vec!["the file holds no plan".into()]))
}
