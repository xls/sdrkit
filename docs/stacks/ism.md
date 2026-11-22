<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ISM sensors

Weather stations, thermometers, tyre pressure sensors, door and motion sensors, remotes and meters on 315 to 915 MHz.

Stack id: `ism`. Group: Sensors, 433 / 868 MHz.

ISM sensors : every burst in the input band is found, channelised, cut into pulse trains and offered to every device protocol. The Sigfox chain runs beside it on a Sigfox band. Each reading leaves as an `ookdev.reading` event.

Signal types: Narrow digital.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 433.92 MHz |
| Tuning range | 300 MHz to 930 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 1.6 MHz |
| Offset from the signal | 250 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ism` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold` | Detection threshold | number (dB) | 13 | 8 to 25 | How far above the noise floor of a frequency bin a burst must rise. |
| `submode` | Submode | choice | `devices` | `devices` (Device decoders), `pulse_analyser` (OOK / ASK pulse analyser) | The pulse analyser measures the frames of an unknown on-off keyed signal and guesses their line code; it decodes no device. |
| `frame_gap_ms` | Longest gap inside a frame | number (ms) | 5 | 1 to 50 | Pulse analyser: a longer gap ends a frame. After 4 pulses the limit is 12 times the median width (at least 1.2 ms), if that is shorter. |
| `min_pulses` | Fewest pulses in a frame | whole number | 8 | 4 to 64 | Pulse analyser: a frame with fewer pulses is not analysed. |
| `cluster_ratio` | Cluster split ratio | number | 1.5 | 1.2 to 2.5 | Pulse analyser: sorted widths or gaps start a new cluster where a value is this many times the one before. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `0` | `0` (433.92 MHz sensors), `1` (868.3 MHz sensors), `2` (Z-Wave) |  |
| `gen_carrier` | Strong continuous carrier | yes or no | false |  | A carrier 20 dB above the devices, 300 kHz above the centre. |
| `gen_pattern` | Pulse analyser test pattern | choice | `pwm` | `pwm` (PWM (PT2262 style)), `ppm` (PPM), `manchester` (Manchester) |  |
| `gen_bits` | Pulse analyser test bits | text | empty |  | 0 and 1; other characters are left out. Empty: 101100101001110010100110 (PWM), 1100101001101011 (PPM), 10110010111000101101 (Manchester). |
| `gen_t_us` | Pulse analyser test unit | number (µs) | 0 | 0 to 5000 |  |
| `gen_pulse_us` | Pulse analyser test PPM pulse | number (µs) | 300 | 20 to 5000 |  |
| `gen_gap0_us` | Pulse analyser test PPM gap of a 0 | number (µs) | 500 | 20 to 5000 |  |
| `gen_gap1_us` | Pulse analyser test PPM gap of a 1 | number (µs) | 1000 | 20 to 5000 |  |
| `gen_jitter` | Pulse analyser test jitter | number | 0 | 0 to 0.2 | Each mark and space is stretched by a uniform share up to this, either way. |
| `gen_frames` | Pulse analyser test frames | whole number | 4 | 1 to 50 |  |
| `gen_frame_gap_s` | Pulse analyser test gap after a frame | number (s) | 0 | 0 to 1 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `bursts_detected` | Bursts detected | whole number |
| `noise_floor_dbfs` | Noise floor | number (dBFS) |
| `input_level_dbfs` | Input level | number (dBFS) |
| `readings` | Readings | whole number |
| `unknown_trains` | Unknown trains | whole number |
| `carriers` | Sigfox carriers | whole number |
| `bad_samples` | Bad samples | whole number |
| `devices` | Devices | whole number |
| `packets_decoded` | Packets decoded | whole number |
| `unknown_bursts` | Unknown bursts | whole number |
| `sigfox_messages` | Sigfox messages | whole number |
| `line_code` | Line code | choice |
| `bits` | Bits | whole number |
| `bits_bin` | Bits (binary) | text |
| `bits_hex` | Bits (hex) | text |
| `short_us` | Short | number (µs) |
| `long_us` | Long | number (µs) |
| `pulse_us` | Pulse | number (µs) |
| `period_us` | Bit period | number (µs) |
| `half_bit_us` | Half-bit | number (µs) |
| `unit_us` | Unit | number (µs) |
| `bit_rate` | Bit rate | number (bit/s) |
| `pulses` | Pulses | whole number |
| `frame_ms` | Frame length | number (ms) |
| `repeats` | Repeats | whole number |
| `frames_seen` | Frames seen | whole number |
| `frames_good` | Frames with a line code | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Frames sent | whole number |
| `tx_waiting` | Frames waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `devices` | Devices | `model` (text), `kind` (text), `id` (text), `channel` (text), `battery_ok` (yes or no), `readings` (text), `history` (text), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `last_heard` (time), `count` (whole number), `frequency` (frequency), `check` (choice) |
| `zwave` | Z-Wave | `home_id` (text), `node` (whole number), `rate` (text), `last_heard` (time), `level_dbfs` (number (dBFS)), `count` (whole number), `command` (text), `value` (text) |
| `log` | Log | `time_s` (number (s)), `model` (text), `id` (text), `frequency` (frequency), `level_dbfs` (number (dBFS)), `text` (text) |
| `unknown` | Unknown signals | `time_s` (number (s)), `frequency` (frequency), `modulation` (text), `line_code` (text), `pulses` (whole number), `short_us` (number (µs)), `long_us` (number (µs)), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `copy` (text) |
| `sigfox` | Sigfox | `time_s` (number (s)), `device_id` (text), `seq` (whole number), `payload` (text), `frequency` (frequency), `level_dbfs` (number (dBFS)), `reps` (whole number), `mac_crc` (text) |
| `pulse_frames` | Pulse frames | `time_s` (number (s)), `frequency` (frequency), `line_code` (choice), `bits` (whole number), `bits_bin` (text), `bits_hex` (text), `short_us` (number (µs)), `long_us` (number (µs)), `pulse_us` (number (µs)), `period_us` (number (µs)), `half_bit_us` (number (µs)), `unit_us` (number (µs)), `bit_rate` (number (bit/s)), `pulses` (whole number), `frame_ms` (number (ms)), `repeats` (whole number), `level_dbfs` (number (dBFS)) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `sfx_finder.waterfall` | Waterfall | column | level |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ism.device` | ISM device | `source` (text), `text` (text), `family` (choice), `model` (text), `id` (text) |
| `sigfox.message` | Sigfox message | `source` (text), `text` (text), `family` (choice), `device_id` (text), `seq` (whole number), `payload` (text) |
| `ookdev.reading` | Device reading | `source` (text), `text` (text), `channel` (text), `family` (choice), `model` (text), `device_kind` (text), `id` (text), `battery_ok` (yes or no), `check` (choice), `offset_hz` (frequency), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `temperature_c` (number (°C)), `temperature_1_c` (number (°C)), `temperature_2_c` (number (°C)), `globe_temperature_c` (number (°C)), `humidity_pct` (number (%)), `absolute_humidity_g_m3` (number (g/m³)), `moisture_pct` (number (%)), `pressure_kpa` (number (kPa)), `wind_speed_m_s` (number (m/s)), `wind_gust_m_s` (number (m/s)), `wind_dir_deg` (number (°)), `rain_mm` (number (mm)), `uv_index` (number), `light_lux` (number (lux)), `battery_level_pct` (number (%)), `level_pct` (number (%)), `power_0_w` (number (W)), `power_1_w` (number (W)), `power_2_w` (number (W)), `power_w` (number (W)), `current_a` (number (A)), `voltage_v` (number (V)), `energy_kwh` (number (kWh)), `gas_volume_m3` (number (m³)), `water_volume_m3` (number (m³)), `luminance_pct` (number (%)), `co2_ppm` (number (ppm)), `hcho_ppb` (number (ppb)), `pm2_5_ug_m3` (number (µg/m³)), `pm10_ug_m3` (number (µg/m³)), `value` (number), `interval` (duration (s)), `wake_up_interval` (duration (s)), `consumption` (whole number), `last_consumption` (whole number), `interval_count` (whole number), `counter` (whole number), `sequence` (whole number), `repeat` (whole number), `transmit_offset` (whole number), `level` (whole number), `voc_level` (whole number), `alarm` (whole number), `alarm_type` (whole number), `alarm_level` (whole number), `status` (whole number), `flags` (whole number), `event` (whole number), `heartbeat` (whole number), `sensor_type` (whole number), `packet_type` (whole number), `ert_type` (whole number), `version` (whole number), `app_version` (whole number), `battery_bits` (whole number), `source_node` (whole number), `length_bytes` (whole number (bytes)), `new_battery` (yes or no), `learning` (yes or no), `ack_requested` (yes or no), `moving` (yes or no), `tamper` (yes or no), `physical_tamper` (yes or no), `encoder_tamper` (yes or no), `door_open` (yes or no), `switch_on` (yes or no), `state` (text), `tri_state` (text), `mode` (text), `type` (text), `button` (text), `code` (text), `command` (text), `command_class` (text), `manufacturer` (text), `home_id` (text), `destination` (text), `frame` (text), `c_field` (text) |
| `ism.pulse_frame` | Pulse frame | `text` (text), `family` (choice), `line_code` (choice), `bits` (whole number), `bits_hex` (text), `repeats` (whole number), `level_dbfs` (number (dBFS)) |
| `ism.tx` | ISM transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send a frame | `family` (choice, default `remote`), `id` (whole number, required), `command` (whole number, default 1), `channel` (whole number, default 1), `battery_ok` (yes or no, default true), `temperature` (number (°C), default 20), `humidity` (whole number (%), default 50), `wind_speed` (number (m/s), default 0), `wind_gust` (number (m/s), default 0), `wind_dir` (number (°), default 0), `rain` (number (mm), default 0), `uv` (number, default 0), `light` (number (lx), default 0), `pressure` (number (kPa), default 0), `flags` (whole number, default 0), `current` (number (A), default 0), `interval` (whole number (s), default 6), `power` (whole number (W), default 0), `payload` (text, default empty), `temperature_2` (number (°C), default 20), `meter_type` (whole number, default 0), `consumption` (whole number, default 0), `moisture` (whole number (%), default 0) |  | Sends one transmission of the device family (its copies of the frame, as the device sends them) on the tuned frequency, as OOK or FSK by the family. Transmissions sent during one wait for it. Transmit must be on. |
| `halt` | Halt |  |  | Drops the frames waiting and ends the frame on air with the fall of the carrier. |

## Typed interface

The stack is in the family `ism`, version 3.1. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `IsmDetectFields`. The result fields of the burst finder of the ISM sensor stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bursts_detected` | whole number | always | 1.0 | Bursts found in the band. |
| `noise_floor_dbfs` | number (dBFS) | optional | 1.0 | The mean noise floor of a frequency bin, once the finder has frames. |
| `input_level_dbfs` | number (dBFS) | optional | 1.0 | The mean input level since the last report, if samples came. |

### Result fields

Struct `DecodeFields`. The counts of `ookdev.decode`, and of a decoder of a stack that offers trains to a protocol list.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `readings` | whole number | always | 1.0 | Readings the protocols made. |
| `unknown_trains` | whole number | always | 1.0 | Trains no protocol decoded. |

### Result fields

Struct `SigfoxFinderFields`. The count of the Sigfox carrier finder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carriers` | whole number | always | 1.0 | Narrow carriers passed to the demodulator. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Result fields

Struct `IsmFields`. The result fields of the device table of the ISM sensor stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `devices` | whole number | always | 1.0 | Devices in the table. |
| `packets_decoded` | whole number | always | 1.0 | Distinct transmissions decoded. |
| `unknown_bursts` | whole number | always | 1.0 | Trains no protocol decoded. |
| `sigfox_messages` | whole number | always | 1.0 | Sigfox messages in the list. |

### Result fields

Struct `IsmPulseFields`. The result fields of the pulse analyser (submode `pulse_analyser`): the facts of the last frame with a line code, and the counts. A fact has no value before the first such frame and after a change of frequency, and a timing has a value only for the line codes that have it.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `line_code` | choice | optional | 1.0 | The line code guessed from the pulse timings. |
| `bits` | whole number | optional | 1.0 | Bits read. |
| `bits_bin` | text | optional | 1.0 | The first 96 bits as 0 and 1, with "..." when there are more. |
| `bits_hex` | text | optional | 1.0 | The first 96 bits as hex; the last digit is padded with zero bits. |
| `short_us` | number (µs) | optional | 1.0 | PWM: the short width. PPM: the short gap. |
| `long_us` | number (µs) | optional | 1.0 | PWM: the long width. PPM: the long gap. |
| `pulse_us` | number (µs) | optional | 1.0 | PPM: the pulse width. |
| `period_us` | number (µs) | optional | 1.0 | PWM: the bit period. |
| `half_bit_us` | number (µs) | optional | 1.0 | Manchester: the half-bit. |
| `unit_us` | number (µs) | optional | 1.0 | PCM: the unit. |
| `bit_rate` | number (bit/s) | optional | 1.0 | Manchester and PCM: the bit rate. |
| `pulses` | whole number | optional | 1.0 | The pulses of the frame. |
| `frame_ms` | number (ms) | optional | 1.0 | From the start of the first pulse to the end of the last. |
| `repeats` | whole number | optional | 1.0 | Frames in a row with the same bits, each within 0.5 s of the one before. |
| `frames_seen` | whole number | always | 1.0 | Frames analysed, with a line code or not. |
| `frames_good` | whole number | always | 1.0 | Frames with a line code. |

### Result fields

Struct `IsmTxFields`. The result fields of the sender of a frame stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.0 | Whether a frame is on air. |
| `tx_sent` | whole number | always | 1.0 | The frames sent since the stack started. |
| `tx_waiting` | whole number | always | 1.0 | The frames waiting for the transmitter. |

### Table `devices`

Struct `IsmDeviceRow`. A row of the devices. The row key is the model, the id and the channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `model` | text | always | 1.0 | The device model. |
| `kind` | text | always | 1.0 | The device kind. |
| `id` | text | always | 1.0 | The device id. |
| `channel` | text | optional | 1.0 | The channel, if the device sends one. |
| `battery_ok` | yes or no | optional | 1.0 | The battery state: yes for good, no for low. |
| `readings` | text | always | 1.0 | The fields of the last reading as one line. |
| `history` | text | optional | 1.0 | The history of the first number with a unit, once the device sent one. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last transmission. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the last transmission. |
| `last_heard` | time | optional | 1.0 | The UTC of the last transmission. |
| `count` | whole number | always | 1.0 | Distinct transmissions. |
| `frequency` | frequency | always | 1.0 | The frequency of the last transmission. |
| `check` | choice | always | 1.0 | The frame test of the last reading. |

The column `id` names a row.

### Table `zwave`

Struct `IsmZwaveRow`. A row of the Z-Wave view of the devices. The row key is the key of the device.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `home_id` | text | optional | 1.0 | The home id, if the frame gave one. |
| `node` | whole number | optional | 1.0 | The node, if the frame gave one. |
| `rate` | text | always | 1.0 | The rate. |
| `last_heard` | time | optional | 1.0 | The UTC of the last frame. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the last frame. |
| `count` | whole number | always | 1.0 | Distinct frames. |
| `command` | text | optional | 1.0 | The command, if the frame gave one. |
| `value` | text | optional | 1.0 | The value of the command, if the frame gave one. |

### Table `log`

Struct `IsmLogRow`. A row of the reading log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `model` | text | always | 1.0 | The device model. |
| `id` | text | always | 1.0 | The device id. |
| `frequency` | frequency | always | 1.0 | The frequency. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |
| `text` | text | always | 1.0 | The fields of the reading as one line. |

The column `id` names a row.

### Table `unknown`

Struct `IsmUnknownRow`. A row of the unknown signals: a train no protocol decoded, with its line-code guess.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `frequency` | frequency | always | 1.0 | The frequency. |
| `modulation` | text | always | 1.0 | OOK or FSK. |
| `line_code` | text | always | 1.0 | The guessed line code. |
| `pulses` | whole number | always | 1.0 | Pulses of the train. |
| `short_us` | number (µs) | always | 1.0 | The short width of the guess. |
| `long_us` | number (µs) | always | 1.0 | The long width of the guess. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `copy` | text | always | 1.0 | The pulses as text to copy ("OOK PWM 464 us/1404 us; 464/1404 ..."). |

### Table `sigfox`

Struct `IsmSigfoxRow`. A row of the Sigfox messages heard inside the ISM sensor stack. The row key is the key of the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the first frame. |
| `device_id` | text | always | 1.0 | The device id. |
| `seq` | whole number | always | 1.0 | The message counter. |
| `payload` | text | always | 1.0 | The payload as hex, or the one-bit value. |
| `frequency` | frequency | always | 1.0 | The frequency of the first repetition. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the strongest repetition. |
| `reps` | whole number | always | 1.0 | Repetitions heard. |
| `mac_crc` | text | always | 1.0 | The authentication code, never checked, and the CRC. |

The column `device_id` names a row.

### Table `pulse_frames`

Struct `IsmPulseFrameRow`. A row of the analysed frames with a line code. The row key is the number of the frame in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the first pulse. |
| `frequency` | frequency | always | 1.0 | The frequency. |
| `line_code` | choice | always | 1.0 | The line code guessed from the pulse timings. |
| `bits` | whole number | always | 1.0 | Bits read. |
| `bits_bin` | text | always | 1.0 | The first 96 bits as 0 and 1, with "..." when there are more. |
| `bits_hex` | text | always | 1.0 | The first 96 bits as hex. |
| `short_us` | number (µs) | optional | 1.0 | PWM: the short width. PPM: the short gap. |
| `long_us` | number (µs) | optional | 1.0 | PWM: the long width. PPM: the long gap. |
| `pulse_us` | number (µs) | optional | 1.0 | PPM: the pulse width. |
| `period_us` | number (µs) | optional | 1.0 | PWM: the bit period. |
| `half_bit_us` | number (µs) | optional | 1.0 | Manchester: the half-bit. |
| `unit_us` | number (µs) | optional | 1.0 | PCM: the unit. |
| `bit_rate` | number (bit/s) | optional | 1.0 | Manchester and PCM: the bit rate. |
| `pulses` | whole number | always | 1.0 | The pulses of the frame. |
| `frame_ms` | number (ms) | always | 1.0 | From the start of the first pulse to the end of the last. |
| `repeats` | whole number | always | 1.0 | Frames in a row with the same bits, this one included. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the marks. |

### Event `ism.device`

Struct `IsmDeviceEvent`. The fields of a device heard for the first time. `source` is the model and the id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The model and the id. |
| `text` | text | always | 1.0 | The line of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `model` | text | always | 1.0 | The device model. |
| `id` | text | always | 1.0 | The device id. |

### Event `sigfox.message`

Struct `SigfoxMessageEvent`. The fields of a new message event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The device id. |
| `text` | text | always | 1.0 | The log line of the message. |
| `family` | choice | always | 1.0 | The family of the event. |
| `device_id` | text | always | 1.0 | The device id. |
| `seq` | whole number | always | 1.0 | The message counter. |
| `payload` | text | always | 1.0 | The payload as hex. |

### Event `ookdev.reading`

Struct `ReadingFields`. The fields of an `ookdev.reading` event. `source` is the model and the id, `text` the measurements as [`Reading::line`] returns them. A measurement that the device does not give has no value.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The model and the id. |
| `text` | text | always | 1.0 | The measurements as one line. |
| `channel` | text | optional | 1.0 | The channel, if the device sends one. |
| `family` | choice | always | 1.0 | The family of the event. |
| `model` | text | always | 1.0 | The model of the device. |
| `device_kind` | text | always | 1.0 | The device kind. |
| `id` | text | always | 1.0 | The device id as the protocol shows it. |
| `battery_ok` | yes or no | optional | 1.0 | The battery state: yes for good, no for low. |
| `check` | choice | always | 1.0 | The frame test that accepted the frame. |
| `offset_hz` | frequency | always | 1.0 | The frequency of the signal from the centre of the stream. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The signal level. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio. |
| `temperature_c` | number (°C) | optional | 1.0 | Air temperature. |
| `temperature_1_c` | number (°C) | optional | 1.0 | The temperature of the first probe. |
| `temperature_2_c` | number (°C) | optional | 1.0 | The temperature of the second probe. |
| `globe_temperature_c` | number (°C) | optional | 1.0 | Black globe temperature. |
| `humidity_pct` | number (%) | optional | 1.0 | Relative humidity. |
| `absolute_humidity_g_m3` | number (g/m³) | optional | 1.0 | Absolute humidity: grams of water in a cubic metre of air. |
| `moisture_pct` | number (%) | optional | 1.0 | Soil moisture. |
| `pressure_kpa` | number (kPa) | optional | 1.0 | Pressure (air or tyre). A decoder that receives inches of mercury multiplies them by [`KPA_PER_INHG`](crate::reading::measure::KPA_PER_INHG). |
| `wind_speed_m_s` | number (m/s) | optional | 1.0 | Mean wind speed. |
| `wind_gust_m_s` | number (m/s) | optional | 1.0 | Wind gust speed. |
| `wind_dir_deg` | number (°) | optional | 1.0 | Wind direction, degrees clockwise from north. |
| `rain_mm` | number (mm) | optional | 1.0 | Rain since the device started counting. |
| `uv_index` | number | optional | 1.0 | UV index. |
| `light_lux` | number (lux) | optional | 1.0 | Light. |
| `battery_level_pct` | number (%) | optional | 1.0 | Battery charge. |
| `level_pct` | number (%) | optional | 1.0 | A fill or dimmer level. |
| `power_0_w` | number (W) | optional | 1.0 | Power of the first channel. |
| `power_1_w` | number (W) | optional | 1.0 | Power of the second channel. |
| `power_2_w` | number (W) | optional | 1.0 | Power of the third channel. |
| `power_w` | number (W) | optional | 1.0 | Power. A decoder that receives British thermal units (IT) per hour multiplies them by [`W_PER_BTU_IT_H`](crate::reading::measure::W_PER_BTU_IT_H). |
| `current_a` | number (A) | optional | 1.0 | Current. |
| `voltage_v` | number (V) | optional | 1.0 | Voltage. |
| `energy_kwh` | number (kWh) | optional | 1.0 | Energy that a meter counted. |
| `gas_volume_m3` | number (m³) | optional | 1.0 | Gas volume that a meter counted. |
| `water_volume_m3` | number (m³) | optional | 1.0 | Water volume that a meter counted. |
| `luminance_pct` | number (%) | optional | 1.0 | Luminance as a share of the full scale of the sensor. |
| `co2_ppm` | number (ppm) | optional | 1.0 | Carbon dioxide. |
| `hcho_ppb` | number (ppb) | optional | 1.0 | Formaldehyde. |
| `pm2_5_ug_m3` | number (µg/m³) | optional | 1.0 | Particles up to 2.5 µm. |
| `pm10_ug_m3` | number (µg/m³) | optional | 1.0 | Particles up to 10 µm. |
| `value` | number | optional | 1.0 | A meter reading or a sensor value without a unit. |
| `interval` | duration (s) | optional | 1.0 | The time between two reports. |
| `wake_up_interval` | duration (s) | optional | 1.0 | The time between two wake-ups. |
| `consumption` | whole number | optional | 1.0 | Consumption counter of a meter. |
| `last_consumption` | whole number | optional | 1.0 | The consumption counter of the last interval. |
| `interval_count` | whole number | optional | 1.0 | The number of intervals counted. |
| `counter` | whole number | optional | 1.0 | A message counter. |
| `sequence` | whole number | optional | 1.0 | A sequence number. |
| `repeat` | whole number | optional | 1.0 | The repeat number of a transmission. |
| `transmit_offset` | whole number | optional | 1.0 | The time offset of the transmission. |
| `level` | whole number | optional | 1.0 | A level step without a unit. |
| `voc_level` | whole number | optional | 1.0 | Volatile organic compounds, as a level step. |
| `alarm` | whole number | optional | 1.0 | An alarm code. |
| `alarm_type` | whole number | optional | 1.0 | An alarm type code. |
| `alarm_level` | whole number | optional | 1.0 | An alarm level. |
| `status` | whole number | optional | 1.0 | A status code. |
| `flags` | whole number | optional | 1.0 | Status bits. |
| `event` | whole number | optional | 1.0 | An event code. |
| `heartbeat` | whole number | optional | 1.0 | A heartbeat code. |
| `sensor_type` | whole number | optional | 1.0 | The type of the sensor. |
| `packet_type` | whole number | optional | 1.0 | The type of the packet. |
| `ert_type` | whole number | optional | 1.0 | The ERT (encoder receiver transmitter) type of a meter. |
| `version` | whole number | optional | 1.0 | The protocol version of the device. |
| `app_version` | whole number | optional | 1.0 | The application version of the device. |
| `battery_bits` | whole number | optional | 1.0 | Battery bits that a device sends without a known meaning. |
| `source_node` | whole number | optional | 1.0 | The node that sent a frame of a network. |
| `length_bytes` | whole number (bytes) | optional | 1.0 | The length of a frame. |
| `new_battery` | yes or no | optional | 1.0 | A battery was put in. |
| `learning` | yes or no | optional | 1.0 | The device is learning a code. |
| `ack_requested` | yes or no | optional | 1.0 | The sender requests an acknowledgement. |
| `moving` | yes or no | optional | 1.0 | The device moves. |
| `tamper` | yes or no | optional | 1.0 | The case was opened. |
| `physical_tamper` | yes or no | optional | 1.0 | The device was tampered with physically. |
| `encoder_tamper` | yes or no | optional | 1.0 | The encoder was tampered with. |
| `door_open` | yes or no | optional | 1.0 | A door or window is open. |
| `switch_on` | yes or no | optional | 1.0 | A switch is on. |
| `state` | text | optional | 1.0 | A state as the device names it ("open", "alarm"). |
| `tri_state` | text | optional | 1.0 | The state of a three-state code. |
| `mode` | text | optional | 1.0 | An operating mode. |
| `type` | text | optional | 1.0 | The type of the device or the message. |
| `button` | text | optional | 1.0 | A button. |
| `code` | text | optional | 1.0 | A code that a remote sends. |
| `command` | text | optional | 1.0 | A command. |
| `command_class` | text | optional | 1.0 | The command class of a home network command. |
| `manufacturer` | text | optional | 1.0 | The manufacturer. |
| `home_id` | text | optional | 1.0 | The id of a home network. |
| `destination` | text | optional | 1.0 | The receiver of a frame of a network. |
| `frame` | text | optional | 1.0 | The kind of a frame. |
| `c_field` | text | optional | 1.0 | The C field of a meter bus frame. |

### Event `ism.pulse_frame`

Struct `IsmPulseFrameEvent`. The fields of an analysed frame with a line code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The line of the event. |
| `family` | choice | always | 1.0 | The family of the event. |
| `line_code` | choice | always | 1.0 | The line code guessed from the pulse timings. |
| `bits` | whole number | always | 1.0 | Bits read. |
| `bits_hex` | text | always | 1.0 | The first 96 bits as hex. |
| `repeats` | whole number | always | 1.0 | Frames in a row with the same bits, this one included. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the marks. |

### Event `ism.tx`

Struct `IsmTxEvent`. The fields of a change of a transmission of a frame stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.0 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.0 | Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused. |
| `target` | text | always | 1.0 | The addressee or the identity of the frame. |
| `text` | text | always | 1.0 | What the frame carries, for started and queued; empty otherwise. |
| `family` | choice | always | 1.0 | The family of the source: a sensor. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ism_clear` | none | none |
| `remove` | `sdrkit_ism_remove` | `sdrkit_ism_pan_device_args` | none |
| `send` | `sdrkit_ism_send` | `sdrkit_ism_send_args` | none |
| `halt` | `sdrkit_ism_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold` | `sdrkit_ism_get_threshold` | `sdrkit_ism_set_threshold` |
| `submode` | `sdrkit_ism_get_submode` | `sdrkit_ism_set_submode` |
| `frame_gap_ms` | `sdrkit_ism_get_frame_gap_ms` | `sdrkit_ism_set_frame_gap_ms` |
| `min_pulses` | `sdrkit_ism_get_min_pulses` | `sdrkit_ism_set_min_pulses` |
| `cluster_ratio` | `sdrkit_ism_get_cluster_ratio` | `sdrkit_ism_set_cluster_ratio` |
| `tx_level` | `sdrkit_ism_get_tx_level` | `sdrkit_ism_set_tx_level` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_ism_gen_get_scene` | `sdrkit_ism_gen_set_scene` |
| `gen_carrier` | `sdrkit_ism_gen_get_carrier` | `sdrkit_ism_gen_set_carrier` |
| `gen_pattern` | `sdrkit_ism_gen_get_pattern` | `sdrkit_ism_gen_set_pattern` |
| `gen_bits` | `sdrkit_ism_gen_get_bits` | `sdrkit_ism_gen_set_bits` |
| `gen_t_us` | `sdrkit_ism_gen_get_t_us` | `sdrkit_ism_gen_set_t_us` |
| `gen_pulse_us` | `sdrkit_ism_gen_get_pulse_us` | `sdrkit_ism_gen_set_pulse_us` |
| `gen_gap0_us` | `sdrkit_ism_gen_get_gap0_us` | `sdrkit_ism_gen_set_gap0_us` |
| `gen_gap1_us` | `sdrkit_ism_gen_get_gap1_us` | `sdrkit_ism_gen_set_gap1_us` |
| `gen_jitter` | `sdrkit_ism_gen_get_jitter` | `sdrkit_ism_gen_set_jitter` |
| `gen_frames` | `sdrkit_ism_gen_get_frames` | `sdrkit_ism_gen_set_frames` |
| `gen_frame_gap_s` | `sdrkit_ism_gen_get_frame_gap_s` | `sdrkit_ism_gen_set_frame_gap_s` |

## Try it

```sh
sdrkit run ism --source synth:ism
sdrkit run ism --source iiod:192.168.2.1 --freq 433.92
```

## Test recordings

- `tests/vectors/ism/ism_433_scene_1msps.sigmf-meta`
- `tests/vectors/ism/ism_868_scene_1msps.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
