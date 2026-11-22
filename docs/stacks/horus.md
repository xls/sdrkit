<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Horus balloons

Amateur high-altitude balloons: Horus Binary v1 and v2 and RTTY telemetry, several balloons at once across the span, with their tracks, climb rates and custom sensor fields. The coding constants of Horus Binary are not confirmed by a public document.

Stack id: `horus`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 400 MHz to 450 MHz |
| Sample rate | 960 kHz |
| Smallest sample rate | 16 kHz |
| Signal bandwidth | 900 kHz |
| Offset from the signal | 30 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:horus` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `span_search` | Look for other balloons in the span | yes or no | true |  | Opens a channel on each narrow signal of the span, at 40 ksps and more. |
| `max_channels` | Channels at most | whole number | 6 | 1 to 8 | The dial channel and the channels of the span search; 1 is the dial only. |
| `payload_ids` | Payload IDs | text | `256, 4FSKTEST-V2` |  | One "id, callsign" a line (id 0 to 65535); # starts a comment; comma, tab, semicolon and spaces separate. An ID that the list does not name is shown as "ID n". |
| `custom_formats` | Custom fields of v2 | text | `4FSKTEST-V2\|<hhBHxx\|ascent_rate:divide_by_100,ext_temperature:divide_by_10,ext_humidity:none,ext_pressure:divide_by_10\|\|\|Default custom fields for RS41ng` |  | One format a line: callsign \| struct \| name:post-processing, ... \| sentence names, ... \| other payload IDs, ... \| comment. Post-processing: none, battery_5v_byte, divide_by_10, divide_by_100. A list with a line that does not read is refused. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_content` | Content | choice | `v2` | `v2` (Horus Binary v2), `v1` (Horus Binary v1), `rtty100` (RTTY 100 baud 7N2), `rtty300` (RTTY 300 baud 8N2), `rtty50` (RTTY 50 baud 7N2), `three` (Three balloons) | Binary v2 or v1, RTTY at 100 baud 7N2, 300 baud 8N2 or 50 baud 7N2, or three balloons (v2, v1 12 kHz above, RTTY 18 kHz below). |
| `gen_spacing_hz` | Tone spacing | number (Hz) | 0 | 0 to 2000 | 0 for 270 Hz (4FSK) and 500 Hz (RTTY). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `channels` | Channels | whole number |
| `syncs` | Syncs | whole number |
| `failed` | Failed | whole number |
| `snr_db` | SNR | number (dB) |
| `packets` | Packets | whole number |
| `payloads_active` | Active payloads | whole number |
| `unreadable_ids` | Unreadable ID lines | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `payloads` | Payloads | `payload` (text), `type` (choice), `payload_id` (whole number), `seq` (whole number), `time` (time), `position` (position), `height_m` (number (m)), `max_alt_m` (number (m)), `climb_m_s` (number (m/s)), `speed_kmh` (number (km/h)), `sats` (whole number), `temp_c` (number (°C)), `battery_v` (number (V)), `snr_db` (number (dB)), `packets` (whole number), `offset` (frequency), `frequency` (frequency), `baud` (number (Bd)), `spacing` (frequency), `corrections` (whole number), `rtty_bits` (whole number), `track_points` (whole number), `sentence` (text), `first_heard` (time), `last_heard` (time) |
| `custom_fields` | Custom fields | `payload` (text), `name` (text), `value` (text) |
| `packets` | Packets | `received` (time), `payload` (text), `type` (choice), `snr_db` (number (dB)), `offset` (frequency), `channel` (whole number), `sentence` (text) |
| `channels` | Channels | `offset` (frequency), `decoding` (choice), `tone_1` (frequency), `tone_2` (frequency), `tone_3` (frequency), `tone_4` (frequency), `rtty_low` (frequency), `rtty_high` (frequency), `spacing` (frequency), `snr_db` (number (dB)), `packets` (whole number), `syncs` (whole number), `failed` (whole number), `searched` (yes or no) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Dial channel spectrum | Hz | dB |

## Map points

The stack puts points on a map: Horus balloons.

## Events

| Kind | Label | Fields |
|---|---|---|
| `horus.packet` | Horus packet | `source` (text), `text` (text), `family` (choice), `position` (position), `height_m` (number (m)), `type` (choice), `seq` (whole number), `climb_m_s` (number (m/s)), `speed_kmh` (number (km/h)), `sats` (whole number), `temp_c` (number (°C)), `battery_v` (number (V)), `snr_db` (number (dB)), `offset` (frequency), `channel` (whole number), `sentence` (text) |
| `horus.channel_opened` | Channel opened | `text` (text), `channel` (whole number), `offset` (frequency) |
| `horus.channel_closed` | Channel closed | `text` (text), `channel` (whole number), `offset` (frequency) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `science`, version 1.3. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `HorusFields`. The present state of the channels of `horus`. The channel bank writes it; a change of frequency clears it.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | Waiting for tones, listening to tones, or decoding (a packet within the last 30 s). |
| `channels` | whole number | always | 1.0 | Channels running, the dial channel included. |
| `syncs` | whole number | always | 1.0 | Unique words found. |
| `failed` | whole number | always | 1.0 | Frames that failed their CRC. |
| `snr_db` | number (dB) | optional | 1.0 | The best SNR in 3 kHz of a packet of the last 60 s. |

### Result fields

Struct `HorusLogFields`. What `horus` received: the packet log and the payloads. It stays over a change of frequency.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `packets` | whole number | always | 1.0 | Packets decoded, duplicates left out. |
| `payloads_active` | whole number | always | 1.0 | Payloads heard within the last 120 s. |
| `unreadable_ids` | whole number | always | 1.0 | Lines of the setting `payload_ids` that could not be read. |

### Table `payloads`

Struct `HorusPayloadRow`. A row of the table `payloads`. The row key is the payload: its callsign, or "ID n" for an ID that the list does not name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `payload` | text | always | 1.0 | The callsign, or "ID n". |
| `type` | choice | always | 1.0 | The kind of the last packet. |
| `payload_id` | whole number | optional | 1.0 | The payload ID of a binary packet. |
| `seq` | whole number | always | 1.0 | The sequence number of the last packet. |
| `time` | time | always | 1.0 | The time of the last packet: its time of day on the receive date, UTC. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `height_m` | number (m) | always | 1.0 | The height of the object that the event is about, metres above mean sea level. |
| `max_alt_m` | number (m) | always | 1.0 | The highest altitude so far. |
| `climb_m_s` | number (m/s) | optional | 1.0 | The climb rate over the track of the last 30 to 180 s. |
| `speed_kmh` | number (km/h) | optional | 1.0 | The speed of a binary packet. |
| `sats` | whole number | optional | 1.0 | The satellites of a binary packet. |
| `temp_c` | number (°C) | optional | 1.0 | The temperature of a binary packet. |
| `battery_v` | number (V) | optional | 1.0 | The battery of a binary packet. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `packets` | whole number | always | 1.0 | Packets of the payload. |
| `offset` | frequency | always | 1.0 | The middle of the tones of the last packet, from the dial. |
| `frequency` | frequency | optional | 1.0 | The frequency of the middle of the tones. |
| `baud` | number (Bd) | always | 1.0 | The symbol rate of the last packet. |
| `spacing` | frequency | always | 1.0 | The tone spacing (4FSK) or shift (RTTY) of the last packet. |
| `corrections` | whole number | optional | 1.0 | The bits that the Golay code corrected in the last binary packet. |
| `rtty_bits` | whole number | optional | 1.0 | The data bits of a character of the last RTTY packet. |
| `track_points` | whole number | always | 1.0 | Points of the track. |
| `sentence` | text | always | 1.0 | The last packet as a sentence. |
| `first_heard` | time | optional | 1.0 | Wall-clock time when first heard, in whole seconds. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

The column `payload` names a row. Place of a row: the column `position`. Height: `height_m`. Speed: `speed_kmh`.

### Table `custom_fields`

Struct `HorusCustomFieldRow`. A custom field of the last v2 packet of a payload, or a field after the altitude of its last RTTY sentence. The row key is the payload, "/" and the place of the field from 1.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `payload` | text | always | 1.0 | The payload. |
| `name` | text | always | 1.0 | The name of the field. |
| `value` | text | always | 1.0 | The value, written by its post-processing. |

The column `name` names a row.

### Table `packets`

Struct `HorusPacketRow`. A row of the packet log. The row key is the number of the packet, six digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `received` | time | always | 1.0 | When the packet was received. |
| `payload` | text | always | 1.0 | The payload. |
| `type` | choice | always | 1.0 | The kind of the packet. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset` | frequency | always | 1.0 | The middle of the tones, from the dial. |
| `channel` | whole number | always | 1.0 | The channel that received it (0 is the dial). |
| `sentence` | text | always | 1.0 | The packet as a sentence. |

The column `payload` names a row.

### Table `channels`

Struct `HorusChannelRow`. A row of the table `channels`. The row key is the channel number (0 is the dial channel).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset` | frequency | always | 1.0 | The centre of the channel, from the dial. |
| `decoding` | choice | always | 1.0 | What the channel decodes. |
| `tone_1` | frequency | optional | 1.0 | The lowest 4FSK tone, from the channel centre. |
| `tone_2` | frequency | optional | 1.0 | The second 4FSK tone. |
| `tone_3` | frequency | optional | 1.0 | The third 4FSK tone. |
| `tone_4` | frequency | optional | 1.0 | The highest 4FSK tone. |
| `rtty_low` | frequency | optional | 1.0 | The lower RTTY tone, from the channel centre. |
| `rtty_high` | frequency | optional | 1.0 | The higher RTTY tone. |
| `spacing` | frequency | optional | 1.0 | The 4FSK tone spacing. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `packets` | whole number | always | 1.0 | Packets decoded. |
| `syncs` | whole number | always | 1.0 | Unique words found. |
| `failed` | whole number | always | 1.0 | Frames that failed their CRC. |
| `searched` | yes or no | always | 1.0 | The span search opened the channel. |

### Event `horus.packet`

Struct `HorusPacketEvent`. A Horus packet. `source` is the payload, `text` the summary "Horus CALL #seq lat lon alt m".

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `height_m` | number (m) | optional | 1.0 | The height of the object that the event is about, metres above mean sea level. |
| `type` | choice | always | 1.0 | The kind of the packet. |
| `seq` | whole number | always | 1.0 | The sequence number. |
| `climb_m_s` | number (m/s) | optional | 1.0 | The climb rate. |
| `speed_kmh` | number (km/h) | optional | 1.0 | The speed of a binary packet. |
| `sats` | whole number | optional | 1.0 | The satellites of a binary packet. |
| `temp_c` | number (°C) | optional | 1.0 | The temperature of a binary packet. |
| `battery_v` | number (V) | optional | 1.0 | The battery of a binary packet. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset` | frequency | always | 1.0 | The middle of the tones, from the dial. |
| `channel` | whole number | always | 1.0 | The channel that received it (0 is the dial). |
| `sentence` | text | always | 1.0 | The packet as a sentence. |

### Event `horus.channel_opened`

Struct `HorusChannelOpenedEvent`. The span search opened a channel on a signal. `text` is "a signal +x.x kHz from the dial".

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `channel` | whole number | always | 1.0 | The channel number. |
| `offset` | frequency | always | 1.0 | The centre of the channel, from the dial. |

### Event `horus.channel_closed`

Struct `HorusChannelClosedEvent`. A channel that the span search opened closed: it decoded nothing for 120 s and its signal was gone for 30 s.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `channel` | whole number | always | 1.0 | The channel number. |
| `offset` | frequency | always | 1.0 | The centre of the channel, from the dial. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_horus_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `span_search` | `sdrkit_horus_get_span_search` | `sdrkit_horus_set_span_search` |
| `max_channels` | `sdrkit_horus_get_max_channels` | `sdrkit_horus_set_max_channels` |
| `payload_ids` | `sdrkit_horus_get_payload_ids` | `sdrkit_horus_set_payload_ids` |
| `custom_formats` | `sdrkit_horus_get_custom_formats` | `sdrkit_horus_set_custom_formats` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_content` | `sdrkit_horus_gen_get_content` | `sdrkit_horus_gen_set_content` |
| `gen_spacing_hz` | `sdrkit_horus_gen_get_spacing_hz` | `sdrkit_horus_gen_set_spacing_hz` |

## Try it

```sh
sdrkit run horus --source synth:horus
sdrkit run horus --source <driver>:<address> --freq 0
```
<!-- generated: end -->
