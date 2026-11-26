//! PlutoSDR, ANTSDR and other AD936x boards over the network through the iiod protocol, on the shared base of the network
//! radios. A control connection carries the context and the attributes. A second connection carries the samples, so a change
//! never waits behind a blocking read. The address is `ip:192.168.2.1`, `192.168.2.1` or `host:port` (port 30431 if none).

pub mod binary;
pub mod context;
pub mod fir;
pub mod proto;
pub mod samples;
pub mod source;
pub mod tx;

use crate::net::Abort;
use crate::rate::{self, LinkKind};
use crate::gain::{HeldGain, HoldStatus, Level, Meter};
use crate::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use crate::{
    fmt_m, own_id, Capabilities, DriverDescriptor, DriverError, Protocol, RadioSetting, RateLimits,
    RingWriter, Source, SourceDriver, StreamFormat, StreamLayout, TxLimits, AGC, BANDWIDTH, CENTRE,
    GAIN, RATE, RATE_EXACT,
};
use binary::BinaryRx;
use context::{parse_range, Context, Device};
use proto::{IioError, IiodConn, Target, IIOD_PORT};
use samples::{bit, layout, mask_of, Layout};
use sdrkit_core::net::{host_port, is_local, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{Hz, Setting, SettingId, SettingSet, Values};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use source::IiodSource;

/// The AD9361 control device.
pub const PHY: &str = "ad9361-phy";
/// The AD9361 receive stream device.
pub const RX_DEV: &str = "cf-ad9361-lpc";
/// The AD9361 transmit stream device.
pub const TX_DEV: &str = "cf-ad9361-dds-core-lpc";
/// The RX LO channel (output) of the phy.
pub const RX_LO: &str = "altvoltage0";
/// The TX LO channel (output) of the phy.
pub const TX_LO: &str = "altvoltage1";

/// The lowest converter rate the AD9361 clock chain returns without the FIR (25 MHz / 12).
pub const CONVERTER_MIN: Hz = 25e6 / 12.0;
/// The driver's lowest rate without the FIR.
pub const MIN_RATE: Hz = 2.1e6;
/// The lowest rate the radio offers without the FIR (DAB, sent as requested).
const NO_FIR_MIN_RATE: Hz = 2.048e6;
/// The lowest rate with the FIR decimating by 4.
pub const FIR_MIN_RATE: Hz = CONVERTER_MIN / 4.0;
/// The highest rate.
pub const MAX_RATE: Hz = 61.44e6;
/// The highest rate of an AD9361 running two receivers and two transmitters (2R2T): its data port carries half.
pub const MAX_RATE_2R2T: Hz = 30.72e6;
/// The rate of a radio that no stack has set a rate on. It is low, so that an open radio without a session does not load
/// the network.
const DEFAULT_RATE: Hz = 2.5e6;
/// The highest rate that a radio keeps when no stack has set a rate on it. A change of rate can take the radio more than
/// a second, so an open keeps the rate the radio runs at, unless that rate would load the network.
const IDLE_KEEP_MAX: Hz = 5e6;
const SMALL_BUFFER: usize = 65_536;
const MAX_BUFFER: usize = 1_048_576;
const CONTEXT_TIMEOUT_MS: u32 = 3000;
const MAX_REFILL_FAILURES: u32 = 20;
/// What gigabit Ethernet carries of sample data, in bytes per second (about 940 Mbit/s of TCP payload).
const GIGABIT_BYTES: f64 = 117e6;
/// The part of the measured link that a stream may use: the rest is room for the jitter of the link.
const LINK_SHARE: f64 = 0.85;
/// The window over which the stream measures what arrives.
const WATCH_WINDOW: Duration = Duration::from_secs(2);
/// A stream that gets less than this part of its rate over a window is short of the link.
const SHORT: f64 = 0.9;
/// The longest wait at a stream start for a buffer that a stream of this program left behind.
const OWN_BUFFER_WAIT: Duration = Duration::from_secs(3);
/// How long a buffer left behind counts as ours after our stream ended.
const OWN_BUFFER_RECENT: Duration = Duration::from_secs(10);
/// The longest time the kernel buffers fill before the timing at the start of a stream reads them.
const PROBE_FILL_MAX: Duration = Duration::from_millis(400);
/// The start of a stream that the watch does not judge: a start, a retune or a change of stack delivers a little short
/// for a moment, which says nothing about the link.
const WATCH_SETTLE: Duration = Duration::from_secs(3);
/// The windows in a row that must get less than [`SHORT`] of the rate before the link counts as the limit.
const SHORT_WINDOWS: u32 = 2;
/// How long a reading of the link counts. The link to a board across Wi-Fi changes from second to second, so an older
/// reading says nothing about it.
const LIVE_FRESH: Duration = Duration::from_secs(5);
/// The weight of the previous figure when a new reading of the running stream comes: the live figure follows the link
/// over a few windows, up as well as down.
const LIVE_SMOOTH: f64 = 0.5;

/// `x` to two significant figures.
fn round2(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        return x;
    }
    let scale = 10f64.powi(1 - x.log10().floor() as i32);
    (x * scale).round() / scale
}

/// What queued buffers that came at `bps` bytes a second say of a link, for a stream that needs `need_bps`: `true` when the
/// link carries no more than that (they came faster than the rate, or the link held them below it), `false` when they came
/// at the rate (the link carries at least that).
fn timed_at_most(bps: f64, need_bps: f64) -> bool {
    bps > 1.2 * need_bps || bps < SHORT * need_bps
}

sdrkit_core::choice_enum! {
    /// The gain mode of the AD936x ([`GAIN_MODE`]).
    pub enum GainMode {
        /// The gain setting sets the gain. With [`AGC`](crate::AGC) on, the driver holds a gain that it chose from the level
        /// of the samples ([`GainPlan::Held`]).
        Manual = ("manual", "Manual"),
        /// The AGC of the radio, slow attack.
        SlowAttack = ("slow_attack", "Radio AGC, slow attack"),
        /// The AGC of the radio, fast attack.
        FastAttack = ("fast_attack", "Radio AGC, fast attack"),
        /// The AGC of the radio, hybrid.
        Hybrid = ("hybrid", "Radio AGC, hybrid"),
    }
}

impl GainMode {
    /// The value of the AD936x attribute `gain_control_mode`.
    pub fn attr(self) -> &'static str {
        match self {
            GainMode::Manual => "manual",
            GainMode::SlowAttack => "slow_attack",
            GainMode::FastAttack => "fast_attack",
            GainMode::Hybrid => "hybrid",
        }
    }
}

sdrkit_core::choice_enum! {
    /// The gain mode of RX2 ([`GAIN_MODE2`]).
    pub enum Rx2GainMode {
        /// The gain mode of RX1.
        Same = ("same", "Same as RX1"),
        /// Manual, with [`GAIN2`].
        Manual = ("manual", "Manual"),
        /// The AGC of the radio, slow attack.
        SlowAttack = ("slow_attack", "Radio AGC, slow attack"),
        /// The AGC of the radio, fast attack.
        FastAttack = ("fast_attack", "Radio AGC, fast attack"),
        /// The AGC of the radio, hybrid.
        Hybrid = ("hybrid", "Radio AGC, hybrid"),
    }
}

sdrkit_core::choice_enum! {
    /// The receiver of one stream ([`ANTENNA`]).
    pub enum Antenna {
        /// RX1.
        Rx1 = ("rx1", "RX1"),
        /// RX2.
        Rx2 = ("rx2", "RX2"),
    }
}

sdrkit_core::choice_enum! {
    /// How many receivers stream, and how ([`RECEIVERS`]).
    pub enum Receivers {
        /// One, the one of [`ANTENNA`].
        One = ("one", "One (the antenna)"),
        /// Both, each in its own ring.
        Both = ("both", "Both, as two streams"),
        /// Both, interleaved in one ring.
        Interleaved = ("interleaved", "Both, interleaved in one stream"),
    }
}

sdrkit_core::choice_enum! {
    /// The input of the AD936x that the receivers use ([`RX_PORT`]).
    pub enum RxPort {
        /// As the radio is.
        Auto = ("auto", "As the radio is"),
        /// A_BALANCED.
        ABalanced = ("A_BALANCED", "A_BALANCED"),
        /// B_BALANCED.
        BBalanced = ("B_BALANCED", "B_BALANCED"),
        /// C_BALANCED.
        CBalanced = ("C_BALANCED", "C_BALANCED"),
        /// A_N.
        AN = ("A_N", "A_N"),
        /// A_P.
        AP = ("A_P", "A_P"),
        /// B_N.
        BN = ("B_N", "B_N"),
        /// B_P.
        BP = ("B_P", "B_P"),
        /// C_N.
        CN = ("C_N", "C_N"),
        /// C_P.
        CP = ("C_P", "C_P"),
        /// TX_MONITOR1.
        TxMonitor1 = ("TX_MONITOR1", "TX_MONITOR1"),
        /// TX_MONITOR2.
        TxMonitor2 = ("TX_MONITOR2", "TX_MONITOR2"),
        /// TX_MONITOR1_2.
        TxMonitor12 = ("TX_MONITOR1_2", "TX_MONITOR1_2"),
    }
}

impl RxPort {
    /// The value of the AD936x attribute `rf_port_select`. `None` for [`RxPort::Auto`].
    pub fn attr(self) -> Option<&'static str> {
        Some(match self {
            RxPort::Auto => return None,
            RxPort::ABalanced => "A_BALANCED",
            RxPort::BBalanced => "B_BALANCED",
            RxPort::CBalanced => "C_BALANCED",
            RxPort::AN => "A_N",
            RxPort::AP => "A_P",
            RxPort::BN => "B_N",
            RxPort::BP => "B_P",
            RxPort::CN => "C_N",
            RxPort::CP => "C_P",
            RxPort::TxMonitor1 => "TX_MONITOR1",
            RxPort::TxMonitor2 => "TX_MONITOR2",
            RxPort::TxMonitor12 => "TX_MONITOR1_2",
        })
    }
}

sdrkit_core::choice_enum! {
    /// Samples per transfer ([`BUFFER_SIZE`]).
    pub enum BufferSize {
        /// At least 25 ms.
        Auto = ("auto", "Auto (25 ms)"),
        /// 65 536.
        S64k = ("65536", "65 536"),
        /// 131 072.
        S128k = ("131072", "131 072"),
        /// 262 144.
        S256k = ("262144", "262 144"),
        /// 524 288.
        S512k = ("524288", "524 288"),
        /// 1 048 576.
        S1m = ("1048576", "1 048 576"),
        /// 2 097 152.
        S2m = ("2097152", "2 097 152"),
    }
}

impl IqFormat {
    /// How one sample of this format is stored in the ring.
    pub fn sample_type(self) -> SampleType {
        match self {
            IqFormat::Cs16 => SampleType::Cs16,
            IqFormat::Cs8 => SampleType::Cs8,
        }
    }
}

impl BufferSize {
    /// The samples per transfer. `None` for [`BufferSize::Auto`].
    pub fn samples(self) -> Option<usize> {
        match self {
            BufferSize::Auto => None,
            BufferSize::S64k => Some(1 << 16),
            BufferSize::S128k => Some(1 << 17),
            BufferSize::S256k => Some(1 << 18),
            BufferSize::S512k => Some(1 << 19),
            BufferSize::S1m => Some(1 << 20),
            BufferSize::S2m => Some(1 << 21),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The protocol of the sample stream ([`PROTOCOL`]).
    pub enum StreamProtocol {
        /// The binary protocol if the server offers it, else the text protocol.
        Auto = ("auto", "Auto"),
        /// The text protocol only.
        Text = ("text", "Text only"),
    }
}

sdrkit_core::choice_enum! {
    /// The sample format of Tezuka firmware ([`IQ_FORMAT`]).
    pub enum IqFormat {
        /// 16-bit.
        Cs16 = ("cs16", "CS16"),
        /// 8-bit, half the data.
        Cs8 = ("cs8", "CS8"),
    }
}

/// The frequency correction.
pub const PPM: Setting<field::F64> = crate::form::PPM
    .range(-200.0, 200.0, 0.01)
    .help("The clock error in ppm, positive if the clock runs fast. Corrects tuning and sample rate together.");
/// Receive below 2.1 Msps through the hardware FIR.
pub const LOW_RATE_FIR: Setting<field::Bool> = Setting::new("lowratefir", "Low-rate FIR (RX only)", "", false)
    .section(crate::form::RATE_SECTION)
    .restart()
    .help(
        "Receive below 2.1 Msps (down to 0.521 Msps, the 2.048 Msps of DAB among them) through the hardware FIR, RX only. Off: \
         at least 2.1 Msps, resampled in software. The AD936x loads its receive and transmit filter taps from one file, so this \
         also replaces the transmit filter taps until the radio restarts, and some boards switch both filters on together. \
         Nothing is transmitted, but leave it off on a radio you also transmit with.",
    );
/// The gain mode of the AD936x.
pub const GAIN_MODE: Setting<field::Choice<GainMode>> = crate::form::agc_mode(GainMode::Manual)
    .help(
        "Manual: the gain setting sets the gain. With automatic gain on, the driver chooses the gain from the level of the samples \
         (the peaks 10 dB below full scale), holds it, and moves it only when the level stays more than 6 dB off for half a \
         second, after a retune, or when the samples clip. The AGC choices give the gain to the AD936x, and the gain setting does \
         nothing: its AGC can step the gain while a signal is on, and each step turns the phase of the signal, which breaks the \
         reception of OFDM signals such as DVB-T. Hybrid: the AGC holds the gain until told to change it.",
    );
/// The receiver of one stream.
pub const ANTENNA: Setting<field::Choice<Antenna>> =
    crate::form::antenna(Antenna::Rx1)
        .restart()
        .help("RX2 needs two receivers.");
/// How many receivers stream.
pub const RECEIVERS: Setting<field::Choice<Receivers>> = Setting::choice("receivers", "Receivers", Receivers::One)
    .section(crate::form::ANTENNA_SECTION)
    .restart()
    .help(
        "Both receivers at once, on one clock and one LO (AD9361 boards: ANTSDR E310, Pluto+). Two streams: RX1 in the first, RX2 \
         in the second. Interleaved: one stream of RX1, RX2, RX1... at twice the rate. Twice the data on the link.",
    );
/// The gain mode of RX2.
pub const GAIN_MODE2: Setting<field::Choice<Rx2GainMode>> =
    Setting::choice("gainmode2", "RX2 AGC mode", Rx2GainMode::Same)
        .section(crate::form::GAIN_SECTION)
        .help("With both receivers: the gain mode of RX2, or the same as RX1.");
/// The gain of RX2.
pub const GAIN2: Setting<field::F64> = Setting::new("gain2", "RX2 gain", "dB", 30.0)
    .range(0.0, 77.0, 1.0)
    .section(crate::form::GAIN_SECTION)
    .help("With both receivers and RX2 in manual gain: its gain.");
/// The input of the receivers.
pub const RX_PORT: Setting<field::Choice<RxPort>> = Setting::choice("rxport", "RX port", RxPort::Auto)
    .section(crate::form::ANTENNA_SECTION)
    .help("The AD936x input that the receivers use. Most boards wire only A_BALANCED. Some switch bands with B and C.");
/// Quadrature tracking.
pub const QUAD_TRACK: Setting<field::Bool> = Setting::new("quadtrack", "Quadrature tracking", "", true)
    .section(crate::form::CORRECTIONS_SECTION)
    .help("The AD936x keeps correcting the I/Q balance. Turn it off only for measurements that need it untouched.");
/// RF DC offset tracking.
pub const RF_DC_TRACK: Setting<field::Bool> = Setting::new("rfdctrack", "RF DC tracking", "", true)
    .section(crate::form::CORRECTIONS_SECTION)
    .help("The AD936x keeps removing the DC offset of the RF stage.");
/// Baseband DC offset tracking.
pub const BB_DC_TRACK: Setting<field::Bool> = Setting::new("bbdctrack", "Baseband DC tracking", "", true)
    .section(crate::form::CORRECTIONS_SECTION)
    .help("The AD936x keeps removing the DC offset of the baseband. Off keeps a carrier at the centre frequency.");
/// Sample buffers in flight on the board.
pub const BUFFERS: Setting<field::I64> = crate::form::buffers(8, 2.0, 64.0)
    .restart()
    .help("Sample buffers that the board keeps in flight. More ride out a slow link or a busy board, at the cost of latency.");
/// Samples per transfer.
pub const BUFFER_SIZE: Setting<field::Choice<BufferSize>> = crate::form::buffer_size(BufferSize::Auto)
    .restart()
    .help("Samples per transfer. Auto holds at least 25 ms, which keeps high rates flowing over a network.");
/// The protocol of the sample stream.
pub const PROTOCOL: Setting<field::Choice<StreamProtocol>> = crate::form::protocol(StreamProtocol::Auto)
    .restart()
    .help("Auto streams over the binary protocol if the server of the board offers it (libiio 1.x), else the text protocol.");
/// What carries the samples.
pub const LINK: Setting<field::Choice<LinkKind>> = Setting::choice("link", "Link", LinkKind::Auto)
    .section(crate::form::LINK_SECTION)
    .help(
    "Auto takes the default address as the network of the USB cable (about 3 Msps without loss). Ethernet means a Pluto+ or a \
     USB Ethernet adapter, with no cap. USB holds the USB rate.",
);
/// The sample format of Tezuka firmware.
pub const IQ_FORMAT: Setting<field::Choice<IqFormat>> = crate::form::format(IqFormat::Cs16)
    .restart()
    .help(
        "CS16 is 16-bit (the 12-bit converter). CS8 is half the data, so a higher rate over USB or a slow network, with less \
         dynamic range. Tezuka firmware only.",
    );

/// The time between two passes of the readings monitor.
pub const READINGS_EVERY: Setting<field::F64> = Setting::new("readings_s", "Readings every", "s", 5.0)
    .range(0.0, 60.0, 0.5)
    .section(crate::form::LINK_SECTION)
    .help("How often the temperature, the RSSI, the gain and the state of the board are read, on a connection of their own. 0 reads none.");

sdrkit_core::settings! {
    /// The own settings of the iiod driver. A board that lacks a part refuses the setting when it is applied.
    pub enum IiodSetting {
        /// [`GAIN_MODE`].
        GainMode = GAIN_MODE,
        /// [`GAIN_MODE2`].
        GainMode2 = GAIN_MODE2,
        /// [`GAIN2`].
        Gain2 = GAIN2,
        /// [`LOW_RATE_FIR`].
        LowRateFir = LOW_RATE_FIR,
        /// [`PPM`].
        Ppm = PPM,
        /// [`QUAD_TRACK`].
        QuadTrack = QUAD_TRACK,
        /// [`RF_DC_TRACK`].
        RfDcTrack = RF_DC_TRACK,
        /// [`BB_DC_TRACK`].
        BbDcTrack = BB_DC_TRACK,
        /// [`ANTENNA`].
        Antenna = ANTENNA,
        /// [`RECEIVERS`].
        Receivers = RECEIVERS,
        /// [`RX_PORT`].
        RxPort = RX_PORT,
        /// [`BUFFERS`].
        Buffers = BUFFERS,
        /// [`BUFFER_SIZE`].
        BufferSize = BUFFER_SIZE,
        /// [`PROTOCOL`].
        Protocol = PROTOCOL,
        /// [`LINK`].
        Link = LINK,
        /// [`IQ_FORMAT`].
        IqFormat = IQ_FORMAT,
        /// [`READINGS_EVERY`].
        ReadingsEvery = READINGS_EVERY,
    }
}

/// The tracking switches: the setting and the attribute.
const TRACKING: [(Setting<field::Bool>, &str); 3] = [
    (QUAD_TRACK, "quadrature_tracking_en"),
    (RF_DC_TRACK, "rf_dc_offset_tracking_en"),
    (BB_DC_TRACK, "bb_dc_offset_tracking_en"),
];

/// The sample buffer size for a rate over the network: the smallest power of two holding at least 25 ms, from 65 536 to 1 048 576.
pub fn buffer_samples(rate_hz: Hz) -> usize {
    ((rate_hz * 0.025).ceil().max(1.0) as usize)
        .next_power_of_two()
        .clamp(SMALL_BUFFER, MAX_BUFFER)
}

/// The rate to ask for: [`RATE`] as it is if [`RATE_EXACT`] is on. Else the rate of the stack (2.5 Msps if none), kept as it is
/// from 0.521 to 2.1 Msps if the low-rate FIR is on, else held to 2.1 to 61.44 Msps.
pub fn rate_wanted(v: &Values, lowratefir: bool) -> Hz {
    let asked = v.get(&RATE);
    if v.get(&RATE_EXACT) && asked > 0.0 {
        return asked;
    }
    let r = if asked > 0.0 { asked } else { DEFAULT_RATE };
    if lowratefir && (FIR_MIN_RATE - 1.0..MIN_RATE).contains(&r) {
        r
    } else {
        r.clamp(MIN_RATE, MAX_RATE)
    }
}

/// How the gain of one receiver is set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GainPlan {
    /// The AD936x in manual gain at this gain, in dB.
    Manual(f64),
    /// The AD936x in manual gain, at the gain that the held gain of the driver ([`crate::gain`]) chooses.
    Held,
    /// The AGC of the AD936x in this mode.
    Chip(GainMode),
}

/// The plan for gain mode `mode`, automatic gain `agc` and gain `gain`: manual with automatic gain on is the held gain of the
/// driver. A manual mode with no gain written would keep the gain that the last program left.
pub fn gain_plan(mode: GainMode, agc: bool, gain: f64) -> GainPlan {
    match (mode, agc) {
        (GainMode::Manual, true) => GainPlan::Held,
        (GainMode::Manual, false) => GainPlan::Manual(gain),
        (m, _) => GainPlan::Chip(m),
    }
}

/// How far below the top of the band's range the held gain starts after a retune, in dB.
pub const HELD_START_BELOW_MAX_DB: f64 = 30.0;

/// The gain that the held gain starts from at `lo`: [`HELD_START_BELOW_MAX_DB`] below the top of the band
/// ([`default_max_gain`]) or of `range`, whichever is lower, within `range`.
pub fn held_start_gain(lo: Hz, range: (f64, f64)) -> f64 {
    let top = range.1.min(default_max_gain(lo));
    (top - HELD_START_BELOW_MAX_DB)
        .clamp(range.0.max(0.0), range.1.max(0.0))
        .round()
}

/// The FIR decimation a rate needs: 2 from 1.042 Msps, 4 from 0.521 Msps, none at 2.1 Msps and above (or below 0.521 Msps).
pub fn fir_decimation(rate_hz: Hz) -> Option<u32> {
    if rate_hz >= MIN_RATE || rate_hz < FIR_MIN_RATE - 1.0 {
        None
    } else if rate_hz * 2.0 >= CONVERTER_MIN - 1.0 {
        Some(2)
    } else {
        Some(4)
    }
}

/// The narrowest and the widest analogue filter of an AD936x, receive and transmit.
pub const FILTER_WIDTH: (Hz, Hz) = (0.2e6, 56e6);
/// The sample rates of an AD936x that people normally use. An open radio offers those inside its rate range.
pub const COMMON_RATES: &[Hz] = &[
    2.1e6, 2.5e6, 3e6, 4e6, 5e6, 6e6, 8e6, 10e6, 12.5e6, 15e6, 20e6,
];
/// The widths of the analogue filter of an AD936x that people normally use.
pub const COMMON_WIDTHS: &[Hz] = &[0.2e6, 0.5e6, 1e6, 2e6, 5e6, 10e6, 20e6, 40e6, 56e6];

/// The analogue filter: `bandwidth_hz` if above 0, else 0.95 times the rate, held to [`FILTER_WIDTH`], whole Hz.
pub fn rf_bandwidth(bandwidth_hz: Hz, rate_hz: Hz) -> Hz {
    let b = if bandwidth_hz > 0.0 {
        bandwidth_hz
    } else {
        0.95 * rate_hz
    };
    b.clamp(FILTER_WIDTH.0, FILTER_WIDTH.1).round()
}

/// The hardware gain limits when `hardwaregain_available` cannot be read.
pub fn default_max_gain(freq_hz: Hz) -> f64 {
    if freq_hz < 1.3e9 {
        73.0
    } else if freq_hz < 4e9 {
        71.0
    } else {
        62.0
    }
}

/// The network address of a Pluto on its USB cable, where [`LINK`] applies.
pub fn is_default_address(host: &str) -> bool {
    host == "192.168.2.1" || host.eq_ignore_ascii_case("pluto.local")
}

/// The host and the port of an iiod address. Only network addresses work.
///
/// # Errors
///
/// [`DriverError::Address`] for a `usb:` or `local:` address, or a text that is not a network address.
pub fn parse_address(address: &str) -> Result<(String, u16), DriverError> {
    let a = address.trim();
    let lower = a.to_ascii_lowercase();
    if lower.starts_with("usb:") || lower.starts_with("local:") {
        return Err(DriverError::Address);
    }
    let a = a.strip_prefix("ip:").unwrap_or(a);
    host_port(a, Some(IIOD_PORT))
        .map(|(h, p)| (h.to_owned(), p))
        .map_err(|_| DriverError::Address)
}

/// The note for a network that cannot carry the stream: `bytes_per_s` of samples above what gigabit Ethernet carries.
pub fn network_note(rate_hz: Hz, receivers: usize, bytes_per_s: f64) -> Option<String> {
    (bytes_per_s > GIGABIT_BYTES).then(|| {
        let per = GIGABIT_BYTES / (bytes_per_s / rate_hz);
        format!(
            "{} Msps from {} carries {:.0} Mbit/s of samples, more than gigabit Ethernet's about 940 Mbit/s: samples will be lost. \
             Gigabit carries about {} Msps{}.",
            fmt_m(rate_hz),
            if receivers > 1 { "both receivers" } else { "one receiver" },
            bytes_per_s * 8e-6,
            fmt_m(per),
            if receivers > 1 { " with both receivers" } else { "" }
        )
    })
}

/// What the driver has learnt about the boards it opened, per address.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Known {
    /// Tezuka firmware (the IQ format setting).
    pub tezuka: bool,
    /// Two receivers.
    pub two_rx: bool,
    /// The receive ports the board lists.
    pub rx_ports: Vec<String>,
    /// The gain modes the board lists.
    pub gain_modes: Vec<String>,
    /// What the link to the board carried in the last reading of a running stream. It counts only while it is fresh
    /// ([`LiveLink::fresh`]): it is a figure of the moment, never a limit that outlives it.
    pub link: Option<LiveLink>,
    /// When a stream of this program from the board last ended: a buffer still enabled soon after is ours, on its way out.
    pub stream_ended: Option<Instant>,
}

/// A reading of what the link to a board carries, from a running stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiveLink {
    /// Bytes a second, smoothed over the last readings.
    pub bps: f64,
    /// When it was read.
    pub at: Instant,
}

impl LiveLink {
    /// The figure, if it was read less than [`LIVE_FRESH`] ago.
    pub fn fresh(&self) -> Option<f64> {
        (self.at.elapsed() < LIVE_FRESH).then_some(self.bps)
    }
}

impl Receivers {
    /// The ring layout.
    pub fn layout(&self) -> StreamLayout {
        match self {
            Receivers::One => StreamLayout::Single,
            Receivers::Both => StreamLayout::Separate(2),
            Receivers::Interleaved => StreamLayout::Interleaved(2),
        }
    }
    fn count(&self) -> usize {
        if *self == Receivers::One {
            1
        } else {
            2
        }
    }
}

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "iiod",
    name: "PlutoSDR / ANTSDR (network)",
    opens: crate::Opens::Once,
    addresses: &[
        "iiod:<host> (port 30431)",
        "iiod:<host>:<port>",
        "iiod:ip:<host>",
    ],
    address: &[
        crate::AddressPart::host(true),
        crate::AddressPart::port(Some(IIOD_PORT)),
    ],
    settings: &[IiodSetting::SETTINGS],
    commands: &[],
    readings: source::READINGS,
    produces: &[
        StreamFormat {
            kind: Kind::Iq,
            sample: SampleType::Cs16,
            min_rate_hz: FIR_MIN_RATE,
            max_rate_hz: MAX_RATE,
            channels: 1,
        },
        StreamFormat {
            kind: Kind::Iq,
            sample: SampleType::Cs8,
            min_rate_hz: FIR_MIN_RATE,
            max_rate_hz: MAX_RATE,
            channels: 1,
        },
    ],
    accepts: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cf32,
        min_rate_hz: MIN_RATE,
        max_rate_hz: MAX_RATE,
        channels: 1,
    }],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The iiod driver. It remembers what it learnt about each address (firmware, receivers, ports).
#[derive(Clone)]
pub struct IiodDriver {
    transport: Arc<dyn Transport>,
    known: Arc<Mutex<HashMap<String, Known>>>,
}

impl IiodDriver {
    /// A driver that reaches its boards through `transport`.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        IiodDriver {
            transport,
            known: Arc::default(),
        }
    }
}

impl SourceDriver for IiodDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }
    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        Ok(Box::new(IiodSource::open(
            self.transport.clone(),
            address,
            values,
            self.known.clone(),
        )?))
    }
}

/// What the context says about the board, worked out once at connect.
#[derive(Clone, Debug, Default)]
pub struct BoardFacts {
    /// The phy's device id.
    pub phy: String,
    /// The receive stream device.
    pub rx: Device,
    /// The receive stream device's number in the context (the binary protocol).
    pub rx_index: usize,
    /// The transmit stream device, when the board has one.
    pub tx: Option<Device>,
    /// The phy device (its channels and attributes).
    pub phy_dev: Device,
    /// Receivers: 1 or 2.
    pub receivers: usize,
    /// Transmitters: 0, 1 or 2.
    pub transmitters: usize,
    /// "AD9361", "AD9363", "AD9364" (a best guess from the context).
    pub chip: String,
    /// The board's name (`hw_model`, else a name from the address).
    pub name: String,
    /// The firmware version.
    pub firmware: String,
    /// The iiod server version (major, minor), when it said.
    pub server: Option<(u32, u32)>,
}

impl BoardFacts {
    /// From the context XML.
    ///
    /// # Errors
    ///
    /// [`Protocol::MissingPart`] if the AD9361 devices or the receive channels are not in the context.
    pub fn from_context(ctx: &Context, label: &str) -> Result<BoardFacts, DriverError> {
        let missing = DriverError::Protocol(Protocol::MissingPart);
        let (Some(phy), Some(rx)) = (ctx.device(PHY), ctx.device(RX_DEV)) else {
            return Err(missing);
        };
        let has = |dev: &Device, id: &str, out: bool| dev.channel(id, out).is_some();
        if !(has(phy, "voltage0", false)
            && has(rx, "voltage0", false)
            && has(rx, "voltage1", false))
        {
            return Err(missing);
        }
        let receivers = if has(phy, "voltage1", false)
            && has(rx, "voltage2", false)
            && has(rx, "voltage3", false)
        {
            2
        } else {
            1
        };
        let tx = ctx.device(TX_DEV).cloned();
        let transmitters = match &tx {
            Some(d)
                if has(phy, "voltage0", true)
                    && d.channel("voltage0", true).is_some()
                    && d.channel("voltage1", true).is_some() =>
            {
                if has(phy, "voltage1", true)
                    && d.channel("voltage2", true).is_some()
                    && d.channel("voltage3", true).is_some()
                {
                    2
                } else {
                    1
                }
            }
            _ => 0,
        };
        let model = ctx
            .attrs
            .iter()
            .find(|(k, _)| k.ends_with(",model") || k.as_str() == "ad9361-phy,model")
            .map(|(_, v)| v.to_ascii_uppercase());
        let chip = match model {
            Some(m) if m.contains("AD936") => m,
            _ if receivers == 2 => "AD9361".into(),
            _ => "AD9363/AD9364".into(),
        };
        Ok(BoardFacts {
            phy: phy.id.clone(),
            rx_index: ctx.device_index(&rx.id).unwrap_or(0),
            rx: rx.clone(),
            tx,
            phy_dev: phy.clone(),
            receivers,
            transmitters,
            chip,
            name: ctx
                .attrs
                .get("hw_model")
                .cloned()
                .unwrap_or_else(|| format!("PlutoSDR at {label}")),
            firmware: ctx.attrs.get("fw_version").cloned().unwrap_or_default(),
            server: None,
        })
    }

    /// The highest rate: 30.72 Msps on a board running two receivers (2R2T, checked on an ANTSDR E310: 40 Msps is refused even
    /// with one receiver streaming), else 61.44 Msps.
    pub fn max_rate(&self) -> Hz {
        if self.receivers == 2 {
            MAX_RATE_2R2T
        } else {
            MAX_RATE
        }
    }

    /// The phy channel has this attribute.
    pub fn phy_has(&self, chan: &str, output: bool, attr: &str) -> bool {
        self.phy_dev
            .channel(chan, output)
            .is_some_and(|c| c.has(attr))
    }
}

/// What the receiver and the transmitter of one board share: the clock (rate and PPM) and the state of each side.
#[derive(Debug, Default)]
pub struct BoardState {
    /// The facts, once connected.
    pub facts: Option<BoardFacts>,
    /// The receiver's rate (0 before the first configure).
    pub rx_rate: Hz,
    /// The receiver is streaming.
    pub rx_streaming: bool,
    /// The receiver uses the low-rate FIR.
    pub rx_fir: bool,
    /// The receiver streams CS8.
    pub rx_cs8: bool,
    /// The board runs full duplex (`ensm_mode` reads `fdd`), when known.
    pub fdd: Option<bool>,
    /// A transmitter is open.
    pub tx_open: bool,
    /// The transmitter is on air.
    pub tx_armed: bool,
    /// The transmitter's rate while it is on air.
    pub tx_rate: Hz,
    /// The readings the monitor last made.
    pub readings: source::Readings,
    /// While the stream gets clearly less than its rate: the part of the rate that arrives, and the rate.
    pub link_short: Option<(f64, Hz)>,
    /// The held gain of RX1 and RX2 (by phy channel), when it runs.
    pub gain_hold: [Option<HoldStatus>; 2],
    /// The overflows of the capture buffer of the board that the monitor counted since the board was opened.
    pub overflows: Option<u64>,
}

/// The shared board state.
pub type Board = Arc<Mutex<BoardState>>;

/// The second receiver's ring when the receivers stream as two rings; the stream takes it while it runs and returns it.
pub type AuxRing = Arc<Mutex<Option<RingWriter>>>;

/// One open AD936x board, receive side.
pub struct IiodRadio {
    transport: Arc<dyn Transport>,
    host: String,
    port: u16,
    label: String,
    ctl: IiodConn,
    notices: Notices,
    facts: BoardFacts,
    board: Board,
    aux: AuxRing,
    /// The phy channels of the receivers in use (`voltage0`, `voltage1`).
    phy_chans: Vec<&'static str>,
    /// The stream channels (positions) of each receiver's I and Q.
    want: Vec<(usize, usize)>,
    mask: Vec<u32>,
    receivers: Receivers,
    cs8: bool,
    tezuka: bool,
    default_address: bool,
    xo_base: Option<f64>,
    ppm_in_effect: f64,
    ppm_written: Option<f64>,
    lo_range: (Hz, Hz),
    /// The limits of the transmitter with one output in use, read at open. `None` for a board without a transmitter.
    tx_limits: Option<TxLimits>,
    lo_written: Option<Hz>,
    rate: Hz,
    fir_dec: Option<u32>,
    /// The FIR was switched on through the `out` channel (receive and transmit filters together).
    fir_both: bool,
    lowratefir: bool,
    gain_modes: Vec<String>,
    gain_mode: Vec<Option<GainMode>>,
    gain_range: Vec<Option<(f64, f64)>>,
    gain_written: Vec<Option<f64>>,
    /// The held gain of each receiver in use, shared with the stream.
    holds: Vec<Arc<Mutex<HeldGain>>>,
    rx_ports: Vec<String>,
    port_written: Option<RxPort>,
    tracking_written: [Option<bool>; 3],
    kbuffers: u32,
    bufsize: Option<usize>,
    text_only: bool,
    binary_ok: Option<bool>,
    /// A stream was opened before on this connection (so a buffer still enabled may be our own, on its way out).
    streamed: bool,
    centre: Hz,
    link: LinkKind,
    closed: bool,
    /// What the driver learnt per address; the link measurement of this board is kept there.
    known: Arc<Mutex<HashMap<String, Known>>>,
}

/// The error of a device that refused the value of setting `id`. A lost connection stays a network error.
fn refused(id: SettingId, e: IioError) -> DriverError {
    match e {
        IioError::Errno(_) => DriverError::Refused { id },
        other => other.into(),
    }
}

/// A control connection, the context it read, and the version of the server.
pub(crate) type Connected = (IiodConn, Context, Option<(u32, u32)>);

/// Where a board is, and what reaches it.
#[derive(Clone)]
pub(crate) struct Address {
    pub(crate) transport: Arc<dyn Transport>,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) label: String,
}

/// Connects a control connection and reads the context: (connection, context, server version).
pub(crate) fn connect_ctl(at: &Address, abort: &Arc<Abort>) -> Result<Connected, DriverError> {
    let mut ctl = IiodConn::connect(
        &at.transport,
        &at.host,
        at.port,
        Duration::from_secs(8),
        abort,
    )?;
    ctl.timeout(CONTEXT_TIMEOUT_MS)?;
    let version = ctl.version().ok().and_then(|v| proto::parse_version(&v));
    let xml = ctl.print()?;
    let ctx = Context::parse(&xml).map_err(|_| DriverError::Protocol(Protocol::Description))?;
    Ok((ctl, ctx, version))
}

fn list(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

/// Refuses RX2 or both receivers on a board with one (`board_receivers`), and CS8 on firmware other than Tezuka or with
/// both receivers.
fn check_receivers(v: &Values, board_receivers: usize, tezuka: bool) -> Result<(), DriverError> {
    let receivers = v.get(&RECEIVERS);
    let refused = |id| Err(DriverError::Refused { id: own_id(id) });
    if board_receivers < 2 {
        if v.get(&ANTENNA) == Antenna::Rx2 {
            return refused(IiodSetting::Antenna);
        }
        if receivers != Receivers::One {
            return refused(IiodSetting::Receivers);
        }
    }
    if v.get(&IQ_FORMAT) == IqFormat::Cs8 && (!tezuka || receivers != Receivers::One) {
        return refused(IiodSetting::IqFormat);
    }
    Ok(())
}

impl IiodRadio {
    pub(crate) fn connect(
        at: &Address,
        known: &Arc<Mutex<HashMap<String, Known>>>,
        board: &Board,
        aux: &AuxRing,
        v: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<IiodRadio, DriverError> {
        let (host, port, label) = (at.host.as_str(), at.port, at.label.as_str());
        let (mut ctl, ctx, version) = connect_ctl(at, abort)?;
        let mut facts = BoardFacts::from_context(&ctx, label)?;
        facts.server = version;
        let phy_id = facts.phy.clone();
        // The clock base, for PPM.
        let xo_base = ctl
            .read_attr(&phy_id, Target::Device, "xo_correction")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| *v > 1e6);
        // Firmware.
        let fw = facts.firmware.clone();
        let tezuka = fw.to_ascii_lowercase().contains("tezuka");
        check_receivers(v, facts.receivers, tezuka)?;
        let receivers = v.get(&RECEIVERS);
        let rx2 = v.get(&ANTENNA) == Antenna::Rx2;
        let cs8 = v.get(&IQ_FORMAT) == IqFormat::Cs8;
        let pos = |id: &str| facts.rx.channel_index(id, false).unwrap_or(0);
        let (rx1, rx2p) = (
            (pos("voltage0"), pos("voltage1")),
            (pos("voltage2"), pos("voltage3")),
        );
        let (phy_chans, want): (Vec<&'static str>, Vec<(usize, usize)>) = match (receivers, rx2) {
            (Receivers::One, false) => (vec!["voltage0"], vec![rx1]),
            (Receivers::One, true) => (vec!["voltage1"], vec![rx2p]),
            _ => (vec!["voltage0", "voltage1"], vec![rx1, rx2p]),
        };
        let want: Vec<(usize, usize)> = if cs8 {
            want.iter().map(|&(i, _)| (i, i)).collect()
        } else {
            want
        };
        let bits: Vec<usize> = want.iter().flat_map(|&(i, q)| [i, q]).collect();
        let mask = mask_of(&bits, facts.rx.channels.len());
        // The LO range.
        let lo_range = ctl
            .read_attr(&phy_id, Target::Channel(true, RX_LO), "frequency_available")
            .ok()
            .and_then(|s| parse_range(&s))
            .map(|(a, _, b)| (a, b))
            .unwrap_or((70e6, 6e9));
        let tx_limits = (facts.transmitters > 0).then(|| tx::read_limits(&mut ctl, &phy_id, 1));
        let read_list = |ctl: &mut IiodConn, attr: &str| {
            if facts.phy_has("voltage0", false, attr) {
                ctl.read_attr(&phy_id, Target::Channel(false, "voltage0"), attr)
                    .map(|s| list(&s))
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        };
        let rx_ports = read_list(&mut ctl, "rf_port_select_available");
        let gain_modes = read_list(&mut ctl, "gain_control_mode_available");
        let fdd = ctl
            .read_attr(&phy_id, Target::Device, "ensm_mode")
            .ok()
            .map(|m| m == "fdd");
        {
            let mut k = lock(known);
            let e = k.entry(label.to_string()).or_default();
            e.tezuka = tezuka;
            e.two_rx = facts.receivers == 2;
            e.rx_ports.clone_from(&rx_ports);
            e.gain_modes.clone_from(&gain_modes);
        }
        notices.log(format!(
            "PlutoSDR {label}: {} ({}, {} RX, {} TX), firmware {fw}, iiod {}",
            facts.name,
            facts.chip,
            facts.receivers,
            facts.transmitters,
            version
                .map(|(a, b)| format!("{a}.{b}"))
                .unwrap_or_else(|| "?".into())
        ));
        {
            let mut b = lock(board);
            b.facts = Some(facts.clone());
            b.fdd = fdd;
            b.rx_cs8 = cs8;
        }
        let n = phy_chans.len();
        let bufsize = v.get(&BUFFER_SIZE).samples();
        Ok(IiodRadio {
            transport: at.transport.clone(),
            host: host.to_string(),
            port,
            label: label.to_string(),
            ctl,
            notices: notices.clone(),
            facts,
            board: board.clone(),
            aux: aux.clone(),
            phy_chans,
            want,
            mask,
            receivers,
            cs8,
            tezuka,
            default_address: is_default_address(host),
            xo_base,
            ppm_in_effect: 0.0,
            ppm_written: None,
            lo_range,
            tx_limits,
            lo_written: None,
            rate: 0.0,
            fir_dec: None,
            fir_both: false,
            lowratefir: v.get(&LOW_RATE_FIR),
            gain_modes,
            gain_mode: vec![None; n],
            gain_range: vec![None; n],
            gain_written: vec![None; n],
            holds: (0..n).map(|_| Arc::default()).collect(),
            rx_ports,
            port_written: None,
            tracking_written: [None; 3],
            kbuffers: u32::try_from(v.get(&BUFFERS).clamp(2, 64)).unwrap_or(8),
            bufsize,
            text_only: v.get(&PROTOCOL) == StreamProtocol::Text,
            binary_ok: None,
            streamed: false,
            centre: v.get(&CENTRE),
            link: v.get(&LINK),
            closed: false,
            known: known.clone(),
        })
    }

    fn chan_attr(&mut self, chan: &str, attr: &str, value: &str) -> Result<i64, IioError> {
        let phy = self.facts.phy.clone();
        self.ctl
            .write_attr(&phy, Target::Channel(false, chan), attr, value)
    }

    /// Writes an attribute of the first receiver's phy channel (the shared ones: rate, filter, FIR, tracking).
    fn rx_attr(&mut self, attr: &str, value: &str) -> Result<i64, IioError> {
        let ch = self.phy_chans[0];
        self.chan_attr(ch, attr, value)
    }

    /// The rate the link carries without loss (0: no cap): the USB rule at the default address, else the measured link.
    fn link_cap(&self) -> Hz {
        if self.default_address {
            return rate::link_cap(None, true, self.link, self.cs8);
        }
        self.measured_cap().unwrap_or(0.0)
    }

    /// The bytes of one sample instant of the receivers in use.
    fn instant_bytes(&self) -> usize {
        self.receivers.count() * if self.cs8 { 2 } else { 4 }
    }

    /// The rate that the link carries now with room to spare, for a board across a network, from a fresh reading of a running
    /// stream ([`LiveLink::fresh`]). `None` without one and on the loopback.
    fn measured_cap(&self) -> Option<Hz> {
        if is_local(&self.host) {
            return None;
        }
        let bps = lock(&self.known).get(&self.label)?.link?.fresh()?;
        Some(LINK_SHARE * bps / self.instant_bytes() as f64)
    }

    /// Turns the low-rate FIR on for `rate`, decimating by `dec`. On a refusal the caller turns it off again.
    fn fir_enable(&mut self, rate: Hz, dec: u32) -> Result<(), IioError> {
        self.rx_attr(
            "sampling_frequency",
            &format!("{}", (f64::from(dec) * rate).round()),
        )?;
        let phy = self.facts.phy.clone();
        self.ctl.write_attr(
            &phy,
            Target::Device,
            "filter_fir_config",
            &fir::config_text(dec),
        )?;
        if let Err(e) = self.rx_attr("filter_fir_en", "1") {
            // At low rates the AD936x driver requires both filters to be enabled together (the clock chain is shared): the `out`
            // channel's switch does that. The transmit filter only shapes transmit samples; transmit itself stays off.
            if !self.facts.phy_has("out", false, "voltage_filter_fir_en") {
                return Err(e);
            }
            self.chan_attr("out", "voltage_filter_fir_en", "1")?;
            self.fir_both = true;
        }
        self.fir_dec = Some(dec);
        let low = format!("{}", rate.round());
        if let Err(e) = self.rx_attr("sampling_frequency", &low) {
            // Some boards (an ANTSDR E310 with an AD9361) take the receive filter alone but then refuse a rate below the
            // converters' lowest: with both filters on (as libad9361 switches them) the rate is accepted.
            if self.fir_both || !self.facts.phy_has("out", false, "voltage_filter_fir_en") {
                return Err(e);
            }
            self.chan_attr("out", "voltage_filter_fir_en", "1")?;
            self.fir_both = true;
            self.rx_attr("sampling_frequency", &low)?;
        }
        Ok(())
    }

    /// Turns the FIR off (the way it was turned on).
    fn fir_off(&mut self) {
        if std::mem::take(&mut self.fir_both) {
            let _ = self.chan_attr("out", "voltage_filter_fir_en", "0");
        } else {
            let _ = self.rx_attr("filter_fir_en", "0");
        }
        self.fir_dec = None;
    }

    /// A number attribute of the first receiver's phy channel as the radio has it now.
    fn rx_now(&mut self, attr: &str) -> Option<f64> {
        let phy = self.facts.phy.clone();
        self.ctl
            .read_attr(&phy, Target::Channel(false, self.phy_chans[0]), attr)
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    fn configure_rate(&mut self, v: &Values, notes: &mut Vec<String>) -> Result<(), DriverError> {
        let now = self.rx_now("sampling_frequency");
        let mut want = match now {
            // No stack has set a rate: the radio keeps its own, if the converters give it without the FIR.
            Some(r) if v.get(&RATE) <= 0.0 && (MIN_RATE..=IDLE_KEEP_MAX).contains(&r) => r,
            _ => rate_wanted(v, self.lowratefir),
        };
        let max = self.facts.max_rate();
        if !v.get(&RATE_EXACT) && want > max {
            notes.push(format!(
                "this board runs at most {} Msps (two receivers, 2R2T)",
                fmt_m(max)
            ));
            want = max;
        }
        let (tx_armed, tx_rate) = {
            let b = lock(&self.board);
            (b.tx_armed, b.tx_rate)
        };
        if tx_armed && tx_rate > 0.0 {
            if (want - tx_rate).abs() > 1.0 {
                notes.push(format!(
                    "the transmitter holds the shared rate at {} Msps",
                    fmt_m(tx_rate)
                ));
            }
            want = tx_rate;
        }
        if self.fir_dec.is_some() {
            self.fir_off();
        }
        // A rate below what the converters run at without the FIR (a rate the user chose, such as 0.5 or 1 Msps) needs the
        // decimating FIR: it is switched on for this run even when the setting is off, as the radio would refuse the rate.
        if want < FIR_MIN_RATE - 1.0 {
            // Below what the converters give even with the FIR decimating by 4: the nearest rate the radio has.
            notes.push(format!(
                "{} Msps is below the {} Msps an AD936x delivers: it runs at {} Msps",
                fmt_m(want),
                fmt_m(FIR_MIN_RATE.ceil()),
                fmt_m(FIR_MIN_RATE.ceil())
            ));
            want = FIR_MIN_RATE.ceil();
        }
        let needs_fir = want < NO_FIR_MIN_RATE - 1.0 && fir_decimation(want).is_some();
        if needs_fir && !self.lowratefir {
            if tx_armed {
                // The low-rate FIR cannot be switched on while the transmitter is on air.
                return Err(DriverError::NotNow {
                    state: crate::Doing::Transmitting,
                });
            }
            notes.push(format!(
                "{} Msps: the low-rate FIR is on for this rate (receive only)",
                fmt_m(want)
            ));
        }
        if (self.lowratefir || needs_fir) && !tx_armed {
            if let Some(dec) = fir_decimation(want) {
                if let Err(e) = self.fir_enable(want, dec) {
                    self.fir_off();
                    self.notices.log(format!(
                        "the radio did not accept the decimating FIR ({e}): running at 2.1 Msps, resampled in software"
                    ));
                    want = MIN_RATE;
                }
            }
        }
        // A write of the rate the radio already has still costs the radio's time.
        let same = now.is_some_and(|r| (r - want.round()).abs() < 0.5);
        if self.fir_dec.is_none() && !tx_armed && !same {
            self.rx_attr("sampling_frequency", &format!("{}", want.round()))
                .map_err(|e| refused(RadioSetting::Rate.id(), e))?;
        }
        let n = self.receivers.count();
        let usb_note = if self.default_address {
            rate::link_note(want * n as f64, self.link_cap(), self.cs8, self.tezuka)
        } else {
            None
        };
        let cap = self.measured_cap().unwrap_or(0.0);
        if let Some(note) = usb_note {
            self.notices.log(note.clone());
            notes.push(note);
        } else if cap > 0.0 && want > cap * 1.01 {
            let note = format!(
                "PlutoSDR {}: the link to this board carries about {} Msps; at {} Msps samples will be lost",
                self.label,
                fmt_m(cap),
                fmt_m(want)
            );
            self.notices.log(note.clone());
            notes.push(note);
        } else if !self.default_address && !is_local(&self.host) {
            let bytes = want * n as f64 * if self.cs8 { 2.0 } else { 4.0 };
            if let Some(note) = network_note(want, n, bytes) {
                self.notices.log(note.clone());
                notes.push(note);
            }
        }
        let asked_bw = v.get(&BANDWIDTH);
        let bw = rf_bandwidth(asked_bw, want);
        let written = if self.rx_now("rf_bandwidth") == Some(bw) {
            Ok(0)
        } else {
            self.rx_attr("rf_bandwidth", &format!("{bw}"))
        };
        if let Err(e) = written {
            if asked_bw > 0.0 {
                return Err(refused(RadioSetting::Bandwidth.id(), e));
            }
            self.notices.log(format!(
                "PlutoSDR: the filter width {} MHz was refused: {e}",
                fmt_m(bw)
            ));
        }
        let phy = self.facts.phy.clone();
        let got = self
            .ctl
            .read_attr(
                &phy,
                Target::Channel(false, self.phy_chans[0]),
                "sampling_frequency",
            )
            .map_err(DriverError::from)?;
        self.rate = got.parse::<f64>().unwrap_or(want);
        if self.rate < 0.97 * want {
            notes.push(format!(
                "this radio only offers {} Msps, {} Msps were requested for this channel width",
                fmt_m(self.rate),
                fmt_m(want)
            ));
        }
        let mut b = lock(&self.board);
        b.rx_rate = self.rate;
        b.rx_fir = self.fir_dec.is_some();
        Ok(())
    }

    /// The rate the ring runs at: the radio's rate, times two when both receivers are interleaved.
    fn ring_rate(&self) -> Hz {
        if self.receivers == Receivers::Interleaved {
            2.0 * self.rate
        } else {
            self.rate
        }
    }

    /// The gain range of receiver `i` (read once from the board, else the default of the band at `lo`).
    fn gain_range(&mut self, i: usize, lo: Hz) -> (f64, f64) {
        if self.gain_range[i].is_none() {
            let (phy, ch) = (self.facts.phy.clone(), self.phy_chans[i]);
            self.gain_range[i] = self
                .ctl
                .read_attr(&phy, Target::Channel(false, ch), "hardwaregain_available")
                .ok()
                .and_then(|s| parse_range(&s))
                .map(|(a, _, b)| (a, b));
        }
        self.gain_range[i].unwrap_or((0.0, default_max_gain(lo)))
    }

    /// The samples that can be in flight between the converter and the meter of the stream: the kernel buffers and two more.
    fn latency(&self) -> u64 {
        let buffer = self.bufsize.unwrap_or_else(|| buffer_samples(self.rate));
        (u64::from(self.kbuffers) + 2) * buffer as u64
    }

    /// Sets the gain of receiver `i` by `plan`. `retuned`: the LO or the port changed, so a held gain starts again. `setting` and
    /// `gain_id` name the settings that a refusal names.
    fn configure_gain(
        &mut self,
        i: usize,
        plan: GainPlan,
        lo: Hz,
        retuned: bool,
        setting: SettingId,
        gain_id: SettingId,
    ) -> Result<(), DriverError> {
        let ch = self.phy_chans[i];
        let mode = match plan {
            GainPlan::Chip(m) => m,
            _ => GainMode::Manual,
        };
        let mut restart = retuned;
        if self.gain_mode[i] != Some(mode) {
            if !self.gain_modes.is_empty() && !self.gain_modes.iter().any(|m| m == mode.attr()) {
                return Err(DriverError::Refused { id: setting });
            }
            self.chan_attr(ch, "gain_control_mode", mode.attr())
                .map_err(|e| refused(setting, e))?;
            self.gain_mode[i] = Some(mode);
            self.gain_written[i] = None;
            restart = true;
        }
        let hold = self.holds[i].clone();
        let write = match plan {
            GainPlan::Chip(_) => {
                lock(&hold).stop();
                None
            }
            GainPlan::Manual(g) => {
                lock(&hold).stop();
                let (min, max) = self.gain_range(i, lo);
                let g = g.round();
                if g < min.max(0.0) || g > max {
                    return Err(DriverError::Refused { id: gain_id });
                }
                Some(g)
            }
            // The stream cannot step while this runs (the radio is busy), so the decision and the write need no lock held.
            GainPlan::Held if restart || !lock(&hold).active() => {
                Some(held_start_gain(lo, self.gain_range(i, lo)))
            }
            GainPlan::Held => None,
        };
        if let Some(g) = write {
            if self.gain_written[i] != Some(g) {
                self.chan_attr(ch, "hardwaregain", &format!("{g}"))
                    .map_err(|e| refused(gain_id, e))?;
                self.gain_written[i] = Some(g);
            }
            if plan == GainPlan::Held {
                let range = self.gain_range(i, lo);
                let latency = self.latency();
                lock(&hold).start(g, range, self.rate, latency);
            }
        }
        let status = lock(&hold).status();
        lock(&self.board).gain_hold[usize::from(ch == "voltage1")] = status;
        Ok(())
    }
}

impl Radio for IiodRadio {
    type Stream = IiodStream;

    fn configure(&mut self, v: &Values, live: bool) -> Result<String, DriverError> {
        let mut notes = Vec::new();
        // A held gain starts again when the LO or the port changes, and when no stream ran.
        let mut retuned = !live;
        self.link = v.get(&LINK);
        if !live {
            self.configure_rate(v, &mut notes)?;
        }
        // The tracking switches, at open and when changed. A board that lacks one refuses it.
        let tracking_ids = [
            IiodSetting::QuadTrack,
            IiodSetting::RfDcTrack,
            IiodSetting::BbDcTrack,
        ];
        for (k, ((setting, attr), id)) in TRACKING.iter().zip(tracking_ids).enumerate() {
            let on = v.get(setting);
            if self.tracking_written[k] == Some(on) {
                continue;
            }
            if self.facts.phy_has(self.phy_chans[0], false, attr) {
                self.rx_attr(attr, if on { "1" } else { "0" })
                    .map_err(|e| refused(own_id(id), e))?;
            } else if !on {
                return Err(DriverError::Refused { id: own_id(id) });
            }
            self.tracking_written[k] = Some(on);
        }
        // The RX port, if chosen and changed.
        let port = v.get(&RX_PORT);
        if let Some(name) = port.attr() {
            if self.port_written != Some(port) {
                let id = own_id(IiodSetting::RxPort);
                if !self.rx_ports.is_empty() && !self.rx_ports.iter().any(|p| p == name) {
                    return Err(DriverError::Refused { id });
                }
                for ch in self.phy_chans.clone() {
                    self.chan_attr(ch, "rf_port_select", name)
                        .map_err(|e| refused(id, e))?;
                }
                self.port_written = Some(port);
                retuned = true;
            }
        }
        // PPM: through the clock if it is known and the transmitter does not hold it, else through the LO.
        let ppm = v.get(&PPM);
        let tx_armed = lock(&self.board).tx_armed;
        if let (Some(base), false) = (self.xo_base, tx_armed) {
            if self.ppm_written != Some(ppm) && (ppm != 0.0 || self.ppm_in_effect != 0.0) {
                let phy = self.facts.phy.clone();
                let xo = (base * (1.0 + ppm * 1e-6)).round();
                match self
                    .ctl
                    .write_attr(&phy, Target::Device, "xo_correction", &format!("{xo}"))
                {
                    Ok(_) => self.ppm_in_effect = ppm,
                    Err(e) => return Err(refused(own_id(IiodSetting::Ppm), e)),
                }
            }
            self.ppm_written = Some(ppm);
        }
        // LO.
        let centre = v.get(&CENTRE);
        let lo = (centre / (1.0 + ppm * 1e-6) * (1.0 + self.ppm_in_effect * 1e-6))
            .max(0.0)
            .round();
        if !live || self.lo_written != Some(lo) {
            let phy = self.facts.phy.clone();
            if let Err(e) = self.ctl.write_attr(
                &phy,
                Target::Channel(true, RX_LO),
                "frequency",
                &format!("{lo}"),
            ) {
                if self.lo_range.1 <= 3.8e9 {
                    self.notices.warn(
                        "PlutoSDR: the stock firmware tunes 325 to 3800 MHz. 70 to 6000 MHz needs the AD9364 setting \
                         (fw_setenv attr_name compatible; fw_setenv attr_val ad9364).",
                    );
                }
                return Err(refused(RadioSetting::Centre.id(), e));
            }
            self.lo_written = Some(lo);
            self.centre = centre;
            retuned = true;
            let (a, b) = self.lo_range;
            if lo < a || lo > b {
                notes.push(format!(
                    "this PlutoSDR is specified for {}-{} MHz",
                    fmt_m(a),
                    fmt_m(b)
                ));
            }
        }
        // The gain mode and the gain of each receiver.
        let plan = gain_plan(v.get(&GAIN_MODE), v.get(&AGC), v.get(&GAIN));
        let (mode_id, gain_id) = (own_id(IiodSetting::GainMode), RadioSetting::Gain.id());
        self.configure_gain(0, plan, lo, retuned, mode_id, gain_id)?;
        if self.phy_chans.len() > 1 {
            let plan2 = match v.get(&GAIN_MODE2) {
                Rx2GainMode::Same => None,
                Rx2GainMode::Manual => Some(GainPlan::Manual(v.get(&GAIN2))),
                Rx2GainMode::SlowAttack => Some(GainPlan::Chip(GainMode::SlowAttack)),
                Rx2GainMode::FastAttack => Some(GainPlan::Chip(GainMode::FastAttack)),
                Rx2GainMode::Hybrid => Some(GainPlan::Chip(GainMode::Hybrid)),
            };
            let (p2, ids) = match plan2 {
                None => (plan, (mode_id, gain_id)),
                Some(p) => (
                    p,
                    (own_id(IiodSetting::GainMode2), own_id(IiodSetting::Gain2)),
                ),
            };
            self.configure_gain(1, p2, lo, retuned, ids.0, ids.1)?;
        }
        Ok(notes.join("; "))
    }

    fn check(&self, v: &Values) -> Result<(), DriverError> {
        check_receivers(v, self.facts.receivers, self.tezuka)?;
        let b = lock(&self.board);
        if b.tx_armed && v.get(&LOW_RATE_FIR) && fir_decimation(rate_wanted(v, true)).is_some() {
            // The low-rate FIR cannot be used while the transmitter is on air.
            return Err(DriverError::NotNow {
                state: crate::Doing::Transmitting,
            });
        }
        Ok(())
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<IiodStream, DriverError> {
        let connect = |me: &IiodRadio| -> Result<IiodConn, DriverError> {
            let mut conn = IiodConn::connect(
                &me.transport,
                &me.host,
                me.port,
                Duration::from_secs(8),
                abort,
            )?;
            conn.recv_buffer_size(&[8 << 20, 4 << 20]);
            conn.timeout(CONTEXT_TIMEOUT_MS)?;
            Ok(conn)
        };
        let mut samples = self.bufsize.unwrap_or_else(|| buffer_samples(self.rate));
        // iiod shares one device buffer between its clients and refills it in the size of the first; a smaller buffer would get
        // only the start of each refill. When another client already streams, take at least its size.
        let rx = self.facts.rx.id.clone();
        let enabled = |ctl: &mut IiodConn| {
            ctl.read_attr(&rx, Target::Buffer, "enable")
                .is_ok_and(|v| v == "1")
        };
        let mut busy = enabled(&mut self.ctl);
        let ours = self.streamed
            || lock(&self.known)
                .get(&self.label)
                .and_then(|k| k.stream_ended)
                .is_some_and(|t| t.elapsed() < OWN_BUFFER_RECENT);
        if busy && ours {
            // Our own previous stream (a restart, or a new open soon after a stop): the server frees its buffer once it sees
            // that connection end. A new buffer must not join the old one as a second client: the start fails as busy (which
            // the base tries again) when it is not freed in time.
            let end = Instant::now() + OWN_BUFFER_WAIT;
            while busy && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(20));
                busy = enabled(&mut self.ctl);
            }
            if busy {
                self.notices.log(format!(
                    "PlutoSDR {}: the board still holds the buffer of our last stream; trying again",
                    self.label
                ));
                return Err(DriverError::Busy);
            }
        }
        if busy {
            let theirs = self
                .ctl
                .read_attr(&rx, Target::Buffer, "length")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            if theirs > samples && theirs <= 4 * MAX_BUFFER {
                self.notices.log(format!(
                    "PlutoSDR {}: another client is streaming from this board with {theirs}-sample buffers; using that size",
                    self.label
                ));
                samples = theirs;
            }
            self.notices.warn(format!(
                "PlutoSDR {}: another client is streaming from this board; the rate, the frequency and the gains are shared with it",
                self.label
            ));
        }
        let lay = layout(&self.facts.rx, &self.mask, &self.want, self.cs8)
            .map_err(|_| DriverError::Protocol(Protocol::MissingPart))?;
        let bytes = samples * lay.stride;
        let mut conn = connect(self)?;
        // The binary protocol, when the server offers it and no other client holds the buffer (a 1.x server lets one client have it).
        if !self.text_only && self.binary_ok != Some(false) && !busy {
            // The server frees an old buffer only once it sees the old connection end: a fresh open may meet "exists" or "busy" for a
            // moment, so it is tried again a few times.
            let mut tries = 0;
            loop {
                match conn.binary() {
                    Ok(true) => {
                        self.binary_ok = Some(true);
                        match BinaryRx::open(
                            conn,
                            self.facts.rx_index as u8,
                            &self.mask,
                            self.kbuffers as u16,
                            bytes,
                        ) {
                            Ok(mut b) => {
                                b.conn().register(abort);
                                b.conn_mut().set_read_timeout(Duration::from_secs(5));
                                self.notices.log(format!(
                                    "PlutoSDR {}: streaming over the binary protocol",
                                    self.label
                                ));
                                return Ok(self.make_stream(Link2::Binary(b), samples, lay, false));
                            }
                            Err(IioError::Errno(e @ (16 | 17))) if tries < 10 => {
                                let _ = e;
                                tries += 1;
                                std::thread::sleep(Duration::from_millis(50));
                                conn = connect(self)?;
                            }
                            Err(e) => {
                                self.notices.log(format!(
                                    "PlutoSDR {}: the binary stream did not open ({e}); using the text protocol",
                                    self.label
                                ));
                                conn = connect(self)?;
                                break;
                            }
                        }
                    }
                    Ok(false) => {
                        self.binary_ok = Some(false);
                        break;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
        conn.set_buffers_count(&rx, self.kbuffers)?;
        if let Err(e) = conn.open_buffer(&rx, samples, &self.mask, false) {
            if samples == SMALL_BUFFER {
                return Err(e.into());
            }
            samples = SMALL_BUFFER;
            conn.open_buffer(&rx, samples, &self.mask, false)?;
        }
        conn.set_read_timeout(Duration::from_secs(5));
        Ok(self.make_stream(
            Link2::Text {
                conn,
                dev: rx,
                open: true,
            },
            samples,
            lay,
            busy,
        ))
    }

    fn stream_ended(&mut self) {
        lock(&self.board).rx_streaming = false;
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if let Some(dec) = self.fir_dec {
            let _ = self.rx_attr(
                "sampling_frequency",
                &format!("{}", (f64::from(dec) * self.rate).round()),
            );
            self.fir_off();
        }
        if let (Some(base), true) = (self.xo_base, self.ppm_in_effect != 0.0) {
            let phy = self.facts.phy.clone();
            let _ = self.ctl.write_attr(
                &phy,
                Target::Device,
                "xo_correction",
                &format!("{}", base.round()),
            );
        }
        self.ctl.shutdown();
        for h in &self.holds {
            lock(h).stop();
        }
        let mut b = lock(&self.board);
        b.rx_streaming = false;
        b.rx_fir = false;
        b.gain_hold = [None; 2];
    }

    fn rate_hz(&self) -> Hz {
        self.ring_rate()
    }

    fn centre_hz(&self) -> Hz {
        self.centre
    }

    fn sample_type(&self) -> SampleType {
        let format = if self.cs8 {
            IqFormat::Cs8
        } else {
            IqFormat::Cs16
        };
        format.sample_type()
    }

    fn capabilities(&self) -> Capabilities {
        let cap = self.link_cap();
        let (gmin, gmax) = self.gain_range[0].unwrap_or((0.0, 73.0));
        let min_rate = if self.lowratefir {
            FIR_MIN_RATE
        } else {
            NO_FIR_MIN_RATE
        };
        Capabilities {
            name: self.facts.name.clone(),
            rates: RateLimits {
                min_hz: min_rate,
                max_hz: self.facts.max_rate(),
                ranges: Vec::new(),
                steady_hz: (cap > 0.0).then_some(cap),
            },
            min_freq_hz: self.lo_range.0,
            max_freq_hz: self.lo_range.1,
            gain_db: Some((gmin.max(0.0), gmax, 1.0)),
            bandwidth_hz: Some(FILTER_WIDTH),
            has_agc: true,
            transmit: self.tx_limits.clone(),
            has_bias_tee: false,
            realtime: true,
            common_rates_hz: COMMON_RATES.to_vec(),
            common_bandwidths_hz: COMMON_WIDTHS.to_vec(),
            gain_steps_db: Vec::new(),
        }
    }

    fn live_rate_change(&self) -> bool {
        false
    }

    fn status(&self) -> String {
        let what = match self.receivers {
            Receivers::One => "",
            Receivers::Both => " x 2",
            Receivers::Interleaved => " x 2 interleaved",
        };
        let line = format!("{} Msps{what} from {}", fmt_m(self.rate), self.label);
        match lock(&self.board).link_short {
            Some((part, rate)) => format!("{line}; {}: samples are lost", crate::arrive_text(part * rate, rate)),
            None => line,
        }
    }

    fn write_held_gain(&mut self, rx: usize, gain_db: f64) -> Result<(), DriverError> {
        let id = RadioSetting::Gain.id();
        let ch = *self.phy_chans.get(rx).ok_or(DriverError::Refused { id })?;
        self.chan_attr(ch, "hardwaregain", &format!("{gain_db}"))
            .map_err(|e| refused(id, e))?;
        self.gain_written[rx] = Some(gain_db);
        Ok(())
    }
}

impl IiodRadio {
    /// The stream over `link`, of buffers of `samples`. A stream on a `shared` buffer (another client's) learns nothing about the
    /// link; a text stream on a board across a network times the queued buffers at its start ([`Probe`]).
    fn make_stream(&mut self, link: Link2, samples: usize, lay: Layout, shared: bool) -> IiodStream {
        lock(&self.board).rx_streaming = true;
        self.streamed = true;
        let aux = if self.receivers == Receivers::Both {
            lock(&self.aux).take()
        } else {
            None
        };
        let full = lay.full_scale();
        let holds = self
            .holds
            .iter()
            .zip(&self.phy_chans)
            .enumerate()
            .map(|(rx, (hold, ch))| HoldLoop {
                rx,
                slot: usize::from(*ch == "voltage1"),
                meter: Meter::new(lock(hold).window_samples(self.rate), full),
                hold: hold.clone(),
                levels: Vec::new(),
                shown: None,
            })
            .collect();
        let learns = !shared && !self.default_address && !is_local(&self.host);
        let need_bps = self.rate * lay.stride as f64;
        let probe = (learns && matches!(link, Link2::Text { .. }) && self.kbuffers >= 4 && self.rate > 0.0).then(|| {
            let fill = Duration::from_secs_f64(f64::from(self.kbuffers - 1) * samples as f64 / self.rate);
            Probe {
                fill: fill.min(PROBE_FILL_MAX),
                left: self.kbuffers - 1,
                bytes: 0,
                from: None,
            }
        });
        // A new stream starts with no shortfall: one of the stream before, at another rate, is gone.
        lock(&self.board).link_short = None;
        IiodStream {
            holds,
            board: self.board.clone(),
            link,
            raw: Vec::new(),
            bytes: samples * lay.stride,
            label: self.label.clone(),
            known: self.known.clone(),
            learns,
            need_bps,
            rate: self.rate,
            probe,
            watch: Watch::default(),
            dev: self.facts.rx.clone(),
            want: self.want.clone(),
            cs8: self.cs8,
            asked: self.mask.clone(),
            lay,
            receivers: self.receivers,
            aux,
            aux_home: self.aux.clone(),
        }
    }
}

impl Drop for IiodRadio {
    fn drop(&mut self) {
        self.close();
    }
}

enum Link2 {
    /// The text protocol. The `READBUF`s on their way (sent ahead, so the server never waits for the next request) are those
    /// the connection still owes ([`IiodConn::owed`]).
    Text {
        conn: IiodConn,
        dev: String,
        /// The buffer is open: it is closed with `CLOSE` when the connection still has its framing.
        open: bool,
    },
    Binary(BinaryRx),
}

/// The timing of the buffers that the kernel queued while the stream started: they come as fast as the link carries them.
struct Probe {
    /// How long the kernel buffers fill before the first read.
    fill: Duration,
    /// Answers still to read.
    left: u32,
    /// Bytes after the first answer.
    bytes: u64,
    /// The end of the first answer.
    from: Option<Instant>,
}

/// The watch on what arrives: bytes in the current window, and the part of the rate last warned about.
#[derive(Default)]
struct Watch {
    /// When the stream started: the first [`WATCH_SETTLE`] say nothing about the link.
    started: Option<Instant>,
    since: Option<Instant>,
    bytes: u64,
    warned: Option<f64>,
    /// Windows in a row that got less than [`SHORT`] of the rate.
    short: u32,
}

/// The held gain of one receiver as the stream measures and steps it.
struct HoldLoop {
    /// The receiver: its position in the layout and in the phy channels in use.
    rx: usize,
    /// Its place in [`BoardState::gain_hold`].
    slot: usize,
    hold: Arc<Mutex<HeldGain>>,
    meter: Meter,
    levels: Vec<Level>,
    shown: Option<HoldStatus>,
}

/// The stream connection: buffer after buffer, converted straight into the ring.
pub struct IiodStream {
    holds: Vec<HoldLoop>,
    board: Board,
    link: Link2,
    raw: Vec<u8>,
    bytes: usize,
    label: String,
    known: Arc<Mutex<HashMap<String, Known>>>,
    /// The stream measures the link (a board across a network, a buffer of its own).
    learns: bool,
    /// The bytes a second the rate needs.
    need_bps: f64,
    /// The radio's rate.
    rate: Hz,
    probe: Option<Probe>,
    watch: Watch,
    dev: Device,
    want: Vec<(usize, usize)>,
    cs8: bool,
    /// The requested mask; the layout follows the mask the server reports.
    asked: Vec<u32>,
    lay: Layout,
    receivers: Receivers,
    aux: Option<RingWriter>,
    aux_home: AuxRing,
}

impl IiodStream {
    /// Reads one buffer into `raw`: the bytes, and the mask they carry when the server said. Unless `stopping`, the next request
    /// goes out before this buffer is converted (two while the probe runs, so the link never waits for a request).
    fn refill(&mut self, stopping: bool) -> Result<(usize, Option<Vec<u32>>), IioError> {
        let ahead = if self.probe.is_some() { 2 } else { 1 };
        match &mut self.link {
            Link2::Text { conn, dev, .. } => {
                if conn.owed() == 0 {
                    conn.send_readbuf(dev, self.bytes)?;
                }
                let r = conn.recv_buffer(self.bytes, &mut self.raw);
                if r.is_ok() && !stopping {
                    while conn.owed() < ahead {
                        conn.send_readbuf(dev, self.bytes)?;
                    }
                }
                r
            }
            Link2::Binary(b) => b.recv(&mut self.raw).map(|n| (n, None)),
        }
    }

    /// Takes `bps` bytes a second, what a stream got while the link held it below its rate, into the live figure of the
    /// board: the figure moves toward the reading. A figure that is no longer fresh is replaced.
    fn learn_limit(&self, bps: f64) {
        if !self.learns || !bps.is_finite() || bps <= 0.0 {
            return;
        }
        let mut k = lock(&self.known);
        let e = k.entry(self.label.clone()).or_default();
        let bps = match e.link.and_then(|l| l.fresh()) {
            Some(old) => LIVE_SMOOTH * old + (1.0 - LIVE_SMOOTH) * bps,
            None => bps,
        };
        // Two figures, so that a small change does not make the capabilities of the radio new at every window.
        let shown = round2(bps);
        e.link = Some(LiveLink {
            bps: shown,
            at: Instant::now(),
        });
    }

    /// Times one answer of `n` bytes for the probe; at its end, keeps what the link carried.
    fn probe_step(&mut self, n: usize, ctx: &StreamCtx) {
        let Some(p) = &mut self.probe else {
            return;
        };
        let now = Instant::now();
        match p.from {
            None => p.from = Some(now),
            Some(_) => p.bytes += n as u64,
        }
        p.left = p.left.saturating_sub(1);
        if p.left > 0 {
            return;
        }
        let secs = p.from.map_or(0.0, |f| (now - f).as_secs_f64());
        let bytes = p.bytes as f64;
        self.probe = None;
        if secs <= 0.0 || bytes <= 0.0 {
            return;
        }
        let bps = bytes / secs;
        // Queued buffers that came no faster than the rate say only that the link carries the rate. A timing at the start
        // of a stream is for the log only: it limits nothing.
        let at_most = timed_at_most(bps, self.need_bps);
        ctx.notices.log(format!(
            "PlutoSDR {}: the link carried {:.1} MB/s at the start of the stream{}",
            self.label,
            bps / 1e6,
            if at_most { "" } else { " (the rate of the stream, or more)" }
        ));
    }

    /// Adds `n` bytes to the watch, which judges windows of [`WATCH_WINDOW`] after the first [`WATCH_SETTLE`] of the stream.
    /// When [`SHORT_WINDOWS`] windows in a row get less than [`SHORT`] of the rate, the link is the limit: what arrived goes
    /// into the live figure, and a warning says "6 of 20 Msps arrive (30 %): samples are lost". A window that gets the rate clears the
    /// figure: the link carries at least the rate, which limits nothing.
    fn watch(&mut self, n: usize, ctx: &StreamCtx) {
        let now = Instant::now();
        let w = &mut self.watch;
        let started = *w.started.get_or_insert(now);
        let Some(since) = w.since else {
            w.since = Some(now);
            w.bytes = 0;
            return;
        };
        w.bytes += n as u64;
        let secs = (now - since).as_secs_f64();
        if secs < WATCH_WINDOW.as_secs_f64() || self.need_bps <= 0.0 {
            return;
        }
        let bps = w.bytes as f64 / secs;
        let part = bps / self.need_bps;
        w.since = Some(now);
        w.bytes = 0;
        if now - started < WATCH_SETTLE {
            return;
        }
        w.short = if part < SHORT { w.short + 1 } else { 0 };
        // Only a shortfall that lasts says that the link is the limit.
        let limited = w.short >= SHORT_WINDOWS;
        lock(&self.board).link_short = limited.then_some((part, self.rate));
        if limited {
            let shown = w.warned.is_some_and(|p| (p - part).abs() < 0.1);
            if !shown {
                w.warned = Some(part);
                ctx.notices.warn(format!(
                    "PlutoSDR {}: {}: samples are lost",
                    self.label,
                    crate::arrive_text(part * self.rate, self.rate)
                ));
            }
            self.learn_limit(bps);
        } else if part >= SHORT {
            if w.warned.take().is_some() {
                ctx.notices.log(format!(
                    "PlutoSDR {}: all of {} Msps arrive again",
                    self.label,
                    fmt_m(self.rate)
                ));
            }
            // The stream gets its rate: the link carries at least that, and no figure limits it.
            if self.learns {
                if let Some(e) = lock(&self.known).get_mut(&self.label) {
                    e.link = None;
                }
            }
        }
    }

    /// Measures the level of the receivers with a held gain in the first `n` bytes of the buffer, and steps a gain when the
    /// control asks for it and the radio is free.
    fn measure(&mut self, n: usize, ctx: &StreamCtx) {
        let IiodStream {
            holds,
            board,
            raw,
            lay,
            ..
        } = self;
        let raw = &raw[..n - n % lay.stride.max(1)];
        for h in holds {
            if !lock(&h.hold).active() {
                continue;
            }
            let (meter, levels) = (&mut h.meter, &mut h.levels);
            lay.components(raw, h.rx, |i, q| {
                if let Some(l) = meter.add(i, q) {
                    levels.push(l);
                }
            });
            for l in h.levels.drain(..) {
                let mut g = lock(&h.hold);
                if let Some(want) = g.feed(&l) {
                    match ctx.write_gain(h.rx, want) {
                        Some(Ok(())) => g.stepped(want),
                        Some(Err(e)) => {
                            g.not_stepped();
                            ctx.notices
                                .log(format!("PlutoSDR: the gain of {want} dB was not written: {e}"));
                        }
                        None => {}
                    }
                }
                let status = g.status();
                drop(g);
                if status != h.shown {
                    h.shown = status;
                    lock(board).gain_hold[h.slot] = status;
                }
            }
        }
    }

    fn deliver(&mut self, n: usize, sink: &mut Sink) {
        let inst = self.lay.instants(n);
        if inst == 0 {
            return;
        }
        let (raw, lay) = (&self.raw[..n], &self.lay);
        match self.receivers {
            Receivers::One => {
                sink.write_with(inst, |dst, from| lay.fill(raw, 0, dst, from));
            }
            Receivers::Both => {
                sink.write_with(inst, |dst, from| lay.fill(raw, 0, dst, from));
                if let Some(w) = &mut self.aux {
                    w.write_with(inst, |dst, from| lay.fill(raw, 1, dst, from));
                }
            }
            Receivers::Interleaved => {
                sink.write_with(2 * inst, |dst, from| lay.fill_interleaved(raw, dst, from));
            }
        }
    }
}

impl RadioStream for IiodStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let mut failures = 0;
        let mut seen_mask = self.asked.clone();
        // A text READBUF that fails or ends early: the server does not send the rest of that request again, so its samples are
        // lost. A failed block answer of the binary protocol carries no data and skips none.
        let text = matches!(self.link, Link2::Text { .. });
        if let Some(p) = &self.probe {
            // The kernel buffers fill while nothing is read; then they come as fast as the link carries them.
            let end = Instant::now() + p.fill;
            while Instant::now() < end && !ctx.stopping() {
                std::thread::sleep(Duration::from_millis(5).min(end.saturating_duration_since(Instant::now())));
            }
        }
        loop {
            if ctx.stopping() {
                return Ok(());
            }
            match self.refill(ctx.stopping()) {
                Ok((n, mask)) if n > 0 => {
                    failures = 0;
                    if let Some(m) = mask {
                        if m != seen_mask {
                            // Another client enabled more channels: the data carries them all.
                            match layout(&self.dev, &m, &self.want, self.cs8) {
                                Ok(l) => {
                                    if m.iter().zip(&self.asked).any(|(a, b)| a & b != *b) {
                                        // The server streams channels other than those requested.
                                        return Err(DriverError::Protocol(Protocol::Malformed));
                                    }
                                    if !(0..m.len() * 32).all(|i| bit(&m, i) == bit(&self.asked, i))
                                    {
                                        ctx.notices.log(
                                            "PlutoSDR: another client widened the buffer; picking our channels out of it",
                                        );
                                    }
                                    self.lay = l;
                                }
                                Err(_) => return Err(DriverError::Protocol(Protocol::MissingPart)),
                            }
                            seen_mask = m;
                        }
                    }
                    self.deliver(n, sink);
                    self.measure(n, ctx);
                    if self.probe.is_some() {
                        self.probe_step(n, ctx);
                    } else {
                        self.watch(n, ctx);
                    }
                    if text && n < self.bytes {
                        sink.lost(((self.bytes - n) / self.lay.stride.max(1)) as u64);
                    }
                }
                // A whole answer that carried no samples: the framing holds, the next request may go.
                Ok(_) | Err(IioError::Errno(_)) => {
                    if text {
                        sink.lost((self.bytes / self.lay.stride.max(1)) as u64);
                    }
                    self.probe = None;
                    self.watch(0, ctx);
                    failures += 1;
                    if failures > MAX_REFILL_FAILURES {
                        return Err(DriverError::Protocol(Protocol::Malformed));
                    }
                }
                // The framing is lost: the connection takes no more commands, and the base opens a new one.
                Err(IioError::Malformed) => return Err(DriverError::Protocol(Protocol::Malformed)),
                Err(e @ IioError::Io(_)) => return Err(e.into()),
            }
        }
    }
}

impl Drop for IiodStream {
    fn drop(&mut self) {
        if let Some(w) = self.aux.take() {
            *lock(&self.aux_home) = Some(w);
        }
        // The answers still owed are read to their end within a time that fits the link; then the buffer is closed. A connection
        // whose framing is lost, or one that does not finish in time, is shut down without a word: the server frees the buffer
        // when it sees the connection end.
        let link_bps = lock(&self.known)
            .get(&self.label)
            .and_then(|k| k.link?.fresh())
            .unwrap_or(self.need_bps)
            .max(1e5);
        match &mut self.link {
            Link2::Text { conn, dev, open } => {
                if !conn.torn() && conn.owed() > 0 {
                    let owed = f64::from(conn.owed()) * self.bytes as f64;
                    let secs = (0.3 + 1.5 * owed / link_bps).clamp(0.3, 2.0);
                    let deadline = Instant::now() + Duration::from_secs_f64(secs);
                    let mut drain = std::mem::take(&mut self.raw);
                    while conn.owed() > 0 && !conn.torn() {
                        let _ = conn.recv_buffer_until(self.bytes, &mut drain, Some(deadline));
                    }
                }
                if *open && !conn.torn() {
                    conn.set_read_timeout(Duration::from_millis(500));
                    let dev = dev.clone();
                    let _ = conn.close_buffer(&dev);
                }
                conn.shutdown();
            }
            Link2::Binary(b) => {
                b.conn_mut().set_read_timeout(Duration::from_millis(500));
                b.close();
            }
        }
        lock(&self.known)
            .entry(self.label.clone())
            .or_default()
            .stream_ended = Some(Instant::now());
    }
}

/// A fake iiod server (an AD9361 board on the loopback interface), for the tests of this crate and, with the `fake` feature, of
/// the crates above it. Never a real radio.
#[cfg(any(test, feature = "fake"))]
#[allow(missing_docs)]
pub mod fake;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tx_tests;
