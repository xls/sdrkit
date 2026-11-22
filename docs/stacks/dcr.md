<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# DCR (Japan digital convenience radio)

Japanese digital convenience radio (four-level FSK): calls by user code, call signs, manufacturer messages and data blocks, on one channel or every plan channel in the span; voice frames are counted, not played.

Stack id: `dcr`. Group: Digital voice.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 150 MHz to 470 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 12.5 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 200 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:dcr` prints this table with every option.

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
| `plan` | Channel plan | text | empty |  | The channels to watch: entries separated by ";", each a label, "=" and the frequency in Hz, for example "351 MHz ch 15=351287500". At most 512 entries. The host fills it from the DCR plan file. Empty: the tuned frequency only. |
| `scan` | Watch every plan channel in the span | yes or no | true |  | Decode every plan channel inside 42 % of the sample rate around the centre that carries a signal. It needs two plan channels in the span and 100 ksps or more. |
| `max_channels` | Channels decoded at once | whole number | 8 | 1 to 8 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_content` | Content | choice | `user_code_1` | `user_code_1` (User code test 1), `user_code_2` (User code test 2), `user_code_3` (User code test 3), `privacy` (Private communication test), `sweep` (Eight calls), `messages` (Messages and data) | A test signal of chapter 7 of the standard, eight calls, or manufacturer messages, data frames and a call sign. |
| `gen_deviation_scale` | Deviation scale | number | 1 | 0.5 to 1.5 | The outer deviation over 945 Hz. |

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
| `dcr_frames` | Frames | whole number |
| `sacch_ok` | SACCH good | whole number |
| `sacch_bad` | SACCH bad | whole number |
| `rich_bad` | RICH parity bad | whole number |
| `pich_ok` | Call signs read | whole number |
| `data_ok` | Data blocks good | whole number |
| `data_bad` | Data blocks bad | whole number |
| `voice_frames` | Voice frames | whole number |
| `calls` | Calls | whole number |
| `ber_pct` | Bit errors | number (%) |
| `last_user_code` | Last user code | whole number |
| `last_call_sign` | Last call sign | text |
| `channel` | Plan channel | text |
| `scan_channels` | Plan channels in the span | whole number |
| `scan_decoders` | Decoders running | whole number |
| `scan_decoding` | Decoders decoding | whole number |
| `note` | Note | text |
| `playing` | Playing call | text |
| `held` | Held target | text |
| `codec` | Codec | text |
| `dropped` | Dropped | whole number |

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
| `events` | Events | `time_s` (number (s)), `kind` (choice), `identity` (text), `text` (text), `channel` (text) |
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

## Typed interface

The stack is in the family `dvoice`, version 1.5. The C header `sdrkit/dvoice.h`, the registry record `registry/dvoice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `DcrFields`. The result fields of DCR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dcr_frames` | whole number | always | 1.0 | Frames read after a sync. |
| `sacch_ok` | whole number | always | 1.0 | SACCH units that passed their CRC. |
| `sacch_bad` | whole number | always | 1.0 | SACCH units that failed their CRC. |
| `rich_bad` | whole number | always | 1.0 | Frames whose RICH parity failed. |
| `pich_ok` | whole number | always | 1.0 | Call signs read from a PICH. |
| `data_ok` | whole number | always | 1.0 | Data blocks that passed their CRC. |
| `data_bad` | whole number | always | 1.0 | Data blocks that failed their CRC. |
| `voice_frames` | whole number | always | 1.0 | Voice frames of 20 ms in frames whose SACCH decoded. |
| `calls` | whole number | always | 1.0 | Calls opened. |
| `ber_pct` | number (%) | always | 1.0 | The bits that the Viterbi decoder repaired, of the coded bits of the blocks that passed their CRC. |
| `last_user_code` | whole number | optional | 1.0 | The user code of the last message that counted, 0 to 511. |
| `last_call_sign` | text | optional | 1.0 | The last call sign read, 9 digits. |
| `channel` | text | optional | 1.0 | The plan channel of the tuned frequency, or "not on the plan". No value without a plan. |
| `scan_channels` | whole number | always | 1.0 | The plan channels in the span of a scan. |
| `scan_decoders` | whole number | always | 1.0 | The decoders of scanned channels that run. |
| `scan_decoding` | whole number | always | 1.0 | The decoders of scanned channels that decode. |
| `note` | text | always | 1.0 | What the stack decodes of the voice. |

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

### Table `events`

Struct `DcrEventRow`. A row of the DCR list: a call, a call end, a call sign, a data block or a message. The row key counts the rows.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time, seconds. |
| `kind` | choice | always | 1.0 | What the row reports. |
| `identity` | text | always | 1.0 | Who: "UC n", a call sign, "TCH1" or "TCH2", after the plan label of a scanned channel. |
| `text` | text | always | 1.0 | The text. |
| `channel` | text | optional | 1.0 | The plan label of a scanned channel. |

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
| `clear` | `sdrkit_dcr_clear` | none | none |
| `hold_current` | `sdrkit_dcr_hold_current` | none | none |
| `release` | `sdrkit_dcr_release` | none | none |
| `remove` | `sdrkit_dcr_remove` | `sdrkit_dvoice_talkgroup_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `squelch_pct` | `sdrkit_dcr_get_squelch_pct` | `sdrkit_dcr_set_squelch_pct` |
| `allow_only` | `sdrkit_dcr_get_allow_only` | `sdrkit_dcr_set_allow_only` |
| `allow` | `sdrkit_dcr_get_allow` | `sdrkit_dcr_set_allow` |
| `mute_list` | `sdrkit_dcr_get_mute_list` | `sdrkit_dcr_set_mute_list` |
| `priority` | `sdrkit_dcr_get_priority` | `sdrkit_dcr_set_priority` |
| `hold` | `sdrkit_dcr_get_hold` | `sdrkit_dcr_set_hold` |
| `labels` | `sdrkit_dcr_get_labels` | `sdrkit_dcr_set_labels` |
| `hold_timeout_s` | `sdrkit_dcr_get_hold_timeout_s` | `sdrkit_dcr_set_hold_timeout_s` |
| `hang_s` | `sdrkit_dcr_get_hang_s` | `sdrkit_dcr_set_hang_s` |
| `listen` | `sdrkit_dcr_get_listen` | `sdrkit_dcr_set_listen` |
| `plan` | `sdrkit_dcr_get_plan` | `sdrkit_dcr_set_plan` |
| `scan` | `sdrkit_dcr_get_scan` | `sdrkit_dcr_set_scan` |
| `max_channels` | `sdrkit_dcr_get_max_channels` | `sdrkit_dcr_set_max_channels` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_content` | `sdrkit_dcr_gen_get_content` | `sdrkit_dcr_gen_set_content` |
| `gen_deviation_scale` | `sdrkit_dcr_gen_get_deviation_scale` | `sdrkit_dcr_gen_set_deviation_scale` |

## Try it

```sh
sdrkit run dcr --source synth:dcr
sdrkit run dcr --source <driver>:<address> --freq 0
```
<!-- generated: end -->
