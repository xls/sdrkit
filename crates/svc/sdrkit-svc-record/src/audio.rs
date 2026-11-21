//! Audio recording: what is heard, whole or one file per transmission, as WAV or FLAC, with pre-roll, hang, a minimum length,
//! parts, a log of kept files and the channel IQ beside each file.

use crate::error::RecordError;
use crate::format::{clean, i16_audio, iso_utc, stamp, AudioContainer};
use crate::spool::{self, Encoding, Live, LogLine, Op, Spool, Writer};
use sdrkit_core::{Clock, Complex32, Storage, StorePath};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Bytes in one block of the writer.
const BLOCK_BYTES: usize = 64 * 1024;
/// The file slot of the audio.
const AUDIO: usize = 0;
/// The file slot of the channel IQ.
const IQ: usize = 1;
/// The first line of the log of kept files.
const LOG_HEADER: &str = "start_utc,seconds,rf_hz,stack,file";

sdrkit_core::choice_enum! {
    /// Whole, or one file per transmission.
    pub enum Split {
        /// Everything heard, in one file (cut into parts of the longest file length).
        Whole = ("whole", "Whole"),
        /// One file per transmission.
        Transmission = ("transmission", "Each transmission"),
    }
}

sdrkit_core::choice_enum! {
    /// What opens a transmission.
    pub enum Gate {
        /// The squelch of the receiver.
        Squelch = ("squelch", "Squelch"),
        /// The audio level: it opens above the open level and closes below the close level, over 10 ms.
        Level = ("level", "Level"),
        /// Nothing: the whole time is one transmission.
        Always = ("always", "Always"),
    }
}

/// How the audio is recorded.
#[derive(Clone, Debug)]
pub(crate) struct AudioSpec {
    pub folder: StorePath,
    pub prefix: String,
    pub container: AudioContainer,
    pub mono: bool,
    pub split: Split,
    pub gate: Gate,
    pub open_db: f64,
    pub close_db: f64,
    pub preroll_s: f64,
    pub hang_s: f64,
    pub min_s: f64,
    pub max_file_s: f64,
    pub max_s: f64,
    pub log: bool,
    pub label: String,
    pub title: String,
    pub comment: String,
    pub stack_id: String,
    pub rf_hz: f64,
    pub utc_offset_min: i32,
    pub channel_iq: bool,
    pub queue_bytes: u64,
}

/// A pre-roll ring of bytes with a fixed size.
#[derive(Debug)]
struct Ring {
    buf: Vec<u8>,
    start: usize,
    len: usize,
}

impl Ring {
    fn new(bytes: usize) -> Ring {
        Ring {
            buf: vec![0; bytes],
            start: 0,
            len: 0,
        }
    }
    fn push(&mut self, bytes: &[u8]) {
        let cap = self.buf.len();
        if cap == 0 {
            return;
        }
        for &b in bytes {
            let at = (self.start + self.len) % cap;
            self.buf[at] = b;
            if self.len == cap {
                self.start = (self.start + 1) % cap;
            } else {
                self.len += 1;
            }
        }
    }
    /// The held bytes, oldest first, as two slices.
    fn parts(&self) -> (&[u8], &[u8]) {
        let cap = self.buf.len();
        let end = self.start + self.len;
        if end <= cap {
            (&self.buf[self.start..end], &[])
        } else {
            (&self.buf[self.start..], &self.buf[..end - cap])
        }
    }
    fn clear(&mut self) {
        self.start = 0;
        self.len = 0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Idle,
    Open,
    Hang(u64),
}

/// The control side of an audio recording.
pub(crate) struct AudioControl {
    pub writer: Writer<Vec<u8>>,
    pub live: Arc<Live>,
}

/// The feed of an audio recording. The engine (or a graph component) passes it the audio as it is heard. [`AudioFeed::push`]
/// never waits on storage and allocates nothing while a file stays open.
pub struct AudioFeed {
    spec: AudioSpec,
    clock: Clock,
    spool: Spool<Vec<u8>>,
    live: Arc<Live>,
    session: String,
    /// The rate and the input channel count of the open file.
    shape: Option<(u32, u16)>,
    out_channels: usize,
    pre: Ring,
    pre_iq: Ring,
    iq_rate: f64,
    state: State,
    win_sum: f64,
    win_n: usize,
    level_open: bool,
    file_open: bool,
    file_frames: u64,
    active_frames: u64,
    total_frames: u64,
    file_start: f64,
    file_base: String,
    continued: bool,
    ended: bool,
}

impl std::fmt::Debug for AudioFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioFeed")
            .field("session", &self.session)
            .field("state", &self.state)
            .finish()
    }
}

/// Starts an audio recording of `session` on `storage`. Nothing is written before the first audio arrives.
pub(crate) fn start(
    spec: AudioSpec,
    session: &str,
    storage: Arc<dyn Storage>,
    clock: Clock,
) -> Result<(AudioFeed, AudioControl), RecordError> {
    let buffers = (spec.queue_bytes / BLOCK_BYTES as u64).max(2) as usize;
    let (spool, writer) = spool::start(storage, 2, buffers, || Vec::with_capacity(BLOCK_BYTES))?;
    let live = Arc::new(Live::default());
    let feed = AudioFeed {
        spec,
        clock,
        spool,
        live: live.clone(),
        session: session.into(),
        shape: None,
        out_channels: 1,
        pre: Ring::new(0),
        pre_iq: Ring::new(0),
        iq_rate: 0.0,
        state: State::Idle,
        win_sum: 0.0,
        win_n: 0,
        level_open: false,
        file_open: false,
        file_frames: 0,
        active_frames: 0,
        total_frames: 0,
        file_start: 0.0,
        file_base: String::new(),
        continued: false,
        ended: false,
    };
    Ok((feed, AudioControl { writer, live }))
}

impl AudioFeed {
    /// The session whose audio this records (its row key in the engine's sessions table).
    pub fn session(&self) -> &str {
        &self.session
    }

    /// True once the recording has ended. The engine drops an ended feed.
    pub fn is_ended(&self) -> bool {
        self.ended
    }

    /// Adds interleaved audio of `channels` channels at `rate_hz`, heard while the squelch was `squelch_open`. A change of the
    /// rate or the channel count closes the open file and starts a new one.
    pub fn push(&mut self, samples: &[f32], channels: u16, rate_hz: f64, squelch_open: bool) {
        if self.check_stop() || channels == 0 || !rate_hz.is_finite() || rate_hz < 1.0 {
            return;
        }
        let shape = (rate_hz.round() as u32, channels);
        if self.shape != Some(shape) {
            self.reshape(shape);
        }
        let ch = usize::from(channels);
        let win_len = (shape.0 as usize / 100).max(1);
        let mut rest = samples;
        while rest.len() >= ch {
            // Up to the end of the level window, so the gate moves at the same frames whatever the block sizes.
            let n = (win_len - self.win_n).min(rest.len() / ch);
            let (seg, tail) = rest.split_at(n * ch);
            rest = tail;
            for f in seg.chunks_exact(ch) {
                self.win_sum +=
                    f.iter().map(|&x| f64::from(x) * f64::from(x)).sum::<f64>() / ch as f64;
            }
            self.win_n += n;
            let open = match self.spec.gate {
                Gate::Squelch => squelch_open,
                Gate::Level => self.level_open,
                Gate::Always => true,
            };
            self.segment(seg, ch, open);
            if self.win_n >= win_len {
                let db = 10.0 * (self.win_sum / self.win_n as f64 + 1e-20).log10();
                self.level_open = if self.level_open {
                    db >= self.spec.close_db
                } else {
                    db > self.spec.open_db
                };
                self.win_sum = 0.0;
                self.win_n = 0;
            }
            if self.ended {
                return;
            }
        }
        self.live
            .gate_open
            .store(self.state != State::Idle, Ordering::Relaxed);
    }

    /// Adds channel IQ at `rate_hz`. It goes beside each audio file when the recording keeps channel IQ.
    pub fn push_iq(&mut self, iq: &[Complex32], rate_hz: f64) {
        if !self.spec.channel_iq || self.ended || !rate_hz.is_finite() || rate_hz < 1.0 {
            return;
        }
        if (rate_hz - self.iq_rate).abs() > 0.5 {
            self.iq_rate = rate_hz;
            let bytes = (self.spec.preroll_s.max(0.0) * rate_hz) as usize * 8;
            self.pre_iq = Ring::new(bytes);
        }
        if self.file_open {
            let mut rest = iq;
            while !rest.is_empty() {
                let Some(block) = self.spool.block(IQ) else {
                    return;
                };
                let n = ((block.capacity() - block.len()) / 8)
                    .min(rest.len())
                    .max(1);
                let (now, tail) = rest.split_at(n.min(rest.len()));
                crate::format::IqSample::Cf32.encode(now, block);
                rest = tail;
                if block.capacity() - block.len() < 8 {
                    self.spool.send(IQ);
                }
            }
        } else {
            for c in iq {
                let mut b = [0u8; 8];
                b[..4].copy_from_slice(&c.re.to_le_bytes());
                b[4..].copy_from_slice(&c.im.to_le_bytes());
                self.pre_iq.push(&b);
            }
        }
    }

    /// Sets the stack id and the frequency listened to, for the names, tags and log lines of the next files.
    pub fn set_tuning(&mut self, stack_id: &str, rf_hz: f64) {
        stack_id.clone_into(&mut self.spec.stack_id);
        self.spec.rf_hz = rf_hz;
    }

    /// Ends the recording when the control side requested it. Returns true if the feed has ended.
    fn check_stop(&mut self) -> bool {
        if !self.ended && self.live.stopping() {
            self.end();
        }
        self.ended
    }

    fn reshape(&mut self, shape: (u32, u16)) {
        if self.file_open {
            self.close_file(true);
        }
        self.shape = Some(shape);
        self.out_channels = if self.spec.mono || self.spec.container == AudioContainer::Flac {
            1
        } else {
            usize::from(shape.1).min(2)
        };
        let frame_bytes = 2 * self.out_channels;
        self.pre =
            Ring::new((self.spec.preroll_s.max(0.0) * f64::from(shape.0)) as usize * frame_bytes);
        self.state = State::Idle;
        if self.spec.split == Split::Whole {
            self.open_file(0);
            self.state = State::Open;
        }
    }

    fn rate(&self) -> f64 {
        self.shape.map_or(1.0, |s| f64::from(s.0))
    }

    /// One piece of interleaved audio under one gate decision.
    fn segment(&mut self, x: &[f32], ch: usize, open: bool) {
        let frames = (x.len() / ch) as u64;
        let rate = self.rate();
        if self.spec.split == Split::Whole {
            self.write(x, ch);
            self.after_write();
            return;
        }
        match self.state {
            State::Idle => {
                if open {
                    self.open_file(frames);
                    self.write(x, ch);
                    self.active_frames += frames;
                    self.state = State::Open;
                } else {
                    let mut b = [0u8; 4];
                    for f in x.chunks_exact(ch) {
                        let n = convert(f, self.out_channels, &mut b);
                        self.pre.push(&b[..n]);
                    }
                }
            }
            State::Open | State::Hang(_) => {
                self.write(x, ch);
                if open {
                    self.active_frames += frames;
                    self.state = State::Open;
                } else {
                    let left = match self.state {
                        State::Hang(l) => l,
                        _ => (self.spec.hang_s.max(0.0) * rate) as u64,
                    };
                    if left <= frames {
                        let keep =
                            self.continued || self.active_frames as f64 / rate >= self.spec.min_s;
                        self.close_file(keep);
                        self.state = State::Idle;
                    } else {
                        self.state = State::Hang(left - frames);
                    }
                }
                if self.state != State::Idle {
                    self.after_write();
                }
            }
        }
    }

    /// Cuts a long file into parts, and ends a recording that reached its length.
    fn after_write(&mut self) {
        let rate = self.rate();
        if self.spec.max_s > 0.0 && self.total_frames as f64 >= self.spec.max_s * rate {
            self.end();
            return;
        }
        if self.spec.max_file_s > 0.0
            && self.file_open
            && self.file_frames as f64 >= self.spec.max_file_s * rate
        {
            self.close_file(true);
            self.open_file(0);
            // A continuation is never too short.
            self.continued = true;
        }
    }

    /// Converts `x` straight into the writer's blocks: the only copy of the samples.
    fn write(&mut self, x: &[f32], ch: usize) {
        if !self.file_open {
            return;
        }
        let frame_bytes = 2 * self.out_channels;
        let mut frames = x.chunks_exact(ch);
        let total = frames.len() as u64;
        let mut written = 0u64;
        loop {
            let Some(block) = self.spool.block(AUDIO) else {
                self.live.add_dropped(total - written);
                break;
            };
            let room = (block.capacity() - block.len()) / frame_bytes;
            let mut b = [0u8; 4];
            let mut n = 0;
            for f in frames.by_ref().take(room) {
                let len = convert(f, self.out_channels, &mut b);
                block.extend_from_slice(&b[..len]);
                n += 1;
            }
            written += n;
            if block.capacity() - block.len() < frame_bytes {
                self.spool.send(AUDIO);
            }
            if written == total || n == 0 {
                break;
            }
        }
        self.file_frames += written;
        self.total_frames += written;
        self.live.add_frames(written);
    }

    /// Opens a file whose audio started `frames` plus the pre-roll before now, and writes the pre-roll.
    fn open_file(&mut self, frames: u64) {
        let Some((rate, _)) = self.shape else {
            return;
        };
        let frame_bytes = 2 * self.out_channels;
        let pre_frames = (self.pre.len / frame_bytes) as u64;
        let start = self.clock.now() - (frames + pre_frames) as f64 / f64::from(rate);
        let base = self.base_name(start);
        let ext = match self.spec.container {
            AudioContainer::Wav => "wav",
            AudioContainer::Flac => "flac",
        };
        let Ok(path) = self.spec.folder.join(&format!("{base}.{ext}")) else {
            self.end();
            return;
        };
        let encoding = match self.spec.container {
            AudioContainer::Wav => Encoding::Wav {
                rate,
                channels: self.out_channels as u16,
            },
            AudioContainer::Flac => Encoding::Flac {
                rate,
                tags: self.tags(start),
            },
        };
        self.spool.op(Op::Open {
            slot: AUDIO,
            path,
            encoding,
            meta: None,
        });
        if self.spec.channel_iq && self.iq_rate > 0.0 {
            let name = format!(
                "{base}_{}Msps.cf32",
                crate::format::rate_for_name(self.iq_rate)
            );
            if let Ok(path) = self.spec.folder.join(&name) {
                self.spool.op(Op::Open {
                    slot: IQ,
                    path,
                    encoding: Encoding::Plain,
                    meta: None,
                });
            }
        }
        self.file_open = true;
        self.file_base = base;
        self.file_start = start;
        self.file_frames = 0;
        self.active_frames = 0;
        self.continued = false;
        let mut pre = std::mem::replace(&mut self.pre, Ring::new(0));
        let (a, b) = pre.parts();
        self.write_bytes(AUDIO, a);
        self.write_bytes(AUDIO, b);
        pre.clear();
        self.pre = pre;
        self.file_frames += pre_frames;
        self.total_frames += pre_frames;
        self.live.add_frames(pre_frames);
        let mut pre = std::mem::replace(&mut self.pre_iq, Ring::new(0));
        let (a, b) = pre.parts();
        self.write_bytes(IQ, a);
        self.write_bytes(IQ, b);
        pre.clear();
        self.pre_iq = pre;
    }

    /// Writes already converted bytes (the pre-roll).
    fn write_bytes(&mut self, slot: usize, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let Some(block) = self.spool.block(slot) else {
                return;
            };
            let n = (block.capacity() - block.len()).min(bytes.len());
            block.extend_from_slice(&bytes[..n]);
            bytes = &bytes[n..];
            if block.len() == block.capacity() {
                self.spool.send(slot);
            }
        }
    }

    fn close_file(&mut self, keep: bool) {
        if !self.file_open {
            return;
        }
        self.spool.send_all();
        let seconds = self.file_frames as f64 / self.rate();
        let log = (keep && self.spec.log)
            .then(|| {
                self.spec
                    .folder
                    .join(&format!("{}.csv", self.spec.prefix))
                    .ok()
            })
            .flatten()
            .map(|path| LogLine {
                path,
                header: LOG_HEADER,
                line: format!(
                    "{},{seconds:.2},{:.0},{}",
                    iso_utc(self.file_start),
                    self.spec.rf_hz,
                    self.spec.stack_id
                ),
            });
        self.spool.op(Op::Close {
            slot: AUDIO,
            keep,
            log,
        });
        self.spool.op(Op::Close {
            slot: IQ,
            keep,
            log: None,
        });
        self.file_open = false;
    }

    /// Closes the open file and ends the writer.
    fn end(&mut self) {
        if self.ended {
            return;
        }
        let keep = self.spec.split == Split::Whole
            || self.continued
            || self.active_frames as f64 / self.rate() >= self.spec.min_s;
        self.close_file(keep);
        self.spool.op(Op::Finish);
        self.ended = true;
        self.live.gate_open.store(false, Ordering::Relaxed);
        self.live.ended.store(true, Ordering::Relaxed);
    }

    fn base_name(&self, start: f64) -> String {
        let mut n = self.spec.prefix.clone();
        let stack = clean(&self.spec.stack_id);
        if !stack.is_empty() {
            n.push('_');
            n.push_str(&stack);
        }
        if self.spec.rf_hz > 0.0 {
            n.push_str(&format!("_{:.4}MHz", self.spec.rf_hz / 1e6));
        }
        n.push('_');
        n.push_str(&stamp(start, self.spec.utc_offset_min));
        if !self.spec.label.is_empty() {
            n.push('_');
            n.push_str(&clean(&self.spec.label));
        }
        n
    }

    fn tags(&self, start: f64) -> Vec<String> {
        let mut tags = vec![
            format!("DATE={}", iso_utc(start)),
            "ENCODER=sdrkit".to_string(),
        ];
        if !self.spec.title.is_empty() {
            tags.push(format!("TITLE={}", self.spec.title));
        }
        if !self.spec.comment.is_empty() {
            tags.push(format!("COMMENT={}", self.spec.comment));
        }
        if !self.spec.stack_id.is_empty() {
            tags.push(format!("STACK={}", self.spec.stack_id));
        }
        if self.spec.rf_hz > 0.0 {
            tags.push(format!("FREQUENCY={:.0}", self.spec.rf_hz));
        }
        tags
    }
}

impl Drop for AudioFeed {
    fn drop(&mut self) {
        self.end();
    }
}

/// Converts one frame `f` to `out` (1 or 2) channels of 16-bit little-endian samples in `b`. For one channel, the channels
/// of `f` are mixed. For two, the first two are kept. Returns the bytes written.
fn convert(f: &[f32], out: usize, b: &mut [u8; 4]) -> usize {
    if out == 1 {
        let v = f.iter().sum::<f32>() / f.len() as f32;
        b[..2].copy_from_slice(&i16_audio(v).to_le_bytes());
        2
    } else {
        b[..2].copy_from_slice(&i16_audio(f[0]).to_le_bytes());
        b[2..].copy_from_slice(&i16_audio(f[1]).to_le_bytes());
        4
    }
}
