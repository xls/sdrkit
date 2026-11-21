//! The Wireless M-Bus channel chains: the channel plan, one front end, discriminator and chip clock for each channel, and the
//! deframer of each physical mode.
//!
//! Standard: EN 13757-4 (physical modes S, T, C, R2 and N)

use super::link::{air_len, strip_crcs, Format as FrameFormat};
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_algo_dsp::iir::{BiquadCoeffs, ComplexBiquad, BUTTERWORTH4_Q};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Complex32;
use sdrkit_graph::Item;
use sdrkit_util_bits::line::{three_of_six, Manchester, ManchesterConv};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// A physical mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phy {
    /// S1, Manchester at 32 768 chip/s.
    S1,
    /// T1 and C1 on 868.95 MHz at 100 kchip/s.
    T1C1,
    /// R2, Manchester at 4800 chip/s.
    R2,
    /// N1, NRZ at 4800 bit/s.
    N1,
}

impl Phy {
    /// Chip rate.
    pub fn chip_rate(self) -> f64 {
        match self {
            Phy::S1 => 32768.0,
            Phy::T1C1 => 100_000.0,
            Phy::R2 | Phy::N1 => 4800.0,
        }
    }
    /// Deviation (Hz).
    pub fn dev_hz(self) -> f64 {
        match self {
            Phy::S1 | Phy::T1C1 => 50e3,
            Phy::R2 => 6e3,
            Phy::N1 => 2.4e3,
        }
    }
    /// The 16-chip sync word.
    pub fn sync(self) -> u16 {
        match self {
            Phy::S1 | Phy::R2 => 0x7696,
            Phy::T1C1 => 0x543D,
            Phy::N1 => 0xF68D,
        }
    }
    /// Its name in channel labels.
    pub fn label(self) -> &'static str {
        match self {
            Phy::S1 => "S1",
            Phy::T1C1 => "T1 C1",
            Phy::R2 => "R2",
            Phy::N1 => "N1",
        }
    }
    /// Wide (S, T, C) or narrow (R2, N1).
    pub fn wide(self) -> bool {
        matches!(self, Phy::S1 | Phy::T1C1)
    }
}

/// A channel of the plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    /// Frequency (Hz).
    pub hz: f64,
    /// Its PHY.
    pub phy: Phy,
}

impl Channel {
    /// "868.3000 MHz S1".
    pub fn label(&self) -> String {
        format!("{:.4} MHz {}", self.hz / 1e6, self.phy.label())
    }
}

/// The plan for a radio centre (0 = unknown, the 868 MHz plan centred on 868.625 MHz) and a rate.
pub fn plan(centre: f64, rate: f64) -> Vec<Channel> {
    let c = if centre <= 0.0 { 868.625e6 } else { centre };
    let all: Vec<Channel> = if c < 300e6 {
        (0..6)
            .map(|k| Channel {
                hz: 169.40625e6 + 12.5e3 * k as f64,
                phy: Phy::N1,
            })
            .collect()
    } else {
        vec![
            Channel {
                hz: 868.3e6,
                phy: Phy::S1,
            },
            Channel {
                hz: 868.95e6,
                phy: Phy::T1C1,
            },
            Channel {
                hz: 868.03e6,
                phy: Phy::R2,
            },
            Channel {
                hz: 868.09e6,
                phy: Phy::R2,
            },
            Channel {
                hz: 868.15e6,
                phy: Phy::R2,
            },
        ]
    };
    all.into_iter()
        .filter(|ch| (ch.hz - c).abs() + if ch.phy.wide() { 150e3 } else { 12e3 } <= 0.46 * rate)
        .collect()
}

/// One link-layer frame (`wmbus.frame`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WmbusFrame {
    /// PHY name: "S1", "T1", "C1 (A)", "C1 (B)", "R2", "N1".
    pub phy: String,
    /// Channel frequency (Hz).
    pub channel_hz: f64,
    /// The bytes on air (CRCs included).
    pub air: Vec<u8>,
    /// The bytes after CRC removal when every CRC passed.
    pub data: Option<Vec<u8>>,
    /// Level (dBFS).
    pub level_db: f32,
    /// Carrier offset (Hz).
    pub offset_hz: f64,
    /// Signal time (s).
    pub time: f64,
    /// Soft chips of the frame (±1 nominal deviation), at most 1000.
    pub trace: Vec<f32>,
}

impl Item for WmbusFrame {
    const KIND: &'static str = "wmbus.frame";
}

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Search,
    /// Collecting: (format, coding: 0 Manchester, 1 3-of-6, 2 NRZ).
    Collect,
}

/// A chip clock and deframer for one PHY family on a channel.
#[derive(Clone, Debug)]
pub struct Deframer {
    /// The PHY.
    pub phy: Phy,
    spc: f64,
    prev: f32,
    hist: u64,
    nchips: u64,
    mode: Mode,
    inverted: bool,
    pending: Vec<u8>,
    bytes: Vec<u8>,
    total: usize,
    format: FrameFormat,
    coding: u8,
    name: &'static str,
    nib: Vec<u8>,
    bit: Vec<u8>,
    trace: Vec<f32>,
    /// Frame level and offset noted at sync.
    level: f32,
    offset: f64,
    start_time: f64,
    soft_hist: [f32; 48],
    adj: f32,
    /// Counters.
    pub syncs: u64,
    /// False syncs.
    pub false_syncs: u64,
}

impl Deframer {
    fn new(phy: Phy, rate: f64) -> Deframer {
        Deframer {
            phy,
            spc: rate / phy.chip_rate(),
            prev: 0.0,
            hist: 0,
            nchips: 0,
            mode: Mode::Search,
            inverted: false,
            pending: vec![],
            bytes: vec![],
            total: 0,
            format: FrameFormat::A,
            coding: 0,
            name: "",
            nib: vec![],
            bit: Vec::with_capacity(1),
            trace: vec![],
            level: 0.0,
            offset: 0.0,
            start_time: 0.0,
            soft_hist: [0.0; 48],
            adj: 0.0,
            syncs: 0,
            false_syncs: 0,
        }
    }

    fn abort(&mut self, false_sync: bool) {
        if false_sync {
            self.false_syncs += 1;
        }
        self.mode = Mode::Search;
        self.pending.clear();
        self.bytes.clear();
        self.nib.clear();
        self.trace.clear();
    }

    fn push_byte(&mut self, b: u8) -> bool {
        self.bytes.push(b);
        if self.bytes.len() == 1 {
            if b < 9 {
                self.abort(true);
                return false;
            }
            match air_len(b, self.format) {
                Some(t) if t <= 300 => self.total = t,
                _ => {
                    self.abort(true);
                    return false;
                }
            }
        }
        true
    }

    /// One soft chip (positive = upper frequency). Returns a finished frame's bytes.
    fn chip(&mut self, soft: f32) -> Option<(Vec<u8>, &'static str)> {
        let soft = if self.mode == Mode::Collect {
            soft - self.adj
        } else {
            soft
        };
        self.soft_hist[(self.nchips % 48) as usize] = soft;
        let hard = (soft > 0.0) as u8;
        self.hist = (self.hist << 1) | hard as u64;
        self.nchips += 1;
        match self.mode {
            Mode::Search => {
                if self.nchips < 48 {
                    return None;
                }
                let last = (self.hist & 0xFFFF) as u16;
                let sync = self.phy.sync();
                let e = (last ^ sync).count_ones();
                let ei = (!last ^ sync).count_ones();
                if e > 1 && ei > 1 {
                    return None;
                }
                let pre = (self.hist >> 16) & 0xFFFF_FFFF;
                let trans = ((pre ^ (pre >> 1)) & 0x7FFF_FFFF).count_ones();
                if trans < 26 {
                    return None;
                }
                self.inverted = ei < e;
                self.syncs += 1;
                // The preamble alternates, so its mean is the residual carrier offset.
                let mut acc = 0.0;
                for k in 16..48u64 {
                    acc += self.soft_hist[((self.nchips - 1 - k) % 48) as usize];
                }
                self.adj = acc / 32.0;
                self.mode = Mode::Collect;
                self.pending.clear();
                self.bytes.clear();
                self.nib.clear();
                self.trace.clear();
                self.total = usize::MAX;
                let (fmt, coding, name) = match self.phy {
                    Phy::S1 => (FrameFormat::A, 0, "S1"),
                    Phy::R2 => (FrameFormat::A, 0, "R2"),
                    Phy::N1 => (FrameFormat::A, 2, "N1"),
                    Phy::T1C1 => (FrameFormat::A, 9, "T1"),
                };
                self.format = fmt;
                self.coding = coding;
                self.name = name;
                None
            }
            Mode::Collect => {
                let c = if self.inverted { 1 - hard } else { hard };
                if self.trace.len() < 1000 {
                    self.trace.push(if self.inverted { -soft } else { soft });
                }
                self.pending.push(c);
                match self.coding {
                    9 => {
                        // T or C: decide after 8 (or 16) chips.
                        if self.pending.len() == 8 {
                            let v = self.pending.iter().fold(0u8, |a, &b| (a << 1) | b);
                            if v != 0x54 {
                                self.coding = 1;
                                self.name = "T1";
                                let p = std::mem::take(&mut self.pending);
                                for ch in p {
                                    if let Some(f) = self.feed_3of6(ch) {
                                        return Some(f);
                                    }
                                    if self.mode == Mode::Search {
                                        return None;
                                    }
                                }
                            }
                        } else if self.pending.len() == 16 {
                            let v = self.pending[8..].iter().fold(0u8, |a, &b| (a << 1) | b);
                            self.pending.clear();
                            match v {
                                0xCD => {
                                    self.coding = 2;
                                    self.format = FrameFormat::A;
                                    self.name = "C1 (A)";
                                }
                                0x3D => {
                                    self.coding = 2;
                                    self.format = FrameFormat::B;
                                    self.name = "C1 (B)";
                                }
                                _ => self.abort(true),
                            }
                        }
                        None
                    }
                    1 => {
                        self.pending.pop();
                        self.feed_3of6(c)
                    }
                    0 => {
                        if self.pending.len() == 2 {
                            // Manchester, 1 is chips 10: a pair without a change ends the frame as a false sync.
                            self.bit.clear();
                            let pair = Manchester::decode(
                                &self.pending,
                                ManchesterConv::Thomas,
                                &mut self.bit,
                            );
                            self.pending.clear();
                            if pair.is_err() {
                                self.abort(true);
                                return None;
                            }
                            self.nib.push(self.bit[0]);
                            if self.nib.len() == 8 {
                                let v = self.nib.iter().fold(0u8, |x, &y| (x << 1) | y);
                                self.nib.clear();
                                if !self.push_byte(v) {
                                    return None;
                                }
                                return self.done();
                            }
                        }
                        None
                    }
                    _ => {
                        if self.pending.len() == 8 {
                            let v = self.pending.iter().fold(0u8, |x, &y| (x << 1) | y);
                            self.pending.clear();
                            if !self.push_byte(v) {
                                return None;
                            }
                            return self.done();
                        }
                        None
                    }
                }
            }
        }
    }

    fn feed_3of6(&mut self, ch: u8) -> Option<(Vec<u8>, &'static str)> {
        self.nib.push(ch);
        if self.nib.len() == 6 {
            let v = self.nib.iter().fold(0u8, |x, &y| (x << 1) | y);
            self.nib.clear();
            let Some(n) = three_of_six::decode(v) else {
                self.abort(true);
                return None;
            };
            self.pending.push(n);
            if self.pending.len() == 2 {
                let b = (self.pending[0] << 4) | self.pending[1];
                self.pending.clear();
                if !self.push_byte(b) {
                    return None;
                }
                return self.done();
            }
        }
        None
    }

    fn done(&mut self) -> Option<(Vec<u8>, &'static str)> {
        if self.bytes.len() >= self.total {
            let b = std::mem::take(&mut self.bytes);
            self.mode = Mode::Search;
            self.pending.clear();
            self.nib.clear();
            return Some((b, self.name));
        }
        None
    }
}

/// One channel's chain: front end, discriminator and the deframers of its PHYs.
pub struct Chain {
    /// The channel.
    pub ch: Channel,
    rate_in: f64,
    /// Working rate.
    pub rate: f64,
    nco: Nco,
    mixed: Vec<Complex32>,
    ratio: f64,
    boundary: f64,
    pos: f64,
    sum: Complex32,
    bq: [ComplexBiquad; 2],
    prev: Complex32,
    dc: f32,
    dc_k: f32,
    /// Slow power.
    pub slow: f32,
    /// Fast power.
    pub fast: f32,
    fast_k: f32,
    /// Noise floor.
    pub floor: f32,
    low_run: f32,
    n: u64,
    dev_rad: f32,
    deframers: Vec<(Deframer, f64, f32, f32)>,
    /// Good frames.
    pub good: u64,
    /// CRC failures.
    pub bad: u64,
}

impl Chain {
    /// A chain for `ch` at the input rate `rate_in`, the input centre `centre`.
    pub fn new(ch: Channel, rate_in: f64, centre: f64) -> Chain {
        let rate = if ch.phy.wide() { 400e3 } else { 48e3 };
        let fc = match ch.phy {
            Phy::S1 | Phy::T1C1 => 140e3,
            Phy::R2 => 14e3,
            Phy::N1 => 5.5e3,
        };
        let spc = rate / ch.phy.chip_rate();
        let c = if centre <= 0.0 { 868.625e6 } else { centre };
        let mut deframers = vec![(Deframer::new(ch.phy, rate), 0.0, 0.0, 0.0)];
        deframers[0].1 = 0.0;
        Chain {
            nco: Nco::new(rate_in, c - ch.hz),
            mixed: Vec::new(),
            ratio: rate_in / rate,
            boundary: rate_in / rate,
            pos: 0.0,
            sum: Complex32::new(0.0, 0.0),
            // A fourth-order Butterworth low-pass as two sections.
            bq: [
                ComplexBiquad::from_real(BiquadCoeffs::lowpass(rate, fc, BUTTERWORTH4_Q[0])),
                ComplexBiquad::from_real(BiquadCoeffs::lowpass(rate, fc, BUTTERWORTH4_Q[1])),
            ],
            prev: Complex32::new(0.0, 0.0),
            dc: 0.0,
            dc_k: 1.0 / (8.0 * spc as f32),
            slow: 0.0,
            fast: 0.0,
            fast_k: 1.0 / (16.0 * spc as f32),
            floor: 1.0,
            low_run: f32::MAX,
            n: 0,
            dev_rad: (TAU * ch.phy.dev_hz() / rate) as f32,
            deframers,
            good: 0,
            bad: 0,
            ch,
            rate_in,
            rate,
        }
    }

    /// Feeds input samples; finished frames go to `out`. `first` is the input index of `x[0]`.
    pub fn feed(&mut self, x: &[Complex32], first: u64, out: &mut Vec<WmbusFrame>) {
        self.mixed.resize(x.len(), Complex32::default());
        self.nco.mix(x, &mut self.mixed);
        for i in 0..x.len() {
            let s = self.mixed[i];
            self.pos += 1.0;
            if self.pos < self.boundary {
                self.sum += s;
                continue;
            }
            let frac = (1.0 - (self.pos - self.boundary)) as f32;
            let y0 = (self.sum + s * frac) / self.ratio as f32;
            self.sum = s * (1.0 - frac);
            self.boundary += self.ratio;
            let y1 = self.bq[0].process_sample(y0);
            let y = self.bq[1].process_sample(y1);
            let step = y * self.prev.conj();
            let d = atan2(step.im, step.re);
            self.prev = y;
            let p = y.norm_sqr();
            self.n += 1;
            self.slow += (p - self.slow) / 2000.0;
            self.fast += (p - self.fast) * self.fast_k;
            self.low_run = self.low_run.min(self.slow);
            if self.n > 8000 && self.n.is_multiple_of(256) {
                self.floor = (self.floor * 1.0003).min(self.low_run).max(1e-12);
                self.low_run = f32::MAX;
            } else if self.n == 8000 {
                self.floor = self.slow.max(1e-12);
            }
            let collecting = self.deframers.iter().any(|df| df.0.mode == Mode::Collect);
            if !collecting {
                self.dc += (d - self.dc) * self.dc_k;
            }
            let v = d - self.dc;
            let t = (first + i as u64) as f64 / self.rate_in;
            let dev = self.dev_rad;
            let rate = self.rate;
            let dc = self.dc;
            let fast = self.fast;
            for k in 0..self.deframers.len() {
                let (df, phase, acc, accn) = &mut self.deframers[k];
                let spc = df.spc;
                // clock recovery: pull toward the chip edge at a sign change
                if (v > 0.0) != (df.prev > 0.0) {
                    let err = if *phase < 0.5 { *phase } else { *phase - 1.0 };
                    *phase -= 0.12 * err;
                }
                df.prev = v;
                if (0.25..0.75).contains(phase) {
                    // A discriminator click (a 2 pi slip near threshold) is clamped so one sample cannot flip a chip.
                    *acc += v.clamp(-1.5 * dev, 1.5 * dev);
                    *accn += 1.0;
                }
                *phase += 1.0 / spc;
                if *phase >= 1.0 {
                    *phase -= 1.0;
                    let soft = if *accn > 0.0 {
                        *acc / *accn / dev
                    } else {
                        v / dev
                    };
                    *acc = 0.0;
                    *accn = 0.0;
                    let was = df.mode.clone();
                    let r = df.chip(soft);
                    if was == Mode::Search && df.mode == Mode::Collect {
                        df.level = (10.0 * (fast.max(1e-20) as f64).log10()) as f32;
                        df.offset = dc as f64 * rate / TAU;
                        df.start_time = t;
                    }
                    if let Some((air, name)) = r {
                        let fmt = if name == "C1 (B)" {
                            FrameFormat::B
                        } else {
                            FrameFormat::A
                        };
                        let data = strip_crcs(&air, fmt);
                        if data.is_some() {
                            self.good += 1;
                        } else {
                            self.bad += 1;
                        }
                        out.push(WmbusFrame {
                            phy: name.into(),
                            channel_hz: self.ch.hz,
                            air,
                            data,
                            level_db: df.level,
                            offset_hz: df.offset,
                            time: df.start_time,
                            trace: std::mem::take(&mut df.trace),
                        });
                    }
                }
            }
        }
    }

    /// The deframers' sync counts.
    pub fn syncs(&self) -> (u64, u64) {
        self.deframers
            .iter()
            .fold((0, 0), |a, d| (a.0 + d.0.syncs, a.1 + d.0.false_syncs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_at_1_2_msps() {
        let p = plan(868.625e6, 1.2e6);
        let l: Vec<String> = p.iter().map(|c| c.label()).collect();
        assert_eq!(
            l,
            vec![
                "868.3000 MHz S1",
                "868.9500 MHz T1 C1",
                "868.0900 MHz R2",
                "868.1500 MHz R2"
            ]
        );
    }
}
