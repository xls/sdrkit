//! fm_audio: receives FM, writes the audio to a WAV file, and prints the station name from RDS, stereo and the signal
//! quality from the typed result set of the stack.
//!
//! `cargo run -p sdrkit-examples --example fm_audio -- [source] [seconds] [MHz] [file]`
//!
//! - `source`: `synth:fm` (the default) is a test signal. A radio is an address such as `rtl_tcp:192.168.1.20:1234`.
//! - `seconds`: how much audio to write, 5 by default.
//! - `MHz`: the frequency to start on. The test signal needs none.
//! - `file`: the WAV file, `fm_audio.wav` by default.

use sdrkit_core::interface::ReadFields;
use sdrkit_host_engine::OwnerRef;
use sdrkit_stack_fm::typed::FmFields;
use std::error::Error;
use std::io::{Seek, SeekFrom, Write};
use std::time::Duration;

/// The 44-byte header of a 16-bit PCM WAV file with `data_bytes` bytes of samples.
fn wav_header(rate: u32, channels: u16, data_bytes: u32) -> Vec<u8> {
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&channels.to_le_bytes());
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    h.extend_from_slice(&(channels * 2).to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data_bytes.to_le_bytes());
    h
}

fn main() -> Result<(), Box<dyn Error>> {
    let arg = |n: usize| std::env::args().nth(n);
    let source = arg(1).unwrap_or_else(|| "synth:fm".into());
    let seconds: f64 = arg(2).map_or(Ok(5.0), |s| s.parse())?;
    let mhz: Option<f64> = arg(3).map(|s| s.parse()).transpose()?;
    let path = arg(4).unwrap_or_else(|| "fm_audio.wav".into());

    // The engine with every stack family, the radio, and a session of the stack "fm" on it.
    let sdr = sdrkit_host_catalog::engine()?;
    let radio = sdr.open(&source)?;
    let session = sdr.start("fm", radio, mhz.map(|m| m * 1e6))?;

    let mut wav = std::fs::File::create(&path)?;
    wav.write_all(&wav_header(0, 0, 0))?;
    let mut audio = vec![0f32; 16384];
    let (mut rate, mut channels, mut frames, mut bytes) = (0u32, 0u16, 0usize, 0u32);
    let passes = (seconds * 150.0) as usize + 50;
    for pass in 0..passes {
        if rate > 0 && frames as f64 >= seconds * f64::from(rate) {
            break;
        }
        // Whole interleaved frames of one rate and one channel count, or None if no audio waits.
        if let Some(got) = sdr.audio(session, &mut audio)? {
            (rate, channels) = (got.rate_hz as u32, got.channels);
            let pcm: Vec<u8> = audio[..got.frames * usize::from(got.channels)]
                .iter()
                .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
                .collect();
            wav.write_all(&pcm)?;
            bytes += pcm.len() as u32;
            frames += got.frames;
        }
        if pass % 50 == 49 {
            // The result fields of the stack, read as the typed struct of its interface.
            let line = sdr.owner(OwnerRef::Stack(session), |o| {
                FmFields::read(o.results().fields()).map(|f| {
                    let sound = if f.stereo { "stereo" } else { "mono" };
                    format!("station \"{}\"  {sound}  SNR {:.1} dB", f.station, f.snr_db)
                })
            })?;
            if let Some(line) = line {
                println!("{line}");
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    wav.seek(SeekFrom::Start(0))?;
    wav.write_all(&wav_header(rate, channels, bytes))?;
    sdr.stop_session(session)?;
    let written = if rate > 0 {
        frames as f64 / f64::from(rate)
    } else {
        0.0
    };
    println!("wrote {written:.1} s of audio to {path}");
    if frames == 0 {
        return Err("no audio".into());
    }
    Ok(())
}
