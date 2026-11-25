<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# AM modulation monitor

One AM broadcast station once a second: carrier offset and level, modulation depth, peaks, asymmetry, mains hum, audio bandwidth and over-modulation.

Stack id: `am_monitor`. Group: Measurement.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1 MHz |
| Tuning range | 148 kHz to 30 MHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 12 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:am_monitor` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `over_mod_share_pct` | Over-modulation: share of a second at zero carrier | number (%) | 0.5 | 0.1 to 5 | A second with more of its blocks at zero carrier than this is counted and logged. |
| `carrier_loss_db` | Carrier lost below the best level by | number (dB) | 12 | 6 to 30 | The carrier counts as lost when its level falls this far below the best level of about the last minute. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_depth1` | First tone depth | number | 0.5 | 0 to 2 | The peak of the first tone as a share of the carrier. |
| `gen_depth2` | Second tone depth | number | 0.2 | 0 to 2 | The peak of the second tone as a share of the carrier. |
| `gen_tone1_hz` | First tone | number (Hz) | 1000 | 20 to 10000 |  |
| `gen_tone2_hz` | Second tone | number (Hz) | 3300 | 20 to 10000 |  |
| `gen_asymmetry` | Asymmetry | number | 0 | 0 to 2 | The positive half of the modulation is raised by this share of itself. |
| `gen_cut_from_s` | Carrier cut from | number (s) | -1 | -1 to 1000000 | The carrier is off from this time; a negative value is no cut. |
| `gen_cut_until_s` | Carrier cut until | number (s) | -1 | -1 to 1000000 | The carrier is back from this time. |
| `gen_clip` | Clip at zero carrier | yes or no | true |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `carrier_offset_hz` | Carrier offset | number (Hz) |
| `carrier_db` | Carrier level | number (dB) |
| `depth_pct` | Modulation (RMS, as a sine) | number (%) |
| `peak_pos_pct` | Positive peak | number (%) |
| `peak_neg_pct` | Negative peak | number (%) |
| `asymmetry_pct` | Asymmetry | number (%) |
| `hum_db` | Mains hum below the carrier | number (dB) |
| `hum_hz` | Mains hum frequency | number (Hz) |
| `bandwidth_khz` | Audio bandwidth | number (kHz) |
| `over_mod_pct` | Share of the last second at zero carrier | number (%) |
| `over_mod_s` | Seconds over-modulated | whole number |
| `carrier` | Carrier present | yes or no |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `log` | Log | `signal_time_s` (number (s)), `kind` (text), `identity` (text), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `am_monitor.carrier` | Carrier | `text` (text), `change` (choice), `carrier_offset_hz` (number (Hz)), `carrier_db` (number (dB)) |
| `am_monitor.over_modulation` | Over-modulation | `text` (text), `over_mod_pct` (number (%)), `peak_pos_pct` (number (%)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `meter`, version 2.0. The C header `sdrkit/meter.h`, the registry record `registry/meter.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Measurement and analysis (`measurement`). Media: none.

Key readings: `depth_pct`, `peak_pos_pct`, `peak_neg_pct`, `carrier_offset_hz`, `carrier_db`.

### Result fields

Struct `AmMonitorFields`. The result fields of `am_monitor`, measured once a second. The measurements have no value while the carrier test fails.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carrier_offset_hz` | number (Hz) | optional | 1.0 | The carrier offset: a tuning or station error. |
| `carrier_db` | number (dB) | optional | 1.0 | The carrier level, relative to full scale. |
| `depth_pct` | number (%) | optional | 1.0 | The modulation depth: the depth of a sine of the same RMS. |
| `peak_pos_pct` | number (%) | optional | 1.0 | The positive peak against the mean carrier. |
| `peak_neg_pct` | number (%) | optional | 1.0 | The negative peak against the mean carrier (at most 100 %). |
| `asymmetry_pct` | number (%) | optional | 1.0 | The asymmetry: (positive - negative) / (positive + negative). |
| `hum_db` | number (dB) | optional | 1.0 | The mains hum below the carrier; no value when no hum line is seen. |
| `hum_hz` | number (Hz) | optional | 1.0 | The frequency of the mains hum line. |
| `bandwidth_khz` | number (kHz) | optional | 1.0 | The audio bandwidth: 99 % of the audio power lies below it. |
| `over_mod_pct` | number (%) | optional | 1.0 | The share of the last second at zero carrier. |
| `over_mod_s` | whole number | always | 1.0 | The seconds that were over-modulated. |
| `carrier` | yes or no | always | 1.0 | The carrier is present (the carrier test and the level rule). |
| `note` | text | always | 1.0 | What the monitor measures. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `log`

Struct `AmMonitorLogRow`. A row of the log of `am_monitor`. The row key is the sequence number of the row.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time_s` | number (s) | always | 1.0 | The signal time of the row. |
| `kind` | text | always | 1.0 | The kind: `Carrier` or `Over-modulation`. |
| `identity` | text | always | 1.0 | What changed: `present`, `lost`, `returned`, or the share of the second at zero carrier. |
| `text` | text | always | 1.0 | The text of the row. |

The rows are a log: they arrive over time. The column `identity` names a row. Time: `signal_time_s`.

### Event `am_monitor.carrier`

Struct `AmMonitorCarrierEvent`. The carrier of an AM station was seen, was lost or came back.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `change` | choice | always | 1.0 | What changed. |
| `carrier_offset_hz` | number (Hz) | always | 1.0 | The carrier offset of the second. |
| `carrier_db` | number (dB) | always | 1.0 | The carrier level of the second. |

### Event `am_monitor.over_modulation`

Struct `AmMonitorOverModulationEvent`. A second of an AM station was over-modulated.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the event. |
| `over_mod_pct` | number (%) | always | 1.0 | The share of the second at zero carrier. |
| `peak_pos_pct` | number (%) | always | 1.0 | The positive peak of the second. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_am_monitor_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `over_mod_share_pct` | `sdrkit_am_monitor_get_over_mod_share_pct` | `sdrkit_am_monitor_set_over_mod_share_pct` |
| `carrier_loss_db` | `sdrkit_am_monitor_get_carrier_loss_db` | `sdrkit_am_monitor_set_carrier_loss_db` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_depth1` | `sdrkit_am_monitor_gen_get_depth1` | `sdrkit_am_monitor_gen_set_depth1` |
| `gen_depth2` | `sdrkit_am_monitor_gen_get_depth2` | `sdrkit_am_monitor_gen_set_depth2` |
| `gen_tone1_hz` | `sdrkit_am_monitor_gen_get_tone1_hz` | `sdrkit_am_monitor_gen_set_tone1_hz` |
| `gen_tone2_hz` | `sdrkit_am_monitor_gen_get_tone2_hz` | `sdrkit_am_monitor_gen_set_tone2_hz` |
| `gen_asymmetry` | `sdrkit_am_monitor_gen_get_asymmetry` | `sdrkit_am_monitor_gen_set_asymmetry` |
| `gen_cut_from_s` | `sdrkit_am_monitor_gen_get_cut_from_s` | `sdrkit_am_monitor_gen_set_cut_from_s` |
| `gen_cut_until_s` | `sdrkit_am_monitor_gen_get_cut_until_s` | `sdrkit_am_monitor_gen_set_cut_until_s` |
| `gen_clip` | `sdrkit_am_monitor_gen_get_clip` | `sdrkit_am_monitor_gen_set_clip` |

## Try it

```sh
sdrkit run am_monitor --source synth:am_monitor
sdrkit run am_monitor --source <driver>:<address> --freq 1
```
<!-- generated: end -->
