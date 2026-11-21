<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# EM4100 LF RFID

The 40-bit identity of EM4100 tags from the load modulation on a 100 to 150 kHz reader carrier.

Stack id: `rfid_lf`. Group: Tags and charging.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 250 kHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 10 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:rfid_lf` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `bit_rates` | Bit rates read | choice | `both` | `both` (Both), `rf64` (RF/64), `rf32` (RF/32) |  |
| `codes` | Codes read | choice | `both` | `both` (Both), `manchester` (Manchester), `biphase` (Biphase) |  |
| `same_tag_s` | Same tag held for | number (s) | 3 | 0.5 to 30 | A read of the same tag within this time of its last read adds to its row. |
| `gate_sigmas` | Swing gate | number (sigma) | 5 | 3 to 10 | Level changes are decided only while the swing of the envelope is this many deviations of its noise. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_id` | Tags | text | `0123456789` |  | Tags of 10 hex digits (version and number), separated by commas; each is sent in turn for the time of gen_id_s. |
| `gen_id_s` | Time of each tag | number (s) | 1 | 0.1 to 3600 |  |
| `gen_depth` | Modulation depth | number | 0.08 | 0.001 to 0.9 |  |
| `gen_rf_div` | Bit rate | choice | `rf64` | `rf64` (RF/64), `rf32` (RF/32) |  |
| `gen_coding` | Code | choice | `manchester` | `manchester` (Manchester), `biphase` (Biphase) |  |
| `gen_bit_clock_ppm` | Tag clock error | number (ppm) | 0 | -200000 to 200000 | The bits run this much fast (positive) or slow. |
| `gen_start_s` | Tag starts at | number (s) | 0.05 | 0 to 3600 |  |
| `gen_invert` | Inverted load | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good | whole number |
| `bad` | Refused | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `carrier_db` | Carrier level (relative) | number (dB) |
| `depth_pct` | Modulation depth | number (%) |
| `tags` | Different tags seen | whole number |
| `reads` | Reads | whole number |
| `refused` | Frames refused | whole number |
| `bit_rate` | Bit rate of the last read | choice |
| `coding` | Code of the last read | choice |
| `last_tag` | Last tag | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `tags` | Tags | `time_s` (duration (s)), `tag` (text), `version` (whole number), `number` (whole number), `number_hi` (whole number), `number_lo` (whole number), `bit_rate` (choice), `coding` (choice), `reads` (whole number), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `rfid.tag` | Tag read | `time_s` (duration (s)), `tag` (text), `version` (whole number), `number` (whole number), `number_hi` (whole number), `number_lo` (whole number), `bit_rate` (choice), `coding` (choice), `reads` (whole number), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, the bit clocks and the half-bits so far; the table stays. |

## Typed interface

The stack is in the family `tags`, version 1.0. The C header `sdrkit/tags.h`, the registry record `registry/tags.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `TagsStatusFields`. The status of a stack of the family: the headline, the state, the counts of results and the input level.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The headline. |
| `state` | choice | always | 1.0 | The state. |
| `good` | whole number | always | 1.0 | The good results: what passed every check of its format. |
| `bad` | whole number | always | 1.0 | The refused results: what failed a check. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level of the last report, relative to full scale. |
| `note` | text | always | 1.0 | What the stack reads and what no public document confirms. |

### Result fields

Struct `RfidLfFields`. The result fields of `rfid_lf`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carrier_db` | number (dB) | optional | 1.0 | The level of the reader carrier, relative; no value before the first input. |
| `depth_pct` | number (%) | optional | 1.0 | The modulation depth of the load (swing over carrier); no value without load modulation in the last 2 s. |
| `tags` | whole number | always | 1.0 | The different tags seen. |
| `reads` | whole number | always | 1.0 | The reads: frames that passed every check. |
| `refused` | whole number | always | 1.0 | Frames at the place of the frames of the last read (the same code, the same position in the stream) whose header and stop bit held and whose row or column parity failed. |
| `bit_rate` | choice | optional | 1.0 | The bit rate of the last read. |
| `coding` | choice | optional | 1.0 | The code of the last read. |
| `last_tag` | text | optional | 1.0 | The last tag: version and number, 10 hex digits. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `tags`

Struct `RfidLfTagRow`. A row of the tag table: one tag, read until another tag came or it was not read for the hold time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the last read. |
| `tag` | text | always | 1.0 | Version (2 hex digits) and number (8 hex digits). |
| `version` | whole number | always | 1.0 | The version or customer code, 0 to 255. |
| `number` | whole number | always | 1.0 | The number, 0 to 4294967295. |
| `number_hi` | whole number | always | 1.0 | The upper 16 bits of the number. |
| `number_lo` | whole number | always | 1.0 | The lower 16 bits of the number. |
| `bit_rate` | choice | always | 1.0 | The bit rate. |
| `coding` | choice | always | 1.0 | The code. |
| `reads` | whole number | always | 1.0 | The reads of the row. |
| `level_db` | number (dB) | always | 1.0 | The carrier level, relative. |

### Event `rfid.tag`

Struct `RfidTagEvent`. A tag was read for the first time, or again after another tag or after the hold time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the read. |
| `tag` | text | always | 1.0 | Version (2 hex digits) and number (8 hex digits). |
| `version` | whole number | always | 1.0 | The version or customer code, 0 to 255. |
| `number` | whole number | always | 1.0 | The number, 0 to 4294967295. |
| `number_hi` | whole number | always | 1.0 | The upper 16 bits of the number. |
| `number_lo` | whole number | always | 1.0 | The lower 16 bits of the number. |
| `bit_rate` | choice | always | 1.0 | The bit rate. |
| `coding` | choice | always | 1.0 | The code. |
| `reads` | whole number | always | 1.0 | The reads of the row. |
| `level_db` | number (dB) | always | 1.0 | The carrier level, relative. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_rfid_lf_clear` | none | none |
| `reset` | `sdrkit_rfid_lf_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `bit_rates` | `sdrkit_rfid_lf_get_bit_rates` | `sdrkit_rfid_lf_set_bit_rates` |
| `codes` | `sdrkit_rfid_lf_get_codes` | `sdrkit_rfid_lf_set_codes` |
| `same_tag_s` | `sdrkit_rfid_lf_get_same_tag_s` | `sdrkit_rfid_lf_set_same_tag_s` |
| `gate_sigmas` | `sdrkit_rfid_lf_get_gate_sigmas` | `sdrkit_rfid_lf_set_gate_sigmas` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_id` | `sdrkit_rfid_lf_gen_get_id` | `sdrkit_rfid_lf_gen_set_id` |
| `gen_id_s` | `sdrkit_rfid_lf_gen_get_id_s` | `sdrkit_rfid_lf_gen_set_id_s` |
| `gen_depth` | `sdrkit_rfid_lf_gen_get_depth` | `sdrkit_rfid_lf_gen_set_depth` |
| `gen_rf_div` | `sdrkit_rfid_lf_gen_get_rf_div` | `sdrkit_rfid_lf_gen_set_rf_div` |
| `gen_coding` | `sdrkit_rfid_lf_gen_get_coding` | `sdrkit_rfid_lf_gen_set_coding` |
| `gen_bit_clock_ppm` | `sdrkit_rfid_lf_gen_get_bit_clock_ppm` | `sdrkit_rfid_lf_gen_set_bit_clock_ppm` |
| `gen_start_s` | `sdrkit_rfid_lf_gen_get_start_s` | `sdrkit_rfid_lf_gen_set_start_s` |
| `gen_invert` | `sdrkit_rfid_lf_gen_get_invert` | `sdrkit_rfid_lf_gen_set_invert` |

## Try it

```sh
sdrkit run rfid_lf --source synth:rfid_lf
sdrkit run rfid_lf --source <driver>:<address> --freq 0
```
<!-- generated: end -->
