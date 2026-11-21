//! The sweep machine: it says where to tune next, takes the spectrum frames of each hop, stitches the hops into sweeps and
//! analyses each sweep. The service owns it and the engine drives it through the service.

use crate::burst::{Burst, BurstMonitor, DriftConfig, DriftFinder};
use crate::channels::{guess_type, ActionOutcome, ChannelConfig, ChannelList, HitAction};
use crate::floor::{DynamicFloor, FloorConfig, FloorSnapshot};
use crate::grid::{fft_size_for, Grid, MAX_FFT, MAX_FFT_RADIO_SWEEP, UNCOVERED_DB};
use crate::health::{assess, HealthBand, HealthReport};
use crate::hits::{detect, Detection, HitConfig, HitEvent, HitTracker};
use crate::hunter::{Hunter, HunterConfig};
use crate::plan::{HopPlan, PlanConfig, Range};
use crate::profile::{Purpose, ScanProfile};
use crate::record::{self, OpenHit, RecordRequest};
use crate::schedule::{Next, Scheduler};
use crate::settle::{SettleMeter, SettleStep};
use crate::stitch::Stitcher;
use crate::survey::{BandSurvey, SurveyConfig};
use crate::traces::{TraceConfig, Traces};
use crate::{db, lin, SweepError};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::Complex32;

/// What the sweep requests from the engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Tune the radio to `centre_hz` at `rate_hz`, then feed what `input` names, tagged with the centre it was captured at.
    Tune {
        /// The centre, Hz.
        centre_hz: f64,
        /// The sample rate, Hz.
        rate_hz: f64,
        /// What to feed.
        input: Input,
    },
    /// Let the radio sweep `low_hz .. high_hz` by itself and feed spectrum frames tagged with the centre of each block. If
    /// the radio cannot, call [`crate::SweepService::radio_sweep_failed`].
    RadioSweep {
        /// The low end, Hz.
        low_hz: f64,
        /// The high end, Hz.
        high_hz: f64,
        /// The sample rate, Hz.
        rate_hz: f64,
    },
    /// Nothing to capture until `until_s`, signal time. Frames that come are not used.
    Wait {
        /// When to step again, s.
        until_s: f64,
    },
    /// The sweep is over. Tune back to `return_hz`.
    Done {
        /// Where the session was tuned when the sweep started, Hz.
        return_hz: f64,
    },
}

/// What the engine feeds after a [`Step::Tune`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Spectrum frames as [`crate::SweepService::spectrum`] declares them.
    Spectrum,
    /// IQ samples, to measure the settle time of the radio after a retune.
    Samples,
}

/// One spectrum frame: dBFS per bin, lowest frequency first, the centre at bin `len / 2`. A complex tone of magnitude 1.0 on
/// a bin reads 0 dBFS.
#[derive(Clone, Copy, Debug)]
pub struct SpectrumFrame<'a> {
    /// The signal time of the frame, s.
    pub time_s: f64,
    /// The centre the radio was tuned to when the samples of the frame were captured, Hz.
    pub centre_hz: f64,
    /// The span of the frame: the sample rate, Hz.
    pub span_hz: f64,
    /// The bins.
    pub bins_db: &'a [f32],
}

/// The spectrum that a sweep needs: frames of `bins` bins with `window`, made back to back from the samples at `rate_hz`
/// (`rate_hz / bins` frames per second), not averaged. The sweep averages the frames of each hop itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumNeed {
    /// The bins per frame, a power of two from 256 to 65 536.
    pub bins: usize,
    /// The window.
    pub window: Window,
    /// The sample rate, Hz.
    pub rate_hz: f64,
    /// The frames averaged before a frame is given: always 1.
    pub average: u32,
}

/// The radio properties the sweep uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadioInfo {
    /// The lowest frequency it tunes, Hz.
    pub min_hz: f64,
    /// The highest frequency it tunes, Hz.
    pub max_hz: f64,
    /// Its sample rate now, Hz.
    pub rate_hz: f64,
    /// It can sweep by itself.
    pub can_sweep: bool,
    /// Where the session is tuned now, Hz.
    pub tuned_hz: f64,
}

sdrkit_core::choice_enum! {
    /// Where a sweep is.
    pub enum Phase {
        /// No sweep runs.
        Idle = ("idle", "Idle"),
        /// Measuring the settle time of the radio.
        Settle = ("settle", "Settle"),
        /// Capturing hops.
        Hop = ("hop", "Hop"),
        /// The radio sweeps by itself.
        RadioSweep = ("radio_sweep", "Radio sweep"),
        /// Waiting for the next sweep.
        Wait = ("wait", "Wait"),
        /// The sweep ended.
        Done = ("done", "Done"),
        /// The sweep did not start.
        Failed = ("failed", "Failed"),
    }
}

sdrkit_core::choice_enum! {
    /// Whether the sweep of the radio itself is used.
    pub enum RadioSweepUse {
        /// It was not tried: the profile does not ask for it, or the radio cannot.
        NotTried = ("not_tried", "Not tried"),
        /// The radio sweeps by itself.
        Used = ("used", "Used"),
        /// The engine said that the radio cannot sweep. The sweep retunes.
        Refused = ("refused", "Refused"),
    }
}

/// The sweep machine.
pub(crate) struct Machine {
    pub profile: ScanProfile,
    radio: RadioInfo,
    rate: f64,
    return_hz: f64,
    pub grid: Grid,
    plan_cfg: PlanConfig,
    pub plan: HopPlan,
    hop_ranges: Vec<std::ops::Range<usize>>,
    replan: bool,
    fft: usize,
    drop_frames: usize,
    frames_seen: usize,
    frame_acc: Vec<f32>,
    stitcher: Stitcher,
    pub floor: DynamicFloor,
    tracker: HitTracker,
    pub channels: ChannelList,
    pub traces: Traces,
    pub survey: Option<BandSurvey>,
    pub hunter: Option<Hunter>,
    health_bands: Vec<HealthBand>,
    pub health: Option<HealthReport>,
    pub burst: Option<(f64, BurstMonitor)>,
    pub drift: Option<DriftFinder>,
    scheduler: Scheduler,
    settle: Option<SettleMeter>,
    pub settle_s: f64,
    pub settle_measured: bool,
    pub phase: Phase,
    pub pos: usize,
    expect_hz: f64,
    wait_until: f64,
    pending: Option<usize>,
    pending_power: Vec<f32>,
    hop_db: Vec<f32>,
    park_since: Option<f64>,
    pub sweeps: u64,
    pub hops_done: u64,
    pub sweep_db: Vec<f32>,
    pub radio_sweep: RadioSweepUse,
    rs_centre: Option<f64>,
    rs_done: bool,
    mask: Vec<bool>,
    dets: Vec<Detection>,
    runs: Vec<(usize, usize)>,
    open_hits: Vec<OpenHit>,
    pub recording: Vec<RecordRequest>,
    pub cpu_load: f64,
    pub hit_events: Vec<HitEvent>,
    pub new_burst: Option<Burst>,
}

impl std::fmt::Debug for Machine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Machine")
            .field("profile", &self.profile.name)
            .field("grid", &self.grid)
            .field("hops", &self.plan.len())
            .field("phase", &self.phase)
            .field("sweeps", &self.sweeps)
            .finish()
    }
}

fn plan_config(
    profile: &ScanProfile,
    low: f64,
    high: f64,
    rate: f64,
    radio: &RadioInfo,
) -> PlanConfig {
    PlanConfig {
        low_hz: low,
        high_hz: high,
        rate_hz: rate,
        usable: profile.usable,
        overlap: profile.overlap,
        exclusions: profile.all_exclusions(),
        priority_hz: profile.priority_hz.clone(),
        priority_every: profile.priority_every,
        tune_limits: Some(Range::new(radio.min_hz, radio.max_hz)),
    }
}

/// The burst monitor and the drifting-carrier finder of a sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Watches {
    /// Watches this frequency for bursts, with the rise in dB and the shortest burst in s.
    pub burst: Option<(f64, f32, f64)>,
    /// Finds drifting carriers.
    pub drift: bool,
    /// The rows of a survey.
    pub survey_rows: usize,
}

impl Machine {
    /// A sweep of `profile` on `radio`.
    ///
    /// # Errors
    ///
    /// [`SweepError`] if the profile has no band, the bands lie outside the radio, the rate is not positive or every hop is
    /// excluded.
    pub fn new(
        profile: ScanProfile,
        radio: RadioInfo,
        watches: Watches,
    ) -> Result<Self, SweepError> {
        profile.validate()?;
        let span = profile.span().ok_or(SweepError::EmptyRange {
            low_hz: 0.0,
            high_hz: 0.0,
        })?;
        let low = span.low_hz.max(radio.min_hz);
        let high = span.high_hz.min(radio.max_hz);
        if high.is_nan() || low.is_nan() || high <= low {
            return Err(SweepError::OutsideRadio {
                low_hz: span.low_hz,
                high_hz: span.high_hz,
            });
        }
        let rate = profile.rate_hz.unwrap_or(radio.rate_hz);
        if !(rate.is_finite() && rate > 0.0) {
            return Err(SweepError::BadRate { rate_hz: rate });
        }
        let max_fft = if profile.radio_sweep && radio.can_sweep {
            MAX_FFT_RADIO_SWEEP
        } else {
            MAX_FFT
        };
        let fft = fft_size_for(rate, profile.bin_hz, max_fft).max(256);
        let grid = Grid::new(low, high, profile.bin_hz)?;
        let plan_cfg = plan_config(&profile, low, high, rate, &radio);
        let plan = HopPlan::new(&plan_cfg)?;
        let hop_ranges = plan
            .hops
            .iter()
            .map(|h| grid.bins_between(h.keep_lo_hz, h.keep_hi_hz))
            .collect();
        let mut stitcher = Stitcher::new(grid, fft, rate, profile.stitch.clone());
        stitcher.reserve_hops(plan.order.len() * 2 + 8);
        let hit_cfg = HitConfig {
            open_db: profile.threshold_db,
            close_db: (profile.threshold_db - profile.hysteresis_db).max(0.5),
            min_duration_s: profile.min_duration_s,
            hang_s: profile.hang_s,
            ..Default::default()
        };
        let channels = ChannelList::new(ChannelConfig {
            min_sweeps: profile.min_sweeps,
            raster: profile.raster,
            ..Default::default()
        });
        let traces = Traces::new(
            grid,
            TraceConfig {
                averaging: profile.averaging.max(1),
                ..Default::default()
            },
        );
        let survey = (profile.purpose == Purpose::Survey).then(|| {
            BandSurvey::new(
                grid,
                SurveyConfig {
                    threshold_db: profile.threshold_db,
                    max_rows: watches.survey_rows,
                },
            )
        });
        let hunter =
            (profile.purpose == Purpose::Hunt).then(|| Hunter::new(grid, HunterConfig::default()));
        let health_bands = if profile.purpose == Purpose::Health {
            profile
                .bands
                .iter()
                .map(|b| HealthBand {
                    name: b.label,
                    low_hz: b.low_hz,
                    high_hz: b.high_hz,
                    expect_stations: b.label == "FM broadcast",
                })
                .collect()
        } else {
            Vec::new()
        };
        let settle = match profile.settle_s {
            Some(_) => None,
            None => Some(SettleMeter::new(
                plan.hops[0].centre_hz,
                plan.hops[plan.len() - 1].centre_hz,
                rate,
                2,
                0.05,
            )),
        };
        Ok(Machine {
            settle_s: profile.settle_s.unwrap_or(0.004),
            scheduler: Scheduler::new(profile.schedule),
            pending_power: vec![0.0; fft],
            frame_acc: vec![0.0; fft],
            hop_db: Vec::with_capacity(fft),
            sweep_db: vec![UNCOVERED_DB; grid.bins],
            floor: DynamicFloor::new(grid, FloorConfig::default()),
            tracker: HitTracker::new(hit_cfg),
            mask: vec![false; grid.bins],
            dets: Vec::with_capacity(256),
            runs: Vec::with_capacity(256),
            open_hits: Vec::new(),
            burst: watches
                .burst
                .map(|(hz, rise, min_s)| (hz, BurstMonitor::new(rise, min_s))),
            drift: watches
                .drift
                .then(|| DriftFinder::new(DriftConfig::default())),
            radio,
            rate,
            return_hz: radio.tuned_hz,
            grid,
            plan_cfg,
            plan,
            hop_ranges,
            replan: false,
            fft,
            drop_frames: 0,
            frames_seen: 0,
            stitcher,
            channels,
            traces,
            survey,
            hunter,
            health_bands,
            health: None,
            settle,
            settle_measured: false,
            phase: Phase::Settle,
            pos: 0,
            expect_hz: f64::NAN,
            wait_until: 0.0,
            pending: None,
            park_since: None,
            sweeps: 0,
            hops_done: 0,
            radio_sweep: RadioSweepUse::NotTried,
            rs_centre: None,
            rs_done: false,
            recording: Vec::new(),
            cpu_load: 0.0,
            hit_events: Vec::new(),
            new_burst: None,
            profile,
        })
    }

    /// The spectrum that the sweep needs now.
    pub fn spectrum(&self) -> SpectrumNeed {
        SpectrumNeed {
            bins: self.fft,
            window: Window::BlackmanHarris4,
            rate_hz: self.rate,
            average: 1,
        }
    }

    /// The sample rate in use, Hz.
    pub fn rate_hz(&self) -> f64 {
        self.rate
    }

    /// The bins of a hop spectrum.
    pub fn fft(&self) -> usize {
        self.fft
    }

    /// The hops of one sweep.
    pub fn hops(&self) -> usize {
        self.plan.order.len()
    }

    /// The bins of one line of the rtl_power file.
    pub fn bins_per_hop(&self) -> usize {
        self.plan
            .hops
            .first()
            .map_or(1, |h| self.hop_ranges[h.index].len().max(1))
    }

    /// The samples behind one value of a sweep.
    pub fn samples_per_value(&self) -> u64 {
        (self.fft * self.profile.averages) as u64
    }

    /// The resolution: the larger of the grid bin and the -6 dB width of the window (2.7 bins for Blackman-Harris).
    pub fn res_hz(&self) -> f64 {
        self.grid
            .bin_hz
            .max(2.7 * self.stitcher.rate_hz() / self.stitcher.fft_size() as f64)
    }

    /// The first step, at `now_s`.
    pub fn start(&mut self, now_s: f64) -> Step {
        if let Some(m) = &self.settle {
            self.phase = Phase::Settle;
            self.expect_hz = m.first();
            return Step::Tune {
                centre_hz: self.expect_hz,
                rate_hz: self.rate,
                input: Input::Samples,
            };
        }
        self.begin_sweep(now_s)
    }

    fn drop_for(&self) -> usize {
        let samples = (self.settle_s * self.rate).round() as usize + self.profile.flush_samples;
        samples.div_ceil(self.fft)
    }

    fn begin_sweep(&mut self, now_s: f64) -> Step {
        if self.replan {
            self.replan = false;
            if let Ok(p) = HopPlan::new(&self.plan_cfg) {
                self.hop_ranges.clear();
                self.hop_ranges.extend(
                    p.hops
                        .iter()
                        .map(|h| self.grid.bins_between(h.keep_lo_hz, h.keep_hi_hz)),
                );
                self.stitcher.reserve_hops(p.order.len() * 2 + 8);
                self.plan = p;
            }
        }
        self.stitcher.begin_sweep();
        self.scheduler.started(now_s);
        self.pos = 0;
        let (low, high) = (self.plan_cfg.low_hz, self.plan_cfg.high_hz);
        if self.profile.radio_sweep
            && self.radio.can_sweep
            && self.radio_sweep != RadioSweepUse::Refused
            && high - low > self.plan.keep_hz
        {
            self.phase = Phase::RadioSweep;
            self.radio_sweep = RadioSweepUse::Used;
            self.rs_centre = None;
            self.rs_done = false;
            self.stitcher.set_inner_keep_hz(0.125 * self.rate);
            return Step::RadioSweep {
                low_hz: low,
                high_hz: high,
                rate_hz: self.rate,
            };
        }
        self.stitcher
            .set_inner_keep_hz(self.profile.stitch.inner_keep_hz);
        self.tune_hop()
    }

    fn tune_hop(&mut self) -> Step {
        let hop = self.plan.hops[self.plan.order[self.pos]];
        self.expect_hz = hop.centre_hz;
        self.begin_capture(self.drop_for());
        self.phase = Phase::Hop;
        Step::Tune {
            centre_hz: hop.centre_hz,
            rate_hz: self.rate,
            input: Input::Spectrum,
        }
    }

    fn begin_capture(&mut self, drop_frames: usize) {
        self.drop_frames = drop_frames;
        self.frames_seen = 0;
    }

    fn same_centre(&self, hz: f64) -> bool {
        (hz - self.expect_hz).abs() <= 1.0
    }

    /// Adds a frame to the hop. Returns true when the hop has its frames: their mean power is in `pending_power`.
    fn add_frame(&mut self, bins_db: &[f32]) -> bool {
        self.frames_seen += 1;
        if self.frames_seen <= self.drop_frames {
            return false;
        }
        let got = self.frames_seen - self.drop_frames;
        if got == 1 {
            for (a, &d) in self.frame_acc.iter_mut().zip(bins_db) {
                *a = lin(d);
            }
        } else {
            for (a, &d) in self.frame_acc.iter_mut().zip(bins_db) {
                *a += lin(d);
            }
        }
        if got < self.profile.averages.max(1) {
            return false;
        }
        let k = 1.0 / got as f32;
        for (p, a) in self.pending_power.iter_mut().zip(&self.frame_acc) {
            *p = a * k;
        }
        true
    }

    /// Stitches the hop captured last. It runs while the next hop is captured.
    fn process_pending(&mut self, now_s: f64) {
        let Some(h) = self.pending.take() else {
            return;
        };
        let hop = self.plan.hops[h];
        self.stitcher.add_hop(
            hop.centre_hz,
            hop.keep_lo_hz,
            hop.keep_hi_hz,
            &self.pending_power,
        );
        self.hops_done += 1;
        if let Some(d) = &mut self.drift {
            let n = self.pending_power.len();
            self.hop_db.clear();
            self.hop_db
                .extend(self.pending_power.iter().map(|&p| db(p)));
            let bin = self.rate / n as f64;
            d.update(now_s, hop.centre_hz - self.rate / 2.0, bin, &self.hop_db);
        }
    }

    /// The power of a hop is in `pending_power`: parks on the hop while its hits are recorded, else moves on.
    fn hop_complete(&mut self, now_s: f64) -> Option<Step> {
        let hop_index = self.plan.order[self.pos];
        self.pending = Some(hop_index);
        if self.profile.record.enabled {
            self.process_pending(now_s);
            let hop = self.plan.hops[hop_index];
            let thr = self.profile.threshold_db;
            let floor = self.floor.floor_db();
            let res = self.res_hz();
            self.open_hits.clear();
            for t in self.tracker.tracks() {
                if !t.is_open() || t.centre_hz < hop.keep_lo_hz || t.centre_hz > hop.keep_hi_hz {
                    continue;
                }
                let Some(k) = self.grid.index_of(t.centre_hz) else {
                    continue;
                };
                let level = self.stitcher.partial_db(k);
                if floor[k].is_finite() && level > floor[k] + thr {
                    let kind = guess_type(
                        t.width_hz,
                        t.centre_hz,
                        t.share_present(),
                        t.count,
                        t.peak_to_mean_db,
                        res,
                    );
                    self.open_hits.push(OpenHit {
                        id: t.id,
                        hz: t.centre_hz,
                        width_hz: t.width_hz,
                        snr_db: t.snr_db,
                        demod: kind.demod(),
                    });
                }
            }
            let since = *self.park_since.get_or_insert(now_s);
            record::choose(
                &self.profile.record,
                self.cpu_load,
                &mut self.open_hits,
                &mut self.recording,
            );
            if !self.recording.is_empty() && now_s - since < self.profile.record.max_park_s {
                self.begin_capture(0);
                return None;
            }
            self.recording.clear();
            self.park_since = None;
        }
        self.pos += 1;
        if self.pos < self.plan.order.len() {
            Some(self.tune_hop())
        } else {
            self.process_pending(now_s);
            Some(self.end_sweep(now_s))
        }
    }

    fn end_sweep(&mut self, now_s: f64) -> Step {
        self.analyse(now_s);
        match self.scheduler.finished(now_s) {
            Next::Now => self.begin_sweep(now_s),
            Next::At(t) => {
                self.phase = Phase::Wait;
                self.wait_until = t;
                Step::Wait { until_s: t }
            }
            Next::Finished => self.finish(),
        }
    }

    /// Ends the sweep. Returns the step that tunes back.
    pub fn finish(&mut self) -> Step {
        self.phase = Phase::Done;
        self.recording.clear();
        if let Some(d) = &mut self.drift {
            d.flush();
        }
        Step::Done {
            return_hz: self.return_hz,
        }
    }

    /// Ends the sweep after the analysis of the hops captured so far.
    pub fn finish_now(&mut self, now_s: f64) -> Step {
        if self.phase != Phase::Done {
            self.process_pending(now_s);
            if self.stitcher.hops() > 0 {
                self.analyse(now_s);
            }
        }
        self.finish()
    }

    fn analyse(&mut self, now_s: f64) {
        let stitched = self.stitcher.finish_sweep();
        self.sweep_db.copy_from_slice(stitched);
        self.floor.update(&self.sweep_db, &self.hop_ranges);
        self.channels
            .mask_bins(&self.grid, &self.plan_cfg.exclusions, &mut self.mask);
        detect(
            &self.grid,
            &self.sweep_db,
            self.floor.floor_db(),
            &self.mask,
            self.tracker.config(),
            &mut self.runs,
            &mut self.dets,
        );
        self.hit_events.clear();
        let events = self.tracker.update(now_s, &self.dets, self.grid.bin_hz);
        self.hit_events.extend_from_slice(events);
        let res = self.res_hz();
        self.channels.update(self.tracker.tracks(), &self.plan, res);
        self.traces.update(&self.sweep_db);
        if let Some(s) = &mut self.survey {
            s.add(now_s, &self.sweep_db, self.floor.floor_db());
        }
        if let Some(h) = &mut self.hunter {
            h.update(now_s, &self.sweep_db);
        }
        if !self.health_bands.is_empty() {
            self.health = Some(assess(&self.grid, &self.sweep_db, &self.health_bands));
        }
        self.new_burst = None;
        if let Some((hz, m)) = &mut self.burst {
            if let Some(k) = self.grid.index_of(*hz) {
                if self.sweep_db[k] > UNCOVERED_DB {
                    self.new_burst = m.update(now_s, self.sweep_db[k]);
                }
            }
        }
        self.sweeps += 1;
    }

    /// The track that a hit event is about.
    pub fn track(&self, id: u64) -> Option<&crate::hits::Track> {
        self.tracker.track(id)
    }

    /// IQ samples captured at `centre_hz`, while the settle time is measured.
    pub fn feed_samples(&mut self, now_s: f64, centre_hz: f64, iq: &[Complex32]) -> Option<Step> {
        if self.phase != Phase::Settle || !self.same_centre(centre_hz) {
            return None;
        }
        let m = self.settle.as_mut()?;
        match m.push(iq)? {
            SettleStep::Retune(hz) => {
                self.expect_hz = hz;
                Some(Step::Tune {
                    centre_hz: hz,
                    rate_hz: self.rate,
                    input: Input::Samples,
                })
            }
            SettleStep::Done(s) => {
                self.settle_s = s;
                self.settle_measured = true;
                self.settle = None;
                Some(self.begin_sweep(now_s))
            }
        }
    }

    /// A spectrum frame.
    ///
    /// # Errors
    ///
    /// [`SweepError::FrameShape`] if the frame does not have the bins and the span of [`Machine::spectrum`].
    pub fn feed(&mut self, f: &SpectrumFrame<'_>) -> Result<Option<Step>, SweepError> {
        let now_s = f.time_s;
        match self.phase {
            Phase::Settle => {
                self.settle = None;
                Ok(Some(self.begin_sweep(now_s)))
            }
            Phase::Hop => {
                self.process_pending(now_s);
                if !self.same_centre(f.centre_hz) {
                    return Ok(None);
                }
                self.check(f)?;
                if !self.add_frame(f.bins_db) {
                    return Ok(None);
                }
                Ok(self.hop_complete(now_s))
            }
            Phase::RadioSweep => self.radio_sweep_frame(f),
            Phase::Wait => {
                self.process_pending(now_s);
                Ok((now_s >= self.wait_until).then(|| self.begin_sweep(now_s)))
            }
            Phase::Idle | Phase::Done | Phase::Failed => {
                self.process_pending(now_s);
                Ok(None)
            }
        }
    }

    fn check(&self, f: &SpectrumFrame<'_>) -> Result<(), SweepError> {
        if f.bins_db.len() != self.fft || (f.span_hz - self.rate).abs() > 1.0 {
            return Err(SweepError::FrameShape {
                bins: f.bins_db.len(),
                span_hz: f.span_hz,
            });
        }
        Ok(())
    }

    /// Time passing, at `now_s`.
    pub fn tick(&mut self, now_s: f64) -> Option<Step> {
        self.process_pending(now_s);
        (self.phase == Phase::Wait && now_s >= self.wait_until).then(|| self.begin_sweep(now_s))
    }

    /// The radio cannot sweep by itself: the sweep retunes from now on.
    pub fn radio_sweep_failed(&mut self) -> Option<Step> {
        self.radio_sweep = RadioSweepUse::Refused;
        if self.phase == Phase::RadioSweep {
            self.stitcher.begin_sweep();
            self.stitcher
                .set_inner_keep_hz(self.profile.stitch.inner_keep_hz);
            self.pos = 0;
            return Some(self.tune_hop());
        }
        None
    }

    fn radio_sweep_frame(&mut self, f: &SpectrumFrame<'_>) -> Result<Option<Step>, SweepError> {
        let (now_s, c) = (f.time_s, f.centre_hz);
        if self.rs_centre.is_none_or(|prev| (prev - c).abs() > 1.0) {
            if let Some(prev) = self.rs_centre {
                if c < prev - 1.0 && self.stitcher.hops() > 0 {
                    // The radio wrapped: a sweep is complete.
                    self.analyse(now_s);
                    match self.scheduler.finished(now_s) {
                        Next::Now => {
                            self.stitcher.begin_sweep();
                            self.scheduler.started(now_s);
                        }
                        Next::At(t) => {
                            self.phase = Phase::Wait;
                            self.wait_until = t;
                            return Ok(Some(Step::Wait { until_s: t }));
                        }
                        Next::Finished => return Ok(Some(self.finish())),
                    }
                }
            }
            self.rs_centre = Some(c);
            self.rs_done = false;
            self.begin_capture(0);
        }
        if self.rs_done {
            return Ok(None);
        }
        self.check(f)?;
        if self.add_frame(f.bins_db) {
            self.rs_done = true;
            let (low, high) = (self.plan_cfg.low_hz, self.plan_cfg.high_hz);
            let half = 0.375 * self.rate;
            self.stitcher.add_hop(
                c,
                (c - half).max(low),
                (c + half).min(high),
                &self.pending_power,
            );
            self.hops_done += 1;
        }
        Ok(None)
    }

    /// Applies `action` to channel `id`. Locking out and removing change the hops and the hits at once.
    pub fn act(&mut self, id: u64, action: HitAction) -> Option<ActionOutcome> {
        let out = self.channels.apply(id, action)?;
        match out {
            ActionOutcome::Priority(hz) => {
                self.plan_cfg.priority_hz.push(hz);
                self.replan = true;
            }
            ActionOutcome::Removed => {
                self.tracker.remove(id);
            }
            ActionOutcome::Tune | ActionOutcome::Save | ActionOutcome::Lockout(_) => {}
        }
        Some(out)
    }

    /// Never visits `low_hz ..= high_hz` again, from the next sweep on.
    pub fn exclude(&mut self, low_hz: f64, high_hz: f64) {
        self.plan_cfg.exclusions.push(Range::new(low_hz, high_hz));
        self.tracker.remove_between(low_hz, high_hz);
        self.replan = true;
    }

    /// Loads a saved floor.
    pub fn load_floor(&mut self, snap: &FloorSnapshot) {
        self.floor.load(snap);
    }
}
