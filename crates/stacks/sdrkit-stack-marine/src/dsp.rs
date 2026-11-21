//! The front end of the marine HF, VHF DSC and ATIS receivers: the channel moved to 0 Hz and resampled to exactly the decoders'
//! rate, from any input rate, below it too.

use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Port, RateFrom};

const OFFSET: Setting<field::Hz> =
    Setting::new("offset", "Offset", "Hz", 0.0).range(-1e10, 1e10, 0.0);

sdrkit_core::settings! {
    /// The settings of [`FixedRateFrontEnd`].
    pub enum FrontEndSetting {
        /// The signal's offset from the tuned frequency.
        Offset = OFFSET,
    }
}

/// Front end to a fixed rate (`marine.frontend`): the signal at `offset` Hz from the stream's tuned frequency (plus the stream's own
/// signal offset) moved to 0 Hz and resampled to exactly `rate` (sdrkit-algo-dsp `Channelizer`). Below the output rate the input is only
/// shifted and resampled up.
pub struct FixedRateFrontEnd {
    offset: f64,
    rate: f64,
    width: f64,
    chan: Option<Channelizer>,
    up: Option<(
        sdrkit_algo_dsp::nco::Nco,
        sdrkit_algo_dsp::resample::FractionalResampler<Complex32>,
    )>,
    mixed: Vec<Complex32>,
}

impl FixedRateFrontEnd {
    /// The signal at `offset_hz` from the tuned frequency, `width_hz` wide, to `rate_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the schema refuses `offset_hz`.
    pub fn new(offset_hz: f64, width_hz: f64, rate_hz: f64) -> Result<Self, SettingError> {
        FrontEndSetting::Offset.check(&Value::Hz(offset_hz))?;
        Ok(FixedRateFrontEnd {
            offset: offset_hz,
            rate: rate_hz,
            width: width_hz,
            chan: None,
            up: None,
            mixed: Vec::new(),
        })
    }
}

impl Default for FixedRateFrontEnd {
    /// The dial at 0 Hz, 10 kHz wide, to 24 kHz.
    fn default() -> Self {
        FixedRateFrontEnd {
            offset: 0.0,
            rate: 24e3,
            width: 10e3,
            chan: None,
            up: None,
            mixed: Vec::new(),
        }
    }
}

impl FixedRateFrontEnd {
    /// Appends the output for `x` (configured first).
    pub fn process_slice(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        if let Some(c) = &mut self.chan {
            c.process(x, out);
        } else if let Some((nco, res)) = &mut self.up {
            self.mixed.clear();
            self.mixed.extend_from_slice(x);
            nco.mix_in_place(&mut self.mixed);
            res.process(&self.mixed, out);
        }
    }
}

impl Component for FixedRateFrontEnd {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.frontend",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(self.rate))],
        )
        .with_settings(FrontEndSetting::schema())
        .manual_tags()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let sig = f.signal_offset_hz() + self.offset;
        self.chan = None;
        self.up = None;
        if f.rate >= self.rate * 1.0001 {
            let mut cfg =
                ChannelConfig::new(f.rate, sig, self.width.min(self.rate / 1.25), self.rate);
            cfg.exact_rate = true;
            cfg.atten_db = 70.0;
            self.chan = Some(Channelizer::new(cfg)?);
        } else {
            let res = sdrkit_algo_dsp::resample::FractionalResampler::new(
                f.rate,
                self.rate,
                0.45 * f.rate.min(self.rate),
                0.5 * f.rate.min(self.rate),
                70.0,
            )?;
            self.up = Some((sdrkit_algo_dsp::nco::Nco::new(f.rate, -sig), res));
        }
        let centre = f.tuned_hz + self.offset;
        Ok(vec![Format {
            rate: self.rate,
            centre_hz: centre,
            tuned_hz: centre,
            bandwidth_hz: self.width,
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let out = io.iq_out(0);
        self.process_slice(x, out);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontEndSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontEndSetting::Offset => self.offset = OFFSET.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(FrontEndSetting::Offset, Value::Hz(self.offset))])
    }
}
