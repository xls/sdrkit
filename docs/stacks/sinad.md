<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FM SINAD meter

SINAD, THD, THD and noise, peak deviation and carrier offset of an FM carrier with a test tone; the 12 dB crossing.

Stack id: `sinad`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 26 MHz to 6 GHz |
| Sample rate | 48 kHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 16 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sinad` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `band_lo_hz` | Audio band, low edge | number (Hz) | 300 | 50 to 8000 |  |
| `band_hi_hz` | Audio band, high edge | number (Hz) | 3000 | 50 to 8000 |  |
| `sinad_mark_db` | Sensitivity mark | number (dB) | 12 | 6 to 30 | The SINAD of the sensitivity figure: 12 dB in TIA-603. |
| `tone_min_db` | Tone over the band median | number (dB) | 20 | 10 to 40 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tone_hz` | Test tone | number (Hz) | 1000 | 50 to 8000 |  |
| `gen_deviation_hz` | Deviation | number (Hz) | 3000 | 0 to 10000 |  |
| `gen_h2` | Second harmonic | number | 0 | 0 to 1 |  |
| `gen_h3` | Third harmonic | number | 0 | 0 to 1 |  |
| `gen_deviation_after` | Later deviations | text | empty |  | Changes separated by ';', each 'time s, deviation Hz'. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `sinad_db` | SINAD | number (dB) |
| `below_12db` | Under the sensitivity mark | yes or no |
| `tone_hz` | Test tone | frequency |
| `deviation_khz` | Peak deviation | number (kHz) |
| `thd_pct` | THD | number (%) |
| `thdn_pct` | THD + noise | number (%) |
| `carrier_offset_hz` | Carrier offset | frequency |
| `windows` | Windows | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Readings | `time_s` (duration (s)), `sinad_db` (number (dB)), `crossing` (choice), `tone_hz` (frequency), `deviation_khz` (number (kHz)), `thd_pct` (number (%)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `sinad.crossing` | Crossing | `time_s` (duration (s)), `sinad_db` (number (dB)), `crossing` (choice), `tone_hz` (frequency), `deviation_khz` (number (kHz)), `thd_pct` (number (%)) |

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

Struct `SinadFields`. The result fields of `sinad`, from the last window. The readings have no value while no test tone is found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sinad_db` | number (dB) | optional | 1.0 | SINAD in the audio band, flat weighting. |
| `below_12db` | yes or no | optional | 1.0 | The SINAD is under the sensitivity mark (12 dB by default). |
| `tone_hz` | frequency | optional | 1.0 | The test tone. |
| `deviation_khz` | number (kHz) | optional | 1.0 | The peak deviation of the tone. |
| `thd_pct` | number (%) | optional | 1.0 | THD of harmonics 2 to 5. |
| `thdn_pct` | number (%) | optional | 1.0 | THD and noise. |
| `carrier_offset_hz` | frequency | optional | 1.0 | The carrier offset: the mean of the discriminator audio. |
| `windows` | whole number | always | 1.0 | The windows measured. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `SinadEventRow`. A row of the `sinad` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `sinad_db` | number (dB) | always | 1.0 | SINAD. |
| `crossing` | choice | always | 1.0 | The change against the mark. |
| `tone_hz` | frequency | always | 1.0 | The test tone. |
| `deviation_khz` | number (kHz) | always | 1.0 | The peak deviation. |
| `thd_pct` | number (%) | always | 1.0 | THD. |

### Event `sinad.crossing`

Struct `SinadCrossingEvent`. The SINAD crossed the sensitivity mark.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `sinad_db` | number (dB) | always | 1.0 | SINAD. |
| `crossing` | choice | always | 1.0 | The change against the mark. |
| `tone_hz` | frequency | always | 1.0 | The test tone. |
| `deviation_khz` | number (kHz) | always | 1.0 | The peak deviation. |
| `thd_pct` | number (%) | always | 1.0 | THD. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sinad_clear` | none | none |
| `reset` | `sdrkit_sinad_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `band_lo_hz` | `sdrkit_sinad_get_band_lo_hz` | `sdrkit_sinad_set_band_lo_hz` |
| `band_hi_hz` | `sdrkit_sinad_get_band_hi_hz` | `sdrkit_sinad_set_band_hi_hz` |
| `sinad_mark_db` | `sdrkit_sinad_get_sinad_mark_db` | `sdrkit_sinad_set_sinad_mark_db` |
| `tone_min_db` | `sdrkit_sinad_get_tone_min_db` | `sdrkit_sinad_set_tone_min_db` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tone_hz` | `sdrkit_sinad_gen_get_tone_hz` | `sdrkit_sinad_gen_set_tone_hz` |
| `gen_deviation_hz` | `sdrkit_sinad_gen_get_deviation_hz` | `sdrkit_sinad_gen_set_deviation_hz` |
| `gen_h2` | `sdrkit_sinad_gen_get_h2` | `sdrkit_sinad_gen_set_h2` |
| `gen_h3` | `sdrkit_sinad_gen_get_h3` | `sdrkit_sinad_gen_set_h3` |
| `gen_deviation_after` | `sdrkit_sinad_gen_get_deviation_after` | `sdrkit_sinad_gen_set_deviation_after` |

## Try it

```sh
sdrkit run sinad --source synth:sinad
sdrkit run sinad --source <driver>:<address> --freq 0
```
<!-- generated: end -->
