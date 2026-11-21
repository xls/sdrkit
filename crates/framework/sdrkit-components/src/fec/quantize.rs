//! `fec.quantize`: `f32` soft bits to `i8` LLRs with the shared [`Quantizer`].

use sdrkit_algo_fec::Quantizer;
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Field, Published};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_core::{SettingId, SettingSet};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Port};

/// The scale in use (`i8` units per natural-log unit).
const LLR_SCALE: Field<sdrkit_core::publish::field::F64> = Field::new("llr_scale", "LLR scale", "");

const SCALE: Setting<field::F64> = Setting::new("scale", "Scale", "", 0.0)
    .range(0.0, 1000.0, 0.0)
    .help("i8 units per unit of natural-log LLR; 0 adapts the scale to the signal.");
const TARGET: Setting<field::F64> = Setting::new("target", "Target magnitude", "", 20.0)
    .range(1.0, 127.0, 0.0)
    .help("With an adaptive scale: the mean magnitude of the output values.");
const LIMIT: Setting<field::I64> =
    Setting::new("limit", "Largest magnitude", "", 127).range(1.0, 127.0, 1.0);
const WINDOW: Setting<field::I64> = Setting::new("window", "Window", "", 1024)
    .range(16.0, 1_048_576.0, 1.0)
    .help("With an adaptive scale: values per scale estimate.");
const NEGATE: Setting<field::Bool> = Setting::new("negate", "Negate", "", false)
    .help("Flip every sign (for soft bits that are positive for 1).");

sdrkit_core::settings! {
    /// The settings of [`Quantize`].
    pub enum QuantizeSetting {
        /// Scale.
        Scale = SCALE,
        /// Target magnitude.
        Target = TARGET,
        /// Largest magnitude.
        Limit = LIMIT,
        /// Window.
        Window = WINDOW,
        /// Negate.
        Negate = NEGATE,
    }
}

/// Quantizes a `Real` stream of soft bits (`f32` LLRs, positive means 0) into an `Llr` stream (`i8`, saturated at `limit`).
///
/// - With `scale` above 0 every value is multiplied by it: the output format's `llr_scale` is that scale, so later decoders know
///   what an `i8` unit means. Use this when the demapper produces true LLRs.
/// - With `scale` 0 the scale adapts: the stream is cut into windows of `window` values and each window is quantized with the scale
///   that brings its mean magnitude to `target` ([`Quantizer::quantize_auto`]). The output then lags by up to one window, and its
///   format's `llr_scale` is `None` (the scale changes); the scale of the last window is published as `llr_scale`.
///
/// `negate` flips every sign (for a source whose soft bits are positive for 1). A value that is not a number becomes an erasure
/// (0) and is counted in `bad_samples`.
#[derive(Debug)]
pub struct Quantize {
    q: Quantizer,
    scale: f64,
    window: usize,
    negate: bool,
    buf: Vec<f32>,
    last_scale: f32,
    bad: u64,
    every: Every,
    published: Published,
}

impl Default for Quantize {
    fn default() -> Self {
        Quantize {
            q: Quantizer::default(),
            scale: SCALE.default,
            window: WINDOW.default as _,
            negate: NEGATE.default,
            buf: Vec::new(),
            last_scale: 0.0,
            bad: 0,
            every: Every::default(),
            published: Published::new(&[&LLR_SCALE, &keys::BAD_SAMPLES]),
        }
    }
}

impl Quantize {
    fn emit(&mut self, x: &[f32], scale: f32, out: &mut Vec<i8>) {
        let start = out.len();
        out.resize(start + x.len(), 0);
        if self.q.quantize(x, scale, &mut out[start..]).is_err() {
            self.bad += x.iter().filter(|v| v.is_nan()).count() as u64;
        }
        if self.negate {
            for v in &mut out[start..] {
                *v = -*v;
            }
        }
        self.last_scale = scale;
    }
}

impl Component for Quantize {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fec.quantize",
            vec![Port::real("in")],
            vec![Port::llr("out")],
        )
        .with_settings(QuantizeSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.buf.clear();
        self.buf.reserve(self.window);
        self.bad = 0;
        self.last_scale = self.scale as f32;
        self.every = Every::new(inputs[0].rate, 0.25);
        let fixed = (self.scale > 0.0).then_some(self.scale as f32);
        Ok(vec![Format {
            llr_scale: fixed,
            ..inputs[0].with_kind(sdrkit_graph::Kind::Llr)
        }])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let mut out = std::mem::take(io.llrs_out(0));
        if self.scale > 0.0 {
            self.emit(x, self.scale as f32, &mut out);
        } else {
            let mut rest = x;
            while !rest.is_empty() {
                let take = (self.window - self.buf.len()).min(rest.len());
                let (now, later) = rest.split_at(take);
                rest = later;
                if self.buf.is_empty() && take == self.window {
                    // A whole window in place: no copy.
                    let s = self.q.scale_for(now);
                    self.emit(now, s, &mut out);
                    continue;
                }
                self.buf.extend_from_slice(now);
                if self.buf.len() == self.window {
                    let buf = std::mem::take(&mut self.buf);
                    let s = self.q.scale_for(&buf);
                    self.emit(&buf, s, &mut out);
                    self.buf = buf;
                    self.buf.clear();
                }
            }
        }
        *io.llrs_out(0) = out;
        if self.every.tick(x.len()) {
            self.published.set(&LLR_SCALE, self.last_scale as f64);
            self.published.set(&keys::BAD_SAMPLES, self.bad as i64);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match QuantizeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            QuantizeSetting::Scale => {
                self.scale = SCALE.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            QuantizeSetting::Target => self.q.target = TARGET.read(id, v)? as f32,
            QuantizeSetting::Limit => self.q.limit = LIMIT.read(id, v)? as i8,
            QuantizeSetting::Window => {
                self.window = WINDOW.read(id, v)? as usize;
                return Ok(Applied::Reformat);
            }
            QuantizeSetting::Negate => self.negate = NEGATE.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (QuantizeSetting::Scale, Value::F64(self.scale)),
            (QuantizeSetting::Target, Value::F64(self.q.target as f64)),
            (QuantizeSetting::Limit, Value::I64(self.q.limit as i64)),
            (QuantizeSetting::Window, Value::I64(self.window as i64)),
            (QuantizeSetting::Negate, Value::Bool(self.negate)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}
