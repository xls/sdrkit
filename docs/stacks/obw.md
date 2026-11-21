<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Occupied bandwidth meter

The 99 % and x dB bandwidths, the centre, power and SNR, and the adjacent and alternate power of the strongest signal in the span.

Stack id: `obw`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 100 kHz |
| Signal bandwidth | 500 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:obw` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `power_pct` | Occupied power share | number (%) | 99 | 90 to 99.9 |  |
| `x_db` | The x of the x dB bandwidth in the headline | number (dB) | 26 | 3 to 60 |  |
| `average_s` | Averaging time | number (s) | 1 | 0.2 to 10 |  |
| `adjacent_offset` | Adjacent band centre, in bandwidths | number | 1.25 | 1 to 3 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_symbol_rate` | Symbol rate | number (Bd) | 25000 | 100 to 10000000 |  |
| `gen_rolloff` | Roll-off | number | 0.35 | 0.05 to 1 |  |
| `gen_psk` | PSK points | whole number | 4 | 2 to 16 |  |
| `gen_neighbour` | Neighbour | yes or no | false |  |  |
| `gen_neighbour_db` | Neighbour level | number (dB) | -40 | -100 to 20 |  |
| `gen_neighbour_offset` | Neighbour offset | number | 1.25 | -10 to 10 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `obw_khz` | Occupied bandwidth (99 %) | number (kHz) |
| `obw_low` | Lower edge | frequency |
| `obw_high` | Upper edge | frequency |
| `bw3_khz` | Bandwidth 3 dB under the peak | number (kHz) |
| `bw6_khz` | Bandwidth 6 dB under the peak | number (kHz) |
| `bw20_khz` | Bandwidth 20 dB under the peak | number (kHz) |
| `bw26_khz` | Bandwidth 26 dB under the peak | number (kHz) |
| `centre_offset_hz` | Centre against the tuned frequency | number (Hz) |
| `power_db` | Signal power | number (dB) |
| `snr_db` | Over the noise in the same bandwidth | number (dB) |
| `adjacent_low_db` | Adjacent power under the signal, below | number (dB) |
| `adjacent_high_db` | Adjacent power under the signal, above | number (dB) |
| `alternate_low_db` | Alternate power under the signal, below | number (dB) |
| `alternate_high_db` | Alternate power under the signal, above | number (dB) |
| `signal` | Signal found | yes or no |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bandwidths` | Bandwidths | `time_s` (duration (s)), `obw_khz` (number (kHz)), `obw_low` (frequency), `obw_high` (frequency), `bw3_khz` (number (kHz)), `bw6_khz` (number (kHz)), `bw20_khz` (number (kHz)), `bw26_khz` (number (kHz)), `centre_offset_hz` (number (Hz)), `power_db` (number (dB)), `snr_db` (number (dB)), `adjacent_low_db` (number (dB)), `adjacent_high_db` (number (dB)), `alternate_low_db` (number (dB)), `alternate_high_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `obw.reading` | Bandwidth reading | `time_s` (duration (s)), `obw_khz` (number (kHz)), `obw_low` (frequency), `obw_high` (frequency), `bw3_khz` (number (kHz)), `bw6_khz` (number (kHz)), `bw20_khz` (number (kHz)), `bw26_khz` (number (kHz)), `centre_offset_hz` (number (Hz)), `power_db` (number (dB)), `snr_db` (number (dB)), `adjacent_low_db` (number (dB)), `adjacent_high_db` (number (dB)), `alternate_low_db` (number (dB)), `alternate_high_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Starts the average again; the table stays. |

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

Struct `ObwFields`. The result fields of `obw`. The readings have no value while no signal is found.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `obw_khz` | number (kHz) | optional | 1.0 | The band that holds the power share, half the rest below and half above it. |
| `obw_low` | frequency | optional | 1.0 | The lower edge of the occupied band. |
| `obw_high` | frequency | optional | 1.0 | The upper edge of the occupied band. |
| `bw3_khz` | number (kHz) | optional | 1.0 | The width at 3 dB under the peak of the smoothed spectrum. |
| `bw6_khz` | number (kHz) | optional | 1.0 | The width at 6 dB under the peak. |
| `bw20_khz` | number (kHz) | optional | 1.0 | The width at 20 dB under the peak. |
| `bw26_khz` | number (kHz) | optional | 1.0 | The width at 26 dB under the peak. |
| `centre_offset_hz` | number (Hz) | optional | 1.0 | The middle of the occupied band from the tuned frequency. |
| `power_db` | number (dB) | optional | 1.0 | The power between the edges over the floor, dB re full scale. |
| `snr_db` | number (dB) | optional | 1.0 | The signal power over the floor in the same bins. |
| `adjacent_low_db` | number (dB) | optional | 1.0 | The power of the band one adjacent offset below, under the signal; no value outside the spectrum. |
| `adjacent_high_db` | number (dB) | optional | 1.0 | The same above. |
| `alternate_low_db` | number (dB) | optional | 1.0 | The power of the band two adjacent offsets below, under the signal. |
| `alternate_high_db` | number (dB) | optional | 1.0 | The same above. |
| `signal` | yes or no | always | 1.0 | A signal was found in the last evaluation. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `bandwidths`

Struct `ObwBandwidthRow`. A row of the bandwidth log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the evaluation. |
| `obw_khz` | number (kHz) | always | 1.0 | The band that holds the power share, half the rest below and half above it. |
| `obw_low` | frequency | always | 1.0 | The lower edge of the occupied band. |
| `obw_high` | frequency | always | 1.0 | The upper edge of the occupied band. |
| `bw3_khz` | number (kHz) | always | 1.0 | The width at 3 dB under the peak of the smoothed spectrum. |
| `bw6_khz` | number (kHz) | always | 1.0 | The width at 6 dB under the peak. |
| `bw20_khz` | number (kHz) | always | 1.0 | The width at 20 dB under the peak. |
| `bw26_khz` | number (kHz) | always | 1.0 | The width at 26 dB under the peak. |
| `centre_offset_hz` | number (Hz) | always | 1.0 | The middle of the occupied band from the tuned frequency. |
| `power_db` | number (dB) | always | 1.0 | The power between the edges over the floor, dB re full scale. |
| `snr_db` | number (dB) | always | 1.0 | The signal power over the floor in the same bins. |
| `adjacent_low_db` | number (dB) | optional | 1.0 | The power of the band one adjacent offset below, under the signal; no value outside the spectrum. |
| `adjacent_high_db` | number (dB) | optional | 1.0 | The same above. |
| `alternate_low_db` | number (dB) | optional | 1.0 | The power of the band two adjacent offsets below, under the signal. |
| `alternate_high_db` | number (dB) | optional | 1.0 | The same above. |

### Event `obw.reading`

Struct `ObwReadingEvent`. An evaluation was logged.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the evaluation. |
| `obw_khz` | number (kHz) | always | 1.0 | The band that holds the power share, half the rest below and half above it. |
| `obw_low` | frequency | always | 1.0 | The lower edge of the occupied band. |
| `obw_high` | frequency | always | 1.0 | The upper edge of the occupied band. |
| `bw3_khz` | number (kHz) | always | 1.0 | The width at 3 dB under the peak of the smoothed spectrum. |
| `bw6_khz` | number (kHz) | always | 1.0 | The width at 6 dB under the peak. |
| `bw20_khz` | number (kHz) | always | 1.0 | The width at 20 dB under the peak. |
| `bw26_khz` | number (kHz) | always | 1.0 | The width at 26 dB under the peak. |
| `centre_offset_hz` | number (Hz) | always | 1.0 | The middle of the occupied band from the tuned frequency. |
| `power_db` | number (dB) | always | 1.0 | The power between the edges over the floor, dB re full scale. |
| `snr_db` | number (dB) | always | 1.0 | The signal power over the floor in the same bins. |
| `adjacent_low_db` | number (dB) | optional | 1.0 | The power of the band one adjacent offset below, under the signal; no value outside the spectrum. |
| `adjacent_high_db` | number (dB) | optional | 1.0 | The same above. |
| `alternate_low_db` | number (dB) | optional | 1.0 | The power of the band two adjacent offsets below, under the signal. |
| `alternate_high_db` | number (dB) | optional | 1.0 | The same above. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_obw_clear` | none | none |
| `reset` | `sdrkit_obw_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `power_pct` | `sdrkit_obw_get_power_pct` | `sdrkit_obw_set_power_pct` |
| `x_db` | `sdrkit_obw_get_x_db` | `sdrkit_obw_set_x_db` |
| `average_s` | `sdrkit_obw_get_average_s` | `sdrkit_obw_set_average_s` |
| `adjacent_offset` | `sdrkit_obw_get_adjacent_offset` | `sdrkit_obw_set_adjacent_offset` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_symbol_rate` | `sdrkit_obw_gen_get_symbol_rate` | `sdrkit_obw_gen_set_symbol_rate` |
| `gen_rolloff` | `sdrkit_obw_gen_get_rolloff` | `sdrkit_obw_gen_set_rolloff` |
| `gen_psk` | `sdrkit_obw_gen_get_psk` | `sdrkit_obw_gen_set_psk` |
| `gen_neighbour` | `sdrkit_obw_gen_get_neighbour` | `sdrkit_obw_gen_set_neighbour` |
| `gen_neighbour_db` | `sdrkit_obw_gen_get_neighbour_db` | `sdrkit_obw_gen_set_neighbour_db` |
| `gen_neighbour_offset` | `sdrkit_obw_gen_get_neighbour_offset` | `sdrkit_obw_gen_set_neighbour_offset` |

## Try it

```sh
sdrkit run obw --source synth:obw
sdrkit run obw --source <driver>:<address> --freq 0
```
<!-- generated: end -->
