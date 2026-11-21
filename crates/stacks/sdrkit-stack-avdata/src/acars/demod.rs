//! One ACARS channel: the AM envelope brought down to about 12.5 kS/s and moved so that each bit holds its sign, then three
//! coherent receivers (shared timing and carrier loops) started a third of a bit apart, each with its own framer, and the choice
//! between what they found.
//! Standard: ARINC 618 clause 2.3 (VHF physical layer and block format).

use super::RawBlock;
use sdrkit_algo_dsp::cic::Cic;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::DspError;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas};
use sdrkit_algo_modem::timing::{Gardner, SymbolSync, SymbolSyncConfig};
use sdrkit_algo_modem::ModemError;
use sdrkit_core::Complex32;
use sdrkit_proto_aero::acars::{check_and_repair, parity_ok, Check, DEL, ETB, ETX, SOH, SYN};

/// A Kaiser-windowed low-pass of `taps` taps with its -6 dB point at `cutoff_hz` at `rate`, unity gain at DC.
fn kaiser_lowpass(rate: f64, cutoff_hz: f64, taps: usize, beta: f64) -> Result<Vec<f32>, DspError> {
    design_by_taps(taps | 1, cutoff_hz / rate, Window::kaiser(beta))
}

/// The ARINC 618 bit rate.
pub const BAUD: f64 = 2400.0;

/// What a framer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FState {
    Hunt,
    Syn2,
    Soh,
    Body,
    Bcs1,
    Bcs2,
}

/// Finds blocks in a demodulator's bits (bytes least significant bit first).
#[derive(Clone, Debug)]
pub struct Framer {
    state: FState,
    reg: u8,
    invert: bool,
    nbits: u32,
    byte: u8,
    body: Vec<u8>,
    bcs: [u8; 2],
    bad_parity: u32,
    /// SYN SYN SOH seen.
    pub started: u64,
}

/// A block the framer finished: body, check bytes.
pub type Framed = (Vec<u8>, [u8; 2]);

impl Default for Framer {
    fn default() -> Self {
        Framer {
            state: FState::Hunt,
            reg: 0,
            invert: false,
            nbits: 0,
            byte: 0,
            body: Vec::with_capacity(256),
            bcs: [0; 2],
            bad_parity: 0,
            started: 0,
        }
    }
}

impl Framer {
    /// Takes one bit. Returns a finished block.
    pub fn push(&mut self, bit: bool) -> Option<Framed> {
        if self.state == FState::Hunt {
            self.reg = (self.reg >> 1) | ((bit as u8) << 7);
            if self.reg == SYN {
                self.invert = false;
            } else if self.reg == !SYN {
                self.invert = true;
            } else {
                return None;
            }
            self.state = FState::Syn2;
            self.nbits = 0;
            self.byte = 0;
            return None;
        }
        let b = bit ^ self.invert;
        self.byte = (self.byte >> 1) | ((b as u8) << 7);
        self.nbits += 1;
        if self.nbits < 8 {
            return None;
        }
        self.nbits = 0;
        let c = self.byte;
        match self.state {
            FState::Hunt => None,
            FState::Syn2 => {
                self.state = if c == SYN { FState::Soh } else { FState::Hunt };
                self.reg = 0;
                None
            }
            FState::Soh => {
                if c == SOH {
                    self.state = FState::Body;
                    self.body.clear();
                    self.bad_parity = 0;
                    self.started += 1;
                } else if c != SYN {
                    self.state = FState::Hunt;
                }
                None
            }
            FState::Body => {
                if c == DEL && self.body.len() > 20 {
                    // The suffix was lost: the two bytes before DEL are the check bytes.
                    let n = self.body.len();
                    self.bcs = [self.body[n - 2], self.body[n - 1]];
                    self.body.truncate(n - 2);
                    self.state = FState::Hunt;
                    return Some((self.body.clone(), self.bcs));
                }
                self.body.push(c);
                if !parity_ok(c) {
                    self.bad_parity += 1;
                }
                if c == ETX || c == ETB {
                    self.state = FState::Bcs1;
                } else if self.bad_parity > 4 || self.body.len() > 240 {
                    self.state = FState::Hunt;
                }
                None
            }
            FState::Bcs1 => {
                self.bcs[0] = c;
                self.state = FState::Bcs2;
                None
            }
            FState::Bcs2 => {
                self.bcs[1] = c;
                self.state = FState::Hunt;
                Some((self.body.clone(), self.bcs))
            }
        }
    }

    /// Whether a block is being read.
    pub fn busy(&self) -> bool {
        !matches!(self.state, FState::Hunt | FState::Syn2)
    }
}

/// One of the three receivers on the channel: its own timing loop (started at its own phase), carrier loop and framer.
#[derive(Clone, Debug)]
struct Rail {
    /// Samples of delay before the timing loop, which set its starting phase.
    delay: usize,
    sync: SymbolSync<Complex32, Gardner, Cubic>,
    carrier: CarrierSync<Costas<2>>,
    framer: Framer,
    symbols: Vec<Complex32>,
}

impl Rail {
    fn new(sps: f64, delay: usize) -> Result<Self, ModemError> {
        let mut cfg = SymbolSyncConfig::new(sps);
        cfg.bn_t = 0.006;
        cfg.max_ppm = 3000.0;
        cfg.pulse = Shape::Rc { beta: 1.0 };
        Ok(Rail {
            delay,
            sync: SymbolSync::new(Gardner, Cubic, cfg)?,
            carrier: CarrierSync::new(Costas::<2>, CarrierConfig::new(0.01))?,
            framer: Framer::default(),
            symbols: Vec::with_capacity(4096),
        })
    }

    /// Demodulates `z` (the matched filter's output). Calls `on_block` with each block and the baseband index of its end.
    fn run(&mut self, z: &[Complex32], mut on_block: impl FnMut(Framed, f64)) {
        if self.delay > 0 {
            let lead = [Complex32::default(); 8];
            self.symbols.clear();
            self.sync.process(&lead[..self.delay], &mut self.symbols);
        }
        self.symbols.clear();
        self.sync.process(z, &mut self.symbols);
        let sps = self.sync.config().sps;
        let newest = self.sync.last_instant() - self.delay as f64;
        self.delay = 0;
        let k = self.symbols.len();
        for j in 0..k {
            let y = self.carrier.push(self.symbols[j]);
            if let Some(b) = self.framer.push(y.re > 0.0) {
                on_block(b, newest - (k - 1 - j) as f64 * sps);
            }
        }
    }
}

/// A decoder on one channel.
pub struct ChannelDecoder {
    /// Channel frequency, Hz.
    pub freq_hz: f64,
    nco: Nco,
    cic: Cic<Complex32>,
    lp1: FirDecimator<Complex32>,
    lp2: FirDecimator<f32>,
    mix: Nco,
    matched: Fir<Complex32>,
    turn: Nco,
    s6: Vec<Complex32>,
    dc: f32,
    env_mean: f32,
    /// Input samples per baseband sample.
    pub per_out: u64,
    /// The input index of the decoder's first input sample.
    pub start: u64,
    /// Input samples taken.
    pub taken: u64,
    rate: f64,
    /// The delay of the matched filter, baseband samples.
    lag: f64,
    rails: [Rail; 3],
    s0: Vec<Complex32>,
    s1: Vec<Complex32>,
    s2: Vec<Complex32>,
    s3: Vec<f32>,
    s4: Vec<f32>,
    s5: Vec<Complex32>,
    found: Vec<(Framed, f64, usize)>,
    pending: Vec<(RawBlock, Option<u32>, usize)>,
    /// Finished blocks, in order.
    pub out: Vec<RawBlock>,
}

impl ChannelDecoder {
    /// A decoder at input `rate` for the channel at `offset_hz` from the centre (the carrier's own offset included).
    ///
    /// # Errors
    ///
    /// The errors of the filters and loops, when the rate needs a decimation that the integer CIC cannot hold.
    pub fn new(rate: f64, freq_hz: f64, offset_hz: f64, start: u64) -> Result<Self, ModemError> {
        let r = ((rate / 100e3).round() as usize).max(1);
        let f1 = rate / r as f64;
        let f2 = f1 / 4.0;
        let fa = f2 / 2.0;
        let sps = fa / BAUD;
        let rail = |i: f64| Rail::new(sps, (i * sps / 3.0).round() as usize);
        // The matched filter of one rail: a half sine over two bits.
        let matched = taps(Shape::HalfSine, 2.0 * sps, 1.0, Norm::Energy)?;
        Ok(ChannelDecoder {
            freq_hz,
            nco: Nco::new(rate, -offset_hz),
            cic: Cic::new(4, r)?,
            lp1: FirDecimator::new(&kaiser_lowpass(f1, 7.5e3, 65, 7.0)?, 4),
            lp2: FirDecimator::new(&kaiser_lowpass(f2, 3.6e3, 33, 6.0)?, 2),
            mix: Nco::new(fa, -1800.0),
            // After the matched filter, the upper tone (1800 + 600 Hz) to 0 Hz: each bit then holds its sign.
            turn: Nco::new(fa, -BAUD / 4.0),
            s6: Vec::new(),
            lag: (matched.len() - 1) as f64 / 2.0,
            matched: Fir::new(&matched),
            dc: 0.0,
            env_mean: 0.0,
            per_out: (r * 8) as u64,
            start,
            taken: 0,
            rate,
            rails: [rail(0.0)?, rail(1.0)?, rail(2.0)?],
            s0: Vec::new(),
            s1: Vec::new(),
            s2: Vec::new(),
            s3: Vec::new(),
            s4: Vec::new(),
            s5: Vec::new(),
            found: Vec::new(),
            pending: Vec::new(),
            out: Vec::new(),
        })
    }

    /// Frames started by the first receiver.
    pub fn frames_started(&self) -> u64 {
        self.rails[0].framer.started
    }

    /// A block is being read by any receiver.
    pub fn busy(&self) -> bool {
        self.rails.iter().any(|d| d.framer.busy()) || !self.pending.is_empty()
    }

    /// The input index after the last sample taken.
    pub fn pos(&self) -> u64 {
        self.start + self.taken
    }

    /// Takes input samples (the next ones after [`pos`](Self::pos)).
    pub fn feed(&mut self, x: &[Complex32]) {
        if x.is_empty() {
            return;
        }
        self.s0.resize(x.len(), Complex32::default());
        self.nco.mix(x, &mut self.s0);
        self.s1.clear();
        for &v in &self.s0 {
            if let Some(y) = self.cic.push(v) {
                self.s1.push(y);
            }
        }
        self.taken += x.len() as u64;
        self.s2.clear();
        self.lp1.process(&self.s1, &mut self.s2);
        self.s3.clear();
        for v in &self.s2 {
            let e = v.norm();
            self.env_mean += 0.002 * (e - self.env_mean);
            self.dc += 0.01 * (e - self.dc);
            self.s3.push(e - self.dc);
        }
        self.s4.clear();
        self.lp2.process(&self.s3, &mut self.s4);
        self.s5.resize(self.s4.len(), Complex32::default());
        self.mix.mix_real(&self.s4, &mut self.s5);
        for v in self.s5.iter_mut() {
            *v = self.matched.process_sample(*v);
        }
        self.s6.resize(self.s5.len(), Complex32::default());
        self.turn.mix(&self.s5, &mut self.s6);
        let level = 20.0 * (self.env_mean.max(1e-9)).log10();
        self.found.clear();
        for (i, d) in self.rails.iter_mut().enumerate() {
            let found = &mut self.found;
            d.run(&self.s6, |b, t| found.push((b, t, i)));
        }
        for ((body, bcs), t, di) in self.found.drain(..) {
            let end_index = self.start as f64 + (t - self.lag) * self.per_out as f64;
            let end_s = end_index / self.rate;
            let mut tmp = body.clone();
            let rep = match check_and_repair(&mut tmp, bcs) {
                Check::Good { bits_fixed } => Some(bits_fixed),
                Check::Bad => None,
            };
            let rb = RawBlock {
                freq_hz: self.freq_hz,
                bytes: body,
                bcs,
                level_db: level,
                end_s,
            };
            // Blocks that end within 30 ms of each other are the same block.
            if let Some(p) = self
                .pending
                .iter_mut()
                .find(|p| (p.0.end_s - end_s).abs() < 0.030)
            {
                // A good one replaces a bad one. Of two good ones the one with fewer repaired bits. On a tie the first receiver.
                let better = match (p.1, rep) {
                    (None, Some(_)) => true,
                    (Some(a), Some(b)) => b < a || (b == a && di < p.2),
                    (None, None) => di < p.2,
                    _ => false,
                };
                if better {
                    *p = (rb, rep, di);
                }
            } else {
                self.pending.push((rb, rep, di));
            }
        }
        // A finished block is held 50 ms for the comparison.
        let now = self.pos() as f64 / self.rate;
        let mut i = 0;
        while i < self.pending.len() {
            if now - self.pending[i].0.end_s > 0.050 {
                let (b, _, _) = self.pending.remove(i);
                self.out.push(b);
            } else {
                i += 1;
            }
        }
        self.out.sort_by(|a, b| a.end_s.total_cmp(&b.end_s));
    }

    /// Returns out what is still held (the decoder stops).
    pub fn flush(&mut self) {
        for (b, _, _) in self.pending.drain(..) {
            self.out.push(b);
        }
    }
}
