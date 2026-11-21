//! A WAV file as the microphone of a session: `run --tx-audio <file>` gives it to the session as host audio, in real
//! time, while the session asks for host audio (`session.audio_in_wanted`), so voice can be sent without a microphone.

use crate::error::CliError;
use sdrkit_host_engine::{AudioInput, Engine, OwnerRef, SessionId, AUDIO_IN_WANTED};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the feeder writes audio.
const STEP: Duration = Duration::from_millis(10);
/// How often the feeder reads whether the session wants audio.
const LOOK: Duration = Duration::from_millis(50);
/// The audio written ahead of real time, s: a little more than one step, far less than the half second the session keeps.
const LEAD_S: f64 = 0.03;

/// The mono audio of a WAV file.
#[derive(Clone, Debug)]
pub struct TxAudio {
    /// The samples, the channels of each frame averaged.
    pub samples: Vec<f32>,
    /// Frames per second.
    pub rate_hz: f64,
}

impl TxAudio {
    /// Reads the WAV file at `path`: integer or float samples, any channel count, mixed to mono.
    ///
    /// # Errors
    ///
    /// [`CliError::Wav`] if the file does not open or read.
    pub fn read(path: &Path) -> Result<TxAudio, CliError> {
        let mut r = hound::WavReader::open(path)?;
        let spec = r.spec();
        let channels = usize::from(spec.channels.max(1));
        let all: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
            hound::SampleFormat::Int => {
                let full = (1_i64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
                r.samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / full))
                    .collect::<Result<_, _>>()?
            }
        };
        let samples = all
            .chunks(channels)
            .map(|f| f.iter().sum::<f32>() / f.len() as f32)
            .collect();
        Ok(TxAudio {
            samples,
            rate_hz: f64::from(spec.sample_rate),
        })
    }
}

/// The thread that feeds a [`TxAudio`] to a session. Each time the session starts to want host audio, the file plays
/// from its start in real time; after its end the session gets no audio (the transmitter sends silence).
pub struct Feeder {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Feeder {
    /// Starts feeding `audio` to session `session` of `engine`.
    ///
    /// # Errors
    ///
    /// [`CliError::Engine`] if the session does not exist, [`CliError::Io`] if the thread does not start.
    pub fn start(
        engine: &Arc<Engine>,
        session: SessionId,
        audio: Arc<TxAudio>,
    ) -> Result<Feeder, CliError> {
        let input = engine.audio_input(session)?;
        let stop = Arc::new(AtomicBool::new(false));
        let (e, halt) = (Arc::clone(engine), Arc::clone(&stop));
        let thread = std::thread::Builder::new()
            .name("sdrkit-tx-audio".into())
            .spawn(move || feed(&e, session, &input, &audio, &halt))?;
        Ok(Feeder {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Feeder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// True if session `s` wants host audio now.
fn wanted(engine: &Engine, s: SessionId) -> bool {
    engine
        .owner(OwnerRef::Session(s), |o| {
            o.results().fields().get(&AUDIO_IN_WANTED)
        })
        .ok()
        .flatten()
        .unwrap_or(false)
}

/// The loop of the feeder, until `stop`.
fn feed(engine: &Engine, s: SessionId, input: &AudioInput, audio: &TxAudio, stop: &AtomicBool) {
    // The start of the present playing and the frames written since, while the session wants audio.
    let mut playing: Option<(Instant, usize)> = None;
    let mut looked = Instant::now() - LOOK;
    let mut want = false;
    while !stop.load(Ordering::Relaxed) {
        if looked.elapsed() >= LOOK {
            looked = Instant::now();
            want = wanted(engine, s);
        }
        playing = match (want, playing) {
            (false, _) => None,
            (true, None) => Some((Instant::now(), 0)),
            (true, Some(p)) => Some(p),
        };
        if let Some((t0, done)) = &mut playing {
            let due = ((t0.elapsed().as_secs_f64() + LEAD_S) * audio.rate_hz) as usize;
            let end = due.min(audio.samples.len());
            if end > *done
                && input
                    .write(&audio.samples[*done..end], 1, audio.rate_hz)
                    .is_ok()
            {
                *done = end;
            }
        }
        std::thread::sleep(STEP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stereo_file_of_integers_reads_as_mono_floats() {
        let dir = std::env::temp_dir().join(format!("sdrkit-txaudio-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("two.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for (l, r) in [(16384_i16, 0_i16), (-32768, -32768)] {
            w.write_sample(l).unwrap();
            w.write_sample(r).unwrap();
        }
        w.finalize().unwrap();
        let a = TxAudio::read(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(a.rate_hz, 8000.0);
        assert_eq!(a.samples, vec![0.25, -1.0]);
    }
}
