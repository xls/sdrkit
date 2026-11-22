<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Qi wireless charging

The packets a phone sends to its Qi charger by load modulation of the power signal: signal strength, control error, received power, charge status, configuration and identification.

Stack id: `qi`. Group: Tags and charging.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 250 kHz |
| Smallest sample rate | 24 kHz |
| Signal bandwidth | 12 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:qi` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gate_sigmas` | Swing gate | number (sigma) | 5 | 3 to 10 | Level changes are decided only while the swing of the envelope is this many deviations of its noise. |
| `clock_tolerance_pct` | Bit clock followed within | number (%) | 12 | 5 to 20 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_depth` | Load modulation depth | number | 0.05 | 0.001 to 0.9 |  |
| `gen_bit_clock_ppm` | Bit clock error | number (ppm) | 0 | -200000 to 200000 | The bits run this much fast (positive) or slow. |
| `gen_wander_hz` | Wander frequency | number (Hz) | 0 | 0 to 100 |  |
| `gen_wander_depth` | Wander depth | number | 0 | 0 to 0.5 |  |
| `gen_gap_s` | Time between packets | number (s) | 0.03 | 0.005 to 10 |  |
| `gen_preamble` | Preamble | whole number (bits) | 12 | 11 to 25 |  |
| `gen_packets` | Packets | text | `01 A0; 02 01; 03 F6; 04 50; 05 64; 06 0A; 51 0C 00 00 00 00; 71 11 00 1D 12 34 56 78; p11 04 12` |  | Packets separated by ';': the header and the message in hex, the checksum added. In an entry, 'pN' sets a preamble of N bits, 'x' sends the bytes as given (the last is the checksum), 'fN' sends bit N of the packet inverted. |
| `gen_end_edge_bits` | Change after each packet | number (bit) | 0 | 0 to 10 | One more change this long after the middle of the last stop bit; 0 for none. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good | whole number |
| `bad` | Refused | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `power_signal_db` | Power signal level (relative) | number (dB) |
| `load_modulation_pct` | Load modulation | number (%) |
| `packets_ok` | Packets good | whole number |
| `packets_failed` | Packets failed | whole number |
| `bit_rate` | Bit rate | number (bit/s) |
| `last_message` | Last message | choice |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `packets` | Packets | `time_s` (duration (s)), `header` (text), `message` (choice), `bytes` (text), `value` (whole number), `end_reason` (text), `charge` (text), `power_class` (whole number), `max_power` (whole number), `version` (text), `manufacturer` (text), `device_id` (text), `extended` (yes or no), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `qi.packet` | Packet | `time_s` (duration (s)), `header` (text), `message` (choice), `bytes` (text), `value` (whole number), `end_reason` (text), `charge` (text), `power_class` (whole number), `max_power` (whole number), `version` (text), `manufacturer` (text), `device_id` (text), `extended` (yes or no), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers, the bit clock and the packet in progress; the table stays. |

## Typed interface

The stack is in the family `tags`, version 1.1. The C header `sdrkit/tags.h`, the registry record `registry/tags.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

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

Struct `QiFields`. The result fields of `qi`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `power_signal_db` | number (dB) | optional | 1.0 | The level of the power signal, relative; no value before the first input. |
| `load_modulation_pct` | number (%) | optional | 1.0 | The load modulation (swing over power signal); no value without load modulation in the last 2 s. |
| `packets_ok` | whole number | always | 1.0 | The packets that passed every check. |
| `packets_failed` | whole number | always | 1.0 | The packets that failed a parity, a start or stop bit, the checksum, or lost their bit clock after the header. |
| `bit_rate` | number (bit/s) | optional | 1.0 | The bit rate that the bit clock follows; no value without load modulation in the last 2 s. |
| `last_message` | choice | optional | 1.0 | The type of the last good packet. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `packets`

Struct `QiPacketRow`. A row of the packet table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the end of the packet. |
| `header` | text | always | 1.0 | The header, 2 hex digits. |
| `message` | choice | always | 1.0 | The type. |
| `bytes` | text | always | 1.0 | The message bytes in hex. |
| `value` | whole number | optional | 1.0 | Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms). |
| `end_reason` | text | optional | 1.0 | The reason of an End Power Transfer packet. |
| `charge` | text | optional | 1.0 | The meaning of a Charge Status value. |
| `power_class` | whole number | optional | 1.0 | The power class of a Configuration packet. |
| `max_power` | whole number | optional | 1.0 | The maximum power value of a Configuration packet. |
| `version` | text | optional | 1.0 | The version of an Identification packet, major.minor. |
| `manufacturer` | text | optional | 1.0 | The manufacturer code of an Identification packet, 4 hex digits. |
| `device_id` | text | optional | 1.0 | The basic device identifier of an Identification packet, 8 hex digits (31 bits). |
| `extended` | yes or no | optional | 1.0 | An Extended Identification packet follows an Identification packet. |
| `level_db` | number (dB) | always | 1.0 | The power signal level, relative. |

The column `device_id` names a row.

### Event `qi.packet`

Struct `QiPacketEvent`. A packet passed every check.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the end of the packet. |
| `header` | text | always | 1.0 | The header, 2 hex digits. |
| `message` | choice | always | 1.0 | The type. |
| `bytes` | text | always | 1.0 | The message bytes in hex. |
| `value` | whole number | optional | 1.0 | Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms). |
| `end_reason` | text | optional | 1.0 | The reason of an End Power Transfer packet. |
| `charge` | text | optional | 1.0 | The meaning of a Charge Status value. |
| `power_class` | whole number | optional | 1.0 | The power class of a Configuration packet. |
| `max_power` | whole number | optional | 1.0 | The maximum power value of a Configuration packet. |
| `version` | text | optional | 1.0 | The version of an Identification packet, major.minor. |
| `manufacturer` | text | optional | 1.0 | The manufacturer code of an Identification packet, 4 hex digits. |
| `device_id` | text | optional | 1.0 | The basic device identifier of an Identification packet, 8 hex digits (31 bits). |
| `extended` | yes or no | optional | 1.0 | An Extended Identification packet follows an Identification packet. |
| `level_db` | number (dB) | always | 1.0 | The power signal level, relative. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_qi_clear` | none | none |
| `reset` | `sdrkit_qi_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `gate_sigmas` | `sdrkit_qi_get_gate_sigmas` | `sdrkit_qi_set_gate_sigmas` |
| `clock_tolerance_pct` | `sdrkit_qi_get_clock_tolerance_pct` | `sdrkit_qi_set_clock_tolerance_pct` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_depth` | `sdrkit_qi_gen_get_depth` | `sdrkit_qi_gen_set_depth` |
| `gen_bit_clock_ppm` | `sdrkit_qi_gen_get_bit_clock_ppm` | `sdrkit_qi_gen_set_bit_clock_ppm` |
| `gen_wander_hz` | `sdrkit_qi_gen_get_wander_hz` | `sdrkit_qi_gen_set_wander_hz` |
| `gen_wander_depth` | `sdrkit_qi_gen_get_wander_depth` | `sdrkit_qi_gen_set_wander_depth` |
| `gen_gap_s` | `sdrkit_qi_gen_get_gap_s` | `sdrkit_qi_gen_set_gap_s` |
| `gen_preamble` | `sdrkit_qi_gen_get_preamble` | `sdrkit_qi_gen_set_preamble` |
| `gen_packets` | `sdrkit_qi_gen_get_packets` | `sdrkit_qi_gen_set_packets` |
| `gen_end_edge_bits` | `sdrkit_qi_gen_get_end_edge_bits` | `sdrkit_qi_gen_set_end_edge_bits` |

## Try it

```sh
sdrkit run qi --source synth:qi
sdrkit run qi --source <driver>:<address> --freq 0
```
<!-- generated: end -->
