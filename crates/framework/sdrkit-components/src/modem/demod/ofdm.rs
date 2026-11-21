//! `modem.ofdm_sync` and `modem.ofdm_demod`: OFDM symbol timing and carrier offset, then the cells of every symbol, equalised,
//! with their channel state.

use sdrkit_algo_modem::ofdm::{
    CpSync, CpSyncConfig, CpSyncEvent, Estimator, OfdmLayout, OfdmReceiver,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::SlicePool;
use sdrkit_graph::{
    Applied, Component, ComponentInfo, Every, Format, Io, Item, Kind, Port, Tag, TagKind, Timed,
};
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// A named layout of `sdrkit_algo_modem::ofdm::OfdmLayout`.
    pub enum OfdmLayoutName {
        /// IEEE 802.11a/g.
        Ieee80211a = ("ieee80211a", "IEEE 802.11a/g"),
        /// DVB-T 2K.
        Dvbt2k = ("dvbt_2k", "DVB-T 2K"),
    }
}

sdrkit_core::choice_enum! {
    /// A channel estimator of `sdrkit_algo_modem::ofdm::Estimator`.
    pub enum ChannelEstimator {
        /// Pilots, least squares.
        PilotLs = ("pilot_ls", "Pilots, least squares"),
        /// Pilots, least squares, smoothed.
        PilotLsSmooth = ("pilot_ls_smooth", "Pilots, least squares, smoothed"),
        /// Scattered pilots in time and frequency.
        TimeFreq = ("time_freq", "Scattered pilots in time and frequency"),
    }
}

impl OfdmLayoutName {
    /// The layout with `guard` (the FFT size over the cyclic prefix).
    fn layout(self, guard: u32) -> Result<OfdmLayout, ConfigError> {
        Ok(OfdmLayout::by_name(
            &Self::CHOICES[usize::from(self.index())].key,
            guard as usize,
        )?)
    }
}

impl ChannelEstimator {
    /// The estimator for `layout`.
    fn estimator(self, layout: &OfdmLayout) -> Result<Estimator, ConfigError> {
        Ok(Estimator::by_name(
            &Self::CHOICES[usize::from(self.index())].key,
            layout,
        )?)
    }
}

const LAYOUT: Setting<field::Choice<OfdmLayoutName>> =
    Setting::choice("layout", "Layout", OfdmLayoutName::Ieee80211a);
const GUARD: Setting<field::I64> = Setting::new("guard", "Guard interval", "", 4)
    .range(4.0, 32.0, 1.0)
    .help("DVB-T: the cyclic prefix is the FFT size over this (4, 8, 16 or 32).");
const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "Detection threshold", "", 0.7)
    .range(0.1, 1.0, 0.0)
    .help("Normalised prefix correlation (0 to 1) above which a symbol is found.");
const MAX_SHIFT: Setting<field::I64> = Setting::new("max_shift", "Carrier offset range", "", 2)
    .range(0.0, 64.0, 1.0)
    .help("The largest whole-carrier offset searched, in carriers.");
const ESTIMATOR: Setting<field::Choice<ChannelEstimator>> =
    Setting::choice("estimator", "Channel estimator", ChannelEstimator::PilotLs);
const BACKOFF: Setting<field::I64> = Setting::new("backoff", "Window backoff", "samples", 2)
    .range(0.0, 1024.0, 1.0)
    .help("The FFT window starts this many samples before the end of the prefix (at most the prefix).");
const FIRST_SYMBOL: Setting<field::I64> = Setting::new("first_symbol", "First symbol", "", 0)
    .range(0.0, 1e9, 1.0)
    .help("The number, in the pilot pattern's cycle, of the symbol at a frame start.");

sdrkit_core::settings! {
    /// The settings of [`OfdmSync`].
    pub enum OfdmSyncSetting {
        /// The layout.
        Layout = LAYOUT,
        /// The guard interval.
        Guard = GUARD,
        /// The detection threshold.
        Threshold = THRESHOLD,
        /// The carrier offset range.
        MaxShift = MAX_SHIFT,
    }
}

sdrkit_core::settings! {
    /// The settings of [`OfdmDemod`].
    pub enum OfdmDemodSetting {
        /// The layout.
        Layout = LAYOUT,
        /// The guard interval.
        Guard = GUARD,
        /// The channel estimator.
        Estimator = ESTIMATOR,
        /// The window backoff.
        Backoff = BACKOFF,
        /// The first symbol.
        FirstSymbol = FIRST_SYMBOL,
    }
}

/// The guard interval `v` of setting `id`: 4, 8, 16 or 32.
fn read_guard(id: SettingId, v: &Value) -> Result<u32, SettingError> {
    let g = GUARD.read(id, v)?;
    if matches!(g, 4 | 8 | 16 | 32) {
        Ok(g as u32)
    } else {
        Err(SettingError::NotPermitted { id })
    }
}

/// `modem.ofdm_sync`: finds OFDM symbols in `Iq` and passes them on whole, back to back, with the carrier offset removed.
///
/// A thin wrapper over `sdrkit_algo_modem::ofdm::CpSync`: the cyclic prefix is found by `ofdm::LagCorrelator` (lag = FFT size, window
/// = prefix; a symbol is found when the normalised metric rises above `threshold`), which also measures the fractional carrier
/// offset. At the first symbol found, the whole-carrier offset is measured over the FFT of that symbol (`ofdm::integer_cfo`, power
/// over the used carriers, within `max_shift` carriers) and the stream locks: from then on symbols are cut on a fixed grid, kept by
/// the following prefix peaks (moved when one lands more than a quarter of the prefix away), and the fractional offset is smoothed
/// over them. Output symbols start with their prefix; the offset is removed with a phase continuous over the stream.
///
/// Tags: `FrameStart` (inverted false, errors 0) on the first sample of the first symbol after locking, `Lock { locked }` where
/// lock is gained and lost (after 8 symbols with no prefix peak). A symbol leaves two prefix lengths after its last sample. Input
/// tags move to the first output sample at or after them; a `Retuned` or `Discontinuity` tag drops the lock. Publishes `offset_hz`
/// and `locked`.
#[derive(Debug)]
pub struct OfdmSync {
    layout_name: OfdmLayoutName,
    guard: u32,
    threshold: f64,
    max_shift: u32,
    sync: Option<Box<CpSync>>,
    /// The longest symbol of the layout: the input is given to the synchroniser at most 16 of them at a time, so the events of
    /// one step fit in `events`.
    step: usize,
    events: Vec<CpSyncEvent>,
    /// The absolute index of the synchroniser's position 0.
    origin: u64,
    pending: Vec<Tag>,
    started: bool,
    rate: f64,
    results: Published,
    every: Every,
}

impl Default for OfdmSync {
    fn default() -> Self {
        OfdmSync {
            layout_name: OfdmLayoutName::Ieee80211a,
            guard: GUARD.default as _,
            threshold: THRESHOLD.default,
            max_shift: MAX_SHIFT.default as _,
            sync: None,
            step: 0,
            events: Vec::new(),
            origin: 0,
            pending: Vec::new(),
            started: false,
            rate: 0.0,
            results: Published::new(&[&keys::OFFSET_HZ, &keys::LOCKED]),
            every: Every::default(),
        }
    }
}

/// Most input tags held for their output sample.
const MAX_PENDING: usize = 64;
/// Room for the events of one step of at most 16 symbols.
const MAX_EVENTS: usize = 64;

impl OfdmSync {
    /// Publishes the lock and the offset (at every tick, and at once when the lock changes).
    fn publish(&mut self) {
        let Some(sync) = &self.sync else { return };
        let locked = sync.locked();
        self.results.set(&keys::LOCKED, locked);
        if locked {
            self.results.set(&keys::OFFSET_HZ, sync.freq() * self.rate);
        } else {
            self.results.none(&keys::OFFSET_HZ);
        }
    }

    /// Starts afresh at the next sample (a retune or a gap), tagging a lock that is dropped.
    fn reset(&mut self, io: &mut Io) {
        let Some(sync) = &mut self.sync else { return };
        if sync.reset() {
            let at = io.output(0).len();
            io.output(0).tag_here(at, TagKind::Lock { locked: false });
            self.publish();
        }
    }

    /// Passes `piece` to the synchroniser and turns what it did into output samples, tags and results.
    fn feed(&mut self, io: &mut Io, piece: &[Complex32]) {
        for step in piece.chunks(self.step.max(1)) {
            let Some(sync) = &mut self.sync else { return };
            self.events.clear();
            sync.process(step, io.iq_out(0), &mut self.events);
            for k in 0..self.events.len() {
                match self.events[k] {
                    CpSyncEvent::Lock { locked, out_at } => {
                        let buf = io.output(0);
                        buf.tag_here(out_at, TagKind::Lock { locked });
                        if locked {
                            buf.tag_here(
                                out_at,
                                TagKind::FrameStart {
                                    inverted: false,
                                    errors: 0,
                                },
                            );
                        }
                        self.publish();
                    }
                    CpSyncEvent::Symbol {
                        start, len, out_at, ..
                    } => {
                        // Input tags that came before the end of this symbol land on its first sample at or after them.
                        let (start, buf) = (self.origin + start, io.output(0));
                        let mut i = 0;
                        while i < self.pending.len() {
                            if self.pending[i].at < start + len as u64 {
                                let t = self.pending.remove(i);
                                let off = t.at.saturating_sub(start) as usize;
                                buf.tag_here(out_at + off, t.kind);
                            } else {
                                i += 1;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Component for OfdmSync {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.ofdm_sync",
            vec![Port::iq("in")],
            vec![Port::iq("out")],
        )
        .with_settings(OfdmSyncSetting::schema())
        .manual_tags()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let layout = self.layout_name.layout(self.guard)?;
        let cfg = CpSyncConfig {
            threshold: self.threshold as f32,
            max_shift: self.max_shift,
        };
        self.step = 16 * layout.longest_symbol();
        self.sync = Some(Box::new(
            CpSync::new(&layout, cfg).map_err(ConfigError::from)?,
        ));
        self.events = Vec::with_capacity(MAX_EVENTS);
        self.pending = Vec::with_capacity(MAX_PENDING);
        self.started = false;
        self.rate = inputs[0].rate;
        self.every = Every::new(inputs[0].rate, 0.25);
        Ok(vec![inputs[0].clone()])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let input = io.input(0);
        let first = input.first();
        let x = io.iq(0);
        if !self.started {
            self.started = true;
            self.origin = first - self.sync.as_ref().map_or(0, |s| s.position());
        }
        let mut at = 0usize;
        for t in input.tags() {
            let p = (t.at.saturating_sub(first) as usize).min(n);
            let reset = matches!(
                t.kind,
                TagKind::Retuned { .. } | TagKind::Discontinuity { .. }
            );
            if reset {
                if p > at {
                    self.feed(io, &x[at..p]);
                    at = p;
                }
                self.reset(io);
            }
            // Every input tag waits for its output sample.
            if self.pending.len() < MAX_PENDING {
                self.pending.push(t.clone());
            }
        }
        if n > at {
            self.feed(io, &x[at..n]);
        }
        if self.every.tick(n) {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match OfdmSyncSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OfdmSyncSetting::Layout => self.layout_name = LAYOUT.read(id, v)?,
            OfdmSyncSetting::Guard => self.guard = read_guard(id, v)?,
            OfdmSyncSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            OfdmSyncSetting::MaxShift => self.max_shift = MAX_SHIFT.read(id, v)? as u32,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (
                OfdmSyncSetting::Layout,
                Value::Choice(self.layout_name.index()),
            ),
            (OfdmSyncSetting::Guard, Value::I64(i64::from(self.guard))),
            (OfdmSyncSetting::Threshold, Value::F64(self.threshold)),
            (
                OfdmSyncSetting::MaxShift,
                Value::I64(i64::from(self.max_shift)),
            ),
        ])
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}

/// The cells of one OFDM symbol (the output of `modem.ofdm_demod`): every used carrier (`layout.first() ..= layout.last()`, nulls
/// and reserved carriers included) equalised by zero forcing, with its weight `|H|^2 / noise`, ready for
/// `Constellation::demap_block_weighted`. `OfdmLayout::data(symbol)` lists which of them carry data.
#[derive(Clone, Debug, PartialEq)]
pub struct OfdmCells {
    /// The symbol's number in the layout's cycle (from `first_symbol` at the last frame start).
    pub symbol: u64,
    /// Equalised cells, one per used carrier (shared, reused once every reader has dropped them).
    pub cells: Arc<[Complex32]>,
    /// The weight of each cell, `|H|^2 / noise` (0 for a cell in a deep fade: an erasure).
    pub weights: Arc<[f32]>,
    /// The noise variance per cell the estimator measured (`E|n|^2`, before equalisation).
    pub noise: f32,
}

impl Item for OfdmCells {
    const KIND: &'static str = "ofdm.cells";
}

/// `modem.ofdm_demod`: whole OFDM symbols (from `modem.ofdm_sync`) to [`OfdmCells`].
///
/// A thin wrapper over `sdrkit_algo_modem::ofdm::OfdmReceiver`: each symbol goes through `ofdm::FftFramer` (FFT window `backoff`
/// samples early, its phase ramp removed), the chosen channel estimator (`ofdm::Estimator::by_name`: `PilotLs`, raw or smoothed
/// over neighbouring pilots, with no delay; or `TimeFreq`, scattered pilots interpolated in time then frequency, `period - 1`
/// symbols late) and `ofdm::equalise` (zero forcing with channel state information). Each item's time is the start of its own
/// symbol, also when the estimator reports it late. A `FrameStart` tag starts the symbol count again at `first_symbol` (the phase of
/// the pilot pattern) and clears the estimator; `Retuned` and `Discontinuity` do the same. Publishes `snr_db`, the mean `|H|^2`
/// over the noise per cell.
#[derive(Debug)]
pub struct OfdmDemod {
    layout_name: OfdmLayoutName,
    guard: u32,
    estimator_name: ChannelEstimator,
    backoff: u32,
    first_symbol: u64,
    rx: Option<Box<OfdmReceiver<Estimator>>>,
    /// The absolute index of the receiver's position 0.
    origin: u64,
    started: bool,
    rate: f64,
    pool_c: SlicePool<Complex32>,
    pool_w: SlicePool<f32>,
    results: Published,
    every: Every,
}

impl Default for OfdmDemod {
    fn default() -> Self {
        OfdmDemod {
            layout_name: OfdmLayoutName::Ieee80211a,
            guard: GUARD.default as _,
            estimator_name: ChannelEstimator::PilotLs,
            backoff: BACKOFF.default as _,
            first_symbol: FIRST_SYMBOL.default as _,
            rx: None,
            origin: 0,
            started: false,
            rate: 0.0,
            pool_c: SlicePool::default(),
            pool_w: SlicePool::default(),
            results: Published::new(&[&keys::SNR_DB]),
            every: Every::default(),
        }
    }
}

impl OfdmDemod {
    /// Passes `x` to the receiver and sends out the symbols it equalises.
    fn feed(&mut self, io: &mut Io, x: &[Complex32]) {
        let Some(rx) = &mut self.rx else { return };
        let (pool_c, pool_w, origin, rate) =
            (&mut self.pool_c, &mut self.pool_w, self.origin, self.rate);
        let out = io.items_out::<OfdmCells>(0);
        rx.process(x, |s| {
            let item = OfdmCells {
                symbol: s.symbol,
                cells: pool_c.exact(s.cells.len(), |c| c.copy_from_slice(s.cells)),
                weights: pool_w.exact(s.weights.len(), |w| w.copy_from_slice(s.weights)),
                noise: s.noise,
            };
            out.push(Timed {
                time: (origin + s.start) as f64 / rate,
                value: item,
            });
        });
    }
}

impl Component for OfdmDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.ofdm_demod",
            vec![Port::iq("in")],
            vec![Port::typed::<OfdmCells>("out")],
        )
        .with_settings(OfdmDemodSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let layout = self.layout_name.layout(self.guard)?;
        let est = self.estimator_name.estimator(&layout)?;
        let rx = OfdmReceiver::new(&layout, est, self.backoff as usize, self.first_symbol)
            .map_err(ConfigError::from)?;
        let used = layout.used();
        self.pool_c = SlicePool::with_ready(used, 8);
        self.pool_w = SlicePool::with_ready(used, 8);
        self.rx = Some(Box::new(rx));
        self.started = false;
        self.rate = inputs[0].rate;
        self.every = Every::new(inputs[0].rate, 0.25);
        Ok(vec![Format::new(Kind::typed::<OfdmCells>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let input = io.input(0);
        let first = input.first();
        let x = io.iq(0);
        if !self.started {
            self.started = true;
            self.origin = first - self.rx.as_ref().map_or(0, |r| r.position());
        }
        // Restart at every frame start, retune or discontinuity, at its sample.
        let mut at = 0usize;
        for t in input.tags() {
            if !matches!(
                t.kind,
                TagKind::FrameStart { .. }
                    | TagKind::Retuned { .. }
                    | TagKind::Discontinuity { .. }
            ) {
                continue;
            }
            let p = (t.at.saturating_sub(first) as usize).min(n);
            if p > at {
                self.feed(io, &x[at..p]);
                at = p;
            }
            if let Some(rx) = &mut self.rx {
                rx.restart();
            }
        }
        if n > at {
            self.feed(io, &x[at..n]);
        }
        if let Some(rx) = &self.rx {
            let snr = rx.snr();
            if self.every.tick(n) && snr > 0.0 && snr.is_finite() {
                self.results.set(&keys::SNR_DB, 10.0 * (snr as f64).log10());
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match OfdmDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OfdmDemodSetting::Layout => self.layout_name = LAYOUT.read(id, v)?,
            OfdmDemodSetting::Guard => self.guard = read_guard(id, v)?,
            OfdmDemodSetting::Estimator => self.estimator_name = ESTIMATOR.read(id, v)?,
            OfdmDemodSetting::Backoff => self.backoff = BACKOFF.read(id, v)? as u32,
            OfdmDemodSetting::FirstSymbol => self.first_symbol = FIRST_SYMBOL.read(id, v)? as u64,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (
                OfdmDemodSetting::Layout,
                Value::Choice(self.layout_name.index()),
            ),
            (OfdmDemodSetting::Guard, Value::I64(i64::from(self.guard))),
            (
                OfdmDemodSetting::Estimator,
                Value::Choice(self.estimator_name.index()),
            ),
            (
                OfdmDemodSetting::Backoff,
                Value::I64(i64::from(self.backoff)),
            ),
            (
                OfdmDemodSetting::FirstSymbol,
                Value::I64(self.first_symbol as i64),
            ),
        ])
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}
