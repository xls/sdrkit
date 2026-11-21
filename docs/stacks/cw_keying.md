<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# CW keying quality meter

Speed, dit and dah, dah ratio, gaps, weight, dit scatter, rise and fall times and chirp of a keyed carrier.

Stack id: `cw_keying`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 500 MHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 4 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cw_keying` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `search_hz` | Carrier search half width | number (Hz) | 1500 | 100 to 1500 |  |
| `on_share` | Key-down above (of the peak) | number | 0.5 | 0.1 to 0.9 |  |
| `off_share` | Key-up under (of the peak) | number | 0.35 | 0.1 to 0.9 |  |
| `elements` | Elements kept | whole number | 60 | 20 to 200 |  |
| `min_chirp_mark_s` | Shortest mark for the chirp | number (s) | 0.05 | 0.02 to 0.5 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_text` | Text | text | `CQ CQ DE WIAW WIAW K PARIS PARIS QRZ TEST` |  |  |
| `gen_wpm` | Speed | number (wpm) | 20 | 1 to 100 |  |
| `gen_dah_ratio` | Dah ratio | number | 3 | 1.5 to 6 |  |
| `gen_weight` | Weight | number | 1 | 0.5 to 1.5 |  |
| `gen_char_gap` | Character gap | number (units) | 3 | 2 to 10 |  |
| `gen_word_gap` | Word gap | number (units) | 7 | 4 to 20 |  |
| `gen_jitter` | Mark jitter | number | 0 | 0 to 0.5 |  |
| `gen_ramp_s` | Edge length | number (s) | 0.006 | 0 to 0.05 |  |
| `gen_chirp_hz` | Chirp | number (Hz) | 0 | -1000 to 1000 |  |
| `gen_chirp_tau_s` | Chirp time constant | number (s) | 0.02 | 0.001 to 1 |  |
| `gen_fade_db` | Level swing at 0.2 Hz | number (dB) | 0 | 0 to 40 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `wpm` | Speed (PARIS) | number (wpm) |
| `dit_ms` | Dit | number (ms) |
| `dah_ms` | Dah | number (ms) |
| `dah_ratio` | Dah over dit (ideal 3.00) | number |
| `element_gap` | Element gap (ideal 1) | number (dits) |
| `char_gap` | Character gap (ideal 3) | number (dits) |
| `word_gap` | Word gap (ideal 7) | number (dits) |
| `char_gaps` | Character gaps | whole number |
| `word_gaps` | Word gaps | whole number |
| `weight` | Dit over element gap (ideal 1.00) | number |
| `dit_scatter_pct` | Coefficient of variation of the dit | number (%) |
| `rise_ms` | 10 to 90 % rise | number (ms) |
| `fall_ms` | 90 to 10 % fall | number (ms) |
| `chirp_hz` | Chirp | number (Hz) |
| `chirp_marks` | Marks of the chirp | whole number |
| `marks` | Marks | whole number |
| `spaces` | Spaces | whole number |
| `carrier_offset_hz` | Carrier | frequency |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Keying | `time_s` (duration (s)), `wpm` (number (wpm)), `dit_ms` (number (ms)), `dah_ratio` (number), `element_gap` (number (dits)), `char_gap` (number (dits)), `word_gap` (number (dits)), `weight` (number), `dit_scatter_pct` (number (%)), `rise_ms` (number (ms)), `fall_ms` (number (ms)), `chirp_hz` (number (Hz)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 1.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

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

Struct `CwKeyingFields`. The result fields of `cw_keying`: the evaluation of the last marks and spaces. The evaluation has no value before 10 marks and when no mark came in the last 8 s.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `wpm` | number (wpm) | optional | 1.0 | The speed in words a minute of 50 units. |
| `dit_ms` | number (ms) | optional | 1.0 | The mean dit. |
| `dah_ms` | number (ms) | optional | 1.0 | The mean dah; no value with fewer than 2 dahs. |
| `dah_ratio` | number | optional | 1.0 | The mean dah over the mean dit. |
| `element_gap` | number (dits) | optional | 1.0 | The mean gap inside a character, in dits. |
| `char_gap` | number (dits) | optional | 1.0 | The mean gap between characters, in dits. |
| `word_gap` | number (dits) | optional | 1.0 | The mean gap between words, in dits. |
| `char_gaps` | whole number | optional | 1.0 | The character gaps counted. |
| `word_gaps` | whole number | optional | 1.0 | The word gaps counted. |
| `weight` | number | optional | 1.0 | The dit length over the element gap length. |
| `dit_scatter_pct` | number (%) | optional | 1.0 | The population standard deviation of the dits over their mean. |
| `rise_ms` | number (ms) | optional | 1.0 | The mean 10 to 90 % rise of the marks; 1.5 ms is the least the envelope filter shows. |
| `fall_ms` | number (ms) | optional | 1.0 | The mean 90 to 10 % fall of the marks. |
| `chirp_hz` | number (Hz) | optional | 1.0 | The mean frequency of the start of a mark minus that of its second half. |
| `chirp_marks` | whole number | optional | 1.0 | The marks the chirp is from. |
| `marks` | whole number | always | 1.0 | The marks in the window. |
| `spaces` | whole number | always | 1.0 | The spaces in the window. |
| `carrier_offset_hz` | frequency | optional | 1.0 | The carrier offset from the tuned frequency. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `CwKeyingEventRow`. A row of the `cw_keying` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `wpm` | number (wpm) | always | 1.0 | The speed. |
| `dit_ms` | number (ms) | always | 1.0 | The mean dit. |
| `dah_ratio` | number | optional | 1.0 | The dah ratio. |
| `element_gap` | number (dits) | optional | 1.0 | The element gap. |
| `char_gap` | number (dits) | optional | 1.0 | The character gap. |
| `word_gap` | number (dits) | optional | 1.0 | The word gap. |
| `weight` | number | optional | 1.0 | The weight. |
| `dit_scatter_pct` | number (%) | always | 1.0 | The dit scatter. |
| `rise_ms` | number (ms) | optional | 1.0 | The rise. |
| `fall_ms` | number (ms) | optional | 1.0 | The fall. |
| `chirp_hz` | number (Hz) | optional | 1.0 | The chirp. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_cw_keying_clear` | none | none |
| `reset` | `sdrkit_cw_keying_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `search_hz` | `sdrkit_cw_keying_get_search_hz` | `sdrkit_cw_keying_set_search_hz` |
| `on_share` | `sdrkit_cw_keying_get_on_share` | `sdrkit_cw_keying_set_on_share` |
| `off_share` | `sdrkit_cw_keying_get_off_share` | `sdrkit_cw_keying_set_off_share` |
| `elements` | `sdrkit_cw_keying_get_elements` | `sdrkit_cw_keying_set_elements` |
| `min_chirp_mark_s` | `sdrkit_cw_keying_get_min_chirp_mark_s` | `sdrkit_cw_keying_set_min_chirp_mark_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_text` | `sdrkit_cw_keying_gen_get_text` | `sdrkit_cw_keying_gen_set_text` |
| `gen_wpm` | `sdrkit_cw_keying_gen_get_wpm` | `sdrkit_cw_keying_gen_set_wpm` |
| `gen_dah_ratio` | `sdrkit_cw_keying_gen_get_dah_ratio` | `sdrkit_cw_keying_gen_set_dah_ratio` |
| `gen_weight` | `sdrkit_cw_keying_gen_get_weight` | `sdrkit_cw_keying_gen_set_weight` |
| `gen_char_gap` | `sdrkit_cw_keying_gen_get_char_gap` | `sdrkit_cw_keying_gen_set_char_gap` |
| `gen_word_gap` | `sdrkit_cw_keying_gen_get_word_gap` | `sdrkit_cw_keying_gen_set_word_gap` |
| `gen_jitter` | `sdrkit_cw_keying_gen_get_jitter` | `sdrkit_cw_keying_gen_set_jitter` |
| `gen_ramp_s` | `sdrkit_cw_keying_gen_get_ramp_s` | `sdrkit_cw_keying_gen_set_ramp_s` |
| `gen_chirp_hz` | `sdrkit_cw_keying_gen_get_chirp_hz` | `sdrkit_cw_keying_gen_set_chirp_hz` |
| `gen_chirp_tau_s` | `sdrkit_cw_keying_gen_get_chirp_tau_s` | `sdrkit_cw_keying_gen_set_chirp_tau_s` |
| `gen_fade_db` | `sdrkit_cw_keying_gen_get_fade_db` | `sdrkit_cw_keying_gen_set_fade_db` |

## Try it

```sh
sdrkit run cw_keying --source synth:cw_keying
sdrkit run cw_keying --source <driver>:<address> --freq 0
```
<!-- generated: end -->
