<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# OpenSky identifier

Says whether an FM channel carries the OpenSky pattern (9600 baud 4-level FSK in TDMA bursts) from its symbol rate, levels, width and bursts; nothing is decoded.

Stack id: `opensky`. Group: Trunking.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 850 MHz |
| Tuning range | 30 MHz to 1 GHz |
| Sample rate | 240 kHz |
| Smallest sample rate | 96 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:opensky` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_shape` | Test signal shape | choice | `opensky` | `opensky` (OpenSky: 9600 baud, 4 levels, 4800 Hz, 2 of 4 slots), `p25` (4800 baud, 4 levels, 1944 Hz, continuous), `edacs` (9600 baud, 2 levels, 2500 Hz, continuous), `3600` (3600 baud, 2 levels, 1200 Hz, continuous) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `rx_state` | Decoder state | whole number |
| `blocks` | Blocks analysed | whole number |
| `symbol_rate_hz` | Symbol rate | frequency |
| `line_db` | Line strength | number (dB) |
| `levels` | Levels | whole number |
| `deviation_hz` | Deviation | number (Hz) |
| `burst_ms` | Burst period | number (ms) |
| `duty` | Duty | number |
| `width_hz` | Occupied width | number (Hz) |
| `level_dbfs` | Level | number (dBFS) |
| `verdict` | Verdict | choice |
| `text` | Text | text |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `blocks` | Blocks | `time_s` (number (s)), `symbol_rate_hz` (frequency), `levels` (whole number), `width_hz` (number (Hz)), `burst_ms` (number (ms)), `verdict` (choice), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `opensky.verdict` | OpenSky verdict | `verdict` (choice), `symbol_rate_hz` (frequency), `levels` (whole number), `deviation_hz` (number (Hz)), `burst_ms` (number (ms)), `width_hz` (number (Hz)), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `trunk`, version 1.3. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Voice calls (`voice`). Media: none.

### Result fields

Struct `OpenskyFields`. The result fields of the OpenSky identifier.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rx_state` | whole number | always | 1.0 | The state: 0 no input, 1 analysing, 2 a result. |
| `blocks` | whole number | always | 1.0 | Blocks analysed. |
| `symbol_rate_hz` | frequency | optional | 1.0 | Symbol rate found, Hz. |
| `line_db` | number (dB) | optional | 1.0 | The strength of the symbol rate line. |
| `levels` | whole number | optional | 1.0 | The levels. |
| `deviation_hz` | number (Hz) | optional | 1.0 | The deviation. |
| `burst_ms` | number (ms) | optional | 1.0 | The burst period. |
| `duty` | number | optional | 1.0 | The duty. |
| `width_hz` | number (Hz) | optional | 1.0 | The occupied width. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `verdict` | choice | optional | 1.0 | The verdict. |
| `text` | text | optional | 1.0 | The text of a decoded message. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `blocks`

Struct `OpenskyBlockRow`. The measures of one block. The row key is the sequence number of the block.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `symbol_rate_hz` | frequency | always | 1.0 | Symbol rate found, Hz. |
| `levels` | whole number | always | 1.0 | The levels. |
| `width_hz` | number (Hz) | always | 1.0 | The occupied width. |
| `burst_ms` | number (ms) | always | 1.0 | The burst period. |
| `verdict` | choice | always | 1.0 | The verdict. |
| `text` | text | always | 1.0 | The text of a decoded message. |

### Event `opensky.verdict`

Struct `OpenskyVerdictEvent`. The verdict of one block.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `verdict` | choice | always | 1.0 | The verdict. |
| `symbol_rate_hz` | frequency | always | 1.0 | Symbol rate found, Hz. |
| `levels` | whole number | always | 1.0 | The levels. |
| `deviation_hz` | number (Hz) | always | 1.0 | The deviation. |
| `burst_ms` | number (ms) | always | 1.0 | The burst period. |
| `width_hz` | number (Hz) | always | 1.0 | The occupied width. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_opensky_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_shape` | `sdrkit_opensky_gen_get_shape` | `sdrkit_opensky_gen_set_shape` |

## Try it

```sh
sdrkit run opensky --source synth:opensky
sdrkit run opensky --source iiod:192.168.2.1 --freq 850
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
