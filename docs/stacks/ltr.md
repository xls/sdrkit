<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# LTR trunking

LTR repeaters: the sub-audible 300 bit/s words with talkgroups, go-to and free repeaters, and the repeater table.

Stack id: `ltr`. Group: Trunking.

Signal types: LTR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 460 MHz |
| Tuning range | 29.7 MHz to 960 MHz |
| Sample rate | 480 kHz |
| Smallest sample rate | 240 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 150 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ltr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `follow` | Follow calls | yes or no | false |  | Ask the radio to listen to the voice channel of each call. |
| `follow_groups` | Follow groups | text | empty |  | Talkgroups to follow ("1201 2000-2099"). Empty follows every call. A bad item is refused. |
| `follow_max_s` | Longest follow | duration (s) | 120 s | 5 to 1800 | A call followed longer than this ends (a stuck carrier). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `locked` | Locked | yes or no |
| `words_good` | Good words | whole number |
| `words_bad` | Bad words | whole number |
| `syncs` | Syncs | whole number |
| `area` | Area | whole number |
| `repeaters` | Repeaters seen | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `swing_hz` | Data swing | frequency |
| `data_valid` | Words heard | yes or no |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `words` | Words | `talkgroup` (text), `area` (whole number), `goto` (whole number), `home` (whole number), `id` (whole number), `free` (whole number), `check` (whole number), `kind` (choice), `time_s` (number (s)) |
| `repeaters` | Repeaters | `repeater` (whole number), `as_goto` (whole number), `as_home` (whole number), `as_free` (whole number), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ltr.word` | LTR word | `target` (text), `area` (whole number), `goto` (whole number), `home` (whole number), `id` (whole number), `free` (whole number), `kind` (choice), `family` (choice) |
| `trunk.follow` | Follow decision | `action` (choice), `reason` (choice), `source` (text), `target` (text), `channel` (text), `urgency` (choice), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `trunk`, version 1.6. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `LtrFields`. The result fields of LTR.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `locked` | yes or no | always | 1.0 | The receiver holds lock (symbol, carrier and frame sync as the stack defines it). |
| `words_good` | whole number | always | 1.0 | The good words. |
| `words_bad` | whole number | always | 1.0 | Bad words. |
| `syncs` | whole number | always | 1.0 | Syncs. |
| `area` | whole number | optional | 1.0 | The AREA bit. |
| `repeaters` | whole number | always | 1.0 | Repeaters seen. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `swing_hz` | frequency | always | 1.0 | Data swing. |
| `data_valid` | yes or no | always | 1.0 | Words heard. |

### Table `words`

Struct `LtrWordRow`. A good LTR word. The row key is the sequence number of the word.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `talkgroup` | text | always | 1.0 | The talkgroup, "{AREA}-{HOME}-{ID}". |
| `area` | whole number | always | 1.0 | The AREA bit. |
| `goto` | whole number | always | 1.0 | The GO-TO repeater. |
| `home` | whole number | always | 1.0 | The HOME repeater. |
| `id` | whole number | always | 1.0 | The ID. |
| `free` | whole number | always | 1.0 | The FREE repeater. |
| `check` | whole number | always | 1.0 | The 7 check bits as received. |
| `kind` | choice | always | 1.0 | What the ID says. |
| `time_s` | number (s) | always | 1.0 | The signal time of a word, s. |

The rows are a log: they arrive over time. The column `talkgroup` names a row. Time: `time_s`.

### Table `repeaters`

Struct `LtrRepeaterRow`. How often a repeater was named in each field. The row key is the repeater.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `repeater` | whole number | always | 1.0 | A repeater number. |
| `as_goto` | whole number | always | 1.0 | Times a repeater was named as go-to. |
| `as_home` | whole number | always | 1.0 | Times named as home. |
| `as_free` | whole number | always | 1.0 | Times named as free. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

### Event `ltr.word`

Struct `LtrWordEvent`. A good LTR word: `target` is the talkgroup.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `target` | text | always | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `area` | whole number | always | 1.0 | The AREA bit. |
| `goto` | whole number | always | 1.0 | The GO-TO repeater. |
| `home` | whole number | always | 1.0 | The HOME repeater. |
| `id` | whole number | always | 1.0 | The ID. |
| `free` | whole number | always | 1.0 | The FREE repeater. |
| `kind` | choice | always | 1.0 | What the ID says. |
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
| `clear` | `sdrkit_ltr_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `follow` | `sdrkit_ltr_get_follow` | `sdrkit_ltr_set_follow` |
| `follow_groups` | `sdrkit_ltr_get_follow_groups` | `sdrkit_ltr_set_follow_groups` |
| `follow_max_s` | `sdrkit_ltr_get_follow_max_s` | `sdrkit_ltr_set_follow_max_s` |

## Try it

```sh
sdrkit run ltr --source synth:ltr
sdrkit run ltr --source iiod:192.168.2.1 --freq 460
```
<!-- generated: end -->
