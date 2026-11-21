<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Voice inversion scrambler

Detects speech with an inverted spectrum on a narrow-FM channel, estimates the inversion carrier and descrambles it.

Stack id: `voice_inversion`. Group: Listening.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 1.5 MHz to 500 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 12 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:voice_inversion` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `carrier_hz` | Carrier (Hz, 0 = automatic) | frequency | 0 Hz | 0 Hz to 4.3 kHz | 0 uses the carrier that the detector finds. A carrier is from 2000 to 4300 Hz. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_invert_hz` | Test signal: inversion carrier | frequency | 3 kHz | 0 Hz to 4.3 kHz | 0 sends clear speech. |
| `gen_seconds` | Test signal: length | duration (s) | 12 s | 2 to 120 |  |
| `gen_deviation_hz` | Test signal: deviation | frequency | 2.5 kHz | 100 Hz to 5 kHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `level_dbfs` | Level | number (dBFS) |
| `quality_db` | Quality | number (dB) |
| `signal_time_s` | Signal time | duration (s) |
| `input_rate_hz` | Input rate | frequency |
| `good` | Good | whole number |
| `bad` | Bad | whole number |
| `inverted` | Inverted | yes or no |
| `carrier_hz` | Carrier | frequency |
| `fit_heard` | Fit as heard | number |
| `fit_inverted` | Fit inverted | number |
| `guard.bad_samples` | Bad samples | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `time_utc` (time), `signal_time_s` (duration (s)), `text` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Long-term spectrum | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `voice_inversion.found` | Inverted speech found | `text` (text), `carrier_hz` (frequency) |
| `voice_inversion.clear` | Speech, not inverted | `text` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `voice`, version 1.1. The C header `sdrkit/voice.h`, the registry record `registry/voice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `DetectorFields`. The result fields that every single-channel stack shares.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | How far the detector is. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `quality_db` | number (dB) | optional | 1.0 | The quality measure of the stack. |
| `signal_time_s` | duration (s) | always | 1.0 | The signal time. |
| `input_rate_hz` | frequency | always | 1.0 | The rate of the input. |
| `good` | whole number | always | 1.0 | The count of good results. |
| `bad` | whole number | always | 1.0 | The count of bad results. |

### Result fields

Struct `VoiceInversionFields`. The result fields of the voice inversion detector.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `inverted` | yes or no | always | 1.0 | Inverted speech is there. |
| `carrier_hz` | frequency | optional | 1.0 | The inversion carrier found. |
| `fit_heard` | number | optional | 1.0 | The fit of the spectrum as heard. |
| `fit_inverted` | number | optional | 1.0 | The best fit of the flipped spectrum. |

### Table `events`

Struct `DetectorEventRow`. An event of a single-channel stack. The row key is the number of the event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_utc` | time | optional | 1.0 | The wall-clock time of an event. |
| `signal_time_s` | duration (s) | always | 1.0 | The signal time. |
| `text` | text | always | 1.0 | The text of a decoded message. |

### Event `voice_inversion.found`

Struct `VoiceInversionFoundEvent`. Inverted speech found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `carrier_hz` | frequency | always | 1.0 | The inversion carrier found. |

### Event `voice_inversion.clear`

Struct `VoiceInversionClearEvent`. Speech that is not inverted.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_voice_inversion_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `carrier_hz` | `sdrkit_voice_inversion_get_carrier_hz` | `sdrkit_voice_inversion_set_carrier_hz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_invert_hz` | `sdrkit_voice_inversion_gen_get_invert_hz` | `sdrkit_voice_inversion_gen_set_invert_hz` |
| `gen_seconds` | `sdrkit_voice_inversion_gen_get_seconds` | `sdrkit_voice_inversion_gen_set_seconds` |
| `gen_deviation_hz` | `sdrkit_voice_inversion_gen_get_deviation_hz` | `sdrkit_voice_inversion_gen_set_deviation_hz` |

## Try it

```sh
sdrkit run voice_inversion --source synth:voice_inversion
sdrkit run voice_inversion --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
