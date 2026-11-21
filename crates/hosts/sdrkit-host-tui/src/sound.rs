//! The audio of a session to the default speaker. A thread of its own waits for the audio, opens the device and holds
//! the stream, so the screen does not wait for the device. Only the speaker reads the audio of the session, through
//! its own session handle, and plays every sample that it read.

use crate::api::{AudioInfo, Handle};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The level and the mute, which the callback of the output and the screen share.
#[derive(Debug, Default)]
pub struct Meter {
    /// The peak of the last buffer, as the bits of an `f32` from 0 to 1.
    peak: AtomicU32,
    /// The speaker plays silence.
    muted: AtomicBool,
}

impl Meter {
    /// Returns the peak of the last buffer in dB full scale, or `None` for silence.
    pub fn peak_dbfs(&self) -> Option<f32> {
        let p = f32::from_bits(self.peak.load(Ordering::Relaxed));
        (p > 1e-6).then(|| 20.0 * p.log10())
    }

    /// Returns true if the speaker plays silence.
    pub fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Turns the mute on or off.
    pub fn set_muted(&self, on: bool) {
        self.muted.store(on, Ordering::Relaxed);
    }
}

/// Why the speaker did not start.
#[derive(Debug)]
pub enum SoundError {
    /// The system has no output device.
    NoDevice,
    /// The device refused the stream.
    Build(cpal::BuildStreamError),
    /// The stream did not start.
    Play(cpal::PlayStreamError),
    /// The library did not return the audio of the session.
    Session(crate::api::Error),
    /// The sound thread did not start.
    Thread,
}

impl fmt::Display for SoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoundError::NoDevice => f.write_str("no sound output device"),
            SoundError::Build(e) => write!(f, "sound output: {e}"),
            SoundError::Play(e) => write!(f, "sound output: {e}"),
            SoundError::Session(e) => e.fmt(f),
            SoundError::Thread => f.write_str("the sound thread did not start"),
        }
    }
}

impl std::error::Error for SoundError {}

/// Where a speaker reads its audio: a session, or the scripted audio of a test.
pub trait Source: Send {
    /// Copies the audio that waits into `out`: whole frames of one rate and one channel count, 0 frames if none waits.
    ///
    /// # Errors
    ///
    /// The code of the library.
    fn audio_read(&mut self, out: &mut [f32]) -> crate::api::Result<AudioInfo>;
}

impl Source for Handle {
    fn audio_read(&mut self, out: &mut [f32]) -> crate::api::Result<AudioInfo> {
        Handle::audio_read(self, out)
    }
}

/// The audio of the session that waits for an output of its format, and the handle that reads the session.
struct Feed {
    session: Box<dyn Source>,
    /// Audio read from the session and not played yet: whole frames from `at` on.
    kept: Vec<f32>,
    at: usize,
    /// The format of `kept`, and of the output that plays.
    rate_hz: f64,
    channels: u16,
}

/// State that the sound thread, the callback of the output and the screen share.
struct Line {
    feed: Mutex<Feed>,
    /// The session sent audio of another format than the output plays: the thread opens the output again.
    reopen: AtomicBool,
    /// The format that plays, for the screen: the rate as bits, 0 before the first audio.
    rate_bits: AtomicU64,
    channels: AtomicU32,
}

/// The speaker of a session. Dropping it stops the sound.
pub struct Speaker {
    line: Arc<Line>,
    /// The thread drops the stream when this sender is dropped.
    _stop: Sender<()>,
    /// An error of the sound thread, until [`Speaker::failed`] returned it.
    report: Option<Receiver<SoundError>>,
}

/// The time between two looks of the sound thread at the session and at the format.
const LOOK: Duration = Duration::from_millis(20);
/// The floats that the sound thread reads from the session at once while no output plays.
const KEPT: usize = 9600;

impl Speaker {
    /// Starts a thread that plays the audio of `session`, a handle that only the speaker uses, through `output`. The
    /// thread waits for the first audio, opens the output in the format of that audio and opens it again when the
    /// format changes. Audio that it reads while no output plays is kept and played first. Does not wait for the
    /// device: [`Speaker::failed`] returns an error of the thread.
    pub fn start(output: Arc<dyn Output>, session: impl Source + 'static, meter: Arc<Meter>) -> Speaker {
        let line = Arc::new(Line {
            feed: Mutex::new(Feed {
                session: Box::new(session),
                kept: Vec::with_capacity(2 * KEPT),
                at: 0,
                rate_hz: 0.0,
                channels: 0,
            }),
            reopen: AtomicBool::new(false),
            rate_bits: AtomicU64::new(0),
            channels: AtomicU32::new(0),
        });
        let (stop, stopped) = mpsc::channel::<()>();
        let (report, failed) = mpsc::channel();
        let shared = Arc::clone(&line);
        let on_thread = report.clone();
        let spawned = std::thread::Builder::new()
            .name("sdrkit-tui-sound".into())
            .spawn(move || {
                if let Err(e) = play(&output, &shared, &meter, &stopped) {
                    let _ = on_thread.send(e);
                }
            });
        if spawned.is_err() {
            let _ = report.send(SoundError::Thread);
        }
        Speaker {
            line,
            _stop: stop,
            report: Some(failed),
        }
    }

    /// Returns the rate in Hz and the channel count that play, or `None` before the first audio of the session.
    pub fn format(&self) -> Option<(f64, u16)> {
        let rate = f64::from_bits(self.line.rate_bits.load(Ordering::Relaxed));
        let channels = self.line.channels.load(Ordering::Relaxed);
        (rate > 0.0).then_some((rate, channels as u16))
    }

    /// Returns why the sound stopped, once, if it did. `None` while it plays or waits for audio.
    pub fn failed(&mut self) -> Option<SoundError> {
        match self.report.as_ref()?.try_recv() {
            Ok(e) => {
                self.report = None;
                Some(e)
            }
            Err(_) => None,
        }
    }
}

fn lock(feed: &Mutex<Feed>) -> std::sync::MutexGuard<'_, Feed> {
    feed.lock().unwrap_or_else(|p| p.into_inner())
}

/// The work of the sound thread: for each format of the audio, opens `output` and holds it until the format changes.
/// Returns when `stopped` loses its sender.
///
/// # Errors
///
/// [`SoundError`]: the library did not return the audio of the session, or the output did not open.
fn play(
    output: &Arc<dyn Output>,
    line: &Arc<Line>,
    meter: &Arc<Meter>,
    stopped: &Receiver<()>,
) -> Result<(), SoundError> {
    let goes_on = || matches!(stopped.recv_timeout(LOOK), Err(RecvTimeoutError::Timeout));
    loop {
        // No output plays here: the thread reads the session until audio waits, and keeps that audio.
        let (rate_hz, channels) = loop {
            let mut f = lock(&line.feed);
            let f = &mut *f;
            if f.at == f.kept.len() {
                f.kept.resize(KEPT, 0.0);
                f.at = 0;
                let read = f.session.audio_read(&mut f.kept);
                let n = read.as_ref().map_or(0, |i| i.frames * usize::from(i.channels));
                f.kept.truncate(n);
                let i = match read {
                    Ok(i) => i,
                    // The session is closed: there is no more audio.
                    Err(e) if e.is_dead_handle() => return Ok(()),
                    Err(e) => return Err(SoundError::Session(e)),
                };
                if n > 0 {
                    (f.rate_hz, f.channels) = (i.rate_hz, i.channels);
                }
            }
            if f.at < f.kept.len() {
                break (f.rate_hz, f.channels);
            }
            if !goes_on() {
                return Ok(());
            }
        };
        line.reopen.store(false, Ordering::Relaxed);
        line.channels.store(u32::from(channels), Ordering::Relaxed);
        line.rate_bits.store(rate_hz.to_bits(), Ordering::Relaxed);
        let stream = output.open(rate_hz, channels, fill(Arc::clone(line), Arc::clone(meter)))?;
        while !line.reopen.load(Ordering::Relaxed) {
            if !goes_on() {
                return Ok(());
            }
        }
        // The output stops before the next one opens, so one callback at a time reads the session.
        drop(stream);
    }
}

/// The function that an output calls for each buffer: it writes interleaved floats over the whole buffer and returns how
/// many of them, from the start, are audio of the session. The rest is silence.
pub type Fill = Box<dyn FnMut(&mut [f32]) -> usize + Send>;

/// An audio output device. The application plays through [`Device`]; a test counts what an output of its own receives.
pub trait Output: Send + Sync {
    /// Opens the output at `rate_hz` with `channels` and starts it: it calls `fill` for each buffer until the returned
    /// value is dropped. Runs on the sound thread, and the returned value stays on it.
    ///
    /// # Errors
    ///
    /// [`SoundError`].
    fn open(&self, rate_hz: f64, channels: u16, fill: Fill) -> Result<Box<dyn std::any::Any>, SoundError>;
}

/// The default speaker of the system.
#[derive(Debug, Default)]
pub struct Device;

impl Output for Device {
    fn open(&self, rate_hz: f64, channels: u16, mut fill: Fill) -> Result<Box<dyn std::any::Any>, SoundError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(SoundError::NoDevice)?;
        let config = cpal::StreamConfig {
            channels,
            // An audio rate is far below `u32::MAX`, and the cast saturates.
            sample_rate: cpal::SampleRate(rate_hz.round().max(1.0) as u32),
            buffer_size: cpal::BufferSize::Default,
        };
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    fill(data);
                },
                |_| {},
                None,
            )
            .map_err(SoundError::Build)?;
        stream.play().map_err(SoundError::Play)?;
        Ok(Box::new(stream))
    }
}

/// Returns the function that fills a buffer of the output that plays the format of `line` from the audio of its session:
/// the kept audio first, silence where the session has none. Audio of another format is kept for the next output. The
/// level goes to `meter`.
fn fill(line: Arc<Line>, meter: Arc<Meter>) -> Fill {
    let (rate_hz, channels) = {
        let f = lock(&line.feed);
        (f.rate_hz, f.channels)
    };
    Box::new(move |data: &mut [f32]| {
        let mut f = lock(&line.feed);
        let f = &mut *f;
        let mut n = 0;
        if f.at < f.kept.len() && (f.rate_hz, f.channels) == (rate_hz, channels) {
            n = (f.kept.len() - f.at).min(data.len());
            data[..n].copy_from_slice(&f.kept[f.at..f.at + n]);
            f.at += n;
        }
        if f.at == f.kept.len() {
            if let Ok(i) = f.session.audio_read(&mut data[n..]) {
                let got = i.frames * usize::from(i.channels);
                if got > 0 && (i.rate_hz, i.channels) != (rate_hz, channels) {
                    f.kept.clear();
                    f.kept.extend_from_slice(&data[n..n + got]);
                    f.at = 0;
                    (f.rate_hz, f.channels) = (i.rate_hz, i.channels);
                } else {
                    n += got;
                }
            }
        }
        if (f.rate_hz, f.channels) != (rate_hz, channels) {
            line.reopen.store(true, Ordering::Relaxed);
        }
        let peak = data[..n].iter().fold(0.0f32, |p, s| p.max(s.abs()));
        meter.peak.store(peak.to_bits(), Ordering::Relaxed);
        let from = if meter.muted() { 0 } else { n };
        data[from..].fill(0.0);
        n
    })
}
