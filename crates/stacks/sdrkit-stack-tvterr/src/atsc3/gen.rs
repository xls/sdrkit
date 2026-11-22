//! The ATSC 3.0 test signal: frames of a bootstrap, the Preamble symbols with L1-Basic and L1-Detail, and one or more
//! subframes, each with its own waveform, with one or more PLPs that carry baseband packets of UDP datagrams with a known payload, each with or without a time
//! interleaver, and enhanced PLPs layered on core PLPs.
//!
//! Standard: ATSC A/321, A/322:2023-03 clauses 5.2, 6, 7.1, 7.2, 7.3 and 8, A/322:2026-06 clauses 6.4 and 7.2.7.4, A/330
//! clause 5.

use super::bicm::{constellation, Outer, PlpCoding, PlpEncoder};
use super::bootstrap::{self, Fields};
use super::frame::{
    boundary_null_cells, first_subframe, preamble_symbols, subframe_symbols, Symbol,
};
use super::l1::{
    detail_segments, DetailSubframe, FrameLength, Hti, L1Basic, L1Detail, Plp, Scrambler,
    SubframeParams,
};
use super::ti::{cell_sequence, cell_shifts};
use super::layout::{guard_code, interleave, FftSize, Pattern, PreambleStructure};
use super::tables::LDM_FACTORS;
use super::OCCUPIED;
use crate::band::resampler;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_fec::a322::{L1Code, L1Kind};
use sdrkit_algo_fec::interleave::DelayLanes;
use sdrkit_algo_modem::constellation::Constellation;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_ipcast::alp::{write_bbp_header, write_single, BbpExtension, PacketType};
use sdrkit_proto_ipcast::ip::write_ipv4_udp;
use std::collections::VecDeque;
use std::net::Ipv4Addr;

/// The time interleaver of a PLP of the test signal (A/322 clause 7.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GenTi {
    /// None.
    #[default]
    None,
    /// The convolutional time interleaver at depth code `depth`, 0 to 3: 512, 724, 887 and 1024 rows, or with `extended`
    /// interleaving (QPSK only) 512, 724, 1254 and 1448 rows.
    Convolutional {
        /// `L1D_plp_CTI_depth`.
        depth: u8,
        /// `L1D_plp_TI_extended_interleaving`.
        extended: bool,
    },
    /// The hybrid time interleaver. The PLP carries whole FEC blocks only.
    Hybrid {
        /// Each TI block is spread over `ti_blocks` subframes. Without it the subframe holds `ti_blocks` TI blocks.
        inter_subframe: bool,
        /// The TI blocks of the subframe, or the subframes of a TI block, 1 to 16.
        ti_blocks: u8,
        /// The cells of each FEC block are interleaved.
        cell_interleaver: bool,
        /// The FEC blocks that the signalled maximum holds more than the subframe carries.
        spare: u16,
    },
}

/// The layer of an enhanced PLP of the test signal (A/322 clause 6.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenLdm {
    /// The core PLP that it is combined with: 0 for the first, `n` for `more_plps[n - 1]`. It must be in every subframe
    /// that carries this PLP, non-dispersed, without the convolutional time interleaver and without the hybrid one
    /// across subframes.
    pub core: usize,
    /// `L1D_plp_ldm_injection_level`, 0 to 30 (table 9.24).
    pub level: u8,
}

/// One PLP of the test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenPlp {
    /// Its coding.
    pub coding: PlpCoding,
    /// The seed of its payload.
    pub seed: u64,
    /// Its first data cell.
    pub start: usize,
    /// Its data cells in the first subframe. `None`: all from `start` to the end of the subframe. 0: the first subframe
    /// does not carry it.
    pub cells: Option<usize>,
    /// Its subslices: 1 for a non-dispersed PLP.
    pub subslices: u16,
    /// The cells from the start of one subslice to the start of the next: more than the cells of a subslice.
    pub interval: u32,
    /// Its time interleaver.
    pub time_interleaver: GenTi,
    /// It is marked as a carrier of low level signalling.
    pub lls: bool,
    /// It is an enhanced PLP on this layer. Its place, subslices and time interleaver are then those of the core PLP: it
    /// takes the cells of the core PLP before time interleaving from the first, all of them, or with the hybrid time
    /// interleaver as many whole FEC blocks as fit, with dummy cells after them.
    pub ldm: Option<GenLdm>,
}

/// The place of a PLP in a subframe after the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenPlace {
    /// The PLP: 0 for the first, `n` for `more_plps[n - 1]`.
    pub plp: usize,
    /// Its first data cell in the subframe.
    pub start: usize,
    /// Its data cells in the subframe. `None`: all from `start` to the end of the subframe.
    pub cells: Option<usize>,
    /// Its subslices: 1 for a non-dispersed PLP.
    pub subslices: u16,
    /// The cells from the start of one subslice to the start of the next.
    pub interval: u32,
}

/// A subframe of the test signal after the first.
#[derive(Clone, Debug, PartialEq)]
pub struct GenSubframe {
    /// Its FFT size.
    pub fft: FftSize,
    /// Its guard interval, samples.
    pub guard: usize,
    /// Its data and subframe boundary symbols.
    pub symbols: usize,
    /// Its carrier reduction, 0 to 4.
    pub reduced_carriers: u8,
    /// Its scattered pilot pattern `(D_X, D_Y)`.
    pub pilots: (usize, usize),
    /// Its scattered pilot boost code, 0 to 4.
    pub pilot_boost: u8,
    /// Its data cells are frequency interleaved.
    pub frequency_interleaver: bool,
    /// Its first and its last symbol are subframe boundary symbols.
    pub boundary: (bool, bool),
    /// The PLPs that it carries.
    pub plps: Vec<GenPlace>,
}

/// Generator options.
#[derive(Clone, Debug, PartialEq)]
pub struct Atsc3GenParams {
    /// Output rate (6.144 Msps or more).
    pub rate_hz: Hz,
    /// Bootstrap fields. The preamble structure sets the FFT size, the guard interval, the Preamble pilots and the L1-Basic
    /// mode; the frame runs at 6.144 Msps, so the sample-rate coefficient is 0.
    pub fields: Fields,
    /// The data and subframe boundary symbols of the subframe.
    pub symbols: usize,
    /// The carrier reduction of the subframe, 0 to 4.
    pub reduced_carriers: u8,
    /// The scattered pilot pattern `(D_X, D_Y)`.
    pub pilots: (usize, usize),
    /// The scattered pilot boost code, 0 to 4.
    pub pilot_boost: u8,
    /// The data cells of the subframe are frequency interleaved.
    pub frequency_interleaver: bool,
    /// The coding of the PLP.
    pub coding: PlpCoding,
    /// The L1-Detail mode, 1 to 7.
    pub l1_detail_mode: u8,
    /// Reserved bytes at the end of L1-Detail, before its CRC.
    pub l1_detail_reserved: usize,
    /// The frame is time-aligned: its length is a whole number of 5 ms, the samples to that length go into the guards of the
    /// symbols of the subframe and after the last symbol (A/322 clause 8.5.1). Otherwise it is symbol-aligned.
    pub time_aligned: bool,
    /// The first data cell of the PLP. The cells of no PLP carry the dummy values of A/322 clause 7.2.6.5.
    pub plp_start: usize,
    /// The data cells of the PLP in the subframe. `None`: all from `plp_start` to the end of the subframe.
    pub plp_cells: Option<usize>,
    /// The subslices of the PLP (1 for a non-dispersed PLP) and the cells from the start of one to the start of the next.
    pub subslices: (u16, u32),
    /// The time interleaver of the PLP.
    pub time_interleaver: GenTi,
    /// The PLPs after the first, with the ids 1, 2 and so on.
    pub more_plps: Vec<GenPlp>,
    /// The subframes after the first.
    pub more_subframes: Vec<GenSubframe>,
    /// Seed of the payload.
    pub seed: u64,
}

impl Default for Atsc3GenParams {
    fn default() -> Self {
        Atsc3GenParams {
            rate_hz: 8e6,
            fields: Fields {
                minor: 0,
                ea_wake: false,
                min_time: 2,
                bandwidth: 0,
                rate_n: 0,
                preamble: 3,
            },
            symbols: 72,
            reduced_carriers: 0,
            pilots: (16, 2),
            pilot_boost: 1,
            frequency_interleaver: true,
            coding: PlpCoding {
                ldpc_bits: 64800,
                rate: 8,
                bits: 2,
                outer: Outer::Bch,
            },
            l1_detail_mode: 2,
            l1_detail_reserved: 0,
            time_aligned: false,
            plp_start: 0,
            plp_cells: None,
            subslices: (1, 0),
            time_interleaver: GenTi::None,
            more_plps: Vec::new(),
            more_subframes: Vec::new(),
            seed: 1,
        }
    }
}

/// Why the generator refuses its options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenError {
    /// The preamble structure is reserved, or the sample-rate coefficient is not 0.
    #[error("the bootstrap fields are not ones the test signal has")]
    Bootstrap,
    /// The pilot pattern, the boost, the symbol count or the carrier reduction is not one of the standard, or the PLPs do not
    /// fit the subframe.
    #[error("the subframe options are not ones of the standard")]
    Subframe,
    /// The code, the modulation, the time interleaver or the L1-Detail mode is not one of the standard.
    #[error("the coding options are not ones of the standard")]
    Coding,
    /// No resampler serves the output rate.
    #[error("no resampler serves the output rate")]
    Rate,
}

/// The destination of the test datagrams.
pub const PAYLOAD_ADDR: [u8; 4] = [239, 255, 10, 1];
/// Their UDP port.
pub const PAYLOAD_PORT: u16 = 5000;
/// The bytes of one test datagram's payload: a counter of 8 bytes and 1192 bytes that follow from the seed and the counter.
pub const PAYLOAD_BYTES: usize = 1200;

/// The payload of the PLP: UDP datagrams in ALP packets, cut into baseband packets.
#[derive(Clone, Debug)]
pub struct Payload {
    seed: u64,
    datagrams: u64,
    alp: VecDeque<u8>,
    /// Bytes of `alp` before the next ALP packet starts.
    to_start: usize,
    scratch: Vec<u8>,
}

impl Payload {
    /// The payload of `seed`.
    pub fn new(seed: u64) -> Payload {
        Payload {
            seed,
            datagrams: 0,
            alp: VecDeque::new(),
            to_start: 0,
            scratch: Vec::new(),
        }
    }

    /// The payload bytes of datagram `counter` of `seed`.
    pub fn datagram(seed: u64, counter: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(PAYLOAD_BYTES);
        out.extend_from_slice(&counter.to_be_bytes());
        let mut x = seed ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
        while out.len() < PAYLOAD_BYTES {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            out.push((x >> 32) as u8);
        }
        out
    }

    fn more(&mut self) {
        let payload = Payload::datagram(self.seed, self.datagrams);
        let mut ip = Vec::new();
        let mut ranges = Vec::new();
        write_ipv4_udp(
            &mut ip,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::from(PAYLOAD_ADDR),
            self.datagrams as u16,
            1500,
            PAYLOAD_PORT,
            PAYLOAD_PORT,
            &payload,
            |r| ranges.push(r),
        )
        .expect("a datagram below the MTU");
        self.datagrams += 1;
        self.scratch.clear();
        for r in ranges {
            write_single(PacketType::Ipv4, None, &ip[r], &mut self.scratch)
                .expect("a packet below the ALP limit");
        }
        self.alp.extend(self.scratch.iter());
    }

    /// The next baseband packet of `len` bytes (A/322 clause 5.2.2): a base field of one or two bytes with the pointer to the
    /// first ALP packet that starts in it, then ALP bytes.
    pub fn packet(&mut self, len: usize) -> Vec<u8> {
        // A pointer below 128 has a base field of one byte.
        let (pointer, room) = if self.to_start < 128 && self.to_start < len - 1 {
            (self.to_start as u16, len - 1)
        } else if self.to_start < len - 2 {
            (self.to_start as u16, len - 2)
        } else {
            (8191, len - 2)
        };
        let mut bb = Vec::with_capacity(len);
        write_bbp_header(pointer, BbpExtension::None, &mut bb).expect("a pointer of 13 bits");
        while self.alp.len() < room {
            self.more();
        }
        bb.extend(self.alp.drain(..room));
        // Every ALP packet of the payload has one length, so the next start follows from it.
        let unit = self.scratch.len();
        self.to_start = if self.to_start >= room {
            self.to_start - room
        } else {
            (unit - (room - self.to_start) % unit) % unit
        };
        bb
    }
}

/// The convolutional time interleaver of the sender (A/322 clause 7.1.4): row `r` of `rows` holds `r` cells. Each cell
/// enters the next row in turn, and the cell that the row held longest leaves it. Row 0 passes its cell at once. The rows
/// start filled with zeros.
pub fn convolutional_interleaver(rows: usize) -> DelayLanes<Complex32> {
    DelayLanes::forney(rows as u32, 1, false, Complex32::default())
}

/// One TI block of the hybrid time interleaver (A/322 clauses 7.1.5.2 and 7.1.5.4). `cells` holds whole FEC blocks of
/// `block` cells and `columns` is the most FEC blocks of a TI block. Each FEC block is read in the order of the cell
/// interleaver (with `cell_interleaver`) and written down one column of the memory, after the columns of the virtual FEC
/// blocks. Output `i` is read from row `R = i mod block` and column `(R mod columns + i div block) mod columns`, without
/// the cells of the virtual blocks.
pub fn twisted_block(
    cells: &[Complex32],
    block: usize,
    columns: usize,
    cell_interleaver: bool,
) -> Vec<Complex32> {
    let blocks = cells.len() / block;
    let empty = columns - blocks;
    let sequence = if cell_interleaver {
        cell_sequence(block).expect("a FEC block of the standard")
    } else {
        Vec::new()
    };
    let mut shifts = Vec::new();
    cell_shifts(block, blocks, &mut shifts);
    let mut memory: Vec<Option<Complex32>> = vec![None; columns * block];
    for (r, fec) in cells.chunks(block).enumerate() {
        let column = &mut memory[(empty + r) * block..][..block];
        for (q, d) in column.iter_mut().enumerate() {
            let from = if cell_interleaver {
                (sequence[q] as usize + shifts[r] as usize) % block
            } else {
                q
            };
            *d = Some(fec[from]);
        }
    }
    (0..columns * block)
        .filter_map(|i| {
            let row = i % block;
            let column = (row % columns + i / block) % columns;
            memory[block * column + row]
        })
        .collect()
}

/// The convolutional delay line of the hybrid time interleaver across subframes (A/322 clauses 7.1.5.5 to 7.1.5.7). The
/// output of the twisted block interleaver for one TI block is cut into `rows` groups, each filled with virtual cells to
/// `columns` places. Group `g` enters branch `g mod units`, whose registers hold what the branch takes in as many
/// subframes as its number, and the places that leave the branch are sent without the virtual cells.
pub struct DelayLine {
    branches: Vec<VecDeque<Option<Complex32>>>,
    columns: usize,
}

impl DelayLine {
    /// The start cells of a line of `units` branches for FEC blocks of `rows` cells.
    pub fn start_cells(units: usize, rows: usize) -> usize {
        (0..units)
            .map(|i| i * (rows / units + usize::from(i < rows % units)))
            .sum()
    }

    /// The line with its registers filled: each of `start`, branch 1 first, takes the first place of one group.
    pub fn new(units: usize, rows: usize, columns: usize, start: &[Complex32]) -> DelayLine {
        let mut start = start.iter().copied();
        let branches = (0..units.max(1))
            .map(|i| {
                let groups = i * (rows / units + usize::from(i < rows % units));
                let mut branch = VecDeque::with_capacity((groups + 1) * columns);
                for _ in 0..groups {
                    branch.push_back(Some(start.next().unwrap_or_default()));
                    branch.extend(std::iter::repeat_n(None, columns - 1));
                }
                branch
            })
            .collect();
        DelayLine { branches, columns }
    }

    /// The cells of one subframe: `cells` is the TI block that begins in it, of `blocks` FEC blocks.
    pub fn subframe(&mut self, cells: &[Complex32], blocks: usize) -> Vec<Complex32> {
        let units = self.branches.len();
        let mut out = Vec::with_capacity(cells.len());
        for (g, group) in cells.chunks(blocks).enumerate() {
            let branch = &mut self.branches[g % units];
            branch.extend(group.iter().map(|&c| Some(c)));
            branch.extend(std::iter::repeat_n(None, self.columns - blocks));
            out.extend(branch.drain(..self.columns).flatten());
        }
        out
    }
}

/// One PLP of the test signal: its coder, its payload, its time interleaver and its place in the subframe.
struct PlpSender {
    coding: PlpCoding,
    enc: PlpEncoder,
    payload: Payload,
    /// The cells of the FEC blocks made and not yet sent.
    cells: VecDeque<Complex32>,
    ti: GenTi,
    lines: Option<DelayLanes<Complex32>>,
    /// The delay line across subframes, made with the first subframe.
    line: Option<DelayLine>,
    /// The FEC blocks of the TI blocks that began in the last subframes, the newest first.
    history: VecDeque<u16>,
    /// The most FEC blocks of a subframe that L1-Detail signals, with the hybrid time interleaver.
    most: usize,
    /// The FEC blocks of every TI block across subframes: those of the smallest subframe that carries the PLP.
    fewest: usize,
}

/// The place of a PLP in one subframe.
#[derive(Clone, Copy, Debug)]
struct Place {
    /// The PLP: its place among the senders and among the L1-Detail entries of the subframe.
    sender: usize,
    entry: usize,
    start: usize,
    /// Its cells in a subframe once the time interleaver is full.
    size: usize,
    subslices: usize,
    interval: usize,
    /// For an enhanced PLP: the place of its core PLP in the list.
    on: Option<usize>,
}

/// The places of the PLPs `wants` (`plp`, `start`, `cells`, `subslices`, `interval`) in a subframe of `total` data cells:
/// each PLP in its subslices, within the subframe, and no cell in two PLPs. A hybrid time interleaved PLP takes whole
/// FEC blocks. An enhanced PLP lies on the cells of its core PLP, which the subframe carries without subslices, and a core
/// PLP carries one enhanced PLP at most.
fn places(wants: &[GenPlace], options: &[GenPlp], total: usize) -> Result<Vec<Place>, GenError> {
    let mut runs = Vec::new();
    let mut out = Vec::with_capacity(wants.len());
    for (entry, w) in wants.iter().enumerate() {
        let g = options.get(w.plp).ok_or(GenError::Subframe)?;
        if g.ldm.is_some() {
            out.push(Place {
                sender: w.plp,
                entry,
                start: 0,
                size: 0,
                subslices: 1,
                interval: 0,
                on: None,
            });
            continue;
        }
        let (n, interval) = (usize::from(w.subslices), w.interval as usize);
        let mut size = match w.cells {
            Some(cells) => cells,
            None => total.checked_sub(w.start).ok_or(GenError::Subframe)?,
        };
        if matches!(g.time_interleaver, GenTi::Hybrid { .. }) {
            size -= size % g.coding.cells();
        }
        // Clause 7.2.6: a subslice holds `ceil(size / n)` cells, the last one what is left, and the next begins later.
        let each = size.div_ceil(n.max(1));
        if size == 0 || n == 0 || (n > 1 && (interval <= each || size <= (n - 1) * each)) {
            return Err(GenError::Subframe);
        }
        runs.extend((0..n).map(|j| (w.start + j * interval, w.start + j * interval + each)));
        out.push(Place {
            sender: w.plp,
            entry,
            start: w.start,
            size,
            subslices: n,
            interval,
            on: None,
        });
    }
    for i in 0..out.len() {
        let Some(ldm) = options[out[i].sender].ldm else {
            continue;
        };
        let core = out
            .iter()
            .position(|c| c.sender == ldm.core && options[c.sender].ldm.is_none())
            .filter(|&c| out[c].subslices == 1 && !out.iter().any(|e| e.on == Some(c)))
            .ok_or(GenError::Subframe)?;
        out[i] = Place {
            start: out[core].start,
            size: out[core].size,
            on: Some(core),
            ..out[i]
        };
    }
    runs.sort_unstable();
    if runs.windows(2).any(|w| w[0].1 > w[1].0) || runs.last().is_some_and(|r| r.1 > total) {
        return Err(GenError::Subframe);
    }
    Ok(out)
}

/// The rows of the convolutional time interleaver at each depth code, without and with extended interleaving (A/322
/// table 9.26).
const ROWS: [[usize; 4]; 2] = [[512, 724, 887, 1024], [512, 724, 1254, 1448]];

impl PlpSender {
    fn new(g: &GenPlp) -> Result<PlpSender, GenError> {
        Ok(PlpSender {
            coding: g.coding,
            enc: PlpEncoder::new(g.coding).ok_or(GenError::Coding)?,
            payload: Payload::new(g.seed),
            cells: VecDeque::new(),
            ti: g.time_interleaver,
            lines: match g.time_interleaver {
                GenTi::Convolutional { depth, extended } => Some(convolutional_interleaver(
                    *ROWS[usize::from(extended)]
                        .get(usize::from(depth))
                        .ok_or(GenError::Coding)?,
                )),
                _ => None,
            },
            line: None,
            history: VecDeque::new(),
            most: 0,
            fewest: usize::MAX,
        })
    }

    /// The L1-Detail entry of the PLP `id` with the options `g`: every field that it sends is there, and the fields of its
    /// place and of its FEC blocks are not yet set.
    fn entry(id: u8, g: &GenPlp) -> Result<Plp, GenError> {
        let (fec_type, modulation, code_rate) = g.coding.codes();
        if g.subslices == 0 {
            return Err(GenError::Subframe);
        }
        let dispersed = g.subslices > 1;
        let qpsk = g.coding.bits == 2;
        if let Some(ldm) = g.ldm {
            // Clauses 7.1.1 and 9.3.4: the time interleaver mode of the core PLP, and no fields of its own after it.
            let ti_mode = match g.time_interleaver {
                GenTi::None => 0,
                GenTi::Hybrid {
                    inter_subframe,
                    ti_blocks,
                    ..
                } if !inter_subframe || ti_blocks == 1 => 2,
                _ => return Err(GenError::Coding),
            };
            if usize::from(ldm.level) >= LDM_FACTORS.len() {
                return Err(GenError::Coding);
            }
            return Ok(Plp {
                id,
                lls: g.lls,
                layer: 1,
                fec_type,
                modulation: Some(modulation),
                code_rate: Some(code_rate),
                ti_mode,
                fec_block_start: (ti_mode == 0).then_some(0),
                ldm_injection_level: Some(ldm.level),
                ..Plp::default()
            });
        }
        let mut plp = Plp {
            id,
            lls: g.lls,
            fec_type,
            modulation: Some(modulation),
            code_rate: Some(code_rate),
            dispersed: Some(dispersed),
            subslices: dispersed.then_some((g.subslices, g.interval)),
            ..Plp::default()
        };
        match g.time_interleaver {
            GenTi::None => plp.fec_block_start = Some(0),
            GenTi::Convolutional { depth, extended } => {
                if depth > 3 || (extended && !qpsk) {
                    return Err(GenError::Coding);
                }
                plp.ti_mode = 1;
                plp.cti_fec_block_start = Some(0);
                plp.cti = Some((depth, 0));
                plp.ti_extended = qpsk.then_some(extended);
            }
            GenTi::Hybrid {
                inter_subframe,
                ti_blocks,
                cell_interleaver,
                ..
            } => {
                if !(1..=16).contains(&ti_blocks) {
                    return Err(GenError::Coding);
                }
                plp.ti_mode = 2;
                plp.hti = Some(Hti {
                    inter_subframe,
                    ti_blocks: ti_blocks - 1,
                    cell_interleaver,
                    ..Hti::default()
                });
                plp.ti_extended = qpsk.then_some(false);
            }
        }
        Ok(plp)
    }

    /// The cells of the next FEC block.
    fn next_block(&mut self) -> Vec<Complex32> {
        let packet = self.payload.packet(self.coding.packet_bytes());
        let mut cells = Vec::with_capacity(self.coding.cells());
        self.enc.block(&packet, &mut cells);
        cells
    }

    /// The next `size` cells of FEC blocks that run on from subframe to subframe, and the cells that were left of the block
    /// that began in the subframe before (clause 9.3.4).
    fn run_on(&mut self, size: usize) -> (Vec<Complex32>, usize) {
        let left = self.cells.len();
        while self.cells.len() < size {
            let cells = self.next_block();
            self.cells.extend(cells);
        }
        debug_assert!(left < self.coding.cells());
        (self.cells.drain(..size).collect(), left)
    }

    /// Combines the cells `core` of a core PLP before time interleaving with the cells of this enhanced PLP into
    /// `beta (core + alpha enhanced)` (clause 6.4, table 6.16). With the hybrid time interleaver the PLP takes whole FEC
    /// blocks and dummy cells fill the rest: the scrambling sequence of clause 5.2.3 from its start, mapped as the PLP
    /// (clause 7.2.7.4.6). Sets the size and the first FEC block of its L1-Detail entry `plp`.
    fn layer(&mut self, plp: &mut Plp, core: &mut [Complex32]) {
        let n = core.len();
        let block = self.coding.cells();
        let (alpha, beta) = LDM_FACTORS[usize::from(plp.ldm_injection_level.unwrap_or(0))];
        let cells = if matches!(self.ti, GenTi::Hybrid { .. }) {
            let blocks = n / block;
            let mut cells = Vec::with_capacity(n);
            for _ in 0..blocks {
                cells.extend(self.next_block());
            }
            plp.size = (blocks * block) as u32;
            let bits = Scrambler::bits(self.coding.bits * (n - blocks * block));
            self.enc
                .points()
                .set()
                .map_bits(&bits, &mut cells)
                .expect("whole cells");
            cells
        } else {
            let (cells, left) = self.run_on(n);
            plp.size = n as u32;
            plp.fec_block_start = Some(if left >= n { 0x7FFF } else { left as u16 });
            cells
        };
        for (c, e) in core.iter_mut().zip(cells) {
            *c = (*c + e * alpha) * beta;
        }
    }

    /// The cells of the PLP in the next subframe that carries it, which gives it `size` cells once the time interleaver
    /// is full. `layer` combines its cells before time interleaving with an enhanced PLP. Sets the fields of its
    /// L1-Detail entry `plp` that change from subframe to subframe.
    fn subframe(
        &mut self,
        plp: &mut Plp,
        size: usize,
        layer: &mut dyn FnMut(&mut [Complex32]),
    ) -> Vec<Complex32> {
        let block = self.coding.cells();
        if let GenTi::Hybrid {
            inter_subframe,
            ti_blocks,
            cell_interleaver,
            ..
        } = self.ti
        {
            let (blocks, n_ti) = (size / block, usize::from(ti_blocks));
            let most = self.most;
            if let Some(h) = &mut plp.hti {
                h.fec_blocks[0] = (blocks - 1) as u16;
            }
            if inter_subframe {
                let blocks = blocks.min(self.fewest);
                // Clauses 7.1.5.5 to 7.1.5.7: one TI block begins in each subframe. The TI blocks before the first count
                // as one FEC block each, and the start cells come from the scrambling sequence.
                let mut cells = Vec::with_capacity(size);
                for _ in 0..blocks {
                    cells.extend(self.next_block());
                }
                layer(&mut cells);
                if self.line.is_none() {
                    let bits = Scrambler::bits(self.coding.bits * DelayLine::start_cells(n_ti, block));
                    let mut start = Vec::new();
                    self.enc
                        .points()
                        .set()
                        .map_bits(&bits, &mut start)
                        .expect("whole cells");
                    self.line = Some(DelayLine::new(n_ti, block, most, &start));
                    self.history = std::iter::repeat_n(1, n_ti).collect();
                }
                self.history.push_front(blocks as u16);
                self.history.truncate(n_ti);
                if let Some(h) = &mut plp.hti {
                    for (field, count) in h.fec_blocks.iter_mut().zip(&self.history) {
                        *field = count - 1;
                    }
                }
                let twisted = twisted_block(&cells, block, most, cell_interleaver);
                return match &mut self.line {
                    Some(line) => line.subframe(&twisted, blocks),
                    None => twisted,
                };
            }
            // Clause 7.1.5.4: the last TI blocks hold one FEC block more when the blocks do not divide evenly.
            let max = most.div_ceil(n_ti);
            let mut cells = Vec::with_capacity(blocks * block);
            for _ in 0..blocks {
                cells.extend(self.next_block());
            }
            layer(&mut cells);
            let mut out = Vec::with_capacity(size);
            let mut at = 0;
            for s in 0..n_ti {
                let n = blocks / n_ti + usize::from(s >= n_ti - blocks % n_ti);
                out.extend(twisted_block(&cells[at..at + n * block], block, max, cell_interleaver));
                at += n * block;
            }
            return out;
        }
        // The cells left of the block that began in the frame before come first (clause 9.3.4).
        let (mut cells, left) = self.run_on(size);
        layer(&mut cells);
        match &mut self.lines {
            Some(lines) => {
                // The first cell of the first FEC block that begins in this subframe enters row `(row + left) mod rows`
                // and leaves that many times `rows` cells later.
                let (rows, row) = (lines.lanes(), lines.lane());
                plp.cti = plp.cti.map(|(depth, _)| (depth, row as u16));
                plp.cti_fec_block_start = Some((left + (row + left) % rows * rows) as u32);
                cells.into_iter().map(|c| lines.push(c)).collect()
            }
            None => {
                plp.fec_block_start = Some(if left >= size { 0x7FFF } else { left as u16 });
                cells
            }
        }
    }
}

/// Builds the frames of the test signal at 6.144 Msps.
pub struct FrameBuilder {
    p: Atsc3GenParams,
    structure: PreambleStructure,
    basic: L1Basic,
    detail: L1Detail,
    preamble: Vec<Symbol>,
    subframe: Vec<Symbol>,
    l1_basic: L1Code,
    l1_detail: L1Code,
    l1_basic_points: Constellation,
    l1_detail_points: Constellation,
    plps: Vec<PlpSender>,
    /// The data cells of the first subframe, with those of the last Preamble symbol after the signalling, and the places
    /// of its PLPs.
    total: usize,
    places: Vec<Place>,
    /// The subframes after the first: their symbols, their data cells and the places of their PLPs.
    later: Vec<(Vec<Symbol>, usize, Vec<Place>)>,
    iffts: [Option<Fft>; 3],
    /// Samples after the last symbol of a time-aligned frame: the start of its useful part again.
    postfix: usize,
    frames: u64,
}

/// The samples of 5 ms at 6.144 Msps.
const FIVE_MS: usize = 30_720;

impl FrameBuilder {
    /// The builder for `p`.
    ///
    /// # Errors
    ///
    /// [`GenError`] when an option is not one of the standard or the PLPs do not fit the subframe.
    pub fn new(p: Atsc3GenParams) -> Result<FrameBuilder, GenError> {
        let structure = PreambleStructure::of(p.fields.preamble).ok_or(GenError::Bootstrap)?;
        if p.fields.rate_n != 0 {
            return Err(GenError::Bootstrap);
        }
        let pattern = Pattern::of(p.pilots.0, p.pilots.1).ok_or(GenError::Subframe)?;
        if pattern.amplitude(p.pilot_boost).is_none()
            || p.reduced_carriers > 4
            || !(4 * pattern.dy()..=2048).contains(&p.symbols)
        {
            return Err(GenError::Subframe);
        }
        let first_plp = GenPlp {
            coding: p.coding,
            seed: p.seed,
            start: p.plp_start,
            cells: p.plp_cells,
            subslices: p.subslices.0,
            interval: p.subslices.1,
            time_interleaver: p.time_interleaver,
            lls: false,
            ldm: None,
        };
        let mut options: Vec<GenPlp> = std::iter::once(first_plp)
            .chain(p.more_plps.iter().copied())
            .collect();
        // An enhanced PLP follows the time interleaver of its core PLP.
        for i in 0..options.len() {
            if let Some(ldm) = options[i].ldm {
                let core = options.get(ldm.core).ok_or(GenError::Subframe)?;
                if core.ldm.is_some() || core.subslices != 1 {
                    return Err(GenError::Subframe);
                }
                options[i].time_interleaver = core.time_interleaver;
            }
        }
        let mut plps = options
            .iter()
            .map(PlpSender::new)
            .collect::<Result<Vec<_>, _>>()?;
        // The PLPs of each subframe, and their L1-Detail entries.
        let first_wants: Vec<GenPlace> = options
            .iter()
            .enumerate()
            .filter(|(_, g)| g.cells != Some(0))
            .map(|(plp, g)| GenPlace {
                plp,
                start: g.start,
                cells: g.cells,
                subslices: g.subslices,
                interval: g.interval,
            })
            .collect();
        let entries_of = |wants: &[GenPlace]| {
            wants
                .iter()
                .map(|w| {
                    let g = options.get(w.plp).ok_or(GenError::Subframe)?;
                    let placed = GenPlp {
                        subslices: w.subslices,
                        interval: w.interval,
                        ..*g
                    };
                    PlpSender::entry(w.plp as u8, &placed)
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let entries = entries_of(&first_wants)?;
        if options.len() > 64 || p.more_subframes.len() > 255 {
            return Err(GenError::Subframe);
        }
        let l1_basic =
            L1Code::new(L1Kind::Basic, structure.l1_basic_mode).map_err(|_| GenError::Bootstrap)?;
        let l1_detail =
            L1Code::new(L1Kind::Detail, p.l1_detail_mode).map_err(|_| GenError::Coding)?;
        let l1_points = |c: &L1Code| {
            constellation(c.bits_per_cell(), c.constellation_rate()).ok_or(GenError::Coding)
        };
        let first = SubframeParams {
            mimo: false,
            miso: 0,
            fft_size: structure.fft.code(),
            reduced_carriers: p.reduced_carriers,
            guard_interval: guard_code(structure.guard).ok_or(GenError::Bootstrap)?,
            symbols: p.symbols as u16,
            pilot_pattern: pattern.code(),
            pilot_boost: p.pilot_boost,
            sbs_first: true,
            sbs_last: true,
        };
        let nulls = boundary_null_cells(&first).map_err(|_| GenError::Subframe)?;
        let mut basic = L1Basic {
            version: 0,
            mimo_pilot_encoding: false,
            lls: entries.iter().any(|p| p.lls),
            time_info: 0,
            return_channel: false,
            papr: 0,
            frame_length: FrameLength::SymbolAligned {
                time_offset: 0,
                additional_samples: 0,
            },
            subframes: 1 + p.more_subframes.len() as u16,
            preamble_symbols: 1,
            preamble_reduced_carriers: 0,
            detail_content_tag: 0,
            detail_bytes: 0,
            detail_mode: p.l1_detail_mode,
            detail_additional_parity: 0,
            detail_cells: 0,
            first,
        };
        let mut detail = L1Detail {
            version: 1,
            bonded_bsids: Vec::new(),
            time: None,
            subframes: vec![DetailSubframe {
                params: None,
                multiplex: None,
                frequency_interleaver: p.frequency_interleaver,
                sbs_null_cells: Some(nulls as u16),
                plps: entries,
            }],
            bsid: 1,
        };
        // Clause 9.3: each later subframe gives its own waveform, and every subframe the multiplex flag.
        for g in &p.more_subframes {
            let pattern = Pattern::of(g.pilots.0, g.pilots.1).ok_or(GenError::Subframe)?;
            if pattern.amplitude(g.pilot_boost).is_none()
                || g.reduced_carriers > 4
                || !(4 * pattern.dy()..=2048).contains(&g.symbols)
            {
                return Err(GenError::Subframe);
            }
            let params = SubframeParams {
                mimo: false,
                miso: 0,
                fft_size: g.fft.code(),
                reduced_carriers: g.reduced_carriers,
                guard_interval: guard_code(g.guard).ok_or(GenError::Subframe)?,
                symbols: g.symbols as u16,
                pilot_pattern: pattern.code(),
                pilot_boost: g.pilot_boost,
                sbs_first: g.boundary.0,
                sbs_last: g.boundary.1,
            };
            let nulls = boundary_null_cells(&params).map_err(|_| GenError::Subframe)?;
            detail.subframes.push(DetailSubframe {
                params: Some(params),
                multiplex: None,
                frequency_interleaver: g.frequency_interleaver,
                sbs_null_cells: (g.boundary.0 || g.boundary.1).then_some(nulls as u16),
                plps: entries_of(&g.plps)?,
            });
        }
        if detail.subframes.len() > 1 {
            detail
                .subframes
                .iter_mut()
                .for_each(|s| s.multiplex = Some(false));
        }
        let detail_bits = detail.pack_with(&basic, p.l1_detail_reserved).len();
        let (blocks, k_sig) = detail_segments(detail_bits, l1_detail.max_k_sig());
        let lengths = l1_detail.lengths(k_sig).map_err(|_| GenError::Coding)?;
        basic.detail_bytes = (detail_bits / 8) as u16;
        basic.detail_cells = (blocks * lengths.sent() / l1_detail.bits_per_cell()) as u32;
        let l1_cells = l1_basic
            .lengths(200)
            .map_err(|_| GenError::Bootstrap)?
            .sent()
            / l1_basic.bits_per_cell()
            + basic.detail_cells as usize;
        // As many Preamble symbols as the signalling needs (clause 7.2.5).
        let room = |n: u8| -> usize {
            preamble_symbols(&structure, n, 0)
                .iter()
                .map(|s| s.active)
                .sum()
        };
        let count = (1..=8u8)
            .find(|&n| room(n) >= l1_cells)
            .ok_or(GenError::Coding)?;
        basic.preamble_symbols = count;
        // Clause 8.5.1: the samples up to a whole number of 5 ms are shared by the symbols of the subframe; the rest follows
        // the last symbol.
        let symbol = structure.fft.size() + structure.guard;
        let natural = 4 * bootstrap::SYM
            + (usize::from(count) + p.symbols) * symbol
            + p.more_subframes
                .iter()
                .map(|g| g.symbols * (g.fft.size() + g.guard))
                .sum::<usize>();
        // Clause 8.5.1: every symbol after the Preamble takes the same excess samples; the rest follows the last symbol.
        let all_symbols = p.symbols + p.more_subframes.iter().map(|g| g.symbols).sum::<usize>();
        let (mut postfix, mut excess) = (0, 0);
        if p.time_aligned {
            let units = natural.div_ceil(FIVE_MS);
            let extra = units * FIVE_MS - natural;
            postfix = extra % all_symbols;
            excess = extra / all_symbols;
            basic.frame_length = FrameLength::TimeAligned {
                units_5ms: units as u16,
                excess_samples: excess as u16,
            };
        }
        let preamble = preamble_symbols(&structure, count, 0);
        let subframe = first_subframe(&basic, &detail.subframes[0], preamble.len())
            .map_err(|_| GenError::Subframe)?;
        let total = room(count) - l1_cells + subframe.iter().map(|s| s.active).sum::<usize>();
        let first_places = places(&first_wants, &options, total)?;
        let mut later = Vec::with_capacity(p.more_subframes.len());
        for (g, sub) in p.more_subframes.iter().zip(&detail.subframes[1..]) {
            let params = sub.params.ok_or(GenError::Subframe)?;
            let symbols =
                subframe_symbols(&params, 0, excess, sub, 0).map_err(|_| GenError::Subframe)?;
            let total = symbols.iter().map(|s| s.active).sum::<usize>();
            let placed = places(&g.plps, &options, total)?;
            later.push((symbols, total, placed));
        }
        // The most FEC blocks of a hybrid time interleaved PLP in a subframe, and the place of each PLP in L1-Detail.
        let all = || {
            std::iter::once(&first_places)
                .chain(later.iter().map(|l| &l.2))
                .enumerate()
        };
        for (_, list) in all() {
            for place in list.iter().filter(|p| p.on.is_none()) {
                let (sender, g) = (&mut plps[place.sender], &options[place.sender]);
                if let GenTi::Hybrid { spare, .. } = g.time_interleaver {
                    let most = place.size / g.coding.cells() + usize::from(spare);
                    sender.most = sender.most.max(most);
                    sender.fewest = sender.fewest.min(place.size / g.coding.cells());
                    if most > 4096 {
                        return Err(GenError::Subframe);
                    }
                }
            }
        }
        for (k, list) in all() {
            for place in list {
                let entry = &mut detail.subframes[k].plps[place.entry];
                entry.start = place.start as u32;
                entry.size = place.size as u32;
                if let Some(h) = &mut entry.hti {
                    let block = options[place.sender].coding.cells();
                    h.fec_blocks[0] = (place.size / block - 1) as u16;
                    h.fec_blocks_max = (plps[place.sender].most - 1) as u16;
                }
            }
        }
        Ok(FrameBuilder {
            l1_basic_points: l1_points(&l1_basic)?,
            l1_detail_points: l1_points(&l1_detail)?,
            iffts: [None, None, None],
            p,
            structure,
            basic,
            detail,
            preamble,
            subframe,
            l1_basic,
            l1_detail,
            plps,
            total,
            places: first_places,
            later,
            postfix,
            frames: 0,
        })
    }

    /// The Preamble structure that the bootstrap signals.
    pub fn structure(&self) -> &PreambleStructure {
        &self.structure
    }

    /// The options.
    pub fn params(&self) -> &Atsc3GenParams {
        &self.p
    }

    /// The L1-Basic of the frames.
    pub fn basic(&self) -> &L1Basic {
        &self.basic
    }

    /// The L1-Detail of the frame built last (of the first frame before any).
    pub fn detail(&self) -> &L1Detail {
        &self.detail
    }

    /// The samples of one frame at 6.144 Msps.
    pub fn frame_samples(&self) -> usize {
        4 * bootstrap::SYM
            + self
                .preamble
                .iter()
                .chain(&self.subframe)
                .chain(self.later.iter().flat_map(|l| &l.0))
                .map(Symbol::samples)
                .sum::<usize>()
            + self.postfix
    }

    /// The cells of a signalling block of `code` from its bits `sig`.
    fn l1_cells(code: &mut L1Code, points: &Constellation, sig: &[u8], out: &mut Vec<Complex32>) {
        let mut scrambled = sig.to_vec();
        Scrambler::apply_bits(&mut scrambled);
        let mut sent = Vec::new();
        code.encode(&scrambled, &mut sent)
            .expect("a block the mode carries");
        points.set().map_bits(&sent, out).expect("whole cells");
    }

    /// One OFDM symbol with its guard from its data cells `x`, at unit power.
    fn symbol(&mut self, s: &Symbol, x: &[Complex32], out: &mut Vec<Complex32>) {
        let n = s.carriers.fft.size();
        let mut a = vec![Complex32::default(); x.len()];
        if s.interleaved {
            interleave(s.carriers.fft, s.l, x, &mut a);
        } else {
            a.copy_from_slice(x);
        }
        let mut f = vec![Complex32::default(); n];
        let mut data = a.iter();
        for (k, &pilot) in s.carriers.pilot.iter().enumerate() {
            f[s.carriers.bin(k)] = if pilot != 0.0 {
                Complex32::new(pilot, 0.0)
            } else {
                *data.next().expect("a cell per data carrier")
            };
        }
        let power: f64 = s.carriers.pilot_power() + s.active as f64;
        let scale = (1.0 / power).sqrt() as f32;
        self.iffts[usize::from(s.carriers.fft.code())]
            .get_or_insert_with(|| Fft::inverse(n))
            .process(&mut f);
        f.iter_mut().for_each(|v| *v *= scale);
        out.extend_from_slice(&f[n - s.guard..]);
        out.extend_from_slice(&f);
    }

    /// The data cells of one subframe of `total` cells: each PLP in its subslices, and the dummy values of clause 7.2.6.5
    /// in the cells of no PLP. Sets the fields of the L1-Detail entries `entries` that change from subframe to subframe.
    fn data_cells(
        plps: &mut [PlpSender],
        places: &[Place],
        entries: &mut [Plp],
        total: usize,
    ) -> Vec<Complex32> {
        let mut data: Vec<Option<Complex32>> = vec![None; total];
        for (i, place) in places.iter().enumerate() {
            if place.on.is_some() {
                continue;
            }
            let cells = match places.iter().find(|e| e.on == Some(i)) {
                None => plps[place.sender].subframe(&mut entries[place.entry], place.size, &mut |_| {}),
                Some(e) => {
                    let (core, enhanced) = if place.sender < e.sender {
                        let (a, b) = plps.split_at_mut(e.sender);
                        (&mut a[place.sender], &mut b[0])
                    } else {
                        let (a, b) = plps.split_at_mut(place.sender);
                        (&mut b[0], &mut a[e.sender])
                    };
                    let mut layered = entries[e.entry];
                    let cells = core.subframe(&mut entries[place.entry], place.size, &mut |c| {
                        enhanced.layer(&mut layered, c)
                    });
                    entries[e.entry] = layered;
                    cells
                }
            };
            let entry = &mut entries[place.entry];
            entry.size = cells.len() as u32;
            let each = cells.len().div_ceil(place.subslices).max(1);
            for (j, part) in cells.chunks(each).enumerate() {
                let at = place.start + j * place.interval;
                for (to, &c) in data[at..at + part.len()].iter_mut().zip(part) {
                    *to = Some(c);
                }
            }
        }
        // Bit `i` of the scrambling sequence belongs to data cell `i`, whether or not a PLP takes the cell.
        data.into_iter()
            .zip(Scrambler::bits(total))
            .map(|(c, b)| c.unwrap_or(Complex32::new(1.0 - 2.0 * f32::from(b), 0.0)))
            .collect()
    }

    /// The next frame at 6.144 Msps: the bootstrap, the Preamble symbols and the subframes.
    pub fn frame(&mut self) -> Vec<Complex32> {
        let plp = Self::data_cells(
            &mut self.plps,
            &self.places,
            &mut self.detail.subframes[0].plps,
            self.total,
        );
        let later: Vec<Vec<Complex32>> = self
            .later
            .iter()
            .zip(&mut self.detail.subframes[1..])
            .map(|((_, total, places), sub)| {
                Self::data_cells(&mut self.plps, places, &mut sub.plps, *total)
            })
            .collect();
        let mut l1 = Vec::new();
        let basic_bits = self.basic.pack();
        Self::l1_cells(
            &mut self.l1_basic,
            &self.l1_basic_points,
            &basic_bits,
            &mut l1,
        );
        let detail_bits = self
            .detail
            .pack_with(&self.basic, self.p.l1_detail_reserved);
        let (blocks, k_sig) = detail_segments(detail_bits.len(), self.l1_detail.max_k_sig());
        let mut padded = detail_bits;
        padded.resize(blocks * k_sig, 0);
        for part in padded.chunks(k_sig) {
            Self::l1_cells(&mut self.l1_detail, &self.l1_detail_points, part, &mut l1);
        }
        // Clause 7.2.5.2: the cells of L1-Detail go row by row into one column per Preamble symbol and are read by columns.
        let basic_cells = l1.len() - self.basic.detail_cells as usize;
        let total = self.basic.detail_cells as usize;
        let columns = self.preamble.len();
        let rows = total / columns;
        let written: Vec<Complex32> = l1[basic_cells..].to_vec();
        for n in 0..rows * columns {
            l1[basic_cells + n] = written[(n % rows) * columns + n / rows];
        }
        let mut x = bootstrap::bootstrap(&self.p.fields);
        x.reserve(self.frame_samples());
        let (mut l1_at, mut plp_at) = (0, 0);
        let symbols: Vec<Symbol> = self
            .preamble
            .iter()
            .chain(&self.subframe)
            .cloned()
            .collect();
        for (i, s) in symbols.iter().enumerate() {
            let total = s.carriers.data_cells();
            let mut cells = vec![Complex32::default(); total];
            // The signalling fills the Preamble symbols in turn; the PLP follows it in the last one.
            let signalling = if i < self.preamble.len() {
                (l1.len() - l1_at).min(total)
            } else {
                0
            };
            cells[..signalling].copy_from_slice(&l1[l1_at..l1_at + signalling]);
            l1_at += signalling;
            let (first, take) = if i < self.preamble.len() {
                (signalling, total - signalling)
            } else {
                (s.null_low, s.active)
            };
            cells[first..first + take].copy_from_slice(&plp[plp_at..plp_at + take]);
            plp_at += take;
            self.symbol(s, &cells, &mut x);
        }
        debug_assert_eq!(plp_at, plp.len());
        for (k, data) in later.iter().enumerate() {
            let symbols = self.later[k].0.clone();
            let mut at = 0;
            for s in &symbols {
                let mut cells = vec![Complex32::default(); s.carriers.data_cells()];
                cells[s.null_low..s.null_low + s.active].copy_from_slice(&data[at..at + s.active]);
                at += s.active;
                self.symbol(s, &cells, &mut x);
            }
        }
        let last = self.later.last().and_then(|l| l.0.last());
        let useful = x.len() - last.map_or(self.structure.fft.size(), |s| s.carriers.fft.size());
        x.extend_from_within(useful..useful + self.postfix);
        self.frames += 1;
        x
    }
}

/// The ATSC 3.0 test signal: the frames of a [`FrameBuilder`], resampled to the output rate. With the default options a frame
/// is a bootstrap (preamble structure 3: 8K FFT, guard 192 samples, L1-Basic mode 4), one Preamble symbol and 72 symbols of
/// one subframe (SP16_2 at boost 1, both boundary symbols, frequency interleaved) with one PLP (QPSK, 8/15, 64800-bit LDPC with
/// BCH, no time interleaver), 0.1016 s long. The options add PLPs, subslices and the time interleavers. It makes the clean signal, and the framework adds the impairments.
pub struct Atsc3Generator {
    rate_hz: Hz,
    builder: FrameBuilder,
    rs: FractionalResampler<Complex32>,
    pending: Vec<Complex32>,
    at: usize,
}

impl Atsc3Generator {
    /// With these options.
    ///
    /// # Errors
    ///
    /// [`GenError`] if an option is not one of the standard or no resampler serves the output rate.
    pub fn new(p: Atsc3GenParams) -> Result<Atsc3Generator, GenError> {
        let rate_hz = p.rate_hz;
        Ok(Atsc3Generator {
            rs: resampler(
                bootstrap::RATE,
                rate_hz,
                OCCUPIED / 2.0,
                0.5 * bootstrap::RATE.min(rate_hz),
            )
            .map_err(|_| GenError::Rate)?,
            builder: FrameBuilder::new(p)?,
            rate_hz,
            pending: Vec::new(),
            at: 0,
        })
    }
    /// From the engine's configuration.
    ///
    /// # Errors
    ///
    /// As [`Atsc3Generator::new`].
    pub fn from_config(cfg: &GenConfig) -> Result<Atsc3Generator, GenError> {
        Atsc3Generator::new(Atsc3GenParams {
            rate_hz: cfg.rate_hz,
            seed: cfg.seed,
            ..Default::default()
        })
    }
    /// The frame builder.
    pub fn builder(&self) -> &FrameBuilder {
        &self.builder
    }
}

impl Generator for Atsc3Generator {
    fn rate_hz(&self) -> Hz {
        self.rate_hz
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR is measured in 6.144 MHz.
        Some(SignalInfo::new(bootstrap::RATE))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.at >= self.pending.len() {
                let x = self.builder.frame();
                self.pending.clear();
                self.rs.process(&x, &mut self.pending);
                self.at = 0;
                continue;
            }
            let n = (out.len() - w).min(self.pending.len() - self.at);
            out[w..w + n].copy_from_slice(&self.pending[self.at..self.at + n]);
            self.at += n;
            w += n;
        }
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A digest of the sample bits of `frames` frames of `p`.
    fn digest(p: Atsc3GenParams, frames: usize) -> u64 {
        let mut b = FrameBuilder::new(p).unwrap();
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for _ in 0..frames {
            for v in b.frame() {
                for x in [v.re.to_bits(), v.im.to_bits()] {
                    h = (h ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        h
    }

    /// The signal without layers stays as it was, sample for sample, with each time interleaver, two PLPs and subslices.
    #[test]
    fn the_signal_without_layers_is_unchanged() {
        let short = PlpCoding {
            ldpc_bits: 16200,
            rate: 8,
            bits: 2,
            outer: Outer::Bch,
        };
        let base = Atsc3GenParams {
            symbols: 24,
            ..Atsc3GenParams::default()
        };
        let hti = |inter_subframe, ti_blocks| GenTi::Hybrid {
            inter_subframe,
            ti_blocks,
            cell_interleaver: true,
            spare: 1,
        };
        let cases = [
            base.clone(),
            Atsc3GenParams {
                coding: short,
                time_interleaver: GenTi::Convolutional {
                    depth: 1,
                    extended: false,
                },
                ..base.clone()
            },
            Atsc3GenParams {
                coding: short,
                time_interleaver: hti(false, 3),
                ..base.clone()
            },
            Atsc3GenParams {
                coding: short,
                time_interleaver: hti(true, 3),
                ..base.clone()
            },
            Atsc3GenParams {
                coding: short,
                plp_cells: Some(40_000),
                subslices: (4, 25_000),
                more_plps: vec![GenPlp {
                    coding: PlpCoding {
                        bits: 4,
                        outer: Outer::Crc,
                        ..short
                    },
                    seed: 9,
                    start: 12_000,
                    cells: Some(40_000),
                    subslices: 4,
                    interval: 25_000,
                    time_interleaver: hti(false, 2),
                    lls: true,
                    ldm: None,
                }],
                ..base.clone()
            },
        ];
        let got: Vec<u64> = cases.into_iter().map(|p| digest(p, 3)).collect();
        assert_eq!(
            got,
            [
                0x252f_5798_967d_7a83,
                0x0927_15c5_302d_c899,
                0x80c0_bd71_b605_cbb0,
                0xab1b_a27b_940c_7a68,
                0xfd95_eb48_52e6_969f,
            ]
        );
    }
}
