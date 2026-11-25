//! The ATSC 3.0 components of this crate: `atsc3.sync` (bootstraps), `atsc3.frame` (frames to the cells of a PLP), `atsc3.fec`
//! (time de-interleaving and FEC blocks to baseband packets), `atsc3.lls` (the service list) and `atsc3.media` (the ROUTE objects of a service to media packets). The link and delivery layers between them are the
//! `ipcast.*` components.

use super::bicm::{BlockResult, PlpCoding, PlpDecoder, PlpPoints, PlpStream};
use super::bootstrap::{self, Detector, Fields};
use super::frame::NotDecoded;
use super::ldm::{CoreCells, LayerDecoder};
use super::rx::{FrameDecoder, FrameInfo, Head, HeadResult, Layers, PlpCells, BACKOFF};
use super::ti::{TiFrame, TimeDeinterleaver};
use super::{Atsc3Setting, PLP};
use super::svc::{
    parse_lls, parse_slt, parse_system_time, SltService, SystemTime, LLS_ADDR, LLS_PORT, TABLE_SLT,
    TABLE_SYSTEM_TIME,
};
use super::delivery::{
    clean_start, describe, entity_body, parse_stsid, Channel, ComponentKind, TrackInfo,
};
use super::results::service_to_receive;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::{
    Complex32, ConfigError, Record, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::media::MediaPacket;
use sdrkit_graph::stream::SlicePool;
use sdrkit_proto_mpegts::publish::{SERVICE_ROW, SERVICE_SELECT_KIND};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Packet,
    PacketPool, Port, RateFrom, RateRule, TagKind, Timed,
};
use sdrkit_proto_ipcast::components::{RouteObjectItem, UdpDatagram};
use sdrkit_proto_ipcast::mp4::{self, Segment, Track};
use sdrkit_proto_ipcast::sls::{read_sls, SlsKind, SlsRead};
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A bootstrap found by `atsc3.sync`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BootstrapFound {
    /// Its start, in samples at 6.144 Msps from the start of the stream.
    pub position: u64,
    /// Its signalling fields.
    pub fields: Fields,
    /// The carrier offset, Hz.
    pub cfo_hz: f64,
    /// Seconds since the bootstrap before it (0 for the first).
    pub frame_s: f64,
}

impl Item for BootstrapFound {
    const KIND: &'static str = "atsc3.bootstrap";
}

/// An output port of typed items, which have no sample rate.
fn items<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// `atsc3.sync`: the bootstrap search. Input `in`: samples at 6.144 Msps (the graph changes the rate). Output `out`: one
/// [`BootstrapFound`] per frame. Output `iq`: the samples it read, for `atsc3.frame`.
#[derive(Default)]
pub struct Sync {
    buf: Vec<Complex32>,
    found: Vec<(usize, Fields, f64)>,
    base: u64,
    det: Detector,
    last: Option<u64>,
}

impl Component for Sync {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc3.sync",
            vec![Port::iq("in").rate(RateRule::Exact(bootstrap::RATE))],
            vec![
                items::<BootstrapFound>("out"),
                items::<BootstrapFound>("state"),
                Port::iq("iq").same_format_as(0),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.buf.clear();
        self.base = 0;
        self.last = None;
        self.det = Detector::new();
        let iq = inputs
            .first()
            .cloned()
            .unwrap_or_else(|| Format::iq(bootstrap::RATE, 0.0, 0.0));
        Ok(vec![
            Format::new(Kind::typed::<BootstrapFound>(), 0.0),
            Format::new(Kind::typed::<BootstrapFound>(), 0.0),
            iq,
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        io.iq_out(2).extend_from_slice(x);
        self.buf.extend_from_slice(x);
        let need = 4 * bootstrap::SYM + bootstrap::N + 4096;
        if self.buf.len() < 3 * need {
            return;
        }
        let mut found = std::mem::take(&mut self.found);
        self.det.find(&self.buf, self.base, &mut found);
        let mut consumed = self.buf.len() - need;
        for &(pos, fields, cfo_hz) in &found {
            let abs = self.base + pos as u64;
            if self.last.is_some_and(|l| abs <= l + 1000) {
                continue;
            }
            let frame_s = self
                .last
                .map_or(0.0, |l| (abs - l) as f64 / bootstrap::RATE);
            self.last = Some(abs);
            let b = BootstrapFound {
                position: abs,
                fields,
                cfo_hz,
                frame_s,
            };
            let time = abs as f64 / bootstrap::RATE;
            io.items_out::<BootstrapFound>(0)
                .push(Timed { time, value: b });
            io.items_out::<BootstrapFound>(1)
                .push(Timed { time, value: b });
            consumed = consumed.max(pos + 4 * bootstrap::SYM);
        }
        self.found = found;
        let consumed = consumed.min(self.buf.len());
        self.buf.drain(..consumed);
        self.base += consumed as u64;
    }
}

/// One subframe of a frame as `atsc3.frame` demodulated it, or a frame that it did not demodulate.
#[derive(Clone, Debug)]
pub struct FrameCells {
    /// The record is the first of its frame.
    pub first: bool,
    /// The number of the subframe in the frame.
    pub subframe: usize,
    /// The start of its bootstrap, in samples at 6.144 Msps from the start of the stream.
    pub position: u64,
    /// What its signalling says, as far as it was read.
    pub info: Option<Arc<FrameInfo>>,
    /// Why the frame is not decoded.
    pub failed: Option<NotDecoded>,
    /// The carrier offset of the frame, Hz.
    pub cfo_hz: f64,
    /// The id of the PLP that is decoded.
    pub id: u8,
    /// The coding of its cells in this subframe: of its core PLP for an enhanced PLP.
    pub coding: Option<PlpCoding>,
    /// The layers on the cells, with the enhanced PLP when it is the one decoded.
    pub layers: Option<Layers>,
    /// The first cell of the first FEC block that begins in the subframe, for a PLP without time interleaving.
    pub fec_block_start: Option<usize>,
    /// The time interleaving of the PLP in the subframe.
    pub ti: TiFrame,
    /// The cells follow those of the record before: no subframe that carries the PLP was lost between them.
    pub follows: bool,
    /// The cells of the PLP, when the subframe was demodulated.
    pub cells: Option<Arc<PlpCells>>,
    /// The PLP is bonded with another RF channel without SNR averaging: every baseband packet follows a gap.
    pub bonded: bool,
}

impl Item for FrameCells {
    const KIND: &'static str = "atsc3.frame_cells";
}

/// What `atsc3.fec` reports after each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameState {
    /// Frames taken.
    pub frames: u64,
    /// Frames decoded: the signalling read and every whole FEC block of the PLP good.
    pub decoded: u64,
    /// The signalling of the last frame whose L1-Basic was read.
    pub info: Option<Arc<FrameInfo>>,
    /// The id of the PLP that is decoded.
    pub plp_id: Option<u8>,
    /// Its coding.
    pub coding: Option<PlpCoding>,
    /// Why the last frame was not decoded. `None` after a decoded frame.
    pub failed: Option<NotDecoded>,
    /// Baseband packets given out.
    pub packets: u64,
    /// FEC blocks whose baseband packet was not good (not given out).
    pub bad_packets: u64,
    /// FEC blocks that ended in the last frame (all its subframes), and those of them that were good.
    pub last_blocks: (u32, u32),
    /// Bits that the outer BCH code corrected after the LDPC decoder.
    pub outer_corrected: u64,
    /// The mean LDPC iterations of the blocks of the last frame.
    pub iterations: f32,
    /// The modulation error ratio of the cells of the PLP in the last frame against the nearest constellation points, dB.
    /// An enhanced PLP is measured on its cells after the core PLP under it is taken off.
    pub mer_db: Option<f32>,
    /// The signal to noise ratio of the pilots of the last frame, dB.
    pub pilot_snr_db: Option<f32>,
}

impl Item for FrameState {
    const KIND: &'static str = "atsc3.frame_state";
}

/// Samples kept while no frame waits: more than `atsc3.sync` is behind.
const KEEP: usize = 1 << 20;

/// `atsc3.frame`: each frame after its bootstrap, to the cells of one PLP. Input `in`: the bootstraps. Input `iq`: the
/// samples that `atsc3.sync` read, at 6.144 Msps. Output `out`: one [`FrameCells`] per subframe that carries the PLP, or
/// one per frame that is not demodulated.
///
/// The Preamble is demodulated as soon as its samples are in: L1-Basic and L1-Detail give the symbols of every subframe
/// and its PLPs. Each subframe that carries the PLP is demodulated when its last sample is in. Setting `plp` chooses the PLP by its id; at -1
/// the first core PLP with low level signalling that the stack decodes is taken, or without one the first core PLP that it
/// decodes.
#[derive(Default)]
pub struct Frame {
    /// The id of the PLP to decode.
    want: Option<u8>,
    buf: Vec<Complex32>,
    base: u64,
    waiting: VecDeque<BootstrapFound>,
    head: Option<Head>,
    /// The subframe of `head` to demodulate next: its place in the parts of the decoder.
    next: usize,
    /// A record of the frame of `head` went out.
    sent: bool,
    /// No subframe that carries the PLP was lost since the last record.
    chain: bool,
    /// The sample after the last demodulated frame.
    end: Option<u64>,
    dec: FrameDecoder,
    /// Cell buffers given out before; one that nothing else holds is used again.
    cells: Vec<Arc<PlpCells>>,
}

impl Frame {
    fn reset(&mut self) {
        self.buf.clear();
        self.base = 0;
        self.waiting.clear();
        self.head = None;
        self.chain = false;
        self.end = None;
    }

    /// A cell buffer that nothing else holds: its place in `self.cells`.
    fn free_cells(&mut self) -> usize {
        match self
            .cells
            .iter_mut()
            .position(|c| Arc::get_mut(c).is_some())
        {
            Some(i) => i,
            None => {
                self.cells.push(Arc::default());
                self.cells.len() - 1
            }
        }
    }
}

impl Component for Frame {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc3.frame",
            vec![
                Port::typed::<BootstrapFound>("in"),
                Port::iq("iq").rate(RateRule::Exact(bootstrap::RATE)),
            ],
            vec![items::<FrameCells>("out")],
        )
        .with_settings(Atsc3Setting::schema())
    }
    fn values(&self) -> Values {
        let id = self.want.map_or(-1, i64::from);
        values_of([(Atsc3Setting::Plp, Value::I64(id))])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        Ok(vec![Format::new(Kind::typed::<FrameCells>(), 0.0)])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match Atsc3Setting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Atsc3Setting::Plp => self.want = u8::try_from(PLP.read(id, v)?).ok(),
        }
        self.dec.select(self.want);
        Ok(Applied::Live)
    }
    /// The frames are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.reset();
    }
    fn process(&mut self, io: &mut Io) {
        self.waiting
            .extend(io.items::<BootstrapFound>(0).iter().map(|b| b.value));
        self.buf.extend_from_slice(io.iq(1));
        while let Some(b) = self.waiting.front().copied() {
            let time = b.position as f64 / bootstrap::RATE;
            let mut record = FrameCells {
                first: true,
                subframe: 0,
                position: b.position,
                info: None,
                failed: None,
                cfo_hz: b.cfo_hz,
                id: 0,
                coding: None,
                layers: None,
                fec_block_start: None,
                ti: TiFrame::None,
                follows: false,
                cells: None,
                bonded: false,
            };
            let from = ((b.position.saturating_sub(self.base)) as usize).min(self.buf.len());
            if b.position < self.base {
                record.failed = Some(NotDecoded::Samples);
            } else if self.head.is_none() {
                match self.dec.head(&b.fields, &self.buf[from..], b.cfo_hz) {
                    HeadResult::Need(_) => break,
                    HeadResult::Failed(why, info) => {
                        record.failed = Some(why);
                        record.info = info;
                    }
                    HeadResult::Ready(h) => {
                        // The bootstrap is found within a few samples of its place.
                        self.chain &= self
                            .end
                            .is_some_and(|e| e.abs_diff(b.position) <= 2 * BACKOFF as u64);
                        self.head = Some(h);
                        self.next = 0;
                        self.sent = false;
                    }
                }
            }
            if record.failed.is_some() {
                self.head = None;
                self.chain = false;
                self.end = None;
                self.waiting.pop_front();
                io.items_out::<FrameCells>(0).push(Timed {
                    time,
                    value: record,
                });
                continue;
            }
            let Some(h) = self.head.take() else {
                break;
            };
            record.info = Some(h.info.clone());
            record.cfo_hz = h.cfo_hz;
            record.id = h.id;
            let mut waits = false;
            while let Some(part) = self.dec.parts().get(self.next).copied() {
                let Some(plp) = part.plp else {
                    self.chain &= !part.refused;
                    self.next += 1;
                    continue;
                };
                if self.buf.len() - from < part.samples {
                    waits = true;
                    break;
                }
                let slot = self.free_cells();
                let read = match Arc::get_mut(&mut self.cells[slot]) {
                    Some(out) => self.dec.part(&h, self.next, &self.buf[from..], out),
                    None => false,
                };
                let mut value = record.clone();
                value.first = !self.sent;
                value.subframe = part.subframe;
                value.coding = Some(plp.coding);
                value.layers = plp.layers;
                value.bonded = plp.bonded;
                if read {
                    value.fec_block_start = plp.fec_block_start;
                    value.ti = plp.ti;
                    value.follows = self.chain;
                    value.cells = Some(self.cells[slot].clone());
                } else {
                    value.failed = Some(NotDecoded::Samples);
                }
                self.chain = read;
                self.sent = true;
                self.next += 1;
                io.items_out::<FrameCells>(0).push(Timed { time, value });
            }
            if waits {
                self.head = Some(h);
                break;
            }
            // A subframe that the decoder did not take may have carried the PLP.
            self.chain &= self.dec.parts().len() == usize::from(h.info.subframes);
            self.end = h.frame_samples.map(|n| b.position + n as u64);
            self.waiting.pop_front();
        }
        // The samples before the frame that waits are done with; with none waiting, the newest are kept for the next one.
        let keep_from = match self.waiting.front() {
            Some(b) => b.position.max(self.base),
            None => self.base + self.buf.len().saturating_sub(KEEP) as u64,
        };
        let drop = ((keep_from - self.base) as usize).min(self.buf.len());
        if drop >= KEEP / 4 || drop == self.buf.len() {
            self.buf.drain(..drop);
            self.base += drop as u64;
        }
    }
}

/// The most LDPC iterations of a FEC block.
pub const LDPC_ITERATIONS: u32 = 50;

/// Cells of a frame that the modulation error ratio is measured on.
const MER_CELLS: usize = 4096;

/// `atsc3.fec`: the FEC blocks of the PLP of each frame to baseband packets, after its time de-interleaver; for an
/// enhanced PLP, after the core PLP under it is taken off ([`LayerDecoder`]). Input `in`: [`FrameCells`]. Output `out`: descrambled baseband packets, one per `Bytes` packet; a block that does not decode gives no
/// packet, and the next packet is marked as following a gap. Output `state`: [`FrameState`] after each frame.
#[derive(Default)]
pub struct Fec {
    stream: Option<PlpStream>,
    layers: Option<LayerDecoder>,
    deinterleaver: TimeDeinterleaver,
    /// The id of the PLP of the record before.
    plp: Option<u8>,
    /// Every subframe of the present frame decoded so far.
    good: bool,
    points: Option<(PlpCoding, PlpPoints)>,
    state: FrameState,
    pool: PacketPool,
    gap: bool,
}

impl Fec {
    /// Counts the FEC blocks of one record: `blocks` ended in it and `good` of them were good. A frame is decoded while
    /// every record of it has cells (`read`) and every block is good.
    fn count(&mut self, first: bool, read: bool, blocks: u32, good: u32) {
        let ok = read && blocks == good;
        if first {
            self.state.last_blocks = (0, 0);
            self.good = false;
        }
        self.state.last_blocks.0 += blocks;
        self.state.last_blocks.1 += good;
        if first && ok {
            self.state.decoded += 1;
            self.good = true;
        } else if !ok && self.good {
            self.state.decoded -= 1;
            self.good = false;
        }
    }
}

impl Component for Fec {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc3.fec",
            vec![Port::typed::<FrameCells>("in")],
            vec![
                Port::bytes("out").rate_from(RateFrom::Fixed(0.0)),
                items::<FrameState>("state"),
            ],
        )
        .manual_tags()
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.stream = None;
        self.layers = None;
        self.deinterleaver.restart();
        self.plp = None;
        self.state = FrameState::default();
        self.gap = true;
        Ok(vec![
            Format::new(Kind::Bytes, 0.0),
            Format::new(Kind::typed::<FrameState>(), 0.0),
        ])
    }
    /// The counts and the block that has begun are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.stream = None;
        self.layers = None;
        self.deinterleaver.restart();
        self.state = FrameState::default();
        self.gap = true;
    }
    fn process(&mut self, io: &mut Io) {
        let frames = io.items::<FrameCells>(0);
        if frames.is_empty() {
            return;
        }
        let mut packets = std::mem::take(io.bytes_out(0));
        let mut gaps = Vec::new();
        for f in frames {
            let c = &f.value;
            self.state.frames += u64::from(c.first);
            if c.info.is_some() {
                self.state.info.clone_from(&c.info);
            }
            let enhanced = c.layers.and_then(|l| l.enhanced);
            if c.coding.is_some() {
                self.state.plp_id = Some(c.id);
                self.state.coding = enhanced.map(|e| e.coding).or(c.coding);
            }
            let coding = c.coding.filter(|_| c.failed.is_none());
            let (Some(coding), Some(plp)) = (coding, c.cells.as_deref()) else {
                self.state.failed = c.failed.or(Some(NotDecoded::Plp));
                self.count(c.first, false, 0, 0);
                if let Some(s) = &mut self.stream {
                    s.restart();
                }
                if let Some(l) = &mut self.layers {
                    l.restart();
                }
                self.deinterleaver.restart();
                self.gap = true;
                io.items_out::<FrameState>(1).push(Timed {
                    time: f.time,
                    value: self.state.clone(),
                });
                continue;
            };
            if self.points.as_ref().is_none_or(|p| p.0 != coding) {
                self.points = PlpPoints::new(coding.bits, coding.rate).map(|p| (coding, p));
            }
            match enhanced {
                None if self.stream.as_ref().is_none_or(|s| *s.coding() != coding) => {
                    self.stream = PlpDecoder::new(coding).map(PlpStream::new);
                }
                Some(e) if self.layers.as_ref().is_none_or(|l| !l.fits(coding, &e)) => {
                    self.layers = LayerDecoder::of(coding, &e);
                }
                _ => {}
            }
            let ready = match enhanced {
                None => self.stream.is_some(),
                Some(_) => self.layers.is_some(),
            };
            if !ready {
                continue;
            }
            // The cells of another PLP do not follow those of the PLP before.
            let follows = c.follows && self.plp == Some(c.id);
            self.plp = Some(c.id);
            // An enhanced PLP across core PLPs: the cells of the first core PLP, then those of the next ones.
            let first = plp.cells.len().saturating_sub(c.layers.map_or(0, |l| l.next_cells()));
            let Some(cells) = self.deinterleaver.frame(
                &c.ti,
                follows,
                coding.cells(),
                &plp.cells[..first],
                &plp.weights[..first],
                c.fec_block_start,
            ) else {
                self.state.failed = Some(NotDecoded::Signalling);
                if let Some(s) = &mut self.stream {
                    s.restart();
                }
                if let Some(l) = &mut self.layers {
                    l.restart();
                }
                self.count(c.first, false, 0, 0);
                self.gap = true;
                io.items_out::<FrameState>(1).push(Timed {
                    time: f.time,
                    value: self.state.clone(),
                });
                continue;
            };
            if cells.restart {
                if let Some(s) = &mut self.stream {
                    s.restart();
                }
                if let Some(l) = &mut self.layers {
                    l.restart();
                }
                self.gap = true;
            }
            let (mut blocks, mut good, mut iterations) = (0u32, 0u32, 0u32);
            let (state, pool, gap) = (&mut self.state, &mut self.pool, &mut self.gap);
            let bonded = c.bonded;
            let each = |r: BlockResult, packet: &[u8]| {
                blocks += 1;
                iterations += r.iterations;
                if r.good {
                    good += 1;
                    state.packets += 1;
                    state.outer_corrected += u64::from(r.outer_corrected);
                    // Annex K.2: up to five baseband packets in a row go to one RF channel, so the packets of the other
                    // channel may lie between any two of this one.
                    if *gap || bonded {
                        gaps.push(packets.len());
                        *gap = false;
                    }
                    packets.push(pool.make(packet.len(), f.time, |o| o.copy_from_slice(packet)));
                } else {
                    state.bad_packets += 1;
                    *gap = true;
                }
            };
            match (&mut self.stream, &mut self.layers, &c.layers) {
                (_, Some(l), Some(layers)) if enhanced.is_some() => l.frame(
                    CoreCells::from(&cells),
                    (&plp.cells[first..], &plp.weights[first..], follows),
                    (layers, c.ti.subframes_late()),
                    LDPC_ITERATIONS,
                    each,
                ),
                (Some(s), _, _) => s.frame(
                    cells.cells,
                    cells.weights,
                    cells.block_start,
                    LDPC_ITERATIONS,
                    each,
                ),
                _ => {}
            }
            self.count(c.first, true, blocks, good);
            self.state.iterations = iterations as f32 / blocks.max(1) as f32;
            self.state.pilot_snr_db = Some(plp.pilot_snr_db);
            // The enhanced PLP is measured after the core PLP is taken off, against its own constellation.
            let enhanced_mer = match (enhanced, &mut self.layers) {
                (Some(_), Some(l)) => Some(l.take_mer()),
                _ => None,
            };
            self.state.mer_db = enhanced_mer.unwrap_or_else(|| self.points.as_ref().and_then(|(_, points)| {
                let (mut signal, mut error) = (0f64, 0f64);
                let every = (plp.cells.len() / MER_CELLS).max(1);
                for (z, w) in plp.cells.iter().zip(&plp.weights).step_by(every) {
                    if *w > 0.0 {
                        let p = points.nearest(*z);
                        signal += f64::from(p.norm_sqr());
                        error += f64::from((*z - p).norm_sqr());
                    }
                }
                (signal > 0.0 && error > 0.0).then(|| (10.0 * (signal / error).log10()) as f32)
            }));
            if blocks != good {
                self.state.failed = Some(NotDecoded::Fec);
            } else if self.good {
                self.state.failed = None;
            }
            io.items_out::<FrameState>(1).push(Timed {
                time: f.time,
                value: self.state.clone(),
            });
        }
        *io.bytes_out(0) = packets;
        for at in gaps {
            io.output(0)
                .tag_here(at, TagKind::Discontinuity { lost: 0 });
        }
    }
}

/// The service list as `atsc3.lls` last read it. The lists are shared: a new list is made only when a table changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LlsState {
    /// LLS tables read.
    pub tables: u64,
    /// LLS tables refused (short, broken gzip or XML).
    pub bad: u64,
    /// Signed multi-tables (id 0xFE) arrived in the last [`SIGNED_HOLD_S`] seconds: the station signs its signalling.
    pub signed: bool,
    /// The BSIDs of the newest SLT.
    pub bsids: Arc<[u16]>,
    /// The services of the newest SLT.
    pub services: Arc<[SltService]>,
    /// The newest system time table.
    pub system_time: Option<Arc<SystemTime>>,
}

/// Seconds of signal time the signed flag holds after the last signed multi-table.
pub const SIGNED_HOLD_S: f64 = 10.0;

impl Item for LlsState {
    const KIND: &'static str = "atsc3.lls_state";
}

/// An LLS datagram that was read, and what it held: tables, refused tables, signed.
#[derive(Default)]
struct LlsSeen {
    bytes: Vec<u8>,
    tables: u64,
    refused: u64,
    signed: bool,
}

/// LLS datagrams remembered: one per table id and group.
const LLS_SEEN: usize = 16;

/// `atsc3.lls`: the low-level signalling on 224.0.23.60, UDP port 4937, plain tables and signed multi-tables. Input `in`: UDP
/// datagrams. Output `state`: [`LlsState`] after each datagram. A datagram equal to the last one of its table id and group
/// is counted and not read again.
#[derive(Default)]
pub struct Lls {
    state: LlsState,
    signed_at: Option<f64>,
    seen: Vec<LlsSeen>,
}

impl Lls {
    fn reset(&mut self) {
        self.state = LlsState::default();
        self.signed_at = None;
        self.seen.clear();
    }
}

impl Component for Lls {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc3.lls",
            vec![Port::typed::<UdpDatagram>("in")],
            vec![items::<LlsState>("state")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        Ok(vec![Format::new(Kind::typed::<LlsState>(), 0.0)])
    }
    /// The service list is of the multiplex of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.reset();
    }
    fn process(&mut self, io: &mut Io) {
        let lls = IpAddr::V4(Ipv4Addr::from(LLS_ADDR));
        for d in io.items::<UdpDatagram>(0) {
            if d.value.dst != lls || d.value.dst_port != LLS_PORT {
                continue;
            }
            let bytes = d.value.payload.bytes();
            let known = self.seen.iter().position(|s| s.bytes == bytes);
            let (tables, refused, signed) = match known {
                Some(at) => {
                    let s = &self.seen[at];
                    (s.tables, s.refused, s.signed)
                }
                None => {
                    let read = parse_lls(bytes);
                    let seen = (read.tables.len() as u64, read.refused as u64, read.signed);
                    for t in read.tables {
                        match t.table_id {
                            TABLE_SLT => {
                                if let Some(slt) = parse_slt(&t.xml) {
                                    self.state.bsids = slt.bsids.into();
                                    self.state.services = slt.services.into();
                                }
                            }
                            TABLE_SYSTEM_TIME => {
                                if let Some(st) = parse_system_time(&t.xml) {
                                    self.state.system_time = Some(Arc::new(st));
                                }
                            }
                            _ => {}
                        }
                    }
                    // The datagram takes the place of the last one of its table id and group.
                    let at = self
                        .seen
                        .iter()
                        .position(|s| s.bytes.get(..2) == bytes.get(..2))
                        .unwrap_or_else(|| {
                            if self.seen.len() == LLS_SEEN {
                                self.seen.remove(0);
                            }
                            self.seen.push(LlsSeen::default());
                            self.seen.len() - 1
                        });
                    let s = &mut self.seen[at];
                    s.bytes.clear();
                    s.bytes.extend_from_slice(bytes);
                    (s.tables, s.refused, s.signed) = seen;
                    seen
                }
            };
            self.state.tables += tables;
            self.state.bad += refused;
            if signed {
                self.signed_at = Some(d.time);
            }
            self.state.signed = self.signed_at.is_some_and(|t| d.time - t <= SIGNED_HOLD_S);
            io.items_out::<LlsState>(0).push(Timed {
                time: d.time,
                value: self.state.clone(),
            });
        }
    }
}

sdrkit_core::choice_enum! {
    /// What `atsc3.media` does with a component.
    pub enum ComponentState {
        /// Its access units are given as media packets.
        Given = ("given", "Given"),
        /// It belongs to a service that is not received.
        OtherService = ("other_service", "Of another service"),
        /// The received service has an earlier component of its kind, and that one is given.
        OtherTrack = ("other_track", "Another track of the service"),
        /// Its samples are encrypted.
        Encrypted = ("encrypted", "Encrypted"),
        /// The shared media form has no name for its codec, or the init segment lacks its decoder configuration.
        NoFormat = ("no_format", "No media form"),
        /// No session description that was read names its channel.
        Unbound = ("unbound", "In no session description"),
    }
}

/// One component seen: an LCT channel with an init segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentRow {
    /// Its LCT channel.
    pub channel: Channel,
    /// The sample entry type of its clear track (`hev1`, `ac-4`, `mp4a`).
    pub entry: [u8; 4],
    /// What it carries.
    pub kind: ComponentKind,
    /// The service whose session description names its channel.
    pub service: Option<u16>,
    /// What is done with it.
    pub state: ComponentState,
}

/// The lists of [`MediaState`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaLists {
    /// The components, in the order they appeared.
    pub components: Vec<ComponentRow>,
    /// The service layer signalling (TSI 0) per ROUTE session.
    pub sls: Vec<SlsSession>,
}

/// What `atsc3.media` has seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    /// ROUTE objects taken.
    pub objects: u64,
    /// Init segments read (repeats not counted).
    pub inits: u64,
    /// Media packets given out.
    pub samples: u64,
    /// Those of the video component.
    pub video_packets: u64,
    /// Those of the audio component.
    pub audio_packets: u64,
    /// Media segments of a given component that did not parse, and media segments that came before their init segment.
    pub dropped: u64,
    /// A signed signalling package arrived in the last [`SIGNED_HOLD_S`] seconds.
    pub signed: bool,
    /// The service whose components are given: the chosen one, or the automatic choice.
    pub receiving: Option<u16>,
    /// The components and the signalling sessions.
    pub lists: Arc<MediaLists>,
}

/// The service layer signalling received on TSI 0 of one ROUTE session. The counts are shared with every copy of the
/// state, so a signalling object that repeats makes no new list.
#[derive(Clone, Debug)]
pub struct SlsSession {
    /// The destination address of the session's datagrams.
    pub dst: IpAddr,
    /// Their UDP port.
    pub port: u16,
    counts: Arc<[AtomicU64; 4]>,
}

impl SlsSession {
    fn count(&self, at: usize) -> u64 {
        self.counts[at].load(Ordering::Relaxed)
    }
    fn add(&self, at: usize, n: u64) {
        self.counts[at].fetch_add(n, Ordering::Relaxed);
    }
    /// Objects received.
    pub fn objects(&self) -> u64 {
        self.count(0)
    }
    /// Documents read from them (USBD, S-TSID, MPD and others).
    pub fn documents(&self) -> u64 {
        self.count(1)
    }
    /// Objects that were signed packages. The signature is not verified.
    pub fn signed_packages(&self) -> u64 {
        self.count(2)
    }
    /// Gzip members in them that did not inflate.
    pub fn broken_gzip(&self) -> u64 {
        self.count(3)
    }
}

impl PartialEq for SlsSession {
    fn eq(&self, o: &SlsSession) -> bool {
        self.dst == o.dst && self.port == o.port && (0..4).all(|k| self.count(k) == o.count(k))
    }
}

impl Item for MediaState {
    const KIND: &'static str = "atsc3.media_state";
}

/// A signalling object that was read, by its TOI.
struct SlsSeen {
    toi: u64,
    bytes: Vec<u8>,
    read: SlsRead,
}

/// Signalling objects remembered per session.
const SLS_SEEN: usize = 8;

/// What is kept of the signalling of one session, beside its [`SlsSession`] of the same index.
#[derive(Default)]
struct SlsKept {
    seen: Vec<SlsSeen>,
    /// The entry of `seen` that the next new TOI replaces once the list is full.
    next: usize,
    /// The channels of the newest session description, in its order.
    channels: Vec<Channel>,
}

/// The least size of a buffer of a media packet, and the buffers made when the component is configured.
const MEDIA_UNIT: usize = 4096;
const MEDIA_READY: usize = 64;

/// One component: a channel with its init segment.
struct MediaComponent {
    channel: Channel,
    /// The init segment that `track` was read from.
    init: Vec<u8>,
    track: Track,
    info: TrackInfo,
    service: Option<u16>,
    given: bool,
}

sdrkit_core::commands! {
    /// The commands of `atsc3.media`.
    pub enum MediaCommand {
        /// Receive a service of the service list. An empty row receives the first visible linear TV service.
        ServiceSelect = SERVICE_SELECT_KIND.command,
    }
}

/// `atsc3.media`: the media of one service as media packets. Input `in`: ROUTE objects. Input `lls`: the service list.
/// Output `out`: [`MediaPacket`]s. Output `state`: [`MediaState`]. Command `service.select` chooses the service; a row
/// that is not in the service list changes nothing.
///
/// Objects of TSI 0 are the service layer signalling of their session: each is read in any of its forms ([`read_sls`]), its
/// documents and signed packages are counted, and its session description names the LCT channels of the service. An
/// object equal to the last one of its TOI is counted and not read again. The other objects are fragmented MP4: an init
/// segment is known by its first box (not every sender sets the code point) and gives the format of its component; a
/// repeated one is ignored. The access units of a media segment are given when its channel is the first video or the
/// first audio channel, in the order of the session description, of the service that is received, the track is not
/// encrypted and the shared media form names its codec. Each packet has the presentation and decoding time of its
/// sample in ticks of the media timescale, which the format states. An object in entity mode is read after its header
/// lines.
#[derive(Default)]
pub struct Media {
    tracks: Vec<MediaComponent>,
    sls: Vec<SlsKept>,
    services: Arc<[SltService]>,
    chosen: Option<u16>,
    pool: SlicePool<u8>,
    state: MediaState,
    signed_at: Option<f64>,
    time: f64,
    /// The state changed outside `process`.
    changed: bool,
}

impl Media {
    fn reset(&mut self) {
        self.tracks.clear();
        self.sls.clear();
        self.services = Arc::default();
        self.chosen = None;
        self.state = MediaState::default();
        self.signed_at = None;
        self.changed = false;
    }

    /// Binds every component to the service whose session description names its channel, chooses the components that
    /// are given and writes the component rows.
    fn rebind(&mut self) {
        let receiving = service_to_receive(self.chosen, &self.services);
        self.state.receiving = receiving;
        let lists = Arc::make_mut(&mut self.state.lists);
        // The signalling session of each service, by its index.
        let session_of = |s: &SltService| {
            let dst = s.sls_dst.parse::<IpAddr>().ok()?;
            lists
                .sls
                .iter()
                .position(|o| o.dst == dst && o.port == s.sls_port)
        };
        for t in &mut self.tracks {
            t.given = false;
            t.service = self.services.iter().find_map(|s| {
                let channels = &self.sls[session_of(s)?].channels;
                channels
                    .iter()
                    .any(|c| c.is(t.channel.dst, t.channel.port, t.channel.tsi))
                    .then_some(s.id)
            });
        }
        let channels = self
            .services
            .iter()
            .find(|s| Some(s.id) == receiving)
            .and_then(session_of)
            .map_or(&[][..], |at| &self.sls[at].channels[..]);
        let (mut video, mut audio) = (false, false);
        for c in channels {
            let Some(t) = self
                .tracks
                .iter_mut()
                .find(|t| t.info.format.is_some() && c.is(t.channel.dst, t.channel.port, t.channel.tsi))
            else {
                continue;
            };
            let taken = match t.info.kind {
                ComponentKind::Video => &mut video,
                ComponentKind::Audio => &mut audio,
                _ => continue,
            };
            t.given = !*taken;
            *taken = true;
        }
        lists.components.clear();
        lists.components.extend(self.tracks.iter().map(|t| ComponentRow {
            channel: t.channel,
            entry: t.info.entry,
            kind: t.info.kind,
            service: t.service,
            state: if t.info.encrypted {
                ComponentState::Encrypted
            } else if t.info.format.is_none() {
                ComponentState::NoFormat
            } else if t.given {
                ComponentState::Given
            } else if t.service.is_none() {
                ComponentState::Unbound
            } else if t.service != receiving {
                ComponentState::OtherService
            } else {
                ComponentState::OtherTrack
            },
        }));
    }

    /// Takes a signalling object of the session to `dst`, `port`. Returns whether the channels of the session changed.
    fn signalling(&mut self, dst: IpAddr, port: u16, toi: u64, bytes: &[u8], time: f64) -> bool {
        let found = self
            .state
            .lists
            .sls
            .iter()
            .position(|s| s.dst == dst && s.port == port);
        let at = found.unwrap_or_else(|| {
            let lists = Arc::make_mut(&mut self.state.lists);
            lists.sls.push(SlsSession {
                dst,
                port,
                counts: Arc::default(),
            });
            self.sls.push(SlsKept::default());
            lists.sls.len() - 1
        });
        let kept = &mut self.sls[at];
        let known = kept
            .seen
            .iter()
            .find(|o| o.toi == toi && o.bytes == bytes)
            .map(|o| o.read);
        let mut rebound = false;
        let read = known.unwrap_or_else(|| {
            let mut channels = None;
            let read = read_sls(bytes, &mut |doc| {
                if doc.kind == SlsKind::Stsid {
                    if let Ok(xml) = std::str::from_utf8(doc.xml) {
                        let mut list = Vec::new();
                        parse_stsid(xml, dst, port, &mut list);
                        channels = Some(list);
                    }
                }
            });
            if let Some(list) = channels.filter(|l| *l != kept.channels) {
                kept.channels = list;
                rebound = true;
            }
            let slot = match kept.seen.iter().position(|o| o.toi == toi) {
                Some(slot) => slot,
                None if kept.seen.len() < SLS_SEEN => {
                    kept.seen.push(SlsSeen {
                        toi,
                        bytes: Vec::new(),
                        read,
                    });
                    kept.seen.len() - 1
                }
                None => {
                    kept.next = (kept.next + 1) % SLS_SEEN;
                    kept.next
                }
            };
            let o = &mut kept.seen[slot];
            o.toi = toi;
            o.read = read;
            o.bytes.clear();
            o.bytes.extend_from_slice(bytes);
            read
        });
        let s = &self.state.lists.sls[at];
        s.add(0, 1);
        s.add(1, read.documents as u64);
        s.add(3, read.broken_gzip as u64);
        if read.signed {
            s.add(2, 1);
            self.signed_at = Some(time);
        }
        rebound
    }
}

impl Component for Media {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc3.media",
            vec![
                Port::typed::<RouteObjectItem>("in"),
                Port::typed::<LlsState>("lls"),
            ],
            vec![items::<MediaPacket>("out"), items::<MediaState>("state")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The media, the signalling and the choice of the service are of the multiplex of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.reset();
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        self.pool = SlicePool::with_ready(MEDIA_UNIT, MEDIA_READY);
        Ok(vec![
            Format::new(Kind::typed::<MediaPacket>(), 0.0),
            Format::new(Kind::typed::<MediaState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut rebind = false;
        let mut changed = std::mem::take(&mut self.changed);
        if let Some(l) = io.items::<LlsState>(1).last() {
            self.time = l.time;
            if !Arc::ptr_eq(&l.value.services, &self.services) {
                self.services = Arc::clone(&l.value.services);
                rebind = true;
            }
        }
        for o in io.items::<RouteObjectItem>(0) {
            changed = true;
            let time = o.time;
            self.time = time;
            self.state.objects += 1;
            let (dst, port, tsi) = (o.value.dst, o.value.dst_port, o.value.tsi);
            let bytes = o.value.data.bytes();
            if tsi == 0 {
                if self.signalling(dst, port, o.value.toi, bytes, time) {
                    rebind = true;
                }
                continue;
            }
            let at = self
                .tracks
                .iter()
                .position(|t| t.channel.is(dst, port, tsi));
            // The session description says which code points of the channel are in entity mode. A channel that none
            // names has the code points of the standard only.
            let entity = self
                .sls
                .iter()
                .flat_map(|s| &s.channels)
                .find(|c| c.is(dst, port, tsi))
                .map_or(matches!(o.value.codepoint, 2 | 9), |c| {
                    c.entity_mode(o.value.codepoint)
                });
            let data = if entity { entity_body(bytes) } else { bytes };
            match mp4::segment_kind(data) {
                Segment::Init => {
                    // A repeated init segment is not read again.
                    if at.is_some_and(|at| self.tracks[at].init == data) {
                        continue;
                    }
                    let Ok(track) = Track::parse_init(data) else {
                        self.state.dropped += 1;
                        continue;
                    };
                    let info = describe(data, &track);
                    match at {
                        Some(at) => {
                            let t = &mut self.tracks[at];
                            (t.track, t.info) = (track, info);
                            t.init.clear();
                            t.init.extend_from_slice(data);
                        }
                        None => self.tracks.push(MediaComponent {
                            channel: Channel {
                                dst,
                                port,
                                tsi,
                                entity: 0,
                            },
                            init: data.to_vec(),
                            track,
                            info,
                            service: None,
                            given: false,
                        }),
                    }
                    self.state.inits += 1;
                    // The component is bound at once, so the media segments of this pass that follow it are given.
                    self.rebind();
                    rebind = false;
                }
                Segment::Media => {
                    if rebind {
                        self.rebind();
                        rebind = false;
                    }
                    let Some(t) = at.map(|at| &self.tracks[at]) else {
                        self.state.dropped += 1;
                        continue;
                    };
                    let Some(format) = t.info.format.as_ref().filter(|_| t.given) else {
                        continue;
                    };
                    let pool = &mut self.pool;
                    let out = io.items_out::<MediaPacket>(0);
                    if out.capacity() < MEDIA_READY {
                        out.reserve(MEDIA_READY - out.len());
                    }
                    let r = mp4::samples(data, &t.track, |s| {
                        // The buffers have sizes of powers of two, so a buffer serves access units of many sizes.
                        let n = s.data.len();
                        let size = n.next_power_of_two().max(MEDIA_UNIT);
                        let buffer = pool.at_least(size, |b| b[..n].copy_from_slice(s.data));
                        let packet = Packet::slice(&buffer, 0, n, time)
                            .stamped(Some(s.pts), Some(s.dts), s.keyframe)
                            .clean_if(s.keyframe && clean_start(format, s.data));
                        out.push(Timed {
                            time,
                            value: MediaPacket::new(packet, Arc::clone(format)),
                        });
                    });
                    match r {
                        Ok(n) => {
                            self.state.samples += n as u64;
                            match t.info.kind {
                                ComponentKind::Video => self.state.video_packets += n as u64,
                                _ => self.state.audio_packets += n as u64,
                            }
                        }
                        Err(_) => self.state.dropped += 1,
                    }
                }
                Segment::Other => {}
            }
        }
        if rebind {
            self.rebind();
            changed = true;
        }
        if !changed {
            return;
        }
        let time = self.time;
        self.state.signed = self.signed_at.is_some_and(|t| time - t <= SIGNED_HOLD_S);
        io.items_out::<MediaState>(1).push(Timed {
            time,
            value: self.state.clone(),
        });
    }
    fn commands(&self) -> &Commands {
        MediaCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match MediaCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MediaCommand::ServiceSelect => {
                let row = args.arg(&SERVICE_ROW)?;
                let chosen = if row.is_empty() {
                    None
                } else {
                    let id = row
                        .parse::<u16>()
                        .ok()
                        .filter(|&id| self.services.iter().any(|s| s.id == id));
                    // `atsc3.report` refuses a row that is not in the list. The choice stays.
                    if id.is_none() {
                        return Ok(None);
                    }
                    id
                };
                self.chosen = chosen;
                self.rebind();
                self.changed = true;
            }
        }
        Ok(None)
    }
}
