//! The sound stage of the STL stack (`stl.sound`): at 48 kHz, in place, the gain that makes the link's full deviation full
//! scale, silence until the FM demodulator reports a carrier and while it sees none, and the peak and RMS level of the
//! sound every 0.5 s.

use super::link::REPORT_TICKS;
use super::settings::DEVIATION;
use crate::audio::AUDIO_RATE;
use crate::fmdemod::DemodReport;
use crate::generator::FULL_DEVIATION_HZ;
use sdrkit_core::{ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, RateRule,
    Timed,
};

/// The sound level over a report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoundReport {
    /// The peak of both channels, dBFS (`None` for silence).
    pub peak_dbfs: Option<f64>,
    /// The RMS of both channels, dBFS (`None` for silence).
    pub rms_dbfs: Option<f64>,
    /// The deviation of full modulation in force, Hz.
    pub deviation_hz: f64,
}
impl Item for SoundReport {
    const KIND: &'static str = "stl.sound_report";
}

sdrkit_core::settings! {
    /// The settings of [`SoundComponent`].
    pub enum SoundSetting {
        /// The deviation of full modulation.
        Deviation = DEVIATION,
    }
}

/// The sound stage (`stl.sound`). Its input takes audio at 48 kHz, so the graph changes the rate in front of it.
pub struct SoundComponent {
    deviation_hz: f64,
    carrier: bool,
    ticks: u32,
    peak: f32,
    sum2: f64,
    n: u64,
}

impl Default for SoundComponent {
    fn default() -> Self {
        SoundComponent {
            deviation_hz: DEVIATION.default,
            carrier: false,
            ticks: 0,
            peak: 0.0,
            sum2: 0.0,
            n: 0,
        }
    }
}

impl Component for SoundComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "stl.sound",
            vec![
                Port::audio("in").rate(RateRule::Exact(AUDIO_RATE)),
                Port::typed::<DemodReport>("demod"),
            ],
            vec![
                Port::audio("out").same_format_as(0),
                Port::typed::<SoundReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .in_place(0, 0)
        .with_settings(SoundSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        (self.carrier, self.ticks, self.peak, self.sum2, self.n) = (false, 0, 0.0, 0.0, 0);
        Ok(vec![
            inputs[0].clone(),
            Format::new(Kind::typed::<SoundReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(d) = io.items::<DemodReport>(1).last() {
            self.carrier = d.value.carrier;
        }
        let gain = if self.carrier {
            (FULL_DEVIATION_HZ / self.deviation_hz) as f32
        } else {
            0.0
        };
        let buf = io.audio_pass_through(0, 0);
        for v in buf.iter_mut() {
            *v *= gain;
            self.peak = self.peak.max(v.abs());
            self.sum2 += f64::from(*v) * f64::from(*v);
        }
        self.n += buf.len() as u64;
        if !io.tick() {
            return;
        }
        self.ticks += 1;
        if !self.ticks.is_multiple_of(REPORT_TICKS) {
            return;
        }
        let db = |x: f64| (x > 0.0).then(|| 20.0 * x.log10());
        let rms = if self.n > 0 {
            (self.sum2 / self.n as f64).sqrt()
        } else {
            0.0
        };
        let r = SoundReport {
            peak_dbfs: db(f64::from(self.peak)),
            rms_dbfs: db(rms),
            deviation_hz: self.deviation_hz,
        };
        (self.peak, self.sum2, self.n) = (0.0, 0.0, 0);
        io.items_out::<SoundReport>(1).push(Timed {
            time: 0.0,
            value: r,
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SoundSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SoundSetting::Deviation => self.deviation_hz = DEVIATION.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SoundSetting::Deviation, Value::Hz(self.deviation_hz))])
    }
}
