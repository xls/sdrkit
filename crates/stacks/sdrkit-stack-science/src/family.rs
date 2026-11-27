//! What the stacks of the crate share: the report period, the lock state of a stack state, the low-pass design they use, the wall
//! clock of live samples, the status line and the reading of a results field in the verify checks.

use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{Clock, LockState, Results, Status, Value};

/// The report period of every graph of the crate, seconds of signal.
pub const REPORT_S: f64 = 0.25;

/// The lock state of a stack state: 0 searching, 1 a signal, 2 or more decoding.
pub fn lock_state_of(state: u8) -> LockState {
    match state {
        0 => LockState::Searching,
        1 => LockState::Tracking,
        _ => LockState::Locked,
    }
}

/// A Kaiser-windowed (beta 7) low-pass of `n` taps with its half-amplitude point at `cutoff` cycles per sample and unit DC gain.
/// A cut-off outside (0, 0.5] is moved to the nearest end of that range.
pub fn lowpass(n: usize, cutoff: f64) -> Vec<f32> {
    design_by_taps(n.max(1), cutoff.clamp(1e-6, 0.5), Window::kaiser(7.0))
        .unwrap_or_else(|_| vec![1.0])
}

/// Whether a test generator that makes a noise floor of its own keeps it: only while the framework adds no noise (`gen_noise`
/// off or not among the settings). Otherwise noise would be added twice and an SNR setting referred to signal plus noise.
pub fn own_noise(cfg: &sdrkit_core::GenConfig) -> bool {
    use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE};
    // Settings without the generator settings have no framework noise.
    cfg.settings.schema().id(GEN_NOISE.key()).is_err() || cfg.settings.get(&GEN_NOISE) == NoiseKind::Off
}

/// The wall-clock time now (seconds since 1970, UTC) when the samples are live, else `None`. A recording has no wall time.
pub fn wall(live: bool, clock: &Clock) -> Option<f64> {
    live.then(|| clock.now())
}

/// The UTC of the first sample of a test signal that no setting places: 2026-01-01T00:00:00Z.
pub const TEST_START_UTC: f64 = 1_767_225_600.0;

/// Fills `s` with a status: the lock state, the measures and the summary that `summary` writes. It reuses the summary's memory. The
/// stacks write the summary with `write!`, which cannot fail on a `String`.
pub fn status(
    s: &mut Status,
    lock_state: LockState,
    snr_db: Option<f32>,
    level_dbfs: Option<f32>,
    offset_hz: Option<f64>,
    summary: impl FnOnce(&mut String),
) {
    s.lock_state = lock_state;
    s.snr_db = snr_db;
    s.level_dbfs = level_dbfs;
    s.offset_hz = offset_hz;
    s.summary.clear();
    summary(&mut s.summary);
}


/// The number in field `key` of `r`, or 0 when it is absent or not a number.
pub fn field(r: &Results, key: &str) -> f64 {
    let Some(v) = r.decl().field_id(key).and_then(|id| r.fields().value(id)) else {
        return 0.0;
    };
    match v {
        Value::I64(x) => *x as f64,
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => *x,
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// Replaces the points of series `id` of `r`. It reuses the memory of the points.
pub fn write_series(
    r: &mut sdrkit_core::Results,
    id: sdrkit_core::SeriesId,
    line: bool,
    points: impl IntoIterator<Item = [f32; 2]>,
) {
    r.update_series(id, |s| {
        s.line = line;
        s.points.clear();
        s.points.extend(points);
        true
    });
}
