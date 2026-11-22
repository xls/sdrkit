//! Inmarsat Aero (`aero`): the P channels (600, 1200, 10500 bit/s) of one capture: log-ons, ACARS, ADS-C, aircraft positions, the
//! system table.
//! Standard: Inmarsat Aeronautical System Definition Manual; ARINC 741; ARINC 618, 620, 622, 745.

pub mod coding;
pub mod demod;
pub mod gen;
pub mod su;

use crate::display::Clock;
use crate::typed::{
    AeroAircraftArgs, AeroAircraftRow, AeroChannelRow, AeroFactRow, AeroFields, AeroLogonEvent,
    AeroLogonRow, AeroMessageEvent, AeroMessageRow, AeroSuTypeRow, AERO_AIRCRAFT, AERO_CHANNELS,
    AERO_FACTS, AERO_LOGON, AERO_LOGONS, AERO_MESSAGE, AERO_MESSAGES, AERO_REMOVE, AERO_SU_TYPES,
};
use demod::{AeroFrame, Msk, Oqpsk};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, HostRequest, LockProfile,
    LockState, Receiver, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet,
    StackConfig, StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_core::{Command, Record};
use sdrkit_graph::{
    no_values, values_of, Catalog, Decoder, DecoderComponent, Format, Graph, GraphBuilder,
    GraphError, Item, Kind, Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_aero::aircraft::{Aircraft, AircraftTable, Fix, Source};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::{Arc, OnceLock};
use su::{AeroMessage, Reassembler, SuInfo, SysFact};

/// The stack id.
pub const ID: &str = "aero";
/// The stack name.
pub const NAME: &str = "Inmarsat Aero";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 240e3;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// The most channels decoded at once.
pub const MAX_CHANNELS: usize = 12;
/// The most messages kept.
pub const MESSAGE_CAP: usize = 200;
/// The most log-ons kept.
pub const LOGON_CAP: usize = 100;
/// The most aircraft kept.
pub const AIRCRAFT_CAP: usize = 100;
/// The most system facts kept.
pub const FACT_CAP: usize = 24;

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum AeroItem {
    Frame {
        channel: usize,
        frame: Box<AeroFrame>,
        offset_hz: f64,
    },
    Su {
        channel: usize,
        rate: u32,
        su: [u8; 12],
        ok: bool,
        offset_hz: f64,
    },
    Message(Box<AeroMessage>),
    Logon {
        channel: usize,
        aes: u32,
        ges: u8,
        on: bool,
    },
    Fact(Box<SysFact>),
    SuType {
        t: u8,
        ok: bool,
    },
    Channels(Vec<ChannelRow>),
}
impl Item for AeroItem {
    const KIND: &'static str = "aero.item";
}

/// One channel's row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelRow {
    pub id: usize,
    pub offset_hz: f64,
    pub frequency_hz: f64,
    /// 0 while it is still being tried.
    pub rate: u32,
    /// 0 carrier seen, 1 carrier locked, 2 frames in sync, 3 data (SU checks good).
    pub state: u8,
    pub ebn0_db: f64,
    pub level_db: f64,
    pub loop_offset_hz: f64,
    pub frames: u64,
    pub uw_misses: u64,
    pub sus_good: u64,
    pub sus_bad: u64,
    pub channel_ber: f64,
    pub last_heard: f64,
}

struct Channel {
    id: usize,
    offset: f64,
    level: f64,
    rate: u32,
    m600: Option<Msk>,
    m1200: Option<Msk>,
    oq: Option<Oqpsk>,
    row: ChannelRow,
    last_sync: f64,
    unseen: u32,
    good_recent: u32,
}

/// The receiver: the band search and the channels.
pub struct AeroRx {
    rate: f64,
    centre: f64,
    limit: f64,
    fft: Option<Fft>,
    n: usize,
    win: Vec<f32>,
    frame: Vec<Complex32>,
    buf: Vec<Complex32>,
    pow: Vec<f32>,
    acc: Vec<f32>,
    prev_acc: Vec<f32>,
    frames: usize,
    skip: u64,
    samples: u64,
    next_eval: u64,
    candidates: Vec<(f64, f64, f64)>,
    chans: Vec<Channel>,
    next_id: usize,
    tmp: Vec<AeroFrame>,
    scratch: Vec<f32>,
}

impl Default for AeroRx {
    fn default() -> Self {
        AeroRx {
            rate: 0.0,
            centre: 0.0,
            limit: 0.0,
            fft: None,
            n: 0,
            win: Vec::new(),
            frame: Vec::new(),
            buf: Vec::new(),
            pow: Vec::new(),
            acc: Vec::new(),
            prev_acc: Vec::new(),
            frames: 0,
            skip: 1,
            samples: 0,
            next_eval: 0,
            candidates: Vec::new(),
            chans: Vec::new(),
            next_id: 1,
            tmp: Vec::new(),
            scratch: Vec::new(),
        }
    }
}

impl AeroRx {
    fn reset(&mut self) {
        self.n = ((self.rate / 250.0) as usize)
            .next_power_of_two()
            .clamp(1024, 131_072);
        self.fft = Some(Fft::forward(self.n));
        self.win = Window::Hann.coefficients(self.n);
        self.frame = Vec::with_capacity(self.n);
        self.pow = vec![0.0; self.n];
        self.acc = vec![0.0; self.n];
        self.prev_acc = vec![0.0; self.n];
        self.frames = 0;
        self.skip = ((self.rate / self.n as f64) / 16.0).floor().max(1.0) as u64;
        self.samples = 0;
        self.next_eval = self.rate as u64;
        self.candidates.clear();
        self.chans.clear();
    }

    /// The power spectrum of the frame just filled, with bin n/2 at 0 Hz, into `pow`.
    fn spectrum(&mut self) {
        let n = self.n;
        self.buf.clear();
        self.buf
            .extend(self.frame.iter().zip(&self.win).map(|(x, w)| x * *w));
        if let Some(f) = self.fft.as_mut() {
            f.process(&mut self.buf);
        }
        for (i, v) in self.buf.iter().enumerate() {
            self.pow[(i + n / 2) % n] = v.norm_sqr();
        }
    }

    /// The band search: this second and the previous one, smoothed over 3 bins; regions 3 dB over the median.
    fn search(&mut self, now: f64) {
        let n = self.n;
        let bin = self.rate / n as f64;
        let p: Vec<f32> = (0..n)
            .map(|i| {
                let a = |k: usize| self.acc[k] + self.prev_acc[k];
                (a(i.saturating_sub(1)) + a(i) + a((i + 1).min(n - 1))) / 3.0
            })
            .collect();
        let f = |i: usize| (i as f64 - (n / 2) as f64) * bin;
        let usable: Vec<usize> = (0..n)
            .filter(|&i| {
                f(i).abs() <= 0.45 * self.rate
                    && (self.limit <= 0.0 || f(i).abs() <= self.limit)
                    && f(i).abs() > 2e3
            })
            .collect();
        if usable.is_empty() {
            return;
        }
        let vals: Vec<f32> = usable.iter().map(|&i| p[i]).collect();
        // The median bin is the noise floor. A clean signal without noise has no floor: 50 dB below the strongest bin stands in.
        let peak = vals.iter().copied().fold(0.0f32, f32::max);
        let floor = percentile(&vals, 0.5, &mut self.scratch)
            .max(peak * 1e-5)
            .max(1e-30);
        let thr = floor * 2.0;
        let mut regions: Vec<(f64, f64, f64)> = Vec::new();
        let mut k = 0;
        while k < usable.len() {
            if p[usable[k]] > thr {
                let s = k;
                let mut e = k;
                let mut gap = 0;
                let mut j = k + 1;
                while j < usable.len() && usable[j] == usable[j - 1] + 1 {
                    if p[usable[j]] > thr {
                        e = j;
                        gap = 0;
                    } else {
                        gap += 1;
                        if gap > 2 {
                            break;
                        }
                    }
                    j += 1;
                }
                let width = (e - s + 1) as f64 * bin;
                if width <= 40e3 {
                    let (mut num, mut den, mut peak) = (0.0, 0.0, 0.0f32);
                    for &i in &usable[s..=e] {
                        let w = (p[i] - floor).max(0.0) as f64;
                        num += w * f(i);
                        den += w;
                        peak = peak.max(p[i]);
                    }
                    let mut level = db((peak / floor) as f64);
                    // A wide carrier spreads its power: a bonus in the order of new carriers.
                    if width >= 6e3 {
                        level += 8.0;
                    }
                    regions.push((num / den.max(1e-30), width, level));
                }
                k = e + 1;
            } else {
                k += 1;
            }
        }
        regions.sort_by(|a, b| b.2.total_cmp(&a.2));
        // Match to existing channels.
        for ch in self.chans.iter_mut() {
            ch.unseen += 1;
        }
        let mut fresh = Vec::new();
        for r in &regions {
            let hit = self.chans.iter_mut().find(|c| {
                let reach = if c.rate == 10500 {
                    7e3
                } else {
                    (r.1 / 2.0).max(1.5e3)
                };
                (c.offset - r.0).abs() <= reach
            });
            match hit {
                Some(c) => {
                    c.unseen = 0;
                    c.level = r.2;
                    // Not in sync for 3 s and moved: follow it.
                    if now - c.last_sync > 3.0 && (c.offset - r.0).abs() > 400.0 && c.rate == 0 {
                        c.offset = r.0;
                    }
                }
                None => fresh.push(*r),
            }
        }
        // A new carrier must show in two searches in a row.
        let prev = std::mem::take(&mut self.candidates);
        for r in fresh {
            if prev
                .iter()
                .any(|q| (q.0 - r.0).abs() < (r.1 / 2.0).max(1.5e3))
            {
                if self.chans.len() >= MAX_CHANNELS {
                    if r.1 < 6e3 {
                        continue;
                    }
                    // A wide carrier takes the weakest channel that holds no sync and has not lately.
                    let Some(i) = self
                        .chans
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| now - c.last_sync > 10.0 && !c.in_sync())
                        .min_by(|a, b| a.1.level.total_cmp(&b.1.level))
                        .map(|x| x.0)
                    else {
                        continue;
                    };
                    self.chans.remove(i);
                }
                let id = self.next_id;
                self.next_id += 1;
                self.chans.push(Channel::new(id, self.rate, r.0, r.2, now));
            } else {
                self.candidates.push(r);
            }
        }
        // Drop: unseen for 8 searches and no sync, or no sync for 40 s; of two in sync within 1.5 kHz the newer.
        self.chans
            .retain(|c| !((c.unseen >= 8 && !c.in_sync()) || now - c.last_sync > 40.0));
        let mut drop = Vec::new();
        for i in 0..self.chans.len() {
            for j in i + 1..self.chans.len() {
                let (a, b) = (&self.chans[i], &self.chans[j]);
                if a.in_sync() && b.in_sync() && (a.offset - b.offset).abs() < 1.5e3 {
                    drop.push(if a.id > b.id { a.id } else { b.id });
                }
            }
        }
        self.chans.retain(|c| !drop.contains(&c.id));
    }
}

impl Channel {
    fn new(id: usize, rate: f64, offset: f64, level: f64, now: f64) -> Self {
        Channel {
            id,
            offset,
            level,
            rate: 0,
            m600: Some(Msk::new(rate, offset, 600, 700.0)),
            m1200: Some(Msk::new(rate, offset, 1200, 700.0)),
            oq: Some(Oqpsk::new(rate, offset, 1500.0)),
            row: ChannelRow {
                id,
                offset_hz: offset,
                ..Default::default()
            },
            last_sync: now,
            unseen: 0,
            good_recent: 0,
        }
    }
    fn in_sync(&self) -> bool {
        self.m600.as_ref().is_some_and(|m| m.in_sync())
            || self.m1200.as_ref().is_some_and(|m| m.in_sync())
            || self.oq.as_ref().is_some_and(|m| m.in_sync())
    }
}

/// The band search looks only within this distance of the centre (0: the whole input).
pub const SEARCH_LIMIT_HZ: Setting<field::Hz> =
    Setting::new("search_limit_hz", "Search limit", "Hz", 0.0)
        .range(0.0, 10e6, 1.0)
        .help("The band search looks only within +-n Hz (0: the whole input).");

sdrkit_core::settings! {
    /// The settings of [`AeroRx`].
    pub enum AeroRxSetting {
        /// Search limit.
        SearchLimitHz = SEARCH_LIMIT_HZ,
    }
}

impl Decoder for AeroRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.aero_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<AeroItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        AeroRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(AeroRxSetting::SearchLimitHz, Value::Hz(self.limit))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match AeroRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AeroRxSetting::SearchLimitHz => self.limit = SEARCH_LIMIT_HZ.read(id, v)?,
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.start(f)?;
        Ok(vec![f.with_kind(Kind::typed::<AeroItem>())])
    }
    fn retune(&mut self, f: &Format) {
        self.centre = f.centre_hz;
        self.reset();
    }
    fn process(&mut self, iq: &[Complex32], _first: u64, out: &mut Outbox) {
        self.feed(iq, out.items::<AeroItem>(0));
    }
    fn tick(&mut self, out: &mut Outbox) {
        self.channels(out.items::<AeroItem>(0));
    }
}

impl AeroRx {
    /// Starts on the input `f`.
    pub fn start(&mut self, f: &Format) -> Result<(), ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.reset();
        Ok(())
    }

    /// Searches and demodulates `iq`, appending frames and SUs to `out`. Samples are counted from the last start.
    pub fn feed(&mut self, iq: &[Complex32], out: &mut Vec<Timed<AeroItem>>) {
        let first = self.samples;
        if self.fft.is_none() {
            return;
        }
        let mut done = 0usize;
        while done < iq.len() {
            let at = first + done as u64;
            let m = (self.next_eval.saturating_sub(at) as usize).clamp(1, iq.len() - done);
            let piece = &iq[done..done + m];
            let nn = self.n as u64;
            for (k, &v) in piece.iter().enumerate() {
                if ((at + k as u64) / nn).is_multiple_of(self.skip) {
                    self.frame.push(v);
                    if self.frame.len() == self.n {
                        self.spectrum();
                        for (a, p) in self.acc.iter_mut().zip(self.pow.iter()) {
                            *a += *p;
                        }
                        self.frames += 1;
                        self.frame.clear();
                    }
                }
            }
            let t_end = (at + m as u64) as f64 / self.rate;
            for ch in self.chans.iter_mut() {
                for rate in [600u32, 1200, 10500] {
                    self.tmp.clear();
                    match rate {
                        600 => {
                            if let Some(d) = &mut ch.m600 {
                                d.process(piece, &mut self.tmp)
                            }
                        }
                        1200 => {
                            if let Some(d) = &mut ch.m1200 {
                                d.process(piece, &mut self.tmp)
                            }
                        }
                        _ => {
                            if let Some(d) = &mut ch.oq {
                                d.process(piece, &mut self.tmp)
                            }
                        }
                    }
                    for f in self.tmp.drain(..) {
                        let sus: Vec<([u8; 12], bool)> = f
                            .info
                            .chunks(12)
                            .map(|c| {
                                let mut s = [0u8; 12];
                                s.copy_from_slice(c);
                                (s, su::ok(&s))
                            })
                            .collect();
                        let good = sus.iter().filter(|s| s.1).count();
                        if ch.rate == 0 && good * 2 >= sus.len() {
                            // This rate holds: the others stop.
                            ch.rate = rate;
                            if rate != 600 {
                                ch.m600 = None;
                            }
                            if rate != 1200 {
                                ch.m1200 = None;
                            }
                            if rate != 10500 {
                                ch.oq = None;
                            }
                        }
                        if ch.rate != rate {
                            continue;
                        }
                        ch.last_sync = t_end;
                        ch.row.frames += 1;
                        if f.uw_errors < 0 {
                            ch.row.uw_misses += 1;
                        }
                        ch.row.sus_good += good as u64;
                        ch.row.sus_bad += (sus.len() - good) as u64;
                        ch.row.channel_ber = f.channel_ber;
                        ch.row.ebn0_db = f.ebn0_db;
                        ch.row.loop_offset_hz = f.carrier_hz;
                        ch.row.last_heard = t_end;
                        ch.good_recent = good as u32 * 2 / sus.len().max(1) as u32;
                        let off = ch.offset + f.carrier_hz;
                        out.push(Timed {
                            time: t_end,
                            value: AeroItem::Frame {
                                channel: ch.id,
                                frame: Box::new(f.clone()),
                                offset_hz: off,
                            },
                        });
                        for (s, ok) in sus {
                            out.push(Timed {
                                time: t_end,
                                value: AeroItem::Su {
                                    channel: ch.id,
                                    rate,
                                    su: s,
                                    ok,
                                    offset_hz: off,
                                },
                            });
                        }
                    }
                }
                // A rate that lost sync for 20 s: try all three again.
                if ch.rate != 0 && t_end - ch.last_sync > 20.0 {
                    let (id, off, lvl) = (ch.id, ch.offset, ch.level);
                    *ch = Channel::new(id, self.rate, off, lvl, t_end);
                }
            }
            done += m;
            if first + done as u64 >= self.next_eval {
                self.next_eval += self.rate as u64;
                let now = (first + done as u64) as f64 / self.rate;
                self.search(now);
                std::mem::swap(&mut self.acc, &mut self.prev_acc);
                self.acc.iter_mut().for_each(|v| *v = 0.0);
                self.frames = 0;
            }
        }
        self.samples = first + iq.len() as u64;
    }
    /// Appends the channel table to `out`.
    pub fn channels(&mut self, out: &mut Vec<Timed<AeroItem>>) {
        let rows = self
            .chans
            .iter()
            .map(|c| {
                let mut r = c.row.clone();
                r.offset_hz = c.offset;
                r.frequency_hz = self.centre + c.offset;
                r.rate = c.rate;
                r.level_db = c.level;
                let locked = c.m600.as_ref().is_some_and(|m| m.locked_carrier)
                    || c.m1200.as_ref().is_some_and(|m| m.locked_carrier)
                    || c.oq.as_ref().is_some_and(|m| m.locked_carrier);
                r.state = if c.in_sync() && c.good_recent >= 1 {
                    3
                } else if c.in_sync() {
                    2
                } else if locked {
                    1
                } else {
                    0
                };
                r
            })
            .collect();
        out.push(Timed {
            time: self.samples as f64 / self.rate,
            value: AeroItem::Channels(rows),
        });
    }
}

/// The SU layer: SU types, log-ons, system facts, reassembly into messages.
#[derive(Default)]
pub struct SuStage {
    re: Reassembler,
    msgs: Vec<AeroMessage>,
}

impl Decoder for SuStage {
    type Input = Timed<AeroItem>;
    fn name(&self) -> &'static str {
        "satcom.aero_su"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<AeroItem>("out").same_format_as(0)]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![f.with_kind(Kind::typed::<AeroItem>())])
    }
    fn process(&mut self, items: &[Timed<AeroItem>], _first: u64, out: &mut Outbox) {
        let out = out.items::<AeroItem>(0);
        for it in items {
            self.item(it, out);
        }
    }
    fn clear(&mut self) {
        self.re.clear();
    }
}

impl SuStage {
    /// Takes one item of the receiver and appends its output to `out`.
    pub fn item(&mut self, it: &Timed<AeroItem>, out: &mut Vec<Timed<AeroItem>>) {
        match &it.value {
            AeroItem::Su {
                channel,
                rate,
                su,
                ok,
                offset_hz,
            } => {
                let t = it.time;
                if !*ok {
                    out.push(Timed {
                        time: t,
                        value: AeroItem::SuType {
                            t: su[0],
                            ok: false,
                        },
                    });
                    return;
                }
                let (info, _) = su::read(su);
                out.push(Timed {
                    time: t,
                    value: AeroItem::SuType { t: su[0], ok: true },
                });
                match &info {
                    SuInfo::Logon { aes, ges, on } => out.push(Timed {
                        time: t,
                        value: AeroItem::Logon {
                            channel: *channel,
                            aes: *aes,
                            ges: *ges,
                            on: *on,
                        },
                    }),
                    SuInfo::Fact(f) => out.push(Timed {
                        time: t,
                        value: AeroItem::Fact(Box::new(f.clone())),
                    }),
                    _ => {}
                }
                self.msgs.clear();
                self.re.push(&info, t, &mut self.msgs);
                for mut m in self.msgs.drain(..) {
                    m.channel = *channel;
                    m.rate = *rate;
                    m.carrier_hz = *offset_hz;
                    out.push(Timed {
                        time: t,
                        value: AeroItem::Message(Box::new(m)),
                    });
                }
            }
            _ => out.push(it.clone()),
        }
    }
}
// ------------------------------------------------------------------------------------------------------------------------------
// Results
// ------------------------------------------------------------------------------------------------------------------------------

sdrkit_core::choice_enum! {
    /// The state of a channel.
    pub enum ChannelState {
        /// A carrier was seen.
        Seen = ("seen", "Seen"),
        /// The carrier is locked.
        Carrier = ("carrier", "Carrier"),
        /// Frames decode.
        Frames = ("frames", "Frames"),
        /// Signal units decode.
        Data = ("data", "Data"),
    }
}

/// What the display publishes.
pub fn results_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(AeroFields::SHAPE.fields)
                .with_table(&AERO_CHANNELS)
                .with_table(&AERO_SU_TYPES)
                .with_table(&AERO_LOGONS)
                .with_table(&AERO_MESSAGES)
                .with_table(&AERO_AIRCRAFT)
                .with_table(&AERO_FACTS)
                .points("Aircraft"),
        )
    })
    .clone()
}

/// The frequency an aircraft was last heard on, Hz: the link data of the aircraft table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeardOn(pub f64);

impl Shift for HeardOn {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// Something heard at a moment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Heard<T> {
    /// Signal time, seconds.
    pub at: f64,
    /// Wall-clock time, when known.
    pub wall: Option<f64>,
    /// What was heard.
    pub item: T,
}

impl<T> Shift for Heard<T> {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

/// A log-on or log-off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Logon {
    /// The channel it came on.
    pub channel: usize,
    /// The AES id.
    pub aes: u32,
    /// The GES id.
    pub ges: u8,
    /// A log-on (else a log-off).
    pub on: bool,
}

impl Shaped for Heard<Logon> {
    type Row = AeroLogonRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let l = &self.item;
        let (mut aes, mut ges) = (String::with_capacity(8), String::with_capacity(4));
        let _ = write!(aes, "{:06X}", l.aes);
        let _ = write!(ges, "{:02X}", l.ges);
        out.put(&AeroLogonRow {
            at: Seconds(self.at),
            time: self.wall.map(Time),
            channel: l.channel as i64,
            aes: &aes,
            ges: &ges,
            logon: l.on,
        });
    }
}

/// `s`, or `None` when it is empty.
fn nonempty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// A message as kept, with its absolute frequency.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    /// The message.
    pub message: AeroMessage,
    /// The frequency it came on, Hz.
    pub frequency_hz: f64,
}

impl Heard<Kept> {
    /// The row of the message in the message table.
    fn row(&self) -> AeroMessageRow<'_> {
        let m = &self.item.message;
        let c = m.cpdlc.as_ref();
        AeroMessageRow {
            at: Seconds(self.at),
            time: self.wall.map(Time),
            channel: m.channel as i64,
            rate: i64::from(m.rate),
            frequency: Hertz(self.item.frequency_hz),
            aes: &m.aes,
            ges: &m.ges,
            registration: &m.registration,
            flight: &m.flight,
            label: &m.label,
            label_name: &m.label_name,
            message_number: &m.message_number,
            text: &m.text,
            check_ok: m.check_ok,
            complete: m.complete,
            adsc: &m.adsc,
            position: m.lat.zip(m.lon),
            alt_ft: m.alt_ft,
            reading: nonempty(&m.reading),
            reading_kind: nonempty(&m.reading_kind),
            reading_category: nonempty(&m.reading_category),
            cpdlc_imi: c.map(|c| c.imi.as_str()),
            cpdlc_ground: c.and_then(|c| nonempty(&c.ground)),
            cpdlc_aircraft: c.and_then(|c| nonempty(&c.aircraft)),
            cpdlc_crc_ok: c.map(|c| c.crc_ok),
            cpdlc_parsed: c.map(|c| c.parsed),
            cpdlc_text: c.and_then(|c| nonempty(&c.text)),
        }
    }
}

impl Shaped for SysFact {
    type Row = AeroFactRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AeroFactRow {
            kind: &self.kind,
            text: &self.text,
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

impl Shift for SysFact {
    /// A fact holds no signal times of its own (the table's `Seen` holds them).
    fn shift_times(&mut self, _by_s: f64) {}
}

/// What the display keeps across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    su_types: BTreeMap<u8, u64>,
    logons: LogRing<Heard<Logon>>,
    messages: LogRing<Heard<Kept>>,
    facts: ObjectTable<String, SysFact>,
    sus_good: u64,
    sus_bad: u64,
    positions: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            su_types: BTreeMap::new(),
            logons: LogRing::new(LOGON_CAP),
            messages: LogRing::new(MESSAGE_CAP),
            facts: ObjectTable::new(FACT_CAP),
            sus_good: 0,
            sus_bad: 0,
            positions: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.logons.shift_times(by_s);
        self.messages.shift_times(by_s);
        self.facts.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("satcom.aero", 1);
    fn clear(&mut self) {
        self.su_types.clear();
        self.logons.clear();
        self.messages.clear();
        self.facts.clear();
        self.sus_good = 0;
        self.sus_bad = 0;
        self.positions = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.su_types = saved.su_types;
        self.logons.adopt(saved.logons);
        self.messages.adopt(saved.messages);
        self.facts.adopt(saved.facts);
        self.sus_good = saved.sus_good;
        self.sus_bad = saved.sus_bad;
        self.positions = saved.positions;
    }
}

sdrkit_core::choice_enum! {
    /// The satellite whose P channel groups the host tunes to.
    pub enum Satellite {
        /// None: tune by hand.
        None = ("none", "None (tune by hand, 1545.57 MHz)"),
        /// Alphasat 25E.
        Alphasat = ("alphasat", "Alphasat 25E (Europe, Africa)"),
        /// Inmarsat-4 F3 98W.
        I4f3 = ("i4f3", "Inmarsat-4 F3 98W (Americas)"),
        /// Inmarsat-3 F5 54W.
        I3f5 = ("i3f5", "Inmarsat-3 F5 54W (Atlantic, South America)"),
        /// Inmarsat-6 F1 83.5E.
        I6f1 = ("i6f1", "Inmarsat-6 F1 83.5E (Indian Ocean, Asia-Pacific)"),
        /// Inmarsat-4 F1 143.5E.
        I4f1 = ("i4f1", "Inmarsat-4 F1 143.5E (Pacific, Asia)"),
    }
}

/// The dial of a satellite: the middle of its two P groups, Hz.
pub fn satellite_hz(s: Satellite) -> Option<f64> {
    match s {
        Satellite::Alphasat => Some((1545.115e6 + 1546.0275e6) / 2.0),
        Satellite::I4f3 => Some((1545.020e6 + 1546.0775e6) / 2.0),
        Satellite::I3f5 => Some((1545.025e6 + 1546.070e6) / 2.0),
        Satellite::I6f1 => Some((1545.160e6 + 1546.1225e6) / 2.0),
        Satellite::I4f1 => Some((1545.10e6 + 1546.05e6) / 2.0),
        Satellite::None => None,
    }
}

/// The satellite whose P channel groups the host tunes to.
pub const SATELLITE: Setting<field::Choice<Satellite>> =
    Setting::choice("satellite", "Satellite", Satellite::None).help(
        "Asks the host to tune the dial to the middle of the satellite's two P channel groups.",
    );
/// Shows the messages whose label holds this text.
pub const FILTER_LABEL: Setting<field::Text> = Setting::new("filter_label", "Label filter", "", "")
    .help("Shows the messages whose label holds this text.");
/// Shows the messages whose registration holds this text.
pub const FILTER_REGISTRATION: Setting<field::Text> =
    Setting::new("filter_registration", "Registration filter", "", "")
        .help("Shows the messages whose registration holds this text.");
/// Shows the messages whose block check failed too.
pub const SHOW_FAILED: Setting<field::Bool> =
    Setting::new("show_failed", "Show failed messages", "", false)
        .help("Shows messages whose block check failed too.");

sdrkit_core::settings! {
    /// The settings of [`AeroDisplay`].
    pub enum AeroDisplaySetting {
        /// Satellite.
        Satellite = SATELLITE,
        /// Label filter.
        FilterLabel = FILTER_LABEL,
        /// Registration filter.
        FilterRegistration = FILTER_REGISTRATION,
        /// Show failed messages.
        ShowFailed = SHOW_FAILED,
    }
}

/// The display: channels, SU types, log-ons, messages, aircraft, system facts.
pub struct AeroDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    centre_hz: f64,
    tuned_hz: f64,
    channels: Vec<ChannelRow>,
    store: Store,
    /// The aircraft. Each row keeps the frequency it was last heard on.
    pub aircraft: AircraftTable<HeardOn>,
    frames: u64,
    filter_label: String,
    filter_reg: String,
    show_failed: bool,
    satellite: Satellite,
    request: Option<HostRequest>,
    /// The tables changed since the last report.
    dirty: bool,
}

impl Default for AeroDisplay {
    fn default() -> Self {
        AeroDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            centre_hz: 0.0,
            tuned_hz: 0.0,
            channels: Vec::new(),
            store: Store::default(),
            aircraft: AircraftTable::new(AIRCRAFT_CAP),
            frames: 0,
            filter_label: String::new(),
            filter_reg: String::new(),
            show_failed: false,
            satellite: Satellite::None,
            request: None,
            dirty: true,
        }
    }
}

impl AeroDisplay {
    /// The absolute frequency of a channel, or of an offset from the input's centre.
    fn channel_hz(&self, channel: usize, offset_hz: f64) -> f64 {
        self.channels
            .iter()
            .find(|c| c.id == channel)
            .map_or(self.centre_hz + offset_hz, |c| c.frequency_hz)
    }

    fn shown(&self, m: &AeroMessage) -> bool {
        (self.show_failed || m.check_ok)
            && (self.filter_label.is_empty()
                || m.label
                    .to_lowercase()
                    .contains(&self.filter_label.to_lowercase()))
            && (self.filter_reg.is_empty()
                || m.registration
                    .to_lowercase()
                    .contains(&self.filter_reg.to_lowercase()))
    }

    fn item(&mut self, it: &Timed<AeroItem>, events: &mut Vec<sdrkit_core::Event>) {
        let t = self.clock.at(it.time);
        let wall = self.clock.wall(t);
        if !matches!(
            it.value,
            AeroItem::Channels(_) | AeroItem::Frame { .. } | AeroItem::Su { .. }
        ) {
            self.dirty = true;
        }
        match &it.value {
            AeroItem::Channels(c) => self.channels.clone_from(c),
            AeroItem::Frame { .. } => self.frames += 1,
            AeroItem::Su { .. } => {}
            AeroItem::SuType { t: ty, ok } => {
                let key = if *ok { *ty } else { 0xFE };
                *self.store.su_types.entry(key).or_insert(0) += 1;
                if *ok {
                    self.store.sus_good += 1;
                } else {
                    self.store.sus_bad += 1;
                }
            }
            AeroItem::Logon {
                channel,
                aes,
                ges,
                on,
            } => {
                let l = Logon {
                    channel: *channel,
                    aes: *aes,
                    ges: *ges,
                    on: *on,
                };
                self.store.logons.push(Heard {
                    at: t,
                    wall,
                    item: l,
                });
                let freq = self.channel_hz(*channel, 0.0);
                if let Some(a) = self.aircraft.heard(Some(*aes), "", self.clock.stamp(t)) {
                    a.extra = HeardOn(freq);
                }
                let mut src = String::with_capacity(8);
                let _ = write!(src, "{aes:06X}");
                let mut ges_text = String::with_capacity(4);
                let _ = write!(ges_text, "{ges:02X}");
                events.push(AERO_LOGON.event(
                    t,
                    freq,
                    &AeroLogonEvent {
                        source: &src,
                        text: if *on { "log-on" } else { "log-off" },
                        family: Family::Aircraft,
                        ges: &ges_text,
                        logon: *on,
                    },
                ));
            }
            AeroItem::Fact(f) => {
                if let Ok(up) = self
                    .store
                    .facts
                    .upsert(&f.key, self.clock.stamp(t), || (**f).clone())
                {
                    if !up.new {
                        up.row.text.clone_from(&f.text);
                    }
                }
            }
            AeroItem::Message(m) => {
                let m = (**m).clone();
                let freq = self.centre_hz + m.carrier_hz;
                if !m.non_acars {
                    if let Some(a) =
                        self.aircraft
                            .heard(m.icao, &m.registration, self.clock.stamp(t))
                    {
                        a.message(&m.flight, &m.label, &m.text);
                        a.extra = HeardOn(freq);
                        if let (Some(la), Some(lo)) = (m.lat, m.lon) {
                            let fix = Fix {
                                position: Position::new(la, lo),
                                alt_ft: m.alt_ft,
                                on_ground: false,
                                source: if m.adsc.is_empty() {
                                    Source::Text
                                } else {
                                    Source::Adsc
                                },
                            };
                            if a.position(t, fix, None).is_ok() {
                                self.store.positions += 1;
                            }
                        }
                    }
                }
                events.push(AERO_MESSAGE.event(
                    t,
                    freq,
                    &AeroMessageEvent {
                        source: nonempty(&m.registration),
                        text: &m.text,
                        family: Family::Aircraft,
                        position: m.lat.zip(m.lon),
                        label: &m.label,
                        aes: &m.aes,
                        flight: &m.flight,
                        check_ok: m.check_ok,
                    },
                ));
                self.store.messages.push(Heard {
                    at: t,
                    wall,
                    item: Kept {
                        message: m,
                        frequency_hz: freq,
                    },
                });
            }
        }
    }

    fn publish(&mut self, p: &mut Published) {
        let decoding: Vec<&ChannelRow> = self.channels.iter().filter(|c| c.state >= 2).collect();
        let state = if !decoding.is_empty() {
            2
        } else if !self.channels.is_empty() {
            1
        } else {
            0
        };
        let best = decoding
            .iter()
            .map(|c| c.ebn0_db)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut summary = String::new();
        let _ = match state {
            2 => write!(
                summary,
                "Inmarsat Aero: {} channels, Eb/N0 {:.1} dB, SUs {} ok {} bad, {} messages, {} aircraft",
                decoding.len(),
                best,
                self.store.sus_good,
                self.store.sus_bad,
                self.store.messages.len(),
                self.aircraft.len()
            ),
            1 => write!(summary, "Inmarsat Aero: {} carriers, none in sync", self.channels.len()),
            _ => write!(summary, "Inmarsat Aero: searching"),
        };
        let dial = self.tuned_hz - self.centre_hz;
        let near = decoding.iter().min_by(|a, b| {
            (a.offset_hz - dial)
                .abs()
                .total_cmp(&(b.offset_hz - dial).abs())
        });
        let now = self.clock.now();
        let lock_state = LockState::from_level(state);
        let offset = near.map(|c| c.offset_hz + c.loop_offset_hz - dial);
        let snr = near.map(|c| c.ebn0_db as f32);
        p.results_mut().update_status(|st| {
            let same = st.lock_state == lock_state
                && st.offset_hz == offset
                && st.snr_db == snr
                && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.offset_hz = offset;
                st.snr_db = snr;
                st.summary = summary;
            }
            !same
        });
        p.put(&AeroFields {
            frames_ok: self.store.sus_good as i64,
            frames_bad: self.store.sus_bad as i64,
            frames: self.frames as i64,
            messages: self.store.messages.len() as i64,
            positions: self.store.positions as i64,
            aircraft: self.aircraft.len() as i64,
            channels_decoding: decoding.len() as i64,
            ebn0_db: (state == 2).then_some(best),
        });
        shaped::indexed_rows(p, &AERO_CHANNELS, &self.channels, |c, out| {
            out.put(&AeroChannelRow {
                id: c.id as i64,
                offset_hz: Hertz(c.offset_hz),
                frequency: Hertz(c.frequency_hz),
                rate: i64::from(c.rate),
                state: match c.state {
                    3 => ChannelState::Data,
                    2 => ChannelState::Frames,
                    1 => ChannelState::Carrier,
                    _ => ChannelState::Seen,
                },
                ebn0_db: c.ebn0_db,
                level_db: c.level_db,
                frames: c.frames as i64,
                uw_misses: c.uw_misses as i64,
                sus_good: c.sus_good as i64,
                sus_bad: c.sus_bad as i64,
                ber: c.channel_ber,
            });
        });
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let mut key = String::new();
        for (&ty, &n) in &self.store.su_types {
            let name = if ty == 0xFE {
                su::type_name(0, false)
            } else {
                su::type_name(ty, true)
            };
            key.clear();
            let _ = write!(key, "{ty:02X}");
            let row = AeroSuTypeRow {
                su_type: i64::from(ty),
                su_name: name,
                count: n as i64,
            };
            AERO_SU_TYPES.put(p, &key, &row);
        }
        shaped::log_rows(&self.store.logons, &AERO_LOGONS, now, p);
        p.clear_rows(&AERO_MESSAGES);
        for (i, m) in self.store.messages.iter().enumerate() {
            if !self.shown(&m.item.message) {
                continue;
            }
            key.clear();
            let _ = write!(key, "{i:06}");
            AERO_MESSAGES.put(p, &key, &m.row());
        }
        shaped::rows_by(
            self.aircraft.rows(),
            &AERO_AIRCRAFT,
            p,
            |_| true,
            |r, out| {
                let mut address = [0u8; 7];
                let s = r.record.standard(Some(r.seen), &mut address);
                out.put(&AeroAircraftRow {
                    address: s.address,
                    registration: s.registration,
                    country: s.country,
                    flight: s.flight,
                    label: s.label,
                    messages: s.messages,
                    position: s.position,
                    alt_ft: s.alt_ft,
                    track_deg: s.track_deg,
                    gs_kt: s.gs_kt,
                    vs_fpm: s.vs_fpm,
                    last_heard: s.last_heard.map(Time),
                    frequency: Hertz(r.record.extra.0),
                });
            },
        );
        shaped::row_points_by(
            self.aircraft.rows(),
            AERO_AIRCRAFT.def(),
            "aircraft/",
            p.results_mut(),
            Aircraft::standard_point,
        );
        shaped::rows(&self.store.facts, &AERO_FACTS, now, p);
    }
}

sdrkit_core::commands! {
    /// The commands of [`AeroDisplay`].
    pub enum AeroDisplayCommand {
        /// Forgets one aircraft.
        Remove = AERO_REMOVE.command,
    }
}

impl Decoder for AeroDisplay {
    type Input = Timed<AeroItem>;
    fn name(&self) -> &'static str {
        "satcom.aero_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        AeroDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                AeroDisplaySetting::Satellite,
                Value::Choice(self.satellite.index()),
            ),
            (
                AeroDisplaySetting::FilterLabel,
                Value::Text(self.filter_label.as_str().into()),
            ),
            (
                AeroDisplaySetting::FilterRegistration,
                Value::Text(self.filter_reg.as_str().into()),
            ),
            (
                AeroDisplaySetting::ShowFailed,
                Value::Bool(self.show_failed),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match AeroDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AeroDisplaySetting::Satellite => {
                let c = SATELLITE.read(id, v)?;
                if c != self.satellite {
                    if let Some(hz) = satellite_hz(c) {
                        self.request = Some(HostRequest::retune(hz, "Inmarsat Aero P channels"));
                    }
                }
                self.satellite = c;
            }
            AeroDisplaySetting::FilterLabel => {
                self.filter_label = FILTER_LABEL.read(id, v)?.to_string()
            }
            AeroDisplaySetting::FilterRegistration => {
                self.filter_reg = FILTER_REGISTRATION.read(id, v)?.to_string()
            }
            AeroDisplaySetting::ShowFailed => self.show_failed = SHOW_FAILED.read(id, v)?,
        }
        self.dirty = true;
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(results_decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
        self.channels.clear();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
        self.channels.clear();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, items: &[Timed<AeroItem>], _first: u64, out: &mut Outbox) {
        let mut events = std::mem::take(out.messages(0));
        for it in items {
            self.item(it, &mut events);
        }
        *out.messages(0) = events;
        if let Some(r) = self.request.take() {
            out.items::<HostRequest>(1).push(Timed {
                time: self.clock.now(),
                value: r,
            });
        }
    }
    fn report(&mut self, results: &mut Published) {
        self.publish(results);
    }
    fn commands(&self) -> &'static [&'static Command] {
        AeroDisplayCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match AeroDisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AeroDisplayCommand::Remove => {
                let AeroAircraftArgs { row } = AERO_REMOVE.args(args)?;
                let key = sdrkit_util_objects::rows::key_of_row(self.aircraft.rows(), row)
                    .ok_or_else(|| self.results().no_such_row(&AERO_AIRCRAFT, row))?;
                self.aircraft.remove(&key);
            }
        }
        Ok(None)
    }
    fn clear(&mut self) {
        Tables::clear(&mut self.store);
        self.aircraft.clear();
        self.frames = 0;
        self.dirty = true;
    }
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.frames = 0;
        self.channels.clear();
        if !kept.table(AERO_SU_TYPES.key) {
            self.store.su_types.clear();
        }
        if !kept.table(AERO_LOGONS.key) {
            self.store.logons.clear();
        }
        if !kept.table(AERO_MESSAGES.key) {
            self.store.messages.clear();
        }
        if !kept.table(AERO_AIRCRAFT.key) {
            self.aircraft.clear();
        }
        if !kept.table(AERO_FACTS.key) {
            self.store.facts.clear();
        }
        if !kept.field("frames_ok") {
            self.store.sus_good = 0;
        }
        if !kept.field("frames_bad") {
            self.store.sus_bad = 0;
        }
        if !kept.field("positions") {
            self.store.positions = 0;
        }
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

sdrkit_core::settings! {
    /// The settings of the Inmarsat Aero stack. Noise and other impairments of the test signal come from the standard `gen_*`
    /// settings.
    pub enum AeroSetting {
        /// Search limit.
        SearchLimitHz = SEARCH_LIMIT_HZ,
        /// Satellite.
        Satellite = SATELLITE,
        /// Label filter.
        FilterLabel = FILTER_LABEL,
        /// Registration filter.
        FilterRegistration = FILTER_REGISTRATION,
        /// Show failed messages.
        ShowFailed = SHOW_FAILED,
        /// Channels of the test signal.
        GenChannels = gen::GEN_CHANNELS,
        /// Plain test signal.
        GenPlain = gen::GEN_PLAIN,
    }
}

/// The receiver's graph: the receiver, the SU layer and the display.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("receiver", DecoderComponent::new(AeroRx::default()))
        .add("su", DecoderComponent::new(SuStage::default()))
        .add("display", DecoderComponent::new(AeroDisplay::default()))
        .input("receiver.in")
        .connect("receiver.out", "su.in")
        .connect("su.out", "display.in")
        .output("display.events")
        .output("display.requests")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(catalog)
        .build()
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE,
            "Inmarsat Aero needs a sample rate of at least 240 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz + DESCRIPTOR.tuning.tune_offset_hz)
        .route("search_limit_hz", &["receiver.search_limit_hz"])?
        .route("satellite", &["display.satellite"])?
        .route("filter_label", &["display.filter_label"])?
        .route("filter_registration", &["display.filter_registration"])?
        .route("show_failed", &["display.show_failed"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut Registry) {
    r.add("satcom.aero_receiver", || {
        Box::new(DecoderComponent::new(AeroRx::default()))
    });
    r.add("satcom.aero_su", || {
        Box::new(DecoderComponent::new(SuStage::default()))
    });
    r.add("satcom.aero_display", || {
        Box::new(DecoderComponent::new(AeroDisplay::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::AeroGenerator::from_config(cfg)))
}

/// The engine's checks of the test signal: signal units check and messages arrive.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three P channels", o, 0.0);
    c.seconds = o.seconds.max(20.0);
    c.expect_audio = Some(false);
    c.check = Some(check_messages);
    vec![c]
}

fn check_messages(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let num = |k: &str| crate::field(r, k);
    vec![
        CheckLine::new(
            "frames_ok",
            num("frames_ok") >= 1.0,
            format!("{} signal units good", num("frames_ok")),
        ),
        CheckLine::new(
            "messages",
            num("messages") >= 1.0,
            format!("{} messages", num("messages")),
        ),
    ]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Inmarsat Aero P channels (600, 1200, 10500 bit/s): log-ons, ACARS, ADS-C and aircraft positions. Receive only.",
    tuning: Tuning {
        min_hz: 1525e6,
        max_hz: 1559e6,
        default_hz: 1545.57e6,
        rate_hz: 4e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 20e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(3.5e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: AeroSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::InmarsatAero],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
