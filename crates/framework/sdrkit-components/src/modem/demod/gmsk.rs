//! `modem.gmsk_demod`: GMSK and GFSK to soft bits, by the discriminator receiver or by MLSE over the discriminator's levels.

use super::pieces;
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::gmsk::{GfskConfig, GfskDemod, GfskMlseDemod};
use sdrkit_algo_modem::Complex32;
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Port};

/// Symbols decided per MLSE block, and the symbols of look-ahead after each.
const BLOCK: usize = 64;
const CONTEXT: usize = 16;

/// The receiver chosen in `configure`.
#[derive(Debug)]
enum Engine {
    Idle,
    Disc(Box<GfskDemod>),
    Mlse(Box<GfskMlseDemod>),
}

sdrkit_core::choice_enum! {
    /// The detector of [`GmskDemod`].
    pub enum GmskMethod {
        /// Discriminator and slicer.
        Discriminator = ("discriminator", "Discriminator and slicer"),
        /// Discriminator and MLSE.
        Mlse = ("mlse", "Discriminator and MLSE"),
    }
}

const BT: Setting<field::F64> = Setting::new("bt", "Bandwidth-time product", "", 0.5)
    .range(0.1, 2.0, 0.0)
    .help("Of the transmitter's Gaussian filter: 0.3 GSM, 0.4 AIS, 0.5 BLE and most packet radio.");
const INDEX: Setting<field::F64> = Setting::new("index", "Modulation index", "", 0.5)
    .range(0.1, 4.0, 0.0)
    .help("h: 0.5 for GMSK; the peak deviation is h times the symbol rate over 2.");
const BAUD: Setting<field::Hz> =
    Setting::new("baud", "Symbol rate", "Hz", 9600.0).range(1.0, 1e8, 0.0);
const METHOD: Setting<field::Choice<GmskMethod>> =
    Setting::choice("method", "Method", GmskMethod::Discriminator);
const INVERT: Setting<field::Bool> =
    Setting::new("invert", "Invert", "", false).help("Make the higher frequency bit 1.");
const LLR_SCALE: Setting<field::F64> = Setting::new("llr_scale", "Soft bit scale", "", 4.0)
    .range(0.01, 127.0, 0.0)
    .help("i8 units per natural-log unit of the soft bits.");

sdrkit_core::settings! {
    /// The settings of [`GmskDemod`].
    pub enum GmskDemodSetting {
        /// Bandwidth-time product.
        Bt = BT,
        /// Modulation index.
        Index = INDEX,
        /// Symbol rate.
        Baud = BAUD,
        /// Method.
        Method = METHOD,
        /// Invert.
        Invert = INVERT,
        /// Soft bit scale.
        LlrScale = LLR_SCALE,
    }
}

/// `modem.gmsk_demod`: GMSK or GFSK (`Iq`) to soft bits (`Llr`) and hard bits (`HardBits`), one per symbol.
///
/// - **discriminator** wraps `sdrkit_algo_modem::gmsk::GfskDemod`: FM discriminator, one-symbol integrate-and-dump, zero-crossing
///   symbol synchroniser and binary level slicer (which removes a frequency offset as a DC level). Cheap and non-coherent.
/// - **mlse** wraps `sdrkit_algo_modem::gmsk::GfskMlseDemod`: the same discriminator levels (`fsk::DiscriminatorLevels`, normalised
///   by the slicer's fitted centre and spacing) decided by the streaming max-log BCJR (`gmsk::MlseStream`, true LLRs) over the
///   intersymbol interference the Gaussian pulse leaves at the symbol centres (`gmsk::discriminator_taps`), 64 symbols at a time
///   with 16 symbols of look-ahead. Better than slicing at low BT (GSM 0.3, AIS 0.4); the soft bits come up to 79 symbols late.
///
/// The higher frequency (positive deviation, level +1) is bit 0 unless `invert`. Publishes `clock_ppm`.
#[derive(Debug)]
pub struct GmskDemod {
    bt: f64,
    index: f64,
    baud: f64,
    mlse: bool,
    invert: bool,
    llr_scale: f64,
    engine: Engine,
    soft: Vec<f32>,
    q: Vec<i8>,
    results: Published,
    every: Every,
}

impl Default for GmskDemod {
    fn default() -> Self {
        GmskDemod {
            bt: BT.default,
            index: INDEX.default,
            baud: BAUD.default,
            mlse: false,
            invert: INVERT.default,
            llr_scale: LLR_SCALE.default,
            engine: Engine::Idle,
            soft: Vec::new(),
            q: Vec::new(),
            results: Published::new(&[&keys::CLOCK_PPM]),
            every: Every::default(),
        }
    }
}

impl GmskDemod {
    fn reset(&mut self) {
        match &mut self.engine {
            Engine::Idle => {}
            Engine::Disc(d) => d.reset(),
            Engine::Mlse(d) => d.reset(),
        }
    }

    /// Demodulates `x`, appending soft values to `self.soft`.
    fn run(&mut self, x: &[Complex32]) {
        // Errors only on a level that is not a number, which the discriminator never returns.
        let _ = match &mut self.engine {
            Engine::Idle => Ok(()),
            Engine::Disc(d) => d.process(x, &mut self.soft),
            Engine::Mlse(d) => d.process(x, &mut self.soft),
        };
    }
}

impl Component for GmskDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.gmsk_demod",
            vec![Port::iq("in")],
            vec![Port::llr("out"), Port::hard_bits("bits")],
        )
        .with_settings(GmskDemodSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let sps = f.rate / self.baud;
        if !(2.0..=4096.0).contains(&sps) {
            return Err(ConfigError::SettingAtRate {
                id: GmskDemodSetting::Baud.id(),
                rate_hz: f.rate,
            });
        }
        let deviation = self.index / (2.0 * sps);
        if deviation >= 0.5 {
            return Err(ConfigError::SettingAtRate {
                id: GmskDemodSetting::Index.id(),
                rate_hz: f.rate,
            });
        }
        let cfg = GfskConfig::new(sps, deviation, Some(self.bt));
        self.engine = if self.mlse {
            Engine::Mlse(Box::new(
                GfskMlseDemod::new(cfg, BLOCK, CONTEXT)
                    .map_err(ConfigError::from)?
                    .with_inverted(self.invert),
            ))
        } else {
            Engine::Disc(Box::new(
                GfskDemod::new(cfg)
                    .map_err(ConfigError::from)?
                    .with_inverted(self.invert),
            ))
        };
        let most = (4096.0 / sps).ceil() as usize * 2 + BLOCK + 8;
        self.soft = Vec::with_capacity(most);
        self.q = vec![0; most];
        self.every = Every::new(f.rate, 0.25);
        Ok(vec![
            Format::llr(self.baud, Some(self.llr_scale as f32)),
            Format::hard_bits(self.baud),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let x = io.iq(0);
        for (a, b, reset) in pieces(io.input(0), n) {
            if reset {
                self.reset();
            }
            // In pieces of 4096 samples, so the scratch vectors keep the size made in configure.
            for p in x[a..b].chunks(4096) {
                self.soft.clear();
                self.run(p);
                let k = self.soft.len();
                if self.q.len() < k {
                    self.q.resize(k, 0);
                }
                // The scale is positive and the discriminator produces numbers, so this cannot fail.
                let _ = Quantizer::default().quantize(
                    &self.soft,
                    self.llr_scale as f32,
                    &mut self.q[..k],
                );
                io.llrs_out(0).extend_from_slice(&self.q[..k]);
                io.hard_bits_out(1)
                    .extend(self.soft.iter().map(|&l| (l < 0.0) as u8));
            }
        }
        if self.every.tick(n) {
            let ppm = match &self.engine {
                Engine::Idle => return,
                Engine::Disc(d) => d.timing().clock_ppm(),
                Engine::Mlse(d) => d.timing().clock_ppm(),
            };
            self.results.set(&keys::CLOCK_PPM, ppm);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match GmskDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GmskDemodSetting::Bt => self.bt = BT.read(id, v)?,
            GmskDemodSetting::Index => self.index = INDEX.read(id, v)?,
            GmskDemodSetting::Baud => self.baud = BAUD.read(id, v)?,
            GmskDemodSetting::Method => self.mlse = METHOD.read(id, v)? == GmskMethod::Mlse,
            GmskDemodSetting::Invert => self.invert = INVERT.read(id, v)?,
            GmskDemodSetting::LlrScale => self.llr_scale = LLR_SCALE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (GmskDemodSetting::Bt, Value::F64(self.bt)),
            (GmskDemodSetting::Index, Value::F64(self.index)),
            (GmskDemodSetting::Baud, Value::Hz(self.baud)),
            (
                GmskDemodSetting::Method,
                Value::Choice(
                    if self.mlse {
                        GmskMethod::Mlse
                    } else {
                        GmskMethod::Discriminator
                    }
                    .index(),
                ),
            ),
            (GmskDemodSetting::Invert, Value::Bool(self.invert)),
            (GmskDemodSetting::LlrScale, Value::F64(self.llr_scale)),
        ])
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}
