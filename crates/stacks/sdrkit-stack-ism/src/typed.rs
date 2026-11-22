//! The interface of the short-range device family: the shapes of the result sets, the table rows and the events, the
//! tables, the event types and what each stack declares.

use crate::subghzchat::LineKind;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{field, SeriesDef};
use sdrkit_core::settings::Argument;
use sdrkit_core::SettingSet;
use sdrkit_graph::{DecoderFields, GuardFields};
use sdrkit_proto_ookdev::components::DecodeFields;
use sdrkit_proto_ookdev::Check;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

// The sender of the frame stacks.

/// The result fields of the sender of a frame stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmTxFields {
    /// Whether a frame is on air.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The frames sent since the stack started.
    #[field(label = "Frames sent")]
    pub tx_sent: i64,
    /// The frames waiting for the transmitter.
    #[field(label = "Frames waiting")]
    pub tx_waiting: i64,
}

/// The fields of a change of a transmission of a frame stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmTxEvent<'a> {
    /// The state: started, queued, finished or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The addressee or the identity of the frame.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// What the frame carries, for started and queued; empty otherwise.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a sensor.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// Ends the transmission of a frame stack.
pub const TX_HALT: sdrkit_core::command::Command =
    sdrkit_core::command::Command::new("halt", "Halt")
        .help("Drops the frames waiting and ends the frame on air with the fall of the carrier.");

// ISM sensors.

/// The result fields of the burst finder of the ISM sensor stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmDetectFields {
    /// Bursts found in the band.
    #[field(label = "Bursts detected")]
    pub bursts_detected: i64,
    /// The mean noise floor of a frequency bin, once the finder has frames.
    #[field(label = "Noise floor", unit = "dBFS")]
    pub noise_floor_dbfs: Option<f64>,
    /// The mean input level since the last report, if samples came.
    #[field(label = "Input level", unit = "dBFS")]
    pub input_level_dbfs: Option<f64>,
}

/// The result fields of the device table of the ISM sensor stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmFields {
    /// Devices in the table.
    #[field(label = "Devices")]
    pub devices: i64,
    /// Distinct transmissions decoded.
    #[field(label = "Packets decoded")]
    pub packets_decoded: i64,
    /// Trains no protocol decoded.
    #[field(label = "Unknown bursts")]
    pub unknown_bursts: i64,
    /// Sigfox messages in the list.
    #[field(label = "Sigfox messages")]
    pub sigfox_messages: i64,
}

/// A row of the devices. The row key is the model, the id and the channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmDeviceRow<'a> {
    /// The device model.
    #[field(label = "Model")]
    pub model: &'a str,
    /// The device kind.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The device id.
    #[field(label = "ID")]
    pub id: &'a str,
    /// The channel, if the device sends one.
    #[field(shared = keys::CHANNEL)]
    pub channel: Option<&'a str>,
    /// The battery state: yes for good, no for low.
    #[field(label = "Battery good")]
    pub battery_ok: Option<bool>,
    /// The fields of the last reading as one line.
    #[field(label = "Readings")]
    pub readings: &'a str,
    /// The history of the first number with a unit, once the device sent one.
    #[field(label = "History")]
    pub history: Option<&'a str>,
    /// The level of the last transmission.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The signal to noise ratio of the last transmission.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The UTC of the last transmission.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Distinct transmissions.
    #[field(label = "Count")]
    pub count: i64,
    /// The frequency of the last transmission.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The frame test of the last reading.
    #[field(label = "Check")]
    pub check: Check,
}

/// A row of the Z-Wave view of the devices. The row key is the key of the device.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmZwaveRow<'a> {
    /// The home id, if the frame gave one.
    #[field(label = "Home ID")]
    pub home_id: Option<&'a str>,
    /// The node, if the frame gave one.
    #[field(label = "Node")]
    pub node: Option<i64>,
    /// The rate.
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Distinct frames.
    #[field(label = "Count")]
    pub count: i64,
    /// The command, if the frame gave one.
    #[field(label = "Command")]
    pub command: Option<&'a str>,
    /// The value of the command, if the frame gave one.
    #[field(label = "Value")]
    pub value: Option<&'a str>,
}

/// A row of the reading log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmLogRow<'a> {
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The device model.
    #[field(label = "Model")]
    pub model: &'a str,
    /// The device id.
    #[field(label = "ID")]
    pub id: &'a str,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The fields of the reading as one line.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A row of the unknown signals: a train no protocol decoded, with its line-code guess.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmUnknownRow<'a> {
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// OOK or FSK.
    #[field(label = "Modulation")]
    pub modulation: &'a str,
    /// The guessed line code.
    #[field(label = "Line code")]
    pub line_code: &'a str,
    /// Pulses of the train.
    #[field(label = "Pulses")]
    pub pulses: i64,
    /// The short width of the guess.
    #[field(label = "Short", unit = "µs")]
    pub short_us: f64,
    /// The long width of the guess.
    #[field(label = "Long", unit = "µs")]
    pub long_us: f64,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The pulses as text to copy ("OOK PWM 464 us/1404 us; 464/1404 ...").
    #[field(label = "Pulses")]
    pub copy: &'a str,
}

/// A row of the Sigfox messages heard inside the ISM sensor stack. The row key is the key of the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmSigfoxRow<'a> {
    /// The signal time of the first frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The device id.
    #[field(label = "Device ID")]
    pub device_id: &'a str,
    /// The message counter.
    #[field(label = "Seq")]
    pub seq: i64,
    /// The payload as hex, or the one-bit value.
    #[field(label = "Payload")]
    pub payload: &'a str,
    /// The frequency of the first repetition.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level of the strongest repetition.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Repetitions heard.
    #[field(label = "Reps")]
    pub reps: i64,
    /// The authentication code, never checked, and the CRC.
    #[field(label = "MAC / CRC")]
    pub mac_crc: &'a str,
}

/// The fields of a device heard for the first time. `source` is the model and the id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmDeviceEvent<'a> {
    /// The model and the id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The line of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The device model.
    #[field(label = "Model")]
    pub model: &'a str,
    /// The device id.
    #[field(label = "ID")]
    pub id: &'a str,
}

/// The devices.
pub static ISM_DEVICES: Table<IsmDeviceRow<'static>> =
    Table::new("devices", "Devices", 300).label(&IsmDeviceRow::ID);
/// The Z-Wave devices.
pub static ISM_ZWAVE: Table<IsmZwaveRow<'static>> = Table::new("zwave", "Z-Wave", 300);
/// The reading log, newest last.
pub static ISM_LOG: Table<IsmLogRow<'static>> = Table::new("log", "Log", 400).label(&IsmLogRow::ID);
/// The unknown signals, newest last.
pub static ISM_UNKNOWN: Table<IsmUnknownRow<'static>> =
    Table::new("unknown", "Unknown signals", 60);
/// The Sigfox messages, newest first.
pub static ISM_SIGFOX: Table<IsmSigfoxRow<'static>> =
    Table::new("sigfox", "Sigfox", 300).label(&IsmSigfoxRow::DEVICE_ID);
/// A device heard for the first time.
pub static ISM_DEVICE: EventKind<IsmDeviceEvent<'static>> =
    EventKind::new("ism.device", "ISM device");

sdrkit_core::choice_enum! {
    /// The line code that the pulse analyser names for a frame.
    pub enum PulseCode {
        /// Pulse width: one bit per pulse.
        Pwm = ("pwm", "PWM"),
        /// Pulse position: one bit per gap.
        Ppm = ("ppm", "PPM"),
        /// Manchester: one bit per two half-bits.
        Manchester = ("manchester", "Manchester"),
        /// Runs of a whole number of units.
        Pcm = ("pcm", "PCM (NRZ-like)"),
    }
}

/// The result fields of the pulse analyser (submode `pulse_analyser`): the facts of the last frame with a line code, and
/// the counts. A fact has no value before the first such frame and after a change of frequency, and a timing has a value
/// only for the line codes that have it.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmPulseFields<'a> {
    /// The line code guessed from the pulse timings.
    #[field(label = "Line code")]
    pub line_code: Option<PulseCode>,
    /// Bits read.
    #[field(label = "Bits")]
    pub bits: Option<i64>,
    /// The first 96 bits as 0 and 1, with "..." when there are more.
    #[field(label = "Bits (binary)")]
    pub bits_bin: Option<&'a str>,
    /// The first 96 bits as hex; the last digit is padded with zero bits.
    #[field(label = "Bits (hex)")]
    pub bits_hex: Option<&'a str>,
    /// PWM: the short width. PPM: the short gap.
    #[field(label = "Short", unit = "µs")]
    pub short_us: Option<f64>,
    /// PWM: the long width. PPM: the long gap.
    #[field(label = "Long", unit = "µs")]
    pub long_us: Option<f64>,
    /// PPM: the pulse width.
    #[field(label = "Pulse", unit = "µs")]
    pub pulse_us: Option<f64>,
    /// PWM: the bit period.
    #[field(label = "Bit period", unit = "µs")]
    pub period_us: Option<f64>,
    /// Manchester: the half-bit.
    #[field(label = "Half-bit", unit = "µs")]
    pub half_bit_us: Option<f64>,
    /// PCM: the unit.
    #[field(label = "Unit", unit = "µs")]
    pub unit_us: Option<f64>,
    /// Manchester and PCM: the bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Option<f64>,
    /// The pulses of the frame.
    #[field(label = "Pulses")]
    pub pulses: Option<i64>,
    /// From the start of the first pulse to the end of the last.
    #[field(label = "Frame length", unit = "ms")]
    pub frame_ms: Option<f64>,
    /// Frames in a row with the same bits, each within 0.5 s of the one before.
    #[field(label = "Repeats")]
    pub repeats: Option<i64>,
    /// Frames analysed, with a line code or not.
    #[field(label = "Frames seen")]
    pub frames_seen: i64,
    /// Frames with a line code.
    #[field(label = "Frames with a line code")]
    pub frames_good: i64,
}

/// A row of the analysed frames with a line code. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmPulseFrameRow<'a> {
    /// The signal time of the first pulse.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The line code guessed from the pulse timings.
    #[field(label = "Line code")]
    pub line_code: PulseCode,
    /// Bits read.
    #[field(label = "Bits")]
    pub bits: i64,
    /// The first 96 bits as 0 and 1, with "..." when there are more.
    #[field(label = "Bits (binary)")]
    pub bits_bin: &'a str,
    /// The first 96 bits as hex.
    #[field(label = "Bits (hex)")]
    pub bits_hex: &'a str,
    /// PWM: the short width. PPM: the short gap.
    #[field(label = "Short", unit = "µs")]
    pub short_us: Option<f64>,
    /// PWM: the long width. PPM: the long gap.
    #[field(label = "Long", unit = "µs")]
    pub long_us: Option<f64>,
    /// PPM: the pulse width.
    #[field(label = "Pulse", unit = "µs")]
    pub pulse_us: Option<f64>,
    /// PWM: the bit period.
    #[field(label = "Bit period", unit = "µs")]
    pub period_us: Option<f64>,
    /// Manchester: the half-bit.
    #[field(label = "Half-bit", unit = "µs")]
    pub half_bit_us: Option<f64>,
    /// PCM: the unit.
    #[field(label = "Unit", unit = "µs")]
    pub unit_us: Option<f64>,
    /// Manchester and PCM: the bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Option<f64>,
    /// The pulses of the frame.
    #[field(label = "Pulses")]
    pub pulses: i64,
    /// From the start of the first pulse to the end of the last.
    #[field(label = "Frame length", unit = "ms")]
    pub frame_ms: f64,
    /// Frames in a row with the same bits, this one included.
    #[field(label = "Repeats")]
    pub repeats: i64,
    /// The level of the marks.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The fields of an analysed frame with a line code.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsmPulseFrameEvent<'a> {
    /// The line of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The line code guessed from the pulse timings.
    #[field(label = "Line code")]
    pub line_code: PulseCode,
    /// Bits read.
    #[field(label = "Bits")]
    pub bits: i64,
    /// The first 96 bits as hex.
    #[field(label = "Bits (hex)")]
    pub bits_hex: &'a str,
    /// Frames in a row with the same bits, this one included.
    #[field(label = "Repeats")]
    pub repeats: i64,
    /// The level of the marks.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The analysed frames with a line code, newest last.
pub static ISM_PULSE_FRAMES: Table<IsmPulseFrameRow<'static>> =
    Table::new("pulse_frames", "Pulse frames", 500);
/// An analysed frame with a line code.
pub static ISM_PULSE_FRAME: EventKind<IsmPulseFrameEvent<'static>> =
    EventKind::new("ism.pulse_frame", "Pulse frame");

/// The arguments of `send` of the ISM sensor stack. Each family reads only the arguments its help names; the values are
/// those that the stack reports for the family.
#[derive(Arguments)]
pub struct IsmSendArgs<'a> {
    /// The device family.
    #[arg(shared = crate::ism::send::FAMILY)]
    pub family: Option<crate::ism::send::IsmFamily>,
    /// The id of the device.
    #[arg(
        label = "ID",
        help = "The id as the stack reports it, as a number (an id shown in hex is the same number). Remote: 1 to 65535, \
                the first 16 bits of the code. A family with a raw payload takes its id from the payload."
    )]
    pub id: i64,
    /// The command of a remote.
    #[arg(
        label = "Command",
        help = "Remote: 1 to 255, the last 8 bits of the code.",
        default = 1,
        min = 1,
        max = 255,
        step = 1
    )]
    pub command: Option<i64>,
    /// The channel of a sensor.
    #[arg(
        label = "Channel",
        help = "As the stack reports it. TX141TH-Bv2 0 to 3; Nexus and Prologue 1 to 4; Acurite tower 1 to 3 (A, B, C); \
                F007TH 1 to 8; Oregon 0 to 15; Bresser 6-in-1 0 to 7.",
        default = 1,
        min = 0,
        max = 15,
        step = 1
    )]
    pub channel: Option<i64>,
    /// Whether the battery of a sensor is good.
    #[arg(
        label = "Battery good",
        help = "Clear sends the battery-low state of the family.",
        default = true
    )]
    pub battery_ok: Option<bool>,
    /// The temperature of a sensor.
    #[arg(
        label = "Temperature",
        unit = "°C",
        help = "Sent in the resolution of the family (tenths of °C, or tenths of °F for F007TH); each family refuses a value \
                outside its range.",
        default = 20.0,
        min = -50.0,
        max = 140.0,
        step = 0.1
    )]
    pub temperature: Option<f64>,
    /// The humidity of a sensor.
    #[arg(
        label = "Humidity",
        unit = "%",
        help = "1 to 100 (Oregon, Bresser: to 99). 0 sends no humidity where the family has a value for that (Nexus-T, \
                TX29-IT, TX35DTH-IT, Acurite tower, WH24, Prologue).",
        default = 50,
        min = 0,
        max = 100,
        step = 1
    )]
    pub humidity: Option<i64>,
    /// The mean wind speed.
    #[arg(
        label = "Wind speed",
        unit = "m/s",
        help = "WH1080 (units of 0.34 m/s), WH24 (0.14 m/s), Bresser 5-in-1 and 6-in-1 (0.1 m/s).",
        default = 0.0,
        min = 0.0,
        max = 99.9,
        step = 0.1
    )]
    pub wind_speed: Option<f64>,
    /// The wind gust.
    #[arg(
        label = "Wind gust",
        unit = "m/s",
        help = "WH1080 (units of 0.34 m/s), WH24 (1.12 m/s), Bresser 5-in-1 and 6-in-1 (0.1 m/s).",
        default = 0.0,
        min = 0.0,
        max = 285.0,
        step = 0.1
    )]
    pub wind_gust: Option<f64>,
    /// The wind direction.
    #[arg(
        label = "Wind direction",
        unit = "°",
        help = "From north. WH1080 and Bresser 5-in-1 send one of 16 directions, WH24 and Bresser 6-in-1 whole degrees.",
        default = 0.0,
        min = 0.0,
        max = 359.0,
        step = 1.0
    )]
    pub wind_dir: Option<f64>,
    /// The rain count.
    #[arg(
        label = "Rain",
        unit = "mm",
        help = "WH1080 (units of 0.3 mm), WH24 (0.3 mm), Bresser 5-in-1 (0.1 mm, at most 999.9).",
        default = 0.0,
        min = 0.0,
        max = 19660.5,
        step = 0.1
    )]
    pub rain: Option<f64>,
    /// The UV index.
    #[arg(
        label = "UV index",
        help = "Bresser 6-in-1 (tenths, at most 99.9); WH24 (whole index 0 to 13).",
        default = 0.0,
        min = 0.0,
        max = 99.9,
        step = 0.1
    )]
    pub uv: Option<f64>,
    /// The light level.
    #[arg(
        label = "Light",
        unit = "lx",
        help = "WH24, in tenths of a lux.",
        default = 0.0,
        min = 0.0,
        max = 1_677_721.4,
        step = 0.1
    )]
    pub light: Option<f64>,
    /// The tyre pressure.
    #[arg(
        label = "Pressure",
        unit = "kPa",
        help = "Schrader (units of 2.5 kPa), Toyota (0.25 PSI from -7 PSI), Renault (0.75 kPa), Ford (0.25 PSI).",
        default = 0.0,
        min = 0.0,
        max = 1000.0,
        step = 0.01
    )]
    pub pressure: Option<f64>,
    /// The flags of a tyre sensor.
    #[arg(
        label = "Flags",
        help = "Schrader: the 8 flag bits. Renault: the 6 flag bits. Toyota: the status (bit 7 and bits 6-0).",
        default = 0,
        min = 0,
        max = 255,
        step = 1
    )]
    pub flags: Option<i64>,
    /// The current of a clamp.
    #[arg(
        label = "Current",
        unit = "A",
        help = "Efergy e2: 0 to 511 A.",
        default = 0.0,
        min = 0.0,
        max = 511.0,
        step = 0.01
    )]
    pub current: Option<f64>,
    /// The report interval.
    #[arg(
        label = "Interval",
        unit = "s",
        help = "Efergy e2: 6, 12, 18 or 24 s.",
        default = 6,
        min = 6,
        max = 24,
        step = 6
    )]
    pub interval: Option<i64>,
    /// The power of a meter.
    #[arg(
        label = "Power",
        unit = "W",
        help = "CurrentCost TX: power 0, 0 to 32767 W.",
        default = 0,
        min = 0,
        max = 32767,
        step = 1
    )]
    pub power: Option<i64>,
    /// The frame octets of a family with a raw payload.
    #[arg(
        label = "Payload",
        help = "The octets as pairs of hex digits, spaces allowed, as the stack reads them; the checks are added. Acurite \
                tower family: 5 to 9 octets (the message without its checksum; the parity bits are set). Bresser 3CH: 4 \
                octets. Citroen: octets 0 to 8. Schrader EG53MA4: 14 octets (5 before the message, then octets 0 to 8; the \
                first bit is 0). Bresser 7-in-1: octets 2 to 24 before whitening.",
        default = ""
    )]
    pub payload: Option<&'a str>,
}

/// Sends the frames of a sensor or a remote.
pub const ISM_SEND: CommandKind<IsmSendArgs<'static>> = CommandKind::new("send", "Send a frame")
    .help(
        "Sends one transmission of the device family (its copies of the frame, as the device sends them) on the tuned \
         frequency, as OOK or FSK by the family. Transmissions sent during one wait for it. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("halt");
/// A transmission of a sensor or a remote.
pub static ISM_TX: EventKind<IsmTxEvent<'static>> = EventKind::new("ism.tx", "ISM transmission");

/// The ISM sensor stack.
fn ism() -> StackShape {
    StackShape::new("ism")
        .fields::<IsmDetectFields>()
        .fields::<DecodeFields>()
        .fields::<SigfoxFinderFields>()
        .receiver_fields::<GuardFields>()
        .fields::<IsmFields>()
        .fields::<IsmPulseFields>()
        .component("burstdetect", |c| {
            c.fields::<IsmDetectFields>().shares_declared()
        })
        .component("decode", |c| c.fields::<DecodeFields>().shares_declared())
        .component("sfx_finder", |c| {
            c.fields::<SigfoxFinderFields>()
                .shares_declared()
                .series(SIGFOX_WATERFALL)
        })
        .table(&ISM_DEVICES)
        .table(&ISM_ZWAVE)
        .table(&ISM_LOG)
        .table(&ISM_UNKNOWN)
        .table(&ISM_SIGFOX)
        .table(&ISM_PULSE_FRAMES)
        .event(&ISM_DEVICE)
        .event(&SIGFOX_MESSAGE)
        .event(sdrkit_proto_ookdev::reading::reading())
        .event(&ISM_PULSE_FRAME)
        .settings(crate::ism::IsmSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&PAN_REMOVE)
        .fields::<IsmTxFields>()
        .event(&ISM_TX)
        .command(&ISM_SEND)
        .command(&TX_HALT)
        .kept(&[
            "devices",
            "zwave",
            "log",
            "unknown",
            "sigfox",
            "pulse_frames",
            "IsmFields",
            "DecodeFields",
            "SigfoxFinderFields",
            "IsmDetectFields.bursts_detected",
        ])
}

// DSRC road toll.

/// The result fields of the DSRC road toll stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DsrcFields<'a> {
    /// Frames with a good check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Broadcast frames.
    #[field(label = "Broadcast frames")]
    pub broadcast_frames: i64,
    /// Private link frames.
    #[field(label = "Private link frames")]
    pub private_frames: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The power of the input. It has no value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DsrcFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The class of the frame.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The bytes of the frame, without the check.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// The control byte as hex, if the frame has one.
    #[field(label = "Control byte")]
    pub control: Option<&'a str>,
    /// What is not shown.
    #[field(label = "Link identifier")]
    pub identifier: &'a str,
}

/// The fields of a frame event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DsrcFrameEvent<'a> {
    /// The class of the frame.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The bytes of the frame, without the check.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The frame log, newest first.
pub static DSRC_FRAMES: Table<DsrcFrameRow<'static>> =
    Table::new("frames", "Frames", 300).label(&DsrcFrameRow::IDENTIFIER);

/// A frame.
pub static DSRC_FRAME: EventKind<DsrcFrameEvent<'static>> =
    EventKind::new("dsrc_toll.frame", "DSRC frame");

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Described::new("ism", 2, 1)
            .added(
                2,
                1,
                &[
                    "ism.content",
                    "wmbus.content",
                    "zwave.content",
                    "zigbee.content",
                    "sigfox.content",
                    "ant.content",
                    "nrf24.content",
                    "subghzchat.content",
                    "dect.content",
                    "nfc.content",
                    "wisun.content",
                    "thread.content",
                    "mioty.content",
                    "dect_nrplus.content",
                    "npr70.content",
                    "dsrc_toll.content",
                ],
            )
            .content(sdrkit_core::ContentKind::Voice, &["dect"])
            .content(sdrkit_core::ContentKind::TextChat, &["subghzchat"])
            .content(sdrkit_core::ContentKind::Networks, &["npr70"])
            .content(
                sdrkit_core::ContentKind::Devices,
                &[
                    "ism",
                    "wmbus",
                    "zwave",
                    "zigbee",
                    "sigfox",
                    "ant",
                    "nrf24",
                    "nfc",
                    "wisun",
                    "thread",
                    "mioty",
                    "dect_nrplus",
                    "dsrc_toll",
                ],
            )
            .text(&["subghzchat"])
            .uses(sdrkit_proto_ookdev::components::interface())
            .stack(ism())
            .stack(wmbus())
            .stack(zwave())
            .stack(zigbee())
            .stack(sigfox())
            .stack(ant())
            .stack(nrf24())
            .stack(subghzchat())
            .stack(dect())
            .stack(nfc())
            .stack(wisun())
            .stack(thread())
            .stack(mioty())
            .stack(dect_nrplus())
            .stack(npr70())
            .stack(dsrc_toll())
    })
}

/// The DSRC road toll stack.
fn dsrc_toll() -> StackShape {
    StackShape::new(crate::dsrc::ID)
        .fields::<DsrcFields>()
        .fields::<DecoderFields>()
        .table(&DSRC_FRAMES)
        .event(&DSRC_FRAME)
        .settings(crate::dsrc::DsrcStackSetting::SETTINGS)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&[
            "frames",
            "DsrcFields.frames_ok",
            "DsrcFields.broadcast_frames",
            "DsrcFields.private_frames",
            "DsrcFields.frames_bad",
        ])
}

// New Packet Radio.

/// The result fields of the New Packet Radio stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NprFields<'a> {
    /// Sync words found.
    #[field(label = "Sync words")]
    pub sync_words: i64,
    /// The symbol rates searched, in ksym/s.
    #[field(label = "Rates searched", unit = "ksym/s")]
    pub rates_searched: &'a str,
    /// The power of the input. It has no value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NprFrameRow<'a> {
    /// The UTC of the frame, when the samples are live.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The network.
    #[field(label = "Network")]
    pub network: &'a str,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The sync word.
    #[field(label = "Sync word")]
    pub sync: &'a str,
    /// The tuning offset of the frame.
    #[field(label = "Tuning offset", unit = "Hz")]
    pub tuning_hz: f64,
    /// What the payload is.
    #[field(label = "Payload")]
    pub payload: &'a str,
}

/// A row of the networks heard. The row key is the network number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NprNetworkRow<'a> {
    /// The network.
    #[field(label = "Network")]
    pub network: &'a str,
    /// The frames heard.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The fields of a frame event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NprFrameEvent<'a> {
    /// The network.
    #[field(label = "Network")]
    pub network: &'a str,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The frames, newest first.
pub static NPR_FRAMES: Table<NprFrameRow<'static>> = Table::new("frames", "Frames", 300);
/// The networks heard.
pub static NPR_NETWORKS: Table<NprNetworkRow<'static>> = Table::new("networks", "Networks", 16);
/// A sync word.
pub static NPR_FRAME: EventKind<NprFrameEvent<'static>> =
    EventKind::new("npr70.frame", "New Packet Radio frame");

/// The New Packet Radio stack.
fn npr70() -> StackShape {
    StackShape::new(crate::npr70::ID)
        .fields::<NprFields>()
        .fields::<DecoderFields>()
        .table(&NPR_FRAMES)
        .table(&NPR_NETWORKS)
        .event(&NPR_FRAME)
        .settings(crate::npr70::NprSetting::SETTINGS)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&["frames", "networks", "NprFields.sync_words"])
}

// DECT NR+.

/// The result fields of the DECT NR+ stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrPlusFields<'a> {
    /// Packets read with a good check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Short training field detections.
    #[field(label = "STF detections")]
    pub stf_detections: i64,
    /// The devices in the table.
    #[field(label = "Devices")]
    pub devices: i64,
    /// The SNR of the last packet. It has no value before the first packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last packet. It has no value before the first packet.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub cfo_hz: Option<f64>,
    /// The power of the input. It has no value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the packet log. The row key is the number of the packet in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrPlusPacketRow<'a> {
    /// The UTC of the packet, when the samples are live.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the packet.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The device.
    #[field(label = "Id")]
    pub id: &'a str,
    /// The type of the packet.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The fields of the packet.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The carrier offset.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub cfo_hz: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// A row of the devices. The row key is the device number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrPlusDeviceRow<'a> {
    /// The transmitter.
    #[field(label = "Transmitter")]
    pub transmitter: &'a str,
    /// The device.
    #[field(label = "Id")]
    pub id: &'a str,
    /// The packets heard.
    #[field(label = "Packets")]
    pub packets: i64,
    /// The fields of the last packet.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The SNR of the last packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The UTC of the first packet.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last packet.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The fields of a packet event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrPlusPacketEvent<'a> {
    /// The device.
    #[field(label = "Id")]
    pub id: &'a str,
    /// The type of the packet.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The fields of the packet.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The packets, newest first.
pub static NRPLUS_PACKETS: Table<NrPlusPacketRow<'static>> =
    Table::new("packets", "Packets", 300).label(&NrPlusPacketRow::ID);
/// The devices, most recently heard first.
pub static NRPLUS_DEVICES: Table<NrPlusDeviceRow<'static>> =
    Table::new("devices", "Devices", 200).label(&NrPlusDeviceRow::ID);
/// A packet.
pub static NRPLUS_PACKET: EventKind<NrPlusPacketEvent<'static>> =
    EventKind::new("dect_nrplus.packet", "DECT NR+ packet");

/// The DECT NR+ stack.
fn dect_nrplus() -> StackShape {
    StackShape::new(crate::nrplus::ID)
        .fields::<NrPlusFields>()
        .fields::<DecoderFields>()
        .table(&NRPLUS_PACKETS)
        .table(&NRPLUS_DEVICES)
        .event(&NRPLUS_PACKET)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&[
            "packets",
            "devices",
            "NrPlusFields.frames_ok",
            "NrPlusFields.stf_detections",
            "NrPlusFields.devices",
        ])
}

// mioty.

/// The result fields of the mioty stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MiotyFields<'a> {
    /// Frames read.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bursts detected.
    #[field(label = "Bursts detected")]
    pub bursts_detected: i64,
    /// Patterns that did not decode.
    #[field(label = "Failed hypotheses")]
    pub failed_hypotheses: i64,
    /// The devices in the table.
    #[field(label = "Devices")]
    pub devices: i64,
    /// The SNR of the last frame. It has no value before the first frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The power of the input. It has no value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MiotyFrameRow<'a> {
    /// The UTC of the frame, when the samples are live.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The device.
    #[field(label = "Device")]
    pub id: &'a str,
    /// The pattern of the bursts.
    #[field(label = "Pattern")]
    pub pattern: &'a str,
    /// The bytes of the MPDU.
    #[field(label = "MPDU bytes")]
    pub psi: i64,
    /// The MAC mode.
    #[field(label = "MAC mode")]
    pub mmode: i64,
    /// The MAC.
    #[field(label = "MAC")]
    pub mac: &'a str,
    /// The first bytes of the MPDU as hex.
    #[field(label = "MPDU")]
    pub mpdu: &'a str,
    /// The frequency.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The bursts of the pattern that matched.
    #[field(label = "Bursts matched", unit = "of 24")]
    pub bursts_matched: i64,
}

/// A row of the devices. The row key is the device.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MiotyDeviceRow<'a> {
    /// The device.
    #[field(label = "Device")]
    pub id: &'a str,
    /// The frames heard.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The last packet counter.
    #[field(label = "Packet counter")]
    pub counter: i64,
    /// The SNR of the last frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The frequency of the last frame.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The fields of a frame event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MiotyFrameEvent<'a> {
    /// The device.
    #[field(label = "Device")]
    pub id: &'a str,
    /// The pattern of the bursts.
    #[field(label = "Pattern")]
    pub pattern: &'a str,
    /// The bytes of the MPDU.
    #[field(label = "MPDU bytes")]
    pub psi: i64,
    /// The MAC.
    #[field(label = "MAC")]
    pub mac: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The frames, newest first.
pub static MIOTY_FRAMES: Table<MiotyFrameRow<'static>> =
    Table::new("frames", "Frames", 200).label(&MiotyFrameRow::ID);
/// The devices, most recently heard first.
pub static MIOTY_DEVICES: Table<MiotyDeviceRow<'static>> =
    Table::new("devices", "Devices", 200).label(&MiotyDeviceRow::ID);
/// A frame.
pub static MIOTY_FRAME: EventKind<MiotyFrameEvent<'static>> =
    EventKind::new("mioty.frame", "mioty frame");

/// The mioty stack.
fn mioty() -> StackShape {
    StackShape::new(crate::mioty::ID)
        .fields::<MiotyFields>()
        .fields::<DecoderFields>()
        .table(&MIOTY_FRAMES)
        .table(&MIOTY_DEVICES)
        .event(&MIOTY_FRAME)
        .settings(crate::mioty::MiotySetting::SETTINGS)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&[
            "frames",
            "devices",
            "MiotyFields.frames_ok",
            "MiotyFields.bursts_detected",
            "MiotyFields.failed_hypotheses",
            "MiotyFields.devices",
        ])
}

// Wireless M-Bus.

/// The result fields of the Wireless M-Bus stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmbusFields {
    /// Frames whose CRCs passed.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames with a failed CRC.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The meters in the table.
    #[field(label = "Meters")]
    pub meters: i64,
    /// Syncs found.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Syncs that did not give a frame.
    #[field(label = "False syncs")]
    pub false_syncs: i64,
    /// Frames with an encrypted payload.
    #[field(label = "Encrypted frames")]
    pub encrypted_frames: i64,
}

/// A row of the meters. The row key is the maker, the id and the type.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmbusMeterRow<'a> {
    /// The manufacturer letters.
    #[field(label = "Maker")]
    pub maker: &'a str,
    /// The meter id (8 digits).
    #[field(label = "ID")]
    pub meter_id: &'a str,
    /// The device type name.
    #[field(label = "Type")]
    pub device_type: &'a str,
    /// The physical mode of the last frame ("S1", "T1", "C1 (A)" ...).
    #[field(label = "Mode")]
    pub phy: &'a str,
    /// The channel of the last frame.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The payload is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The summary of the last frame.
    #[field(label = "Last reading")]
    pub last_reading: &'a str,
    /// The application status.
    #[field(label = "Status")]
    pub status: &'a str,
}

/// A row of the data records of the last frame of each meter. The row key is the row key of the meter, "/" and the index.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmbusRecordRow<'a> {
    /// The row key of the meter.
    #[field(label = "Meter", refers_to = "meters")]
    pub meter: &'a str,
    /// The place of the record in its frame, from 0.
    #[field(label = "Index")]
    pub index: i64,
    /// The quantity ("Volume").
    #[field(label = "Quantity")]
    pub quantity: &'a str,
    /// The value with its unit ("123.456 m3").
    #[field(label = "Value")]
    pub value: &'a str,
    /// The notes ("storage 1"), if any.
    #[field(label = "Notes")]
    pub notes: Option<&'a str>,
}

/// A row of the frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmbusFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The UTC of the frame, when the samples are live.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The physical mode.
    #[field(label = "Mode")]
    pub phy: &'a str,
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The meter ("KAM 12345678"), if the frame names one.
    #[field(label = "Meter")]
    pub meter: Option<&'a str>,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Every CRC of the frame passed.
    #[field(label = "CRC passed")]
    pub crc_ok: bool,
    /// The payload is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The summary of the frame.
    #[field(label = "Result")]
    pub result: &'a str,
    /// The frame bytes in hex.
    #[field(label = "Frame")]
    pub frame: &'a str,
}

/// A row of the channels. The row key is the frequency in MHz and the modes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmbusChannelRow<'a> {
    /// The channel frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The physical modes on the channel.
    #[field(label = "Mode")]
    pub phy: &'a str,
    /// The slow power of the channel.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The noise floor of the channel.
    #[field(label = "Noise floor", unit = "dBFS")]
    pub noise_dbfs: f64,
    /// Good frames on the channel.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Frames with a failed CRC on the channel.
    #[field(label = "CRC failures")]
    pub crc_failures: i64,
}

/// The fields of a frame event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmbusFrameEvent<'a> {
    /// The meter: maker and id ("KAM 12345678").
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The manufacturer letters.
    #[field(label = "Maker")]
    pub maker: &'a str,
    /// The meter id.
    #[field(label = "ID")]
    pub meter_id: &'a str,
    /// The device type name.
    #[field(label = "Type")]
    pub device_type: &'a str,
    /// The physical mode.
    #[field(label = "Mode")]
    pub phy: &'a str,
    /// The payload is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The meters, most recently heard first.
pub static WMBUS_METERS: Table<WmbusMeterRow<'static>> = Table::new("meters", "Meters", 300);
/// The data records of the last frame of each meter.
pub static WMBUS_RECORDS: Table<WmbusRecordRow<'static>> =
    Table::new("meter_records", "Meter records", 3000);
/// The frame log, newest first.
pub static WMBUS_FRAMES: Table<WmbusFrameRow<'static>> = Table::new("frames", "Frames", 200);
/// The channels of the plan in the capture.
pub static WMBUS_CHANNELS: Table<WmbusChannelRow<'static>> = Table::new("channels", "Channels", 16);
/// A frame decoded.
pub static WMBUS_FRAME: EventKind<WmbusFrameEvent<'static>> =
    EventKind::new("wmbus.frame", "Wireless M-Bus frame");
/// The level of the strongest channel, one value a second.
pub const WMBUS_LEVEL_HISTORY: SeriesDef = SeriesDef {
    key: "level_history",
    label: "Level of the strongest channel",
    x_unit: "s",
    y_unit: "dBFS",
};
/// The soft chips of the last frame.
pub const WMBUS_TRACE: SeriesDef = SeriesDef {
    key: "trace",
    label: "Soft chips of the last frame",
    x_unit: "chip",
    y_unit: "",
};

/// The row key of a meter.
pub const WMBUS_METER_ROW: Argument<field::Text> = objcmd::row_of("meters");

/// The arguments of `remove` of the Wireless M-Bus stack.
#[derive(Arguments)]
pub struct WmbusMeterArgs<'a> {
    /// The row key of the meter.
    #[arg(shared = WMBUS_METER_ROW)]
    pub row: &'a str,
}

/// Forgets one meter and its records.
pub const WMBUS_REMOVE: CommandKind<WmbusMeterArgs<'static>> = objcmd::remove_kind();

/// The arguments of `send` of the Wireless M-Bus stack.
#[derive(Arguments)]
pub struct WmbusSendArgs<'a> {
    /// The physical mode and frame format.
    #[arg(shared = crate::wmbus::send::MODE)]
    pub mode: Option<crate::wmbus::send::WmbusMode>,
    /// The manufacturer.
    #[arg(
        label = "Manufacturer",
        help = "Three letters A to Z, as the meters table shows them."
    )]
    pub manufacturer: &'a str,
    /// The address (identification number).
    #[arg(
        label = "Address",
        help = "8 hex digits, as the meters table shows the id."
    )]
    pub address: &'a str,
    /// The version.
    #[arg(label = "Version", help = "0 to 255.")]
    pub version: i64,
    /// The device type.
    #[arg(
        label = "Device type",
        help = "0 to 255, for example 7 for water or 2 for electricity."
    )]
    pub device_type: i64,
    /// The application data.
    #[arg(
        label = "Payload",
        help = "The CI field and the data after it as pairs of hex digits, spaces allowed: at most 246 octets in \
                format A, 242 in format B."
    )]
    pub payload: &'a str,
}

/// Sends a Wireless M-Bus frame.
pub const WMBUS_SEND: CommandKind<WmbusSendArgs<'static>> = CommandKind::new("send", "Send a frame")
    .help(
        "Sends a link-layer frame with the C field 0x44 (SND-NR) on the channel of the mode nearest the tuned \
         frequency, with the CRCs of its format. Frames sent during a frame wait for it. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("halt");
/// A Wireless M-Bus transmission.
pub static WMBUS_TX: EventKind<IsmTxEvent<'static>> =
    EventKind::new("wmbus.tx", "Wireless M-Bus transmission");

/// The Wireless M-Bus stack.
fn wmbus() -> StackShape {
    StackShape::new("wmbus")
        .fields::<WmbusFields>()
        .receiver_fields::<GuardFields>()
        .table(&WMBUS_METERS)
        .table(&WMBUS_RECORDS)
        .table(&WMBUS_FRAMES)
        .table(&WMBUS_CHANNELS)
        .series(WMBUS_LEVEL_HISTORY)
        .series(WMBUS_TRACE)
        .event(&WMBUS_FRAME)
        .settings(crate::wmbus::WmbusSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&WMBUS_REMOVE)
        .fields::<IsmTxFields>()
        .event(&WMBUS_TX)
        .command(&WMBUS_SEND)
        .command(&TX_HALT)
        .kept(&["meters", "meter_records", "frames", "WmbusFields"])
}

// Z-Wave.

/// The result fields of the Z-Wave stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveFields {
    /// Frames decoded at every rate.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bursts whose check failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The networks in the table.
    #[field(label = "Networks")]
    pub networks: i64,
    /// The nodes in the table.
    #[field(label = "Nodes")]
    pub nodes: i64,
    /// Frames at R1.
    #[field(label = "R1 frames")]
    pub frames_r1: i64,
    /// Frames at R2.
    #[field(label = "R2 frames")]
    pub frames_r2: i64,
    /// Frames at R3.
    #[field(label = "R3 frames")]
    pub frames_r3: i64,
    /// Security S0 frames.
    #[field(label = "S0 frames")]
    pub s0_frames: i64,
    /// Security S2 frames.
    #[field(label = "S2 frames")]
    pub s2_frames: i64,
    /// Acknowledgements.
    #[field(label = "Acks")]
    pub acks: i64,
}

/// A row of the Z-Wave frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The UTC of the frame, when the samples are live.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The channel frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The rate ("R1", "R2", "R3").
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// The home id (8 hex digits).
    #[field(label = "Home ID")]
    pub home_id: &'a str,
    /// Source and destination ("2 > 1").
    #[field(label = "Source > destination")]
    pub route: &'a str,
    /// The header type.
    #[field(label = "Type")]
    pub frame_type: &'a str,
    /// The sequence number.
    #[field(label = "Seq")]
    pub seq: i64,
    /// An acknowledgement is requested.
    #[field(label = "Ack requested")]
    pub ack_requested: bool,
    /// The encryption ("none", "S0", "S2").
    #[field(label = "Encryption")]
    pub encryption: &'a str,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The SNR of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The one-line description.
    #[field(label = "Command")]
    pub command: &'a str,
    /// The MPDU in hex.
    #[field(label = "MPDU")]
    pub mpdu: &'a str,
}

/// A row of the Z-Wave nodes. The row key is the home id and the node id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveNodeRow<'a> {
    /// The row key of the network of the node.
    #[field(label = "Home ID", refers_to = "networks")]
    pub home_id: &'a str,
    /// The node id.
    #[field(label = "Node")]
    pub node: i64,
    /// "controller" or "node".
    #[field(label = "Role")]
    pub role: &'a str,
    /// Frames the node sent.
    #[field(label = "Sent")]
    pub sent: i64,
    /// Frames addressed to the node.
    #[field(label = "Addressed")]
    pub addressed: i64,
    /// Encrypted frames the node sent.
    #[field(label = "Encrypted")]
    pub encrypted: i64,
    /// The rate of the last frame.
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The last command class the node sent.
    #[field(label = "Last command class")]
    pub last_class: &'a str,
    /// The node uses beaming.
    #[field(label = "Beaming")]
    pub beaming: bool,
    /// The node sent at low power.
    #[field(label = "Low power")]
    pub low_power: bool,
}

/// A row of the Z-Wave networks. The row key is the home id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveNetworkRow<'a> {
    /// The home id.
    #[field(label = "Home ID")]
    pub home_id: &'a str,
    /// The nodes heard.
    #[field(label = "Nodes")]
    pub nodes: i64,
    /// Node 1 of the network has sent.
    #[field(label = "Controller heard")]
    pub controller: bool,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// Encrypted frames.
    #[field(label = "Encrypted")]
    pub encrypted: i64,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the Z-Wave channels in use. The row key is the frequency and the rate.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveChannelRow<'a> {
    /// The channel frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The rate.
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// The channel is in the band taken in.
    #[field(label = "In the band taken in")]
    pub covered: bool,
    /// Frames decoded on the channel.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Bursts whose check failed on the channel.
    #[field(label = "Failed checks")]
    pub failed_checks: i64,
}

/// A row of the command class counts. The row key is the name of the class.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveClassRow<'a> {
    /// The command class.
    #[field(label = "Command class")]
    pub class: &'a str,
    /// How often it was sent.
    #[field(label = "Count")]
    pub count: i64,
}

/// The fields of a Z-Wave frame event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveFrameEvent<'a> {
    /// The sender: home id and node ("C0FFEE01:2").
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The addressee, for a frame to one node.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The home id.
    #[field(label = "Home ID")]
    pub home_id: &'a str,
    /// Source and destination.
    #[field(label = "Source > destination")]
    pub route: &'a str,
    /// The rate.
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// The header type.
    #[field(label = "Type")]
    pub frame_type: &'a str,
    /// The encryption.
    #[field(label = "Encryption")]
    pub encryption: &'a str,
    /// The one-line description.
    #[field(label = "Command")]
    pub command: &'a str,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The SNR of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// The fields of the event that Z-Wave frames start or stop coming in.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZwaveStateEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The Z-Wave frame log, newest first.
pub static ZWAVE_FRAMES: Table<ZwaveFrameRow<'static>> = Table::new("frames", "Frames", 600);
/// The Z-Wave nodes, most recently heard first.
pub static ZWAVE_NODES: Table<ZwaveNodeRow<'static>> = Table::new("nodes", "Nodes", 1000);
/// The Z-Wave networks, most recently heard first.
pub static ZWAVE_NETWORKS: Table<ZwaveNetworkRow<'static>> = Table::new("networks", "Networks", 64)
    .sorted(
        &ZwaveNetworkRow::LEVEL_DBFS,
        sdrkit_core::SortOrder::Descending,
    );
/// The Z-Wave channels in use.
pub static ZWAVE_CHANNELS: Table<ZwaveChannelRow<'static>> = Table::new("channels", "Channels", 32);
/// The command class counts.
pub static ZWAVE_CLASSES: Table<ZwaveClassRow<'static>> =
    Table::new("classes", "Command classes", 256);
/// A Z-Wave frame decoded.
pub static ZWAVE_FRAME: EventKind<ZwaveFrameEvent<'static>> =
    EventKind::new("zwave.frame", "Z-Wave frame");
/// Z-Wave frames start or stop coming in.
pub static ZWAVE_STATE: EventKind<ZwaveStateEvent<'static>> =
    EventKind::new("zwave.state", "Z-Wave activity");

/// The row key of a Z-Wave node.
pub const ZWAVE_NODE_ROW: Argument<field::Text> = objcmd::row_of("nodes");

/// The arguments of `remove` of the Z-Wave stack.
#[derive(Arguments)]
pub struct ZwaveNodeArgs<'a> {
    /// The row key of the node.
    #[arg(shared = ZWAVE_NODE_ROW)]
    pub row: &'a str,
}

/// Forgets one node.
pub const ZWAVE_REMOVE: CommandKind<ZwaveNodeArgs<'static>> = objcmd::remove_kind();

/// The arguments of `send` of the Z-Wave stack.
#[derive(Arguments)]
pub struct ZwaveSendArgs<'a> {
    /// The home id.
    #[arg(
        label = "Home ID",
        help = "8 hex digits, as the networks table shows it."
    )]
    pub home_id: &'a str,
    /// The source node.
    #[arg(label = "Source node", help = "1 to 232.")]
    pub source: i64,
    /// The destination node.
    #[arg(label = "Destination node", help = "1 to 232, or 255 for every node.")]
    pub destination: i64,
    /// The application command.
    #[arg(
        label = "Command",
        help = "The command class, the command and its parameters as pairs of hex digits (spaces allowed): 1 to 54 \
                octets at R1 and R2, 1 to 159 at R3."
    )]
    pub payload: &'a str,
    /// The rate.
    #[arg(shared = crate::zwave::send::RATE)]
    pub rate: Option<crate::zwave::send::ZwaveRate>,
    /// Whether the frame asks for an acknowledgement.
    #[arg(
        label = "Ack requested",
        help = "Sets bit 6 of frame control 1.",
        default = false
    )]
    pub ack: Option<bool>,
    /// The sequence number.
    #[arg(
        label = "Sequence",
        help = "0 to 15, in bits 3 to 0 of frame control 2.",
        default = 0,
        min = 0,
        max = 15,
        step = 1
    )]
    pub sequence: Option<i64>,
}

/// Sends a Z-Wave singlecast frame.
pub const ZWAVE_SEND: CommandKind<ZwaveSendArgs<'static>> = CommandKind::new("send", "Send a frame")
    .help(
        "Sends a singlecast MPDU on the first channel of the rate in the setting channels (the tuned frequency when \
         the setting is empty). Frames sent during a frame wait for it. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("halt");
/// A Z-Wave transmission.
pub static ZWAVE_TX: EventKind<IsmTxEvent<'static>> =
    EventKind::new("zwave.tx", "Z-Wave transmission");

/// The Z-Wave stack.
fn zwave() -> StackShape {
    StackShape::new("zwave")
        .fields::<ZwaveFields>()
        .receiver_fields::<GuardFields>()
        .table(&ZWAVE_FRAMES)
        .table(&ZWAVE_NODES)
        .table(&ZWAVE_NETWORKS)
        .table(&ZWAVE_CHANNELS)
        .table(&ZWAVE_CLASSES)
        .event(&ZWAVE_FRAME)
        .event(&ZWAVE_STATE)
        .settings(crate::zwave::ZwaveSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&ZWAVE_REMOVE)
        .fields::<IsmTxFields>()
        .event(&ZWAVE_TX)
        .command(&ZWAVE_SEND)
        .command(&TX_HALT)
        .kept(&[
            "frames",
            "nodes",
            "networks",
            "classes",
            "ZwaveFields.frames_ok",
            "ZwaveFields.frames_bad",
            "ZwaveFields.networks",
            "ZwaveFields.nodes",
            "ZwaveFields.frames_r1",
            "ZwaveFields.frames_r2",
            "ZwaveFields.frames_r3",
            "ZwaveFields.s0_frames",
            "ZwaveFields.s2_frames",
            "ZwaveFields.acks",
        ])
}

// IEEE 802.15.4: Zigbee and Thread.

/// A row of the devices of an IEEE 802.15.4 stack. The row key is the number of the device.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PanDeviceRow<'a> {
    /// The short address, if heard.
    #[field(label = "Short")]
    pub short: Option<&'a str>,
    /// The extended address, if heard.
    #[field(label = "Extended")]
    pub extended: Option<&'a str>,
    /// The vendor of the extended address.
    #[field(label = "Vendor")]
    pub vendor: Option<&'a str>,
    /// The PAN id, if known.
    #[field(label = "PAN ID")]
    pub pan: Option<&'a str>,
    /// The channel.
    #[field(label = "Ch")]
    pub ch: i64,
    /// "coordinator", "router" or "device".
    #[field(label = "Role")]
    pub role: &'a str,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The description of the last frame.
    #[field(label = "Last frame")]
    pub last_frame: &'a str,
}

/// The devices of an IEEE 802.15.4 stack, most recently heard first.
pub static PAN_DEVICES: Table<PanDeviceRow<'static>> = Table::new("devices", "Devices", 600);

/// The row key of a device of an IEEE 802.15.4 stack.
pub const PAN_DEVICE_ROW: Argument<field::Text> = objcmd::row_of("devices");

/// The arguments of `remove` of an IEEE 802.15.4 stack.
#[derive(Arguments)]
pub struct PanDeviceArgs<'a> {
    /// The row key of the device.
    #[arg(shared = PAN_DEVICE_ROW)]
    pub row: &'a str,
}

/// Forgets one device of an IEEE 802.15.4 stack.
pub const PAN_REMOVE: CommandKind<PanDeviceArgs<'static>> = objcmd::remove_kind();

/// The arguments of `send` of the Zigbee stack.
#[derive(Arguments)]
pub struct ZigbeeSendArgs<'a> {
    /// The PAN id.
    #[arg(
        label = "PAN ID",
        help = "4 hex digits, as the networks table shows it without 0x."
    )]
    pub pan_id: &'a str,
    /// The destination address.
    #[arg(
        label = "Destination",
        help = "4 hex digits for a short address (FFFF is broadcast), 16 for an extended address, empty for none."
    )]
    pub destination: &'a str,
    /// The source address.
    #[arg(
        label = "Source",
        help = "4 hex digits for a short address, 16 for an extended address, empty for none."
    )]
    pub source: &'a str,
    /// The MAC payload.
    #[arg(
        label = "Payload",
        help = "The MAC payload (for a data frame the Zigbee NWK frame) as pairs of hex digits, spaces allowed; the \
                whole frame with its header and FCS is at most 127 octets."
    )]
    pub payload: &'a str,
    /// The frame type.
    #[arg(shared = crate::zigbee::send::FRAME_TYPE)]
    pub frame_type: Option<crate::zigbee::send::ZigbeeFrameType>,
    /// The sequence number.
    #[arg(
        label = "Sequence",
        help = "0 to 255.",
        default = 0,
        min = 0,
        max = 255,
        step = 1
    )]
    pub sequence: Option<i64>,
    /// Whether the frame asks for an acknowledgement.
    #[arg(
        label = "Ack requested",
        help = "Sets the acknowledgement request bit.",
        default = false
    )]
    pub ack: Option<bool>,
    /// The security level.
    #[arg(
        label = "Security level",
        help = "0 for none. 1 to 3: a MIC of 4, 8 or 16 octets; 4: encryption only; 5 to 7: encryption and a MIC of 4, 8 \
                or 16 octets. A secured frame needs the key and an extended source address.",
        default = 0,
        min = 0,
        max = 7,
        step = 1
    )]
    pub security_level: Option<i64>,
    /// The key of a secured frame.
    #[arg(
        label = "Key",
        help = "The AES-128 key of a secured frame as 32 hex digits.",
        default = ""
    )]
    pub key: Option<&'a str>,
    /// The frame counter of a secured frame.
    #[arg(
        label = "Frame counter",
        help = "The frame counter of the auxiliary security header: 0 to 4294967295.",
        default = 0,
        min = 0,
        max = 4_294_967_295i64,
        step = 1
    )]
    pub frame_counter: Option<i64>,
    /// The key identifier mode of a secured frame.
    #[arg(
        label = "Key identifier mode",
        help = "0: the key is known from the addresses; 1: a key index; 2: a 4-octet key source and a key index; 3: an \
                8-octet key source and a key index.",
        default = 1,
        min = 0,
        max = 3,
        step = 1
    )]
    pub key_id_mode: Option<i64>,
    /// The key index of a secured frame.
    #[arg(
        label = "Key index",
        help = "Key identifier modes 1 to 3: 1 to 255.",
        default = 1,
        min = 1,
        max = 255,
        step = 1
    )]
    pub key_index: Option<i64>,
    /// The key source of a secured frame.
    #[arg(
        label = "Key source",
        help = "Key identifier mode 2: 8 hex digits; mode 3: 16 hex digits; sent in the order given.",
        default = ""
    )]
    pub key_source: Option<&'a str>,
}

/// Sends an IEEE 802.15.4 MAC frame.
pub const ZIGBEE_SEND: CommandKind<ZigbeeSendArgs<'static>> = CommandKind::new("send", "Send a frame")
    .help(
        "Sends an IEEE 802.15.4 MAC frame on the tuned frequency: frame version 2003 without security, frame version 2006 \
         with the auxiliary security header and CCM* at the security level. With both addresses the source PAN id is \
         left out. Frames sent during a frame wait for it. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("halt");
/// A Zigbee transmission.
pub static ZIGBEE_TX: EventKind<IsmTxEvent<'static>> =
    EventKind::new("zigbee.tx", "Zigbee transmission");

/// The result fields of the Zigbee stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZigbeeFields {
    /// Good frames.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames whose SFD was found and whose FCS failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Preamble candidates.
    #[field(label = "Preamble candidates")]
    pub candidates: i64,
    /// Frames with an encrypted payload.
    #[field(label = "Encrypted frames")]
    pub encrypted_frames: i64,
    /// The networks in the table.
    #[field(label = "Networks")]
    pub networks: i64,
    /// The devices in the table.
    #[field(label = "Devices")]
    pub devices: i64,
    /// The channel listened to (0 when not known).
    #[field(label = "Channel")]
    pub channel: i64,
    /// Good frames in the last second.
    #[field(label = "Frames in the last second")]
    pub frames_per_s: i64,
    /// The SNR of the last frame since the last change of frequency.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last frame since the last change of frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A row of the Zigbee networks. The row key is the PAN and the channel as one number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZigbeeNetworkRow<'a> {
    /// The PAN id.
    #[field(label = "PAN ID")]
    pub pan: &'a str,
    /// The extended PAN id, if a beacon gave it.
    #[field(label = "Extended PAN ID")]
    pub ext_pan: Option<&'a str>,
    /// The channel.
    #[field(label = "Ch")]
    pub ch: i64,
    /// The coordinator, if known.
    #[field(label = "Coordinator")]
    pub coordinator: Option<&'a str>,
    /// The devices in the PAN.
    #[field(label = "Devices")]
    pub devices: i64,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the Zigbee frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZigbeeFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The channel.
    #[field(label = "Ch")]
    pub ch: i64,
    /// The MAC frame type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// "source > destination".
    #[field(label = "Source > destination")]
    pub route: &'a str,
    /// The PAN id, if known.
    #[field(label = "PAN ID")]
    pub pan: Option<&'a str>,
    /// The PSDU length with the FCS.
    #[field(label = "Len")]
    pub len: i64,
    /// Some payload is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The description.
    #[field(label = "Details")]
    pub details: &'a str,
}

/// The fields of a new PAN event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ZigbeePanEvent<'a> {
    /// The PAN id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The PAN id.
    #[field(label = "PAN ID")]
    pub pan: &'a str,
    /// The channel.
    #[field(label = "Ch")]
    pub ch: i64,
}

/// The Zigbee networks, most recently heard first.
pub static ZIGBEE_NETWORKS: Table<ZigbeeNetworkRow<'static>> =
    Table::new("networks", "Networks", 100).sorted(
        &ZigbeeNetworkRow::LEVEL_DBFS,
        sdrkit_core::SortOrder::Descending,
    );
/// The Zigbee frame log, newest first.
pub static ZIGBEE_FRAMES: Table<ZigbeeFrameRow<'static>> = Table::new("frames", "Frames", 600);
/// A new Zigbee PAN.
pub static ZIGBEE_PAN: EventKind<ZigbeePanEvent<'static>> =
    EventKind::new("zigbee.pan", "New Zigbee PAN");
/// The good frames of each of the last seconds.
pub const ZIGBEE_RATE: SeriesDef = SeriesDef {
    key: "frames_per_second",
    label: "Frames per second",
    x_unit: "s",
    y_unit: "frames",
};

/// The Zigbee stack.
fn zigbee() -> StackShape {
    StackShape::new("zigbee")
        .fields::<ZigbeeFields>()
        .receiver_fields::<GuardFields>()
        .table(&ZIGBEE_NETWORKS)
        .table(&PAN_DEVICES)
        .table(&ZIGBEE_FRAMES)
        .series(ZIGBEE_RATE)
        .event(&ZIGBEE_PAN)
        .settings(crate::zigbee::ZigbeeSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&PAN_REMOVE)
        .fields::<IsmTxFields>()
        .event(&ZIGBEE_TX)
        .command(&ZIGBEE_SEND)
        .command(&TX_HALT)
        .kept(&[
            "networks",
            "devices",
            "frames",
            "ZigbeeFields.frames_ok",
            "ZigbeeFields.frames_bad",
            "ZigbeeFields.candidates",
            "ZigbeeFields.encrypted_frames",
            "ZigbeeFields.networks",
            "ZigbeeFields.devices",
        ])
}

/// The result fields of the Thread stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ThreadFields {
    /// Good frames.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Candidates whose check failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Frames that the link layer encrypts.
    #[field(label = "Encrypted frames")]
    pub encrypted_frames: i64,
    /// Beacon frames.
    #[field(label = "Beacon frames")]
    pub beacons: i64,
    /// Data frames.
    #[field(label = "Data frames")]
    pub data_frames: i64,
    /// Acknowledgements.
    #[field(label = "Acknowledgements")]
    pub acks: i64,
    /// Command frames.
    #[field(label = "Command frames")]
    pub commands: i64,
    /// Good frames on channel 11.
    #[field(label = "Channel 11 frames")]
    pub ch11: i64,
    /// Good frames on channel 12.
    #[field(label = "Channel 12 frames")]
    pub ch12: i64,
    /// Good frames on channel 13.
    #[field(label = "Channel 13 frames")]
    pub ch13: i64,
    /// Good frames on channel 14.
    #[field(label = "Channel 14 frames")]
    pub ch14: i64,
    /// Good frames on channel 15.
    #[field(label = "Channel 15 frames")]
    pub ch15: i64,
    /// Good frames on channel 16.
    #[field(label = "Channel 16 frames")]
    pub ch16: i64,
    /// Good frames on channel 17.
    #[field(label = "Channel 17 frames")]
    pub ch17: i64,
    /// Good frames on channel 18.
    #[field(label = "Channel 18 frames")]
    pub ch18: i64,
    /// Good frames on channel 19.
    #[field(label = "Channel 19 frames")]
    pub ch19: i64,
    /// Good frames on channel 20.
    #[field(label = "Channel 20 frames")]
    pub ch20: i64,
    /// Good frames on channel 21.
    #[field(label = "Channel 21 frames")]
    pub ch21: i64,
    /// Good frames on channel 22.
    #[field(label = "Channel 22 frames")]
    pub ch22: i64,
    /// Good frames on channel 23.
    #[field(label = "Channel 23 frames")]
    pub ch23: i64,
    /// Good frames on channel 24.
    #[field(label = "Channel 24 frames")]
    pub ch24: i64,
    /// Good frames on channel 25.
    #[field(label = "Channel 25 frames")]
    pub ch25: i64,
    /// Good frames on channel 26.
    #[field(label = "Channel 26 frames")]
    pub ch26: i64,
    /// Good frames in the last second.
    #[field(label = "Frames in the last second")]
    pub frames_per_s: i64,
    /// The channel listened to (0 when not known).
    #[field(label = "Channel")]
    pub channel: i64,
    /// The stack steps through the channels.
    #[field(label = "Scanning")]
    pub scanning: bool,
    /// The networks in the table.
    #[field(label = "Networks")]
    pub networks: i64,
    /// The devices in the table.
    #[field(label = "Devices")]
    pub devices: i64,
    /// The level of the last frame since the last change of frequency.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The SNR of the last frame since the last change of frequency.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last frame since the last change of frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A row of the Thread networks. The row key is the PAN and the channel as one number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ThreadNetworkRow<'a> {
    /// The PAN id.
    #[field(label = "PAN ID")]
    pub pan: &'a str,
    /// The channel where it was heard (0 when not known).
    #[field(label = "Ch")]
    pub ch: i64,
    /// The network name of a Thread beacon.
    #[field(label = "Network name")]
    pub network_name: Option<&'a str>,
    /// The extended PAN id.
    #[field(label = "Extended PAN ID")]
    pub ext_pan: Option<&'a str>,
    /// The Thread version of the beacon.
    #[field(label = "Thread version")]
    pub version: Option<i64>,
    /// The last beacon permitted joining.
    #[field(label = "Joining open")]
    pub joining_open: bool,
    /// A Thread beacon or a 6LoWPAN header was seen in the PAN.
    #[field(label = "Thread seen")]
    pub thread_seen: bool,
    /// The sender of the beacon.
    #[field(label = "Coordinator")]
    pub coordinator: Option<&'a str>,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the Thread frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ThreadFrameRow<'a> {
    /// The UTC of the frame, when the samples are live.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The channel.
    #[field(label = "Ch")]
    pub ch: i64,
    /// The MAC frame type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The source address.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The destination address.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The PAN id, if known.
    #[field(label = "PAN ID")]
    pub pan: Option<&'a str>,
    /// The PSDU length with the FCS.
    #[field(label = "Len")]
    pub len: i64,
    /// The link layer encrypts the payload.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The sequence number, if the frame has one.
    #[field(label = "Sequence number")]
    pub seq: Option<i64>,
    /// The frame requests an acknowledgement.
    #[field(label = "Acknowledge request")]
    pub ack_request: bool,
    /// The description.
    #[field(label = "Details")]
    pub details: &'a str,
}

/// The fields of a new Thread network event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ThreadNetworkEvent<'a> {
    /// The PAN id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The network name.
    #[field(label = "Network name")]
    pub network_name: &'a str,
    /// The extended PAN id.
    #[field(label = "Extended PAN ID")]
    pub ext_pan: &'a str,
    /// The channel.
    #[field(label = "Ch")]
    pub ch: i64,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a new Thread device event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ThreadDeviceEvent<'a> {
    /// The address of the device.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The PAN id, if known.
    #[field(label = "PAN ID")]
    pub pan: Option<&'a str>,
    /// The channel.
    #[field(label = "Ch")]
    pub ch: i64,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The Thread networks, most recently heard first.
pub static THREAD_NETWORKS: Table<ThreadNetworkRow<'static>> =
    Table::new("networks", "Networks", 100)
        .label(&ThreadNetworkRow::NETWORK_NAME)
        .sorted(
            &ThreadNetworkRow::LEVEL_DBFS,
            sdrkit_core::SortOrder::Descending,
        );
/// The Thread frame log, newest first.
pub static THREAD_FRAMES: Table<ThreadFrameRow<'static>> =
    Table::new("frames", "Frames", 600).label(&ThreadFrameRow::SOURCE);
/// A new PAN with a Thread beacon.
pub static THREAD_NETWORK: EventKind<ThreadNetworkEvent<'static>> =
    EventKind::new("thread.network", "Thread network");
/// A new Thread device.
pub static THREAD_DEVICE: EventKind<ThreadDeviceEvent<'static>> =
    EventKind::new("thread.device", "Thread device");

/// The Thread stack.
fn thread() -> StackShape {
    StackShape::new("thread")
        .fields::<ThreadFields>()
        .receiver_fields::<GuardFields>()
        .table(&THREAD_NETWORKS)
        .table(&PAN_DEVICES)
        .table(&THREAD_FRAMES)
        .event(&THREAD_NETWORK)
        .event(&THREAD_DEVICE)
        .settings(crate::thread::ThreadSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&PAN_REMOVE)
        .kept(&[
            "networks",
            "devices",
            "frames",
            "ThreadFields.frames_ok",
            "ThreadFields.frames_bad",
            "ThreadFields.encrypted_frames",
            "ThreadFields.beacons",
            "ThreadFields.data_frames",
            "ThreadFields.acks",
            "ThreadFields.commands",
            "ThreadFields.ch11",
            "ThreadFields.ch12",
            "ThreadFields.ch13",
            "ThreadFields.ch14",
            "ThreadFields.ch15",
            "ThreadFields.ch16",
            "ThreadFields.ch17",
            "ThreadFields.ch18",
            "ThreadFields.ch19",
            "ThreadFields.ch20",
            "ThreadFields.ch21",
            "ThreadFields.ch22",
            "ThreadFields.ch23",
            "ThreadFields.ch24",
            "ThreadFields.ch25",
            "ThreadFields.ch26",
            "ThreadFields.networks",
            "ThreadFields.devices",
        ])
}

// Wi-SUN FAN.

/// The result fields of the Wi-SUN FAN stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WisunFields {
    /// Good frames.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bad frames.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Frames corrected by the least-reliable-bit search.
    #[field(label = "Corrected frames")]
    pub corrected_frames: i64,
    /// Frames with an FEC SFD, counted and not decoded.
    #[field(label = "FEC frames")]
    pub fec_frames: i64,
    /// Mode switches.
    #[field(label = "Mode switches")]
    pub mode_switches: i64,
    /// Secured frames.
    #[field(label = "Encrypted frames")]
    pub encrypted_frames: i64,
    /// The networks in the table.
    #[field(label = "Networks")]
    pub networks: i64,
    /// The nodes in the table.
    #[field(label = "Nodes")]
    pub nodes: i64,
    /// The links in the table.
    #[field(label = "Links")]
    pub links: i64,
}

/// A row of the Wi-SUN networks. The row key is the PAN id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WisunNetworkRow<'a> {
    /// The network name, once heard.
    #[field(label = "Network name")]
    pub name: Option<&'a str>,
    /// The border router EUI-64, once known.
    #[field(label = "Border router")]
    pub border_router: Option<&'a str>,
    /// The PAN size, once heard.
    #[field(label = "Size")]
    pub size: Option<i64>,
    /// The nodes in the PAN.
    #[field(label = "Nodes")]
    pub nodes: i64,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The unicast schedule, once heard.
    #[field(label = "Schedule")]
    pub schedule: Option<&'a str>,
}

/// A row of the Wi-SUN nodes. The row key is the EUI-64.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WisunNodeRow<'a> {
    /// The vendor of the EUI-64.
    #[field(label = "Vendor")]
    pub vendor: Option<&'a str>,
    /// The role: node, LFN, joining, router or border router.
    #[field(label = "Role")]
    pub role: &'a str,
    /// The row key of the network of the node, once known.
    #[field(label = "PAN", refers_to = "networks")]
    pub pan: Option<&'a str>,
    /// The routing cost, once heard.
    #[field(label = "Cost")]
    pub cost: Option<i64>,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Frames sent.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The last channel centre.
    #[field(label = "Channel", unit = "Hz")]
    pub freq: Hertz,
    /// The authenticator EUI-64, once heard.
    #[field(label = "Authenticator")]
    pub authenticator: Option<&'a str>,
    /// The EAP state, once heard.
    #[field(label = "EAP")]
    pub eap: Option<&'a str>,
    /// The summary of the last frame.
    #[field(label = "Last frame")]
    pub last: &'a str,
}

/// A row of the Wi-SUN links. The row key is the two ends.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WisunLinkRow<'a> {
    /// The row key of the sender.
    #[field(label = "From", refers_to = "nodes")]
    pub from: &'a str,
    /// The row key of the receiver.
    #[field(label = "To", refers_to = "nodes")]
    pub to: &'a str,
    /// Frames from the sender.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Acks from the receiver.
    #[field(label = "Acks")]
    pub acks: i64,
    /// The level the receiver reported for the frames of the sender, once reported.
    #[field(label = "RSL", unit = "dBm")]
    pub rsl_dbm: Option<f64>,
    /// The level of the sender here.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the Wi-SUN frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WisunFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The channel centre.
    #[field(label = "Channel", unit = "Hz")]
    pub freq: Hertz,
    /// The operating class and channel number in the plan of the bit rate, if the channel is in the plan.
    #[field(label = "Ch")]
    pub ch: Option<&'a str>,
    /// The bit rate.
    #[field(label = "kbit/s")]
    pub kbps: i64,
    /// The UTT or 802.15.4 frame type name.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// "source > destination".
    #[field(label = "Source > destination")]
    pub route: &'a str,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The summary of the frame.
    #[field(label = "Details")]
    pub details: &'a str,
    /// The MPDU without the FCS in hex.
    #[field(label = "MPDU")]
    pub mpdu: &'a str,
}

/// The fields of a new Wi-SUN PAN event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WisunPanEvent<'a> {
    /// The PAN id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The PAN id.
    #[field(label = "PAN")]
    pub pan: &'a str,
}

/// The Wi-SUN networks, most recently heard first.
pub static WISUN_NETWORKS: Table<WisunNetworkRow<'static>> = Table::new("networks", "Networks", 64)
    .label(&WisunNetworkRow::NAME)
    .sorted(
        &WisunNetworkRow::LEVEL_DBFS,
        sdrkit_core::SortOrder::Descending,
    );
/// The Wi-SUN nodes, most recently heard first.
pub static WISUN_NODES: Table<WisunNodeRow<'static>> = Table::new("nodes", "Nodes", 2000);
/// The Wi-SUN links, most recently heard first.
pub static WISUN_LINKS: Table<WisunLinkRow<'static>> =
    Table::new("links", "Links", 4000).label(&WisunLinkRow::FROM);
/// The Wi-SUN frame log, newest first.
pub static WISUN_FRAMES: Table<WisunFrameRow<'static>> = Table::new("frames", "Frames", 800);
/// A new Wi-SUN PAN.
pub static WISUN_PAN: EventKind<WisunPanEvent<'static>> =
    EventKind::new("wisun.pan", "New Wi-SUN PAN");
/// The hop chart: (signal time, channel in MHz) for each frame.
pub const WISUN_HOPS: SeriesDef = SeriesDef {
    key: "hops",
    label: "Hop chart",
    x_unit: "s",
    y_unit: "MHz",
};

/// The row key of a Wi-SUN node.
pub const WISUN_NODE_ROW: Argument<field::Text> = objcmd::row_of("nodes");

/// The arguments of `remove` of the Wi-SUN FAN stack.
#[derive(Arguments)]
pub struct WisunNodeArgs<'a> {
    /// The row key of the node.
    #[arg(shared = WISUN_NODE_ROW)]
    pub row: &'a str,
}

/// Forgets one node.
pub const WISUN_REMOVE: CommandKind<WisunNodeArgs<'static>> = objcmd::remove_kind();

/// The arguments of `send` of the Wi-SUN stack.
#[derive(Arguments)]
pub struct WisunSendArgs<'a> {
    /// The frame type of the UTT IE.
    #[arg(shared = crate::wisun::send::FRAME_TYPE)]
    pub frame_type: Option<crate::wisun::send::WisunFrameType>,
    /// The PAN id.
    #[arg(
        label = "PAN ID",
        help = "4 hex digits; sent as the source PAN id of a frame without a destination."
    )]
    pub pan_id: &'a str,
    /// The source.
    #[arg(label = "Source", help = "The EUI-64 of the sender as 16 hex digits.")]
    pub source: &'a str,
    /// The destination.
    #[arg(
        label = "Destination",
        help = "16 hex digits for an EUI-64, FFFF for broadcast, empty for none."
    )]
    pub destination: &'a str,
    /// The payload IEs.
    #[arg(
        label = "Payload IEs",
        help = "The payload IEs (for example the WP-IE or the MPX-IE) as pairs of hex digits, spaces allowed; empty for \
                none. The whole frame with its 4-octet FCS is at most 2047 octets."
    )]
    pub payload: &'a str,
    /// More header IEs.
    #[arg(
        label = "Header IEs",
        help = "Header IEs after the UTT-IE (for example the BT-IE) as pairs of hex digits, spaces allowed.",
        default = ""
    )]
    pub header_ies: Option<&'a str>,
    /// The sequence number.
    #[arg(
        label = "Sequence",
        help = "0 to 255, sent with a destination; a frame without one leaves it out.",
        default = 0,
        min = 0,
        max = 255,
        step = 1
    )]
    pub sequence: Option<i64>,
    /// The bit rate.
    #[arg(
        label = "Bit rate",
        unit = "kbit/s",
        help = "An FSK mode of the channel; 0 for the slowest mode of the channel.",
        default = 0,
        min = 0,
        max = 150,
        step = 1
    )]
    pub kbps: Option<i64>,
    /// The UFSI of the UTT-IE.
    #[arg(
        label = "UFSI",
        help = "The unicast fractional sequence interval of the UTT-IE, 24 bits: 0 to 16777215.",
        default = 0,
        min = 0,
        max = 16_777_215,
        step = 1
    )]
    pub ufsi: Option<i64>,
}

/// Sends a Wi-SUN FAN frame.
pub const WISUN_SEND: CommandKind<WisunSendArgs<'static>> = CommandKind::new("send", "Send a frame")
    .help(
        "Sends an IEEE 802.15.4 frame of version 2015 with the UTT-IE of the frame type, the header IEs, a header \
         termination IE and the payload IEs, whitened with a 4-octet FCS, on the channel of the plan nearest the tuned \
         frequency. Frames sent during a frame wait for it. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("halt");
/// A Wi-SUN transmission.
pub static WISUN_TX: EventKind<IsmTxEvent<'static>> =
    EventKind::new("wisun.tx", "Wi-SUN transmission");

/// The Wi-SUN FAN stack.
fn wisun() -> StackShape {
    StackShape::new("wisun")
        .fields::<WisunFields>()
        .receiver_fields::<GuardFields>()
        .table(&WISUN_NETWORKS)
        .table(&WISUN_NODES)
        .table(&WISUN_LINKS)
        .table(&WISUN_FRAMES)
        .series(WISUN_HOPS)
        .event(&WISUN_PAN)
        .settings(crate::wisun::WisunSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&WISUN_REMOVE)
        .fields::<IsmTxFields>()
        .event(&WISUN_TX)
        .command(&WISUN_SEND)
        .command(&TX_HALT)
        .kept(&[
            "networks",
            "nodes",
            "links",
            "frames",
            "WisunFields.frames_ok",
            "WisunFields.frames_bad",
            "WisunFields.corrected_frames",
            "WisunFields.fec_frames",
            "WisunFields.mode_switches",
            "WisunFields.encrypted_frames",
            "WisunFields.networks",
            "WisunFields.nodes",
            "WisunFields.links",
        ])
}

// Sigfox.

/// The result fields of the message list of the Sigfox stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SigfoxFields<'a> {
    /// Frames decoded, every repetition.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The messages in the list.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The zone of the dial, if the dial is in one.
    #[field(label = "Zone")]
    pub zone: Option<&'a str>,
}

/// The count of the Sigfox carrier finder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SigfoxFinderFields {
    /// Narrow carriers passed to the demodulator.
    #[field(label = "Sigfox carriers")]
    pub carriers: i64,
}

/// The count of the Sigfox demodulator.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SigfoxDemodFields {
    /// Carriers that did not decode.
    #[field(label = "Carriers not decoded")]
    pub carriers_bad: i64,
}

/// A row of the messages. The row key is the device id, the counter and the signal time of the first frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SigfoxMessageRow<'a> {
    /// The signal time of the first frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The device id.
    #[field(label = "Device ID")]
    pub device_id: &'a str,
    /// The message counter.
    #[field(label = "Seq")]
    pub seq: i64,
    /// The payload as hex, or the one-bit value.
    #[field(label = "Payload")]
    pub payload: &'a str,
    /// The payload length.
    #[field(label = "Payload bytes")]
    pub payload_bytes: i64,
    /// The frequency of the first repetition.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level of the strongest repetition.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Repetitions heard.
    #[field(label = "Reps")]
    pub reps: i64,
    /// The ranks heard as a mask (bit 0 is rank 1).
    #[field(label = "Ranks")]
    pub ranks: i64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Hertz,
    /// The authentication code, as received and never checked.
    #[field(label = "MAC (not checked)")]
    pub auth: &'a str,
    /// The CRC, checked.
    #[field(label = "CRC")]
    pub crc: &'a str,
    /// The frame type of the first repetition.
    #[field(label = "Frame type")]
    pub frame_type: &'a str,
    /// The length indicator.
    #[field(label = "LI")]
    pub li: i64,
    /// The container size.
    #[field(label = "Container", unit = "bytes")]
    pub container_bytes: i64,
    /// The device requests a downlink.
    #[field(label = "Downlink request")]
    pub downlink: bool,
    /// A control message.
    #[field(label = "Control")]
    pub control: bool,
}

/// The fields of a new message event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SigfoxMessageEvent<'a> {
    /// The device id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The log line of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The device id.
    #[field(label = "Device ID")]
    pub device_id: &'a str,
    /// The message counter.
    #[field(label = "Seq")]
    pub seq: i64,
    /// The payload as hex.
    #[field(label = "Payload")]
    pub payload: &'a str,
}

/// The messages, newest first.
pub static SIGFOX_MESSAGES: Table<SigfoxMessageRow<'static>> =
    Table::new("sigfox", "Sigfox", 300).label(&SigfoxMessageRow::DEVICE_ID);
/// A new message.
pub static SIGFOX_MESSAGE: EventKind<SigfoxMessageEvent<'static>> =
    EventKind::new("sigfox.message", "Sigfox message");
/// The waterfall of the carrier finder: one point for each lit cell, x the column plus 1000 times the row.
pub const SIGFOX_WATERFALL: SeriesDef = SeriesDef {
    key: "waterfall",
    label: "Waterfall",
    x_unit: "column",
    y_unit: "level",
};

/// The Sigfox stack.
fn sigfox() -> StackShape {
    StackShape::new("sigfox")
        .fields::<SigfoxFields>()
        .fields::<SigfoxFinderFields>()
        .fields::<SigfoxDemodFields>()
        .receiver_fields::<GuardFields>()
        .component("finder", |c| {
            c.fields::<SigfoxFinderFields>().shares(&["carriers"])
        })
        .component("demod", |c| {
            c.fields::<SigfoxDemodFields>().shares(&["carriers_bad"])
        })
        .table(&SIGFOX_MESSAGES)
        .series(SIGFOX_WATERFALL)
        .event(&SIGFOX_MESSAGE)
        .settings(crate::sigfox::SigfoxSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .kept(&[
            "sigfox",
            "SigfoxFields.frames_ok",
            "SigfoxFields.messages",
            "SigfoxFinderFields",
            "SigfoxDemodFields",
        ])
}

// ANT+.

/// The result fields of the sensor table of the ANT+ stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AntFields {
    /// Frames with a good CRC.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The sensors in the table.
    #[field(label = "Sensors")]
    pub sensors: i64,
    /// The newest heart rate of any strap, if a strap sent one.
    #[field(label = "Heart rate", unit = "bpm")]
    pub heart_rate_bpm: Option<f64>,
    /// The deviation of the last frame. It has no value before the first frame.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation: Option<Hertz>,
    /// The channel the receiver is tuned to.
    #[field(label = "Channel", unit = "Hz")]
    pub channel: Hertz,
}

/// The counts of the ANT+ demodulator.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AntDemodFields {
    /// Frames whose CRC failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Sync words found.
    #[field(label = "Sync hits")]
    pub syncs: i64,
}

/// A row of the sensors. The row key is the device number, type and transmission type as one number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AntSensorRow<'a> {
    /// The device number.
    #[field(label = "Device")]
    pub device: i64,
    /// The device type name.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The network marker as hex.
    #[field(label = "Network")]
    pub marker: &'a str,
    /// The manufacturer, once a page names it.
    #[field(label = "Manufacturer")]
    pub manufacturer: Option<&'a str>,
    /// The serial number, once a page gives it.
    #[field(label = "Serial")]
    pub serial: Option<i64>,
    /// The main value of the sensor in words.
    #[field(label = "Value")]
    pub value: &'a str,
    /// The battery status, once a page gives it.
    #[field(label = "Battery")]
    pub battery: Option<&'a str>,
    /// The measured message period.
    #[field(label = "Message period", unit = "s")]
    pub period: Option<Seconds>,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AntFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The device number.
    #[field(label = "Device")]
    pub device: i64,
    /// The device type name.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The transmission type.
    #[field(label = "Transmission type")]
    pub tx_type: i64,
    /// The network marker as hex.
    #[field(label = "Network")]
    pub marker: &'a str,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The payload as hex.
    #[field(label = "Payload")]
    pub payload: &'a str,
    /// The page and its decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The fields of a new sensor event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AntSensorEvent<'a> {
    /// The device number.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The device number.
    #[field(label = "Device")]
    pub device: i64,
    /// The device type name.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The network marker as hex.
    #[field(label = "Network")]
    pub marker: &'a str,
}

/// The sensors, most recently heard first.
pub static ANT_SENSORS: Table<AntSensorRow<'static>> = Table::new("sensors", "Sensors", 100);
/// The frame log, newest first.
pub static ANT_FRAMES: Table<AntFrameRow<'static>> = Table::new("frames", "Frames", 400);
/// A new sensor.
pub static ANT_SENSOR: EventKind<AntSensorEvent<'static>> =
    EventKind::new("ant.sensor", "New ANT sensor");
/// The main value of the newest sensor over time.
pub const ANT_HISTORY: SeriesDef = SeriesDef {
    key: "history",
    label: "Main value of the newest sensor",
    x_unit: "s",
    y_unit: "",
};

/// The row key of a sensor.
pub const ANT_SENSOR_ROW: Argument<field::Text> = objcmd::row_of("sensors");

/// The arguments of `remove` of the ANT+ stack.
#[derive(Arguments)]
pub struct AntSensorArgs<'a> {
    /// The row key of the sensor.
    #[arg(shared = ANT_SENSOR_ROW)]
    pub row: &'a str,
}

/// Forgets one sensor.
pub const ANT_REMOVE: CommandKind<AntSensorArgs<'static>> = objcmd::remove_kind();

/// The arguments of `send` of the ANT+ stack.
#[derive(Arguments)]
pub struct AntSendArgs<'a> {
    /// The device number.
    #[arg(label = "Device number", help = "0 to 65535.")]
    pub device: i64,
    /// The device type.
    #[arg(
        label = "Device type",
        help = "1 to 127, for example 120 for heart rate or 11 for bike power."
    )]
    pub device_type: i64,
    /// The transmission type.
    #[arg(label = "Transmission type", help = "0 to 255.")]
    pub tx_type: i64,
    /// The data page.
    #[arg(
        label = "Payload",
        help = "The 8 octets of the data page as 16 hex digits, spaces allowed."
    )]
    pub payload: &'a str,
    /// The pairing bit.
    #[arg(
        label = "Pairing",
        help = "Sets the pairing bit of the device type octet.",
        default = false
    )]
    pub pairing: Option<bool>,
}

/// Sends an ANT+ frame.
pub const ANT_SEND: CommandKind<AntSendArgs<'static>> = CommandKind::new("send", "Send a frame")
    .help(
        "Sends one ANT+ broadcast frame (the ANT+ network marker, the channel id, the data page and the CRC) on the tuned \
         frequency. Frames sent during a frame wait for it. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("halt");
/// An ANT+ transmission.
pub static ANT_TX: EventKind<IsmTxEvent<'static>> = EventKind::new("ant.tx", "ANT+ transmission");

/// The ANT+ stack.
fn ant() -> StackShape {
    StackShape::new("ant")
        .fields::<AntFields>()
        .fields::<AntDemodFields>()
        .component("demod", |c| {
            c.fields::<AntDemodFields>()
                .shares(&["frames_bad", "syncs"])
                .dropped()
        })
        .table(&ANT_SENSORS)
        .table(&ANT_FRAMES)
        .series(ANT_HISTORY)
        .event(&ANT_SENSOR)
        .settings(crate::ant::AntSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&ANT_REMOVE)
        .fields::<IsmTxFields>()
        .event(&ANT_TX)
        .command(&ANT_SEND)
        .command(&TX_HALT)
        .kept(&[
            "sensors",
            "frames",
            "AntFields.frames_ok",
            "AntFields.sensors",
            "AntFields.heart_rate_bpm",
            "AntDemodFields",
        ])
}

// NFC.

/// The result fields of the tag table of the NFC stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NfcFields {
    /// Frames heard.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Tags identified.
    #[field(label = "Tags")]
    pub tags: i64,
    /// CRC or parity failures outside encrypted sessions.
    #[field(label = "Failures")]
    pub failures: i64,
    /// Frames from tags.
    #[field(label = "Tag frames")]
    pub tag_frames: i64,
}

/// The levels of the NFC detector. They have no value before the first input.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NfcDetectFields {
    /// The field level of the input.
    #[field(label = "Field level", unit = "dBFS")]
    pub field_dbfs: Option<f64>,
    /// The level of the stronger subcarrier detector.
    #[field(label = "Subcarrier level", unit = "dBFS")]
    pub subcarrier_dbfs: Option<f64>,
}

/// A row of the tags. The row key is the UID as hex.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NfcTagRow<'a> {
    /// The UID as hex, most significant first.
    #[field(label = "UID")]
    pub uid: &'a str,
    /// The type of the tag.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The card-type hint.
    #[field(label = "Hint")]
    pub hint: Option<&'a str>,
    /// The ATQA.
    #[field(label = "ATQA")]
    pub atqa: Option<&'a str>,
    /// The SAK.
    #[field(label = "SAK")]
    pub sak: Option<&'a str>,
    /// The ATS historical bytes.
    #[field(label = "Historical bytes")]
    pub historical: Option<&'a str>,
    /// The PUPI application data.
    #[field(label = "Application data")]
    pub app_data: Option<&'a str>,
    /// The DSFID, AFI, block count and size and IC version byte of an ISO 15693 tag.
    #[field(label = "System information")]
    pub system: Option<&'a str>,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NfcFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// "reader" or "tag".
    #[field(label = "Direction")]
    pub direction: &'a str,
    /// The link.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The bytes in hex, or nothing for content not shown.
    #[field(label = "Bytes")]
    pub hex: &'a str,
    /// The name of the frame.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The CRC result.
    #[field(label = "CRC")]
    pub crc: &'a str,
    /// The parity result.
    #[field(label = "Parity good")]
    pub parity_ok: bool,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The fields of a tag event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NfcTagEvent<'a> {
    /// The UID as hex.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The UID as hex.
    #[field(label = "UID")]
    pub uid: &'a str,
}

/// The tags, most recently heard first.
pub static NFC_TAGS: Table<NfcTagRow<'static>> =
    Table::new("tags", "Tags", 100).label(&NfcTagRow::UID);
/// The frame log, newest first.
pub static NFC_FRAMES: Table<NfcFrameRow<'static>> =
    Table::new("frames", "Frames", 200).label(&NfcFrameRow::NAME);
/// A tag identified.
pub static NFC_TAG: EventKind<NfcTagEvent<'static>> = EventKind::new("nfc.tag", "NFC tag");

/// The row key of a tag.
pub const NFC_TAG_ROW: Argument<field::Text> = objcmd::row_of("tags");

/// The arguments of `remove` of the NFC stack.
#[derive(Arguments)]
pub struct NfcTagArgs<'a> {
    /// The row key of the tag.
    #[arg(shared = NFC_TAG_ROW)]
    pub row: &'a str,
}

/// Forgets one tag.
pub const NFC_REMOVE: CommandKind<NfcTagArgs<'static>> = objcmd::remove_kind();

/// The NFC stack.
fn nfc() -> StackShape {
    StackShape::new("nfc")
        .fields::<NfcFields>()
        .fields::<NfcDetectFields>()
        .receiver_fields::<GuardFields>()
        .component("detect", |c| {
            c.fields::<NfcDetectFields>()
                .shares(&["field_dbfs", "subcarrier_dbfs"])
        })
        .table(&NFC_TAGS)
        .table(&NFC_FRAMES)
        .event(&NFC_TAG)
        .command(&objcmd::CLEAR)
        .command(&NFC_REMOVE)
        .kept(&["tags", "frames", "NfcFields"])
}

// nRF24L01.

/// The result fields of the tables of the nRF24L01 stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrfFields {
    /// Frames decoded.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Bursts without a frame.
    #[field(label = "Bursts without frame")]
    pub bad_bursts: i64,
    /// Bursts.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Frames with a CRC-16.
    #[field(label = "CRC-16 frames")]
    pub crc16_frames: i64,
    /// Frames with a CRC-8.
    #[field(label = "CRC-8 frames")]
    pub crc8_frames: i64,
    /// The addresses in the table.
    #[field(label = "Addresses")]
    pub addresses: i64,
    /// The channel number N (2400 + N MHz).
    #[field(label = "Channel")]
    pub channel: i64,
}

/// A row of the frame log. The row key is the number of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrfFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Hertz,
    /// The address as hex.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The address length in bytes.
    #[field(label = "Address length")]
    pub address_len: i64,
    /// The payload length in bytes.
    #[field(label = "Length")]
    pub length: i64,
    /// The PID.
    #[field(label = "PID")]
    pub pid: i64,
    /// The NO_ACK bit.
    #[field(label = "NO_ACK")]
    pub no_ack: bool,
    /// The CRC bits.
    #[field(label = "CRC bits")]
    pub crc_bits: i64,
    /// The payload as hex.
    #[field(label = "Payload")]
    pub payload: &'a str,
    /// The printable text of the payload.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The same frame again in a row within 2 s.
    #[field(label = "Repeats")]
    pub repeats: i64,
}

/// A row of the addresses. The row key is the address as hex.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrfAddressRow<'a> {
    /// The address as hex.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The address length in bytes.
    #[field(label = "Address length")]
    pub address_len: i64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Hertz,
    /// The CRC bits.
    #[field(label = "CRC bits")]
    pub crc_bits: i64,
    /// The last payload length in bytes.
    #[field(label = "Length")]
    pub length: i64,
    /// The carrier frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the bursts without a frame. The row key is the number of the burst in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrfBadBurstRow {
    /// The signal time of the burst.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The duration of the burst.
    #[field(label = "Duration", unit = "µs")]
    pub duration_us: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// The fields of a new address event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrfAddressEvent<'a> {
    /// The address as hex.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Hertz,
    /// The CRC bits.
    #[field(label = "CRC bits")]
    pub crc_bits: i64,
}

/// The frame log, newest first.
pub static NRF_FRAMES: Table<NrfFrameRow<'static>> =
    Table::new("frames", "Frames", 300).label(&NrfFrameRow::ADDRESS);
/// The addresses, most recently heard first.
pub static NRF_ADDRESSES: Table<NrfAddressRow<'static>> =
    Table::new("addresses", "Addresses", 100).label(&NrfAddressRow::ADDRESS);
/// The bursts without a frame, newest first.
pub static NRF_BAD_BURSTS: Table<NrfBadBurstRow> =
    Table::new("bad_bursts", "Bursts without frame", 60);
/// A new address.
pub static NRF_ADDRESS: EventKind<NrfAddressEvent<'static>> =
    EventKind::new("nrf24.address", "New nRF24 address");

/// The row key of an address.
pub const NRF_ADDRESS_ROW: Argument<field::Text> = objcmd::row_of("addresses");

/// The arguments of `remove` of the nRF24L01 stack.
#[derive(Arguments)]
pub struct NrfAddressArgs<'a> {
    /// The row key of the address.
    #[arg(shared = NRF_ADDRESS_ROW)]
    pub row: &'a str,
}

/// Forgets one address.
pub const NRF_REMOVE: CommandKind<NrfAddressArgs<'static>> = objcmd::remove_kind();

/// The nRF24L01 stack.
fn nrf24() -> StackShape {
    StackShape::new("nrf24")
        .fields::<NrfFields>()
        .decoder("demod")
        .table(&NRF_FRAMES)
        .table(&NRF_ADDRESSES)
        .table(&NRF_BAD_BURSTS)
        .event(&NRF_ADDRESS)
        .settings(crate::nrf24::NrfSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&NRF_REMOVE)
        .kept(&[
            "frames",
            "addresses",
            "bad_bursts",
            "NrfFields.frames",
            "NrfFields.bad_bursts",
            "NrfFields.bursts",
            "NrfFields.crc16_frames",
            "NrfFields.crc8_frames",
            "NrfFields.addresses",
        ])
}

// Sub-GHz chat.

/// The result fields of the Sub-GHz chat stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SubghzchatFields {
    /// Lines heard.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Packets with a good CRC.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Packets whose CRC failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Bursts seen.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Lines decrypted.
    #[field(label = "Decrypted")]
    pub decrypted: i64,
    /// Encrypted lines that the key does not open.
    #[field(label = "Not readable")]
    pub unreadable: i64,
    /// Encrypted repeats dropped.
    #[field(label = "Repeats dropped")]
    pub repeats: i64,
    /// Lines the user sent.
    #[field(label = "Sent")]
    pub sent: i64,
    /// A key is set.
    #[field(label = "Key set")]
    pub key_set: bool,
    /// The SNR of the packets. It has no value before the first good packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the last good packet. It has no value before the first good packet.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
}

/// A row of the chat. The row key is the number of the line in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SubghzchatLineRow<'a> {
    /// The signal time of the line.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// "heard" or "sent".
    #[field(label = "Direction")]
    pub direction: &'a str,
    /// How the line was read.
    #[field(label = "Kind")]
    pub kind: LineKind,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The packets joined into the line.
    #[field(label = "Packets")]
    pub packets: i64,
    /// The level of the first packet of a line heard.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The SNR of the first packet of a line heard.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the first packet of a line heard.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The run id and counter of an encrypted line.
    #[field(label = "Run and counter")]
    pub run: Option<&'a str>,
}

/// A row of the packet log. The row key is the number of the packet in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SubghzchatPacketRow<'a> {
    /// The signal time of the packet.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The payload length.
    #[field(label = "Length", unit = "octets")]
    pub length: i64,
    /// The CRC is right.
    #[field(label = "CRC good")]
    pub crc_ok: bool,
    /// The packet was found inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// The level of the packet.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The SNR of the packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The carrier offset of the packet.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The payload in hex.
    #[field(label = "Octets")]
    pub hex: &'a str,
}

/// The fields of a chat line event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SubghzchatLineEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How the line was read.
    #[field(label = "Kind")]
    pub kind: LineKind,
    /// The SNR of the first packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// The fields of the event that packets come in after a quiet time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SubghzchatStateEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The chat, newest first.
pub static SUBGHZCHAT_CHAT: Table<SubghzchatLineRow<'static>> = Table::new("chat", "Chat", 500);
/// The packet log, newest first.
pub static SUBGHZCHAT_PACKETS: Table<SubghzchatPacketRow<'static>> =
    Table::new("packets", "Packets", 200).label(&SubghzchatPacketRow::HEX);
/// A chat line heard.
pub static SUBGHZCHAT_LINE: EventKind<SubghzchatLineEvent<'static>> =
    EventKind::new("subghzchat.line", "Chat line");
/// Packets coming in after a quiet time.
pub static SUBGHZCHAT_STATE: EventKind<SubghzchatStateEvent<'static>> =
    EventKind::new("subghzchat.state", "Chat packets coming in");

/// The Sub-GHz chat stack.
fn subghzchat() -> StackShape {
    StackShape::new("subghzchat")
        .fields::<SubghzchatFields>()
        .receiver_fields::<GuardFields>()
        .table(&SUBGHZCHAT_CHAT)
        .table(&SUBGHZCHAT_PACKETS)
        .event(&SUBGHZCHAT_LINE)
        .event(&SUBGHZCHAT_STATE)
        .settings(crate::subghzchat::ChatSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .kept(&[
            "chat",
            "packets",
            "SubghzchatFields.messages",
            "SubghzchatFields.frames_ok",
            "SubghzchatFields.frames_bad",
            "SubghzchatFields.bursts",
            "SubghzchatFields.decrypted",
            "SubghzchatFields.unreadable",
            "SubghzchatFields.repeats",
            "SubghzchatFields.sent",
            "SubghzchatFields.key_set",
        ])
}

// DECT.

/// The result fields of the base station table of the DECT stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DectFields<'a> {
    /// The base stations in the table.
    #[field(label = "Base stations")]
    pub bases: i64,
    /// Bursts from base stations.
    #[field(label = "Base bursts")]
    pub base_bursts: i64,
    /// Bursts from handsets.
    #[field(label = "Handset bursts")]
    pub handset_bursts: i64,
    /// The carrier the receiver is tuned to.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier: Hertz,
    /// The band of the carrier, if the carrier is one of a band.
    #[field(label = "Band")]
    pub band: Option<&'a str>,
    /// The number of the carrier in its band, if the carrier is one of a band.
    #[field(label = "Carrier number")]
    pub carrier_number: Option<i64>,
}

/// The counts of the DECT demodulator.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DectDemodFields {
    /// A-fields with a good R-CRC.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bursts whose S-field was found and whose A-field failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
}

/// A row of the base stations. The row key is the RFPI.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DectBaseRow<'a> {
    /// The RFPI as 10 hex digits.
    #[field(label = "RFPI")]
    pub rfpi: &'a str,
    /// The access rights class.
    #[field(label = "Class")]
    pub arc: &'a str,
    /// The equipment manufacturer code (class A).
    #[field(label = "EMC")]
    pub emc: Option<i64>,
    /// The fixed part number (class A).
    #[field(label = "FPN")]
    pub fpn: Option<i64>,
    /// The radio fixed part number (class A).
    #[field(label = "RPN")]
    pub rpn: Option<i64>,
    /// The slots in use, as 24 digits.
    #[field(label = "Slots")]
    pub slots: &'a str,
    /// The level of the last beacon.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The last Qt tail as hex.
    #[field(label = "Last Qt")]
    pub last_qt: Option<&'a str>,
    /// The decoded system information of the last Qt tail.
    #[field(label = "System information")]
    pub system_info: Option<&'a str>,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last beacon.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the burst log. The row key is the number of the burst in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DectBurstRow<'a> {
    /// The signal time of the burst.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// "base" or "handset".
    #[field(label = "From")]
    pub from: &'a str,
    /// The name of the tail type.
    #[field(label = "Tail type")]
    pub tail_type: &'a str,
    /// The tail as hex.
    #[field(label = "Tail")]
    pub tail: &'a str,
    /// The level of the burst.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The fields of a new base station event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DectBaseEvent<'a> {
    /// The RFPI as 10 hex digits.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The RFPI as 10 hex digits.
    #[field(label = "RFPI")]
    pub rfpi: &'a str,
    /// The equipment manufacturer code (class A).
    #[field(label = "EMC")]
    pub emc: Option<i64>,
}

/// The base stations, most recently heard first.
pub static DECT_BASES: Table<DectBaseRow<'static>> = Table::new("bases", "Base stations", 60)
    .sorted(&DectBaseRow::LEVEL_DBFS, sdrkit_core::SortOrder::Descending);
/// The burst log, newest first.
pub static DECT_BURSTS: Table<DectBurstRow<'static>> =
    Table::new("bursts", "Bursts", 300).label(&DectBurstRow::FROM);
/// A new base station.
pub static DECT_BASE: EventKind<DectBaseEvent<'static>> =
    EventKind::new("dect.base", "New DECT base station");

/// The row key of a base station.
pub const DECT_BASE_ROW: Argument<field::Text> = objcmd::row_of("bases");

/// The arguments of `remove` of the DECT stack.
#[derive(Arguments)]
pub struct DectBaseArgs<'a> {
    /// The row key of the base station.
    #[arg(shared = DECT_BASE_ROW)]
    pub row: &'a str,
}

/// Forgets one base station.
pub const DECT_REMOVE: CommandKind<DectBaseArgs<'static>> = objcmd::remove_kind();

/// The DECT stack.
fn dect() -> StackShape {
    StackShape::new("dect")
        .fields::<DectFields>()
        .fields::<DectDemodFields>()
        .component("demod", |c| {
            c.fields::<DectDemodFields>()
                .shares(&["frames_ok", "frames_bad"])
                .dropped()
        })
        .table(&DECT_BASES)
        .table(&DECT_BURSTS)
        .event(&DECT_BASE)
        .settings(crate::dect::DectSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(&DECT_REMOVE)
        .kept(&[
            "bases",
            "bursts",
            "DectFields.bases",
            "DectFields.base_bursts",
            "DectFields.handset_bursts",
            "DectDemodFields",
        ])
}

/// The shape of the stack `id`.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the ism family does not describe the stack {id}"))
}

/// An empty store of the results of the stack `id`, for its decoder.
pub fn results(id: &str) -> sdrkit_core::publish::Published {
    sdrkit_core::publish::Published::from_decl(std::sync::Arc::new(shape(id).results_decl()))
}
