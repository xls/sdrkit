//! Inmarsat, all carriers (`inmarsat`): one wide capture of the L-band segment. Every carrier is found and classified, the Aero P
//! channels and the Inmarsat-C channels are decoded at once, and the satellite is identified.
//! Standard: Inmarsat Aeronautical and Inmarsat-C System Definition Manuals.

pub mod satellite;

use sdrkit_algo_dsp::meter::power_to_db as db;
use crate::display::set_series;
use crate::display::Clock;
use crate::modes::aero::{self, AeroItem, ChannelRow as AeroChannel};
use crate::modes::inmarsatc::{self, coding as icoding, egc, ChannelInfo, InmcItem};
use crate::typed::{
    InmarsatCandidateReasonRow, InmarsatCandidateRow, InmarsatCarrierArgs, InmarsatCarrierRow,
    InmarsatFields, INMARSAT_CANDIDATES, INMARSAT_CANDIDATE_REASONS, INMARSAT_CARRIERS,
    INMARSAT_CARRIER_FOCUS,
};
use satellite::{identify, Evidence, Identification};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, CatalogueSetting, ChoiceDecl, Command, Complex32, ConfigError, DeclareSetting,
    GenConfig, Generator, LockProfile, LockState, Receiver, Record, ResultsDecl, Schema, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor, Tuning, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "inmarsat";
/// The stack name.
pub const NAME: &str = "Inmarsat (all carriers)";
/// The receiver works from this rate. The stack requests more (see the descriptor).
pub const MIN_RATE: f64 = 600e3;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// The width of a window of the Aero receivers, Hz.
pub const WINDOW_HZ: f64 = 160e3;
/// The most Inmarsat-C receivers at one time.
pub const MAX_INMC: usize = 3;

/// A detected carrier.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Carrier {
    pub id: u64,
    pub offset_hz: f64,
    pub width_hz: f64,
    pub snr_db: f64,
    /// Its class by width.
    pub class: CarrierClass,
    pub first_seen: f64,
    pub last_seen: f64,
}

/// What the merged display hears.
#[derive(Clone, Debug)]
pub enum InmItem {
    Carriers(Vec<Carrier>, Vec<f32>),
    Aero(Box<AeroItem>),
    Inmc(Box<InmcItem>),
}
impl Item for InmItem {
    const KIND: &'static str = "inmarsat.item";
}

sdrkit_core::choice_enum! {
    /// The class of a carrier by its width.
    #[derive(Default)]
    pub enum CarrierClass {
        /// Under 1.5 kHz.
        #[default]
        Narrow = ("narrow", "Narrow"),
        /// 1.5 to 4.5 kHz.
        Medium = ("medium", "Medium"),
        /// 4.5 kHz and more.
        Wide = ("wide", "Wide"),
    }
}

sdrkit_core::choice_enum! {
    /// What decodes a carrier.
    #[derive(Default)]
    pub enum Decodes {
        /// Nothing yet.
        #[default]
        Unknown = ("unknown", "Unknown"),
        /// The Aero P receiver.
        AeroP = ("aero_p", "Aero P"),
        /// An Inmarsat-C receiver.
        InmarsatC = ("inmarsat_c", "Inmarsat-C"),
    }
}

/// The class of a carrier by its width.
pub fn class_of(width: f64) -> CarrierClass {
    if width < 1.5e3 {
        CarrierClass::Narrow
    } else if width < 4.5e3 {
        CarrierClass::Medium
    } else {
        CarrierClass::Wide
    }
}

/// The limits of a [`Detector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Seconds between two searches.
    pub eval_s: f64,
    /// The widest region kept, Hz.
    pub max_width_hz: f64,
    /// Bins under the threshold bridged inside a region.
    pub gap_bins: usize,
    /// A carrier not seen for this long is dropped, s.
    pub drop_s: f64,
    /// Bins this close to 0 Hz are not used, Hz.
    pub dc_hz: f64,
}

impl Default for Limits {
    /// One search a second, regions up to 40 kHz, gaps of 2 bins, dropped after 8 s, 2 kHz around 0 Hz unused.
    fn default() -> Self {
        Limits {
            eval_s: 1.0,
            max_width_hz: 40e3,
            gap_bins: 2,
            drop_s: 8.0,
            dc_hz: 2e3,
        }
    }
}

/// The carrier detector: 250 Hz bins, 32 spectra a second, regions 3 dB over the median, confirmed in two searches.
#[derive(Default)]
pub struct Detector {
    /// The limits of the search.
    pub limits: Limits,
    rate: f64,
    fft: Option<Fft>,
    n: usize,
    win: Vec<f32>,
    frame: Vec<Complex32>,
    buf: Vec<Complex32>,
    acc: Vec<f32>,
    skip: u64,
    next_eval: u64,
    prev: Vec<(f64, f64, f64)>,
    pub carriers: Vec<Carrier>,
    next_id: u64,
    smoothed: Vec<f32>,
    scratch: Vec<f32>,
    /// Samples since the last start.
    count: u64,
}

impl Detector {
    fn evaluate(&mut self, now: f64) {
        let n = self.n;
        let bin = self.rate / n as f64;
        let f = |i: usize| (i as f64 - (n / 2) as f64) * bin;
        let p: Vec<f32> = (0..n)
            .map(|i| {
                (self.acc[i.saturating_sub(1)] + self.acc[i] + self.acc[(i + 1).min(n - 1)]) / 3.0
            })
            .collect();
        // The median bin is the noise floor. A clean signal without noise has no floor: 50 dB below the strongest bin stands in.
        let peak = p.iter().copied().fold(0.0f32, f32::max);
        let floor = percentile(&p, 0.5, &mut self.scratch)
            .max(peak * 1e-5)
            .max(1e-30);
        self.smoothed = p.iter().map(|v| db((v / floor) as f64) as f32).collect();
        let thr = floor * 2.0;
        let lim = self.limits;
        let usable = |i: usize| f(i).abs() <= 0.45 * self.rate && f(i).abs() >= lim.dc_hz;
        let mut regions = Vec::new();
        let mut i = 0;
        while i < n {
            if usable(i) && p[i] > thr {
                let s = i;
                let mut e = i;
                let mut gap = 0;
                let mut j = i + 1;
                while j < n && usable(j) {
                    if p[j] > thr {
                        e = j;
                        gap = 0;
                    } else {
                        gap += 1;
                        if gap > lim.gap_bins {
                            break;
                        }
                    }
                    j += 1;
                }
                let width = (e - s + 1) as f64 * bin;
                if width <= lim.max_width_hz {
                    let (mut num, mut den, mut pk) = (0.0, 0.0, 0.0f32);
                    for k in s..=e {
                        let w = (p[k] - floor).max(0.0) as f64;
                        num += w * f(k);
                        den += w;
                        pk = pk.max(p[k]);
                    }
                    regions.push((num / den.max(1e-30), width, db((pk / floor) as f64)));
                }
                i = e + 1;
            } else {
                i += 1;
            }
        }
        for r in &regions {
            match self
                .carriers
                .iter_mut()
                .find(|c| (c.offset_hz - r.0).abs() <= (r.1 / 2.0).max(1.5e3))
            {
                Some(c) => {
                    c.last_seen = now;
                    c.offset_hz = r.0;
                    c.width_hz = r.1;
                    c.snr_db = r.2;
                    c.class = class_of(r.1);
                }
                None => {
                    // Confirmed in two searches in a row.
                    if self
                        .prev
                        .iter()
                        .any(|q| (q.0 - r.0).abs() <= (r.1 / 2.0).max(1.5e3))
                    {
                        self.carriers.push(Carrier {
                            id: self.next_id,
                            offset_hz: r.0,
                            width_hz: r.1,
                            snr_db: r.2,
                            class: class_of(r.1),
                            first_seen: now,
                            last_seen: now,
                        });
                        self.next_id += 1;
                    }
                }
            }
        }
        // A carrier not seen for a while is dropped.
        self.carriers.retain(|c| now - c.last_seen <= lim.drop_s);
        self.prev = regions;
    }

    /// Starts on an input at `rate`.
    pub fn start(&mut self, rate: f64) {
        self.rate = rate;
        self.n = ((rate / 250.0) as usize)
            .next_power_of_two()
            .clamp(1024, 131_072);
        self.fft = Some(Fft::forward(self.n));
        self.win = Window::Hann.coefficients(self.n);
        self.frame = Vec::with_capacity(self.n);
        self.acc = vec![0.0; self.n];
        self.skip = ((rate / self.n as f64) / 32.0).floor().max(1.0) as u64;
        self.next_eval = (rate * self.limits.eval_s) as u64;
        self.carriers.clear();
        self.prev.clear();
        self.count = 0;
    }

    /// Takes samples (counted from the last start); the carriers are evaluated every `limits.eval_s` seconds.
    pub fn feed(&mut self, iq: &[Complex32]) {
        let first = self.count;
        self.count += iq.len() as u64;
        let Some(fft) = self.fft.as_mut() else {
            return;
        };
        let n = self.n;
        let nn = n as u64;
        for (k, &v) in iq.iter().enumerate() {
            let at = first + k as u64;
            if (at / nn).is_multiple_of(self.skip) {
                self.frame.push(v);
                if self.frame.len() == n {
                    self.buf.clear();
                    self.buf
                        .extend(self.frame.iter().zip(&self.win).map(|(x, w)| x * *w));
                    fft.process(&mut self.buf);
                    // Bin n/2 holds 0 Hz.
                    for (i, b) in self.buf.iter().enumerate() {
                        self.acc[(i + n / 2) % n] += b.norm_sqr();
                    }
                    self.frame.clear();
                }
            }
            if at + 1 == self.next_eval {
                self.next_eval += (self.rate * self.limits.eval_s) as u64;
                self.evaluate((at + 1) as f64 / self.rate);
                self.acc.iter_mut().for_each(|v| *v = 0.0);
                self.count = at + 1;
                return self.feed(&iq[k + 1..]);
            }
        }
    }

    /// The last smoothed spectrum in 512 columns, dB over the floor.
    pub fn columns(&self) -> Vec<f32> {
        let n = self.smoothed.len();
        if n == 0 {
            return Vec::new();
        }
        (0..512)
            .map(|c| self.smoothed[(c * n / 512).min(n - 1)])
            .collect()
    }
}

/// One Inmarsat-C receiver on a carrier of the capture.
struct Window_ {
    offset_hz: f64,
    demod: inmarsatc::InmcDemod,
    frames: u64,
    born_s: f64,
}

/// The receiver: the carrier detector, the Aero receiver with its SU layer, and up to three Inmarsat-C receivers on the medium
/// carriers the detector confirms. All read the same samples.
#[derive(Default)]
pub struct InmarsatRx {
    rate: f64,
    centre: f64,
    dial: f64,
    det: Detector,
    aero: aero::AeroRx,
    su: aero::SuStage,
    inmc: Vec<Window_>,
    tried: Vec<f64>,
    inmc_enabled: bool,
    /// Samples since the last start.
    samples: u64,
    aero_items: Vec<Timed<AeroItem>>,
    inmc_items: Vec<Timed<InmcItem>>,
}

impl InmarsatRx {
    fn start(&mut self, f: &Format) -> Result<(), ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.dial = f.signal_offset_hz();
        self.det.start(f.rate);
        self.aero.start(f)?;
        self.inmc.clear();
        self.tried.clear();
        self.samples = 0;
        Ok(())
    }

    /// Starts Inmarsat-C receivers on new medium carriers (and narrow ones of 1 kHz or more), and frees those that found no frame
    /// in 25 s.
    fn manage(&mut self, now: f64) {
        let det = &self.det;
        self.inmc.retain(|r| {
            r.frames > 0
                || now - r.born_s < 25.0
                || det
                    .carriers
                    .iter()
                    .all(|c| (c.offset_hz - r.offset_hz).abs() > 3e3)
        });
        if !self.inmc_enabled {
            self.inmc.clear();
            return;
        }
        let mut cands: Vec<&Carrier> = det
            .carriers
            .iter()
            .filter(|c| {
                c.class == CarrierClass::Medium
                    || (c.class == CarrierClass::Narrow && c.width_hz >= 1e3)
            })
            .collect();
        cands.sort_by_key(|c| {
            if c.class == CarrierClass::Medium {
                0
            } else {
                1
            }
        });
        for c in cands {
            if self.inmc.len() >= MAX_INMC
                || self
                    .inmc
                    .iter()
                    .any(|r| (r.offset_hz - c.offset_hz).abs() < 3e3)
                || self.tried.iter().any(|t| (t - c.offset_hz).abs() < 3e3)
            {
                continue;
            }
            let mut d = inmarsatc::InmcDemod::without_search();
            let fmt = Format::iq(self.rate, self.centre, self.centre + c.offset_hz);
            if d.start(&fmt).is_ok() {
                self.inmc.push(Window_ {
                    offset_hz: c.offset_hz,
                    demod: d,
                    frames: 0,
                    born_s: now,
                });
                self.tried.push(c.offset_hz);
            }
        }
    }
}

/// Runs Inmarsat-C decoders on medium carriers.
pub const INMARSAT_C: Setting<field::Bool> = Setting::new("inmarsat_c", "Inmarsat-C", "", true)
    .help("Runs Inmarsat-C decoders on medium carriers.");

sdrkit_core::settings! {
    /// The settings of [`InmarsatRx`].
    pub enum InmarsatRxSetting {
        /// Inmarsat-C.
        InmarsatC = INMARSAT_C,
    }
}

impl Decoder for InmarsatRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.inmarsat_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<AeroItem>("aero").same_format_as(0),
            Port::typed::<InmcItem>("inmc").same_format_as(0),
            Port::typed::<InmItem>("merged").same_format_as(0),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        InmarsatRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(InmarsatRxSetting::InmarsatC, Value::Bool(self.inmc_enabled))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match InmarsatRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            InmarsatRxSetting::InmarsatC => self.inmc_enabled = INMARSAT_C.read(id, v)?,
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.start(f)?;
        Ok(vec![
            f.with_kind(Kind::typed::<AeroItem>()),
            f.with_kind(Kind::typed::<InmcItem>()),
            f.with_kind(Kind::typed::<InmItem>()),
        ])
    }
    fn retune(&mut self, f: &Format) {
        let _ = self.start(f);
    }
    fn process(&mut self, iq: &[Complex32], _first: u64, out: &mut Outbox) {
        if self.rate <= 0.0 {
            return;
        }
        self.det.feed(iq);
        self.aero_items.clear();
        self.aero.feed(iq, &mut self.aero_items);
        self.pass_aero(out);
        self.samples += iq.len() as u64;
        let now = self.samples as f64 / self.rate;
        self.manage(now);
        for (k, w) in self.inmc.iter_mut().enumerate() {
            self.inmc_items.clear();
            w.demod.feed(iq, &mut self.inmc_items);
            for it in self.inmc_items.iter_mut() {
                if let InmcItem::Frame { channel, .. } = &mut it.value {
                    *channel = k;
                    w.frames += 1;
                }
            }
            for it in self.inmc_items.drain(..) {
                out.items::<InmItem>(2).push(Timed {
                    time: it.time,
                    value: InmItem::Inmc(Box::new(it.value.clone())),
                });
                out.items::<InmcItem>(1).push(it);
            }
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        if self.rate <= 0.0 {
            return;
        }
        self.aero_items.clear();
        self.aero.channels(&mut self.aero_items);
        self.pass_aero(out);
        // One status of every Inmarsat-C receiver, numbered by window, each offset from the dial.
        let mut channels = Vec::with_capacity(self.inmc.len());
        let mut time = 0.0;
        for (k, w) in self.inmc.iter_mut().enumerate() {
            self.inmc_items.clear();
            w.demod.status(&mut self.inmc_items);
            for it in self.inmc_items.drain(..) {
                time = it.time;
                if let InmcItem::Status { channels: c } = it.value {
                    channels.extend(c.into_iter().map(|c| ChannelInfo {
                        index: k,
                        offset_hz: w.offset_hz - self.dial,
                        ..c
                    }));
                }
            }
        }
        let status = InmcItem::Status { channels };
        out.items::<InmItem>(2).push(Timed {
            time,
            value: InmItem::Inmc(Box::new(status.clone())),
        });
        out.items::<InmcItem>(1).push(Timed {
            time,
            value: status,
        });
        out.items::<InmItem>(2).push(Timed {
            time,
            value: InmItem::Carriers(self.det.carriers.clone(), self.det.columns()),
        });
    }
}

impl InmarsatRx {
    /// Puts the Aero receiver's items through the SU layer, to the Aero display and (without the SUs) to the merged display.
    fn pass_aero(&mut self, out: &mut Outbox) {
        let mut items = Vec::new();
        for it in &self.aero_items {
            self.su.item(it, &mut items);
        }
        for it in items {
            if !matches!(it.value, AeroItem::Su { .. } | AeroItem::SuType { .. }) {
                out.items::<InmItem>(2).push(Timed {
                    time: it.time,
                    value: InmItem::Aero(Box::new(it.value.clone())),
                });
            }
            out.items::<AeroItem>(0).push(it);
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------------------
// Results
// ------------------------------------------------------------------------------------------------------------------------------

/// A row of the carrier table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CarrierRow {
    pub id: u64,
    pub offset_hz: f64,
    pub frequency_hz: f64,
    pub width_hz: f64,
    pub snr_db: f64,
    pub class: CarrierClass,
    /// What decodes it.
    pub decodes: Decodes,
    pub rate: u32,
    pub suggested_rate: u32,
    pub state: u8,
    pub ebn0_db: f64,
    pub frames: u64,
    pub since_good_s: f64,
    pub satellite: String,
    pub voice_candidate: bool,
    pub parked: bool,
    pub focused: bool,
    pub window: usize,
}

/// The spectrum series: dB over the floor against the offset from the centre.
pub const SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

/// What the merged display publishes.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(InmarsatFields::SHAPE.fields)
                .with_table(&INMARSAT_CARRIERS)
                .with_table(&INMARSAT_CANDIDATES)
                .with_table(&INMARSAT_CANDIDATE_REASONS)
                .series(
                    SPECTRUM.key,
                    SPECTRUM.label,
                    SPECTRUM.x_unit,
                    SPECTRUM.y_unit,
                ),
        )
    })
    .clone()
}

/// The merged display: the carrier table, the windows and the satellite identification.
pub struct InmDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    rate: f64,
    centre_hz: f64,
    carriers: Vec<Carrier>,
    spectrum: Vec<f32>,
    aero: Vec<AeroChannel>,
    inmc: Vec<(f64, u8, u64, f64)>,
    region: Option<u8>,
    sat_lon: Vec<f64>,
    spot: Option<bool>,
    p_freqs: Vec<(f64, u32)>,
    messages: u64,
    positions: u64,
    egc_frames: u64,
    budget: f64,
    focus: u64,
    /// The satellite named by the user: 0 automatic, else the index in the satellite table plus one.
    choice: usize,
    location: Option<(f64, f64)>,
    windows: Vec<(f64, f64)>,
    id: Identification,
    /// The identification the candidate tables show.
    shown_id: Option<Identification>,
    /// The carrier ids the carrier table shows.
    shown_carriers: Vec<u64>,
}

impl Default for InmDisplay {
    fn default() -> Self {
        InmDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            rate: 0.0,
            centre_hz: 0.0,
            carriers: Vec::new(),
            spectrum: Vec::new(),
            aero: Vec::new(),
            inmc: Vec::new(),
            region: None,
            sat_lon: Vec::new(),
            spot: None,
            p_freqs: Vec::new(),
            messages: 0,
            positions: 0,
            egc_frames: 0,
            budget: 10.0,
            focus: 0,
            choice: 0,
            location: None,
            windows: Vec::new(),
            id: Identification::default(),
            shown_id: None,
            shown_carriers: Vec::new(),
        }
    }
}

/// The satellite choices: automatic, then every satellite of the table.
fn satellite_choices() -> Vec<ChoiceDecl> {
    let mut v = vec![ChoiceDecl::new("auto", "Automatic")];
    v.extend(
        satellite::SATELLITES
            .iter()
            .map(|s| ChoiceDecl::new(s.name, s.name)),
    );
    v
}

/// The CPU budget of the windows.
pub const BUDGET: Setting<field::F64> = Setting::new("budget", "CPU budget", "units", 10.0)
    .range(2.0, 24.0, 1.0)
    .help("Aero windows of 160 kHz cost 1, an Inmarsat-C decoder in one 0.6. Windows over the budget are parked.");
/// The satellite named by the user instead of the identification.
pub const SATELLITE_CHOICE: CatalogueSetting = CatalogueSetting {
    key: "satellite",
    label: "Satellite",
    help: "Names the satellite instead of the identification.",
    choices: satellite_choices,
    default: "auto",
    advanced: false,
};
sdrkit_core::commands! {
    /// The commands of [`InmDisplay`].
    pub enum InmDisplayCommand {
        /// Focus on a carrier.
        CarrierFocus = INMARSAT_CARRIER_FOCUS.command,
    }
}

sdrkit_core::settings! {
    /// The settings of [`InmDisplay`].
    pub enum InmDisplaySetting {
        /// CPU budget.
        Budget = BUDGET,
        /// Satellite.
        Satellite = SATELLITE_CHOICE,
    }
}

impl InmDisplay {
    /// Windows of 160 kHz over the carriers, and the ones the budget parks.
    fn plan(&mut self) -> Vec<CarrierRow> {
        let mut cs = self.carriers.clone();
        cs.sort_by(|a, b| b.snr_db.total_cmp(&a.snr_db));
        // Windows stay while they hold a carrier.
        self.windows
            .retain(|w| cs.iter().any(|c| c.offset_hz >= w.0 && c.offset_hz < w.1));
        for c in &cs {
            if !self
                .windows
                .iter()
                .any(|w| c.offset_hz >= w.0 && c.offset_hz < w.1)
            {
                let lo = c.offset_hz - WINDOW_HZ / 2.0;
                self.windows.push((lo, lo + WINDOW_HZ));
            }
        }
        // Cost: an Aero window 1, an Inmarsat-C receiver in it 0.6.
        let mut cost = 0.0;
        let mut parked_windows = Vec::new();
        let focus_w = cs.iter().find(|c| c.id == self.focus).and_then(|c| {
            self.windows
                .iter()
                .position(|w| c.offset_hz >= w.0 && c.offset_hz < w.1)
        });
        for (wi, w) in self.windows.iter().enumerate() {
            let has_c = cs.iter().any(|c| {
                c.offset_hz >= w.0 && c.offset_hz < w.1 && c.class == CarrierClass::Medium
            });
            let wc = 1.0 + if has_c { 0.6 } else { 0.0 };
            if cost + wc > self.budget && Some(wi) != focus_w {
                parked_windows.push(wi);
            } else {
                cost += wc;
            }
        }
        cs.iter()
            .map(|c| {
                let wi = self
                    .windows
                    .iter()
                    .position(|w| c.offset_hz >= w.0 && c.offset_hz < w.1)
                    .unwrap_or(0);
                let mut r = CarrierRow {
                    id: c.id,
                    offset_hz: c.offset_hz,
                    frequency_hz: self.centre_hz + c.offset_hz,
                    width_hz: c.width_hz,
                    snr_db: c.snr_db,
                    class: c.class,
                    decodes: Decodes::Unknown,
                    suggested_rate: if c.class == CarrierClass::Wide {
                        10500
                    } else {
                        600
                    },
                    since_good_s: -1.0,
                    parked: parked_windows.contains(&wi),
                    focused: c.id == self.focus,
                    window: wi,
                    ..Default::default()
                };
                if let Some(a) = self.aero.iter().find(|a| {
                    (a.offset_hz - c.offset_hz).abs() < (c.width_hz / 2.0).max(1.5e3)
                        && a.state >= 2
                }) {
                    r.decodes = Decodes::AeroP;
                    r.rate = a.rate;
                    r.state = a.state;
                    r.ebn0_db = a.ebn0_db;
                    r.frames = a.frames;
                    r.since_good_s = (self.clock.now() - a.last_heard).max(0.0);
                } else if let Some(m) = self
                    .inmc
                    .iter()
                    .find(|m| (m.0 - c.offset_hz).abs() < 2e3 && m.1 >= 1)
                {
                    r.decodes = Decodes::InmarsatC;
                    r.rate = 1200;
                    r.state = m.1;
                    r.frames = m.2;
                    r.ebn0_db = m.3;
                } else if c.class == CarrierClass::Wide && self.clock.now() - c.first_seen > 5.0 {
                    r.voice_candidate = true;
                }
                r
            })
            .collect()
    }
}

impl InmDisplay {
    fn item(&mut self, it: &Timed<InmItem>) {
        let t = self.clock.at(it.time);
        match &it.value {
            InmItem::Carriers(c, s) => {
                self.carriers.clone_from(c);
                self.spectrum.clone_from(s);
            }
            InmItem::Aero(a) => match &**a {
                AeroItem::Channels(ch) => self.aero.clone_from(ch),
                AeroItem::Fact(f) => {
                    if let Some(l) = f.longitude {
                        if !self.sat_lon.iter().any(|x| (x - l).abs() < 0.01) {
                            self.sat_lon.push(l);
                        }
                    }
                    self.spot = Some(self.spot.unwrap_or(false) || f.spot_beam);
                }
                AeroItem::Frame {
                    offset_hz, frame, ..
                } => {
                    let abs = self.centre_hz + *offset_hz;
                    if !self.p_freqs.iter().any(|p| (p.0 - abs).abs() < 1e3) {
                        self.p_freqs.push((abs, frame.rate));
                        if self.p_freqs.len() > 64 {
                            self.p_freqs.remove(0);
                        }
                    }
                }
                AeroItem::Message(m) => {
                    self.messages += 1;
                    if m.lat.is_some() {
                        self.positions += 1;
                    }
                }
                _ => {}
            },
            InmItem::Inmc(i) => match &**i {
                InmcItem::Status { channels } => {
                    self.inmc = channels
                        .iter()
                        .map(|c| {
                            (
                                c.offset_hz + c.status.carrier_hz,
                                c.status.state,
                                c.status.frames_found,
                                c.status.esn0_db + 3.0,
                            )
                        })
                        .collect();
                }
                InmcItem::Frame { frame, .. } => {
                    let pk = icoding::split_packets(&frame.bytes);
                    if let Some((0x7D, p, true)) = pk.first().map(|x| (x.0, x.1, x.2)) {
                        if let Some(b) = egc::Bulletin::parse(p) {
                            self.region = Some(b.ocean_region);
                            self.egc_frames += 1;
                        }
                    }
                }
                _ => {}
            },
        }
        let _ = t;
    }

    fn publish(&mut self, p: &mut Published) {
        let mut rows = self.plan();
        self.id = identify(&Evidence {
            longitudes: self.sat_lon.clone(),
            region: self.region,
            p_freqs: self.p_freqs.clone(),
            location: self.location,
            spot: self.spot,
            manual: self
                .choice
                .checked_sub(1)
                .and_then(|i| satellite::SATELLITES.get(i))
                .map(|s| s.name.to_string()),
        });
        for r in rows.iter_mut() {
            r.satellite = satellite::carrier_label(&self.id, r.frequency_hz, r.rate);
        }
        let decoding = rows.iter().filter(|r| r.state >= 2).count();
        let state = if decoding > 0 {
            2
        } else if !rows.is_empty() {
            1
        } else {
            0
        };
        let snr = rows
            .iter()
            .filter(|r| r.state >= 2)
            .map(|r| r.ebn0_db)
            .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))));
        let lock_state = LockState::from_level(state);
        let snr32 = snr.map(|v| v as f32);
        let (n, messages) = (rows.len(), self.messages);
        p.results_mut().update_status(|st| {
            let mut summary = String::new();
            let _ = write!(
                summary,
                "Inmarsat: {n} carriers, {decoding} decoding, {messages} messages"
            );
            let same = st.lock_state == lock_state && st.snr_db == snr32 && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.snr_db = snr32;
                st.summary = summary;
            }
            !same
        });
        let rate = self.rate;
        let bin = move |c: usize| ((c as f64 / 512.0 - 0.5) * rate) as f32;
        set_series(p.results_mut(), SPECTRUM.key, true, || {
            self.spectrum
                .iter()
                .enumerate()
                .map(move |(c, &v)| [bin(c), v])
        });
        let named = !self.id.name.is_empty();
        p.put(&InmarsatFields {
            frames_ok: (self.messages + self.egc_frames) as i64,
            satellite: named.then_some(self.id.name.as_str()),
            satellite_confidence: named.then_some(self.id.confidence),
            satellite_ambiguous: self.id.ambiguous,
            beam: &self.id.beam,
            identification_note: &self.id.note,
            messages: self.messages as i64,
            positions: self.positions as i64,
            egc_frames: self.egc_frames as i64,
            carriers: rows.len() as i64,
            decoding: decoding as i64,
        });
        let mut key = String::new();
        for gone in self
            .shown_carriers
            .iter()
            .filter(|id| !rows.iter().any(|r| r.id == **id))
        {
            key.clear();
            let _ = write!(key, "{gone:05}");
            p.remove_row(&INMARSAT_CARRIERS, &key);
        }
        self.shown_carriers.clear();
        self.shown_carriers.extend(rows.iter().map(|r| r.id));
        for c in &rows {
            key.clear();
            let _ = write!(key, "{:05}", c.id);
            let row = InmarsatCarrierRow {
                id: c.id as i64,
                offset_hz: Hertz(c.offset_hz),
                frequency: Hertz(c.frequency_hz),
                width: Hertz(c.width_hz),
                snr_db: c.snr_db,
                class: c.class,
                decodes: c.decodes,
                rate: i64::from(c.rate),
                state: i64::from(c.state),
                ebn0_db: c.ebn0_db,
                frames: c.frames as i64,
                satellite: &c.satellite,
                parked: c.parked,
                focused: c.focused,
                window: c.window as i64,
                voice_candidate: c.voice_candidate,
            };
            INMARSAT_CARRIERS.put(p, &key, &row);
        }
        if self.shown_id.as_ref() == Some(&self.id) {
            return;
        }
        self.shown_id = Some(self.id.clone());
        p.clear_rows(&INMARSAT_CANDIDATES);
        p.clear_rows(&INMARSAT_CANDIDATE_REASONS);
        for c in &self.id.candidates {
            let row = InmarsatCandidateRow {
                name: &c.name,
                satellite_confidence: c.confidence,
                content: c.content,
                frequency_score: c.frequency,
                geometry: c.geometry,
                elevation_deg: c.elevation,
                azimuth_deg: c.azimuth,
            };
            INMARSAT_CANDIDATES.put(p, &c.name, &row);
            for (i, w) in c.words.iter().enumerate() {
                key.clear();
                let _ = write!(key, "{}/{i:02}", c.name);
                let row = InmarsatCandidateReasonRow {
                    candidate: &c.name,
                    why: w,
                };
                INMARSAT_CANDIDATE_REASONS.put(p, &key, &row);
            }
        }
    }
}

impl Decoder for InmDisplay {
    type Input = Timed<InmItem>;
    fn name(&self) -> &'static str {
        "satcom.inmarsat_display"
    }
    fn outputs(&self) -> Vec<Port> {
        // The merged display writes no events of its own (the Aero and Inmarsat-C displays do), but a component needs an output.
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        InmDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (InmDisplaySetting::Budget, Value::F64(self.budget)),
            (
                InmDisplaySetting::Satellite,
                Value::Choice(u16::try_from(self.choice).unwrap_or(0)),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match InmDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            InmDisplaySetting::Budget => self.budget = BUDGET.read(id, v)?,
            InmDisplaySetting::Satellite => self.choice = SATELLITE_CHOICE.read(id, v)?,
        }
        Ok(())
    }
    fn commands(&self) -> &'static [&'static Command] {
        InmDisplayCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match InmDisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            InmDisplayCommand::CarrierFocus => {
                let InmarsatCarrierArgs { row } = INMARSAT_CARRIER_FOCUS.args(args)?;
                if row.is_empty() {
                    self.focus = 0;
                } else {
                    self.focus = row
                        .parse::<u64>()
                        .ok()
                        .filter(|id| self.shown_carriers.contains(id))
                        .ok_or_else(|| self.results().no_such_row(&INMARSAT_CARRIERS, row))?;
                }
            }
        }
        Ok(None)
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.rate = f.rate;
        self.centre_hz = f.centre_hz;
        self.windows.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_hz = f.centre_hz;
        self.windows.clear();
        self.carriers.clear();
        self.aero.clear();
        self.inmc.clear();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
        self.location = ctx.station.position.map(|p| (p.lat_deg, p.lon_deg));
    }
    fn process(&mut self, items: &[Timed<InmItem>], _first: u64, _out: &mut Outbox) {
        for it in items {
            self.item(it);
        }
    }
    fn report(&mut self, results: &mut Published) {
        self.publish(results);
    }
    fn clear(&mut self) {
        self.messages = 0;
        self.positions = 0;
        self.egc_frames = 0;
        self.sat_lon.clear();
        self.region = None;
        self.spot = None;
        self.p_freqs.clear();
    }
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.field("messages") {
            self.messages = 0;
        }
        if !kept.field("positions") {
            self.positions = 0;
        }
        if !kept.field("egc_frames") {
            self.egc_frames = 0;
        }
        self.sat_lon.clear();
        self.region = None;
        self.spot = None;
        self.p_freqs.clear();
    }
}

/// The settings of the stack: the receiver's, the merged display's, then the Aero test signal's. Noise and the other
/// impairments of the test signal come from the standard `gen_*` settings.
pub const SETTINGS: &[&dyn DeclareSetting] = &[
    &INMARSAT_C,
    &BUDGET,
    &SATELLITE_CHOICE,
    &aero::gen::GEN_CHANNELS,
    &aero::gen::GEN_PLAIN,
];

/// Every setting of the stack.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(SETTINGS)))
        .clone()
}

/// The receiver: the shared receiver feeding the Aero display, the Inmarsat-C display and the merged display.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(InmarsatRx::default()))
        .add(
            "aero_display",
            DecoderComponent::new(aero::AeroDisplay::default()),
        )
        .add(
            "egc",
            DecoderComponent::new(inmarsatc::InmcDisplay::default()),
        )
        .add("display", DecoderComponent::new(InmDisplay::default()))
        .input("receiver.in")
        .connect("receiver.aero", "aero_display.in")
        .connect("receiver.inmc", "egc.in")
        .connect("receiver.merged", "display.in")
        .output("aero_display.events")
        .output("egc.events")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .costs(&sdrkit_core::CostTable::parse(crate::COSTS), ID, "-")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE,
            "Inmarsat needs a sample rate of at least 600 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .route("inmarsat_c", &["receiver.inmarsat_c"])?
        .route("budget", &["display.budget"])?
        .route("satellite", &["display.satellite"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            INMARSAT_CARRIER_FOCUS.command.key,
            sdrkit_graph::CommandTarget::of("display", INMARSAT_CARRIER_FOCUS.command.key),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut Registry) {
    r.add("satcom.inmarsat_receiver", || {
        Box::new(DecoderComponent::new(InmarsatRx::default()))
    });
    r.add("satcom.inmarsat_display", || {
        Box::new(DecoderComponent::new(InmDisplay::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// The generator: the Aero generator's three P channels and an Inmarsat-C channel at +250 kHz in one wide capture, each at the
/// same energy per channel bit.
pub struct InmGenerator {
    aero: aero::gen::AeroGenerator,
    inmc: inmarsatc::gen::InmcGenerator,
    aero_gain: f32,
    inmc_gain: f32,
    buf: Vec<Complex32>,
}

/// The channel bit rates: the three Aero P channels, and the Inmarsat-C channel.
const AERO_BITS: f64 = 600.0 + 1200.0 + 10500.0;
const INMC_BITS: f64 = 1200.0;

impl InmGenerator {
    /// The generator at `rate` with seed `seed`.
    pub fn new(rate: f64, seed: u64) -> Self {
        let mut a = aero::gen::GenParams::new(rate);
        a.mask = 7;
        a.dial_hz = 0.0;
        let mut c = inmarsatc::gen::GenParams::new(rate);
        c.channel_hz = 250e3;
        // A frame starts about 2 s in, when the detector has confirmed the carrier.
        c.start_symbols = 7_900;
        c.seed = seed;
        let total = AERO_BITS + INMC_BITS;
        InmGenerator {
            aero: aero::gen::AeroGenerator::new(a),
            inmc: inmarsatc::gen::InmcGenerator::new(c),
            aero_gain: (AERO_BITS / total).sqrt() as f32,
            inmc_gain: (INMC_BITS / total).sqrt() as f32,
            buf: Vec::new(),
        }
    }
}

impl Generator for InmGenerator {
    fn rate_hz(&self) -> f64 {
        self.aero.rate_hz()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.aero.generate(out);
        self.buf.resize(out.len(), Complex32::new(0.0, 0.0));
        self.inmc.generate(&mut self.buf);
        for (o, b) in out.iter_mut().zip(self.buf.iter()) {
            *o = *o * self.aero_gain + *b * self.inmc_gain;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Each carrier has power in proportion to its channel bit rate, so one Eb/N0 holds for all of them.
        let bits = AERO_BITS + INMC_BITS;
        Some(SignalInfo::new(1.5 * bits).with_bit_rate(bits))
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(InmGenerator::new(cfg.rate_hz, cfg.seed)))
}

/// The engine's checks of the test signal: carriers are found and both systems decode.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, Aero P and Inmarsat-C carriers", o, 0.0);
    c.seconds = o.seconds.max(28.0);
    c.expect_audio = Some(false);
    c.check = Some(check_carriers);
    vec![c]
}

fn check_carriers(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let num = |k: &str| crate::field(r, k);
    vec![
        CheckLine::new(
            "decoding",
            num("decoding") >= 2.0,
            format!("{} carriers decoding", num("decoding")),
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
    blurb: "One wide L-band capture: every Inmarsat carrier found, classified and decoded at once (Aero P, Inmarsat-C), and the satellite identified. Receive only.",
    tuning: Tuning {
        min_hz: 1525e6,
        max_hz: 1559e6,
        default_hz: 1545.57e6,
        rate_hz: 6e6,
        min_rate_hz: 1.2e6,
        bandwidth_hz: 20e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(5e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::InmarsatC, SignalType::InmarsatAero],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
