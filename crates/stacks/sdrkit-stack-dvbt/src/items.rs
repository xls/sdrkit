//! The typed items that flow between the DVB-T components, and the control link between them.

use crate::bandwidth::WidthSource;
use crate::params::{Bandwidth, TpsParams};
use sdrkit_core::Complex32;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

/// The control link between the components of one receiver: lock-free flags and values a later component sets and an earlier one reads
/// at its next pass (the receiver runs the graph in passes of a fixed length, so the effect does not depend on how the input is cut).
/// A component after a stage boundary writes nothing here that a component before the boundary reads, apart from the
/// telemetry: the equaliser and the error correction may run on other threads than the components before them.
#[derive(Debug, Default)]
pub struct Link {
    /// TPS is locked (the locator stops, window placement under echoes can run).
    pub tps_locked: AtomicBool,
    /// Requests for mode detection to start again (a counter).
    pub restart: AtomicU32,
    /// Sample clock correction to apply, ppm (f64 bits), accumulated by the timing loop and taken by the input.
    pub clock_delta: AtomicU64,
    /// Total clock correction applied, ppm (f64 bits).
    pub clock_ppm: AtomicU64,
    /// The first sample that the input stage gave with the last correction. `u64::MAX` while a correction waits.
    pub clock_from: AtomicU64,
    /// The locator's mixer offset in Hz (f64 bits).
    pub locator_hz: AtomicU64,
    /// Receiver state: 0 searching, 1 tracking, 2 locked, 3 streaming.
    pub state: AtomicU8,
    /// Detection level 0 to 3.
    pub detect: AtomicU8,
    /// A candidate has been seen while searching.
    pub candidate: AtomicBool,
    /// Packet alignment holds.
    pub fec_sync: AtomicBool,
    /// The receiver reads DVB-SH (ETSI EN 302 583): the 1K and 4K modes are searched and TPS is read with the DVB-SH fields.
    pub sh: AtomicBool,
    /// Measurements the components publish for the receiver's results (written a few times per pass, never contended).
    pub telemetry: std::sync::Mutex<Telemetry>,
}

/// The channel width facts of the input stage.
#[derive(Clone, Debug, Default)]
pub struct WidthReport {
    /// The width in use.
    pub bandwidth: Option<Bandwidth>,
    /// Where it came from.
    pub source: Option<WidthSource>,
    /// The `bandwidth` setting is Auto.
    pub auto: bool,
    /// The measured occupied width, Hz.
    pub measured_hz: Option<f64>,
    /// The level of the channel above the floor outside it, dB.
    pub occupancy_db: Option<f64>,
    /// The input rate, Hz.
    pub input_rate: f64,
    /// The natural rate of the width in use, Hz.
    pub natural_rate: f64,
}

/// What the components measure, collected for the stack's results.
#[derive(Clone, Debug, Default)]
pub struct Telemetry {
    /// The channel width facts.
    pub width: WidthReport,
    /// Input level, dBFS.
    pub level_dbfs: Option<f32>,
    /// Non-finite samples replaced.
    pub scrubbed: u64,
    /// The fractional carrier offset, subcarrier spacings.
    pub frac: f64,
    /// Timing error of the last symbol (samples) and its correlation coefficient.
    pub timing: (f64, f64),
    /// The drift of the sample clock that the resampler does not correct, ppm.
    pub drift_ppm: f64,
    /// Resynchronisations.
    pub resyncs: u64,
    /// The integer carrier offset, once found.
    pub int_offset: Option<i32>,
    /// The spectrum was mirrored.
    pub mirrored: bool,
    /// FFT size and guard samples of the acquisition.
    pub fft: Option<(usize, usize)>,
    /// TPS parameters, frame and symbol, once locked.
    pub tps: Option<(TpsParams, u8, usize)>,
    /// The 16-bit cell id was assembled.
    pub cell_known: bool,
    /// TPS frames decoded and failed.
    pub tps_frames: (u64, u64),
    /// Signal time of the last good TPS frame.
    pub tps_good_time: f64,
    /// What TPS said is unsupported.
    pub unsupported: Option<&'static str>,
    /// Equaliser measurements.
    pub eq: crate::equaliser::EqStats,
    /// Error correction report.
    pub fec: crate::fecstage::FecReport,
    /// Signal time of the first good packet.
    pub first_packet: Option<f64>,
    /// Signal time of the newest sample processed.
    pub now: f64,
}

fn f64_load(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Ordering::Relaxed))
}

impl Link {
    /// A new link.
    pub fn new() -> Arc<Link> {
        Arc::new(Link::default())
    }
    /// Adds a clock correction to hand over.
    pub fn add_clock(&self, ppm: f64) {
        self.clock_from.store(u64::MAX, Ordering::Relaxed);
        let v = f64_load(&self.clock_delta) + ppm;
        self.clock_delta.store(v.to_bits(), Ordering::Relaxed);
    }
    /// Takes the clock correction waiting.
    pub fn take_clock(&self) -> f64 {
        f64::from_bits(self.clock_delta.swap(0f64.to_bits(), Ordering::Relaxed))
    }
    /// Says that the samples from `sample` of the output of the input stage have every correction taken so far.
    pub fn set_clock_from(&self, sample: u64) {
        self.clock_from.store(sample, Ordering::Relaxed);
    }
    /// True if the sample `sample` of the output of the input stage has every correction that was handed over.
    pub fn clock_applied_at(&self, sample: u64) -> bool {
        sample >= self.clock_from.load(Ordering::Relaxed)
    }
    /// The total clock correction, ppm.
    pub fn clock_ppm(&self) -> f64 {
        f64_load(&self.clock_ppm)
    }
    /// Sets the total clock correction.
    pub fn set_clock_ppm(&self, v: f64) {
        self.clock_ppm.store(v.to_bits(), Ordering::Relaxed);
    }
    /// The locator offset, Hz.
    pub fn locator_hz(&self) -> f64 {
        f64_load(&self.locator_hz)
    }
    /// Sets the locator offset.
    pub fn set_locator_hz(&self, v: f64) {
        self.locator_hz.store(v.to_bits(), Ordering::Relaxed);
    }
    /// Requests a new mode detection.
    pub fn request_restart(&self) {
        self.restart.fetch_add(1, Ordering::Relaxed);
        self.tps_locked.store(false, Ordering::Relaxed);
        self.state.store(0, Ordering::Relaxed);
        self.detect.store(0, Ordering::Relaxed);
    }
    /// Clears everything (a retune).
    pub fn reset(&self) {
        self.tps_locked.store(false, Ordering::Relaxed);
        self.clock_delta.store(0f64.to_bits(), Ordering::Relaxed);
        self.clock_ppm.store(0f64.to_bits(), Ordering::Relaxed);
        self.clock_from.store(0, Ordering::Relaxed);
        self.locator_hz.store(0f64.to_bits(), Ordering::Relaxed);
        self.state.store(0, Ordering::Relaxed);
        self.detect.store(0, Ordering::Relaxed);
        self.candidate.store(false, Ordering::Relaxed);
        self.fec_sync.store(false, Ordering::Relaxed);
    }
    /// Relaxed load helpers.
    pub fn locked(&self) -> bool {
        self.tps_locked.load(Ordering::Relaxed)
    }
}

/// The timing the finder committed to (the value of a typed tag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// FFT size.
    pub fft: usize,
    /// Guard samples.
    pub guard: usize,
    /// Absolute index of a symbol start (the first sample of a guard interval).
    pub start: u64,
    /// Fractional carrier offset in subcarrier spacings.
    pub frac: f64,
}

impl sdrkit_graph::Item for Timing {
    const KIND: &'static str = "dvbt.timing";
}

/// A request to start mode detection again from the sample it marks (the value of a typed tag).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Restart;

impl sdrkit_graph::Item for Restart {
    const KIND: &'static str = "dvbt.restart";
}

/// One OFDM symbol's FFT window, frequency-corrected (the item `dvbt.symbol`, from `dvbt.sync`).
#[derive(Clone, Debug)]
pub struct SymbolBlock {
    /// N samples.
    pub samples: Arc<[Complex32]>,
    /// FFT size and guard.
    pub fft: usize,
    /// Guard samples.
    pub guard: usize,
    /// Symbols since acquisition (lost symbols not counted).
    pub count: u64,
    /// Absolute index of the window start.
    pub window: u64,
    /// The window moved by this many samples relative to one symbol period after the previous one.
    pub moved: i64,
    /// The symbol's correlation collapsed: no information.
    pub collapsed: bool,
    /// The first symbol after a resynchronisation (a pilot check counts lost symbols).
    pub resynced: bool,
    /// A new acquisition: everything downstream starts fresh.
    pub fresh: bool,
    /// Change of the integer carrier offset (a wrap of the fractional loop), in carriers.
    pub int_step: i32,
    /// The fractional offset in use, in subcarrier spacings.
    pub frac: f64,
    /// The correlation coefficient.
    pub rho: f32,
    /// Signal time of the window start.
    pub time: f64,
    /// The channel estimate must be rebuilt (a large window move).
    pub rebuild: bool,
    /// The sample rate of the symbol, Hz.
    pub rate: f64,
}

impl sdrkit_graph::Item for SymbolBlock {
    const KIND: &'static str = "dvbt.symbol";
}

/// K carriers of one symbol (the item `dvbt.carriers`, from `dvbt.fft`), and after `dvbt.tps`, with its place in the frame.
#[derive(Clone, Debug)]
pub struct Carriers {
    /// K cells, carrier 0 first.
    pub cells: Arc<[Complex32]>,
    /// FFT size.
    pub fft: usize,
    /// Guard samples.
    pub guard: usize,
    /// Symbols since acquisition.
    pub count: u64,
    /// Window move (samples) since the previous symbol.
    pub moved: i64,
    /// No information (collapsed).
    pub collapsed: bool,
    /// The first symbol after a resynchronisation.
    pub resynced: bool,
    /// A new acquisition.
    pub fresh: bool,
    /// The integer carrier offset in use.
    pub int_offset: i32,
    /// The spectrum is mirrored (and was turned round).
    pub mirrored: bool,
    /// The fractional offset in subcarrier spacings.
    pub frac: f64,
    /// Signal time.
    pub time: f64,
    /// Rebuild the channel estimate.
    pub rebuild: bool,
    /// The sample rate of the symbol, Hz.
    pub rate: f64,
    /// Index in the frame (0 to 67), once TPS is locked.
    pub l: usize,
    /// Frame number (0 to 3), once TPS is locked.
    pub frame: u8,
    /// The parameters in force, once TPS is locked.
    pub params: Option<TpsParams>,
    /// An erased symbol (lost or without information), inserted to keep the stream aligned.
    pub erased: bool,
    /// The TPS differential point of this symbol (for the plot).
    pub tps_point: Complex32,
    /// In the DVB-SH reading, on the first symbol after a frame whose TPS decoded: that frame's bits s0 to s67 (bit `i` of the
    /// value is s_i).
    pub tps_word: Option<u128>,
}

impl sdrkit_graph::Item for Carriers {
    const KIND: &'static str = "dvbt.carriers";
}

/// Equalised data cells of one symbol with their noise (the item `dvbt.cells`, from `dvbt.equaliser`).
#[derive(Clone, Debug)]
pub struct DataCells {
    /// Nd cells in carrier order.
    pub cells: Arc<[Complex32]>,
    /// Noise variance per real dimension of each cell.
    pub noise: Arc<[f32]>,
    /// Index in the frame.
    pub l: usize,
    /// Frame number (0 to 3).
    pub frame: u8,
    /// The TPS word that came with the symbol ([`Carriers::tps_word`]).
    pub tps_word: Option<u128>,
    /// The parameters.
    pub params: TpsParams,
    /// An erased symbol.
    pub erased: bool,
    /// Start the error correction afresh (a new acquisition or new parameters).
    pub fresh: bool,
    /// Signal time.
    pub time: f64,
    /// The sample rate of the symbol, Hz.
    pub rate: f64,
}

impl sdrkit_graph::Item for DataCells {
    const KIND: &'static str = "dvbt.cells";
}

/// Decoded bits from the Viterbi decoder (the item `dvbt.bits`, from `dvbt.inner`): one bit per byte, MSB first in the stream.
#[derive(Clone, Debug)]
pub struct DecodedBits {
    /// The bits.
    pub bits: Arc<[u8]>,
    /// The stream starts afresh here.
    pub fresh: bool,
    /// Signal time of the last symbol in them.
    pub time: f64,
    /// Signal seconds the bits stand for.
    pub duration: f64,
}

impl sdrkit_graph::Item for DecodedBits {
    const KIND: &'static str = "dvbt.bits";
}

/// The TPS parameters as they change (the item `dvbt.tps`).
#[derive(Clone, Debug, PartialEq)]
pub struct TpsMessage {
    /// The parameters (cell id as assembled).
    pub params: TpsParams,
    /// Frame number.
    pub frame: u8,
    /// Symbol index in the frame.
    pub symbol: usize,
    /// The 16-bit cell id is known (two frames agreed).
    pub cell_id_known: bool,
}

impl sdrkit_graph::Item for TpsMessage {
    const KIND: &'static str = "dvbt.tps";
}
