<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Telephone FSK modems

Bell 103, V.21, V.23 mode 2 and Bell 202 bursts heard in an SSB channel: the standard, sideband and dial error found, the bytes of each burst (CHU's time code as bytes).

Stack id: `fsk_modems`. Group: HF utility.

Telephone FSK modems heard as audio in an SSB channel: Bell 103 (originate and answer), ITU-T V.21 (channels 1 and 2), ITU-T V.23 mode 2 and Bell 202. Bursts of asynchronous bytes (a start bit, eight data bits least significant first, stop bits) are found, the standard, the sideband and the dial error that fit are chosen, and each burst is shown as bytes. The meaning of the bytes is not decoded (the time code of the Canadian time station CHU, sent in Bell 103 answer tones, is shown as bytes).

Standard: ITU-T V.21 (11/1988) and V.23 for the tones and speeds of their channels; the Bell 103 and Bell 202 tone plans

## Tuning

| Property | Value |
|---|---|
| Default frequency | 6.5 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `auto` | Find it | 6.5 MHz |
| `bell103_orig` | Bell 103 originate | 6.5 MHz |
| `bell103_ans` | Bell 103 answer | 6.5 MHz |
| `v21_ch1` | V.21 channel 1 | 6.5 MHz |
| `v21_ch2` | V.21 channel 2 | 6.5 MHz |
| `v23_mode2` | V.23 mode 2 | 6.5 MHz |
| `bell202` | Bell 202 | 6.5 MHz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fsk_modems` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Standard | choice | `auto` | `auto` (Find it), `bell103_orig` (Bell 103 originate), `bell103_ans` (Bell 103 answer), `v21_ch1` (V.21 channel 1), `v21_ch2` (V.21 channel 2), `v23_mode2` (V.23 mode 2), `bell202` (Bell 202) | Find it tries the six standards and names the one that fits. |
| `dial_search_hz` | Dial error searched | frequency | 40 Hz | 0 Hz to 80 Hz | The dial error is tried over plus and minus this, in 10 Hz steps, then refined in 2 Hz steps. |
| `sideband` | Sideband | choice | `auto` | `auto` (The stronger), `upper` (Upper), `lower` (Lower) |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_standard` | Standard | choice | `bell103_ans` | `bell103_orig` (Bell 103 originate), `bell103_ans` (Bell 103 answer), `v21_ch1` (V.21 channel 1), `v21_ch2` (V.21 channel 2), `v23_mode2` (V.23 mode 2), `bell202` (Bell 202) |  |
| `gen_count` | Bytes | whole number | 24 | 1 to 1000 | Random printable bytes from the seed. |
| `gen_stop_bits` | Stop bits | whole number | 1 | 1 to 2 |  |
| `gen_gap_bits` | Idle bits between bytes | whole number | 0 | 0 to 100 |  |
| `gen_idle_s` | Idle mark | duration (s) | 0.4 s | 0 to 10 |  |
| `gen_lead_s` | Silence before | duration (s) | 1 s | 0 to 10 |  |
| `gen_tail_s` | Silence after | duration (s) | 1 s | 0 to 10 |  |
| `gen_lower` | Lower sideband | yes or no | false |  |  |
| `gen_baud_scale` | Speed factor | number | 1 | 0.25 to 4 | The speed of the test signal times this (a wrong speed for a test of false decodes). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `state` | State | whole number |
| `standard` | Standard | text |
| `tones` | Tones | text |
| `last_message` | Last message | text |
| `messages` | Messages | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `rows` | Rows | `time` (time), `kind` (text), `id` (text), `text` (text), `snr` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `modem.message` | Message | `text` (text), `standard` (text), `sideband` (text), `tones` (text), `bytes` (text), `framing_errors` (whole number), `duration` (duration (s)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.4. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FskModemsFields`. The result fields of the telephone FSK modem stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What no public document confirms. |
| `state` | whole number | always | 1.0 | The state: 0 nothing heard, 1 a burst is open, 2 messages read. |
| `standard` | text | optional | 1.0 | The standard of the last message. |
| `tones` | text | optional | 1.0 | The tones, the sideband and the dial error of the last message. |
| `last_message` | text | optional | 1.0 | The bytes, framing errors and length of the last message. |
| `messages` | whole number | always | 1.0 | Messages read. |
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

### Event `modem.message`

Struct `ModemMessageEvent`. A burst of a telephone FSK modem was read.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the row. |
| `standard` | text | always | 1.0 | The standard. |
| `sideband` | text | always | 1.0 | The sideband: upper or lower. |
| `tones` | text | always | 1.0 | The mark and space tones. |
| `bytes` | text | always | 1.0 | The bytes, as hex. |
| `framing_errors` | whole number | always | 1.0 | Framing errors. |
| `duration` | duration (s) | always | 1.0 | The length of the burst. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fsk_modems_clear` | none | none |
| `reset` | `sdrkit_fsk_modems_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_fsk_modems_get_submode` | `sdrkit_fsk_modems_set_submode` |
| `dial_search_hz` | `sdrkit_fsk_modems_get_dial_search_hz` | `sdrkit_fsk_modems_set_dial_search_hz` |
| `sideband` | `sdrkit_fsk_modems_get_sideband` | `sdrkit_fsk_modems_set_sideband` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_standard` | `sdrkit_fsk_modems_gen_get_standard` | `sdrkit_fsk_modems_gen_set_standard` |
| `gen_count` | `sdrkit_fsk_modems_gen_get_count` | `sdrkit_fsk_modems_gen_set_count` |
| `gen_stop_bits` | `sdrkit_fsk_modems_gen_get_stop_bits` | `sdrkit_fsk_modems_gen_set_stop_bits` |
| `gen_gap_bits` | `sdrkit_fsk_modems_gen_get_gap_bits` | `sdrkit_fsk_modems_gen_set_gap_bits` |
| `gen_idle_s` | `sdrkit_fsk_modems_gen_get_idle_s` | `sdrkit_fsk_modems_gen_set_idle_s` |
| `gen_lead_s` | `sdrkit_fsk_modems_gen_get_lead_s` | `sdrkit_fsk_modems_gen_set_lead_s` |
| `gen_tail_s` | `sdrkit_fsk_modems_gen_get_tail_s` | `sdrkit_fsk_modems_gen_set_tail_s` |
| `gen_lower` | `sdrkit_fsk_modems_gen_get_lower` | `sdrkit_fsk_modems_gen_set_lower` |
| `gen_baud_scale` | `sdrkit_fsk_modems_gen_get_baud_scale` | `sdrkit_fsk_modems_gen_set_baud_scale` |

## Try it

```sh
sdrkit run fsk_modems --source synth:fsk_modems
sdrkit run fsk_modems --source synth:fsk_modems --submode auto
sdrkit run fsk_modems --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->
