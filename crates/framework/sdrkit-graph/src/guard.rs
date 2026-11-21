//! The input guard: samples that are not finite (NaN, infinite) become 0, and samples far beyond full scale are limited with
//! their phase kept.
//!
//! [`StackGraph`](crate::StackGraph) guards every receiver's input with [`guard`]. [`SampleGuard`] is the same as a component,
//! registered as `dsp.sample_guard`, for a stream inside a graph.

use crate::component::values_of;
use crate::component::{Applied, Component, ComponentInfo, Io, Port};
use sdrkit_core::Values;

use crate::stream::Format;
use sdrkit_core::{
    publish::{field, keys},
    Complex32, ConfigError, Published, Results, Setting, SettingError, SettingId, SettingSet,
    Value,
};

/// The default limit on a sample's magnitude: 16 times full scale. A radio never returns more; a scaling bug or a corrupt file does.
pub const DEFAULT_LIMIT: f32 = 16.0;

/// Whether `s` needs fixing at `limit`.
#[inline]
fn bad(s: Complex32, limit2: f32) -> bool {
    let p = s.re * s.re + s.im * s.im;
    // NaN (and infinity) is never "at most the limit".
    p.is_nan() || p > limit2
}

#[inline]
fn fix(s: Complex32, limit: f32) -> Complex32 {
    if !(s.re.is_finite() && s.im.is_finite()) {
        return Complex32::new(0.0, 0.0);
    }
    let m = s.norm();
    if m > limit {
        s * (limit / m)
    } else {
        s
    }
}

/// The index of the first sample of `x` beyond `limit` or not finite, if any. Reads only.
pub fn first_bad(x: &[Complex32], limit: f32) -> Option<usize> {
    let l2 = limit * limit;
    x.iter().position(|&s| bad(s, l2))
}

/// Fixes `x` in place; returns how many samples were replaced.
pub fn guard(x: &mut [Complex32], limit: f32) -> usize {
    let l2 = limit * limit;
    let mut n = 0;
    for s in x {
        if bad(*s, l2) {
            *s = fix(*s, limit);
            n += 1;
        }
    }
    n
}

/// The guard as a component: `in` (Iq) to `out` (Iq), in place. Publishes `bad_samples`.
#[derive(Debug)]
pub struct SampleGuard {
    limit: f32,
    replaced: u64,
    results: Published,
}

const LIMIT: Setting<field::F64> = Setting::new("limit", "Largest magnitude", "", DEFAULT_LIMIT as f64)
    .range(1.0, 1e6, 0.0)
    .help("Samples beyond this many times full scale are brought back to it; samples that are not numbers become 0.");

sdrkit_core::settings! {
    /// The settings of [`SampleGuard`].
    pub enum SampleGuardSetting {
        /// The largest magnitude passed.
        Limit = LIMIT,
    }
}

impl Default for SampleGuard {
    fn default() -> Self {
        SampleGuard {
            limit: DEFAULT_LIMIT,
            replaced: 0,
            results: Published::new(&[&keys::BAD_SAMPLES]),
        }
    }
}

impl SampleGuard {
    /// Samples replaced since the start.
    pub fn replaced(&self) -> u64 {
        self.replaced
    }
}

impl Component for SampleGuard {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.sample_guard",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(SampleGuardSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(SampleGuardSetting::Limit, Value::F64(f64::from(self.limit)))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let limit = self.limit;
        let n = guard(io.pass_through::<Complex32>(0, 0), limit);
        if n > 0 {
            self.replaced += n as u64;
            self.results.set(&keys::BAD_SAMPLES, self.replaced as i64);
        }
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match SampleGuardSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SampleGuardSetting::Limit => self.limit = LIMIT.read(id, value)? as f32,
        }
        Ok(Applied::Live)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_samples_are_fixed_and_counted() {
        let mut x = vec![
            Complex32::new(0.5, 0.5),
            Complex32::new(f32::NAN, 0.0),
            Complex32::new(0.0, f32::INFINITY),
            Complex32::new(300.0, 400.0),
            Complex32::new(-1.0, 0.0),
        ];
        assert_eq!(first_bad(&x, 16.0), Some(1));
        assert_eq!(guard(&mut x, 16.0), 3);
        assert_eq!(x[1], Complex32::new(0.0, 0.0));
        assert_eq!(x[2], Complex32::new(0.0, 0.0));
        assert!((x[3] - Complex32::new(9.6, 12.8)).norm() < 1e-4);
        assert_eq!(x[4], Complex32::new(-1.0, 0.0));
        assert_eq!(first_bad(&x, 16.0), None);
    }
}
