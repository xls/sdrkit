//! The enhanced layer of ATSC 3.0: the core PLPs under an enhanced PLP are decoded, encoded and mapped again, and taken
//! off the cells after time de-interleaving; the enhanced PLP is decoded from what is left.
//!
//! Standard: ATSC A/322:2026-06 clauses 6.4 and 7.2.7.4; A/327:2021 clause 6.4.

use super::bicm::{BlockResult, PlpCoding, PlpDecoder, PlpEncoder, PlpPoints, PlpStream};
use super::rx::{EnhancedPlp, Layers, NEXT_CORES};
use super::ti::{Deinterleaved, TimeDeinterleaver};
use sdrkit_core::Complex32;
use std::collections::VecDeque;

/// The tag of a core cell with no enhanced cell on it.
const NONE: u64 = u64::MAX;

/// Every this many enhanced cells, one counts in the modulation error ratio.
const MER_STEP: u64 = 32;

/// The cells of one core PLP in one subframe after time de-interleaving.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoreCells<'a> {
    /// The cells `W = z / beta`.
    pub cells: &'a [Complex32],
    /// Their weights `beta^2 |H|^2 / sigma^2`.
    pub weights: &'a [f32],
    /// The first cell of the first core FEC block that begins in them.
    pub block_start: Option<usize>,
    /// The cells that the de-interleaver left out before them.
    pub skipped: usize,
}

impl<'a> From<&Deinterleaved<'a>> for CoreCells<'a> {
    fn from(d: &Deinterleaved<'a>) -> CoreCells<'a> {
        CoreCells {
            cells: d.cells,
            weights: d.weights,
            block_start: d.block_start,
            skipped: d.skipped,
        }
    }
}

/// One core PLP under the enhanced PLP: its decoder and encoder, and its cells after the last whole FEC block.
struct CoreLayer {
    decoder: PlpDecoder,
    encoder: PlpEncoder,
    /// The cells from the first cell of a core block, their weights, and the number of the enhanced cell on each among
    /// all enhanced cells taken (`NONE` for none).
    cells: Vec<Complex32>,
    weights: Vec<f32>,
    tags: Vec<u64>,
    /// `cells` begins at the first cell of a core FEC block.
    synced: bool,
    core_weights: Vec<f32>,
    regenerated: Vec<Complex32>,
    packet: Vec<u8>,
}

/// The enhanced PLP: its stream of FEC blocks and what the core layers give it.
struct EnhancedSide {
    stream: PlpStream,
    /// The number of the first cell of some enhanced FEC block.
    grid: Option<u64>,
    /// The number of the enhanced cell that `stream` takes next.
    given: u64,
    left: (Vec<Complex32>, Vec<f32>),
    /// The constellation, and the power of the nearest points and of the errors of the cells measured since
    /// [`LayerDecoder::take_mer`].
    points: PlpPoints,
    mer: (f64, f64),
}

/// The decoder of an enhanced PLP from the cells of its core PLPs. After the first blocks it allocates nothing.
pub struct LayerDecoder {
    /// The core PLPs under the enhanced PLP, in the order of L1-Detail.
    cores: Vec<CoreLayer>,
    /// The time de-interleavers of the core PLPs after the first.
    next_ti: [TimeDeinterleaver; NEXT_CORES],
    side: EnhancedSide,
    /// The enhanced cells taken.
    seen: u64,
    /// The layers of the subframes taken, newest first: those of the cells before time interleaving that the
    /// de-interleaver gives back some subframes later.
    history: VecDeque<Layers>,
}

impl CoreLayer {
    fn new(coding: PlpCoding) -> Option<CoreLayer> {
        Some(CoreLayer {
            decoder: PlpDecoder::new(coding)?,
            encoder: PlpEncoder::new(coding)?,
            cells: Vec::new(),
            weights: Vec::new(),
            tags: Vec::new(),
            synced: false,
            core_weights: Vec::new(),
            regenerated: Vec::new(),
            packet: Vec::new(),
        })
    }

    fn restart(&mut self) {
        self.cells.clear();
        self.weights.clear();
        self.tags.clear();
        self.synced = false;
    }

    /// Takes the cells `c` of this core PLP in one subframe, with the enhanced cells `from..from + count` among them of
    /// numbers from `base`, and gives each whole core block, taken off the enhanced cells on it, to `side`. False when
    /// a block could not be encoded again.
    fn take(
        &mut self,
        c: &CoreCells,
        (from, count, base): (usize, usize, u64),
        (alpha, max_iter): (f32, u32),
        side: &mut EnhancedSide,
        each: &mut impl FnMut(BlockResult, &[u8]),
    ) -> bool {
        let tag = |i: usize| {
            if (from..from + count).contains(&i) {
                base + (i - from) as u64
            } else {
                NONE
            }
        };
        // The core cells from the first cell of a core FEC block.
        let block = self.decoder.coding().cells();
        let skip = match (self.synced, c.block_start) {
            (true, Some(s)) if !(self.cells.len() + s).is_multiple_of(block) => {
                self.restart();
                s
            }
            (true, _) => 0,
            (false, Some(s)) => s,
            (false, None) => c.cells.len(),
        };
        let skip = skip.min(c.cells.len());
        if !self.synced {
            self.synced = skip < c.cells.len();
        }
        self.cells.extend_from_slice(&c.cells[skip..]);
        self.weights.extend_from_slice(&c.weights[skip..]);
        self.tags.extend((skip..c.cells.len()).map(tag));
        let (alpha2, gain) = (alpha * alpha, 1.0 / alpha);
        let k = self.decoder.coding().outer_bits();
        let mut at = 0;
        while self.synced && at + block <= self.cells.len() {
            let r = at..at + block;
            self.core_weights.clear();
            self.core_weights.extend(
                self.weights[r.clone()]
                    .iter()
                    .map(|&v| v / (1.0 + v * alpha2)),
            );
            let result = self.decoder.decode(
                &self.cells[r.clone()],
                &self.core_weights,
                max_iter,
                &mut self.packet,
            );
            // A/327 clause 6.4.3: the code word of the LDPC decoder when its checks closed and the outer code changed
            // nothing; another good block is encoded again from its outer code word; a bad one keeps its hard decisions,
            // whose errors stay where they are.
            self.regenerated.clear();
            let word = result.checks_closed && result.outer_corrected == 0;
            let made = if result.good && !word {
                self.encoder
                    .outer_word_cells(&self.decoder.word()[..k], &mut self.regenerated)
            } else {
                self.encoder
                    .code_word_cells(self.decoder.word(), &mut self.regenerated)
            };
            if made.is_err() {
                return false;
            }
            let mut first = NONE;
            for (j, i) in r.enumerate() {
                let t = self.tags[i];
                if t == NONE {
                    continue;
                }
                if first != NONE && t != first + side.left.0.len() as u64 {
                    side.give(first, max_iter, each);
                    first = NONE;
                }
                if first == NONE {
                    first = t;
                }
                let z = (self.cells[i] - self.regenerated[j]) * gain;
                if t.is_multiple_of(MER_STEP) && self.weights[i] > 0.0 {
                    let p = side.points.nearest(z);
                    side.mer.0 += f64::from(p.norm_sqr());
                    side.mer.1 += f64::from((z - p).norm_sqr());
                }
                side.left.0.push(z);
                side.left.1.push(self.weights[i] * alpha2);
            }
            if first != NONE {
                side.give(first, max_iter, each);
            }
            at += block;
        }
        self.cells.drain(..at);
        self.weights.drain(..at);
        self.tags.drain(..at);
        true
    }
}

impl EnhancedSide {
    /// Gives the enhanced cells in `self.left`, the first of number `first`, to the enhanced decoder, and empties it.
    fn give(&mut self, first: u64, max_iter: u32, each: &mut impl FnMut(BlockResult, &[u8])) {
        let n = self.left.0.len() as u64;
        if first != self.given {
            self.stream.restart();
        }
        self.given = first + n;
        let block = self.stream.coding().cells() as u64;
        let start = self.grid.map(|g| {
            if first >= g {
                (block - (first - g) % block) % block
            } else {
                (g - first) % block
            }
        });
        match start {
            Some(s) => self.stream.frame(
                &self.left.0,
                &self.left.1,
                (s < n).then_some(s as usize),
                max_iter,
                each,
            ),
            None => self.stream.restart(),
        }
        self.left.0.clear();
        self.left.1.clear();
    }
}

impl LayerDecoder {
    /// The decoder of an enhanced PLP coded as `enhanced` on the core PLPs coded as `cores` (1 to `1 + NEXT_CORES`).
    /// `None` when a part of a coding does not exist.
    pub fn new(cores: &[PlpCoding], enhanced: PlpCoding) -> Option<LayerDecoder> {
        if cores.is_empty() || cores.len() > 1 + NEXT_CORES {
            return None;
        }
        Some(LayerDecoder {
            cores: cores.iter().map(|&c| CoreLayer::new(c)).collect::<Option<_>>()?,
            next_ti: std::array::from_fn(|_| TimeDeinterleaver::default()),
            side: EnhancedSide {
                stream: PlpStream::new(PlpDecoder::new(enhanced)?),
                grid: None,
                given: 0,
                left: (Vec::new(), Vec::new()),
                points: PlpPoints::new(enhanced.bits, enhanced.rate)?,
                mer: (0.0, 0.0),
            },
            seen: 0,
            history: VecDeque::with_capacity(16),
        })
    }

    /// The decoder for the enhanced PLP `e` on a first core PLP coded as `core`.
    pub fn of(core: PlpCoding, e: &EnhancedPlp) -> Option<LayerDecoder> {
        let mut cores = [core; 1 + NEXT_CORES];
        let mut n = 1;
        for (slot, next) in cores[1..].iter_mut().zip(e.next.iter().flatten()) {
            *slot = next.coding;
            n += 1;
        }
        LayerDecoder::new(&cores[..n], e.coding)
    }

    /// Whether this decodes the enhanced PLP `e` on a first core PLP coded as `core`.
    pub fn fits(&self, core: PlpCoding, e: &EnhancedPlp) -> bool {
        let codings = std::iter::once(core).chain(e.next.iter().flatten().map(|n| n.coding));
        *self.side.stream.coding() == e.coding
            && self.cores.len() == 1 + e.next.iter().flatten().count()
            && self.cores.iter().zip(codings).all(|(c, k)| *c.decoder.coding() == k)
    }

    /// The modulation error ratio of the enhanced cells after the core PLPs are taken off, against the nearest points of
    /// the enhanced constellation, since the last call, dB. `None` when no cell was measured.
    pub fn take_mer(&mut self) -> Option<f32> {
        let (signal, error) = std::mem::take(&mut self.side.mer);
        (signal > 0.0 && error > 0.0).then(|| (10.0 * (signal / error).log10()) as f32)
    }

    /// Forgets the cells of the frames before.
    pub fn restart(&mut self) {
        self.cores.iter_mut().for_each(CoreLayer::restart);
        self.next_ti.iter_mut().for_each(TimeDeinterleaver::restart);
        self.side.grid = None;
        self.side.stream.restart();
    }

    /// Takes the cells of the core PLPs under the enhanced PLP in one subframe (as [`super::rx::FrameDecoder::part`]
    /// gives them for an enhanced PLP): `first` those of the first core PLP after time de-interleaving, `next` the cells
    /// and weights of the core PLPs after it as received, with whether they follow those of the subframe before.
    /// `layers` holds the enhanced PLP of this subframe; the cells are those before time interleaving of the subframe
    /// `late` subframes before this one, and the layers of that subframe count, with the skipped cells. Each core block
    /// is decoded with the enhanced layer as noise, encoded and mapped again and taken off the cells under the enhanced
    /// PLP: `(W - S_core) / alpha` with the weight times `alpha^2` (A/327 clause 6.4.3, on cells that the channel is
    /// divided out of). `each` gets the result and the baseband packet of every enhanced block that ends.
    pub fn frame(
        &mut self,
        first: CoreCells,
        next: (&[Complex32], &[f32], bool),
        (layers, late): (&Layers, usize),
        max_iter: u32,
        mut each: impl FnMut(BlockResult, &[u8]),
    ) {
        self.history.truncate(15);
        self.history.push_front(*layers);
        let Some(&layers) = self.history.get(late) else {
            self.restart();
            return;
        };
        let Some(e) = layers.enhanced else {
            self.restart();
            return;
        };
        let count = 1 + e.next.iter().flatten().count();
        let (next_cells, next_weights, follows) = next;
        if self.cores.len() != count || next_cells.len() != layers.next_cells() || next_weights.len() != next_cells.len() {
            self.restart();
            return;
        }
        // The cells of each core PLP after time de-interleaving.
        let mut cores = [first; 1 + NEXT_CORES];
        let mut at = 0;
        let mut restart = [false; 1 + NEXT_CORES];
        for (k, (n, ti)) in e.next.iter().flatten().zip(self.next_ti.iter_mut()).enumerate() {
            let r = at..at + n.cells;
            at += n.cells;
            match ti.frame(&n.ti, follows, n.coding.cells(), &next_cells[r.clone()], &next_weights[r], Some(0)) {
                Some(d) => {
                    restart[k + 1] = d.restart;
                    cores[k + 1] = CoreCells::from(&d);
                }
                None => {
                    cores[k + 1] = CoreCells::default();
                    restart[0] = true;
                }
            }
        }
        // The enhanced cells on each core PLP: from the place of the first, and from the first cell of the next ones.
        let mut spans = [(0, 0, 0); 1 + NEXT_CORES];
        let (mut sum, mut cut0, mut fits) = (0, 0, !restart[0]);
        for (k, (c, s)) in cores[..count].iter().zip(&mut spans).enumerate() {
            let (from, n) = match k {
                0 => e.span,
                _ => e.next[k - 1].map_or((0, 0), |n| (0, n.enhanced)),
            };
            let cut = c.skipped.saturating_sub(from).min(n);
            let (from, n) = (from.saturating_sub(c.skipped), n - cut);
            fits &= c.cells.len() == c.weights.len() && from + n <= c.cells.len();
            if k == 0 {
                cut0 = cut;
            }
            *s = (from, n, self.seen + sum);
            sum += n as u64;
        }
        if !fits {
            self.cores.iter_mut().for_each(CoreLayer::restart);
            self.side.grid = None;
            self.side.stream.restart();
            return;
        }
        // The first enhanced FEC block among the enhanced cells of this subframe.
        let base = self.seen;
        self.seen += sum;
        let block_e = self.side.stream.coding().cells() as u64;
        if let Some(s) = e.fec_block_start {
            let at = base + (s as u64 + block_e - cut0 as u64 % block_e) % block_e;
            if self.side.grid.is_some_and(|g| at.abs_diff(g) % block_e != 0) {
                self.side.stream.restart();
            }
            self.side.grid = Some(at);
        }
        let mut made = true;
        for (((core, c), &span), &again) in self.cores.iter_mut().zip(&cores).zip(&spans).zip(&restart) {
            if again {
                core.restart();
            }
            made &= core.take(c, span, (layers.alpha, max_iter), &mut self.side, &mut each);
        }
        if !made {
            self.restart();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::bicm::Outer;
    use super::super::gen::{Atsc3GenParams, FrameBuilder, GenLdm, GenPlp, GenTi, Payload};
    use super::super::rx::{FrameDecoder, HeadResult, PlpCells};
    use super::super::ti::TimeDeinterleaver;
    use super::*;

    fn coding(ldpc_bits: usize, rate: u8, bits: usize) -> PlpCoding {
        PlpCoding {
            ldpc_bits,
            rate,
            bits,
            outer: Outer::Bch,
        }
    }

    /// The core PLP of A/322 clause 6.4.4: QPSK 4/15 with the 64800-bit code.
    fn core() -> PlpCoding {
        coding(64800, 4, 2)
    }

    /// The enhanced PLP of A/322 clause 6.4.4: 64 points 10/15 with the 64800-bit code.
    fn enhanced() -> PlpCoding {
        coding(64800, 10, 6)
    }

    /// Frames of 24 symbols with the core PLP `core` and the enhanced PLP `enhanced` on it at injection level code `level`.
    fn layered(core: PlpCoding, enhanced: PlpCoding, level: u8, ti: GenTi) -> Atsc3GenParams {
        Atsc3GenParams {
            symbols: 24,
            coding: core,
            time_interleaver: ti,
            more_plps: vec![GenPlp {
                coding: enhanced,
                seed: 77,
                start: 0,
                cells: None,
                subslices: 1,
                interval: 0,
                time_interleaver: GenTi::None,
                lls: false,
                ldm: Some(GenLdm { core: 0, cores: 1, level }),
            }],
            ..Atsc3GenParams::default()
        }
    }

    /// Decodes PLP `want` of `frames` frames of `p`, with white noise at `snr_db` (unit signal power in 6.144 MHz) when it is
    /// given. Returns the good FEC blocks and all blocks; every good baseband packet must be one of the generator, in order.
    fn run(p: &Atsc3GenParams, want: u8, snr_db: Option<f64>, seed: u64, frames: usize) -> (usize, usize) {
        let (good, all, _) = run_mer(p, want, snr_db, seed, frames);
        (good, all)
    }

    /// [`run`], and the modulation error ratio of the enhanced cells of the last frame after the core PLP is taken off.
    fn run_mer(
        p: &Atsc3GenParams,
        want: u8,
        snr_db: Option<f64>,
        seed: u64,
        frames: usize,
    ) -> (usize, usize, Option<f32>) {
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        let mut rx = FrameDecoder::default();
        rx.select(Some(want));
        let g = if want == 0 {
            (p.coding, p.seed)
        } else {
            let m = &p.more_plps[usize::from(want) - 1];
            (m.coding, m.seed)
        };
        let mut payload = Payload::new(g.1);
        let sent: Vec<Vec<u8>> = (0..60 * frames)
            .map(|_| payload.packet(g.0.packet_bytes()))
            .collect();
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        let (mut stream, mut layers, mut ti) = (None, None, TimeDeinterleaver::default());
        let mut cells = PlpCells::default();
        let (mut at, mut good, mut all, mut follows, mut mer) = (0, 0, 0, false, None);
        for _ in 0..frames {
            let mut x = b.frame();
            if let Some(snr) = snr_db {
                let power = 10f64.powf(-snr / 10.0);
                x.iter_mut().for_each(|v| *v += rng.complex_normal(power));
            }
            let HeadResult::Ready(head) = rx.head(&p.fields, &x, 0.0) else {
                follows = false;
                continue;
            };
            assert_eq!(head.id, want);
            for k in 0..rx.parts().len() {
                let Some(plp) = rx.parts()[k].plp else {
                    continue;
                };
                assert!(rx.part(&head, k, &x, &mut cells));
                let first = cells.cells.len() - plp.layers.map_or(0, |l| l.next_cells());
                let (z, w) = (&cells.cells[..first], &cells.weights[..first]);
                let d = ti.frame(&plp.ti, follows, plp.coding.cells(), z, w, plp.fec_block_start).unwrap();
                let next = (&cells.cells[first..], &cells.weights[first..], follows);
                follows = true;
                let mut each = |r: BlockResult, packet: &[u8]| {
                    all += 1;
                    if r.good {
                        good += 1;
                        let found = sent[at..].iter().position(|w| w == packet);
                        at += found.expect("a packet of the generator, in order") + 1;
                    }
                };
                match plp.layers {
                    Some(l) if l.enhanced.is_some() => {
                        let e = l.enhanced.unwrap();
                        let dec = layers.get_or_insert_with(|| LayerDecoder::of(plp.coding, &e).unwrap());
                        if d.restart {
                            dec.restart();
                        }
                        let late = plp.ti.subframes_late();
                        dec.frame(CoreCells::from(&d), next, (&l, late), 50, &mut each);
                        mer = dec.take_mer();
                    }
                    _ => {
                        let s = stream.get_or_insert_with(|| PlpStream::new(PlpDecoder::new(plp.coding).unwrap()));
                        if d.restart {
                            s.restart();
                        }
                        s.frame(d.cells, d.weights, d.block_start, 50, &mut each);
                    }
                }
            }
        }
        (good, all, mer)
    }

    /// The processor time of the FEC stage on the default frame (72 symbols) with the PLPs of A/322 clause 6.4.4: the core
    /// PLP alone, the core PLP under the enhanced one, and the enhanced PLP.
    #[test]
    #[ignore = "a measurement: run it alone in an optimised build"]
    fn the_cost_of_each_layer() {
        let mut p = layered(core(), enhanced(), 8, GenTi::None);
        p.symbols = 72;
        let mut alone = p.clone();
        alone.more_plps.clear();
        let enhanced_alone = Atsc3GenParams {
            coding: enhanced(),
            ..alone.clone()
        };
        for (name, p, want) in [
            ("core alone", &alone, 0u8),
            ("enhanced alone", &enhanced_alone, 0),
            ("core", &p, 0),
            ("enhanced", &p, 1),
        ] {
            // The cells of 4 frames, then each decoded 3 times.
            let mut b = FrameBuilder::new(p.clone()).unwrap();
            let mut rx = FrameDecoder::default();
            rx.select(Some(want));
            let mut frames = Vec::new();
            for _ in 0..4 {
                let x = b.frame();
                let HeadResult::Ready(head) = rx.head(&p.fields, &x, 0.0) else {
                    panic!("a frame");
                };
                let plp = rx.parts()[0].plp.unwrap();
                let mut cells = PlpCells::default();
                assert!(rx.part(&head, 0, &x, &mut cells));
                frames.push((plp, cells));
            }
            let (mut blocks, mut iterations, mut took) = (0, 0, std::time::Duration::ZERO);
            for _ in 0..3 {
                let (mut stream, mut layers) = (None, None);
                let t = std::time::Instant::now();
                for (plp, c) in &frames {
                    let each = |r: BlockResult, _: &[u8]| {
                        blocks += 1;
                        iterations += r.iterations;
                    };
                    match plp.layers {
                        Some(l) if l.enhanced.is_some() => {
                            let first = CoreCells {
                                cells: &c.cells,
                                weights: &c.weights,
                                block_start: plp.fec_block_start,
                                skipped: 0,
                            };
                            layers
                                .get_or_insert_with(|| LayerDecoder::of(plp.coding, &l.enhanced.unwrap()).unwrap())
                                .frame(first, (&[], &[], true), (&l, 0), 50, each)
                        }
                        _ => stream
                            .get_or_insert_with(|| PlpStream::new(PlpDecoder::new(plp.coding).unwrap()))
                            .frame(&c.cells, &c.weights, plp.fec_block_start, 50, each),
                    }
                }
                took += t.elapsed();
            }
            let frame_ms = took.as_secs_f64() * 1e3 / 12.0;
            eprintln!(
                "{name}: {frame_ms:.1} ms a frame of 101.6 ms, {:.2} ms and {:.1} LDPC iterations a block",
                frame_ms * 12.0 / f64::from(blocks),
                f64::from(iterations) / f64::from(blocks)
            );
        }
    }

    /// The signalling of the enhanced PLP, and both layers without noise: on all cells of the core PLP without a time
    /// interleaver, and with the hybrid time interleaver (2 TI blocks, the cell interleaver) on 16200-bit core blocks of 16
    /// points, 4050 cells, which hold no whole number of the 10 800-cell enhanced blocks: dummy cells fill the rest.
    #[test]
    fn both_layers_decode_without_noise() {
        let p = layered(core(), enhanced(), 8, GenTi::None);
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        b.frame();
        let plps = &b.detail().subframes[0].plps;
        assert_eq!((plps[1].layer, plps[1].ldm_injection_level), (1, Some(8)));
        assert_eq!((plps[1].start, plps[1].size, plps[1].ti_mode), (0, plps[0].size, 0));
        // 3 frames of about 161 000 cells: 4.9 core blocks and 14.9 enhanced blocks a frame.
        assert_eq!(run(&p, 0, None, 1, 3), (15, 15));
        assert_eq!(run(&p, 1, None, 1, 3), (45, 45));
        let mut auto = FrameDecoder::default();
        let x = FrameBuilder::new(p.clone()).unwrap().frame();
        let HeadResult::Ready(head) = auto.head(&p.fields, &x, 0.0) else {
            panic!("a frame");
        };
        assert_eq!(head.id, 0);
        let hti = GenTi::Hybrid {
            inter_subframe: false,
            ti_blocks: 2,
            cell_interleaver: true,
            spare: 0,
        };
        // 10 dB below: 16 points 8/15 need more than the 4 dB of the first case.
        let mut p = layered(coding(16200, 8, 4), enhanced(), 15, hti);
        p.symbols = 23;
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        b.frame();
        let plps = &b.detail().subframes[0].plps;
        let (core_cells, enhanced_cells) = (plps[0].size as usize, plps[1].size as usize);
        assert_eq!((core_cells % 4050, enhanced_cells % 10_800), (0, 0));
        assert!(enhanced_cells < core_cells && core_cells < enhanced_cells + 10_800);
        let blocks = 3 * enhanced_cells / 10_800;
        assert_eq!(run(&p, 0, None, 1, 3), (3 * core_cells / 4050, 3 * core_cells / 4050));
        assert_eq!(run(&p, 1, None, 1, 3), (blocks, blocks));
        // Across 2 subframes: each TI block is whole one subframe after it began, with the enhanced PLP that the
        // subframe where it began signals.
        p.time_interleaver = GenTi::Hybrid {
            inter_subframe: true,
            ti_blocks: 2,
            cell_interleaver: true,
            spare: 0,
        };
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        b.frame();
        let plps = &b.detail().subframes[0].plps;
        assert_eq!((plps[0].hti.unwrap().inter_subframe, plps[1].ti_mode), (true, 2));
        // The first subframe carries half of its TI block and the cells that start the delay line.
        let (core_blocks, enhanced_cells) = (usize::from(plps[0].hti.unwrap().fec_blocks[0]) + 1, plps[1].size as usize);
        assert!(core_blocks * 4050 > plps[0].size as usize);
        assert_eq!(run(&p, 0, None, 1, 4), (3 * core_blocks, 3 * core_blocks));
        let blocks = 3 * enhanced_cells / 10_800;
        assert_eq!(run(&p, 1, None, 1, 4), (blocks, blocks));
    }

    /// Both layers under the convolutional time interleaver of 512 rows: after the 261 632 cells that fill it, every block
    /// of 4 frames decodes, without noise and at 24 dB.
    #[test]
    fn both_layers_decode_under_the_convolutional_time_interleaver() {
        let cti = GenTi::Convolutional {
            depth: 0,
            extended: false,
        };
        let p = layered(core(), enhanced(), 8, cti);
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        b.frame();
        let plps = &b.detail().subframes[0].plps;
        assert_eq!((plps[1].ti_mode, plps[1].cti.is_none()), (1, true));
        assert!(plps[1].cti_fec_block_start.is_some());
        for (want, snr) in [(0, None), (1, None), (1, Some(24.0))] {
            let (good, all) = run(&p, want, snr, 5, 4);
            assert!(good == all && all >= if want == 0 { 10 } else { 30 }, "PLP {want}: {good} of {all}");
        }
    }

    /// An enhanced PLP 10 dB below two core PLPs that it lies on (clause 7.2.7.4): 16 points 8/15 and 6/15 with the
    /// 16200-bit code, 5 and 4 blocks of 4050 cells; the enhanced FEC blocks of 10 800 cells cross from the first core
    /// PLP to the second and, without time interleaving, from subframe to subframe. With the hybrid time interleaver
    /// within the subframe the second core PLP has 3 blocks and the enhanced PLP 3 whole blocks. Every block of 3 frames
    /// of each PLP decodes.
    #[test]
    fn an_enhanced_plp_decodes_across_two_core_plps() {
        for (ti, second_blocks) in [
            (GenTi::None, 4),
            (
                GenTi::Hybrid {
                    inter_subframe: false,
                    ti_blocks: 1,
                    cell_interleaver: true,
                    spare: 0,
                },
                3,
            ),
        ] {
            let first = 5 * 4050;
            let second = |start| GenPlp {
                coding: coding(16200, 6, 4),
                seed: 31,
                start,
                cells: Some(second_blocks * 4050),
                subslices: 1,
                interval: 0,
                time_interleaver: ti,
                lls: false,
                ldm: None,
            };
            let mut p = layered(coding(16200, 8, 4), enhanced(), 15, ti);
            p.plp_cells = Some(first);
            p.more_plps[0].ldm = Some(GenLdm { core: 0, cores: 2, level: 15 });
            p.more_plps.insert(0, second(first));
            let mut b = FrameBuilder::new(p.clone()).unwrap();
            b.frame();
            let plps = &b.detail().subframes[0].plps;
            let total = first + second_blocks * 4050;
            assert_eq!((plps[2].layer, plps[2].start, plps[2].size as usize), (1, 0, total));
            assert_eq!(run(&p, 0, None, 1, 3), (15, 15), "{ti:?}");
            let n = 3 * second_blocks;
            assert_eq!(run(&p, 1, None, 1, 3), (n, n), "{ti:?}");
            let n = 3 * total / 10_800;
            assert_eq!(run(&p, 2, None, 1, 3), (n, n), "{ti:?}");
        }
        // Not under a core PLP with another time interleaver.
        let mut p = layered(coding(16200, 8, 4), enhanced(), 15, GenTi::None);
        p.plp_cells = Some(5 * 4050);
        p.more_plps[0].ldm = Some(GenLdm { core: 0, cores: 2, level: 15 });
        p.more_plps.insert(
            0,
            GenPlp {
                coding: coding(16200, 6, 4),
                seed: 31,
                start: 5 * 4050,
                cells: Some(4 * 4050),
                subslices: 1,
                interval: 0,
                time_interleaver: GenTi::Hybrid {
                    inter_subframe: false,
                    ti_blocks: 1,
                    cell_interleaver: false,
                    spare: 0,
                },
                lls: false,
                ldm: None,
            },
        );
        assert!(FrameBuilder::new(p).is_err());
    }

    /// The core PLP with the enhanced layer as noise at injection levels of 3, 6, 10 and 15 dB, in white noise 0.5 dB above
    /// the A/327 clause 4.2.8.1 estimate `SNR + 10 log(1 + 10^(-IL/10)) - 10 log(1 - 10^((SNR - IL)/10))` from the -1.5 dB
    /// at which the core PLP alone decodes (it does not at -2 dB). Every block of 3 frames decodes. At 3 dB the core PLP
    /// does not decode at 0 dB, where it would alone.
    #[test]
    fn the_core_plp_decodes_under_the_enhanced_layer() {
        let mut p = layered(core(), enhanced(), 8, GenTi::None);
        // L1-Basic mode 1 and L1-Detail mode 1 read at -2 dB.
        p.fields.preamble = 0;
        p.l1_detail_mode = 1;
        let mut alone = p.clone();
        alone.more_plps.clear();
        assert_eq!(run(&alone, 0, Some(-1.5), 1, 3), (15, 15));
        assert_eq!(run(&alone, 0, Some(-2.0), 1, 3).0, 0);
        let alone_db = -1.5f64;
        for (level, il) in [(6u8, 3.0f64), (11, 6.0), (15, 10.0), (20, 15.0)] {
            p.more_plps[0].ldm = Some(GenLdm { core: 0, cores: 1, level });
            let need = alone_db + 10.0 * (1.0 + 10f64.powf(-il / 10.0)).log10()
                - 10.0 * (1.0 - 10f64.powf((alone_db - il) / 10.0)).log10();
            assert_eq!(run(&p, 0, Some(need + 0.5), 2, 3), (15, 15), "{il} dB at {need:.2} dB");
        }
        p.more_plps[0].ldm = Some(GenLdm { core: 0, cores: 1, level: 6 });
        assert_eq!(run(&p, 0, Some(0.0), 1, 3).0, 0);
    }

    /// The enhanced PLP 4 dB below the core PLP, after the core PLP is taken off: every block of 3 frames decodes at 22 and
    /// 30 dB (A/327 clause 4.2.8.1: 5.5 dB more than the enhanced PLP needs alone).
    #[test]
    fn the_enhanced_plp_decodes_after_the_core_plp_is_taken_off() {
        let p = layered(core(), enhanced(), 8, GenTi::None);
        for snr in [22.0, 30.0] {
            assert_eq!(run(&p, 1, Some(snr), 3, 3), (45, 45), "{snr} dB");
        }
    }

    /// The modulation error ratio of the enhanced PLP is that of its cells after the core PLP is taken off: within 1 dB of
    /// the signal to noise ratio less the 5.46 dB by which table 6.15 puts the enhanced layer below the total at 4 dB.
    /// Against the core constellation it would stay near the 4 dB of the injection level.
    #[test]
    fn the_enhanced_mer_is_measured_after_the_core_plp_is_taken_off() {
        let p = layered(core(), enhanced(), 8, GenTi::None);
        for snr in [24.0, 30.0] {
            let (good, all, mer) = run_mer(&p, 1, Some(snr), 4, 2);
            let mer = f64::from(mer.unwrap());
            assert_eq!(good, all);
            assert!((mer - (snr - 5.46)).abs() < 1.0, "{mer:.2} dB at {snr} dB");
        }
    }
}
