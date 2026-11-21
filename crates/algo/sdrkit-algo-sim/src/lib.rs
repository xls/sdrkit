//! Signal simulation: noise, offsets, clock error, fading and the other impairments applied to a generated signal.
//!
//! A stack's generator makes the clean signal only. The impairments are applied here from one standard set of settings
//! ([`chain::GenSetting`]), so they do not depend on the generator or the decoder.
//!
//! | Module | Contents |
//! |---|---|
//! | [`rng`] | the one seeded generator (xoshiro256** seeded by SplitMix64), with named independent streams |
//! | [`gauss`] | Gaussian values by the ziggurat method; the normal distribution function and its inverse |
//! | [`noise`] | AWGN set by SNR in a bandwidth, C/N0, Es/N0, Eb/N0 or an absolute floor, and the conversions between them |
//! | [`offset`] | carrier offset with linear drift |
//! | [`clock`] | sample-clock error by fractional resampling |
//! | [`phase_noise`] | random-walk (Lorentzian) phase noise with a white floor, set in dBc/Hz or by linewidth |
//! | [`front`] | gain, spectral inversion, hum, DC offset, IQ imbalance, impulse noise, the ADC quantiser |
//! | [`fading`] | tapped delay lines with Jakes, Gaussian and Rice Doppler spectra; DRM channels 1 to 12, ITU-R F.520 and F.1487 HF, GSM (COST 207), Rayleigh, Rician, echo |
//! | [`chain`] | the standard generator settings, a [`chain::Chain`] built from them, and [`chain::Impaired`], any generator through it |
//! | [`modulate`] | clean modulators: linear (PSK, QAM, APSK, ASK, OQPSK with any pulse), CPM (FSK, CPFSK, MSK, GMSK, GFSK), AFSK, OOK keying, Morse, linear sweeps, AM, FM, SSB |
//! | [`points`] | Gray-labelled PSK, square QAM, PAM and APSK point sets of unit mean energy |
//! | [`scene`] | timed bursts and streams from several sources at their own offsets and levels, summed |
//! | [`voice`] | speech-like audio: noise of the voice band in syllables with pauses |
//!
//! Samples are [`Complex32`] (real audio is `f32`). Full scale is `[-1, 1]` on each of I and Q; a mean power of 1 is 0 dBFS.
//! Every random part draws from its own [`rng::Rng::stream`] of the one seed, so the same seed produces the same signal on every
//! platform, and adding a part changes nothing another part draws. A streaming type keeps its state between calls, so a signal
//! cut into pieces of any size produces the same output as one piece. Types that change the number of samples append to a
//! caller's `Vec`. Constructors check their parameters and return [`SimError`].
//!
//! ```
//! use sdrkit_algo_sim::chain::{Chain, ChainConfig, SignalInfo};
//! use sdrkit_algo_sim::noise::NoiseLevel;
//! use sdrkit_core::Complex32;
//!
//! // A unit-power signal at 250 kHz, 10 kHz wide: 12 dB SNR, 1.5 kHz off, 20 ppm clock error.
//! let sig = SignalInfo::new(250e3).with_bandwidth(10e3);
//! let cfg = ChainConfig {
//!     noise: NoiseLevel::SnrDb { snr_db: 12.0, bandwidth_hz: 10e3 },
//!     offset_hz: 1500.0,
//!     clock_ppm: 20.0,
//!     ..ChainConfig::default()
//! };
//! let mut chain = Chain::new(&cfg, &sig).unwrap();
//! let clean = vec![Complex32::new(1.0, 0.0); 1000];
//! let mut out = Vec::with_capacity(2000);
//! chain.process(&clean, &mut out);
//! assert!(out.len() >= 1000);
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

mod bench;
pub mod chain;
pub mod clock;
mod error;
pub mod fading;
pub mod front;
pub mod gauss;
pub mod modulate;
pub mod noise;
pub mod offset;
pub mod phase_noise;
pub mod points;
pub mod rng;
pub mod scene;
pub mod voice;

pub use bench::benchmarks;
pub use error::{EchoFault, Param, SimError};
pub use sdrkit_core::Complex32;
