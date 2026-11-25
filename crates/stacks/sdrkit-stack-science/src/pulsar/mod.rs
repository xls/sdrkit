//! Pulsar folding (`pulsar`). A filter bank turns the IQ into rows of power by channel ([`bank`]); the rows are kept, and a
//! search dedisperses them incoherently over trial dispersion measures and folds them over trial periods round the user's
//! ephemeris (the shared [`sdrkit_algo_dsp::fold`]). A pulse strong enough is then timed over parts of the time and of the
//! band, which gives the period and the DM far finer than the trials. The result is the S/N, the best period and DM, the pulse
//! phase and width, the profile, and the S/N curves over the trials.
//!
//! Documents: the cold-plasma dispersion delay `dt = 4.148808e3 s x DM x (1 / f_lo^2 - 1 / f_hi^2)`, DM in pc cm⁻³ and f in
//! MHz (D. R. Lorimer, M. Kramer, "Handbook of Pulsar Astronomy"). The area estimate uses the radiometer equation, which was
//! not checked against a document.

pub mod bank;
pub mod gen;

use crate::family::{write_series, REPORT_S};
use bank::{FilterBank, Layout};
use sdrkit_algo_dsp::dispersion::{cold_plasma, COLD_PLASMA_S_MHZ2};
use sdrkit_algo_dsp::fold::{fold, linear_trials, FoldSearch};
use sdrkit_algo_dsp::peak::{line_fit, parabolic};
use sdrkit_core::command::{Command, CommandError, CommandId};
use crate::typed::{
    PulsarDetectionEvent, PulsarFields, PulsarSearchRow, PULSAR_DETECTION, PULSAR_SEARCHES,
};
use sdrkit_core::interface::Seconds;
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::verify::{CaseCommand, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile,
    LockState, Receiver, Record, ResultsDecl, Schema, Setting, SettingError, SettingId,
    SettingSet, StackConfig, StackDescriptor, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Pool, Port, RateFrom, RunContext, StackGraph, Ticket,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "pulsar";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The most values of rows kept (64 MB); the oldest rows go first.
pub const MAX_VALUES: usize = 16 * 1024 * 1024;
/// The fewest rows of a search.
const MIN_ROWS: usize = 200;
/// A search needs at least this many periods of rows.
const MIN_PERIODS: f64 = 8.0;
/// Rows that must remain after the largest shift of a trial DM.
const MARGIN_ROWS: usize = 100;
/// The rows of a normalisation group: at least this many, else one second.
const MIN_GROUP_ROWS: usize = 8;
/// A search with at least this S/N is a detection.
pub const DETECT_SNR: f64 = 7.0;
/// The first search, s observed.
pub const FIRST_SEARCH_S: f64 = 20.0;
/// The least time between searches, s.
const SEARCH_GAP_S: f64 = 15.0;
/// The time between searches as a share of the time observed.
const SEARCH_GAP_SHARE: f64 = 0.3;
/// The duty cycle of the area estimate.
pub const AREA_DUTY: f64 = 0.03;
/// The S/N of the area estimate.
pub const AREA_SNR: f64 = 5.0;
/// Polarisations of the area estimate.
const AREA_POLS: f64 = 2.0;
/// The Boltzmann constant, J/K.
const BOLTZMANN: f64 = 1.380_649e-23;
/// Searches kept in the table.
pub const MAX_SEARCHES: usize = 500;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: the filter bank (Hann transforms without overlap, a row \
the mean of whole spectra), the normalisation in groups of 1 s, the 100 rows kept after the largest shift, the pulse widths \
of 1 to 16 bins, the off-pulse share of 75 % and its variance share of 0.5, the detection at S/N 7, the search schedule \
(first at 20 s, then after max(15 s, 0.3 x the time observed)), the default trial spans and steps, and the radiometer \
equation of the area estimate. The dispersion constant 4.148808e3 s MHz² is from Lorimer and Kramer, Handbook of Pulsar \
Astronomy. The period span covers the Earth's orbital Doppler only: no acceleration search, no coherent dedispersion.";

/// The dispersion delay of `dm` (pc cm⁻³) at `f_lo_hz` behind `f_hi_hz`, s.
pub fn delay_s(dm: f64, f_lo_hz: f64, f_hi_hz: f64) -> f64 {
    let (lo, hi) = (f_lo_hz * 1e-6, f_hi_hz * 1e-6);
    COLD_PLASMA_S_MHZ2 * dm * (1.0 / (lo * lo) - 1.0 / (hi * hi))
}

/// The dispersion of the law [`cold_plasma`] (s Hz²) of `dm` (pc cm⁻³).
pub fn dispersion_of_dm(dm: f64) -> f64 {
    COLD_PLASMA_S_MHZ2 * 1e12 * dm
}

/// The effective area needed for S/N [`AREA_SNR`] at duty [`AREA_DUTY`] with two polarisations, from the radiometer
/// equation: `G = S/N T_sys / (S sqrt(n_pol B t) sqrt((1 - d) / d))`, `A = 2 k G / 1e-26`, m². `d` is held to 1e-4 to 0.5.
pub fn area_needed_m2(flux_jy: f64, bandwidth_hz: f64, tsys_k: f64, hours: f64, duty: f64) -> f64 {
    let d = duty.clamp(1e-4, 0.5);
    let g = AREA_SNR * tsys_k
        / (flux_jy * (AREA_POLS * bandwidth_hz * hours * 3600.0).sqrt() * ((1.0 - d) / d).sqrt());
    2.0 * BOLTZMANN * g / 1e-26
}

/// The pulsar period.
pub const PERIOD_S: Setting<field::F64> = Setting::new("period_s", "Pulsar period", "s", 0.5)
    .range(0.0015, 10.0, 0.0)
    .help("The period of the pulsar from your ephemeris. 0.5 s is a test value; the data file data/science/pulsars.csv lists real ones.");
/// The dispersion measure.
pub const DM: Setting<field::F64> = Setting::new("dm", "Dispersion measure", "pc cm⁻³", 20.0)
    .range(0.0, 1000.0, 0.0)
    .help("The dispersion measure of the pulsar. 20 is a test value.");
/// Channels of the filter bank.
pub const CHANNELS: Setting<field::I64> = Setting::new("channels", "Filter-bank channels", "", 64)
    .range(8.0, 1024.0, 1.0)
    .help("A power of two from 8 to 1024.")
    .section("Search")
    .advanced();
/// The time of a row.
pub const SAMPLE_MS: Setting<field::F64> = Setting::new("sample_ms", "Power sample time", "ms", 0.5)
    .range(0.1, 100.0, 0.0)
    .help("The time of a row asked; a row is a whole number of spectra, so the time used differs a little.")
    .section("Search")
    .advanced();
/// Bins of the profile.
pub const BINS: Setting<field::I64> = Setting::new("bins", "Profile bins", "", 64)
    .range(16.0, 256.0, 1.0)
    .section("Search")
    .advanced();
/// The span of the trial DMs either side of the DM.
pub const DM_SPAN: Setting<field::F64> = Setting::new("dm_span", "Trial DM span (±)", "pc cm⁻³", 4.0)
    .range(0.0, 100.0, 0.0)
    .section("Search")
    .advanced();
/// Trial DMs.
pub const DM_STEPS: Setting<field::I64> = Setting::new("dm_steps", "Trial DMs", "", 9)
    .range(1.0, 101.0, 1.0)
    .section("Search")
    .advanced();
/// The span of the trial periods either side of the period, as a share of it.
pub const PERIOD_SPAN: Setting<field::F64> =
    Setting::new("period_span", "Trial period span (± relative)", "", 2e-4)
        .range(0.0, 1e-3, 0.0)
        .help("2e-4 covers the Earth's orbital Doppler (up to 1e-4 of the period) for a period known to 1e-5.")
        .section("Search")
        .advanced();
/// Trial periods.
pub const PERIOD_STEPS: Setting<field::I64> = Setting::new("period_steps", "Trial periods", "", 21)
    .range(1.0, 201.0, 1.0)
    .section("Search")
    .advanced();
/// The flux of the area estimate.
pub const FLUX_JY: Setting<field::F64> = Setting::new("flux_jy", "Flux of the pulsar", "Jy", 1.0)
    .range(0.01, 10_000.0, 0.0)
    .help("For the area estimate only.")
    .section("Area estimate")
    .advanced();
/// The system temperature of the area estimate.
pub const TSYS_K: Setting<field::F64> = Setting::new("tsys_k", "System temperature", "K", 150.0)
    .range(10.0, 100_000.0, 0.0)
    .help("For the area estimate only.")
    .section("Area estimate")
    .advanced();
/// The observing time of the area estimate.
pub const HOURS: Setting<field::F64> = Setting::new("hours", "Observing time", "h", 1.0)
    .range(0.05, 10_000.0, 0.0)
    .help("For the area estimate only.")
    .section("Area estimate")
    .advanced();

sdrkit_core::settings! {
    /// The settings of [`PulsarDecoder`].
    pub enum SearchSetting {
        /// The period.
        PeriodS = PERIOD_S,
        /// The DM.
        Dm = DM,
        /// The channels.
        Channels = CHANNELS,
        /// The time of a row.
        SampleMs = SAMPLE_MS,
        /// The bins.
        Bins = BINS,
        /// The DM span.
        DmSpan = DM_SPAN,
        /// The trial DMs.
        DmSteps = DM_STEPS,
        /// The period span.
        PeriodSpan = PERIOD_SPAN,
        /// The trial periods.
        PeriodSteps = PERIOD_STEPS,
        /// The flux.
        FluxJy = FLUX_JY,
        /// The system temperature.
        TsysK = TSYS_K,
        /// The observing time.
        Hours = HOURS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `pulsar` stack.
    pub enum PulsarSetting {
        /// The period.
        PeriodS = PERIOD_S,
        /// The DM.
        Dm = DM,
        /// The channels.
        Channels = CHANNELS,
        /// The time of a row.
        SampleMs = SAMPLE_MS,
        /// The bins.
        Bins = BINS,
        /// The DM span.
        DmSpan = DM_SPAN,
        /// The trial DMs.
        DmSteps = DM_STEPS,
        /// The period span.
        PeriodSpan = PERIOD_SPAN,
        /// The trial periods.
        PeriodSteps = PERIOD_STEPS,
        /// The flux.
        FluxJy = FLUX_JY,
        /// The system temperature.
        TsysK = TSYS_K,
        /// The observing time.
        Hours = HOURS,
        /// The S/N of the test signal.
        GenSnr = gen::GEN_SNR,
        /// The period of the test signal.
        GenPeriodS = gen::GEN_PERIOD_S,
        /// The DM of the test signal.
        GenDm = gen::GEN_DM,
    }
}

/// The target and the trials of a search.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    /// The period, s.
    pub period_s: f64,
    /// The DM, pc cm⁻³.
    pub dm: f64,
    /// Trial DMs either side, pc cm⁻³.
    pub dm_span: f64,
    /// Trial DMs.
    pub dm_steps: usize,
    /// Trial periods either side, a share of the period.
    pub period_span: f64,
    /// Trial periods.
    pub period_steps: usize,
    /// Profile bins.
    pub bins: usize,
}

impl Default for Target {
    fn default() -> Self {
        Target {
            period_s: PERIOD_S.default,
            dm: DM.default,
            dm_span: DM_SPAN.default,
            dm_steps: DM_STEPS.default as usize,
            period_span: PERIOD_SPAN.default,
            period_steps: PERIOD_STEPS.default as usize,
            bins: BINS.default as usize,
        }
    }
}

/// The result of a search.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The time observed when the search started, s.
    pub observed_s: f64,
    /// The rows searched.
    pub rows: usize,
    /// A detection: the best S/N is [`DETECT_SNR`] or more.
    pub ok: bool,
    /// The best S/N; `None` when no trial gave a score.
    pub snr: Option<f64>,
    /// The best period, s.
    pub period_s: f64,
    /// The best DM, pc cm⁻³.
    pub dm: f64,
    /// The phase of the middle of the pulse, 0 to 1.
    pub peak_phase: f64,
    /// The pulse width as a share of the period.
    pub duty: f64,
    /// The best profile in units of the off-pulse deviation, mean removed, the peak at the middle bin.
    pub profile: Vec<f64>,
    /// The best S/N of each trial DM: (DM, S/N).
    pub dm_curve: Vec<[f64; 2]>,
    /// The best S/N of each trial period: (period s, S/N).
    pub period_curve: Vec<[f64; 2]>,
}

/// The fewest rows of a search at `period_s` with rows of `row_s`: max(200, 8 periods).
pub fn min_rows(period_s: f64, row_s: f64) -> usize {
    MIN_ROWS.max((MIN_PERIODS * period_s / row_s).ceil() as usize)
}

/// Searches the rows `cells` (row-major, `layout.channels` a row, channel `c` at `freqs_hz[c]`) for `target`. `None` when
/// there are fewer rows than [`min_rows`] or the search cannot be made.
pub fn search(cells: &[f32], layout: &Layout, freqs_hz: &[f64], target: &Target, observed_s: f64) -> Option<Outcome> {
    let rows = cells.len() / layout.channels;
    if rows < min_rows(target.period_s, layout.row_s) {
        return None;
    }
    let group = MIN_GROUP_ROWS.max((1.0 / layout.row_s).floor() as usize);
    let s = FoldSearch::new(
        layout.channels,
        layout.row_s,
        freqs_hz,
        cold_plasma,
        group,
        MARGIN_ROWS,
        target.bins,
    )
    .ok()?;
    let dms = linear_trials(target.dm, target.dm_span, target.dm_steps);
    let periods = linear_trials(
        target.period_s,
        target.period_s * target.period_span,
        target.period_steps,
    );
    let ds: Vec<f64> = dms.iter().map(|&d| dispersion_of_dm(d)).collect();
    let f = s.run(cells, &ds, &periods);
    let mut o = Outcome {
        observed_s,
        rows: f.rows,
        dm_curve: dms
            .iter()
            .zip(&f.dispersion_curve)
            .filter_map(|(&d, v)| v.map(|v| [d, v]))
            .collect(),
        period_curve: periods
            .iter()
            .zip(&f.period_curve)
            .filter_map(|(&p, v)| v.map(|v| [p, v]))
            .collect(),
        ..Outcome::default()
    };
    if let Some(b) = f.best {
        o.snr = Some(b.pulse.snr);
        o.ok = b.pulse.snr >= DETECT_SNR;
        o.period_s = periods[b.period];
        o.dm = dms[b.dispersion];
        let fine = Refine {
            search: &s,
            z: s.normalised(&cells[..rows * layout.channels]),
            delays: channel_delays(freqs_hz),
            row_s: layout.row_s,
            bins: target.bins,
            parts: REFINE_PARTS.min((b.pulse.snr / REFINE_PART_SNR).powi(2) as usize),
        };
        if let Some((period_s, dm)) = fine.run(o.period_s, o.dm, &periods, &dms) {
            o.period_s = period_s;
            o.dm = dm;
        }
        o.peak_phase = b.pulse.peak / target.bins as f64;
        o.duty = b.pulse.duty;
        o.profile = b.profile;
    }
    Some(o)
}

/// The most parts (of the time, of the band) that the refinement of a search times against each other.
const REFINE_PARTS: usize = 16;
/// The S/N a part must have for its pulse to be timed: the parts are as many as leave each this share of the S/N found.
const REFINE_PART_SNR: f64 = 6.0;
/// The fewest parts for a line through their times.
const REFINE_MIN_PARTS: usize = 3;
/// The most bins of the profiles of the refinement.
const REFINE_BINS: usize = 2048;
/// The fewest periods in a part of the time.
const REFINE_MIN_PERIODS: f64 = 4.0;

/// The delay of each channel behind the channel of least delay for a dispersion of 1 (the law [`cold_plasma`]), s.
fn channel_delays(freqs_hz: &[f64]) -> Vec<f64> {
    let least = freqs_hz.iter().map(|&f| cold_plasma(f)).fold(f64::INFINITY, f64::min);
    freqs_hz.iter().map(|&f| cold_plasma(f) - least).collect()
}

/// How far the pulse of `profile` lies behind the pulse of `template` (the same bins), turns from -0.5 to 0.5: the peak of
/// their circular correlation, means removed, by a parabola through the top and the two points either side where the
/// correlation has fallen to half (the flanks carry the timing; next to the top the noise decides).
fn lag_turns(profile: &[f64], template: &[f64]) -> f64 {
    let b = profile.len();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / b as f64;
    let (mp, mt) = (mean(profile), mean(template));
    let at = |k: usize| -> f64 {
        (0..b).map(|j| (profile[(j + k) % b] - mp) * (template[j] - mt)).sum()
    };
    let c: Vec<f64> = (0..b).map(at).collect();
    let top = (0..b).max_by(|&i, &j| c[i].total_cmp(&c[j])).unwrap_or(0);
    let side = |h: usize| (c[(top + b - h) % b], c[(top + h) % b]);
    let h = (1..b / 4)
        .find(|&h| side(h).0.min(side(h).1) < 0.5 * c[top])
        .unwrap_or(1);
    let fit = parabolic(side(h).0, c[top], side(h).1);
    let turns = (top as f64 + fit.offset * h as f64) / b as f64;
    turns - (turns + 0.5).floor()
}

/// The refinement of a search: the period from the drift of the pulse over parts of the time, the DM from the delay of the
/// pulse over parts of the band. Both resolve far below one bin of the profile, which bounds what the S/N of the trials tells.
struct Refine<'a> {
    search: &'a FoldSearch,
    /// The normalised rows, channel by channel.
    z: Vec<f32>,
    /// [`channel_delays`].
    delays: Vec<f64>,
    row_s: f64,
    /// The bins of the profile of the search.
    bins: usize,
    /// The parts to time.
    parts: usize,
}

impl Refine<'_> {
    /// The period and the DM round the best trial (`period_s`, `dm`). `None` when the pulse is too weak to time
    /// [`REFINE_MIN_PARTS`] parts, or a result lies more than one trial step outside the trials `periods` or `dms`.
    fn run(&self, period_s: f64, dm: f64, periods: &[f64], dms: &[f64]) -> Option<(f64, f64)> {
        let d0 = dispersion_of_dm(dm);
        let shifts = self.search.shifts(d0);
        let series = self.search.dedispersed(&self.z, &shifts);
        if series.is_empty() {
            return None;
        }
        let bins = REFINE_BINS.min((period_s / self.row_s) as usize).max(self.bins);
        // Twice: the second pass times parts that the first period has sharpened.
        let mut p = period_s;
        for _ in 0..2 {
            p = self.period(&series, p, bins)?;
        }
        let d = d0 + self.dispersion_error(series.len(), &shifts, d0, p, bins)?;
        let dm = d / dispersion_of_dm(1.0);
        (within_trials(p, periods) && within_trials(dm, dms)).then_some((p, dm))
    }

    /// The period from the dedispersed `series` folded at `period_s`: the phase of the pulse in each part of the time, a line
    /// through the phases over time, and `P = P0 / (1 - P0 x slope)`.
    fn period(&self, series: &[f32], period_s: f64, bins: usize) -> Option<f64> {
        let rows_in_part = (REFINE_MIN_PERIODS * period_s / self.row_s).ceil() as usize;
        let parts = self.parts.min(series.len() / rows_in_part.max(1));
        if parts < REFINE_MIN_PARTS {
            return None;
        }
        let len = series.len() / parts;
        // A part starts on a whole bin of the fold, so that the profiles of the parts share their bins.
        let first = |k: usize| {
            let turns = (k * len) as f64 * self.row_s / period_s;
            ((turns * bins as f64).ceil() / bins as f64 * period_s / self.row_s).round() as usize
        };
        let mut profiles = vec![0.0; parts * bins];
        let mut times = Vec::with_capacity(parts);
        for (k, profile) in profiles.chunks_mut(bins).enumerate() {
            let (a, b) = (first(k), first(k + 1).min(series.len()));
            fold(&series[a..b], self.row_s, period_s, profile);
            // The fold of a part counts its phase from the part's first row.
            let turns = (a as f64 * self.row_s / period_s * bins as f64).round() as usize % bins;
            profile.rotate_right(turns);
            times.push(0.5 * (a + b) as f64 * self.row_s);
        }
        let slope = line_fit(&times, &lags(&profiles, bins), None).ok()?.slope;
        let p = period_s / (1.0 - period_s * slope);
        (p > 0.0 && p.is_finite()).then_some(p)
    }

    /// The error of the dispersion `d0` that the rows were dedispersed with (at `shifts`, a series of `rows` rows): the delay
    /// of the pulse in each part of the band, less what the rounding of the shifts to whole rows left, and a line through the
    /// delays over the parts' mean delay for a dispersion of 1.
    fn dispersion_error(&self, rows: usize, shifts: &[usize], d0: f64, period_s: f64, bins: usize) -> Option<f64> {
        let channels = shifts.len();
        let parts = self.parts.min(channels);
        if parts < REFINE_MIN_PARTS {
            return None;
        }
        let all = self.z.len() / channels;
        let mut profiles = vec![0.0; parts * bins];
        let mut band = vec![0f32; rows];
        let (mut per_unit, mut left) = (Vec::with_capacity(parts), Vec::with_capacity(parts));
        for (k, profile) in profiles.chunks_mut(bins).enumerate() {
            let cs = k * channels / parts..(k + 1) * channels / parts;
            band.fill(0.0);
            let (mut unit, mut rest) = (0.0, 0.0);
            for c in cs.clone() {
                let col = &self.z[c * all + shifts[c]..c * all + shifts[c] + rows];
                for (a, &v) in band.iter_mut().zip(col) {
                    *a += v;
                }
                unit += self.delays[c];
                rest += d0 * self.delays[c] - shifts[c] as f64 * self.row_s;
            }
            fold(&band, self.row_s, period_s, profile);
            per_unit.push(unit / cs.len() as f64);
            left.push(rest / cs.len() as f64);
        }
        let late: Vec<f64> = lags(&profiles, bins)
            .iter()
            .zip(&left)
            .map(|(turns, rest)| turns * period_s - rest)
            .collect();
        Some(line_fit(&per_unit, &late, None).ok()?.slope)
    }
}

/// Whether `v` lies within the rising trials `trials` or up to one step outside them.
fn within_trials(v: f64, trials: &[f64]) -> bool {
    match trials {
        [first, .., last] => {
            let step = (last - first) / (trials.len() - 1) as f64;
            (first - step..=last + step).contains(&v)
        }
        _ => false,
    }
}

/// How far the pulse of each profile of `profiles` (`bins` values each) lies behind the mean place of the pulses, turns: each
/// profile against the sum of the others ([`lag_turns`]; its own noise in the template would hold the peak at no lag), times
/// `(n - 1) / n` because the sum of the others lies ahead of the mean of all by a share `1 / (n - 1)` of the lag.
fn lags(profiles: &[f64], bins: usize) -> Vec<f64> {
    let n = profiles.len() / bins;
    let mut sum = vec![0.0; bins];
    for p in profiles.chunks(bins) {
        sum.iter_mut().zip(p).for_each(|(s, v)| *s += v);
    }
    let mut others = vec![0.0; bins];
    profiles
        .chunks(bins)
        .map(|p| {
            others.iter_mut().zip(sum.iter().zip(p)).for_each(|(o, (s, v))| *o = s - v);
            lag_turns(p, &others) * (n as f64 - 1.0) / n as f64
        })
        .collect()
}

/// The profile of the best trial.
pub const PROFILE: SeriesDef = SeriesDef {
    key: "profile",
    label: "Profile",
    x_unit: "phase",
    y_unit: "σ",
};
/// The best S/N of each trial DM.
pub const DM_CURVE: SeriesDef = SeriesDef {
    key: "dm_curve",
    label: "S/N over DM",
    x_unit: "pc cm⁻³",
    y_unit: "",
};
/// The best S/N of each trial period.
pub const PERIOD_CURVE: SeriesDef = SeriesDef {
    key: "period_curve",
    label: "S/N over period",
    x_unit: "s",
    y_unit: "",
};

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The `search_now` command.
pub const SEARCH_NOW: Command = Command::new("search_now", "Search now")
    .help("Runs a search on the rows held now.");
static COMMANDS: [&Command; 1] = [&SEARCH_NOW];

/// The `pulsar` decoder: the filter bank, the rows held, the search schedule and the results.
pub struct PulsarDecoder {
    target: Target,
    channels_set: usize,
    sample_ms: f64,
    flux_jy: f64,
    tsys_k: f64,
    hours: f64,
    rate: f64,
    centre_hz: f64,
    bank: Option<FilterBank>,
    freqs: Arc<Vec<f64>>,
    held: VecDeque<f32>,
    rows_total: u64,
    next_search_row: u64,
    searched_rows: u64,
    origin_s: f64,
    input: u64,
    live: bool,
    pool: Option<Pool>,
    job: Option<Ticket<Option<Outcome>>>,
    job_due: bool,
    last: Option<Outcome>,
    searches: u64,
    detections: u64,
    table: VecDeque<(u64, f64, Outcome)>,
    serial: u64,
    fresh: usize,
    cleared: bool,
    series_changed: bool,
    events: Vec<Event>,
    made: Vec<f32>,
    started: bool,
    status: Status,
}

impl Default for PulsarDecoder {
    fn default() -> Self {
        PulsarDecoder {
            target: Target::default(),
            channels_set: CHANNELS.default as usize,
            sample_ms: SAMPLE_MS.default,
            flux_jy: FLUX_JY.default,
            tsys_k: TSYS_K.default,
            hours: HOURS.default,
            rate: 0.0,
            centre_hz: 0.0,
            bank: None,
            freqs: Arc::new(Vec::new()),
            held: VecDeque::new(),
            rows_total: 0,
            next_search_row: 0,
            searched_rows: 0,
            origin_s: 0.0,
            input: 0,
            live: false,
            pool: None,
            job: None,
            job_due: false,
            last: None,
            searches: 0,
            detections: 0,
            table: VecDeque::new(),
            serial: 0,
            fresh: 0,
            cleared: false,
            series_changed: false,
            events: Vec::new(),
            made: Vec::new(),
            started: false,
            status: Status::default(),
        }
    }
}

impl PulsarDecoder {
    /// The layout of the rows, when configured.
    pub fn layout(&self) -> Option<Layout> {
        self.bank.as_ref().map(|b| *b.layout())
    }

    /// The rows held, row-major, oldest first.
    pub fn rows_held(&self) -> Vec<f32> {
        self.held.iter().copied().collect()
    }

    /// Rows made since the rows were last dropped.
    pub fn rows_total(&self) -> u64 {
        self.rows_total
    }

    /// The time observed, s.
    fn observed_s(&self) -> f64 {
        self.layout()
            .map_or(0.0, |l| self.rows_total as f64 * l.row_s)
    }

    /// The row at which a search at `observed_s` is next due.
    fn due_row(&self, observed_s: f64) -> u64 {
        let Some(l) = self.layout() else {
            return u64::MAX;
        };
        (observed_s / l.row_s - 1e-9).ceil().max(1.0) as u64
    }

    /// Drops the rows and makes the filter bank again from the next sample.
    fn reset_rows(&mut self) {
        self.held.clear();
        self.rows_total = 0;
        self.searched_rows = 0;
        self.job = None;
        self.job_due = false;
        self.origin_s = self.input as f64 / self.rate.max(1.0);
        if self.rate > 0.0 {
            let layout = Layout::new(self.rate, self.channels_set, self.sample_ms);
            self.freqs = Arc::new(layout.freqs(self.centre_hz, self.rate));
            self.bank = Some(FilterBank::new(layout));
        }
        self.next_search_row = self.due_row(FIRST_SEARCH_S);
    }

    /// Starts a search of the rows held: on a helper thread when live, else in line (its result is then taken at once).
    fn start_search(&mut self) {
        let Some(layout) = self.layout() else {
            return;
        };
        if self.job.is_some() {
            self.job_due = true;
            return;
        }
        self.job_due = false;
        self.searched_rows = self.rows_total;
        let observed = self.observed_s();
        let after = observed + SEARCH_GAP_S.max(SEARCH_GAP_SHARE * observed);
        self.next_search_row = self.due_row(after);
        let cells = self.rows_held();
        let freqs = self.freqs.clone();
        let target = self.target.clone();
        match (&self.pool, self.live) {
            (Some(pool), true) => {
                self.job = Some(pool.spawn(move || search(&cells, &layout, &freqs, &target, observed)));
            }
            _ => {
                let o = search(&cells, &layout, &freqs, &target, observed);
                self.take_outcome(o);
            }
        }
    }

    /// Picks up the result of a search on a helper thread, if it has finished.
    fn poll_job(&mut self) {
        let Some(t) = self.job.as_mut() else {
            return;
        };
        let Some(r) = t.try_take() else {
            return;
        };
        self.job = None;
        self.take_outcome(r.ok().flatten());
        if self.job_due {
            self.start_search();
        }
    }

    /// Records the result of a search: the counters, the table, the series and, for a detection, the event.
    fn take_outcome(&mut self, o: Option<Outcome>) {
        let Some(o) = o else {
            return;
        };
        self.searches += 1;
        if o.ok {
            self.detections += 1;
            let text = format!(
                "S/N {:.1}, period {:.6} s, DM {:.2}",
                o.snr.unwrap_or(0.0),
                o.period_s,
                o.dm
            );
            self.events.push(PULSAR_DETECTION.event(
                self.origin_s + o.observed_s,
                self.centre_hz,
                &PulsarDetectionEvent {
                    text: &text,
                    ok: true,
                    snr: o.snr.unwrap_or(0.0),
                    period_found_s: o.period_s,
                    dm_found: o.dm,
                    peak_phase: o.peak_phase,
                    duty: o.duty,
                    observed_s: Seconds(o.observed_s),
                },
            ));
        }
        self.table.push_back((self.serial, self.origin_s + o.observed_s, o.clone()));
        self.serial += 1;
        self.fresh = (self.fresh + 1).min(MAX_SEARCHES);
        while self.table.len() > MAX_SEARCHES {
            self.table.pop_front();
        }
        self.last = Some(o);
        self.series_changed = true;
    }

    /// Adds a row to the rows held, dropping the oldest beyond [`MAX_VALUES`].
    fn keep_row(held: &mut VecDeque<f32>, row: &[f32]) {
        while held.len() + row.len() > MAX_VALUES && !held.is_empty() {
            held.drain(..row.len().min(held.len()));
        }
        held.extend(row.iter().copied());
    }

    fn emit(&mut self, out: &mut Outbox) {
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }

    /// The text of the status line.
    fn summary(&self) -> String {
        if !self.started {
            return "Pulsar: waiting for samples".into();
        }
        let observed = self.observed_s();
        match &self.last {
            Some(o) if o.ok => format!(
                "Pulsar detected: S/N {:.1}, period {:.6} s, DM {:.2} ({observed:.0} s observed)",
                o.snr.unwrap_or(0.0),
                o.period_s,
                o.dm
            ),
            Some(o) => format!(
                "Pulsar not detected: best S/N {:.1} ({observed:.0} s observed)",
                o.snr.unwrap_or(0.0)
            ),
            None => format!("Pulsar: {observed:.1} s observed, first search at {FIRST_SEARCH_S:.0} s"),
        }
    }
}

impl Decoder for PulsarDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.pulsar"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        SearchSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = SearchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        let count = |v: i64| usize::try_from(v).unwrap_or(1);
        let mut reset = false;
        match s {
            SearchSetting::PeriodS => {
                let v = PERIOD_S.read(id, value)?;
                reset = v != self.target.period_s;
                self.target.period_s = v;
            }
            SearchSetting::Dm => {
                let v = DM.read(id, value)?;
                reset = v != self.target.dm;
                self.target.dm = v;
            }
            SearchSetting::Channels => {
                let v = count(CHANNELS.read(id, value)?);
                if !v.is_power_of_two() {
                    return Err(SettingError::NotPermitted { id });
                }
                reset = v != self.channels_set;
                self.channels_set = v;
            }
            SearchSetting::SampleMs => {
                let v = SAMPLE_MS.read(id, value)?;
                reset = v != self.sample_ms;
                self.sample_ms = v;
            }
            SearchSetting::Bins => self.target.bins = count(BINS.read(id, value)?),
            SearchSetting::DmSpan => self.target.dm_span = DM_SPAN.read(id, value)?,
            SearchSetting::DmSteps => self.target.dm_steps = count(DM_STEPS.read(id, value)?),
            SearchSetting::PeriodSpan => self.target.period_span = PERIOD_SPAN.read(id, value)?,
            SearchSetting::PeriodSteps => {
                self.target.period_steps = count(PERIOD_STEPS.read(id, value)?);
            }
            SearchSetting::FluxJy => self.flux_jy = FLUX_JY.read(id, value)?,
            SearchSetting::TsysK => self.tsys_k = TSYS_K.read(id, value)?,
            SearchSetting::Hours => self.hours = HOURS.read(id, value)?,
        }
        if reset && self.bank.is_some() {
            self.reset_rows();
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let t = &self.target;
        let int = |v: usize| Value::I64(i64::try_from(v).unwrap_or(i64::MAX));
        values_of([
            (SearchSetting::PeriodS, Value::F64(t.period_s)),
            (SearchSetting::Dm, Value::F64(t.dm)),
            (SearchSetting::Channels, int(self.channels_set)),
            (SearchSetting::SampleMs, Value::F64(self.sample_ms)),
            (SearchSetting::Bins, int(t.bins)),
            (SearchSetting::DmSpan, Value::F64(t.dm_span)),
            (SearchSetting::DmSteps, int(t.dm_steps)),
            (SearchSetting::PeriodSpan, Value::F64(t.period_span)),
            (SearchSetting::PeriodSteps, int(t.period_steps)),
            (SearchSetting::FluxJy, Value::F64(self.flux_jy)),
            (SearchSetting::TsysK, Value::F64(self.tsys_k)),
            (SearchSetting::Hours, Value::F64(self.hours)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.start_search();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.pool = Some(ctx.pool.clone());
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre_hz = if input.centre_hz > 0.0 {
            input.centre_hz
        } else {
            input.tuned_hz
        };
        self.input = 0;
        self.reset_rows();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        let centre = if input.centre_hz > 0.0 {
            input.centre_hz
        } else {
            input.tuned_hz
        };
        if centre != self.centre_hz {
            self.centre_hz = centre;
            self.reset_rows();
        }
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        self.poll_job();
        let Some(bank) = self.bank.as_mut() else {
            return;
        };
        let mut made = std::mem::take(&mut self.made);
        made.clear();
        bank.push(x, |row| made.extend_from_slice(row));
        let channels = bank.layout().channels;
        // Each row in turn, so that a search sees the same rows whatever the size of the pieces.
        for row in made.chunks_exact(channels) {
            Self::keep_row(&mut self.held, row);
            self.rows_total += 1;
            if self.rows_total == self.next_search_row {
                self.start_search();
            }
        }
        self.made = made;
        self.emit(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        self.poll_job();
        if self.job.is_none() && self.rows_total > self.searched_rows {
            if let Some(l) = self.layout() {
                if self.held.len() / l.channels >= min_rows(self.target.period_s, l.row_s) {
                    let live = std::mem::replace(&mut self.live, false);
                    self.start_search();
                    self.live = live;
                }
            }
        }
        self.emit(out);
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&PULSAR_SEARCHES);
        }
        let start = self.table.len().saturating_sub(self.fresh);
        for (serial, time_s, o) in self.table.iter().skip(start) {
            PULSAR_SEARCHES.put(
                p,
                &format!("{serial:05}"),
                &PulsarSearchRow {
                    observed_s: Seconds(o.observed_s),
                    ok: o.ok,
                    snr: o.snr,
                    period_found_s: o.period_s,
                    dm_found: o.dm,
                    peak_phase: o.peak_phase,
                    duty: o.duty,
                    time_s: *time_s,
                },
            );
        }
        self.fresh = 0;
        if std::mem::take(&mut self.series_changed) {
            let r = p.results_mut();
            let (profile, dm_curve, period_curve) = match &self.last {
                Some(o) => {
                    let b = o.profile.len().max(1) as f64;
                    (
                        o.profile
                            .iter()
                            .enumerate()
                            .map(|(i, v)| [(i as f64 / b) as f32, *v as f32])
                            .collect(),
                        o.dm_curve.iter().map(|v| [v[0] as f32, v[1] as f32]).collect(),
                        o.period_curve.iter().map(|v| [v[0] as f32, v[1] as f32]).collect(),
                    )
                }
                None => (Vec::new(), Vec::new(), Vec::new()),
            };
            for (key, points) in [
                (PROFILE.key, profile),
                (DM_CURVE.key, dm_curve),
                (PERIOD_CURVE.key, period_curve),
            ] {
                if let Some(id) = r.decl().series_id(key) {
                    write_series(r, id, true, points);
                }
            }
        }
        let text = self.summary();
        let observed_s = self.observed_s();
        let layout = self.layout();
        let channels = layout.map_or(1, |l| l.channels);
        let o = self.last.as_ref();
        let found = o.filter(|o| o.snr.is_some());
        let st = &mut self.status;
        st.lock_state = match (self.started, &self.last) {
            (false, _) => LockState::Off,
            (_, Some(o)) if o.ok => LockState::Locked,
            (_, Some(_)) => LockState::Tracking,
            (_, None) => LockState::Searching,
        };
        st.snr_db = self
            .last
            .as_ref()
            .and_then(|o| o.snr)
            .filter(|s| *s > 0.0)
            .map(|s| (10.0 * s.log10()) as f32);
        st.summary.clear();
        st.summary.push_str(&text);
        p.put(&PulsarFields {
            text: &st.summary,
            ok: o.is_some_and(|o| o.ok),
            snr: o.and_then(|o| o.snr),
            period_found_s: found.map(|o| o.period_s),
            dm_found: found.map(|o| o.dm),
            peak_phase: found.map(|o| o.peak_phase),
            duty: found.map(|o| o.duty),
            observed_s: Seconds(observed_s),
            searching: self.job.is_some(),
            searches: i64::try_from(self.searches).unwrap_or(i64::MAX),
            detections: i64::try_from(self.detections).unwrap_or(i64::MAX),
            rows_held: i64::try_from(self.held.len() / channels).unwrap_or(i64::MAX),
            row_ms: layout.map(|l| l.row_s * 1e3),
            bank_channels: layout.map(|l| l.channels as i64),
            area_m2: (self.rate > 0.0).then(|| {
                area_needed_m2(self.flux_jy, self.rate, self.tsys_k, self.hours, AREA_DUTY)
            }),
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.table.clear();
        self.fresh = 0;
        self.cleared = true;
        self.searches = 0;
        self.detections = 0;
        self.events.clear();
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("dec", DecoderComponent::new(PulsarDecoder::default()))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut b = StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "Pulsar folding needs a sample rate of at least 1 Msps");
    for s in SearchSetting::SETTINGS {
        let key = s.key();
        let to = format!("dec.{key}");
        b = b.route(key, &[to.as_str()])?;
    }
    b.command("clear", CommandTarget::every("clear"))
        .command(SEARCH_NOW.key, CommandTarget::of("dec", SEARCH_NOW.key))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("science.pulsar", || {
        Box::new(DecoderComponent::new(PulsarDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::PulsarGenerator::from_config(cfg)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 0.5 s and DM 20, searched after 7.5 s", o, 0.0);
    c.seconds = o.seconds.max(8.0);
    // The first scheduled search is at 20 s: the case asks for one sooner. A run that does not send commands searches at the
    // end of the input.
    c.commands = vec![CaseCommand::new(7.5, &SEARCH_NOW)];
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let ok = crate::family::field(r, PulsarFields::OK.key);
        let snr = crate::family::field(r, PulsarFields::SNR.key);
        let p = crate::family::field(r, PulsarFields::PERIOD_FOUND_S.key);
        vec![
            CheckLine::new(PulsarFields::OK.key, ok >= 1.0, format!("{ok}, 1 expected")),
            CheckLine::new(PulsarFields::SNR.key, snr >= DETECT_SNR, format!("{snr:.1}, at least 7 expected")),
            CheckLine::new(
                PulsarFields::PERIOD_FOUND_S.key,
                (p - 0.5).abs() <= 0.5 * 2e-4,
                format!("{p:.6} s, 0.5 s expected"),
            ),
        ]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// Pulsar folding.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Pulsar folding",
    group: "Science, time and utility",
    blurb: "A filter bank, incoherent dedispersion over trial DMs and folding over trial periods round your ephemeris: the \
            S/N, the period and DM found, the pulse profile and the S/N curves. Needs a dish or long Yagis and a low-noise \
            amplifier.",
    tuning: Tuning {
        min_hz: 100e6,
        max_hz: 6000e6,
        default_hz: 408e6,
        rate_hz: 2.4e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 2.4e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: PulsarSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: None,
};
