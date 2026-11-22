//! The ATSC 3.0 components of this crate: `atsc3.sync` (bootstraps), `atsc3.frame` (frames to the cells of a PLP), `atsc3.fec`
//! (time de-interleaving and FEC blocks to baseband packets), `atsc3.lls` (the service list) and `atsc3.media` (ROUTE objects to media samples). The link and delivery layers between them are the
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
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Packet,
    PacketPool, Port, RateFrom, RateRule, TagKind, Timed,
};
use sdrkit_proto_ipcast::components::{RouteObjectItem, UdpDatagram};
use sdrkit_proto_ipcast::mp4::{self, Segment, Track};
use sdrkit_proto_ipcast::sls::read_sls;
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr};
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

/// The service list as `atsc3.lls` last read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LlsState {
    /// LLS tables read.
    pub tables: u64,
    /// LLS tables refused (short, broken gzip or XML).
    pub bad: u64,
    /// Signed multi-tables (id 0xFE) arrived in the last [`SIGNED_HOLD_S`] seconds: the station signs its signalling.
    pub signed: bool,
    /// The BSIDs of the newest SLT.
    pub bsids: Vec<u16>,
    /// The services of the newest SLT.
    pub services: Vec<SltService>,
    /// The newest system time table.
    pub system_time: Option<SystemTime>,
}

/// Seconds of signal time the signed flag holds after the last signed multi-table.
pub const SIGNED_HOLD_S: f64 = 10.0;

impl Item for LlsState {
    const KIND: &'static str = "atsc3.lls_state";
}

/// `atsc3.lls`: the low-level signalling on 224.0.23.60, UDP port 4937, plain tables and signed multi-tables. Input `in`: UDP
/// datagrams. Output `state`: [`LlsState`] after each datagram.
#[derive(Default)]
pub struct Lls {
    state: LlsState,
    signed_at: Option<f64>,
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
        self.state = LlsState::default();
        self.signed_at = None;
        Ok(vec![Format::new(Kind::typed::<LlsState>(), 0.0)])
    }
    /// The service list is of the multiplex of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.state = LlsState::default();
        self.signed_at = None;
    }
    fn process(&mut self, io: &mut Io) {
        let lls = IpAddr::V4(Ipv4Addr::from(LLS_ADDR));
        for d in io.items::<UdpDatagram>(0) {
            if d.value.dst != lls || d.value.dst_port != LLS_PORT {
                continue;
            }
            let read = parse_lls(d.value.payload.bytes());
            self.state.bad += read.refused as u64;
            if read.signed {
                self.signed_at = Some(d.time);
            }
            self.state.signed = self.signed_at.is_some_and(|t| d.time - t <= SIGNED_HOLD_S);
            for t in read.tables {
                self.state.tables += 1;
                match t.table_id {
                    TABLE_SLT => {
                        if let Some(slt) = parse_slt(&t.xml) {
                            self.state.bsids = slt.bsids;
                            self.state.services = slt.services;
                        }
                    }
                    TABLE_SYSTEM_TIME => {
                        if let Some(st) = parse_system_time(&t.xml) {
                            self.state.system_time = Some(st);
                        }
                    }
                    _ => {}
                }
            }
            io.items_out::<LlsState>(0).push(Timed {
                time: d.time,
                value: self.state.clone(),
            });
        }
    }
}

/// One media sample of a component.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaSample {
    /// The ROUTE session it came in.
    pub tsi: u64,
    /// The FourCC of its codec (`hvc1`, `avc1`, `aac `, `ac-4` ...).
    pub fourcc: [u8; 4],
    /// Decoding time, seconds on the component's timeline.
    pub dts_s: f64,
    /// Presentation time, seconds.
    pub pts_s: f64,
    /// A sync sample.
    pub keyframe: bool,
    /// The sample.
    pub data: Packet,
}

impl Item for MediaSample {
    const KIND: &'static str = "atsc3.media";
}

/// What `atsc3.media` has seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    /// ROUTE objects taken.
    pub objects: u64,
    /// Init segments read (repeats not counted).
    pub inits: u64,
    /// Media samples given out.
    pub samples: u64,
    /// Media segments that came before their init segment, or that did not parse.
    pub dropped: u64,
    /// The FourCC of each component (by session), in the order they appeared.
    pub components: Vec<(u64, [u8; 4])>,
    /// The service layer signalling (TSI 0) per ROUTE session.
    pub sls: Vec<SlsSession>,
    /// A signed signalling package arrived in the last [`SIGNED_HOLD_S`] seconds.
    pub signed: bool,
}

/// The service layer signalling received on TSI 0 of one ROUTE session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlsSession {
    /// The destination address of the session's datagrams.
    pub dst: IpAddr,
    /// Their UDP port.
    pub port: u16,
    /// Objects received.
    pub objects: u64,
    /// Documents read from them (USBD, S-TSID, MPD and others).
    pub documents: u64,
    /// Objects that were signed packages. The signature is not verified.
    pub signed_packages: u64,
    /// Gzip members in them that did not inflate.
    pub broken_gzip: u64,
}

impl Item for MediaState {
    const KIND: &'static str = "atsc3.media_state";
}

/// `atsc3.media`: fragmented MP4 in ROUTE objects to media samples. Input `in`: ROUTE objects. Output `out`: [`MediaSample`]s.
/// Output `state`: [`MediaState`]. An init segment is known by its first box (not every sender sets the code point). A repeated
/// one is ignored. Media before the first init segment of its session cannot be played and is dropped. Objects of TSI 0 are the
/// service layer signalling of their session: each is read in any of its forms ([`read_sls`]) and its documents and signed
/// packages are counted per destination; they are not read as media.
#[derive(Default)]
pub struct Media {
    tracks: Vec<(u64, Track)>,
    pool: PacketPool,
    state: MediaState,
    signed_at: Option<f64>,
}

impl Component for Media {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atsc3.media",
            vec![Port::typed::<RouteObjectItem>("in")],
            vec![items::<MediaSample>("out"), items::<MediaState>("state")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The media and the signalling are of the multiplex of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.tracks.clear();
        self.state = MediaState::default();
        self.signed_at = None;
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tracks.clear();
        self.state = MediaState::default();
        self.signed_at = None;
        Ok(vec![
            Format::new(Kind::typed::<MediaSample>(), 0.0),
            Format::new(Kind::typed::<MediaState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let objects = io.items::<RouteObjectItem>(0);
        if objects.is_empty() {
            return;
        }
        let mut time = 0.0;
        for o in objects {
            time = o.time;
            self.state.objects += 1;
            let tsi = o.value.tsi;
            if tsi == 0 {
                let (dst, port) = (o.value.dst, o.value.dst_port);
                let read = read_sls(o.value.data.bytes(), &mut |_| {});
                let at = match self
                    .state
                    .sls
                    .iter()
                    .position(|s| s.dst == dst && s.port == port)
                {
                    Some(at) => at,
                    None => {
                        self.state.sls.push(SlsSession {
                            dst,
                            port,
                            objects: 0,
                            documents: 0,
                            signed_packages: 0,
                            broken_gzip: 0,
                        });
                        self.state.sls.len() - 1
                    }
                };
                let s = &mut self.state.sls[at];
                s.objects += 1;
                s.documents += read.documents as u64;
                s.broken_gzip += read.broken_gzip as u64;
                if read.signed {
                    s.signed_packages += 1;
                    self.signed_at = Some(o.time);
                }
                continue;
            }
            let data = o.value.data.bytes();
            match mp4::segment_kind(data) {
                Segment::Init => {
                    let Ok(track) = Track::parse_init(data) else {
                        self.state.dropped += 1;
                        continue;
                    };
                    match self.tracks.iter_mut().find(|t| t.0 == tsi) {
                        Some(t) if t.1 == track => {}
                        Some(t) => {
                            t.1 = track;
                            self.state.inits += 1;
                        }
                        None => {
                            self.state.components.push((tsi, track.codec_fourcc()));
                            self.tracks.push((tsi, track));
                            self.state.inits += 1;
                        }
                    }
                }
                Segment::Media => {
                    let Some((_, track)) = self.tracks.iter().find(|t| t.0 == tsi) else {
                        self.state.dropped += 1;
                        continue;
                    };
                    let scale = track.timescale.max(1) as f64;
                    let fourcc = track.codec_fourcc();
                    let pool = &mut self.pool;
                    let out = io.items_out::<MediaSample>(0);
                    let r = mp4::samples(data, track, |s| {
                        out.push(Timed {
                            time,
                            value: MediaSample {
                                tsi,
                                fourcc,
                                dts_s: s.dts as f64 / scale,
                                pts_s: s.pts as f64 / scale,
                                keyframe: s.keyframe,
                                data: pool.make(s.data.len(), time, |b| b.copy_from_slice(s.data)),
                            },
                        })
                    });
                    match r {
                        Ok(n) => self.state.samples += n as u64,
                        Err(_) => self.state.dropped += 1,
                    }
                }
                Segment::Other => {}
            }
        }
        self.state.signed = self.signed_at.is_some_and(|t| time - t <= SIGNED_HOLD_S);
        io.items_out::<MediaState>(1).push(Timed {
            time,
            value: self.state.clone(),
        });
    }
}
