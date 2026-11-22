//! The streams of a session that the host drains: its audio, its transmit samples, its media packets and its pictures. The
//! session thread writes them and never waits. The host reads each one from a position of its own. One stream goes the
//! other way: the audio that the host gives to the graph of the session (`host.in`).

use crate::mix::Gains;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::Stereo;
use sdrkit_core::{
    ChannelLayout, Complex32, Elementary, MediaFormat, Picture, PictureState, PictureTag,
    StreamKind,
};
use sdrkit_graph::components::{
    convert_channels, HostData, HostPort, HostStreams, HOST_IN_RATE,
};
use sdrkit_graph::{Buffer, Data, Kind, TapReader};
use std::collections::VecDeque;
use std::sync::atomic::{fence, AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// A value that a [`Ring`] holds in atomic cells.
pub(crate) trait Slot: Copy + Default {
    /// The cell of one value.
    type Atom: Default + Send + Sync;
    fn put(a: &Self::Atom, v: Self);
    fn get(a: &Self::Atom) -> Self;
}

impl Slot for f32 {
    type Atom = AtomicU32;
    fn put(a: &AtomicU32, v: f32) {
        a.store(v.to_bits(), Ordering::Relaxed);
    }
    fn get(a: &AtomicU32) -> f32 {
        f32::from_bits(a.load(Ordering::Relaxed))
    }
}

impl Slot for Complex32 {
    type Atom = AtomicU64;
    fn put(a: &AtomicU64, v: Complex32) {
        a.store(
            (u64::from(v.re.to_bits()) << 32) | u64::from(v.im.to_bits()),
            Ordering::Relaxed,
        );
    }
    fn get(a: &AtomicU64) -> Complex32 {
        let x = a.load(Ordering::Relaxed);
        Complex32::new(f32::from_bits((x >> 32) as u32), f32::from_bits(x as u32))
    }
}

/// Where a run of one audio format starts in the audio ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Run {
    start: u64,
    rate_hz: f64,
    channels: u16,
    layout: ChannelLayout,
}

/// The channel count and the layout of a run as one value: the count in the low 16 bits, the layout above.
fn pack(channels: u16, layout: ChannelLayout) -> u64 {
    u64::from(channels) | (layout as u64) << 16
}

/// The channel count and the layout of a value of [`pack`].
fn unpack(v: u64) -> (u16, ChannelLayout) {
    let layout = match v >> 16 {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        3 => ChannelLayout::Surround51,
        _ => ChannelLayout::Unknown,
    };
    (v as u16, layout)
}

impl Slot for u64 {
    type Atom = AtomicU64;
    fn put(a: &AtomicU64, v: u64) {
        a.store(v, Ordering::Relaxed);
    }
    fn get(a: &AtomicU64) -> u64 {
        a.load(Ordering::Relaxed)
    }
}

impl Slot for Run {
    type Atom = [AtomicU64; 3];
    fn put(a: &[AtomicU64; 3], v: Run) {
        a[0].store(v.start, Ordering::Relaxed);
        a[1].store(v.rate_hz.to_bits(), Ordering::Relaxed);
        a[2].store(pack(v.channels, v.layout), Ordering::Relaxed);
    }
    fn get(a: &[AtomicU64; 3]) -> Run {
        let (channels, layout) = unpack(a[2].load(Ordering::Relaxed));
        Run {
            start: a[0].load(Ordering::Relaxed),
            rate_hz: f64::from_bits(a[1].load(Ordering::Relaxed)),
            channels,
            layout,
        }
    }
}

/// A ring of fixed size that one thread writes and one thread reads. The writer never waits: when the reader is behind, the
/// oldest values are written over and counted as lost. A value has a position, which counts the values written before it.
pub(crate) struct Ring<T: Slot> {
    cells: Box<[T::Atom]>,
    /// The end of the values that the writer is writing now.
    claim: AtomicU64,
    /// The end of the values written.
    written: AtomicU64,
    /// The end of the values that the reader copied.
    read: AtomicU64,
    lost: AtomicU64,
}

impl<T: Slot> Ring<T> {
    /// A ring of `cap` values, at least one.
    pub(crate) fn new(cap: usize) -> Self {
        Ring {
            cells: (0..cap.max(1)).map(|_| T::Atom::default()).collect(),
            claim: AtomicU64::new(0),
            written: AtomicU64::new(0),
            read: AtomicU64::new(0),
            lost: AtomicU64::new(0),
        }
    }

    fn cap(&self) -> u64 {
        self.cells.len() as u64
    }

    fn cell(&self, pos: u64) -> &T::Atom {
        &self.cells[(pos % self.cap()) as usize]
    }

    /// Appends `v`. Only one thread writes.
    pub(crate) fn push(&self, v: &[T]) {
        self.push_map(v, |x| x);
    }

    /// Appends `v`, each value changed by `f`. Only one thread writes.
    pub(crate) fn push_map(&self, v: &[T], f: impl Fn(T) -> T) {
        if v.is_empty() {
            return;
        }
        let cap = self.cap();
        let w = self.written.load(Ordering::Relaxed);
        let end = w + v.len() as u64;
        let unread = self.read.load(Ordering::Relaxed).max(w.saturating_sub(cap));
        let kept = end.saturating_sub(cap);
        if kept > unread {
            self.lost.fetch_add(kept - unread, Ordering::Relaxed);
        }
        self.claim.store(end, Ordering::Relaxed);
        fence(Ordering::Release);
        let tail = &v[v.len().saturating_sub(self.cells.len())..];
        let first = end - tail.len() as u64;
        for (i, x) in tail.iter().enumerate() {
            T::put(self.cell(first + i as u64), f(*x));
        }
        self.written.store(end, Ordering::Release);
    }

    /// The end of the values written.
    pub(crate) fn written(&self) -> u64 {
        self.written.load(Ordering::Acquire)
    }

    /// Values written over before the reader copied them.
    pub(crate) fn lost(&self) -> u64 {
        self.lost.load(Ordering::Relaxed)
    }

    /// Copies the values from position `from` on into `out`, as many as fit and are written. A value that was written over
    /// is skipped. Returns the position of the first value copied and the number copied.
    pub(crate) fn read_at(&self, from: u64, out: &mut [T]) -> (u64, usize) {
        let cap = self.cap();
        let w = self.written();
        let mut start = from.max(w.saturating_sub(cap)).min(w);
        let mut n = usize::try_from(w - start)
            .unwrap_or(usize::MAX)
            .min(out.len());
        for (i, o) in out[..n].iter_mut().enumerate() {
            *o = T::get(self.cell(start + i as u64));
        }
        fence(Ordering::Acquire);
        let safe = self.claim.load(Ordering::Relaxed).saturating_sub(cap);
        if safe > start {
            let bad = usize::try_from(safe - start).unwrap_or(usize::MAX).min(n);
            out.copy_within(bad..n, 0);
            n -= bad;
            start += bad as u64;
        }
        (start, n)
    }

    /// Marks every value before `to` as read.
    pub(crate) fn mark_read(&self, to: u64) {
        self.read.store(to, Ordering::Relaxed);
    }
}

/// The host audio that a session keeps for its graph, in samples of the host: 0.68 s of mono or 0.34 s of stereo at
/// 48 kHz. The oldest go first.
const AUDIO_IN_CAP: usize = 1 << 15;
/// The host samples that the session thread converts at a time, at most.
const AUDIO_IN_CHUNK: usize = 4096;
/// The converted samples that wait for the graph, at most.
const AUDIO_IN_READY: usize = 1 << 15;

/// The converter of host audio to [`HOST_IN_RATE`], for the channel count that the graph takes.
enum Converter {
    Mono(Resampler<f32>),
    Stereo(Resampler<Stereo>),
}

impl Converter {
    /// The converter from `rate_hz` for `channels` channels (1 or 2). `None` for audio at [`HOST_IN_RATE`] and for a
    /// rate that the resampler refuses.
    fn new(rate_hz: f64, channels: u16) -> Option<Self> {
        if rate_hz == HOST_IN_RATE {
            return None;
        }
        if channels == 2 {
            Resampler::new(rate_hz, HOST_IN_RATE)
                .ok()
                .map(Converter::Stereo)
        } else {
            Resampler::new(rate_hz, HOST_IN_RATE)
                .ok()
                .map(Converter::Mono)
        }
    }

    /// Appends the converted frames of the interleaved `input` to `out`.
    fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        match self {
            Converter::Mono(r) => r.process(input, out),
            Converter::Stereo(r) => r.process_interleaved(input, out),
        }
    }

    fn reset(&mut self) {
        match self {
            Converter::Mono(r) => r.reset(),
            Converter::Stereo(r) => r.reset(),
        }
    }
}

/// The side of the session thread of the host audio: where it reads, and the converter to [`HOST_IN_RATE`].
struct AudioIn {
    at: u64,
    /// The rate of the samples and the channel count of the graph that the converter was made for.
    rate_hz: f64,
    channels: u16,
    /// `None` for samples at [`HOST_IN_RATE`].
    converter: Option<Converter>,
    raw: Vec<f32>,
    /// The samples of `raw` in the channel count of the graph, when the host gave another.
    frames: Vec<f32>,
    /// Converted samples, and how many of them the graph has taken.
    ready: Vec<f32>,
    taken: usize,
}

/// The audio runs that the reader keeps in view: a run starts at each change of the rate or the channel count.
pub(crate) const RUNS: usize = 32;
/// The media packets a session keeps, at most.
const MEDIA_CAP: usize = 1024;
/// The seconds of media packets a session keeps.
const MEDIA_KEEP_S: f64 = 10.0;
/// The bytes of media packets a session keeps, at most.
pub(crate) const MEDIA_KEEP_BYTES: usize = 32 << 20;
/// The seconds of decoded video frames a session keeps: about 13 frames at 25 per second.
pub(crate) const RAW_VIDEO_KEEP_S: f64 = 0.5;
/// The bytes of decoded video frames a session keeps, at most: 26 frames of 720x576 or 5 of 1920x1080 in I420.
pub(crate) const RAW_VIDEO_KEEP_BYTES: usize = 16 << 20;
/// The pictures a session keeps.
const PICTURE_CAP: usize = 16;
/// The burst ends that the transmit stream keeps.
const ENDS_CAP: usize = 64;

/// A part of the audio of a session, copied by [`Engine::audio`](crate::Engine::audio).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioRead {
    /// The frames copied.
    pub frames: usize,
    /// Frames per second.
    pub rate_hz: f64,
    /// Samples per frame.
    pub channels: u16,
    /// The order of the channels.
    pub layout: ChannelLayout,
}

/// Complex samples, copied by [`Engine::transmit`](crate::Engine::transmit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IqRead {
    /// The samples copied.
    pub samples: usize,
    /// Samples per second.
    pub rate_hz: f64,
    /// The frequency at 0 Hz of the samples.
    pub centre_hz: f64,
}

/// A media packet or a picture, copied by [`Engine::media`](crate::Engine::media) or
/// [`Engine::picture`](crate::Engine::picture). Its bytes are in the caller's buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaRead {
    /// Its counter. The next call requests the packet after it.
    pub counter: u64,
    /// Packets that the session dropped between the counter of the call and this one. For a picture, the states that
    /// a newer state of the same picture replaced count too.
    pub missed: u64,
    /// The kind of decoder it needs.
    pub kind: StreamKind,
    /// The stream's format: codec, packaging, configuration, timebase, and the pixel layout of a raw picture.
    pub format: Arc<MediaFormat>,
    /// Presentation time, in `format.timebase`.
    pub pts: Option<i64>,
    /// Decoding time, in `format.timebase`.
    pub dts: Option<i64>,
    /// It starts a group that decodes on its own.
    pub keyframe: bool,
    /// It is a keyframe after which nothing refers to data before it (an IDR picture, a decoded frame, an audio
    /// frame). A decoder that starts at a keyframe that is not a clean start can show damage until the next one.
    pub clean_start: bool,
    /// Seconds of signal at which it was made.
    pub time: f64,
    /// For a picture: its number in the session, whether more of it can come, and its name. Packets with the same
    /// number are states of one picture. `None` for media.
    pub picture: Option<PictureTag>,
}

/// The packets of one kind that a session keeps, by counter. The oldest go first when the queue is full: by count, by
/// bytes, or by seconds of signal. Decoded video frames have smaller bounds of their own, so that they leave before the
/// coded packets and their buffers go back to the pool of the decoder.
struct Queue {
    items: VecDeque<(u64, Elementary)>,
    next: u64,
    cap: usize,
    keep_s: f64,
    keep_bytes: usize,
    /// The bytes of the packets held, and of the decoded video frames among them.
    bytes: usize,
    raw_bytes: usize,
    /// The highest counter that the host read.
    read_to: u64,
    /// Packets that left before the host read them.
    lost: u64,
    /// The pictures of the receiver that the session follows: the number that the receiver gave, the number in the
    /// session, and whether the picture is finished.
    open: Vec<(u64, u64, bool)>,
    /// The number of the next picture of the session.
    next_picture: u64,
}

/// The pictures of its receiver that a session follows. The oldest leaves when one more starts.
const OPEN_PICTURES: usize = 64;

/// True for a packet of decoded video.
fn raw(e: &Elementary) -> bool {
    e.kind == StreamKind::RawVideo
}

impl Queue {
    fn new(cap: usize, keep_s: f64, keep_bytes: usize) -> Self {
        Queue {
            items: VecDeque::with_capacity(cap.min(64)),
            next: 1,
            cap,
            keep_s,
            keep_bytes,
            bytes: 0,
            raw_bytes: 0,
            read_to: 0,
            lost: 0,
            open: Vec::new(),
            next_picture: 1,
        }
    }

    /// Adds the picture `e` with its number in the session. A packet with a number that the receiver used before is
    /// that picture again, and the state before it leaves the queue: the queue holds the newest state of each
    /// picture. A picture without a tag is whole and has a number of its own.
    fn push_picture(&mut self, mut e: Elementary) {
        let (source, state, name) = match e.picture.take() {
            Some(t) if t.id & 0xffff_ffff == u64::from(Picture::WHOLE) => {
                (None, PictureState::Finished, t.name)
            }
            Some(t) => (Some(t.id), t.state, t.name),
            None => (None, PictureState::Finished, None),
        };
        let at = source.and_then(|s| self.open.iter().position(|(o, _, _)| *o == s));
        let id = match at {
            Some(i) => self.open[i].1,
            None => {
                self.next_picture += 1;
                self.next_picture - 1
            }
        };
        let finished = state == PictureState::Finished;
        match (at, source) {
            (Some(i), _) => self.open[i].2 = finished,
            (None, Some(s)) => {
                if self.open.len() >= OPEN_PICTURES {
                    self.open.remove(0);
                }
                self.open.push((s, id, finished));
            }
            (None, None) => {}
        }
        self.supersede(id);
        e.picture = Some(PictureTag { id, state, name });
        self.push(e);
    }

    /// Removes the state of picture `id` that the queue holds. It does not count as lost: a newer state follows.
    fn supersede(&mut self, id: u64) {
        let old = self
            .items
            .iter()
            .position(|(_, f)| f.picture.as_ref().is_some_and(|t| t.id == id));
        if let Some((_, f)) = old.and_then(|i| self.items.remove(i)) {
            self.bytes -= f.packet.len;
        }
    }

    /// Ends every picture that grows: the receiver that made them is gone. The state of each that the queue still
    /// holds goes in again as finished, with the same bytes.
    fn end_pictures(&mut self) {
        for (_, id, finished) in std::mem::take(&mut self.open) {
            if finished {
                continue;
            }
            let held = self
                .items
                .iter()
                .find(|(_, f)| f.picture.as_ref().is_some_and(|t| t.id == id))
                .map(|(_, f)| f.clone());
            if let Some(mut e) = held {
                self.supersede(id);
                if let Some(t) = e.picture.as_mut() {
                    t.state = PictureState::Finished;
                }
                self.push(e);
            }
        }
    }

    /// Removes the packet at `i` and counts it as lost if the host has not read it.
    fn remove(&mut self, i: usize) {
        if let Some((c, e)) = self.items.remove(i) {
            self.bytes -= e.packet.len;
            if raw(&e) {
                self.raw_bytes -= e.packet.len;
            }
            if c > self.read_to {
                self.lost += 1;
            }
        }
    }

    fn push(&mut self, e: Elementary) {
        let (newest, len) = (e.packet.time, e.packet.len);
        if raw(&e) {
            while let Some(i) = self.items.iter().position(|(_, f)| raw(f)) {
                let old = newest - self.items[i].1.packet.time > RAW_VIDEO_KEEP_S;
                if !old && self.raw_bytes + len <= RAW_VIDEO_KEEP_BYTES {
                    break;
                }
                self.remove(i);
            }
        }
        while self.items.len() >= self.cap
            || (!self.items.is_empty() && self.bytes + len > self.keep_bytes)
            || self
                .items
                .front()
                .is_some_and(|(_, f)| newest - f.packet.time > self.keep_s)
        {
            self.remove(0);
        }
        self.bytes += len;
        if raw(&e) {
            self.raw_bytes += len;
        }
        self.items.push_back((self.next, e));
        self.next += 1;
    }

    /// The first packet after counter `after`, its bytes copied into `out`.
    fn read(&mut self, after: u64, out: &mut Vec<u8>) -> Option<MediaRead> {
        let i = self.items.partition_point(|(c, _)| *c <= after);
        let (counter, e) = self.items.get(i)?;
        self.read_to = self.read_to.max(*counter);
        out.clear();
        out.extend_from_slice(e.packet.bytes());
        Some(MediaRead {
            counter: *counter,
            missed: counter - after.min(*counter) - 1,
            kind: e.kind,
            format: Arc::clone(&e.format),
            pts: e.packet.pts,
            dts: e.packet.dts,
            keyframe: e.packet.keyframe,
            clean_start: e.packet.clean_start,
            time: e.packet.time,
            picture: e.picture.clone(),
        })
    }
}

/// The streams of one session.
pub(crate) struct Streams {
    audio: Ring<f32>,
    runs: Ring<Run>,
    /// The rate, and the channel count with the layout ([`pack`]), of the last run, which only the writer reads.
    last_rate: AtomicU64,
    last_channels: AtomicU64,
    transmit: Ring<Complex32>,
    /// The positions in `transmit` where a burst ends.
    tx_ends: Ring<u64>,
    /// The bursts with a stated start, two values each: the position in `transmit` of the first sample of the burst,
    /// and the bits of the UTC at which that sample is on the air.
    tx_starts: Ring<u64>,
    tx_rate: AtomicU64,
    tx_centre: AtomicU64,
    /// How long before its stated start a burst must be written, nanoseconds. 0 while it is not known.
    tx_lead: AtomicU64,
    media: Mutex<Queue>,
    pictures: Mutex<Queue>,
    /// The signal of the session is made as it plays (a test signal).
    present: AtomicBool,
    /// Media packets dropped while they waited for the queues.
    pending_lost: AtomicU64,
    /// Media packets and pictures that left the queues before the host read them.
    queued_lost: AtomicU64,
    /// The volume, pan, mute and solo of the session. The level acts on every audio sample written.
    pub(crate) gains: Arc<Gains>,
    /// The audio goes to the mix of the bank, its only reader.
    pub(crate) mixed: AtomicBool,
    /// The audio that the host gave for the graph, at the rate of the host.
    audio_in: Ring<f32>,
    /// The rate and the channel count of the host audio written last.
    audio_in_rate: AtomicU64,
    audio_in_channels: AtomicU32,
    /// The host audio before this position has another rate or channel count than the audio written last.
    audio_in_from: AtomicU64,
    /// Odd while the host thread changes the rate, the channel count and the position above. It rises by 2 with
    /// each change.
    audio_in_seq: AtomicU64,
    /// The graph of the session takes host audio.
    audio_in_wanted: AtomicBool,
    /// Host samples of another rate or channel count that the session thread left out.
    audio_in_skipped: AtomicU64,
    audio_in_side: Mutex<AudioIn>,
    /// Whether a transmission sends the host audio: one of the `VOICE_*` states.
    voice: AtomicU8,
    /// The passes of the graph left until host audio that a component kept counts as no longer kept: [`KEPT_PASSES`]
    /// after each pass in which a component kept it, 0 when none did of late.
    kept: AtomicU8,
}

/// The passes of the graph that host audio counts as kept after the last pass in which a component kept it. More
/// than one, as a component on a stage thread does not run in step with the passes of the session thread.
const KEPT_PASSES: u8 = 3;

/// No transmission sends the host audio.
const VOICE_OFF: u8 = 0;
/// A command that sends speech was accepted; the next pass of the graph shows whether a transmission runs.
const VOICE_ARMED: u8 = 1;
/// A transmission that sends speech runs, until its end mark.
const VOICE_ON: u8 = 2;

impl Streams {
    /// Streams with room for `audio` audio samples and `transmit` transmit samples.
    pub(crate) fn new(audio: usize, transmit: usize) -> Self {
        Streams {
            audio: Ring::new(audio),
            runs: Ring::new(RUNS),
            last_rate: AtomicU64::new(0),
            last_channels: AtomicU64::new(0),
            transmit: Ring::new(transmit),
            tx_ends: Ring::new(ENDS_CAP),
            tx_starts: Ring::new(2 * ENDS_CAP),
            tx_rate: AtomicU64::new(0),
            tx_centre: AtomicU64::new(0),
            tx_lead: AtomicU64::new(0),
            media: Mutex::new(Queue::new(MEDIA_CAP, MEDIA_KEEP_S, MEDIA_KEEP_BYTES)),
            pictures: Mutex::new(Queue::new(PICTURE_CAP, f64::INFINITY, usize::MAX)),
            present: AtomicBool::new(false),
            pending_lost: AtomicU64::new(0),
            queued_lost: AtomicU64::new(0),
            gains: Arc::default(),
            mixed: AtomicBool::new(false),
            audio_in: Ring::new(AUDIO_IN_CAP),
            audio_in_rate: AtomicU64::new(0),
            audio_in_channels: AtomicU32::new(1),
            audio_in_from: AtomicU64::new(0),
            audio_in_seq: AtomicU64::new(0),
            audio_in_wanted: AtomicBool::new(false),
            audio_in_skipped: AtomicU64::new(0),
            audio_in_side: Mutex::new(AudioIn {
                at: 0,
                rate_hz: 0.0,
                channels: 1,
                converter: None,
                raw: vec![0.0; AUDIO_IN_CHUNK],
                frames: Vec::with_capacity(2 * AUDIO_IN_CHUNK),
                ready: Vec::with_capacity(AUDIO_IN_READY),
                taken: 0,
            }),
            voice: AtomicU8::new(VOICE_OFF),
            kept: AtomicU8::new(0),
        }
    }

    /// True once the graph of the session has asked for host audio: it has a `host.in`.
    pub(crate) fn wants_audio_in(&self) -> bool {
        self.audio_in_wanted.load(Ordering::Relaxed)
    }

    /// True while a transmission that sends the host audio runs, or a command that starts one was accepted and the
    /// graph has not run since.
    pub(crate) fn voice_wanted(&self) -> bool {
        self.voice.load(Ordering::Acquire) != VOICE_OFF
    }

    /// True while a component of the graph keeps the host audio: a sender whose transmission carries it, whatever
    /// command started it. False a few passes after the last pass that kept it, and at the end mark of the
    /// transmission.
    pub(crate) fn audio_in_kept(&self) -> bool {
        self.kept.load(Ordering::Acquire) != 0
    }

    /// The receiver accepted a command that transmits `content`. Speech arms the host audio; other content ends it.
    /// Called with the receiver held, between two passes of the graph.
    pub(crate) fn transmit_command(&self, content: sdrkit_core::TxContent) {
        let state = if content == sdrkit_core::TxContent::Voice {
            VOICE_ARMED
        } else {
            VOICE_OFF
        };
        self.voice.store(state, Ordering::Release);
    }

    /// The graph finished a pass. A command that sends speech and wrote no transmit samples in the pass after it
    /// started no transmission (a release with nothing running): the host audio is no longer wanted. Called with the
    /// receiver held.
    pub(crate) fn pass_done(&self) {
        let _ = self.voice.compare_exchange(
            VOICE_ARMED,
            VOICE_OFF,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        // A component that keeps audio at this moment stores the full count: the exchange then fails, as it should.
        let n = self.kept.load(Ordering::Acquire);
        if n > 0 {
            let _ = self
                .kept
                .compare_exchange(n, n - 1, Ordering::AcqRel, Ordering::Acquire);
        }
    }

    /// Host audio that the graph did not take in time: samples written over, and samples left out at a change of
    /// the rate or the channel count.
    pub(crate) fn audio_in_lost(&self) -> u64 {
        self.audio_in.lost() + self.audio_in_skipped.load(Ordering::Relaxed)
    }

    /// Appends audio of the host at `rate_hz` for the graph of the session: interleaved frames of `channels`
    /// channels, at least 1. A part of a frame at the end is left out. Never waits and allocates nothing. The oldest
    /// samples go when the graph does not take them in time. Audio for a session whose graph takes none is dropped.
    /// Only one thread writes at a time.
    pub(crate) fn write_audio_in(&self, samples: &[f32], channels: u16, rate_hz: f64) {
        if !self.wants_audio_in() {
            return;
        }
        let samples = &samples[..samples.len() - samples.len() % usize::from(channels)];
        if self.audio_in_rate.load(Ordering::Relaxed) != rate_hz.to_bits()
            || self.audio_in_channels.load(Ordering::Relaxed) != u32::from(channels)
        {
            self.audio_in_seq.fetch_add(1, Ordering::SeqCst);
            self.audio_in_from
                .store(self.audio_in.written(), Ordering::SeqCst);
            self.audio_in_rate
                .store(rate_hz.to_bits(), Ordering::SeqCst);
            self.audio_in_channels
                .store(u32::from(channels), Ordering::SeqCst);
            self.audio_in_seq.fetch_add(1, Ordering::SeqCst);
        }
        self.audio_in.push(samples);
    }

    /// Copies the host audio that waits into `out`, converted to whole interleaved frames of `channels` channels (1
    /// or 2) at [`HOST_IN_RATE`], oldest first, and returns the count of samples. Mono audio of the host goes to both
    /// channels of stereo, and stereo audio of the host becomes its mean for mono. At a change of the rate or the
    /// channel count of the host, the samples from before that the graph has not taken are left out. Never waits.
    /// Allocates only when the rate of the host audio or `channels` changes: the converter is made then. Only the
    /// session thread reads.
    fn read_audio_in(&self, channels: u16, out: &mut [f32]) -> usize {
        self.audio_in_wanted.store(true, Ordering::Relaxed);
        if !(1..=2).contains(&channels) {
            return 0;
        }
        let Ok(mut side) = self.audio_in_side.try_lock() else {
            return 0;
        };
        let a = &mut *side;
        let want = usize::from(channels);
        if a.channels != channels {
            // Audio converted for the other channel count is left out, and the converter is made again.
            a.channels = channels;
            a.rate_hz = 0.0;
            a.ready.clear();
            a.taken = 0;
        }
        let room = out.len() - out.len() % want;
        let mut done = 0;
        while done < room {
            if a.taken < a.ready.len() {
                let n = (a.ready.len() - a.taken).min(room - done);
                out[done..done + n].copy_from_slice(&a.ready[a.taken..a.taken + n]);
                a.taken += n;
                done += n;
                continue;
            }
            a.ready.clear();
            a.taken = 0;
            let seq = self.audio_in_seq.load(Ordering::SeqCst);
            if seq % 2 == 1 {
                break;
            }
            let from = self.audio_in_from.load(Ordering::SeqCst);
            let rate_hz = f64::from_bits(self.audio_in_rate.load(Ordering::SeqCst));
            let host = self.audio_in_channels.load(Ordering::SeqCst).max(1) as usize;
            if a.at < from {
                // Samples of the rate or the channel count before are left out.
                let oldest = self.audio_in.written().saturating_sub(AUDIO_IN_CAP as u64);
                self.audio_in_skipped
                    .fetch_add(from - a.at.max(oldest).min(from), Ordering::Relaxed);
                a.at = from;
            }
            // The converted samples of one piece fit the room reserved for them.
            let frames = ((AUDIO_IN_READY / (2 * want)) as f64 * rate_hz / HOST_IN_RATE) as usize;
            let most = frames.clamp(1, AUDIO_IN_CHUNK / host) * host;
            if rate_hz != a.rate_hz {
                a.rate_hz = rate_hz;
                a.converter = Converter::new(rate_hz, channels);
                if let Some(c) = &mut a.converter {
                    // One piece of silence makes the converter reserve its buffers now.
                    a.frames.clear();
                    a.frames.resize(most / host * want, 0.0);
                    c.process(&a.frames, &mut a.ready);
                    c.reset();
                    a.ready.clear();
                }
            }
            let (start, n) = self.audio_in.read_at(a.at, &mut a.raw[..most]);
            if self.audio_in_seq.load(Ordering::SeqCst) != seq {
                // The host began audio of another rate or channel count: the next call reads it.
                break;
            }
            // Whole frames, counted from the first sample of this rate and channel count.
            let part = ((start - from) % host as u64) as usize;
            let skip = ((host - part) % host).min(n);
            let n = (n - skip) / host * host;
            a.at = start + (skip + n) as u64;
            self.audio_in.mark_read(a.at);
            if n == 0 {
                break;
            }
            let raw = &a.raw[skip..skip + n];
            let frames = if host == want {
                raw
            } else {
                a.frames.clear();
                convert_channels(raw, host as u16, channels, &mut a.frames);
                &a.frames
            };
            match &mut a.converter {
                Some(c) => c.process(frames, &mut a.ready),
                None => a.ready.extend_from_slice(frames),
            }
        }
        done
    }

    /// Audio samples written over before the host read them.
    pub(crate) fn audio_lost(&self) -> u64 {
        self.audio.lost()
    }

    /// Media packets and pictures that left before the host read them.
    pub(crate) fn media_lost(&self) -> u64 {
        self.pending_lost.load(Ordering::Relaxed) + self.queued_lost.load(Ordering::Relaxed)
    }

    /// True if the transmit stream has room for more than one sample.
    pub(crate) fn has_transmit(&self) -> bool {
        self.transmit.cap() > 1
    }

    /// Transmit samples written over before they were read.
    pub(crate) fn transmit_lost(&self) -> u64 {
        self.transmit.lost()
    }

    /// Appends interleaved audio of `channels` channels at `rate_hz`, at the session's volume (silent when muted). Only the
    /// session thread writes.
    pub(crate) fn write_audio(&self, samples: &[f32], rate_hz: f64, channels: u16) {
        self.write_audio_layout(
            samples,
            rate_hz,
            channels,
            ChannelLayout::of_count(channels),
        );
    }

    /// [`Streams::write_audio`] for channels in the order `layout`.
    pub(crate) fn write_audio_layout(
        &self,
        samples: &[f32],
        rate_hz: f64,
        channels: u16,
        layout: ChannelLayout,
    ) {
        if samples.is_empty() || channels == 0 {
            return;
        }
        let same = self.last_rate.load(Ordering::Relaxed) == rate_hz.to_bits()
            && self.last_channels.load(Ordering::Relaxed) == pack(channels, layout);
        if !same {
            self.runs.push(&[Run {
                start: self.audio.written(),
                rate_hz,
                channels,
                layout,
            }]);
            self.last_rate.store(rate_hz.to_bits(), Ordering::Relaxed);
            self.last_channels
                .store(pack(channels, layout), Ordering::Relaxed);
        }
        let level = self.gains.level();
        self.audio.push_map(samples, |x| x * level);
    }

    /// Moves `pending` into the media and picture queues, unless the host reads them now: then they wait for the next call.
    pub(crate) fn offer_media(&self, pending: &mut Vec<Elementary>) {
        if pending.is_empty() {
            return;
        }
        let (Ok(mut media), Ok(mut pictures)) = (self.media.try_lock(), self.pictures.try_lock())
        else {
            // The packets that wait have the bounds of the media queue, so decoded frames do not pile up here.
            let mut bytes: usize = pending.iter().map(|e| e.packet.len).sum();
            let mut drop = 0;
            while drop < pending.len()
                && (pending.len() - drop > MEDIA_CAP || bytes > MEDIA_KEEP_BYTES)
            {
                bytes -= pending[drop].packet.len;
                drop += 1;
            }
            pending.drain(..drop);
            self.pending_lost.fetch_add(drop as u64, Ordering::Relaxed);
            return;
        };
        for e in pending.drain(..) {
            if e.kind == StreamKind::Picture {
                pictures.push_picture(e);
            } else {
                media.push(e);
            }
        }
        self.queued_lost
            .store(media.lost + pictures.lost, Ordering::Relaxed);
    }

    /// States whether the signal of the session is made as it plays (a test signal).
    pub(crate) fn set_present(&self, present: bool) {
        self.present.store(present, Ordering::Relaxed);
    }

    /// True if the signal of the session is made as it plays.
    pub(crate) fn present(&self) -> bool {
        self.present.load(Ordering::Relaxed)
    }

    /// Ends the pictures that grow, when the session takes another receiver or ends: each goes to the host once more
    /// as finished. A picture of the next receiver is a new picture, whatever number that receiver gives it.
    pub(crate) fn end_pictures(&self) {
        crate::feed::lock(&self.pictures).end_pictures();
    }

    /// The first media packet after counter `after`, its bytes copied into `out`. Pictures are not in it.
    pub(crate) fn read_media(&self, after: u64, out: &mut Vec<u8>) -> Option<MediaRead> {
        crate::feed::lock(&self.media).read(after, out)
    }

    /// The first picture after counter `after`, its bytes copied into `out`.
    pub(crate) fn read_picture(&self, after: u64, out: &mut Vec<u8>) -> Option<MediaRead> {
        crate::feed::lock(&self.pictures).read(after, out)
    }

    /// Copies audio from the host's position `at` into `out`, whole frames of one format, and moves `at` on. `runs` is the
    /// host's room for the runs. `None` if no audio waits.
    pub(crate) fn read_audio(
        &self,
        at: &mut u64,
        runs: &mut [Run; RUNS],
        out: &mut [f32],
    ) -> Option<AudioRead> {
        let (_, n) = self
            .runs
            .read_at(self.runs.written().saturating_sub(RUNS as u64), runs);
        let runs = &runs[..n];
        let w = self.audio.written();
        let from = (*at).max(w.saturating_sub(self.audio.cap()));
        let i = runs.iter().rposition(|r| r.start <= from).unwrap_or(0);
        let run = *runs.get(i)?;
        let ch = u64::from(run.channels.max(1));
        let mut from = from.max(run.start);
        from += (ch - (from - run.start) % ch) % ch;
        let end = runs.get(i + 1).map_or(w, |r| r.start).min(w);
        if from >= end {
            *at = from.max(*at);
            return None;
        }
        let want = usize::try_from(end - from)
            .unwrap_or(usize::MAX)
            .min(out.len());
        let want = want - want % ch as usize;
        let (start, got) = self.audio.read_at(from, &mut out[..want]);
        // A part of a frame that was written over is left out.
        let skip = ((ch - (start - run.start) % ch) % ch) as usize;
        let skip = skip.min(got);
        let got = got - skip;
        let got = got - got % ch as usize;
        out.copy_within(skip..skip + got, 0);
        *at = start + (skip + got) as u64;
        self.audio.mark_read(*at);
        (got > 0).then_some(AudioRead {
            frames: got / ch as usize,
            rate_hz: run.rate_hz,
            channels: run.channels,
            layout: run.layout,
        })
    }

    /// Copies transmit samples from the host's position `at` into `out` and moves `at` on. `None` if none wait.
    pub(crate) fn read_transmit(&self, at: &mut u64, out: &mut [Complex32]) -> Option<IqRead> {
        let (start, n) = self.transmit.read_at(*at, out);
        *at = start + n as u64;
        self.transmit.mark_read(*at);
        (n > 0).then(|| self.iq_read(n))
    }

    /// The transmit samples written after position `at`.
    pub(crate) fn transmit_queued(&self, at: u64) -> u64 {
        self.transmit.written().saturating_sub(at)
    }

    /// The rate of the transmit samples written last, 0 before the first.
    pub(crate) fn transmit_rate(&self) -> f64 {
        f64::from_bits(self.tx_rate.load(Ordering::Relaxed))
    }

    /// The centre of the transmit samples written last, 0 before the first.
    pub(crate) fn transmit_centre(&self) -> f64 {
        f64::from_bits(self.tx_centre.load(Ordering::Relaxed))
    }

    /// States the start of the burst whose first sample is the next transmit sample written: that sample is on the
    /// air at `utc`, seconds since 1970-01-01 UTC. The burst is written before that time, early by at least the
    /// latency of the transmitter. A burst without a stated start goes out as it comes. Only the thread that writes
    /// the transmit samples calls it.
    pub(crate) fn transmit_start(&self, utc: f64) {
        self.tx_starts
            .push(&[self.transmit.written(), utc.to_bits()]);
    }

    /// Sets what [`HostStreams::transmit_lead`] gives the graph: how long before its stated start a burst must be
    /// written. Zero while it is not known.
    pub(crate) fn set_transmit_lead(&self, lead: std::time::Duration) {
        let ns = u64::try_from(lead.as_nanos()).unwrap_or(u64::MAX);
        self.tx_lead.store(ns, Ordering::Relaxed);
    }

    /// The stated start of the burst whose first sample is at position `at`, if it has one that is a finite time.
    /// `next` is the place of the reader in the stated starts: it moves past those of bursts before `at`.
    pub(crate) fn burst_start(&self, at: u64, next: &mut u64) -> Option<f64> {
        let mut e = [0_u64; 2];
        while self.tx_starts.written() >= *next + 2 {
            let (i, n) = self.tx_starts.read_at(*next, &mut e);
            if i % 2 == 1 {
                // A start written over in part: the reader goes to the next whole one.
                *next = i + 1;
                continue;
            }
            *next = i;
            if n < 2 {
                break;
            }
            match e[0].cmp(&at) {
                std::cmp::Ordering::Less => *next = i + 2,
                std::cmp::Ordering::Equal => {
                    return Some(f64::from_bits(e[1])).filter(|utc| utc.is_finite());
                }
                std::cmp::Ordering::Greater => break,
            }
        }
        None
    }

    /// The samples that the transmit stream holds at most.
    pub(crate) fn transmit_room(&self) -> usize {
        usize::try_from(self.transmit.cap()).unwrap_or(usize::MAX)
    }

    /// The samples from position `at` to the end of the burst at end index `next_end`, written over or not. `None` if
    /// that end is not queued.
    pub(crate) fn burst_len(&self, at: u64, next_end: u64) -> Option<usize> {
        if self.tx_ends.written() <= next_end {
            return None;
        }
        let mut e = [0_u64];
        let (_, n) = self.tx_ends.read_at(next_end, &mut e);
        (n == 1).then(|| usize::try_from(e[0].saturating_sub(at)).unwrap_or(usize::MAX))
    }

    /// Copies the transmit samples from position `at` into `out`, up to the end of the burst at end index `next_end`,
    /// and moves `at` on. If the copy reaches the end of the burst, moves `next_end` on and returns `end` true. A burst
    /// whose samples were written over still returns its end. `None` if no sample and no end waits.
    pub(crate) fn read_burst(
        &self,
        at: &mut u64,
        next_end: &mut u64,
        out: &mut [Complex32],
    ) -> Option<BurstRead> {
        let mut e = [0_u64];
        let end = if self.tx_ends.written() > *next_end {
            let (i, n) = self.tx_ends.read_at(*next_end, &mut e);
            *next_end = i;
            (n == 1).then_some(e[0])
        } else {
            None
        };
        let oldest = self.transmit.written().saturating_sub(self.transmit.cap());
        let from = (*at).max(oldest);
        let limit = match end {
            Some(e) if e <= from => {
                *at = (*at).max(e);
                *next_end += 1;
                self.transmit.mark_read(*at);
                return Some(BurstRead {
                    iq: self.iq_read(0),
                    end: true,
                });
            }
            Some(e) => usize::try_from(e - from)
                .unwrap_or(usize::MAX)
                .min(out.len()),
            None => out.len(),
        };
        let (start, n) = self.transmit.read_at(from, &mut out[..limit]);
        *at = start + n as u64;
        self.transmit.mark_read(*at);
        let ended = end == Some(*at);
        if ended {
            *next_end += 1;
        }
        (n > 0 || ended).then(|| BurstRead {
            iq: self.iq_read(n),
            end: ended,
        })
    }

    fn iq_read(&self, samples: usize) -> IqRead {
        IqRead {
            samples,
            rate_hz: f64::from_bits(self.tx_rate.load(Ordering::Relaxed)),
            centre_hz: f64::from_bits(self.tx_centre.load(Ordering::Relaxed)),
        }
    }
}

/// The host audio input of one session ([`Engine::audio_input`](crate::Engine::audio_input)). It holds the streams of
/// the session, so a write takes no lock and allocates nothing: a real-time audio thread can call it.
#[derive(Clone)]
pub struct AudioInput(pub(crate) Arc<Streams>);

impl AudioInput {
    /// As [`Engine::write_audio`](crate::Engine::write_audio), without a lock: appends interleaved frames of
    /// `channels` channels (1, or 2 as left then right) at `rate_hz`. Never waits and allocates nothing. One thread at
    /// a time writes the audio of a session.
    ///
    /// # Errors
    ///
    /// [`crate::EngineError::AudioChannels`] for a channel count that is not 1 or 2, or
    /// [`crate::EngineError::Config`] with [`sdrkit_core::ConfigError::RateTooLow`] for a rate that is not above 0 or
    /// not finite.
    pub fn write(
        &self,
        samples: &[f32],
        channels: u16,
        rate_hz: f64,
    ) -> Result<(), crate::EngineError> {
        if !(1..=2).contains(&channels) {
            return Err(crate::EngineError::AudioChannels { got: channels });
        }
        if !(rate_hz.is_finite() && rate_hz > 0.0) {
            return Err(sdrkit_core::ConfigError::RateTooLow {
                need: f64::MIN_POSITIVE,
                got: rate_hz,
            }
            .into());
        }
        self.0.write_audio_in(samples, channels, rate_hz);
        Ok(())
    }
}

/// Transmit samples of a burst, copied by [`Streams::read_burst`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct BurstRead {
    pub(crate) iq: IqRead,
    /// The last sample copied ends the burst.
    pub(crate) end: bool,
}

impl HostStreams for Streams {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        match (port, data) {
            (
                HostPort::Out,
                HostData::Audio {
                    samples,
                    rate_hz,
                    channels,
                },
            ) => self.write_audio(samples, rate_hz, channels),
            (
                HostPort::Transmit,
                HostData::Iq {
                    samples,
                    rate_hz,
                    centre_hz,
                    end,
                    start,
                },
            ) => {
                self.transmit_format(rate_hz, centre_hz);
                if let Some(utc) = start {
                    self.transmit_start(utc);
                }
                self.transmit.push(samples);
                if end {
                    self.tx_ends.push(&[self.transmit.written()]);
                    // The transmission that sent speech ended. An armed command waits for the samples after the mark: a
                    // new transmission cuts the one before with a mark.
                    let _ = self.voice.compare_exchange(
                        VOICE_ON,
                        VOICE_OFF,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    // The sender keeps no host audio after the end; one that goes on says so in its next pass.
                    self.kept.store(0, Ordering::Release);
                } else if !samples.is_empty() {
                    let _ = self.voice.compare_exchange(
                        VOICE_ARMED,
                        VOICE_ON,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                }
            }
            (HostPort::Out, HostData::Iq { .. })
            | (HostPort::Transmit, HostData::Audio { .. })
            | (HostPort::Microphone, _) => {}
        }
    }

    fn read(&self, port: HostPort, out: &mut [f32]) -> usize {
        match port {
            HostPort::Microphone => self.read_audio_in(1, out),
            HostPort::Out | HostPort::Transmit => 0,
        }
    }

    fn read_channels(&self, port: HostPort, channels: u16, out: &mut [f32]) -> usize {
        match port {
            HostPort::Microphone => self.read_audio_in(channels, out),
            HostPort::Out | HostPort::Transmit => 0,
        }
    }

    fn kept(&self, port: HostPort) {
        if port == HostPort::Microphone {
            self.kept.store(KEPT_PASSES, Ordering::Release);
        }
    }

    fn transmit_format(&self, rate_hz: f64, centre_hz: f64) {
        self.tx_rate.store(rate_hz.to_bits(), Ordering::Relaxed);
        self.tx_centre.store(centre_hz.to_bits(), Ordering::Relaxed);
    }

    fn transmit_lead(&self) -> std::time::Duration {
        std::time::Duration::from_nanos(self.tx_lead.load(Ordering::Relaxed))
    }
}

/// A tap that the host opened on a port of a session's graph.
pub(crate) struct Tap {
    pub(crate) id: u32,
    pub(crate) reader: TapReader,
    /// The chunk that the host reads now, and how much of it the host copied.
    pub(crate) chunk: Buffer,
    pub(crate) taken: usize,
}

/// Copies the values of tap `t` that wait into `out`, from where the last call stopped in the piece it reads. Complex samples
/// go as I then Q. `None` if nothing waits, or the stream is of another kind than complex, real or audio.
pub(crate) fn read_tap(t: &mut Tap, out: &mut [f32]) -> Option<crate::TapRead> {
    if t.taken >= values(&t.chunk.data) {
        if !t.reader.pop(&mut t.chunk) {
            return None;
        }
        t.taken = 0;
    }
    let left = values(&t.chunk.data) - t.taken;
    let n = left.min(out.len());
    let kind = match &t.chunk.data {
        Data::Iq(v) => {
            // Whole samples only.
            let n = n & !1;
            for (k, o) in out[..n].iter_mut().enumerate() {
                let c = v[(t.taken + k) / 2];
                *o = if (t.taken + k).is_multiple_of(2) {
                    c.re
                } else {
                    c.im
                };
            }
            t.taken += n;
            return Some(tap_read(t, Kind::Iq, n));
        }
        Data::Real(v) => {
            out[..n].copy_from_slice(&v[t.taken..t.taken + n]);
            Kind::Real
        }
        Data::Audio(a) => {
            out[..n].copy_from_slice(&a.samples[t.taken..t.taken + n]);
            Kind::Audio
        }
        _ => return None,
    };
    t.taken += n;
    Some(tap_read(t, kind, n))
}

fn tap_read(t: &Tap, kind: Kind, values: usize) -> crate::TapRead {
    crate::TapRead {
        kind,
        values,
        rate_hz: t.reader.rate(),
        centre_hz: t.reader.centre_hz(),
        dropped: t.reader.dropped_items(),
    }
}

/// The values in a piece of a tap: two for each complex sample.
fn values(d: &Data) -> usize {
    match d {
        Data::Iq(v) => 2 * v.len(),
        Data::Real(v) => v.len(),
        Data::Audio(a) => a.samples.len(),
        _ => 0,
    }
}

/// What only the host's calls touch: its positions in the streams of a session, and its taps.
#[derive(Default)]
pub(crate) struct Readers {
    pub(crate) audio_at: u64,
    pub(crate) runs: [Run; RUNS],
    pub(crate) transmit_at: u64,
    pub(crate) taps: Vec<Tap>,
    pub(crate) next_tap: u32,
}

impl Default for Run {
    fn default() -> Self {
        Run {
            start: 0,
            rate_hz: 0.0,
            channels: 1,
            layout: ChannelLayout::Mono,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iq(n: usize) -> Vec<Complex32> {
        (0..n).map(|i| Complex32::new(i as f32, 0.0)).collect()
    }

    fn write(s: &Streams, samples: &[Complex32], end: bool) {
        s.write(
            HostPort::Transmit,
            HostData::Iq {
                samples,
                rate_hz: 1e6,
                centre_hz: 915e6,
                end,
                start: None,
            },
        );
    }

    /// A picture packet of the receiver's picture `id` in `state`, with `bytes` as its only byte.
    fn picture(id: u64, state: PictureState, name: &str, byte: u8) -> Elementary {
        let layout = sdrkit_core::PixelLayout::packed(sdrkit_core::PixelFormat::Grey8, 1, 1);
        let mut format = MediaFormat::new(layout.pixel.codec(), sdrkit_core::Packaging::Raw);
        format.pixels = Some(layout);
        Elementary {
            kind: StreamKind::Picture,
            packet: sdrkit_core::Packet::new(Arc::from(&[byte][..]), 0.0),
            format: Arc::new(format),
            picture: Some(PictureTag {
                id,
                state,
                name: Some(Arc::from(name)),
            }),
        }
    }

    /// Every picture that waits after counter `after`: (number, state, name, byte).
    fn pictures(s: &Streams, mut after: u64) -> Vec<(u64, PictureState, String, u8)> {
        let (mut out, mut bytes) = (vec![], vec![]);
        while let Some(m) = s.read_picture(after, &mut bytes) {
            let t = m.picture.expect("a picture states its number");
            out.push((t.id, t.state, t.name.unwrap().to_string(), bytes[0]));
            after = m.counter;
        }
        out
    }

    #[test]
    fn two_pictures_in_a_row_have_two_numbers_and_the_first_is_finished() {
        use PictureState::{Finished, Growing};
        let s = Streams::new(1, 1);
        // The same size and the same name: the number alone tells the pictures apart.
        let mut pending = vec![
            picture(7, Growing, "Robot 36", 1),
            picture(7, Growing, "Robot 36", 2),
            picture(7, Finished, "Robot 36", 3),
            picture(8, Growing, "Robot 36", 4),
        ];
        s.offer_media(&mut pending);
        // The queue holds the newest state of each picture.
        assert_eq!(
            pictures(&s, 0),
            [
                (1, Finished, "Robot 36".to_string(), 3),
                (2, Growing, "Robot 36".to_string(), 4)
            ]
        );
    }

    #[test]
    fn interleaved_pictures_keep_their_numbers() {
        use PictureState::{Finished, Growing};
        let s = Streams::new(1, 1);
        let mut after = 0;
        let mut seen = vec![];
        for (id, state, name, byte) in [
            (1, Growing, "ch1", 1),
            (2, Growing, "ch2", 2),
            (1, Growing, "ch1", 3),
            (2, Finished, "ch2", 4),
            (1, Finished, "ch1", 5),
        ] {
            s.offer_media(&mut vec![picture(id, state, name, byte)]);
            let m = s.read_picture(after, &mut vec![]).unwrap();
            after = m.counter;
            let t = m.picture.unwrap();
            seen.push((t.id, t.state));
        }
        assert_eq!(
            seen,
            [
                (1, Growing),
                (2, Growing),
                (1, Growing),
                (2, Finished),
                (1, Finished)
            ]
        );
        // A finished picture that the receiver draws again keeps its number.
        s.offer_media(&mut vec![picture(2, Finished, "ch2", 6)]);
        assert_eq!(pictures(&s, after), [(2, Finished, "ch2".to_string(), 6)]);
    }

    #[test]
    fn a_picture_that_grows_ends_when_the_session_takes_another_receiver() {
        use PictureState::{Finished, Growing};
        let s = Streams::new(1, 1);
        s.offer_media(&mut vec![
            picture(1, Growing, "a", 1),
            picture(2, Finished, "b", 2),
        ]);
        s.end_pictures();
        // The next receiver uses the number 1 again: it is another picture.
        s.offer_media(&mut vec![picture(1, Growing, "c", 3)]);
        assert_eq!(
            pictures(&s, 0),
            [
                (2, Finished, "b".to_string(), 2),
                (1, Finished, "a".to_string(), 1),
                (3, Growing, "c".to_string(), 3)
            ]
        );
    }

    #[test]
    fn a_picture_without_a_number_is_whole() {
        let s = Streams::new(1, 1);
        let mut whole = picture(
            u64::from(sdrkit_core::Picture::WHOLE),
            PictureState::Growing,
            "slide",
            1,
        );
        let mut bare = whole.clone();
        bare.picture = None;
        whole.packet = sdrkit_core::Packet::new(Arc::from(&[2u8][..]), 0.0);
        s.offer_media(&mut vec![bare, whole.clone(), whole]);
        let got = pictures_loose(&s);
        assert_eq!(
            got,
            [
                (1, PictureState::Finished),
                (2, PictureState::Finished),
                (3, PictureState::Finished)
            ]
        );
    }

    /// The number and the state of every picture that waits.
    fn pictures_loose(s: &Streams) -> Vec<(u64, PictureState)> {
        let (mut out, mut after) = (vec![], 0);
        while let Some(m) = s.read_picture(after, &mut vec![]) {
            let t = m.picture.unwrap();
            out.push((t.id, t.state));
            after = m.counter;
        }
        out
    }

    #[test]
    fn a_read_stops_at_the_end_of_a_burst() {
        let s = Streams::new(1, 64);
        write(&s, &iq(10), true);
        write(&s, &iq(5), false);
        let (mut at, mut next) = (0, 0);
        let mut out = [Complex32::default(); 32];
        let r = s.read_burst(&mut at, &mut next, &mut out).unwrap();
        assert_eq!((r.iq.samples, r.end), (10, true));
        let r = s.read_burst(&mut at, &mut next, &mut out).unwrap();
        assert_eq!((r.iq.samples, r.end), (5, false));
        assert!(s.read_burst(&mut at, &mut next, &mut out).is_none());
        write(&s, &[], true);
        let r = s.read_burst(&mut at, &mut next, &mut out).unwrap();
        assert_eq!((r.iq.samples, r.end), (0, true));
    }

    #[test]
    fn decoded_frames_have_a_small_bound_and_their_buffers_return() {
        use sdrkit_core::media::{Codec, Packaging, PixelFormat};
        use sdrkit_core::Packet;
        let s = Streams::new(1, 1);
        let mut pool = sdrkit_graph::PacketPool::default();
        let frame = 720 * 576 * 3 / 2;
        let video = Arc::new(MediaFormat::new(
            Codec::Pixels(PixelFormat::I420),
            Packaging::Raw,
        ));
        let sound = Arc::new(MediaFormat::new(Codec::Mp2, Packaging::Raw));
        let mut buffers = std::collections::HashSet::new();
        let (mut after, mut missed, mut read) = (0_u64, 0_u64, 0_u64);
        let mut out = Vec::new();
        let mut take = |s: &Streams| {
            let m = s.read_media(after, &mut out)?;
            (missed, read, after) = (missed + m.missed, read + 1, m.counter);
            Some(())
        };
        for i in 0..500_u32 {
            let t = f64::from(i) / 25.0;
            let p = pool.make(frame, t, |b| b.fill(i as u8));
            buffers.insert(p.data.as_ptr() as usize);
            let mut pending = vec![
                Elementary {
                    kind: StreamKind::RawVideo,
                    packet: p,
                    format: Arc::clone(&video),
                    picture: None,
                },
                Elementary {
                    kind: StreamKind::Mp2,
                    packet: Packet::new(Arc::from(&[0_u8; 384][..]), t),
                    format: Arc::clone(&sound),
                    picture: None,
                },
            ];
            s.offer_media(&mut pending);
            // A slow host: one packet for every fifth frame.
            if i % 5 == 0 {
                take(&s);
            }
            let q = crate::feed::lock(&s.media);
            let frames = q.items.iter().filter(|(_, e)| raw(e)).count();
            assert!(
                q.raw_bytes <= RAW_VIDEO_KEEP_BYTES && frames <= 13,
                "{frames} frames held"
            );
            assert!(q.bytes <= MEDIA_KEEP_BYTES);
        }
        while take(&s).is_some() {}
        assert_eq!(read + missed, 1000);
        assert!(missed > 0);
        assert_eq!(s.media_lost(), missed);
        assert!(buffers.len() <= 16, "{} frame buffers", buffers.len());
    }

    #[test]
    fn a_burst_written_over_still_ends() {
        let s = Streams::new(1, 8);
        write(&s, &iq(20), true);
        assert_eq!(s.transmit_lost(), 12);
        let (mut at, mut next) = (0, 0);
        let mut out = [Complex32::default(); 32];
        let r = s.read_burst(&mut at, &mut next, &mut out).unwrap();
        assert_eq!((r.iq.samples, r.end), (8, true));
        assert_eq!(out[0].re, 12.0);
        write(&s, &iq(30), true);
        write(&s, &iq(3), true);
        let r = s.read_burst(&mut at, &mut next, &mut out).unwrap();
        assert_eq!((r.iq.samples, r.end), (5, true));
        assert_eq!(s.transmit_lost(), 12 + 25);
        let r = s.read_burst(&mut at, &mut next, &mut out).unwrap();
        assert_eq!((r.iq.samples, r.end), (3, true));
    }
}
