//! What every stack of the crate shares: the report period, the status line, the hex text of bytes, the minimum-rate message,
//! the wall clock of live samples, the counts that the verify cases read and the generator of a clean scene.

use sdrkit_algo_sim::scene::Scene;
use sdrkit_core::publish::Published;
use sdrkit_core::verify::CheckLine;
use sdrkit_core::{Complex32, Generator, LockState, SignalInfo};

/// The report period of every graph of the crate, seconds of signal.
pub const REPORT_S: f64 = 0.25;

/// The bytes as upper-case hex text with no separator ("0A1B").
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 * bytes.len());
    sdrkit_core::base64::hex_into(bytes, &mut s);
    s
}

/// The bytes of hex text, or `None` if the text is not hex.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    let mut v = Vec::with_capacity(text.len() / 2);
    sdrkit_core::base64::unhex_into(text, &mut v).ok()?;
    Some(v)
}

/// A sample rate as text for a message: "1 Msps", "4.608 Msps", "400 ksps".
fn rate_text(rate_hz: f64) -> String {
    if rate_hz >= 1e6 {
        let v = (rate_hz / 1e3).round() / 1e3;
        format!("{v} Msps")
    } else {
        format!("{} ksps", (rate_hz / 1e3).round())
    }
}

/// The status line of a stack that refuses a rate below `rate_hz`: "`<name>` needs a sample rate of at least `<rate>`".
pub fn min_rate_message(name: &str, rate_hz: f64) -> String {
    format!(
        "{name} needs a sample rate of at least {}",
        rate_text(rate_hz)
    )
}

/// Sets the lock state and the summary line of the status of `p`. The other parts of the status stay.
pub fn set_status(p: &mut Published, lock_state: LockState, summary: &str) {
    p.results_mut().update_status(|s| {
        let changed = s.lock_state != lock_state || s.summary != summary;
        if changed {
            s.lock_state = lock_state;
            s.summary.clear();
            s.summary.push_str(summary);
        }
        changed
    });
}

/// A check that the count `v` of the field `key` is at least `min`.
pub(crate) fn at_least_value(key: &str, v: i64, min: i64) -> CheckLine {
    CheckLine::new(key, v >= min, format!("{v}, at least {min} expected"))
}

/// The wall-clock time now (seconds since 1970, UTC) when the samples are live, else `None`. A recording has no wall time.
pub fn wall(live: bool) -> Option<f64> {
    if !live {
        return None;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

/// A generator of a clean scene: the scene of `sdrkit-algo-sim` with the declared signal. Noise and the receiver's impairments come
/// from the standard `gen_*` settings of the framework. The station is at 0 Hz in the samples unless
/// [`CleanScene::with_station_at`] sets another offset.
pub struct CleanScene {
    scene: Scene,
    rate_hz: f64,
    info: SignalInfo,
    station_hz: f64,
}

impl CleanScene {
    /// A generator of `scene` at `rate_hz` that declares `info`.
    pub fn new(scene: Scene, rate_hz: f64, info: SignalInfo) -> CleanScene {
        CleanScene {
            scene,
            rate_hz,
            info,
            station_hz: 0.0,
        }
    }

    /// Places the station `hz` from the centre of the samples (the framework moves the signal from there to where the receiver
    /// expects it).
    pub fn with_station_at(mut self, hz: f64) -> CleanScene {
        self.station_hz = hz;
        self
    }

    /// The scene, to add sources.
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }
}

impl Generator for CleanScene {
    fn rate_hz(&self) -> f64 {
        self.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        self.station_hz
    }
}
