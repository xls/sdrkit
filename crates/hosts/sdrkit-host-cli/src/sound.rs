//! Where the sound of a session goes: the speaker, a WAV file or nowhere.

use crate::error::{CliError, SoundError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sdrkit_host_engine::{Engine, SessionId};
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;

/// Where the sound goes, as the command line names it.
#[derive(Clone, Debug, PartialEq)]
pub enum SoundTo {
    /// The computer's default output.
    Speaker,
    /// A WAV file of 32-bit floats.
    Wav(PathBuf),
    /// Nowhere.
    Off,
}

/// The text is not a sound output.
#[derive(Debug, thiserror::Error)]
#[error("write speaker, none, or a file name that ends in .wav")]
pub struct NotASoundOutput;

impl std::str::FromStr for SoundTo {
    type Err = NotASoundOutput;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "speaker" => SoundTo::Speaker,
            "none" | "off" => SoundTo::Off,
            p if p.ends_with(".wav") => SoundTo::Wav(PathBuf::from(p)),
            _ => return Err(NotASoundOutput),
        })
    }
}

/// The sound output of one session.
pub enum Sound {
    /// No output.
    Off,
    /// The speaker. The stream starts when the first audio arrives with its rate. Its callback copies the session's audio
    /// straight into the device's buffer on the device's own thread.
    Speaker(Option<cpal::Stream>),
    /// WAV files, written by the control loop.
    Wav(WavOut),
}

/// The WAV files of one session. The first opens when the first audio arrives with its rate. When the rate or the
/// number of channels changes, the file ends and the next one opens: `name.wav`, then `name-2.wav`, `name-3.wav`.
pub struct WavOut {
    /// The path that the command line named.
    path: PathBuf,
    /// The number of the open file: 1 for the named path.
    number: u32,
    writer: Option<hound::WavWriter<BufWriter<std::fs::File>>>,
}

impl WavOut {
    /// The path of file `number`.
    fn path_of(&self, number: u32) -> PathBuf {
        if number == 1 {
            return self.path.clone();
        }
        let stem = self
            .path
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        self.path.with_file_name(format!("{stem}-{number}.wav"))
    }

    /// Writes `samples` in `spec`. Returns the path of the file that a change of the format opened.
    fn write(
        &mut self,
        spec: hound::WavSpec,
        samples: &[f32],
    ) -> Result<Option<PathBuf>, CliError> {
        let mut opened = None;
        if self.writer.as_ref().is_some_and(|w| w.spec() != spec) {
            if let Some(w) = self.writer.take() {
                w.finalize()?;
            }
            self.number += 1;
            opened = Some(self.path_of(self.number));
        }
        if self.writer.is_none() {
            self.writer = Some(hound::WavWriter::create(self.path_of(self.number), spec)?);
        }
        if let Some(w) = self.writer.as_mut() {
            for x in samples {
                w.write_sample(*x)?;
            }
        }
        Ok(opened)
    }
}

impl Sound {
    /// The output for `to`.
    pub fn new(to: &SoundTo) -> Sound {
        match to {
            SoundTo::Speaker => Sound::Speaker(None),
            SoundTo::Wav(p) => Sound::Wav(WavOut {
                path: p.clone(),
                number: 1,
                writer: None,
            }),
            SoundTo::Off => Sound::Off,
        }
    }

    /// Moves the audio of session `id` that waits to its output. `buf` is the buffer of the control loop. Returns the
    /// path of the WAV file that a change of the format of the audio opened.
    ///
    /// # Errors
    ///
    /// [`CliError::Sound`] if the speaker does not start, [`CliError::Wav`] if the file does not write.
    pub fn pump(
        &mut self,
        engine: &Arc<Engine>,
        id: SessionId,
        buf: &mut [f32],
    ) -> Result<Option<PathBuf>, CliError> {
        let mut opened = None;
        match self {
            Sound::Off => while engine.audio(id, buf)?.is_some() {},
            Sound::Speaker(Some(_)) => {}
            Sound::Speaker(slot @ None) => {
                if let Some(first) = engine.audio(id, buf)? {
                    *slot = Some(speaker(engine, id, first.rate_hz, first.channels)?);
                }
            }
            Sound::Wav(out) => {
                while let Some(r) = engine.audio(id, buf)? {
                    let spec = hound::WavSpec {
                        channels: r.channels,
                        sample_rate: rate_u32(r.rate_hz),
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float,
                    };
                    let n = r.frames * usize::from(r.channels);
                    if let Some(p) = out.write(spec, &buf[..n])? {
                        opened = Some(p);
                    }
                }
            }
        }
        Ok(opened)
    }

    /// Ends the output: the WAV file gets its final header.
    ///
    /// # Errors
    ///
    /// [`CliError::Wav`].
    pub fn finish(self) -> Result<(), CliError> {
        if let Sound::Wav(WavOut {
            writer: Some(w), ..
        }) = self
        {
            w.finalize()?;
        }
        Ok(())
    }
}

/// A rate in whole hertz for a device or a file.
fn rate_u32(hz: f64) -> u32 {
    // A rate of audio is far below `u32::MAX`, and the cast saturates.
    hz.round().max(1.0) as u32
}

/// Starts the default output at `rate_hz` with `channels`, fed from the audio of session `id`.
fn speaker(
    engine: &Arc<Engine>,
    id: SessionId,
    rate_hz: f64,
    channels: u16,
) -> Result<cpal::Stream, SoundError> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or(SoundError::NoDevice)?;
    let config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(rate_u32(rate_hz)),
        buffer_size: cpal::BufferSize::Default,
    };
    let e = Arc::clone(engine);
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let n = match e.audio(id, data) {
                Ok(Some(r)) => r.frames * usize::from(r.channels),
                _ => 0,
            };
            data[n..].fill(0.0);
        },
        |err| eprintln!("sound output: {err}"),
        None,
    )?;
    stream.play()?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(channels: u16) -> hound::WavSpec {
        hound::WavSpec {
            channels,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        }
    }

    /// Six channels and then two give two complete files: `name.wav` and `name-2.wav`.
    #[test]
    fn a_change_of_channels_opens_the_next_file() {
        let dir = std::env::temp_dir().join(format!("sdrkit-cli-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut out = WavOut {
            path: dir.join("tv.wav"),
            number: 1,
            writer: None,
        };
        assert_eq!(out.write(spec(6), &[0.1; 600]).unwrap(), None);
        let next = out.write(spec(2), &[0.2; 200]).unwrap();
        assert_eq!(next, Some(dir.join("tv-2.wav")));
        assert_eq!(out.write(spec(2), &[0.2; 200]).unwrap(), None);
        Sound::Wav(out).finish().unwrap();
        for (name, channels, frames) in [("tv.wav", 6, 100), ("tv-2.wav", 2, 200)] {
            let r = hound::WavReader::open(dir.join(name)).unwrap();
            assert_eq!(r.spec().channels, channels, "{name}");
            assert_eq!(r.duration(), frames, "{name}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
