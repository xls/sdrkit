//! The vision channel: a mixer puts the middle of the video band at 0 Hz, a low-pass filter decimates to the video rate, and a
//! second mixer brings the vision carrier to 0 Hz. The detector follows in the video decoder ([`super::sync`]).

use super::{last, Tune, VisionReport};
use crate::settings::{SystemChoice, SYSTEM};
use crate::systems::{System, SystemId};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::{design_beta, kaiser_len, windowed_sinc};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::{
    Choice, Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};

sdrkit_core::settings! {
    /// The settings of the vision channel.
    pub enum VisionSetting {
        /// The system: E keeps 22 MHz of input.
        System = SYSTEM,
    }
}

/// The decimation for an input rate.
pub fn decimation(fs: f64, system_e: bool) -> usize {
    let d = if system_e {
        (fs / 22e6).floor()
    } else {
        (fs / 10e6).floor()
    };
    (d as usize).max(1)
}

/// The vision channel.
pub struct VisionChannel {
    fs: f64,
    d: usize,
    system: SystemChoice,
    system_e: bool,
    mix1: Nco,
    mix2: Nco,
    filt: FirDecimator<Complex32>,
    noise_gain: f64,
    scratch: Vec<Complex32>,
    out: Vec<Complex32>,
    key: (u64, u64),
    carrier: f64,
    centre: f64,
    epoch: u32,
}

impl Default for VisionChannel {
    fn default() -> Self {
        VisionChannel {
            fs: 1.0,
            d: 1,
            system: SystemChoice::Auto,
            system_e: false,
            mix1: Nco::new(1.0, 0.0),
            mix2: Nco::new(1.0, 0.0),
            filt: FirDecimator::new(&[1.0], 1),
            noise_gain: 1.0,
            scratch: Vec::new(),
            out: Vec::new(),
            key: (0, 0),
            carrier: 0.0,
            centre: 0.0,
            epoch: u32::MAX,
        }
    }
}

/// The band edges (U above, L below the carrier) for a spacing and a system.
pub fn band(spacing_hz: f64, system: Option<SystemId>, video_rate: f64) -> (f64, f64) {
    let spacing = if spacing_hz.abs() > 1e5 {
        spacing_hz.abs()
    } else {
        5.5e6
    };
    let vest = match system {
        Some(s) => System::get(s).vestige_mhz * 1e6,
        None => {
            if (spacing - 6.0e6).abs() < 0.2e6 {
                1.25e6
            } else if spacing > 8e6 {
                2.0e6
            } else {
                0.75e6
            }
        }
    };
    let u = (spacing - 0.55e6).min(video_rate / 2.0 - 0.1e6);
    let l = if vest > 1.3e6 { vest + 0.5e6 } else { 1.25e6 };
    (u, l)
}

impl VisionChannel {
    /// Designs the channel filter for the band from `u` above to `l` below the carrier: a Kaiser windowed sinc of at most
    /// 481 taps (`sdrkit_algo_dsp::fir::lowpass` has no cap on the length).
    fn design(&mut self, u: f64, l: f64) {
        let half = (u + l) / 2.0;
        let stop = (half + 0.4e6).min(self.fs / 2.0);
        let mut n = kaiser_len(42.0, ((stop - half) / self.fs).clamp(1e-3, 0.49))
            .expect("the filter settings are valid");
        n = n.min(481);
        let h = windowed_sinc(
            n,
            ((half + stop) / 2.0 / self.fs).min(0.5),
            design_beta(42.0),
        )
        .expect("the filter settings are valid");
        let taps: Vec<f32> = h.iter().map(|&v| v as f32).collect();
        self.noise_gain = taps.iter().map(|&t| (t as f64) * (t as f64)).sum();
        self.filt = FirDecimator::new(&taps, self.d);
        self.centre = (u - l) / 2.0;
    }
    fn retune(&mut self, carrier: f64) {
        self.carrier = carrier;
        self.mix1.set_freq(-(carrier + self.centre));
        self.mix2.set_freq(self.centre);
    }
}

impl Component for VisionChannel {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.vision",
            vec![Port::iq("in"), Port::typed::<Tune>("control")],
            vec![
                Port::iq("out"),
                Port::typed::<VisionReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(VisionSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.d = decimation(self.fs, self.system_e);
        let fv = self.fs / self.d as f64;
        self.mix1 = Nco::new(self.fs, 0.0);
        self.mix2 = Nco::new(fv, 0.0);
        let (u, l) = band(if self.system_e { 11.15e6 } else { 0.0 }, None, fv);
        self.design(u, l);
        self.key = (0, 0);
        self.epoch = u32::MAX;
        self.retune(-2.75e6);
        let mut f = inputs[0].with_rate(fv);
        f.bandwidth_hz = fv;
        Ok(vec![f, Format::new(Kind::typed::<VisionReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let fv = self.fs / self.d as f64;
        if let Some(t) = last::<Tune>(io, 1) {
            let (u, l) = band(t.spacing_hz, t.system, fv);
            let key = (u.to_bits(), l.to_bits());
            if key != self.key {
                self.key = key;
                self.design(u, l);
                self.retune(t.carrier_hz);
            }
            if t.carrier_hz != self.carrier {
                self.retune(t.carrier_hz);
            }
            if t.epoch != self.epoch {
                self.epoch = t.epoch;
                self.filt.reset();
            }
        }
        let x = io.iq(0);
        self.scratch.clear();
        self.scratch.extend_from_slice(x);
        self.mix1.mix_in_place(&mut self.scratch);
        self.out.clear();
        self.filt.process(&self.scratch, &mut self.out);
        self.mix2.mix_in_place(&mut self.out);
        io.iq_out(0).extend_from_slice(&self.out);
        if io.tick() {
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<VisionReport>(1).push(Timed {
                time: t,
                value: VisionReport {
                    rate_hz: fv,
                    noise_gain: self.noise_gain,
                    input_hz: self.fs,
                    vsb: 0.0,
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match VisionSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            VisionSetting::System => {
                self.system = SYSTEM.read(id, v)?;
                let e = self.system == SystemChoice::E;
                if e == self.system_e {
                    return Ok(Applied::Live);
                }
                self.system_e = e;
            }
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(VisionSetting::System, Value::Choice(self.system.index()))])
    }
}
