//! Snow: while there is no picture signal, the detected input (AM: the magnitude, FM: the discriminator) is drawn on a free-running
//! 768 x 576 raster. The pictures are grey, in BGRA pixels.

use super::{last, Phase, Tune};
use crate::settings::SNOW;
use sdrkit_core::{ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Codec, Component, ComponentInfo, Format, Io, Kind, Picture, PixelFormat,
    PixelLayout, Port, RateFrom, Timed,
};

sdrkit_core::settings! {
    /// The settings of the snow raster.
    pub enum SnowSetting {
        /// Snow on or off.
        Snow = SNOW,
    }
}
use std::sync::Arc;

const W: usize = 768;
const H: usize = 576;

/// The snow raster.
pub struct Snow {
    /// Shown at all (the `snow` setting).
    pub on: bool,
    /// FM: the discriminator instead of the magnitude.
    pub fm: bool,
    fs: f64,
    per_px: f64,
    acc: f64,
    acc_n: u32,
    pos: f64,
    px: usize,
    mean: f64,
    var: f64,
    frame: Vec<u8>,
    disc: sdrkit_algo_dsp::fm::Discriminator,
    running: bool,
}

impl Default for Snow {
    fn default() -> Self {
        Snow {
            on: false,
            fm: false,
            fs: 1.0,
            per_px: 1.0,
            acc: 0.0,
            acc_n: 0,
            pos: 0.0,
            px: 0,
            mean: 0.0,
            var: 1e-6,
            frame: vec![0; W * H * 4],
            disc: crate::util::phase_steps(),
            running: false,
        }
    }
}

impl Component for Snow {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.snow",
            vec![Port::iq("in"), Port::typed::<Tune>("control")],
            vec![Port::typed::<Picture>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SnowSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.per_px = self.fs * 64e-6 / W as f64;
        self.pos = 0.0;
        self.px = 0;
        self.acc = 0.0;
        self.acc_n = 0;
        Ok(vec![Format::new(Kind::typed::<Picture>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 1) {
            self.running = t.phase == Phase::Run && !t.snow;
        }
        if !self.on || self.running {
            return;
        }
        let x = io.iq(0);
        let t0 = io.input_format(0).time_of(io.input(0).first());
        for (i, &s) in x.iter().enumerate() {
            let v = if self.fm {
                self.disc.process_sample(s) as f64
            } else {
                s.norm() as f64
            };
            self.acc += v;
            self.acc_n += 1;
            self.pos += 1.0;
            let m = self.acc / self.acc_n.max(1) as f64;
            if self.pos >= self.per_px {
                self.acc = 0.0;
                self.acc_n = 0;
            }
            while self.pos >= self.per_px {
                self.pos -= self.per_px;
                let w = 2e-5;
                self.mean += w * (m - self.mean);
                self.var += w * ((m - self.mean).powi(2) - self.var);
                let g = (128.0 + 40.0 * (m - self.mean) / self.var.max(1e-30).sqrt())
                    .clamp(0.0, 255.0) as u8;
                let p = &mut self.frame[self.px * 4..self.px * 4 + 4];
                p[0] = g;
                p[1] = g;
                p[2] = g;
                p[3] = 255;
                self.px += 1;
                if self.px == W * H {
                    self.px = 0;
                    let pic = Picture {
                        codec: Codec::Pixels(PixelFormat::Bgra),
                        layout: Some(PixelLayout::packed(PixelFormat::Bgra, W as u32, H as u32)),
                        name: None,
                        id: Picture::WHOLE,
                        state: sdrkit_core::PictureState::Finished,
                        data: Arc::from(&self.frame[..]),
                    };
                    io.items_out::<Picture>(0).push(Timed {
                        time: t0 + i as f64 / self.fs,
                        value: pic,
                    });
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SnowSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SnowSetting::Snow => self.on = SNOW.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SnowSetting::Snow, Value::Bool(self.on))])
    }
}
