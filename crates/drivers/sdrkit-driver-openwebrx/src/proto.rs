//! The OpenWebRX message protocol as pure functions: the messages of the client, the greeting of the server, the server
//! messages read through typed serde, the server state merged from `config`, the mode list, passbands and the tuning
//! arithmetic.
//!
//! Standard: the OpenWebRX WebSocket protocol, versions 0.18 and later.

use serde::Deserialize;
use serde_json::value::RawValue;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// The greeting of the client.
pub const GREETING: &str = "SERVER DE CLIENT client=sdrkit type=receiver";
/// The narrow audio rate the client requests.
pub const OUTPUT_RATE: u32 = 12_000;
/// The HD audio rate the client requests.
pub const HD_OUTPUT_RATE: u32 = 48_000;
/// The squelch value that means off.
pub const SQUELCH_OFF: f64 = -150.0;
/// The first version of the JSON protocol, as major × 1000 + minor.
pub const FIRST_JSON_VERSION: u32 = 18;

/// `connectionproperties`.
pub fn connection_properties() -> String {
    format!(
        "{{\"type\":\"connectionproperties\",\"params\":{{\"output_rate\":{OUTPUT_RATE},\"hd_output_rate\":{HD_OUTPUT_RATE}}}}}"
    )
}

/// `dspcontrol` `start`.
pub const DSP_START: &str = "{\"type\":\"dspcontrol\",\"action\":\"start\"}";

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// A number as JSON: whole numbers without a fraction.
pub fn json_num(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else if v.is_finite() {
        format!("{v}")
    } else {
        "0".into()
    }
}

/// `selectprofile`.
pub fn select_profile(id: &str) -> String {
    format!(
        "{{\"type\":\"selectprofile\",\"params\":{{\"profile\":{}}}}}",
        json_str(id)
    )
}

/// `setfrequency`.
pub fn set_frequency(hz: i64, key: &str) -> String {
    format!(
        "{{\"type\":\"setfrequency\",\"params\":{{\"frequency\":{hz},\"key\":{}}}}}",
        json_str(key)
    )
}

/// The parameters of the demodulator of the server.
#[derive(Clone, Debug, PartialEq)]
pub struct DspParams {
    /// `mod`.
    pub modulation: String,
    /// `offset_freq`.
    pub offset: i64,
    /// `low_cut`.
    pub low: i64,
    /// `high_cut`.
    pub high: i64,
    /// `squelch_level`.
    pub squelch: f64,
    /// `secondary_mod`: `None` is off (`false`).
    pub secondary: Option<String>,
    /// `secondary_offset_freq`, sent only when set.
    pub secondary_offset: Option<i64>,
    /// `dmr_filter`, only for `dmr`.
    pub dmr: Option<i64>,
}

/// The `dspcontrol` message from `prev` (what this connection sent, `None` at its start) to `new`: every key the first time,
/// only the keys that changed afterwards. `None` when nothing changed.
pub fn dsp_message(prev: Option<&DspParams>, new: &DspParams) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let first = prev.is_none();
    let changed = |f: &dyn Fn(&DspParams) -> String| prev.map(f) != Some(f(new));
    if first || changed(&|p| p.modulation.clone()) {
        parts.push(format!("\"mod\":{}", json_str(&new.modulation)));
    }
    if first || changed(&|p| p.offset.to_string()) {
        parts.push(format!("\"offset_freq\":{}", new.offset));
    }
    if first || changed(&|p| p.low.to_string()) {
        parts.push(format!("\"low_cut\":{}", new.low));
    }
    if first || changed(&|p| p.high.to_string()) {
        parts.push(format!("\"high_cut\":{}", new.high));
    }
    if first || changed(&|p| json_num(p.squelch)) {
        parts.push(format!("\"squelch_level\":{}", json_num(new.squelch)));
    }
    let sec_changed = prev.map(|p| &p.secondary) != Some(&new.secondary);
    if (first && new.secondary.is_some()) || (!first && sec_changed) {
        parts.push(match &new.secondary {
            Some(s) => format!("\"secondary_mod\":{}", json_str(s)),
            None => "\"secondary_mod\":false".into(),
        });
    }
    if let Some(o) = new.secondary_offset {
        if prev.map(|p| p.secondary_offset) != Some(Some(o)) {
            parts.push(format!("\"secondary_offset_freq\":{o}"));
        }
    }
    if let Some(d) = new.dmr {
        if prev.map(|p| p.dmr) != Some(Some(d)) {
            parts.push(format!("\"dmr_filter\":{d}"));
        }
    }
    (!parts.is_empty()).then(|| {
        format!(
            "{{\"type\":\"dspcontrol\",\"params\":{{{}}}}}",
            parts.join(",")
        )
    })
}

// ---- The greeting ----

/// What the first text message of the server says.
#[derive(Clone, Debug, PartialEq)]
pub enum Hello {
    /// A server that speaks the JSON protocol.
    Json {
        /// The `server=` name, if any.
        name: Option<String>,
        /// The version as sent.
        version_text: Option<String>,
        /// The version as numbers, if it parsed.
        version: Option<(u32, u32, u32)>,
    },
    /// A server older than 0.18 (the text protocol). The version, if the greeting provides one.
    Classic(Option<(u32, u32, u32)>),
    /// Not `CLIENT DE SERVER`.
    NotOpenWebRx,
}

impl Hello {
    /// The server is OpenWebRX by name with a known version.
    pub fn known(&self) -> bool {
        matches!(self, Hello::Json { name: Some(n), version: Some(_), .. } if n == "openwebrx")
    }
}

/// A version as one number: major × 1000 + minor.
pub fn version_number((major, minor, _): (u32, u32, u32)) -> u32 {
    major.saturating_mul(1000).saturating_add(minor)
}

/// Parses `MAJOR.MINOR[.PATCH]`. A suffix after `-` or a letter is left out.
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let end = v
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(v.len());
    let mut it = v[..end].split('.').filter(|s| !s.is_empty());
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// Reads the greeting line of the server.
pub fn parse_hello(text: &str) -> Hello {
    let Some(rest) = text.trim().strip_prefix("CLIENT DE SERVER") else {
        return Hello::NotOpenWebRx;
    };
    let mut name = None;
    let mut version_text = None;
    let mut bare = Vec::new();
    for w in rest.split_whitespace() {
        match w.split_once('=') {
            Some(("server", v)) => name = Some(v.to_string()),
            Some(("version", v)) => version_text = Some(v.to_string()),
            Some(_) => {}
            None => bare.push(w),
        }
    }
    if name.is_none() && version_text.is_none() && bare.iter().any(|w| w.starts_with("openwebrx")) {
        return Hello::Classic(None);
    }
    let version = version_text.as_deref().and_then(parse_version);
    if name.as_deref() == Some("openwebrx") {
        if let Some(v) = version {
            if version_number(v) < FIRST_JSON_VERSION {
                return Hello::Classic(Some(v));
            }
        }
    }
    Hello::Json {
        name: name.or_else(|| bare.first().map(|s| s.to_string())),
        version_text,
        version,
    }
}

// ---- Server messages ----

/// How audio or waterfall frames are coded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compression {
    /// IMA ADPCM.
    #[default]
    Adpcm,
    /// Not compressed.
    None,
}

impl Compression {
    fn parse(v: &str) -> Compression {
        if v.eq_ignore_ascii_case("none") {
            Compression::None
        } else {
            Compression::Adpcm
        }
    }
}

/// A value that the server sends as text or as a number.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Loose {
    /// Text.
    Text(String),
    /// A number, as the server wrote it.
    Number(serde_json::Number),
    /// Yes or no.
    Bool(bool),
}

impl Loose {
    /// The value as text.
    pub fn text(&self) -> String {
        match self {
            Loose::Text(s) => s.clone(),
            Loose::Number(n) => n.to_string(),
            Loose::Bool(b) => b.to_string(),
        }
    }
}

/// A `min` and `max` pair.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct MinMax {
    /// `min`.
    pub min: f64,
    /// `max`.
    pub max: f64,
}

/// The keys of a `config` message that the driver reads. A key with `null` is absent.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ConfigDelta {
    /// `center_freq`.
    pub center_freq: Option<f64>,
    /// `samp_rate`.
    pub samp_rate: Option<f64>,
    /// `fft_size`.
    pub fft_size: Option<f64>,
    /// `audio_compression`.
    pub audio_compression: Option<String>,
    /// `fft_compression`.
    pub fft_compression: Option<String>,
    /// `sdr_id`.
    pub sdr_id: Option<Loose>,
    /// `profile_id`.
    pub profile_id: Option<Loose>,
    /// `start_offset_freq`.
    pub start_offset_freq: Option<f64>,
    /// `start_freq`.
    pub start_freq: Option<f64>,
    /// `max_clients`.
    pub max_clients: Option<f64>,
    /// `waterfall_levels`.
    pub waterfall_levels: Option<MinMax>,
    /// `allow_center_freq_changes`.
    pub allow_center_freq_changes: Option<bool>,
}

/// One entry of the `profiles` list.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ProfileEntry {
    /// `id`: `<sdr_id>|<profile_id>`.
    pub id: String,
    /// `name`.
    pub name: Option<String>,
}

/// The passband of a mode.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Bandpass {
    /// `low_cut`.
    pub low_cut: f64,
    /// `high_cut`.
    pub high_cut: f64,
}

/// One entry of the `modes` list.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ModeEntry {
    /// `modulation`.
    pub modulation: String,
    /// `name`.
    pub name: Option<String>,
    /// `requirements`.
    pub requirements: Option<Vec<String>>,
    /// `bandpass`.
    pub bandpass: Option<Bandpass>,
    /// `underlying`.
    pub underlying: Option<Vec<String>>,
}

/// One entry of the `bookmarks` list.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Bookmark {
    /// `name`.
    pub name: Option<Loose>,
    /// `frequency`.
    pub frequency: f64,
    /// `modulation`.
    pub modulation: Option<Loose>,
}

/// One entry of the `dial_frequencies` list.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Dial {
    /// `frequency`.
    pub frequency: f64,
    /// `mode`.
    pub mode: Option<Loose>,
}

/// The position of the receiver.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Gps {
    /// `lat`.
    pub lat: f64,
    /// `lon`.
    pub lon: f64,
}

/// The `receiver_details` value.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Details {
    /// `receiver_name`.
    pub receiver_name: Option<Loose>,
    /// `receiver_location`.
    pub receiver_location: Option<Loose>,
    /// `receiver_asl`, metres.
    pub receiver_asl: Option<Loose>,
    /// `receiver_admin`.
    pub receiver_admin: Option<Loose>,
    /// `receiver_gps`.
    pub receiver_gps: Option<Gps>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Clients {
    Count(f64),
    Object { count: f64 },
}

#[derive(Deserialize)]
struct SecondaryConfig {
    secondary_fft_size: Option<f64>,
    if_samp_rate: Option<f64>,
}

/// One decode of a decoder of the server.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decode {
    /// `mode`.
    pub mode: String,
    /// `freq`, Hz.
    pub freq: Option<f64>,
    /// `db`.
    pub db: Option<f64>,
    /// `dt`, seconds.
    pub dt: Option<f64>,
    /// `msg`, or `text` when there is no `msg`.
    pub message: String,
    /// The other keys, as a JSON object. Empty when there are none.
    pub rest: String,
}

/// A server message that the driver reads.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// `config`.
    Config(ConfigDelta),
    /// `secondary_config`: the FFT size and the rate of the decoder spectrum.
    SecondaryConfig {
        /// `secondary_fft_size`.
        size: f64,
        /// `if_samp_rate`.
        rate: f64,
    },
    /// `receiver_details`.
    Details(Details),
    /// `profiles`.
    Profiles(Vec<ProfileEntry>),
    /// `features`.
    Features(BTreeMap<String, bool>),
    /// `modes`.
    Modes(Vec<Mode>),
    /// `bookmarks`.
    Bookmarks(Vec<Bookmark>),
    /// `dial_frequencies`.
    Dials(Vec<Dial>),
    /// `smeter`: linear power.
    Smeter(f64),
    /// `clients`.
    Clients(f64),
    /// `cpuusage`: 0 to 1.
    Cpu(f64),
    /// `temperature`, °C.
    Temperature(f64),
    /// `metadata` of digital voice: name and value pairs.
    Metadata(Vec<(String, String)>),
    /// `secondary_demod` as text.
    DecoderText(String),
    /// `secondary_demod` as an object, or a `*_message` or `*_data` message.
    Decode(Decode),
    /// `log_message`.
    Log(String),
    /// `sdr_error`.
    SdrError(String),
    /// `demodulator_error`.
    DemodError(String),
    /// `chat_message`.
    Chat {
        /// `name`.
        name: String,
        /// `text`.
        text: String,
    },
    /// `backoff`.
    Backoff {
        /// `reason`.
        reason: String,
    },
    /// A message of a type the driver does not use.
    Unused(String),
}

/// Why a text message was not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    /// It is not JSON.
    NotJson,
    /// It has no `type`.
    NoType,
    /// Its value does not have the form of its type.
    Malformed(String),
}

#[derive(Deserialize)]
struct Envelope<'a> {
    #[serde(rename = "type", borrow)]
    ty: Option<Cow<'a, str>>,
    #[serde(borrow)]
    value: Option<&'a RawValue>,
    name: Option<Loose>,
    text: Option<Loose>,
    reason: Option<Loose>,
}

/// The text of a raw value: a string as it is, `null` as empty, anything else as its JSON text.
fn raw_text(v: &RawValue) -> String {
    match serde_json::from_str::<Option<String>>(v.get()) {
        Ok(Some(s)) => s,
        Ok(None) => String::new(),
        Err(_) => v.get().to_string(),
    }
}

fn opt_text(v: &Option<Loose>) -> String {
    v.as_ref().map(Loose::text).unwrap_or_default()
}

fn decode_row(kind: &str, raw: &str) -> Option<Decode> {
    let o: BTreeMap<String, &RawValue> = serde_json::from_str(raw).ok()?;
    let text = |k: &str| o.get(k).map(|v| raw_text(v)).unwrap_or_default();
    let num = |k: &str| {
        o.get(k)
            .and_then(|v| serde_json::from_str::<f64>(v.get()).ok())
    };
    let mode = match text("mode") {
        m if m.is_empty() => kind
            .trim_end_matches("_message")
            .trim_end_matches("_data")
            .to_uppercase(),
        m => m,
    };
    let message = match text("msg") {
        m if m.is_empty() => text("text"),
        m => m,
    };
    let rest: Vec<String> = o
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "mode" | "msg" | "text" | "db" | "dt" | "freq"))
        .map(|(k, v)| format!("{}:{}", json_str(k), v.get()))
        .collect();
    Some(Decode {
        mode,
        freq: num("freq"),
        db: num("db"),
        dt: num("dt"),
        message,
        rest: if rest.is_empty() {
            String::new()
        } else {
            format!("{{{}}}", rest.join(","))
        },
    })
}

/// Reads one text message of the server.
///
/// # Errors
///
/// [`Unread`] for a message that is not JSON, has no type, or whose value does not have the form of its type.
pub fn parse(raw: &[u8]) -> Result<Msg, Unread> {
    let e: Envelope = serde_json::from_slice(raw).map_err(|_| Unread::NotJson)?;
    let ty = e.ty.ok_or(Unread::NoType)?;
    let value = e.value.map(RawValue::get).unwrap_or("null");
    fn val<'a, T: Deserialize<'a>>(ty: &str, v: &'a str) -> Result<T, Unread> {
        serde_json::from_str(v).map_err(|_| Unread::Malformed(ty.to_string()))
    }
    let t = ty.as_ref();
    Ok(match t {
        "config" => Msg::Config(val(t, value)?),
        "secondary_config" => {
            let c: SecondaryConfig = val(t, value)?;
            match (c.secondary_fft_size, c.if_samp_rate) {
                (Some(size), Some(rate)) => Msg::SecondaryConfig { size, rate },
                _ => return Err(Unread::Malformed(t.to_string())),
            }
        }
        "receiver_details" => Msg::Details(val(t, value)?),
        "profiles" => Msg::Profiles(val(t, value)?),
        "features" => {
            let m: BTreeMap<String, &RawValue> = val(t, value)?;
            Msg::Features(m.into_iter().map(|(k, v)| (k, v.get() == "true")).collect())
        }
        "modes" => {
            let m: Vec<ModeEntry> = val(t, value)?;
            Msg::Modes(m.into_iter().map(Mode::from).collect())
        }
        "bookmarks" => Msg::Bookmarks(val(t, value)?),
        "dial_frequencies" => Msg::Dials(val(t, value)?),
        "smeter" => Msg::Smeter(val(t, value)?),
        "clients" => Msg::Clients(match val(t, value)? {
            Clients::Count(n) | Clients::Object { count: n } => n,
        }),
        "cpuusage" => Msg::Cpu(val(t, value)?),
        "temperature" => Msg::Temperature(val(t, value)?),
        "metadata" => {
            let m: BTreeMap<String, &RawValue> = val(t, value)?;
            Msg::Metadata(m.into_iter().map(|(k, v)| (k, raw_text(v))).collect())
        }
        "secondary_demod" => match decode_row(t, value) {
            Some(d) => Msg::Decode(d),
            None => Msg::DecoderText(e.value.map(raw_text).unwrap_or_default()),
        },
        "log_message" => Msg::Log(e.value.map(raw_text).unwrap_or_default()),
        "sdr_error" => Msg::SdrError(e.value.map(raw_text).unwrap_or_default()),
        "demodulator_error" => Msg::DemodError(e.value.map(raw_text).unwrap_or_default()),
        "chat_message" => Msg::Chat {
            name: opt_text(&e.name),
            text: opt_text(&e.text),
        },
        "backoff" => Msg::Backoff {
            reason: opt_text(&e.reason),
        },
        t if t.ends_with("_message") || t.ends_with("_data") => match decode_row(t, value) {
            Some(d) => Msg::Decode(d),
            None => return Err(Unread::Malformed(t.to_string())),
        },
        t => Msg::Unused(t.to_string()),
    })
}

// ---- Server state ----

/// What the `config` messages said, merged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerConfig {
    /// `center_freq`.
    pub centre: Option<f64>,
    /// `samp_rate`.
    pub rate: Option<f64>,
    /// `fft_size`.
    pub fft_size: Option<usize>,
    /// `audio_compression`.
    pub audio_comp: Compression,
    /// `fft_compression`.
    pub fft_comp: Compression,
    /// `sdr_id`.
    pub sdr_id: Option<String>,
    /// `profile_id`.
    pub profile_id: Option<String>,
    /// `start_offset_freq`.
    pub start_offset: Option<f64>,
    /// `start_freq`.
    pub start_freq: Option<f64>,
    /// `max_clients`.
    pub max_clients: Option<i64>,
    /// `waterfall_levels`.
    pub waterfall_levels: Option<(f64, f64)>,
    /// `allow_center_freq_changes`.
    pub allow_centre: Option<bool>,
}

/// What a `config` changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConfigChange {
    /// The centre or the rate changed.
    pub band: bool,
    /// The profile changed.
    pub profile: bool,
    /// The FFT size changed.
    pub fft: bool,
    /// The compression changed.
    pub compression: bool,
}

fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite())
}

impl ServerConfig {
    /// Merges a `config` value.
    pub fn merge(&mut self, m: &ConfigDelta) -> ConfigChange {
        let mut ch = ConfigChange::default();
        if let Some(c) = finite(m.center_freq).map(f64::round) {
            ch.band |= self.centre != Some(c);
            self.centre = Some(c);
        }
        if let Some(r) = finite(m.samp_rate).map(f64::round) {
            ch.band |= self.rate != Some(r);
            self.rate = Some(r);
        }
        if let Some(n) = finite(m.fft_size) {
            let n = n.max(0.0) as usize;
            ch.fft |= self.fft_size != Some(n);
            self.fft_size = Some(n);
        }
        if let Some(c) = &m.audio_compression {
            let c = Compression::parse(c);
            ch.compression |= self.audio_comp != c;
            self.audio_comp = c;
        }
        if let Some(c) = &m.fft_compression {
            let c = Compression::parse(c);
            ch.compression |= self.fft_comp != c;
            self.fft_comp = c;
        }
        if let Some(s) = m.sdr_id.as_ref().map(Loose::text) {
            ch.profile |= self.sdr_id.as_ref() != Some(&s);
            self.sdr_id = Some(s);
        }
        if let Some(s) = m.profile_id.as_ref().map(Loose::text) {
            ch.profile |= self.profile_id.as_ref() != Some(&s);
            self.profile_id = Some(s);
        }
        if let Some(v) = finite(m.start_offset_freq) {
            self.start_offset = Some(v.round());
        }
        if let Some(v) = finite(m.start_freq) {
            self.start_freq = Some(v.round());
        }
        if let Some(v) = finite(m.max_clients) {
            self.max_clients = Some(v as i64);
        }
        if let Some(p) = m.waterfall_levels {
            self.waterfall_levels = Some((p.min, p.max));
        }
        if let Some(b) = m.allow_center_freq_changes {
            self.allow_centre = Some(b);
        }
        ch
    }

    /// The band, once the centre and the rate are known.
    pub fn band(&self) -> Option<Band> {
        Some(Band {
            centre: self.centre?,
            rate: self.rate?,
        })
    }

    /// The profile in use, `<sdr_id>|<profile_id>`.
    pub fn profile(&self) -> Option<String> {
        Some(format!(
            "{}|{}",
            self.sdr_id.as_deref()?,
            self.profile_id.as_deref()?
        ))
    }

    /// The start that the server suggests, a demodulator frequency.
    pub fn start(&self) -> Option<f64> {
        if self.start_offset.is_none() {
            if let Some(s) = self.start_freq {
                return Some(s);
            }
        }
        Some(self.centre? + self.start_offset.unwrap_or(0.0))
    }
}

// ---- Bands and tuning ----

/// A band: centre ± rate / 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// The centre, Hz.
    pub centre: f64,
    /// The width, Hz.
    pub rate: f64,
}

impl Band {
    /// The low edge.
    pub fn lo(&self) -> f64 {
        self.centre - self.rate / 2.0
    }
    /// The high edge.
    pub fn hi(&self) -> f64 {
        self.centre + self.rate / 2.0
    }
    /// The passband `d + low .. d + high` lies inside the band.
    pub fn fits(&self, d: f64, low: f64, high: f64) -> bool {
        d + low >= self.lo() - 0.5 && d + high <= self.hi() + 0.5
    }
    /// The nearest demodulator frequency whose passband fits.
    pub fn nearest(&self, d: f64, low: f64, high: f64) -> f64 {
        let (a, b) = (self.lo() - low, self.hi() - high);
        if a > b {
            self.centre
        } else {
            d.clamp(a, b)
        }
    }
    /// "A to B MHz".
    pub fn text(&self) -> String {
        format!("{} to {} MHz", fmt_mhz(self.lo()), fmt_mhz(self.hi()))
    }
}

/// A frequency in MHz with up to 6 decimals, without trailing zeros ("7.0767", "13.975").
pub fn fmt_mhz(hz: f64) -> String {
    let s = format!("{:.6}", hz / 1e6);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// The width B of the `iq` delivery: the USB passband of the server, 0 to B Hz above the dial.
pub fn iq_width(audio_rate: f64) -> f64 {
    (0.45 * audio_rate).round()
}

// ---- Modes and passbands ----

/// One mode of the list of the server.
#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    /// `modulation`.
    pub id: String,
    /// `name`.
    pub name: String,
    /// `requirements`.
    pub requirements: Vec<String>,
    /// `bandpass`.
    pub bandpass: Option<(f64, f64)>,
    /// `underlying`.
    pub underlying: Vec<String>,
}

impl From<ModeEntry> for Mode {
    fn from(m: ModeEntry) -> Mode {
        Mode {
            name: m.name.unwrap_or_else(|| m.modulation.clone()),
            id: m.modulation,
            requirements: m.requirements.unwrap_or_default(),
            bandpass: m.bandpass.map(|b| (b.low_cut, b.high_cut)),
            underlying: m.underlying.unwrap_or_default(),
        }
    }
}

impl Mode {
    /// A decoder that runs on the audio of another demodulator.
    pub fn is_decoder(&self) -> bool {
        !self.underlying.is_empty()
    }
    /// Every requirement is met. An unknown feature list meets all.
    pub fn available(&self, features: Option<&BTreeMap<String, bool>>) -> bool {
        match features {
            None => true,
            Some(f) => self
                .requirements
                .iter()
                .all(|r| f.get(r).copied().unwrap_or(false)),
        }
    }
}

/// The built-in passband of an analogue mode.
pub fn default_passband(id: &str) -> Option<(f64, f64)> {
    Some(match id {
        "nfm" | "am" | "sam" => (-4000.0, 4000.0),
        "usb" => (300.0, 3000.0),
        "lsb" => (-3000.0, -300.0),
        "cw" => (700.0, 900.0),
        "wfm" => (-75000.0, 75000.0),
        _ => return None,
    })
}

/// The passband of the `audio` delivery: from the passband setting `w`, else the passband of the mode, else the built-in
/// table. A narrow mode stays inside ± half the narrow audio rate, and the passband is at least 100 Hz wide.
pub fn audio_passband(id: &str, w: f64, mode_bandpass: Option<(f64, f64)>) -> (f64, f64) {
    let (mut lo, mut hi) = if w > 0.0 {
        match id {
            "usb" => (0.0, w),
            "lsb" => (-w, 0.0),
            "cw" => (800.0 - w / 2.0, 800.0 + w / 2.0),
            _ => (-w / 2.0, w / 2.0),
        }
    } else {
        mode_bandpass
            .or_else(|| default_passband(id))
            .unwrap_or((-4000.0, 4000.0))
    };
    if id != "wfm" {
        let lim = f64::from(OUTPUT_RATE) / 2.0;
        lo = lo.clamp(-lim, lim);
        hi = hi.clamp(-lim, lim);
    }
    if hi < lo {
        std::mem::swap(&mut lo, &mut hi);
    }
    if hi - lo < 100.0 {
        let mid = (lo + hi) / 2.0;
        (lo, hi) = (mid - 50.0, mid + 50.0);
    }
    (lo.round(), hi.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iq(offset: i64) -> DspParams {
        DspParams {
            modulation: "usb".into(),
            offset,
            low: 0,
            high: 5400,
            squelch: -150.0,
            secondary: None,
            secondary_offset: None,
            dmr: None,
        }
    }

    #[test]
    fn client_messages_byte_for_byte() {
        assert_eq!(GREETING.len(), 44);
        let cp = connection_properties();
        assert_eq!(
            cp,
            r#"{"type":"connectionproperties","params":{"output_rate":12000,"hd_output_rate":48000}}"#
        );
        assert_eq!(cp.len(), 85);
        let first = dsp_message(None, &iq(74000)).unwrap();
        assert_eq!(
            first,
            r#"{"type":"dspcontrol","params":{"mod":"usb","offset_freq":74000,"low_cut":0,"high_cut":5400,"squelch_level":-150}}"#
        );
        assert_eq!(first.len(), 113);
        assert_eq!(DSP_START.len(), 38);
        let d = dsp_message(Some(&iq(74000)), &iq(-102000)).unwrap();
        assert_eq!(
            d,
            r#"{"type":"dspcontrol","params":{"offset_freq":-102000}}"#
        );
        assert_eq!(d.len(), 54);
        assert_eq!(dsp_message(Some(&iq(1)), &iq(1)), None);
        let sp = select_profile("rtl|40m");
        assert_eq!(
            sp,
            r#"{"type":"selectprofile","params":{"profile":"rtl|40m"}}"#
        );
        assert_eq!(sp.len(), 55);
        let sf = set_frequency(7102700, "s3cret");
        assert_eq!(sf.len(), 69);
        let mut a = iq(0);
        a.modulation = "dmr".into();
        a.secondary = Some("x".into());
        a.dmr = Some(3);
        a.secondary_offset = Some(1000);
        assert_eq!(
            dsp_message(None, &a).unwrap(),
            r#"{"type":"dspcontrol","params":{"mod":"dmr","offset_freq":0,"low_cut":0,"high_cut":5400,"squelch_level":-150,"secondary_mod":"x","secondary_offset_freq":1000,"dmr_filter":3}}"#
        );
        let mut b = a.clone();
        b.secondary = None;
        assert_eq!(
            dsp_message(Some(&a), &b).unwrap(),
            r#"{"type":"dspcontrol","params":{"secondary_mod":false}}"#
        );
    }

    #[test]
    fn greetings_and_versions() {
        let h = parse_hello("CLIENT DE SERVER server=openwebrx version=1.2.2");
        assert!(h.known());
        assert_eq!(
            h,
            Hello::Json {
                name: Some("openwebrx".into()),
                version_text: Some("1.2.2".into()),
                version: Some((1, 2, 2))
            }
        );
        assert_eq!(
            parse_hello("CLIENT DE SERVER openwebrx.py"),
            Hello::Classic(None)
        );
        assert_eq!(
            parse_hello("CLIENT DE SERVER server=openwebrx version=0.17.3"),
            Hello::Classic(Some((0, 17, 3)))
        );
        assert!(matches!(
            parse_hello("CLIENT DE SERVER server=openwebrx version=1.2.70-plus"),
            Hello::Json {
                version: Some((1, 2, 70)),
                ..
            }
        ));
        assert_eq!(parse_version("1.3.0-dev"), Some((1, 3, 0)));
        assert_eq!(parse_version("0.18"), Some((0, 18, 0)));
        assert_eq!(version_number((1, 2, 2)), 1002);
        assert_eq!(
            parse_hello(r#"{"type":"config","value":{}}"#),
            Hello::NotOpenWebRx
        );
        assert!(!parse_hello("CLIENT DE SERVER server=fork").known());
    }

    fn config(text: &str) -> ConfigDelta {
        match parse(text.as_bytes()).unwrap() {
            Msg::Config(c) => c,
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn config_merges_deltas() {
        let mut c = ServerConfig::default();
        let ch = c.merge(&config(
            r#"{"type":"config","value":{"sdr_id":"rtl","profile_id":"20m","center_freq":14100000,"samp_rate":250000,
            "fft_size":4096,"audio_compression":"adpcm","fft_compression":"adpcm","max_clients":20,"start_mod":"usb",
            "start_offset_freq":74000,"waterfall_levels":{"min":-88,"max":-20},"tuning_step":10}}"#,
        ));
        assert!(ch.band && ch.profile && ch.fft);
        let b = c.band().unwrap();
        assert_eq!(b.text(), "13.975 to 14.225 MHz");
        assert_eq!(c.profile().as_deref(), Some("rtl|20m"));
        assert_eq!(c.start(), Some(14_174_000.0));
        assert_eq!(c.waterfall_levels, Some((-88.0, -20.0)));
        let ch = c.merge(&config(
            r#"{"type":"config","value":{"center_freq":14150000.4,"lfo_offset":null,"sdr_id":null}}"#,
        ));
        assert!(ch.band && !ch.profile);
        assert_eq!(c.centre, Some(14_150_000.0));
        assert_eq!(c.rate, Some(250_000.0));
        // An id sent as a number reads as its text.
        c.merge(&config(r#"{"type":"config","value":{"profile_id":7}}"#));
        assert_eq!(c.profile().as_deref(), Some("rtl|7"));
    }

    #[test]
    fn messages_read_typed() {
        assert_eq!(parse(b"not json"), Err(Unread::NotJson));
        assert_eq!(parse(br#"{"value":1}"#), Err(Unread::NoType));
        assert_eq!(
            parse(br#"{"type":"smeter","value":"x"}"#),
            Err(Unread::Malformed("smeter".into()))
        );
        assert_eq!(
            parse(br#"{"type":"clients","value":{"count":4}}"#),
            Ok(Msg::Clients(4.0))
        );
        assert_eq!(
            parse(br##"{"type":"chat_message","name":"op","text":"hi all","color":"#fff"}"##),
            Ok(Msg::Chat {
                name: "op".into(),
                text: "hi all".into()
            })
        );
        let Ok(Msg::Decode(d)) = parse(
            br#"{"type":"wsjt_message","value":{"mode":"WSPR","msg":"K1ABC FN42 37","db":-20,"dt":0.1,"freq":1500,"drift":0}}"#,
        ) else {
            panic!()
        };
        assert_eq!(
            (
                d.mode.as_str(),
                d.freq,
                d.db,
                d.dt,
                d.message.as_str(),
                d.rest.as_str()
            ),
            (
                "WSPR",
                Some(1500.0),
                Some(-20.0),
                Some(0.1),
                "K1ABC FN42 37",
                r#"{"drift":0}"#
            )
        );
        assert_eq!(
            parse(br#"{"type":"secondary_demod","value":"CQ"}"#),
            Ok(Msg::DecoderText("CQ".into()))
        );
        assert_eq!(
            parse(br#"{"type":"metadata","value":{"protocol":"DMR","slot":1}}"#),
            Ok(Msg::Metadata(vec![
                ("protocol".into(), "DMR".into()),
                ("slot".into(), "1".into())
            ]))
        );
        assert_eq!(
            parse(br#"{"type":"nonsense","value":1}"#),
            Ok(Msg::Unused("nonsense".into()))
        );
    }

    #[test]
    fn bands_fit_and_nearest() {
        let b = Band {
            centre: 7_100_000.0,
            rate: 250_000.0,
        };
        assert!(b.fits(7_074_000.0, 0.0, 5400.0));
        assert!(!b.fits(7_224_000.0, 0.0, 5400.0));
        assert_eq!(b.nearest(14_174_000.0, 0.0, 5400.0), 7_219_600.0);
        assert_eq!(fmt_mhz(7_222_300.0), "7.2223");
        assert_eq!(fmt_mhz(7_076_700.0), "7.0767");
        assert_eq!(iq_width(12_000.0), 5400.0);
    }

    #[test]
    fn passbands() {
        assert_eq!(audio_passband("usb", 0.0, None), (300.0, 3000.0));
        assert_eq!(audio_passband("usb", 2400.0, None), (0.0, 2400.0));
        assert_eq!(audio_passband("lsb", 2400.0, None), (-2400.0, 0.0));
        assert_eq!(audio_passband("cw", 500.0, None), (550.0, 1050.0));
        assert_eq!(audio_passband("am", 20000.0, None), (-6000.0, 6000.0));
        assert_eq!(audio_passband("wfm", 0.0, None), (-75000.0, 75000.0));
        assert_eq!(audio_passband("nfm", 50.0, None), (-50.0, 50.0));
        assert_eq!(
            audio_passband("dmr", 0.0, Some((-6250.0, 6250.0))),
            (-6000.0, 6000.0)
        );
        let Ok(Msg::Modes(modes)) = parse(
            br#"{"type":"modes","value":[
            {"modulation":"nfm","name":"FM","type":"analog","squelch":true,"bandpass":{"low_cut":-4000,"high_cut":4000}},
            {"modulation":"ft8","name":"FT8","type":"digital","requirements":["wsjt-x"],"underlying":["usb"],"secondaryFft":true}
            ]}"#,
        ) else {
            panic!()
        };
        assert_eq!(modes.len(), 2);
        assert!(modes[1].is_decoder());
        assert_eq!(modes[0].bandpass, Some((-4000.0, 4000.0)));
        let mut f = BTreeMap::new();
        assert!(!modes[1].available(Some(&f)));
        f.insert("wsjt-x".to_string(), true);
        assert!(modes[1].available(Some(&f)));
    }
}
