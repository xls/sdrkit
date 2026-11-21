//! The OFDM demodulator component (`dab.ofdm`): frame sync, tracking and soft bits.
//!
//! Standard: EN 300 401 clause 14.

use super::front::Recentred;
use super::items::{CifSoft, FicSoft, NullItem, OfdmStatusItem};
use crate::ofdm::{Ofdm, OfdmOut, OfdmSettings};
use crate::settings::{DetectionChoice, DETECTION, TII};
use crate::tables;
use sdrkit_core::Values;
use sdrkit_core::{
    Choice, Complex32 as C, ConfigError, SettingError, SettingId, SettingSet, Value,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RateRule,
    TagKind, Timed,
};

sdrkit_core::settings! {
    /// The settings of [`OfdmComponent`].
    pub enum OfdmSetting {
        /// The detection.
        Detection = DETECTION,
        /// Transmitter identification.
        Tii = TII,
    }
}

/// The rate of the cells output: the carriers of one symbol shown as a constellation.
const CELLS_RATE: f64 = 16_000.0;

/// The demodulator component.
pub struct OfdmComponent {
    ofdm: Ofdm,
    settings: OfdmSettings,
    out: OfdmOut,
    centring_hz: f64,
}

impl Default for OfdmComponent {
    fn default() -> Self {
        let settings = OfdmSettings::default();
        OfdmComponent {
            ofdm: Ofdm::new(settings),
            settings,
            out: OfdmOut::default(),
            centring_hz: 0.0,
        }
    }
}

impl OfdmComponent {
    /// The centring shift seen, Hz.
    pub fn centring_hz(&self) -> f64 {
        self.centring_hz
    }
}

impl Component for OfdmComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.ofdm",
            vec![Port::iq("in").rate(RateRule::Exact(tables::RATE))],
            vec![
                Port::typed::<FicSoft>("fic").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<CifSoft>("cif").rate_from(RateFrom::Fixed(0.0)),
                Port::new("cells", Kind::Symbols).rate_from(RateFrom::Fixed(CELLS_RATE)),
                Port::typed::<NullItem>("null").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<OfdmStatusItem>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(OfdmSetting::schema())
        .manual_tags()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if (f.rate - tables::RATE).abs() > 1.0 {
            return Err(ConfigError::WrongRate {
                need: tables::RATE,
                got: f.rate,
            });
        }
        self.ofdm = Ofdm::new(self.settings);
        self.centring_hz = 0.0;
        // Every output keeps the frequency, so the events made from it carry it.
        let typed = |k: Kind| {
            let mut x = Format::new(k, 0.0);
            x.centre_hz = f.centre_hz;
            x.tuned_hz = f.tuned_hz;
            x
        };
        let mut cells = Format::new(Kind::Symbols, CELLS_RATE);
        cells.centre_hz = f.centre_hz;
        Ok(vec![
            typed(Kind::typed::<FicSoft>()),
            typed(Kind::typed::<CifSoft>()),
            cells,
            typed(Kind::typed::<NullItem>()),
            typed(Kind::typed::<OfdmStatusItem>()),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let sync = io.section("sync_fft");
        // Cut at the centring tags: the samples before a tag are dropped from the search.
        let mut at = 0usize;
        for t in io.input(0).tags() {
            let TagKind::Typed(tag) = &t.kind else {
                continue;
            };
            let Some(r) = tag.get::<Recentred>() else {
                continue;
            };
            let (pos, shift) = ((t.at - first) as usize, r.shift_hz);
            let pos = pos.min(x.len());
            self.ofdm.push(&x[at..pos], &mut self.out);
            self.ofdm.restart_search();
            self.centring_hz = shift;
            at = pos;
        }
        self.ofdm.push(&x[at..], &mut self.out);
        drop(sync);
        let rate = tables::RATE;
        for f in self.out.frames.drain(..) {
            let t = f.start.max(0) as f64 / rate;
            io.items_out::<FicSoft>(0).push(Timed {
                time: t,
                value: FicSoft {
                    frame: f.frame,
                    start: f.start,
                    faded: f.faded,
                    soft: f.soft.clone(),
                },
            });
            for c in 0..4u8 {
                io.items_out::<CifSoft>(1).push(Timed {
                    time: t,
                    value: CifSoft {
                        frame: f.frame,
                        start: f.start,
                        index: c,
                        soft: f.soft.clone(),
                    },
                });
            }
        }
        for n in self.out.nulls.drain(..) {
            io.items_out::<NullItem>(3).push(Timed {
                time: 0.0,
                value: NullItem(n),
            });
        }
        let centring = self.centring_hz;
        let mut last_cells = None;
        for mut s in self.out.stats.drain(..) {
            s.cfo_hz += centring;
            if let Some(c) = &s.constellation {
                last_cells = Some(c.clone());
            }
            let t = s.time_s;
            io.items_out::<OfdmStatusItem>(4).push(Timed {
                time: t,
                value: OfdmStatusItem(s),
            });
        }
        if let Some(c) = last_cells {
            let out: &mut Vec<C> = io.samples_out(2);
            out.extend_from_slice(&c);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match OfdmSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OfdmSetting::Detection => {
                self.settings.coherent = DETECTION.read(id, v)? == DetectionChoice::Auto
            }
            OfdmSetting::Tii => self.settings.tii = TII.read(id, v)?,
        }
        self.ofdm.set_settings(self.settings);
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let d = if self.settings.coherent {
            DetectionChoice::Auto
        } else {
            DetectionChoice::Differential
        };
        values_of([
            (OfdmSetting::Detection, Value::Choice(d.index())),
            (OfdmSetting::Tii, Value::Bool(self.settings.tii)),
        ])
    }
}
