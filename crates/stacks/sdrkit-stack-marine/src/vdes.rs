//! VDES: ASM frames decoded through the AIS physical layer, and VDE-TER bursts detected per 25 kHz sub-channel, measured and
//! classified.
//!
//! Standard: ITU-R M.2092-1

use crate::ais::demod;
use crate::ais::gmsk;
use crate::ais::line;
use crate::ais::message;
use crate::ais::receiver::{AisDecoded, BurstDecoder, BurstDetector, ChannelLevel, NoHints};
use crate::ais::station as st;
use crate::report::{Clock, Hearing};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::{self, Convolver, Fir};
use sdrkit_algo_dsp::interp::{CatmullRom, Reader};
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_algo_modem::freq::centroid;
use sdrkit_algo_modem::squelch::Gate;
use sdrkit_algo_modem::timing::spectral_line;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{EventKind, Fields, Hertz, Seconds, Table, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, Record, Results, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder,
    GraphError, Io, Item, Kind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// The ASM channels: label, name, frequency.
pub const ASM: [(char, &str, f64); 4] = [
    ('1', "ASM 1 (2027)", 161.950e6),
    ('2', "ASM 2 (2028)", 162.000e6),
    ('A', "AIS-channel ASM 1", 161.975e6),
    ('B', "AIS-channel ASM 2", 162.025e6),
];
/// The VDE-TER channels (100 kHz, four 25 kHz sub-channels each).
pub const TER: [f64; 2] = [157.200e6, 161.800e6];
/// The channeliser rate.
pub const TER_RATE: f64 = 230_400.0;
/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 400e3;
/// The default frequency of a file without one.
pub const DEFAULT_HZ: f64 = 161.9e6;
/// The VDE-TER symbol rate per 25 kHz sub-channel, Bd.
pub const TER_BAUD: f64 = 19_200.0;
/// The roll-off of the VDE-TER pulse.
pub const ROLL_OFF: f64 = 0.3;
/// The most ASM frames listed.
pub const ASM_CAP: usize = 200;
/// The most VDE-TER bursts listed.
pub const BURST_CAP: usize = 300;
/// The time between two reports, seconds.
pub const REPORT_S: f64 = 0.25;

/// The stream centre and the dial of a format. A file without a frequency is taken as [`DEFAULT_HZ`].
fn place(f: &Format) -> (f64, f64) {
    if f.centre_hz > 0.0 {
        (f.centre_hz, f.tuned_hz)
    } else {
        let d = f.tuned_hz - f.centre_hz;
        (DEFAULT_HZ - d, DEFAULT_HZ)
    }
}

/// Where the channel at `freq_hz`, `width_hz` wide, lies in the stream of `f` (Hz from its centre), or `None` when the capture
/// does not hold it.
fn offset_in(freq_hz: f64, width_hz: f64, f: &Format) -> Option<f64> {
    let (centre, _) = place(f);
    let off = freq_hz - centre;
    (off.abs() + width_hz / 2.0 <= 0.49 * f.rate).then_some(off)
}

/// Moves one channel (`vdes.channel`) at an absolute frequency to 0 Hz at a fixed rate. It outputs nothing when the capture
/// does not hold the channel.
pub struct ChannelAt {
    freq: f64,
    width: f64,
    rate: f64,
    chan: Option<Channelizer>,
    filt: Option<Fir<Complex32>>,
}

impl ChannelAt {
    /// The channel at `freq_hz`, `width_hz` wide, out at `rate_hz`.
    pub fn new(freq_hz: f64, width_hz: f64, rate_hz: f64) -> Self {
        ChannelAt {
            freq: freq_hz,
            width: width_hz,
            rate: rate_hz,
            chan: None,
            filt: None,
        }
    }
}

impl Component for ChannelAt {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "vdes.channel",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(self.rate))],
        )
        .manual_tags()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.chan = None;
        self.filt = None;
        if let Some(off) = offset_in(self.freq, self.width, f) {
            let mut cfg = ChannelConfig::new(f.rate, off, self.width, self.rate);
            cfg.exact_rate = true;
            cfg.atten_db = 60.0;
            self.chan = Some(Channelizer::new(cfg)?);
            if self.rate == demod::RATE {
                self.filt = Some(Fir::new(&fir::lowpass(self.rate, 7.5e3, 15e3, 60.0)?));
            }
        }
        Ok(vec![Format::iq(self.rate, self.freq, self.freq)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if let Some(c) = &mut self.chan {
            let out = io.iq_out(0);
            let s = out.len();
            c.process(x, out);
            if let Some(f) = &mut self.filt {
                f.process_in_place(&mut out[s..]);
            }
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// What the parts find.

/// A decoded ASM frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AsmFrame {
    /// When it was heard.
    pub at: Stamp,
    /// The ASM channel label (`1`, `2`, `A`, `B`).
    pub channel: char,
    /// The length of the payload, bits.
    pub bits: usize,
    /// The payload in hexadecimal.
    pub hex: String,
    /// The AIS message type when bits 0 to 5 hold 1 to 27, else 0.
    pub msg_type: u8,
    /// The AIS MMSI (bits 8 to 37) of an AIS message, else 0.
    pub mmsi: u32,
    /// The SNR in 48 kHz, dB.
    pub snr_db: f64,
    /// The carrier, Hz from the channel centre.
    pub offset_hz: f64,
    /// The latitude of an AIS position, degrees.
    pub lat: Option<f64>,
    /// The longitude of an AIS position, degrees.
    pub lon: Option<f64>,
    /// The speed over the ground of an AIS position, knots.
    pub sog_kt: Option<f64>,
    /// The course over the ground of an AIS position, degrees.
    pub cog_deg: Option<f64>,
    /// The `!AIVDM` sentences of an AIS message.
    pub sentences: Vec<String>,
}

impl AsmFrame {
    /// The MMSI as nine digits, or empty for a payload that is not AIS.
    pub fn mmsi_text(&self) -> String {
        if self.msg_type == 0 {
            String::new()
        } else {
            format!("{:09}", self.mmsi)
        }
    }
}

sdrkit_core::choice_enum! {
    /// The modulation of a VDE-TER burst, as the classifier finds it.
    #[derive(Default, Serialize, Deserialize)]
    pub enum Modulation {
        /// pi/4-QPSK.
        Pi4Qpsk = ("pi/4-QPSK", "pi/4-QPSK"),
        /// 8PSK.
        Psk8 = ("8PSK", "8PSK"),
        /// 16QAM.
        Qam16 = ("16QAM", "16QAM"),
        /// Not one of the three.
        #[default]
        Unknown = ("unknown", "Unknown"),
    }
}

impl Modulation {
    /// Every modulation, in the order of its choices.
    pub const ALL: [Modulation; 4] = [
        Modulation::Pi4Qpsk,
        Modulation::Psk8,
        Modulation::Qam16,
        Modulation::Unknown,
    ];
    /// The name: `pi/4-QPSK`, `8PSK`, `16QAM` or `unknown`.
    pub fn name(self) -> &'static str {
        match self {
            Modulation::Pi4Qpsk => "pi/4-QPSK",
            Modulation::Psk8 => "8PSK",
            Modulation::Qam16 => "16QAM",
            Modulation::Unknown => "unknown",
        }
    }
}

/// A VDE-TER burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TerBurst {
    /// When it started.
    pub at: Stamp,
    /// The centre of its 100 kHz channel, Hz.
    pub band_hz: f64,
    /// The centre of its occupied sub-channels, Hz.
    pub centre_hz: f64,
    /// Its bandwidth: 25, 50 or 100 kHz.
    pub bandwidth_hz: f64,
    /// The modulation found.
    pub modulation: Modulation,
    /// The peak of its smoothed power, dBFS.
    pub rssi_dbfs: f64,
    /// The SNR in the occupied bandwidth, dB.
    pub snr_db: f64,
    /// How long it lasted, seconds.
    pub duration_s: f64,
    /// The carrier, Hz from the centre.
    pub offset_hz: f64,
    /// The spread of the symbol amplitudes (standard deviation over the mean).
    pub cv: f64,
    /// The mean of cos(4 x phase step).
    pub psi: f64,
    /// The symbols used for the classification.
    pub symbols: usize,
    /// The occupied sub-channels (0 to 3).
    pub subchannels: Vec<usize>,
}

impl Shift for AsmFrame {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Shift for TerBurst {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

/// What the VDES parts tell the display.
#[derive(Clone, Debug)]
pub enum VdesReport {
    /// An ASM frame (its time is signal time since the configure).
    Asm(Box<AsmFrame>),
    /// A frame on ASM channel `i` failed its check.
    AsmBad(usize),
    /// The level of ASM channel `i`.
    AsmLevel(usize, ChannelLevel),
    /// A VDE-TER burst (its time is signal time since the configure).
    Burst(Box<TerBurst>),
    /// The level of a VDE-TER channel.
    TerLevel {
        /// The channel (0: 157.2 MHz, 1: 161.8 MHz).
        band: usize,
        /// The mean power per sub-channel.
        level: f64,
        /// The mean floor of the sub-channels.
        floor: f64,
    },
}
impl Item for VdesReport {
    const KIND: &'static str = "vdes.report";
}

/// The ASM interpreter (`vdes.asm`): frames to ASM records, with the AIS fields when the payload is an AIS message with a position.
#[derive(Default)]
pub struct AsmInterpreter {
    seq: u8,
    decoded: Vec<Timed<AisDecoded>>,
}

impl Component for AsmInterpreter {
    fn info(&self) -> ComponentInfo {
        let mut ins: Vec<Port> = (0..4)
            .map(|i| Port::typed::<AisDecoded>(&format!("in{i}")))
            .collect();
        ins.extend((0..4).map(|i| Port::typed::<ChannelLevel>(&format!("level{i}"))));
        ComponentInfo::processor(
            "vdes.asm",
            ins,
            vec![Port::typed::<VdesReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<VdesReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for i in 0..4 {
            let mut items = std::mem::take(&mut self.decoded);
            items.clear();
            items.extend_from_slice(io.items::<AisDecoded>(i));
            for d in items.drain(..) {
                if d.value.frames.is_empty() {
                    if d.value.bad {
                        io.items_out::<VdesReport>(0).push(Timed {
                            time: d.time,
                            value: VdesReport::AsmBad(i),
                        });
                    }
                    continue;
                }
                for f in &d.value.frames {
                    let t = f.start_index / demod::RATE;
                    let (ty, mmsi) = message::header(&f.bits).unwrap_or((0, 0));
                    let ais = (1..=27).contains(&ty);
                    let mut a = AsmFrame {
                        at: Stamp::at(t),
                        channel: ASM[i].0,
                        bits: f.bits.len(),
                        hex: message::hex(&f.bits),
                        msg_type: if ais { ty } else { 0 },
                        mmsi: if ais { mmsi } else { 0 },
                        snr_db: f.snr_db,
                        offset_hz: f.offset_hz,
                        ..Default::default()
                    };
                    if let Ok(m) = message::decode(&f.bits) {
                        a.lat = m.lat;
                        a.lon = m.lon;
                        a.sog_kt = m.sog_kn;
                        a.cog_deg = m.cog_deg;
                        let ch = if i == 3 || i == 1 { 'B' } else { 'A' };
                        // A payload that the sentence writer refuses has no sentences.
                        let _ = message::sentences(&f.bits, ch, &mut self.seq, &mut a.sentences);
                    }
                    io.items_out::<VdesReport>(0).push(Timed {
                        time: t,
                        value: VdesReport::Asm(Box::new(a)),
                    });
                }
            }
            self.decoded = items;
            if let Some(l) = io.items::<ChannelLevel>(4 + i).last() {
                let v = l.value.clone();
                io.items_out::<VdesReport>(0).push(Timed {
                    time: l.time,
                    value: VdesReport::AsmLevel(i, v),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The VDE-TER sub-channel detector and classifier.

const NFFT: usize = 256;
const HOP: usize = 128;
/// Frames of the warm-up that sets the first floors.
const WARM_FRAMES: usize = 300;
/// The longest burst, frames.
const MAX_FRAMES: usize = 900;
/// The shortest burst kept, frames.
const MIN_FRAMES: usize = 12;
/// The start of a burst: a sub-channel 3.2 times (5.05 dB) above its floor.
const START_RATIO: f64 = 3.2;
/// A burst goes on while a sub-channel is twice (3.01 dB) its floor.
const CONTINUE_RATIO: f64 = 2.0;
/// A burst ends after this many quiet frames.
const QUIET_FRAMES: u32 = 4;

/// The matched filter of a burst `subs` sub-channels wide (1, 2 or 4): the root-raised-cosine pulse over 12 symbols.
fn matched_filter(subs: usize) -> Convolver<Complex32> {
    let sps = TER_RATE / (TER_BAUD * subs as f64);
    let taps = pulse::taps(Shape::Rrc { beta: ROLL_OFF }, sps, 12.0, Norm::Energy)
        .expect("the VDE-TER pulse is valid");
    Convolver::new(&taps).expect("the VDE-TER taps are finite")
}

/// The sub-channel detector and burst classifier (`vdes.subchannels`) of one 100 kHz channel at 230.4 kHz.
pub struct SubChannels {
    band: usize,
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    fill: usize,
    work: Vec<Complex32>,
    power: Vec<f64>,
    frames: u64,
    floor: [f64; 4],
    warm: Vec<[f64; 4]>,
    level: f64,
    gate: Gate,
    // The burst in progress.
    start_frame: u64,
    frame_act: Vec<[bool; 4]>,
    spec: Vec<f64>,
    samples: Vec<Complex32>,
    hist: VecDeque<Complex32>,
    count: u64,
    // The classifier's filters (1, 2 and 4 sub-channels) and buffers, kept between bursts.
    filters: [Convolver<Complex32>; 3],
    shifted: Vec<f32>,
    mixed: Vec<Complex32>,
    filtered: Vec<Complex32>,
    smooth: Vec<f64>,
    syms: Vec<Complex32>,
    scratch: Vec<f64>,
    out: Vec<Timed<VdesReport>>,
}

fn burst_gate() -> Gate {
    Gate::new(
        db(START_RATIO) as f32,
        db(CONTINUE_RATIO) as f32,
        1,
        QUIET_FRAMES - 1,
    )
    .expect("the burst thresholds are ordered")
}

impl SubChannels {
    /// For VDE-TER channel `band` (0: 157.2 MHz, 1: 161.8 MHz).
    pub fn new(band: usize) -> Self {
        SubChannels {
            band,
            fft: Fft::forward(NFFT),
            win: sdrkit_algo_dsp::window::Window::Hann.coefficients(NFFT),
            buf: vec![Complex32::default(); NFFT],
            fill: 0,
            work: vec![Complex32::default(); NFFT],
            power: vec![0.0; NFFT],
            frames: 0,
            floor: [0.0; 4],
            warm: Vec::with_capacity(WARM_FRAMES),
            level: 0.0,
            gate: burst_gate(),
            start_frame: 0,
            frame_act: Vec::with_capacity(MAX_FRAMES),
            spec: vec![0.0; NFFT],
            samples: Vec::with_capacity(MAX_FRAMES * HOP + NFFT),
            hist: VecDeque::with_capacity(NFFT + 1),
            count: 0,
            filters: [matched_filter(1), matched_filter(2), matched_filter(4)],
            shifted: vec![0.0; NFFT],
            mixed: Vec::new(),
            filtered: Vec::new(),
            smooth: Vec::new(),
            syms: Vec::new(),
            scratch: Vec::with_capacity(WARM_FRAMES),
            out: Vec::new(),
        }
    }

    fn bin_of(hz: f64) -> isize {
        (hz / (TER_RATE / NFFT as f64)).round() as isize
    }

    /// The mean power of the bins of sub-channel `k` (its centre +-11 kHz).
    fn sub_power(p: &[f64], k: usize) -> f64 {
        let c = -37.5e3 + 25e3 * k as f64;
        let (a, b) = (Self::bin_of(c - 11e3), Self::bin_of(c + 11e3));
        let mut s = 0.0;
        for i in a..=b {
            s += p[i.rem_euclid(NFFT as isize) as usize];
        }
        s / (b - a + 1) as f64
    }

    fn frame(&mut self, out: &mut Vec<Timed<VdesReport>>) {
        for i in 0..NFFT {
            self.work[i] = self.buf[i] * self.win[i];
        }
        self.fft.process(&mut self.work);
        let norm = (NFFT * NFFT) as f64;
        for (p, c) in self.power.iter_mut().zip(&self.work) {
            *p = f64::from(c.norm_sqr()) / norm;
        }
        let sub: [f64; 4] = std::array::from_fn(|k| Self::sub_power(&self.power, k));
        self.frames += 1;
        let total: f64 = sub.iter().sum::<f64>() / 4.0;
        self.level += 0.01 * (total - self.level);
        if self.frames as usize <= WARM_FRAMES {
            self.warm.push(sub);
            if self.frames as usize == WARM_FRAMES {
                // The first floors: the lower quartile of each sub-channel over the warm-up.
                for k in 0..4 {
                    let v: Vec<f64> = self.warm.iter().map(|s| s[k]).collect();
                    let q = sdrkit_algo_dsp::stats::quantile_f64(&v, 0.25, &mut self.scratch);
                    self.floor[k] = q.unwrap_or(0.0).max(1e-20);
                }
                self.warm.clear();
            }
            return;
        }
        let measure = (0..4)
            .map(|k| db(sub[k] / self.floor[k]))
            .fold(f64::NEG_INFINITY, f64::max);
        let was_open = self.gate.is_open();
        let open = self.gate.update(Some(measure as f32));
        if open && !was_open {
            self.start_frame = self.frames;
            self.frame_act.clear();
            self.spec.iter_mut().for_each(|v| *v = 0.0);
            self.samples.clear();
            self.samples.extend(self.hist.iter().copied());
        }
        if !(open || was_open) {
            for k in 0..4 {
                // The floor follows the quiet sub-channels slowly.
                self.floor[k] += 0.002 * (sub[k].min(4.0 * self.floor[k]) - self.floor[k]);
            }
            return;
        }
        let act: [bool; 4] = std::array::from_fn(|k| sub[k] > CONTINUE_RATIO * self.floor[k]);
        if act.iter().any(|b| *b) {
            self.frame_act.push(act);
            for (s, p) in self.spec.iter_mut().zip(&self.power) {
                *s += p;
            }
        }
        let len = self.frame_act.len();
        if !open || len >= MAX_FRAMES {
            if open {
                self.gate.reset();
            }
            if len >= MIN_FRAMES {
                if let Some(b) = self.classify() {
                    out.push(Timed {
                        time: b.at.t,
                        value: VdesReport::Burst(Box::new(b)),
                    });
                }
            }
        }
    }

    fn classify(&mut self) -> Option<TerBurst> {
        let n = self.frame_act.len();
        let mut occ: Vec<usize> = Vec::with_capacity(4);
        for k in 0..4 {
            let c = self.frame_act.iter().filter(|a| a[k]).count();
            if c >= 2 && c as f64 >= 0.4 * n as f64 {
                occ.push(k);
            }
        }
        let (first, last) = (*occ.first()?, *occ.last()?);
        let nsub: usize = match occ.len() {
            1 => 1,
            2 => 2,
            _ => 4,
        };
        let band_c = TER[self.band];
        let mid_k = (first + last) as f64 / 2.0;
        let centre_off = -37.5e3 + 25e3 * mid_k;
        let bw = 25e3 * nsub as f64;
        // Carrier offset: the centroid of the burst's spectrum above the floor over the occupied band, on the spectrum with 0 Hz at
        // the middle.
        let bin = TER_RATE / NFFT as f64;
        let half = (NFFT / 2) as isize;
        let fl: f64 = occ.iter().map(|&k| self.floor[k]).sum::<f64>() / occ.len() as f64 * n as f64;
        for (j, v) in self.shifted.iter_mut().enumerate() {
            *v = self.spec[(j as isize - half).rem_euclid(NFFT as isize) as usize] as f32;
        }
        let (a, b) = (
            Self::bin_of(centre_off - bw / 2.0),
            Self::bin_of(centre_off + bw / 2.0),
        );
        let (lo, hi) = ((a + half) as usize, (b + half + 1) as usize);
        let offset = centroid(&self.shifted, lo, hi, fl as f32)
            .map_or(0.0, |c| (c - half as f64) * bin - centre_off);
        let ptot: f64 = (a..=b)
            .map(|i| self.spec[i.rem_euclid(NFFT as isize) as usize])
            .sum();
        let nb = (b - a + 1) as f64;
        let sig = (ptot - fl * nb).max(1e-30);
        let snr = db(sig / (fl * nb).max(1e-30));
        // Symbols: the burst moved to 0 Hz, the matched filter, timing from the |y|^2 line, Catmull-Rom interpolation.
        let rs = TER_BAUD * nsub as f64;
        let sps = TER_RATE / rs;
        self.mixed.resize(self.samples.len(), Complex32::default());
        Nco::new(TER_RATE, -(centre_off + offset)).mix(&self.samples, &mut self.mixed);
        self.filtered.resize(self.mixed.len(), Complex32::default());
        let filter = &self.filters[nsub.trailing_zeros() as usize];
        filter.centred(&self.mixed, &mut self.filtered).ok()?;
        let y = &self.filtered;
        // The part within 3 dB of the peak of the smoothed power.
        let m = (sps.ceil() as usize * 4).max(8);
        self.smooth.clear();
        let mut acc = 0.0;
        for i in 0..y.len() {
            acc += f64::from(y[i].norm_sqr());
            if i >= m {
                acc -= f64::from(y[i - m].norm_sqr());
            }
            self.smooth.push(acc / m as f64);
        }
        let peak = self.smooth.iter().copied().fold(0.0, f64::max);
        let good = |i: &usize| self.smooth[*i] >= peak * 0.5;
        let g0 = (0..self.smooth.len())
            .find(good)
            .unwrap_or(0)
            .saturating_sub(m / 2);
        let g1 = (0..self.smooth.len())
            .rev()
            .find(good)
            .unwrap_or(0)
            .saturating_sub(m / 2);
        self.syms.clear();
        if g1 > g0 {
            if let Ok(tau) = spectral_line(&y[g0..g1], sps) {
                let reader = Reader::new(CatmullRom);
                let mut t = g0 as f64 + tau;
                while t + 2.0 < g1 as f64 {
                    self.syms.push(reader.at(y, t));
                    t += sps;
                }
            }
        }
        let syms = &self.syms;
        let (mut cv, mut psi) = (0.0, 0.0);
        let mut modulation = Modulation::Unknown;
        if syms.len() >= 24 {
            let amps = syms.iter().map(|s| f64::from(s.norm()));
            let k = syms.len() as f64;
            let mean = amps.clone().sum::<f64>() / k;
            let sd = (amps.map(|a| (a - mean).powi(2)).sum::<f64>() / k).sqrt();
            cv = sd / mean.max(1e-12);
            let mut c = 0.0;
            for w in syms.windows(2) {
                let dp = f64::from((w[1] * w[0].conj()).arg());
                c += (4.0 * dp).cos();
            }
            psi = c / (k - 1.0);
            modulation = if cv > 0.18 {
                Modulation::Qam16
            } else if psi < -0.55 {
                Modulation::Pi4Qpsk
            } else if psi.abs() < 0.45 {
                Modulation::Psk8
            } else {
                Modulation::Unknown
            };
        }
        Some(TerBurst {
            at: Stamp::at(self.start_frame as f64 * HOP as f64 / TER_RATE),
            band_hz: band_c,
            centre_hz: band_c + centre_off,
            bandwidth_hz: bw,
            modulation,
            rssi_dbfs: db(peak),
            snr_db: snr,
            duration_s: n as f64 * HOP as f64 / TER_RATE,
            offset_hz: offset,
            cv,
            psi,
            symbols: syms.len(),
            subchannels: occ,
        })
    }
}

impl Component for SubChannels {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "vdes.subchannels",
            vec![Port::iq("in").rate(sdrkit_graph::RateRule::Exact(TER_RATE))],
            vec![Port::typed::<VdesReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let b = self.band;
        *self = SubChannels::new(b);
        Ok(vec![Format::new(Kind::typed::<VdesReport>(), 0.0)])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let mut out = std::mem::take(&mut self.out);
        for &s in x {
            self.buf[self.fill] = s;
            self.fill += 1;
            self.count += 1;
            if self.hist.len() == NFFT {
                self.hist.pop_front();
            }
            self.hist.push_back(s);
            if self.gate.is_open() && self.samples.len() < MAX_FRAMES * HOP + NFFT {
                self.samples.push(s);
            }
            if self.fill == NFFT {
                self.frame(&mut out);
                self.buf.copy_within(HOP.., 0);
                self.fill = NFFT - HOP;
            }
        }
        io.items_out::<VdesReport>(0).append(&mut out);
        self.out = out;
        if io.tick() {
            let fl = self.floor.iter().sum::<f64>() / 4.0;
            let t = self.count as f64 / TER_RATE;
            io.items_out::<VdesReport>(0).push(Timed {
                time: t,
                value: VdesReport::TerLevel {
                    band: self.band,
                    level: self.level,
                    floor: fl,
                },
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Fields, tables and events.

/// The result fields of the VDES stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Bursts found on the VDE-TER channels.
    #[field(label = "VDE-TER bursts")]
    pub bursts: i64,
    /// Reports made.
    #[field(label = "Reports")]
    pub reports: i64,
}

/// A row of the ASM frame table. The row key is the place of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesAsmRow<'a> {
    /// When (wall-clock time, when known).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The payload length.
    #[field(label = "Bits")]
    pub bits: i64,
    /// The AIS message type of the payload.
    #[field(label = "AIS type")]
    pub msg_type: Option<i64>,
    /// The MMSI, nine digits.
    #[field(label = "MMSI")]
    pub mmsi: Option<&'a str>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The payload in hexadecimal.
    #[field(label = "Hex")]
    pub hex: &'a str,
}

/// The ASM frames, newest first.
pub static ASM_TABLE: Table<VdesAsmRow<'static>> =
    Table::new("asm", "ASM frames", ASM_CAP).label(&VdesAsmRow::MMSI);

/// A row of the VDE-TER burst table. The row key is the place of the burst in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesBurstRow<'a> {
    /// When (wall-clock time, when known).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The centre of a burst's occupied sub-channels.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The bandwidth of a burst.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bandwidth_hz: Hertz,
    /// The modulation of a burst.
    #[field(label = "Modulation")]
    pub modulation: Modulation,
    /// The peak power of a burst.
    #[field(label = "RSSI", unit = "dBFS")]
    pub rssi_dbfs: f64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// How long a burst lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The spread of the symbol amplitudes.
    #[field(label = "Amplitude spread")]
    pub cv: f64,
    /// The mean of cos(4 x phase step).
    #[field(label = "Phase-step measure")]
    pub psi: f64,
    /// The symbols used for the classification.
    #[field(label = "Symbols")]
    pub symbols: i64,
    /// The occupied sub-channels.
    #[field(label = "Sub-channels")]
    pub subchannels: &'a str,
}

/// The VDE-TER bursts, newest first.
pub static BURST_TABLE: Table<VdesBurstRow<'static>> =
    Table::new("bursts", "VDE-TER bursts", BURST_CAP);

/// A row of the channel table of the VDES stack. The row key is the letter of an ASM channel, or `TER` and the number of a VDE-TER band.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesChannelRow<'a> {
    /// The name of a channel.
    #[field(label = "Channel")]
    pub name: &'a str,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The capture holds the channel.
    #[field(label = "In view")]
    pub in_view: bool,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: Option<i64>,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: Option<i64>,
    /// Bursts found on the VDE-TER channels.
    #[field(label = "VDE-TER bursts")]
    pub bursts: Option<i64>,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Noise floor, dBFS.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
}

/// The channels of VDES.
pub static CHANNEL_TABLE: Table<VdesChannelRow<'static>> = Table::new("channels", "Channels", 6)
    .label(&VdesChannelRow::NAME)
    .sorted(&VdesChannelRow::FREQ_HZ, sdrkit_core::SortOrder::Ascending);

/// A row of the count of bursts by modulation. The row key is the name of the modulation.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesModulationRow {
    /// The modulation of a burst.
    #[field(label = "Modulation")]
    pub modulation: Modulation,
    /// A number of bursts.
    #[field(label = "Bursts")]
    pub count: i64,
}

/// The bursts by modulation.
pub static BY_MODULATION: Table<VdesModulationRow> =
    Table::new("by_modulation", "Bursts by modulation", 4).label(&VdesModulationRow::MODULATION);

/// A row of the count of bursts by bandwidth. The row key is the bandwidth in Hz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesBandwidthRow {
    /// The bandwidth of a burst.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bandwidth_hz: Hertz,
    /// A number of bursts.
    #[field(label = "Bursts")]
    pub count: i64,
}

/// The bursts by bandwidth.
pub static BY_BANDWIDTH: Table<VdesBandwidthRow> =
    Table::new("by_bandwidth", "Bursts by bandwidth", 3)
        .label(&VdesBandwidthRow::BANDWIDTH_HZ)
        .sorted(
            &VdesBandwidthRow::BANDWIDTH_HZ,
            sdrkit_core::SortOrder::Ascending,
        );

/// The fields of the event of an ASM frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesAsmEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The payload length.
    #[field(label = "Bits")]
    pub bits: i64,
    /// The payload in hexadecimal.
    #[field(label = "Hex")]
    pub hex: &'a str,
    /// The AIS message type of the payload.
    #[field(label = "AIS type")]
    pub msg_type: Option<i64>,
}

/// An ASM frame was decoded.
pub static ASM_FRAME: EventKind<VdesAsmEvent<'static>> = EventKind::new("vdes.asm", "ASM frame");

/// The fields of the event of an NMEA sentence of an ASM frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesSentenceEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// An `!AIVDM` sentence.
    #[field(label = "Sentence")]
    pub sentence: &'a str,
}

/// An NMEA sentence of an ASM frame.
pub static ASM_SENTENCE: EventKind<VdesSentenceEvent<'static>> =
    EventKind::new("vdes.nmea", "ASM sentence");

/// The fields of the event of a VDE-TER burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VdesBurstEvent {
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The modulation of a burst.
    #[field(label = "Modulation")]
    pub modulation: Modulation,
    /// The bandwidth of a burst.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bandwidth_hz: Hertz,
    /// How long a burst lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
}

/// A VDE-TER burst was measured.
pub static TER_BURST: EventKind<VdesBurstEvent> = EventKind::new("vdes.burst", "VDE-TER burst");

impl Shaped for AsmFrame {
    type Row = VdesAsmRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let ais = self.msg_type != 0;
        let mut b = [0u8; 4];
        let mut digits = [0u8; 9];
        let mmsi = st::MmsiKey(self.mmsi).digits(&mut digits);
        out.put(&VdesAsmRow {
            time: self.at.wall.map(Time),
            channel: self.channel.encode_utf8(&mut b),
            bits: self.bits as i64,
            msg_type: ais.then_some(i64::from(self.msg_type)),
            mmsi: ais.then_some(mmsi),
            snr_db: self.snr_db,
            offset_hz: Hertz(self.offset_hz),
            position: self.lat.zip(self.lon),
            speed_kt: self.sog_kt,
            course_deg: self.cog_deg,
            hex: &self.hex,
        });
    }
}

impl Shaped for TerBurst {
    type Row = VdesBurstRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let mut subs = String::new();
        for (i, k) in self.subchannels.iter().enumerate() {
            if i > 0 {
                subs.push(',');
            }
            let _ = write!(subs, "{k}");
        }
        out.put(&VdesBurstRow {
            time: self.at.wall.map(Time),
            centre_hz: Hertz(self.centre_hz),
            bandwidth_hz: Hertz(self.bandwidth_hz),
            modulation: self.modulation,
            rssi_dbfs: self.rssi_dbfs,
            snr_db: self.snr_db,
            duration: Seconds(self.duration_s),
            offset_hz: Hertz(self.offset_hz),
            cv: self.cv,
            psi: self.psi,
            symbols: self.symbols as i64,
            subchannels: &subs,
        });
    }
}

/// What the display keeps across sessions: the ASM frames and the VDE-TER bursts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VdesLists {
    /// The ASM frames, oldest first.
    pub asm: LogRing<AsmFrame>,
    /// The VDE-TER bursts, oldest first.
    pub bursts: LogRing<TerBurst>,
}

impl Default for VdesLists {
    fn default() -> Self {
        VdesLists {
            asm: LogRing::new(ASM_CAP),
            bursts: LogRing::new(BURST_CAP),
        }
    }
}

impl Shift for VdesLists {
    fn shift_times(&mut self, by_s: f64) {
        self.asm.shift_times(by_s);
        self.bursts.shift_times(by_s);
    }
}

impl Tables for VdesLists {
    const FORMAT: StateFormat = StateFormat::new("marine.vdes_lists", 1);
    fn clear(&mut self) {
        self.asm.clear();
        self.bursts.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.asm.adopt(saved.asm);
        self.bursts.adopt(saved.bursts);
    }
}

/// The level of one channel as the display shows it.
#[derive(Clone, Copy, Debug, Default)]
struct Level {
    in_view: bool,
    level_dbfs: Option<f64>,
    noise_dbfs: Option<f64>,
}

/// The index in [`BY_BANDWIDTH`] of a bandwidth of 25, 50 or 100 kHz.
fn bandwidth_index(bw: f64) -> usize {
    if bw < 37.5e3 {
        0
    } else if bw < 75e3 {
        1
    } else {
        2
    }
}

/// Send AIS-like ASM frames as NMEA.
pub const NMEA_OUT: Setting<field::Bool> = Setting::new("nmea_out", "Send ASM as NMEA", "", true)
    .help("AIS-like ASM frames go out as !AIVDM sentences.");
/// The wall-clock time of the first sample of a recording.
pub const TIME_BASE: Setting<field::Time> = Setting::new("time_base", "Time base", "", 0.0)
    .range(0.0, 1e11, 0.0)
    .help("UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.");

sdrkit_core::settings! {
    /// The settings of the VDES stack and of [`VdesDisplay`].
    pub enum VdesSetting {
        /// Send AIS-like ASM frames as NMEA.
        NmeaOut = NMEA_OUT,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`VdesDisplay`].
    pub enum VdesCommand {
        /// Forget every frame and burst.
        Clear = objcmd::CLEAR,
    }
}

/// The VDES results (`vdes.display`).
pub struct VdesDisplay {
    p: Published,
    clock: Clock,
    host_clock: sdrkit_core::Clock,
    live: bool,
    lists: VdesLists,
    asm_good: [u64; 4],
    asm_bad: [u64; 4],
    asm_level: [Level; 4],
    ter_level: [Level; 2],
    ter_bursts: [u64; 2],
    by_modulation: [u64; 4],
    by_bandwidth: [u64; 3],
    last_asm: f64,
    last_energy: f64,
    nmea_out: bool,
    time_base: f64,
    /// The map points are out of date with the ASM frames.
    points_stale: bool,
    pending: Vec<Event>,
    items: Vec<Timed<VdesReport>>,
    reports: u64,
}

impl Default for VdesDisplay {
    fn default() -> Self {
        let mut p = Published::from_decl(Arc::new(crate::typed::shape("vdes").results_decl()));
        p.results_mut().set_status(&Status {
            lock_state: Hearing::Searching.lock_state(),
            summary: "listening".into(),
            ..Status::default()
        });
        VdesDisplay {
            p,
            clock: Clock::default(),
            host_clock: sdrkit_core::Clock::fixed(0.0),
            live: false,
            lists: VdesLists::default(),
            asm_good: [0; 4],
            asm_bad: [0; 4],
            asm_level: Default::default(),
            ter_level: Default::default(),
            ter_bursts: [0; 2],
            by_modulation: [0; 4],
            by_bandwidth: [0; 3],
            last_asm: f64::NEG_INFINITY,
            last_energy: f64::NEG_INFINITY,
            nmea_out: true,
            time_base: 0.0,
            points_stale: false,
            pending: Vec::new(),
            items: Vec::new(),
            reports: 0,
        }
    }
}

impl VdesDisplay {
    /// The lists.
    pub fn lists(&self) -> &VdesLists {
        &self.lists
    }

    fn clear(&mut self) {
        self.points_stale = true;
        self.lists.clear();
        self.asm_good = [0; 4];
        self.asm_bad = [0; 4];
        self.ter_bursts = [0; 2];
        self.by_modulation = [0; 4];
        self.by_bandwidth = [0; 3];
    }

    fn take(&mut self, time: f64, r: VdesReport) {
        let t = self.clock.at(time);
        match r {
            VdesReport::Asm(mut a) => {
                a.at = self.clock.stamp(t);
                let i = ASM.iter().position(|c| c.0 == a.channel).unwrap_or(0);
                self.asm_good[i] += 1;
                self.last_asm = t;
                let freq = ASM[i].2;
                let ais = a.msg_type != 0;
                let mmsi = ais.then(|| a.mmsi_text());
                if self.nmea_out {
                    for s in &a.sentences {
                        self.pending.push(ASM_SENTENCE.event(
                            t,
                            freq,
                            &VdesSentenceEvent {
                                source: mmsi.as_deref(),
                                family: Family::Vessel,
                                sentence: s,
                            },
                        ));
                    }
                }
                let mut b = [0u8; 4];
                let e = ASM_FRAME.event(
                    t,
                    freq,
                    &VdesAsmEvent {
                        source: mmsi.as_deref(),
                        family: Family::Vessel,
                        channel: a.channel.encode_utf8(&mut b),
                        bits: a.bits as i64,
                        hex: &a.hex,
                        msg_type: ais.then_some(i64::from(a.msg_type)),
                    },
                );
                self.pending.push(e);
                self.lists.asm.push(*a);
                self.points_stale = true;
            }
            VdesReport::AsmBad(i) => {
                self.asm_bad[i] += 1;
                self.last_energy = t;
            }
            VdesReport::AsmLevel(i, l) => {
                self.asm_level[i] = Level {
                    in_view: self.asm_level[i].in_view,
                    level_dbfs: (l.level > 0.0).then(|| db(l.level)),
                    noise_dbfs: l.floor.map(|f| db(f * ASM_NOISE_SCALE)),
                };
                if l.detected > 0 {
                    self.last_energy = self.last_energy.max(t - 1.0);
                }
            }
            VdesReport::Burst(mut b) => {
                b.at = self.clock.stamp(t);
                self.last_energy = t;
                let m = Modulation::ALL
                    .iter()
                    .position(|m| *m == b.modulation)
                    .unwrap_or(3);
                self.by_modulation[m] += 1;
                self.by_bandwidth[bandwidth_index(b.bandwidth_hz)] += 1;
                let band = TER
                    .iter()
                    .position(|f| (f - b.band_hz).abs() < 1.0)
                    .unwrap_or(0);
                self.ter_bursts[band] += 1;
                self.pending.push(TER_BURST.event(
                    t,
                    b.centre_hz,
                    &VdesBurstEvent {
                        family: Family::Vessel,
                        modulation: b.modulation,
                        bandwidth_hz: Hertz(b.bandwidth_hz),
                        duration: Seconds(b.duration_s),
                    },
                ));
                self.lists.bursts.push(*b);
            }
            VdesReport::TerLevel { band, level, floor } => {
                self.ter_level[band] = Level {
                    in_view: self.ter_level[band].in_view,
                    level_dbfs: (level > 0.0).then(|| db(level)),
                    noise_dbfs: (floor > 0.0).then(|| db(floor)),
                };
            }
        }
    }

    fn report(&mut self) {
        self.reports += 1;
        let now = self.clock.now();
        let state = if now - self.last_asm <= 30.0 {
            Hearing::Decoding
        } else if now - self.last_energy <= 30.0 {
            Hearing::Signal
        } else {
            Hearing::Searching
        };
        let good: u64 = self.asm_good.iter().sum();
        let bad: u64 = self.asm_bad.iter().sum();
        let nb = self.lists.bursts.len();
        let na = self.lists.asm.len();
        let p = &mut self.p;
        p.put(&VdesFields {
            frames_ok: good as i64,
            frames_bad: bad as i64,
            bursts: self.ter_bursts.iter().sum::<u64>() as i64,
            reports: self.reports as i64,
        });
        shaped::log_rows(&self.lists.asm, &ASM_TABLE, now, p);
        shaped::log_rows(&self.lists.bursts, &BURST_TABLE, now, p);
        let mut key = String::new();
        for (i, (label, name, f)) in ASM.iter().enumerate() {
            let l = &self.asm_level[i];
            key.clear();
            key.push(*label);
            CHANNEL_TABLE.put(
                p,
                &key,
                &VdesChannelRow {
                    name,
                    freq_hz: Hertz(*f),
                    in_view: l.in_view,
                    frames_ok: Some(self.asm_good[i] as i64),
                    frames_bad: Some(self.asm_bad[i] as i64),
                    bursts: None,
                    level_dbfs: l.level_dbfs,
                    noise_dbfs: l.noise_dbfs,
                },
            );
        }
        for (k, f) in TER.iter().enumerate() {
            let l = &self.ter_level[k];
            key.clear();
            let _ = write!(key, "TER{k}");
            CHANNEL_TABLE.put(
                p,
                &key,
                &VdesChannelRow {
                    name: TER_NAMES[k],
                    freq_hz: Hertz(*f),
                    in_view: l.in_view,
                    frames_ok: None,
                    frames_bad: None,
                    bursts: Some(self.ter_bursts[k] as i64),
                    level_dbfs: l.level_dbfs,
                    noise_dbfs: l.noise_dbfs,
                },
            );
        }
        for (k, m) in Modulation::ALL.iter().enumerate() {
            BY_MODULATION.put(
                p,
                m.name(),
                &VdesModulationRow {
                    modulation: *m,
                    count: self.by_modulation[k] as i64,
                },
            );
        }
        for (k, (bw, name)) in [(25e3, "25000"), (50e3, "50000"), (100e3, "100000")]
            .iter()
            .enumerate()
        {
            BY_BANDWIDTH.put(
                p,
                name,
                &VdesBandwidthRow {
                    bandwidth_hz: Hertz(*bw),
                    count: self.by_bandwidth[k] as i64,
                },
            );
        }
        let r = p.results_mut();
        if std::mem::take(&mut self.points_stale) {
            // Each point shows the row of the newest frame that placed it.
            let table = r.decl().table_id(ASM_TABLE.def().key);
            let mut placed: Vec<String> = Vec::new();
            for (seq, a) in self.lists.asm.iter_seq() {
                if let (Some(lat), Some(lon)) = (a.lat, a.lon) {
                    let id = a.mmsi_text();
                    r.update_point(&id, lat, lon, &id, a.cog_deg.map(|c| c as f32));
                    let row = format!("{seq:06}");
                    r.link_point(&id, table.map(|t| (t, row.as_str())));
                    placed.push(id);
                }
            }
            let gone: Vec<String> = r
                .points()
                .keys()
                .filter(|k| !placed.contains(k))
                .cloned()
                .collect();
            for k in &gone {
                r.remove_point(k);
            }
        }
        let summary = if na > 0 || nb > 0 {
            format!("{na} ASM frames, {nb} VDE-TER bursts")
        } else if state == Hearing::Signal {
            "energy seen, nothing decoded".into()
        } else {
            "listening".into()
        };
        r.set_status(&Status {
            lock_state: state.lock_state(),
            summary,
            ..Status::default()
        });
    }
}

/// The noise in 48 kHz over the floor of an ASM channel: the floor is per sample of the 17 kHz channel filter.
const ASM_NOISE_SCALE: f64 = 48e3 / 17.0e3;

/// The names of the VDE-TER channels.
const TER_NAMES: [&str; 2] = ["VDE-TER 157.2 MHz", "VDE-TER 161.8 MHz"];

impl Component for VdesDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "vdes.display",
            vec![
                Port::typed::<VdesReport>("asm"),
                Port::typed::<VdesReport>("ter_lo"),
                Port::typed::<VdesReport>("ter_hi"),
                // The input itself, for its rate and centre only (which channels the capture holds).
                Port::iq("iq"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(VdesSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(f) = inputs.get(3) {
            for (l, c) in self.asm_level.iter_mut().zip(ASM) {
                l.in_view = offset_in(c.2, 25e3, f).is_some();
            }
            for (l, c) in self.ter_level.iter_mut().zip(TER) {
                l.in_view = offset_in(c, 100e3, f).is_some();
            }
        }
        // A new input: its item times start again at 0, the clock runs on.
        self.clock.restart(self.live, self.host_clock.now());
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, io: &mut Io) {
        let mut items = std::mem::take(&mut self.items);
        items.clear();
        for i in 0..3 {
            items.extend(io.items::<VdesReport>(i).iter().cloned());
        }
        items.sort_by(|a, b| a.time.total_cmp(&b.time));
        for it in items.drain(..) {
            self.take(it.time, it.value);
        }
        self.items = items;
        io.messages_out(0).append(&mut self.pending);
        if io.tick() {
            self.report();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match VdesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            VdesSetting::NmeaOut => self.nmea_out = NMEA_OUT.read(id, v)?,
            VdesSetting::TimeBase => {
                let t = TIME_BASE.read(id, v)?;
                self.time_base = t;
                self.clock.set_time_base((t > 0.0).then_some(t));
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (VdesSetting::NmeaOut, Value::Bool(self.nmea_out)),
            (VdesSetting::TimeBase, Value::Time(self.time_base)),
        ])
    }
    fn commands(&self) -> &Commands {
        VdesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match VdesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            VdesCommand::Clear => self.clear(),
        }
        self.report();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.clock.now())?;
        self.points_stale = true;
        self.report();
        Ok(())
    }
}

/// The VDES components.
pub fn register(r: &mut Registry) {
    r.add("vdes.channel", || {
        Box::new(ChannelAt::new(DEFAULT_HZ, 100e3, TER_RATE))
    });
    r.add("vdes.asm", || Box::new(AsmInterpreter::default()));
    r.add("vdes.subchannels", || Box::new(SubChannels::new(1)));
    r.add("vdes.display", || Box::new(VdesDisplay::default()));
}

/// The VDES graph: the AIS front end at each ASM frequency into the interpreter, the VDE-TER channels into their detectors, and
/// both into the display.
pub fn builder() -> GraphBuilder {
    let mut b = GraphBuilder::new()
        .add("asm", AsmInterpreter::default())
        .add("display", VdesDisplay::default())
        .add("no_hints", NoHints);
    for (i, (l, _, f)) in ASM.iter().enumerate() {
        let id = format!("asm_{}", l.to_ascii_lowercase());
        b = b
            .add(&id, ChannelAt::new(*f, 25e3, demod::RATE))
            .add(&format!("detect_{i}"), BurstDetector::new(i))
            .add(&format!("decode_{i}"), BurstDecoder::new(i))
            .input(&format!("{id}.in"))
            .connect(&format!("{id}.out"), &format!("detect_{i}.in"))
            .connect(&format!("detect_{i}.bursts"), &format!("decode_{i}.bursts"))
            .connect("no_hints.hints", &format!("decode_{i}.hints"))
            .connect(&format!("decode_{i}.out"), &format!("asm.in{i}"))
            .connect(&format!("detect_{i}.level"), &format!("asm.level{i}"));
    }
    for (k, (name, f)) in [("ter_lo", TER[0]), ("ter_hi", TER[1])].iter().enumerate() {
        b = b
            .add(name, ChannelAt::new(*f, 100e3, TER_RATE))
            .add(&format!("sub_{k}"), SubChannels::new(k))
            .input(&format!("{name}.in"))
            .connect(&format!("{name}.out"), &format!("sub_{k}.in"))
            .connect(&format!("sub_{k}.report"), &format!("display.{name}"));
    }
    b.connect("asm.report", "display.asm")
        .input("display.iq")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .reconfigure_on_retune()
}

/// The VDES stack graph: the settings routed to the display, and its clear command.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = StackGraph::of(crate::typed::shape("vdes"), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "VDES needs a sample rate of at least 400 ksps")
        .command("clear", sdrkit_graph::CommandTarget::of("display", "clear"));
    for s in VdesSetting::SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("display.{k}")])?;
    }
    g.with_settings(&cfg.settings).build()
}

/// The VDES graph with no settings applied.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder().catalog(catalog).build()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The modulation of a VDE-TER burst to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerMod {
    /// pi/4-QPSK: each symbol turns by an odd multiple of pi/4.
    Pi4Qpsk,
    /// 8PSK.
    Psk8,
    /// 16QAM.
    Qam16,
}

/// A VDE-TER burst to send.
#[derive(Clone, Debug)]
pub struct TerSpec {
    /// The centre of its 100 kHz channel, Hz.
    pub band_hz: f64,
    /// The first sub-channel it occupies (0 to 3).
    pub first_sub: usize,
    /// The number of sub-channels: 1, 2 or 4.
    pub subs: usize,
    /// The modulation.
    pub modulation: TerMod,
    /// The start, seconds.
    pub start_s: f64,
    /// The duration, seconds.
    pub dur_s: f64,
    /// The power per 25 kHz (1 for unit power in one sub-channel).
    pub level: f64,
    /// The seed of its random symbols.
    pub seed: u64,
}

/// The symbols of a burst: `n` random points of `m`, unit mean power.
fn ter_symbols(m: TerMod, n: usize, seed: u64, out: &mut Vec<Complex32>) {
    let mut rng = Rng::new(seed);
    out.clear();
    match m {
        TerMod::Pi4Qpsk => {
            let steps = PointSet::psk(4, std::f64::consts::FRAC_PI_4).expect("QPSK steps");
            let mut z = Complex32::new(1.0, 0.0);
            for _ in 0..n {
                z *= steps.points()[rng.below(4) as usize];
                out.push(z);
            }
        }
        TerMod::Psk8 => {
            let c = PointSet::psk(8, 0.0).expect("8PSK points");
            out.extend((0..n).map(|_| c.points()[rng.below(8) as usize]));
        }
        TerMod::Qam16 => {
            let c = PointSet::qam(16, QamMap::THREE_GPP).expect("16QAM points");
            out.extend((0..n).map(|_| c.points()[rng.below(16) as usize]));
        }
    }
}

/// Adds a VDE-TER burst into `buf` (samples at `rate`, the stream centred at `centre_hz`): random symbols at 19.2 kBd per
/// sub-channel through the root-raised-cosine pulse (roll-off 0.3, 12 symbols), with equal power density per sub-channel.
pub fn add_ter(buf: &mut [Complex32], rate: f64, centre_hz: f64, b: &TerSpec) {
    let rs = TER_BAUD * b.subs as f64;
    let n_sym = (b.dur_s * rs).ceil() as usize;
    let mut syms = Vec::with_capacity(n_sym);
    ter_symbols(b.modulation, n_sym, b.seed, &mut syms);
    let Ok(mut m) = Linear::rrc(ROLL_OFF, 12.0, rate / rs) else {
        return;
    };
    let mut x = Vec::new();
    m.push(&syms, &mut x);
    m.flush(&mut x);
    let sub_c = -37.5e3 + 25e3 * (b.first_sub as f64 + (b.subs as f64 - 1.0) / 2.0);
    let f = b.band_hz + sub_c - centre_hz;
    let i0 = (b.start_s * rate).floor() as usize;
    let amp = (b.level * b.subs as f64).sqrt() as f32;
    let mut nco = Nco::new(rate, f);
    nco.set_phase_cycles((f * i0 as f64 / rate).rem_euclid(1.0));
    nco.mix_in_place(&mut x);
    for (o, v) in buf.iter_mut().skip(i0).zip(&x) {
        *o += *v * amp;
    }
}

/// Adds an ASM burst (AIS framing, GMSK) at `freq_hz` into `buf` (samples at `rate`, the stream centred at `centre_hz`), with
/// amplitude `amplitude` (1 for unit power).
pub fn add_asm(
    buf: &mut [Complex32],
    rate: f64,
    centre_hz: f64,
    freq_hz: f64,
    start_s: f64,
    bits: &[u8],
    amplitude: f64,
) {
    let b = gmsk::Burst {
        levels: line::burst_levels(bits, 8),
        start_s,
        amplitude,
        offset_hz: freq_hz - centre_hz,
        phase: 0.0,
        clock_ppm: 0.0,
        ramp_bits: 1.0,
    };
    b.add_to(buf, 0.0, rate);
}

/// The length of one cycle of the demo signal, seconds.
pub const CYCLE_S: f64 = 0.6;

/// The demo signal at 161.9 MHz: the AIS-channel ASM at +75 and +125 kHz, and the 161.8 MHz channel at -100 kHz with bursts of 25,
/// 50 and 100 kHz, in a cycle of 0.6 s. A 25 kHz burst and an ASM burst have unit power.
pub struct VdesGenerator {
    rate: f64,
    seed: u64,
    pos: u64,
    block: Vec<Complex32>,
    block_start: u64,
    cycle: u64,
}

impl VdesGenerator {
    /// The demo at `rate`, its symbols from `seed`.
    pub fn new(rate: f64, seed: u64) -> Self {
        VdesGenerator {
            rate,
            seed,
            pos: 0,
            block: Vec::new(),
            block_start: 0,
            cycle: 0,
        }
    }

    /// Cycle `k` of the demo.
    pub fn cycle(&self, k: u64) -> Vec<Complex32> {
        let rate = self.rate;
        let mut x = vec![Complex32::default(); (CYCLE_S * rate) as usize];
        let pos = crate::ais::encode::position_a(&crate::ais::encode::PositionA {
            msg_type: 1,
            mmsi: 244_123_456,
            status: 0,
            rot_raw: -128,
            sog_kn: Some(12.5),
            lat: Some(52.5),
            lon: Some(5.25),
            cog_deg: Some(80.0),
            heading: None,
            second: 60,
        });
        add_asm(&mut x, rate, DEFAULT_HZ, ASM[2].2, 0.05, &pos, 1.0);
        let mut other = Vec::new();
        if sdrkit_util_bits::pack::hex_to_bits("0210842108421084210842108421084210", &mut other)
            .is_ok()
        {
            add_asm(&mut x, rate, DEFAULT_HZ, ASM[3].2, 0.15, &other, 1.0);
        }
        let specs = [
            (0usize, 1usize, TerMod::Pi4Qpsk, 0.25),
            (1, 2, TerMod::Psk8, 0.35),
            (0, 4, TerMod::Qam16, 0.45),
        ];
        for (j, (first, subs, m, t)) in specs.iter().enumerate() {
            add_ter(
                &mut x,
                rate,
                DEFAULT_HZ,
                &TerSpec {
                    band_hz: TER[1],
                    first_sub: *first,
                    subs: *subs,
                    modulation: *m,
                    start_s: *t,
                    dur_s: 0.03,
                    level: 1.0,
                    seed: self.seed + k * 7 + j as u64,
                },
            );
        }
        x
    }
}

impl Generator for VdesGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for s in out.iter_mut() {
            let i = (self.pos - self.block_start) as usize;
            if i >= self.block.len() {
                self.block = self.cycle(self.cycle);
                self.cycle += 1;
                self.block_start = self.pos;
            }
            *s = self.block[(self.pos - self.block_start) as usize];
            self.pos += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(signal_info())
    }
}

/// What the VDES test signal states: a 25 kHz burst has unit power, its SNR is measured in 25 kHz, at 19 200 symbols a second.
pub fn signal_info() -> SignalInfo {
    SignalInfo::new(25e3).with_symbol_rate(TER_BAUD)
}

/// The VDES test signal of a generator configuration.
pub fn from_config(cfg: &GenConfig) -> Box<dyn Generator> {
    Box::new(VdesGenerator::new(cfg.rate_hz, cfg.seed))
}
