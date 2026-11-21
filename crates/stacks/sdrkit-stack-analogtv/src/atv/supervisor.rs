//! The supervisor: search, trial and run, the bad-carrier list, carrier follow, system identification, the FM video polarity
//! trials, the `channel.tune` command (a request to the host), and the snapshot of each report for the status component.

use super::status::{AtvReport, StatusItem};
use super::{
    last, A2Report, Candidates, Modulation, Monitor, NbtvReport, NicamReport, Phase, PictureReport,
    SelectorReport, SoundReport, SyncReport, Tune, VisionReport, REPORT_TICKS, TICK_S,
};
use crate::channels::{Channel, ATV_CHANNELS_CAP};
use crate::results::{DetectorUsed, FmPolarity, SoundPlaying, State};
use crate::settings::{
    ChannelWidth, ColourChoice, DetectorChoice, SoundChoice, SystemChoice, CHANNELS, CHANNEL_WIDTH,
    COLOUR, DETECTOR, SOUND_SYSTEM, SYSTEM,
};
use crate::systems::{identify, Colour, System, SystemId};
use crate::typed::{AtvChannelArgs, AtvStateEvent, ATV_CHANNELS, ATV_CHANNEL_TUNE, ATV_STATE};
use sdrkit_core::command::CommandSet;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::{
    Choice, CommandError, CommandId, Commands, ConfigError, HostRequest, Record, SettingError,
    SettingId, SettingSet, Value, Values,
};
use std::sync::Arc;

use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};

sdrkit_core::commands! {
    /// The commands of the supervisor.
    pub enum SupervisorCommand {
        /// Tunes to a channel.
        ChannelTune = ATV_CHANNEL_TUNE.command,
    }
}

sdrkit_core::settings! {
    /// The settings of the supervisor.
    pub enum SupervisorSetting {
        /// The system.
        System = SYSTEM,
        /// The detector.
        Detector = DETECTOR,
        /// The channel width.
        ChannelWidth = CHANNEL_WIDTH,
        /// The channels.
        Channels = CHANNELS,
        /// The sound.
        SoundSystem = SOUND_SYSTEM,
        /// The colour system.
        Colour = COLOUR,
    }
}

/// The status snapshot of the `atv` stack at each report. The results fields are written from it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AtvDetail {
    /// How far the receiver has come.
    pub state: State,
    /// The modulation.
    pub modulation: Modulation,
    /// "PAL B/G 625/50", empty until known.
    pub system: String,
    /// The colour system, once decided.
    pub colour_system: Option<Colour>,
    /// The picture has colour.
    pub colour: bool,
    /// The colour killer is on.
    pub colour_killer: bool,
    /// Lines a frame.
    pub lines: u32,
    /// Fields a second.
    pub field_hz: f64,
    /// Line rate, Hz.
    pub line_hz: f64,
    /// Line error, ppm.
    pub line_error_ppm: f64,
    /// The detector.
    pub detector: DetectorUsed,
    /// Vision carrier, Hz from the centre.
    pub vision_hz: f64,
    /// Sound carrier, Hz from the centre (0: none).
    pub sound_hz: f64,
    /// Sound spacing, MHz (negative below the carrier).
    pub sound_spacing_mhz: f64,
    /// Carrier offset from its place, Hz.
    pub carrier_offset_hz: f64,
    /// Carrier level, dBFS.
    pub carrier_dbfs: f64,
    /// Carrier to noise, dB.
    pub carrier_to_noise_db: f64,
    /// Video SNR, dB.
    pub video_snr_db: f64,
    /// Sync found (0..1).
    pub sync_quality: f64,
    /// Sync depth, %.
    pub sync_depth_pct: f64,
    /// Sync compression, %.
    pub sync_compression_pct: f64,
    /// The white peak.
    pub white_peak: f64,
    /// Burst level (share of nominal).
    pub burst_level: f64,
    /// Burst phase error, degrees.
    pub burst_phase_error_deg: f64,
    /// SECAM chroma level.
    pub chroma_level: f64,
    /// How SECAM was found (empty until it is).
    pub secam_identification: &'static str,
    /// The sound that plays.
    pub sound_system: SoundPlaying,
    /// A sound carrier is there.
    pub sound_present: bool,
    /// Peak deviation, kHz.
    pub sound_deviation_khz: f64,
    /// Audio level, dBFS.
    pub sound_level_dbfs: f64,
    /// Lines since the loop started.
    pub line_count: u64,
    /// Field syncs.
    pub field_count: u64,
    /// Pictures published.
    pub pictures: u64,
    /// Snow pictures published.
    pub snow_pictures: u64,
    /// Fields decoded completely.
    pub fields_ok: u64,
    /// Fields damaged.
    pub fields_bad: u64,
    /// The last field (1 or 2).
    pub last_field: u8,
    /// The FM video polarity.
    pub fm_polarity: Option<FmPolarity>,
    /// NBTV line locked.
    pub nbtv_line_locked: bool,
    /// NBTV picture locked.
    pub nbtv_picture_locked: bool,
    /// NICAM state.
    pub nicam: Option<NicamReport>,
    /// A2 state.
    pub a2: Option<A2Report>,
    /// The one-line summary.
    pub summary: String,
}

/// The supervisor.
pub struct Supervisor {
    /// The modulation the graph is built for.
    pub modulation: Modulation,
    fs: f64,
    tune: Tune,
    ticks: u64,
    phase_since: u64,
    pending: Vec<super::Candidate>,
    current: Option<super::Candidate>,
    bad: Vec<(f64, bool, u64)>,
    lost_ticks: u64,
    follow_ref: f64,
    follow_misses: u32,
    follow_last: Option<f64>,
    snapshot: AtvDetail,
    /// Log lines and reports for the status component, sent at the end of the pass.
    notes: Vec<StatusItem>,
    /// The value of [`CHANNELS`].
    channels_text: String,
    /// The channels of [`CHANNELS`].
    channels: Vec<Channel>,
    /// Requests to the host, sent at the next pass.
    requests: Vec<HostRequest>,
    // Settings.
    system: SystemChoice,
    forced: Option<SystemId>,
    detector: DetectorChoice,
    envelope_only: bool,
    width: ChannelWidth,
    channel_width: f64,
    sound_choice: SoundChoice,
    sound_carrier_only: bool,
    colour_choice: ColourChoice,
    /// SECAM chosen: on M and N it has no sound carrier and the 5.5 MHz plan.
    colour_secam: bool,
    // Last reports.
    candidates: Option<Candidates>,
    monitor: Option<Monitor>,
    vision: Option<VisionReport>,
    sync: Option<SyncReport>,
    picture: Option<PictureReport>,
    sound: Option<SoundReport>,
    nicam: Option<NicamReport>,
    a2: Option<A2Report>,
    selector: Option<SelectorReport>,
    nbtv: Option<NbtvReport>,
    was_state: State,
    trial_weak: bool,
    fm_trial_since: u64,
    rate_warned: bool,
    last_mirror: Option<bool>,
    ports: Vec<&'static str>,
}

impl Supervisor {
    /// For a modulation.
    pub fn new(modulation: Modulation) -> Self {
        let ports: Vec<&'static str> = match modulation {
            Modulation::Am => vec![
                "found", "monitor", "vision", "sync", "picture", "sound", "nicam", "a2", "selector",
            ],
            Modulation::FmVideo => vec!["iq", "monitor", "sync", "picture"],
            _ => vec!["iq", "nbtv"],
        };
        Supervisor {
            modulation,
            fs: 1.0,
            tune: Tune::default(),
            ticks: 0,
            phase_since: 0,
            pending: Vec::new(),
            current: None,
            bad: Vec::new(),
            lost_ticks: 0,
            follow_ref: f64::MIN,
            follow_misses: 0,
            follow_last: None,
            snapshot: AtvDetail::default(),
            notes: Vec::new(),
            channels_text: String::new(),
            channels: Vec::new(),
            requests: Vec::new(),
            system: SystemChoice::Auto,
            forced: None,
            detector: DetectorChoice::Auto,
            envelope_only: false,
            width: ChannelWidth::W8,
            channel_width: 8.0,
            sound_choice: SoundChoice::Auto,
            sound_carrier_only: false,
            colour_choice: ColourChoice::Auto,
            colour_secam: false,
            candidates: None,
            monitor: None,
            vision: None,
            sync: None,
            picture: None,
            sound: None,
            nicam: None,
            a2: None,
            selector: None,
            nbtv: None,
            was_state: State::Searching,
            trial_weak: false,
            fm_trial_since: 0,
            rate_warned: false,
            last_mirror: None,
            ports,
        }
    }

    fn port(&self, name: &str) -> Option<usize> {
        self.ports.iter().position(|p| *p == name)
    }

    fn log(&mut self, line: impl Into<String>) {
        self.notes.push(StatusItem::Log(line.into()));
    }

    /// Requests the host to tune to the channel of [`CHANNELS`] labelled `row`.
    fn tune_channel(&mut self, row: &str) -> Result<(), CommandError> {
        let Some(ch) = crate::channels::find(&self.channels, row) else {
            return Err(CommandError::NoSuchRow {
                table: crate::results::table_id(&crate::results::atv_published(), &ATV_CHANNELS),
                key: row.to_owned(),
            });
        };
        self.requests.push(HostRequest::Retune {
            tuned_hz: ch.hz,
            reason: format!("channel {}", ch.label),
        });
        Ok(())
    }

    fn secs(&self) -> f64 {
        self.ticks as f64 * TICK_S
    }

    fn base_tune(&self) -> Tune {
        Tune {
            forced: self.forced.is_some(),
            system: self.forced,
            envelope_only: self.envelope_only,
            ..Tune::default()
        }
    }

    fn go_search(&mut self) {
        let epoch = self.tune.epoch + 1;
        self.tune = Tune {
            epoch,
            phase: Phase::Search,
            ..self.base_tune()
        };
        self.current = None;
        self.pending.clear();
        self.phase_since = self.ticks;
        self.lost_ticks = 0;
    }

    fn start_trial(&mut self, c: super::Candidate) {
        let epoch = self.tune.epoch + 1;
        let sp = c.spacing_hz;
        let sound_am = (sp - 3.5e6).abs() < 0.2e6
            || sp > 8e6
            || self
                .forced
                .map(|s| System::get(s).am_sound)
                .unwrap_or(false);
        let spacing = match self.forced {
            Some(SystemId::M | SystemId::N) if self.colour_secam => 0.0,
            Some(s) if sp == 0.0 => System::get(s).spacing_mhz.abs() * 1e6,
            _ => sp,
        };
        self.tune = Tune {
            epoch,
            phase: Phase::Trial,
            carrier_hz: c.hz,
            mirrored: c.mirrored,
            spike_off: c.centre,
            spacing_hz: spacing,
            dsb_db: c.dsb_db,
            weak: false,
            sound_am,
            ..self.base_tune()
        };
        if self.last_mirror != Some(c.mirrored) {
            if c.mirrored {
                self.log("the spectrum is mirrored (I and Q swapped)");
            } else if self.last_mirror.is_some() {
                self.log("the spectrum is the right way round");
            }
            self.last_mirror = Some(c.mirrored);
        }
        if c.centre {
            self.log(
                "the vision carrier is at the centre of the input (tuned to the vision frequency)",
            );
        }
        self.current = Some(c);
        self.phase_since = self.ticks;
        self.trial_weak = false;
        self.follow_ref = f64::MIN;
        self.follow_misses = 0;
        self.follow_last = None;
    }

    fn next_candidate(&mut self) {
        let now = self.ticks;
        self.bad.retain(|b| b.2 > now);
        while !self.pending.is_empty() {
            let c = self.pending.remove(0);
            let banned = self
                .bad
                .iter()
                .any(|b| b.1 == c.mirrored && (b.0 - c.hz).abs() < 60e3);
            if c.prominence >= 12.0 && !banned {
                self.start_trial(c);
                return;
            }
        }
        self.go_search();
    }

    fn step_am(&mut self) {
        let sync = self.sync.unwrap_or_default();
        match self.tune.phase {
            Phase::Search => {
                if let Some(c) = self.candidates.take() {
                    self.pending = c.list;
                    self.next_candidate();
                }
            }
            Phase::Trial => {
                let t = (self.ticks - self.phase_since) as f64 * TICK_S;
                if sync.state >= 1 {
                    self.tune.phase = Phase::Run;
                    self.phase_since = self.ticks;
                    self.lost_ticks = 0;
                    self.log("locked");
                } else if !self.trial_weak && t > 1.0 {
                    self.trial_weak = true;
                    self.tune.weak = true;
                    self.phase_since = self.ticks;
                    self.log("no lines found, trying again for a weak signal");
                } else if self.trial_weak && t > 1.6 {
                    if let Some(c) = self.current {
                        self.bad
                            .push((c.hz, c.mirrored, self.ticks + (6.0 / TICK_S) as u64));
                    }
                    self.log("no picture signal on that carrier");
                    self.next_candidate();
                }
            }
            Phase::Run => {
                if sync.state == 0 {
                    self.lost_ticks += 1;
                    if self.lost_ticks as f64 * TICK_S > 2.0 {
                        self.log("signal lost");
                        self.go_search();
                        return;
                    }
                } else {
                    self.lost_ticks = 0;
                }
                if sync.positive && !self.tune.sound_am {
                    self.tune.sound_am = true;
                }
                self.carrier_follow();
            }
        }
        if self.tune.phase != Phase::Search {
            // Identification updates (spacing from colour N, system known).
            let sys = self.system_now();
            if let Some(s) = sys {
                if self.tune.system != Some(s) && self.forced.is_none() {
                    self.tune.system = Some(s);
                }
            }
        }
        let sp = self.tune.spacing_hz.abs();
        let known = [5.5e6, 6.0e6, 6.5e6].iter().any(|x| (sp - x).abs() < 0.2e6);
        self.tune.second_sound = self.tune.phase != Phase::Search
            && sync.state >= 1
            && !self.sound_carrier_only
            && known;
        self.tune.snow = self.tune.phase != Phase::Run;
    }

    fn carrier_follow(&mut self) {
        let Some(m) = self.monitor.clone() else {
            return;
        };
        if m.spectra < 6 || !self.ticks.is_multiple_of(20) {
            return;
        }
        if m.level_db > self.follow_ref {
            self.follow_ref = m.level_db;
        } else {
            self.follow_ref -= 0.1;
        }
        if m.prominence < 12.0 || m.level_db < self.follow_ref - 12.0 {
            self.follow_misses += 1;
            if self.follow_misses >= 4 {
                self.log("the vision carrier is gone");
                self.go_search();
            }
            return;
        }
        self.follow_misses = 0;
        let d = m.line_hz - self.tune.carrier_hz;
        if m.prominence >= 20.0 && d.abs() > 1.5e3 {
            if let Some(prev) = self.follow_last {
                if (prev - m.line_hz).abs() < 1e3 {
                    let trim = self.sync.map_or(0.0, |s| s.trim_hz);
                    if (m.line_hz - (self.tune.carrier_hz + trim)).abs() > 1.5e3 {
                        self.log(format!("vision carrier moved by {:+.1} kHz", d / 1e3));
                        self.tune.carrier_hz = m.line_hz;
                    }
                }
            }
            self.follow_last = Some(m.line_hz);
        } else {
            self.follow_last = None;
        }
    }

    fn step_fm(&mut self) {
        let sync = self.sync.unwrap_or_default();
        if !self.rate_warned && self.fs < 16e6 {
            self.rate_warned = true;
            self.log(
                "FM video: a link is about 17 MHz wide, this input rate is too low (use 20 Msps)",
            );
        }
        if self.tune.phase == Phase::Search {
            self.tune.phase = Phase::Trial;
            self.fm_trial_since = self.ticks;
        }
        match self.tune.phase {
            Phase::Trial => {
                if sync.state >= 1 {
                    self.tune.phase = Phase::Run;
                    self.lost_ticks = 0;
                    let pol = if self.tune.inverted {
                        "highest"
                    } else {
                        "lowest"
                    };
                    self.log(format!(
                        "FM video: locked, the sync tip is the {pol} frequency"
                    ));
                } else if (self.ticks - self.fm_trial_since) as f64 * TICK_S > 1.2 {
                    self.tune.inverted = !self.tune.inverted;
                    self.fm_trial_since = self.ticks;
                    let pol = if self.tune.inverted {
                        "highest"
                    } else {
                        "lowest"
                    };
                    self.log(format!(
                        "no lines, trying the sync tip at the {pol} frequency"
                    ));
                }
            }
            Phase::Run => {
                if sync.state == 0 {
                    self.lost_ticks += 1;
                    if self.lost_ticks as f64 * TICK_S > 2.0 {
                        self.log("FM video: signal lost");
                        self.tune.phase = Phase::Trial;
                        self.fm_trial_since = self.ticks;
                    }
                } else {
                    self.lost_ticks = 0;
                }
            }
            Phase::Search => {}
        }
        self.tune.snow = self.tune.phase != Phase::Run;
    }

    /// The system as identified now (forced first).
    fn system_now(&self) -> Option<SystemId> {
        if let Some(s) = self.forced {
            return Some(s);
        }
        let sync = self.sync?;
        if sync.lines == 0 {
            return None;
        }
        let colour = self.picture.as_ref().and_then(|p| p.colour);
        let mut sp = self.tune.spacing_hz / 1e6;
        if colour.is_some_and(|c| c.is_n()) && sync.lines == 625 {
            sp = 4.5;
        }
        let s = identify(sync.lines, sync.positive, sp, self.tune.mirrored);
        // A 7 MHz system B: the name does not depend on the channel width.
        Some(
            if s == SystemId::G && (self.channel_width - 7.0).abs() < 0.5 {
                SystemId::B
            } else {
                s
            },
        )
    }

    /// The vision carrier's nominal place from the centre, Hz.
    fn nominal_place(&self, sys: Option<SystemId>) -> f64 {
        // A system the user chose has its own layout.
        if let Some(s) = self.forced {
            return System::get(s).vision_from_centre_mhz() * 1e6;
        }
        if let Some(s) = sys {
            if matches!(
                s,
                SystemId::A
                    | SystemId::C
                    | SystemId::E
                    | SystemId::H
                    | SystemId::Kp
                    | SystemId::L
                    | SystemId::Lp
            ) {
                return System::get(s).vision_from_centre_mhz() * 1e6;
            }
        }
        let sp = self.tune.spacing_hz.abs() / 1e6;
        let lines = self.sync.map_or(0, |s| s.lines);
        if lines == 525 || (sp - 4.5).abs() < 0.2 {
            -1.75e6
        } else if (sp - 6.0).abs() < 0.2 || (sp - 6.5).abs() < 0.2 {
            -2.75e6
        } else {
            -(self.channel_width / 2.0 - 1.25) * 1e6
        }
    }

    fn state(&self) -> u8 {
        let s = self.sync.map_or(0, |s| s.state);
        match self.modulation {
            Modulation::Am => {
                if self.tune.phase == Phase::Search {
                    0
                } else {
                    s
                }
            }
            Modulation::FmVideo => s,
            _ => {
                let n = self.nbtv.as_ref();
                if n.is_some_and(|n| n.picture_locked) {
                    2
                } else if n.is_some_and(|n| n.line_locked) {
                    1
                } else {
                    0
                }
            }
        }
    }

    /// Makes the snapshot of this report for the status component, and sends the state event when the state changed.
    fn report(&mut self, io: &mut Io) {
        let mut out = AtvReport::default();
        let state = State::of(self.state());
        let sys = match self.modulation {
            Modulation::Am | Modulation::FmVideo => self.system_now_for(self.modulation),
            _ => None,
        };
        let place = self.nominal_place(sys);
        let d = &mut self.snapshot;
        d.state = state;
        d.modulation = self.modulation;
        let sync = self.sync.unwrap_or_default();
        let empty = PictureReport::default();
        let pic = self.picture.as_ref().unwrap_or(&empty);
        let colour = pic.colour;
        if self.modulation == Modulation::NbtvAm || self.modulation == Modulation::NbtvFm {
            let empty = NbtvReport::default();
            let n = self.nbtv.as_ref().unwrap_or(&empty);
            d.system.clone_from(&n.name);
            d.colour_system = Some(Colour::Mono);
            d.colour = false;
            d.lines = 0;
            d.line_hz = n.line_hz;
            d.line_error_ppm = n.line_ppm;
            d.sync_quality = n.quality;
            d.video_snr_db = n.snr_db;
            d.carrier_dbfs = n.level_dbfs;
            d.pictures = n.pictures;
            d.line_count = n.lines;
            d.nbtv_line_locked = n.line_locked;
            d.nbtv_picture_locked = n.picture_locked;
            d.sound_system = SoundPlaying::Picture;
            d.detector = if self.modulation == Modulation::NbtvAm {
                DetectorUsed::Envelope
            } else {
                DetectorUsed::FmDiscriminator
            };
            out.log.extend(n.log.iter().cloned());
            out.line = Arc::from(&n.line[..]);
        } else {
            d.system.clear();
            if let (Some(s), Some(c), true) = (sys, colour, state != State::Searching) {
                d.system = System::get(s).name(c);
            }
            d.colour_system = colour;
            d.colour = pic.colour_on;
            d.colour_killer = pic.killer;
            d.lines = sync.lines;
            d.line_hz = sync.line_hz;
            let nominal = if sync.lines == 525 {
                4.5e6 / 286.0
            } else {
                sys.map_or(15_625.0, |s| System::get(s).raster.line_hz)
            };
            d.line_error_ppm = if sync.lines > 0 {
                (sync.line_hz / nominal - 1.0) * 1e6
            } else {
                0.0
            };
            d.field_hz = if sync.lines > 0 {
                2.0 * sync.line_hz / f64::from(sync.lines)
            } else {
                0.0
            };
            d.detector = if self.modulation == Modulation::FmVideo {
                DetectorUsed::FmDiscriminator
            } else if sync.synchronous {
                DetectorUsed::Synchronous
            } else {
                DetectorUsed::Envelope
            };
            d.sync_quality = sync.quality;
            d.line_count = sync.line_count;
            d.field_count = sync.fields;
            d.last_field = sync.last_field;
            d.pictures = pic.pictures;
            d.snow_pictures = pic.snow_pictures;
            d.fields_ok = pic.fields_ok;
            d.fields_bad = pic.fields_bad;
            d.white_peak = pic.white_peak;
            d.burst_level = pic.burst;
            d.burst_phase_error_deg = pic.burst_phase_deg;
            d.chroma_level = pic.chroma_level;
            d.secam_identification = pic.secam_how;
            d.sync_compression_pct = pic.compression_pct;
            let vis = self.vision.unwrap_or_default();
            let sigma = if sync.gain > 0.0 {
                sync.noise_var.sqrt() / sync.gain
            } else {
                1.0
            };
            d.video_snr_db = (20.0 * (1.0 / sigma.max(1e-9)).log10()).clamp(0.0, 60.0);
            if self.modulation == Modulation::Am {
                let white = sync.blank + sync.gain;
                let peak = if sync.positive { white } else { sync.tip } / sync.dc_gain.max(1e-9);
                d.carrier_dbfs = 20.0 * peak.abs().max(1e-12).log10();
                let ns = vis.noise_gain * sync.vsb_noise_gain * vis.input_hz / 5e6 / 2.0;
                let cn = 10.0 * (peak * peak * ns / sync.noise_var.max(1e-30)).log10();
                d.carrier_to_noise_db = cn.clamp(-10.0, 70.0);
                d.sync_depth_pct = if sync.positive {
                    100.0 * (sync.blank - sync.tip) / white.max(1e-12)
                } else {
                    100.0 * (sync.tip - sync.blank) / sync.tip.max(1e-12)
                };
                // Carrier positions, back in the input's own orientation.
                let mir = if self.tune.mirrored { -1.0 } else { 1.0 };
                let vision = (self.tune.carrier_hz + sync.trim_hz) * mir;
                d.vision_hz = vision;
                let snd = self.sound.unwrap_or_default();
                let spacing = self.tune.spacing_hz;
                d.sound_spacing_mhz = if sys.is_some_and(|s| System::get(s).turned_round()) {
                    -spacing / 1e6
                } else {
                    spacing / 1e6
                };
                d.sound_hz = if spacing != 0.0 {
                    (self.tune.carrier_hz + spacing + snd.trim_hz) * mir
                } else {
                    0.0
                };
                d.carrier_offset_hz = if state != State::Searching {
                    vision - place
                } else {
                    0.0
                };
                d.sound_present = snd.present;
                d.sound_deviation_khz = snd.deviation_khz;
                d.sound_level_dbfs = snd.level_dbfs;
                d.sound_system = if self.tune.spacing_hz == 0.0 {
                    SoundPlaying::NoneFound
                } else {
                    self.selector
                        .as_ref()
                        .map_or(SoundPlaying::Fm, |s| s.system)
                };
                d.nicam.clone_from(&self.nicam);
                d.a2.clone_from(&self.a2);
            } else {
                d.sound_system = SoundPlaying::NotDecoded;
                d.fm_polarity = Some(if state == State::Searching {
                    FmPolarity::Trying
                } else if self.tune.inverted {
                    FmPolarity::TipHigh
                } else {
                    FmPolarity::TipLow
                });
                d.carrier_to_noise_db = 0.0;
            }
            out.log.extend(pic.log.iter().cloned());
            out.line = Arc::clone(&pic.line);
            out.vbi = Arc::clone(&pic.vbi);
        }
        if let Some(n) = &self.nicam {
            out.log.extend(n.log.iter().cloned());
        }
        if let Some(m) = &self.monitor {
            out.spectrum = Arc::from(&m.spectrum[..]);
        }
        let d = &self.snapshot;
        out.detail.clone_from(d);
        self.notes.push(StatusItem::Report(Box::new(out)));
        if state != self.was_state {
            let t = self.secs();
            io.messages_out(1).push(ATV_STATE.event(
                t,
                0.0,
                &AtvStateEvent {
                    state,
                    text: d.system.as_str(),
                    family: Family::Receiver,
                },
            ));
            self.was_state = state;
        }
    }

    fn system_now_for(&self, m: Modulation) -> Option<SystemId> {
        match m {
            Modulation::FmVideo => {
                let lines = self.sync.map_or(0, |s| s.lines);
                match (self.forced, lines) {
                    (Some(s), _) => Some(s),
                    (None, 525) => Some(SystemId::M),
                    (None, 625) => Some(SystemId::G),
                    _ => None,
                }
            }
            _ => self.system_now(),
        }
    }
}

/// The one-line summary of a snapshot.
pub fn summary_line(d: &AtvDetail) -> String {
    let locked = |b: bool| if b { "locked" } else { "not locked" };
    match d.modulation {
        Modulation::NbtvAm | Modulation::NbtvFm => format!(
            "Analog TV: {}  line {}  picture {}",
            d.system,
            locked(d.nbtv_line_locked),
            locked(d.nbtv_picture_locked)
        ),
        _ if d.system.is_empty() => {
            if d.state == State::Searching {
                "Analog TV: no signal".into()
            } else {
                "Analog TV: carrier found".into()
            }
        }
        _ => format!(
            "Analog TV: {}  video SNR {:.1} dB  sync {:.0}%  burst {:.0}%  sound {}  fields ok {} bad {}{}",
            d.system,
            d.video_snr_db,
            100.0 * d.sync_quality,
            100.0 * d.burst_level,
            if d.sound_present { "yes" } else { "no" },
            d.fields_ok,
            d.fields_bad,
            if d.state != State::Locked { " (not locked)" } else { "" }
        ),
    }
}

impl Supervisor {
    /// Forces system `c`, or finds the system from the signal for [`SystemChoice::Auto`].
    pub fn set_system(&mut self, c: SystemChoice) {
        self.system = c;
        self.forced = c.system();
        self.tune.forced = self.forced.is_some();
        self.tune.system = self.forced;
    }
}

impl Component for Supervisor {
    fn info(&self) -> ComponentInfo {
        let inputs = self
            .ports
            .iter()
            .map(|p| match *p {
                "iq" => Port::iq("iq"),
                "found" => Port::typed::<Candidates>("found"),
                "monitor" => Port::typed::<Monitor>("monitor"),
                "vision" => Port::typed::<VisionReport>("vision"),
                "sync" => Port::typed::<SyncReport>("sync"),
                "picture" => Port::typed::<PictureReport>("picture"),
                "sound" => Port::typed::<SoundReport>("sound"),
                "nicam" => Port::typed::<NicamReport>("nicam"),
                "a2" => Port::typed::<A2Report>("a2"),
                "selector" => Port::typed::<SelectorReport>("selector"),
                _ => Port::typed::<NbtvReport>("nbtv"),
            })
            .collect();
        ComponentInfo::processor(
            "atv.supervisor",
            inputs,
            vec![
                Port::typed::<Tune>("tune").rate_from(RateFrom::Fixed(0.0)),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<StatusItem>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(SupervisorSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(i) = self.port("iq") {
            self.fs = inputs[i].rate;
        }
        self.rate_warned = false;
        self.ticks = 0;
        self.tune.epoch += 1;
        let epoch = self.tune.epoch;
        self.tune = Tune {
            epoch,
            ..self.base_tune()
        };
        if self.modulation != Modulation::Am {
            self.tune.phase = Phase::Trial;
            self.fm_trial_since = 0;
        }
        self.pending.clear();
        self.current = None;
        self.bad.clear();
        self.snapshot = AtvDetail::default();
        self.notes.clear();
        self.candidates = None;
        self.monitor = None;
        self.sync = None;
        self.picture = None;
        self.sound = None;
        self.nicam = None;
        self.a2 = None;
        self.selector = None;
        self.nbtv = None;
        self.was_state = State::Searching;
        Ok(vec![
            Format::new(Kind::typed::<Tune>(), 0.0),
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
            Format::new(Kind::typed::<StatusItem>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(2).push(Timed {
                time: self.ticks as f64 * TICK_S,
                value: r,
            });
        }
        macro_rules! take {
            ($name:expr, $t:ty, $field:ident) => {
                if let Some(i) = self.port($name) {
                    if let Some(v) = last::<$t>(io, i) {
                        self.$field = Some(v);
                    }
                }
            };
        }
        take!("found", Candidates, candidates);
        take!("monitor", Monitor, monitor);
        take!("vision", VisionReport, vision);
        take!("sync", SyncReport, sync);
        take!("sound", SoundReport, sound);
        take!("selector", SelectorReport, selector);
        take!("a2", A2Report, a2);
        take!("nbtv", NbtvReport, nbtv);
        if let Some(i) = self.port("picture") {
            for p in io.items::<PictureReport>(i) {
                let log = self
                    .picture
                    .as_mut()
                    .map(|prev| std::mem::take(&mut prev.log));
                let mut v = p.value.clone();
                if let Some(mut log) = log {
                    log.append(&mut v.log);
                    v.log = log;
                }
                self.picture = Some(v);
            }
        }
        if let Some(i) = self.port("nicam") {
            for p in io.items::<NicamReport>(i) {
                let log = self
                    .nicam
                    .as_mut()
                    .map(|prev| std::mem::take(&mut prev.log));
                let mut v = p.value.clone();
                if let Some(mut log) = log {
                    log.append(&mut v.log);
                    v.log = log;
                }
                self.nicam = Some(v);
            }
        }
        if !io.tick() {
            return;
        }
        if let Some(v) = &self.vision {
            self.fs = v.input_hz;
        }
        self.ticks += 1;
        match self.modulation {
            Modulation::Am => self.step_am(),
            Modulation::FmVideo => self.step_fm(),
            _ => {}
        }
        let t = self.secs();
        io.items_out::<Tune>(0).push(Timed {
            time: t,
            value: self.tune,
        });
        if self.ticks.is_multiple_of(u64::from(REPORT_TICKS)) {
            self.report(io);
            if let Some(p) = &mut self.picture {
                p.log.clear();
            }
            if let Some(n) = &mut self.nicam {
                n.log.clear();
            }
            if let Some(n) = &mut self.nbtv {
                n.log.clear();
            }
        }
        for value in self.notes.drain(..) {
            io.items_out::<StatusItem>(3).push(Timed { time: t, value });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SupervisorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SupervisorSetting::System => self.set_system(SYSTEM.read(id, v)?),
            SupervisorSetting::Detector => {
                self.detector = DETECTOR.read(id, v)?;
                self.envelope_only = self.detector == DetectorChoice::Envelope;
                self.tune.envelope_only = self.envelope_only;
            }
            SupervisorSetting::ChannelWidth => {
                self.width = CHANNEL_WIDTH.read(id, v)?;
                self.channel_width = self.width.mhz();
            }
            SupervisorSetting::Channels => {
                let text = CHANNELS.read(id, v)?;
                self.channels = crate::channels::read(id, text, ATV_CHANNELS_CAP)?;
                self.channels_text = text.to_owned();
            }
            SupervisorSetting::SoundSystem => {
                self.sound_choice = SOUND_SYSTEM.read(id, v)?;
                self.sound_carrier_only = self.sound_choice == SoundChoice::Carrier;
            }
            SupervisorSetting::Colour => {
                self.colour_choice = COLOUR.read(id, v)?;
                self.colour_secam = self.colour_choice == ColourChoice::Secam;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                SupervisorSetting::System,
                Value::Choice(self.system.index()),
            ),
            (
                SupervisorSetting::Detector,
                Value::Choice(self.detector.index()),
            ),
            (
                SupervisorSetting::ChannelWidth,
                Value::Choice(self.width.index()),
            ),
            (
                SupervisorSetting::Channels,
                Value::Text(self.channels_text.clone()),
            ),
            (
                SupervisorSetting::SoundSystem,
                Value::Choice(self.sound_choice.index()),
            ),
            (
                SupervisorSetting::Colour,
                Value::Choice(self.colour_choice.index()),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        SupervisorCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SupervisorCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SupervisorCommand::ChannelTune => {
                let AtvChannelArgs { row } = ATV_CHANNEL_TUNE.args(args)?;
                self.tune_channel(row)?;
            }
        }
        Ok(None)
    }
}
