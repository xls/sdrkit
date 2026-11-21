/*
 * sdrkit: the typed interface of the shared interface "ookdev", whose items the stacks of several families show, version 1.0. MIT licence.
 *
 * Written from the interface description of the family. Do not edit.
 *
 * Structs. Each struct starts with `size`. A call that fills a struct writes at most the size of the struct that
 * the caller was compiled with, and sets `size` to the number of bytes that it wrote. A struct only grows at its
 * end: a library that is newer than the caller fills the members that the caller knows, and a library that is
 * older leaves the members that it does not know at 0, with `size` below the size of the struct. The caller
 * sets `size` of an arguments struct to its sizeof before the call: use the *_INIT initialiser.
 *
 * Values. A member with a `has_` member before it holds a value only if the `has_` member is true. A string or
 * bytes member that the library writes is a view into a buffer of the handle, valid until the next call on the
 * same handle. A member of an enumeration holds a value of the enum that its comment names.
 *
 * Versions. The minor version rises when the family gets a new stack, table, member, setting, command or event
 * type. The major version rises when one is removed or renamed, or its type or unit changes.
 * sdrkit_family_version() returns the version of the family in the library, or SDRKIT_E_NO_SUCH_KEY if the
 * library was built without the family. sdrkit_object_find() with "stack.<id>" finds out if an engine has a
 * stack.
 *
 * Results. Take a snapshot with sdrkit_snapshot_read() on the handle of a session, then call the *_rows() and
 * *_read() functions on the same handle. Events. Take the events with sdrkit_events_take(), then call the
 * *_event_read() function of the type of each event; it returns SDRKIT_E_WRONG_TYPE for an event of another type.
 * Settings and commands. Call the *_get_*(), *_set_*() and command functions on the handle of a session of the
 * stack; the getters also take the handle of the stack type ("stack.<id>") and return the defaults. One call
 * sets one setting. A function returns SDRKIT_E_WRONG_HANDLE if the object of the handle does not have the
 * table, the fields, the setting or the command with the types of this header.
 */

#ifndef SDRKIT_OOKDEV_H
#define SDRKIT_OOKDEV_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_OOKDEV_VERSION_MAJOR 1
#define SDRKIT_OOKDEV_VERSION_MINOR 0

// The keys of the event types.
#define SDRKIT_EVENT_OOKDEV_READING "ookdev.reading"

// The count of `ookdev.pulses`.
typedef struct sdrkit_ookdev_pulses_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Pulse trains made.
  int64_t trains;
} sdrkit_ookdev_pulses_fields;

// The counts of `ookdev.burst_pulses`.
typedef struct sdrkit_ookdev_burst_pulses_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Pulse trains made.
  int64_t trains;
  // Bursts that made no train.
  int64_t dropped;
} sdrkit_ookdev_burst_pulses_fields;

// The counts of `ookdev.decode`, and of a decoder of a stack that offers trains to a protocol list.
typedef struct sdrkit_ookdev_decode_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Readings the protocols made.
  int64_t readings;
  // Trains no protocol decoded.
  int64_t unknown_trains;
} sdrkit_ookdev_decode_fields;

// The options of `family` of `sdrkit_ookdev_reading_event`.
typedef enum sdrkit_ookdev_reading_event_family {
  // Aircraft
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_OOKDEV_READING_EVENT_FAMILY_OTHER = 12,
} sdrkit_ookdev_reading_event_family;

// The options of `check` of `sdrkit_ookdev_reading_event`.
typedef enum sdrkit_ookdev_reading_event_check {
  // CRC
  SDRKIT_OOKDEV_READING_EVENT_CHECK_CRC = 0,
  // Checksum
  SDRKIT_OOKDEV_READING_EVENT_CHECK_CHECKSUM = 1,
  // Parity
  SDRKIT_OOKDEV_READING_EVENT_CHECK_PARITY = 2,
  // Repeat
  SDRKIT_OOKDEV_READING_EVENT_CHECK_REPEAT = 3,
  // Manchester
  SDRKIT_OOKDEV_READING_EVENT_CHECK_MANCHESTER = 4,
} sdrkit_ookdev_reading_event_check;

// The fields of an `ookdev.reading` event. `source` is the model and the id, `text` the measurements as [`Reading::line`] returns them. A measurement that the device does not give has no value.
typedef struct sdrkit_ookdev_reading_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The model and the id.
  sdrkit_string source;
  // The measurements as one line.
  sdrkit_string text;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel, if the device sends one.
  sdrkit_string channel;
  // The family of the event. A value of `sdrkit_ookdev_reading_event_family`.
  int32_t family;
  // The model of the device.
  sdrkit_string model;
  // The device kind.
  sdrkit_string device_kind;
  // The device id as the protocol shows it.
  sdrkit_string id;
  // True if `battery_ok` holds a value.
  bool has_battery_ok;
  // The battery state: yes for good, no for low.
  bool battery_ok;
  // The frame test that accepted the frame. A value of `sdrkit_ookdev_reading_event_check`.
  int32_t check;
  // The frequency of the signal from the centre of the stream. A frequency in Hz.
  double offset_hz;
  // The signal level. Unit: dBFS.
  double level_dbfs;
  // The signal-to-noise ratio. Unit: dB.
  double snr_db;
  // True if `temperature_c` holds a value.
  bool has_temperature_c;
  // Air temperature. Unit: °C.
  double temperature_c;
  // True if `temperature_1_c` holds a value.
  bool has_temperature_1_c;
  // The temperature of the first probe. Unit: °C.
  double temperature_1_c;
  // True if `temperature_2_c` holds a value.
  bool has_temperature_2_c;
  // The temperature of the second probe. Unit: °C.
  double temperature_2_c;
  // True if `globe_temperature_c` holds a value.
  bool has_globe_temperature_c;
  // Black globe temperature. Unit: °C.
  double globe_temperature_c;
  // True if `humidity_pct` holds a value.
  bool has_humidity_pct;
  // Relative humidity. Unit: %.
  double humidity_pct;
  // True if `absolute_humidity_g_m3` holds a value.
  bool has_absolute_humidity_g_m3;
  // Absolute humidity: grams of water in a cubic metre of air. Unit: g/m³.
  double absolute_humidity_g_m3;
  // True if `moisture_pct` holds a value.
  bool has_moisture_pct;
  // Soil moisture. Unit: %.
  double moisture_pct;
  // True if `pressure_kpa` holds a value.
  bool has_pressure_kpa;
  // Pressure (air or tyre). A decoder that receives inches of mercury multiplies them by [`KPA_PER_INHG`](crate::reading::measure::KPA_PER_INHG). Unit: kPa.
  double pressure_kpa;
  // True if `wind_speed_m_s` holds a value.
  bool has_wind_speed_m_s;
  // Mean wind speed. Unit: m/s.
  double wind_speed_m_s;
  // True if `wind_gust_m_s` holds a value.
  bool has_wind_gust_m_s;
  // Wind gust speed. Unit: m/s.
  double wind_gust_m_s;
  // True if `wind_dir_deg` holds a value.
  bool has_wind_dir_deg;
  // Wind direction, degrees clockwise from north. Unit: °.
  double wind_dir_deg;
  // True if `rain_mm` holds a value.
  bool has_rain_mm;
  // Rain since the device started counting. Unit: mm.
  double rain_mm;
  // True if `uv_index` holds a value.
  bool has_uv_index;
  // UV index.
  double uv_index;
  // True if `light_lux` holds a value.
  bool has_light_lux;
  // Light. Unit: lux.
  double light_lux;
  // True if `battery_level_pct` holds a value.
  bool has_battery_level_pct;
  // Battery charge. Unit: %.
  double battery_level_pct;
  // True if `level_pct` holds a value.
  bool has_level_pct;
  // A fill or dimmer level. Unit: %.
  double level_pct;
  // True if `power_0_w` holds a value.
  bool has_power_0_w;
  // Power of the first channel. Unit: W.
  double power_0_w;
  // True if `power_1_w` holds a value.
  bool has_power_1_w;
  // Power of the second channel. Unit: W.
  double power_1_w;
  // True if `power_2_w` holds a value.
  bool has_power_2_w;
  // Power of the third channel. Unit: W.
  double power_2_w;
  // True if `power_w` holds a value.
  bool has_power_w;
  // Power. A decoder that receives British thermal units (IT) per hour multiplies them by [`W_PER_BTU_IT_H`](crate::reading::measure::W_PER_BTU_IT_H). Unit: W.
  double power_w;
  // True if `current_a` holds a value.
  bool has_current_a;
  // Current. Unit: A.
  double current_a;
  // True if `voltage_v` holds a value.
  bool has_voltage_v;
  // Voltage. Unit: V.
  double voltage_v;
  // True if `energy_kwh` holds a value.
  bool has_energy_kwh;
  // Energy that a meter counted. Unit: kWh.
  double energy_kwh;
  // True if `gas_volume_m3` holds a value.
  bool has_gas_volume_m3;
  // Gas volume that a meter counted. Unit: m³.
  double gas_volume_m3;
  // True if `water_volume_m3` holds a value.
  bool has_water_volume_m3;
  // Water volume that a meter counted. Unit: m³.
  double water_volume_m3;
  // True if `luminance_pct` holds a value.
  bool has_luminance_pct;
  // Luminance as a share of the full scale of the sensor. Unit: %.
  double luminance_pct;
  // True if `co2_ppm` holds a value.
  bool has_co2_ppm;
  // Carbon dioxide. Unit: ppm.
  double co2_ppm;
  // True if `hcho_ppb` holds a value.
  bool has_hcho_ppb;
  // Formaldehyde. Unit: ppb.
  double hcho_ppb;
  // True if `pm2_5_ug_m3` holds a value.
  bool has_pm2_5_ug_m3;
  // Particles up to 2.5 µm. Unit: µg/m³.
  double pm2_5_ug_m3;
  // True if `pm10_ug_m3` holds a value.
  bool has_pm10_ug_m3;
  // Particles up to 10 µm. Unit: µg/m³.
  double pm10_ug_m3;
  // True if `value` holds a value.
  bool has_value;
  // A meter reading or a sensor value without a unit.
  double value;
  // True if `interval` holds a value.
  bool has_interval;
  // The time between two reports. A length of time in seconds.
  double interval;
  // True if `wake_up_interval` holds a value.
  bool has_wake_up_interval;
  // The time between two wake-ups. A length of time in seconds.
  double wake_up_interval;
  // True if `consumption` holds a value.
  bool has_consumption;
  // Consumption counter of a meter.
  int64_t consumption;
  // True if `last_consumption` holds a value.
  bool has_last_consumption;
  // The consumption counter of the last interval.
  int64_t last_consumption;
  // True if `interval_count` holds a value.
  bool has_interval_count;
  // The number of intervals counted.
  int64_t interval_count;
  // True if `counter` holds a value.
  bool has_counter;
  // A message counter.
  int64_t counter;
  // True if `sequence` holds a value.
  bool has_sequence;
  // A sequence number.
  int64_t sequence;
  // True if `repeat` holds a value.
  bool has_repeat;
  // The repeat number of a transmission.
  int64_t repeat;
  // True if `transmit_offset` holds a value.
  bool has_transmit_offset;
  // The time offset of the transmission.
  int64_t transmit_offset;
  // True if `level` holds a value.
  bool has_level;
  // A level step without a unit.
  int64_t level;
  // True if `voc_level` holds a value.
  bool has_voc_level;
  // Volatile organic compounds, as a level step.
  int64_t voc_level;
  // True if `alarm` holds a value.
  bool has_alarm;
  // An alarm code.
  int64_t alarm;
  // True if `alarm_type` holds a value.
  bool has_alarm_type;
  // An alarm type code.
  int64_t alarm_type;
  // True if `alarm_level` holds a value.
  bool has_alarm_level;
  // An alarm level.
  int64_t alarm_level;
  // True if `status` holds a value.
  bool has_status;
  // A status code.
  int64_t status;
  // True if `flags` holds a value.
  bool has_flags;
  // Status bits.
  int64_t flags;
  // True if `event` holds a value.
  bool has_event;
  // An event code.
  int64_t event;
  // True if `heartbeat` holds a value.
  bool has_heartbeat;
  // A heartbeat code.
  int64_t heartbeat;
  // True if `sensor_type` holds a value.
  bool has_sensor_type;
  // The type of the sensor.
  int64_t sensor_type;
  // True if `packet_type` holds a value.
  bool has_packet_type;
  // The type of the packet.
  int64_t packet_type;
  // True if `ert_type` holds a value.
  bool has_ert_type;
  // The ERT (encoder receiver transmitter) type of a meter.
  int64_t ert_type;
  // True if `version` holds a value.
  bool has_version;
  // The protocol version of the device.
  int64_t version;
  // True if `app_version` holds a value.
  bool has_app_version;
  // The application version of the device.
  int64_t app_version;
  // True if `battery_bits` holds a value.
  bool has_battery_bits;
  // Battery bits that a device sends without a known meaning.
  int64_t battery_bits;
  // True if `source_node` holds a value.
  bool has_source_node;
  // The node that sent a frame of a network.
  int64_t source_node;
  // True if `length_bytes` holds a value.
  bool has_length_bytes;
  // The length of a frame. Unit: bytes.
  int64_t length_bytes;
  // True if `new_battery` holds a value.
  bool has_new_battery;
  // A battery was put in.
  bool new_battery;
  // True if `learning` holds a value.
  bool has_learning;
  // The device is learning a code.
  bool learning;
  // True if `ack_requested` holds a value.
  bool has_ack_requested;
  // The sender requests an acknowledgement.
  bool ack_requested;
  // True if `moving` holds a value.
  bool has_moving;
  // The device moves.
  bool moving;
  // True if `tamper` holds a value.
  bool has_tamper;
  // The case was opened.
  bool tamper;
  // True if `physical_tamper` holds a value.
  bool has_physical_tamper;
  // The device was tampered with physically.
  bool physical_tamper;
  // True if `encoder_tamper` holds a value.
  bool has_encoder_tamper;
  // The encoder was tampered with.
  bool encoder_tamper;
  // True if `door_open` holds a value.
  bool has_door_open;
  // A door or window is open.
  bool door_open;
  // True if `switch_on` holds a value.
  bool has_switch_on;
  // A switch is on.
  bool switch_on;
  // True if `state` holds a value.
  bool has_state;
  // A state as the device names it ("open", "alarm").
  sdrkit_string state;
  // True if `tri_state` holds a value.
  bool has_tri_state;
  // The state of a three-state code.
  sdrkit_string tri_state;
  // True if `mode` holds a value.
  bool has_mode;
  // An operating mode.
  sdrkit_string mode;
  // True if `type` holds a value.
  bool has_type;
  // The type of the device or the message.
  sdrkit_string type;
  // True if `button` holds a value.
  bool has_button;
  // A button.
  sdrkit_string button;
  // True if `code` holds a value.
  bool has_code;
  // A code that a remote sends.
  sdrkit_string code;
  // True if `command` holds a value.
  bool has_command;
  // A command.
  sdrkit_string command;
  // True if `command_class` holds a value.
  bool has_command_class;
  // The command class of a home network command.
  sdrkit_string command_class;
  // True if `manufacturer` holds a value.
  bool has_manufacturer;
  // The manufacturer.
  sdrkit_string manufacturer;
  // True if `home_id` holds a value.
  bool has_home_id;
  // The id of a home network.
  sdrkit_string home_id;
  // True if `destination` holds a value.
  bool has_destination;
  // The receiver of a frame of a network.
  sdrkit_string destination;
  // True if `frame` holds a value.
  bool has_frame;
  // The kind of a frame.
  sdrkit_string frame;
  // True if `c_field` holds a value.
  bool has_c_field;
  // The C field of a meter bus frame.
  sdrkit_string c_field;
} sdrkit_ookdev_reading_event;

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ookdev.reading"
// (Device reading). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ookdev_reading_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ookdev_reading_event *event) {
  return sdrkit_typed_event(handle, n, "ookdev.reading", "ookdev.ReadingFields", event, sizeof(sdrkit_ookdev_reading_event));
}

// Fills `fields` with the result fields of the component "pulses" of the stack "ookdev" in the snapshot of
// `handle`. A generic consumer reads them under the keys "pulses.<key>".
static inline int32_t sdrkit_ookdev_pulses_fields_read(SDRKIT_HANDLE handle, sdrkit_ookdev_pulses_fields *fields) {
  return sdrkit_typed_fields(handle, "ookdev.PulsesFields@pulses", fields, sizeof(sdrkit_ookdev_pulses_fields));
}

// Fills `fields` with the result fields of the component "burst_pulses" of the stack "ookdev" in the snapshot of
// `handle`. A generic consumer reads them under the keys "burst_pulses.<key>".
static inline int32_t sdrkit_ookdev_burst_pulses_fields_read(SDRKIT_HANDLE handle, sdrkit_ookdev_burst_pulses_fields *fields) {
  return sdrkit_typed_fields(handle, "ookdev.BurstPulsesFields@burst_pulses", fields, sizeof(sdrkit_ookdev_burst_pulses_fields));
}

// Fills `fields` with the result fields of the component "decode" of the stack "ookdev" in the snapshot of
// `handle`. A generic consumer reads them under the keys "decode.<key>".
static inline int32_t sdrkit_ookdev_decode_fields_read(SDRKIT_HANDLE handle, sdrkit_ookdev_decode_fields *fields) {
  return sdrkit_typed_fields(handle, "ookdev.DecodeFields@decode", fields, sizeof(sdrkit_ookdev_decode_fields));
}

#endif
