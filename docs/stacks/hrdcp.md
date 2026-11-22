<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Meteosat HRDCP

Meteosat high-rate data collection platforms (1200 bit/s offset QPSK, coded): address, sequence, health, data and the platform table, for every carrier in the capture. Experimental: the demodulator's loop values are not confirmed by a public document. Receive only.

Stack id: `hrdcp`. Group: Satellite.

Signal types: Data collection platform.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 402.3 MHz |
| Tuning range | 401 MHz to 403 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 250 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hrdcp` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `threshold_db` | Carrier threshold | number (dB) | 12 | 6 to 25 | A carrier line must stand this far over the local noise floor. |
| `channel_plan` | Channel plan | text | empty |  | The path of a plan file in the host's storage whose numbered bands give the channel numbers (docs/schema/plans.json). Empty for none. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Scene | choice | `0` | `0` (Two platforms), `1` (Platform A alone) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `carriers` | Carriers | whole number |
| `plan` | Channel plan | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |
| `rx.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `reports` | Reports | `at` (duration (s)), `heard_at` (time), `address` (text), `bits_corrected` (whole number), `sequence` (whole number), `type` (text), `health` (text), `compression` (text), `crc_ok` (yes or no), `frames` (whole number), `frames_bad` (whole number), `rs_corrected` (whole number), `bytes` (whole number), `format` (text), `bit_rate_hz` (frequency), `manchester` (yes or no), `freq_hz` (frequency), `channel` (whole number), `snr_db` (number (dB)), `text` (text) |
| `platforms` | Platforms | `address` (text), `channel` (whole number), `snr_db` (number (dB)), `format` (text), `text` (text), `last_heard` (time), `heard` (whole number) |
| `carriers` | Carriers | `offset_hz` (frequency), `channel` (whole number), `snr_db` (number (dB)), `state` (whole number), `bit_rate_hz` (frequency), `bytes` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `spectrum` | Band | Hz | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `hrdcp.report` | HRDCP message | `source` (text), `text` (text), `snr_db` (number (dB)), `family` (choice), `address` (text), `channel` (whole number), `sequence` (whole number), `type` (text), `health` (text), `compression` (text), `bytes` (whole number), `rs_corrected` (whole number), `crc_ok` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.3. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite links (`satellite_links`). Media: none.

### Result fields

Struct `HrdcpFields`. The result fields of Meteosat HRDCP.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `carriers` | whole number | always | 1.0 | The carriers followed now. |
| `plan` | text | always | 1.0 | The channel plan in use. |
| `note` | text | always | 1.0 | What is not confirmed by a public document. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `reports`

Struct `HrdcpReportRow`. A message of the log. The row key is the sequence number of the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time of a message. |
| `heard_at` | time | optional | 1.0 | The wall-clock time of a message (live radios only). |
| `address` | text | always | 1.0 | The address word (8 hex digits). |
| `bits_corrected` | whole number | always | 1.0 | The bits corrected. |
| `sequence` | whole number | always | 1.0 | The sequence counter. |
| `type` | text | always | 1.0 | Self-timed or alert. |
| `health` | text | always | 1.0 | The 10 health bits. |
| `compression` | text | always | 1.0 | The compression of the data. |
| `crc_ok` | yes or no | always | 1.0 | The CRC passed. |
| `frames` | whole number | always | 1.0 | Coded frames of the message. |
| `frames_bad` | whole number | always | 1.0 | Frames whose words did not decode. |
| `rs_corrected` | whole number | always | 1.0 | Reed-Solomon bytes corrected. |
| `bytes` | whole number | always | 1.0 | Platform data bytes. |
| `format` | text | always | 1.0 | ASCII or binary. |
| `bit_rate_hz` | frequency | always | 1.0 | The bit rate. |
| `manchester` | yes or no | always | 1.0 | Manchester coded. |
| `freq_hz` | frequency | always | 1.0 | The frequency of the carrier. |
| `channel` | whole number | optional | 1.0 | The channel number of the plan. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `text` | text | always | 1.0 | The data of the message. |

The column `address` names a row.

### Table `platforms`

Struct `HrdcpPlatformRow`. A platform heard. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address word (8 hex digits). |
| `channel` | whole number | optional | 1.0 | The channel number of the plan. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `format` | text | always | 1.0 | ASCII or binary. |
| `text` | text | always | 1.0 | The data of the last message. |
| `last_heard` | time | optional | 1.0 | When it was last heard. |
| `heard` | whole number | optional | 1.0 | How many times it was heard. |

The column `address` names a row.

### Table `carriers`

Struct `HrdcpCarrierRow`. A carrier followed now. The row key is its place in the list ("000").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `offset_hz` | frequency | always | 1.0 | The offset of the carrier from the tuned frequency. |
| `channel` | whole number | optional | 1.0 | The channel number of the plan. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio. |
| `state` | whole number | always | 1.0 | The carrier state: 0 looking for the marker, 2 decoding. |
| `bit_rate_hz` | frequency | optional | 1.0 | The bit rate. |
| `bytes` | whole number | always | 1.0 | Platform data bytes. |

Default order: the column `snr_db`, highest first.

### Event `hrdcp.report`

Struct `HrdcpReportEvent`. A message received whole with a good CRC. The source is the address, the text the data.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The address of the platform. |
| `text` | text | always | 1.0 | The data of the message. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `address` | text | always | 1.0 | The address word (8 hex digits). |
| `channel` | whole number | optional | 1.0 | The channel number of the plan. |
| `sequence` | whole number | always | 1.0 | The sequence counter. |
| `type` | text | always | 1.0 | Self-timed or alert. |
| `health` | text | always | 1.0 | The 10 health bits. |
| `compression` | text | always | 1.0 | The compression of the data. |
| `bytes` | whole number | always | 1.0 | Platform data bytes. |
| `rs_corrected` | whole number | always | 1.0 | Reed-Solomon bytes corrected. |
| `crc_ok` | yes or no | always | 1.0 | The CRC passed. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hrdcp_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `threshold_db` | `sdrkit_hrdcp_get_threshold_db` | `sdrkit_hrdcp_set_threshold_db` |
| `channel_plan` | `sdrkit_hrdcp_get_channel_plan` | `sdrkit_hrdcp_set_channel_plan` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_hrdcp_gen_get_scene` | `sdrkit_hrdcp_gen_set_scene` |

## Try it

```sh
sdrkit run hrdcp --source synth:hrdcp
sdrkit run hrdcp --source iiod:192.168.2.1 --freq 402.3
```
<!-- generated: end -->

## Description

Each platform sends one message in its slot: a 2 s carrier, a 192-symbol BPSK preamble and marker, then the message in offset QPSK at 1200 symbols a second on each rail. The message (a 12-byte header with the BCH address, the data and a CRC-32) is coded in frames of three interleaved RS(255,223) words, randomised, and with the K = 7 code. The band scanner of `goes_dcs` finds the carriers; each gets a follower that measures the carrier, finds the known symbols, demodulates and decodes. Channel numbers come from a plan file that the host names in `channel_plan`; the stack's crate has `data/meteosat_dcp_plan.json` for it. The field `note` says what is not confirmed by a public document.

Standard: EUMETSAT TD 16, Meteosat Data Collection and Distribution Service, sections 3.2 to 3.4 and 5.3, tables 4 and 11.

## Tuning: notes

The default frequency is the dial of the test signal, not a channel of a regional plan.

## Settings: notes

Advanced settings. The owner works without any of them set.
