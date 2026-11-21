//! New Packet Radio (`npr70`): a detector of the 4GFSK IP modems on 70 cm. At each symbol rate that the capture holds, the
//! discriminator output is searched for the alternating preamble and the sync word, whose third byte names the network. The
//! frame after the sync word is whitened by the modem chip in a way that is not published: nothing after it is read.

use crate::family::{min_rate_message, set_status, REPORT_S};
use crate::typed::{
    NprFields, NprFrameEvent, NprFrameRow, NprNetworkRow, NPR_FRAME, NPR_FRAMES, NPR_NETWORKS,
};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::levelsync::LevelPatterns;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, EventDef, GenConfig, Generator, LockProfile, LockState,
    Receiver, Record, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kept,
    Kind, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "npr70";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The symbol rates, symbols per second.
pub const RATES: [f64; 5] = [50e3, 100e3, 180e3, 300e3, 500e3];
/// The first two bytes of the sync word.
pub const SYNC_START: [u8; 2] = [0x45, 0xD2];
/// The third byte of the sync word for networks 0 to 15.
pub const NETWORK_HASH: [u8; 16] = [
    0x33, 0x36, 0x39, 0x3C, 0x63, 0x66, 0x69, 0x6C, 0x93, 0x96, 0x99, 0x9C, 0xC3, 0xC6, 0xC9, 0xCC,
];
/// The least sync correlation of a hit.
pub const MIN_CORRELATION: f32 = 0.93;
/// The default frequency of the test signal, Hz.
pub const TEST_DIAL_HZ: f64 = 437e6;
/// The most frames kept.
pub const FRAME_CAP: usize = 300;
/// A sync word within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 10.0;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the five symbol rates, the 4FSK level mapping, the preamble, the sync word 45 D2 and the network bytes. The whitening of the length byte and the payload is not known, so nothing after the sync word is read.";

/// The level of a bit pair (first bit the more significant): 00 → −3, 01 → −1, 11 → +1, 10 → +3.
pub fn level(pair: u8) -> f32 {
    match pair & 3 {
        0 => -3.0,
        1 => -1.0,
        3 => 1.0,
        _ => 3.0,
    }
}

/// The levels of bytes, two bits a symbol, most significant first.
pub fn levels(bytes: &[u8]) -> Vec<f32> {
    bytes
        .iter()
        .flat_map(|&b| (0..4).rev().map(move |k| level(b >> (2 * k))))
        .collect()
}

/// The 12 sync levels of network `n`.
pub fn sync_levels(n: usize) -> Vec<f32> {
    levels(&[SYNC_START[0], SYNC_START[1], NETWORK_HASH[n % 16]])
}

/// The symbol rates searched: those of `wanted` (all when empty) with 4 samples per symbol inside `rate`.
pub fn rates_searched(rate: f64, wanted: &[f64]) -> Vec<f64> {
    RATES
        .iter()
        .copied()
        .filter(|r| rate >= 4.0 * r && (wanted.is_empty() || wanted.contains(r)))
        .collect()
}

/// The rates of the setting text: "all", or symbol rates in ksym/s separated by commas. `None` for text that names no rate.
pub fn parse_rates(text: &str) -> Option<Vec<f64>> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("all") {
        return Some(Vec::new());
    }
    let mut v = Vec::new();
    for part in t.split(',') {
        let k: f64 = part.trim().parse().ok()?;
        let r = RATES.iter().copied().find(|r| (r / 1e3 - k).abs() < 1e-6)?;
        v.push(r);
    }
    Some(v)
}

/// A sync word found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NprFrame {
    /// Signal time of the sync word, s.
    pub t: f64,
    /// Wall time, when live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The network, 0 to 15.
    pub network: u8,
    /// The symbol rate, symbols per second.
    pub rate: f64,
    /// The tuning offset, Hz (absolute when the radio centre is known).
    pub offset_hz: f64,
    /// The sync correlation.
    pub correlation: f32,
}

impl Shift for NprFrame {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// A network heard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NprNetwork {
    /// The network.
    pub network: u8,
    /// Frames heard.
    pub frames: u64,
    /// The last symbol rate.
    pub rate: f64,
}

impl Shift for NprNetwork {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// What the stack keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct NprHeld {
    /// The frames.
    pub frames: LogRing<NprFrame>,
    /// The networks.
    pub networks: ObjectTable<u8, NprNetwork>,
    /// Sync words found.
    pub syncs: u64,
}

impl Default for NprHeld {
    fn default() -> Self {
        NprHeld {
            frames: LogRing::new(FRAME_CAP),
            networks: ObjectTable::new(16),
            syncs: 0,
        }
    }
}

impl Shift for NprHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.networks.shift_times(by_s);
    }
}

impl Tables for NprHeld {
    const FORMAT: StateFormat = StateFormat::new("ism.npr70", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.networks.clear();
        self.syncs = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.networks.adopt(saved.networks);
        self.syncs = saved.syncs;
    }
}

fn sync_text(n: u8) -> String {
    format!(
        "sync word {:02X} {:02X} {:02X}",
        SYNC_START[0],
        SYNC_START[1],
        NETWORK_HASH[usize::from(n) % 16]
    )
}

impl Shaped for NprFrame {
    type Row = NprFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NprFrameRow {
            utc: self.wall.map(Time),
            time_s: self.t,
            network: &format!("network {}", self.network),
            symbol_rate_hz: Hertz(self.rate),
            sync: &sync_text(self.network),
            tuning_hz: self.offset_hz,
            payload: "payload whitened",
        });
    }
}

impl Shaped for NprNetwork {
    type Row = NprNetworkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NprNetworkRow {
            network: &format!("network {}", self.network),
            frames: self.frames as i64,
            symbol_rate_hz: Hertz(self.rate),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

impl Item for NprFrame {
    const KIND: &'static str = "npr70.frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(NPR_FRAME.def())
    }
    fn write_fields(&self, f: &mut Record) {
        NprFrameEvent {
            network: &format!("network {}", self.network),
            symbol_rate_hz: Hertz(self.rate),
            family: Family::Other,
        }
        .write(f);
    }
}

/// The symbol rates to search, ksym/s, separated by commas, or "all".
pub const RATES_SETTING: Setting<field::Text> =
    Setting::new("rates", "Symbol rates searched", "ksym/s", "all").help(
        "\"all\" searches every rate that fits the sample rate; else a list such as \"100,500\" (50, 100, 180, 300, 500).",
    );

sdrkit_core::settings! {
    /// The settings of `ism.npr70` and of the `npr70` stack.
    pub enum NprSetting {
        /// The rates searched.
        Rates = RATES_SETTING,
    }
}

/// The search at one symbol rate.
struct RateSearch {
    rate: f64,
    resampler: Resampler<Complex32>,
    work: Vec<Complex32>,
    prev: Complex32,
    /// Discriminator samples at 4 per symbol; `d[0]` has index `start`.
    d: Vec<f32>,
    start: u64,
    /// The next sync start to test.
    next: u64,
    last_hit: Option<u64>,
    pending: Option<(u64, f32, usize, f32)>,
}

/// Samples of the preamble span and the sync word at 4 per symbol.
const SPAN: u64 = 4 * 28;

impl RateSearch {
    fn new(in_rate: f64, rate: f64) -> Result<RateSearch, ConfigError> {
        Ok(RateSearch {
            rate,
            resampler: Resampler::new(in_rate, 4.0 * rate).map_err(ConfigError::from)?,
            work: Vec::new(),
            prev: Complex32::new(1.0, 0.0),
            d: Vec::new(),
            start: 0,
            next: 4 * 16,
            last_hit: None,
            pending: None,
        })
    }

    /// The symbol value at sample `i`: samples 1 and 2 of the symbol added.
    fn value(&self, i: u64) -> f32 {
        let k = (i - self.start) as usize;
        self.d[k + 1] + self.d[k + 2]
    }

    /// Feeds samples and reports each hit (sample index of the sync word at 4R, network, correlation, preamble mean).
    fn run(
        &mut self,
        x: &[Complex32],
        sync: &LevelPatterns,
        mut hit: impl FnMut(u64, usize, f32, f32),
    ) {
        self.work.clear();
        self.resampler.process(x, &mut self.work);
        for &z in &self.work {
            let p = z * self.prev.conj();
            self.d.push(p.im.atan2(p.re));
            self.prev = z;
        }
        let end = self.start + self.d.len() as u64;
        let mut pre = [0f32; 12];
        let mut val = [0f32; 12];
        while self.next + 4 * 12 + 3 <= end {
            let p = self.next;
            self.next += 1;
            if let Some((q, c, n, m)) = self.pending {
                if p > q + 8 {
                    self.pending = None;
                    self.last_hit = Some(q);
                    hit(q, n, c, m);
                }
            }
            if self.last_hit.is_some_and(|h| p < h + 48) {
                continue;
            }
            for (k, v) in pre.iter_mut().enumerate() {
                *v = self.value(p - 48 + 4 * k as u64);
            }
            let mean = pre.iter().sum::<f32>() / 12.0;
            let alternate = pre.windows(2).all(|w| (w[0] - mean) * (w[1] - mean) < 0.0);
            let outer = pre
                .iter()
                .filter(|v| (0.55..=1.45).contains(&((*v - mean).abs() / FRAC_PI_2)))
                .count();
            if !alternate || outer < 11 {
                continue;
            }
            for (k, v) in val.iter_mut().enumerate() {
                *v = self.value(p + 4 * k as u64);
            }
            let Some((n, c)) = sync.best(&val) else {
                continue;
            };
            if c < MIN_CORRELATION {
                continue;
            }
            if self.pending.is_none_or(|(_, pc, _, _)| c > pc) {
                self.pending = Some((p, c, n, mean));
            }
        }
        // Keep what the next tests need.
        let keep_from = self.next.saturating_sub(SPAN + 64).max(self.start);
        let cut = (keep_from - self.start) as usize;
        if cut > 4096 {
            self.d.drain(..cut);
            self.start += cut as u64;
        }
    }
}

/// The `ism.npr70` decoder.
pub struct NprDecoder {
    wanted: Vec<f64>,
    rate: f64,
    /// Signal time of the first sample after the last (re)configure or change of rates, s.
    origin_s: f64,
    fresh: bool,
    centre_hz: f64,
    tuned_hz: f64,
    mixer: Option<Nco>,
    mixed: Vec<Complex32>,
    searches: Vec<RateSearch>,
    sync: LevelPatterns,
    held: NprHeld,
    out: Vec<Timed<NprFrame>>,
    level_acc: f64,
    level_n: u64,
    level_dbfs: Option<f64>,
    now: f64,
    live: bool,
    clock: Option<sdrkit_core::Clock>,
    dirty: bool,
    cleared: bool,
    started: bool,
}

impl Default for NprDecoder {
    fn default() -> Self {
        NprDecoder {
            wanted: Vec::new(),
            rate: 0.0,
            origin_s: 0.0,
            fresh: true,
            centre_hz: 0.0,
            tuned_hz: 0.0,
            mixer: None,
            mixed: Vec::new(),
            searches: Vec::new(),
            sync: LevelPatterns::new(&(0..16).map(sync_levels).collect::<Vec<_>>()),
            held: NprHeld::default(),
            out: Vec::new(),
            level_acc: 0.0,
            level_n: 0,
            level_dbfs: None,
            now: 0.0,
            live: false,
            clock: None,
            dirty: true,
            cleared: false,
            started: false,
        }
    }
}

impl NprDecoder {
    /// The frames, the networks and the counters.
    pub fn held(&self) -> &NprHeld {
        &self.held
    }

    fn build(&mut self) -> Result<(), ConfigError> {
        self.fresh = true;
        self.searches = rates_searched(self.rate, &self.wanted)
            .into_iter()
            .map(|r| RateSearch::new(self.rate, r))
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    fn hit(&mut self, rate: f64, at: u64, network: usize, correlation: f32, mean: f32) {
        let t = self.origin_s + at as f64 / (4.0 * rate);
        let off = f64::from(mean) / 2.0 / std::f64::consts::TAU * 4.0 * rate;
        let base = if self.centre_hz > 0.0 {
            self.tuned_hz
        } else {
            0.0
        };
        let stamp = match self.clock.as_ref().and_then(|c| self.live.then(|| c.now())) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        };
        let f = NprFrame {
            t,
            wall: stamp.wall,
            network: network as u8,
            rate,
            offset_hz: base + off,
            correlation,
        };
        self.held.syncs += 1;
        if let Ok(up) = self.held.networks.upsert(&f.network, stamp, || NprNetwork {
            network: f.network,
            frames: 0,
            rate,
        }) {
            up.row.frames += 1;
            up.row.rate = rate;
        }
        self.held.frames.push(f.clone());
        self.out.push(Timed { time: t, value: f });
        self.dirty = true;
    }
}

impl Decoder for NprDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "ism.npr70"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<NprFrame>("frames")]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        NprSetting::schema()
    }
    fn values(&self) -> Values {
        let t = if self.wanted.is_empty() {
            "all".to_string()
        } else {
            self.wanted
                .iter()
                .map(|r| format!("{}", r / 1e3))
                .collect::<Vec<_>>()
                .join(",")
        };
        values_of([(NprSetting::Rates, Value::Text(t))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match NprSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NprSetting::Rates => {
                let t = RATES_SETTING.read(id, v)?;
                self.wanted = parse_rates(t).ok_or(SettingError::Malformed { id, at: None })?;
            }
        }
        if self.rate > 0.0 {
            self.build()
                .map_err(|_| SettingError::NotPermitted { id })?;
        }
        Ok(())
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre_hz = input.centre_hz;
        self.tuned_hz = input.tuned_hz;
        let off = input.signal_offset_hz();
        self.mixer = (off.abs() > 0.5).then(|| Nco::new(input.rate, -off));
        self.build()?;
        Ok(vec![Format {
            rate: 0.0,
            frame_len: 0,
            media: None,
            ..input.with_kind(Kind::typed::<NprFrame>())
        }])
    }
    fn retune(&mut self, input: &Format) {
        let _ = self.configure(input);
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = Some(ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        if std::mem::take(&mut self.fresh) {
            self.origin_s = first as f64 / self.rate.max(1.0);
        }
        for z in x {
            self.level_acc += f64::from(z.norm_sqr());
        }
        self.level_n += x.len() as u64;
        let input: &[Complex32] = match self.mixer.as_mut() {
            Some(m) => {
                self.mixed.clear();
                self.mixed.resize(x.len(), Complex32::default());
                m.mix(x, &mut self.mixed);
                &self.mixed
            }
            None => x,
        };
        let mut hits = Vec::new();
        for s in &mut self.searches {
            let rate = s.rate;
            s.run(input, &self.sync, |at, n, c, m| {
                hits.push((rate, at, n, c, m))
            });
        }
        for (rate, at, n, c, m) in hits {
            self.hit(rate, at, n, c, m);
        }
        out.items::<NprFrame>(0).append(&mut self.out);
    }
    fn report(&mut self, p: &mut Published) {
        if self.level_n > 0 {
            self.level_dbfs =
                Some(10.0 * (self.level_acc / self.level_n as f64).max(1e-20).log10());
            self.level_acc = 0.0;
            self.level_n = 0;
        }
        let searched: Vec<String> = self
            .searches
            .iter()
            .map(|s| format!("{}", s.rate / 1e3))
            .collect();
        p.put(&NprFields {
            sync_words: self.held.syncs as i64,
            rates_searched: &searched.join(", "),
            level_dbfs: self.level_dbfs,
            note: NOT_CONFIRMED,
        });
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&NPR_FRAMES);
            p.clear_rows(&NPR_NETWORKS);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.frames, &NPR_FRAMES, self.now, p);
            shaped::rows(&self.held.networks, &NPR_NETWORKS, self.now, p);
        }
        let newest = self.held.frames.newest_first().next();
        let recent = newest.is_some_and(|f| self.now - f.t <= RECENT_S);
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let summary = match newest {
            Some(f) if recent => format!(
                "NPR detector: {} sync words; last network {} at {} ksym/s (frame content not read)",
                self.held.syncs,
                f.network,
                f.rate / 1e3
            ),
            _ if self.started => format!(
                "NPR detector: searching {} ksym/s",
                if searched.is_empty() {
                    "no rate".to_string()
                } else {
                    searched.join(", ")
                }
            ),
            _ => "NPR detector: waiting for samples".to_string(),
        };
        set_status(p, lock, &summary);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn forget(&mut self, kept: &Kept) {
        self.level_dbfs = None;
        self.level_acc = 0.0;
        self.level_n = 0;
        if !kept.table(NPR_FRAMES.key) {
            self.held.frames.clear();
            self.cleared = true;
        }
        if !kept.table(NPR_NETWORKS.key) {
            self.held.networks.clear();
            self.cleared = true;
        }
        if !kept.field("sync_words") {
            self.held.syncs = 0;
        }
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The `npr70` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(NprDecoder::default()))
        .input("rx")
        .output("rx")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            &min_rate_message("New Packet Radio", MIN_RATE_HZ),
        )
        .route(RATES_SETTING.key(), &["rx.rates"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The samples of one test frame at `rate`: 40 preamble symbols (+3, −3), the sync word of network `network`, then 120
/// random bytes, 4GFSK with BT 0.5 at symbol rate `symbol_rate` (the outer levels at ± half the symbol rate).
///
/// # Errors
///
/// [`ConfigError`] if the modulator refuses the rate.
pub fn frame(
    rate: f64,
    symbol_rate: f64,
    network: usize,
    rng: &mut Rng,
) -> Result<Vec<Complex32>, ConfigError> {
    let mut lv: Vec<f32> = (0..40)
        .map(|i| if i % 2 == 0 { 3.0 } else { -3.0 })
        .collect();
    lv.extend(sync_levels(network));
    let body: Vec<u8> = (0..120).map(|_| rng.next_u64() as u8).collect();
    lv.extend(levels(&body));
    let mut m = Cpm::gfsk(0.5, 1.0 / 3.0, rate / symbol_rate)
        .map_err(ConfigError::from)?
        .with_held_edges();
    let mut out = Vec::new();
    m.push(&lv, &mut out);
    m.flush(&mut out);
    Ok(out)
}

/// The test signal at `rate`: frames 20 ms apart, in turn at each symbol rate that fits, networks 0, 5 and 15 and tuning
/// offsets of 0 and ±2 % of the rate.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(rate: f64, seed: u64) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(rate)?;
    let rates: Vec<f64> = RATES.iter().copied().filter(|r| rate >= 8.0 * r).collect();
    let mut rng = Rng::stream(seed, 0x4E50);
    let mut k = 0usize;
    let mut t = 0.005;
    scene.add_source(Box::new(move || {
        let r = *rates.get(k % rates.len().max(1))?;
        let n = [0usize, 5, 15][k / rates.len() % 3];
        let off = [0.0, 0.02, -0.02][k % 3] * r;
        let samples = frame(rate, r, n, &mut rng).ok()?;
        let b = Burst {
            start_s: t,
            offset_hz: off,
            level_db: 0.0,
            samples,
        };
        k += 1;
        t += 0.02;
        Some(b)
    }));
    Ok(scene)
}

struct NprGenerator {
    scene: Scene,
    rate: f64,
}

impl Generator for NprGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(500e3)
                .with_symbol_rate(100e3)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(NprGenerator {
        scene: scene(cfg.rate_hz, cfg.seed)?,
        rate: cfg.rate_hz,
    }))
}

/// New Packet Radio. The default frequency is that of the test signal; the session gives the real one.
pub static NPR70: StackDescriptor = StackDescriptor {
    id: ID,
    name: "New Packet Radio",
    group: "Amateur digital",
    blurb: "A detector of New Packet Radio 4GFSK IP modems on 70 cm: sync word, network id, symbol rate and tuning offset of each frame; the frame content is not read.",
    tuning: Tuning::anywhere(TEST_DIAL_HZ, 4e6, MIN_RATE_HZ, 2e6),
    submodes: &[],
    settings: NprSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &sdrkit_core::verify::VerifyOptions) -> Vec<sdrkit_core::verify::VerifyCase> {
    let mut c = sdrkit_core::verify::VerifyCase::new("test signal, five rates", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = NprFields::read(r.fields()).map_or(0, |f| f.sync_words);
        vec![crate::family::at_least_value("sync_words", n, 20)]
    });
    vec![c]
}

/// The verify case of `npr70`.
pub static VERIFY: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the receiver: 0.5 s of the test signal at 4 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "npr70.receiver_4m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 4e6;
    let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
    if let Ok(mut s) = scene(rate, 1) {
        s.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&NprSetting::schema()))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ =
        rx.configure(&sdrkit_core::InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.5, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}
