//! `modem.msk_demod`: MSK to soft bits by the non-coherent MSK receiver, with an optional slow carrier-offset removal.

use super::pieces;
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::msk::MskDemod as Receiver;
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{ConfigError, Setting, SettingError, Value, Values};
use sdrkit_core::{SettingId, SettingSet};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Port};

/// Input samples the receiver takes at a time, so the scratch vectors keep the size made in `configure`.
const PIECE: usize = 4096;

const BAUD: Setting<field::Hz> =
    Setting::new("baud", "Bit rate", "Hz", 2400.0).range(1.0, 1e8, 0.0);
const INVERT: Setting<field::Bool> =
    Setting::new("invert", "Invert", "", false).help("Make the higher frequency bit 1.");
const OFFSET_TAU_S: Setting<field::F64> = Setting::new("offset_tau_s", "Offset tracking", "s", 0.0)
    .range(0.0, 1e4, 0.0)
    .help("Time constant of the slow carrier-offset removal; 0 turns it off.");
const CLOCK_BANDWIDTH: Setting<field::F64> =
    Setting::new("clock_bandwidth", "Clock loop bandwidth", "", 0.01)
        .range(1e-4, 0.2, 0.0)
        .help("Noise bandwidth of the timing loop, per bit.");
const LLR_SCALE: Setting<field::F64> = Setting::new("llr_scale", "Soft bit scale", "", 4.0)
    .range(0.01, 127.0, 0.0)
    .help("i8 units per natural-log unit of the soft bits.");

sdrkit_core::settings! {
    /// The settings of [`MskDemod`].
    pub enum MskDemodSetting {
        /// Bit rate.
        Baud = BAUD,
        /// Invert.
        Invert = INVERT,
        /// Offset tracking.
        OffsetTauS = OFFSET_TAU_S,
        /// Clock loop bandwidth.
        ClockBandwidth = CLOCK_BANDWIDTH,
        /// Soft bit scale.
        LlrScale = LLR_SCALE,
    }
}

/// `modem.msk_demod`: MSK (`Iq`) to soft bits (`Llr`) and hard bits (`HardBits`), one per bit.
///
/// It wraps `sdrkit_algo_modem::msk::MskDemod`: a one-bit differential detector, the zero-crossing symbol synchroniser and a level
/// slicer. Alone it follows a carrier offset up to about an eighth of the bit rate. With `offset_tau_s` above 0 an
/// `sdrkit_algo_modem::msk::OffsetTracker` first removes the slow mean of the instantaneous frequency (time constant `offset_tau_s`
/// seconds). The receiver then follows an offset up to about half the bit rate. The higher frequency is bit 0 unless `invert`.
/// Publishes `clock_ppm` and, with the tracker, `offset_hz`.
///
/// ```
/// use sdrkit_components::modem::demod::MskDemod;
/// use sdrkit_core::Value;
/// use sdrkit_graph::{components::NullSink, GraphBuilder};
/// // A DGPS beacon receiver at 100 bit/s with a 2 s offset tracker.
/// let g = GraphBuilder::new()
///     .add("msk", MskDemod::default())
///     .add("sink", NullSink::default())
///     .input("msk")
///     .connect("msk.out", "sink")
///     .set("msk.baud", Value::Hz(100.0))
///     .set("msk.offset_tau_s", Value::F64(2.0))
///     .build();
/// assert!(g.is_ok());
/// ```
#[derive(Debug)]
pub struct MskDemod {
    baud: f64,
    invert: bool,
    offset_tau_s: f64,
    clock_bandwidth: f64,
    llr_scale: f64,
    rate: f64,
    rx: Option<Box<Receiver>>,
    soft: Vec<f32>,
    q: Vec<i8>,
    results: Published,
    every: Every,
}

impl Default for MskDemod {
    fn default() -> Self {
        MskDemod {
            baud: BAUD.default,
            invert: INVERT.default,
            offset_tau_s: OFFSET_TAU_S.default,
            clock_bandwidth: CLOCK_BANDWIDTH.default,
            llr_scale: LLR_SCALE.default,
            rate: 0.0,
            rx: None,
            soft: Vec::new(),
            q: Vec::new(),
            results: Published::new(&[&keys::CLOCK_PPM, &keys::OFFSET_HZ]),
            every: Every::default(),
        }
    }
}

impl Component for MskDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.msk_demod",
            vec![Port::iq("in")],
            vec![Port::llr("out"), Port::hard_bits("bits")],
        )
        .with_settings(MskDemodSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let sps = f.rate / self.baud;
        if !(2.0..=4096.0).contains(&sps) {
            return Err(ConfigError::SettingAtRate {
                id: MskDemodSetting::Baud.id(),
                rate_hz: f.rate,
            });
        }
        let mut rx = Receiver::new(sps, self.clock_bandwidth)
            .map_err(ConfigError::from)?
            .with_inverted(self.invert);
        if self.offset_tau_s > 0.0 {
            rx = rx
                .with_offset_tracking(self.offset_tau_s * self.baud)
                .map_err(ConfigError::from)?;
        }
        self.rx = Some(Box::new(rx));
        self.rate = f.rate;
        let most = (PIECE as f64 / sps).ceil() as usize + 8;
        self.soft = Vec::with_capacity(most);
        self.q = vec![0; most];
        self.every = Every::new(f.rate, 0.25);
        let mut llr = Format::llr(self.baud, Some(self.llr_scale as f32));
        let mut bits = Format::hard_bits(self.baud);
        for o in [&mut llr, &mut bits] {
            o.centre_hz = f.centre_hz;
            o.tuned_hz = f.tuned_hz;
        }
        Ok(vec![llr, bits])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let x = io.iq(0);
        let Some(rx) = &mut self.rx else { return };
        for (a, b, reset) in pieces(io.input(0), n) {
            if reset {
                rx.reset();
            }
            for p in x[a..b].chunks(PIECE) {
                self.soft.clear();
                // Errors only on a level that is not a number, which the detector never returns.
                let _ = rx.process(p, &mut self.soft);
                let k = self.soft.len();
                if self.q.len() < k {
                    self.q.resize(k, 0);
                }
                // The scale is positive and the detector produces numbers, so this cannot fail.
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
            self.results.set(&keys::CLOCK_PPM, rx.timing().clock_ppm());
            if let Some(t) = rx.offset_tracker() {
                self.results.set(&keys::OFFSET_HZ, t.offset() * self.rate);
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MskDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MskDemodSetting::Baud => self.baud = BAUD.read(id, v)?,
            MskDemodSetting::Invert => self.invert = INVERT.read(id, v)?,
            MskDemodSetting::OffsetTauS => self.offset_tau_s = OFFSET_TAU_S.read(id, v)?,
            MskDemodSetting::ClockBandwidth => {
                self.clock_bandwidth = CLOCK_BANDWIDTH.read(id, v)?
            }
            MskDemodSetting::LlrScale => self.llr_scale = LLR_SCALE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (MskDemodSetting::Baud, Value::Hz(self.baud)),
            (MskDemodSetting::Invert, Value::Bool(self.invert)),
            (MskDemodSetting::OffsetTauS, Value::F64(self.offset_tau_s)),
            (
                MskDemodSetting::ClockBandwidth,
                Value::F64(self.clock_bandwidth),
            ),
            (MskDemodSetting::LlrScale, Value::F64(self.llr_scale)),
        ])
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}
