//! The HF channel front ends of this crate: [`add_front`], the graph's `dsp.shift_resample` adapter at 24 000 samples/s, and
//! the beacon stack's own front end: the dial mixed to 0 Hz, a 4th-order integer CIC by floor(rate / 96 kHz), then a polyphase
//! resampler to exactly 24 000 samples/s.

use sdrkit_algo_dsp::cic::Cic;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::components::ShiftResample;
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, GraphBuilder, Io, Port, RateFrom, TagKind,
};

/// The output rate.
pub const RATE_24K: f64 = 24_000.0;
/// The CIC's full scale: peaks up to 8 are not clipped.
const CIC_FULL_SCALE: f32 = 8.0;

/// The extra shift of the channel.
pub const SHIFT_HZ: Setting<field::Hz> =
    Setting::new("shift_hz", "Extra shift", "Hz", 0.0).range(-1e7, 1e7, 0.0);
/// Listen to the beacon on this frequency: the dial goes 700 Hz below it. 0 is off.
pub const BEACON_KHZ: Setting<field::F64> =
    Setting::new("beacon_khz", "Listen to the beacon on", "kHz", 0.0).range(0.0, 1e6, 0.0);

sdrkit_core::settings! {
    /// The settings of [`HfChannelFrontEnd`].
    pub enum FrontSetting {
        /// Extra shift.
        ShiftHz = SHIFT_HZ,
        /// Listen to the beacon on.
        BeaconKhz = BEACON_KHZ,
    }
}

/// The highest input rate.
const MAX_RATE: f64 = 1e9;

/// The front end as a plain object.
pub struct HfFront {
    nco: Nco,
    cic: Option<Cic<Complex32>>,
    /// `None` when the input is already at 24 kHz.
    resampler: Option<FractionalResampler<Complex32>>,
    offset_hz: f64,
    shift_hz: f64,
    mixed: Vec<Complex32>,
    decimated: Vec<Complex32>,
}

impl HfFront {
    /// A front end for `rate`, with the dial at `offset_hz` in the input.
    ///
    /// # Errors
    ///
    /// The error of the CIC or the resampler if it refuses the rate.
    pub fn new(rate: f64, offset_hz: f64) -> Result<HfFront, ConfigError> {
        let r = rate.round().max(1.0);
        let d = if r >= 96_000.0 {
            ((r / 96_000.0).floor() as usize).clamp(1, 4096)
        } else {
            1
        };
        let r1 = r / d as f64;
        let cic = if d > 1 {
            Some(Cic::new(4, d).and_then(|c| c.with_full_scale(CIC_FULL_SCALE))?)
        } else {
            None
        };
        // Flat to 8 kHz, and 80 dB down from 12 kHz: an alias of the band edge stays outside the pass band.
        let resampler = if r1 == RATE_24K {
            None
        } else {
            Some(FractionalResampler::new(
                r1, RATE_24K, 8_000.0, 12_000.0, 80.0,
            )?)
        };
        Ok(HfFront {
            nco: Nco::new(r, -offset_hz),
            cic,
            resampler,
            offset_hz,
            shift_hz: 0.0,
            mixed: Vec::new(),
            decimated: Vec::new(),
        })
    }
    fn retarget(&mut self) {
        self.nco.set_freq(-(self.offset_hz + self.shift_hz));
    }
    /// A new dial offset, from the next sample (filters kept).
    pub fn set_offset(&mut self, offset_hz: f64) {
        self.offset_hz = offset_hz;
        self.retarget();
    }
    /// An extra shift (the beacon Listen puts the channel 700 Hz below a beacon).
    pub fn set_shift(&mut self, shift_hz: f64) {
        self.shift_hz = shift_hz;
        self.retarget();
    }
    /// Processes input samples, appending 24 kHz output.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        self.mixed.clear();
        self.mixed.extend_from_slice(input);
        self.nco.mix_in_place(&mut self.mixed);
        let r1 = match self.cic.as_mut() {
            Some(c) => {
                self.decimated.clear();
                c.process(&self.mixed, &mut self.decimated);
                &self.decimated
            }
            None => &self.mixed,
        };
        match self.resampler.as_mut() {
            Some(r) => r.process(r1, out),
            None => out.extend_from_slice(r1),
        }
    }
}

/// Adds the front of a stack of this crate under `id`: the graph's `dsp.shift_resample` adapter, which follows the tuned
/// frequency to 0 Hz and brings the samples to [`RATE_24K`].
pub fn add_front(b: GraphBuilder, id: &str) -> GraphBuilder {
    b.add(id, ShiftResample::default())
        .set(&format!("{id}.follow"), Value::Bool(true))
        .set(&format!("{id}.rate"), Value::Hz(RATE_24K))
}

/// The front end of the beacon stack as a component: `Iq` in, `Iq` at 24 kHz out with the dial at 0 Hz, or 700 Hz below the
/// beacon of [`BEACON_KHZ`]. The beacon is an absolute frequency, which the `follow` of `dsp.shift_resample` cannot hold.
pub struct HfChannelFrontEnd {
    front: Option<HfFront>,
    rate: f64,
    shift_hz: f64,
    /// Listen to a beacon: the dial 700 Hz below it (kHz; 0 off).
    beacon_khz: f64,
    centre_hz: f64,
    tuned_hz: f64,
    scratch: Vec<Complex32>,
}

impl HfChannelFrontEnd {
    fn dial_offset(&self) -> f64 {
        if self.beacon_khz > 0.0 {
            self.beacon_khz * 1e3 - 700.0 - self.centre_hz
        } else {
            self.tuned_hz - self.centre_hz
        }
    }
}

impl Default for HfChannelFrontEnd {
    fn default() -> Self {
        HfChannelFrontEnd {
            front: None,
            rate: 0.0,
            shift_hz: 0.0,
            beacon_khz: 0.0,
            centre_hz: 0.0,
            tuned_hz: 0.0,
            scratch: Vec::new(),
        }
    }
}

impl Component for HfChannelFrontEnd {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hf_front",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(RATE_24K))],
        )
        .with_settings(FrontSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if !(f.rate.is_finite() && f.rate >= RATE_24K - 1.0) {
            return Err(ConfigError::RateTooLow {
                need: RATE_24K,
                got: if f.rate.is_finite() { f.rate } else { 0.0 },
            });
        }
        if f.rate > MAX_RATE {
            return Err(ConfigError::RateTooHigh {
                max: MAX_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
        let mut fe = HfFront::new(f.rate, self.dial_offset())?;
        fe.set_shift(self.shift_hz);
        self.front = Some(fe);
        let mut o = Format::iq(RATE_24K, f.tuned_hz + self.shift_hz, f.tuned_hz);
        o.bandwidth_hz = 18_000.0;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(fe) = self.front.as_mut() else {
            return;
        };
        let input = io.iq(0);
        let buf = io.input(0);
        let first = buf.first();
        let mut out = std::mem::take(&mut self.scratch);
        out.clear();
        let mut start = 0usize;
        for t in buf.tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                let at = (t.at.saturating_sub(first) as usize).min(input.len());
                fe.process(&input[start..at], &mut out);
                start = at;
                self.centre_hz = centre_hz;
                self.tuned_hz = tuned_hz;
                let off = if self.beacon_khz > 0.0 {
                    self.beacon_khz * 1e3 - 700.0 - centre_hz
                } else {
                    tuned_hz - centre_hz
                };
                fe.set_offset(off);
            }
        }
        fe.process(&input[start..], &mut out);
        io.iq_out(0).extend_from_slice(&out);
        self.scratch = out;
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::ShiftHz => {
                self.shift_hz = SHIFT_HZ.read(id, value)?;
                if let Some(f) = self.front.as_mut() {
                    f.set_shift(self.shift_hz);
                }
            }
            FrontSetting::BeaconKhz => {
                self.beacon_khz = BEACON_KHZ.read(id, value)?;
                let off = self.dial_offset();
                if let Some(f) = self.front.as_mut() {
                    f.set_offset(off);
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (FrontSetting::ShiftHz, Value::Hz(self.shift_hz)),
            (FrontSetting::BeaconKhz, Value::F64(self.beacon_khz)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn tone(n: usize, rate: f64, f: f64) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = TAU * f * i as f64 / rate;
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect()
    }

    fn power_at(x: &[Complex32], rate: f64, f: f64) -> f64 {
        let mut acc = num_complex::Complex64::new(0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let p = -TAU * f * i as f64 / rate;
            acc += num_complex::Complex64::new(v.re as f64, v.im as f64)
                * num_complex::Complex64::new(p.cos(), p.sin());
        }
        (acc / x.len() as f64).norm_sqr()
    }

    #[test]
    fn rates_and_gain() {
        for &rate in &[24_000.0, 250_000.0, 2e6] {
            let mut fe = HfFront::new(rate, -20_000.0).unwrap();
            // A tone 1 kHz above the dial.
            let x = tone((rate * 0.5) as usize, rate, -20_000.0 + 1_000.0);
            let mut out = Vec::new();
            fe.process(&x, &mut out);
            assert!(
                (out.len() as f64 - 12_000.0).abs() <= 2.0,
                "{rate}: {}",
                out.len()
            );
            let p = power_at(&out[2000..], RATE_24K, 1_000.0);
            assert!((p - 1.0).abs() < 0.02, "{rate}: gain {p}");
            // A tone 15 kHz from the dial is stopped.
            if rate > 30e3 {
                let x = tone((rate * 0.2) as usize, rate, -20_000.0 + 15_000.0);
                let mut out = Vec::new();
                fe.process(&x, &mut out);
                let p: f64 = out[1000..].iter().map(|v| v.norm_sqr() as f64).sum::<f64>()
                    / (out.len() - 1000) as f64;
                assert!(p < 1e-3, "{rate}: leak {p}");
            }
        }
    }

    #[test]
    fn cut_independent() {
        let rate = 250_000.0;
        let x = tone(50_000, rate, -19_000.0);
        let mut a = HfFront::new(rate, -20_000.0).unwrap();
        let mut oa = Vec::new();
        a.process(&x, &mut oa);
        let mut b = HfFront::new(rate, -20_000.0).unwrap();
        let mut ob = Vec::new();
        for c in x.chunks(777) {
            b.process(c, &mut ob);
        }
        assert_eq!(oa, ob);
    }
}
