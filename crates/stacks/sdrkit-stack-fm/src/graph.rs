//! FM as a graph: each FM part as a component with typed ports, the registry of their kinds, and the receiver graph built
//! from them, with the transmit branch: the host's audio (`mic`), the sender (`transmit`), the transmit sink (`tx`) and
//! the mute (`mute`) that silences the receive audio while the sender is keyed. The graph has a clock of 0.1 s: every decision that changes the processing and every report is taken at a tick,
//! so any cut of the input produces the same results.

use crate::audio::{AudioFilters, Deemphasis, Squelch, AUDIO_RATE};
use crate::bands::BandPlan;
use crate::channel::ChannelSelect;
use crate::finder::StationFinder;
use crate::fmdemod::{DemodReport, FmDemod, Mpx};
use crate::frontend;
use crate::measure::{MpxSpectrum, MPX_FFT};
use crate::pty::{Evidence, Variant};
use crate::rds::{self, RdsDecoder, RdsDemod, RdsView};
use crate::results::{self, Figures, Meanings, Shown};
use crate::settings::*;
use crate::stereo::{StereoDecoder, StereoReport};
use crate::typed::{StationEvent, STATION_NAMED, TEXT_DONE};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Command, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, LockState,
    Record, Results, SettingError, SettingId, SettingSet, StackConfig, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Buffer, Catalog, ChannelsFrom, Component, ComponentInfo, Description,
    Format, Graph, GraphBuilder, GraphError, Io, Item, Kind, Port, RateFrom, RateRule, Registry,
    StackGraph, TagKind, TagSource, Timed,
};
use sdrkit_components::tx::TxMute;
use sdrkit_graph::components::{HostIn, TxSink};
use std::num::NonZeroU32;
use std::sync::Arc;

/// The lowest input rate the receiver accepts.
pub const MIN_RATE_HZ: f64 = 500e3;
/// The report interval, seconds of input.
pub const REPORT_S: f64 = 0.1;
/// The stack id.
pub const ID: &str = "fm";

const TWO: NonZeroU32 = NonZeroU32::new(2).expect("2 is not zero");
const FIVE: NonZeroU32 = NonZeroU32::new(5).expect("5 is not zero");

/// Forgets what was decoded on this frequency.
pub const CLEAR: Command =
    Command::new("clear", "Clear").help("Forgets the station's name, text and the other RDS data.");

// The typed items between the components.

/// The station finder's decision: from input sample `at` on, the station is `offset_hz` from where the user tuned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinderMove {
    /// The absolute index of the input sample the move takes effect at.
    pub at: u64,
    /// The finder's offset from the tuned frequency, Hz.
    pub offset_hz: f64,
}
impl Item for FinderMove {
    const KIND: &'static str = "fm.finder_move";
}

/// What the display requests from the station finder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinderCommand {
    /// Search again (after 2 s without a station).
    Start,
}
impl Item for FinderCommand {
    const KIND: &'static str = "fm.finder_command";
}

/// The mark at which the FM components start afresh (after a station finder move).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reset {
    /// The finder's new offset from the tuned frequency, Hz.
    pub offset_hz: f64,
}
impl Item for Reset {
    const KIND: &'static str = "fm.reset";
}

/// Where the channel is and how wide, from input sample `at` on: the link centre and the channel width that a stack decides
/// at a tick. The shift is phase continuous; a new width restarts the channel filter. Nothing else starts afresh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelSteer {
    /// The absolute index of the input sample the steer takes effect at.
    pub at: u64,
    /// The offset of the channel centre from the tuned frequency, Hz.
    pub shift_hz: f64,
    /// The channel width, Hz.
    pub width_hz: f64,
}
impl Item for ChannelSteer {
    const KIND: &'static str = "fm.channel_steer";
}

/// How the channel was made, sent once after each configure: the demodulator needs it for its droop correction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelSetup {
    /// The decimation from the input rate to the channel rate.
    pub decimation: usize,
}
impl Item for ChannelSetup {
    const KIND: &'static str = "fm.channel_setup";
}

/// The channel's report at each tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChannelReport {
    /// The mean channel power since the last report, dBFS.
    pub level_dbfs: f32,
    /// The channel width in use, Hz.
    pub width_hz: f64,
    /// Seconds of input since the last reset.
    pub time_s: f64,
    /// The frequency the user tuned, Hz.
    pub tuned_hz: f64,
    /// The offset of the channel centre from the tuned frequency in use (the station finder's or the steer's), Hz.
    pub shift_hz: f64,
    /// The components started afresh since the last report (a finder move).
    pub reset: bool,
}
impl Item for ChannelReport {
    const KIND: &'static str = "fm.channel_report";
}

impl Item for DemodReport {
    const KIND: &'static str = "fm.demod_report";
}

impl Item for StereoReport {
    const KIND: &'static str = "fm.stereo_report";
}

/// The most points of the multiplex scope.
pub const MAX_SCOPE_POINTS: usize = MPX_FFT / 2 + 1;

/// The MPX spectrum's report at each tick: the SNR measure and the multiplex scope (inline, so that passing it on allocates
/// nothing).
#[derive(Clone, Debug)]
pub struct MpxReport {
    /// The SNR measure, dB (0 without a carrier).
    pub snr_db: f32,
    /// Points used in `points`.
    pub len: usize,
    /// (Hz, dB against a full-deviation sine), 0 to 80 kHz.
    pub points: [[f32; 2]; MAX_SCOPE_POINTS],
}
impl MpxReport {
    /// The scope's points.
    pub fn scope(&self) -> &[[f32; 2]] {
        &self.points[..self.len]
    }
}
impl Item for MpxReport {
    const KIND: &'static str = "fm.mpx_report";
}

/// The squelch's report at each tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SquelchReport {
    /// The squelch is closed.
    pub squelched: bool,
}
impl Item for SquelchReport {
    const KIND: &'static str = "fm.squelch_report";
}

/// What the RDS component has at each tick: the RDS fields, the meanings in force, the events since the last report and the
/// scope.
#[derive(Clone, Debug, Default)]
pub struct RdsSnapshot {
    /// The RDS fields.
    pub view: RdsView,
    /// RBDS meanings apply.
    pub rbds: bool,
    /// The RBDS call letters, or empty.
    pub call_letters: String,
    /// The PS, when it changed since the last report.
    pub ps_changed: Option<String>,
    /// A RadioText was completed since the last report.
    pub rt_completed: bool,
    /// Group monitor lines since the last report.
    pub monitor: Vec<String>,
    /// The symbols of the RDS scope (normalised matched-filter outputs), oldest first.
    pub symbols: Vec<[f32; 2]>,
}

/// The RDS component's report: a shared snapshot, so that passing it on copies nothing. The component updates it in place
/// when no one else holds it.
#[derive(Clone, Debug)]
pub struct RdsReport(pub Arc<RdsSnapshot>);
impl Item for RdsReport {
    const KIND: &'static str = "fm.rds_report";
}

// Helpers.

/// Offsets (from the buffer's first item) of the [`Reset`] tags of `b`, up to `len` items.
fn resets(b: &Buffer, len: usize, out: &mut Vec<usize>) {
    out.clear();
    for t in b.tags() {
        if matches!(&t.kind, TagKind::Typed(x) if x.get::<Reset>().is_some()) {
            out.push(((t.at - b.first()) as usize).min(len));
        }
    }
}

fn last<T: Item + Copy>(io: &Io, port: usize) -> Option<T> {
    io.items::<T>(port).last().map(|t| t.value)
}

/// The SNR measure of the last [`MpxReport`] on `port`.
fn snr(io: &Io, port: usize) -> Option<f32> {
    io.items::<MpxReport>(port).last().map(|t| t.value.snr_db)
}

/// A typed item port, with no rate.
fn item_out<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

// The components.

sdrkit_core::settings! {
    /// The settings of [`FinderComponent`].
    pub enum FinderSetting {
        /// The station finder is on.
        StationFinder = STATION_FINDER,
    }
}

/// The station finder (`fm.finder`): for recordings and the generator, finds a station off the centre. It outputs its
/// decision as a [`FinderMove`] at the exact input sample. A [`FinderCommand::Start`] (from the display) starts it again.
pub struct FinderComponent {
    on: bool,
    rate: f64,
    signal_hz: f64,
    offset_hz: f64,
    finder: Option<StationFinder>,
}

impl Default for FinderComponent {
    fn default() -> Self {
        FinderComponent {
            on: STATION_FINDER.default,
            rate: 0.0,
            signal_hz: 0.0,
            offset_hz: 0.0,
            finder: None,
        }
    }
}

impl FinderComponent {
    fn start(&mut self) {
        self.finder = self.on.then(|| {
            let mut f = StationFinder::new(self.rate);
            f.start();
            f
        });
    }
}

impl Component for FinderComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.finder",
            vec![Port::iq("in"), Port::typed::<FinderCommand>("control")],
            vec![item_out::<FinderMove>("moves")],
        )
        .with_settings(FinderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.signal_hz = inputs[0].signal_offset_hz();
        self.offset_hz = 0.0;
        if self.rate > 0.0 {
            self.start();
        }
        Ok(vec![Format::new(Kind::typed::<FinderMove>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let start = io
            .items::<FinderCommand>(1)
            .iter()
            .any(|c| c.value == FinderCommand::Start);
        if let (true, Some(f)) = (start, &mut self.finder) {
            if !f.active() {
                f.start();
            }
        }
        let x = io.iq(0);
        let first = io.input(0).first();
        let Some(f) = &mut self.finder else { return };
        let mut i = 0;
        while f.active() && i < x.len() {
            let n = f.needed().min(x.len() - i);
            let decided = f.push(&x[i..i + n]);
            i += n;
            if !decided {
                continue;
            }
            let current = self.signal_hz + self.offset_hz;
            if let Some(p) = f.decide(current) {
                self.offset_hz = p - self.signal_hz;
                let mv = FinderMove {
                    at: first + i as u64,
                    offset_hz: self.offset_hz,
                };
                let time = io.input_format(0).time_of(mv.at);
                io.items_out::<FinderMove>(0)
                    .push(Timed { time, value: mv });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FinderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FinderSetting::StationFinder => {
                let on = STATION_FINDER.read(id, v)?;
                if on != self.on {
                    self.on = on;
                    if self.rate > 0.0 {
                        self.start();
                    }
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(FinderSetting::StationFinder, Value::Bool(self.on))])
    }
}

sdrkit_core::settings! {
    /// The settings of [`ChannelComponent`].
    pub enum ChannelSetting {
        /// The channel width.
        ChannelWidth = CHANNEL_WIDTH,
        /// The channel filter.
        ChannelFilter = CHANNEL_FILTER,
    }
}

/// Channel selection (`fm.channel`): the station moved to 0 Hz and the channel filter down to near 500 kHz
/// ([`ChannelSelect`]). At a [`FinderMove`] it moves the station at that sample, starts afresh and marks the position with a
/// [`Reset`] tag. At a [`ChannelSteer`] it moves the channel centre phase continuously and takes the new width, at that
/// sample, without a reset. Both inputs are optional. After each configure it sends a [`ChannelSetup`]. Report:
/// [`ChannelReport`].
pub struct ChannelComponent {
    width_hz: f64,
    filter: ChannelFilterChoice,
    rate: f64,
    signal_hz: f64,
    tuned_hz: f64,
    shift_hz: f64,
    steer_width_hz: Option<f64>,
    chan: Option<ChannelSelect>,
    samples: u64,
    reset: bool,
    announce: bool,
    tags: TagSource<Reset>,
    changes: Vec<(u64, Change)>,
}

/// A change of the channel at an input sample.
#[derive(Clone, Copy, Debug)]
enum Change {
    Move(f64),
    Steer(f64, f64),
}

impl Default for ChannelComponent {
    fn default() -> Self {
        ChannelComponent {
            width_hz: CHANNEL_WIDTH.default,
            filter: CHANNEL_FILTER.default,
            rate: 0.0,
            signal_hz: 0.0,
            tuned_hz: 0.0,
            shift_hz: 0.0,
            steer_width_hz: None,
            chan: None,
            samples: 0,
            reset: false,
            announce: false,
            tags: TagSource::new(),
            changes: Vec::new(),
        }
    }
}

impl ChannelComponent {
    /// The width asked for: the steer's, else the setting's.
    fn width(&self) -> f64 {
        self.steer_width_hz
            .unwrap_or_else(|| self.filter.width_hz(self.width_hz))
    }
    fn build(&mut self) {
        self.chan = Some(ChannelSelect::new(
            self.rate,
            self.signal_hz + self.shift_hz,
            self.width(),
        ));
        self.samples = 0;
    }
}

impl Component for ChannelComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.channel",
            vec![
                Port::iq("in"),
                Port::typed::<FinderMove>("finder").optional("no station finder"),
                Port::typed::<ChannelSteer>("steer").optional("the width settings, at the centre"),
            ],
            vec![
                Port::iq("out"),
                item_out::<ChannelReport>("report"),
                item_out::<ChannelSetup>("setup"),
            ],
        )
        .with_settings(ChannelSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate.is_nan() || f.rate < MIN_RATE_HZ * 0.999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: f.rate,
            });
        }
        (self.rate, self.signal_hz, self.tuned_hz, self.shift_hz) =
            (f.rate, f.signal_offset_hz(), f.tuned_hz, 0.0);
        self.steer_width_hz = None;
        self.reset = false;
        self.announce = true;
        self.build();
        let d = frontend::decimation(f.rate);
        let mut out = Format::iq(f.rate / d as f64, f.tuned_hz, f.tuned_hz);
        out.bandwidth_hz = self.width();
        Ok(vec![
            out,
            Format::new(Kind::typed::<ChannelReport>(), 0.0),
            Format::new(Kind::typed::<ChannelSetup>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.announce) {
            let decimation = frontend::decimation(self.rate);
            io.items_out::<ChannelSetup>(2).push(Timed {
                time: 0.0,
                value: ChannelSetup { decimation },
            });
        }
        let x = io.iq(0);
        let first = io.input(0).first();
        let mut changes = std::mem::take(&mut self.changes);
        changes.clear();
        if io.is_connected(1) {
            changes.extend(
                io.items::<FinderMove>(1)
                    .iter()
                    .map(|m| (m.value.at, Change::Move(m.value.offset_hz))),
            );
        }
        if io.is_connected(2) {
            changes.extend(io.items::<ChannelSteer>(2).iter().map(|s| {
                (
                    s.value.at,
                    Change::Steer(s.value.shift_hz, s.value.width_hz),
                )
            }));
        }
        changes.sort_by_key(|c| c.0);
        let mut done = 0usize;
        for &(at, change) in &changes {
            let at = (at.saturating_sub(first) as usize).clamp(done, x.len());
            if let Some(c) = &mut self.chan {
                c.process(&x[done..at], io.iq_out(0));
            }
            self.samples += (at - done) as u64;
            done = at;
            match change {
                Change::Move(offset_hz) => {
                    self.shift_hz = offset_hz;
                    self.build();
                    self.reset = true;
                    let tag = TagKind::Typed(self.tags.tag(Reset { offset_hz }));
                    let o = io.output(0);
                    let k = o.len();
                    o.tag_here(k, tag);
                }
                Change::Steer(shift_hz, width_hz) => {
                    let width_changed = self.steer_width_hz != Some(width_hz);
                    (self.shift_hz, self.steer_width_hz) = (shift_hz, Some(width_hz));
                    if let Some(c) = &mut self.chan {
                        c.set_shift(self.signal_hz + shift_hz);
                        if width_changed {
                            c.set_width(width_hz);
                        }
                    }
                }
            }
        }
        self.changes = changes;
        if let Some(c) = &mut self.chan {
            c.process(&x[done..], io.iq_out(0));
        }
        self.samples += (x.len() - done) as u64;
        if io.tick() {
            let Some(c) = &mut self.chan else { return };
            let r = ChannelReport {
                level_dbfs: c.take_level_dbfs(),
                width_hz: c.width(),
                time_s: self.samples as f64 / self.rate,
                tuned_hz: self.tuned_hz,
                shift_hz: self.shift_hz,
                reset: std::mem::take(&mut self.reset),
            };
            let time = io.input_format(0).time_of(first);
            io.items_out::<ChannelReport>(1)
                .push(Timed { time, value: r });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let before = self.width();
        match ChannelSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelSetting::ChannelWidth => self.width_hz = CHANNEL_WIDTH.read(id, v)?,
            ChannelSetting::ChannelFilter => self.filter = CHANNEL_FILTER.read(id, v)?,
        }
        let w = self.width();
        if w != before {
            if let Some(c) = &mut self.chan {
                c.set_width(w);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ChannelSetting::ChannelWidth, Value::Hz(self.width_hz)),
            (
                ChannelSetting::ChannelFilter,
                Value::Choice(sdrkit_core::Choice::index(self.filter)),
            ),
        ])
    }
}

/// The FM demodulator (`fm.demod`, [`FmDemod`]): the equaliser, the discriminators, the multiplex at half the channel rate.
/// The SNR measure (fed back from the spectrum) chooses the paths for the next period. Outputs: `sound` and `meas` MPX, and a
/// [`DemodReport`] at each tick.
#[derive(Default)]
pub struct DemodComponent {
    fo: f64,
    decimation: usize,
    demod: Option<FmDemod>,
    mpx: Mpx,
    points: Vec<usize>,
}

impl Component for DemodComponent {
    fn info(&self) -> ComponentInfo {
        let half = RateFrom::Divided { input: 0, by: TWO };
        ComponentInfo::processor(
            "fm.demod",
            vec![
                Port::iq("in"),
                Port::typed::<MpxReport>("snr"),
                Port::typed::<ChannelSetup>("setup"),
            ],
            vec![
                Port::real("sound").rate_from(half),
                Port::real("meas").rate_from(half),
                item_out::<DemodReport>("report"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate.is_nan() || f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        if f.rate < crate::fmdemod::MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: crate::fmdemod::MIN_RATE_HZ,
                got: f.rate,
            });
        }
        self.fo = f.rate;
        self.demod = None;
        let mut mpx = Format::new(Kind::Real, self.fo / 2.0);
        mpx.bandwidth_hz = 112e3;
        (mpx.centre_hz, mpx.tuned_hz) = (f.centre_hz, f.tuned_hz);
        Ok(vec![
            mpx.clone(),
            mpx,
            Format::new(Kind::typed::<DemodReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(s) = last::<ChannelSetup>(io, 2) {
            self.decimation = s.decimation;
            self.demod = FmDemod::new(self.fo, self.decimation).ok();
        }
        let Some(d) = &mut self.demod else { return };
        // The SNR measure of the last tick chooses the paths from here on.
        if let Some(s) = snr(io, 1) {
            d.set_snr(s);
        }
        let x = io.iq(0);
        resets(io.input(0), x.len(), &mut self.points);
        let mut done = 0;
        for k in 0..=self.points.len() {
            let end = self.points.get(k).copied().unwrap_or(x.len());
            self.mpx.clear();
            d.process(&x[done..end], &mut self.mpx);
            io.real_out(0).extend_from_slice(&self.mpx.sound);
            io.real_out(1).extend_from_slice(&self.mpx.meas);
            if k < self.points.len() {
                if let Ok(fresh) = FmDemod::new(self.fo, self.decimation) {
                    *d = fresh;
                }
            }
            done = end;
        }
        if io.tick() {
            let r = d.measure();
            io.items_out::<DemodReport>(2).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

/// The MPX spectrum (`fm.mpx_spectrum`, [`MpxSpectrum`]): at each tick, the SNR measure (0 without a carrier, from the
/// demodulator's report) and the multiplex scope, as an [`MpxReport`].
#[derive(Default)]
pub struct SpectrumComponent {
    rate: f64,
    spectrum: Option<MpxSpectrum>,
    points: Vec<usize>,
}

impl Component for SpectrumComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.mpx_spectrum",
            vec![Port::real("in"), Port::typed::<DemodReport>("demod")],
            vec![item_out::<MpxReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.spectrum = Some(MpxSpectrum::new(self.rate));
        Ok(vec![Format::new(Kind::typed::<MpxReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        resets(io.input(0), x.len(), &mut self.points);
        let rate = self.rate;
        let Some(s) = &mut self.spectrum else { return };
        let mut done = 0;
        for &p in &self.points {
            s.process(&x[done..p]);
            *s = MpxSpectrum::new(rate);
            done = p;
        }
        s.process(&x[done..]);
        if io.tick() {
            let carrier = last::<DemodReport>(io, 1).is_some_and(|d| d.carrier);
            s.update();
            let mut r = MpxReport {
                snr_db: if carrier { s.snr_db() } else { 0.0 },
                len: 0,
                points: [[0.0; 2]; MAX_SCOPE_POINTS],
            };
            r.len = s.points_into(&mut r.points);
            io.items_out::<MpxReport>(0).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

sdrkit_core::settings! {
    /// The settings of [`StereoComponent`].
    pub enum StereoSetting {
        /// Stereo or mono.
        Stereo = STEREO,
        /// Mono whatever the stereo choice.
        ForceMono = FORCE_MONO,
    }
}

/// The stereo decoder (`fm.stereo`, [`StereoDecoder`]): pilot loop, matrix and blend. It outputs the sound at the working
/// rate (MPX / 5), one channel when the stereo choice in force is mono and two otherwise, and the 57 kHz phasor of the pilot
/// loop for RDS (`carrier`, IQ at the MPX rate, 0 while unlocked). At each tick it takes the SNR measure (the target blend)
/// and reports ([`StereoReport`]).
pub struct StereoComponent {
    choice: StereoChoice,
    force_mono: bool,
    fm: f64,
    dec: Option<StereoDecoder>,
    audio: Vec<f32>,
    pilot: Vec<crate::pilot::PilotSample>,
    points: Vec<usize>,
}

impl Default for StereoComponent {
    fn default() -> Self {
        StereoComponent {
            choice: STEREO.default,
            force_mono: FORCE_MONO.default,
            fm: 0.0,
            dec: None,
            audio: Vec::new(),
            pilot: Vec::new(),
            points: Vec::new(),
        }
    }
}

impl StereoComponent {
    /// The stereo choice in force: mono while force mono is on.
    fn stereo(&self) -> StereoChoice {
        if self.force_mono {
            StereoChoice::Mono
        } else {
            self.choice
        }
    }
    /// The channels of the sound.
    fn channels(&self) -> u16 {
        if self.stereo() == StereoChoice::Mono {
            1
        } else {
            2
        }
    }
}

impl Component for StereoComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.stereo",
            vec![
                Port::real("in"),
                Port::typed::<MpxReport>("snr"),
                Port::typed::<DemodReport>("demod"),
            ],
            vec![
                Port::audio("out")
                    .rate_from(RateFrom::Divided { input: 0, by: FIVE })
                    .channels_from(ChannelsFrom::Configure),
                Port::iq("carrier").rate_from(RateFrom::Input(0)),
                item_out::<StereoReport>("report"),
            ],
        )
        .with_settings(StereoSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fm = inputs[0].rate;
        self.dec = Some(StereoDecoder::new(self.fm, self.stereo()));
        let mut audio = Format::audio(self.fm / 5.0, self.channels());
        audio.bandwidth_hz = 15e3;
        let mut carrier = Format::iq(self.fm, 57e3, 57e3);
        carrier.bandwidth_hz = 0.0;
        Ok(vec![
            audio,
            carrier,
            Format::new(Kind::typed::<StereoReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        resets(io.input(0), x.len(), &mut self.points);
        let (fm, choice, channels) = (self.fm, self.stereo(), self.channels());
        let Some(dec) = &mut self.dec else { return };
        let mut done = 0;
        for k in 0..=self.points.len() {
            let end = self.points.get(k).copied().unwrap_or(x.len());
            self.audio.clear();
            self.pilot.clear();
            dec.process(&x[done..end], channels, &mut self.audio, &mut self.pilot);
            io.audio_out(0).samples.extend_from_slice(&self.audio);
            io.iq_out(1).extend(self.pilot.iter().map(|p| {
                if p.locked {
                    p.carrier57
                } else {
                    Complex32::default()
                }
            }));
            if k < self.points.len() {
                *dec = StereoDecoder::new(fm, choice);
            }
            done = end;
        }
        if io.tick() {
            if let Some(s) = snr(io, 1) {
                dec.set_snr(s);
            }
            let carrier = last::<DemodReport>(io, 2).is_some_and(|d| d.carrier);
            let r = dec.report(carrier);
            io.items_out::<StereoReport>(2).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let channels = self.channels();
        match StereoSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            StereoSetting::Stereo => self.choice = STEREO.read(id, v)?,
            StereoSetting::ForceMono => self.force_mono = FORCE_MONO.read(id, v)?,
        }
        if self.channels() != channels {
            return Ok(Applied::Reformat);
        }
        let choice = self.stereo();
        if let Some(d) = &mut self.dec {
            d.set_choice(choice);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                StereoSetting::Stereo,
                Value::Choice(sdrkit_core::Choice::index(self.choice)),
            ),
            (StereoSetting::ForceMono, Value::Bool(self.force_mono)),
        ])
    }
}

/// Works on the interleaved frames of audio input 0 in place, piece by piece between its reset points: `f` on each piece,
/// `reset` at each point.
fn audio_pieces<T>(
    io: &mut Io,
    points: &mut Vec<usize>,
    state: &mut T,
    f: impl Fn(&mut T, &mut [f32]),
    reset: impl Fn(&mut T),
) {
    let input = io.input(0);
    let channels = usize::from(io.input_format(0).channels.max(1));
    let buf = io.audio_pass_through(0, 0);
    resets(input, buf.len() / channels, points);
    let mut done = 0;
    for &p in points.iter() {
        f(state, &mut buf[done * channels..p * channels]);
        reset(state);
        done = p;
    }
    f(state, &mut buf[done * channels..]);
}

sdrkit_core::settings! {
    /// The settings of [`DeemphasisComponent`].
    pub enum DeemphasisSetting {
        /// The de-emphasis.
        Deemphasis = DEEMPHASIS,
    }
}

/// De-emphasis (`fm.deemphasis`, [`Deemphasis`]) on the audio frames, in place. Its setting is the time constant.
pub struct DeemphasisComponent {
    choice: crate::settings::Deemphasis,
    rate: f64,
    channels: usize,
    d: Deemphasis,
    points: Vec<usize>,
}

impl Default for DeemphasisComponent {
    fn default() -> Self {
        DeemphasisComponent {
            choice: DEEMPHASIS.default,
            rate: 0.0,
            channels: 1,
            d: Deemphasis::new(0.0, 1.0, 1),
            points: Vec::new(),
        }
    }
}

impl Component for DeemphasisComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.deemphasis",
            vec![Port::audio("in")],
            vec![Port::audio("out").same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(DeemphasisSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.channels = usize::from(inputs[0].channels);
        self.d = Deemphasis::new(self.choice.tau_s(), self.rate, self.channels);
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let (tau, rate, channels) = (self.choice.tau_s(), self.rate, self.channels);
        audio_pieces(
            io,
            &mut self.points,
            &mut self.d,
            |d, b| d.process(b),
            |d| *d = Deemphasis::new(tau, rate, channels),
        );
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DeemphasisSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DeemphasisSetting::Deemphasis => self.choice = DEEMPHASIS.read(id, v)?,
        }
        if self.rate > 0.0 {
            self.d.set_tau(self.choice.tau_s());
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            DeemphasisSetting::Deemphasis,
            Value::Choice(sdrkit_core::Choice::index(self.choice)),
        )])
    }
}

sdrkit_core::settings! {
    /// The settings of [`AudioFiltersComponent`].
    pub enum AudioFiltersSetting {
        /// The audio bandwidth.
        AudioBandwidth = AUDIO_BANDWIDTH,
        /// The bass shelf.
        Bass = BASS,
        /// The treble shelf.
        Treble = TREBLE,
        /// 3 dB more off the treble.
        Warm = WARM,
    }
}

/// Tone and bandwidth (`fm.audio_filters`, [`AudioFilters`]) at 48 kHz, in place. Its input takes audio at 48 kHz, so the
/// graph changes the rate in front of it.
pub struct AudioFiltersComponent {
    bandwidth: AudioBandwidth,
    bass_db: f64,
    treble_db: f64,
    warm: bool,
    channels: usize,
    f: AudioFilters,
    points: Vec<usize>,
}

impl Default for AudioFiltersComponent {
    fn default() -> Self {
        let mut c = AudioFiltersComponent {
            bandwidth: AUDIO_BANDWIDTH.default,
            bass_db: BASS.default,
            treble_db: TREBLE.default,
            warm: WARM.default,
            channels: 1,
            f: AudioFilters::default(),
            points: Vec::new(),
        };
        c.f = c.filters();
        c
    }
}

impl AudioFiltersComponent {
    fn filters(&self) -> AudioFilters {
        let mut f = AudioFilters::new(self.channels);
        f.set_tone(self.bass_db, self.treble_db, self.warm);
        f.set_bandwidth(self.bandwidth.hz());
        f
    }
}

impl Component for AudioFiltersComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.audio_filters",
            vec![Port::audio("in").rate(RateRule::Exact(AUDIO_RATE))],
            vec![Port::audio("out").same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(AudioFiltersSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.channels = usize::from(inputs[0].channels);
        self.f = self.filters();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let (bass, treble, warm, hz, channels) = (
            self.bass_db,
            self.treble_db,
            self.warm,
            self.bandwidth.hz(),
            self.channels,
        );
        audio_pieces(
            io,
            &mut self.points,
            &mut self.f,
            |f, b| f.process(b),
            |f| {
                *f = AudioFilters::new(channels);
                f.set_tone(bass, treble, warm);
                f.set_bandwidth(hz);
            },
        );
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let tone = (self.bass_db, self.treble_db, self.warm);
        let bandwidth = self.bandwidth;
        match AudioFiltersSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AudioFiltersSetting::AudioBandwidth => self.bandwidth = AUDIO_BANDWIDTH.read(id, v)?,
            AudioFiltersSetting::Bass => self.bass_db = BASS.read(id, v)?,
            AudioFiltersSetting::Treble => self.treble_db = TREBLE.read(id, v)?,
            AudioFiltersSetting::Warm => self.warm = WARM.read(id, v)?,
        }
        if (self.bass_db, self.treble_db, self.warm) != tone {
            self.f.set_tone(self.bass_db, self.treble_db, self.warm);
        }
        if self.bandwidth != bandwidth {
            self.f.set_bandwidth(self.bandwidth.hz());
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                AudioFiltersSetting::AudioBandwidth,
                Value::Choice(sdrkit_core::Choice::index(self.bandwidth)),
            ),
            (AudioFiltersSetting::Bass, Value::F64(self.bass_db)),
            (AudioFiltersSetting::Treble, Value::F64(self.treble_db)),
            (AudioFiltersSetting::Warm, Value::Bool(self.warm)),
        ])
    }
}

sdrkit_core::settings! {
    /// The settings of [`SquelchComponent`].
    pub enum SquelchSetting {
        /// The squelch threshold.
        Squelch = SQUELCH,
    }
}

/// The squelch (`fm.squelch`, [`Squelch`]) at 48 kHz, in place. It takes the SNR measure at each tick and reports
/// ([`SquelchReport`]).
pub struct SquelchComponent {
    threshold_db: f64,
    channels: usize,
    s: Squelch,
    points: Vec<usize>,
}

impl Default for SquelchComponent {
    fn default() -> Self {
        SquelchComponent {
            threshold_db: SQUELCH.default,
            channels: 1,
            s: Squelch::new(SQUELCH.default as f32),
            points: Vec::new(),
        }
    }
}

impl Component for SquelchComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.squelch",
            vec![Port::audio("in"), Port::typed::<MpxReport>("snr")],
            vec![
                Port::audio("out").same_format_as(0),
                item_out::<SquelchReport>("report"),
            ],
        )
        .in_place(0, 0)
        .with_settings(SquelchSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.channels = usize::from(inputs[0].channels);
        self.s = Squelch::new(self.threshold_db as f32);
        Ok(vec![
            inputs[0].clone(),
            Format::new(Kind::typed::<SquelchReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let (t, channels) = (self.threshold_db as f32, self.channels);
        audio_pieces(
            io,
            &mut self.points,
            &mut self.s,
            |s, b| s.process(b, channels),
            |s| *s = Squelch::new(t),
        );
        if io.tick() {
            if let Some(s) = snr(io, 1) {
                self.s.update(s);
            }
            let r = SquelchReport {
                squelched: self.s.squelched(),
            };
            io.items_out::<SquelchReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SquelchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SquelchSetting::Squelch => self.threshold_db = SQUELCH.read(id, v)?,
        }
        self.s.set_threshold(self.threshold_db as f32);
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SquelchSetting::Squelch, Value::F64(self.threshold_db))])
    }
}

sdrkit_core::settings! {
    /// The settings of [`RdsComponent`].
    pub enum RdsSetting {
        /// RDS or RBDS meanings.
        Variant = RDS_VARIANT,
        /// The RDS correction.
        Correction = RDS_CORRECTION,
        /// RDS voting.
        Voting = RDS_VOTING,
        /// The RDS character set.
        Charset = RDS_CHARSET,
        /// The group monitor.
        Monitor = RDS_MONITOR,
    }
}

sdrkit_core::commands! {
    /// The commands of [`RdsComponent`] and [`DisplayComponent`].
    pub enum FmCommand {
        /// Forgets what was decoded on this frequency.
        Clear = CLEAR,
    }
}

/// RDS (`fm.rds`): the demodulator ([`RdsDemod`], with the 57 kHz phasor from the pilot loop as its carrier) and the decoder
/// ([`RdsDecoder`]), in one component because block sync narrows the demodulator's loop. It also decides RDS against RBDS
/// from its settings, the tuned frequency and the extended country code. At each tick it reports an
/// [`RdsSnapshot`]. Command: `clear`.
pub struct RdsComponent {
    variant: Variant,
    correction: RdsCorrection,
    voting: bool,
    charset: RdsCharsetChoice,
    monitor: bool,
    fm: f64,
    tuned_hz: f64,
    demod: Option<RdsDemod>,
    decoder: RdsDecoder,
    ps_event: Option<String>,
    rt_event: bool,
    snap: Arc<RdsSnapshot>,
    points: Vec<usize>,
}

impl Default for RdsComponent {
    fn default() -> Self {
        let (correction, voting, charset) = (
            RDS_CORRECTION.default,
            RDS_VOTING.default,
            RDS_CHARSET.default,
        );
        RdsComponent {
            variant: RDS_VARIANT.default,
            correction,
            voting,
            charset,
            monitor: RDS_MONITOR.default,
            fm: 0.0,
            tuned_hz: 0.0,
            demod: None,
            decoder: RdsDecoder::new(correction.correction(voting), charset.charset()),
            ps_event: None,
            rt_event: false,
            snap: Arc::default(),
            points: Vec::new(),
        }
    }
}

impl RdsComponent {
    fn restart(&mut self) {
        self.demod = Some(RdsDemod::new(self.fm));
        self.decoder.reset();
        self.ps_event = None;
        self.rt_event = false;
    }
    /// RBDS meanings apply.
    fn rbds(&self) -> bool {
        let e = Evidence {
            tuned_hz: self.tuned_hz,
            ecc: self.decoder.groups.ecc,
        };
        self.variant.rbds_applies(&e)
    }
}

impl Component for RdsComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.rds",
            vec![Port::real("in"), Port::iq("carrier")],
            vec![item_out::<RdsReport>("report")],
        )
        .with_settings(RdsSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fm = inputs[0].rate;
        self.tuned_hz = inputs[0].tuned_hz;
        self.restart();
        self.snap = Arc::default();
        Ok(vec![Format::new(Kind::typed::<RdsReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let carrier = io.iq(1);
        let n = x.len().min(carrier.len());
        resets(io.input(0), n, &mut self.points);
        let points = std::mem::take(&mut self.points);
        let mut done = 0;
        let mut wall = None;
        for k in 0..=points.len() {
            let end = points.get(k).copied().unwrap_or(n);
            let Some(demod) = &mut self.demod else { return };
            let rds = &mut self.decoder;
            let (ps_event, rt_event) = (&mut self.ps_event, &mut self.rt_event);
            demod.process(&x[done..end], &carrier[done..end], &mut |bit, rel| {
                let w = *wall.get_or_insert_with(rds::wall_clock_s);
                rds.push_bit(bit, rel, w);
                if rds.groups.ps_changed {
                    match ps_event {
                        Some(s) => s.clone_from(&rds.groups.ps),
                        None => *ps_event = Some(rds.groups.ps.clone()),
                    }
                }
                *rt_event |= rds.groups.rt_completed;
                rds.blocks.synced()
            });
            if k < points.len() {
                self.restart();
            }
            done = end;
        }
        self.points = points;
        if io.tick() {
            let rbds = self.rbds();
            let snap = Arc::make_mut(&mut self.snap);
            snap.view.fill(&self.decoder);
            snap.rbds = rbds;
            let call = (rbds && snap.view.groups > 0)
                .then(|| crate::pty::call_letters(snap.view.pi))
                .flatten();
            match call {
                Some(c) if c != snap.call_letters => snap.call_letters = c,
                Some(_) => {}
                None => snap.call_letters.clear(),
            }
            snap.ps_changed = self.ps_event.take();
            snap.rt_completed = std::mem::take(&mut self.rt_event);
            snap.monitor.clear();
            snap.monitor.append(&mut self.decoder.monitor);
            snap.symbols.clear();
            if let Some(d) = &self.demod {
                snap.symbols.extend(d.points().map(|p| [p.re, p.im]));
            }
            let r = RdsReport(self.snap.clone());
            io.items_out::<RdsReport>(0).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RdsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RdsSetting::Variant => self.variant = RDS_VARIANT.read(id, v)?,
            RdsSetting::Correction => {
                self.correction = RDS_CORRECTION.read(id, v)?;
                self.decoder
                    .set_correction(self.correction.correction(self.voting));
            }
            RdsSetting::Voting => {
                self.voting = RDS_VOTING.read(id, v)?;
                self.decoder
                    .set_correction(self.correction.correction(self.voting));
            }
            RdsSetting::Charset => {
                self.charset = RDS_CHARSET.read(id, v)?;
                self.decoder.groups.set_charset(self.charset.charset());
            }
            RdsSetting::Monitor => {
                self.monitor = RDS_MONITOR.read(id, v)?;
                self.decoder.keep_monitor = self.monitor;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        use sdrkit_core::Choice;
        values_of([
            (RdsSetting::Variant, Value::Choice(self.variant.index())),
            (
                RdsSetting::Correction,
                Value::Choice(self.correction.index()),
            ),
            (RdsSetting::Voting, Value::Bool(self.voting)),
            (RdsSetting::Charset, Value::Choice(self.charset.index())),
            (RdsSetting::Monitor, Value::Bool(self.monitor)),
        ])
    }
    fn commands(&self) -> &Commands {
        FmCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match FmCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            FmCommand::Clear => {
                self.decoder.reset();
                self.ps_event = None;
                self.rt_event = false;
                self.snap = Arc::default();
            }
        }
        Ok(None)
    }
}

sdrkit_core::settings! {
    /// The settings of [`DisplayComponent`].
    pub enum DisplaySetting {
        /// The lowest frequency of the band.
        BandLowHz = BAND_LOW_HZ,
        /// The highest frequency of the band.
        BandHighHz = BAND_HIGH_HZ,
        /// The first channel of the raster.
        FirstHz = FIRST_HZ,
        /// The channel step.
        RasterHz = RASTER_HZ,
        /// Where the band scan starts.
        ScanLow = SCAN_LOW,
        /// Where the band scan ends.
        ScanHigh = SCAN_HIGH,
        /// The band scan waits for the whole RDS.
        ScanFullRds = SCAN_FULL_RDS,
    }
}

/// The FM display (`fm.display`): gathers every component's report at each tick into the FM results (status, the typed
/// fields and tables of [`crate::results`], the multiplex and RDS scopes) and the events (`fm.station`, `fm.text`). After 2 s
/// without a station it requests a new search from the station finder. It is the graph's status component. It holds the band
/// and the band scan's range, which the band scan reads from the stack's settings. Command: `clear`.
pub struct DisplayComponent {
    band: BandPlan,
    scan_low_mhz: f64,
    scan_high_mhz: f64,
    scan_full_rds: bool,
    store: Published,
    shown: Shown,
    summary: String,
    searching_s: f64,
}

impl Default for DisplayComponent {
    fn default() -> Self {
        let mut d = DisplayComponent {
            band: BandPlan::default(),
            scan_low_mhz: SCAN_LOW.default,
            scan_high_mhz: SCAN_HIGH.default,
            scan_full_rds: SCAN_FULL_RDS.default,
            store: results::store(),
            shown: Shown::default(),
            summary: String::new(),
            searching_s: 0.0,
        };
        d.restart();
        d
    }
}

/// One tick's reports of the FM components, as a display reads them.
pub(crate) struct Tick<'a> {
    /// The channel's report.
    pub ch: &'a ChannelReport,
    /// The demodulator's report.
    pub dm: &'a DemodReport,
    /// The multiplex spectrum's report.
    pub mpx: &'a MpxReport,
    /// The stereo decoder's report.
    pub st: &'a StereoReport,
    /// The squelch is closed.
    pub squelched: bool,
    /// The RDS snapshot.
    pub rds: &'a RdsSnapshot,
}

impl Tick<'_> {
    /// The carrier offset from the tuned frequency: the channel shift plus what the demodulator still measures, Hz.
    pub fn offset_hz(&self) -> f64 {
        self.dm.offset_hz + self.ch.shift_hz
    }
}

/// Clears the fields, tables, series and log of `store` in place and shows "Searching". Shared by the FM and the STL
/// displays.
pub(crate) fn clear_results(store: &mut Published, shown: &mut Shown) {
    *shown = Shown::default();
    let r = store.results_mut();
    r.fields_mut().clear();
    for i in 0..r.decl().tables().len() {
        if let Some(mut t) = r.table_mut(sdrkit_core::TableId(i as u16)) {
            t.clear();
        }
    }
    for id in [results::MPX_SERIES, results::RDS_SYMBOLS_SERIES] {
        r.update_series(id, |s| {
            let changed = !s.points.is_empty();
            s.points.clear();
            changed
        });
    }
    r.clear_log();
    r.set_status(&results::searching());
}

/// Writes what the FM components measured and decoded at one tick: the RDS events on `io`'s event output 0, the lines of
/// the group monitor, the multiplex and RDS scopes, and the reception fields with the RDS tables. Shared by the FM and
/// the STL displays.
pub(crate) fn publish_reception(store: &mut Published, shown: &mut Shown, t: &Tick, io: &mut Io) {
    let (rds, v) = (t.rds, &t.rds.view);
    let (time_s, freq) = (t.ch.time_s, t.ch.tuned_hz);
    let pi = results::hex4(v.pi);
    if let Some(ps) = &rds.ps_changed {
        io.messages_out(0).push(STATION_NAMED.event(
            time_s,
            freq,
            &StationEvent {
                source: pi.as_str(),
                text: ps,
                family: Family::Broadcast,
            },
        ));
    }
    if rds.rt_completed {
        io.messages_out(0).push(TEXT_DONE.event(
            time_s,
            freq,
            &StationEvent {
                source: pi.as_str(),
                text: &v.text,
                family: Family::Broadcast,
            },
        ));
    }
    let r = store.results_mut();
    for line in &rds.monitor {
        r.log(line.clone());
    }
    results::write_series(r, results::MPX_SERIES, true, t.mpx.scope());
    results::write_series(r, results::RDS_SYMBOLS_SERIES, false, &rds.symbols);
    let figures = Figures {
        carrier: t.dm.carrier,
        stereo: t.st.stereo,
        snr_db: t.mpx.snr_db,
        level_dbfs: t.ch.level_dbfs,
        deviation_hz: t.dm.dev_khz as f64 * 1e3,
        multipath_pct: t.dm.multipath_pct,
        pilot_pct: t.st.pilot_pct,
        stereo_pct: t.st.stereo_pct,
        squelched: t.squelched,
        offset_hz: t.offset_hz(),
        equaliser: t.dm.equaliser,
        pilot_locked: t.st.locked,
        channel_width_hz: t.ch.width_hz,
        signal_s: t.ch.time_s,
    };
    let meanings = Meanings {
        rbds: rds.rbds,
        programme: v.pty.map_or("", |p| crate::pty::pty_name(p, rds.rbds)),
        call_letters: (!rds.call_letters.is_empty()).then_some(rds.call_letters.as_str()),
    };
    results::publish(store, shown, v, &figures, &meanings);
}

/// The lock state of an FM carrier from its SNR measure: searching without a carrier or under 10 dB, tracking under 20 dB,
/// else locked.
pub(crate) fn lock_state(carrier: bool, snr_db: f32) -> LockState {
    if !carrier || snr_db < 10.0 {
        LockState::Searching
    } else if snr_db < 20.0 {
        LockState::Tracking
    } else {
        LockState::Locked
    }
}

impl DisplayComponent {
    /// Clears the results in place and writes the band again.
    fn restart(&mut self) {
        self.searching_s = 0.0;
        clear_results(&mut self.store, &mut self.shown);
        results::publish_band(&mut self.store, &self.band);
    }

    fn report(&mut self, t: &Tick, io: &mut Io) {
        use std::fmt::Write;
        if t.ch.reset {
            self.restart();
        }
        let snr = t.mpx.snr_db;
        let lock_state = lock_state(t.dm.carrier, snr);
        // The finder searches again after 2 s of searching.
        self.searching_s = if lock_state == LockState::Searching {
            self.searching_s + REPORT_S
        } else {
            0.0
        };
        if self.searching_s > 2.0 {
            io.items_out::<FinderCommand>(1).push(Timed {
                time: 0.0,
                value: FinderCommand::Start,
            });
            self.searching_s = 0.0;
        }
        let v = &t.rds.view;
        let s = &mut self.summary;
        s.clear();
        s.push_str(match lock_state {
            LockState::Searching => "Searching",
            LockState::Tracking => "Tracking",
            _ => "Locked",
        });
        if t.st.stereo {
            s.push_str(", stereo");
        }
        if v.sync {
            let _ = write!(s, ", RDS {:.0} %", v.ok_pct);
        }
        if !v.station.trim().is_empty() {
            let _ = write!(s, ", {}", v.station.trim());
        }
        publish_reception(&mut self.store, &mut self.shown, t, io);
        let r = self.store.results_mut();
        results::write_status(r, lock_state, snr, t.ch.level_dbfs, t.offset_hz(), &self.summary);
    }
}

impl Component for DisplayComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fm.display",
            vec![
                Port::typed::<ChannelReport>("channel"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<MpxReport>("spectrum"),
                Port::typed::<StereoReport>("stereo"),
                Port::typed::<SquelchReport>("squelch"),
                Port::typed::<RdsReport>("rds"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                item_out::<FinderCommand>("finder"),
            ],
        )
        .with_settings(DisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<FinderCommand>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let (Some(ch), Some(dm), Some(st), Some(sq)) = (
            last::<ChannelReport>(io, 0),
            last::<DemodReport>(io, 1),
            last::<StereoReport>(io, 3),
            last::<SquelchReport>(io, 4),
        ) else {
            return;
        };
        let (Some(mpx), Some(rds)) = (
            io.items::<MpxReport>(2).last(),
            io.items::<RdsReport>(5).last(),
        ) else {
            return;
        };
        let rds = Arc::clone(&rds.value.0);
        let t = Tick {
            ch: &ch,
            dm: &dm,
            mpx: &mpx.value,
            st: &st,
            squelched: sq.squelched,
            rds: &rds,
        };
        self.report(&t, io);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DisplaySetting::BandLowHz => self.band.min_hz = BAND_LOW_HZ.read(id, v)?,
            DisplaySetting::BandHighHz => self.band.max_hz = BAND_HIGH_HZ.read(id, v)?,
            DisplaySetting::FirstHz => self.band.first_hz = FIRST_HZ.read(id, v)?,
            DisplaySetting::RasterHz => self.band.step_hz = RASTER_HZ.read(id, v)?,
            DisplaySetting::ScanLow => self.scan_low_mhz = SCAN_LOW.read(id, v)?,
            DisplaySetting::ScanHigh => self.scan_high_mhz = SCAN_HIGH.read(id, v)?,
            DisplaySetting::ScanFullRds => self.scan_full_rds = SCAN_FULL_RDS.read(id, v)?,
        }
        results::publish_band(&mut self.store, &self.band);
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DisplaySetting::BandLowHz, Value::Hz(self.band.min_hz)),
            (DisplaySetting::BandHighHz, Value::Hz(self.band.max_hz)),
            (DisplaySetting::FirstHz, Value::Hz(self.band.first_hz)),
            (DisplaySetting::RasterHz, Value::Hz(self.band.step_hz)),
            (DisplaySetting::ScanLow, Value::F64(self.scan_low_mhz)),
            (DisplaySetting::ScanHigh, Value::F64(self.scan_high_mhz)),
            (DisplaySetting::ScanFullRds, Value::Bool(self.scan_full_rds)),
        ])
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

// The registry and the graph.

/// Adds the FM components (kinds `fm.*`) and those of the STL stack (kinds `stl.*`) to `r`.
pub fn register(r: &mut Registry) {
    r.add("fm.finder", || Box::new(FinderComponent::default()));
    r.add("fm.channel", || Box::new(ChannelComponent::default()));
    r.add("fm.demod", || Box::new(DemodComponent::default()));
    r.add("fm.mpx_spectrum", || Box::new(SpectrumComponent::default()));
    r.add("fm.stereo", || Box::new(StereoComponent::default()));
    r.add("fm.deemphasis", || Box::new(DeemphasisComponent::default()));
    r.add("fm.audio_filters", || {
        Box::new(AudioFiltersComponent::default())
    });
    r.add("fm.squelch", || Box::new(SquelchComponent::default()));
    r.add("fm.rds", || Box::new(RdsComponent::default()));
    r.add("fm.display", || Box::new(DisplayComponent::default()));
    r.add(crate::transmit::ID, || {
        Box::new(crate::transmit::FmTransmit::default())
    });
    crate::stl::register(r);
}

/// The standard components and the FM ones, so that an FM graph can be built from its description.
pub fn registry() -> Registry {
    let mut r = sdrkit_components::registry();
    register(&mut r);
    r
}

/// The component that each stack setting goes to.
fn component_of(s: FmSetting) -> Option<&'static str> {
    use FmSetting as S;
    Some(match s {
        S::StationFinder => "finder",
        S::ChannelWidth | S::ChannelFilter => "channel",
        S::Stereo | S::ForceMono => "stereo",
        S::Deemphasis => "deemph",
        S::AudioBandwidth | S::Bass | S::Treble | S::Warm => "audio",
        S::Squelch => "squelch",
        S::ScanLow
        | S::ScanHigh
        | S::ScanFullRds
        | S::BandLowHz
        | S::BandHighHz
        | S::FirstHz
        | S::RasterHz => "display",
        S::RdsVariant | S::RdsCorrection | S::RdsVoting | S::RdsCharset => "rds",
        S::GenStereo
        | S::GenLeftHz
        | S::GenLeftAmp
        | S::GenRightHz
        | S::GenRightAmp
        | S::GenPilot
        | S::GenRds
        | S::GenRdsKhz
        | S::GenPs
        | S::GenRt
        | S::GenPi
        | S::GenPty
        | S::GenTp
        | S::GenTa
        | S::GenMusic
        | S::GenPreemphasis
        | S::GenExtraGroups => return None,
        S::TxPi
        | S::TxPs
        | S::TxRt
        | S::TxPty
        | S::TxTp
        | S::TxTa
        | S::TxMusic
        | S::TxAf
        | S::TxCt
        | S::TxCtOffset
        | S::TxStereo
        | S::TxPreemphasis
        | S::TxDeviation
        | S::TxPilot
        | S::TxRdsLevel
        | S::TxLimiter => "transmit",
    })
}

/// The FM graph at its defaults, with the host's audio input in stereo.
pub fn builder() -> GraphBuilder {
    builder_with_audio(Box::new(HostIn::stereo()))
}

/// The FM graph at its defaults, with `audio` in place of the host's audio input: a component with input `in` (the
/// session's samples) and audio output `out`, mono or stereo.
pub fn builder_with_audio(audio: Box<dyn Component>) -> GraphBuilder {
    let tx = |c: &str| sdrkit_graph::CommandTarget::of("transmit", c);
    let mut b = GraphBuilder::new()
        .add("finder", FinderComponent::default())
        .add("channel", ChannelComponent::default())
        .add("demod", DemodComponent::default())
        .add("spectrum", SpectrumComponent::default())
        .add("stereo", StereoComponent::default())
        .add("deemph", DeemphasisComponent::default())
        .add("audio", AudioFiltersComponent::default())
        .add("squelch", SquelchComponent::default())
        .add("rds", RdsComponent::default())
        .add("display", DisplayComponent::default())
        .add_boxed("mic", audio)
        .add("transmit", crate::transmit::FmTransmit::default())
        .add("tx", TxSink::default())
        .add("mute", TxMute::default())
        .input("finder.in")
        .input("channel.in")
        .input("mic.in")
        .input("transmit.in")
        .connect("finder.moves", "channel.finder")
        .connect("channel.out", "demod.in")
        .connect("channel.setup", "demod.setup")
        .connect("demod.meas", "spectrum.in")
        .connect("demod.report", "spectrum.demod")
        .connect("demod.sound", "stereo.in")
        .connect("spectrum.report", "stereo.snr")
        .connect("demod.report", "stereo.demod")
        .connect("stereo.out", "deemph.in")
        .connect("deemph.out", "audio.in")
        .connect("audio.out", "squelch.in")
        .connect("spectrum.report", "squelch.snr")
        .connect("demod.meas", "rds.in")
        .connect("stereo.carrier", "rds.carrier")
        .connect("channel.report", "display.channel")
        .connect("demod.report", "display.demod")
        .connect("spectrum.report", "display.spectrum")
        .connect("stereo.report", "display.stereo")
        .connect("squelch.report", "display.squelch")
        .connect("rds.report", "display.rds")
        .connect("mic.out", "transmit.audio")
        .connect("transmit.out", "tx.in")
        .connect("squelch.out", "mute.in")
        .connect("transmit.keyed", "mute.keyed")
        .feedback("spectrum.report", "demod.snr")
        .feedback("display.finder", "finder.control")
        .output("mute.out")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .command_alias("clear", sdrkit_graph::CommandTarget::every("clear"));
    for c in crate::transmit::COMMANDS {
        b = b.command_alias(c, tx(c));
    }
    for s in FmSetting::SETTINGS {
        let key = s.key();
        let Some(c) = FmSetting::schema()
            .id(key)
            .ok()
            .and_then(FmSetting::from_id)
            .and_then(component_of)
        else {
            continue;
        };
        b = b.alias(key, &format!("{c}.{key}"));
    }
    b
}

/// The FM graph at its defaults.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build: a fault of this crate, never of the input.
pub fn fm_graph() -> Result<Graph, GraphError> {
    builder().build()
}

/// The FM receiver: the FM graph as the engine runs it, with the stack's settings applied.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    receiver_with_audio(cfg, Box::new(HostIn::stereo()))
}

/// As [`receiver`], with `audio` in place of the host's audio input (see [`builder_with_audio`]).
///
/// # Errors
///
/// As [`receiver`].
pub fn receiver_with_audio(
    cfg: &StackConfig,
    audio: Box<dyn Component>,
) -> Result<StackGraph, GraphError> {
    let mut b = StackGraph::of(
        crate::typed::shape(),
        builder_with_audio(audio)
            .catalog(&Catalog::of(cfg)?)
            .build()?,
    )
    .min_rate(MIN_RATE_HZ, "FM needs a sample rate of at least 500 ksps")
    .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for c in crate::transmit::COMMANDS {
        b = b.command(c, sdrkit_graph::CommandTarget::of("transmit", c));
    }
    b.on_retune(sdrkit_graph::CommandTarget::every("clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The FM graph's description (components, settings, edges, feedback, aliases), which [`registry`] builds again: for a
/// front end to show or alter.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn description() -> Result<Description, GraphError> {
    Ok(fm_graph()?.describe())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::Owner;

    #[test]
    fn the_description_builds_the_same_graph_again() {
        let d = description().expect("builds");
        assert_eq!(d.components.len(), 14);
        assert_eq!(d.feedback.len(), 2);
        assert_eq!(d.clock_s, Some(REPORT_S));
        let g = Graph::from_json(&d.to_json(), &registry()).expect("builds");
        assert_eq!(g.describe(), d);
        // Every stack setting that the receiver takes reaches a component.
        let s = g.schema();
        for key in FmSetting::SETTINGS
            .iter()
            .map(|s| s.key())
            .filter(|k| !k.starts_with("gen_"))
        {
            assert!(s.id(key).is_ok(), "{key}");
        }
    }
}
