//! What the senders of the crate share: the section, transmit offset and time-out settings, the level and the rise of the
//! carrier, the Codec 2 tables of a speech encoder (handed over by the host or read from its storage), the refusal of a
//! command, the constant-envelope burst of an FSK modulator, and the graph side of a sender of text ([`TxCore`]).

use sdrkit_codec_codec2::{Codebooks, Encoder, Mode};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut};
use sdrkit_core::publish::field;
use sdrkit_core::storage::{Storage, StorePath};
use sdrkit_core::{CommandError, Complex32, Setting, StageError};
use sdrkit_graph::{Format, Io, RunContext};
use std::sync::Arc;

/// The section of the transmit settings.
pub const TX: &str = "Transmit";
/// The folder of the host's storage that holds the Codec 2 tables.
pub const TABLE_FOLDER: &str = "codec2";
/// The peak magnitude of a signal.
pub const PEAK: f32 = 0.9;
/// The rise and the fall of the carrier (raised cosine), seconds.
pub const RISE_S: f64 = 0.005;

/// The transmit frequency less the listening frequency.
pub const TX_SPLIT: Setting<field::Hz> = Setting::new("tx_split", "Transmit offset", "Hz", 0.0)
    .range(-1e10, 1e10, 1.0)
    .help("The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex.")
    .section(TX);
/// The time-out of push-to-talk.
pub const TX_TIMEOUT: Setting<field::Duration> = Setting::new("tx_timeout", "Time-out", "s", 180.0)
    .range(1.0, 3600.0, 1.0)
    .help("A voice stream ends after this time on air.")
    .section(TX);

/// Why the Codec 2 tables cannot make an encoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableError {
    /// The table with this file name is not in the storage folder [`TABLE_FOLDER`].
    Missing(&'static str),
    /// The table with this file name does not have its length or its SHA-256.
    Wrong(&'static str),
}

/// The Codec 2 tables of a sender: the ones the host handed over, else the ones read once from the host's storage.
#[derive(Default)]
pub struct Codec2Tables {
    books: Option<Arc<Codebooks>>,
    storage: Option<Arc<dyn Storage>>,
}

impl Codec2Tables {
    /// Uses `books` instead of the tables of the storage.
    pub fn set(&mut self, books: Arc<Codebooks>) {
        self.books = Some(books);
    }

    /// The storage to read the tables from when none were handed over.
    pub fn attach(&mut self, storage: Arc<dyn Storage>) {
        self.storage = Some(storage);
    }

    /// A speech encoder of `mode`.
    ///
    /// # Errors
    ///
    /// [`TableError`] when a table of `mode` is missing or is not the right file.
    pub fn encoder(&mut self, mode: Mode) -> Result<Encoder, TableError> {
        let first = mode.tables()[0].file_name();
        let books = match &self.books {
            Some(b) => Arc::clone(b),
            None => {
                let storage = self.storage.as_ref().ok_or(TableError::Missing(first))?;
                let b = Arc::new(load(storage.as_ref(), mode)?);
                self.books = Some(Arc::clone(&b));
                b
            }
        };
        Encoder::new(mode, &books).map_err(|e| match e {
            sdrkit_codec_codec2::Error::MissingTable(t) => TableError::Missing(t.file_name()),
            _ => TableError::Wrong(first),
        })
    }
}

/// The Codec 2 tables of `mode` from the folder [`TABLE_FOLDER`] of `storage`.
fn load(storage: &dyn Storage, mode: Mode) -> Result<Codebooks, TableError> {
    let mut books = Codebooks::new();
    for &t in mode.tables() {
        let name = t.file_name();
        let path = StorePath::new(&format!("{TABLE_FOLDER}/{name}"))
            .map_err(|_| TableError::Missing(name))?;
        let bytes = storage.read(&path).map_err(|_| TableError::Missing(name))?;
        books
            .set_file_bytes(t, &bytes)
            .map_err(|_| TableError::Wrong(name))?;
    }
    Ok(books)
}

/// The refusal `e` of a sender as a command error.
pub fn failed<E: std::error::Error + Send + Sync + 'static>(e: E) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// The raised-cosine rise of `n` samples: the gain of sample `i` is `0.5 − 0.5 cos(π (i + 0.5) / n)`.
pub fn rise(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (0.5 - 0.5 * (std::f64::consts::PI * (i as f64 + 0.5) / n as f64).cos()) as f32)
        .collect()
}

/// One constant-envelope burst of an FSK or GMSK modulator: the levels of each frame in turn with the rise at the start,
/// then the tail of the pulse with the fall; every sample at [`PEAK`] times the ramp. It knows nothing of the frames.
pub struct FskBurst {
    modulator: crate::gen::FskModulator,
    rise: Vec<f32>,
    made: usize,
    done: bool,
}

impl FskBurst {
    /// A burst of `modulator` at `rate` Hz whose carrier rises and falls in `rise_s` seconds.
    pub fn new(modulator: crate::gen::FskModulator, rate: f64, rise_s: f64) -> FskBurst {
        FskBurst {
            modulator,
            rise: rise((rise_s * rate).round() as usize),
            made: 0,
            done: false,
        }
    }

    /// Appends the samples of the symbol levels `levels` to `out`.
    pub fn levels(&mut self, levels: &[f32], out: &mut Vec<Complex32>) {
        let start = out.len();
        self.modulator.symbols(levels, out);
        for (i, z) in out[start..].iter_mut().enumerate() {
            let r = self.rise.get(self.made + i).copied().unwrap_or(1.0);
            *z *= r * PEAK;
        }
        self.made += out.len() - start;
    }

    /// Appends the samples of the binary symbols `bits` (1 is the level +1, 0 the level −1) to `out`.
    pub fn bits(&mut self, bits: &[u8], out: &mut Vec<Complex32>) {
        let lv: Vec<f32> = bits
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        self.levels(&lv, out);
    }

    /// Appends the end: the pulse runs out on the carrier, which then falls. Nothing more after it.
    pub fn end(&mut self, out: &mut Vec<Complex32>) {
        if self.done {
            return;
        }
        let start = out.len();
        self.modulator.flush(8, out);
        let tail = &mut out[start..];
        let n = self.rise.len().min(tail.len());
        let k = tail.len() - n;
        for (z, r) in tail[k..].iter_mut().zip(self.rise.iter().rev()) {
            *z *= *r;
        }
        for z in tail.iter_mut() {
            *z *= PEAK;
        }
        self.done = true;
    }

    /// Whether the end is made.
    pub fn done(&self) -> bool {
        self.done
    }
}

/// A transmission that needs no host audio: it makes its samples at its own rate part by part.
pub trait Maker: Send {
    /// Appends the samples of the next part to `out`; false once the transmission is made and nothing was appended.
    fn next(&mut self, out: &mut Vec<Complex32>) -> bool;
    /// Ends the transmission after the part on air, with the end sequence of the stack.
    fn stop(&mut self);
    /// The bursts or frames made so far.
    fn frames_sent(&self) -> u64;
}

/// What every sender of text on the graph does around its [`Maker`]: the rate and the dial of the input, the transmit
/// switch of the engine, the rate change to the session's rate with the stated start and the end mark, the keyed output,
/// and the frames of the last transmission.
pub struct TxCore {
    rate: f64,
    dial: f64,
    transmit: bool,
    burst: Option<(Box<dyn Maker>, BurstOut)>,
    end_owed: bool,
    keyed: KeyedOut,
    last_frames: u64,
    base: Vec<Complex32>,
    timing: BurstTiming,
}

impl Default for TxCore {
    fn default() -> Self {
        TxCore {
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            last_frames: 0,
            base: Vec::new(),
            timing: BurstTiming::default(),
        }
    }
}

impl TxCore {
    /// The dial of the input: its tuned frequency, else its centre.
    pub fn dial(&self) -> f64 {
        self.dial
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// The frames of the transmission on air, or of the last one.
    pub fn frames(&self) -> u64 {
        self.burst
            .as_ref()
            .map_or(self.last_frames, |b| b.0.frames_sent())
    }

    /// Whether the receive audio is muted: a transmission, its end mark or the hold after it.
    pub fn keyed(&self) -> bool {
        self.keyed.keyed(self.burst.is_some() || self.end_owed)
    }

    /// Checks that a transmission can start now.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`] with the switch off; `busy` while another transmission runs; `no_input` before the
    /// first input.
    pub fn ready<E: std::error::Error + Send + Sync + 'static>(
        &self,
        busy: E,
        no_input: E,
    ) -> Result<(), CommandError> {
        if !self.transmit {
            return Err(CommandError::TransmitOff);
        }
        if self.burst.is_some() {
            return Err(failed(busy));
        }
        if self.rate <= 0.0 {
            return Err(failed(no_input));
        }
        Ok(())
    }

    /// Starts the transmission `maker`, made at `rate` Hz.
    ///
    /// # Errors
    ///
    /// `no_input` when the rate change cannot be made.
    pub fn begin<E: std::error::Error + Send + Sync + 'static>(
        &mut self,
        maker: Box<dyn Maker>,
        rate: f64,
        no_input: E,
    ) -> Result<(), CommandError> {
        let out = BurstOut::new(rate, self.rate, PEAK).map_err(|_| failed(no_input))?;
        self.last_frames = 0;
        self.burst = Some((maker, out));
        Ok(())
    }

    /// Ends the transmission after the part on air.
    pub fn halt(&mut self) {
        if let Some(b) = &mut self.burst {
            b.0.stop();
        }
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if let Some(b) = self.burst.take() {
            self.last_frames = b.0.frames_sent();
            self.end_owed = true;
        }
    }

    /// Takes the format of input `in`: its rate and its dial. Drops a transmission on air.
    pub fn configure(&mut self, f: &Format) {
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.cut();
        self.keyed.reset();
    }

    /// Takes the transmit switch and the burst timing of the run context; the switch off drops a transmission on air.
    pub fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        if !ctx.transmit {
            self.cut();
        }
    }

    /// One pass: the samples of the transmission on output 0, the end mark, and the keyed state on output 1. True when
    /// the results should be written: a tick or the end of a transmission.
    pub fn run(&mut self, io: &mut Io) -> bool {
        let room = io.input_len(0);
        let mut ended = std::mem::take(&mut self.end_owed);
        if ended {
            end_mark(io, 0);
        }
        if let Some((maker, out)) = &mut self.burst {
            while out.wants(room) {
                self.base.clear();
                let more = maker.next(&mut self.base);
                out.push(&self.base);
                if !more {
                    out.finish(&mut self.base);
                }
            }
            if out.write(io, 0, room, &self.timing) {
                self.last_frames = maker.frames_sent();
                self.burst = None;
                ended = true;
            }
        }
        let busy = self.burst.is_some();
        self.keyed.pass(io, 1, busy, ended, room, self.rate);
        io.tick() || ended
    }

    /// The last pass: as [`TxCore::run`], then a transmission still on air is cut with its end mark. True when the
    /// results should be written.
    pub fn finish(&mut self, io: &mut Io) -> bool {
        let mut publish = self.run(io);
        self.cut();
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            publish = true;
        }
        publish
    }
}
