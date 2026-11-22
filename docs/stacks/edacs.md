<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# EDACS trunking

EDACS control channels: calls, talkgroups and the site, a power scan of the voice channels, and following calls.

Stack id: `edacs`. Group: Trunking.

Signal types: EDACS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 856 MHz |
| Tuning range | 29.7 MHz to 960 MHz |
| Sample rate | 480 kHz |
| Smallest sample rate | 307.2 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 100 kHz |
| Radio filter | 300 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:edacs` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `extended` | Extended Addressing | yes or no | false |  | Extended Addressing instead of Standard / Networked. |
| `esk` | ESK | yes or no | false |  | Remove the ESK byte A0 some systems XOR into both messages. |
| `lcn_table` | Voice channel table | text | empty |  | One line per channel, MHz: "frequency" (LCN = line number) or "LCN frequency". LCN 1 to 64. |
| `follow` | Follow calls | yes or no | false |  | Ask the radio to listen to the voice channel of each call. |
| `follow_carriers` | Also follow carriers | yes or no | false |  | Also follow carriers that no message announced. |
| `follow_groups` | Follow groups | text | empty |  | Talkgroups to follow ("1201 2000-2099"). Empty follows every call. A bad item is refused. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `locked` | Locked | yes or no |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `polarity` | Data polarity | choice |
| `site` | Site | whole number |
| `area` | Area | whole number |
| `control_lcn` | Control channel LCN | whole number |
| `highest_lcn` | Highest LCN | whole number |
| `words` | Words | whole number |
| `words_known` | Words of a known type | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `swing_hz` | Data swing | frequency |
| `following_lcn` | Followed LCN | whole number |
| `following` | Following | choice |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `calls` | Calls | `source` (text), `target` (text), `channel` (text), `freq_hz` (frequency), `group` (yes or no), `start_s` (number (s)), `duration` (duration (s)), `urgency` (choice), `digital` (yes or no), `carrier_only` (yes or no), `end` (choice), `followed` (yes or no) |
| `talkgroups` | Talkgroups | `gid` (whole number), `calls` (whole number), `last_unit` (whole number), `last_lcn` (whole number), `freq_hz` (frequency), `active` (yes or no), `last_heard` (time) |
| `channels` | Channels | `lcn` (whole number), `freq_hz` (frequency), `level_db` (number (dB)), `floor_db` (number (dB)), `in_capture` (yes or no), `carrier` (yes or no), `control` (yes or no) |
| `frames` | Frames | `a` (whole number), `b` (whole number), `err_a` (whole number), `err_b` (whole number), `time_s` (number (s)), `text` (text) |
| `words` | Words | `word` (whole number), `count` (whole number) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `edacs.call` | EDACS call | `end` (choice), `source` (text), `target` (text), `channel` (text), `group` (yes or no), `urgency` (choice), `carrier_only` (yes or no), `duration` (duration (s)), `family` (choice) |
| `trunk.follow` | Follow decision | `action` (choice), `reason` (choice), `source` (text), `target` (text), `channel` (text), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `trunk`, version 1.2. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `EdacsFields`. The result fields of EDACS.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `locked` | yes or no | always | 1.0 | The receiver holds lock (symbol, carrier and frame sync as the stack defines it). |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `polarity` | choice | always | 1.0 | The data polarity. |
| `site` | whole number | optional | 1.0 | The site. |
| `area` | whole number | optional | 1.0 | The area. |
| `control_lcn` | whole number | optional | 1.0 | The control channel's LCN. |
| `highest_lcn` | whole number | optional | 1.0 | The highest LCN seen in a message. |
| `words` | whole number | always | 1.0 | Words. |
| `words_known` | whole number | always | 1.0 | Words of a known type. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `swing_hz` | frequency | always | 1.0 | Data swing. |
| `following_lcn` | whole number | optional | 1.0 | The LCN followed now. |
| `following` | choice | always | 1.0 | What the follower does now. |

### Table `calls`

Struct `EdacsCallRow`. An EDACS call. `source` is the calling unit, `target` the group or the called unit, `channel` the LCN.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `target` | text | optional | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `freq_hz` | frequency | optional | 1.0 | A frequency. |
| `group` | yes or no | always | 1.0 | A group call. |
| `start_s` | number (s) | always | 1.0 | The start of a call, signal time. |
| `duration` | duration (s) | always | 1.0 | The length of a call. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `digital` | yes or no | always | 1.0 | A digital call (not decoded). |
| `carrier_only` | yes or no | always | 1.0 | A call that no message announced. |
| `end` | choice | always | 1.0 | Where a call is in its life. |
| `followed` | yes or no | always | 1.0 | The call is followed now. |

### Table `talkgroups`

Struct `EdacsTalkgroupRow`. An EDACS talkgroup. The row key is the group id.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `gid` | whole number | always | 1.0 | A group. |
| `calls` | whole number | always | 1.0 | Calls. |
| `last_unit` | whole number | optional | 1.0 | Last unit. |
| `last_lcn` | whole number | always | 1.0 | Last LCN. |
| `freq_hz` | frequency | optional | 1.0 | A frequency. |
| `active` | yes or no | always | 1.0 | On air. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

### Table `channels`

Struct `EdacsChannelRow`. An EDACS channel. The row key is the LCN.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `lcn` | whole number | always | 1.0 | A voice channel number. |
| `freq_hz` | frequency | optional | 1.0 | A frequency. |
| `level_db` | number (dB) | always | 1.0 | Level. |
| `floor_db` | number (dB) | always | 1.0 | Floor. |
| `in_capture` | yes or no | always | 1.0 | In the capture. |
| `carrier` | yes or no | always | 1.0 | Carrier. |
| `control` | yes or no | always | 1.0 | Control channel. |

### Table `frames`

Struct `EdacsFrameRow`. An EDACS frame. The row key is the sequence number of the frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `a` | whole number | always | 1.0 | Message A. |
| `b` | whole number | always | 1.0 | Message B. |
| `err_a` | whole number | always | 1.0 | Copy errors of A. |
| `err_b` | whole number | always | 1.0 | Copy errors of B. |
| `time_s` | number (s) | always | 1.0 | Time. |
| `text` | text | always | 1.0 | Read as. |

### Table `words`

Struct `EdacsWordRow`. A message type and how often it was heard. The row key is the type.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `word` | whole number | always | 1.0 | Word. |
| `count` | whole number | always | 1.0 | Count. |

### Event `edacs.call`

Struct `EdacsCallEvent`. An EDACS call that started or ended.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `end` | choice | always | 1.0 | Where a call is in its life. |
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `target` | text | optional | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `group` | yes or no | always | 1.0 | A group call. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `carrier_only` | yes or no | always | 1.0 | A call that no message announced. |
| `duration` | duration (s) | always | 1.0 | The length of a call. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `trunk.follow`

Struct `FollowEvent`. A follow decision. `source` is the caller, `target` the talkgroup or unit, `channel` the channel number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `action` | choice | always | 1.0 | The action of a follow decision. |
| `reason` | choice | optional | 1.0 | The reason of a skip or a release. |
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `target` | text | optional | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `channel` | text | optional | 1.0 | The channel or talk group of a call. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_edacs_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `extended` | `sdrkit_edacs_get_extended` | `sdrkit_edacs_set_extended` |
| `esk` | `sdrkit_edacs_get_esk` | `sdrkit_edacs_set_esk` |
| `lcn_table` | `sdrkit_edacs_get_lcn_table` | `sdrkit_edacs_set_lcn_table` |
| `follow` | `sdrkit_edacs_get_follow` | `sdrkit_edacs_set_follow` |
| `follow_carriers` | `sdrkit_edacs_get_follow_carriers` | `sdrkit_edacs_set_follow_carriers` |
| `follow_groups` | `sdrkit_edacs_get_follow_groups` | `sdrkit_edacs_set_follow_groups` |

## Try it

```sh
sdrkit run edacs --source synth:edacs
sdrkit run edacs --source iiod:192.168.2.1 --freq 856
```
<!-- generated: end -->
