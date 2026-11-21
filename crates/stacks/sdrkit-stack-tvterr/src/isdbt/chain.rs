//! One ISDB-T layer from packets to cells and back, as streams. Transmit: RS(204,188), energy dispersal per frame, byte
//! interleaving, the inner code and its puncturing, bit interleaving with its delay adjustment, mapping, and the time interleaver.
//! Receive: the inverse, with a streaming Viterbi decoder and the two bit-delay models that transmitters use.
//!
//! Standard: ARIB STD-B31 clauses 3.3 to 3.11.

use super::fec::{constellation, label, points, puncture, INNER};
use super::layout::{bit_delays, ti_adjust, ti_delays, ti_length};
use super::tmcc::LayerParams;
use num_complex::Complex32;
use sdrkit_algo_fec::conv::{puncture as puncture_into, Depuncturer, Encoder, Puncture, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_proto_mpegts::outer::{Dispersal, OuterDecoder, OuterEncoder, OuterWord, WordStatus};
use sdrkit_util_bits::interleave::DelayLanes;

/// The two transmitter models of the bit-interleaving delay adjustment: the whole delay is one frame, or two OFDM symbols.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BitDelay {
    /// A whole frame (the common case).
    #[default]
    Frame,
    /// Two OFDM symbols.
    TwoSymbols,
}

impl BitDelay {
    /// The total delay in cells for a layer of `l` cells per symbol.
    pub fn cells(self, l: usize) -> usize {
        match self {
            BitDelay::Frame => 204 * l,
            BitDelay::TwoSymbols => 2 * l,
        }
    }
    /// The symbol of the frame at which the receiver sees a frame's first cell.
    pub fn start_symbol(self) -> usize {
        match self {
            BitDelay::Frame => 0,
            BitDelay::TwoSymbols => 2,
        }
    }
    fn other(self) -> BitDelay {
        match self {
            BitDelay::Frame => BitDelay::TwoSymbols,
            BitDelay::TwoSymbols => BitDelay::Frame,
        }
    }
}

/// Packets per frame of a layer.
pub fn packets_per_frame(mode: u8, p: &LayerParams) -> usize {
    super::packets_per_segment(mode, p) * p.segments as usize
}

/// The transmit chain of one layer.
pub struct LayerTx {
    p: LayerParams,
    l: usize,
    outer: OuterEncoder,
    enc: Encoder,
    pattern: Puncture<'static>,
    phase: usize,
    coded: Vec<u8>,
    bits: Vec<u8>,
    bil: DelayLanes<u8>,
    map: Vec<Complex32>,
    cells: Vec<Complex32>,
    dq: Vec<Complex32>,
    ti: DelayLanes<Complex32>,
}

impl LayerTx {
    /// A layer of `mode` with these parameters and bit-delay model.
    pub fn new(mode: u8, p: LayerParams, model: BitDelay) -> LayerTx {
        let d = 96usize << (mode - 1);
        let l = p.segments as usize * d;
        let nb = p.bits() as usize;
        let adj = (model.cells(l) - 120) as u32;
        let delays: Vec<u32> = bit_delays(nb).iter().map(|&x| x + adj).collect();
        let i_len = ti_length(mode, p.interleave);
        // The bit interleaver starts full of random bits, as a transmitter that has been on the air.
        let mut bil = DelayLanes::new(&delays, 0u8);
        let total: u32 = delays.iter().sum();
        let mut rng = sdrkit_algo_sim::rng::Rng::new(0x2545_F491);
        for _ in 0..total {
            bil.push(rng.bit());
        }
        bil.sync();
        let map = points(p.modulation).points().to_vec();
        // The time interleaver starts full of random cells too.
        let tid = ti_delays(l, d, i_len, false);
        let mut ti = DelayLanes::new(&tid, Complex32::new(0.0, 0.0));
        let held: u32 = tid.iter().sum();
        for _ in 0..held {
            ti.push(map[rng.below(map.len() as u64) as usize]);
        }
        ti.sync();
        LayerTx {
            p,
            l,
            outer: OuterEncoder::new(Dispersal::IsdbFrame {
                words: packets_per_frame(mode, &p),
            }),
            enc: Encoder::new(INNER),
            pattern: puncture(p.code_rate),
            phase: 0,
            coded: Vec::new(),
            bits: Vec::new(),
            bil,
            map,
            cells: Vec::with_capacity(204 * l),
            dq: vec![Complex32::new(1.0, 0.0); l],
            ti,
        }
    }
    /// Cells per symbol.
    pub fn cells_per_symbol(&self) -> usize {
        self.l
    }
    /// Encodes one frame of packets (exactly the layer's packets per frame) into the frame's cells (before time interleaving).
    pub fn encode_frame(&mut self, packets: &[[u8; 188]]) {
        let mut w = [0u8; 204];
        let mut data = Vec::with_capacity(packets.len() * 204 * 8);
        for pk in packets {
            self.outer.encode(pk, &mut w);
            for &y in &w {
                for i in (0..8).rev() {
                    data.push((y >> i) & 1);
                }
            }
        }
        self.coded.clear();
        self.enc.encode(&data, &mut self.coded);
        puncture_into(&self.pattern, self.phase, &self.coded, &mut self.bits);
        self.phase = (self.phase + self.coded.len()) % self.pattern.period();
        let nb = self.p.bits() as usize;
        let full = self.bits.len() / nb * nb;
        self.cells.clear();
        let mut c = [0u8; 6];
        for chunk in self.bits[..full].chunks(nb) {
            c[..nb].copy_from_slice(chunk);
            self.bil.process(&mut c[..nb]);
            self.cells.push(self.map[label(&c[..nb])]);
        }
        self.bits.drain(..full);
    }
    /// The cells of symbol `t` of the frame, after the differential mapping and the time interleaver.
    pub fn symbol(&mut self, t: usize, out: &mut [Complex32]) {
        let l = self.l;
        out[..l].copy_from_slice(&self.cells[t * l..(t + 1) * l]);
        if self.p.modulation == 0 {
            for (z, q) in self.dq.iter_mut().zip(out.iter_mut()) {
                *z *= *q;
                *q = *z;
            }
        }
        self.ti.process(&mut out[..l]);
    }
}

/// Per-layer counters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayerCounters {
    /// Packets delivered.
    pub packets: u64,
    /// Reed-Solomon words clean.
    pub rs_clean: u64,
    /// Corrected.
    pub rs_corrected: u64,
    /// Failed.
    pub rs_failed: u64,
    /// Viterbi margin: the best path's metric growth over the largest it can have (1 = noiseless).
    pub viterbi_margin: f32,
    /// Reed-Solomon has succeeded (byte alignment found).
    pub locked: bool,
    /// The bit-delay model in use.
    pub model: BitDelay,
}

/// The time de-interleaver of one layer (receive side): cells with their noise variance. Cells not received yet come out as
/// erasures (infinite variance).
pub struct LayerTi {
    l: usize,
    held: usize,
    lanes: DelayLanes<(Complex32, f32)>,
}

impl LayerTi {
    /// For a layer of `mode`.
    pub fn new(mode: u8, p: &LayerParams) -> LayerTi {
        let d = 96usize << (mode - 1);
        let l = p.segments as usize * d;
        let delays = ti_delays(l, d, ti_length(mode, p.interleave), true);
        LayerTi {
            l,
            held: delays.iter().map(|&x| x as usize).sum(),
            lanes: DelayLanes::new(&delays, (Complex32::new(0.0, 0.0), f32::INFINITY)),
        }
    }
    /// One symbol of the layer's cells, in place.
    pub fn push(&mut self, cells: &mut [(Complex32, f32)]) {
        self.lanes.process(&mut cells[..self.l]);
    }
    /// Bytes held.
    pub fn memory(&self) -> usize {
        self.held * std::mem::size_of::<(Complex32, f32)>()
    }
}

/// The traceback depth of the layer Viterbi decoder: enough for rate 7/8.
const TRACEBACK: usize = 128;
/// Bits the layer Viterbi decoder releases at a time.
const CHUNK: usize = 32;

/// The receive chain of one layer after the time de-interleaver.
pub struct LayerRx {
    p: LayerParams,
    l: usize,
    con: Constellation,
    bdi: DelayLanes<f32>,
    llr: Vec<f32>,
    q: Vec<i8>,
    dep: Vec<i8>,
    model: BitDelay,
    started: bool,
    depunct: Depuncturer<'static>,
    vit: Viterbi,
    abs_sum: u64,
    vbits: Vec<u8>,
    byte: u8,
    nbit: u8,
    bytes: Vec<u8>,
    outer: OuterDecoder,
    words: Vec<OuterWord>,
    fails: u32,
    ever_ok: bool,
    syms: usize,
    fill: usize,
    /// Counters.
    pub c: LayerCounters,
}

impl LayerRx {
    /// A layer of `mode`, starting with the bit-delay model `model`.
    pub fn new(mode: u8, p: LayerParams, model: BitDelay) -> LayerRx {
        let d = 96usize << (mode - 1);
        let l = p.segments as usize * d;
        let nb = p.bits() as usize;
        let i_len = ti_length(mode, p.interleave);
        let delays: Vec<u32> = bit_delays(nb).iter().map(|&x| 120 - x).collect();
        let npk = packets_per_frame(mode, &p);
        let mut vit = Viterbi::new(INNER, 0).unwrap_or_else(|_| unreachable!());
        let _ = vit.set_traceback(TRACEBACK, CHUNK);
        LayerRx {
            p,
            l,
            con: constellation(p.modulation),
            bdi: DelayLanes::new(&delays, 0.0),
            llr: Vec::with_capacity(l * nb),
            q: Vec::with_capacity(l * nb),
            dep: Vec::with_capacity(4 * l * nb),
            model,
            started: false,
            depunct: Depuncturer::new(puncture(p.code_rate)),
            vit,
            abs_sum: 0,
            vbits: Vec::with_capacity(8 * l * nb),
            byte: 0,
            nbit: 0,
            bytes: Vec::with_capacity(l * nb),
            outer: OuterDecoder::new(Dispersal::IsdbFrame { words: npk }),
            words: Vec::with_capacity(l * nb / 204 + 1),
            fails: 0,
            ever_ok: false,
            syms: 0,
            fill: 95 * i_len + ti_adjust(i_len) + 4 * 204,
            c: LayerCounters {
                model,
                ..Default::default()
            },
        }
    }
    /// The layer's parameters.
    pub fn params(&self) -> LayerParams {
        self.p
    }
    /// Cells per symbol.
    pub fn cells_per_symbol(&self) -> usize {
        self.l
    }
    fn restart(&mut self) {
        self.started = false;
        self.depunct.reset(0);
        self.vit.reset();
        self.abs_sum = 0;
        self.vbits.clear();
        self.nbit = 0;
        self.outer.restart();
    }
    /// One symbol of the layer's cells (de-interleaved in frequency and time) with their noise variance. `frame_pos` is the
    /// symbol's index in its frame. Decoded packets go to `out` with whether Reed-Solomon accepted them (a refused packet carries the
    /// transport_error_indicator). A cell that is not finite, or whose variance is not finite, is an erasure.
    pub fn push_symbol(
        &mut self,
        cells: &[(Complex32, f32)],
        frame_pos: usize,
        out: &mut Vec<([u8; 188], bool)>,
    ) {
        self.syms += 1;
        let nb = self.p.bits() as usize;
        if !self.started && frame_pos == self.model.start_symbol() {
            self.restart();
            self.started = true;
        }
        self.llr.clear();
        let mut soft = [0f32; 6];
        for &(c, n) in cells[..self.l].iter() {
            let w = if n.is_finite() {
                1.0 / n.max(1e-6)
            } else {
                0.0
            };
            if self
                .con
                .demap_weighted(c, w, Demap::MaxLog, &mut soft[..nb])
                .is_err()
            {
                soft[..nb].fill(0.0);
            }
            self.llr.extend_from_slice(&soft[..nb]);
        }
        for cell in self.llr.chunks_exact_mut(nb) {
            self.bdi.process(cell);
        }
        if !self.started {
            return;
        }
        self.q.clear();
        self.q.resize(self.llr.len(), 0);
        let qz = Quantizer::default();
        let _ = qz.quantize_auto(&self.llr, &mut self.q);
        self.dep.clear();
        self.depunct.push(&self.q, &mut self.dep);
        self.abs_sum += self
            .dep
            .iter()
            .map(|&v| v.unsigned_abs() as u64)
            .sum::<u64>();
        self.vit.push(&self.dep, &mut self.vbits);
        self.bytes(out);
        if self.vit.steps() > 0 && self.abs_sum > 0 {
            let best = self.abs_sum as f32 / self.vit.steps() as f32;
            self.c.viterbi_margin = (self.vit.mean_gain() / best).clamp(0.0, 1.0);
        }
    }

    fn bytes(&mut self, out: &mut Vec<([u8; 188], bool)>) {
        self.bytes.clear();
        for &b in &self.vbits {
            self.byte = (self.byte << 1) | b;
            self.nbit += 1;
            if self.nbit == 8 {
                self.nbit = 0;
                self.bytes.push(self.byte);
            }
        }
        self.vbits.clear();
        let words = &mut self.words;
        words.clear();
        self.outer.push(&self.bytes, &mut |w| words.push(*w));
        let words = std::mem::take(&mut self.words);
        for w in &words {
            if !self.packet(w, out) {
                break;
            }
        }
        self.words = words;
    }

    /// One decoded word. Returns false when the layer starts again with the other bit-delay model.
    fn packet(&mut self, w: &OuterWord, out: &mut Vec<([u8; 188], bool)>) -> bool {
        let ok = !w.failed();
        if self.syms <= self.fill {
            if ok {
                self.ever_ok = true;
                self.c.locked = true;
            }
            return true;
        }
        match w.status {
            WordStatus::Clean => self.c.rs_clean += 1,
            WordStatus::Corrected { .. } => self.c.rs_corrected += 1,
            WordStatus::Failed => self.c.rs_failed += 1,
        }
        if ok {
            self.ever_ok = true;
            self.c.locked = true;
            self.fails = 0;
        } else {
            self.fails += 1;
            if self.fails >= 64 && !self.ever_ok {
                // The other transmitter model of the bit-delay adjustment (statistics kept).
                self.model = self.model.other();
                self.c.model = self.model;
                self.fails = 0;
                self.started = false;
                return false;
            }
        }
        self.c.packets += 1;
        out.push((w.packet, ok));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_mpegts::source::{Numbered, NumberedCheck};

    fn chain(
        mode: u8,
        p: LayerParams,
        model: BitDelay,
        rx_model: BitDelay,
        frames: usize,
        sigma: f32,
    ) -> usize {
        let mut tx = LayerTx::new(mode, p, model);
        let mut rx = LayerRx::new(mode, p, rx_model);
        let mut ti = LayerTi::new(mode, &p);
        let npk = packets_per_frame(mode, &p);
        let l = tx.cells_per_symbol();
        let mut sym = vec![Complex32::new(0.0, 0.0); l];
        let mut cells = vec![(Complex32::new(0.0, 0.0), 0.0f32); l];
        let mut n = 0u32;
        let mut got = Vec::new();
        let mut rng = sdrkit_algo_sim::rng::Rng::new(7);
        let pattern = Numbered::TERRESTRIAL;
        let mut prev = vec![Complex32::new(1.0, 0.0); l];
        for _ in 0..frames {
            let pk: Vec<[u8; 188]> = (0..npk)
                .map(|_| {
                    n += 1;
                    pattern.packet(n - 1)
                })
                .collect();
            tx.encode_frame(&pk);
            for t in 0..204 {
                tx.symbol(t, &mut sym);
                for i in 0..l {
                    let v = sym[i] + rng.complex_normal(2.0 * (sigma * sigma) as f64);
                    // A DQPSK layer is detected differentially (here on the time-interleaved stream, as the receiver does on
                    // its carriers).
                    cells[i] = if p.modulation == 0 {
                        let d = v * prev[i].conj();
                        prev[i] = v;
                        (d, 2.0 * sigma * sigma + 1e-3)
                    } else {
                        (v, 2.0 * sigma * sigma + 1e-3)
                    };
                }
                ti.push(&mut cells);
                rx.push_symbol(&cells, t, &mut got);
            }
        }
        let mut c = NumberedCheck::default();
        for (p, _) in &got {
            c.push(&pattern, p, 0.0);
        }
        assert_eq!(c.wrong, 0);
        assert_eq!(c.gaps, 0);
        c.good as usize
    }

    #[test]
    fn layer_chains_round_trip_with_both_models() {
        let lp = |m, r, i, s| LayerParams {
            modulation: m,
            code_rate: r,
            interleave: i,
            segments: s,
        };
        // Mode 1, 1 segment 16QAM 2/3, I = 4: fill 95*4+28+816 = 1224 symbols = 6 frames.
        let good = chain(1, lp(2, 1, 1, 1), BitDelay::Frame, BitDelay::Frame, 9, 0.0);
        assert!(good >= 2 * 32, "{good}");
        // The receiver starts with the frame model and finds the two-symbol one.
        let good = chain(
            1,
            lp(1, 3, 0, 2),
            BitDelay::TwoSymbols,
            BitDelay::Frame,
            10,
            0.0,
        );
        assert!(good >= 3 * 40, "{good}");
        // DQPSK 1/2, no time interleaving, a little noise.
        let good = chain(1, lp(0, 0, 0, 1), BitDelay::Frame, BitDelay::Frame, 7, 0.05);
        assert!(good >= 2 * 12, "{good}");
        // 64QAM 7/8.
        let good = chain(1, lp(3, 4, 0, 1), BitDelay::Frame, BitDelay::Frame, 7, 0.0);
        assert!(good >= 2 * 63, "{good}");
    }
}
