<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Narrowband FM deviation and CTCSS meter

Voice peak deviation, the CTCSS tone and its deviation, the carrier offset and the windows over the deviation limits.

Stack id: `nfm_meter`. Group: Measurement.

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

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:nfm_meter` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `limit_narrow_hz` | Narrow channel deviation limit | number (Hz) | 2500 | 1000 to 10000 | A regional limit for 12.5 kHz channels. |
| `limit_wide_hz` | Wide channel deviation limit | number (Hz) | 5000 | 1000 to 10000 | A regional limit for 25 kHz channels. |
| `peak_percentile` | Voice peak percentile | number (%) | 99.9 | 99 to 100 |  |
| `ctcss_min_db` | CTCSS over its neighbourhood | number (dB) | 14.8 | 6 to 30 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_voice_hz` | Voice scale | number (Hz) | 1600 | 0 to 20000 |  |
| `gen_ctcss_hz` | CTCSS tone | number (Hz) | 100 | 0 to 300 |  |
| `gen_ctcss_dev_hz` | CTCSS deviation | number (Hz) | 400 | 0 to 5000 |  |
| `gen_ctcss_off_s` | CTCSS off at | number (s) | 0 | 0 to 1000000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `good` | Good readings | whole number |
| `bad` | Bad readings | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `voice_peak_khz` | Voice peak deviation | number (kHz) |
| `voice_peak_max_khz` | Largest voice peak | number (kHz) |
| `over_narrow` | Windows over the narrow limit | whole number |
| `over_wide` | Windows over the wide limit | whole number |
| `windows` | Windows | whole number |
| `ctcss_hz` | CTCSS tone | frequency |
| `ctcss_standard_hz` | Nearest standard tone | frequency |
| `ctcss_error_hz` | CTCSS error | frequency |
| `ctcss_deviation_hz` | CTCSS deviation | frequency |
| `ctcss_share_narrow_pct` | CTCSS of the narrow limit | number (%) |
| `ctcss_share_wide_pct` | CTCSS of the wide limit | number (%) |
| `carrier_offset_hz` | Carrier offset | frequency |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `time_s` (duration (s)), `kind` (choice), `ctcss_standard_hz` (frequency), `ctcss_hz` (frequency), `ctcss_deviation_hz` (frequency), `voice_peak_khz` (number (kHz)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, averages and floors, as after a retune; the tables stay. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `voice_peak_khz`, `voice_peak_max_khz`, `ctcss_hz`, `ctcss_deviation_hz`, `carrier_offset_hz`.

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

Struct `NfmMeterFields`. The result fields of `nfm_meter`, from the last window. The voice readings have no value without a carrier, the CTCSS readings none without a tone.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `voice_peak_khz` | number (kHz) | optional | 1.0 | The 99.9th percentile of the voice band deviation of the window. |
| `voice_peak_max_khz` | number (kHz) | optional | 1.0 | The largest voice peak so far. |
| `over_narrow` | whole number | always | 1.0 | Windows over the narrow channel limit. |
| `over_wide` | whole number | always | 1.0 | Windows over the wide channel limit. |
| `windows` | whole number | always | 1.0 | Windows measured. |
| `ctcss_hz` | frequency | optional | 1.0 | The CTCSS tone measured. |
| `ctcss_standard_hz` | frequency | optional | 1.0 | The nearest standard tone. |
| `ctcss_error_hz` | frequency | optional | 1.0 | Measured minus standard. |
| `ctcss_deviation_hz` | frequency | optional | 1.0 | The peak deviation of the CTCSS tone. |
| `ctcss_share_narrow_pct` | number (%) | optional | 1.0 | The CTCSS deviation as a share of the narrow limit. |
| `ctcss_share_wide_pct` | number (%) | optional | 1.0 | The CTCSS deviation as a share of the wide limit. |
| `carrier_offset_hz` | frequency | optional | 1.0 | The carrier offset: the mean of the discriminator audio. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `events`

Struct `NfmMeterEventRow`. A row of the `nfm_meter` log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time. |
| `kind` | choice | always | 1.0 | The kind. |
| `ctcss_standard_hz` | frequency | optional | 1.0 | The nearest standard tone. |
| `ctcss_hz` | frequency | optional | 1.0 | The CTCSS tone measured. |
| `ctcss_deviation_hz` | frequency | optional | 1.0 | The CTCSS deviation. |
| `voice_peak_khz` | number (kHz) | optional | 1.0 | The voice peak of the window. |

The rows are a log: they arrive over time. Time: `time_s`.

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_nfm_meter_clear` | none | none |
| `reset` | `sdrkit_nfm_meter_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `limit_narrow_hz` | `sdrkit_nfm_meter_get_limit_narrow_hz` | `sdrkit_nfm_meter_set_limit_narrow_hz` |
| `limit_wide_hz` | `sdrkit_nfm_meter_get_limit_wide_hz` | `sdrkit_nfm_meter_set_limit_wide_hz` |
| `peak_percentile` | `sdrkit_nfm_meter_get_peak_percentile` | `sdrkit_nfm_meter_set_peak_percentile` |
| `ctcss_min_db` | `sdrkit_nfm_meter_get_ctcss_min_db` | `sdrkit_nfm_meter_set_ctcss_min_db` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_voice_hz` | `sdrkit_nfm_meter_gen_get_voice_hz` | `sdrkit_nfm_meter_gen_set_voice_hz` |
| `gen_ctcss_hz` | `sdrkit_nfm_meter_gen_get_ctcss_hz` | `sdrkit_nfm_meter_gen_set_ctcss_hz` |
| `gen_ctcss_dev_hz` | `sdrkit_nfm_meter_gen_get_ctcss_dev_hz` | `sdrkit_nfm_meter_gen_set_ctcss_dev_hz` |
| `gen_ctcss_off_s` | `sdrkit_nfm_meter_gen_get_ctcss_off_s` | `sdrkit_nfm_meter_gen_set_ctcss_off_s` |

## Try it

```sh
sdrkit run nfm_meter --source synth:nfm_meter
sdrkit run nfm_meter --source <driver>:<address> --freq 0
```
<!-- generated: end -->
