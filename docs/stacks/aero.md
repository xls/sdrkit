<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Inmarsat Aero

Inmarsat Aero P channels (600, 1200, 10500 bit/s): log-ons, ACARS, ADS-C and aircraft positions. Receive only.

Stack id: `aero`. Group: Satellite.

Signal types: Inmarsat Aero.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.54557 GHz |
| Tuning range | 1.525 GHz to 1.559 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 240 kHz |
| Signal bandwidth | 20 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 3.5 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:aero` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `search_limit_hz` | Search limit | frequency | 0 Hz | 0 Hz to 10 MHz | The band search looks only within +-n Hz (0: the whole input). |
| `satellite` | Satellite | choice | `none` | `none` (None (tune by hand, 1545.57 MHz)), `alphasat` (Alphasat 25E (Europe, Africa)), `i4f3` (Inmarsat-4 F3 98W (Americas)), `i3f5` (Inmarsat-3 F5 54W (Atlantic, South America)), `i6f1` (Inmarsat-6 F1 83.5E (Indian Ocean, Asia-Pacific)), `i4f1` (Inmarsat-4 F1 143.5E (Pacific, Asia)) | Asks the host to tune the dial to the middle of the satellite's two P channel groups. |
| `filter_label` | Label filter | text | empty |  | Shows the messages whose label holds this text. |
| `filter_registration` | Registration filter | text | empty |  | Shows the messages whose registration holds this text. |
| `show_failed` | Show failed messages | yes or no | false |  | Shows messages whose block check failed too. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_channels` | Channels (1 = 600, 2 = 1200, 4 = 10500) | whole number | 6 | 1 to 7 |  |
| `gen_plain` | No built-in offsets and drift | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `frames` | Frames | whole number |
| `messages` | Messages | whole number |
| `positions` | Positions | whole number |
| `aircraft` | Aircraft | whole number |
| `channels_decoding` | Channels decoding | whole number |
| `ebn0_db` | Eb/N0 | number (dB) |
| `dropped` | Dropped | whole number |
| `receiver.dropped` | Dropped | whole number |
| `su.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `channels` | Channels | `id` (whole number), `offset_hz` (frequency), `frequency` (frequency), `rate` (whole number (bit/s)), `state` (choice), `ebn0_db` (number (dB)), `level_db` (number (dB)), `frames` (whole number), `uw_misses` (whole number), `sus_good` (whole number), `sus_bad` (whole number), `ber` (number) |
| `su_types` | SU types | `su_type` (whole number), `su_name` (text), `count` (whole number) |
| `logons` | Log-ons | `at` (duration (s)), `time` (time), `channel` (whole number), `aes` (text), `ges` (text), `logon` (yes or no) |
| `messages` | Messages | `at` (duration (s)), `time` (time), `channel` (whole number), `rate` (whole number (bit/s)), `frequency` (frequency), `aes` (text), `ges` (text), `registration` (text), `flight` (text), `label` (text), `label_name` (text), `message_number` (text), `text` (text), `check_ok` (yes or no), `complete` (yes or no), `adsc` (text), `position` (position), `alt_ft` (number (ft)), `reading` (text), `reading_kind` (text), `reading_category` (text), `cpdlc_imi` (text), `cpdlc_ground` (text), `cpdlc_aircraft` (text), `cpdlc_crc_ok` (yes or no), `cpdlc_parsed` (yes or no), `cpdlc_text` (text) |
| `aircraft` | Aircraft | `address` (text), `registration` (text), `country` (text), `flight` (text), `label` (text), `messages` (whole number), `position` (position), `alt_ft` (number (ft)), `track_deg` (number (deg)), `gs_kt` (number (kt)), `vs_fpm` (number (ft/min)), `last_heard` (time), `frequency` (frequency) |
| `facts` | System | `kind` (text), `text` (text), `heard` (whole number), `last_heard` (time) |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `aero.message` | ACARS message | `source` (text), `text` (text), `family` (choice), `position` (position), `label` (text), `aes` (text), `flight` (text), `check_ok` (yes or no) |
| `aero.logon` | Log-on | `source` (text), `text` (text), `family` (choice), `ges` (text), `logon` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `satcom`, version 1.4. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: text.

### Result fields

Struct `AeroFields`. The result fields of the Aero display.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | The signal units with a good check. |
| `frames_bad` | whole number | always | 1.0 | The signal units with a bad check. |
| `frames` | whole number | always | 1.0 | The frames read. |
| `messages` | whole number | always | 1.0 | The messages in the log. |
| `positions` | whole number | always | 1.0 | The positions heard. |
| `aircraft` | whole number | always | 1.0 | The aircraft in the table. |
| `channels_decoding` | whole number | always | 1.0 | The channels that decode frames. |
| `ebn0_db` | number (dB) | optional | 1.0 | The best Eb/N0 of the channels that decode. Absent while none decodes. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `channels`

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

### Table `su_types`

Struct `AeroSuTypeRow`. The count of one SU type. The row key is the type in two hexadecimal digits (FE for SUs that failed their check).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `su_type` | whole number | always | 1.0 | The SU type. |
| `su_name` | text | always | 1.0 | The name of the type. |
| `count` | whole number | always | 1.0 | The SUs of the type. |

### Table `logons`

Struct `AeroLogonRow`. A log-on or log-off. The row key is the number of the entry in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time. |
| `time` | time | optional | 1.0 | The wall-clock time, when known. |
| `channel` | whole number | always | 1.0 | The id of the channel. |
| `aes` | text | always | 1.0 | The AES id, six hexadecimal digits. |
| `ges` | text | always | 1.0 | The GES id, two hexadecimal digits. |
| `logon` | yes or no | always | 1.0 | A log-on (else a log-off). |

### Table `messages`

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

### Table `aircraft`

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

### Table `facts`

Struct `AeroFactRow`. A system fact (a satellite, the control of the P and R channels). The row key is the key of the fact.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | text | always | 1.0 | The kind of the fact. |
| `text` | text | always | 1.0 | The fact in words. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard. |

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

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_aero_clear` | none | none |
| `remove` | `sdrkit_aero_remove` | `sdrkit_aero_aircraft_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `search_limit_hz` | `sdrkit_aero_get_search_limit_hz` | `sdrkit_aero_set_search_limit_hz` |
| `satellite` | `sdrkit_aero_get_satellite` | `sdrkit_aero_set_satellite` |
| `filter_label` | `sdrkit_aero_get_filter_label` | `sdrkit_aero_set_filter_label` |
| `filter_registration` | `sdrkit_aero_get_filter_registration` | `sdrkit_aero_set_filter_registration` |
| `show_failed` | `sdrkit_aero_get_show_failed` | `sdrkit_aero_set_show_failed` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_channels` | `sdrkit_aero_gen_get_channels` | `sdrkit_aero_gen_set_channels` |
| `gen_plain` | `sdrkit_aero_gen_get_plain` | `sdrkit_aero_gen_set_plain` |

## Try it

```sh
sdrkit run aero --source synth:aero
sdrkit run aero --source iiod:192.168.2.1 --freq 1545.57
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
