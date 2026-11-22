<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Marine VHF

Voice on every marine VHF channel in the captured span, with a call log and a scan that watches a priority channel.

Stack id: `marinevhf`. Group: Marine.

Signal types: Narrow FM voice, Keyed voice.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 159 MHz |
| Tuning range | 156 MHz to 162.1 MHz |
| Sample rate | 8 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 6 MHz |
| Offset from the signal | 12.5 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:marinevhf` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `plan` | Channel plan | text | empty |  | The channels: entries separated by ";", each a label, "=" and the frequency in Hz, or the ship and the coast frequency of a duplex pair separated by "/", for example "16=156800000" or "1=156050000/160650000". An entry that ends in ":data" is a channel of data, left out of an empty scan list. At most 200 entries. The host fills it from a channel plan file. Empty: no channel has a label. |
| `channel_hz` | Channel | frequency | 0 Hz | 0 Hz to 1 GHz | The frequency of the channel to play; either half of a duplex pair of the plan plays both. 0 plays the channel that is active. |
| `squelch` | Squelch | number (%) | 50 | 0 to 100 |  |
| `deemphasis` | De-emphasis | yes or no | true |  |  |
| `priority_hz` | Priority channel | frequency | 156.8 MHz | 0 Hz to 1 GHz | A channel that takes over while it carries a call. The default is 156.8 MHz, the distress, safety and calling frequency of the ITU Radio Regulations (Appendix 15). |
| `scan` | Scan | yes or no | false |  |  |
| `scan_hz` | Scan list | text | empty |  | Frequencies in Hz separated by spaces, commas or semicolons; a frequency of a duplex pair of the plan scans both halves. Empty scans every voice channel of the plan. |
| `scan_dwell_s` | Dwell | duration (s) | 0.4 s | 0.1 to 3 |  |
| `scan_hang_s` | Hang | duration (s) | 2 s | 0 to 6 |  |
| `carriers` | Carriers | whole number | 10 | 1 to 24 |  |
| `show_heard` | Only channels heard | yes or no | false |  | Lists only the channels that are active now or had a call. |
| `time_base` | Time base | time | 0 s | 0 to 100000000000 | UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `carriers` | Carriers | whole number |
| `active_channels` | Active channels | whole number |
| `playing` | Playing | text |
| `playing_hz` | Playing frequency | frequency |
| `scan_state` | Scan | choice |
| `tune_hz` | Radio on | frequency |
| `calls` | Calls | whole number |
| `multi_channel` | Whole span | yes or no |
| `reports` | Reports | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `calls` | Calls | `serial` (whole number), `channel` (text), `freq_hz` (frequency), `time` (time), `start` (duration (s)), `duration` (duration (s)), `peak_dbfs` (number (dBFS)), `mean_dbfs` (number (dBFS)), `open` (yes or no) |
| `channels` | Channels | `channel` (text), `freq_hz` (frequency), `active` (yes or no), `open` (yes or no), `now_playing` (yes or no), `in_scan` (yes or no), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `calls` (whole number), `total` (duration (s)), `last_heard` (time), `peak_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `marinevhf.call` | Call | `source` (text), `target` (text), `channel` (text), `family` (choice), `start` (duration (s)), `duration` (duration (s)), `peak_dbfs` (number (dBFS)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `close_all` | Close all |  |  | Ends every open call. |

## Typed interface

The stack is in the family `marine`, version 1.1. The C header `sdrkit/marine.h`, the registry record `registry/marine.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Voice calls (`voice`). Media: sound (voice).

### Result fields

Struct `VhfFields`. The result fields of the marine VHF stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carriers` | whole number | always | 1.0 | Carriers the finder tracks. |
| `active_channels` | whole number | always | 1.0 | Carriers active now. |
| `playing` | text | optional | 1.0 | The channel that plays. |
| `playing_hz` | frequency | optional | 1.0 | The frequency that plays. |
| `scan_state` | choice | always | 1.0 | What the player does. |
| `tune_hz` | frequency | optional | 1.0 | The frequency the single-tuner scan requested from the radio. |
| `calls` | whole number | always | 1.0 | Calls since the start. |
| `multi_channel` | yes or no | always | 1.0 | The capture holds more than one channel. |
| `reports` | whole number | always | 1.0 | Reports made. |

### Table `calls`

Struct `VhfCallRow`. A row of the call table of the marine VHF stack. The row key is the place of the call in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `serial` | whole number | always | 1.0 | The number of a call. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `freq_hz` | frequency | always | 1.0 | The frequency a station was heard on. |
| `time` | time | optional | 1.0 | When a call started (wall-clock time, when known). |
| `start` | duration (s) | always | 1.0 | The signal time a call started. |
| `duration` | duration (s) | always | 1.0 | How long a call lasted. |
| `peak_dbfs` | number (dBFS) | always | 1.0 | The highest level of a call or channel. |
| `mean_dbfs` | number (dBFS) | always | 1.0 | The mean level of a call. |
| `open` | yes or no | always | 1.0 | The call or channel is open. |

### Table `channels`

Struct `VhfChannelRow`. A row of the channel table of the marine VHF stack. The row key is the channel frequency in units of the raster.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `freq_hz` | frequency | always | 1.0 | The frequency a station was heard on. |
| `active` | yes or no | always | 1.0 | The channel carries a signal now. |
| `open` | yes or no | always | 1.0 | The call or channel is open. |
| `now_playing` | yes or no | always | 1.0 | The channel plays. |
| `in_scan` | yes or no | always | 1.0 | The channel is in the scan list. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `calls` | whole number | always | 1.0 | Calls on the channel. |
| `total` | duration (s) | always | 1.0 | The time the channel was open in all. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `peak_dbfs` | number (dBFS) | optional | 1.0 | The highest level of a call or channel. |

### Event `marinevhf.call`

Struct `VhfCallEvent`. The fields of the event of a call on a marine VHF channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `target` | text | optional | 1.0 | The receiver of a message or call: an address, a call sign or a talk group member. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `start` | duration (s) | always | 1.0 | The signal time a call started. |
| `duration` | duration (s) | always | 1.0 | How long a call lasted. |
| `peak_dbfs` | number (dBFS) | always | 1.0 | The highest level of a call or channel. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_marinevhf_clear` | none | none |
| `close_all` | `sdrkit_marinevhf_close_all` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `plan` | `sdrkit_marinevhf_get_plan` | `sdrkit_marinevhf_set_plan` |
| `channel_hz` | `sdrkit_marinevhf_get_channel_hz` | `sdrkit_marinevhf_set_channel_hz` |
| `squelch` | `sdrkit_marinevhf_get_squelch` | `sdrkit_marinevhf_set_squelch` |
| `deemphasis` | `sdrkit_marinevhf_get_deemphasis` | `sdrkit_marinevhf_set_deemphasis` |
| `priority_hz` | `sdrkit_marinevhf_get_priority_hz` | `sdrkit_marinevhf_set_priority_hz` |
| `scan` | `sdrkit_marinevhf_get_scan` | `sdrkit_marinevhf_set_scan` |
| `scan_hz` | `sdrkit_marinevhf_get_scan_hz` | `sdrkit_marinevhf_set_scan_hz` |
| `scan_dwell_s` | `sdrkit_marinevhf_get_scan_dwell_s` | `sdrkit_marinevhf_set_scan_dwell_s` |
| `scan_hang_s` | `sdrkit_marinevhf_get_scan_hang_s` | `sdrkit_marinevhf_set_scan_hang_s` |
| `carriers` | `sdrkit_marinevhf_get_carriers` | `sdrkit_marinevhf_set_carriers` |
| `show_heard` | `sdrkit_marinevhf_get_show_heard` | `sdrkit_marinevhf_set_show_heard` |
| `time_base` | `sdrkit_marinevhf_get_time_base` | `sdrkit_marinevhf_set_time_base` |

## Try it

```sh
sdrkit run marinevhf --source synth:marinevhf
sdrkit run marinevhf --source iiod:192.168.2.1 --freq 159
```
<!-- generated: end -->
