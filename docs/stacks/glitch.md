<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SDR stream glitch detector

Dropped and repeated samples on a steady tone, zero runs, repeated blocks and level steps of the raw stream.

Stack id: `glitch`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 100 kHz |
| Signal bandwidth | 2 MHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:glitch` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `zero_run` | Shortest zero run | whole number (samples) | 16 | 2 to 10000 |  |
| `block` | Repeated block length | whole number (samples) | 256 | 16 to 65536 |  |
| `step_db` | Level step | number (dB) | 1 | 0.2 to 20 |  |
| `tone_db` | Tone over the noise per sample | number (dB) | 20 | 10 to 60 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tone_hz` | Tone | number (Hz) | 123456 | -10000000 to 10000000 |  |
| `gen_tone_amp` | Tone amplitude | number | 1 | 0 to 1 |  |
| `gen_drops` | Drops and repeats | text | `1.0, 1; 1.5, 2; 2.0, 3; 2.5, 5; 3.0, -1; 3.5, -2` |  | Entries separated by ';', each 'time s, samples'; negative samples are repeated. |
| `gen_zero_runs` | Zero runs | text | empty |  | Entries separated by ';', each 'time s, samples'. |
| `gen_repeat_blocks` | Repeated blocks | text | empty |  | Times in s separated by ';': the 256 samples after each time plus 256 are those from the time. |
| `gen_gain_steps` | Gain steps | text | empty |  | Entries separated by ';', each 'time s, dB': the gain from that time on. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `tone_hz` | Watched tone | frequency |
| `sample_drops` | Sample drops | whole number |
| `sample_repeats` | Sample repeats | whole number |
| `zero_runs` | Zero runs | whole number |
| `repeated_blocks` | Repeated blocks | whole number |
| `level_steps` | Level steps | whole number |
| `elapsed_s` | Time observed | duration (s) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Faults | `time_s` (duration (s)), `kind` (choice), `samples` (whole number), `phase_step_rad` (number (rad)), `step_db` (number (dB)), `from_db` (number (dB)), `to_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `glitch.event` | Stream fault | `time_s` (duration (s)), `kind` (choice), `samples` (whole number), `phase_step_rad` (number (rad)), `step_db` (number (dB)), `from_db` (number (dB)), `to_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 1.1. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

### Result fields

Struct `WatchFields`. The status of a spectrum watcher: the headline, the state, the counts of readings and the input level.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The headline. |
| `state` | choice | always | 1.0 | The state. |
| `good` | whole number | always | 1.0 | The good readings: what the stack counts as a measurement. |
| `bad` | whole number | always | 1.0 | The bad readings: what the stack counts as a fault of the watched band. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level of the last report, relative to full scale. |
| `note` | text | always | 1.0 | What the stack measures. |

### Result fields

Struct `GlitchFields`. The result fields of `glitch`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tone_hz` | frequency | optional | 1.0 | The steady tone whose phase is watched; no value while none is found. |
| `sample_drops` | whole number | always | 1.0 | Sample drop events. |
| `sample_repeats` | whole number | always | 1.0 | Sample repeat events. |
| `zero_runs` | whole number | always | 1.0 | Zero runs. |
| `repeated_blocks` | whole number | always | 1.0 | Repeated blocks. |
| `level_steps` | whole number | always | 1.0 | Level steps. |
| `elapsed_s` | duration (s) | always | 1.0 | The time observed since the start or the last retune. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `GlitchEventRow`. A row of the `glitch` log: one stream fault.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the fault. |
| `kind` | choice | always | 1.0 | The kind. |
| `samples` | whole number | optional | 1.0 | The samples missing or repeated: the smallest count that fits the phase step. |
| `phase_step_rad` | number (rad) | optional | 1.0 | The phase step of the tone. |
| `step_db` | number (dB) | optional | 1.0 | The size of a level step. |
| `from_db` | number (dB) | optional | 1.0 | The level before a step. |
| `to_db` | number (dB) | optional | 1.0 | The level after a step. |

### Event `glitch.event`

Struct `GlitchEvent`. A stream fault of `glitch`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the fault. |
| `kind` | choice | always | 1.0 | The kind. |
| `samples` | whole number | optional | 1.0 | The samples missing or repeated. |
| `phase_step_rad` | number (rad) | optional | 1.0 | The phase step of the tone. |
| `step_db` | number (dB) | optional | 1.0 | The size of a level step. |
| `from_db` | number (dB) | optional | 1.0 | The level before a step. |
| `to_db` | number (dB) | optional | 1.0 | The level after a step. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_glitch_clear` | none | none |
| `reset` | `sdrkit_glitch_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `zero_run` | `sdrkit_glitch_get_zero_run` | `sdrkit_glitch_set_zero_run` |
| `block` | `sdrkit_glitch_get_block` | `sdrkit_glitch_set_block` |
| `step_db` | `sdrkit_glitch_get_step_db` | `sdrkit_glitch_set_step_db` |
| `tone_db` | `sdrkit_glitch_get_tone_db` | `sdrkit_glitch_set_tone_db` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tone_hz` | `sdrkit_glitch_gen_get_tone_hz` | `sdrkit_glitch_gen_set_tone_hz` |
| `gen_tone_amp` | `sdrkit_glitch_gen_get_tone_amp` | `sdrkit_glitch_gen_set_tone_amp` |
| `gen_drops` | `sdrkit_glitch_gen_get_drops` | `sdrkit_glitch_gen_set_drops` |
| `gen_zero_runs` | `sdrkit_glitch_gen_get_zero_runs` | `sdrkit_glitch_gen_set_zero_runs` |
| `gen_repeat_blocks` | `sdrkit_glitch_gen_get_repeat_blocks` | `sdrkit_glitch_gen_set_repeat_blocks` |
| `gen_gain_steps` | `sdrkit_glitch_gen_get_gain_steps` | `sdrkit_glitch_gen_set_gain_steps` |

## Try it

```sh
sdrkit run glitch --source synth:glitch
sdrkit run glitch --source <driver>:<address> --freq 0
```
<!-- generated: end -->
