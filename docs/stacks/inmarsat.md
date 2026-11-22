<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Inmarsat (all carriers)

One wide L-band capture: every Inmarsat carrier found, classified and decoded at once (Aero P, Inmarsat-C), and the satellite identified. Receive only.

Stack id: `inmarsat`. Group: Satellite.

Signal types: Inmarsat-C, Inmarsat Aero.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.54557 GHz |
| Tuning range | 1.525 GHz to 1.559 GHz |
| Sample rate | 6 MHz |
| Smallest sample rate | 1.2 MHz |
| Signal bandwidth | 20 kHz |
| Radio filter | 5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:inmarsat` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `inmarsat_c` | Inmarsat-C | yes or no | true |  | Runs Inmarsat-C decoders on medium carriers. |
| `budget` | CPU budget | number (units) | 10 | 2 to 24 | Aero windows of 160 kHz cost 1, an Inmarsat-C decoder in one 0.6. Windows over the budget are parked. |
| `satellite` | Satellite | choice | `auto` | `auto` (Automatic), `Alphasat 25E`, `Inmarsat-4 F3 98W`, `Inmarsat-3 F5 54W (AOR-W)`, `Inmarsat-6 F1 83.5E`, `Inmarsat-4 F1 143.5E`, `Inmarsat 15.5W (AOR-E)`, `Inmarsat 64.5E (IOR)`, `Inmarsat 178E (POR)` | Names the satellite instead of the identification. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_channels` | Channels (1 = 600, 2 = 1200, 4 = 10500) | whole number | 6 | 1 to 7 |  |
| `gen_plain` | No built-in offsets and drift | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `satellite` | Satellite | text |
| `satellite_confidence` | Confidence | number |
| `satellite_ambiguous` | Ambiguous | yes or no |
| `beam` | Beam | text |
| `identification_note` | Note | text |
| `messages` | Messages | whole number |
| `positions` | Positions | whole number |
| `egc_frames` | Inmarsat-C frames | whole number |
| `carriers` | Carriers | whole number |
| `decoding` | Carriers decoding | whole number |
| `dropped` | Dropped | whole number |
| `receiver.dropped` | Dropped | whole number |
| `aero_display.frames_bad` | Frames bad | whole number |
| `aero_display.frames` | Frames | whole number |
| `aero_display.aircraft` | Aircraft | whole number |
| `aero_display.channels_decoding` | Channels decoding | whole number |
| `aero_display.ebn0_db` | Eb/N0 | number (dB) |
| `aero_display.dropped` | Dropped | whole number |
| `egc.frames_bad` | Frames bad | whole number |
| `egc.offset_hz` | Offset | frequency |
| `egc.drift_hz_s` | Drift | number (Hz/s) |
| `egc.frames_found` | Frames found | whole number |
| `egc.packets_ok` | Packets good | whole number |
| `egc.packets_bad` | Packets bad | whole number |
| `egc.message_count` | Messages seen | whole number |
| `egc.esn0_db` | Es/N0 | number (dB) |
| `egc.ebn0_db` | Eb/N0 | number (dB) |
| `egc.uw_errors` | Unique word errors | whole number |
| `egc.uw_errors_avg` | Unique word errors (average) | number |
| `egc.symbol_error_rate` | Symbol error rate | number |
| `egc.polarity_slips` | Polarity slips | whole number |
| `egc.ocean_region_code` | Ocean region code | whole number |
| `egc.region` | Ocean region | text |
| `egc.station_id` | Station id | whole number |
| `egc.station_name` | Station | text |
| `egc.channel_type_code` | Channel type code | whole number |
| `egc.channel_type` | Channel type | text |
| `egc.network_version` | Network version | whole number |
| `egc.signalling_channel` | Signalling channel | whole number |
| `egc.random_interval` | Random interval | whole number |
| `egc.status_byte` | Status byte | whole number |
| `egc.status_text` | Status | text |
| `egc.services_mask` | Services mask | whole number |
| `egc.services_text` | Services | text |
| `egc.signalling_uplink_hz` | Signalling uplink | frequency |
| `egc.frame_number` | Frame number | whole number |
| `egc.frame_time` | Frame time of day | duration (s) |
| `egc.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Carriers | `id` (whole number), `offset_hz` (frequency), `frequency` (frequency), `width` (frequency), `snr_db` (number (dB)), `class` (choice), `decodes` (choice), `rate` (whole number (bit/s)), `state` (whole number), `ebn0_db` (number (dB)), `frames` (whole number), `satellite` (text), `parked` (yes or no), `focused` (yes or no), `window` (whole number), `voice_candidate` (yes or no) |
| `candidates` | Satellite candidates | `name` (text), `satellite_confidence` (number), `content` (number), `frequency_score` (number), `geometry` (number), `elevation_deg` (number (deg)), `azimuth_deg` (number (deg)) |
| `candidate_reasons` | Why | `candidate` (text), `why` (text) |
| `aero_display.channels` | Channels | `id` (whole number), `offset_hz` (frequency), `frequency` (frequency), `rate` (whole number (bit/s)), `state` (choice), `ebn0_db` (number (dB)), `level_db` (number (dB)), `frames` (whole number), `uw_misses` (whole number), `sus_good` (whole number), `sus_bad` (whole number), `ber` (number) |
| `aero_display.su_types` | SU types | `su_type` (whole number), `su_name` (text), `count` (whole number) |
| `aero_display.logons` | Log-ons | `at` (duration (s)), `time` (time), `channel` (whole number), `aes` (text), `ges` (text), `logon` (yes or no) |
| `aero_display.messages` | Messages | `at` (duration (s)), `time` (time), `channel` (whole number), `rate` (whole number (bit/s)), `frequency` (frequency), `aes` (text), `ges` (text), `registration` (text), `flight` (text), `label` (text), `label_name` (text), `message_number` (text), `text` (text), `check_ok` (yes or no), `complete` (yes or no), `adsc` (text), `position` (position), `alt_ft` (number (ft)), `reading` (text), `reading_kind` (text), `reading_category` (text), `cpdlc_imi` (text), `cpdlc_ground` (text), `cpdlc_aircraft` (text), `cpdlc_crc_ok` (yes or no), `cpdlc_parsed` (yes or no), `cpdlc_text` (text) |
| `aero_display.aircraft` | Aircraft | `address` (text), `registration` (text), `country` (text), `flight` (text), `label` (text), `messages` (whole number), `position` (position), `alt_ft` (number (ft)), `track_deg` (number (deg)), `gs_kt` (number (kt)), `vs_fpm` (number (ft/min)), `last_heard` (time), `frequency` (frequency) |
| `aero_display.facts` | System | `kind` (text), `text` (text), `heard` (whole number), `last_heard` (time) |
| `egc.messages` | EGC messages | `first_heard` (time), `last_heard` (time), `channel` (whole number), `message_number` (whole number), `service_code` (whole number), `service` (text), `kind` (text), `priority` (text), `presentation` (text), `packets` (whole number), `complete` (yes or no), `times_heard` (whole number), `area` (text), `subject` (text), `warning_number` (text), `text` (text), `truncated` (yes or no) |
| `egc.channels` | Channels | `index` (whole number), `offset_hz` (frequency), `carrier` (yes or no), `state` (choice), `ebn0_db` (number (dB)), `uw_errors_avg` (number), `frame_number` (whole number), `frames_ok` (whole number), `frames_bad` (whole number), `messages` (whole number), `region` (text), `station_name` (text), `channel_type` (text) |
| `egc.packet_counts` | Packets by type | `descriptor` (whole number), `descriptor_name` (text), `packets_ok` (whole number), `packets_bad` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Spectrum | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `aero.message` | ACARS message | `source` (text), `text` (text), `family` (choice), `position` (position), `label` (text), `aes` (text), `flight` (text), `check_ok` (yes or no) |
| `aero.logon` | Log-on | `source` (text), `text` (text), `family` (choice), `ges` (text), `logon` (yes or no) |
| `inmarsatc.egc` | EGC message | `source` (text), `text` (text), `family` (choice), `urgency` (choice), `message_number` (whole number), `service_code` (whole number), `priority_code` (whole number), `subject` (text), `area` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `carrier.focus` | Focus on carrier | `row` (text, required) |  | Never parks the window of this carrier. An empty row ends the focus. |

## Typed interface

The stack is in the family `satcom`, version 1.4. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite links (`satellite_links`). Media: none.

### Result fields

Struct `InmarsatFields`. The result fields of the merged Inmarsat display.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | The ACARS messages and the Inmarsat-C bulletin frames. |
| `satellite` | text | optional | 1.0 | The satellite that the identification names. Absent while it names none. |
| `satellite_confidence` | number | optional | 1.0 | How sure the identification is, 0 to 1. |
| `satellite_ambiguous` | yes or no | always | 1.0 | Two satellites fit about as well. |
| `beam` | text | always | 1.0 | The beam, in words. |
| `identification_note` | text | always | 1.0 | A note on the identification. |
| `messages` | whole number | always | 1.0 | The ACARS messages. |
| `positions` | whole number | always | 1.0 | The ACARS messages with a position. |
| `egc_frames` | whole number | always | 1.0 | The Inmarsat-C frames with a bulletin board. |
| `carriers` | whole number | always | 1.0 | The carriers found. |
| `decoding` | whole number | always | 1.0 | The carriers that decode. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `carriers`

Struct `InmarsatCarrierRow`. A carrier found. The row key is the carrier id in five digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The id of the carrier. |
| `offset_hz` | frequency | always | 1.0 | The offset from the centre of the input. |
| `frequency` | frequency | always | 1.0 | The frequency of the carrier. |
| `width` | frequency | always | 1.0 | The width of the carrier. |
| `snr_db` | number (dB) | always | 1.0 | The level over the floor. |
| `class` | choice | always | 1.0 | The class by the width. |
| `decodes` | choice | always | 1.0 | What decodes the carrier. |
| `rate` | whole number (bit/s) | always | 1.0 | The bit rate. |
| `state` | whole number | always | 1.0 | How far the carrier decodes: 0 seen, 1 carrier, 2 frames, 3 data. |
| `ebn0_db` | number (dB) | always | 1.0 | The energy per bit over the noise density. |
| `frames` | whole number | always | 1.0 | The frames read. |
| `satellite` | text | always | 1.0 | What the carrier is on the identified satellite. |
| `parked` | yes or no | always | 1.0 | The window of the carrier is parked. |
| `focused` | yes or no | always | 1.0 | The carrier has the focus. |
| `window` | whole number | always | 1.0 | The window that holds the carrier. |
| `voice_candidate` | yes or no | always | 1.0 | The carrier has the width of a voice channel. |

The column `satellite` names a row. Default order: the column `snr_db`, highest first.

### Table `candidates`

Struct `InmarsatCandidateRow`. A satellite that the identification weighed. The row key is the name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the satellite. |
| `satellite_confidence` | number | always | 1.0 | How sure the identification is of this satellite, 0 to 1. |
| `content` | number | optional | 1.0 | The score of the decoded content. |
| `frequency_score` | number | optional | 1.0 | The score of the frequencies. |
| `geometry` | number | optional | 1.0 | The score of the geometry. |
| `elevation_deg` | number (deg) | optional | 1.0 | The elevation from the station. |
| `azimuth_deg` | number (deg) | optional | 1.0 | The azimuth from the station. |

The column `name` names a row.

### Table `candidate_reasons`

Struct `InmarsatCandidateReasonRow`. A reason of a candidate. The row key is `<candidate>/<n>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `candidate` | text | always | 1.0 | The candidate. |
| `why` | text | always | 1.0 | The reason in words. |

### Table `aero_display.channels`

Struct `AeroChannelRow`. A row of the channel table of the Aero stacks. The row key is the place of the channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The id of the channel. |
| `offset_hz` | frequency | always | 1.0 | The offset from the centre of the input. |
| `frequency` | frequency | always | 1.0 | The frequency of the channel. |
| `rate` | whole number (bit/s) | always | 1.0 | The bit rate. |
| `state` | choice | always | 1.0 | How far the channel decodes. |
| `ebn0_db` | number (dB) | always | 1.0 | The energy per bit over the noise density. |
| `level_db` | number (dB) | always | 1.0 | The level of the carrier. |
| `frames` | whole number | always | 1.0 | The frames read. |
| `uw_misses` | whole number | always | 1.0 | The frames whose unique word was missed. |
| `sus_good` | whole number | always | 1.0 | The signal units with a good check. |
| `sus_bad` | whole number | always | 1.0 | The signal units with a bad check. |
| `ber` | number | always | 1.0 | The bit error rate of the channel. |

### Table `aero_display.su_types`

Struct `AeroSuTypeRow`. The count of one SU type. The row key is the type in two hexadecimal digits (FE for SUs that failed their check).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `su_type` | whole number | always | 1.0 | The SU type. |
| `su_name` | text | always | 1.0 | The name of the type. |
| `count` | whole number | always | 1.0 | The SUs of the type. |

### Table `aero_display.logons`

Struct `AeroLogonRow`. A log-on or log-off. The row key is the number of the entry in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time. |
| `time` | time | optional | 1.0 | The wall-clock time, when known. |
| `channel` | whole number | always | 1.0 | The id of the channel. |
| `aes` | text | always | 1.0 | The AES id, six hexadecimal digits. |
| `ges` | text | always | 1.0 | The GES id, two hexadecimal digits. |
| `logon` | yes or no | always | 1.0 | A log-on (else a log-off). |

### Table `aero_display.messages`

Struct `AeroMessageRow`. An ACARS message. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time. |
| `time` | time | optional | 1.0 | The wall-clock time, when known. |
| `channel` | whole number | always | 1.0 | The id of the channel. |
| `rate` | whole number (bit/s) | always | 1.0 | The bit rate of the channel. |
| `frequency` | frequency | always | 1.0 | The frequency of the channel. |
| `aes` | text | always | 1.0 | The AES id, six hexadecimal digits. |
| `ges` | text | always | 1.0 | The GES id, two hexadecimal digits. |
| `registration` | text | always | 1.0 | The registration. |
| `flight` | text | always | 1.0 | The flight id. |
| `label` | text | always | 1.0 | The ACARS label. |
| `label_name` | text | always | 1.0 | What the label means. |
| `message_number` | text | always | 1.0 | The message sequence number. |
| `text` | text | always | 1.0 | The text. |
| `check_ok` | yes or no | always | 1.0 | The block check is good. |
| `complete` | yes or no | always | 1.0 | Every block of the message was heard. |
| `adsc` | text | always | 1.0 | The ADS-C report in words. Empty for a message that is not ADS-C. |
| `position` | position | optional | 1.0 | The position in the message. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude in the message. |
| `reading` | text | optional | 1.0 | The reading of the fixed format. |
| `reading_kind` | text | optional | 1.0 | The kind of the reading. |
| `reading_category` | text | optional | 1.0 | The category of the reading. |
| `cpdlc_imi` | text | optional | 1.0 | The CPDLC envelope: AT1, CR1, CC1 or DR1. |
| `cpdlc_ground` | text | optional | 1.0 | The CPDLC ground facility. |
| `cpdlc_aircraft` | text | optional | 1.0 | The aircraft named in the CPDLC envelope. |
| `cpdlc_crc_ok` | yes or no | optional | 1.0 | The CPDLC envelope check passed. |
| `cpdlc_parsed` | yes or no | optional | 1.0 | The CPDLC message body decoded completely. |
| `cpdlc_text` | text | optional | 1.0 | The decoded CPDLC message. |

The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`.

### Table `aero_display.aircraft`

Struct `AeroAircraftRow`. An aircraft heard: the standard aircraft columns, then the frequency last heard on. The row key is the address, else the registration.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | optional | 1.0 | The 24-bit address, six hex digits. |
| `registration` | text | optional | 1.0 | The registration. |
| `country` | text | optional | 1.0 | The State the address is allocated to. |
| `flight` | text | optional | 1.0 | The flight id. |
| `label` | text | optional | 1.0 | The label of the last message. |
| `messages` | whole number | always | 1.0 | Messages heard. |
| `position` | position | optional | 1.0 | The last position. |
| `alt_ft` | number (ft) | optional | 1.0 | The altitude of the last position. |
| `track_deg` | number (deg) | optional | 1.0 | The track, degrees true. |
| `gs_kt` | number (kt) | optional | 1.0 | The ground speed. |
| `vs_fpm` | number (ft/min) | optional | 1.0 | The vertical rate. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard. |
| `frequency` | frequency | always | 1.0 | The frequency last heard on. |

The column `flight` names a row. Place of a row: the column `position`. Height: `alt_ft`. Course: `track_deg`. Speed: `gs_kt`.

### Table `aero_display.facts`

Struct `AeroFactRow`. A system fact (a satellite, the control of the P and R channels). The row key is the key of the fact.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | text | always | 1.0 | The kind of the fact. |
| `text` | text | always | 1.0 | The fact in words. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard. |

### Table `egc.messages`

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

### Table `egc.channels`

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

### Table `egc.packet_counts`

Struct `InmarsatcPacketCountRow`. The packets of one descriptor type. The row key is the descriptor in hexadecimal ("2A").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `descriptor` | whole number | always | 1.0 | The descriptor. |
| `descriptor_name` | text | always | 1.0 | The packet type. |
| `packets_ok` | whole number | always | 1.0 | The packets good. |
| `packets_bad` | whole number | always | 1.0 | The packets bad. |

### Event `aero.message`

Struct `AeroMessageEvent`. An ACARS message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The registration. Absent when the message names none. |
| `text` | text | always | 1.0 | The text. |
| `family` | choice | always | 1.0 | What the event is about: an aircraft. |
| `position` | position | optional | 1.0 | The position in the message. |
| `label` | text | always | 1.0 | The ACARS label. |
| `aes` | text | always | 1.0 | The AES id, six hexadecimal digits. |
| `flight` | text | always | 1.0 | The flight id. |
| `check_ok` | yes or no | always | 1.0 | The block check is good. |

### Event `aero.logon`

Struct `AeroLogonEvent`. A log-on or log-off.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The AES id, six hexadecimal digits. |
| `text` | text | always | 1.0 | `log-on` or `log-off`. |
| `family` | choice | always | 1.0 | What the event is about: an aircraft. |
| `ges` | text | always | 1.0 | The GES id, two hexadecimal digits. |
| `logon` | yes or no | always | 1.0 | A log-on (else a log-off). |

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
| `clear` | `sdrkit_inmarsat_clear` | none | none |
| `carrier.focus` | `sdrkit_inmarsat_carrier_focus` | `sdrkit_inmarsat_carrier_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `inmarsat_c` | `sdrkit_inmarsat_get_inmarsat_c` | `sdrkit_inmarsat_set_inmarsat_c` |
| `budget` | `sdrkit_inmarsat_get_budget` | `sdrkit_inmarsat_set_budget` |
| `satellite` | `sdrkit_inmarsat_get_satellite` | `sdrkit_inmarsat_set_satellite` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_channels` | `sdrkit_inmarsat_gen_get_channels` | `sdrkit_inmarsat_gen_set_channels` |
| `gen_plain` | `sdrkit_inmarsat_gen_get_plain` | `sdrkit_inmarsat_gen_set_plain` |

## Try it

```sh
sdrkit run inmarsat --source synth:inmarsat
sdrkit run inmarsat --source iiod:192.168.2.1 --freq 1545.57
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
