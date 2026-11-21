//! Tone signalling tables: CTCSS tones, DCS codes, the DTMF matrix, sequential (selcall) tone sets and aviation SELCAL tones.
//! [`plan`] names the tone sets a detector listens for. The detectors and generators that use them are in `sdrkit-proto-tones`; the tone
//! maths is in `sdrkit-algo-dsp` and `sdrkit-algo-modem` (`sdrkit_algo_modem::tone::ToneDetector`).

pub mod ctcss;
pub mod dcs;
pub mod dtmf;
pub mod plan;
pub mod selcal;
pub mod selcall;
