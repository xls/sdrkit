<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FM modulation monitor

One FM broadcast station once a second: peak and RMS deviation, MPX power, carrier offset, pilot, stereo, RDS and subcarrier levels, and the seconds over a threshold.

Stack id: `fm_monitor`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 100 MHz |
| Tuning range | 64 MHz to 110 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 200 kHz |
| Signal bandwidth | 200 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fm_monitor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `over_deviation_khz` | Over-deviation threshold | number (kHz) | 80 | 50 to 150 | A second whose peak deviation is over this is counted and logged. |
| `deviation_limit` | Peak deviation limit | choice | `75` | `75` (75 kHz), `50` (50 kHz) | The maximum peak deviation of ITU-R BS.450; the pilot is in its usual range at 8 to 10 % of it. |
| `pilot_margin_khz` | Pilot range margin | number (kHz) | 0.5 | 0 to 2 | The reading may lie this far outside 8 to 10 % of the limit and still count as in the usual range. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_audio_khz` | Audio tone deviation | number (kHz) | 30 | 0 to 150 |  |
| `gen_tone2_khz` | Second tone deviation | number (kHz) | 12 | 0 to 150 |  |
| `gen_pilot_khz` | Pilot deviation | number (kHz) | 7.5 | 0 to 150 |  |
| `gen_stereo_khz` | L-R tone deviation | number (kHz) | 15 | 0 to 150 |  |
| `gen_rds_khz` | RDS deviation | number (kHz) | 2 | 0 to 150 |  |
| `gen_sca67_khz` | 67 kHz subcarrier deviation | number (kHz) | 0 | 0 to 150 |  |
| `gen_sca92_khz` | 92 kHz subcarrier deviation | number (kHz) | 0 | 0 to 150 |  |
| `gen_scale` | Multiplex scale | number | 1 | 0 to 4 |  |
| `gen_pilot_from_s` | Pilot from | number (s) | 0 | 0 to 1000000 |  |
| `gen_pilot_until_s` | Pilot until | number (s) | -1 | -1 to 1000000 | A negative value keeps the pilot on. |
| `gen_rds_from_s` | RDS from | number (s) | 0 | 0 to 1000000 |  |
| `gen_rds_until_s` | RDS until | number (s) | -1 | -1 to 1000000 | A negative value keeps RDS on. |
| `gen_sca_from_s` | Subcarriers from | number (s) | 0 | 0 to 1000000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `peak_deviation_khz` | Peak deviation | number (kHz) |
| `rms_deviation_khz` | RMS deviation | number (kHz) |
| `mpx_power_dbr` | MPX power, 1 s | number (dBr) |
| `mpx_power_60s_dbr` | MPX power, 60 s | number (dBr) |
| `carrier_offset_khz` | Carrier offset | number (kHz) |
| `audio_khz` | Audio (L+R) | number (kHz) |
| `pilot_khz` | Pilot | number (kHz) |
| `pilot_pct` | Pilot share of the limit | number (%) |
| `pilot` | Pilot range | choice |
| `stereo_band_khz` | Stereo (L-R) band | number (kHz) |
| `stereo` | Stereo | yes or no |
| `rds_khz` | RDS injection | number (kHz) |
| `rds` | RDS | yes or no |
| `sca67_db` | 67 kHz subcarrier | number (dB) |
| `sca92_db` | 92 kHz subcarrier | number (dB) |
| `over_deviation_s` | Seconds over the threshold | whole number |
| `level_db` | Level | number (dB) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `log` | Log | `signal_time_s` (number (s)), `kind` (text), `identity` (text), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `fm_monitor.pilot` | Pilot | `text` (text), `change` (choice), `pilot_khz` (number (kHz)) |
| `fm_monitor.rds` | RDS | `text` (text), `change` (choice), `rds_khz` (number (kHz)) |
| `fm_monitor.sca` | Subcarrier | `text` (text), `subcarrier_khz` (number (kHz)), `on` (yes or no), `level_db` (number (dB)) |
| `fm_monitor.over_deviation` | Over-deviation | `text` (text), `peak_deviation_khz` (number (kHz)), `mpx_power_dbr` (number (dBr)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `meter`, version 1.1. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

### Result fields

Struct `FmMonitorFields`. The result fields of `fm_monitor`, measured once a second from the FM-demodulated multiplex (MPX).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The headline: stereo or mono, the peak, the pilot and RDS levels. |
| `peak_deviation_khz` | number (kHz) | optional | 1.0 | The peak deviation of the last second: the largest third-largest excursion of its 5 ms blocks. |
| `rms_deviation_khz` | number (kHz) | optional | 1.0 | The RMS deviation. |
| `mpx_power_dbr` | number (dBr) | optional | 1.0 | The MPX power of the last second (0 dBr is a sine of 19 kHz deviation). |
| `mpx_power_60s_dbr` | number (dBr) | optional | 1.0 | The MPX power over the last 60 s in steps of 1 s, as ITU-R BS.412 integrates it; no value before 60 s. |
| `carrier_offset_khz` | number (kHz) | optional | 1.0 | The carrier offset. |
| `audio_khz` | number (kHz) | optional | 1.0 | The audio (L+R), peak equivalent. |
| `pilot_khz` | number (kHz) | optional | 1.0 | The 19 kHz pilot, peak equivalent. |
| `pilot_pct` | number (%) | optional | 1.0 | The pilot as a share of the deviation limit. |
| `pilot` | choice | always | 1.0 | The pilot against its usual range. |
| `stereo_band_khz` | number (kHz) | optional | 1.0 | The stereo (L-R) band, peak equivalent. |
| `stereo` | yes or no | always | 1.0 | Stereo: a pilot and an L-R band over 0.5 kHz. |
| `rds_khz` | number (kHz) | optional | 1.0 | The RDS injection, peak equivalent. |
| `rds` | yes or no | always | 1.0 | RDS is present. |
| `sca67_db` | number (dB) | optional | 1.0 | The 67 kHz subcarrier over the floor; no value when absent. |
| `sca92_db` | number (dB) | optional | 1.0 | The 92 kHz subcarrier over the floor; no value when absent or below a working rate of 250 ksps. |
| `over_deviation_s` | whole number | always | 1.0 | The seconds whose peak was over the threshold. |
| `level_db` | number (dB) | optional | 1.0 | The input level. |
| `note` | text | always | 1.0 | What the monitor measures. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `log`

Struct `FmMonitorLogRow`. A row of the log of `fm_monitor`. The row key is the sequence number of the row.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time_s` | number (s) | always | 1.0 | The signal time of the row. |
| `kind` | text | always | 1.0 | The kind: `Pilot`, `RDS`, `SCA` or `Over-deviation`. |
| `identity` | text | always | 1.0 | What changed, or the peak of an over-deviation. |
| `text` | text | always | 1.0 | The text of the row. |

The column `identity` names a row.

### Event `fm_monitor.pilot`

Struct `FmMonitorPilotEvent`. The pilot of an FM station was present or absent at the first measurement, came or went.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `change` | choice | always | 1.0 | What changed. |
| `pilot_khz` | number (kHz) | always | 1.0 | The pilot of the second, peak equivalent. |

### Event `fm_monitor.rds`

Struct `FmMonitorRdsEvent`. RDS came or went.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `change` | choice | always | 1.0 | What changed. |
| `rds_khz` | number (kHz) | always | 1.0 | The RDS injection of the second, peak equivalent. |

### Event `fm_monitor.sca`

Struct `FmMonitorScaEvent`. A subcarrier at 67 or 92 kHz came or went.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `subcarrier_khz` | number (kHz) | always | 1.0 | The subcarrier, 67 or 92. |
| `on` | yes or no | always | 1.0 | The subcarrier came (true) or went. |
| `level_db` | number (dB) | optional | 1.0 | Its level over the floor in that second. |

### Event `fm_monitor.over_deviation`

Struct `FmMonitorOverDeviationEvent`. A second whose peak deviation was over the threshold.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `peak_deviation_khz` | number (kHz) | always | 1.0 | The peak deviation of the second. |
| `mpx_power_dbr` | number (dBr) | always | 1.0 | The MPX power of the second. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fm_monitor_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `over_deviation_khz` | `sdrkit_fm_monitor_get_over_deviation_khz` | `sdrkit_fm_monitor_set_over_deviation_khz` |
| `deviation_limit` | `sdrkit_fm_monitor_get_deviation_limit` | `sdrkit_fm_monitor_set_deviation_limit` |
| `pilot_margin_khz` | `sdrkit_fm_monitor_get_pilot_margin_khz` | `sdrkit_fm_monitor_set_pilot_margin_khz` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_audio_khz` | `sdrkit_fm_monitor_gen_get_audio_khz` | `sdrkit_fm_monitor_gen_set_audio_khz` |
| `gen_tone2_khz` | `sdrkit_fm_monitor_gen_get_tone2_khz` | `sdrkit_fm_monitor_gen_set_tone2_khz` |
| `gen_pilot_khz` | `sdrkit_fm_monitor_gen_get_pilot_khz` | `sdrkit_fm_monitor_gen_set_pilot_khz` |
| `gen_stereo_khz` | `sdrkit_fm_monitor_gen_get_stereo_khz` | `sdrkit_fm_monitor_gen_set_stereo_khz` |
| `gen_rds_khz` | `sdrkit_fm_monitor_gen_get_rds_khz` | `sdrkit_fm_monitor_gen_set_rds_khz` |
| `gen_sca67_khz` | `sdrkit_fm_monitor_gen_get_sca67_khz` | `sdrkit_fm_monitor_gen_set_sca67_khz` |
| `gen_sca92_khz` | `sdrkit_fm_monitor_gen_get_sca92_khz` | `sdrkit_fm_monitor_gen_set_sca92_khz` |
| `gen_scale` | `sdrkit_fm_monitor_gen_get_scale` | `sdrkit_fm_monitor_gen_set_scale` |
| `gen_pilot_from_s` | `sdrkit_fm_monitor_gen_get_pilot_from_s` | `sdrkit_fm_monitor_gen_set_pilot_from_s` |
| `gen_pilot_until_s` | `sdrkit_fm_monitor_gen_get_pilot_until_s` | `sdrkit_fm_monitor_gen_set_pilot_until_s` |
| `gen_rds_from_s` | `sdrkit_fm_monitor_gen_get_rds_from_s` | `sdrkit_fm_monitor_gen_set_rds_from_s` |
| `gen_rds_until_s` | `sdrkit_fm_monitor_gen_get_rds_until_s` | `sdrkit_fm_monitor_gen_set_rds_until_s` |
| `gen_sca_from_s` | `sdrkit_fm_monitor_gen_get_sca_from_s` | `sdrkit_fm_monitor_gen_set_sca_from_s` |

## Try it

```sh
sdrkit run fm_monitor --source synth:fm_monitor
sdrkit run fm_monitor --source iiod:192.168.2.1 --freq 100
```
<!-- generated: end -->
