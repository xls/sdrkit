//! `fec.trellis`: trellis-coded modulation decoded with the trellis decoders of `sdrkit-algo-fec`: ATSC 8-VSB over a sliding window
//! with interleaved trellises, and the DMR and P25 4-level trellis codes a code word at a time.

use super::common::Framer;
use sdrkit_algo_fec::conv::tcm::{self, TcmCodec, DIBITS};
use sdrkit_algo_fec::conv::{Ends, LabelMetrics, Trellis, TrellisDecoder};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::stream::Data;
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Kind, Port, TagKind};
use std::collections::VecDeque;

sdrkit_core::choice_enum! {
    /// The trellis code of [`TrellisDecode`].
    pub enum TrellisName {
        /// ATSC 8-VSB.
        Atsc = ("atsc", "ATSC 8-VSB"),
        /// DMR rate 3/4.
        Dmr = ("dmr", "DMR rate 3/4"),
        /// P25 rate 3/4.
        P2534 = ("p25_3_4", "P25 rate 3/4"),
        /// P25 rate 1/2.
        P2512 = ("p25_1_2", "P25 rate 1/2"),
    }
}

/// The 4-level code of a `trellis` setting, `None` for `atsc`.
fn tcm_code(name: TrellisName) -> Option<tcm::TcmCode> {
    match name {
        TrellisName::Dmr => Some(tcm::DMR_3_4),
        TrellisName::P2534 => Some(tcm::P25_3_4),
        TrellisName::P2512 => Some(tcm::P25_1_2),
        TrellisName::Atsc => None,
    }
}

/// Metric units per unit of squared level distance.
const METRIC_SCALE: f32 = 16.0;

/// One of the interleaved trellises: its window of received values and its decided symbols.
#[derive(Debug, Default)]
struct Lane {
    win: Vec<f32>,
    /// Values at the front of `win` already decided (kept for the decoder's run-in).
    back: usize,
    decided: VecDeque<u8>,
}

const TRELLIS: Setting<field::Choice<TrellisName>> =
    Setting::choice("trellis", "Trellis", TrellisName::Atsc);
const PHASES: Setting<field::I64> =
    Setting::new("phases", "Interleaved trellises", "", 12).range(1.0, 64.0, 1.0);
const CHUNK: Setting<field::I64> =
    Setting::new("chunk", "Symbols decided at a time", "", 256).range(16.0, 65_536.0, 1.0);
const DEPTH: Setting<field::I64> = Setting::new("depth", "Run-in", "", 32)
    .range(4.0, 4096.0, 1.0)
    .help("Symbols decoded before and after each chunk and thrown away; at most the chunk.");

sdrkit_core::settings! {
    /// The settings of [`TrellisDecode`].
    pub enum TrellisDecodeSetting {
        /// Trellis.
        Trellis = TRELLIS,
        /// Interleaved trellises.
        Phases = PHASES,
        /// Symbols decided at a time.
        Chunk = CHUNK,
        /// Run-in.
        Depth = DEPTH,
    }
}

/// Decodes the ATSC 8-VSB trellis code: received levels in (`Real` for `fec.trellis`, the real part of `Symbols` for
/// `fec.trellis_symbols`; the 8 levels at ±1, ±3, ±5,
/// ±7), `HardBits` out, two per symbol: `Z2` (the precoded bit, sliced once the coded label is known) then `Z1` (= `X1`). Undoing
/// the precoder (`X2 = Z2` XOR the `Z2` of 12 symbols earlier) is left to the stack.
///
/// `phases` trellises are interleaved symbol by symbol (12 for ATSC: symbol `i` belongs to trellis `i mod 12`, counted from the
/// stream start or the last `FrameStart` tag). Each trellis is decoded over a sliding window: `chunk` symbols are decided at a
/// time, with `depth` symbols of run-in before them and `depth` after, from an unknown state, so any cut of the stream produces the
/// same bits. A `FrameStart` tag (a data field start) decides every symbol held with the values there are, then starts the
/// interleaving again at its symbol.
///
/// `dmr`, `p25_3_4` and `p25_1_2` decode code words of 98 symbol levels (nominally ±1 and ±3) by squared distance
/// ([`TcmCodec::decode_levels`]), producing 144 (rate 3/4) or 96 (rate 1/2) bits each. Code words follow each other from the
/// stream start; a `FrameStart` tag starts a new one at its symbol (a partial word before it is dropped). `phases`, `chunk` and
/// `depth` are not used.
#[derive(Debug)]
pub struct TrellisDecode {
    on_symbols: bool,
    trellis: TrellisName,
    phases: usize,
    chunk: usize,
    depth: usize,
    t: Trellis,
    dec: Option<TrellisDecoder>,
    lanes: Vec<Lane>,
    phase: usize,
    emit: usize,
    metrics: Vec<i32>,
    slice: Vec<u8>,
    xs: Vec<u8>,
    tcm: Option<TcmCodec>,
    framer: Framer<f32>,
    levels: Vec<f32>,
    info: Vec<u8>,
}

impl Default for TrellisDecode {
    fn default() -> Self {
        TrellisDecode {
            on_symbols: false,
            trellis: TrellisName::Atsc,
            phases: PHASES.default as _,
            chunk: CHUNK.default as _,
            depth: DEPTH.default as _,
            t: Trellis::atsc_8vsb(),
            dec: None,
            lanes: Vec::new(),
            phase: 0,
            emit: 0,
            metrics: Vec::new(),
            slice: Vec::new(),
            xs: Vec::new(),
            tcm: None,
            framer: Framer::default(),
            levels: Vec::new(),
            info: Vec::new(),
        }
    }
}

impl TrellisDecode {
    /// Decides the symbols `[back, to)` of lane `p`'s window, run-in included.
    fn decide(&mut self, p: usize, to: usize) {
        let Some(dec) = self.dec.as_mut() else {
            return;
        };
        let lane = &mut self.lanes[p];
        let steps = lane.win.len();
        let labels = self.t.labels();
        // Metric of each label: the nearer of its two levels (2 label − 7 for Z2 = 0, 2 label + 1 for Z2 = 1).
        for (i, &r) in lane.win.iter().enumerate() {
            for l in 0..labels {
                let d0 = r - (2 * l as i32 - 7) as f32;
                let d1 = d0 - 8.0;
                let (a, b) = (d0 * d0, d1 * d1);
                self.metrics[i * labels + l] = -((a.min(b) * METRIC_SCALE) as i32);
                self.slice[i * labels + l] = (b < a) as u8;
            }
        }
        let mut bm = LabelMetrics {
            labels,
            metrics: &self.metrics[..steps * labels],
        };
        if dec
            .decode(&mut bm, steps, Ends::Open, None, &mut self.xs)
            .is_err()
        {
            return;
        }
        // The start state of the decided path: the one whose labels for these inputs score best.
        let mut best = (i64::MIN, 0usize);
        for s0 in 0..self.t.states() {
            let (mut s, mut m) = (s0, 0i64);
            for (i, &x) in self.xs[..steps].iter().enumerate() {
                m += self.metrics[i * labels + self.t.label(s, x as usize)] as i64;
                s = self.t.next(s, x as usize);
            }
            if m > best.0 {
                best = (m, s0);
            }
        }
        let mut s = best.1;
        for (i, &x) in self.xs[..to].iter().enumerate() {
            let l = self.t.label(s, x as usize);
            if i >= lane.back {
                lane.decided
                    .push_back((self.slice[i * labels + l] << 1) | x);
            }
            s = self.t.next(s, x as usize);
        }
    }

    /// Adds one received value.
    fn push(&mut self, r: f32) {
        let p = self.phase;
        self.lanes[p].win.push(r);
        let lane = &self.lanes[p];
        if lane.win.len() == lane.back + self.chunk + self.depth {
            let to = lane.back + self.chunk;
            self.decide(p, to);
            let lane = &mut self.lanes[p];
            let keep = 2 * self.depth;
            let drop = lane.win.len() - keep;
            lane.win.drain(..drop);
            lane.back = self.depth;
        }
        self.phase = (self.phase + 1) % self.phases;
    }

    /// Decides everything held and starts the interleaving again.
    fn flush(&mut self) {
        for p in 0..self.phases {
            let len = self.lanes[p].win.len();
            if len > self.lanes[p].back {
                self.decide(p, len);
            }
            let lane = &mut self.lanes[p];
            lane.win.clear();
            lane.back = 0;
        }
    }

    /// Decodes whole 4-level code words.
    fn process_words(&mut self, io: &mut Io) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let mut levels = std::mem::take(&mut self.levels);
        levels.clear();
        match &input.data {
            Data::Real(x) => levels.extend_from_slice(x),
            Data::Symbols(x) => levels.extend(x.iter().map(|s| s.re)),
            _ => {}
        }
        let mut out = std::mem::take(io.hard_bits_out(0));
        if let Some(codec) = self.tcm.as_mut() {
            let info = &mut self.info;
            self.framer.feed(&levels, first, tags, |word, _| {
                if codec.decode_levels(word, info).is_ok() {
                    out.extend_from_slice(info);
                }
            });
        }
        *io.hard_bits_out(0) = out;
        self.levels = levels;
    }

    /// Moves the decided symbols to `out` in stream order.
    fn emit(&mut self, out: &mut Vec<u8>) {
        while let Some(v) = self.lanes[self.emit].decided.pop_front() {
            out.push(v >> 1);
            out.push(v & 1);
            self.emit = (self.emit + 1) % self.phases;
        }
    }
}

impl TrellisDecode {
    /// `fec.trellis_symbols`: the decoder on the real parts of `Symbols`.
    pub fn on_symbols() -> Self {
        TrellisDecode {
            on_symbols: true,
            ..Self::default()
        }
    }
}

impl Component for TrellisDecode {
    fn info(&self) -> ComponentInfo {
        let (name, kind) = if self.on_symbols {
            ("fec.trellis_symbols", Kind::Symbols)
        } else {
            ("fec.trellis", Kind::Real)
        };
        ComponentInfo::processor(
            name,
            vec![Port::new("in", kind)],
            vec![Port::hard_bits("out")],
        )
        .with_settings(TrellisDecodeSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(code) = tcm_code(self.trellis) {
            let codec = TcmCodec::new(code).map_err(crate::stage)?;
            self.framer.reset(DIBITS);
            self.info = vec![0; code.info_bits()];
            self.levels = Vec::with_capacity(4096);
            self.tcm = Some(codec);
            self.dec = None;
            let rate = inputs[0].rate * code.info_bits() as f64 / DIBITS as f64;
            return Ok(vec![Format::hard_bits(rate)]);
        }
        self.tcm = None;
        if self.depth > self.chunk {
            return Err(SettingError::Conflict {
                id: TrellisDecodeSetting::Depth.id(),
                with: TrellisDecodeSetting::Chunk.id(),
            }
            .into());
        }
        self.t = Trellis::atsc_8vsb();
        let steps = self.chunk + 2 * self.depth;
        self.dec = Some(TrellisDecoder::new(&self.t, steps).map_err(crate::stage)?);
        self.lanes = (0..self.phases)
            .map(|_| Lane {
                win: Vec::with_capacity(steps),
                back: 0,
                decided: VecDeque::with_capacity(steps + 1),
            })
            .collect();
        self.metrics = vec![0; steps * self.t.labels()];
        self.slice = vec![0; steps * self.t.labels()];
        self.xs = vec![0; steps];
        (self.phase, self.emit) = (0, 0);
        Ok(vec![Format::hard_bits(inputs[0].rate * 2.0)])
    }

    fn process(&mut self, io: &mut Io) {
        if self.tcm.is_some() {
            self.process_words(io);
            return;
        }
        let input = io.input(0);
        let first = input.first();
        let mut out = std::mem::take(io.hard_bits_out(0));
        let mut starts = input
            .tags()
            .iter()
            .filter(|t| matches!(t.kind, TagKind::FrameStart { .. }))
            .map(|t| t.at.saturating_sub(first) as usize)
            .peekable();
        let mut run = |me: &mut Self, at: usize| {
            if starts.peek() == Some(&at) {
                while starts.peek() == Some(&at) {
                    starts.next();
                }
                me.flush();
                me.emit(&mut out);
                (me.phase, me.emit) = (0, 0);
            }
        };
        let len = match &input.data {
            Data::Real(x) => {
                for (i, &r) in x.iter().enumerate() {
                    run(self, i);
                    self.push(r);
                }
                x.len()
            }
            Data::Symbols(x) => {
                for (i, s) in x.iter().enumerate() {
                    run(self, i);
                    self.push(s.re);
                }
                x.len()
            }
            _ => 0,
        };
        // A frame start just after the last value: what is held is complete.
        run(self, len);
        self.emit(&mut out);
        *io.hard_bits_out(0) = out;
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TrellisDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TrellisDecodeSetting::Trellis => self.trellis = TRELLIS.read(id, v)?,
            TrellisDecodeSetting::Phases => self.phases = PHASES.read(id, v)? as usize,
            TrellisDecodeSetting::Chunk => self.chunk = CHUNK.read(id, v)? as usize,
            TrellisDecodeSetting::Depth => self.depth = DEPTH.read(id, v)? as usize,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (
                TrellisDecodeSetting::Trellis,
                Value::Choice(self.trellis.index()),
            ),
            (TrellisDecodeSetting::Phases, Value::I64(self.phases as i64)),
            (TrellisDecodeSetting::Chunk, Value::I64(self.chunk as i64)),
            (TrellisDecodeSetting::Depth, Value::I64(self.depth as i64)),
        ])
    }
}
