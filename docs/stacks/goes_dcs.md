<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GOES DCS

GOES / Meteosat data collection platforms at 100, 300 and 1200 bit/s: address, sensor data, platform table. Receive only.

Stack id: `goes_dcs`. Group: Satellite.

Signal types: Data collection platform.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 401.85 MHz |
| Tuning range | 401 MHz to 403 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 500 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:goes_dcs` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Carrier threshold | number (dB) | 11 | 6 to 20 | A carrier line must stand this far over the local noise floor. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scenario` | Scenario | choice | `0` | `0` (Three platforms (100, 300, 1200 bit/s Manchester)), `1` (Only the 300 bit/s platform), `2` (One of each rate, 300 and 1200 as NRZ), `3` (Three platforms and a steady carrier) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `carriers` | Carriers | whole number |
| `dropped` | Dropped | whole number |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `reports` | Reports | `at` (duration (s)), `heard_at` (time), `address` (text), `bit_rate_hz` (frequency), `manchester` (yes or no), `channel` (whole number), `snr_db` (number (dB)), `flags` (whole number), `format` (choice), `eot` (yes or no), `parity_errors` (whole number), `text` (text) |
| `platforms` | Platforms | `address` (text), `channel` (whole number), `bit_rate_hz` (frequency), `snr_db` (number (dB)), `format` (choice), `text` (text), `last_heard` (time), `heard` (whole number) |
| `carriers` | Carriers | `offset_hz` (frequency), `channel` (whole number), `snr_db` (number (dB)), `bit_rate_hz` (frequency) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Band | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `goes_dcs.report` | Platform report | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `address` (text), `channel` (whole number), `bit_rate_hz` (frequency), `manchester` (yes or no), `flags` (whole number), `format` (choice), `eot` (yes or no), `parity_errors` (whole number), `bits_corrected` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `satcom`, version 1.4. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite links (`satellite_links`). Media: none.

### Result fields

Struct `GoesDcsFields`. The result fields of GOES DCS.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | The reports read whole. |
| `frames_bad` | whole number | always | 1.0 | The reports cut off or lost. |
| `carriers` | whole number | always | 1.0 | The carriers followed now. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `reports`

Struct `GoesDcsReportRow`. A platform report. The row key is the sequence number of the report.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time of a report. |
| `heard_at` | time | optional | 1.0 | The wall-clock time of a report (live radios only). |
| `address` | text | always | 1.0 | The address word (8 hex digits). |
| `bit_rate_hz` | frequency | always | 1.0 | The bit rate. |
| `manchester` | yes or no | always | 1.0 | Manchester coded. |
| `channel` | whole number | always | 1.0 | The channel number. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `flags` | whole number | always | 1.0 | The flag byte, or -1. |
| `format` | choice | always | 1.0 | The data format. |
| `eot` | yes or no | always | 1.0 | The report ended with an EOT. |
| `parity_errors` | whole number | always | 1.0 | Characters whose parity failed. |
| `text` | text | always | 1.0 | The data of the report. |

The column `address` names a row.

### Table `platforms`

Struct `GoesDcsPlatformRow`. A platform heard. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address word (8 hex digits). |
| `channel` | whole number | always | 1.0 | The channel number. |
| `bit_rate_hz` | frequency | always | 1.0 | The bit rate. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `format` | choice | always | 1.0 | The data format. |
| `text` | text | always | 1.0 | The data of the report. |
| `last_heard` | time | optional | 1.0 | When it was last heard. |
| `heard` | whole number | optional | 1.0 | How many times it was heard. |

The column `address` names a row.

### Table `carriers`

Struct `GoesDcsCarrierRow`. A carrier followed now. The row key is its place in the list ("000").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset_hz` | frequency | always | 1.0 | The offset of the carrier from the tuned frequency. |
| `channel` | whole number | always | 1.0 | The channel number. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `bit_rate_hz` | frequency | optional | 1.0 | The bit rate. |

Default order: the column `snr_db`, highest first.

### Event `goes_dcs.report`

Struct `GoesDcsReportEvent`. A platform report. The source is the address, the text the data.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The address of the platform. |
| `text` | text | always | 1.0 | The data of the report. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `address` | text | always | 1.0 | The address word (8 hex digits). |
| `channel` | whole number | always | 1.0 | The channel number. |
| `bit_rate_hz` | frequency | always | 1.0 | The bit rate. |
| `manchester` | yes or no | always | 1.0 | Manchester coded. |
| `flags` | whole number | always | 1.0 | The flag byte, or -1. |
| `format` | choice | always | 1.0 | The data format. |
| `eot` | yes or no | always | 1.0 | The report ended with an EOT. |
| `parity_errors` | whole number | always | 1.0 | Characters whose parity failed. |
| `bits_corrected` | whole number | always | 1.0 | The address bits corrected. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_goes_dcs_clear` | none | none |
| `remove` | `sdrkit_goes_dcs_remove` | `sdrkit_goes_dcs_platform_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_goes_dcs_get_threshold_db` | `sdrkit_goes_dcs_set_threshold_db` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scenario` | `sdrkit_goes_dcs_gen_get_scenario` | `sdrkit_goes_dcs_gen_set_scenario` |

## Try it

```sh
sdrkit run goes_dcs --source synth:goes_dcs
sdrkit run goes_dcs --source iiod:192.168.2.1 --freq 401.85
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
