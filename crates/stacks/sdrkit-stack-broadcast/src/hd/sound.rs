//! HD Radio sound: the HDC packets of the program go to a host decoder. The decoded packets of 2048 samples at 44.1 kHz fill
//! slots that advance one packet per 2048 / 44 100 s of signal time. A slot with nothing plays silence, and a decode error
//! resets the decoder. The sound is resampled from 44.1 kHz to 48 kHz.

use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::Frame;
use sdrkit_core::Complex32;

/// Samples per HDC packet.
pub const PACKET_SAMPLES: usize = 2048;
/// The HDC sample rate.
pub const HDC_RATE: f64 = 44_100.0;

/// A packet the decoder did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeError;

/// A decoder the host supplies.
pub trait HdcDecoder: Send {
    /// Decodes one packet into 2048 stereo frames; `Err` on a decode error.
    fn decode(&mut self, packet: &[u8], out: &mut Vec<Frame<2>>) -> Result<(), DecodeError>;
    /// Starts again.
    fn reset(&mut self);
}

/// Counters per program.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct SoundStats {
    /// Packets decoded.
    pub decoded: u64,
    /// Decode errors.
    pub errors: u64,
    /// Lost slots (played as silence).
    pub lost: u64,
    /// Decoder restarts.
    pub restarts: u64,
    /// Frames out at 48 kHz.
    pub frames_out: u64,
    /// A packet was decoded in the last second of signal.
    pub playing: bool,
}

/// The sound slots of the program being heard.
pub struct SoundSlots {
    dec: Option<Box<dyn HdcDecoder>>,
    rs: Resampler<Complex32>,
    pcm: Vec<Frame<2>>,
    tmp_in: Vec<Complex32>,
    tmp_out: Vec<Complex32>,
    /// Counters.
    pub stats: SoundStats,
    error_run: u32,
    last_decoded_slot: Option<u64>,
    slot: u64,
    half: Option<Vec<u8>>,
    logged: bool,
    /// Lines for the log.
    pub log: Vec<String>,
}

impl SoundSlots {
    /// Slots with a decoder (or none: packets are counted, no sound frames are made).
    pub fn new(dec: Option<Box<dyn HdcDecoder>>) -> SoundSlots {
        SoundSlots {
            dec,
            rs: Resampler::new(HDC_RATE, 48_000.0).expect("rates"),
            pcm: Vec::new(),
            tmp_in: Vec::new(),
            tmp_out: Vec::new(),
            stats: SoundStats::default(),
            error_run: 0,
            last_decoded_slot: None,
            slot: 0,
            half: None,
            logged: false,
            log: Vec::new(),
        }
    }
    /// Clears the counters and the state (a reset).
    pub fn reset(&mut self) {
        self.stats = SoundStats::default();
        self.slot = 0;
        self.half = None;
        self.error_run = 0;
        self.last_decoded_slot = None;
        self.rs.reset();
        if let Some(d) = &mut self.dec {
            d.reset();
        }
    }
    fn emit(&mut self, frames: &[Frame<2>], out: &mut Vec<f32>) {
        self.tmp_in.clear();
        self.tmp_in
            .extend(frames.iter().map(|s| Complex32::new(s.0[0], s.0[1])));
        self.tmp_out.clear();
        self.rs.process(&self.tmp_in, &mut self.tmp_out);
        for s in &self.tmp_out {
            out.push(s.re);
            out.push(s.im);
        }
        self.stats.frames_out += self.tmp_out.len() as u64;
    }
    /// Takes the next slot: a packet (`Some(bytes)`; `None` when lost), or a half packet that continues in the next slot
    /// (`continues`). Appends interleaved stereo at 48 kHz to `out`.
    pub fn slot(&mut self, packet: Option<&[u8]>, continues: bool, out: &mut Vec<f32>) {
        self.slot += 1;
        let Some(dec) = self.dec.as_mut() else {
            if !self.logged {
                self.logged = true;
                self.log.push("HDC sound codec not installed".into());
            }
            return;
        };
        let silence = vec![Frame([0.0; 2]); PACKET_SAMPLES];
        let bytes: Option<Vec<u8>> = match (packet, continues) {
            (Some(p), true) => {
                self.half = Some(p.to_vec());
                None
            }
            (Some(p), false) => match self.half.take() {
                Some(mut h) => {
                    h.extend_from_slice(p);
                    Some(h)
                }
                None => Some(p.to_vec()),
            },
            (None, _) => {
                self.half = None;
                None
            }
        };
        if packet.is_some() && continues {
            // A half packet plays nothing until it is joined.
            return;
        }
        match bytes {
            Some(b) => {
                self.pcm.clear();
                let mut pcm = std::mem::take(&mut self.pcm);
                match dec.decode(&b, &mut pcm) {
                    Ok(()) => {
                        self.stats.decoded += 1;
                        self.error_run = 0;
                        self.last_decoded_slot = Some(self.slot);
                        pcm.resize(PACKET_SAMPLES, Frame([0.0; 2]));
                        self.emit(&pcm, out);
                    }
                    Err(DecodeError) => {
                        self.stats.errors += 1;
                        self.error_run += 1;
                        if self.error_run == 1 {
                            dec.reset();
                            self.stats.restarts += 1;
                        }
                        self.emit(&silence, out);
                    }
                }
                self.pcm = pcm;
            }
            None => {
                self.stats.lost += 1;
                self.emit(&silence, out);
            }
        }
        // Playing when a packet was decoded in the last second of signal (21.5 slots).
        self.stats.playing = self
            .last_decoded_slot
            .is_some_and(|s| (self.slot - s) as f64 * PACKET_SAMPLES as f64 / HDC_RATE <= 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in decoder: a packet's first byte sets a DC level; a first byte of 0xEE is a decode error.
    struct Standin;
    impl HdcDecoder for Standin {
        fn decode(&mut self, p: &[u8], out: &mut Vec<Frame<2>>) -> Result<(), DecodeError> {
            if p[0] == 0xEE {
                return Err(DecodeError);
            }
            let v = p[0] as f32 / 255.0;
            out.extend(std::iter::repeat_n(Frame([v, v]), PACKET_SAMPLES));
            Ok(())
        }
        fn reset(&mut self) {}
    }

    #[test]
    fn without_a_decoder_nothing_plays_and_the_log_says_so_once() {
        let mut s = SoundSlots::new(None);
        let mut out = Vec::new();
        for _ in 0..5 {
            s.slot(Some(&[1, 2, 3]), false, &mut out);
        }
        assert!(out.is_empty());
        assert_eq!(s.log, vec!["HDC sound codec not installed".to_string()]);
    }

    #[test]
    fn eight_packets_give_the_right_length_and_levels() {
        let mut s = SoundSlots::new(Some(Box::new(Standin)));
        let mut out = Vec::new();
        for i in 0..8u8 {
            s.slot(Some(&[100 + i, 0]), false, &mut out);
        }
        let want = 8.0 * 2048.0 * 48_000.0 / 44_100.0;
        assert!(
            ((out.len() / 2) as f64 - want).abs() <= 400.0,
            "{}",
            out.len() / 2
        );
        assert!(s.stats.playing);
        assert_eq!(s.stats.decoded, 8);
        // the level of a packet in the middle
        let mid = out.len() / 2;
        assert!((out[mid] - 104.0 / 255.0).abs() < 0.02, "{}", out[mid]);
    }

    #[test]
    fn gaps_errors_and_halves() {
        let mut s = SoundSlots::new(Some(Box::new(Standin)));
        let mut out = Vec::new();
        s.slot(Some(&[50]), false, &mut out);
        s.slot(None, false, &mut out);
        s.slot(None, false, &mut out);
        assert_eq!(s.stats.lost, 2);
        for _ in 0..3 {
            s.slot(Some(&[0xEE]), false, &mut out);
        }
        assert_eq!(s.stats.errors, 3);
        assert_eq!(s.stats.restarts, 1);
        // A joined half packet plays; a half whose other half is lost does not.
        let before = s.stats.decoded;
        s.slot(Some(&[60]), true, &mut out);
        s.slot(Some(&[1]), false, &mut out);
        assert_eq!(s.stats.decoded, before + 1);
        s.slot(Some(&[70]), true, &mut out);
        s.slot(None, false, &mut out);
        assert_eq!(s.stats.decoded, before + 1);
        s.reset();
        assert_eq!(s.stats, SoundStats::default());
    }
}
