//! One Inmarsat-C channel: the channel filter, the carrier search on the squared signal, the matched filter, symbol timing, the
//! carrier loop, the soft symbol ring and the unique word frame finder with the one-flip polarity fit.

use sdrkit_algo_dsp::meter::power_to_db as db;
use super::coding::{
    self, FrameDecoder, FRAME_BYTES, FRAME_SYMBOLS, ROWS, ROW_SYMBOLS, SYMBOL_RATE,
};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::peak::parabolic;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas};
use sdrkit_algo_modem::timing::{Gardner, SymbolSync, SymbolSyncConfig};
use sdrkit_core::Complex32;

/// A frame found and decoded by a channel.
#[derive(Clone, Debug)]
pub struct CFrame {
    pub bytes: [u8; FRAME_BYTES],
    /// Unique word errors of the fit.
    pub uw_errors: usize,
    pub symbol_errors: usize,
    pub slip: bool,
    pub esn0_db: f64,
    /// Carrier offset from the channel's nominal place, Hz.
    pub carrier_hz: f64,
    /// Seconds of input (at the chain's input) of the frame's first symbol.
    pub time: f64,
}

/// The state of a channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChainStatus {
    pub state: u8,
    pub carrier_found: bool,
    pub carrier_hz: f64,
    pub esn0_db: f64,
    pub uw_last: usize,
    pub frames_found: u64,
    pub polarity_slips: u64,
    pub drift_hz_s: f64,
    pub ser: f64,
}

const LONG_N: usize = 32_768;
const THRESH: [(usize, f32); 5] = [(2, 6.9), (4, 4.1), (8, 3.2), (16, 2.5), (32, 1.8)];

/// The carrier search on the squared signal over +-`span` Hz.
struct SquareSearch {
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    acc: Vec<f32>,
    blocks: usize,
    block: Vec<Complex32>,
    rate: f64,
    span: f64,
}

impl SquareSearch {
    fn new(rate: f64, span: f64) -> Self {
        SquareSearch {
            fft: Fft::forward(LONG_N),
            win: Window::Hann.coefficients(LONG_N),
            buf: Vec::with_capacity(LONG_N),
            acc: vec![0.0; LONG_N],
            blocks: 0,
            block: Vec::with_capacity(LONG_N),
            rate,
            span,
        }
    }
    fn reset(&mut self) {
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.blocks = 0;
        self.block.clear();
    }
    /// Feeds samples; returns the carrier offset (Hz) once a line passes the threshold.
    fn push(&mut self, x: Complex32) -> Option<f64> {
        self.block.push(x * x);
        if self.block.len() < LONG_N {
            return None;
        }
        self.buf.clear();
        self.buf
            .extend(self.block.iter().zip(self.win.iter()).map(|(a, w)| a * *w));
        self.block.clear();
        self.fft.process(&mut self.buf);
        for (a, b) in self.acc.iter_mut().zip(self.buf.iter()) {
            *a += b.norm_sqr();
        }
        self.blocks += 1;
        let thr = THRESH.iter().find(|t| t.0 == self.blocks).map(|t| t.1);
        let r = thr.and_then(|t| self.evaluate(t));
        if r.is_some() || self.blocks >= 32 {
            self.reset();
        }
        r
    }
    fn evaluate(&self, thr: f32) -> Option<f64> {
        let n = LONG_N;
        let bin = self.rate / n as f64;
        let kmax = ((2.0 * self.span / bin) as usize).min(n / 2 - 70);
        let at = |k: i64| -> f32 { self.acc[k.rem_euclid(n as i64) as usize] };
        let s3 = |k: i64| at(k - 1) + at(k) + at(k + 1);
        let mut best = (0i64, 0f32);
        for k in -(kmax as i64)..=kmax as i64 {
            let v = s3(k);
            if v > best.1 {
                best = (k, v);
            }
        }
        let k = best.0;
        let mut around = 0.0;
        let mut cnt = 0;
        for d in 6..60i64 {
            around += s3(k + d) + s3(k - d);
            cnt += 2;
        }
        let floor = around / cnt as f32;
        if best.1 > thr * floor {
            let p = parabolic(db(at(k - 1) as f64), db(at(k) as f64), db(at(k + 1) as f64));
            Some((k as f64 + p.offset) * bin / 2.0)
        } else {
            None
        }
    }
}

enum Sync {
    Searching {
        checked: u64,
    },
    Locked {
        next: u64,
        invert: bool,
        misses: u32,
    },
}

/// One channel.
pub struct CChain {
    in_rate: f64,
    /// Offset of the channel from the input centre, Hz.
    pub offset: f64,
    chan: Channelizer,
    r1: f64,
    search: SquareSearch,
    pub carrier: Option<f64>,
    nco: Nco,
    mf: FirDecimator<Complex32>,
    r2: f64,
    timing: SymbolSync<Complex32, Gardner, Cubic>,
    costas: CarrierSync<Costas<2>>,
    lock_metric: f32,
    held_s: f64,
    lost_s: f64,
    ring: Vec<f32>,
    /// Absolute index of the next symbol.
    sym_index: u64,
    sync: Sync,
    decoder: FrameDecoder,
    frame: Vec<f32>,
    t1: Vec<Complex32>,
    t2: Vec<Complex32>,
    syms: Vec<Complex32>,
    /// Samples at the chain's input seen.
    in_samples: u64,
    pub status: ChainStatus,
    carrier_track: Vec<(f64, f64)>,
    last_track_t: f64,
}

const RING: usize = 16_384;

/// The carrier loop: 15 Hz wide, and free to follow a satellite's Doppler drift for up to 300 Hz from where the search found the
/// carrier.
fn carrier_config() -> CarrierConfig {
    let mut c = CarrierConfig::new(15.0 / SYMBOL_RATE);
    c.max_freq = 300.0 / SYMBOL_RATE;
    c
}

/// The channel filter keeps the signal and the carrier search span: +-24 kHz around the channel.
const CHANNEL_BW: f64 = 48_000.0;

impl CChain {
    /// A channel `offset` Hz from the input centre, for input at `rate` (at least 96 ksps).
    pub fn new(rate: f64, offset: f64) -> Self {
        let chan = Channelizer::new(ChannelConfig::new(rate, offset, CHANNEL_BW, 96_000.0))
            .expect("the Inmarsat-C channel fits any rate the stack accepts");
        let r1 = chan.output_rate();
        let m = ((r1 / 10_000.0).round() as usize).max(1);
        let r2 = r1 / m as f64;
        let mf = taps(Shape::Rrc { beta: 1.0 }, r1 / SYMBOL_RATE, 6.0, Norm::Dc)
            .expect("a valid RRC pulse");
        let mut tc = SymbolSyncConfig::new(r2 / SYMBOL_RATE);
        tc.bn_t = 0.005;
        tc.pulse = Shape::Rc { beta: 1.0 };
        CChain {
            in_rate: rate,
            offset,
            chan,
            r1,
            search: SquareSearch::new(r1, 20_000.0),
            carrier: None,
            nco: Nco::new(r1, 0.0),
            mf: FirDecimator::new(&mf, m),
            r2,
            timing: SymbolSync::new(Gardner, Cubic, tc).expect("valid timing loop"),
            costas: CarrierSync::new(Costas::<2>, carrier_config()).expect("valid carrier loop"),
            lock_metric: 0.0,
            held_s: 0.0,
            lost_s: 0.0,
            ring: vec![0.0; RING],
            sym_index: 0,
            sync: Sync::Searching { checked: 0 },
            decoder: FrameDecoder::default(),
            frame: vec![0.0; FRAME_SYMBOLS],
            t1: Vec::new(),
            t2: Vec::new(),
            syms: Vec::new(),
            in_samples: 0,
            status: ChainStatus::default(),
            carrier_track: Vec::new(),
            last_track_t: 0.0,
        }
    }

    /// Moves the channel (the filter state is kept), dropping the carrier.
    pub fn retune(&mut self, offset: f64) {
        self.offset = offset;
        self.chan.set_offset(offset);
        self.drop_carrier();
    }

    fn drop_carrier(&mut self) {
        self.carrier = None;
        self.search.reset();
        self.status.carrier_found = false;
        self.sync = Sync::Searching {
            checked: self.sym_index,
        };
        self.held_s = 0.0;
        self.lost_s = 0.0;
    }

    /// The time (input seconds) now.
    pub fn time(&self) -> f64 {
        self.in_samples as f64 / self.in_rate
    }

    /// Feeds input samples; appends decoded frames.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<CFrame>) {
        self.in_samples += x.len() as u64;
        self.t1.clear();
        self.chan.process(x, &mut self.t1);
        let mut t1 = std::mem::take(&mut self.t1);
        for s in t1.iter_mut() {
            if self.carrier.is_none() {
                if let Some(f) = self.search.push(*s) {
                    self.carrier = Some(f);
                    self.nco.set_freq(-f);
                    self.costas.reset();
                    self.status.carrier_found = true;
                    self.held_s = 0.0;
                    self.lost_s = 0.0;
                }
            }
            *s *= self.nco.next_sample();
        }
        let mut t2 = std::mem::take(&mut self.t2);
        t2.clear();
        self.mf.process(&t1, &mut t2);
        let mut syms = std::mem::take(&mut self.syms);
        syms.clear();
        self.timing.process(&t2, &mut syms);
        for &z in &syms {
            self.symbol(z, out);
        }
        self.syms = syms;
        self.t2 = t2;
        self.t1 = t1;
    }

    fn symbol(&mut self, sym: Complex32, out: &mut Vec<CFrame>) {
        let y = if self.carrier.is_some() {
            self.costas.push(sym)
        } else {
            sym
        };
        // Lock: |mean(y^2)| / mean(|y|^2).
        let y2 = y * y;
        self.lock_metric += 0.002 * ((y2.re / (y.norm_sqr() + 1e-12)) - self.lock_metric);
        let dt = 1.0 / SYMBOL_RATE;
        if self.carrier.is_some() {
            if self.lock_metric > 0.4 {
                self.held_s += dt;
                self.lost_s = 0.0;
            } else {
                self.lost_s += dt;
                if self.lost_s > 3.0 {
                    self.drop_carrier();
                }
            }
        }
        let cf = self.carrier.unwrap_or(0.0) + self.costas.freq() * SYMBOL_RATE;
        self.status.carrier_hz = cf;
        // A carrier sample a second, for the drift.
        let t = self.time();
        if self.carrier.is_some() && self.lock_metric > 0.4 && t - self.last_track_t >= 1.0 {
            self.last_track_t = t;
            self.carrier_track.push((t, cf));
            if self.carrier_track.len() > 120 {
                self.carrier_track.remove(0);
            }
            self.status.drift_hz_s = crate::display::slope_of(&self.carrier_track);
        }
        self.ring[(self.sym_index % RING as u64) as usize] = y.re;
        self.sym_index += 1;
        self.status.state = if matches!(self.sync, Sync::Locked { .. }) {
            2
        } else if self.carrier.is_some() && self.lock_metric > 0.4 {
            1
        } else {
            0
        };
        self.frame_sync(out);
    }

    fn sym(&self, i: u64) -> f32 {
        self.ring[(i % RING as u64) as usize]
    }

    fn uw_at(&self, start: u64, invert: bool) -> usize {
        let mut e = 0;
        for j in 0..ROWS {
            let want = (coding::UW[j] == 1) ^ invert;
            for k in 0..2 {
                if (self.sym(start + (j * ROW_SYMBOLS + k) as u64) > 0.0) != want {
                    e += 1;
                }
            }
        }
        e
    }

    fn frame_sync(&mut self, out: &mut Vec<CFrame>) {
        let newest = self.sym_index;
        let fs = FRAME_SYMBOLS as u64;
        match self.sync {
            Sync::Searching { checked } => {
                if self.carrier.is_none() || newest < fs + 2 {
                    return;
                }
                // Check every start whose frame is complete (spread over time: one start per symbol).
                let mut c = checked.max(newest.saturating_sub(RING as u64 - 4));
                let last = newest - fs;
                while c <= last {
                    for inv in [false, true] {
                        let e = self.uw_at(c, inv);
                        if e <= 30 {
                            // The best start nearby.
                            let mut best = (c, e);
                            for d in 1..3u64 {
                                if c + d <= last {
                                    let e2 = self.uw_at(c + d, inv);
                                    if e2 < best.1 {
                                        best = (c + d, e2);
                                    }
                                }
                            }
                            self.decode_at(best.0, inv, false, out);
                            self.sync = Sync::Locked {
                                next: best.0 + fs,
                                invert: inv,
                                misses: 0,
                            };
                            return;
                        }
                    }
                    c += 1;
                }
                self.sync = Sync::Searching { checked: c };
            }
            Sync::Locked {
                next,
                invert,
                misses,
            } => {
                if newest < next + fs + 2 {
                    return;
                }
                // The frame at next-2 .. next+2 with the flip fit.
                let mut best = (next, invert, ROWS, usize::MAX);
                for d in -2i64..=2 {
                    let st = (next as i64 + d) as u64;
                    for i in 0..FRAME_SYMBOLS {
                        self.frame[i] = self.sym(st + i as u64);
                    }
                    let (inv, k, e) = coding::flip_fit(&self.frame);
                    if e < best.3 {
                        best = (st, inv, k, e);
                    }
                }
                if best.3 <= 30 {
                    let (st, inv, k, _) = best;
                    let slip = k < ROWS;
                    self.decode_at(st, inv, slip, out);
                    if slip {
                        self.status.polarity_slips += 1;
                    }
                    let end_inv = if slip { !inv } else { inv };
                    self.sync = Sync::Locked {
                        next: st + fs,
                        invert: end_inv,
                        misses: 0,
                    };
                } else if misses >= 2 {
                    self.sync = Sync::Searching { checked: next };
                } else {
                    self.sync = Sync::Locked {
                        next: next + fs,
                        invert,
                        misses: misses + 1,
                    };
                }
            }
        }
    }

    fn decode_at(&mut self, st: u64, invert: bool, slip: bool, out: &mut Vec<CFrame>) {
        for i in 0..FRAME_SYMBOLS {
            self.frame[i] = self.sym(st + i as u64);
        }
        let (inv0, k) = if slip {
            let (a, k, _) = coding::flip_fit(&self.frame);
            (a, k)
        } else {
            (invert, ROWS)
        };
        for (j, row) in self.frame.chunks_mut(ROW_SYMBOLS).enumerate() {
            let inv = if j < k { inv0 } else { !inv0 };
            if inv {
                for v in row.iter_mut() {
                    *v = -*v;
                }
            }
        }
        let uw = coding::uw_errors(&self.frame, false);
        // Es/N0 from the moments of the soft symbols.
        let (mut m2, mut m4) = (0f64, 0f64);
        for &v in &self.frame {
            let v = v as f64;
            m2 += v * v;
            m4 += v * v * v * v;
        }
        let n = self.frame.len() as f64;
        m2 /= n;
        m4 /= n;
        let s = ((3.0 * m2 * m2 - m4) / 2.0).max(1e-12).sqrt();
        let noise = (m2 - s).max(1e-12);
        let esn0 = db(s / (2.0 * noise));
        let Some(f) = self.decoder.decode(&self.frame) else {
            return;
        };
        self.status.esn0_db = esn0;
        self.status.uw_last = uw;
        self.status.frames_found += 1;
        self.status.ser = f.symbol_errors as f64 / coding::CODED as f64;
        let back = (self.sym_index - st) as f64 / SYMBOL_RATE;
        out.push(CFrame {
            bytes: f.bytes,
            uw_errors: uw,
            symbol_errors: f.symbol_errors,
            slip,
            esn0_db: esn0,
            carrier_hz: self.status.carrier_hz,
            time: self.time() - back,
        });
    }

    /// The newest soft symbols, oldest first (at most the ring), for scopes and tests.
    pub fn recent_symbols(&self, n: usize) -> Vec<f32> {
        let n = n.min(RING).min(self.sym_index as usize);
        (0..n)
            .map(|i| self.sym(self.sym_index - n as u64 + i as u64))
            .collect()
    }

    /// The first rate (after the CIC).
    pub fn first_rate(&self) -> f64 {
        self.r1
    }
    /// The matched filter's output rate.
    pub fn symbol_stage_rate(&self) -> f64 {
        self.r2
    }
}
