<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FreeDV

FreeDV on HF (upper sideband): the 1600, 700D and 700E waveforms with their callsign text; Codec 2 speech through a plug-in. RADE is not received. Transmits voice from the microphone on 1600 (Codec 2 1300), 700D or 700E (Codec 2 700C) with the callsign text.

Stack id: `freedv`. Group: Digital voice.

Signal types: FreeDV.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 96 kHz |
| Signal bandwidth | 3 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `auto` | Automatic (700D, 700E and 1600) | 0 Hz |
| `1600` | FreeDV 1600 | 0 Hz |
| `700D` | FreeDV 700D | 0 Hz |
| `700E` | FreeDV 700E | 0 Hz |
| `rade` | FreeDV RADE (not received) | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:freedv` prints this table with every option.

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
| `callsign` | Callsign text | text | empty |  | Sent in the text channel with a carriage return after it, again and again: up to 40 printable ASCII characters, normally the call sign of the station. Nothing is sent while it is empty. |
| `tx_clip` | Clipper | yes or no | false |  | The drive gain and a limit at half of full scale before the level, with the transmit filter after it: more average power for a little distortion of the carriers. It works on 1600, 700D and 700E. |
| `tx_filter` | Transmit filter | yes or no | false |  | A band-pass around 1500 Hz: about 725 Hz to 2275 Hz for 1600, 750 Hz to 2250 Hz for 700D, 500 Hz to 2500 Hz for 700E. The clipper turns it on as well. |
| `tx_split` | Transmit offset | frequency | 0 Hz | -10 GHz to 10 GHz | The transmit frequency less the listening frequency: 0 for simplex, the shift of a repeater for duplex. |
| `tx_timeout` | Time-out | duration (s) | 180 s | 1 to 3600 | A voice stream ends after this time on air. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_clip` | Clipper | yes or no | false |  | The 700D and 700E test signals pass the clipper and the transmit filter. |

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
| `submode` | Submode | text |
| `sync_700d` | 700D sync | text |
| `sync_700e` | 700E sync | text |
| `sync_1600` | 1600 sync | text |
| `confirmed` | Confirmed | yes or no |
| `freq_offset_hz` | Frequency offset | number (Hz) |
| `clock_offset_ppm` | Clock offset | number (ppm) |
| `timing_metric` | Timing metric | number |
| `uw_ber_pct` | Unique word errors | number (%) |
| `ldpc_failed_frames` | LDPC failed | whole number |
| `freedv_frames` | Frames | whole number |
| `speech_frames` | Speech frames | whole number |
| `callsign` | Callsign | text |
| `playing` | Playing call | text |
| `held` | Held target | text |
| `codec` | Codec | text |
| `dropped` | Dropped | whole number |
| `transmitting` | Transmitting | yes or no |
| `tx_freq_hz` | Transmit frequency | frequency |
| `tx_kind` | Sending | choice |
| `tx_frames` | Frames sent | whole number |

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
| `ptt` | Push to talk | `on` (yes or no, required) |  | With on true, sends the microphone input on the waveform of the submode: Codec 2 1300 on 1600 in the submode 1600, else Codec 2 700C on 700D (700E in the submode 700E); with the setting callsign in the text channel, until ptt with on false, halt or the time-out. The submode RADE does not send. |
| `halt` | Halt |  |  | Ends the transmission after the frame or the burst on air, with the end sequence of the stack. |

## Typed interface

The stack is in the family `dvoice`, version 2.1. The C header `sdrkit/dvoice.h`, the registry record `registry/dvoice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Voice calls (`voice`). Media: sound (voice).

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

Struct `FreedvFields`. The result fields of FreeDV.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `submode` | text | always | 1.0 | The active waveform: "700D", "700E", "1600", or empty when none is. |
| `sync_700d` | text | always | 1.0 | The sync state of the 700D waveform: "search", "trial" or "synced"; empty when it does not run. |
| `sync_700e` | text | always | 1.0 | The sync state of the 700E waveform: "search", "trial" or "synced"; empty when it does not run. |
| `sync_1600` | text | always | 1.0 | The sync state of the 1600 waveform: "search" or "synced"; empty when it does not run. |
| `confirmed` | yes or no | always | 1.0 | The active waveform held sync at 3 dB or more for 1.5 s. |
| `freq_offset_hz` | number (Hz) | always | 1.0 | The frequency estimate of the active OFDM waveform, relative to the 1500 Hz centre. |
| `clock_offset_ppm` | number (ppm) | always | 1.0 | The sample clock offset of the active OFDM waveform. |
| `timing_metric` | number | always | 1.0 | The last timing metric of the active OFDM waveform (0 to about 1). |
| `uw_ber_pct` | number (%) | always | 1.0 | Unique word bit errors of the bits of the unique word, since the call started. |
| `ldpc_failed_frames` | whole number | always | 1.0 | Frames whose LDPC decoder did not satisfy every check, since the call started. |
| `freedv_frames` | whole number | always | 1.0 | Modem frames decoded with sync. |
| `speech_frames` | whole number | always | 1.0 | Codec frames sent to the speech output. |
| `callsign` | text | always | 1.0 | The text line in progress, or the last finished line. |

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

Struct `FreedvTxFields`. The result fields of the FreeDV sender.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.0 | Whether a transmission is under way. |
| `tx_freq_hz` | frequency | optional | 1.0 | The transmit frequency (the suppressed carrier): the listening frequency plus the transmit offset. No value while the stack does not transmit. |
| `tx_kind` | choice | optional | 1.0 | What the transmission sends, or the last one sent. |
| `tx_frames` | whole number | always | 1.0 | The modem frames of the transmission, or of the last one, that are written. |

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

The column `alias` names a row.

### Table `call_details`

Struct `CallDetailRow`. A fact of a call that only some stacks send. The row key is the call id, a slash and the detail key.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `call` | text | always | 1.0 | The call a row belongs to: a row key of the `calls` table. |
| `detail` | choice | always | 1.0 | Which fact a detail row holds. |
| `value` | any | always | 1.0 | The value of a detail. |

The column `call` names a row.

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

The column `src` names a row.

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

The column `src` names a row. Place of a row: the column `position`. Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.

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

The column `call` names a row.

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

The column `src` names a row.

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
| `clear` | `sdrkit_freedv_clear` | none | none |
| `hold_current` | `sdrkit_freedv_hold_current` | none | none |
| `release` | `sdrkit_freedv_release` | none | none |
| `remove` | `sdrkit_freedv_remove` | `sdrkit_dvoice_talkgroup_args` | none |
| `ptt` | `sdrkit_freedv_ptt` | `sdrkit_dvoice_ptt_args` | none |
| `halt` | `sdrkit_freedv_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `squelch_pct` | `sdrkit_freedv_get_squelch_pct` | `sdrkit_freedv_set_squelch_pct` |
| `allow_only` | `sdrkit_freedv_get_allow_only` | `sdrkit_freedv_set_allow_only` |
| `allow` | `sdrkit_freedv_get_allow` | `sdrkit_freedv_set_allow` |
| `mute_list` | `sdrkit_freedv_get_mute_list` | `sdrkit_freedv_set_mute_list` |
| `priority` | `sdrkit_freedv_get_priority` | `sdrkit_freedv_set_priority` |
| `hold` | `sdrkit_freedv_get_hold` | `sdrkit_freedv_set_hold` |
| `labels` | `sdrkit_freedv_get_labels` | `sdrkit_freedv_set_labels` |
| `hold_timeout_s` | `sdrkit_freedv_get_hold_timeout_s` | `sdrkit_freedv_set_hold_timeout_s` |
| `hang_s` | `sdrkit_freedv_get_hang_s` | `sdrkit_freedv_set_hang_s` |
| `listen` | `sdrkit_freedv_get_listen` | `sdrkit_freedv_set_listen` |
| `callsign` | `sdrkit_freedv_get_callsign` | `sdrkit_freedv_set_callsign` |
| `tx_clip` | `sdrkit_freedv_get_tx_clip` | `sdrkit_freedv_set_tx_clip` |
| `tx_filter` | `sdrkit_freedv_get_tx_filter` | `sdrkit_freedv_set_tx_filter` |
| `tx_split` | `sdrkit_freedv_get_tx_split` | `sdrkit_freedv_set_tx_split` |
| `tx_timeout` | `sdrkit_freedv_get_tx_timeout` | `sdrkit_freedv_set_tx_timeout` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_clip` | `sdrkit_freedv_gen_get_clip` | `sdrkit_freedv_gen_set_clip` |

## Try it

```sh
sdrkit run freedv --source synth:freedv
sdrkit run freedv --source synth:freedv --submode auto
sdrkit run freedv --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Description

FreeDV is digital voice for HF on the upper sideband. The stack receives three waveforms and gives their Codec 2 speech
frames to a codec plug-in:

| Waveform | Carriers | Modem frame | Error correction | Speech | Sync range |
|---|---|---|---|---|---|
| 1600 | 16 DQPSK carriers of 75 Hz and a BPSK pilot | 40 ms | Golay (23,12) on 12 of 52 bits | Codec 2 1300 | about ±8 Hz |
| 700D | 17 QPSK carriers of 55.6 Hz, 1000 Hz wide (944 Hz between the outer data carriers) | 160 ms | LDPC (224,112) | Codec 2 700C, 4 frames | −60 Hz to +59 Hz |
| 700E | 21 QPSK carriers of 71.4 Hz, 1500 Hz wide | 80 ms | LDPC (112,56) | Codec 2 700C, 2 frames | −60 Hz to +59 Hz |

700D and 700E are OFDM. Each modem frame is one row of BPSK pilots and 7 (700D) or 3 (700E) rows of QPSK symbols. The rows
hold a unique word of 10 or 12 zero bits, 4 or 2 text bits without error correction, and one LDPC code word whose symbols
are interleaved inside the frame. The cyclic prefix is 2 ms in 700D and 6 ms in 700E. The demodulator component of each
waveform does these steps:

1. Search: two pilot rows one frame apart, over one frame of timing and the offsets −40, 0 and +40 Hz, then the offset in
   steps of 1 Hz. A timing metric above 0.30 starts the trial.
2. Each frame: fine timing, frequency tracking from the turn between two pilot rows, and the channel of each carrier from the
   pilot rows. 700D averages three carriers and four pilot rows once it is synced.
3. Soft decisions, de-interleaving and the sum-product LDPC decoder (at most 100 iterations).
4. Sync from the unique word. 700D: a frame with more than 2 errors is bad; 2 bad frames in the trial go back to the search,
   4 good frames in a row sync, 7 bad frames in a row lose sync. 700E: more than 3 errors is bad; the first frame of the
   trial decides, 6 bad frames in a row lose sync.

The stack trusts an OFDM waveform after it held sync at an SNR of 3 dB or more (in 3000 Hz) for 1.5 s. A frame without sync
or below 3 dB removes the trust and the text at once. Only the active waveform gives speech, text and a call: the first trusted
one in the order 700D, 700E, 1600. It stays active until it has not been trusted for 2 s. The call ends after 1 s. A
channel below −100 dBFS is silence.

The text channel of every waveform is PSK31 varicode at 25 bit/s (1600: the last bit of each frame). A carriage return, a
line feed or a NUL ends a line, and so do 80 characters. `callsign` shows the line in progress, or the last line. The call
takes its source from the first line that was received whole (1600: or twice in a row).

The 1600 waveform shapes each carrier's symbols with a root raised cosine of roll-off 0.5 over 6 symbols, and the pilot
has twice the power of a data carrier (both from the "FreeDV Specification" for the older 1450 bit/s mode; unconfirmed for
1600). The 1600 demodulator passes each carrier through the matched root raised cosine filter (the same taps as the
modulator) and reads it once a symbol. A Gardner detector on the pilot tracks the symbol timing. A timing search finds the
symbol timing when the receiver starts anywhere in a symbol: for each timing in steps of 8 samples, the pilot's phase step
between two symbols, its sign turned on every other symbol (the pilot steps by π and by 0 in turn), adds to a score that
forgets with 0.9 a symbol. When the middle of the best run of timings is 24 samples or more from the present timing, holds
together (0.6 of its weight) and scores 1.2 times the present one, the timing jumps there and the frequency correction
starts again. A symbol whose pilot is 10 dB below its mean ends no frame. A 1600 frame is good with at most one Golay
repair; the codec bits go out when the 63rd bit of the frame has come, the text bit with the next.

Not received:

- RADE. Its decoder is a trained network; the stack has no weights and lists the submode only.
- A 700D signal whose interleaver covers more than one modem frame. The FreeDV User Manual calls interleaving in 700D
  optional; no public document gives the layout of a deeper one.
- The text channel with its own LDPC protection ("reliable text").

Speech frames of 700D and 700E name no codec mode: the media format has no value for Codec 2 700C.

Standard: FreeDV project documentation ("FreeDV User Manual", "FreeDV Specification");
P. Martinez, "PSK31: a new radio-teletype mode" for the varicode.

## Submodes: notes

| Id | Listens for | Test signal |
|---|---|---|
| `auto` (also when none is named) | 700D, 700E and 1600 at the same time | 1600 |
| `1600` | 1600 | 1600 |
| `700D` | 700D | 700D, test-frame data bits, text `N0CALL` |
| `700E` | 700E | 700E, test-frame data bits, text `N0CALL` |
| `rade` | nothing | 1600 |

`gen_clip=true` puts the 700D and 700E test signals through the clipper and the transmit filter. Senders have both as
options that are off by default; the stack receives both forms.

## Tuning: notes

Tune to the suppressed carrier of the upper sideband. The centre of all three waveforms is 1500 Hz above it. `offset_hz` and
`freq_offset_hz` are relative to that centre.

## Results: notes

| Key | Meaning |
|---|---|
| `submode` | The active waveform, or empty |
| `sync_700d`, `sync_700e` | `search`, `trial` or `synced`; empty when the waveform does not run |
| `sync_1600` | `search` or `synced`; empty when the waveform does not run |
| `confirmed` | The active waveform is trusted |
| `snr_db` | 700D and 700E: the SNR in 3000 Hz from the scatter of the symbols; −5 dB while searching |
| `freq_offset_hz`, `clock_offset_ppm`, `timing_metric` | Of the active OFDM waveform |
| `uw_ber_pct`, `ldpc_failed_frames` | Since the call started |
| `freedv_frames` | Modem frames demodulated |
| `speech_frames` | Codec frames given to the speech output |

## Measured

White noise, SNR in 3000 Hz, test signal of 10 s, seeds 1 to 10. A frame counts when its data bits are the sent bits; the
frames before sync count as lost.

| Waveform | SNR | Frames decoded |
|---|---|---|
| 700D | −2 dB | 85.7 % |
| 700D | 3 dB | 98.9 % |
| 700E | +1 dB | 91.0 % |
| 700E | 3 dB | 97.9 % |

The stack gives speech from about 3 dB, because of the 3 dB rule of the trust. At 1 dB both waveforms decode their frames
and the stack stays silent. No multipath case is tested: no public document gives a channel with a pass mark.

Published limits that are not tested here: drift 0.2 Hz/s and a sample clock error of 1000 ppm (700D); 700D needs about
2 dB on a multipath channel and fails above about 1 Hz of fading; 700E is made for 4 Hz of Doppler spread and 6 ms of delay
and needs about 3 dB more power than 700D. A sample clock error of 100 ppm is tested.

Processor share of a whole receiver at 96 ksps on its test signal (release build, this development machine, the
benchmarks `freedv.700d_receiver_96k` and `freedv.700e_receiver_96k`): 0.8 % of one core for 700D and 1.6 % for 700E.

## What no public document confirms

The public documents give the number of carriers, the symbol period and prefix of 700D, the prefix of 700E, the frame
periods, the code sizes, the 10-bit unique word of 700D, the text rate, the ±60 Hz sync range and the threshold SNRs (−2 dB
for 700D, +1 dB for 700E). The stack takes these from the waveform specification alone:

- The carrier spacing, the edge carriers of 700D, the symbols per frame and the 12-bit unique word of 700E.
- The pilot values, the places of the unique word and the text symbols, both parity-check matrices and the interleaver
  constants.
- The sync counters, the unique word error limits, the timing metric threshold and the tracking gain.
- The clipper gains and limits and the cut-offs of the transmit filter (all three waveforms).
- For 1600 (no public document gives its frame bit by bit; the documents describe the older 1450 bit/s mode): which 12
  codec bits the Golay word protects, the bit order inside the Golay word (generator 0xC75 here) and the place of the
  parity bits in the frame; the order of the bits over the carriers (here from the lowest carrier up) and inside a pair;
  the map from bit pairs to phase steps (here Gray: 00 0, 01 π/2, 11 π, 10 3π/2); the frame sync by the pilot alone (no
  sync bit); the roll-off 0.5 and the pilot at twice the power for 1600; the full varicode table beyond the entries a
  document prints; the absence of scrambling; and the number of data carriers (a public table says 14, the bit rate needs
  16).
- Compatibility of the 1600 sender and receiver with other FreeDV 1600 stations is not proven. Only a recording of a real
  signal can show it.
- The 28-bit layout of a Codec 2 700C frame and the number of codec frames in a modem frame.
- The raw bit rates: a public table gives 1900 bit/s for 700D and 3000 bit/s for 700E and does not say what they count; the
  frames here carry 1487.5 bit/s and 1575 bit/s.

## Transmit

The stack sends 1600, 700D and 700E voice from the microphone. The engine setting `transmit_allowed` is the only switch;
the stack adds no other gate.

Commands:

- `ptt` with `on` true starts a transmission; `ptt` with `on` false ends it after the frame on air. It also ends with
  `halt` and at the time-out (`tx_timeout`).
- `halt` ends the transmission after the frame on air.

The waveform follows the submode: `1600` sends 1600; `700E` sends 700E; `auto` and `700D` send 700D. In the submode
`rade`, `ptt` is refused: the stack has no sender for it.

Settings: `callsign` (the callsign text, up to 40 printable ASCII characters; empty by default), `tx_clip` (the clipper,
off), `tx_filter` (the transmit filter, off), `tx_split` (transmit offset) and `tx_timeout`. A setting change applies to
the next transmission. While `callsign` is empty, `ptt` is refused with an error that names the setting.

The signal of 1600:

- The first symbol (every carrier at its phase), then frames of 40 ms as the receiver of the stack decodes them: two
  symbols of 16 DQPSK carriers (two bits each, Gray coded) and the pilot, which steps by π on the first symbol of a frame
  and stays on the second. A frame is the 52 bits of one Codec 2 1300 frame, the 11 Golay (23,12) parity bits of its first
  12, and one text bit. The text bits carry `callsign` and a carriage return, again and again, in the PSK31 varicode
  (25 bit/s). Each carrier's symbols are shaped by a root raised cosine of roll-off 0.5 over 6 symbols (unconfirmed for
  1600); the pilot has twice the power of a data carrier. There is no scrambling.
- The first two frames carry coded silence while the microphone audio arrives; the later frames carry the speech.
- Push-to-talk off, `halt` and the time-out end the transmission after the frame on air. Six symbol slots follow in which
  the shaped pulses of the last symbols end (120 ms), with the fall over their end; no frame follows.
- The transmit chain scales the modulator output by 0.2 (its peak near half of full scale). The clipper multiplies by 1.5
  and limits the magnitude to half of full scale; the transmit filter (a band-pass around 1500 Hz, −6 dB at ±775 Hz)
  follows; then a gain of 0.8 and the limit again. Without the clipper the limit at 0.9 acts on about 0.2 % of the
  samples. The shaped signal already lies inside the filter's band.
- The carriers of the modem sit 900 Hz to 2100 Hz above the suppressed carrier.

The signal of 700D and 700E:

- The modem frames of the waveform, as the receiver of the stack decodes them. Each frame carries one LDPC code word of 4
  (700D) or 2 (700E) Codec 2 700C frames, the unique word, and 4 or 2 text bits. The text bits carry `callsign` and a
  carriage return, again and again, in the PSK31 varicode (25 bit/s).
- The first modem frame carries coded silence while the microphone audio arrives; the later frames carry the speech.
- Push-to-talk off, `halt` and the time-out end the transmission after the modem frame on air. One pilot row follows, so
  the receiver has the pilots after the last frame.
- The transmit chain scales the modulator output so that its peak is near half of full scale. The clipper multiplies by
  2.5 (700D) or 3.0 (700E) and limits the magnitude to half of full scale; the transmit filter (a band-pass around
  1500 Hz, −6 dB at ±750 Hz for 700D and ±1000 Hz for 700E) follows; then a gain of 0.8 and the limit again. Without the
  clipper the limit at 0.9 acts on about 0.3 % of the samples of 700D and 0.06 % of 700E.
- The carriers of the modem sit 1000 Hz to 2000 Hz (700D) or 714 Hz to 2286 Hz (700E) above the suppressed carrier.

All waveforms: the clipper always has the filter after it. The nominal peak of the chain becomes 0.9 of full scale; no
sample is above 0.9. The carrier rises and falls with a 5 ms raised cosine. The output has the rate of the session. Its
samples carry the upper sideband: 0 Hz is the suppressed carrier at the transmit frequency (the listening frequency plus
`tx_split`). Every transmission ends with an end mark. On a live radio whose host times bursts, the first sample states
its time on air.

Measured (the tests of the sender, 99 % power bandwidth of the 8 kHz sideband; for 1600 also the ratio of the peak power
to the mean power):

| Waveform | Chain | Band | Width |
|---|---|---|---|
| 1600 | plain | 875 Hz to 2125 Hz | 1258 Hz, peak to mean 8.0 dB |
| 1600 | filter | 875 Hz to 2125 Hz | 1258 Hz, peak to mean 8.0 dB |
| 1600 | clipper and filter | 875 Hz to 2125 Hz | 1258 Hz, peak to mean 4.8 dB |
| 700D | plain | 781 Hz to 2164 Hz | 1391 Hz |
| 700D | filter | 977 Hz to 2031 Hz | 1062 Hz |
| 700D | clipper and filter | 891 Hz to 2109 Hz | 1227 Hz |
| 700E | plain | 641 Hz to 2305 Hz | 1672 Hz |
| 700E | filter | 750 Hz to 2250 Hz | 1508 Hz |
| 700E | clipper and filter | 625 Hz to 2391 Hz | 1773 Hz |

The receiver of the stack decodes the callsign text and every Codec 2 frame, bit for bit, from the moment it trusts the
waveform to the last frame, with the chain plain and with the clipper and filter: 700D and 700E after about 1.5 s; 1600,
5 Hz off, within 0.6 s from any start in a symbol, after at most 3 damaged frames while it finds the symbol timing. 1600 in
white noise (SNR in 3000 Hz, seeds 1 to 10, 250 frames each, from the first frame received): 97.0 % of the frames bit for
bit at 9 dB and 99.7 % at 12 dB; with the clipper and the filter, at the same peak and noise as 12 dB without (15.2 dB
against its mean power), 99.6 %. No public document gives an SNR or a peak-to-mean ratio for 1600.

Speech comes in at the microphone input of the component `freedv.transmit` (`mic`, mono audio of any rate, converted to
8 kHz). The encoder needs the Codec 2 tables of its rate. 700C: `newamp1vq_cb_0.f32`, `newamp1vq_cb_1.f32`,
`newamp1_energy_cb_0.f32` and `nlp_fir_0.f32`. 1300: `lsp_cb_0.f32` to `lsp_cb_9.f32` and `nlp_fir_0.f32`. The stack reads
them from the folder `codec2` of the host's storage and checks each file by its SHA-256; a host can also hand them over
with `FreedvTransmit::set_codebooks`. Without them `ptt` is refused with the name of the missing table.

Results: `transmitting`, `tx_freq_hz` (while transmitting), `tx_kind` (`1600`, `700D` or `700E`) and `tx_frames` (the
frames sent in the transmission on air, or in the last one).

Not sent: RADE, the lower sideband, an audio centre other than 1500 Hz, a level setting, pre-coded speech frames from the
host, and the text channel with its own LDPC protection. The receive audio is muted while the stack sends.
