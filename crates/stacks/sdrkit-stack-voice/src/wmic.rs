//! Wireless microphone finder (`wmic`): the carrier finder of [`crate::channels`] on a 100 kHz raster, and for each carrier
//! a wide FM candidate receiver that measures the deviation and says whether the carrier looks like analogue FM. One picked
//! candidate plays. The bands that name a candidate come from a plan file that the host names in the `plan` setting; the
//! stack holds no band list.

use crate::channels::{
    CarrierFinder, PlanFile, AUDIO_RATE, CARRIERS_SETTING, GEN_CNR_DB, RASTER_ORIGIN_HZ,
    RASTER_STEP_HZ, SQUELCH,
};
use crate::plans::{self, Bands, WIDE_CARRIERS};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use crate::typed::{
    ListenArgs, WmicCandidateEvent, WmicCandidateRow, WmicFields, WMIC_CANDIDATE,
    WMIC_CANDIDATES, WMIC_LISTEN,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Complex32, ConfigError, Event, LockState, Record, Results, ResultsDecl, Setting,
    SettingError, SettingId, SettingSet, StackConfig, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Kind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_channels::{Carrier, Carriers, Chan, PlayState};
use sdrkit_util_objects::commands as objcmd;
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "wmic";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The rate of a candidate's channel, Hz.
pub const CHANNEL_RATE: f64 = 192_000.0;
/// The width a candidate's channel keeps, Hz.
pub const CHANNEL_WIDTH_HZ: f64 = 160e3;
/// The samples of one measuring window: 0.1 s.
const WINDOW: usize = 19_200;
/// A carrier this near a candidate is a second raster cell of its signal, Hz.
const SAME_SIGNAL_HZ: f64 = CHANNEL_WIDTH_HZ / 2.0;
/// The windows a candidate stays FM after its last window with FM modulation: 2 s, the pauses of speech.
const FM_HOLD: u32 = 20;

/// The band plan: a plan file in the host's storage.
pub const PLAN: Setting<field::Text> = Setting::new("plan", "Band plan", "", "")
    .help("The path of a plan file in the host's storage, in a format that plans.import takes (docs/schema/plans.json). Its bands name the band of each candidate. Empty for no plan: the band of a candidate is then empty.");
/// The candidate that plays.
pub const LISTEN: Setting<field::I64> = Setting::new("listen", "Listen", "", 0)
    .range(0.0, 1e9, 1.0)
    .help("The id of the candidate that plays. 0 plays nothing.");

sdrkit_core::settings! {
    /// The settings of the wireless microphone finder.
    pub enum WmicSetting {
        /// The band plan.
        Plan = PLAN,
        /// The raster step.
        RasterStep = RASTER_STEP_HZ,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN_HZ,
        /// The squelch: the threshold of the carrier finder.
        Squelch = SQUELCH,
        /// The most carriers followed.
        Carriers = CARRIERS_SETTING,
        /// The candidate that plays.
        Listen = LISTEN,
        /// The carrier-to-noise ratio of the test signal.
        GenCnr = GEN_CNR_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Candidates`].
    pub enum CandidateSetting {
        /// The band plan.
        Plan = PLAN,
        /// The candidate that plays.
        Listen = LISTEN,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Candidates`].
    pub enum CandidateCommand {
        /// Forget every candidate.
        Clear = objcmd::CLEAR,
        /// Play a candidate.
        Listen = WMIC_LISTEN.command,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The wide FM receiver of one carrier.
struct Candidate {
    /// The carrier as the finder gave it last, with the cells folded into it.
    carrier: Carrier,
    /// Where the channel sits, absolute, Hz.
    tuned: f64,
    /// The row key: the raster cell, so that the rows are in order of frequency.
    row: String,
    chan: Channelizer,
    prev: Complex32,
    /// The mean of the discriminator: the carrier's offset from the channel, Hz.
    mean: f64,
    samples: u64,
    lp: [Biquad; 2],
    /// The window: its length, the sums of the magnitude and of its square, and the range of the filtered discriminator.
    n: usize,
    mag: f64,
    mag2: f64,
    lo: f32,
    hi: f32,
    /// The running maximum of the window power.
    max_power: f64,
    /// Windows in a row with the carrier active and above 6 dB.
    strong: u32,
    deviation: Option<f64>,
    /// Windows left of the FM hold.
    fm_left: u32,
    /// The FM event went out.
    told_fm: bool,
    phase: u8,
    audio: Vec<f32>,
}

impl Candidate {
    fn new(c: &Carrier, rate: f64, centre: f64) -> Option<Candidate> {
        let tuned = c.centroid_hz;
        let mut cfg = ChannelConfig::new(rate, tuned - centre, CHANNEL_WIDTH_HZ, CHANNEL_RATE);
        cfg.exact_rate = true;
        cfg.atten_db = 60.0;
        let lp = || Biquad::new(BiquadCoeffs::lowpass(CHANNEL_RATE, 15e3, 0.707));
        Some(Candidate {
            carrier: c.clone(),
            tuned,
            row: format!("{:011.0}", c.freq_hz),
            chan: Channelizer::new(cfg).ok()?,
            prev: Complex32::default(),
            mean: 0.0,
            samples: 0,
            lp: [lp(), lp()],
            n: 0,
            mag: 0.0,
            mag2: 0.0,
            lo: f32::MAX,
            hi: f32::MIN,
            max_power: 0.0,
            strong: 0,
            deviation: None,
            fm_left: 0,
            told_fm: false,
            phase: 0,
            audio: Vec::new(),
        })
    }

    /// The frequency of the carrier, Hz.
    fn hz(&self) -> f64 {
        self.tuned + self.mean
    }

    fn fm(&self) -> bool {
        self.fm_left > 0
    }

    /// Takes the cell `c` of the same signal.
    fn fold(&mut self, c: &Carrier) {
        let own = &mut self.carrier;
        own.active |= c.active;
        own.level_dbfs = own.level_dbfs.max(c.level_dbfs);
        own.snr_db = own.snr_db.max(c.snr_db);
        own.width_hz = own.width_hz.max(c.width_hz);
    }

    /// Runs the span samples `x`. With `listen` the audio at 48 kHz goes to `self.audio`. Returns whether the candidate
    /// became FM for the first time.
    fn process(&mut self, x: &[Complex32], scratch: &mut Vec<Complex32>, listen: bool) -> bool {
        scratch.clear();
        self.chan.process(x, scratch);
        let mut news = false;
        for &y in scratch.iter() {
            let d = f64::from((y * self.prev.conj()).arg()) * (CHANNEL_RATE / TAU);
            self.prev = y;
            // A plain mean at first, then a one-pole mean of about 0.25 s.
            self.samples += 1;
            self.mean += (d - self.mean) * (1.0 / self.samples as f64).max(2e-5);
            let first = self.lp[0].process_sample(d as f32);
            let f = self.lp[1].process_sample(first);
            let m = f64::from(y.norm_sqr());
            self.mag += m.sqrt();
            self.mag2 += m;
            self.lo = self.lo.min(f);
            self.hi = self.hi.max(f);
            if listen {
                if self.phase == 0 {
                    let a = (f64::from(f) - self.mean) / 75e3 * 3.0;
                    self.audio.push(a.clamp(-1.0, 1.0) as f32);
                }
                self.phase = (self.phase + 1) % 4;
            }
            self.n += 1;
            if self.n == WINDOW {
                news |= self.end_window();
            }
        }
        news
    }

    /// Ends a window: the carrier is "on" in it when the finder holds it active above 6 dB for the third window in a row
    /// and the window is near full strength. An "on" window gives the deviation and says FM or not.
    fn end_window(&mut self) -> bool {
        let n = self.n as f64;
        let mean = self.mag / n;
        let power = self.mag2 / n;
        let variation = if mean > 0.0 {
            (power - mean * mean).max(0.0).sqrt() / mean
        } else {
            1.0
        };
        // Half the range of the discriminator: an offset of the channel does not count.
        let peak = f64::from(self.hi - self.lo) / 2.0;
        self.max_power = (0.97 * self.max_power).max(power);
        self.strong = if self.carrier.active && self.carrier.snr_db > 6.0 {
            self.strong + 1
        } else {
            0
        };
        let mut news = false;
        if self.strong >= 3 && power > 0.7 * self.max_power {
            self.deviation = Some(self.deviation.map_or(peak, |d| 0.7 * d + 0.3 * peak));
            if variation < 0.35 && peak > 3000.0 {
                self.fm_left = FM_HOLD;
                news = !std::mem::replace(&mut self.told_fm, true);
            } else {
                self.fm_left = self.fm_left.saturating_sub(1);
            }
        }
        self.n = 0;
        self.mag = 0.0;
        self.mag2 = 0.0;
        self.lo = f32::MAX;
        self.hi = f32::MIN;
        news
    }
}

/// The candidate receivers (`wmic.candidates`): a wide FM receiver for each carrier of the finder, the `candidates` table,
/// and the audio of the picked candidate.
pub struct Candidates {
    p: Published,
    bands: PlanFile<Bands>,
    rate: f64,
    centre: f64,
    chains: BTreeMap<u64, Candidate>,
    listen: u64,
    count: u64,
    reports: u64,
    events: Vec<Event>,
    scratch: Vec<Complex32>,
    /// The candidate told to the finder as the one that plays.
    told: Option<u64>,
}

impl Default for Candidates {
    fn default() -> Self {
        let mut p = Published::new(WmicFields::SHAPE.fields).with_tables(&[WMIC_CANDIDATES.def()]);
        p.results_mut().set_status(&Status {
            summary: "Wireless mic finder: waiting for samples".into(),
            ..Status::default()
        });
        Candidates {
            p,
            bands: PlanFile::new(plans::load_bands),
            rate: 0.0,
            centre: 0.0,
            chains: BTreeMap::new(),
            listen: 0,
            count: 0,
            reports: 0,
            events: Vec::new(),
            scratch: Vec::new(),
            told: None,
        }
    }
}

impl Candidates {
    /// The band of the plan that holds `hz`. Empty without a plan and outside every band.
    fn band(&self, hz: f64) -> &str {
        self.bands
            .plan
            .as_ref()
            .and_then(|b| b.name_of(hz))
            .unwrap_or("")
    }

    /// The event of candidate `c` as it is now.
    fn event(&self, c: &Candidate) -> Event {
        let hz = c.hz();
        WMIC_CANDIDATE.event(
            self.count as f64 / self.rate.max(1.0),
            hz,
            &WmicCandidateEvent {
                id: c.carrier.id as i64,
                hz: Hertz(hz),
                width_hz: Hertz(c.carrier.width_hz),
                level_dbfs: c.carrier.level_dbfs,
                snr_db: c.carrier.snr_db,
                deviation_hz: c.deviation.map(Hertz),
                fm: c.fm(),
                band: self.band(hz),
            },
        )
    }

    /// Takes the carrier list `list` of the finder: a new carrier gets a receiver, unless it is a second cell of a signal
    /// that has one; a carrier that the finder dropped loses its receiver and its row.
    fn carriers(&mut self, list: &[Carrier]) {
        let gone: Vec<u64> = self
            .chains
            .keys()
            .copied()
            .filter(|id| !list.iter().any(|c| c.id == *id))
            .collect();
        for id in gone {
            if let Some(ch) = self.chains.remove(&id) {
                self.p.remove_row(&WMIC_CANDIDATES, &ch.row);
            }
        }
        for c in list {
            if let Some(ch) = self.chains.get_mut(&c.id) {
                ch.carrier = c.clone();
            }
        }
        for c in list {
            if self.chains.contains_key(&c.id) {
                continue;
            }
            let same = self
                .chains
                .values_mut()
                .find(|ch| (c.centroid_hz - ch.hz()).abs() < SAME_SIGNAL_HZ);
            if let Some(ch) = same {
                ch.fold(c);
            } else if let Some(ch) = Candidate::new(c, self.rate, self.centre) {
                self.events.push(self.event(&ch));
                self.chains.insert(c.id, ch);
            }
        }
    }

    fn report(&mut self) {
        self.reports += 1;
        let now = self.count as f64 / self.rate.max(1.0);
        let p = &mut self.p;
        p.put(&WmicFields {
            candidates: self.chains.len() as i64,
            plan: self.bands.plan.as_ref().map(|b| b.name.as_str()),
            input_rate_hz: Hertz(self.rate),
            centre_hz: Hertz(self.centre),
            signal_time_s: Seconds(now),
            reports: self.reports as i64,
        });
        // A row that is written becomes the newest: the rows are written in order of frequency, as the row keys are.
        let mut order: Vec<(&u64, &Candidate)> = self.chains.iter().collect();
        order.sort_by(|a, b| a.1.row.cmp(&b.1.row));
        for (id, c) in order {
            let hz = c.hz();
            let band = self.bands.plan.as_ref().and_then(|b| b.name_of(hz));
            WMIC_CANDIDATES.put(
                p,
                &c.row,
                &WmicCandidateRow {
                    id: *id as i64,
                    hz: Hertz(hz),
                    width_hz: Hertz(c.carrier.width_hz),
                    level_dbfs: c.carrier.level_dbfs,
                    snr_db: c.carrier.snr_db,
                    deviation_hz: c.deviation.map(Hertz),
                    fm: c.fm(),
                    active: c.carrier.active,
                    listening: *id == self.listen,
                    age_s: Seconds(c.carrier.age_s),
                    band: band.unwrap_or(""),
                },
            );
        }
        let lock_state = if !self.chains.is_empty() {
            LockState::Locked
        } else if self.count > 0 {
            LockState::Searching
        } else {
            LockState::Off
        };
        let summary = if lock_state == LockState::Off {
            "Wireless mic finder: waiting for samples".into()
        } else {
            format!(
                "Wireless mic finder: {} candidates{}",
                self.chains.len(),
                self.bands.status()
            )
        };
        p.results_mut().set_status(&Status {
            lock_state,
            summary,
            ..Status::default()
        });
    }

    /// Forgets every candidate and its row.
    fn forget(&mut self) {
        for ch in self.chains.values() {
            self.p.remove_row(&WMIC_CANDIDATES, &ch.row);
        }
        self.chains.clear();
    }
}

impl Component for Candidates {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wmic.candidates",
            vec![Port::iq("in"), Port::typed::<Carriers>("carriers")],
            vec![
                Port::audio("out").rate_from(RateFrom::Fixed(AUDIO_RATE)),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<PlayState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(CandidateSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.count = 0;
        self.told = None;
        self.forget();
        Ok(vec![
            Format::new(Kind::Audio, AUDIO_RATE),
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<PlayState>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.bands.attach(ctx);
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(cl) = io.items::<Carriers>(1).last() {
            self.carriers(&cl.value.0);
        }
        let x = io.iq(0);
        self.count += x.len() as u64;
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut news = Vec::new();
        for (id, ch) in &mut self.chains {
            if ch.process(x, &mut scratch, *id == self.listen) {
                news.push(*id);
            }
        }
        self.scratch = scratch;
        for id in news {
            if let Some(ch) = self.chains.get(&id) {
                self.events.push(self.event(ch));
            }
        }
        if let Some(ch) = self.chains.get_mut(&self.listen) {
            io.audio_out(0).samples.append(&mut ch.audio);
        }
        io.messages_out(1).append(&mut self.events);
        // The finder keeps the candidate that plays when it has more carriers than it may follow.
        let playing = self.chains.get(&self.listen).map(|ch| ch.carrier.id);
        if playing != self.told {
            self.told = playing;
            let chan = self.chains.get(&self.listen).map(|ch| Chan {
                key: ch.carrier.id as i64,
                hz: ch.carrier.freq_hz,
            });
            io.items_out::<PlayState>(2).push(Timed {
                time: self.count as f64 / self.rate.max(1.0),
                value: PlayState {
                    playing: chan,
                    ..PlayState::default()
                },
            });
        }
        if io.tick() {
            self.report();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CandidateSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CandidateSetting::Plan => {
                self.bands.set_path(&PLAN, id, v)?;
            }
            CandidateSetting::Listen => self.listen = LISTEN.read(id, v)? as u64,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (CandidateSetting::Plan, Value::Text(self.bands.path.clone())),
            (CandidateSetting::Listen, Value::I64(self.listen as i64)),
        ])
    }
    fn commands(&self) -> &Commands {
        CandidateCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match CandidateCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            CandidateCommand::Clear => self.forget(),
            CandidateCommand::Listen => {
                let pick = WMIC_LISTEN.args::<ListenArgs>(args)?.id.max(0) as u64;
                self.listen = if pick == self.listen { 0 } else { pick };
            }
        }
        self.report();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

/// The candidate component.
pub fn register(r: &mut Registry) {
    r.add("wmic.candidates", || Box::new(Candidates::default()));
}

/// The graph: the carrier finder for wide carriers, and the candidate receivers.
pub fn builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("finder", CarrierFinder::with_profile(WIDE_CARRIERS))
        .add("candidates", Candidates::default())
        .input("finder.in")
        .input("candidates.in")
        .connect("finder.carriers", "candidates.carriers")
        .feedback("candidates.state", "finder.playing")
        .output("candidates.out")
        .output("candidates.events")
        .status_from("candidates")
        .clock(0.1)
        .reconfigure_on_retune()
}

/// The graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder().catalog(catalog).build()
}

/// The stack graph: the settings routed to the components that own them, and the commands of the candidates.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "Wireless mic finder: the sample rate is too low (1 MHz at least)",
        )
        .route(PLAN.key(), &["candidates.plan"])?
        .route(RASTER_STEP_HZ.key(), &["finder.raster_step_hz"])?
        .route(RASTER_ORIGIN_HZ.key(), &["finder.raster_origin_hz"])?
        .route(SQUELCH.key(), &["finder.squelch"])?
        .route(CARRIERS_SETTING.key(), &["finder.carriers"])?
        .route(LISTEN.key(), &["candidates.listen"])?
        .command(
            "clear",
            sdrkit_graph::CommandTarget::of("candidates", "clear"),
        )
        .command(
            "listen",
            sdrkit_graph::CommandTarget::of("candidates", "listen"),
        )
        .with_settings(&cfg.settings)
        .build()
}
