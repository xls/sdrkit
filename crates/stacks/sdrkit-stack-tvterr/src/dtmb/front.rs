//! `dtmb.front`: the input level, the coarse carrier offset from the occupied band, and the resampling to one sample per symbol
//! with the clock correction the tracker sends.

use super::{Width, ROLLOFF, WIDTH};
use crate::band::{resampler, strongest_box, AvgSpectrum, BandFft};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    apply_commands, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port,
    RateFrom, Timed,
};

/// The largest clock correction the tracker can ask for, ppm.
pub const MAX_CLOCK_PPM: f64 = 500.0;

/// The clock correction, ppm (positive: the radio's clock is fast). `dtmb.track` sets it.
pub const CLOCK_PPM: Setting<field::F64> =
    Setting::new("clock_ppm", "Clock correction", "ppm", 0.0).range(
        -MAX_CLOCK_PPM,
        MAX_CLOCK_PPM,
        0.0,
    );

/// I and Q change places (a radio that delivers the spectrum mirrored). `dtmb.track` sets it when a clean header gives no
/// system information.
pub const IQ_SWAP: Setting<field::Bool> = Setting::new("iq_swap", "Swap I and Q", "", false);

sdrkit_core::settings! {
    /// The settings of `dtmb.front`.
    pub enum FrontSetting {
        /// The channel width.
        Width = WIDTH,
        /// The clock correction.
        ClockPpm = CLOCK_PPM,
        /// I and Q change places.
        IqSwap = IQ_SWAP,
    }
}

/// What the front end measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrontState {
    /// Mean input power, dBFS.
    pub level_dbfs: f64,
    /// The coarse carrier offset removed before resampling, Hz.
    pub coarse_hz: f64,
    /// The clock correction in use, ppm (positive: the radio's clock is fast).
    pub clock_ppm: f64,
    /// The input rate is enough for the width.
    pub rate_ok: bool,
}

impl Item for FrontState {
    const KIND: &'static str = "dtmb.front_state";
}

/// `dtmb.front`. Input `in`: samples. Input `control`: commands from the tracker (`clock_ppm`, `iq_swap`). Output `out`: symbols at the
/// symbol rate. Output `state`: [`FrontState`] after each call.
pub struct Front {
    width: Width,
    rate: f64,
    fsym: f64,
    spec: AvgSpectrum,
    spec_done: bool,
    nco: Nco,
    rs: Option<FractionalResampler<Complex32>>,
    clock_ppm: f64,
    iq_swap: bool,
    swapped: Vec<Complex32>,
    tmp: Vec<Complex32>,
    state: FrontState,
}

impl Default for Front {
    fn default() -> Self {
        Front::new(Width::W8)
    }
}

impl Front {
    /// For a channel width.
    pub fn new(width: Width) -> Front {
        Front {
            width,
            rate: 10e6,
            fsym: width.symbol_rate(),
            spec: AvgSpectrum::new(BandFft::N4096),
            spec_done: false,
            nco: Nco::new(10e6, 0.0),
            rs: None,
            clock_ppm: 0.0,
            iq_swap: false,
            swapped: Vec::new(),
            tmp: Vec::new(),
            state: FrontState::default(),
        }
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dtmb.front",
            vec![Port::iq("in"), Port::command("control")],
            vec![
                Port::iq("out"),
                Port::typed::<FrontState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (FrontSetting::Width, Value::Choice(self.width.index())),
            (FrontSetting::ClockPpm, Value::F64(self.clock_ppm)),
            (FrontSetting::IqSwap, Value::Bool(self.iq_swap)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.fsym = self.width.symbol_rate();
        // The receive filter: flat over the occupied band (roll-off 0.05), down where the next symbol-rate image begins. A
        // rate that leaves less room than that above the band would need a filter many times longer: the filter is then
        // flat to where the roll-off begins and down where the image of the band's own edge begins, and the equaliser
        // takes the slope over the roll-off as part of the channel.
        let half = 0.5 * self.fsym;
        let edge = half * (1.0 + ROLLOFF);
        self.rs = Some(if self.rate - edge < 1.2 * half {
            FractionalResampler::new(
                self.rate,
                self.fsym,
                half * (1.0 - ROLLOFF),
                self.rate - edge,
                80.0,
            )?
        } else {
            resampler(self.rate, self.fsym, edge, 1.2 * half)?
        });
        self.spec.clear();
        self.spec_done = false;
        self.nco = Nco::new(self.rate, 0.0);
        self.clock_ppm = 0.0;
        self.iq_swap = false;
        self.state = FrontState {
            rate_ok: self.rate >= self.width.hz(),
            ..Default::default()
        };
        let mut out = inputs[0].clone();
        out.rate = self.fsym;
        Ok(vec![out, Format::new(Kind::typed::<FrontState>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        // A clock correction outside the range is refused by the schema and the one in use stays.
        let _ = apply_commands(self, io, 1);
        let mut x = io.iq(0);
        if x.is_empty() {
            return;
        }
        if self.iq_swap {
            self.swapped.clear();
            self.swapped
                .extend(x.iter().map(|s| Complex32::new(s.im, s.re)));
            x = &self.swapped;
        }
        let time = io.input(0).first() as f64 / self.rate.max(1.0);
        let p: f64 = x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
        self.state.level_dbfs = 10.0 * p.max(1e-20).log10();
        // The coarse carrier offset from the occupied band, once. The samples of the measurement are not used.
        if !self.spec_done {
            self.spec.add_all(x);
            if self.spec.frames >= 32 {
                let m = self.spec.mean();
                let n = m.len();
                let bins = (self.fsym / self.rate * n as f64) as usize;
                if let Some((s, _, _)) = strongest_box(&m, bins.min(n)) {
                    let centre =
                        (s as f64 + bins as f64 / 2.0 - n as f64 / 2.0) * self.rate / n as f64;
                    self.state.coarse_hz = centre;
                    self.nco.set_freq(-centre);
                }
                self.spec_done = true;
            }
        } else {
            self.tmp.resize(x.len(), Complex32::new(0.0, 0.0));
            self.nco.mix(x, &mut self.tmp);
            if let Some(rs) = &mut self.rs {
                rs.process(&self.tmp, io.iq_out(0));
            }
        }
        self.state.clock_ppm = self.clock_ppm;
        io.items_out::<FrontState>(1).push(Timed {
            time,
            value: self.state,
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Width => {
                self.width = WIDTH.read(id, v)?;
                Ok(Applied::Reformat)
            }
            FrontSetting::ClockPpm => {
                self.clock_ppm = CLOCK_PPM.read(id, v)?;
                if let Some(rs) = &mut self.rs {
                    rs.set_correction(1.0 + self.clock_ppm * 1e-6);
                }
                Ok(Applied::Live)
            }
            FrontSetting::IqSwap => {
                // The mirrored band has another centre: the coarse offset is measured again.
                self.iq_swap = IQ_SWAP.read(id, v)?;
                self.spec.clear();
                self.spec_done = false;
                Ok(Applied::Live)
            }
        }
    }
}
