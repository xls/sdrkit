//! The DVB-C2 test signal: a system of one or two L1 blocks placed by the tuned frequency, with one to three data slices and
//! their PLPs (`gen_system`: one PLP, two PLPs, a data PLP with its common PLP, a PLP bundled over two slices, a type 1 and a
//! type 2 slice, two data PLPs of one group with their common PLP in a slice of its own, and that group over two L1 blocks)
//! carrying the test transport stream, GSE, a GFPS or a GCS. The generator places the signal itself: its samples are centred
//! on the frequency of the setting `gen_frequency`, carrier `k` at `k / T_U - f` from 0 Hz.
//!
//! Standard: ETSI EN 302 769 V1.3.1.

use super::bicm::PlpEncoder;
use super::header::{self, preamble_bits};
use super::l1::{L1Codec, L1Part2, L1Sizes, Notch, Plp, Slice};
use super::packets::SliceTx;
use super::settings::*;
use super::signal::{
    freq_addresses, l1_cell, sequences, Cell, Guard, Layout, BLOCK, DATA_SYMBOLS, FFT, L1_CELLS,
    PILOT_AMP, Raster,
};
use super::tables::{ModCod, W_RM_DEFAULT};
use sdrkit_proto_dvbs2::transmit::{payload, Payload, GEN_PAYLOAD};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_fec::twistblock::TwistedBlock;
use sdrkit_core::{Choice, Complex32, Generator, SignalInfo, Values};
use sdrkit_proto_ipcast::adapt::{TsFramer, NULL_PACKET};
use sdrkit_proto_ipcast::bbheader::{BbHeader, Mode, StreamFormat, BBHEADER_LEN};
use sdrkit_proto_ipcast::gse::{GseWriter, Label};
use sdrkit_proto_ipcast::ip::write_ipv4_udp;
use sdrkit_proto_mpegts::source::{Fill, Numbered, NumberedSource, PacketSource};
use std::collections::VecDeque;
use std::net::Ipv4Addr;

/// The test signal cannot be built from its settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenError;

impl std::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the DVB-C2 test signal cannot be built from these settings")
    }
}

impl std::error::Error for GenError {}

/// What the test signal is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenParams {
    /// The RF centre of the samples.
    pub freq_hz: f64,
    /// The frequency that places the system when the samples are centred on another one; `None`: `freq_hz`.
    pub system_hz: Option<f64>,
    /// The channel raster.
    pub raster: Raster,
    /// The guard interval.
    pub guard: Guard,
    /// The coding of the PLPs.
    pub modcod: ModCod,
    /// The slice type.
    pub slice: GenSliceType,
    /// DSLICE_TI_DEPTH.
    pub ti_depth: u8,
    /// L1_TI_MODE.
    pub l1_ti: u8,
    /// A narrowband notch.
    pub notch: bool,
    /// Reserved tones.
    pub reserved_tones: bool,
    /// The slices and PLPs.
    pub system: GenSystem,
    /// What the data PLPs carry.
    pub stream: GenStream,
    /// High efficiency mode BBFRAMEs (transport stream and GSE).
    pub hem: bool,
    /// The numbered packets only.
    pub numbered: bool,
    /// The time interleaver has one column per carrier.
    pub ti_full_width: bool,
    /// The header scrambling word w^RM, w^RM_0 the most significant bit.
    pub w_rm: u32,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            freq_hz: 346e6,
            system_hz: None,
            raster: Raster::R8,
            guard: Guard::G1_64,
            modcod: GenModCod::Q256R56.modcod(true),
            slice: GenSliceType::Type1,
            ti_depth: 0,
            l1_ti: 0,
            notch: false,
            reserved_tones: false,
            system: GenSystem::Single,
            stream: GenStream::Ts,
            hem: false,
            numbered: false,
            ti_full_width: true,
            w_rm: W_RM_DEFAULT,
        }
    }
}

/// The PLP_ID of the common PLP of the test signal.
pub const COMMON_PLP: u8 = 9;

impl GenParams {
    /// The parameters of the `gen_*` settings in `v`.
    pub fn from_values(v: &Values) -> GenParams {
        GenParams {
            freq_hz: v.get(&GEN_FREQUENCY),
            system_hz: None,
            raster: v.get(&RASTER),
            guard: v.get(&GEN_GUARD),
            modcod: v.get(&GEN_MODCOD).modcod(!v.get(&GEN_SHORT_FRAMES)),
            slice: v.get(&GEN_SLICE_TYPE),
            ti_depth: v.get(&GEN_TI_DEPTH).index() as u8,
            l1_ti: v.get(&GEN_L1_TI).index() as u8,
            notch: v.get(&GEN_NOTCH),
            reserved_tones: v.get(&GEN_RESERVED_TONES),
            system: v.get(&GEN_SYSTEM),
            stream: v.get(&GEN_STREAM),
            hem: v.get(&GEN_HEM),
            numbered: v.get(&GEN_PAYLOAD) == Payload::Numbered,
            ti_full_width: v.get(&TI_FULL_WIDTH),
            w_rm: v.get(&RM_SCRAMBLING) as u32,
        }
    }

    /// The carrier at 0 Hz of the samples, and the carrier that places the system: the multiple of D_X at or below the
    /// frequency of `system_hz` (of `freq_hz` without it).
    pub fn carriers(&self) -> (usize, usize) {
        let carrier = |hz: f64| (hz / self.raster.spacing()).round() as usize;
        let dx = self.guard.d_x();
        (carrier(self.freq_hz), carrier(self.system_hz.unwrap_or(self.freq_hz)) / dx * dx)
    }

    /// Whether the slices that follow `gen_slice_type` are of type 2.
    pub fn type2(&self) -> bool {
        self.slice != GenSliceType::Type1
            || matches!(
                self.system,
                GenSystem::TwoPlps | GenSystem::Common | GenSystem::Bundled | GenSystem::Group | GenSystem::FarSlice
            )
    }

    /// Whether the system carries a group of data PLPs with a common PLP.
    pub fn grouped(&self) -> bool {
        matches!(self.system, GenSystem::Common | GenSystem::Group | GenSystem::FarSlice)
    }

    /// Whether the PLPs of the group run at one packet rate in slices of their own: each sends one FEC block per round, and
    /// stuffing packets fill what a slice has left.
    pub fn co_timed(&self) -> bool {
        matches!(self.system, GenSystem::Group | GenSystem::FarSlice)
    }

    /// Whether the BBFRAMEs are of high efficiency mode: asked for (transport stream and GSE only), or a bundled PLP.
    pub fn high_efficiency(&self) -> bool {
        (self.hem && matches!(self.stream, GenStream::Ts | GenStream::Gse)) || self.system == GenSystem::Bundled
    }

    /// The L1 part 2 of the test signal (PLP_START 0).
    pub fn l1(&self) -> L1Part2 {
        let dx = self.guard.d_x();
        let (_, kt) = self.carriers();
        let half = (BLOCK / 2 / dx) as i16;
        let type2 = self.type2();
        let payload = match self.stream {
            GenStream::Gfps => 0,
            GenStream::Gcs => 1,
            GenStream::Gse => 2,
            GenStream::Ts => 3,
        };
        // A PLP of a slice of type 1 (`type2` off: the L1 names its coding) or of type 2.
        let plp_of = |id: u8, type2: bool| Plp {
            id,
            plp_type: 2,
            payload,
            long: !type2 && self.modcod.long,
            plp_mod: if type2 { 0 } else { self.modcod.qam.code() },
            plp_cod: if type2 { 0 } else { self.modcod.rate.code() },
            ts_id: 0x0C21,
            on_id: 0x2F00,
            ..Plp::default()
        };
        let plp = |id: u8| plp_of(id, type2);
        let slice_of = |id: u8, tune: i16, left: i16, right: i16, type2: bool, plps: Vec<Plp>| Slice {
            id,
            tune_pos: tune as u16,
            offset_left: left,
            offset_right: right,
            ti_depth: self.ti_depth,
            type2,
            efficient_header: type2 && self.slice == GenSliceType::Type2Efficient,
            const_conf: type2,
            left_notch: false,
            plps,
        };
        let slice = |id: u8, tune: i16, left: i16, right: i16, plps: Vec<Plp>| slice_of(id, tune, left, right, type2, plps);
        // The PLPs of the group: grouped data PLPs 1 and 2 and the common PLP, all carrying a transport stream.
        let grouped = |id: u8| Plp { plp_type: 1, group: 1, ..plp(id) };
        let common = Plp { plp_type: 0, group: 1, payload: 3, ..plp(COMMON_PLP) };
        let wide = matches!(self.system, GenSystem::TwoSlices | GenSystem::FarSlice);
        let slices = match self.system {
            GenSystem::Single => vec![slice(0, half, -half, half, vec![plp(1)])],
            GenSystem::TwoPlps => vec![slice(0, half, -half, half, vec![plp(1), plp(2)])],
            GenSystem::Common => vec![slice(0, half, -half, half, vec![grouped(1), common])],
            GenSystem::Bundled => {
                let b = Plp { bundled: true, ..plp(1) };
                let cut = -half + 2 * half / 3;
                vec![slice(0, half, -half, cut, vec![b]), slice(1, half, cut, half, vec![b])]
            }
            GenSystem::TwoSlices => {
                vec![slice(0, half, -half, half, vec![plp(1)]), slice(1, 3 * half, -half, half, vec![plp(2)])]
            }
            GenSystem::MixedSlices => vec![
                slice_of(0, half, -half, 0, false, vec![plp_of(1, false)]),
                slice_of(1, half, 0, half, true, vec![plp_of(2, true)]),
            ],
            GenSystem::Group => {
                // The data slice has two thirds of the block for its two PLPs, the slice of the common PLP one third.
                let cut = half - 2 * half / 3;
                vec![
                    slice(0, half, -half, cut, vec![grouped(1), grouped(2)]),
                    slice(1, half, cut, half, vec![common]),
                ]
            }
            GenSystem::FarSlice => {
                // PLP 1 over the lower L1 block; the common PLP in the first 432 carriers of the upper block; PLP 2 in the
                // rest of it. The window at the tuning position of slice 2 holds slices 1 and 2; no window holds 0 and 1.
                let narrow = (432 / dx) as i16;
                vec![
                    slice(0, half, -half, half, vec![grouped(1)]),
                    slice(1, 2 * half, 0, narrow, vec![common]),
                    slice(2, 3 * half, narrow - half, half, vec![grouped(2)]),
                ]
            }
        };
        let blocks = if wide { 2 } else { 1 };
        // The frequency of the test signal is the centre of the system, or of its lower L1 block (`far_slice`).
        let below = if self.system == GenSystem::FarSlice { BLOCK / 2 } else { blocks * BLOCK / 2 };
        L1Part2 {
            network_id: 0x3001,
            system_id: 0x0001,
            start: (kt - below) as u32,
            bandwidth: (blocks * BLOCK / dx) as u16,
            guard: self.guard,
            frame_length: DATA_SYMBOLS as u16,
            change_counter: 0,
            slices,
            notches: if self.notch {
                vec![Notch {
                    start: (half / 2) as u16,
                    width: if self.guard == Guard::G1_64 { 3 } else { 1 },
                }]
            } else {
                Vec::new()
            },
            reserved_tones: self.reserved_tones,
            ews: false,
            version: 1,
        }
    }

    /// The layout of the data symbols.
    pub fn layout(&self) -> Layout {
        layout_of(&self.l1())
    }
}

/// The layout of the data symbols of a system.
pub fn layout_of(l1: &L1Part2) -> Layout {
    let dx = l1.guard.d_x();
    Layout {
        guard: l1.guard,
        k_min: l1.k_min(),
        k_max: l1.k_max(),
        notches: l1
            .notches
            .iter()
            .filter_map(|n| n.carriers(l1.k_min(), dx))
            .collect(),
        reserved: l1.reserved_tones,
    }
}

/// The payload cells of a slice in data symbol `l`: carrier offsets from K_DS,min, increasing. With `full_width` off the last
/// carrier is left out.
pub fn slice_payload(layout: &Layout, ds: (usize, usize), l: usize, full_width: bool, out: &mut Vec<u32>) {
    out.clear();
    let end = if full_width { ds.1 } else { ds.1.saturating_sub(1) };
    for k in ds.0..=end {
        if layout.class(k, l) == Cell::Data {
            out.push((k - ds.0) as u32);
        }
    }
}

/// The IPv4/UDP packet number `n` of the GSE test signal (a UDP payload of 40 to 3000 bytes).
pub fn gse_test_packet(n: u32, out: &mut Vec<u8>) {
    out.clear();
    let len = 40 + (n as usize * 797) % 2961;
    let data: Vec<u8> = (0..len).map(|k| (k as u32 ^ n.wrapping_mul(31)) as u8).collect();
    let _ = write_ipv4_udp(
        out,
        Ipv4Addr::new(10, 2, 0, 1),
        Ipv4Addr::new(239, 3, 0, 1),
        n as u16,
        9000,
        4000,
        5004,
        &data,
        |_| {},
    );
}

/// The bytes of user packet `n` of the GFPS test stream: [`GFPS_UPL`] bytes, the sync byte [`GFPS_SYNC`] first, then `n` (32
/// bits, big-endian) and a fill that depends on `n`.
pub fn gfps_test_packet(n: u32, out: &mut [u8; GFPS_UPL]) {
    out[0] = GFPS_SYNC;
    out[1..5].copy_from_slice(&n.to_be_bytes());
    for (k, b) in out[5..].iter_mut().enumerate() {
        *b = (k as u32 ^ n.wrapping_mul(29)) as u8;
    }
}

/// The user packet length of the GFPS test stream, bytes.
pub const GFPS_UPL: usize = 96;
/// The sync byte of the GFPS test stream.
pub const GFPS_SYNC: u8 = 0xB8;

/// Byte `i` of the GCS test stream: the 32-bit counters 0, 1, 2 ... big-endian, one after the other.
pub fn gcs_test_byte(i: u64) -> u8 {
    ((i / 4) as u32).to_be_bytes()[(i % 4) as usize]
}

/// Whether slot `n` of the transport streams of a group carries a packet of the common PLP: about one slot in four, chosen
/// by a hash of its index so that no shift of the pattern resembles it.
pub fn common_slot(n: u64) -> bool {
    sdrkit_algo_sim::rng::SplitMix64::mix(n) >> 62 == 3
}

/// The input transport stream of data PLP `plp` of the group of the test signal (`gen_system` `group` and `far_slice`):
/// numbered packets of the PLP's own PID, and in the slots of [`common_slot`] the numbered packets of the PID that every
/// stream of the group carries.
#[derive(Clone, Copy, Debug)]
pub struct GroupInput {
    own: NumberedSource,
    shared: NumberedSource,
    n: u64,
}

/// The PID of the packets that every transport stream of the group carries.
pub const GROUP_SHARED_PID: u16 = 0x002F;

impl GroupInput {
    /// The stream of data PLP `plp` (1 or 2) from its first packet. PLP 1 has the numbered packets of the cable pattern.
    pub fn new(plp: u8) -> GroupInput {
        let own = Numbered::new(Numbered::SATELLITE_CABLE.pid + u16::from(plp.saturating_sub(1)), Fill::N7I13);
        GroupInput {
            own: NumberedSource::new(own),
            shared: NumberedSource::new(Numbered::new(GROUP_SHARED_PID, Fill::N7I13)),
            n: 0,
        }
    }
}

impl PacketSource for GroupInput {
    fn next_packet(&mut self, out: &mut [u8; 188]) {
        if common_slot(self.n) {
            self.shared.next_packet(out);
        } else {
            self.own.next_packet(out);
        }
        self.n += 1;
    }
}

/// The split of the transport streams of a group into its grouped data PLPs and its common PLP: the packet of a slot of
/// [`common_slot`] (the same in every stream of the group) moves to the common PLP and leaves a null packet in each data PLP;
/// the common PLP has a null packet in every other slot. All streams keep the packet slots of the inputs, so a receiver that
/// puts the common packets into the null slots of a data PLP gets the input of that PLP back.
struct Splitter {
    inputs: Vec<SplitInput>,
    common: VecDeque<[u8; 188]>,
    n: u64,
}

/// The input of one data PLP of the splitter and its packets not yet sent.
struct SplitInput {
    plp: u8,
    src: Box<dyn PacketSource>,
    data: VecDeque<[u8; 188]>,
}

impl Splitter {
    fn new(inputs: Vec<(u8, Box<dyn PacketSource>)>) -> Splitter {
        Splitter {
            inputs: inputs.into_iter().map(|(plp, src)| SplitInput { plp, src, data: VecDeque::new() }).collect(),
            common: VecDeque::new(),
            n: 0,
        }
    }

    /// The next packet of data PLP `plp`, or of the common PLP (`None`).
    fn next(&mut self, plp: Option<u8>) -> [u8; 188] {
        loop {
            let q = match plp {
                Some(id) => self.inputs.iter_mut().find(|i| i.plp == id).map(|i| &mut i.data),
                None => Some(&mut self.common),
            };
            let Some(q) = q else { return NULL_PACKET };
            if let Some(p) = q.pop_front() {
                return p;
            }
            // One more slot of every stream.
            let shared = common_slot(self.n);
            for (k, i) in self.inputs.iter_mut().enumerate() {
                let mut p = [0u8; 188];
                i.src.next_packet(&mut p);
                if !shared {
                    i.data.push_back(p);
                    continue;
                }
                i.data.push_back(NULL_PACKET);
                if k == 0 {
                    self.common.push_back(p);
                }
            }
            if !shared {
                self.common.push_back(NULL_PACKET);
            }
            self.n += 1;
        }
    }
}

/// Where the transport packets of a PLP come from.
enum TsFrom {
    Own(Box<dyn PacketSource>),
    /// The splitter: a data PLP, or the common PLP (`None`).
    Split(Option<u8>),
}

impl TsFrom {
    fn next(&mut self, split: &mut Option<Splitter>) -> [u8; 188] {
        match (self, split) {
            (TsFrom::Own(src), _) => {
                let mut p = [0u8; 188];
                src.next_packet(&mut p);
                p
            }
            (TsFrom::Split(plp), Some(s)) => s.next(*plp),
            (TsFrom::Split(_), None) => NULL_PACKET,
        }
    }
}

/// Where the BBFRAMEs of a PLP come from.
enum Feed {
    Ts { from: TsFrom, framer: TsFramer },
    Gse { writer: GseWriter, next: u32, pkt: Vec<u8> },
    Gfps { framer: TsFramer, next: u32 },
    Gcs { next: u64 },
}

struct PlpSource {
    id: u8,
    enc: PlpEncoder,
    feed: Feed,
    hem: bool,
    /// ISCR of the next BBFRAME when the header carries ISSY.
    iscr: Option<u32>,
    frame: Vec<u8>,
    cells: Vec<Complex32>,
}

/// The step of the ISCR of the test signal per BBFRAME of a bundled PLP.
const ISCR_STEP: u32 = 1000;

impl PlpSource {
    /// The source of PLP `id`; `common`: the common PLP of the splitter, `split`: the data PLP of the splitter.
    fn new(id: u8, m: ModCod, p: &GenParams, common: bool, split: bool) -> Option<PlpSource> {
        let enc = PlpEncoder::new(m)?;
        let hem = p.high_efficiency();
        let from = || {
            if common {
                TsFrom::Split(None)
            } else if split {
                TsFrom::Split(Some(id))
            } else {
                TsFrom::Own(payload(p.numbered || id != 1))
            }
        };
        // Co-timed PLPs carry the same packets per BBFRAME: the common PLP has the mode of the data PLPs.
        let plain = common && !p.co_timed();
        let feed = match p.stream {
            _ if plain => Feed::Ts { from: from(), framer: TsFramer::default() },
            GenStream::Ts if hem => Feed::Ts { from: from(), framer: TsFramer::high_efficiency(188) },
            GenStream::Ts => Feed::Ts { from: from(), framer: TsFramer::default() },
            GenStream::Gse => Feed::Gse { writer: GseWriter::default(), next: 0, pkt: Vec::new() },
            GenStream::Gfps => Feed::Gfps { framer: TsFramer::normal(GFPS_UPL), next: 0 },
            GenStream::Gcs => Feed::Gcs { next: 0 },
        };
        Some(PlpSource {
            id,
            frame: vec![0; enc.frame_bytes()],
            cells: vec![Complex32::default(); m.cells()],
            hem: hem && !plain,
            iscr: (p.system == GenSystem::Bundled).then_some(0),
            enc,
            feed,
        })
    }

    /// Encodes the next BBFRAME into `cells`. `multi`: the system carries more than one PLP.
    fn next_block(&mut self, multi: bool, split: &mut Option<Splitter>) -> Option<()> {
        let dfl = self.frame.len() - BBHEADER_LEN;
        let df = &mut self.frame[BBHEADER_LEN..];
        let (format, upl, sync, syncd) = match &mut self.feed {
            Feed::Ts { from, framer } => {
                let syncd = framer.fill(df, &mut || from.next(split));
                if self.hem {
                    (StreamFormat::Transport, 0, 0, syncd)
                } else {
                    (StreamFormat::Transport, 188 * 8, 0x47, syncd)
                }
            }
            Feed::Gse { writer, next, pkt } => {
                while writer.backlog() < 2 * dfl {
                    gse_test_packet(*next, pkt);
                    *next += 1;
                    writer.push_pdu(0x0800, Label::Broadcast, pkt).ok()?;
                }
                writer.fill(df);
                (StreamFormat::Gse, 0, 0, 0)
            }
            Feed::Gfps { framer, next } => {
                let syncd = framer.fill(df, &mut || {
                    let mut p = [0u8; GFPS_UPL];
                    gfps_test_packet(*next, &mut p);
                    *next = next.wrapping_add(1);
                    p
                });
                (StreamFormat::GenericPacketised, (GFPS_UPL * 8) as u16, GFPS_SYNC, syncd)
            }
            Feed::Gcs { next } => {
                for (j, b) in df.iter_mut().enumerate() {
                    *b = gcs_test_byte(*next + j as u64);
                }
                *next += dfl as u64;
                (StreamFormat::GenericContinuous, 0, 0, 0)
            }
        };
        let issy = self.iscr.map(|v| {
            self.iscr = Some((v + ISCR_STEP) & 0x3F_FFFF);
            // ISCRlong: 10 then 22 bits (EN 302 755 annex C).
            [0x80 | (v >> 16) as u8 & 0x3F, (v >> 8) as u8, v as u8]
        });
        let h = BbHeader {
            format,
            single_stream: !multi,
            ccm: true,
            issyi: issy.is_some(),
            npd: false,
            ext: 0,
            stream_id: if multi { self.id } else { 0 },
            upl,
            dfl: (dfl * 8) as u16,
            sync,
            syncd,
            mode: if self.hem { Mode::HighEfficiency } else { Mode::Normal },
            issy,
        };
        let mut hb = [0u8; BBHEADER_LEN];
        h.write(&mut hb);
        self.frame[..BBHEADER_LEN].copy_from_slice(&hb);
        self.enc.encode(&self.frame, &mut self.cells)
    }
}

/// The transmit side of one data slice.
struct SliceGen {
    ds: (usize, usize),
    cols: usize,
    ti: TwistedBlock,
    tx: SliceTx,
    payload: [Vec<u32>; 4],
    /// The sources of its PLPs (indices into the builder's sources).
    sources: Vec<usize>,
    turn: usize,
    /// Co-timed PLPs: the sources whose FEC block of the current round is not yet in the slice.
    waiting: VecDeque<usize>,
    mask: Vec<bool>,
    rows: Vec<Complex32>,
}

/// The frame builder: the cells of each symbol of a C2 frame by absolute carrier.
pub struct FrameBuilder {
    /// The parameters.
    pub p: GenParams,
    /// The L1 part 2 of the current frame.
    pub l1: L1Part2,
    /// The layout.
    pub layout: Layout,
    slices: Vec<SliceGen>,
    sources: Vec<PlpSource>,
    split: Option<Splitter>,
    pilots: Vec<u8>,
    addr: Vec<(bool, usize, Vec<u32>)>,
    codec: Option<L1Codec>,
    hcons: sdrkit_algo_modem::Constellation,
    l1_rows: Vec<Vec<Complex32>>,
    block_cells: Vec<Complex32>,
    fft_len: usize,
    /// Frames built.
    pub frames: u64,
}

impl FrameBuilder {
    /// A builder for `p`.
    pub fn new(p: GenParams) -> Option<FrameBuilder> {
        let l1 = p.l1();
        let layout = layout_of(&l1);
        let dx = p.guard.d_x();
        let mut sources: Vec<PlpSource> = Vec::new();
        let mut slices = Vec::new();
        let common = p.grouped();
        for s in &l1.slices {
            let ds = s.carriers(l1.k_min(), dx)?;
            let cols = ds.1 - ds.0 + 1;
            let mut ids = Vec::new();
            for q in &s.plps {
                let i = match sources.iter().position(|x| x.id == q.id) {
                    Some(i) => i,
                    None => {
                        let is_common = common && q.plp_type == 0;
                        sources.push(PlpSource::new(q.id, p.modcod, &p, is_common, common && q.plp_type == 1)?);
                        sources.len() - 1
                    }
                };
                ids.push(i);
            }
            let payload = std::array::from_fn(|d| {
                let mut v = Vec::new();
                slice_payload(&layout, ds, d, p.ti_full_width, &mut v);
                v
            });
            slices.push(SliceGen {
                ds,
                cols,
                ti: TwistedBlock::new(s.depth(), cols).ok()?,
                tx: SliceTx::new(s.type2, !s.efficient_header, p.w_rm),
                payload,
                sources: ids,
                turn: 0,
                waiting: VecDeque::new(),
                mask: Vec::new(),
                rows: Vec::new(),
            });
        }
        // The common PLP needs a transport stream; a bundled PLP's high efficiency mode needs a transport stream or GSE.
        let ts_or_gse = matches!(p.stream, GenStream::Ts | GenStream::Gse);
        if (common && p.stream != GenStream::Ts) || (p.system == GenSystem::Bundled && !ts_or_gse) {
            return None;
        }
        let split = common.then(|| {
            Splitter::new(if p.co_timed() {
                [1, 2].map(|id| (id, Box::new(GroupInput::new(id)) as Box<dyn PacketSource>)).into()
            } else {
                vec![(1, payload(p.numbered))]
            })
        });
        let mut pilots = Vec::new();
        let first = layout.k_min.div_ceil(6) * 6;
        sequences().pilot_bits(first, (layout.k_max - first) / 6 + 1, &mut pilots);
        // The synthesis FFT spans the whole system with a margin.
        let mut fft_len = FFT;
        while (layout.k_max - layout.k_min) as f64 > 0.88 * fft_len as f64 {
            fft_len *= 2;
        }
        Some(FrameBuilder {
            hcons: header::constellation(true),
            p,
            layout,
            slices,
            sources,
            split,
            pilots,
            addr: Vec::new(),
            codec: None,
            l1_rows: Vec::new(),
            block_cells: Vec::new(),
            fft_len,
            l1,
            frames: 0,
        })
    }

    /// K_DS,min and K_DS,max of slice `i`.
    pub fn slice_carriers(&self, i: usize) -> Option<(usize, usize)> {
        self.slices.get(i).map(|s| s.ds)
    }

    /// The size of the synthesis FFT: 4096 times the oversampling of the samples before resampling.
    pub fn fft_len(&self) -> usize {
        self.fft_len
    }

    /// The pilot bit r_k (k a multiple of 6 inside the system).
    fn r(&self, k: usize) -> u8 {
        let first = self.layout.k_min.div_ceil(6) * 6;
        self.pilots[(k - first) / 6]
    }

    fn addresses(&mut self, odd: bool, n: usize) -> usize {
        if let Some(i) = self.addr.iter().position(|a| a.0 == odd && a.1 == n) {
            return i;
        }
        let mut v = Vec::new();
        freq_addresses(odd, n, &mut v);
        self.addr.push((odd, n, v));
        self.addr.len() - 1
    }

    /// Queues FEC blocks into slice `i` until `n` cells are waiting. Co-timed PLPs send one block each per round: a new round
    /// starts when no slice still waits for a block of the last one, and a slice with nothing waiting takes a stuffing packet.
    fn fill(&mut self, i: usize, n: usize) -> Option<()> {
        let multi = self.sources.len() > 1;
        let m = self.p.modcod;
        let co_timed = self.p.co_timed();
        while self.slices[i].tx.queued() < n {
            if co_timed && self.slices.iter().all(|s| s.waiting.is_empty()) {
                for s in &mut self.slices {
                    s.waiting.extend(s.sources.iter().copied());
                }
            }
            let sl = &mut self.slices[i];
            let j = if co_timed {
                let Some(j) = sl.waiting.pop_front() else {
                    sl.tx.push_stuffing();
                    continue;
                };
                j
            } else {
                sl.turn += 1;
                sl.sources[(sl.turn - 1) % sl.sources.len()]
            };
            let s = &mut self.sources[j];
            s.next_block(multi, &mut self.split)?;
            self.slices[i].tx.push_block(s.id, &m, &s.cells);
        }
        Some(())
    }

    /// Builds one frame: `out(symbol cells, preamble)` for each symbol. The cells are written into a buffer of
    /// [`FrameBuilder::fft_len`] bins, carrier `k` at bin `(k - k_ref) mod fft_len`.
    pub fn frame(&mut self, k_ref: usize, out: &mut impl FnMut(&[Complex32], bool)) -> Option<()> {
        // PLP_START of this frame in each type 1 slice.
        for (s, g) in self.l1.slices.iter_mut().zip(&self.slices) {
            if !s.type2 {
                let n = self.p.modcod.cells() as u64;
                s.plps[0].start = ((n - g.tx.sent % n) % n) as u16;
            }
        }
        let bits = self.l1.to_bits();
        let sizes = L1Sizes::new(self.l1.info_size(), self.p.l1_ti)?;
        if self.codec.as_ref().map(|c| *c.sizes()) != Some(sizes) {
            self.codec = Some(L1Codec::new(sizes)?);
        }
        let codec = self.codec.as_mut()?;
        let mut rows = std::mem::take(&mut self.l1_rows);
        codec.encode(&bits, &mut rows)?;
        let mut head = Vec::new();
        header::cells(preamble_bits(sizes.info_size as u16, sizes.ti_mode), true, &self.hcons, self.p.w_rm, &mut head);
        let nb = self.fft_len;
        let bin = |k: usize| (k + nb - k_ref % nb) % nb;
        let mut bins = vec![Complex32::default(); nb];
        let mut x = vec![Complex32::default(); L1_CELLS];
        let a_pp = self.p.guard.a_pp();
        for (sym, row) in rows.iter().enumerate() {
            let unit = head.len() + row.len();
            for (i, c) in x.iter_mut().enumerate() {
                let j = i % unit;
                *c = if j < head.len() { head[j] } else { row[j - head.len()] };
            }
            let ai = self.addresses(sym % 2 == 1, L1_CELLS);
            bins.fill(Complex32::default());
            for k in self.layout.k_min..=self.layout.k_max {
                if self.layout.notches.iter().any(|&(a, b)| (a..=b).contains(&k)) {
                    continue;
                }
                let v = match l1_cell(k % BLOCK) {
                    None => Complex32::new(if self.r(k) == 0 { a_pp } else { -a_pp }, 0.0),
                    Some(q) => {
                        let a = x[self.addr[ai].2[q] as usize];
                        if sequences().w(k) == 1 {
                            -a
                        } else {
                            a
                        }
                    }
                };
                bins[bin(k)] = v;
            }
            out(&bins, true);
        }
        self.l1_rows = rows;
        // Data symbols, one time interleaving block at a time (every slice of the test signal has the same depth).
        let depth = self.slices[0].ti.rows();
        for blk in (0..DATA_SYMBOLS).step_by(depth) {
            for i in 0..self.slices.len() {
                let sl = &mut self.slices[i];
                sl.mask.clear();
                sl.mask.resize(depth * sl.cols, false);
                for r in 0..depth {
                    for &c in &sl.payload[(blk + r) % 4] {
                        sl.mask[r * sl.cols + c as usize] = true;
                    }
                }
                let n: usize = (0..depth).map(|r| sl.payload[(blk + r) % 4].len()).sum();
                self.fill(i, n)?;
                let mut cells = std::mem::take(&mut self.block_cells);
                let sl = &mut self.slices[i];
                sl.tx.take(n, &mut cells)?;
                sl.rows.resize(n, Complex32::default());
                sl.ti.interleave(Some(&sl.mask), &cells, &mut sl.rows).ok()?;
                self.block_cells = cells;
            }
            let mut at = vec![0usize; self.slices.len()];
            for r in 0..depth {
                let l = blk + r;
                let d = l % 4;
                bins.fill(Complex32::default());
                for k in self.layout.k_min..=self.layout.k_max {
                    if self.layout.class(k, l) == Cell::Pilot {
                        bins[bin(k)] = Complex32::new(if self.r(k) == 0 { PILOT_AMP } else { -PILOT_AMP }, 0.0);
                    }
                }
                for i in 0..self.slices.len() {
                    let cnt = self.slices[i].payload[d].len();
                    let ai = self.addresses(l % 2 == 1, cnt);
                    let sl = &self.slices[i];
                    let row = &sl.rows[at[i]..at[i] + cnt];
                    for (q, &c) in sl.payload[d].iter().enumerate() {
                        bins[bin(sl.ds.0 + c as usize)] = row[self.addr[ai].2[q] as usize];
                    }
                    at[i] += cnt;
                }
                out(&bins, false);
            }
        }
        self.frames += 1;
        Some(())
    }

    /// The sum of |X_k|^2 of a data symbol (for the output scale).
    pub fn data_power(&self) -> f32 {
        let mut p = 0f32;
        for k in self.layout.k_min..=self.layout.k_max {
            p += match self.layout.class(k, 0) {
                Cell::Pilot => PILOT_AMP * PILOT_AMP,
                Cell::Data => 1.0,
                _ => 0.0,
            };
        }
        p
    }

    /// The mean power of a used carrier over the power of a data cell.
    pub fn carrier_power_ratio(&self) -> f64 {
        let used = (self.layout.k_min..=self.layout.k_max)
            .filter(|&k| matches!(self.layout.class(k, 0), Cell::Pilot | Cell::Data))
            .count();
        f64::from(self.data_power()) / used as f64
    }
}

/// The DVB-C2 test signal generator.
pub struct C2Generator {
    fb: FrameBuilder,
    k_ref: usize,
    rate: f64,
    fft: Fft,
    scale: f32,
    /// Synthesis samples made so far (the time of the next symbol's first sample).
    sample: u64,
    /// The frequency shift that puts the carriers where `freq_hz` says, cycles per synthesis sample.
    shift: f64,
    phase: f64,
    native: Vec<Complex32>,
    resampler: Option<FractionalResampler<Complex32>>,
    queue: VecDeque<Complex32>,
}

impl C2Generator {
    /// A generator of `p` at `rate` samples per second. `None` for parameters it cannot build.
    pub fn new(p: GenParams, rate: f64) -> Option<C2Generator> {
        let fb = FrameBuilder::new(p)?;
        let (k_ref, _) = p.carriers();
        let synth = p.raster.rate() * (fb.fft_len() / FFT) as f64;
        let resampler = if (rate - synth).abs() < 1e-3 {
            None
        } else {
            Some(FractionalResampler::with_rates(synth, rate).ok()?)
        };
        let scale = 1.0 / fb.data_power().sqrt();
        Some(C2Generator {
            shift: (k_ref as f64 * p.raster.spacing() - p.freq_hz) / synth,
            fft: Fft::inverse(fb.fft_len()),
            fb,
            k_ref,
            rate,
            scale,
            sample: 0,
            phase: 0.0,
            native: Vec::new(),
            resampler,
            queue: VecDeque::new(),
        })
    }

    /// The frame builder.
    pub fn builder(&self) -> &FrameBuilder {
        &self.fb
    }

    /// One more frame into the queue.
    fn frame(&mut self) -> Option<()> {
        let n = self.fb.fft_len();
        let g = self.fb.p.guard.samples() * (n / FFT);
        let (k_ref, scale) = (self.k_ref, self.scale);
        let mut native = std::mem::take(&mut self.native);
        native.clear();
        let fft = &mut self.fft;
        let mut sample = self.sample;
        let mut buf = vec![Complex32::default(); n];
        self.fb.frame(k_ref, &mut |bins, _| {
            buf.copy_from_slice(bins);
            fft.process(&mut buf);
            // The phase of every carrier starts at the useful part on the absolute grid.
            let nu = sample + g as u64;
            let turns = ((k_ref as u64 % n as u64) * (nu % n as u64)) % n as u64;
            let a = -std::f64::consts::TAU * turns as f64 / n as f64;
            let rot = Complex32::new(a.cos() as f32, a.sin() as f32) * scale;
            native.extend(buf[n - g..].iter().map(|v| v * rot));
            native.extend(buf.iter().map(|v| v * rot));
            sample += (n + g) as u64;
        })?;
        self.sample = sample;
        if self.shift != 0.0 {
            for v in native.iter_mut() {
                let a = std::f64::consts::TAU * self.phase;
                *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
                self.phase = (self.phase + self.shift).fract();
            }
        }
        match &mut self.resampler {
            Some(r) => {
                let synth = self.fb.p.raster.rate() * (n / FFT) as f64;
                let mut out = Vec::with_capacity((native.len() as f64 * self.rate / synth) as usize + 64);
                r.process(&native, &mut out);
                self.queue.extend(out);
            }
            None => self.queue.extend(native.iter().copied()),
        }
        self.native = native;
        Some(())
    }
}

impl Generator for C2Generator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut n = 0;
        while n < out.len() {
            if self.queue.is_empty() && self.frame().is_none() {
                return n;
            }
            let take = (out.len() - n).min(self.queue.len());
            for (o, v) in out[n..n + take].iter_mut().zip(self.queue.drain(..take)) {
                *o = v;
            }
            n += take;
        }
        n
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        let used = (self.fb.layout.k_max - self.fb.layout.k_min + 1) as f64;
        let spacing = self.fb.p.raster.spacing();
        Some(SignalInfo {
            bandwidth_hz: used * spacing,
            symbol_rate_hz: Some(used * spacing * self.fb.carrier_power_ratio()),
            bit_rate_hz: None,
            power: 1.0,
        })
    }

    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
