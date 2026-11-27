//! HD Radio (NRSC-5): hybrid FM (MP1, MP3), hybrid AM (MA1) and all-digital AM (MA3), with the receiver graph, the station and
//! the generator of the clean test signal.
//!
//! Standard: NRSC-5-D.

pub mod l1;
pub mod l2;
pub mod results;
pub mod sound;

use crate::component::ClearCommand;
use l1::{AmFrames, Band, BandPlan, Cell, CellPlan, Channel, Px1, ServiceMode, NSYM};
use l2::{
    despread, hdlc_frame, lot_fragments, match_pci, parse_pdu, parse_sig, pci, psd_port, sig_bytes,
    spread, AasPacket, AasReceiver, FixedConfig, FixedReader, FixedWriter, LotCollector, Pdu, Psd,
    SigComponent, SigService, Sis, SisReceiver, SisTransmitter, SubChannel,
};
use results as res;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::ofdm::{FrameDemodulator, FrameModulator, FrameState};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::media::Codec;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, EventDecl, EventDef,
    GenConfig, Generator, LockState, Owner, Receiver, Record, Results, ResultsDecl, Schema,
    SeriesId, Setting, SettingError, SettingId, SignalInfo, StackConfig, StateBlob, StateError,
    Status, Value, Values,
};
use sdrkit_graph::components::{
    DeemphasisComponent, FirFilter, FmDemod, RealToAudio, Resample,
};
use sdrkit_graph::{
    no_values, Applied, Catalog, ChannelsFrom, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Kept, Kind, Picture, Port, RateFrom, RateRule, StackGraph, Timed,
};
use sdrkit_proto_bcdata::publish::Publisher;
use sdrkit_proto_bcdata::tags::TagSet;
use sdrkit_proto_mpegts::publish::{SERVICE_ROW, SERVICE_SELECT_KIND};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "hdradio";

/// A port for typed items, which have no rate.
fn item_out<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// FM minimum input rate.
pub const MIN_RATE_FM: f64 = 1.5e6;
/// AM minimum input rate.
pub const MIN_RATE_AM: f64 = 200e3;

// ---------------------------------------------------------------------------------------------------------------------------
// Test content

/// One program of the test station.
#[derive(Clone, Debug)]
pub struct Program {
    /// Number 0..7 (HD1 = 0).
    pub number: u8,
    /// Program type.
    pub kind: u8,
    /// SIG name.
    pub name: String,
    /// PSD.
    pub psd: Psd,
}

/// The test station.
#[derive(Clone, Debug)]
pub struct Station {
    /// Station information.
    pub sis: Sis,
    /// Programs.
    pub programs: Vec<Program>,
    /// The album art (LOT 1 on port 0x1000).
    pub art: Vec<u8>,
}

/// The album art port.
pub const ART_PORT: u16 = 0x1000;
/// The traffic port.
pub const TRAFFIC_PORT: u16 = 0x1100;

/// The 64 x 64 PNG album art of the test signal: a checker of 16 colours.
pub fn test_art() -> Vec<u8> {
    let pal: Vec<[u8; 3]> = (0..16usize)
        .map(|i| [(i * 16) as u8, (255 - i * 16) as u8, (i * 37 % 256) as u8])
        .collect();
    let px: Vec<u8> = (0..64 * 64)
        .flat_map(|i| pal[((i % 64) / 8 + (i / 64) / 8) % 16])
        .collect();
    sdrkit_codec_image::Raster::from_rgb(64, 64, &px)
        .and_then(|r| sdrkit_codec_image::png::encode(&r))
        .unwrap_or_default()
}

impl Station {
    /// The test station of a service mode.
    pub fn test(mode: ServiceMode) -> Station {
        let psd = |t: &str, a: &str, al: &str, g: &str, art: bool| Psd {
            title: t.into(),
            artist: a.into(),
            album: al.into(),
            genre: g.into(),
            comment: String::new(),
            ufid: String::new(),
            commercial: None,
            xhdr: art.then_some((1, 0xBE4B_7536)),
        };
        let mut programs = vec![
            Program {
                number: 0,
                kind: 5,
                name: "Test HD".into(),
                psd: psd(
                    "Test Pattern",
                    "The Test Band",
                    "Signals Vol. 1",
                    "Rock",
                    true,
                ),
            },
            Program {
                number: 1,
                kind: 14,
                name: "Test HD2".into(),
                psd: psd("Second Program", "Test Quartet", "Data Only", "Jazz", false),
            },
        ];
        if mode == ServiceMode::Mp3 {
            programs.push(Program {
                number: 2,
                kind: 9,
                name: "Test HD3".into(),
                psd: psd(
                    "Third Program",
                    "Test Trio",
                    "Extended Partitions",
                    "Pop",
                    false,
                ),
            });
        }
        Station {
            sis: Sis {
                short_name: "WTST".into(),
                fm_suffix: mode.band() == Band::Fm,
                // A short name is sent, so no universal short name (1020s clause 5.2), and the long name repeats the slogan
                // (1020s clause 4.2.2).
                universal_name: String::new(),
                long_name: "Station data you can see".into(),
                slogan: "Station data you can see".into(),
                message: "This is a test signal. HD Radio sound uses the HDC codec.".into(),
                message_priority: false,
                country: "US".into(),
                facility_id: 54321,
                lat: 40.75,
                lon: -73.99,
                alt_m: 96,
                audio_services: programs
                    .iter()
                    .map(|p| (p.number, p.kind, false, 0))
                    .collect(),
                data_services: vec![(1, 0x2D7, false)],
                params: [(0, l2::DEFAULT_LEAP_SECONDS), (1, 0), (2, 0)].into(),
                time_locked: false,
                alfn: None,
            },
            programs,
            art: test_art(),
        }
    }
    /// The SIG of the station.
    pub fn sig(&self) -> Vec<SigService> {
        let mut v: Vec<SigService> = self
            .programs
            .iter()
            .map(|p| SigService {
                data: false,
                number: p.number as u16 + 1,
                name: p.name.clone(),
                components: {
                    let mut c = vec![SigComponent {
                        data: false,
                        id: 0,
                        port: p.number as u16,
                        kind: 0,
                        mime: 0x4DC6_6C5A,
                        sdt: 0,
                    }];
                    if p.number == 0 {
                        c.push(SigComponent {
                            data: true,
                            id: 1,
                            port: ART_PORT,
                            kind: 3,
                            mime: 0xBE4B_7536,
                            sdt: 0,
                        });
                    }
                    c
                },
            })
            .collect();
        v.push(SigService {
            data: true,
            number: 0x101,
            name: "Test Traffic".into(),
            components: vec![SigComponent {
                data: true,
                id: 0,
                port: TRAFFIC_PORT,
                kind: 1,
                mime: 0xFF84_22D7,
                sdt: 0,
            }],
        });
        v
    }
}

/// The filler bytes of an audio packet (HDC is not encoded: sdrkit has no HDC encoder).
pub fn audio_packet(program: u8, n: u64, len: usize) -> Vec<u8> {
    let mut p = vec![program];
    p.extend_from_slice(&(n as u32).to_be_bytes());
    let mut x = (n as u32).wrapping_mul(2_654_435_761) ^ program as u32;
    while p.len() < len {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        p.push(x as u8);
    }
    p.truncate(len);
    p
}

/// The fixed data bytes of each FM P1 PDU (sdrkit's own allocation).
const FM_P1_FIXED: u16 = 2400;
/// The fixed data bytes of each AM P1 PDU (sdrkit's own allocation).
const AM_P1_FIXED: u16 = 100;
/// The CCC width of the test station: one whole message of one sub-channel in each PDU (1019s table 7-4).
const CCC_WIDTH: usize = 8;
/// The maximum packet length indicator of the test station: 2048 bytes in sub-channel 0 (1019s table 7-5).
const MPL: u8 = 0b01;

/// A fixed bearer of one sub-channel without FEC (1019s clause 7.5).
fn fixed_writer(len: usize) -> FixedWriter {
    let cfg = FixedConfig {
        mpl: MPL,
        subs: vec![SubChannel {
            parity: 0,
            depth: 0,
            len: len as u16,
        }],
    };
    FixedWriter::new(cfg, CCC_WIDTH).unwrap_or_else(|| unreachable!("an allowed CCC width"))
}

/// Builds the Layer 2 PDUs of the test station: the audio PDUs from byte 0 and the fixed bearer at the end (1014s clause 5.3),
/// with the PCI spread over the transfer frame (1014s clause 5.2.1).
pub struct L2Builder {
    station: Station,
    mode: ServiceMode,
    psd_streams: Vec<VecDeque<u8>>,
    fixed: VecDeque<u8>,
    p1_fixed: FixedWriter,
    p3_fixed: FixedWriter,
    port_seq: BTreeMap<u16, u16>,
    pkt_n: Vec<u64>,
    pdu_seq: Vec<u8>,
}

impl L2Builder {
    /// A builder for the station.
    pub fn new(station: Station, mode: ServiceMode) -> L2Builder {
        let n = station.programs.len();
        let fm = mode.band() == Band::Fm;
        let p3_len = match mode {
            ServiceMode::Ma1 | ServiceMode::Ma3 => {
                l2::payload_bytes(Channel::am_p3(mode).info()) - CCC_WIDTH - 1
            }
            _ => 0,
        };
        L2Builder {
            station,
            mode,
            psd_streams: vec![VecDeque::new(); n],
            fixed: VecDeque::new(),
            p1_fixed: fixed_writer(if fm { FM_P1_FIXED } else { AM_P1_FIXED } as usize),
            p3_fixed: fixed_writer(p3_len),
            port_seq: BTreeMap::new(),
            pkt_n: vec![0; n],
            pdu_seq: vec![0; n],
        }
    }
    /// The next sequence number of a port (1019s clause 5.2).
    fn seq(&mut self, port: u16) -> u16 {
        let s = self.port_seq.entry(port).or_insert(0);
        let v = *s;
        *s = s.wrapping_add(1);
        v
    }
    fn refill_fixed(&mut self) {
        let sig = sig_bytes(&self.station.sig());
        let seq = self.seq(l2::SIG_PORT);
        self.fixed.extend(hdlc_frame(l2::SIG_PORT, seq, &sig));
        for frag in lot_fragments(1, "logo.png", 0xBE4B_7536, 61_400, &self.station.art, 200) {
            let seq = self.seq(ART_PORT);
            self.fixed.extend(hdlc_frame(ART_PORT, seq, &frag));
        }
    }
    fn psd_chunk(&mut self, prog: usize, n: usize) -> Vec<u8> {
        while self.psd_streams[prog].len() < n {
            let p = &self.station.programs[prog];
            let (tag, port) = (p.psd.to_id3(), psd_port(p.number));
            let seq = self.seq(port);
            self.psd_streams[prog].extend(hdlc_frame(port, seq, &tag));
        }
        self.psd_streams[prog].drain(..n).collect()
    }
    /// The audio PDU of program `prog` in `budget` bytes: `packets` packets, `psd` PSD bytes, codec mode `codec_mode` (1017s
    /// clause 5.2.1). The blend control of the main program is BLEND ENABLE on a hybrid waveform and BLEND DISABLE on MA3, and
    /// 0b00 for the other programs (1017s clause 5.2.1.2).
    fn pdu(
        &mut self,
        prog: usize,
        budget: usize,
        packets: usize,
        psd: usize,
        codec_mode: u8,
    ) -> Vec<u8> {
        let Some(cs) = l2::codec_stream(codec_mode, 0) else {
            return Vec::new();
        };
        let p = self.station.programs[prog].clone();
        let head = l2::FIXED_HEADER_LEN + (packets * cs.lc as usize).div_ceil(8) + 3;
        let psd = self.psd_chunk(prog, psd);
        let room = budget.saturating_sub(head + psd.len());
        let per = room / packets;
        let mut pkts = Vec::with_capacity(packets);
        let start_seq = (self.pkt_n[prog] % 64) as u8;
        for i in 0..packets {
            let len = if i + 1 == packets {
                room - per * (packets - 1)
            } else {
                per
            };
            pkts.push(audio_packet(
                p.number,
                self.pkt_n[prog],
                len.saturating_sub(1),
            ));
            self.pkt_n[prog] += 1;
        }
        let seq = self.pdu_seq[prog];
        self.pdu_seq[prog] = (seq + 1) % (cs.seq_max + 1);
        Pdu {
            codec_mode,
            stream: 0,
            seq,
            blend: match (p.number, self.mode) {
                (0, ServiceMode::Ma3) => 0b00,
                (0, _) => 0b10,
                _ => 0b00,
            },
            latency: 2,
            start_seq,
            program: p.number,
            program_type: p.kind,
            psd,
            packets: pkts,
            ..Default::default()
        }
        .to_bytes()
        .unwrap_or_default()
    }
    /// The bits of a P1 transfer frame of `l` bits. FM: one PDU of each of two programs, codec mode 0b0000 (32 packets, 128 PSD
    /// bytes). AM: one PDU of each of two programs a block, codec mode 0b0010 core (4 packets, 16 PSD bytes) (1017s tables 5-2
    /// and 5-9). Then the fixed data.
    pub fn p1(&mut self, l: usize) -> Vec<u8> {
        let fm = self.mode.band() == Band::Fm;
        let (packets, codec, psd) = if fm {
            (32, 0b0000, 128)
        } else {
            (4, 0b0010, 16)
        };
        let n = l2::payload_bytes(l);
        let audio = n - self.p1_fixed.region_len();
        let progs = 2;
        let mut b = Vec::with_capacity(n);
        for p in 0..progs {
            let budget = if p + 1 == progs {
                audio - b.len()
            } else {
                audio / progs
            };
            let pdu = self.pdu(p, budget, packets, psd, codec);
            b.extend_from_slice(&pdu);
        }
        b.resize(audio, 0);
        let f = self.p1_fixed.config().streams_len();
        while self.fixed.len() < f {
            self.refill_fixed();
        }
        let stream: Vec<u8> = self.fixed.drain(..f).collect();
        self.p1_fixed.region(&[&stream], &mut b);
        let mut bits = Vec::with_capacity(l);
        spread(pci::AUDIO_FIXED, &b, l, &mut bits);
        bits
    }
    /// The bits of a P3 transfer frame of `l` bits. MP3: one PDU of HD3, codec mode 0b1101 (4 packets, 16 PSD bytes). AM: fixed
    /// data only, whole SIG packets and then flags.
    pub fn p3(&mut self, l: usize) -> Vec<u8> {
        let n = l2::payload_bytes(l);
        let mut b = Vec::with_capacity(n);
        let cw = if self.mode == ServiceMode::Mp3 {
            let pdu = self.pdu(2, n, 4, 16, 0b1101);
            b.extend_from_slice(&pdu);
            b.resize(n, 0);
            pci::AUDIO
        } else {
            let f = self.p3_fixed.config().streams_len();
            let seq = self.seq(l2::SIG_PORT);
            let sig = hdlc_frame(l2::SIG_PORT, seq, &sig_bytes(&self.station.sig()));
            let mut fx = Vec::with_capacity(f);
            while fx.len() + sig.len() <= f {
                fx.extend_from_slice(&sig);
            }
            self.p3_fixed.region(&[&fx], &mut b);
            pci::FIXED
        };
        let mut bits = Vec::with_capacity(l);
        spread(cw, &b, l, &mut bits);
        bits
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Transmitter

/// The power level control of the AM test station: the low level (1012s clause 6.3.1).
const GEN_PL: bool = false;
/// The high-power PIDS control of the AM test station: on, so that the station information is read at the SNR of the primary
/// sidebands (1012s clause 6.3.3).
const GEN_HPP: bool = true;

/// The HD Radio transmitter (digital part) at the band's OFDM rate.
pub struct HdTransmitter {
    mode: ServiceMode,
    bp: BandPlan,
    cells: CellPlan,
    modu: FrameModulator,
    p1: Channel,
    pids: Channel,
    p3: Option<Channel>,
    l2: L2Builder,
    sis: SisTransmitter,
    frame: u64,
    /// AM: the interleaving, and the coded P1 (and MA3 P3) of the last frames for the diversity delay.
    am: Option<AmFrames>,
    am_p1_history: VecDeque<Vec<u8>>,
    am_p3_history: VecDeque<Vec<u8>>,
    /// MP3: the PX1 interleaver, its internal matrix and the cycle position.
    px1: Px1,
    px1_matrix: Vec<u8>,
    px1_at: usize,
    coded: Vec<u8>,
}

impl HdTransmitter {
    /// A transmitter of the test station.
    pub fn new(mode: ServiceMode) -> HdTransmitter {
        let bp = l1::mode_plan(mode);
        let station = Station::test(mode);
        let fm = mode.band() == Band::Fm;
        let sis = SisTransmitter::new(&station.sis, fm, 1);
        HdTransmitter {
            cells: l1::cells(mode, GEN_PL, GEN_HPP),
            modu: FrameModulator::new(bp.layout.clone()),
            p1: if fm {
                Channel::fm_p1()
            } else {
                Channel::am_p1()
            },
            pids: if fm {
                Channel::fm_pids()
            } else {
                Channel::am_pids()
            },
            p3: match mode {
                ServiceMode::Mp1 => None,
                ServiceMode::Mp3 => Some(Channel::fm_p3()),
                m => Some(Channel::am_p3(m)),
            },
            l2: L2Builder::new(station, mode),
            sis,
            frame: 0,
            am: (!fm).then(|| AmFrames::new(mode)),
            am_p1_history: VecDeque::new(),
            am_p3_history: VecDeque::new(),
            px1: if mode == ServiceMode::Mp3 {
                Px1::new()
            } else {
                Px1::default()
            },
            px1_matrix: if mode == ServiceMode::Mp3 {
                vec![0; Px1::N]
            } else {
                Vec::new()
            },
            px1_at: 0,
            coded: Vec::new(),
            bp,
            mode,
        }
    }
    /// The service mode.
    pub fn mode(&self) -> ServiceMode {
        self.mode
    }
    /// The OFDM rate.
    pub fn rate(&self) -> f64 {
        self.bp.layout.rate
    }
    /// The SIS PDU of block `block` of this L1 frame. The frame number of the test signal counts from 0.
    fn next_pids(&mut self, block: usize) -> Vec<u8> {
        self.sis.pdu(block, self.frame as u32)
    }
    /// The coded bits of the next transfer frame of P1 (`p3` false) or P3, from Layer 2.
    fn coded_frame(&mut self, p3: bool) -> Vec<u8> {
        let ch = if p3 {
            self.p3.as_mut()
        } else {
            Some(&mut self.p1)
        };
        let Some(ch) = ch else {
            return Vec::new();
        };
        let l = ch.info();
        let bits = if p3 { self.l2.p3(l) } else { self.l2.p1(l) };
        let ch = if p3 {
            self.p3.as_mut()
        } else {
            Some(&mut self.p1)
        };
        let mut out = Vec::new();
        if let Some(ch) = ch {
            ch.encode(&bits, &mut out);
        }
        out
    }
    /// Passes one P3 transfer frame through the PX1 interleaver (1011s clause 10.2.2.2): each input bit is written to its place
    /// in the internal matrix, and one bit is read row by row for each bit written.
    fn px1(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &x in input {
            let i = self.px1_at;
            self.px1_matrix[self.px1.write_position(i)] = x;
            out.push(self.px1_matrix[i]);
            self.px1_at = (i + 1) % Px1::N;
        }
        out
    }
    /// Appends one L1 frame of samples (digital only, unit power per active subcarrier scaled by the modulator).
    pub fn next_frame(&mut self, out: &mut Vec<Complex32>) {
        let fm = self.mode.band() == Band::Fm;
        let blocks = self.mode.blocks();
        let k = self.bp.layout.ncarriers();
        // The bits of each block's P1 (FM: with PIDS), PIDS (AM) and P3 cells.
        let mut p1_blocks = vec![0u8; blocks * l1::cell_bits(&self.cells.p1)];
        let mut pids_blocks: Vec<Vec<u8>> = vec![Vec::new(); blocks];
        let mut p3_blocks: Vec<u8> = Vec::new();
        if fm {
            let p1 = self.coded_frame(false);
            for (i, &x) in p1.iter().enumerate() {
                p1_blocks[l1::pm_p1_position(i)] = x;
            }
            for (b, pb) in pids_blocks.iter_mut().enumerate() {
                let f = self.next_pids(b);
                self.pids.encode(&f, &mut self.coded);
                for (n, &x) in self.coded.iter().enumerate() {
                    p1_blocks[b * l1::FM_BLOCK_BITS + l1::pm_pids_position(b, n)] = x;
                }
                pb.clear();
            }
            if self.mode == ServiceMode::Mp3 {
                for _ in 0..blocks / 2 {
                    let c = self.coded_frame(true);
                    let x = self.px1(&c);
                    p3_blocks.extend_from_slice(&x);
                }
            }
        } else if let Some(am) = self.am.clone() {
            let mut now = Vec::with_capacity(72_000);
            for _ in 0..blocks {
                now.extend(self.coded_frame(false));
            }
            let before = if self.am_p1_history.len() >= l1::AM_DIVERSITY_FRAMES {
                self.am_p1_history.pop_front().unwrap_or_default()
            } else {
                Vec::new()
            };
            am.p1_frame(&now, &before, &mut p1_blocks);
            self.am_p1_history.push_back(now);
            for (b, pb) in pids_blocks.iter_mut().enumerate() {
                let f = self.next_pids(b);
                self.pids.encode(&f, &mut self.coded);
                pb.resize(l1::AM_PIDS_BLOCK, 0);
                am.pids_block(&self.coded, pb);
            }
            let p3 = self.coded_frame(true);
            let before = if self.mode == ServiceMode::Ma3
                && self.am_p3_history.len() >= l1::AM_DIVERSITY_FRAMES
            {
                self.am_p3_history.pop_front().unwrap_or_default()
            } else {
                Vec::new()
            };
            p3_blocks.resize(8 * am.p3_block(), 0);
            am.p3_frame(&p3, &before, &mut p3_blocks);
            if self.mode == ServiceMode::Ma3 {
                self.am_p3_history.push_back(p3);
            }
        }
        let p1_per = p1_blocks.len() / blocks;
        let p3_per = p3_blocks.len() / blocks;
        for b in 0..blocks {
            let mut grid = vec![Complex32::new(0.0, 0.0); NSYM * k];
            if let Some(psmi) = self.mode.psmi() {
                // The system control data sequence R of each pilot column (1011s clause 11). MP1 sends no pilot on the
                // extended partitions (1011s table 12-24).
                let ctrl = l1::FmControl::standard(b as u8, psmi);
                for &r in &self.bp.refs {
                    if self.mode == ServiceMode::Mp1 && r.abs() < 356 {
                        continue;
                    }
                    let big_r =
                        l1::differential(ctrl.sequence(l1::fm_pilot_column(r).unwrap_or(0)));
                    for s in 0..NSYM {
                        let i = s * k + (r - self.bp.layout.kmin) as usize;
                        grid[i] = l1::fm_pilot_cell(big_r >> (31 - s));
                    }
                }
            } else {
                // The AM system control data sequence on subcarrier 1 and its negated conjugate on -1 (1012s clauses 11 and
                // 12.2.3).
                let ctrl = l1::AmControl {
                    pli: GEN_PL,
                    hppi: GEN_HPP,
                    ..l1::AmControl::standard(b as u8, self.mode)
                }
                .sequence();
                let lv = l1::am_levels(self.mode, GEN_PL, GEN_HPP);
                let kmin = self.bp.layout.kmin;
                for s in 0..NSYM {
                    let v = l1::am_pilot_cell(ctrl >> (31 - s)) * lv.gain(lv.pilot);
                    grid[s * k + (1 - kmin) as usize] = v;
                    grid[s * k + (-1 - kmin) as usize] = -v.conj();
                }
            }
            l1::map_bits(
                &self.cells.p1,
                &p1_blocks[b * p1_per..(b + 1) * p1_per],
                &mut grid,
            );
            if !fm {
                l1::map_bits(&self.cells.pids, &pids_blocks[b], &mut grid);
            }
            if p3_per > 0 {
                l1::map_bits(
                    &self.cells.p3,
                    &p3_blocks[b * p3_per..(b + 1) * p3_per],
                    &mut grid,
                );
            }
            self.modu.frame(&grid, out);
        }
        self.frame += 1;
    }
}

sdrkit_core::choice_enum! {
    /// The signal of the test signal.
    #[derive(Default)]
    pub enum Waveform {
        /// FM hybrid MP1.
        #[default]
        Mp1 = ("mp1", "FM hybrid MP1"),
        /// AM hybrid MA1.
        Ma1 = ("ma1", "AM hybrid MA1"),
        /// FM extended hybrid MP3.
        Mp3 = ("mp3", "FM extended hybrid MP3"),
        /// AM all digital MA3.
        Ma3 = ("ma3", "AM all-digital MA3"),
    }
}

impl Waveform {
    /// The service mode.
    pub fn mode(self) -> ServiceMode {
        match self {
            Waveform::Mp1 => ServiceMode::Mp1,
            Waveform::Mp3 => ServiceMode::Mp3,
            Waveform::Ma1 => ServiceMode::Ma1,
            Waveform::Ma3 => ServiceMode::Ma3,
        }
    }
}

/// The generator's options. Noise, offsets and the channel come from the framework's `gen_*` settings.
#[derive(Clone, Debug)]
pub struct GenOptions {
    /// Waveform.
    pub waveform: Waveform,
    /// Analogue host level against nominal, dB.
    pub analog_db: f64,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            waveform: Waveform::Mp1,
            analog_db: 0.0,
        }
    }
}

/// The HD Radio test signal: the digital sidebands and the analogue host, in the RF orientation, resampled to the requested
/// rate.
pub struct HdGenerator {
    tx: HdTransmitter,
    o: GenOptions,
    out_rate: f64,
    rs: Option<Resampler<Complex32>>,
    pending: VecDeque<Complex32>,
    n: u64,
    analog_phase: f64,
    frame: Vec<Complex32>,
    tmp: Vec<Complex32>,
    sc_power: f32,
    /// The mean power of the digital sidebands.
    digital: f32,
}

impl HdGenerator {
    /// A generator at `out_rate`; `None` when the rate cannot be reached.
    pub fn new(o: GenOptions, out_rate: f64) -> Option<HdGenerator> {
        let tx = HdTransmitter::new(o.waveform.mode());
        let rate = tx.rate();
        let active = (tx.bp.layout.ncarriers() - tx.bp.layout.null.len()) as f32;
        // The modulator scales a frame of unit cells to unit power: one subcarrier has 1 / active of it.
        let sc_power = 1.0 / active;
        // FM: every cell has unit level. AM: the levels of 1082s table 4-6, against a primary cell of unit level.
        let digital = if tx.mode.band() == Band::Fm {
            1.0
        } else {
            let c = &tx.cells;
            let cells: f32 =
                c.p1.iter()
                    .chain(&c.p3)
                    .chain(&c.pids)
                    .map(|x| x.gain * x.gain * if x.twin.is_some() { 2.0 } else { 1.0 })
                    .sum();
            let lv = l1::am_levels(tx.mode, GEN_PL, GEN_HPP);
            let pilots = 2.0 * NSYM as f32 * lv.gain(lv.pilot).powi(2);
            (cells + pilots) * sc_power / NSYM as f32
        };
        let rs = if (out_rate - rate).abs() > 1e-6 {
            Some(Resampler::with_quality(rate, out_rate, 0.45, 70.0).ok()?)
        } else {
            None
        };
        Some(HdGenerator {
            rs,
            tx,
            out_rate,
            pending: VecDeque::new(),
            n: 0,
            analog_phase: 0.0,
            frame: Vec::new(),
            tmp: Vec::new(),
            sc_power,
            digital,
            o,
        })
    }
    /// The options of a generator configuration (the mode's `gen_*` test signal options).
    pub fn options(cfg: &GenConfig) -> GenOptions {
        let s = &cfg.settings;
        GenOptions {
            waveform: s.get(&GEN_WAVEFORM),
            analog_db: s.get(&GEN_ANALOG_DB),
        }
    }
    /// The service mode.
    pub fn mode(&self) -> ServiceMode {
        self.tx.mode()
    }
    /// The power of the AM carrier: its level against a primary subcarrier is that of 1082s table 4-6 (30 dB in MA1, 15 dB in
    /// MA3).
    fn carrier_power(&self) -> f64 {
        let lv = l1::am_levels(self.tx.mode(), GEN_PL, GEN_HPP);
        self.sc_power as f64 * 10f64.powf(-lv.primary as f64 / 10.0)
    }
    /// The power of the analogue host (the digital sidebands of FM have unit power).
    fn analog_power(&self) -> f64 {
        let a2 = 10f64.powf(self.o.analog_db / 10.0);
        match self.tx.mode() {
            ServiceMode::Mp1 | ServiceMode::Mp3 => 100.0 * a2,
            ServiceMode::Ma1 => self.carrier_power() * a2 * 1.125,
            ServiceMode::Ma3 => self.carrier_power(),
        }
    }
    fn add_analog(&mut self) {
        let rate = self.tx.rate();
        let mode = self.tx.mode();
        let digital = self.digital;
        let carrier = self.carrier_power().sqrt() as f32;
        let a = 10f32.powf(self.o.analog_db as f32 / 20.0);
        let tone = 6.0 * self.tx.bp.layout.spacing();
        for s in self.frame.iter_mut() {
            let t = self.n as f64 / rate;
            let v = match mode {
                // The digital sidebands sit 20 dB below the FM host: a 1 kHz tone at 75 kHz deviation.
                ServiceMode::Mp1 | ServiceMode::Mp3 => {
                    let amp = (digital * 100.0).sqrt() * a;
                    self.analog_phase +=
                        std::f64::consts::TAU * 75e3 * (std::f64::consts::TAU * 1e3 * t).cos()
                            / rate;
                    Complex32::from_polar(amp, self.analog_phase as f32)
                }
                // The tone is on subcarrier 6 (about 1.09 kHz): whole periods in each symbol, so it does not leak into the
                // digital subcarriers.
                ServiceMode::Ma1 => {
                    let c = carrier * a;
                    Complex32::new(
                        c * (1.0 + 0.5 * (std::f64::consts::TAU * tone * t).sin() as f32),
                        0.0,
                    )
                }
                ServiceMode::Ma3 => Complex32::new(carrier, 0.0),
            };
            *s += v;
            self.n += 1;
        }
        self.analog_phase %= std::f64::consts::TAU;
    }
}

impl Generator for HdGenerator {
    fn rate_hz(&self) -> f64 {
        self.out_rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        while self.pending.len() < out.len() {
            self.frame.clear();
            self.tx.next_frame(&mut self.frame);
            self.add_analog();
            // The FM RF spectrum is the mirror of the baseband one (1011s clause 14.2.2).
            if self.tx.mode().band() == Band::Fm {
                for s in &mut self.frame {
                    *s = s.conj();
                }
            }
            match &mut self.rs {
                Some(r) => {
                    self.tmp.clear();
                    r.process(&self.frame, &mut self.tmp);
                    self.pending.extend(self.tmp.iter());
                }
                None => self.pending.extend(self.frame.iter()),
            }
        }
        for o in out.iter_mut() {
            *o = self.pending.pop_front().unwrap_or_default();
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // An SNR is stated per digital subcarrier (FM) or per primary subcarrier (AM): this is the bandwidth that corresponds
        // to it, for the whole signal's power.
        let l = &self.tx.bp.layout;
        let active = (l.ncarriers() - l.null.len()) as f64;
        let power = self.digital as f64 + self.analog_power();
        Some(SignalInfo::new(power * active * l.spacing()).with_power(power))
    }
}

/// Makes the generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    match HdGenerator::new(HdGenerator::options(cfg), cfg.rate_hz) {
        Some(g) => Ok(Box::new(g)),
        None => Err(crate::silence::NoSignal {
            rate_hz: cfg.rate_hz,
        }
        .refused()),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Layer 1 receiver

/// What a logical channel carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logical {
    /// P1.
    P1,
    /// P3.
    P3,
}

/// A decoded transfer frame.
#[derive(Clone, Debug)]
pub struct TransferFrame {
    /// P1 or P3.
    pub channel: Logical,
    /// The bits of the L2 PDU, one a byte.
    pub bits: Arc<[u8]>,
    /// The service mode.
    pub mode: ServiceMode,
    /// Channel bit error rate before the decoder.
    pub ber: f32,
}
impl Item for TransferFrame {
    const KIND: &'static str = "hdr.transfer";
}

/// A PIDS frame.
#[derive(Clone, Debug)]
pub struct PidsFrame {
    /// The 80 bits.
    pub bits: Vec<u8>,
    /// The block number in the L1 frame.
    pub block: u8,
    /// FM (else AM).
    pub fm: bool,
}
impl Item for PidsFrame {
    const KIND: &'static str = "hdr.pids";
}

/// Layer 1 status after each block.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct L1Status {
    /// 0 searching, 1 signal found, 2 block sync, 3 decoding L1 frames.
    pub state: u8,
    /// The band.
    pub band: Option<Band>,
    /// The service mode.
    pub mode: Option<ServiceMode>,
    /// Carrier offset at the input, Hz.
    pub offset_hz: f64,
    /// MER, dB (FM lower, upper; AM primary in both).
    pub mer_db: (f32, f32),
    /// Channel BER of P1.
    pub ber: f32,
    /// Blocks.
    pub blocks: u64,
    /// Block syncs.
    pub syncs: u64,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// Constellation of the last block.
    #[serde(skip)]
    pub points: Vec<[f32; 2]>,
    /// Time.
    pub time: f64,
    /// Mirrored.
    pub mirrored: bool,
}
impl Item for L1Status {
    const KIND: &'static str = "hdr.l1";
}

struct BandRx {
    band: Band,
    /// The mode of the layout (MP1 stands for both FM modes).
    mode: ServiceMode,
    /// The pilot subcarriers and the grid width, kept from the band plan (designed once).
    refs: Vec<i32>,
    kmin: i32,
    width: usize,
    demod: FrameDemodulator,
    enabled: bool,
    /// Blocks in a row whose system control data sequence was not valid.
    bad_ctrl: u32,
}

impl BandRx {
    fn new(mode: ServiceMode) -> BandRx {
        let p = l1::mode_plan(mode);
        BandRx {
            band: mode.band(),
            mode,
            kmin: p.layout.kmin,
            width: p.layout.ncarriers(),
            refs: p.refs,
            demod: FrameDemodulator::new(p.layout),
            enabled: true,
            bad_ctrl: 0,
        }
    }
}

/// The receive side of the PX1 interleaver: the LLRs read out, and the transfer frames they complete.
#[derive(Default)]
struct Px1Rx {
    /// The time (in bits of the cycle, up to two cycles) each input bit of a cycle is read out.
    read_at: Vec<u32>,
    /// The latest read time of each of the 16 transfer frames of a cycle.
    frame_done: Vec<u32>,
    /// The last two cycles of LLRs read out, by output time modulo 2N.
    ring: Vec<f32>,
    /// Output bits received since the cycle start the receiver locked to.
    received: u64,
    /// The next transfer frame to deliver, counted from that cycle start.
    next: u64,
}

impl Px1Rx {
    fn new(px: &Px1) -> Px1Rx {
        let n = Px1::N;
        let read_at: Vec<u32> = (0..n)
            .map(|j| {
                let l = px.write_position(j);
                (if l >= j { l } else { n + l }) as u32
            })
            .collect();
        let frame_done = (0..n / Px1::B)
            .map(|f| {
                read_at[f * Px1::B..(f + 1) * Px1::B]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        Px1Rx {
            read_at,
            frame_done,
            ring: vec![0.0; 2 * n],
            received: 0,
            next: 0,
        }
    }
    /// Takes the LLRs of one block pair; each transfer frame they complete goes to `out`.
    fn push(&mut self, llr: &[f32], mut out: impl FnMut(&[f32])) {
        let n = Px1::N as u64;
        for &v in llr {
            self.ring[(self.received % (2 * n)) as usize] = v;
            self.received += 1;
        }
        // Transfer frames whose bits are all read out (the first cycle's frames are not, as their writes came before the lock).
        let mut frame = vec![0.0f32; Px1::B];
        loop {
            let f = self.next;
            let cycle = f / 16;
            let fi = (f % 16) as usize;
            let done = cycle * n + self.frame_done[fi] as u64;
            if done >= self.received {
                break;
            }
            for (k, o) in frame.iter_mut().enumerate() {
                let t = cycle * n + self.read_at[fi * Px1::B + k] as u64;
                *o = self.ring[(t % (2 * n)) as usize];
            }
            out(&frame);
            self.next += 1;
        }
    }
}

/// One candidate phase of the PX1 cycle: its receive side, whether it started, and the lowest channel bit error rate of its
/// transfer frames.
struct Px1Phase {
    rx: Px1Rx,
    started: bool,
    ber: f32,
    frames: u32,
}

/// The Layer 1 receiver (`broadcast.hd_l1`, iq FM lane + iq AM lane -> typed transfer frames, PIDS, status).
pub struct HdL1 {
    rx: Vec<BandRx>,
    active: Option<usize>,
    mode: Option<ServiceMode>,
    p1_fm: Channel,
    p1_am: Channel,
    pids_fm: Channel,
    pids_am: Channel,
    p3: Option<Channel>,
    /// The LLRs of the P1 (FM: and PIDS) cells of the blocks of the frame, and of the P3 cells.
    llr_p1: Vec<f32>,
    llr_p3: Vec<f32>,
    have: Vec<bool>,
    am: Option<AmFrames>,
    /// AM: the main subframes of the last frames, waiting for their backup subframes.
    am_main: VecDeque<Vec<f32>>,
    am_p3_main: VecDeque<Vec<f32>>,
    /// MP3: the two candidate phases of the PX1 cycle, the one chosen, and the whole L1 frames seen since the mode was set.
    px1: Vec<Px1Phase>,
    px1_pick: Option<usize>,
    frames_seen: u64,
    status: L1Status,
    soft: Vec<f32>,
    scratch: Vec<Complex32>,
    /// The cells of the service mode, made when the mode or the AM power level changes, and the same cells on a mirrored
    /// spectrum.
    cells: Arc<CellPlan>,
    cells_mirrored: Arc<CellPlan>,
    /// AM: the power level and high-power PIDS indicators the cells follow.
    power: (bool, bool),
    /// AM: whether the spectrum is mirrored, once decided. A mirrored AM signal fits the known cells as well as the true one
    /// (only the sidebands swap), so the first P1 transfer frame is decoded both ways and the lower channel bit error rate
    /// decides.
    am_mirrored: Option<bool>,
    /// AM, while undecided: the LLRs of the P1 cells read as mirrored, and their main subframes.
    llr_p1_mirrored: Vec<f32>,
    am_main_mirrored: VecDeque<Vec<f32>>,
}

impl Default for HdL1 {
    fn default() -> Self {
        HdL1 {
            rx: vec![
                BandRx::new(ServiceMode::Mp1),
                BandRx::new(ServiceMode::Ma1),
                BandRx::new(ServiceMode::Ma3),
            ],
            active: None,
            mode: None,
            p1_fm: Channel::fm_p1(),
            p1_am: Channel::am_p1(),
            pids_fm: Channel::fm_pids(),
            pids_am: Channel::am_pids(),
            p3: None,
            llr_p1: Vec::new(),
            llr_p3: Vec::new(),
            have: Vec::new(),
            am: None,
            am_main: VecDeque::new(),
            am_p3_main: VecDeque::new(),
            px1: Vec::new(),
            px1_pick: None,
            frames_seen: 0,
            status: L1Status::default(),
            soft: Vec::new(),
            scratch: Vec::new(),
            cells: Arc::new(l1::cells(ServiceMode::Mp1, false, false)),
            cells_mirrored: Arc::new(l1::cells(ServiceMode::Mp1, false, false)),
            power: (false, false),
            am_mirrored: None,
            llr_p1_mirrored: Vec::new(),
            am_main_mirrored: VecDeque::new(),
        }
    }
}

impl HdL1 {
    fn reset(&mut self) {
        for r in &mut self.rx {
            r.demod.reset();
        }
        self.active = None;
        self.mode = None;
        self.have.clear();
        self.am_main.clear();
        self.am_p3_main.clear();
        self.am_main_mirrored.clear();
        self.am_mirrored = None;
        self.px1.clear();
        self.px1_pick = None;
        self.frames_seen = 0;
        for r in &mut self.rx {
            r.bad_ctrl = 0;
        }
        self.status.state = 0;
    }
    fn set_mode(&mut self, mode: ServiceMode, power: (bool, bool)) {
        self.mode = Some(mode);
        self.power = power;
        self.am_mirrored = None;
        self.am_main_mirrored.clear();
        self.p3 = match mode {
            ServiceMode::Mp1 => None,
            ServiceMode::Mp3 => Some(Channel::fm_p3()),
            m => Some(Channel::am_p3(m)),
        };
        self.am = (mode.band() == Band::Am).then(|| AmFrames::new(mode));
        self.am_main.clear();
        self.am_p3_main.clear();
        self.px1.clear();
        self.px1_pick = None;
        self.frames_seen = 0;
        if mode == ServiceMode::Mp3 {
            let px = Px1::new();
            for _ in 0..2 {
                self.px1.push(Px1Phase {
                    rx: Px1Rx::new(&px),
                    started: false,
                    ber: f32::INFINITY,
                    frames: 0,
                });
            }
        }
        self.have = vec![false; mode.blocks()];
        let cells = l1::cells(mode, power.0, power.1);
        self.llr_p1 = vec![0.0; mode.blocks() * l1::cell_bits(&cells.p1)];
        self.llr_p1_mirrored = self.llr_p1.clone();
        self.llr_p3 = vec![0.0; mode.blocks() * l1::cell_bits(&cells.p3)];
        let p = l1::mode_plan(mode);
        self.cells_mirrored = Arc::new(cells.mirrored(p.layout.ncarriers(), p.layout.kmin));
        self.cells = Arc::new(cells);
    }
}

/// Decodes one transfer frame's LLRs with `ch` into packed bytes and the channel bit error rate.
fn decode_frame(ch: &mut Channel, llr: &[f32]) -> Option<(Vec<u8>, f32)> {
    let mut bits = Vec::new();
    let ber = ch.decode(llr, &mut bits)?;
    Some((bits, ber))
}

/// Reads the FM system control data sequence of a block (1011s clause 11) from the equalised cells of the MP1 pilot
/// subcarriers: each bit of r by differential detection, summed over the subcarriers. Rows 10, 11 and 13 differ between columns
/// and stay 0. It returns the block count and the mode, or `None` when a sync or parity bit is wrong or the mode is not one
/// sdrkit decodes.
fn read_fm_control(
    z: &[Complex32],
    w: &[f32],
    refs: &[i32],
    kmin: i32,
    k: usize,
) -> Option<(u8, ServiceMode)> {
    let mut r = 0u32;
    for row in 0..NSYM {
        if matches!(row, 10 | 11 | 13) {
            continue;
        }
        let mut acc = 0.0f32;
        for &c in refs.iter().filter(|c| c.abs() >= 356) {
            let i = row * k + (c - kmin) as usize;
            acc += if row == 0 {
                (z[i].re + z[i].im) * w[i].min(1e4)
            } else {
                -(z[i] * z[i - k].conj()).re * w[i].min(w[i - k]).min(1e4)
            };
        }
        if acc.abs() < 1e-9 {
            return None;
        }
        r |= ((acc > 0.0) as u32) << (31 - row);
    }
    let c = l1::FmControl::parse(r)?;
    if c.psmi != 0 && (!c.asm0 || c.asm1) {
        return None;
    }
    Some((c.block, ServiceMode::from_psmi(c.psmi)?))
}

/// Reads the AM system control data sequence of a block (1012s clause 11) from the equalised cells of the pilot subcarriers:
/// each bit of R from subcarrier 1 and the negated conjugate on -1. It returns the fields, or `None` when a sync or parity bit
/// is wrong.
fn read_am_control(z: &[Complex32], w: &[f32], kmin: i32, k: usize) -> Option<l1::AmControl> {
    let mut r = 0u32;
    for row in 0..NSYM {
        let (i, j) = (
            row * k + (1 - kmin) as usize,
            row * k + (-1 - kmin) as usize,
        );
        let acc = z[i].im * w[i].min(1e4) + (-z[j].conj()).im * w[j].min(1e4);
        if acc.abs() < 1e-9 {
            return None;
        }
        r |= ((acc > 0.0) as u32) << (31 - row);
    }
    l1::AmControl::parse(r)
}

fn mer(cells: &[Cell], z: &[Complex32], w: &[f32]) -> f32 {
    let (mut e, mut s) = (0.0f32, 0.0f32);
    for c in cells {
        let v = c.value(z, w).0;
        let n = c.qam.nearest(v);
        e += (v - n).norm_sqr();
        s += n.norm_sqr();
    }
    10.0 * (s / e.max(1e-12)).log10()
}

impl Component for HdL1 {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.hd_l1",
            vec![
                Port::iq("fm").rate(RateRule::Exact(l1::FM_RATE)),
                Port::iq("am").rate(RateRule::Exact(l1::AM_RATE)),
            ],
            vec![
                item_out::<TransferFrame>("transfer"),
                item_out::<PidsFrame>("pids"),
                item_out::<L1Status>("status"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        // Below 500 ksps only AM is searched.
        self.rx[0].enabled = inputs[0].bandwidth_hz >= 500e3;
        Ok(vec![
            Format::new(Kind::typed::<TransferFrame>(), 0.0),
            Format::new(Kind::typed::<PidsFrame>(), 0.0),
            Format::new(Kind::typed::<L1Status>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let fm = io.iq(0);
        let am = io.iq(1);
        let mut tf = Vec::new();
        let mut pids_out = Vec::new();
        let mut st = Vec::new();
        for ri in 0..self.rx.len() {
            if !self.rx[ri].enabled || self.active.is_some_and(|a| a != ri) {
                continue;
            }
            let band = self.rx[ri].band;
            if band == Band::Fm {
                // The FM RF spectrum is the mirror of the baseband one (1011s clause 14.2.2): work on the conjugate.
                let mut v = std::mem::take(&mut self.scratch);
                v.clear();
                v.extend(fm.iter().map(|s| s.conj()));
                self.rx[ri].demod.push(&v);
                self.scratch = v;
            } else {
                self.rx[ri].demod.push(am);
            }
            if self.status.state == 0 && self.rx[ri].demod.state() == FrameState::Tracking {
                self.status.state = 1;
            }
            while let Some(f) = self.rx[ri].demod.next_frame() {
                let (k, kmin) = (self.rx[ri].width, self.rx[ri].kmin);
                // The block count and the mode from the system control data sequence; an AM layout takes only its own mode.
                let control = if band == Band::Fm {
                    read_fm_control(&f.z, &f.w, &self.rx[ri].refs, kmin, k)
                        .map(|(b, m)| (b, m, (false, false)))
                } else {
                    read_am_control(&f.z, &f.w, kmin, k).and_then(|c| {
                        let m = ServiceMode::from_smi(c.smi)?;
                        let power = (c.pli && m == ServiceMode::Ma1, c.hppi);
                        (m == self.rx[ri].mode).then_some((c.block, m, power))
                    })
                };
                let Some((block, mode, power)) =
                    control.filter(|(b, m, _)| (*b as usize) < m.blocks())
                else {
                    self.rx[ri].bad_ctrl += 1;
                    if self.rx[ri].bad_ctrl >= 4 {
                        self.rx[ri].demod.restart();
                        self.rx[ri].bad_ctrl = 0;
                        if self.active == Some(ri) {
                            self.active = None;
                            self.status.state = 0;
                        }
                    }
                    continue;
                };
                self.rx[ri].bad_ctrl = 0;
                if self.active.is_none() {
                    self.active = Some(ri);
                    self.status.syncs += 1;
                    for (j, o) in self.rx.iter_mut().enumerate() {
                        if j != ri {
                            o.demod.reset();
                        }
                    }
                }
                if self.mode != Some(mode) || self.power != power {
                    self.set_mode(mode, power);
                }
                if self.status.state < 2 {
                    self.status.state = 2;
                }
                let cells = if self.am_mirrored == Some(true) {
                    Arc::clone(&self.cells_mirrored)
                } else {
                    Arc::clone(&self.cells)
                };
                let b = block as usize;
                if b == 0 {
                    self.have.fill(false);
                }
                self.have[b] = true;
                self.status.blocks += 1;
                self.status.band = Some(band);
                self.status.mode = Some(mode);
                // The FM demodulator works on the conjugate: its offset is the negative of the one at the input.
                self.status.offset_hz = if band == Band::Fm {
                    -f.cfo_hz
                } else {
                    f.cfo_hz
                };
                self.status.level_dbfs = f.level_db;
                self.status.time = f.time;
                self.status.mirrored = f.mirrored ^ (self.am_mirrored == Some(true));
                let half = cells.p1.len() / 2;
                self.status.mer_db = if band == Band::Fm {
                    (
                        mer(&cells.p1[..half], &f.z, &f.w),
                        mer(&cells.p1[half..], &f.z, &f.w),
                    )
                } else {
                    let m = mer(&cells.p1, &f.z, &f.w);
                    (m, m)
                };
                self.status.points = cells
                    .p1
                    .iter()
                    .step_by((cells.p1.len() / 1024).max(1))
                    .take(1024)
                    .map(|c| {
                        let v = c.value(&f.z, &f.w).0;
                        [v.re, v.im]
                    })
                    .collect();
                // The LLRs of the block's cells (AM, while the mirror is undecided: also read as mirrored).
                if band == Band::Am && self.am_mirrored.is_none() {
                    l1::soft_bits(&self.cells_mirrored.p1, &f.z, &f.w, &mut self.soft);
                    let n1 = self.soft.len();
                    self.llr_p1_mirrored[b * n1..(b + 1) * n1].copy_from_slice(&self.soft);
                }
                l1::soft_bits(&cells.p1, &f.z, &f.w, &mut self.soft);
                let n1 = self.soft.len();
                self.llr_p1[b * n1..(b + 1) * n1].copy_from_slice(&self.soft);
                let mut pids_llr = Vec::with_capacity(240);
                if band == Band::Fm {
                    pids_llr.extend((0..200).map(|n| self.soft[l1::pm_pids_position(b, n)]));
                } else if let Some(am) = &self.am {
                    l1::soft_bits(&cells.pids, &f.z, &f.w, &mut self.soft);
                    pids_llr.resize(240, 0.0);
                    am.pids_unblock(&self.soft, &mut pids_llr);
                }
                let pch = if band == Band::Fm {
                    &mut self.pids_fm
                } else {
                    &mut self.pids_am
                };
                let mut pb = Vec::new();
                if pch.decode(&pids_llr, &mut pb).is_some() {
                    pids_out.push((
                        f.time,
                        PidsFrame {
                            bits: pb,
                            block,
                            fm: band == Band::Fm,
                        },
                    ));
                }
                if !cells.p3.is_empty() {
                    l1::soft_bits(&cells.p3, &f.z, &f.w, &mut self.soft);
                    let n3 = self.soft.len();
                    self.llr_p3[b * n3..(b + 1) * n3].copy_from_slice(&self.soft);
                }
                let frame_done = b == mode.blocks() - 1 && self.have.iter().all(|&h| h);
                let mut push_tf =
                    |channel: Logical, r: Option<(Vec<u8>, f32)>, st: &mut L1Status| {
                        if let Some((bytes, ber)) = r {
                            if channel == Logical::P1 {
                                st.ber = ber;
                                st.state = 3;
                            }
                            tf.push((
                                f.time,
                                TransferFrame {
                                    channel,
                                    bits: bytes.into(),
                                    mode,
                                    ber,
                                },
                            ));
                        }
                    };
                if band == Band::Fm {
                    // P1 over the frame through the PM interleaver.
                    if frame_done {
                        let mut llr = vec![0.0f32; 365_440];
                        for (i, v) in llr.iter_mut().enumerate() {
                            *v = self.llr_p1[l1::pm_p1_position(i)];
                        }
                        let r = decode_frame(&mut self.p1_fm, &llr);
                        push_tf(Logical::P1, r, &mut self.status);
                    }
                    // P3 through the PX1 interleaver, a block pair at a time. Its cycle is two L1 frames, and the control data
                    // does not say which frame starts it: one candidate starts on the first whole frame, the other on the next,
                    // and the one with the lower channel bit error rate is kept.
                    if b == 0 {
                        self.frames_seen += 1;
                    }
                    if let Some(ch) = &mut self.p3 {
                        if b % 2 == 1 && self.frames_seen > 0 {
                            let n3 = self.llr_p3.len() / mode.blocks();
                            let pair = &self.llr_p3[(b - 1) * n3..(b + 1) * n3];
                            for (e, cand) in self.px1.iter_mut().enumerate() {
                                if self.px1_pick.is_some_and(|p| p != e) {
                                    continue;
                                }
                                if !cand.started {
                                    if b != 1 || self.frames_seen % 2 != e as u64 {
                                        continue;
                                    }
                                    cand.started = true;
                                }
                                let mut frames = Vec::new();
                                cand.rx.push(pair, |fr| frames.push(fr.to_vec()));
                                for fr in frames {
                                    let r = decode_frame(ch, &fr);
                                    if self.px1_pick == Some(e) {
                                        push_tf(Logical::P3, r, &mut self.status);
                                    } else if let Some((_, ber)) = r {
                                        cand.ber = cand.ber.min(ber);
                                        cand.frames += 1;
                                    }
                                }
                            }
                            if self.px1_pick.is_none() && self.px1.iter().all(|c| c.frames > 0) {
                                let best = (0..self.px1.len())
                                    .min_by(|&a, &b| self.px1[a].ber.total_cmp(&self.px1[b].ber));
                                self.px1_pick = best;
                            }
                        }
                    }
                } else if frame_done {
                    if let Some(am) = self.am.clone() {
                        // P1: this frame's main subframes wait for their backup subframes, three frames later.
                        let mut now = vec![0.0f32; 72_000];
                        let mut before = vec![0.0f32; 72_000];
                        am.p1_unframe(&self.llr_p1, &mut now, &mut before);
                        self.am_main.push_back(now);
                        let mut before_m = Vec::new();
                        if self.am_mirrored.is_none() {
                            let mut now_m = vec![0.0f32; 72_000];
                            before_m = vec![0.0f32; 72_000];
                            am.p1_unframe(&self.llr_p1_mirrored, &mut now_m, &mut before_m);
                            self.am_main_mirrored.push_back(now_m);
                        }
                        if self.am_main.len() > l1::AM_DIVERSITY_FRAMES {
                            let mut main = self.am_main.pop_front().unwrap_or_default();
                            // The main and backup subframes hold different bits: each LLR is in one of them only.
                            for (m, &bk) in main.iter_mut().zip(&before) {
                                *m += bk;
                            }
                            // The mirror: the reading whose first transfer frame has the lower channel bit error rate.
                            if self.am_mirrored.is_none() {
                                let mut main_m =
                                    self.am_main_mirrored.pop_front().unwrap_or_default();
                                for (m, &bk) in main_m.iter_mut().zip(&before_m) {
                                    *m += bk;
                                }
                                let mut ber = |t: &[f32]| {
                                    decode_frame(&mut self.p1_am, t).map_or(1.0, |r| r.1)
                                };
                                let mirrored = ber(&main_m[..9000]) < ber(&main[..9000]);
                                self.am_mirrored = Some(mirrored);
                                if mirrored {
                                    main = main_m;
                                    std::mem::swap(&mut self.am_main, &mut self.am_main_mirrored);
                                    self.am_p3_main.clear();
                                }
                                self.am_main_mirrored.clear();
                            }
                            for t in main.chunks(9000) {
                                let r = decode_frame(&mut self.p1_am, t);
                                push_tf(Logical::P1, r, &mut self.status);
                            }
                        }
                        // P3: MA1 at once; MA3 with its backup subframes three frames later.
                        if let Some(ch) = &mut self.p3 {
                            let mut now3 = vec![0.0f32; ch.coded()];
                            let mut before3 = vec![0.0f32; ch.coded()];
                            am.p3_unframe(&self.llr_p3, &mut now3, &mut before3);
                            if mode == ServiceMode::Ma3 {
                                self.am_p3_main.push_back(now3);
                                if self.am_p3_main.len() > l1::AM_DIVERSITY_FRAMES {
                                    let mut main = self.am_p3_main.pop_front().unwrap_or_default();
                                    for (m, &bk) in main.iter_mut().zip(&before3) {
                                        *m += bk;
                                    }
                                    let r = decode_frame(ch, &main);
                                    push_tf(Logical::P3, r, &mut self.status);
                                }
                            } else {
                                let r = decode_frame(ch, &now3);
                                push_tf(Logical::P3, r, &mut self.status);
                            }
                        }
                    }
                }
                st.push((f.time, self.status.clone()));
                if self.active.is_some_and(|a| a != ri) {
                    break;
                }
            }
        }
        for (t, v) in tf {
            io.items_out::<TransferFrame>(0)
                .push(Timed { time: t, value: v });
        }
        for (t, v) in pids_out {
            io.items_out::<PidsFrame>(1)
                .push(Timed { time: t, value: v });
        }
        for (t, v) in st {
            io.items_out::<L1Status>(2)
                .push(Timed { time: t, value: v });
        }
    }
    /// The search starts again, and so do the block counts and the measurements.
    fn forget(&mut self, _kept: &Kept) {
        self.reset();
        self.status = L1Status::default();
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => self.reset(),
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Layer 2

/// An HDC audio packet of one program (`hdc ` media; sdrkit ships no decoder).
#[derive(Clone, Debug)]
pub struct HdcPacket {
    /// Program number.
    pub program: u8,
    /// The packet (without its CRC); `None` when the CRC failed.
    pub bytes: Option<Arc<[u8]>>,
    /// Codec mode.
    pub codec_mode: u8,
    /// PTS, 90 kHz.
    pub pts: i64,
}
impl Item for HdcPacket {
    const KIND: &'static str = "hdr.hdc";
}

/// A PDU header seen (per program).
#[derive(Clone, Debug)]
pub struct PduInfo {
    /// The header fields.
    pub header: Pdu,
    /// Packets good.
    pub good: u32,
    /// Packets bad.
    pub bad: u32,
    /// The logical channel.
    pub channel: Logical,
}
impl Item for PduInfo {
    const KIND: &'static str = "hdr.pdu";
}

/// PSD of one program.
#[derive(Clone, Debug)]
pub struct PsdItem {
    /// Program.
    pub program: u8,
    /// The data.
    pub psd: Psd,
}
impl Item for PsdItem {
    const KIND: &'static str = "hdr.psd";
    fn event_def() -> Option<&'static EventDef> {
        Some(res::SONG.def())
    }
    fn write_fields(&self, r: &mut Record) {
        res::HdradioSongEvent {
            channel: &format!("HD{}", u16::from(self.program) + 1),
            title: &self.psd.title,
            artist: &self.psd.artist,
            text: &song_text(&self.psd),
            family: Family::Broadcast,
        }
        .write(r);
    }
}

/// `s`, or `None` when it is empty.
fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// The text of a song: "artist - title", or the one of them that is there.
fn song_text(psd: &Psd) -> String {
    match (psd.artist.is_empty(), psd.title.is_empty()) {
        (false, false) => format!("{} - {}", psd.artist, psd.title),
        (true, _) => psd.title.clone(),
        (false, true) => psd.artist.clone(),
    }
}

/// An AAS packet (`hdr.data_service`).
#[derive(Clone, Debug)]
pub struct AasItem(pub AasPacket);
impl Item for AasItem {
    const KIND: &'static str = "hdr.data_service";
}

/// Layer 2 counters.
#[derive(Clone, Debug, Default)]
pub struct L2Stats {
    /// P1 frames with a good PCI.
    pub p1_good: u64,
    /// P1 frames with a bad header.
    pub p1_bad: u64,
    /// P3 good.
    pub p3_good: u64,
    /// P3 bad.
    pub p3_bad: u64,
    /// Audio PDUs.
    pub pdus: u64,
    /// AAS good.
    pub aas_good: u64,
    /// AAS bad FCS.
    pub aas_bad: u64,
}
impl Item for L2Stats {
    const KIND: &'static str = "hdr.l2";
}

/// Layer 2 (`broadcast.hd_l2`, typed transfer frames -> HDC packets, PDU info, PSD, AAS packets, counters).
pub struct HdL2 {
    rs: sdrkit_algo_fec::rs::Rs,
    /// The fixed data of P1 and of P3: two separate HDLC streams.
    fixed: [AasReceiver; 2],
    /// The fixed bearer of P1 and of P3.
    fixed_rx: [FixedReader; 2],
    psd: BTreeMap<u8, AasReceiver>,
    stats: L2Stats,
    au: BTreeMap<u8, i64>,
}

impl Default for HdL2 {
    fn default() -> Self {
        HdL2 {
            rs: l2::header_rs(),
            fixed: Default::default(),
            fixed_rx: Default::default(),
            psd: BTreeMap::new(),
            stats: L2Stats::default(),
            au: BTreeMap::new(),
        }
    }
}

impl Component for HdL2 {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.hd_l2",
            vec![Port::typed::<TransferFrame>("transfer")],
            vec![
                item_out::<HdcPacket>("hdc"),
                item_out::<PduInfo>("pdus"),
                item_out::<PsdItem>("psd"),
                item_out::<AasItem>("aas"),
                item_out::<L2Stats>("stats"),
            ],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fixed.iter_mut().for_each(AasReceiver::reset);
        self.fixed_rx.iter_mut().for_each(FixedReader::reset);
        self.psd.clear();
        Ok(vec![
            Format::new(Kind::typed::<HdcPacket>(), 0.0),
            Format::new(Kind::typed::<PduInfo>(), 0.0),
            Format::new(Kind::typed::<PsdItem>(), 0.0),
            Format::new(Kind::typed::<AasItem>(), 0.0),
            Format::new(Kind::typed::<L2Stats>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let (mut hdc, mut pdus, mut psds, mut aas, mut st) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for it in io.items::<TransferFrame>(0) {
            let t = it.time;
            let f = &it.value;
            let ch = f.channel as usize;
            // The PCI spread over the transfer frame (1014s clause 5.2.1).
            let d = despread(&f.bits);
            let cw = d.as_ref().and_then(|d| match_pci(d.word, d.mask));
            let (Some(d), Some(cw)) = (d, cw) else {
                match f.channel {
                    Logical::P1 => self.stats.p1_bad += 1,
                    Logical::P3 => self.stats.p3_bad += 1,
                }
                self.fixed[ch].reset();
                st.push((t, self.stats.clone()));
                continue;
            };
            match f.channel {
                Logical::P1 => self.stats.p1_good += 1,
                Logical::P3 => self.stats.p3_good += 1,
            }
            let b = &d.payload;
            // The fixed data at the end of the PDU (1019s clause 7.5).
            let mut audio_end = b.len();
            if pci::has_fixed(cw) {
                let subs = self.fixed_rx[ch].push(b);
                audio_end = self.fixed_rx[ch]
                    .region_len()
                    .map_or(audio_end, |n| b.len().saturating_sub(n));
                for (_, s, e) in subs {
                    let mut out = Vec::new();
                    self.fixed[ch].push(&b[s..e], &mut out);
                    for a in out {
                        aas.push((t, AasItem(a)));
                    }
                }
                self.stats.aas_good = self.fixed.iter().map(|x| x.good).sum();
                self.stats.aas_bad = self.fixed.iter().map(|x| x.bad).sum();
            }
            // The audio PDUs from byte 0, each ending at its last locator (1014s clause 5.3).
            let mut p = 0;
            while pci::has_audio(cw) && p + l2::HEADER_LEN <= audio_end {
                let Some((pdu, len)) = parse_pdu(&b[p..audio_end], &mut self.rs) else {
                    break;
                };
                p += len;
                self.stats.pdus += 1;
                let prog = pdu.header.program;
                let au = self.au.entry(prog).or_insert(0);
                let mut good = 0;
                let mut bad = 0;
                for pk in &pdu.packets {
                    let pts = *au * 90_000 * 2048 / 44_100;
                    *au += 1;
                    match pk {
                        Some(_) => good += 1,
                        None => bad += 1,
                    }
                    hdc.push((
                        t,
                        HdcPacket {
                            program: prog,
                            bytes: pk.as_ref().map(|x| Arc::from(&x[..])),
                            codec_mode: pdu.header.codec_mode,
                            pts,
                        },
                    ));
                }
                // The PSD bytes of the PDU carry an HDLC stream of ID3 tags (1085s clause 5).
                let h = self.psd.entry(prog).or_default();
                let mut ps = Vec::new();
                h.push(&pdu.header.psd, &mut ps);
                for x in ps {
                    if x.port != psd_port(prog) {
                        continue;
                    }
                    if let Some(psd) = Psd::from_id3(&x.payload) {
                        psds.push((t, PsdItem { program: prog, psd }));
                    }
                }
                pdus.push((
                    t,
                    PduInfo {
                        header: pdu.header,
                        good,
                        bad,
                        channel: f.channel,
                    },
                ));
            }
            st.push((t, self.stats.clone()));
        }
        for (t, v) in hdc {
            io.items_out::<HdcPacket>(0)
                .push(Timed { time: t, value: v });
        }
        for (t, v) in pdus {
            io.items_out::<PduInfo>(1).push(Timed { time: t, value: v });
        }
        for (t, v) in psds {
            io.items_out::<PsdItem>(2).push(Timed { time: t, value: v });
        }
        for (t, v) in aas {
            io.items_out::<AasItem>(3).push(Timed { time: t, value: v });
        }
        for (t, v) in st {
            io.items_out::<L2Stats>(4).push(Timed { time: t, value: v });
        }
    }
    /// The receivers of the data and the song data start again, and so do the counts.
    fn forget(&mut self, _kept: &Kept) {
        *self = HdL2::default();
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => {
                self.fixed.iter_mut().for_each(AasReceiver::reset);
                self.fixed_rx.iter_mut().for_each(FixedReader::reset);
                self.psd.clear();
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Station

/// A row of the programs table.
#[derive(Clone, Debug, Default)]
pub struct ProgramRow {
    /// Number (0 = HD1).
    pub number: u8,
    /// "HD1".
    pub label: String,
    /// SIG name.
    pub name: String,
    /// Program type.
    pub program_type: u8,
    /// Type name.
    pub type_name: String,
    /// Restricted access.
    pub restricted: bool,
    /// Codec mode.
    pub codec_mode: Option<u8>,
    /// Blend control.
    pub blend: Option<u8>,
    /// Sound experience.
    pub sound_experience: u8,
    /// Bit rate, bit/s.
    pub bitrate: f64,
    /// Audio packets good.
    pub packets_good: u64,
    /// Audio packets bad.
    pub packets_bad: u64,
    /// In the SIS.
    pub in_sis: bool,
    /// In the SIG.
    pub in_sig: bool,
    /// On air.
    pub broadcasting: bool,
    /// PSD messages.
    pub psd_count: u64,
    /// PSD.
    pub psd: Psd,
    /// Album art link (LOT id, MIME).
    pub art: Option<(u32, u32)>,
    /// Logo port.
    pub logo_port: Option<u16>,
    /// Art port.
    pub art_port: Option<u16>,
}

/// A row of the data services table.
#[derive(Clone, Debug, Default)]
pub struct DataServiceRow {
    /// Name.
    pub name: String,
    /// Service number.
    pub number: u16,
    /// Port.
    pub port: u16,
    /// AAS type.
    pub kind: u8,
    /// MIME hash name.
    pub mime: String,
    /// Packets received on the port.
    pub packets: u64,
    /// Bytes.
    pub bytes: u64,
}

/// The reception state and details of the station.
#[derive(Clone, Debug, Default)]
pub struct HdDetail {
    /// State 0..3.
    pub state: u8,
    /// The service mode.
    pub mode: Option<ServiceMode>,
    /// Carrier offset.
    pub offset_hz: f64,
    /// MER.
    pub mer_db: (f32, f32),
    /// Channel BER.
    pub ber: f32,
    /// Blocks.
    pub blocks: u64,
    /// Block syncs.
    pub syncs: u64,
    /// L2 counters.
    pub l2: L2Stats,
    /// PIDS good.
    pub pids_good: u64,
    /// PIDS bad.
    pub pids_bad: u64,
    /// Level.
    pub level_dbfs: f32,
    /// SIS.
    pub sis: Sis,
    /// Programs.
    pub programs: Vec<ProgramRow>,
    /// Data services.
    pub data_services: Vec<DataServiceRow>,
    /// Files.
    pub files: Vec<l2::LotFile>,
    /// Signal time.
    pub signal_time_s: f64,
    /// Codec note.
    pub codec_note: String,
    /// The indicators.
    pub p1_lamp: bool,
    /// PIDS recent.
    pub pids_on: bool,
    /// PSD seen.
    pub psd_on: bool,
    /// Data seen.
    pub data_on: bool,
    /// The program to hear.
    pub program: u8,
}

/// The station (`broadcast.hd_station`, sink): SIS, SIG, LOT, programs, data services, files, and the results.
pub struct HdStation {
    sis: SisReceiver,
    d: HdDetail,
    published: Published,
    status: Status,
    publisher: Publisher,
    no_tags: TagSet,
    label: String,
    key: String,
    programs: BTreeMap<u8, ProgramRow>,
    sig: Vec<SigService>,
    lot: LotCollector,
    ports: BTreeMap<u16, (u64, u64)>,
    next_report: f64,
    noted: bool,
    last_pids_t: f64,
    rate_window: BTreeMap<u8, VecDeque<(f64, usize)>>,
    program: u8,
    d_points: Vec<[f32; 2]>,
}

impl Default for HdStation {
    fn default() -> Self {
        HdStation {
            sis: SisReceiver::default(),
            d: HdDetail::default(),
            published: Published::from_decl(Arc::new(crate::typed::shape(ID).results_decl())),
            status: Status::new(LockState::Searching),
            publisher: Publisher::new(),
            no_tags: TagSet::default(),
            label: String::new(),
            key: String::with_capacity(16),
            programs: BTreeMap::new(),
            sig: Vec::new(),
            lot: LotCollector::default(),
            ports: BTreeMap::new(),
            next_report: 0.25,
            noted: false,
            last_pids_t: -10.0,
            rate_window: BTreeMap::new(),
            program: 0,
            d_points: Vec::new(),
        }
    }
}

impl HdStation {
    fn row(&mut self, n: u8) -> &mut ProgramRow {
        self.programs.entry(n).or_insert_with(|| ProgramRow {
            number: n,
            label: format!("HD{}", n + 1),
            ..Default::default()
        })
    }
    fn publish(&mut self) {
        let sis = self.sis.sis.clone();
        for &(p, ty, restricted, sound) in &sis.audio_services {
            let r = self.row(p);
            r.in_sis = true;
            r.program_type = ty;
            r.type_name = l2::program_type_name(ty).into();
            r.restricted = restricted;
            r.sound_experience = sound;
        }
        let sig = self.sig.clone();
        let mut ds = Vec::new();
        for s in &sig {
            if !s.data {
                let n = (s.number - 1) as u8;
                let r = self.row(n);
                r.in_sig = true;
                r.name.clone_from(&s.name);
                for c in &s.components {
                    if c.data && c.mime == 0xBE4B_7536 {
                        r.art_port = Some(c.port);
                    }
                    if c.data && c.mime == 0xD9C7_2536 {
                        r.logo_port = Some(c.port);
                    }
                }
            } else {
                for c in &s.components {
                    let (pk, by) = self.ports.get(&c.port).copied().unwrap_or_default();
                    ds.push(DataServiceRow {
                        name: s.name.clone(),
                        number: s.number,
                        port: c.port,
                        kind: c.kind,
                        mime: l2::mime_name(c.mime),
                        packets: pk,
                        bytes: by,
                    });
                }
            }
        }
        let d = &mut self.d;
        d.sis = sis;
        d.programs = self.programs.values().cloned().collect();
        d.data_services = ds;
        let mut files: Vec<l2::LotFile> = self.lot.files.values().cloned().collect();
        files.sort_by(|a, b| b.updated_s.total_cmp(&a.updated_s));
        d.files = files;
        d.pids_good = self.sis.good;
        d.pids_bad = self.sis.bad;
        d.pids_on = d.signal_time_s - self.last_pids_t < 2.0;
        d.psd_on = d.programs.iter().any(|p| p.psd_count > 0);
        d.data_on = !self.ports.is_empty();
        d.program = self.program;
        let p = &mut self.published;
        let found = d.state > 0;
        let sis = &d.sis;
        let station = d.pids_good > 0;
        self.key.clear();
        let _ = write!(self.key, "HD{}", u16::from(d.program) + 1);
        p.put(&res::HdradioFields {
            band: d.mode.map(|m| match m {
                ServiceMode::Mp1 | ServiceMode::Mp3 => res::HdBand::FmHybrid,
                ServiceMode::Ma1 => res::HdBand::AmHybrid,
                ServiceMode::Ma3 => res::HdBand::AmAllDigital,
            }),
            service_mode: d.mode,
            offset_hz: found.then_some(Hertz(d.offset_hz)),
            mer_lower_db: found.then_some(f64::from(d.mer_db.0)),
            mer_upper_db: found.then_some(f64::from(d.mer_db.1)),
            ber: found.then_some(f64::from(d.ber)),
            blocks: d.blocks as i64,
            block_syncs: d.syncs as i64,
            p1_ok: d.l2.p1_good as i64,
            p1_bad: d.l2.p1_bad as i64,
            p3_ok: d.l2.p3_good as i64,
            p3_bad: d.l2.p3_bad as i64,
            pids_ok: d.pids_good as i64,
            pids_bad: d.pids_bad as i64,
            pdus: d.l2.pdus as i64,
            aas_ok: d.l2.aas_good as i64,
            aas_bad: d.l2.aas_bad as i64,
            level_dbfs: found.then_some(f64::from(d.level_dbfs)),
            signal_time: Seconds(d.signal_time_s),
            codec_note: non_empty(&d.codec_note),
            p1_on: d.p1_lamp,
            pids_on: d.pids_on,
            psd_on: d.psd_on,
            data_on: d.data_on,
            program: &self.key,
            call_sign: non_empty(&sis.short_name),
            universal_name: non_empty(&sis.universal_name),
            long_name: non_empty(&sis.long_name),
            slogan: non_empty(&sis.slogan),
            message: non_empty(&sis.message),
            country: non_empty(&sis.country),
            facility_id: station.then_some(sis.facility_id as i64),
            location: station.then_some((sis.lat, sis.lon)),
            altitude_m: station.then_some(f64::from(sis.alt_m)),
        });
        for r in &d.programs {
            let commercial = r
                .psd
                .commercial
                .as_ref()
                .map(|c| format!("{}; {}; {}; {}; {}", c.0, c.1, c.2, c.3, c.4));
            let art_mime = r.art.map(|a| l2::mime_name(a.1));
            let row = res::HdradioProgramRow {
                number: i64::from(r.number),
                label: &r.label,
                name: non_empty(&r.name),
                program_type: i64::from(r.program_type),
                type_name: non_empty(&r.type_name),
                restricted: r.restricted,
                codec_mode: r.codec_mode.map(i64::from),
                blend: r.blend.map(i64::from),
                sound_experience: i64::from(r.sound_experience),
                bit_rate_bps: r.bitrate,
                packets_ok: r.packets_good as i64,
                packets_bad: r.packets_bad as i64,
                in_sis: r.in_sis,
                in_sig: r.in_sig,
                broadcasting: r.broadcasting,
                psd_count: r.psd_count as i64,
                title: non_empty(&r.psd.title),
                artist: non_empty(&r.psd.artist),
                album: non_empty(&r.psd.album),
                genre: non_empty(&r.psd.genre),
                comment: non_empty(&r.psd.comment),
                ufid: non_empty(&r.psd.ufid),
                commercial: commercial.as_deref(),
                art_lot: r.art.map(|a| i64::from(a.0)),
                art_mime: art_mime.as_deref(),
                logo_port: r.logo_port.map(i64::from),
                art_port: r.art_port.map(i64::from),
            };
            res::PROGRAMS.put(p, &r.label, &row);
        }
        for ds in &d.data_services {
            self.key.clear();
            let _ = write!(self.key, "{}", ds.port);
            let row = res::HdradioDataServiceRow {
                name: &ds.name,
                service: i64::from(ds.number),
                port: i64::from(ds.port),
                kind: i64::from(ds.kind),
                mime: &ds.mime,
                packets: ds.packets as i64,
                bytes: ds.bytes as i64,
            };
            res::DATA_SERVICES.put(p, &self.key, &row);
        }
        for f in &d.files {
            self.key.clear();
            let _ = write!(self.key, "{}:{}", f.port, f.lot);
            let mime = l2::mime_name(f.mime);
            let row = res::HdradioFileRow {
                port: f.port as i64,
                lot: f.lot as i64,
                name: &f.name,
                mime: &mime,
                size: f.size as i64,
                received: f.received as i64,
                complete: f.complete,
                version: f.version as i64,
                expiry_mjd: f.expiry_mjd as i64,
                updated: Seconds(f.updated_s),
            };
            res::FILES.put(p, &self.key, &row);
        }
        // The song of the program playing is the station's text.
        if let Some(r) = d.programs.iter().find(|r| r.number == d.program) {
            let text = song_text(&r.psd);
            if text != self.label {
                self.publisher.label(p, &text, &self.no_tags);
                self.label = text;
            }
        }
        let st = &mut self.status;
        st.lock_state = match d.state {
            0 => LockState::Searching,
            1 | 2 => LockState::Tracking,
            _ => LockState::Locked,
        };
        st.snr_db = Some(d.mer_db.0.min(d.mer_db.1));
        st.level_dbfs = Some(d.level_dbfs);
        st.offset_hz = Some(d.offset_hz);
        st.summary.clear();
        let _ = match d.state {
            0 => write!(st.summary, "searching"),
            1 => write!(st.summary, "signal found, synchronising"),
            2 => write!(st.summary, "locked, waiting for a frame"),
            _ => write!(
                st.summary,
                "{} {}, {}; {}",
                match d.mode {
                    Some(ServiceMode::Mp1 | ServiceMode::Mp3) => "FM hybrid",
                    Some(ServiceMode::Ma1) => "AM hybrid",
                    Some(ServiceMode::Ma3) => "AM all-digital",
                    None => "",
                },
                d.mode.map_or("", ServiceMode::name),
                d.sis.short_name,
                match d.mode {
                    Some(ServiceMode::Mp1 | ServiceMode::Mp3) => {
                        "analogue FM sound, digital sound not decoded"
                    }
                    _ => "no sound, digital sound not decoded",
                }
            ),
        };
        let r = p.results_mut();
        r.set_status(&self.status);
        let pts = &self.d_points;
        r.update_series(SeriesId(0), |s| {
            sdrkit_core::results::copy_if_changed(&mut s.points, pts)
        });
    }
}

sdrkit_core::commands! {
    /// The commands of [`HdStation`].
    pub enum StationCommand {
        /// Plays the program whose `programs` row key ("HD1" to "HD8") is the argument `row`. "0" plays HD1.
        ServiceSelect = SERVICE_SELECT_KIND.command,
    }
}

impl Component for HdStation {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.hd_station",
            vec![
                Port::typed::<L1Status>("status"),
                Port::typed::<PidsFrame>("pids"),
                Port::typed::<PduInfo>("pdus"),
                Port::typed::<PsdItem>("psd"),
                Port::typed::<AasItem>("aas"),
                Port::typed::<L2Stats>("stats"),
            ],
            vec![item_out::<Picture>("pictures")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Picture>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut tmax = 0.0f64;
        for s in io.items::<L1Status>(0) {
            let v = &s.value;
            tmax = tmax.max(s.time);
            let d = &mut self.d;
            d.state = v.state;
            d.mode = v.mode;
            d.offset_hz = v.offset_hz;
            d.mer_db = v.mer_db;
            d.ber = v.ber;
            d.blocks = v.blocks;
            d.syncs = v.syncs;
            d.level_dbfs = v.level_dbfs;
            d.signal_time_s = v.time;
            self.d_points.clone_from(&v.points);
        }
        for p in io.items::<PidsFrame>(1) {
            let before = self.sis.good;
            self.sis
                .push(&p.value.bits, p.value.block as usize, p.value.fm);
            if self.sis.good > before {
                self.last_pids_t = p.time;
            }
        }
        for p in io.items::<PduInfo>(2) {
            let h = &p.value.header;
            let t = p.time;
            let good = p.value.good as u64;
            let bad = p.value.bad as u64;
            let w = self.rate_window.entry(h.program).or_default();
            w.push_back((t, (good + bad) as usize));
            while w.front().is_some_and(|x| t - x.0 > 3.0) {
                w.pop_front();
            }
            let n: usize = w.iter().map(|x| x.1).sum();
            let r = self.row(h.program);
            r.broadcasting = true;
            r.codec_mode = Some(h.codec_mode);
            r.blend = Some(h.blend);
            r.program_type = h.program_type;
            r.type_name = l2::program_type_name(h.program_type).into();
            r.packets_good += good;
            r.packets_bad += bad;
            r.bitrate = n as f64 * 2048.0 / 44_100.0; // packets per second of sound
            if !self.noted {
                self.noted = true;
                self.d.codec_note = HDC_NOTE.into();
                self.published.results_mut().log(HDC_NOTE);
            }
        }
        for p in io.items::<PsdItem>(3) {
            let r = self.row(p.value.program);
            r.psd_count += 1;
            r.art = p.value.psd.xhdr;
            r.psd = p.value.psd.clone();
        }
        let mut pics = Vec::new();
        for a in io.items::<AasItem>(4) {
            let pk = &a.value.0;
            let e = self.ports.entry(pk.port).or_default();
            e.0 += 1;
            e.1 += pk.payload.len() as u64;
            if pk.port == l2::SIG_PORT {
                if let Some(s) = parse_sig(&pk.payload) {
                    self.sig = s;
                }
            } else if pk.port >= 0x1000 {
                if let Some(key) = self.lot.push(pk.port, &pk.payload, a.time) {
                    let f = &self.lot.files[&key];
                    let is_png = f.mime == 0x4F32_8CA0 || f.name.ends_with(".png");
                    let is_jpg = f.mime == 0x1E65_3E9C || f.name.ends_with(".jpg");
                    if is_png || is_jpg || f.mime == 0xBE4B_7536 || f.mime == 0xD9C7_2536 {
                        let ext = if is_jpg { "jpg" } else { "png" };
                        pics.push((
                            a.time,
                            Picture {
                                codec: if is_jpg { Codec::Jpeg } else { Codec::Png },
                                layout: None,
                                name: Some(format!("hdradio_{}_{}.{}", f.port, f.lot, ext)),
                                id: Picture::WHOLE,
                                state: sdrkit_core::PictureState::Finished,
                                data: f.bytes.clone().unwrap_or_else(|| Arc::from(&[][..])),
                            },
                        ));
                    }
                }
            }
        }
        for s in io.items::<L2Stats>(5) {
            self.d.l2 = s.value.clone();
            self.d.p1_lamp = s.value.p1_good > 0;
        }
        let t = self.d.signal_time_s.max(tmax);
        if t >= self.next_report {
            while self.next_report <= t {
                self.next_report += 0.25;
            }
            self.publish();
        }
        for (t, p) in pics {
            io.items_out::<Picture>(0).push(Timed { time: t, value: p });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        StationCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match StationCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            StationCommand::ServiceSelect => {
                let row = args.arg(&SERVICE_ROW)?;
                let program = match row {
                    "0" => Some(0),
                    _ => row
                        .strip_prefix("HD")
                        .and_then(|n| n.parse::<u8>().ok())
                        .filter(|n| (1..=8).contains(n))
                        .map(|n| n - 1),
                };
                let Some(program) = program else {
                    return Err(CommandError::NoSuchRow {
                        table: res::programs_table(),
                        key: row.to_string(),
                    });
                };
                self.program = program;
                self.publish();
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    /// Everything of the station on the frequency before goes: the programs, the data services, the files, the station
    /// data, the text, the counts and the measurements. The program to hear and the tables of `kept` stay.
    fn forget(&mut self, kept: &Kept) {
        let mut published = std::mem::replace(&mut self.published, Published::new(&[]));
        let mut publisher = std::mem::take(&mut self.publisher);
        let program = self.program;
        *self = HdStation::default();
        self.program = program;
        publisher.clear_label(&mut published);
        for t in &crate::typed::shape(ID).tables {
            if !kept.table(t.def.key) {
                published.clear_rows(t.def);
            }
        }
        published.results_mut().update_series(SeriesId(0), |s| {
            let changed = !s.points.is_empty();
            s.points.clear();
            changed
        });
        self.published = published;
        self.publisher = publisher;
        self.publish();
    }
}

/// Why the digital sound does not play: sdrkit has no HDC decoder.
pub const HDC_NOTE: &str = "HDC digital sound is not decoded";

/// The sound stage (`broadcast.hd_sound`, audio + typed `status` -> audio): passes the analogue FM programme while the
/// signal is FM: the band that Layer 1 found, or before that a frequency above 30 MHz (or an unknown one). It writes
/// [`res::HdSoundFields`].
pub struct HdSoundStage {
    fm_band: bool,
    band: Option<Band>,
    sound: res::HdSound,
    published: Published,
}

impl Default for HdSoundStage {
    fn default() -> Self {
        let mut published = Published::new(res::HdSoundFields::SHAPE.fields);
        published.put(&res::HdSoundFields {
            sound: res::HdSound::Silent,
        });
        HdSoundStage {
            fm_band: true,
            band: None,
            sound: res::HdSound::Silent,
            published,
        }
    }
}

impl HdSoundStage {
    fn show(&mut self, sound: res::HdSound) {
        if sound != self.sound {
            self.sound = sound;
            self.published.put(&res::HdSoundFields { sound });
        }
    }
}

impl Component for HdSoundStage {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.hd_sound",
            vec![Port::audio("in"), Port::typed::<L1Status>("status")],
            vec![Port::audio("out")
                .rate_from(RateFrom::Input(0))
                .channels_from(ChannelsFrom::Input(0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let t = inputs[0].tuned_hz;
        self.fm_band = t <= 0.0 || t > 30e6;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        for s in io.items::<L1Status>(1) {
            if s.value.band.is_some() {
                self.band = s.value.band;
            }
        }
        let x = &io.audio(0).samples;
        let fm = self.band.map_or(self.fm_band, |b| b == Band::Fm);
        if !fm {
            self.show(res::HdSound::Silent);
            return;
        }
        if x.is_empty() {
            return;
        }
        io.audio_out(0).samples.extend_from_slice(x);
        self.show(res::HdSound::AnalogueFm);
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The band found and the sound shown start again.
    fn forget(&mut self, _kept: &Kept) {
        self.band = None;
        self.show(res::HdSound::Silent);
    }
}

/// The HD Radio graph. The FM lane also feeds the analogue programme: ±100 kHz around the carrier, the shared FM
/// demodulator at 75 kHz deviation, 48 kHz, 15 kHz wide, 75 µs de-emphasis.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build: a fault of this crate, never of the input.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let am_front = GraphError::checked("am_front", crate::component::channel(20e3, l1::AM_RATE))?;
    let fm_front = FmFront::new()?;
    let analog = GraphError::checked("analog", crate::component::channel(100e3, 250e3))?;
    let analog_demod = GraphError::checked("analog_demod", FmDemod::new(75e3))?;
    let analog_rate = GraphError::checked("analog_rate", Resample::new(48e3))?;
    let analog_filter = GraphError::checked("analog_filter", FirFilter::lowpass(15e3, 2e3))?;
    GraphBuilder::new()
        .add("fm_front", fm_front)
        .add("am_front", am_front)
        .add("l1", HdL1::default())
        .add("l2", HdL2::default())
        .add("station", HdStation::default())
        .add("analog", analog)
        .add("analog_demod", analog_demod)
        .add("analog_rate", analog_rate)
        .add("analog_filter", analog_filter)
        .add("analog_deemph", DeemphasisComponent::new(75.0))
        .add("analog_audio", RealToAudio::default())
        .add("sound", HdSoundStage::default())
        .input("fm_front")
        .input("am_front")
        .connect("fm_front", "analog")
        .connect("analog", "analog_demod")
        .connect("analog_demod", "analog_rate")
        .connect("analog_rate", "analog_filter")
        .connect("analog_filter", "analog_deemph")
        .connect("analog_deemph", "analog_audio")
        .connect("analog_audio", "sound.in")
        .connect("l1.status", "sound.status")
        .connect("fm_front", "l1.fm")
        .connect("am_front", "l1.am")
        .connect("l1.transfer", "l2")
        .connect("l1.status", "station.status")
        .connect("l1.pids", "station.pids")
        .connect("l2.pdus", "station.pdus")
        .connect("l2.psd", "station.psd")
        .connect("l2.aas", "station.aas")
        .connect("l2.stats", "station.stats")
        .output("station")
        .output("l2.psd")
        .output("sound")
        .status_from("station")
        .catalog(catalog)
        .build()
}

/// The FM front: the shared channel to the FM OFDM rate. Below the rate that it needs (an AM input) it sends nothing, with a
/// bandwidth of 0, and Layer 1 then searches AM only.
pub struct FmFront {
    inner: sdrkit_graph::components::Channel,
    on: bool,
}

impl FmFront {
    /// The FM front end, or the refusal of its channel, named by its label.
    fn new() -> Result<FmFront, GraphError> {
        Ok(FmFront {
            inner: GraphError::checked("fm_front", crate::component::channel(250e3, l1::FM_RATE))?,
            on: false,
        })
    }
}

impl Component for FmFront {
    fn info(&self) -> ComponentInfo {
        let mut i = self.inner.info();
        i.name = "broadcast.hd_fm_front".into();
        i
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        match self.inner.configure(inputs) {
            Ok(f) => {
                self.on = true;
                Ok(f)
            }
            Err(ConfigError::RateTooLow { .. }) => {
                self.on = false;
                let mut f = inputs[0].clone();
                f.rate = l1::FM_RATE;
                f.bandwidth_hz = 0.0;
                Ok(vec![f])
            }
            Err(e) => Err(e),
        }
    }
    fn process(&mut self, io: &mut Io) {
        if self.on {
            self.inner.process(io);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.inner.apply(id, v)
    }
    fn values(&self) -> Values {
        self.inner.values()
    }
}

/// The HD Radio receiver: its graph in a [`StackGraph`], with the minimum rate set by the band (above 30 MHz the FM band).
pub struct HdReceiver {
    inner: StackGraph,
}

impl HdReceiver {
    /// A receiver with these settings.
    ///
    /// # Errors
    ///
    /// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
    pub fn new(cfg: &StackConfig) -> Result<HdReceiver, GraphError> {
        Ok(HdReceiver {
            inner: StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
                .min_rate(
                    MIN_RATE_AM,
                    "The sample rate is too low for HD Radio in this band",
                )
                .command(
                    "service.select",
                    sdrkit_graph::CommandTarget::of("station", "service.select"),
                )
                .command("clear", sdrkit_graph::CommandTarget::every("clear"))
                .on_retune(sdrkit_graph::CommandTarget::of("l1", "clear"))?
                .with_settings(&cfg.settings)
                .build()?,
        })
    }
    /// The graph.
    pub fn graph_mut(&mut self) -> &mut Graph {
        self.inner.graph_mut()
    }
}

impl Owner for HdReceiver {
    fn schema(&self) -> &Arc<Schema> {
        self.inner.schema()
    }
    fn values(&self) -> Option<&Values> {
        self.inner.values()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.inner.apply(id, value)
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.inner.results_decl()
    }
    fn results(&self) -> &Results {
        self.inner.results()
    }
    fn events(&self) -> &[EventDecl] {
        self.inner.events()
    }
    fn commands(&self) -> &Commands {
        Owner::commands(&self.inner)
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        Owner::invoke(&mut self.inner, id, args)
    }
}

impl Receiver for HdReceiver {
    fn configure(&mut self, input: &sdrkit_core::InputInfo) -> Result<(), ConfigError> {
        // Above 30 MHz is the FM band: a radio needs the filter room, a recording or a generator at the OFDM rate is enough.
        let need = if input.tuned_hz > 30e6 {
            if input.live {
                MIN_RATE_FM
            } else {
                l1::FM_RATE
            }
        } else {
            MIN_RATE_AM
        };
        self.inner.set_min_rate(need);
        self.inner.configure(input)
    }
    fn feed(&mut self, iq: &[Complex32], out: &mut sdrkit_core::Output) {
        self.inner.feed(iq, out)
    }
    fn finish(&mut self, out: &mut sdrkit_core::Output) {
        self.inner.finish(out)
    }
    fn retuned(&mut self, input: &sdrkit_core::InputInfo) {
        self.inner.retuned(input)
    }
    fn forget(&mut self) {
        self.inner.forget()
    }
    fn set_station(&mut self, station: &sdrkit_core::Station) {
        self.inner.set_station(station)
    }
    fn set_facilities(&mut self, facilities: &sdrkit_core::Facilities) {
        self.inner.set_facilities(facilities)
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.inner.save_state()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.inner.restore_state(state)
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        self.inner.as_any_mut()
    }
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(HdReceiver::new(cfg)?))
}

/// Test signal: the signal.
pub const GEN_WAVEFORM: Setting<field::Choice<Waveform>> =
    Setting::choice("gen_waveform", "Waveform", Waveform::Mp1)
        .section("Test signal")
        .advanced();
/// Test signal: the level of the analogue host against nominal.
pub const GEN_ANALOG_DB: Setting<field::F64> =
    Setting::new("gen_analog_db", "Analogue level", "dB", 0.0)
        .range(-20.0, 20.0, 1.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the HD Radio stack: the test signal's.
    pub enum HdSetting {
        /// Test signal: the signal.
        GenWaveform = GEN_WAVEFORM,
        /// Test signal: the level of the analogue host.
        GenAnalogDb = GEN_ANALOG_DB,
    }
}
