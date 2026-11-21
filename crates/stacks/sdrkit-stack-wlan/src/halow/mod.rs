//! Wi-Fi HaLow (`halow`): the S1G beacons and frames of sub-GHz 802.11ah networks, heard with a bank of receiving chains, and a
//! network table and frame log.
//! Standard: IEEE Std 802.11-2020 clause 23 (S1G PHY) and clause 9 (MAC).

pub mod chain;
pub mod coding;
pub mod gen;
pub mod mac;
pub mod phy;
pub mod plan;
pub mod table;

use crate::testgen::route_to;
use chain::{ChainStats, S1gChain, S1gPpdu};
use phy::{width_index, width_rates};
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting,
    SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, Graph, GraphBuilder, GraphError, Item,
    Kind, Outbox, Port, Registry, StackGraph, Timed,
};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "halow";
/// The lowest rate, Hz.
pub const MIN_RATE_HZ: f64 = 2e6;

impl Item for S1gPpdu {
    const KIND: &'static str = "s1g.ppdu";
}

/// The chains' counters (kept across rebuilds), the widths tried and the time (the `s1g.stats` stream).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BankStats {
    /// Summed counters.
    pub chains: ChainStats,
    /// Widths tried, a bit per width index.
    pub width_mask: u32,
    /// Chains running.
    pub n_chains: u32,
    /// Signal time at the end of the piece, s.
    pub time_s: f64,
    /// The input rate, Hz.
    pub rate_hz: f64,
    /// The input level over the piece, dBFS.
    pub level_dbfs: Option<f64>,
}

impl Item for BankStats {
    const KIND: &'static str = "s1g.stats";
}

sdrkit_core::choice_enum! {
    /// The widths the receiver reads.
    pub enum Receive {
        /// The 1, 2 and 4 MHz widths.
        Auto14 = ("auto14", "Auto 1 - 4 MHz"),
        /// Every width.
        AutoAll = ("auto_all", "Auto all widths"),
        /// 1 MHz only.
        W1 = ("1", "1 MHz"),
        /// 2 MHz only.
        W2 = ("2", "2 MHz"),
        /// 4 MHz only.
        W4 = ("4", "4 MHz"),
        /// 8 MHz only.
        W8 = ("8", "8 MHz"),
        /// 16 MHz only.
        W16 = ("16", "16 MHz"),
    }
}

impl Receive {
    /// The one width read, MHz, `None` for the automatic choices.
    pub fn width_mhz(self) -> Option<u32> {
        match self {
            Receive::Auto14 | Receive::AutoAll => None,
            Receive::W1 => Some(1),
            Receive::W2 => Some(2),
            Receive::W4 => Some(4),
            Receive::W8 => Some(8),
            Receive::W16 => Some(16),
        }
    }
}

/// The bit mask of the widths that `receive` reads: a bit per width index.
pub fn receive_mask(receive: Receive) -> u32 {
    match receive {
        Receive::Auto14 => 0x07,
        Receive::AutoAll => 0x1F,
        Receive::W1 => 1,
        Receive::W2 => 2,
        Receive::W4 => 4,
        Receive::W8 => 8,
        Receive::W16 => 16,
    }
}

/// The rate that `receive` requires, Hz.
pub fn rate_for(receive: Receive) -> f64 {
    match receive.width_mhz() {
        Some(w) => width_rates(w).1,
        None if receive == Receive::AutoAll => 20e6,
        None => 6e6,
    }
}

/// The widths read. A change requests a new rate from the host and reopens the radio.
pub const RECEIVE: Setting<field::Choice<Receive>> =
    Setting::choice("receive", "Receive", Receive::Auto14)
        .restart()
        .help(
            "The widths read; the rate asked follows, and the radio is reopened when it changes.",
        );
/// Look for 1 and 2 MHz primary channels away from the centre of a wider capture.
pub const SUB_CHANNEL_SCAN: Setting<field::Bool> =
    Setting::new("sub_channel_scan", "Sub-channel scan", "", true)
        .help("Look for 1 and 2 MHz primary channels away from the centre of a wider capture.");

sdrkit_core::settings! {
    /// The settings of [`S1gBank`].
    pub enum BankSetting {
        /// The widths read.
        Receive = RECEIVE,
        /// The sub-channel scan.
        SubChannelScan = SUB_CHANNEL_SCAN,
    }
}

/// A delay-and-correlate detector at the radio rate (lag 8 µs, window 24 µs, 16 µs above 0.30, re-armed under 0.20): it wakes the sub-channel scan
/// chains, which run only around a detection.
pub struct WideDetector {
    lag: usize,
    win: usize,
    need: u32,
    hist: Vec<Complex32>,
    prods: Vec<Complex32>,
    p1: Vec<f32>,
    p2: Vec<f32>,
    k: usize,
    h: usize,
    c: (f64, f64),
    a: f64,
    b: f64,
    run: u32,
    armed: bool,
    n: u64,
}

impl WideDetector {
    /// A detector at `rate`.
    pub fn new(rate: f64) -> WideDetector {
        let lag = ((8e-6 * rate).round() as usize).max(1);
        let win = ((24e-6 * rate).round() as usize).max(2);
        WideDetector {
            lag,
            win,
            need: ((16e-6 * rate).round() as u32).max(1),
            hist: vec![Complex32::new(0.0, 0.0); lag],
            prods: vec![Complex32::new(0.0, 0.0); win],
            p1: vec![0.0; win],
            p2: vec![0.0; win],
            k: 0,
            h: 0,
            c: (0.0, 0.0),
            a: 0.0,
            b: 0.0,
            run: 0,
            armed: true,
            n: 0,
        }
    }
    /// Runs samples; detections (input indices) are appended.
    pub fn process(&mut self, x: &[Complex32], det: &mut Vec<u64>) {
        for &s in x {
            let d = self.hist[self.h];
            self.hist[self.h] = s;
            self.h = if self.h + 1 == self.lag {
                0
            } else {
                self.h + 1
            };
            let k = self.k;
            self.k = if k + 1 == self.win { 0 } else { k + 1 };
            let p = s * d.conj();
            let o = self.prods[k];
            self.prods[k] = p;
            self.c.0 += p.re as f64 - o.re as f64;
            self.c.1 += p.im as f64 - o.im as f64;
            let na = s.norm_sqr();
            self.a += na as f64 - self.p1[k] as f64;
            self.p1[k] = na;
            let nb = d.norm_sqr();
            self.b += nb as f64 - self.p2[k] as f64;
            self.p2[k] = nb;
            if k == 0 && (self.n & 16383) < self.win as u64 {
                self.c = self
                    .prods
                    .iter()
                    .fold((0.0, 0.0), |a, v| (a.0 + v.re as f64, a.1 + v.im as f64));
                self.a = self.p1.iter().map(|&v| v as f64).sum();
                self.b = self.p2.iter().map(|&v| v as f64).sum();
            }
            let c2 = self.c.0 * self.c.0 + self.c.1 * self.c.1;
            let pp = self.a * self.b;
            if self.a <= 1e-10 || self.b <= 1e-10 || c2 < 0.04 * pp {
                self.armed = true;
                self.run = 0;
            } else if c2 > 0.09 * pp {
                self.run += 1;
                if self.armed && self.run >= self.need {
                    self.armed = false;
                    det.push(self.n);
                }
            } else {
                self.run = 0;
            }
            self.n += 1;
        }
    }
}

/// The S1G receiver as a decoder (`wlan.halow_rx`): the main chains (4 Msps for 2, 1 and 4 MHz, 8 Msps for 8 MHz, 16 Msps for
/// 16 MHz) and the sub-channel scan chains. A new rate, centre or width mask rebuilds them, and the counters stay.
pub struct S1gBank {
    chains: Vec<S1gChain>,
    scans: Vec<(S1gChain, Option<u64>)>,
    wide: Option<WideDetector>,
    raw: Vec<Complex32>,
    raw_base: u64,
    n_in: u64,
    det: Vec<u64>,
    rate: f64,
    offset: f64,
    mask: u32,
    receive: Receive,
    scan: bool,
    kept: ChainStats,
    time: f64,
    out: Vec<S1gPpdu>,
    width_mask: u32,
}

impl Default for S1gBank {
    fn default() -> Self {
        S1gBank {
            chains: Vec::new(),
            scans: Vec::new(),
            wide: None,
            raw: Vec::new(),
            raw_base: 0,
            n_in: 0,
            det: Vec::new(),
            rate: 0.0,
            offset: 0.0,
            mask: receive_mask(RECEIVE.default),
            receive: RECEIVE.default,
            scan: SUB_CHANNEL_SCAN.default,
            kept: ChainStats::default(),
            time: 0.0,
            out: Vec::new(),
            width_mask: 0,
        }
    }
}

impl S1gBank {
    fn rebuild(&mut self) {
        for c in &self.chains {
            self.kept += c.stats;
        }
        for (c, _) in &self.scans {
            self.kept += c.stats;
        }
        if self.rate > 0.0 && self.n_in > 0 {
            self.time += self.n_in as f64 / self.rate;
        }
        self.chains.clear();
        self.scans.clear();
        self.raw.clear();
        self.raw_base = 0;
        self.n_in = 0;
        let rate = self.rate;
        let allowed =
            |w: u32| rate >= width_rates(w).0 * 0.999_999 && self.mask & (1 << width_index(w)) != 0;
        let main: Vec<u32> = [2, 1, 4].into_iter().filter(|&w| allowed(w)).collect();
        self.width_mask = 0;
        for &w in &main {
            self.width_mask |= 1 << width_index(w);
        }
        if !main.is_empty() {
            self.chains.push(S1gChain::new(
                rate,
                4e6,
                main.clone(),
                self.offset,
                0.0,
                false,
            ));
        }
        if allowed(8) {
            self.width_mask |= 1 << width_index(8);
            self.chains
                .push(S1gChain::new(rate, 8e6, vec![8], self.offset, 0.0, false));
        }
        if allowed(16) {
            self.width_mask |= 1 << width_index(16);
            self.chains
                .push(S1gChain::new(rate, 16e6, vec![16], self.offset, 0.0, false));
        }
        if self.offset == 0.0 && self.scan {
            let lim = 0.42 * rate / 1e6;
            if main.contains(&1) && rate >= 2.9e6 {
                let mut k = 0;
                let mut o: f64 = 0.5;
                while o + 0.6 <= lim + 1e-9 && k < 12 {
                    for s in [o, -o] {
                        if k < 12 {
                            self.scans.push((
                                S1gChain::new(rate, 4e6, vec![1], 0.0, s * 1e6, true),
                                None,
                            ));
                            k += 1;
                        }
                    }
                    o += 1.0;
                }
            }
            if main.contains(&2) && rate >= 4e6 {
                let mut k = 0;
                let mut o: f64 = 0.5;
                while o + 1.1 <= lim + 1e-9 && k < 16 {
                    for s in [o, -o] {
                        if k < 16 {
                            self.scans.push((
                                S1gChain::new(rate, 4e6, vec![2], 0.0, s * 1e6, true),
                                None,
                            ));
                            k += 1;
                        }
                    }
                    o += 0.5;
                }
            }
        }
        for (i, c) in self.chains.iter_mut().enumerate() {
            c.time_base = self.time;
            c.primary = i == 0;
        }
        self.wide = (!self.scans.is_empty()).then(|| WideDetector::new(rate));
    }

    /// Runs the scan chains lazily: a chain wakes at a wide detection, is fed from 150 µs before it, and sleeps again once it has
    /// nothing waiting.
    fn run_scans(&mut self, x: &[Complex32]) {
        let Some(wd) = self.wide.as_mut() else {
            return;
        };
        let start = self.n_in;
        self.raw.extend_from_slice(x);
        self.det.clear();
        wd.process(x, &mut self.det);
        let rate = self.rate;
        let back = (150e-6 * rate).round() as u64;
        let span = (800e-6 * rate).round() as u64;
        let end = start + x.len() as u64;
        // Chains awake before this piece take it whole.
        for (c, act) in self.scans.iter_mut() {
            if act.is_some() {
                c.process(x, &mut self.out);
            }
        }
        for &d in &self.det {
            for (c, act) in self.scans.iter_mut() {
                match act {
                    Some(u) => *u = (*u).max(d + span),
                    None => {
                        let from = d.saturating_sub(back).max(self.raw_base);
                        c.reset(self.time + from as f64 / rate);
                        let a = (from - self.raw_base) as usize;
                        let b = (end - self.raw_base) as usize;
                        c.process(&self.raw[a..b], &mut self.out);
                        *act = Some(d + span);
                    }
                }
            }
        }
        for (c, act) in self.scans.iter_mut() {
            if let Some(u) = *act {
                if end > u && c.idle() {
                    *act = None;
                }
            }
        }
        // Keep what a wake-up needs.
        let keep = end.saturating_sub(back + (64e-6 * rate) as u64);
        if keep > self.raw_base + (1 << 18) {
            let d = (keep - self.raw_base) as usize;
            self.raw.drain(..d);
            self.raw_base = keep;
        }
    }

    fn stats(&self) -> BankStats {
        let mut s = self.kept;
        for c in &self.chains {
            s += c.stats;
        }
        for (c, _) in &self.scans {
            s += c.stats;
        }
        BankStats {
            chains: s,
            width_mask: self.width_mask,
            n_chains: (self.chains.len() + self.scans.len()) as u32,
            ..Default::default()
        }
    }
}

impl Decoder for S1gBank {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "wlan.halow_rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<S1gPpdu>("ppdus"),
            Port::typed::<BankStats>("stats"),
        ]
    }
    fn settings(&self) -> std::sync::Arc<sdrkit_core::Schema> {
        BankSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BankSetting::Receive => {
                self.receive = RECEIVE.read(id, v)?;
                self.mask = receive_mask(self.receive);
            }
            BankSetting::SubChannelScan => self.scan = SUB_CHANNEL_SCAN.read(id, v)?,
        }
        if self.rate > 0.0 {
            self.rebuild();
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (BankSetting::Receive, Value::Choice(self.receive.index())),
            (BankSetting::SubChannelScan, Value::Bool(self.scan)),
        ])
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = f.rate;
        self.offset = f.signal_offset_hz();
        self.rebuild();
        Ok(vec![
            Format::new(Kind::typed::<S1gPpdu>(), 0.0),
            Format::new(Kind::typed::<BankStats>(), 0.0),
        ])
    }
    fn retune(&mut self, f: &Format) {
        self.offset = f.signal_offset_hz();
        self.rebuild();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.out.clear();
        for c in &mut self.chains {
            c.process(x, &mut self.out);
        }
        self.run_scans(x);
        self.n_in += x.len() as u64;
        let p: f64 = x.iter().map(|s| s.norm_sqr() as f64).sum();
        self.out.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
        let o = out.items::<S1gPpdu>(0);
        for p in self.out.drain(..) {
            o.push(Timed {
                time: p.time_s,
                value: p,
            });
        }
        let mut s = self.stats();
        s.time_s = self.time + self.n_in as f64 / self.rate.max(1.0);
        s.rate_hz = self.rate;
        s.level_dbfs =
            (!x.is_empty()).then(|| sdrkit_algo_dsp::meter::power_to_db(p / x.len() as f64));
        out.items::<BankStats>(1).push(Timed {
            time: s.time_s,
            value: s,
        });
    }
}

/// The `halow` graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("rx", DecoderComponent::new(S1gBank::default()))
        .add("table", table::HalowTable::default())
        .input("rx")
        .connect("rx.ppdus", "table.ppdus")
        .connect("rx.stats", "table.stats")
        .output("table.requests")
        .clock(0.25)
        .status_from("table")
        .catalog(catalog)
        .build()
}

sdrkit_core::settings! {
    /// The settings of the `halow` stack: the network table's and the receiver's.
    pub enum HalowSetting {
        /// The channel starting frequency.
        StartHz = table::START_HZ,
        /// The 1 MHz channels.
        Channels1 = table::CHANNELS_1,
        /// The 2 MHz channels.
        Channels2 = table::CHANNELS_2,
        /// The 4 MHz channels.
        Channels4 = table::CHANNELS_4,
        /// The 8 MHz channels.
        Channels8 = table::CHANNELS_8,
        /// The 16 MHz channels.
        Channels16 = table::CHANNELS_16,
        /// The width of the channel list.
        ListWidth = table::LIST_WIDTH,
        /// The channel.
        Channel = table::CHANNEL_SETTING,
        /// Scan the channels.
        Scan = table::SCAN,
        /// The dwell while scanning.
        Dwell = table::DWELL,
        /// The network filter text.
        FilterText = table::FILTER_TEXT,
        /// The least level shown.
        MinLevel = table::MIN_LEVEL,
        /// The widths read.
        Receive = RECEIVE,
        /// The sub-channel scan.
        SubChannelScan = SUB_CHANNEL_SCAN,
    }
}

/// The `halow` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let b = StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?).min_rate(
        MIN_RATE_HZ,
        "Wi-Fi HaLow needs a sample rate of at least 2 Msps",
    );
    route_to(b, "table", &table::HalowTableSetting::schema())?
        .route("receive", &["rx.receive"])?
        .route("sub_channel_scan", &["rx.sub_channel_scan"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("table", "remove"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the HaLow components of this crate to a registry.
pub fn register(r: &mut Registry) {
    r.add("wlan.halow_rx", || {
        Box::new(DecoderComponent::new(S1gBank::default()))
    });
    r.add("wlan.halow_networks", || {
        Box::new(table::HalowTable::default())
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::HalowGenerator::new(gen::HalowGenParams {
        rate_hz: cfg.rate_hz,
        ppdus: gen::test_schedule(cfg.rate_hz),
        seed: cfg.seed,
        ..Default::default()
    })))
}

/// The `halow` stack.
pub static HALOW: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Wi-Fi HaLow (802.11ah)",
    group: "Data and IoT",
    blurb: "Sub-GHz Wi-Fi HaLow networks from their S1G beacons: names, security, channels, widths and MCS.",
    tuning: Tuning {
        min_hz: 755e6,
        max_hz: 928e6,
        default_hz: 0.0,
        rate_hz: 6e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 4e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(4e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: HalowSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::WifiHalow],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::HALOW),
    benchmarks: None,
};
