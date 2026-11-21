//! The interface of the terrestrial television family: the shapes of the result sets and the table rows, the tables
//! and what each stack declares.
//!
//! The stacks show the transport stream layer of `sdrkit-proto-mpegts` through the shared interface of that crate. The
//! components of the OFDM chain (`sync`, `fft`, `tps`, `eq`) run in the graphs of `dvbt` and `dvbsh`. Each writes one
//! result set, and a field whose key a result set of the stack declares is the field of the stack.

use crate::bandwidth::WidthSource;
use crate::dvbsh::results::Waveform;
use crate::dvbsh::settings::DvbshSetting;
use crate::params::{CodeRate, Constellation, Guard, Hierarchy, Mode};
use crate::results::{Decoding, Quality, TS_LAYER};
use crate::settings::{DvbtSetting, StreamChoice, TerrestrialSetting};
use sdrkit_core::interface::{
    CommandKind, Family, Fields, Hertz, Seconds, StackShape, Time,
};
use sdrkit_core::publish::keys;
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{Command, SettingSet, TxContent};
use sdrkit_proto_mpegts::demux::PlayerStatus;
use sdrkit_proto_mpegts::layer::{self, TsLayer};
use sdrkit_proto_mpegts::sendqueue::TxAppendArgs;
use sdrkit_proto_mpegts::publish::{
    NETWORK_ID, NETWORK_NAME, SERVICE_COUNT, SERVICE_SELECT_KIND, TELETEXT_SELECT,
    TRACK_SELECT_KIND, UTC,
};
use std::sync::OnceLock;

// The components of the OFDM chain.

/// The result fields of the synchroniser `sync`. No value before the FFT mode and the guard interval are found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SyncFields {
    /// The timing error of the last symbol.
    #[field(label = "Timing error", unit = "samples")]
    pub timing: Option<f64>,
    /// The correlation coefficient of the last symbol.
    #[field(label = "Correlation")]
    pub rho: Option<f64>,
    /// The fractional carrier offset in use.
    #[field(label = "Fractional offset", unit = "carriers")]
    pub frac: Option<f64>,
}

/// The result fields of the FFT stage `fft`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FftFields {
    /// The integer carrier offset. No value before it is found.
    #[field(label = "Integer carrier offset", unit = "carriers")]
    pub integer_offset: Option<i64>,
    /// The spectrum is mirrored.
    #[field(label = "Spectrum inverted")]
    pub spectrum_inverted: bool,
}

/// The result fields of the TPS decoder `tps`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TpsFields {
    /// TPS frames decoded.
    #[field(label = "TPS frames decoded")]
    pub tps_frames_ok: i64,
    /// TPS frames that failed.
    #[field(label = "TPS frames failed")]
    pub tps_frames_failed: i64,
}

/// The result fields of the equaliser `eq`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EqFields {
    /// The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the
    /// channel estimate is filled.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The modulation error ratio. No value before the channel estimate is filled.
    #[field(shared = keys::MER_DB)]
    pub mer_db: Option<f64>,
    /// The time-averaging gain of the channel estimate.
    #[field(label = "Averaging gain")]
    pub alpha: f64,
    /// The centre of the delay window.
    #[field(label = "Delay window centre", unit = "samples")]
    pub delay_window_centre: f64,
    /// The half width of the delay window.
    #[field(label = "Delay window half width", unit = "samples")]
    pub delay_window_half: f64,
    /// The measured delay spread.
    #[field(label = "Delay spread", unit = "samples")]
    pub delay_spread: f64,
}

/// The result fields of the error correction `fec`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FecFields {
    /// Transport packets delivered, with those of the blocks that failed.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Reed-Solomon blocks that could not be corrected. Each gives a packet with the transport_error_indicator set.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
}

// DVB-T.

/// The result fields of `dvbt`, which `report` writes: the input, the OFDM parameters, the TPS, the error correction
/// and the quality. A value of the TPS has no value while the TPS is not locked; a value of the FFT mode has no
/// value before the mode is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbtFields<'a> {
    /// The detection level, 0 to 3.
    #[field(label = "Detection level")]
    pub detect_level: i64,
    /// The channel width in use.
    #[field(label = "Channel width", unit = "Hz")]
    pub channel_width: Hertz,
    /// The `bandwidth` setting is Auto.
    #[field(label = "Bandwidth found automatically")]
    pub bandwidth_auto: bool,
    /// Where the width in use came from.
    #[field(label = "Bandwidth from")]
    pub bandwidth_from: Option<WidthSource>,
    /// The measured occupied width.
    #[field(label = "Measured width", unit = "Hz")]
    pub measured_width: Option<Hertz>,
    /// The level of the channel above the floor outside it.
    #[field(label = "Channel above floor", unit = "dB")]
    pub occupancy_db: Option<f64>,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate: Hertz,
    /// The natural rate of the width in use.
    #[field(label = "Natural rate", unit = "Hz")]
    pub natural_rate: Hertz,
    /// The input is resampled.
    #[field(label = "Resampling")]
    pub resampling: bool,
    /// The input rate is enough for the width.
    #[field(label = "Input rate enough")]
    pub rate_ok: bool,
    /// The FFT mode.
    #[field(label = "FFT mode")]
    pub fft: Option<Mode>,
    /// The FFT size.
    #[field(label = "FFT size")]
    pub fft_size: Option<i64>,
    /// The guard interval.
    #[field(label = "Guard interval")]
    pub guard: Option<Guard>,
    /// The guard samples.
    #[field(label = "Guard samples")]
    pub guard_samples: Option<i64>,
    /// The carriers.
    #[field(label = "Carriers")]
    pub carriers: Option<i64>,
    /// The constellation.
    #[field(label = "Constellation")]
    pub constellation: Option<Constellation>,
    /// The hierarchy.
    #[field(label = "Hierarchy")]
    pub hierarchy: Option<Hierarchy>,
    /// The HP code rate.
    #[field(label = "Code rate (HP)")]
    pub code_rate_hp: Option<CodeRate>,
    /// The LP code rate.
    #[field(label = "Code rate (LP)")]
    pub code_rate_lp: Option<CodeRate>,
    /// The stream decoded.
    #[field(label = "Stream")]
    pub stream: StreamChoice,
    /// The cell id. No value until two frames agree.
    #[field(label = "Cell id")]
    pub cell_id: Option<i64>,
    /// The TPS length indicator.
    #[field(label = "TPS length")]
    pub tps_length: Option<i64>,
    /// The DVB-H signalling bits.
    #[field(label = "DVB-H flags")]
    pub dvbh_flags: Option<i64>,
    /// The frame number, 0 to 3.
    #[field(label = "Frame")]
    pub frame: Option<i64>,
    /// The symbol in the frame, 0 to 67.
    #[field(label = "Symbol")]
    pub symbol: Option<i64>,
    /// The duration of a frame.
    #[field(label = "Frame duration", unit = "s")]
    pub frame_duration: Option<Seconds>,
    /// The net bit rate of the stream decoded.
    #[field(label = "Net bit rate", unit = "bit/s")]
    pub net_bitrate_bps: Option<f64>,
    /// The carrier offset from the centre.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The integer carrier offset.
    #[field(label = "Integer carrier offset", unit = "carriers")]
    pub integer_offset: Option<i64>,
    /// The shift of the channel locator.
    #[field(label = "Channel locator shift", unit = "Hz")]
    pub locator_hz: Hertz,
    /// The error of the sample clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The spectrum is mirrored.
    #[field(label = "Spectrum inverted")]
    pub spectrum_inverted: bool,
    /// The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the
    /// channel estimate is filled.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The modulation error ratio. No value before the channel estimate is filled.
    #[field(shared = keys::MER_DB)]
    pub mer_db: Option<f64>,
    /// The BER before Viterbi over the last second: the received hard bits that differ from the decoded bits, coded
    /// again.
    #[field(label = "BER before Viterbi")]
    pub ber_pre_viterbi: f64,
    /// The BER after Viterbi over the last second: the bits that Reed-Solomon corrected over the bits of the blocks
    /// that it decoded. A block that failed is not in it and is counted in `rs_failed`. 0 while no block decoded.
    #[field(label = "BER after Viterbi")]
    pub ber_post_viterbi: f64,
    /// The Viterbi margin.
    #[field(label = "Viterbi margin")]
    pub viterbi_margin: f64,
    /// The puncturing phase.
    #[field(label = "Puncturing phase")]
    pub puncture_phase: Option<i64>,
    /// RS blocks handled: those without error, those corrected and those that failed.
    #[field(label = "RS blocks")]
    pub rs_blocks: i64,
    /// RS blocks without error.
    #[field(label = "RS blocks clean")]
    pub rs_clean: i64,
    /// RS blocks with 1 to 8 bytes wrong, which were corrected. Their corrected bits are the BER after Viterbi.
    #[field(label = "RS blocks corrected")]
    pub rs_corrected: i64,
    /// RS blocks that could not be corrected (more than 8 bytes wrong). Each gives a packet with the
    /// transport_error_indicator set. They do not enter the BER after Viterbi.
    #[field(label = "RS blocks failed")]
    pub rs_failed: i64,
    /// Bytes that were corrected in the RS blocks corrected.
    #[field(label = "RS bytes corrected")]
    pub rs_bytes_corrected: i64,
    /// Transport packets delivered, with those of the blocks that failed.
    #[field(label = "Packets")]
    pub packets: i64,
    /// The packet alignment holds.
    #[field(label = "Packet alignment")]
    pub fec_sync: bool,
    /// Resynchronisations.
    #[field(label = "Resynchronisations")]
    pub resyncs: i64,
    /// The signal time since the last good TPS frame. No value before the first good frame.
    #[field(label = "Since the last good TPS", unit = "s")]
    pub since_tps: Option<Seconds>,
    /// The quality score, 0 to 100.
    #[field(label = "Quality", unit = "%")]
    pub quality_pct: f64,
    /// The quality class.
    #[field(label = "Quality")]
    pub quality_label: Quality,
    /// The C/N that the constellation and code rate need.
    #[field(label = "Required C/N", unit = "dB")]
    pub required_cn_db: Option<f64>,
    /// The SNR above the required C/N.
    #[field(label = "SNR margin", unit = "dB")]
    pub margin_db: Option<f64>,
    /// The features that TPS signals and the receiver does not decode.
    #[field(label = "Unsupported features")]
    pub unsupported: Option<&'a str>,
    /// The channel estimate is filled.
    #[field(label = "Data valid")]
    pub data_valid: bool,
    /// The signal time of the first good packet.
    #[field(label = "First packet", unit = "s")]
    pub first_packet: Option<Seconds>,
    /// The level of the input. No value before it is measured.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
}

/// The fields of the transport stream layer that `dvbt` shows under its own keys: the network and the time of the
/// stream (`psi_si`), and the service that plays (`ts_demux`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbtMultiplexFields<'a> {
    /// The network name.
    #[field(shared = NETWORK_NAME)]
    pub network_name: &'a str,
    /// network_id.
    #[field(shared = NETWORK_ID)]
    pub network_id: i64,
    /// The number of services.
    #[field(shared = SERVICE_COUNT)]
    pub service_count: i64,
    /// The UTC of the stream. No value before the stream gave the time.
    #[field(shared = UTC)]
    pub utc: Option<Time>,
    /// The service that plays. No value while none plays.
    #[field(label = "Service playing")]
    pub selected_service: Option<i64>,
    /// What the player does.
    #[field(label = "Player")]
    pub player_status: PlayerStatus,
    /// The bit rate of the multiplex.
    #[field(label = "Multiplex bit rate", unit = "kbit/s")]
    pub mux_kbps: f64,
}

/// The result fields of the sender `transmit` of `dvbt`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbtTxFields {
    /// Whether a transmission is under way.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the centre of the channel sent. No value while the stack does not transmit.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// The bit rate of the transport stream that the settings carry (of the high-priority stream in a hierarchical
    /// multiplex): the rate that the host feeds at most.
    #[field(label = "Useful bit rate", unit = "bit/s")]
    pub tx_bitrate_bps: f64,
    /// The bit rate of the low-priority stream. No value in a multiplex that is not hierarchical.
    #[field(label = "Useful bit rate (LP)", unit = "bit/s")]
    pub tx_bitrate_lp_bps: Option<f64>,
    /// The packets of the host sent in this transmission, or in the last one.
    #[field(label = "Packets sent")]
    pub tx_packets: i64,
    /// The null packets put in slots that found no packet of the host.
    #[field(label = "Null packets inserted")]
    pub tx_nulls: i64,
    /// The packets that wait to be sent.
    #[field(label = "Packets waiting")]
    pub tx_queue: i64,
    /// The packets of the low-priority stream that wait to be sent.
    #[field(label = "Packets waiting (LP)")]
    pub tx_queue_lp: i64,
    /// The time that the waiting packets take at the useful bit rate.
    #[field(label = "Queue length", unit = "s")]
    pub tx_queue_s: Seconds,
    /// The PCRs rewritten to the time of their slot.
    #[field(label = "PCRs rewritten")]
    pub tx_pcr_restamped: i64,
}

/// Starts sending.
pub const TX_START: Command = Command::new("start", "Start sending")
    .help(
        "Sends a DVB-T signal with the tx_* settings from the start of a super-frame, until stop. The packets of append \
         fill it; a slot with no packet carries a null packet. Transmit must be on.",
    )
    .transmits(TxContent::File)
    .stopped_by("stop");
/// Stops sending.
pub const TX_STOP: Command = Command::new("stop", "Stop sending").help(
    "Ends the transmission after the frame on air with a fall of 5 ms. The packets that wait are dropped.",
);
/// Adds packets to the stream.
pub const TX_APPEND: CommandKind<TxAppendArgs> = CommandKind::new("append", "Add packets")
    .help(
        "Adds transport packets to the queue of the stream, or of the high-priority stream of a hierarchical \
         multiplex. All or none: refused when the queue has no room, so the host feeds at the useful bit rate.",
    );
/// Adds packets to the low-priority stream.
pub const TX_APPEND_LP: CommandKind<TxAppendArgs> =
    CommandKind::new("append_lp", "Add packets (LP)").help(
        "Adds transport packets to the queue of the low-priority stream of a hierarchical multiplex. All or none.",
    );

// DVB-SH.

/// The result fields of `dvbsh`, which `data` writes: the OFDM parameters, the SH frame and the turbo code. A value
/// of the TPS has no value while the TPS is not locked.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbshFields<'a> {
    /// OFDM or TDM.
    #[field(label = "Waveform")]
    pub waveform: Waveform,
    /// The channel width in use.
    #[field(label = "Channel width", unit = "MHz")]
    pub width_mhz: f64,
    /// 0 searching, 1 OFDM symbols found, 2 TPS, 3 parameters, 4 transport stream flowing.
    #[field(label = "Lock")]
    pub lock: i64,
    /// The carrier offset. No value before the FFT mode is found.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub cfo_hz: Option<Hertz>,
    /// The FFT size.
    #[field(label = "FFT size")]
    pub fft_size: Option<i64>,
    /// The guard samples.
    #[field(label = "Guard samples")]
    pub guard_samples: Option<i64>,
    /// The OFDM mode from TPS.
    #[field(label = "Mode")]
    pub mode: Option<&'a str>,
    /// The guard interval from TPS.
    #[field(label = "Guard interval")]
    pub guard: Option<&'a str>,
    /// The constellation from TPS.
    #[field(label = "Constellation")]
    pub constellation: Option<&'a str>,
    /// The hierarchy from TPS.
    #[field(label = "Hierarchy")]
    pub hierarchy: Option<&'a str>,
    /// The code rate of the high-priority stream ("1/3", "1/3c" for complementary).
    #[field(label = "HP code rate")]
    pub rate_hp: Option<&'a str>,
    /// The code rate of the low-priority stream of a hierarchical multiplex.
    #[field(label = "LP code rate")]
    pub rate_lp: Option<&'a str>,
    /// The stream decoded, HP or LP.
    #[field(label = "Stream decoded")]
    pub stream: &'a str,
    /// The interleaver profile, "cm / lt / ns / sd / nli".
    #[field(label = "Time interleaver")]
    pub interleaver: Option<&'a str>,
    /// The length of the time interleaver.
    #[field(label = "Time interleaver length", unit = "s")]
    pub interleaver_s: Option<Seconds>,
    /// The memory that the time de-interleaver holds.
    #[field(label = "Time de-interleaver memory", unit = "MB")]
    pub interleaver_mb: Option<f64>,
    /// Why the data path does not run (an interleaver too long for this receiver, a reserved pattern). No value while
    /// it runs.
    #[field(label = "Data path refused")]
    pub refused: Option<&'a str>,
    /// The 16-bit cell id from TPS.
    #[field(label = "Cell id")]
    pub cell_id: Option<i64>,
    /// The super-frame number in the SH frame.
    #[field(label = "Super-frame in the SH frame")]
    pub superframe: Option<i64>,
    /// Code words decoded.
    #[field(label = "Code words decoded")]
    pub codewords_ok: i64,
    /// Code words not decoded.
    #[field(label = "Code words failed")]
    pub codewords_bad: i64,
    /// Packets with a good CRC.
    #[field(label = "Packets good")]
    pub packets_ok: i64,
    /// Packets passed with the transport_error_indicator.
    #[field(label = "Packets flagged")]
    pub packets_bad: i64,
    /// The mean turbo iterations of the decoded code words. No value before a code word decodes.
    #[field(label = "Turbo iterations (mean)")]
    pub turbo_iterations: Option<f64>,
    /// The signal time of the first good packet.
    #[field(label = "First packet", unit = "s")]
    pub first_packet_s: Option<Seconds>,
    /// What the stack decodes and which of its values no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The error of the sample clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The level of the input. No value before it is measured.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The modulation of the TDM form, which the SOF shows.
    #[field(label = "TDM modulation")]
    pub tdm_modulation: Option<&'a str>,
    /// The symbol rate of the TDM form that is tried or received.
    #[field(label = "TDM symbol rate", unit = "sym/s")]
    pub tdm_symbol_rate: Option<f64>,
    /// The roll-off of the TDM form, from the row of the symbol rate. No value before slots are found.
    #[field(label = "TDM roll-off")]
    pub tdm_rolloff: Option<f64>,
    /// The width of the TDM SH frame from its signalling field.
    #[field(label = "TDM frame width", unit = "CU")]
    pub tdm_frame_width_cu: Option<i64>,
    /// The power of a pilot symbol of the TDM form over the noise in the symbol rate.
    #[field(label = "TDM MER", unit = "dB")]
    pub tdm_mer_db: Option<f64>,
    /// Slots of the TDM form erased for want of pilots.
    #[field(label = "TDM slots erased")]
    pub tdm_slots_erased: i64,
    /// Signalling fields of the TDM form that did not decode.
    #[field(label = "TDM signalling fields failed")]
    pub tdm_signalling_failed: i64,
}

/// The fields of the OFDM chain that `dvbsh` shows under its own keys: the SNR of the equaliser and the counts of the
/// TPS decoder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbshChainFields {
    /// The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the
    /// channel estimate is filled.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// TPS frames decoded.
    #[field(label = "TPS frames decoded")]
    pub tps_frames_ok: i64,
    /// TPS frames that failed.
    #[field(label = "TPS frames failed")]
    pub tps_frames_failed: i64,
}

// The description.

/// `shape` with the components of the OFDM chain from the synchroniser to the equaliser, in the order of the graph.
fn with_chain(shape: StackShape) -> StackShape {
    shape
        .component("sync", |c| c.fields::<SyncFields>().shares_declared())
        .component("fft", |c| c.fields::<FftFields>().shares_declared())
        .component("tps", |c| c.fields::<TpsFields>().shares_declared())
        .component("eq", |c| c.fields::<EqFields>().shares_declared())
}

/// `shape` with the transport stream layer of `parts`, the plots of the equaliser and the commands of the layer.
fn with_layer(shape: StackShape, parts: &TsLayer) -> StackShape {
    let mut s = layer::describe(shape, parts);
    for series in crate::equaliser::SERIES {
        s = s.series(series);
    }
    s = s.command(&SERVICE_SELECT_KIND);
    if parts.teletext {
        s = s.command(&TELETEXT_SELECT);
    }
    s.command(&TRACK_SELECT_KIND)
}

/// The description of the DVB-T receiver of [`crate::receiver::receiver_with`] for the stack `id` with `settings`.
/// The guide holds the events of every multiplex that named them, so it is kept over a change of frequency; the services, the tracks, the teletext pages and every measurement belong to the
/// multiplex of the frequency before.
pub fn dvbt_receiver(id: &'static str, settings: &[&'static dyn DeclareSetting]) -> StackShape {
    let s = StackShape::new(id)
        .fields::<DvbtFields<'static>>()
        .fields::<DvbtMultiplexFields<'static>>();
    let s = with_chain(s).component("fec", |c| c.fields::<FecFields>().shares_declared());
    with_layer(s, &TS_LAYER)
        .settings(settings)
        .kept(&["guide", "guide_ratings"])
}

/// The result set that the receiver of `dvb_terrestrial` adds to the results of the DVB-T receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fields)]
pub struct DvbTerrestrialFields {
    /// The standard that decodes now.
    #[field(label = "Standard")]
    pub standard: Decoding,
}

/// The description of the receiver graph of `dvb_terrestrial`: that of `dvbt` with the settings of the stack. The
/// stack adds [`DvbTerrestrialFields`] after the fields of the components.
pub fn terrestrial_receiver() -> &'static StackShape {
    static SHAPE: OnceLock<StackShape> = OnceLock::new();
    SHAPE.get_or_init(|| dvbt_receiver(crate::terrestrial::ID, TerrestrialSetting::SETTINGS))
}

/// The declarations of the sender of `dvbt`, added in 2.4.
/// The declarations of the TDM form of `dvbsh`.
const ADDED_2_5: &[&str] = &[
    "shape.DvbshFields.tdm_modulation",
    "shape.DvbshFields.tdm_symbol_rate",
    "shape.DvbshFields.tdm_rolloff",
    "shape.DvbshFields.tdm_frame_width_cu",
    "shape.DvbshFields.tdm_mer_db",
    "shape.DvbshFields.tdm_slots_erased",
    "shape.DvbshFields.tdm_signalling_failed",
    "dvbsh.setting.waveform",
    "dvbsh.setting.tdm_gold",
    "dvbsh.setting.tdm_symbol_rate",
    "dvbsh.setting.gen_waveform",
    "dvbsh.setting.gen_rolloff",
];

const ADDED_2_4: &[&str] = &[
    "dvbt.fields.DvbtTxFields",
    "shape.DvbtTxFields",
    "shape.DvbtTxFields.transmitting",
    "shape.DvbtTxFields.tx_freq_hz",
    "shape.DvbtTxFields.tx_bitrate_bps",
    "shape.DvbtTxFields.tx_bitrate_lp_bps",
    "shape.DvbtTxFields.tx_packets",
    "shape.DvbtTxFields.tx_nulls",
    "shape.DvbtTxFields.tx_queue",
    "shape.DvbtTxFields.tx_queue_lp",
    "shape.DvbtTxFields.tx_queue_s",
    "shape.DvbtTxFields.tx_pcr_restamped",
    "dvbt.setting.tx_bandwidth",
    "dvbt.setting.tx_mode",
    "dvbt.setting.tx_guard",
    "dvbt.setting.tx_constellation",
    "dvbt.setting.tx_hierarchy",
    "dvbt.setting.tx_rate_hp",
    "dvbt.setting.tx_rate_lp",
    "dvbt.setting.tx_cell_id",
    "dvbt.command.start",
    "dvbt.command.stop",
    "dvbt.command.append",
    "dvbt.command.append_lp",
];

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let dvbt = dvbt_receiver(crate::receiver::ID, DvbtSetting::SETTINGS)
            .fields::<DvbtTxFields>()
            .command(&TX_START)
            .command(&TX_STOP)
            .command(&TX_APPEND)
            .command(&TX_APPEND_LP);
        // The guide is kept; the services, the tracks and every measurement belong to the multiplex before.
        let dvbsh = with_chain(
            StackShape::new(crate::dvbsh::ID)
                .fields::<DvbshFields<'static>>()
                .fields::<DvbshChainFields>(),
        );
        let dvbsh = with_layer(dvbsh, &TsLayer::basic())
            .settings(DvbshSetting::SETTINGS)
            .kept(&["guide", "guide_ratings"]);
        let terrestrial = terrestrial_receiver()
            .clone()
            .fields_after_components::<DvbTerrestrialFields>();
        Family::new("dvbt", 2, 5)
            .uses(sdrkit_proto_mpegts::publish::interface())
            .shared_source("results", &["dvbt"])
            .shared_source("report", &["dvbt"])
            .shared_source("scan", &["dvbt"])
            .shared_source("fecstage", &["dvbt"])
            .shared_source("sync", &["dvbt", "dvbsh"])
            .shared_source("fft", &["dvbt", "dvbsh"])
            .shared_source("tpsdec", &["dvbt", "dvbsh"])
            .shared_source("equaliser", &["dvbt", "dvbsh"])
            .shared_source("terrestrial", &["dvb_terrestrial"])
            .stack(dvbt)
            .stack(terrestrial)
            .stack(dvbsh)
            .added(
                2,
                1,
                &["dvbt.setting.downmix", "dvb_terrestrial.setting.downmix"],
            )
            .added(
                2,
                2,
                &[
                    "dvbt.setting.decode_video",
                    "dvb_terrestrial.setting.decode_video",
                ],
            )
            .added(2, 3, &["dvbsh.setting.decode_video"])
            .added(2, 4, ADDED_2_4)
            .added(2, 5, ADDED_2_5)
    })
}

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the dvbt family does not describe the stack {id}"))
}
