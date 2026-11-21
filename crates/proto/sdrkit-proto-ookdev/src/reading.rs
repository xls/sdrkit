//! Device readings: what a protocol decoder makes from bit rows.

use sdrkit_core::interface::{Cell, Column, EventShape, Fields, Hertz, Seconds};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{field, Declare};
use sdrkit_core::record::Kind;
use sdrkit_core::{EventDef, FieldDecl, Record, Value as CoreValue, ValueKind};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Write;
/// Text that is static for most readings (names, units) and owned only when a decoder makes it at run time.
pub type Text = Cow<'static, str>;

sdrkit_core::choice_enum! {
    /// The frame test that accepted a frame.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Check {
        /// A cyclic redundancy check (CRC).
        Crc = ("crc", "CRC"),
        /// A checksum (a sum, an XOR or an LFSR digest of the bytes).
        Checksum = ("checksum", "Checksum"),
        /// A parity bit.
        Parity = ("parity", "Parity"),
        /// The same row received several times in one train.
        Repeat = ("repeat", "Repeat"),
        /// The Manchester code holds over the whole frame.
        Manchester = ("manchester", "Manchester"),
    }
}

/// A measurement field that a device decoder writes: one entry of the catalogue in [`measure`], the column of the same
/// key of [`ReadingFields`]. Only this module makes them.
pub struct Measure<K: Kind>(Column<K>);

impl<K: Kind> Measure<K> {
    /// The column of [`ReadingFields`].
    pub const fn column(&self) -> &Column<K> {
        &self.0
    }
}

impl<K: Kind> Declare for Measure<K> {
    fn key(&self) -> &'static str {
        self.0.key
    }
    fn kind(&self) -> ValueKind {
        K::VALUE_KIND
    }
    fn decl(&self) -> FieldDecl {
        self.0.decl()
    }
}

/// The value of `v` as the struct field type `C`, if `v` holds a value of the kind of `C`.
fn cell<'a, C: Cell<'a>>(v: &'a CoreValue) -> Option<C> {
    <C::Kind as Kind>::load(v).map(C::take)
}

/// Makes [`ReadingFields`], the [`measure`] catalogue and the fill of a reading's measurements from one list.
macro_rules! reading_fields {
    ($($(#[$doc:meta])* $key:ident: $kind:ident = $ty:ty, $cname:ident, $label:tt, $unit:tt;)*) => {
        /// The fields of an `ookdev.reading` event. `source` is the model and the id, `text` the measurements as
        /// [`Reading::line`] returns them. A measurement that the device does not give has no value.
        #[derive(Clone, Debug, PartialEq, Fields)]
        pub struct ReadingFields<'a> {
            /// The model and the id.
            #[field(shared = keys::SOURCE)]
            pub source: &'a str,
            /// The measurements as one line.
            #[field(shared = keys::TEXT)]
            pub text: &'a str,
            /// The channel, if the device sends one.
            #[field(shared = keys::CHANNEL)]
            pub channel: Option<&'a str>,
            /// The family of the event.
            #[field(shared = keys::FAMILY)]
            pub family: Family,
            /// The model of the device.
            #[field(label = "Model")]
            pub model: &'a str,
            /// The device kind.
            #[field(label = "Device kind")]
            pub device_kind: &'a str,
            /// The device id as the protocol shows it.
            #[field(label = "ID")]
            pub id: &'a str,
            /// The battery state: yes for good, no for low.
            #[field(label = "Battery good")]
            pub battery_ok: Option<bool>,
            /// The frame test that accepted the frame.
            #[field(label = "Check")]
            pub check: Check,
            /// The frequency of the signal from the centre of the stream.
            #[field(shared = keys::OFFSET_HZ)]
            pub offset_hz: Hertz,
            /// The signal level.
            #[field(shared = keys::LEVEL_DBFS)]
            pub level_dbfs: f64,
            /// The signal-to-noise ratio.
            #[field(shared = keys::SNR_DB)]
            pub snr_db: f64,
            $($(#[$doc])* #[field(label = $label, unit = $unit)] pub $key: Option<$ty>,)*
        }

        impl<'a> ReadingFields<'a> {
            /// The fields of `r` with the texts `source`, `text` and `channel`.
            fn of(r: &'a Reading, source: &'a str, text: &'a str, channel: Option<&'a str>) -> Self {
                let mut f = ReadingFields {
                    source,
                    text,
                    channel,
                    family: Family::Sensor,
                    model: &r.model,
                    device_kind: &r.kind,
                    id: &r.id,
                    battery_ok: r.battery_ok,
                    check: r.check,
                    offset_hz: Hertz(r.offset_hz),
                    level_dbfs: f64::from(r.level_dbfs),
                    snr_db: f64::from(r.snr_db),
                    $($key: None,)*
                };
                for m in &r.fields {
                    let key = m.key();
                    $(if key == ReadingFields::$cname.key {
                        f.$key = cell(&m.value);
                        continue;
                    })*
                }
                f
            }
        }

        /// The measurements that device decoders give, declared once with their units: the columns of
        /// [`ReadingFields`] after the standard fields. A field that a device does not give has no value.
        pub mod measure {
            use super::{Measure, ReadingFields, STANDARD_FIELDS};
            use sdrkit_core::interface::Fields;
            use sdrkit_core::publish::{field, Declare};

            $($(#[$doc])* pub const $cname: Measure<field::$kind> = Measure(ReadingFields::$cname);)*

            /// Kilopascals in one conventional inch of mercury.
            ///
            /// Standard: NIST Special Publication 811 (2008), Appendix B.8, "inch of mercury, conventional".
            pub const KPA_PER_INHG: f64 = 3.386_389;
            /// Watts in one British thermal unit (IT) per hour.
            ///
            /// Standard: NIST Special Publication 811 (2008), Appendix B.8, "British thermal unit_IT per hour".
            pub const W_PER_BTU_IT_H: f64 = 0.293_071_1;

            /// Every measurement field, in the order of the event.
            pub const ALL: &[&dyn Declare] =
                <ReadingFields<'static> as Fields>::SHAPE.fields.split_at(STANDARD_FIELDS).1;
        }
    };
}

/// The number of standard fields at the start of [`ReadingFields`].
const STANDARD_FIELDS: usize = 12;

reading_fields! {
    /// Air temperature.
    temperature_c: F64 = f64, TEMPERATURE_C, "Temperature", "°C";
    /// The temperature of the first probe.
    temperature_1_c: F64 = f64, TEMPERATURE_1_C, "Temperature 1", "°C";
    /// The temperature of the second probe.
    temperature_2_c: F64 = f64, TEMPERATURE_2_C, "Temperature 2", "°C";
    /// Black globe temperature.
    globe_temperature_c: F64 = f64, GLOBE_TEMPERATURE_C, "Globe temperature", "°C";
    /// Relative humidity.
    humidity_pct: F64 = f64, HUMIDITY_PCT, "Humidity", "%";
    /// Absolute humidity: grams of water in a cubic metre of air.
    absolute_humidity_g_m3: F64 = f64, ABSOLUTE_HUMIDITY_G_M3, "Absolute humidity", "g/m³";
    /// Soil moisture.
    moisture_pct: F64 = f64, MOISTURE_PCT, "Moisture", "%";
    /// Pressure (air or tyre). A decoder that receives inches of mercury multiplies them by [`KPA_PER_INHG`](crate::reading::measure::KPA_PER_INHG).
    pressure_kpa: F64 = f64, PRESSURE_KPA, "Pressure", "kPa";
    /// Mean wind speed.
    wind_speed_m_s: F64 = f64, WIND_SPEED_M_S, "Wind speed", "m/s";
    /// Wind gust speed.
    wind_gust_m_s: F64 = f64, WIND_GUST_M_S, "Wind gust", "m/s";
    /// Wind direction, degrees clockwise from north.
    wind_dir_deg: F64 = f64, WIND_DIR_DEG, "Wind direction", "°";
    /// Rain since the device started counting.
    rain_mm: F64 = f64, RAIN_MM, "Rain", "mm";
    /// UV index.
    uv_index: F64 = f64, UV_INDEX, "UV index", "";
    /// Light.
    light_lux: F64 = f64, LIGHT_LUX, "Light", "lux";
    /// Battery charge.
    battery_level_pct: F64 = f64, BATTERY_LEVEL_PCT, "Battery level", "%";
    /// A fill or dimmer level.
    level_pct: F64 = f64, LEVEL_PCT, "Level", "%";
    /// Power of the first channel.
    power_0_w: F64 = f64, POWER_0_W, "Power 0", "W";
    /// Power of the second channel.
    power_1_w: F64 = f64, POWER_1_W, "Power 1", "W";
    /// Power of the third channel.
    power_2_w: F64 = f64, POWER_2_W, "Power 2", "W";
    /// Power. A decoder that receives British thermal units (IT) per hour multiplies them by [`W_PER_BTU_IT_H`](crate::reading::measure::W_PER_BTU_IT_H).
    power_w: F64 = f64, POWER_W, "Power", "W";
    /// Current.
    current_a: F64 = f64, CURRENT_A, "Current", "A";
    /// Voltage.
    voltage_v: F64 = f64, VOLTAGE_V, "Voltage", "V";
    /// Energy that a meter counted.
    energy_kwh: F64 = f64, ENERGY_KWH, "Energy", "kWh";
    /// Gas volume that a meter counted.
    gas_volume_m3: F64 = f64, GAS_VOLUME_M3, "Gas volume", "m³";
    /// Water volume that a meter counted.
    water_volume_m3: F64 = f64, WATER_VOLUME_M3, "Water volume", "m³";
    /// Luminance as a share of the full scale of the sensor.
    luminance_pct: F64 = f64, LUMINANCE_PCT, "Luminance", "%";
    /// Carbon dioxide.
    co2_ppm: F64 = f64, CO2_PPM, "CO2", "ppm";
    /// Formaldehyde.
    hcho_ppb: F64 = f64, HCHO_PPB, "HCHO", "ppb";
    /// Particles up to 2.5 µm.
    pm2_5_ug_m3: F64 = f64, PM2_5_UG_M3, "PM2.5", "µg/m³";
    /// Particles up to 10 µm.
    pm10_ug_m3: F64 = f64, PM10_UG_M3, "PM10", "µg/m³";
    /// A meter reading or a sensor value without a unit.
    value: F64 = f64, VALUE, "Value", "";
    /// The time between two reports.
    interval: Duration = Seconds, INTERVAL, "Interval", "s";
    /// The time between two wake-ups.
    wake_up_interval: Duration = Seconds, WAKE_UP_INTERVAL, "Wake up interval", "s";
    /// Consumption counter of a meter.
    consumption: I64 = i64, CONSUMPTION, "Consumption", "";
    /// The consumption counter of the last interval.
    last_consumption: I64 = i64, LAST_CONSUMPTION, "Last consumption count", "";
    /// The number of intervals counted.
    interval_count: I64 = i64, INTERVAL_COUNT, "Interval count", "";
    /// A message counter.
    counter: I64 = i64, COUNTER, "Counter", "";
    /// A sequence number.
    sequence: I64 = i64, SEQUENCE, "Sequence", "";
    /// The repeat number of a transmission.
    repeat: I64 = i64, REPEAT, "Repeat", "";
    /// The time offset of the transmission.
    transmit_offset: I64 = i64, TRANSMIT_OFFSET, "Transmit time offset", "";
    /// A level step without a unit.
    level: I64 = i64, LEVEL, "Level", "";
    /// Volatile organic compounds, as a level step.
    voc_level: I64 = i64, VOC_LEVEL, "VOC level", "";
    /// An alarm code.
    alarm: I64 = i64, ALARM, "Alarm", "";
    /// An alarm type code.
    alarm_type: I64 = i64, ALARM_TYPE, "Alarm type", "";
    /// An alarm level.
    alarm_level: I64 = i64, ALARM_LEVEL, "Alarm level", "";
    /// A status code.
    status: I64 = i64, STATUS, "Status", "";
    /// Status bits.
    flags: I64 = i64, FLAGS, "Flags", "";
    /// An event code.
    event: I64 = i64, EVENT, "Event", "";
    /// A heartbeat code.
    heartbeat: I64 = i64, HEARTBEAT, "Heartbeat", "";
    /// The type of the sensor.
    sensor_type: I64 = i64, SENSOR_TYPE, "Sensor type", "";
    /// The type of the packet.
    packet_type: I64 = i64, PACKET_TYPE, "Packet type", "";
    /// The ERT (encoder receiver transmitter) type of a meter.
    ert_type: I64 = i64, ERT_TYPE, "ERT type", "";
    /// The protocol version of the device.
    version: I64 = i64, VERSION, "Version", "";
    /// The application version of the device.
    app_version: I64 = i64, APP_VERSION, "Application version", "";
    /// Battery bits that a device sends without a known meaning.
    battery_bits: I64 = i64, BATTERY_BITS, "Battery bits", "";
    /// The node that sent a frame of a network.
    source_node: I64 = i64, SOURCE_NODE, "Source node", "";
    /// The length of a frame.
    length_bytes: I64 = i64, LENGTH_BYTES, "Length", "bytes";
    /// A battery was put in.
    new_battery: Bool = bool, NEW_BATTERY, "New battery", "";
    /// The device is learning a code.
    learning: Bool = bool, LEARNING, "Learning", "";
    /// The sender requests an acknowledgement.
    ack_requested: Bool = bool, ACK_REQUESTED, "Ack requested", "";
    /// The device moves.
    moving: Bool = bool, MOVING, "Moving", "";
    /// The case was opened.
    tamper: Bool = bool, TAMPER, "Tamper", "";
    /// The device was tampered with physically.
    physical_tamper: Bool = bool, PHYSICAL_TAMPER, "Physical tamper", "";
    /// The encoder was tampered with.
    encoder_tamper: Bool = bool, ENCODER_TAMPER, "Encoder tamper", "";
    /// A door or window is open.
    door_open: Bool = bool, DOOR_OPEN, "Door open", "";
    /// A switch is on.
    switch_on: Bool = bool, SWITCH_ON, "Switch on", "";
    /// A state as the device names it ("open", "alarm").
    state: Text = &'a str, STATE, "State", "";
    /// The state of a three-state code.
    tri_state: Text = &'a str, TRI_STATE, "Tri-state", "";
    /// An operating mode.
    mode: Text = &'a str, MODE, "Mode", "";
    /// The type of the device or the message.
    r#type: Text = &'a str, TYPE, "Type", "";
    /// A button.
    button: Text = &'a str, BUTTON, "Button", "";
    /// A code that a remote sends.
    code: Text = &'a str, CODE, "Code", "";
    /// A command.
    command: Text = &'a str, COMMAND, "Command", "";
    /// The command class of a home network command.
    command_class: Text = &'a str, COMMAND_CLASS, "Command class", "";
    /// The manufacturer.
    manufacturer: Text = &'a str, MANUFACTURER, "Manufacturer", "";
    /// The id of a home network.
    home_id: Text = &'a str, HOME_ID, "Home ID", "";
    /// The receiver of a frame of a network.
    destination: Text = &'a str, DESTINATION, "Destination", "";
    /// The kind of a frame.
    frame: Text = &'a str, FRAME, "Frame", "";
    /// The C field of a meter bus frame.
    c_field: Text = &'a str, C_FIELD, "C field", "";
}

/// One measurement of a reading: a field of [`measure`] and its value.
#[derive(Clone)]
pub struct Measurement {
    field: &'static dyn Declare,
    value: CoreValue,
    decimals: u8,
}

impl Measurement {
    /// The key of the field.
    pub fn key(&self) -> &'static str {
        self.field.key()
    }
    /// The field.
    pub fn field(&self) -> &'static dyn Declare {
        self.field
    }
    /// The value, of the kind of the field.
    pub fn value(&self) -> &CoreValue {
        &self.value
    }
    /// The value of `m`, if this is a measurement of `m`.
    pub fn get<K: Kind>(&self, m: &Measure<K>) -> Option<K::Out<'_>> {
        if self.field.key() == m.0.key {
            K::load(&self.value)
        } else {
            None
        }
    }
    /// The measurement as people read it: "Temperature 21.4 °C", "Door open yes", "Button A".
    pub fn line(&self) -> String {
        let d = self.field.decl();
        let mut s = d.label.into_owned();
        s.push(' ');
        let _ = match &self.value {
            CoreValue::F64(v) | CoreValue::Duration(v) => {
                write!(s, "{:.*}", usize::from(self.decimals), v)
            }
            CoreValue::I64(v) => write!(s, "{v}"),
            CoreValue::Bool(b) => write!(s, "{}", if *b { "yes" } else { "no" }),
            CoreValue::Text(t) => write!(s, "{t}"),
            _ => Ok(()),
        };
        if !d.unit.is_empty() {
            s.push(' ');
            s.push_str(&d.unit);
        }
        s
    }
}

impl std::fmt::Debug for Measurement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Measurement")
            .field("key", &self.field.key())
            .field("value", &self.value)
            .field("decimals", &self.decimals)
            .finish()
    }
}

/// Two measurements are equal if their fields, values and decimals are equal.
impl PartialEq for Measurement {
    fn eq(&self, o: &Self) -> bool {
        self.field.key() == o.field.key() && self.value == o.value && self.decimals == o.decimals
    }
}

/// The stored form of a [`Measurement`]: the key of its field, its value and its decimals.
#[derive(Deserialize)]
struct StoredMeasurement {
    key: String,
    value: CoreValue,
    decimals: u8,
}

impl Serialize for Measurement {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("StoredMeasurement", 3)?;
        st.serialize_field("key", self.field.key())?;
        st.serialize_field("value", &self.value)?;
        st.serialize_field("decimals", &self.decimals)?;
        st.end()
    }
}

/// A stored measurement reads back if [`measure::ALL`] has its key and its value is of the field's kind.
impl<'de> Deserialize<'de> for Measurement {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let m = StoredMeasurement::deserialize(d)?;
        let Some(field) = measure::ALL.iter().copied().find(|f| f.key() == m.key) else {
            return Err(serde::de::Error::custom(format_args!(
                "no measurement field {}",
                m.key
            )));
        };
        if m.value.kind() != Some(field.kind()) {
            return Err(serde::de::Error::custom(format_args!(
                "measurement field {} holds another kind",
                m.key
            )));
        }
        Ok(Measurement {
            field,
            value: m.value,
            decimals: m.decimals,
        })
    }
}

/// The channel a device sends on: a number, a name, or none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Channel {
    /// The device does not send a channel.
    #[default]
    None,
    /// A channel number.
    Number(i32),
    /// A channel name ("A", "CH3").
    Name(Text),
}

impl std::fmt::Display for Channel {
    /// The number, the name, or "-" for none.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::None => f.write_str("-"),
            Channel::Number(n) => write!(f, "{n}"),
            Channel::Name(s) => f.write_str(s),
        }
    }
}

/// One decoded transmission of one device. A reading leaves a decoder only when a frame test passed. Its measurements are
/// fields of the catalogue in [`measure`].
///
/// ```
/// use sdrkit_proto_ookdev::reading::measure;
/// use sdrkit_proto_ookdev::{Check, Reading};
/// let r = Reading::new("Nexus-TH", "weather", "136", Check::Repeat)
///    .with_channel(1)
///    .with_battery(true)
///    .num(&measure::TEMPERATURE_C, 21.4, 1)
///    .num(&measure::HUMIDITY_PCT, 48.0, 0);
/// assert_eq!(r.line(), "Temperature 21.4 °C, Humidity 48 %");
/// assert_eq!(r.get(&measure::HUMIDITY_PCT), Some(48.0));
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    /// The model ("Nexus-TH").
    pub model: Text,
    /// The device kind in snake_case ("weather", "tpms", "remote", "energy", "meter", "sensor").
    pub kind: Text,
    /// The device id as the protocol shows it.
    pub id: Text,
    /// The channel.
    pub channel: Channel,
    /// The battery state: `Some(true)` for good, `Some(false)` for low, `None` when the device does not send it.
    pub battery_ok: Option<bool>,
    /// The frame test that accepted the frame.
    pub check: Check,
    /// The measurements in the order the decoder added them.
    pub fields: Vec<Measurement>,
    /// The index of the protocol in its [`crate::ProtocolList`]. The list sets it.
    pub protocol: usize,
    /// The frequency of the signal, in Hz from the centre of the stream. The list copies it from the train.
    pub offset_hz: f64,
    /// Signal level, dBFS. The list copies it from the train.
    pub level_dbfs: f32,
    /// Signal-to-noise ratio, dB. The list copies it from the train.
    pub snr_db: f32,
    /// When the transmission started, seconds of the stream. The list copies it from the train.
    pub time_s: f64,
}

/// The event of a reading that leaves the graph, with the fields of [`ReadingFields`].
pub static READING_EVENT: EventDef = EventDef::new(
    "ookdev.reading",
    "Device reading",
    <ReadingFields<'static> as Fields>::SHAPE.fields,
);

/// The event type `ookdev.reading` with its shape, for the description of a stack.
pub fn reading() -> EventShape {
    EventShape::of::<ReadingFields<'static>>(&READING_EVENT)
}

impl sdrkit_graph::Item for Reading {
    const KIND: &'static str = "ookdev.reading";
    fn event_def() -> Option<&'static EventDef> {
        Some(&READING_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        let source = format!("{} {}", self.model, self.id);
        let text = self.line();
        let channel = (self.channel != Channel::None).then(|| self.channel.to_string());
        ReadingFields::of(self, &source, &text, channel.as_deref()).write(r);
    }
}

impl Reading {
    /// A reading with no fields yet.
    pub fn new(
        model: impl Into<Text>,
        kind: impl Into<Text>,
        id: impl Into<Text>,
        check: Check,
    ) -> Reading {
        Reading {
            model: model.into(),
            kind: kind.into(),
            id: id.into(),
            channel: Channel::None,
            battery_ok: None,
            check,
            fields: Vec::new(),
            protocol: 0,
            offset_hz: 0.0,
            level_dbfs: 0.0,
            snr_db: 0.0,
            time_s: 0.0,
        }
    }

    /// With a channel number.
    pub fn with_channel(mut self, channel: i32) -> Self {
        self.channel = Channel::Number(channel);
        self
    }

    /// With a channel name.
    pub fn with_channel_name(mut self, name: impl Into<Text>) -> Self {
        self.channel = Channel::Name(name.into());
        self
    }

    /// With the battery state (true for good).
    pub fn with_battery(mut self, ok: bool) -> Self {
        self.battery_ok = Some(ok);
        self
    }

    /// Adds a number of field `m`, shown with `decimals` decimals.
    pub fn num(mut self, m: &'static Measure<field::F64>, value: f64, decimals: u8) -> Self {
        self.fields.push(Measurement {
            field: m,
            value: CoreValue::F64(value),
            decimals,
        });
        self
    }
    /// Adds the value of field `m`.
    pub fn with<K: Kind>(mut self, m: &'static Measure<K>, value: K::In<'_>) -> Self {
        let mut v = CoreValue::None;
        K::store(&mut v, value);
        self.fields.push(Measurement {
            field: m,
            value: v,
            decimals: 0,
        });
        self
    }
    /// The value of field `m`, if the reading has it.
    pub fn get<K: Kind>(&self, m: &Measure<K>) -> Option<K::Out<'_>> {
        self.fields.iter().find_map(|f| f.get(m))
    }
    /// The measurements as one line, joined by ", ".
    pub fn line(&self) -> String {
        let mut s = String::new();
        for (i, f) in self.fields.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&f.line());
        }
        s
    }
    /// Whether `o` is the same device with the same values. Where and when the signal was received is not compared.
    pub fn same_as(&self, o: &Reading) -> bool {
        self.model == o.model
            && self.id == o.id
            && self.channel == o.channel
            && self.battery_ok == o.battery_ok
            && self.fields == o.fields
    }
}
