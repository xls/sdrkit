<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Inmarsat-C

Inmarsat-C NCS and LES channels: the bulletin board and SafetyNET / FleetNET messages. Receive only.

Stack id: `inmarsatc`. Group: Satellite.

Signal types: Inmarsat-C.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.5371 GHz |
| Tuning range | 1.525 GHz to 1.559 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 5 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:inmarsatc` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel_search` | Channel search | yes or no | true |  | Look for up to 3 more channels in the capture. |
| `search_span_hz` | Search span | frequency | 0 Hz | 0 Hz to 100 MHz | Limit of the channel search around 0 Hz (0: the whole input). |
| `satellite` | Satellite | choice | `none` | `none` (None (tune by hand)), `alphasat` (Alphasat 25E (Europe, Africa, Indian Ocean) 1537.10 MHz IOR), `i4f3` (Inmarsat-4 F3 98W (Americas) 1537.70 MHz AOR-W), `i3f5` (Inmarsat-3 F5 54W (Atlantic East) 1541.45 MHz AOR-E), `i4f1` (Inmarsat-4 F1 143.5E (Pacific, Asia) 1541.45 MHz POR (community value)) | Asks the host to tune the dial to the NCS common channel of this satellite. |
| `min_priority` | Priority | choice | `0` | `0` (All), `1` (Safety and higher), `2` (Urgency and higher), `3` (Distress) | Shows the messages of this priority and higher. |
| `service_filter` | Service | choice | `all` | `all` (All), `safetynet` (SafetyNET), `fleetnet` (FleetNET) | Shows the messages of this service only. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_invert` | Inverted polarity | yes or no | false |  |  |
| `gen_first_frame` | First frame number | whole number | 1000 | 0 to 9999 |  |
| `gen_extra` | Extra channels | whole number | 0 | 0 to 3 |  |
| `gen_start_symbols` | Start into the first frame | whole number (symbols) | 0 | 0 to 10367 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `offset_hz` | Offset | frequency |
| `drift_hz_s` | Drift | number (Hz/s) |
| `frames_found` | Frames found | whole number |
| `packets_ok` | Packets good | whole number |
| `packets_bad` | Packets bad | whole number |
| `message_count` | Messages seen | whole number |
| `esn0_db` | Es/N0 | number (dB) |
| `ebn0_db` | Eb/N0 | number (dB) |
| `uw_errors` | Unique word errors | whole number |
| `uw_errors_avg` | Unique word errors (average) | number |
| `symbol_error_rate` | Symbol error rate | number |
| `polarity_slips` | Polarity slips | whole number |
| `ocean_region_code` | Ocean region code | whole number |
| `region` | Ocean region | text |
| `station_id` | Station id | whole number |
| `station_name` | Station | text |
| `channel_type_code` | Channel type code | whole number |
| `channel_type` | Channel type | text |
| `network_version` | Network version | whole number |
| `signalling_channel` | Signalling channel | whole number |
| `random_interval` | Random interval | whole number |
| `status_byte` | Status byte | whole number |
| `status_text` | Status | text |
| `services_mask` | Services mask | whole number |
| `services_text` | Services | text |
| `signalling_uplink_hz` | Signalling uplink | frequency |
| `frame_number` | Frame number | whole number |
| `frame_time` | Frame time of day | duration (s) |
| `dropped` | Dropped | whole number |
| `demod.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | EGC messages | `first_heard` (time), `last_heard` (time), `channel` (whole number), `message_number` (whole number), `service_code` (whole number), `service` (text), `kind` (text), `priority` (text), `presentation` (text), `packets` (whole number), `complete` (yes or no), `times_heard` (whole number), `area` (text), `subject` (text), `warning_number` (text), `text` (text), `truncated` (yes or no) |
| `channels` | Channels | `index` (whole number), `offset_hz` (frequency), `carrier` (yes or no), `state` (choice), `ebn0_db` (number (dB)), `uw_errors_avg` (number), `frame_number` (whole number), `frames_ok` (whole number), `frames_bad` (whole number), `messages` (whole number), `region` (text), `station_name` (text), `channel_type` (text) |
| `packet_counts` | Packets by type | `descriptor` (whole number), `descriptor_name` (text), `packets_ok` (whole number), `packets_bad` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `inmarsatc.egc` | EGC message | `source` (text), `text` (text), `family` (choice), `urgency` (choice), `message_number` (whole number), `service_code` (whole number), `priority_code` (whole number), `subject` (text), `area` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.5. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: text.

### Result fields

Struct `InmarsatcFields`. The result fields of Inmarsat-C.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `offset_hz` | frequency | always | 1.0 | The offset of the carrier of the first channel from the tuned frequency. |
| `drift_hz_s` | number (Hz/s) | always | 1.0 | The drift. |
| `frames_found` | whole number | always | 1.0 | The frames found. |
| `packets_ok` | whole number | always | 1.0 | The packets good. |
| `packets_bad` | whole number | always | 1.0 | The packets bad. |
| `message_count` | whole number | always | 1.0 | The messages seen. |
| `esn0_db` | number (dB) | optional | 1.0 | The es/N0. |
| `ebn0_db` | number (dB) | optional | 1.0 | The eb/N0. |
| `uw_errors` | whole number | optional | 1.0 | The unique word errors. |
| `uw_errors_avg` | number | optional | 1.0 | The unique word errors (average). |
| `symbol_error_rate` | number | optional | 1.0 | The symbol error rate. |
| `polarity_slips` | whole number | always | 1.0 | The polarity slips. |
| `ocean_region_code` | whole number | optional | 1.0 | The ocean region code. |
| `region` | text | optional | 1.0 | The ocean region. |
| `station_id` | whole number | optional | 1.0 | The station id. |
| `station_name` | text | optional | 1.0 | The station. |
| `channel_type_code` | whole number | optional | 1.0 | The channel type code. |
| `channel_type` | text | optional | 1.0 | The channel type. |
| `network_version` | whole number | optional | 1.0 | The network version. |
| `signalling_channel` | whole number | optional | 1.0 | The signalling channel. |
| `random_interval` | whole number | optional | 1.0 | The random interval. |
| `status_byte` | whole number | optional | 1.0 | The status byte. |
| `status_text` | text | optional | 1.0 | The status. |
| `services_mask` | whole number | optional | 1.0 | The services mask. |
| `services_text` | text | optional | 1.0 | The services. |
| `signalling_uplink_hz` | frequency | optional | 1.0 | The signalling uplink. |
| `frame_number` | whole number | optional | 1.0 | The frame number. |
| `frame_time` | duration (s) | optional | 1.0 | The frame time of day. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `InmarsatcMessageRow`. An EGC message. The row key is the message number and the service.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `first_heard` | time | optional | 1.0 | The first heard. |
| `last_heard` | time | optional | 1.0 | When it was last heard. |
| `channel` | whole number | always | 1.0 | The channel a message came on (0 the main one). |
| `message_number` | whole number | always | 1.0 | The message number. |
| `service_code` | whole number | always | 1.0 | The EGC service code. |
| `service` | text | always | 1.0 | The service. |
| `kind` | text | always | 1.0 | The kind. |
| `priority` | text | always | 1.0 | The priority. |
| `presentation` | text | always | 1.0 | The presentation. |
| `packets` | whole number | always | 1.0 | The packets of the message heard. |
| `complete` | yes or no | always | 1.0 | Every packet of the message was heard. |
| `times_heard` | whole number | always | 1.0 | The heard complete. |
| `area` | text | always | 1.0 | The area of a message. |
| `subject` | text | always | 1.0 | The subject of a message. |
| `warning_number` | text | always | 1.0 | The warning. |
| `text` | text | always | 1.0 | The text of the message. |
| `truncated` | yes or no | always | 1.0 | The text of the message is cut short. |

The rows are the messages of the stack. Text: `text`. Time: `first_heard`.

### Table `channels`

Struct `InmarsatcChannelRow`. A channel followed now. The row key is the channel index.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `index` | whole number | always | 1.0 | The channel index. |
| `offset_hz` | frequency | always | 1.0 | The offset of the carrier from the tuned frequency. |
| `carrier` | yes or no | always | 1.0 | A carrier was found. |
| `state` | choice | always | 1.0 | The state. |
| `ebn0_db` | number (dB) | always | 1.0 | The eb/N0. |
| `uw_errors_avg` | number | always | 1.0 | The unique word errors (average). |
| `frame_number` | whole number | always | 1.0 | The frame number. |
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `messages` | whole number | always | 1.0 | The messages. |
| `region` | text | always | 1.0 | The ocean region. |
| `station_name` | text | always | 1.0 | The station. |
| `channel_type` | text | always | 1.0 | The channel type. |

### Table `packet_counts`

Struct `InmarsatcPacketCountRow`. The packets of one descriptor type. The row key is the descriptor in hexadecimal ("2A").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `descriptor` | whole number | always | 1.0 | The descriptor. |
| `descriptor_name` | text | always | 1.0 | The packet type. |
| `packets_ok` | whole number | always | 1.0 | The packets good. |
| `packets_bad` | whole number | always | 1.0 | The packets bad. |

### Event `inmarsatc.egc`

Struct `InmarsatcEgcEvent`. A complete EGC message. The source is the message number and the service, the text the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The message number and the service. |
| `text` | text | always | 1.0 | The text of the message. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `urgency` | choice | always | 1.0 | How urgent the message is, from its priority. |
| `message_number` | whole number | always | 1.0 | The message number. |
| `service_code` | whole number | always | 1.0 | The EGC service code. |
| `priority_code` | whole number | always | 1.0 | The EGC priority code (0 routine, 1 safety, 2 urgency, 3 distress). |
| `subject` | text | always | 1.0 | The subject of a message. |
| `area` | text | always | 1.0 | The area of a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_inmarsatc_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel_search` | `sdrkit_inmarsatc_get_channel_search` | `sdrkit_inmarsatc_set_channel_search` |
| `search_span_hz` | `sdrkit_inmarsatc_get_search_span_hz` | `sdrkit_inmarsatc_set_search_span_hz` |
| `satellite` | `sdrkit_inmarsatc_get_satellite` | `sdrkit_inmarsatc_set_satellite` |
| `min_priority` | `sdrkit_inmarsatc_get_min_priority` | `sdrkit_inmarsatc_set_min_priority` |
| `service_filter` | `sdrkit_inmarsatc_get_service_filter` | `sdrkit_inmarsatc_set_service_filter` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_invert` | `sdrkit_inmarsatc_gen_get_invert` | `sdrkit_inmarsatc_gen_set_invert` |
| `gen_first_frame` | `sdrkit_inmarsatc_gen_get_first_frame` | `sdrkit_inmarsatc_gen_set_first_frame` |
| `gen_extra` | `sdrkit_inmarsatc_gen_get_extra` | `sdrkit_inmarsatc_gen_set_extra` |
| `gen_start_symbols` | `sdrkit_inmarsatc_gen_get_start_symbols` | `sdrkit_inmarsatc_gen_set_start_symbols` |

## Try it

```sh
sdrkit run inmarsatc --source synth:inmarsatc
sdrkit run inmarsatc --source iiod:192.168.2.1 --freq 1537.1
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
