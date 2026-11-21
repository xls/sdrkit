//! The SDRconnect WebSocket API as the driver uses it: the JSON envelope, the event and property names, and the binary
//! messages.
//!
//! Standard: SDRconnect WebSocket API 1.0.3 (SDRplay).

/// The port SDRconnect's WebSocket server listens on when the address names none.
pub const DEFAULT_PORT: u16 = 5454;

/// The event types.
pub mod event {
    /// A property changed (from SDRconnect).
    pub const PROPERTY_CHANGED: &str = "property_changed";
    /// The answer to a `get_property` (from SDRconnect).
    pub const GET_PROPERTY_RESPONSE: &str = "get_property_response";
    /// Set a property.
    pub const SET_PROPERTY: &str = "set_property";
    /// Ask for a property.
    pub const GET_PROPERTY: &str = "get_property";
    /// Switch the IQ stream on or off.
    pub const IQ_STREAM_ENABLE: &str = "iq_stream_enable";
    /// Start or stop the selected radio.
    pub const DEVICE_STREAM_ENABLE: &str = "device_stream_enable";
    /// Let the primary tuner's events and binary data through, or hold them back.
    pub const SET_PRIMARY_DEVICE_ENABLE: &str = "set_primary_device_enable";
    /// Let the secondary tuner's events and binary data through, or hold them back.
    pub const SET_SECONDARY_DEVICE_ENABLE: &str = "set_secondary_device_enable";
}

/// The property names the driver uses.
pub mod prop {
    /// The radio's centre frequency, whole Hz.
    pub const CENTRE: &str = "device_center_frequency";
    /// The radio's sample rate, Hz.
    pub const RATE: &str = "device_sample_rate";
    /// The LNA state.
    pub const LNA: &str = "lna_state";
    /// The lowest LNA state.
    pub const LNA_MIN: &str = "lna_state_min";
    /// The highest LNA state.
    pub const LNA_MAX: &str = "lna_state_max";
    /// The antenna names, comma-separated.
    pub const ANTENNAS: &str = "valid_antennas";
    /// The antenna in use.
    pub const ANTENNA: &str = "active_antenna";
    /// The radio streams.
    pub const STARTED: &str = "started";
    /// This client can control the hardware.
    pub const CAN_CONTROL: &str = "can_control";
    /// The radio is overloaded.
    pub const OVERLOAD: &str = "overload";
    /// The measured signal power, dB.
    pub const POWER: &str = "signal_power";
    /// The measured signal SNR, dB.
    pub const SNR: &str = "signal_snr";
    /// The display name of the radio in use.
    pub const DEVICE: &str = "active_device";
    /// The WebSocket API version.
    pub const API: &str = "api_version";
}

sdrkit_core::choice_enum! {
    /// A tuner: the primary or the secondary device of a dual-tuner radio (an RSPduo in dual-tuner mode).
    pub enum Tuner {
        /// The first tuner, and the only one of a single-tuner radio.
        Primary = ("primary", "Primary (tuner 1)"),
        /// The second tuner of an RSPduo in dual-tuner mode (API 1.0.2).
        Secondary = ("secondary", "Secondary (tuner 2)"),
    }
}

impl Tuner {
    /// The `device` field's value.
    pub fn name(self) -> &'static str {
        match self {
            Tuner::Primary => "primary",
            Tuner::Secondary => "secondary",
        }
    }

    /// The binary type of this tuner's IQ.
    pub fn iq_type(self) -> u16 {
        match self {
            Tuner::Primary => 2,
            Tuner::Secondary => 5,
        }
    }

    /// The enable event of this tuner.
    pub fn enable_event(self) -> &'static str {
        match self {
            Tuner::Primary => event::SET_PRIMARY_DEVICE_ENABLE,
            Tuner::Secondary => event::SET_SECONDARY_DEVICE_ENABLE,
        }
    }

    /// The other tuner.
    pub fn other(self) -> Tuner {
        match self {
            Tuner::Primary => Tuner::Secondary,
            Tuner::Secondary => Tuner::Primary,
        }
    }
}

/// One text message: the envelope of 3.1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    /// The event type.
    pub event_type: String,
    /// The property name (empty for events that have none).
    pub property: String,
    /// The value, as text.
    pub value: String,
    /// The tuner. A missing or empty `device` is the primary tuner.
    pub device: Tuner,
}

/// Why a text message is malformed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Not JSON.
    NotJson,
    /// JSON, but not an object.
    NotObject,
    /// No `event_type` string.
    NoEventType,
    /// No `property` string.
    NoProperty,
    /// A `device` that is neither primary nor secondary.
    BadDevice(String),
}

/// Reads one text message. `device` is read without regard to case. A number or a yes/no in `value` is taken as its text, and
/// a missing `value` as empty.
///
/// ```
/// use sdrkit_driver_sdrconnect::proto::{parse, Tuner};
/// // The push message example of the API, page 2.
/// let e = parse(r#"{"event_type": "property_changed", "property": "device_center_frequency", "device": "primary",
///                   "value": "100000000"}"#).unwrap();
/// assert_eq!((e.property.as_str(), e.value.as_str(), e.device), ("device_center_frequency", "100000000", Tuner::Primary));
/// ```
pub fn parse(text: &str) -> Result<Envelope, Malformed> {
    let raw: Raw = match serde_json::from_str(text) {
        Ok(r) if text.trim_start().starts_with('{') => r,
        Ok(_) => return Err(Malformed::NotObject),
        Err(_) => {
            return Err(match serde_json::from_str::<serde::de::IgnoredAny>(text) {
                Ok(_) => Malformed::NotObject,
                Err(_) => Malformed::NotJson,
            })
        }
    };
    let text_of = |v: &Option<Scalar>| match v {
        Some(Scalar::Text(s)) => Some(s.clone()),
        _ => None,
    };
    let event_type = text_of(&raw.event_type).ok_or(Malformed::NoEventType)?;
    let property = text_of(&raw.property).ok_or(Malformed::NoProperty)?;
    let value = match raw.value {
        Some(Scalar::Text(s)) => s,
        Some(Scalar::Number(n)) => n.to_string(),
        Some(Scalar::Bool(b)) => b.to_string(),
        _ => String::new(),
    };
    let device = match text_of(&raw.device).as_deref().map(str::trim) {
        None | Some("") => Tuner::Primary,
        Some(d) if d.eq_ignore_ascii_case("primary") => Tuner::Primary,
        Some(d) if d.eq_ignore_ascii_case("secondary") => Tuner::Secondary,
        Some(d) => return Err(Malformed::BadDevice(d.to_string())),
    };
    Ok(Envelope {
        event_type,
        property,
        value,
        device,
    })
}

/// The fields of a text message that the driver reads.
#[derive(serde::Deserialize)]
struct Raw {
    event_type: Option<Scalar>,
    property: Option<Scalar>,
    value: Option<Scalar>,
    device: Option<Scalar>,
}

/// One value of a text message.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Scalar {
    Text(String),
    Number(serde_json::Number),
    Bool(bool),
    Other(serde::de::IgnoredAny),
}

/// One client message as JSON text: all four fields, `device` only when `device` is given.
///
/// ```
/// use sdrkit_driver_sdrconnect::proto::{request, Tuner};
/// // The set_property example of the API, page 3.
/// assert_eq!(
///     request("set_property", "device_center_frequency", "101000000", Some(Tuner::Primary)),
///     r#"{"event_type":"set_property","property":"device_center_frequency","value":"101000000","device":"primary"}"#
/// );
/// ```
pub fn request(event_type: &str, property: &str, value: &str, device: Option<Tuner>) -> String {
    // Written field by field, so the order is the API's whatever features serde_json is built with.
    let q = |t: &str| serde_json::to_string(t).unwrap_or_default();
    let mut s = format!(
        "{{\"event_type\":{},\"property\":{},\"value\":{}",
        q(event_type),
        q(property),
        q(value)
    );
    if let Some(d) = device {
        s.push_str(&format!(",\"device\":{}", q(d.name())));
    }
    s.push('}');
    s
}

/// A yes/no value: `true` or `false`, and also `1` or `0`.
pub fn parse_bool(s: &str) -> Option<bool> {
    match s.trim() {
        t if t.eq_ignore_ascii_case("true") || t == "1" => Some(true),
        t if t.eq_ignore_ascii_case("false") || t == "0" => Some(false),
        _ => None,
    }
}

/// A finite number.
pub fn parse_num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A whole number from 0 to `u32::MAX`.
pub fn parse_u32(s: &str) -> Option<u32> {
    s.trim().parse::<u32>().ok()
}

/// The antenna names of `valid_antennas`: split at commas, trimmed, empty names dropped.
pub fn split_antennas(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(String::from)
        .collect()
}

/// A version text such as `1.0.3` as (major, minor, patch). Missing parts are 0.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.trim().split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().map_or(Some(0), |p| p.parse().ok())?;
    let patch = it.next().map_or(Some(0), |p| p.parse().ok())?;
    Some((major, minor, patch))
}

/// The API version that the dual-tuner (RSPduo) messages need.
pub const DUAL_TUNER_API: (u32, u32, u32) = (1, 0, 2);

/// What a binary message is.
#[derive(Debug, PartialEq, Eq)]
pub enum Binary<'a> {
    /// IQ of `tuner`: the payload after the type.
    Iq {
        /// The tuner.
        tuner: Tuner,
        /// The interleaved 16-bit values.
        payload: &'a [u8],
    },
    /// Audio (type 1 or 4).
    Audio(Tuner),
    /// Spectrum bins (type 3 or 6).
    Spectrum(Tuner),
    /// A type the API does not list.
    Unknown(u16),
    /// Shorter than the 2-byte type.
    Short,
}

/// Reads the type of a binary message: a little-endian unsigned 16-bit number in its first 2 bytes.
///
/// ```
/// use sdrkit_driver_sdrconnect::proto::{binary, Binary, Tuner};
/// assert_eq!(binary(&[0x02, 0x00, 1, 2, 3, 4]), Binary::Iq { tuner: Tuner::Primary, payload: &[1, 2, 3, 4] });
/// assert_eq!(binary(&[0x05, 0x00]), Binary::Iq { tuner: Tuner::Secondary, payload: &[] });
/// assert_eq!(binary(&[0x03]), Binary::Short);
/// ```
pub fn binary(msg: &[u8]) -> Binary<'_> {
    if msg.len() < 2 {
        return Binary::Short;
    }
    let payload = &msg[2..];
    match u16::from_le_bytes([msg[0], msg[1]]) {
        1 => Binary::Audio(Tuner::Primary),
        2 => Binary::Iq {
            tuner: Tuner::Primary,
            payload,
        },
        3 => Binary::Spectrum(Tuner::Primary),
        4 => Binary::Audio(Tuner::Secondary),
        5 => Binary::Iq {
            tuner: Tuner::Secondary,
            payload,
        },
        6 => Binary::Spectrum(Tuner::Secondary),
        t => Binary::Unknown(t),
    }
}

/// Bytes per complex sample on the link: two signed 16-bit values.
pub const SAMPLE_BYTES: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    /// The JSON examples of the API, pages 2 to 4, with straight quotes.
    const PUSH: &str = r#"{
"event_type": "property_changed",
"property": "device_center_frequency",
"device": "primary",
"value": "100000000"
}"#;
    const RESPONSE: &str = r#"{
"event_type": "get_property_response",
"property": "device_vfo_frequency",
"device": "primary",
"value": "101000000"
}"#;
    const SET: &str = r#"{
"event_type": "set_property",
"property": "device_center_frequency",
"device": "primary",
"value": "101000000"
}"#;
    const AUDIO: &str = r#"{
"event_type": "audio_stream_enable",
"property": "",
"value": "true"
}"#;
    const SELECT: &str = r#"{
"event_type": "selected_device",
"property": "",
"value": "1"
}"#;

    fn env(e: &str, p: &str, v: &str, d: Tuner) -> Envelope {
        Envelope {
            event_type: e.into(),
            property: p.into(),
            value: v.into(),
            device: d,
        }
    }

    #[test]
    fn the_api_examples_parse_to_their_fields() {
        assert_eq!(
            parse(PUSH),
            Ok(env(
                "property_changed",
                "device_center_frequency",
                "100000000",
                Tuner::Primary
            ))
        );
        assert_eq!(
            parse(RESPONSE),
            Ok(env(
                "get_property_response",
                "device_vfo_frequency",
                "101000000",
                Tuner::Primary
            ))
        );
        assert_eq!(
            parse(SET),
            Ok(env(
                "set_property",
                "device_center_frequency",
                "101000000",
                Tuner::Primary
            ))
        );
        // No device field: the API takes the primary tuner (page 1).
        assert_eq!(
            parse(AUDIO),
            Ok(env("audio_stream_enable", "", "true", Tuner::Primary))
        );
        assert_eq!(
            parse(SELECT),
            Ok(env("selected_device", "", "1", Tuner::Primary))
        );
    }

    #[test]
    fn requests_round_trip_to_the_api_examples() {
        for (text, device) in [(SET, Some(Tuner::Primary)), (AUDIO, None), (SELECT, None)] {
            let e = parse(text).unwrap();
            let r = request(&e.event_type, &e.property, &e.value, device);
            assert_eq!(parse(&r).unwrap(), e, "{r}");
        }
        assert_eq!(
            request("iq_stream_enable", "", "true", None),
            r#"{"event_type":"iq_stream_enable","property":"","value":"true"}"#
        );
        // Text that needs escaping stays one JSON string.
        let r = request(
            "set_property",
            "active_antenna",
            "Antenna \"A\"",
            Some(Tuner::Secondary),
        );
        assert_eq!(parse(&r).unwrap().value, "Antenna \"A\"");
        assert_eq!(parse(&r).unwrap().device, Tuner::Secondary);
    }

    #[test]
    fn the_device_and_value_are_read_leniently() {
        let p = |t: &str| parse(t);
        assert_eq!(
            p(r#"{"event_type":"x","property":"y","value":"1","device":"SECONDARY"}"#)
                .unwrap()
                .device,
            Tuner::Secondary
        );
        assert_eq!(
            p(r#"{"event_type":"x","property":"y","value":"1","device":""}"#)
                .unwrap()
                .device,
            Tuner::Primary
        );
        assert_eq!(
            p(r#"{"event_type":"x","property":"y","value":2.5}"#)
                .unwrap()
                .value,
            "2.5"
        );
        assert_eq!(
            p(r#"{"event_type":"x","property":"y","value":true}"#)
                .unwrap()
                .value,
            "true"
        );
        assert_eq!(p(r#"{"event_type":"x","property":"y"}"#).unwrap().value, "");
    }

    #[test]
    fn malformed_text_is_told_apart() {
        assert_eq!(parse("not json"), Err(Malformed::NotJson));
        assert_eq!(parse("[1,2]"), Err(Malformed::NotObject));
        assert_eq!(
            parse(r#"{"property":"y","value":"1"}"#),
            Err(Malformed::NoEventType)
        );
        assert_eq!(
            parse(r#"{"event_type":"x","value":"1"}"#),
            Err(Malformed::NoProperty)
        );
        assert_eq!(
            parse(r#"{"event_type":"x","property":"y","value":"1","device":"third"}"#),
            Err(Malformed::BadDevice("third".into()))
        );
        // The PDF's typographic quotes are not JSON.
        assert_eq!(
            parse("{“event_type”: “property_changed”}"),
            Err(Malformed::NotJson)
        );
    }

    #[test]
    fn values_parse_as_the_property_table_says() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("False"), Some(false));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("yes"), None);
        assert_eq!(parse_num("2000000.0"), Some(2e6));
        assert_eq!(parse_num("nan"), None);
        assert_eq!(parse_num("x"), None);
        assert_eq!(parse_u32("9"), Some(9));
        assert_eq!(parse_u32("-1"), None);
        assert_eq!(
            split_antennas("Antenna A, Antenna B,,Hi-Z "),
            ["Antenna A", "Antenna B", "Hi-Z"]
        );
        assert!(split_antennas("").is_empty());
        assert_eq!(parse_version("1.0.3"), Some((1, 0, 3)));
        assert_eq!(parse_version("1.1"), Some((1, 1, 0)));
        assert_eq!(parse_version("x"), None);
        assert!(parse_version("1.0.1").unwrap() < DUAL_TUNER_API);
    }

    #[test]
    fn the_binary_types_follow_the_api_table() {
        assert_eq!(binary(&[1, 0, 9]), Binary::Audio(Tuner::Primary));
        assert_eq!(binary(&[3, 0, 9]), Binary::Spectrum(Tuner::Primary));
        assert_eq!(binary(&[4, 0]), Binary::Audio(Tuner::Secondary));
        assert_eq!(binary(&[6, 0]), Binary::Spectrum(Tuner::Secondary));
        assert_eq!(binary(&[2, 1]), Binary::Unknown(0x0102));
        assert_eq!(binary(&[7, 0]), Binary::Unknown(7));
        assert_eq!(binary(&[]), Binary::Short);
        assert_eq!(
            (Tuner::Primary.iq_type(), Tuner::Secondary.iq_type()),
            (2, 5)
        );
    }

    /// Reads `n` samples from sample `from` of `payload` through a `Cs16` ring.
    fn convert(payload: &[u8], from: usize, n: usize) -> Vec<sdrkit_core::Complex32> {
        let (mut w, mut r) = sdrkit_driver_core::ring(16, sdrkit_graph::stream::SampleType::Cs16);
        assert_eq!(
            w.write(&payload[from * SAMPLE_BYTES..(from + n) * SAMPLE_BYTES]),
            n
        );
        let mut out = vec![sdrkit_core::Complex32::default(); n];
        assert_eq!(r.read_iq(&mut out), n);
        out
    }

    #[test]
    fn iq_payloads_are_signed_16_bit_pairs() {
        // The examples of the API: 02 00 00 40 00 C0 and 05 00 FF 7F 00 80.
        let Binary::Iq { payload, tuner } = binary(&[0x02, 0x00, 0x00, 0x40, 0x00, 0xC0]) else {
            panic!("not IQ")
        };
        assert_eq!(tuner, Tuner::Primary);
        assert_eq!(
            convert(payload, 0, 1),
            [sdrkit_core::Complex32::new(0.5, -0.5)]
        );
        let Binary::Iq { payload, tuner } = binary(&[0x05, 0x00, 0xFF, 0x7F, 0x00, 0x80]) else {
            panic!("not IQ")
        };
        assert_eq!(tuner, Tuner::Secondary);
        assert_eq!(
            convert(payload, 0, 1),
            [sdrkit_core::Complex32::new(32767.0 / 32768.0, -1.0)]
        );
        // A part of a payload, from sample 1.
        let p = [0, 0, 0, 0, 0x00, 0x20, 0x00, 0xE0];
        assert_eq!(
            convert(&p, 1, 1),
            [sdrkit_core::Complex32::new(0.25, -0.25)]
        );
    }
}
