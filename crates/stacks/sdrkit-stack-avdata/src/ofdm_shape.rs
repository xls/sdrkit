//! The structure detector of the OFDM aviation links: the input resampled to the working rate of a numerology and given to
//! the OFDM structure detector of sdrkit-algo-modem, which says whether the cyclic-prefix OFDM of that numerology is there,
//! with its symbol timing, carrier offset and frame shape. Nothing is decoded. The FFT length, the prefix and the frame period
//! are settings of each stack; the test signal is CP-OFDM with random QPSK carriers.

use crate::typed::{OfdmShapeDetectionRow, OfdmShapeFields, OFDM_DETECTIONS};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::ofdm::{FrameShape, Numerology, StructureDetector, StructureReport};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Complex32, ConfigError, GenConfig, Generator, Hz, LockState, Schema, SettingError, SettingId,
    SignalInfo, Value, Values,
};
use sdrkit_graph::{Decoder, Format, Kept, Outbox, Port};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use std::sync::Arc;

/// The key of the FFT length setting.
pub const FFT_LEN_KEY: &str = "fft_len";
/// The key of the prefix setting.
pub const PREFIX_KEY: &str = "prefix";
/// The key of the frame period setting.
pub const FRAME_MS_KEY: &str = "frame_ms";
/// The key of the duty of the test signal.
pub const GEN_DUTY_KEY: &str = "gen_duty";

/// What a stack of this kind is.
#[derive(Debug)]
pub struct Row {
    /// The stack id.
    pub id: &'static str,
    /// The registry name of the decoder.
    pub decoder: &'static str,
    /// The name in texts ("LDACS").
    pub title: &'static str,
    /// The working rate, Hz.
    pub rate_hz: f64,
    /// The least input rate, Hz.
    pub min_rate_hz: f64,
    /// The FFT length by default, samples.
    pub fft_len: usize,
    /// The prefix by default, samples.
    pub prefix: usize,
    /// The frame period by default, ms.
    pub frame_ms: f64,
    /// The share of carriers in use in the test signal.
    pub used: f64,
    /// The settings of the stack.
    pub schema: fn() -> Arc<Schema>,
    /// The settings of the decoder: those of the stack without the test signal.
    pub rx_schema: fn() -> Arc<Schema>,
    /// What no public document confirms.
    pub note: &'static str,
}

/// The most rows of the detection table.
pub const DETECTION_CAP: usize = 100;

/// One row a detection start: when, the numerology, the offset, the correlation and the frame shape then.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectionRow {
    /// Signal time, s.
    pub t: f64,
    /// The symbol with its prefix, us.
    pub symbol_us: f64,
    /// The prefix, us.
    pub prefix_us: f64,
    /// The carrier offset, Hz.
    pub cfo_hz: f64,
    /// The prefix correlation.
    pub correlation: f64,
    /// The frame structure in words (empty while unknown).
    pub frame: String,
}

impl Shaped for DetectionRow {
    type Row = OfdmShapeDetectionRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&OfdmShapeDetectionRow {
            time_s: self.t,
            symbol_us: self.symbol_us,
            prefix_us: self.prefix_us,
            offset_hz: Hertz(self.cfo_hz),
            correlation: self.correlation,
            frame: (!self.frame.is_empty()).then_some(self.frame.as_str()),
        });
    }
}

/// The text of a frame shape over a period of `frame_ms`.
pub fn frame_text(shape: FrameShape, frame_ms: f64) -> String {
    match shape {
        FrameShape::Unknown => String::new(),
        FrameShape::Continuous => format!("continuous over each {frame_ms} ms frame"),
        FrameShape::Bursts { duty } => {
            format!(
                "bursts: on for {:.0} % of each {frame_ms} ms frame",
                100.0 * duty
            )
        }
    }
}

/// The decoder of a structure-only stack.
pub struct ShapeDecoder {
    row: &'static Row,
    fft_len: usize,
    prefix: usize,
    frame_ms: f64,
    nco: Option<Nco>,
    resampler: Option<Resampler<Complex32>>,
    mixed: Vec<Complex32>,
    work: Vec<Complex32>,
    det: Option<StructureDetector>,
    last: Option<StructureReport>,
    last_hit: Option<StructureReport>,
    was_detected: bool,
    rows: LogRing<DetectionRow>,
    dirty: bool,
    cleared: bool,
    lines: Vec<String>,
    started: bool,
    summary: String,
}

impl ShapeDecoder {
    /// The decoder of the stack `row` at its default numerology.
    pub fn new(row: &'static Row) -> Self {
        ShapeDecoder {
            row,
            fft_len: row.fft_len,
            prefix: row.prefix,
            frame_ms: row.frame_ms,
            nco: None,
            resampler: None,
            mixed: Vec::new(),
            work: Vec::new(),
            det: None,
            last: None,
            last_hit: None,
            was_detected: false,
            rows: LogRing::new(DETECTION_CAP),
            dirty: true,
            cleared: false,
            lines: Vec::new(),
            started: false,
            summary: String::new(),
        }
    }

    /// The numerology of the settings.
    pub fn numerology(&self) -> Result<Numerology, ConfigError> {
        Numerology::new(
            self.row.rate_hz,
            self.fft_len,
            &[self.prefix],
            self.frame_ms / 1e3,
        )
        .map_err(|_| ConfigError::RateTooLow {
            need: self.row.min_rate_hz,
            got: 0.0,
        })
    }

    fn build(&mut self, f: &Format) -> Result<(), ConfigError> {
        let off = f.signal_offset_hz();
        self.nco = (off != 0.0).then(|| Nco::new(f.rate, -off));
        self.resampler = if (f.rate - self.row.rate_hz).abs() > 1e-6 {
            Some(
                Resampler::new(f.rate, self.row.rate_hz).map_err(|_| ConfigError::RateTooLow {
                    need: self.row.min_rate_hz,
                    got: f.rate,
                })?,
            )
        } else {
            None
        };
        self.det = Some(StructureDetector::new(self.numerology()?).map_err(|_| {
            ConfigError::RateTooLow {
                need: self.row.min_rate_hz,
                got: f.rate,
            }
        })?);
        Ok(())
    }
}

impl Decoder for ShapeDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        self.row.decoder
    }
    fn outputs(&self) -> Vec<Port> {
        Vec::new()
    }
    fn settings(&self) -> Arc<Schema> {
        (self.row.rx_schema)()
    }
    fn values(&self) -> Values {
        let schema = self.settings();
        let mut v = Values::defaults(&schema);
        for (key, val) in [
            (FFT_LEN_KEY, Value::I64(self.fft_len as i64)),
            (PREFIX_KEY, Value::I64(self.prefix as i64)),
            (FRAME_MS_KEY, Value::F64(self.frame_ms)),
        ] {
            if let Ok(id) = schema.id(key) {
                let _ = v.set(id, &val);
            }
        }
        v
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let schema = self.settings();
        let key = schema
            .setting(id)
            .map(|s| s.field.key.to_string())
            .ok_or(SettingError::NoSuchId { id })?;
        match (key.as_str(), v) {
            (FFT_LEN_KEY, Value::I64(n)) => self.fft_len = usize::try_from(*n).unwrap_or(64),
            (PREFIX_KEY, Value::I64(n)) => self.prefix = usize::try_from(*n).unwrap_or(1),
            (FRAME_MS_KEY, Value::F64(x)) => self.frame_ms = *x,
            _ => return Err(SettingError::NoSuchId { id }),
        }
        if self.prefix > self.fft_len {
            self.prefix = self.fft_len;
        }
        if let Ok(num) = self.numerology() {
            if let Ok(d) = StructureDetector::new(num) {
                if self.det.is_some() {
                    self.det = Some(d);
                }
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        crate::typed::results(self.row.id)
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < self.row.min_rate_hz * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: self.row.min_rate_hz,
                got: f.rate,
            });
        }
        self.build(f)?;
        Ok(Vec::new())
    }
    fn retune(&mut self, f: &Format) {
        let _ = self.build(f);
    }
    fn process(&mut self, x: &[Complex32], _first: u64, _out: &mut Outbox) {
        self.started = true;
        let input: &[Complex32] = match self.nco.as_mut() {
            Some(nco) => {
                self.mixed.clear();
                self.mixed.extend_from_slice(x);
                nco.mix_in_place(&mut self.mixed);
                &self.mixed
            }
            None => x,
        };
        let samples: &[Complex32] = match self.resampler.as_mut() {
            Some(r) => {
                self.work.clear();
                r.process(input, &mut self.work);
                &self.work
            }
            None => input,
        };
        let Some(det) = self.det.as_mut() else {
            return;
        };
        let (last, last_hit) = (&mut self.last, &mut self.last_hit);
        det.process(samples, |r| {
            *last = Some(*r);
            if r.hit {
                *last_hit = Some(*r);
            }
        });
    }
    fn report(&mut self, p: &mut Published) {
        let num = self.numerology().ok();
        let detected = self.last.is_some_and(|r| r.detected);
        let durations = num
            .as_ref()
            .map(|n| (n.symbol_s() * 1e6, n.prefix_s() * 1e6));
        let (symbol_us, prefix_us) = durations.unwrap_or((0.0, 0.0));
        let frame = self
            .last
            .map(|r| frame_text(r.frame, self.frame_ms))
            .unwrap_or_default();
        let now = self.last.map_or(0.0, |r| r.time_s);
        if let (true, false, Some(h)) = (detected, self.was_detected, self.last_hit) {
            self.rows.push(DetectionRow {
                t: now,
                symbol_us,
                prefix_us,
                cfo_hz: h.cfo_hz,
                correlation: h.correlation,
                frame: frame.clone(),
            });
            self.dirty = true;
            self.lines.push(format!(
                "{}: OFDM symbol {symbol_us:.1} us, prefix {prefix_us:.1} us, {:.0} samples/s",
                self.row.title, self.row.rate_hz
            ));
        }
        if !detected && self.was_detected {
            self.lines
                .push(format!("{}: the OFDM is no longer there", self.row.title));
        }
        self.was_detected = detected;
        let hit = self.last_hit.filter(|_| detected);
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        for l in self.lines.drain(..) {
            p.results_mut().log(l);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.rows, &OFDM_DETECTIONS, now, p);
        }
        let energy = self.last.is_some_and(|r| r.energy);
        self.summary.clear();
        self.summary.push_str(&if !self.started {
            format!("{}: waiting for samples", self.row.title)
        } else if detected {
            let mut s = format!(
                "{}: OFDM found, symbol {symbol_us:.1} us, prefix {prefix_us:.1} us",
                self.row.title
            );
            if !frame.is_empty() {
                s.push_str(&format!("; {frame}"));
            }
            s.push_str("; structure only");
            s
        } else if energy {
            format!("{}: energy, not this OFDM", self.row.title)
        } else {
            format!("{}: no OFDM of this numerology", self.row.title)
        });
        p.put(&OfdmShapeFields {
            detected,
            symbol_us: durations.map(|d| d.0),
            prefix_us: durations.map(|d| d.1),
            timing: hit.map(|h| h.timing as i64),
            correlation: hit.map(|h| h.correlation),
            offset_hz: hit.map(|h| Hertz(h.cfo_hz)),
            frame: (!frame.is_empty()).then_some(frame.as_str()),
            content: "structure only",
            text: &self.summary,
            note: self.row.note,
        });
        let lock = match (self.started, detected, energy) {
            (false, ..) => LockState::Off,
            (_, true, _) => LockState::Locked,
            (_, false, true) => LockState::Tracking,
            _ => LockState::Searching,
        };
        let level = self
            .last
            .filter(|r| r.mean_power > 0.0)
            .map(|r| (10.0 * r.mean_power.log10()) as f32);
        let summary = &self.summary;
        p.results_mut().update_status(|st| {
            let changed = st.lock_state != lock || st.summary != *summary || st.level_dbfs != level;
            if changed {
                st.lock_state = lock;
                st.summary.clone_from(summary);
                st.level_dbfs = level;
            }
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.dirty = true;
        self.cleared = true;
        self.forget_signal();
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(OFDM_DETECTIONS.key) {
            self.rows.clear();
            self.dirty = true;
        }
        self.forget_signal();
    }
}

impl ShapeDecoder {
    /// Forgets the present signal: the last report and the detector state.
    fn forget_signal(&mut self) {
        self.last = None;
        self.last_hit = None;
        self.was_detected = false;
        if let Ok(num) = self.numerology() {
            self.det = StructureDetector::new(num)
                .ok()
                .filter(|_| self.det.is_some());
        }
    }
}

/// The test signal: CP-OFDM with random QPSK on the carriers in use (none at DC), at the working rate of the numerology and
/// resampled to the rate asked for, on for `duty` of each frame period. Unit power while it is on.
pub struct ShapeGenerator {
    rate: f64,
    work_rate: f64,
    fft_len: usize,
    prefix: usize,
    used: Vec<usize>,
    frame: f64,
    duty: f64,
    fft: Fft,
    rng: Rng,
    n: u64,
    sym: Vec<Complex32>,
    work: Vec<Complex32>,
    out: Vec<Complex32>,
    pos: usize,
    resampler: Option<Resampler<Complex32>>,
}

impl ShapeGenerator {
    /// The generator of stack `row` with the settings of `cfg`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below the least rate of the stack.
    pub fn from_config(row: &'static Row, cfg: &GenConfig) -> Result<Self, ConfigError> {
        if cfg.rate_hz < row.min_rate_hz * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: row.min_rate_hz,
                got: cfg.rate_hz,
            });
        }
        let schema = (row.schema)();
        let get = |key: &str| {
            schema
                .id(key)
                .ok()
                .and_then(|id| cfg.settings.value(id).cloned())
        };
        let fft_len = match get(FFT_LEN_KEY) {
            Some(Value::I64(n)) => usize::try_from(n).unwrap_or(row.fft_len).max(8),
            _ => row.fft_len,
        };
        let prefix = match get(PREFIX_KEY) {
            Some(Value::I64(n)) => usize::try_from(n).unwrap_or(row.prefix).min(fft_len),
            _ => row.prefix,
        };
        let frame_ms = match get(FRAME_MS_KEY) {
            Some(Value::F64(x)) => x,
            _ => row.frame_ms,
        };
        let duty = match get(GEN_DUTY_KEY) {
            Some(Value::F64(x)) => (x / 100.0).clamp(0.01, 1.0),
            _ => 1.0,
        };
        let half = ((fft_len as f64 * row.used) / 2.0).round() as usize;
        let used = (1..=half).chain((1..=half).map(|k| fft_len - k)).collect();
        let resampler = if (cfg.rate_hz - row.rate_hz).abs() > 1e-6 {
            Some(
                Resampler::new(row.rate_hz, cfg.rate_hz).map_err(|_| ConfigError::RateTooLow {
                    need: row.min_rate_hz,
                    got: cfg.rate_hz,
                })?,
            )
        } else {
            None
        };
        Ok(ShapeGenerator {
            rate: cfg.rate_hz,
            work_rate: row.rate_hz,
            fft_len,
            prefix,
            used,
            frame: frame_ms / 1e3 * row.rate_hz,
            duty,
            fft: Fft::inverse(fft_len),
            rng: Rng::new(cfg.seed ^ 0x4F46_444D),
            n: 0,
            sym: vec![Complex32::default(); fft_len],
            work: Vec::new(),
            out: Vec::new(),
            pos: 0,
            resampler,
        })
    }

    /// Appends one symbol with its prefix at the working rate to `self.work`.
    fn symbol(&mut self) {
        self.sym.fill(Complex32::default());
        let scale = 1.0 / (self.used.len() as f32).sqrt();
        for &k in &self.used {
            let q = self.rng.next_u64();
            self.sym[k] = Complex32::new(
                if q & 1 == 0 { scale } else { -scale },
                if q & 2 == 0 { scale } else { -scale },
            ) * std::f32::consts::FRAC_1_SQRT_2;
        }
        self.fft.process(&mut self.sym);
        let start = self.work.len();
        self.work
            .extend_from_slice(&self.sym[self.fft_len - self.prefix..]);
        self.work.extend_from_slice(&self.sym);
        for v in &mut self.work[start..] {
            let on = (self.n as f64).rem_euclid(self.frame) < self.duty * self.frame;
            if !on {
                *v = Complex32::default();
            }
            self.n += 1;
        }
    }
}

impl Generator for ShapeGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.out.len() {
                self.out.clear();
                self.pos = 0;
                self.work.clear();
                while self.work.len() < 4096 {
                    self.symbol();
                }
                match self.resampler.as_mut() {
                    Some(r) => r.process(&self.work, &mut self.out),
                    None => self.out.extend_from_slice(&self.work),
                }
                continue;
            }
            let n = (self.out.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let width = self.work_rate * self.used.len() as f64 / self.fft_len as f64;
        Some(SignalInfo::new(width))
    }
}
