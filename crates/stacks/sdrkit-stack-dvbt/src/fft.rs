//! `dvbt.fft`: the FFT of every window, the integer carrier offset from the continual pilots, mirrored-spectrum detection and
//! carrier re-indexing. K carriers for each symbol leave as [`Carriers`] items.
//!
//! Standard: ETSI EN 300 744 clause 4.4.

use crate::coding::continual;
use crate::items::{Carriers, SymbolBlock};
use crate::params::Mode;
use crate::typed::FftFields;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, ConfigError, Results, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};
use sdrkit_graph::{Kept, SlicePool};
use std::f64::consts::PI;
use std::sync::Arc;

struct ModeState {
    n: usize,
    k: usize,
    fft: Fft,
    pilots: Vec<i64>,
}

/// `dvbt.fft`.
pub struct DvbtFft {
    link: Option<Arc<crate::items::Link>>,
    rate: f64,
    modes: Vec<ModeState>,
    work: Vec<Complex32>,
    prev: Vec<Complex32>,
    prev_count: Option<u64>,
    shift: Option<i32>,
    mirrored: bool,
    sums: Vec<f64>,
    msums: Vec<f64>,
    pairs: usize,
    pool: SlicePool<Complex32>,
    p: Published,
    /// Spectrum mirroring was detected.
    pub spectrum_inverted: bool,
}

impl Default for DvbtFft {
    fn default() -> Self {
        Self::new()
    }
}

impl DvbtFft {
    /// A new FFT stage.
    pub fn new() -> Self {
        DvbtFft {
            link: None,
            rate: 1.0,
            modes: Vec::new(),
            work: Vec::new(),
            prev: Vec::new(),
            prev_count: None,
            shift: None,
            mirrored: false,
            sums: Vec::new(),
            msums: Vec::new(),
            pairs: 0,
            pool: SlicePool::default(),
            p: Published::new(FftFields::SHAPE.fields),
            spectrum_inverted: false,
        }
    }

    /// A stage that publishes into `link`'s telemetry.
    pub fn with_link(link: Arc<crate::items::Link>) -> Self {
        DvbtFft {
            link: Some(link),
            ..Self::new()
        }
    }

    /// The integer carrier offset (None while searching).
    pub fn shift(&self) -> Option<i32> {
        self.shift
    }

    fn mode(&mut self, n: usize) -> usize {
        if let Some(i) = self.modes.iter().position(|m| m.n == n) {
            return i;
        }
        let m = Mode::from_fft(n).unwrap_or(Mode::K2);
        let k = m.carriers();
        let pilots = continual(m)
            .iter()
            .map(|&c| c as i64 - (k as i64 - 1) / 2)
            .collect();
        self.modes.push(ModeState {
            n,
            k,
            fft: Fft::forward(n),
            pilots,
        });
        self.modes.len() - 1
    }

    /// The widest integer carrier offset that the search tries, in carriers: 175 kHz, which holds the transmitter
    /// offsets of 125 kHz and 166.67 kHz with the error of a radio, and at least 20 carriers.
    fn search_range(&self, n: usize) -> i32 {
        let spacing = self.rate / n as f64;
        ((175e3 / spacing).ceil() as i32).max(20)
    }

    fn reset_search(&mut self) {
        self.shift = None;
        self.sums.clear();
        self.msums.clear();
        self.pairs = 0;
    }

    /// Scores every candidate shift on the continual pilots of this symbol against the previous one.
    fn score(&mut self, mi: usize, moved: i64) {
        let range = self.search_range(self.modes[mi].n);
        let m = &self.modes[mi];
        let n = m.n as i64;
        let cands = (2 * range + 1) as usize;
        if self.sums.len() != cands {
            self.sums = vec![0.0; cands];
            self.msums = vec![0.0; cands];
        }
        for (ci, c) in (-range..=range).enumerate() {
            for mirror in [false, true] {
                let mut acc = Complex32::default();
                let mut mag = 0f32;
                for &f in &m.pilots {
                    let freq = if mirror { c as i64 - f } else { f + c as i64 };
                    let bin = freq.rem_euclid(n) as usize;
                    let a = 2.0 * PI * (freq as f64) * moved as f64 / n as f64;
                    let ramp = Complex32::new(a.cos() as f32, a.sin() as f32);
                    let z = self.work[bin] * (self.prev[bin] * ramp).conj();
                    acc += z;
                    mag += z.norm();
                }
                let s = if mag > 0.0 {
                    (acc.norm() / mag) as f64
                } else {
                    0.0
                };
                if mirror {
                    self.msums[ci] += s;
                } else {
                    self.sums[ci] += s;
                }
            }
        }
        self.pairs += 1;
        if self.pairs < 11 {
            return;
        }
        let best = |v: &[f64]| -> (usize, f64, f64) {
            let mut b = (0usize, -1.0f64);
            for (i, &x) in v.iter().enumerate() {
                if x > b.1 {
                    b = (i, x);
                }
            }
            let second = v
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != b.0)
                .map(|(_, &x)| x)
                .fold(0.0, f64::max);
            (b.0, b.1, second)
        };
        let (di, dv, d2) = best(&self.sums);
        let (_, mv, _) = best(&self.msums);
        if mv > 1.5 * dv {
            self.mirrored = !self.mirrored;
            self.spectrum_inverted = self.mirrored;
            self.p
                .results_mut()
                .log("spectrum is mirrored: turned round".to_string());
            self.reset_search();
            return;
        }
        if dv > 1.5 * d2 {
            self.shift = Some(di as i32 - range);
            self.p.results_mut().log(format!(
                "integer carrier offset {} carriers",
                di as i32 - range
            ));
        } else {
            self.sums.iter_mut().for_each(|x| *x = 0.0);
            self.msums.iter_mut().for_each(|x| *x = 0.0);
            self.pairs = 0;
        }
    }
}

impl Component for DvbtFft {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.fft",
            vec![Port::typed::<SymbolBlock>("in")],
            vec![Port::typed::<Carriers>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset_search();
        self.mirrored = false;
        self.spectrum_inverted = false;
        self.prev_count = None;
        Ok(vec![Format::new(Kind::typed::<Carriers>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut out = std::mem::take(io.items_out::<Carriers>(0));
        let meter = io.meter();
        for item in io.items::<SymbolBlock>(0) {
            let b = &item.value;
            self.rate = b.rate;
            if b.fresh {
                self.reset_search();
                self.prev_count = None;
            }
            let mi = self.mode(b.fft);
            let n = self.modes[mi].n;
            let k = self.modes[mi].k;
            self.work.clear();
            if self.mirrored {
                self.work.extend(b.samples.iter().map(|x| x.conj()));
            } else {
                self.work.extend_from_slice(&b.samples);
            }
            if self.work.len() != n {
                continue;
            }
            let timer = meter.section("fft");
            self.modes[mi].fft.process(&mut self.work);
            drop(timer);
            let step = if self.mirrored {
                -b.int_step
            } else {
                b.int_step
            };
            if step != 0 {
                match self.shift.as_mut() {
                    Some(s) => *s += step,
                    None => self.reset_search(),
                }
            }
            let consecutive =
                self.prev_count == Some(b.count.wrapping_sub(1)) && self.prev.len() == n;
            if self.shift.is_none() && consecutive && !b.collapsed {
                self.score(mi, b.moved);
            }
            if let Some(s) = self.shift {
                let half = (k as i64 - 1) / 2;
                let work = &self.work;
                let cells = self.pool.exact(k, |v| {
                    for (c, o) in v.iter_mut().enumerate() {
                        *o = work[((c as i64 - half + s as i64).rem_euclid(n as i64)) as usize];
                    }
                });
                out.push(Timed {
                    time: item.time,
                    value: Carriers {
                        cells,
                        fft: n,
                        guard: b.guard,
                        count: b.count,
                        moved: b.moved,
                        collapsed: b.collapsed,
                        resynced: b.resynced,
                        fresh: b.fresh,
                        int_offset: s,
                        mirrored: self.mirrored,
                        frac: if self.mirrored { -b.frac } else { b.frac },
                        time: b.time,
                        rebuild: b.rebuild,
                        rate: b.rate,
                        l: 0,
                        frame: 0,
                        params: None,
                        erased: false,
                        tps_point: Complex32::default(),
                        tps_word: None,
                    },
                });
            }
            std::mem::swap(&mut self.prev, &mut self.work);
            self.prev_count = Some(b.count);
        }
        *io.items_out::<Carriers>(0) = out;
        if let Some(l) = &self.link {
            if let Ok(mut t) = l.telemetry.lock() {
                t.int_offset = self.shift;
                t.mirrored = self.spectrum_inverted;
            }
        }
        self.p.put(&FftFields {
            integer_offset: self.shift.map(i64::from),
            spectrum_inverted: self.spectrum_inverted,
        });
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The carrier offset belongs to the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.p.results_mut().fields_mut().clear();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
