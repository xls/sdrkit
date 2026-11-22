//! The decoding of one slot by any mode, the slot decoder as a framework decoder (in line for a recording, on a helper thread for a
//! live radio, with a backlog of at most 6 slots, JS8 8), the count of slots a cutter gave its decoder (an early slot is cut only
//! when the decoder is idle), and a runner on the same helper threads for the dial chains of whole-span decoding.

use crate::mode::Mode;
use crate::proto::msg77::CallHashes;
use crate::proto::wsprmsg::WsprHashes;
use crate::rx::js8::Js8Frame;
use crate::rx::ftmem::FtMemory;
use crate::rx::slot::{Decode, Slot};
use crate::rx::{ft, js8, msk144, slowmodes};
use crate::typed::SlotDecoderFields;
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    ConfigError, LockState, Schema, Setting, SettingError, SettingId, SettingSet, Status, Value,
    Values,
};
use sdrkit_graph::context::Ticket;
use sdrkit_graph::{
    values_of, Decoder, Format, Helper, Kind, Outbox, Pool, Port, RateFrom, RunContext, Timed,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Decoder options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodeOptions {
    /// Upside down too (FT8, FT4, FT2, WSPR).
    pub mirrored: bool,
    /// The deep stage.
    pub deep: bool,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        DecodeOptions {
            mirrored: true,
            deep: true,
        }
    }
}

/// The result of a decoded slot (stream kind `wsjt.slot_result`).
#[derive(Clone, Debug, Default)]
pub struct SlotResult {
    /// The mode.
    pub mode: Option<Mode>,
    /// Slot start, UTC.
    pub utc: f64,
    /// Decodes (all modes but JS8).
    pub decodes: Vec<Decode>,
    /// JS8 frames.
    pub frames: Vec<Js8Frame>,
    /// JS8 candidates that did not decode.
    pub failed: u64,
    /// Decode time, ms.
    pub ms: f64,
    /// A slot-search window.
    pub search: bool,
    /// The sub-mode label.
    pub label: String,
    /// JS8 sub-mode.
    pub submode: u8,
    /// The period, s.
    pub period: f64,
    /// The decodes of an early slot (the slot is still being received): shown, not counted.
    pub early: bool,
}

/// The state of a mode's decoder: its hash tables and its options.
#[derive(Clone, Debug, Default)]
pub struct DecodeState {
    /// The 77-bit callsign hashes.
    pub calls: CallHashes,
    /// The WSPR hashes.
    pub wspr: WsprHashes,
    /// The options.
    pub opt: DecodeOptions,
    /// What FT8, FT4 and FT2 keep across slots: the failed candidates and the calls heard.
    pub ft: FtMemory,
}

impl DecodeState {
    /// Forgets the hash tables (Clear).
    pub fn clear(&mut self) {
        self.calls.clear();
        self.wspr.clear();
        self.ft.clear();
    }
}

/// Decodes one slot (any mode) with `state`.
pub fn decode_slot(slot: &Slot, state: &mut DecodeState) -> SlotResult {
    let t = std::time::Instant::now();
    let mode = slot.sub.mode;
    let mut r = SlotResult {
        mode: Some(mode),
        utc: slot.utc,
        search: slot.search,
        label: slot.sub.label(),
        submode: slot.sub.sub,
        period: slot.sub.period,
        early: slot.early,
        ..Default::default()
    };
    let opt = state.opt;
    match mode {
        Mode::Ft8 | Mode::Ft4 | Mode::Ft2 => {
            state.ft.begin_slot(slot.utc, slot.sub.period, slot.dial_hz);
            r.decodes = ft::decode(
                slot,
                &mut ft::Ctx {
                    hashes: &mut state.calls,
                    mirrored: opt.mirrored,
                    deep: opt.deep,
                    memory: Some(&mut state.ft),
                },
            );
            if !slot.search {
                for d in &r.decodes {
                    state.ft.heard.take(slot.utc, &d.text);
                }
            }
        }
        Mode::Msk144 => {
            r.decodes = msk144::decode(slot, &mut state.calls);
        }
        Mode::Js8 => {
            let mut failed = 0;
            r.frames = js8::decode(slot, &mut failed);
            r.failed = failed;
        }
        m => {
            let mut ctx = slowmodes::SlowCtx {
                hashes: &mut state.calls,
                wspr: &mut state.wspr,
                mirrored: opt.mirrored,
            };
            r.decodes = match m {
                Mode::Wspr => slowmodes::decode_wspr(slot, &mut ctx),
                Mode::Jt65 => slowmodes::decode_jt65(slot, &mut ctx),
                Mode::Jt9 => slowmodes::decode_jt9(slot, &mut ctx),
                Mode::Jt4 => slowmodes::decode_jt4(slot, &mut ctx),
                Mode::Q65 => slowmodes::decode_q65(slot, &mut ctx),
                _ => slowmodes::decode_fst4(slot, &mut ctx),
            };
        }
    }
    for d in r.decodes.iter_mut() {
        if d.dial == 0.0 {
            d.dial = slot.dial_hz;
        }
    }
    for f in r.frames.iter_mut() {
        if f.dial == 0.0 {
            f.dial = slot.dial_hz;
        }
    }
    r.ms = t.elapsed().as_secs_f64() * 1000.0;
    r
}

/// The slots a cutter gave its decoder and the newest one decoded, shared by the two components. A slot dropped from a full
/// backlog counts as decoded once a newer one is.
#[derive(Debug, Default)]
pub struct SlotQueue {
    cut: AtomicU64,
    done: AtomicU64,
}

impl SlotQueue {
    /// A new queue, shared.
    pub fn new() -> Arc<SlotQueue> {
        Arc::new(SlotQueue::default())
    }
    /// The number of the next slot cut (from 1).
    pub fn next(&self) -> u64 {
        self.cut.fetch_add(1, Ordering::AcqRel) + 1
    }
    /// The slot numbered `seq` is decoded (0: a slot without a number).
    pub fn done(&self, seq: u64) {
        self.done.fetch_max(seq, Ordering::AcqRel);
    }
    /// Whether the decoder has no slot queued or being decoded.
    pub fn idle(&self) -> bool {
        self.done.load(Ordering::Acquire) >= self.cut.load(Ordering::Acquire)
    }
}

pub(crate) const MIRRORED: Setting<field::Bool> =
    Setting::new("mirrored", "Upside down too", "", true)
        .help("Also find stations sent on the other sideband (FT8, FT4, FT2 and WSPR).");
pub(crate) const DEEP: Setting<field::Bool> = Setting::new("deep", "Deep decoding", "", true)
    .help(
        "A second stage with coherent soft bits, OSD and a-priori CQ decoding. FT8, FT4 and FT2 also add the soft bits of \
         candidates that failed in earlier slots, and assume the calls heard on the band.",
    );

sdrkit_core::settings! {
    /// The settings of [`SlotDecoder`].
    pub enum SlotDecoderSetting {
        /// Upside down too.
        Mirrored = MIRRORED,
        /// Deep decoding.
        Deep = DEEP,
    }
}

/// The slot decoder of one mode as a framework decoder (registered as `wsjt.decoder`): slots in; decodes, JS8 frames and slot
/// results out. Outputs: `decodes` ([`Decode`]), `frames` ([`Js8Frame`]), `results` ([`SlotResult`]). The slots and decodes it
/// counts leave out early slots.
pub struct SlotDecoder {
    mode: Mode,
    queue: Option<Arc<SlotQueue>>,
    state: DecodeState,
    slots: u64,
    decodes: u64,
    last_ms: f64,
    /// Whether the status was written.
    shown: bool,
}

impl SlotDecoder {
    /// The decoder of `mode`.
    pub fn new(mode: Mode) -> Self {
        SlotDecoder {
            mode,
            queue: None,
            state: DecodeState::default(),
            slots: 0,
            decodes: 0,
            last_ms: 0.0,
            shown: false,
        }
    }

    /// The same decoder, telling `queue` which slots it decoded.
    pub fn with_queue(mut self, queue: Arc<SlotQueue>) -> Self {
        self.queue = Some(queue);
        self
    }

    /// The decoder's state.
    pub fn state(&self) -> &DecodeState {
        &self.state
    }
}

impl Decoder for SlotDecoder {
    type Input = Timed<Slot>;
    fn name(&self) -> &'static str {
        "wsjt.decoder"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<Decode>("decodes").rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<Js8Frame>("frames").rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<SlotResult>("results").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        SlotDecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match SlotDecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SlotDecoderSetting::Mirrored => self.state.opt.mirrored = MIRRORED.read(id, value)?,
            SlotDecoderSetting::Deep => self.state.opt.deep = DEEP.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (
                SlotDecoderSetting::Mirrored,
                Value::Bool(self.state.opt.mirrored),
            ),
            (SlotDecoderSetting::Deep, Value::Bool(self.state.opt.deep)),
        ])
    }
    fn results(&self) -> Published {
        Published::new(SlotDecoderFields::SHAPE.fields)
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<Decode>(), 0.0),
            Format::new(Kind::typed::<Js8Frame>(), 0.0),
            Format::new(Kind::typed::<SlotResult>(), 0.0),
        ])
    }
    fn process(&mut self, x: &[Timed<Slot>], _first: u64, out: &mut Outbox) {
        for s in x {
            let r = decode_slot(&s.value, &mut self.state);
            if let Some(q) = &self.queue {
                q.done(s.value.seq);
            }
            if !r.early {
                self.slots += 1;
                self.decodes += (r.decodes.len() + r.frames.len()) as u64;
                self.last_ms = r.ms;
            }
            for d in &r.decodes {
                out.items::<Decode>(0).push(Timed {
                    time: s.time,
                    value: d.clone(),
                });
            }
            for f in &r.frames {
                out.items::<Js8Frame>(1).push(Timed {
                    time: s.time,
                    value: f.clone(),
                });
            }
            out.items::<SlotResult>(2).push(Timed {
                time: s.time,
                value: r,
            });
        }
    }
    fn report(&mut self, results: &mut Published) {
        results.put(&SlotDecoderFields {
            slots: self.slots as i64,
            decodes: self.decodes as i64,
            decode_ms: self.last_ms,
        });
        if !self.shown {
            self.shown = true;
            results
                .results_mut()
                .set_status(&Status::new(LockState::Searching));
        }
    }
    fn report_interval_s(&self) -> f64 {
        1.0
    }
    fn clear(&mut self) {
        self.state.clear();
        self.slots = 0;
        self.decodes = 0;
    }
    fn helper(&self) -> Helper {
        Helper::WhenLive {
            batch: 1,
            backlog: if self.mode == Mode::Js8 { 8 } else { 6 },
        }
    }
}

/// A runner of slots for one dial chain: in line for a recording, on the graph's helper threads for a live radio, one slot at a time
/// with at most `cap` waiting (the oldest waiting slot is dropped).
pub struct PoolRunner {
    state: Option<DecodeState>,
    job: Option<Ticket<(DecodeState, SlotResult)>>,
    waiting: VecDeque<Slot>,
    cap: usize,
    live: bool,
    pool: Pool,
    clear: bool,
    /// Slots dropped because decoding was too slow.
    pub dropped: u64,
}

impl PoolRunner {
    /// A runner with `state`, on `ctx`'s helper threads when its samples are live.
    pub fn new(state: DecodeState, ctx: &RunContext, cap: usize) -> PoolRunner {
        PoolRunner {
            state: Some(state),
            job: None,
            waiting: VecDeque::new(),
            cap: cap.max(1),
            live: ctx.live,
            pool: ctx.pool.clone(),
            clear: false,
            dropped: 0,
        }
    }

    /// Queues a slot for decoding; when the queue is full, the oldest slot is dropped.
    pub fn submit(&mut self, slot: Slot) {
        self.waiting.push_back(slot);
        while self.waiting.len() > self.cap {
            self.waiting.pop_front();
            self.dropped += 1;
        }
    }

    /// Forgets the hash tables (at once, or when the slot being decoded is done).
    pub fn clear(&mut self) {
        match self.state.as_mut() {
            Some(s) => s.clear(),
            None => self.clear = true,
        }
    }

    /// Decodes what it can now and appends the results to `out` (never waits for a helper thread).
    pub fn poll(&mut self, out: &mut Vec<SlotResult>) {
        if let Some(t) = self.job.as_mut() {
            if let Some(r) = t.try_take() {
                self.job = None;
                if let Ok((mut st, res)) = r {
                    if std::mem::take(&mut self.clear) {
                        st.clear();
                    }
                    self.state = Some(st);
                    out.push(res);
                } else {
                    // The decoder failed on the helper: start again with empty tables.
                    self.state = Some(DecodeState::default());
                }
            }
        }
        while self.state.is_some() {
            let Some(slot) = self.waiting.pop_front() else {
                return;
            };
            if !self.live {
                if let Some(st) = self.state.as_mut() {
                    out.push(decode_slot(&slot, st));
                }
                continue;
            }
            let mut st = self.state.take().unwrap_or_default();
            self.job = Some(self.pool.spawn(move || {
                let r = decode_slot(&slot, &mut st);
                (st, r)
            }));
        }
    }
}
