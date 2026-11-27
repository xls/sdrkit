//! The AIS receiver as a graph of components: two pair front ends (`ais.pair`), four channels (`ais.channel`) with a burst
//! detector (`ais.detector`) and decoder (`ais.decoder`) each, the message layer (`ais.messages`) and the display (`ais.stations`).

use super::demod::{self, BurstCut, DecodeOptions, Decoder, Detector, Frame, Layer};
pub use super::display::Stations;
use super::display::{StationsSetting, SATELLITE};
use super::message::{self, AisMessage};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Schema, Setting, SettingError, SettingId, SettingSet,
    StackConfig, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder,
    GraphError, Io, Item, Kind, Port, RateFrom, Registry, StackGraph, Timed,
};
use std::sync::{Arc, OnceLock};

/// The four channels: letter, name, frequency.
pub const CHANNELS: [(char, &str, f64); 4] = [
    ('A', "AIS 1 (87B)", 161.975e6),
    ('B', "AIS 2 (88B)", 162.025e6),
    ('C', "Long range 75", 156.775e6),
    ('D', "Long range 76", 156.825e6),
];
/// The middles of the two pairs.
pub const PAIRS: [f64; 2] = [162.0e6, 156.8e6];
/// The pair rate.
pub const PAIR_RATE: f64 = 96_000.0;
/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The report interval, seconds of input.
pub const REPORT_S: f64 = 0.25;

sdrkit_core::choice_enum! {
    /// Which AIS channels are decoded.
    pub enum ChannelLimit {
        /// All four.
        All = ("all", "All four"),
        /// AIS 1 and 2 only.
        Ais12 = ("ais12", "AIS 1 + 2 only"),
        /// Channels 75 and 76 only.
        LongRange = ("longrange", "75 + 76 only"),
    }
}

/// The channels a pair front end and the display take.
pub const LIMIT: Setting<field::Choice<ChannelLimit>> =
    Setting::choice("limit", "Channels", ChannelLimit::All);

/// Where each pair's middle lies in the stream (Hz from its centre), or `None` when it is not in view.
pub fn placement(
    rate: f64,
    centre_hz: f64,
    tuned_hz: f64,
    limit: ChannelLimit,
) -> [Option<f64>; 2] {
    let mut out = [None, None];
    let known = centre_hz > 0.0;
    for (p, mid) in PAIRS.iter().enumerate() {
        if (limit == ChannelLimit::Ais12 && p == 1) || (limit == ChannelLimit::LongRange && p == 0)
        {
            continue;
        }
        let off = if known {
            mid - centre_hz
        } else {
            let dial = tuned_hz - centre_hz;
            if p == 0 {
                dial
            } else {
                dial - 5.2e6
            }
        };
        if off.abs() + 40e3 <= 0.49 * rate {
            out[p] = Some(off);
        }
    }
    out
}

/// The hopping plan for a radio that cannot hold all four: (centre, dwell seconds) steps.
pub fn hop_plan(rate: f64) -> Vec<(f64, f64)> {
    let all = placement(rate, 159.4e6, 159.4e6, ChannelLimit::All);
    if all[0].is_some() && all[1].is_some() {
        vec![(159.4e6, f64::INFINITY)]
    } else {
        vec![(162.0e6, 60.0), (156.8e6, 200.0)]
    }
}

/// Hopping: steps through [`hop_plan`] and reports where the radio must go. The display requests the host to move the radio (a
/// `HostRequest::Retune`). The station table survives the retune.
#[derive(Clone, Debug, Default)]
pub struct Hopper {
    plan: Vec<(f64, f64)>,
    step: usize,
    since: f64,
}

impl Hopper {
    /// The plan for this input rate, from its first step at `t`.
    pub fn new(rate: f64, t: f64) -> Self {
        Hopper {
            plan: hop_plan(rate),
            step: 0,
            since: t,
        }
    }
    /// The plan: (centre, dwell seconds) steps.
    pub fn plan(&self) -> &[(f64, f64)] {
        &self.plan
    }
    /// The centre the radio must be on now.
    pub fn current(&self) -> Option<f64> {
        self.plan.get(self.step).map(|s| s.0)
    }
    /// The dwell of the current step, seconds (infinite for a single step).
    pub fn dwell(&self) -> Option<f64> {
        self.plan.get(self.step).map(|s| s.1)
    }
    /// Advances the clock to `t`. Returns the new centre when the radio must move.
    pub fn tick(&mut self, t: f64) -> Option<f64> {
        let (_, dwell) = *self.plan.get(self.step)?;
        if self.plan.len() < 2 || t - self.since < dwell {
            return None;
        }
        self.step = (self.step + 1) % self.plan.len();
        self.since = t;
        self.current()
    }
}

// Items.

/// A burst cut out of a channel.
#[derive(Clone, Debug)]
pub struct AisBurst(pub Arc<BurstCut>);
impl Item for AisBurst {
    const KIND: &'static str = "ais.burst";
}

/// A detector's report at each tick.
#[derive(Clone, Debug, Default)]
pub struct ChannelLevel {
    /// The channel index (0 to 3).
    pub channel: u8,
    /// Its frequency, Hz.
    pub freq_hz: f64,
    /// Mean power (48 kHz band).
    pub level: f64,
    /// The noise floor, mean power per sample, once known.
    pub floor: Option<f64>,
    /// Bursts detected.
    pub detected: u64,
    /// Signal seconds since the configure.
    pub clock_s: f64,
}
impl Item for ChannelLevel {
    const KIND: &'static str = "ais.channel_level";
}

/// The result of decoding one burst.
#[derive(Clone, Debug, Default)]
pub struct AisDecoded {
    /// The channel index (0 to 3).
    pub channel: u8,
    /// The frames found.
    pub frames: Vec<Frame>,
    /// A frame failed its FCS and nothing was decoded.
    pub bad: bool,
    /// The start and the end of the burst's core, seconds of input.
    pub core_s: (f64, f64),
    /// The mean power of the core.
    pub power: f64,
}
impl Item for AisDecoded {
    const KIND: &'static str = "ais.decoded";
}

/// A frame with a good FCS (for VDES and other listeners), whether or not it is AIS.
#[derive(Clone, Debug)]
pub struct AisFrame {
    /// The channel letter.
    pub channel: char,
    /// The payload bits.
    pub bits: Arc<Vec<u8>>,
    /// The SNR in 48 kHz, dB.
    pub snr_db: f64,
    /// The carrier, Hz from the channel centre.
    pub offset_hz: f64,
    /// How it was found.
    pub layer: Layer,
}
impl Item for AisFrame {
    const KIND: &'static str = "ais.frame";
}

/// A decoded message.
#[derive(Clone, Debug)]
pub struct AisMsg(pub Arc<AisMessage>);
impl Item for AisMsg {
    const KIND: &'static str = "ais.message";
}

/// An NMEA sentence.
#[derive(Clone, Debug)]
pub struct NmeaSentence(pub String);
impl Item for NmeaSentence {
    const KIND: &'static str = "nmea.sentence";
}

/// A reception: an accepted message with how it was heard.
#[derive(Clone, Debug)]
pub struct Reception {
    /// Signal seconds since the configure.
    pub time: f64,
    /// The channel letter.
    pub channel: char,
    /// The SNR in 48 kHz, dB.
    pub snr_db: f64,
    /// The carrier, Hz from the channel centre.
    pub offset_hz: f64,
    /// How it was found.
    pub layer: Layer,
    /// The message.
    pub message: Arc<AisMessage>,
    /// Its `!AIVDM` sentences.
    pub sentences: Vec<String>,
}

/// What the message layer reports to the station table.
#[derive(Clone, Debug)]
pub enum AisLayerEvent {
    /// An accepted message.
    Reception(Arc<Reception>),
    /// A burst with a bad FCS, or a frame refused as AIS, on channel index `channel`.
    Bad {
        /// The channel index (0 to 3).
        channel: u8,
    },
}
impl Item for AisLayerEvent {
    const KIND: &'static str = "ais.reception";
}

/// Frequency candidates per channel, fed back from the station table to the decoders.
#[derive(Clone, Debug, Default)]
pub struct AisHints(pub [Vec<f64>; 4]);
impl Item for AisHints {
    const KIND: &'static str = "ais.hints";
}

/// No frequency candidates (`ais.no_hints`): feeds the decoders' hint input in graphs without a station table.
#[derive(Default)]
pub struct NoHints;

impl Component for NoHints {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator(
            "ais.no_hints",
            vec![Port::typed::<AisHints>("hints").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<AisHints>(), 0.0)])
    }
    fn process(&mut self, _: &mut Io) {}
    fn values(&self) -> Values {
        no_values()
    }
}

// The pair front end.

sdrkit_core::settings! {
    /// The settings of [`PairFrontEnd`].
    pub enum PairSetting {
        /// Which channels are decoded.
        Limit = LIMIT,
    }
}

/// The pair front end (`ais.pair`): the pair's middle mixed to 0 Hz and resampled exactly to 96 000 Hz. A pair out of view
/// produces no samples.
pub struct PairFrontEnd {
    pair: usize,
    limit: ChannelLimit,
    chan: Option<Channelizer>,
    input: Option<Format>,
}

impl PairFrontEnd {
    /// For pair 0 (A + B) or 1 (C + D).
    pub fn new(pair: usize) -> Self {
        PairFrontEnd {
            pair,
            limit: ChannelLimit::All,
            chan: None,
            input: None,
        }
    }
    fn build(&mut self) -> Result<(), ConfigError> {
        let Some(f) = &self.input else {
            return Ok(());
        };
        let place = placement(f.rate, f.centre_hz, f.tuned_hz, self.limit);
        self.chan = match place[self.pair] {
            Some(off) => {
                let mut cfg = ChannelConfig::new(f.rate, off, 76e3, PAIR_RATE);
                cfg.exact_rate = true;
                cfg.atten_db = 60.0;
                Some(Channelizer::new(cfg)?)
            }
            None => None,
        };
        Ok(())
    }
}

impl Component for PairFrontEnd {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ais.pair",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(PAIR_RATE))],
        )
        .with_settings(PairSetting::schema())
        .manual_tags()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.input = Some(inputs[0].clone());
        self.build()?;
        Ok(vec![Format::iq(
            PAIR_RATE,
            PAIRS[self.pair],
            PAIRS[self.pair],
        )])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if let Some(c) = &mut self.chan {
            c.process(x, io.iq_out(0));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PairSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PairSetting::Limit => self.limit = LIMIT.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(PairSetting::Limit, Value::Choice(self.limit.index()))])
    }
}

// The channel filter.

sdrkit_core::settings! {
    /// The settings of [`ChannelFilter`] and [`BurstDecoder`] beside their own.
    pub enum ChannelSetting {
        /// Satellite mode.
        Satellite = SATELLITE,
    }
}

/// The channel (`ais.channel`): mixed by -+25 kHz (a 96-entry table), low-passed and decimated by 2 to 48 kHz, then the channel
/// filter (coast: passband 7.5 kHz, stopband 15 kHz, satellite: 12 / 18 kHz).
pub struct ChannelFilter {
    index: usize,
    satellite: bool,
    table: Vec<Complex32>,
    pos: usize,
    dec: Option<FirDecimator<Complex32>>,
    filt: Option<Fir<Complex32>>,
    mixed: Vec<Complex32>,
}

impl ChannelFilter {
    /// For channel 0 to 3 (A, B, C, D).
    pub fn new(index: usize) -> Self {
        // A and C sit 25 kHz below the middle: move them up.
        let sign = if index.is_multiple_of(2) { 1.0 } else { -1.0 };
        let table = (0..96)
            .map(|k| {
                let p = sign * std::f64::consts::TAU * 25.0 * k as f64 / 96.0;
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect();
        ChannelFilter {
            index,
            satellite: false,
            table,
            pos: 0,
            dec: None,
            filt: None,
            mixed: Vec::new(),
        }
    }
    /// The same channel with satellite mode on or off.
    pub fn with_satellite(mut self, on: bool) -> Self {
        self.satellite = on;
        self
    }
    fn taps(&self) -> (Vec<f32>, Vec<f32>) {
        let (pass, stop) = if self.satellite {
            (12e3, 18e3)
        } else {
            (7.5e3, 15e3)
        };
        // The other channel of the pair lies 50 kHz away and folds into the passband at 48 kHz: 100 dB keeps it out.
        let d = fir::lowpass(PAIR_RATE, (9.5e3f64).max(pass + 1.5e3), 38e3, 100.0)
            .expect("the filter settings are valid");
        let c = fir::lowpass(demod::RATE, pass, stop, 60.0).expect("the filter settings are valid");
        (d, c)
    }
    /// The noise bandwidth of the two filters together, Hz.
    pub fn enbw(&self) -> f64 {
        let (d, c) = self.taps();
        let n = 960;
        let mut sum = 0.0;
        let mut h0 = 0.0;
        for k in 0..n {
            let f = -24e3 + 48e3 * k as f64 / n as f64;
            let h1 = fir::response(&d, f / PAIR_RATE);
            let h2 = fir::response(&c, f / demod::RATE);
            let g = (h1 * h2).norm_sqr();
            sum += g;
            if k == n / 2 {
                h0 = g;
            }
        }
        sum / n as f64 * 48e3 / h0.max(1e-30)
    }
}

impl Component for ChannelFilter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ais.channel",
            vec![Port::iq("in").rate(sdrkit_graph::RateRule::Exact(PAIR_RATE))],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(demod::RATE))],
        )
        .with_settings(ChannelSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let (d, c) = self.taps();
        self.dec = Some(FirDecimator::new(&d, 2));
        self.filt = Some(Fir::new(&c));
        self.pos = 0;
        let freq = CHANNELS[self.index].2;
        Ok(vec![Format::iq(demod::RATE, freq, freq)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if x.is_empty() {
            return;
        }
        self.mixed.clear();
        for &s in x {
            self.mixed.push(s * self.table[self.pos]);
            self.pos = (self.pos + 1) % 96;
        }
        let out = io.iq_out(0);
        let start = out.len();
        if let (Some(d), Some(c)) = (&mut self.dec, &mut self.filt) {
            d.process(&self.mixed, out);
            c.process_in_place(&mut out[start..]);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChannelSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelSetting::Satellite => self.satellite = SATELLITE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(ChannelSetting::Satellite, Value::Bool(self.satellite))])
    }
}

// The burst detector and decoder.

/// The burst detector (`ais.detector`): bursts cut out with their core, and a level report at each tick.
pub struct BurstDetector {
    index: usize,
    det: Detector,
    samples: u64,
    cuts: Vec<BurstCut>,
}

impl BurstDetector {
    /// For channel 0 to 3.
    pub fn new(index: usize) -> Self {
        BurstDetector {
            index,
            det: Detector::new(index as u8),
            samples: 0,
            cuts: Vec::new(),
        }
    }
}

impl Component for BurstDetector {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ais.detector",
            vec![Port::iq("in")],
            vec![
                Port::typed::<AisBurst>("bursts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<ChannelLevel>("level").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.det.reset();
        self.samples = 0;
        Ok(vec![
            Format::new(Kind::typed::<AisBurst>(), 0.0),
            Format::new(Kind::typed::<ChannelLevel>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.samples += x.len() as u64;
        let cuts = &mut self.cuts;
        self.det.process(x, |c| cuts.push(c));
        let out = io.items_out::<AisBurst>(0);
        for c in self.cuts.drain(..) {
            out.push(Timed {
                time: c.first as f64 / demod::RATE,
                value: AisBurst(Arc::new(c)),
            });
        }
        if io.tick() {
            let lv = ChannelLevel {
                channel: self.index as u8,
                freq_hz: CHANNELS[self.index].2,
                level: self.det.level,
                floor: self.det.floor(),
                detected: self.det.detected,
                clock_s: self.samples as f64 / demod::RATE,
            };
            io.items_out::<ChannelLevel>(1).push(Timed {
                time: lv.clock_s,
                value: lv,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// Collision handling.
pub const COLLISION_CANCEL: Setting<field::Bool> =
    Setting::new("collision_cancel", "Collision handling", "", true)
        .help("Tries again on other frequencies and after cancelling a stronger burst.");

sdrkit_core::settings! {
    /// The settings of [`BurstDecoder`].
    pub enum DecoderSetting {
        /// Satellite mode.
        Satellite = SATELLITE,
        /// Collision handling.
        CollisionCancel = COLLISION_CANCEL,
    }
}

/// The burst decoder (`ais.decoder`).
pub struct BurstDecoder {
    index: usize,
    dec: Decoder,
    opts: DecodeOptions,
}

impl BurstDecoder {
    /// For channel 0 to 3.
    pub fn new(index: usize) -> Self {
        BurstDecoder {
            index,
            dec: Decoder::default(),
            opts: DecodeOptions {
                collision: true,
                ..Default::default()
            },
        }
    }
}

impl Component for BurstDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ais.decoder",
            vec![
                Port::typed::<AisBurst>("bursts"),
                Port::typed::<AisHints>("hints"),
            ],
            vec![Port::typed::<AisDecoded>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DecoderSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<AisDecoded>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(h) = io.items::<AisHints>(1).last() {
            self.opts.hints = h.value.0[self.index].clone();
        }
        let bursts = io.items::<AisBurst>(0);
        for b in bursts {
            self.opts.noise_scale = 1.0;
            let c = &b.value.0;
            let r = self.dec.decode(c, &self.opts);
            let (a, e) = c.core;
            let power = c.samples[a..e]
                .iter()
                .map(|s| f64::from(s.norm_sqr()))
                .sum::<f64>()
                / e.saturating_sub(a).max(1) as f64;
            io.items_out::<AisDecoded>(0).push(Timed {
                time: b.time,
                value: AisDecoded {
                    channel: self.index as u8,
                    frames: r.frames,
                    bad: r.bad,
                    core_s: (
                        (c.first + a as u64) as f64 / demod::RATE,
                        (c.first + e as u64) as f64 / demod::RATE,
                    ),
                    power,
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::Satellite => {
                self.opts.satellite = SATELLITE.read(id, v)?;
                self.dec.set_satellite(self.opts.satellite);
            }
            DecoderSetting::CollisionCancel => {
                self.opts.collision = COLLISION_CANCEL.read(id, v)?
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::Satellite, Value::Bool(self.opts.satellite)),
            (
                DecoderSetting::CollisionCancel,
                Value::Bool(self.opts.collision),
            ),
        ])
    }
}

// The message layer.

/// The message layer (`ais.messages`): frames to messages, `!AIVDM` sentences and receptions, released in time order.
#[derive(Default)]
pub struct MessageLayer {
    seq: u8,
    pending: Vec<Timed<AisDecoded>>,
    ready: Vec<Timed<AisDecoded>>,
    /// Bursts with frames lately released: channel, core start and end (seconds), power.
    heard: Vec<(u8, f64, f64, f64)>,
    ticks: u64,
}

/// A failed burst this far (40 dB) under a decoded burst on the other channel of its pair, at the same time, is that burst's
/// spill into the channel (its own edges and what the filters let through), not a frame of its own.
const SPILL_RATIO: f64 = 1e4;

/// True if the failed burst `d` lies under a much stronger burst with frames on the other channel of its pair, in `heard`.
fn spill(d: &AisDecoded, heard: &[(u8, f64, f64, f64)]) -> bool {
    let (a, e) = d.core_s;
    heard
        .iter()
        .any(|&(ch, s, t, p)| ch == d.channel ^ 1 && s < e && a < t && p >= SPILL_RATIO * d.power)
}

impl Component for MessageLayer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ais.messages",
            (0..4)
                .map(|i| Port::typed::<AisDecoded>(&format!("in{i}")))
                .collect(),
            vec![
                Port::typed::<AisFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<AisMsg>("messages").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<NmeaSentence>("nmea").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<AisLayerEvent>("receptions").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.pending.clear();
        self.heard.clear();
        self.ticks = 0;
        Ok(vec![
            Format::new(Kind::typed::<AisFrame>(), 0.0),
            Format::new(Kind::typed::<AisMsg>(), 0.0),
            Format::new(Kind::typed::<NmeaSentence>(), 0.0),
            Format::new(Kind::typed::<AisLayerEvent>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for i in 0..4 {
            self.pending
                .extend(io.items::<AisDecoded>(i).iter().cloned());
        }
        // Frames are released in time order at the ticks, once every burst that started before them has been decoded (a burst is
        // decoded at most 160 ms after its cut starts), so the order does not depend on how the input is cut.
        if !io.tick() {
            return;
        }
        self.ticks += 1;
        let now = self.ticks as f64 * REPORT_S;
        self.ready.clear();
        let mut k = 0;
        while k < self.pending.len() {
            if self.pending[k].time < now - 0.2 {
                self.ready.push(self.pending.swap_remove(k));
            } else {
                k += 1;
            }
        }
        self.ready.sort_by(|a, b| {
            a.time
                .total_cmp(&b.time)
                .then(a.value.channel.cmp(&b.value.channel))
        });
        // The bursts with frames a failed one is checked against: those of the last two seconds and those still waiting.
        self.heard.retain(|h| h.2 > now - 2.0);
        for d in self.ready.iter().chain(&self.pending) {
            let v = &d.value;
            if !v.frames.is_empty()
                && !self
                    .heard
                    .iter()
                    .any(|h| (h.0, h.1) == (v.channel, v.core_s.0))
            {
                self.heard
                    .push((v.channel, v.core_s.0, v.core_s.1, v.power));
            }
        }
        for d in std::mem::take(&mut self.ready) {
            let ch = CHANNELS[d.value.channel as usize].0;
            let bad = |io: &mut Io, time| {
                io.items_out::<AisLayerEvent>(3).push(Timed {
                    time,
                    value: AisLayerEvent::Bad {
                        channel: d.value.channel,
                    },
                })
            };
            if d.value.frames.is_empty() {
                if d.value.bad && !spill(&d.value, &self.heard) {
                    bad(io, d.time);
                }
                continue;
            }
            for f in &d.value.frames {
                let t = f.start_index / demod::RATE;
                let bits = Arc::new(f.bits.clone());
                io.items_out::<AisFrame>(0).push(Timed {
                    time: t,
                    value: AisFrame {
                        channel: ch,
                        bits: bits.clone(),
                        snr_db: f.snr_db,
                        offset_hz: f.offset_hz,
                        layer: f.layer,
                    },
                });
                let Ok(m) = message::decode(&bits) else {
                    bad(io, t);
                    continue;
                };
                let m = Arc::new(m);
                let mut lines = Vec::new();
                // A frame of at most 150 bytes fits nine sentences, so this cannot fail.
                let _ = message::sentences(&bits, ch, &mut self.seq, &mut lines);
                for l in &lines {
                    io.items_out::<NmeaSentence>(2).push(Timed {
                        time: t,
                        value: NmeaSentence(l.clone()),
                    });
                }
                io.items_out::<AisMsg>(1).push(Timed {
                    time: t,
                    value: AisMsg(m.clone()),
                });
                let r = Reception {
                    time: t,
                    channel: ch,
                    snr_db: f.snr_db,
                    offset_hz: f.offset_hz,
                    layer: f.layer,
                    message: m,
                    sentences: lines,
                };
                io.items_out::<AisLayerEvent>(3).push(Timed {
                    time: t,
                    value: AisLayerEvent::Reception(Arc::new(r)),
                });
            }
        }
    }
}

// The graph.

/// The AIS components (types `ais.*`), so an AIS graph can be built from its description.
pub fn register(r: &mut Registry) {
    r.add("ais.pair", || Box::new(PairFrontEnd::new(0)));
    r.add("ais.channel", || Box::new(ChannelFilter::new(0)));
    r.add("ais.detector", || Box::new(BurstDetector::new(0)));
    r.add("ais.decoder", || Box::new(BurstDecoder::new(0)));
    r.add("ais.messages", || Box::new(MessageLayer::default()));
    r.add("ais.stations", || Box::new(Stations::default()));
    r.add("ais.no_hints", || Box::new(NoHints));
}

/// The front of the AIS graph (pairs, channels, detectors, decoders, message layer) added to `b`, with the input fed to both pairs.
/// The message layer is `messages`.
pub fn add_front(mut b: GraphBuilder, prefix: &str) -> GraphBuilder {
    let id = |s: &str| format!("{prefix}{s}");
    b = b
        .add(&id("pair_ab"), PairFrontEnd::new(0))
        .add(&id("pair_cd"), PairFrontEnd::new(1))
        .add(&id("messages"), MessageLayer::default())
        .input(&format!("{}.in", id("pair_ab")))
        .input(&format!("{}.in", id("pair_cd")));
    for (i, (l, _, _)) in CHANNELS.iter().enumerate() {
        let l = l.to_ascii_lowercase();
        let pair = if i < 2 { id("pair_ab") } else { id("pair_cd") };
        b = b
            .add(&id(&format!("chan_{l}")), ChannelFilter::new(i))
            .add(&id(&format!("detect_{l}")), BurstDetector::new(i))
            .add(&id(&format!("decode_{l}")), BurstDecoder::new(i))
            .connect(
                &format!("{pair}.out"),
                &format!("{}.in", id(&format!("chan_{l}"))),
            )
            .connect(
                &format!("{}.out", id(&format!("chan_{l}"))),
                &format!("{}.in", id(&format!("detect_{l}"))),
            )
            .connect(
                &format!("{}.bursts", id(&format!("detect_{l}"))),
                &format!("{}.bursts", id(&format!("decode_{l}"))),
            )
            .connect(
                &format!("{}.out", id(&format!("decode_{l}"))),
                &format!("{}.in{i}", id("messages")),
            );
    }
    b
}

/// The AIS receiver graph.
pub fn builder() -> GraphBuilder {
    let mut b = add_front(GraphBuilder::new(), "").add("stations", Stations::default());
    b = b
        .connect("messages.receptions", "stations.receptions")
        .input("stations.iq");
    for (i, (l, _, _)) in CHANNELS.iter().enumerate() {
        let l = l.to_ascii_lowercase();
        b = b
            .connect(&format!("detect_{l}.level"), &format!("stations.level{i}"))
            .feedback("stations.hints", &format!("decode_{l}.hints"));
    }
    b.output("stations.events")
        .output("stations.requests")
        .status_from("stations")
        .clock(REPORT_S)
        .reconfigure_on_retune()
}

/// The channels decoded.
pub const CHANNELS_SETTING: Setting<field::Choice<ChannelLimit>> =
    Setting::choice("channels", "Channels", ChannelLimit::All)
        .help("Which AIS channels are decoded.");

/// The settings of the AIS stack: its own, the display's, then the test signal's.
pub fn schema() -> Arc<Schema> {
    static CELL: OnceLock<Arc<Schema>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(crate::typed::shape("ais").schema()))
        .clone()
}

/// The AIS stack graph: the settings routed to the components that own them, and the station commands.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let chans = ["a", "b", "c", "d"];
    let list = |f: fn(&str) -> String| chans.iter().map(|c| f(c)).collect::<Vec<String>>();
    let mut sat = list(|c| format!("chan_{c}.satellite"));
    sat.extend(list(|c| format!("decode_{c}.satellite")));
    sat.push("stations.satellite".into());
    let cc = list(|c| format!("decode_{c}.collision_cancel"));
    let sat: Vec<&str> = sat.iter().map(String::as_str).collect();
    let cc: Vec<&str> = cc.iter().map(String::as_str).collect();
    let mut g = StackGraph::of(crate::typed::shape("ais"), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "AIS needs a sample rate of at least 250 ksps")
        .route(
            CHANNELS_SETTING.key(),
            &["pair_ab.limit", "pair_cd.limit", "stations.limit"],
        )?
        .route(SATELLITE.key(), &sat)?
        .route(COLLISION_CANCEL.key(), &cc)?
        .command(
            "select",
            sdrkit_graph::CommandTarget::of("stations", "select"),
        )
        .command(
            "clear",
            sdrkit_graph::CommandTarget::of("stations", "clear"),
        )
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("stations", "remove"),
        );
    for s in &StationsSetting::SETTINGS[1..] {
        let key = s.key();
        if key != SATELLITE.key() {
            g = g.route(key, &[&format!("stations.{key}")])?;
        }
    }
    g.with_settings(&cfg.settings).build()
}

/// The AIS graph with no settings applied.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder().catalog(catalog).build()
}
