//! Z-Wave (`zwave`): every channel of the `channels` setting in the capture, MPDUs at R1, R2 and R3, and tables of networks, nodes and commands.
//! Security S0 and S2 payloads are shown as encrypted and never decoded.
//!
//! Standard: ITU-T G.9959 (PHY and MAC)

pub mod frame;

use crate::family::{at_least_value, hex, min_rate_message, wall, CleanScene, REPORT_S};
use crate::typed::{
    ZwaveChannelRow, ZwaveClassRow, ZwaveFields, ZwaveFrameEvent, ZwaveFrameRow, ZwaveNetworkRow,
    ZwaveNodeRow, ZwaveStateEvent, ZWAVE_CHANNELS, ZWAVE_CLASSES, ZWAVE_FRAME, ZWAVE_FRAMES,
    ZWAVE_NETWORKS, ZWAVE_NODES, ZWAVE_NODE_ROW, ZWAVE_REMOVE, ZWAVE_STATE,
};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::fsk::TonePair;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, StateBlob, StateError, Status, TableId, Tuning, Value, Values, VerifyCase,
    VerifyHooks, VerifyOptions,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Graph, GraphBuilder,
    GraphError, Io, Item, Kept, Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_bits::line::{Manchester, ManchesterConv};
use sdrkit_util_bits::pack::{pack, BitOrder};
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::Tables as _;
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tally};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::Arc;

/// The least sample rate of the stack.
pub const MIN_RATE_HZ: f64 = 1e6;
/// Frames kept in the log.
pub const FRAMES_CAP: usize = 600;
/// Networks kept.
pub const NETWORKS_CAP: usize = 64;
/// Nodes kept.
pub const NODES_CAP: usize = 1000;
/// Command classes counted one by one.
pub const CLASSES_CAP: usize = 256;

/// The channels of the `channels` setting: entries separated by ";", each the frequency in Hz and the rates on it (digits
/// 1 to 3), separated by "/". Returns (frequency, rate) for each rate of each entry, in order; `None` for a malformed
/// list or more than 64 entries. An empty text is an empty list.
pub fn parse_channels(text: &str) -> Option<Vec<(f64, u8)>> {
    let mut out = Vec::new();
    let entries: Vec<&str> = text
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect();
    if entries.len() > 64 {
        return None;
    }
    for e in entries {
        let (hz, rates) = e.split_once('/')?;
        let hz: f64 = hz.trim().parse().ok()?;
        let rates = rates.trim();
        if !(hz.is_finite() && hz > 0.0) || rates.is_empty() {
            return None;
        }
        for c in rates.chars() {
            let r = match c {
                '1' => 1u8,
                '2' => 2,
                '3' => 3,
                _ => return None,
            };
            out.push((hz, r));
        }
    }
    Some(out)
}

/// The channels in use: the list set, or one channel at `centre` with the three rates when the list is empty.
fn channels_or_centre(list: &[(f64, u8)], centre: f64) -> Vec<(f64, u8)> {
    if list.is_empty() {
        vec![(centre, 1), (centre, 2), (centre, 3)]
    } else {
        list.to_vec()
    }
}

/// A group of channels sharing a front end: frequency and the rates its channels use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// Frequency (Hz).
    pub hz: f64,
    /// Rates (1, 2, 3), in order.
    pub rates: Vec<u8>,
}

/// The groups of the channels (frequency, rate) within ±0.45 `rate` of `centre`: channels within 60 kHz share a group at
/// the frequency of the first of them.
pub fn groups(channels: &[(f64, u8)], centre: f64, rate: f64) -> Vec<Group> {
    let mut g: Vec<Group> = Vec::new();
    for &(f, r) in channels {
        if (f - centre).abs() >= 0.45 * rate {
            continue;
        }
        match g.iter_mut().find(|x| (x.hz - f).abs() <= 60e3) {
            Some(x) => {
                if !x.rates.contains(&r) {
                    x.rates.push(r);
                    x.rates.sort();
                }
            }
            None => g.push(Group {
                hz: f,
                rates: vec![r],
            }),
        }
    }
    g
}

/// A burst from one group (`zw.burst`).
#[derive(Clone, Debug, Default)]
pub struct ZwBurst {
    /// Samples at `rate`, mixed to the group frequency.
    pub samples: Vec<Complex32>,
    /// The sample rate.
    pub rate: f64,
    /// The group frequency, from the input centre.
    pub group_offset_hz: f64,
    /// The group frequency (absolute when the centre is known).
    pub group_hz: f64,
    /// The rates the group tries.
    pub rates: Vec<u8>,
    /// The index of the burst start in `samples`.
    pub start: usize,
    /// The noise power at the burst start.
    pub noise: f32,
    /// Signal time, seconds.
    pub time: f64,
}

impl Item for ZwBurst {
    const KIND: &'static str = "zw.burst";
}

/// Samples of pre-roll a burst keeps (the newest sample included).
const PRE_ROLL: usize = 206;
/// A burst starts when the smoothed power exceeds this many times the noise floor (near the decoding threshold the signal is
/// only about 3.5 times the floor); the frame checks drop the bursts that noise opens.
const BURST_START: f32 = 3.0;
/// A burst ends when the smoothed power stays under this many times the floor measured at its start.
const BURST_END: f32 = 1.8;
/// A burst is decoded only when its open part averages this many times the floor at its start (a frame at the decoding
/// threshold averages about 3.5; noise about 1).
const BURST_MEAN: f32 = 2.0;

/// The front end and burst detector of one channel group (spec 6.3).
struct GroupChain {
    g: Group,
    nco: Nco,
    fir: FirDecimator<Complex32>,
    rate: f64,
    mixed: Vec<Complex32>,
    dec: Vec<Complex32>,
    pe: f32,
    nz: f32,
    nz_acc: f64,
    n: u64,
    ring: SampleRing<Complex32>,
    pre: Vec<Complex32>,
    above: u32,
    open: bool,
    buf: Vec<Complex32>,
    below: u32,
    start_noise: f32,
    t0: f64,
}

impl GroupChain {
    fn new(g: Group, rate_in: f64, centre: f64) -> Option<GroupChain> {
        let d = ((rate_in / 400e3).round() as usize).max(1);
        let taps = design_by_taps(8 * d + 1, (110e3 / rate_in).min(0.5), Window::Hamming).ok()?;
        Some(GroupChain {
            nco: Nco::new(rate_in, centre - g.hz),
            fir: FirDecimator::new(&taps, d),
            rate: rate_in / d as f64,
            mixed: Vec::new(),
            dec: Vec::new(),
            g,
            pe: 0.0,
            nz: 0.0,
            nz_acc: 0.0,
            n: 0,
            ring: SampleRing::new(256).ok()?,
            pre: vec![Complex32::default(); PRE_ROLL],
            above: 0,
            open: false,
            buf: Vec::new(),
            below: 0,
            start_noise: 0.0,
            t0: 0.0,
        })
    }

    fn feed(&mut self, x: &[Complex32], centre: f64, out: &mut Vec<ZwBurst>) {
        self.mixed.resize(x.len(), Complex32::default());
        self.nco.mix(x, &mut self.mixed);
        self.dec.clear();
        self.fir.process(&self.mixed, &mut self.dec);
        for k in 0..self.dec.len() {
            let y = self.dec[k];
            self.detect(y, centre, out);
        }
    }

    /// The burst detector of spec 6.3, one decimated sample at a time.
    fn detect(&mut self, y: Complex32, centre: f64, out: &mut Vec<ZwBurst>) {
        let p = y.norm_sqr();
        self.pe += 0.2 * (p - self.pe);
        self.n += 1;
        self.ring.push(y);
        if self.n <= 2000 {
            self.nz_acc += self.pe as f64;
            self.nz = (self.nz_acc / self.n as f64) as f32;
            return;
        }
        self.nz = self.nz.max(1e-12);
        if self.open {
            self.buf.push(y);
            if self.pe < BURST_END * self.start_noise {
                self.below += 1;
            } else {
                self.below = 0;
            }
            if self.below >= 160 || self.buf.len() as f64 > 0.25 * self.rate {
                self.open = false;
                // The open part (after the pre-roll, before the quiet tail) must hold a signal on average: noise opens
                // bursts at the lower gates but does not keep their mean up.
                let body = &self.buf[PRE_ROLL.min(self.buf.len())
                    ..self.buf.len().saturating_sub(self.below as usize)];
                let mean =
                    body.iter().map(|z| z.norm_sqr()).sum::<f32>() / body.len().max(1) as f32;
                if self.buf.len() >= 400 && mean >= BURST_MEAN * self.start_noise {
                    out.push(ZwBurst {
                        samples: std::mem::take(&mut self.buf),
                        rate: self.rate,
                        group_offset_hz: self.g.hz - centre,
                        group_hz: self.g.hz,
                        rates: self.g.rates.clone(),
                        start: PRE_ROLL - 12,
                        noise: self.start_noise,
                        time: self.t0,
                    });
                }
                self.buf.clear();
            }
            return;
        }
        if self.pe < 3.0 * self.nz {
            self.nz += 0.002 * (self.pe - self.nz);
        }
        if self.pe > BURST_START * self.nz {
            self.above += 1;
            if self.above >= 6 {
                self.open = true;
                self.above = 0;
                self.below = 0;
                self.start_noise = self.nz;
                self.ring
                    .copy(self.ring.end() - PRE_ROLL as u64, &mut self.pre);
                self.buf.clear();
                self.buf.extend_from_slice(&self.pre);
                self.t0 = (self.n as f64 - PRE_ROLL as f64) / self.rate;
            }
        } else {
            self.above = 0;
        }
    }
}

/// `zw.groups`: one front end and burst detector for each channel group.
#[derive(Default)]
pub struct Groups {
    channels: Vec<(f64, u8)>,
    channels_text: String,
    chains: Vec<GroupChain>,
    centre: f64,
    plan: Option<Plan>,
    out: Vec<ZwBurst>,
}

/// What the tables need to know about the channels (`zw.plan`): sent once after each configure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Every channel in use: (frequency, rate, covered).
    pub channels: Vec<(f64, u8, bool)>,
}

impl Item for Plan {
    const KIND: &'static str = "zw.plan";
}

/// The channels.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "").help(
    "Entries separated by \";\", each the frequency in Hz and the rates on it (1, 2, 3) separated by \"/\", for example \
     \"868420000/1;868400000/2;869850000/3\". At most 64 entries. Empty: one channel at the tuned frequency with the three \
     rates.",
);

/// Reads [`CHANNELS`] from `v`: the text and its channels.
fn read_channels(id: SettingId, v: &Value) -> Result<(String, Vec<(f64, u8)>), SettingError> {
    let t = CHANNELS.read(id, v)?;
    let list = parse_channels(t).ok_or(SettingError::Malformed { id, at: None })?;
    Ok((t.to_owned(), list))
}

sdrkit_core::settings! {
    /// The settings of [`Groups`].
    pub enum GroupsSetting {
        /// The channels.
        Channels = CHANNELS,
    }
}

impl Component for Groups {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "zw.groups",
            vec![Port::iq("in")],
            vec![
                Port::typed::<ZwBurst>("bursts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Plan>("plan").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(GroupsSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.centre = f.centre_hz;
        let channels = channels_or_centre(&self.channels, self.centre);
        self.chains = groups(&channels, self.centre, f.rate)
            .into_iter()
            .filter_map(|g| GroupChain::new(g, f.rate, self.centre))
            .collect();
        let plan = Plan {
            channels: channels
                .into_iter()
                .map(|(hz, rt)| (hz, rt, (hz - self.centre).abs() < 0.45 * f.rate))
                .collect(),
        };
        self.plan = Some(plan);
        let mut o = f.with_kind(Kind::typed::<ZwBurst>());
        o.rate = 0.0;
        Ok(vec![o, Format::new(Kind::typed::<Plan>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let c = self.centre;
        for ch in self.chains.iter_mut() {
            ch.feed(x, c, &mut self.out);
        }
        if !self.out.is_empty() {
            let items = io.items_out::<ZwBurst>(0);
            for b in self.out.drain(..) {
                items.push(Timed {
                    time: b.time,
                    value: b,
                });
            }
        }
        if let Some(plan) = self.plan.take() {
            io.items_out::<Plan>(1).push(Timed {
                time: 0.0,
                value: plan,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match GroupsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GroupsSetting::Channels => {
                (self.channels_text, self.channels) = read_channels(id, v)?;
            }
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(
            GroupsSetting::Channels,
            Value::Text(self.channels_text.clone()),
        )])
    }
}

/// One decoded MPDU (`zw.mpdu`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZwFrame {
    /// The rate: 1, 2 or 3.
    pub rate: u8,
    /// The channel frequency.
    pub channel_hz: f64,
    /// The MPDU bytes.
    pub bytes: Vec<u8>,
    /// The level, dBFS.
    pub level_db: f32,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// Signal time, seconds.
    pub time: f64,
}

impl Item for ZwFrame {
    const KIND: &'static str = "zw.mpdu";
}

/// The outcome of decoding a burst.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ZwOutcome {
    /// A frame.
    Frame(ZwFrame),
    /// A burst with a plausible start of frame and length but no check passing, at these rates, on this group frequency.
    FailedCheck(Vec<u8>, f64),
    /// Nothing.
    Burst,
}

impl Item for ZwOutcome {
    const KIND: &'static str = "zw.outcome";
}

/// The R1 bit streams of hard chips at both chip alignments. Manchester with 1 as chips 10 (G.9959 R1): a pair without a change
/// (00 or 11) is refused, so it ends a run of bits. Each run is one stream.
pub fn r1_streams(chips: &[u8]) -> Vec<Vec<u8>> {
    let mut streams = Vec::new();
    for align in 0..2 {
        let mut at = align;
        while at + 1 < chips.len() {
            let mut bits = Vec::new();
            match Manchester::decode(&chips[at..], ManchesterConv::Thomas, &mut bits) {
                Ok(()) => at = chips.len(),
                Err(sdrkit_util_bits::Error::Invalid { at: pair }) => at += 2 * pair + 2,
                Err(_) => at = chips.len(),
            }
            if bits.len() >= 21 {
                streams.push(bits);
            }
        }
    }
    streams
}

/// Decodes one burst (spec 6.4). `sof_any` takes 0xF0 as the start of frame at R3 too.
pub fn decode_burst(b: &ZwBurst, sof_any: bool) -> ZwOutcome {
    let n = b.samples.len();
    let rate = b.rate;
    let mut f = vec![0f32; n];
    if let (Ok(mut disc), Some(&first)) = (Discriminator::try_new(rate, 1.0), b.samples.first()) {
        disc.set_last(first);
        disc.process_into(&b.samples, &mut f);
    }
    // The preamble alternates, so its mean frequency is the carrier.
    let a = (b.start + (0.4e-3 * rate) as usize).min(n);
    let e = (b.start + (2.4e-3 * rate) as usize).min(n);
    let off = if e > a {
        f[a..e].iter().sum::<f32>() / (e - a) as f32
    } else {
        0.0
    };
    for v in f.iter_mut() {
        *v -= off;
    }
    let lvl_b = n.saturating_sub(160).max(a + 1).min(n);
    let level = b.samples[a.min(lvl_b - 1)..lvl_b]
        .iter()
        .map(|s| s.norm_sqr() as f64)
        .sum::<f64>()
        / (lvl_b - a.min(lvl_b - 1)).max(1) as f64;
    let snr = (10.0 * (level / b.noise.max(1e-20) as f64 - 1.0).max(1e-3).log10()).max(-30.0);
    let mut plausible = false;
    let mut avg = vec![0f32; n];
    let mut bytes = Vec::new();
    let found = |r: u8, m: &[u8]| ZwFrame {
        rate: r,
        channel_hz: b.group_hz,
        bytes: m.to_vec(),
        level_db: (10.0 * level.max(1e-20).log10()) as f32,
        snr_db: snr as f32,
        offset_hz: off as f64,
        time: b.time,
    };
    for &r in &b.rates {
        let (chip_rate, dev) = match r {
            1 => (19200.0, 20e3),
            2 => (40000.0, 20e3),
            _ => (100_000.0, 29e3),
        };
        let spc = rate / chip_rate;
        let w = spc.round().max(1.0) as usize;
        // Chip values from the tone energies over one chip, scaled to Hz; when they give no frame, the mean of the discriminator.
        if tone_chip_values(&b.samples, rate, off as f64, dev, w, &mut avg) {
            if let Some(m) =
                chips_to_frame(&avg, b.start, spc, r, sof_any, &mut plausible, &mut bytes)
            {
                return ZwOutcome::Frame(found(r, &m));
            }
        }
        let Ok(mut boxcar) = WindowSum::<f32>::new(w) else {
            continue;
        };
        for k in 0..n {
            avg[k] = (boxcar.push(f[k]) / w as f64) as f32;
        }
        if let Some(m) = chips_to_frame(&avg, b.start, spc, r, sof_any, &mut plausible, &mut bytes)
        {
            return ZwOutcome::Frame(found(r, &m));
        }
    }
    if plausible {
        ZwOutcome::FailedCheck(b.rates.clone(), b.group_hz)
    } else {
        ZwOutcome::Burst
    }
}

/// The tone-energy chip values of a burst into `out` (the length of `samples`): at each sample, the soft value of the tones at
/// `centre_hz +- dev_hz` over the `w` samples ending there, times `dev_hz` (so the values read as a discriminator mean in Hz).
/// `false` when the tones cannot be set up.
fn tone_chip_values(
    samples: &[Complex32],
    rate: f64,
    centre_hz: f64,
    dev_hz: f64,
    w: usize,
    out: &mut [f32],
) -> bool {
    let Ok(mut pair) = TonePair::new(rate, centre_hz, dev_hz, w) else {
        return false;
    };
    for (k, (&z, o)) in samples.iter().zip(out.iter_mut()).enumerate() {
        pair.push(z);
        *o = pair.value(w.min(k + 1), 0).unwrap_or(0.0) * dev_hz as f32;
    }
    true
}

/// A frame from chip values `avg` (Hz, positive for the upper tone) of a burst whose preamble starts at sample `from`, at rate
/// `r` with `spc` samples a chip: the chip phase from the preamble, a Gardner timing tracker, both polarities, the start of
/// frame and the checks. Sets `plausible` when a start of frame and length fit but no check passes.
fn chips_to_frame(
    avg: &[f32],
    from: usize,
    spc: f64,
    r: u8,
    sof_any: bool,
    plausible: &mut bool,
    bytes: &mut Vec<u8>,
) -> Option<Vec<u8>> {
    let n = avg.len();
    // The chip phase in half-sample steps: the end of each chip's average window.
    let steps = (2.0 * spc).ceil() as usize;
    let mut best = (0.0f64, -1f32);
    for h in 0..steps {
        let ph = h as f64 / 2.0;
        let mut s = 0f32;
        for c in 0..16 {
            let i = (from as f64 + ph + (c as f64 + 1.0) * spc) as usize;
            if i < n {
                s += avg[i].abs();
            }
        }
        if s > best.1 {
            best = (ph, s);
        }
    }
    // One hard chip per chip time, with a Gardner timing tracker (chip clocks are a few hundred ppm off).
    let mut chips = Vec::new();
    let mut t = from as f64 + best.0 + spc;
    let mut dev = 0f32;
    let mut tt = t;
    for _ in 0..16 {
        if tt as usize >= n {
            break;
        }
        dev += avg[tt as usize].abs() / 16.0;
        tt += spc;
    }
    let dev = dev.max(1.0);
    let mut prev = f32::NAN;
    while (t as usize) < n {
        let a = avg[t as usize];
        chips.push((a > 0.0) as u8);
        if prev.is_finite() {
            let mid = (t - spc / 2.0).max(0.0) as usize;
            let e = (prev - a) * avg[mid] / (2.0 * dev * dev);
            t += (0.08 * spc * e as f64).clamp(-0.1 * spc, 0.1 * spc);
        }
        prev = a;
        t += spc;
    }
    for inv in [false, true] {
        let c: Vec<u8> = chips.iter().map(|&x| x ^ inv as u8).collect();
        let streams = if r == 1 { r1_streams(&c) } else { vec![c] };
        let sofs: &[u8] = match (r, sof_any) {
            (3, true) => &[0x5B, 0xF0],
            (3, false) => &[0x5B],
            _ => &[0xF0],
        };
        for bits in streams {
            for i in 12..bits.len().saturating_sub(8) {
                let v = bits[i..i + 8].iter().fold(0u8, |a, &x| (a << 1) | x);
                if !sofs.contains(&v) {
                    continue;
                }
                let non_alt = (i - 11..i - 1).filter(|&k| bits[k] == bits[k + 1]).count();
                if non_alt > 1 {
                    continue;
                }
                let start = i + 8;
                let avail = (bits.len() - start) / 8;
                if avail < 9 {
                    continue;
                }
                bytes.clear();
                pack(
                    &bits[start..start + avail * 8],
                    BitOrder::Msb,
                    sdrkit_util_bits::Pad::Drop,
                    bytes,
                );
                let l = bytes[7] as usize;
                let ck = if r == 3 { 2 } else { 1 };
                let max = if r == 3 { 170 } else { 64 };
                if l < 9 + ck || l > max || l > bytes.len() {
                    continue;
                }
                *plausible = true;
                let m = &bytes[..l];
                if frame::check(m, r) && frame::parse(m, r).is_some() {
                    return Some(m.to_vec());
                }
            }
        }
    }
    None
}

/// `zw.decode`.
#[derive(Default)]
pub struct Decode {
    /// Takes 0xF0 as the R3 start of frame (recordings made that way).
    pub sof_any: bool,
}

impl Component for Decode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "zw.decode",
            vec![Port::typed::<ZwBurst>("in")],
            vec![Port::typed::<ZwOutcome>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DecodeSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::typed::<ZwOutcome>())])
    }
    fn process(&mut self, io: &mut Io) {
        let v: Vec<Timed<ZwOutcome>> = io
            .items::<ZwBurst>(0)
            .iter()
            .map(|b| Timed {
                time: b.time,
                value: decode_burst(&b.value, self.sof_any),
            })
            .collect();
        if !v.is_empty() {
            io.items_out::<ZwOutcome>(0).extend(v);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecodeSetting::SofF0R3 => self.sof_any = SOF_F0_R3.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(DecodeSetting::SofF0R3, Value::Bool(self.sof_any))])
    }
}

const SOF_F0_R3: Setting<field::Bool> = Setting::new(
    "sof_f0_r3",
    "Accept 0xF0 as the R3 start of frame",
    "",
    false,
)
.help("For recordings made with the ISM-mode start of frame at every rate.");

sdrkit_core::settings! {
    /// The settings of [`Decode`].
    pub enum DecodeSetting {
        /// Accept 0xF0 as the R3 start of frame.
        SofF0R3 = SOF_F0_R3,
    }
}

/// A frame of the log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Signal time, seconds.
    pub at: f64,
    /// Wall-clock time, when the samples are live.
    pub wall: Option<f64>,
    /// The channel frequency.
    pub channel_hz: f64,
    /// The rate ("R1").
    pub rate: String,
    /// The home id.
    pub home: String,
    /// "src > dst".
    pub route: String,
    /// The header type name.
    pub kind: String,
    /// The sequence number.
    pub seq: u8,
    /// An acknowledgement is requested.
    pub ack_req: bool,
    /// "none", "S0" or "S2".
    pub encryption: String,
    /// The level, dBFS.
    pub level_db: f32,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The length octet.
    pub len: u8,
    /// The one-line description.
    pub describe: String,
    /// The MPDU in hex.
    pub hex: String,
}

impl Shift for LogEntry {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

impl Shaped for LogEntry {
    type Row = ZwaveFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ZwaveFrameRow {
            time_s: self.at,
            time: self.wall.map(Time),
            frequency: Hertz(self.channel_hz),
            rate: &self.rate,
            home_id: &self.home,
            route: &self.route,
            frame_type: &self.kind,
            seq: i64::from(self.seq),
            ack_requested: self.ack_req,
            encryption: &self.encryption,
            level_dbfs: f64::from(self.level_db),
            snr_db: f64::from(self.snr_db),
            command: &self.describe,
            mpdu: &self.hex,
        });
    }
}

/// A network (a home id).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
    /// The home id.
    pub home: String,
    /// Nodes heard.
    pub nodes: usize,
    /// Node 1 has sent.
    pub controller: bool,
    /// Encrypted frames.
    pub encrypted: u64,
    /// The level of the last frame, dBFS.
    pub level_db: f32,
}

impl Shift for Network {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Network {
    type Row = ZwaveNetworkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ZwaveNetworkRow {
            home_id: &self.home,
            nodes: self.nodes as i64,
            controller: self.controller,
            heard: columns::heard(seen),
            encrypted: i64::try_from(self.encrypted).unwrap_or(i64::MAX),
            level_dbfs: f64::from(self.level_db),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// A node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The home id.
    pub home: String,
    /// The node id.
    pub node: u16,
    /// "controller" or "node".
    pub role: String,
    /// Frames sent.
    pub sent: u64,
    /// Frames addressed to it.
    pub addressed: u64,
    /// Encrypted frames sent.
    pub encrypted: u64,
    /// The rate of the last frame.
    pub rate: String,
    /// The level of the last frame, dBFS.
    pub level_db: f32,
    /// The last command class sent.
    pub last_class: String,
    /// Beaming.
    pub beaming: bool,
    /// Low power.
    pub low_power: bool,
}

impl Shift for Node {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Node {
    type Row = ZwaveNodeRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ZwaveNodeRow {
            home_id: &self.home,
            node: i64::from(self.node),
            role: &self.role,
            sent: i64::try_from(self.sent).unwrap_or(i64::MAX),
            addressed: i64::try_from(self.addressed).unwrap_or(i64::MAX),
            encrypted: i64::try_from(self.encrypted).unwrap_or(i64::MAX),
            rate: &self.rate,
            level_dbfs: f64::from(self.level_db),
            last_heard: columns::last_heard(seen).map(Time),
            last_class: &self.last_class,
            beaming: self.beaming,
            low_power: self.low_power,
        });
    }
}

/// A channel in use with its counts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelRow {
    /// The frequency.
    pub hz: f64,
    /// The rate.
    pub rate: u8,
    /// In the band taken in.
    pub covered: bool,
    /// Frames decoded.
    pub frames: u64,
    /// Bursts whose check failed.
    pub failed: u64,
}

/// What the stack keeps through a restart and in the saved state.
#[derive(Serialize, Deserialize)]
pub struct Store {
    frames: LogRing<LogEntry>,
    networks: ObjectTable<String, Network>,
    nodes: ObjectTable<String, Node>,
    classes: Tally<String>,
    per_rate: [u64; 3],
    s0: u64,
    s2: u64,
    acks: u64,
    failed: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            frames: LogRing::new(FRAMES_CAP),
            networks: ObjectTable::new(NETWORKS_CAP),
            nodes: ObjectTable::new(NODES_CAP),
            classes: Tally::new(CLASSES_CAP),
            per_rate: [0; 3],
            s0: 0,
            s2: 0,
            acks: 0,
            failed: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.networks.shift_times(by_s);
        self.nodes.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.zwave", 1);
    fn clear(&mut self) {
        let fresh = Store::default();
        self.frames.clear();
        self.networks.clear();
        self.nodes.clear();
        self.classes = fresh.classes;
        self.per_rate = [0; 3];
        self.s0 = 0;
        self.s2 = 0;
        self.acks = 0;
        self.failed = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.networks.adopt(saved.networks);
        self.nodes.adopt(saved.nodes);
        self.classes = saved.classes;
        self.per_rate = saved.per_rate;
        self.s0 = saved.s0;
        self.s2 = saved.s2;
        self.acks = saved.acks;
        self.failed = saved.failed;
    }
}

/// `zw.tables`.
pub struct Tables {
    published: Published,
    nodes_table: TableId,
    channel_keys: Vec<String>,
    class_count: usize,
    store: Store,
    channels: Vec<ChannelRow>,
    tuned_hz: f64,
    snr: f32,
    level: f32,
    offset: f64,
    heard: bool,
    last_good: f64,
    last_burst: f64,
    now: f64,
    was_on: bool,
    live: bool,
    events: Vec<Event>,
}

impl Default for Tables {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(ZwaveFields::SHAPE.fields)
                .with_table(&ZWAVE_FRAMES)
                .with_table(&ZWAVE_NODES)
                .with_table(&ZWAVE_NETWORKS)
                .with_table(&ZWAVE_CHANNELS)
                .with_table(&ZWAVE_CLASSES),
        ));
        let nodes_table = published
            .results()
            .decl()
            .table_id(ZWAVE_NODES.key)
            .expect("the tables of zw.tables are declared");
        Tables {
            published,
            nodes_table,
            channel_keys: Vec::new(),
            class_count: 0,
            store: Store::default(),
            channels: Vec::new(),
            tuned_hz: 0.0,
            snr: 0.0,
            level: 0.0,
            offset: 0.0,
            heard: false,
            last_good: f64::NEG_INFINITY,
            last_burst: f64::NEG_INFINITY,
            now: 0.0,
            was_on: false,
            live: false,
            events: Vec::new(),
        }
    }
}

impl Tables {
    fn frame(&mut self, f: &ZwFrame) {
        let Some(m) = frame::parse(&f.bytes, f.rate) else {
            return;
        };
        let home = format!("{:08X}", m.home);
        let rate = format!("R{}", f.rate);
        let enc = m.encryption();
        let route = if m.header_type() == 2 {
            let list: Vec<String> = m.members.iter().map(|x| x.to_string()).collect();
            format!("{} > {{{}}}", m.src, list.join(", "))
        } else if m.dst == 0xFF {
            format!("{} > all", m.src)
        } else {
            format!("{} > {}", m.src, m.dst)
        };
        let desc = frame::describe(&m);
        let wall = wall(self.live);
        let at = match wall {
            Some(w) => Stamp::at(f.time).with_wall(w),
            None => Stamp::at(f.time),
        };
        self.store.frames.push(LogEntry {
            at: f.time,
            wall,
            channel_hz: f.channel_hz,
            rate: rate.clone(),
            home: home.clone(),
            route: route.clone(),
            kind: m.type_name().to_lowercase(),
            seq: m.seq(),
            ack_req: m.ack_req(),
            encryption: enc.into(),
            level_db: f.level_db,
            snr_db: f.snr_db,
            len: m.len,
            describe: desc.clone(),
            hex: hex(&f.bytes),
        });
        self.store.per_rate[(f.rate.clamp(1, 3) - 1) as usize] += 1;
        match enc {
            "S0" => self.store.s0 += 1,
            "S2" => self.store.s2 += 1,
            _ => {}
        }
        if m.header_type() == 3 {
            self.store.acks += 1;
        }
        self.snr = 0.8 * self.snr + 0.2 * f.snr_db;
        self.level = f.level_db;
        self.offset = f.offset_hz;
        self.heard = true;
        self.last_good = f.time;
        self.now = self.now.max(f.time);
        let target = (m.header_type() != 2 && m.dst != 0xFF).then(|| format!("{home}:{}", m.dst));
        self.events.push(ZWAVE_FRAME.event(
            f.time,
            f.channel_hz,
            &ZwaveFrameEvent {
                source: &format!("{home}:{}", m.src),
                target: target.as_deref(),
                text: &format!("Z-Wave {home} {route}: {desc}"),
                family: Family::Sensor,
                home_id: &home,
                route: &route,
                rate: &rate,
                frame_type: &m.type_name().to_lowercase(),
                encryption: enc,
                command: &desc,
                level_dbfs: f64::from(f.level_db),
                snr_db: f64::from(f.snr_db),
            },
        ));
        if let Ok(up) = self.store.networks.upsert(&home, at, Network::default) {
            up.row.home.clone_from(&home);
            up.row.level_db = f.level_db;
            if enc != "none" {
                up.row.encrypted += 1;
            }
            if m.src == 1 {
                up.row.controller = true;
            }
        }
        // The command class (not for acks and routed frames).
        let class = if m.header_type() == 3 || m.routed() {
            None
        } else {
            m.payload.first().map(|&c| {
                frame::class_name(c)
                    .map(String::from)
                    .unwrap_or(format!("0x{c:02X}"))
            })
        };
        if let Some(n) = self.node(&home, m.src as u16, at) {
            n.sent += 1;
            n.rate.clone_from(&rate);
            n.level_db = f.level_db;
            n.beaming = m.fc2 & 0x60 != 0;
            n.low_power = m.fc1 & 0x20 != 0;
            if enc != "none" {
                n.encrypted += 1;
            }
            if let Some(c) = &class {
                n.last_class.clone_from(c);
            }
        }
        if let Some(c) = class {
            self.store.classes.add(c);
        }
        let dests: Vec<u16> = if m.header_type() == 2 {
            m.members.clone()
        } else if (1..=232).contains(&m.dst) {
            vec![m.dst as u16]
        } else {
            vec![]
        };
        for d in dests {
            let key = format!("{home}:{d}");
            match self.store.nodes.get_mut(key.as_str()) {
                Some(n) => n.addressed += 1,
                None => {
                    if let Ok(up) = self.store.nodes.upsert(&key, at, || new_node(&home, d)) {
                        up.row.addressed += 1;
                    }
                }
            }
        }
        let nodes = self
            .store
            .nodes
            .iter()
            .filter(|r| r.record.home == home)
            .count();
        if let Some(n) = self.store.networks.get_mut(home.as_str()) {
            n.nodes = nodes;
        }
        for c in self.channels.iter_mut() {
            if c.rate == f.rate && (c.hz - f.channel_hz).abs() < 60e3 {
                c.frames += 1;
            }
        }
    }

    fn node(&mut self, home: &str, id: u16, at: Stamp) -> Option<&mut Node> {
        let key = format!("{home}:{id}");
        self.store
            .nodes
            .upsert(&key, at, || new_node(home, id))
            .ok()
            .map(|u| u.row)
    }

    fn state(&self) -> u8 {
        if self.now - self.last_good < 5.0 {
            2
        } else if self.now - self.last_burst < 10.0 {
            1
        } else {
            0
        }
    }

    fn publish(&mut self) {
        let state = self.state();
        let on = state == 2;
        if on != self.was_on {
            let text = if on {
                "Z-Wave frames are coming in"
            } else {
                "no Z-Wave frames for 5 s"
            };
            self.events.push(ZWAVE_STATE.event(
                self.now,
                self.tuned_hz,
                &ZwaveStateEvent {
                    text,
                    family: Family::Sensor,
                },
            ));
            self.published.results_mut().log(text.to_string());
            self.was_on = on;
        }
        let covered = self.channels.iter().filter(|c| c.covered).count();
        let total: u64 = self.store.per_rate.iter().sum();
        let mut summary = String::new();
        if total > 0 {
            let _ = write!(
                summary,
                "{} networks, {} nodes, {} frames",
                self.store.networks.len(),
                self.store.nodes.len(),
                total
            );
        } else if covered > 0 {
            let _ = write!(summary, "listening on {covered} Z-Wave channels");
        } else {
            summary.push_str("no Z-Wave channel in the band taken in");
        }
        let p = &mut self.published;
        {
            let r = p.results_mut();
            let heard = self.heard;
            let status = Status {
                lock_state: LockState::from_level(state),
                snr_db: heard.then_some(self.snr),
                level_dbfs: heard.then_some(self.level),
                offset_hz: heard.then_some(self.offset),
                summary,
            };
            r.set_status(&status);
        }
        let s = &self.store;
        p.put(&ZwaveFields {
            frames_ok: total as i64,
            frames_bad: s.failed as i64,
            networks: s.networks.len() as i64,
            nodes: s.nodes.len() as i64,
            frames_r1: s.per_rate[0] as i64,
            frames_r2: s.per_rate[1] as i64,
            frames_r3: s.per_rate[2] as i64,
            s0_frames: s.s0 as i64,
            s2_frames: s.s2 as i64,
            acks: s.acks as i64,
        });
        shaped::log_rows(&s.frames, &ZWAVE_FRAMES, self.now, p);
        shaped::rows(&s.nodes, &ZWAVE_NODES, self.now, p);
        shaped::rows(&s.networks, &ZWAVE_NETWORKS, self.now, p);
        if self.channel_keys.len() != self.channels.len() {
            p.clear_rows(&ZWAVE_CHANNELS);
            self.channel_keys = self
                .channels
                .iter()
                .map(|c| format!("{}:{}", c.hz, c.rate))
                .collect();
        }
        for (c, key) in self.channels.iter().zip(&self.channel_keys) {
            ZWAVE_CHANNELS.put(
                p,
                key,
                &ZwaveChannelRow {
                    frequency: Hertz(c.hz),
                    rate: &format!("R{}", c.rate),
                    covered: c.covered,
                    frames: i64::try_from(c.frames).unwrap_or(i64::MAX),
                    failed_checks: i64::try_from(c.failed).unwrap_or(i64::MAX),
                },
            );
        }
        let classes = s.classes.iter().count();
        if classes < self.class_count {
            p.clear_rows(&ZWAVE_CLASSES);
        }
        self.class_count = classes;
        for (name, n) in s.classes.iter() {
            ZWAVE_CLASSES.put(
                p,
                name,
                &ZwaveClassRow {
                    class: name,
                    count: i64::try_from(n).unwrap_or(i64::MAX),
                },
            );
        }
    }

    /// Takes the channel plan of the groups, and keeps the counts of channels that stay.
    fn plan(&mut self, p: &Plan) {
        self.channel_keys.clear();
        self.published.clear_rows(&ZWAVE_CHANNELS);
        let old = std::mem::take(&mut self.channels);
        self.channels = p
            .channels
            .iter()
            .map(|&(hz, rate, covered)| {
                let o = old.iter().find(|c| c.hz == hz && c.rate == rate);
                ChannelRow {
                    hz,
                    rate,
                    covered,
                    frames: o.map_or(0, |c| c.frames),
                    failed: o.map_or(0, |c| c.failed),
                }
            })
            .collect();
    }
}

fn new_node(home: &str, id: u16) -> Node {
    Node {
        home: home.into(),
        node: id,
        role: if id == 1 { "controller" } else { "node" }.into(),
        ..Default::default()
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "zw.tables",
            vec![Port::typed::<ZwOutcome>("in"), Port::typed::<Plan>("plan")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned_hz = inputs[0].tuned_hz;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for p in io.items::<Plan>(1) {
            self.plan(&p.value);
            new = true;
        }
        for o in io.items::<ZwOutcome>(0) {
            self.last_burst = o.time;
            self.now = self.now.max(o.time);
            match &o.value {
                ZwOutcome::Frame(f) => self.frame(f),
                ZwOutcome::FailedCheck(rates, hz) => {
                    self.store.failed += 1;
                    for c in self.channels.iter_mut() {
                        if rates.contains(&c.rate) && (c.hz - hz).abs() <= 60e3 {
                            c.failed += 1;
                        }
                    }
                }
                ZwOutcome::Burst => {}
            }
            new = true;
        }
        if io.tick() {
            self.now += REPORT_S;
        }
        if new || io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => {
                self.store.clear();
                for c in self.channels.iter_mut() {
                    c.frames = 0;
                    c.failed = 0;
                }
                self.published.clear_rows(&ZWAVE_FRAMES);
                self.published.clear_rows(&ZWAVE_NODES);
                self.published.clear_rows(&ZWAVE_NETWORKS);
                self.published.clear_rows(&ZWAVE_CLASSES);
                self.class_count = 0;
            }
            TablesCommand::Remove => commands::remove(
                &mut self.store.nodes,
                self.nodes_table,
                &mut self.published,
                args,
                &ZWAVE_NODE_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        let s = &mut self.store;
        if !kept.table(ZWAVE_FRAMES.key) {
            s.frames.clear();
            self.published.clear_rows(&ZWAVE_FRAMES);
        }
        if !kept.table(ZWAVE_NODES.key) {
            s.nodes.clear();
            self.published.clear_rows(&ZWAVE_NODES);
        }
        if !kept.table(ZWAVE_NETWORKS.key) {
            s.networks.clear();
            self.published.clear_rows(&ZWAVE_NETWORKS);
        }
        if !kept.table(ZWAVE_CLASSES.key) {
            s.classes = Tally::new(CLASSES_CAP);
            self.class_count = 0;
            self.published.clear_rows(&ZWAVE_CLASSES);
        }
        let [r1, r2, r3] = &mut s.per_rate;
        for (key, n) in [
            ("frames_r1", r1),
            ("frames_r2", r2),
            ("frames_r3", r3),
            ("frames_bad", &mut s.failed),
            ("s0_frames", &mut s.s0),
            ("s2_frames", &mut s.s2),
            ("acks", &mut s.acks),
        ] {
            if !kept.field(key) {
                *n = 0;
            }
        }
        for c in self.channels.iter_mut() {
            c.frames = 0;
            c.failed = 0;
        }
        self.heard = false;
        self.snr = 0.0;
        self.last_good = f64::NEG_INFINITY;
        self.last_burst = f64::NEG_INFINITY;
        self.publish();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one node.
        Remove = ZWAVE_REMOVE.command,
    }
}

/// Builds the `zwave` graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn build_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .catalog(catalog)
        .add("groups", Groups::default())
        .add("decode", Decode::default())
        .add("tables", Tables::default())
        .input("groups")
        .connect("groups.bursts", "decode")
        .connect("groups.plan", "tables.plan")
        .connect("decode", "tables.in")
        .output("tables.events")
        .clock(REPORT_S)
        .status_from("tables")
        .build()
}

sdrkit_core::settings! {
    /// The settings of the `zwave` stack.
    pub enum ZwaveSetting {
        /// The channels.
        Channels = CHANNELS,
        /// Accept 0xF0 as the R3 start of frame.
        SofF0R3 = SOF_F0_R3,
        /// The centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
    }
}

/// The `zwave` receiver.
///
/// # Errors
///
/// The errors of [`build_graph`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among them a
/// refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("zwave"),
        build_graph(&Catalog::of(cfg)?)?,
    )
    .min_rate(MIN_RATE_HZ, &min_rate_message("Z-Wave", MIN_RATE_HZ))
    .route(CHANNELS.key(), &["groups.channels"])?
    .route(SOF_F0_R3.key(), &["decode.sof_f0_r3"])?
    .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
    .command(
        "remove",
        sdrkit_graph::CommandTarget::of("tables", "remove"),
    )
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The generator schedule: (rate, MPDU) of each frame, in order (spec 6.8).
pub fn schedule() -> Vec<(u8, Vec<u8>)> {
    use frame::build;
    let h = 0xC0FFEE01;
    // The encrypted payloads are random bytes after the encapsulation header.
    let mut rng = Rng::new(0x5EC);
    let mut s0 = vec![0x98, 0x81];
    s0.extend((0..20).map(|_| rng.below(256) as u8));
    let mut s2 = vec![0x9F, 0x03];
    s2.extend((0..16).map(|_| rng.below(256) as u8));
    vec![
        (2, build(h, 1, 0x41, 1, &[2], &[0x25, 0x01, 0xFF], 2)),
        (2, build(h, 2, 0x03, 1, &[1], &[], 2)),
        (2, build(h, 1, 0x41, 2, &[2], &[0x25, 0x02], 2)),
        (2, build(h, 2, 0x01, 3, &[1], &[0x25, 0x03, 0xFF], 2)),
        (1, build(h, 1, 0x41, 4, &[3], &[0x26, 0x01, 0x32], 1)),
        (1, build(h, 3, 0x03, 4, &[1], &[], 1)),
        (
            2,
            build(
                h,
                4,
                0x01,
                5,
                &[1],
                &[0x31, 0x05, 0x01, 0x22, 0x00, 0xDC],
                2,
            ),
        ),
        (1, build(h, 5, 0x01, 6, &[1], &[0x80, 0x03, 0x5A], 1)),
        (2, build(h, 5, 0x01, 7, &[1], &[0x84, 0x07], 2)),
        (
            2,
            build(
                h,
                6,
                0x01,
                8,
                &[0xFF],
                &[0x01, 0x01, 0xD3, 0x9C, 0x00, 0x04, 0x10, 0x01],
                2,
            ),
        ),
        (2, build(h, 1, 0x41, 9, &[2], &[0x00], 2)),
        (
            2,
            build(h, 1, 0x02, 10, &[0x01, 0x16], &[0x25, 0x01, 0x00], 2),
        ),
        (2, build(h, 1, 0x41, 11, &[2], &s0, 2)),
        (2, build(h, 3, 0x01, 12, &[1], &s2, 2)),
        (3, build(h, 1, 0x41, 13, &[2], &[0x20, 0x01, 0xFF], 3)),
        (3, build(h, 2, 0x03, 13, &[1], &[], 3)),
        (
            1,
            build(
                h,
                2,
                0x01,
                14,
                &[1],
                &[0x71, 0x05, 0x00, 0x00, 0x00, 0xFF, 0x06, 0x16],
                1,
            ),
        ),
    ]
}

/// The chips of one frame at rate `r`: the preamble octets (0x55, 10 of them, 24 at R3), the start of frame, the MPDU, and two
/// extra chips that repeat the last one. R1 is Manchester with 1 as chips 10.
pub fn frame_chips(r: u8, mpdu: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x55u8; if r == 3 { 24 } else { 10 }];
    bytes.push(if r == 3 { 0x5B } else { 0xF0 });
    bytes.extend_from_slice(mpdu);
    let mut bits = Vec::new();
    sdrkit_util_bits::pack::unpack(&bytes, BitOrder::Msb, &mut bits);
    let mut chips = Vec::with_capacity(2 * bits.len() + 2);
    if r == 1 {
        Manchester::encode(&bits, ManchesterConv::Thomas, &mut chips);
    } else {
        chips = bits;
    }
    let last = *chips.last().unwrap_or(&0);
    chips.extend([last, last]);
    chips
}

/// The clean samples at 0 Hz of `chips` at rate `r` (sample rate `rate`): 2-FSK at R1 and R2 (deviation 20 kHz), GFSK with BT 0.6
/// at R3 (deviation 29 kHz), unit amplitude.
pub fn chips_samples(r: u8, chips: &[u8], rate: f64) -> Vec<Complex32> {
    let (chip_rate, dev, pulse, span) = match r {
        1 => (19200.0, 20e3, Shape::Rect, 1.0),
        2 => (40000.0, 20e3, Shape::Rect, 1.0),
        _ => (100_000.0, 29e3, Shape::GaussianFreq { bt: 0.6 }, 4.0),
    };
    let Ok(mut m) = Cpm::with_deviation(pulse, span, dev, chip_rate, rate) else {
        return Vec::new();
    };
    let levels: Vec<f32> = chips
        .iter()
        .map(|&c| if c != 0 { 1.0 } else { -1.0 })
        .collect();
    let mut out = Vec::new();
    m.push(&levels, &mut out);
    m.flush(&mut out);
    out
}

/// The clean samples at 0 Hz of one frame at rate `r`.
pub fn frame_samples(r: u8, mpdu: &[u8], rate: f64) -> Vec<Complex32> {
    chips_samples(r, &frame_chips(r, mpdu), rate)
}

/// The centre of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new(
    "gen_centre_hz",
    "Test signal radio centre",
    "Hz",
    0.0,
)
.help("The frequency that the radio is tuned to: the test signal puts the channels of the setting channels around it.")
.section("Test signal")
.advanced();

/// The `zwave` generator: the schedule of [`schedule`] in a loop, 40 ms of silence between frames, each frame on the first
/// channel of its rate (the first channel when no channel has the rate), at its offset from `centre`. An empty list puts
/// every frame at the centre. `None` when the rate cannot hold the scene.
pub fn generator(rate: f64, channels: &[(f64, u8)], centre: f64) -> Option<CleanScene> {
    let channels = channels_or_centre(channels, centre);
    let first = channels[0].0;
    let on: [f64; 3] =
        [1u8, 2, 3].map(|r| channels.iter().find(|c| c.1 == r).map_or(first, |c| c.0));
    let channel = move |r: u8| on[usize::from(r.clamp(1, 3) - 1)];
    let sched = schedule();
    let mut t = 0.05f64;
    let mut k = 0usize;
    let mut scene = Scene::new(rate).ok()?;
    scene.add_source(Box::new(move || {
        let (r, m) = &sched[k % sched.len()];
        k += 1;
        let samples = frame_samples(*r, m, rate);
        let start_s = t;
        t += samples.len() as f64 / rate + 0.04;
        Some(Burst {
            start_s,
            offset_hz: channel(*r) - centre,
            level_db: 0.0,
            samples,
        })
    }));
    Some(CleanScene::new(
        scene,
        rate,
        SignalInfo::new(100e3).with_symbol_rate(40e3),
    ))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let channels = parse_channels(s.get(&CHANNELS)).ok_or_else(|| {
        s.schema()
            .id(CHANNELS.key())
            .map_or(ConfigError::NoRate, |id| {
                SettingError::Malformed { id, at: None }.into()
            })
    })?;
    // At 0 the channels go around the frequency where the session listens.
    let centre = match s.get(&GEN_CENTRE_HZ) {
        c if c > 0.0 => c,
        _ => cfg.tuned_hz().unwrap_or(0.0),
    };
    let g = generator(cfg.rate_hz, &channels, centre).ok_or(ConfigError::NoRate)?;
    Ok(Box::new(g))
}

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = o.seconds.max(3.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = ZwaveFields::read(r.fields());
        let n = |v: fn(&ZwaveFields) -> i64| f.as_ref().map_or(0, v);
        vec![
            at_least_value("frames_ok", n(|f| f.frames_ok), 20),
            at_least_value("nodes", n(|f| f.nodes), 6),
            at_least_value("frames_r1", n(|f| f.frames_r1), 1),
            at_least_value("frames_r3", n(|f| f.frames_r3), 1),
        ]
    });
    vec![c]
}

/// The test-signal case of `zwave`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

/// Z-Wave.
pub static ZWAVE: StackDescriptor = StackDescriptor {
    id: "zwave",
    name: "Z-Wave",
    group: "IoT, sub-GHz",
    blurb: "Z-Wave home automation (ITU-T G.9959, R1 R2 R3): networks, nodes and commands; Security S0 and S2 payloads shown as encrypted.",
    tuning: Tuning { min_hz: 860e6, max_hz: 930e6, no_offset_tuning: true, ..Tuning::anywhere(0.0, 2e6, MIN_RATE_HZ, 1.8e6) },
    submodes: &[],
    settings: ZwaveSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::ZWave],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
