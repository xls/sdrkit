<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# D-STAR

D-STAR digital voice: callsigns and repeaters from the header, slow-data messages and positions. Transmits a slow-data text with the radio header; the voice frames carry AMBE silence (no AMBE encoder).

Stack id: `dstar`. Group: Digital voice.

Signal types: D-STAR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 28 MHz to 1.3 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 96 kHz |
| Signal bandwidth | 6.25 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dstar` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `squelch_pct` | Error squelch | number (%) | 0 | 0 to 50 | Frames with a higher bit error rate are not played. |
| `allow_only` | Allowed only | yes or no | false |  | Play only calls on the allow list. |
| `allow` | Allow list | text | empty |  | Numbers, ranges 100-199, callsigns and prefixes LY*. |
| `mute_list` | Mute list | text | empty |  | Numbers, ranges 100-199, callsigns and prefixes LY*. |
| `priority` | Priority list | text | empty |  | A call on this list takes the sound at once. |
| `hold` | Hold | text | empty |  | Only this target is played. |
| `labels` | Labels | text | empty |  | Lines 91=Worldwide, and RID 3120123=W1ABC Joe. |
| `hold_timeout_s` | Hold timeout | number (s) | 0 | 0 to 3600 | The hold lets go after this long without a call on it (0: until released). |
| `hang_s` | Hang time | number (s) | 2 | 0.5 to 10 |  |
| `listen` | Listen | yes or no | true |  | Off: decode and list, never play speech. |
| `callsign` | Own call sign | text | empty |  | MY of the radio header: up to 8 characters of A to Z, 0 to 9, / and space. Nothing is sent while it is empty. |
| `suffix` | Suffix | text | empty |  | Up to 4 characters after MY, for example the radio type. |
| `your_call` | Your call sign | text | `CQCQCQ` |  | YOUR of the radio header: a call sign, CQCQCQ for everyone, or a command of a gateway; up to 8 characters. |
| `rpt1` | Repeater 1 | text | `DIRECT` |  | RPT1 of the radio header: DIRECT for simplex, or the repeater with its module letter in position 8. |
| `rpt2` | Repeater 2 | text | `DIRECT` |  | RPT2 of the radio header: DIRECT for simplex, or the repeater with G in position 8 for the gateway. |
| `via_repeater` | Via repeater | yes or no | false |  | Sets bit 6 (0x40) of flag byte 1: the transmission goes through a repeater. |
| `tx_split` | Transmit offset | frequency | 0 Hz | -10 GHz to 10 GHz | The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `cnr_db` | Carrier to noise | number (dB) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `deviation_hz` | Deviation | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `ber` | Bit error rate | number |
| `inverted` | Inverted | yes or no |
| `blocks_ok` | Blocks good | whole number |
| `blocks_bad` | Blocks bad | whole number |
| `frames` | Frames | whole number |
| `syncs` | Frame syncs | whole number |
| `reports` | Reports | whole number |
| `verdict` | Verdict | choice |
| `headers_ok` | Headers | whole number |
| `dstar_voice_frames` | Voice frames | whole number |
| `sync_inverted` | Inverted sync | yes or no |
| `playing` | Playing call | text |
| `held` | Held target | text |
| `codec` | Codec | text |
| `dropped` | Dropped | whole number |
| `transmitting` | Transmitting | yes or no |
| `tx_freq_hz` | Transmit frequency | frequency |
| `tx_kind` | Sending | choice |
| `tx_frames` | Frames sent | whole number |
| `tx_note` | Transmit note | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `calls` | Calls | `id` (whole number), `start_s` (number (s)), `duration` (duration (s)), `slot` (whole number), `kind` (choice), `src` (text), `dst` (text), `group` (yes or no), `emergency` (yes or no), `access_kind` (choice), `access_code` (whole number), `encryption` (choice), `algorithm` (text), `alg_id` (whole number), `enc_key_id` (whole number), `alias` (text), `text` (text), `voice_frames` (whole number), `media_frames` (whole number), `ber` (number), `active` (yes or no), `late_entry` (yes or no), `end_seen` (yes or no), `signal_lost` (yes or no), `carrier_hz` (frequency), `end` (choice) |
| `call_details` | Call details | `call` (text), `detail` (choice), `value` (any) |
| `messages` | Messages | `time_s` (number (s)), `src` (text), `dst` (text), `group` (yes or no), `format` (choice), `data_format` (whole number), `text` (text), `crc_ok` (yes or no), `slot` (whole number), `sms` (yes or no) |
| `positions` | Positions | `time_s` (number (s)), `src` (text), `position` (position), `height_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `accuracy_m` (number (m)), `format` (choice), `comment` (text), `station_type` (choice), `data_source` (whole number) |
| `sites` | Sites | `time_s` (number (s)), `valid` (yes or no), `system` (whole number), `wacn` (whole number), `rfss` (whole number), `site` (whole number), `nac` (whole number), `control_channel` (text), `location` (whole number), `ran` (whole number), `country` (whole number), `network` (whole number), `base_station` (whole number), `channel` (whole number), `colour_code` (whole number), `location_area` (whole number), `main_carrier` (whole number), `band` (whole number), `main_carrier_hz` (frequency) |
| `grants` | Grants | `time_s` (number (s)), `target` (whole number), `group` (yes or no), `source_id` (whole number), `channel_number` (whole number), `channel` (text), `freq_hz` (frequency), `slot` (whole number), `emergency` (yes or no), `encrypted` (yes or no), `priority` (whole number), `update` (yes or no) |
| `talkgroups` | Talkgroups | `talkgroup` (whole number), `grants` (whole number), `calls` (whole number), `encrypted` (yes or no), `last_heard` (time) |
| `history` | Heard calls | `call` (text), `start_s` (number (s)), `src` (text), `dst` (text), `label` (text), `verdict` (choice), `played_frames` (whole number), `encrypted` (yes or no), `active` (yes or no) |
| `heard` | Heard stations | `src` (text), `calls` (whole number), `airtime` (duration (s)), `last_s` (number (s)), `mean_ber` (number), `emergency` (yes or no) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `eye` | Eye |  |  |
| `levels` | Level histogram |  |  |
| `spectrum` | Channel spectrum | Hz | dB |
| `snr_history` | SNR history |  | dB |
| `ber_history` | Bit error history |  |  |

## Map points

The stack puts points on a map: Positions.

## Events

| Kind | Label | Fields |
|---|---|---|
| `dvoice.call` | Call | `source` (text), `target` (text), `channel` (text), `text` (text), `urgency` (choice), `family` (choice), `private` (yes or no), `id` (whole number), `kind` (choice), `group` (yes or no), `end` (choice), `duration` (duration (s)) |
| `dvoice.message` | Message | `source` (text), `target` (text), `text` (text), `family` (choice), `format` (choice), `group` (yes or no) |
| `dvoice.position` | Position | `source` (text), `position` (position), `height_m` (number (m)), `speed_kt` (number (kt)), `course_deg` (number (°)), `text` (text), `family` (choice), `format` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `hold_current` | Hold current |  |  | Plays only the target of the call that plays now, or of the newest call. |
| `release` | Release |  |  | Lets the held target go. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `send` | Send a text | `text` (text, required) |  | Sends the radio header from the setting callsign to the setting your_call, and the text in the slow data of voice frames that carry AMBE silence: the library has no AMBE encoder. The transmission ends after every slow-data block went out once, or at halt. |
| `halt` | Halt |  |  | Ends the transmission after the frame or the burst on air, with the end sequence of the stack. |

## Typed interface

The stack is in the family `dvoice`, version 2.0. The C header `sdrkit/dvoice.h`, the registry record `registry/dvoice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `SharedFields`. The measurements that every digital voice stack publishes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | always | 1.0 | The channel level. |
| `noise_dbfs` | number (dBFS) | always | 1.0 | The noise floor. |
| `cnr_db` | number (dB) | optional | 1.0 | The carrier to noise ratio of the channel. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio on the symbols. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `deviation_hz` | frequency | always | 1.0 | The measured deviation. |
| `clock_ppm` | number (ppm) | always | 1.0 | The symbol clock error. |
| `ber` | number | always | 1.0 | The bit error rate. |
| `inverted` | yes or no | always | 1.0 | The signal is inverted. |
| `blocks_ok` | whole number | always | 1.0 | Blocks that passed their error check. |
| `blocks_bad` | whole number | always | 1.0 | Blocks that failed their error check. |
| `frames` | whole number | always | 1.0 | Frames whose identity field was read. |
| `syncs` | whole number | always | 1.0 | Frame syncs found. |
| `reports` | whole number | always | 1.0 | Reports written. |
| `verdict` | choice | always | 1.0 | Why the stack decodes or does not. |

### Result fields

Struct `DstarFields`. The result fields of D-STAR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `headers_ok` | whole number | always | 1.0 | Headers. |
| `dstar_voice_frames` | whole number | always | 1.0 | Voice frames. |
| `sync_inverted` | yes or no | always | 1.0 | Inverted sync. |

### Result fields

Struct `MonitorFields`. The result fields of the voice monitor.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `playing` | text | optional | 1.0 | The call that plays now: a row key of the `calls` table. |
| `held` | text | optional | 1.0 | The target held. |
| `codec` | text | optional | 1.0 | What the codec plug-ins say: a codec that is not installed or was turned off. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Result fields

Struct `DstarTxFields`. The result fields of the D-STAR sender.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.0 | Whether a transmission is under way. |
| `tx_freq_hz` | frequency | optional | 1.0 | The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit. |
| `tx_kind` | choice | optional | 1.0 | What the transmission sends, or the last one sent. |
| `tx_frames` | whole number | always | 1.0 | The voice frames of the transmission, or of the last one, that are written. |
| `tx_note` | text | always | 1.0 | What the sender puts in the voice frames. |

### Table `calls`

Struct `CallRow`. A row of the call log. The row key is the call id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | whole number | always | 1.0 | The id of a call. |
| `start_s` | number (s) | always | 1.0 | Signal time of the start of a call, seconds. |
| `duration` | duration (s) | always | 1.0 | How long a call lasted. |
| `slot` | whole number | always | 1.0 | The time slot, 0 when the stack has none. |
| `kind` | choice | always | 1.0 | What a transmission carries. |
| `src` | text | always | 1.0 | Who sends. |
| `dst` | text | always | 1.0 | Who is called. |
| `group` | yes or no | always | 1.0 | The target is a group. |
| `emergency` | yes or no | always | 1.0 | An emergency call. |
| `access_kind` | choice | optional | 1.0 | Which access code the call carried. |
| `access_code` | whole number | optional | 1.0 | The value of the access code. |
| `encryption` | choice | always | 1.0 | Encrypted or not. |
| `algorithm` | text | always | 1.0 | The name of the algorithm. |
| `alg_id` | whole number | optional | 1.0 | The algorithm id as sent. |
| `enc_key_id` | whole number | optional | 1.0 | The key id. |
| `alias` | text | always | 1.0 | The talker alias. |
| `text` | text | always | 1.0 | Text carried by a transmission. |
| `voice_frames` | whole number | always | 1.0 | Voice frames received. |
| `media_frames` | whole number | always | 1.0 | Voice frames passed to the media output. |
| `ber` | number | always | 1.0 | The bit error rate of a call. |
| `active` | yes or no | always | 1.0 | The call is on air. |
| `late_entry` | yes or no | always | 1.0 | The call started from an identity inside it. |
| `end_seen` | yes or no | always | 1.0 | The call ended by its terminator. |
| `signal_lost` | yes or no | always | 1.0 | The call ended after the hang time. |
| `carrier_hz` | frequency | always | 1.0 | The carrier frequency. |
| `end` | choice | always | 1.0 | Where a call is in its life. |

### Table `call_details`

Struct `CallDetailRow`. A fact of a call that only some stacks send. The row key is the call id, a slash and the detail key.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `call` | text | always | 1.0 | The call a row belongs to: a row key of the `calls` table. |
| `detail` | choice | always | 1.0 | Which fact a detail row holds. |
| `value` | any | always | 1.0 | The value of a detail. |

### Table `messages`

Struct `MessageRow`. A text message. The row key counts the messages.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time, seconds. |
| `src` | text | always | 1.0 | Who sends. |
| `dst` | text | always | 1.0 | Who is called. |
| `group` | yes or no | always | 1.0 | The target is a group. |
| `format` | choice | always | 1.0 | How a message was sent. |
| `data_format` | whole number | optional | 1.0 | The DMR short data format code. |
| `text` | text | always | 1.0 | Text carried by a transmission. |
| `crc_ok` | yes or no | always | 1.0 | The error check was good. |
| `slot` | whole number | always | 1.0 | The time slot, 0 when the stack has none. |
| `sms` | yes or no | always | 1.0 | An SMS. |

### Table `positions`

Struct `PositionRow`. A position. The row key counts the positions.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time, seconds. |
| `src` | text | always | 1.0 | Who sends. |
| `position` | position | always | 1.0 | The position, latitude first. |
| `height_m` | number (m) | optional | 1.0 | The height. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed. |
| `course_deg` | number (°) | optional | 1.0 | The course. |
| `accuracy_m` | number (m) | optional | 1.0 | The accuracy of a position. |
| `format` | choice | always | 1.0 | How a position was sent. |
| `comment` | text | always | 1.0 | The comment sent with a position. |
| `station_type` | choice | optional | 1.0 | The kind of station that sent a position. |
| `data_source` | whole number | optional | 1.0 | The data source code of a position. |

### Table `sites`

Struct `SiteRow`. The site and system identity of the stack. The row key is the stack id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time, seconds. |
| `valid` | yes or no | always | 1.0 | The identity is complete. |
| `system` | whole number | optional | 1.0 | The system id. |
| `wacn` | whole number | optional | 1.0 | The wide area communications network. |
| `rfss` | whole number | optional | 1.0 | The RF subsystem. |
| `site` | whole number | optional | 1.0 | The site. |
| `nac` | whole number | optional | 1.0 | The network access code. |
| `control_channel` | text | optional | 1.0 | The control channel. |
| `location` | whole number | optional | 1.0 | The location id. |
| `ran` | whole number | optional | 1.0 | The radio access number. |
| `country` | whole number | optional | 1.0 | The country code. |
| `network` | whole number | optional | 1.0 | The network code. |
| `base_station` | whole number | optional | 1.0 | The base station. |
| `channel` | whole number | optional | 1.0 | The channel of a site. |
| `colour_code` | whole number | optional | 1.0 | The colour code of a cell. |
| `location_area` | whole number | optional | 1.0 | The location area of a cell. |
| `main_carrier` | whole number | optional | 1.0 | The main carrier number of a cell. |
| `band` | whole number | optional | 1.0 | The frequency band of a cell. |
| `main_carrier_hz` | frequency | optional | 1.0 | The main carrier frequency of a cell. |

### Table `grants`

Struct `GrantRow`. A channel grant. The row key counts the grants.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time, seconds. |
| `target` | whole number | always | 1.0 | The talkgroup or radio granted. |
| `group` | yes or no | always | 1.0 | The target is a group. |
| `source_id` | whole number | always | 1.0 | The source of a grant request. |
| `channel_number` | whole number | always | 1.0 | The channel number of a grant as sent. |
| `channel` | text | always | 1.0 | The channel of a grant for people. |
| `freq_hz` | frequency | optional | 1.0 | The frequency of a grant. |
| `slot` | whole number | optional | 1.0 | The time slot, 0 when the stack has none. |
| `emergency` | yes or no | always | 1.0 | An emergency call. |
| `encrypted` | yes or no | always | 1.0 | The call is encrypted. |
| `priority` | whole number | always | 1.0 | The priority of a grant. |
| `update` | yes or no | always | 1.0 | An update of a running call. |

### Table `talkgroups`

Struct `TalkgroupRow`. A talkgroup heard, with its counters. The row key is the talkgroup.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `talkgroup` | whole number | always | 1.0 | A talkgroup. |
| `grants` | whole number | always | 1.0 | Grants to a talkgroup. |
| `calls` | whole number | always | 1.0 | Voice calls to a talkgroup. |
| `encrypted` | yes or no | always | 1.0 | The call is encrypted. |
| `last_heard` | time | optional | 1.0 | When it was last heard. |

### Table `history`

Struct `HistoryRow`. A call the monitor heard. The row key is the call id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `call` | text | always | 1.0 | The call a row belongs to: a row key of the `calls` table. |
| `start_s` | number (s) | always | 1.0 | Signal time of the start of a call, seconds. |
| `src` | text | always | 1.0 | Who sends. |
| `dst` | text | always | 1.0 | Who is called. |
| `label` | text | always | 1.0 | Who is called and its name. |
| `verdict` | choice | always | 1.0 | Why a call is or is not heard. |
| `played_frames` | whole number | always | 1.0 | Frames played. |
| `encrypted` | yes or no | always | 1.0 | The call is encrypted. |
| `active` | yes or no | always | 1.0 | The call is on air. |

### Table `heard`

Struct `HeardRow`. A source radio or callsign. The row key is the source.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `src` | text | always | 1.0 | Who sends. |
| `calls` | whole number | always | 1.0 | Calls heard. |
| `airtime` | duration (s) | always | 1.0 | Time on air. |
| `last_s` | number (s) | always | 1.0 | Signal time of the end of the last call, seconds. |
| `mean_ber` | number | always | 1.0 | The mean bit error rate. |
| `emergency` | yes or no | always | 1.0 | An emergency call. |

### Event `dvoice.call`

Struct `CallEvent`. A call started, ended, or was a single transmission. The field `end` is `active` when it started.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | Who sends. |
| `target` | text | always | 1.0 | Who is called. |
| `channel` | text | optional | 1.0 | The talkgroup of a group call, else the time slot. |
| `text` | text | optional | 1.0 | The text. |
| `urgency` | choice | always | 1.0 | How urgent it is. |
| `family` | choice | always | 1.0 | The family of the event. |
| `private` | yes or no | always | 1.0 | The content is encrypted. |
| `id` | whole number | always | 1.0 | The id of a call. |
| `kind` | choice | always | 1.0 | What a transmission carries. |
| `group` | yes or no | always | 1.0 | The target is a group. |
| `end` | choice | always | 1.0 | Where a call is in its life. |
| `duration` | duration (s) | optional | 1.0 | How long a call lasted. |

### Event `dvoice.message`

Struct `MessageEvent`. A text message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | Who sends. |
| `target` | text | always | 1.0 | Who is called. |
| `text` | text | always | 1.0 | The text. |
| `family` | choice | always | 1.0 | The family of the event. |
| `format` | choice | always | 1.0 | How a message was sent. |
| `group` | yes or no | always | 1.0 | The target is a group. |

### Event `dvoice.position`

Struct `PositionEvent`. A position.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | Who sends. |
| `position` | position | always | 1.0 | The position, latitude first. |
| `height_m` | number (m) | optional | 1.0 | The height. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed. |
| `course_deg` | number (°) | optional | 1.0 | The course. |
| `text` | text | optional | 1.0 | The text. |
| `family` | choice | always | 1.0 | The family of the event. |
| `format` | choice | always | 1.0 | How a position was sent. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_dstar_clear` | none | none |
| `hold_current` | `sdrkit_dstar_hold_current` | none | none |
| `release` | `sdrkit_dstar_release` | none | none |
| `remove` | `sdrkit_dstar_remove` | `sdrkit_dvoice_talkgroup_args` | none |
| `send` | `sdrkit_dstar_send` | `sdrkit_dstar_send_args` | none |
| `halt` | `sdrkit_dstar_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `squelch_pct` | `sdrkit_dstar_get_squelch_pct` | `sdrkit_dstar_set_squelch_pct` |
| `allow_only` | `sdrkit_dstar_get_allow_only` | `sdrkit_dstar_set_allow_only` |
| `allow` | `sdrkit_dstar_get_allow` | `sdrkit_dstar_set_allow` |
| `mute_list` | `sdrkit_dstar_get_mute_list` | `sdrkit_dstar_set_mute_list` |
| `priority` | `sdrkit_dstar_get_priority` | `sdrkit_dstar_set_priority` |
| `hold` | `sdrkit_dstar_get_hold` | `sdrkit_dstar_set_hold` |
| `labels` | `sdrkit_dstar_get_labels` | `sdrkit_dstar_set_labels` |
| `hold_timeout_s` | `sdrkit_dstar_get_hold_timeout_s` | `sdrkit_dstar_set_hold_timeout_s` |
| `hang_s` | `sdrkit_dstar_get_hang_s` | `sdrkit_dstar_set_hang_s` |
| `listen` | `sdrkit_dstar_get_listen` | `sdrkit_dstar_set_listen` |
| `callsign` | `sdrkit_dstar_get_callsign` | `sdrkit_dstar_set_callsign` |
| `suffix` | `sdrkit_dstar_get_suffix` | `sdrkit_dstar_set_suffix` |
| `your_call` | `sdrkit_dstar_get_your_call` | `sdrkit_dstar_set_your_call` |
| `rpt1` | `sdrkit_dstar_get_rpt1` | `sdrkit_dstar_set_rpt1` |
| `rpt2` | `sdrkit_dstar_get_rpt2` | `sdrkit_dstar_set_rpt2` |
| `via_repeater` | `sdrkit_dstar_get_via_repeater` | `sdrkit_dstar_set_via_repeater` |
| `tx_split` | `sdrkit_dstar_get_tx_split` | `sdrkit_dstar_set_tx_split` |

## Try it

```sh
sdrkit run dstar --source synth:dstar
sdrkit run dstar --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/dvoice/dvoice_dstar_96k.sigmf-meta`
<!-- generated: end -->

## Transmit

The stack sends a D-STAR DV transmission with a text in the slow data, by the JARL D-STAR specification. The library has
no AMBE encoder: the voice frames carry AMBE silence, and the stack sends no speech. The engine setting
`transmit_allowed` is the only switch; the stack adds no other gate.

Commands:

- `send` sends a text (up to 20 printable ASCII characters) from the setting `callsign` to the setting `your_call`.
- `halt` ends the transmission after the frame on air, with the end pattern.

Settings: `callsign` (MY, up to 8 characters of A to Z, 0 to 9, / and space; empty by default), `suffix` (up to 4
characters), `your_call` (`CQCQCQ`), `rpt1` and `rpt2` (`DIRECT`; for a repeater, the repeater with its module letter in
position 8, and with `G` for the gateway), `via_repeater` (bit 6 of flag byte 1) and `tx_split` (transmit offset). Text
is upper case. A setting change applies to the next transmission. While `callsign` is empty, `send` is refused with an
error that names the setting.

The signal:

- GMSK at 4800 bit/s, BT 0.5, deviation ±1.2 kHz (measured: 1200 Hz at most). The 99 % power bandwidth is 4945 Hz
  (measured), within the 6.25 kHz channel.
- A transmission is the bit sync (64 bits `1010…`), the frame sync, the radio header (41 bytes with the CRC, the rate 1/2
  convolutional code, the interleave and the scrambler), the voice frames, then the end pattern.
- A voice frame of 20 ms is 72 voice bits and 24 slow-data bits. Frame 0 of every 21 carries the data sync; the others
  carry the slow-data blocks in turn, 3 bytes a frame, XORed with `70 4F 93`: the text (4 blocks of 5 characters, type
  0x40 to 0x43) and the header copy (9 blocks, type 0x5n), so a receiver that missed the header takes it from the slow
  data. The transmission lasts the groups of 21 frames that send every block once: 42 frames (0.84 s).
- The voice bits of every frame are the AMBE silence frame `9E 8D 32 88 26 1A 3F 61 E8` (the 72 channel bits as sent,
  the first bit most significant). This value is not confirmed against a radio or the vocoder documentation.
- The carrier rises and falls with a 5 ms raised cosine. The magnitude is 0.9 of full scale, constant over the burst.
- The output has the rate of the session; its samples carry the signal at 0 Hz, and its centre is the transmit frequency
  (the listening frequency plus `tx_split`). Every transmission ends with an end mark. On a live radio whose host times
  bursts, the first sample states its time on air.

Results: `transmitting`, `tx_freq_hz` (while transmitting), `tx_kind` (`text`), `tx_frames` (the voice frames sent in
the transmission on air, or in the last one) and `tx_note` ("AMBE encoder not installed: identities and data go on the
air, voice frames are silence").

The receiver of the stack decodes what the stack sends: MY, the suffix, YOUR, RPT1, RPT2, the flags, the text and every
voice frame as the silence frame, byte for byte; with the start of the transmission lost, the header from the slow data.

Not sent: speech (no AMBE encoder), pre-coded voice frames from the host, a GPS position, the code squelch, listen before
talk. The receive audio is muted while the stack sends.
