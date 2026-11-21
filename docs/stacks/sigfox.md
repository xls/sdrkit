<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Sigfox

Sigfox uplinks: device ids, message counters and payloads (authentication codes shown, never checked).

Stack id: `sigfox`. Group: IoT, 868 / 902 MHz.

Sigfox uplinks : the frame layer, the narrow-carrier finder, the D-BPSK demodulator, the joint decoder of the repetitions and the message list. The authentication code is shown as received and never checked.

Standard: Sigfox EP-SPECS Rev. 1.3 sections 2 and 3.

Signal types: Sigfox.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 868.13 MHz |
| Tuning range | 862 MHz to 928 MHz |
| Sample rate | 1.024 MHz |
| Smallest sample rate | 400 kHz |
| Signal bandwidth | 200 kHz |
| Offset from the signal | 250 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sigfox` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_bit_rate` | Test signal bit rate | choice | `100` | `100` (100 bit/s), `600` (600 bit/s) |  |
| `gen_reps` | Repetitions | choice | `3` | `3` (3 frames), `1` (1 frame) |  |
| `gen_gap` | Gap between messages | duration (s) | 1 s | 0.1 to 10 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `messages` | Messages | whole number |
| `zone` | Zone | text |
| `carriers` | Sigfox carriers | whole number |
| `carriers_bad` | Carriers not decoded | whole number |
| `bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `sigfox` | Sigfox | `time_s` (number (s)), `device_id` (text), `seq` (whole number), `payload` (text), `payload_bytes` (whole number), `frequency` (frequency), `level_dbfs` (number (dBFS)), `reps` (whole number), `ranks` (whole number), `bit_rate` (frequency), `auth` (text), `crc` (text), `frame_type` (text), `li` (whole number), `container_bytes` (whole number (bytes)), `downlink` (yes or no), `control` (yes or no) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `waterfall` | Waterfall | column | level |

## Events

| Kind | Label | Fields |
|---|---|---|
| `sigfox.message` | Sigfox message | `source` (text), `text` (text), `family` (choice), `device_id` (text), `seq` (whole number), `payload` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `ism`, version 1.0. The C header `sdrkit/ism.h`, the registry record `registry/ism.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `SigfoxFields`. The result fields of the message list of the Sigfox stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames decoded, every repetition. |
| `messages` | whole number | always | 1.0 | The messages in the list. |
| `zone` | text | optional | 1.0 | The zone of the dial, if the dial is in one. |

### Result fields

Struct `SigfoxFinderFields`. The count of the Sigfox carrier finder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carriers` | whole number | always | 1.0 | Narrow carriers passed to the demodulator. |

### Result fields

Struct `SigfoxDemodFields`. The count of the Sigfox demodulator.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carriers_bad` | whole number | always | 1.0 | Carriers that did not decode. |

### Result fields

Struct `GuardFields`. The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bad_samples` | whole number | always | 1.0 | Input samples replaced because they were not finite or far beyond full scale. |

### Table `sigfox`

Struct `SigfoxMessageRow`. A row of the messages. The row key is the device id, the counter and the signal time of the first frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the first frame. |
| `device_id` | text | always | 1.0 | The device id. |
| `seq` | whole number | always | 1.0 | The message counter. |
| `payload` | text | always | 1.0 | The payload as hex, or the one-bit value. |
| `payload_bytes` | whole number | always | 1.0 | The payload length. |
| `frequency` | frequency | always | 1.0 | The frequency of the first repetition. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the strongest repetition. |
| `reps` | whole number | always | 1.0 | Repetitions heard. |
| `ranks` | whole number | always | 1.0 | The ranks heard as a mask (bit 0 is rank 1). |
| `bit_rate` | frequency | always | 1.0 | The bit rate. |
| `auth` | text | always | 1.0 | The authentication code, as received and never checked. |
| `crc` | text | always | 1.0 | The CRC, checked. |
| `frame_type` | text | always | 1.0 | The frame type of the first repetition. |
| `li` | whole number | always | 1.0 | The length indicator. |
| `container_bytes` | whole number (bytes) | always | 1.0 | The container size. |
| `downlink` | yes or no | always | 1.0 | The device requests a downlink. |
| `control` | yes or no | always | 1.0 | A control message. |

### Event `sigfox.message`

Struct `SigfoxMessageEvent`. The fields of a new message event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The device id. |
| `text` | text | always | 1.0 | The log line of the message. |
| `family` | choice | always | 1.0 | The family of the event. |
| `device_id` | text | always | 1.0 | The device id. |
| `seq` | whole number | always | 1.0 | The message counter. |
| `payload` | text | always | 1.0 | The payload as hex. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sigfox_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_bit_rate` | `sdrkit_sigfox_gen_get_bit_rate` | `sdrkit_sigfox_gen_set_bit_rate` |
| `gen_reps` | `sdrkit_sigfox_gen_get_reps` | `sdrkit_sigfox_gen_set_reps` |
| `gen_gap` | `sdrkit_sigfox_gen_get_gap` | `sdrkit_sigfox_gen_set_gap` |

## Try it

```sh
sdrkit run sigfox --source synth:sigfox
sdrkit run sigfox --source iiod:192.168.2.1 --freq 868.13
```

## Test recordings

- `tests/vectors/ism/sigfox_rc1_400k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
