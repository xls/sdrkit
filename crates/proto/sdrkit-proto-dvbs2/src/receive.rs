//! The receive parts that the satellite and cable TV receivers share: symbol timing, the lost-stream item, the item format and
//! the status writer.

use sdrkit_algo_modem::ModemError;
use sdrkit_core::results::{LockState, Results};
use sdrkit_core::Complex32;

/// The outer decoder lost the stream: the demodulator before it searches again. It travels on a feedback edge.
#[derive(Clone, Copy, Debug)]
pub struct Lost;

impl sdrkit_graph::Item for Lost {
    const KIND: &'static str = "dvb.lost";
}

/// Writes the status of a receiver into `r` if a part of it changed, and says whether it changed. The summary memory of the
/// status is kept.
pub fn write_status(
    r: &mut Results,
    lock_state: LockState,
    snr_db: Option<f32>,
    level_dbfs: Option<f32>,
    offset_hz: Option<f64>,
    summary: &str,
) -> bool {
    r.update_status(|x| {
        let same = x.lock_state == lock_state
            && x.snr_db == snr_db
            && x.level_dbfs == level_dbfs
            && x.offset_hz == offset_hz
            && x.summary == summary;
        if !same {
            x.lock_state = lock_state;
            x.snr_db = snr_db;
            x.level_dbfs = level_dbfs;
            x.offset_hz = offset_hz;
            x.summary.clear();
            x.summary.push_str(summary);
        }
        !same
    })
}

/// The format of a stream of items of `T` made from `input`: its frequencies stay, so the transport stream and the events after
/// it carry the tuned frequency.
pub fn item_format<T: sdrkit_graph::Item>(input: &sdrkit_graph::Format) -> sdrkit_graph::Format {
    sdrkit_graph::Format {
        rate: 0.0,
        ..input.with_kind(sdrkit_graph::Kind::typed::<T>())
    }
}

/// The symbol timing of the TV receivers: the matched filter and Gardner loop of `sdrkit-algo-modem`, `TAPS` taps and 128 fractional delays.
pub type Timing<const TAPS: usize = 64> = sdrkit_algo_modem::timing::SymbolSync<
    Complex32,
    sdrkit_algo_modem::timing::Gardner,
    sdrkit_algo_modem::timing::MatchedInterp<TAPS, 128>,
>;

/// A [`Timing`] at `sps` input samples per symbol for a root raised cosine of roll-off `alpha`. The loop pulls in with noise
/// bandwidth `pull_in` and tracks with `track` (per symbol) once its lock test holds, within `max_ppm` of `sps`. With `two`, it
/// also outputs the mid-point before each symbol.
///
/// # Errors
///
/// The [`ModemError`] of the interpolator or the loop if a parameter is out of range.
pub fn timing<const TAPS: usize>(
    sps: f64,
    alpha: f64,
    pull_in: f64,
    track: f64,
    max_ppm: f64,
    two: bool,
) -> Result<Timing<TAPS>, ModemError> {
    use sdrkit_algo_dsp::pulse::Shape;
    use sdrkit_algo_modem::timing::{Gardner, MatchedInterp, SymbolSyncConfig};
    let mut cfg = SymbolSyncConfig::new(sps);
    cfg.pulse = Shape::Rc { beta: alpha };
    cfg.bn_t = track;
    cfg.gears = Some(sdrkit_algo_modem::lock::Gears {
        pull_in_bn_t: pull_in,
        track_bn_t: track,
    });
    cfg.max_ppm = max_ppm;
    cfg.two_per_symbol = two;
    let interp = MatchedInterp::new(Shape::Rrc { beta: alpha }, sps)?;
    Timing::new(Gardner, interp, cfg)
}

/// The symbol timing of a front end for high symbol rates: the matched filter of [`Timing`] read at the nearest of its 128
/// fractional delays (`sdrkit_algo_modem::timing::MatchedBank`), with the clock steered once per run of symbols
/// (`sdrkit_algo_modem::timing::RunSymbolSync`).
pub type RunTiming<const TAPS: usize = 64> = sdrkit_algo_modem::timing::RunSymbolSync<
    Complex32,
    sdrkit_algo_modem::timing::MatchedBank<TAPS, 128>,
>;

/// A [`RunTiming`] with the settings of [`timing`] and runs of `run` symbols once locked.
///
/// # Errors
///
/// The [`ModemError`] of the interpolator or the loop if a parameter is out of range.
pub fn run_timing<const TAPS: usize>(
    sps: f64,
    alpha: f64,
    pull_in: f64,
    track: f64,
    max_ppm: f64,
    run: usize,
) -> Result<RunTiming<TAPS>, ModemError> {
    use sdrkit_algo_dsp::pulse::Shape;
    use sdrkit_algo_modem::timing::{MatchedBank, SymbolSyncConfig};
    let mut cfg = SymbolSyncConfig::new(sps);
    cfg.pulse = Shape::Rc { beta: alpha };
    cfg.bn_t = track;
    cfg.gears = Some(sdrkit_algo_modem::lock::Gears {
        pull_in_bn_t: pull_in,
        track_bn_t: track,
    });
    cfg.max_ppm = max_ppm;
    let interp = MatchedBank::new(Shape::Rrc { beta: alpha }, sps)?;
    RunTiming::new(interp, cfg, run)
}
