<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# AM stereo (C-QUAM)

Medium-wave AM stereo stations using C-QUAM, with the 25 Hz pilot, stereo meters and separation.

Stack id: `cquam`. Group: Broadcast.

Signal types: AM broadcast.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 30 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 20 kHz |
| Offset from the signal | 250 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cquam` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `stereo` | Stereo | choice | `auto` | `auto` (Auto), `mono` (Mono), `stereo` (Stereo) | Auto follows the 25 Hz pilot. Mono plays L + R only. Stereo uses L - R whatever the pilot says. |
| `width` | Width | frequency | 5 kHz | 3 kHz to 10 kHz | Audio width either side of the carrier. |
| `squelch` | Squelch (4 dB) | yes or no | true |  | Mutes below 2.5 dB C/N, opens above 4 dB. |
| `channel_offset` | Channel offset | frequency | 0 Hz | -10 kHz to 10 kHz | The station's distance from the dial. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_kind` | Test signal | choice | `stereo` | `stereo` (C-QUAM stereo), `am` (Plain AM (mono, no pilot)), `noise` (Noise only) |  |
| `gen_left_silent` | Left silent | yes or no | false |  |  |
| `gen_left_hz` | Left tone | frequency | 440 Hz | 100 Hz to 4 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `offset_hz` | Offset | frequency |
| `cn_db` | C/N | number (dB) |
| `carrier_dbfs` | Carrier level | number (dBFS) |
| `stereo` | Stereo | yes or no |
| `pilot_seen` | Pilot seen | yes or no |
| `pilot_db` | Pilot over its neighbours | number (dB) |
| `pilot_pct` | Pilot | number (%) |
| `lock_quality` | Lock quality | number |
| `left_dbfs` | Left | number (dBFS) |
| `right_dbfs` | Right | number (dBFS) |
| `width_db` | Stereo width | number (dB) |
| `mod_peak_pct` | Peak modulation | number (%) |
| `stereo_mode` | Stereo mode | choice |
| `filter_width` | Filter width | frequency |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `pilot_checks_ok` | Pilot checks good | whole number |
| `pilot_checks_bad` | Pilot checks bad | whole number |
| `signal_time` | Signal time | duration (s) |
| `input_rate` | Input rate | frequency |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `pilot_spectrum` | Pilot spectrum | Hz | dB |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets the signal and starts again. |

## Typed interface

The stack is in the family `broadcast`, version 2.1. The C header `sdrkit/broadcast.h`, the registry record `registry/broadcast.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Broadcast radio (`broadcast`). Media: sound (programme).

### Result fields

Struct `CquamFields`. The results of the C-QUAM stack that the stereo matrix writes. No offset, carrier level, C/N or lock quality without a carrier, no pilot level before the first pilot check, no levels before the first audio, no width in mono and no noise floor before the first noise measure.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset_hz` | frequency | optional | 1.0 | The carrier offset. |
| `cn_db` | number (dB) | optional | 1.0 | Carrier to noise in the channel. |
| `carrier_dbfs` | number (dBFS) | optional | 1.0 | The carrier level. |
| `stereo` | yes or no | always | 1.0 | Stereo now. |
| `pilot_seen` | yes or no | always | 1.0 | The 25 Hz pilot is seen (also with mono forced). |
| `pilot_db` | number (dB) | optional | 1.0 | The pilot over its neighbours. |
| `pilot_pct` | number (%) | optional | 1.0 | The pilot as a percentage of the carrier. |
| `lock_quality` | number | optional | 1.0 | The mean cos φ of the stereo phase (1 for a mono station). |
| `left_dbfs` | number (dBFS) | optional | 1.0 | The left level, rms over 100 ms. |
| `right_dbfs` | number (dBFS) | optional | 1.0 | The right level, rms over 100 ms. |
| `width_db` | number (dB) | optional | 1.0 | L - R over L + R. |
| `mod_peak_pct` | number (%) | always | 1.0 | The peak modulation of L + R in the last second. |
| `stereo_mode` | choice | always | 1.0 | The stereo mode setting. |
| `filter_width` | frequency | always | 1.0 | The audio filter width either side of the carrier. |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | The noise floor in the audio width. |
| `pilot_checks_ok` | whole number | always | 1.0 | Pilot checks that found the pilot. |
| `pilot_checks_bad` | whole number | always | 1.0 | Pilot checks without it. |
| `signal_time` | duration (s) | always | 1.0 | Signal time. |

### Result fields

Struct `CquamFrontFields`. The input rate, which the front end of the C-QUAM stack writes.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `input_rate` | frequency | always | 1.0 | The input rate. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_cquam_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `stereo` | `sdrkit_cquam_get_stereo` | `sdrkit_cquam_set_stereo` |
| `width` | `sdrkit_cquam_get_width` | `sdrkit_cquam_set_width` |
| `squelch` | `sdrkit_cquam_get_squelch` | `sdrkit_cquam_set_squelch` |
| `channel_offset` | `sdrkit_cquam_get_channel_offset` | `sdrkit_cquam_set_channel_offset` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_kind` | `sdrkit_cquam_gen_get_kind` | `sdrkit_cquam_gen_set_kind` |
| `gen_left_silent` | `sdrkit_cquam_gen_get_left_silent` | `sdrkit_cquam_gen_set_left_silent` |
| `gen_left_hz` | `sdrkit_cquam_gen_get_left_hz` | `sdrkit_cquam_gen_set_left_hz` |

## Try it

```sh
sdrkit run cquam --source synth:cquam
sdrkit run cquam --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
