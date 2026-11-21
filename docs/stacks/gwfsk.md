<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GW FSK

100 Bd 2-FSK bursts of a shore-to-ship data service: free-channel markers with the shore station code, packets with type and subtype, MMSI numbers, position reports, ACKs.

Stack id: `gwfsk`. Group: HF utility.

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

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gwfsk` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `polarity` | Bit sense | choice | `auto` | `auto` (Find it), `normal` (Low tone is 1), `reversed` (High tone is 1) | Find it tries both readings of each burst. |
| `show_unknown` | Show bursts not understood | yes or no | true |  |  |
| `stations` | Shore station names | text | empty |  | code=name pairs separated by semicolons (hex codes), from the host's data file; a code not listed shows Unknown. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_position` | Position report | yes or no | false |  | The position report, a marker, a connection opening and an ACK instead of the default bursts. |
| `gen_gap_s` | Gap | duration (s) | 0.6 s | 0.1 to 10 |  |
| `gen_centre_hz` | Centre | frequency | 1.7 kHz | 400 Hz to 3 kHz |  |
| `gen_reversed` | Reversed sense | yes or no | false |  |  |
| `gen_content` | Bursts | text | `marker FA; text 2/106 0 AB12E3; mmsi 2/101 1 235012389; marker CC; text 2/106 0 WN,7.<` |  | Items separated by semicolons: marker HH (station code), text T/S C CHARS, mmsi T/S C DIGITS, packet T/S C HEX (six bytes), ack, frame200 HEX (14 bytes at 200 Bd). T/S is type/subtype, C the count bit. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `state` | State | whole number |
| `tones` | Tones | text |
| `last_station` | Last shore station | text |
| `bursts` | Bursts | whole number |
| `not_understood` | Not understood | whole number |
| `packets` | Packets | whole number |
| `markers` | Channel markers | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `rows` | Rows | `time` (time), `kind` (text), `id` (text), `text` (text), `snr` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `gw.packet` | Packet | `text` (text), `kind` (text), `packet_type` (whole number), `subtype` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.2. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `GwfskFields`. The result fields of the GW FSK stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What no public document confirms. |
| `state` | whole number | always | 1.0 | The state: 0 nothing heard, 1 bursts heard, 2 bursts read. |
| `tones` | text | optional | 1.0 | The tones of the last burst whose tones were found. |
| `last_station` | text | optional | 1.0 | The code of the last shore station heard. |
| `bursts` | whole number | always | 1.0 | Bursts analysed. |
| `not_understood` | whole number | always | 1.0 | Bursts with tones that were not understood. |
| `packets` | whole number | always | 1.0 | Packets read. |
| `markers` | whole number | always | 1.0 | Channel markers read. |
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

### Event `gw.packet`

Struct `GwPacketEvent`. A burst of GW FSK was read.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the row. |
| `kind` | text | always | 1.0 | Channel marker, Packet, Position or ACK. |
| `packet_type` | whole number | optional | 1.0 | The type of a packet. |
| `subtype` | whole number | optional | 1.0 | The subtype of a packet. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_gwfsk_clear` | none | none |
| `reset` | `sdrkit_gwfsk_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `polarity` | `sdrkit_gwfsk_get_polarity` | `sdrkit_gwfsk_set_polarity` |
| `show_unknown` | `sdrkit_gwfsk_get_show_unknown` | `sdrkit_gwfsk_set_show_unknown` |
| `stations` | `sdrkit_gwfsk_get_stations` | `sdrkit_gwfsk_set_stations` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_position` | `sdrkit_gwfsk_gen_get_position` | `sdrkit_gwfsk_gen_set_position` |
| `gen_gap_s` | `sdrkit_gwfsk_gen_get_gap_s` | `sdrkit_gwfsk_gen_set_gap_s` |
| `gen_centre_hz` | `sdrkit_gwfsk_gen_get_centre_hz` | `sdrkit_gwfsk_gen_set_centre_hz` |
| `gen_reversed` | `sdrkit_gwfsk_gen_get_reversed` | `sdrkit_gwfsk_gen_set_reversed` |
| `gen_content` | `sdrkit_gwfsk_gen_get_content` | `sdrkit_gwfsk_gen_set_content` |

## Try it

```sh
sdrkit run gwfsk --source synth:gwfsk
sdrkit run gwfsk --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->
