<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ATIS (inland waterways)

The voice of an inland waterway channel and the ATIS identity each radio sends after it speaks.

Stack id: `atis`. Group: Marine.

Signal types: ATIS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 156.5 MHz |
| Tuning range | 156 MHz to 162.1 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 500 kHz |
| Offset from the signal | 150 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:atis` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `listen` | Listen | yes or no | true |  | The voice of the channel on the audio output. |
| `names` | Name table | text | empty |  | Ship and station names by MMSI to import, one record per line. |
| `plan` | Channel plan | text | empty |  | The channels: entries separated by ";", each a label, "=" and the frequency in Hz, or the ship and the coast frequency of a duplex pair separated by "/", for example "16=156800000" or "1=156050000/160650000". An entry that ends in ":data" is a channel of data, left out of an empty scan list. At most 200 entries. The host fills it from a channel plan file. Empty: no channel has a label. |
| `time_base` | Time base | time | 0 s | 0 to 100000000000 | UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `other_calls` | Other calls | whole number |
| `stations` | Stations | whole number |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `reports` | Reports | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `messages` | Messages | `time` (time), `digits` (text), `callsign` (text), `mmsi` (text), `country` (text), `name` (text), `check_ok` (yes or no), `standard_z` (yes or no), `inverted` (yes or no), `repaired` (whole number), `erasures` (whole number), `level_db` (number (dB)), `branch_hz` (frequency) |
| `stations` | Stations | `digits` (text), `callsign` (text), `mmsi` (text), `country` (text), `name` (text), `best_level_db` (number (dB)), `heard` (whole number), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `atis.message` | ATIS message | `source` (text), `target` (text), `channel` (text), `family` (choice), `callsign` (text), `mmsi` (text), `country` (text), `check_ok` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `marine`, version 1.3. The C header `sdrkit/marine.h`, the registry record `registry/marine.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Voice calls (`voice`). Media: sound (voice).

### Result fields

Struct `AtisFields`. The result fields of the ATIS stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `other_calls` | whole number | always | 1.0 | Checked DSC calls of another format. |
| `stations` | whole number | always | 1.0 | Stations in the table. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `reports` | whole number | always | 1.0 | Reports made. |

### Table `messages`

Struct `AtisMessageRow`. A row of the ATIS message table. The row key is the place of the message in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | When (wall-clock time, when known). |
| `digits` | text | always | 1.0 | The ten digits. |
| `callsign` | text | always | 1.0 | The call sign of a station heard. |
| `mmsi` | text | always | 1.0 | The nine digits after the 9. |
| `country` | text | always | 1.0 | The country of the MID. |
| `name` | text | always | 1.0 | The name from the name table. |
| `check_ok` | yes or no | always | 1.0 | The check symbol agrees. |
| `standard_z` | yes or no | always | 1.0 | The identity starts with 9. |
| `inverted` | yes or no | always | 1.0 | The tones were swapped. |
| `repaired` | whole number | always | 1.0 | Symbols taken from the second copy or rebuilt. |
| `erasures` | whole number | always | 1.0 | Symbols unreadable in both copies. |
| `level_db` | number (dB) | always | 1.0 | The level over the floor. |
| `branch_hz` | frequency | always | 1.0 | The offset of the branch that found the message. |

The rows are the messages of the stack. The column `name` names a row. Sender: `callsign`. Time: `time`.

### Table `stations`

Struct `AtisStationRow`. A row of the ATIS station table. The row key is the digits of the identity.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `digits` | text | always | 1.0 | The ten digits. |
| `callsign` | text | always | 1.0 | The call sign of a station heard. |
| `mmsi` | text | always | 1.0 | The nine digits after the 9. |
| `country` | text | always | 1.0 | The country of the MID. |
| `name` | text | always | 1.0 | The name from the name table. |
| `best_level_db` | number (dB) | always | 1.0 | The best level over the floor. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |

The rows are the stations or nodes of the stack. The column `name` names a row. Time: `last_heard`.

### Event `atis.message`

Struct `AtisMessageEvent`. The fields of the event of an ATIS message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `target` | text | optional | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `channel` | text | optional | 1.0 | The channel or talk group of a call. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `callsign` | text | always | 1.0 | The call sign of a station heard. |
| `mmsi` | text | always | 1.0 | The nine digits after the 9. |
| `country` | text | always | 1.0 | The country of the MID. |
| `check_ok` | yes or no | always | 1.0 | The check symbol agrees. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_atis_clear` | none | none |
| `remove` | `sdrkit_atis_remove` | `sdrkit_atis_remove_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `listen` | `sdrkit_atis_get_listen` | `sdrkit_atis_set_listen` |
| `names` | `sdrkit_atis_get_names` | `sdrkit_atis_set_names` |
| `plan` | `sdrkit_atis_get_plan` | `sdrkit_atis_set_plan` |
| `time_base` | `sdrkit_atis_get_time_base` | `sdrkit_atis_set_time_base` |

## Try it

```sh
sdrkit run atis --source synth:atis
sdrkit run atis --source iiod:192.168.2.1 --freq 156.5
```

## Test recordings

- `tests/vectors/marine/atis_inland_48k.sigmf-meta`
<!-- generated: end -->
