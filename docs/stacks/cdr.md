<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# CDR

China Digital Radio (GY/T 268) in the FM band: the multiplex, its services, the network table and DRA+ audio units.

Stack id: `cdr`. Group: Digital radio.

Signal types: CDR.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 87 MHz to 108 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 768 kHz |
| Signal bandwidth | 500 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:cdr` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `cdr_tables` | GY/T 268.1 tables | text | empty |  | A JSON file with the LDPC matrices and the bit interleavers of GY/T 268.1. Without it nothing is decoded. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tx_mode` | Transmission mode | choice | `1` | `1`, `2`, `3` |  |
| `gen_spectrum_mode` | Spectrum mode | choice | `1` | `1`, `2`, `9`, `10`, `22`, `23` |  |
| `gen_data_qam` | Data | choice | `4` | `4` (QPSK), `16` (16-QAM), `64` (64-QAM) |  |
| `gen_ldpc_rate` | LDPC rate | choice | `3` | `0` (1/4), `1` (1/3), `2` (1/2), `3` (3/4) |  |
| `gen_sdi_qam` | SDI | choice | `4` | `4` (QPSK), `16` (16-QAM), `64` (64-QAM) |  |
| `gen_allocation` | Allocation | choice | `1` | `1`, `2`, `3` |  |
| `gen_fm_programme` | FM programme | yes or no | true |  |  |
| `gen_text_service` | Text service | yes or no | true |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `tables` | GY/T 268.1 tables | text |
| `tx_mode` | Transmission mode | whole number |
| `spectrum_mode` | Spectrum mode | whole number |
| `spectrum` | Spectrum | text |
| `ldpc_rate` | LDPC rate | text |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `subframes` | Sub-frames | whole number |
| `si_ok` | SI good | whole number |
| `si_bad` | SI bad | whole number |
| `cw_ok` | Code words good | whole number |
| `cw_bad` | Code words bad | whole number |
| `iterations` | LDPC iterations | number |
| `control_ok` | Control frames good | whole number |
| `control_bad` | Control frames bad | whole number |
| `service_ok` | Service frames good | whole number |
| `service_bad` | Service frames bad | whole number |
| `network` | Network | text |
| `country` | Country | text |
| `network_id` | Network id | whole number |
| `selected_service` | Service playing | text |
| `units_in` | DRA+ units | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `services` | Services | `sid` (whole number), `audio` (yes or no), `data` (yes or no), `sample_rate` (whole number (Hz)), `channels` (whole number), `language` (text), `algorithm` (whole number), `kbps` (number (kbit/s)), `units` (whole number), `text` (text), `good` (whole number), `bad` (whole number) |
| `service_data_types` | Data types | `service` (text), `data_type` (whole number) |
| `network_frequencies` | Network frequencies | `freq_hz` (frequency) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `si_constellation` | SI constellation |  |  |
| `msc_constellation` | MSDS constellation |  |  |
| `channel` | Channel | kHz | dB |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `service.select` | Select a service | `row` (text, required) |  | Plays a service of the multiplex. An empty row plays the first service that can play. |
| `clear` | Clear |  |  | Forgets the signal and starts again. |
| `resync` | Clear |  |  | Forgets the signal and starts again. |

## Typed interface

The stack is in the family `broadcast`, version 2.0. The C header `sdrkit/broadcast.h`, the registry record `registry/broadcast.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Broadcast radio (`broadcast`). Media: sound (programme).

### Result fields

Struct `CdrFields`. The results of the CDR stack. No transmission mode, spectrum or LDPC rate before the system information, no offset or SNR before the first sub-frame, no level before the first report of the decoder, no iterations before the first code word and no network before the network information table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tables` | text | always | 1.0 | Whether the GY/T 268.1 tables are loaded ("loaded"), or what is missing. |
| `tx_mode` | whole number | optional | 1.0 | The transmission mode (1 to 3). |
| `spectrum_mode` | whole number | optional | 1.0 | The spectrum mode. |
| `spectrum` | text | optional | 1.0 | The spectrum mode in words. |
| `ldpc_rate` | text | optional | 1.0 | The LDPC rate of the service data ("3/4"). |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR (Es/N0). |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level. |
| `subframes` | whole number | always | 1.0 | Sub-frames received. |
| `si_ok` | whole number | always | 1.0 | System information good. |
| `si_bad` | whole number | always | 1.0 | System information bad. |
| `cw_ok` | whole number | always | 1.0 | LDPC code words good. |
| `cw_bad` | whole number | always | 1.0 | LDPC code words bad. |
| `iterations` | number | optional | 1.0 | Mean LDPC iterations. |
| `control_ok` | whole number | always | 1.0 | Control multiplex frames good. |
| `control_bad` | whole number | always | 1.0 | Control multiplex frames bad. |
| `service_ok` | whole number | always | 1.0 | Service multiplex frames good. |
| `service_bad` | whole number | always | 1.0 | Service multiplex frames bad. |
| `network` | text | optional | 1.0 | The network name. |
| `country` | text | optional | 1.0 | The country of the network. |
| `network_id` | whole number | optional | 1.0 | The network id. |
| `selected_service` | text | optional | 1.0 | The row key of the service playing in [`SERVICES`]. |
| `units_in` | whole number | always | 1.0 | DRA+ units of the service listened to. |

### Table `services`

Struct `CdrServiceRow`. A service of the multiplex. The row key is the service id in hexadecimal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sid` | whole number | always | 1.0 | The service id. |
| `audio` | yes or no | always | 1.0 | Audio units seen. |
| `data` | yes or no | always | 1.0 | Data units seen. |
| `sample_rate` | whole number (Hz) | optional | 1.0 | The sample rate of the first audio stream. |
| `channels` | whole number | optional | 1.0 | Channels of the first audio stream. |
| `language` | text | optional | 1.0 | The language of the first audio stream. |
| `algorithm` | whole number | optional | 1.0 | The audio algorithm code of the first stream. |
| `kbps` | number (kbit/s) | always | 1.0 | The share of the multiplex. |
| `units` | whole number | always | 1.0 | Audio units in the last sub-frame. |
| `text` | text | optional | 1.0 | The last data unit, when it is printable text. |
| `good` | whole number | always | 1.0 | Sub-frame headers good. |
| `bad` | whole number | always | 1.0 | Sub-frame headers or units bad. |

### Table `service_data_types`

Struct `CdrDataTypeRow`. A data unit type of a service. The row key is `<service>/<type>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `service` | text | always | 1.0 | The row key of the service. |
| `data_type` | whole number | always | 1.0 | The data unit type. |

### Table `network_frequencies`

Struct `CdrFrequencyRow`. A frequency of the network information table. The row key is its place in the table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `freq_hz` | frequency | always | 1.0 | The frequency. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `service.select` | `sdrkit_cdr_service_select` | `sdrkit_broadcast_service_select_args` | none |
| `clear` | `sdrkit_cdr_clear` | none | none |
| `resync` | `sdrkit_cdr_resync` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `cdr_tables` | `sdrkit_cdr_get_cdr_tables` | `sdrkit_cdr_set_cdr_tables` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tx_mode` | `sdrkit_cdr_gen_get_tx_mode` | `sdrkit_cdr_gen_set_tx_mode` |
| `gen_spectrum_mode` | `sdrkit_cdr_gen_get_spectrum_mode` | `sdrkit_cdr_gen_set_spectrum_mode` |
| `gen_data_qam` | `sdrkit_cdr_gen_get_data_qam` | `sdrkit_cdr_gen_set_data_qam` |
| `gen_ldpc_rate` | `sdrkit_cdr_gen_get_ldpc_rate` | `sdrkit_cdr_gen_set_ldpc_rate` |
| `gen_sdi_qam` | `sdrkit_cdr_gen_get_sdi_qam` | `sdrkit_cdr_gen_set_sdi_qam` |
| `gen_allocation` | `sdrkit_cdr_gen_get_allocation` | `sdrkit_cdr_gen_set_allocation` |
| `gen_fm_programme` | `sdrkit_cdr_gen_get_fm_programme` | `sdrkit_cdr_gen_set_fm_programme` |
| `gen_text_service` | `sdrkit_cdr_gen_get_text_service` | `sdrkit_cdr_gen_set_text_service` |

## Try it

```sh
sdrkit run cdr --source synth:cdr
sdrkit run cdr --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
