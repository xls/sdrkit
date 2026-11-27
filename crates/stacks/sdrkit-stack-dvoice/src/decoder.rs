//! The decoder that every digital voice stack runs: the demodulation chain, the symbol layer, the stack's protocol, the
//! shared measurements, the verdict, the lists, the encryption rule of the media output, and the results.

use crate::calls::Tracker;
use crate::front::{Chain, ChainSpec};
use crate::items::*;
use crate::results::{self, SeriesIds, Verdict};
use crate::typed::{CallRow, TalkgroupArgs, REMOVE};
use sdrkit_core::interface::Table;
use crate::symbols::{Sym, SymAnswer, SymbolLayer, SymbolSink, SyncAnswer, SyncPattern};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::media::Codec;
use sdrkit_core::publish::{field, Declare, Published, TableDef};
use sdrkit_core::Record;
use sdrkit_core::{
    Complex32, ConfigError, DeclareSetting, LockState, Schema, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::media::SpeechPacket;
use sdrkit_graph::{values_of, Decoder, Format, Kind, Outbox, PacketPool, Port, RateFrom, Timed};
use sdrkit_util_objects::calls::{CallEnd, CallEvent};
use std::collections::VecDeque;
use std::sync::Arc;

/// Speech goes to the media output. Off: calls are decoded and listed, and no speech leaves.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", true)
    .help("Off: decode and list, never play speech.")
    .section("Voice");

/// A call ends after this long without a frame.
pub const HANG_S: Setting<field::F64> =
    Setting::new("hang_s", "Hang time", "s", crate::calls::HANG_S)
        .range(0.5, 10.0, 0.5)
        .section("Voice");

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// Speech goes to the media output.
        Listen = LISTEN,
        /// The hang time of a call.
        HangS = HANG_S,
    }
}

/// What a stack's decoder is made with.
#[derive(Clone, Debug)]
pub struct ModeParams {
    /// The stack id.
    pub mode: &'static str,
    /// The stack name for people.
    pub name: &'static str,
    /// The symbol rate, Hz.
    pub baud: f64,
    /// Two levels instead of four.
    pub two_level: bool,
    /// The channel width (12 500 or 6 250 Hz).
    pub channel_hz: f64,
    /// The call log table, with its cap.
    pub calls: &'static Table<CallRow<'static>>,
    /// The most messages kept.
    pub message_cap: usize,
    /// The inverted sync is another valid sync, so the polarity is turned when nothing decodes for 1 s.
    pub polarity_rule: bool,
    /// The symbol layer tracks the symbol clock between syncs (a pulse-shaped level stream: the FSK and GMSK stacks).
    pub clock_loop: bool,
    /// A second symbol rate searched at the same time (NXDN48 beside NXDN96): a second lane of the symbol layer.
    pub extra_baud: Option<f64>,
    /// The sample rate of the symbol layer's input.
    pub stream_rate: f64,
}

/// Items a protocol produced, waiting for the outputs.
#[derive(Default)]
pub struct Produced {
    /// Calls that started or ended.
    pub calls: Vec<CallItem>,
    /// Messages.
    pub messages: Vec<MessageRecord>,
    /// Positions.
    pub positions: Vec<PositionRecord>,
    /// Speech frames.
    pub speech: Vec<SpeechPacket>,
}

/// What a protocol works with: the time, the lists, the counters and the outputs.
pub struct Ctx {
    /// The sample rate of the symbol layer's input.
    pub rate: f64,
    /// Signal time of the current symbol (seconds).
    pub now: f64,
    /// Items waiting for the outputs.
    pub out: Produced,
    /// Blocks that passed their check.
    pub blocks_ok: u64,
    /// Blocks that failed their check.
    pub blocks_bad: u64,
    /// Bits in checked blocks.
    pub bits: u64,
    /// Bits repaired, or half the bits of a failed block.
    pub bits_fixed: f64,
    /// Identity fields that decoded.
    pub identity_ok: u64,
    /// Identity fields that did not decode.
    pub identity_bad: u64,
    /// Frames without an error check whose place in the transmission the stack confirmed ([`Ctx::confirmed`]).
    pub frames_confirmed: u64,
    /// Frames whose identity field was read.
    pub frames: u64,
    /// Signal time of the last block that passed its check.
    pub last_ok_s: f64,
    /// Signal time of the last frame.
    pub last_frame_s: f64,
    /// The carrier frequency, Hz.
    pub carrier_hz: f64,
    /// Speech goes to the media output.
    pub listen: bool,
    held: VecDeque<(u32, SpeechPacket)>,
    replays: Vec<(u64, u64)>,
    /// Calls that started or ended since the last report.
    pub changed: bool,
    /// The lists.
    pub tracker: Tracker,
    /// The buffers of the speech frames.
    pool: PacketPool,
    /// Which lane of the symbol layer is calling (0, or 1 for a stack's second symbol rate).
    pub lane: u8,
}

/// Most voice frames held while a call's encryption is not known.
const HELD_FRAMES: usize = 200;

impl Ctx {
    /// A context of no time, no counts and no outputs, around `tracker`.
    pub(crate) fn new(tracker: Tracker) -> Ctx {
        Ctx {
            rate: crate::front::RATE,
            now: 0.0,
            out: Produced::default(),
            blocks_ok: 0,
            blocks_bad: 0,
            bits: 0,
            bits_fixed: 0.0,
            identity_ok: 0,
            identity_bad: 0,
            frames_confirmed: 0,
            frames: 0,
            last_ok_s: -10.0,
            last_frame_s: -10.0,
            carrier_hz: 0.0,
            listen: true,
            held: VecDeque::with_capacity(HELD_FRAMES),
            replays: Vec::with_capacity(MAX_REPLAYS),
            changed: false,
            tracker,
            pool: PacketPool::default(),
            lane: 0,
        }
    }

    /// Signal seconds of sample `at` of the symbol layer's input (replayed samples taken out).
    pub fn time_of(&self, at: u64) -> f64 {
        let mut a = at;
        for &(start, n) in &self.replays {
            if at >= start + n {
                a = a.saturating_sub(n);
            } else if at >= start {
                a = a.saturating_sub(at - start);
            }
        }
        a as f64 / self.rate
    }

    /// One FEC-protected block: decoded well or not, its bits, and the bits repaired.
    pub fn block(&mut self, ok: bool, bits: usize, fixed: usize) {
        self.bits += bits as u64;
        if ok {
            self.blocks_ok += 1;
            self.bits_fixed += fixed as f64;
            self.last_ok_s = self.now;
        } else {
            self.blocks_bad += 1;
            self.bits_fixed += bits as f64 / 2.0;
        }
    }

    /// A frame that carries no error check, at the place the stack expects it (a D-STAR voice frame whose data sync arrives on
    /// schedule). It counts as decoded for the verdict.
    pub fn confirmed(&mut self) {
        self.frames_confirmed += 1;
        self.last_ok_s = self.now;
    }

    /// A frame's identity field (slot type, LICH, NID ...) decoded or not.
    pub fn identity(&mut self, ok: bool) {
        self.frames += 1;
        self.last_frame_s = self.now;
        if ok {
            self.identity_ok += 1;
        } else {
            self.identity_bad += 1;
        }
    }

    /// The lists.
    pub fn with_tracker<R>(&mut self, f: impl FnOnce(&mut Tracker) -> R) -> R {
        let r = f(&mut self.tracker);
        if self.tracker.dirty {
            self.changed = true;
        }
        r
    }

    /// Returns out the calls that started or ended on the `call` output. They leave the graph as `dvoice.call` events.
    fn flush_calls(&mut self) {
        let mut events = std::mem::take(&mut self.tracker.call_events);
        for e in events.drain(..) {
            let (id, ended) = match e {
                CallEvent::Started { id, .. } => (id, false),
                CallEvent::Ended {
                    end: CallEnd::Single,
                    ..
                } => continue,
                CallEvent::Ended { id, .. } => (id, true),
            };
            let Some(c) = self.tracker.calls().get(id).cloned() else {
                continue;
            };
            if ended {
                self.flush_held(c.channel, &c.record.encryption);
            }
            self.out.calls.push(CallItem(c));
            self.changed = true;
        }
        self.tracker.call_events = events;
    }

    /// Starts a call on `key` (`late_entry`: from an identity inside the call) and returns its id. If the call on air there has
    /// the same identity, it goes on.
    pub fn start_call(&mut self, key: u32, mut rec: CallRecord, late_entry: bool) -> u64 {
        rec.carrier_hz = self.carrier_hz;
        let now = self.now;
        let id = self.with_tracker(|t| t.start(key, now, rec, late_entry));
        self.flush_calls();
        id
    }

    /// Updates the call on `key` (a frame arrived).
    pub fn update_call(&mut self, key: u32, f: impl FnOnce(&mut CallRecord)) -> bool {
        let now = self.now;
        self.with_tracker(|t| {
            t.touch(key, now);
            match t.active(key) {
                Some(c) => {
                    f(c);
                    true
                }
                None => false,
            }
        })
    }

    /// Returns true while a call is on air on `key`.
    pub fn has_call(&self, key: u32) -> bool {
        self.tracker.active_ref(key).is_some()
    }

    /// The record of the call on `key`, copied.
    pub fn call(&self, key: u32) -> Option<CallRecord> {
        self.tracker.active_ref(key).map(|c| c.record.clone())
    }

    /// The id of the call on `key`.
    pub fn call_id(&self, key: u32) -> Option<u64> {
        self.tracker.active_ref(key).map(|c| c.id)
    }

    /// Ends the call on `key`: by its terminator (`end_seen`) or because the signal went.
    pub fn end_call(&mut self, key: u32, end_seen: bool) {
        let now = self.now;
        self.with_tracker(|t| t.close(key, now, end_seen));
        self.flush_calls();
    }

    /// Moves the end of call `id`, which is off air, to now: a repeated terminator.
    pub fn extend_call(&mut self, id: u64) {
        let now = self.now;
        self.with_tracker(|t| t.extend_end(id, now));
    }

    /// Ends the calls silent for longer than the hang time. Returns how many ended.
    pub fn expire_calls(&mut self) -> usize {
        let now = self.now;
        let n = self.with_tracker(|t| t.expire(now));
        self.flush_calls();
        n
    }

    /// Ends every call on air (a retune).
    pub fn close_calls(&mut self) {
        self.with_tracker(|t| t.close_all());
        self.flush_calls();
    }

    /// A transmission that ends as it starts (message, data, control, grant).
    pub fn log_single(&mut self, mut rec: CallRecord) {
        rec.carrier_hz = self.carrier_hz;
        let now = self.now;
        self.with_tracker(|t| t.single(now, rec));
        self.flush_calls();
        self.changed = true;
    }

    /// A text message.
    pub fn message(&mut self, m: MessageRecord) {
        self.with_tracker(|t| t.message(m.clone()));
        self.out.messages.push(m);
    }

    /// A position.
    pub fn position(&mut self, p: PositionRecord) {
        self.with_tracker(|t| t.position(p.clone()));
        self.out.positions.push(p);
    }

    /// The site of the system.
    pub fn site(&mut self, s: SiteRecord) {
        self.with_tracker(|t| t.site(s));
    }

    /// A channel grant.
    pub fn grant(&mut self, g: GrantRecord) {
        self.with_tracker(|t| t.grant(g));
    }

    /// A control message: its text goes to the event log.
    pub fn control(&mut self, c: ControlRecord) {
        self.with_tracker(|t| t.control(&c));
    }

    /// A line of the event log.
    pub fn event(&mut self, line: String) {
        let now = self.now;
        self.with_tracker(|t| t.event(now, line));
    }

    /// A voice frame of the call on `key`. It reaches the media output only in the clear. While the encryption of the call is
    /// not known, it is held up to 1 s.
    pub fn voice(&mut self, key: u32, mut frame: SpeechPacket) {
        let Some((id, state)) = self
            .tracker
            .active_ref(key)
            .map(|c| (c.id, c.record.encryption.state))
        else {
            return;
        };
        frame.call_id = id;
        match state {
            EncState::Clear | EncState::Decrypted => self.emit(key, frame),
            EncState::Encrypted => {}
            EncState::Unknown => {
                if self.held.len() == HELD_FRAMES {
                    self.held.pop_front();
                }
                self.held.push_back((key, frame));
            }
        }
    }

    /// A speech frame of `spec` with the payload `data` at signal time `time`, for the call on `key` (see [`Ctx::voice`]). A
    /// `damaged` frame failed its own check. `bit_errors` are the errors the stack found, when known.
    pub fn speech(
        &mut self,
        key: u32,
        spec: crate::voice::FrameSpec,
        data: &[u8],
        time: f64,
        damaged: bool,
        bit_errors: Option<u16>,
    ) {
        if self.tracker.active_ref(key).is_none() {
            return;
        }
        let f = crate::voice::frame(&mut self.pool, spec, data, time, damaged, bit_errors);
        self.voice(key, f);
    }

    fn emit(&mut self, key: u32, frame: SpeechPacket) {
        if !self.listen {
            return;
        }
        if !frame.erasure {
            if let Some(c) = self.tracker.active(key) {
                c.media_frames += 1;
            }
        }
        self.out.speech.push(frame);
    }

    /// The encryption of the call on `key` became known: held frames go out or are dropped.
    pub fn flush_held(&mut self, key: u32, enc: &Encryption) {
        let clear = matches!(enc.state, EncState::Clear | EncState::Decrypted);
        let mut i = 0;
        while i < self.held.len() {
            if self.held[i].0 != key {
                i += 1;
                continue;
            }
            let Some((_, f)) = self.held.remove(i) else {
                break;
            };
            if clear && self.listen {
                if let Some(c) = self.tracker.active(key) {
                    c.media_frames += 1;
                }
                self.out.speech.push(f);
            }
        }
    }

    /// Sets the encryption of the call on `key`, releasing or dropping held voice.
    pub fn set_encryption(&mut self, key: u32, enc: Encryption) {
        let e2 = enc.clone();
        self.update_call(key, |c| c.encryption = e2);
        if enc.state != EncState::Unknown {
            self.flush_held(key, &enc);
        }
    }

    /// Drops held frames older than 1 s.
    fn drop_old_held(&mut self) {
        let now = self.now;
        while self
            .held
            .front()
            .is_some_and(|(_, f)| now - f.packet.time > 1.0)
        {
            self.held.pop_front();
        }
    }
}

/// The protocol of a stack on top of the symbol layer.
pub trait Protocol: Send + 'static {
    /// What the decoder of the stack is made with.
    fn params(&self) -> ModeParams;
    /// The frame sync patterns.
    fn patterns(&self) -> Vec<SyncPattern>;
    /// A frame sync `id` was found (`inverted`: its inverse), with the `pre` symbols before it, at sample `at`. `tracking`:
    /// the sync came where the frame timing expects it.
    fn on_sync(
        &mut self,
        id: u8,
        inverted: bool,
        pre: &[Sym],
        at: u64,
        tracking: bool,
        ctx: &mut Ctx,
    ) -> SyncAnswer;
    /// One symbol after a sync, at sample `at`.
    fn on_symbol(&mut self, s: Sym, at: u64, ctx: &mut Ctx) -> SymAnswer;
    /// About every 0.25 s of signal.
    fn on_tick(&mut self, _ctx: &mut Ctx) {}
    /// A retune: drop sync and anything in progress.
    fn on_reset(&mut self, _ctx: &mut Ctx) {}
    /// The stack's own result fields, written by [`Protocol::publish`].
    fn fields(&self) -> Vec<&'static dyn Declare> {
        Vec::new()
    }
    /// The stack's own result tables.
    fn tables(&self) -> Vec<&'static TableDef> {
        Vec::new()
    }
    /// Writes the stack's own fields and tables.
    fn publish(&self, _p: &mut Published) {}
    /// The speech codec of the stack.
    fn codec(&self) -> Codec;
    /// Returns true while a call is on air.
    fn in_call(&self, ctx: &Ctx) -> bool {
        ctx.tracker.active_keys().next().is_some()
    }
    /// The stack's own settings. The schema of the decoder lists them after its own.
    fn own_settings(&self) -> &'static [&'static dyn DeclareSetting] {
        &[]
    }
    /// Applies `value` to the stack's own setting at `index` of [`Protocol::own_settings`]. `id` names it in an error.
    fn apply_own(&mut self, _index: usize, id: SettingId, _value: &Value) -> Result<(), SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    /// The present value of the stack's own setting at `index` of [`Protocol::own_settings`].
    fn own_value(&self, _index: usize) -> Value {
        Value::None
    }
    /// The format of the radio's samples, when the decoder is configured and after a retune.
    fn on_format(&mut self, _input: &Format) {}
    /// The radio's samples of each piece, before the demodulation chain. A stack that decodes more channels of the capture
    /// takes them here.
    fn on_input(&mut self, _x: &[Complex32], _ctx: &mut Ctx) {}
    /// The clear command: forgets what the stack keeps beside the lists.
    fn on_clear(&mut self) {}
    /// The 8 kHz channel of each piece (0 Hz is the dial frequency), for a stack whose chain makes one
    /// ([`Chain::channel_8k`]). A stack with waveforms that do not pass the symbol layer demodulates them here.
    fn on_channel(&mut self, _x: &[Complex32], _ctx: &mut Ctx) {}
    /// The SNR (dB) and the carrier offset (Hz) of a waveform that the stack demodulates itself, while it has one. They
    /// replace the measurements of the symbol layer in the status.
    fn measured(&self) -> Option<(f32, f32)> {
        None
    }
}

struct Adapter<'a, P: Protocol> {
    p: &'a mut P,
    ctx: &'a mut Ctx,
}

impl<P: Protocol> SymbolSink for Adapter<'_, P> {
    fn sync(&mut self, id: u8, inverted: bool, pre: &[Sym], at: u64, tracking: bool) -> SyncAnswer {
        self.ctx.now = self.ctx.time_of(at);
        self.p.on_sync(id, inverted, pre, at, tracking, self.ctx)
    }
    fn symbol(&mut self, s: Sym, at: u64) -> SymAnswer {
        self.ctx.now = self.ctx.time_of(at);
        self.p.on_symbol(s, at, self.ctx)
    }
}

/// The counters of one report, for the verdict window.
#[derive(Clone, Copy, Debug, Default)]
struct Window {
    t: f64,
    syncs: u64,
    ok: u64,
    bad: u64,
    id_ok: u64,
    id_bad: u64,
    confirmed: u64,
}

/// Points of the eye picture.
const EYE: usize = 800;
/// Points of the history plots.
const HISTORY: usize = 240;
/// Points of the transform that measures the channel.
const NFFT: usize = 256;
/// The carrier to noise below which a channel without frames has no signal, dB.
const NO_POWER_CNR_DB: f32 = 3.0;
/// The most replays whose samples are taken out of the signal time.
const MAX_REPLAYS: usize = 64;

/// The decoder of a stack: the radio's samples in, typed items and results out.
pub struct VoiceDecoder<P: Protocol> {
    /// The protocol.
    pub proto: P,
    params: ModeParams,
    decl: Arc<sdrkit_core::ResultsDecl>,
    series: SeriesIds,
    spectrum: Vec<[f32; 2]>,
    summary: &'static str,
    spec: ChainSpec,
    chain: Chain,
    layers: Vec<SymbolLayer>,
    lane: usize,
    ctx: Ctx,
    // Measurements.
    fft: Fft,
    fft_buf: Vec<Complex32>,
    fft_fill: usize,
    spec_acc: Vec<f32>,
    spec_n: u32,
    pow_acc: f64,
    pow_n: u64,
    level_dbfs: f32,
    noise_dbfs: f32,
    cnr_db: Option<f32>,
    snr_db: Option<f32>,
    front_offset_hz: f32,
    snr_hist: VecDeque<f32>,
    ber_hist: VecDeque<f32>,
    reports: u64,
    last_flip_s: f64,
    win: VecDeque<Window>,
    verdict: Verdict,
    scratch: Vec<f32>,
    fine_acc: f32,
    cfo_shown: f32,
    dev_shown: f32,
    eye: Vec<f32>,
    published_lists: bool,
    in_samples: u64,
    /// The settings of the decoder, then those of the stack.
    schema: Arc<Schema>,
}

/// The results of a stack with protocol `proto`.
pub fn declaration<P: Protocol>(proto: &P) -> sdrkit_core::ResultsDecl {
    results::declaration(proto.params().calls, &proto.fields(), &proto.tables())
}

impl<P: Protocol> VoiceDecoder<P> {
    /// The decoder of `proto` behind the demodulation chain `spec`.
    pub fn new(proto: P, spec: ChainSpec) -> VoiceDecoder<P> {
        let params = proto.params();
        let decl = Arc::new(declaration(&proto));
        let series = SeriesIds::of(&decl).expect("the family declares its series");
        let pats = proto.patterns();
        let sps = (params.stream_rate / params.baud).round() as usize;
        let mut layers = vec![SymbolLayer::new(sps, params.two_level, &pats)];
        if let Some(b) = params.extra_baud {
            layers.push(SymbolLayer::new(
                (params.stream_rate / b).round() as usize,
                params.two_level,
                &pats,
            ));
        }
        for l in layers.iter_mut() {
            l.set_clock_loop(params.clock_loop);
        }
        let mut ctx = Ctx::new(Tracker::new(
            params.mode,
            params.calls.cap,
            params.message_cap,
        ));
        ctx.rate = params.stream_rate;
        let own = proto.own_settings();
        let schema = if own.is_empty() {
            DecoderSetting::schema()
        } else {
            Arc::new(Schema::of(&[DecoderSetting::SETTINGS, own].concat()))
        };
        VoiceDecoder {
            schema,
            proto,
            decl,
            series,
            spectrum: Vec::with_capacity(128),
            summary: "",
            spec,
            chain: Chain::new(spec),
            layers,
            lane: 0,
            ctx,
            fft: Fft::forward(NFFT),
            fft_buf: vec![Complex32::new(0.0, 0.0); NFFT],
            fft_fill: 0,
            spec_acc: vec![0.0; NFFT],
            spec_n: 0,
            pow_acc: 0.0,
            pow_n: 0,
            level_dbfs: -120.0,
            noise_dbfs: -120.0,
            cnr_db: None,
            snr_db: None,
            front_offset_hz: 0.0,
            snr_hist: VecDeque::with_capacity(HISTORY + 1),
            ber_hist: VecDeque::with_capacity(HISTORY + 1),
            reports: 0,
            last_flip_s: 0.0,
            win: VecDeque::with_capacity(64),
            verdict: Verdict::NoInput,
            scratch: Vec::with_capacity(NFFT),
            fine_acc: 0.0,
            cfo_shown: 0.0,
            dev_shown: 0.0,
            eye: Vec::with_capacity(EYE),
            published_lists: false,
            params,
            in_samples: 0,
        }
    }

    /// The lists the decoder keeps.
    pub fn tracker(&self) -> &Tracker {
        &self.ctx.tracker
    }

    /// The lists, to change.
    pub fn tracker_mut(&mut self) -> &mut Tracker {
        &mut self.ctx.tracker
    }

    fn measure_iq(&mut self, n0: usize) {
        for i in n0..self.chain.iq.len() {
            let s = self.chain.iq[i];
            self.pow_acc += s.norm_sqr() as f64;
            self.pow_n += 1;
            self.fft_buf[self.fft_fill] = s;
            self.fft_fill += 1;
            if self.fft_fill == NFFT {
                self.fft_fill = 0;
                self.fft.process(&mut self.fft_buf);
                for (a, b) in self.spec_acc.iter_mut().zip(&self.fft_buf) {
                    *a += b.norm_sqr() / (NFFT * NFFT) as f32;
                }
                self.spec_n += 1;
            }
        }
    }

    /// Level, noise floor and CNR from the channel's spectrum since the last report, and the spectrum picture.
    fn measure_report(&mut self, r: &mut Published) {
        if self.pow_n > 0 {
            let p = self.pow_acc / self.pow_n as f64;
            self.level_dbfs = (10.0 * p.max(1e-14).log10()) as f32;
            self.pow_acc = 0.0;
            self.pow_n = 0;
        }
        if self.spec_n == 0 {
            return;
        }
        let half = self.params.channel_hz / 2.0;
        let df = crate::front::RATE / NFFT as f64;
        let n = self.spec_n as f32;
        self.scratch.clear();
        let mut inband = 0f32;
        for i in 0..NFFT {
            let k = if i < NFFT / 2 {
                i as f64
            } else {
                i as f64 - NFFT as f64
            };
            let f = k * df;
            let p = self.spec_acc[i] / n;
            if f.abs() <= half {
                inband += p;
            } else if f.abs() >= half + 1_000.0 && f.abs() <= 21_000.0 {
                self.scratch.push(p);
            }
        }
        if self.scratch.len() > 4 {
            self.scratch
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let per_bin = self.scratch[self.scratch.len() / 2];
            let bins_in = (self.params.channel_hz / df) as f32;
            let noise_in = per_bin * bins_in;
            self.noise_dbfs = 10.0 * noise_in.max(1e-14).log10();
            let c = (inband - noise_in).max(0.0);
            self.cnr_db = (c > 0.0 && noise_in > 0.0).then(|| 10.0 * (c / noise_in).log10());
        }
        // Channel spectrum: 128 bins over +-12 kHz (+-6 kHz for 6.25 kHz stacks), dB relative to the strongest.
        let span = if self.params.channel_hz < 10_000.0 {
            6_000.0
        } else {
            12_000.0
        };
        let mut maxp = 1e-20f32;
        let pts = &mut self.spectrum;
        pts.clear();
        for j in 0..128 {
            let f = -span + (j as f64 + 0.5) * 2.0 * span / 128.0;
            let k = ((f / df).round() as i64).rem_euclid(NFFT as i64) as usize;
            let p = self.spec_acc[k] / n;
            maxp = maxp.max(p);
            pts.push([f as f32, p]);
        }
        for p in pts.iter_mut() {
            p[1] = 10.0 * (p[1] / maxp).max(1e-12).log10();
        }
        r.results_mut().update_series(self.series.spectrum, |s| {
            results::set_series(s, true, pts.iter().copied())
        });
        self.spec_acc.iter_mut().for_each(|a| *a = 0.0);
        self.spec_n = 0;
    }

    fn verdict_now(&mut self, now: f64) -> Verdict {
        let c = &self.ctx;
        self.win.push_back(Window {
            t: now,
            syncs: self.layers.iter().map(|l| l.syncs_seen).sum(),
            ok: c.blocks_ok,
            bad: c.blocks_bad,
            id_ok: c.identity_ok,
            id_bad: c.identity_bad,
            confirmed: c.frames_confirmed,
        });
        while self.win.len() > 1 && self.win.front().is_some_and(|f| now - f.t > 2.0) {
            self.win.pop_front();
        }
        let (Some(f), Some(l)) = (self.win.front(), self.win.back()) else {
            return Verdict::NoInput;
        };
        let (syncs, ok, bad, id_ok, id_bad) = (
            l.syncs - f.syncs,
            l.ok - f.ok,
            l.bad - f.bad,
            l.id_ok - f.id_ok,
            l.id_bad - f.id_bad,
        );
        let decoding = ok > 0 || l.confirmed > f.confirmed;
        let quiet = syncs == 0 && !decoding && id_ok + id_bad == 0;
        // A channel with no power above the noise beside it (a call that ended, an empty channel) has no signal.
        let no_power = self.cnr_db.is_none_or(|c| c < NO_POWER_CNR_DB);
        if self.level_dbfs < -110.0 || (quiet && no_power) {
            Verdict::NoInput
        } else if quiet {
            Verdict::NoSync
        } else if !decoding && id_bad * 2 > id_ok + id_bad {
            Verdict::Identity
        } else if !decoding {
            Verdict::Fec
        } else if bad * 3 > ok + bad {
            Verdict::Marginal
        } else {
            Verdict::Decoding
        }
    }

    fn report_into(&mut self, r: &mut Published) {
        let now = self
            .ctx
            .now
            .max(self.layers[0].samples() as f64 / self.params.stream_rate);
        self.measure_report(r);
        let snr_now = self.layers[self.lane].take_snr_db();
        self.ctx.now = now;
        if self.ctx.expire_calls() > 0 {
            self.ctx.changed = true;
        }
        self.ctx.drop_old_held();
        let n = self.win.len();
        let ok_now = n >= 1 && self.win[n - 1].ok < self.ctx.blocks_ok;
        // Measurements are taken while frames decode, and kept otherwise.
        if ok_now {
            if let Some(s) = snr_now {
                self.snr_db = Some(s);
            }
            self.cfo_shown =
                self.front_offset_hz + self.chain.lane_hz(self.layers[self.lane].dc_hz());
            self.dev_shown = self.layers[self.lane].deviation_hz();
        }
        if let Some((snr, offset)) = self.proto.measured() {
            self.snr_db = Some(snr);
            self.cfo_shown = self.front_offset_hz + offset;
        }
        self.verdict = self.verdict_now(now);
        let in_call = self.proto.in_call(&self.ctx);
        let recent = now - self.ctx.last_frame_s < 2.0;
        let ber = if self.ctx.bits > 0 {
            (self.ctx.bits_fixed / self.ctx.bits as f64) as f32
        } else {
            0.0
        };
        if in_call || recent {
            if let Some(s) = self.snr_db {
                self.snr_hist.push_back(s);
            }
            self.ber_hist.push_back(ber);
            while self.snr_hist.len() > HISTORY {
                self.snr_hist.pop_front();
            }
            while self.ber_hist.len() > HISTORY {
                self.ber_hist.pop_front();
            }
        }
        self.reports += 1;
        let layer = &self.layers[self.lane];
        let lock_state = if in_call {
            LockState::Locked
        } else if recent {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let snr_db = self.snr_db;
        let level = Some(self.level_dbfs);
        let offset = Some(f64::from(self.cfo_shown));
        let summary = self.verdict.text();
        let summary_changed = summary != self.summary;
        self.summary = summary;
        r.results_mut().update_status(|s| {
            let same = !summary_changed
                && s.lock_state == lock_state
                && s.snr_db == snr_db
                && s.level_dbfs == level
                && s.offset_hz == offset;
            if !same {
                s.lock_state = lock_state;
                s.snr_db = snr_db;
                s.level_dbfs = level;
                s.offset_hz = offset;
                if summary_changed {
                    s.summary.clear();
                    s.summary.push_str(summary);
                }
            }
            !same
        });
        let m = results::Measured {
            level_dbfs: f64::from(self.level_dbfs),
            noise_dbfs: f64::from(self.noise_dbfs),
            cnr_db: self.cnr_db.map(f64::from),
            snr_db: self.snr_db.map(f64::from),
            offset_hz: f64::from(self.cfo_shown),
            deviation_hz: f64::from(self.dev_shown),
            clock_ppm: f64::from(layer.clock_ppm()),
            ber: f64::from(ber),
            inverted: layer.inverted,
            blocks_ok: self.ctx.blocks_ok,
            blocks_bad: self.ctx.blocks_bad,
            frames: self.ctx.frames,
            syncs: self.layers.iter().map(|l| l.syncs_seen).sum(),
            reports: self.reports,
            verdict: self.verdict,
        };
        results::put_measured(r, &m);
        self.eye.clear();
        layer.eye_ordered(&mut self.eye);
        results::put_pictures(
            r,
            &self.series,
            &self.eye,
            &layer.hist,
            &self.snr_hist,
            &self.ber_hist,
        );
        self.proto.publish(r);
        if self.ctx.tracker.dirty || !self.published_lists {
            results::put_lists(r, &mut self.ctx.tracker, self.params.calls);
            self.ctx.tracker.dirty = false;
            self.published_lists = true;
        }
        self.ctx.changed = false;
    }

    fn push_out(&mut self, out: &mut Outbox) {
        fn push<T: sdrkit_graph::Item>(
            out: &mut Outbox,
            o: usize,
            v: &mut Vec<T>,
            time: impl Fn(&T) -> f64,
        ) {
            let dst = out.items::<T>(o);
            for x in v.drain(..) {
                dst.push(Timed {
                    time: time(&x),
                    value: x,
                });
            }
        }
        let p = &mut self.ctx.out;
        push(out, 0, &mut p.calls, |c| c.start_s());
        push(out, 1, &mut p.messages, |m| m.time);
        push(out, 2, &mut p.positions, |p| p.time);
        push(out, 3, &mut p.speech, |f| f.packet.time);
    }
}

sdrkit_core::commands! {
    /// The commands of a [`VoiceDecoder`].
    pub enum VoiceDecoderCommand {
        /// Forgets one talkgroup.
        Remove = REMOVE.command,
    }
}

impl<P: Protocol> Decoder for VoiceDecoder<P> {
    type Input = Complex32;

    fn name(&self) -> &'static str {
        "dvoice.decoder"
    }

    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<CallItem>("call").rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<MessageRecord>("message").rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<PositionRecord>("position").rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<SpeechPacket>("speech").rate_from(RateFrom::Fixed(0.0)),
        ]
    }

    fn settings(&self) -> Arc<Schema> {
        Arc::clone(&self.schema)
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let Some(setting) = DecoderSetting::from_id(id) else {
            let index = id.index().wrapping_sub(DecoderSetting::SETTINGS.len());
            if index < self.proto.own_settings().len() {
                return self.proto.apply_own(index, id, value);
            }
            return Err(SettingError::NoSuchId { id });
        };
        match setting {
            DecoderSetting::Listen => self.ctx.listen = LISTEN.read(id, value)?,
            DecoderSetting::HangS => {
                if !self
                    .ctx
                    .tracker
                    .calls_mut()
                    .set_hang_s(HANG_S.read(id, value)?)
                {
                    return Err(SettingError::NotPermitted { id });
                }
            }
        }
        Ok(())
    }

    fn values(&self) -> Values {
        let own = self.proto.own_settings();
        let decoder = [
            (DecoderSetting::Listen, Value::Bool(self.ctx.listen)),
            (
                DecoderSetting::HangS,
                Value::F64(self.ctx.tracker.calls().hang_s()),
            ),
        ];
        if own.is_empty() {
            return values_of(decoder);
        }
        let mut v = Values::defaults(&self.schema);
        let first = DecoderSetting::SETTINGS.len();
        let own_values = (0..own.len()).map(|i| (SettingId((first + i) as u16), self.proto.own_value(i)));
        for (id, x) in decoder.map(|(s, x)| (s.id(), x)).into_iter().chain(own_values) {
            if v.set(id, &x).is_err() {
                sdrkit_core::record::refuse_write(&self.schema.settings()[id.index()].field.key, "a present value that its schema refuses");
            }
        }
        v
    }

    fn results(&self) -> Published {
        Published::from_decl(self.decl.clone())
    }

    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.chain.configure(input.rate, input.signal_offset_hz())?;
        self.ctx.rate = self.chain.stream_rate(self.params.stream_rate);
        self.ctx.carrier_hz = input.tuned_hz;
        self.ctx.replays.clear();
        self.in_samples = 0;
        for l in self.layers.iter_mut() {
            l.reset();
        }
        self.proto.on_format(input);
        let item = |kind: Kind| {
            let mut f = Format::new(kind, 0.0);
            f.tuned_hz = input.tuned_hz;
            f.centre_hz = input.tuned_hz;
            f
        };
        Ok(vec![
            item(Kind::typed::<CallItem>()),
            item(Kind::typed::<MessageRecord>()),
            item(Kind::typed::<PositionRecord>()),
            item(Kind::typed::<SpeechPacket>()),
        ])
    }

    fn retune(&mut self, input: &Format) {
        self.chain.retune(input.signal_offset_hz());
        self.ctx.carrier_hz = input.tuned_hz;
        for l in self.layers.iter_mut() {
            l.reset();
        }
        self.ctx.close_calls();
        self.ctx.changed = true;
        self.proto.on_reset(&mut self.ctx);
        self.proto.on_format(input);
    }

    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.in_samples += x.len() as u64;
        let info = self.chain.process(x);
        if info.replay_n > 0 {
            // The replay repeats signal already fed: its samples do not count as time. The chain's 48 kHz positions are the
            // symbol layer's positions for the FSK stacks. The other chains do not replay.
            if self.ctx.replays.len() == MAX_REPLAYS {
                self.ctx.replays.remove(0);
            }
            self.ctx.replays.push((info.replay_at, info.replay_n));
        }
        let moved = info.offset_hz - self.front_offset_hz;
        if moved != 0.0 {
            for l in self.layers.iter_mut() {
                l.shift_dc(moved);
            }
        }
        self.front_offset_hz = info.offset_hz;
        self.measure_iq(0);
        // No carrier at all (below -100 dBFS over the piece): the frame layer lets go.
        let iq = &self.chain.iq;
        if !iq.is_empty() {
            let p = iq.iter().map(|s| s.norm_sqr()).sum::<f32>() / iq.len() as f32;
            if p < 1e-10 {
                for l in self.layers.iter_mut() {
                    l.stop();
                }
            }
        }
        let was_tracking = self.layers.iter().any(|l| l.tracking());
        let mut good = false;
        for (i, layer) in self.layers.iter_mut().enumerate() {
            let ok_before = self.ctx.blocks_ok;
            self.ctx.lane = i as u8;
            {
                let mut a = Adapter {
                    p: &mut self.proto,
                    ctx: &mut self.ctx,
                };
                let lane = if i == 0 {
                    &self.chain.lane1
                } else {
                    &self.chain.lane2
                };
                layer.process(lane, &mut a);
            }
            let g = self.ctx.blocks_ok > ok_before;
            layer.commit(g);
            if g {
                self.lane = i;
                good = true;
            }
        }
        let now = self.layers[0].samples() as f64 / self.ctx.rate;
        // The polarity turns when nothing decodes for 1 s, for stacks whose inverted sync is another sync.
        if self.params.polarity_rule
            && self.level_dbfs > -100.0
            && now - self.ctx.last_ok_s > 1.0
            && now - self.last_flip_s > 1.0
        {
            self.layers[self.lane].inverted = !self.layers[self.lane].inverted;
            self.last_flip_s = now;
        }
        // The front follows the frame layer: while a frame layer is locked the coarse search moves only a carrier 3 kHz or more
        // off, and the fine NCO follows the centre measured on syncs that led to a decode (a slow loop of a few hertz).
        let locked = self.layers.iter().any(|l| l.tracking());
        if locked != was_tracking {
            self.chain.front.set_locked(locked);
        }
        if good {
            let dc = self.chain.lane_hz(self.layers[self.lane].dc_hz());
            if dc.abs() > 15.0 && dc.abs() < 3_000.0 {
                self.fine_acc += 0.3 * dc;
            }
            if self.fine_acc.abs() > 10.0 {
                self.chain.front.nudge_fine(self.fine_acc as f64);
                self.fine_acc = 0.0;
            }
        }
        self.proto.on_input(x, &mut self.ctx);
        let channel = self.chain.channel_8k();
        if !channel.is_empty() {
            self.proto.on_channel(channel, &mut self.ctx);
        }
        self.push_out(out);
    }

    fn tick(&mut self, out: &mut Outbox) {
        self.proto.on_tick(&mut self.ctx);
        self.push_out(out);
    }

    fn report(&mut self, results: &mut Published) {
        self.report_into(results);
    }

    fn report_interval_s(&self) -> f64 {
        // A call that starts or ends is reported at once.
        if self.ctx.changed {
            0.0
        } else {
            0.25
        }
    }

    fn commands(&self) -> &'static [&'static Command] {
        VoiceDecoderCommand::COMMANDS
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match VoiceDecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            VoiceDecoderCommand::Remove => {
                let TalkgroupArgs { row } = REMOVE.args(args)?;
                let talkgroups = &mut self.ctx.tracker.lists.talkgroups;
                let gone = row
                    .parse::<u64>()
                    .is_ok_and(|tg| talkgroups.remove(&tg).is_some());
                if !gone {
                    return Err(self.results().no_such_row(&results::TALKGROUPS, row));
                }
                self.ctx.tracker.dirty = true;
            }
        }
        Ok(None)
    }

    fn clear(&mut self) {
        self.ctx.with_tracker(|t| t.clear());
        self.proto.on_clear();
        self.published_lists = false;
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.ctx.tracker.save(self.ctx.now).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let now = self.ctx.now;
        self.ctx.tracker.load(state, now)?;
        self.ctx.changed = true;
        self.published_lists = false;
        Ok(())
    }
}

impl<P: Protocol> VoiceDecoder<P> {
    /// The chain this decoder runs (for the tests of the chain's parts).
    pub fn chain_spec(&self) -> ChainSpec {
        self.spec
    }
}
