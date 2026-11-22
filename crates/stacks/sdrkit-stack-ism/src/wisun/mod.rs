//! Wi-SUN FAN (`wisun`): SUN FSK frames on every channel of the band taken in, through a polyphase filter bank, bursts per band,
//! demodulation per channel and rate, de-whitening, the FCS with least-reliable-bit correction, the MAC header and the Wi-SUN
//! information elements. Secured frames are shown with their header IEs and lengths as encrypted.
//! Standard: IEEE 802.15.4-2020 clause 19 (SUN FSK) and clause 7 (MAC), Wi-SUN FAN 1.1.

pub mod frame;
pub mod send;

use crate::family::{at_least_value, min_rate_message, set_status, CleanScene, REPORT_S};
use crate::mac154::{self, eui, Addr};
use crate::typed::{
    WisunFields, WisunFrameRow, WisunLinkRow, WisunNetworkRow, WisunNodeRow, WisunPanEvent,
    WISUN_FRAMES, WISUN_HOPS, WISUN_LINKS, WISUN_NETWORKS, WISUN_NODES, WISUN_NODE_ROW, WISUN_PAN,
    WISUN_REMOVE,
};
use crate::zigbee::Frame154;
use frame::{add_fcs, channels, fcs_ok, lrb_correct, parse_ies, phr, pn9, summary, utt_name, Plan};
use sdrkit_algo_dsp::bank::PolyphaseBank;
use sdrkit_algo_dsp::fir::{design_by_taps, Convolver};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    Results, ResultsDecl, SeriesId, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StackConfig, StackDescriptor, StageError, StateBlob, StateError, TableId, Tuning, Value, Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kept, Kind, Port, RateFrom, StackGraph, Stage, Timed,
};
use sdrkit_util_bits::{pack, BitOrder, Pad};
use sdrkit_util_ids::oui::vendor_of_address;
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::Tables as _;
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::{self, Write as _};
use std::sync::Arc;

/// A burst of one band (`ws.burst`) with the channels to try.
#[derive(Clone, Debug, Default)]
pub struct WsBurst {
    /// The band samples.
    pub samples: Vec<Complex32>,
    /// The band rate.
    pub rate: f64,
    /// The channels: (Hz, offset from the band centre, modes as (kbit/s, modulation index)).
    pub channels: Vec<(f64, f64, Vec<(u32, f32)>)>,
    /// The signal time of the first sample, seconds.
    pub time: f64,
}

impl Item for WsBurst {
    const KIND: &'static str = "ws.burst";
}

/// Band samples kept before a burst opens.
const PRE_ROLL: usize = 96;

/// The burst detector of one band.
struct BandState {
    p: f32,
    floor: f32,
    n: u64,
    hist: SampleRing<Complex32>,
    open: bool,
    buf: Vec<Complex32>,
    quiet: u32,
    t0: f64,
}

impl BandState {
    fn new() -> BandState {
        BandState {
            p: 0.0,
            floor: 0.0,
            n: 0,
            hist: SampleRing::new(PRE_ROLL).expect("a ring of a fixed size"),
            open: false,
            buf: Vec::new(),
            quiet: 0,
            t0: 0.0,
        }
    }
}

/// `ws.bank`: the polyphase filter bank of `sdrkit-algo-dsp`, with a burst detector for each band that carries channels.
pub struct Bank {
    rate: f64,
    centre: f64,
    m: usize,
    bank: Option<PolyphaseBank>,
    outs: Vec<Vec<Complex32>>,
    read: Vec<usize>,
    bands: Vec<BandState>,
    chans: Vec<Vec<(f64, f64, Vec<(u32, f32)>)>>,
    count: u64,
    plan: PlanTexts,
    out: Vec<WsBurst>,
    /// Bursts found.
    pub bursts: u64,
}

impl Default for Bank {
    fn default() -> Self {
        Bank {
            rate: 0.0,
            centre: 0.0,
            m: 4,
            bank: None,
            outs: Vec::new(),
            read: Vec::new(),
            bands: Vec::new(),
            chans: Vec::new(),
            count: 0,
            plan: PlanTexts::default(),
            out: Vec::new(),
            bursts: 0,
        }
    }
}

/// The usable ranges of the channel plan.
pub const RANGES: Setting<field::Text> = Setting::new("ranges", "Usable ranges", "", "").help(
    "Entries separated by \";\", each the lower and the upper edge in Hz separated by \"-\", for example \
     \"902000000-907500000;915000000-928000000\". At most 16 entries. Empty: every channel of the classes is usable.",
);

/// The operating classes of the channel plan.
pub const CLASSES: Setting<field::Text> = Setting::new("classes", "Operating classes", "", "").help(
    "Entries separated by \";\", one for each operating class in the order of the class numbers: the first channel in Hz, \
     the spacing in Hz, the number of channels and the FSK modes, separated by \"/\". The modes are separated by spaces, \
     each the bit rate in kbit/s and the modulation index separated by \":\", for example \
     \"915200000/200000/64/50:1;915400000/400000/32/100:0.5 150:0.5\". At most 16 entries. Empty: class 1 every 200 kHz \
     at 50 kbit/s and class 2 every 400 kHz at 100 and 150 kbit/s, on grids through the tuned frequency, without channel \
     numbers.",
);

/// The texts of [`RANGES`] and [`CLASSES`] and the plan they give (`None` when the classes are empty).
#[derive(Clone, Debug, Default)]
struct PlanTexts {
    ranges: String,
    classes: String,
    plan: Option<Plan>,
}

impl PlanTexts {
    /// Applies a value of [`RANGES`] or [`CLASSES`].
    fn apply(&mut self, s: PlanSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let (mut ranges, mut classes) = (self.ranges.clone(), self.classes.clone());
        match s {
            PlanSetting::Ranges => RANGES.read(id, v)?.clone_into(&mut ranges),
            PlanSetting::Classes => CLASSES.read(id, v)?.clone_into(&mut classes),
        }
        let plan =
            Plan::parse(&ranges, &classes).ok_or(SettingError::Malformed { id, at: None })?;
        self.plan = (!plan.classes.is_empty()).then_some(plan);
        (self.ranges, self.classes) = (ranges, classes);
        Ok(())
    }

    /// The plan in use for a capture centred on `centre_hz`: the plan set, or [`Plan::around`] the centre.
    fn in_use(&self, centre_hz: f64) -> Plan {
        self.plan.clone().unwrap_or_else(|| Plan::around(centre_hz))
    }

    fn values(&self) -> Values {
        values_of([
            (PlanSetting::Ranges, Value::Text(self.ranges.clone())),
            (PlanSetting::Classes, Value::Text(self.classes.clone())),
        ])
    }
}

sdrkit_core::settings! {
    /// The settings of [`Bank`] and [`Tables`].
    pub enum PlanSetting {
        /// The usable ranges.
        Ranges = RANGES,
        /// The operating classes.
        Classes = CLASSES,
    }
}

impl Bank {
    fn band_hz(&self, k: usize) -> f64 {
        let s = if k < self.m / 2 {
            k as f64
        } else {
            k as f64 - self.m as f64
        };
        s * self.rate / self.m as f64
    }
}

impl Component for Bank {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ws.bank",
            vec![Port::iq("in")],
            vec![Port::typed::<WsBurst>("bursts").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(PlanSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.centre = f.centre_hz;
        let mut m = 4;
        while m < 1024 && f.rate / (2.0 * m as f64) >= 250e3 {
            m *= 2;
        }
        self.m = m;
        let bw = f.rate / m as f64;
        let used: Vec<bool> = (0..m)
            .map(|k| self.band_hz(k).abs() <= f.rate / 2.0 - 0.75 * bw)
            .collect();
        let mut chans = vec![Vec::new(); m];
        let plan = self.plan.in_use(self.centre);
        for (hz, modes) in channels(&plan, self.centre, 0.45 * f.rate) {
            let off = hz - self.centre;
            let k = ((off / bw).round() as i64).rem_euclid(m as i64) as usize;
            if used[k] {
                let c = self.band_hz(k);
                chans[k].push((hz, off - c, modes));
            }
        }
        self.read = (0..m).filter(|&k| !chans[k].is_empty()).collect();
        self.chans = self.read.iter().map(|&k| chans[k].clone()).collect();
        self.bank = Some(PolyphaseBank::new(m, 16, 2, 80.0, &self.read)?);
        self.outs = vec![Vec::new(); self.read.len()];
        self.bands = self.read.iter().map(|_| BandState::new()).collect();
        self.count = 0;
        let mut o = f.with_kind(Kind::typed::<WsBurst>());
        o.rate = 0.0;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(bank) = self.bank.as_mut() else {
            return;
        };
        for o in &mut self.outs {
            o.clear();
        }
        if bank.process(io.iq(0), &mut self.outs).is_err() {
            return;
        }
        let brate = 2.0 * self.rate / self.m as f64;
        let n = self.outs.first().map_or(0, Vec::len);
        for (i, b) in self.bands.iter_mut().enumerate() {
            for (j, &z) in self.outs[i].iter().enumerate() {
                let idx = self.count + j as u64;
                b.p += (z.norm_sqr() - b.p) / 16.0;
                b.n += 1;
                b.hist.push(z);
                if b.n <= 65 {
                    b.floor = b.p;
                    continue;
                }
                if b.open {
                    b.buf.push(z);
                    b.quiet = if b.p < 2.5 * b.floor { b.quiet + 1 } else { 0 };
                    let longest = b.buf.len() as f64 > 0.35 * brate;
                    if b.quiet >= 160 || longest {
                        b.open = false;
                        // A burst that did not end within the longest frame: the floor was measured on other samples
                        // (those of the former rate after a change of rate, for example). The power now is the floor.
                        if b.quiet < 160 {
                            b.floor = b.p;
                        }
                        self.bursts += 1;
                        self.out.push(WsBurst {
                            samples: std::mem::take(&mut b.buf),
                            rate: brate,
                            channels: self.chans[i].clone(),
                            time: b.t0,
                        });
                    }
                    continue;
                }
                if b.p > 5.0 * b.floor.max(1e-9) {
                    b.open = true;
                    b.quiet = 0;
                    let end = b.hist.end();
                    let start = end.saturating_sub(PRE_ROLL as u64);
                    b.buf.resize((end - start) as usize, Complex32::default());
                    b.hist.copy(start, &mut b.buf);
                    b.t0 = (idx + 1).saturating_sub(end - start) as f64 / brate;
                } else if b.p < b.floor {
                    b.floor += (b.p - b.floor) / 512.0;
                } else {
                    b.floor += (b.p - b.floor) / 65536.0;
                }
            }
        }
        self.count += n as u64;
        for b in self.out.drain(..) {
            io.items_out::<WsBurst>(0).push(Timed {
                time: b.time,
                value: b,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = PlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        self.plan.apply(s, id, v)?;
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        self.plan.values()
    }
}

/// The result of a burst.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WsOut {
    /// A frame: its PSDU (FCS included, de-whitened), bit rate in kbit/s, channel centre (Hz), corrected by the least-reliable
    /// bit search.
    Frame(Frame154, u32, f64, bool),
    /// A mode switch with its PHY mode id.
    ModeSwitch(u8),
    /// An FEC frame (counted, not decoded).
    Fec,
    /// A bad frame.
    Bad,
}

impl Item for WsOut {
    const KIND: &'static str = "ws.out";
}

/// Demodulates a burst on one channel at one mode. `ch_off` is the channel's offset from the band centre,
/// `ch_hz` the channel centre, `kbps` the bit rate and `h` the modulation index. `None` when nothing was found.
pub fn demod(b: &WsBurst, ch_off: f64, ch_hz: f64, kbps: u32, h: f32) -> Option<WsOut> {
    let r = kbps as f64 * 1e3;
    if (b.samples.len() as f64) < 120.0 * b.rate / r {
        return None;
    }
    let cut = h as f64 * r / 2.0 + 0.35 * r;
    let ntaps = ((5.5 * b.rate / (0.6 * cut)).max(31.0) as usize) | 1;
    let taps = design_by_taps(ntaps, (cut / b.rate).min(0.5), Window::Blackman).ok()?;
    let d = ((b.rate / (5.0 * r)).floor() as usize).max(1);
    let r2 = b.rate / d as f64;
    // the channel to 0 Hz by an oscillator, then a zero-phase low-pass of the stored burst, kept every d-th sample
    let mut mixed = b.samples.clone();
    Nco::new(b.rate, -ch_off).mix_in_place(&mut mixed);
    let mut filtered = vec![Complex32::default(); mixed.len()];
    Convolver::new(&taps)
        .ok()?
        .centred(&mixed, &mut filtered)
        .ok()?;
    let ys: Vec<Complex32> = filtered.iter().step_by(d).copied().collect();
    let m = ys.len();
    let mut f = vec![0f32; m];
    let mut disc = Discriminator::try_new(r2, 1.0).ok()?;
    disc.set_last(*ys.first()?);
    disc.process_into(&ys, &mut f);
    let spb = r2 / r;
    let w = (spb.floor() as usize).max(1);
    let mut avg = WindowSum::<f32>::new(w).ok()?;
    let a: Vec<f32> = f.iter().map(|&x| (avg.push(x) / w as f64) as f32).collect();
    // SFD search at 8 phases over the first 512 bits
    let mut best: Option<(f64, f32, bool)> = None;
    for ph in 0..8 {
        let t0 = ph as f64 * spb / 8.0;
        let mut vals: Vec<f32> = Vec::new();
        let mut t = t0;
        while (t as usize) < m && vals.len() < 512 {
            vals.push(a[t as usize]);
            t += spb;
        }
        let mut bits: Vec<u8> = Vec::new();
        for i in 0..vals.len() {
            let lo = i.saturating_sub(16);
            let lvl = if i > lo {
                vals[lo..i].iter().sum::<f32>() / (i - lo) as f32
            } else {
                0.0
            };
            bits.push((vals[i] > lvl) as u8);
            if bits.len() < 40 {
                continue;
            }
            let k = bits.len();
            let pre = bits[k - 40..k - 16]
                .iter()
                .fold(0u32, |x, &y| (x << 1) | y as u32);
            let sfd = bits[k - 16..]
                .iter()
                .fold(0u16, |x, &y| (x << 1) | y as u16);
            if (pre ^ 0x555555).count_ones() > 1 {
                continue;
            }
            let fec = (sfd ^ 0x6F4E).count_ones() <= 1;
            if (sfd ^ 0x904E).count_ones() <= 1 || fec {
                let end_t = t0 + (k - 1) as f64 * spb;
                let lvl = vals[k - 32..k].iter().sum::<f32>() / 32.0;
                let score: f32 = vals[k - 32..k].iter().map(|v| (v - lvl).abs()).sum();
                match best {
                    Some((bt, bs, _))
                        if end_t > bt + spb / 2.0
                            || (((end_t - bt).abs() <= spb / 2.0) && score <= bs) => {}
                    _ => best = Some((end_t, score, fec)),
                }
                break;
            }
        }
    }
    let (end_t, _, fec) = best?;
    if fec {
        return Some(WsOut::Fec);
    }
    // carrier and deviation from the preamble and SFD
    let at = |t: f64| a[(t.max(0.0) as usize).min(m - 1)];
    let mut off = (17..33).map(|j| at(end_t - j as f64 * spb)).sum::<f32>() / 16.0;
    let mut dev = (0..32)
        .map(|j| (at(end_t - j as f64 * spb) - off).abs())
        .sum::<f32>()
        / 32.0;
    if dev <= 0.0 {
        return None;
    }
    let mut t = end_t + spb;
    let mut prev = at(end_t) - off;
    let mut bits = Vec::new();
    let mut rel = Vec::new();
    let read = |count: usize,
                bits: &mut Vec<u8>,
                rel: &mut Vec<f32>,
                t: &mut f64,
                prev: &mut f32,
                off: &mut f32,
                dev: &mut f32|
     -> bool {
        for _ in 0..count {
            if *t as usize >= m {
                return false;
            }
            let v = at(*t) - *off;
            if (v > 0.0) != (*prev > 0.0) {
                let mid = ((at(*t - spb / 2.0) - *off) / *dev).clamp(-1.0, 1.0);
                let dir = if *prev > 0.0 { 1.0 } else { -1.0 };
                *t += 0.12 * (mid * dir) as f64 * spb / 2.0;
            }
            let v = at(*t) - *off;
            bits.push((v > 0.0) as u8);
            rel.push(v.abs() / *dev);
            *off += 0.01 * (at(*t) - (*off + if v > 0.0 { *dev } else { -*dev }));
            *dev += 0.01 * (v.abs() - *dev);
            *prev = v;
            *t += spb;
        }
        true
    };
    if !read(
        16, &mut bits, &mut rel, &mut t, &mut prev, &mut off, &mut dev,
    ) {
        return None;
    }
    let phrv = bits.iter().fold(0u16, |x, &y| (x << 1) | y as u16);
    let (ms, fcs2, white, len) = phr(phrv);
    let fl = if fcs2 { 2 } else { 4 };
    if len < fl + 3 || end_t + (17 + 8 * len) as f64 * spb > m as f64 + spb {
        return ms.then_some(WsOut::ModeSwitch(((phrv >> 5) & 0xFF) as u8));
    }
    let mut pb = Vec::new();
    let mut pr = Vec::new();
    if !read(
        8 * len,
        &mut pb,
        &mut pr,
        &mut t,
        &mut prev,
        &mut off,
        &mut dev,
    ) {
        return None;
    }
    let mut p = Vec::with_capacity(len);
    pack::pack(&pb, BitOrder::Lsb, Pad::Zero, &mut p);
    if white {
        pn9(&mut p);
    }
    let mut corrected = false;
    if !fcs_ok(&p, fcs2) {
        let mode_switch = || {
            Some(if ms {
                WsOut::ModeSwitch(((phrv >> 5) & 0xFF) as u8)
            } else {
                WsOut::Bad
            })
        };
        if fcs2 {
            return mode_switch();
        }
        match lrb_correct(&mut p, &pr) {
            Some(n) if n > 0 => corrected = true,
            _ => return mode_switch(),
        }
    }
    let lvl_a = ((end_t - 48.0 * spb).max(0.0) as usize).min(m);
    let lvl_b = (t as usize).min(m).max(lvl_a + 1);
    let level = ys[lvl_a.min(m.saturating_sub(1))..lvl_b.min(m)]
        .iter()
        .map(|s| s.norm_sqr() as f64)
        .sum::<f64>()
        / (lvl_b - lvl_a) as f64;
    let mse = pr.iter().map(|&x| ((x - 1.0) as f64).powi(2)).sum::<f64>() / pr.len().max(1) as f64;
    Some(WsOut::Frame(
        Frame154 {
            psdu: p,
            fcs_ok: true,
            phy: "SUN FSK".into(),
            channel: 0,
            freq_hz: ch_hz + off as f64,
            level_db: (10.0 * level.max(1e-20).log10()) as f32,
            snr_db: (10.0 * (1.0 / mse.max(1e-6)).log10()) as f32,
            offset_hz: off as f64,
            time: b.time + end_t / r2,
        },
        kbps,
        ch_hz,
        corrected,
    ))
}

/// Frames remembered to drop the same frame heard in two bands or on two channels.
const RECENT: usize = 64;

/// `ws.demod`: each burst on each of its channels and modes, fastest first. It drops a frame already decoded within 3 ms.
#[derive(Default)]
pub struct Demod {
    recent: VecDeque<(u32, f64)>,
    /// The pool of the engine: the bursts of one call are tried there. `None` tries them in line.
    pool: Option<sdrkit_graph::Pool>,
}

/// The result of a burst: the frame of the first channel and mode that hold one, else the last other result.
fn try_burst(b: &WsBurst) -> Option<WsOut> {
    let mut got = None;
    for (hz, off, modes) in &b.channels {
        for &(k, h) in modes {
            if let Some(o) = demod(b, *off, *hz, k, h) {
                let fin = matches!(o, WsOut::Frame(..));
                got = Some(o);
                if fin {
                    return got;
                }
            }
        }
    }
    got
}

impl Component for Demod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ws.demod",
            vec![Port::typed::<WsBurst>("in")],
            vec![Port::typed::<WsOut>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::typed::<WsOut>())])
    }
    fn attach(&mut self, ctx: &sdrkit_graph::RunContext) {
        self.pool = Some(ctx.pool.clone());
    }
    fn process(&mut self, io: &mut Io) {
        let mut out = Vec::new();
        let bursts = io.items::<WsBurst>(0);
        // The result of a burst depends on that burst alone. The filter of repeats below sees the results in order.
        let results: Vec<Option<WsOut>> = match &self.pool {
            Some(pool) if bursts.len() > 1 => pool.map(bursts.to_vec(), |_, b| try_burst(&b.value)),
            _ => bursts.iter().map(|b| try_burst(&b.value)).collect(),
        };
        for (b, got) in bursts.iter().zip(results) {
            let Some(o) = got else { continue };
            if let WsOut::Frame(f, ..) = &o {
                let n = f.psdu.len();
                let c = u32::from_le_bytes([
                    f.psdu[n - 4],
                    f.psdu[n - 3],
                    f.psdu[n - 2],
                    f.psdu[n - 1],
                ]);
                if self
                    .recent
                    .iter()
                    .any(|&(x, t)| x == c && (t - f.time).abs() < 3e-3)
                {
                    continue;
                }
                if self.recent.len() == RECENT {
                    self.recent.pop_front();
                }
                self.recent.push_back((c, f.time));
            }
            out.push(Timed {
                time: b.time,
                value: o,
            });
        }
        io.items_out::<WsOut>(0).extend(out);
    }
}

/// Networks kept.
pub const NETWORK_CAP: usize = 64;
/// Nodes kept.
pub const NODE_CAP: usize = 2000;
/// Links kept.
pub const LINK_CAP: usize = 4000;
/// Frames kept in the log.
pub const FRAME_CAP: usize = 800;
/// Points of the hop chart.
pub const HOP_CAP: usize = 3000;
/// New PANs that each give an event.
pub const PAN_EVENTS: u32 = 20;

/// A network, keyed by its PAN id.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
    /// The network name.
    pub name: Option<String>,
    /// The PAN size.
    pub size: Option<u16>,
    /// The PAN version.
    pub version: Option<u16>,
    /// The FAN TPS version.
    pub tps: Option<u8>,
    /// The border router EUI-64.
    pub border_router: Option<String>,
    /// The unicast schedule.
    pub us: Option<String>,
    /// The broadcast schedule.
    pub bs: Option<String>,
    /// The PHY operating modes.
    pub pom: Vec<u8>,
    /// The nodes in the PAN.
    pub nodes: usize,
    /// The last level, dBFS.
    pub level_db: f32,
}

impl Shift for Network {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Network {
    type Row = WisunNetworkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WisunNetworkRow {
            name: self.name.as_deref(),
            border_router: self.border_router.as_deref(),
            size: self.size.map(i64::from),
            nodes: self.nodes as i64,
            level_dbfs: f64::from(self.level_db),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
            schedule: self.us.as_deref(),
        });
    }
}

/// A PAN id as the key of a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PanId(pub u16);

impl fmt::Display for PanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:04X}", self.0)
    }
}

/// An EUI-64 as the key of a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Eui(pub u64);

impl fmt::Display for Eui {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&eui(self.0))
    }
}

/// A node, keyed by its EUI-64.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The vendor of the EUI-64.
    pub vendor: Option<String>,
    /// The PAN.
    pub pan: Option<u16>,
    /// The role: node, LFN, joining, router or border router.
    pub role: String,
    /// The routing cost.
    pub cost: Option<u16>,
    /// The last level, dBFS.
    pub level_db: f32,
    /// The last SNR, dB.
    pub snr_db: f32,
    /// Frames sent.
    pub frames: u64,
    /// Encrypted frames sent.
    pub encrypted: u64,
    /// The last frame counter.
    pub counter: Option<u32>,
    /// The last channel centre.
    pub freq_hz: f64,
    /// The last bit rate, kbit/s.
    pub kbps: u32,
    /// The authenticator EUI-64.
    pub authenticator: Option<String>,
    /// The EAP state.
    pub eap: Option<String>,
    /// The last summary.
    pub last: String,
}

impl Shift for Node {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Node {
    type Row = WisunNodeRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let pan = self.pan.map(|p| PanId(p).to_string());
        out.put(&WisunNodeRow {
            vendor: self.vendor.as_deref(),
            role: &self.role,
            pan: pan.as_deref(),
            cost: self.cost.map(i64::from),
            level_dbfs: f64::from(self.level_db),
            frames: i64::try_from(self.frames).unwrap_or(i64::MAX),
            last_heard: columns::last_heard(seen).map(Time),
            freq: Hertz(self.freq_hz),
            authenticator: self.authenticator.as_deref(),
            eap: self.eap.as_deref(),
            last: &self.last,
        });
    }
}

/// A directed link, keyed by its two ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkKey {
    /// The sender of the frames.
    pub from: u64,
    /// The receiver of the frames.
    pub to: u64,
}

impl fmt::Display for LinkKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} > {}", eui(self.from), eui(self.to))
    }
}

/// A link.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// The sender's EUI-64.
    pub from: String,
    /// The receiver's EUI-64.
    pub to: String,
    /// Frames from the sender.
    pub frames: u64,
    /// Acks from the receiver.
    pub acks: u64,
    /// The level the receiver reported for the sender's frames, dBm.
    pub rsl: Option<i16>,
    /// The level of the sender here, dBFS.
    pub level_db: f32,
}

impl Shift for Link {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Link {
    type Row = WisunLinkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WisunLinkRow {
            from: &self.from,
            to: &self.to,
            frames: i64::try_from(self.frames).unwrap_or(i64::MAX),
            acks: i64::try_from(self.acks).unwrap_or(i64::MAX),
            rsl_dbm: self.rsl.map(f64::from),
            level_dbfs: f64::from(self.level_db),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// A line of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameLine {
    /// The signal time, seconds.
    pub at: f64,
    /// The channel centre.
    pub freq_hz: f64,
    /// The operating class and channel number in the plan of the bit rate.
    pub channel: Option<(usize, u32)>,
    /// The bit rate, kbit/s.
    pub kbps: u32,
    /// The UTT or 802.15.4 frame type name.
    pub kind: String,
    /// "source > destination".
    pub route: String,
    /// The PAN.
    pub pan: Option<u16>,
    /// The PSDU length with the FCS.
    pub len: usize,
    /// The level, dBFS.
    pub level_db: f32,
    /// The SNR, dB.
    pub snr_db: f32,
    /// Secured.
    pub encrypted: bool,
    /// The IE list.
    pub ies: String,
    /// The summary line.
    pub summary: String,
    /// The MPDU without the FCS, hex.
    pub hex: String,
}

impl Shift for FrameLine {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

impl Shaped for FrameLine {
    type Row = WisunFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let ch = self.channel.map(|(c, k)| format!("class {c} ch {k}"));
        out.put(&WisunFrameRow {
            time_s: self.at,
            freq: Hertz(self.freq_hz),
            ch: ch.as_deref(),
            kbps: i64::from(self.kbps),
            r#type: &self.kind,
            route: &self.route,
            level_dbfs: f64::from(self.level_db),
            details: &self.summary,
            mpdu: &self.hex,
        });
    }
}

/// A point of the hop chart.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hop {
    /// The signal time, seconds.
    pub at: f64,
    /// The channel centre, MHz.
    pub mhz: f64,
    /// The UTT frame type (255 when none).
    pub utt: u8,
    /// A colour index 0 to 11 from the source EUI-64.
    pub colour: u8,
}

impl Shift for Hop {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

/// What `ws.tables` keeps across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    networks: ObjectTable<PanId, Network>,
    nodes: ObjectTable<Eui, Node>,
    links: ObjectTable<LinkKey, Link>,
    frames: LogRing<FrameLine>,
    hops: LogRing<Hop>,
    good: u64,
    bad: u64,
    mode_switches: u64,
    modes: Vec<(u8, u64)>,
    fec: u64,
    corrected: u64,
    encrypted: u64,
    new_pans: u32,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            networks: ObjectTable::new(NETWORK_CAP),
            nodes: ObjectTable::new(NODE_CAP),
            links: ObjectTable::new(LINK_CAP),
            frames: LogRing::new(FRAME_CAP),
            hops: LogRing::new(HOP_CAP),
            good: 0,
            bad: 0,
            mode_switches: 0,
            modes: Vec::new(),
            fec: 0,
            corrected: 0,
            encrypted: 0,
            new_pans: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.networks.shift_times(by_s);
        self.nodes.shift_times(by_s);
        self.links.shift_times(by_s);
        self.frames.shift_times(by_s);
        self.hops.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.wisun", 1);
    fn clear(&mut self) {
        self.networks.clear();
        self.nodes.clear();
        self.links.clear();
        self.frames.clear();
        self.hops.clear();
        self.good = 0;
        self.bad = 0;
        self.mode_switches = 0;
        self.modes.clear();
        self.fec = 0;
        self.corrected = 0;
        self.encrypted = 0;
        self.new_pans = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.networks.adopt(saved.networks);
        self.nodes.adopt(saved.nodes);
        self.links.adopt(saved.links);
        self.frames.adopt(saved.frames);
        self.hops.adopt(saved.hops);
        self.good = saved.good;
        self.bad = saved.bad;
        self.mode_switches = saved.mode_switches;
        self.modes = saved.modes;
        self.fec = saved.fec;
        self.corrected = saved.corrected;
        self.encrypted = saved.encrypted;
        self.new_pans = saved.new_pans;
    }
}

/// The rank of a role: a node's role only rises.
fn role_rank(r: &str) -> u8 {
    match r {
        "border router" => 4,
        "router" => 3,
        "joining" => 2,
        "LFN" => 1,
        _ => 0,
    }
}

impl Store {
    /// Takes a good frame on the channel `hz` at `kbps`, numbered in `plan` when there is one. Returns the event of a new
    /// PAN.
    fn frame(
        &mut self,
        f: &Frame154,
        kbps: u32,
        hz: f64,
        corrected: bool,
        plan: Option<&Plan>,
        at: Stamp,
    ) -> Option<Event> {
        let n = f.psdu.len();
        let body = &f.psdu[..n.saturating_sub(4)];
        let m = mac154::parse(body)?;
        let (ies, rest) = parse_ies(body, &m);
        self.good += 1;
        if corrected {
            self.corrected += 1;
        }
        let enc = m.encrypted();
        if enc {
            self.encrypted += 1;
        }
        let pan = m
            .src_pan
            .filter(|&p| p != 0xFFFF)
            .or(m.dst_pan.filter(|&p| p != 0xFFFF));
        let utt = ies.utt.unwrap_or(255);
        let sum = summary(&m, &ies, rest);
        self.frames.push(FrameLine {
            at: f.time,
            freq_hz: hz,
            channel: plan.and_then(|p| frame::channel_number(p, hz, kbps)),
            kbps,
            kind: if ies.utt.is_some() {
                utt_name(utt).0.into()
            } else {
                m.type_name().into()
            },
            route: format!(
                "{} > {}",
                m.src.text(),
                if m.dst == Addr::None {
                    "broadcast".to_string()
                } else {
                    m.dst.text()
                }
            ),
            pan,
            len: n,
            level_db: f.level_db,
            snr_db: f.snr_db,
            encrypted: enc,
            ies: ies.list.join(" "),
            summary: sum.clone(),
            hex: crate::family::hex(body),
        });
        let src = frame::src_eui(&m);
        let colour = src
            .map(|e| (e.wrapping_mul(0x9E3779B97F4A7C15) >> 60) as u8 % 12)
            .unwrap_or(0);
        self.hops.push(Hop {
            at: f.time,
            mhz: hz / 1e6,
            utt,
            colour,
        });
        let mut event = None;
        if let Some(p) = pan {
            if let Ok(up) = self.networks.upsert(&PanId(p), at, Network::default) {
                if up.new && self.new_pans < PAN_EVENTS {
                    self.new_pans += 1;
                    let pan_id = PanId(p).to_string();
                    event = Some(WISUN_PAN.event(
                        f.time,
                        hz,
                        &WisunPanEvent {
                            source: &pan_id,
                            text: &format!("Wi-SUN: new PAN {pan_id}"),
                            family: Family::Sensor,
                            pan: &pan_id,
                        },
                    ));
                }
                let nw = up.row;
                nw.level_db = f.level_db;
                if let Some(nm) = &ies.netname {
                    nw.name = Some(nm.clone());
                }
                if let Some((size, cost, flags)) = ies.pan {
                    nw.size = Some(size);
                    nw.tps = Some(flags >> 5);
                    if cost == 0 && size > 0 {
                        nw.border_router = src.map(eui);
                    }
                }
                if ies.panver.is_some() {
                    nw.version = ies.panver;
                }
                if ies.us.is_some() && (utt == 0 || nw.us.is_none()) {
                    nw.us = ies.us.clone();
                }
                if ies.bs.is_some() {
                    nw.bs = ies.bs.clone();
                }
                if !ies.pom.is_empty() {
                    nw.pom = ies.pom.clone();
                }
            }
        }
        if let Some(e) = src {
            if let Ok(up) = self.nodes.upsert(&Eui(e), at, || Node {
                vendor: vendor_of_address(&e.to_be_bytes()).map(String::from),
                role: "node".into(),
                ..Default::default()
            }) {
                let nd = up.row;
                nd.frames += 1;
                nd.pan = pan.or(nd.pan);
                nd.level_db = f.level_db;
                nd.snr_db = f.snr_db;
                nd.freq_hz = hz;
                nd.kbps = kbps;
                nd.last.clone_from(&sum);
                if enc {
                    nd.encrypted += 1;
                    nd.counter = m.aux.as_ref().and_then(|a| a.counter);
                }
                if let Some(a) = ies.ea {
                    nd.authenticator = Some(eui(a));
                }
                if let Some(x) = &ies.eap {
                    nd.eap = Some(x.clone());
                }
                if let Some((_, cost, _)) = ies.pan {
                    nd.cost = Some(cost);
                }
                let role = match utt {
                    0 if ies.pan.is_some_and(|p| p.1 == 0 && p.0 > 0) => "border router",
                    0 | 2 | 7 | 9 => "router",
                    1 | 3 | 6 => "joining",
                    8 | 10 => "LFN",
                    _ => "node",
                };
                if role_rank(role) > role_rank(&nd.role) {
                    nd.role = role.into();
                }
            }
            if let Addr::Ext(d) = m.dst {
                if !self.nodes.contains(&Eui(d)) {
                    let _ = self.nodes.upsert(&Eui(d), at, || Node {
                        vendor: vendor_of_address(&d.to_be_bytes()).map(String::from),
                        role: "node".into(),
                        ..Default::default()
                    });
                }
                let is_ack = utt == 5 || m.ftype == 2;
                let key = if is_ack {
                    LinkKey { from: d, to: e }
                } else {
                    LinkKey { from: e, to: d }
                };
                if let Ok(up) = self.links.upsert(&key, at, || Link {
                    from: eui(key.from),
                    to: eui(key.to),
                    ..Default::default()
                }) {
                    let l = up.row;
                    if is_ack {
                        l.acks += 1;
                        l.rsl = ies.rsl.or(l.rsl);
                    } else {
                        l.frames += 1;
                        l.level_db = f.level_db;
                    }
                }
            }
        }
        let counts: Vec<(PanId, usize)> = self
            .networks
            .iter()
            .map(|r| {
                let p = r.key.0;
                let n = self
                    .nodes
                    .iter()
                    .filter(|x| x.record.pan == Some(p))
                    .count();
                (*r.key, n)
            })
            .collect();
        for (k, n) in counts {
            if let Some(nw) = self.networks.get_mut(&k) {
                nw.nodes = n;
            }
        }
        event
    }
}

/// `ws.tables`: networks, nodes, links, the frame log, the hop chart, the counters and the status.
pub struct Tables {
    published: Published,
    nodes_table: TableId,
    hops_series: Option<SeriesId>,
    summary: String,
    store: Store,
    events: Vec<Event>,
    plan: PlanTexts,
    now: f64,
    last: f64,
    heard: Option<(f32, f32)>,
    live: bool,
}

impl Default for Tables {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(WisunFields::SHAPE.fields)
                .with_table(&WISUN_NETWORKS)
                .with_table(&WISUN_NODES)
                .with_table(&WISUN_LINKS)
                .with_table(&WISUN_FRAMES)
                .with_series(&WISUN_HOPS),
        ));
        let decl = published.results().decl();
        let nodes_table = decl
            .table_id(WISUN_NODES.key)
            .expect("the tables of ws.tables are declared");
        let hops_series = decl.series_id(WISUN_HOPS.key);
        Tables {
            published,
            nodes_table,
            hops_series,
            summary: String::new(),
            store: Store::default(),
            events: Vec::new(),
            plan: PlanTexts::default(),
            now: 0.0,
            last: f64::NEG_INFINITY,
            heard: None,
            live: false,
        }
    }
}

impl Tables {
    fn publish(&mut self) {
        let s = &self.store;
        let state = (self.now - self.last < 10.0 && s.good > 0) as u8 * 2;
        self.summary.clear();
        if s.good > 0 {
            let _ = write!(
                self.summary,
                "{} networks, {} nodes, {} links, {} frames",
                s.networks.len(),
                s.nodes.len(),
                s.links.len(),
                s.good
            );
        } else {
            self.summary.push_str("listening for Wi-SUN frames");
        }
        let p = &mut self.published;
        set_status(p, LockState::from_level(state), &self.summary);
        {
            let r = p.results_mut();
            let heard = self.heard;
            r.update_status(|st| {
                let (level, snr) = (heard.map(|h| h.0), heard.map(|h| h.1));
                let changed = st.level_dbfs != level || st.snr_db != snr;
                st.level_dbfs = level;
                st.snr_db = snr;
                changed
            });
            if let Some(id) = self.hops_series {
                let pts = || s.hops.iter().map(|h| [h.at as f32, h.mhz as f32]);
                r.update_series(id, |series| {
                    if !series.line && series.points.iter().copied().eq(pts()) {
                        return false;
                    }
                    series.line = false;
                    series.points.clear();
                    series.points.extend(pts());
                    true
                });
            }
        }
        p.put(&WisunFields {
            frames_ok: s.good as i64,
            frames_bad: s.bad as i64,
            corrected_frames: s.corrected as i64,
            fec_frames: s.fec as i64,
            mode_switches: s.mode_switches as i64,
            encrypted_frames: s.encrypted as i64,
            networks: s.networks.len() as i64,
            nodes: s.nodes.len() as i64,
            links: s.links.len() as i64,
        });
        shaped::rows(&s.networks, &WISUN_NETWORKS, self.now, p);
        shaped::rows(&s.nodes, &WISUN_NODES, self.now, p);
        shaped::rows(&s.links, &WISUN_LINKS, self.now, p);
        shaped::log_rows(&s.frames, &WISUN_FRAMES, self.now, p);
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ws.tables",
            vec![Port::typed::<WsOut>("in")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(PlanSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &sdrkit_graph::RunContext) {
        self.live = ctx.live;
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for o in io.items::<WsOut>(0) {
            match &o.value {
                WsOut::Frame(f, k, hz, c) => {
                    let at = match crate::family::wall(self.live) {
                        Some(w) => Stamp::at(f.time).with_wall(w),
                        None => Stamp::at(f.time),
                    };
                    self.now = self.now.max(f.time);
                    self.last = f.time;
                    self.heard = Some((f.level_db, f.snr_db));
                    if let Some(e) = self
                        .store
                        .frame(f, *k, *hz, *c, self.plan.plan.as_ref(), at)
                    {
                        self.events.push(e);
                    }
                }
                WsOut::ModeSwitch(id) => {
                    let s = &mut self.store;
                    s.mode_switches += 1;
                    if let Some(m) = s.modes.iter_mut().find(|m| m.0 == *id) {
                        m.1 += 1;
                    } else if s.modes.len() < 16 {
                        s.modes.push((*id, 1));
                    }
                }
                WsOut::Fec => self.store.fec += 1,
                WsOut::Bad => self.store.bad += 1,
            }
            self.now = self.now.max(o.time);
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
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = PlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        self.plan.apply(s, id, v)?;
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        self.plan.values()
    }
    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => {
                self.store.clear();
                self.published.clear_rows(&WISUN_NETWORKS);
                self.published.clear_rows(&WISUN_NODES);
                self.published.clear_rows(&WISUN_LINKS);
                self.published.clear_rows(&WISUN_FRAMES);
            }
            TablesCommand::Remove => commands::remove(
                &mut self.store.nodes,
                self.nodes_table,
                &mut self.published,
                args,
                &WISUN_NODE_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        let s = &mut self.store;
        if !kept.table(WISUN_NETWORKS.key) {
            s.networks.clear();
            self.published.clear_rows(&WISUN_NETWORKS);
        }
        if !kept.table(WISUN_NODES.key) {
            s.nodes.clear();
            self.published.clear_rows(&WISUN_NODES);
        }
        if !kept.table(WISUN_LINKS.key) {
            s.links.clear();
            self.published.clear_rows(&WISUN_LINKS);
        }
        if !kept.table(WISUN_FRAMES.key) {
            s.frames.clear();
            self.published.clear_rows(&WISUN_FRAMES);
        }
        for (key, n) in [
            ("frames_ok", &mut s.good),
            ("frames_bad", &mut s.bad),
            ("corrected_frames", &mut s.corrected),
            ("fec_frames", &mut s.fec),
            ("mode_switches", &mut s.mode_switches),
            ("encrypted_frames", &mut s.encrypted),
        ] {
            if !kept.field(key) {
                *n = 0;
            }
        }
        s.hops.clear();
        self.last = f64::NEG_INFINITY;
        self.heard = None;
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
        Remove = WISUN_REMOVE.command,
    }
}

/// The span of the channels of the test signal.
pub const GEN_SPAN: Setting<field::Hz> =
    Setting::new("gen_span", "Span of channels used", "Hz", 4e6)
        .range(0.2e6, 13e6, 0.1e6)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `wisun` stack.
    pub enum WisunSetting {
        /// The usable ranges.
        Ranges = RANGES,
        /// The operating classes.
        Classes = CLASSES,
        /// The span of the channels of the test signal.
        GenSpan = GEN_SPAN,
        /// The centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
        /// The level sent.
        TxLevel = crate::tx::TX_LEVEL,
    }
}

/// The least sample rate.
pub const MIN_RATE_HZ: f64 = 2e6;

/// The `wisun` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("bank", Bank::default())
        .add("demod", Demod::default())
        .add("tables", Tables::default())
        .add("sender", crate::tx::FrameSender::<send::WisunFrames>::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("bank")
        .input("sender.in")
        .connect("bank", "demod")
        .connect("demod", "tables")
        .connect("sender.out", "tx.in")
        .output("tables.events")
        .output("sender.events")
        .clock(REPORT_S)
        .status_from("tables")
        // Whole bursts cross into the demodulation, and whole results into the tables.
        .first_stage("bank")
        .stage("bank", "demod", Stage::new("demod").ahead_bytes(8 << 20))
        .stage("demod", "tables", Stage::new("tables").ahead_bytes(1 << 20))
        .costs(&sdrkit_core::CostTable::parse(crate::COSTS), "wisun", "-")
        .build()?;
    StackGraph::of(crate::typed::shape("wisun"), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("Wi-SUN FAN", MIN_RATE_HZ))
        .route(RANGES.key(), &["bank.ranges", "tables.ranges", "sender.ranges"])?
        .route(CLASSES.key(), &["bank.classes", "tables.classes", "sender.classes"])?
        .route(crate::tx::TX_LEVEL.key(), &["sender.tx_level"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tables", "remove"),
        )
        .command("send", sdrkit_graph::CommandTarget::of("sender", "send"))
        .command("halt", sdrkit_graph::CommandTarget::of("sender", "halt"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

const BR: u64 = 0x00124BFFFE1A2B3C;
const RT: u64 = 0x000D6FFFFE445566;
const MT: u64 = 0x02A1B2FFFEC3D4E5;

fn ext_hdr(
    ftype: u8,
    seq: Option<u8>,
    dst: Addr,
    src: u64,
    src_pan: Option<u16>,
    secured: bool,
    ack: bool,
) -> Vec<u8> {
    let dm: u16 = match dst {
        Addr::None => 0,
        Addr::Short(_) => 2,
        Addr::Ext(_) => 3,
    };
    let fc: u16 = ftype as u16
        | (secured as u16) << 3
        | (ack as u16) << 5
        | (src_pan.is_none() as u16) << 6
        | (seq.is_none() as u16) << 8
        | 1 << 9
        | dm << 10
        | 2 << 12
        | 3 << 14;
    let mut v = fc.to_le_bytes().to_vec();
    if let Some(s) = seq {
        v.push(s);
    }
    match dst {
        Addr::Short(s) => v.extend(s.to_le_bytes()),
        Addr::Ext(e) => v.extend(e.to_le_bytes()),
        Addr::None => {}
    }
    if let Some(p) = src_pan {
        v.extend(p.to_le_bytes());
    }
    v.extend(src.to_le_bytes());
    v
}

fn wh(sub: u8, v: &[u8]) -> Vec<u8> {
    let mut c = vec![sub];
    c.extend_from_slice(v);
    frame::hie(0x2A, &c)
}

/// The UFSI of the frames of the test signal.
const TEST_UFSI: u32 = 0x56_3412;

/// The UTT-IE of frame type `t` with the 24-bit `ufsi`, least significant octet first.
fn utt_ie(t: u8, ufsi: u32) -> Vec<u8> {
    let u = ufsi.to_le_bytes();
    wh(0x01, &[t, u[0], u[1], u[2]])
}

/// The UTT-IE of frame type `t` of the test signal.
fn utt(t: u8) -> Vec<u8> {
    utt_ie(t, TEST_UFSI)
}

fn us_ie() -> Vec<u8> {
    frame::wp_long(1, &[255, 0, 0, 0x10, 0x08, 2])
}

fn mpx_eap(code: u8, ty: u8) -> Vec<u8> {
    frame::pie(
        0x3,
        &[
            0x01, 0x01, 0x00, 0x01, 0x02, 0x00, 0x00, 0x08, code, 1, 0x00, 0x08, ty,
        ],
    )
}

/// The generator schedule: (PSDU without FCS, sent at the slow mode of [`test_modes`], level dB, offset Hz).
pub fn schedule() -> Vec<(Vec<u8>, bool, f64, f64)> {
    let pan = 0x7A31u16;
    let pa = |src: u64, cost: u16, size: u16| {
        let mut f = ext_hdr(1, None, Addr::None, src, Some(pan), false, false);
        f.extend(utt(0));
        f.extend(frame::hie(0x7E, &[]));
        let mut wp = frame::wp_short(
            0x04,
            &[
                size as u8,
                (size >> 8) as u8,
                cost as u8,
                (cost >> 8) as u8,
                0x40 | 0x02,
            ],
        );
        wp.extend(frame::wp_short(0x05, b"Test-FAN"));
        wp.extend(us_ie());
        wp.extend(frame::wp_short(0x08, &[2, 3, 5]));
        f.extend(frame::pie(0x4, &wp));
        f
    };
    let mut v = vec![
        (pa(BR, 0, 3), false, 0.0, 3e3),
        (pa(RT, 128, 3), false, -4.4, -5e3),
    ];
    let mut pas = ext_hdr(1, None, Addr::None, MT, Some(0xFFFF), false, false);
    pas.extend(utt(1));
    pas.extend(frame::hie(0x7E, &[]));
    let mut wp = us_ie();
    wp.extend(frame::wp_short(0x05, b"Test-FAN"));
    pas.extend(frame::pie(0x4, &wp));
    v.push((pas, false, -8.0, 8e3));
    let mut e1 = ext_hdr(1, Some(1), Addr::Ext(RT), MT, None, false, false);
    e1.extend(utt(6));
    e1.extend(wh(0x09, &BR.to_be_bytes()));
    e1.extend(frame::hie(0x7E, &[]));
    e1.extend(mpx_eap(2, 1));
    v.push((e1, false, -8.0, 0.0));
    let mut e2 = ext_hdr(1, Some(2), Addr::Ext(MT), RT, None, false, false);
    e2.extend(utt(6));
    e2.extend(frame::hie(0x7E, &[]));
    e2.extend(mpx_eap(1, 13));
    v.push((e2, false, -4.4, 0.0));
    let mut pcs = ext_hdr(1, None, Addr::None, MT, Some(pan), false, false);
    pcs.extend(utt(3));
    pcs.extend(frame::hie(0x7F, &[]));
    v.push((pcs, false, -8.0, 0.0));
    let secured =
        |src: u64, dst: Addr, seq: Option<u8>, ctr: u32, ies: Vec<u8>, n: usize, ack: bool| {
            let mut f = ext_hdr(
                1,
                seq,
                dst,
                src,
                if matches!(dst, Addr::None) {
                    Some(pan)
                } else {
                    None
                },
                true,
                ack,
            );
            f.push(0x0E);
            f.extend(ctr.to_le_bytes());
            f.push(1);
            f.extend(ies);
            f.extend(frame::hie(0x7E, &[]));
            f.extend((0..n).map(|i| (i as u8).wrapping_mul(29).wrapping_add(7)));
            f
        };
    for (src, ctr, bsn) in [(BR, 417967u32, 0x3995u16), (RT, 736547, 0x3996)] {
        let mut ies = utt(2);
        let b = bsn.to_le_bytes();
        ies.extend(wh(0x02, &[b[0], b[1], 181, 0, 0]));
        ies.extend(wh(0x80, &[0x60, 0xEA, 0x00]));
        v.push((
            secured(src, Addr::None, None, ctr, ies, 77, false),
            false,
            if src == BR { 0.0 } else { -4.4 },
            0.0,
        ));
    }
    for (k, n) in [40usize, 50, 60].iter().enumerate() {
        let seq = 10 + k as u8;
        v.push((
            secured(
                MT,
                Addr::Ext(RT),
                Some(seq),
                1000 + k as u32,
                utt(4),
                *n,
                true,
            ),
            false,
            -8.0,
            0.0,
        ));
        let mut ack = ext_hdr(1, Some(seq), Addr::Ext(MT), RT, None, false, false);
        ack.extend(utt(5));
        ack.extend(wh(0x04, &[(174 - 88 + k as i16) as u8]));
        ack.extend(frame::hie(0x7F, &[]));
        v.push((ack, false, -4.4, 0.0));
    }
    let mut ies = utt(4);
    ies.extend(wh(0x02, &[0x96, 0x39, 22, 0, 0]));
    v.push((
        secured(BR, Addr::Short(0xFFFF), Some(0x55), 417968, ies, 90, false),
        false,
        0.0,
        0.0,
    ));
    let mut pcs = ext_hdr(1, None, Addr::None, RT, Some(pan), false, false);
    pcs.extend(utt(3));
    pcs.extend(frame::hie(0x7F, &[]));
    v.push((pcs, true, -4.4, 0.0));
    v
}

/// How a test frame is sent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameOptions {
    /// A 2-octet FCS (else 4 octets).
    pub fcs2: bool,
    /// No whitening.
    pub no_white: bool,
    /// A bad FCS (the last octet XOR 0x5A).
    pub bad: bool,
    /// The mode-switch bit set in the PHR.
    pub mode_switch: bool,
}

/// The bits of a frame as sent: 8 preamble octets (0101...), SFD 0x904E and the PHR most significant bit first, then the
/// whitened PSDU with its FCS, least significant bit first in each octet.
pub fn frame_bits(psdu: &[u8], o: FrameOptions) -> Vec<u8> {
    let mut p = psdu.to_vec();
    add_fcs(&mut p, o.fcs2);
    if o.bad {
        if let Some(l) = p.last_mut() {
            *l ^= 0x5A;
        }
    }
    if !o.no_white {
        pn9(&mut p);
    }
    let len = p.len() as u16;
    let phrv: u16 = (o.mode_switch as u16) << 15
        | (o.fcs2 as u16) << 12
        | (!o.no_white as u16) << 11
        | (len & 0x7FF);
    let mut bits: Vec<u8> = Vec::with_capacity(64 + 32 + 8 * p.len());
    for _ in 0..8 {
        bits.extend([0, 1, 0, 1, 0, 1, 0, 1]);
    }
    pack::push_u64(&mut bits, 0x904E, 16);
    pack::push_u64(&mut bits, phrv as u64, 16);
    pack::unpack(&p, BitOrder::Lsb, &mut bits);
    bits
}

/// The clean samples of a frame at `kbps` and index `h`: 2-GFSK with BT 0.5 (`sdrkit_algo_sim::modulate::Cpm`), unit power. One
/// more bit of the last level follows the frame, and the carrier ramps in over the first bit and out over that extra bit.
pub fn frame_samples(
    psdu: &[u8],
    kbps: f64,
    h: f64,
    rate: f64,
    o: FrameOptions,
) -> Result<Vec<Complex32>, ConfigError> {
    let sps = rate / (kbps * 1e3);
    let mut m = Cpm::gfsk(0.5, h, sps)?
        .with_held_edges()
        .with_ramp(Ramp::RaisedCosine(sps))?;
    let mut levels: Vec<f32> = frame_bits(psdu, o)
        .iter()
        .map(|&b| if b != 0 { 1.0 } else { -1.0 })
        .collect();
    if let Some(&l) = levels.last() {
        levels.push(l);
    }
    let mut out = Vec::with_capacity((levels.len() as f64 * sps) as usize + 8);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    Ok(out)
}

/// The centre of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new(
    "gen_centre_hz",
    "Test signal radio centre",
    "Hz",
    0.0,
)
.help("The frequency that the radio is tuned to: the test signal puts the channels of the plan around it.")
.section("Test signal")
.advanced();

/// The two FSK modes of the test signal on the channels `near` (as [`channels`] gives them): the hopping mode, the fastest
/// mode of at most 100 kbit/s (the slowest mode when every mode is faster), and the slow mode, the slowest mode. `None`
/// when no channel has a mode.
pub fn test_modes(near: &[(f64, Vec<(u32, f32)>)]) -> Option<((u32, f32), (u32, f32))> {
    let all = || near.iter().flat_map(|c| c.1.iter().copied());
    let slow = all().min_by_key(|m| m.0)?;
    let hop = all()
        .filter(|m| m.0 <= 100)
        .max_by_key(|m| m.0)
        .unwrap_or(slow);
    Some((hop, slow))
}

/// The `wisun` generator: the test schedule in a loop, the frames hopping over the channels of `plan` within `half_span_hz`
/// of `centre_hz` that carry the hopping mode of [`test_modes`], and the slow frames on a channel with the slow mode. The
/// clean signal only, at unit power.
///
/// # Errors
///
/// [`ConfigError::RateTooLow`] if a channel of the plan lies within `half_span_hz` of `centre_hz` but outside the band of
/// the rate; [`ConfigError::Stage`] with a [`PlanError`] if no channel of the plan lies within `half_span_hz`; and the
/// errors of the scene.
pub fn generator(
    rate: f64,
    half_span_hz: f64,
    plan: &Plan,
    centre_hz: f64,
) -> Result<CleanScene, ConfigError> {
    let lim = half_span_hz.min(0.45 * rate - 200e3);
    let near = channels(plan, centre_hz, lim.max(0.0));
    let Some((fast, slow_mode)) = test_modes(&near) else {
        return Err(no_channel_error(rate, half_span_hz, plan, centre_hz));
    };
    let with = |m: (u32, f32)| -> Vec<f64> {
        near.iter()
            .filter(|c| c.1.contains(&m))
            .map(|c| c.0)
            .collect()
    };
    let hops = with(fast);
    let slows = with(slow_mode);
    let sched = schedule();
    let mut scene = Scene::new(rate)?;
    let mut t = 0.002f64;
    let mut hop = 0usize;
    let mut k = 0usize;
    scene.add_source(Box::new(move || {
        let (psdu, slow, lvl, off) = &sched[k % sched.len()];
        k += 1;
        let (hz, (kb, hi)) = if *slow {
            (slows[(hop * 5 + 1) % slows.len()], slow_mode)
        } else {
            let c = hops[hop % hops.len()];
            hop = (7 * hop + 3) % hops.len();
            (c, fast)
        };
        let samples = frame_samples(
            psdu,
            f64::from(kb),
            f64::from(hi),
            rate,
            FrameOptions::default(),
        )
        .ok()?;
        let b = Burst {
            start_s: t,
            offset_hz: hz - centre_hz + off,
            level_db: *lvl,
            samples,
        };
        t += b.samples.len() as f64 / rate + 0.003;
        Some(b)
    }));
    let info = SignalInfo::new(400e3)
        .with_symbol_rate(100e3)
        .with_bit_rate(100e3);
    Ok(CleanScene::new(scene, rate, info))
}

/// The test signal has no channel of the plan to send on.
#[derive(Clone, Debug, PartialEq)]
pub enum PlanError {
    /// The frequency is in no usable range of the plan, and no channel lies within the span. `nearest` is the plan range
    /// (low, high) nearest to the frequency, Hz; `None` when the plan has no channel.
    OutsidePlan {
        /// The frequency, Hz.
        hz: f64,
        /// The nearest range, Hz.
        nearest: Option<(f64, f64)>,
    },
    /// The frequency is in the plan, but no channel lies within `half_span_hz` of it.
    NoChannelInSpan {
        /// The frequency, Hz.
        hz: f64,
        /// Half the span of the test signal, Hz.
        half_span_hz: f64,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mhz = |hz: f64| hz / 1e6;
        match *self {
            PlanError::OutsidePlan { hz, nearest: Some((lo, hi)) } => write!(
                f,
                "{:.3} MHz is in no channel plan of the region; the nearest plan range is {:.3} to {:.3} MHz",
                mhz(hz),
                mhz(lo),
                mhz(hi)
            ),
            PlanError::OutsidePlan { hz, nearest: None } => write!(
                f,
                "{:.3} MHz is in no channel plan of the region; the plan has no channel",
                mhz(hz)
            ),
            PlanError::NoChannelInSpan { hz, half_span_hz } => write!(
                f,
                "no channel of the plan lies within {:.3} MHz of {:.3} MHz",
                mhz(half_span_hz),
                mhz(hz)
            ),
        }
    }
}

impl std::error::Error for PlanError {}

/// The ranges of `plan` (low, high), Hz: the `ranges` of the plan, or when it has none, the span of each class whose
/// channels carry a mode that the stack takes.
fn plan_ranges(plan: &Plan) -> Vec<(f64, f64)> {
    if !plan.ranges.is_empty() {
        return plan.ranges.clone();
    }
    plan.classes
        .iter()
        .filter(|c| c.count > 0 && c.modes.iter().any(|m| m.0 <= 150))
        .map(|c| (c.first_hz, c.first_hz + c.spacing_hz * f64::from(c.count - 1)))
        .collect()
}

/// The error of [`generator`] when no channel fits around `centre_hz`. A rate error only when a wider band of the rate
/// would reach a channel within `half_span_hz`; then `need` is the rate that reaches the nearest such channel.
fn no_channel_error(rate: f64, half_span_hz: f64, plan: &Plan, centre_hz: f64) -> ConfigError {
    let wide = channels(plan, centre_hz, half_span_hz.max(0.0));
    if test_modes(&wide).is_some() {
        let d = wide
            .iter()
            .map(|c| (c.0 - centre_hz).abs())
            .fold(f64::INFINITY, f64::min);
        return ConfigError::RateTooLow {
            need: MIN_RATE_HZ.max(((d + 200e3) / 0.45 / 1e3).ceil() * 1e3),
            got: rate,
        };
    }
    let ranges = plan_ranges(plan);
    let inside = ranges.iter().any(|&(lo, hi)| centre_hz >= lo && centre_hz <= hi);
    let e = if inside {
        PlanError::NoChannelInSpan { hz: centre_hz, half_span_hz }
    } else {
        let dist = |&(lo, hi): &(f64, f64)| (lo - centre_hz).max(centre_hz - hi);
        let nearest = ranges.iter().copied().min_by(|a, b| dist(a).total_cmp(&dist(b)));
        PlanError::OutsidePlan { hz: centre_hz, nearest }
    };
    ConfigError::Stage(StageError::new(e))
}

/// The half span of the test channels from settings that follow the schema of the stack.
pub fn gen_half_span(s: &Values) -> f64 {
    s.get(&GEN_SPAN) / 2.0
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    // At 0 the channels go around the frequency where the session listens.
    let centre = match s.get(&GEN_CENTRE_HZ) {
        c if c > 0.0 => c,
        _ => cfg.tuned_hz().unwrap_or(0.0),
    };
    let plan = match Plan::parse(s.get(&RANGES), s.get(&CLASSES)) {
        Some(p) if !p.classes.is_empty() => p,
        Some(_) => Plan::around(centre),
        None => {
            let id = s
                .schema()
                .id(CLASSES.key())
                .map_err(|_| ConfigError::NoRate)?;
            return Err(SettingError::Malformed { id, at: None }.into());
        }
    };
    Ok(Box::new(generator(
        cfg.rate_hz,
        gen_half_span(s),
        &plan,
        centre,
    )?))
}

/// Wi-SUN FAN.
pub static WISUN: StackDescriptor = StackDescriptor {
    id: "wisun",
    name: "Wi-SUN FAN",
    group: "IoT, sub-GHz",
    blurb: "Wi-SUN FAN smart utility networks: PAN adverts and configuration, nodes, links and header IEs; secured frames shown as encrypted.",
    tuning: Tuning { min_hz: 470e6, max_hz: 2484e6, no_offset_tuning: true, ..Tuning::anywhere(0.0, 20e6, 2e6, 18e6) },
    submodes: &[],
    settings: WisunSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::WiSun],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = o.seconds.max(0.5);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = WisunFields::read(r.fields());
        let n = |v: fn(&WisunFields) -> i64| f.as_ref().map_or(0, v);
        vec![
            at_least_value("frames_ok", n(|f| f.frames_ok), 10),
            at_least_value("networks", n(|f| f.networks), 1),
            at_least_value("links", n(|f| f.links), 1),
        ]
    });
    vec![c]
}

/// The verify case of `wisun`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the Wi-SUN receiver: 0.1 s of the test signal at 5 Msps (the filter bank, the burst detectors and the
/// demodulator), fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "ism.wisun_receiver_5m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 5e6;
    let mut x = vec![Complex32::default(); (rate * 0.1) as usize];
    if let Ok(mut g) = generator(rate, 2e6, &Plan::around(0.0), 0.0) {
        g.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&WisunSetting::schema()))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ = rx.configure(&sdrkit_core::InputInfo::new(rate, 0.0, 0.0).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.1, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame::Class;

    /// A plan of 902 to 928 MHz with channels every 200 kHz from 902.2 MHz.
    fn plan_915() -> Plan {
        Plan {
            ranges: vec![(902e6, 928e6)],
            classes: vec![Class {
                first_hz: 902.2e6,
                spacing_hz: 200e3,
                count: 129,
                modes: vec![(50, 1.0)],
            }],
        }
    }

    #[test]
    fn a_frequency_outside_the_plan_names_the_frequency_and_the_nearest_range() {
        let e = generator(5e6, 2e6, &plan_915(), 868e6).err().expect("no channel near 868 MHz");
        assert_eq!(
            e.to_string(),
            "868.000 MHz is in no channel plan of the region; the nearest plan range is 902.000 to 928.000 MHz"
        );
    }

    #[test]
    fn a_rate_too_low_for_a_channel_in_the_span_says_so() {
        let e = generator(300e3, 2e6, &plan_915(), 915.1e6).err().expect("the rate is too low");
        assert_eq!(e, ConfigError::RateTooLow { need: MIN_RATE_HZ, got: 300e3 });
        assert!(generator(MIN_RATE_HZ, 2e6, &plan_915(), 915.1e6).is_ok());
    }
}
