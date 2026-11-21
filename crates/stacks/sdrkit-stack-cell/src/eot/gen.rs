//! The end-of-train test signal: frames of bit sync, frame sync and body as AFSK at 1200 bit/s (1200 Hz for a one, 1800 Hz for
//! a zero) on FM with 3000 Hz of deviation, at unit amplitude, a pause of 0.05 s between frames, repeated without end.

use super::frame::{self, EotData, BIT_SYNC_BITS, FRAME_SYNC, FRAME_SYNC_BITS};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The deviation of the test signal, Hz.
pub const DEVIATION_HZ: f64 = 3000.0;
/// The pause between frames, s.
pub const GAP_S: f64 = 0.05;

/// The unit address of the test frames.
pub const GEN_UNIT: Setting<field::I64> = Setting::new("gen_unit", "Test signal unit", "", 12345)
    .range(0.0, 131_071.0, 1.0)
    .section("Test signal")
    .advanced();
/// The brake pipe pressure of the test frames.
pub const GEN_PSI: Setting<field::I64> =
    Setting::new("gen_psi", "Test signal brake pipe", "psi", 90)
        .range(0.0, 127.0, 1.0)
        .section("Test signal")
        .advanced();
/// The message type of the test frames.
pub const GEN_TYPE: Setting<field::I64> =
    Setting::new("gen_type", "Test signal message type", "", 0)
        .range(0.0, 7.0, 1.0)
        .section("Test signal")
        .advanced();
/// The motion bit of the test frames.
pub const GEN_MOTION: Setting<field::Bool> =
    Setting::new("gen_motion", "Test signal moving", "", true)
        .section("Test signal")
        .advanced();

/// The test signal settings. The framework adds the standard `gen_*` settings (noise, offsets, impairments).
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_UNIT, &GEN_PSI, &GEN_TYPE, &GEN_MOTION];

/// The fields of the test frames: those of the worked example, with the unit, pressure, type and motion given.
pub fn example(unit: u32, psi: u8, message_type: u8, moving: bool) -> EotData {
    EotData {
        battery: 3,
        unit,
        pressure_psi: psi,
        message_type,
        charge: 100,
        valve_open: true,
        moving,
        marker_light: true,
        ..EotData::default()
    }
}

/// The line bits of one frame: the bit sync (ending `...101010`), the frame sync and the body.
pub fn frame_bits(data: &EotData) -> Vec<u8> {
    let mut bits: Vec<u8> = (0..BIT_SYNC_BITS)
        .map(|i| ((BIT_SYNC_BITS - 1 - i) & 1) as u8)
        .collect();
    bits.extend(
        (0..FRAME_SYNC_BITS).map(|i| ((FRAME_SYNC >> (FRAME_SYNC_BITS - 1 - i)) & 1) as u8),
    );
    bits.extend_from_slice(&frame::body(&data.to_bits()));
    bits
}

/// The options of the generator.
#[derive(Clone, Debug)]
pub struct EotGenOptions {
    /// The frames sent in turn.
    pub frames: Vec<EotData>,
    /// The channel relative to the capture centre, Hz.
    pub channel_hz: f64,
    /// The pause between frames, s.
    pub gap_s: f64,
    /// Silence before the first frame, s.
    pub lead_s: f64,
    /// Send the frames again after the last.
    pub repeat: bool,
}

impl EotGenOptions {
    /// The options of the test signal settings of `cfg`, the channel where a radio tuned `tune_offset_hz` above it sees it.
    pub fn from_config(cfg: &GenConfig, tune_offset_hz: f64) -> Self {
        let s = &cfg.settings;
        let unit = s.get(&GEN_UNIT).clamp(0, 131_071) as u32;
        let psi = s.get(&GEN_PSI).clamp(0, 127) as u8;
        let kind = s.get(&GEN_TYPE).clamp(0, 7) as u8;
        EotGenOptions {
            frames: vec![example(unit, psi, kind, s.get(&GEN_MOTION))],
            channel_hz: -tune_offset_hz,
            gap_s: GAP_S,
            lead_s: 0.05,
            repeat: true,
        }
    }
}

/// The end-of-train generator.
pub struct EotGenerator {
    rate: f64,
    opt: EotGenOptions,
    /// The line bits of each frame.
    frames: Vec<Vec<u8>>,
    /// The frame being sent, the bit and the sample within the bit.
    frame: usize,
    bit: usize,
    /// Samples of the bit sent, as a running position in bits.
    pos: f64,
    /// Silence left, samples.
    quiet: usize,
    /// No frame is left.
    done: bool,
    tone_phase: f64,
    carrier_phase: f64,
}

impl EotGenerator {
    /// A generator at `rate` with `opt`.
    pub fn new(rate: f64, opt: EotGenOptions) -> EotGenerator {
        let frames = opt.frames.iter().map(frame_bits).collect();
        let quiet = (opt.lead_s * rate).round() as usize;
        EotGenerator {
            rate,
            frames,
            opt,
            frame: 0,
            bit: 0,
            pos: 0.0,
            quiet,
            done: false,
            tone_phase: 0.0,
            carrier_phase: 0.0,
        }
    }

    /// The generator of the test signal settings of `cfg` for a stack tuned `tune_offset_hz` above the channel.
    pub fn from_config(cfg: &GenConfig, tune_offset_hz: f64) -> EotGenerator {
        EotGenerator::new(cfg.rate_hz, EotGenOptions::from_config(cfg, tune_offset_hz))
    }

    /// The next frame after the one sent; false when none is left.
    fn next_frame(&mut self) {
        self.bit = 0;
        self.pos = 0.0;
        self.quiet = (self.opt.gap_s * self.rate).round() as usize;
        self.frame += 1;
        if self.frame >= self.frames.len() {
            if self.opt.repeat && !self.frames.is_empty() {
                self.frame = 0;
            } else {
                self.done = true;
            }
        }
    }
}

impl Generator for EotGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let per_bit = frame::BIT_RATE / self.rate;
        let mut written = 0;
        while written < out.len() {
            if self.quiet > 0 {
                let n = self.quiet.min(out.len() - written);
                out[written..written + n].fill(Complex32::new(0.0, 0.0));
                self.quiet -= n;
                written += n;
                continue;
            }
            if self.done || self.frames.is_empty() {
                break;
            }
            let bits = &self.frames[self.frame];
            let tone = if bits[self.bit] == 1 {
                frame::ONE_HZ
            } else {
                frame::ZERO_HZ
            };
            let audio = self.tone_phase.cos();
            self.tone_phase = (self.tone_phase + TAU * tone / self.rate) % TAU;
            self.carrier_phase = (self.carrier_phase
                + TAU * (self.opt.channel_hz + DEVIATION_HZ * audio) / self.rate)
                % TAU;
            out[written] = Complex32::new(
                self.carrier_phase.cos() as f32,
                self.carrier_phase.sin() as f32,
            );
            written += 1;
            self.pos += per_bit;
            if self.pos >= 1.0 {
                self.pos -= 1.0;
                self.bit += 1;
                if self.bit >= bits.len() {
                    self.next_frame();
                }
            }
        }
        written
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is the carrier over the noise in 12.5 kHz.
        Some(
            SignalInfo::new(12.5e3)
                .with_symbol_rate(frame::BIT_RATE)
                .with_bit_rate(frame::BIT_RATE),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_has_its_sync_and_body() {
        let b = frame_bits(&example(12345, 90, 0, true));
        assert_eq!(b.len(), 69 + 11 + 63);
        // The last six bits of the bit sync and the frame sync form the search word.
        let w = b[63..80].iter().fold(0u32, |w, &x| (w << 1) | u32::from(x));
        assert_eq!(w, frame::SEARCH_WORD);
    }
}
