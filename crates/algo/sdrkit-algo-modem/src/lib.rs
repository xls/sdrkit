//! Demodulation and synchronisation: the plain types that turn samples into symbols, decisions and soft bits.
//!
//! The crate uses `sdrkit-algo-dsp` (filters, interpolators, FFTs, constellations), `sdrkit-util-bits` (soft-bit types, sync
//! words) and `sdrkit-algo-fec` (the trellis decoder used by MLSE). Modulators are in `sdrkit-algo-sim`.
//!
//! # Modules
//!
//! | Module | What it holds |
//! |---|---|
//! | [`analog`] | [`analog::AnalogDemod`]: NFM, WFM, AM, USB, LSB and CW to real audio, with the channel, sideband and audio filters, the decimation, the AGC of `sdrkit_algo_dsp`, DC blocking, de-emphasis and a clamp |
//! | [`constellation`] | [`Constellation`]: hard decisions and exact or max-log soft demapping over any `sdrkit_algo_dsp::constellation::PointSet` (per-axis for square QAM, PAM and VSB; sectors for PSK; generic for APSK and cross QAM) |
//! | [`graydemap`] | soft demapping of reflected-Gray square QAM in a few operations per bit at any size |
//! | [`differential`] | DBPSK, DQPSK, pi/4-DQPSK (DAB and TETRA maps), D8PSK, and the DVB-C differential quadrant |
//! | [`timing`] | [`timing::SymbolSync`] generic over detector (Gardner, zero crossing, early-late, Mueller and Muller) and interpolator, OQPSK with a lock test that needs no carrier, a carrier phase taken off before the detector, a half-symbol move, T/2 outputs, clock range, lock; the polyphase matched interpolator; Oerder-Meyr and max-energy burst estimators; [`timing::EdgeClock`], the bit clock of hard-limited levels |
//! | [`carrier`] | [`carrier::CarrierSync`] generic over phase detector (Costas 2/4/8, decision-directed on any constellation, ML tanh, known pilots), frequency-loop assist, gears; [`carrier::ToneLoop`] for pilots and carriers, with gears |
//! | [`lock`] | the shared lock detector with hysteresis, and loop gears |
//! | [`freq`] | M-th power FFT line and MSK line pairs; Kay, Fitz, Luise-Reggiannini, lag and multi-lag estimators; centroid; the raised-cosine template fit and the band-power search (coarse carrier searches over a spectrum) |
//! | [`fsk`] | M-FSK by correlator, FFT bins or discriminator; exact FSK LLRs, blind-tracked for streams; the discriminator front end; the 2/4-level slicer (C4FM); the 2-FSK tone-energy decision at a moving centre; the slicer threshold from smoothed extremes; named bit labels |
//! | [`follow`] | carrier follow-up for a narrow channel filter: spectral (closed or open loop), a narrow filter centred in open loop, discriminator mean |
//! | [`fskauto`] | a 2-FSK front from a list of candidate bauds: tone pair, baud, clock phase and refined tones |
//! | [`afsk`] | hard bits of an AFSK signal for every sampling phase, for a frame search per phase |
//! | [`fmmeasure`] | measures of an FSK signal on an FM channel: symbol rate, number of levels, occupied band |
//! | [`toneslots`] | the front of a one-tone-at-a-time MFSK signal: base tone, slot length and phase, the tone of each slot |
//! | [`msklines`] | the bit rate and carrier of MSK from the two lines of its square, over a report of averaged spectra |
//! | [`toneset`] | a known tone set found anywhere in a channel by a comb fit on a long averaged spectrum, the lock with a replay of the stored channel, the symbol quality gate and the chooser among parallel readings |
//! | [`fsksync`] | 2-FSK sync search over discriminator values: a bit pattern at fractional symbol steps, its own threshold, either polarity, the bits after it |
//! | [`fskscan`] | candidates of a start-stop 2-FSK signal for trial decoding: tone pairs from a list of shifts, the speed from the run lengths of one tone |
//! | [`gmsk`], [`msk`] | GFSK/GMSK by discriminator and by MLSE (Viterbi through `sdrkit-algo-fec`, max-log BCJR, burst and streaming), LS channel fit; MSK |
//! | [`msktiming`] | the symbol-clock timing of an MSK station whose clock comes from a frequency standard, with its error |
//! | [`pm`] | phase modulation with a residual carrier; coherent detection of a split-phase burst |
//! | [`slowmorse`] | very slow Morse (QRSS, DFCW) from runs of a key state: the element length fitted with the bias of a half-element window, the runs as dots, dashes and gaps, the letters' codes |
//! | [`keysurvey`] | keyed carriers anywhere in a wide block from successive power spectra: on and off states per bin, the width test, the rhythm of dots, dashes and gaps, the speed; no decoding |
//! | [`occupancy`] | occupied channels of any modulation in an averaged power spectrum: runs of bins over the local noise, with width and level; a carrier alone is none |
//! | [`key`] | Morse keying: the key detector whose levels and timing scale with the dot length, and the key state (decisions to mark and space lengths) |
//! | [`ook`] | OOK and ASK slicers, pulse-position decisions over exact fractional windows |
//! | [`envtrack`] | envelope slicer with peak and floor trackers and a gate: load modulation, reader ASK, keyed carriers |
//! | [`chirp`] | LoRa-style dechirp with soft bins, Candan fraction and timing tracking |
//! | [`sweeps`] | preambles of linear sweeps: synthesis with raised-cosine edges, and the start and carrier offset of a group of rising and falling sweeps |
//! | [`guardscan`] | the structure of an OFDM signal with a cyclic prefix from the signal alone: useful length, symbol rate |
//! | [`ofdm`] | layouts (802.11a, DVB-T 2K and 8K), CP and preamble correlators (also with the second copy shifted in frequency), integer CFO, the DAB frame pieces (null symbol, impulse response, window placement, delay-domain cleaning), FFT framing, pilot channel estimators and the windowed-sinc grid interpolator, ZF with CSI, phase and clock tracking; the streaming CP synchroniser with its clock loop, and the receiver |
//! | [`psk`] | generic BPSK, QPSK, offset QPSK and 8PSK: point maps and Gray labels, the decimating front end with its band locator, template and power-line carrier searches, the streaming demodulator (matched filter, timing, Costas, decision-directed or ML carrier loop, hand-over), and the blind analyser of rate, order, shape and carrier |
//! | [`pskorder`] | the order of a PSK from the moments of its differential phase steps |
//! | [`dsss`] | chip despreading (Barker-11, any code), FFT code acquisition, E/P/L code tracking |
//! | [`spreading`] | Walsh and OVSF codes, the WCDMA scrambling and synchronisation codes, the cdma2000 short pilot PN sequences |
//! | [`rake`] | RAKE combining of a few multipath components found on a known sequence |
//! | [`equalizer`] | LMS, NLMS, CMA and decision-directed adaptive equalisers, T/2 and DFE, input normalisation and a divergence reset; least-squares channel from training; long 8-VSB echo cancellers fitted on the field sync |
//! | [`mmse_dfe`] | an MMSE decision-feedback equaliser designed from a known symbol-spaced channel |
//! | [`correlate`] | FFT sample-domain sync correlation with frequency hypotheses; soft-symbol correlator; the autocorrelation of a capture at every lag |
//! | [`template_bank`] | FFT correlation of one stream with several templates at once (overlap-save) |
//! | [`burst`] | burst detection over a noise floor with pooled buffers |
//! | [`bandburst`] | bursts anywhere in a band: short FFTs with a floor per bin, one burst over the band with its signals, or a burst per run of lit bins |
//! | [`burstmix`] | the channel of one burst: mixed to 0 Hz, filtered and decimated |
//! | [`stretchburst`] | bursts of one signal of known width anywhere in a band: the strongest stretch of bins over the noise of the same frame |
//! | [`chanburst`] | bursts on a grid of equal channels at once: a floor per channel, leakage and wideband signals told apart |
//! | [`fm0`] | FM0 (bi-phase space) bits from the times between level changes |
//! | [`halfbit`] | Manchester half-bits and biphase mark bits from the times between level changes, with a clock that follows them |
//! | [`levelsync`] | sync search over symbol levels: the best of several level patterns by normalised correlation |
//! | [`level`], [`squelch`] | SNR (M2M4, data-aided, decision-directed, pilots, spectrum), EVM and MER; power, SNR and FM noise-quieting squelch |
//! | [`tone`] | tone presence over Goertzel blocks, with start and stop times (CTCSS, DTMF, selcall, SELCAL tone sets) |
//! | [`tonegrid`] | tone energies of a slow MFSK signal on a grid of time and fractions of the tone spacing, and the fold of a message that repeats |
//! | [`tsma`] | telegram splitting: radio bursts found by their pilot, and the pattern, start and frequency that explain them |
//! | [`theory`] | closed-form error rates and the frequency Cramer-Rao bound, for tests and link budgets |
//! | [`zadoff_chu`] | Zadoff-Chu sequences with the phase index computed exactly in integers |
//!
//! [`benchmarks`] times the hot paths (for the C API's `sdrkit_benchmark`, run on the device).
//!
//! Soft bits are true LLRs `L = ln P(b=0) / P(b=1)`: a positive value means 0. Every demapper takes the noise variance (or the
//! weight `|H|^2 / sigma^2` of an equalised cell), so the caller applies no scale of its own. `sdrkit_algo_fec::llr::Quantizer`
//! turns them into `i8`. The noise variance is the total complex variance `E|n|^2` (`N0` for unit-energy symbols), not the
//! per-axis variance.
//!
//! Loops take a noise bandwidth normalised to their update rate and a damping factor; their gains come from
//! `sdrkit_algo_dsp::loopfilter::LoopFilter::new(bn_t, zeta)`. Long-running phases are `f64` cycles. Streaming types keep their
//! state, so the output does not depend on how the input is split. Burst forms read a stored slice at fractional positions through
//! `sdrkit_algo_dsp::interp::Reader`. Bad parameters, short buffers and non-finite input return a [`ModemError`].

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod afsk;
pub mod analog;
pub mod bandburst;
mod bench;
pub mod burst;
pub mod burstmix;
pub mod carrier;
pub mod chanburst;
pub mod charframe;
pub mod chirp;
pub mod constellation;
pub mod correlate;
pub mod differential;
pub mod dsss;
pub mod envtrack;
pub mod equalizer;
mod error;
pub mod fm0;
pub mod fmmeasure;
pub mod follow;
pub mod freq;
pub mod fsk;
pub mod fskauto;
pub mod fskscan;
pub mod fsksync;
pub mod gmsk;
pub mod graydemap;
pub mod guardscan;
pub mod halfbit;
pub mod key;
pub mod keysurvey;
pub mod level;
pub mod levelsync;
pub mod lock;
pub mod mmse_dfe;
pub mod msk;
pub mod msklines;
pub mod msktiming;
pub mod occupancy;
pub mod ofdm;
pub mod ook;
pub mod pm;
pub mod psk;
pub mod pskorder;
pub mod rake;
pub mod slowmorse;
pub mod spreading;
pub mod squelch;
pub mod stretchburst;
pub mod sweeps;
pub mod template_bank;
pub mod theory;
pub mod timing;
pub mod tonebank;
pub mod tone;
pub mod tonegrid;
pub mod toneslots;
pub mod toneset;
pub mod tsma;
pub mod zadoff_chu;

pub use bench::benchmarks;
pub use constellation::{Constellation, Demap};
pub use error::{ModemError, Param};
pub use sdrkit_algo_dsp::constellation::PointSet;
pub use sdrkit_core::Complex32;
