//! `modem.slicer`: decisions and soft bits from symbols.

use super::points::{ConstellationName, CONSTELLATION};
use super::{is_retune, points, split_at_tags};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::differential::Differential;
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, TagCarry};

sdrkit_core::choice_enum! {
    /// How [`Slicer`] computes its LLRs.
    pub enum DemapMethod {
        /// Exact.
        Exact = ("exact", "Exact"),
        /// Max-log.
        MaxLog = ("max_log", "Max-log"),
    }
}

sdrkit_core::choice_enum! {
    /// The differential coding that [`Slicer`] undoes.
    pub enum DifferentialCoding {
        /// None (coherent).
        None = ("none", "None (coherent)"),
        /// DBPSK.
        Dbpsk = ("dbpsk", "DBPSK"),
        /// DQPSK.
        Dqpsk = ("dqpsk", "DQPSK"),
        /// pi/4-DQPSK (DAB).
        Pi4DqpskDab = ("pi4_dqpsk_dab", "pi/4-DQPSK (DAB)"),
        /// pi/4-DQPSK (TETRA).
        Pi4DqpskTetra = ("pi4_dqpsk_tetra", "pi/4-DQPSK (TETRA)"),
        /// D8PSK.
        D8psk = ("d8psk", "D8PSK"),
    }
}

/// The most bits per symbol (1024-QAM).
const MAX_BITS: usize = 16;

enum Detector {
    Coherent(Constellation),
    Differential(Differential),
}

const DEMAP: Setting<field::Choice<DemapMethod>> =
    Setting::choice("demap", "Demapping", DemapMethod::MaxLog);
const DIFFERENTIAL: Setting<field::Choice<DifferentialCoding>> =
    Setting::choice("differential", "Differential", DifferentialCoding::None);
const NOISE_VAR: Setting<field::F64> = Setting::new("noise_var", "Noise variance", "", 0.1)
    .range(1e-6, 100.0, 0.0)
    .help("Complex noise variance of unit-power symbols (1 / (Es/N0)).");
const LLR_SCALE: Setting<field::F64> = Setting::new("llr_scale", "LLR scale", "", 4.0)
    .range(0.01, 127.0, 0.0)
    .help("i8 units per natural-log unit of LLR.");

sdrkit_core::settings! {
    /// The settings of [`Slicer`].
    pub enum SlicerSetting {
        /// Constellation.
        Constellation = CONSTELLATION,
        /// Demapping.
        Demap = DEMAP,
        /// Differential.
        Differential = DIFFERENTIAL,
        /// Noise variance.
        NoiseVar = NOISE_VAR,
        /// LLR scale.
        LlrScale = LLR_SCALE,
    }
}

/// The slicer (`modem.slicer`): `Symbols` in; soft bits (`Llr`, `bits` per symbol) and the decided points (`Symbols`, for EVM) out.
///
/// The LLRs are true LLRs, `L = ln P(0) / P(1)` (positive means 0), from `sdrkit_algo_modem::Constellation::demap` with the noise
/// variance of the symbols (`noise_var`, the total complex variance `E|n|^2` of unit-power symbols, or the optional `Real` input),
/// then quantised to `i8` by `sdrkit_algo_fec::llr::Quantizer` at the fixed scale `llr_scale` (`i8` units per natural-log unit), which
/// the output format carries. A symbol that is not finite returns erasures (0).
///
/// With `differential` (`dbpsk`, `dqpsk`, `pi4_dqpsk_dab`, `pi4_dqpsk_tetra`, `d8psk`) the bits are the phase step from the
/// previous symbol (`sdrkit_algo_modem::differential::Differential`), demapped with twice the noise variance, and the decision output
/// is the decided step; `constellation` is then not used. A `Retuned` tag forgets the previous symbol.
///
/// The optional input `noise` (`Real`, may be left unconnected): noise variances; symbol `k` of a call uses its item `k` (or its
/// last item when it holds fewer), and with no items the last value received. Unconnected, every symbol uses the `noise_var`
/// setting.
///
/// Settings (each rebuilds the slicer): `constellation` (a [`points::ConstellationName`]), `demap` (`exact`, `max_log`),
/// `differential`, `noise_var`, `llr_scale`.
pub struct Slicer {
    constellation: ConstellationName,
    demap: DemapMethod,
    differential: DifferentialCoding,
    noise_var: f64,
    llr_scale: f64,
    det: Option<Detector>,
    nv: f32,
    bits: usize,
    soft: Vec<f32>,
    quant: Quantizer,
    carry_llr: TagCarry,
    carry_dec: TagCarry,
}

impl Default for Slicer {
    /// QPSK, max-log, coherent, noise variance 0.1, scale 4.
    fn default() -> Self {
        Slicer {
            constellation: ConstellationName::Qpsk,
            demap: DemapMethod::MaxLog,
            differential: DifferentialCoding::None,
            noise_var: NOISE_VAR.default,
            llr_scale: LLR_SCALE.default,
            det: None,
            nv: 0.1,
            bits: 2,
            soft: Vec::new(),
            quant: Quantizer::default(),
            carry_llr: TagCarry::default(),
            carry_dec: TagCarry::default(),
        }
    }
}

impl Slicer {
    /// A coherent slicer on the constellation `c`.
    pub fn new(c: ConstellationName) -> Self {
        Slicer {
            constellation: c,
            ..Self::default()
        }
    }
    fn method(&self) -> Demap {
        match self.demap {
            DemapMethod::Exact => Demap::Exact,
            DemapMethod::MaxLog => Demap::MaxLog,
        }
    }
}

impl Component for Slicer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.slicer",
            vec![
                Port::symbols("in"),
                Port::real("noise").optional("every symbol uses the `noise_var` setting"),
            ],
            vec![Port::llr("out"), Port::symbols("decisions")],
        )
        .manual_tags()
        .with_settings(SlicerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        let det = match self.differential {
            DifferentialCoding::None => {
                Detector::Coherent(points::constellation(self.constellation)?)
            }
            DifferentialCoding::Dbpsk => Detector::Differential(Differential::dbpsk()),
            DifferentialCoding::Dqpsk => Detector::Differential(Differential::dqpsk()),
            DifferentialCoding::Pi4DqpskDab => {
                Detector::Differential(Differential::pi4_dqpsk_dab())
            }
            DifferentialCoding::Pi4DqpskTetra => {
                Detector::Differential(Differential::pi4_dqpsk_tetra())
            }
            DifferentialCoding::D8psk => Detector::Differential(Differential::d8psk()),
        };
        self.bits = match &det {
            Detector::Coherent(c) => c.bits(),
            Detector::Differential(d) => d.bits(),
        };
        if self.bits > MAX_BITS {
            return Err(SettingError::NotPermitted {
                id: SlicerSetting::Constellation.id(),
            }
            .into());
        }
        self.det = Some(det);
        self.nv = self.noise_var as f32;
        self.soft = Vec::with_capacity(1024 * self.bits);
        let llr_rate = f.rate * self.bits as f64;
        self.carry_llr = TagCarry::new(f.rate, llr_rate);
        self.carry_dec = TagCarry::same_rate();
        Ok(vec![
            Format::llr(llr_rate, Some(self.llr_scale as f32)),
            f.clone(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.samples::<Complex32>(0);
        let input = io.input(0);
        let noise: &[f32] = if io.is_connected(1) { io.real(1) } else { &[] };
        if self.det.is_none() {
            return;
        }
        let bits = self.bits;
        let method = self.method();
        let dec_out = io.samples_out::<Complex32>(1);
        self.soft.clear();
        let mut st = (&mut *self, dec_out);
        split_at_tags(
            &mut st,
            input,
            x.len(),
            |(me, dec), a, b| {
                let mut l = [0f32; MAX_BITS];
                for (k, &z) in x[a..b].iter().enumerate() {
                    let i = a + k;
                    if !noise.is_empty() {
                        let v = noise[i.min(noise.len() - 1)];
                        if v > 0.0 && v.is_finite() {
                            me.nv = v;
                        }
                    }
                    let nv = me.nv;
                    let (ok, d) = match me.det.as_mut() {
                        Some(Detector::Coherent(c)) => {
                            (c.demap(z, nv, method, &mut l).is_ok(), c.nearest(z))
                        }
                        Some(Detector::Differential(dd)) => {
                            let s = dd.step(z);
                            let st = dd.steps();
                            (st.demap(s, 2.0 * nv, method, &mut l).is_ok(), st.nearest(s))
                        }
                        None => (false, Complex32::default()),
                    };
                    if ok {
                        me.soft.extend_from_slice(&l[..bits]);
                    } else {
                        me.soft.extend(std::iter::repeat_n(0.0, bits));
                    }
                    dec.push(d);
                }
            },
            |(me, _), t| {
                if is_retune(t) {
                    if let Some(Detector::Differential(d)) = me.det.as_mut() {
                        d.reset();
                    }
                }
            },
        );
        let out = io.llrs_out(0);
        let start = out.len();
        out.resize(start + self.soft.len(), 0);
        // The scale is fixed and positive, and NaN never reaches here, so this cannot fail; an error leaves erasures.
        let _ = self
            .quant
            .quantize(&self.soft, self.llr_scale as f32, &mut out[start..]);
        self.carry_llr.forward(input, io.output(0));
        self.carry_dec.forward(input, io.output(1));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SlicerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SlicerSetting::Constellation => self.constellation = CONSTELLATION.read(id, v)?,
            SlicerSetting::Demap => self.demap = DEMAP.read(id, v)?,
            SlicerSetting::Differential => self.differential = DIFFERENTIAL.read(id, v)?,
            SlicerSetting::NoiseVar => {
                self.noise_var = NOISE_VAR.read(id, v)?;
                self.nv = self.noise_var as f32;
                return Ok(Applied::Live);
            }
            SlicerSetting::LlrScale => self.llr_scale = LLR_SCALE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (
                SlicerSetting::Constellation,
                Value::Choice(self.constellation.index()),
            ),
            (SlicerSetting::Demap, Value::Choice(self.demap.index())),
            (
                SlicerSetting::Differential,
                Value::Choice(self.differential.index()),
            ),
            (SlicerSetting::NoiseVar, Value::F64(self.noise_var)),
            (SlicerSetting::LlrScale, Value::F64(self.llr_scale)),
        ])
    }
}
