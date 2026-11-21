//! Aero P channel demodulators: MSK for 600 and 1200 bit/s and offset QPSK for 10500 bit/s, each with the frame finder and its
//! flywheel.
//! Standard: Inmarsat Aeronautical System Definition Manual; ARINC 741.

use sdrkit_algo_dsp::meter::power_to_db as db;
use super::coding::{Decoded, FrameDecoder, Layout, UW};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_modem::carrier::{Costas, PhaseDetector};
use sdrkit_algo_modem::freq::PowerLine;
use sdrkit_algo_modem::msk::MskDemod;
use sdrkit_algo_modem::timing::{Gardner, SymbolSync, SymbolSyncConfig};
use sdrkit_core::Complex32;
use std::sync::OnceLock;

/// A frame found.
#[derive(Clone, Debug, PartialEq)]
pub struct AeroFrame {
    pub rate: u32,
    pub header: u16,
    pub info: Vec<u8>,
    /// Unique word errors, or -1 when taken on timing alone.
    pub uw_errors: i32,
    pub channel_ber: f64,
    pub ebn0_db: f64,
    /// Carrier, Hz from the channel's centre.
    pub carrier_hz: f64,
    /// Channel samples (at the demodulator's rate) since its start at the frame's end.
    pub at: u64,
}

/// The frame finder: unique word hunt (either sign), then frames on timing with a flywheel, and the decoder of the continuous code.
struct Framer {
    layout: Layout,
    sr: u64,
    soft: Vec<f32>,
    inv: bool,
    locked: bool,
    misses: u32,
    pos: usize,
    decoder: FrameDecoder<AeroFrame>,
    decoded: Vec<(Decoded, AeroFrame)>,
    last_uw: i32,
}

/// The unique word as channel bits: 32 bits, or for 10500 each bit on both arms (Q then I).
fn uw_word(bits: usize) -> u64 {
    if bits == 64 {
        (0..32).rev().fold(0u64, |v, i| {
            let b = ((UW >> i) & 1) as u64;
            (v << 2) | (b << 1) | b
        })
    } else {
        UW as u64
    }
}

impl Framer {
    fn new(rate: u32) -> Self {
        Framer {
            layout: Layout::of(rate),
            sr: 0,
            soft: Vec::new(),
            inv: false,
            locked: false,
            misses: 0,
            pos: 0,
            decoder: FrameDecoder::default(),
            decoded: Vec::new(),
            last_uw: 0,
        }
    }

    fn uw_errs(&self) -> (u32, bool) {
        let n = self.layout.uw_bits.min(64);
        let mask = if n >= 64 { u64::MAX } else { (1u64 << n) - 1 };
        let w = uw_word(n);
        let e = ((self.sr & mask) ^ w).count_ones();
        let ei = ((!self.sr & mask) ^ w).count_ones();
        if e <= ei {
            (e, false)
        } else {
            (ei, true)
        }
    }

    /// Starts the frames just after a unique word found by the caller: the code stream starts again.
    fn lock_at_uw(&mut self, inv: bool) {
        self.locked = true;
        self.inv = inv;
        self.pos = self.layout.uw_bits;
        self.soft.clear();
        self.misses = 0;
        self.decoder.restart(&mut self.decoded);
    }

    /// Frames whose bits are decided, with their meta data.
    fn take(&mut self, out: &mut Vec<AeroFrame>) {
        for (d, mut f) in self.decoded.drain(..) {
            f.channel_ber = d.channel_errors as f64 / self.layout.coded_bits as f64;
            f.info = d.bytes;
            out.push(f);
        }
    }

    /// One soft bit (positive = 1). Frames come out one frame later, when the code's next bits have decided their end.
    fn push(&mut self, s: f32, ebn0: f64, carrier: f64, at: u64, out: &mut Vec<AeroFrame>) {
        self.sr = (self.sr << 1) | (s > 0.0) as u64;
        let l = self.layout;
        let uw = l.uw_bits;
        if !self.locked {
            let (e, inv) = self.uw_errs();
            if e <= (uw as u32) / 10 {
                self.lock_at_uw(inv);
                self.take(out);
            }
            return;
        }
        self.pos += 1;
        if self.pos > uw {
            self.soft.push(if self.inv { -s } else { s });
        }
        if self.pos == l.frame_bits() {
            // The frame: header, unused bits, coded bits.
            let hdr = self.soft[..16]
                .iter()
                .fold(0u16, |v, &x| (v << 1) | (x > 0.0) as u16);
            let f = AeroFrame {
                rate: l.rate,
                header: hdr,
                info: Vec::new(),
                uw_errors: self.last_uw,
                channel_ber: 0.0,
                ebn0_db: ebn0,
                carrier_hz: carrier,
                at,
            };
            let coded = &self.soft[16 + l.unused_bits..];
            self.decoder.push(&l, coded, f, &mut self.decoded);
            self.soft.clear();
            self.pos = 0;
            self.take(out);
            return;
        }
        if self.pos == uw {
            // The next unique word: on timing when it is damaged.
            let (e, inv) = self.uw_errs();
            if e <= (uw as u32) / 6 {
                self.misses = 0;
                self.inv = inv;
            } else {
                self.misses += 1;
                if self.misses > 3 {
                    self.locked = false;
                    self.decoder.flush(&mut self.decoded);
                    self.take(out);
                }
            }
            self.last_uw = if e <= (uw as u32) / 6 { e as i32 } else { -1 };
        }
    }
}

/// The FFT of the carrier search on the square of the signal.
pub const SQ_N: usize = 4096;
/// FFT frames the carrier search averages.
const SQ_FRAMES: usize = 4;
/// The weaker line of the pair must stand this far over the noise, dB.
const LINE_SNR_DB: f32 = 8.0;

/// The carrier search on the square's two lines (MSK: 2 fc +- Rb / 2; offset QPSK: 2 fc +- Rs), refreshed while tracking.
struct LineSearch {
    lines: PowerLine,
    spacing: f64,
    buf: Vec<Complex32>,
    fs: f64,
}

impl LineSearch {
    fn new(fs: f64, spacing_hz: f64, span_hz: f64) -> Self {
        let mut lines = PowerLine::new(2, SQ_N).expect("a valid line search");
        let _ = lines.set_span((span_hz / fs).min(0.25));
        LineSearch {
            lines,
            spacing: spacing_hz / fs,
            buf: Vec::with_capacity(SQ_N * SQ_FRAMES),
            fs,
        }
    }
    /// Takes a sample; returns the carrier (Hz) each time a search is due and the lines stand clear of the noise.
    fn push(&mut self, x: Complex32) -> Option<Option<f64>> {
        self.buf.push(x);
        if self.buf.len() < SQ_N * SQ_FRAMES {
            return None;
        }
        let r = self
            .lines
            .estimate_pair(&self.buf, self.spacing)
            .ok()
            .filter(|p| p.snr_db >= LINE_SNR_DB)
            .map(|p| p.freq * self.fs);
        self.buf.clear();
        Some(r)
    }
}

/// The MSK demodulator (600, 1200 bit/s).
pub struct Msk {
    pub rate: u32,
    pub fs: f64,
    chan: Channelizer,
    tmp: Vec<Complex32>,
    llr: Vec<f32>,
    search: LineSearch,
    pub locked_carrier: bool,
    pub fine: f64,
    nco: Nco,
    demod: MskDemod,
    framer: Framer,
    um: f64,
    err: f64,
    n: u64,
    pub frames: u64,
    pub span: f64,
}

/// A new MSK demodulator. Its filters are the same for every channel and take long to design, so each channel gets a
/// copy of one design.
fn msk_demod() -> MskDemod {
    static DESIGN: OnceLock<MskDemod> = OnceLock::new();
    DESIGN
        .get_or_init(|| {
            // The higher tone is bit 1, so a positive soft value means 1 after the sign change in `Msk::process`.
            MskDemod::new(16.0, 0.01)
                .expect("valid MSK demodulator")
                .with_inverted(true)
        })
        .clone()
}

impl Msk {
    /// An MSK demodulator at `rate` on the channel at `offset` of an input at `rate_in`, searching +-`span` Hz.
    pub fn new(rate_in: f64, offset: f64, rate: u32, span: f64) -> Self {
        let fs = 16.0 * rate as f64;
        let mut cc = ChannelConfig::new(rate_in, offset, 0.8 * fs, fs);
        cc.exact_rate = true;
        Msk {
            rate,
            fs,
            chan: Channelizer::new(cc).expect("the Aero channel fits the input"),
            tmp: Vec::new(),
            llr: Vec::new(),
            search: LineSearch::new(fs, rate as f64, span),
            locked_carrier: false,
            fine: 0.0,
            nco: Nco::new(fs, 0.0),
            demod: msk_demod(),
            framer: Framer::new(rate),
            um: 0.0,
            err: 0.0,
            n: 0,
            frames: 0,
            span,
        }
    }

    /// The bit energy per noise from the soft values' mean against their spread.
    pub fn ebn0_db(&self) -> f64 {
        let var = (self.err - self.um * self.um).max(1e-30);
        db(self.um * self.um / var) - 3.0
    }

    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<AeroFrame>) {
        self.tmp.clear();
        self.chan.process(x, &mut self.tmp);
        let mut tmp = std::mem::take(&mut self.tmp);
        for &v in &tmp {
            if let Some(found) = self.search.push(v) {
                match found {
                    Some(f) if !self.locked_carrier => {
                        self.fine = f;
                        self.locked_carrier = true;
                        self.demod.reset();
                    }
                    // While tracking, a new estimate close by moves the carrier half way to it.
                    Some(f) if (f - self.fine).abs() < 0.1 * self.rate as f64 => {
                        self.fine += 0.5 * (f - self.fine)
                    }
                    _ => {}
                }
                self.nco.set_freq(-self.fine);
            }
        }
        if self.locked_carrier {
            self.nco.mix_in_place(&mut tmp);
            self.llr.clear();
            if self.demod.process(&tmp, &mut self.llr).is_ok() {
                let spb = 16;
                for i in 0..self.llr.len() {
                    // The decoder's LLR is positive for 0: the framer takes positive for 1.
                    let s = -self.llr[i];
                    self.n += spb;
                    let u = s.abs() as f64;
                    self.um += 0.01 * (u - self.um);
                    self.err += 0.01 * (u * u - self.err);
                    let before = out.len();
                    self.framer.push(s, self.ebn0_db(), self.fine, self.n, out);
                    self.frames += (out.len() - before) as u64;
                }
            }
        }
        self.tmp = tmp;
    }

    pub fn in_sync(&self) -> bool {
        self.framer.locked
    }
}

/// The offset QPSK demodulator (10500 bit/s).
pub struct Oqpsk {
    pub fs: f64,
    chan: Channelizer,
    tmp: Vec<Complex32>,
    filtered: Vec<Complex32>,
    syms: Vec<Complex32>,
    search: LineSearch,
    pub locked_carrier: bool,
    pub fine: f64,
    nco: Nco,
    mf: FirDecimator<Complex32>,
    timing: SymbolSync<Complex32, Gardner, Cubic>,
    /// The carrier loop. It runs on the symbols, and its correction turns the samples before the timing loop: the timing of
    /// offset QPSK depends on the carrier phase.
    carrier: LoopFilter,
    costas: Costas<4>,
    evm: f64,
    /// Arm bit histories per hypothesis (rotation 0..3, pairing shift 0/1): (Q register, I register, previous Q).
    hyps: Vec<(u32, u32, f32)>,
    chosen: Option<usize>,
    framer: Framer,
    n: u64,
    pub frames: u64,
    pub span: f64,
}

pub const SYM_RATE: f64 = 5250.0;

/// A new timing loop of the OQPSK demodulator: a copy of one design, as [`msk_demod`].
fn oqpsk_timing() -> SymbolSync<Complex32, Gardner, Cubic> {
    static DESIGN: OnceLock<SymbolSync<Complex32, Gardner, Cubic>> = OnceLock::new();
    DESIGN
        .get_or_init(|| {
            let mut tc = SymbolSyncConfig::new(8.0);
            tc.bn_t = 0.004;
            tc.oqpsk = true;
            tc.pulse = Shape::Rc { beta: 1.0 };
            SymbolSync::new(Gardner, Cubic, tc).expect("valid timing loop")
        })
        .clone()
}

impl Oqpsk {
    pub fn new(rate_in: f64, offset: f64, span: f64) -> Self {
        let fs = 8.0 * SYM_RATE;
        let mut cc = ChannelConfig::new(rate_in, offset, 0.8 * fs, fs);
        cc.exact_rate = true;
        let h = taps(Shape::Rrc { beta: 1.0 }, 8.0, 8.0, Norm::Energy).expect("a valid RRC pulse");
        let mut carrier =
            LoopFilter::new(0.01, std::f64::consts::FRAC_1_SQRT_2).expect("valid carrier loop");
        // The loop can pull up to a tenth of the symbol rate from where the search put the carrier.
        carrier.set_clamp(0.1);
        Oqpsk {
            fs,
            chan: Channelizer::new(cc).expect("the Aero channel fits the input"),
            tmp: Vec::new(),
            filtered: Vec::new(),
            syms: Vec::new(),
            search: LineSearch::new(fs, 2.0 * SYM_RATE, span),
            locked_carrier: false,
            fine: 0.0,
            nco: Nco::new(fs, 0.0),
            mf: FirDecimator::new(&h, 1),
            timing: oqpsk_timing(),
            carrier,
            costas: Costas::<4>,
            evm: 1.0,
            hyps: vec![(0, 0, 0.0); 8],
            chosen: None,
            framer: Framer::new(10500),
            n: 0,
            frames: 0,
            span,
        }
    }

    pub fn in_sync(&self) -> bool {
        self.framer.locked
    }

    pub fn carrier(&self) -> f64 {
        self.fine + self.carrier.integrator() * SYM_RATE
    }

    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<AeroFrame>) {
        self.tmp.clear();
        self.chan.process(x, &mut self.tmp);
        let mut tmp = std::mem::take(&mut self.tmp);
        if !self.locked_carrier {
            for &v in &tmp {
                if let Some(Some(f)) = self.search.push(v) {
                    self.fine = f;
                    self.locked_carrier = true;
                    self.nco.set_freq(-f);
                    self.carrier.reset();
                    self.chosen = None;
                    break;
                }
            }
            self.tmp = tmp;
            return;
        }
        // One symbol of samples at a time, so the carrier loop turns the next samples.
        for piece in tmp.chunks_mut(8) {
            self.nco.mix_in_place(piece);
            // The Q arm leads: swapping the arms lets the timing loop read the arm that lags half a symbol after the other.
            for v in piece.iter_mut() {
                *v = Complex32::new(v.im, v.re);
            }
            self.filtered.clear();
            self.mf.process(piece, &mut self.filtered);
            self.syms.clear();
            self.timing.process(&self.filtered, &mut self.syms);
            let syms = std::mem::take(&mut self.syms);
            for &w in &syms {
                self.symbol(Complex32::new(w.im, w.re), out);
            }
            self.syms = syms;
        }
        self.tmp = tmp;
    }

    fn symbol(&mut self, z: Complex32, out: &mut Vec<AeroFrame>) {
        self.n += 8;
        let e = self.costas.error(z) as f64 / std::f64::consts::TAU;
        let step = self.carrier.update(e);
        self.nco.set_freq(-(self.fine + step * SYM_RATE));
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let d = Complex32::new(s.copysign(z.re), s.copysign(z.im));
        self.evm += 0.01 * ((z - d).norm_sqr() as f64 - self.evm);
        let ebn0 = -db(self.evm.max(1e-6)) - 3.0;
        let carrier = self.carrier();
        // The arms of each hypothesis: rotation by k quarter turns, and the pairing of a Q with the I before or after it.
        let mut found = None;
        for (h, st) in self.hyps.iter_mut().enumerate() {
            let rot = h % 4;
            let shift = h / 4;
            let y = match rot {
                0 => z,
                1 => Complex32::new(z.im, -z.re),
                2 => -z,
                _ => Complex32::new(-z.im, z.re),
            };
            let (i, q) = if shift == 1 {
                let pq = st.2;
                st.2 = y.im;
                (y.re, pq)
            } else {
                (y.re, y.im)
            };
            st.0 = (st.0 << 1) | (q > 0.0) as u32;
            st.1 = (st.1 << 1) | (i > 0.0) as u32;
            if self.chosen.is_none() && (st.0 ^ UW).count_ones() + (st.1 ^ UW).count_ones() <= 6 {
                found = Some(h);
            }
            if self.chosen == Some(h) {
                // The bits of this symbol: Q (leading), then I.
                for b in [q, i] {
                    let before = out.len();
                    self.framer.push(b, ebn0, carrier, self.n, out);
                    self.frames += (out.len() - before) as u64;
                }
            }
        }
        if let Some(h) = found {
            // Start the framer just after the unique word on both arms.
            self.chosen = Some(h);
            self.framer.lock_at_uw(false);
            self.framer.take(out);
        }
    }
}
