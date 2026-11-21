//! The DVB-T modulator: transport packets to OFDM samples, clause by clause. It makes the clean signal only. The test signal
//! generator ([`DvbtGenerator`]) and the sender of the stack ([`crate::transmit`]) both run it.
//!
//! Standard: ETSI EN 300 744 clauses 4.3 to 4.6.

use crate::coding::{self, branches, DVBT_BIT_OFFSETS as BIT_OFFSETS, INNER};
use crate::params::{Bandwidth, Mode, TpsParams};
use crate::tables::{CarrierKind, Layout};
use crate::tps;
use sdrkit_algo_dsp::constellation::PointSet;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_fec::conv::{puncture, Encoder, Puncture};
use sdrkit_core::{Complex32, ConfigError, Generator};
use sdrkit_proto_mpegts::outer::{Dispersal, OuterEncoder, GROUP, RS_N};
use sdrkit_proto_mpegts::source::{Fill, NullSource, Numbered, PacketSource, TestMux, TestSound};

/// What the generator makes.
pub struct GenParams {
    /// Channel bandwidth (sets the natural rate).
    pub bandwidth: Bandwidth,
    /// Everything TPS signals.
    pub tps: TpsParams,
    /// The starting position in the puncturing period (output bits), to test the receiver's phase search.
    pub puncture_phase: usize,
    /// The HP packet source (default: the built-in multiplex).
    pub hp: Box<dyn PacketSource>,
    /// The LP packet source (default: null packets).
    pub lp: Box<dyn PacketSource>,
    /// The frame number (0 to 3) of the first frame.
    pub first_frame: u8,
    /// A raw LP code-rate field for TPS (a reserved value, as some transmitters send in a non-hierarchical multiplex).
    pub lp_rate_field: Option<u8>,
}

impl GenParams {
    /// The parameters with the built-in multiplex (service 1, "Test service", with its sound).
    pub fn new(bandwidth: Bandwidth, tps: TpsParams) -> Self {
        GenParams {
            bandwidth,
            tps,
            puncture_phase: 0,
            hp: test_mux(1, "Test service", tps.net_bitrate(bandwidth, false)),
            lp: Box::new(NullSource),
            first_frame: 0,
            lp_rate_field: None,
        }
    }
}

/// The numbered packets of the `pattern` source on the high-priority stream (the DVB-T test vectors).
pub const PATTERN_HP: Numbered = Numbered::new(0x0100, Fill::I31N7);
/// The numbered packets of the `pattern` source on the low-priority stream.
pub const PATTERN_LP: Numbered = Numbered::new(0x0200, Fill::I31N7);

/// The PID of the sound of the test multiplex.
pub const SOUND_PID: u16 = 0x0102;

/// The test multiplex of the generator (`TestMux::dvbt`): one TV service with its tables, numbered packets on PID 0x0101,
/// and the sound of a tone ([`TestSound`]) on [`SOUND_PID`], paced for a multiplex of `bit_rate` bit/s (the net bit rate
/// of the high-priority stream). A name longer than 255 bytes, or a bit rate too low for the sound, produces null packets.
pub fn test_mux(service_id: u16, name: &str, bit_rate: f64) -> Box<dyn PacketSource> {
    let sound = TestSound {
        pid: SOUND_PID,
        mux_bit_rate: bit_rate,
    };
    match TestMux::dvbt_builder(service_id, name).sound(sound).build() {
        Ok(m) => Box::new(m),
        Err(_) => Box::new(NullSource),
    }
}

/// One coded stream: packets to punctured coded bits (clauses 4.3.1 to 4.3.3).
struct TxStream {
    outer: OuterEncoder,
    enc: Encoder,
    pattern: Puncture<'static>,
    /// The place in the puncturing pattern of the next mother-code bit.
    at: usize,
    /// Punctured bits still to drop at the start (the puncturing phase of a receiver that joins mid-stream).
    skip: usize,
    /// Coded bits; those before `head` are taken.
    bits: Vec<u8>,
    head: usize,
    scratch: Vec<u8>,
    unpacked: Vec<u8>,
    mother: Vec<u8>,
}

impl TxStream {
    fn new(rate: crate::params::CodeRate, phase: usize) -> Self {
        TxStream {
            outer: OuterEncoder::new(Dispersal::DvbGroups),
            enc: Encoder::new(INNER),
            pattern: coding::puncture(rate),
            at: 0,
            skip: phase % rate.ratio().1,
            bits: Vec::new(),
            head: 0,
            scratch: Vec::new(),
            unpacked: Vec::new(),
            mother: Vec::new(),
        }
    }

    /// Codes dispersal groups of packets from `src` until `n` bits wait.
    fn fill(&mut self, n: usize, src: &mut dyn PacketSource) {
        if self.head > 0 && self.head * 2 >= self.bits.len() {
            self.bits.drain(..self.head);
            self.head = 0;
        }
        while self.bits.len() - self.head < n {
            self.scratch.clear();
            let (mut p, mut w) = ([0u8; 188], [0u8; RS_N]);
            for _ in 0..GROUP {
                src.next_packet(&mut p);
                self.outer.encode(&p, &mut w);
                self.scratch.extend_from_slice(&w);
            }
            self.unpacked.clear();
            for &b in &self.scratch {
                for k in (0..8).rev() {
                    self.unpacked.push((b >> k) & 1);
                }
            }
            self.mother.clear();
            self.enc.encode(&self.unpacked, &mut self.mother);
            let before = self.bits.len();
            puncture(&self.pattern, self.at, &self.mother, &mut self.bits);
            self.at = (self.at + self.mother.len()) % self.pattern.period();
            let skip = self.skip.min(self.bits.len() - before);
            if skip > 0 {
                self.bits.drain(before..before + skip);
                self.skip -= skip;
            }
        }
    }

    /// The next `n` coded bits, appended to `out`.
    fn take(&mut self, n: usize, src: &mut dyn PacketSource, out: &mut Vec<u8>) {
        self.fill(n, src);
        out.extend_from_slice(&self.bits[self.head..self.head + n]);
        self.head += n;
    }
}

/// The DVB-T chain from transport packets to OFDM symbols at the natural rate: energy dispersal, Reed-Solomon, the outer
/// interleaver, the convolutional code with puncturing, the bit and symbol interleavers, the mapping, the pilots and the TPS,
/// the inverse FFT and the guard interval. The packet sources are given with each symbol.
pub struct Modulator {
    /// The bandwidth.
    pub bandwidth: Bandwidth,
    /// The parameters.
    pub tps: TpsParams,
    layout: Layout,
    points: PointSet,
    hp: TxStream,
    lp: Option<TxStream>,
    ifft: Fft,
    sym: Vec<Complex32>,
    l: usize,
    frame: u8,
    tps_bits: [u8; 68],
    tps_prev: f32,
    scale: f32,
    cells: Vec<Complex32>,
    bins: Vec<Complex32>,
    /// Symbols made.
    pub symbols: u64,
    lp_field: Option<u8>,
    hp_bits: Vec<u8>,
    lp_bits: Vec<u8>,
    yq: Vec<u8>,
    y: Vec<u8>,
}

impl Modulator {
    /// A modulator for `bandwidth` and `tps`, starting at symbol 0 of frame `first_frame` (0 to 3), the puncturing at
    /// `puncture_phase`, and the LP code-rate field of TPS set to `lp_rate_field` when given.
    pub fn new(
        bandwidth: Bandwidth,
        tps: TpsParams,
        puncture_phase: usize,
        first_frame: u8,
        lp_rate_field: Option<u8>,
    ) -> Self {
        let n = tps.mode.fft();
        let k = tps.mode.carriers();
        let hier = tps.hierarchy.hierarchical();
        Modulator {
            bandwidth,
            tps,
            layout: Layout::new(tps.mode),
            points: coding::point_set(tps.constellation, tps.hierarchy),
            hp: TxStream::new(tps.rate_hp, puncture_phase),
            lp: hier.then(|| TxStream::new(tps.rate_lp, puncture_phase)),
            ifft: Fft::inverse(n),
            sym: Vec::with_capacity(tps.symbol_samples()),
            l: 0,
            frame: first_frame & 3,
            tps_bits: tps::encode_with(&tps, first_frame & 3, lp_rate_field),
            tps_prev: 1.0,
            scale: 1.0 / (1.04 * k as f32).sqrt(),
            cells: Vec::new(),
            bins: vec![Complex32::default(); n],
            symbols: 0,
            lp_field: lp_rate_field,
            hp_bits: Vec::new(),
            lp_bits: Vec::new(),
            yq: Vec::new(),
            y: Vec::new(),
        }
    }

    /// The natural sample rate.
    pub fn rate(&self) -> f64 {
        self.bandwidth.natural_rate()
    }

    /// The symbol index in the frame (0 to 67) and the frame number of the next symbol.
    pub fn position(&self) -> (usize, u8) {
        (self.l, self.frame)
    }

    /// The next symbol's data cells in carrier order (after the symbol interleaver and mapping), with its index in the
    /// frame. The symbol counters advance.
    pub fn next_cells(
        &mut self,
        hp: &mut dyn PacketSource,
        lp: &mut dyn PacketSource,
    ) -> (Vec<Complex32>, usize) {
        let l = self.l;
        self.make_cells(hp, lp);
        let cells = self.cells.clone();
        self.advance();
        (cells, l)
    }

    fn make_cells(&mut self, hp: &mut dyn PacketSource, lp: &mut dyn PacketSource) {
        let nd = self.tps.mode.data_carriers();
        let v = self.tps.constellation.bits();
        let c = self.tps.constellation;
        let h = self.tps.hierarchy;
        // Coded bits of this symbol.
        self.hp_bits.clear();
        self.lp_bits.clear();
        let hp_per = if h.hierarchical() { 2 } else { v };
        self.hp.take(hp_per * nd, hp, &mut self.hp_bits);
        if let Some(s) = self.lp.as_mut() {
            s.take((v - 2) * nd, lp, &mut self.lp_bits);
        }
        // Bit interleaver, per block of 126 cells (clause 4.3.4.1).
        self.yq.clear();
        self.yq.resize(nd, 0);
        let hp_map = branches(c, h, false);
        let lp_map = branches(c, h, true);
        for blk in 0..nd / 126 {
            let mut b = [[0u8; 126]; 6];
            let hp_blk = &self.hp_bits[blk * 126 * hp_per..(blk + 1) * 126 * hp_per];
            for (w, bits) in hp_blk.chunks_exact(hp_per).enumerate() {
                for (&e, &bit) in hp_map.iter().zip(bits) {
                    b[e][w] = bit;
                }
            }
            if h.hierarchical() {
                let lp_blk = &self.lp_bits[blk * 126 * (v - 2)..(blk + 1) * 126 * (v - 2)];
                for (w, bits) in lp_blk.chunks_exact(v - 2).enumerate() {
                    for (&e, &bit) in lp_map.iter().zip(bits) {
                        b[e][w] = bit;
                    }
                }
            }
            for (w, word) in self.yq[blk * 126..(blk + 1) * 126].iter_mut().enumerate() {
                *word = 0;
                for (e, be) in b.iter().enumerate().take(v) {
                    *word |= be[(w + BIT_OFFSETS[e]) % 126] << e;
                }
            }
        }
        // Symbol interleaver (clause 4.3.4.2).
        let perm = &self.layout.perm;
        self.y.clear();
        self.y.resize(nd, 0);
        if self.l.is_multiple_of(2) {
            for (&p, &w) in perm.iter().zip(&self.yq) {
                self.y[p as usize] = w;
            }
        } else {
            for (y, &p) in self.y.iter_mut().zip(perm.iter()) {
                *y = self.yq[p as usize];
            }
        }
        self.cells.clear();
        for &w in &self.y {
            // The label sends y0 first, in its most significant place.
            let label = (0..v).fold(0usize, |a, i| (a << 1) | ((w >> i) & 1) as usize);
            self.cells
                .push(self.points.point(label).unwrap_or_default());
        }
    }

    fn advance(&mut self) {
        self.l += 1;
        self.symbols += 1;
        if self.l == 68 {
            self.l = 0;
            self.frame = (self.frame + 1) & 3;
            self.tps_bits = tps::encode_with(&self.tps, self.frame, self.lp_field);
        }
    }

    /// Makes the next OFDM symbol (N + G samples at the natural rate, about unit mean power) from the packets of `hp` and
    /// `lp` (`lp` is read only in a hierarchical multiplex) and returns it.
    pub fn next_symbol(
        &mut self,
        hp: &mut dyn PacketSource,
        lp: &mut dyn PacketSource,
    ) -> &[Complex32] {
        self.make_cells(hp, lp);
        // TPS: symbol 0 takes its value from w_k. Then a 1 inverts.
        let tps_sign = if self.l == 0 {
            1.0
        } else if self.tps_bits[self.l] == 1 {
            -self.tps_prev
        } else {
            self.tps_prev
        };
        self.tps_prev = tps_sign;
        let g = self.tps.guard_samples();
        ofdm_symbol(
            &self.layout,
            self.l,
            &self.cells,
            tps_sign,
            self.scale,
            g,
            &mut self.ifft,
            &mut self.bins,
            &mut self.sym,
        );
        self.advance();
        &self.sym
    }
}

/// The samples of a modulator at an output rate: the symbols as they are at the natural rate, or through a resampler that
/// is flat over the occupied band. A rate below the occupied band cuts the outer carriers.
pub struct OutRate {
    rate: f64,
    resampler: Option<Resampler<Complex32>>,
    pending: Vec<Complex32>,
    at: usize,
}

impl OutRate {
    /// From `natural` Hz, with the occupied band of `mode`, to `rate` Hz.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoRate`] if `rate` is not a positive number.
    pub fn new(natural: f64, mode: Mode, rate: f64) -> Result<Self, ConfigError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Err(ConfigError::NoRate);
        }
        let mut resampler = None;
        if (rate - natural).abs() > 1e-9 * natural {
            let edge = mode.carriers() as f64 / mode.fft() as f64 / 2.0 * natural;
            let pass = (1.01 * edge / natural.min(rate)).clamp(0.42, 0.48);
            resampler = Some(
                Resampler::with_quality(natural, rate, pass, 70.0)
                    .map_err(|_| ConfigError::NoRate)?,
            );
        }
        Ok(OutRate {
            rate,
            resampler,
            pending: Vec::new(),
            at: 0,
        })
    }

    /// The output rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The output samples not yet taken.
    pub fn waiting(&self) -> usize {
        self.pending.len() - self.at
    }

    /// Adds one symbol at the natural rate.
    pub fn push(&mut self, symbol: &[Complex32]) {
        if self.at == self.pending.len() {
            self.pending.clear();
            self.at = 0;
        }
        match self.resampler.as_mut() {
            Some(r) => r.process(symbol, &mut self.pending),
            None => self.pending.extend_from_slice(symbol),
        }
    }

    /// Copies waiting samples into `out`; returns how many.
    pub fn take(&mut self, out: &mut [Complex32]) -> usize {
        let k = self.waiting().min(out.len());
        out[..k].copy_from_slice(&self.pending[self.at..self.at + k]);
        self.at += k;
        k
    }

    /// Moves every waiting sample to the end of `out`.
    pub fn take_all(&mut self, out: &mut Vec<Complex32>) {
        out.extend_from_slice(&self.pending[self.at..]);
        self.at = self.pending.len();
    }
}

/// The DVB-T test signal generator: the [`Modulator`] with its own packet sources.
pub struct DvbtGenerator {
    m: Modulator,
    hp: Box<dyn PacketSource>,
    lp: Box<dyn PacketSource>,
    out: OutRate,
}

impl DvbtGenerator {
    /// A generator.
    pub fn new(p: GenParams) -> Self {
        let m = Modulator::new(
            p.bandwidth,
            p.tps,
            p.puncture_phase,
            p.first_frame,
            p.lp_rate_field,
        );
        let natural = m.rate();
        let out = OutRate::new(natural, p.tps.mode, natural)
            .unwrap_or_else(|_| unreachable!("the natural rate is positive"));
        DvbtGenerator {
            m,
            hp: p.hp,
            lp: p.lp,
            out,
        }
    }

    /// The generator with `generate` at `rate_hz` in place of the natural rate. The symbols of `next_symbol` stay at the
    /// natural rate. The filter is flat over the occupied band; a rate below the occupied band cuts the outer carriers.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoRate`] if `rate_hz` is not a positive number.
    pub fn at_rate(mut self, rate_hz: f64) -> Result<Self, ConfigError> {
        self.out = OutRate::new(self.m.rate(), self.m.tps.mode, rate_hz)?;
        Ok(self)
    }

    /// The natural sample rate.
    pub fn rate(&self) -> f64 {
        self.m.rate()
    }

    /// The symbol index in the frame (0 to 67) and the frame number of the next symbol.
    pub fn position(&self) -> (usize, u8) {
        self.m.position()
    }

    /// The next symbol's data cells in carrier order (after the symbol interleaver and mapping), with its index in the frame. The
    /// generator's own symbol counters advance (used without OFDM for the error-correction tests).
    pub fn next_cells(&mut self) -> (Vec<Complex32>, usize) {
        self.m.next_cells(&mut *self.hp, &mut *self.lp)
    }

    /// Makes the next OFDM symbol (N + G samples) into the internal buffer and returns it.
    pub fn next_symbol(&mut self) -> &[Complex32] {
        self.m.next_symbol(&mut *self.hp, &mut *self.lp)
    }
}

/// Makes OFDM symbol `l` of a DVB-T family layout into `out` (guard first, `N + guard` samples): `cells` on the data carriers in
/// carrier order, the boosted pilots, and the TPS carriers at `tps_sign` times their w_k value, all times `scale`. `bins` (N)
/// is working space.
#[allow(clippy::too_many_arguments)]
pub fn ofdm_symbol(
    lay: &Layout,
    l: usize,
    cells: &[Complex32],
    tps_sign: f32,
    scale: f32,
    guard: usize,
    ifft: &mut Fft,
    bins: &mut [Complex32],
    out: &mut Vec<Complex32>,
) {
    let n = bins.len();
    let p = l % 4;
    bins.fill(Complex32::default());
    let mut d = 0;
    for k in 0..lay.k {
        let v = match lay.kind[p][k] {
            CarrierKind::Data => {
                let c = cells.get(d).copied().unwrap_or_default();
                d += 1;
                c
            }
            CarrierKind::Scattered | CarrierKind::Continual => Complex32::new(lay.pilot[k], 0.0),
            CarrierKind::Tps => Complex32::new(tps_sign * (1.0 - 2.0 * lay.wk[k] as f32), 0.0),
        };
        bins[lay.bin(k)] = v * scale;
    }
    ifft.process(bins);
    out.clear();
    out.extend_from_slice(&bins[n - guard..]);
    out.extend_from_slice(bins);
}

impl Generator for DvbtGenerator {
    fn rate_hz(&self) -> f64 {
        self.out.rate()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.out.waiting() == 0 {
                let s = self.m.next_symbol(&mut *self.hp, &mut *self.lp);
                self.out.push(s);
                continue;
            }
            i += self.out.take(&mut out[i..]);
        }
        out.len()
    }

    /// The occupied band (K carriers), the cell rate and the net bit rate of the high-priority stream, at the mean power the
    /// generator makes.
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        let rate = self.rate();
        let tps = &self.m.tps;
        let n = tps.mode.fft() as f64;
        let width = tps.mode.carriers() as f64 * rate / n;
        let cells = tps.mode.data_carriers() as f64 * rate / tps.symbol_samples() as f64;
        Some(
            sdrkit_core::SignalInfo::new(width)
                .with_symbol_rate(cells)
                .with_bit_rate(tps.net_bitrate(self.m.bandwidth, false))
                .with_power(MEAN_POWER),
        )
    }
}

/// The mean power of the generated signal: the data cells have unit power, and the boosted pilots (4/3 in amplitude) add about
/// 5 % (clause 4.5.3).
pub const MEAN_POWER: f64 = 1.05;
