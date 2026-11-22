//! The enhanced layer of ATSC 3.0: the core PLP under an enhanced PLP is decoded, encoded and mapped again, and taken off
//! the cells after time de-interleaving; the enhanced PLP is decoded from what is left.
//!
//! Standard: ATSC A/322:2026-06 clauses 6.4 and 7.2.7.4; A/327:2021 clause 6.4.

use super::bicm::{BlockResult, PlpCoding, PlpDecoder, PlpEncoder, PlpStream};
use super::rx::Layers;
use sdrkit_core::Complex32;

/// The tag of a core cell with no enhanced cell on it.
const NONE: u64 = u64::MAX;

/// The decoder of an enhanced PLP from the cells of its core PLP. After the first blocks it allocates nothing.
pub struct LayerDecoder {
    core: PlpDecoder,
    encoder: PlpEncoder,
    enhanced: PlpStream,
    /// The cells after the last whole core FEC block, from the first cell of a core block: the cells `W = z / beta`,
    /// their weights `beta^2 |H|^2 / sigma^2`, and the number of the enhanced cell on each among all enhanced cells taken
    /// (`NONE` for none).
    cells: Vec<Complex32>,
    weights: Vec<f32>,
    tags: Vec<u64>,
    /// `cells` begins at the first cell of a core FEC block.
    synced: bool,
    /// The enhanced cells taken, and the number of the first cell of some enhanced FEC block.
    seen: u64,
    grid: Option<u64>,
    /// The number of the enhanced cell that `enhanced` takes next.
    given: u64,
    core_weights: Vec<f32>,
    regenerated: Vec<Complex32>,
    left: (Vec<Complex32>, Vec<f32>),
    packet: Vec<u8>,
}

impl LayerDecoder {
    /// The decoder of an enhanced PLP coded as `enhanced` on a core PLP coded as `core`. `None` when a part of a coding does
    /// not exist.
    pub fn new(core: PlpCoding, enhanced: PlpCoding) -> Option<LayerDecoder> {
        Some(LayerDecoder {
            core: PlpDecoder::new(core)?,
            encoder: PlpEncoder::new(core)?,
            enhanced: PlpStream::new(PlpDecoder::new(enhanced)?),
            cells: Vec::new(),
            weights: Vec::new(),
            tags: Vec::new(),
            synced: false,
            seen: 0,
            grid: None,
            given: 0,
            core_weights: Vec::new(),
            regenerated: Vec::new(),
            left: (Vec::new(), Vec::new()),
            packet: Vec::new(),
        })
    }

    /// The codings of the core and of the enhanced PLP.
    pub fn codings(&self) -> (PlpCoding, PlpCoding) {
        (*self.core.coding(), *self.enhanced.coding())
    }

    /// Forgets the cells of the frames before.
    pub fn restart(&mut self) {
        self.cells.clear();
        self.weights.clear();
        self.tags.clear();
        self.synced = false;
        self.grid = None;
        self.enhanced.restart();
    }

    /// Takes the cells of the core PLP in one subframe after time de-interleaving: `cells` are `z / beta` and `weights`
    /// `beta^2 |H|^2 / sigma^2` (as [`super::rx::FrameDecoder::part`] gives them for an enhanced PLP), `block_start` is
    /// the first cell of the first core FEC block that begins in them, and `layers` holds the enhanced PLP. Each core block
    /// is decoded with the enhanced layer as noise, encoded and mapped again and taken off the cells under the enhanced
    /// PLP: `(W - S_core) / alpha` with the weight times `alpha^2` (A/327 clause 6.4.3, on cells that the channel is
    /// divided out of). `each` gets the result and the baseband packet of every enhanced block that ends.
    pub fn frame(
        &mut self,
        cells: &[Complex32],
        weights: &[f32],
        block_start: Option<usize>,
        layers: &Layers,
        max_iter: u32,
        mut each: impl FnMut(BlockResult, &[u8]),
    ) {
        let Some(e) = layers.enhanced else {
            return;
        };
        let (from, count) = e.span;
        if cells.len() != weights.len() || from + count > cells.len() {
            self.restart();
            return;
        }
        // The enhanced cells of this subframe and the first enhanced FEC block among them.
        let base = self.seen;
        self.seen += count as u64;
        let block_e = self.enhanced.coding().cells() as u64;
        if let Some(s) = e.fec_block_start {
            let at = base + s as u64;
            if self.grid.is_some_and(|g| at.abs_diff(g) % block_e != 0) {
                self.enhanced.restart();
            }
            self.grid = Some(at);
        }
        let tag = |i: usize| {
            if (from..from + count).contains(&i) {
                base + (i - from) as u64
            } else {
                NONE
            }
        };
        // The core cells from the first cell of a core FEC block.
        let block = self.core.coding().cells();
        let skip = match (self.synced, block_start) {
            (true, Some(s)) if !(self.cells.len() + s).is_multiple_of(block) => {
                self.cells.clear();
                self.weights.clear();
                self.tags.clear();
                s
            }
            (true, _) => 0,
            (false, Some(s)) => s,
            (false, None) => cells.len(),
        };
        let skip = skip.min(cells.len());
        if !self.synced {
            self.synced = skip < cells.len();
        }
        self.cells.extend_from_slice(&cells[skip..]);
        self.weights.extend_from_slice(&weights[skip..]);
        self.tags.extend((skip..cells.len()).map(tag));
        // Each whole core block, taken off the enhanced cells on it.
        let alpha = layers.alpha;
        let (alpha2, gain) = (alpha * alpha, 1.0 / alpha);
        let k = self.core.coding().outer_bits();
        let mut at = 0;
        while self.synced && at + block <= self.cells.len() {
            let r = at..at + block;
            self.core_weights.clear();
            self.core_weights.extend(
                self.weights[r.clone()]
                    .iter()
                    .map(|&v| v / (1.0 + v * alpha2)),
            );
            let result = self.core.decode(
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
                    .outer_word_cells(&self.core.word()[..k], &mut self.regenerated)
            } else {
                self.encoder
                    .code_word_cells(self.core.word(), &mut self.regenerated)
            };
            if made.is_err() {
                self.restart();
                return;
            }
            self.left.0.clear();
            self.left.1.clear();
            let mut first = NONE;
            for (j, i) in r.enumerate() {
                let t = self.tags[i];
                if t == NONE {
                    continue;
                }
                if first != NONE && t != first + self.left.0.len() as u64 {
                    self.give(first, max_iter, &mut each);
                    first = NONE;
                }
                if first == NONE {
                    first = t;
                }
                self.left
                    .0
                    .push((self.cells[i] - self.regenerated[j]) * gain);
                self.left.1.push(self.weights[i] * alpha2);
            }
            if first != NONE {
                self.give(first, max_iter, &mut each);
            }
            at += block;
        }
        self.cells.drain(..at);
        self.weights.drain(..at);
        self.tags.drain(..at);
    }

    /// Gives the enhanced cells in `self.left`, the first of number `first`, to the enhanced decoder, and empties it.
    fn give(&mut self, first: u64, max_iter: u32, each: &mut impl FnMut(BlockResult, &[u8])) {
        let n = self.left.0.len() as u64;
        if first != self.given {
            self.enhanced.restart();
        }
        self.given = first + n;
        let block = self.enhanced.coding().cells() as u64;
        let start = self.grid.map(|g| {
            if first >= g {
                (block - (first - g) % block) % block
            } else {
                (g - first) % block
            }
        });
        match start {
            Some(s) => self.enhanced.frame(
                &self.left.0,
                &self.left.1,
                (s < n).then_some(s as usize),
                max_iter,
                each,
            ),
            None => self.enhanced.restart(),
        }
        self.left.0.clear();
        self.left.1.clear();
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
                ldm: Some(GenLdm { core: 0, level }),
            }],
            ..Atsc3GenParams::default()
        }
    }

    /// Decodes PLP `want` of `frames` frames of `p`, with white noise at `snr_db` (unit signal power in 6.144 MHz) when it is
    /// given. Returns the good FEC blocks and all blocks; every good baseband packet must be one of the generator, in order.
    fn run(p: &Atsc3GenParams, want: u8, snr_db: Option<f64>, seed: u64, frames: usize) -> (usize, usize) {
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
        let (mut at, mut good, mut all, mut follows) = (0, 0, 0, false);
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
                let d = ti
                    .frame(&plp.ti, follows, plp.coding.cells(), &cells.cells, &cells.weights, plp.fec_block_start)
                    .unwrap();
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
                        let e = l.enhanced.unwrap().coding;
                        let dec = layers.get_or_insert_with(|| LayerDecoder::new(plp.coding, e).unwrap());
                        if d.restart {
                            dec.restart();
                        }
                        dec.frame(d.cells, d.weights, d.block_start, &l, 50, &mut each);
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
        (good, all)
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
                        Some(l) if l.enhanced.is_some() => layers
                            .get_or_insert_with(|| LayerDecoder::new(plp.coding, l.enhanced.unwrap().coding).unwrap())
                            .frame(&c.cells, &c.weights, plp.fec_block_start, &l, 50, each),
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
        // Not across subframes: there the core PLP of a subframe holds the cells of TI blocks that began in other
        // subframes, and its size is not the cells before time interleaving that an enhanced PLP counts in.
        p.time_interleaver = GenTi::Hybrid {
            inter_subframe: true,
            ti_blocks: 2,
            cell_interleaver: true,
            spare: 0,
        };
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
            p.more_plps[0].ldm = Some(GenLdm { core: 0, level });
            let need = alone_db + 10.0 * (1.0 + 10f64.powf(-il / 10.0)).log10()
                - 10.0 * (1.0 - 10f64.powf((alone_db - il) / 10.0)).log10();
            assert_eq!(run(&p, 0, Some(need + 0.5), 2, 3), (15, 15), "{il} dB at {need:.2} dB");
        }
        p.more_plps[0].ldm = Some(GenLdm { core: 0, level: 6 });
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
}
