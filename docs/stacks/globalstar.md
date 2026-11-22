<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Globalstar

Activity of the Globalstar terminal uplink: occupancy, power and Doppler per CDMA channel. Nothing is decoded. Receive only.

Stack id: `globalstar`. Group: Satellite.

Signal types: Globalstar.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.61825 GHz |
| Tuning range | 1.6 GHz to 1.64 GHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 3 MHz |
| Signal bandwidth | 10 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:globalstar` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `window_s` | Doppler plot span | duration (s) | 300 s | 60 to 600 |  |
| `pass_overlay` | Pass overlay | yes or no | false |  | Draw the pass planner's predicted Doppler over the track. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scenario` | Scenario | choice | `pass` | `pass` (A pass), `steady` (Steady), `silence` (Silence) | A pass: one channel sweeps from +30 to -30 kHz in 40 s, and the next channel sends 300 ms bursts every 800 ms. Steady: one channel at a constant frequency. Silence: no terminal. |
| `gen_channel` | Channel | whole number | 0 | 0 to 13 | 0: the channel nearest the centre. |
| `gen_time_scale` | Time scale | number | 1 | 1 to 100 | 10 plays a pass in a tenth of the time. |
| `gen_centre_hz` | Radio centre | frequency | 1.61825 GHz | 1.6 GHz to 1.64 GHz | Where the radio is tuned, so the generator puts the channels in the right place. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `active_channels` | Active channels | whole number |
| `activities` | Activities | whole number |
| `doppler` | Doppler | frequency |
| `dropped` | Dropped | whole number |
| `channels.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `channels` | Channels | `channel` (whole number), `centre` (frequency), `in_view` (yes or no), `active` (yes or no), `power_dbfs` (number (dBFS)), `floor_dbfs` (number (dBFS)), `snr_db` (number (dB)), `occupancy_pct` (number (%)), `doppler` (frequency), `last_active` (duration (s)), `events` (whole number) |
| `activity` | Activity | `start` (duration (s)), `duration` (duration (s)), `channel` (whole number), `peak_snr_db` (number (dB)), `doppler_start` (frequency), `doppler_end` (frequency), `wall_start` (time) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `doppler_01` | Doppler track, channel 1 | s | Hz |
| `doppler_02` | Doppler track, channel 2 | s | Hz |
| `doppler_03` | Doppler track, channel 3 | s | Hz |
| `doppler_04` | Doppler track, channel 4 | s | Hz |
| `doppler_05` | Doppler track, channel 5 | s | Hz |
| `doppler_06` | Doppler track, channel 6 | s | Hz |
| `doppler_07` | Doppler track, channel 7 | s | Hz |
| `doppler_08` | Doppler track, channel 8 | s | Hz |
| `doppler_09` | Doppler track, channel 9 | s | Hz |
| `doppler_10` | Doppler track, channel 10 | s | Hz |
| `doppler_11` | Doppler track, channel 11 | s | Hz |
| `doppler_12` | Doppler track, channel 12 | s | Hz |
| `doppler_13` | Doppler track, channel 13 | s | Hz |

## Events

| Kind | Label | Fields |
|---|---|---|
| `globalstar.activity` | Activity | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `channel` (whole number), `duration` (duration (s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.2. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `GlobalstarFields`. The result fields of Globalstar.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `active_channels` | whole number | always | 1.0 | Channels active now. |
| `activities` | whole number | always | 1.0 | Stretches of activity completed since the start (or the last Clear). |
| `doppler` | frequency | optional | 1.0 | The mean Doppler of the active channels. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `channels`

Struct `GlobalstarChannelRow`. A row of the 13 channels. The row key is the channel number ("07").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | whole number | always | 1.0 | A channel number, 1 to 13. |
| `centre` | frequency | always | 1.0 | The centre of the channel. |
| `in_view` | yes or no | always | 1.0 | The channel lies inside the input. |
| `active` | yes or no | always | 1.0 | The channel is active now. |
| `power_dbfs` | number (dBFS) | always | 1.0 | The power of the channel. |
| `floor_dbfs` | number (dBFS) | always | 1.0 | The noise floor of the channel. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the channel. |
| `occupancy_pct` | number (%) | always | 1.0 | The share of time the channel was on (60 s average). |
| `doppler` | frequency | always | 1.0 | The Doppler shift of the channel. |
| `last_active` | duration (s) | optional | 1.0 | The signal time when the channel was last active. |
| `events` | whole number | always | 1.0 | The stretches of activity on the channel. |

### Table `activity`

Struct `GlobalstarActivityRow`. A stretch of activity on a channel. The row key is the sequence number of the stretch.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time of the start. |
| `duration` | duration (s) | always | 1.0 | How long a stretch of activity lasted. |
| `channel` | whole number | always | 1.0 | A channel number, 1 to 13. |
| `peak_snr_db` | number (dB) | always | 1.0 | The peak SNR. |
| `doppler_start` | frequency | always | 1.0 | The Doppler shift at the start. |
| `doppler_end` | frequency | always | 1.0 | The Doppler shift at the end. |
| `wall_start` | time | optional | 1.0 | The wall-clock time of the start, when the samples are live. |

### Event `globalstar.activity`

Struct `GlobalstarActivityEvent`. A stretch of activity on a channel. The source is the channel ("channel 7").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The channel ("channel 7"). |
| `text` | text | always | 1.0 | The length and the Doppler shift of the stretch. |
| `snr_db` | number (dB) | optional | 1.0 | The peak signal to noise ratio of the stretch. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `channel` | whole number | always | 1.0 | A channel number, 1 to 13. |
| `duration` | duration (s) | always | 1.0 | How long a stretch of activity lasted. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_globalstar_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `window_s` | `sdrkit_globalstar_get_window_s` | `sdrkit_globalstar_set_window_s` |
| `pass_overlay` | `sdrkit_globalstar_get_pass_overlay` | `sdrkit_globalstar_set_pass_overlay` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scenario` | `sdrkit_globalstar_gen_get_scenario` | `sdrkit_globalstar_gen_set_scenario` |
| `gen_channel` | `sdrkit_globalstar_gen_get_channel` | `sdrkit_globalstar_gen_set_channel` |
| `gen_time_scale` | `sdrkit_globalstar_gen_get_time_scale` | `sdrkit_globalstar_gen_set_time_scale` |
| `gen_centre_hz` | `sdrkit_globalstar_gen_get_centre_hz` | `sdrkit_globalstar_gen_set_centre_hz` |

## Try it

```sh
sdrkit run globalstar --source synth:globalstar
sdrkit run globalstar --source iiod:192.168.2.1 --freq 1618.25
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
