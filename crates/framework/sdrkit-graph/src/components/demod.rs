//! Demodulators: FM, AM and SSB/CW.

use super::{dsp, Every};
use crate::component::values_of;
use crate::component::{Applied, ChannelsFrom, Component, ComponentInfo, Io, Port, RateFrom};
use crate::stream::{Format, Kind};
use sdrkit_algo_dsp::fir::{self, ComplexFir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::Values;
use sdrkit_core::{
    Complex32, ConfigError, Published, Results, Setting, SettingError, SettingId, SettingSet, Value,
};
use std::f64::consts::TAU;

const DEVIATION: Setting<field::Hz> = Setting::new("deviation", "Deviation", "Hz", 5e3)
    .range(1.0, 1e6, 0.0)
    .help("The peak deviation, which gives full scale.");

sdrkit_core::settings! {
    /// The settings of [`FmDemod`].
    pub enum FmDemodSetting {
        /// The peak deviation.
        Deviation = DEVIATION,
    }
}

sdrkit_core::choice_enum! {
    /// The detector of an AM demodulator.
    pub enum AmDetector {
        /// The envelope.
        Envelope = ("envelope", "Envelope"),
        /// The in-phase part after a loop that tracks the carrier.
        Synchronous = ("synchronous", "Synchronous"),
    }
}

const DETECTOR: Setting<field::Choice<AmDetector>> =
    Setting::choice("detector", "Detector", AmDetector::Envelope)
        .help("Synchronous detection tracks the carrier, and copes better with fading.");

sdrkit_core::settings! {
    /// The settings of [`AmDemod`].
    pub enum AmDemodSetting {
        /// The detector.
        Detector = DETECTOR,
    }
}

/// The result set of [`AmDemod`].
#[derive(Clone, Copy, Debug, PartialEq, Fields)]
pub struct AmDemodFields {
    /// The offset of the carrier that the synchronous detector tracks. No value with the envelope detector and before
    /// the first measurement.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Option<Hertz>,
}

sdrkit_core::choice_enum! {
    /// What an SSB demodulator keeps.
    pub enum Sideband {
        /// The upper sideband.
        Usb = ("usb", "USB"),
        /// The lower sideband.
        Lsb = ("lsb", "LSB"),
        /// A CW tone.
        Cw = ("cw", "CW"),
    }
}

const SIDEBAND: Setting<field::Choice<Sideband>> =
    Setting::choice("sideband", "Sideband", Sideband::Usb);
const BFO: Setting<field::Hz> = Setting::new("bfo", "BFO offset", "Hz", 0.0)
    .range(-20e3, 20e3, 0.0)
    .help("Raises the pitch of what is heard by this much.");
const PITCH: Setting<field::Hz> =
    Setting::new("pitch", "CW pitch", "Hz", 700.0).range(0.0, 20e3, 0.0);
const LOW: Setting<field::Hz> = Setting::new("low", "Low cut", "Hz", 300.0).range(0.0, 20e3, 0.0);
const HIGH: Setting<field::Hz> =
    Setting::new("high", "High cut", "Hz", 2700.0).range(0.0, 20e3, 0.0);
const CW_WIDTH: Setting<field::Hz> =
    Setting::new("cw_width", "CW width", "Hz", 500.0).range(10.0, 20e3, 0.0);

sdrkit_core::settings! {
    /// The settings of [`SsbDemod`].
    pub enum SsbDemodSetting {
        /// The sideband.
        Sideband = SIDEBAND,
        /// The BFO offset.
        Bfo = BFO,
        /// The CW pitch.
        Pitch = PITCH,
        /// The low edge of the pass band.
        Low = LOW,
        /// The high edge of the pass band.
        High = HIGH,
        /// The width of the CW filter.
        CwWidth = CW_WIDTH,
    }
}

/// FM demodulator (`dsp.fm_demod`): a quadrature discriminator, `arg(x[n] conj(x[n-1]))` (sdrkit-algo-dsp [`Discriminator`]), scaled so a
/// deviation of `deviation` Hz produces +-1.0. The output is `Real` at the input rate.
///
/// Settings: `deviation` (Hz, live; 75 kHz for broadcast, 2.5 to 5 kHz for narrow FM).
pub struct FmDemod {
    deviation: f64,
    disc: Option<Discriminator>,
}

impl Default for FmDemod {
    fn default() -> Self {
        FmDemod {
            deviation: DEVIATION.default,
            disc: None,
        }
    }
}

impl FmDemod {
    /// For a peak deviation of `deviation_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `deviation` setting refuses `deviation_hz`.
    pub fn new(deviation_hz: f64) -> Result<Self, SettingError> {
        FmDemodSetting::Deviation.check(&Value::Hz(deviation_hz))?;
        Ok(FmDemod {
            deviation: deviation_hz,
            disc: None,
        })
    }
}

impl Component for FmDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.fm_demod",
            vec![Port::iq("in")],
            vec![Port::real("out")
                .rate_from(RateFrom::Input(0))
                .channels_from(ChannelsFrom::Fixed(1))],
        )
        .with_settings(FmDemodSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(FmDemodSetting::Deviation, Value::Hz(self.deviation))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.disc = Some(Discriminator::new(f.rate, self.deviation).map_err(dsp)?);
        Ok(vec![Format {
            bandwidth_hz: f.bandwidth_hz / 2.0,
            ..f.with_kind(Kind::Real)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(d) = &mut self.disc else { return };
        let x = io.iq(0);
        d.process(x, io.real_out(0));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FmDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FmDemodSetting::Deviation => {
                let d = DEVIATION.read(id, v)?;
                if let Some(disc) = &mut self.disc {
                    disc.set_deviation(d);
                }
                self.deviation = d;
            }
        }
        Ok(Applied::Live)
    }
}

/// AM demodulator (`dsp.am_demod`): `Iq` to `Real`, normalised to the carrier, so 100 % modulation produces +-1.0 and the carrier
/// itself produces 0.
///
/// - `envelope`: `|x| / carrier - 1`, the carrier level being a slow average of `|x|` (0.1 s).
/// - `synchronous`: a phase-locked loop (about 30 Hz wide) tracks the carrier, and the in-phase part is detected; it copes with
///   selective fading and a carrier a little off the centre. Results: `carrier_offset_hz`.
///
/// Settings: `detector` (`envelope` or `synchronous`, live).
pub struct AmDemod {
    detector: AmDetector,
    rate: f64,
    alpha: f32,
    carrier: f32,
    started: bool,
    phase: f64,
    freq: f64,
    k1: f64,
    k2: f64,
    every: Every,
    results: Published,
}

impl Default for AmDemod {
    fn default() -> Self {
        AmDemod::new(AmDetector::Envelope)
    }
}

impl AmDemod {
    /// With `detector`.
    pub fn new(detector: AmDetector) -> Self {
        AmDemod {
            detector,
            rate: 0.0,
            alpha: 0.0,
            carrier: 0.0,
            started: false,
            phase: 0.0,
            freq: 0.0,
            k1: 0.0,
            k2: 0.0,
            every: Every::default(),
            results: Published::new(AmDemodFields::SHAPE.fields),
        }
    }
    /// The tracked carrier's offset from 0 Hz (synchronous detector).
    pub fn carrier_offset_hz(&self) -> f64 {
        self.freq * self.rate / TAU
    }
}

impl Component for AmDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.am_demod",
            vec![Port::iq("in")],
            vec![Port::real("out")
                .rate_from(RateFrom::Input(0))
                .channels_from(ChannelsFrom::Fixed(1))],
        )
        .with_settings(AmDemodSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(
            AmDemodSetting::Detector,
            Value::Choice(sdrkit_core::Choice::index(self.detector)),
        )])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        *self = AmDemod {
            rate: f.rate,
            every: Every::new(f.rate, 0.25),
            ..AmDemod::new(self.detector)
        };
        self.alpha = (1.0 - (-1.0 / (0.1 * f.rate)).exp()) as f32;
        // A second-order loop, natural frequency 30 Hz, damping 0.707.
        let wn = TAU * 30.0 / f.rate;
        self.k1 = 2.0 * 0.707 * wn;
        self.k2 = wn * wn;
        Ok(vec![Format {
            bandwidth_hz: f.bandwidth_hz / 2.0,
            ..f.with_kind(Kind::Real)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let out = io.real_out(0);
        out.reserve(x.len());
        let sync = self.detector == AmDetector::Synchronous;
        for &s in x {
            let (level, detected) = if sync {
                let lo = Complex32::new(self.phase.cos() as f32, -self.phase.sin() as f32);
                let z = s * lo;
                let err = if z.norm_sqr() > 0.0 {
                    z.im.atan2(z.re) as f64
                } else {
                    0.0
                };
                self.freq += self.k2 * err;
                self.phase = (self.phase + self.freq + self.k1 * err).rem_euclid(TAU);
                (z.re, z.re)
            } else {
                let m = s.norm();
                (m, m)
            };
            if !self.started {
                self.carrier = level;
                self.started = true;
            }
            self.carrier += self.alpha * (level - self.carrier);
            out.push(if self.carrier > 1e-12 {
                detected / self.carrier - 1.0
            } else {
                0.0
            });
        }
        if sync && self.every.tick(x.len()) {
            let off = self.carrier_offset_hz();
            self.results.put(&AmDemodFields {
                carrier_offset_hz: Some(Hertz(off)),
            });
            self.results.results_mut().update_status(|s| {
                let c = s.offset_hz != Some(off);
                s.offset_hz = Some(off);
                c
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AmDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AmDemodSetting::Detector => self.detector = DETECTOR.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// SSB and CW demodulator (`dsp.ssb_demod`): `Iq` (the channel, with the suppressed carrier at 0 Hz) to `Real`.
///
/// The signal is shifted by the BFO, one sideband is kept by a complex band-pass, and the real part is the audio:
///
/// - `usb`: audio `low..high` Hz above the carrier, heard `bfo` Hz higher.
/// - `lsb`: the same below the carrier.
/// - `cw`: a carrier at 0 Hz is heard as a tone of `pitch + bfo` Hz, through a `cw_width` band-pass around it.
///
/// Settings (live): `sideband` (`usb`, `lsb`, `cw`), `bfo` (Hz), `pitch` (Hz), `low`, `high`, `cw_width` (Hz).
pub struct SsbDemod {
    sideband: Sideband,
    bfo: f64,
    pitch: f64,
    low: f64,
    high: f64,
    cw_width: f64,
    rate: f64,
    nco: Option<Nco>,
    filter: Option<ComplexFir>,
    work: Vec<Complex32>,
}

impl Default for SsbDemod {
    fn default() -> Self {
        SsbDemod::new(Sideband::Usb)
    }
}

impl SsbDemod {
    /// For `sideband`.
    pub fn new(sideband: Sideband) -> Self {
        SsbDemod {
            sideband,
            bfo: BFO.default,
            pitch: PITCH.default,
            low: LOW.default,
            high: HIGH.default,
            cw_width: CW_WIDTH.default,
            rate: 0.0,
            nco: None,
            filter: None,
            work: Vec::new(),
        }
    }
    /// The shift, and the band kept after it.
    fn plan(&self) -> (f64, f64, f64) {
        match self.sideband {
            Sideband::Lsb => (-self.bfo, -(self.high + self.bfo), -(self.low + self.bfo)),
            Sideband::Cw => {
                let p = self.pitch + self.bfo;
                (p, p - self.cw_width / 2.0, p + self.cw_width / 2.0)
            }
            Sideband::Usb => (self.bfo, self.low + self.bfo, self.high + self.bfo),
        }
    }
    /// Makes the shift and the band-pass for the present settings.
    ///
    /// # Errors
    ///
    /// [`ConfigError::BandwidthNotSupported`] if the pass band does not fit the rate.
    fn build(&mut self) -> Result<(), ConfigError> {
        let (shift, lo, hi) = self.plan();
        let nyq = self.rate / 2.0;
        let t = (0.1 * (hi - lo)).clamp(50.0, 300.0);
        if !(lo < hi && lo - t > -nyq && hi + t < nyq) {
            return Err(ConfigError::BandwidthNotSupported { got: hi - lo });
        }
        match &mut self.nco {
            Some(n) => n.set_freq(shift),
            None => self.nco = Some(Nco::new(self.rate, shift)),
        }
        self.filter = Some(ComplexFir::new(
            &fir::bandpass_complex(self.rate, lo, hi, t, 60.0).map_err(dsp)?,
        ));
        Ok(())
    }
}

impl Component for SsbDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.ssb_demod",
            vec![Port::iq("in")],
            vec![Port::real("out")
                .rate_from(RateFrom::Input(0))
                .channels_from(ChannelsFrom::Fixed(1))],
        )
        .with_settings(SsbDemodSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (
                SsbDemodSetting::Sideband,
                Value::Choice(sdrkit_core::Choice::index(self.sideband)),
            ),
            (SsbDemodSetting::Bfo, Value::Hz(self.bfo)),
            (SsbDemodSetting::Pitch, Value::Hz(self.pitch)),
            (SsbDemodSetting::Low, Value::Hz(self.low)),
            (SsbDemodSetting::High, Value::Hz(self.high)),
            (SsbDemodSetting::CwWidth, Value::Hz(self.cw_width)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.nco = None;
        self.build()?;
        let (_, lo, hi) = self.plan();
        Ok(vec![Format {
            bandwidth_hz: lo.abs().max(hi.abs()),
            ..f.with_kind(Kind::Real)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let (Some(nco), Some(filter)) = (&mut self.nco, &mut self.filter) else {
            return;
        };
        let x = io.iq(0);
        self.work.clear();
        self.work.extend_from_slice(x);
        nco.mix_in_place(&mut self.work);
        filter.process_in_place(&mut self.work);
        io.real_out(0).extend(self.work.iter().map(|z| z.re));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let old = (
            self.sideband,
            self.bfo,
            self.pitch,
            self.low,
            self.high,
            self.cw_width,
        );
        match SsbDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SsbDemodSetting::Sideband => self.sideband = SIDEBAND.read(id, v)?,
            SsbDemodSetting::Bfo => self.bfo = BFO.read(id, v)?,
            SsbDemodSetting::Pitch => self.pitch = PITCH.read(id, v)?,
            SsbDemodSetting::Low => self.low = LOW.read(id, v)?,
            SsbDemodSetting::High => self.high = HIGH.read(id, v)?,
            SsbDemodSetting::CwWidth => self.cw_width = CW_WIDTH.read(id, v)?,
        }
        if self.rate > 0.0 {
            if let Err(e) = self.build() {
                (
                    self.sideband,
                    self.bfo,
                    self.pitch,
                    self.low,
                    self.high,
                    self.cw_width,
                ) = old;
                // The old values built before, so they build again.
                let _ = self.build();
                return Err(SettingError::Config {
                    id,
                    source: Box::new(e),
                });
            }
        }
        Ok(Applied::Live)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::testing::Harness;
    use crate::stream::Data;

    fn run(c: &mut dyn Component, x: &[Complex32], rate: f64) -> Vec<f32> {
        let mut h = Harness::new(c, vec![Format::iq(rate, 0.0, 0.0)]);
        let mut out = Vec::new();
        for chunk in x.chunks(777) {
            h.run(c, |ins| {
                if let Data::Iq(v) = &mut ins[0].data {
                    v.extend_from_slice(chunk)
                }
            });
            if let Data::Real(v) = &h.outs[0].data {
                out.extend_from_slice(v)
            }
        }
        out
    }

    fn amp(x: &[f32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let p = TAU * f * i as f64;
            re += *v as f64 * p.cos();
            im += *v as f64 * p.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    #[test]
    fn fm_returns_the_tone_scaled_by_the_deviation() {
        let rate = 48e3;
        let (fm, dev) = (1000.0, 2500.0);
        let x: Vec<Complex32> = (0..48_000)
            .map(|i| {
                let t = i as f64 / rate;
                let ph = TAU * 3000.0 * t + dev / fm * (TAU * fm * t).sin();
                Complex32::new(ph.cos() as f32, ph.sin() as f32)
            })
            .collect();
        let y = run(&mut FmDemod::new(5000.0).unwrap(), &x, rate);
        let mean = y.iter().map(|&v| v as f64).sum::<f64>() / y.len() as f64;
        assert!(
            (mean - 3000.0 / 5000.0).abs() < 0.01,
            "carrier offset reads {mean}"
        );
        assert!((amp(&y, fm / rate) - 0.5).abs() < 0.01);
    }

    fn am(n: usize, rate: f64, carrier_hz: f64, m: f64) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let t = i as f64 / rate;
                let e = 0.3 * (1.0 + m * (TAU * 1000.0 * t).cos());
                let p = TAU * carrier_hz * t + 0.7;
                Complex32::new((e * p.cos()) as f32, (e * p.sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn am_envelope_and_synchronous_detect_the_tone() {
        let rate = 24e3;
        let x = am(72_000, rate, 0.0, 0.5);
        let y = run(&mut AmDemod::new(AmDetector::Envelope), &x, rate);
        assert!((amp(&y[24_000..], 1000.0 / rate) - 0.5).abs() < 0.02);
        let x = am(72_000, rate, 12.0, 0.5);
        let mut s = AmDemod::new(AmDetector::Synchronous);
        let y = run(&mut s, &x, rate);
        assert!(
            (amp(&y[48_000..], 1000.0 / rate) - 0.5).abs() < 0.03,
            "{}",
            amp(&y[48_000..], 1000.0 / rate)
        );
        assert!(
            (s.carrier_offset_hz() - 12.0).abs() < 1.0,
            "tracked {}",
            s.carrier_offset_hz()
        );
    }

    #[test]
    fn usb_keeps_the_upper_side_and_cw_beats_to_the_pitch() {
        let rate = 12e3;
        let tone = |f: f64| -> Vec<Complex32> {
            (0..24_000)
                .map(|i| Complex32::from_polar(0.5, (TAU * f * i as f64 / rate) as f32))
                .collect()
        };
        let y = run(&mut SsbDemod::new(Sideband::Usb), &tone(1000.0), rate);
        assert!((amp(&y[6000..], 1000.0 / rate) - 0.5).abs() < 0.02);
        let y = run(&mut SsbDemod::new(Sideband::Usb), &tone(-1000.0), rate);
        assert!(amp(&y[6000..], 1000.0 / rate) < 0.01, "lower side leaks");
        let y = run(&mut SsbDemod::new(Sideband::Lsb), &tone(-1000.0), rate);
        assert!((amp(&y[6000..], 1000.0 / rate) - 0.5).abs() < 0.02);
        let y = run(&mut SsbDemod::new(Sideband::Cw), &tone(0.0), rate);
        assert!((amp(&y[6000..], 700.0 / rate) - 0.5).abs() < 0.02);
    }
}
