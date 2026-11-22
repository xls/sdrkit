//! The sender of the FreeDV stack (`freedv.transmit`): speech from the microphone input, coded with Codec 2 1300 on the
//! 1600 waveform ([`crate::freedv`]) or with Codec 2 700C on the OFDM waveform 700D or 700E ([`crate::freedv_ofdm`]), with
//! the setting `callsign` in the text channel. Standard: FreeDV project documentation.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `mic` is mono audio of any rate. Output `out` is
//! complex baseband at the session's rate for `tx.sink`: the upper sideband, with 0 Hz the suppressed carrier at the
//! transmit frequency (the dial plus the transmit offset) and the carriers of the modem 900 Hz to 2100 Hz (1600) or
//! 1000 Hz to 2000 Hz (700D, 700E) above it. Output `keyed` mutes the receive audio of the stack from key down to a short
//! hold after the end mark.
//!
//! The waveform is the one of the submode: 1600 in the submode 1600, 700E in the submode 700E, else 700D; RADE does not
//! send. The text bits are the setting `callsign` and a carriage return, again and again, in the PSK31 varicode.
//!
//! - 1600: the first symbol, then frames of 40 ms, each one Codec 2 1300 frame with the Golay parity of its first 12
//!   bits and one text bit. The first frames carry coded silence while the microphone audio arrives. Push-to-talk off,
//!   `halt` and the time-out end it after the frame on air; the six slots in which the shaped pulses of the last symbols
//!   end follow, with the fall over their end.
//! - 700D and 700E: modem frames, each with one LDPC code word of Codec 2 frames, the unique word and the text bits; the
//!   first carries coded silence. Push-to-talk off, `halt` and the time-out end it after the frame on air; one pilot row
//!   follows, so the receiver has the pilots after the last frame.
//!
//! The settings `tx_clip` and `tx_filter` turn on the clipper and the transmit filter of the waveform ([`TxShape`]); the
//! clipper always has the filter after it, so its products stay in the band. The carrier rises and falls with a 5 ms raised
//! cosine; the nominal peak of the chain ([`TxChain::peak`]) is 0.9 of full scale, and no sample is above 0.9. Every
//! transmission ends with an end mark; on a live radio whose host times bursts, the first sample states its time on air.
//!
//! The Codec 2 tables come from [`FreedvTransmit::set_codebooks`] or from the host's storage, folder
//! [`crate::sender::TABLE_FOLDER`], one file for each table of the codec rate under its file name.

use crate::freedv::{frame_bits, text_bits, FdmdvModulator, Submode};
use crate::freedv_ofdm::{Modulator, TxChain, TxShape, Waveform, RATE, W700D, W700E};
use crate::sender::{
    failed, Codec2Tables, TableError, MIC_KEEP_S, PEAK, RISE_S, TABLE_FOLDER, TX, TX_SPLIT,
    TX_TIMEOUT,
};
use crate::typed::{FreedvTxFields, FreedvTxKind, PttArgs, FREEDV_PTT, HALT};
use sdrkit_codec_codec2::{Codebooks, Encoder, Mode};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, MicInput};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published, Record,
    Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use std::sync::Arc;

/// The name of the component.
pub const ID: &str = "freedv.transmit";
/// The speech samples of one Codec 2 frame, 700C and 1300 (40 ms at 8 kHz).
const CODEC_SAMPLES: usize = 320;
/// The bits of one Codec 2 700C frame.
const CODEC_BITS: usize = 28;
/// The bits of one Codec 2 1300 frame.
const CODEC_BITS_1300: usize = 52;
/// The 1600 frames of coded silence at the start of a transmission (80 ms), so the microphone audio of each later frame
/// has arrived before the frame is made.
const LEAD_1600: u64 = 2;
/// The transmit chain of 1600: the scale puts the peak of the modulator output near half of full scale; the clipper drives
/// it by 1.5; the filter passes 725 Hz to 2275 Hz at −6 dB, the carriers and their first side lobes.
pub const TX_1600: TxShape = TxShape {
    scale: 0.2,
    clip_gain: 1.5,
    cut_hz: 775.0,
};
/// The most characters of the callsign text.
pub const TEXT_MAX: usize = 40;

/// The callsign text of the text channel.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Callsign text", "", "")
    .help("Sent in the text channel with a carriage return after it, again and again: up to 40 printable ASCII characters, normally the call sign of the station. Nothing is sent while it is empty.")
    .section(TX);
/// The clipper.
pub const TX_CLIP: Setting<field::Bool> = Setting::new("tx_clip", "Clipper", "", false)
    .help("The drive gain and a limit at half of full scale before the level, with the transmit filter after it: more average power for a little distortion of the carriers. It works on 1600, 700D and 700E.")
    .section(TX);
/// The transmit filter.
pub const TX_FILTER: Setting<field::Bool> = Setting::new("tx_filter", "Transmit filter", "", false)
    .help("A band-pass around 1500 Hz: about 725 Hz to 2275 Hz for 1600, 750 Hz to 2250 Hz for 700D, 500 Hz to 2500 Hz for 700E. The clipper turns it on as well.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The callsign text.
        Callsign = CALLSIGN,
        /// The clipper.
        Clip = TX_CLIP,
        /// The transmit filter.
        Filter = TX_FILTER,
        /// The transmit offset.
        Split = TX_SPLIT,
        /// The time-out.
        Timeout = TX_TIMEOUT,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Push-to-talk.
        Ptt = FREEDV_PTT.command,
        /// Ends the transmission.
        Halt = HALT,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 2] = [FREEDV_PTT.command.key, HALT.key];

/// Why a transmit command was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FreedvTxError {
    /// The setting `callsign` is empty.
    NoSource,
    /// The submode has no sender: RADE.
    NoWaveform,
    /// Another transmission is under way.
    Busy,
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// The Codec 2 table with this file name is not in the storage folder [`TABLE_FOLDER`].
    NoTable(&'static str),
    /// The Codec 2 table with this file name does not have its length or its SHA-256.
    BadTable(&'static str),
}

impl FreedvTxError {
    /// The key of the setting the error is about, if any.
    pub fn setting(&self) -> Option<&'static str> {
        match self {
            FreedvTxError::NoSource => Some(CALLSIGN.key()),
            _ => None,
        }
    }
}

impl std::fmt::Display for FreedvTxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FreedvTxError::NoSource => write!(
                f,
                "the setting {} (callsign text) is empty: enter the call sign of the station",
                CALLSIGN.key()
            ),
            FreedvTxError::NoWaveform => {
                f.write_str("the submode RADE does not send: choose 1600, 700D, 700E or the automatic submode")
            }
            FreedvTxError::Busy => f.write_str("another transmission is under way"),
            FreedvTxError::NoInput => f.write_str("the stack has no input yet"),
            FreedvTxError::NoTable(t) => write!(
                f,
                "the Codec 2 table {t} is not in the folder {TABLE_FOLDER} of the storage"
            ),
            FreedvTxError::BadTable(t) => write!(f, "the Codec 2 table {t} is not the right file"),
        }
    }
}

impl std::error::Error for FreedvTxError {}

impl From<TableError> for FreedvTxError {
    fn from(e: TableError) -> Self {
        match e {
            TableError::Missing(t) => FreedvTxError::NoTable(t),
            TableError::Wrong(t) => FreedvTxError::BadTable(t),
        }
    }
}

/// A waveform that the sender sends.
#[derive(Clone, Copy, Debug)]
pub enum TxWaveform {
    /// 1600.
    W1600,
    /// An OFDM waveform: 700D or 700E.
    Ofdm(&'static Waveform),
}

/// The waveform that `submode` sends, or `None` for a submode that does not send.
pub fn waveform_of(submode: Submode) -> Option<TxWaveform> {
    match submode {
        Submode::Auto | Submode::W700D => Some(TxWaveform::Ofdm(&W700D)),
        Submode::W700E => Some(TxWaveform::Ofdm(&W700E)),
        Submode::W1600 => Some(TxWaveform::W1600),
        Submode::Rade => None,
    }
}

/// Puts the samples `out` of a transmission at their level in place: the gain that takes the nominal peak of `chain` to
/// [`PEAK`], the rise over the first samples of the transmission (`made` samples came before `out`), the fall over the
/// last ones of `out` when `last`, and a limit of every magnitude to [`PEAK`].
fn level(out: &mut [Complex32], chain: &TxChain, rise: &[f32], made: usize, last: bool) {
    let gain = PEAK / chain.peak();
    let tail = out.len();
    let fall_from = if last {
        tail.saturating_sub(rise.len())
    } else {
        tail
    };
    for (i, z) in out.iter_mut().enumerate() {
        let mut g = gain * rise.get(made + i).copied().unwrap_or(1.0);
        if i >= fall_from {
            g *= rise[tail - 1 - i];
        }
        let y = *z * g;
        let m = y.norm();
        *z = if m > PEAK { y * (PEAK / m) } else { y };
    }
}

/// One transmission as 8 kHz samples: the modem frames in order, the closing pilot row, the rise, the level and the fall.
/// It knows nothing of the graph, so a test can drive it with codec frames of its own.
pub struct Sender {
    w: &'static Waveform,
    modulator: Modulator,
    chain: TxChain,
    text: Vec<u8>,
    text_at: usize,
    data: Vec<u8>,
    sent: u64,
    limit: u64,
    stopping: bool,
    done: bool,
    rise: Vec<f32>,
    made: usize,
    raw: Vec<Complex32>,
}

impl Sender {
    /// A transmission of at most `limit` modem frames on `w` with `callsign` in the text channel, with the clipper and
    /// the transmit filter on or off. The clipper always has the transmit filter after it.
    pub fn new(
        w: &'static Waveform,
        callsign: &str,
        clip: bool,
        filter: bool,
        limit: u64,
    ) -> Sender {
        Sender {
            w,
            modulator: Modulator::new(w),
            chain: TxChain::new(w.tx, clip, filter || clip),
            text: text_bits(callsign),
            text_at: 0,
            data: vec![0; w.k()],
            sent: 0,
            limit: limit.max(1),
            stopping: false,
            done: false,
            rise: crate::sender::rise((RISE_S * RATE).round() as usize),
            made: 0,
            raw: Vec::with_capacity(w.frame()),
        }
    }

    /// The waveform.
    pub fn waveform(&self) -> &'static Waveform {
        self.w
    }

    /// The Codec 2 frames of one modem frame: 4 for 700D, 2 for 700E.
    pub fn codec_frames(&self) -> usize {
        self.w.k() / CODEC_BITS
    }

    /// The modem frames made so far.
    pub fn frames_sent(&self) -> u64 {
        self.sent
    }

    /// Whether the transmission ends after the frame on air.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// Ends the transmission after the frame on air: the closing pilot row and the fall follow.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Appends the 8 kHz samples of the next modem frame to `out`, asking `speech` for each of its Codec 2 frames (4
    /// bytes, the first bit most significant) in turn. After the last frame it appends the closing pilot row, the tail
    /// of the transmit filter and the fall; then it returns false and appends nothing more.
    pub fn next(&mut self, mut speech: impl FnMut(&mut [u8; 4]), out: &mut Vec<Complex32>) -> bool {
        if self.done {
            return false;
        }
        let w = self.w;
        let start = out.len();
        self.raw.clear();
        let last = self.stopping || self.sent >= self.limit;
        if last {
            self.modulator.pilot_row(&mut self.raw);
            self.raw.resize(
                self.raw.len() + self.chain.delay(),
                Complex32::new(0.0, 0.0),
            );
        } else {
            for j in 0..self.codec_frames() {
                let mut f = [0u8; 4];
                speech(&mut f);
                for b in 0..CODEC_BITS {
                    self.data[j * CODEC_BITS + b] = (f[b / 8] >> (7 - b % 8)) & 1;
                }
            }
            let text: Vec<u8> = (0..w.text_bits)
                .map(|i| self.text[(self.text_at + i) % self.text.len()])
                .collect();
            self.text_at = (self.text_at + w.text_bits) % self.text.len();
            let symbols = self.modulator.frame_symbols(&self.data, &text);
            self.modulator.synthesize(&symbols, &mut self.raw);
            self.sent += 1;
        }
        self.chain.process(&self.raw, out);
        level(&mut out[start..], &self.chain, &self.rise, self.made, last);
        self.made += out.len() - start;
        self.done = last;
        true
    }
}

/// One 1600 transmission as 8 kHz samples: the first symbol, the frames in order, the slots in which the pulses end,
/// the rise, the level and the fall. It knows nothing of the graph, so a test can drive it with codec frames of its own.
pub struct FdmdvSender {
    modulator: FdmdvModulator,
    chain: TxChain,
    text: Vec<u8>,
    text_at: usize,
    sent: u64,
    limit: u64,
    stopping: bool,
    done: bool,
    rise: Vec<f32>,
    made: usize,
    raw: Vec<Complex32>,
}

impl FdmdvSender {
    /// A transmission of at most `limit` frames with `callsign` in the text channel, with the clipper and the transmit
    /// filter on or off. The clipper always has the transmit filter after it.
    pub fn new(callsign: &str, clip: bool, filter: bool, limit: u64) -> FdmdvSender {
        let mut text = text_bits(callsign);
        if text.is_empty() {
            text.push(0);
        }
        FdmdvSender {
            modulator: FdmdvModulator::default(),
            chain: TxChain::new(TX_1600, clip, filter || clip),
            text,
            text_at: 0,
            sent: 0,
            limit: limit.max(1),
            stopping: false,
            done: false,
            rise: crate::sender::rise((RISE_S * RATE).round() as usize),
            made: 0,
            raw: Vec::with_capacity(3 * crate::freedv::SPS),
        }
    }

    /// The frames made so far.
    pub fn frames_sent(&self) -> u64 {
        self.sent
    }

    /// Whether the transmission ends after the frame on air.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// Ends the transmission after the frame on air: the fall follows.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Appends the 8 kHz samples of the next frame to `out`, asking `speech` for its Codec 2 1300 frame (7 bytes, the first
    /// bit most significant; the low 4 bits of the last byte are not sent). The first symbol comes before the first
    /// frame. After the last frame it appends the slots in which the pulses of the last symbols end
    /// ([`FdmdvModulator::finish`]) and the tail of the transmit filter, with the fall over their end; then it returns
    /// false and appends nothing more.
    pub fn next(&mut self, mut speech: impl FnMut(&mut [u8; 7]), out: &mut Vec<Complex32>) -> bool {
        if self.done {
            return false;
        }
        let start = out.len();
        self.raw.clear();
        let last = self.stopping || self.sent >= self.limit;
        if last {
            self.modulator.finish(&mut self.raw);
            self.raw.resize(
                self.raw.len() + self.chain.delay(),
                Complex32::new(0.0, 0.0),
            );
        } else {
            if self.sent == 0 {
                self.modulator.first_symbol(&mut self.raw);
            }
            let mut f = [0u8; 7];
            speech(&mut f);
            let codec: [u8; CODEC_BITS_1300] = std::array::from_fn(|b| (f[b / 8] >> (7 - b % 8)) & 1);
            let bit = self.text[self.text_at];
            self.text_at = (self.text_at + 1) % self.text.len();
            self.modulator.frame(&frame_bits(&codec, bit), &mut self.raw);
            self.sent += 1;
        }
        self.chain.process(&self.raw, out);
        level(&mut out[start..], &self.chain, &self.rise, self.made, last);
        self.made += out.len() - start;
        self.done = last;
        true
    }
}

/// The sender of a transmission on its waveform.
enum Air {
    Fdmdv(Box<FdmdvSender>),
    Ofdm(Box<Sender>),
}

impl Air {
    fn frames_sent(&self) -> u64 {
        match self {
            Air::Fdmdv(s) => s.frames_sent(),
            Air::Ofdm(s) => s.frames_sent(),
        }
    }

    fn stopping(&self) -> bool {
        match self {
            Air::Fdmdv(s) => s.stopping(),
            Air::Ofdm(s) => s.stopping(),
        }
    }

    fn stop(&mut self) {
        match self {
            Air::Fdmdv(s) => s.stop(),
            Air::Ofdm(s) => s.stop(),
        }
    }

    /// The frames of coded silence at the start.
    fn lead(&self) -> u64 {
        match self {
            Air::Fdmdv(_) => LEAD_1600,
            Air::Ofdm(_) => 1,
        }
    }
}

/// One transmission on the graph: the sender, the speech encoder and the rate change.
struct Burst {
    sender: Air,
    encoder: Box<Encoder>,
    out: BurstOut,
}

impl Burst {
    /// Makes samples until at least `room` output samples wait, or until the transmission is made with the tail of the
    /// rate change. The first frames carry coded silence ([`Air::lead`]), so the microphone audio of each later frame has
    /// arrived before the frame is made.
    fn fill(&mut self, room: usize, mic: &mut MicInput, base: &mut Vec<Complex32>) {
        let mut speech = [0f32; CODEC_SAMPLES];
        let mut pcm = [0i16; CODEC_SAMPLES];
        while self.out.wants(room) {
            base.clear();
            let encoder = &mut self.encoder;
            let lead = self.sender.frames_sent() < self.sender.lead();
            let mut code = |frame: &mut [u8]| {
                if lead {
                    speech.fill(0.0);
                } else {
                    mic.read(&mut speech);
                }
                for (p, s) in pcm.iter_mut().zip(speech) {
                    *p = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                }
                if encoder.encode(&pcm, frame).is_err() {
                    frame.fill(0);
                }
            };
            let more = match &mut self.sender {
                Air::Fdmdv(s) => s.next(|f| code(f), base),
                Air::Ofdm(s) => s.next(|f| code(f), base),
            };
            self.out.push(base);
            if !more {
                self.out.finish(base);
            }
        }
    }
}

/// The values of the settings.
#[derive(Clone, Debug, PartialEq)]
struct TxConfig {
    callsign: String,
    clip: bool,
    filter: bool,
    split: f64,
    timeout: f64,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            callsign: String::new(),
            clip: TX_CLIP.default,
            filter: TX_FILTER.default,
            split: TX_SPLIT.default,
            timeout: TX_TIMEOUT.default,
        }
    }
}

/// The sender (`freedv.transmit`). See the module.
pub struct FreedvTransmit {
    cfg: TxConfig,
    waveform: Option<TxWaveform>,
    rate: f64,
    dial: f64,
    transmit: bool,
    mic: MicInput,
    burst: Option<Burst>,
    end_owed: bool,
    keyed: KeyedOut,
    tables: Codec2Tables,
    last_kind: Option<FreedvTxKind>,
    last_frames: u64,
    results: Published,
    base: Vec<Complex32>,
    timing: BurstTiming,
}

impl FreedvTransmit {
    /// The sender of the stack in `submode`.
    pub fn new(submode: Submode) -> FreedvTransmit {
        let mut t = FreedvTransmit {
            cfg: TxConfig::default(),
            waveform: waveform_of(submode),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            mic: MicInput::new(RATE, MIC_KEEP_S),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            tables: Codec2Tables::default(),
            last_kind: None,
            last_frames: 0,
            results: Published::new(FreedvTxFields::SHAPE.fields),
            base: Vec::new(),
            timing: BurstTiming::default(),
        };
        t.publish();
        t
    }

    /// The transmit frequency (the suppressed carrier): the dial plus the transmit offset.
    pub fn frequency(&self) -> f64 {
        self.dial + self.cfg.split
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// Uses `books` for the speech encoder instead of the tables of the storage.
    pub fn set_codebooks(&mut self, books: Arc<Codebooks>) {
        self.tables.set(books);
    }

    /// Whether the receive audio is muted: a transmission, its end mark or the hold after it.
    pub fn keyed(&self) -> bool {
        self.keyed.keyed(self.burst.is_some() || self.end_owed)
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if let Some(b) = self.burst.take() {
            self.last_frames = b.sender.frames_sent();
            self.end_owed = true;
        }
    }

    /// Push-to-talk on: starts a transmission; one already under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`FreedvTxError`] when the submode does not send, another transmission runs,
    /// the stack has no input, the setting `callsign` is empty, or the Codec 2 tables are missing.
    pub fn key_down(&mut self) -> Result<(), CommandError> {
        if !self.transmit {
            return Err(CommandError::TransmitOff);
        }
        if let Some(b) = &self.burst {
            if b.sender.stopping() {
                return Err(failed(FreedvTxError::Busy));
            }
            return Ok(());
        }
        let w = self
            .waveform
            .ok_or_else(|| failed(FreedvTxError::NoWaveform))?;
        if self.rate <= 0.0 {
            return Err(failed(FreedvTxError::NoInput));
        }
        if self.cfg.callsign.is_empty() {
            return Err(failed(FreedvTxError::NoSource));
        }
        let (mode, frame_s) = match w {
            TxWaveform::W1600 => (Mode::M1300, 0.04),
            TxWaveform::Ofdm(o) => (Mode::M700C, o.frame_s()),
        };
        let encoder = self
            .tables
            .encoder(mode)
            .map_err(|e| failed(FreedvTxError::from(e)))?;
        let out =
            BurstOut::new(RATE, self.rate, PEAK).map_err(|_| failed(FreedvTxError::NoInput))?;
        let limit = (self.cfg.timeout / frame_s).ceil() as u64;
        let (call, clip, filter) = (&self.cfg.callsign, self.cfg.clip, self.cfg.filter);
        let sender = match w {
            TxWaveform::W1600 => Air::Fdmdv(Box::new(FdmdvSender::new(call, clip, filter, limit))),
            TxWaveform::Ofdm(o) => Air::Ofdm(Box::new(Sender::new(o, call, clip, filter, limit))),
        };
        self.mic.restart();
        self.last_kind = Some(kind_of(w));
        self.last_frames = 0;
        self.burst = Some(Burst {
            sender,
            encoder: Box::new(encoder),
            out,
        });
        Ok(())
    }

    /// Push-to-talk off or `halt`: the transmission ends after the frame on air.
    pub fn key_up(&mut self) {
        if let Some(b) = &mut self.burst {
            b.sender.stop();
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let f = FreedvTxFields {
            transmitting: self.burst.is_some(),
            tx_freq_hz: self.burst.as_ref().map(|_| Hertz(self.frequency())),
            tx_kind: self.last_kind,
            tx_frames: self
                .burst
                .as_ref()
                .map_or(self.last_frames, |b| b.sender.frames_sent()) as i64,
        };
        self.results.put(&f);
    }

    /// One pass: the samples of the transmission, the end mark, and the mute state.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        self.mic.take(io, 1, self.burst.is_some());
        let mut ended = std::mem::take(&mut self.end_owed);
        if ended {
            end_mark(io, 0);
        }
        if let Some(b) = &mut self.burst {
            b.fill(room, &mut self.mic, &mut self.base);
            if b.out.write(io, 0, room, &self.timing) {
                self.last_frames = b.sender.frames_sent();
                self.burst = None;
                ended = true;
            }
        }
        let busy = self.burst.is_some();
        self.keyed.pass(io, 1, busy, ended, room, self.rate);
        if io.tick() || ended {
            self.publish();
        }
    }
}

/// What a transmission on `w` sends.
fn kind_of(w: TxWaveform) -> FreedvTxKind {
    match w {
        TxWaveform::W1600 => FreedvTxKind::Voice1600,
        TxWaveform::Ofdm(o) if std::ptr::eq(o, &W700E) => FreedvTxKind::Voice700E,
        TxWaveform::Ofdm(_) => FreedvTxKind::Voice700D,
    }
}

/// Whether `text` is a callsign text: up to [`TEXT_MAX`] printable ASCII characters.
fn valid_text(text: &str) -> bool {
    text.len() <= TEXT_MAX && text.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
}

impl Component for FreedvTransmit {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            ID,
            vec![
                Port::iq("in"),
                Port::audio("mic").optional("silence: the voice frames carry coded silence"),
            ],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.mic.configure(&inputs[1]);
        self.cut();
        self.keyed.reset();
        self.publish();
        let c = self.frequency();
        Ok(vec![Format::iq(self.rate, c, c), KeyedOut::format()])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        self.mic.attach(ctx);
        self.tables.attach(Arc::clone(&ctx.storage));
        if !ctx.transmit {
            self.cut();
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        self.cut();
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => {
                let t = CALLSIGN.read(id, v)?.trim();
                if !valid_text(t) {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.cfg.callsign = t.to_string();
            }
            TxSetting::Clip => self.cfg.clip = TX_CLIP.read(id, v)?,
            TxSetting::Filter => self.cfg.filter = TX_FILTER.read(id, v)?,
            TxSetting::Split => {
                let s = TX_SPLIT.read(id, v)?;
                let moved = s != self.cfg.split;
                self.cfg.split = s;
                return Ok(if moved {
                    Applied::Reformat
                } else {
                    Applied::Live
                });
            }
            TxSetting::Timeout => self.cfg.timeout = TX_TIMEOUT.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        sdrkit_graph::values_of([
            (TxSetting::Callsign, Value::Text(self.cfg.callsign.clone())),
            (TxSetting::Clip, Value::Bool(self.cfg.clip)),
            (TxSetting::Filter, Value::Bool(self.cfg.filter)),
            (TxSetting::Split, Value::Hz(self.cfg.split)),
            (TxSetting::Timeout, Value::Duration(self.cfg.timeout)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Ptt => {
                let PttArgs { on } = FREEDV_PTT.args(args)?;
                if on {
                    self.key_down()?;
                } else {
                    self.key_up();
                }
            }
            TxCommand::Halt => self.key_up(),
        }
        self.publish();
        Ok(None)
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
