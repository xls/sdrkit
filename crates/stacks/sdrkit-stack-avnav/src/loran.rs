//! Loran-C, eLoran and Chayka on 100 kHz: the chain's GRI (set, or found among the chains of the `chains` setting), the master
//! and the secondaries, their time differences (TD), envelope-to-cycle differences (ECD) and signal to noise. The chain list is
//! data that the consumer passes (the rows of `data/navigation/loran_chains.csv`).
//! Standard: ITU-R M.589, the Loran-C signal specification (Chayka uses the same pulse groups and phase codes).

use crate::fields::put_status;
use crate::typed::{LoranFields, LoranStationRow, LORAN_STATIONS};
use num_complex::Complex64;
use sdrkit_algo_dsp::peak::parabolic;
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_components::dsp::Channel;
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig, Generator,
    LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StackConfig, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Item, Kind, Port, RateFrom, RateRule, StackGraph, Timed,
};
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "loran";
/// The carrier, Hz.
pub const CARRIER_HZ: f64 = 100e3;
/// The rate the receiver works at: one sample per carrier cycle (10 µs).
pub const WORK_RATE_HZ: f64 = 100e3;
/// The least radio rate.
pub const MIN_RATE_HZ: f64 = 100e3;
/// The envelope's time to peak, seconds.
pub const PULSE_RISE_S: f64 = 65e-6;
/// Master phase codes, interval A and B.
pub const MASTER_CODE: [[i8; 9]; 2] = [
    [1, 1, -1, -1, 1, -1, 1, -1, 1],
    [1, -1, -1, 1, 1, 1, 1, 1, -1],
];
/// Secondary phase codes, interval A and B.
pub const SECONDARY_CODE: [[i8; 8]; 2] = [[1, 1, 1, 1, 1, -1, -1, 1], [1, -1, 1, -1, 1, 1, -1, -1]];

/// The chains of a `chains` setting: one a line (or separated by `;`), the GRI in tens of microseconds, a comma, the name.
/// Lines starting with `#`, a header line and rows whose GRI is not a number from 4000 to 9999 are left out; a GRI given twice
/// keeps its first name.
pub fn parse_chains(text: &str) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = Vec::new();
    for row in text.split(['\n', ';']) {
        let row = row.trim();
        if row.starts_with('#') {
            continue;
        }
        let (gri, name) = row.split_once(',').unwrap_or((row, ""));
        let Ok(gri) = gri.trim().parse::<u32>() else {
            continue;
        };
        if (4000..=9999).contains(&gri) && !out.iter().any(|c| c.0 == gri) {
            out.push((gri, name.trim().trim_matches('"').to_string()));
        }
    }
    out
}

/// The pulse envelope at `t` seconds after the pulse starts (peak 1 at 65 µs).
pub fn envelope(t: f64) -> f64 {
    if t <= 0.0 {
        0.0
    } else {
        let u = t / PULSE_RISE_S;
        u * u * (2.0 - 2.0 * u).exp()
    }
}

/// The offset of pulse `p` of a group, µs.
pub fn pulse_offset_us(p: usize) -> f64 {
    if p < 8 {
        1000.0 * p as f64
    } else {
        9000.0
    }
}

/// One station heard.
#[derive(Clone, Debug, PartialEq)]
pub struct LoranStation {
    /// `M` for the master. Secondaries are lettered in order of their delay (`W`, `X`, `Y`, `Z`, then `V`, `U`, `T`).
    pub name: &'static str,
    /// Time difference from the master, µs (0 for the master).
    pub td_us: f64,
    /// Envelope-to-cycle difference relative to the master, µs.
    pub ecd_us: f64,
    /// Signal to noise of the correlation, dB.
    pub snr_db: f32,
    /// Level, relative to the master, dB.
    pub level_db: f32,
}

/// One measurement of a chain.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoranFix {
    /// The GRI.
    pub gri: u32,
    /// The master and secondaries, master first.
    pub stations: Vec<LoranStation>,
    /// The chain of the GRI in the `chains` setting, empty when it has none.
    pub chain: String,
}

impl Item for LoranFix {
    const KIND: &'static str = "loran.fix";
}

/// The envelope template, one value per 10 µs sample from the pulse start.
fn template() -> Vec<f64> {
    (0..30).map(|j| envelope(j as f64 * 10e-6)).collect()
}

/// A coherent accumulator for one GRI.
#[derive(Clone, Debug)]
struct Accumulator {
    gri: u32,
    buf: Vec<Complex32>,
    pos: usize,
    /// Phase code intervals added.
    pcis: u64,
}

impl Accumulator {
    fn new(gri: u32) -> Self {
        Accumulator {
            gri,
            buf: vec![Complex32::default(); 2 * gri as usize],
            pos: 0,
            pcis: 0,
        }
    }
    #[inline]
    fn add(&mut self, x: &[Complex32], a: f32) {
        // Contiguous runs, so the loop vectorises.
        let mut i = 0;
        let len = self.buf.len();
        while i < x.len() {
            let run = (len - self.pos).min(x.len() - i);
            let dst = &mut self.buf[self.pos..self.pos + run];
            let first = self.pcis == 0;
            for (d, &s) in dst.iter_mut().zip(&x[i..i + run]) {
                *d = if first { s } else { *d + (s - *d) * a };
            }
            self.pos += run;
            i += run;
            if self.pos == len {
                self.pos = 0;
                self.pcis += 1;
            }
        }
    }
}

/// A peak of a correlation.
#[derive(Clone, Copy, Debug)]
struct Peak {
    k: usize,
    t_env: f64,
    value: Complex64,
    mag: f64,
}

pub const GRI_SETTING: Setting<field::I64> = Setting::new("gri", "GRI", "", 0)
    .range(0.0, 99990.0, 1.0)
    .help("The chain's group repetition interval (9960 for 99.6 ms). 0 searches the chains of the setting chains.");
/// The `chains` setting.
pub const CHAINS: Setting<field::Text> = Setting::new("chains", "Chains", "", "").help(
    "One a line (or separated by ;): the GRI in tens of microseconds, a comma, the chain's name. With GRI 0 these GRIs are \
     searched; the chain found takes its name. Fewer rows cost less to search.",
);
pub const AVERAGE: Setting<field::Duration> =
    Setting::new("average", "Averaging", "s", 2.0).range(0.2, 60.0, 0.0);
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Secondary threshold", "dB", 12.0).range(3.0, 40.0, 0.0);
pub const MAX_SECONDARIES: Setting<field::I64> =
    Setting::new("max_secondaries", "Secondaries", "", 5).range(0.0, 7.0, 1.0);
pub const INTERVAL: Setting<field::Duration> =
    Setting::new("interval", "Measure every", "s", 1.0).range(0.2, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`LoranTracker`].
    pub enum LoranTrackerSetting {
        /// The GRI, 0 to search.
        Gri = GRI_SETTING,
        /// The time of coherent averaging.
        Average = AVERAGE,
        /// The correlation SNR a secondary needs.
        Threshold = THRESHOLD,
        /// The most secondaries looked for.
        MaxSecondaries = MAX_SECONDARIES,
        /// The time between measurements.
        Interval = INTERVAL,
        /// The chains searched and named.
        Chains = CHAINS,
    }
}

/// The chain tracker (`avnav.loran_tracker`, iq at 100 ksps -> [`LoranFix`]). It adds the signal coherently into one buffer per
/// phase code interval (two GRIs) for each GRI it looks at, correlates the buffer with the master's and the secondaries' codes and
/// with the pulse envelope, and finds the master and the secondaries. A station's time of arrival comes from its envelope (to a
/// fraction of the 10 µs cycle) and its carrier phase (within the cycle).
pub struct LoranTracker {
    gri: u32,
    average: f64,
    threshold: f64,
    max_secondaries: usize,
    interval: f64,
    rate: f64,
    accs: Vec<Accumulator>,
    alpha: f32,
    count: u64,
    every: u64,
    tmpl: Vec<f64>,
    s: Vec<Complex64>,
    m: Vec<Complex64>,
    mags: Vec<f64>,
    scratch: Vec<f64>,
    work: Vec<Complex32>,
    chosen: Option<u32>,
    chains_text: String,
    chains: Vec<(u32, String)>,
}

impl Default for LoranTracker {
    fn default() -> Self {
        LoranTracker {
            gri: GRI_SETTING.default as u32,
            average: AVERAGE.default,
            threshold: THRESHOLD.default,
            max_secondaries: MAX_SECONDARIES.default as usize,
            interval: INTERVAL.default,
            rate: 0.0,
            accs: Vec::new(),
            alpha: 0.0,
            count: 0,
            every: 1,
            tmpl: template(),
            s: Vec::new(),
            m: Vec::new(),
            mags: Vec::new(),
            scratch: Vec::new(),
            work: Vec::new(),
            chosen: None,
            chains_text: String::new(),
            chains: Vec::new(),
        }
    }
}

impl LoranTracker {
    /// With `gri` (0: search).
    ///
    /// # Errors
    ///
    /// The [`SettingError`] of the `gri` setting for a GRI outside its range.
    pub fn new(gri: u32) -> Result<Self, SettingError> {
        LoranTrackerSetting::Gri.check(&Value::I64(i64::from(gri)))?;
        Ok(LoranTracker {
            gri,
            ..Default::default()
        })
    }

    fn build(&mut self) {
        let gris: Vec<u32> = if self.gri > 0 {
            vec![self.gri]
        } else {
            self.chains.iter().map(|c| c.0).collect()
        };
        self.accs = gris.into_iter().map(Accumulator::new).collect();
        // One update per phase code interval in each buffer slot: the time constant in intervals.
        let pci_s = 2.0 * self.accs.first().map_or(0, |a| a.gri) as f64 * 10e-6;
        self.alpha = (pci_s / self.average.max(pci_s)).min(1.0) as f32;
        self.count = 0;
        self.chosen = None;
    }

    /// Correlates `acc` with a code (master or secondary) and the envelope, into `self.m` (`|m|` into `self.mags`).
    fn correlate(&mut self, a: usize, master: bool) {
        let acc = &self.accs[a];
        let gri = acc.gri as usize;
        correlate_into(
            &acc.buf,
            gri,
            master,
            &self.tmpl,
            (&mut self.s, &mut self.m, &mut self.mags),
        );
    }

    /// Copies accumulator `a` into `self.work` less the master's pulses, the master starting at sample `t_env` (fractional).
    /// The master's amplitude and phase are fitted to its pulses.
    fn without_master(&mut self, a: usize, t_env: f64) {
        let acc = &self.accs[a];
        let (l, gri) = (acc.buf.len(), acc.gri as usize);
        self.work.clear();
        self.work.extend_from_slice(&acc.buf);
        let k0 = t_env.floor();
        let frac = t_env - k0;
        let k0 = (k0 as isize).rem_euclid(l as isize) as usize;
        let e: Vec<f64> = (0..32)
            .map(|j| envelope((j as f64 - frac) * 10e-6))
            .collect();
        let at = |i: usize, p: usize, j: usize| {
            (k0 + i * gri + (pulse_offset_us(p) / 10.0) as usize + j) % l
        };
        let (mut num, mut den) = (Complex64::new(0.0, 0.0), 0.0);
        for i in 0..2 {
            for p in 0..9 {
                let c = f64::from(MASTER_CODE[i][p]);
                for (j, &ej) in e.iter().enumerate() {
                    let x = self.work[at(i, p, j)];
                    num += Complex64::new(f64::from(x.re), f64::from(x.im)) * c * ej;
                    den += ej * ej;
                }
            }
        }
        let amp = num / den.max(1e-30);
        for i in 0..2 {
            for p in 0..9 {
                let c = f64::from(MASTER_CODE[i][p]);
                for (j, &ej) in e.iter().enumerate() {
                    let v = amp * c * ej;
                    self.work[at(i, p, j)] -= Complex32::new(v.re as f32, v.im as f32);
                }
            }
        }
    }

    /// The rms of the noise in the correlation: its magnitudes follow a Rayleigh law, whose median is `sigma sqrt(2 ln 2)` and
    /// whose rms is `sigma sqrt(2)`.
    fn noise(&mut self) -> f64 {
        let median = quantile_f64(&self.mags, 0.5, &mut self.scratch).unwrap_or(0.0);
        median / (2.0 * std::f64::consts::LN_2).sqrt() * std::f64::consts::SQRT_2
    }

    fn peak_at(&self, k: usize) -> Peak {
        let n = self.mags.len();
        let (l, c, r) = (
            self.mags[(k + n - 1) % n],
            self.mags[k],
            self.mags[(k + 1) % n],
        );
        Peak {
            k,
            t_env: k as f64 + parabolic(l, c, r).offset,
            value: self.m[k],
            mag: c,
        }
    }

    fn best(&self, exclude: &[(usize, usize)]) -> Option<usize> {
        let n = self.mags.len();
        let mut best: Option<usize> = None;
        for k in 0..n {
            if exclude.iter().any(|&(a, w)| {
                let d = (k as isize - a as isize).rem_euclid(n as isize) as usize;
                d.min(n - d) <= w
            }) {
                continue;
            }
            if best.is_none_or(|b| self.mags[k] > self.mags[b]) {
                best = Some(k);
            }
        }
        best
    }

    /// How far the master of accumulator `a`'s GRI stands above the noise, dB.
    fn master_snr(&mut self, a: usize) -> f64 {
        self.correlate(a, true);
        let noise = self.noise().max(1e-30);
        let peak = self.mags.iter().copied().fold(0.0, f64::max);
        20.0 * (peak / noise).log10()
    }

    /// Measures the chain in accumulator `a`.
    fn measure(&mut self, a: usize) -> Option<(LoranFix, f64)> {
        let gri = self.accs[a].gri;
        self.correlate(a, true);
        let noise_m = self.noise().max(1e-30);
        let mk = self.best(&[])?;
        let master = self.peak_at(mk);
        let msnr = 20.0 * (master.mag / noise_m).log10();
        // The secondaries are searched with the master's pulses taken out, so that a secondary close to the master is found.
        self.without_master(a, master.t_env);
        correlate_into(
            &self.work,
            gri as usize,
            false,
            &self.tmpl,
            (&mut self.s, &mut self.m, &mut self.mags),
        );
        let noise_s = self.noise().max(1e-30);
        let n = self.mags.len();
        // From 7 ms before the master to the end of its group a secondary's code lines up in part with the master's pulses, and
        // what the subtraction leaves of the master can peak there: a secondary there must be within 20 dB of the master,
        // elsewhere within 40 dB (a clean signal has almost no noise to compare with). Each must stand above the noise.
        let near_master = |k: usize| {
            let d = (k + n - mk) % n;
            d <= 947 || d >= n - 723
        };
        let mut exclude = vec![(mk, 60)];
        let mut found: Vec<Peak> = Vec::new();
        while found.len() < self.max_secondaries {
            let Some(k) = self.best(&exclude) else { break };
            let p = self.peak_at(k);
            if 20.0 * (p.mag / noise_s).log10() < self.threshold || p.mag < master.mag * 0.01 {
                break;
            }
            if near_master(k) && p.mag < master.mag * 0.1 {
                exclude.push((k, 60));
                continue;
            }
            found.push(p);
            // Over A and B the secondary codes cancel at every whole-pulse shift: only the peak itself is left out.
            exclude.push((k, 60));
        }
        found.sort_by_key(|x| (x.k + n - mk) % n);
        let letters = ["W", "X", "Y", "Z", "V", "U", "T"];
        let mut stations = vec![LoranStation {
            name: "M",
            td_us: 0.0,
            ecd_us: 0.0,
            snr_db: msnr as f32,
            level_db: 0.0,
        }];
        let cycle = 1e6 / CARRIER_HZ; // 10 µs
        for (i, p) in found.iter().enumerate() {
            // Envelope: samples of 10 µs. Phase: the carrier's phase falls by a cycle for every 10 µs of delay.
            let env_td = ((p.t_env - master.t_env).rem_euclid(n as f64)) * cycle;
            let dphi = (p.value / master.value).arg();
            let phase_td = (-dphi / TAU).rem_euclid(1.0) * cycle;
            let cycles = ((env_td - phase_td) / cycle).round();
            let td = phase_td + cycles * cycle;
            stations.push(LoranStation {
                name: letters[i.min(letters.len() - 1)],
                td_us: td,
                ecd_us: env_td - td,
                snr_db: (20.0 * (p.mag / noise_s).log10()) as f32,
                level_db: (20.0 * (p.mag / master.mag.max(1e-30)).log10()) as f32,
            });
        }
        let chain = self
            .chains
            .iter()
            .find(|c| c.0 == gri)
            .map_or(String::new(), |c| c.1.clone());
        Some((
            LoranFix {
                gri,
                stations,
                chain,
            },
            msnr,
        ))
    }
}

/// Correlates the buffer `buf` of a phase code interval (two GRIs of `gri` samples) with a code (master or secondary), then
/// with the envelope template `tmpl`, into `m`; `|m|` into `mags`. `s` is scratch.
fn correlate_into(
    buf: &[Complex32],
    gri: usize,
    master: bool,
    tmpl: &[f64],
    (s, m, mags): (&mut Vec<Complex64>, &mut Vec<Complex64>, &mut Vec<f64>),
) {
    let l = buf.len();
    s.clear();
    s.resize(l / 2, Complex64::new(0.0, 0.0));
    let pulses = if master { 9 } else { 8 };
    for (k, sk) in s.iter_mut().enumerate() {
        let mut v = Complex64::new(0.0, 0.0);
        for p in 0..pulses {
            let off = (pulse_offset_us(p) / 10.0) as usize;
            let (ca, cb) = if master {
                (MASTER_CODE[0][p], MASTER_CODE[1][p])
            } else {
                (SECONDARY_CODE[0][p], SECONDARY_CODE[1][p])
            };
            let xa = buf[(k + off) % l];
            let xb = buf[(k + gri + off) % l];
            v += Complex64::new(xa.re as f64, xa.im as f64) * ca as f64
                + Complex64::new(xb.re as f64, xb.im as f64) * cb as f64;
        }
        *sk = v;
    }
    let n = s.len();
    m.clear();
    mags.clear();
    for k in 0..n {
        let mut v = Complex64::new(0.0, 0.0);
        for (j, &e) in tmpl.iter().enumerate() {
            v += s[(k + j) % n] * e;
        }
        m.push(v);
        mags.push(v.norm());
    }
}

impl Component for LoranTracker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.loran_tracker",
            vec![Port::iq("in").rate(RateRule::Exact(WORK_RATE_HZ))],
            vec![Port::typed::<LoranFix>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(LoranTrackerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if (f.rate - WORK_RATE_HZ).abs() > 1.0 {
            return Err(ConfigError::WrongRate {
                need: WORK_RATE_HZ,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.every = (self.interval * f.rate).round() as u64;
        self.build();
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<LoranFix>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.iq(0);
        let mut i = 0;
        while i < x.len() {
            let run = ((self.every - self.count) as usize).min(x.len() - i);
            let a = self.alpha;
            for acc in self.accs.iter_mut() {
                acc.add(&x[i..i + run], a);
            }
            self.count += run as u64;
            i += run;
            if self.count >= self.every {
                self.count = 0;
                let time = (first + i as u64) as f64 / self.rate;
                // The GRI: the one set, or the known one whose master stands out most (kept unless another is far better).
                let mut best: Option<(usize, f64)> = None;
                for a in 0..self.accs.len() {
                    if self.accs[a].pcis < 2 {
                        continue;
                    }
                    let snr = self.master_snr(a);
                    let bonus = if self.chosen == Some(self.accs[a].gri) {
                        3.0
                    } else {
                        0.0
                    };
                    if best.is_none_or(|b| snr + bonus > b.1) {
                        best = Some((a, snr + bonus));
                    }
                }
                if let Some((a, _)) = best {
                    if let Some((fix, _)) = self.measure(a) {
                        self.chosen = Some(fix.gri);
                        io.items_out::<LoranFix>(0).push(Timed { time, value: fix });
                    }
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match LoranTrackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LoranTrackerSetting::Gri => {
                self.gri = GRI_SETTING.read(id, v)? as u32;
                return Ok(Applied::Reformat);
            }
            LoranTrackerSetting::Average => {
                self.average = AVERAGE.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            LoranTrackerSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            LoranTrackerSetting::MaxSecondaries => {
                self.max_secondaries = MAX_SECONDARIES.read(id, v)? as usize;
            }
            LoranTrackerSetting::Interval => {
                self.interval = INTERVAL.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            LoranTrackerSetting::Chains => {
                let text = CHAINS.read(id, v)?;
                if text != self.chains_text {
                    self.chains_text = text.to_string();
                    self.chains = parse_chains(text);
                    return Ok(Applied::Reformat);
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (LoranTrackerSetting::Gri, Value::I64(i64::from(self.gri))),
            (LoranTrackerSetting::Average, Value::Duration(self.average)),
            (LoranTrackerSetting::Threshold, Value::F64(self.threshold)),
            (
                LoranTrackerSetting::MaxSecondaries,
                Value::I64(self.max_secondaries as i64),
            ),
            (
                LoranTrackerSetting::Interval,
                Value::Duration(self.interval),
            ),
            (
                LoranTrackerSetting::Chains,
                Value::Text(self.chains_text.clone()),
            ),
        ])
    }
}

fn new_published() -> Published {
    let mut p = crate::typed::store(&[LoranFields::SHAPE], &[&LORAN_STATIONS]);
    put_status(&mut p, LockState::Searching, None, None, None, "Searching");
    p
}

/// The Loran report (`avnav.loran_report`). Its command `clear` forgets the chain.
pub struct LoranReport {
    fixes: u64,
    /// The GRI and the SNR of the master of the last measurement.
    last: Option<(u32, f32)>,
    /// The chain of the last measurement, empty when it has none.
    chain: String,
    published: Published,
    summary: String,
}

impl Default for LoranReport {
    fn default() -> Self {
        LoranReport {
            fixes: 0,
            last: None,
            chain: String::with_capacity(32),
            published: new_published(),
            summary: String::with_capacity(64),
        }
    }
}

impl LoranReport {
    /// Writes the rows of `fix` and removes the rows of stations that it does not hold.
    fn put_stations(&mut self, fix: &LoranFix) {
        let p = &mut self.published;
        for s in &fix.stations {
            LORAN_STATIONS.put(
                p,
                s.name,
                &LoranStationRow {
                    station: s.name,
                    td_us: s.td_us,
                    ecd_us: s.ecd_us,
                    snr_db: f64::from(s.snr_db),
                    level_db: f64::from(s.level_db),
                },
            );
        }
        for name in ["M", "W", "X", "Y", "Z", "V", "U", "T"] {
            if !fix.stations.iter().any(|s| s.name == name) {
                p.remove_row(&LORAN_STATIONS, name);
            }
        }
    }

    /// Writes the result fields from the last measurement.
    fn put_fields(&mut self) {
        let snr = self.last.map(|(_, snr)| snr);
        self.published.put(&LoranFields {
            gri: self.last.map(|(gri, _)| i64::from(gri)),
            chain: Some(self.chain.as_str()).filter(|c| !c.is_empty()),
            master: snr.is_some_and(|s| s >= 10.0),
            master_snr_db: snr.map(f64::from),
            fixes: self.fixes as i64,
        });
    }
}

impl Component for LoranReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("avnav.loran_report", vec![Port::typed::<LoranFix>("fix")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.put_fields();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(f) = io.items::<LoranFix>(0).last() else {
            return;
        };
        let fix = &f.value;
        self.fixes += 1;
        let snr = fix.stations.first().map_or(0.0, |s| s.snr_db);
        let master = snr >= 10.0;
        self.last = Some((fix.gri, snr));
        self.chain.clear();
        self.chain.push_str(&fix.chain);
        self.put_fields();
        self.put_stations(fix);
        let lock_state = if master && fix.stations.len() > 1 {
            LockState::Locked
        } else if master {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        self.summary.clear();
        if master {
            // Writing into a String cannot fail.
            let _ = write!(
                self.summary,
                "GRI {}, {} secondaries",
                fix.gri,
                fix.stations.len() - 1
            );
        } else {
            self.summary.push_str("Searching");
        }
        put_status(
            &mut self.published,
            lock_state,
            Some(snr),
            None,
            None,
            &self.summary,
        );
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.fixes = 0;
                self.last = None;
                self.chain.clear();
                self.published = new_published();
                self.put_fields();
            }
        }
        Ok(None)
    }
}

/// The Loran graph: the channel at exactly 100 ksps (one sample for each carrier cycle) to the tracker and the report.
///
/// # Errors
///
/// A [`GraphError`] if the graph cannot be built.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("chan", GraphError::checked("chan", Channel::new(24e3, WORK_RATE_HZ))?)
        .set("chan.wide", Value::Hz(24e3))
        .set("chan.exact", Value::Bool(true))
        .add("tracker", LoranTracker::default())
        .add("report", LoranReport::default())
        .input("chan")
        .connect("chan", "tracker")
        .connect("tracker", "report")
        .clock(0.5)
        .status_from("report")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

pub const GEN_GRI: Setting<field::I64> = Setting::new("gen_gri", "Test signal: GRI", "", 9960)
    .range(4000.0, 9999.0, 10.0)
    .section("Test signal")
    .advanced();
pub const GEN_TDS: Setting<field::Text> = Setting::new(
    "gen_tds",
    "Test signal: TDs (µs)",
    "",
    "13797.2,26969.93,42221.65",
)
.section("Test signal")
.help("The time differences of the secondaries in µs, separated by commas.")
.advanced();
pub const GEN_ECD: Setting<field::F64> = Setting::new(
    "gen_ecd",
    "Test signal: ECD of the first secondary",
    "us",
    0.0,
)
.range(-5.0, 5.0, 0.0)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of the Loran stack.
    pub enum LoranSetting {
        /// The GRI, 0 to search.
        Gri = GRI_SETTING,
        /// The time of coherent averaging.
        Average = AVERAGE,
        /// The correlation SNR a secondary needs.
        Threshold = THRESHOLD,
        /// The most secondaries looked for.
        MaxSecondaries = MAX_SECONDARIES,
        /// The chains searched and named.
        Chains = CHAINS,
        /// The GRI of the test signal.
        GenGri = GEN_GRI,
        /// The time differences of the test signal.
        GenTds = GEN_TDS,
        /// The ECD of the first secondary of the test signal.
        GenEcd = GEN_ECD,
    }
}

fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "Loran-C needs a sample rate of at least 100 kHz",
        )
        .route("gri", &["tracker.gri"])?
        .route("average", &["tracker.average"])?
        .route("threshold", &["tracker.threshold"])?
        .route("max_secondaries", &["tracker.max_secondaries"])?
        .route("chains", &["tracker.chains"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("report", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The Loran receiver.
pub fn receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(stack_graph(cfg)?))
}

/// One station of the test signal.
#[derive(Clone, Debug)]
pub struct TestStation {
    /// Master (nine pulses, master codes).
    pub master: bool,
    /// Arrival after the master, µs.
    pub td_us: f64,
    /// Envelope delay against the carrier, µs.
    pub ecd_us: f64,
    /// Amplitude.
    pub amplitude: f32,
}

/// The Loran test signal: a chain at baseband (100 kHz at 0 Hz). The master's pulses peak at amplitude 1.
pub struct LoranGenerator {
    rate: f64,
    gri_s: f64,
    stations: Vec<TestStation>,
    start_s: f64,
    phase0: f64,
    n: u64,
}

impl LoranGenerator {
    /// A chain of GRI `gri` with secondaries at `tds` µs.
    pub fn new(rate: f64, gri: u32, tds: &[f64], ecd_us: f64, seed: u64) -> Self {
        let mut stations = vec![TestStation {
            master: true,
            td_us: 0.0,
            ecd_us: 0.0,
            amplitude: 1.0,
        }];
        for (i, &td) in tds.iter().enumerate() {
            stations.push(TestStation {
                master: false,
                td_us: td,
                ecd_us: if i == 0 { ecd_us } else { 0.0 },
                amplitude: (5.0 - i as f32).max(1.0) / 6.0,
            });
        }
        Self::with_stations(rate, gri, stations, seed)
    }
    /// A chain of GRI `gri` with the stations `stations` (the master among them).
    pub fn with_stations(rate: f64, gri: u32, stations: Vec<TestStation>, seed: u64) -> Self {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        LoranGenerator {
            rate,
            gri_s: gri as f64 * 10e-6,
            stations,
            start_s: 1e-3 + rng.uniform() * 1e-3,
            phase0: rng.uniform() * TAU,
            n: 0,
        }
    }
    /// From the stack's settings. A time difference in `gen_tds` that is not a number is left out.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let gri = s.get(&GEN_GRI).clamp(0, i64::from(u32::MAX)) as u32;
        let tds: Vec<f64> = s
            .get(&GEN_TDS)
            .split(',')
            .filter_map(|t| t.trim().parse().ok())
            .collect();
        LoranGenerator::new(cfg.rate_hz, gri, &tds, s.get(&GEN_ECD), cfg.seed)
    }
    /// The signal at time `t`.
    fn at(&self, t: f64) -> Complex32 {
        let mut acc = Complex64::new(0.0, 0.0);
        let rel = t - self.start_s;
        for st in &self.stations {
            let base = rel - st.td_us * 1e-6;
            // Which interval this time falls in (a station's group can run into the next interval).
            let k = (base / self.gri_s).floor();
            for back in 0..2 {
                let kk = k - back as f64;
                if kk < 0.0 {
                    continue;
                }
                let ab = (kk as i64).rem_euclid(2) as usize;
                let t0 = base - kk * self.gri_s;
                let pulses = if st.master { 9 } else { 8 };
                for p in 0..pulses {
                    let tp = t0 - pulse_offset_us(p) * 1e-6;
                    if !(0.0..300e-6).contains(&(tp - st.ecd_us * 1e-6)) {
                        continue;
                    }
                    let code = if st.master {
                        MASTER_CODE[ab][p]
                    } else {
                        SECONDARY_CODE[ab][p]
                    } as f64;
                    let e = envelope(tp - st.ecd_us * 1e-6) * code * st.amplitude as f64;
                    // The carrier's phase at baseband: its delay in cycles of 100 kHz.
                    let arrival = self.start_s
                        + st.td_us * 1e-6
                        + kk * self.gri_s
                        + pulse_offset_us(p) * 1e-6;
                    let ph = self.phase0 - TAU * ((CARRIER_HZ * arrival) % 1.0);
                    acc += Complex64::from_polar(e, ph);
                }
            }
        }
        Complex32::new(acc.re as f32, acc.im as f32)
    }
}

impl Generator for LoranGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The energy of one pulse of peak 1 is the integral of envelope^2: 65 µs e^4 4! / 4^5.
        let pulse = PULSE_RISE_S * 4f64.exp() * 24.0 / 1024.0;
        let per_gri: f64 = self
            .stations
            .iter()
            .map(|s| if s.master { 9.0 } else { 8.0 } * (s.amplitude as f64).powi(2) * pulse)
            .sum();
        Some(SignalInfo::new(20e3).with_power(per_gri / self.gri_s))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            *o = self.at(self.n as f64 / self.rate);
            self.n += 1;
        }
        out.len()
    }
}

/// Makes the Loran generator.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(LoranGenerator::from_config(cfg)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn envelope_peaks_at_65_us() {
        assert!((envelope(65e-6) - 1.0).abs() < 1e-12);
        assert!(envelope(30e-6) < envelope(65e-6));
        assert_eq!(envelope(-1.0), 0.0);
    }
    #[test]
    fn codes_balance() {
        // Over A and B every pulse position sums to 0 or ±2, and the codes of a secondary and the master differ.
        for p in 0..8 {
            assert!(SECONDARY_CODE[0][p] + SECONDARY_CODE[1][p] != 1);
        }
    }
}
