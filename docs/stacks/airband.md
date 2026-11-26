<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Airband

Aircraft voice on VHF AM: several channels at once, with squelch, scanning, a transmission log and a band watch, and push-to-talk AM on the tuned channel.

Stack id: `airband`. Group: Aviation.

Signal types: AM voice.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 118.7 MHz |
| Tuning range | 108 MHz to 140 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 1.6 MHz |
| Offset from the signal | 54.166667 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:airband` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | text | empty |  | The channel list (v3: records). Empty uses the test layout around the dial. |
| `squelch_db` | Squelch | number (dB) | 6 | -5 to 30 | Carrier-to-noise to open a channel. |
| `hang_s` | Hang | duration (s) | 0.5 s | 0 to 3 | How long a channel stays open after its carrier goes. |
| `scan` | Scan | yes or no | false |  | Plays one open channel at a time, in turn. |
| `scan_hold` | Hold | yes or no | false |  | Stays on the channel the scan stopped on. |
| `noise_remover` | Noise remover | yes or no | false |  | Takes the steady noise out of the audio. |
| `noise_remover_strength` | Strength | number (%) | 80 | 0 to 100 | How much of the noise remover's output is heard. |
| `only_active` | Only active | yes or no | false |  | Shows only the open channels in the channel table. |
| `listen` | Listen | yes or no | true |  | Plays the audio. |
| `tx_spacing` | Channel spacing | choice | `8.33` | `8.33` (8.33 kHz), `25` (25 kHz) | The spacing of the channel sent on. It sets the audio band: 350 to 2500 Hz for 8.33 kHz, 300 to 3000 Hz for 25 kHz. A signal for 8.33 kHz also fits a 25 kHz channel. |
| `tx_am_depth` | Modulation depth | number (%) | 85 | 10 to 100 | The peak modulation depth. The level control holds the audio peaks at this depth; between the samples a peak can rise by up to 4 % of it. 100 % gives 96 %, so that the carrier never pinches off. |
| `tx_mic_gain` | Microphone gain | number (dB) | 0 | -20 to 20 | The gain of the microphone audio before the level control. The level control adds up to 12 dB to quiet speech and takes loud speech down to the depth. |
| `tx_timeout` | Time-out | duration (s) | 180 s | 1 to 3600 | Push-to-talk ends after this time on air. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `channel_rate` | Channel rate | frequency |
| `tuning_error` | Radio off by | frequency |
| `squelch_db` | Squelch | number (dB) |
| `hang` | Hang | duration (s) |
| `scan` | Scan | yes or no |
| `scan_channel` | Scan channel | frequency |
| `transmissions` | Transmissions heard | whole number |
| `transmitting` | Transmitting | yes or no |
| `tx_freq_hz` | Transmit frequency | frequency |
| `tx_time_s` | Time on air | duration (s) |
| `tx_mic_dbfs` | Microphone peak | number (dBFS) |
| `tx_depth` | Modulation depth | number (%) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `channels` | Channels | `name` (text), `label` (text), `frequency` (frequency), `in_band` (yes or no), `cn_db` (number (dB)), `level_db` (number (dB)), `squelch_db` (number (dB)), `open` (yes or no), `playing` (yes or no), `heterodyne` (yes or no), `offset_hz` (frequency), `count` (whole number) |
| `seen` | Band watch | `name` (text), `frequency` (frequency), `is833` (yes or no), `excess_db` (number (dB)), `reports` (whole number), `active` (yes or no), `approximate` (yes or no), `last_s` (number (s)) |
| `activity` | Transmissions | `time` (time), `start_s` (number (s)), `name` (text), `label` (text), `frequency` (frequency), `length` (duration (s)), `cn_db` (number (dB)), `level_db` (number (dB)), `heterodyne` (yes or no) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `airband.activity` | Transmission | `source` (text), `target` (text), `channel` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `length` (duration (s)), `cn_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `ptt` | Push to talk | `on` (yes or no, required) |  | With on true, sends the speech of the microphone input as AM with full carrier on the tuned frequency, in the audio band of the channel spacing, until ptt with on false, halt or the time-out. |
| `halt` | Halt |  |  | Ends the transmission at once, with the fall of the carrier. |

## Typed interface

The stack is in the family `avdata`, version 2.0. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Voice calls (`voice`). Media: sound (voice).

### Result fields

Struct `AirbandFields`. The result fields of Airband.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel_rate` | frequency | always | 1.0 | The rate of the channel receivers. |
| `tuning_error` | frequency | optional | 1.0 | How far the radio is off. No value while it is not known. |
| `squelch_db` | number (dB) | always | 1.0 | The squelch over the noise. |
| `hang` | duration (s) | always | 1.0 | How long a channel stays open after its carrier goes. |
| `scan` | yes or no | always | 1.0 | The scan is on. |
| `scan_channel` | frequency | optional | 1.0 | The channel the scan holds. No value while it holds none. |
| `transmissions` | whole number | always | 1.0 | Transmissions heard. |

### Result fields

Struct `AirbandTxFields`. The result fields of the Airband transmitter. The measurements have no value while the stack does not transmit.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.0 | Whether the stack transmits now. |
| `tx_freq_hz` | frequency | optional | 1.0 | The transmit frequency: the tuned frequency. |
| `tx_time_s` | duration (s) | optional | 1.0 | The time on air of the transmission. |
| `tx_mic_dbfs` | number (dBFS) | optional | 1.0 | The peak of the microphone audio over the last report. |
| `tx_depth` | number (%) | optional | 1.0 | The peak modulation depth over the last report. |

### Table `channels`

Struct `AirbandChannelRow`. A row of the channels. The row key is the name of the channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the channel. |
| `label` | text | optional | 1.0 | The label of the channel. |
| `frequency` | frequency | always | 1.0 | The frequency of the channel. |
| `in_band` | yes or no | always | 1.0 | The channel is inside the band of the input. |
| `cn_db` | number (dB) | always | 1.0 | The carrier over the noise. |
| `level_db` | number (dB) | always | 1.0 | The level of the channel. |
| `squelch_db` | number (dB) | optional | 1.0 | The squelch of the channel, when it has its own. |
| `open` | yes or no | always | 1.0 | The squelch is open. |
| `playing` | yes or no | always | 1.0 | The channel plays. |
| `heterodyne` | yes or no | always | 1.0 | Two stations on the channel at once. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `count` | whole number | always | 1.0 | Transmissions on the channel. |

The rows are the groups of the stack. The column `name` names a row.

### Table `seen`

Struct `AirbandSeenRow`. A row of the band watch: a carrier on the air that is not in the list. The row key is its name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name of the channel. |
| `frequency` | frequency | always | 1.0 | The frequency. |
| `is833` | yes or no | always | 1.0 | The channel is on the 8.33 kHz grid. |
| `excess_db` | number (dB) | always | 1.0 | The level over the floor. |
| `reports` | whole number | always | 1.0 | How often the carrier was heard. |
| `active` | yes or no | always | 1.0 | The carrier is on now. |
| `approximate` | yes or no | always | 1.0 | The frequency is approximate. |
| `last_s` | number (s) | always | 1.0 | The signal time it was last seen. |

The column `name` names a row.

### Table `activity`

Struct `AirbandActivityRow`. A row of the transmissions. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | The wall-clock time. No value for a recording. |
| `start_s` | number (s) | always | 1.0 | The signal time of the start. |
| `name` | text | always | 1.0 | The name of the channel. |
| `label` | text | optional | 1.0 | The label of the channel. |
| `frequency` | frequency | always | 1.0 | The frequency of the channel. |
| `length` | duration (s) | always | 1.0 | The length of the transmission. |
| `cn_db` | number (dB) | always | 1.0 | The carrier over the noise. |
| `level_db` | number (dB) | always | 1.0 | The level. |
| `heterodyne` | yes or no | always | 1.0 | Two stations at once. |

The rows are the calls of the stack. The column `name` names a row. Time: `time`.

### Event `airband.activity`

Struct `AirbandActivityEvent`. A transmission, when it ends. Analogue voice carries no identity: the source and the target have no value. The channel is the name of the channel, the text its label.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender. No value: voice carries no identity. |
| `target` | text | optional | 1.0 | The receiver. No value: voice carries no identity. |
| `channel` | text | always | 1.0 | The name of the channel. |
| `text` | text | optional | 1.0 | The label of the channel. |
| `snr_db` | number (dB) | always | 1.0 | The carrier over the noise. |
| `family` | choice | always | 1.0 | What the event is about: voice. |
| `length` | duration (s) | always | 1.0 | The length of the transmission. |
| `cn_db` | number (dB) | always | 1.0 | The carrier over the noise. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_airband_clear` | none | none |
| `ptt` | `sdrkit_airband_ptt` | `sdrkit_airband_ptt_args` | none |
| `halt` | `sdrkit_airband_halt` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_airband_get_channels` | `sdrkit_airband_set_channels` |
| `squelch_db` | `sdrkit_airband_get_squelch_db` | `sdrkit_airband_set_squelch_db` |
| `hang_s` | `sdrkit_airband_get_hang_s` | `sdrkit_airband_set_hang_s` |
| `scan` | `sdrkit_airband_get_scan` | `sdrkit_airband_set_scan` |
| `scan_hold` | `sdrkit_airband_get_scan_hold` | `sdrkit_airband_set_scan_hold` |
| `noise_remover` | `sdrkit_airband_get_noise_remover` | `sdrkit_airband_set_noise_remover` |
| `noise_remover_strength` | `sdrkit_airband_get_noise_remover_strength` | `sdrkit_airband_set_noise_remover_strength` |
| `only_active` | `sdrkit_airband_get_only_active` | `sdrkit_airband_set_only_active` |
| `listen` | `sdrkit_airband_get_listen` | `sdrkit_airband_set_listen` |
| `tx_spacing` | `sdrkit_airband_get_tx_spacing` | `sdrkit_airband_set_tx_spacing` |
| `tx_am_depth` | `sdrkit_airband_get_tx_am_depth` | `sdrkit_airband_set_tx_am_depth` |
| `tx_mic_gain` | `sdrkit_airband_get_tx_mic_gain` | `sdrkit_airband_set_tx_mic_gain` |
| `tx_timeout` | `sdrkit_airband_get_tx_timeout` | `sdrkit_airband_set_tx_timeout` |

## Try it

```sh
sdrkit run airband --source synth:airband
sdrkit run airband --source iiod:192.168.2.1 --freq 118.7
```
<!-- generated: end -->
