//! The `atv` receiver as components, the typed items between them, and its graphs. The graph has a clock of 10 ms: the supervisor
//! decides at a tick, and its decision acts from the first sample after it, however the input is cut.

pub mod a2;
pub mod colour;
pub mod fmvideo;
pub mod front;
pub mod graph;
pub mod nbtv;
pub mod nicam_rx;
pub mod picture;
pub mod search;
pub mod selector;
pub mod snow;
pub mod sound;
pub mod status;
pub mod supervisor;
pub mod sync;
pub mod teletext;
pub mod vision;

use crate::systems::{Colour, SystemId};
use sdrkit_core::Complex32;
use sdrkit_graph::Item;

/// The graph's clock, seconds.
pub const TICK_S: f64 = 0.01;
/// Reports every this many ticks (0.25 s).
pub const REPORT_TICKS: u32 = 25;

sdrkit_core::choice_enum! {
    /// The modulation the receiver is built for.
    #[derive(Default)]
    pub enum Modulation {
        /// AM broadcast (vestigial sideband).
        #[default]
        Am = ("am", "AM (broadcast)"),
        /// FM video (FPV and video links).
        FmVideo = ("fm", "FM video (FPV)"),
        /// NBTV or mechanical television on AM.
        NbtvAm = ("nbtv_am", "NBTV / mechanical on AM"),
        /// NBTV on narrow FM.
        NbtvFm = ("nbtv_fm", "NBTV / mechanical on FM"),
    }
}

/// The supervisor's instructions to the other components (fed back; they take effect from the first sample
/// of the next pass).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tune {
    /// Counts the commands that restart the signal path; a component that sees a new epoch starts again.
    pub epoch: u32,
    /// The supervisor's state.
    pub phase: Phase,
    /// The vision carrier in the (mirrored if `mirrored`) input, Hz from the centre.
    pub carrier_hz: f64,
    /// The input is conjugated.
    pub mirrored: bool,
    /// Centre-spike removal off (the carrier is at the centre).
    pub spike_off: bool,
    /// Sound spacing above the carrier, Hz (0: unknown).
    pub spacing_hz: f64,
    /// The double-sideband measure of the search, dB.
    pub dsb_db: f64,
    /// The weak-signal slicer.
    pub weak: bool,
    /// The sound carrier is AM.
    pub sound_am: bool,
    /// The system as identified or forced.
    pub system: Option<SystemId>,
    /// Look for NICAM and A2.
    pub second_sound: bool,
    /// Snow is drawn (no picture signal).
    pub snow: bool,
    /// Polarity for FM video and NBTV: true when the sync tip is the highest value (inverted).
    pub inverted: bool,
    /// The system was chosen by the user (only its raster and polarity are searched).
    pub forced: bool,
    /// The detector setting is "envelope only".
    pub envelope_only: bool,
}

impl Default for Tune {
    fn default() -> Self {
        Tune {
            epoch: 0,
            phase: Phase::Search,
            carrier_hz: 0.0,
            mirrored: false,
            spike_off: false,
            spacing_hz: 0.0,
            dsb_db: 0.0,
            weak: false,
            sound_am: false,
            system: None,
            second_sound: false,
            snow: false,
            inverted: false,
            forced: false,
            envelope_only: false,
        }
    }
}

impl Item for Tune {
    const KIND: &'static str = "atv.tune";
}

/// The supervisor's states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Looking for a carrier.
    Search,
    /// Trying a carrier.
    Trial,
    /// Following a carrier.
    Run,
}

/// One candidate vision carrier of the search.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Candidate {
    /// Position from the centre in the (mirrored if `mirrored`) input, Hz.
    pub hz: f64,
    /// Found in the mirrored spectrum.
    pub mirrored: bool,
    /// Found without spike removal at the centre.
    pub centre: bool,
    /// Prominence, dB.
    pub prominence: f64,
    /// Sound spacing found, Hz (0: none).
    pub spacing_hz: f64,
    /// The double-sideband measure, dB.
    pub dsb_db: f64,
    /// Score.
    pub score: f64,
    /// Level of the line, dB.
    pub level_db: f64,
}

/// What the search found after its spectra were judged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Candidates {
    /// Best first (at most 6).
    pub list: Vec<Candidate>,
}

impl Item for Candidates {
    const KIND: &'static str = "atv.carriers";
}

/// The search's monitor report (each tick): the strongest line near the carrier, and the display spectrum.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Monitor {
    /// Monitor spectra so far.
    pub spectra: u32,
    /// The strongest line within ±150 kHz of the carrier, Hz.
    pub line_hz: f64,
    /// Its prominence, dB.
    pub prominence: f64,
    /// Its level, dB.
    pub level_db: f64,
    /// The display spectrum: (MHz from the carrier or the centre, dB below the strongest).
    pub spectrum: Vec<[f32; 2]>,
    /// Bins of the display spectrum in use (the spectrum is updated in place).
    pub fresh: bool,
}

impl Item for Monitor {
    const KIND: &'static str = "atv.monitor";
}

/// The vision channel's report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VisionReport {
    /// Video rate.
    pub rate_hz: f64,
    /// Noise gain of the channel filter (sum of squared taps).
    pub noise_gain: f64,
    /// Input rate.
    pub input_hz: f64,
    /// The VSB correction amount.
    pub vsb: f64,
}

impl Item for VisionReport {
    const KIND: &'static str = "atv.vision_report";
}

/// One line as the sync decided it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineInfo {
    /// Running count of lines since the loop started.
    pub counter: u64,
    /// Line number in the frame, 1-based (0: not known yet).
    pub number: u32,
    /// Absolute sample index (video rate) of 0H.
    pub t0: f64,
    /// Line period, samples.
    pub period: f64,
    /// Blanking level (sync-polarity units, sync tip the largest).
    pub blank: f32,
    /// Gain: (tip - blanking) / (-sync level).
    pub gain: f32,
    /// +1 when the video is the detected signal with the sync tip the largest, -1 when negated.
    pub pol: f32,
    /// Lines a frame of the raster.
    pub lines: u32,
    /// A pulse was found for this line.
    pub found: bool,
    /// Global line parity is odd.
    pub odd: bool,
}

impl Item for LineInfo {
    const KIND: &'static str = "atv.line";
}

/// The burst loop of a line, as the picture decoder uses it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carrier {
    /// The phase of the loop (radians) at sample `anchor`.
    pub phase: f64,
    /// The sample of `phase`.
    pub anchor: f64,
    /// The subcarrier of the loop, Hz.
    pub fsc: f64,
    /// Burst level, share of nominal.
    pub amp: f64,
    /// The V axis switches line by line (PAL).
    pub v_switch: bool,
    /// The V part of the burst on even and on odd lines.
    pub vre: [f64; 2],
}

impl Carrier {
    /// The phase of the loop at sample `k` of a stream of rate `fv`.
    #[inline]
    pub fn phase_at(&self, k: f64, fv: f64) -> f64 {
        self.phase + std::f64::consts::TAU * self.fsc * (k - self.anchor) / fv
    }
}

/// One visible line for the picture decoder, with the colour state of that line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowJob {
    /// The line.
    pub line: LineInfo,
    /// The row of the frame.
    pub row: u32,
    /// The field of the row, 0 or 1.
    pub field: u8,
    /// The colour decided.
    pub decided: Option<Colour>,
    /// Colour locked, not killed and switched on.
    pub colour: bool,
    /// The gain that takes out the sync compression.
    pub gain: f64,
    /// The burst loop chosen.
    pub carrier: Option<Carrier>,
    /// SECAM: the odd lines carry D'R. `None` until the lines are identified.
    pub odd_is_r: Option<bool>,
    /// The baseband at blanking.
    pub dc: Complex32,
    /// The samples that the colour lock held when it ordered the row: a sample from this index on reads as zero.
    pub held: u64,
}

/// An order of the colour lock to the picture decoder. The orders of a pass are in line order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineJob {
    /// A new epoch: the picture, the counts and the SECAM rows start again.
    Restart,
    /// The colour setting changed: the SECAM rows start again.
    SecamRestart,
    /// The raster changed to this many lines.
    Raster(u32),
    /// The field ended at `time` seconds, with this video SNR in dB.
    EndField {
        /// The field, 0 or 1.
        field: u8,
        /// Seconds of the stream.
        time: f64,
        /// Video SNR, dB.
        video_snr: f64,
    },
    /// A visible line.
    Row(RowJob),
}

impl Item for LineJob {
    const KIND: &'static str = "atv.line_job";
}

/// The sync's report (each tick).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SyncReport {
    /// 0 searching, 1 line sync, 2 line and field sync.
    pub state: u8,
    /// Lines a frame (0 unknown).
    pub lines: u32,
    /// Smoothed share of lines with a pulse.
    pub quality: f64,
    /// Line rate in Hz of the sample clock.
    pub line_hz: f64,
    /// Lines since the loop started.
    pub line_count: u64,
    /// Field syncs since the loop started.
    pub fields: u64,
    /// The last field (1 or 2).
    pub last_field: u8,
    /// Positive modulation (the decoder works on the negated video).
    pub positive: bool,
    /// Sync tip and blanking levels of the detected video.
    pub tip: f64,
    /// Blanking.
    pub blank: f64,
    /// Gain.
    pub gain: f64,
    /// Porch noise variance (detected units).
    pub noise_var: f64,
    /// Synchronous detector in use.
    pub synchronous: bool,
    /// Carrier phase locked.
    pub phase_locked: bool,
    /// The carrier loop's frequency trim, Hz.
    pub trim_hz: f64,
    /// Matched-filter (noisy) mode.
    pub noisy: bool,
    /// Field syncs missed in a row.
    pub field_misses: u32,
    /// Seconds since the line sync was last held.
    pub lost_s: f64,
    /// The VSB correction's DC gain.
    pub dc_gain: f64,
    /// The VSB correction's noise gain (sum of squared taps).
    pub vsb_noise_gain: f64,
    /// The video rate.
    pub rate_hz: f64,
}

impl Item for SyncReport {
    const KIND: &'static str = "atv.sync_report";
}

/// The report of the picture path (each tick). The colour lock fills the colour, the burst, the compression and the
/// scopes; the picture decoder adds the counts, the white peak and the SECAM chroma level.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PictureReport {
    /// The colour decided ("" until then).
    pub colour: Option<Colour>,
    /// Colour locked and not killed.
    pub colour_on: bool,
    /// The killer is on.
    pub killer: bool,
    /// Burst level, share of nominal.
    pub burst: f64,
    /// Burst phase error, degrees rms.
    pub burst_phase_deg: f64,
    /// SECAM chroma level.
    pub chroma_level: f64,
    /// Fields good and bad.
    pub fields_ok: u64,
    /// Bad fields.
    pub fields_bad: u64,
    /// Pictures published.
    pub pictures: u64,
    /// Snow pictures passed on.
    pub snow_pictures: u64,
    /// The highest picture level of the last frame.
    pub white_peak: f64,
    /// Sync compression, %.
    pub compression_pct: f64,
    /// How SECAM was found (empty until it is).
    pub secam_how: &'static str,
    /// Log lines since the last report.
    pub log: Vec<String>,
    /// The line waveform (512 points), shared until the next line is drawn.
    pub line: std::sync::Arc<[[f32; 2]]>,
    /// The vertical-interval waveform (512 points), shared until the next field.
    pub vbi: std::sync::Arc<[[f32; 2]]>,
    /// Video SNR, dB.
    pub video_snr_db: f64,
}

impl Item for PictureReport {
    const KIND: &'static str = "atv.picture_report";
}

/// The sound channel's report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoundReport {
    /// A carrier is there.
    pub present: bool,
    /// AM.
    pub am: bool,
    /// Peak deviation in the last 20 ms, kHz.
    pub deviation_khz: f64,
    /// Audio level, dBFS.
    pub level_dbfs: f64,
    /// The carrier trim, Hz.
    pub trim_hz: f64,
    /// The spacing in use, Hz.
    pub spacing_hz: f64,
}

impl Item for SoundReport {
    const KIND: &'static str = "atv.sound_report";
}

/// The NICAM decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NicamReport {
    /// Frame locked.
    pub locked: bool,
    /// Valid sound.
    pub valid: bool,
    /// The mode, while locked.
    pub mode: Option<crate::nicam::Mode>,
    /// The reserve flag C4.
    pub c4: bool,
    /// Frames since the lock.
    pub frames: u64,
    /// Parity errors.
    pub parity_errors: u64,
    /// Samples decoded.
    pub samples: u64,
    /// Smoothed error rate.
    pub error_rate: f64,
    /// Carrier level, dBFS.
    pub level_dbfs: f64,
    /// Carrier error, Hz.
    pub carrier_error_hz: f64,
    /// Log lines.
    pub log: Vec<String>,
}

impl Item for NicamReport {
    const KIND: &'static str = "atv.nicam_report";
}

/// The A2 decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct A2Report {
    /// The second carrier is there with its pilot.
    pub present: bool,
    /// The identification found.
    pub mode: Option<crate::results::A2Mode>,
    /// Pilot against the noise beside it, dB.
    pub pilot_db: f64,
    /// Pilot level.
    pub level: f64,
    /// Running (part-time while nothing is found).
    pub running: bool,
}

impl Item for A2Report {
    const KIND: &'static str = "atv.a2_report";
}

/// The selector's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectorReport {
    /// The sound that plays.
    pub system: crate::results::SoundPlaying,
}

impl Item for SelectorReport {
    const KIND: &'static str = "atv.selector_report";
}

/// The NBTV decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NbtvReport {
    /// Line locked.
    pub line_locked: bool,
    /// Picture locked.
    pub picture_locked: bool,
    /// Line rate, Hz.
    pub line_hz: f64,
    /// Line error, ppm.
    pub line_ppm: f64,
    /// Sync quality.
    pub quality: f64,
    /// Lines.
    pub lines: u64,
    /// Pictures.
    pub pictures: u64,
    /// Video SNR, dB.
    pub snr_db: f64,
    /// Line waveform (256 points).
    pub line: Vec<[f32; 2]>,
    /// The format name.
    pub name: String,
    /// Carrier level, dBFS.
    pub level_dbfs: f64,
    /// Inverted.
    pub inverted: bool,
    /// Log lines.
    pub log: Vec<String>,
}

impl Item for NbtvReport {
    const KIND: &'static str = "atv.nbtv_report";
}

/// The last item of a typed input this pass.
pub(crate) fn last<T: Item>(io: &sdrkit_graph::Io, i: usize) -> Option<T> {
    io.items::<T>(i).last().map(|t| t.value.clone())
}
