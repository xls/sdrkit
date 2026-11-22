<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# VDL Mode 3

VDL Mode 3 TDMA bursts on a 25 kHz channel: burst kinds by training sequence and length, Golay words, Reed-Solomon data with its printable text; voice bursts detected, not decoded.

Stack id: `vdl3`. Group: Aviation.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 136.9 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 1 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 100 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:vdl3` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `sync_threshold` | Training threshold | number | 0.8 | 0.6 to 0.95 | The normalised differential correlation that starts a burst. |
| `evm_max` | Largest phase error | number (% of pi/4) | 30 | 15 to 40 | A burst whose RMS phase error is larger is taken for noise and dropped. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `rs_bit_order` | Reed-Solomon octet bit order | choice | `lsb_first` | `lsb_first` (Least significant bit first), `msb_first` (Most significant bit first) | The order of the bits of each octet of the data word on air. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_adjacent` | Test signal adjacent station | yes or no | false |  | A second station 25 kHz above the channel at the same level. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `rx_state` | Decoder state | whole number |
| `trainings` | Training sequences | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `m_uplink` | M uplink bursts | whole number |
| `m_uplink_3t` | M uplink (3T) bursts | whole number |
| `handoff` | Handoff check bursts | whole number |
| `poll_response` | Poll response bursts | whole number |
| `m_downlink` | M downlink bursts | whole number |
| `net_entry` | Net entry request bursts | whole number |
| `vd_data` | V/D data bursts | whole number |
| `vd_voice` | V/D voice bursts | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `text` | Text | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bursts` | Bursts | `time_s` (number (s)), `kind` (choice), `training` (choice), `symbols` (whole number), `golay_ok` (yes or no), `golay_corrected` (whole number (bits)), `info` (text), `rs_ok` (yes or no), `rs_corrected` (whole number (octets)), `data_hex` (text), `data_text` (text), `level_dbfs` (number (dBFS)), `snr_db` (number (dB)), `cfo_hz` (number (Hz)), `evm_pct` (number (%)), `summary` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `avdata`, version 1.3. The C header `sdrkit/avdata.h`, the registry record `registry/avdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: none.

### Result fields

Struct `Vdl3Fields`. The result fields of VDL Mode 3.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rx_state` | whole number | always | 1.0 | The state: 0 listening, 1 bursts seen, 2 decoding. |
| `trainings` | whole number | always | 1.0 | Training sequences found. |
| `frames_ok` | whole number | always | 1.0 | Good bursts. |
| `frames_bad` | whole number | always | 1.0 | Bursts that failed. |
| `m_uplink` | whole number | always | 1.0 | M uplink bursts. |
| `m_uplink_3t` | whole number | always | 1.0 | M uplink bursts of the 3T configuration. |
| `handoff` | whole number | always | 1.0 | Handoff check bursts. |
| `poll_response` | whole number | always | 1.0 | Poll response bursts. |
| `m_downlink` | whole number | always | 1.0 | M downlink bursts. |
| `net_entry` | whole number | always | 1.0 | Net entry request bursts. |
| `vd_data` | whole number | always | 1.0 | V/D data bursts. |
| `vd_voice` | whole number | always | 1.0 | V/D voice bursts. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the last burst. No value before a burst. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the last burst. No value before a burst. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the last burst. No value before a burst. |
| `text` | text | optional | 1.0 | The summary of the newest burst. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `bursts`

Struct `Vdl3BurstRow`. A row of the bursts. The row key is the place in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the end of the training. |
| `kind` | choice | always | 1.0 | The burst kind. |
| `training` | choice | always | 1.0 | The training sequence. |
| `symbols` | whole number | always | 1.0 | Symbols after the training. |
| `golay_ok` | yes or no | always | 1.0 | Every Golay word decoded. |
| `golay_corrected` | whole number (bits) | always | 1.0 | Bits corrected by Golay. |
| `info` | text | always | 1.0 | The information words. |
| `rs_ok` | yes or no | always | 1.0 | The Reed-Solomon word was good. |
| `rs_corrected` | whole number (octets) | always | 1.0 | Octets corrected by Reed-Solomon. |
| `data_hex` | text | always | 1.0 | The data octets, hex. |
| `data_text` | text | always | 1.0 | The printable text of the data. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the burst. |
| `snr_db` | number (dB) | always | 1.0 | The signal to noise ratio of the burst. |
| `cfo_hz` | number (Hz) | always | 1.0 | The carrier offset of the burst. |
| `evm_pct` | number (%) | always | 1.0 | The error vector magnitude of the burst. |
| `summary` | text | always | 1.0 | The summary of the burst. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_vdl3_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `sync_threshold` | `sdrkit_vdl3_get_sync_threshold` | `sdrkit_vdl3_set_sync_threshold` |
| `evm_max` | `sdrkit_vdl3_get_evm_max` | `sdrkit_vdl3_set_evm_max` |
| `rs_bit_order` | `sdrkit_vdl3_get_rs_bit_order` | `sdrkit_vdl3_set_rs_bit_order` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_adjacent` | `sdrkit_vdl3_gen_get_adjacent` | `sdrkit_vdl3_gen_set_adjacent` |

## Try it

```sh
sdrkit run vdl3 --source synth:vdl3
sdrkit run vdl3 --source iiod:192.168.2.1 --freq 136.9
```
<!-- generated: end -->

## Description

Bursts are found by their four training sequences (S1, S1*, S2, S2*) through a differential correlation and told apart by sequence and length; the carrier comes from the training and a decision-directed loop follows it. The Golay (24, 12) words and the Reed-Solomon (72, 62) word of a data burst are decoded. Data octets and their printable text are shown; voice bursts are detected, not decoded. The system data, header and handoff words are shown as raw values.

Standard: ICAO Annex 10 Volume III Part I, 6.4 (VDL Mode 3).
