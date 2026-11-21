//! The wave-buoy receiver components: front end, carrier finder, FSK demodulator, HXV decoder and the
//! frame analyser.

use super::frames::{self, Attempt};
use super::hxv::{Vector, VectorSync};
use crate::util::update_points;
use sdrkit_algo_dsp::decim::{Decimator, FirDecimator};
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::results::SeriesId;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, TagKind,
    Timed,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The symbol rate of both formats (RX-C4 specifications).
pub const BAUD: f64 = 81.92;
/// The nominal distance between the 2FSK tones, Hz.
pub const STEP_2FSK: f64 = 129.0;
/// The nominal distance between the 4FSK tones, Hz.
pub const STEP_4FSK: f64 = 78.0;

sdrkit_core::choice_enum! {
    /// The format of the link.
    #[derive(Serialize, Deserialize)]
    pub enum LinkFormat {
        /// A carrier or a narrow tone.
        Carrier = ("carrier", "carrier / narrow tone"),
        /// 2FSK (HXV).
        Fsk2 = ("2fsk", "2FSK"),
        /// 4MFSK (HVA).
        Mfsk4 = ("4mfsk", "4MFSK"),
    }
}

impl LinkFormat {
    /// The name for people.
    pub fn name(self) -> &'static str {
        match self {
            LinkFormat::Carrier => "carrier / narrow tone",
            LinkFormat::Fsk2 => "2FSK",
            LinkFormat::Mfsk4 => "4MFSK",
        }
    }
    /// The number of tones.
    pub fn levels(self) -> usize {
        if self == LinkFormat::Mfsk4 {
            4
        } else {
            2
        }
    }
    /// The nominal distance between the tones, Hz.
    pub fn step(self) -> f64 {
        if self == LinkFormat::Mfsk4 {
            STEP_4FSK
        } else {
            STEP_2FSK
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Front end

/// The decimation of the front end: D = round(rate / 16 kHz), at least 1.
pub fn decimation(rate: f64) -> usize {
    (rate / 16_000.0).round().max(1.0) as usize
}

/// `hfdata.buoy_front`: the signal offset mixed to 0 Hz by an oscillator, then a multi-stage decimator to about 16 kHz, flat to 40 % of
/// the output rate.
pub struct BuoyFront {
    nco: Nco,
    dec: Decimator<Complex32>,
    mixed: Vec<Complex32>,
    tags: Vec<(usize, f64)>,
}

impl Default for BuoyFront {
    fn default() -> Self {
        BuoyFront {
            nco: Nco::new(1.0, 0.0),
            dec: Decimator::new(2.0, 1, 0.4, 60.0),
            mixed: Vec::new(),
            tags: Vec::new(),
        }
    }
}

impl BuoyFront {
    fn run(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.nco.mix_in_place(&mut self.mixed);
        self.dec.process(&self.mixed, out);
    }
}

impl Component for BuoyFront {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.buoy_front",
            vec![Port::iq("in")],
            vec![Port::iq("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let d = decimation(f.rate);
        let out_rate = f.rate / d as f64;
        *self = BuoyFront {
            nco: Nco::new(f.rate, -f.signal_offset_hz()),
            dec: Decimator::new(f.rate, d, 0.4 * out_rate, 60.0),
            mixed: Vec::new(),
            tags: Vec::new(),
        };
        Ok(vec![Format::iq(out_rate, f.tuned_hz, f.tuned_hz)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let mut start = 0;
        let mut tags = std::mem::take(&mut self.tags);
        tags.clear();
        tags.extend(io.input(0).tags().iter().filter_map(|t| match t.kind {
            TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } => Some((
                (t.at.saturating_sub(first) as usize).min(x.len()),
                tuned_hz - centre_hz,
            )),
            _ => None,
        }));
        let out = io.iq_out(0);
        for &(at, off) in &tags {
            self.run(&x[start..at], out);
            start = at;
            self.nco.set_freq(-off);
        }
        self.run(&x[start..], out);
        self.tags = tags;
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Carrier finder

/// What the carrier finder sees (`wavebuoy.carrier`), once per report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Carrier {
    pub found: bool,
    /// Signal centre in the front end's stream, Hz.
    pub centre_hz: f64,
    /// Width holding 90 % of the power, Hz.
    pub width_hz: f64,
    pub peak_db: f64,
    pub snr_100_db: f64,
    pub format: LinkFormat,
    pub forced: bool,
    /// The spectrum around the signal: 256 columns over 3 kHz, 0 - 255 = 0 - 40 dB over the floor.
    pub columns: Vec<u8>,
    pub time: f64,
}

impl Item for Carrier {
    const KIND: &'static str = "wavebuoy.carrier";
}

/// `hfdata.buoy_carrier`, the carrier finder: `Iq` at rA in, `wavebuoy.carrier` out at every report.
pub struct CarrierFinder {
    rate: f64,
    ps: Welch,
    buf: Vec<Complex32>,
    fill: usize,
    avg: Vec<f64>,
    frames: u64,
    tmp: Vec<f32>,
    scratch: Vec<f64>,
    good_run: u32,
    bad_run: u32,
    found: bool,
    force: Option<LinkFormat>,
    last: Option<Carrier>,
    n: u64,
    results: Published,
    spectrum: SeriesId,
}

/// The spectrum around the signal: 256 columns over 3 kHz, 0 - 255 = 0 - 40 dB over the floor.
pub const SPECTRUM_SERIES: &str = "spectrum";

/// What the carrier finder publishes: the spectrum series.
fn finder_declaration() -> ResultsDecl {
    ResultsDecl::new().series(SPECTRUM_SERIES, "Spectrum", "column", "")
}

impl Default for CarrierFinder {
    fn default() -> Self {
        let decl = Arc::new(finder_declaration());
        let spectrum = decl
            .series_id(SPECTRUM_SERIES)
            .expect("the declaration holds the spectrum series");
        CarrierFinder {
            rate: 1.0,
            ps: Welch::new(2048, Window::Hann, 0.5, Scaling::Tone)
                .expect("2048 points is a valid Welch size"),
            buf: vec![Complex32::new(0.0, 0.0); 2048],
            fill: 0,
            avg: vec![0.0; 2048],
            frames: 0,
            tmp: vec![0.0; 2048],
            scratch: Vec::with_capacity(2048),
            good_run: 0,
            bad_run: 0,
            found: false,
            force: None,
            last: None,
            n: 0,
            results: Published::from_decl(decl),
            spectrum,
        }
    }
}

impl CarrierFinder {
    fn push(&mut self, x: &[Complex32]) {
        for &v in x {
            self.buf[self.fill] = v;
            self.fill += 1;
            self.n += 1;
            if self.fill == 2048 {
                // One frame, centred: index i is frequency (i - n/2) * bin.
                let _ = self.ps.compute(&self.buf, &mut self.tmp);
                if self.frames == 0 {
                    for (a, &t) in self.avg.iter_mut().zip(&self.tmp) {
                        *a = t as f64;
                    }
                } else {
                    for (a, &t) in self.avg.iter_mut().zip(&self.tmp) {
                        *a = 0.94 * *a + 0.06 * t as f64;
                    }
                }
                self.frames += 1;
                self.buf.copy_within(1024.., 0);
                self.fill = 1024;
            }
        }
    }

    /// Measures the averaged spectrum: the centre, the 90 % width, the peak, the SNR in 100 Hz and the format guess.
    fn analyse(&mut self) -> Carrier {
        let n = 2048usize;
        let bin = self.rate / n as f64;
        // Centred spectrum: index i is frequency (i - n/2) * bin.
        let sp: Vec<f64> = self.avg.clone();
        let floor = quantile_f64(&sp, 0.5, &mut self.scratch)
            .unwrap_or(0.0)
            .max(1e-30);
        let lim = (0.82 * self.rate / 2.0 / bin) as usize;
        let (lo, hi) = (n / 2 - lim.min(n / 2 - 2), n / 2 + lim.min(n / 2 - 2));
        let mut peak = n / 2;
        let mut pv = 0.0;
        for i in lo.max(1)..hi.min(n - 1) {
            let v = (sp[i - 1] + sp[i] + sp[i + 1]) / 3.0;
            if v > pv {
                pv = v;
                peak = i;
            }
        }
        let peak_db = 10.0 * (pv / floor).max(1e-3).log10();
        // The cluster: bins over twice the floor, bridging up to 4 bins below.
        let walk = |dir: i64| -> usize {
            let mut last = peak;
            let mut gap = 0;
            let mut i = peak as i64;
            for _ in 0..400 {
                i += dir;
                if i <= 0 || i >= n as i64 - 1 {
                    break;
                }
                if sp[i as usize] > 2.0 * floor {
                    last = i as usize;
                    gap = 0;
                } else {
                    gap += 1;
                    if gap > 4 {
                        break;
                    }
                }
            }
            last
        };
        let (a, b) = (walk(-1), walk(1));
        let pw: Vec<f64> = (a..=b).map(|i| (sp[i] - floor).max(0.0)).collect();
        let total: f64 = pw.iter().sum::<f64>().max(1e-30);
        let mut cum = 0.0;
        let (mut i5, mut i95) = (0usize, pw.len().saturating_sub(1));
        let mut got5 = false;
        for (k, &p) in pw.iter().enumerate() {
            cum += p;
            if !got5 && cum >= 0.05 * total {
                i5 = k;
                got5 = true;
            }
            if cum >= 0.95 * total {
                i95 = k;
                break;
            }
        }
        let width = ((i95 as f64 - i5 as f64).max(1.0)) * bin;
        let mean_off: f64 = pw
            .iter()
            .enumerate()
            .map(|(k, &p)| (a + k) as f64 * p)
            .sum::<f64>()
            / total;
        let centre = (mean_off - (n / 2) as f64) * bin;
        let snr = 10.0 * (total / (floor * 100.0 / bin)).max(1e-3).log10();
        let good = peak_db >= 8.0;
        if good {
            self.good_run += 1;
            self.bad_run = 0;
            if self.good_run >= 3 {
                self.found = true;
            }
        } else {
            self.bad_run += 1;
            self.good_run = 0;
            if self.bad_run >= 12 {
                self.found = false;
            }
        }
        let guess = if width < 35.0 {
            LinkFormat::Carrier
        } else if width < 205.0 {
            LinkFormat::Fsk2
        } else {
            LinkFormat::Mfsk4
        };
        // 256 columns over 3 kHz around the signal.
        let c0 = if self.found { centre } else { 0.0 };
        let columns = (0..256)
            .map(|k| {
                let f = c0 - 1500.0 + 3000.0 * (k as f64 + 0.5) / 256.0;
                let i = ((f / bin).round() as i64 + (n / 2) as i64).clamp(0, n as i64 - 1) as usize;
                let db = 10.0 * (sp[i] / floor).max(1e-3).log10();
                (db / 40.0 * 255.0).clamp(0.0, 255.0) as u8
            })
            .collect();
        Carrier {
            found: self.found,
            centre_hz: centre,
            width_hz: width,
            peak_db,
            snr_100_db: snr,
            format: self.force.unwrap_or(guess),
            forced: self.force.is_some(),
            columns,
            time: self.n as f64 / self.rate,
        }
    }
}

impl Component for CarrierFinder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.buoy_carrier",
            vec![Port::iq("in")],
            vec![Port::typed::<Carrier>("out")],
        )
        .with_settings(CarrierFinderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let force = self.force;
        *self = CarrierFinder::default();
        self.force = force;
        self.rate = inputs[0].rate;
        let mut o = Format::new(Kind::typed::<Carrier>(), 0.0);
        o.tuned_hz = inputs[0].tuned_hz;
        o.centre_hz = inputs[0].centre_hz;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        self.push(io.iq(0));
        if io.tick() && self.frames > 0 {
            let c = self.analyse();
            update_points(
                self.results.results_mut(),
                self.spectrum,
                true,
                c.columns
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| [i as f32, f32::from(v)]),
            );
            self.last = Some(c.clone());
            io.items_out::<Carrier>(0).push(Timed {
                time: c.time,
                value: c,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CarrierFinderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CarrierFinderSetting::Format => {
                self.force = match FORMAT.read(id, v)? {
                    FormatChoice::Auto => None,
                    FormatChoice::Fsk2 => Some(LinkFormat::Fsk2),
                    FormatChoice::Mfsk4 => Some(LinkFormat::Mfsk4),
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let format = match self.force {
            None | Some(LinkFormat::Carrier) => FormatChoice::Auto,
            Some(LinkFormat::Fsk2) => FormatChoice::Fsk2,
            Some(LinkFormat::Mfsk4) => FormatChoice::Mfsk4,
        };
        values_of([(CarrierFinderSetting::Format, Value::Choice(format.index()))])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

sdrkit_core::choice_enum! {
    /// The format that the receiver expects.
    pub enum FormatChoice {
        /// From the bandwidth.
        Auto = ("auto", "automatic (from the bandwidth)"),
        /// 2FSK.
        Fsk2 = ("2fsk", "2FSK"),
        /// 4MFSK.
        Mfsk4 = ("4mfsk", "4MFSK"),
    }
}

/// The `format` setting.
pub const FORMAT: Setting<field::Choice<FormatChoice>> = Setting::choice("format", "Format", FormatChoice::Auto)
    .help("A buoy that sends another format than the stack expects shows as a wrong-format signal. Use this choice to force one.");

sdrkit_core::settings! {
    /// The settings of [`CarrierFinder`].
    pub enum CarrierFinderSetting {
        /// Format.
        Format = FORMAT,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Demodulator

/// One symbol (`wavebuoy.symbol`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub time: f64,
    pub tone: u8,
    /// Mean frequency of the symbol, Hz from the centre.
    pub freq: f32,
    pub locked: bool,
    pub format: LinkFormat,
    /// The demodulator restarted before this symbol (a change of format).
    pub restart: bool,
    pub baud: f64,
    pub clock_ppm: f64,
    pub step_hz: f64,
    pub error: f64,
    pub count: u64,
    /// The signal centre, Hz in the front end's stream.
    pub centre_hz: f64,
}

impl Item for Symbol {
    const KIND: &'static str = "wavebuoy.symbol";
}

/// The FSK demodulator: `Iq` at rA and `wavebuoy.carrier` in, `wavebuoy.symbol` out.
pub struct FskDemod {
    ra: f64,
    rb: f64,
    nco: Nco,
    lp1: FirDecimator<Complex32>,
    lp2: Fir<Complex32>,
    centre: f64,
    have_centre: bool,
    prev: Complex32,
    integ: Vec<f64>,
    acc: f64,
    nb: u64,
    t: f64,
    tn: f64,
    next: f64,
    cur: usize,
    errs: [f64; 17],
    format: LinkFormat,
    tones: Vec<f64>,
    q: f64,
    count: u64,
    locked: bool,
    restart: bool,
    out: Vec<Symbol>,
    /// The finder sees a plain carrier: nothing to lock to.
    carrier_only: bool,
}

impl Default for FskDemod {
    fn default() -> Self {
        FskDemod {
            ra: 1.0,
            rb: 1.0,
            nco: Nco::new(1.0, 0.0),
            lp1: FirDecimator::new(&[1.0], 4),
            lp2: Fir::new(&[1.0]),
            centre: 0.0,
            have_centre: false,
            prev: Complex32::new(0.0, 0.0),
            integ: vec![0.0; 1024],
            acc: 0.0,
            nb: 0,
            t: 1.0,
            tn: 1.0,
            next: 0.0,
            cur: 8,
            errs: [0.25; 17],
            format: LinkFormat::Fsk2,
            tones: Vec::new(),
            q: 1.0,
            count: 0,
            locked: false,
            restart: false,
            out: Vec::new(),
            carrier_only: false,
        }
    }
}

impl FskDemod {
    fn set_rate(&mut self, ra: f64) {
        self.ra = ra;
        self.rb = ra / 4.0;
        self.nco = Nco::new(ra, -self.centre);
        // Hamming-windowed sinc lowpass filters: 63 taps at 600 Hz before the decimation by 4, 65 taps at 400 Hz after it.
        let lp1 = design_by_taps(63, 600.0 / ra, Window::Hamming)
            .expect("600 Hz fits the front end rate");
        let lp2 = design_by_taps(65, 400.0 / self.rb, Window::Hamming)
            .expect("400 Hz fits the symbol filter rate");
        self.lp1 = FirDecimator::new(&lp1, 4);
        self.lp2 = Fir::new(&lp2);
        self.tn = self.rb / BAUD;
        self.restart_demod(self.format);
    }

    fn restart_demod(&mut self, f: LinkFormat) {
        self.format = f;
        let l = f.levels();
        self.tones = (0..l)
            .map(|i| (i as f64 - (l as f64 - 1.0) / 2.0) * f.step())
            .collect();
        self.t = self.tn;
        self.next = self.nb as f64 + self.t;
        self.cur = 8;
        self.errs = [0.25; 17];
        self.q = 1.0;
        self.count = 0;
        self.locked = false;
        self.restart = true;
    }

    /// The integral of the frequency at fractional sample `x`.
    fn integral(&self, x: f64) -> f64 {
        let i = x.floor();
        let fr = x - i;
        let k = i as u64;
        let a = self.integ[(k % 1024) as usize];
        let b = self.integ[((k + 1) % 1024) as usize];
        a + fr * (b - a)
    }

    fn nearest(&self, f: f64) -> (usize, f64) {
        let mut best = (0usize, f64::MAX);
        for (i, &t) in self.tones.iter().enumerate() {
            let d = (f - t).abs();
            if d < best.1 {
                best = (i, d);
            }
        }
        best
    }

    fn symbol(&mut self) {
        let step = self.format.step();
        let mut fs = [0f64; 17];
        for (p, fp) in fs.iter_mut().enumerate() {
            let e = self.next + (p as f64 - 8.0) * self.t / 16.0;
            *fp = (self.integral(e) - self.integral(e - self.t)) / self.t;
            let (_, d) = self.nearest(*fp);
            let d2 = ((d / step) * (d / step)).min(4.0);
            self.errs[p] = 0.95 * self.errs[p] + 0.05 * d2;
        }
        let (mut bp, mut be) = (self.cur, self.errs[self.cur]);
        for (p, &e) in self.errs.iter().enumerate() {
            if e < be {
                be = e;
                bp = p;
            }
        }
        if be < 0.85 * self.errs[self.cur] {
            self.cur = bp;
        }
        let f = fs[self.cur];
        let (i, d) = self.nearest(f);
        if d < step / 2.0 {
            self.tones[i] += 0.04 * (f - self.tones[i]);
        }
        let l = self.tones.len();
        let sp = (self.tones[l - 1] - self.tones[0]) / (l as f64 - 1.0);
        if !(0.6 * step..=1.5 * step).contains(&sp) {
            self.tones = (0..l)
                .map(|k| (k as f64 - (l as f64 - 1.0) / 2.0) * step)
                .collect();
        }
        let d2 = ((d / step) * (d / step)).min(4.0);
        self.q = 0.97 * self.q + 0.03 * d2;
        self.count += 1;
        let err = self.q.sqrt();
        if self.carrier_only {
            self.locked = false;
        } else if !self.locked && err < 0.28 && self.count >= 48 {
            self.locked = true;
        } else if self.locked && err > 0.45 {
            self.locked = false;
        }
        if self.locked {
            let mid: f64 = self.tones.iter().sum::<f64>() / l as f64;
            let m = 0.002 * mid;
            self.centre += m;
            self.nco.set_freq(-self.centre);
            for t in self.tones.iter_mut() {
                *t -= m;
            }
        }
        let time = self.next / self.rb;
        let sym = Symbol {
            time,
            tone: i as u8,
            freq: f as f32,
            locked: self.locked,
            format: self.format,
            restart: std::mem::take(&mut self.restart),
            baud: self.rb / self.t,
            clock_ppm: (self.tn / self.t - 1.0) * 1e6,
            step_hz: sp,
            error: err,
            count: self.count,
            centre_hz: self.centre,
        };
        self.out.push(sym);
        // The clock loop.
        let pe = (self.cur as f64 - 8.0) * self.t / 16.0;
        self.next += self.t + 0.02 * pe;
        self.t = (self.t + 0.0003 * pe).clamp(0.98 * self.tn, 1.02 * self.tn);
        let k = self.cur as i64 - 8;
        if k.abs() >= 4 {
            self.next += k as f64 * self.t / 16.0;
            let old = self.errs;
            for p in 0..17i64 {
                let q = p + k;
                self.errs[p as usize] = if (0..17).contains(&q) {
                    old[q as usize]
                } else {
                    0.25
                };
            }
            self.cur = 8;
        }
    }

    fn run(&mut self, x: &[Complex32]) {
        for &v in x {
            // The centre can move at any symbol (it follows the tones while locked).
            let Some(y) = self.lp1.push(v * self.nco.next_sample()) else {
                continue;
            };
            let z = self.lp2.process_sample(y);
            let f = (z * self.prev.conj()).arg() as f64 * self.rb / TAU;
            self.prev = z;
            self.acc += f;
            self.nb += 1;
            self.integ[(self.nb % 1024) as usize] = self.acc;
            while (self.nb as f64) > self.next + self.t / 2.0 + 1.0 {
                self.symbol();
            }
        }
    }

    fn carrier(&mut self, c: &Carrier) {
        self.carrier_only = c.found && c.format == LinkFormat::Carrier && !c.forced;
        if c.format != self.format && (c.found || c.forced) && c.format != LinkFormat::Carrier {
            self.restart_demod(c.format);
        }
        if c.found && !self.locked {
            let d = c.centre_hz - self.centre;
            if !self.have_centre {
                self.centre = c.centre_hz;
                self.have_centre = true;
            } else if d.abs() > 3.0 {
                self.centre += 0.3 * d;
            }
            self.nco.set_freq(-self.centre);
        }
    }
}

impl Component for FskDemod {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.buoy_fsk_demod",
            vec![Port::iq("in"), Port::typed::<Carrier>("carrier")],
            vec![Port::typed::<Symbol>("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = FskDemod::default();
        self.set_rate(inputs[0].rate);
        Ok(vec![Format::new(Kind::typed::<Symbol>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.run(x);
        for c in io.items::<Carrier>(1) {
            self.carrier(&c.value);
        }
        let o = io.items_out::<Symbol>(0);
        for s in self.out.drain(..) {
            o.push(Timed {
                time: s.time,
                value: s,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// HXV decoder

/// What the HXV decoder sends (`wavebuoy.vector`).
#[derive(Clone, Debug, PartialEq)]
pub enum VectorEvent {
    Vector(Vector),
    Lost(u64),
    /// The polarity flipped or the lock was lost: the block in progress is dropped.
    Restart,
    /// The synchroniser's state: locked, inverted, vectors, clean, repaired, lost.
    State {
        locked: bool,
        inverted: bool,
        vectors: u64,
        clean: u64,
        repaired: u64,
        lost: u64,
    },
}

impl Item for VectorEvent {
    const KIND: &'static str = "wavebuoy.vector";
}

/// The HXV decoder: 2FSK symbols in (every symbol, locked or not), vectors out.
#[derive(Default)]
pub struct HxvDecoder {
    sync: VectorSync,
    since_sync: Option<u64>,
    out: Vec<VectorEvent>,
}

impl HxvDecoder {
    /// Takes one symbol.
    pub fn take(&mut self, s: &Symbol) {
        if s.restart {
            self.sync.reset();
            self.since_sync = None;
            self.out.push(VectorEvent::Restart);
        }
        if s.format != LinkFormat::Fsk2 {
            return;
        }
        let was = self.sync.locked.is_some();
        match self.sync.push(s.tone.min(1), s.time) {
            Some(Ok(v)) => {
                if v.cyclic == super::hxv::SYNC {
                    self.since_sync = Some(v.index);
                }
                let in_block = self.since_sync.is_some_and(|i| v.index - i < 18);
                if v.cyclic == 0x8000 && !in_block {
                    self.sync.flip();
                    self.since_sync = None;
                    self.out.push(VectorEvent::Restart);
                } else {
                    self.out.push(VectorEvent::Vector(v));
                }
            }
            Some(Err(i)) => self.out.push(VectorEvent::Lost(i)),
            None => {}
        }
        if was && self.sync.locked.is_none() {
            self.out.push(VectorEvent::Restart);
        }
    }
    fn state(&self) -> VectorEvent {
        let (locked, inverted) = self.sync.state();
        VectorEvent::State {
            locked,
            inverted,
            vectors: self.sync.vectors,
            clean: self.sync.clean,
            repaired: self.sync.repaired,
            lost: self.sync.lost,
        }
    }
}

impl Component for HxvDecoder {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hxv_decoder",
            vec![Port::typed::<Symbol>("in")],
            vec![Port::typed::<VectorEvent>("out")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.sync.reset();
        self.since_sync = None;
        Ok(vec![Format::new(Kind::typed::<VectorEvent>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.items::<Symbol>(0) {
            self.take(&s.value);
        }
        if io.tick() {
            let st = self.state();
            self.out.push(st);
        }
        let o = io.items_out::<VectorEvent>(0);
        for e in self.out.drain(..) {
            o.push(Timed {
                time: 0.0,
                value: e,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Frame analyser

/// A frame found by the analyser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    pub time: f64,
    pub hex: String,
    pub accepted: bool,
}

/// One analysis (`wavebuoy.frame`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub period: Option<usize>,
    pub z: f64,
    pub sync_start: Option<usize>,
    pub sync_len: usize,
    pub sync_hex: String,
    pub attempt: Option<Attempt>,
    pub new_frames: Vec<FrameRow>,
    pub bits: usize,
}

impl Item for Analysis {
    const KIND: &'static str = "wavebuoy.frame";
}

const STORE: usize = 24_000;

/// The frame analyser: symbols in; every 1.5 s, with 1500 locked bits or more, the newest 8192 analysed.
#[derive(Default)]
pub struct FrameAnalyser {
    bits: VecDeque<u8>,
    /// Absolute index of the first stored bit.
    first: u64,
    unlocked_since: Option<f64>,
    last_t: f64,
    next_t: f64,
    attempt: Option<Attempt>,
    counted_to: u64,
    out: Vec<Analysis>,
}

impl FrameAnalyser {
    fn store(&mut self, b: u8) {
        if self.bits.len() >= STORE {
            self.bits.pop_front();
            self.first += 1;
        }
        self.bits.push_back(b);
    }

    /// Takes one symbol.
    pub fn take(&mut self, s: &Symbol) {
        if s.restart {
            self.drop_bits();
            self.attempt = None;
        }
        self.last_t = s.time;
        if s.locked {
            self.unlocked_since = None;
            if s.format == LinkFormat::Mfsk4 {
                self.store((s.tone >> 1) & 1);
                self.store(s.tone & 1);
            } else {
                self.store(s.tone & 1);
            }
        } else {
            let since = *self.unlocked_since.get_or_insert(s.time);
            if s.time - since > 3.0 && !self.bits.is_empty() {
                self.drop_bits();
            }
        }
        if s.time >= self.next_t {
            self.next_t = s.time + 1.5;
            if self.bits.len() >= 1500 {
                let a = self.analyse();
                self.out.push(a);
            }
        }
    }

    fn drop_bits(&mut self) {
        self.first += self.bits.len() as u64;
        self.bits.clear();
    }

    fn analyse(&mut self) -> Analysis {
        let n = self.bits.len().min(8192);
        let start_abs = self.first + (self.bits.len() - n) as u64;
        let bits: Vec<u8> = self
            .bits
            .iter()
            .skip(self.bits.len() - n)
            .copied()
            .collect();
        let mut a = Analysis {
            period: None,
            z: 0.0,
            sync_start: None,
            sync_len: 0,
            sync_hex: String::new(),
            attempt: None,
            new_frames: Vec::new(),
            bits: n,
        };
        let Some((lag, z)) = frames::period(&bits) else {
            a.attempt = self.attempt.clone();
            return a;
        };
        a.period = Some(lag);
        a.z = z;
        let Some((s0, len, maj)) = frames::sync(&bits, lag) else {
            return a;
        };
        a.sync_start = Some(s0);
        a.sync_len = len;
        a.sync_hex = frames::bits_hex(&maj);
        let mut fr: Vec<(u64, Vec<u8>)> = Vec::new();
        let mut i = s0;
        while i + lag <= n {
            fr.push((start_abs + i as u64, bits[i..i + lag].to_vec()));
            i += lag;
        }
        let frames_only: Vec<Vec<u8>> = fr.iter().map(|f| f.1.clone()).collect();
        let found = frames::attempt(&frames_only, len);
        let att = match (found, &self.attempt) {
            (Some(x), _) => Some(x),
            (None, Some(prev)) => {
                let p = frames_only
                    .iter()
                    .filter(|f| frames::attempt_passes(prev, f, len))
                    .count();
                (p >= 3 && p * 2 >= frames_only.len()).then(|| prev.clone())
            }
            (None, None) => None,
        };
        self.attempt = att.clone();
        a.attempt = att.clone();
        for (abs, f) in &fr {
            if *abs < self.counted_to {
                continue;
            }
            self.counted_to = abs + lag as u64;
            let ok = att
                .as_ref()
                .is_some_and(|x| frames::attempt_passes(x, f, len));
            a.new_frames.push(FrameRow {
                time: self.last_t,
                hex: frames::bits_hex(f),
                accepted: ok,
            });
        }
        a
    }
}

impl Component for FrameAnalyser {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.buoy_frame_analyser",
            vec![Port::typed::<Symbol>("in")],
            vec![Port::typed::<Analysis>("out")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.drop_bits();
        Ok(vec![Format::new(Kind::typed::<Analysis>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.items::<Symbol>(0) {
            self.take(&s.value);
        }
        let o = io.items_out::<Analysis>(0);
        for a in self.out.drain(..) {
            o.push(Timed {
                time: 0.0,
                value: a,
            });
        }
    }
}
