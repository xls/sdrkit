//! Voice codec 4, a speech codec: code-excited linear prediction on an 8 kHz core with a parametric high band at 16 kHz.
//!
//! ```
//! use sdrkit_codec_custom::voice4::*;
//! let cfg = Config { rate: SampleRate::Hz16000, frame: FrameLength::Ms40, profile: Profile::P6_0, type_in_band: true };
//! let mut enc = Encoder::new(cfg, EncoderSettings::default()).unwrap();
//! let mut dec = Decoder::new(cfg, DecoderSettings::default()).unwrap();
//! let pcm = vec![0i16; enc.frame_samples()];
//! let mut frame = [0u8; 80];
//! let coded = enc.encode(&pcm, &mut frame).unwrap();
//! assert_eq!(coded, Encoded::Speech(30));
//! let mut out = vec![0i16; dec.frame_samples()];
//! dec.decode(Input::Frame(&frame[..coded.len()]), Damage::default(), &mut out).unwrap();
//! ```

mod core;
mod decoder;
mod encoder;
mod enhancer;
mod frame;
mod highband;
mod layout;
mod lpc;
mod math;
mod pvq;
mod tables;

pub use decoder::{Decoded, Decoder, DecoderSettings, Input};
pub use encoder::{Encoded, Encoder, EncoderSettings};
pub use enhancer::EnhancerMode;
pub use frame::Damage;
pub use layout::{Config, FrameLength, Layout, Profile, SampleRate};
