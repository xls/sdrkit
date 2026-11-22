<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Motorola Type II trunking

Motorola Type II control channels: talkgroups, units, sites, patches and voice grants, with a channel plan file from the host and following calls.

Stack id: `motorola_type2`. Group: Trunking.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 850 MHz |
| Tuning range | 29.7 MHz to 960 MHz |
| Sample rate | 240 kHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 100 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:motorola_type2` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `plan` | Channel plan | text | empty |  | The path of a plan file in the host's storage (docs/schema/plans.json): one numbered band for each row of channel numbers. Empty for no plan: grants are listed with no frequency and never followed. |
| `follow` | Follow calls | yes or no | false |  | Ask the radio to listen to the voice channel of each call. |
| `follow_private` | Also follow unit-to-unit calls | yes or no | false |  | Hand private calls to the follower too. Telephone interconnect calls are never followed. |
| `follow_groups` | Follow groups | text | empty |  | Talkgroups to follow ("1201 2000-2099"). Empty follows every call. A bad item is refused. |
| `follow_max_s` | Longest follow | duration (s) | 120 s | 5 to 1800 | A call followed longer than this ends (a stuck carrier). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_deviation_hz` | Test signal deviation | frequency | 1.2 kHz | 300 Hz to 3 kHz |  |
| `gen_invert` | Test signal inverted | yes or no | false |  |  |
| `gen_word_errors` | Test signal bad words | number | 0 | 0 to 1 |  |
| `gen_idle_inside` | Test signal idle inside a message | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `locked` | Locked | yes or no |
| `rx_state` | Decoder state | whole number |
| `good_words` | Good words | whole number |
| `bad_words` | Bad words | whole number |
| `bits_repaired` | Bits repaired | whole number |
| `messages` | Messages | whole number |
| `messages_known` | Messages known | whole number |
| `system` | System | text |
| `site_id` | Site | whole number |
| `features` | Features | text |
| `control_channel_hz` | Control channel | frequency |
| `system_type` | System type | text |
| `network` | Network | text |
| `inverted` | Inverted | yes or no |
| `swing_hz` | Data swing | number (Hz) |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `plan` | Channel plan | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `time_s` (number (s)), `address` (text), `group` (yes or no), `command` (text), `text` (text) |
| `talkgroups` | Talkgroups | `talkgroup` (text), `calls` (whole number), `last_source` (whole number), `freq_hz` (frequency), `last_s` (number (s)), `active` (yes or no), `digital` (yes or no), `encrypted` (yes or no), `patched` (yes or no) |
| `sites` | Sites | `system` (text), `site` (whole number), `band` (whole number), `features` (text), `control_channel_hz` (frequency), `adjacent` (yes or no) |
| `patches` | Patches | `talkgroup` (text), `member` (text), `last_s` (number (s)) |
| `calls` | Calls | `talkgroup` (text), `source_unit` (whole number), `freq_hz` (frequency), `digital` (yes or no), `emergency` (yes or no), `encrypted` (yes or no), `start_s` (number (s)), `duration_s` (number (s)), `state` (text), `note` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `type2.message` | Type II message | `address` (text), `group` (yes or no), `command` (text), `text` (text), `family` (choice) |
| `trunk.grant` | Voice grant | `target` (text), `source` (text), `channel` (text), `group` (yes or no), `emergency` (yes or no), `encrypted` (yes or no), `digital` (yes or no), `urgency` (choice), `family` (choice) |
| `trunk.follow` | Follow decision | `action` (choice), `reason` (choice), `source` (text), `target` (text), `channel` (text), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `trunk`, version 1.2. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `MotorolaType2Fields`. The result fields of Motorola Type II.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `locked` | yes or no | always | 1.0 | The receiver holds lock (symbol, carrier and frame sync as the stack defines it). |
| `rx_state` | whole number | always | 1.0 | The state: 0 no input, 1 listening, 2 a good word in the last 5 s. |
| `good_words` | whole number | always | 1.0 | Good words. |
| `bad_words` | whole number | always | 1.0 | Bad words. |
| `bits_repaired` | whole number | always | 1.0 | Information bits repaired. |
| `messages` | whole number | always | 1.0 | Messages. |
| `messages_known` | whole number | always | 1.0 | Messages understood. |
| `system` | text | optional | 1.0 | A system ID. |
| `site_id` | whole number | optional | 1.0 | The site ID. |
| `features` | text | optional | 1.0 | Feature bits, as text. |
| `control_channel_hz` | frequency | optional | 1.0 | A control channel frequency. |
| `system_type` | text | optional | 1.0 | Type II or Type I. |
| `network` | text | always | 1.0 | SmartZone or SmartNet. |
| `inverted` | yes or no | always | 1.0 | Inverted polarity. |
| `swing_hz` | number (Hz) | always | 1.0 | The data swing. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `plan` | text | optional | 1.0 | The plan in use. |
| `note` | text | always | 1.0 | A note. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `messages`

Struct `MotorolaType2MessageRow`. A Type II message. The row key is the sequence number of the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time. |
| `address` | text | always | 1.0 | An address. |
| `group` | yes or no | always | 1.0 | A group word. |
| `command` | text | always | 1.0 | A command. |
| `text` | text | always | 1.0 | The text of a decoded message. |

### Table `talkgroups`

Struct `MotorolaType2TalkgroupRow`. A Type II talkgroup. The row key is the talkgroup.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `talkgroup` | text | always | 1.0 | A talkgroup. |
| `calls` | whole number | always | 1.0 | Grants counted. |
| `last_source` | whole number | optional | 1.0 | The last calling unit. |
| `freq_hz` | frequency | optional | 1.0 | A frequency. |
| `last_s` | number (s) | always | 1.0 | The last time. |
| `active` | yes or no | always | 1.0 | Active. |
| `digital` | yes or no | always | 1.0 | Digital. |
| `encrypted` | yes or no | always | 1.0 | Encrypted. |
| `patched` | yes or no | always | 1.0 | Patched. |

### Table `sites`

Struct `MotorolaType2SiteRow`. A Type II site. The row key is the system and the site.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `system` | text | always | 1.0 | A system ID. |
| `site` | whole number | always | 1.0 | A site. |
| `band` | whole number | always | 1.0 | A band code. |
| `features` | text | always | 1.0 | Feature bits, as text. |
| `control_channel_hz` | frequency | optional | 1.0 | A control channel frequency. |
| `adjacent` | yes or no | always | 1.0 | An adjacent site. |

### Table `patches`

Struct `MotorolaType2PatchRow`. A member of a patch. The row key is the talkgroup and the member.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `talkgroup` | text | always | 1.0 | A talkgroup. |
| `member` | text | always | 1.0 | A patch member. |
| `last_s` | number (s) | always | 1.0 | The last time. |

### Table `calls`

Struct `MotorolaType2CallRow`. A Type II call. The row key is the talkgroup.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `talkgroup` | text | always | 1.0 | A talkgroup. |
| `source_unit` | whole number | always | 1.0 | The source of a call. |
| `freq_hz` | frequency | optional | 1.0 | A frequency. |
| `digital` | yes or no | always | 1.0 | Digital. |
| `emergency` | yes or no | always | 1.0 | Emergency. |
| `encrypted` | yes or no | always | 1.0 | Encrypted. |
| `start_s` | number (s) | always | 1.0 | The start of a call. |
| `duration_s` | number (s) | always | 1.0 | The length of a call. |
| `state` | text | always | 1.0 | The state of a call. |
| `note` | text | optional | 1.0 | A note. |

### Event `type2.message`

Struct `MotorolaType2MessageEvent`. A Type II message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | An address. |
| `group` | yes or no | always | 1.0 | A group word. |
| `command` | text | always | 1.0 | A command. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `trunk.grant`

Struct `GrantEvent`. A voice grant: `target` is the talkgroup, `source` the caller, `channel` the channel number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `target` | text | always | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `group` | yes or no | always | 1.0 | A group word. |
| `emergency` | yes or no | always | 1.0 | Emergency. |
| `encrypted` | yes or no | always | 1.0 | Encrypted. |
| `digital` | yes or no | always | 1.0 | Digital. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
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
| `clear` | `sdrkit_motorola_type2_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `plan` | `sdrkit_motorola_type2_get_plan` | `sdrkit_motorola_type2_set_plan` |
| `follow` | `sdrkit_motorola_type2_get_follow` | `sdrkit_motorola_type2_set_follow` |
| `follow_private` | `sdrkit_motorola_type2_get_follow_private` | `sdrkit_motorola_type2_set_follow_private` |
| `follow_groups` | `sdrkit_motorola_type2_get_follow_groups` | `sdrkit_motorola_type2_set_follow_groups` |
| `follow_max_s` | `sdrkit_motorola_type2_get_follow_max_s` | `sdrkit_motorola_type2_set_follow_max_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_deviation_hz` | `sdrkit_motorola_type2_gen_get_deviation_hz` | `sdrkit_motorola_type2_gen_set_deviation_hz` |
| `gen_invert` | `sdrkit_motorola_type2_gen_get_invert` | `sdrkit_motorola_type2_gen_set_invert` |
| `gen_word_errors` | `sdrkit_motorola_type2_gen_get_word_errors` | `sdrkit_motorola_type2_gen_set_word_errors` |
| `gen_idle_inside` | `sdrkit_motorola_type2_gen_get_idle_inside` | `sdrkit_motorola_type2_gen_set_idle_inside` |

## Try it

```sh
sdrkit run motorola_type2 --source synth:motorola_type2
sdrkit run motorola_type2 --source iiod:192.168.2.1 --freq 850
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
