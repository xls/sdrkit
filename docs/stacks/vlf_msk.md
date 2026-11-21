<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# VLF MSK transmitter monitor

Carrier and bit rate of a VLF MSK station from the two lines of the squared signal; on, off and change events, a hint of the listed station. A monitor: the traffic is not decoded.

Stack id: `vlf_msk`. Group: HF utility.

The VLF MSK transmitter monitor: the carrier and the bit rate of an MSK station in 10 to 30 kHz from the two lines of the squared signal, and the events when a transmitter comes on, goes off or changes its rate. It measures only: the traffic is encrypted and is not decoded. encryption of the traffic are those of the Wikipedia articles "Very low frequency" and "VLF Transmitter Cutler"; the carriers of the stations come from the host's data file.

Standard: no issuing body publishes the signals of the naval VLF stations. The use of MSK at up to 300 bit/s and the

## Tuning

| Property | Value |
|---|---|
| Default frequency | 15 kHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:vlf_msk` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `report_s` | Report time | duration (s) | 4 s | 2 to 16 | A state must hold for two reports before it is taken. |
| `rates` | Standard rates | text (bit/s) | `50,75,100,150,200,300` |  | A measured rate within the tolerance of one of these is shown as it. |
| `rate_tolerance` | Rate tolerance | number (%) | 8 | 2 to 20 |  |
| `hint_hz` | Station hint window | frequency | 60 Hz | 0 Hz to 500 Hz |  |
| `stations` | Station list | text | empty |  | frequency_hz=name pairs separated by semicolons, from the host's data file. Empty: no hint. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_rate` | Bit rate | number (bit/s) | 200 | 25 to 600 |  |
| `gen_fc_hz` | Carrier offset | frequency | 0 Hz | -1.5 kHz to 1.5 kHz |  |
| `gen_from_s` | On from | duration (s) | 0 s | 0 to 3600 |  |
| `gen_to_s` | Off at | duration (s) | 0 s | 0 to 3600 | 0: on to the end. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `state` | State | whole number |
| `signal_class` | Class | text |
| `carrier_hz` | Carrier | frequency |
| `bit_rate` | Bit rate | number (bit/s) |
| `line_spacing_hz` | Line spacing | frequency |
| `level_db` | Level over the noise | number (dB) |
| `hint` | Station hint | text |
| `transmitters` | Transmitters | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `rows` | Rows | `time` (time), `kind` (text), `id` (text), `text` (text), `snr` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `vlf.transmitter` | Transmitter | `text` (text), `kind` (text), `rate` (number (bit/s)), `carrier` (frequency), `level` (number (dB)), `hint` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.2. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `VlfMskFields`. The result fields of the VLF MSK monitor. The measures are absent while no transmitter is on.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What the stack does not do and what no public document confirms. |
| `state` | whole number | always | 1.0 | The state: 0 off air or too weak, 1 carrier only, 2 MSK. |
| `signal_class` | text | always | 1.0 | The class: "MSK 200 bit/s", "MSK about N bit/s", "carrier only" or "off air or too weak". |
| `carrier_hz` | frequency | optional | 1.0 | The carrier from the tuned frequency. |
| `bit_rate` | number (bit/s) | optional | 1.0 | The bit rate. |
| `line_spacing_hz` | frequency | optional | 1.0 | The spacing of the two lines of the squared signal. |
| `level_db` | number (dB) | optional | 1.0 | The level over the noise. |
| `hint` | text | optional | 1.0 | The listed station near the carrier: "likely NAME". |
| `transmitters` | whole number | always | 1.0 | Transmitters that came on. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel, dBFS. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `rows`

Struct `SenderRow`. A row of a number-sender stack: one thing read. The row key is the count of rows before it, as six digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | always | 1.0 | The time it was read. |
| `kind` | text | always | 1.0 | What it is: start, message start, groups, block sync, break, end, sync, text. |
| `id` | text | always | 1.0 | The signal and speed it was read as. |
| `text` | text | always | 1.0 | What was read. |
| `snr` | number (dB) | optional | 1.0 | The SNR in 3 kHz of the last fit of the tones. Absent without a fit. |

### Event `vlf.transmitter`

Struct `VlfTransmitterEvent`. A transmitter came on, went off or changed.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the row. |
| `kind` | text | always | 1.0 | on, off or changed. |
| `rate` | number (bit/s) | optional | 1.0 | The bit rate. |
| `carrier` | frequency | optional | 1.0 | The carrier from the tuned frequency. |
| `level` | number (dB) | optional | 1.0 | The level over the noise. |
| `hint` | text | optional | 1.0 | The listed station near the carrier. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_vlf_msk_clear` | none | none |
| `reset` | `sdrkit_vlf_msk_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `report_s` | `sdrkit_vlf_msk_get_report_s` | `sdrkit_vlf_msk_set_report_s` |
| `rates` | `sdrkit_vlf_msk_get_rates` | `sdrkit_vlf_msk_set_rates` |
| `rate_tolerance` | `sdrkit_vlf_msk_get_rate_tolerance` | `sdrkit_vlf_msk_set_rate_tolerance` |
| `hint_hz` | `sdrkit_vlf_msk_get_hint_hz` | `sdrkit_vlf_msk_set_hint_hz` |
| `stations` | `sdrkit_vlf_msk_get_stations` | `sdrkit_vlf_msk_set_stations` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_rate` | `sdrkit_vlf_msk_gen_get_rate` | `sdrkit_vlf_msk_gen_set_rate` |
| `gen_fc_hz` | `sdrkit_vlf_msk_gen_get_fc_hz` | `sdrkit_vlf_msk_gen_set_fc_hz` |
| `gen_from_s` | `sdrkit_vlf_msk_gen_get_from_s` | `sdrkit_vlf_msk_gen_set_from_s` |
| `gen_to_s` | `sdrkit_vlf_msk_gen_get_to_s` | `sdrkit_vlf_msk_gen_set_to_s` |

## Try it

```sh
sdrkit run vlf_msk --source synth:vlf_msk
sdrkit run vlf_msk --source <driver>:<address> --freq 0.015
```
<!-- generated: end -->
