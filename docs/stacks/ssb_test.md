<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SSB transmitter test meter

One tone: carrier leak, opposite sideband and harmonics; two tones: IMD3, IMD5 and IMD7 under one tone and PEP.

Stack id: `ssb_test`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ssb_test` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tone_min_hz` | Tone search, low edge | number (Hz) | 100 | 50 to 3800 |  |
| `tone_max_hz` | Tone search, high edge | number (Hz) | 3500 | 50 to 3800 |  |
| `two_tone_db` | Second tone within | number (dB) | 10 | 3 to 20 |  |
| `pep_offset_db` | PEP over one tone | number (dB) | 6.02 | 0 to 10 | 6.02 dB for two equal tones. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tones` | Tones | text | `1000, 0; -1000, -55; 2000, -60; 3000, -58; 4000, -70; 5000, -66` |  | Tones separated by ';', each 'audio Hz, dBc'; a negative audio frequency is on the opposite sideband. |
| `gen_sideband` | Sideband | choice | `upper` | `upper` (Upper), `lower` (Lower) |  |
| `gen_carrier_dbc` | Carrier leak | number (dBc) | -50 | -200 to 0 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `kind` | Test | choice |
| `sideband` | Sideband | choice |
| `tone1_hz` | Tone 1 | frequency |
| `tone2_hz` | Tone 2 | frequency |
| `tone_level_diff_db` | Level difference of the tones | number (dB) |
| `carrier_db` | Carrier leak under the tone | number (dB) |
| `carrier_under` | Carrier under the floor | yes or no |
| `carrier_found` | Carrier seen | yes or no |
| `opposite_db` | Opposite sideband under the tone | number (dB) |
| `opposite_under` | Opposite sideband under the floor | yes or no |
| `h2_db` | Harmonic 2 under the tone | number (dB) |
| `h3_db` | Harmonic 3 under the tone | number (dB) |
| `h4_db` | Harmonic 4 under the tone | number (dB) |
| `h5_db` | Harmonic 5 under the tone | number (dB) |
| `h2_under` | Harmonic 2 under the floor | yes or no |
| `h3_under` | Harmonic 3 under the floor | yes or no |
| `h4_under` | Harmonic 4 under the floor | yes or no |
| `h5_under` | Harmonic 5 under the floor | yes or no |
| `imd3_db` | IMD3 under one tone | number (dB) |
| `imd5_db` | IMD5 under one tone | number (dB) |
| `imd7_db` | IMD7 under one tone | number (dB) |
| `imd3_under` | IMD3 under the floor | yes or no |
| `imd5_under` | IMD5 under the floor | yes or no |
| `imd7_under` | IMD7 under the floor | yes or no |
| `imd3_pep_db` | IMD3 under PEP | number (dB) |
| `imd5_pep_db` | IMD5 under PEP | number (dB) |
| `imd7_pep_db` | IMD7 under PEP | number (dB) |
| `floor_db` | Measurement floor under the tone | number (dB) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Tests | `time_s` (duration (s)), `kind` (choice), `sideband` (choice), `tone1_hz` (frequency), `tone2_hz` (frequency), `tone_level_diff_db` (number (dB)), `carrier_db` (number (dB)), `carrier_under` (yes or no), `carrier_found` (yes or no), `opposite_db` (number (dB)), `opposite_under` (yes or no), `h2_db` (number (dB)), `h3_db` (number (dB)), `h4_db` (number (dB)), `h5_db` (number (dB)), `h2_under` (yes or no), `h3_under` (yes or no), `h4_under` (yes or no), `h5_under` (yes or no), `imd3_db` (number (dB)), `imd5_db` (number (dB)), `imd7_db` (number (dB)), `imd3_under` (yes or no), `imd5_under` (yes or no), `imd7_under` (yes or no), `imd3_pep_db` (number (dB)), `imd5_pep_db` (number (dB)), `imd7_pep_db` (number (dB)), `floor_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `kind`, `imd3_db`, `imd5_db`, `carrier_db`, `opposite_db`, `h2_db`.

### Result fields

Struct `WatchFields`. The status of a spectrum watcher: the state, the counts of readings and the input level. The headline is the summary of the status.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The state. |
| `good` | whole number | always | 1.0 | The good readings: what the stack counts as a measurement. |
| `bad` | whole number | always | 1.0 | The bad readings: what the stack counts as a fault of the watched band. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level of the last report, relative to full scale. |
| `note` | text | always | 1.0 | What the stack measures. |

### Result fields

Struct `SsbTestFields`. The result fields of `ssb_test`, from the last window. A level under the floor of the measurement is the floor with its flag set.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | The test seen. |
| `sideband` | choice | optional | 1.0 | The sideband of the tones. |
| `tone1_hz` | frequency | optional | 1.0 | The audio frequency of the (lower) tone. |
| `tone2_hz` | frequency | optional | 1.0 | The audio frequency of the higher tone of two. |
| `tone_level_diff_db` | number (dB) | optional | 1.0 | The stronger tone over the weaker. |
| `carrier_db` | number (dB) | optional | 1.0 | The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search: the leak is at least this far under the tone. |
| `carrier_under` | yes or no | optional | 1.0 | No carrier is seen and the value is the floor of the carrier search, a bound. |
| `carrier_found` | yes or no | optional | 1.0 | A carrier was seen; else it is assumed at the tuned frequency for the tones and products. |
| `opposite_db` | number (dB) | optional | 1.0 | The opposite sideband of the (lower) tone. |
| `opposite_under` | yes or no | optional | 1.0 | The opposite sideband is under the floor. |
| `h2_db` | number (dB) | optional | 1.0 | Harmonic 2 of the audio tone (one tone). |
| `h3_db` | number (dB) | optional | 1.0 | Harmonic 3 of the audio tone (one tone). |
| `h4_db` | number (dB) | optional | 1.0 | Harmonic 4 of the audio tone (one tone). |
| `h5_db` | number (dB) | optional | 1.0 | Harmonic 5 of the audio tone (one tone). |
| `h2_under` | yes or no | optional | 1.0 | Harmonic 2 is under the floor. |
| `h3_under` | yes or no | optional | 1.0 | Harmonic 3 is under the floor. |
| `h4_under` | yes or no | optional | 1.0 | Harmonic 4 is under the floor. |
| `h5_under` | yes or no | optional | 1.0 | Harmonic 5 is under the floor. |
| `imd3_db` | number (dB) | optional | 1.0 | The stronger product of order 3 under the weaker tone (two tones). |
| `imd5_db` | number (dB) | optional | 1.0 | The stronger product of order 5 under the weaker tone (two tones). |
| `imd7_db` | number (dB) | optional | 1.0 | The stronger product of order 7 under the weaker tone (two tones). |
| `imd3_under` | yes or no | optional | 1.0 | The products of order 3 are under the floor. |
| `imd5_under` | yes or no | optional | 1.0 | The products of order 5 are under the floor. |
| `imd7_under` | yes or no | optional | 1.0 | The products of order 7 are under the floor. |
| `imd3_pep_db` | number (dB) | optional | 1.0 | The product of order 3 under the peak envelope power. |
| `imd5_pep_db` | number (dB) | optional | 1.0 | The product of order 5 under the peak envelope power. |
| `imd7_pep_db` | number (dB) | optional | 1.0 | The product of order 7 under the peak envelope power. |
| `floor_db` | number (dB) | optional | 1.0 | The floor of this measurement under the tone: the noise of 7 bins. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `SsbTestEventRow`. A row of the `ssb_test` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | The test seen. |
| `sideband` | choice | optional | 1.0 | The sideband of the tones. |
| `tone1_hz` | frequency | optional | 1.0 | The audio frequency of the (lower) tone. |
| `tone2_hz` | frequency | optional | 1.0 | The audio frequency of the higher tone of two. |
| `tone_level_diff_db` | number (dB) | optional | 1.0 | The stronger tone over the weaker. |
| `carrier_db` | number (dB) | optional | 1.0 | The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search: the leak is at least this far under the tone. |
| `carrier_under` | yes or no | optional | 1.0 | No carrier is seen and the value is the floor of the carrier search, a bound. |
| `carrier_found` | yes or no | optional | 1.0 | A carrier was seen; else it is assumed at the tuned frequency for the tones and products. |
| `opposite_db` | number (dB) | optional | 1.0 | The opposite sideband of the (lower) tone. |
| `opposite_under` | yes or no | optional | 1.0 | The opposite sideband is under the floor. |
| `h2_db` | number (dB) | optional | 1.0 | Harmonic 2 of the audio tone (one tone). |
| `h3_db` | number (dB) | optional | 1.0 | Harmonic 3 of the audio tone (one tone). |
| `h4_db` | number (dB) | optional | 1.0 | Harmonic 4 of the audio tone (one tone). |
| `h5_db` | number (dB) | optional | 1.0 | Harmonic 5 of the audio tone (one tone). |
| `h2_under` | yes or no | optional | 1.0 | Harmonic 2 is under the floor. |
| `h3_under` | yes or no | optional | 1.0 | Harmonic 3 is under the floor. |
| `h4_under` | yes or no | optional | 1.0 | Harmonic 4 is under the floor. |
| `h5_under` | yes or no | optional | 1.0 | Harmonic 5 is under the floor. |
| `imd3_db` | number (dB) | optional | 1.0 | The stronger product of order 3 under the weaker tone (two tones). |
| `imd5_db` | number (dB) | optional | 1.0 | The stronger product of order 5 under the weaker tone (two tones). |
| `imd7_db` | number (dB) | optional | 1.0 | The stronger product of order 7 under the weaker tone (two tones). |
| `imd3_under` | yes or no | optional | 1.0 | The products of order 3 are under the floor. |
| `imd5_under` | yes or no | optional | 1.0 | The products of order 5 are under the floor. |
| `imd7_under` | yes or no | optional | 1.0 | The products of order 7 are under the floor. |
| `imd3_pep_db` | number (dB) | optional | 1.0 | The product of order 3 under the peak envelope power. |
| `imd5_pep_db` | number (dB) | optional | 1.0 | The product of order 5 under the peak envelope power. |
| `imd7_pep_db` | number (dB) | optional | 1.0 | The product of order 7 under the peak envelope power. |
| `floor_db` | number (dB) | optional | 1.0 | The floor of this measurement under the tone: the noise of 7 bins. |

The rows are a log: they arrive over time. Time: `time_s`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ssb_test_clear` | none | none |
| `reset` | `sdrkit_ssb_test_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `tone_min_hz` | `sdrkit_ssb_test_get_tone_min_hz` | `sdrkit_ssb_test_set_tone_min_hz` |
| `tone_max_hz` | `sdrkit_ssb_test_get_tone_max_hz` | `sdrkit_ssb_test_set_tone_max_hz` |
| `two_tone_db` | `sdrkit_ssb_test_get_two_tone_db` | `sdrkit_ssb_test_set_two_tone_db` |
| `pep_offset_db` | `sdrkit_ssb_test_get_pep_offset_db` | `sdrkit_ssb_test_set_pep_offset_db` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tones` | `sdrkit_ssb_test_gen_get_tones` | `sdrkit_ssb_test_gen_set_tones` |
| `gen_sideband` | `sdrkit_ssb_test_gen_get_sideband` | `sdrkit_ssb_test_gen_set_sideband` |
| `gen_carrier_dbc` | `sdrkit_ssb_test_gen_get_carrier_dbc` | `sdrkit_ssb_test_gen_set_carrier_dbc` |

## Try it

```sh
sdrkit run ssb_test --source synth:ssb_test
sdrkit run ssb_test --source <driver>:<address> --freq 0
```
<!-- generated: end -->
