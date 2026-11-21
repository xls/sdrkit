//! The carrier search: averaged Hann-windowed FFTs every 0.5 s of signal, a median noise floor, and the carriers that look like
//! sondes.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Values;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Item, Port, RateFrom, Timed};

/// One carrier found by a search.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Carrier {
    /// Offset from the centre, Hz.
    pub offset_hz: f64,
    /// Level, dB over the floor in about 10 kHz.
    pub level_db: f64,
}

/// The carriers of one search, strongest first, with the noise it measured.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct CarrierList {
    /// At most 32 carriers.
    pub carriers: Vec<Carrier>,
    /// Noise variance per input sample.
    pub noise_var: f64,
    /// The input rate.
    pub rate: f64,
}

impl Item for CarrierList {
    const KIND: &'static str = "sonde.carriers";
}

/// The FFT size for a rate: the smallest power of two from 256 with bins of 1.3 kHz or finer (at most 2^17).
pub fn fft_size(rate: f64) -> usize {
    let mut n = 256usize;
    while rate / n as f64 > 1300.0 && n < (1 << 17) {
        n *= 2;
    }
    n
}

/// The analysis of one averaged power spectrum `p` (lowest frequency first, centre bin N/2). `p` is modified (DC bridged).
/// `scale` widens the box, the separation and the centroid span (1 for the 400 MHz sondes, 10 for the wide LMS6 in L band).
pub fn analyse(
    p: &mut [f32],
    rate: f64,
    win_energy: f64,
    scratch: &mut Vec<f32>,
    scale: f64,
) -> CarrierList {
    let n = p.len();
    let c = n / 2;
    let df = rate / n as f64;
    if n >= 16 {
        let fill = 0.5 * (p[c - 4] + p[c + 4]);
        for v in &mut p[c - 1..=c + 1] {
            *v = fill;
        }
    }
    let floor = percentile(&p[n * 5 / 100..n * 95 / 100], 0.5, scratch).max(1e-30) as f64;
    let noise_var = floor / win_energy;
    // Box score over ±4 kHz.
    let hw4 = ((4000.0 * scale / df).round() as usize).max(1);
    let hw6 = ((6000.0 * if scale > 1.0 { 17.0 } else { 1.0 } / df).round() as usize).max(1);
    let hw3 = ((3000.0 * scale / df).round() as usize).max(1);
    let lim = ((0.45 * rate / df) as usize).min(c - 1);
    let (lo, hi) = (c - lim, c + lim);
    let mut cum = vec![0f64; n + 1];
    for i in 0..n {
        cum[i + 1] = cum[i] + p[i] as f64;
    }
    let boxm = |i: usize, hw: usize| {
        let a = i.saturating_sub(hw);
        let b = (i + hw + 1).min(n);
        (cum[b] - cum[a]) / (b - a) as f64
    };
    let mut cands: Vec<(f64, usize)> = Vec::new();
    let thr = 10f64.powf(0.6);
    for i in lo.max(1)..hi.min(n - 1) {
        let s = boxm(i, hw4) / floor;
        if s > thr && s >= boxm(i - 1, hw4) / floor && s >= boxm(i + 1, hw4) / floor {
            cands.push((s, i));
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0));
    let sep = (14000.0 * if scale > 1.0 { 15.0 } else { 1.0 } / df).ceil() as usize;
    let mut taken: Vec<usize> = Vec::new();
    let mut out = CarrierList {
        carriers: Vec::new(),
        noise_var,
        rate,
    };
    for &(_, i) in &cands {
        if taken.iter().any(|&t| t.abs_diff(i) < sep) {
            continue;
        }
        taken.push(i);
        // The spur test always looks at ±6 kHz: a narrow unmodulated carrier is not a sonde. A carrier is one line when its
        // strongest bin and its two neighbours hold more than 90 % of the power over the floor in ±6 kHz (a Hann window keeps 93 %
        // to all of a line there). An FSK of index 1 such as M10 also has a carrier line, but its sidebands hold a large share.
        let hws = ((6000.0 / df).round() as usize).max(1);
        let (sa, sb) = (i.saturating_sub(hws), (i + hws + 1).min(n));
        let excess = |a: usize, b: usize| -> f64 {
            p[a..b].iter().map(|&v| (v as f64 - floor).max(0.0)).sum()
        };
        let m = (sa..sb).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap_or(i);
        let line = excess(m.saturating_sub(1).max(sa), (m + 2).min(sb));
        if line > 0.9 * excess(sa, sb) {
            continue;
        }
        let a = i.saturating_sub(hw6);
        let b = (i + hw6 + 1).min(n);
        let (mut sw, mut swf) = (0f64, 0f64);
        for (k, &v) in p.iter().enumerate().take(b).skip(a) {
            let w = (v as f64 - floor).max(0.0);
            sw += w;
            swf += w * (k as f64 - c as f64) * df;
        }
        if sw <= 0.0 {
            continue;
        }
        let a3 = i.saturating_sub(hw3);
        let b3 = (i + hw3 + 1).min(n);
        let ex: f64 = p[a3..b3].iter().map(|&v| (v as f64 - floor).max(0.0)).sum();
        let level = 10.0 * (ex / (floor * (b3 - a3) as f64)).max(1e-12).log10();
        out.carriers.push(Carrier {
            offset_hz: swf / sw,
            level_db: level,
        });
        if out.carriers.len() >= 32 {
            break;
        }
    }
    out
}

/// The carrier search component (`sonde_search`, iq → `sonde.carriers`).
pub struct CarrierSearch {
    rate: f64,
    n: usize,
    win: Vec<f32>,
    win_energy: f64,
    fft: Option<Fft>,
    buf: Vec<Complex32>,
    acc: Vec<f32>,
    fill: usize,
    ffts: usize,
    period: u64,
    next_start: u64,
    active: bool,
    scratch: Vec<f32>,
    pbuf: Vec<f32>,
    scale: f64,
    /// Searches done.
    pub searches: u64,
}

impl Default for CarrierSearch {
    fn default() -> Self {
        CarrierSearch {
            rate: 0.0,
            n: 256,
            win: Vec::new(),
            win_energy: 1.0,
            fft: None,
            buf: Vec::new(),
            acc: Vec::new(),
            fill: 0,
            ffts: 0,
            period: 1,
            next_start: 0,
            active: false,
            scratch: Vec::new(),
            pbuf: Vec::new(),
            scale: 1.0,
            searches: 0,
        }
    }
}

impl CarrierSearch {
    fn restart(&mut self, at: u64) {
        self.next_start = at + (0.05 * self.rate) as u64;
        self.active = false;
        self.fill = 0;
        self.ffts = 0;
        self.acc.iter_mut().for_each(|v| *v = 0.0);
    }
}

impl Component for CarrierSearch {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sonde.search",
            vec![Port::iq("in")],
            vec![Port::typed::<CarrierList>("carriers").rate_from(RateFrom::Fixed(0.0))],
        )
        .manual_tags()
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.scale = if inputs[0].centre_hz > 1500e6 {
            10.0
        } else {
            1.0
        };
        self.n = fft_size(self.rate);
        self.win = Window::Hann.coefficients(self.n);
        self.win_energy = self.win.iter().map(|&w| (w * w) as f64).sum();
        self.fft = Some(Fft::forward(self.n));
        self.buf = vec![Complex32::new(0.0, 0.0); self.n];
        self.acc = vec![0.0; self.n];
        self.period = (0.5 * self.rate) as u64;
        self.restart(0);
        Ok(vec![Format::new(
            sdrkit_graph::Kind::typed::<CarrierList>(),
            0.0,
        )])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned { centre_hz, .. } = t.kind {
                self.scale = if centre_hz > 1500e6 { 10.0 } else { 1.0 };
                self.restart(t.at);
            }
        }
        let n = self.n;
        let mut i = 0usize;
        while i < x.len() {
            let at = first + i as u64;
            if !self.active {
                if at < self.next_start {
                    i += (self.next_start - at).min((x.len() - i) as u64) as usize;
                    continue;
                }
                self.active = true;
                self.fill = 0;
                self.ffts = 0;
                self.acc.iter_mut().for_each(|v| *v = 0.0);
            }
            let Some(fft) = self.fft.as_mut() else { return };
            let take = (n - self.fill).min(x.len() - i);
            for k in 0..take {
                self.buf[self.fill + k] = x[i + k] * self.win[self.fill + k];
            }
            self.fill += take;
            i += take;
            if self.fill == n {
                fft.process(&mut self.buf);
                for k in 0..n {
                    let src = (k + n / 2) % n;
                    self.acc[k] += self.buf[src].norm_sqr();
                }
                self.fill = 0;
                self.ffts += 1;
                if self.ffts == 32 {
                    self.pbuf.clear();
                    self.pbuf.extend(self.acc.iter().map(|v| v / 32.0));
                    let list = analyse(
                        &mut self.pbuf,
                        self.rate,
                        self.win_energy,
                        &mut self.scratch,
                        self.scale,
                    );
                    self.searches += 1;
                    let time = (first + i as u64) as f64 / self.rate;
                    io.items_out::<CarrierList>(0)
                        .push(Timed { time, value: list });
                    self.active = false;
                    self.next_start += self.period;
                    let now = first + i as u64;
                    if self.next_start < now {
                        self.next_start = now;
                    }
                }
            }
        }
    }
}
