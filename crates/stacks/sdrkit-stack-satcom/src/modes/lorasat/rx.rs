//! The LoRa satellite receiver: one channel at the dial, LoRa detectors for each spreading factor, bandwidth and sync hypothesis,
//! and a second detector that follows the carrier through each frame.

use super::{doppler_limits, nearest, LoraBw, Preset, Sf, MIN_RATE};
use crate::stage_error;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Schema, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Item, Kind, Outbox, Port, Timed};
use sdrkit_proto_lora_phy::channel::in_band;
use sdrkit_proto_lora_phy::{ChannelRx, DetectorConfig, DetectorStats, LoraFrame, LoraSymbols};
use std::sync::Arc;

/// The bit of `LoraFrame::tag` that marks a frame from a detector with Doppler tracking.
const TRACKED: u32 = 0x100;

/// A frame as the receiver passes it on.
#[derive(Clone, Debug)]
pub struct LsFrame {
    /// The frame, the better of the copies the detectors heard.
    pub frame: LoraFrame,
    /// The carrier rate the Doppler-tracking detector measured over this frame, Hz/s, when it heard the frame.
    pub tracked_rate_hz_s: Option<f64>,
    /// How long the frame lasted, seconds.
    pub duration_s: f64,
    /// Where the user tuned, from the input centre, Hz.
    pub dial_hz: f64,
    /// The input centre, Hz.
    pub centre_hz: f64,
}

/// One detector's counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemodRow {
    /// The spreading factor.
    pub sf: u8,
    /// The bandwidth, Hz.
    pub bw_hz: f64,
    /// The sync words it accepts.
    pub syncs: Vec<u8>,
    /// Its counters.
    pub stats: DetectorStats,
}

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum LsItem {
    /// A decoded frame.
    Frame(Box<LsFrame>),
    /// The state of the receiver at a clock tick.
    Status {
        /// The detectors without Doppler tracking (one row for each hypothesis).
        demods: Vec<DemodRow>,
        /// The preset that runs, if the dial follows one.
        preset: Option<&'static Preset>,
        /// The Doppler allowance: the largest shift (Hz) and rate (Hz/s).
        allowance: (f64, f64),
    },
}

impl Item for LsItem {
    const KIND: &'static str = "satcom.lora_sat_item";
}

/// The receiver's settings.
#[derive(Clone, Debug, PartialEq)]
struct RxSet {
    follow_dial: bool,
    auto_sf: bool,
    sf: Sf,
    auto_bw: bool,
    bw: LoraBw,
    sync: Sync,
    sync_other: u8,
    preamble: u16,
    doppler_max: f64,
    track: bool,
    offset: f64,
}

sdrkit_core::choice_enum! {
    /// The sync words the receiver tries.
    pub enum Sync {
        /// 0x12 and 0x34.
        Both = ("both", "Try 0x12 and 0x34"),
        /// 0x12, the private networks' word.
        Private = ("0x12", "0x12"),
        /// 0x34, the public networks' word.
        Public = ("0x34", "0x34"),
        /// The word of `sync_other`.
        Other = ("other", "Other"),
    }
}

/// Run the SF, bandwidth and sync of a preset within 15 kHz of the dial.
pub const FOLLOW_DIAL: Setting<field::Bool> =
    Setting::new("follow_dial", "Follow the dial", "", true)
        .help("When the dial is within 15 kHz of a preset, run its SF, bandwidth and sync.");
/// Search SF7 to SF12.
pub const AUTO_SF: Setting<field::Bool> = Setting::new("auto_sf", "Search SF", "", true)
    .help("SF7 to SF12 (a preset's SF overrides while following the dial).");
/// The spreading factor when it is not searched.
pub const SF: Setting<field::Choice<Sf>> = Setting::choice("sf", "SF", Sf::Sf11);
/// Search 125, 250 and 500 kHz.
pub const AUTO_BW: Setting<field::Bool> =
    Setting::new("auto_bw", "Search bandwidth", "", false).help("125, 250 and 500 kHz.");
/// The bandwidth when it is not searched.
pub const BW_KHZ: Setting<field::Choice<LoraBw>> =
    Setting::choice("bw_khz", "Bandwidth, kHz", LoraBw::B125k);
/// The sync words tried.
pub const SYNC: Setting<field::Choice<Sync>> = Setting::choice("sync", "Sync word", Sync::Both);
/// The other sync word.
pub const SYNC_OTHER: Setting<field::I64> =
    Setting::new("sync_other", "Other sync word", "", 18).range(0.0, 255.0, 1.0);
/// The preamble length.
pub const PREAMBLE: Setting<field::I64> =
    Setting::new("preamble", "Preamble", "symbols", 8).range(6.0, 65535.0, 1.0);
/// The Doppler allowance (0: from the dial).
pub const DOPPLER_MAX_HZ: Setting<field::Hz> =
    Setting::new("doppler_max_hz", "Doppler allowance", "Hz", 0.0)
        .range(0.0, 60_000.0, 100.0)
        .help("0: from the dial.");
/// Follow the carrier through each frame.
pub const TRACK: Setting<field::Bool> = Setting::new("track", "Track the carrier", "", true)
    .help("Follow the carrier through each frame.");
/// The channel offset from the dial.
pub const OFFSET_HZ: Setting<field::Hz> =
    Setting::new("offset_hz", "Channel offset", "Hz", 0.0).range(-500e3, 500e3, 1.0);

sdrkit_core::settings! {
    /// The settings of [`LoraSatRx`].
    pub enum LoraSatRxSetting {
        /// Follow the dial.
        FollowDial = FOLLOW_DIAL,
        /// Search SF.
        AutoSf = AUTO_SF,
        /// SF.
        Sf = SF,
        /// Search bandwidth.
        AutoBw = AUTO_BW,
        /// Bandwidth.
        BwKhz = BW_KHZ,
        /// Sync word.
        Sync = SYNC,
        /// Other sync word.
        SyncOther = SYNC_OTHER,
        /// Preamble.
        Preamble = PREAMBLE,
        /// Doppler allowance.
        DopplerMaxHz = DOPPLER_MAX_HZ,
        /// Track the carrier.
        Track = TRACK,
        /// Channel offset.
        OffsetHz = OFFSET_HZ,
    }
}

impl Default for RxSet {
    fn default() -> Self {
        RxSet {
            follow_dial: true,
            auto_sf: true,
            sf: Sf::Sf11,
            auto_bw: false,
            bw: LoraBw::B125k,
            sync: Sync::Both,
            sync_other: 0x12,
            preamble: 8,
            doppler_max: 0.0,
            track: true,
            offset: 0.0,
        }
    }
}

/// The receiver decoder (`satcom.lora_sat_receiver`).
#[derive(Default)]
pub struct LoraSatRx {
    set: RxSet,
    rate: f64,
    centre: f64,
    dial: f64,
    tuned: f64,
    chans: Vec<ChannelRx>,
    plan: Vec<(f64, f64, Vec<DetectorConfig>)>,
    found: Vec<(LoraFrame, LoraSymbols)>,
    pending: Vec<LsFrame>,
    preset: Option<&'static Preset>,
    /// The index of the next input sample.
    next: u64,
}

impl LoraSatRx {
    /// The channels and detectors the settings ask for: (channel offset, bandwidth, detectors).
    fn plan(&self) -> Vec<(f64, f64, Vec<DetectorConfig>)> {
        let s = &self.set;
        let preset = if s.follow_dial {
            nearest(self.tuned)
        } else {
            None
        };
        let syncs_set: Vec<u8> = match s.sync {
            Sync::Private => vec![0x12],
            Sync::Public => vec![0x34],
            Sync::Other => vec![s.sync_other],
            Sync::Both => vec![0x12, 0x34],
        };
        let (sfs, bws, syncs): (Vec<u8>, Vec<f64>, Vec<u8>) = match preset {
            Some(p) => (
                if p.sf == 0 {
                    (7..=12).collect()
                } else {
                    vec![p.sf]
                },
                vec![p.bw_khz * 1e3],
                if p.syncs.is_empty() {
                    vec![0x12, 0x34]
                } else {
                    p.syncs.to_vec()
                },
            ),
            None => (
                if s.auto_sf {
                    (7..=12).collect()
                } else {
                    vec![s.sf.value()]
                },
                if s.auto_bw {
                    vec![125e3, 250e3, 500e3]
                } else {
                    vec![s.bw.hz()]
                },
                syncs_set,
            ),
        };
        let preamble = preset.map_or(s.preamble, |p| p.preamble.unwrap_or(s.preamble));
        // Per SF a detector whose timing loop follows the carrier, and with tracking on a second one that follows it in bins.
        let modes: &[bool] = if s.track { &[false, true] } else { &[false] };
        let mut plan = Vec::new();
        for (bi, &bw) in bws.iter().enumerate() {
            let offset = self.dial + s.offset;
            if !in_band(self.rate, offset, bw) {
                continue;
            }
            let mut dets = Vec::new();
            for &sf in &sfs {
                for &dop in modes {
                    let mut c = DetectorConfig::new(sf, bw, syncs[0]);
                    c.syncs = syncs.clone();
                    c.preamble = preamble;
                    c.doppler = dop;
                    c.carrier_hz = self.tuned;
                    c.jump_search = false;
                    c.tag = (bi * 16 + sf as usize) as u32 | if dop { TRACKED } else { 0 };
                    dets.push(c);
                }
            }
            plan.push((offset, bw, dets));
        }
        plan
    }

    /// Builds the channels of the plan. A plan equal to the running one keeps the channels and resets them, so the detector
    /// counters stay.
    fn build(&mut self) -> Result<(), ConfigError> {
        let plan = self.plan();
        self.preset = if self.set.follow_dial {
            nearest(self.tuned)
        } else {
            None
        };
        self.pending.clear();
        if plan == self.plan && self.chans.len() == plan.len() {
            for c in &mut self.chans {
                c.reset().map_err(stage_error)?;
            }
            return Ok(());
        }
        let mut chans = Vec::with_capacity(plan.len());
        for (offset, bw, dets) in &plan {
            chans.push(ChannelRx::new(self.rate, *offset, *bw, dets.clone()).map_err(stage_error)?);
        }
        self.chans = chans;
        self.plan = plan;
        Ok(())
    }

    /// The Doppler allowance: the setting, or the limits of a 600 km orbit at the dial.
    pub fn allowance(&self) -> (f64, f64) {
        if self.set.doppler_max > 0.0 {
            (self.set.doppler_max, self.set.doppler_max / 60.0)
        } else {
            doppler_limits(self.tuned.max(1.0))
        }
    }

    fn set_one(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let s = &mut self.set;
        let bad = |_| SettingError::NotPermitted { id };
        match LoraSatRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LoraSatRxSetting::FollowDial => s.follow_dial = FOLLOW_DIAL.read(id, v)?,
            LoraSatRxSetting::AutoSf => s.auto_sf = AUTO_SF.read(id, v)?,
            LoraSatRxSetting::Sf => s.sf = SF.read(id, v)?,
            LoraSatRxSetting::AutoBw => s.auto_bw = AUTO_BW.read(id, v)?,
            LoraSatRxSetting::BwKhz => s.bw = BW_KHZ.read(id, v)?,
            LoraSatRxSetting::Sync => s.sync = SYNC.read(id, v)?,
            LoraSatRxSetting::SyncOther => {
                s.sync_other = u8::try_from(SYNC_OTHER.read(id, v)?).map_err(bad)?
            }
            LoraSatRxSetting::Preamble => {
                s.preamble = u16::try_from(PREAMBLE.read(id, v)?).map_err(bad)?
            }
            LoraSatRxSetting::DopplerMaxHz => s.doppler_max = DOPPLER_MAX_HZ.read(id, v)?,
            LoraSatRxSetting::Track => s.track = TRACK.read(id, v)?,
            LoraSatRxSetting::OffsetHz => s.offset = OFFSET_HZ.read(id, v)?,
        }
        Ok(())
    }
}

impl Decoder for LoraSatRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.lora_sat_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<LsItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        LoraSatRxSetting::schema()
    }
    fn values(&self) -> Values {
        let s = &self.set;
        values_of([
            (LoraSatRxSetting::FollowDial, Value::Bool(s.follow_dial)),
            (LoraSatRxSetting::AutoSf, Value::Bool(s.auto_sf)),
            (LoraSatRxSetting::Sf, Value::Choice(s.sf.index())),
            (LoraSatRxSetting::AutoBw, Value::Bool(s.auto_bw)),
            (LoraSatRxSetting::BwKhz, Value::Choice(s.bw.index())),
            (LoraSatRxSetting::Sync, Value::Choice(s.sync.index())),
            (
                LoraSatRxSetting::SyncOther,
                Value::I64(i64::from(s.sync_other)),
            ),
            (
                LoraSatRxSetting::Preamble,
                Value::I64(i64::from(s.preamble)),
            ),
            (LoraSatRxSetting::DopplerMaxHz, Value::Hz(s.doppler_max)),
            (LoraSatRxSetting::Track, Value::Bool(s.track)),
            (LoraSatRxSetting::OffsetHz, Value::Hz(s.offset)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let before = self.set.clone();
        self.set_one(id, v)?;
        if self.rate > 0.0 {
            if let Err(e) = self.build() {
                self.set = before;
                return Err(SettingError::Config {
                    id,
                    source: Box::new(e),
                });
            }
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.dial = f.signal_offset_hz();
        self.tuned = f.tuned_hz;
        self.next = 0;
        self.plan.clear();
        self.build()?;
        Ok(vec![f.with_kind(Kind::typed::<LsItem>())])
    }
    fn retune(&mut self, f: &Format) {
        self.centre = f.centre_hz;
        self.dial = f.signal_offset_hz();
        self.tuned = f.tuned_hz;
        if self.build().is_err() {
            self.chans.clear();
            self.plan.clear();
        }
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        let rate = self.rate;
        for c in &mut self.chans {
            self.found.clear();
            c.process(iq, &mut self.found);
            let centre = c.offset_hz;
            for (mut f, _) in self.found.drain(..) {
                f.channel_hz = centre;
                merge(&mut self.pending, f, rate, self.dial, self.centre);
            }
        }
        // A frame leaves once every detector had time to finish it (two symbols after its end).
        let now = first + iq.len() as u64;
        self.next = now;
        let items = out.items::<LsItem>(0);
        let mut i = 0;
        while i < self.pending.len() {
            let f = &self.pending[i].frame;
            let sym = (1u64 << f.sf) as f64 * rate / f.bw;
            if now as f64 > f.end as f64 + 2.0 * sym {
                let f = self.pending.remove(i);
                items.push(Timed {
                    time: f.frame.start as f64 / rate,
                    value: LsItem::Frame(Box::new(f)),
                });
            } else {
                i += 1;
            }
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        let demods = self
            .chans
            .iter()
            .flat_map(|c| {
                c.detectors()
                    .iter()
                    .filter(|d| !d.config().doppler)
                    .map(|d| DemodRow {
                        sf: d.config().sf,
                        bw_hz: d.config().bw,
                        syncs: d.config().syncs.clone(),
                        stats: *d.stats(),
                    })
            })
            .collect();
        let time = self.next as f64 / self.rate.max(1.0);
        out.items::<LsItem>(0).push(Timed {
            time,
            value: LsItem::Status {
                demods,
                preset: self.preset,
                allowance: self.allowance(),
            },
        });
    }
}

/// Adds a frame to those waiting. A frame heard by both detectors of its SF keeps the better copy (a good CRC, then fewer
/// corrections), and the carrier rate of the copy with Doppler tracking.
fn merge(pending: &mut Vec<LsFrame>, f: LoraFrame, rate: f64, dial: f64, centre: f64) {
    let sym = (1u64 << f.sf) as f64 * rate / f.bw;
    let tracked = (f.tag & TRACKED != 0).then_some(f.cfo_rate_hz_s);
    match pending
        .iter_mut()
        .find(|p| p.frame.sf == f.sf && (p.frame.start as f64 - f.start as f64).abs() < sym)
    {
        Some(p) => {
            if tracked.is_some() {
                p.tracked_rate_hz_s = tracked;
            }
            if (f.good() && !p.frame.good())
                || (f.good() == p.frame.good() && f.corrected < p.frame.corrected)
            {
                p.duration_s = f.end.saturating_sub(f.start) as f64 / rate;
                p.frame = f;
            }
        }
        None => pending.push(LsFrame {
            duration_s: f.end.saturating_sub(f.start) as f64 / rate,
            tracked_rate_hz_s: tracked,
            frame: f,
            dial_hz: dial,
            centre_hz: centre,
        }),
    }
}
