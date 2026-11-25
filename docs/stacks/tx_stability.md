<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Transmitter frequency stability meter

Per transmission: start and final frequency, key-up step, settling time, drift, mean offset and scatter.

Stack id: `tx_stability`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:tx_stability` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `on_db` | Key-up over the floor | number (dB) | 10 | 3 to 30 |  |
| `off_db` | Key-down under | number (dB) | 5 | 3 to 30 |  |
| `hang_s` | Hang before key-down | number (s) | 0.3 | 0.05 to 2 |  |
| `min_tx_s` | Shortest transmission analysed | number (s) | 1 | 0.5 to 60 |  |
| `reading_s` | Least reading length | number (s) | 0.05 | 0.02 to 0.5 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_keyed` | Transmissions | text | `4, 10; 14, 14.4; 18, 22` |  | Transmissions separated by ';', each 'from s, to s'. |
| `gen_final_hz` | Final frequency | number (Hz) | 500 | -100000 to 100000 |  |
| `gen_start_offset_hz` | Start offset | number (Hz) | 250 | -100000 to 100000 |  |
| `gen_tau_s` | Settling time constant | number (s) | 0.15 | 0.001 to 100 |  |
| `gen_tx_drift_hz_s` | Drift in a transmission | number (Hz/s) | 0 | -10000 to 10000 |  |
| `gen_mod_hz` | 1 kHz tone deviation | number (Hz) | 0 | 0 to 10000 |  |
| `gen_ramp_s` | Key ramp | number (s) | 0.004 | 0 to 1 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `transmitting` | Transmission on | yes or no |
| `length_s` | Length | duration (s) |
| `readings` | Readings | whole number |
| `start_hz` | Start frequency | frequency |
| `final_hz` | Final frequency | frequency |
| `keyup_step_hz` | Key-up step (start minus final) | frequency |
| `settle_s` | Settling time | duration (s) |
| `settle_band_hz` | Settling band | frequency |
| `drift_hz_s` | Drift over the second half | number (Hz/s) |
| `mean_hz` | Mean offset | frequency |
| `mean_ppm` | Mean offset | number (ppm) |
| `scatter_hz` | RMS about the drift line | frequency |
| `transmissions` | Transmissions seen | whole number |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `transmissions` | Transmissions | `time_s` (duration (s)), `length_s` (duration (s)), `readings` (whole number), `start_hz` (frequency), `final_hz` (frequency), `keyup_step_hz` (frequency), `settle_s` (duration (s)), `settle_band_hz` (frequency), `drift_hz_s` (number (Hz/s)), `mean_hz` (frequency), `mean_ppm` (number (ppm)), `scatter_hz` (frequency) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `tx_stability.transmission` | Transmission | `time_s` (duration (s)), `length_s` (duration (s)), `readings` (whole number), `start_hz` (frequency), `final_hz` (frequency), `keyup_step_hz` (frequency), `settle_s` (duration (s)), `settle_band_hz` (frequency), `drift_hz_s` (number (Hz/s)), `mean_hz` (frequency), `mean_ppm` (number (ppm)), `scatter_hz` (frequency) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `mean_hz`, `mean_ppm`, `drift_hz_s`, `keyup_step_hz`, `settle_s`.

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

Struct `TxStabilityFields`. The result fields of `tx_stability`: the transmission now, and the last analysed transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `transmitting` | yes or no | always | 1.0 | A transmission is on. |
| `length_s` | duration (s) | optional | 1.0 | The length of the transmission. |
| `readings` | whole number | optional | 1.0 | The frequency readings in it. |
| `start_hz` | frequency | optional | 1.0 | The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency. |
| `final_hz` | frequency | optional | 1.0 | The mean frequency of the last 0.4 s. |
| `keyup_step_hz` | frequency | optional | 1.0 | Start minus final. |
| `settle_s` | duration (s) | optional | 1.0 | The time of the last reading outside the settling band, plus 0.05 s. |
| `settle_band_hz` | frequency | optional | 1.0 | The larger of 10 Hz and a tenth of the key-up step. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The least-squares slope of the readings of the second half. |
| `mean_hz` | frequency | optional | 1.0 | The mean of all readings. |
| `mean_ppm` | number (ppm) | optional | 1.0 | The mean over the tuned frequency; no value without it. |
| `scatter_hz` | frequency | optional | 1.0 | The RMS residual about the drift line. |
| `transmissions` | whole number | always | 1.0 | The transmissions seen, the short ones included. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `transmissions`

Struct `TxStabilityTransmissionRow`. A row of the transmission table: one analysed transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The key-up, signal time. |
| `length_s` | duration (s) | always | 1.0 | The length of the transmission. |
| `readings` | whole number | always | 1.0 | The frequency readings in it. |
| `start_hz` | frequency | always | 1.0 | The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency. |
| `final_hz` | frequency | always | 1.0 | The mean frequency of the last 0.4 s. |
| `keyup_step_hz` | frequency | always | 1.0 | Start minus final. |
| `settle_s` | duration (s) | always | 1.0 | The time of the last reading outside the settling band, plus 0.05 s. |
| `settle_band_hz` | frequency | always | 1.0 | The larger of 10 Hz and a tenth of the key-up step. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The least-squares slope of the readings of the second half. |
| `mean_hz` | frequency | always | 1.0 | The mean of all readings. |
| `mean_ppm` | number (ppm) | optional | 1.0 | The mean over the tuned frequency; no value without it. |
| `scatter_hz` | frequency | optional | 1.0 | The RMS residual about the drift line. |

The rows are a log: they arrive over time. Time: `time_s`.

### Event `tx_stability.transmission`

Struct `TxStabilityTransmissionEvent`. A transmission was analysed.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The key-up, signal time. |
| `length_s` | duration (s) | always | 1.0 | The length of the transmission. |
| `readings` | whole number | always | 1.0 | The frequency readings in it. |
| `start_hz` | frequency | always | 1.0 | The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency. |
| `final_hz` | frequency | always | 1.0 | The mean frequency of the last 0.4 s. |
| `keyup_step_hz` | frequency | always | 1.0 | Start minus final. |
| `settle_s` | duration (s) | always | 1.0 | The time of the last reading outside the settling band, plus 0.05 s. |
| `settle_band_hz` | frequency | always | 1.0 | The larger of 10 Hz and a tenth of the key-up step. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The least-squares slope of the readings of the second half. |
| `mean_hz` | frequency | always | 1.0 | The mean of all readings. |
| `mean_ppm` | number (ppm) | optional | 1.0 | The mean over the tuned frequency; no value without it. |
| `scatter_hz` | frequency | optional | 1.0 | The RMS residual about the drift line. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_tx_stability_clear` | none | none |
| `reset` | `sdrkit_tx_stability_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `on_db` | `sdrkit_tx_stability_get_on_db` | `sdrkit_tx_stability_set_on_db` |
| `off_db` | `sdrkit_tx_stability_get_off_db` | `sdrkit_tx_stability_set_off_db` |
| `hang_s` | `sdrkit_tx_stability_get_hang_s` | `sdrkit_tx_stability_set_hang_s` |
| `min_tx_s` | `sdrkit_tx_stability_get_min_tx_s` | `sdrkit_tx_stability_set_min_tx_s` |
| `reading_s` | `sdrkit_tx_stability_get_reading_s` | `sdrkit_tx_stability_set_reading_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_keyed` | `sdrkit_tx_stability_gen_get_keyed` | `sdrkit_tx_stability_gen_set_keyed` |
| `gen_final_hz` | `sdrkit_tx_stability_gen_get_final_hz` | `sdrkit_tx_stability_gen_set_final_hz` |
| `gen_start_offset_hz` | `sdrkit_tx_stability_gen_get_start_offset_hz` | `sdrkit_tx_stability_gen_set_start_offset_hz` |
| `gen_tau_s` | `sdrkit_tx_stability_gen_get_tau_s` | `sdrkit_tx_stability_gen_set_tau_s` |
| `gen_tx_drift_hz_s` | `sdrkit_tx_stability_gen_get_tx_drift_hz_s` | `sdrkit_tx_stability_gen_set_tx_drift_hz_s` |
| `gen_mod_hz` | `sdrkit_tx_stability_gen_get_mod_hz` | `sdrkit_tx_stability_gen_set_mod_hz` |
| `gen_ramp_s` | `sdrkit_tx_stability_gen_get_ramp_s` | `sdrkit_tx_stability_gen_set_ramp_s` |

## Try it

```sh
sdrkit run tx_stability --source synth:tx_stability
sdrkit run tx_stability --source <driver>:<address> --freq 0
```
<!-- generated: end -->
